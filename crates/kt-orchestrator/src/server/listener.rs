//! SSH server listener
//!
//! Accepts incoming connections and spawns handlers for each client.

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Context, Result};
use russh_keys::key::KeyPair;
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::server::handler::{ClientHandler, ConnectionEvent, ServerConfig};
use crate::state::OrchestratorState;

/// SSH server that listens for incoming connections
pub struct SshServer {
    /// Server configuration
    config: ServerConfig,
    /// Shared orchestrator state
    state: Arc<OrchestratorState>,
    /// Cancellation token for graceful shutdown
    cancel: CancellationToken,
    /// Event sender for connection events
    event_tx: mpsc::Sender<ConnectionEvent>,
}

impl SshServer {
    /// Create a new SSH server
    pub fn new(
        host_key: KeyPair,
        state: Arc<OrchestratorState>,
        cancel: CancellationToken,
        event_tx: mpsc::Sender<ConnectionEvent>,
    ) -> Self {
        Self {
            config: ServerConfig::new(host_key),
            state,
            cancel,
            event_tx,
        }
    }

    /// Run the SSH server
    pub async fn run(&self, bind_addr: &str) -> Result<()> {
        let listener = TcpListener::bind(bind_addr)
            .await
            .with_context(|| format!("Failed to bind to {}", bind_addr))?;

        let local_addr = listener.local_addr()?;
        tracing::info!("SSH server listening on {}", local_addr);

        loop {
            tokio::select! {
                // Check for shutdown
                _ = self.cancel.cancelled() => {
                    tracing::info!("SSH server shutting down");
                    break;
                }

                // Accept new connections
                result = listener.accept() => {
                    match result {
                        Ok((socket, peer_addr)) => {
                            self.handle_connection(socket, peer_addr).await;
                        }
                        Err(e) => {
                            tracing::error!("Failed to accept connection: {}", e);
                        }
                    }
                }
            }
        }

        Ok(())
    }

    /// Handle a new incoming connection
    async fn handle_connection(&self, socket: tokio::net::TcpStream, peer_addr: SocketAddr) {
        tracing::info!("New connection from {}", peer_addr);

        let config = Arc::clone(&self.config.ssh_config);
        let state = Arc::clone(&self.state);
        let event_tx = self.event_tx.clone();
        let global_cancel = self.cancel.clone();

        // Create a cancellation token for this specific connection
        let connection_cancel = CancellationToken::new();

        // Spawn a task to handle this connection
        tokio::spawn(async move {
            let handler = ClientHandler::new(state, event_tx, connection_cancel.clone(), peer_addr);

            let result = tokio::select! {
                // Global shutdown
                _ = global_cancel.cancelled() => {
                    tracing::debug!("Connection handler cancelled (global) for {}", peer_addr);
                    return;
                }
                // Per-connection disconnect
                _ = connection_cancel.cancelled() => {
                    tracing::info!("Connection disconnected by request for {}", peer_addr);
                    return;
                }
                result = russh::server::run_stream(config, socket, handler) => result
            };

            match result {
                Ok(_) => {
                    tracing::info!("Connection from {} closed normally", peer_addr);
                }
                Err(e) => {
                    tracing::warn!("Connection from {} closed with error: {}", peer_addr, e);
                }
            }
        });
    }
}

/// Load or generate a host key
pub async fn load_or_generate_host_key(path: &std::path::Path) -> Result<KeyPair> {
    if path.exists() {
        tracing::info!("Loading host key from {:?}", path);
        let key = russh_keys::load_secret_key(path, None)
            .with_context(|| format!("Failed to load host key from {:?}", path))?;
        Ok(key)
    } else {
        tracing::info!("Generating new host key at {:?}", path);

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("Failed to create directory {:?}", parent))?;
        }

        // Generate a new Ed25519 key using ssh-key crate
        let private_key =
            ssh_key::PrivateKey::random(&mut rand::thread_rng(), ssh_key::Algorithm::Ed25519)
                .map_err(|e| anyhow::anyhow!("Failed to generate Ed25519 key: {}", e))?;

        // Encode to OpenSSH format and save
        let openssh_pem = private_key
            .to_openssh(ssh_key::LineEnding::LF)
            .map_err(|e| anyhow::anyhow!("Failed to encode key: {}", e))?;

        // Write to file with restricted permissions
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600) // Owner read/write only
                .open(path)
                .with_context(|| format!("Failed to create key file {:?}", path))?;
            std::io::Write::write_all(&mut file, openssh_pem.as_bytes())
                .with_context(|| "Failed to write key file")?;
        }
        #[cfg(not(unix))]
        {
            tokio::fs::write(path, openssh_pem.as_bytes())
                .await
                .with_context(|| format!("Failed to write key file {:?}", path))?;
        }

        tracing::info!("Generated and saved new host key to {:?}", path);

        // Load it back using russh_keys for compatibility
        let key = russh_keys::load_secret_key(path, None)
            .with_context(|| format!("Failed to load newly generated key from {:?}", path))?;

        Ok(key)
    }
}

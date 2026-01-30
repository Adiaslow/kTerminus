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
        let cancel = self.cancel.clone();

        // Spawn a task to handle this connection
        tokio::spawn(async move {
            let handler = ClientHandler::new(state, event_tx);

            let result = tokio::select! {
                _ = cancel.cancelled() => {
                    tracing::debug!("Connection handler cancelled for {}", peer_addr);
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

        // Generate a new Ed25519 key
        let key = KeyPair::generate_ed25519()
            .ok_or_else(|| anyhow::anyhow!("Failed to generate Ed25519 key"))?;

        // Save to file
        // Note: russh_keys doesn't have a direct save function, so we'll use a workaround
        // For now, we'll just use the key in memory
        // TODO: Implement proper key persistence

        tracing::warn!("Host key persistence not yet implemented - key will change on restart");

        Ok(key)
    }
}

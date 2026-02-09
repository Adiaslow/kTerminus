//! Embedded orchestrator for the desktop app
//!
//! Runs the orchestrator as part of the GUI process, eliminating the need
//! for a separate daemon.

use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;

use kt_core::config::{self, ConfigFile, OrchestratorConfig};
use kt_core::ipc::{IpcEvent, IpcEventEnvelope, StateEpoch};
use kt_orchestrator::connection::TunnelConnection;
use kt_orchestrator::ipc::IpcServer;
use kt_orchestrator::server::{load_or_generate_host_key, ConnectionEvent, SshServer};
use kt_orchestrator::OrchestratorState;

/// Handle to the embedded orchestrator
pub struct EmbeddedOrchestrator {
    /// Cancellation token for shutdown
    cancel: CancellationToken,
    /// Whether the orchestrator is running
    running: bool,
}

impl EmbeddedOrchestrator {
    /// Create a new embedded orchestrator (not yet started)
    pub fn new() -> Self {
        Self {
            cancel: CancellationToken::new(),
            running: false,
        }
    }

    /// Start the embedded orchestrator
    pub async fn start(&mut self) -> Result<()> {
        if self.running {
            tracing::info!("Orchestrator already running");
            return Ok(());
        }

        tracing::info!("Starting embedded orchestrator...");

        // Load configuration
        let config = load_config()?;

        // Load or generate host key
        let host_key = load_or_generate_host_key(&config.host_key_path).await?;
        let host_key_fingerprint = host_key
            .clone_public_key()
            .map(|pk| pk.fingerprint())
            .unwrap_or_else(|e| {
                tracing::warn!("Could not extract public key for fingerprint: {}", e);
                "<unknown>".to_string()
            });
        tracing::info!("Host key fingerprint: {}", host_key_fingerprint);

        // Create orchestrator state
        let state = Arc::new(OrchestratorState::new(config.clone()));

        // Create event channel for connection events
        let (event_tx, mut event_rx) = mpsc::channel::<ConnectionEvent>(256);

        // Start IPC server
        let ipc_address = config.ipc_address();
        let ipc_server = Arc::new(
            IpcServer::new(ipc_address.clone(), Arc::clone(&state))?
                .with_shutdown_token(self.cancel.clone()),
        );
        let ipc_event_tx = ipc_server.event_sender();

        // Spawn event handler
        let state_clone = Arc::clone(&state);
        let epoch_clone = Arc::clone(&state.epoch);
        let ipc_event_tx_clone = ipc_event_tx.clone();
        tokio::spawn(async move {
            while let Some(event) = event_rx.recv().await {
                handle_connection_event(&state_clone, &epoch_clone, event, &ipc_event_tx_clone).await;
            }
        });

        // Spawn IPC server task
        let ipc_server_clone = Arc::clone(&ipc_server);
        let cancel_ipc = self.cancel.clone();
        tokio::spawn(async move {
            tokio::select! {
                result = ipc_server_clone.run() => {
                    if let Err(e) = result {
                        tracing::error!("IPC server error: {}", e);
                    }
                }
                _ = cancel_ipc.cancelled() => {
                    tracing::info!("IPC server shutting down");
                }
            }
        });
        tracing::info!("IPC server listening on {}", ipc_address);

        // Start health monitor
        let health_monitor = kt_orchestrator::connection::HealthMonitor::new(
            config.heartbeat_interval,
            config.heartbeat_timeout,
        );
        let _health_handle = health_monitor.spawn(Arc::clone(&state), self.cancel.clone());
        tracing::info!(
            "Health monitor started (interval={:?}, timeout={:?})",
            config.heartbeat_interval,
            config.heartbeat_timeout
        );

        // Create and run SSH server
        let server = SshServer::new(host_key, Arc::clone(&state), self.cancel.clone(), event_tx);

        let bind_addr = config.bind_address.clone();
        let cancel = self.cancel.clone();

        // Spawn SSH server in background
        tokio::spawn(async move {
            tracing::info!("Starting SSH server on {}", bind_addr);
            if let Err(e) = server.run(&bind_addr).await {
                if !cancel.is_cancelled() {
                    tracing::error!("SSH server error: {}", e);
                }
            }
            tracing::info!("SSH server stopped");
        });

        self.running = true;
        tracing::info!("Embedded orchestrator started");

        Ok(())
    }

    /// Stop the embedded orchestrator
    pub fn stop(&mut self) {
        if !self.running {
            return;
        }

        tracing::info!("Stopping embedded orchestrator...");
        self.cancel.cancel();
        self.running = false;
    }

    /// Check if the orchestrator is running
    pub fn is_running(&self) -> bool {
        self.running
    }
}

impl Default for EmbeddedOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for EmbeddedOrchestrator {
    fn drop(&mut self) {
        self.stop();
        // Clean up token file on shutdown
        if let Err(e) = kt_core::remove_ipc_token() {
            tracing::warn!("Failed to remove IPC token file: {}", e);
        }
    }
}

/// Load orchestrator configuration
fn load_config() -> Result<OrchestratorConfig> {
    let default_path = config::default_config_path();
    tracing::info!("Looking for config at {:?}", default_path);
    if default_path.exists() {
        // Config file has [orchestrator] section, so we need to use ConfigFile wrapper
        let config_file: ConfigFile = config::load_config(&default_path)
            .with_context(|| format!("Failed to load config from {:?}", default_path))?;
        let config = config_file.orchestrator;
        tracing::info!("Loaded orchestrator config");
        Ok(config)
    } else {
        tracing::info!("Config file not found, using default configuration");
        Ok(OrchestratorConfig::default())
    }
}

/// Handle connection events from SSH handlers
async fn handle_connection_event(
    state: &OrchestratorState,
    epoch: &Arc<StateEpoch>,
    event: ConnectionEvent,
    ipc_event_tx: &broadcast::Sender<IpcEventEnvelope>,
) {
    match event {
        ConnectionEvent::MachineConnected {
            machine_id,
            alias,
            hostname,
            os,
            arch,
            command_tx,
            cancel,
        } => {
            tracing::info!(
                "Machine connected: {} (alias: {}, hostname: {}, os: {}, arch: {})",
                machine_id,
                alias,
                hostname,
                os,
                arch
            );
            // Register in connection pool with command channel
            state.coordinator.connections.insert(TunnelConnection::new(
                machine_id.clone(),
                Some(alias.clone()),
                Some(hostname.clone()),
                os.clone(),
                arch.clone(),
                command_tx,
                cancel,
            ));

            // Broadcast to IPC clients wrapped in envelope
            let event = IpcEvent::MachineConnected(kt_core::ipc::MachineInfo {
                id: machine_id.to_string(),
                alias: Some(alias),
                hostname,
                os,
                arch,
                status: kt_core::ipc::MachineStatus::Connected,
                connected_at: None,
                last_heartbeat: None,
                session_count: 0,
                tags: vec![],
            });
            let _ = ipc_event_tx.send(epoch.wrap_event(event));
        }

        ConnectionEvent::MachineDisconnected { machine_id } => {
            tracing::info!("Machine disconnected: {}", machine_id);
            // Remove from connection pool
            state.coordinator.connections.remove(&machine_id);

            // Broadcast to IPC clients wrapped in envelope
            let event = IpcEvent::MachineDisconnected {
                machine_id: machine_id.to_string(),
            };
            let _ = ipc_event_tx.send(epoch.wrap_event(event));
        }

        ConnectionEvent::SessionCreated {
            machine_id,
            session_id,
            pid,
        } => {
            tracing::info!(
                "Session {} ready on {} (pid={})",
                session_id,
                machine_id,
                pid
            );
            // Update session with PID from agent
            state.coordinator.sessions.set_pid(session_id, pid);

            // Broadcast to IPC clients wrapped in envelope
            let event = IpcEvent::SessionCreated(kt_core::ipc::SessionInfo {
                id: session_id.to_string(),
                machine_id: machine_id.to_string(),
                shell: None,
                created_at: String::new(),
                pid: Some(pid),
                size: None,
            });
            let _ = ipc_event_tx.send(epoch.wrap_event(event));
        }

        ConnectionEvent::SessionClosed {
            machine_id,
            session_id,
        } => {
            tracing::info!("Session {} closed on {}", session_id, machine_id);
            // Remove session from session manager
            state.coordinator.sessions.remove(session_id);

            // Broadcast to IPC clients wrapped in envelope
            let event = IpcEvent::SessionClosed {
                session_id: session_id.to_string(),
            };
            let _ = ipc_event_tx.send(epoch.wrap_event(event));
        }

        ConnectionEvent::SessionData {
            machine_id: _,
            session_id,
            data,
        } => {
            // Broadcast to IPC clients wrapped in envelope
            let event = IpcEvent::TerminalOutput {
                session_id: session_id.to_string(),
                data,
            };
            let _ = ipc_event_tx.send(epoch.wrap_event(event));
        }
    }
}

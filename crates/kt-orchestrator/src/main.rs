//! k-Terminus Orchestrator Daemon
//!
//! The orchestrator runs on the local machine and accepts incoming
//! reverse SSH connections from remote agents.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;
use tokio::sync::{broadcast, mpsc};
use tokio_util::sync::CancellationToken;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use kt_core::ipc::IpcEvent;

use kt_core::config::{self, OrchestratorConfig};
use kt_orchestrator::ipc::IpcServer;
use kt_orchestrator::server::{load_or_generate_host_key, ConnectionEvent, SshServer};
use kt_orchestrator::OrchestratorState;

#[derive(Parser)]
#[command(name = "kt-orchestrator")]
#[command(about = "k-Terminus orchestrator daemon")]
#[command(version)]
struct Args {
    /// Path to configuration file
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Bind address (overrides config)
    #[arg(short, long)]
    bind: Option<String>,

    /// Run in foreground with verbose output
    #[arg(short, long)]
    foreground: bool,

    /// Log level (error, warn, info, debug, trace)
    #[arg(long, default_value = "info")]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    let log_level = if args.foreground {
        "debug"
    } else {
        &args.log_level
    };
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| log_level.into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("k-Terminus Orchestrator starting...");

    // Load configuration
    let config = if let Some(config_path) = &args.config {
        config::load_config(config_path)
            .with_context(|| format!("Failed to load config from {:?}", config_path))?
    } else {
        let default_path = config::default_config_path();
        if default_path.exists() {
            config::load_config(&default_path).unwrap_or_else(|e| {
                tracing::warn!("Failed to load config from {:?}: {}", default_path, e);
                OrchestratorConfig::default()
            })
        } else {
            tracing::info!("Using default configuration");
            OrchestratorConfig::default()
        }
    };

    // Override bind address if specified
    let bind_addr = args.bind.unwrap_or_else(|| config.bind_address.clone());

    // Load or generate host key
    let host_key = load_or_generate_host_key(&config.host_key_path).await?;
    tracing::info!(
        "Host key fingerprint: {}",
        host_key.clone_public_key().unwrap().fingerprint()
    );

    // Create orchestrator state
    let state = Arc::new(OrchestratorState::new(config.clone()));

    // Create cancellation token for graceful shutdown
    let cancel = CancellationToken::new();

    // Setup signal handlers
    let cancel_clone = cancel.clone();
    tokio::spawn(async move {
        let ctrl_c = tokio::signal::ctrl_c();

        #[cfg(unix)]
        let terminate = async {
            tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("failed to install signal handler")
                .recv()
                .await;
        };

        #[cfg(not(unix))]
        let terminate = std::future::pending::<()>();

        tokio::select! {
            _ = ctrl_c => {
                tracing::info!("Received Ctrl+C, initiating shutdown...");
            }
            _ = terminate => {
                tracing::info!("Received SIGTERM, initiating shutdown...");
            }
        }

        cancel_clone.cancel();
    });

    // Create event channel for connection events
    let (event_tx, mut event_rx) = mpsc::channel::<ConnectionEvent>(256);

    // Start IPC server for CLI/GUI communication
    // (Create early so we can get the event sender for the event handler)
    let ipc_address = config.ipc_address();
    let ipc_server = Arc::new(
        IpcServer::new(ipc_address.clone(), Arc::clone(&state)).with_shutdown_token(cancel.clone()),
    );
    let ipc_event_tx = ipc_server.event_sender();

    // Spawn event handler
    let state_clone = Arc::clone(&state);
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            handle_connection_event(&state_clone, event, &ipc_event_tx).await;
        }
    });

    // Spawn IPC server task
    let ipc_server_clone = Arc::clone(&ipc_server);
    let cancel_ipc = cancel.clone();
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
    let _health_handle = health_monitor.spawn(Arc::clone(&state), cancel.clone());
    tracing::info!(
        "Health monitor started (interval={:?}, timeout={:?})",
        config.heartbeat_interval,
        config.heartbeat_timeout
    );

    // Create and run SSH server
    let server = SshServer::new(host_key, Arc::clone(&state), cancel.clone(), event_tx);

    tracing::info!("Starting SSH server on {}", bind_addr);
    server.run(&bind_addr).await?;

    tracing::info!("Orchestrator shutdown complete");
    Ok(())
}

/// Handle connection events from SSH handlers
async fn handle_connection_event(
    state: &OrchestratorState,
    event: ConnectionEvent,
    ipc_event_tx: &broadcast::Sender<IpcEvent>,
) {
    use kt_orchestrator::connection::TunnelConnection;

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
            state.connections.insert(TunnelConnection::new(
                machine_id.clone(),
                Some(alias.clone()),
                Some(hostname.clone()),
                os.clone(),
                arch.clone(),
                command_tx,
                cancel,
            ));

            // Broadcast to IPC clients
            let _ = ipc_event_tx.send(IpcEvent::MachineConnected(kt_core::ipc::MachineInfo {
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
            }));
        }

        ConnectionEvent::MachineDisconnected { machine_id } => {
            tracing::info!("Machine disconnected: {}", machine_id);
            // Remove from connection pool
            state.connections.remove(&machine_id);

            // Broadcast to IPC clients
            let _ = ipc_event_tx.send(IpcEvent::MachineDisconnected {
                machine_id: machine_id.to_string(),
            });
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
            state.sessions.set_pid(session_id, pid);

            // Broadcast to IPC clients
            let _ = ipc_event_tx.send(IpcEvent::SessionCreated(kt_core::ipc::SessionInfo {
                id: session_id.to_string(),
                machine_id: machine_id.to_string(),
                shell: None,
                created_at: String::new(),
                pid: Some(pid),
                size: None,
            }));
        }

        ConnectionEvent::SessionClosed {
            machine_id,
            session_id,
        } => {
            tracing::info!("Session {} closed on {}", session_id, machine_id);
            // Remove session from session manager
            state.sessions.remove(session_id);

            // Broadcast to IPC clients
            let _ = ipc_event_tx.send(IpcEvent::SessionClosed {
                session_id: session_id.to_string(),
            });
        }

        ConnectionEvent::SessionData {
            machine_id,
            session_id,
            data,
        } => {
            tracing::trace!(
                "Session data: {} bytes from {} on {}",
                data.len(),
                session_id,
                machine_id
            );
            // Broadcast to IPC clients
            let event = IpcEvent::TerminalOutput {
                session_id: session_id.to_string(),
                data,
            };
            // Ignore send errors (no subscribers is fine)
            let _ = ipc_event_tx.send(event);
        }
    }
}

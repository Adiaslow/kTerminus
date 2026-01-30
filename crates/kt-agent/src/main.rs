//! k-Terminus Agent Daemon
//!
//! The agent runs on remote machines and establishes a reverse SSH tunnel
//! to the orchestrator, enabling terminal session management.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;
use tokio::sync::Mutex;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use kt_core::config::{self, AgentConfig};
use kt_agent::pty::PtyManager;
use kt_agent::tunnel::{ExponentialBackoff, TunnelConnector, TunnelEvent};

#[derive(Parser)]
#[command(name = "kt-agent")]
#[command(about = "k-Terminus agent daemon")]
#[command(version)]
struct Args {
    /// Path to configuration file
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Orchestrator address (overrides config)
    #[arg(short, long)]
    orchestrator: Option<String>,

    /// Path to private key (overrides config)
    #[arg(short, long)]
    key: Option<PathBuf>,

    /// Machine alias (overrides config)
    #[arg(long)]
    alias: Option<String>,

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
    let log_level = if args.foreground { "debug" } else { &args.log_level };
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| log_level.into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("k-Terminus Agent starting...");

    // Load configuration
    let mut config = if let Some(config_path) = &args.config {
        config::load_config(config_path)
            .with_context(|| format!("Failed to load config from {:?}", config_path))?
    } else {
        let default_path = config::default_config_dir().join("agent.toml");
        if default_path.exists() {
            config::load_config(&default_path).unwrap_or_else(|e| {
                tracing::warn!("Failed to load config from {:?}: {}", default_path, e);
                AgentConfig::default()
            })
        } else {
            tracing::info!("Using default configuration");
            AgentConfig::default()
        }
    };

    // Apply command-line overrides
    if let Some(orchestrator) = args.orchestrator {
        config.orchestrator_address = orchestrator;
    }
    if let Some(key) = args.key {
        config.private_key_path = key;
    }
    if let Some(alias) = args.alias {
        config.alias = Some(alias);
    }

    tracing::info!("Connecting to orchestrator at {}", config.orchestrator_address);
    tracing::info!("Using private key from {:?}", config.private_key_path);
    tracing::info!("Machine alias: {}", config.machine_alias());

    // Create tunnel connector
    let connector = TunnelConnector::new(config.clone())
        .with_context(|| "Failed to create tunnel connector")?;

    // Create PTY manager
    let pty_manager = Arc::new(Mutex::new(PtyManager::with_defaults(
        config.default_shell.clone(),
        config.default_env.clone(),
    )));

    // Main loop with reconnection
    loop {
        // Create backoff for this connection attempt
        let backoff = ExponentialBackoff::from_config(&config.backoff);

        // Connect to orchestrator
        let mut tunnel = match connector.connect_with_retry(backoff).await {
            Ok(tunnel) => tunnel,
            Err(e) => {
                tracing::error!("Failed to connect: {}", e);
                // Wait before trying again
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                continue;
            }
        };

        tracing::info!("Connected to orchestrator, entering event loop");

        // Event loop
        let disconnect_reason = run_event_loop(&mut tunnel, Arc::clone(&pty_manager)).await;

        tracing::warn!("Disconnected: {:?}", disconnect_reason);

        // Clean up any active sessions
        {
            let mut manager = pty_manager.lock().await;
            for session_id in manager.list_sessions() {
                manager.close(session_id);
            }
        }

        // Brief delay before reconnecting
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        tracing::info!("Reconnecting...");
    }
}

/// Run the main event loop for handling orchestrator events
async fn run_event_loop(
    tunnel: &mut kt_agent::tunnel::ActiveTunnel,
    pty_manager: Arc<Mutex<PtyManager>>,
) -> String {
    use kt_protocol::SessionId;

    loop {
        // Check for orchestrator events
        let event = match tunnel.recv_event().await {
            Some(event) => event,
            None => return "Channel closed".to_string(),
        };

        match event {
            TunnelEvent::Registered { accepted, reason } => {
                if accepted {
                    tracing::info!("Registration accepted by orchestrator");
                } else {
                    tracing::error!("Registration rejected: {:?}", reason);
                    return format!("Registration rejected: {:?}", reason);
                }
            }

            TunnelEvent::CreateSession { session_id, shell, env, size } => {
                tracing::info!("Creating session {}", session_id);

                let mut manager = pty_manager.lock().await;
                match manager.create_session(session_id, shell, env, size) {
                    Ok(pid) => {
                        // Send session ready notification
                        if let Err(e) = tunnel.send_session_ready(session_id, pid).await {
                            tracing::error!("Failed to send session ready: {}", e);
                        }

                        // Spawn a task to read from the PTY and send to orchestrator
                        // Note: In a real implementation, we'd need to handle this more carefully
                        // with async I/O. For now, we'll use a simple polling approach.
                    }
                    Err(e) => {
                        tracing::error!("Failed to create session: {}", e);
                        // TODO: Send error back to orchestrator
                    }
                }
            }

            TunnelEvent::SessionData { session_id, data } => {
                let mut manager = pty_manager.lock().await;
                if let Err(e) = manager.write(session_id, &data) {
                    tracing::error!("Failed to write to session {}: {}", session_id, e);
                }
            }

            TunnelEvent::SessionResize { session_id, size } => {
                let mut manager = pty_manager.lock().await;
                if let Err(e) = manager.resize(session_id, size) {
                    tracing::error!("Failed to resize session {}: {}", session_id, e);
                }
            }

            TunnelEvent::SessionClose { session_id } => {
                tracing::info!("Closing session {}", session_id);
                let mut manager = pty_manager.lock().await;
                let exit_code = manager.close(session_id);

                // Send close notification
                if let Err(e) = tunnel.send_session_close(session_id, exit_code).await {
                    tracing::error!("Failed to send session close: {}", e);
                }
            }

            TunnelEvent::Heartbeat { timestamp } => {
                tracing::trace!("Heartbeat received, sending ack");
                if let Err(e) = tunnel.send_heartbeat_ack(timestamp).await {
                    tracing::error!("Failed to send heartbeat ack: {}", e);
                }
            }

            TunnelEvent::Disconnected => {
                return "Disconnected by orchestrator".to_string();
            }
        }

        // Poll active sessions for output
        // Note: This is a simplified approach. A production implementation would
        // use proper async I/O with tokio::spawn for each session reader.
        {
            let mut manager = pty_manager.lock().await;
            let sessions: Vec<SessionId> = manager.list_sessions();

            for session_id in sessions {
                // Check if session exited
                if let Ok(Some(exit_code)) = manager.try_wait(session_id) {
                    if let Err(e) = tunnel.send_session_close(session_id, Some(exit_code)).await {
                        tracing::error!("Failed to send session close: {}", e);
                    }
                    manager.close(session_id);
                }
            }
        }
    }
}

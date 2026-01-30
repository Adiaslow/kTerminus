//! k-Terminus Orchestrator Daemon
//!
//! The orchestrator runs on the local machine and accepts incoming
//! reverse SSH connections from remote agents.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use kt_core::config::{self, OrchestratorConfig};
use kt_orchestrator::auth::AuthorizedKeys;
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
    let log_level = if args.foreground { "debug" } else { &args.log_level };
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
    tracing::info!("Host key fingerprint: {}", host_key.clone_public_key().unwrap().fingerprint());

    // Load authorized keys
    let auth_keys = if config.auth_keys.is_empty() {
        tracing::warn!("No authorized keys configured - all connections will be rejected");
        AuthorizedKeys::new()
    } else {
        AuthorizedKeys::load_from_files(&config.auth_keys)?
    };

    if auth_keys.is_empty() {
        tracing::warn!("No valid authorized keys found - all connections will be rejected");
    } else {
        tracing::info!("Loaded {} authorized keys", auth_keys.len());
    }

    // Create orchestrator state
    let state = Arc::new(OrchestratorState::with_auth(config.clone(), auth_keys));

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

    // Spawn event handler
    let state_clone = Arc::clone(&state);
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            handle_connection_event(&state_clone, event).await;
        }
    });

    // Create and run SSH server
    let server = SshServer::new(host_key, Arc::clone(&state), cancel.clone(), event_tx);

    tracing::info!("Starting SSH server on {}", bind_addr);
    server.run(&bind_addr).await?;

    tracing::info!("Orchestrator shutdown complete");
    Ok(())
}

/// Handle connection events from SSH handlers
async fn handle_connection_event(state: &OrchestratorState, event: ConnectionEvent) {
    match event {
        ConnectionEvent::MachineConnected {
            machine_id,
            alias,
            hostname,
        } => {
            tracing::info!(
                "Machine connected: {} (alias: {}, hostname: {})",
                machine_id,
                alias,
                hostname
            );
            // TODO: Register in connection pool
        }

        ConnectionEvent::MachineDisconnected { machine_id } => {
            tracing::info!("Machine disconnected: {}", machine_id);
            // TODO: Remove from connection pool
        }

        ConnectionEvent::SessionCreated {
            machine_id,
            session_id,
        } => {
            tracing::info!("Session created: {} on {}", session_id, machine_id);
            // TODO: Track session
        }

        ConnectionEvent::SessionClosed {
            machine_id,
            session_id,
        } => {
            tracing::info!("Session closed: {} on {}", session_id, machine_id);
            // TODO: Remove session
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
            // TODO: Route to attached client
        }
    }
}

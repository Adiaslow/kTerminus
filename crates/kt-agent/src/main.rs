//! k-Terminus Agent Daemon
//!
//! The agent runs on remote machines and establishes a reverse SSH tunnel
//! to the orchestrator, enabling terminal session management.
//!
//! With Tailscale, authentication is automatic - the orchestrator verifies
//! that the agent is in the same tailnet.

use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;
use tokio::sync::{mpsc, Mutex};
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use kt_agent::pty::PtyManager;
use kt_agent::tunnel::{ConnectionError, ExponentialBackoff, TunnelConnector, TunnelEvent};
use kt_core::config::{self, AgentConfig};
use kt_core::tailscale;
use kt_protocol::SessionId;

/// Message sent from PTY reader tasks to the main event loop
struct PtyOutput {
    session_id: SessionId,
    data: Vec<u8>,
}

#[derive(Parser)]
#[command(name = "kt-agent")]
#[command(about = "k-Terminus agent - connects to orchestrator via Tailscale")]
#[command(version)]
struct Args {
    /// Orchestrator to connect to (Tailscale hostname or IP)
    /// Example: my-laptop or my-laptop.tailnet-abc.ts.net
    #[arg(short, long)]
    orchestrator: Option<String>,

    /// Pairing code from orchestrator (auto-discovers orchestrator)
    /// Example: ABC123
    #[arg(long)]
    code: Option<String>,

    /// Path to private key (auto-generated if not specified)
    #[arg(short, long)]
    key: Option<PathBuf>,

    /// Machine alias (defaults to hostname)
    #[arg(long)]
    alias: Option<String>,

    /// Run in foreground with verbose output
    #[arg(short, long)]
    foreground: bool,

    /// Log level (error, warn, info, debug, trace)
    #[arg(long, default_value = "info")]
    log_level: String,

    /// Path to configuration file
    #[arg(short, long)]
    config: Option<PathBuf>,

    /// Local testing mode - skip Tailscale requirement
    #[arg(long)]
    local: bool,
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

    tracing::info!("k-Terminus Agent starting...");

    // Check Tailscale (skip in local mode)
    let ts_info = if args.local {
        tracing::info!("Local testing mode - skipping Tailscale check");
        None
    } else {
        let info = tailscale::get_tailscale_info()
            .context("Failed to check Tailscale status")?
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Tailscale is not installed.\n\n{}\n\nFor local testing, use --local flag.",
                    tailscale::get_install_instructions()
                )
            })?;

        if !info.logged_in {
            anyhow::bail!(
                "Tailscale is not logged in. Please run:\n\n    sudo tailscale up\n\nThen try again.\n\nFor local testing, use --local flag."
            );
        }

        tracing::info!(
            "Tailscale connected: {} ({})",
            info.device_name,
            info.ip
        );
        Some(info)
    };

    // Load configuration
    let config_path = args
        .config
        .clone()
        .unwrap_or_else(|| config::default_config_dir().join("agent.toml"));

    let mut config = if config_path.exists() {
        config::load_config(&config_path).unwrap_or_else(|e| {
            tracing::warn!("Failed to load config from {:?}: {}", config_path, e);
            AgentConfig::default()
        })
    } else {
        AgentConfig::default()
    };

    // Apply command-line overrides
    if let Some(orchestrator) = args.orchestrator {
        let resolved = if let Some(ref ts) = ts_info {
            // Resolve short name to full Tailscale hostname
            tailscale::resolve_device_name(&orchestrator, &ts.tailnet)
        } else {
            // Local mode - use address as-is
            orchestrator
        };
        // Add default port if not specified
        config.orchestrator_address = if resolved.contains(':') {
            resolved
        } else {
            format!("{}:2222", resolved)
        };
    } else if let Some(code) = args.code {
        // Use pairing code discovery
        if args.local {
            anyhow::bail!("Pairing code discovery requires Tailscale (cannot use --local with --code)");
        }

        let discovered = kt_agent::discover_orchestrator(&code).await
            .context("Failed to discover orchestrator")?;

        tracing::info!(
            "Discovered orchestrator: {} ({})",
            discovered.peer.device_name,
            discovered.ssh_address
        );
        config.orchestrator_address = discovered.ssh_address;
    } else if config.orchestrator_address.is_empty() {
        // No orchestrator specified and not in config - prompt for pairing code
        if args.local {
            anyhow::bail!(
                "No orchestrator specified. Use --orchestrator <hostname> to specify the orchestrator address."
            );
        }

        tracing::info!("No orchestrator specified. Starting pairing code discovery...");
        let code = kt_agent::prompt_pairing_code()
            .context("Failed to get pairing code")?;

        let discovered = kt_agent::discover_orchestrator(&code).await
            .context("Failed to discover orchestrator")?;

        tracing::info!(
            "Discovered orchestrator: {} ({})",
            discovered.peer.device_name,
            discovered.ssh_address
        );
        config.orchestrator_address = discovered.ssh_address;
    }
    if let Some(key) = args.key {
        config.private_key_path = key;
    }
    if let Some(alias) = args.alias {
        config.alias = Some(alias);
    }

    // Ensure SSH key exists (auto-generate if needed)
    ensure_ssh_key(&config.private_key_path).await?;

    tracing::info!(
        "Connecting to orchestrator at {}",
        config.orchestrator_address
    );
    tracing::info!("Machine alias: {}", config.machine_alias());

    // Create tunnel connector (no host key verification needed - Tailscale handles security)
    let connector =
        TunnelConnector::new(config.clone()).context("Failed to create tunnel connector")?;

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
            Err(ConnectionError::AuthRejected) => {
                tracing::error!(
                    "Connection rejected by orchestrator. Make sure both machines are on the same Tailscale network."
                );
                return Err(anyhow::anyhow!(
                    "Authentication rejected - not on same Tailscale network"
                ));
            }
            Err(ConnectionError::HostKeyRejected { .. }) => {
                // With Tailscale, this shouldn't happen - but handle it gracefully
                tracing::warn!(
                    "Host key rejected - this is unexpected with Tailscale. Retrying..."
                );
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                continue;
            }
            Err(e) => {
                tracing::error!("Failed to connect: {}", e);
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                continue;
            }
        };

        tracing::info!("Connected to orchestrator, entering event loop");

        // Create channel for PTY output (reader tasks -> event loop)
        let (pty_output_tx, pty_output_rx) = mpsc::channel::<PtyOutput>(256);

        // Track reader tasks and their cancellation tokens for cleanup
        let reader_tasks: HashMap<SessionId, (JoinHandle<()>, CancellationToken)> = HashMap::new();

        // Event loop
        let disconnect_reason = run_event_loop(
            &mut tunnel,
            Arc::clone(&pty_manager),
            pty_output_tx,
            pty_output_rx,
            reader_tasks,
        )
        .await;

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
    pty_output_tx: mpsc::Sender<PtyOutput>,
    mut pty_output_rx: mpsc::Receiver<PtyOutput>,
    mut reader_tasks: HashMap<SessionId, (JoinHandle<()>, CancellationToken)>,
) -> String {
    loop {
        tokio::select! {
            // Handle events from the orchestrator
            event = tunnel.recv_event() => {
                let event = match event {
                    Some(event) => event,
                    None => return "Channel closed".to_string(),
                };

                match event {
                    TunnelEvent::Registered { accepted, reason } => {
                        if accepted {
                            tracing::info!("Registration accepted by orchestrator");
                        } else {
                            // Issue #18: Detect and handle protocol version mismatch
                            let reason_str = reason.as_deref().unwrap_or("Unknown reason");

                            if reason_str.contains("Protocol version mismatch") {
                                tracing::error!(
                                    "Protocol version mismatch detected!\n\
                                     {}\n\
                                     Please update the agent to match the orchestrator version.",
                                    reason_str
                                );
                            } else {
                                tracing::error!("Registration rejected: {}", reason_str);
                            }

                            // Gracefully cancel all reader tasks and wait for cleanup
                            for (session_id, (handle, cancel_token)) in reader_tasks.drain() {
                                cancel_token.cancel();
                                let _ = tokio::time::timeout(
                                    std::time::Duration::from_millis(500),
                                    handle
                                ).await;
                                tracing::debug!("Reader task cleaned up for session {} on rejection", session_id);
                            }
                            return format!("Registration rejected: {}", reason_str);
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

                                // Take the PTY reader and spawn a blocking task to read from it
                                match manager.take_reader(session_id) {
                                    Ok(reader) => {
                                        let tx = pty_output_tx.clone();
                                        let cancel_token = CancellationToken::new();
                                        let handle = spawn_pty_reader(session_id, reader, tx, cancel_token.clone());
                                        reader_tasks.insert(session_id, (handle, cancel_token));
                                        tracing::debug!("Spawned reader task for session {}", session_id);
                                    }
                                    Err(e) => {
                                        tracing::error!("Failed to take reader for session {}: {}", session_id, e);
                                    }
                                }
                            }
                            Err(e) => {
                                tracing::error!("Failed to create session: {}", e);
                                // Send error back to orchestrator
                                if let Err(send_err) = tunnel.send_error(
                                    session_id,
                                    kt_protocol::ErrorCode::PtyAllocationFailed,
                                    format!("Failed to create session: {}", e),
                                ).await {
                                    tracing::error!("Failed to send error to orchestrator: {}", send_err);
                                }
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

                        // Close the PTY first - this will cause the reader to exit
                        let exit_code = {
                            let mut manager = pty_manager.lock().await;
                            manager.close(session_id)
                        };

                        // Now wait for the reader task to finish (with timeout)
                        if let Some((handle, cancel_token)) = reader_tasks.remove(&session_id) {
                            // Signal the reader to stop gracefully
                            cancel_token.cancel();
                            // Give the reader task time to finish (increased timeout for graceful shutdown)
                            let _ = tokio::time::timeout(
                                std::time::Duration::from_millis(500),
                                handle
                            ).await;
                            tracing::debug!("Reader task cleaned up for session {}", session_id);
                        }

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
                        // Gracefully cancel all reader tasks and wait for cleanup
                        for (session_id, (handle, cancel_token)) in reader_tasks.drain() {
                            cancel_token.cancel();
                            let _ = tokio::time::timeout(
                                std::time::Duration::from_millis(500),
                                handle
                            ).await;
                            tracing::debug!("Reader task cleaned up for session {} on disconnect", session_id);
                        }
                        return "Disconnected by orchestrator".to_string();
                    }
                }

                // Check for exited sessions - collect all at once while holding lock
                let exited_sessions: Vec<(SessionId, i32)> = {
                    let mut manager = pty_manager.lock().await;
                    let mut exited = Vec::new();

                    for session_id in manager.list_sessions() {
                        if let Ok(Some(exit_code)) = manager.try_wait(session_id) {
                            tracing::info!("Session {} exited with code {}", session_id, exit_code);
                            manager.close(session_id);
                            exited.push((session_id, exit_code));
                        }
                    }
                    exited
                };

                // Clean up reader tasks and notify orchestrator (outside the lock)
                for (session_id, exit_code) in exited_sessions {
                    if let Some((handle, cancel_token)) = reader_tasks.remove(&session_id) {
                        cancel_token.cancel();
                        let _ = tokio::time::timeout(
                            std::time::Duration::from_millis(500),
                            handle
                        ).await;
                    }

                    if let Err(e) = tunnel.send_session_close(session_id, Some(exit_code)).await {
                        tracing::error!("Failed to send session close: {}", e);
                    }
                }
            }

            // Handle PTY output from reader tasks
            pty_output = pty_output_rx.recv() => {
                match pty_output {
                    Some(output) => {
                        // Send the PTY output to the orchestrator
                        if let Err(e) = tunnel.send_data(output.session_id, &output.data).await {
                            tracing::error!("Failed to send PTY data for session {}: {}", output.session_id, e);
                        }
                    }
                    None => {
                        // All senders dropped - this shouldn't happen during normal operation
                        tracing::warn!("PTY output channel closed unexpectedly");
                    }
                }
            }
        }
    }
}

/// Spawn a blocking task to read from a PTY and send output to the channel.
///
/// Uses a `CancellationToken` for graceful shutdown instead of task abort.
/// The token is checked between reads to allow clean termination.
fn spawn_pty_reader(
    session_id: SessionId,
    mut reader: Box<dyn Read + Send>,
    tx: mpsc::Sender<PtyOutput>,
    cancel_token: CancellationToken,
) -> JoinHandle<()> {
    tokio::task::spawn_blocking(move || {
        let mut buf = [0u8; 4096];

        loop {
            // Check for cancellation before each read
            if cancel_token.is_cancelled() {
                tracing::debug!("PTY reader cancelled for session {}", session_id);
                break;
            }

            match reader.read(&mut buf) {
                Ok(0) => {
                    // EOF - PTY closed
                    tracing::debug!("PTY reader EOF for session {}", session_id);
                    break;
                }
                Ok(n) => {
                    let data = buf[..n].to_vec();
                    // Try to send the data - if the channel is closed, the session was closed
                    if tx.blocking_send(PtyOutput { session_id, data }).is_err() {
                        tracing::debug!("PTY output channel closed for session {}", session_id);
                        break;
                    }
                }
                Err(e) => {
                    // Check if it's a "normal" error from PTY closing or due to cancellation
                    if cancel_token.is_cancelled() {
                        tracing::debug!("PTY reader stopped for session {} (cancelled)", session_id);
                    } else if e.kind() == std::io::ErrorKind::Other
                        || e.kind() == std::io::ErrorKind::BrokenPipe
                    {
                        tracing::debug!("PTY reader closed for session {}: {}", session_id, e);
                    } else {
                        tracing::error!("PTY read error for session {}: {}", session_id, e);
                    }
                    break;
                }
            }
        }

        tracing::debug!("PTY reader task exiting cleanly for session {}", session_id);
    })
}

/// Ensure an SSH key exists at the given path, generating one if needed
async fn ensure_ssh_key(path: &std::path::Path) -> Result<()> {
    if path.exists() {
        tracing::debug!("Using existing SSH key at {:?}", path);
        return Ok(());
    }

    tracing::info!("Generating new SSH key at {:?}", path);

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("Failed to create directory {:?}", parent))?;
    }

    // Generate Ed25519 key using ssh-keygen
    let status = tokio::process::Command::new("ssh-keygen")
        .args([
            "-t",
            "ed25519",
            "-f",
            &path.to_string_lossy(),
            "-N",
            "", // No passphrase
            "-C",
            "k-terminus-agent",
        ])
        .status()
        .await
        .context("Failed to run ssh-keygen")?;

    if !status.success() {
        anyhow::bail!("ssh-keygen failed");
    }

    tracing::info!("SSH key generated successfully");
    Ok(())
}

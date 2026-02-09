//! k-Terminus CLI
//!
//! Single binary for all k-Terminus operations:
//! - Orchestrator (server that accepts agent connections)
//! - Agent (client that runs on remote machines)
//! - Management commands (list, connect, exec, etc.)

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use k_terminus::commands;
use k_terminus::ipc::OrchestratorClient;
use k_terminus::output::{print_error, print_info, print_success, print_warning};
use kt_core::config::{self, AgentConfig, ConfigFile, OrchestratorConfig};
use kt_core::{auto_setup, is_initialized};

#[derive(Parser)]
#[command(name = "k-terminus")]
#[command(author, version, about = "Distributed terminal session manager")]
#[command(propagate_version = true)]
struct Cli {
    /// Path to configuration file
    #[arg(short, long, global = true)]
    config: Option<PathBuf>,

    /// Enable verbose output
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    verbose: u8,

    /// Suppress all output except errors
    #[arg(short, long, global = true)]
    quiet: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Start orchestrator (accepts connections from agents)
    /// Alias: start
    #[command(alias = "start")]
    Serve {
        /// Run in foreground (don't daemonize)
        #[arg(short, long)]
        foreground: bool,
        /// Bind address (overrides config)
        #[arg(short, long)]
        bind: Option<String>,
    },

    /// Stop orchestrator
    Stop,

    /// Connect to an orchestrator as an agent
    /// Alias: agent
    #[command(alias = "agent")]
    Join {
        /// Orchestrator to connect to (hostname, address, or pairing code)
        /// Examples: "my-laptop", "my-laptop.tailnet.ts.net:2222", "ABC123"
        /// If omitted, prompts for pairing code
        target: Option<String>,
        /// Machine alias (defaults to hostname)
        #[arg(short = 'a', long)]
        alias: Option<String>,
        /// Path to private key (auto-generated if not specified)
        #[arg(short, long)]
        key: Option<PathBuf>,
        /// Run in foreground with verbose output
        #[arg(short, long)]
        foreground: bool,
    },

    /// List connected machines and sessions
    List {
        /// Filter by machine name/alias
        #[arg(short, long)]
        machine: Option<String>,
        /// Filter by tag
        #[arg(short, long)]
        tag: Option<Vec<String>>,
        /// Show detailed information
        #[arg(short, long)]
        long: bool,
    },

    /// Create new session on machine and attach
    Connect {
        /// Machine identifier (name, alias, or ID)
        machine: String,
        /// Shell to spawn (overrides machine default)
        #[arg(short, long)]
        shell: Option<String>,
    },

    /// Attach to an existing session
    Attach {
        /// Session ID to attach to
        session: String,
    },

    /// Show orchestrator status and health
    Status {
        /// Show detailed health metrics
        #[arg(short, long)]
        detailed: bool,
    },

    /// Terminate a session
    Kill {
        /// Session identifier(s) to kill
        #[arg(required = true)]
        sessions: Vec<String>,
        /// Force kill without confirmation
        #[arg(short, long)]
        force: bool,
    },

    /// Manage configuration
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
}

#[derive(Subcommand)]
enum ConfigAction {
    /// Show current configuration
    Show,
    /// Get specific config value
    Get { key: String },
    /// Set config value
    Set { key: String, value: String },
    /// Edit config in editor
    Edit,
    /// Show config directory path
    Path,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Setup logging based on verbosity
    let log_level = match (cli.quiet, cli.verbose) {
        (true, _) => "error",
        (false, 0) => "warn",
        (false, 1) => "info",
        (false, 2) => "debug",
        (false, _) => "trace",
    };

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| log_level.into()),
        ))
        .with(tracing_subscriber::fmt::layer().with_target(false))
        .init();

    // Auto-setup on first run (for most commands)
    let needs_setup = matches!(
        &cli.command,
        None | Some(Commands::Serve { .. })
            | Some(Commands::List { .. })
            | Some(Commands::Connect { .. })
            | Some(Commands::Attach { .. })
            | Some(Commands::Status { .. })
    );

    if needs_setup && !is_initialized() {
        print_info("First run detected - initializing k-Terminus...");
        match auto_setup() {
            Ok(result) => {
                print_success(&format!("Initialized at {:?}", result.config_dir));
            }
            Err(e) => {
                print_error(&format!("Setup failed: {}", e));
                return Err(e);
            }
        }
    }

    // Handle no command - show quick status
    let command = match cli.command {
        Some(cmd) => cmd,
        None => {
            show_quick_status().await;
            return Ok(());
        }
    };

    // Create IPC client for management commands
    let mut client = OrchestratorClient::new();

    match command {
        Commands::Serve { foreground, bind } => {
            run_orchestrator(foreground, bind, cli.config.as_ref()).await?;
        }

        Commands::Stop => {
            print_info("Stopping orchestrator...");
            match client.shutdown().await {
                Ok(()) => {
                    print_success("Orchestrator stopped");
                }
                Err(e) => {
                    // Connection refused likely means it's not running
                    if e.to_string().contains("Connection refused")
                        || e.to_string().contains("Is it running")
                    {
                        print_warning("Orchestrator is not running");
                    } else {
                        print_error(&format!("Failed to stop orchestrator: {}", e));
                    }
                }
            }
        }

        Commands::Join {
            target,
            alias,
            key,
            foreground,
        } => {
            run_join(target.as_deref(), alias.as_deref(), key, foreground).await?;
        }

        Commands::List { machine, tag, long } => {
            ensure_orchestrator_running().await?;
            commands::list_command(&mut client, machine.as_deref(), tag.as_deref(), long).await?;
        }

        Commands::Connect { machine, shell } => {
            ensure_orchestrator_running().await?;
            commands::connect_command(client, &machine, shell.as_deref()).await?;
        }

        Commands::Attach { session } => {
            ensure_orchestrator_running().await?;
            commands::attach_command(client, &session).await?;
        }

        Commands::Status { detailed } => {
            ensure_orchestrator_running().await?;
            commands::status_command(&mut client, detailed).await?;
        }

        Commands::Kill { sessions, force } => {
            commands::kill_command(&mut client, &sessions, force).await?;
        }

        Commands::Config { action } => match action {
            ConfigAction::Show => {
                commands::config_show(cli.config.as_ref())?;
            }
            ConfigAction::Get { key } => {
                commands::config_get(cli.config.as_ref(), &key)?;
            }
            ConfigAction::Set { key, value } => {
                commands::config_set(cli.config.as_ref(), &key, &value)?;
            }
            ConfigAction::Edit => {
                commands::config_edit(cli.config.as_ref())?;
            }
            ConfigAction::Path => {
                let path = kt_core::config::default_config_dir();
                println!("{}", path.display());
            }
        },
    }

    Ok(())
}

// ============================================================================
// Orchestrator Implementation
// ============================================================================

async fn run_orchestrator(
    foreground: bool,
    bind_override: Option<String>,
    config_path: Option<&PathBuf>,
) -> Result<()> {
    use kt_orchestrator::ipc::IpcServer;
    use kt_orchestrator::server::{load_or_generate_host_key, ConnectionEvent, SshServer};
    use kt_orchestrator::OrchestratorState;

    if !foreground {
        // Daemonize by re-spawning ourselves
        let exe = std::env::current_exe()?;
        let mut cmd = std::process::Command::new(exe);
        cmd.arg("start").arg("--foreground");
        if let Some(bind) = &bind_override {
            cmd.arg("--bind").arg(bind);
        }
        if let Some(path) = config_path {
            cmd.arg("--config").arg(path);
        }

        let child = cmd
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;

        print_success(&format!("Orchestrator started (PID: {})", child.id()));
        return Ok(());
    }

    // Foreground mode - run the orchestrator directly
    tracing::info!("k-Terminus Orchestrator starting...");

    // Load configuration (wrapped in ConfigFile to handle [orchestrator] section)
    let config: OrchestratorConfig = if let Some(config_path) = config_path {
        let config_file: ConfigFile = config::load_config(config_path)
            .with_context(|| format!("Failed to load config from {:?}", config_path))?;
        config_file.orchestrator
    } else {
        let default_path = config::default_config_path();
        if default_path.exists() {
            let config_file: ConfigFile = config::load_config(&default_path).unwrap_or_else(|e| {
                tracing::warn!("Failed to load config from {:?}: {}", default_path, e);
                ConfigFile::default()
            });
            config_file.orchestrator
        } else {
            tracing::info!("Using default configuration");
            OrchestratorConfig::default()
        }
    };

    // Override bind address if specified
    let bind_addr = bind_override.unwrap_or_else(|| config.bind_address.clone());

    // Load or generate host key
    let host_key = load_or_generate_host_key(&config.host_key_path).await?;
    let public_key = host_key
        .clone_public_key()
        .context("Failed to extract public key from host key")?;
    let host_key_fingerprint = public_key.fingerprint();
    tracing::info!("Host key fingerprint: {}", host_key_fingerprint);

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
    let ipc_address = config.ipc_address();
    let ipc_server = Arc::new(
        IpcServer::new(ipc_address.clone(), Arc::clone(&state))?
            .with_shutdown_token(cancel.clone()),
    );
    let ipc_event_tx = ipc_server.event_sender();

    // Spawn event handler that updates state and broadcasts IPC events
    let state_clone = Arc::clone(&state);
    tokio::spawn(async move {
        while let Some(event) = event_rx.recv().await {
            handle_connection_event_with_ipc(&state_clone, event, &ipc_event_tx).await;
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

    // Print connection info with pairing code
    let pairing_code = state.pairing_code();
    if let Ok(Some(ts_info)) = kt_core::tailscale::get_tailscale_info() {
        if ts_info.logged_in {
            let port = bind_addr.split(':').next_back().unwrap_or("2222");
            println!();
            println!("  \x1b[1;32mk-Terminus Orchestrator\x1b[0m");
            println!();
            println!(
                "  Listening on: {}.{}:{}",
                ts_info.device_name, ts_info.tailnet, port
            );
            println!();
            println!("  \x1b[1;36mPairing Code: {}\x1b[0m", pairing_code);
            println!();
            println!("  To connect agents, run on remote machines:");
            println!("    k-terminus join {}      \x1b[90m# using pairing code\x1b[0m", pairing_code);
            println!("    k-terminus join {}  \x1b[90m# using hostname\x1b[0m", ts_info.device_name);
            println!();
        }
    } else {
        // No Tailscale, still show pairing code for local testing
        println!();
        println!("  \x1b[1;32mk-Terminus Orchestrator\x1b[0m");
        println!();
        println!("  Listening on: {}", bind_addr);
        println!();
        println!("  \x1b[1;36mPairing Code: {}\x1b[0m", pairing_code);
        println!();
    }

    tracing::info!("Starting SSH server on {}", bind_addr);
    server.run(&bind_addr).await?;

    tracing::info!("Orchestrator shutdown complete");
    Ok(())
}

async fn handle_connection_event_with_ipc(
    state: &kt_orchestrator::OrchestratorState,
    event: kt_orchestrator::server::ConnectionEvent,
    ipc_event_tx: &tokio::sync::broadcast::Sender<kt_core::ipc::IpcEventEnvelope>,
) {
    use kt_core::ipc::IpcEvent;
    use kt_orchestrator::connection::TunnelConnection;
    use kt_orchestrator::server::ConnectionEvent;

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

            // Register in connection pool
            state.coordinator.connections.insert(TunnelConnection::new(
                machine_id.clone(),
                Some(alias.clone()),
                Some(hostname.clone()),
                os.clone(),
                arch.clone(),
                command_tx,
                cancel,
            ));

            // Broadcast to IPC clients (wrapped in envelope)
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
            let _ = ipc_event_tx.send(state.epoch.wrap_event(event));
        }

        ConnectionEvent::MachineDisconnected { machine_id } => {
            tracing::info!("Machine disconnected: {}", machine_id);

            // Remove from connection pool
            state.coordinator.connections.remove(&machine_id);

            // Clean up all sessions for the disconnected machine
            let removed_sessions = state.coordinator.sessions.remove_by_machine(&machine_id);
            for session in &removed_sessions {
                tracing::info!(
                    "Cleaned up orphaned session {} on machine disconnect",
                    session.id
                );
                // Notify IPC clients that the session was closed
                let event = IpcEvent::SessionClosed {
                    session_id: session.id.to_string(),
                };
                let _ = ipc_event_tx.send(state.epoch.wrap_event(event));
            }
            if !removed_sessions.is_empty() {
                tracing::info!(
                    "Cleaned up {} orphaned sessions for disconnected machine {}",
                    removed_sessions.len(),
                    machine_id
                );
            }

            // Broadcast machine disconnected to IPC clients
            let event = IpcEvent::MachineDisconnected {
                machine_id: machine_id.to_string(),
            };
            let _ = ipc_event_tx.send(state.epoch.wrap_event(event));
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

            // Update session with PID
            state.coordinator.sessions.set_pid(session_id, pid);

            // Broadcast to IPC clients
            let event = IpcEvent::SessionCreated(kt_core::ipc::SessionInfo {
                id: session_id.to_string(),
                machine_id: machine_id.to_string(),
                shell: None,
                created_at: String::new(),
                pid: Some(pid),
                size: None,
            });
            let _ = ipc_event_tx.send(state.epoch.wrap_event(event));
        }

        ConnectionEvent::SessionClosed {
            machine_id,
            session_id,
        } => {
            tracing::info!("Session {} closed on {}", session_id, machine_id);

            // Remove session
            state.coordinator.sessions.remove(session_id);

            // Broadcast to IPC clients
            let event = IpcEvent::SessionClosed {
                session_id: session_id.to_string(),
            };
            let _ = ipc_event_tx.send(state.epoch.wrap_event(event));
        }

        ConnectionEvent::SessionData {
            machine_id: _,
            session_id,
            data,
        } => {
            // Broadcast terminal output to IPC clients (wrapped in envelope)
            let event = IpcEvent::TerminalOutput {
                session_id: session_id.to_string(),
                data,
            };
            let _ = ipc_event_tx.send(state.epoch.wrap_event(event));
        }
    }
}

// ============================================================================
// Join (Agent) Implementation
// ============================================================================

/// Check if a string looks like a pairing code (6 alphanumeric chars, no dots or colons)
/// Pairing code length (must match kt-orchestrator::state::PAIRING_CODE_LENGTH)
const PAIRING_CODE_LENGTH: usize = 8;

/// Pairing code charset: uppercase letters (no I, O) + digits (no 0, 1)
const PAIRING_CODE_CHARSET: &str = "ABCDEFGHJKLMNPQRSTUVWXYZ23456789";

fn looks_like_pairing_code(s: &str) -> bool {
    s.len() == PAIRING_CODE_LENGTH
        && !s.contains('.')
        && !s.contains(':')
        && s.chars().all(|c| PAIRING_CODE_CHARSET.contains(c))
}

/// Connect to an orchestrator as an agent
async fn run_join(
    target: Option<&str>,
    alias: Option<&str>,
    key_path: Option<PathBuf>,
    foreground: bool,
) -> Result<()> {
    use kt_agent::pty::PtyManager;
    use kt_agent::tunnel::{ExponentialBackoff, TunnelConnector};
    use kt_core::tailscale;

    // Check Tailscale
    let ts_info = tailscale::get_tailscale_info()
        .context("Failed to check Tailscale status")?
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Tailscale is not installed.\n\n{}",
                tailscale::get_install_instructions()
            )
        })?;

    if !ts_info.logged_in {
        anyhow::bail!("Tailscale is not logged in. Run: sudo tailscale up");
    }

    // Handle daemonization first (before interactive prompts)
    if !foreground {
        if target.is_none() {
            // Can't daemonize without a target (needs interactive input)
            anyhow::bail!(
                "Cannot run in background without specifying a target.\n\
                 Use --foreground to be prompted for a pairing code, or specify a target:\n\n\
                   k-terminus join ABC123           # pairing code\n\
                   k-terminus join my-laptop        # hostname"
            );
        }

        // Daemonize with the provided target
        let exe = std::env::current_exe()?;
        let mut cmd = std::process::Command::new(exe);
        cmd.arg("join").arg(target.unwrap()).arg("--foreground");
        if let Some(a) = alias {
            cmd.arg("--alias").arg(a);
        }
        if let Some(k) = &key_path {
            cmd.arg("--key").arg(k);
        }

        let child = cmd
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;

        print_success(&format!("Agent started (PID: {})", child.id()));
        return Ok(());
    }

    // Determine orchestrator address
    let address = match target {
        Some(t) if looks_like_pairing_code(t) => {
            // It's a pairing code - discover orchestrator
            print_info(&format!("Discovering orchestrator with pairing code {}...", t.to_uppercase()));
            let discovered = kt_agent::discover_orchestrator(t)
                .await
                .context("Failed to discover orchestrator")?;
            print_success(&format!("Found orchestrator: {}", discovered.peer.device_name));
            discovered.ssh_address
        }
        Some(t) => {
            // It's a hostname/address - resolve it
            let resolved = tailscale::resolve_device_name(t, &ts_info.tailnet);
            if resolved.contains(':') {
                resolved
            } else {
                format!("{}:2222", resolved)
            }
        }
        None => {
            // No target - prompt for pairing code (only in foreground mode)
            print_info("No orchestrator specified. Enter the pairing code shown on the orchestrator.");
            let code = kt_agent::prompt_pairing_code()
                .context("Failed to get pairing code")?;
            print_info(&format!("Discovering orchestrator with pairing code {}...", code));
            let discovered = kt_agent::discover_orchestrator(&code)
                .await
                .context("Failed to discover orchestrator")?;
            print_success(&format!("Found orchestrator: {}", discovered.peer.device_name));
            discovered.ssh_address
        }
    };

    print_info(&format!("Connecting to {} via Tailscale...", address));

    // Build config
    let config = AgentConfig {
        orchestrator_address: address,
        alias: alias.map(|a| a.to_string()),
        private_key_path: key_path.unwrap_or_else(|| AgentConfig::default().private_key_path),
        ..Default::default()
    };

    // Ensure SSH key exists
    ensure_ssh_key(&config.private_key_path).await?;

    // Create connector
    let connector =
        TunnelConnector::new(config.clone()).context("Failed to create tunnel connector")?;

    // Create PTY manager
    let pty_manager = Arc::new(Mutex::new(PtyManager::with_defaults(
        config.default_shell.clone(),
        config.default_env.clone(),
    )));

    print_success(&format!("Connected as '{}'", config.machine_alias()));

    // Main loop
    loop {
        let backoff = ExponentialBackoff::from_config(&config.backoff);

        let mut tunnel = match connector.connect_with_retry(backoff).await {
            Ok(tunnel) => tunnel,
            Err(e) => {
                tracing::error!("Connection failed: {}", e);
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                continue;
            }
        };

        tracing::info!("Connected to orchestrator");

        let reason = run_agent_event_loop(&mut tunnel, Arc::clone(&pty_manager)).await;
        tracing::warn!("Disconnected: {}", reason);

        // Cleanup
        {
            let mut manager = pty_manager.lock().await;
            for sid in manager.list_sessions() {
                manager.close(sid);
            }
        }

        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}

async fn run_agent_event_loop(
    tunnel: &mut kt_agent::tunnel::ActiveTunnel,
    pty_manager: Arc<Mutex<kt_agent::pty::PtyManager>>,
) -> String {
    use kt_agent::tunnel::TunnelEvent;

    loop {
        let event = match tunnel.recv_event().await {
            Some(e) => e,
            None => return "Channel closed".to_string(),
        };

        match event {
            TunnelEvent::Registered { accepted, reason } => {
                if !accepted {
                    return format!("Registration rejected: {:?}", reason);
                }
            }
            TunnelEvent::CreateSession {
                session_id,
                shell,
                env,
                size,
            } => {
                let mut manager = pty_manager.lock().await;
                if let Ok(pid) = manager.create_session(session_id, shell, env, size) {
                    let _ = tunnel.send_session_ready(session_id, pid).await;
                }
            }
            TunnelEvent::SessionData { session_id, data } => {
                let mut manager = pty_manager.lock().await;
                let _ = manager.write(session_id, &data);
            }
            TunnelEvent::SessionResize { session_id, size } => {
                let mut manager = pty_manager.lock().await;
                let _ = manager.resize(session_id, size);
            }
            TunnelEvent::SessionClose { session_id } => {
                let mut manager = pty_manager.lock().await;
                let exit_code = manager.close(session_id);
                let _ = tunnel.send_session_close(session_id, exit_code).await;
            }
            TunnelEvent::Heartbeat { timestamp } => {
                let _ = tunnel.send_heartbeat_ack(timestamp).await;
            }
            TunnelEvent::Disconnected => {
                return "Disconnected by orchestrator".to_string();
            }
        }
    }
}

/// Ensure SSH key exists, generate if needed
async fn ensure_ssh_key(path: &std::path::Path) -> Result<()> {
    if path.exists() {
        return Ok(());
    }

    print_info("Generating SSH key...");

    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }

    let status = tokio::process::Command::new("ssh-keygen")
        .args([
            "-t",
            "ed25519",
            "-f",
            &path.to_string_lossy(),
            "-N",
            "",
            "-C",
            "k-terminus",
        ])
        .status()
        .await
        .context("Failed to run ssh-keygen")?;

    if !status.success() {
        anyhow::bail!("ssh-keygen failed");
    }

    Ok(())
}

// ============================================================================
// Management Commands
// ============================================================================

async fn show_quick_status() {
    use kt_core::tailscale;

    println!();
    println!("  \x1b[1;34mk-Terminus\x1b[0m - Remote Terminal Access via Tailscale");
    println!();

    // Check Tailscale status
    match tailscale::get_tailscale_info() {
        Ok(Some(info)) if info.logged_in => {
            println!(
                "  Tailscale: \x1b[32m●\x1b[0m {} ({})",
                info.device_name, info.ip
            );
        }
        Ok(Some(_)) => {
            println!("  Tailscale: \x1b[33m●\x1b[0m Not logged in");
            println!("             Run: sudo tailscale up");
        }
        _ => {
            println!("  Tailscale: \x1b[31m●\x1b[0m Not installed");
            println!("             Visit: https://tailscale.com/download");
        }
    }

    let mut client = OrchestratorClient::new();

    match client.ping().await {
        Ok(true) => {
            println!("  Orchestrator: \x1b[32m●\x1b[0m Running");

            if let Ok(status) = client.status().await {
                println!("  Machines: {}", status.machine_count);
                println!("  Sessions: {}", status.session_count);
            }
        }
        _ => {
            println!("  Orchestrator: \x1b[31m●\x1b[0m Not running");
        }
    }

    println!();
    println!("  Commands:");
    println!("    k-terminus serve          Start orchestrator (accept connections)");
    println!("    k-terminus join <name>    Connect to orchestrator as agent");
    println!("    k-terminus list           List connected machines");
    println!("    k-terminus connect <m>    Open terminal to machine");
    println!();
}

async fn ensure_orchestrator_running() -> Result<()> {
    let mut client = OrchestratorClient::new();

    if client.ping().await.unwrap_or(false) {
        return Ok(());
    }

    print_info("Orchestrator not running, starting...");
    run_orchestrator(false, None, None).await?;

    // Wait for it to be ready
    for _ in 0..10 {
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        if client.ping().await.unwrap_or(false) {
            print_success("Orchestrator started");
            return Ok(());
        }
    }

    print_warning("Orchestrator may still be starting...");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_looks_like_pairing_code_valid() {
        // Valid 8-character codes using the charset
        assert!(looks_like_pairing_code("XX7WU27R"));
        assert!(looks_like_pairing_code("ABCD2345"));
        assert!(looks_like_pairing_code("ZZZZ9999"));
    }

    #[test]
    fn test_looks_like_pairing_code_invalid_length() {
        // Too short (old 6-char format)
        assert!(!looks_like_pairing_code("ABC123"));
        // Too long
        assert!(!looks_like_pairing_code("ABCD12345"));
        // Empty
        assert!(!looks_like_pairing_code(""));
    }

    #[test]
    fn test_looks_like_pairing_code_invalid_chars() {
        // Contains excluded chars (I, O, 0, 1)
        assert!(!looks_like_pairing_code("ABCD1234")); // has 1
        assert!(!looks_like_pairing_code("ABCD0234")); // has 0
        assert!(!looks_like_pairing_code("ABCDI234")); // has I
        assert!(!looks_like_pairing_code("ABCDO234")); // has O
        // Lowercase
        assert!(!looks_like_pairing_code("abcd2345"));
    }

    #[test]
    fn test_looks_like_pairing_code_not_hostname() {
        // Hostnames contain dots
        assert!(!looks_like_pairing_code("my-laptop.tailnet.ts.net"));
        assert!(!looks_like_pairing_code("host.com"));
        // Addresses contain colons
        assert!(!looks_like_pairing_code("host:2222"));
    }
}

//! k-Terminus CLI

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use kt_cli::commands;
use kt_cli::ipc::OrchestratorClient;
use kt_cli::output::{print_error, print_info, print_success, print_warning};
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
    /// Start orchestrator daemon (runs automatically on first use)
    Start {
        /// Run in foreground (don't daemonize)
        #[arg(short, long)]
        foreground: bool,
    },

    /// Stop orchestrator daemon
    Stop,

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

    /// Attach to existing session
    Attach {
        /// Session identifier (machine:session-id format)
        session: String,
    },

    /// Execute one-off command on machine
    Exec {
        /// Machine identifier
        machine: String,
        /// Command to execute
        #[arg(trailing_var_arg = true, required = true)]
        command: Vec<String>,
        /// Timeout in seconds
        #[arg(short, long, default_value = "60")]
        timeout: u64,
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

    /// Display orchestrator logs
    Logs {
        /// Follow log output
        #[arg(short, long)]
        follow: bool,
        /// Number of lines to show
        #[arg(short, long, default_value = "100")]
        lines: usize,
    },

    /// Show command to add a new machine
    #[command(name = "add-machine")]
    AddMachine {
        /// Your public IP or hostname (what remote machines will connect to)
        #[arg(long)]
        address: Option<String>,
    },

    /// Initialize or re-initialize k-Terminus
    Setup {
        /// Force re-initialization
        #[arg(long)]
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
        None | Some(Commands::Start { .. })
            | Some(Commands::List { .. })
            | Some(Commands::Connect { .. })
            | Some(Commands::Status { .. })
            | Some(Commands::AddMachine { .. })
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

    // Create IPC client
    let mut client = OrchestratorClient::new();

    match command {
        Commands::Start { foreground } => {
            start_orchestrator(foreground, cli.config.as_ref()).await?;
        }

        Commands::Stop => {
            print_info("Stopping orchestrator...");
            print_warning("Stop command not yet implemented");
        }

        Commands::List { machine, tag, long } => {
            // Ensure orchestrator is running
            ensure_orchestrator_running().await?;

            commands::list_command(
                &mut client,
                machine.as_deref(),
                tag.as_deref(),
                long,
            )
            .await?;
        }

        Commands::Connect { machine, shell } => {
            ensure_orchestrator_running().await?;
            commands::connect_command(&mut client, &machine, shell.as_deref()).await?;
        }

        Commands::Attach { session } => {
            print_info(&format!("Attaching to session: {}", session));
            print_warning("Attach not fully implemented - use 'connect' to create new session");
        }

        Commands::Exec {
            machine,
            command,
            timeout: _timeout,
        } => {
            let cmd = command.join(" ");
            print_info(&format!("Executing on {}: {}", machine, cmd));
            print_warning("Exec command not yet implemented");
        }

        Commands::Status { detailed } => {
            commands::status_command(&mut client, detailed).await?;
        }

        Commands::Kill { sessions, force } => {
            commands::kill_command(&mut client, &sessions, force).await?;
        }

        Commands::Logs { follow, lines } => {
            print_info(&format!(
                "Showing logs (follow: {}, lines: {})...",
                follow, lines
            ));
            print_warning("Logs command not yet implemented");
        }

        Commands::AddMachine { address } => {
            show_add_machine_instructions(address.as_deref())?;
        }

        Commands::Setup { force } => {
            run_setup(force)?;
        }

        Commands::Config { action } => match action {
            ConfigAction::Show => {
                commands::config_show(cli.config.as_ref())?;
            }
            ConfigAction::Get { key } => {
                print_info(&format!("Getting config key: {}", key));
                print_warning("Config get not yet implemented");
            }
            ConfigAction::Set { key, value } => {
                print_info(&format!("Setting {} = {}", key, value));
                print_warning("Config set not yet implemented");
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

/// Show quick status when run without arguments
async fn show_quick_status() {
    println!();
    println!("  \x1b[1;34mk-Terminus\x1b[0m - Distributed Terminal Session Manager");
    println!();

    let mut client = OrchestratorClient::new();

    match client.ping().await {
        Ok(true) => {
            println!("  Status: \x1b[32m●\x1b[0m Orchestrator running");

            if let Ok(status) = client.status().await {
                println!("  Machines: {}", status.machine_count);
                println!("  Sessions: {}", status.session_count);
            }
        }
        _ => {
            println!("  Status: \x1b[31m●\x1b[0m Orchestrator not running");
            println!();
            println!("  Run \x1b[1mk-terminus start\x1b[0m to start the orchestrator");
        }
    }

    println!();
    println!("  Quick commands:");
    println!("    k-terminus start        Start orchestrator");
    println!("    k-terminus list         List machines");
    println!("    k-terminus connect <m>  Connect to machine");
    println!("    k-terminus add-machine  Show how to add machines");
    println!();
}

/// Ensure orchestrator is running, start if not
async fn ensure_orchestrator_running() -> Result<()> {
    let mut client = OrchestratorClient::new();

    if client.ping().await.unwrap_or(false) {
        return Ok(());
    }

    print_info("Orchestrator not running, starting...");
    start_orchestrator(false, None).await?;

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

/// Start the orchestrator daemon
async fn start_orchestrator(foreground: bool, config_path: Option<&PathBuf>) -> Result<()> {
    use std::process::Command;

    if foreground {
        print_info("Starting orchestrator in foreground...");

        let mut cmd = Command::new("kt-orchestrator");
        if let Some(path) = config_path {
            cmd.arg("--config").arg(path);
        }
        cmd.arg("--foreground");

        let status = cmd.status()?;
        if !status.success() {
            print_error("Orchestrator exited with error");
            std::process::exit(status.code().unwrap_or(1));
        }
    } else {
        // Daemonize
        let mut cmd = Command::new("kt-orchestrator");
        if let Some(path) = config_path {
            cmd.arg("--config").arg(path);
        }

        let child = cmd
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()?;

        print_success(&format!("Orchestrator started (PID: {})", child.id()));
    }

    Ok(())
}

/// Show instructions for adding a machine
fn show_add_machine_instructions(address: Option<&str>) -> Result<()> {
    let config_dir = kt_core::config::default_config_dir();
    let agent_key_path = config_dir.join("agent_key");

    // Detect address if not provided
    let orchestrator_addr = if let Some(addr) = address {
        addr.to_string()
    } else {
        // Try to detect public IP
        print_info("Detecting your IP address...");
        detect_public_ip().unwrap_or_else(|| {
            print_warning("Could not detect IP. Use --address to specify.");
            "YOUR_IP:2222".to_string()
        })
    };

    println!();
    println!("  \x1b[1;34mAdd a Machine to k-Terminus\x1b[0m");
    println!();
    println!("  On the remote machine, run these commands:");
    println!();
    println!("  \x1b[1;33m# 1. Install kt-agent (if not installed)\x1b[0m");
    println!("  curl -sSL https://k-terminus.dev/install.sh | sh");
    println!();
    println!("  \x1b[1;33m# 2. Copy the authentication key\x1b[0m");
    println!("  mkdir -p ~/.config/k-terminus");

    // Show the key copy command
    if agent_key_path.exists() {
        println!("  # Copy this key to ~/.config/k-terminus/agent_key on the remote machine:");
        println!("  cat << 'EOF' > ~/.config/k-terminus/agent_key");
        if let Ok(key) = std::fs::read_to_string(&agent_key_path) {
            for line in key.lines() {
                println!("  {}", line);
            }
        }
        println!("  EOF");
        println!("  chmod 600 ~/.config/k-terminus/agent_key");
    } else {
        println!("  scp {}:{} ~/.config/k-terminus/agent_key",
            whoami::fallible::hostname().unwrap_or_else(|_| "localhost".to_string()),
            agent_key_path.display()
        );
    }

    println!();
    println!("  \x1b[1;33m# 3. Start the agent\x1b[0m");
    println!("  kt-agent --orchestrator {} --alias my-server", orchestrator_addr);
    println!();
    println!("  \x1b[2mTip: Add --foreground to see connection logs\x1b[0m");
    println!();

    Ok(())
}

/// Run setup command
fn run_setup(force: bool) -> Result<()> {
    if is_initialized() && !force {
        print_info("k-Terminus is already initialized");
        print_info(&format!("Config directory: {:?}", kt_core::config::default_config_dir()));
        print_info("Use --force to re-initialize");
        return Ok(());
    }

    if force {
        // Remove initialized marker to force re-setup
        let marker = kt_core::config::default_config_dir().join("initialized");
        let _ = std::fs::remove_file(marker);
    }

    match auto_setup() {
        Ok(result) => {
            print_success("k-Terminus initialized successfully!");
            println!();
            println!("  Config directory: {:?}", result.config_dir);
            println!("  Host key: {:?}", result.host_key_path);
            println!("  Agent key: {:?}", result.agent_key_path);
            println!();
            print_info("Run 'k-terminus start' to start the orchestrator");
        }
        Err(e) => {
            print_error(&format!("Setup failed: {}", e));
            return Err(e);
        }
    }

    Ok(())
}

/// Try to detect public IP
fn detect_public_ip() -> Option<String> {
    // Try to get local IP
    let output = std::process::Command::new("hostname")
        .arg("-I")
        .output()
        .ok()?;

    if output.status.success() {
        let ips = String::from_utf8_lossy(&output.stdout);
        if let Some(ip) = ips.split_whitespace().next() {
            return Some(format!("{}:2222", ip));
        }
    }

    // Fallback for macOS
    let output = std::process::Command::new("ipconfig")
        .arg("getifaddr")
        .arg("en0")
        .output()
        .ok()?;

    if output.status.success() {
        let ip = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if !ip.is_empty() {
            return Some(format!("{}:2222", ip));
        }
    }

    None
}

//! k-Terminus CLI

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use kt_cli::commands;
use kt_cli::ipc::OrchestratorClient;
use kt_cli::output::{print_error, print_info, print_warning};

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
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start orchestrator daemon
    Start {
        /// Run in foreground (don't daemonize)
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
    /// Validate configuration
    Validate,
    /// Initialize default configuration
    Init {
        /// Overwrite existing config
        #[arg(long)]
        force: bool,
    },
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

    // Create IPC client
    let mut client = OrchestratorClient::new();

    match cli.command {
        Commands::Start { foreground } => {
            start_orchestrator(foreground, cli.config.as_ref()).await?;
        }

        Commands::List { machine, tag, long } => {
            commands::list_command(
                &mut client,
                machine.as_deref(),
                tag.as_deref(),
                long,
            )
            .await?;
        }

        Commands::Connect { machine, shell } => {
            commands::connect_command(&mut client, &machine, shell.as_deref()).await?;
        }

        Commands::Attach { session } => {
            print_info(&format!("Attaching to session: {}", session));
            // In full implementation, would call attach_to_session directly
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
            // In full implementation:
            // 1. Create session with one-shot flag
            // 2. Send command
            // 3. Wait for output/exit
            // 4. Close session
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
            // Would read from orchestrator log file or stream via IPC
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
            ConfigAction::Validate => {
                print_info("Validating configuration...");
                print_warning("Config validate not yet implemented");
            }
            ConfigAction::Init { force } => {
                commands::config_init(cli.config.as_ref(), force)?;
            }
        },
    }

    Ok(())
}

/// Start the orchestrator daemon
async fn start_orchestrator(foreground: bool, config_path: Option<&PathBuf>) -> Result<()> {
    use std::process::Command;

    print_info("Starting orchestrator...");

    if foreground {
        // Run the orchestrator binary in foreground
        // In a full implementation, this would exec() into kt-orchestrator
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
        #[cfg(unix)]
        {
            let mut cmd = Command::new("kt-orchestrator");

            if let Some(path) = config_path {
                cmd.arg("--config").arg(path);
            }

            // Spawn as daemon
            let child = cmd
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()?;

            print_info(&format!("Orchestrator started (PID: {})", child.id()));
            print_info("Use 'k-terminus status' to check status");
        }

        #[cfg(not(unix))]
        {
            print_warning("Daemonization not supported on this platform");
            print_info("Use --foreground to run in foreground mode");
        }
    }

    Ok(())
}

//! Config command implementations

use std::path::PathBuf;

use anyhow::{Context, Result};

use kt_core::config;
use crate::output::{print_error, print_info, print_success, print_warning};

/// Show current configuration
pub fn config_show(config_path: Option<&PathBuf>) -> Result<()> {
    let path = config_path
        .cloned()
        .unwrap_or_else(|| config::default_config_dir().join("config.toml"));

    if !path.exists() {
        print_warning(&format!("No configuration file found at {:?}", path));
        print_info("Run 'k-terminus config init' to create one");
        return Ok(());
    }

    print_info(&format!("Configuration file: {:?}", path));
    println!();

    // Read and display the config file
    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read config file: {:?}", path))?;

    println!("{}", content);

    Ok(())
}

/// Initialize default configuration
pub fn config_init(config_path: Option<&PathBuf>, force: bool) -> Result<()> {
    let config_dir = config_path
        .and_then(|p| p.parent().map(PathBuf::from))
        .unwrap_or_else(config::default_config_dir);

    let config_file = config_path
        .cloned()
        .unwrap_or_else(|| config_dir.join("config.toml"));

    // Create config directory if needed
    if !config_dir.exists() {
        std::fs::create_dir_all(&config_dir)
            .with_context(|| format!("Failed to create config directory: {:?}", config_dir))?;
        print_success(&format!("Created config directory: {:?}", config_dir));
    }

    // Check if config already exists
    if config_file.exists() && !force {
        print_error(&format!("Config file already exists: {:?}", config_file));
        print_info("Use --force to overwrite");
        return Ok(());
    }

    // Generate default configuration
    let default_config = generate_default_config();

    // Write configuration
    std::fs::write(&config_file, default_config)
        .with_context(|| format!("Failed to write config file: {:?}", config_file))?;

    print_success(&format!("Created configuration file: {:?}", config_file));

    // Generate SSH keys if they don't exist
    let key_path = config_dir.join("id_ed25519");
    if !key_path.exists() {
        print_info("Consider generating SSH keys for agent authentication:");
        print_info(&format!("  ssh-keygen -t ed25519 -f {:?} -N ''", key_path));
    }

    Ok(())
}

/// Open config in editor
pub fn config_edit(config_path: Option<&PathBuf>) -> Result<()> {
    let path = config_path
        .cloned()
        .unwrap_or_else(|| config::default_config_dir().join("config.toml"));

    if !path.exists() {
        print_error(&format!("Config file not found: {:?}", path));
        print_info("Run 'k-terminus config init' to create one");
        return Ok(());
    }

    // Find editor
    let editor = std::env::var("EDITOR")
        .or_else(|_| std::env::var("VISUAL"))
        .unwrap_or_else(|_| {
            if cfg!(windows) {
                "notepad".to_string()
            } else {
                "vi".to_string()
            }
        });

    print_info(&format!("Opening config with: {}", editor));

    // Open editor
    std::process::Command::new(&editor)
        .arg(&path)
        .status()
        .with_context(|| format!("Failed to open editor: {}", editor))?;

    Ok(())
}

/// Generate default configuration content
fn generate_default_config() -> String {
    r#"# k-Terminus Configuration
# See https://github.com/your-org/k-terminus for documentation

[orchestrator]
# Address to bind SSH server
bind_address = "0.0.0.0:2222"

# Paths to authorized public keys for agent authentication
auth_keys = ["~/.config/k-terminus/authorized_keys"]

# Host key file path (will be generated if missing)
host_key_path = "~/.config/k-terminus/host_key"

# Heartbeat interval in seconds
heartbeat_interval = 30

# Connection timeout in seconds
connect_timeout = 10

[orchestrator.backoff]
# Initial retry delay in seconds
initial_secs = 1
# Maximum retry delay in seconds
max_secs = 60
# Backoff multiplier
multiplier = 2.0

# Example machine profiles
# [[machines]]
# alias = "dev-server"
# host_key = "ssh-ed25519 AAAAC3..."
# tags = ["development"]
# default_shell = "/bin/bash"
# [machines.env]
# CUSTOM_VAR = "value"
"#
    .to_string()
}

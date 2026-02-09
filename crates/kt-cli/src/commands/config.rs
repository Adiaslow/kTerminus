//! Config command implementations

use std::path::PathBuf;

use anyhow::{Context, Result};

use crate::output::{print_error, print_info, print_success, print_warning};
use kt_core::config;

/// Get a config value by key
pub fn config_get(config_path: Option<&PathBuf>, key: &str) -> Result<()> {
    let path = config_path
        .cloned()
        .unwrap_or_else(|| config::default_config_dir().join("config.toml"));

    if !path.exists() {
        print_error(&format!("Config file not found: {:?}", path));
        print_info("Run 'k-terminus config init' to create one");
        return Ok(());
    }

    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read config file: {:?}", path))?;

    let table: toml::Table =
        toml::from_str(&content).with_context(|| "Failed to parse config file")?;

    // Navigate through the key path (e.g., "orchestrator.bind_address")
    let parts: Vec<&str> = key.split('.').collect();
    let mut current: &toml::Value = &toml::Value::Table(table);

    for part in &parts {
        match current {
            toml::Value::Table(t) => {
                if let Some(v) = t.get(*part) {
                    current = v;
                } else {
                    print_error(&format!("Key not found: {}", key));
                    return Ok(());
                }
            }
            _ => {
                print_error(&format!("Key not found: {}", key));
                return Ok(());
            }
        }
    }

    // Print the value
    match current {
        toml::Value::String(s) => println!("{}", s),
        toml::Value::Integer(i) => println!("{}", i),
        toml::Value::Float(f) => println!("{}", f),
        toml::Value::Boolean(b) => println!("{}", b),
        toml::Value::Array(a) => {
            for item in a {
                println!("{}", item);
            }
        }
        toml::Value::Table(_) => {
            // Print sub-table as TOML
            println!("{}", toml::to_string_pretty(current)?);
        }
        toml::Value::Datetime(d) => println!("{}", d),
    }

    Ok(())
}

/// Set a config value by key
pub fn config_set(config_path: Option<&PathBuf>, key: &str, value: &str) -> Result<()> {
    let path = config_path
        .cloned()
        .unwrap_or_else(|| config::default_config_dir().join("config.toml"));

    // Create default config if it doesn't exist
    if !path.exists() {
        print_info("Creating default configuration...");
        config_init(config_path, false)?;
    }

    let content = std::fs::read_to_string(&path)
        .with_context(|| format!("Failed to read config file: {:?}", path))?;

    let mut table: toml::Table =
        toml::from_str(&content).with_context(|| "Failed to parse config file")?;

    // Navigate through the key path and set the value
    let parts: Vec<&str> = key.split('.').collect();

    if parts.is_empty() {
        anyhow::bail!("Invalid key");
    }

    // Navigate/create path to the parent
    let mut current = &mut table;
    for part in &parts[..parts.len() - 1] {
        if !current.contains_key(*part) {
            current.insert(part.to_string(), toml::Value::Table(toml::Table::new()));
        }
        current = current
            .get_mut(*part)
            .and_then(|v| v.as_table_mut())
            .ok_or_else(|| anyhow::anyhow!("Cannot navigate to key: {}", key))?;
    }

    // Set the value (try to parse as appropriate type)
    // Safety: We already checked parts.is_empty() above and returned early if true,
    // so parts.last() is guaranteed to return Some
    let last_key = parts
        .last()
        .ok_or_else(|| anyhow::anyhow!("Invalid key: key path cannot be empty"))?;
    let toml_value = if value == "true" {
        toml::Value::Boolean(true)
    } else if value == "false" {
        toml::Value::Boolean(false)
    } else if let Ok(i) = value.parse::<i64>() {
        toml::Value::Integer(i)
    } else if let Ok(f) = value.parse::<f64>() {
        toml::Value::Float(f)
    } else {
        toml::Value::String(value.to_string())
    };

    current.insert(last_key.to_string(), toml_value);

    // Write back
    let new_content = toml::to_string_pretty(&table)?;
    std::fs::write(&path, new_content)
        .with_context(|| format!("Failed to write config file: {:?}", path))?;

    print_success(&format!("Set {} = {}", key, value));
    Ok(())
}

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

# Host key file path (will be generated if missing)
host_key_path = "~/.config/k-terminus/host_key"

# Heartbeat interval in seconds
heartbeat_interval = 30

# Heartbeat timeout in seconds
heartbeat_timeout = 90

[orchestrator.backoff]
# Initial retry delay in seconds
initial = 1
# Maximum retry delay in seconds
max = 60
# Backoff multiplier
multiplier = 2.0
# Jitter factor
jitter = 0.25

# Example machine profiles
# [[machines]]
# alias = "dev-server"
# tags = ["development"]
# default_shell = "/bin/bash"
# [machines.env]
# CUSTOM_VAR = "value"
"#
    .to_string()
}

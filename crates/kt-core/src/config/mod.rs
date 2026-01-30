//! Configuration management for k-Terminus

mod orchestrator;
mod agent;
mod machine;

pub use orchestrator::{OrchestratorConfig, BackoffConfig};
pub use agent::AgentConfig;
pub use machine::MachineProfile;

use crate::error::ConfigError;
use std::path::{Path, PathBuf};

/// Get the default configuration directory
pub fn default_config_dir() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("k-terminus")
}

/// Get the default configuration file path
pub fn default_config_path() -> PathBuf {
    default_config_dir().join("config.toml")
}

/// Load configuration from a file
pub fn load_config<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T, ConfigError> {
    if !path.exists() {
        return Err(ConfigError::NotFound(path.to_path_buf()));
    }

    let content = std::fs::read_to_string(path)
        .map_err(|e| ConfigError::Invalid(format!("Failed to read config: {}", e)))?;

    let config: T = toml::from_str(&content)?;
    Ok(config)
}

/// Save configuration to a file
pub fn save_config<T: serde::Serialize>(path: &Path, config: &T) -> Result<(), ConfigError> {
    let content = toml::to_string_pretty(config)?;

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| ConfigError::Invalid(format!("Failed to create config dir: {}", e)))?;
    }

    std::fs::write(path, content)
        .map_err(|e| ConfigError::Invalid(format!("Failed to write config: {}", e)))?;

    Ok(())
}

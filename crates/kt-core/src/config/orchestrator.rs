//! Orchestrator configuration

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Duration;

use super::MachineProfile;

/// Configuration for the orchestrator daemon
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct OrchestratorConfig {
    /// Address to bind the SSH server to
    pub bind_address: String,

    /// Heartbeat interval in seconds
    #[serde(with = "humantime_serde")]
    pub heartbeat_interval: Duration,

    /// Heartbeat timeout (how long to wait before considering connection dead)
    #[serde(with = "humantime_serde")]
    pub heartbeat_timeout: Duration,

    /// Path to the host key file
    pub host_key_path: PathBuf,

    /// Backoff configuration for reconnections
    pub backoff: BackoffConfig,

    /// Machine profiles
    #[serde(default)]
    pub machines: HashMap<String, MachineProfile>,

    /// IPC port for CLI/desktop communication (localhost only)
    pub ipc_port: u16,

    /// Maximum number of concurrent connections
    pub max_connections: Option<u32>,

    /// Maximum sessions per machine
    pub max_sessions_per_machine: Option<u32>,

    /// Tailscale hostname (auto-detected during setup)
    pub tailscale_hostname: Option<String>,
}

impl Default for OrchestratorConfig {
    fn default() -> Self {
        let config_dir = super::default_config_dir();

        Self {
            bind_address: "0.0.0.0:2222".to_string(),
            heartbeat_interval: Duration::from_secs(30),
            heartbeat_timeout: Duration::from_secs(90),
            host_key_path: config_dir.join("host_key"),
            backoff: BackoffConfig::default(),
            machines: HashMap::new(),
            ipc_port: 22230,
            max_connections: None,
            max_sessions_per_machine: None,
            tailscale_hostname: None,
        }
    }
}

impl OrchestratorConfig {
    /// Get the IPC address (localhost:port)
    pub fn ipc_address(&self) -> String {
        format!("127.0.0.1:{}", self.ipc_port)
    }
}

/// Exponential backoff configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackoffConfig {
    /// Initial delay
    #[serde(with = "humantime_serde")]
    pub initial: Duration,

    /// Maximum delay
    #[serde(with = "humantime_serde")]
    pub max: Duration,

    /// Multiplier for each retry
    pub multiplier: f64,

    /// Jitter factor (0.0 to 1.0)
    pub jitter: f64,
}

impl Default for BackoffConfig {
    fn default() -> Self {
        Self {
            initial: Duration::from_secs(1),
            max: Duration::from_secs(60),
            multiplier: 2.0,
            jitter: 0.25,
        }
    }
}

// Helper module for Duration serialization with humantime
mod humantime_serde {
    use serde::{self, Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration.as_secs())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(Duration::from_secs(secs))
    }
}

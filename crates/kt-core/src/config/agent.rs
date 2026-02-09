//! Agent configuration

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

use super::orchestrator::BackoffConfig;
use super::serde_utils::duration_secs;

/// Configuration for the client agent
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AgentConfig {
    /// Orchestrator address to connect to.
    ///
    /// **Important**: Use a Tailscale hostname (e.g., `my-laptop.tailnet.ts.net:2222`),
    /// NOT an IP address. Tailscale hostnames remain stable regardless of network
    /// changes, while IP addresses may change when networks change.
    pub orchestrator_address: String,

    /// Path to the private key for authentication
    pub private_key_path: PathBuf,

    /// Expected orchestrator host key (for verification)
    pub orchestrator_host_key: Option<String>,

    /// Username for SSH authentication
    pub username: String,

    /// Machine alias (optional, defaults to hostname)
    pub alias: Option<String>,

    /// Tags for this machine
    pub tags: Vec<String>,

    /// Default shell to spawn
    pub default_shell: Option<String>,

    /// Default environment variables for sessions
    pub default_env: Vec<(String, String)>,

    /// Backoff configuration for reconnections
    pub backoff: BackoffConfig,

    /// Connection timeout
    #[serde(with = "duration_secs")]
    pub connect_timeout: Duration,

    /// Maximum number of concurrent sessions
    pub max_sessions: Option<u32>,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            orchestrator_address: "localhost:2222".to_string(),
            private_key_path: dirs::home_dir()
                .unwrap_or_default()
                .join(".config")
                .join("k-terminus")
                .join("agent_key"),
            orchestrator_host_key: None,
            username: whoami::username(),
            alias: None,
            tags: vec![],
            default_shell: None,
            default_env: vec![("TERM".to_string(), "xterm-256color".to_string())],
            backoff: BackoffConfig::default(),
            connect_timeout: Duration::from_secs(30),
            max_sessions: None,
        }
    }
}

impl AgentConfig {
    /// Get the machine alias, falling back to hostname
    pub fn machine_alias(&self) -> String {
        self.alias
            .clone()
            .unwrap_or_else(|| gethostname::gethostname().to_string_lossy().into_owned())
    }
}

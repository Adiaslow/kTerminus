//! Machine profile configuration

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Profile for a known machine
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineProfile {
    /// Human-readable alias for the machine
    pub alias: String,

    /// SSH host key fingerprint for verification
    #[serde(default)]
    pub host_key: Option<String>,

    /// Tags for categorization
    #[serde(default)]
    pub tags: Vec<String>,

    /// Default shell to spawn on this machine
    #[serde(default)]
    pub default_shell: Option<String>,

    /// Default environment variables for sessions
    #[serde(default)]
    pub env: HashMap<String, String>,

    /// Whether to automatically connect when the machine connects
    #[serde(default)]
    pub auto_connect: bool,

    /// Notes/description for this machine
    #[serde(default)]
    pub notes: Option<String>,
}

impl Default for MachineProfile {
    fn default() -> Self {
        Self {
            alias: String::new(),
            host_key: None,
            tags: vec![],
            default_shell: None,
            env: HashMap::new(),
            auto_connect: false,
            notes: None,
        }
    }
}

impl MachineProfile {
    /// Create a new machine profile with just an alias
    pub fn new(alias: impl Into<String>) -> Self {
        Self {
            alias: alias.into(),
            ..Default::default()
        }
    }

    /// Check if the machine has a specific tag
    pub fn has_tag(&self, tag: &str) -> bool {
        self.tags.iter().any(|t| t.eq_ignore_ascii_case(tag))
    }
}

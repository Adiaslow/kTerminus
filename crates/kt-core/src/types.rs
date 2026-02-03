//! Core domain types

use serde::{Deserialize, Serialize};
use std::fmt;

/// Unique identifier for a machine
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct MachineId(pub String);

impl MachineId {
    /// Create a new machine ID
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    /// Create a machine ID from a public key fingerprint
    pub fn from_fingerprint(fingerprint: &str) -> Self {
        // Use first 16 chars of fingerprint as ID
        let id = fingerprint
            .chars()
            .filter(|c| c.is_alphanumeric())
            .take(16)
            .collect::<String>()
            .to_lowercase();
        Self(id)
    }

    /// Get the raw ID string
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for MachineId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for MachineId {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for MachineId {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Machine capability flags
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Capability {
    /// Whether the machine supports PTY allocation
    pub pty: bool,
    /// Whether the machine supports file transfer
    pub file_transfer: bool,
    /// Whether the machine supports port forwarding
    pub port_forward: bool,
    /// Maximum number of concurrent sessions
    pub max_sessions: Option<u32>,
}

impl Capability {
    /// Create default capabilities (PTY only)
    pub fn default_capabilities() -> Self {
        Self {
            pty: true,
            file_transfer: false,
            port_forward: false,
            max_sessions: None,
        }
    }
}

/// Connection status for a machine
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConnectionStatus {
    /// Machine is connected and ready
    Connected,
    /// Machine is connecting
    Connecting,
    /// Machine is disconnected
    Disconnected,
    /// Machine is reconnecting after a disconnect
    Reconnecting,
}

impl fmt::Display for ConnectionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ConnectionStatus::Connected => write!(f, "connected"),
            ConnectionStatus::Connecting => write!(f, "connecting"),
            ConnectionStatus::Disconnected => write!(f, "disconnected"),
            ConnectionStatus::Reconnecting => write!(f, "reconnecting"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_machine_id_from_fingerprint() {
        let fingerprint = "SHA256:AbCdEfGhIjKlMnOpQrStUvWxYz123456";
        let id = MachineId::from_fingerprint(fingerprint);
        assert_eq!(id.as_str().len(), 16);
        assert!(id.as_str().chars().all(|c| c.is_alphanumeric()));
    }

    #[test]
    fn test_connection_status_display() {
        assert_eq!(format!("{}", ConnectionStatus::Connected), "connected");
        assert_eq!(
            format!("{}", ConnectionStatus::Disconnected),
            "disconnected"
        );
    }
}

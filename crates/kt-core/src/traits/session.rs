//! Session traits

use async_trait::async_trait;
use bytes::Bytes;
use std::sync::Arc;

use crate::error::SessionError;
use crate::types::MachineId;
use kt_protocol::{SessionId, TerminalSize};

/// Session lifecycle state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Session is being created
    Creating,
    /// Session is active and ready for I/O
    Active,
    /// Session is closing
    Closing,
    /// Session is closed
    Closed,
}

/// Configuration for creating a new session
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Shell to spawn (None = default)
    pub shell: Option<String>,
    /// Environment variables
    pub env: Vec<(String, String)>,
    /// Initial terminal size
    pub size: TerminalSize,
    /// Optional session name/label
    pub name: Option<String>,
}

impl Default for SessionConfig {
    fn default() -> Self {
        Self {
            shell: None,
            env: vec![],
            size: TerminalSize::default(),
            name: None,
        }
    }
}

/// Abstraction over a terminal session
#[async_trait]
pub trait Session: Send + Sync {
    /// Session identifier
    fn id(&self) -> SessionId;

    /// Machine this session belongs to
    fn machine_id(&self) -> &MachineId;

    /// Current session state
    fn state(&self) -> SessionState;

    /// Optional session name
    fn name(&self) -> Option<&str>;

    /// Write data to session stdin
    async fn write(&self, data: &[u8]) -> Result<(), SessionError>;

    /// Read data from session stdout
    /// Returns None if the session is closed
    async fn read(&self) -> Result<Option<Bytes>, SessionError>;

    /// Resize terminal
    async fn resize(&self, size: TerminalSize) -> Result<(), SessionError>;

    /// Close session
    async fn close(&self) -> Result<(), SessionError>;
}

/// Manages multiple sessions
#[async_trait]
pub trait SessionManager: Send + Sync {
    /// The session type managed
    type Sess: Session;

    /// Create a new session on the specified machine
    async fn create(
        &self,
        machine_id: &MachineId,
        config: SessionConfig,
    ) -> Result<Arc<Self::Sess>, SessionError>;

    /// Get session by ID
    fn get(&self, session_id: SessionId) -> Option<Arc<Self::Sess>>;

    /// List all sessions
    fn list(&self) -> Vec<Arc<Self::Sess>>;

    /// List sessions for a specific machine
    fn list_by_machine(&self, machine_id: &MachineId) -> Vec<Arc<Self::Sess>>;

    /// Close a session
    async fn close(&self, session_id: SessionId) -> Result<(), SessionError>;

    /// Close all sessions for a machine
    async fn close_all_for_machine(&self, machine_id: &MachineId) -> Result<(), SessionError>;
}

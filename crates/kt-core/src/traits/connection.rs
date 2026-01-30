//! Connection traits

use async_trait::async_trait;
use std::sync::Arc;
use std::time::Instant;

use crate::error::ConnectionError;
use crate::types::{Capability, ConnectionStatus, MachineId};
use kt_protocol::{Message, SessionId};

/// Abstraction over a connection to a remote machine
#[async_trait]
pub trait Connection: Send + Sync {
    /// Unique identifier for this connection's machine
    fn machine_id(&self) -> &MachineId;

    /// Human-readable alias for the machine
    fn alias(&self) -> Option<&str>;

    /// Current connection status
    fn status(&self) -> ConnectionStatus;

    /// Machine capabilities
    fn capabilities(&self) -> &Capability;

    /// Send a message over this connection
    async fn send(&self, session_id: SessionId, message: Message) -> Result<(), ConnectionError>;

    /// Check if connection is still alive
    fn is_alive(&self) -> bool;

    /// Time of last successful communication
    fn last_activity(&self) -> Instant;

    /// Close the connection gracefully
    async fn close(&self) -> Result<(), ConnectionError>;
}

/// Connection pool management
#[async_trait]
pub trait ConnectionPool: Send + Sync {
    /// The connection type managed by this pool
    type Conn: Connection;

    /// Get connection by machine ID
    fn get(&self, machine_id: &MachineId) -> Option<Arc<Self::Conn>>;

    /// Get connection by alias
    fn get_by_alias(&self, alias: &str) -> Option<Arc<Self::Conn>>;

    /// List all active connections
    fn list(&self) -> Vec<Arc<Self::Conn>>;

    /// List connections matching a tag
    fn list_by_tag(&self, tag: &str) -> Vec<Arc<Self::Conn>>;

    /// Register a new connection
    async fn register(&self, conn: Self::Conn) -> Result<(), ConnectionError>;

    /// Remove a connection
    async fn remove(&self, machine_id: &MachineId) -> Option<Arc<Self::Conn>>;

    /// Get connection count
    fn len(&self) -> usize;

    /// Check if pool is empty
    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

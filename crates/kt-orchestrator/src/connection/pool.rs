//! Connection pool implementation

use dashmap::DashMap;
use std::sync::Arc;

use kt_core::types::MachineId;

/// Pool of active connections to remote machines
pub struct ConnectionPool {
    /// Connections indexed by machine ID
    connections: DashMap<MachineId, Arc<TunnelConnection>>,
}

/// A connection to a remote machine
pub struct TunnelConnection {
    /// Machine identifier
    pub machine_id: MachineId,
    /// Machine alias
    pub alias: Option<String>,
    // TODO: Add actual connection state
}

impl ConnectionPool {
    /// Create a new empty connection pool
    pub fn new() -> Self {
        Self {
            connections: DashMap::new(),
        }
    }

    /// Get a connection by machine ID
    pub fn get(&self, machine_id: &MachineId) -> Option<Arc<TunnelConnection>> {
        self.connections.get(machine_id).map(|r| Arc::clone(&r))
    }

    /// List all connections
    pub fn list(&self) -> Vec<Arc<TunnelConnection>> {
        self.connections
            .iter()
            .map(|r| Arc::clone(&r))
            .collect()
    }

    /// Number of active connections
    pub fn len(&self) -> usize {
        self.connections.len()
    }

    /// Check if pool is empty
    pub fn is_empty(&self) -> bool {
        self.connections.is_empty()
    }
}

impl Default for ConnectionPool {
    fn default() -> Self {
        Self::new()
    }
}

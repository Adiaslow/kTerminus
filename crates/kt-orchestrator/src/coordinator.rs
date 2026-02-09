//! State coordinator for cross-collection atomicity
//!
//! The `StateCoordinator` provides a unified interface for managing connections
//! and sessions with proper atomicity guarantees. It wraps both the connection
//! pool and session manager behind a single RwLock to ensure that operations
//! spanning both collections cannot be interleaved.
//!
//! # Atomicity Model
//!
//! Normal operations (reading state, checking sessions) acquire a read lock,
//! allowing high concurrency. Operations that modify state across collections
//! (like disconnecting a machine and cleaning up its sessions) acquire a write
//! lock to ensure exclusivity.
//!
//! This prevents race conditions such as:
//! - A client subscribing to a session while it's being removed due to disconnect
//! - A session cleanup task racing with a machine reconnection
//! - Multiple cleanup paths (health monitor, disconnect handler) racing to clean up

use std::sync::Arc;
use tokio::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use kt_core::types::MachineId;

use crate::connection::{ConnectionPool, TunnelConnection};
use crate::session::{SessionHandle, SessionManager};

/// Coordinates access to connections and sessions with cross-collection atomicity.
///
/// The `StateCoordinator` ensures that operations spanning both the connection pool
/// and session manager are atomic - they cannot be interleaved with other operations.
///
/// # Usage
///
/// For most read operations, use direct access to `connections` and `sessions`:
/// ```ignore
/// let conn = state.coordinator.connections.get(&machine_id);
/// let sessions = state.coordinator.sessions.list();
/// ```
///
/// For operations that need atomicity across both collections, acquire the appropriate lock:
/// ```ignore
/// // For read operations that need a consistent snapshot
/// let _lock = state.coordinator.read().await;
/// let conn = state.coordinator.connections.get(&machine_id);
/// let sessions = state.coordinator.sessions.list_for_machine(&machine_id);
///
/// // For write operations that modify both collections
/// let _lock = state.coordinator.write().await;
/// state.coordinator.connections.remove(&machine_id);
/// state.coordinator.sessions.remove_by_machine(&machine_id);
/// ```
///
/// For common atomic operations, use the provided helper methods like `atomic_disconnect`.
pub struct StateCoordinator {
    /// RwLock for cross-collection atomicity.
    /// The unit type `()` indicates this lock is purely for coordination,
    /// not for protecting any specific data.
    inner: RwLock<()>,

    /// Connection pool for managing tunnel connections to agents
    pub connections: Arc<ConnectionPool>,

    /// Session manager for managing terminal sessions
    pub sessions: Arc<SessionManager>,
}

impl StateCoordinator {
    /// Create a new StateCoordinator with fresh connection pool and session manager.
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(()),
            connections: Arc::new(ConnectionPool::new()),
            sessions: Arc::new(SessionManager::new()),
        }
    }

    /// Create a StateCoordinator with existing connection pool and session manager.
    ///
    /// This is useful for testing or when migrating from existing code.
    pub fn with_pools(connections: Arc<ConnectionPool>, sessions: Arc<SessionManager>) -> Self {
        Self {
            inner: RwLock::new(()),
            connections,
            sessions,
        }
    }

    /// Acquire a read lock for cross-collection operations.
    ///
    /// Multiple readers can hold the lock simultaneously, allowing high concurrency
    /// for operations that only need a consistent view of state.
    ///
    /// # When to use
    ///
    /// Use this when you need to:
    /// - Read from both connections and sessions atomically
    /// - Ensure a session doesn't get deleted while you're operating on it
    /// - Get a consistent snapshot of state across collections
    ///
    /// # Example
    ///
    /// ```ignore
    /// let _lock = coordinator.read().await;
    /// let session = coordinator.sessions.get_by_string_id(&session_id)?;
    /// let conn = coordinator.connections.get(&session.machine_id);
    /// // Both reads are guaranteed to see consistent state
    /// ```
    pub async fn read(&self) -> RwLockReadGuard<'_, ()> {
        self.inner.read().await
    }

    /// Acquire a write lock for exclusive cross-collection operations.
    ///
    /// Only one writer can hold the lock, and it blocks all readers.
    /// Use sparingly for operations that modify state across collections.
    ///
    /// # When to use
    ///
    /// Use this when you need to:
    /// - Remove a connection AND its sessions atomically
    /// - Perform state transitions that span both collections
    /// - Ensure no one reads intermediate/inconsistent state
    ///
    /// # Example
    ///
    /// ```ignore
    /// let _lock = coordinator.write().await;
    /// coordinator.connections.remove(&machine_id);
    /// coordinator.sessions.remove_by_machine(&machine_id);
    /// // No other task can see a state where connection is removed but sessions remain
    /// ```
    pub async fn write(&self) -> RwLockWriteGuard<'_, ()> {
        self.inner.write().await
    }

    /// Atomically disconnect a machine and remove all its sessions.
    ///
    /// This method holds a write lock while performing the disconnect, ensuring that:
    /// 1. No new sessions can be created for this machine during disconnect
    /// 2. No subscribers can observe partial state (connection gone, sessions remaining)
    /// 3. All session cleanup happens atomically with connection removal
    ///
    /// # Returns
    ///
    /// A tuple containing:
    /// - The removed connection, if it existed
    /// - A vector of all sessions that were removed for this machine
    ///
    /// # Example
    ///
    /// ```ignore
    /// let (conn, sessions) = coordinator.atomic_disconnect(&machine_id).await;
    ///
    /// // Send events for all removed sessions
    /// for session in sessions {
    ///     event_tx.send(IpcEvent::SessionClosed { session_id: session.id.to_string() });
    /// }
    ///
    /// if conn.is_some() {
    ///     event_tx.send(IpcEvent::MachineDisconnected { machine_id: machine_id.to_string() });
    /// }
    /// ```
    pub async fn atomic_disconnect(
        &self,
        machine_id: &MachineId,
    ) -> (Option<Arc<TunnelConnection>>, Vec<Arc<SessionHandle>>) {
        // Hold write lock for the entire operation
        let _lock = self.write().await;

        // Remove connection first
        let connection = self.connections.remove(machine_id);

        // Then remove all sessions for this machine
        let sessions = self.sessions.remove_by_machine(machine_id);

        (connection, sessions)
    }
}

impl Default for StateCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;

    fn create_test_connection(id: &str) -> TunnelConnection {
        let (tx, _rx) = mpsc::channel(1);
        TunnelConnection::new(
            MachineId::new(id),
            Some(format!("{}-alias", id)),
            Some(format!("{}.local", id)),
            "linux".to_string(),
            "x86_64".to_string(),
            tx,
            CancellationToken::new(),
        )
    }

    #[tokio::test]
    async fn test_coordinator_new() {
        let coordinator = StateCoordinator::new();
        assert!(coordinator.connections.is_empty());
        assert!(coordinator.sessions.is_empty());
    }

    #[tokio::test]
    async fn test_coordinator_with_pools() {
        let connections = Arc::new(ConnectionPool::new());
        let sessions = Arc::new(SessionManager::new());

        connections.insert(create_test_connection("test-machine"));
        sessions.create(MachineId::new("test-machine"), None);

        let coordinator = StateCoordinator::with_pools(connections, sessions);

        assert_eq!(coordinator.connections.len(), 1);
        assert_eq!(coordinator.sessions.len(), 1);
    }

    #[tokio::test]
    async fn test_coordinator_read_lock() {
        let coordinator = StateCoordinator::new();

        // Multiple read locks should be allowed simultaneously
        let lock1 = coordinator.read().await;
        let lock2 = coordinator.read().await;

        drop(lock1);
        drop(lock2);
    }

    #[tokio::test]
    async fn test_coordinator_atomic_disconnect_removes_connection() {
        let coordinator = StateCoordinator::new();
        let machine_id = MachineId::new("test-machine");

        coordinator.connections.insert(create_test_connection("test-machine"));
        assert_eq!(coordinator.connections.len(), 1);

        let (conn, sessions) = coordinator.atomic_disconnect(&machine_id).await;

        assert!(conn.is_some());
        assert_eq!(conn.unwrap().machine_id.as_str(), "test-machine");
        assert!(sessions.is_empty());
        assert!(coordinator.connections.is_empty());
    }

    #[tokio::test]
    async fn test_coordinator_atomic_disconnect_removes_sessions() {
        let coordinator = StateCoordinator::new();
        let machine_id = MachineId::new("test-machine");

        coordinator.connections.insert(create_test_connection("test-machine"));
        coordinator.sessions.create(machine_id.clone(), None);
        coordinator.sessions.create(machine_id.clone(), None);
        coordinator.sessions.create(machine_id.clone(), None);

        assert_eq!(coordinator.connections.len(), 1);
        assert_eq!(coordinator.sessions.len(), 3);

        let (conn, sessions) = coordinator.atomic_disconnect(&machine_id).await;

        assert!(conn.is_some());
        assert_eq!(sessions.len(), 3);
        assert!(coordinator.connections.is_empty());
        assert!(coordinator.sessions.is_empty());
    }

    #[tokio::test]
    async fn test_coordinator_atomic_disconnect_preserves_other_machines() {
        let coordinator = StateCoordinator::new();
        let machine_a = MachineId::new("machine-a");
        let machine_b = MachineId::new("machine-b");

        coordinator.connections.insert(create_test_connection("machine-a"));
        coordinator.connections.insert(create_test_connection("machine-b"));
        coordinator.sessions.create(machine_a.clone(), None);
        coordinator.sessions.create(machine_a.clone(), None);
        coordinator.sessions.create(machine_b.clone(), None);

        let (conn, sessions) = coordinator.atomic_disconnect(&machine_a).await;

        assert!(conn.is_some());
        assert_eq!(sessions.len(), 2);

        // machine-b should still be there
        assert_eq!(coordinator.connections.len(), 1);
        assert_eq!(coordinator.sessions.len(), 1);
        assert!(coordinator.connections.get(&machine_b).is_some());
    }

    #[tokio::test]
    async fn test_coordinator_atomic_disconnect_nonexistent() {
        let coordinator = StateCoordinator::new();
        let machine_id = MachineId::new("nonexistent");

        let (conn, sessions) = coordinator.atomic_disconnect(&machine_id).await;

        assert!(conn.is_none());
        assert!(sessions.is_empty());
    }
}

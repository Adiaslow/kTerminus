//! Connection pool implementation

use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use kt_core::time::current_time_millis;
use kt_core::types::MachineId;
use kt_protocol::{Message, SessionId, TerminalSize};

/// Error returned when connection limit is exceeded
#[derive(Debug, Clone)]
pub struct ConnectionLimitExceeded {
    /// Current number of connections
    pub current: usize,
    /// Maximum allowed connections
    pub max: usize,
}

impl std::fmt::Display for ConnectionLimitExceeded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Connection limit exceeded: {} connections (max {})",
            self.current, self.max
        )
    }
}

impl std::error::Error for ConnectionLimitExceeded {}

/// Commands that can be sent to an agent
#[derive(Debug, Clone)]
pub enum AgentCommand {
    /// Create a new terminal session
    CreateSession {
        session_id: SessionId,
        shell: Option<String>,
        env: Vec<(String, String)>,
        size: TerminalSize,
    },
    /// Send input data to a session
    SessionInput { session_id: SessionId, data: Bytes },
    /// Resize a session's terminal
    SessionResize {
        session_id: SessionId,
        size: TerminalSize,
    },
    /// Close a session
    CloseSession { session_id: SessionId },
    /// Send a heartbeat
    Heartbeat { timestamp: u64 },
}

impl AgentCommand {
    /// Convert an agent command to a protocol message and session ID
    ///
    /// This method transforms the high-level `AgentCommand` into the lower-level
    /// protocol `Message` that can be sent over the wire. The returned session ID
    /// indicates which session the message belongs to (or `SessionId::CONTROL` for
    /// control messages like heartbeats).
    ///
    /// # Returns
    /// A tuple of `(SessionId, Message)` where:
    /// - `SessionId` - The target session for the message
    /// - `Message` - The wire protocol message to send
    pub fn to_message(self) -> (SessionId, Message) {
        match self {
            AgentCommand::CreateSession {
                session_id,
                shell,
                env,
                size,
            } => (
                session_id,
                Message::SessionCreate {
                    shell,
                    env,
                    initial_size: size,
                },
            ),
            AgentCommand::SessionInput { session_id, data } => (session_id, Message::Data(data)),
            AgentCommand::SessionResize { session_id, size } => (session_id, Message::Resize(size)),
            AgentCommand::CloseSession { session_id } => {
                (session_id, Message::SessionClose { exit_code: None })
            }
            AgentCommand::Heartbeat { timestamp } => {
                (SessionId::CONTROL, Message::Heartbeat { timestamp })
            }
        }
    }
}

/// Pool of active connections to remote machines
///
/// `ConnectionPool` provides thread-safe access to all active agent connections.
/// It uses `DashMap` internally for lock-free concurrent reads and writes,
/// making it suitable for high-concurrency scenarios where multiple sessions
/// may be accessing different machines simultaneously.
///
/// # Thread Safety
/// All methods on `ConnectionPool` are thread-safe and can be called from
/// multiple tasks without external synchronization.
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
    /// Hostname
    pub hostname: Option<String>,
    /// Operating system
    pub os: String,
    /// CPU architecture
    pub arch: String,
    /// Channel for sending commands to this agent
    pub command_tx: mpsc::Sender<AgentCommand>,
    /// Cancellation token to disconnect this specific connection
    pub cancel: CancellationToken,
    /// Last heartbeat received (epoch millis)
    last_heartbeat_millis: AtomicU64,
    /// When the connection was established
    connected_at: Instant,
}

impl TunnelConnection {
    /// Create a new tunnel connection
    pub fn new(
        machine_id: MachineId,
        alias: Option<String>,
        hostname: Option<String>,
        os: String,
        arch: String,
        command_tx: mpsc::Sender<AgentCommand>,
        cancel: CancellationToken,
    ) -> Self {
        Self {
            machine_id,
            alias,
            hostname,
            os,
            arch,
            command_tx,
            cancel,
            last_heartbeat_millis: AtomicU64::new(current_time_millis()),
            connected_at: Instant::now(),
        }
    }

    /// Signal this connection to disconnect
    pub fn disconnect(&self) {
        self.cancel.cancel();
    }

    /// Update the last heartbeat timestamp
    pub fn record_heartbeat(&self) {
        self.last_heartbeat_millis
            .store(current_time_millis(), Ordering::SeqCst);
    }

    /// Get the last heartbeat timestamp (epoch millis)
    pub fn last_heartbeat_millis(&self) -> u64 {
        self.last_heartbeat_millis.load(Ordering::SeqCst)
    }

    /// Check if the connection is considered healthy (heartbeat within timeout)
    pub fn is_healthy(&self, timeout: Duration) -> bool {
        let last = self.last_heartbeat_millis.load(Ordering::SeqCst);
        let elapsed = current_time_millis().saturating_sub(last);
        elapsed < timeout.as_millis() as u64
    }

    /// Get connection uptime
    pub fn uptime(&self) -> Duration {
        self.connected_at.elapsed()
    }
}

impl ConnectionPool {
    /// Create a new empty connection pool
    pub fn new() -> Self {
        Self {
            connections: DashMap::new(),
        }
    }

    /// Get a connection by machine ID
    ///
    /// Returns `None` if no connection exists for the given machine ID.
    /// The returned `Arc` can be safely cloned and used across tasks.
    pub fn get(&self, machine_id: &MachineId) -> Option<Arc<TunnelConnection>> {
        self.connections.get(machine_id).map(|r| Arc::clone(&r))
    }

    /// Get a connection by machine ID or alias
    ///
    /// First tries an exact match by machine ID, then falls back to searching
    /// by alias. Returns `None` if no connection matches.
    pub fn get_by_id_or_alias(&self, id_or_alias: &str) -> Option<Arc<TunnelConnection>> {
        // First try exact machine ID match
        let machine_id = MachineId::new(id_or_alias);
        if let Some(conn) = self.get(&machine_id) {
            return Some(conn);
        }

        // Fall back to alias search
        for entry in self.connections.iter() {
            if let Some(ref alias) = entry.alias {
                if alias == id_or_alias {
                    return Some(Arc::clone(&entry));
                }
            }
        }

        None
    }

    /// List all active connections
    ///
    /// Returns a snapshot of all connections at the time of the call.
    /// Note that connections may be added or removed by other tasks
    /// while iterating over the returned vector.
    pub fn list(&self) -> Vec<Arc<TunnelConnection>> {
        self.connections.iter().map(|r| Arc::clone(&r)).collect()
    }

    /// Get the number of active connections
    pub fn len(&self) -> usize {
        self.connections.len()
    }

    /// Check if the pool has no active connections
    pub fn is_empty(&self) -> bool {
        self.connections.is_empty()
    }

    /// Add a connection to the pool
    ///
    /// If a connection with the same machine ID already exists, it will be
    /// replaced. The old connection (if any) is not explicitly disconnected;
    /// callers should handle cleanup before calling `insert`.
    pub fn insert(&self, connection: TunnelConnection) {
        let machine_id = connection.machine_id.clone();
        self.connections.insert(machine_id, Arc::new(connection));
    }

    /// Try to add a connection to the pool, checking against a maximum limit.
    ///
    /// Returns `Ok(())` if the connection was added, or `Err(ConnectionLimitExceeded)` if the pool
    /// is at capacity. If a connection with the same machine ID already exists,
    /// it will be replaced (doesn't count against the limit).
    ///
    /// # Connection Replacement Policy
    ///
    /// Replacement connections (same machine_id) are always allowed, even when at the limit.
    /// This is **intentional behavior** for the following reasons:
    ///
    /// 1. **Network resilience**: Agents may reconnect after network interruptions. Blocking
    ///    reconnection would leave the machine orphaned even though it's trying to restore
    ///    the connection.
    ///
    /// 2. **No amplification**: Replacing a connection doesn't increase the total count.
    ///    The old connection is replaced atomically.
    ///
    /// 3. **Agent identity**: Machine IDs are tied to Tailscale identity, so an attacker
    ///    cannot impersonate a machine to "steal" a connection slot.
    ///
    /// 4. **Graceful failover**: If an agent crashes and restarts, it should be able to
    ///    reconnect without waiting for the old connection to timeout.
    ///
    /// The connection limit only prevents *new* machines from connecting when at capacity.
    ///
    /// # Arguments
    /// * `connection` - The connection to add
    /// * `max_connections` - Optional maximum number of connections. If `None`, no limit is enforced.
    pub fn try_insert(
        &self,
        connection: TunnelConnection,
        max_connections: Option<u32>,
    ) -> Result<(), ConnectionLimitExceeded> {
        let machine_id = connection.machine_id.clone();

        // Check limit only if this is a new connection (not replacing existing)
        // Replacement connections are always allowed - see docstring for rationale
        if let Some(max) = max_connections {
            if !self.connections.contains_key(&machine_id) && self.connections.len() >= max as usize
            {
                return Err(ConnectionLimitExceeded {
                    current: self.connections.len(),
                    max: max as usize,
                });
            }
        }

        self.connections.insert(machine_id, Arc::new(connection));
        Ok(())
    }

    /// Remove a connection from the pool
    ///
    /// Returns the removed connection if it existed, or `None` if no
    /// connection was found for the given machine ID. The returned
    /// connection is not disconnected; callers should call `disconnect()`
    /// on it if needed.
    pub fn remove(&self, machine_id: &MachineId) -> Option<Arc<TunnelConnection>> {
        self.connections.remove(machine_id).map(|(_, v)| v)
    }
}

impl Default for ConnectionPool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn test_connection_pool_new() {
        let pool = ConnectionPool::new();
        assert!(pool.is_empty());
        assert_eq!(pool.len(), 0);
    }

    #[test]
    fn test_connection_pool_insert_and_get() {
        let pool = ConnectionPool::new();
        let conn = create_test_connection("machine-1");

        pool.insert(conn);

        assert_eq!(pool.len(), 1);
        assert!(!pool.is_empty());

        let retrieved = pool.get(&MachineId::new("machine-1"));
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().machine_id.as_str(), "machine-1");
    }

    #[test]
    fn test_connection_pool_get_nonexistent() {
        let pool = ConnectionPool::new();
        let result = pool.get(&MachineId::new("nonexistent"));
        assert!(result.is_none());
    }

    #[test]
    fn test_connection_pool_remove() {
        let pool = ConnectionPool::new();
        pool.insert(create_test_connection("machine-1"));
        pool.insert(create_test_connection("machine-2"));

        assert_eq!(pool.len(), 2);

        let removed = pool.remove(&MachineId::new("machine-1"));
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().machine_id.as_str(), "machine-1");

        assert_eq!(pool.len(), 1);
        assert!(pool.get(&MachineId::new("machine-1")).is_none());
        assert!(pool.get(&MachineId::new("machine-2")).is_some());
    }

    #[test]
    fn test_connection_pool_remove_nonexistent() {
        let pool = ConnectionPool::new();
        let removed = pool.remove(&MachineId::new("nonexistent"));
        assert!(removed.is_none());
    }

    #[test]
    fn test_connection_pool_get_by_id_or_alias() {
        let pool = ConnectionPool::new();
        pool.insert(create_test_connection("machine-1"));

        // Lookup by exact machine ID should work
        let conn = pool.get_by_id_or_alias("machine-1");
        assert!(conn.is_some());
        assert_eq!(conn.unwrap().machine_id.as_str(), "machine-1");

        // Lookup by alias should work (test helper creates alias as "{id}-alias")
        let conn = pool.get_by_id_or_alias("machine-1-alias");
        assert!(conn.is_some());
        assert_eq!(conn.unwrap().machine_id.as_str(), "machine-1");

        // Lookup by nonexistent ID should return None
        let conn = pool.get_by_id_or_alias("nonexistent");
        assert!(conn.is_none());

        // Lookup by nonexistent alias should return None
        let conn = pool.get_by_id_or_alias("nonexistent-alias");
        assert!(conn.is_none());
    }

    #[test]
    fn test_connection_pool_get_by_alias_prefers_exact_id() {
        let pool = ConnectionPool::new();

        // Create a connection where the machine ID is "foo-alias"
        // This tests that exact ID match takes precedence over alias search
        let (tx, _rx) = mpsc::channel(1);
        let conn = TunnelConnection::new(
            MachineId::new("foo-alias"),
            Some("bar".to_string()),  // Different alias
            Some("host.local".to_string()),
            "linux".to_string(),
            "x86_64".to_string(),
            tx,
            CancellationToken::new(),
        );
        pool.insert(conn);

        // Lookup "foo-alias" should find by exact ID, not by searching aliases
        let result = pool.get_by_id_or_alias("foo-alias");
        assert!(result.is_some());
        assert_eq!(result.unwrap().machine_id.as_str(), "foo-alias");
    }

    #[test]
    fn test_connection_pool_list() {
        let pool = ConnectionPool::new();
        pool.insert(create_test_connection("machine-1"));
        pool.insert(create_test_connection("machine-2"));
        pool.insert(create_test_connection("machine-3"));

        let list = pool.list();
        assert_eq!(list.len(), 3);

        let ids: Vec<&str> = list.iter().map(|c| c.machine_id.as_str()).collect();
        assert!(ids.contains(&"machine-1"));
        assert!(ids.contains(&"machine-2"));
        assert!(ids.contains(&"machine-3"));
    }

    #[test]
    fn test_tunnel_connection_fields() {
        let conn = create_test_connection("test-machine");

        assert_eq!(conn.machine_id.as_str(), "test-machine");
        assert_eq!(conn.alias.as_deref(), Some("test-machine-alias"));
        assert_eq!(conn.hostname.as_deref(), Some("test-machine.local"));
        assert_eq!(conn.os, "linux");
        assert_eq!(conn.arch, "x86_64");
    }

    #[test]
    fn test_tunnel_connection_heartbeat() {
        let conn = create_test_connection("test-machine");

        let initial_hb = conn.last_heartbeat_millis();
        assert!(initial_hb > 0);

        // Record a new heartbeat
        std::thread::sleep(std::time::Duration::from_millis(10));
        conn.record_heartbeat();

        let new_hb = conn.last_heartbeat_millis();
        assert!(new_hb >= initial_hb);
    }

    #[test]
    fn test_tunnel_connection_is_healthy() {
        let conn = create_test_connection("test-machine");

        // Should be healthy with a long timeout
        assert!(conn.is_healthy(Duration::from_secs(60)));

        // Should be healthy with a very short timeout right after creation
        assert!(conn.is_healthy(Duration::from_millis(100)));
    }

    #[test]
    fn test_tunnel_connection_disconnect() {
        let conn = create_test_connection("test-machine");
        let cancel = conn.cancel.clone();

        assert!(!cancel.is_cancelled());
        conn.disconnect();
        assert!(cancel.is_cancelled());
    }

    #[test]
    fn test_agent_command_to_message_create_session() {
        let cmd = AgentCommand::CreateSession {
            session_id: SessionId::new(1),
            shell: Some("/bin/bash".to_string()),
            env: vec![("TERM".to_string(), "xterm".to_string())],
            size: TerminalSize { cols: 80, rows: 24 },
        };

        let (session_id, msg) = cmd.to_message();
        assert_eq!(session_id.as_u32(), 1);

        match msg {
            Message::SessionCreate {
                shell,
                env,
                initial_size,
            } => {
                assert_eq!(shell, Some("/bin/bash".to_string()));
                assert_eq!(env, vec![("TERM".to_string(), "xterm".to_string())]);
                assert_eq!(initial_size.cols, 80);
                assert_eq!(initial_size.rows, 24);
            }
            _ => panic!("Expected SessionCreate message"),
        }
    }

    #[test]
    fn test_agent_command_to_message_heartbeat() {
        let cmd = AgentCommand::Heartbeat { timestamp: 12345 };

        let (session_id, msg) = cmd.to_message();
        assert_eq!(session_id, SessionId::CONTROL);

        match msg {
            Message::Heartbeat { timestamp } => {
                assert_eq!(timestamp, 12345);
            }
            _ => panic!("Expected Heartbeat message"),
        }
    }
}

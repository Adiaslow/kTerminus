//! Connection pool implementation

use dashmap::DashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use bytes::Bytes;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use kt_core::types::MachineId;
use kt_protocol::{Message, SessionId, TerminalSize};

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
    /// Convert to protocol message and session ID
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
        let now_millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;

        Self {
            machine_id,
            alias,
            hostname,
            os,
            arch,
            command_tx,
            cancel,
            last_heartbeat_millis: AtomicU64::new(now_millis),
            connected_at: Instant::now(),
        }
    }

    /// Signal this connection to disconnect
    pub fn disconnect(&self) {
        self.cancel.cancel();
    }

    /// Update the last heartbeat timestamp
    pub fn record_heartbeat(&self) {
        let now_millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        self.last_heartbeat_millis
            .store(now_millis, Ordering::SeqCst);
    }

    /// Get the last heartbeat timestamp (epoch millis)
    pub fn last_heartbeat_millis(&self) -> u64 {
        self.last_heartbeat_millis.load(Ordering::SeqCst)
    }

    /// Check if the connection is considered healthy (heartbeat within timeout)
    pub fn is_healthy(&self, timeout: Duration) -> bool {
        let now_millis = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        let last = self.last_heartbeat_millis.load(Ordering::SeqCst);
        let elapsed_millis = now_millis.saturating_sub(last);
        elapsed_millis < timeout.as_millis() as u64
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
    pub fn get(&self, machine_id: &MachineId) -> Option<Arc<TunnelConnection>> {
        self.connections.get(machine_id).map(|r| Arc::clone(&r))
    }

    /// List all connections
    pub fn list(&self) -> Vec<Arc<TunnelConnection>> {
        self.connections.iter().map(|r| Arc::clone(&r)).collect()
    }

    /// Number of active connections
    pub fn len(&self) -> usize {
        self.connections.len()
    }

    /// Check if pool is empty
    pub fn is_empty(&self) -> bool {
        self.connections.is_empty()
    }

    /// Add a connection to the pool
    pub fn insert(&self, connection: TunnelConnection) {
        let machine_id = connection.machine_id.clone();
        self.connections.insert(machine_id, Arc::new(connection));
    }

    /// Remove a connection from the pool
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

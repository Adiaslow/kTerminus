//! IPC protocol for desktop/CLI to orchestrator communication
//!
//! Uses JSON-encoded messages over TCP on localhost (127.0.0.1).
//! TCP is used instead of Unix sockets for cross-platform compatibility
//! (works on macOS, Linux, and Windows without platform-specific code).
//!
//! ## Event Sequencing
//!
//! All events are wrapped in `IpcEventEnvelope` with monotonic sequence numbers.
//! This enables:
//! - Gap detection (clients can detect missing events)
//! - State recovery (via `GetStateSnapshot` and `GetEventsSince` requests)
//! - Epoch tracking (detect orchestrator restarts)

use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use uuid::Uuid;

use crate::time::current_time_millis;

// ============================================================================
// State Epoch - Global event sequencing
// ============================================================================

/// Global state epoch for event ordering and orchestrator identity.
///
/// The epoch provides:
/// - Monotonically increasing sequence numbers for all events
/// - A unique epoch ID that changes on orchestrator restart (for detecting restarts)
/// - Per-session sequence counters for terminal output ordering
#[derive(Debug)]
pub struct StateEpoch {
    /// Monotonically increasing sequence number for all events
    sequence: AtomicU64,
    /// Unique epoch ID (changes on orchestrator restart)
    epoch_id: Uuid,
}

impl StateEpoch {
    /// Create a new state epoch with sequence starting at 0
    pub fn new() -> Self {
        Self {
            sequence: AtomicU64::new(0),
            epoch_id: Uuid::new_v4(),
        }
    }

    /// Get the next sequence number (atomically increments)
    pub fn next_sequence(&self) -> u64 {
        self.sequence.fetch_add(1, Ordering::SeqCst)
    }

    /// Get the current sequence number (without incrementing)
    pub fn current_sequence(&self) -> u64 {
        self.sequence.load(Ordering::SeqCst)
    }

    /// Get the epoch ID
    pub fn epoch_id(&self) -> &Uuid {
        &self.epoch_id
    }

    /// Get the epoch ID as a string
    pub fn epoch_id_string(&self) -> String {
        self.epoch_id.to_string()
    }

    /// Wrap an event in an envelope with the next sequence number
    pub fn wrap_event(&self, event: IpcEvent) -> IpcEventEnvelope {
        IpcEventEnvelope {
            seq: self.next_sequence(),
            timestamp: current_time_millis(),
            event,
            session_seq: None,
        }
    }

    /// Wrap an event with a per-session sequence number (for terminal output)
    pub fn wrap_event_with_session_seq(&self, event: IpcEvent, session_seq: u64) -> IpcEventEnvelope {
        IpcEventEnvelope {
            seq: self.next_sequence(),
            timestamp: current_time_millis(),
            event,
            session_seq: Some(session_seq),
        }
    }
}

impl Default for StateEpoch {
    fn default() -> Self {
        Self::new()
    }
}

/// Event envelope with sequence number for ordering and gap detection
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IpcEventEnvelope {
    /// Monotonically increasing sequence number
    pub seq: u64,
    /// Timestamp when event was generated (milliseconds since Unix epoch)
    pub timestamp: u64,
    /// The actual event
    pub event: IpcEvent,
    /// Per-session sequence number for terminal output ordering
    /// Only present for TerminalOutput events
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_seq: Option<u64>,
}

// ============================================================================
// IPC Protocol Constants
// ============================================================================

/// Default IPC port
pub const DEFAULT_IPC_PORT: u16 = 22230;

/// Default IPC address
pub fn default_ipc_address() -> String {
    format!("127.0.0.1:{}", DEFAULT_IPC_PORT)
}

/// Check if an orchestrator is running by sending a Ping request
///
/// Returns `Ok(true)` if the orchestrator responds with Pong,
/// `Ok(false)` if it responds with something else,
/// or an error if the connection fails.
pub async fn try_ipc_ping(address: &str) -> Result<bool, std::io::Error> {
    try_ipc_ping_with_timeout(address, Duration::from_secs(2)).await
}

/// Check if an orchestrator is running with a custom timeout
pub async fn try_ipc_ping_with_timeout(
    address: &str,
    timeout: Duration,
) -> Result<bool, std::io::Error> {
    // Connect with timeout
    let stream = tokio::time::timeout(timeout, TcpStream::connect(address))
        .await
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "Connection timed out"))?
        ?;

    // Send Ping request
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    let request = IpcRequest::Ping;
    let mut request_json = serde_json::to_string(&request)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
    request_json.push('\n');

    writer.write_all(request_json.as_bytes()).await?;

    // Read response with timeout
    let mut line = String::new();
    tokio::time::timeout(timeout, reader.read_line(&mut line))
        .await
        .map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "Response timed out"))?
        ?;

    // Parse response
    let response: IpcResponse = serde_json::from_str(line.trim())
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;

    Ok(matches!(response, IpcResponse::Pong))
}

/// Check if an orchestrator is running on the default IPC address
pub async fn is_orchestrator_running() -> bool {
    try_ipc_ping(&default_ipc_address()).await.unwrap_or(false)
}

/// IPC request from client (desktop/CLI) to orchestrator
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcRequest {
    /// Authenticate with the orchestrator using a token
    /// This must be the first request after connecting (except Ping)
    Authenticate {
        token: String,
        /// Optional logical client ID for session ownership tracking.
        /// If provided, sessions will be owned by this ID (survives reconnections).
        /// If not provided, sessions are owned by the connection ID (legacy behavior).
        #[serde(default, skip_serializing_if = "Option::is_none")]
        client_id: Option<String>,
    },

    /// Get orchestrator status
    GetStatus,

    /// List connected machines
    ListMachines,

    /// Get details for a specific machine
    GetMachine { machine_id: String },

    /// List sessions (optionally filtered by machine)
    ListSessions { machine_id: Option<String> },

    /// Create a new session on a machine
    CreateSession {
        machine_id: String,
        shell: Option<String>,
    },

    /// Send input to a session
    SessionInput { session_id: String, data: Vec<u8> },

    /// Resize a session's terminal
    SessionResize {
        session_id: String,
        cols: u16,
        rows: u16,
    },

    /// Close a session
    CloseSession { session_id: String, force: bool },

    /// Subscribe to events for a session (terminal output)
    Subscribe { session_id: String },

    /// Unsubscribe from session events
    Unsubscribe { session_id: String },

    /// Disconnect a machine
    DisconnectMachine { machine_id: String },

    /// Ping (for keepalive)
    Ping,

    /// Shutdown the orchestrator
    Shutdown,

    /// Get the pairing code (for displaying in UI)
    GetPairingCode,

    /// Verify a pairing code (for agent discovery)
    VerifyPairingCode { code: String },

    /// Get a full state snapshot for reconciliation
    ///
    /// Returns current epoch_id, sequence number, and all machines/sessions.
    /// Used for initial sync and recovery after event gaps.
    GetStateSnapshot,

    /// Get events since a specific sequence number
    ///
    /// Used to catch up after brief disconnections.
    /// Returns truncated=true if requested events are no longer available.
    GetEventsSince {
        /// Sequence number to start from (exclusive)
        since_seq: u64,
    },
}

/// IPC response from orchestrator to client
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcResponse {
    /// Authentication successful
    Authenticated {
        /// Unique epoch ID (changes on orchestrator restart)
        epoch_id: String,
        /// Current sequence number for gap detection
        current_seq: u64,
    },

    /// Authentication required - client must authenticate before other requests
    AuthenticationRequired,

    /// Orchestrator status
    Status(OrchestratorStatus),

    /// List of machines
    Machines { machines: Vec<MachineInfo> },

    /// Single machine details
    Machine(MachineInfo),

    /// List of sessions
    Sessions { sessions: Vec<SessionInfo> },

    /// Session created
    SessionCreated(SessionInfo),

    /// Generic success
    Ok,

    /// Error response
    Error { message: String },

    /// Pong response
    Pong,

    /// Pairing code response
    PairingCode { code: String },

    /// Pairing code verification result
    PairingCodeValid { valid: bool },

    /// Full state snapshot for reconciliation
    StateSnapshot {
        /// Unique epoch ID (changes on orchestrator restart)
        epoch_id: String,
        /// Current sequence number
        current_seq: u64,
        /// All connected machines
        machines: Vec<MachineInfo>,
        /// All active sessions
        sessions: Vec<SessionInfo>,
    },

    /// Events since requested sequence number
    EventsSince {
        /// Events in order (may be empty if all caught up)
        events: Vec<IpcEventEnvelope>,
        /// True if some requested events are no longer available (too old)
        truncated: bool,
        /// Oldest available sequence number (if truncated)
        oldest_available_seq: Option<u64>,
    },

    /// Subscription successful with current state
    Subscribed {
        /// Current sequence number at subscription time
        current_seq: u64,
        /// Session info at subscription time
        session: SessionInfo,
    },
}

/// IPC event pushed from orchestrator to subscribed clients
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcEvent {
    /// Machine connected
    MachineConnected(MachineInfo),

    /// Machine disconnected
    MachineDisconnected { machine_id: String },

    /// Machine status updated
    MachineUpdated(MachineInfo),

    /// Session created
    SessionCreated(SessionInfo),

    /// Session closed
    SessionClosed { session_id: String },

    /// Terminal output data
    TerminalOutput { session_id: String, data: Vec<u8> },

    /// Orchestrator status changed
    StatusChanged(OrchestratorStatus),

    /// Events were dropped due to slow client
    ///
    /// This is sent when the client's event queue lagged behind.
    /// The client should refresh its state (re-fetch machines, sessions)
    /// to ensure it has accurate information.
    EventsDropped { count: u32 },
}

/// Orchestrator status information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrchestratorStatus {
    /// Whether orchestrator is running
    pub running: bool,
    /// Uptime in seconds
    pub uptime_secs: u64,
    /// Number of connected machines
    pub machine_count: usize,
    /// Number of active sessions
    pub session_count: usize,
    /// Orchestrator version
    pub version: String,
    /// Tailscale hostname (if available)
    pub tailscale_hostname: Option<String>,
    /// Bind address
    pub bind_address: String,
    /// Pairing code for easy agent connection
    pub pairing_code: Option<String>,
}

/// Machine information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MachineInfo {
    /// Unique machine identifier
    pub id: String,
    /// User-friendly alias
    pub alias: Option<String>,
    /// Machine hostname
    pub hostname: String,
    /// Operating system
    pub os: String,
    /// CPU architecture
    pub arch: String,
    /// Connection status
    pub status: MachineStatus,
    /// When the machine connected
    pub connected_at: Option<String>,
    /// Last heartbeat timestamp
    pub last_heartbeat: Option<String>,
    /// Number of active sessions
    pub session_count: usize,
    /// Machine tags
    pub tags: Vec<String>,
}

/// Machine connection status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MachineStatus {
    /// Machine is connected and responsive
    Connected,
    /// Machine is connecting
    Connecting,
    /// Machine is disconnected
    Disconnected,
    /// Connection error
    Error,
}

impl std::fmt::Display for MachineStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MachineStatus::Connected => write!(f, "connected"),
            MachineStatus::Connecting => write!(f, "connecting"),
            MachineStatus::Disconnected => write!(f, "disconnected"),
            MachineStatus::Error => write!(f, "error"),
        }
    }
}

/// Session information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionInfo {
    /// Session ID
    pub id: String,
    /// Machine ID this session belongs to
    pub machine_id: String,
    /// Shell being used
    pub shell: Option<String>,
    /// When the session was created
    pub created_at: String,
    /// Process ID on remote machine
    pub pid: Option<u32>,
    /// Terminal dimensions
    pub size: Option<TerminalSize>,
}

/// Terminal dimensions
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct TerminalSize {
    pub cols: u16,
    pub rows: u16,
}

/// IPC message wrapper (for framing)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IpcMessage {
    Request(IpcRequest),
    Response(IpcResponse),
    Event(IpcEventEnvelope),
}

impl IpcMessage {
    /// Serialize to JSON bytes
    ///
    /// Returns an error if serialization fails (e.g., due to invalid UTF-8 in strings).
    pub fn to_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// Deserialize from JSON bytes
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, serde_json::Error> {
        serde_json::from_slice(bytes)
    }
}

impl From<IpcRequest> for IpcMessage {
    fn from(req: IpcRequest) -> Self {
        IpcMessage::Request(req)
    }
}

impl From<IpcResponse> for IpcMessage {
    fn from(resp: IpcResponse) -> Self {
        IpcMessage::Response(resp)
    }
}

impl From<IpcEventEnvelope> for IpcMessage {
    fn from(envelope: IpcEventEnvelope) -> Self {
        IpcMessage::Event(envelope)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_serialization() {
        let req = IpcRequest::CreateSession {
            machine_id: "machine-1".to_string(),
            shell: Some("/bin/bash".to_string()),
        };

        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("create_session"));
        assert!(json.contains("machine_id"));

        let decoded: IpcRequest = serde_json::from_str(&json).unwrap();
        match decoded {
            IpcRequest::CreateSession { machine_id, shell } => {
                assert_eq!(machine_id, "machine-1");
                assert_eq!(shell, Some("/bin/bash".to_string()));
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_response_serialization() {
        let resp = IpcResponse::Status(OrchestratorStatus {
            running: true,
            uptime_secs: 3600,
            machine_count: 2,
            session_count: 5,
            version: "0.1.0".to_string(),
            tailscale_hostname: Some("my-laptop.ts.net".to_string()),
            bind_address: "0.0.0.0:2222".to_string(),
            pairing_code: Some("ABC123".to_string()),
        });

        let json = serde_json::to_string(&resp).unwrap();
        let decoded: IpcResponse = serde_json::from_str(&json).unwrap();

        match decoded {
            IpcResponse::Status(status) => {
                assert!(status.running);
                assert_eq!(status.machine_count, 2);
                assert_eq!(status.pairing_code, Some("ABC123".to_string()));
            }
            _ => panic!("Wrong variant"),
        }
    }

    #[test]
    fn test_machines_response_serialization() {
        let resp = IpcResponse::Machines { machines: vec![] };
        let json = serde_json::to_string(&resp);
        println!("Machines empty: {:?}", json);

        let resp2 = IpcResponse::Machines {
            machines: vec![MachineInfo {
                id: "test".to_string(),
                alias: None,
                hostname: "test.local".to_string(),
                os: "linux".to_string(),
                arch: "x86_64".to_string(),
                status: MachineStatus::Connected,
                connected_at: None,
                last_heartbeat: None,
                session_count: 0,
                tags: vec![],
            }],
        };
        let json2 = serde_json::to_string(&resp2);
        println!("Machines with one: {:?}", json2);

        // Both should serialize successfully
        assert!(json.is_ok(), "Empty machines should serialize: {:?}", json);
        assert!(
            json2.is_ok(),
            "Machines with data should serialize: {:?}",
            json2
        );
    }

    #[test]
    fn test_sessions_response_serialization() {
        let resp = IpcResponse::Sessions { sessions: vec![] };
        let json = serde_json::to_string(&resp);
        println!("Sessions empty: {:?}", json);
        assert!(json.is_ok(), "Empty sessions should serialize: {:?}", json);
    }
}

//! IPC protocol for desktop/CLI to orchestrator communication
//!
//! Uses JSON-encoded messages over TCP on localhost (127.0.0.1).
//! TCP is used instead of Unix sockets for cross-platform compatibility
//! (works on macOS, Linux, and Windows without platform-specific code).

use serde::{Deserialize, Serialize};

/// IPC request from client (desktop/CLI) to orchestrator
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcRequest {
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
}

/// IPC response from orchestrator to client
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcResponse {
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
    Event(IpcEvent),
}

impl IpcMessage {
    /// Serialize to JSON bytes
    pub fn to_bytes(&self) -> Vec<u8> {
        serde_json::to_vec(self).expect("IpcMessage serialization should not fail")
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

impl From<IpcEvent> for IpcMessage {
    fn from(event: IpcEvent) -> Self {
        IpcMessage::Event(event)
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
        });

        let json = serde_json::to_string(&resp).unwrap();
        let decoded: IpcResponse = serde_json::from_str(&json).unwrap();

        match decoded {
            IpcResponse::Status(status) => {
                assert!(status.running);
                assert_eq!(status.machine_count, 2);
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

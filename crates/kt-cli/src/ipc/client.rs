//! IPC client implementation using JSON-RPC 2.0 over Unix socket

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use anyhow::{Context, Result};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

/// Default socket path
pub const DEFAULT_SOCKET_PATH: &str = "/tmp/k-terminus.sock";

/// JSON-RPC 2.0 request
#[derive(Debug, Clone, Serialize)]
struct JsonRpcRequest<T> {
    jsonrpc: &'static str,
    id: u64,
    method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    params: Option<T>,
}

impl<T> JsonRpcRequest<T> {
    fn new(id: u64, method: impl Into<String>, params: Option<T>) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            method: method.into(),
            params,
        }
    }
}

/// JSON-RPC 2.0 response
#[derive(Debug, Clone, Deserialize)]
struct JsonRpcResponse<T> {
    #[allow(dead_code)]
    jsonrpc: String,
    #[allow(dead_code)]
    id: u64,
    result: Option<T>,
    error: Option<JsonRpcError>,
}

/// JSON-RPC 2.0 error
#[derive(Debug, Clone, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(default)]
    #[allow(dead_code)]
    pub data: Option<serde_json::Value>,
}

impl std::fmt::Display for JsonRpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} (code: {})", self.message, self.code)
    }
}

impl std::error::Error for JsonRpcError {}

/// Machine information returned by the orchestrator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineInfo {
    pub id: String,
    pub alias: Option<String>,
    pub hostname: String,
    pub os: String,
    pub arch: String,
    pub status: String,
    pub connected_at: Option<String>,
    pub last_heartbeat: Option<String>,
    pub session_count: usize,
}

/// Session information returned by the orchestrator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub machine_id: String,
    pub shell: Option<String>,
    pub created_at: String,
    pub pid: Option<u32>,
}

/// Orchestrator status information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OrchestratorStatus {
    pub running: bool,
    pub uptime_secs: u64,
    pub machine_count: usize,
    pub session_count: usize,
    pub version: String,
}

/// Client for communicating with the orchestrator daemon
pub struct OrchestratorClient {
    /// Socket path
    socket_path: PathBuf,
    /// Request ID counter
    request_id: AtomicU64,
    /// Active connection
    stream: Option<UnixStream>,
}

impl OrchestratorClient {
    /// Create a new client with default socket path
    pub fn new() -> Self {
        Self::with_socket_path(PathBuf::from(DEFAULT_SOCKET_PATH))
    }

    /// Create a new client with custom socket path
    pub fn with_socket_path(socket_path: PathBuf) -> Self {
        Self {
            socket_path,
            request_id: AtomicU64::new(1),
            stream: None,
        }
    }

    /// Get the socket path
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    /// Connect to the orchestrator
    pub async fn connect(&mut self) -> Result<()> {
        if self.stream.is_some() {
            return Ok(());
        }

        tracing::debug!("Connecting to orchestrator at {:?}", self.socket_path);

        let stream = UnixStream::connect(&self.socket_path)
            .await
            .with_context(|| {
                format!(
                    "Failed to connect to orchestrator at {:?}. Is it running?",
                    self.socket_path
                )
            })?;

        self.stream = Some(stream);
        Ok(())
    }

    /// Check if the orchestrator is running
    pub async fn ping(&mut self) -> Result<bool> {
        self.connect().await?;

        match self.call::<(), OrchestratorStatus>("status", None).await {
            Ok(_) => Ok(true),
            Err(_) => Ok(false),
        }
    }

    /// Get orchestrator status
    pub async fn status(&mut self) -> Result<OrchestratorStatus> {
        self.connect().await?;
        self.call("status", None::<()>).await
    }

    /// List connected machines
    pub async fn list_machines(&mut self) -> Result<Vec<MachineInfo>> {
        self.connect().await?;
        self.call("list_machines", None::<()>).await
    }

    /// List active sessions
    pub async fn list_sessions(&mut self, machine_id: Option<&str>) -> Result<Vec<SessionInfo>> {
        self.connect().await?;

        #[derive(Serialize)]
        struct Params<'a> {
            machine_id: Option<&'a str>,
        }

        self.call("list_sessions", Some(Params { machine_id })).await
    }

    /// Create a new session on a machine
    pub async fn create_session(
        &mut self,
        machine_id: &str,
        shell: Option<&str>,
    ) -> Result<SessionInfo> {
        self.connect().await?;

        #[derive(Serialize)]
        struct Params<'a> {
            machine_id: &'a str,
            shell: Option<&'a str>,
        }

        self.call("create_session", Some(Params { machine_id, shell })).await
    }

    /// Kill a session
    pub async fn kill_session(&mut self, session_id: &str, force: bool) -> Result<()> {
        self.connect().await?;

        #[derive(Serialize)]
        struct Params<'a> {
            session_id: &'a str,
            force: bool,
        }

        self.call::<_, ()>("kill_session", Some(Params { session_id, force })).await
    }

    /// Make a JSON-RPC call
    async fn call<P: Serialize, R: DeserializeOwned>(
        &mut self,
        method: &str,
        params: Option<P>,
    ) -> Result<R> {
        let stream = self
            .stream
            .as_mut()
            .ok_or_else(|| anyhow::anyhow!("Not connected"))?;

        // Create request
        let id = self.request_id.fetch_add(1, Ordering::SeqCst);
        let request = JsonRpcRequest::new(id, method, params);

        // Serialize and send
        let mut request_bytes = serde_json::to_vec(&request)?;
        request_bytes.push(b'\n'); // Line-delimited JSON

        stream.write_all(&request_bytes).await?;

        // Read response
        let mut reader = BufReader::new(stream);
        let mut response_line = String::new();
        reader.read_line(&mut response_line).await?;

        // Parse response
        let response: JsonRpcResponse<R> = serde_json::from_str(&response_line)?;

        if let Some(error) = response.error {
            anyhow::bail!(error);
        }

        response
            .result
            .ok_or_else(|| anyhow::anyhow!("No result in response"))
    }
}

impl Default for OrchestratorClient {
    fn default() -> Self {
        Self::new()
    }
}

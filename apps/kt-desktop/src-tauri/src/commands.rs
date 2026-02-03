//! Tauri command implementations
//!
//! These commands are called from the frontend via Tauri's IPC mechanism.
//! They communicate with the orchestrator daemon via Unix socket IPC.

use serde::{Deserialize, Serialize};
use tauri::State;

use kt_core::ipc::{IpcRequest, IpcResponse};

use crate::state::AppState;

/// Machine information for frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Machine {
    pub id: String,
    pub alias: Option<String>,
    pub hostname: String,
    pub os: String,
    pub arch: String,
    pub status: String,
    pub connected_at: Option<String>,
    pub last_heartbeat: Option<String>,
    pub session_count: usize,
    pub tags: Option<Vec<String>>,
}

impl From<kt_core::ipc::MachineInfo> for Machine {
    fn from(info: kt_core::ipc::MachineInfo) -> Self {
        Self {
            id: info.id,
            alias: info.alias,
            hostname: info.hostname,
            os: info.os,
            arch: info.arch,
            status: info.status.to_string(),
            connected_at: info.connected_at,
            last_heartbeat: info.last_heartbeat,
            session_count: info.session_count,
            tags: if info.tags.is_empty() {
                None
            } else {
                Some(info.tags)
            },
        }
    }
}

/// Session information for frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub machine_id: String,
    pub shell: Option<String>,
    pub created_at: String,
    pub pid: Option<u32>,
}

impl From<kt_core::ipc::SessionInfo> for Session {
    fn from(info: kt_core::ipc::SessionInfo) -> Self {
        Self {
            id: info.id,
            machine_id: info.machine_id,
            shell: info.shell,
            created_at: info.created_at,
            pid: info.pid,
        }
    }
}

/// Orchestrator status for frontend
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrchestratorStatus {
    pub running: bool,
    pub uptime_secs: u64,
    pub machine_count: usize,
    pub session_count: usize,
    pub version: String,
    pub tailscale_hostname: Option<String>,
}

impl From<kt_core::ipc::OrchestratorStatus> for OrchestratorStatus {
    fn from(status: kt_core::ipc::OrchestratorStatus) -> Self {
        Self {
            running: status.running,
            uptime_secs: status.uptime_secs,
            machine_count: status.machine_count,
            session_count: status.session_count,
            version: status.version,
            tailscale_hostname: status.tailscale_hostname,
        }
    }
}

impl Default for OrchestratorStatus {
    fn default() -> Self {
        Self {
            running: false,
            uptime_secs: 0,
            machine_count: 0,
            session_count: 0,
            version: env!("CARGO_PKG_VERSION").to_string(),
            tailscale_hostname: None,
        }
    }
}

/// Get orchestrator status
#[tauri::command]
pub async fn get_status(state: State<'_, AppState>) -> Result<OrchestratorStatus, String> {
    match state.ipc.request(IpcRequest::GetStatus).await {
        Ok(IpcResponse::Status(status)) => Ok(status.into()),
        Ok(IpcResponse::Error { message }) => Err(message),
        Ok(_) => Err("Unexpected response from orchestrator".to_string()),
        Err(e) => {
            // Orchestrator not running - return offline status
            tracing::debug!("Orchestrator not running: {}", e);
            Ok(OrchestratorStatus::default())
        }
    }
}

/// Start the orchestrator (embedded in the GUI)
#[tauri::command]
pub async fn start_orchestrator(state: State<'_, AppState>) -> Result<(), String> {
    let mut orchestrator = state.orchestrator.write().await;

    if orchestrator.is_running() {
        return Ok(());
    }

    orchestrator
        .start()
        .await
        .map_err(|e| format!("Failed to start orchestrator: {}", e))?;

    // Wait a moment for it to be ready
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

    Ok(())
}

/// Stop the orchestrator (embedded in the GUI)
#[tauri::command]
pub async fn stop_orchestrator(state: State<'_, AppState>) -> Result<(), String> {
    let mut orchestrator = state.orchestrator.write().await;

    if !orchestrator.is_running() {
        return Ok(());
    }

    orchestrator.stop();
    Ok(())
}

/// List all connected machines
#[tauri::command]
pub async fn list_machines(state: State<'_, AppState>) -> Result<Vec<Machine>, String> {
    match state.ipc.request(IpcRequest::ListMachines).await {
        Ok(IpcResponse::Machines { machines }) => {
            Ok(machines.into_iter().map(Into::into).collect())
        }
        Ok(IpcResponse::Error { message }) => Err(message),
        Ok(_) => Err("Unexpected response from orchestrator".to_string()),
        Err(e) => {
            tracing::debug!("Failed to list machines: {}", e);
            Ok(vec![]) // Return empty list if not connected
        }
    }
}

/// Get a specific machine by ID
#[tauri::command]
pub async fn get_machine(state: State<'_, AppState>, id: String) -> Result<Machine, String> {
    match state
        .ipc
        .request(IpcRequest::GetMachine {
            machine_id: id.clone(),
        })
        .await
    {
        Ok(IpcResponse::Machine(machine)) => Ok(machine.into()),
        Ok(IpcResponse::Error { message }) => Err(message),
        Ok(_) => Err("Unexpected response from orchestrator".to_string()),
        Err(e) => Err(format!("Failed to get machine {}: {}", id, e)),
    }
}

/// Disconnect a machine
#[tauri::command]
pub async fn disconnect_machine(state: State<'_, AppState>, id: String) -> Result<(), String> {
    match state
        .ipc
        .request(IpcRequest::DisconnectMachine {
            machine_id: id.clone(),
        })
        .await
    {
        Ok(IpcResponse::Ok) => Ok(()),
        Ok(IpcResponse::Error { message }) => Err(message),
        Ok(_) => Err("Unexpected response from orchestrator".to_string()),
        Err(e) => Err(format!("Failed to disconnect machine {}: {}", id, e)),
    }
}

/// List sessions, optionally filtered by machine
#[tauri::command]
pub async fn list_sessions(
    state: State<'_, AppState>,
    machine_id: Option<String>,
) -> Result<Vec<Session>, String> {
    match state
        .ipc
        .request(IpcRequest::ListSessions { machine_id })
        .await
    {
        Ok(IpcResponse::Sessions { sessions }) => {
            Ok(sessions.into_iter().map(Into::into).collect())
        }
        Ok(IpcResponse::Error { message }) => Err(message),
        Ok(_) => Err("Unexpected response from orchestrator".to_string()),
        Err(e) => {
            tracing::debug!("Failed to list sessions: {}", e);
            Ok(vec![])
        }
    }
}

/// Create a new session on a machine
#[tauri::command]
pub async fn create_session(
    state: State<'_, AppState>,
    machine_id: String,
    shell: Option<String>,
) -> Result<Session, String> {
    match state
        .ipc
        .request(IpcRequest::CreateSession { machine_id, shell })
        .await
    {
        Ok(IpcResponse::SessionCreated(session)) => Ok(session.into()),
        Ok(IpcResponse::Error { message }) => Err(message),
        Ok(_) => Err("Unexpected response from orchestrator".to_string()),
        Err(e) => Err(format!("Failed to create session: {}", e)),
    }
}

/// Kill a session
#[tauri::command]
pub async fn kill_session(
    state: State<'_, AppState>,
    session_id: String,
    force: bool,
) -> Result<(), String> {
    match state
        .ipc
        .request(IpcRequest::CloseSession { session_id, force })
        .await
    {
        Ok(IpcResponse::Ok) => Ok(()),
        Ok(IpcResponse::Error { message }) => Err(message),
        Ok(_) => Err("Unexpected response from orchestrator".to_string()),
        Err(e) => Err(format!("Failed to kill session: {}", e)),
    }
}

/// Write data to a terminal session
#[tauri::command]
pub async fn terminal_write(
    state: State<'_, AppState>,
    session_id: String,
    data: Vec<u8>,
) -> Result<(), String> {
    match state
        .ipc
        .request(IpcRequest::SessionInput { session_id, data })
        .await
    {
        Ok(IpcResponse::Ok) => Ok(()),
        Ok(IpcResponse::Error { message }) => Err(message),
        Ok(_) => Err("Unexpected response from orchestrator".to_string()),
        Err(e) => Err(format!("Failed to write to terminal: {}", e)),
    }
}

/// Resize a terminal session
#[tauri::command]
pub async fn terminal_resize(
    state: State<'_, AppState>,
    session_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    match state
        .ipc
        .request(IpcRequest::SessionResize {
            session_id,
            cols,
            rows,
        })
        .await
    {
        Ok(IpcResponse::Ok) => Ok(()),
        Ok(IpcResponse::Error { message }) => Err(message),
        Ok(_) => Err("Unexpected response from orchestrator".to_string()),
        Err(e) => Err(format!("Failed to resize terminal: {}", e)),
    }
}

/// Close a terminal session
#[tauri::command]
pub async fn terminal_close(state: State<'_, AppState>, session_id: String) -> Result<(), String> {
    kill_session(state, session_id, false).await
}

/// Subscribe to a session's events (terminal output)
#[tauri::command]
pub async fn subscribe_session(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    // Send subscribe request through the event subscriber connection
    let subscriber = state.event_subscriber.read().await;
    subscriber
        .send(IpcRequest::Subscribe {
            session_id: session_id.clone(),
        })
        .await
        .map_err(|e| format!("Failed to subscribe to session {}: {}", session_id, e))
}

/// Unsubscribe from a session's events
#[tauri::command]
pub async fn unsubscribe_session(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    let subscriber = state.event_subscriber.read().await;
    subscriber
        .send(IpcRequest::Unsubscribe {
            session_id: session_id.clone(),
        })
        .await
        .map_err(|e| format!("Failed to unsubscribe from session {}: {}", session_id, e))
}

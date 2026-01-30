//! Tauri command implementations

use tauri::State;

use crate::state::{AppState, Machine, OrchestratorStatus, Session};

/// Get orchestrator status
#[tauri::command]
pub async fn get_status(state: State<'_, AppState>) -> Result<OrchestratorStatus, String> {
    let status = state.status.read().await;
    Ok(status.clone())
}

/// Start the orchestrator daemon
#[tauri::command]
pub async fn start_orchestrator(state: State<'_, AppState>) -> Result<(), String> {
    // In a full implementation, this would:
    // 1. Spawn the kt-orchestrator process
    // 2. Connect to it via IPC
    // 3. Update the status

    let mut status = state.status.write().await;
    status.running = true;
    status.uptime_secs = 0;

    // Mock: Add some demo machines
    let mut machines = state.machines.write().await;
    machines.insert(
        "machine-001".to_string(),
        Machine {
            id: "machine-001".to_string(),
            alias: Some("dev-server".to_string()),
            hostname: "dev-server.local".to_string(),
            os: "linux".to_string(),
            arch: "x86_64".to_string(),
            status: "connected".to_string(),
            connected_at: Some("2024-01-15T10:30:00Z".to_string()),
            last_heartbeat: Some("2024-01-15T10:35:00Z".to_string()),
            session_count: 0,
            tags: Some(vec!["development".to_string()]),
        },
    );
    machines.insert(
        "machine-002".to_string(),
        Machine {
            id: "machine-002".to_string(),
            alias: Some("gpu-node".to_string()),
            hostname: "gpu-01.compute.local".to_string(),
            os: "linux".to_string(),
            arch: "x86_64".to_string(),
            status: "connected".to_string(),
            connected_at: Some("2024-01-15T10:31:00Z".to_string()),
            last_heartbeat: Some("2024-01-15T10:35:00Z".to_string()),
            session_count: 1,
            tags: Some(vec!["gpu".to_string(), "compute".to_string()]),
        },
    );

    status.machine_count = machines.len();

    Ok(())
}

/// Stop the orchestrator daemon
#[tauri::command]
pub async fn stop_orchestrator(state: State<'_, AppState>) -> Result<(), String> {
    let mut status = state.status.write().await;
    status.running = false;
    status.uptime_secs = 0;
    status.machine_count = 0;
    status.session_count = 0;

    // Clear machines and sessions
    state.machines.write().await.clear();
    state.sessions.write().await.clear();

    Ok(())
}

/// List all connected machines
#[tauri::command]
pub async fn list_machines(state: State<'_, AppState>) -> Result<Vec<Machine>, String> {
    let machines = state.machines.read().await;
    Ok(machines.values().cloned().collect())
}

/// Get a specific machine by ID
#[tauri::command]
pub async fn get_machine(
    state: State<'_, AppState>,
    id: String,
) -> Result<Machine, String> {
    let machines = state.machines.read().await;
    machines
        .get(&id)
        .cloned()
        .ok_or_else(|| format!("Machine not found: {}", id))
}

/// List sessions, optionally filtered by machine
#[tauri::command]
pub async fn list_sessions(
    state: State<'_, AppState>,
    machine_id: Option<String>,
) -> Result<Vec<Session>, String> {
    let sessions = state.sessions.read().await;

    let result: Vec<Session> = if let Some(mid) = machine_id {
        sessions
            .values()
            .filter(|s| s.machine_id == mid)
            .cloned()
            .collect()
    } else {
        sessions.values().cloned().collect()
    };

    Ok(result)
}

/// Create a new session on a machine
#[tauri::command]
pub async fn create_session(
    state: State<'_, AppState>,
    machine_id: String,
    shell: Option<String>,
) -> Result<Session, String> {
    // Verify machine exists
    {
        let machines = state.machines.read().await;
        if !machines.contains_key(&machine_id) {
            return Err(format!("Machine not found: {}", machine_id));
        }
    }

    // Generate session ID
    let session_id = format!("session-{}", uuid_simple());

    let session = Session {
        id: session_id.clone(),
        machine_id: machine_id.clone(),
        shell,
        created_at: chrono_now(),
        pid: Some(12345), // Mock PID
    };

    // Add session
    {
        let mut sessions = state.sessions.write().await;
        sessions.insert(session_id, session.clone());
    }

    // Update machine session count
    {
        let mut machines = state.machines.write().await;
        if let Some(machine) = machines.get_mut(&machine_id) {
            machine.session_count += 1;
        }
    }

    // Update status
    {
        let mut status = state.status.write().await;
        status.session_count += 1;
    }

    Ok(session)
}

/// Kill a session
#[tauri::command]
pub async fn kill_session(
    state: State<'_, AppState>,
    session_id: String,
    _force: bool,
) -> Result<(), String> {
    let machine_id = {
        let mut sessions = state.sessions.write().await;
        let session = sessions
            .remove(&session_id)
            .ok_or_else(|| format!("Session not found: {}", session_id))?;
        session.machine_id
    };

    // Update machine session count
    {
        let mut machines = state.machines.write().await;
        if let Some(machine) = machines.get_mut(&machine_id) {
            machine.session_count = machine.session_count.saturating_sub(1);
        }
    }

    // Update status
    {
        let mut status = state.status.write().await;
        status.session_count = status.session_count.saturating_sub(1);
    }

    Ok(())
}

/// Write data to a terminal session
#[tauri::command]
pub async fn terminal_write(
    _state: State<'_, AppState>,
    session_id: String,
    data: Vec<u8>,
) -> Result<(), String> {
    // In a full implementation, this would send data to the orchestrator
    // which would forward it to the agent's PTY
    tracing::debug!(
        "Terminal write to {}: {} bytes",
        session_id,
        data.len()
    );
    Ok(())
}

/// Resize a terminal session
#[tauri::command]
pub async fn terminal_resize(
    _state: State<'_, AppState>,
    session_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    // In a full implementation, this would send resize to the orchestrator
    tracing::debug!(
        "Terminal resize {}: {}x{}",
        session_id,
        cols,
        rows
    );
    Ok(())
}

/// Close a terminal session
#[tauri::command]
pub async fn terminal_close(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    kill_session(state, session_id, false).await
}

// Helper functions

fn uuid_simple() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap();
    format!("{:x}{:x}", duration.as_secs(), duration.subsec_nanos())
}

fn chrono_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let duration = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap();
    format!("{}Z", duration.as_secs())
}

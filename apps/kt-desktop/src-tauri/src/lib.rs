//! k-Terminus Desktop - Tauri Backend

mod commands;
mod ipc_client;
mod orchestrator;
mod state;

use kt_core::ipc::IpcEvent;
use serde::Serialize;
use tauri::{async_runtime, Emitter, Manager};

pub use state::AppState;

/// Terminal output event payload for frontend
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TerminalOutputPayload {
    session_id: String,
    data: Vec<u8>,
}

/// Machine event payload for frontend
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MachineEventPayload {
    #[serde(rename = "type")]
    event_type: String,
    machine: Option<serde_json::Value>,
    machine_id: Option<String>,
}

/// Session event payload for frontend
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct SessionEventPayload {
    #[serde(rename = "type")]
    event_type: String,
    session: Option<serde_json::Value>,
    session_id: Option<String>,
}

/// Initialize and run the Tauri application
#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize logging first
    tracing_subscriber::fmt::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            // Initialize application state
            let state = AppState::new();

            // Clone what we need for the async initialization
            let orchestrator = state.orchestrator.clone();
            let event_subscriber = state.event_subscriber.clone();
            let app_handle = app.handle().clone();

            app.manage(state);

            // Spawn async initialization after Tauri's runtime is ready
            async_runtime::spawn(async move {
                // Start the embedded orchestrator
                {
                    let mut orch = orchestrator.write().await;
                    if let Err(e) = orch.start().await {
                        tracing::error!("Failed to start orchestrator: {}", e);
                    }
                }

                // Give orchestrator a moment to bind sockets
                tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

                // Start event subscriber
                let event_rx = {
                    let mut subscriber = event_subscriber.write().await;
                    subscriber.start()
                };

                // Forward events to frontend
                forward_events(app_handle, event_rx).await;
            });

            // Get the main window
            if let Some(window) = app.get_webview_window("main") {
                #[cfg(debug_assertions)]
                {
                    window.open_devtools();
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::start_orchestrator,
            commands::stop_orchestrator,
            commands::list_machines,
            commands::get_machine,
            commands::disconnect_machine,
            commands::list_sessions,
            commands::create_session,
            commands::kill_session,
            commands::terminal_write,
            commands::terminal_resize,
            commands::terminal_close,
            commands::subscribe_session,
            commands::unsubscribe_session,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// Forward IPC events from orchestrator to frontend
async fn forward_events(
    app_handle: tauri::AppHandle,
    mut event_rx: tokio::sync::mpsc::Receiver<IpcEvent>,
) {
    tracing::info!("Starting event forwarder");

    while let Some(event) = event_rx.recv().await {
        match event {
            IpcEvent::TerminalOutput { session_id, data } => {
                let event_name = format!("terminal-output:{}", session_id);
                let payload = TerminalOutputPayload {
                    session_id: session_id.clone(),
                    data,
                };

                if let Err(e) = app_handle.emit(&event_name, payload) {
                    tracing::warn!("Failed to emit terminal output event: {}", e);
                }
            }

            IpcEvent::MachineConnected(machine) => {
                let payload = MachineEventPayload {
                    event_type: "connected".to_string(),
                    machine: Some(serde_json::to_value(&machine).unwrap_or_default()),
                    machine_id: None,
                };
                if let Err(e) = app_handle.emit("machine-event", payload) {
                    tracing::debug!("Failed to emit machine-connected event: {}", e);
                }
            }

            IpcEvent::MachineDisconnected { machine_id } => {
                let payload = MachineEventPayload {
                    event_type: "disconnected".to_string(),
                    machine: None,
                    machine_id: Some(machine_id),
                };
                if let Err(e) = app_handle.emit("machine-event", payload) {
                    tracing::debug!("Failed to emit machine-disconnected event: {}", e);
                }
            }

            IpcEvent::MachineUpdated(machine) => {
                let payload = MachineEventPayload {
                    event_type: "updated".to_string(),
                    machine: Some(serde_json::to_value(&machine).unwrap_or_default()),
                    machine_id: None,
                };
                if let Err(e) = app_handle.emit("machine-event", payload) {
                    tracing::debug!("Failed to emit machine-updated event: {}", e);
                }
            }

            IpcEvent::SessionCreated(session) => {
                let payload = SessionEventPayload {
                    event_type: "created".to_string(),
                    session: Some(serde_json::to_value(&session).unwrap_or_default()),
                    session_id: None,
                };
                if let Err(e) = app_handle.emit("session-event", payload) {
                    tracing::debug!("Failed to emit session-created event: {}", e);
                }
            }

            IpcEvent::SessionClosed { session_id } => {
                let payload = SessionEventPayload {
                    event_type: "closed".to_string(),
                    session: None,
                    session_id: Some(session_id),
                };
                if let Err(e) = app_handle.emit("session-event", payload) {
                    tracing::debug!("Failed to emit session-closed event: {}", e);
                }
            }

            IpcEvent::StatusChanged(status) => {
                if let Err(e) = app_handle.emit("orchestrator-status", status) {
                    tracing::debug!("Failed to emit orchestrator-status event: {}", e);
                }
            }
        }
    }

    tracing::info!("Event forwarder stopped");
}

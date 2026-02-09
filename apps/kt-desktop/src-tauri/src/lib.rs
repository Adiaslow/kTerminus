//! k-Terminus Desktop - Tauri Backend

mod commands;
mod ipc_client;
mod orchestrator;
mod state;

use std::sync::Arc;

use kt_core::ipc::IpcEvent;
use kt_core::try_ipc_ping;
use serde::Serialize;
use tauri::{async_runtime, Emitter, Manager};
use tokio::sync::RwLock;

use crate::ipc_client::PersistentIpcClient;
use crate::orchestrator::EmbeddedOrchestrator;

pub use state::{AppState, OrchestratorMode};

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
            let orchestrator_mode = state.orchestrator_mode.clone();
            let event_subscriber = state.event_subscriber.clone();
            let app_handle = app.handle().clone();

            app.manage(state);

            // Spawn async initialization after Tauri's runtime is ready
            async_runtime::spawn(async move {
                // Smart startup: Try to connect to existing orchestrator first
                let mode = initialize_orchestrator(&orchestrator).await;
                *orchestrator_mode.write().await = mode;

                tracing::info!("Orchestrator mode: {:?}", mode);

                // Start event subscriber
                let event_rx = {
                    let mut subscriber = event_subscriber.write().await;
                    subscriber.start()
                };

                // Forward events to frontend
                forward_events(app_handle, event_rx).await;
            });

            // Open devtools in debug builds
            #[cfg(debug_assertions)]
            if let Some(window) = app.get_webview_window("main") {
                window.open_devtools();
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::get_status,
            commands::get_state_snapshot,
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

/// Smart startup: Check for existing orchestrator or start embedded
///
/// This implements the "just works" philosophy:
/// 1. Check token ownership to detect if another orchestrator process is alive
/// 2. If alive, verify it's responding (ping) and use External mode
/// 3. If not responding or no owner, start embedded orchestrator
///
/// The token ownership check is the authoritative source - it checks if the
/// process that wrote the token is still alive, preventing token mismatches.
async fn initialize_orchestrator(
    orchestrator: &Arc<RwLock<EmbeddedOrchestrator>>,
) -> OrchestratorMode {
    let address = PersistentIpcClient::default_address();

    tracing::info!("Checking for existing orchestrator...");

    // Check token ownership first - this is authoritative
    match kt_core::read_token_info() {
        Ok(Some(info)) => {
            if kt_core::is_process_alive(info.pid) {
                // There's a live orchestrator process - verify it's responding
                tracing::info!(
                    "Found live orchestrator process (PID {}) at {}",
                    info.pid,
                    info.address
                );

                match try_ipc_ping(&info.address).await {
                    Ok(true) => {
                        tracing::info!("External orchestrator is healthy, using external mode");
                        return OrchestratorMode::External;
                    }
                    Ok(false) => {
                        tracing::warn!(
                            "Orchestrator (PID {}) responded but not with Pong - it may be unhealthy",
                            info.pid
                        );
                        // Still use external mode - the process is alive
                        return OrchestratorMode::External;
                    }
                    Err(e) => {
                        tracing::warn!(
                            "Orchestrator (PID {}) is alive but not responding to ping: {}",
                            info.pid,
                            e
                        );
                        // Process is alive but not responding - might be starting up
                        // Wait a bit and try again
                        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
                        if try_ipc_ping(&info.address).await.unwrap_or(false) {
                            tracing::info!("External orchestrator now responding, using external mode");
                            return OrchestratorMode::External;
                        }
                        // Still not responding - the process might be stuck
                        // We should use external mode anyway to avoid overwriting their token
                        tracing::warn!(
                            "External orchestrator (PID {}) not responding, but keeping external mode to avoid conflicts",
                            info.pid
                        );
                        return OrchestratorMode::External;
                    }
                }
            } else {
                tracing::debug!("Previous orchestrator (PID {}) is no longer running", info.pid);
            }
        }
        Ok(None) => {
            tracing::debug!("No existing token file found");
        }
        Err(e) => {
            tracing::debug!("Failed to read token info: {}", e);
        }
    }

    // No live orchestrator - start embedded
    tracing::info!("Starting embedded orchestrator...");
    {
        let mut orch = orchestrator.write().await;
        if let Err(e) = orch.start().await {
            tracing::error!("Failed to start embedded orchestrator: {}", e);
            return OrchestratorMode::NotConnected;
        }
    }

    // Give orchestrator a moment to bind sockets
    tokio::time::sleep(tokio::time::Duration::from_millis(300)).await;

    // Verify it's running
    match try_ipc_ping(&address).await {
        Ok(true) => {
            tracing::info!("Embedded orchestrator started successfully");
            OrchestratorMode::Embedded
        }
        Ok(false) | Err(_) => {
            tracing::error!("Embedded orchestrator started but not responding");
            OrchestratorMode::NotConnected
        }
    }
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

            IpcEvent::EventsDropped { count } => {
                // Notify frontend that events were dropped so it can refresh state
                tracing::warn!(
                    "IPC event queue lagged, {} events dropped - frontend should refresh",
                    count
                );
                if let Err(e) = app_handle.emit("events-dropped", serde_json::json!({ "count": count })) {
                    tracing::debug!("Failed to emit events-dropped event: {}", e);
                }
            }
        }
    }

    tracing::info!("Event forwarder stopped");
}

//! Application state management

use std::sync::Arc;

use tokio::sync::RwLock;
use uuid::Uuid;

use crate::ipc_client::{EventSubscriber, PersistentIpcClient};
use crate::orchestrator::EmbeddedOrchestrator;

/// How the orchestrator was started
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrchestratorMode {
    /// We started the embedded orchestrator
    Embedded,
    /// Connected to an external orchestrator (standalone daemon or another app instance)
    External,
    /// Not connected yet
    NotConnected,
}

/// Application state shared across commands
pub struct AppState {
    /// IPC client for orchestrator communication (request/response)
    /// Uses a persistent connection with a stable client ID for session ownership
    pub ipc: Arc<PersistentIpcClient>,
    /// Event subscriber for receiving orchestrator events
    pub event_subscriber: Arc<RwLock<EventSubscriber>>,
    /// Embedded orchestrator instance
    pub orchestrator: Arc<RwLock<EmbeddedOrchestrator>>,
    /// How the orchestrator was started (embedded vs external)
    pub orchestrator_mode: Arc<RwLock<OrchestratorMode>>,
}

impl AppState {
    pub fn new() -> Self {
        let address = PersistentIpcClient::default_address();

        // Generate a stable client ID for this app instance.
        // This ID is used for session ownership and survives reconnections.
        let client_id = Uuid::new_v4().to_string();
        tracing::info!("Generated client ID for session ownership: {}", client_id);

        Self {
            ipc: Arc::new(PersistentIpcClient::new(address.clone(), client_id.clone())),
            event_subscriber: Arc::new(RwLock::new(
                EventSubscriber::new(address).with_client_id(client_id),
            )),
            orchestrator: Arc::new(RwLock::new(EmbeddedOrchestrator::new())),
            orchestrator_mode: Arc::new(RwLock::new(OrchestratorMode::NotConnected)),
        }
    }

    /// Set the orchestrator mode
    pub async fn set_mode(&self, mode: OrchestratorMode) {
        *self.orchestrator_mode.write().await = mode;
    }

    /// Get the current orchestrator mode
    pub async fn get_mode(&self) -> OrchestratorMode {
        *self.orchestrator_mode.read().await
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

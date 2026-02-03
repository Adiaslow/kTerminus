//! Application state management

use std::sync::Arc;

use tokio::sync::RwLock;

use crate::ipc_client::{EventSubscriber, SimpleIpcClient};
use crate::orchestrator::EmbeddedOrchestrator;

/// Application state shared across commands
pub struct AppState {
    /// IPC client for orchestrator communication (request/response)
    pub ipc: Arc<SimpleIpcClient>,
    /// Event subscriber for receiving orchestrator events
    pub event_subscriber: Arc<RwLock<EventSubscriber>>,
    /// Embedded orchestrator instance
    pub orchestrator: Arc<RwLock<EmbeddedOrchestrator>>,
}

impl AppState {
    pub fn new() -> Self {
        let address = SimpleIpcClient::default_address();
        Self {
            ipc: Arc::new(SimpleIpcClient::new(address.clone())),
            event_subscriber: Arc::new(RwLock::new(EventSubscriber::new(address))),
            orchestrator: Arc::new(RwLock::new(EmbeddedOrchestrator::new())),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

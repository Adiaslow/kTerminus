//! Application state management

use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

use serde::{Deserialize, Serialize};

/// Machine information
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

/// Session information
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub machine_id: String,
    pub shell: Option<String>,
    pub created_at: String,
    pub pid: Option<u32>,
}

/// Orchestrator status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OrchestratorStatus {
    pub running: bool,
    pub uptime_secs: u64,
    pub machine_count: usize,
    pub session_count: usize,
    pub version: String,
}

impl Default for OrchestratorStatus {
    fn default() -> Self {
        Self {
            running: false,
            uptime_secs: 0,
            machine_count: 0,
            session_count: 0,
            version: env!("CARGO_PKG_VERSION").to_string(),
        }
    }
}

/// Application state shared across commands
pub struct AppState {
    /// Current orchestrator status
    pub status: Arc<RwLock<OrchestratorStatus>>,
    /// Connected machines
    pub machines: Arc<RwLock<HashMap<String, Machine>>>,
    /// Active sessions
    pub sessions: Arc<RwLock<HashMap<String, Session>>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            status: Arc::new(RwLock::new(OrchestratorStatus::default())),
            machines: Arc::new(RwLock::new(HashMap::new())),
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

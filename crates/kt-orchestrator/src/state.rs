//! Global orchestrator state

use std::sync::Arc;

use kt_core::config::OrchestratorConfig;

use crate::auth::TailscaleVerifier;
use crate::connection::ConnectionPool;
use crate::session::SessionManager;

/// Global state for the orchestrator daemon
pub struct OrchestratorState {
    /// Configuration
    pub config: OrchestratorConfig,
    /// Connection pool
    pub connections: Arc<ConnectionPool>,
    /// Session manager
    pub sessions: Arc<SessionManager>,
    /// Tailscale peer verifier
    pub tailscale: Arc<TailscaleVerifier>,
}

impl OrchestratorState {
    /// Create new orchestrator state
    pub fn new(config: OrchestratorConfig) -> Self {
        Self {
            config,
            connections: Arc::new(ConnectionPool::new()),
            sessions: Arc::new(SessionManager::new()),
            tailscale: Arc::new(TailscaleVerifier::new()),
        }
    }

    /// Get the connection pool
    pub fn connection_pool(&self) -> &Arc<ConnectionPool> {
        &self.connections
    }

    /// Get the session manager
    pub fn session_manager(&self) -> &Arc<SessionManager> {
        &self.sessions
    }
}

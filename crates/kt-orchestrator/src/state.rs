//! Global orchestrator state

use std::sync::Arc;

use kt_core::config::OrchestratorConfig;

use crate::auth::AuthorizedKeys;
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
    /// Authorized keys
    pub auth: Arc<AuthorizedKeys>,
}

impl OrchestratorState {
    /// Create new orchestrator state with default auth
    pub fn new(config: OrchestratorConfig) -> Self {
        Self {
            config,
            connections: Arc::new(ConnectionPool::new()),
            sessions: Arc::new(SessionManager::new()),
            auth: Arc::new(AuthorizedKeys::new()),
        }
    }

    /// Create new orchestrator state with provided authorized keys
    pub fn with_auth(config: OrchestratorConfig, auth: AuthorizedKeys) -> Self {
        Self {
            config,
            connections: Arc::new(ConnectionPool::new()),
            sessions: Arc::new(SessionManager::new()),
            auth: Arc::new(auth),
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

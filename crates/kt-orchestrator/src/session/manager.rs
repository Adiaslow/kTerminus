//! Session manager implementation

use dashmap::DashMap;
use std::sync::Arc;

use kt_protocol::SessionId;

/// Manages all active sessions across all connections
pub struct SessionManager {
    /// Sessions indexed by session ID
    sessions: DashMap<SessionId, Arc<SessionHandle>>,
}

/// Handle to an active session
pub struct SessionHandle {
    /// Session ID
    pub id: SessionId,
    // TODO: Add session state
}

impl SessionManager {
    /// Create a new session manager
    pub fn new() -> Self {
        Self {
            sessions: DashMap::new(),
        }
    }

    /// Get a session by ID
    pub fn get(&self, id: SessionId) -> Option<Arc<SessionHandle>> {
        self.sessions.get(&id).map(|r| Arc::clone(&r))
    }

    /// List all sessions
    pub fn list(&self) -> Vec<Arc<SessionHandle>> {
        self.sessions.iter().map(|r| Arc::clone(&r)).collect()
    }

    /// Number of active sessions
    pub fn len(&self) -> usize {
        self.sessions.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.sessions.is_empty()
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        Self::new()
    }
}

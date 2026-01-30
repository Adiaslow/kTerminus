//! Session multiplexing over SSH tunnels

use std::sync::atomic::{AtomicU32, Ordering};

use kt_protocol::SessionId;

/// Multiplexes multiple sessions over a single connection
pub struct SessionMultiplexer {
    /// Next session ID to allocate
    next_session_id: AtomicU32,
}

impl SessionMultiplexer {
    /// Create a new session multiplexer
    pub fn new() -> Self {
        Self {
            next_session_id: AtomicU32::new(1), // Start at 1, 0 is reserved for control
        }
    }

    /// Allocate a new session ID
    pub fn allocate_session_id(&self) -> SessionId {
        SessionId::new(self.next_session_id.fetch_add(1, Ordering::SeqCst))
    }
}

impl Default for SessionMultiplexer {
    fn default() -> Self {
        Self::new()
    }
}

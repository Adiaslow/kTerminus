//! Session identifier type

use serde::{Deserialize, Serialize};
use std::fmt;

/// Unique identifier for a terminal session
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SessionId(pub u32);

impl SessionId {
    /// Create a new session ID
    pub fn new(id: u32) -> Self {
        Self(id)
    }

    /// Get the raw ID value
    pub fn as_u32(&self) -> u32 {
        self.0
    }

    /// Special session ID for control messages (not bound to a session)
    pub const CONTROL: SessionId = SessionId(0);
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "session-{}", self.0)
    }
}

impl From<u32> for SessionId {
    fn from(id: u32) -> Self {
        Self(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_id_display() {
        let id = SessionId::new(42);
        assert_eq!(format!("{}", id), "session-42");
    }

    #[test]
    fn test_session_id_equality() {
        let id1 = SessionId::new(1);
        let id2 = SessionId::new(1);
        let id3 = SessionId::new(2);

        assert_eq!(id1, id2);
        assert_ne!(id1, id3);
    }
}

//! Session manager implementation

use dashmap::DashMap;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{Instant, SystemTime};

use kt_core::types::MachineId;
use kt_protocol::SessionId;

/// Manages all active sessions across all connections
pub struct SessionManager {
    /// Sessions indexed by session ID
    sessions: DashMap<SessionId, Arc<SessionHandle>>,
    /// Next session ID to allocate
    next_session_id: AtomicU32,
}

/// Handle to an active session
pub struct SessionHandle {
    /// Session ID
    pub id: SessionId,
    /// Machine ID this session belongs to
    pub machine_id: MachineId,
    /// Shell command (if specified)
    pub shell: Option<String>,
    /// Process ID on the remote machine (mutable with interior mutability)
    pid: RwLock<Option<u32>>,
    /// When the session was created
    created_at: Instant,
    /// System time when created (for display purposes)
    created_at_system: SystemTime,
}

impl SessionHandle {
    /// Get the process ID
    pub fn pid(&self) -> Option<u32> {
        *self.pid.read().unwrap()
    }

    /// Set the process ID
    pub fn set_pid(&self, pid: u32) {
        *self.pid.write().unwrap() = Some(pid);
    }

    /// Get session uptime
    pub fn uptime(&self) -> std::time::Duration {
        self.created_at.elapsed()
    }

    /// Get creation time as ISO 8601 string
    pub fn created_at_iso(&self) -> String {
        // Convert SystemTime to a simple ISO-ish format
        match self
            .created_at_system
            .duration_since(SystemTime::UNIX_EPOCH)
        {
            Ok(duration) => {
                let secs = duration.as_secs();
                // Simple UTC timestamp format
                format!("{}Z", secs)
            }
            Err(_) => String::new(),
        }
    }
}

impl SessionManager {
    /// Create a new session manager
    pub fn new() -> Self {
        Self {
            sessions: DashMap::new(),
            // Start at 1 since 0 is reserved for CONTROL
            next_session_id: AtomicU32::new(1),
        }
    }

    /// Allocate a new session ID
    pub fn allocate_id(&self) -> SessionId {
        SessionId::new(self.next_session_id.fetch_add(1, Ordering::SeqCst))
    }

    /// Create a new session
    pub fn create(&self, machine_id: MachineId, shell: Option<String>) -> SessionId {
        let id = self.allocate_id();
        let handle = Arc::new(SessionHandle {
            id,
            machine_id,
            shell,
            pid: RwLock::new(None),
            created_at: Instant::now(),
            created_at_system: SystemTime::now(),
        });
        self.sessions.insert(id, handle);
        id
    }

    /// Update session with process ID
    pub fn set_pid(&self, id: SessionId, pid: u32) {
        if let Some(entry) = self.sessions.get(&id) {
            entry.set_pid(pid);
        }
    }

    /// Get a session by ID
    pub fn get(&self, id: SessionId) -> Option<Arc<SessionHandle>> {
        self.sessions.get(&id).map(|r| Arc::clone(&r))
    }

    /// Get machine ID for a session
    pub fn get_machine_id(&self, id: SessionId) -> Option<MachineId> {
        self.sessions.get(&id).map(|r| r.machine_id.clone())
    }

    /// Look up a session by string ID
    pub fn get_by_string_id(&self, id_str: &str) -> Option<Arc<SessionHandle>> {
        // Try to parse "session-N" format
        let id_num = if let Some(stripped) = id_str.strip_prefix("session-") {
            stripped.parse::<u32>().ok()?
        } else {
            // Try direct number parsing
            id_str.parse::<u32>().ok()?
        };
        self.get(SessionId::new(id_num))
    }

    /// Remove a session
    pub fn remove(&self, id: SessionId) -> Option<Arc<SessionHandle>> {
        self.sessions.remove(&id).map(|(_, v)| v)
    }

    /// List all sessions
    pub fn list(&self) -> Vec<Arc<SessionHandle>> {
        self.sessions.iter().map(|r| Arc::clone(&r)).collect()
    }

    /// List sessions for a specific machine
    pub fn list_for_machine(&self, machine_id: &MachineId) -> Vec<Arc<SessionHandle>> {
        self.sessions
            .iter()
            .filter(|r| &r.machine_id == machine_id)
            .map(|r| Arc::clone(&r))
            .collect()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_manager_new() {
        let manager = SessionManager::new();
        assert!(manager.is_empty());
        assert_eq!(manager.len(), 0);
    }

    #[test]
    fn test_session_manager_allocate_id() {
        let manager = SessionManager::new();

        let id1 = manager.allocate_id();
        let id2 = manager.allocate_id();
        let id3 = manager.allocate_id();

        // IDs should start at 1 (0 is reserved for CONTROL)
        assert_eq!(id1.as_u32(), 1);
        assert_eq!(id2.as_u32(), 2);
        assert_eq!(id3.as_u32(), 3);
    }

    #[test]
    fn test_session_manager_create() {
        let manager = SessionManager::new();
        let machine_id = MachineId::new("test-machine");

        let session_id = manager.create(machine_id.clone(), Some("/bin/bash".to_string()));

        assert_eq!(manager.len(), 1);
        assert!(!manager.is_empty());

        let session = manager.get(session_id).expect("Session should exist");
        assert_eq!(session.id, session_id);
        assert_eq!(session.machine_id.as_str(), "test-machine");
        assert_eq!(session.shell.as_deref(), Some("/bin/bash"));
    }

    #[test]
    fn test_session_manager_create_multiple() {
        let manager = SessionManager::new();
        let machine_id = MachineId::new("test-machine");

        let id1 = manager.create(machine_id.clone(), None);
        let id2 = manager.create(machine_id.clone(), Some("/bin/zsh".to_string()));
        let id3 = manager.create(MachineId::new("other-machine"), None);

        assert_eq!(manager.len(), 3);
        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
    }

    #[test]
    fn test_session_manager_get() {
        let manager = SessionManager::new();
        let session_id = manager.create(MachineId::new("test"), None);

        let session = manager.get(session_id);
        assert!(session.is_some());

        let nonexistent = manager.get(SessionId::new(999));
        assert!(nonexistent.is_none());
    }

    #[test]
    fn test_session_manager_get_machine_id() {
        let manager = SessionManager::new();
        let machine_id = MachineId::new("test-machine");
        let session_id = manager.create(machine_id.clone(), None);

        let retrieved_machine_id = manager.get_machine_id(session_id);
        assert_eq!(retrieved_machine_id, Some(machine_id));

        let nonexistent = manager.get_machine_id(SessionId::new(999));
        assert!(nonexistent.is_none());
    }

    #[test]
    fn test_session_manager_remove() {
        let manager = SessionManager::new();
        let id1 = manager.create(MachineId::new("machine-1"), None);
        let id2 = manager.create(MachineId::new("machine-2"), None);

        assert_eq!(manager.len(), 2);

        let removed = manager.remove(id1);
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().id, id1);

        assert_eq!(manager.len(), 1);
        assert!(manager.get(id1).is_none());
        assert!(manager.get(id2).is_some());
    }

    #[test]
    fn test_session_manager_remove_nonexistent() {
        let manager = SessionManager::new();
        let removed = manager.remove(SessionId::new(999));
        assert!(removed.is_none());
    }

    #[test]
    fn test_session_manager_list() {
        let manager = SessionManager::new();
        manager.create(MachineId::new("machine-1"), None);
        manager.create(MachineId::new("machine-2"), None);
        manager.create(MachineId::new("machine-3"), None);

        let sessions = manager.list();
        assert_eq!(sessions.len(), 3);
    }

    #[test]
    fn test_session_manager_list_for_machine() {
        let manager = SessionManager::new();
        let machine_a = MachineId::new("machine-a");
        let machine_b = MachineId::new("machine-b");

        manager.create(machine_a.clone(), None);
        manager.create(machine_a.clone(), None);
        manager.create(machine_b.clone(), None);

        let sessions_a = manager.list_for_machine(&machine_a);
        assert_eq!(sessions_a.len(), 2);

        let sessions_b = manager.list_for_machine(&machine_b);
        assert_eq!(sessions_b.len(), 1);

        let sessions_c = manager.list_for_machine(&MachineId::new("machine-c"));
        assert_eq!(sessions_c.len(), 0);
    }

    #[test]
    fn test_session_manager_set_pid() {
        let manager = SessionManager::new();
        let session_id = manager.create(MachineId::new("test"), None);

        // Initially no PID
        let session = manager.get(session_id).unwrap();
        assert!(session.pid().is_none());

        // Set PID
        manager.set_pid(session_id, 12345);

        let session = manager.get(session_id).unwrap();
        assert_eq!(session.pid(), Some(12345));
    }

    #[test]
    fn test_session_manager_get_by_string_id() {
        let manager = SessionManager::new();
        let session_id = manager.create(MachineId::new("test"), None);

        // Test "session-N" format
        let found = manager.get_by_string_id(&format!("session-{}", session_id.as_u32()));
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, session_id);

        // Test direct number format
        let found = manager.get_by_string_id(&session_id.as_u32().to_string());
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, session_id);

        // Test nonexistent
        let not_found = manager.get_by_string_id("session-999");
        assert!(not_found.is_none());

        // Test invalid format
        let invalid = manager.get_by_string_id("invalid");
        assert!(invalid.is_none());
    }

    #[test]
    fn test_session_handle_uptime() {
        let manager = SessionManager::new();
        let session_id = manager.create(MachineId::new("test"), None);

        let session = manager.get(session_id).unwrap();
        let uptime = session.uptime();

        // Uptime should be very small (just created)
        assert!(uptime.as_millis() < 1000);
    }

    #[test]
    fn test_session_handle_created_at_iso() {
        let manager = SessionManager::new();
        let session_id = manager.create(MachineId::new("test"), None);

        let session = manager.get(session_id).unwrap();
        let created_at = session.created_at_iso();

        // Should end with 'Z' and contain a timestamp
        assert!(created_at.ends_with('Z'));
        assert!(!created_at.is_empty());
    }
}

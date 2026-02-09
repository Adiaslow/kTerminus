//! Session manager implementation
//!
//! The session manager is responsible for tracking all active terminal sessions
//! across all connected machines. It provides:
//!
//! - **Session creation**: Allocating unique session IDs and tracking metadata
//! - **Session ownership**: Each session is bound to a specific machine (MachineId)
//! - **Session lookup**: Finding sessions by ID, string ID, or machine
//! - **Session cleanup**: Removing sessions individually or by machine (for disconnect handling)
//!
//! # Session Ownership Model
//!
//! Sessions are bound to machines at creation time via the `machine_id` field.
//! This ownership is immutable and used to:
//!
//! - Route terminal I/O to the correct agent
//! - Verify session operations come from the owning machine
//! - Clean up all sessions when a machine disconnects (via `remove_by_machine`)
//!
//! # Thread Safety
//!
//! The session manager uses `DashMap` for concurrent access, allowing multiple
//! tasks to read/write sessions without explicit locking.

use dashmap::DashMap;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Instant, SystemTime};

use kt_core::types::MachineId;
use kt_protocol::SessionId;

/// Session state machine states.
///
/// Sessions progress through these states during their lifecycle:
/// - `Creating`: Initial state while waiting for agent confirmation
/// - `Active`: Normal operation, session is usable
/// - `Orphaned`: Client disconnected, session in grace period (can be reclaimed)
/// - `Closing`: Terminal state, session is being cleaned up
///
/// State transitions:
/// ```text
/// Creating ──► Active ◄──► Orphaned
///     │          │            │
///     └──────────┴────────────┴──► Closing (terminal)
/// ```
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Waiting for agent confirmation
    Creating = 0,
    /// Normal operation
    Active = 1,
    /// Client disconnected, in grace period
    Orphaned = 2,
    /// Terminal state, being cleaned up
    Closing = 3,
}

impl SessionState {
    /// Convert from u8 representation
    fn from_u8(value: u8) -> Option<Self> {
        match value {
            0 => Some(SessionState::Creating),
            1 => Some(SessionState::Active),
            2 => Some(SessionState::Orphaned),
            3 => Some(SessionState::Closing),
            _ => None,
        }
    }
}

// Packing format for state: AtomicU64
// - Low 8 bits: SessionState (0-3)
// - High 56 bits: orphaned_at timestamp / 256 (milliseconds, ~8 million years range)
//
// This allows atomic CAS operations on both state and timestamp together.
const STATE_MASK: u64 = 0xFF;
const TIMESTAMP_SHIFT: u32 = 8;

/// Pack state and timestamp into a single u64
fn pack_state(state: SessionState, orphaned_at_millis: u64) -> u64 {
    let state_bits = state as u64;
    let timestamp_bits = (orphaned_at_millis / 256) << TIMESTAMP_SHIFT;
    state_bits | timestamp_bits
}

/// Unpack state from packed u64
fn unpack_state(packed: u64) -> SessionState {
    SessionState::from_u8((packed & STATE_MASK) as u8).unwrap_or(SessionState::Creating)
}

/// Unpack orphaned_at timestamp from packed u64 (returns 0 if not orphaned)
fn unpack_orphaned_at(packed: u64) -> u64 {
    let state = unpack_state(packed);
    if state == SessionState::Orphaned {
        (packed >> TIMESTAMP_SHIFT) * 256
    } else {
        0
    }
}

/// Error returned when session limit is exceeded for a machine
#[derive(Debug, Clone)]
pub struct SessionLimitExceeded {
    /// Machine that exceeded the limit
    pub machine_id: MachineId,
    /// Current number of sessions for this machine
    pub current: usize,
    /// Maximum allowed sessions per machine
    pub max: usize,
}

impl std::fmt::Display for SessionLimitExceeded {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Session limit exceeded for machine {}: {} sessions (max {})",
            self.machine_id, self.current, self.max
        )
    }
}

impl std::error::Error for SessionLimitExceeded {}

/// Manages all active sessions across all connections.
///
/// The session manager provides thread-safe session tracking with ownership
/// enforcement. Sessions are identified by unique `SessionId` values and
/// are bound to specific machines via `MachineId`.
///
/// # Example
///
/// ```ignore
/// let manager = SessionManager::new();
///
/// // Create a session for a specific machine
/// let session_id = manager.create(machine_id.clone(), Some("/bin/bash".to_string()));
///
/// // Later, clean up all sessions for a disconnected machine
/// let removed = manager.remove_by_machine(&machine_id);
/// ```
pub struct SessionManager {
    /// Sessions indexed by session ID.
    /// Using DashMap for concurrent access without external locking.
    sessions: DashMap<SessionId, Arc<SessionHandle>>,
    /// Next session ID to allocate (starts at 1, 0 is reserved for CONTROL channel)
    next_session_id: AtomicU32,
}

/// Handle to an active session.
///
/// Each session represents a terminal session (PTY) running on a remote machine.
/// Sessions are immutably bound to their owning machine at creation time.
///
/// # Ownership
///
/// The `machine_id` field identifies which machine owns this session. This is
/// used for:
/// - Routing terminal I/O to the correct agent
/// - Verifying that session operations come from the owning machine
/// - Cleaning up sessions when a machine disconnects
///
/// The optional `owner_client_id` tracks which IPC client created the session,
/// enabling fine-grained access control for session operations.
///
/// # State Machine
///
/// Sessions have an explicit state machine with atomic CAS transitions:
/// - `Creating` -> `Active`: Agent confirmed session creation
/// - `Active` -> `Orphaned`: Client disconnected, session in grace period
/// - `Orphaned` -> `Active`: Client reconnected and reclaimed session
/// - Any -> `Closing`: Session is being cleaned up (terminal state)
///
/// # Thread Safety
///
/// The `pid` field uses `AtomicU32` and `state` uses `AtomicU64` to allow
/// lock-free updates from multiple tasks without risk of lock poisoning.
pub struct SessionHandle {
    /// Session ID - unique identifier for this session
    pub id: SessionId,
    /// Machine ID this session belongs to (immutable after creation).
    /// Used for routing I/O and cleanup on disconnect.
    pub machine_id: MachineId,
    /// Shell command (if specified, otherwise uses default shell)
    pub shell: Option<String>,
    /// Process ID on the remote machine (0 means not set yet).
    /// Uses AtomicU32 to avoid RwLock poisoning panics.
    pid: AtomicU32,
    /// When the session was created (monotonic, for uptime calculation)
    created_at: Instant,
    /// System time when created (for display purposes)
    created_at_system: SystemTime,
    /// Client ID that owns this session (for access control).
    /// None means the session was created internally (e.g., by the orchestrator).
    pub owner_client_id: Option<String>,
    /// Packed session state and orphaned_at timestamp.
    /// Low 8 bits: SessionState enum value
    /// High 56 bits: orphaned_at timestamp / 256 (only valid when state is Orphaned)
    state: AtomicU64,
}

impl SessionHandle {
    /// Get the process ID (returns None if PID is 0, meaning not yet set)
    pub fn pid(&self) -> Option<u32> {
        match self.pid.load(Ordering::SeqCst) {
            0 => None,
            pid => Some(pid),
        }
    }

    /// Set the process ID
    pub fn set_pid(&self, pid: u32) {
        self.pid.store(pid, Ordering::SeqCst);
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

    // ========== State Machine Methods ==========

    /// Get the current session state.
    pub fn state(&self) -> SessionState {
        let packed = self.state.load(Ordering::SeqCst);
        unpack_state(packed)
    }

    /// Attempt to transition from Active to Orphaned state.
    ///
    /// Returns `true` if the transition succeeded, `false` if the session
    /// was not in Active state (already orphaned, closing, or creating).
    ///
    /// This is a CAS operation that ensures only one caller can successfully
    /// orphan the session.
    pub fn try_orphan(&self, time_millis: u64) -> bool {
        let current = self.state.load(Ordering::SeqCst);
        if unpack_state(current) != SessionState::Active {
            return false;
        }

        let new_packed = pack_state(SessionState::Orphaned, time_millis);
        self.state
            .compare_exchange(current, new_packed, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    /// Attempt to transition from Orphaned to Active state (reclaim session).
    ///
    /// Returns `true` if the transition succeeded, `false` if the session
    /// was not in Orphaned state (not orphaned, closing, or creating).
    ///
    /// This is a CAS operation that ensures only one caller can successfully
    /// reclaim the session.
    pub fn try_reclaim(&self) -> bool {
        let current = self.state.load(Ordering::SeqCst);
        if unpack_state(current) != SessionState::Orphaned {
            return false;
        }

        let new_packed = pack_state(SessionState::Active, 0);
        self.state
            .compare_exchange(current, new_packed, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    /// Attempt to transition to Closing state from any state.
    ///
    /// Returns `true` if the transition succeeded or if already in Closing state
    /// (idempotent). Returns `false` only on CAS contention (retry needed).
    ///
    /// This is a CAS operation that ensures only one cleanup path wins.
    /// Once in Closing state, the session cannot transition to any other state.
    pub fn try_close(&self) -> bool {
        loop {
            let current = self.state.load(Ordering::SeqCst);
            if unpack_state(current) == SessionState::Closing {
                // Already closing, success (idempotent)
                return true;
            }

            let new_packed = pack_state(SessionState::Closing, 0);
            match self
                .state
                .compare_exchange(current, new_packed, Ordering::SeqCst, Ordering::SeqCst)
            {
                Ok(_) => return true,
                Err(_) => {
                    // CAS failed, retry (state changed between load and CAS)
                    continue;
                }
            }
        }
    }

    /// Attempt to transition from Creating to Active state.
    ///
    /// Returns `true` if the transition succeeded, `false` if the session
    /// was not in Creating state.
    pub fn try_activate(&self) -> bool {
        let current = self.state.load(Ordering::SeqCst);
        if unpack_state(current) != SessionState::Creating {
            return false;
        }

        let new_packed = pack_state(SessionState::Active, 0);
        self.state
            .compare_exchange(current, new_packed, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
    }

    // ========== Legacy Compatibility Methods ==========
    // These methods delegate to the state machine for backward compatibility.

    /// Mark the session as orphaned (client disconnected).
    /// The session will be cleaned up after the grace period expires.
    ///
    /// Note: This delegates to `try_orphan` but does not return the success status.
    /// Prefer using `try_orphan` directly for new code.
    pub fn set_orphaned_at(&self, time_millis: u64) {
        // For backward compatibility, we force the orphaned state
        // This is less safe than try_orphan but maintains the old API contract
        let new_packed = pack_state(SessionState::Orphaned, time_millis);
        self.state.store(new_packed, Ordering::SeqCst);
    }

    /// Clear the orphaned status (client reconnected).
    ///
    /// Note: This delegates to `try_reclaim` but does not return the success status.
    /// Prefer using `try_reclaim` directly for new code.
    pub fn clear_orphaned(&self) {
        // For backward compatibility, we force the active state
        // This is less safe than try_reclaim but maintains the old API contract
        let new_packed = pack_state(SessionState::Active, 0);
        self.state.store(new_packed, Ordering::SeqCst);
    }

    /// Get the time when this session was orphaned, if any.
    /// Returns None if the session is not in the Orphaned state.
    pub fn orphaned_at(&self) -> Option<u64> {
        let packed = self.state.load(Ordering::SeqCst);
        match unpack_orphaned_at(packed) {
            0 => None,
            t => Some(t),
        }
    }

    /// Check if this session is orphaned.
    pub fn is_orphaned(&self) -> bool {
        self.state() == SessionState::Orphaned
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
        self.create_with_owner(machine_id, shell, None)
    }

    /// Create a new session with an owner client ID for access control
    pub fn create_with_owner(
        &self,
        machine_id: MachineId,
        shell: Option<String>,
        owner_client_id: Option<String>,
    ) -> SessionId {
        let id = self.allocate_id();
        let handle = Arc::new(SessionHandle {
            id,
            machine_id,
            shell,
            pid: AtomicU32::new(0), // 0 indicates PID not yet set
            created_at: Instant::now(),
            created_at_system: SystemTime::now(),
            owner_client_id,
            // Start in Active state (state=1, timestamp=0)
            // Note: Could start in Creating state if we want to wait for agent confirmation
            state: AtomicU64::new(pack_state(SessionState::Active, 0)),
        });
        self.sessions.insert(id, handle);
        id
    }

    /// Try to create a new session, checking against a per-machine session limit.
    ///
    /// Returns `Ok(SessionId)` if the session was created, or `Err(SessionLimitExceeded)`
    /// if the machine already has the maximum number of sessions.
    ///
    /// # Arguments
    /// * `machine_id` - The machine this session belongs to
    /// * `shell` - Optional shell command to spawn
    /// * `owner_client_id` - Optional IPC client that owns this session
    /// * `max_sessions_per_machine` - Optional maximum sessions per machine. If `None`, no limit.
    pub fn try_create_with_owner(
        &self,
        machine_id: MachineId,
        shell: Option<String>,
        owner_client_id: Option<String>,
        max_sessions_per_machine: Option<u32>,
    ) -> Result<SessionId, SessionLimitExceeded> {
        // Check limit if configured
        if let Some(max) = max_sessions_per_machine {
            let current = self.list_for_machine(&machine_id).len();
            if current >= max as usize {
                return Err(SessionLimitExceeded {
                    machine_id,
                    current,
                    max: max as usize,
                });
            }
        }

        Ok(self.create_with_owner(machine_id, shell, owner_client_id))
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

    /// List sessions for a specific machine.
    ///
    /// Returns all sessions owned by the given machine. Useful for:
    /// - Displaying sessions per machine in the UI
    /// - Checking session limits before creating new sessions
    /// - Getting sessions to clean up when a machine disconnects
    pub fn list_for_machine(&self, machine_id: &MachineId) -> Vec<Arc<SessionHandle>> {
        self.sessions
            .iter()
            .filter(|r| &r.machine_id == machine_id)
            .map(|r| Arc::clone(&r))
            .collect()
    }

    /// Remove all sessions for a specific machine.
    ///
    /// This is called when an agent disconnects (intentionally or due to network
    /// failure) to clean up all sessions that were running on that machine.
    ///
    /// # Returns
    ///
    /// A vector of the removed session handles. This allows the caller to:
    /// - Log which sessions were cleaned up
    /// - Notify IPC clients that their sessions were terminated
    /// - Perform any additional cleanup (e.g., close PTY handles)
    ///
    /// # Example
    ///
    /// ```ignore
    /// // When an agent disconnects
    /// let removed_sessions = manager.remove_by_machine(&disconnected_machine_id);
    /// for session in removed_sessions {
    ///     tracing::info!("Cleaned up session {} on disconnect", session.id);
    ///     // Notify subscribed clients that the session was terminated
    /// }
    /// ```
    pub fn remove_by_machine(&self, machine_id: &MachineId) -> Vec<Arc<SessionHandle>> {
        // Collect session IDs to remove
        let ids_to_remove: Vec<SessionId> = self
            .sessions
            .iter()
            .filter(|r| &r.machine_id == machine_id)
            .map(|r| r.id)
            .collect();

        // Remove each session and collect the handles
        ids_to_remove
            .into_iter()
            .filter_map(|id| self.remove(id))
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

    #[test]
    fn test_session_manager_remove_by_machine() {
        let manager = SessionManager::new();
        let machine_a = MachineId::new("machine-a");
        let machine_b = MachineId::new("machine-b");

        // Create sessions for both machines
        let a1 = manager.create(machine_a.clone(), None);
        let a2 = manager.create(machine_a.clone(), None);
        let a3 = manager.create(machine_a.clone(), None);
        let b1 = manager.create(machine_b.clone(), None);

        assert_eq!(manager.len(), 4);

        // Remove all sessions for machine_a
        let removed = manager.remove_by_machine(&machine_a);

        // Should have removed 3 sessions
        assert_eq!(removed.len(), 3);

        // Verify the removed sessions were for machine_a
        for session in &removed {
            assert_eq!(session.machine_id, machine_a);
        }

        // Verify the removed session IDs
        let removed_ids: Vec<_> = removed.iter().map(|s| s.id).collect();
        assert!(removed_ids.contains(&a1));
        assert!(removed_ids.contains(&a2));
        assert!(removed_ids.contains(&a3));

        // Only machine_b's session should remain
        assert_eq!(manager.len(), 1);
        assert!(manager.get(b1).is_some());
        assert!(manager.get(a1).is_none());
    }

    #[test]
    fn test_session_manager_remove_by_machine_empty() {
        let manager = SessionManager::new();
        let machine_a = MachineId::new("machine-a");

        // Remove sessions for a machine with no sessions
        let removed = manager.remove_by_machine(&machine_a);

        // Should return empty vec
        assert!(removed.is_empty());
    }

    #[test]
    fn test_session_manager_create_with_owner() {
        let manager = SessionManager::new();
        let machine_id = MachineId::new("test-machine");
        let client_id = "client-123";

        let session_id = manager.create_with_owner(
            machine_id.clone(),
            Some("/bin/bash".to_string()),
            Some(client_id.to_string()),
        );

        let session = manager.get(session_id).expect("Session should exist");
        assert_eq!(session.owner_client_id.as_deref(), Some(client_id));
    }

    // ========== State Machine Tests ==========

    #[test]
    fn test_session_initial_state_is_active() {
        let manager = SessionManager::new();
        let session_id = manager.create(MachineId::new("test"), None);
        let session = manager.get(session_id).unwrap();

        assert_eq!(session.state(), SessionState::Active);
        assert!(!session.is_orphaned());
        assert!(session.orphaned_at().is_none());
    }

    #[test]
    fn test_session_try_orphan_from_active() {
        let manager = SessionManager::new();
        let session_id = manager.create(MachineId::new("test"), None);
        let session = manager.get(session_id).unwrap();

        let time_millis = 1234567890_u64;
        assert!(session.try_orphan(time_millis));

        assert_eq!(session.state(), SessionState::Orphaned);
        assert!(session.is_orphaned());
        // Timestamp is stored / 256, so we lose some precision
        let stored = session.orphaned_at().unwrap();
        assert!(stored <= time_millis);
        assert!(stored >= time_millis - 256);
    }

    #[test]
    fn test_session_try_orphan_from_orphaned_fails() {
        let manager = SessionManager::new();
        let session_id = manager.create(MachineId::new("test"), None);
        let session = manager.get(session_id).unwrap();

        // First orphan succeeds
        assert!(session.try_orphan(1000));
        assert_eq!(session.state(), SessionState::Orphaned);

        // Second orphan fails (already orphaned)
        assert!(!session.try_orphan(2000));
        assert_eq!(session.state(), SessionState::Orphaned);
    }

    #[test]
    fn test_session_try_reclaim_from_orphaned() {
        let manager = SessionManager::new();
        let session_id = manager.create(MachineId::new("test"), None);
        let session = manager.get(session_id).unwrap();

        session.try_orphan(1000);
        assert_eq!(session.state(), SessionState::Orphaned);

        assert!(session.try_reclaim());
        assert_eq!(session.state(), SessionState::Active);
        assert!(!session.is_orphaned());
        assert!(session.orphaned_at().is_none());
    }

    #[test]
    fn test_session_try_reclaim_from_active_fails() {
        let manager = SessionManager::new();
        let session_id = manager.create(MachineId::new("test"), None);
        let session = manager.get(session_id).unwrap();

        // Session starts Active, reclaim should fail
        assert!(!session.try_reclaim());
        assert_eq!(session.state(), SessionState::Active);
    }

    #[test]
    fn test_session_try_close_from_active() {
        let manager = SessionManager::new();
        let session_id = manager.create(MachineId::new("test"), None);
        let session = manager.get(session_id).unwrap();

        assert!(session.try_close());
        assert_eq!(session.state(), SessionState::Closing);
    }

    #[test]
    fn test_session_try_close_from_orphaned() {
        let manager = SessionManager::new();
        let session_id = manager.create(MachineId::new("test"), None);
        let session = manager.get(session_id).unwrap();

        session.try_orphan(1000);
        assert_eq!(session.state(), SessionState::Orphaned);

        assert!(session.try_close());
        assert_eq!(session.state(), SessionState::Closing);
    }

    #[test]
    fn test_session_try_close_idempotent() {
        let manager = SessionManager::new();
        let session_id = manager.create(MachineId::new("test"), None);
        let session = manager.get(session_id).unwrap();

        // First close
        assert!(session.try_close());
        assert_eq!(session.state(), SessionState::Closing);

        // Second close should also return true (idempotent)
        assert!(session.try_close());
        assert_eq!(session.state(), SessionState::Closing);
    }

    #[test]
    fn test_session_no_transition_from_closing() {
        let manager = SessionManager::new();
        let session_id = manager.create(MachineId::new("test"), None);
        let session = manager.get(session_id).unwrap();

        session.try_close();
        assert_eq!(session.state(), SessionState::Closing);

        // Cannot orphan from Closing
        assert!(!session.try_orphan(1000));
        assert_eq!(session.state(), SessionState::Closing);

        // Cannot reclaim from Closing
        assert!(!session.try_reclaim());
        assert_eq!(session.state(), SessionState::Closing);
    }

    #[test]
    fn test_session_legacy_set_orphaned_at() {
        let manager = SessionManager::new();
        let session_id = manager.create(MachineId::new("test"), None);
        let session = manager.get(session_id).unwrap();

        // Legacy method should force orphaned state
        session.set_orphaned_at(5000);
        assert_eq!(session.state(), SessionState::Orphaned);
        assert!(session.is_orphaned());
    }

    #[test]
    fn test_session_legacy_clear_orphaned() {
        let manager = SessionManager::new();
        let session_id = manager.create(MachineId::new("test"), None);
        let session = manager.get(session_id).unwrap();

        session.set_orphaned_at(5000);
        assert!(session.is_orphaned());

        session.clear_orphaned();
        assert!(!session.is_orphaned());
        assert_eq!(session.state(), SessionState::Active);
    }

    #[test]
    fn test_session_state_enum_values() {
        // Verify repr(u8) values
        assert_eq!(SessionState::Creating as u8, 0);
        assert_eq!(SessionState::Active as u8, 1);
        assert_eq!(SessionState::Orphaned as u8, 2);
        assert_eq!(SessionState::Closing as u8, 3);
    }

    #[test]
    fn test_session_state_from_u8() {
        assert_eq!(SessionState::from_u8(0), Some(SessionState::Creating));
        assert_eq!(SessionState::from_u8(1), Some(SessionState::Active));
        assert_eq!(SessionState::from_u8(2), Some(SessionState::Orphaned));
        assert_eq!(SessionState::from_u8(3), Some(SessionState::Closing));
        assert_eq!(SessionState::from_u8(4), None);
        assert_eq!(SessionState::from_u8(255), None);
    }

    #[test]
    fn test_pack_unpack_state() {
        // Test packing and unpacking for all states
        for state in [
            SessionState::Creating,
            SessionState::Active,
            SessionState::Orphaned,
            SessionState::Closing,
        ] {
            let packed = pack_state(state, 0);
            assert_eq!(unpack_state(packed), state);
        }
    }

    #[test]
    fn test_pack_unpack_timestamp() {
        // Test timestamp is preserved when orphaned
        let time_millis = 1_700_000_000_000_u64; // ~2023 timestamp in millis
        let packed = pack_state(SessionState::Orphaned, time_millis);

        assert_eq!(unpack_state(packed), SessionState::Orphaned);

        // Timestamp loses some precision due to /256 packing
        let unpacked_time = unpack_orphaned_at(packed);
        assert!(unpacked_time <= time_millis);
        assert!(unpacked_time >= time_millis - 256);
    }

    #[test]
    fn test_timestamp_only_for_orphaned_state() {
        // Timestamp should only be returned for Orphaned state
        let time_millis = 1_000_000_u64;

        let packed_active = pack_state(SessionState::Active, time_millis);
        assert_eq!(unpack_orphaned_at(packed_active), 0);

        let packed_creating = pack_state(SessionState::Creating, time_millis);
        assert_eq!(unpack_orphaned_at(packed_creating), 0);

        let packed_closing = pack_state(SessionState::Closing, time_millis);
        assert_eq!(unpack_orphaned_at(packed_closing), 0);

        // Only Orphaned state returns the timestamp
        let packed_orphaned = pack_state(SessionState::Orphaned, time_millis);
        assert!(unpack_orphaned_at(packed_orphaned) > 0);
    }
}

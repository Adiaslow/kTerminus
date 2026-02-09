//! Orphan session cleanup task
//!
//! This module provides a background task that periodically cleans up
//! orphaned sessions whose grace period has expired.
//!
//! # Grace Period
//!
//! When an IPC client disconnects, its sessions are marked as "orphaned"
//! rather than immediately deleted. This gives the client a chance to
//! reconnect and reclaim the sessions within a grace period.
//!
//! After the grace period expires, the sessions are cleaned up:
//! - A close command is sent to the agent to terminate the PTY
//! - The session is removed from the session manager

use std::sync::Arc;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use crate::connection::AgentCommand;
use crate::state::OrchestratorState;

/// Grace period before orphaned sessions are cleaned up.
/// Sessions that remain orphaned after this period will be terminated.
pub const ORPHAN_GRACE_PERIOD: Duration = Duration::from_secs(30);

/// Interval between cleanup checks.
const CLEANUP_INTERVAL: Duration = Duration::from_secs(10);

/// Get current time in milliseconds since UNIX epoch.
fn current_time_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// Run the orphan cleanup task.
///
/// This task periodically checks for orphaned sessions whose grace period
/// has expired and cleans them up.
///
/// # Arguments
///
/// * `state` - The orchestrator state containing the session manager
/// * `cancel` - Cancellation token for graceful shutdown
pub async fn run_orphan_cleanup(state: Arc<OrchestratorState>, cancel: CancellationToken) {
    let mut interval = tokio::time::interval(CLEANUP_INTERVAL);

    tracing::info!(
        "Starting orphan cleanup task (grace period: {:?}, check interval: {:?})",
        ORPHAN_GRACE_PERIOD,
        CLEANUP_INTERVAL
    );

    loop {
        tokio::select! {
            _ = interval.tick() => {
                cleanup_expired_orphans(&state, ORPHAN_GRACE_PERIOD);
            }
            _ = cancel.cancelled() => {
                tracing::info!("Orphan cleanup task shutting down");
                break;
            }
        }
    }
}

/// Clean up orphaned sessions whose grace period has expired.
fn cleanup_expired_orphans(state: &OrchestratorState, grace_period: Duration) {
    let now = current_time_millis();
    let cutoff = now.saturating_sub(grace_period.as_millis() as u64);
    let mut cleaned_count = 0;

    // Use coordinator.sessions for proper state management
    for session in state.coordinator.sessions.list() {
        if let Some(orphaned_at) = session.orphaned_at() {
            if orphaned_at < cutoff {
                // Use try_close() CAS to ensure only one cleanup path wins
                // This prevents races between cleanup, disconnect, and health monitor
                if session.try_close() {
                    // Grace period expired, clean up this session
                    tracing::info!(
                        "Cleaning up orphaned session {} (orphaned {}ms ago, owner: {:?})",
                        session.id,
                        now.saturating_sub(orphaned_at),
                        session.owner_client_id
                    );

                    // Send close command to agent if machine is still connected
                    if let Some(conn) = state.coordinator.connections.get(&session.machine_id) {
                        let command = AgentCommand::CloseSession {
                            session_id: session.id,
                        };
                        // Best effort - don't block on sending
                        if conn.command_tx.try_send(command).is_err() {
                            tracing::warn!(
                                "Failed to send close command for expired orphan session {}",
                                session.id
                            );
                        }
                    }

                    // Remove from session manager
                    state.coordinator.sessions.remove(session.id);
                    cleaned_count += 1;
                }
                // If try_close() returns false, another cleanup path already claimed this session
            }
        }
    }

    if cleaned_count > 0 {
        tracing::info!("Cleaned up {} expired orphan sessions", cleaned_count);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grace_period_is_reasonable() {
        // Grace period should be at least 10 seconds
        assert!(ORPHAN_GRACE_PERIOD >= Duration::from_secs(10));
        // Grace period should be at most 5 minutes
        assert!(ORPHAN_GRACE_PERIOD <= Duration::from_secs(300));
    }

    #[test]
    fn test_cleanup_interval_is_reasonable() {
        // Cleanup interval should be at least 5 seconds
        assert!(CLEANUP_INTERVAL >= Duration::from_secs(5));
        // Cleanup interval should be less than grace period
        assert!(CLEANUP_INTERVAL < ORPHAN_GRACE_PERIOD);
    }
}

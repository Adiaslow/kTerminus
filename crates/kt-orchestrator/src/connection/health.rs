//! Connection health monitoring

use std::sync::Arc;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

use kt_core::time::current_time_millis;

use super::pool::AgentCommand;
use crate::state::OrchestratorState;

/// Monitors connection health via heartbeats
pub struct HealthMonitor {
    /// Heartbeat interval
    pub interval: Duration,
    /// Heartbeat timeout
    pub timeout: Duration,
}

impl HealthMonitor {
    /// Create a new health monitor
    pub fn new(interval: Duration, timeout: Duration) -> Self {
        Self { interval, timeout }
    }

    /// Start the health monitoring task
    ///
    /// This spawns a background task that:
    /// - Sends periodic heartbeats to all connected agents
    /// - Checks for agents that haven't responded within the timeout
    /// - Disconnects unresponsive agents
    pub fn spawn(
        &self,
        state: Arc<OrchestratorState>,
        cancel: CancellationToken,
    ) -> tokio::task::JoinHandle<()> {
        let interval = self.interval;
        let timeout = self.timeout;

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);

            loop {
                tokio::select! {
                    _ = ticker.tick() => {
                        // Get current timestamp
                        let timestamp = current_time_millis();

                        // Send heartbeat to all connections and check health
                        // Use coordinator.connections for proper state management
                        let connections = state.coordinator.connections.list();
                        for conn in connections {
                            // Check if connection is healthy
                            if !conn.is_healthy(timeout) {
                                tracing::warn!(
                                    "Connection {} is unhealthy (no heartbeat for {:?}), disconnecting",
                                    conn.machine_id,
                                    timeout
                                );

                                // Clean up all sessions for this machine BEFORE removing the connection
                                // to prevent orphaned sessions (atomic cleanup)
                                // Use coordinator.sessions for proper state management
                                let sessions = state.coordinator.sessions.list_for_machine(&conn.machine_id);
                                for session in sessions {
                                    // Use try_close() CAS to ensure only one cleanup path wins
                                    // This prevents races between health monitor, cleanup task, and disconnect handler
                                    if session.try_close() {
                                        tracing::debug!(
                                            "Cleaning up orphaned session {} for unhealthy connection {}",
                                            session.id,
                                            conn.machine_id
                                        );
                                        state.coordinator.sessions.remove(session.id);
                                    }
                                    // If try_close() returns false, another cleanup path already claimed this session
                                }

                                conn.disconnect();
                                state.coordinator.connections.remove(&conn.machine_id);
                                continue;
                            }

                            // Send heartbeat using try_send to avoid blocking the health monitor.
                            // If the channel is full (backpressure), the heartbeat is dropped
                            // and logged. This indicates the agent is not processing commands
                            // fast enough, which will eventually trigger an unhealthy state
                            // when heartbeat acks stop arriving.
                            let command = AgentCommand::Heartbeat { timestamp };
                            if let Err(e) = conn.command_tx.try_send(command) {
                                tracing::warn!(
                                    "Command channel backpressure for {}: failed to send heartbeat ({}). \
                                     Agent may not be processing commands fast enough.",
                                    conn.machine_id,
                                    e
                                );
                            }
                        }
                    }
                    _ = cancel.cancelled() => {
                        tracing::debug!("Health monitor shutting down");
                        break;
                    }
                }
            }
        })
    }
}

//! Connection health monitoring

use std::sync::Arc;
use std::time::Duration;

use tokio_util::sync::CancellationToken;

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
                        let timestamp = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap()
                            .as_millis() as u64;

                        // Send heartbeat to all connections and check health
                        let connections = state.connections.list();
                        for conn in connections {
                            // Check if connection is healthy
                            if !conn.is_healthy(timeout) {
                                tracing::warn!(
                                    "Connection {} is unhealthy (no heartbeat for {:?}), disconnecting",
                                    conn.machine_id,
                                    timeout
                                );
                                conn.disconnect();
                                state.connections.remove(&conn.machine_id);
                                continue;
                            }

                            // Send heartbeat
                            let command = AgentCommand::Heartbeat { timestamp };
                            if let Err(e) = conn.command_tx.try_send(command) {
                                tracing::warn!(
                                    "Failed to send heartbeat to {}: {}",
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

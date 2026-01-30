//! Connection health monitoring

use std::time::Duration;
use tokio_util::sync::CancellationToken;

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

    /// Start monitoring a connection
    pub fn spawn_monitor(&self, _cancel: CancellationToken) -> tokio::task::JoinHandle<()> {
        let interval = self.interval;
        let _timeout = self.timeout;

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            loop {
                ticker.tick().await;
                // TODO: Send heartbeat and check responses
            }
        })
    }
}

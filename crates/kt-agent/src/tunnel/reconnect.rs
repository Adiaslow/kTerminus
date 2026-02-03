//! Exponential backoff for reconnection

use std::time::Duration;

use kt_core::config::BackoffConfig;

/// Exponential backoff with jitter for reconnection attempts
pub struct ExponentialBackoff {
    /// Current delay
    current: Duration,
    /// Maximum delay
    max: Duration,
    /// Multiplier
    multiplier: f64,
    /// Jitter factor (0.0 to 1.0)
    jitter: f64,
}

impl ExponentialBackoff {
    /// Create a new backoff from configuration
    pub fn from_config(config: &BackoffConfig) -> Self {
        Self {
            current: config.initial,
            max: config.max,
            multiplier: config.multiplier,
            jitter: config.jitter,
        }
    }

    /// Create a new backoff with custom parameters
    pub fn new(initial: Duration, max: Duration, multiplier: f64, jitter: f64) -> Self {
        Self {
            current: initial,
            max,
            multiplier,
            jitter,
        }
    }

    /// Get the next delay and advance the backoff
    pub fn next_delay(&mut self) -> Duration {
        let delay = self.current;

        // Calculate next delay with multiplier
        let next = Duration::from_secs_f64(self.current.as_secs_f64() * self.multiplier);
        self.current = std::cmp::min(next, self.max);

        // Add jitter
        let jitter_amount = delay.as_secs_f64() * self.jitter * rand::random::<f64>();
        delay + Duration::from_secs_f64(jitter_amount)
    }

    /// Reset the backoff to initial delay
    pub fn reset(&mut self, initial: Duration) {
        self.current = initial;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_backoff_increases() {
        let mut backoff = ExponentialBackoff::new(
            Duration::from_secs(1),
            Duration::from_secs(60),
            2.0,
            0.0, // No jitter for deterministic test
        );

        let d1 = backoff.next_delay();
        let d2 = backoff.next_delay();
        let d3 = backoff.next_delay();

        assert_eq!(d1, Duration::from_secs(1));
        assert_eq!(d2, Duration::from_secs(2));
        assert_eq!(d3, Duration::from_secs(4));
    }

    #[test]
    fn test_backoff_max() {
        let mut backoff =
            ExponentialBackoff::new(Duration::from_secs(30), Duration::from_secs(60), 2.0, 0.0);

        let d1 = backoff.next_delay();
        let d2 = backoff.next_delay();
        let d3 = backoff.next_delay();

        assert_eq!(d1, Duration::from_secs(30));
        assert_eq!(d2, Duration::from_secs(60)); // Capped at max
        assert_eq!(d3, Duration::from_secs(60)); // Still capped
    }
}

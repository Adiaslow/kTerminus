//! Time utilities for k-Terminus
//!
//! Provides common time-related operations used across crates.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// Get the current Unix timestamp in milliseconds.
///
/// # Panics
/// Panics if the system time is before the Unix epoch (1970-01-01),
/// which would indicate a severely misconfigured system.
///
/// # Examples
/// ```
/// use kt_core::time::current_time_millis;
///
/// let now = current_time_millis();
/// assert!(now > 0);
/// ```
pub fn current_time_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before UNIX epoch")
        .as_millis() as u64
}

/// Get the current Unix timestamp in seconds.
///
/// # Panics
/// Panics if the system time is before the Unix epoch.
pub fn current_time_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before UNIX epoch")
        .as_secs()
}

/// Calculate elapsed time in milliseconds since a given timestamp.
///
/// Returns 0 if the given time is in the future.
pub fn elapsed_millis(since: u64) -> u64 {
    current_time_millis().saturating_sub(since)
}

/// Calculate elapsed time as a Duration since a given millisecond timestamp.
///
/// Returns Duration::ZERO if the given time is in the future.
pub fn elapsed_duration(since_millis: u64) -> Duration {
    Duration::from_millis(elapsed_millis(since_millis))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_time_millis_is_positive() {
        let now = current_time_millis();
        assert!(now > 0);
    }

    #[test]
    fn test_current_time_secs_is_positive() {
        let now = current_time_secs();
        assert!(now > 0);
    }

    #[test]
    fn test_elapsed_millis() {
        let now = current_time_millis();
        std::thread::sleep(Duration::from_millis(10));
        let elapsed = elapsed_millis(now);
        assert!(elapsed >= 10);
    }

    #[test]
    fn test_elapsed_millis_future_time() {
        let future = current_time_millis() + 1000000;
        let elapsed = elapsed_millis(future);
        assert_eq!(elapsed, 0);
    }
}

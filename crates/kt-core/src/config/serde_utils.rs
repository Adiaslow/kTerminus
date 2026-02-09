//! Shared serialization/deserialization utilities for configuration
//!
//! This module provides common serde helpers used across configuration types.

/// Helper module for Duration serialization as seconds
///
/// This module serializes `std::time::Duration` as a u64 representing seconds,
/// which is more human-readable in TOML/JSON configuration files.
///
/// # Example
///
/// ```ignore
/// use std::time::Duration;
/// use serde::{Deserialize, Serialize};
///
/// #[derive(Serialize, Deserialize)]
/// struct Config {
///     #[serde(with = "kt_core::config::serde_utils::duration_secs")]
///     timeout: Duration,
/// }
/// ```
pub mod duration_secs {
    use serde::{self, Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    /// Serialize a Duration as seconds (u64)
    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration.as_secs())
    }

    /// Deserialize a Duration from seconds (u64)
    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(Duration::from_secs(secs))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use std::time::Duration;

    #[derive(Debug, PartialEq, Serialize, Deserialize)]
    struct TestConfig {
        #[serde(with = "duration_secs")]
        timeout: Duration,
    }

    #[test]
    fn test_duration_secs_serialize() {
        let config = TestConfig {
            timeout: Duration::from_secs(30),
        };
        let json = serde_json::to_string(&config).unwrap();
        assert_eq!(json, r#"{"timeout":30}"#);
    }

    #[test]
    fn test_duration_secs_deserialize() {
        let json = r#"{"timeout":60}"#;
        let config: TestConfig = serde_json::from_str(json).unwrap();
        assert_eq!(config.timeout, Duration::from_secs(60));
    }

    #[test]
    fn test_duration_secs_roundtrip() {
        let original = TestConfig {
            timeout: Duration::from_secs(3600),
        };
        let json = serde_json::to_string(&original).unwrap();
        let parsed: TestConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(original, parsed);
    }
}

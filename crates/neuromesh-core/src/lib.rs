//! Core contracts for the experimental NeuroMesh distributed network.
//!
//! This crate contains no network I/O. Validate configuration before allocating
//! resources or starting any listener.

pub mod identity;
pub mod lifecycle;
pub mod peers;

use serde::{Deserialize, Serialize};
use std::{fmt, time::Duration};

/// Millisecond-precision duration encoding for strict configuration documents.
mod millis_duration {
    use serde::{Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    /// Encode a duration as whole milliseconds.
    pub fn serialize<S: Serializer>(value: &Duration, serializer: S) -> Result<S::Ok, S::Error> {
        let millis = u64::try_from(value.as_millis())
            .map_err(|_| serde::ser::Error::custom("duration exceeds millisecond range"))?;
        serializer.serialize_u64(millis)
    }

    /// Decode a non-negative integer of milliseconds.
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Duration, D::Error> {
        u64::deserialize(deserializer).map(Duration::from_millis)
    }
}

/// Hard resource ceilings for this experimental implementation.
pub const MAX_PEERS: usize = 1024;
/// Maximum encoded protocol frame size, in bytes.
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;

/// Resource and liveness settings. Call [`NodeConfig::validate`] before use.
///
/// The JSON representation is strict: unknown, missing, or mistyped fields
/// are rejected, integer durations are whole milliseconds, and
/// [`NodeConfig::from_json`] validates bounds in the same step.
///
/// ```
/// use neuromesh_core::NodeConfig;
///
/// let document = r#"{
///     "max_peers": 64,
///     "max_frame_bytes": 65536,
///     "heartbeat_interval_ms": 2000,
///     "failure_timeout_ms": 10000
/// }"#;
/// let config = NodeConfig::from_json(document)?;
/// assert_eq!(config, NodeConfig::default());
/// # Ok::<(), neuromesh_core::ConfigError>(())
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NodeConfig {
    /// Maximum simultaneous peer connections.
    pub max_peers: usize,
    /// Maximum encoded frame size in bytes.
    pub max_frame_bytes: usize,
    /// Interval between probes; measured using a monotonic clock at runtime.
    #[serde(rename = "heartbeat_interval_ms", with = "millis_duration")]
    pub heartbeat_interval: Duration,
    /// Time without a valid response before a peer is unavailable.
    #[serde(rename = "failure_timeout_ms", with = "millis_duration")]
    pub failure_timeout: Duration,
}

impl Default for NodeConfig {
    fn default() -> Self {
        Self {
            max_peers: 64,
            max_frame_bytes: 64 * 1024,
            heartbeat_interval: Duration::from_secs(2),
            failure_timeout: Duration::from_secs(10),
        }
    }
}

/// A configuration violates a resource or timing invariant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigError {
    /// Peer limit must be between one and [`MAX_PEERS`].
    PeerLimit,
    /// Frame limit must be between 256 and [`MAX_FRAME_BYTES`].
    FrameLimit,
    /// Probe interval must be nonzero and shorter than the failure timeout.
    HeartbeatTiming,
    /// Configuration document is malformed, incomplete, or uses unknown fields.
    Encoding,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::PeerLimit => "max_peers must be in 1..=1024",
            Self::FrameLimit => "max_frame_bytes must be in 256..=1048576",
            Self::HeartbeatTiming => "heartbeat interval must be nonzero and below failure timeout",
            Self::Encoding => "configuration document must be strict JSON with all known fields",
        })
    }
}
impl std::error::Error for ConfigError {}

impl NodeConfig {
    /// Check bounds before network startup, without allocating or changing state.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if !(1..=MAX_PEERS).contains(&self.max_peers) {
            return Err(ConfigError::PeerLimit);
        }
        if !(256..=MAX_FRAME_BYTES).contains(&self.max_frame_bytes) {
            return Err(ConfigError::FrameLimit);
        }
        if self.heartbeat_interval.is_zero() || self.failure_timeout <= self.heartbeat_interval {
            return Err(ConfigError::HeartbeatTiming);
        }
        Ok(())
    }
    /// Parse a strict JSON configuration document and validate its bounds.
    ///
    /// Encoding problems (unknown or missing fields, wrong types, malformed
    /// JSON) fail with [`ConfigError::Encoding`]; values outside the
    /// supported ranges fail with the same errors as [`NodeConfig::validate`].
    /// The document is never partially applied: validation happens before the
    /// configuration is returned.
    pub fn from_json(document: &str) -> Result<Self, ConfigError> {
        let config: Self = serde_json::from_str(document).map_err(|_| ConfigError::Encoding)?;
        config.validate()?;
        Ok(config)
    }
    /// Encode this configuration back to its strict JSON representation.
    pub fn to_json(&self) -> Result<String, ConfigError> {
        serde_json::to_string(self).map_err(|_| ConfigError::Encoding)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_are_valid() {
        assert_eq!(NodeConfig::default().validate(), Ok(()));
    }

    #[test]
    fn peer_limit_boundaries() {
        for (max_peers, valid) in [
            (0, false),
            (1, true),
            (MAX_PEERS, true),
            (MAX_PEERS + 1, false),
        ] {
            let config = NodeConfig {
                max_peers,
                ..NodeConfig::default()
            };
            assert_eq!(config.validate().is_ok(), valid);
        }
    }

    #[test]
    fn frame_limit_boundaries() {
        for (max_frame_bytes, valid) in [
            (255, false),
            (256, true),
            (MAX_FRAME_BYTES, true),
            (MAX_FRAME_BYTES + 1, false),
        ] {
            let config = NodeConfig {
                max_frame_bytes,
                ..NodeConfig::default()
            };
            assert_eq!(config.validate().is_ok(), valid);
        }
    }

    #[test]
    fn rejects_zero_and_inverted_heartbeat_windows() {
        for (interval, timeout) in [(0, 1), (1, 0), (2, 2), (3, 2)] {
            let config = NodeConfig {
                heartbeat_interval: Duration::from_secs(interval),
                failure_timeout: Duration::from_secs(timeout),
                ..NodeConfig::default()
            };
            assert_eq!(config.validate(), Err(ConfigError::HeartbeatTiming));
        }
    }

    const DOCUMENT: &str = r#"{
        "max_peers": 8,
        "max_frame_bytes": 256,
        "heartbeat_interval_ms": 2000,
        "failure_timeout_ms": 10000
    }"#;

    #[test]
    fn strict_json_round_trips_with_documented_field_names() {
        let config = NodeConfig::from_json(DOCUMENT).unwrap();
        assert_eq!(config.max_peers, 8);
        assert_eq!(config.heartbeat_interval, Duration::from_secs(2));
        assert_eq!(config.failure_timeout, Duration::from_secs(10));
        let encoded = config.to_json().unwrap();
        assert!(encoded.contains("heartbeat_interval_ms"));
        assert!(encoded.contains("failure_timeout_ms"));
        assert_eq!(NodeConfig::from_json(&encoded), Ok(config));
    }

    #[test]
    fn rejects_unknown_missing_mistyped_and_malformed_documents() {
        for document in [
            "",
            "not json",
            r#"{"max_peers":8}"#,
            r#"{
                "max_peers": 64,
                "max_frame_bytes": 65536,
                "heartbeat_interval_ms": 2000,
                "failure_timeout_ms": 10000,
                "surprise": true
            }"#,
            r#"{
                "max_peers": 64,
                "max_frame_bytes": 65536,
                "heartbeat_interval": 2000,
                "failure_timeout_ms": 10000
            }"#,
            r#"{
                "max_peers": "64",
                "max_frame_bytes": 65536,
                "heartbeat_interval_ms": 2000,
                "failure_timeout_ms": 10000
            }"#,
            r#"{
                "max_peers": 64,
                "max_frame_bytes": 65536,
                "heartbeat_interval_ms": 2.5,
                "failure_timeout_ms": 10000
            }"#,
        ] {
            assert_eq!(
                NodeConfig::from_json(document),
                Err(ConfigError::Encoding),
                "document: {document}"
            );
        }
    }

    #[test]
    fn validates_bounds_after_decoding_documents() {
        for (document, error) in [
            (
                r#"{
                    "max_peers": 1025,
                    "max_frame_bytes": 65536,
                    "heartbeat_interval_ms": 2000,
                    "failure_timeout_ms": 10000
                }"#,
                ConfigError::PeerLimit,
            ),
            (
                r#"{
                    "max_peers": 64,
                    "max_frame_bytes": 1048577,
                    "heartbeat_interval_ms": 2000,
                    "failure_timeout_ms": 10000
                }"#,
                ConfigError::FrameLimit,
            ),
            (
                r#"{
                    "max_peers": 64,
                    "max_frame_bytes": 65536,
                    "heartbeat_interval_ms": 10000,
                    "failure_timeout_ms": 10000
                }"#,
                ConfigError::HeartbeatTiming,
            ),
            (
                r#"{
                    "max_peers": 64,
                    "max_frame_bytes": 65536,
                    "heartbeat_interval_ms": 0,
                    "failure_timeout_ms": 10000
                }"#,
                ConfigError::HeartbeatTiming,
            ),
        ] {
            assert_eq!(NodeConfig::from_json(document), Err(error));
        }
    }
}

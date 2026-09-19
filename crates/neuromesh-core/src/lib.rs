//! Core contracts for the experimental NeuroMesh distributed network.
//!
//! This crate contains no network I/O. Validate configuration before allocating
//! resources or starting any listener.

pub mod identity;
pub mod lifecycle;

use std::{fmt, time::Duration};

/// Hard resource ceilings for this experimental implementation.
pub const MAX_PEERS: usize = 1024;
/// Maximum encoded protocol frame size, in bytes.
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;

/// Resource and liveness settings. Call [`NodeConfig::validate`] before use.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NodeConfig {
    /// Maximum simultaneous peer connections.
    pub max_peers: usize,
    /// Maximum encoded frame size in bytes.
    pub max_frame_bytes: usize,
    /// Interval between probes; measured using a monotonic clock at runtime.
    pub heartbeat_interval: Duration,
    /// Time without a valid response before a peer is unavailable.
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
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::PeerLimit => "max_peers must be in 1..=1024",
            Self::FrameLimit => "max_frame_bytes must be in 256..=1048576",
            Self::HeartbeatTiming => "heartbeat interval must be nonzero and below failure timeout",
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
}

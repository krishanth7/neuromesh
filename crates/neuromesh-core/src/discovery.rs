//! Bounded discovery hints. Advertisements are untrusted and never authorize a peer.
use crate::{identity::NodeId, MAX_PEERS};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fmt,
    net::SocketAddr,
    time::{Duration, Instant},
};

/// Largest accepted lifetime for one untrusted advertisement.
pub const MAX_ADVERTISEMENT_TTL: Duration = Duration::from_secs(300);
/// Longest implementation label carried in an advertisement.
pub const MAX_ADVERTISEMENT_AGENT_BYTES: usize = 64;

/// A claimed, NMP/1-carried endpoint. It is a discovery hint, not authorization.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct DiscoveryAdvertisement {
    /// Claimed application identity; transport authentication is still required.
    pub node: NodeId,
    /// Claimed QUIC endpoint.
    pub address: SocketAddr,
    /// Lifetime in seconds, interpreted using the local monotonic clock.
    pub ttl_seconds: u16,
    /// Optional human-readable implementation label.
    pub agent: String,
}

impl DiscoveryAdvertisement {
    /// Validate bounded syntax before it can affect discovery state.
    pub fn validate(&self) -> Result<(), DiscoveryError> {
        let valid_endpoint = self.address.port() != 0
            && !self.address.ip().is_unspecified()
            && !self.address.ip().is_multicast();
        let valid_agent = self.agent.len() <= MAX_ADVERTISEMENT_AGENT_BYTES
            && !self.agent.chars().any(char::is_control);
        if self.ttl_seconds == 0
            || Duration::from_secs(u64::from(self.ttl_seconds)) > MAX_ADVERTISEMENT_TTL
            || !valid_endpoint
            || !valid_agent
        {
            return Err(DiscoveryError::Advertisement);
        }
        Ok(())
    }

    fn ttl(&self) -> Duration {
        Duration::from_secs(u64::from(self.ttl_seconds))
    }
}

/// An immutable discovery-table record.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiscoveredPeer {
    /// Last accepted untrusted advertisement.
    pub advertisement: DiscoveryAdvertisement,
    /// Monotonic local expiry time.
    pub expires_at: Instant,
}

/// Discovery operations fail without changing table state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiscoveryError {
    /// Capacity is outside the supported range.
    Configuration,
    /// An advertisement violates endpoint, lifetime, or metadata bounds.
    Advertisement,
    /// A node must not discover its own advertisement.
    SelfAdvertisement,
    /// The table is full; expire or remove records explicitly first.
    Capacity,
    /// A caller supplied a monotonic time earlier than a prior operation.
    ClockRegression,
}

impl fmt::Display for DiscoveryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "discovery table: {self:?}")
    }
}
impl std::error::Error for DiscoveryError {}

/// Pure, bounded state for discovery hints.
///
/// This type performs no I/O, does not initiate connections, and does not confer
/// trust. A caller must authenticate an endpoint through the transport before
/// registering it in [`crate::peers::PeerRegistry`].
pub struct DiscoveryTable {
    local: NodeId,
    capacity: usize,
    clock: Instant,
    peers: BTreeMap<NodeId, DiscoveredPeer>,
}

impl DiscoveryTable {
    /// Create an empty table with a capacity bounded by the global peer ceiling.
    pub fn new(local: NodeId, capacity: usize, now: Instant) -> Result<Self, DiscoveryError> {
        if !(1..=MAX_PEERS).contains(&capacity) {
            return Err(DiscoveryError::Configuration);
        }
        Ok(Self {
            local,
            capacity,
            clock: now,
            peers: BTreeMap::new(),
        })
    }

    fn check_time(&self, now: Instant) -> Result<(), DiscoveryError> {
        if now < self.clock {
            Err(DiscoveryError::ClockRegression)
        } else {
            Ok(())
        }
    }

    /// Insert or refresh an untrusted hint. Expiry remains an explicit operation.
    pub fn observe(
        &mut self,
        advertisement: DiscoveryAdvertisement,
        now: Instant,
    ) -> Result<(), DiscoveryError> {
        self.check_time(now)?;
        advertisement.validate()?;
        if advertisement.node == self.local {
            return Err(DiscoveryError::SelfAdvertisement);
        }
        if !self.peers.contains_key(&advertisement.node) && self.peers.len() == self.capacity {
            return Err(DiscoveryError::Capacity);
        }
        let expires_at = now
            .checked_add(advertisement.ttl())
            .ok_or(DiscoveryError::Advertisement)?;
        self.peers.insert(
            advertisement.node,
            DiscoveredPeer {
                advertisement,
                expires_at,
            },
        );
        self.clock = now;
        Ok(())
    }

    /// Remove records at or beyond their expiry boundary and return the count.
    pub fn expire(&mut self, now: Instant) -> Result<usize, DiscoveryError> {
        self.check_time(now)?;
        let before = self.peers.len();
        self.peers.retain(|_, peer| peer.expires_at > now);
        self.clock = now;
        Ok(before - self.peers.len())
    }

    /// Read a record without exposing mutable state.
    pub fn get(&self, id: NodeId) -> Option<&DiscoveredPeer> {
        self.peers.get(&id)
    }

    /// Iterate in stable identity order. Entries are not filtered implicitly by time.
    pub fn peers(&self) -> impl Iterator<Item = &DiscoveredPeer> {
        self.peers.values()
    }

    /// Explicitly remove one hint and free capacity.
    pub fn remove(&mut self, id: NodeId) -> Option<DiscoveredPeer> {
        self.peers.remove(&id)
    }

    /// Number of retained discovery hints.
    pub fn len(&self) -> usize {
        self.peers.len()
    }

    /// Whether no discovery hints are retained.
    pub fn is_empty(&self) -> bool {
        self.peers.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::Identity;

    fn id(n: u8) -> NodeId {
        Identity::from_seed(&[n; 32]).node_id()
    }
    fn advertisement(n: u8, ttl_seconds: u16) -> DiscoveryAdvertisement {
        DiscoveryAdvertisement {
            node: id(n),
            address: format!("127.0.0.{n}:4433").parse().unwrap(),
            ttl_seconds,
            agent: "neuromesh-test".into(),
        }
    }
    fn table(now: Instant) -> DiscoveryTable {
        DiscoveryTable::new(id(1), 2, now).unwrap()
    }

    #[test]
    fn validates_capacity_and_advertisement_bounds() {
        let t = Instant::now();
        assert!(matches!(
            DiscoveryTable::new(id(1), 0, t),
            Err(DiscoveryError::Configuration)
        ));
        assert!(matches!(
            DiscoveryTable::new(id(1), MAX_PEERS + 1, t),
            Err(DiscoveryError::Configuration)
        ));
        for mut invalid in [advertisement(2, 0), advertisement(2, 301)] {
            assert_eq!(invalid.validate(), Err(DiscoveryError::Advertisement));
            invalid.ttl_seconds = 1;
            invalid.address = "0.0.0.0:4433".parse().unwrap();
            assert_eq!(invalid.validate(), Err(DiscoveryError::Advertisement));
        }
        let mut invalid = advertisement(2, 1);
        invalid.agent = "x".repeat(MAX_ADVERTISEMENT_AGENT_BYTES + 1);
        assert_eq!(invalid.validate(), Err(DiscoveryError::Advertisement));
    }

    #[test]
    fn observe_refresh_and_expiry_are_explicit_and_inclusive() {
        let t = Instant::now();
        let mut table = table(t);
        table.observe(advertisement(2, 2), t).unwrap();
        assert_eq!(table.expire(t + Duration::from_secs(1)), Ok(0));
        table
            .observe(advertisement(2, 3), t + Duration::from_secs(1))
            .unwrap();
        assert_eq!(table.expire(t + Duration::from_secs(3)), Ok(0));
        assert_eq!(table.expire(t + Duration::from_secs(4)), Ok(1));
        assert!(table.is_empty());
    }

    #[test]
    fn rejects_self_capacity_and_clock_regression_without_mutation() {
        let t = Instant::now();
        let mut table = table(t);
        assert_eq!(
            table.observe(advertisement(1, 1), t),
            Err(DiscoveryError::SelfAdvertisement)
        );
        table.observe(advertisement(2, 10), t).unwrap();
        table.observe(advertisement(3, 10), t).unwrap();
        assert_eq!(
            table.observe(advertisement(4, 10), t),
            Err(DiscoveryError::Capacity)
        );
        let before = table.get(id(2)).unwrap().clone();
        assert_eq!(
            table.observe(advertisement(2, 10), t - Duration::from_secs(1)),
            Err(DiscoveryError::ClockRegression)
        );
        assert_eq!(table.get(id(2)), Some(&before));
        assert_eq!(
            table.expire(t - Duration::from_secs(1)),
            Err(DiscoveryError::ClockRegression)
        );
        assert_eq!(table.len(), 2);
    }

    #[test]
    fn snapshots_are_stably_ordered_and_removal_is_idempotent() {
        let t = Instant::now();
        let mut table = table(t);
        table.observe(advertisement(3, 5), t).unwrap();
        table.observe(advertisement(2, 5), t).unwrap();
        let ids: Vec<_> = table.peers().map(|peer| peer.advertisement.node).collect();
        assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
        assert!(table.remove(id(2)).is_some());
        assert!(table.remove(id(2)).is_none());
    }
}

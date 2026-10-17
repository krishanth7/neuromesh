//! Bounded peer health state. Callers supply monotonic time and authenticated identities.
use crate::{identity::NodeId, MAX_PEERS};
use std::{
    collections::BTreeMap,
    fmt,
    time::{Duration, Instant},
};

/// Health derived from time since the last authenticated observation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Health {
    /// Observation is more recent than the suspect threshold.
    Healthy,
    /// Observation has aged beyond the suspect threshold.
    Suspect,
    /// Observation has aged beyond the failure threshold.
    Unavailable,
}
/// Immutable snapshot of a registered peer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Peer {
    /// Authenticated application identity.
    pub id: NodeId,
    /// Last accepted monotonic observation.
    pub last_seen: Instant,
    /// Derived health state.
    pub health: Health,
}
/// Registry operation failed without changing state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PeerError {
    /// Capacity or thresholds are invalid.
    Configuration,
    /// The registry does not register its own identity.
    SelfPeer,
    /// Identity already exists; use observe for a heartbeat.
    Duplicate,
    /// Capacity reached; unavailable peers require explicit removal.
    Capacity,
    /// Identity is not registered.
    Unknown,
    /// Time moved backwards relative to a prior operation.
    ClockRegression,
}
impl fmt::Display for PeerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "peer registry: {self:?}")
    }
}
impl std::error::Error for PeerError {}

/// Aggregate registry statistics for health checks and operational dashboards.
///
/// Counts are derived from caller-supplied monotonic time at the moment
/// [`PeerRegistry::stats`] is called; they never mutate stored observations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegistryStats {
    /// Currently registered peers; always `<= capacity`.
    pub registered: usize,
    /// Provisioned capacity of this registry.
    pub capacity: usize,
    /// Peers whose observation age is below the suspect threshold.
    pub healthy: usize,
    /// Peers whose observation age is at or beyond the suspect threshold but
    /// below the failure threshold.
    pub suspect: usize,
    /// Peers whose observation age is at or beyond the failure threshold.
    pub unavailable: usize,
    /// Age of the oldest outstanding observation, or `None` when empty.
    pub oldest_observation: Option<Duration>,
}

impl RegistryStats {
    /// Fraction of provisioned capacity currently registered, in `0.0..=1.0`.
    pub fn utilization(&self) -> f64 {
        self.registered as f64 / self.capacity as f64
    }
    /// Whether every registered peer is currently healthy.
    pub fn all_healthy(&self) -> bool {
        self.suspect == 0 && self.unavailable == 0
    }
}

/// Pure state machine; discovery hints must never be inserted as authenticated peers.
/// No network I/O, background timers, implicit eviction, or authorization decisions.
pub struct PeerRegistry {
    local: NodeId,
    capacity: usize,
    suspect_after: Duration,
    unavailable_after: Duration,
    clock: Instant,
    peers: BTreeMap<NodeId, Peer>,
}
impl PeerRegistry {
    /// Create a registry with strictly ordered, nonzero health thresholds.
    pub fn new(
        local: NodeId,
        capacity: usize,
        suspect_after: Duration,
        unavailable_after: Duration,
        now: Instant,
    ) -> Result<Self, PeerError> {
        if !(1..=MAX_PEERS).contains(&capacity)
            || suspect_after.is_zero()
            || unavailable_after <= suspect_after
        {
            return Err(PeerError::Configuration);
        }
        Ok(Self {
            local,
            capacity,
            suspect_after,
            unavailable_after,
            clock: now,
            peers: BTreeMap::new(),
        })
    }
    fn check_time(&self, now: Instant) -> Result<(), PeerError> {
        if now < self.clock {
            Err(PeerError::ClockRegression)
        } else {
            Ok(())
        }
    }
    /// Register after transport authentication. Duplicate insertion never overwrites health.
    pub fn register(&mut self, id: NodeId, now: Instant) -> Result<(), PeerError> {
        self.check_time(now)?;
        if id == self.local {
            return Err(PeerError::SelfPeer);
        }
        if self.peers.contains_key(&id) {
            return Err(PeerError::Duplicate);
        }
        if self.peers.len() == self.capacity {
            return Err(PeerError::Capacity);
        }
        self.peers.insert(
            id,
            Peer {
                id,
                last_seen: now,
                health: Health::Healthy,
            },
        );
        self.clock = now;
        Ok(())
    }
    /// Refresh only after a valid authenticated response. Failed probes must not call this.
    pub fn observe(&mut self, id: NodeId, now: Instant) -> Result<(), PeerError> {
        self.check_time(now)?;
        let peer = self.peers.get_mut(&id).ok_or(PeerError::Unknown)?;
        peer.last_seen = now;
        peer.health = Health::Healthy;
        self.clock = now;
        Ok(())
    }
    /// Recompute health at inclusive threshold boundaries; returns number of transitions.
    pub fn tick(&mut self, now: Instant) -> Result<usize, PeerError> {
        self.check_time(now)?;
        let mut changed = 0;
        for peer in self.peers.values_mut() {
            let age = now.duration_since(peer.last_seen);
            let health = health_for(age, self.suspect_after, self.unavailable_after);
            changed += usize::from(health != peer.health);
            peer.health = health;
        }
        self.clock = now;
        Ok(changed)
    }
    /// Aggregate statistics derived at `now` without mutating stored state.
    ///
    /// Effective health uses the same inclusive thresholds as
    /// [`PeerRegistry::tick`], so a dashboard can read live counts between
    /// ticks. Returns [`PeerError::ClockRegression`] if `now` precedes the
    /// last accepted observation.
    pub fn stats(&self, now: Instant) -> Result<RegistryStats, PeerError> {
        self.check_time(now)?;
        let mut healthy = 0;
        let mut suspect = 0;
        let mut unavailable = 0;
        let mut oldest_observation: Option<Duration> = None;
        for peer in self.peers.values() {
            let age = now.duration_since(peer.last_seen);
            match health_for(age, self.suspect_after, self.unavailable_after) {
                Health::Healthy => healthy += 1,
                Health::Suspect => suspect += 1,
                Health::Unavailable => unavailable += 1,
            }
            oldest_observation = Some(match oldest_observation {
                Some(existing) => existing.max(age),
                None => age,
            });
        }
        Ok(RegistryStats {
            registered: self.peers.len(),
            capacity: self.capacity,
            healthy,
            suspect,
            unavailable,
            oldest_observation,
        })
    }
    /// Get one snapshot, without exposing mutable records.
    pub fn get(&self, id: NodeId) -> Option<Peer> {
        self.peers.get(&id).copied()
    }
    /// Iterate snapshots in stable identity order, bounded by capacity.
    pub fn peers(&self) -> impl Iterator<Item = Peer> + '_ {
        self.peers.values().copied()
    }
    /// Explicitly remove a peer and free capacity.
    pub fn remove(&mut self, id: NodeId) -> Option<Peer> {
        self.peers.remove(&id)
    }
    /// Current number of registered peers.
    pub fn len(&self) -> usize {
        self.peers.len()
    }
    /// Whether no peers are registered.
    pub fn is_empty(&self) -> bool {
        self.peers.is_empty()
    }
}

/// Effective health for an observation age at inclusive threshold boundaries.
fn health_for(age: Duration, suspect_after: Duration, unavailable_after: Duration) -> Health {
    if age >= unavailable_after {
        Health::Unavailable
    } else if age >= suspect_after {
        Health::Suspect
    } else {
        Health::Healthy
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::Identity;
    fn id(n: u8) -> NodeId {
        Identity::from_seed(&[n; 32]).node_id()
    }
    fn registry(now: Instant) -> PeerRegistry {
        PeerRegistry::new(
            id(1),
            2,
            Duration::from_secs(2),
            Duration::from_secs(5),
            now,
        )
        .unwrap()
    }
    #[test]
    fn rejects_invalid_configuration() {
        let t = Instant::now();
        for (cap, suspect, failed) in [
            (0, 2, 5),
            (MAX_PEERS + 1, 2, 5),
            (2, 0, 5),
            (2, 5, 5),
            (2, 6, 5),
        ] {
            assert!(PeerRegistry::new(
                id(1),
                cap,
                Duration::from_secs(suspect),
                Duration::from_secs(failed),
                t
            )
            .is_err());
        }
    }
    #[test]
    fn duplicates_self_and_capacity_do_not_mutate_state() {
        let t = Instant::now();
        let mut r = registry(t);
        assert_eq!(r.register(id(1), t), Err(PeerError::SelfPeer));
        r.register(id(2), t).unwrap();
        r.register(id(3), t).unwrap();
        assert_eq!(
            r.register(id(2), t + Duration::from_secs(1)),
            Err(PeerError::Duplicate)
        );
        assert_eq!(r.get(id(2)).unwrap().last_seen, t);
        assert_eq!(r.register(id(4), t), Err(PeerError::Capacity));
        assert_eq!(r.len(), 2);
        r.remove(id(2)).unwrap();
        r.register(id(4), t).unwrap();
    }
    #[test]
    fn exact_thresholds_recovery_and_no_implicit_eviction() {
        let t = Instant::now();
        let mut r = registry(t);
        r.register(id(2), t).unwrap();
        assert_eq!(r.tick(t + Duration::from_millis(1999)), Ok(0));
        assert_eq!(r.tick(t + Duration::from_secs(2)), Ok(1));
        assert_eq!(r.get(id(2)).unwrap().health, Health::Suspect);
        assert_eq!(r.tick(t + Duration::from_secs(5)), Ok(1));
        assert_eq!(r.get(id(2)).unwrap().health, Health::Unavailable);
        assert_eq!(r.tick(t + Duration::from_secs(6)), Ok(0));
        assert_eq!(r.len(), 1);
        r.observe(id(2), t + Duration::from_secs(6)).unwrap();
        assert_eq!(r.get(id(2)).unwrap().health, Health::Healthy);
    }
    #[test]
    fn backwards_time_and_unknown_observations_are_atomic() {
        let t = Instant::now();
        let mut r = registry(t);
        r.register(id(2), t).unwrap();
        r.tick(t + Duration::from_secs(5)).unwrap();
        let before = r.get(id(2));
        assert_eq!(r.observe(id(2), t), Err(PeerError::ClockRegression));
        assert_eq!(r.tick(t), Err(PeerError::ClockRegression));
        assert_eq!(r.register(id(3), t), Err(PeerError::ClockRegression));
        assert_eq!(
            r.observe(id(3), t + Duration::from_secs(9)),
            Err(PeerError::Unknown)
        );
        assert_eq!(r.get(id(2)), before);
        r.observe(id(2), t + Duration::from_secs(5)).unwrap();
    }
    #[test]
    fn stats_report_effective_health_without_mutating_state() {
        let t = Instant::now();
        let mut r =
            PeerRegistry::new(id(1), 4, Duration::from_secs(2), Duration::from_secs(5), t).unwrap();
        r.register(id(2), t).unwrap();
        r.register(id(3), t + Duration::from_secs(5)).unwrap();
        r.register(id(4), t + Duration::from_secs(6)).unwrap();

        let later = t + Duration::from_secs(7);
        let stats = r.stats(later).unwrap();
        assert_eq!(
            stats,
            RegistryStats {
                registered: 3,
                capacity: 4,
                healthy: 1,
                suspect: 1,
                unavailable: 1,
                oldest_observation: Some(Duration::from_secs(7)),
            }
        );
        assert!((stats.utilization() - 0.75).abs() < f64::EPSILON);
        assert!(!stats.all_healthy());
        // Reading statistics must not rewrite stored observations or the clock.
        assert_eq!(r.stats(later).unwrap(), stats);
        assert_eq!(r.get(id(2)).unwrap().health, Health::Healthy);
        assert_eq!(r.get(id(2)).unwrap().last_seen, t);
    }
    #[test]
    fn stats_are_empty_for_a_new_registry_and_reject_backwards_time() {
        let t = Instant::now();
        let r = registry(t);
        let stats = r.stats(t).unwrap();
        assert_eq!(stats.registered, 0);
        assert_eq!(stats.capacity, 2);
        assert_eq!(stats.oldest_observation, None);
        assert!(stats.all_healthy());
        assert_eq!(stats.utilization(), 0.0);
        assert_eq!(
            r.stats(t - Duration::from_secs(1)),
            Err(PeerError::ClockRegression)
        );
    }
    #[test]
    fn snapshots_have_stable_order_and_removal_is_idempotent() {
        let t = Instant::now();
        let mut r = registry(t);
        r.register(id(3), t).unwrap();
        r.register(id(2), t).unwrap();
        let ids: Vec<_> = r.peers().map(|p| p.id).collect();
        assert!(ids.windows(2).all(|w| w[0] < w[1]));
        assert!(r.remove(id(2)).is_some());
        assert!(r.remove(id(2)).is_none());
        r.remove(id(3));
        assert!(r.is_empty());
    }
}

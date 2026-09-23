# ADR 0005: retain bounded untrusted discovery hints

## Status

Accepted for the experimental Phase 2 discovery primitives.

## Context

NeuroMesh needs a path from a locally observed endpoint to an authenticated
session without treating discovery traffic as authority. The existing peer
registry intentionally accepts only application identities already admitted by
the authenticated QUIC transport.

## Decision

NMP/1 adds a bounded `Advertise` message and the core crate owns a pure
`DiscoveryTable`. An advertisement carries a claimed NodeId, a nonzero unicast
endpoint, a 1–300 second lifetime, and an optional bounded implementation
label. The receiver measures expiry from its own monotonic clock.

The table has a caller-selected capacity no greater than the global peer limit,
stable NodeId ordering, explicit expiry/removal, and rejects self hints,
malformed data, capacity overflow, and backwards time without mutation.

## Consequences

This creates a testable discovery boundary without opening listeners,
performing multicast, connecting to advertised endpoints, or changing trust.
Callers must authenticate and authorize a candidate through QUIC before adding
it to `PeerRegistry`. Bootstrap policy, LAN transport, heartbeat scheduling,
rate limits, and routing remain separate work.

# ADR 0004: bounded peer registry and explicit health observations

Status: implemented library boundary.

## Decision

`neuromesh-core::peers::PeerRegistry` stores at most a caller-configured 1–1024
identities. It rejects self-registration, duplicate identities, unknown heartbeat
observations, and regressing monotonic time without mutating state. Unavailable
peers retain their slots until the caller explicitly removes them. Iteration is
ordered by identity for deterministic diagnostics.

The caller supplies `Instant` to `register`, `observe`, and `tick`; no wall-clock
values or hidden background tasks influence health. `tick` marks a peer suspect
at `age >= suspect_after` and unavailable at `age >= unavailable_after`. A valid
observation restores healthy status. Both thresholds are nonzero and strictly
ordered. Snapshots reflect the last explicit update, not an implicit clock read.

## Integration and trust

The registry does not authenticate identities. Register only `Session::peer()`
after successful transport authentication; never register discovery hints as
trusted peers. Refresh observations only after a verified application response.
The mutable registry is owned by one event loop; no lock is held across I/O.

The QUIC integration test creates two authenticated sessions with the same
identity and proves duplicate registration is rejected while the original
session still responds. The transport intentionally remains a lower-level
connection library: it does not automatically own or enforce this registry.
A runtime must explicitly drop rejected duplicate sessions.

## Evidence and limits

Five deterministic core tests cover configuration, capacity, duplicates, exact
health thresholds, recovery, stale time, unknown peers, ordering, and removal.
An additional real QUIC integration test exercises authenticated registration.
The measurement example also registers its authenticated peer and refreshes it
after each valid Pong.

This is a prerequisite for discovery and failure detection. Automatic discovery,
probe scheduling, reconnection, routing, and cluster membership consensus are
not implemented by this change. Unavailability reflects a timeout policy, not
proof that a machine has failed.

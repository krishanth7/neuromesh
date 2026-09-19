# Architecture

## Current implementation

`neuromesh-core` is a pure Rust library defining resource and heartbeat settings.
Validation rejects peer counts outside 1–1024, frames outside 256–1,048,576 bytes,
zero heartbeat intervals, and failure timeouts no greater than the interval.
These are configuration contracts; no runtime enforcement is claimed yet.

The core also owns Ed25519 identities and domain-separated BLAKE3 fingerprints.
Secret keys are neither serializable nor printable. Lifecycle transitions enforce
Created → Starting → Running → Draining → Stopped, with explicit failure exits.
See [ADR 0002](docs/architecture/0002-identity.md).

## Planned boundaries

- Core: identities, validated values, lifecycle and task contracts.
- Protocol: bounded versioned serialization, no socket ownership.
- Network/node: Tokio runtime, authenticated QUIC, bounded queues and graceful shutdown.
- Discovery: untrusted hints promoted to peers only after authentication.
- Routing: graph snapshots and deterministic route selection.
- Scheduler: explicit task states, attempt generations, bounded retry policy.
- Observability: read-only snapshots; never private keys or task secrets.

Create a new crate only when an implementation and dependency boundary justify it.
No locks may be held across network I/O. Runtime deadlines use monotonic clocks.
Topology updates and task attempts need generations to reject stale events.
Discovery is not authorization. Route selection alone is not packet forwarding.
Task retries do not imply exactly-once execution.

## Trust model

Assume peers can supply malformed data, lie about metrics, replay messages,
withhold results, or disconnect. Initial deployments should use explicit trusted
peer configuration. Authenticate identities, bound all inputs/queues, apply
connection and operation timeouts, and document denial-of-service limitations.

## Decisions

See [ADR 0001](docs/architecture/0001-foundation.md). Later security-sensitive
protocol decisions require their own rationale and tests before release.

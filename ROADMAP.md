# Engineering roadmap

[Public project](https://github.com/users/krishanth7/projects/6)

This roadmap describes intent, not delivered features or promised dates.
Releases require all relevant tests, CI, review, and documentation to pass.

| Phase | Target release | Scope | Exit evidence |
|---|---|---|---|
| 0–1 | v0.1.0 Foundation | Workspace, CI, policies, identity, lifecycle | Build/lint/tests, identity tampering tests |
| 1 | v0.2.0 Connected Nodes | NMP/1, QUIC, registry | Real two-node handshake and authentication failures |
| 2 | v0.3.0 Mesh Discovery | LAN discovery, bootstrap, heartbeat | Discovery, expiration, timeout tests |
| 3–4 | v0.4.0 Adaptive Routing | Graph, scoring, recovery | Alternate route after intermediate node failure |
| 5 | v0.5.0 Distributed Compute | Tasks, deterministic placement, retries | Remote execution and stale-result rejection |
| 6 | v0.6.0 Observatory | Metrics, API, dashboard | Live topology and read-only API verification |
| 7 | v0.7.0 Hardened Mesh | Simulation, benchmarks, security review | Recorded measurements, negative tests, artifacts |

## Pull request sequence

1. Workspace and repository foundation.
2. Identity and lifecycle.
3. Versioned bounded protocol.
4. Authenticated QUIC transport.
5. Peer registry.
6. LAN discovery and bootstrap.
7. Heartbeat and failure detection.
8. Topology graph.
9. Shortest-path routing.
10. Transparent adaptive scoring.
11. Failure propagation, alternate routing, reconnection.
12. Operational CLI and reproducible multi-node demo.
13. Distributed task model.
14. Deterministic scheduler and workers.
15. Timeout, retry, reassignment, stale-result protection.
16. Tracing and metrics.
17. Read-only topology API.
18. Minimal live dashboard.
19. Reproducible simulations and benchmarks.
20. Security hardening and release artifacts.

The limit is 20 substantive PRs, not a quota. Related steps may be combined when
that makes review clearer. Approximately 10 engineering issues will be opened
as their requirements are investigated, not created solely to close them.

## Current state

The current code includes foundation, identity/lifecycle, bounded protocol,
authenticated QUIC transport, a bounded peer registry, and bounded discovery
advertisement primitives. LAN discovery transport and bootstrap policy remain
pending. The autonomous mesh remains pending. The v0.1.0 foundation release is saved as a draft;
publication remains pending. No releases have been published.

## Peer registry and interoperability update

The bounded peer registry and deterministic health policy are implemented as a core library,
with an authenticated QUIC integration test. Automatic discovery, heartbeat scheduling,
routing, tasks, and observability remain planned. The 17-language toolkit is an interoperability
and reporting surface; it does not imply 17 implementations of the mesh runtime.

## Discovery advertisement update

NMP/1 now carries a bounded, untrusted `Advertise` message and the core library
retains valid hints in a deterministic, expiring table. This is not a multicast
listener, bootstrap implementation, connection manager, or authorization path.

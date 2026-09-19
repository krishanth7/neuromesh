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

Foundation implementation under review. No releases have been published.

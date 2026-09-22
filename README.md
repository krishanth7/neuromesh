# NeuroMesh

**Self-Organizing Distributed Intelligence Network**

[![CI](https://github.com/krishanth7/neuromesh/actions/workflows/ci.yml/badge.svg)](https://github.com/krishanth7/neuromesh/actions/workflows/ci.yml)

NeuroMesh is an experimental Rust project exploring secure peer communication,
adaptive routing, failure recovery, and decentralized computation. Its planned
adaptive behavior uses graph algorithms, statistical measurements, and explicit
scheduling heuristics—not hosted inference, pretrained models, or machine learning.

## Project status

**Experimental transport library stage.** The current implementation provides a Rust workspace and
validated configuration, Ed25519 identities, lifecycle transitions, bounded NMP/1
encoding, and authenticated QUIC sessions. It does not yet launch a mesh, route
traffic, or execute distributed tasks. No production-readiness or performance
claims are made. See the [roadmap](ROADMAP.md) and
[public engineering project](https://github.com/users/krishanth7/projects/6).

## Why NeuroMesh?

Explore how small independent nodes can discover each other, measure connection
health, select transparent routes, and recover from failures. Every milestone
must be reproducible through tests and documented commands.

## Quick start

Install Rust stable with Cargo, rustfmt, and Clippy, plus Python 3 for the local
link checker. Clone the repository, then run:

```sh
git clone https://github.com/krishanth7/neuromesh.git
cd neuromesh
cargo build --locked --workspace
cargo test --locked --workspace --all-features
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets --all-features -- -D warnings
cargo doc --locked --workspace --no-deps
python3 scripts/check_links.py
```

There is no CLI binary yet. Do not use future `neuromesh node start` examples
until the networking and CLI milestones are delivered.

## Architecture

The [architecture](ARCHITECTURE.md) separates pure domain contracts from async I/O.
`neuromesh-core`, `neuromesh-protocol`, and `neuromesh-network` exist today. Discovery, routing, and scheduler
modules will be introduced with actual implementations, not empty placeholders.

| Capability | Status |
|---|---|
| Resource and heartbeat configuration validation | Implemented |
| Ed25519 identity, fingerprints, strict signature verification, lifecycle | Implemented |
| Bounded NMP/1 control-message encoding | Implemented; [specification](docs/protocol/NMP-1.md) |
| Mutual-TLS QUIC and session-bound identity proofs | Implemented library; [design and limits](docs/architecture/0003-quic.md) |
| Discovery and failure detection | Planned |
| Weighted routing and recovery | Planned |
| Distributed tasks and reassignment | Planned |
| Metrics, read-only API, and observatory | Planned |
| Reproducible benchmarks | Planned; no results published |

## Run the two-node transport test

```sh
cargo test --locked -p neuromesh-network --test quic real_quic_identity_ping_and_graceful_close -- --nocapture
```

This starts real localhost QUIC endpoints with ephemeral test certificates,
authenticates both identities, exchanges three pings, and checks shutdown. It is
a transport integration test, not the future autonomous multi-node mesh demo.

## Security

This is experimental software. Review [SECURITY.md](SECURITY.md) before use.
Private keys and network-specific secrets must never be committed. Security
boundaries and remaining limitations will be documented with each implementation.

## Contributing and support

Read [CONTRIBUTING.md](CONTRIBUTING.md), [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md),
and [SUPPORT.md](SUPPORT.md). Work proceeds through substantive pull requests,
passing CI, actual diff review, and evidence-based milestone releases.

## License

[Apache License 2.0](LICENSE). The complete license is retained without changes.

Designed for Developers

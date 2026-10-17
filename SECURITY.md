# Security policy

NeuroMesh is experimental and has no production-supported release yet. Do not
expose development listeners to untrusted networks or use sensitive workloads.
The default branch receives development fixes; historical snapshots have no
maintenance guarantee.

## Supported versions

| Version | Supported |
|---|---|
| `main` (development head) | Yes — fixes land here |
| Tagged releases | None published yet |
| Historical snapshots and forks | No maintenance guarantee |

## Reporting

Do not publish exploit details or secrets in an issue. Use GitHub's private
vulnerability reporting option if available under the repository Security tab.
If unavailable, open an issue requesting a private reporting channel without
technical exploit details, and wait for maintainer instructions. No response
time or remediation SLA is promised.

Include, when you can do so safely: the component and commit, the impact in
concrete terms (which contract or property breaks), a minimal reproduction or
poc, and your discovery method. Best-effort handling targets, explicitly not a
contractual SLA: acknowledge reports that reach a private channel, triage by
severity, and coordinate disclosure timing with the reporter. Fixes are
published to `main` with an explanatory change record.

## Severity classification

| Severity | Meaning in this project |
|---|---|
| Critical | Disclosure or derivation of private key material; authentication bypass allowing an unlisted identity to establish or impersonate a session; memory unsafety (the workspace forbids `unsafe`) |
| High | Replay or reflection of identity proofs; accepting malformed or oversized frames past documented bounds; remote resource exhaustion that defeats the configured caps |
| Medium | Logic errors in health, lifecycle, or sequence handling that mislead an operator without granting channel access; diagnostic output leaking payload bytes |
| Low | Denial of service requiring pre-existing authorized access; hardening gaps with no proven path to the above |

## Threat model

Assume the network is hostile, peers lie, and discovery hints are untrusted
claims. Assume the host running your node is not in the threat model for key
protection at rest: file permissions and your secret store are yours.

| Asset | Threat | Existing control | Residual risk |
|---|---|---|---|
| Ed25519 private keys | Theft from memory or logs at rest | Generated seeds are zeroized; secret identities have no `Debug` or serialization; keys never appear in errors or telemetry | Imported seeds are caller-owned; OS compromise is out of scope |
| Session authenticity | Forged or substituted peer identity | Mutual TLS 1.3, hostname checks, explicit NodeId allowlist, session-bound Ed25519 proof over the TLS exporter with role separation | No per-certificate-to-NodeId pinning; a compromised trusted CA plus allowlisted identity enables impersonation |
| Message replay | Replayed proofs, requests, or responses | Proof binding prevents cross-session reflection; per-session monotonically increasing IDs with responder-side sequence checks; response sender and ID verified against the authenticated peer | Application-level nonce hygiene for own protocols remains the caller's job |
| Resource exhaustion | Flooded connections, streams, or huge frames | Connection caps including in-flight handshakes, 64 KiB frame ceiling, stream and window limits, handshake and request deadlines, bounded registry | No per-peer rate limiting or Internet-facing DoS guarantee; independent audit not performed |
| Malformed input | Crashes or unexpected acceptance | Strict decoding with unknown/duplicate field rejection, Serde recursion limit, bounded writers, `unsafe` forbidden workspace-wide, adversarial decode corpus in tests | Fuzzing beyond the deterministic corpus is not yet automated in CI |
| Diagnostic leakage | Secrets or payloads in logs and metrics | Every error string is static; counters and labels are fixed-cardinality and payload-free by the bounded-telemetry decision recorded in [Architecture](ARCHITECTURE.md) | Embedding applications choose their own logging elsewhere |
| Dependency supply chain | Tampered or vulnerable crates | `Cargo.lock` committed and enforced (`--locked` in every command and CI), scoped dependency set, automated version-bump proposals reviewed as pull requests | No automated advisory-audit job in CI yet |

## Engineering requirements

Authenticate peers independently of discovery; reject malformed/version-mismatched
messages; cap frames, connections, queues, and task work; use monotonic deadlines;
keep keys out of logs and version control. Retries require explicit attempt IDs
and stale-result rejection. Document remaining risks before each release.

Current scope includes configuration, identity primitives, and a QUIC transport library.
Generated temporary key seeds are zeroized, and secret identity objects have no
Debug or serialization implementation. Raw signature verification alone does not
prevent replay or authorize peers. Caller-owned imported seeds must be protected.

The transport implements mutual TLS, explicit NodeId authorization, session-bound
identity proofs, request sequence checks, bounded frames/connections, and deadlines.
See [transport limits](docs/architecture/0003-quic.md) and the
[NMP/1 specification](docs/protocol/NMP-1.md). It has not received an
independent security audit; there is no task sandbox or Internet DoS guarantee.

## Operator checklist before any non-lab deployment

- Populate the NodeId allowlist explicitly; empty or wildcard trust is not a mode.
- Rotate certificate chains and trust roots on a schedule you can test.
- Keep listeners private until the audit gap above is accepted by your organization.
- Run `cargo audit` or equivalent advisory scanning yourself until CI does it.
- Never commit seeds, keys, or deployment configuration with embedded secrets.

[Contributing](CONTRIBUTING.md) · [Support](SUPPORT.md) ·
[Architecture](ARCHITECTURE.md)
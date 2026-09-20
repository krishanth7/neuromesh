# Security policy

NeuroMesh is experimental and has no production-supported release yet. Do not
expose development listeners to untrusted networks or use sensitive workloads.
The default branch receives development fixes; historical snapshots have no
maintenance guarantee.

## Reporting

Do not publish exploit details or secrets in an issue. Use GitHub's private
vulnerability reporting option if available under the repository Security tab.
If unavailable, open an issue requesting a private reporting channel without
technical exploit details, and wait for maintainer instructions. No response
time or remediation SLA is promised.

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
See [transport limits](docs/architecture/0003-quic.md). It has not received an
independent security audit; there is no task sandbox or Internet DoS guarantee.

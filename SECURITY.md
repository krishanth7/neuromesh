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

Current scope is configuration validation only. Transport authentication, replay
handling, runtime resource limits, and sandboxing are not implemented yet.

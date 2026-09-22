# Reproducible evidence

These files contain observed runs, not estimated targets or invented dashboard data.
`base_commit` identifies the starting revision; per-file SHA-256 values identify
the actual source used, including uncommitted changes in the recorded development
run. These are historical snapshots; subsequent compatibility and harness fixes
are validated by the current CI run and its artifact, not retroactively inserted
into an earlier measurement. Dates, compiler/runtime versions, and environment information are retained.

## Localhost transport

[localhost-quic.json](localhost-quic.json) contains all 100 RTT samples and the
handshake time. Reproduce from the repository root:

```sh
python3 scripts/record_measurement.py
```

The script builds the measurement example in release mode, exchanges 10 warmup
and 100 measured pings over real authenticated localhost QUIC, validates the
reported nearest-rank percentiles, and writes the report. The p50 is sorted sample
50 and p95 is sorted sample 95, using one-based numbering.

Limitations: one run, two endpoints in one process, no CPU isolation, ephemeral
test certificates and fixed test identities, no WAN emulation, no throughput or
memory benchmark. RTT includes application serialization and transport ACK wait.
Do not generalize these results to deployed clusters or compare against other
transports without matching the methodology.

## Interoperability

[interop.json](interop.json) records the completed 570/570-case development run.
[verification.txt](verification.txt) preserves the full successful validation log
with the checkout path normalized to `<checkout>`.

The conformance runner records every case and its outcome, runtime versions,
source hashes, and SQL aggregates. To generate a fresh report:

```sh
python3 scripts/check_interop.py --output target/interop-results.json
```

Every encoder runs four valid and 34 invalid cases. Valid cases include unsigned
64-bit maximum values and a value beyond JavaScript's exact-number range. Invalid
cases cover argument counts, non-ASCII input, invalid hex, zero message IDs,
noncanonical decimals, whitespace, signs, fractions, exponents, and overflow.
Each valid output must match the expected JSON values and pass Rust decoding.
These checks establish the documented Ping encoder contract, not complete NMP/1
client interoperability or security certification.

The Bash validation entry point invokes the complete Rust checks and the entire
conformance suite. SQL aggregation runs inside Python's SQLite engine and is
cross-checked against the actual in-memory outcomes. The CI artifact is named
`interop-results` and is separate from the checked-in development run.

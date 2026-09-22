# NMP/1 interoperability toolkit

Fifteen executable encoders implement the same small, useful boundary: an NMP/1
`Ping` JSON envelope. SQL reports conformance results and Bash runs the validation
pipeline. Together with the Rust core, this repository uses **17 languages**.
These are protocol examples, **not 15 production SDKs or non-Rust QUIC clients**.

## Contract

Every encoder takes exactly three positional arguments:

```text
NODE_HEX MESSAGE_ID NONCE
```

- `NODE_HEX`: exactly 64 ASCII hexadecimal characters, case insensitive.
- `MESSAGE_ID`: canonical decimal integer in `1..18446744073709551615`.
- `NONCE`: canonical decimal integer in `0..18446744073709551615`.
- Success: one UTF-8 JSON envelope on stdout; optional final newline.
- Failure: nonzero exit, diagnostic on stderr, empty stdout.

Leading zeroes, signs, whitespace, exponents, fractions, and overflow are rejected.
Full unsigned 64-bit values must stay exact, including values above JavaScript's
safe integer limit. String-based implementations validate before interpolation.
`sender` is a JSON array of 32 bytes, matching the Rust `NodeId` representation;
the CLI hex input is a convenience format, not the wire format.

The output excludes QUIC stream framing and performs no authentication. A node ID
in JSON is only a claim. Real sessions must verify mutual TLS, the session-bound
Ed25519 proof, the allowlist, sender identity, and request ordering in Rust.

## Language inventory

| Language | Implementation | Role |
|---|---|---|
| Rust | [reference encoder](../crates/neuromesh-protocol/examples/nmp_ping.rs) | Core, QUIC transport, registry, reference encoder and decoder |
| Python | [ping.py](python/ping.py) | Encoder, conformance runner, measurement provenance |
| JavaScript | [ping.mjs](javascript/ping.mjs) | Node.js encoder with lossless u64 tokens |
| TypeScript | [ping.ts](typescript/ping.ts) | Typed Node.js encoder; Node 24 native type stripping |
| Go | [ping.go](go/ping.go) | Standalone compiled encoder |
| C | [ping.c](c/ping.c) | Dependency-free encoder with checked hexadecimal input |
| C++ | [ping.cpp](cpp/ping.cpp) | Standard-library encoder |
| Java | [Ping.java](java/Ping.java) | JVM encoder |
| C# | [Ping.cs](csharp/Ping.cs) | Mono-compatible encoder |
| Ruby | [ping.rb](ruby/ping.rb) | Arbitrary-precision encoder |
| PHP | [ping.php](php/ping.php) | Encoder preserving unsigned decimal values |
| Perl | [ping.pl](perl/ping.pl) | Encoder using lexical bounds checks |
| Lua | [ping.lua](lua/ping.lua) | Encoder avoiding floating-point integer conversion |
| R | [ping.R](r/ping.R) | Encoder preserving full u64 values |
| Haskell | [Ping.hs](haskell/Ping.hs) | Arbitrary-precision encoder |
| SQL | [schema](sql/schema.sql), [summary](sql/summary.sql) | Constrained result storage and per-language aggregation |
| Bash | [verify.sh](../scripts/verify.sh) | Fail-fast build, lint, test, documentation, and conformance pipeline |

JSON, YAML, Markdown, and configuration files are not counted as programming languages.

## Run

Install Rust stable, Python 3, Node.js 24, GCC/G++, Go, a JDK, Mono (`mcs` and
`mono`), Ruby, PHP CLI, Perl, Lua 5.4, R, and GHC. Python's standard SQLite module
executes SQL; no standalone database service is required. Ubuntu package setup
is recorded in [the CI workflow](../.github/workflows/interop.yml).

```sh
bash scripts/verify.sh
# Explicit subset for contributors without every runtime:
python3 scripts/check_interop.py --languages rust python javascript
```

The default run requires every encoder; missing runtimes fail instead of silently
skipping coverage. A subset is always identified in the report. Non-Rust builds go into
`target/interop`; Rust oracle and encoder builds explicitly use `target/interop-cargo`
so Cargo environment/configuration cannot redirect them away from the executed paths; the report is `target/interop-results.json`. Thirty-eight cases
per encoder cover four valid vectors and 34 invalid vectors. Every valid output
is compared with expected values **and decoded by the actual Rust NMP/1 decoder**.
SQL aggregates are compared with the runner's observed outcomes.

The TypeScript example is executed by Node's native type stripping. It is not
advertised as a separately type-checked SDK. These examples currently cover Ping
encoding only; general decoding and the other control messages remain Rust-only.

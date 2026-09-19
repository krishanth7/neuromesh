# ADR 0001: Pure core and staged network implementation

Status: accepted for the foundation milestone.

Keep validation and state contracts separate from async networking so boundary
conditions can be exhaustively tested without sockets or timing assumptions.
Start with one useful crate; do not create the target directory tree as empty
packages. Rust stable is the development toolchain; Cargo.lock records dependency
resolution. There is no promised MSRV until a separate compiler is tested in CI.

Future network crates must apply these configuration bounds at runtime. A
successful configuration check alone is not a claim of network safety.

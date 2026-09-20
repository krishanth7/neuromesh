# NeuroMesh Protocol version 1 (NMP/1)

Status: experimental encoding contract. Transport is the next milestone; this
specification does not claim a running network implementation yet.

## Framing and versioning

One stream request contains a four-byte unsigned big-endian payload length,
followed by UTF-8 JSON of at most 65,536 bytes. The response uses the same framing.
A transport must check the length before allocating and enforce an operation
 deadline; it must not wait indefinitely for a peer to finish sending.

`Envelope` requires exactly `version`, `message_id`, `sender`, and `message`.
Version is integer 1; other versions fail closed. IDs are positive u64 values,
monotonically increasing per authenticated request session. Responses echo the
request ID. Sender is a 32-byte JSON array holding the NodeId fingerprint; the
transport must compare it to the authenticated peer identity. Timestamps are
omitted: local monotonic clocks drive deadlines and remote wall clocks are not
trusted. IDs correlate requests; they do not replace authenticated session binding.

## Messages

The message uses `type` and, for data-bearing variants, `body`.

| Type | Body | Validation |
|---|---|---|
| Hello | agent | 1–64 UTF-8 bytes; no control characters |
| Ping | nonce | u64 |
| Pong | nonce | u64; caller must match the outstanding probe |
| GetPeers | absent | no payload |
| Peers | peers | At most 64 node/address hints; nonzero port; no unspecified/multicast addresses |
| Error | code, detail | u16 code; at most 256 UTF-8 bytes; no control characters |

Unknown/duplicate envelope fields, malformed JSON, invalid shapes, zero IDs,
unsupported versions, and invalid message bounds are rejected. Serde's JSON
recursion limit remains enabled. Both inbound parsing and outbound serialization
have hard byte ceilings. The core crate has broader configurable ceilings; NMP/1
always applies its stricter 64 KiB maximum.

## Authentication and trust (required transport behavior)

TLS must validate configured trust roots and peer certificates. The Ed25519
application identity must be bound to the TLS session and explicitly authorized;
discovery advertisements never authorize a peer. No 0-RTT application work.
HELLO occurs only after identity proof validation. Implementations must cap
connection counts, concurrent streams, queues, and message rates.

## Errors and termination

Encoding errors are returned as stable categories without embedding received
payloads. A transport should terminate malformed or unauthenticated streams and
close sessions that violate identity/version contracts. Sanitized Error messages
may report expected application failures; never include secrets or raw internals.

## Compatibility and future extensions

Routing metadata, distributed task messages, retry generations, and topology
propagation are not implemented in this revision. Adding a variant is a protocol
change requiring tests, documentation, and explicit compatibility consideration.
Do not describe a route calculation as packet forwarding or a retry as exactly-once.

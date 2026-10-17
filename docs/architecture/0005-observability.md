# ADR 0005: bounded telemetry, stable diagnostic labels, and explicit close reports

Status: accepted; the described surface lands incrementally with the changes it documents.

## Decision

Observability in NeuroMesh is additive, constant-bounded, and payload-free.

1. **Aggregate counters, snapshot reads.** `MeshEndpoint::metrics()` returns an
   `EndpointMetrics` copy compiled from relaxed atomic counters written at the
   transport's decision points: dial attempts and outcomes, accepted sessions,
   capacity refusals, handshake rejections, exchange successes and failures, and
   explicit endpoint closes. Counting is advisory: it may coarsen under
   concurrency, holds no lock across I/O, and allocates nothing per event. The
   counter set is a fixed enum-like struct, so metric cardinality is constant
   regardless of peer count or traffic.

2. **Stable labels, not Debug dumps.** Rejection categories expose fixed
   identifiers: `ProtocolError::code()` (lowercase wire labels such as
   `frame-size`), `ProtocolError::wire_code()` (the numeric categories reserved
   for `Message::Error` and tabulated in the NMP/1 specification), and
   `PeerError::code()` for the registry. `Display` renders the stable label so
   dashboards and log pipelines never depend on Rust identifier spelling. The
   labels are a compatibility surface: renaming one is a breaking change that
   belongs behind a version bump, pinned by explicit stability tests.

3. **Classified failures instead of collapsed messages.** A request deadline is
   reported as `std::io::ErrorKind::TimedOut`, a sender or message-ID mismatch
   as `ErrorKind::InvalidData`, and transport failures propagate their original
   error type unchanged, so callers can distinguish "the peer is slow" from
   "the peer or the network is broken" without string matching. Every error
   message is a static string; none embed received bytes.

4. **Explicit close reports.** An application close carries a code and a reason
   bounded to 127 UTF-8 bytes, validated before the connection is touched.
   `Session::wait_closed()` yields `{code, reason, local}`, separating a close
   the peer applied from one this endpoint applied and collapsing
   transport-level terminations (timeout, reset) to a code-0 report without
   transport internals.

## Non-goals

The library itself emits no log lines, ships no tracing subscriber, and holds
no event queue. There are no histograms, no per-peer time series, no wall-clock
timestamps inside counters, and no labels containing node IDs, addresses, frame
bytes, or key material — both to bound cardinality and to keep telemetry free of
identity data that could leak through a scrape. Counters are operational
signals, never an audit log and never an authorization input: a writable metric
must not change who is trusted, and a dashboard must not become a control
plane.

## Integration and trust

Telemetry is derived, never declared. Counters increment only where the
transport already made the decision they name, so a denied dial counts once and
a successful session cannot be counted twice. Snapshots are local reads of an
endpoint handle; exposing them across a process boundary is the embedding
application's choice and must not weaken the identity allowlist or session
policy in doing so. Reasons and labels flow outward only: nothing in this
surface reads a peer-supplied value back into a decision.

## Evidence and limits

Each element is pinned by tests delivered with the change that introduces it:
snapshot copies and zero-default semantics; exact counter values across a real
localhost QUIC session (one dial, one accept, three exchanges per side, one
close each); label and wire-code stability with uniqueness checks; kind and
exact static message for both failure classes, including a full 5-second
deadline expiry observed over a live connection; reason bounds in ASCII and
multi-byte UTF-8 with an error that carries no reason bytes; and a close code
and reason observed by the opposite endpoint.

Limits worth stating: relaxed ordering means concurrent increments are
eventually, not transactionally, visible; counters do not survive a process
restart; the deadline, close, and accept tests run on one machine's loopback
and are not a latency study. Automatic discovery, sampling, and any persisted
telemetry store remain out of scope for this decision.
# ADR 0002: Ed25519 identity and domain-separated fingerprints

Status: accepted for core identity primitives; transport binding remains future work.

A node owns an Ed25519 key generated from OS entropy. The NodeId is the 32-byte
BLAKE3 derived-key output using context `NeuroMesh NodeId v1` over the Ed25519
public key. Display and CLI parsing use 64 hexadecimal characters. Serde encodes
the fingerprint as a 32-byte array; no wire compatibility is promised yet.

Strict signature verification uses ed25519-dalek. It proves message/key integrity,
not permission to join a mesh. Future handshakes must bind a fresh challenge and
session to an explicitly trusted identity. Signatures do not provide replay
protection, confidentiality, or network authorization on their own.

Identity intentionally does not implement Debug, Serialize, or Clone. Generated
seeds are zeroized after import and on entropy failure; dalek zeroizes its signing
key on drop. Callers own the safety of imported seed copies. Persistence and OS
file permissions are deferred to the operational CLI implementation.

Lifecycle state is read-only to callers. Invalid transitions leave state unchanged.
Stopped and Failed are terminal: create a new runtime instance for a restart.

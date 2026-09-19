//! Cryptographic node identity. Key ownership is not authorization or replay protection.
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use rand_core::{OsRng, RngCore};
use serde::{Deserialize, Serialize};
use std::{fmt, str::FromStr};
use zeroize::Zeroize;

/// Domain-separated BLAKE3 fingerprint of an Ed25519 public key.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
pub struct NodeId([u8; 32]);

impl NodeId {
    /// Derive a node fingerprint from a verified Ed25519 public key.
    pub fn from_public_key(key: &VerifyingKey) -> Self {
        Self(blake3::derive_key("NeuroMesh NodeId v1", key.as_bytes()))
    }
    /// Return the 32-byte public fingerprint.
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}
impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

/// Invalid node fingerprint encoding.
#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub struct InvalidNodeId;
impl fmt::Display for InvalidNodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("node ID must be exactly 64 ASCII hexadecimal characters")
    }
}
impl std::error::Error for InvalidNodeId {}
impl FromStr for NodeId {
    type Err = InvalidNodeId;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.len() != 64 || !s.is_ascii() {
            return Err(InvalidNodeId);
        }
        let mut bytes = [0; 32];
        for (i, byte) in bytes.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).map_err(|_| InvalidNodeId)?;
        }
        Ok(Self(bytes))
    }
}

/// Owns an Ed25519 secret. Deliberately has no Debug or serialization implementation.
pub struct Identity(SigningKey);
impl Identity {
    /// Generate a key using OS entropy, returning an error if entropy is unavailable.
    pub fn generate() -> Result<Self, rand_core::Error> {
        let mut seed = [0; 32];
        if let Err(error) = OsRng.try_fill_bytes(&mut seed) {
            seed.zeroize();
            return Err(error);
        }
        let identity = Self::from_seed(&seed);
        // The signing key zeroizes on drop. The caller owns any imported seed.
        seed.zeroize();
        Ok(identity)
    }
    /// Import a 32-byte secret seed. Caller must protect and erase its own copy.
    pub fn from_seed(seed: &[u8; 32]) -> Self {
        Self(SigningKey::from_bytes(seed))
    }
    /// Public key, safe to advertise; does not disclose the secret seed.
    pub fn public_key(&self) -> VerifyingKey {
        self.0.verifying_key()
    }
    /// Stable fingerprint of this identity's public key.
    pub fn node_id(&self) -> NodeId {
        NodeId::from_public_key(&self.public_key())
    }
    /// Sign a message. Protocol callers must bind context, nonce, and session.
    pub fn sign(&self, message: &[u8]) -> Signature {
        self.0.sign(message)
    }
    /// Strict Ed25519 verification. This does not establish peer authorization.
    pub fn verify(
        key: &VerifyingKey,
        message: &[u8],
        signature: &Signature,
    ) -> Result<(), ed25519_dalek::SignatureError> {
        key.verify_strict(message, signature)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn identity_is_stable_and_keys_are_distinct() {
        let a = Identity::from_seed(&[1; 32]);
        assert_eq!(a.node_id(), Identity::from_seed(&[1; 32]).node_id());
        assert_ne!(a.node_id(), Identity::from_seed(&[2; 32]).node_id());
        assert_eq!(
            a.node_id().to_string().parse::<NodeId>().unwrap(),
            a.node_id()
        );
        let encoded = serde_json::to_vec(&a.node_id()).unwrap();
        assert_eq!(
            serde_json::from_slice::<NodeId>(&encoded).unwrap(),
            a.node_id()
        );
    }
    #[test]
    fn rejects_message_key_and_signature_tampering() {
        let a = Identity::from_seed(&[1; 32]);
        let b = Identity::from_seed(&[2; 32]);
        let sig = a.sign(b"nmp/1 test");
        assert!(Identity::verify(&a.public_key(), b"nmp/1 test", &sig).is_ok());
        assert!(Identity::verify(&a.public_key(), b"nmp/1 altered", &sig).is_err());
        assert!(Identity::verify(&b.public_key(), b"nmp/1 test", &sig).is_err());
        let mut bytes = sig.to_bytes();
        bytes[0] ^= 1;
        assert!(Identity::verify(
            &a.public_key(),
            b"nmp/1 test",
            &Signature::from_bytes(&bytes)
        )
        .is_err());
    }
    #[test]
    fn rejects_bad_fingerprints_without_panicking() {
        for value in [
            "".to_owned(),
            "a".repeat(63),
            "g".repeat(64),
            "é".repeat(32),
            "a".repeat(65),
        ] {
            assert!(value.parse::<NodeId>().is_err());
        }
    }
    #[test]
    fn os_generated_keys_are_distinct() {
        assert_ne!(
            Identity::generate().unwrap().node_id(),
            Identity::generate().unwrap().node_id()
        );
    }
}

//! Bounded, versioned NMP/1 encoding. Authentication belongs to transport.
use neuromesh_core::identity::NodeId;
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    io::{self, Write},
    net::SocketAddr,
};

/// Current wire version; mismatches fail closed.
pub const VERSION: u16 = 1;
/// Encoded envelope byte limit, excluding the four-byte stream length prefix.
pub const MAX_FRAME: usize = 64 * 1024;
/// Maximum peer hints in one response.
pub const MAX_HINTS: usize = 64;

/// Untrusted discovery hint, not authorization.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PeerHint {
    /// Claimed peer identity.
    pub node: NodeId,
    /// Advertised QUIC socket.
    pub address: SocketAddr,
}
/// Allowed control messages. Future task/routing messages require explicit version work.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "type", content = "body", deny_unknown_fields)]
pub enum Message {
    /// Initial application hello, after transport authentication.
    Hello {
        /// Human-readable implementation label, at most 64 UTF-8 bytes.
        agent: String,
    },
    /// Liveness request; nonce echoed exactly.
    Ping {
        /// Probe correlation token.
        nonce: u64,
    },
    /// Response to a probe.
    Pong {
        /// Original probe token.
        nonce: u64,
    },
    /// Request peer discovery hints.
    GetPeers,
    /// Bounded response of untrusted hints.
    Peers {
        /// At most MAX_HINTS peer advertisements.
        peers: Vec<PeerHint>,
    },
    /// Bounded protocol error; no internal secret-bearing details.
    Error {
        /// Stable error category.
        code: u16,
        /// Sanitized explanation, at most 256 UTF-8 bytes.
        detail: String,
    },
}
/// One message. Sender must match the authenticated transport identity.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Envelope {
    /// NMP wire version.
    pub version: u16,
    /// Per-session monotonically increasing correlation identifier.
    pub message_id: u64,
    /// Claimed sender, checked by transport.
    pub sender: NodeId,
    /// Typed payload.
    pub message: Message,
}
/// Rejected input category; raw payloads are never embedded in errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProtocolError {
    /// Frame exceeds hard byte limit or is empty.
    FrameSize,
    /// Malformed encoding, duplicate/unknown fields, or invalid shape.
    Encoding,
    /// Unsupported protocol version.
    Version,
    /// Invalid message identifier or payload bound.
    Validation,
}
impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "NMP/1 rejected message: {self:?}")
    }
}
impl std::error::Error for ProtocolError {}
impl Envelope {
    /// Construct a versioned message. Encoding still validates payload bounds.
    pub fn new(sender: NodeId, message_id: u64, message: Message) -> Self {
        Self {
            version: VERSION,
            sender,
            message_id,
            message,
        }
    }
    /// Check semantic bounds without network I/O.
    pub fn validate(&self) -> Result<(), ProtocolError> {
        if self.version != VERSION {
            return Err(ProtocolError::Version);
        }
        if self.message_id == 0 {
            return Err(ProtocolError::Validation);
        }
        let valid = match &self.message {
            Message::Hello { agent } => {
                !agent.is_empty() && agent.len() <= 64 && !agent.chars().any(char::is_control)
            }
            Message::Peers { peers } => {
                peers.len() <= MAX_HINTS
                    && peers.iter().all(|p| {
                        p.address.port() != 0
                            && !p.address.ip().is_unspecified()
                            && !p.address.ip().is_multicast()
                    })
            }
            Message::Error { detail, .. } => {
                detail.len() <= 256 && !detail.chars().any(char::is_control)
            }
            _ => true,
        };
        if valid {
            Ok(())
        } else {
            Err(ProtocolError::Validation)
        }
    }
}
struct Bounded(Vec<u8>);
impl Write for Bounded {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.len() > MAX_FRAME - self.0.len() {
            return Err(io::Error::other("frame limit"));
        }
        self.0.extend_from_slice(buf);
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
/// Encode without allowing an unbounded serialization allocation.
pub fn encode(envelope: &Envelope) -> Result<Vec<u8>, ProtocolError> {
    envelope.validate()?;
    let mut output = Bounded(Vec::new());
    serde_json::to_writer(&mut output, envelope).map_err(|_| ProtocolError::FrameSize)?;
    Ok(output.0)
}
/// Reject oversized bytes before parsing; Serde's recursion limit remains enabled.
pub fn decode(bytes: &[u8]) -> Result<Envelope, ProtocolError> {
    if bytes.is_empty() || bytes.len() > MAX_FRAME {
        return Err(ProtocolError::FrameSize);
    }
    let envelope: Envelope = serde_json::from_slice(bytes).map_err(|_| ProtocolError::Encoding)?;
    envelope.validate()?;
    Ok(envelope)
}
#[cfg(test)]
mod tests {
    use super::*;
    use neuromesh_core::identity::Identity;
    fn sender() -> NodeId {
        Identity::from_seed(&[1; 32]).node_id()
    }
    #[test]
    fn all_control_messages_round_trip() {
        for message in [
            Message::Hello {
                agent: "neuromesh".into(),
            },
            Message::Ping { nonce: 42 },
            Message::Pong { nonce: 42 },
            Message::GetPeers,
            Message::Peers {
                peers: vec![PeerHint {
                    node: sender(),
                    address: "127.0.0.1:4433".parse().unwrap(),
                }],
            },
            Message::Error {
                code: 1,
                detail: "unsupported".into(),
            },
        ] {
            let envelope = Envelope::new(sender(), 1, message);
            assert_eq!(decode(&encode(&envelope).unwrap()).unwrap(), envelope);
        }
    }
    #[test]
    fn rejects_versions_ids_and_payload_limits() {
        let mut e = Envelope::new(sender(), 1, Message::Ping { nonce: 0 });
        e.version = 2;
        assert_eq!(encode(&e), Err(ProtocolError::Version));
        e.version = 1;
        e.message_id = 0;
        assert_eq!(encode(&e), Err(ProtocolError::Validation));
        e.message_id = 1;
        for message in [
            Message::Hello {
                agent: "x".repeat(65),
            },
            Message::Error {
                code: 1,
                detail: "secret\nlog injection".into(),
            },
            Message::Peers {
                peers: vec![PeerHint {
                    node: sender(),
                    address: "0.0.0.0:0".parse().unwrap(),
                }],
            },
        ] {
            e.message = message;
            assert_eq!(encode(&e), Err(ProtocolError::Validation));
        }
    }
    #[test]
    fn rejects_malformed_duplicate_unknown_and_oversized_input() {
        assert_eq!(
            decode(&vec![b' '; MAX_FRAME + 1]),
            Err(ProtocolError::FrameSize)
        );
        assert_eq!(decode(b"{"), Err(ProtocolError::Encoding));
        let valid =
            String::from_utf8(encode(&Envelope::new(sender(), 1, Message::GetPeers)).unwrap())
                .unwrap();
        for prefix in ["\"extra\":1,", "\"version\":1,"] {
            let invalid = format!("{{{prefix}{}", &valid[1..]);
            assert_eq!(decode(invalid.as_bytes()), Err(ProtocolError::Encoding));
        }
    }
    #[test]
    fn bounded_writer_refuses_to_grow_past_limit() {
        let mut w = Bounded(vec![0; MAX_FRAME]);
        assert!(w.write_all(&[1]).is_err());
        assert_eq!(w.0.len(), MAX_FRAME);
    }
}

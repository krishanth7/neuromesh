//! Bounded QUIC sessions with mutual TLS and session-bound Ed25519 identity proofs.
use ed25519_dalek::{Signature, VerifyingKey};
use neuromesh_core::identity::{Identity, NodeId};
use neuromesh_protocol::{decode, encode, Envelope, MAX_FRAME};
use quinn::{
    crypto::rustls::{QuicClientConfig, QuicServerConfig},
    Connection, Endpoint, RecvStream, SendStream,
};
use rustls::{
    pki_types::{CertificateDer, PrivateKeyDer},
    RootCertStore,
};
use std::{
    collections::BTreeSet,
    error::Error,
    net::SocketAddr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// Transport errors are propagated without logging raw frames or key material.
pub type TransportError = Box<dyn Error + Send + Sync>;
/// Deadline for TLS, identity proof, or an application exchange.
pub const DEADLINE: Duration = Duration::from_secs(5);
fn invalid(reason: &'static str) -> TransportError {
    std::io::Error::new(std::io::ErrorKind::InvalidData, reason).into()
}

/// TLS credentials. The private key intentionally has no debug/serialization wrapper.
pub struct Credentials {
    /// Certificate chain for this endpoint.
    pub chain: Vec<CertificateDer<'static>>,
    /// Private key matching the leaf certificate.
    pub key: PrivateKeyDer<'static>,
    /// Explicit trust roots for both client and server authentication.
    pub roots: RootCertStore,
}

/// Shared endpoint enforcing a cap that includes in-flight handshakes.
pub struct MeshEndpoint {
    endpoint: Endpoint,
    identity: Arc<Identity>,
    allowed: Arc<BTreeSet<NodeId>>,
    permits: Arc<Semaphore>,
}
impl MeshEndpoint {
    /// Bind QUIC with mandatory peer certificates and an explicit identity allowlist.
    pub fn bind(
        address: SocketAddr,
        credentials: Credentials,
        identity: Identity,
        allowed: BTreeSet<NodeId>,
        max_connections: usize,
    ) -> Result<Self, TransportError> {
        if !(1..=neuromesh_core::MAX_PEERS).contains(&max_connections) {
            return Err(invalid("invalid connection cap"));
        }
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let verifier = rustls::server::WebPkiClientVerifier::builder_with_provider(
            Arc::new(credentials.roots.clone()),
            provider.clone(),
        )
        .build()?;
        let mut server_tls = rustls::ServerConfig::builder_with_provider(provider.clone())
            .with_protocol_versions(&[&rustls::version::TLS13])?
            .with_client_cert_verifier(verifier)
            .with_single_cert(credentials.chain.clone(), credentials.key.clone_key())?;
        server_tls.alpn_protocols = vec![b"nmp/1".to_vec()];
        server_tls.max_early_data_size = 0;
        let mut client_tls = rustls::ClientConfig::builder_with_provider(provider)
            .with_protocol_versions(&[&rustls::version::TLS13])?
            .with_root_certificates(credentials.roots)
            .with_client_auth_cert(credentials.chain, credentials.key)?;
        client_tls.alpn_protocols = vec![b"nmp/1".to_vec()];
        client_tls.enable_early_data = false;
        let mut transport = quinn::TransportConfig::default();
        transport.max_concurrent_bidi_streams(1u32.into());
        transport.max_concurrent_uni_streams(0u32.into());
        transport.stream_receive_window((MAX_FRAME as u32 + 4).into());
        transport.receive_window((2 * MAX_FRAME as u32).into());
        transport.send_window((2 * MAX_FRAME) as u64);
        transport.max_idle_timeout(Some(Duration::from_secs(15).try_into()?));
        let transport = Arc::new(transport);
        let mut server =
            quinn::ServerConfig::with_crypto(Arc::new(QuicServerConfig::try_from(server_tls)?));
        server.transport_config(transport.clone());
        server.max_incoming(max_connections);
        server.incoming_buffer_size(64 * 1024);
        server.incoming_buffer_size_total((max_connections * 64 * 1024) as u64);
        let mut client =
            quinn::ClientConfig::new(Arc::new(QuicClientConfig::try_from(client_tls)?));
        client.transport_config(transport);
        let mut endpoint = Endpoint::server(server, address)?;
        endpoint.set_default_client_config(client);
        Ok(Self {
            endpoint,
            identity: Arc::new(identity),
            allowed: Arc::new(allowed),
            permits: Arc::new(Semaphore::new(max_connections)),
        })
    }
    /// Bound UDP address, including an OS-selected port when zero was requested.
    pub fn local_addr(&self) -> Result<SocketAddr, TransportError> {
        Ok(self.endpoint.local_addr()?)
    }
    /// Local application identity.
    pub fn node_id(&self) -> NodeId {
        self.identity.node_id()
    }
    /// Connect using certificate hostname verification, then verify the expected identity.
    pub async fn connect(
        &self,
        address: SocketAddr,
        server_name: &str,
        expected: NodeId,
    ) -> Result<Session, TransportError> {
        if !self.allowed.contains(&expected) {
            return Err(invalid("identity not authorized"));
        }
        let permit = self.permits.clone().try_acquire_owned()?;
        let connection =
            tokio::time::timeout(DEADLINE, self.endpoint.connect(address, server_name)?).await??;
        match tokio::time::timeout(
            DEADLINE,
            authenticate(&connection, &self.identity, &self.allowed, true),
        )
        .await
        {
            Ok(Ok(peer)) if peer == expected => {
                Ok(Session::new(connection, self.node_id(), peer, permit, true))
            }
            _ => {
                connection.close(1u32.into(), b"identity rejected");
                Err(invalid("identity handshake failed"))
            }
        }
    }
    /// Accept one incoming connection. Saturated endpoints refuse incoming work.
    pub async fn accept(&self) -> Result<Session, TransportError> {
        let incoming = self
            .endpoint
            .accept()
            .await
            .ok_or_else(|| invalid("endpoint closed"))?;
        let permit = match self.permits.clone().try_acquire_owned() {
            Ok(p) => p,
            Err(_) => {
                incoming.refuse();
                return Err(invalid("connection limit"));
            }
        };
        let connection = tokio::time::timeout(DEADLINE, incoming).await??;
        match tokio::time::timeout(
            DEADLINE,
            authenticate(&connection, &self.identity, &self.allowed, false),
        )
        .await
        {
            Ok(Ok(peer)) => Ok(Session::new(
                connection,
                self.node_id(),
                peer,
                permit,
                false,
            )),
            _ => {
                connection.close(1u32.into(), b"identity rejected");
                Err(invalid("identity handshake failed"))
            }
        }
    }
    /// Close every connection and stop accepting work.
    pub fn close(&self) {
        self.endpoint.close(0u32.into(), b"shutdown");
    }
}

fn proof(identity: &Identity, binding: &[u8; 32], role: u8) -> [u8; 96] {
    let mut message = Vec::from(b"NeuroMesh identity proof v1".as_slice());
    message.push(role);
    message.extend_from_slice(binding);
    let mut output = [0; 96];
    output[..32].copy_from_slice(identity.public_key().as_bytes());
    output[32..].copy_from_slice(&identity.sign(&message).to_bytes());
    output
}
fn verify_proof(
    bytes: &[u8; 96],
    binding: &[u8; 32],
    role: u8,
    allowed: &BTreeSet<NodeId>,
) -> Result<NodeId, TransportError> {
    let key = VerifyingKey::from_bytes(bytes[..32].try_into()?)
        .map_err(|_| invalid("invalid public key"))?;
    let node = NodeId::from_public_key(&key);
    if !allowed.contains(&node) {
        return Err(invalid("identity not authorized"));
    }
    let mut message = Vec::from(b"NeuroMesh identity proof v1".as_slice());
    message.push(role);
    message.extend_from_slice(binding);
    Identity::verify(
        &key,
        &message,
        &Signature::from_bytes(bytes[32..].try_into()?),
    )
    .map_err(|_| invalid("invalid identity proof"))?;
    Ok(node)
}
async fn authenticate(
    connection: &Connection,
    identity: &Identity,
    allowed: &BTreeSet<NodeId>,
    initiator: bool,
) -> Result<NodeId, TransportError> {
    let mut binding = [0; 32];
    connection
        .export_keying_material(&mut binding, b"EXPORTER-NeuroMesh-identity-v1", b"nmp/1")
        .map_err(|_| invalid("TLS exporter failed"))?;
    let (mut send, mut recv) = if initiator {
        connection.open_bi().await?
    } else {
        connection.accept_bi().await?
    };
    if initiator {
        send.write_all(&proof(identity, &binding, 0)).await?;
        send.finish()?;
    }
    let mut remote = [0; 96];
    recv.read_exact(&mut remote).await?;
    let mut extra = [0; 1];
    if recv.read(&mut extra).await?.is_some() {
        return Err(invalid("extra proof bytes"));
    }
    let peer = verify_proof(&remote, &binding, if initiator { 1 } else { 0 }, allowed)?;
    if !initiator {
        send.write_all(&proof(identity, &binding, 1)).await?;
        send.finish()?;
    }
    Ok(peer)
}

/// Condition under which a session's connection closed, without transport internals.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CloseReport {
    /// Application close code. From the peer when `local` is `false`, otherwise
    /// the code this session passed to [`Session::close`] if it closed first.
    pub code: u32,
    /// Application close reason bytes; empty for non-application closures.
    pub reason: Vec<u8>,
    /// Whether the closure originated locally instead of from the peer.
    pub local: bool,
}

/// Validate an application close reason without emitting it in errors.
fn checked_reason(reason: &str) -> Result<&[u8], TransportError> {
    if reason.len() > 127 {
        return Err(invalid("close reason exceeds 127 bytes"));
    }
    Ok(reason.as_bytes())
}

/// One authenticated session; request operations are serialized for replay ordering.
/// Dropping it closes the connection and releases its endpoint capacity permit.
pub struct Session {
    connection: Connection,
    local: NodeId,
    peer: NodeId,
    sequence: AtomicU64,
    operation: Semaphore,
    _permit: OwnedSemaphorePermit,
    initiator: bool,
    local_close: Mutex<Option<(u32, Vec<u8>)>>,
}
impl Session {
    fn new(
        connection: Connection,
        local: NodeId,
        peer: NodeId,
        permit: OwnedSemaphorePermit,
        initiator: bool,
    ) -> Self {
        Self {
            connection,
            local,
            peer,
            sequence: AtomicU64::new(0),
            operation: Semaphore::new(1),
            _permit: permit,
            initiator,
            local_close: Mutex::new(None),
        }
    }
    /// Authenticated remote identity.
    pub fn peer(&self) -> NodeId {
        self.peer
    }
    /// Close this connection explicitly with an application error code and reason.
    ///
    /// The peer observes the pair through [`Session::wait_closed`]. Application
    /// close reasons are bounded to 127 UTF-8 bytes so one datagram carries the
    /// whole notification; longer reasons are rejected without touching the
    /// connection, and the reason bytes are never logged by this crate.
    pub fn close(&self, code: u32, reason: &str) -> Result<(), TransportError> {
        let reason = checked_reason(reason)?;
        *self
            .local_close
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some((code, reason.to_vec()));
        self.connection.close(code.into(), reason);
        Ok(())
    }
    /// Whether close has begun at either end (local call, peer notice, or timeout).
    pub fn is_closed(&self) -> bool {
        self.connection.close_reason().is_some()
    }
    /// Wait for the connection to close and report what both ends agreed on.
    ///
    /// An application close from the peer yields its code and reason with
    /// `local == false`. A local [`Session::close`] yields the code and reason
    /// this session sent, with `local == true`. Transport-level closures
    /// (timeout, reset) report code 0, an empty reason, and `local == false`;
    /// transport-internal details are never exposed.
    pub async fn wait_closed(&self) -> CloseReport {
        let error = self.connection.closed().await;
        match error {
            quinn::ConnectionError::ApplicationClosed(close) => CloseReport {
                code: u32::try_from(close.error_code.into_inner()).unwrap_or(u32::MAX),
                reason: close.reason.to_vec(),
                local: false,
            },
            quinn::ConnectionError::LocallyClosed => {
                let stored = self
                    .local_close
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                match stored.as_ref() {
                    Some((code, reason)) => CloseReport {
                        code: *code,
                        reason: reason.clone(),
                        local: true,
                    },
                    None => CloseReport {
                        code: 0,
                        reason: Vec::new(),
                        local: true,
                    },
                }
            }
            _ => CloseReport {
                code: 0,
                reason: Vec::new(),
                local: false,
            },
        }
    }
    /// Send a message and receive its matching response; only connection initiators may call.
    pub async fn request(
        &self,
        message: neuromesh_protocol::Message,
    ) -> Result<Envelope, TransportError> {
        if !self.initiator {
            return Err(invalid("responder cannot initiate requests"));
        }
        let _operation = self.operation.try_acquire()?;
        let sequence = self.sequence.load(Ordering::Relaxed);
        let next = sequence
            .checked_add(1)
            .ok_or_else(|| invalid("sequence exhausted"))?;
        self.sequence.store(next, Ordering::Relaxed);
        let request = Envelope::new(self.local, next, message);
        let result = tokio::time::timeout(DEADLINE, async {
            let (mut send, mut recv) = self.connection.open_bi().await?;
            write_frame(&mut send, &request).await?;
            let response = read_frame(&mut recv).await?;
            if response.sender != self.peer || response.message_id != request.message_id {
                return Err(invalid("response identity or ID mismatch"));
            }
            Ok(response)
        })
        .await;
        match result {
            Ok(Ok(response)) => Ok(response),
            _ => {
                self.connection.close(2u32.into(), b"request failed");
                Err(invalid("request failed or timed out"))
            }
        }
    }
    /// Handle one request; handler must be quick and nonblocking. Only responders may call.
    pub async fn respond<F>(&self, handler: F) -> Result<(), TransportError>
    where
        F: FnOnce(neuromesh_protocol::Message) -> neuromesh_protocol::Message,
    {
        if self.initiator {
            return Err(invalid("initiator cannot accept requests"));
        }
        let _operation = self.operation.try_acquire()?;
        let sequence = self.sequence.load(Ordering::Relaxed);
        let result = tokio::time::timeout(DEADLINE, async {
            let (mut send, mut recv) = self.connection.accept_bi().await?;
            let request = read_frame(&mut recv).await?;
            if request.sender != self.peer || request.message_id <= sequence {
                return Err(invalid("sender mismatch or replay"));
            }
            self.sequence.store(request.message_id, Ordering::Relaxed);
            let response = Envelope::new(self.local, request.message_id, handler(request.message));
            write_frame(&mut send, &response).await
        })
        .await;
        match result {
            Ok(Ok(())) => Ok(()),
            _ => {
                self.connection.close(2u32.into(), b"request rejected");
                Err(invalid("request rejected or timed out"))
            }
        }
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        self.connection.close(0u32.into(), b"session dropped");
    }
}
async fn write_frame(send: &mut SendStream, envelope: &Envelope) -> Result<(), TransportError> {
    let bytes = encode(envelope)?;
    send.write_all(&(bytes.len() as u32).to_be_bytes()).await?;
    send.write_all(&bytes).await?;
    send.finish()?;
    if send.stopped().await?.is_some() {
        return Err(invalid("peer stopped response"));
    }
    Ok(())
}
async fn read_frame(recv: &mut RecvStream) -> Result<Envelope, TransportError> {
    let mut header = [0; 4];
    recv.read_exact(&mut header).await?;
    let len = u32::from_be_bytes(header) as usize;
    if len == 0 || len > MAX_FRAME {
        return Err(invalid("frame limit"));
    }
    let mut bytes = vec![0; len];
    recv.read_exact(&mut bytes).await?;
    let mut extra = [0; 1];
    if recv.read(&mut extra).await?.is_some() {
        return Err(invalid("trailing frame bytes"));
    }
    Ok(decode(&bytes)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn close_reason_boundaries_accept_short_and_multibyte_reject_long() {
        assert!(checked_reason("").is_ok());
        assert!(checked_reason("draining for maintenance").is_ok());
        // 31 four-byte characters = 124 bytes, inside the bound.
        assert!(checked_reason("\u{1f600}".repeat(31).as_str()).is_ok());
        // 32 four-byte characters = 128 bytes, over the bound.
        assert!(checked_reason("\u{1f600}".repeat(32).as_str()).is_err());
        assert!(checked_reason(&"x".repeat(127)).is_ok());
        assert!(checked_reason(&"x".repeat(128)).is_err());
        let error = checked_reason(&"x".repeat(200)).unwrap_err().to_string();
        // The error must carry only the static bound, never reason bytes.
        assert_eq!(error, "close reason exceeds 127 bytes");
    }
    #[test]
    fn proofs_reject_other_sessions_roles_and_untrusted_keys() {
        let identity = Identity::from_seed(&[7; 32]);
        let allowed = BTreeSet::from([identity.node_id()]);
        let bytes = proof(&identity, &[1; 32], 0);
        assert_eq!(
            verify_proof(&bytes, &[1; 32], 0, &allowed).unwrap(),
            identity.node_id()
        );
        assert!(verify_proof(&bytes, &[2; 32], 0, &allowed).is_err());
        assert!(verify_proof(&bytes, &[1; 32], 1, &allowed).is_err());
        assert!(verify_proof(&bytes, &[1; 32], 0, &BTreeSet::new()).is_err());
    }
}

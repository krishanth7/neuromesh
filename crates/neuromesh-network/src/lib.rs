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
        Arc,
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

/// Point-in-time counters for one endpoint.
///
/// Counting is advisory telemetry, not an audit log: increments use relaxed
/// atomic ordering and each snapshot is an independent copy, so counters are
/// cheap enough to scrape on every metrics collection. No counter touches key
/// material, frame contents, or peer addresses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EndpointMetrics {
    /// Dial attempts that passed the identity allowlist check.
    pub outbound_attempts: u64,
    /// Dials that completed mutual authentication and produced a session.
    pub outbound_connected: u64,
    /// Inbound connections that completed mutual authentication.
    pub inbound_accepted: u64,
    /// Dial failures: endpoint state, certificate or hostname mismatch, or deadline.
    pub connect_failed: u64,
    /// Inbound connections refused before handshake because the cap was reached.
    pub refused_capacity: u64,
    /// Handshakes rejected by deadline, certificate material, proof, allowlist, or identity mismatch.
    pub handshake_rejected: u64,
    /// Request/response exchanges completed successfully from either role's perspective.
    pub exchanges_succeeded: u64,
    /// Exchanges rejected or timed out from either role's perspective.
    pub exchanges_failed: u64,
    /// Explicit [`MeshEndpoint::close`] invocations on this endpoint.
    pub local_closes: u64,
}

/// Shared atomic counters behind an [`EndpointMetrics`] snapshot.
#[derive(Debug, Default)]
struct Counters {
    outbound_attempts: AtomicU64,
    outbound_connected: AtomicU64,
    inbound_accepted: AtomicU64,
    connect_failed: AtomicU64,
    refused_capacity: AtomicU64,
    handshake_rejected: AtomicU64,
    exchanges_succeeded: AtomicU64,
    exchanges_failed: AtomicU64,
    local_closes: AtomicU64,
}

impl Counters {
    fn incremented(counter: &AtomicU64) {
        counter.fetch_add(1, Ordering::Relaxed);
    }
    fn snapshot(&self) -> EndpointMetrics {
        let read = |counter: &AtomicU64| counter.load(Ordering::Relaxed);
        EndpointMetrics {
            outbound_attempts: read(&self.outbound_attempts),
            outbound_connected: read(&self.outbound_connected),
            inbound_accepted: read(&self.inbound_accepted),
            connect_failed: read(&self.connect_failed),
            refused_capacity: read(&self.refused_capacity),
            handshake_rejected: read(&self.handshake_rejected),
            exchanges_succeeded: read(&self.exchanges_succeeded),
            exchanges_failed: read(&self.exchanges_failed),
            local_closes: read(&self.local_closes),
        }
    }
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
    counters: Arc<Counters>,
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
            counters: Arc::new(Counters::default()),
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
        Counters::incremented(&self.counters.outbound_attempts);
        let permit = match self.permits.clone().try_acquire_owned() {
            Ok(permit) => permit,
            Err(error) => {
                Counters::incremented(&self.counters.connect_failed);
                return Err(error.into());
            }
        };
        let connecting = match self.endpoint.connect(address, server_name) {
            Ok(connecting) => connecting,
            Err(error) => {
                Counters::incremented(&self.counters.connect_failed);
                return Err(error.into());
            }
        };
        let connection = match tokio::time::timeout(DEADLINE, connecting).await {
            Ok(Ok(connection)) => connection,
            Ok(Err(error)) => {
                Counters::incremented(&self.counters.connect_failed);
                return Err(error.into());
            }
            Err(error) => {
                Counters::incremented(&self.counters.connect_failed);
                return Err(error.into());
            }
        };
        match tokio::time::timeout(
            DEADLINE,
            authenticate(&connection, &self.identity, &self.allowed, true),
        )
        .await
        {
            Ok(Ok(peer)) if peer == expected => {
                Counters::incremented(&self.counters.outbound_connected);
                Ok(Session::new(
                    connection,
                    self.node_id(),
                    peer,
                    permit,
                    true,
                    self.counters.clone(),
                ))
            }
            _ => {
                Counters::incremented(&self.counters.handshake_rejected);
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
                Counters::incremented(&self.counters.refused_capacity);
                incoming.refuse();
                return Err(invalid("connection limit"));
            }
        };
        let connection = match tokio::time::timeout(DEADLINE, incoming).await {
            Ok(Ok(connection)) => connection,
            Ok(Err(error)) => {
                Counters::incremented(&self.counters.handshake_rejected);
                return Err(error.into());
            }
            Err(error) => {
                Counters::incremented(&self.counters.handshake_rejected);
                return Err(error.into());
            }
        };
        match tokio::time::timeout(
            DEADLINE,
            authenticate(&connection, &self.identity, &self.allowed, false),
        )
        .await
        {
            Ok(Ok(peer)) => {
                Counters::incremented(&self.counters.inbound_accepted);
                Ok(Session::new(
                    connection,
                    self.node_id(),
                    peer,
                    permit,
                    false,
                    self.counters.clone(),
                ))
            }
            _ => {
                Counters::incremented(&self.counters.handshake_rejected);
                connection.close(1u32.into(), b"identity rejected");
                Err(invalid("identity handshake failed"))
            }
        }
    }
    /// Close every connection and stop accepting work.
    pub fn close(&self) {
        Counters::incremented(&self.counters.local_closes);
        self.endpoint.close(0u32.into(), b"shutdown");
    }
    /// Independent copy of this endpoint's counters for metrics collection.
    pub fn metrics(&self) -> EndpointMetrics {
        self.counters.snapshot()
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
    counters: Arc<Counters>,
}
impl Session {
    fn new(
        connection: Connection,
        local: NodeId,
        peer: NodeId,
        permit: OwnedSemaphorePermit,
        initiator: bool,
        counters: Arc<Counters>,
    ) -> Self {
        Self {
            connection,
            local,
            peer,
            sequence: AtomicU64::new(0),
            operation: Semaphore::new(1),
            _permit: permit,
            initiator,
            counters,
        }
    }
    /// Authenticated remote identity.
    pub fn peer(&self) -> NodeId {
        self.peer
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
            Ok(Ok(response)) => {
                Counters::incremented(&self.counters.exchanges_succeeded);
                Ok(response)
            }
            _ => {
                Counters::incremented(&self.counters.exchanges_failed);
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
            Ok(Ok(())) => {
                Counters::incremented(&self.counters.exchanges_succeeded);
                Ok(())
            }
            _ => {
                Counters::incremented(&self.counters.exchanges_failed);
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
    fn metrics_snapshots_are_stable_copies_that_default_to_zero() {
        assert_eq!(
            EndpointMetrics::default(),
            EndpointMetrics {
                ..Default::default()
            }
        );
        let counters = Counters::default();
        Counters::incremented(&counters.outbound_attempts);
        Counters::incremented(&counters.exchanges_succeeded);
        Counters::incremented(&counters.exchanges_succeeded);
        let snapshot = counters.snapshot();
        assert_eq!(
            snapshot,
            EndpointMetrics {
                outbound_attempts: 1,
                exchanges_succeeded: 2,
                ..EndpointMetrics::default()
            }
        );
        Counters::incremented(&counters.exchanges_succeeded);
        assert_eq!(snapshot.exchanges_succeeded, 2);
        assert_eq!(counters.snapshot().exchanges_succeeded, 3);
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

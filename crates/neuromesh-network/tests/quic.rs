//! Real localhost QUIC integration tests with ephemeral test-only certificates.
use neuromesh_core::identity::Identity;
use neuromesh_network::{Credentials, MeshEndpoint};
use neuromesh_protocol::Message;
use rcgen::{BasicConstraints, CertificateParams, IsCa, KeyPair};
use rustls::{pki_types::PrivatePkcs8KeyDer, RootCertStore};
use std::{collections::BTreeSet, sync::Arc, time::Duration};

fn credentials_pair() -> (Credentials, Credentials) {
    let mut params = CertificateParams::new(vec!["mesh.test".into()]).unwrap();
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    let ca_key = KeyPair::generate().unwrap();
    let ca = params.self_signed(&ca_key).unwrap();
    let make = || {
        let key = KeyPair::generate().unwrap();
        let params = CertificateParams::new(vec!["mesh.test".into()]).unwrap();
        let cert = params.signed_by(&key, &ca, &ca_key).unwrap();
        let mut roots = RootCertStore::empty();
        roots.add(ca.der().clone()).unwrap();
        Credentials {
            chain: vec![cert.der().clone()],
            key: PrivatePkcs8KeyDer::from(key.serialize_der()).into(),
            roots,
        }
    };
    (make(), make())
}
fn endpoints(untrusted_cert: bool) -> (Arc<MeshEndpoint>, Arc<MeshEndpoint>) {
    let (a_tls, mut b_tls) = credentials_pair();
    if untrusted_cert {
        b_tls = credentials_pair().0;
    }
    let a = Identity::from_seed(&[1; 32]);
    let b = Identity::from_seed(&[2; 32]);
    let a_id = a.node_id();
    let b_id = b.node_id();
    let a = MeshEndpoint::bind(
        "127.0.0.1:0".parse().unwrap(),
        a_tls,
        a,
        BTreeSet::from([b_id]),
        2,
    )
    .unwrap();
    let b = MeshEndpoint::bind(
        "127.0.0.1:0".parse().unwrap(),
        b_tls,
        b,
        BTreeSet::from([a_id]),
        2,
    )
    .unwrap();
    (Arc::new(a), Arc::new(b))
}
#[tokio::test]
async fn real_quic_identity_ping_and_graceful_close() {
    let (a, b) = endpoints(false);
    let server = b.clone();
    let a_id = a.node_id();
    let (done_tx, done_rx) = tokio::sync::oneshot::channel();
    let worker = tokio::spawn(async move {
        let session = server.accept().await.unwrap();
        assert_eq!(session.peer(), a_id);
        for _ in 0..3 {
            session
                .respond(|message| match message {
                    Message::Ping { nonce } => Message::Pong { nonce },
                    _ => Message::Error {
                        code: 1,
                        detail: "unexpected".into(),
                    },
                })
                .await
                .unwrap();
        }
        done_rx.await.unwrap();
    });
    let session = a
        .connect(b.local_addr().unwrap(), "mesh.test", b.node_id())
        .await
        .unwrap();
    for nonce in [11, 22, 33] {
        let response = session.request(Message::Ping { nonce }).await.unwrap();
        assert_eq!(response.message, Message::Pong { nonce });
        assert_eq!(response.sender, b.node_id());
    }
    done_tx.send(()).unwrap();
    worker.await.unwrap();
    drop(session);
    a.close();
    b.close();
    assert!(tokio::time::timeout(Duration::from_secs(1), b.accept())
        .await
        .unwrap()
        .is_err());
}
#[tokio::test]
async fn untrusted_certificate_is_rejected() {
    let (a, b) = endpoints(true);
    let server = b.clone();
    let worker = tokio::spawn(async move { server.accept().await });
    assert!(a
        .connect(b.local_addr().unwrap(), "mesh.test", b.node_id())
        .await
        .is_err());
    assert!(worker.await.unwrap().is_err());
    a.close();
    b.close();
}
#[tokio::test]
async fn wrong_hostname_and_unlisted_identity_are_rejected() {
    let (a, b) = endpoints(false);
    assert!(a
        .connect(
            b.local_addr().unwrap(),
            "mesh.test",
            Identity::from_seed(&[3; 32]).node_id()
        )
        .await
        .is_err());
    let server = b.clone();
    let worker = tokio::spawn(async move { server.accept().await });
    assert!(a
        .connect(b.local_addr().unwrap(), "wrong.test", b.node_id())
        .await
        .is_err());
    assert!(worker.await.unwrap().is_err());
    a.close();
    b.close();
}

#[tokio::test]
async fn connection_cap_releases_when_session_drops() {
    let (a, b) = endpoints(false);
    let server = b.clone();
    let (ready_tx, ready_rx) = tokio::sync::oneshot::channel();
    let (done_tx, done_rx) = tokio::sync::oneshot::channel();
    let worker = tokio::spawn(async move {
        let first = server.accept().await.unwrap();
        let second = server.accept().await.unwrap();
        ready_tx.send(()).unwrap();
        done_rx.await.unwrap();
        drop((first, second));
    });
    let first = a
        .connect(b.local_addr().unwrap(), "mesh.test", b.node_id())
        .await
        .unwrap();
    let second = a
        .connect(b.local_addr().unwrap(), "mesh.test", b.node_id())
        .await
        .unwrap();
    ready_rx.await.unwrap();
    assert!(a
        .connect(b.local_addr().unwrap(), "mesh.test", b.node_id())
        .await
        .is_err());
    drop((first, second));
    done_tx.send(()).unwrap();
    worker.await.unwrap();
    let server = b.clone();
    let (done_tx, done_rx) = tokio::sync::oneshot::channel();
    let worker = tokio::spawn(async move {
        let session = server.accept().await.unwrap();
        done_rx.await.unwrap();
        drop(session);
    });
    let session = a
        .connect(b.local_addr().unwrap(), "mesh.test", b.node_id())
        .await
        .unwrap();
    done_tx.send(()).unwrap();
    worker.await.unwrap();
    drop(session);
    a.close();
    b.close();
}

#[tokio::test]
async fn allowlisted_but_wrong_expected_identity_is_rejected() {
    let (a_tls, b_tls) = credentials_pair();
    let a_key = Identity::from_seed(&[1; 32]);
    let b_key = Identity::from_seed(&[2; 32]);
    let wrong = Identity::from_seed(&[3; 32]).node_id();
    let a_id = a_key.node_id();
    let b_id = b_key.node_id();
    let a = MeshEndpoint::bind(
        "127.0.0.1:0".parse().unwrap(),
        a_tls,
        a_key,
        BTreeSet::from([b_id, wrong]),
        2,
    )
    .unwrap();
    let b = Arc::new(
        MeshEndpoint::bind(
            "127.0.0.1:0".parse().unwrap(),
            b_tls,
            b_key,
            BTreeSet::from([a_id]),
            2,
        )
        .unwrap(),
    );
    let server = b.clone();
    let (done_tx, done_rx) = tokio::sync::oneshot::channel();
    let worker = tokio::spawn(async move {
        let session = server.accept().await.unwrap();
        let _ = done_rx.await;
        drop(session);
    });
    assert!(a
        .connect(b.local_addr().unwrap(), "mesh.test", wrong)
        .await
        .is_err());
    let _ = done_tx.send(());
    worker.await.unwrap();
    a.close();
    b.close();
}

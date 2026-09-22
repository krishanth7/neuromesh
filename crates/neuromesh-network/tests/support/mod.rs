//! Ephemeral test-only credentials; never deploy these identities.
use neuromesh_core::identity::Identity;
use neuromesh_network::{Credentials, MeshEndpoint};
use rcgen::{BasicConstraints, CertificateParams, IsCa, KeyPair};
use rustls::{pki_types::PrivatePkcs8KeyDer, RootCertStore};
use std::{collections::BTreeSet, sync::Arc};

pub fn credentials_pair() -> (Credentials, Credentials) {
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
pub fn endpoints(untrusted_cert: bool) -> (Arc<MeshEndpoint>, Arc<MeshEndpoint>) {
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

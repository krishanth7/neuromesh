//! Localhost transport measurement with authenticated peers and raw RTT samples.
//! Test-only identities and ephemeral certificates; not a deployment example.
#[path = "../tests/support/mod.rs"]
mod support;
use neuromesh_core::peers::{Health, PeerRegistry};
use neuromesh_protocol::Message;
use std::time::{Duration, Instant};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    const SAMPLES: u64 = 100;
    const WARMUP: u64 = 10;
    let (a, b) = support::endpoints(false);
    let server = b.clone();
    let (done_tx, done_rx) = tokio::sync::oneshot::channel();
    let worker = tokio::spawn(async move {
        let session = server.accept().await?;
        for _ in 0..SAMPLES + WARMUP {
            session
                .respond(|message| match message {
                    Message::Ping { nonce } => Message::Pong { nonce },
                    _ => Message::Error {
                        code: 1,
                        detail: "expected ping".into(),
                    },
                })
                .await?;
        }
        let _ = done_rx.await;
        Ok::<(), neuromesh_network::TransportError>(())
    });
    let start = Instant::now();
    let session = a.connect(b.local_addr()?, "mesh.test", b.node_id()).await?;
    let handshake_us = start.elapsed().as_micros();
    let mut peers = PeerRegistry::new(
        a.node_id(),
        2,
        Duration::from_secs(2),
        Duration::from_secs(5),
        Instant::now(),
    )?;
    peers.register(session.peer(), Instant::now())?;
    let mut samples = Vec::new();
    for nonce in 0..SAMPLES + WARMUP {
        let started = Instant::now();
        let response = session.request(Message::Ping { nonce }).await?;
        let elapsed = started.elapsed().as_micros();
        if response.message != (Message::Pong { nonce }) {
            return Err("incorrect pong".into());
        }
        peers.observe(response.sender, Instant::now())?;
        if nonce >= WARMUP {
            samples.push(elapsed);
        }
    }
    assert_eq!(peers.get(session.peer()).unwrap().health, Health::Healthy);
    let _ = done_tx.send(());
    worker.await??;
    drop(session);
    a.close();
    b.close();
    let mut sorted = samples.clone();
    sorted.sort_unstable();
    // Nearest-rank percentiles; samples include the request/response and transport ACK wait.
    println!("{{\"transport\":\"localhost QUIC with mutual TLS and Ed25519 proof\",\"samples\":{SAMPLES},\"warmup\":{WARMUP},\"handshake_us\":{handshake_us},\"p50_us\":{},\"p95_us\":{},\"min_us\":{},\"max_us\":{},\"registered_peers\":{},\"rtt_us\":{:?}}}", sorted[49], sorted[94], sorted[0], sorted[99], peers.len(), samples);
    Ok(())
}

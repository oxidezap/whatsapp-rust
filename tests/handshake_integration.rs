//! Integration tests for the full Noise handshake orchestration in
//! `whatsapp_rust::handshake::do_handshake`.
//!
//! Each test stands up an in-process Noise responder, drives the client
//! through one or more handshakes, and asserts on the outcome plus any
//! state mutations visible via the `PersistenceManager` (cached cert
//! chain) and the per-process IK failure counter.
//!
//! These tests do NOT depend on the external mock server (bartender) —
//! the responder side is implemented inline using the same primitives
//! that `wacore-noise` exports for unit-test use.
//!
//! Coverage:
//!   - bypass XX succeeds without populating the trusted cert cache, and a
//!     later strict connect on the same backend still picks IK from a seeded
//!     cache (per-client policy, not a global flag)
//!   - strict XX rejects the zero-signed fixture
//!   - server rejects IK → client pivots to XX-fallback, which enforces cert
//!     verification on the fresh chain
//!   - IK with a stale cached static key → counter incremented, cache
//!     cleared, next connect can fall back to XX cleanly
//!
//! The "IK Continue" path is asserted indirectly by checking that the
//! second connect both runs without sending three ClientHello bytes
//! (i.e. it speaks IK shape, not XX shape) AND completes successfully.

// Tests/benches exercise the raw buffa API.
#![allow(clippy::disallowed_methods)]

use async_trait::async_trait;
use buffa::Message;
use bytes::Bytes;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::AtomicU32;
use std::time::Duration;

use wacore::handshake::NoiseHandshake;
use wacore::libsignal::protocol::KeyPair;
use wacore_binary::consts::{NOISE_PATTERN_IK, NOISE_PATTERN_XX, WA_CONN_HEADER};
use wacore_noise::test_util::build_cert_chain_bytes;
use whatsapp_rust::waproto::whatsapp as wa;

use whatsapp_rust::handshake::{NoiseCertPolicy, do_handshake_with_cert_policy};
use whatsapp_rust::socket::noise_socket::SendObservers;
use whatsapp_rust::transport::{Transport, TransportEvent};

// Explicit per-handshake cert policies. The responder serves zero-signed
// fixtures, so XX flows take BYPASS while IK-selection flows take STRICT
// (IK never verifies; the policy only decides whether the cache may be
// used). Mixing both in one process proves the selection is per-handshake,
// not a global flag.
const STRICT: NoiseCertPolicy = NoiseCertPolicy::Strict;
const BYPASS: NoiseCertPolicy = NoiseCertPolicy::DangerSkipCertChainVerify;

/// In-process responder driving Noise XX or IK from the server side.
/// Holds the long-lived server identity keypair and the cert chain
/// bytes that the client must verify and cache.
struct InProcessServer {
    identity_kp: KeyPair,
    cert_chain_bytes: Vec<u8>,
}

impl InProcessServer {
    fn new() -> Self {
        let identity_kp = KeyPair::generate(&mut rand::rng());
        let server_static_pub: [u8; 32] = identity_kp
            .public_key
            .public_key_bytes()
            .try_into()
            .expect("X25519 pub key is 32 bytes");
        let cert_chain_bytes = build_cert_chain_bytes(&server_static_pub);
        Self {
            identity_kp,
            cert_chain_bytes,
        }
    }

    fn server_static_pub(&self) -> [u8; 32] {
        self.identity_kp
            .public_key
            .public_key_bytes()
            .try_into()
            .expect("X25519 pub key is 32 bytes")
    }
}

/// Strips the leading WA_CONN_HEADER (4 bytes) when present and then
/// parses one length-prefixed frame from the buffer. Returns the inner
/// payload bytes.
fn parse_first_client_frame(raw: &[u8]) -> Vec<u8> {
    // Detect optional WA_CONN_HEADER at the very front.
    let body_start = if raw.starts_with(&WA_CONN_HEADER) {
        WA_CONN_HEADER.len()
    } else {
        0
    };
    parse_one_frame(&raw[body_start..])
}

fn parse_one_frame(buf: &[u8]) -> Vec<u8> {
    assert!(buf.len() >= 3, "frame buffer must hold at least the length");
    let len = ((buf[0] as usize) << 16) | ((buf[1] as usize) << 8) | (buf[2] as usize);
    assert!(buf.len() >= 3 + len, "frame buffer truncated");
    buf[3..3 + len].to_vec()
}

/// Mock transport: capture client→server bytes into a shared Vec; the
/// test drives the server→client side directly via `events_tx`.
#[derive(Clone)]
struct CaptureTransport {
    sent: Arc<StdMutex<Vec<Bytes>>>,
}

#[async_trait]
impl Transport for CaptureTransport {
    async fn send(&self, data: Bytes) -> anyhow::Result<()> {
        self.sent.lock().unwrap().push(data);
        Ok(())
    }
    async fn disconnect(&self) {}
}

fn new_transport_pair() -> (
    Arc<CaptureTransport>,
    async_channel::Sender<TransportEvent>,
    async_channel::Receiver<TransportEvent>,
) {
    let transport = Arc::new(CaptureTransport {
        sent: Arc::new(StdMutex::new(Vec::new())),
    });
    let (events_tx, events_rx) = async_channel::unbounded::<TransportEvent>();
    (transport, events_tx, events_rx)
}

/// Variant of `xx_serve_full` that returns after delivering ServerHello. A
/// client that rejects the chain never sends ClientFinish, so waiting for
/// it would only burn the deadline.
async fn xx_serve_hello_only(
    server: &InProcessServer,
    transport: &Arc<CaptureTransport>,
    events_tx: &async_channel::Sender<TransportEvent>,
) {
    wait_for_send(transport, 1).await;
    let raw_hello = transport.sent.lock().unwrap()[0].to_vec();
    let client_hello_bytes = parse_first_client_frame(&raw_hello);

    let msg = wa::HandshakeMessage::decode_from_slice(client_hello_bytes.as_slice()).unwrap();
    let client_eph_pub_vec = msg.client_hello.into_option().unwrap().ephemeral.unwrap();
    let client_eph_pub: [u8; 32] = client_eph_pub_vec.try_into().unwrap();

    let mut noise = NoiseHandshake::new(NOISE_PATTERN_XX, &WA_CONN_HEADER).unwrap();
    noise.authenticate(&client_eph_pub);

    let server_eph = KeyPair::generate(&mut rand::rng());
    let server_eph_pub: [u8; 32] = server_eph.public_key.public_key_bytes().try_into().unwrap();
    noise.authenticate(&server_eph_pub);
    noise
        .mix_shared_secret(server_eph.private_key.serialize(), &client_eph_pub)
        .unwrap();
    let encrypted_static = noise.encrypt(&server.server_static_pub()).unwrap();
    noise
        .mix_shared_secret(server.identity_kp.private_key.serialize(), &client_eph_pub)
        .unwrap();
    let encrypted_payload = noise.encrypt(&server.cert_chain_bytes).unwrap();

    let server_hello = wa::HandshakeMessage {
        server_hello: buffa::MessageField::some(wa::handshake_message::ServerHello {
            ephemeral: Some(server_eph_pub.to_vec()),
            r#static: Some(encrypted_static),
            payload: Some(encrypted_payload),
            ..Default::default()
        }),
        ..Default::default()
    };
    let sh_bytes = server_hello.encode_to_vec();
    let framed = wacore::framing::encode_frame(&sh_bytes, None).unwrap();
    events_tx
        .send(TransportEvent::DataReceived(framed.into()))
        .await
        .unwrap();
}

/// Synchronously serves an XX handshake: consumes the buffered
/// ClientHello frame, builds + signals the ServerHello via events_tx,
/// then consumes the ClientFinish.
async fn xx_serve_full(
    server: &InProcessServer,
    transport: &Arc<CaptureTransport>,
    events_tx: &async_channel::Sender<TransportEvent>,
) {
    // Wait until the client put something on the wire.
    wait_for_send(transport, 1).await;
    let raw_hello = transport.sent.lock().unwrap()[0].to_vec();
    let client_hello_bytes = parse_first_client_frame(&raw_hello);

    let msg = wa::HandshakeMessage::decode_from_slice(client_hello_bytes.as_slice()).unwrap();
    let client_eph_pub_vec = msg.client_hello.into_option().unwrap().ephemeral.unwrap();
    let client_eph_pub: [u8; 32] = client_eph_pub_vec.try_into().unwrap();

    let mut noise = NoiseHandshake::new(NOISE_PATTERN_XX, &WA_CONN_HEADER).unwrap();
    noise.authenticate(&client_eph_pub);

    let server_eph = KeyPair::generate(&mut rand::rng());
    let server_eph_pub: [u8; 32] = server_eph.public_key.public_key_bytes().try_into().unwrap();
    noise.authenticate(&server_eph_pub);
    noise
        .mix_shared_secret(server_eph.private_key.serialize(), &client_eph_pub)
        .unwrap();
    let encrypted_static = noise.encrypt(&server.server_static_pub()).unwrap();
    noise
        .mix_shared_secret(server.identity_kp.private_key.serialize(), &client_eph_pub)
        .unwrap();
    let encrypted_payload = noise.encrypt(&server.cert_chain_bytes).unwrap();

    let server_hello = wa::HandshakeMessage {
        server_hello: buffa::MessageField::some(wa::handshake_message::ServerHello {
            ephemeral: Some(server_eph_pub.to_vec()),
            r#static: Some(encrypted_static),
            payload: Some(encrypted_payload),
            ..Default::default()
        }),
        ..Default::default()
    };
    let sh_bytes = server_hello.encode_to_vec();
    let framed = wacore::framing::encode_frame(&sh_bytes, None).unwrap();
    events_tx
        .send(TransportEvent::DataReceived(framed.into()))
        .await
        .unwrap();

    // Consume the ClientFinish. We don't bother completing the responder
    // side — the assertion target is the cipher pair on the client side
    // (and the cached cert chain), both of which are derived before the
    // ClientFinish is consumed by the server.
    wait_for_send(transport, 2).await;
}

/// IK-accept responder: parses the IK ClientHello (e + encrypted s +
/// encrypted payload), and replies with a ServerHello whose `static` is
/// absent (signals IK success).
async fn ik_serve_accept(
    server: &InProcessServer,
    transport: &Arc<CaptureTransport>,
    events_tx: &async_channel::Sender<TransportEvent>,
) {
    wait_for_send(transport, 1).await;
    let raw_hello = transport.sent.lock().unwrap()[0].to_vec();
    let client_hello_bytes = parse_first_client_frame(&raw_hello);

    let msg = wa::HandshakeMessage::decode_from_slice(client_hello_bytes.as_slice()).unwrap();
    let ch = msg.client_hello.into_option().unwrap();
    let client_eph_pub: [u8; 32] = ch.ephemeral.unwrap().try_into().unwrap();
    let encrypted_static = ch.r#static.unwrap();
    let encrypted_payload = ch.payload.unwrap();

    let mut noise = NoiseHandshake::new(NOISE_PATTERN_IK, &WA_CONN_HEADER).unwrap();
    noise.authenticate(&server.server_static_pub());
    noise.authenticate(&client_eph_pub);
    noise
        .mix_shared_secret(server.identity_kp.private_key.serialize(), &client_eph_pub)
        .unwrap();
    let client_static = noise.decrypt(&encrypted_static).unwrap();
    let client_static: [u8; 32] = client_static.try_into().unwrap();
    noise
        .mix_shared_secret(server.identity_kp.private_key.serialize(), &client_static)
        .unwrap();
    let _payload = noise.decrypt(&encrypted_payload).unwrap();

    let server_eph = KeyPair::generate(&mut rand::rng());
    let server_eph_pub: [u8; 32] = server_eph.public_key.public_key_bytes().try_into().unwrap();
    noise.authenticate(&server_eph_pub);
    noise
        .mix_shared_secret(server_eph.private_key.serialize(), &client_eph_pub)
        .unwrap();
    noise
        .mix_shared_secret(server_eph.private_key.serialize(), &client_static)
        .unwrap();
    let encrypted_cert = noise.encrypt(&server.cert_chain_bytes).unwrap();

    let server_hello = wa::HandshakeMessage {
        server_hello: buffa::MessageField::some(wa::handshake_message::ServerHello {
            ephemeral: Some(server_eph_pub.to_vec()),
            r#static: None,
            payload: Some(encrypted_cert),
            ..Default::default()
        }),
        ..Default::default()
    };
    let sh_bytes = server_hello.encode_to_vec();
    let framed = wacore::framing::encode_frame(&sh_bytes, None).unwrap();
    events_tx
        .send(TransportEvent::DataReceived(framed.into()))
        .await
        .unwrap();
}

/// Waits up to ~3s for the captured-sent buffer to reach `min_count`
/// entries. Polls every 5 ms; tight enough for unit tests.
async fn wait_for_send(transport: &Arc<CaptureTransport>, min_count: usize) {
    let start = wacore::time::Instant::now();
    let timeout = Duration::from_secs(3);
    loop {
        if transport.sent.lock().unwrap().len() >= min_count {
            return;
        }
        if start.elapsed() >= timeout {
            panic!(
                "transport did not produce {} sends within deadline (got {})",
                min_count,
                transport.sent.lock().unwrap().len()
            );
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
}

/// Builds a fresh PersistenceManager backed by an in-memory store.
async fn pm() -> Arc<whatsapp_rust::store::persistence_manager::PersistenceManager> {
    let backend: Arc<dyn whatsapp_rust::store::traits::Backend> =
        Arc::new(wacore::store::InMemoryBackend::new());
    Arc::new(
        whatsapp_rust::store::persistence_manager::PersistenceManager::new(backend)
            .await
            .expect("pm init"),
    )
}

/// `pm()` seeded with a `pn`, so `select_pattern` will consider IK.
async fn paired_pm() -> Arc<whatsapp_rust::store::persistence_manager::PersistenceManager> {
    let pm = pm().await;
    pm.process_command(wacore::store::DeviceCommand::SetId(Some(
        "12345@s.whatsapp.net".parse().unwrap(),
    )))
    .await;
    pm
}

fn runtime() -> Arc<dyn wacore::runtime::Runtime> {
    Arc::new(whatsapp_rust::runtime_impl::TokioRuntime)
}

#[tokio::test]
async fn cold_start_xx_then_cached_ik_reconnect() {
    let _ = env_logger::builder().is_test(true).try_init();

    // The XX→IK reconnect path runs only for paired devices; an unpaired
    // device intentionally sticks to XX and never persists the chain (see
    // `should_persist_cert_chain`). Real cold-start happens unpaired and
    // is covered by the unit tests in `src/handshake.rs`.
    let pm = paired_pm().await;
    let counter = Arc::new(AtomicU32::new(0));
    let server = InProcessServer::new();
    let server_static_pub_expected = server.server_static_pub();

    // ── 1. Cold start: no cache → XX
    let (transport, events_tx, mut events_rx) = new_transport_pair();
    let pm2 = pm.clone();
    let counter2 = counter.clone();
    let runtime = runtime();
    let server_t = transport.clone();
    let server_arc = Arc::new(server);
    let server_clone = server_arc.clone();
    let server_task = tokio::spawn(async move {
        xx_serve_full(&server_clone, &server_t, &events_tx).await;
    });

    let result = do_handshake_with_cert_policy(
        runtime.clone(),
        pm2.as_ref(),
        counter2.as_ref(),
        transport.clone(),
        &mut events_rx,
        SendObservers::default(),
        BYPASS,
    )
    .await;

    server_task.await.unwrap();

    // The XX handshake should complete fully under the bypass, and the
    // bypass-accepted chain must NOT populate the trusted cache.
    assert!(
        result.is_ok(),
        "XX handshake should succeed: {:?}",
        result.err()
    );

    let device = pm.get_device_snapshot();
    assert!(
        device.server_cert_chain.is_none(),
        "bypass XX must not persist its chain as trusted IK state"
    );

    // Counter must remain at 0 after a successful handshake.
    assert_eq!(counter.load(std::sync::atomic::Ordering::Acquire), 0);

    // Seed the cache the way a strict XX would have, then reconnect under
    // STRICT on the same backend: per-client policy, not a global flag, is
    // what lets these two handshakes disagree in one process.
    use wacore::store::{CachedNoiseCert, CachedServerCertChain, DeviceCommand};
    pm.process_command(DeviceCommand::SetServerCertChain(CachedServerCertChain {
        intermediate: CachedNoiseCert {
            key: [0xCC; 32],
            not_before: 1_700_000_000,
            not_after: 1_900_000_000,
        },
        leaf: CachedNoiseCert {
            key: server_static_pub_expected,
            not_before: 1_700_000_500,
            not_after: 1_899_999_500,
        },
        signature_verified: true,
    }))
    .await;

    // ── 2. Reconnect: cache present → IK
    let (transport2, events_tx2, mut events_rx2) = new_transport_pair();
    let pm3 = pm.clone();
    let counter3 = counter.clone();
    let server_clone2 = server_arc.clone();
    let server_t2 = transport2.clone();
    let ik_task = tokio::spawn(async move {
        ik_serve_accept(&server_clone2, &server_t2, &events_tx2).await;
    });

    let result = do_handshake_with_cert_policy(
        runtime,
        pm3.as_ref(),
        counter3.as_ref(),
        transport2.clone(),
        &mut events_rx2,
        SendObservers::default(),
        STRICT,
    )
    .await;

    ik_task.await.unwrap();

    assert!(
        result.is_ok(),
        "IK handshake on reconnect should succeed: {:?}",
        result.err()
    );

    // After IK Continue, the on-disk cache stays as-is (the orchestrator
    // does NOT issue SetServerCertChain on the IK path).
    let device = pm.get_device_snapshot();
    assert_eq!(
        device.server_cert_chain.as_ref().unwrap().leaf.key,
        server_static_pub_expected
    );
    assert_eq!(counter.load(std::sync::atomic::Ordering::Acquire), 0);

    // The IK ClientHello should carry static + payload, not just an
    // ephemeral. Inspect the captured bytes to confirm the path.
    let sent = transport2.sent.lock().unwrap();
    let raw = sent[0].to_vec();
    let body = parse_first_client_frame(&raw);
    let parsed = wa::HandshakeMessage::decode_from_slice(body.as_slice()).unwrap();
    let ch = parsed.client_hello.into_option().unwrap();
    assert!(
        ch.r#static.is_some(),
        "IK ClientHello carries client static"
    );
    assert!(ch.payload.is_some(), "IK ClientHello carries 0-RTT payload");
}

#[tokio::test]
async fn strict_xx_rejects_zero_signed_chain() {
    let _ = env_logger::builder().is_test(true).try_init();

    let pm = paired_pm().await;
    let counter = Arc::new(AtomicU32::new(0));
    let server = Arc::new(InProcessServer::new());

    let (transport, events_tx, mut events_rx) = new_transport_pair();
    let server_clone = server.clone();
    let server_t = transport.clone();
    let server_task = tokio::spawn(async move {
        xx_serve_hello_only(&server_clone, &server_t, &events_tx).await;
    });

    let result = do_handshake_with_cert_policy(
        runtime(),
        pm.as_ref(),
        counter.as_ref(),
        transport.clone(),
        &mut events_rx,
        SendObservers::default(),
        STRICT,
    )
    .await;
    server_task.await.unwrap();

    assert!(
        result.is_err(),
        "strict XX must reject the zero-signed chain"
    );
    let err = result.err().unwrap();
    let msg = err.to_string();
    assert!(
        msg.contains("Error verifying server cert"),
        "strict XX must fail at cert verification, got: {msg}"
    );
    assert!(
        err.is_crypto_fatal(),
        "cert failure must classify as crypto-fatal: {err:?}"
    );
    assert!(pm.get_device_snapshot().server_cert_chain.is_none());
    assert_eq!(counter.load(std::sync::atomic::Ordering::Acquire), 0);
}

#[tokio::test]
async fn legacy_unmarked_cache_forces_xx() {
    let _ = env_logger::builder().is_test(true).try_init();

    // A record predating signature provenance (as cached while a global
    // bypass was enabled): fresh, correct static, but unmarked.
    let pm = paired_pm().await;
    let counter = Arc::new(AtomicU32::new(0));
    let server = Arc::new(InProcessServer::new());
    use wacore::store::{CachedNoiseCert, CachedServerCertChain, DeviceCommand};
    pm.process_command(DeviceCommand::SetServerCertChain(CachedServerCertChain {
        intermediate: CachedNoiseCert {
            key: [0xCC; 32],
            not_before: 1_700_000_000,
            not_after: 1_900_000_000,
        },
        leaf: CachedNoiseCert {
            key: server.server_static_pub(),
            not_before: 1_700_000_500,
            not_after: 1_899_999_500,
        },
        signature_verified: false,
    }))
    .await;

    let (transport, events_tx, mut events_rx) = new_transport_pair();
    let server_clone = server.clone();
    let server_t = transport.clone();
    let server_task = tokio::spawn(async move {
        xx_serve_hello_only(&server_clone, &server_t, &events_tx).await;
    });

    let result = do_handshake_with_cert_policy(
        runtime(),
        pm.as_ref(),
        counter.as_ref(),
        transport.clone(),
        &mut events_rx,
        SendObservers::default(),
        STRICT,
    )
    .await;
    server_task.await.unwrap();

    // The unmarked cache must not authorize IK: the hello is XX-shaped.
    let sent = transport.sent.lock().unwrap();
    let body = parse_first_client_frame(&sent[0]);
    let parsed = wa::HandshakeMessage::decode_from_slice(body.as_slice()).unwrap();
    let ch = parsed.client_hello.into_option().unwrap();
    assert!(
        ch.r#static.is_none(),
        "unmarked cache must force XX (no client static in hello)"
    );

    // And the unsigned chain is then rejected. The seeded record survives
    // untouched: an XX failure never clears the cache, it just refuses to
    // add to it.
    assert!(
        result.is_err(),
        "strict XX must reject the zero-signed chain"
    );
    let err = result.err().unwrap();
    assert!(
        err.to_string().contains("Error verifying server cert"),
        "unexpected error: {err}"
    );
    let chain = pm
        .get_device_snapshot()
        .server_cert_chain
        .clone()
        .expect("seeded record survives a failed XX");
    assert!(!chain.signature_verified);
    assert_eq!(chain.leaf.key, server.server_static_pub());
}

#[tokio::test]
async fn bypass_leaves_nothing_for_strict_to_reuse() {
    let _ = env_logger::builder().is_test(true).try_init();

    let pm = paired_pm().await;
    let counter = Arc::new(AtomicU32::new(0));
    let server = Arc::new(InProcessServer::new());

    // Bypass XX succeeds against the mock but persists nothing.
    let (transport, events_tx, mut events_rx) = new_transport_pair();
    let server_clone = server.clone();
    let server_t = transport.clone();
    let server_task = tokio::spawn(async move {
        xx_serve_full(&server_clone, &server_t, &events_tx).await;
    });
    do_handshake_with_cert_policy(
        runtime(),
        pm.as_ref(),
        counter.as_ref(),
        transport.clone(),
        &mut events_rx,
        SendObservers::default(),
        BYPASS,
    )
    .await
    .expect("bypass XX must succeed");
    server_task.await.unwrap();
    assert!(pm.get_device_snapshot().server_cert_chain.is_none());

    // A strict client on the same backend finds no trusted cache, starts
    // at XX, and rejects the unsigned chain: bypass residue cannot
    // authorize a strict IK.
    let (transport2, events_tx2, mut events_rx2) = new_transport_pair();
    let server_clone2 = server.clone();
    let server_t2 = transport2.clone();
    let server_task2 = tokio::spawn(async move {
        xx_serve_hello_only(&server_clone2, &server_t2, &events_tx2).await;
    });
    let result = do_handshake_with_cert_policy(
        runtime(),
        pm.as_ref(),
        counter.as_ref(),
        transport2.clone(),
        &mut events_rx2,
        SendObservers::default(),
        STRICT,
    )
    .await;
    server_task2.await.unwrap();

    let sent = transport2.sent.lock().unwrap();
    let body = parse_first_client_frame(&sent[0]);
    let parsed = wa::HandshakeMessage::decode_from_slice(body.as_slice()).unwrap();
    assert!(
        parsed
            .client_hello
            .into_option()
            .unwrap()
            .r#static
            .is_none(),
        "strict client must start at XX after a bypass connect"
    );
    assert!(
        result.is_err(),
        "strict XX must reject the zero-signed chain"
    );
    let err = result.err().unwrap();
    assert!(
        err.to_string().contains("Error verifying server cert"),
        "unexpected error: {err}"
    );
    assert!(pm.get_device_snapshot().server_cert_chain.is_none());
    assert_eq!(counter.load(std::sync::atomic::Ordering::Acquire), 0);
}

/// Sends a fallback-shaped ServerHello (`static.is_some()`) with garbage
/// AEAD payloads, so the initiator pivots and then fails inside XXfallback.
async fn ik_serve_fallback_with_corrupt_payloads(
    transport: &Arc<CaptureTransport>,
    events_tx: &async_channel::Sender<TransportEvent>,
) {
    wait_for_send(transport, 1).await;

    let server_eph = KeyPair::generate(&mut rand::rng());
    let server_eph_pub: [u8; 32] = server_eph.public_key.public_key_bytes().try_into().unwrap();

    let server_hello = wa::HandshakeMessage {
        server_hello: buffa::MessageField::some(wa::handshake_message::ServerHello {
            ephemeral: Some(server_eph_pub.to_vec()),
            r#static: Some(vec![0xCC; 32 + 16]),
            payload: Some(vec![0xDE; 64]),
            ..Default::default()
        }),
        ..Default::default()
    };
    let sh_bytes = server_hello.encode_to_vec();
    let framed = wacore::framing::encode_frame(&sh_bytes, None).unwrap();
    events_tx
        .send(TransportEvent::DataReceived(framed.into()))
        .await
        .unwrap();
}

#[tokio::test]
async fn post_xxfallback_failure_does_not_invalidate_ik_cache() {
    let _ = env_logger::builder().is_test(true).try_init();

    let pm = paired_pm().await;
    let counter = Arc::new(AtomicU32::new(0));

    // Pre-seed a valid-looking chain with a sentinel `not_after` so we can
    // distinguish "untouched" from "cleared and rewritten by some path".
    const SENTINEL_NOT_AFTER: i64 = 1_899_999_999;
    use wacore::store::{CachedNoiseCert, CachedServerCertChain, DeviceCommand};
    pm.process_command(DeviceCommand::SetServerCertChain(CachedServerCertChain {
        intermediate: CachedNoiseCert {
            key: [0xCC; 32],
            not_before: 1_700_000_000,
            not_after: SENTINEL_NOT_AFTER,
        },
        leaf: CachedNoiseCert {
            key: [0xAA; 32],
            not_before: 1_700_000_500,
            not_after: SENTINEL_NOT_AFTER,
        },
        signature_verified: true,
    }))
    .await;

    let (transport, events_tx, mut events_rx) = new_transport_pair();
    let transport_clone = transport.clone();
    let task = tokio::spawn(async move {
        ik_serve_fallback_with_corrupt_payloads(&transport_clone, &events_tx).await;
    });

    let result = do_handshake_with_cert_policy(
        runtime(),
        pm.as_ref(),
        counter.as_ref(),
        transport.clone(),
        &mut events_rx,
        SendObservers::default(),
        STRICT,
    )
    .await;
    task.await.unwrap();

    assert!(
        result.is_err(),
        "post-pivot AEAD failure must surface as Err"
    );
    let err = result.err().unwrap();
    assert!(
        err.is_crypto_fatal(),
        "AEAD decrypt failure on XXfallback ServerHello must classify as crypto-fatal: {err:?}"
    );

    // Cache must be untouched: the failure happened post-pivot, so the
    // orchestrator's invalidation gate must have skipped both the clear
    // and the counter increment.
    let device = pm.get_device_snapshot();
    let chain = device
        .server_cert_chain
        .as_ref()
        .expect("post-fallback failure must NOT clear the cached chain");
    assert_eq!(chain.leaf.not_after, SENTINEL_NOT_AFTER);
    assert_eq!(chain.intermediate.not_after, SENTINEL_NOT_AFTER);
    assert_eq!(
        counter.load(std::sync::atomic::Ordering::Acquire),
        0,
        "post-fallback failure must NOT increment ik_handshake_failures"
    );
}

#[tokio::test]
async fn ik_continue_does_not_overwrite_cached_chain() {
    let _ = env_logger::builder().is_test(true).try_init();

    let pm = paired_pm().await;
    let counter = Arc::new(AtomicU32::new(0));
    let server = Arc::new(InProcessServer::new());

    // Sentinel distinct from anything `build_cert_chain_bytes` produces.
    const SENTINEL_NOT_AFTER: i64 = 1_899_999_999;
    use wacore::store::{CachedNoiseCert, CachedServerCertChain, DeviceCommand};
    pm.process_command(DeviceCommand::SetServerCertChain(CachedServerCertChain {
        intermediate: CachedNoiseCert {
            key: [0xCC; 32],
            not_before: 1_700_000_000,
            not_after: SENTINEL_NOT_AFTER,
        },
        leaf: CachedNoiseCert {
            key: server.server_static_pub(),
            not_before: 1_700_000_500,
            not_after: SENTINEL_NOT_AFTER,
        },
        signature_verified: true,
    }))
    .await;

    let (transport, events_tx, mut events_rx) = new_transport_pair();
    let server_clone = server.clone();
    let server_t = transport.clone();
    let task = tokio::spawn(async move {
        ik_serve_accept(&server_clone, &server_t, &events_tx).await;
    });

    let result = do_handshake_with_cert_policy(
        runtime(),
        pm.as_ref(),
        counter.as_ref(),
        transport.clone(),
        &mut events_rx,
        SendObservers::default(),
        STRICT,
    )
    .await;
    task.await.unwrap();

    assert!(
        result.is_ok(),
        "IK Continue must succeed: {:?}",
        result.err()
    );

    let device = pm.get_device_snapshot();
    let chain = device
        .server_cert_chain
        .as_ref()
        .expect("chain still present");
    assert_eq!(
        chain.leaf.not_after, SENTINEL_NOT_AFTER,
        "IK Continue must leave the cached leaf.not_after untouched"
    );
    assert_eq!(
        chain.intermediate.not_after, SENTINEL_NOT_AFTER,
        "IK Continue must leave the cached intermediate.not_after untouched"
    );
}

/// XX-1 unpaired (no persist) → SetId (mocks pair-success) → XX-2 paired.
/// Under the bypass neither persists: the pairing gate itself
/// (`should_persist_cert_chain`) is covered by unit tests, while no
/// bypass-accepted chain may populate trusted state even once registered.
#[tokio::test]
async fn xx_after_pair_success_under_bypass_leaves_cache_empty() {
    let _ = env_logger::builder().is_test(true).try_init();

    let pm = pm().await;
    let counter = Arc::new(AtomicU32::new(0));
    let server = Arc::new(InProcessServer::new());

    let (transport1, events_tx1, mut events_rx1) = new_transport_pair();
    let server_t1 = transport1.clone();
    let server_c1 = server.clone();
    let task1 = tokio::spawn(async move {
        xx_serve_full(&server_c1, &server_t1, &events_tx1).await;
    });
    do_handshake_with_cert_policy(
        runtime(),
        pm.as_ref(),
        counter.as_ref(),
        transport1.clone(),
        &mut events_rx1,
        SendObservers::default(),
        BYPASS,
    )
    .await
    .expect("unpaired XX must succeed");
    task1.await.unwrap();

    let device = pm.get_device_snapshot();
    assert!(!device.is_registered(), "still unpaired after first XX");
    assert!(
        device.server_cert_chain.is_none(),
        "unpaired XX must not persist chain"
    );

    pm.process_command(wacore::store::DeviceCommand::SetId(Some(
        "12345@s.whatsapp.net".parse().unwrap(),
    )))
    .await;

    let (transport2, events_tx2, mut events_rx2) = new_transport_pair();
    let server_t2 = transport2.clone();
    let server_c2 = server.clone();
    let task2 = tokio::spawn(async move {
        xx_serve_full(&server_c2, &server_t2, &events_tx2).await;
    });
    do_handshake_with_cert_policy(
        runtime(),
        pm.as_ref(),
        counter.as_ref(),
        transport2.clone(),
        &mut events_rx2,
        SendObservers::default(),
        BYPASS,
    )
    .await
    .expect("paired XX must succeed");
    task2.await.unwrap();

    let device = pm.get_device_snapshot();
    assert!(device.is_registered(), "paired after SetId");
    assert!(
        device.server_cert_chain.is_none(),
        "bypass XX must not persist even once registered"
    );
}

/// Regression: PR #598's unpaired-then-restart loop. XX must not persist
/// the chain when `pn` is unset, otherwise the next connect picks IK
/// against an unregistered identity and the server closes mid-handshake.
#[tokio::test]
async fn unpaired_xx_does_not_persist_cert_chain() {
    let _ = env_logger::builder().is_test(true).try_init();

    let pm = pm().await; // Intentionally NOT paired_pm — that's the point.
    let counter = Arc::new(AtomicU32::new(0));
    let server = Arc::new(InProcessServer::new());

    let (transport, events_tx, mut events_rx) = new_transport_pair();
    let server_clone = server.clone();
    let server_t = transport.clone();
    let task = tokio::spawn(async move {
        xx_serve_full(&server_clone, &server_t, &events_tx).await;
    });

    let result = do_handshake_with_cert_policy(
        runtime(),
        pm.as_ref(),
        counter.as_ref(),
        transport.clone(),
        &mut events_rx,
        SendObservers::default(),
        BYPASS,
    )
    .await;
    task.await.unwrap();

    assert!(
        result.is_ok(),
        "XX handshake on unpaired device should succeed: {:?}",
        result.err()
    );

    let device = pm.get_device_snapshot();
    assert!(
        !device.is_registered(),
        "precondition: device must still be unpaired"
    );
    assert!(device.server_cert_chain.is_none());
    assert_eq!(counter.load(std::sync::atomic::Ordering::Acquire), 0);
}

/// IK-reject responder: parses the ClientHello's ephemeral, then replies
/// with an XX-shaped ServerHello (carrying `static != null`) using the
/// XXfallback pattern. The helper returns after delivering the hello: a
/// strict client rejects the zero-signed chain before sending ClientFinish,
/// so waiting for it would only time out.
async fn ik_serve_force_fallback(
    server: &InProcessServer,
    transport: &Arc<CaptureTransport>,
    events_tx: &async_channel::Sender<TransportEvent>,
) {
    wait_for_send(transport, 1).await;
    let raw_hello = transport.sent.lock().unwrap()[0].to_vec();
    let client_hello_bytes = parse_first_client_frame(&raw_hello);

    let msg = wa::HandshakeMessage::decode_from_slice(client_hello_bytes.as_slice()).unwrap();
    let client_eph_pub: [u8; 32] = msg
        .client_hello
        .into_option()
        .unwrap()
        .ephemeral
        .unwrap()
        .try_into()
        .unwrap();

    // Build XXfallback responder state matching what the initiator will
    // construct after seeing `static != null`.
    let mut noise = NoiseHandshake::new(
        wacore_binary::consts::NOISE_PATTERN_XXFALLBACK,
        &WA_CONN_HEADER,
    )
    .unwrap();
    noise.authenticate(&client_eph_pub);

    let server_eph = KeyPair::generate(&mut rand::rng());
    let server_eph_pub: [u8; 32] = server_eph.public_key.public_key_bytes().try_into().unwrap();
    noise.authenticate(&server_eph_pub);
    noise
        .mix_shared_secret(server_eph.private_key.serialize(), &client_eph_pub)
        .unwrap();
    let encrypted_static = noise.encrypt(&server.server_static_pub()).unwrap();
    noise
        .mix_shared_secret(server.identity_kp.private_key.serialize(), &client_eph_pub)
        .unwrap();
    let encrypted_cert = noise.encrypt(&server.cert_chain_bytes).unwrap();

    let server_hello = wa::HandshakeMessage {
        server_hello: buffa::MessageField::some(wa::handshake_message::ServerHello {
            ephemeral: Some(server_eph_pub.to_vec()),
            r#static: Some(encrypted_static),
            payload: Some(encrypted_cert),
            ..Default::default()
        }),
        ..Default::default()
    };
    let sh_bytes = server_hello.encode_to_vec();
    let framed = wacore::framing::encode_frame(&sh_bytes, None).unwrap();
    events_tx
        .send(TransportEvent::DataReceived(framed.into()))
        .await
        .unwrap();
}

#[tokio::test]
async fn ik_rejected_fallback_still_verifies_fresh_chain() {
    let _ = env_logger::builder().is_test(true).try_init();

    let pm = paired_pm().await;
    let counter = Arc::new(AtomicU32::new(0));
    let server = Arc::new(InProcessServer::new());
    let server_static_pub_expected = server.server_static_pub();

    // Pre-seed the device with the *correct* cert chain (so IK is selected),
    // but the responder will still force fallback (mirrors the case where
    // the server has just rotated and the client's cache is one connect
    // out of date). The fallback chain is zero-signed, so a strict client
    // must reject it at verification.
    use wacore::store::{CachedNoiseCert, CachedServerCertChain, DeviceCommand};
    pm.process_command(DeviceCommand::SetServerCertChain(CachedServerCertChain {
        intermediate: CachedNoiseCert {
            key: [0xCC; 32],
            not_before: 1_700_000_000,
            not_after: 1_900_000_000,
        },
        leaf: CachedNoiseCert {
            key: server_static_pub_expected,
            not_before: 1_700_000_500,
            not_after: 1_899_999_500,
        },
        signature_verified: true,
    }))
    .await;

    let (transport, events_tx, mut events_rx) = new_transport_pair();
    let server_clone = server.clone();
    let transport_clone = transport.clone();
    let task = tokio::spawn(async move {
        ik_serve_force_fallback(&server_clone, &transport_clone, &events_tx).await;
    });

    let result = do_handshake_with_cert_policy(
        runtime(),
        pm.as_ref(),
        counter.as_ref(),
        transport.clone(),
        &mut events_rx,
        SendObservers::default(),
        STRICT,
    )
    .await;
    task.await.unwrap();

    assert!(
        result.is_err(),
        "fallback with a zero-signed chain must fail verification"
    );
    let err = result.err().unwrap();
    let msg = err.to_string();
    assert!(
        msg.contains("Error verifying server cert"),
        "fallback must fail at cert verification, got: {msg}"
    );
    assert!(
        err.is_crypto_fatal(),
        "cert failure must classify as crypto-fatal: {err:?}"
    );

    // The failure happened past the XXfallback pivot, so the invalidation
    // gate must skip both the clear and the counter increment.
    assert_eq!(counter.load(std::sync::atomic::Ordering::Acquire), 0);
    let device = pm.get_device_snapshot();
    let chain = device
        .server_cert_chain
        .as_ref()
        .expect("post-pivot failure must NOT clear the cached chain");
    assert_eq!(chain.leaf.key, server_static_pub_expected);
}

#[tokio::test]
async fn ik_with_stale_cache_invalidates_and_increments_counter() {
    let _ = env_logger::builder().is_test(true).try_init();

    let pm = paired_pm().await;
    let counter = Arc::new(AtomicU32::new(0));

    // Pre-seed the device with a STALE cert chain — the leaf.key is wrong
    // (random, not matching any real server static). The next connect
    // will pick IK based on this cache, the responder will fail to
    // process the resulting clientHello (because the server doesn't have
    // the matching private key), and we expect:
    //   - the handshake errors out crypto-fatally (or the responder side
    //     panics, which we catch and surface as a transient channel close
    //     event)
    //   - the orchestrator increments the counter and clears the cache.
    use wacore::store::{CachedNoiseCert, CachedServerCertChain, DeviceCommand};
    let stale_pub = [0xDE; 32];
    pm.process_command(DeviceCommand::SetServerCertChain(CachedServerCertChain {
        intermediate: CachedNoiseCert {
            key: [0xCC; 32],
            not_before: 1_700_000_000,
            not_after: 1_900_000_000,
        },
        leaf: CachedNoiseCert {
            key: stale_pub,
            not_before: 1_700_000_500,
            not_after: 1_899_999_500,
        },
        signature_verified: true,
    }))
    .await;

    let (transport, events_tx, mut events_rx) = new_transport_pair();

    // Stand up a real server with a DIFFERENT static keypair than the one
    // cached above. When the client sends an IK ClientHello using
    // stale_pub, the responder's `es` derivation diverges and decryption
    // of the client static fails → we surface that by closing the events
    // channel without sending a ServerHello, which the client interprets
    // as a Disconnected event (transient).
    //
    // To test the *crypto-fatal* path instead, we send a malformed
    // ServerHello so the client's read_server_hello errors out at
    // decrypt of the cert (Core handshake error → invalidation runs).
    let bogus_server_eph = KeyPair::generate(&mut rand::rng()).public_key;
    let bogus_server_eph_bytes: [u8; 32] = bogus_server_eph.public_key_bytes().try_into().unwrap();
    let server_hello = wa::HandshakeMessage {
        server_hello: buffa::MessageField::some(wa::handshake_message::ServerHello {
            ephemeral: Some(bogus_server_eph_bytes.to_vec()),
            r#static: None,
            // `payload` here is just garbage — AEAD MAC check will fail.
            payload: Some(vec![0xAB; 64]),
            ..Default::default()
        }),
        ..Default::default()
    };
    let sh_bytes = server_hello.encode_to_vec();
    let framed = wacore::framing::encode_frame(&sh_bytes, None).unwrap();

    let transport_for_task = transport.clone();
    let driver = tokio::spawn(async move {
        wait_for_send(&transport_for_task, 1).await;
        events_tx
            .send(TransportEvent::DataReceived(framed.into()))
            .await
            .unwrap();
    });

    let result = do_handshake_with_cert_policy(
        runtime(),
        pm.as_ref(),
        counter.as_ref(),
        transport.clone(),
        &mut events_rx,
        SendObservers::default(),
        STRICT,
    )
    .await;

    driver.await.unwrap();

    assert!(result.is_err(), "IK with bogus server hello must fail");
    let err = result.err().unwrap();
    assert!(
        err.is_crypto_fatal(),
        "decrypt failure must be classified as crypto-fatal: {err:?}"
    );

    // Invalidation policy: counter incremented, cache cleared.
    assert_eq!(
        counter.load(std::sync::atomic::Ordering::Acquire),
        1,
        "ik_handshake_failures must be 1 after one crypto-fatal failure"
    );
    let device = pm.get_device_snapshot();
    assert!(
        device.server_cert_chain.is_none(),
        "stale cert chain must be cleared after crypto-fatal IK failure"
    );
}

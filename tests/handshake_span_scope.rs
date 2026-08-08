//! The `wa.conn.handshake` span must cover the Noise exchange and nothing that
//! outlives it.
//!
//! A handshake happens once per connection, so consumers read the span as a
//! one-shot cost. Any task spawned while it is entered is rooted at it by every
//! tracing consumer that propagates `Span::current()` into spawned tasks — the
//! propagation `tracing`'s own documentation prescribes — and the connection's
//! outbound sender task, spawned by `NoiseSocket`, then charges the span for
//! every frame the connection ever writes.
//!
//! These tests watch `Span::current()` at each `Runtime::spawn` and assert what
//! the span does, and does not, own.

#![cfg(feature = "tracing")]
// Tests exercise the raw buffa API.
#![allow(clippy::disallowed_methods)]

use async_trait::async_trait;
use buffa::Message;
use bytes::Bytes;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::AtomicU32;
use std::time::Duration;

use tracing_subscriber::registry::LookupSpan;

use wacore::handshake::NoiseHandshake;
use wacore::libsignal::protocol::KeyPair;
use wacore::runtime::{AbortHandle, Runtime};
use wacore_binary::consts::{NOISE_PATTERN_XX, WA_CONN_HEADER};
use wacore_noise::test_util::build_cert_chain_bytes;
use whatsapp_rust::handshake::do_handshake;
use whatsapp_rust::socket::noise_socket::SendObservers;
use whatsapp_rust::transport::{Transport, TransportEvent};
use whatsapp_rust::waproto::whatsapp as wa;

const HANDSHAKE_SPAN: &str = "wa.conn.handshake";

/// The entered span and its ancestors, innermost first; empty outside any span.
///
/// The whole chain, not just the innermost name: what a propagating consumer
/// charges a span for is its entire subtree, so `wa.conn.handshake.xx` is as
/// much "inside the handshake span" as `wa.conn.handshake` itself.
fn current_span_scope() -> Vec<&'static str> {
    tracing::dispatcher::get_default(|dispatch| {
        let Some(id) = dispatch.current_span().id().cloned() else {
            return Vec::new();
        };
        let Some(registry) = dispatch.downcast_ref::<tracing_subscriber::Registry>() else {
            panic!("the tests install a bare Registry");
        };
        registry
            .span(&id)
            .map(|span| span.scope().map(|ancestor| ancestor.name()).collect())
            .unwrap_or_default()
    })
}

/// Span scopes observed at a series of call sites.
type SpanLog = Arc<StdMutex<Vec<Vec<&'static str>>>>;

/// Records the span current at every `spawn`, which is the span a propagating
/// consumer makes the parent of that task.
struct SpanWatchingRuntime {
    spawn_parents: SpanLog,
}

impl SpanWatchingRuntime {
    fn new() -> (Arc<Self>, SpanLog) {
        let spawn_parents = SpanLog::default();
        (
            Arc::new(Self {
                spawn_parents: spawn_parents.clone(),
            }),
            spawn_parents,
        )
    }
}

#[async_trait]
impl Runtime for SpanWatchingRuntime {
    fn spawn(&self, future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>) -> AbortHandle {
        self.spawn_parents
            .lock()
            .unwrap()
            .push(current_span_scope());
        let handle = tokio::spawn(future);
        AbortHandle::new(move || handle.abort())
    }

    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(tokio::time::sleep(duration))
    }

    fn spawn_blocking(
        &self,
        f: Box<dyn FnOnce() + Send + 'static>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(async {
            let _ = tokio::task::spawn_blocking(f).await;
        })
    }

    fn yield_now(&self) -> Option<Pin<Box<dyn Future<Output = ()> + Send>>> {
        None
    }
}

/// Records the span current at every `Transport::send`, which is how the tests
/// see where the handshake's own frames were written.
struct SpanWatchingTransport {
    sent: Arc<StdMutex<Vec<Bytes>>>,
    send_spans: SpanLog,
}

#[async_trait]
impl Transport for SpanWatchingTransport {
    async fn send(&self, data: Bytes) -> anyhow::Result<()> {
        self.send_spans.lock().unwrap().push(current_span_scope());
        self.sent.lock().unwrap().push(data);
        Ok(())
    }
    async fn disconnect(&self) {}
}

/// `Span::current()` only reports a span a subscriber kept, so every test needs
/// one installed. A bare registry stores all of them and filters none.
fn install_subscriber() {
    static ONCE: std::sync::Once = std::sync::Once::new();
    ONCE.call_once(|| {
        tracing::subscriber::set_global_default(tracing_subscriber::registry())
            .expect("no other subscriber in this test binary");
    });
}

// ── in-process XX responder ───────────────────────────────────────────────

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

fn parse_first_client_frame(raw: &[u8]) -> Vec<u8> {
    let body_start = if raw.starts_with(&WA_CONN_HEADER) {
        WA_CONN_HEADER.len()
    } else {
        0
    };
    let buf = &raw[body_start..];
    assert!(buf.len() >= 3, "frame buffer must hold at least the length");
    let len = ((buf[0] as usize) << 16) | ((buf[1] as usize) << 8) | (buf[2] as usize);
    assert!(buf.len() >= 3 + len, "frame buffer truncated");
    buf[3..3 + len].to_vec()
}

async fn wait_for_send(sent: &Arc<StdMutex<Vec<Bytes>>>, min_count: usize) {
    for _ in 0..600 {
        if sent.lock().unwrap().len() >= min_count {
            return;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    panic!("transport did not produce {min_count} sends within deadline");
}

async fn xx_serve_full(
    server: &InProcessServer,
    sent: &Arc<StdMutex<Vec<Bytes>>>,
    events_tx: &async_channel::Sender<TransportEvent>,
) {
    wait_for_send(sent, 1).await;
    let raw_hello = sent.lock().unwrap()[0].to_vec();
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
    let framed = wacore::framing::encode_frame(&server_hello.encode_to_vec(), None).unwrap();
    events_tx
        .send(TransportEvent::DataReceived(framed.into()))
        .await
        .unwrap();

    wait_for_send(sent, 2).await;
}

async fn paired_pm() -> Arc<whatsapp_rust::store::persistence_manager::PersistenceManager> {
    let backend: Arc<dyn whatsapp_rust::store::traits::Backend> =
        Arc::new(wacore::store::InMemoryBackend::new());
    let pm = Arc::new(
        whatsapp_rust::store::persistence_manager::PersistenceManager::new(backend)
            .await
            .expect("pm init"),
    );
    pm.process_command(wacore::store::DeviceCommand::SetId(Some(
        "12345@s.whatsapp.net".parse().unwrap(),
    )))
    .await;
    pm
}

struct Observed {
    spawn_parents: Vec<Vec<&'static str>>,
    send_spans: Vec<Vec<&'static str>>,
    handshake_ok: bool,
}

impl Observed {
    fn spawns_inside_handshake(&self) -> Vec<&Vec<&'static str>> {
        self.spawn_parents
            .iter()
            .filter(|scope| scope.contains(&HANDSHAKE_SPAN))
            .collect()
    }
}

/// One XX handshake against the in-process responder, reporting where tasks
/// were spawned and where handshake frames were written.
async fn observe_handshake(serve: bool) -> Observed {
    let pm = paired_pm().await;
    let counter = Arc::new(AtomicU32::new(0));
    let (runtime, spawn_parents) = SpanWatchingRuntime::new();

    let sent = Arc::new(StdMutex::new(Vec::new()));
    let send_spans = SpanLog::default();
    let transport = Arc::new(SpanWatchingTransport {
        sent: sent.clone(),
        send_spans: send_spans.clone(),
    });
    let (events_tx, mut events_rx) = async_channel::unbounded::<TransportEvent>();

    // Not serving means dropping the producer: the handshake then ends on a
    // closed event stream instead of sitting out its 20s response timeout.
    let server_task = if serve {
        let sent = sent.clone();
        Some(tokio::spawn(async move {
            xx_serve_full(&InProcessServer::new(), &sent, &events_tx).await;
        }))
    } else {
        drop(events_tx);
        None
    };

    let handshake_ok = do_handshake(
        runtime as Arc<dyn Runtime>,
        pm.as_ref(),
        counter.as_ref(),
        transport as Arc<dyn Transport>,
        &mut events_rx,
        SendObservers::default(),
    )
    .await
    .is_ok();

    if let Some(task) = server_task {
        task.await.unwrap();
    }

    let spawn_parents = spawn_parents.lock().unwrap().clone();
    let send_spans = send_spans.lock().unwrap().clone();
    Observed {
        spawn_parents,
        send_spans,
        handshake_ok,
    }
}

/// A successful handshake spawns the connection's sender task. Rooting it at
/// the handshake span charges a one-shot span for every frame the connection
/// writes from then on.
#[tokio::test]
async fn no_task_outliving_the_handshake_is_spawned_inside_its_span() {
    install_subscriber();
    let observed = observe_handshake(true).await;

    assert!(observed.handshake_ok, "the XX handshake must succeed");
    assert!(
        !observed.spawn_parents.is_empty(),
        "the connection's sender task must still be spawned, or this proves nothing"
    );
    let inside = observed.spawns_inside_handshake();
    assert!(
        inside.is_empty(),
        "a task was spawned inside {HANDSHAKE_SPAN}: {inside:?}"
    );
}

/// Narrowing the span must not empty it: the Noise exchange itself still has to
/// be what `wa.conn.handshake` measures.
#[tokio::test]
async fn the_handshake_span_still_covers_the_noise_exchange() {
    install_subscriber();
    let observed = observe_handshake(true).await;

    assert!(observed.handshake_ok, "the XX handshake must succeed");
    assert_eq!(
        observed.send_spans.len(),
        2,
        "XX writes a ClientHello and a ClientFinish"
    );
    for scope in &observed.send_spans {
        assert!(
            scope.contains(&HANDSHAKE_SPAN),
            "a handshake frame was written outside {HANDSHAKE_SPAN}: {scope:?}"
        );
    }
}

/// The failure path must not leak a task into the span either. Nothing answers
/// the ClientHello here, so the handshake ends on a torn-down transport event.
#[tokio::test]
async fn a_failed_handshake_spawns_nothing_inside_its_span() {
    install_subscriber();
    let observed = observe_handshake(false).await;

    assert!(
        !observed.handshake_ok,
        "an unanswered ClientHello must not complete"
    );
    let inside = observed.spawns_inside_handshake();
    assert!(
        inside.is_empty(),
        "a task was spawned inside {HANDSHAKE_SPAN}: {inside:?}"
    );
}

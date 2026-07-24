use std::sync::Arc;

use crate::Client;
use wacore_binary::{Jid, Node, OwnedNodeRef};

/// Marshal a `Node` into an `Arc<OwnedNodeRef>` for use in tests.
pub fn node_to_owned_ref(node: &Node) -> Arc<OwnedNodeRef> {
    let bytes = wacore_binary::marshal::marshal(node).expect("marshal should succeed");
    // marshal() prepends a leading format byte; OwnedNodeRef::new expects raw protocol bytes
    {
        let mut bytes = bytes;
        bytes.remove(0);
        Arc::new(OwnedNodeRef::new(bytes).expect("OwnedNodeRef::new should succeed"))
    }
}

pub async fn wait_for_lock_waiter(lock: &Arc<async_lock::Mutex<()>>, baseline: usize) {
    poll_until("a task to reach the contested lock", || {
        Arc::strong_count(lock) > baseline
    })
    .await;
}

/// Polls `cond` until it holds, panicking once a bounded deadline passes.
///
/// A fixed sleep can only be wrong: long enough to be reliable makes the suite
/// slow, short enough to be fast makes it flaky on a loaded CI runner. Polling
/// the real condition is both fast on the happy path and stable under load.
pub async fn poll_until(what: &str, mut cond: impl FnMut() -> bool) {
    const DEADLINE: std::time::Duration = std::time::Duration::from_secs(5);
    const STEP: std::time::Duration = std::time::Duration::from_millis(2);
    // Most waits here are satisfied by letting a sibling task run once, so yield
    // first and only fall back to a timed step for waits that need real elapsed
    // time (and would otherwise burn a core for the whole deadline).
    const YIELDS: u32 = 64;

    // Tokio's clock, not the wall clock: the timed step below is a tokio sleep, so
    // measuring the deadline on the same clock keeps the two in agreement even
    // under `tokio::time::pause()`.
    let started = tokio::time::Instant::now();
    let mut spins = 0u32;
    loop {
        if cond() {
            return;
        }
        assert!(
            started.elapsed() < DEADLINE,
            "timed out after {DEADLINE:?} waiting for {what}",
        );
        if spins < YIELDS {
            spins += 1;
            tokio::task::yield_now().await;
        } else {
            tokio::time::sleep(STEP).await;
        }
    }
}

/// Waits until every task tracked by the client's outbound flush scope has run.
///
/// Retry receipts, PDO fallbacks and transport acks are all spawned there, and
/// the guard is taken synchronously by the spawn, so a drained scope proves the
/// spawned side effect completed — including the cases where it must
/// deliberately do nothing.
pub async fn wait_for_outbound_tasks(client: &Arc<Client>) {
    poll_until("the outbound flush scope to drain", || {
        client.outbound_flush.pending() == 0
    })
    .await;
}

/// Waits until at least `count` tasks are parked on `notifier`.
///
/// Every waiter in this codebase registers its listener before re-checking the
/// condition it waits on, so a registered listener is proof the task reached its
/// await point — the observable a "still waiting" assertion needs instead of a
/// sleep long enough to hope the scheduler got there.
pub async fn wait_for_notifier_listeners(notifier: &event_listener::Event, count: usize) {
    poll_until(&format!("{count} task(s) parked on the notifier"), || {
        notifier.total_listeners() >= count
    })
    .await;
}

use crate::http::{HttpClient, HttpRequest, HttpResponse};
use crate::runtime_impl::TokioRuntime;
use crate::store::SqliteStore;
use crate::store::persistence_manager::PersistenceManager;
use crate::store::traits::Backend;
use crate::transport::mock::MockTransportFactory;

#[derive(Debug, Clone, Default)]
pub struct MockHttpClient;

#[async_trait::async_trait]
impl HttpClient for MockHttpClient {
    async fn execute(&self, _request: HttpRequest) -> Result<HttpResponse, anyhow::Error> {
        Ok(HttpResponse {
            status_code: 200,
            body: Vec::new(),
        })
    }
}

#[derive(Debug, Clone, Default)]
pub struct FailingMockHttpClient;

#[async_trait::async_trait]
impl HttpClient for FailingMockHttpClient {
    async fn execute(&self, _request: HttpRequest) -> Result<HttpResponse, anyhow::Error> {
        Err(anyhow::anyhow!("Not implemented"))
    }
}

pub async fn create_test_client() -> Arc<Client> {
    create_test_client_with_name("default").await
}

pub async fn create_test_client_with_name(name: &str) -> Arc<Client> {
    create_test_client_with_http(name, Arc::new(MockHttpClient)).await
}

pub async fn create_test_client_with_failing_http(name: &str) -> Arc<Client> {
    create_test_client_with_http(name, Arc::new(FailingMockHttpClient)).await
}

/// Build an isolated in-memory test client backed by the given HTTP client.
pub async fn create_test_client_with_http(
    name: &str,
    http_client: Arc<dyn HttpClient>,
) -> Arc<Client> {
    create_test_client_with_config(
        name,
        http_client,
        crate::cache_config::CacheConfig::default(),
    )
    .await
}

/// Build an isolated in-memory test client with an explicit [`CacheConfig`],
/// e.g. to exercise a non-default [`crate::cache_config::MsgSecretPolicy`].
pub async fn create_test_client_with_config(
    name: &str,
    http_client: Arc<dyn HttpClient>,
    cache_config: crate::cache_config::CacheConfig,
) -> Arc<Client> {
    use portable_atomic::AtomicU64;
    use std::sync::atomic::Ordering;
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let unique_id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let db_name = format!(
        "file:memdb_{}_{}_{}?mode=memory&cache=shared",
        name,
        unique_id,
        std::process::id()
    );

    let backend = Arc::new(
        SqliteStore::new(&db_name)
            .await
            .expect("test backend should initialize"),
    ) as Arc<dyn Backend>;

    create_test_client_from_backend(backend, http_client, cache_config).await
}

pub async fn create_test_client_with_backend(backend: Arc<dyn Backend>) -> Arc<Client> {
    create_test_client_from_backend(
        backend,
        Arc::new(MockHttpClient),
        crate::cache_config::CacheConfig::default(),
    )
    .await
}

async fn create_test_client_from_backend(
    backend: Arc<dyn Backend>,
    http_client: Arc<dyn HttpClient>,
    cache_config: crate::cache_config::CacheConfig,
) -> Arc<Client> {
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("persistence manager should initialize"),
    );

    let (client, _rx) = Client::new_with_cache_config(
        Arc::new(TokioRuntime),
        pm,
        Arc::new(MockTransportFactory::new()),
        http_client,
        None,
        cache_config,
    )
    .await;

    // Tests exercise live-path semantics by default (a fresh client starts in
    // drain mode: 1-permit semaphore, inbound commits batch instead of
    // dispatching immediately). Drain-specific tests re-enter drain state
    // themselves.
    client.enter_live_mode_for_tests();

    client
}

pub async fn seed_peer_session(client: &Arc<Client>, peer: &Jid) {
    use wacore::libsignal::protocol::{
        IdentityKeyPair, KeyPair, PreKeyBundle, SignalProtocolError, UsePQRatchet,
        process_prekey_bundle,
    };
    use wacore::types::jid::JidExt;

    let bundle = tokio::task::spawn_blocking(|| -> Result<PreKeyBundle, SignalProtocolError> {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let receiver = IdentityKeyPair::generate(&mut rng);
        let spk = KeyPair::generate(&mut rng);
        let opk = KeyPair::generate(&mut rng);
        let signature = receiver
            .private_key()
            .calculate_signature(&spk.public_key.serialize(), &mut rng)?;
        PreKeyBundle::new(
            1,
            1u32.into(),
            Some((1u32.into(), opk.public_key)),
            1u32.into(),
            spk.public_key,
            signature.to_vec(),
            *receiver.identity_key(),
        )
    })
    .await
    .expect("bundle task")
    .expect("bundle");

    let mut adapter = client.signal_adapter().await;
    let mut rng = rand::make_rng::<rand::rngs::StdRng>();
    process_prekey_bundle(
        &peer.to_protocol_address(),
        &mut adapter.session_store,
        &mut adapter.identity_store,
        &bundle,
        &mut rng,
        UsePQRatchet::No,
    )
    .await
    .expect("peer session");
}

use std::sync::Mutex;
use wacore::types::events::{Event, EventHandler};

#[derive(Default)]
pub struct TestEventCollector {
    events: Mutex<Vec<Arc<Event>>>,
}

impl EventHandler for TestEventCollector {
    fn handle_event(&self, event: Arc<Event>) {
        self.events
            .lock()
            .expect("collector mutex should not be poisoned")
            .push(event);
    }
}

impl TestEventCollector {
    pub fn events(&self) -> Vec<Arc<Event>> {
        self.events
            .lock()
            .expect("collector mutex should not be poisoned")
            .clone()
    }
}

pub async fn create_test_backend() -> Arc<dyn Backend> {
    use portable_atomic::AtomicU64;
    use std::sync::atomic::Ordering;
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let unique_id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let db_name = format!(
        "file:memdb_backend_{}_{}?mode=memory&cache=shared",
        unique_id,
        std::process::id()
    );

    Arc::new(
        SqliteStore::new(&db_name)
            .await
            .expect("test backend should initialize"),
    ) as Arc<dyn Backend>
}

/// A test client that can complete a full IQ round trip: outgoing frames are
/// captured (and decodable with [`decode_sent_iq`]) and responses are injected
/// with [`answer_iq`], with no server or socket involved.
#[cfg(test)]
pub(crate) async fn create_iq_test_client() -> (
    Arc<Client>,
    Arc<crate::transport::mock::CapturingMockTransport>,
) {
    use crate::transport::mock::CapturingMockTransportFactory;
    use wacore::handshake::NoiseCipher;

    let backend = create_test_backend().await;
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("persistence manager should initialize"),
    );
    let factory = CapturingMockTransportFactory::new();
    let transport = factory.transport();
    let (client, _sync_rx) = Client::new(
        Arc::new(TokioRuntime),
        pm,
        Arc::new(factory),
        Arc::new(MockHttpClient),
        None,
    )
    .await;

    let noise_socket = crate::socket::NoiseSocket::new(
        Arc::new(TokioRuntime),
        transport.clone() as Arc<dyn crate::transport::Transport>,
        NoiseCipher::new(&[0u8; 32]).expect("32-byte key"),
        NoiseCipher::new(&[0u8; 32]).expect("32-byte key"),
    );
    *client.noise_socket.lock().await = Some(Arc::new(noise_socket));
    client.set_connected_for_test(true);
    client
        .is_running
        .store(true, std::sync::atomic::Ordering::Release);
    client.enter_live_mode_for_tests();

    (client, transport)
}

/// Wait for the client to write the `index`-th frame and decode it.
///
/// Frames are encrypted with the counter-based noise cipher, so the index is
/// part of the key material and frames must be read in order. This assumes the
/// client under test writes nothing the test did not ask for: a stray frame
/// (a keepalive, a background query) shifts every later index and the decrypt
/// fails. The harness sends no keepalives, so a test that only drives explicit
/// calls holds that assumption.
#[cfg(test)]
pub(crate) async fn decode_sent_iq(
    transport: &Arc<crate::transport::mock::CapturingMockTransport>,
    index: usize,
) -> Arc<OwnedNodeRef> {
    use wacore::handshake::NoiseCipher;

    let frame = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let frames = transport.sent();
            if let Some(frame) = frames.get(index) {
                return frame.clone();
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("frame {index} should reach the transport"));

    let cipher = NoiseCipher::new(&[0u8; 32]).expect("32-byte key");
    let mut buf = frame[3..].to_vec();
    cipher
        .decrypt_in_place_with_counter(index as u32, &mut buf)
        .expect("captured frame should decrypt");
    // The decrypted payload keeps marshal()'s leading format byte.
    Arc::new(OwnedNodeRef::new(buf[1..].to_vec()).expect("captured frame should decode"))
}

/// Deliver `response` to the pending IQ waiter registered under `request_id`.
#[cfg(test)]
pub(crate) async fn answer_iq(client: &Arc<Client>, request_id: &str, response: &Node) {
    let sender = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if let Some(sender) = client.response_waiters_guard().remove(request_id) {
                return sender;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("an IQ waiter should be registered for {request_id}"));

    let _ = sender.send(node_to_owned_ref(response));
}

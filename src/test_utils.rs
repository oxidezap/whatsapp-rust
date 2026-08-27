use std::sync::Arc;

use crate::Client;
use wacore_binary::{Jid, Node, OwnedNodeRef};

/// Marshal a `Node` into an `Arc<OwnedNodeRef>` for use in tests, through the
/// same unpack step the receive path takes.
pub fn node_to_owned_ref(node: &Node) -> Arc<OwnedNodeRef> {
    let packed = wacore_binary::marshal::marshal(node).expect("marshal should succeed");
    let node_bytes = wacore_binary::util::unpack(&packed).expect("packed payload");
    Arc::new(OwnedNodeRef::new(node_bytes.into_owned()).expect("OwnedNodeRef::new should succeed"))
}

/// Records every [`Event::SentFrame`]
/// it is dispatched, keeping the `Bytes` it arrived with so a test can check the
/// pointer as well as the contents.
#[cfg(test)]
#[derive(Default)]
pub(crate) struct SentFrameRecorder {
    frames: Mutex<Vec<bytes::Bytes>>,
}

#[cfg(test)]
impl SentFrameRecorder {
    pub(crate) fn frames(&self) -> Vec<bytes::Bytes> {
        self.frames.lock().expect("recorded frames mutex").clone()
    }
}

#[cfg(test)]
impl EventHandler for SentFrameRecorder {
    fn handle_event(&self, event: Arc<Event>) {
        if let Event::SentFrame(sent) = &*event {
            self.frames
                .lock()
                .expect("recorded frames mutex")
                .push(sent.plaintext.clone());
        }
    }

    fn interest(&self) -> EventInterest {
        EventInterest::of(&[EventKind::SentFrame])
    }
}

/// Decrypts captured noise frames under the counter each one's position implies,
/// yielding the plaintexts in wire order.
///
/// Takes what [`CapturingMockTransport::sent`] yields: one framed frame per
/// entry, prefix intact, already split out of whatever writes carried them. The
/// length is checked rather than reparsed, so a caller passing raw writes (where
/// one entry can hold several frames) fails here instead of decrypting garbage.
///
/// The counter is the AES-GCM nonce, so this doubles as an order check: a frame
/// read out of position cannot authenticate.
///
/// [`CapturingMockTransport::sent`]: crate::transport::mock::CapturingMockTransport::sent
#[cfg(test)]
pub(crate) fn decrypt_wire_frames(frames: &[bytes::Bytes], key: &[u8; 32]) -> Vec<Vec<u8>> {
    use wacore::framing::FRAME_LENGTH_SIZE;
    use wacore::noise::NoiseCipher;

    let cipher = NoiseCipher::new(key).expect("32-byte key");
    let mut plaintexts = Vec::with_capacity(frames.len());
    for (counter, frame) in frames.iter().enumerate() {
        let mut declared = 0usize;
        for byte in &frame[..FRAME_LENGTH_SIZE] {
            declared = (declared << 8) | *byte as usize;
        }
        assert_eq!(
            declared,
            frame.len() - FRAME_LENGTH_SIZE,
            "expected one frame per entry, as `sent()` yields them"
        );
        let mut body = bytes::BytesMut::from(&frame[FRAME_LENGTH_SIZE..]);
        cipher
            .decrypt_in_place_with_counter(counter as u32, &mut body)
            .expect("each captured frame must decrypt under its own counter");
        plaintexts.push(body.to_vec());
    }
    plaintexts
}

/// The [`crate::request::IqError::ServerError`] an `<iq type="error">` carrying these
/// attributes produces, built by the same parse the receive path runs so a test never
/// states the variant's shape by hand.
pub fn server_error_iq(
    code: u16,
    text: &str,
    error_type: Option<&str>,
    backoff: Option<u32>,
) -> crate::request::IqError {
    use wacore_binary::builder::NodeBuilder;

    let mut error = NodeBuilder::new("error")
        .attr("code", code.to_string())
        .attr("text", text);
    if let Some(error_type) = error_type {
        error = error.attr("type", error_type);
    }
    if let Some(backoff) = backoff {
        error = error.attr("backoff", backoff.to_string());
    }
    let response = node_to_owned_ref(
        &NodeBuilder::new("iq")
            .attr("type", "error")
            .children([error.build()])
            .build(),
    );
    let err = wacore::request::RequestUtils::new("test".to_string())
        .parse_iq_response(response.get())
        .expect_err("an error stanza must not classify as a result");
    crate::request::IqError::from_response(err, &response)
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

/// Build an isolated in-memory test client over a caller-supplied transport
/// factory, for tests that need to decide when — or whether — a connection
/// attempt gets as far as a socket.
#[cfg(test)]
pub(crate) async fn create_test_client_with_transport_factory(
    transport_factory: Arc<dyn crate::transport::TransportFactory>,
) -> Arc<Client> {
    let pm = Arc::new(
        PersistenceManager::new(create_test_backend().await)
            .await
            .expect("persistence manager should initialize"),
    );

    let (client, _rx) = Client::new(
        Arc::new(TokioRuntime),
        pm,
        transport_factory,
        Arc::new(MockHttpClient),
        None,
    )
    .await;

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
            signature,
            *receiver.identity_key(),
        )
    })
    .await
    .expect("bundle task")
    .expect("bundle");

    let mut adapter = client.signal_adapter();
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
use wacore::types::events::{Event, EventHandler, EventInterest, EventKind};

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

    // Wired to the client's own observers like the real socket is, so per-frame
    // bookkeeping and sent-frame forwarding are part of what tests observe.
    let noise_socket = crate::socket::NoiseSocket::with_observers(
        Arc::new(TokioRuntime),
        transport.clone() as Arc<dyn crate::transport::Transport>,
        NoiseCipher::new(&[0u8; 32]).expect("32-byte key"),
        NoiseCipher::new(&[0u8; 32]).expect("32-byte key"),
        crate::socket::noise_socket::SendObservers::with_stats(client.stats.clone())
            .with_sent_frames(client.sent_frame_tap.clone()),
    );
    *client.noise_socket.lock().unwrap() = Some(Arc::new(noise_socket));
    client.set_connected_for_test(true);
    client
        .is_running
        .store(true, std::sync::atomic::Ordering::Release);
    client.enter_live_mode_for_tests();

    (client, transport)
}

/// Buffers what the client announced, so a test can assert that a branch's log
/// describes what the branch actually did.
///
/// One capture for the whole process, because `log`'s global logger is
/// install-once and every asserting test would otherwise race for the slot.
/// Which targets it buffers is one decision, made in `CAPTURED_TARGETS`:
/// filtering at `enabled()` keeps an installed capture from holding the entire
/// suite's logging for the lifetime of the process.
pub mod log_capture {
    use std::sync::{LazyLock, Mutex, OnceLock};

    /// The targets tests assert on. A target absent from here is not buffered,
    /// so [`Session::records_for`] would report it as silent — add it before
    /// asserting on it.
    const CAPTURED_TARGETS: &[&str] = &[
        "whatsapp_rust::client::lifecycle",
        "whatsapp_rust::message::receive",
    ];

    #[derive(Default)]
    struct Capture {
        records: Mutex<Vec<(&'static str, log::Level, String)>>,
    }

    impl log::Log for Capture {
        fn enabled(&self, metadata: &log::Metadata<'_>) -> bool {
            CAPTURED_TARGETS.contains(&metadata.target())
        }

        fn log(&self, record: &log::Record<'_>) {
            // Keyed by the `&'static str` from the list rather than the
            // record's borrow, so a buffered entry owns nothing but its message.
            let Some(target) = CAPTURED_TARGETS
                .iter()
                .find(|target| **target == record.target())
            else {
                return;
            };
            self.records
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .push((target, record.level(), record.args().to_string()));
        }

        fn flush(&self) {}
    }

    static CAPTURE: LazyLock<Capture> = LazyLock::new(Capture::default);

    /// `true` once this process's logger is the capture above. Idempotent: the
    /// install is attempted once per process and the verdict cached.
    fn installed() -> bool {
        static INSTALLED: OnceLock<bool> = OnceLock::new();
        *INSTALLED.get_or_init(|| {
            let installed = log::set_logger(&*CAPTURE).is_ok();
            if installed {
                log::set_max_level(log::LevelFilter::Trace);
            }
            installed
        })
    }

    const CHILD_MARKER: &str = "WA_RUST_OWNS_LOG_CAPTURE";

    /// Whether this process is running exactly one test, which is what makes an
    /// assertion about the buffer an assertion about the test that filled it.
    ///
    /// The sink is process-wide and its targets are generic — half the suite
    /// builds a client and disconnects it — so a sibling test running
    /// concurrently contributes records indistinguishable from the caller's.
    /// Serialising the *asserting* tests is not enough, because the siblings
    /// that pollute never ask for the lock. Isolation is the only guarantee
    /// that holds, so it is required rather than hoped for.
    ///
    /// `cargo nextest`, which is what CI runs, gives every test its own process
    /// and says so in the environment. Everything else — a threaded
    /// `cargo test --lib` above all — earns isolation by re-running in a child.
    fn runs_this_test_alone() -> bool {
        std::env::var_os(CHILD_MARKER).is_some() || std::env::var_os("NEXTEST").is_some()
    }

    /// Hands a test's log assertions to a fresh process unless this one is
    /// already running that test alone and owns the `log` slot.
    ///
    /// Two things can be missing: isolation (see
    /// [`runs_this_test_alone`]) and the logger itself, which is install-once
    /// and may have been claimed by a sibling test's `env_logger`. A child
    /// process supplies both. Skipping the assertions instead would let a
    /// regression in the very behaviour under test go unseen.
    ///
    /// `true` means the caller should return immediately: the child already ran
    /// the real assertions and its failure, if any, has been propagated.
    pub fn delegated_to_child(test_name: &str) -> bool {
        // The order of this `&&` is load-bearing, not incidental: `installed()`
        // is what replaces the process's logger, and this capture forwards
        // nothing it does not buffer. Installed in a process running the whole
        // suite, it would swallow every other test's logging for the rest of
        // the run. Only a process running this one test may reach it.
        if runs_this_test_alone() && installed() {
            return false;
        }
        assert!(
            std::env::var_os(CHILD_MARKER).is_none(),
            "{test_name}: a single-test child must own the log capture, and did not",
        );
        let output =
            std::process::Command::new(std::env::current_exe().expect("running test binary path"))
                .args([test_name, "--exact", "--test-threads=1"])
                .env(CHILD_MARKER, "1")
                .output()
                .expect("re-running the test in its own process");
        // libtest exits 0 for a filter that matched nothing, so accepting the
        // exit code alone would let a stale `test_name` (after a rename, say)
        // restore the very skip this helper exists to remove. Demand the child's
        // own result line for the test we asked it to run.
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            output.status.success() && stdout.contains(&format!("test {test_name} ... ok")),
            "{test_name} did not run and pass in its own process:\n{stdout}{}",
            String::from_utf8_lossy(&output.stderr),
        );
        true
    }

    /// Exclusive use of the capture, and an empty buffer to start from.
    ///
    /// What makes "nothing announced a reconnect" a statement about the test's
    /// own client is [`delegated_to_child`]: the assertions only ever run in a
    /// process running this one test, so nothing else can be logging. The lock
    /// here covers the remaining case within such a process — a test that
    /// itself asserts twice — and the empty buffer keeps a record from before
    /// the client existed out of the answer. Records are readable only through
    /// this, so there is no way to assert without both.
    pub struct Session(#[allow(dead_code)] std::sync::MutexGuard<'static, ()>);

    impl Session {
        /// Every record captured on `target` since this session began, oldest
        /// first.
        pub fn records_for(&self, target: &str) -> Vec<(log::Level, String)> {
            CAPTURE
                .records
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .iter()
                .filter(|(captured, _, _)| *captured == target)
                .map(|(_, level, message)| (*level, message.clone()))
                .collect()
        }
    }

    /// Claim the capture for one test. Call [`delegated_to_child`] first and
    /// return when it says so: this panics rather than let a test assert on a
    /// buffer it does not own, where an uninstalled capture reads as silence
    /// and a sibling test's records read as the caller's.
    pub fn session() -> Session {
        static LOCK: Mutex<()> = Mutex::new(());

        assert!(
            runs_this_test_alone() && installed(),
            "a test may only assert on the log capture in a process running it alone, \
             with the capture installed; call `delegated_to_child` first",
        );
        let guard = LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        CAPTURE
            .records
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clear();
        Session(guard)
    }
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
    // The decrypted payload is a packed payload, exactly as the read loop sees it.
    let node_bytes = wacore_binary::util::unpack(&buf).expect("captured frame should unpack");
    Arc::new(OwnedNodeRef::new(node_bytes.into_owned()).expect("captured frame should decode"))
}

/// Deliver `response` to the pending IQ waiter registered under `request_id`, handing back
/// the very allocation the waiter received so a caller can prove what it got is that one.
#[cfg(test)]
pub(crate) async fn answer_iq(
    client: &Arc<Client>,
    request_id: &str,
    response: &Node,
) -> Arc<OwnedNodeRef> {
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

    #[cfg(feature = "voip-runtime")]
    client.bind_pending_call_link_join_ack(&response.as_node_ref());
    let delivered = node_to_owned_ref(response);
    // Test helper: the map only ever holds Iq waiters in these fixtures.
    if let crate::client::ResponseWaiter::Iq(sender) = sender {
        let _ = sender.send(delivered.clone());
    }
    delivered
}

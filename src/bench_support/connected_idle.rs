//! Connected post-activity fixture. See `benches/connected_idle.md` for measurement boundaries.
//!
//! The Noise transport follows `test_support::call`'s synthetic XX server, but routes all
//! inbound stanzas through encrypted frames and keeps no outgoing/event payload archive.
//! No connection flags are set by this fixture. Unsupported initialization IQs get 503;
//! active and keepalive IQs succeed. This is not full WhatsApp service emulation.

mod util;

use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result, ensure};
use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use wacore::handshake::{NoiseCipher, NoiseHandshake};
use wacore::libsignal::protocol::{KeyPair, SenderKeyName};
use wacore::store::traits::Backend;
use wacore::types::events::{Event, EventHandler, Subscription};
use wacore::types::jid::{JidExt, make_sender_key_name};
use wacore_binary::consts::{NOISE_PATTERN_XX, WA_CONN_HEADER};
use wacore_binary::{Jid, Node, NodeRef, OwnedNodeRef, Server, builder::NodeBuilder, marshal};

use crate::Client;
use crate::client::ClientBuilder;
use crate::handshake::NoiseCertPolicy;
use crate::http::{HttpClient, HttpRequest, HttpResponse};
use crate::runtime_impl::TokioRuntime;
use crate::store::commands::DeviceCommand;
use crate::store::persistence_manager::PersistenceManager;
use crate::transport::{Transport, TransportEvent, TransportFactory};
use crate::types::durability_hook::{InboundDurabilityHook, InboundMessage};
use crate::waproto::whatsapp as wa;

/// A private real SQLite file per process/sample, never `:memory:`. Keeping this
/// guard outside Session also keeps database-path allocations stable across checkpoints.
pub struct BackendFixture {
    backend: Option<Arc<dyn Backend>>,
    #[cfg(feature = "sqlite-storage")]
    path: Option<std::path::PathBuf>,
}

impl BackendFixture {
    pub fn memory() -> Self {
        Self {
            backend: Some(Arc::new(wacore::store::InMemoryBackend::new())),
            #[cfg(feature = "sqlite-storage")]
            path: None,
        }
    }

    #[cfg(feature = "sqlite-storage")]
    pub async fn sqlite() -> Result<Self> {
        Self::sqlite_with_synchronous(crate::store::Synchronous::Normal).await
    }

    #[cfg(feature = "sqlite-storage")]
    async fn sqlite_with_synchronous(synchronous: crate::store::Synchronous) -> Result<Self> {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let directory = std::env::var_os("CARGO_TARGET_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("target"))
            .join("connected-idle");
        std::fs::create_dir_all(&directory)?;
        let path = directory.join(format!(
            "{}-{}.db",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        ensure!(
            !path.exists(),
            "refusing to reuse a fixture database: {}",
            path.display()
        );
        let backend = Self::open_sqlite(&path, synchronous).await?;
        Ok(Self {
            backend: Some(Arc::new(backend)),
            path: Some(path),
        })
    }

    #[cfg(feature = "sqlite-storage")]
    async fn open_sqlite(
        path: &std::path::Path,
        synchronous: crate::store::Synchronous,
    ) -> Result<crate::store::SqliteStore> {
        use crate::store::{SqliteDatabase, SqliteDatabaseConfig};

        let config = SqliteDatabaseConfig {
            synchronous,
            ..Default::default()
        };
        let database =
            SqliteDatabase::open(path.to_str().context("SQLite path encoding")?, config).await?;
        Ok(database.store(1))
    }

    pub fn backend(&self) -> Arc<dyn Backend> {
        self.backend.as_ref().expect("live fixture backend").clone()
    }

    /// Release the backend before unlinking its owned files; failures invalidate
    /// a measurement rather than being silently reported as successful cleanup.
    pub fn cleanup(mut self) -> Result<()> {
        drop(self.backend.take());
        self.cleanup_files()
    }

    fn cleanup_files(&mut self) -> Result<()> {
        #[cfg(feature = "sqlite-storage")]
        if let Some(path) = &self.path {
            util::cleanup_database(path)?;
            self.path = None;
        }
        Ok(())
    }
}

impl Drop for BackendFixture {
    // Even cancellation/unwinding must make fixture cleanup failures visible
    // without relying on an application-installed logger.
    #[allow(clippy::print_stderr)]
    fn drop(&mut self) {
        drop(self.backend.take());
        if let Err(error) = self.cleanup_files() {
            eprintln!("connected idle fixture cleanup failed: {error:#}");
        }
    }
}

pub const IDLE: Duration = Duration::from_secs(61);
pub const LANES: usize = 32;
pub const MESSAGES: usize = 256;
/// One offline publication, then one live publication per remaining message.
pub const COMMIT_BATCHES: usize = 1 + MESSAGES - LANES;
pub const TEXT_BYTES: usize = 4096;
pub const HISTORY_MESSAGES: usize = 256;
/// Fixture-only sender-key/signature randomness, never for a shipping client.
pub const WORKLOAD_SEED: u64 = 0x434f_4e4e_4543_5431;
const DEADLINE: Duration = Duration::from_secs(30);

#[derive(Default)]
struct Counts {
    messages: AtomicUsize,
    history: AtomicUsize,
    committed: AtomicUsize,
    commit_batches: AtomicUsize,
}

impl EventHandler for Counts {
    fn handle_event(&self, event: Arc<Event>) {
        match &*event {
            Event::Messages(batch) => {
                self.messages
                    .fetch_add(batch.messages.len(), Ordering::Relaxed);
            }
            Event::HistorySync(_) => {
                self.history.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
    }
}

// The library's durable pending-inbound store is real. The external consumer is a
// no-op: this fixture measures library retention, not a second application database.
struct Commit(Arc<Counts>);
#[async_trait]
impl InboundDurabilityHook for Commit {
    async fn on_messages(&self, _: Arc<Client>, batch: &[InboundMessage]) -> Result<()> {
        self.0.committed.fetch_add(batch.len(), Ordering::Relaxed);
        self.0.commit_batches.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

/// Prepared ciphertext/history input, consumed by activity so it cannot contaminate idle samples.
pub struct Activity {
    stanzas: Vec<Node>,
    history: wa::message::HistorySyncNotification,
}

pub struct FirstDm {
    stanza: Option<Node>,
    events: async_channel::Receiver<Arc<Event>>,
    _subscription: Subscription,
}

struct FirstDmEvents(async_channel::Sender<Arc<Event>>);

impl EventHandler for FirstDmEvents {
    fn interest(&self) -> wacore::types::events::EventInterest {
        use wacore::types::events::{EventInterest, EventKind};
        EventInterest::of(&[EventKind::Messages])
    }

    fn handle_event(&self, event: Arc<Event>) {
        self.0.try_send(event).expect("first DM event queue");
    }
}

const FIRST_DM_TEXT: &str = "first message";
const FIRST_DM_ID: &str = "IDLE-0";

/// Wall-clock timings for a file-backed first DM, in microseconds.
#[cfg(feature = "sqlite-storage")]
#[derive(Debug, serde::Serialize)]
pub struct SqliteFirstDm {
    pub receive_us: u128,
    pub signal_flush_us: u128,
    pub reopen_and_read_us: u128,
}

#[cfg(feature = "sqlite-storage")]
pub async fn sqlite_first_dm() -> Result<SqliteFirstDm> {
    sqlite_first_dm_with_pause(std::future::ready(())).await
}

#[cfg(feature = "sqlite-storage")]
async fn sqlite_first_dm_with_pause(
    before_flush: impl Future<Output = ()>,
) -> Result<SqliteFirstDm> {
    use crate::store::{SignalStore, Synchronous};
    use wacore::time::Instant;

    let mut backend = BackendFixture::sqlite_with_synchronous(Synchronous::Full).await?;
    let session = Session::connect(backend.backend()).await?;
    session.finish_control().await?;
    session.prepare_maintenance_sample().await?;
    let mut input = session.prepare_first_dm().await?;
    let peer = Jid::new(super::PEER_USER, Server::Pn).to_protocol_address();

    // The live coalescer takes the same gate, so it cannot persist the first
    // receive before the timed explicit flush.
    let flush_gate = session.client.signal_flush_lifecycle.lock().await;
    let start = Instant::now();
    session.receive_first_dm(&mut input).await?;
    let receive_us = start.elapsed().as_micros();
    before_flush.await;
    ensure!(
        backend
            .backend()
            .get_session(peer.as_str())
            .await?
            .is_none(),
        "background write reached SQLite before the explicit flush"
    );
    log::debug!("first DM Signal flush started");
    let start = Instant::now();
    session.client.flush_pending_signal_state().await?;
    let signal_flush_us = start.elapsed().as_micros();
    drop(flush_gate);
    log::debug!("first DM Signal flush completed");
    let saved = backend
        .backend()
        .get_session(peer.as_str())
        .await?
        .context("Signal flush did not save the first DM session")?;

    drop(input);
    session.shutdown().await?;
    drop(backend.backend.take());
    let start = Instant::now();
    let path = backend.path.as_deref().context("SQLite fixture path")?;
    let reopened = BackendFixture::open_sqlite(path, Synchronous::Full).await?;
    let restored = reopened
        .get_session(peer.as_str())
        .await?
        .context("first DM session missing after reopen")?;
    let reopen_and_read_us = start.elapsed().as_micros();
    ensure!(saved == restored, "Signal record changed after reopen");
    let record = wacore::libsignal::protocol::SessionRecord::deserialize(&restored)?;
    ensure!(
        record.session_state().is_some(),
        "restored Signal record has no session"
    );
    drop(reopened);
    backend.cleanup()?;

    Ok(SqliteFirstDm {
        receive_us,
        signal_flush_us,
        reopen_and_read_us,
    })
}

/// Initialization counters taken before a batched maintenance sample.
#[derive(Debug, Clone, Copy)]
pub struct MaintenanceSample {
    started: usize,
    initialization_iqs: usize,
}

/// Only lifecycle observables, not a replacement for heap or process measurements.
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct Checkpoint {
    pub connected: bool,
    pub lanes: usize,
    pub open_lanes: usize,
    pub running_workers: usize,
    pub messages: usize,
    pub committed: usize,
    pub commit_batches: usize,
    pub history: usize,
}

pub struct Session {
    client: Arc<Client>,
    wire: Arc<Wire>,
    counts: Arc<Counts>,
    _subscription: Subscription,
    reader: Option<tokio::task::JoinHandle<Result<()>>>,
}

impl Session {
    /// Connect with production XX, login and keepalive, leaving the initial offline
    /// marker to `receive_activity`/`finish_control`. Requires a running Tokio runtime.
    pub async fn connect(backend: Arc<dyn Backend>) -> Result<Self> {
        Self::connect_with_capture(backend, None).await
    }

    async fn connect_with_capture(
        backend: Arc<dyn Backend>,
        outgoing_capture: Option<OutgoingCapture>,
    ) -> Result<Self> {
        let pm = Arc::new(PersistenceManager::new(backend).await?);
        // A valid fixed-width synthetic registration ID. Random varint widths
        // otherwise change the retained serialized Signal session by a byte.
        pm.modify_device(|device| device.registration_id = 0x1234_5678)
            .await;
        for command in [
            DeviceCommand::SetId(Some(Jid::new(super::OWN_USER, Server::Pn).with_device(1))),
            DeviceCommand::SetLid(Some(
                Jid::new("100000000000001", Server::Lid).with_device(1),
            )),
            DeviceCommand::SetPushName("Synthetic idle fixture".into()),
            DeviceCommand::SetAccount(Some({
                let mut proto_value = wa::ADVSignedDeviceIdentity::default();
                proto_value.details = Some(vec![0; 32]);
                proto_value.account_signature_key = Some(vec![0; 32]);
                proto_value.account_signature = Some(vec![0; 64]);
                proto_value.device_signature = Some(vec![0; 64]);
                proto_value
            })),
        ] {
            pm.process_command(command).await;
        }
        let (server_tx, server_rx) = async_channel::bounded(256);
        let wire = Arc::new(Wire {
            state: Mutex::new(State::Hello),
            server_tx,
            server_rx,
            active: AtomicUsize::new(0),
            pongs: AtomicUsize::new(0),
            initialization_iqs: AtomicUsize::new(0),
            delivery_receipts: Mutex::new([false; MESSAGES]),
            max_batch_frames: AtomicUsize::new(0),
            outgoing_capture: outgoing_capture.map(Arc::new),
        });
        let counts = Arc::new(Counts::default());
        let client = ClientBuilder::new()
            .with_runtime(TokioRuntime)
            .with_persistence_manager(pm)
            .with_transport_factory(Factory(wire.clone()))
            .with_http_client(NoHttp)
            .with_version_override((2, 3000, 0))
            .with_noise_cert_policy(NoiseCertPolicy::DangerSkipCertChainVerify)
            .with_inbound_durability_hook(Commit(counts.clone()))
            .build()
            .await?
            .into_client();
        let subscription = client.subscribe_handler(counts.clone());
        let reader = tokio::spawn({
            let client = client.clone();
            async move {
                let connection = client.connect().await?;
                connection.read_until_disconnected().await;
                Ok(())
            }
        });
        let session = Self {
            client,
            wire,
            counts,
            _subscription: subscription,
            reader: Some(reader),
        };
        // Fully-ready waits include offline completion, deliberately withheld
        // here so activity exercises the real initial drain before publication.
        session
            .wait_until(|| session.wire.active.load(Ordering::Relaxed) > 0)
            .await?;
        ensure!(
            session.client.is_socket_connected(),
            "login lost its connection"
        );
        Ok(session)
    }

    async fn prepare_encrypted_texts(
        &self,
        message_count: usize,
        text_bytes: usize,
    ) -> Result<(Vec<Node>, Vec<Jid>, Jid, String)> {
        // The active IQ does not join passive prekey generation/upload. Fence
        // it before preparing input, so setup cannot pay a variable fraction
        // of that work and leave the rest inside the activity measurement.
        // The post-login parent still waits for our withheld offline marker.
        tokio::time::timeout(DEADLINE, self.client.bench_startup.wait_before_offline())
            .await
            .context("pre-offline startup tasks did not finish")?;
        use wacore::libsignal::protocol::{create_sender_key_distribution_message, group_encrypt};
        super::seed_registry(&self.client, super::PEER_USER, &[0]).await;
        super::seed_registry(&self.client, super::OWN_USER, &[0, 1]).await;
        let peer_jid = Jid::new(super::PEER_USER, Server::Pn);
        let peer = super::establish_acknowledged_session(&self.client, &peer_jid).await;
        let groups: Vec<Jid> = (0..LANES)
            .map(|i| format!("12036300000{i:05}@g.us").parse())
            .collect::<std::result::Result<_, _>>()?;
        let mut keys: Vec<SenderKeyName> = Vec::with_capacity(LANES);
        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::seed_from_u64(WORKLOAD_SEED);
        let mut adapter = peer.signal_adapter();
        for (i, group) in groups.iter().enumerate() {
            let key = make_sender_key_name(group, &peer_jid.to_protocol_address());
            let skdm = create_sender_key_distribution_message(
                &key,
                &mut adapter.sender_key_store,
                &mut rng,
            )
            .await?;
            self.client
                .handle_sender_key_distribution_message(
                    group,
                    &peer_jid,
                    &format!("idle-skdm-{i}"),
                    skdm.serialized(),
                )
                .await?;
            keys.push(key);
        }
        let text = "x".repeat(text_bytes);
        let mut plaintext = waproto::codec::message_to_vec(&{
            let mut proto_value = wa::Message::default();
            proto_value.conversation = Some(text.clone());
            proto_value
        });
        // Fixed valid v2 padding keeps the AES scratch footprint identical.
        // Production padding remains random; only this synthetic input is fixed.
        plaintext.resize(plaintext.len() + 16, 16);
        let mut stanzas = Vec::with_capacity(message_count);
        for i in 0..message_count {
            let ciphertext = group_encrypt(
                &mut adapter.sender_key_store,
                &keys[i % LANES],
                &plaintext,
                &mut rng,
            )
            .await?;
            stanzas.push(
                NodeBuilder::new("message")
                    .attr("from", groups[i % LANES].clone())
                    .attr("participant", peer_jid.clone())
                    .attr("id", format!("IDLE-{i}"))
                    .attr("t", wacore::time::now_secs().to_string())
                    .attr("type", "text")
                    .children([NodeBuilder::new("enc")
                        .attr("type", "skmsg")
                        .attr("v", "2")
                        .bytes(ciphertext.serialized().to_vec())
                        .build()])
                    .build(),
            );
        }
        drop(adapter);
        peer.flush_signal_cache().await?;
        self.client.flush_pending_signal_state().await?;
        Ok((stanzas, groups, peer_jid, text))
    }

    /// Established Signal material and synthetic encryption are fixture setup, not
    /// part of the CodSpeed timed region. The peer is dropped before this returns.
    pub async fn prepare_activity(&self) -> Result<Activity> {
        let (stanzas, groups, peer_jid, text) =
            self.prepare_encrypted_texts(MESSAGES, TEXT_BYTES).await?;
        let history = {
            let mut proto_value = wa::HistorySync::default();
            proto_value.sync_type = wa::history_sync::HistorySyncType::INITIAL_BOOTSTRAP;
            proto_value.conversations = groups
                .iter()
                .enumerate()
                .map(|(lane, group)| {
                    let mut proto_value = wa::Conversation::default();
                    proto_value.id = group.to_string();
                    proto_value.messages = (lane..HISTORY_MESSAGES)
                        .step_by(LANES)
                        .map(|i| {
                            let mut proto_value = wa::HistorySyncMsg::default();
                            proto_value.message = buffa::MessageField::some({
                                let mut proto_value = wa::WebMessageInfo::default();
                                proto_value.key = buffa::MessageField::some({
                                    let mut proto_value = wa::MessageKey::default();
                                    proto_value.remote_jid = Some(group.to_string());
                                    proto_value.from_me = Some(false);
                                    proto_value.id = Some(format!("HISTORY-{i}"));
                                    proto_value.participant = Some(peer_jid.to_string());
                                    proto_value
                                });
                                proto_value.message = buffa::MessageField::some({
                                    let mut proto_value = wa::Message::default();
                                    proto_value.conversation = Some(text.clone());
                                    proto_value
                                });
                                proto_value.message_secret = Some(vec![0x44; 32]);
                                proto_value.message_timestamp =
                                    Some(wacore::time::now_secs() as u64);
                                proto_value
                            });
                            proto_value.msg_order_id = Some(i as u64);
                            proto_value
                        })
                        .collect();
                    proto_value
                })
                .collect();
            proto_value
        };
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&waproto::codec::history_sync_to_vec(&history))?;
        let compressed = encoder.finish()?;
        Ok(Activity {
            stanzas,
            history: {
                let mut proto_value = wa::message::HistorySyncNotification::default();
                proto_value.file_length = Some(compressed.len() as u64);
                proto_value.sync_type = Some(wa::message::HistorySyncType::INITIAL_BOOTSTRAP);
                proto_value.initial_hist_bootstrap_inline_payload = Some(compressed);
                proto_value
            },
        })
    }

    /// All encrypted messages enter the real reader, dispatcher and production lane
    /// workers. History enters the production inline-history parser/task (no HTTP).
    /// Finish the server's offline phase and wait for commit/event/receipt work.
    pub async fn receive_activity(&self, activity: Activity) -> Result<()> {
        ensure!(
            self.client.bench_startup.pending_before_offline() == 0,
            "pre-offline startup entered activity measurement"
        );
        let mut stanzas = activity.stanzas.into_iter();
        for node in stanzas.by_ref().take(LANES) {
            self.wire.reply(node).await?;
        }
        // History input is intentionally at the task boundary, as in history_sync's
        // existing inline fixtures. It does not pretend to measure history download.
        let history: wacore::messages::DetachedHistorySyncNotification = activity.history.into();
        let mut tracker = self
            .client
            .begin_history_sync_task(history.inline_payload.as_ref().map_or(0, Bytes::len));
        self.client
            .process_history_sync_task_tracked("IDLE-HISTORY".into(), history, &mut tracker)
            .await;
        drop(tracker);
        // Wire delivery only enqueues work on the chat lanes. Do not let the
        // offline marker overtake decryption of the first wave. Only send the
        // remaining waves after the transition, fixing both publication modes.
        self.wait_until(|| {
            self.client.inbound_commit_batch.pending_stats().0
                + self.counts.committed.load(Ordering::Relaxed)
                == LANES
        })
        .await
        .context("activity did not reach the offline commit boundary")?;
        ensure!(
            self.counts.committed.load(Ordering::Relaxed) == 0,
            "activity timed out into an early offline commit"
        );
        self.finish_offline(LANES).await?;
        ensure!(
            self.counts.commit_batches.load(Ordering::Relaxed) == 1,
            "activity must publish one complete offline batch"
        );
        for node in stanzas {
            self.wire.reply(node).await?;
        }
        self.wait_until(|| self.counts.messages.load(Ordering::Relaxed) == MESSAGES)
            .await?;
        ensure!(
            self.counts.committed.load(Ordering::Relaxed) == MESSAGES,
            "durability hook missed messages"
        );
        ensure!(
            self.counts.history.load(Ordering::Relaxed) == 1,
            "history parsing did not dispatch"
        );
        // Hook publication precedes conditional pending-row cleanup. An empty
        // outbound scope can therefore be observed before receipts are even
        // scheduled. Observe every workload ID on the wire before settling it.
        self.wait_until(|| self.delivery_receipts() == MESSAGES)
            .await
            .context("activity receipts did not finish pending-row cleanup")?;
        self.settle().await?;
        ensure!(
            self.counts.commit_batches.load(Ordering::Relaxed) == COMMIT_BATCHES,
            "activity did not preserve its offline/live publication boundary"
        );
        Ok(())
    }

    pub async fn finish_control(&self) -> Result<()> {
        self.finish_offline(0).await?;
        self.settle().await
    }

    /// Exercise a fixed, completed IQ burst before native retention sampling.
    /// Startup concurrency otherwise leaves variable capacities in the waiter
    /// table and sender batch vector. Their metadata remains allocated and measured.
    pub async fn prepare_retention(&self) -> Result<()> {
        ensure!(
            self.client.bench_startup.pending() == 0,
            "startup must finish before preparing retention"
        );
        use std::future::Future;
        const REQUESTS: usize = 16;
        self.wire.max_batch_frames.store(0, Ordering::Relaxed);
        let replies = futures::future::join_all((0..REQUESTS).map(|i| {
            self.client.send_iq_node(
                NodeBuilder::new("iq")
                    .attr("id", format!("RETENTION-WARM-{i:02}"))
                    .attr("type", "get")
                    .attr("to", "s.whatsapp.net")
                    .attr("xmlns", "w:p")
                    .build(),
                Some(DEADLINE),
            )
        }));
        futures::pin_mut!(replies);
        let mut peak = 0;
        let replies = std::future::poll_fn(|cx| {
            let result = replies.as_mut().poll(cx);
            peak = peak.max(self.client.response_waiters_guard().len());
            result
        })
        .await;
        for reply in replies {
            reply?;
        }
        ensure!(
            peak == REQUESTS,
            "metadata burst did not overlap all IQs: {peak}"
        );
        ensure!(
            self.wire.max_batch_frames.load(Ordering::Relaxed) >= 8,
            "metadata burst did not fill the sender's eight-slot queue"
        );
        self.settle().await
    }

    async fn finish_offline(&self, count: usize) -> Result<()> {
        self.wire
            .reply(
                NodeBuilder::new("ib")
                    .children([NodeBuilder::new("offline")
                        .attr("count", count.to_string())
                        .build()])
                    .build(),
            )
            .await?;
        self.client.wait_for_startup_sync(DEADLINE).await?;
        self.client.wait_for_session_ready(DEADLINE).await?;
        // Connected/empty IQ waiters do not fence startup tasks awaiting storage
        // or not yet polled. Join their finite descendants before measurement.
        tokio::time::timeout(DEADLINE, self.client.bench_startup.wait())
            .await
            .context("finite startup tasks did not finish")?;
        Ok(())
    }

    async fn settle(&self) -> Result<()> {
        self.wait_until(|| self.client.response_waiters_guard().is_empty())
            .await?;
        let outbound = self
            .client
            .outbound_flush
            .flush(&*self.client.runtime, DEADLINE)
            .await;
        ensure!(
            outbound == crate::flush_scope::DrainOutcome::Completed,
            "fixture outbound work did not finish: {outbound:?}"
        );
        self.client.flush_signal_cache_batch_safe().await?;
        self.wait_until(|| self.wire.server_rx.is_empty()).await?;
        tokio::task::yield_now().await;
        Ok(())
    }

    async fn wait_until(&self, ready: impl Fn() -> bool) -> Result<()> {
        util::wait_until(DEADLINE, ready).await
    }

    /// Prepare a prekey message on the sender only. The receiving client has
    /// completed startup and must still establish its first pairwise session.
    pub async fn prepare_first_dm(&self) -> Result<FirstDm> {
        use rand::SeedableRng;
        use wacore::libsignal::protocol::{
            CiphertextMessage, IdentityKey, PreKeyBundle, UsePQRatchet, message_encrypt,
            process_prekey_bundle,
        };
        use wacore::proto_helpers::MessageBuilderExt;

        let sender = Jid::new(super::PEER_USER, Server::Pn);
        super::seed_registry(&self.client, super::PEER_USER, &[0]).await;
        let receiver = self.client.persistence_manager.get_device_snapshot();
        let receiver_jid = receiver.pn.as_ref().context("fixture receiver PN")?;
        let receiver_address = receiver_jid.to_protocol_address();
        let bundle = PreKeyBundle::new(
            receiver.registration_id,
            u32::from(receiver_jid.device).into(),
            None,
            receiver.signed_pre_key_id.into(),
            receiver.signed_pre_key.public_key,
            receiver.signed_pre_key_signature,
            IdentityKey::new(receiver.identity_key.public_key),
        )?;
        ensure!(
            receiver
                .backend
                .get_session(sender.to_protocol_address().as_str())
                .await?
                .is_none(),
            "first DM receiver already has a peer session"
        );

        let peer = super::build_offline_client().await;
        peer.persistence_manager
            .modify_device(|device| device.registration_id = 0x2345_6789)
            .await;
        let mut stores = peer.signal_adapter();
        let mut rng = rand::rngs::StdRng::seed_from_u64(WORKLOAD_SEED);
        process_prekey_bundle(
            &receiver_address,
            &mut stores.session_store,
            &mut stores.identity_store,
            &bundle,
            &mut rng,
            UsePQRatchet::No,
        )
        .await?;
        let mut plaintext = waproto::codec::message_to_vec(&wa::Message::text(FIRST_DM_TEXT));
        plaintext.push(1);
        let ciphertext = message_encrypt(
            &plaintext,
            &receiver_address,
            &mut stores.session_store,
            &mut stores.identity_store,
        )
        .await?;
        ensure!(
            matches!(ciphertext, CiphertextMessage::PreKeySignalMessage(_)),
            "initial encryption did not produce a prekey message"
        );
        let stanza = NodeBuilder::new("message")
            .attr("from", sender)
            .attr("id", FIRST_DM_ID)
            .attr("t", wacore::time::now_secs().to_string())
            .attr("type", "text")
            .children([NodeBuilder::new("enc")
                .attr("type", "pkmsg")
                .attr("v", "2")
                .bytes(ciphertext.serialize().to_vec())
                .build()])
            .build();
        let (tx, events) = async_channel::bounded(1);
        let subscription = self.client.subscribe_handler(Arc::new(FirstDmEvents(tx)));
        Ok(FirstDm {
            stanza: Some(stanza),
            events,
            _subscription: subscription,
        })
    }

    /// Inject the encrypted stanza and observe the actual plaintext event and
    /// delivery receipt through the production read loop.
    pub async fn receive_first_dm(&self, input: &mut FirstDm) -> Result<()> {
        let stanza = input
            .stanza
            .take()
            .context("first DM input already consumed")?;
        self.wire.reply(stanza).await?;
        let event = tokio::time::timeout(DEADLINE, input.events.recv())
            .await
            .context("first DM did not reach the receiver")??;
        let Event::Messages(batch) = event.as_ref() else {
            anyhow::bail!("expected a message event");
        };
        let [message] = &*batch.messages else {
            anyhow::bail!("expected one first DM");
        };
        ensure!(
            message.info.id == FIRST_DM_ID
                && message.info.source.sender == Jid::new(super::PEER_USER, Server::Pn)
                && message.message.conversation.as_deref() == Some(FIRST_DM_TEXT),
            "unexpected first DM payload or sender"
        );
        self.wait_until(|| self.delivery_receipts() == 1)
            .await
            .context("first DM delivery receipt did not reach the wire")?;
        ensure!(
            self.counts.committed.load(Ordering::Relaxed) == 1 && input.events.is_empty(),
            "first DM must commit and dispatch exactly once"
        );
        Ok(())
    }

    pub async fn checkpoint(&self) -> Checkpoint {
        let (lanes, open_lanes, running_workers) = self
            .client
            .chat_lanes
            .fold_entries((0, 0, 0), |(lanes, open, running), _, lane| {
                (
                    lanes + 1,
                    open + usize::from(!lane.queue_tx.is_closed()),
                    running + usize::from(lane.worker_running.try_lock().is_none()),
                )
            })
            .await;
        Checkpoint {
            connected: self.client.is_socket_connected(),
            lanes,
            open_lanes,
            running_workers,
            messages: self.counts.messages.load(Ordering::Relaxed),
            committed: self.counts.committed.load(Ordering::Relaxed),
            commit_batches: self.counts.commit_batches.load(Ordering::Relaxed),
            history: self.counts.history.load(Ordering::Relaxed),
        }
    }

    /// Do not close/clear lanes here: this must observe production timeout reclamation.
    /// Uses Tokio time, so the same lifecycle can be tested without real minute-long waits.
    pub async fn idle(&self) -> Result<()> {
        tokio::time::sleep(IDLE).await;
        tokio::time::timeout(DEADLINE, async {
            loop {
                let state = self.checkpoint().await;
                ensure!(state.connected, "session disconnected during idle");
                if state.open_lanes == 0 && state.running_workers == 0 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
            Ok::<_, anyhow::Error>(())
        })
        .await??;
        self.settle().await
    }

    /// Retire warm-up Signal flush work before entering a maintenance sample.
    ///
    /// Keep this outside the timed region: the coalesced worker sleeps before
    /// flushing and can otherwise first be polled by the measured `block_on`.
    pub async fn prepare_maintenance_sample(&self) -> Result<()> {
        self.settle().await?;
        self.wait_until(|| !self.client.signal_flush_worker_armed())
            .await
            .context("warm-up Signal flush worker did not retire")?;
        // The warm-up leaves many small chunks in glibc's fastbins and unsorted
        // bin, and glibc only consolidates and sorts them on a request that no
        // tcache, fastbin or small-bin chunk serves. Without this, that request
        // is a small allocation inside the measured maintenance (the
        // `event_listener` in `RwLock::write_unlock`), which then pays a
        // variable +6..15 µs under the MALLOC_* settings pinned in the
        // CodSpeed workflow. A 100 KiB request is above the small-bin and
        // tcache limits and below the 128 KiB mmap threshold, so glibc does
        // that work here, outside the measured region.
        drop(std::hint::black_box(Vec::<u8>::with_capacity(100 * 1024)));
        Ok(())
    }

    /// Measure only after `receive_activity` or `finish_control` has joined
    /// startup. A new initialization invalidates the sample instead of becoming
    /// hidden scheduler work in the maintenance cost.
    pub async fn maintenance(&self) -> Result<()> {
        let started = self.client.bench_startup.started();
        let initialization_iqs = self.initialization_iqs();
        ensure!(
            self.client.bench_startup.pending() == 0,
            "startup is still in flight before maintenance"
        );
        self.client.run_cache_maintenance().await;
        self.settle().await?;
        ensure!(
            self.client.bench_startup.pending() == 0
                && self.client.bench_startup.started() == started
                && self.initialization_iqs() == initialization_iqs,
            "initialization entered measured maintenance"
        );
        Ok(())
    }

    /// Untimed setup for [`Self::maintenance_passes`].
    ///
    /// One checked [`Self::maintenance`] pass runs first: the first write to
    /// each cache lock allocates its `event_listener` state, a once-per-process
    /// cost that would otherwise be the allocator-dependent part of the sample.
    pub async fn prepare_maintenance_passes(&self) -> Result<MaintenanceSample> {
        self.maintenance().await?;
        self.prepare_maintenance_sample().await?;
        Ok(MaintenanceSample {
            started: self.client.bench_startup.started(),
            initialization_iqs: self.initialization_iqs(),
        })
    }

    /// `passes` back-to-back production cache sweeps, without settling.
    ///
    /// One sweep is a few thousand instructions, and a single cold pass is
    /// mostly simulated cache misses whose cost follows the runner CPU. Pair
    /// with [`Self::finish_maintenance_passes`] outside the timed region.
    pub async fn maintenance_passes(&self, passes: usize) -> Result<()> {
        ensure!(
            self.client.bench_startup.pending() == 0,
            "startup is still in flight before maintenance"
        );
        for _ in 0..passes {
            self.client.run_cache_maintenance().await;
        }
        Ok(())
    }

    /// Settle after [`Self::maintenance_passes`] and reject initialization
    /// that the passes started.
    pub async fn finish_maintenance_passes(&self, sample: MaintenanceSample) -> Result<()> {
        self.settle().await?;
        ensure!(
            self.client.bench_startup.pending() == 0
                && self.client.bench_startup.started() == sample.started
                && self.initialization_iqs() == sample.initialization_iqs,
            "initialization entered measured maintenance"
        );
        Ok(())
    }

    /// Unsupported, non-active/non-keepalive IQs observed by the synthetic wire.
    /// Lifecycle tests use this to reject initialization leaking into maintenance.
    pub fn initialization_iqs(&self) -> usize {
        self.wire.initialization_iqs.load(Ordering::Relaxed)
    }

    /// Distinct workload IDs whose delivery receipts reached the transport.
    /// Publication alone does not prove their durable pending rows were removed.
    pub fn delivery_receipts(&self) -> usize {
        self.wire
            .delivery_receipts
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .iter()
            .filter(|seen| **seen)
            .count()
    }

    /// Requested capacity of the drain-only encoding buffer; not total heap or RSS.
    /// `None` means a commit still owns the buffer.
    pub fn drain_encode_capacity(&self) -> Option<usize> {
        self.client.inbound_commit_batch.encode_arena_capacity()
    }

    /// Finish a normal keepalive exchange with an identical final wire frame.
    /// The reader retains its last input buffer while waiting for the next frame;
    /// random request-ID widths otherwise change requested heap by a few bytes.
    /// This does not clear library state or replace the periodic keepalive check.
    pub async fn retention_barrier(&self) -> Result<usize> {
        // feed_received_data starts a fresh receive burst after a ten-second
        // gap. A fixed ID alone is insufficient: appending it to an earlier
        // burst can grow the decoder's accumulator by one CHUNK_SIZE.
        let quiet = Duration::from_secs(11);
        tokio::time::timeout(DEADLINE, async {
            loop {
                let last = self
                    .client
                    .stats
                    .last_data_received()
                    .context("retention checkpoint has no received frame")?;
                let elapsed = self
                    .client
                    .runtime
                    .monotonic_time()
                    .saturating_duration_since(last);
                if elapsed >= quiet {
                    return Ok::<(), anyhow::Error>(());
                }
                self.client.runtime.sleep(quiet - elapsed).await;
            }
        })
        .await
        .context("retention checkpoint did not reach a receive idle boundary")??;
        let pongs = self.pongs();
        self.client
            .send_iq_node(
                NodeBuilder::new("iq")
                    .attr("id", "RETENTION-CHECKPOINT")
                    .attr("type", "get")
                    .attr("to", "s.whatsapp.net")
                    .attr("xmlns", "w:p")
                    .build(),
                Some(DEADLINE),
            )
            .await?;
        self.settle().await?;
        ensure!(
            self.pongs() == pongs + 1,
            "another keepalive overlapped the retention checkpoint"
        );
        Ok(pongs + 1)
    }

    pub fn pongs(&self) -> usize {
        self.wire.pongs.load(Ordering::Relaxed)
    }

    /// Explicit keepalive-path probe for virtual-time tests. The native idle
    /// measurement does not call this; it requires a periodic production ping.
    pub async fn probe(&self) -> Result<()> {
        self.client
            .execute(wacore::iq::keepalive::KeepaliveSpec::new())
            .await?;
        Ok(())
    }

    pub async fn shutdown(mut self) -> Result<()> {
        let client = Arc::downgrade(&self.client);
        let _ = self.client.shutdown().await;
        let reader_result: Result<()> = match self.reader.take() {
            Some(mut reader) => match tokio::time::timeout(DEADLINE, &mut reader).await {
                Ok(joined) => joined
                    .map_err(anyhow::Error::from)
                    .and_then(|result| result),
                Err(elapsed) => {
                    reader.abort();
                    Err(anyhow::Error::from(elapsed).context("fixture reader did not stop"))
                }
            },
            None => Ok(()),
        };
        // Production disconnect closes lane channels, but their workers are
        // detached. Observe all client owners exiting before callers unlink an
        // SQLite database; do not keep the runtime parked behind a live worker.
        drop(self);
        let owners = util::wait_until(DEADLINE, || client.strong_count() == 0)
            .await
            .context("fixture client owners did not finish after disconnect");
        reader_result.and(owners)
    }
}

impl Drop for Session {
    fn drop(&mut self) {
        self.client.signal_shutdown_sync();
        self.wire.server_tx.close();
        if let Some(reader) = self.reader.take() {
            reader.abort();
        }
    }
}

struct NoHttp;
#[async_trait]
impl HttpClient for NoHttp {
    async fn execute(&self, _: HttpRequest) -> Result<HttpResponse> {
        anyhow::bail!("connected idle fixture forbids HTTP")
    }
}

struct Factory(Arc<Wire>);
#[async_trait]
impl TransportFactory for Factory {
    async fn create_transport(
        &self,
    ) -> Result<(Arc<dyn Transport>, async_channel::Receiver<TransportEvent>)> {
        Ok((self.0.clone(), self.0.server_rx.clone()))
    }
}

enum State {
    Hello,
    Finish {
        noise: Box<NoiseHandshake>,
        ephemeral: KeyPair,
    },
    Established {
        read: NoiseCipher,
        write: NoiseCipher,
        received: u32,
        sent: u32,
    },
    Closed,
}

enum OutgoingCapture {
    Messages(async_channel::Sender<Node>),
    OfflineReplay {
        sender: async_channel::Sender<offline_replay::CapturedNode>,
        measure_wall: bool,
    },
}

impl OutgoingCapture {
    fn accepts(&self, node: &NodeRef<'_>) -> bool {
        match self {
            Self::Messages(_) => node.tag == "message",
            Self::OfflineReplay { .. } => {
                node.tag == "receipt"
                    || node.tag == "ib" && node.get_optional_child("offline_batch").is_some()
            }
        }
    }

    fn record(&self, node: Node) -> Result<()> {
        match self {
            Self::Messages(sender) => sender
                .try_send(node)
                .context("fixture outgoing message queue"),
            Self::OfflineReplay {
                sender,
                measure_wall,
            } => sender
                .try_send(offline_replay::CapturedNode {
                    node,
                    timestamp: measure_wall.then(wacore::time::Instant::now),
                })
                .context("fixture outgoing replay queue"),
        }
    }
}

struct Wire {
    state: Mutex<State>,
    server_tx: async_channel::Sender<TransportEvent>,
    server_rx: async_channel::Receiver<TransportEvent>,
    active: AtomicUsize,
    pongs: AtomicUsize,
    initialization_iqs: AtomicUsize,
    delivery_receipts: Mutex<[bool; MESSAGES]>,
    max_batch_frames: AtomicUsize,
    outgoing_capture: Option<Arc<OutgoingCapture>>,
}

impl Wire {
    fn observe_delivery_receipt(&self, node: &NodeRef<'_>) {
        // Delivery may be individual or aggregated. Ignore history, ACKs and
        // retry/read receipts; duplicate IDs cannot satisfy a missing delivery.
        if node.tag != "receipt"
            || node
                .attrs()
                .optional_string("type")
                .is_some_and(|kind| kind != "inactive")
        {
            return;
        }
        let mut seen = self
            .delivery_receipts
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let mut record = |item: &NodeRef<'_>| {
            if let Some(index) = item.attrs().optional_string("id").and_then(|id| {
                id.strip_prefix("IDLE-")
                    .and_then(|index| index.parse::<usize>().ok())
            }) && let Some(slot) = seen.get_mut(index)
            {
                *slot = true;
            }
        };
        record(node);
        if let Some(items) = node.get_optional_child("list").and_then(NodeRef::children) {
            for item in items.iter().filter(|item| item.tag == "item") {
                record(item);
            }
        }
    }

    fn frame(&self, body: &[u8]) -> Result<(Option<Bytes>, Option<OwnedNodeRef>, bool)> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        match std::mem::replace(&mut *state, State::Closed) {
            State::Hello => {
                let hello = waproto::codec::handshake_message_decode(body)?;
                let peer: [u8; 32] = hello
                    .client_hello
                    .into_option()
                    .context("client hello")?
                    .ephemeral
                    .context("client ephemeral")?
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("ephemeral length"))?;
                let identity = KeyPair::generate(&mut rand::rng());
                let ephemeral = KeyPair::generate(&mut rand::rng());
                // The same harness is compiled against the PR base and head.
                let identity_pub_bytes: &[u8] = identity.public_key.public_key_bytes();
                let identity_pub: [u8; 32] = identity_pub_bytes
                    .try_into()
                    .context("identity public key length")?;
                let ephemeral_pub = ephemeral.public_key.public_key_bytes();
                let mut noise = NoiseHandshake::new(NOISE_PATTERN_XX, &WA_CONN_HEADER)?;
                noise.authenticate(&peer);
                noise.authenticate(ephemeral_pub);
                noise.mix_shared_secret(ephemeral.private_key.serialize(), &peer)?;
                let encrypted_static = noise.encrypt(&identity_pub)?;
                noise.mix_shared_secret(identity.private_key.serialize(), &peer)?;
                let payload = noise.encrypt(&wacore_noise::test_util::build_cert_chain_bytes(
                    &identity_pub,
                ))?;
                let response = waproto::codec::handshake_message_to_vec(&{
                    let mut proto_value = wa::HandshakeMessage::default();
                    proto_value.server_hello = buffa::MessageField::some({
                        let mut proto_addition = wa::handshake_message::ServerHello::default();
                        proto_addition.ephemeral = Some(ephemeral_pub.to_vec());
                        proto_addition.r#static = Some(encrypted_static);
                        proto_addition.payload = Some(payload);
                        proto_addition
                    });
                    proto_value
                });
                *state = State::Finish {
                    noise: Box::new(noise),
                    ephemeral,
                };
                Ok((
                    Some(wacore::framing::encode_frame(&response, None)?.into()),
                    None,
                    false,
                ))
            }
            State::Finish {
                mut noise,
                ephemeral,
            } => {
                let finish = waproto::codec::handshake_message_decode(body)?
                    .client_finish
                    .into_option()
                    .context("client finish")?;
                let peer: [u8; 32] = noise
                    .decrypt(&finish.r#static.context("client static")?)?
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("client static length"))?;
                noise.mix_shared_secret(ephemeral.private_key.serialize(), &peer)?;
                let _payload = noise.decrypt(&finish.payload.context("client payload")?)?;
                let (read, write) = noise.finish()?;
                *state = State::Established {
                    read,
                    write,
                    received: 0,
                    sent: 0,
                };
                Ok((None, None, true))
            }
            State::Established {
                read,
                write,
                received,
                sent,
            } => {
                let mut plain = BytesMut::from(body);
                read.decrypt_in_place_with_counter(received, &mut plain)?;
                let unpacked = wacore_binary::util::unpack(&plain)?;
                let node = OwnedNodeRef::new(unpacked.into_owned())?;
                *state = State::Established {
                    read,
                    write,
                    received: received
                        .checked_add(1)
                        .context("receive counter exhausted")?,
                    sent,
                };
                Ok((None, Some(node), false))
            }
            State::Closed => anyhow::bail!("fixture connection closed"),
        }
    }

    async fn reply(&self, node: Node) -> Result<()> {
        let frame = {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            let State::Established { write, sent, .. } = &mut *state else {
                anyhow::bail!("fixture handshake not complete");
            };
            let encrypted = write.encrypt_with_counter(*sent, &marshal(&node)?)?;
            *sent = sent.checked_add(1).context("send counter exhausted")?;
            wacore::framing::encode_frame(&encrypted, None)?
        };
        self.server_tx
            .send(TransportEvent::DataReceived(frame.into()))
            .await?;
        Ok(())
    }
}

#[async_trait]
impl Transport for Wire {
    async fn send(&self, data: Bytes) -> Result<()> {
        let hello = matches!(
            *self.state.lock().unwrap_or_else(|e| e.into_inner()),
            State::Hello
        );
        let mut remaining = if hello {
            data.strip_prefix(&WA_CONN_HEADER)
                .context("missing connection header")?
        } else {
            &data
        };
        let mut batch_frames = 0;
        while !remaining.is_empty() {
            batch_frames += 1;
            ensure!(remaining.len() >= 3, "short fixture frame");
            let length = (usize::from(remaining[0]) << 16)
                | (usize::from(remaining[1]) << 8)
                | usize::from(remaining[2]);
            ensure!(remaining.len() >= 3 + length, "truncated fixture frame");
            let (response, node, logged_in) = self.frame(&remaining[3..3 + length])?;
            if let Some(node) = &node {
                self.observe_delivery_receipt(node.get());
            }
            remaining = &remaining[3 + length..];
            if let Some(response) = response {
                self.server_tx
                    .send(TransportEvent::DataReceived(response))
                    .await?;
            }
            if logged_in {
                self.reply(
                    NodeBuilder::new("success")
                        .attr("lid", "100000000000001:1@lid")
                        .build(),
                )
                .await?;
            }
            let Some(node) = node else { continue };
            let node = node.get();
            if let Some(capture) = &self.outgoing_capture
                && capture.accepts(node)
            {
                capture.record(node.to_owned())?;
            } else if node.tag == "iq" {
                let active = node.get_optional_child("active").is_some();
                let ping = node
                    .attrs()
                    .optional_string("xmlns")
                    .is_some_and(|ns| ns == "w:p");
                let mut reply = NodeBuilder::new("iq")
                    .attr(
                        "id",
                        node.attrs()
                            .optional_string("id")
                            .context("IQ id")?
                            .as_ref(),
                    )
                    .attr("from", Jid::new("", Server::Pn))
                    .attr("type", if active || ping { "result" } else { "error" });
                if !active && !ping {
                    self.initialization_iqs.fetch_add(1, Ordering::Relaxed);
                    reply = reply.children([NodeBuilder::new("error")
                        .attr("code", "503")
                        .attr("text", "unsupported fixture IQ")
                        .build()]);
                }
                self.reply(reply.build()).await?;
                if active {
                    self.active.fetch_add(1, Ordering::Relaxed);
                }
                if ping {
                    self.pongs.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
        self.max_batch_frames
            .fetch_max(batch_frames, Ordering::Relaxed);
        Ok(())
    }

    async fn disconnect(&self) {
        self.server_tx.close();
    }
}

pub mod offline_replay;
pub mod ping_pong;

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "sqlite-storage")]
    #[tokio::test(start_paused = true)]
    async fn sqlite_first_dm_flush_owns_the_record_after_the_coalescing_window() -> Result<()> {
        sqlite_first_dm_with_pause(async {
            tokio::time::sleep(Duration::from_millis(100)).await;
        })
        .await?;
        Ok(())
    }

    #[tokio::test]
    async fn prepared_activity_joins_passive_startup_but_preserves_offline_drain() -> Result<()> {
        #[cfg(feature = "sqlite-storage")]
        let backends = [BackendFixture::memory(), BackendFixture::sqlite().await?];
        #[cfg(not(feature = "sqlite-storage"))]
        let backends = [BackendFixture::memory()];
        for backend in backends {
            let session = Session::connect(backend.backend()).await?;
            let activity = session.prepare_activity().await?;
            ensure!(session.client.bench_startup.pending_before_offline() == 0);
            ensure!(session.client.bench_startup.pending() > 0);
            ensure!(
                !session
                    .client
                    .offline_sync_completed
                    .load(Ordering::Acquire)
            );
            session.receive_activity(activity).await?;
            ensure!(session.client.bench_startup.pending() == 0);
            ensure!(session.delivery_receipts() == MESSAGES);
            session.prepare_maintenance_sample().await?;
            ensure!(!session.client.signal_flush_worker_armed());
            session.maintenance().await?;
            ensure!(!session.client.signal_flush_worker_armed());
            let sample = session.prepare_maintenance_passes().await?;
            session.maintenance_passes(4).await?;
            session.finish_maintenance_passes(sample).await?;
            session.shutdown().await?;
            backend.cleanup()?;
        }
        Ok(())
    }

    fn wire() -> Wire {
        let (server_tx, server_rx) = async_channel::bounded(1);
        Wire {
            state: Mutex::new(State::Hello),
            server_tx,
            server_rx,
            active: AtomicUsize::new(0),
            pongs: AtomicUsize::new(0),
            initialization_iqs: AtomicUsize::new(0),
            delivery_receipts: Mutex::new([false; MESSAGES]),
            max_batch_frames: AtomicUsize::new(0),
            outgoing_capture: None,
        }
    }

    fn count(wire: &Wire) -> usize {
        wire.delivery_receipts
            .lock()
            .unwrap()
            .iter()
            .filter(|seen| **seen)
            .count()
    }

    #[test]
    fn duplicate_receipts_cannot_replace_a_missing_delivery() {
        let wire = wire();
        for index in 0..MESSAGES - 1 {
            let receipt = NodeBuilder::new("receipt")
                .attr("id", format!("IDLE-{index}"))
                .build();
            wire.observe_delivery_receipt(&receipt.as_node_ref());
            wire.observe_delivery_receipt(&receipt.as_node_ref());
        }
        assert_eq!(count(&wire), MESSAGES - 1);
        wire.observe_delivery_receipt(
            &NodeBuilder::new("receipt")
                .attr("id", format!("IDLE-{}", MESSAGES - 1))
                .build()
                .as_node_ref(),
        );
        assert_eq!(count(&wire), MESSAGES);
    }

    #[test]
    fn aggregate_receipt_counts_root_and_list_ids_once() {
        let wire = wire();
        wire.observe_delivery_receipt(
            &NodeBuilder::new("receipt")
                .attr("id", "IDLE-0")
                .attr("type", "inactive")
                .children([NodeBuilder::new("list")
                    .children([
                        NodeBuilder::new("item").attr("id", "IDLE-1").build(),
                        NodeBuilder::new("item").attr("id", "IDLE-1").build(),
                        NodeBuilder::new("item").attr("id", "IDLE-2").build(),
                    ])
                    .build()])
                .build()
                .as_node_ref(),
        );
        assert_eq!(count(&wire), 3);
    }

    #[test]
    fn read_retry_and_ack_do_not_count_as_delivery() {
        let wire = wire();
        for kind in ["read", "retry"] {
            wire.observe_delivery_receipt(
                &NodeBuilder::new("receipt")
                    .attr("id", "IDLE-0")
                    .attr("type", kind)
                    .children([NodeBuilder::new("list")
                        .children([NodeBuilder::new("item").attr("id", "IDLE-1").build()])
                        .build()])
                    .build()
                    .as_node_ref(),
            );
        }
        wire.observe_delivery_receipt(
            &NodeBuilder::new("ack")
                .attr("id", "IDLE-0")
                .build()
                .as_node_ref(),
        );
        assert_eq!(count(&wire), 0);
    }

    #[test]
    fn unrelated_invalid_and_out_of_range_ids_do_not_count() {
        let wire = wire();
        let out_of_range = format!("IDLE-{MESSAGES}");
        for id in ["HISTORY-0", "IDLE-invalid", out_of_range.as_str(), "IDLE-"] {
            wire.observe_delivery_receipt(
                &NodeBuilder::new("receipt")
                    .attr("id", id)
                    .build()
                    .as_node_ref(),
            );
        }
        wire.observe_delivery_receipt(&NodeBuilder::new("receipt").build().as_node_ref());
        assert_eq!(count(&wire), 0);
    }
}

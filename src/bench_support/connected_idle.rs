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
use wacore_binary::{Jid, Node, OwnedNodeRef, Server, builder::NodeBuilder, marshal};

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
        let backend =
            crate::store::SqliteStore::new(path.to_str().context("SQLite path encoding")?).await?;
        Ok(Self {
            backend: Some(Arc::new(backend)),
            path: Some(path),
        })
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
        Ok(())
    }
}

/// Prepared ciphertext/history input, consumed by activity so it cannot contaminate idle samples.
pub struct Activity {
    stanzas: Vec<Node>,
    history: wa::message::HistorySyncNotification,
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
        let pm = Arc::new(PersistenceManager::new(backend).await?);
        for command in [
            DeviceCommand::SetId(Some(Jid::new(super::OWN_USER, Server::Pn).with_device(1))),
            DeviceCommand::SetLid(Some(
                Jid::new("100000000000001", Server::Lid).with_device(1),
            )),
            DeviceCommand::SetPushName("Synthetic idle fixture".into()),
            DeviceCommand::SetAccount(Some(wa::ADVSignedDeviceIdentity {
                details: Some(vec![0; 32]),
                account_signature_key: Some(vec![0; 32]),
                account_signature: Some(vec![0; 64]),
                device_signature: Some(vec![0; 64]),
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
        ensure!(session.client.is_connected(), "login lost its connection");
        Ok(session)
    }

    /// Established Signal material and synthetic encryption are fixture setup, not
    /// part of the CodSpeed timed region. The peer is dropped before this returns.
    pub async fn prepare_activity(&self) -> Result<Activity> {
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
                .await;
            keys.push(key);
        }
        let text = "x".repeat(TEXT_BYTES);
        let plaintext = wacore::messages::MessageUtils::encode_and_pad(&wa::Message {
            conversation: Some(text.clone()),
            ..Default::default()
        });
        let mut stanzas = Vec::with_capacity(MESSAGES);
        for i in 0..MESSAGES {
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
        let history = wa::HistorySync {
            sync_type: wa::history_sync::HistorySyncType::INITIAL_BOOTSTRAP,
            conversations: groups
                .iter()
                .enumerate()
                .map(|(lane, group)| wa::Conversation {
                    id: group.to_string(),
                    messages: (lane..HISTORY_MESSAGES)
                        .step_by(LANES)
                        .map(|i| wa::HistorySyncMsg {
                            message: buffa::MessageField::some(wa::WebMessageInfo {
                                key: buffa::MessageField::some(wa::MessageKey {
                                    remote_jid: Some(group.to_string()),
                                    from_me: Some(false),
                                    id: Some(format!("HISTORY-{i}")),
                                    participant: Some(peer_jid.to_string()),
                                }),
                                message: buffa::MessageField::some(wa::Message {
                                    conversation: Some(text.clone()),
                                    ..Default::default()
                                }),
                                message_secret: Some(vec![0x44; 32]),
                                message_timestamp: Some(wacore::time::now_secs() as u64),
                                ..Default::default()
                            }),
                            msg_order_id: Some(i as u64),
                        })
                        .collect(),
                    ..Default::default()
                })
                .collect(),
            ..Default::default()
        };
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&waproto::codec::history_sync_to_vec(&history))?;
        let compressed = encoder.finish()?;
        Ok(Activity {
            stanzas,
            history: wa::message::HistorySyncNotification {
                file_length: Some(compressed.len() as u64),
                sync_type: Some(wa::message::HistorySyncType::INITIAL_BOOTSTRAP),
                initial_hist_bootstrap_inline_payload: Some(compressed),
                ..Default::default()
            },
        })
    }

    /// All encrypted messages enter the real reader, dispatcher and production lane
    /// workers. History enters the production inline-history parser/task (no HTTP).
    /// Finish the server's offline phase and wait for commit/event/receipt work.
    pub async fn receive_activity(&self, activity: Activity) -> Result<()> {
        for node in activity.stanzas {
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
        self.finish_offline(MESSAGES).await?;
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
        self.settle().await?;
        Ok(())
    }

    pub async fn finish_control(&self) -> Result<()> {
        self.finish_offline(0).await?;
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
        self.client.wait_for_connected(DEADLINE).await?;
        Ok(())
    }

    async fn settle(&self) -> Result<()> {
        self.wait_until(|| self.client.response_waiters_guard().is_empty())
            .await?;
        self.client
            .outbound_flush
            .flush(&*self.client.runtime, DEADLINE)
            .await;
        self.client.flush_signal_cache_batch_safe().await?;
        self.wait_until(|| self.wire.server_rx.is_empty()).await?;
        tokio::task::yield_now().await;
        Ok(())
    }

    async fn wait_until(&self, ready: impl Fn() -> bool) -> Result<()> {
        util::wait_until(DEADLINE, ready).await
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
            connected: self.client.is_connected(),
            lanes,
            open_lanes,
            running_workers,
            messages: self.counts.messages.load(Ordering::Relaxed),
            committed: self.counts.committed.load(Ordering::Relaxed),
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

    pub async fn maintenance(&self) -> Result<()> {
        self.client.run_cache_maintenance().await;
        self.settle().await
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
        self.client.disconnect().await;
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

struct Wire {
    state: Mutex<State>,
    server_tx: async_channel::Sender<TransportEvent>,
    server_rx: async_channel::Receiver<TransportEvent>,
    active: AtomicUsize,
    pongs: AtomicUsize,
}

impl Wire {
    fn frame(&self, body: &[u8]) -> Result<(Option<Bytes>, Option<Node>, bool)> {
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
                let identity_pub: [u8; 32] = identity.public_key.public_key_bytes().try_into()?;
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
                let response = waproto::codec::handshake_message_to_vec(&wa::HandshakeMessage {
                    server_hello: buffa::MessageField::some(wa::handshake_message::ServerHello {
                        ephemeral: Some(ephemeral_pub.to_vec()),
                        r#static: Some(encrypted_static),
                        payload: Some(payload),
                        ..Default::default()
                    }),
                    ..Default::default()
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
                let node = OwnedNodeRef::new(unpacked.into_owned())?.get().to_owned();
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
        while !remaining.is_empty() {
            ensure!(remaining.len() >= 3, "short fixture frame");
            let length = (usize::from(remaining[0]) << 16)
                | (usize::from(remaining[1]) << 8)
                | usize::from(remaining[2]);
            ensure!(remaining.len() >= 3 + length, "truncated fixture frame");
            let (response, node, logged_in) = self.frame(&remaining[3..3 + length])?;
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
            if let Some(node) = node
                && node.tag == "iq"
            {
                let nr = node.as_node_ref();
                let active = nr.get_optional_child("active").is_some();
                let ping = nr
                    .attrs()
                    .optional_string("xmlns")
                    .is_some_and(|ns| ns == "w:p");
                let mut reply = NodeBuilder::new("iq")
                    .attr(
                        "id",
                        nr.attrs().optional_string("id").context("IQ id")?.as_ref(),
                    )
                    .attr("from", Jid::new("", Server::Pn))
                    .attr("type", if active || ping { "result" } else { "error" });
                if !active && !ping {
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
        Ok(())
    }

    async fn disconnect(&self) {
        self.server_tx.close();
    }
}

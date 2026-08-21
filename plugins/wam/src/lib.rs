//! WhatsApp Metrics (WAM) telemetry, as a plugin.
//!
//! WAM is what the official client uploads about itself: a binary buffer of
//! numbered events, sent under `<iq xmlns="w:stats">`. This crate observes what
//! this client already reports on its event bus, turns the part that is honestly
//! derivable into WAM events, and uploads them on the `regular` channel.
//!
//! ```ignore
//! let client = Client::builder()
//!     // platform dependencies...
//!     .with_plugin(WamPlugin::new(WamConfig::default()))
//!     .build()
//!     .await?
//!     .into_client();
//! let wam = client.plugin::<WamPlugin>().expect("wam is installed");
//! println!("{:?}", wam.stats());
//! ```
//!
//! # Why a plugin and not a subsystem
//!
//! `agent_docs/subsystem_boundary.md` asks four questions of anything that wants
//! to be attached to the core, and the first decides this one: a subsystem is
//! entered on a dispatch key the core already routes on. WAM claims no stanza
//! tag, no notification type and no IQ namespace on the way in. It wants to
//! *watch* work the core does for its own reasons, which is the shape the same
//! document calls coupled, not cuttable. A hook for it would need two askers and
//! a measured floor; WAM is one asker. So it attaches where a watcher belongs,
//! through the plugin host's existing observation capability, and the core gains
//! no field, no gate and no line.
//!
//! # What this does not emit, and why
//!
//! The catalog carries 436 events. This plugin emits five, and the gap is not
//! ambition. It is the rule that every field of an emitted event must follow
//! from something this client actually saw.
//!
//! - **Everything derived from sending.** `MessageSend`, `E2eMessageSend`,
//!   `WebcMessageSend`, `EditMessageSend`, `RevokeMessageSend` and the rest of
//!   that family describe an outgoing message: its type, its media, how many
//!   devices it was encrypted for, how long each stage took. The core publishes
//!   `Event::SentFrame`, which is the marshaled bytes of a stanza after the
//!   write, and nothing that carries the send's own semantics. Re-deriving a
//!   94-field event from a frame would be reconstruction, not observation.
//! - **The `private` channel.** Its fifty events need a blind-signed token and a
//!   rotating anonymous id, neither of which exists here. The catalog carries
//!   the rotation groups so a later batch has them.
//! - **Beaconing.** The official client gives one client in a hundred a
//!   per-event sequence number, rolled once per UTC day and remembered. The roll
//!   is only meaningful if it happens once per client per day; a process that
//!   restarts five times a day and cannot remember would roll five times and
//!   over-represent itself in a cohort built on the opposite assumption.
//!   Getting it right needs a durable per-event counter, which needs a storage
//!   capability the host does not grant, so this batch does not beacon at all.
//! - **Events whose input is a counter, not an event.** `MessageHighRetryCount`
//!   and `MdRetryFromUnknownDevice` describe things this client already
//!   measures: `wacore::telemetry` has a counter for each, and the doc comments
//!   there name these very WAM ids. What it does not have is an `Event` carrying
//!   them, and a plugin sees only the event bus. They are one core event away,
//!   not one design away.

#![forbid(unsafe_code)]

pub mod derive;
pub mod identity;
pub mod iq;
pub mod runtime;
pub mod store;

#[cfg(test)]
mod parity;

use std::sync::{Arc, Mutex, OnceLock};

use anyhow::{Context, Result};
use log::warn;
use whatsapp_rust::wacore::stanza::wire_tags::StanzaTag;
use whatsapp_rust::wacore::types::events::{Event, EventHandler, EventInterest, EventKind};
use whatsapp_rust::{
    ClientPlugin, PluginCapability, PluginContext, PluginCoreEventSubscription, PluginFuture,
    PluginIq, PluginManifest, PluginTasks,
};
use whatsapp_rust_wam_catalog::events;

pub use identity::WamIdentity;
pub use runtime::{PendingEvent, UploadFailure, WamStats, WamUploader};
pub use store::{InMemoryWamStore, PendingBuffer, WamStore, WamStoreError};

use runtime::{BUFFERING_INTERVAL, WamRuntime, WamWriter};

/// The plugin's manifest id.
pub const WAM_PLUGIN_ID: &str = "wa.wam";

/// How this plugin is configured.
#[derive(Clone)]
pub struct WamConfig {
    /// What this client says about itself in a buffer's globals.
    pub identity: WamIdentity,
    /// Where sequence numbers and undelivered buffers live.
    pub store: Arc<dyn WamStore>,
    /// The ceiling on events waiting to be written.
    ///
    /// A bound on memory, not on throughput: an event is a few dozen bytes and
    /// the queue drains every few seconds, so the default is far above any rate
    /// a client sustains. What it guarantees is that a stalled flush task cannot
    /// grow the queue without limit.
    pub max_queued_events: usize,
}

impl std::fmt::Debug for WamConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WamConfig")
            .field("identity", &self.identity)
            .field("max_queued_events", &self.max_queued_events)
            .finish_non_exhaustive()
    }
}

impl Default for WamConfig {
    fn default() -> Self {
        Self {
            identity: WamIdentity::web(),
            store: Arc::new(InMemoryWamStore::new()),
            max_queued_events: 4096,
        }
    }
}

/// The typed API `client.plugin::<WamPlugin>()` returns.
pub struct WamApi {
    runtime: Arc<WamRuntime>,
    _core_events: PluginCoreEventSubscription,
}

impl WamApi {
    /// Counters for what this plugin has observed, written, sampled out,
    /// dropped and uploaded.
    ///
    /// A snapshot, approximate under concurrency, and carrying no JID, phone
    /// number or message id, under the same rules `agent_docs/observability.md` puts
    /// on every other snapshot in this repository.
    pub fn stats(&self) -> WamStats {
        self.runtime.stats()
    }
}

/// One WAM plugin installation. Construct a fresh value for each client.
pub struct WamPlugin {
    config: WamConfig,
    runtime: OnceLock<Arc<WamRuntime>>,
    /// Kept so the shutdown flush can send through the same capability the flush
    /// loop uses, rather than opening a second path to the server.
    uploader: OnceLock<Arc<IqUploader>>,
    /// Where the flush loop leaves its buffers when it stops.
    ///
    /// `None` until the loop parks them, and `None` again once the shutdown
    /// flush has taken them, so neither can run a second writer against the same
    /// store no matter how the two are ordered.
    parked_writer: Arc<Mutex<Option<WamWriter>>>,
}

impl WamPlugin {
    pub fn new(config: WamConfig) -> Self {
        Self {
            config,
            runtime: OnceLock::new(),
            uploader: OnceLock::new(),
            parked_writer: Arc::new(Mutex::new(None)),
        }
    }
}

impl Default for WamPlugin {
    fn default() -> Self {
        Self::new(WamConfig::default())
    }
}

/// Forwards the core events WAM is derived from.
///
/// Runs inline on the read loop, so it does nothing but classify and enqueue.
struct WamEventHandler(Arc<WamRuntime>);

impl EventHandler for WamEventHandler {
    fn handle_event(&self, event: Arc<Event>) {
        match &*event {
            Event::Messages(batch) => {
                for pending in derive::from_batch(batch) {
                    self.0.observe(pending);
                }
            }
            Event::DecryptedPayload(payload) => {
                self.0
                    .observe(PendingEvent::E2eMessageRecv(derive::e2e_message_recv(
                        &payload.info,
                        payload.enc_type,
                        true,
                        None,
                    )));
            }
            Event::EncDecryptFailed(failed) => {
                if let Some(event) = derive::from_enc_failure(failed) {
                    self.0.observe(PendingEvent::E2eMessageRecv(event));
                }
            }
            Event::Connected(_) => {
                // The socket this client just authenticated on. WA Web writes
                // this one at the same moment and fills none of its fields, so
                // a fieldless event here is the whole event, not a stub.
                self.0.observe(PendingEvent::WebcSocketConnect(
                    events::WebcSocketConnect::default(),
                ));
            }
            Event::RawNode(node) => {
                let node = node.get();
                if node.tag.as_ref() == StanzaTag::Receipt.as_str()
                    && let Some(event) = derive::receipt_stanza_receive(node)
                {
                    self.0.observe(PendingEvent::ReceiptStanzaReceive(event));
                }
            }
            _ => {}
        }
    }

    fn interest(&self) -> EventInterest {
        WAM_INTEREST
    }
}

/// The core events this plugin subscribes to.
///
/// Three of them, `DecryptedPayload`, `EncDecryptFailed` and `RawNode`, are
/// lease-gated: the client builds and dispatches them only while a consumer asks
/// for them, and subscribing here is what asks. That is the running cost of
/// turning WAM on.
///
/// `RawNode` is the widest of the three, since it forwards every decoded stanza
/// and this handler keeps only `<receipt>`. It is here anyway because
/// `ReceiptStanzaReceive` is a per-stanza metric, and the `Receipt` event it
/// would otherwise be derived from is not one per stanza. Paying a tag
/// comparison per inbound stanza is the price of the number being the number it
/// claims to be.
const WAM_INTEREST: EventInterest = EventInterest::none()
    .with(EventKind::Messages)
    .with(EventKind::DecryptedPayload)
    .with(EventKind::EncDecryptFailed)
    .with(EventKind::RawNode)
    .with(EventKind::Connected);

impl ClientPlugin for WamPlugin {
    type Api = WamApi;

    fn manifest(&self) -> PluginManifest {
        PluginManifest::new(WAM_PLUGIN_ID, env!("CARGO_PKG_VERSION"))
            .with_capability(PluginCapability::CoreEvents)
            .with_capability(PluginCapability::Tasks)
            .with_capability(PluginCapability::Iq)
    }

    fn install(&self, context: PluginContext) -> PluginFuture<'_, Result<Arc<Self::Api>>> {
        Box::pin(async move {
            let core_events = context
                .core_events()
                .cloned()
                .context("core-events capability is missing")?;
            let tasks = context
                .tasks()
                .cloned()
                .context("tasks capability is missing")?;
            let iq = context.iq().cloned().context("iq capability is missing")?;

            let runtime = Arc::new(WamRuntime::new(
                self.config.identity.clone(),
                self.config.store.clone(),
                self.config.max_queued_events,
            ));
            self.runtime
                .set(runtime.clone())
                .map_err(|_| anyhow::anyhow!("the wam plugin was installed more than once"))?;

            let uploader = Arc::new(IqUploader(iq));
            self.uploader
                .set(uploader.clone())
                .map_err(|_| anyhow::anyhow!("the wam plugin was installed more than once"))?;

            let subscription =
                core_events.subscribe(WAM_INTEREST, Arc::new(WamEventHandler(runtime.clone())))?;
            spawn_flush_loop(tasks, uploader, runtime.clone(), self.parked_writer.clone())?;
            Ok(Arc::new(WamApi {
                runtime,
                _core_events: subscription,
            }))
        })
    }

    /// Flush what is still queued, best effort, inside the deadline the host
    /// already applies to this callback.
    ///
    /// Not a new deadline and not a guarantee: the host bounds every shutdown
    /// callback, and an upload that does not finish inside that bound leaves the
    /// buffer retained rather than delaying the teardown. With the in-memory
    /// store that means the last buffer is lost, which is what an in-memory
    /// store is; with a durable one the next run sends it.
    ///
    /// The host drains a plugin's tasks before it calls this, so the flush loop
    /// has already parked its buffers and nothing races for them.
    fn shutdown(&self) -> PluginFuture<'_, Result<()>> {
        Box::pin(async move {
            let (Some(runtime), Some(uploader)) = (self.runtime.get(), self.uploader.get()) else {
                return Ok(());
            };
            let parked = self
                .parked_writer
                .lock()
                .map_err(|_| anyhow::anyhow!("the wam writer lock was poisoned"))?
                .take();
            let Some(mut writer) = parked else {
                return Ok(());
            };
            // The official client's own answer to this instant. Observed before
            // the flush so it rides the buffer it describes.
            runtime.observe(PendingEvent::WebWamForceFlush(
                events::WebWamForceFlush::default(),
            ));
            let now = whatsapp_rust::wacore::time::now_utc().timestamp();
            let mut roll = rand::random::<f64>;
            runtime
                .tick(&mut writer, uploader.as_ref(), now, &mut roll, true)
                .await;
            Ok(())
        })
    }
}

/// The install-scoped task that drains the queue and uploads.
///
/// Install-scoped rather than connection-scoped on purpose: the queue survives a
/// reconnect, so the events observed just before a drop are still uploaded after
/// it. An upload attempted while disconnected fails and the buffer is retained,
/// which is the same path a server error takes.
fn spawn_flush_loop(
    tasks: PluginTasks,
    uploader: Arc<IqUploader>,
    runtime: Arc<WamRuntime>,
    parked_writer: Arc<Mutex<Option<WamWriter>>>,
) -> Result<()> {
    let worker = tasks.clone();
    tasks.spawn(async move {
        let mut writer = WamWriter::default();
        while worker.sleep(BUFFERING_INTERVAL).await.is_ok() {
            let now = whatsapp_rust::wacore::time::now_utc().timestamp();
            let mut roll = rand::random::<f64>;
            runtime
                .tick(&mut writer, uploader.as_ref(), now, &mut roll, false)
                .await;
        }
        // Parked for the shutdown flush, which is the only thing that runs after
        // this loop. A poisoned lock means a panic already lost the buffers, so
        // there is nothing left to hand over.
        if let Ok(mut slot) = parked_writer.lock() {
            *slot = Some(writer);
        }
    })?;
    Ok(())
}

/// Uploads through the plugin's IQ capability.
struct IqUploader(PluginIq);

#[whatsapp_rust::async_trait]
impl WamUploader for IqUploader {
    async fn upload(&self, t: i64, buffer: &[u8]) -> Result<(), UploadFailure> {
        match self.0.execute(iq::SendBufferSpec { t, buffer }).await {
            Ok(()) => Ok(()),
            Err(err) => {
                let message = err.to_string();
                // Anything that is not the server saying no is worth one more
                // try: a disconnected client reconnects, a timeout may not
                // repeat. A 4xx from the server will repeat.
                let permanent = matches!(
                    &err,
                    whatsapp_rust::PluginIqError::Iq(whatsapp_rust::IqError::ServerError {
                        code,
                        ..
                    }) if (400..500).contains(code)
                );
                warn!("wam: buffer upload failed: {message}");
                Err(if permanent {
                    UploadFailure::Permanent(message)
                } else {
                    UploadFailure::Retryable(message)
                })
            }
        }
    }
}

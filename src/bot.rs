use crate::cache_config::CacheConfig;
use crate::client::{Client, ClientBuilderError};
use crate::pair_code::PairCodeOptions;
#[cfg(feature = "plugins")]
use crate::plugins::{ClientPlugin, PluginHostConfig, PluginRegistration, UntypedClientPlugin};
use crate::store::commands::DeviceCommand;
use crate::store::error::StoreError;
use crate::store::persistence_manager::PersistenceManager;
use crate::store::traits::Backend;
use crate::types::durability_hook::InboundDurabilityHook;
use crate::types::enc_handler::EncHandler;
use crate::types::events::{Event, EventHandler, EventInterest, EventKind};
use crate::types::message::MessageInfo;
use futures::FutureExt;
use log::{info, warn};
use std::collections::HashMap;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::{Arc, Weak};
use thiserror::Error;
use wacore::proto_helpers::MessageBuilderExt;
use wacore::runtime::Runtime;
use wacore::store::DevicePropsOverride;
use waproto::whatsapp as wa;

/// Typestate marker: the builder field has been provided (or pre-filled by a
/// feature-gated default).
pub struct Provided;
/// Typestate marker: no storage [`Backend`] has been provided yet.
pub struct MissingBackend;
/// Typestate marker: no transport factory has been provided yet. Enabled
/// `tokio-transport` (default) pre-fills this slot.
pub struct MissingTransport;
/// Typestate marker: no HTTP client has been provided yet. Enabled
/// `ureq-client` (default) pre-fills this slot.
pub struct MissingHttpClient;
/// Typestate marker: no async runtime has been provided yet. Enabled
/// `tokio-runtime` (default) pre-fills this slot.
pub struct MissingRuntime;

#[cfg(feature = "tokio-transport")]
type DefaultTransportState = Provided;
#[cfg(not(feature = "tokio-transport"))]
type DefaultTransportState = MissingTransport;

#[cfg(feature = "ureq-client")]
type DefaultHttpState = Provided;
#[cfg(not(feature = "ureq-client"))]
type DefaultHttpState = MissingHttpClient;

#[cfg(feature = "tokio-runtime")]
type DefaultRuntimeState = Provided;
#[cfg(not(feature = "tokio-runtime"))]
type DefaultRuntimeState = MissingRuntime;

#[cfg(feature = "tokio-transport")]
fn default_transport_factory() -> Option<Arc<dyn crate::transport::TransportFactory>> {
    Some(Arc::new(
        crate::transport::TokioWebSocketTransportFactory::new(),
    ))
}
#[cfg(not(feature = "tokio-transport"))]
fn default_transport_factory() -> Option<Arc<dyn crate::transport::TransportFactory>> {
    None
}

#[cfg(feature = "ureq-client")]
fn default_http_client() -> Option<Arc<dyn crate::http::HttpClient>> {
    Some(Arc::new(crate::http::UreqHttpClient::new()))
}
#[cfg(not(feature = "ureq-client"))]
fn default_http_client() -> Option<Arc<dyn crate::http::HttpClient>> {
    None
}

#[cfg(feature = "tokio-runtime")]
fn default_runtime() -> Option<Arc<dyn Runtime>> {
    Some(Arc::new(crate::runtime_impl::TokioRuntime))
}
#[cfg(not(feature = "tokio-runtime"))]
fn default_runtime() -> Option<Arc<dyn Runtime>> {
    None
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum BotBuilderError {
    /// Initializing the device row in the storage backend failed.
    #[error("failed to initialize the device store: {0}")]
    Store(#[from] StoreError),
    #[error(transparent)]
    Client(#[from] ClientBuilderError),
}

/// `message` is `Arc` so cloning the context across spawned tasks only bumps a
/// refcount, matching the pattern used by serenity's `Context` and matrix-sdk's
/// `Room`/`Client`.
#[derive(Clone)]
pub struct MessageContext {
    pub message: Arc<wa::Message>,
    pub info: MessageInfo,
    pub client: Arc<Client>,
}

impl MessageContext {
    /// Builds a context from borrowed parts, deep-cloning `message`. Prefer
    /// [`MessageContext::from_arc`]/[`MessageContext::from_inbound`] when an
    /// `Arc<wa::Message>` is already at hand (the event bus always has one).
    pub fn from_parts(message: &wa::Message, info: &MessageInfo, client: Arc<Client>) -> Self {
        Self::from_arc(Arc::new(message.clone()), info, client)
    }

    pub fn from_arc(message: Arc<wa::Message>, info: &MessageInfo, client: Arc<Client>) -> Self {
        Self {
            message,
            info: info.clone(),
            client,
        }
    }

    pub fn from_inbound(
        inbound: &wacore::types::events::InboundMessage,
        client: Arc<Client>,
    ) -> Self {
        Self::from_arc(Arc::clone(&inbound.message), &inbound.info, client)
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.bot.send_message", level = "debug", skip_all, fields(chat = %self.info.source.chat.observe()), err(Debug)))]
    pub async fn send_message(
        &self,
        message: wa::Message,
    ) -> Result<crate::send::SendResult, crate::send::SendError> {
        self.client
            .send_message(&self.info.source.chat, message)
            .await
    }

    /// Reply with plain text in the same chat, without quoting.
    pub async fn reply(
        &self,
        text: impl Into<String>,
    ) -> Result<crate::send::SendResult, crate::send::SendError> {
        self.send_message(wa::Message::text(text)).await
    }

    /// Reply with plain text, quoting the received message.
    pub async fn reply_quoting(
        &self,
        text: impl Into<String>,
    ) -> Result<crate::send::SendResult, crate::send::SendError> {
        let context = self.build_quote_context();
        self.send_message(wa::Message::text_with_context(text, context))
            .await
    }

    pub fn build_quote_context(&self) -> wa::ContextInfo {
        // A bot reply is same-chat: quoted chat and send target are both
        // info.source.chat, so remote_jid is omitted (WA Web parity).
        let chat = &self.info.source.chat;
        wacore::proto_helpers::build_quote_context_with_info(
            &self.info.id,
            &self.info.source.sender,
            chat,
            chat,
            &self.message,
        )
    }

    /// Referential [`wa::MessageKey`] for [`wa::message::ReactionMessage::key`].
    /// Sender-side revokes have a different shape; use [`Client::revoke_message`].
    pub fn message_key(&self) -> wa::MessageKey {
        use wacore_binary::JidExt;
        let needs_participant =
            self.info.source.is_group || self.info.source.chat.is_status_broadcast();
        wa::MessageKey {
            remote_jid: Some(self.info.source.chat.to_string()),
            from_me: Some(self.info.source.is_from_me),
            id: Some(self.info.id.clone()),
            participant: needs_participant.then(|| self.info.source.sender.to_string()),
        }
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.bot.edit_message", level = "debug", skip_all, fields(chat = %self.info.source.chat.observe()), err(Debug)))]
    pub async fn edit_message(
        &self,
        original_message_id: impl Into<String>,
        new_message: wa::Message,
    ) -> Result<String, crate::send::SendError> {
        self.client
            .edit_message(&self.info.source.chat, original_message_id, new_message)
            .await
    }

    /// Delete a message for everyone in the chat.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.bot.revoke_message", level = "debug", skip_all, fields(chat = %self.info.source.chat.observe()), err(Debug)))]
    pub async fn revoke_message(
        &self,
        message_id: impl Into<String>,
        revoke_type: crate::send::RevokeType,
    ) -> Result<(), crate::send::SendError> {
        self.client
            .revoke_message(&self.info.source.chat, message_id, revoke_type)
            .await
    }

    /// React to the incoming message. An empty `emoji` removes a previous
    /// reaction. The target key (including the group/status participant) is
    /// taken from [`MessageContext::message_key`].
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.bot.react", level = "debug", skip_all, fields(chat = %self.info.source.chat.observe()), err(Debug)))]
    pub async fn react(
        &self,
        emoji: &str,
    ) -> Result<crate::send::SendResult, crate::send::SendError> {
        self.client
            .send_reaction(&self.info.source.chat, self.message_key(), emoji)
            .await
    }
}

type EventHandlerCallback =
    Arc<dyn Fn(Arc<Event>, Arc<Client>) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

/// The user callback bundled with the set of event kinds it wants. Carrying the
/// interest here lets the bus skip materializing (and boxing) events the
/// callback ignores.
struct RegisteredHandler {
    callback: EventHandlerCallback,
    interest: EventInterest,
}

/// Union of every registered callback's interest, so the bus only materializes
/// events at least one callback wants.
fn combined_interest(handlers: &[RegisteredHandler]) -> EventInterest {
    handlers
        .iter()
        .fold(EventInterest::none(), |acc, h| acc.union(h.interest))
}

/// How a bot's registered callbacks receive events off the core event bus.
#[derive(Clone, Copy, Debug, Default)]
#[non_exhaustive]
pub enum EventDelivery {
    /// Each event is delivered to each interested callback on its own spawned
    /// task (default). A slow callback stalls neither the bus nor its siblings,
    /// but ordering across events is not guaranteed and a persistently slow
    /// consumer can accumulate unbounded in-flight tasks.
    #[default]
    Concurrent,
    /// Events are delivered to the callbacks strictly in arrival order through a
    /// single bounded mailbox drained by one task — the ordered `messages.upsert`
    /// contract used by interoperable clients. Bounds
    /// memory: when the mailbox is full the event is dropped and counted in
    /// [`StatsSnapshot::events_dropped`](wacore::stats::StatsSnapshot::events_dropped)
    /// instead of blocking the receive pipeline or growing without limit.
    /// Register an inbound durability hook when no drop is acceptable
    /// (at-least-once via redelivery).
    Ordered {
        /// Mailbox capacity — events buffered before drops begin. Clamped to ≥1.
        capacity: usize,
    },
}

/// Bridges the registered closures onto the core event bus per the chosen
/// [`EventDelivery`] strategy.
enum Delivery {
    /// Fan each event out to every interested callback on its own spawned task.
    Concurrent { handlers: Arc<[RegisteredHandler]> },
    /// Hand each event to a single ordered drainer via a bounded mailbox.
    Ordered {
        tx: async_channel::Sender<Arc<Event>>,
    },
}

struct CallbackBusAdapter {
    // Weak: the bus lives inside `client.core`, so a strong ref here would pin
    // the client for its whole lifetime. Upgraded per dispatch.
    client: Weak<Client>,
    delivery: Delivery,
    interest: EventInterest,
}

impl CallbackBusAdapter {
    fn new(client: Arc<Client>, handlers: Vec<RegisteredHandler>, delivery: EventDelivery) -> Self {
        let interest = combined_interest(&handlers);
        let delivery = match delivery {
            EventDelivery::Concurrent => Delivery::Concurrent {
                handlers: handlers.into(),
            },
            EventDelivery::Ordered { capacity } => {
                let (tx, rx) = async_channel::bounded::<Arc<Event>>(capacity.max(1));
                let handlers: Arc<[RegisteredHandler]> = handlers.into();
                // Single drainer preserves arrival order; within an event the
                // callbacks run in registration order. Weak so a dropped client
                // exits the loop.
                let drain_client = Arc::downgrade(&client);
                let drain_handlers = Arc::clone(&handlers);
                client
                    .runtime
                    .spawn(Box::pin(async move {
                        while let Ok(event) = rx.recv().await {
                            let Some(client) = drain_client.upgrade() else {
                                break;
                            };
                            let kind = event.kind();
                            for handler in drain_handlers.iter() {
                                if handler.interest.wants(kind) {
                                    // Keep the lone drainer alive across a faulty
                                    // callback. catch_unwind guards only poll, so
                                    // build the future inside the awaited block
                                    // too — a panic while creating it is caught as
                                    // well, not just one while polling.
                                    let cb = handler.callback.clone();
                                    let ev = Arc::clone(&event);
                                    let cl = client.clone();
                                    let ran =
                                        std::panic::AssertUnwindSafe(
                                            async move { cb(ev, cl).await },
                                        )
                                        .catch_unwind()
                                        .await;
                                    if ran.is_err() {
                                        warn!(
                                            "ordered event delivery callback panicked; continuing"
                                        );
                                    }
                                }
                            }
                        }
                    }))
                    .detach();
                Delivery::Ordered { tx }
            }
        };
        Self {
            client: Arc::downgrade(&client),
            delivery,
            interest,
        }
    }
}

impl EventHandler for CallbackBusAdapter {
    fn handle_event(&self, event: Arc<Event>) {
        match &self.delivery {
            Delivery::Concurrent { handlers } => {
                let Some(client) = self.client.upgrade() else {
                    return;
                };
                let kind = event.kind();
                for handler in handlers.iter() {
                    if !handler.interest.wants(kind) {
                        continue;
                    }
                    let callback = handler.callback.clone();
                    let cb_client = client.clone();
                    let event = Arc::clone(&event);
                    client
                        .runtime
                        .spawn(Box::pin(async move {
                            callback(event, cb_client).await;
                        }))
                        .detach();
                }
            }
            // Non-blocking on purpose: dropping on a full mailbox keeps a slow
            // consumer from ever backpressuring the receive pipeline. Only a
            // full mailbox is a capacity drop; a closed channel means the drainer
            // is gone (teardown/panic) and must not be masked as one.
            Delivery::Ordered { tx } => match tx.try_send(event) {
                Ok(()) => {}
                Err(async_channel::TrySendError::Full(_)) => {
                    if let Some(client) = self.client.upgrade() {
                        client.stats.record_event_dropped();
                    }
                }
                Err(async_channel::TrySendError::Closed(_)) => {
                    log::debug!("ordered event delivery channel closed; dropping event");
                }
            },
        }
    }

    fn interest(&self) -> EventInterest {
        self.interest
    }
}

/// Handle to a bot started in the background via [`Bot::spawn`]. Awaiting it
/// resolves once the run loop exits (logout, [`BotHandle::shutdown`], or abort).
///
/// Dropping the handle aborts the bot task. Keep it alive for as long as the
/// bot should run, and prefer [`BotHandle::shutdown`] to stop it.
pub struct BotHandle {
    client: Arc<Client>,
    done_rx: futures::channel::oneshot::Receiver<()>,
    abort_handle: wacore::runtime::AbortHandle,
}

impl BotHandle {
    pub fn client(&self) -> Arc<Client> {
        self.client.clone()
    }

    /// Gracefully stop the bot: disconnects (flushing the device snapshot,
    /// buffered receipts and message secrets) and waits for the run loop to
    /// exit.
    pub async fn shutdown(mut self) {
        self.client.disconnect().await;
        let _ = (&mut self.done_rx).await;
    }

    /// Abort the bot task immediately. Skips the flush work
    /// [`BotHandle::shutdown`] performs, so recently captured state may be
    /// lost; escape hatch only.
    pub fn abort(&self) {
        self.abort_handle.abort();
    }
}

impl std::future::Future for BotHandle {
    type Output = ();

    fn poll(
        mut self: Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        // Canceled only happens when the run task was aborted; both outcomes
        // mean "the bot is no longer running", which is all awaiters care about.
        Pin::new(&mut self.done_rx).poll(cx).map(|_| ())
    }
}

/// `Bot::run` polls the client's main run loop on the caller's task, so the
/// instrumented runtime never sees that future — without this, the most
/// CPU-relevant work of a session would be missing from the hook on the
/// common `bot.run().await` launch path. `Bot::spawn` needs no equivalent:
/// it routes the same loop through `Runtime::spawn`.
async fn run_metered<F: std::future::Future<Output = ()>>(
    fut: F,
    instrument: Option<Arc<dyn wacore::stats::TaskInstrument>>,
) {
    match instrument {
        Some(i) => wacore::stats::MeteredFuture::new(Box::pin(fut), i).await,
        None => fut.await,
    }
}

pub struct Bot {
    client: Arc<Client>,
    sync_task_receiver: Option<async_channel::Receiver<crate::sync_task::MajorSyncTask>>,
    event_handlers: Vec<RegisteredHandler>,
    event_delivery: EventDelivery,
    raw_handlers: Vec<Arc<dyn EventHandler>>,
    pair_code_options: Option<PairCodeOptions>,
    /// Kept alongside the instrumented runtime: `Bot::run` polls the main run
    /// loop on the caller's task, so it must meter that future itself —
    /// `Runtime::spawn` never sees it (`Bot::spawn` does go through it).
    task_instrument: Option<Arc<dyn wacore::stats::TaskInstrument>>,
}

impl std::fmt::Debug for Bot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bot")
            .field("client", &"<Client>")
            .field("sync_task_receiver", &self.sync_task_receiver.is_some())
            .field("event_handlers", &self.event_handlers.len())
            .field("event_delivery", &self.event_delivery)
            .field("raw_handlers", &self.raw_handlers.len())
            .field("pair_code_options", &self.pair_code_options.is_some())
            .field("task_instrument", &self.task_instrument.is_some())
            .finish()
    }
}

impl Bot {
    pub fn builder()
    -> BotBuilder<MissingBackend, DefaultTransportState, DefaultHttpState, DefaultRuntimeState>
    {
        BotBuilder::new()
    }

    pub fn client(&self) -> Arc<Client> {
        self.client.clone()
    }

    /// Run the bot on the current task until it shuts down (logout, or
    /// [`Client::disconnect`] called on [`Bot::client`] from another task).
    ///
    /// To run in the background instead, use [`Bot::spawn`].
    ///
    /// Coroutines are LocalCopy across crates: a consumer crate that awaits
    /// this future re-codegens the whole state machine graph behind it in its
    /// own binary. The boxed barrier below type-erases the graph in a plain
    /// (linker-shared) function, so callers poll through a vtable and the
    /// graph is compiled once, here. One allocation per process.
    pub async fn run(self) {
        self.run_boxed().await
    }

    #[inline(never)]
    fn run_boxed(self) -> wacore::runtime::BoxFuture<'static, ()> {
        Box::pin(self.run_graph())
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.bot.run", level = "debug", skip_all)
    )]
    async fn run_graph(self) {
        let instrument = self.task_instrument.clone();
        let client = self.start_background();
        run_metered(client.run(), instrument).await;
    }

    /// Start the bot on its runtime and return a [`BotHandle`] to await,
    /// gracefully shut down, or abort it.
    pub fn spawn(self) -> BotHandle {
        let client = self.start_background();

        let run_client = client.clone();
        let (done_tx, done_rx) = futures::channel::oneshot::channel::<()>();
        let abort_handle = client.runtime.spawn(Box::pin(async move {
            run_client.run().await;
            let _ = done_tx.send(());
        }));

        BotHandle {
            client,
            done_rx,
            abort_handle,
        }
    }

    /// Wires the background workers and event handlers, returning the client
    /// that drives the connection. Shared by [`Bot::run`] and [`Bot::spawn`].
    fn start_background(self) -> Arc<Client> {
        let Bot {
            client,
            sync_task_receiver,
            event_handlers,
            event_delivery,
            raw_handlers,
            pair_code_options,
            task_instrument: _,
        } = self;

        if let Some(receiver) = sync_task_receiver {
            client.start_sync_task_worker(receiver);
        }

        if !event_handlers.is_empty() {
            client
                .core
                .event_bus
                .subscribe_handler(Arc::new(CallbackBusAdapter::new(
                    client.clone(),
                    event_handlers,
                    event_delivery,
                )))
                .detach();
        }
        for handler in raw_handlers {
            client.core.event_bus.subscribe_handler(handler).detach();
        }

        // If pair code options are set, spawn a task to request pair code after socket is ready
        if let Some(options) = pair_code_options {
            let client_for_pair = client.clone();
            client.runtime.spawn(Box::pin(async move {
                // Wait for socket to be ready (before login) with 30 second timeout
                if let Err(e) = client_for_pair
                    .wait_for_socket(std::time::Duration::from_secs(30))
                    .await
                {
                    warn!(target: "Bot/PairCode", "Timeout waiting for socket: {}", e);
                    return;
                }

                // Check if already logged in (paired via QR or existing session)
                if client_for_pair.is_logged_in() {
                    info!(target: "Bot/PairCode", "Already logged in, skipping pair code request");
                    return;
                }

                // Request pair code
                match client_for_pair.pair_with_code(options).await {
                    Ok(code) => {
                        info!(target: "Bot/PairCode", "Pair code generated: {}", code);
                    }
                    Err(e) => {
                        warn!(target: "Bot/PairCode", "Failed to request pair code: {}", e);
                    }
                }
            })).detach();
        }

        client
    }
}

/// Builder for [`Bot`] using the typestate pattern.
///
/// The four type parameters track whether the required fields (backend,
/// transport factory, HTTP client, runtime) have been provided: `build()` is
/// only available once all four are [`Provided`], turning missing-field errors
/// into compile-time errors. With the default cargo features, transport, HTTP
/// client and runtime start [`Provided`] (Tokio WebSocket, ureq, Tokio), so
/// only the backend is required.
pub struct BotBuilder<
    B = MissingBackend,
    T = MissingTransport,
    H = MissingHttpClient,
    R = MissingRuntime,
> {
    // Required fields (guaranteed present when B/T/H/R = Provided)
    backend: Option<Arc<dyn Backend>>,
    transport_factory: Option<Arc<dyn crate::transport::TransportFactory>>,
    http_client: Option<Arc<dyn crate::http::HttpClient>>,
    runtime: Option<Arc<dyn Runtime>>,
    // Optional fields
    event_handlers: Vec<RegisteredHandler>,
    event_delivery: EventDelivery,
    raw_handlers: Vec<Arc<dyn EventHandler>>,
    custom_enc_handlers: HashMap<String, Arc<dyn EncHandler>>,
    inbound_durability_hook: Option<Arc<dyn InboundDurabilityHook>>,
    override_version: Option<(u32, u32, u32)>,
    device_props_override: Option<DevicePropsOverride>,
    pair_code_options: Option<PairCodeOptions>,
    skip_history_sync: bool,
    initial_push_name: Option<String>,
    cache_config: CacheConfig,
    wanted_pre_key_count: Option<usize>,
    resend_rate_limit: Option<(u32, u32)>,
    task_instrument: Option<Arc<dyn wacore::stats::TaskInstrument>>,
    alloc_meter: Option<Arc<wacore::stats::AllocMeter>>,
    #[cfg(feature = "plugins")]
    plugins: Vec<PluginRegistration>,
    #[cfg(feature = "plugins")]
    plugin_host_config: PluginHostConfig,
    _marker: PhantomData<(B, T, H, R)>,
}

impl BotBuilder<MissingBackend, DefaultTransportState, DefaultHttpState, DefaultRuntimeState> {
    fn new() -> Self {
        Self {
            backend: None,
            transport_factory: default_transport_factory(),
            http_client: default_http_client(),
            runtime: default_runtime(),
            event_handlers: Vec::new(),
            event_delivery: EventDelivery::default(),
            raw_handlers: Vec::new(),
            custom_enc_handlers: HashMap::new(),
            inbound_durability_hook: None,
            override_version: None,
            device_props_override: None,
            pair_code_options: None,
            skip_history_sync: false,
            initial_push_name: None,
            cache_config: CacheConfig::default(),
            wanted_pre_key_count: None,
            resend_rate_limit: None,
            task_instrument: None,
            alloc_meter: None,
            #[cfg(feature = "plugins")]
            plugins: Vec::new(),
            #[cfg(feature = "plugins")]
            plugin_host_config: PluginHostConfig::default(),
            _marker: PhantomData,
        }
    }
}

impl<B, T, H, R> BotBuilder<B, T, H, R> {
    /// Re-tags the typestate without touching any field, so each required-field
    /// setter states only its own transition and the field list lives here.
    fn cast<B2, T2, H2, R2>(self) -> BotBuilder<B2, T2, H2, R2> {
        BotBuilder {
            backend: self.backend,
            transport_factory: self.transport_factory,
            http_client: self.http_client,
            runtime: self.runtime,
            event_handlers: self.event_handlers,
            event_delivery: self.event_delivery,
            raw_handlers: self.raw_handlers,
            custom_enc_handlers: self.custom_enc_handlers,
            inbound_durability_hook: self.inbound_durability_hook,
            override_version: self.override_version,
            device_props_override: self.device_props_override,
            pair_code_options: self.pair_code_options,
            skip_history_sync: self.skip_history_sync,
            initial_push_name: self.initial_push_name,
            cache_config: self.cache_config,
            wanted_pre_key_count: self.wanted_pre_key_count,
            resend_rate_limit: self.resend_rate_limit,
            task_instrument: self.task_instrument,
            alloc_meter: self.alloc_meter,
            #[cfg(feature = "plugins")]
            plugins: self.plugins,
            #[cfg(feature = "plugins")]
            plugin_host_config: self.plugin_host_config,
            _marker: PhantomData,
        }
    }

    // ── Required-field setters (each transitions one type parameter) ──────

    /// Use a backend implementation for storage. This is the only required
    /// field when the default transport/HTTP/runtime features are enabled.
    ///
    /// The backend is wrapped in an `Arc` internally; use
    /// [`BotBuilder::with_backend_arc`] to pass an already-shared backend.
    ///
    /// # Example
    /// ```rust,ignore
    /// let bot = Bot::builder()
    ///     .with_backend(SqliteStore::new("whatsapp.db").await?)
    ///     .build()
    ///     .await?;
    /// ```
    pub fn with_backend(self, backend: impl Backend + 'static) -> BotBuilder<Provided, T, H, R> {
        self.with_backend_arc(Arc::new(backend))
    }

    /// [`BotBuilder::with_backend`] for an already-shared `Arc<dyn Backend>`.
    pub fn with_backend_arc(mut self, backend: Arc<dyn Backend>) -> BotBuilder<Provided, T, H, R> {
        self.backend = Some(backend);
        self.cast()
    }

    /// Set the transport factory for creating network connections, replacing
    /// the `tokio-transport` default when that feature is enabled.
    pub fn with_transport_factory<F>(mut self, factory: F) -> BotBuilder<B, Provided, H, R>
    where
        F: crate::transport::TransportFactory + 'static,
    {
        self.transport_factory = Some(Arc::new(factory));
        self.cast()
    }

    /// Set the HTTP client used for media operations and version fetching,
    /// replacing the `ureq-client` default when that feature is enabled.
    pub fn with_http_client<C>(mut self, client: C) -> BotBuilder<B, T, Provided, R>
    where
        C: crate::http::HttpClient + 'static,
    {
        self.http_client = Some(Arc::new(client));
        self.cast()
    }

    /// Set the async runtime implementation, replacing the `tokio-runtime`
    /// default when that feature is enabled.
    pub fn with_runtime<Rt: Runtime>(mut self, runtime: Rt) -> BotBuilder<B, T, H, Provided> {
        self.runtime = Some(Arc::new(runtime));
        self.cast()
    }

    /// Instrument the client's internal tasks with a
    /// [`TaskInstrument`](wacore::stats::TaskInstrument) hook, called around
    /// each poll (and around blocking work).
    ///
    /// Runtime-agnostic: the hook wraps whatever runtime the client uses, so
    /// every task spawned through the `Runtime` trait is covered, and
    /// [`Bot::run`] meters the main run loop itself — the read loop reports
    /// on either launch path (`voip` media tasks spawn directly on Tokio and
    /// are not covered). Pass a
    /// [`CpuMeter`](wacore::stats::CpuMeter) for per-session CPU accounting
    /// (keep a clone to read snapshots), or a custom hook to scope
    /// allocator-attribution or platform samplers to this client's work.
    /// Default: no hook — the runtime is used untouched.
    ///
    /// Occupies the same single-instrument slot as
    /// [`with_alloc_meter`](Self::with_alloc_meter) (last setter wins): calling
    /// this after `with_alloc_meter` drops the typed alloc-meter handle, so
    /// [`Client::resource_report`](crate::Client::resource_report)'s `alloc`
    /// field reverts to `None`.
    ///
    /// # Example
    /// ```rust,ignore
    /// use std::sync::Arc;
    /// use wacore::stats::CpuMeter;
    ///
    /// let cpu = Arc::new(CpuMeter::new());
    /// let bot = Bot::builder()
    ///     .with_backend(backend)
    ///     .with_task_instrument(cpu.clone())
    ///     .build()
    ///     .await?;
    /// // later: cpu.snapshot().busy
    /// ```
    pub fn with_task_instrument(
        mut self,
        instrument: Arc<dyn wacore::stats::TaskInstrument>,
    ) -> Self {
        self.task_instrument = Some(instrument);
        // Clear any alloc-meter handle: only the last instrument set is driven by
        // the poll hooks, so a stale handle would make resource_report() report a
        // never-updated all-zero snapshot instead of `None`.
        self.alloc_meter = None;
        self
    }

    /// Install an [`AllocMeter`](wacore::stats::AllocMeter) as this client's
    /// task instrument and keep a typed handle so [`Client::resource_report`]
    /// folds in its allocation-churn snapshot.
    ///
    /// Sugar over [`with_task_instrument`](Self::with_task_instrument): it
    /// occupies the single instrument slot (so it's mutually exclusive with a
    /// `CpuMeter` or another hook — last setter wins). The host still installs a
    /// `#[global_allocator]` that calls [`AllocMeter::on_alloc`] /
    /// [`AllocMeter::on_dealloc`]; see `examples/alloc_tracking.rs`.
    ///
    /// [`AllocMeter::on_alloc`]: wacore::stats::AllocMeter::on_alloc
    /// [`AllocMeter::on_dealloc`]: wacore::stats::AllocMeter::on_dealloc
    pub fn with_alloc_meter(mut self, meter: Arc<wacore::stats::AllocMeter>) -> Self {
        self.task_instrument = Some(meter.clone());
        self.alloc_meter = Some(meter);
        self
    }

    /// Register a native plugin without changing the builder's typestate.
    #[cfg(feature = "plugins")]
    #[cfg_attr(docsrs, doc(cfg(feature = "plugins")))]
    pub fn with_plugin<P: ClientPlugin>(mut self, plugin: P) -> Self {
        self.plugins.push(PluginRegistration::new(plugin));
        self
    }

    /// Register an already-shared native plugin without changing its marker type.
    #[cfg(feature = "plugins")]
    #[cfg_attr(docsrs, doc(cfg(feature = "plugins")))]
    pub fn with_plugin_arc<P: ClientPlugin>(mut self, plugin: Arc<P>) -> Self {
        self.plugins.push(PluginRegistration::new_arc(plugin));
        self
    }

    /// Register a manifest-ID-keyed plugin that exposes no Rust typed API.
    #[cfg(feature = "plugins")]
    #[cfg_attr(docsrs, doc(cfg(feature = "plugins")))]
    pub fn with_untyped_plugin<P: UntypedClientPlugin>(mut self, plugin: P) -> Self {
        self.plugins.push(PluginRegistration::new_untyped(plugin));
        self
    }

    /// Register an already-shared manifest-ID-keyed plugin.
    #[cfg(feature = "plugins")]
    #[cfg_attr(docsrs, doc(cfg(feature = "plugins")))]
    pub fn with_untyped_plugin_arc<P: UntypedClientPlugin>(mut self, plugin: Arc<P>) -> Self {
        self.plugins
            .push(PluginRegistration::new_untyped_arc(plugin));
        self
    }

    /// Configure plugin lifecycle and tracked-task deadlines.
    #[cfg(feature = "plugins")]
    #[cfg_attr(docsrs, doc(cfg(feature = "plugins")))]
    pub fn with_plugin_host_config(mut self, config: PluginHostConfig) -> Self {
        self.plugin_host_config = config;
        self
    }

    // ── Event handler registration (additive; order of registration is kept,
    //    but handlers run on their own tasks, so cross-event ordering is not
    //    guaranteed) ──────────────────────────────────────────────────────

    /// Register a handler that receives every event kind.
    pub fn on_event<F, Fut>(self, handler: F) -> Self
    where
        F: Fn(Arc<Event>, Arc<Client>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.register_event_handler(EventInterest::ALL, handler)
    }

    /// Register a handler that receives only the given event kinds. The bus
    /// skips materializing (and boxing the handler future for) every other
    /// kind, so a narrowly-scoped bot does not pay for events it ignores.
    pub fn on_event_for<F, Fut>(self, kinds: &[EventKind], handler: F) -> Self
    where
        F: Fn(Arc<Event>, Arc<Client>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.register_event_handler(EventInterest::of(kinds), handler)
    }

    /// Run `handler` for every incoming message, with a ready
    /// [`MessageContext`] (reply/react/edit helpers included).
    ///
    /// [`Event::Messages`] batches (one per commit during an offline drain,
    /// single-message on live traffic) are fanned out here in arrival order,
    /// awaiting each handler before the next — per-message bots keep their
    /// ergonomics and gain in-batch ordering.
    ///
    /// The handler is CALLED for every message in the batch up front and the
    /// returned futures then run in order (an `async` closure runs no body
    /// code at call time, so for the typical handler this is unobservable).
    /// Interleaving call+await instead would hold a `MessageContext` across
    /// an await, which is not `Send` on wasm32.
    pub fn on_message<F, Fut>(self, handler: F) -> Self
    where
        F: Fn(MessageContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.on_event_for(&[EventKind::Messages], move |event, client| {
            // Futures are built before the async block: `MessageContext` is
            // not `Send` on wasm32 (the Client's trait objects aren't), so it
            // must never be held across an await — only the handler futures
            // (which the `Fut: Send` bound covers) may cross one.
            let futures: Vec<Fut> = event
                .messages()
                .map(|m| handler(MessageContext::from_inbound(m, Arc::clone(&client))))
                .collect();
            async move {
                for future in futures {
                    future.await;
                }
            }
        })
    }

    /// Run `handler` with the QR payload (and validity window) each time a
    /// pairing QR code is issued. Render `code` as a QR image for scanning.
    pub fn on_qr_code<F, Fut>(self, handler: F) -> Self
    where
        F: Fn(String, std::time::Duration) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.on_event_for(&[EventKind::PairingQrCode], move |event, _client| {
            let fut = match &*event {
                Event::PairingQrCode(qr) => Some(handler(qr.code.clone(), qr.timeout)),
                _ => None,
            };
            async move {
                if let Some(fut) = fut {
                    fut.await
                }
            }
        })
    }

    /// Run `handler` with the 8-character pairing code (and validity window)
    /// generated by [`BotBuilder::with_pair_code`] linking.
    pub fn on_pair_code<F, Fut>(self, handler: F) -> Self
    where
        F: Fn(String, std::time::Duration) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.on_event_for(&[EventKind::PairingCode], move |event, _client| {
            let fut = match &*event {
                Event::PairingCode(pc) => Some(handler(pc.code.clone(), pc.timeout)),
                _ => None,
            };
            async move {
                if let Some(fut) = fut {
                    fut.await
                }
            }
        })
    }

    /// Run `handler` when the server asks the companion to refresh an
    /// in-progress pairing code ([`Event::PairingCodeRefresh`]). The `bool` is
    /// `force_manual`. The typical reaction is to request a fresh code via
    /// [`Client::pair_with_code`] with the same phone number.
    pub fn on_pair_code_refresh<F, Fut>(self, handler: F) -> Self
    where
        F: Fn(bool, Arc<Client>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.on_event_for(&[EventKind::PairingCodeRefresh], move |event, client| {
            let fut = match &*event {
                Event::PairingCodeRefresh(r) => Some(handler(r.force_manual, client)),
                _ => None,
            };
            async move {
                if let Some(fut) = fut {
                    fut.await
                }
            }
        })
    }

    /// Run `handler` once the client is connected and authenticated.
    pub fn on_connected<F, Fut>(self, handler: F) -> Self
    where
        F: Fn(Arc<Client>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.on_event_for(&[EventKind::Connected], move |_event, client| {
            handler(client)
        })
    }

    /// Run `handler` when the device is logged out (unlinked from the phone).
    pub fn on_logged_out<F, Fut>(self, handler: F) -> Self
    where
        F: Fn(crate::types::events::LoggedOut) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.on_event_for(&[EventKind::LoggedOut], move |event, _client| {
            let fut = match &*event {
                Event::LoggedOut(info) => Some(handler(info.clone())),
                _ => None,
            };
            async move {
                if let Some(fut) = fut {
                    fut.await
                }
            }
        })
    }

    fn register_event_handler<F, Fut>(mut self, interest: EventInterest, handler: F) -> Self
    where
        F: Fn(Arc<Event>, Arc<Client>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.event_handlers.push(RegisteredHandler {
            callback: Arc::new(move |event, client| Box::pin(handler(event, client))),
            interest,
        });
        self
    }

    /// Register a struct-based [`EventHandler`] directly on the event bus.
    ///
    /// Unlike the closure registrars, the handler keeps its state in `&self`
    /// (no per-field clone dance) and `handle_event` runs inline on the
    /// dispatch path: spawn your own task for slow work.
    pub fn with_event_handler(mut self, handler: impl EventHandler + 'static) -> Self {
        self.raw_handlers.push(Arc::new(handler));
        self
    }

    /// Choose how registered callbacks receive events. Defaults to
    /// [`EventDelivery::Concurrent`]; use [`EventDelivery::Ordered`] for
    /// in-arrival-order, bounded delivery. Only affects the closure-based
    /// callbacks, not raw
    /// [`with_event_handler`](Self::with_event_handler) handlers, which always
    /// run inline on the dispatch path.
    pub fn with_event_delivery(mut self, delivery: EventDelivery) -> Self {
        self.event_delivery = delivery;
        self
    }

    // ── Optional configuration ─────────────────────────────────────────────

    /// Register a custom handler for a specific encrypted message type
    ///
    /// # Arguments
    /// * `enc_type` - The encrypted message type (e.g., "frskmsg")
    /// * `handler` - The handler implementation for this type
    pub fn with_enc_handler<Eh>(mut self, enc_type: impl Into<String>, handler: Eh) -> Self
    where
        Eh: EncHandler + 'static,
    {
        self.custom_enc_handlers
            .insert(enc_type.into(), Arc::new(handler));
        self
    }

    /// Register an inbound durability hook for at-least-once delivery.
    ///
    /// By default the client acks a message as soon as it is decrypted
    /// (at-most-once): a crash or failed commit before the consumer persists it
    /// loses the message. With a hook registered, the ack is deferred until the
    /// hook commits the message; on failure the message is redelivered on the
    /// next connect. The hook must be idempotent (dedupe by `(chat, sender, id)`,
    /// since stanza ids are only unique within a chat/sender). See
    /// [`InboundDurabilityHook`] for the full contract and caveats.
    pub fn with_inbound_durability_hook<Dh>(mut self, hook: Dh) -> Self
    where
        Dh: InboundDurabilityHook + 'static,
    {
        self.inbound_durability_hook = Some(Arc::new(hook));
        self
    }

    /// Override the WhatsApp version used by the client.
    ///
    /// By default, the client will automatically fetch the latest version from WhatsApp's servers.
    /// Use this method to force a specific version instead.
    ///
    /// # Arguments
    /// * `version` - A tuple of (primary, secondary, tertiary) version numbers
    pub fn with_version(mut self, version: (u32, u32, u32)) -> Self {
        self.override_version = Some(version);
        self
    }

    /// Override the device properties sent to WhatsApp servers.
    /// This allows customizing how your device appears on the linked devices list.
    ///
    /// `platform_type` controls the display name in Linked Devices; defaults
    /// to `Unknown` ("Unknown device"). Only applied on the initial pairing.
    ///
    /// # Example
    /// ```rust,ignore
    /// use waproto::whatsapp::device_props::PlatformType;
    /// use wacore::store::DevicePropsOverride;
    ///
    /// Bot::builder()
    ///     .with_backend(backend)
    ///     .with_device_props(
    ///         DevicePropsOverride::new()
    ///             .with_os("macOS")
    ///             .with_platform_type(PlatformType::CHROME),
    ///     );
    /// ```
    pub fn with_device_props(mut self, override_: DevicePropsOverride) -> Self {
        self.device_props_override = Some(override_);
        self
    }

    /// Configure pair code authentication to run automatically after connecting.
    ///
    /// When set, the pair code request will be sent automatically after establishing
    /// a connection, and the pairing code will be dispatched via `Event::PairingCode`
    /// (see [`BotBuilder::on_pair_code`]). This runs concurrently with QR code
    /// pairing - whichever completes first wins.
    ///
    /// # Example
    /// ```rust,ignore
    /// use whatsapp_rust::pair_code::PairCodeOptions;
    ///
    /// // Platform identity is derived from `DeviceProps` configured via
    /// // `Bot::builder().with_device_props(...)`. Explicit overrides below
    /// // are optional — omit them to let derivation do the right thing.
    /// let bot = Bot::builder()
    ///     .with_backend(backend)
    ///     .with_pair_code(PairCodeOptions {
    ///         phone_number: "15551234567".to_string(),
    ///         custom_code: Some("ABCD1234".to_string()),
    ///         ..Default::default()
    ///     })
    ///     .on_pair_code(|code, _timeout| async move {
    ///         println!("Enter this code on your phone: {code}");
    ///     })
    ///     .build()
    ///     .await?;
    /// ```
    pub fn with_pair_code(mut self, options: PairCodeOptions) -> Self {
        self.pair_code_options = Some(options);
        self
    }

    /// Skip processing of history sync notifications from the phone.
    ///
    /// When enabled, the client will acknowledge all incoming history sync
    /// notifications (so the phone considers them delivered) but will not
    /// download or process any historical data (INITIAL_BOOTSTRAP, RECENT,
    /// FULL, PUSH_NAME, etc.). A debug log entry is emitted for each skipped
    /// notification. This is useful for bot use cases where message history
    /// is not needed.
    ///
    /// Default: `false` (history sync is processed normally).
    pub fn skip_history_sync(mut self) -> Self {
        self.skip_history_sync = true;
        self
    }

    /// Set how many one-time pre-keys are generated and uploaded per batch.
    ///
    /// Defaults to WA Web's UPLOAD_KEYS_COUNT (812). The value is clamped to the
    /// protocol-safe range at upload time. Useful for memory-constrained or
    /// embedded consumers that want a smaller batch.
    pub fn with_wanted_pre_key_count(mut self, count: usize) -> Self {
        self.wanted_pre_key_count = Some(count);
        self
    }

    /// Tune the per-chat outbound resend rate limiter.
    ///
    /// Outbound retry resends to a chat are bounded by a token bucket: `burst`
    /// is the instantaneous allowance, `refill_per_min` the sustained ceiling
    /// per chat. This caps the aggregate resend rate WhatsApp's anti-abuse
    /// penalizes during a PN to LID migration fan-out, while throttled devices
    /// still recover via the fresh-SKDM mark. A `burst` of 0 disables it.
    ///
    /// Defaults are conservative (burst 20, refill 10/min) and apply without
    /// calling this. Can also be retuned live via
    /// [`Client::set_resend_rate_limit`](crate::Client::set_resend_rate_limit).
    pub fn with_resend_rate_limit(mut self, burst: u32, refill_per_min: u32) -> Self {
        self.resend_rate_limit = Some((burst, refill_per_min));
        self
    }

    /// Set an initial push name on the device before connecting.
    ///
    /// This is included in the `ClientPayload` during registration, allowing the
    /// mock server to deterministically assign phone numbers based on push name
    /// (same push name = same phone, enabling multi-device testing).
    pub fn with_push_name(mut self, name: impl Into<String>) -> Self {
        self.initial_push_name = Some(name.into());
        self
    }

    /// Configure cache TTL and capacity settings.
    ///
    /// By default, all caches match WhatsApp Web behavior. Use this method
    /// to customize cache durations for your use case.
    ///
    /// # Example
    /// ```rust,ignore
    /// use whatsapp_rust::{CacheConfig, CacheEntryConfig};
    ///
    /// // Disable TTL for group and device caches (good for bots with few groups)
    /// let bot = Bot::builder()
    ///     .with_backend(backend)
    ///     .with_cache_config(CacheConfig {
    ///         group_cache: CacheEntryConfig::new(None, 1_000),
    ///         device_registry_cache: CacheEntryConfig::new(None, 5_000),
    ///         ..Default::default()
    ///     })
    ///     .build()
    ///     .await?;
    /// ```
    pub fn with_cache_config(mut self, config: CacheConfig) -> Self {
        self.cache_config = config;
        self
    }
}

// ── build() — only available when all 4 required fields are Provided ─────

impl BotBuilder<Provided, Provided, Provided, Provided> {
    /// Boxed barrier: see [`Bot::run`]. Building the client wires every cache
    /// and background loop, so an unboxed await here would duplicate that
    /// whole construction graph into the consumer crate.
    pub async fn build(self) -> std::result::Result<Bot, BotBuilderError> {
        self.build_boxed().await
    }

    #[inline(never)]
    fn build_boxed(
        self,
    ) -> wacore::runtime::BoxFuture<'static, std::result::Result<Bot, BotBuilderError>> {
        Box::pin(self.build_graph())
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.bot.build", level = "debug", skip_all, err(Debug))
    )]
    async fn build_graph(self) -> std::result::Result<Bot, BotBuilderError> {
        // Destructure to extract required fields — typestate guarantees all are Some.
        let (Some(runtime), Some(backend), Some(transport_factory), Some(http_client)) = (
            self.runtime,
            self.backend,
            self.transport_factory,
            self.http_client,
        ) else {
            unreachable!("typestate guarantees all required fields are Provided")
        };

        let task_instrument = self.task_instrument;
        let alloc_meter = self.alloc_meter;

        // Note: For multi-account mode, create the backend with SqliteStore::new_for_device()
        // before passing it to with_backend_arc()
        let persistence_manager = Arc::new(PersistenceManager::new(backend).await?);

        // Apply initial push name if specified (for deterministic mock server phone assignment)
        if let Some(name) = self.initial_push_name {
            persistence_manager
                .process_command(DeviceCommand::SetPushName(name))
                .await;
        }

        if let Some(override_) = self.device_props_override
            && !override_.is_empty()
        {
            // Field-by-field to avoid Debug-formatting waproto types (keeps their
            // generated Debug impls out of the binary).
            info!(
                "Applying device props override: os={:?} version={:?} platform_type={:?} history_sync_config={}",
                override_.os.as_deref(),
                override_.version.as_ref().map(|v| {
                    format!(
                        "{}.{}.{}",
                        v.primary.unwrap_or(0),
                        v.secondary.unwrap_or(0),
                        v.tertiary.unwrap_or(0)
                    )
                }),
                override_.platform_type.map(|p| p as i32),
                if override_.history_sync_config.is_some() {
                    "overridden"
                } else {
                    "default"
                },
            );
            persistence_manager
                .process_command(DeviceCommand::SetDeviceProps(override_))
                .await;
        }

        info!("Creating client...");
        let client_builder = Client::builder()
            .with_runtime_arc(runtime)
            .with_persistence_manager(persistence_manager)
            .with_transport_factory_arc(transport_factory)
            .with_http_client_arc(http_client)
            .with_cache_config(self.cache_config)
            .with_custom_enc_handlers(self.custom_enc_handlers)
            .with_skip_history_sync(self.skip_history_sync)
            .with_background_saver_interval(std::time::Duration::from_secs(30));
        #[cfg(feature = "plugins")]
        let client_builder = client_builder
            .with_plugin_registrations(self.plugins)
            .with_plugin_host_config(self.plugin_host_config);
        let mut client_builder = client_builder;

        if let Some(version) = self.override_version {
            client_builder = client_builder.with_version_override(version);
        }
        if let Some(hook) = self.inbound_durability_hook {
            client_builder = client_builder.with_inbound_durability_hook_arc(hook);
        }
        if let Some(count) = self.wanted_pre_key_count {
            client_builder = client_builder.with_wanted_pre_key_count(count);
        }
        if let Some((burst, refill_per_min)) = self.resend_rate_limit {
            client_builder = client_builder.with_resend_rate_limit(burst, refill_per_min);
        }
        client_builder = match alloc_meter {
            Some(meter) => client_builder.with_alloc_meter(meter),
            None => match task_instrument.clone() {
                Some(instrument) => client_builder.with_task_instrument(instrument),
                None => client_builder,
            },
        };

        let (client, sync_task_receiver) = client_builder.build().await?.into_parts();

        Ok(Bot {
            client,
            sync_task_receiver: Some(sync_task_receiver),
            event_handlers: self.event_handlers,
            event_delivery: self.event_delivery,
            raw_handlers: self.raw_handlers,
            pair_code_options: self.pair_code_options,
            task_instrument,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TokioRuntime;
    use crate::http::{HttpClient, HttpRequest, HttpResponse};
    use crate::store::SqliteStore;
    use anyhow::Result;
    use whatsapp_rust_tokio_transport::TokioWebSocketTransportFactory;

    // Mock HTTP client for testing
    #[derive(Debug, Clone)]
    struct MockHttpClient;

    #[async_trait::async_trait]
    impl HttpClient for MockHttpClient {
        async fn execute(&self, _request: HttpRequest) -> Result<HttpResponse> {
            // Return a mock response for version fetching
            Ok(HttpResponse {
                status_code: 200,
                body: br#"self.__swData=JSON.parse(/*BTDS*/"{\"dynamic_data\":{\"SiteData\":{\"server_revision\":1026131876,\"client_revision\":1026131876}}}");"#.to_vec(),
            })
        }
    }

    async fn create_test_sqlite_backend() -> Arc<dyn Backend> {
        let temp_db = format!(
            "file:memdb_bot_{}?mode=memory&cache=shared",
            uuid::Uuid::new_v4()
        );
        Arc::new(
            SqliteStore::new(&temp_db)
                .await
                .expect("Failed to create test SqliteStore"),
        ) as Arc<dyn Backend>
    }

    async fn create_test_sqlite_backend_for_device(device_id: i32) -> Arc<dyn Backend> {
        let temp_db = format!(
            "file:memdb_bot_{}?mode=memory&cache=shared",
            uuid::Uuid::new_v4()
        );
        Arc::new(
            SqliteStore::new_for_device(&temp_db, device_id)
                .await
                .expect("Failed to create test SqliteStore"),
        ) as Arc<dyn Backend>
    }

    async fn test_client() -> Arc<Client> {
        Bot::builder()
            .with_backend_arc(create_test_sqlite_backend().await)
            .with_transport_factory(TokioWebSocketTransportFactory::new())
            .with_http_client(MockHttpClient)
            .with_runtime(TokioRuntime)
            .build()
            .await
            .expect("build")
            .client()
    }

    #[cfg(feature = "plugins")]
    struct BotBuilderPlugin;

    #[cfg(feature = "plugins")]
    impl ClientPlugin for BotBuilderPlugin {
        type Api = &'static str;

        fn manifest(&self) -> crate::plugins::PluginManifest {
            crate::plugins::PluginManifest::new("bot-builder-test", "0.1.0")
        }

        fn install(
            &self,
            _context: crate::plugins::PluginContext,
        ) -> wacore::runtime::BoxFuture<'_, anyhow::Result<Arc<Self::Api>>> {
            Box::pin(async { Ok(Arc::new("installed")) })
        }
    }

    #[cfg(feature = "plugins")]
    #[tokio::test]
    async fn typestate_builder_preserves_registered_plugins() {
        let bot = Bot::builder()
            .with_plugin(BotBuilderPlugin)
            .with_backend_arc(create_test_sqlite_backend().await)
            .with_transport_factory(TokioWebSocketTransportFactory::new())
            .with_http_client(MockHttpClient)
            .with_runtime(TokioRuntime)
            .build()
            .await
            .expect("bot plugin build");
        assert_eq!(
            bot.client().plugin::<BotBuilderPlugin>().as_deref(),
            Some(&"installed")
        );
        bot.client().disconnect().await;
    }

    fn pairing_code_event(code: &str) -> Arc<Event> {
        Arc::new(Event::PairingCode(
            crate::types::events::PairingCode::builder()
                .code(code.to_string())
                .timeout(std::time::Duration::ZERO)
                .build(),
        ))
    }

    /// `EventDelivery::Ordered` delivers events to a callback in arrival order —
    /// the ordered consumer contract the concurrent default can't promise.
    #[tokio::test]
    async fn ordered_delivery_preserves_arrival_order() {
        let client = test_client().await;
        let (order_tx, order_rx) = async_channel::unbounded::<String>();
        let handler = RegisteredHandler {
            callback: Arc::new(move |event, _client| {
                let order_tx = order_tx.clone();
                Box::pin(async move {
                    if let Event::PairingCode(pc) = &*event {
                        let _ = order_tx.send(pc.code.clone()).await;
                    }
                })
            }),
            interest: EventInterest::ALL,
        };
        let adapter = CallbackBusAdapter::new(
            client.clone(),
            vec![handler],
            EventDelivery::Ordered { capacity: 64 },
        );

        let n = 20;
        for i in 0..n {
            adapter.handle_event(pairing_code_event(&i.to_string()));
        }

        let mut seen = Vec::with_capacity(n);
        for _ in 0..n {
            seen.push(
                tokio::time::timeout(std::time::Duration::from_secs(5), order_rx.recv())
                    .await
                    .expect("timed out waiting for ordered callback")
                    .expect("callback ran"),
            );
        }
        let expected: Vec<String> = (0..n).map(|i| i.to_string()).collect();
        assert_eq!(
            seen, expected,
            "ordered delivery must preserve arrival order"
        );
    }

    /// A full bounded mailbox drops events (counted in `events_dropped`) instead
    /// of blocking the dispatch path or growing without bound.
    #[tokio::test]
    async fn ordered_delivery_drops_and_counts_when_full() {
        let client = test_client().await;
        let (started_tx, started_rx) = async_channel::bounded::<()>(1);
        let (release_tx, release_rx) = async_channel::bounded::<()>(1);
        let handler = RegisteredHandler {
            callback: Arc::new(move |event, _client| {
                let started_tx = started_tx.clone();
                let release_rx = release_rx.clone();
                Box::pin(async move {
                    // Only the first event parks the single drainer, so the
                    // capacity-1 mailbox is deterministically full for the rest.
                    if let Event::PairingCode(pc) = &*event
                        && pc.code == "0"
                    {
                        let _ = started_tx.send(()).await;
                        let _ = release_rx.recv().await;
                    }
                })
            }),
            interest: EventInterest::ALL,
        };
        let adapter = CallbackBusAdapter::new(
            client.clone(),
            vec![handler],
            EventDelivery::Ordered { capacity: 1 },
        );

        // #0 is pulled by the drainer, which then parks on `release`; the mailbox
        // is now empty and the drainer won't take another until released.
        adapter.handle_event(pairing_code_event("0"));
        tokio::time::timeout(std::time::Duration::from_secs(5), started_rx.recv())
            .await
            .expect("timed out waiting for drainer to start")
            .expect("drainer entered callback");

        adapter.handle_event(pairing_code_event("1")); // fills capacity-1 mailbox
        adapter.handle_event(pairing_code_event("2")); // dropped
        adapter.handle_event(pairing_code_event("3")); // dropped

        assert_eq!(
            client.stats.events_dropped(),
            2,
            "events past a full mailbox must be dropped and counted"
        );

        let _ = release_tx.send(()).await; // unpark so the drainer can exit cleanly
    }

    /// A panicking callback must not kill the single ordered drainer — later
    /// events still get delivered. The panic fires while *creating* the future
    /// (before the async block), the case a poll-only guard would miss.
    #[tokio::test]
    async fn ordered_delivery_survives_a_panicking_callback() {
        let client = test_client().await;
        let (tx, rx) = async_channel::unbounded::<String>();
        let handler = RegisteredHandler {
            callback: Arc::new(move |event: Arc<Event>, _client| {
                let tx = tx.clone();
                if let Event::PairingCode(pc) = &*event {
                    assert_ne!(&pc.code, "boom", "deliberate test panic");
                }
                Box::pin(async move {
                    if let Event::PairingCode(pc) = &*event {
                        let _ = tx.send(pc.code.clone()).await;
                    }
                })
            }),
            interest: EventInterest::ALL,
        };
        let adapter = CallbackBusAdapter::new(
            client.clone(),
            vec![handler],
            EventDelivery::Ordered { capacity: 16 },
        );

        adapter.handle_event(pairing_code_event("a"));
        adapter.handle_event(pairing_code_event("boom")); // callback panics, isolated
        adapter.handle_event(pairing_code_event("b"));

        let mut seen = Vec::with_capacity(2);
        for _ in 0..2 {
            seen.push(
                tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv())
                    .await
                    .expect("timed out waiting for post-panic delivery")
                    .expect("callback ran"),
            );
        }
        assert_eq!(
            seen,
            vec!["a".to_string(), "b".to_string()],
            "delivery must continue after a callback panic"
        );
    }

    /// Regression: a `with_task_instrument` after `with_alloc_meter` must drop
    /// the alloc-meter handle, so `resource_report()` doesn't report a
    /// never-driven all-zero snapshot as if the meter were active.
    #[tokio::test]
    async fn task_instrument_after_alloc_meter_clears_stale_handle() {
        use wacore::stats::{AllocMeter, CpuMeter};

        let bot = Bot::builder()
            .with_backend_arc(create_test_sqlite_backend().await)
            .with_transport_factory(TokioWebSocketTransportFactory::new())
            .with_http_client(MockHttpClient)
            .with_runtime(TokioRuntime)
            .with_alloc_meter(Arc::new(AllocMeter::new()))
            .with_task_instrument(Arc::new(CpuMeter::new()))
            .build()
            .await
            .expect("build");
        assert!(
            bot.client().resource_report().await.alloc.is_none(),
            "a later with_task_instrument must drop the alloc-meter handle"
        );

        // Reverse order: with_alloc_meter is last, so its snapshot is present.
        let bot = Bot::builder()
            .with_backend_arc(create_test_sqlite_backend().await)
            .with_transport_factory(TokioWebSocketTransportFactory::new())
            .with_http_client(MockHttpClient)
            .with_runtime(TokioRuntime)
            .with_task_instrument(Arc::new(CpuMeter::new()))
            .with_alloc_meter(Arc::new(AllocMeter::new()))
            .build()
            .await
            .expect("build");
        assert!(
            bot.client().resource_report().await.alloc.is_some(),
            "with_alloc_meter installs the handle when it is the last setter"
        );
    }

    #[tokio::test]
    async fn test_bot_builder_single_device() {
        let backend = create_test_sqlite_backend().await;
        let transport = TokioWebSocketTransportFactory::new();
        let http_client = MockHttpClient;

        let bot = Bot::builder()
            .with_backend_arc(backend)
            .with_transport_factory(transport)
            .with_http_client(http_client)
            .with_runtime(TokioRuntime)
            .build()
            .await
            .expect("Failed to build bot");

        // Verify bot was created successfully
        let _client = bot.client();
    }

    #[tokio::test]
    async fn test_bot_builder_multi_device() {
        // Create a backend configured for device ID 42
        let backend = create_test_sqlite_backend_for_device(42).await;
        let transport = TokioWebSocketTransportFactory::new();

        let bot = Bot::builder()
            .with_backend_arc(backend)
            .with_transport_factory(transport)
            .with_http_client(MockHttpClient)
            .with_runtime(TokioRuntime)
            .build()
            .await
            .expect("Failed to build bot");

        // Verify bot was created successfully
        let _client = bot.client();
    }

    #[tokio::test]
    async fn test_bot_builder_defaults_only_need_backend() {
        // With the default features on, transport/HTTP/runtime are pre-filled,
        // so providing the backend alone must reach build().
        let temp_db = format!(
            "file:memdb_bot_{}?mode=memory&cache=shared",
            uuid::Uuid::new_v4()
        );
        let store = SqliteStore::new(&temp_db)
            .await
            .expect("Failed to create test SqliteStore");

        let bot = Bot::builder()
            .with_backend(store)
            // Override the default HTTP client so the test doesn't hit the network.
            .with_http_client(MockHttpClient)
            .build()
            .await
            .expect("Failed to build bot from defaults");

        let _client = bot.client();
    }

    #[tokio::test]
    async fn test_bot_builder_with_version_override() {
        let backend = create_test_sqlite_backend().await;
        let transport = TokioWebSocketTransportFactory::new();
        let http_client = MockHttpClient;

        let bot = Bot::builder()
            .with_backend_arc(backend)
            .with_transport_factory(transport)
            .with_http_client(http_client)
            .with_version((2, 3000, 123456789))
            .with_runtime(TokioRuntime)
            .build()
            .await
            .expect("Failed to build bot with version override");

        // Verify the bot was created successfully
        let client = bot.client();

        // Check that the override version is stored in the client
        assert_eq!(client.override_version, Some((2, 3000, 123456789)));
    }

    #[tokio::test]
    async fn test_bot_builder_with_device_props_override() {
        let backend = create_test_sqlite_backend().await;
        let transport = TokioWebSocketTransportFactory::new();
        let http_client = MockHttpClient;

        let custom_os = "CustomOS".to_string();
        let custom_version = wa::device_props::AppVersion {
            primary: Some(99),
            secondary: Some(88),
            tertiary: Some(77),
            ..Default::default()
        };

        let bot = Bot::builder()
            .with_backend_arc(backend)
            .with_transport_factory(transport)
            .with_http_client(http_client)
            .with_device_props(
                DevicePropsOverride::new()
                    .with_os(custom_os.clone())
                    .with_version(custom_version.clone()),
            )
            .with_runtime(TokioRuntime)
            .build()
            .await
            .expect("Failed to build bot with device props override");

        let client = bot.client();
        let persistence_manager = client.persistence_manager();
        let device = persistence_manager.get_device_snapshot();

        // Verify the device props were overridden
        assert_eq!(device.device_props.os, Some(custom_os));
        assert_eq!(
            device.device_props.version.as_option(),
            Some(&custom_version)
        );
    }

    #[tokio::test]
    async fn test_bot_builder_with_os_only_override() {
        let backend = create_test_sqlite_backend().await;
        let transport = TokioWebSocketTransportFactory::new();
        let http_client = MockHttpClient;

        let custom_os = "CustomOS".to_string();

        let bot = Bot::builder()
            .with_backend_arc(backend)
            .with_transport_factory(transport)
            .with_http_client(http_client)
            .with_device_props(DevicePropsOverride::new().with_os(custom_os.clone()))
            .with_runtime(TokioRuntime)
            .build()
            .await
            .expect("Failed to build bot with OS only override");

        let client = bot.client();
        let persistence_manager = client.persistence_manager();
        let device = persistence_manager.get_device_snapshot();

        // Verify only OS was overridden, version should be default
        assert_eq!(device.device_props.os, Some(custom_os));
        // Version should be the default since we didn't override it
        assert_eq!(
            device.device_props.version.as_option(),
            Some(&wacore::store::Device::default_device_props_version())
        );
    }

    #[tokio::test]
    async fn test_bot_builder_with_version_only_override() {
        let backend = create_test_sqlite_backend().await;
        let transport = TokioWebSocketTransportFactory::new();
        let http_client = MockHttpClient;

        let custom_version = wa::device_props::AppVersion {
            primary: Some(99),
            secondary: Some(88),
            tertiary: Some(77),
            ..Default::default()
        };

        let bot = Bot::builder()
            .with_backend_arc(backend)
            .with_http_client(http_client)
            .with_transport_factory(transport)
            .with_device_props(DevicePropsOverride::new().with_version(custom_version.clone()))
            .with_runtime(TokioRuntime)
            .build()
            .await
            .expect("Failed to build bot with version only override");

        let client = bot.client();
        let persistence_manager = client.persistence_manager();
        let device = persistence_manager.get_device_snapshot();

        // Verify only version was overridden, OS should be default ("rust")
        assert_eq!(
            device.device_props.version.as_option(),
            Some(&custom_version)
        );
        // OS should be the default since we didn't override it
        assert_eq!(
            device.device_props.os,
            Some(wacore::store::Device::default_os().to_string())
        );
    }

    #[tokio::test]
    async fn test_bot_builder_with_platform_type_override() {
        let backend = create_test_sqlite_backend().await;
        let transport = TokioWebSocketTransportFactory::new();
        let http_client = MockHttpClient;

        let bot = Bot::builder()
            .with_backend_arc(backend)
            .with_transport_factory(transport)
            .with_http_client(http_client)
            .with_device_props(
                DevicePropsOverride::new()
                    .with_platform_type(wa::device_props::PlatformType::CHROME),
            )
            .with_runtime(TokioRuntime)
            .build()
            .await
            .expect("Failed to build bot with platform type override");

        let client = bot.client();
        let persistence_manager = client.persistence_manager();
        let device = persistence_manager.get_device_snapshot();

        // Verify platform type was set to Chrome
        assert_eq!(
            device.device_props.platform_type,
            Some(wa::device_props::PlatformType::CHROME)
        );
        // OS and version should remain default
        assert_eq!(
            device.device_props.os,
            Some(wacore::store::Device::default_os().to_string())
        );
        assert_eq!(
            device.device_props.version.as_option(),
            Some(&wacore::store::Device::default_device_props_version())
        );
    }

    #[tokio::test]
    async fn test_bot_builder_with_full_device_props_override() {
        let backend = create_test_sqlite_backend().await;
        let transport = TokioWebSocketTransportFactory::new();
        let http_client = MockHttpClient;

        let custom_os = "macOS".to_string();
        let custom_version = wa::device_props::AppVersion {
            primary: Some(2),
            secondary: Some(0),
            tertiary: Some(0),
            ..Default::default()
        };
        let custom_platform = wa::device_props::PlatformType::SAFARI;

        let bot = Bot::builder()
            .with_backend_arc(backend)
            .with_transport_factory(transport)
            .with_http_client(http_client)
            .with_device_props(
                DevicePropsOverride::new()
                    .with_os(custom_os.clone())
                    .with_version(custom_version.clone())
                    .with_platform_type(custom_platform),
            )
            .with_runtime(TokioRuntime)
            .build()
            .await
            .expect("Failed to build bot with full device props override");

        let client = bot.client();
        let persistence_manager = client.persistence_manager();
        let device = persistence_manager.get_device_snapshot();

        // Verify all device props were overridden
        assert_eq!(device.device_props.os, Some(custom_os));
        assert_eq!(
            device.device_props.version.as_option(),
            Some(&custom_version)
        );
        assert_eq!(device.device_props.platform_type, Some(custom_platform));
    }

    #[tokio::test]
    async fn test_bot_builder_skip_history_sync() {
        let backend = create_test_sqlite_backend().await;
        let transport = TokioWebSocketTransportFactory::new();
        let http_client = MockHttpClient;

        let bot = Bot::builder()
            .with_backend_arc(backend)
            .with_transport_factory(transport)
            .with_http_client(http_client)
            .skip_history_sync()
            .with_runtime(TokioRuntime)
            .build()
            .await
            .expect("Failed to build bot with skip_history_sync");

        assert!(bot.client().skip_history_sync_enabled());
    }

    #[tokio::test]
    async fn test_bot_builder_default_history_sync_enabled() {
        let backend = create_test_sqlite_backend().await;
        let transport = TokioWebSocketTransportFactory::new();
        let http_client = MockHttpClient;

        let bot = Bot::builder()
            .with_backend_arc(backend)
            .with_transport_factory(transport)
            .with_http_client(http_client)
            .with_runtime(TokioRuntime)
            .build()
            .await
            .expect("Failed to build bot");

        assert!(!bot.client().skip_history_sync_enabled());
    }

    #[tokio::test]
    async fn test_bot_builder_wanted_pre_key_count() {
        let backend = create_test_sqlite_backend().await;
        let transport = TokioWebSocketTransportFactory::new();
        let http_client = MockHttpClient;

        let bot = Bot::builder()
            .with_backend_arc(backend)
            .with_transport_factory(transport)
            .with_http_client(http_client)
            .with_wanted_pre_key_count(200)
            .with_runtime(TokioRuntime)
            .build()
            .await
            .expect("Failed to build bot with custom pre-key count");

        assert_eq!(bot.client().wanted_pre_key_count(), 200);
    }

    #[tokio::test]
    async fn test_bot_builder_default_wanted_pre_key_count() {
        let backend = create_test_sqlite_backend().await;
        let transport = TokioWebSocketTransportFactory::new();
        let http_client = MockHttpClient;

        let bot = Bot::builder()
            .with_backend_arc(backend)
            .with_transport_factory(transport)
            .with_http_client(http_client)
            .with_runtime(TokioRuntime)
            .build()
            .await
            .expect("Failed to build bot");

        assert_eq!(
            bot.client().wanted_pre_key_count(),
            crate::prekeys::DEFAULT_WANTED_PRE_KEY_COUNT
        );
    }

    #[tokio::test]
    async fn registered_handlers_accumulate_instead_of_replacing() {
        let backend = create_test_sqlite_backend().await;

        let bot = Bot::builder()
            .with_backend_arc(backend)
            .with_transport_factory(TokioWebSocketTransportFactory::new())
            .with_http_client(MockHttpClient)
            .with_runtime(TokioRuntime)
            .on_message(|_ctx| async {})
            .on_qr_code(|_code, _timeout| async {})
            .on_event(|_event, _client| async {})
            .build()
            .await
            .expect("Failed to build bot");

        assert_eq!(bot.event_handlers.len(), 3);

        // The catch-all handler widens the union to every kind.
        let interest = combined_interest(&bot.event_handlers);
        assert_eq!(interest, EventInterest::ALL);
    }

    #[test]
    fn combined_interest_is_the_union_of_handler_interests() {
        let noop: EventHandlerCallback = Arc::new(|_event, _client| Box::pin(async {}));
        let handlers = vec![
            RegisteredHandler {
                callback: noop.clone(),
                interest: EventInterest::of(&[EventKind::Messages]),
            },
            RegisteredHandler {
                callback: noop,
                interest: EventInterest::of(&[EventKind::PairingQrCode]),
            },
        ];

        let interest = combined_interest(&handlers);
        assert!(interest.wants(EventKind::Messages));
        assert!(interest.wants(EventKind::PairingQrCode));
        assert!(!interest.wants(EventKind::Receipt));
    }

    #[tokio::test]
    async fn from_arc_does_not_deep_clone() {
        let backend = create_test_sqlite_backend().await;
        let bot = Bot::builder()
            .with_backend_arc(backend)
            .with_transport_factory(TokioWebSocketTransportFactory::new())
            .with_http_client(MockHttpClient)
            .with_runtime(TokioRuntime)
            .build()
            .await
            .expect("Failed to build bot");

        let original = Arc::new(wa::Message {
            conversation: Some("ping".to_string()),
            ..Default::default()
        });
        let original_ptr = Arc::as_ptr(&original);

        let ctx =
            MessageContext::from_arc(Arc::clone(&original), &MessageInfo::default(), bot.client());

        assert!(std::ptr::eq(Arc::as_ptr(&ctx.message), original_ptr));
    }

    async fn test_context_with_info(info: MessageInfo) -> MessageContext {
        let backend = create_test_sqlite_backend().await;
        let bot = Bot::builder()
            .with_backend_arc(backend)
            .with_transport_factory(TokioWebSocketTransportFactory::new())
            .with_http_client(MockHttpClient)
            .with_runtime(TokioRuntime)
            .build()
            .await
            .expect("Failed to build bot");
        MessageContext::from_arc(Arc::new(wa::Message::default()), &info, bot.client())
    }

    fn react_info(chat: &str, sender: &str, id: &str, is_group: bool) -> MessageInfo {
        use crate::types::message::MessageSource;
        MessageInfo {
            id: id.to_string(),
            source: MessageSource {
                chat: chat.parse().expect("chat jid"),
                sender: sender.parse().expect("sender jid"),
                is_group,
                is_from_me: false,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    #[tokio::test]
    async fn react_target_key_carries_group_participant() {
        let info = react_info(
            "120363012345@g.us",
            "15551230000@s.whatsapp.net",
            "MSGID01",
            true,
        );
        let ctx = test_context_with_info(info).await;
        let key = ctx.message_key();

        assert_eq!(key.remote_jid.as_deref(), Some("120363012345@g.us"));
        assert_eq!(key.id.as_deref(), Some("MSGID01"));
        assert_eq!(key.from_me, Some(false));
        // Group reactions must attribute the original sender via participant.
        assert_eq!(
            key.participant.as_deref(),
            Some("15551230000@s.whatsapp.net")
        );
    }

    #[tokio::test]
    async fn react_target_key_omits_participant_in_dm() {
        let info = react_info(
            "15559990000@s.whatsapp.net",
            "15559990000@s.whatsapp.net",
            "MSGID02",
            false,
        );
        let ctx = test_context_with_info(info).await;
        let key = ctx.message_key();

        assert_eq!(
            key.remote_jid.as_deref(),
            Some("15559990000@s.whatsapp.net")
        );
        // DMs do not carry participant (matches WA Web message-key shape).
        assert!(key.participant.is_none());
    }

    #[tokio::test]
    async fn react_target_key_carries_status_author() {
        let info = react_info(
            "status@broadcast",
            "15551112222@s.whatsapp.net",
            "MSGID03",
            false,
        );
        let ctx = test_context_with_info(info).await;
        let key = ctx.message_key();

        // status@broadcast reactions fan out to the author's devices, so the
        // author must be present in participant for the send path to extract it.
        assert_eq!(
            key.participant.as_deref(),
            Some("15551112222@s.whatsapp.net")
        );
    }

    #[tokio::test]
    async fn run_metered_reports_to_instrument() {
        let meter = Arc::new(wacore::stats::CpuMeter::new());
        run_metered(
            async {
                tokio::task::yield_now().await;
            },
            Some(meter.clone()),
        )
        .await;
        // yield_now forces Pending once, so the wrapper must see >= 2 polls,
        // and the busy-time attribution path must have accumulated something.
        assert!(meter.snapshot().polls >= 2);
        assert!(meter.snapshot().busy > std::time::Duration::ZERO);

        // No instrument: plain passthrough must still drive to completion.
        run_metered(async {}, None).await;
    }
}

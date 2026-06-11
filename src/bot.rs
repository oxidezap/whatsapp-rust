use crate::cache_config::CacheConfig;
use crate::client::Client;
use crate::pair_code::PairCodeOptions;
use crate::store::commands::DeviceCommand;
use crate::store::error::StoreError;
use crate::store::persistence_manager::PersistenceManager;
use crate::store::traits::Backend;
use crate::types::enc_handler::EncHandler;
use crate::types::events::{Event, EventHandler, EventInterest, EventKind};
use crate::types::message::MessageInfo;
use log::{info, warn};
use std::collections::HashMap;
use std::future::Future;
use std::marker::PhantomData;
use std::pin::Pin;
use std::sync::Arc;
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
    /// [`MessageContext::from_arc`]/[`MessageContext::from_event`] when an
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

    pub fn from_event(event: &Event, client: Arc<Client>) -> Option<Self> {
        let (msg, info) = event.as_message()?;
        Some(Self::from_arc(Arc::clone(msg), info, client))
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.bot.send_message", level = "debug", skip_all, fields(chat = %self.info.source.chat.observe()), err(Debug)))]
    pub async fn send_message(
        &self,
        message: wa::Message,
    ) -> Result<crate::send::SendResult, anyhow::Error> {
        self.client
            .send_message(&self.info.source.chat, message)
            .await
    }

    /// Reply with plain text in the same chat, without quoting.
    pub async fn reply(
        &self,
        text: impl Into<String>,
    ) -> Result<crate::send::SendResult, anyhow::Error> {
        self.send_message(wa::Message::text(text)).await
    }

    /// Reply with plain text, quoting the received message.
    pub async fn reply_quoting(
        &self,
        text: impl Into<String>,
    ) -> Result<crate::send::SendResult, anyhow::Error> {
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
    ) -> Result<String, anyhow::Error> {
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
    ) -> Result<(), anyhow::Error> {
        self.client
            .revoke_message(&self.info.source.chat, message_id, revoke_type)
            .await
    }

    /// React to the incoming message. An empty `emoji` removes a previous
    /// reaction. The target key (including the group/status participant) is
    /// taken from [`MessageContext::message_key`].
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.bot.react", level = "debug", skip_all, fields(chat = %self.info.source.chat.observe()), err(Debug)))]
    pub async fn react(&self, emoji: &str) -> Result<crate::send::SendResult, anyhow::Error> {
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

/// Bridges the registered closures onto the core event bus. Each interested
/// callback runs on its own spawned task, so a slow handler stalls neither the
/// bus nor its sibling handlers; in exchange, ordering across events is not
/// guaranteed.
struct CallbackBusAdapter {
    client: Arc<Client>,
    handlers: Vec<RegisteredHandler>,
    interest: EventInterest,
}

impl EventHandler for CallbackBusAdapter {
    fn handle_event(&self, event: Arc<Event>) {
        let kind = event.kind();
        for handler in &self.handlers {
            if !handler.interest.wants(kind) {
                continue;
            }
            let callback = handler.callback.clone();
            let client = self.client.clone();
            let event = Arc::clone(&event);

            self.client
                .runtime
                .spawn(Box::pin(async move {
                    callback(event, client).await;
                }))
                .detach();
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

pub struct Bot {
    client: Arc<Client>,
    sync_task_receiver: Option<async_channel::Receiver<crate::sync_task::MajorSyncTask>>,
    event_handlers: Vec<RegisteredHandler>,
    raw_handlers: Vec<Arc<dyn EventHandler>>,
    pair_code_options: Option<PairCodeOptions>,
}

impl std::fmt::Debug for Bot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Bot")
            .field("client", &"<Client>")
            .field("sync_task_receiver", &self.sync_task_receiver.is_some())
            .field("event_handlers", &self.event_handlers.len())
            .field("raw_handlers", &self.raw_handlers.len())
            .field("pair_code_options", &self.pair_code_options.is_some())
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
        let client = self.start_background();
        client.run().await;
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
            raw_handlers,
            pair_code_options,
        } = self;

        if let Some(receiver) = sync_task_receiver {
            let worker_client = Arc::downgrade(&client);
            client
                .runtime
                .spawn(Box::pin(async move {
                    while let Ok(task) = receiver.recv().await {
                        let Some(worker_client) = worker_client.upgrade() else {
                            break;
                        };

                        worker_client.process_sync_task(task).await;
                    }
                    info!("Sync worker shutting down.");
                }))
                .detach();
        }

        if !event_handlers.is_empty() {
            let interest = combined_interest(&event_handlers);
            client
                .core
                .event_bus
                .add_handler(Arc::new(CallbackBusAdapter {
                    client: client.clone(),
                    handlers: event_handlers,
                    interest,
                }));
        }
        for handler in raw_handlers {
            client.core.event_bus.add_handler(handler);
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
    raw_handlers: Vec<Arc<dyn EventHandler>>,
    custom_enc_handlers: HashMap<String, Arc<dyn EncHandler>>,
    override_version: Option<(u32, u32, u32)>,
    device_props_override: Option<DevicePropsOverride>,
    pair_code_options: Option<PairCodeOptions>,
    skip_history_sync: bool,
    initial_push_name: Option<String>,
    cache_config: CacheConfig,
    wanted_pre_key_count: Option<usize>,
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
            raw_handlers: Vec::new(),
            custom_enc_handlers: HashMap::new(),
            override_version: None,
            device_props_override: None,
            pair_code_options: None,
            skip_history_sync: false,
            initial_push_name: None,
            cache_config: CacheConfig::default(),
            wanted_pre_key_count: None,
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
            raw_handlers: self.raw_handlers,
            custom_enc_handlers: self.custom_enc_handlers,
            override_version: self.override_version,
            device_props_override: self.device_props_override,
            pair_code_options: self.pair_code_options,
            skip_history_sync: self.skip_history_sync,
            initial_push_name: self.initial_push_name,
            cache_config: self.cache_config,
            wanted_pre_key_count: self.wanted_pre_key_count,
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
    pub fn on_message<F, Fut>(self, handler: F) -> Self
    where
        F: Fn(MessageContext) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        self.on_event_for(&[EventKind::Message], move |event, client| {
            let fut = MessageContext::from_event(&event, client).map(&handler);
            async move {
                if let Some(fut) = fut {
                    fut.await
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
                Event::PairingQrCode { code, timeout } => Some(handler(code.clone(), *timeout)),
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
                Event::PairingCode { code, timeout } => Some(handler(code.clone(), *timeout)),
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
    ///             .with_platform_type(PlatformType::Chrome),
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
            info!("Applying device props override: {:?}", override_);
            persistence_manager
                .process_command(DeviceCommand::SetDeviceProps(override_))
                .await;
        }

        info!("Creating client...");
        let (client, sync_task_receiver) = Client::new_with_cache_config(
            runtime.clone(),
            persistence_manager.clone(),
            transport_factory,
            http_client,
            self.override_version,
            self.cache_config,
        )
        .await;

        let saver_handle = persistence_manager.run_background_saver(
            runtime,
            std::time::Duration::from_secs(30),
            client.shutdown_signal(),
        );
        // Tie the saver task to Arc<Client> so extracting client() and outliving
        // Bot keeps periodic persistence alive. Client::drop on the last Arc
        // drops the AbortHandle and aborts the task.
        let _ = client.saver_handle.set(saver_handle);

        // Register custom enc handlers. Immutable after build, so set the whole
        // map once; the receive hot path then reads it lock-free.
        let _ = client.custom_enc_handlers.set(self.custom_enc_handlers);

        if self.skip_history_sync {
            client.set_skip_history_sync(true);
        }

        if let Some(count) = self.wanted_pre_key_count {
            client.set_wanted_pre_key_count(count);
        }

        Ok(Bot {
            client,
            sync_task_receiver: Some(sync_task_receiver),
            event_handlers: self.event_handlers,
            raw_handlers: self.raw_handlers,
            pair_code_options: self.pair_code_options,
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
                    .with_version(custom_version),
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
        assert_eq!(device.device_props.version, Some(custom_version));
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
            device.device_props.version,
            Some(wacore::store::Device::default_device_props_version())
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
            .with_device_props(DevicePropsOverride::new().with_version(custom_version))
            .with_runtime(TokioRuntime)
            .build()
            .await
            .expect("Failed to build bot with version only override");

        let client = bot.client();
        let persistence_manager = client.persistence_manager();
        let device = persistence_manager.get_device_snapshot();

        // Verify only version was overridden, OS should be default ("rust")
        assert_eq!(device.device_props.version, Some(custom_version));
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
                    .with_platform_type(wa::device_props::PlatformType::Chrome),
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
            Some(wa::device_props::PlatformType::Chrome as i32)
        );
        // OS and version should remain default
        assert_eq!(
            device.device_props.os,
            Some(wacore::store::Device::default_os().to_string())
        );
        assert_eq!(
            device.device_props.version,
            Some(wacore::store::Device::default_device_props_version())
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
        let custom_platform = wa::device_props::PlatformType::Safari;

        let bot = Bot::builder()
            .with_backend_arc(backend)
            .with_transport_factory(transport)
            .with_http_client(http_client)
            .with_device_props(
                DevicePropsOverride::new()
                    .with_os(custom_os.clone())
                    .with_version(custom_version)
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
        assert_eq!(device.device_props.version, Some(custom_version));
        assert_eq!(
            device.device_props.platform_type,
            Some(custom_platform as i32)
        );
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
                interest: EventInterest::of(&[EventKind::Message]),
            },
            RegisteredHandler {
                callback: noop,
                interest: EventInterest::of(&[EventKind::PairingQrCode]),
            },
        ];

        let interest = combined_interest(&handlers);
        assert!(interest.wants(EventKind::Message));
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
}

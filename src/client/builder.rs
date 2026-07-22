use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use thiserror::Error;

use super::Client;
#[cfg(feature = "client-lifecycle")]
use super::{ClientLifecycle, LifecycleRegistration};
use crate::cache_config::CacheConfig;
use crate::http::HttpClient;
#[cfg(feature = "plugins")]
use crate::plugins::{ClientPlugin, PluginHost, PluginPlan, PluginPlanError, PluginRegistration};
use crate::store::error::StoreError;
use crate::store::persistence_manager::PersistenceManager;
use crate::sync_task::MajorSyncTask;
use crate::transport::TransportFactory;
use crate::types::durability_hook::InboundDurabilityHook;
use crate::types::enc_handler::EncHandler;
use wacore::runtime::Runtime;

/// Result of constructing a [`Client`].
///
/// Consume with [`ClientBuild::into_client`] for the standard worker or
/// [`ClientBuild::into_parts`] when the host owns that worker itself.
pub struct ClientBuild {
    client: Arc<Client>,
    sync_task_receiver: async_channel::Receiver<MajorSyncTask>,
}

impl ClientBuild {
    pub(crate) fn new(
        client: Arc<Client>,
        sync_task_receiver: async_channel::Receiver<MajorSyncTask>,
    ) -> Self {
        Self {
            client,
            sync_task_receiver,
        }
    }

    /// Transfer the client and start its default major-sync worker.
    pub fn into_client(self) -> Arc<Client> {
        let (client, sync_task_receiver) = self.into_parts();
        client.start_sync_task_worker(sync_task_receiver);
        client
    }

    /// Transfer ownership of the client and its sole sync-task receiver.
    /// The caller must drain the receiver for history sync to keep working.
    pub fn into_parts(self) -> (Arc<Client>, async_channel::Receiver<MajorSyncTask>) {
        (self.client, self.sync_task_receiver)
    }
}

/// A validated client-construction failure.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ClientBuilderError {
    #[error("missing async runtime")]
    MissingRuntime,
    #[error("missing persistence manager")]
    MissingPersistenceManager,
    #[error("missing transport factory")]
    MissingTransportFactory,
    #[error("missing HTTP client")]
    MissingHttpClient,
    #[error("background saver interval must be greater than zero")]
    InvalidBackgroundSaverInterval,
    #[error("the configured backend does not support the inbound durability hook: {0}")]
    UnsupportedDurabilityBackend(String),
    #[cfg(feature = "client-lifecycle")]
    #[cfg_attr(docsrs, doc(cfg(feature = "client-lifecycle")))]
    #[error("client lifecycle installation failed: {0}")]
    LifecycleInstall(#[source] anyhow::Error),
    #[cfg(feature = "plugins")]
    #[cfg_attr(docsrs, doc(cfg(feature = "plugins")))]
    #[error("plugin host installation failed: {0}")]
    PluginInstall(#[source] anyhow::Error),
    #[cfg(feature = "plugins")]
    #[cfg_attr(docsrs, doc(cfg(feature = "plugins")))]
    #[error("invalid plugin plan: {0}")]
    PluginPlan(#[from] PluginPlanError),
}

/// Runtime-validated, low-level builder for [`Client`].
///
/// Unlike [`crate::BotBuilder`], this builder deliberately does not use
/// typestate. FFI and embedded hosts can populate dependencies dynamically and
/// receive a typed error without encoding Rust generic state in their wrapper.
pub struct ClientBuilder {
    runtime: Option<Arc<dyn Runtime>>,
    persistence_manager: Option<Arc<PersistenceManager>>,
    transport_factory: Option<Arc<dyn TransportFactory>>,
    http_client: Option<Arc<dyn HttpClient>>,
    override_version: Option<(u32, u32, u32)>,
    cache_config: CacheConfig,
    custom_enc_handlers: HashMap<String, Arc<dyn EncHandler>>,
    inbound_durability_hook: Option<Arc<dyn InboundDurabilityHook>>,
    skip_history_sync: bool,
    wanted_pre_key_count: Option<usize>,
    resend_rate_limit: Option<(u32, u32)>,
    task_instrument: Option<Arc<dyn wacore::stats::TaskInstrument>>,
    alloc_meter: Option<Arc<wacore::stats::AllocMeter>>,
    background_saver_interval: Option<Duration>,
    #[cfg(feature = "client-lifecycle")]
    lifecycle: Option<Arc<dyn ClientLifecycle>>,
    #[cfg(feature = "plugins")]
    plugins: Vec<PluginRegistration>,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientBuilder {
    /// Create an empty builder. All four platform dependencies are required.
    pub fn new() -> Self {
        Self {
            runtime: None,
            persistence_manager: None,
            transport_factory: None,
            http_client: None,
            override_version: None,
            cache_config: CacheConfig::default(),
            custom_enc_handlers: HashMap::new(),
            inbound_durability_hook: None,
            skip_history_sync: false,
            wanted_pre_key_count: None,
            resend_rate_limit: None,
            task_instrument: None,
            alloc_meter: None,
            background_saver_interval: None,
            #[cfg(feature = "client-lifecycle")]
            lifecycle: None,
            #[cfg(feature = "plugins")]
            plugins: Vec::new(),
        }
    }

    pub fn with_runtime<R>(mut self, runtime: R) -> Self
    where
        R: Runtime,
    {
        self.runtime = Some(Arc::new(runtime));
        self
    }

    pub fn with_runtime_arc(mut self, runtime: Arc<dyn Runtime>) -> Self {
        self.runtime = Some(runtime);
        self
    }

    pub fn with_persistence_manager(
        mut self,
        persistence_manager: Arc<PersistenceManager>,
    ) -> Self {
        self.persistence_manager = Some(persistence_manager);
        self
    }

    pub fn with_transport_factory<T>(mut self, transport_factory: T) -> Self
    where
        T: TransportFactory + 'static,
    {
        self.transport_factory = Some(Arc::new(transport_factory));
        self
    }

    pub fn with_transport_factory_arc(
        mut self,
        transport_factory: Arc<dyn TransportFactory>,
    ) -> Self {
        self.transport_factory = Some(transport_factory);
        self
    }

    pub fn with_http_client<H>(mut self, http_client: H) -> Self
    where
        H: HttpClient + 'static,
    {
        self.http_client = Some(Arc::new(http_client));
        self
    }

    pub fn with_http_client_arc(mut self, http_client: Arc<dyn HttpClient>) -> Self {
        self.http_client = Some(http_client);
        self
    }

    pub fn with_version_override(mut self, version: (u32, u32, u32)) -> Self {
        self.override_version = Some(version);
        self
    }

    pub fn with_cache_config(mut self, cache_config: CacheConfig) -> Self {
        self.cache_config = cache_config;
        self
    }

    /// Register a handler for one encrypted payload type before the client starts.
    pub fn with_enc_handler<H>(mut self, payload_type: impl Into<String>, handler: H) -> Self
    where
        H: EncHandler + 'static,
    {
        self.custom_enc_handlers
            .insert(payload_type.into(), Arc::new(handler));
        self
    }

    /// Register an already-shared encrypted payload handler.
    pub fn with_enc_handler_arc(
        mut self,
        payload_type: impl Into<String>,
        handler: Arc<dyn EncHandler>,
    ) -> Self {
        self.custom_enc_handlers
            .insert(payload_type.into(), handler);
        self
    }

    pub(crate) fn with_custom_enc_handlers(
        mut self,
        handlers: HashMap<String, Arc<dyn EncHandler>>,
    ) -> Self {
        self.custom_enc_handlers = handlers;
        self
    }

    /// Install the durable-inbound hook after verifying backend support.
    pub fn with_inbound_durability_hook<H>(mut self, hook: H) -> Self
    where
        H: InboundDurabilityHook + 'static,
    {
        self.inbound_durability_hook = Some(Arc::new(hook));
        self
    }

    /// Install an already-shared durable-inbound hook.
    pub fn with_inbound_durability_hook_arc(
        mut self,
        hook: Arc<dyn InboundDurabilityHook>,
    ) -> Self {
        self.inbound_durability_hook = Some(hook);
        self
    }

    pub fn with_skip_history_sync(mut self, skip: bool) -> Self {
        self.skip_history_sync = skip;
        self
    }

    pub fn with_wanted_pre_key_count(mut self, count: usize) -> Self {
        self.wanted_pre_key_count = Some(count);
        self
    }

    pub fn with_resend_rate_limit(mut self, burst: u32, refill_per_min: u32) -> Self {
        self.resend_rate_limit = Some((burst, refill_per_min));
        self
    }

    /// Instrument every task spawned through the configured runtime.
    pub fn with_task_instrument(
        mut self,
        instrument: Arc<dyn wacore::stats::TaskInstrument>,
    ) -> Self {
        self.task_instrument = Some(instrument);
        self.alloc_meter = None;
        self
    }

    /// Install allocation attribution as the task instrument.
    pub fn with_alloc_meter(mut self, meter: Arc<wacore::stats::AllocMeter>) -> Self {
        self.task_instrument = Some(meter.clone());
        self.alloc_meter = Some(meter);
        self
    }

    /// Run periodic device persistence for the lifetime of the client.
    pub fn with_background_saver_interval(mut self, interval: Duration) -> Self {
        self.background_saver_interval = Some(interval);
        self
    }

    /// Install the aggregate lifecycle used by extensions of this client.
    #[cfg(feature = "client-lifecycle")]
    #[cfg_attr(docsrs, doc(cfg(feature = "client-lifecycle")))]
    pub fn with_lifecycle<L>(mut self, lifecycle: L) -> Self
    where
        L: ClientLifecycle + 'static,
    {
        self.lifecycle = Some(Arc::new(lifecycle));
        self
    }

    /// Install an already-shared aggregate lifecycle.
    #[cfg(feature = "client-lifecycle")]
    #[cfg_attr(docsrs, doc(cfg(feature = "client-lifecycle")))]
    pub fn with_lifecycle_arc(mut self, lifecycle: Arc<dyn ClientLifecycle>) -> Self {
        self.lifecycle = Some(lifecycle);
        self
    }

    /// Register a native plugin for transactional installation before services start.
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

    #[cfg(feature = "plugins")]
    pub(crate) fn with_plugin_registrations(
        mut self,
        registrations: Vec<PluginRegistration>,
    ) -> Self {
        self.plugins = registrations;
        self
    }

    /// Validate dependencies, assemble an inert client, then start its services.
    pub async fn build(self) -> Result<ClientBuild, ClientBuilderError> {
        self.build_boxed().await
    }

    #[inline(never)]
    fn build_boxed(
        self,
    ) -> wacore::runtime::BoxFuture<'static, Result<ClientBuild, ClientBuilderError>> {
        Box::pin(async move {
            let runtime = self
                .runtime
                .as_ref()
                .cloned()
                .ok_or(ClientBuilderError::MissingRuntime)?;
            let persistence_manager = self
                .persistence_manager
                .as_ref()
                .cloned()
                .ok_or(ClientBuilderError::MissingPersistenceManager)?;
            let transport_factory = self
                .transport_factory
                .as_ref()
                .cloned()
                .ok_or(ClientBuilderError::MissingTransportFactory)?;
            let http_client = self
                .http_client
                .as_ref()
                .cloned()
                .ok_or(ClientBuilderError::MissingHttpClient)?;

            if self.background_saver_interval == Some(Duration::ZERO) {
                return Err(ClientBuilderError::InvalidBackgroundSaverInterval);
            }

            if self.inbound_durability_hook.is_some() {
                probe_durability_backend(&persistence_manager.backend()).await?;
            }

            self.finish(runtime, persistence_manager, transport_factory, http_client)
                .await
        })
    }

    pub(crate) async fn build_required(
        runtime: Arc<dyn Runtime>,
        persistence_manager: Arc<PersistenceManager>,
        transport_factory: Arc<dyn TransportFactory>,
        http_client: Arc<dyn HttpClient>,
        override_version: Option<(u32, u32, u32)>,
        cache_config: CacheConfig,
    ) -> ClientBuild {
        let result = Self {
            override_version,
            cache_config,
            ..Self::new()
        }
        .finish(runtime, persistence_manager, transport_factory, http_client)
        .await;
        match result {
            Ok(build) => build,
            Err(error) => unreachable!("default lifecycle-free build failed: {error}"),
        }
    }

    async fn finish(
        self,
        runtime: Arc<dyn Runtime>,
        persistence_manager: Arc<PersistenceManager>,
        transport_factory: Arc<dyn TransportFactory>,
        http_client: Arc<dyn HttpClient>,
    ) -> Result<ClientBuild, ClientBuilderError> {
        #[cfg(feature = "plugins")]
        let plugin_plan = PluginPlan::prepare(self.plugins)?;
        let runtime: Arc<dyn Runtime> = match self.task_instrument {
            Some(instrument) => {
                Arc::new(wacore::stats::InstrumentedRuntime::new(runtime, instrument))
            }
            None => runtime,
        };

        #[cfg(feature = "client-lifecycle")]
        let lifecycle_handler = self.lifecycle;
        #[cfg(feature = "plugins")]
        let (lifecycle_handler, plugin_host) = {
            let mut lifecycle_handler = lifecycle_handler;
            let plugin_host = plugin_plan.map(|plan| {
                let host = PluginHost::new(plan, lifecycle_handler.take());
                lifecycle_handler = Some(host.clone());
                host
            });
            (lifecycle_handler, plugin_host)
        };
        #[cfg(feature = "client-lifecycle")]
        let lifecycle = lifecycle_handler.map(|handler| {
            #[cfg(feature = "plugins")]
            if let Some(plugin_host) = &plugin_host {
                return Arc::new(LifecycleRegistration::new_with_timeout(
                    handler,
                    Arc::clone(&runtime),
                    plugin_host.lifecycle_callback_timeout(),
                ));
            }
            Arc::new(LifecycleRegistration::new(handler, Arc::clone(&runtime)))
        });
        let assembly = Client::assemble(
            Arc::clone(&runtime),
            Arc::clone(&persistence_manager),
            transport_factory,
            http_client,
            self.override_version,
            self.cache_config,
            ClientExtensions {
                #[cfg(feature = "client-lifecycle")]
                lifecycle,
                #[cfg(feature = "plugins")]
                plugin_host,
            },
        );
        let client = assembly.client();
        #[cfg(feature = "client-lifecycle")]
        let mut construction = ClientConstructionGuard::new(Arc::clone(&client));

        if !self.custom_enc_handlers.is_empty() {
            let _ = client.custom_enc_handlers.set(self.custom_enc_handlers);
        }
        if let Some(hook) = self.inbound_durability_hook {
            let _ = client.inbound_durability_hook.set(hook);
        }
        if self.skip_history_sync {
            client.set_skip_history_sync(true);
        }
        if let Some(count) = self.wanted_pre_key_count {
            client.set_wanted_pre_key_count(count);
        }
        if let Some((burst, refill_per_min)) = self.resend_rate_limit {
            client.set_resend_rate_limit(burst, refill_per_min);
        }
        if let Some(meter) = self.alloc_meter {
            let _ = client.alloc_meter.set(meter);
        }
        #[cfg(feature = "client-lifecycle")]
        if let Some(lifecycle) = &client.lifecycle
            && let Err(error) = lifecycle.install(Arc::downgrade(&client)).await
        {
            #[cfg(feature = "plugins")]
            if client.plugin_host.is_some() {
                return Err(ClientBuilderError::PluginInstall(error));
            }
            return Err(ClientBuilderError::LifecycleInstall(error));
        }

        let build = assembly.start();
        if let Some(interval) = self.background_saver_interval {
            let saver_handle = persistence_manager.run_background_saver(
                runtime,
                interval,
                build.client.shutdown_signal(),
            );
            let _ = build.client.saver_handle.set(saver_handle);
        }
        #[cfg(feature = "plugins")]
        if let Some(plugin_host) = &client.plugin_host
            && !plugin_host.activate()
        {
            client.signal_shutdown_sync();
            client.shutdown_lifecycle().await;
            return Err(ClientBuilderError::PluginInstall(anyhow::anyhow!(
                "client shutdown began before plugin activation"
            )));
        }
        #[cfg(feature = "client-lifecycle")]
        if let Some(lifecycle) = &client.lifecycle
            && !lifecycle.activate()
        {
            client.signal_shutdown_sync();
            client.shutdown_lifecycle().await;
            #[cfg(feature = "plugins")]
            if client.plugin_host.is_some() {
                return Err(ClientBuilderError::PluginInstall(anyhow::anyhow!(
                    "client shutdown raced plugin activation"
                )));
            }
            return Err(ClientBuilderError::LifecycleInstall(anyhow::anyhow!(
                "client shutdown raced lifecycle activation"
            )));
        }
        #[cfg(feature = "plugins")]
        if let Some(plugin_host) = &client.plugin_host
            && !plugin_host.publish_apis()
        {
            client.signal_shutdown_sync();
            client.shutdown_lifecycle().await;
            return Err(ClientBuilderError::PluginInstall(anyhow::anyhow!(
                "plugin APIs could not be published"
            )));
        }
        #[cfg(feature = "client-lifecycle")]
        construction.disarm();
        Ok(build)
    }
}

async fn probe_durability_backend(
    backend: &Arc<dyn crate::store::traits::Backend>,
) -> Result<(), ClientBuilderError> {
    use portable_atomic::{AtomicU64, Ordering};

    static PROBE_SEQ: AtomicU64 = AtomicU64::new(0);
    const PROBE_JID: &str = "0@s.whatsapp.net";
    const PROBE_PAYLOAD: &[u8] = b"probe";
    let probe_id = format!(
        "__wa_durability_probe_{}_{}__",
        std::process::id(),
        PROBE_SEQ.fetch_add(1, Ordering::Relaxed)
    );
    let map_err =
        |error: StoreError| ClientBuilderError::UnsupportedDurabilityBackend(error.to_string());

    backend
        .store_pending_inbound(PROBE_JID, PROBE_JID, &probe_id, PROBE_PAYLOAD)
        .await
        .map_err(map_err)?;
    let stored = backend
        .get_pending_inbound(PROBE_JID, PROBE_JID, &probe_id)
        .await
        .map_err(map_err)?;
    backend
        .delete_pending_inbound(PROBE_JID, PROBE_JID, &probe_id)
        .await
        .map_err(map_err)?;

    if stored.as_deref() != Some(PROBE_PAYLOAD) {
        return Err(ClientBuilderError::UnsupportedDurabilityBackend(
            "pending-inbound buffer did not round-trip".to_string(),
        ));
    }
    Ok(())
}

/// Owns the only path from a fully allocated client to started background
/// services, preventing callers from publishing a partially configured client.
pub(super) struct ClientAssembly {
    client: Arc<Client>,
    sync_task_receiver: async_channel::Receiver<MajorSyncTask>,
}

#[cfg(feature = "client-lifecycle")]
struct ClientConstructionGuard {
    client: Arc<Client>,
    armed: bool,
}

#[cfg(feature = "client-lifecycle")]
impl ClientConstructionGuard {
    fn new(client: Arc<Client>) -> Self {
        Self {
            client,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

#[cfg(feature = "client-lifecycle")]
impl Drop for ClientConstructionGuard {
    fn drop(&mut self) {
        if self.armed {
            self.client.signal_shutdown_sync();
        }
    }
}

#[derive(Default)]
pub(super) struct ClientExtensions {
    #[cfg(feature = "client-lifecycle")]
    pub(super) lifecycle: Option<Arc<LifecycleRegistration>>,
    #[cfg(feature = "plugins")]
    pub(super) plugin_host: Option<Arc<PluginHost>>,
}

impl ClientAssembly {
    pub(super) fn new(
        client: Arc<Client>,
        sync_task_receiver: async_channel::Receiver<MajorSyncTask>,
    ) -> Self {
        Self {
            client,
            sync_task_receiver,
        }
    }

    pub(super) fn start(self) -> ClientBuild {
        self.client.start_services();
        ClientBuild::new(self.client, self.sync_task_receiver)
    }

    fn client(&self) -> Arc<Client> {
        Arc::clone(&self.client)
    }
}

#[cfg(test)]
mod tests {
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use super::*;
    use crate::runtime_impl::TokioRuntime;
    use crate::test_utils::MockHttpClient;
    use crate::transport::mock::MockTransportFactory;
    use wacore::runtime::AbortHandle;

    #[cfg(feature = "client-lifecycle")]
    struct FailingLifecycle {
        spawns: Arc<AtomicUsize>,
        installed_client: std::sync::Mutex<Option<std::sync::Weak<Client>>>,
    }

    #[cfg(feature = "client-lifecycle")]
    struct RunDuringInstallLifecycle {
        client: async_channel::Sender<Arc<Client>>,
        release: async_channel::Receiver<()>,
        run_finished: async_channel::Sender<()>,
    }

    #[cfg(feature = "client-lifecycle")]
    struct ConnectDuringInstallLifecycle {
        client: async_channel::Sender<Arc<Client>>,
        release: async_channel::Receiver<()>,
        connect_invoked: async_channel::Sender<()>,
        connect_finished: async_channel::Sender<bool>,
    }

    #[cfg(feature = "client-lifecycle")]
    struct BlockingTransportFactory {
        started: async_channel::Sender<()>,
        release: async_channel::Receiver<()>,
    }

    #[cfg(feature = "client-lifecycle")]
    #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
    impl TransportFactory for BlockingTransportFactory {
        async fn create_transport(
            &self,
        ) -> Result<
            (
                Arc<dyn crate::transport::Transport>,
                async_channel::Receiver<crate::transport::TransportEvent>,
            ),
            anyhow::Error,
        > {
            self.started
                .send(())
                .await
                .map_err(|_| anyhow::anyhow!("transport-start receiver closed"))?;
            self.release
                .recv()
                .await
                .map_err(|_| anyhow::anyhow!("transport release closed"))?;
            Err(anyhow::anyhow!("injected transport stop"))
        }
    }

    #[cfg(feature = "client-lifecycle")]
    impl ClientLifecycle for RunDuringInstallLifecycle {
        fn install<'a>(
            &'a self,
            client: std::sync::Weak<Client>,
        ) -> wacore::runtime::BoxFuture<'a, anyhow::Result<()>> {
            Box::pin(async move {
                let client = client
                    .upgrade()
                    .ok_or_else(|| anyhow::anyhow!("client unavailable during install"))?;
                let run_client = client.clone();
                let run_finished = self.run_finished.clone();
                client
                    .runtime
                    .spawn(Box::pin(async move {
                        run_client.run().await;
                        let _ = run_finished.send(()).await;
                    }))
                    .detach();
                self.client
                    .send(client)
                    .await
                    .map_err(|_| anyhow::anyhow!("test client receiver closed"))?;
                self.release
                    .recv()
                    .await
                    .map_err(|_| anyhow::anyhow!("test install release closed"))?;
                Ok(())
            })
        }
    }

    #[cfg(feature = "client-lifecycle")]
    impl ClientLifecycle for ConnectDuringInstallLifecycle {
        fn install<'a>(
            &'a self,
            client: std::sync::Weak<Client>,
        ) -> wacore::runtime::BoxFuture<'a, anyhow::Result<()>> {
            Box::pin(async move {
                let client = client
                    .upgrade()
                    .ok_or_else(|| anyhow::anyhow!("client unavailable during install"))?;
                let connect_client = client.clone();
                let connect_invoked = self.connect_invoked.clone();
                let connect_finished = self.connect_finished.clone();
                client
                    .runtime
                    .spawn(Box::pin(async move {
                        let _ = connect_invoked.send(()).await;
                        let failed = connect_client.connect().await.is_err();
                        let _ = connect_finished.send(failed).await;
                    }))
                    .detach();
                self.client
                    .send(client)
                    .await
                    .map_err(|_| anyhow::anyhow!("test client receiver closed"))?;
                self.release
                    .recv()
                    .await
                    .map_err(|_| anyhow::anyhow!("test install release closed"))?;
                Ok(())
            })
        }
    }

    #[cfg(feature = "client-lifecycle")]
    impl ClientLifecycle for FailingLifecycle {
        fn install<'a>(
            &'a self,
            client: std::sync::Weak<Client>,
        ) -> wacore::runtime::BoxFuture<'a, anyhow::Result<()>> {
            Box::pin(async move {
                assert_eq!(self.spawns.load(Ordering::SeqCst), 0);
                *self
                    .installed_client
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(client);
                Err(anyhow::anyhow!("injected install failure"))
            })
        }
    }

    struct CountingRuntime {
        spawns: Arc<AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl Runtime for CountingRuntime {
        fn spawn(&self, future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>) -> AbortHandle {
            self.spawns.fetch_add(1, Ordering::SeqCst);
            TokioRuntime.spawn(future)
        }

        fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            TokioRuntime.sleep(duration)
        }

        fn spawn_blocking(
            &self,
            f: Box<dyn FnOnce() + Send + 'static>,
        ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            TokioRuntime.spawn_blocking(f)
        }

        fn yield_now(&self) -> Option<Pin<Box<dyn Future<Output = ()> + Send>>> {
            TokioRuntime.yield_now()
        }
    }

    async fn complete_builder() -> ClientBuilder {
        let persistence_manager = Arc::new(
            PersistenceManager::new(crate::test_utils::create_test_backend().await)
                .await
                .expect("persistence manager"),
        );
        ClientBuilder::new()
            .with_runtime(TokioRuntime)
            .with_persistence_manager(persistence_manager)
            .with_transport_factory(MockTransportFactory::new())
            .with_http_client(MockHttpClient)
    }

    #[tokio::test]
    async fn validates_required_dependencies_before_assembly() {
        assert!(matches!(
            ClientBuilder::new().build().await,
            Err(ClientBuilderError::MissingRuntime)
        ));

        assert!(matches!(
            ClientBuilder::new()
                .with_runtime(TokioRuntime)
                .build()
                .await,
            Err(ClientBuilderError::MissingPersistenceManager)
        ));

        let persistence_manager = Arc::new(
            PersistenceManager::new(crate::test_utils::create_test_backend().await)
                .await
                .expect("persistence manager"),
        );
        assert!(matches!(
            ClientBuilder::new()
                .with_runtime(TokioRuntime)
                .with_persistence_manager(Arc::clone(&persistence_manager))
                .build()
                .await,
            Err(ClientBuilderError::MissingTransportFactory)
        ));

        assert!(matches!(
            ClientBuilder::new()
                .with_runtime(TokioRuntime)
                .with_persistence_manager(persistence_manager)
                .with_transport_factory(MockTransportFactory::new())
                .build()
                .await,
            Err(ClientBuilderError::MissingHttpClient)
        ));
    }

    #[tokio::test]
    async fn assembly_is_inert_until_started() {
        let persistence_manager = Arc::new(
            PersistenceManager::new(crate::test_utils::create_test_backend().await)
                .await
                .expect("persistence manager"),
        );
        let spawns = Arc::new(AtomicUsize::new(0));
        let runtime = Arc::new(CountingRuntime {
            spawns: Arc::clone(&spawns),
        }) as Arc<dyn Runtime>;

        let assembly = Client::assemble(
            runtime,
            persistence_manager,
            Arc::new(MockTransportFactory::new()),
            Arc::new(MockHttpClient),
            None,
            CacheConfig::default(),
            ClientExtensions::default(),
        );
        assert_eq!(spawns.load(Ordering::SeqCst), 0);

        let build = assembly.start();
        assert_eq!(spawns.load(Ordering::SeqCst), 2);
        build.into_client().signal_shutdown_sync();
    }

    #[tokio::test]
    #[cfg(feature = "client-lifecycle")]
    async fn run_leaked_during_install_waits_for_complete_construction() {
        let (client_tx, client_rx) = async_channel::bounded(1);
        let (release_tx, release_rx) = async_channel::bounded(1);
        let (run_finished_tx, run_finished_rx) = async_channel::bounded(1);
        let builder = complete_builder()
            .await
            .with_lifecycle(RunDuringInstallLifecycle {
                client: client_tx,
                release: release_rx,
                run_finished: run_finished_tx,
            });
        let build = tokio::spawn(async move { builder.build().await });
        let leaked_client = client_rx
            .recv()
            .await
            .expect("client leaked during install");
        tokio::task::yield_now().await;
        assert!(!leaked_client.is_running.load(Ordering::Acquire));

        release_tx.send(()).await.expect("release installation");
        let client = build
            .await
            .expect("builder task")
            .expect("successful build")
            .into_client();
        tokio::time::timeout(Duration::from_secs(1), async {
            while !client.is_running.load(Ordering::Acquire) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("run released after activation");
        client.signal_shutdown_sync();
        tokio::time::timeout(Duration::from_secs(5), run_finished_rx.recv())
            .await
            .expect("run stop timeout")
            .expect("run stopped");
    }

    #[tokio::test]
    #[cfg(feature = "client-lifecycle")]
    async fn connect_leaked_during_install_waits_for_complete_construction() {
        let (client_tx, client_rx) = async_channel::bounded(1);
        let (install_release_tx, install_release_rx) = async_channel::bounded(1);
        let (connect_invoked_tx, connect_invoked_rx) = async_channel::bounded(1);
        let (connect_finished_tx, connect_finished_rx) = async_channel::bounded(1);
        let (transport_started_tx, transport_started_rx) = async_channel::bounded(1);
        let (transport_release_tx, transport_release_rx) = async_channel::bounded(1);
        let builder = complete_builder()
            .await
            .with_transport_factory(BlockingTransportFactory {
                started: transport_started_tx,
                release: transport_release_rx,
            })
            .with_lifecycle(ConnectDuringInstallLifecycle {
                client: client_tx,
                release: install_release_rx,
                connect_invoked: connect_invoked_tx,
                connect_finished: connect_finished_tx,
            });
        let build = tokio::spawn(async move { builder.build().await });
        let leaked_client = client_rx
            .recv()
            .await
            .expect("client leaked during install");
        connect_invoked_rx
            .recv()
            .await
            .expect("direct connect invoked");

        assert!(
            tokio::time::timeout(Duration::from_millis(100), transport_started_rx.recv())
                .await
                .is_err(),
            "transport started before construction activation"
        );
        assert!(!leaked_client.is_connecting.load(Ordering::Acquire));

        install_release_tx
            .send(())
            .await
            .expect("release installation");
        let client = build
            .await
            .expect("builder task")
            .expect("successful build")
            .into_client();
        tokio::time::timeout(Duration::from_secs(1), transport_started_rx.recv())
            .await
            .expect("connect remained gated after activation")
            .expect("transport-start sender closed");
        transport_release_tx
            .send(())
            .await
            .expect("release transport");
        assert!(
            tokio::time::timeout(Duration::from_secs(1), connect_finished_rx.recv())
                .await
                .expect("direct connect did not finish")
                .expect("connect-finished sender closed")
        );
        client.signal_shutdown_sync();
    }

    #[tokio::test]
    #[cfg(feature = "client-lifecycle")]
    async fn shutdown_during_install_rejects_leaked_connect() {
        let (client_tx, client_rx) = async_channel::bounded(1);
        let (_install_release_tx, install_release_rx) = async_channel::bounded(1);
        let (connect_invoked_tx, connect_invoked_rx) = async_channel::bounded(1);
        let (connect_finished_tx, connect_finished_rx) = async_channel::bounded(1);
        let (transport_started_tx, transport_started_rx) = async_channel::bounded(1);
        let (_transport_release_tx, transport_release_rx) = async_channel::bounded(1);
        let builder = complete_builder()
            .await
            .with_transport_factory(BlockingTransportFactory {
                started: transport_started_tx,
                release: transport_release_rx,
            })
            .with_lifecycle(ConnectDuringInstallLifecycle {
                client: client_tx,
                release: install_release_rx,
                connect_invoked: connect_invoked_tx,
                connect_finished: connect_finished_tx,
            });
        let build = tokio::spawn(async move { builder.build().await });
        let leaked_client = client_rx
            .recv()
            .await
            .expect("client leaked during install");
        connect_invoked_rx
            .recv()
            .await
            .expect("direct connect invoked");
        leaked_client.signal_shutdown_sync();

        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(2), build)
                .await
                .expect("lifecycle install ignored terminal shutdown")
                .expect("builder task"),
            Err(ClientBuilderError::LifecycleInstall(_))
        ));
        assert!(
            tokio::time::timeout(Duration::from_secs(1), connect_finished_rx.recv())
                .await
                .expect("direct connect did not observe rejection")
                .expect("connect-finished sender closed")
        );
        assert!(transport_started_rx.try_recv().is_err());
        assert!(!leaked_client.is_connecting.load(Ordering::Acquire));
    }

    #[tokio::test]
    #[cfg(feature = "client-lifecycle")]
    async fn shutdown_during_install_rejects_leaked_run_and_the_build() {
        let (client_tx, client_rx) = async_channel::bounded(1);
        let (_release_tx, release_rx) = async_channel::bounded(1);
        let (run_finished_tx, run_finished_rx) = async_channel::bounded(1);
        let builder = complete_builder()
            .await
            .with_lifecycle(RunDuringInstallLifecycle {
                client: client_tx,
                release: release_rx,
                run_finished: run_finished_tx,
            });
        let build = tokio::spawn(async move { builder.build().await });
        let leaked_client = client_rx
            .recv()
            .await
            .expect("client leaked during install");
        leaked_client.signal_shutdown_sync();

        assert!(matches!(
            tokio::time::timeout(Duration::from_secs(2), build)
                .await
                .expect("lifecycle install ignored terminal shutdown")
                .expect("builder task"),
            Err(ClientBuilderError::LifecycleInstall(_))
        ));
        tokio::time::timeout(Duration::from_secs(5), run_finished_rx.recv())
            .await
            .expect("rejected run stop timeout")
            .expect("rejected run stopped");
        assert!(!leaked_client.is_running.load(Ordering::Acquire));
    }

    #[tokio::test]
    async fn low_level_builder_installs_options_and_owned_services() {
        let persistence_manager = Arc::new(
            PersistenceManager::new(crate::test_utils::create_test_backend().await)
                .await
                .expect("persistence manager"),
        );
        let spawns = Arc::new(AtomicUsize::new(0));
        let meter = Arc::new(wacore::stats::AllocMeter::new());

        let build = ClientBuilder::new()
            .with_runtime(CountingRuntime {
                spawns: Arc::clone(&spawns),
            })
            .with_persistence_manager(persistence_manager)
            .with_transport_factory(MockTransportFactory::new())
            .with_http_client(MockHttpClient)
            .with_skip_history_sync(true)
            .with_wanted_pre_key_count(123)
            .with_alloc_meter(Arc::clone(&meter))
            .with_background_saver_interval(Duration::from_secs(3600))
            .build()
            .await
            .expect("complete builder");
        let client = build.into_client();

        assert!(client.skip_history_sync_enabled());
        assert_eq!(client.wanted_pre_key_count(), 123);
        assert!(
            client
                .alloc_meter
                .get()
                .is_some_and(|installed| Arc::ptr_eq(installed, &meter))
        );
        assert!(client.saver_handle.get().is_some());
        assert_eq!(spawns.load(Ordering::SeqCst), 4);
        client.signal_shutdown_sync();
    }

    #[tokio::test]
    async fn consuming_build_as_client_keeps_major_sync_worker_alive() {
        let client = complete_builder()
            .await
            .build()
            .await
            .expect("complete builder")
            .into_client();

        assert!(!client.major_sync_task_sender.is_closed());
        client.signal_shutdown_sync();
    }

    #[tokio::test]
    async fn rejects_zero_background_saver_interval() {
        let result = complete_builder()
            .await
            .with_background_saver_interval(Duration::ZERO)
            .build()
            .await;

        assert!(matches!(
            result,
            Err(ClientBuilderError::InvalidBackgroundSaverInterval)
        ));
    }

    #[cfg(feature = "client-lifecycle")]
    struct PanickingInstallLifecycle {
        when_polled: bool,
    }

    #[cfg(feature = "client-lifecycle")]
    impl ClientLifecycle for PanickingInstallLifecycle {
        fn install(
            &self,
            _client: std::sync::Weak<Client>,
        ) -> wacore::runtime::BoxFuture<'_, anyhow::Result<()>> {
            if !self.when_polled {
                panic!("injected synchronous install panic");
            }
            Box::pin(async { panic!("injected asynchronous install panic") })
        }
    }

    #[tokio::test]
    #[cfg(feature = "client-lifecycle")]
    async fn lifecycle_install_panics_are_typed_build_errors() {
        for when_polled in [false, true] {
            let result = complete_builder()
                .await
                .with_lifecycle(PanickingInstallLifecycle { when_polled })
                .build()
                .await;
            assert!(matches!(
                result,
                Err(ClientBuilderError::LifecycleInstall(_))
            ));
        }
    }

    #[tokio::test]
    #[cfg(feature = "client-lifecycle")]
    async fn lifecycle_install_failure_publishes_nothing_and_starts_no_tasks() {
        let persistence_manager = Arc::new(
            PersistenceManager::new(crate::test_utils::create_test_backend().await)
                .await
                .expect("persistence manager"),
        );
        let spawns = Arc::new(AtomicUsize::new(0));
        let lifecycle = Arc::new(FailingLifecycle {
            spawns: Arc::clone(&spawns),
            installed_client: std::sync::Mutex::new(None),
        });

        let result = ClientBuilder::new()
            .with_runtime(CountingRuntime {
                spawns: Arc::clone(&spawns),
            })
            .with_persistence_manager(persistence_manager)
            .with_transport_factory(MockTransportFactory::new())
            .with_http_client(MockHttpClient)
            .with_lifecycle_arc(lifecycle.clone())
            .build()
            .await;

        assert!(matches!(
            result,
            Err(ClientBuilderError::LifecycleInstall(_))
        ));
        assert_eq!(spawns.load(Ordering::SeqCst), 0);
        assert!(
            lifecycle
                .installed_client
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .as_ref()
                .is_some_and(|client| client.upgrade().is_none())
        );
    }
}

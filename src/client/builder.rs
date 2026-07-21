use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use thiserror::Error;

use super::{Client, ClientLifecycle, LifecycleRegistration};
use crate::cache_config::CacheConfig;
use crate::http::HttpClient;
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
/// The sync-task receiver has a single consumer and is therefore transferred
/// together with the client rather than hidden behind a cloneable handle.
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

    /// Return the constructed client.
    pub fn client(&self) -> Arc<Client> {
        Arc::clone(&self.client)
    }

    /// Transfer ownership of the client and its sync-task receiver.
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
    #[error("the configured backend does not support the inbound durability hook: {0}")]
    UnsupportedDurabilityBackend(String),
    #[error("client lifecycle installation failed: {0}")]
    LifecycleInstall(#[source] anyhow::Error),
    #[error("plugin host installation failed: {0}")]
    PluginInstall(#[source] anyhow::Error),
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
    lifecycle: Option<Arc<dyn ClientLifecycle>>,
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
            lifecycle: None,
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
    pub fn with_lifecycle<L>(mut self, lifecycle: L) -> Self
    where
        L: ClientLifecycle + 'static,
    {
        self.lifecycle = Some(Arc::new(lifecycle));
        self
    }

    /// Install an already-shared aggregate lifecycle.
    pub fn with_lifecycle_arc(mut self, lifecycle: Arc<dyn ClientLifecycle>) -> Self {
        self.lifecycle = Some(lifecycle);
        self
    }

    /// Register a native plugin for transactional installation before services start.
    pub fn with_plugin<P: ClientPlugin>(mut self, plugin: P) -> Self {
        self.plugins.push(PluginRegistration::new(plugin));
        self
    }

    /// Register an already-shared native plugin without changing its marker type.
    pub fn with_plugin_arc<P: ClientPlugin>(mut self, plugin: Arc<P>) -> Self {
        self.plugins.push(PluginRegistration::new_arc(plugin));
        self
    }

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
        let plugin_plan = PluginPlan::prepare(self.plugins)?;
        let runtime: Arc<dyn Runtime> = match self.task_instrument {
            Some(instrument) => {
                Arc::new(wacore::stats::InstrumentedRuntime::new(runtime, instrument))
            }
            None => runtime,
        };

        let mut lifecycle_handler = self.lifecycle;
        let plugin_host = plugin_plan.map(|plan| {
            let host = PluginHost::new(plan, lifecycle_handler.take());
            lifecycle_handler = Some(host.clone());
            host
        });
        let lifecycle = lifecycle_handler
            .map(|handler| Arc::new(LifecycleRegistration::new(handler, Arc::clone(&runtime))));
        let assembly = Client::assemble(
            Arc::clone(&runtime),
            Arc::clone(&persistence_manager),
            transport_factory,
            http_client,
            self.override_version,
            self.cache_config,
            ClientExtensions {
                lifecycle,
                plugin_host,
            },
        );
        let client = assembly.client();

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
        if let Some(lifecycle) = &client.lifecycle
            && let Err(error) = lifecycle.install(Arc::downgrade(&client)).await
        {
            return Err(if client.plugin_host.is_some() {
                ClientBuilderError::PluginInstall(error)
            } else {
                ClientBuilderError::LifecycleInstall(error)
            });
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
        if let Some(plugin_host) = &client.plugin_host {
            plugin_host.activate();
        }
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

#[derive(Default)]
pub(super) struct ClientExtensions {
    pub(super) lifecycle: Option<Arc<LifecycleRegistration>>,
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

    struct FailingLifecycle {
        spawns: Arc<AtomicUsize>,
        installed_client: std::sync::Mutex<Option<std::sync::Weak<Client>>>,
    }

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
        build.client().signal_shutdown_sync();
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
        let client = build.client();

        assert!(client.skip_history_sync_enabled());
        assert_eq!(client.wanted_pre_key_count(), 123);
        assert!(
            client
                .alloc_meter
                .get()
                .is_some_and(|installed| Arc::ptr_eq(installed, &meter))
        );
        assert!(client.saver_handle.get().is_some());
        assert_eq!(spawns.load(Ordering::SeqCst), 3);
        client.signal_shutdown_sync();
    }

    #[tokio::test]
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

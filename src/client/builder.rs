use std::sync::Arc;

use thiserror::Error;

use super::Client;
use crate::cache_config::CacheConfig;
use crate::http::HttpClient;
use crate::store::persistence_manager::PersistenceManager;
use crate::sync_task::MajorSyncTask;
use crate::transport::TransportFactory;
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
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
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

    /// Validate dependencies, assemble an inert client, then start its services.
    pub async fn build(self) -> Result<ClientBuild, ClientBuilderError> {
        self.build_boxed().await
    }

    #[inline(never)]
    fn build_boxed(
        self,
    ) -> wacore::runtime::BoxFuture<'static, Result<ClientBuild, ClientBuilderError>> {
        Box::pin(async move {
            let runtime = self.runtime.ok_or(ClientBuilderError::MissingRuntime)?;
            let persistence_manager = self
                .persistence_manager
                .ok_or(ClientBuilderError::MissingPersistenceManager)?;
            let transport_factory = self
                .transport_factory
                .ok_or(ClientBuilderError::MissingTransportFactory)?;
            let http_client = self
                .http_client
                .ok_or(ClientBuilderError::MissingHttpClient)?;

            Ok(Self::build_required(
                runtime,
                persistence_manager,
                transport_factory,
                http_client,
                self.override_version,
                self.cache_config,
            ))
        })
    }

    pub(crate) fn build_required(
        runtime: Arc<dyn Runtime>,
        persistence_manager: Arc<PersistenceManager>,
        transport_factory: Arc<dyn TransportFactory>,
        http_client: Arc<dyn HttpClient>,
        override_version: Option<(u32, u32, u32)>,
        cache_config: CacheConfig,
    ) -> ClientBuild {
        Client::assemble(
            runtime,
            persistence_manager,
            transport_factory,
            http_client,
            override_version,
            cache_config,
        )
        .start()
    }
}

/// Owns the only path from a fully allocated client to started background
/// services, preventing callers from publishing a partially configured client.
pub(super) struct ClientAssembly {
    client: Arc<Client>,
    sync_task_receiver: async_channel::Receiver<MajorSyncTask>,
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
        );
        assert_eq!(spawns.load(Ordering::SeqCst), 0);

        let build = assembly.start();
        assert_eq!(spawns.load(Ordering::SeqCst), 2);
        build.client().signal_shutdown_sync();
    }
}

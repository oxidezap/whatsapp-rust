//! Public construction contract: no private client fields or SQLite required.
use std::sync::Arc;
use std::time::Duration;

use whatsapp_rust::bot::Bot;
use whatsapp_rust::http::{HttpClient, HttpRequest, HttpResponse};
use whatsapp_rust::store::persistence_manager::PersistenceManager;
use whatsapp_rust::transport::{Transport, TransportEvent, TransportFactory};
use whatsapp_rust::wacore::store::in_memory::InMemoryBackend;
use whatsapp_rust::{Client, ClientBuilderError, ClientOptions, PresencePolicy, TokioRuntime};

struct HostHttp;
#[async_trait::async_trait]
impl HttpClient for HostHttp {
    async fn execute(&self, _: HttpRequest) -> anyhow::Result<HttpResponse> {
        anyhow::bail!("offline construction must not use HTTP")
    }
}

struct HostTransport;
#[async_trait::async_trait]
impl TransportFactory for HostTransport {
    async fn create_transport(
        &self,
    ) -> anyhow::Result<(Arc<dyn Transport>, async_channel::Receiver<TransportEvent>)> {
        anyhow::bail!("offline construction must not connect")
    }
}

async fn backend() -> Arc<dyn whatsapp_rust::store::traits::Backend> {
    Arc::new(InMemoryBackend::new())
}

#[test]
fn shared_defaults_and_facade_saver_policy() {
    let low = Client::builder();
    let bot = Bot::builder();
    let a = low.options();
    let b = bot.client_options();
    assert_eq!(a.override_version, b.override_version);
    assert_eq!(a.skip_history_sync, b.skip_history_sync);
    assert!(!a.skip_history_sync);
    assert_eq!(a.ab_props_fetch, b.ab_props_fetch);
    assert!(a.ab_props_fetch);
    assert!(a.watched_ab_props.is_empty() && b.watched_ab_props.is_empty());
    assert_eq!(a.presence_policy, b.presence_policy);
    assert_eq!(a.noise_cert_policy, b.noise_cert_policy);
    assert_eq!(a.wanted_pre_key_count, b.wanted_pre_key_count);
    assert_eq!(a.resend_rate_limit, b.resend_rate_limit);
    assert_eq!(a.background_saver_interval, None);
    assert_eq!(b.background_saver_interval, Some(Duration::from_secs(30)));
    // The cache snapshot is not a public equality contract; selected shared TTL
    // and capacities are covered here, runtime installation in module tests.
    assert_eq!(
        a.cache_config.group_cache.timeout,
        b.cache_config.group_cache.timeout
    );
    assert_eq!(
        a.cache_config.group_cache.capacity,
        b.cache_config.group_cache.capacity
    );
}

#[tokio::test]
async fn options_and_shared_dependencies_reach_both_facades() {
    let mut options = ClientOptions::default();
    options.skip_history_sync = true;
    options.ab_props_fetch = false;
    options.presence_policy = PresencePolicy::Manual;
    options.wanted_pre_key_count = Some(123);
    options.override_version = Some((2, 3000, 1));
    let runtime: Arc<dyn whatsapp_rust::Runtime> = Arc::new(TokioRuntime);
    let http: Arc<dyn HttpClient> = Arc::new(HostHttp);
    let transport: Arc<dyn TransportFactory> = Arc::new(HostTransport);
    let pm = Arc::new(PersistenceManager::new(backend().await).await.unwrap());
    let (client, receiver) = Client::builder()
        .with_options(options.clone())
        .with_runtime_arc(runtime.clone())
        .with_http_client_arc(http.clone())
        .with_transport_factory_arc(transport.clone())
        .with_persistence_manager(pm)
        .build()
        .await
        .unwrap()
        .into_parts();
    let bot = Bot::builder()
        .with_client_options(options)
        .with_backend_arc(backend().await)
        .with_runtime_arc(runtime)
        .with_http_client_arc(http)
        .with_transport_factory_arc(transport)
        .build()
        .await
        .unwrap();
    for built in [client.clone(), bot.client()] {
        assert!(built.skip_history_sync_enabled());
        assert!(!built.ab_props_fetch_enabled());
        assert_eq!(built.presence_policy(), PresencePolicy::Manual);
        assert_eq!(built.wanted_pre_key_count(), 123);
        built.shutdown().await;
    }
    assert_eq!(receiver.receiver_count(), 1);
}

#[tokio::test]
async fn concrete_implementations_and_dynamic_errors() {
    assert!(matches!(
        Client::builder().build().await,
        Err(ClientBuilderError::MissingRuntime)
    ));
    let pm = Arc::new(PersistenceManager::new(backend().await).await.unwrap());
    let client = Client::builder()
        .with_runtime(TokioRuntime)
        .with_http_client(HostHttp)
        .with_transport_factory(HostTransport)
        .with_persistence_manager(pm)
        .build()
        .await
        .unwrap()
        .into_client();
    client.shutdown().await;
    let bot = Bot::builder()
        .with_backend(InMemoryBackend::new())
        .with_runtime(TokioRuntime)
        .with_http_client(HostHttp)
        .with_transport_factory(HostTransport)
        .build()
        .await
        .unwrap();
    bot.client().shutdown().await;
}

#[test]
fn option_replacement_and_setters_are_last_writer_wins() {
    let low = Client::builder()
        .with_skip_history_sync(true)
        .with_options(ClientOptions::default());
    let bot = Bot::builder()
        .skip_history_sync()
        .with_client_options(ClientOptions::default());
    assert!(!low.options().skip_history_sync);
    assert!(!bot.client_options().skip_history_sync);
    assert_eq!(bot.client_options().background_saver_interval, None);
    assert!(
        Bot::builder()
            .with_client_options(ClientOptions::default())
            .skip_history_sync()
            .client_options()
            .skip_history_sync
    );
}

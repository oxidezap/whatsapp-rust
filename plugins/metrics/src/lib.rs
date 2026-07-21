//! Reference out-of-core plugin exercising typed APIs, scoped tasks, and custom events.
//!
//! ```ignore
//! let client = Client::builder()
//!     // platform dependencies...
//!     .with_plugin(MetricsPlugin::default())
//!     .build()
//!     .await?
//!     .into_client();
//! let metrics = client
//!     .plugin::<MetricsPlugin>()
//!     .expect("metrics plugin was registered");
//! let events = client
//!     .plugin_event_router()
//!     .expect("metrics publishes custom events")
//!     .subscribe(
//!         [metrics.tick_selector()],
//!         PluginEventEndpointConfig::new(64, PluginEventOverflow::DropOldest),
//!     )?;
//! ```

use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use anyhow::{Context, Result, ensure};
use portable_atomic::{AtomicBool, AtomicU64, Ordering};
use serde::{Deserialize, Serialize};
use whatsapp_rust::wacore::types::events::{Event, EventHandler, EventInterest, EventKind};
use whatsapp_rust::{
    ClientPlugin, PluginCapability, PluginConnectionScope, PluginEventPayloadEncoding,
    PluginEventSelector, PluginEventTopic, PluginEvents, PluginFuture, PluginManifest, PluginTasks,
};

pub const METRICS_PLUGIN_ID: &str = "wa.metrics";
pub const METRICS_TICK_TOPIC: &str = "tick";
pub const METRICS_TICK_SCHEMA_VERSION: u32 = 1;

/// Stable snapshot exposed by [`MetricsApi`] and encoded in every tick event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, bon::Builder)]
#[non_exhaustive]
pub struct MetricsSnapshot {
    pub core_events: u64,
    pub messages: u64,
    pub receipts: u64,
    pub connected: u64,
    pub disconnected: u64,
    pub install_ticks: u64,
    pub connection_ticks: u64,
    pub ready_scopes: u64,
    pub closed_scopes: u64,
    pub events_enqueued: u64,
    pub events_dropped: u64,
    pub publish_failures: u64,
    pub active_generation: Option<u64>,
    pub last_closed_generation: Option<u64>,
    pub shutdown: bool,
}

#[derive(Default)]
struct MetricsState {
    core_events: AtomicU64,
    messages: AtomicU64,
    receipts: AtomicU64,
    connected: AtomicU64,
    disconnected: AtomicU64,
    install_ticks: AtomicU64,
    connection_ticks: AtomicU64,
    ready_scopes: AtomicU64,
    closed_scopes: AtomicU64,
    events_enqueued: AtomicU64,
    events_dropped: AtomicU64,
    publish_failures: AtomicU64,
    active_generation: Mutex<Option<u64>>,
    last_closed_generation: Mutex<Option<u64>>,
    shutdown: AtomicBool,
}

impl MetricsState {
    fn snapshot(&self) -> MetricsSnapshot {
        let active_generation = *self
            .active_generation
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let last_closed_generation = *self
            .last_closed_generation
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        MetricsSnapshot::builder()
            .core_events(self.core_events.load(Ordering::Relaxed))
            .messages(self.messages.load(Ordering::Relaxed))
            .receipts(self.receipts.load(Ordering::Relaxed))
            .connected(self.connected.load(Ordering::Relaxed))
            .disconnected(self.disconnected.load(Ordering::Relaxed))
            .install_ticks(self.install_ticks.load(Ordering::Relaxed))
            .connection_ticks(self.connection_ticks.load(Ordering::Relaxed))
            .ready_scopes(self.ready_scopes.load(Ordering::Relaxed))
            .closed_scopes(self.closed_scopes.load(Ordering::Relaxed))
            .events_enqueued(self.events_enqueued.load(Ordering::Relaxed))
            .events_dropped(self.events_dropped.load(Ordering::Relaxed))
            .publish_failures(self.publish_failures.load(Ordering::Relaxed))
            .maybe_active_generation(active_generation)
            .maybe_last_closed_generation(last_closed_generation)
            .shutdown(self.shutdown.load(Ordering::Acquire))
            .build()
    }

    fn open_scope(&self, generation: u64) {
        self.ready_scopes.fetch_add(1, Ordering::Relaxed);
        *self
            .active_generation
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(generation);
    }

    fn close_scope(&self, generation: u64) {
        let mut active = self
            .active_generation
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if *active == Some(generation) {
            *active = None;
        }
    }

    fn record_closed(&self, generation: u64) {
        self.closed_scopes.fetch_add(1, Ordering::Relaxed);
        *self
            .last_closed_generation
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(generation);
        self.close_scope(generation);
    }
}

struct MetricsEventHandler(Arc<MetricsState>);

impl EventHandler for MetricsEventHandler {
    fn handle_event(&self, event: Arc<Event>) {
        self.0.core_events.fetch_add(1, Ordering::Relaxed);
        match event.kind() {
            EventKind::Messages => {
                self.0.messages.fetch_add(1, Ordering::Relaxed);
            }
            EventKind::Receipt => {
                self.0.receipts.fetch_add(1, Ordering::Relaxed);
            }
            EventKind::Connected => {
                self.0.connected.fetch_add(1, Ordering::Relaxed);
            }
            EventKind::Disconnected => {
                self.0.disconnected.fetch_add(1, Ordering::Relaxed);
            }
            _ => {}
        }
    }
}

/// Type-safe API returned by `client.plugin::<MetricsPlugin>()`.
pub struct MetricsApi {
    state: Arc<MetricsState>,
    tick_selector: PluginEventSelector,
}

impl MetricsApi {
    pub fn snapshot(&self) -> MetricsSnapshot {
        self.state.snapshot()
    }

    pub fn tick_selector(&self) -> PluginEventSelector {
        self.tick_selector.clone()
    }
}

/// One metrics-plugin installation. Construct a fresh value for each client.
pub struct MetricsPlugin {
    interval: Duration,
    state: OnceLock<Arc<MetricsState>>,
}

impl MetricsPlugin {
    pub const fn new(interval: Duration) -> Self {
        Self {
            interval,
            state: OnceLock::new(),
        }
    }

    fn state(&self) -> Result<Arc<MetricsState>> {
        self.state
            .get()
            .cloned()
            .context("metrics plugin is not installed")
    }
}

impl Default for MetricsPlugin {
    fn default() -> Self {
        Self::new(Duration::from_secs(10))
    }
}

impl ClientPlugin for MetricsPlugin {
    type Api = MetricsApi;

    fn manifest(&self) -> PluginManifest {
        PluginManifest::new(METRICS_PLUGIN_ID, env!("CARGO_PKG_VERSION"))
            .with_capability(PluginCapability::CoreEvents)
            .with_capability(PluginCapability::Tasks)
            .with_capability(PluginCapability::PluginEvents)
    }

    fn install(
        &self,
        context: whatsapp_rust::PluginContext,
    ) -> PluginFuture<'_, Result<Arc<Self::Api>>> {
        Box::pin(async move {
            ensure!(self.interval != Duration::ZERO, "metrics interval is zero");
            let core_events = context
                .core_events()
                .cloned()
                .context("core-events capability is missing")?;
            let tasks = context
                .tasks()
                .cloned()
                .context("tasks capability is missing")?;
            let plugin_events = context
                .plugin_events()
                .cloned()
                .context("plugin-events capability is missing")?;
            let tick = PluginEventTopic::new(METRICS_TICK_TOPIC)?;
            let state = Arc::new(MetricsState::default());
            self.state
                .set(state.clone())
                .map_err(|_| anyhow::anyhow!("metrics plugin was installed more than once"))?;

            core_events.subscribe(
                EventInterest::of(&[
                    EventKind::Messages,
                    EventKind::Receipt,
                    EventKind::Connected,
                    EventKind::Disconnected,
                ]),
                Arc::new(MetricsEventHandler(state.clone())),
            )?;

            let api = Arc::new(MetricsApi {
                state: state.clone(),
                tick_selector: plugin_events.selector(&tick),
            });
            spawn_install_ticker(tasks, plugin_events, tick, state, self.interval)?;
            Ok(api)
        })
    }

    fn on_ready(&self, scope: PluginConnectionScope) -> PluginFuture<'_, Result<()>> {
        Box::pin(async move {
            let state = self.state()?;
            let generation = scope.generation();
            let tasks = scope
                .tasks()
                .cloned()
                .context("connection tasks capability is missing")?;
            state.open_scope(generation);
            let worker_tasks = tasks.clone();
            let worker_state = state.clone();
            let interval = self.interval;
            let guard = ConnectionGuard { state, generation };
            tasks.spawn(async move {
                let _guard = guard;
                while worker_tasks.sleep(interval).await.is_ok() {
                    worker_state
                        .connection_ticks
                        .fetch_add(1, Ordering::Relaxed);
                }
            })?;
            Ok(())
        })
    }

    fn on_closed(&self, scope: PluginConnectionScope) -> PluginFuture<'_, Result<()>> {
        Box::pin(async move {
            self.state()?.record_closed(scope.generation());
            Ok(())
        })
    }

    fn shutdown(&self) -> PluginFuture<'_, Result<()>> {
        Box::pin(async move {
            if let Some(state) = self.state.get() {
                state.shutdown.store(true, Ordering::Release);
            }
            Ok(())
        })
    }
}

fn spawn_install_ticker(
    tasks: PluginTasks,
    plugin_events: PluginEvents,
    topic: PluginEventTopic,
    state: Arc<MetricsState>,
    interval: Duration,
) -> Result<()> {
    let worker_tasks = tasks.clone();
    tasks.spawn(async move {
        while worker_tasks.sleep(interval).await.is_ok() {
            state.install_ticks.fetch_add(1, Ordering::Relaxed);
            if !plugin_events.has_subscribers(&topic) {
                continue;
            }
            let Ok(payload) = serde_json::to_vec(&state.snapshot()) else {
                state.publish_failures.fetch_add(1, Ordering::Relaxed);
                continue;
            };
            match plugin_events.publish(
                &topic,
                METRICS_TICK_SCHEMA_VERSION,
                PluginEventPayloadEncoding::Json,
                payload,
            ) {
                Ok(report) => {
                    state
                        .events_enqueued
                        .fetch_add(report.enqueued, Ordering::Relaxed);
                    state
                        .events_dropped
                        .fetch_add(report.dropped, Ordering::Relaxed);
                }
                Err(_) => {
                    state.publish_failures.fetch_add(1, Ordering::Relaxed);
                }
            }
        }
    })?;
    Ok(())
}

struct ConnectionGuard {
    state: Arc<MetricsState>,
    generation: u64,
}

impl Drop for ConnectionGuard {
    fn drop(&mut self) {
        self.state.close_scope(self.generation);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use whatsapp_rust::async_channel::Receiver;
    use whatsapp_rust::bytes::Bytes;
    use whatsapp_rust::http::{HttpClient, HttpRequest, HttpResponse};
    use whatsapp_rust::store::persistence_manager::PersistenceManager;
    use whatsapp_rust::transport::{Transport, TransportEvent, TransportFactory};
    use whatsapp_rust::wacore::store::InMemoryBackend;
    use whatsapp_rust::{
        Client, PluginEventEndpointConfig, PluginEventOverflow, PluginEventSubscribeError,
        PluginHealth, TokioRuntime,
    };

    use super::*;

    #[test]
    fn retired_scope_cannot_clear_a_newer_generation() {
        let state = Arc::new(MetricsState::default());
        state.open_scope(4);
        let old_guard = ConnectionGuard {
            state: state.clone(),
            generation: 4,
        };
        state.open_scope(5);
        drop(old_guard);
        assert_eq!(state.snapshot().active_generation, Some(5));

        state.record_closed(4);
        let snapshot = state.snapshot();
        assert_eq!(snapshot.active_generation, Some(5));
        assert_eq!(snapshot.last_closed_generation, Some(4));
        state.record_closed(5);
        assert_eq!(state.snapshot().active_generation, None);
    }

    struct TestTransport;

    #[whatsapp_rust::async_trait]
    impl Transport for TestTransport {
        async fn send(&self, _data: Bytes) -> Result<()> {
            Ok(())
        }

        async fn disconnect(&self) {}
    }

    struct TestTransportFactory;

    #[whatsapp_rust::async_trait]
    impl TransportFactory for TestTransportFactory {
        async fn create_transport(&self) -> Result<(Arc<dyn Transport>, Receiver<TransportEvent>)> {
            let (_sender, receiver) = whatsapp_rust::async_channel::bounded(1);
            Ok((Arc::new(TestTransport), receiver))
        }
    }

    struct TestHttpClient;

    #[whatsapp_rust::async_trait]
    impl HttpClient for TestHttpClient {
        async fn execute(&self, _request: HttpRequest) -> Result<HttpResponse> {
            Ok(HttpResponse {
                status_code: 200,
                body: Vec::new(),
            })
        }
    }

    async fn test_client(interval: Duration) -> Arc<Client> {
        let backend = Arc::new(InMemoryBackend::new());
        let persistence = Arc::new(
            PersistenceManager::new(backend)
                .await
                .expect("persistence manager"),
        );
        Client::builder()
            .with_runtime(TokioRuntime)
            .with_persistence_manager(persistence)
            .with_transport_factory(TestTransportFactory)
            .with_http_client(TestHttpClient)
            .with_plugin(MetricsPlugin::new(interval))
            .build()
            .await
            .expect("metrics plugin client")
            .into_client()
    }

    #[tokio::test]
    async fn external_plugin_exposes_typed_api_and_bounded_events() {
        let client = test_client(Duration::from_millis(2)).await;
        let api = client.plugin::<MetricsPlugin>().expect("typed metrics API");
        let router = client.plugin_event_router().expect("plugin event router");
        let selector = api.tick_selector();
        let events = router
            .subscribe(
                [selector.clone()],
                PluginEventEndpointConfig::new(1, PluginEventOverflow::DropNewest),
            )
            .expect("metrics event endpoint");

        let event = tokio::time::timeout(Duration::from_secs(1), events.recv())
            .await
            .expect("metrics tick timeout")
            .expect("metrics tick");
        assert_eq!(&*event.plugin_id, METRICS_PLUGIN_ID);
        assert_eq!(event.topic.as_str(), METRICS_TICK_TOPIC);
        assert_eq!(event.schema_version, METRICS_TICK_SCHEMA_VERSION);
        assert_eq!(event.payload_encoding, PluginEventPayloadEncoding::Json);
        let payload: MetricsSnapshot =
            serde_json::from_slice(&event.payload).expect("typed metrics payload");
        assert!(payload.install_ticks > 0);

        tokio::time::timeout(Duration::from_secs(1), async {
            while events.stats().dropped == 0 {
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        })
        .await
        .expect("bounded endpoint reports pressure");
        assert!(api.snapshot().install_ticks >= payload.install_ticks);
        let stats = client.plugin_stats().expect("plugin host stats");
        let metrics = stats
            .plugins
            .iter()
            .find(|plugin| plugin.plugin_id == METRICS_PLUGIN_ID)
            .expect("metrics plugin stats");
        assert_eq!(metrics.health, PluginHealth::Degraded);
        assert!(metrics.events.expect("metrics event stats").dropped > 0);

        client.disconnect().await;
        assert!(api.snapshot().shutdown);
        assert!(matches!(
            router.subscribe(
                [selector],
                PluginEventEndpointConfig::new(1, PluginEventOverflow::DropNewest),
            ),
            Err(PluginEventSubscribeError::Closed)
        ));
    }
}

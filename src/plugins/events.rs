use std::collections::{HashMap, HashSet};
use std::fmt;
use std::sync::{Arc, Mutex, RwLock};

use async_channel::{Receiver, Sender, TryRecvError, TrySendError};
use bytes::Bytes;
use portable_atomic::{AtomicBool, AtomicU64, Ordering};
use thiserror::Error;

use super::{PluginResourceError, PluginResources, valid_plugin_id};

const MAX_ENDPOINT_CAPACITY: usize = 65_536;
const MAX_ENDPOINT_SELECTORS: usize = 1_024;

/// Encoding of a custom plugin event payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PluginEventPayloadEncoding {
    Json,
    Binary,
}

impl PluginEventPayloadEncoding {
    pub const fn identifier(self) -> &'static str {
        match self {
            Self::Json => "json",
            Self::Binary => "binary",
        }
    }
}

/// Validated second-level topic within one plugin namespace.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct PluginEventTopic(Arc<str>);

impl PluginEventTopic {
    pub fn new(topic: impl Into<String>) -> Result<Self, PluginEventRouteError> {
        let topic = topic.into();
        if !valid_topic(&topic) {
            return Err(PluginEventRouteError::InvalidTopic { topic });
        }
        Ok(Self(Arc::from(topic)))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for PluginEventTopic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("PluginEventTopic")
            .field(&self.0)
            .finish()
    }
}

impl fmt::Display for PluginEventTopic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Exact `(plugin_id, topic)` route selected by one endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PluginEventSelector {
    route: RouteKey,
}

impl PluginEventSelector {
    pub fn new(
        plugin_id: impl Into<String>,
        topic: PluginEventTopic,
    ) -> Result<Self, PluginEventRouteError> {
        let plugin_id = plugin_id.into();
        if !valid_plugin_id(&plugin_id) {
            return Err(PluginEventRouteError::InvalidPluginId { plugin_id });
        }
        Ok(Self {
            route: RouteKey {
                plugin_id: Arc::from(plugin_id),
                topic,
            },
        })
    }

    pub fn plugin_id(&self) -> &str {
        &self.route.plugin_id
    }

    pub fn topic(&self) -> &PluginEventTopic {
        &self.route.topic
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct RouteKey {
    plugin_id: Arc<str>,
    topic: PluginEventTopic,
}

/// Routed event shared by every matching endpoint without copying its payload.
#[derive(Debug, Clone, bon::Builder)]
#[non_exhaustive]
pub struct PluginEventEnvelope {
    pub plugin_id: Arc<str>,
    pub topic: PluginEventTopic,
    pub schema_version: u32,
    pub payload_encoding: PluginEventPayloadEncoding,
    pub payload: Bytes,
    pub connection_generation: u64,
    /// Monotonic sequence for this route while it has at least one subscriber.
    ///
    /// Dropped events consume a sequence number, allowing one endpoint to detect loss. The
    /// sequence resets after the last subscriber to the route is removed.
    pub sequence: u64,
}

/// Behavior when one endpoint cannot keep up with publishers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PluginEventOverflow {
    DropNewest,
    DropOldest,
}

/// Required queue policy for one independent consumer endpoint.
///
/// Capacity counts envelopes rather than bytes. Native plugins are trusted, and payloads are
/// shared across matching endpoints. A foreign adapter must enforce its wire payload limit before
/// publishing into this router.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginEventEndpointConfig {
    capacity: usize,
    overflow: PluginEventOverflow,
}

impl PluginEventEndpointConfig {
    pub const fn new(capacity: usize, overflow: PluginEventOverflow) -> Self {
        Self { capacity, overflow }
    }

    pub const fn capacity(self) -> usize {
        self.capacity
    }

    pub const fn overflow(self) -> PluginEventOverflow {
        self.overflow
    }
}

/// Syntactic route validation failure.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PluginEventRouteError {
    #[error("invalid plugin id `{plugin_id}`")]
    InvalidPluginId { plugin_id: String },
    #[error("invalid plugin event topic `{topic}`")]
    InvalidTopic { topic: String },
}

/// Endpoint registration failure.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum PluginEventSubscribeError {
    #[error(transparent)]
    Resource(#[from] PluginResourceError),
    #[error("at least one plugin event selector is required")]
    EmptySelectors,
    #[error("plugin event endpoint selector count exceeds the maximum of {max}")]
    TooManySelectors { max: usize },
    #[error("plugin event endpoint capacity {capacity} is outside 1..={max}")]
    InvalidCapacity { capacity: usize, max: usize },
    #[error("plugin `{plugin_id}` is not registered as a custom-event publisher")]
    UnknownPublisher { plugin_id: String },
    #[error("plugin event endpoint identifiers are exhausted")]
    EndpointIdsExhausted,
    #[error("the plugin event router is closed")]
    Closed,
}

/// Custom event publication failure.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PluginEventPublishError {
    #[error(transparent)]
    Resource(#[from] PluginResourceError),
    #[error("plugin event schema version must be greater than zero")]
    InvalidSchemaVersion,
    #[error("the plugin event router is closed")]
    Closed,
    #[error("the plugin event sequence is exhausted")]
    SequenceExhausted,
}

/// Result of one non-blocking fan-out attempt.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct PluginEventPublishReport {
    pub matched: u64,
    pub enqueued: u64,
    pub dropped: u64,
    pub closed: u64,
}

/// Cumulative state for one endpoint queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct PluginEventEndpointStats {
    pub enqueued: u64,
    pub delivered: u64,
    pub dropped: u64,
    pub queue_depth: usize,
    pub capacity: usize,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
#[error("the plugin event endpoint is closed")]
pub struct PluginEventReceiveError;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PluginEventTryReceiveError {
    #[error("the plugin event endpoint queue is empty")]
    Empty,
    #[error("the plugin event endpoint is closed")]
    Closed,
}

enum EnqueueOutcome {
    Enqueued,
    Dropped,
    EnqueuedAfterDrop,
    Closed,
}

struct EventEndpoint {
    id: u64,
    sender: Sender<Arc<PluginEventEnvelope>>,
    overflow: PluginEventOverflow,
    capacity: usize,
    enqueued: AtomicU64,
    delivered: AtomicU64,
    dropped: AtomicU64,
}

impl EventEndpoint {
    fn enqueue(&self, event: Arc<PluginEventEnvelope>) -> EnqueueOutcome {
        match self.overflow {
            PluginEventOverflow::DropNewest => match self.sender.try_send(event) {
                Ok(()) => {
                    self.enqueued.fetch_add(1, Ordering::Relaxed);
                    EnqueueOutcome::Enqueued
                }
                Err(TrySendError::Full(_)) => {
                    self.dropped.fetch_add(1, Ordering::Relaxed);
                    EnqueueOutcome::Dropped
                }
                Err(TrySendError::Closed(_)) => EnqueueOutcome::Closed,
            },
            PluginEventOverflow::DropOldest => match self.sender.force_send(event) {
                Ok(evicted) => {
                    self.enqueued.fetch_add(1, Ordering::Relaxed);
                    if evicted.is_some() {
                        self.dropped.fetch_add(1, Ordering::Relaxed);
                        EnqueueOutcome::EnqueuedAfterDrop
                    } else {
                        EnqueueOutcome::Enqueued
                    }
                }
                Err(_) => EnqueueOutcome::Closed,
            },
        }
    }

    fn close(&self) {
        self.sender.close();
    }

    fn stats(&self) -> PluginEventEndpointStats {
        PluginEventEndpointStats {
            enqueued: self.enqueued.load(Ordering::Relaxed),
            delivered: self.delivered.load(Ordering::Relaxed),
            dropped: self.dropped.load(Ordering::Relaxed),
            queue_depth: self.sender.len(),
            capacity: self.capacity,
        }
    }
}

struct RouteClock {
    sequence: Mutex<u64>,
}

struct RouteEntry {
    clock: Arc<RouteClock>,
    endpoints: Arc<[Arc<EventEndpoint>]>,
}

#[derive(Default)]
struct RouterState {
    routes: HashMap<RouteKey, RouteEntry>,
    endpoints: HashMap<u64, Arc<EventEndpoint>>,
}

struct PluginEventRouterInner {
    plugin_ids: HashSet<Arc<str>>,
    state: RwLock<RouterState>,
    next_endpoint_id: AtomicU64,
    closed: AtomicBool,
}

impl PluginEventRouterInner {
    fn unsubscribe(&self, endpoint_id: u64, selectors: &[PluginEventSelector]) {
        let endpoint = {
            let mut state = self
                .state
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            let endpoint = state.endpoints.remove(&endpoint_id);
            for selector in selectors {
                let remove_route = if let Some(route) = state.routes.get_mut(&selector.route) {
                    let remaining = route
                        .endpoints
                        .iter()
                        .filter(|endpoint| endpoint.id != endpoint_id)
                        .cloned()
                        .collect::<Vec<_>>();
                    route.endpoints = remaining.into();
                    route.endpoints.is_empty()
                } else {
                    false
                };
                if remove_route {
                    state.routes.remove(&selector.route);
                }
            }
            endpoint
        };
        if let Some(endpoint) = endpoint {
            endpoint.close();
        }
    }

    fn close(&self) {
        if self.closed.swap(true, Ordering::AcqRel) {
            return;
        }
        let endpoints = {
            let mut state = self
                .state
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            state.routes.clear();
            std::mem::take(&mut state.endpoints)
        };
        for endpoint in endpoints.into_values() {
            endpoint.close();
        }
    }
}

/// Read-only subscription boundary for native consumers and future foreign adapters.
///
/// Routes are exact `(plugin_id, topic)` matches. Closing the router prevents new publications and
/// subscriptions, while already queued envelopes remain available before receivers observe closure.
#[derive(Clone)]
pub struct PluginEventRouter {
    inner: Arc<PluginEventRouterInner>,
}

impl PluginEventRouter {
    pub(super) fn new(plugin_ids: impl IntoIterator<Item = String>) -> Self {
        Self {
            inner: Arc::new(PluginEventRouterInner {
                plugin_ids: plugin_ids.into_iter().map(Arc::from).collect(),
                state: RwLock::new(RouterState::default()),
                next_endpoint_id: AtomicU64::new(1),
                closed: AtomicBool::new(false),
            }),
        }
    }

    pub fn has_subscribers(&self, selector: &PluginEventSelector) -> bool {
        if self.inner.closed.load(Ordering::Acquire) {
            return false;
        }
        self.inner
            .state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .routes
            .get(&selector.route)
            .is_some_and(|route| !route.endpoints.is_empty())
    }

    pub fn subscribe(
        &self,
        selectors: impl IntoIterator<Item = PluginEventSelector>,
        config: PluginEventEndpointConfig,
    ) -> Result<PluginEventSubscription, PluginEventSubscribeError> {
        if config.capacity == 0 || config.capacity > MAX_ENDPOINT_CAPACITY {
            return Err(PluginEventSubscribeError::InvalidCapacity {
                capacity: config.capacity,
                max: MAX_ENDPOINT_CAPACITY,
            });
        }
        if self.inner.closed.load(Ordering::Acquire) {
            return Err(PluginEventSubscribeError::Closed);
        }

        let mut seen = HashSet::new();
        let mut unique_selectors = Vec::new();
        for selector in selectors {
            if !seen.insert(selector.route.clone()) {
                continue;
            }
            if unique_selectors.len() == MAX_ENDPOINT_SELECTORS {
                return Err(PluginEventSubscribeError::TooManySelectors {
                    max: MAX_ENDPOINT_SELECTORS,
                });
            }
            unique_selectors.push(selector);
        }
        let selectors = unique_selectors;
        if selectors.is_empty() {
            return Err(PluginEventSubscribeError::EmptySelectors);
        }
        for selector in &selectors {
            if !self.inner.plugin_ids.contains(selector.plugin_id()) {
                return Err(PluginEventSubscribeError::UnknownPublisher {
                    plugin_id: selector.plugin_id().to_string(),
                });
            }
        }

        let endpoint_id = self
            .inner
            .next_endpoint_id
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .map_err(|_| PluginEventSubscribeError::EndpointIdsExhausted)?;
        let (sender, receiver) = async_channel::bounded(config.capacity);
        let endpoint = Arc::new(EventEndpoint {
            id: endpoint_id,
            sender,
            overflow: config.overflow,
            capacity: config.capacity,
            enqueued: AtomicU64::new(0),
            delivered: AtomicU64::new(0),
            dropped: AtomicU64::new(0),
        });

        {
            let mut state = self
                .inner
                .state
                .write()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if self.inner.closed.load(Ordering::Acquire) {
                return Err(PluginEventSubscribeError::Closed);
            }
            state.endpoints.insert(endpoint_id, endpoint.clone());
            for selector in &selectors {
                let route = state
                    .routes
                    .entry(selector.route.clone())
                    .or_insert_with(|| RouteEntry {
                        clock: Arc::new(RouteClock {
                            sequence: Mutex::new(0),
                        }),
                        endpoints: Arc::from([]),
                    });
                let endpoints = route
                    .endpoints
                    .iter()
                    .cloned()
                    .chain(std::iter::once(endpoint.clone()))
                    .collect::<Vec<_>>();
                route.endpoints = endpoints.into();
            }
        }

        Ok(PluginEventSubscription {
            router: self.clone(),
            endpoint,
            receiver,
            selectors,
        })
    }

    fn publish(
        &self,
        plugin_id: &Arc<str>,
        topic: &PluginEventTopic,
        schema_version: u32,
        payload_encoding: PluginEventPayloadEncoding,
        payload: Bytes,
        connection_generation: u64,
    ) -> Result<PluginEventPublishReport, PluginEventPublishError> {
        if schema_version == 0 {
            return Err(PluginEventPublishError::InvalidSchemaVersion);
        }
        if self.inner.closed.load(Ordering::Acquire) {
            return Err(PluginEventPublishError::Closed);
        }

        let route_key = RouteKey {
            plugin_id: plugin_id.clone(),
            topic: topic.clone(),
        };
        let Some((clock, endpoints)) = self
            .inner
            .state
            .read()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .routes
            .get(&route_key)
            .map(|route| (route.clock.clone(), route.endpoints.clone()))
        else {
            return Ok(PluginEventPublishReport::default());
        };

        let mut sequence = clock
            .sequence
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let next_sequence = sequence
            .checked_add(1)
            .ok_or(PluginEventPublishError::SequenceExhausted)?;
        *sequence = next_sequence;
        let event = Arc::new(
            PluginEventEnvelope::builder()
                .plugin_id(plugin_id.clone())
                .topic(topic.clone())
                .schema_version(schema_version)
                .payload_encoding(payload_encoding)
                .payload(payload)
                .connection_generation(connection_generation)
                .sequence(next_sequence)
                .build(),
        );

        let mut report = PluginEventPublishReport {
            matched: u64::try_from(endpoints.len()).unwrap_or(u64::MAX),
            ..PluginEventPublishReport::default()
        };
        for endpoint in endpoints.iter() {
            match endpoint.enqueue(event.clone()) {
                EnqueueOutcome::Enqueued => report.enqueued += 1,
                EnqueueOutcome::Dropped => report.dropped += 1,
                EnqueueOutcome::EnqueuedAfterDrop => {
                    report.enqueued += 1;
                    report.dropped += 1;
                }
                EnqueueOutcome::Closed => report.closed += 1,
            }
        }
        Ok(report)
    }

    pub(super) fn close(&self) {
        self.inner.close();
    }
}

/// One bounded endpoint. Dropping it unregisters every selected route atomically.
#[must_use = "dropping the subscription unregisters its plugin event routes"]
pub struct PluginEventSubscription {
    router: PluginEventRouter,
    endpoint: Arc<EventEndpoint>,
    receiver: Receiver<Arc<PluginEventEnvelope>>,
    selectors: Vec<PluginEventSelector>,
}

impl PluginEventSubscription {
    pub fn id(&self) -> u64 {
        self.endpoint.id
    }

    pub fn selectors(&self) -> &[PluginEventSelector] {
        &self.selectors
    }

    pub fn stats(&self) -> PluginEventEndpointStats {
        self.endpoint.stats()
    }

    pub async fn recv(&self) -> Result<Arc<PluginEventEnvelope>, PluginEventReceiveError> {
        let event = self
            .receiver
            .recv()
            .await
            .map_err(|_| PluginEventReceiveError)?;
        self.endpoint.delivered.fetch_add(1, Ordering::Relaxed);
        Ok(event)
    }

    pub fn try_recv(&self) -> Result<Arc<PluginEventEnvelope>, PluginEventTryReceiveError> {
        let event = self.receiver.try_recv().map_err(|error| match error {
            TryRecvError::Empty => PluginEventTryReceiveError::Empty,
            TryRecvError::Closed => PluginEventTryReceiveError::Closed,
        })?;
        self.endpoint.delivered.fetch_add(1, Ordering::Relaxed);
        Ok(event)
    }
}

impl Drop for PluginEventSubscription {
    fn drop(&mut self) {
        self.router
            .inner
            .unsubscribe(self.endpoint.id, &self.selectors);
    }
}

/// Context-bound custom event capability. A plugin can publish only under its own ID.
///
/// Consumers subscribe through [`PluginEventRouter`], keeping publication authority separate from
/// native or future foreign endpoints.
#[derive(Clone)]
pub struct PluginEvents {
    plugin_id: Arc<str>,
    router: PluginEventRouter,
    resources: Arc<PluginResources>,
    connection_generation: Arc<AtomicU64>,
}

impl PluginEvents {
    pub fn selector(&self, topic: &PluginEventTopic) -> PluginEventSelector {
        PluginEventSelector {
            route: RouteKey {
                plugin_id: self.plugin_id.clone(),
                topic: topic.clone(),
            },
        }
    }

    pub fn has_subscribers(&self, topic: &PluginEventTopic) -> bool {
        self.router.has_subscribers(&self.selector(topic))
    }

    pub fn publish(
        &self,
        topic: &PluginEventTopic,
        schema_version: u32,
        payload_encoding: PluginEventPayloadEncoding,
        payload: impl Into<Bytes>,
    ) -> Result<PluginEventPublishReport, PluginEventPublishError> {
        self.resources.ensure_active()?;
        self.router.publish(
            &self.plugin_id,
            topic,
            schema_version,
            payload_encoding,
            payload.into(),
            self.connection_generation.load(Ordering::Acquire),
        )
    }
}

pub(super) fn publisher(
    plugin_id: &str,
    router: PluginEventRouter,
    resources: Arc<PluginResources>,
    connection_generation: Arc<AtomicU64>,
) -> PluginEvents {
    PluginEvents {
        plugin_id: Arc::from(plugin_id),
        router,
        resources,
        connection_generation,
    }
}

fn valid_topic(topic: &str) -> bool {
    valid_plugin_id(topic)
}

#[cfg(test)]
mod tests {
    use std::thread;

    use super::*;

    fn topic(value: &str) -> PluginEventTopic {
        PluginEventTopic::new(value).expect("valid topic")
    }

    fn selector(plugin_id: &str, topic: &PluginEventTopic) -> PluginEventSelector {
        PluginEventSelector::new(plugin_id, topic.clone()).expect("valid selector")
    }

    fn publish(
        router: &PluginEventRouter,
        plugin_id: &str,
        topic: &PluginEventTopic,
        value: u32,
    ) -> PluginEventPublishReport {
        router
            .publish(
                &Arc::from(plugin_id),
                topic,
                1,
                PluginEventPayloadEncoding::Binary,
                Bytes::copy_from_slice(&value.to_be_bytes()),
                7,
            )
            .expect("event publication")
    }

    #[test]
    fn routes_only_exact_plugin_and_topic_matches() {
        let router = PluginEventRouter::new(["metrics".to_string(), "audit".to_string()]);
        let tick = topic("tick");
        let other = topic("other");
        let subscription = router
            .subscribe(
                [selector("metrics", &tick)],
                PluginEventEndpointConfig::new(4, PluginEventOverflow::DropNewest),
            )
            .expect("subscription");

        assert_eq!(publish(&router, "metrics", &other, 1).matched, 0);
        assert_eq!(publish(&router, "audit", &tick, 2).matched, 0);
        assert_eq!(publish(&router, "metrics", &tick, 3).enqueued, 1);
        let event = subscription.try_recv().expect("routed event");
        assert_eq!(&*event.plugin_id, "metrics");
        assert_eq!(event.topic, tick);
        assert_eq!(event.connection_generation, 7);
        assert_eq!(event.sequence, 1);
        assert_eq!(event.payload, Bytes::copy_from_slice(&3u32.to_be_bytes()));
    }

    #[test]
    fn drop_newest_preserves_the_queued_prefix_and_counts_loss() {
        let router = PluginEventRouter::new(["metrics".to_string()]);
        let tick = topic("tick");
        let subscription = router
            .subscribe(
                [selector("metrics", &tick)],
                PluginEventEndpointConfig::new(2, PluginEventOverflow::DropNewest),
            )
            .expect("subscription");

        for value in 1..=4 {
            publish(&router, "metrics", &tick, value);
        }

        assert_eq!(subscription.try_recv().expect("first").sequence, 1);
        assert_eq!(subscription.try_recv().expect("second").sequence, 2);
        assert!(matches!(
            subscription.try_recv(),
            Err(PluginEventTryReceiveError::Empty)
        ));
        assert_eq!(
            subscription.stats(),
            PluginEventEndpointStats {
                enqueued: 2,
                delivered: 2,
                dropped: 2,
                queue_depth: 0,
                capacity: 2,
            }
        );
    }

    #[test]
    fn drop_oldest_preserves_the_latest_events_and_counts_evictions() {
        let router = PluginEventRouter::new(["metrics".to_string()]);
        let tick = topic("tick");
        let subscription = router
            .subscribe(
                [selector("metrics", &tick)],
                PluginEventEndpointConfig::new(2, PluginEventOverflow::DropOldest),
            )
            .expect("subscription");

        for value in 1..=4 {
            publish(&router, "metrics", &tick, value);
        }

        assert_eq!(subscription.try_recv().expect("third").sequence, 3);
        assert_eq!(subscription.try_recv().expect("fourth").sequence, 4);
        assert_eq!(subscription.stats().enqueued, 4);
        assert_eq!(subscription.stats().dropped, 2);
    }

    #[test]
    fn backpressure_is_isolated_per_endpoint() {
        let router = PluginEventRouter::new(["metrics".to_string()]);
        let tick = topic("tick");
        let slow = router
            .subscribe(
                [selector("metrics", &tick)],
                PluginEventEndpointConfig::new(1, PluginEventOverflow::DropNewest),
            )
            .expect("slow endpoint");
        let fast = router
            .subscribe(
                [selector("metrics", &tick)],
                PluginEventEndpointConfig::new(4, PluginEventOverflow::DropNewest),
            )
            .expect("fast endpoint");

        publish(&router, "metrics", &tick, 1);
        publish(&router, "metrics", &tick, 2);

        assert_eq!(slow.stats().dropped, 1);
        assert_eq!(fast.stats().dropped, 0);
        assert_eq!(fast.try_recv().expect("fast first").sequence, 1);
        assert_eq!(fast.try_recv().expect("fast second").sequence, 2);
    }

    #[tokio::test]
    async fn drop_unregisters_and_router_close_wakes_receivers() {
        let router = PluginEventRouter::new(["metrics".to_string()]);
        let tick = topic("tick");
        let selector = selector("metrics", &tick);
        let subscription = router
            .subscribe(
                [selector.clone()],
                PluginEventEndpointConfig::new(1, PluginEventOverflow::DropNewest),
            )
            .expect("subscription");
        assert!(router.has_subscribers(&selector));
        drop(subscription);
        assert!(!router.has_subscribers(&selector));
        assert_eq!(publish(&router, "metrics", &tick, 1).matched, 0);

        let subscription = router
            .subscribe(
                [selector],
                PluginEventEndpointConfig::new(1, PluginEventOverflow::DropNewest),
            )
            .expect("second subscription");
        assert_eq!(publish(&router, "metrics", &tick, 2).enqueued, 1);
        router.close();
        assert_eq!(subscription.recv().await.expect("queued event").sequence, 1);
        assert!(matches!(
            subscription.recv().await,
            Err(PluginEventReceiveError)
        ));
    }

    #[test]
    fn rejects_invalid_or_unknown_endpoint_configuration() {
        assert!(PluginEventTopic::new("Invalid").is_err());
        let tick = topic("tick");
        assert!(PluginEventSelector::new("Invalid", tick.clone()).is_err());
        let router = PluginEventRouter::new(["metrics".to_string()]);
        assert!(matches!(
            router.subscribe(
                [selector("unknown", &tick)],
                PluginEventEndpointConfig::new(1, PluginEventOverflow::DropNewest),
            ),
            Err(PluginEventSubscribeError::UnknownPublisher { .. })
        ));
        assert!(matches!(
            router.subscribe(
                [selector("metrics", &tick)],
                PluginEventEndpointConfig::new(0, PluginEventOverflow::DropNewest),
            ),
            Err(PluginEventSubscribeError::InvalidCapacity { .. })
        ));
        assert!(matches!(
            router.subscribe(
                [],
                PluginEventEndpointConfig::new(1, PluginEventOverflow::DropNewest),
            ),
            Err(PluginEventSubscribeError::EmptySelectors)
        ));
        let too_many = (0..=MAX_ENDPOINT_SELECTORS)
            .map(|index| selector("metrics", &topic(&format!("topic-{index}"))))
            .collect::<Vec<_>>();
        assert!(matches!(
            router.subscribe(
                too_many,
                PluginEventEndpointConfig::new(1, PluginEventOverflow::DropNewest),
            ),
            Err(PluginEventSubscribeError::TooManySelectors { .. })
        ));
    }

    #[test]
    fn concurrent_publish_keeps_route_sequences_in_queue_order() {
        const THREADS: usize = 8;
        const EVENTS_PER_THREAD: usize = 100;
        let router = PluginEventRouter::new(["metrics".to_string()]);
        let tick = topic("tick");
        let subscription = router
            .subscribe(
                [selector("metrics", &tick)],
                PluginEventEndpointConfig::new(
                    THREADS * EVENTS_PER_THREAD,
                    PluginEventOverflow::DropNewest,
                ),
            )
            .expect("subscription");

        let threads = (0..THREADS)
            .map(|_| {
                let router = router.clone();
                let tick = tick.clone();
                thread::spawn(move || {
                    for value in 0..EVENTS_PER_THREAD {
                        publish(&router, "metrics", &tick, value as u32);
                    }
                })
            })
            .collect::<Vec<_>>();
        for thread in threads {
            thread.join().expect("publisher thread");
        }

        for sequence in 1..=(THREADS * EVENTS_PER_THREAD) as u64 {
            assert_eq!(
                subscription.try_recv().expect("ordered event").sequence,
                sequence
            );
        }
        assert_eq!(subscription.stats().dropped, 0);
    }
}

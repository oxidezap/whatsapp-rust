//! Build-time client plugins and their capability-scoped host.

mod events;

pub use events::{
    PluginEventEndpointConfig, PluginEventEndpointStats, PluginEventEnvelope, PluginEventOverflow,
    PluginEventPayloadEncoding, PluginEventPublishError, PluginEventPublishReport,
    PluginEventReceiveError, PluginEventRouteError, PluginEventRouter, PluginEventSelector,
    PluginEventSubscribeError, PluginEventSubscription, PluginEventTopic,
    PluginEventTryReceiveError, PluginEvents,
};

use std::any::{Any, TypeId};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::future::Future;
use std::panic::AssertUnwindSafe;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak};
use std::time::Duration;

use futures::FutureExt;
use portable_atomic::AtomicU64;
use thiserror::Error;
use wacore::iq::spec::IqSpec;
use wacore::runtime::{
    BoxFuture, Runtime, ShutdownNotifier, ShutdownSignal, Spawnable, timeout as runtime_timeout,
    wait_for_shutdown,
};
use wacore::sync_marker::MaybeSendSync;
use wacore::types::events::{EventHandler, EventInterest, EventKind, Subscription};
use wacore_binary::Jid;
use waproto::whatsapp::Message;

use crate::Client;
use crate::client::{ClientLifecycle, ConnectionScope, ConnectionScopeState, RawNodeLease};
use crate::request::IqError;
use crate::send::{SendError, SendResult};

const CAP_CORE_EVENTS: u8 = 1 << 0;
const CAP_TASKS: u8 = 1 << 1;
const CAP_MESSAGING: u8 = 1 << 2;
const CAP_IQ: u8 = 1 << 3;
const CAP_PLUGIN_EVENTS: u8 = 1 << 4;
const PLUGIN_CALLBACK_TIMEOUT: Duration = Duration::from_secs(5);

/// A capability a plugin asks the host to expose during installation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PluginCapability {
    CoreEvents,
    Tasks,
    Messaging,
    Iq,
    PluginEvents,
}

impl PluginCapability {
    pub const fn identifier(self) -> &'static str {
        match self {
            Self::CoreEvents => "events.core.observe",
            Self::Tasks => "tasks.spawn",
            Self::Messaging => "messaging.send",
            Self::Iq => "iq.execute",
            Self::PluginEvents => "events.plugin.publish",
        }
    }

    const fn bit(self) -> u8 {
        match self {
            Self::CoreEvents => CAP_CORE_EVENTS,
            Self::Tasks => CAP_TASKS,
            Self::Messaging => CAP_MESSAGING,
            Self::Iq => CAP_IQ,
            Self::PluginEvents => CAP_PLUGIN_EVENTS,
        }
    }
}

/// Compact set of capabilities requested by one plugin.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PluginCapabilities(u8);

impl PluginCapabilities {
    pub const NONE: Self = Self(0);

    pub const fn with(self, capability: PluginCapability) -> Self {
        Self(self.0 | capability.bit())
    }

    pub const fn contains(self, capability: PluginCapability) -> bool {
        self.0 & capability.bit() != 0
    }
}

/// Build-time declaration used for validation, ordering, and future foreign adapters.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct PluginManifest {
    id: String,
    version: String,
    dependencies: Vec<String>,
    capabilities: PluginCapabilities,
}

impl PluginManifest {
    pub fn new(id: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            version: version.into(),
            dependencies: Vec::new(),
            capabilities: PluginCapabilities::NONE,
        }
    }

    pub fn with_dependency(mut self, plugin_id: impl Into<String>) -> Self {
        self.dependencies.push(plugin_id.into());
        self
    }

    pub const fn with_capability(mut self, capability: PluginCapability) -> Self {
        self.capabilities = self.capabilities.with(capability);
        self
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn dependencies(&self) -> &[String] {
        &self.dependencies
    }

    pub const fn capabilities(&self) -> PluginCapabilities {
        self.capabilities
    }
}

/// Target-correct future returned by native plugin entry points.
pub type PluginFuture<'a, T> = BoxFuture<'a, T>;

/// A trusted native plugin installed exactly once while the client is still inert.
/// Capabilities shape the handles it receives; they are not an in-process sandbox.
/// A plugin value belongs to one client installation, even when registered through an `Arc`.
pub trait ClientPlugin: MaybeSendSync + 'static {
    type Api: MaybeSendSync + 'static;

    fn manifest(&self) -> PluginManifest;

    fn install(&self, context: PluginContext) -> PluginFuture<'_, anyhow::Result<Arc<Self::Api>>>;

    fn on_ready(&self, _scope: PluginConnectionScope) -> PluginFuture<'_, anyhow::Result<()>> {
        Box::pin(async { Ok(()) })
    }

    fn on_closed(&self, _scope: PluginConnectionScope) -> PluginFuture<'_, anyhow::Result<()>> {
        Box::pin(async { Ok(()) })
    }

    /// Release plugin-owned state. This may run after `install` began but returned an error.
    fn shutdown(&self) -> PluginFuture<'_, anyhow::Result<()>> {
        Box::pin(async { Ok(()) })
    }
}

/// Manifest validation or dependency-ordering failure.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PluginPlanError {
    #[error("plugin {plugin_type} panicked while producing its manifest")]
    ManifestPanicked { plugin_type: &'static str },
    #[error("invalid plugin id `{id}`")]
    InvalidId { id: String },
    #[error("plugin `{plugin_id}` has an invalid version `{version}`")]
    InvalidVersion { plugin_id: String, version: String },
    #[error("plugin id `{id}` is registered more than once")]
    DuplicateId { id: String },
    #[error("plugin marker type `{plugin_type}` is registered more than once")]
    DuplicateType { plugin_type: &'static str },
    #[error("plugin `{plugin_id}` lists dependency `{dependency}` more than once")]
    DuplicateDependency {
        plugin_id: String,
        dependency: String,
    },
    #[error("plugin `{plugin_id}` requires missing plugin `{dependency}`")]
    MissingDependency {
        plugin_id: String,
        dependency: String,
    },
    #[error("plugin dependency cycle involves: {plugins:?}")]
    DependencyCycle { plugins: Vec<String> },
}

/// Capability use after the client or plugin scope has ended.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PluginResourceError {
    #[error("the client is no longer available")]
    ClientUnavailable,
    #[error("the plugin host has not started yet")]
    NotActive,
    #[error("the plugin scope is shutting down")]
    ShuttingDown,
    #[error("the plugin task capacity is exhausted")]
    TaskCapacityExceeded,
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PluginMessagingError {
    #[error(transparent)]
    Resource(#[from] PluginResourceError),
    #[error(transparent)]
    Send(#[from] SendError),
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PluginIqError {
    #[error(transparent)]
    Resource(#[from] PluginResourceError),
    #[error(transparent)]
    Iq(#[from] IqError),
}

struct PluginResources {
    active: AtomicBool,
    closed: AtomicBool,
    activation: ShutdownNotifier,
    shutdown: ShutdownNotifier,
    install_tasks: Arc<TaskTracker>,
    connection_tasks: Mutex<ConnectionTaskRegistry>,
    subscriptions: Mutex<Vec<PluginCoreEventSubscription>>,
}

#[derive(Default)]
struct ConnectionTaskRegistry {
    closed: bool,
    trackers: HashMap<u64, Arc<TaskTracker>>,
}

#[derive(Default)]
struct TaskTrackerState {
    active: usize,
    closed: bool,
}

struct TaskTracker {
    state: Mutex<TaskTrackerState>,
    idle: ShutdownNotifier,
}

impl TaskTracker {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            state: Mutex::new(TaskTrackerState::default()),
            idle: ShutdownNotifier::new(),
        })
    }

    fn closed() -> Arc<Self> {
        let tracker = Self::new();
        tracker.close();
        tracker
    }

    fn register(self: &Arc<Self>) -> Result<TaskLease, PluginResourceError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.closed {
            return Err(PluginResourceError::ShuttingDown);
        }
        state.active = state
            .active
            .checked_add(1)
            .ok_or(PluginResourceError::TaskCapacityExceeded)?;
        Ok(TaskLease {
            tracker: Arc::clone(self),
        })
    }

    fn close(&self) {
        let idle = {
            let mut state = self
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            state.closed = true;
            state.active == 0
        };
        if idle {
            self.idle.notify();
        }
    }

    fn completion_signal(&self) -> ShutdownSignal {
        self.idle.subscribe()
    }
}

struct TaskLease {
    tracker: Arc<TaskTracker>,
}

impl Drop for TaskLease {
    fn drop(&mut self) {
        let idle = {
            let mut state = self
                .tracker
                .state
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            state.active = state.active.saturating_sub(1);
            state.closed && state.active == 0
        };
        if idle {
            self.tracker.idle.notify();
        }
    }
}

struct PluginCoreEventSubscription {
    _subscription: Subscription,
    _raw_node_lease: Option<RawNodeLease>,
}

impl PluginResources {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            active: AtomicBool::new(false),
            closed: AtomicBool::new(false),
            activation: ShutdownNotifier::new(),
            shutdown: ShutdownNotifier::new(),
            install_tasks: TaskTracker::new(),
            connection_tasks: Mutex::new(ConnectionTaskRegistry::default()),
            subscriptions: Mutex::new(Vec::new()),
        })
    }

    fn activate(&self) {
        if self.closed.load(Ordering::Acquire) {
            return;
        }
        self.active.store(true, Ordering::Release);
        self.activation.notify();
    }

    fn ensure_active(&self) -> Result<(), PluginResourceError> {
        if self.closed.load(Ordering::Acquire) {
            Err(PluginResourceError::ShuttingDown)
        } else if !self.active.load(Ordering::Acquire) {
            Err(PluginResourceError::NotActive)
        } else {
            Ok(())
        }
    }

    fn retain_subscription(
        &self,
        subscription: Subscription,
        raw_node_lease: Option<RawNodeLease>,
    ) -> Result<(), PluginResourceError> {
        let registration = PluginCoreEventSubscription {
            _subscription: subscription,
            _raw_node_lease: raw_node_lease,
        };
        let rejected = {
            let mut subscriptions = self
                .subscriptions
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if self.closed.load(Ordering::Acquire) {
                Some(registration)
            } else {
                subscriptions.push(registration);
                None
            }
        };
        if let Some(rejected) = rejected {
            drop(rejected);
            Err(PluginResourceError::ShuttingDown)
        } else {
            Ok(())
        }
    }

    fn connection_task_tracker(&self, generation: u64) -> Arc<TaskTracker> {
        let mut registry = self
            .connection_tasks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if registry.closed {
            return TaskTracker::closed();
        }
        Arc::clone(
            registry
                .trackers
                .entry(generation)
                .or_insert_with(TaskTracker::new),
        )
    }

    fn close_connection_tasks(&self, generation: u64) -> Arc<TaskTracker> {
        let tracker = Arc::clone(
            self.connection_tasks
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .trackers
                .entry(generation)
                .or_insert_with(TaskTracker::closed),
        );
        tracker.close();
        tracker
    }

    fn forget_connection_tasks(&self, generation: u64, tracker: &Arc<TaskTracker>) {
        let mut registry = self
            .connection_tasks
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if registry
            .trackers
            .get(&generation)
            .is_some_and(|current| Arc::ptr_eq(current, tracker))
        {
            registry.trackers.remove(&generation);
        }
    }

    fn task_completion_signals(&self) -> Vec<ShutdownSignal> {
        let mut signals = vec![self.install_tasks.completion_signal()];
        signals.extend(
            self.connection_tasks
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .trackers
                .values()
                .map(|tracker| tracker.completion_signal()),
        );
        signals
    }

    fn close(&self) {
        if self.closed.swap(true, Ordering::AcqRel) {
            return;
        }
        self.install_tasks.close();
        let connection_trackers = {
            let mut registry = self
                .connection_tasks
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            registry.closed = true;
            registry.trackers.values().cloned().collect::<Vec<_>>()
        };
        for tracker in connection_trackers {
            tracker.close();
        }
        self.shutdown.notify();
        let subscriptions = {
            let mut subscriptions = self
                .subscriptions
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            std::mem::take(&mut *subscriptions)
        };
        for subscription in subscriptions {
            if std::panic::catch_unwind(AssertUnwindSafe(|| drop(subscription))).is_err() {
                log::warn!("Plugin core-event subscription panicked while being dropped");
            }
        }
    }
}

fn close_plugin_resources(plugin_id: &str, resources: &PluginResources) {
    if std::panic::catch_unwind(AssertUnwindSafe(|| resources.close())).is_err() {
        log::warn!("Plugin `{plugin_id}` resource closure panicked");
    }
}

/// Install-scoped task capability. Work starts after the complete plugin set is published and
/// stops during rollback or shutdown.
#[derive(Clone)]
pub struct PluginTasks {
    runtime: Arc<dyn Runtime>,
    resources: Arc<PluginResources>,
}

impl PluginTasks {
    pub fn spawn<F>(&self, future: F) -> Result<(), PluginResourceError>
    where
        F: Future<Output = ()> + Spawnable,
    {
        if self.resources.closed.load(Ordering::Acquire) {
            return Err(PluginResourceError::ShuttingDown);
        }
        let lease = self.resources.install_tasks.register()?;
        spawn_after_activation(&self.runtime, Arc::clone(&self.resources), lease, future);
        Ok(())
    }

    pub fn shutdown_signal(&self) -> ShutdownSignal {
        self.resources.shutdown.subscribe()
    }

    /// Sleep through the configured runtime, returning promptly when this plugin shuts down.
    pub async fn sleep(&self, duration: Duration) -> Result<(), PluginResourceError> {
        self.resources.ensure_active()?;
        let shutdown = self.resources.shutdown.subscribe();
        let cancelled = Box::pin(wait_for_shutdown(&shutdown));
        match futures::future::select(cancelled, self.runtime.sleep(duration)).await {
            futures::future::Either::Left(_) => Err(PluginResourceError::ShuttingDown),
            futures::future::Either::Right(_) => self.resources.ensure_active(),
        }
    }
}

/// Selective subscription access to the sealed core event bus.
/// Handlers run inline and must hand slow work to a task capability.
#[derive(Clone)]
pub struct PluginCoreEvents {
    client: Weak<Client>,
    resources: Arc<PluginResources>,
}

impl PluginCoreEvents {
    pub fn subscribe(
        &self,
        interest: EventInterest,
        handler: Arc<dyn EventHandler>,
    ) -> Result<(), PluginResourceError> {
        let client = self
            .client
            .upgrade()
            .ok_or(PluginResourceError::ClientUnavailable)?;
        let raw_node_lease = interest
            .wants(EventKind::RawNode)
            .then(|| client.acquire_raw_node_forwarding());
        let subscription = client.subscribe(interest, handler);
        self.resources
            .retain_subscription(subscription, raw_node_lease)
    }
}

/// High-level message sending without exposing the raw client or backend.
#[derive(Clone)]
pub struct PluginMessaging {
    client: Weak<Client>,
    resources: Arc<PluginResources>,
}

impl PluginMessaging {
    pub async fn send_message(
        &self,
        to: Jid,
        message: Message,
    ) -> Result<SendResult, PluginMessagingError> {
        self.resources.ensure_active()?;
        let client = self
            .client
            .upgrade()
            .ok_or(PluginResourceError::ClientUnavailable)?;
        Ok(client.send_message(to, message).await?)
    }

    pub async fn send_text(
        &self,
        to: Jid,
        text: String,
    ) -> Result<SendResult, PluginMessagingError> {
        self.resources.ensure_active()?;
        let client = self
            .client
            .upgrade()
            .ok_or(PluginResourceError::ClientUnavailable)?;
        Ok(client.send_text(to, text).await?)
    }
}

/// Typed IQ execution without exposing the raw client or stores.
#[derive(Clone)]
pub struct PluginIq {
    client: Weak<Client>,
    resources: Arc<PluginResources>,
}

impl PluginIq {
    pub async fn execute<S>(&self, spec: S) -> Result<S::Response, PluginIqError>
    where
        S: IqSpec,
    {
        self.resources.ensure_active()?;
        let client = self
            .client
            .upgrade()
            .ok_or(PluginResourceError::ClientUnavailable)?;
        Ok(client.execute(spec).await?)
    }
}

/// Capabilities and already-installed dependencies visible during installation.
pub struct PluginContext {
    plugin_id: String,
    dependencies: HashMap<TypeId, WeakErasedApi>,
    core_events: Option<PluginCoreEvents>,
    tasks: Option<PluginTasks>,
    messaging: Option<PluginMessaging>,
    iq: Option<PluginIq>,
    plugin_events: Option<PluginEvents>,
}

impl PluginContext {
    pub fn plugin_id(&self) -> &str {
        &self.plugin_id
    }

    /// Return a declared dependency without making retained contexts own it.
    /// Clone the returned API during installation if it must outlive this call.
    pub fn plugin<P: ClientPlugin>(&self) -> Option<Arc<P::Api>> {
        let api = self.dependencies.get(&TypeId::of::<P>())?.upgrade()?;
        downcast_api::<P::Api>(&api)
    }

    pub fn core_events(&self) -> Option<&PluginCoreEvents> {
        self.core_events.as_ref()
    }

    pub fn tasks(&self) -> Option<&PluginTasks> {
        self.tasks.as_ref()
    }

    pub fn messaging(&self) -> Option<&PluginMessaging> {
        self.messaging.as_ref()
    }

    pub fn iq(&self) -> Option<&PluginIq> {
        self.iq.as_ref()
    }

    pub fn plugin_events(&self) -> Option<&PluginEvents> {
        self.plugin_events.as_ref()
    }
}

/// One connection generation plus its optional connection-scoped task capability.
#[derive(Clone)]
pub struct PluginConnectionScope {
    scope: ConnectionScope,
    tasks: Option<PluginConnectionTasks>,
}

impl PluginConnectionScope {
    pub fn generation(&self) -> u64 {
        self.scope.generation()
    }

    pub fn state(&self) -> ConnectionScopeState {
        self.scope.state()
    }

    pub fn is_cancelled(&self) -> bool {
        self.scope.is_cancelled()
    }

    pub fn cancellation_signal(&self) -> ShutdownSignal {
        self.scope.cancellation_signal()
    }

    pub fn tasks(&self) -> Option<&PluginConnectionTasks> {
        self.tasks.as_ref()
    }
}

/// Task capability whose work is aborted synchronously when its generation retires.
#[derive(Clone)]
pub struct PluginConnectionTasks {
    runtime: Arc<dyn Runtime>,
    scope: ConnectionScope,
    tracker: Arc<TaskTracker>,
}

impl PluginConnectionTasks {
    pub fn spawn<F>(&self, future: F) -> Result<(), PluginResourceError>
    where
        F: Future<Output = ()> + Spawnable,
    {
        if self.scope.is_cancelled() {
            return Err(PluginResourceError::ShuttingDown);
        }
        let lease = self.tracker.register()?;
        spawn_until_cancelled(
            &self.runtime,
            self.scope.cancellation_signal(),
            lease,
            future,
        );
        Ok(())
    }

    /// Sleep through the configured runtime, returning promptly when this generation retires.
    pub async fn sleep(&self, duration: Duration) -> Result<(), PluginResourceError> {
        if self.scope.is_cancelled() {
            return Err(PluginResourceError::ShuttingDown);
        }
        let cancellation = self.scope.cancellation_signal();
        let cancelled = Box::pin(wait_for_shutdown(&cancellation));
        match futures::future::select(cancelled, self.runtime.sleep(duration)).await {
            futures::future::Either::Left(_) => Err(PluginResourceError::ShuttingDown),
            futures::future::Either::Right(_) if self.scope.is_cancelled() => {
                Err(PluginResourceError::ShuttingDown)
            }
            futures::future::Either::Right(_) => Ok(()),
        }
    }
}

fn spawn_until_cancelled<F>(
    runtime: &Arc<dyn Runtime>,
    cancellation: ShutdownSignal,
    lease: TaskLease,
    future: F,
) where
    F: Future<Output = ()> + Spawnable,
{
    runtime
        .spawn(Box::pin(async move {
            let _lease = lease;
            let cancelled = Box::pin(wait_for_shutdown(&cancellation));
            let work = Box::pin(future);
            let _ = futures::future::select(cancelled, work).await;
        }))
        .detach();
}

fn spawn_after_activation<F>(
    runtime: &Arc<dyn Runtime>,
    resources: Arc<PluginResources>,
    lease: TaskLease,
    future: F,
) where
    F: Future<Output = ()> + Spawnable,
{
    let activation = resources.activation.subscribe();
    let cancellation = resources.shutdown.subscribe();
    runtime
        .spawn(Box::pin(async move {
            let _lease = lease;
            let cancelled = Box::pin(wait_for_shutdown(&cancellation));
            let activated = Box::pin(wait_for_shutdown(&activation));
            if matches!(
                futures::future::select(cancelled, activated).await,
                futures::future::Either::Left(_)
            ) {
                return;
            }
            if resources.closed.load(Ordering::Acquire) {
                return;
            }
            let cancelled = Box::pin(wait_for_shutdown(&cancellation));
            let work = Box::pin(future);
            let _ = futures::future::select(cancelled, work).await;
        }))
        .detach();
}

trait ErasedApiValue: MaybeSendSync {
    fn as_any(&self) -> &dyn Any;
}

struct TypedApi<T>(Arc<T>);

impl<T: MaybeSendSync + 'static> ErasedApiValue for TypedApi<T> {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

type ErasedApi = Arc<dyn ErasedApiValue>;
type WeakErasedApi = Weak<dyn ErasedApiValue>;

#[derive(Default)]
struct ApiRegistry {
    values: Mutex<HashMap<TypeId, ErasedApi>>,
}

impl ApiRegistry {
    fn insert(&self, marker: TypeId, api: ErasedApi) {
        self.values
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(marker, api);
    }

    fn snapshot(&self) -> HashMap<TypeId, ErasedApi> {
        self.values
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn dependency_view(&self, markers: &[TypeId]) -> HashMap<TypeId, WeakErasedApi> {
        let values = self
            .values
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        markers
            .iter()
            .filter_map(|marker| values.get(marker).map(|api| (*marker, Arc::downgrade(api))))
            .collect()
    }
}

fn downcast_api<T: MaybeSendSync + 'static>(api: &ErasedApi) -> Option<Arc<T>> {
    api.as_any()
        .downcast_ref::<TypedApi<T>>()
        .map(|typed| typed.0.clone())
}

trait ErasedClientPlugin: MaybeSendSync {
    fn marker_type_id(&self) -> TypeId;
    fn marker_type_name(&self) -> &'static str;
    fn manifest(&self) -> PluginManifest;
    fn install(&self, context: PluginContext) -> BoxFuture<'_, anyhow::Result<ErasedApi>>;
    fn on_ready(&self, scope: PluginConnectionScope) -> BoxFuture<'_, anyhow::Result<()>>;
    fn on_closed(&self, scope: PluginConnectionScope) -> BoxFuture<'_, anyhow::Result<()>>;
    fn shutdown(&self) -> BoxFuture<'_, anyhow::Result<()>>;
}

struct PluginAdapter<P>(Arc<P>);

impl<P: ClientPlugin> ErasedClientPlugin for PluginAdapter<P> {
    fn marker_type_id(&self) -> TypeId {
        TypeId::of::<P>()
    }

    fn marker_type_name(&self) -> &'static str {
        std::any::type_name::<P>()
    }

    fn manifest(&self) -> PluginManifest {
        self.0.manifest()
    }

    fn install(&self, context: PluginContext) -> BoxFuture<'_, anyhow::Result<ErasedApi>> {
        Box::pin(async move {
            let api = self.0.install(context).await?;
            Ok(Arc::new(TypedApi(api)) as ErasedApi)
        })
    }

    fn on_ready(&self, scope: PluginConnectionScope) -> BoxFuture<'_, anyhow::Result<()>> {
        self.0.on_ready(scope)
    }

    fn on_closed(&self, scope: PluginConnectionScope) -> BoxFuture<'_, anyhow::Result<()>> {
        self.0.on_closed(scope)
    }

    fn shutdown(&self) -> BoxFuture<'_, anyhow::Result<()>> {
        self.0.shutdown()
    }
}

pub(crate) struct PluginRegistration {
    plugin: Arc<dyn ErasedClientPlugin>,
}

impl PluginRegistration {
    pub(crate) fn new<P: ClientPlugin>(plugin: P) -> Self {
        Self::new_arc(Arc::new(plugin))
    }

    pub(crate) fn new_arc<P: ClientPlugin>(plugin: Arc<P>) -> Self {
        Self {
            plugin: Arc::new(PluginAdapter(plugin)),
        }
    }
}

struct PlannedPlugin {
    plugin: Arc<dyn ErasedClientPlugin>,
    manifest: PluginManifest,
    dependency_markers: Vec<TypeId>,
}

pub(crate) struct PluginPlan {
    ordered: Vec<PlannedPlugin>,
}

impl PluginPlan {
    pub(crate) fn prepare(
        registrations: Vec<PluginRegistration>,
    ) -> Result<Option<Self>, PluginPlanError> {
        if registrations.is_empty() {
            return Ok(None);
        }

        let mut plugins = Vec::with_capacity(registrations.len());
        let mut ids = HashMap::with_capacity(registrations.len());
        let mut marker_types = HashSet::with_capacity(registrations.len());

        for registration in registrations {
            let plugin = registration.plugin;
            let marker = plugin.marker_type_id();
            if !marker_types.insert(marker) {
                return Err(PluginPlanError::DuplicateType {
                    plugin_type: plugin.marker_type_name(),
                });
            }
            let manifest = std::panic::catch_unwind(AssertUnwindSafe(|| plugin.manifest()))
                .map_err(|_| PluginPlanError::ManifestPanicked {
                    plugin_type: plugin.marker_type_name(),
                })?;
            validate_manifest(&manifest)?;
            let index = plugins.len();
            if ids.insert(manifest.id.clone(), index).is_some() {
                return Err(PluginPlanError::DuplicateId {
                    id: manifest.id.clone(),
                });
            }
            plugins.push(PlannedPlugin {
                plugin,
                manifest,
                dependency_markers: Vec::new(),
            });
        }

        let mut indegree = vec![0usize; plugins.len()];
        let mut dependents = vec![Vec::new(); plugins.len()];
        let mut dependency_markers = vec![Vec::new(); plugins.len()];
        for (plugin_index, planned) in plugins.iter().enumerate() {
            let mut seen = HashSet::with_capacity(planned.manifest.dependencies.len());
            for dependency in &planned.manifest.dependencies {
                if !seen.insert(dependency) {
                    return Err(PluginPlanError::DuplicateDependency {
                        plugin_id: planned.manifest.id.clone(),
                        dependency: dependency.clone(),
                    });
                }
                let Some(&dependency_index) = ids.get(dependency) else {
                    return Err(PluginPlanError::MissingDependency {
                        plugin_id: planned.manifest.id.clone(),
                        dependency: dependency.clone(),
                    });
                };
                indegree[plugin_index] += 1;
                dependents[dependency_index].push(plugin_index);
                dependency_markers[plugin_index]
                    .push(plugins[dependency_index].plugin.marker_type_id());
            }
        }
        for (planned, markers) in plugins.iter_mut().zip(dependency_markers) {
            planned.dependency_markers = markers;
        }

        let mut ready = indegree
            .iter()
            .enumerate()
            .filter_map(|(index, count)| (*count == 0).then_some(index))
            .collect::<BTreeSet<_>>();
        let mut order = Vec::with_capacity(plugins.len());
        while let Some(index) = ready.pop_first() {
            order.push(index);
            for &dependent in &dependents[index] {
                indegree[dependent] -= 1;
                if indegree[dependent] == 0 {
                    ready.insert(dependent);
                }
            }
        }

        if order.len() != plugins.len() {
            let cycle = indegree
                .iter()
                .enumerate()
                .filter(|(_, count)| **count > 0)
                .map(|(index, _)| plugins[index].manifest.id.clone())
                .collect();
            return Err(PluginPlanError::DependencyCycle { plugins: cycle });
        }

        let mut slots = plugins.into_iter().map(Some).collect::<Vec<_>>();
        let ordered = order
            .into_iter()
            .filter_map(|index| slots[index].take())
            .collect();
        Ok(Some(Self { ordered }))
    }
}

fn validate_manifest(manifest: &PluginManifest) -> Result<(), PluginPlanError> {
    if !valid_plugin_id(&manifest.id) {
        return Err(PluginPlanError::InvalidId {
            id: manifest.id.clone(),
        });
    }
    if manifest.version.is_empty()
        || manifest.version.len() > 64
        || !manifest.version.bytes().all(|byte| byte.is_ascii_graphic())
    {
        return Err(PluginPlanError::InvalidVersion {
            plugin_id: manifest.id.clone(),
            version: manifest.version.clone(),
        });
    }
    Ok(())
}

fn valid_plugin_id(id: &str) -> bool {
    if id.is_empty() || id.len() > 128 {
        return false;
    }
    let mut previous_separator = true;
    for byte in id.bytes() {
        let separator = matches!(byte, b'.' | b'-' | b'_');
        if separator {
            if previous_separator {
                return false;
            }
        } else if !byte.is_ascii_lowercase() && !byte.is_ascii_digit() {
            return false;
        }
        previous_separator = separator;
    }
    !previous_separator && id.as_bytes()[0].is_ascii_lowercase()
}

struct InstalledPlugin {
    plugin: Arc<dyn ErasedClientPlugin>,
    manifest: PluginManifest,
    resources: Arc<PluginResources>,
}

struct PluginInstallRollback {
    runtime: Arc<dyn Runtime>,
    installed: Vec<InstalledPlugin>,
    current: Option<InstalledPlugin>,
    upstream: Option<Arc<dyn ClientLifecycle>>,
    staged_apis: Option<Arc<ApiRegistry>>,
    armed: bool,
}

impl PluginInstallRollback {
    fn new(runtime: Arc<dyn Runtime>, capacity: usize) -> Self {
        Self {
            runtime,
            installed: Vec::with_capacity(capacity),
            current: None,
            upstream: None,
            staged_apis: None,
            armed: true,
        }
    }

    fn close_resources(&self) {
        if let Some(current) = &self.current {
            close_plugin_resources(&current.manifest.id, &current.resources);
        }
        for plugin in self.installed.iter().rev() {
            close_plugin_resources(&plugin.manifest.id, &plugin.resources);
        }
    }

    fn schedule_rollback(&mut self) -> Option<ShutdownSignal> {
        if !self.armed {
            return None;
        }
        self.close_resources();
        let current = self.current.take();
        let installed = std::mem::take(&mut self.installed);
        let upstream = self.upstream.take();
        let staged_apis = self.staged_apis.take();
        self.armed = false;
        if current.is_none() && installed.is_empty() && upstream.is_none() {
            return None;
        }
        if let Some(upstream) = &upstream
            && std::panic::catch_unwind(AssertUnwindSafe(|| upstream.signal_shutdown())).is_err()
        {
            log::warn!("Upstream lifecycle rollback shutdown signal panicked");
        }

        let completed = ShutdownNotifier::new();
        let completion = completed.subscribe();
        let runtime = self.runtime.clone();
        let cleanup_runtime = runtime.clone();
        runtime
            .spawn(Box::pin(async move {
                let result = AssertUnwindSafe(shutdown_staged_plugins(
                    cleanup_runtime,
                    current,
                    installed,
                    upstream,
                    staged_apis,
                ))
                .catch_unwind()
                .await;
                completed.notify();
                if result.is_err() {
                    log::warn!("Plugin installation rollback panicked");
                }
            }))
            .detach();
        Some(completion)
    }

    async fn rollback(&mut self) {
        if let Some(completion) = self.schedule_rollback() {
            wait_for_shutdown(&completion).await;
        }
    }

    fn take_installed(&mut self) -> Vec<InstalledPlugin> {
        std::mem::take(&mut self.installed)
    }

    fn restore_installed(&mut self, installed: Vec<InstalledPlugin>) {
        self.installed = installed;
    }

    fn disarm(&mut self) {
        self.armed = false;
        self.upstream = None;
        self.staged_apis = None;
    }
}

impl Drop for PluginInstallRollback {
    fn drop(&mut self) {
        let _ = self.schedule_rollback();
    }
}

pub(crate) struct PluginHost {
    ordered: Vec<PlannedPlugin>,
    manifests: Vec<PluginManifest>,
    upstream: Option<Arc<dyn ClientLifecycle>>,
    installed: OnceLock<Vec<InstalledPlugin>>,
    apis: OnceLock<HashMap<TypeId, ErasedApi>>,
    runtime: OnceLock<Arc<dyn Runtime>>,
    event_router: Option<PluginEventRouter>,
    callback_timeout: Duration,
    terminal: AtomicBool,
    terminal_notifier: ShutdownNotifier,
    installing_resources: Mutex<Vec<Weak<PluginResources>>>,
}

impl PluginHost {
    pub(crate) fn new(plan: PluginPlan, upstream: Option<Arc<dyn ClientLifecycle>>) -> Arc<Self> {
        Self::new_with_callback_timeout(plan, upstream, PLUGIN_CALLBACK_TIMEOUT)
    }

    fn new_with_callback_timeout(
        plan: PluginPlan,
        upstream: Option<Arc<dyn ClientLifecycle>>,
        callback_timeout: Duration,
    ) -> Arc<Self> {
        let manifests = plan
            .ordered
            .iter()
            .map(|plugin| plugin.manifest.clone())
            .collect::<Vec<_>>();
        let event_publishers = manifests
            .iter()
            .filter(|manifest| {
                manifest
                    .capabilities
                    .contains(PluginCapability::PluginEvents)
            })
            .map(|manifest| manifest.id.clone())
            .collect::<Vec<_>>();
        let event_router =
            (!event_publishers.is_empty()).then(|| PluginEventRouter::new(event_publishers));
        Arc::new(Self {
            ordered: plan.ordered,
            manifests,
            upstream,
            installed: OnceLock::new(),
            apis: OnceLock::new(),
            runtime: OnceLock::new(),
            event_router,
            callback_timeout,
            terminal: AtomicBool::new(false),
            terminal_notifier: ShutdownNotifier::new(),
            installing_resources: Mutex::new(Vec::new()),
        })
    }

    pub(crate) fn plugin<P: ClientPlugin>(&self) -> Option<Arc<P::Api>> {
        downcast_api::<P::Api>(self.apis.get()?.get(&TypeId::of::<P>())?)
    }

    pub(crate) fn manifests(&self) -> &[PluginManifest] {
        &self.manifests
    }

    pub(crate) fn lifecycle_callback_timeout(&self) -> Duration {
        let callback_count = self.ordered.len() + usize::from(self.upstream.is_some());
        let task_barrier_count = self
            .ordered
            .iter()
            .filter(|plugin| {
                plugin
                    .manifest
                    .capabilities
                    .contains(PluginCapability::Tasks)
            })
            .count();
        self.callback_timeout
            .saturating_mul((callback_count + task_barrier_count) as u32)
            .saturating_add(Duration::from_secs(1))
    }

    fn context(
        &self,
        client: &Weak<Client>,
        planned: &PlannedPlugin,
        resources: Arc<PluginResources>,
        apis: Arc<ApiRegistry>,
        runtime: Arc<dyn Runtime>,
        connection_generation: Arc<AtomicU64>,
    ) -> PluginContext {
        let manifest = &planned.manifest;
        let capabilities = manifest.capabilities;
        PluginContext {
            plugin_id: manifest.id.clone(),
            dependencies: apis.dependency_view(&planned.dependency_markers),
            core_events: capabilities
                .contains(PluginCapability::CoreEvents)
                .then(|| PluginCoreEvents {
                    client: client.clone(),
                    resources: Arc::clone(&resources),
                }),
            tasks: capabilities
                .contains(PluginCapability::Tasks)
                .then(|| PluginTasks {
                    runtime: Arc::clone(&runtime),
                    resources: Arc::clone(&resources),
                }),
            messaging: capabilities.contains(PluginCapability::Messaging).then(|| {
                PluginMessaging {
                    client: client.clone(),
                    resources: Arc::clone(&resources),
                }
            }),
            iq: capabilities
                .contains(PluginCapability::Iq)
                .then(|| PluginIq {
                    client: client.clone(),
                    resources: Arc::clone(&resources),
                }),
            plugin_events: self
                .event_router
                .as_ref()
                .filter(|_| capabilities.contains(PluginCapability::PluginEvents))
                .map(|router| {
                    events::publisher(
                        &manifest.id,
                        router.clone(),
                        Arc::clone(&resources),
                        connection_generation,
                    )
                }),
        }
    }

    fn connection_scope(
        &self,
        scope: ConnectionScope,
        manifest: &PluginManifest,
        task_tracker: Option<Arc<TaskTracker>>,
    ) -> PluginConnectionScope {
        let tasks = if manifest.capabilities.contains(PluginCapability::Tasks) {
            self.runtime
                .get()
                .cloned()
                .zip(task_tracker)
                .map(|(runtime, tracker)| PluginConnectionTasks {
                    runtime,
                    scope: scope.clone(),
                    tracker,
                })
        } else {
            None
        };
        PluginConnectionScope { scope, tasks }
    }

    async fn wait_for_tasks(&self, completion_signals: Vec<ShutdownSignal>) -> anyhow::Result<()> {
        let runtime = self
            .runtime
            .get()
            .ok_or_else(|| anyhow::anyhow!("plugin runtime is unavailable"))?;
        wait_for_plugin_tasks(&**runtime, self.callback_timeout, completion_signals).await
    }

    async fn install_all(&self, client: Weak<Client>) -> anyhow::Result<()> {
        let Some(strong_client) = client.upgrade() else {
            anyhow::bail!("client was dropped during plugin installation");
        };
        let runtime = strong_client.runtime.clone();
        let connection_generation = strong_client.connection_generation.clone();
        drop(strong_client);
        self.runtime
            .set(runtime.clone())
            .map_err(|_| anyhow::anyhow!("plugin host was installed more than once"))?;

        let installing_resources = &self.installing_resources;
        let _installing_resources = scopeguard::guard((), move |_| {
            installing_resources
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clear();
        });
        let mut rollback = PluginInstallRollback::new(runtime.clone(), self.ordered.len());
        self.abort_install_if_terminal(&mut rollback).await?;
        if let Some(upstream) = &self.upstream {
            if let Err(error) = plugin_callback(|| upstream.install(client.clone())).await {
                rollback.disarm();
                return Err(error);
            }
            rollback.upstream = Some(upstream.clone());
            self.abort_install_if_terminal(&mut rollback).await?;
        }

        let staging = Arc::new(ApiRegistry::default());
        rollback.staged_apis = Some(Arc::clone(&staging));
        for planned in &self.ordered {
            self.abort_install_if_terminal(&mut rollback).await?;
            let resources = PluginResources::new();
            let context = self.context(
                &client,
                planned,
                Arc::clone(&resources),
                Arc::clone(&staging),
                runtime.clone(),
                connection_generation.clone(),
            );
            rollback.current = Some(InstalledPlugin {
                plugin: planned.plugin.clone(),
                manifest: planned.manifest.clone(),
                resources: Arc::clone(&resources),
            });
            if !self.track_installing_resources(&resources) {
                rollback.rollback().await;
                anyhow::bail!("plugin host shut down during installation");
            }
            let terminal = self.terminal_notifier.subscribe();
            let cancelled = Box::pin(wait_for_shutdown(&terminal));
            let install = Box::pin(plugin_install(|| planned.plugin.install(context)));
            let install_result = match futures::future::select(cancelled, install).await {
                futures::future::Either::Left((_, install)) => {
                    if std::panic::catch_unwind(AssertUnwindSafe(|| drop(install))).is_err() {
                        log::warn!(
                            "Plugin `{}` install future panicked while being cancelled",
                            planned.manifest.id
                        );
                    }
                    rollback.rollback().await;
                    anyhow::bail!("plugin host shut down during installation");
                }
                futures::future::Either::Right((result, _)) => result,
            };
            let api = match install_result {
                Ok(api) => api,
                Err(error) => {
                    rollback.rollback().await;
                    anyhow::bail!(
                        "plugin `{}` installation failed: {error:#}",
                        planned.manifest.id
                    );
                }
            };
            staging.insert(planned.plugin.marker_type_id(), api);
            self.abort_install_if_terminal(&mut rollback).await?;
            let Some(installed) = rollback.current.take() else {
                rollback.rollback().await;
                anyhow::bail!("plugin installation rollback state was lost");
            };
            rollback.installed.push(installed);
        }
        self.abort_install_if_terminal(&mut rollback).await?;

        self.apis
            .set(staging.snapshot())
            .map_err(|_| anyhow::anyhow!("plugin APIs were published more than once"))?;
        let installed = rollback.take_installed();
        if let Err(installed) = self.installed.set(installed) {
            rollback.restore_installed(installed);
            anyhow::bail!("plugins were installed more than once");
        }
        rollback.disarm();
        Ok(())
    }

    async fn abort_install_if_terminal(
        &self,
        rollback: &mut PluginInstallRollback,
    ) -> anyhow::Result<()> {
        if !self.terminal.load(Ordering::Acquire) {
            return Ok(());
        }
        rollback.rollback().await;
        anyhow::bail!("plugin host shut down during installation")
    }

    fn track_installing_resources(&self, resources: &Arc<PluginResources>) -> bool {
        let mut installing = self
            .installing_resources
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if self.terminal.load(Ordering::Acquire) {
            drop(installing);
            if std::panic::catch_unwind(AssertUnwindSafe(|| resources.close())).is_err() {
                log::warn!("Installing plugin resource closure panicked");
            }
            return false;
        }
        installing.push(Arc::downgrade(resources));
        true
    }

    fn close_installing_resources(&self) {
        let resources = self
            .installing_resources
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .iter()
            .filter_map(Weak::upgrade)
            .collect::<Vec<_>>();
        for resources in resources {
            if std::panic::catch_unwind(AssertUnwindSafe(|| resources.close())).is_err() {
                log::warn!("Installing plugin resource closure panicked");
            }
        }
    }

    pub(crate) fn activate(&self) -> bool {
        if self.terminal.load(Ordering::Acquire) {
            self.close_installed_resources();
            return false;
        }
        for plugin in self.installed.get().into_iter().flatten() {
            plugin.resources.activate();
        }
        if self.terminal.load(Ordering::Acquire) {
            self.close_installed_resources();
            false
        } else {
            true
        }
    }

    fn close_installed_resources(&self) {
        for plugin in self.installed.get().into_iter().flatten().rev() {
            close_plugin_resources(&plugin.manifest.id, &plugin.resources);
        }
    }

    async fn run_callback<'a>(
        &'a self,
        make_future: impl FnOnce() -> BoxFuture<'a, anyhow::Result<()>>,
    ) -> anyhow::Result<()> {
        let runtime = self
            .runtime
            .get()
            .ok_or_else(|| anyhow::anyhow!("plugin runtime is unavailable"))?;
        bounded_plugin_callback(&**runtime, self.callback_timeout, make_future).await
    }
}

impl ClientLifecycle for PluginHost {
    fn install(&self, client: Weak<Client>) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async move { self.install_all(client).await })
    }

    fn on_ready(&self, scope: ConnectionScope) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async move {
            let mut failures = Vec::new();
            if let Some(upstream) = &self.upstream
                && let Err(error) = self.run_callback(|| upstream.on_ready(scope.clone())).await
            {
                failures.push(format!("upstream: {error:#}"));
            }
            for plugin in self.installed.get().into_iter().flatten() {
                let task_tracker = plugin
                    .manifest
                    .capabilities
                    .contains(PluginCapability::Tasks)
                    .then(|| plugin.resources.connection_task_tracker(scope.generation()));
                let plugin_scope =
                    self.connection_scope(scope.clone(), &plugin.manifest, task_tracker);
                if let Err(error) = self
                    .run_callback(|| plugin.plugin.on_ready(plugin_scope))
                    .await
                {
                    failures.push(format!("{}: {error:#}", plugin.manifest.id));
                }
            }
            finish_callbacks("ready", failures)
        })
    }

    fn on_closed(&self, scope: ConnectionScope) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async move {
            let mut failures = Vec::new();
            for plugin in self.installed.get().into_iter().flatten().rev() {
                let task_tracker = plugin
                    .manifest
                    .capabilities
                    .contains(PluginCapability::Tasks)
                    .then(|| plugin.resources.close_connection_tasks(scope.generation()));
                if let Some(task_tracker) = &task_tracker {
                    match self
                        .wait_for_tasks(vec![task_tracker.completion_signal()])
                        .await
                    {
                        Ok(()) => plugin
                            .resources
                            .forget_connection_tasks(scope.generation(), task_tracker),
                        Err(error) => {
                            failures.push(format!("{} tasks: {error:#}", plugin.manifest.id));
                        }
                    }
                }
                let plugin_scope =
                    self.connection_scope(scope.clone(), &plugin.manifest, task_tracker);
                if let Err(error) = self
                    .run_callback(|| plugin.plugin.on_closed(plugin_scope))
                    .await
                {
                    failures.push(format!("{}: {error:#}", plugin.manifest.id));
                }
            }
            if let Some(upstream) = &self.upstream
                && let Err(error) = self.run_callback(|| upstream.on_closed(scope)).await
            {
                failures.push(format!("upstream: {error:#}"));
            }
            finish_callbacks("closed", failures)
        })
    }

    fn signal_shutdown(&self) {
        self.terminal.store(true, Ordering::Release);
        self.close_installing_resources();
        self.terminal_notifier.notify();
        if let Some(router) = &self.event_router {
            router.close();
        }
        self.close_installed_resources();
        if let Some(upstream) = &self.upstream
            && std::panic::catch_unwind(AssertUnwindSafe(|| upstream.signal_shutdown())).is_err()
        {
            log::warn!("Upstream lifecycle synchronous shutdown signal panicked");
        }
    }

    fn shutdown(&self) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async move {
            let mut failures = Vec::new();
            self.signal_shutdown();
            for plugin in self.installed.get().into_iter().flatten().rev() {
                if let Err(error) = self
                    .wait_for_tasks(plugin.resources.task_completion_signals())
                    .await
                {
                    failures.push(format!("{} tasks: {error:#}", plugin.manifest.id));
                }
                if let Err(error) = self.run_callback(|| plugin.plugin.shutdown()).await {
                    failures.push(format!("{}: {error:#}", plugin.manifest.id));
                }
            }
            if let Some(upstream) = &self.upstream
                && let Err(error) = self.run_callback(|| upstream.shutdown()).await
            {
                failures.push(format!("upstream: {error:#}"));
            }
            finish_callbacks("shutdown", failures)
        })
    }
}

impl Drop for PluginHost {
    fn drop(&mut self) {
        self.signal_shutdown();
    }
}

async fn shutdown_staged_plugins(
    runtime: Arc<dyn Runtime>,
    current: Option<InstalledPlugin>,
    mut installed: Vec<InstalledPlugin>,
    upstream: Option<Arc<dyn ClientLifecycle>>,
    staged_apis: Option<Arc<ApiRegistry>>,
) {
    if let Some(plugin) = current {
        if let Err(error) = wait_for_plugin_tasks(
            &*runtime,
            PLUGIN_CALLBACK_TIMEOUT,
            plugin.resources.task_completion_signals(),
        )
        .await
        {
            log::warn!(
                "Plugin `{}` failed-install task cleanup failed: {error:#}",
                plugin.manifest.id
            );
        }
        if let Err(error) = bounded_plugin_callback(&*runtime, PLUGIN_CALLBACK_TIMEOUT, || {
            plugin.plugin.shutdown()
        })
        .await
        {
            log::warn!(
                "Plugin `{}` failed-install rollback failed: {error:#}",
                plugin.manifest.id
            );
        }
    }
    while let Some(plugin) = installed.pop() {
        if let Err(error) = wait_for_plugin_tasks(
            &*runtime,
            PLUGIN_CALLBACK_TIMEOUT,
            plugin.resources.task_completion_signals(),
        )
        .await
        {
            log::warn!(
                "Plugin `{}` rollback task cleanup failed: {error:#}",
                plugin.manifest.id
            );
        }
        if let Err(error) = bounded_plugin_callback(&*runtime, PLUGIN_CALLBACK_TIMEOUT, || {
            plugin.plugin.shutdown()
        })
        .await
        {
            log::warn!("Plugin `{}` rollback failed: {error:#}", plugin.manifest.id);
        }
    }
    if let Some(upstream) = upstream
        && let Err(error) =
            bounded_plugin_callback(&*runtime, PLUGIN_CALLBACK_TIMEOUT, || upstream.shutdown())
                .await
    {
        log::warn!("Upstream lifecycle rollback failed: {error:#}");
    }
    if std::panic::catch_unwind(AssertUnwindSafe(|| drop(staged_apis))).is_err() {
        log::warn!("Plugin API panicked while being dropped during rollback");
    }
}

async fn wait_for_plugin_tasks(
    runtime: &dyn Runtime,
    timeout: Duration,
    completion_signals: Vec<ShutdownSignal>,
) -> anyhow::Result<()> {
    let wait_for_all = async move {
        for signal in completion_signals {
            wait_for_shutdown(&signal).await;
        }
    };
    runtime_timeout(runtime, timeout, wait_for_all)
        .await
        .map_err(|_| {
            anyhow::anyhow!(
                "plugin tasks did not stop within {:.3} seconds",
                timeout.as_secs_f64()
            )
        })
}

async fn bounded_plugin_callback<'a>(
    runtime: &dyn Runtime,
    timeout: Duration,
    make_future: impl FnOnce() -> BoxFuture<'a, anyhow::Result<()>>,
) -> anyhow::Result<()> {
    match runtime_timeout(runtime, timeout, plugin_callback(make_future)).await {
        Ok(result) => result,
        Err(_) => anyhow::bail!(
            "callback timed out after {:.3} seconds",
            timeout.as_secs_f64()
        ),
    }
}

async fn plugin_callback<'a>(
    make_future: impl FnOnce() -> BoxFuture<'a, anyhow::Result<()>>,
) -> anyhow::Result<()> {
    let future = std::panic::catch_unwind(AssertUnwindSafe(make_future))
        .map_err(|_| anyhow::anyhow!("callback panicked before returning a future"))?;
    AssertUnwindSafe(future)
        .catch_unwind()
        .await
        .map_err(|_| anyhow::anyhow!("callback future panicked"))?
}

async fn plugin_install<'a>(
    make_future: impl FnOnce() -> BoxFuture<'a, anyhow::Result<ErasedApi>>,
) -> anyhow::Result<ErasedApi> {
    let future = std::panic::catch_unwind(AssertUnwindSafe(make_future))
        .map_err(|_| anyhow::anyhow!("install panicked before returning a future"))?;
    AssertUnwindSafe(future)
        .catch_unwind()
        .await
        .map_err(|_| anyhow::anyhow!("install future panicked"))?
}

fn finish_callbacks(stage: &str, failures: Vec<String>) -> anyhow::Result<()> {
    if failures.is_empty() {
        Ok(())
    } else {
        anyhow::bail!("plugin {stage} callbacks failed: {}", failures.join("; "))
    }
}

impl Client {
    /// Return the API exposed by plugin marker `P`, if that plugin was installed.
    pub fn plugin<P: ClientPlugin>(&self) -> Option<Arc<P::Api>> {
        self.plugin_host.as_ref()?.plugin::<P>()
    }

    /// Manifests in dependency-resolved installation order.
    pub fn plugin_manifests(&self) -> &[PluginManifest] {
        self.plugin_host
            .as_ref()
            .map(|host| host.manifests())
            .unwrap_or_default()
    }

    /// Subscribe to custom events emitted by installed plugins.
    ///
    /// Returns `None` when no manifest requested custom-event publication.
    pub fn plugin_event_router(&self) -> Option<PluginEventRouter> {
        self.plugin_host
            .as_ref()
            .and_then(|host| host.event_router.clone())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicBool;
    use std::time::Duration;

    use bytes::Bytes;

    use super::*;
    use crate::client::{ClientBuilder, ClientBuilderError};
    use crate::runtime_impl::TokioRuntime;
    use crate::store::persistence_manager::PersistenceManager;
    use crate::test_utils::MockHttpClient;
    use crate::transport::mock::MockTransportFactory;

    type Log = Arc<Mutex<Vec<String>>>;

    fn record(log: &Log, value: impl Into<String>) {
        log.lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .push(value.into());
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

    struct FoundationPlugin {
        log: Log,
    }

    struct ShutdownDuringPluginInstall;

    struct CaptureInstallClient {
        client: async_channel::Sender<Weak<Client>>,
    }

    impl ClientLifecycle for CaptureInstallClient {
        fn install(&self, client: Weak<Client>) -> BoxFuture<'_, anyhow::Result<()>> {
            let sender = self.client.clone();
            Box::pin(async move {
                sender.send(client).await?;
                Ok(())
            })
        }
    }

    struct TerminalBlockingInstallPlugin {
        started: async_channel::Sender<ShutdownSignal>,
        install_dropped: Arc<AtomicBool>,
        shutdown_called: Arc<AtomicBool>,
    }

    impl ClientPlugin for TerminalBlockingInstallPlugin {
        type Api = ();

        fn manifest(&self) -> PluginManifest {
            PluginManifest::new("terminal-blocking-install", "0.1.0")
                .with_capability(PluginCapability::Tasks)
        }

        fn install(&self, context: PluginContext) -> BoxFuture<'_, anyhow::Result<Arc<Self::Api>>> {
            let started = self.started.clone();
            let install_dropped = self.install_dropped.clone();
            Box::pin(async move {
                let _drop = DropFlag(install_dropped);
                let shutdown = context
                    .tasks()
                    .ok_or_else(|| anyhow::anyhow!("tasks capability missing"))?
                    .shutdown_signal();
                started.send(shutdown).await?;
                futures::future::pending().await
            })
        }

        fn shutdown(&self) -> BoxFuture<'_, anyhow::Result<()>> {
            let shutdown_called = self.shutdown_called.clone();
            Box::pin(async move {
                shutdown_called.store(true, Ordering::Release);
                Ok(())
            })
        }
    }

    impl ClientLifecycle for ShutdownDuringPluginInstall {
        fn install(&self, client: Weak<Client>) -> BoxFuture<'_, anyhow::Result<()>> {
            Box::pin(async move {
                client
                    .upgrade()
                    .ok_or_else(|| anyhow::anyhow!("client unavailable during install"))?
                    .signal_shutdown_sync();
                Ok(())
            })
        }
    }

    impl ClientPlugin for FoundationPlugin {
        type Api = String;

        fn manifest(&self) -> PluginManifest {
            PluginManifest::new("foundation", "0.1.0")
        }

        fn install(
            &self,
            _context: PluginContext,
        ) -> BoxFuture<'_, anyhow::Result<Arc<Self::Api>>> {
            let log = self.log.clone();
            Box::pin(async move {
                record(&log, "install:foundation");
                Ok(Arc::new("foundation-api".to_string()))
            })
        }

        fn shutdown(&self) -> BoxFuture<'_, anyhow::Result<()>> {
            let log = self.log.clone();
            Box::pin(async move {
                record(&log, "shutdown:foundation");
                Ok(())
            })
        }
    }

    #[tokio::test]
    async fn shutdown_during_upstream_install_prevents_plugin_installation() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let result = complete_builder()
            .await
            .with_lifecycle(ShutdownDuringPluginInstall)
            .with_plugin(FoundationPlugin { log: log.clone() })
            .build()
            .await;

        assert!(matches!(result, Err(ClientBuilderError::PluginInstall(_))));
        assert!(
            log.lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .is_empty()
        );
    }

    #[tokio::test]
    async fn shutdown_cancels_an_inflight_plugin_install_and_closes_its_resources() {
        let (client_tx, client_rx) = async_channel::bounded(1);
        let (started_tx, started_rx) = async_channel::bounded(1);
        let install_dropped = Arc::new(AtomicBool::new(false));
        let shutdown_called = Arc::new(AtomicBool::new(false));
        let builder = complete_builder()
            .await
            .with_lifecycle(CaptureInstallClient { client: client_tx })
            .with_plugin(TerminalBlockingInstallPlugin {
                started: started_tx,
                install_dropped: install_dropped.clone(),
                shutdown_called: shutdown_called.clone(),
            });

        let build = tokio::spawn(async move { builder.build().await });
        let client = client_rx
            .recv()
            .await
            .expect("captured install client")
            .upgrade()
            .expect("client under construction");
        let resource_shutdown = started_rx.recv().await.expect("plugin install started");

        client.signal_shutdown_sync();
        assert!(resource_shutdown.is_fired());
        let result = tokio::time::timeout(Duration::from_secs(2), build)
            .await
            .expect("plugin install ignored terminal shutdown")
            .expect("build task");

        assert!(matches!(result, Err(ClientBuilderError::PluginInstall(_))));
        assert!(install_dropped.load(Ordering::Acquire));
        assert!(shutdown_called.load(Ordering::Acquire));
        drop(client);
    }

    struct DependentPlugin {
        log: Log,
    }

    impl ClientPlugin for DependentPlugin {
        type Api = String;

        fn manifest(&self) -> PluginManifest {
            PluginManifest::new("dependent", "0.1.0").with_dependency("foundation")
        }

        fn install(&self, context: PluginContext) -> BoxFuture<'_, anyhow::Result<Arc<Self::Api>>> {
            let log = self.log.clone();
            Box::pin(async move {
                let foundation = context
                    .plugin::<FoundationPlugin>()
                    .ok_or_else(|| anyhow::anyhow!("foundation API is unavailable"))?;
                anyhow::ensure!(&*foundation == "foundation-api");
                record(&log, "install:dependent");
                Ok(Arc::new("dependent-api".to_string()))
            })
        }

        fn shutdown(&self) -> BoxFuture<'_, anyhow::Result<()>> {
            let log = self.log.clone();
            Box::pin(async move {
                record(&log, "shutdown:dependent");
                Ok(())
            })
        }
    }

    #[tokio::test]
    async fn installs_in_dependency_order_and_indexes_by_marker_type() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let build = complete_builder()
            .await
            .with_plugin(DependentPlugin { log: log.clone() })
            .with_plugin(FoundationPlugin { log: log.clone() })
            .build()
            .await
            .expect("valid plugin plan");
        let client = build.into_client();

        assert_eq!(
            client
                .plugin::<FoundationPlugin>()
                .as_deref()
                .map(String::as_str),
            Some("foundation-api")
        );
        assert_eq!(
            client
                .plugin::<DependentPlugin>()
                .as_deref()
                .map(String::as_str),
            Some("dependent-api")
        );
        assert_eq!(
            client
                .plugin_manifests()
                .iter()
                .map(PluginManifest::id)
                .collect::<Vec<_>>(),
            vec!["foundation", "dependent"]
        );
        assert_eq!(
            *log.lock().unwrap_or_else(|poisoned| poisoned.into_inner()),
            vec!["install:foundation", "install:dependent"]
        );

        client.disconnect().await;
        assert_eq!(
            *log.lock().unwrap_or_else(|poisoned| poisoned.into_inner()),
            vec![
                "install:foundation",
                "install:dependent",
                "shutdown:dependent",
                "shutdown:foundation"
            ]
        );
    }

    struct DeclarativePlugin<const MARKER: u8> {
        id: &'static str,
        dependency: Option<&'static str>,
    }

    struct TransitiveProbe;

    impl ClientPlugin for TransitiveProbe {
        type Api = bool;

        fn manifest(&self) -> PluginManifest {
            PluginManifest::new("transitive-probe", "0.1.0").with_dependency("dependent")
        }

        fn install(&self, context: PluginContext) -> BoxFuture<'_, anyhow::Result<Arc<Self::Api>>> {
            Box::pin(async move {
                anyhow::ensure!(context.plugin::<DependentPlugin>().is_some());
                Ok(Arc::new(context.plugin::<FoundationPlugin>().is_none()))
            })
        }
    }

    #[tokio::test]
    async fn install_context_exposes_only_direct_declared_dependencies() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let build = complete_builder()
            .await
            .with_plugin(FoundationPlugin { log: log.clone() })
            .with_plugin(DependentPlugin { log })
            .with_plugin(TransitiveProbe)
            .build()
            .await
            .expect("declared dependency plan");
        let client = build.into_client();
        assert_eq!(client.plugin::<TransitiveProbe>().as_deref(), Some(&true));
        client.disconnect().await;
    }

    impl<const MARKER: u8> ClientPlugin for DeclarativePlugin<MARKER> {
        type Api = ();

        fn manifest(&self) -> PluginManifest {
            let manifest = PluginManifest::new(self.id, "0.1.0");
            match self.dependency {
                Some(dependency) => manifest.with_dependency(dependency),
                None => manifest,
            }
        }

        fn install(
            &self,
            _context: PluginContext,
        ) -> BoxFuture<'_, anyhow::Result<Arc<Self::Api>>> {
            Box::pin(async { Ok(Arc::new(())) })
        }
    }

    #[test]
    fn rejects_duplicate_ids_missing_dependencies_and_cycles() {
        let duplicate = PluginPlan::prepare(vec![
            PluginRegistration::new(DeclarativePlugin::<1> {
                id: "same",
                dependency: None,
            }),
            PluginRegistration::new(DeclarativePlugin::<2> {
                id: "same",
                dependency: None,
            }),
        ]);
        assert!(matches!(
            duplicate,
            Err(PluginPlanError::DuplicateId { ref id }) if id == "same"
        ));

        let missing = PluginPlan::prepare(vec![PluginRegistration::new(DeclarativePlugin::<3> {
            id: "orphan",
            dependency: Some("absent"),
        })]);
        assert!(matches!(
            missing,
            Err(PluginPlanError::MissingDependency {
                ref plugin_id,
                ref dependency,
            }) if plugin_id == "orphan" && dependency == "absent"
        ));

        let cycle = PluginPlan::prepare(vec![
            PluginRegistration::new(DeclarativePlugin::<4> {
                id: "cycle-a",
                dependency: Some("cycle-b"),
            }),
            PluginRegistration::new(DeclarativePlugin::<5> {
                id: "cycle-b",
                dependency: Some("cycle-a"),
            }),
        ]);
        assert!(matches!(
            cycle,
            Err(PluginPlanError::DependencyCycle { ref plugins })
                if plugins == &["cycle-a", "cycle-b"]
        ));
    }

    struct FixedManifestPlugin<const MARKER: u8>(PluginManifest);

    impl<const MARKER: u8> ClientPlugin for FixedManifestPlugin<MARKER> {
        type Api = ();

        fn manifest(&self) -> PluginManifest {
            self.0.clone()
        }

        fn install(
            &self,
            _context: PluginContext,
        ) -> BoxFuture<'_, anyhow::Result<Arc<Self::Api>>> {
            Box::pin(async { Ok(Arc::new(())) })
        }
    }

    struct PanickingManifestPlugin;

    impl ClientPlugin for PanickingManifestPlugin {
        type Api = ();

        fn manifest(&self) -> PluginManifest {
            panic!("injected manifest panic")
        }

        fn install(
            &self,
            _context: PluginContext,
        ) -> BoxFuture<'_, anyhow::Result<Arc<Self::Api>>> {
            Box::pin(async { Ok(Arc::new(())) })
        }
    }

    #[test]
    fn rejects_invalid_or_ambiguous_manifests_without_installing() {
        let duplicate_type = PluginPlan::prepare(vec![
            PluginRegistration::new(FixedManifestPlugin::<1>(PluginManifest::new(
                "first", "0.1.0",
            ))),
            PluginRegistration::new(FixedManifestPlugin::<1>(PluginManifest::new(
                "second", "0.1.0",
            ))),
        ]);
        assert!(matches!(
            duplicate_type,
            Err(PluginPlanError::DuplicateType { .. })
        ));

        let invalid_id = PluginPlan::prepare(vec![PluginRegistration::new(
            FixedManifestPlugin::<2>(PluginManifest::new("Invalid", "0.1.0")),
        )]);
        assert!(matches!(invalid_id, Err(PluginPlanError::InvalidId { .. })));

        let invalid_version = PluginPlan::prepare(vec![PluginRegistration::new(
            FixedManifestPlugin::<3>(PluginManifest::new("invalid-version", "0.1 0")),
        )]);
        assert!(matches!(
            invalid_version,
            Err(PluginPlanError::InvalidVersion { .. })
        ));

        let duplicate_dependency = PluginPlan::prepare(vec![
            PluginRegistration::new(FixedManifestPlugin::<4>(PluginManifest::new(
                "base", "0.1.0",
            ))),
            PluginRegistration::new(FixedManifestPlugin::<5>(
                PluginManifest::new("duplicate-dependency", "0.1.0")
                    .with_dependency("base")
                    .with_dependency("base"),
            )),
        ]);
        assert!(matches!(
            duplicate_dependency,
            Err(PluginPlanError::DuplicateDependency { .. })
        ));

        let manifest_panic =
            PluginPlan::prepare(vec![PluginRegistration::new(PanickingManifestPlugin)]);
        assert!(matches!(
            manifest_panic,
            Err(PluginPlanError::ManifestPanicked { .. })
        ));
    }

    struct DropFlag(Arc<AtomicBool>);

    impl Drop for DropFlag {
        fn drop(&mut self) {
            self.0.store(true, Ordering::Release);
        }
    }

    struct PanickingDropApi;

    impl Drop for PanickingDropApi {
        fn drop(&mut self) {
            panic!("injected API drop panic");
        }
    }

    struct PanickingDropPlugin {
        shutdown_called: Arc<AtomicBool>,
    }

    impl ClientPlugin for PanickingDropPlugin {
        type Api = PanickingDropApi;

        fn manifest(&self) -> PluginManifest {
            PluginManifest::new("panicking-drop", "0.1.0")
        }

        fn install(
            &self,
            _context: PluginContext,
        ) -> BoxFuture<'_, anyhow::Result<Arc<Self::Api>>> {
            Box::pin(async { Ok(Arc::new(PanickingDropApi)) })
        }

        fn shutdown(&self) -> BoxFuture<'_, anyhow::Result<()>> {
            let shutdown_called = self.shutdown_called.clone();
            Box::pin(async move {
                shutdown_called.store(true, Ordering::Release);
                Ok(())
            })
        }
    }

    #[tokio::test]
    async fn panicking_staged_api_drop_cannot_strand_rollback_completion() {
        let shutdown_called = Arc::new(AtomicBool::new(false));
        let plugin = Arc::new(PanickingDropPlugin {
            shutdown_called: shutdown_called.clone(),
        });
        let manifest = plugin.manifest();
        let erased_plugin: Arc<dyn ErasedClientPlugin> = Arc::new(PluginAdapter(plugin));
        let resources = PluginResources::new();
        let registry = Arc::new(ApiRegistry::default());
        let api: ErasedApi = Arc::new(TypedApi(Arc::new(PanickingDropApi)));
        registry.insert(TypeId::of::<PanickingDropPlugin>(), api);

        let mut rollback = PluginInstallRollback::new(Arc::new(TokioRuntime), 1);
        rollback.installed.push(InstalledPlugin {
            plugin: erased_plugin,
            manifest,
            resources,
        });
        rollback.staged_apis = Some(registry);

        tokio::time::timeout(Duration::from_secs(2), rollback.rollback())
            .await
            .expect("panicking API drop stranded rollback completion");
        assert!(shutdown_called.load(Ordering::Acquire));
    }

    struct ContextRetainingApi {
        _context: PluginContext,
        _drop_flag: DropFlag,
    }

    struct ContextRetainingPlugin {
        api_dropped: Arc<AtomicBool>,
    }

    impl ClientPlugin for ContextRetainingPlugin {
        type Api = ContextRetainingApi;

        fn manifest(&self) -> PluginManifest {
            PluginManifest::new("context-retaining", "0.1.0")
        }

        fn install(&self, context: PluginContext) -> BoxFuture<'_, anyhow::Result<Arc<Self::Api>>> {
            let api_dropped = self.api_dropped.clone();
            Box::pin(async move {
                Ok(Arc::new(ContextRetainingApi {
                    _context: context,
                    _drop_flag: DropFlag(api_dropped),
                }))
            })
        }
    }

    #[tokio::test]
    async fn retained_context_does_not_cycle_with_the_api_registry() {
        let api_dropped = Arc::new(AtomicBool::new(false));
        let build = complete_builder()
            .await
            .with_plugin(ContextRetainingPlugin {
                api_dropped: api_dropped.clone(),
            })
            .build()
            .await
            .expect("context-retaining plugin");
        let (client, sync_tasks) = build.into_parts();
        drop(sync_tasks);
        let api = client
            .plugin::<ContextRetainingPlugin>()
            .expect("retained-context API");
        let weak_api = Arc::downgrade(&api);
        drop(api);

        client.disconnect().await;
        drop(client);
        wait_for_flag(&api_dropped).await;
        assert!(weak_api.upgrade().is_none());
    }

    struct RollbackPlugin {
        log: Log,
        task_dropped: Arc<AtomicBool>,
        api_dropped: Arc<AtomicBool>,
    }

    struct RollbackApi {
        _drop_flag: DropFlag,
    }

    impl ClientPlugin for RollbackPlugin {
        type Api = RollbackApi;

        fn manifest(&self) -> PluginManifest {
            PluginManifest::new("rollback", "0.1.0").with_capability(PluginCapability::Tasks)
        }

        fn install(&self, context: PluginContext) -> BoxFuture<'_, anyhow::Result<Arc<Self::Api>>> {
            let log = self.log.clone();
            let task_dropped = self.task_dropped.clone();
            let api_dropped = self.api_dropped.clone();
            Box::pin(async move {
                record(&log, "install:rollback");
                let guard = DropFlag(task_dropped);
                context
                    .tasks()
                    .ok_or_else(|| anyhow::anyhow!("tasks capability missing"))?
                    .spawn(async move {
                        let _guard = guard;
                        futures::future::pending::<()>().await;
                    })?;
                Ok(Arc::new(RollbackApi {
                    _drop_flag: DropFlag(api_dropped),
                }))
            })
        }

        fn shutdown(&self) -> BoxFuture<'_, anyhow::Result<()>> {
            let log = self.log.clone();
            let task_dropped = self.task_dropped.clone();
            let api_dropped = self.api_dropped.clone();
            Box::pin(async move {
                anyhow::ensure!(
                    task_dropped.load(Ordering::Acquire),
                    "rollback task still running during shutdown"
                );
                anyhow::ensure!(
                    !api_dropped.load(Ordering::Acquire),
                    "rollback API dropped before shutdown"
                );
                record(&log, "shutdown:rollback");
                Ok(())
            })
        }
    }

    struct FailingPlugin {
        log: Log,
    }

    impl ClientPlugin for FailingPlugin {
        type Api = ();

        fn manifest(&self) -> PluginManifest {
            PluginManifest::new("failing", "0.1.0").with_dependency("rollback")
        }

        fn install(
            &self,
            _context: PluginContext,
        ) -> BoxFuture<'_, anyhow::Result<Arc<Self::Api>>> {
            let log = self.log.clone();
            Box::pin(async move {
                record(&log, "install:failing");
                anyhow::bail!("injected failure")
            })
        }

        fn shutdown(&self) -> BoxFuture<'_, anyhow::Result<()>> {
            let log = self.log.clone();
            Box::pin(async move {
                record(&log, "shutdown:failing");
                Ok(())
            })
        }
    }

    #[tokio::test]
    async fn install_failure_rolls_back_resources_and_plugins_in_lifo_order() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let task_dropped = Arc::new(AtomicBool::new(false));
        let api_dropped = Arc::new(AtomicBool::new(false));
        let result = complete_builder()
            .await
            .with_lifecycle(UpstreamLifecycle { log: log.clone() })
            .with_plugin(FailingPlugin { log: log.clone() })
            .with_plugin(RollbackPlugin {
                log: log.clone(),
                task_dropped: task_dropped.clone(),
                api_dropped: api_dropped.clone(),
            })
            .build()
            .await;

        assert!(matches!(result, Err(ClientBuilderError::PluginInstall(_))));
        assert_eq!(
            *log.lock().unwrap_or_else(|poisoned| poisoned.into_inner()),
            vec![
                "install:upstream",
                "install:rollback",
                "install:failing",
                "shutdown:failing",
                "shutdown:rollback",
                "shutdown:upstream"
            ]
        );
        tokio::time::timeout(Duration::from_secs(1), async {
            while !task_dropped.load(Ordering::Acquire) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("rollback aborted the install-scoped task");
        assert!(api_dropped.load(Ordering::Acquire));
    }

    struct BlockingFailingPlugin {
        log: Log,
        started: async_channel::Sender<()>,
        release: async_channel::Receiver<()>,
    }

    impl ClientPlugin for BlockingFailingPlugin {
        type Api = ();

        fn manifest(&self) -> PluginManifest {
            PluginManifest::new("blocking-failure", "0.1.0").with_dependency("rollback")
        }

        fn install(
            &self,
            _context: PluginContext,
        ) -> BoxFuture<'_, anyhow::Result<Arc<Self::Api>>> {
            let log = self.log.clone();
            Box::pin(async move {
                record(&log, "install:blocking-failure");
                anyhow::bail!("injected failure")
            })
        }

        fn shutdown(&self) -> BoxFuture<'_, anyhow::Result<()>> {
            let log = self.log.clone();
            let started = self.started.clone();
            let release = self.release.clone();
            Box::pin(async move {
                record(&log, "shutdown:blocking-failure-started");
                let _ = started.try_send(());
                let _ = release.recv().await;
                record(&log, "shutdown:blocking-failure-finished");
                Ok(())
            })
        }
    }

    struct SignalAwareUpstream {
        log: Log,
        signalled: Arc<AtomicBool>,
        shutdown_saw_signal: Arc<AtomicBool>,
    }

    impl ClientLifecycle for SignalAwareUpstream {
        fn install(&self, _client: Weak<Client>) -> BoxFuture<'_, anyhow::Result<()>> {
            let log = self.log.clone();
            Box::pin(async move {
                record(&log, "install:upstream");
                Ok(())
            })
        }

        fn signal_shutdown(&self) {
            if !self.signalled.swap(true, Ordering::AcqRel) {
                record(&self.log, "signal:upstream");
            }
        }

        fn shutdown(&self) -> BoxFuture<'_, anyhow::Result<()>> {
            let log = self.log.clone();
            let signalled = self.signalled.clone();
            let shutdown_saw_signal = self.shutdown_saw_signal.clone();
            Box::pin(async move {
                shutdown_saw_signal.store(signalled.load(Ordering::Acquire), Ordering::Release);
                record(&log, "shutdown:upstream");
                Ok(())
            })
        }
    }

    #[tokio::test]
    async fn cancelled_explicit_rollback_finishes_detached_and_signals_upstream() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let task_dropped = Arc::new(AtomicBool::new(false));
        let api_dropped = Arc::new(AtomicBool::new(false));
        let signalled = Arc::new(AtomicBool::new(false));
        let shutdown_saw_signal = Arc::new(AtomicBool::new(false));
        let (started_tx, started_rx) = async_channel::bounded(1);
        let (release_tx, release_rx) = async_channel::bounded(1);
        let builder = complete_builder()
            .await
            .with_lifecycle(SignalAwareUpstream {
                log: log.clone(),
                signalled: signalled.clone(),
                shutdown_saw_signal: shutdown_saw_signal.clone(),
            })
            .with_plugin(BlockingFailingPlugin {
                log: log.clone(),
                started: started_tx,
                release: release_rx,
            })
            .with_plugin(RollbackPlugin {
                log: log.clone(),
                task_dropped: task_dropped.clone(),
                api_dropped: api_dropped.clone(),
            });

        let build = tokio::spawn(async move { builder.build().await });
        started_rx
            .recv()
            .await
            .expect("failed plugin rollback started");
        assert!(signalled.load(Ordering::Acquire));
        build.abort();
        let _ = build.await;
        release_tx.send(()).await.expect("release rollback hook");

        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                let complete = log
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .last()
                    .is_some_and(|entry| entry == "shutdown:upstream");
                if complete
                    && task_dropped.load(Ordering::Acquire)
                    && api_dropped.load(Ordering::Acquire)
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("detached rollback completed after build cancellation");

        assert!(shutdown_saw_signal.load(Ordering::Acquire));
        assert_eq!(
            *log.lock().unwrap_or_else(|poisoned| poisoned.into_inner()),
            vec![
                "install:upstream",
                "install:rollback",
                "install:blocking-failure",
                "signal:upstream",
                "shutdown:blocking-failure-started",
                "shutdown:blocking-failure-finished",
                "shutdown:rollback",
                "shutdown:upstream"
            ]
        );
    }

    struct BlockingInstallPlugin {
        log: Log,
        started: async_channel::Sender<()>,
        release: async_channel::Receiver<()>,
    }

    impl ClientPlugin for BlockingInstallPlugin {
        type Api = ();

        fn manifest(&self) -> PluginManifest {
            PluginManifest::new("blocking-install", "0.1.0").with_dependency("rollback")
        }

        fn install(
            &self,
            _context: PluginContext,
        ) -> BoxFuture<'_, anyhow::Result<Arc<Self::Api>>> {
            let log = self.log.clone();
            let started = self.started.clone();
            let release = self.release.clone();
            Box::pin(async move {
                record(&log, "install:blocking");
                let _ = started.try_send(());
                release.recv().await?;
                Ok(Arc::new(()))
            })
        }

        fn shutdown(&self) -> BoxFuture<'_, anyhow::Result<()>> {
            let log = self.log.clone();
            Box::pin(async move {
                record(&log, "shutdown:blocking");
                Ok(())
            })
        }
    }

    #[tokio::test]
    async fn cancelled_build_closes_resources_and_schedules_lifo_rollback() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let task_dropped = Arc::new(AtomicBool::new(false));
        let api_dropped = Arc::new(AtomicBool::new(false));
        let (started_tx, started_rx) = async_channel::bounded(1);
        let (_release_tx, release_rx) = async_channel::bounded(1);
        let builder = complete_builder()
            .await
            .with_plugin(BlockingInstallPlugin {
                log: log.clone(),
                started: started_tx,
                release: release_rx,
            })
            .with_plugin(RollbackPlugin {
                log: log.clone(),
                task_dropped: task_dropped.clone(),
                api_dropped: api_dropped.clone(),
            });

        let build = tokio::spawn(async move { builder.build().await });
        started_rx.recv().await.expect("blocking install started");
        build.abort();
        let _ = build.await;

        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                let complete = log
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .last()
                    .is_some_and(|entry| entry == "shutdown:rollback");
                if complete
                    && task_dropped.load(Ordering::Acquire)
                    && api_dropped.load(Ordering::Acquire)
                {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("cancelled build rollback completed");

        assert_eq!(
            *log.lock().unwrap_or_else(|poisoned| poisoned.into_inner()),
            vec![
                "install:rollback",
                "install:blocking",
                "shutdown:blocking",
                "shutdown:rollback"
            ]
        );
    }

    struct PanickingPlugin {
        log: Log,
    }

    impl ClientPlugin for PanickingPlugin {
        type Api = ();

        fn manifest(&self) -> PluginManifest {
            PluginManifest::new("panicking", "0.1.0").with_dependency("rollback")
        }

        fn install(
            &self,
            _context: PluginContext,
        ) -> BoxFuture<'_, anyhow::Result<Arc<Self::Api>>> {
            record(&self.log, "install:panicking");
            panic!("injected install panic")
        }

        fn shutdown(&self) -> BoxFuture<'_, anyhow::Result<()>> {
            let log = self.log.clone();
            Box::pin(async move {
                record(&log, "shutdown:panicking");
                Ok(())
            })
        }
    }

    #[tokio::test]
    async fn install_panic_isolated_and_rolled_back() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let result = complete_builder()
            .await
            .with_plugin(PanickingPlugin { log: log.clone() })
            .with_plugin(RollbackPlugin {
                log: log.clone(),
                task_dropped: Arc::new(AtomicBool::new(false)),
                api_dropped: Arc::new(AtomicBool::new(false)),
            })
            .build()
            .await;

        assert!(matches!(result, Err(ClientBuilderError::PluginInstall(_))));
        assert_eq!(
            *log.lock().unwrap_or_else(|poisoned| poisoned.into_inner()),
            vec![
                "install:rollback",
                "install:panicking",
                "shutdown:panicking",
                "shutdown:rollback"
            ]
        );
    }

    struct ScopedTaskPlugin {
        install_started: Arc<AtomicBool>,
        install_dropped: Arc<AtomicBool>,
        connection_started: Arc<AtomicBool>,
        connection_dropped: Arc<AtomicBool>,
        closed_after_task: Arc<AtomicBool>,
        shutdown_after_task: Arc<AtomicBool>,
    }

    impl ClientPlugin for ScopedTaskPlugin {
        type Api = ();

        fn manifest(&self) -> PluginManifest {
            PluginManifest::new("scoped-tasks", "0.1.0").with_capability(PluginCapability::Tasks)
        }

        fn install(&self, context: PluginContext) -> BoxFuture<'_, anyhow::Result<Arc<Self::Api>>> {
            let started = self.install_started.clone();
            let observed = self.install_started.clone();
            let dropped = self.install_dropped.clone();
            Box::pin(async move {
                context
                    .tasks()
                    .ok_or_else(|| anyhow::anyhow!("tasks capability missing"))?
                    .spawn(async move {
                        started.store(true, Ordering::Release);
                        let _guard = DropFlag(dropped);
                        futures::future::pending::<()>().await;
                    })?;
                tokio::task::yield_now().await;
                anyhow::ensure!(!observed.load(Ordering::Acquire));
                Ok(Arc::new(()))
            })
        }

        fn on_ready(&self, scope: PluginConnectionScope) -> BoxFuture<'_, anyhow::Result<()>> {
            let started = self.connection_started.clone();
            let dropped = self.connection_dropped.clone();
            Box::pin(async move {
                scope
                    .tasks()
                    .ok_or_else(|| anyhow::anyhow!("connection tasks capability missing"))?
                    .spawn(async move {
                        started.store(true, Ordering::Release);
                        let _guard = DropFlag(dropped);
                        futures::future::pending::<()>().await;
                    })?;
                Ok(())
            })
        }

        fn on_closed(&self, _scope: PluginConnectionScope) -> BoxFuture<'_, anyhow::Result<()>> {
            let task_dropped = self.connection_dropped.load(Ordering::Acquire);
            let closed_after_task = self.closed_after_task.clone();
            Box::pin(async move {
                closed_after_task.store(task_dropped, Ordering::Release);
                Ok(())
            })
        }

        fn shutdown(&self) -> BoxFuture<'_, anyhow::Result<()>> {
            let task_dropped = self.install_dropped.load(Ordering::Acquire);
            let shutdown_after_task = self.shutdown_after_task.clone();
            Box::pin(async move {
                shutdown_after_task.store(task_dropped, Ordering::Release);
                Ok(())
            })
        }
    }

    async fn wait_for_flag(flag: &AtomicBool) {
        tokio::time::timeout(Duration::from_secs(1), async {
            while !flag.load(Ordering::Acquire) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("task state transition");
    }

    #[tokio::test]
    async fn install_tasks_start_after_publish_and_outlive_connection_tasks() {
        let install_started = Arc::new(AtomicBool::new(false));
        let install_dropped = Arc::new(AtomicBool::new(false));
        let connection_started = Arc::new(AtomicBool::new(false));
        let connection_dropped = Arc::new(AtomicBool::new(false));
        let closed_after_task = Arc::new(AtomicBool::new(false));
        let shutdown_after_task = Arc::new(AtomicBool::new(false));
        let build = complete_builder()
            .await
            .with_plugin(ScopedTaskPlugin {
                install_started: install_started.clone(),
                install_dropped: install_dropped.clone(),
                connection_started: connection_started.clone(),
                connection_dropped: connection_dropped.clone(),
                closed_after_task: closed_after_task.clone(),
                shutdown_after_task: shutdown_after_task.clone(),
            })
            .build()
            .await
            .expect("scoped task plugin");
        let client = build.into_client();
        wait_for_flag(&install_started).await;

        let scope = ConnectionScope::new(88);
        client
            .plugin_host
            .as_ref()
            .expect("plugin host")
            .on_ready(scope.clone())
            .await
            .expect("plugin ready callback");
        wait_for_flag(&connection_started).await;
        scope.cancel();
        client
            .plugin_host
            .as_ref()
            .expect("plugin host")
            .on_closed(scope)
            .await
            .expect("plugin closed callback");
        assert!(connection_dropped.load(Ordering::Acquire));
        assert!(closed_after_task.load(Ordering::Acquire));
        assert!(!install_dropped.load(Ordering::Acquire));

        client.disconnect().await;
        assert!(install_dropped.load(Ordering::Acquire));
        assert!(shutdown_after_task.load(Ordering::Acquire));
    }

    #[tokio::test]
    async fn connection_scoped_tasks_stop_when_the_generation_is_cancelled() {
        let scope = ConnectionScope::new(77);
        let task_dropped = Arc::new(AtomicBool::new(false));
        let tasks = PluginConnectionTasks {
            runtime: Arc::new(TokioRuntime),
            scope: scope.clone(),
            tracker: TaskTracker::new(),
        };
        let guard = DropFlag(task_dropped.clone());
        tasks
            .spawn(async move {
                let _guard = guard;
                futures::future::pending::<()>().await;
            })
            .expect("open connection scope");

        scope.cancel();
        tokio::time::timeout(Duration::from_secs(1), async {
            while !task_dropped.load(Ordering::Acquire) {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("connection cancellation stopped the scoped task");
        assert!(matches!(
            tasks.spawn(async {}),
            Err(PluginResourceError::ShuttingDown)
        ));
    }

    #[tokio::test]
    async fn task_sleeps_return_when_their_owner_is_cancelled() {
        let resources = PluginResources::new();
        resources.activate();
        let install_tasks = PluginTasks {
            runtime: Arc::new(TokioRuntime),
            resources: resources.clone(),
        };
        let install_sleeper =
            tokio::spawn(async move { install_tasks.sleep(Duration::from_secs(60)).await });
        tokio::task::yield_now().await;
        resources.close();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), install_sleeper)
                .await
                .expect("install sleep cancellation")
                .expect("install sleeper task"),
            Err(PluginResourceError::ShuttingDown)
        );

        let scope = ConnectionScope::new(91);
        let connection_tasks = PluginConnectionTasks {
            runtime: Arc::new(TokioRuntime),
            scope: scope.clone(),
            tracker: TaskTracker::new(),
        };
        let connection_sleeper =
            tokio::spawn(async move { connection_tasks.sleep(Duration::from_secs(60)).await });
        tokio::task::yield_now().await;
        scope.cancel();
        assert_eq!(
            tokio::time::timeout(Duration::from_secs(1), connection_sleeper)
                .await
                .expect("connection sleep cancellation")
                .expect("connection sleeper task"),
            Err(PluginResourceError::ShuttingDown)
        );
    }

    struct UpstreamLifecycle {
        log: Log,
    }

    impl ClientLifecycle for UpstreamLifecycle {
        fn install(&self, _client: Weak<Client>) -> BoxFuture<'_, anyhow::Result<()>> {
            let log = self.log.clone();
            Box::pin(async move {
                record(&log, "install:upstream");
                Ok(())
            })
        }

        fn shutdown(&self) -> BoxFuture<'_, anyhow::Result<()>> {
            let log = self.log.clone();
            Box::pin(async move {
                record(&log, "shutdown:upstream");
                Ok(())
            })
        }
    }

    struct FailingReadyLifecycle;

    impl ClientLifecycle for FailingReadyLifecycle {
        fn on_ready(&self, _scope: ConnectionScope) -> BoxFuture<'_, anyhow::Result<()>> {
            Box::pin(async { anyhow::bail!("injected upstream ready failure") })
        }
    }

    struct ReadyPlugin<const MARKER: u8> {
        id: &'static str,
        dependency: Option<&'static str>,
        log: Log,
        stalls: bool,
    }

    impl<const MARKER: u8> ClientPlugin for ReadyPlugin<MARKER> {
        type Api = ();

        fn manifest(&self) -> PluginManifest {
            let manifest = PluginManifest::new(self.id, "0.1.0");
            match self.dependency {
                Some(dependency) => manifest.with_dependency(dependency),
                None => manifest,
            }
        }

        fn install(
            &self,
            _context: PluginContext,
        ) -> BoxFuture<'_, anyhow::Result<Arc<Self::Api>>> {
            Box::pin(async { Ok(Arc::new(())) })
        }

        fn on_ready(&self, _scope: PluginConnectionScope) -> BoxFuture<'_, anyhow::Result<()>> {
            let id = self.id;
            let log = self.log.clone();
            let stalls = self.stalls;
            Box::pin(async move {
                record(&log, format!("ready:{id}"));
                if stalls {
                    futures::future::pending::<()>().await;
                }
                Ok(())
            })
        }
    }

    #[tokio::test]
    async fn upstream_ready_failure_does_not_suppress_plugins() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let client = complete_builder()
            .await
            .with_lifecycle(FailingReadyLifecycle)
            .with_plugin(ReadyPlugin::<1> {
                id: "ready-probe",
                dependency: None,
                log: log.clone(),
                stalls: false,
            })
            .build()
            .await
            .expect("ready probe client")
            .into_client();

        let result = client
            .plugin_host
            .as_ref()
            .expect("plugin host")
            .on_ready(ConnectionScope::new(91))
            .await;
        assert!(result.is_err());
        assert_eq!(
            *log.lock().unwrap_or_else(|poisoned| poisoned.into_inner()),
            vec!["ready:ready-probe"]
        );
        client.disconnect().await;
    }

    #[tokio::test]
    async fn timed_out_plugin_callback_does_not_suppress_following_plugins() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let plan = PluginPlan::prepare(vec![
            PluginRegistration::new(ReadyPlugin::<2> {
                id: "stalling-ready",
                dependency: None,
                log: log.clone(),
                stalls: true,
            }),
            PluginRegistration::new(ReadyPlugin::<3> {
                id: "following-ready",
                dependency: Some("stalling-ready"),
                log: log.clone(),
                stalls: false,
            }),
        ])
        .expect("valid callback plan")
        .expect("non-empty callback plan");
        let host = PluginHost::new_with_callback_timeout(plan, None, Duration::from_millis(10));
        let client = complete_builder()
            .await
            .with_lifecycle_arc(host.clone())
            .build()
            .await
            .expect("callback timeout client")
            .into_client();

        let result = host.on_ready(ConnectionScope::new(92)).await;
        assert!(result.is_err());
        assert_eq!(
            *log.lock().unwrap_or_else(|poisoned| poisoned.into_inner()),
            vec!["ready:stalling-ready", "ready:following-ready"]
        );
        client.disconnect().await;
    }

    #[tokio::test]
    async fn composes_existing_lifecycle_outside_plugin_lifo_order() {
        let log = Arc::new(Mutex::new(Vec::new()));
        let build = complete_builder()
            .await
            .with_lifecycle(UpstreamLifecycle { log: log.clone() })
            .with_plugin(FoundationPlugin { log: log.clone() })
            .build()
            .await
            .expect("composed lifecycle");
        let client = build.into_client();
        assert_eq!(
            *log.lock().unwrap_or_else(|poisoned| poisoned.into_inner()),
            vec!["install:upstream", "install:foundation"]
        );

        client.disconnect().await;
        assert_eq!(
            *log.lock().unwrap_or_else(|poisoned| poisoned.into_inner()),
            vec![
                "install:upstream",
                "install:foundation",
                "shutdown:foundation",
                "shutdown:upstream"
            ]
        );
    }

    struct NoopEventHandler;

    impl EventHandler for NoopEventHandler {
        fn handle_event(&self, _event: Arc<wacore::types::events::Event>) {}
    }

    struct EventSubscriptionPlugin;

    struct PanickingDropEventHandler;

    impl EventHandler for PanickingDropEventHandler {
        fn handle_event(&self, _event: Arc<wacore::types::events::Event>) {}
    }

    impl Drop for PanickingDropEventHandler {
        fn drop(&mut self) {
            panic!("injected event handler drop panic");
        }
    }

    struct PanickingSubscriptionPlugin;

    impl ClientPlugin for PanickingSubscriptionPlugin {
        type Api = ();

        fn manifest(&self) -> PluginManifest {
            PluginManifest::new("panicking-subscription", "0.1.0")
                .with_capability(PluginCapability::CoreEvents)
        }

        fn install(&self, context: PluginContext) -> BoxFuture<'_, anyhow::Result<Arc<Self::Api>>> {
            Box::pin(async move {
                context
                    .core_events()
                    .ok_or_else(|| anyhow::anyhow!("core events capability missing"))?
                    .subscribe(
                        EventInterest::of(&[EventKind::Connected]),
                        Arc::new(PanickingDropEventHandler),
                    )?;
                Ok(Arc::new(()))
            })
        }
    }

    struct ShutdownSignalPlugin;

    impl ClientPlugin for ShutdownSignalPlugin {
        type Api = ShutdownSignal;

        fn manifest(&self) -> PluginManifest {
            PluginManifest::new("shutdown-signal", "0.1.0").with_capability(PluginCapability::Tasks)
        }

        fn install(&self, context: PluginContext) -> BoxFuture<'_, anyhow::Result<Arc<Self::Api>>> {
            Box::pin(async move {
                context
                    .tasks()
                    .map(PluginTasks::shutdown_signal)
                    .map(Arc::new)
                    .ok_or_else(|| anyhow::anyhow!("tasks capability missing"))
            })
        }
    }

    struct ShutdownSignalLifecycle(Arc<AtomicBool>);

    impl ClientLifecycle for ShutdownSignalLifecycle {
        fn signal_shutdown(&self) {
            self.0.store(true, Ordering::Release);
        }
    }

    impl ClientPlugin for EventSubscriptionPlugin {
        type Api = ();

        fn manifest(&self) -> PluginManifest {
            PluginManifest::new("event-subscription", "0.1.0")
                .with_capability(PluginCapability::CoreEvents)
        }

        fn install(&self, context: PluginContext) -> BoxFuture<'_, anyhow::Result<Arc<Self::Api>>> {
            Box::pin(async move {
                context
                    .core_events()
                    .ok_or_else(|| anyhow::anyhow!("core events capability missing"))?
                    .subscribe(
                        EventInterest::of(&[EventKind::Connected, EventKind::RawNode]),
                        Arc::new(NoopEventHandler),
                    )?;
                Ok(Arc::new(()))
            })
        }
    }

    struct ReentrantSubscriptionHandler {
        events: PluginCoreEvents,
    }

    impl EventHandler for ReentrantSubscriptionHandler {
        fn handle_event(&self, _event: Arc<wacore::types::events::Event>) {}
    }

    impl Drop for ReentrantSubscriptionHandler {
        fn drop(&mut self) {
            let _ = self.events.subscribe(
                EventInterest::of(&[EventKind::Connected]),
                Arc::new(NoopEventHandler),
            );
        }
    }

    struct ReentrantSubscriptionPlugin;

    impl ClientPlugin for ReentrantSubscriptionPlugin {
        type Api = PluginCoreEvents;

        fn manifest(&self) -> PluginManifest {
            PluginManifest::new("reentrant-subscription", "0.1.0")
                .with_capability(PluginCapability::CoreEvents)
        }

        fn install(&self, context: PluginContext) -> BoxFuture<'_, anyhow::Result<Arc<Self::Api>>> {
            Box::pin(async move {
                let events = context
                    .core_events()
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("core events capability missing"))?;
                events.subscribe(
                    EventInterest::of(&[EventKind::Connected]),
                    Arc::new(ReentrantSubscriptionHandler {
                        events: events.clone(),
                    }),
                )?;
                Ok(Arc::new(events))
            })
        }
    }

    #[tokio::test]
    async fn shutdown_removes_plugin_event_subscriptions_and_raw_lease() {
        let build = complete_builder()
            .await
            .with_plugin(EventSubscriptionPlugin)
            .build()
            .await
            .expect("event subscription plugin");
        let client = build.into_client();
        assert!(
            client
                .core
                .event_bus
                .has_handler_for(wacore::types::events::EventKind::Connected)
        );
        assert!(client.raw_node_forwarding_enabled());

        client.disconnect().await;
        assert!(
            !client
                .core
                .event_bus
                .has_handler_for(wacore::types::events::EventKind::Connected)
        );
        assert!(!client.raw_node_forwarding_enabled());
    }

    #[tokio::test]
    async fn panicking_handler_drop_does_not_strand_later_plugins_or_upstream() {
        let upstream_signalled = Arc::new(AtomicBool::new(false));
        let client = complete_builder()
            .await
            .with_lifecycle(ShutdownSignalLifecycle(upstream_signalled.clone()))
            .with_plugin(ShutdownSignalPlugin)
            .with_plugin(PanickingSubscriptionPlugin)
            .build()
            .await
            .expect("panicking subscription client")
            .into_client();
        let plugin_shutdown = client
            .plugin::<ShutdownSignalPlugin>()
            .expect("shutdown signal API");

        let result = std::panic::catch_unwind(AssertUnwindSafe(|| client.signal_shutdown_sync()));

        assert!(result.is_ok());
        assert!(plugin_shutdown.is_fired());
        assert!(upstream_signalled.load(Ordering::Acquire));
        client.disconnect().await;
    }

    #[tokio::test]
    async fn resource_close_drops_reentrant_handlers_outside_the_subscription_lock() {
        let client = complete_builder()
            .await
            .with_plugin(ReentrantSubscriptionPlugin)
            .build()
            .await
            .expect("reentrant subscription plugin")
            .into_client();
        let shutdown_client = client.clone();
        let (completed_tx, completed_rx) = std::sync::mpsc::sync_channel(1);
        let shutdown = std::thread::spawn(move || {
            shutdown_client.signal_shutdown_sync();
            let _ = completed_tx.send(());
        });

        completed_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("reentrant handler teardown must not deadlock");
        shutdown.join().expect("shutdown thread");
        client.disconnect().await;
    }

    #[tokio::test]
    async fn rejected_subscription_drops_reentrant_handler_outside_the_subscription_lock() {
        let client = complete_builder()
            .await
            .with_plugin(ReentrantSubscriptionPlugin)
            .build()
            .await
            .expect("reentrant subscription plugin")
            .into_client();
        let events = client
            .plugin::<ReentrantSubscriptionPlugin>()
            .expect("plugin event API");
        client.signal_shutdown_sync();

        let (completed_tx, completed_rx) = std::sync::mpsc::sync_channel(1);
        let subscribe_events = events.clone();
        let subscribe = std::thread::spawn(move || {
            let result = subscribe_events.subscribe(
                EventInterest::of(&[EventKind::Connected]),
                Arc::new(ReentrantSubscriptionHandler {
                    events: (*subscribe_events).clone(),
                }),
            );
            let _ = completed_tx.send(result);
        });

        let result = completed_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("rejected reentrant subscription must not deadlock");
        assert!(matches!(result, Err(PluginResourceError::ShuttingDown)));
        subscribe.join().expect("subscription thread");
        client.disconnect().await;
    }

    #[tokio::test]
    async fn synchronous_shutdown_closes_plugin_resources_with_live_client_refs() {
        let task_dropped = Arc::new(AtomicBool::new(false));
        let client = complete_builder()
            .await
            .with_plugin(RollbackPlugin {
                log: Arc::new(Mutex::new(Vec::new())),
                task_dropped: task_dropped.clone(),
                api_dropped: Arc::new(AtomicBool::new(false)),
            })
            .with_plugin(EventSubscriptionPlugin)
            .build()
            .await
            .expect("plugin resource client")
            .into_client();
        let retained_client = client.clone();

        client.signal_shutdown_sync();
        wait_for_flag(&task_dropped).await;

        assert!(!retained_client.raw_node_forwarding_enabled());
        assert!(
            !retained_client
                .core
                .event_bus
                .has_handler_for(EventKind::Connected)
        );
        retained_client.disconnect().await;
    }

    struct CapabilityProbe;

    impl ClientPlugin for CapabilityProbe {
        type Api = [bool; 5];

        fn manifest(&self) -> PluginManifest {
            PluginManifest::new("capability-probe", "0.1.0")
                .with_capability(PluginCapability::Messaging)
        }

        fn install(&self, context: PluginContext) -> BoxFuture<'_, anyhow::Result<Arc<Self::Api>>> {
            Box::pin(async move {
                Ok(Arc::new([
                    context.core_events().is_some(),
                    context.tasks().is_some(),
                    context.messaging().is_some(),
                    context.iq().is_some(),
                    context.plugin_events().is_some(),
                ]))
            })
        }
    }

    #[tokio::test]
    async fn context_exposes_only_declared_capabilities() {
        let build = complete_builder()
            .await
            .with_plugin(CapabilityProbe)
            .build()
            .await
            .expect("capability plugin");
        let client = build.into_client();
        assert_eq!(
            client.plugin::<CapabilityProbe>().as_deref(),
            Some(&[false, false, true, false, false])
        );
        assert!(client.plugin_event_router().is_none());
        client.disconnect().await;
    }

    struct PluginEventPublisher;

    impl ClientPlugin for PluginEventPublisher {
        type Api = PluginEvents;

        fn manifest(&self) -> PluginManifest {
            PluginManifest::new("event-publisher", "0.1.0")
                .with_capability(PluginCapability::PluginEvents)
        }

        fn install(&self, context: PluginContext) -> BoxFuture<'_, anyhow::Result<Arc<Self::Api>>> {
            Box::pin(async move {
                context
                    .plugin_events()
                    .cloned()
                    .map(Arc::new)
                    .ok_or_else(|| anyhow::anyhow!("plugin events capability missing"))
            })
        }
    }

    #[tokio::test]
    async fn typed_plugin_api_publishes_only_to_exact_bounded_routes() {
        let client = complete_builder()
            .await
            .with_plugin(PluginEventPublisher)
            .with_plugin(CapabilityProbe)
            .build()
            .await
            .expect("plugin event publisher")
            .into_client();
        let publisher = client
            .plugin::<PluginEventPublisher>()
            .expect("typed publisher API");
        let router = client.plugin_event_router().expect("plugin event router");
        let tick = PluginEventTopic::new("tick").expect("valid topic");
        let silent_selector =
            PluginEventSelector::new("capability-probe", tick.clone()).expect("valid selector");
        assert!(matches!(
            router.subscribe(
                [silent_selector],
                PluginEventEndpointConfig::new(1, PluginEventOverflow::DropNewest),
            ),
            Err(PluginEventSubscribeError::UnknownPublisher { .. })
        ));
        let selector = publisher.selector(&tick);
        let subscription = router
            .subscribe(
                [selector.clone()],
                PluginEventEndpointConfig::new(1, PluginEventOverflow::DropNewest),
            )
            .expect("bounded event endpoint");

        assert!(publisher.has_subscribers(&tick));
        let generation = client.connection_generation.load(Ordering::Acquire);
        assert_eq!(
            publisher
                .publish(
                    &tick,
                    2,
                    PluginEventPayloadEncoding::Json,
                    r#"{"messages":1}"#,
                )
                .expect("publish tick"),
            PluginEventPublishReport {
                matched: 1,
                enqueued: 1,
                dropped: 0,
                closed: 0,
            }
        );
        let event = subscription.recv().await.expect("routed tick");
        assert_eq!(&*event.plugin_id, "event-publisher");
        assert_eq!(event.topic, tick);
        assert_eq!(event.schema_version, 2);
        assert_eq!(event.payload_encoding, PluginEventPayloadEncoding::Json);
        assert_eq!(event.payload, Bytes::from_static(br#"{"messages":1}"#));
        assert_eq!(event.connection_generation, generation);
        assert_eq!(event.sequence, 1);

        let next_generation = client.connection_generation.fetch_add(1, Ordering::SeqCst) + 1;
        publisher
            .publish(&tick, 2, PluginEventPayloadEncoding::Json, Bytes::new())
            .expect("publish after generation change");
        let event = subscription.recv().await.expect("next generation tick");
        assert_eq!(event.connection_generation, next_generation);
        assert_eq!(event.sequence, 2);

        client.disconnect().await;
        assert!(matches!(
            publisher.publish(&tick, 2, PluginEventPayloadEncoding::Json, Bytes::new(),),
            Err(PluginEventPublishError::Resource(
                PluginResourceError::ShuttingDown
            ))
        ));
        assert!(matches!(
            subscription.recv().await,
            Err(PluginEventReceiveError)
        ));
        assert!(matches!(
            router.subscribe(
                [selector],
                PluginEventEndpointConfig::new(1, PluginEventOverflow::DropNewest),
            ),
            Err(PluginEventSubscribeError::Closed)
        ));
    }
}

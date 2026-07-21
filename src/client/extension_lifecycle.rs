use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Weak};

use super::Client;
use wacore::runtime::{BoxFuture, ShutdownNotifier, ShutdownSignal};

const SCOPE_OPEN: u8 = 0;
const SCOPE_READY: u8 = 1;
const SCOPE_CANCELLED: u8 = 2;
const SCOPE_CLOSED: u8 = 3;

/// Observable state of one authenticated connection generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ConnectionScopeState {
    Open,
    Ready,
    Cancelled,
    Closed,
}

struct ConnectionScopeInner {
    generation: u64,
    state: AtomicU8,
    cancellation: ShutdownNotifier,
}

/// Stable handle for work owned by one authenticated connection generation.
///
/// A scope is cancelled synchronously when its generation is retired and is
/// marked closed only after the client's authoritative connection cleanup.
#[derive(Clone)]
pub struct ConnectionScope {
    inner: Arc<ConnectionScopeInner>,
}

impl ConnectionScope {
    fn new(generation: u64) -> Self {
        Self {
            inner: Arc::new(ConnectionScopeInner {
                generation,
                state: AtomicU8::new(SCOPE_OPEN),
                cancellation: ShutdownNotifier::new(),
            }),
        }
    }

    pub fn generation(&self) -> u64 {
        self.inner.generation
    }

    pub fn state(&self) -> ConnectionScopeState {
        match self.inner.state.load(Ordering::Acquire) {
            SCOPE_OPEN => ConnectionScopeState::Open,
            SCOPE_READY => ConnectionScopeState::Ready,
            SCOPE_CANCELLED => ConnectionScopeState::Cancelled,
            _ => ConnectionScopeState::Closed,
        }
    }

    pub fn cancellation_signal(&self) -> ShutdownSignal {
        self.inner.cancellation.subscribe()
    }

    pub fn is_cancelled(&self) -> bool {
        self.inner.state.load(Ordering::Acquire) >= SCOPE_CANCELLED
    }

    fn mark_ready(&self) -> bool {
        self.inner
            .state
            .compare_exchange(SCOPE_OPEN, SCOPE_READY, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    fn cancel(&self) {
        let mut state = self.inner.state.load(Ordering::Acquire);
        while state < SCOPE_CANCELLED {
            match self.inner.state.compare_exchange_weak(
                state,
                SCOPE_CANCELLED,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => {
                    self.inner.cancellation.notify();
                    return;
                }
                Err(actual) => state = actual,
            }
        }
    }

    fn close(&self) {
        let previous = self.inner.state.swap(SCOPE_CLOSED, Ordering::AcqRel);
        if previous < SCOPE_CANCELLED {
            self.inner.cancellation.notify();
        }
    }
}

impl fmt::Debug for ConnectionScope {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ConnectionScope")
            .field("generation", &self.generation())
            .field("state", &self.state())
            .finish()
    }
}

/// Aggregate lifecycle seam installed during [`Client`](super::Client) construction.
///
/// Implementations must make `install` transactional. The remaining callbacks
/// are serialized, awaited, and must not block indefinitely. A future plugin
/// host owns per-plugin ordering, timeout, and error isolation behind this one
/// client-level seam. `install` receives a weak client reference so retaining
/// the handle cannot create a client-to-extension reference cycle.
pub trait ClientLifecycle: wacore::sync_marker::MaybeSendSync {
    fn install<'a>(&'a self, _client: Weak<Client>) -> BoxFuture<'a, anyhow::Result<()>> {
        Box::pin(async { Ok(()) })
    }

    fn on_ready<'a>(&'a self, _scope: ConnectionScope) -> BoxFuture<'a, anyhow::Result<()>> {
        Box::pin(async { Ok(()) })
    }

    fn on_closed<'a>(&'a self, _scope: ConnectionScope) -> BoxFuture<'a, anyhow::Result<()>> {
        Box::pin(async { Ok(()) })
    }

    fn shutdown(&self) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async { Ok(()) })
    }
}

pub(super) struct LifecycleRegistration {
    handler: Arc<dyn ClientLifecycle>,
    active_scope: std::sync::Mutex<Option<ConnectionScope>>,
    callback_gate: async_lock::Mutex<()>,
    shutdown_gate: async_lock::Mutex<bool>,
    scope_changed: event_listener::Event,
    terminal: AtomicBool,
}

impl LifecycleRegistration {
    pub(super) fn new(handler: Arc<dyn ClientLifecycle>) -> Self {
        Self {
            handler,
            active_scope: std::sync::Mutex::new(None),
            callback_gate: async_lock::Mutex::new(()),
            shutdown_gate: async_lock::Mutex::new(false),
            scope_changed: event_listener::Event::new(),
            terminal: AtomicBool::new(false),
        }
    }

    pub(super) async fn install(&self, client: Weak<Client>) -> anyhow::Result<()> {
        self.handler.install(client).await
    }

    pub(super) fn begin_scope(&self, generation: u64) {
        if self.terminal.load(Ordering::Acquire) {
            return;
        }

        let scope = ConnectionScope::new(generation);
        let mut active = self
            .active_scope
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if self.terminal.load(Ordering::Acquire) {
            return;
        }
        let replaced = active.replace(scope);
        drop(active);
        if let Some(replaced) = replaced {
            log::warn!(
                "Replacing unclosed connection scope for generation {}",
                replaced.generation()
            );
            replaced.cancel();
            replaced.close();
        }
    }

    pub(super) async fn ready(&self, generation: u64) -> bool {
        let _callback_guard = self.callback_gate.lock().await;
        if self.terminal.load(Ordering::Acquire) {
            return false;
        }
        let scope = self.scope_for(generation);
        let Some(scope) = scope.filter(ConnectionScope::mark_ready) else {
            return false;
        };

        if let Err(error) = self.handler.on_ready(scope.clone()).await {
            log::warn!("Client lifecycle on_ready failed: {error:#}");
        }
        !scope.is_cancelled()
    }

    pub(super) fn cancel_scope(&self, generation: u64) {
        if let Some(scope) = self.scope_for(generation) {
            scope.cancel();
        }
    }

    pub(super) fn cancel_active_scope(&self) {
        let scope = self
            .active_scope
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone();
        if let Some(scope) = scope {
            scope.cancel();
        }
    }

    pub(super) async fn close_scope(&self, generation: u64) {
        let _callback_guard = self.callback_gate.lock().await;
        let scope = {
            let mut active = self
                .active_scope
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            if active
                .as_ref()
                .is_some_and(|scope| scope.generation() == generation)
            {
                active.take()
            } else {
                None
            }
        };
        let Some(scope) = scope else {
            return;
        };

        self.scope_changed.notify(usize::MAX);
        scope.close();
        if let Err(error) = self.handler.on_closed(scope).await {
            log::warn!("Client lifecycle on_closed failed: {error:#}");
        }
    }

    pub(super) async fn shutdown(&self) {
        self.terminal.store(true, Ordering::Release);
        self.cancel_active_scope();

        let mut started = self.shutdown_gate.lock().await;
        if *started {
            return;
        }
        *started = true;

        loop {
            let scope_changed = self.scope_changed.listen();
            let callback_guard = self.callback_gate.lock().await;
            if self.scope_for_active_generation().is_none() {
                if let Err(error) = self.handler.shutdown().await {
                    log::warn!("Client lifecycle shutdown failed: {error:#}");
                }
                return;
            }
            drop(callback_guard);
            scope_changed.await;
        }
    }

    fn scope_for_active_generation(&self) -> Option<ConnectionScope> {
        self.active_scope
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    fn scope_for(&self, generation: u64) -> Option<ConnectionScope> {
        self.active_scope
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            .filter(|scope| scope.generation() == generation)
            .cloned()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;

    use async_trait::async_trait;
    use bytes::Bytes;

    use super::*;
    use crate::runtime_impl::TokioRuntime;
    use crate::store::persistence_manager::PersistenceManager;
    use crate::test_utils::MockHttpClient;
    use crate::transport::mock::MockTransportFactory;

    #[derive(Default)]
    struct RecordingLifecycle {
        events: std::sync::Mutex<Vec<String>>,
        scopes: std::sync::Mutex<Vec<ConnectionScope>>,
        shutdowns: AtomicUsize,
    }

    impl RecordingLifecycle {
        fn events(&self) -> Vec<String> {
            self.events
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
                .clone()
        }
    }

    impl ClientLifecycle for RecordingLifecycle {
        fn install<'a>(&'a self, client: Weak<Client>) -> BoxFuture<'a, anyhow::Result<()>> {
            Box::pin(async move {
                assert!(client.upgrade().is_some());
                self.events
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .push("install".to_string());
                Ok(())
            })
        }

        fn on_ready<'a>(&'a self, scope: ConnectionScope) -> BoxFuture<'a, anyhow::Result<()>> {
            Box::pin(async move {
                self.events
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .push(format!("ready:{}", scope.generation()));
                self.scopes
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .push(scope);
                Ok(())
            })
        }

        fn on_closed<'a>(&'a self, scope: ConnectionScope) -> BoxFuture<'a, anyhow::Result<()>> {
            Box::pin(async move {
                self.events
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .push(format!("closed:{}", scope.generation()));
                Ok(())
            })
        }

        fn shutdown(&self) -> BoxFuture<'_, anyhow::Result<()>> {
            Box::pin(async move {
                self.shutdowns.fetch_add(1, Ordering::SeqCst);
                self.events
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .push("shutdown".to_string());
                Ok(())
            })
        }
    }

    struct BlockingDisconnect {
        started: async_channel::Sender<()>,
        release: async_channel::Receiver<()>,
    }

    #[async_trait]
    impl crate::transport::Transport for BlockingDisconnect {
        async fn send(&self, _data: Bytes) -> anyhow::Result<()> {
            Ok(())
        }

        async fn disconnect(&self) {
            let _ = self.started.try_send(());
            let _ = self.release.recv().await;
        }
    }

    struct BlockingReadyLifecycle {
        ready_started: async_channel::Sender<()>,
        release_ready: async_channel::Receiver<()>,
        scope: std::sync::Mutex<Option<ConnectionScope>>,
        events: std::sync::Mutex<Vec<&'static str>>,
    }

    struct LogoutOrderHandler {
        lifecycle: Arc<RecordingLifecycle>,
    }

    impl wacore::types::events::EventHandler for LogoutOrderHandler {
        fn handle_event(&self, event: Arc<wacore::types::events::Event>) {
            if matches!(&*event, wacore::types::events::Event::LoggedOut(_)) {
                self.lifecycle
                    .events
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .push("logged-out".to_string());
            }
        }

        fn interest(&self) -> wacore::types::events::EventInterest {
            wacore::types::events::EventInterest::of(&[wacore::types::events::EventKind::LoggedOut])
        }
    }

    impl ClientLifecycle for BlockingReadyLifecycle {
        fn on_ready<'a>(&'a self, scope: ConnectionScope) -> BoxFuture<'a, anyhow::Result<()>> {
            Box::pin(async move {
                *self
                    .scope
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(scope);
                self.events
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .push("ready-started");
                let _ = self.ready_started.try_send(());
                let _ = self.release_ready.recv().await;
                self.events
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .push("ready-finished");
                Ok(())
            })
        }

        fn on_closed<'a>(&'a self, _scope: ConnectionScope) -> BoxFuture<'a, anyhow::Result<()>> {
            Box::pin(async move {
                self.events
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .push("closed");
                Ok(())
            })
        }
    }

    #[test]
    fn scope_state_machine_is_sticky_and_cancellable() {
        let scope = ConnectionScope::new(41);
        let cancellation = scope.cancellation_signal();

        assert_eq!(scope.state(), ConnectionScopeState::Open);
        assert!(scope.mark_ready());
        assert_eq!(scope.state(), ConnectionScopeState::Ready);
        scope.cancel();
        assert_eq!(scope.state(), ConnectionScopeState::Cancelled);
        assert!(cancellation.is_fired());
        scope.cancel();
        scope.close();
        assert_eq!(scope.state(), ConnectionScopeState::Closed);
    }

    #[tokio::test]
    async fn cleanup_cancels_before_io_and_closes_after_authoritative_teardown() {
        let persistence_manager = Arc::new(
            PersistenceManager::new(crate::test_utils::create_test_backend().await)
                .await
                .expect("persistence manager"),
        );
        let lifecycle = Arc::new(RecordingLifecycle::default());
        let build = Client::builder()
            .with_runtime(TokioRuntime)
            .with_persistence_manager(persistence_manager)
            .with_transport_factory(MockTransportFactory::new())
            .with_http_client(MockHttpClient)
            .with_lifecycle_arc(lifecycle.clone())
            .build()
            .await
            .expect("client build");
        let client = build.client();
        const GENERATION: u64 = 9;
        client
            .connection_generation
            .store(GENERATION, Ordering::SeqCst);
        let registration = client.lifecycle.as_ref().expect("lifecycle registration");
        registration.begin_scope(GENERATION);
        client.dispatch_connected().await;

        let scope = lifecycle
            .scopes
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .first()
            .cloned()
            .expect("ready scope");
        let cancelled = scope.cancellation_signal();
        let (started_tx, started_rx) = async_channel::bounded(1);
        let (release_tx, release_rx) = async_channel::bounded(1);
        *client.transport.lock().await = Some(Arc::new(BlockingDisconnect {
            started: started_tx,
            release: release_rx,
        }));

        let cleanup_client = Arc::clone(&client);
        let cleanup = tokio::spawn(async move {
            cleanup_client.cleanup_connection_state().await;
        });
        tokio::time::timeout(std::time::Duration::from_secs(2), started_rx.recv())
            .await
            .expect("transport cleanup started")
            .expect("transport remained alive");

        assert_eq!(scope.state(), ConnectionScopeState::Cancelled);
        assert!(cancelled.is_fired());
        assert_eq!(lifecycle.events(), vec!["install", "ready:9"]);

        release_tx.send(()).await.expect("release cleanup");
        tokio::time::timeout(std::time::Duration::from_secs(2), cleanup)
            .await
            .expect("cleanup completed")
            .expect("cleanup did not panic");

        assert_eq!(scope.state(), ConnectionScopeState::Closed);
        assert_eq!(lifecycle.events(), vec!["install", "ready:9", "closed:9"]);
        client.shutdown_lifecycle().await;
        client.shutdown_lifecycle().await;
        assert_eq!(lifecycle.shutdowns.load(Ordering::SeqCst), 1);
        assert_eq!(
            lifecycle.events(),
            vec!["install", "ready:9", "closed:9", "shutdown"]
        );
        client.signal_shutdown_sync();
    }

    #[tokio::test]
    async fn cancellation_does_not_wait_for_a_running_callback() {
        let persistence_manager = Arc::new(
            PersistenceManager::new(crate::test_utils::create_test_backend().await)
                .await
                .expect("persistence manager"),
        );
        let (ready_started_tx, ready_started_rx) = async_channel::bounded(1);
        let (release_ready_tx, release_ready_rx) = async_channel::bounded(1);
        let lifecycle = Arc::new(BlockingReadyLifecycle {
            ready_started: ready_started_tx,
            release_ready: release_ready_rx,
            scope: std::sync::Mutex::new(None),
            events: std::sync::Mutex::new(Vec::new()),
        });
        let client = Client::builder()
            .with_runtime(TokioRuntime)
            .with_persistence_manager(persistence_manager)
            .with_transport_factory(MockTransportFactory::new())
            .with_http_client(MockHttpClient)
            .with_lifecycle_arc(lifecycle.clone())
            .build()
            .await
            .expect("client build")
            .client();
        const GENERATION: u64 = 13;
        client
            .connection_generation
            .store(GENERATION, Ordering::SeqCst);
        client
            .lifecycle
            .as_ref()
            .expect("lifecycle registration")
            .begin_scope(GENERATION);

        let ready_client = Arc::clone(&client);
        let ready_task = tokio::spawn(async move {
            ready_client.dispatch_connected().await;
        });
        ready_started_rx
            .recv()
            .await
            .expect("ready callback started");
        let scope = lifecycle
            .scope
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
            .expect("ready scope");

        let cleanup_client = Arc::clone(&client);
        let cleanup_task = tokio::spawn(async move {
            cleanup_client.cleanup_connection_state().await;
        });
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while !scope.is_cancelled() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("scope cancellation");
        assert!(!cleanup_task.is_finished());

        release_ready_tx.send(()).await.expect("release ready hook");
        ready_task.await.expect("ready task did not panic");
        cleanup_task.await.expect("cleanup task did not panic");
        assert_eq!(scope.state(), ConnectionScopeState::Closed);
        assert_eq!(
            *lifecycle
                .events
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
            vec!["ready-started", "ready-finished", "closed"]
        );
        client.signal_shutdown_sync();
    }

    #[tokio::test]
    async fn shutdown_waits_for_the_active_scope_and_is_terminal() {
        let lifecycle = Arc::new(RecordingLifecycle::default());
        let registration = Arc::new(LifecycleRegistration::new(lifecycle.clone()));
        const GENERATION: u64 = 21;
        registration.begin_scope(GENERATION);
        let scope = registration
            .scope_for(GENERATION)
            .expect("active connection scope");

        let shutdown_registration = Arc::clone(&registration);
        let shutdown = tokio::spawn(async move {
            shutdown_registration.shutdown().await;
        });
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while !scope.is_cancelled() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("scope cancellation");
        assert!(!shutdown.is_finished());

        registration.close_scope(GENERATION).await;
        shutdown.await.expect("shutdown did not panic");
        assert_eq!(
            lifecycle.events(),
            vec!["closed:21".to_string(), "shutdown".to_string()]
        );

        registration.begin_scope(GENERATION + 1);
        assert!(registration.scope_for(GENERATION + 1).is_none());
        assert!(!registration.ready(GENERATION + 1).await);
        registration.shutdown().await;
        assert_eq!(lifecycle.shutdowns.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn logout_event_precedes_terminal_lifecycle_shutdown() {
        let persistence_manager = Arc::new(
            PersistenceManager::new(crate::test_utils::create_test_backend().await)
                .await
                .expect("persistence manager"),
        );
        let lifecycle = Arc::new(RecordingLifecycle::default());
        let client = Client::builder()
            .with_runtime(TokioRuntime)
            .with_persistence_manager(persistence_manager)
            .with_transport_factory(MockTransportFactory::new())
            .with_http_client(MockHttpClient)
            .with_lifecycle_arc(lifecycle.clone())
            .build()
            .await
            .expect("client build")
            .client();
        client.register_handler(Arc::new(LogoutOrderHandler {
            lifecycle: lifecycle.clone(),
        }));

        client.logout().await.expect("client logout");

        assert_eq!(
            lifecycle.events(),
            vec!["install", "logged-out", "shutdown"]
        );
    }
}

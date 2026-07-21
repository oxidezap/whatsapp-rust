use std::cell::Cell;
use std::collections::VecDeque;
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Weak};
use std::time::Duration;

use super::Client;
use futures::FutureExt;
use wacore::runtime::{
    BoxFuture, Runtime, ShutdownNotifier, ShutdownSignal, timeout as rt_timeout,
};

const SCOPE_OPEN: u8 = 0;
const SCOPE_READY: u8 = 1;
const SCOPE_CANCELLED: u8 = 2;
const SCOPE_CLOSED: u8 = 3;
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(5);

std::thread_local! {
    static ACTIVE_CALLBACK: Cell<*const LifecycleRegistration> = const { Cell::new(std::ptr::null()) };
}

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

    /// Fires when this scope stops owning connection work, whether it is
    /// cancelled during retirement or reaches final closure after cleanup.
    pub fn cancellation_signal(&self) -> ShutdownSignal {
        self.inner.cancellation.subscribe()
    }

    /// Returns `true` after either cancellation or final closure.
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
/// Implementations must make `install` transactional. Connection callbacks are
/// serialized and bounded; connection cleanup only schedules `on_closed` so a
/// stalled extension cannot block reconnect. A future plugin host owns
/// per-plugin ordering and isolation behind this client-level seam. `install`
/// receives a weak client reference so retaining it cannot create a cycle.
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
    runtime: Arc<dyn Runtime>,
    scopes: std::sync::Mutex<ScopeRegistry>,
    callback_queue: std::sync::Mutex<CallbackQueue>,
    shutdown_complete: AtomicBool,
    shutdown_notifier: ShutdownNotifier,
    callback_timeout: Duration,
    terminal: AtomicBool,
}

#[derive(Default)]
struct ScopeRegistry {
    active: Option<ConnectionScope>,
    retired: Vec<ConnectionScope>,
}

enum LifecycleCallback {
    Ready {
        scope: ConnectionScope,
        done: async_channel::Sender<bool>,
    },
    Closed(ConnectionScope),
    Shutdown,
}

#[derive(Default)]
struct CallbackQueue {
    pending: VecDeque<LifecycleCallback>,
    shutdown_requested: bool,
    shutdown_enqueued: bool,
    drain_scheduled: bool,
}

struct CallbackContextGuard {
    previous: *const LifecycleRegistration,
}

impl CallbackContextGuard {
    fn enter(registration: &LifecycleRegistration) -> Self {
        let previous = ACTIVE_CALLBACK.replace(registration);
        Self { previous }
    }
}

impl Drop for CallbackContextGuard {
    fn drop(&mut self) {
        ACTIVE_CALLBACK.set(self.previous);
    }
}

fn callback_context_active(registration: &LifecycleRegistration) -> bool {
    ACTIVE_CALLBACK.with(|active| std::ptr::eq(active.get(), registration))
}

impl LifecycleRegistration {
    pub(super) fn new(handler: Arc<dyn ClientLifecycle>, runtime: Arc<dyn Runtime>) -> Self {
        Self::new_with_timeout(handler, runtime, CALLBACK_TIMEOUT)
    }

    fn new_with_timeout(
        handler: Arc<dyn ClientLifecycle>,
        runtime: Arc<dyn Runtime>,
        callback_timeout: Duration,
    ) -> Self {
        Self {
            handler,
            runtime,
            scopes: std::sync::Mutex::new(ScopeRegistry::default()),
            callback_queue: std::sync::Mutex::new(CallbackQueue::default()),
            shutdown_complete: AtomicBool::new(false),
            shutdown_notifier: ShutdownNotifier::new(),
            callback_timeout,
            terminal: AtomicBool::new(false),
        }
    }

    pub(super) async fn install(&self, client: Weak<Client>) -> anyhow::Result<()> {
        self.handler.install(client).await
    }

    pub(super) fn begin_scope_if_current(
        &self,
        generation: u64,
        is_current: impl FnOnce() -> bool,
    ) -> bool {
        if self.terminal.load(Ordering::Acquire) {
            return false;
        }

        let scope = ConnectionScope::new(generation);
        let mut scopes = self.scopes();
        if self.terminal.load(Ordering::Acquire) || !is_current() {
            return false;
        }
        let replaced = scopes.active.replace(scope);
        if let Some(replaced) = replaced {
            log::warn!(
                "Replacing unclosed connection scope for generation {}",
                replaced.generation()
            );
            replaced.cancel();
            scopes.retired.push(replaced);
        }
        true
    }

    pub(super) async fn ready(self: &Arc<Self>, generation: u64) -> bool {
        if self.terminal.load(Ordering::Acquire) {
            return false;
        }
        let (done_tx, done_rx) = async_channel::bounded(1);
        let scope = {
            let scopes = self.scopes();
            let scope = scopes
                .active
                .as_ref()
                .filter(|scope| scope.generation() == generation)
                .or_else(|| {
                    scopes
                        .retired
                        .iter()
                        .find(|scope| scope.generation() == generation)
                })
                .cloned();
            let Some(scope) = scope.filter(ConnectionScope::mark_ready) else {
                return false;
            };
            self.enqueue_callback(LifecycleCallback::Ready {
                scope: scope.clone(),
                done: done_tx,
            });
            scope
        };

        done_rx.recv().await.unwrap_or(false) && !scope.is_cancelled()
    }

    pub(super) fn cancel_scope(&self, generation: u64) {
        if let Some(scope) = self.scope_for(generation) {
            scope.cancel();
        }
    }

    pub(super) fn cancel_active_scope(&self) {
        let scopes = self.scopes();
        if let Some(scope) = &scopes.active {
            scope.cancel();
        }
        for scope in &scopes.retired {
            scope.cancel();
        }
    }

    pub(super) fn close_scope(self: &Arc<Self>, generation: u64) {
        self.close_scope_with(generation, || {});
    }

    /// Non-noop hooks are test-only, must run off-executor, and must not re-enter lifecycle APIs.
    fn close_scope_with(self: &Arc<Self>, generation: u64, after_remove: impl FnOnce()) {
        let should_spawn = {
            let mut scopes = self.scopes();
            let scope = if scopes
                .active
                .as_ref()
                .is_some_and(|scope| scope.generation() == generation)
            {
                scopes.active.take()
            } else {
                scopes
                    .retired
                    .iter()
                    .position(|scope| scope.generation() == generation)
                    .map(|position| scopes.retired.remove(position))
            };
            let Some(scope) = scope else {
                return;
            };

            scope.close();
            after_remove();

            // Publish closure before exposing an empty registry so terminal
            // shutdown cannot overtake the final on_closed callback.
            let no_open_scopes = scopes.active.is_none() && scopes.retired.is_empty();
            let mut queue = self.callback_queue();
            queue.pending.push_back(LifecycleCallback::Closed(scope));
            if no_open_scopes && queue.shutdown_requested && !queue.shutdown_enqueued {
                queue.shutdown_enqueued = true;
                queue.pending.push_back(LifecycleCallback::Shutdown);
            }
            if queue.drain_scheduled {
                false
            } else {
                queue.drain_scheduled = true;
                true
            }
        };

        self.spawn_callback_driver(should_spawn);
    }

    pub(super) async fn shutdown(self: &Arc<Self>) {
        self.terminal.store(true, Ordering::Release);
        self.cancel_active_scope();
        {
            let mut queue = self.callback_queue();
            queue.shutdown_requested = true;
        }
        self.enqueue_shutdown_if_ready();

        if self.shutdown_complete.load(Ordering::Acquire) || callback_context_active(self) {
            return;
        }

        let completed = self.shutdown_notifier.subscribe();
        if self.shutdown_complete.load(Ordering::Acquire) {
            return;
        }
        wacore::runtime::wait_for_shutdown(&completed).await;
    }

    fn enqueue_callback(self: &Arc<Self>, callback: LifecycleCallback) {
        let should_spawn = {
            let mut queue = self.callback_queue();
            queue.pending.push_back(callback);
            if queue.drain_scheduled {
                false
            } else {
                queue.drain_scheduled = true;
                true
            }
        };
        self.spawn_callback_driver(should_spawn);
    }

    fn enqueue_shutdown_if_ready(self: &Arc<Self>) {
        let no_open_scopes = {
            let scopes = self.scopes();
            scopes.active.is_none() && scopes.retired.is_empty()
        };
        if !no_open_scopes {
            return;
        }

        let should_spawn = {
            let mut queue = self.callback_queue();
            if !queue.shutdown_requested || queue.shutdown_enqueued {
                return;
            }
            queue.shutdown_enqueued = true;
            queue.pending.push_back(LifecycleCallback::Shutdown);
            if queue.drain_scheduled {
                false
            } else {
                queue.drain_scheduled = true;
                true
            }
        };
        self.spawn_callback_driver(should_spawn);
    }

    fn spawn_callback_driver(self: &Arc<Self>, should_spawn: bool) {
        if !should_spawn {
            return;
        }
        let registration = Arc::clone(self);
        self.runtime
            .spawn(Box::pin(async move {
                registration.drive_callbacks().await;
            }))
            .detach();
    }

    async fn drive_callbacks(self: Arc<Self>) {
        loop {
            let callback = {
                let mut queue = self.callback_queue();
                match queue.pending.pop_front() {
                    Some(callback) => callback,
                    None => {
                        queue.drain_scheduled = false;
                        return;
                    }
                }
            };

            match callback {
                LifecycleCallback::Ready { scope, done } => {
                    let callback_scope = scope.clone();
                    self.run_callback("on_ready", move |handler| handler.on_ready(callback_scope))
                        .await;
                    let _ = done.try_send(!scope.is_cancelled());
                }
                LifecycleCallback::Closed(scope) => {
                    self.run_callback("on_closed", move |handler| handler.on_closed(scope))
                        .await;
                }
                LifecycleCallback::Shutdown => {
                    self.run_callback("shutdown", |handler| handler.shutdown())
                        .await;
                    self.shutdown_complete.store(true, Ordering::Release);
                    self.shutdown_notifier.notify();
                }
            }
        }
    }

    async fn run_callback<'a>(
        &'a self,
        name: &'static str,
        create: impl FnOnce(&'a dyn ClientLifecycle) -> BoxFuture<'a, anyhow::Result<()>>,
    ) {
        let callback = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _callback_context = CallbackContextGuard::enter(self);
            create(&*self.handler)
        }));
        let Ok(mut callback) = callback else {
            log::warn!("Client lifecycle {name} panicked");
            return;
        };
        let callback = std::future::poll_fn(|context| {
            let _callback_context = CallbackContextGuard::enter(self);
            callback.as_mut().poll(context)
        });
        let result = std::panic::AssertUnwindSafe(rt_timeout(
            &*self.runtime,
            self.callback_timeout,
            callback,
        ))
        .catch_unwind()
        .await;
        match result {
            Ok(Ok(Ok(()))) => {}
            Ok(Ok(Err(error))) => log::warn!("Client lifecycle {name} failed: {error:#}"),
            Ok(Err(_)) => log::warn!("Client lifecycle {name} timed out"),
            Err(_) => log::warn!("Client lifecycle {name} panicked"),
        }
    }

    fn scopes(&self) -> std::sync::MutexGuard<'_, ScopeRegistry> {
        self.scopes
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn callback_queue(&self) -> std::sync::MutexGuard<'_, CallbackQueue> {
        self.callback_queue
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn scope_for(&self, generation: u64) -> Option<ConnectionScope> {
        let scopes = self.scopes();
        scopes
            .active
            .as_ref()
            .filter(|scope| scope.generation() == generation)
            .or_else(|| {
                scopes
                    .retired
                    .iter()
                    .find(|scope| scope.generation() == generation)
            })
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

    #[derive(Default)]
    struct ReentrantDisconnectLifecycle {
        client: std::sync::Mutex<Option<Weak<Client>>>,
        events: std::sync::Mutex<Vec<&'static str>>,
    }

    impl ClientLifecycle for ReentrantDisconnectLifecycle {
        fn install<'a>(&'a self, client: Weak<Client>) -> BoxFuture<'a, anyhow::Result<()>> {
            Box::pin(async move {
                *self
                    .client
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(client);
                Ok(())
            })
        }

        fn on_ready<'a>(&'a self, _scope: ConnectionScope) -> BoxFuture<'a, anyhow::Result<()>> {
            Box::pin(async move {
                self.events
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .push("ready-started");
                let client = self
                    .client
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .as_ref()
                    .and_then(Weak::upgrade)
                    .expect("installed client");
                client.disconnect().await;
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

        fn shutdown(&self) -> BoxFuture<'_, anyhow::Result<()>> {
            Box::pin(async move {
                self.events
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .push("shutdown");
                Ok(())
            })
        }
    }

    struct BlockingShutdownLifecycle {
        started: async_channel::Sender<()>,
        release: async_channel::Receiver<()>,
        calls: AtomicUsize,
        completed: AtomicBool,
    }

    impl ClientLifecycle for BlockingShutdownLifecycle {
        fn shutdown(&self) -> BoxFuture<'_, anyhow::Result<()>> {
            Box::pin(async move {
                self.calls.fetch_add(1, Ordering::SeqCst);
                let _ = self.started.try_send(());
                let _ = self.release.recv().await;
                self.completed.store(true, Ordering::Release);
                Ok(())
            })
        }
    }

    #[derive(Default)]
    struct SynchronousPanicLifecycle {
        ready_calls: AtomicUsize,
        closed_calls: AtomicUsize,
        shutdown_calls: AtomicUsize,
    }

    impl ClientLifecycle for SynchronousPanicLifecycle {
        fn on_ready<'a>(&'a self, _scope: ConnectionScope) -> BoxFuture<'a, anyhow::Result<()>> {
            self.ready_calls.fetch_add(1, Ordering::SeqCst);
            panic!("synchronous on_ready panic");
        }

        fn on_closed<'a>(&'a self, _scope: ConnectionScope) -> BoxFuture<'a, anyhow::Result<()>> {
            self.closed_calls.fetch_add(1, Ordering::SeqCst);
            panic!("synchronous on_closed panic");
        }

        fn shutdown(&self) -> BoxFuture<'_, anyhow::Result<()>> {
            self.shutdown_calls.fetch_add(1, Ordering::SeqCst);
            panic!("synchronous shutdown panic");
        }
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
        assert!(registration.begin_scope_if_current(GENERATION, || true));
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
        assert!(
            client
                .lifecycle
                .as_ref()
                .expect("lifecycle registration")
                .begin_scope_if_current(GENERATION, || true)
        );

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
        tokio::time::timeout(std::time::Duration::from_secs(2), cleanup_task)
            .await
            .expect("cleanup completed while callback was blocked")
            .expect("cleanup task did not panic");
        assert_eq!(scope.state(), ConnectionScopeState::Closed);

        release_ready_tx.send(()).await.expect("release ready hook");
        ready_task.await.expect("ready task did not panic");
        client.shutdown_lifecycle().await;
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
    async fn ready_callback_can_disconnect_its_client() {
        let persistence_manager = Arc::new(
            PersistenceManager::new(crate::test_utils::create_test_backend().await)
                .await
                .expect("persistence manager"),
        );
        let lifecycle = Arc::new(ReentrantDisconnectLifecycle::default());
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
        const GENERATION: u64 = 17;
        client
            .connection_generation
            .store(GENERATION, Ordering::SeqCst);
        assert!(
            client
                .lifecycle
                .as_ref()
                .expect("lifecycle registration")
                .begin_scope_if_current(GENERATION, || true)
        );

        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            client.dispatch_connected(),
        )
        .await
        .expect("reentrant disconnect completed");
        client.shutdown_lifecycle().await;

        assert_eq!(
            *lifecycle
                .events
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
            vec!["ready-started", "ready-finished", "closed", "shutdown"]
        );
        assert!(!client.is_logged_in());
        assert!(!client.is_ready.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn callback_timeout_does_not_hold_connection_cleanup() {
        let (ready_started_tx, ready_started_rx) = async_channel::bounded(1);
        let (_release_ready_tx, release_ready_rx) = async_channel::bounded(1);
        let lifecycle = Arc::new(BlockingReadyLifecycle {
            ready_started: ready_started_tx,
            release_ready: release_ready_rx,
            scope: std::sync::Mutex::new(None),
            events: std::sync::Mutex::new(Vec::new()),
        });
        let registration = Arc::new(LifecycleRegistration::new_with_timeout(
            lifecycle.clone(),
            Arc::new(TokioRuntime),
            std::time::Duration::from_millis(20),
        ));
        const GENERATION: u64 = 19;
        assert!(registration.begin_scope_if_current(GENERATION, || true));

        let ready_registration = Arc::clone(&registration);
        let ready = tokio::spawn(async move { ready_registration.ready(GENERATION).await });
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

        registration.cancel_scope(GENERATION);
        registration.close_scope(GENERATION);
        assert_eq!(scope.state(), ConnectionScopeState::Closed);
        assert!(
            !tokio::time::timeout(std::time::Duration::from_secs(1), ready)
                .await
                .expect("ready callback was bounded")
                .expect("ready task did not panic")
        );
        tokio::time::timeout(std::time::Duration::from_secs(1), registration.shutdown())
            .await
            .expect("lifecycle shutdown completed");

        assert_eq!(
            *lifecycle
                .events
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner()),
            vec!["ready-started", "closed"]
        );
    }

    #[tokio::test]
    async fn cancelled_shutdown_waiter_does_not_cancel_shutdown() {
        let (started_tx, started_rx) = async_channel::bounded(1);
        let (release_tx, release_rx) = async_channel::bounded(1);
        let lifecycle = Arc::new(BlockingShutdownLifecycle {
            started: started_tx,
            release: release_rx,
            calls: AtomicUsize::new(0),
            completed: AtomicBool::new(false),
        });
        let registration = Arc::new(LifecycleRegistration::new(
            lifecycle.clone(),
            Arc::new(TokioRuntime),
        ));

        let first_registration = Arc::clone(&registration);
        let first = tokio::spawn(async move { first_registration.shutdown().await });
        started_rx.recv().await.expect("shutdown callback started");
        first.abort();
        let _ = first.await;

        release_tx
            .send(())
            .await
            .expect("release shutdown callback");
        tokio::time::timeout(std::time::Duration::from_secs(2), registration.shutdown())
            .await
            .expect("later shutdown waiter observed completion");

        assert!(lifecycle.completed.load(Ordering::Acquire));
        assert_eq!(lifecycle.calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn synchronous_callback_panics_do_not_strand_the_driver() {
        let lifecycle = Arc::new(SynchronousPanicLifecycle::default());
        let registration = Arc::new(LifecycleRegistration::new(
            lifecycle.clone(),
            Arc::new(TokioRuntime),
        ));
        const GENERATION: u64 = 23;
        assert!(registration.begin_scope_if_current(GENERATION, || true));

        assert!(registration.ready(GENERATION).await);
        registration.close_scope(GENERATION);
        tokio::time::timeout(std::time::Duration::from_secs(2), registration.shutdown())
            .await
            .expect("callback driver recovered from synchronous panics");

        assert_eq!(lifecycle.ready_calls.load(Ordering::SeqCst), 1);
        assert_eq!(lifecycle.closed_calls.load(Ordering::SeqCst), 1);
        assert_eq!(lifecycle.shutdown_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn final_scope_closure_is_published_before_shutdown() {
        let lifecycle = Arc::new(RecordingLifecycle::default());
        let registration = Arc::new(LifecycleRegistration::new(
            lifecycle.clone(),
            Arc::new(TokioRuntime),
        ));
        const GENERATION: u64 = 29;
        assert!(registration.begin_scope_if_current(GENERATION, || true));

        let removed = Arc::new(std::sync::Barrier::new(2));
        let release = Arc::new(std::sync::Barrier::new(2));
        let close_registration = Arc::clone(&registration);
        let close_removed = Arc::clone(&removed);
        let close_release = Arc::clone(&release);
        let close = tokio::task::spawn_blocking(move || {
            close_registration.close_scope_with(GENERATION, || {
                close_removed.wait();
                close_release.wait();
            });
        });

        removed.wait();
        let shutdown_registration = Arc::clone(&registration);
        let shutdown = tokio::spawn(async move { shutdown_registration.shutdown().await });
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        assert!(!shutdown.is_finished());

        release.wait();
        close.await.expect("scope close task");
        shutdown.await.expect("shutdown task");
        assert_eq!(
            lifecycle.events(),
            vec!["closed:29".to_string(), "shutdown".to_string()]
        );
    }

    #[tokio::test]
    async fn cancelled_cleanup_waiter_does_not_strand_its_scope() {
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
        const GENERATION: u64 = 37;
        client
            .connection_generation
            .store(GENERATION, Ordering::SeqCst);
        let registration = client.lifecycle.as_ref().expect("lifecycle registration");
        assert!(registration.begin_scope_if_current(GENERATION, || true));
        client.dispatch_connected().await;
        let scope = registration
            .scope_for(GENERATION)
            .expect("connection scope");

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
        started_rx.recv().await.expect("cleanup reached transport");
        cleanup.abort();
        let _ = cleanup.await;
        assert_eq!(scope.state(), ConnectionScopeState::Cancelled);

        release_tx.send(()).await.expect("release cleanup");
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while scope.state() != ConnectionScopeState::Closed {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("detached cleanup closed the scope");
        assert!(registration.scope_for(GENERATION).is_none());

        registration.shutdown().await;
        assert_eq!(
            lifecycle.events(),
            vec!["install", "ready:37", "closed:37", "shutdown"]
        );
        client.signal_shutdown_sync();
    }

    #[tokio::test]
    async fn stale_generation_is_rejected_before_scope_publication() {
        let lifecycle = Arc::new(RecordingLifecycle::default());
        let registration = Arc::new(LifecycleRegistration::new(
            lifecycle.clone(),
            Arc::new(TokioRuntime),
        ));
        let generation = portable_atomic::AtomicU64::new(24);
        generation.store(25, Ordering::SeqCst);

        assert!(
            !registration
                .begin_scope_if_current(24, || { generation.load(Ordering::SeqCst) == 24 })
        );
        assert!(registration.scope_for(24).is_none());
        tokio::time::timeout(std::time::Duration::from_secs(2), registration.shutdown())
            .await
            .expect("shutdown did not wait for a rejected scope");
        assert_eq!(lifecycle.shutdowns.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn replaced_scope_stays_closeable_by_its_generation() {
        let lifecycle = Arc::new(RecordingLifecycle::default());
        let registration = Arc::new(LifecycleRegistration::new(
            lifecycle.clone(),
            Arc::new(TokioRuntime),
        ));

        assert!(registration.begin_scope_if_current(31, || true));
        assert!(registration.ready(31).await);
        let first = registration.scope_for(31).expect("first scope");

        assert!(registration.begin_scope_if_current(32, || true));
        assert_eq!(first.state(), ConnectionScopeState::Cancelled);
        assert!(registration.scope_for(31).is_some());
        assert!(registration.ready(32).await);

        registration.close_scope(31);
        assert_eq!(first.state(), ConnectionScopeState::Closed);
        assert!(registration.scope_for(31).is_none());
        assert!(registration.scope_for(32).is_some());

        registration.close_scope(32);
        registration.shutdown().await;
        assert_eq!(
            lifecycle.events(),
            vec!["ready:31", "ready:32", "closed:31", "closed:32", "shutdown",]
        );
    }

    #[tokio::test]
    async fn shutdown_waits_for_the_active_scope_and_is_terminal() {
        let lifecycle = Arc::new(RecordingLifecycle::default());
        let registration = Arc::new(LifecycleRegistration::new(
            lifecycle.clone(),
            Arc::new(TokioRuntime),
        ));
        const GENERATION: u64 = 21;
        assert!(registration.begin_scope_if_current(GENERATION, || true));
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

        registration.close_scope(GENERATION);
        shutdown.await.expect("shutdown did not panic");
        assert_eq!(
            lifecycle.events(),
            vec!["closed:21".to_string(), "shutdown".to_string()]
        );

        assert!(!registration.begin_scope_if_current(GENERATION + 1, || true));
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
        client
            .subscribe_handler(Arc::new(LogoutOrderHandler {
                lifecycle: lifecycle.clone(),
            }))
            .detach();

        client.logout().await.expect("client logout");

        assert_eq!(
            lifecycle.events(),
            vec!["install", "logged-out", "shutdown"]
        );
    }
}

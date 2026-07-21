use std::cell::Cell;
use std::collections::VecDeque;
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::{Arc, Weak};
use std::time::Duration;

use super::Client;
use futures::FutureExt;
use wacore::runtime::{BoxFuture, Runtime, ShutdownNotifier, ShutdownSignal, wait_for_shutdown};

const SCOPE_OPEN: u8 = 0;
const SCOPE_READY: u8 = 1;
const SCOPE_CANCELLED: u8 = 2;
const SCOPE_CLOSED: u8 = 3;
const CONSTRUCTION_INSTALLING: u8 = 0;
const CONSTRUCTION_ACTIVE: u8 = 1;
const CONSTRUCTION_REJECTED: u8 = 2;
const CALLBACK_TIMEOUT: Duration = Duration::from_secs(5);
const CALLBACK_QUEUE_TARGET_CAPACITY: usize = 64;

std::thread_local! {
    static ACTIVE_CALLBACK: Cell<*const LifecycleRegistration> = const { Cell::new(std::ptr::null()) };
    static ACTIVE_READY_PUBLICATION: Cell<*const LifecycleRegistration> = const { Cell::new(std::ptr::null()) };
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
    pub(crate) fn new(generation: u64) -> Self {
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

    pub(crate) fn cancel(&self) {
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
/// serialized, with bounded ready work; connection cleanup only schedules
/// `on_closed` so a stalled extension cannot block reconnect. Closure callbacks
/// are lossless and may temporarily exceed the target capacity. A future plugin
/// host owns per-plugin ordering and isolation behind this client-level seam.
/// `install` receives a weak client reference so retaining it cannot create a cycle.
/// `signal_shutdown` is the non-blocking boundary for resources that must stop
/// even when an FFI host cannot await `shutdown`.
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

    /// Stop synchronously owned resources before asynchronous shutdown begins.
    /// Implementations must return promptly and make repeated calls harmless.
    fn signal_shutdown(&self) {}

    fn shutdown(&self) -> BoxFuture<'_, anyhow::Result<()>> {
        Box::pin(async { Ok(()) })
    }
}

pub(super) struct LifecycleRegistration {
    handler: Arc<dyn ClientLifecycle>,
    runtime: Arc<dyn Runtime>,
    ready_publication: std::sync::Mutex<()>,
    scopes: std::sync::Mutex<ScopeRegistry>,
    callback_queue: std::sync::Mutex<CallbackQueue>,
    shutdown_complete: AtomicBool,
    shutdown_notifier: ShutdownNotifier,
    callback_timeout: Duration,
    terminal: AtomicBool,
    construction_state: AtomicU8,
    construction_notifier: ShutdownNotifier,
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
    overflowed: bool,
}

impl CallbackQueue {
    fn push_with_pressure_policy(
        &mut self,
        callback: LifecycleCallback,
    ) -> Option<LifecycleCallback> {
        if self.pending.len() < CALLBACK_QUEUE_TARGET_CAPACITY {
            self.pending.push_back(callback);
            return None;
        }

        self.overflowed = true;
        let ready_position = self
            .pending
            .iter()
            .position(|pending| matches!(pending, LifecycleCallback::Ready { .. }));
        match (callback, ready_position) {
            (callback @ LifecycleCallback::Ready { .. }, None) => Some(callback),
            (callback, Some(position)) => {
                let dropped = self.pending.remove(position);
                self.pending.push_back(callback);
                dropped
            }
            (callback, None) => {
                // Every closed scope must reach the extension even when a callback stalls.
                self.pending.push_back(callback);
                None
            }
        }
    }

    fn compact_for_shutdown(&mut self) -> Vec<LifecycleCallback> {
        if !self.overflowed {
            return Vec::new();
        }

        let mut retained = VecDeque::with_capacity(self.pending.len());
        let mut dropped = Vec::new();
        for callback in self.pending.drain(..) {
            match callback {
                callback @ LifecycleCallback::Closed(_) => retained.push_back(callback),
                callback => dropped.push(callback),
            }
        }
        self.pending = retained;
        dropped
    }
}

struct CallbackContextGuard {
    previous: *const LifecycleRegistration,
}

struct ReadyPublicationGuard {
    previous: *const LifecycleRegistration,
}

impl ReadyPublicationGuard {
    fn enter(registration: &LifecycleRegistration) -> Self {
        let previous = ACTIVE_READY_PUBLICATION.replace(registration);
        Self { previous }
    }
}

impl Drop for ReadyPublicationGuard {
    fn drop(&mut self) {
        ACTIVE_READY_PUBLICATION.set(self.previous);
    }
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

fn ready_publication_active(registration: &LifecycleRegistration) -> bool {
    ACTIVE_READY_PUBLICATION.with(|active| std::ptr::eq(active.get(), registration))
}

impl LifecycleRegistration {
    pub(super) fn new(handler: Arc<dyn ClientLifecycle>, runtime: Arc<dyn Runtime>) -> Self {
        Self::new_with_timeout(handler, runtime, CALLBACK_TIMEOUT)
    }

    pub(super) fn new_with_timeout(
        handler: Arc<dyn ClientLifecycle>,
        runtime: Arc<dyn Runtime>,
        callback_timeout: Duration,
    ) -> Self {
        Self {
            handler,
            runtime,
            ready_publication: std::sync::Mutex::new(()),
            scopes: std::sync::Mutex::new(ScopeRegistry::default()),
            callback_queue: std::sync::Mutex::new(CallbackQueue::default()),
            shutdown_complete: AtomicBool::new(false),
            shutdown_notifier: ShutdownNotifier::new(),
            callback_timeout,
            terminal: AtomicBool::new(false),
            construction_state: AtomicU8::new(CONSTRUCTION_INSTALLING),
            construction_notifier: ShutdownNotifier::new(),
        }
    }

    pub(super) async fn install(&self, client: Weak<Client>) -> anyhow::Result<()> {
        let rejected = self.construction_notifier.subscribe();
        if self.construction_state.load(Ordering::Acquire) == CONSTRUCTION_REJECTED {
            return Err(anyhow::anyhow!(
                "client shutdown began during lifecycle installation"
            ));
        }
        let mut install = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.handler.install(client)
        }))
        .map_err(|_| anyhow::anyhow!("lifecycle install panicked before returning a future"))?;
        let cancelled = Box::pin(wait_for_shutdown(&rejected));
        let result = {
            let install_poll = std::future::poll_fn(|context| install.as_mut().poll(context));
            let install_poll = Box::pin(std::panic::AssertUnwindSafe(install_poll).catch_unwind());
            match futures::future::select(cancelled, install_poll).await {
                futures::future::Either::Left((_, install_poll)) => {
                    drop(install_poll);
                    None
                }
                futures::future::Either::Right((result, _)) => Some(result),
            }
        };
        let drop_panicked =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| drop(install))).is_err();
        if drop_panicked {
            return Err(anyhow::anyhow!(
                "lifecycle install future panicked while being dropped"
            ));
        }
        let Some(result) = result else {
            return Err(anyhow::anyhow!(
                "client shutdown began during lifecycle installation"
            ));
        };
        let result = result.map_err(|_| anyhow::anyhow!("lifecycle install future panicked"))?;
        if result.is_ok()
            && self.construction_state.load(Ordering::Acquire) == CONSTRUCTION_REJECTED
        {
            return Err(anyhow::anyhow!(
                "client shutdown began during lifecycle installation"
            ));
        }
        result
    }

    pub(super) fn activate(&self) -> bool {
        if self.terminal.load(Ordering::Acquire) {
            self.reject_construction();
            return false;
        }
        match self.construction_state.compare_exchange(
            CONSTRUCTION_INSTALLING,
            CONSTRUCTION_ACTIVE,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => self.construction_notifier.notify(),
            Err(CONSTRUCTION_ACTIVE) => {}
            Err(_) => return false,
        }
        !self.terminal.load(Ordering::Acquire)
    }

    pub(super) async fn wait_until_active(&self) -> bool {
        let activated = self.construction_notifier.subscribe();
        match self.construction_state.load(Ordering::Acquire) {
            CONSTRUCTION_ACTIVE => return !self.terminal.load(Ordering::Acquire),
            CONSTRUCTION_REJECTED => return false,
            _ => {}
        }
        wacore::runtime::wait_for_shutdown(&activated).await;
        self.construction_state.load(Ordering::Acquire) == CONSTRUCTION_ACTIVE
            && !self.terminal.load(Ordering::Acquire)
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

    pub(super) fn publish_ready(&self, generation: u64, publish: impl FnOnce()) -> bool {
        let _publication = self.ready_publication();
        if self.terminal.load(Ordering::Acquire) {
            return false;
        }
        let Some(scope) = self.scope_for(generation) else {
            return false;
        };
        if scope.state() != ConnectionScopeState::Ready {
            return false;
        }

        let _publication_context = ReadyPublicationGuard::enter(self);
        publish();
        true
    }

    pub(super) fn cancel_scope(&self, generation: u64) {
        if ready_publication_active(self) {
            self.cancel_scope_inner(generation);
        } else {
            let _publication = self.ready_publication();
            self.cancel_scope_inner(generation);
        }
    }

    pub(super) fn cancel_active_scope(&self) {
        if ready_publication_active(self) {
            self.cancel_active_scope_inner();
        } else {
            let _publication = self.ready_publication();
            self.cancel_active_scope_inner();
        }
    }

    pub(super) fn close_scope(self: &Arc<Self>, generation: u64) {
        self.close_scope_with(generation, || {});
    }

    /// Non-noop hooks are test-only, must run off-executor, and must not re-enter lifecycle APIs.
    fn close_scope_with(self: &Arc<Self>, generation: u64, after_remove: impl FnOnce()) {
        let (should_spawn, dropped) = {
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
            let mut dropped = Vec::new();
            if queue.shutdown_requested {
                dropped.extend(queue.push_with_pressure_policy(LifecycleCallback::Closed(scope)));
                dropped.extend(queue.compact_for_shutdown());
                if no_open_scopes && !queue.shutdown_enqueued {
                    queue.shutdown_enqueued = true;
                    queue.pending.push_back(LifecycleCallback::Shutdown);
                }
            } else {
                dropped.extend(queue.push_with_pressure_policy(LifecycleCallback::Closed(scope)));
            }
            let should_spawn = !queue.pending.is_empty() && !queue.drain_scheduled;
            queue.drain_scheduled |= should_spawn;
            (should_spawn, dropped)
        };

        warn_dropped_callbacks(dropped);
        self.spawn_callback_driver(should_spawn);
    }

    pub(super) async fn shutdown(self: &Arc<Self>) {
        self.request_shutdown();

        if self.shutdown_complete.load(Ordering::Acquire) || callback_context_active(self) {
            return;
        }

        let completed = self.shutdown_notifier.subscribe();
        if self.shutdown_complete.load(Ordering::Acquire) {
            return;
        }
        wacore::runtime::wait_for_shutdown(&completed).await;
    }

    pub(super) fn request_shutdown(self: &Arc<Self>) {
        self.signal_shutdown_sync();
        {
            let mut queue = self.callback_queue();
            queue.shutdown_requested = true;
        }
        self.enqueue_shutdown_if_ready();
    }

    pub(super) fn signal_shutdown_sync(&self) {
        self.reject_construction();
        let first_signal = if ready_publication_active(self) {
            self.mark_terminal_and_cancel_scopes()
        } else {
            let _publication = self.ready_publication();
            self.mark_terminal_and_cancel_scopes()
        };
        if first_signal
            && std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                self.handler.signal_shutdown();
            }))
            .is_err()
        {
            log::warn!("Client lifecycle synchronous shutdown signal panicked");
        }
    }

    fn enqueue_callback(self: &Arc<Self>, callback: LifecycleCallback) {
        let (should_spawn, dropped) = {
            let mut queue = self.callback_queue();
            if queue.shutdown_requested || self.terminal.load(Ordering::Acquire) {
                (false, Some(callback))
            } else {
                let dropped = queue.push_with_pressure_policy(callback);
                let should_spawn = !queue.drain_scheduled;
                queue.drain_scheduled = true;
                (should_spawn, dropped)
            }
        };
        warn_dropped_callbacks(dropped.into_iter().collect());
        self.spawn_callback_driver(should_spawn);
    }

    fn enqueue_shutdown_if_ready(self: &Arc<Self>) {
        let no_open_scopes = {
            let scopes = self.scopes();
            scopes.active.is_none() && scopes.retired.is_empty()
        };
        let (should_spawn, dropped) = {
            let mut queue = self.callback_queue();
            if !queue.shutdown_requested || queue.shutdown_enqueued {
                return;
            }
            let dropped = queue.compact_for_shutdown();
            if no_open_scopes {
                queue.shutdown_enqueued = true;
                queue.pending.push_back(LifecycleCallback::Shutdown);
            }
            let should_spawn = !queue.pending.is_empty() && !queue.drain_scheduled;
            queue.drain_scheduled |= should_spawn;
            (should_spawn, dropped)
        };
        warn_dropped_callbacks(dropped);
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
                        queue.overflowed = false;
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
        let result = {
            let callback_poll = std::future::poll_fn(|context| {
                let _callback_context = CallbackContextGuard::enter(self);
                callback.as_mut().poll(context)
            });
            let callback_poll =
                Box::pin(std::panic::AssertUnwindSafe(callback_poll).catch_unwind());
            match futures::future::select(callback_poll, self.runtime.sleep(self.callback_timeout))
                .await
            {
                futures::future::Either::Left((result, _)) => Some(result),
                futures::future::Either::Right(((), callback_poll)) => {
                    drop(callback_poll);
                    None
                }
            }
        };
        let drop_panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _callback_context = CallbackContextGuard::enter(self);
            drop(callback);
        }))
        .is_err();
        if drop_panicked {
            log::warn!("Client lifecycle {name} panicked while being dropped");
            return;
        }
        match result {
            Some(Ok(Ok(()))) => {}
            Some(Ok(Err(error))) => log::warn!("Client lifecycle {name} failed: {error:#}"),
            Some(Err(_)) => log::warn!("Client lifecycle {name} panicked"),
            None => log::warn!("Client lifecycle {name} timed out"),
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

    fn ready_publication(&self) -> std::sync::MutexGuard<'_, ()> {
        self.ready_publication
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn cancel_scope_inner(&self, generation: u64) {
        if let Some(scope) = self.scope_for(generation) {
            scope.cancel();
        }
    }

    fn reject_construction(&self) {
        if self
            .construction_state
            .compare_exchange(
                CONSTRUCTION_INSTALLING,
                CONSTRUCTION_REJECTED,
                Ordering::AcqRel,
                Ordering::Acquire,
            )
            .is_ok()
        {
            self.construction_notifier.notify();
        }
    }

    fn cancel_active_scope_inner(&self) {
        if let Some(scope) = &self.scopes().active {
            scope.cancel();
        }
    }

    fn mark_terminal_and_cancel_scopes(&self) -> bool {
        let first_signal = !self.terminal.swap(true, Ordering::AcqRel);
        let scopes = self.scopes();
        if let Some(scope) = &scopes.active {
            scope.cancel();
        }
        for scope in &scopes.retired {
            scope.cancel();
        }
        first_signal
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

fn warn_dropped_callbacks(dropped: Vec<LifecycleCallback>) {
    if !dropped.is_empty() {
        log::warn!(
            "Dropped {} stale client lifecycle callback(s) under queue pressure or terminal shutdown",
            dropped.len()
        );
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

    struct PanickingDisconnect;

    #[async_trait]
    impl crate::transport::Transport for PanickingDisconnect {
        async fn send(&self, _data: Bytes) -> anyhow::Result<()> {
            Ok(())
        }

        async fn disconnect(&self) {
            panic!("injected disconnect panic");
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

    struct ReentrantReconnectLifecycle {
        client: std::sync::Mutex<Option<Weak<Client>>>,
        events: std::sync::Mutex<Vec<&'static str>>,
        immediate: bool,
    }

    impl ClientLifecycle for ReentrantReconnectLifecycle {
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
                if self.immediate {
                    client.reconnect_immediately().await;
                } else {
                    client.reconnect().await;
                }
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

    struct QueuePressureLifecycle {
        ready_started: async_channel::Sender<()>,
        release_ready: async_channel::Receiver<()>,
        closed_calls: AtomicUsize,
        shutdown_calls: AtomicUsize,
    }

    impl ClientLifecycle for QueuePressureLifecycle {
        fn on_ready<'a>(&'a self, _scope: ConnectionScope) -> BoxFuture<'a, anyhow::Result<()>> {
            Box::pin(async move {
                let _ = self.ready_started.try_send(());
                let _ = self.release_ready.recv().await;
                Ok(())
            })
        }

        fn on_closed<'a>(&'a self, _scope: ConnectionScope) -> BoxFuture<'a, anyhow::Result<()>> {
            Box::pin(async move {
                self.closed_calls.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
        }

        fn shutdown(&self) -> BoxFuture<'_, anyhow::Result<()>> {
            Box::pin(async move {
                self.shutdown_calls.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
        }
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
    struct EarlyShutdownLifecycle {
        signalled: AtomicBool,
        shutdowns: AtomicUsize,
    }

    impl ClientLifecycle for EarlyShutdownLifecycle {
        fn signal_shutdown(&self) {
            self.signalled.store(true, Ordering::Release);
        }

        fn shutdown(&self) -> BoxFuture<'_, anyhow::Result<()>> {
            Box::pin(async move {
                assert!(self.signalled.load(Ordering::Acquire));
                self.shutdowns.fetch_add(1, Ordering::SeqCst);
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

    #[derive(Default)]
    struct DropPanickingFutureLifecycle {
        closed_calls: AtomicUsize,
        shutdown_calls: AtomicUsize,
    }

    struct DropPanickingPendingFuture;

    impl Future for DropPanickingPendingFuture {
        type Output = anyhow::Result<()>;

        fn poll(
            self: std::pin::Pin<&mut Self>,
            _context: &mut std::task::Context<'_>,
        ) -> std::task::Poll<Self::Output> {
            std::task::Poll::Pending
        }
    }

    impl Drop for DropPanickingPendingFuture {
        fn drop(&mut self) {
            panic!("injected lifecycle callback drop panic");
        }
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

    impl ClientLifecycle for DropPanickingFutureLifecycle {
        fn on_ready<'a>(&'a self, _scope: ConnectionScope) -> BoxFuture<'a, anyhow::Result<()>> {
            Box::pin(DropPanickingPendingFuture)
        }

        fn on_closed<'a>(&'a self, _scope: ConnectionScope) -> BoxFuture<'a, anyhow::Result<()>> {
            Box::pin(async move {
                self.closed_calls.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
        }

        fn shutdown(&self) -> BoxFuture<'_, anyhow::Result<()>> {
            Box::pin(async move {
                self.shutdown_calls.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
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
    async fn terminal_signal_rejects_ready_publication() {
        let registration = Arc::new(LifecycleRegistration::new(
            Arc::new(RecordingLifecycle::default()),
            Arc::new(TokioRuntime),
        ));
        const GENERATION: u64 = 43;
        assert!(registration.begin_scope_if_current(GENERATION, || true));
        assert!(registration.ready(GENERATION).await);
        registration.signal_shutdown_sync();

        let published = AtomicBool::new(false);
        assert!(!registration.publish_ready(GENERATION, || {
            published.store(true, Ordering::Release);
        }));
        assert!(!published.load(Ordering::Acquire));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn terminal_cancellation_waits_for_ready_publication() {
        let registration = Arc::new(LifecycleRegistration::new(
            Arc::new(RecordingLifecycle::default()),
            Arc::new(TokioRuntime),
        ));
        const GENERATION: u64 = 47;
        assert!(registration.begin_scope_if_current(GENERATION, || true));
        assert!(registration.ready(GENERATION).await);
        let scope = registration
            .scope_for(GENERATION)
            .expect("ready connection scope");

        let (started_tx, started_rx) = std::sync::mpsc::sync_channel(1);
        let (release_tx, release_rx) = std::sync::mpsc::sync_channel(1);
        let (published_tx, published_rx) = std::sync::mpsc::sync_channel(1);
        let publish_registration = registration.clone();
        let publish = std::thread::spawn(move || {
            let published = publish_registration.publish_ready(GENERATION, || {
                let _ = started_tx.send(());
                let _ = release_rx.recv();
            });
            let _ = published_tx.send(published);
        });
        started_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("ready publication started");

        let (attempted_tx, attempted_rx) = std::sync::mpsc::sync_channel(1);
        let (cancelled_tx, cancelled_rx) = std::sync::mpsc::sync_channel(1);
        let cancel_registration = registration.clone();
        let cancel = std::thread::spawn(move || {
            let _ = attempted_tx.send(());
            cancel_registration.signal_shutdown_sync();
            let _ = cancelled_tx.send(());
        });
        attempted_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("terminal cancellation attempted");
        assert!(
            cancelled_rx
                .recv_timeout(Duration::from_millis(100))
                .is_err()
        );

        release_tx.send(()).expect("release ready publication");
        assert!(
            published_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("ready publication completed")
        );
        cancelled_rx
            .recv_timeout(Duration::from_secs(2))
            .expect("terminal cancellation completed");
        publish.join().expect("publication thread");
        cancel.join().expect("cancellation thread");
        assert_eq!(scope.state(), ConnectionScopeState::Cancelled);
    }

    #[tokio::test]
    async fn ready_publication_allows_reentrant_terminal_signal() {
        let registration = Arc::new(LifecycleRegistration::new(
            Arc::new(RecordingLifecycle::default()),
            Arc::new(TokioRuntime),
        ));
        const GENERATION: u64 = 53;
        assert!(registration.begin_scope_if_current(GENERATION, || true));
        assert!(registration.ready(GENERATION).await);
        let scope = registration
            .scope_for(GENERATION)
            .expect("ready connection scope");

        let (completed_tx, completed_rx) = std::sync::mpsc::sync_channel(1);
        let publish_registration = registration.clone();
        let signal_registration = registration.clone();
        let publish = std::thread::spawn(move || {
            let published = publish_registration.publish_ready(GENERATION, || {
                signal_registration.signal_shutdown_sync();
            });
            let _ = completed_tx.send(published);
        });

        assert!(
            completed_rx
                .recv_timeout(Duration::from_secs(2))
                .expect("reentrant terminal signal completed")
        );
        publish.join().expect("publication thread");
        assert_eq!(scope.state(), ConnectionScopeState::Cancelled);
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
        let client = build.into_client();
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
    async fn disconnect_requests_lifecycle_shutdown_before_cancellable_io() {
        let persistence_manager = Arc::new(
            PersistenceManager::new(crate::test_utils::create_test_backend().await)
                .await
                .expect("persistence manager"),
        );
        let lifecycle = Arc::new(EarlyShutdownLifecycle::default());
        let client = Client::builder()
            .with_runtime(TokioRuntime)
            .with_persistence_manager(persistence_manager)
            .with_transport_factory(MockTransportFactory::new())
            .with_http_client(MockHttpClient)
            .with_lifecycle_arc(lifecycle.clone())
            .build()
            .await
            .expect("client build")
            .into_client();
        let (started_tx, started_rx) = async_channel::bounded(1);
        let (_release_tx, release_rx) = async_channel::bounded(1);
        *client.transport.lock().await = Some(Arc::new(BlockingDisconnect {
            started: started_tx,
            release: release_rx,
        }));

        let disconnect_client = Arc::clone(&client);
        let disconnect = tokio::spawn(async move {
            disconnect_client.disconnect().await;
        });
        tokio::time::timeout(std::time::Duration::from_secs(2), started_rx.recv())
            .await
            .expect("disconnect reached cancellable transport I/O")
            .expect("transport remained alive");
        assert!(lifecycle.signalled.load(Ordering::Acquire));

        disconnect.abort();
        let _ = disconnect.await;
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while lifecycle.shutdowns.load(Ordering::SeqCst) != 1 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("detached lifecycle shutdown completed");
    }

    #[tokio::test]
    async fn dropping_last_client_owner_signals_standalone_lifecycle() {
        let persistence_manager = Arc::new(
            PersistenceManager::new(crate::test_utils::create_test_backend().await)
                .await
                .expect("persistence manager"),
        );
        let lifecycle = Arc::new(EarlyShutdownLifecycle::default());
        let client = Client::builder()
            .with_runtime(TokioRuntime)
            .with_persistence_manager(persistence_manager)
            .with_transport_factory(MockTransportFactory::new())
            .with_http_client(MockHttpClient)
            .with_lifecycle_arc(lifecycle.clone())
            .build()
            .await
            .expect("client build")
            .into_client();
        let weak = Arc::downgrade(&client);

        drop(client);

        tokio::time::timeout(Duration::from_secs(2), async {
            while weak.upgrade().is_some() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("background services released the client");
        assert!(lifecycle.signalled.load(Ordering::Acquire));
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
            .into_client();
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
            .into_client();
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
    async fn ready_callback_reconnect_requests_retire_the_scope_before_returning() {
        for immediate in [false, true] {
            let persistence_manager = Arc::new(
                PersistenceManager::new(crate::test_utils::create_test_backend().await)
                    .await
                    .expect("persistence manager"),
            );
            let lifecycle = Arc::new(ReentrantReconnectLifecycle {
                client: std::sync::Mutex::new(None),
                events: std::sync::Mutex::new(Vec::new()),
                immediate,
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
                .into_client();
            const GENERATION: u64 = 18;
            client
                .connection_generation
                .store(GENERATION, Ordering::SeqCst);
            let registration = client.lifecycle.as_ref().expect("lifecycle registration");
            assert!(registration.begin_scope_if_current(GENERATION, || true));

            tokio::time::timeout(Duration::from_secs(2), client.dispatch_connected())
                .await
                .expect("reentrant reconnect completed");
            let scope = registration
                .scope_for(GENERATION)
                .expect("cancelled connection scope");
            assert_eq!(scope.state(), ConnectionScopeState::Cancelled);
            assert!(!client.is_ready.load(Ordering::Relaxed));

            registration.close_scope(GENERATION);
            registration.shutdown().await;
            assert_eq!(
                *lifecycle
                    .events
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner()),
                vec!["ready-started", "ready-finished", "closed", "shutdown"]
            );
        }
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
    async fn callback_queue_preserves_every_scope_closure_before_shutdown() {
        let (ready_started_tx, ready_started_rx) = async_channel::bounded(1);
        let (release_ready_tx, release_ready_rx) = async_channel::bounded(1);
        let lifecycle = Arc::new(QueuePressureLifecycle {
            ready_started: ready_started_tx,
            release_ready: release_ready_rx,
            closed_calls: AtomicUsize::new(0),
            shutdown_calls: AtomicUsize::new(0),
        });
        let registration = Arc::new(LifecycleRegistration::new_with_timeout(
            lifecycle.clone(),
            Arc::new(TokioRuntime),
            Duration::from_secs(1),
        ));

        let active_scope = ConnectionScope::new(1);
        assert!(active_scope.mark_ready());
        let (done, _done_rx) = async_channel::bounded(1);
        registration.enqueue_callback(LifecycleCallback::Ready {
            scope: active_scope,
            done,
        });
        ready_started_rx
            .recv()
            .await
            .expect("ready callback started");

        let closed_callbacks = CALLBACK_QUEUE_TARGET_CAPACITY as u64 * 4 - 2;
        for generation in 2..(CALLBACK_QUEUE_TARGET_CAPACITY as u64 * 4) {
            let scope = ConnectionScope::new(generation);
            scope.close();
            registration.enqueue_callback(LifecycleCallback::Closed(scope));

            let (done, _done_rx) = async_channel::bounded(1);
            registration.enqueue_callback(LifecycleCallback::Ready {
                scope: ConnectionScope::new(generation + closed_callbacks),
                done,
            });
        }
        assert_eq!(
            registration.callback_queue().pending.len(),
            usize::try_from(closed_callbacks).expect("closure count fits usize")
        );

        let shutdown_registration = registration.clone();
        let shutdown = tokio::spawn(async move { shutdown_registration.shutdown().await });
        tokio::time::timeout(Duration::from_secs(1), async {
            loop {
                let compacted = {
                    let queue = registration.callback_queue();
                    if queue.shutdown_enqueued {
                        assert_eq!(
                            queue.pending.len(),
                            usize::try_from(closed_callbacks + 1)
                                .expect("terminal callback count fits usize")
                        );
                        true
                    } else {
                        false
                    }
                };
                if compacted {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("terminal backlog compaction");

        release_ready_tx
            .send(())
            .await
            .expect("release ready callback");
        shutdown.await.expect("bounded terminal shutdown");
        assert_eq!(
            lifecycle.closed_calls.load(Ordering::SeqCst),
            usize::try_from(closed_callbacks).expect("closure count fits usize")
        );
        assert_eq!(lifecycle.shutdown_calls.load(Ordering::SeqCst), 1);
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

    #[tokio::test]
    async fn callback_drop_panics_do_not_strand_the_driver() {
        let lifecycle = Arc::new(DropPanickingFutureLifecycle::default());
        let registration = Arc::new(LifecycleRegistration::new_with_timeout(
            lifecycle.clone(),
            Arc::new(TokioRuntime),
            Duration::from_millis(10),
        ));
        const GENERATION: u64 = 24;
        assert!(registration.begin_scope_if_current(GENERATION, || true));

        assert!(
            tokio::time::timeout(Duration::from_secs(1), registration.ready(GENERATION))
                .await
                .expect("ready callback cancellation completed")
        );
        registration.close_scope(GENERATION);
        tokio::time::timeout(Duration::from_secs(1), registration.shutdown())
            .await
            .expect("callback driver recovered from a drop panic");

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
            .into_client();
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
    async fn detached_cleanup_propagates_panics_to_its_waiter() {
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
            .into_client();
        const GENERATION: u64 = 41;
        client
            .connection_generation
            .store(GENERATION, Ordering::SeqCst);
        let registration = client.lifecycle.as_ref().expect("lifecycle registration");
        assert!(registration.begin_scope_if_current(GENERATION, || true));
        client.dispatch_connected().await;
        let scope = registration
            .scope_for(GENERATION)
            .expect("connection scope");
        *client.transport.lock().await = Some(Arc::new(PanickingDisconnect));

        let cleanup_client = Arc::clone(&client);
        let cleanup = tokio::spawn(async move {
            cleanup_client.cleanup_connection_state().await;
        });
        let panic = tokio::time::timeout(Duration::from_secs(2), cleanup)
            .await
            .expect("cleanup waiter did not hang")
            .expect_err("cleanup panic should reach its waiter");

        assert!(panic.is_panic());
        assert_eq!(scope.state(), ConnectionScopeState::Closed);
        assert!(registration.scope_for(GENERATION).is_none());
        tokio::time::timeout(Duration::from_secs(2), registration.shutdown())
            .await
            .expect("shutdown waited for the panicked cleanup scope");
        assert_eq!(
            lifecycle.events(),
            vec!["install", "ready:41", "closed:41", "shutdown"]
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
    async fn rejected_success_restores_logged_out_state() {
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
            .with_lifecycle_arc(lifecycle)
            .build()
            .await
            .expect("client build")
            .into_client();
        client
            .lifecycle
            .as_ref()
            .expect("lifecycle registration")
            .signal_shutdown_sync();

        let success = wacore_binary::builder::NodeBuilder::new("success").build();
        client.handle_success(&success.as_node_ref()).await;

        assert!(!client.is_logged_in());
        assert_eq!(client.connection_generation.load(Ordering::SeqCst), 1);
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
            .into_client();
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

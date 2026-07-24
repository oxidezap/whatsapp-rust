use std::future::Future;
use std::pin::Pin;
use std::time::Duration;

use async_trait::async_trait;

/// A runtime-agnostic abstraction over async executor capabilities.
///
/// On native targets, futures must be `Send` (multi-threaded executors).
/// On wasm32, `Send` is dropped (single-threaded).
#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait Runtime: Send + Sync + 'static {
    fn spawn(&self, future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>) -> AbortHandle;
    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>>;
    fn spawn_blocking(
        &self,
        f: Box<dyn FnOnce() + Send + 'static>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>>;

    /// Cooperatively yield, allowing other tasks and I/O to make progress.
    ///
    /// Use this in tight async loops that process many items to avoid
    /// starving other work. Returns `None` if yielding is unnecessary
    /// (e.g. multi-threaded runtimes where other tasks run on separate
    /// threads), or `Some(future)` that the caller must `.await` to
    /// actually yield.
    ///
    /// Returning `None` avoids any allocation or async overhead, making
    /// the call zero-cost on runtimes that don't need cooperative yielding.
    fn yield_now(&self) -> Option<Pin<Box<dyn Future<Output = ()> + Send>>>;

    /// How often to yield in tight loops (every N items). Defaults to 10.
    /// Single-threaded runtimes should return 1 to avoid starving the event loop.
    fn yield_frequency(&self) -> u32 {
        10
    }
}

/// WASM variant — `Send` bounds removed since WASM is single-threaded.
/// Concrete types use `unsafe impl Send + Sync` since there's only one thread.
#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
pub trait Runtime: Send + Sync + 'static {
    fn spawn(&self, future: Pin<Box<dyn Future<Output = ()> + 'static>>) -> AbortHandle;
    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()>>>;
    fn spawn_blocking(&self, f: Box<dyn FnOnce() + 'static>) -> Pin<Box<dyn Future<Output = ()>>>;

    /// Cooperatively yield, allowing other tasks and I/O to make progress.
    ///
    /// Returns `None` if yielding is unnecessary, or `Some(future)` that
    /// the caller must `.await` to actually yield.
    fn yield_now(&self) -> Option<Pin<Box<dyn Future<Output = ()>>>>;

    /// How often to yield in tight loops (every N items). Defaults to 10.
    /// Single-threaded runtimes should return 1 to avoid starving the event loop.
    fn yield_frequency(&self) -> u32 {
        10
    }
}

/// Boxed future with the target-correct thread bound: `Send` on native
/// (multi-threaded executors), none on wasm32 (single-threaded). Use this for
/// type-erased entry-point futures so the same signature builds on both.
#[cfg(not(target_arch = "wasm32"))]
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;
#[cfg(target_arch = "wasm32")]
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + 'a>>;

/// Bound for futures a [`Runtime`] can spawn: `Send + 'static` on native
/// (work-stealing executors move tasks across threads), just `'static` on wasm
/// (single-threaded). Generic spawn helpers carry this one bound so they stay
/// correct on both targets instead of hardcoding `Send`.
#[cfg(not(target_arch = "wasm32"))]
pub trait Spawnable: Send + 'static {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Send + 'static> Spawnable for T {}

#[cfg(target_arch = "wasm32")]
pub trait Spawnable: 'static {}
#[cfg(target_arch = "wasm32")]
impl<T: 'static> Spawnable for T {}

/// Handle returned by [`Runtime::spawn`]. Aborts the spawned task when dropped.
///
/// Uses `std::sync::Mutex` internally so that the handle is `Send + Sync`,
/// which is required because it may be stored inside structs shared across
/// tasks (e.g. `NoiseSocket` behind an `Arc`).
#[must_use = "dropping an AbortHandle aborts the task; use .detach() for fire-and-forget"]
pub struct AbortHandle {
    abort_fn: std::sync::Mutex<Option<Box<dyn FnOnce() + Send + 'static>>>,
}

impl AbortHandle {
    /// Create a new abort handle with the given cancellation function.
    pub fn new(abort_fn: impl FnOnce() + Send + 'static) -> Self {
        Self {
            abort_fn: std::sync::Mutex::new(Some(Box::new(abort_fn))),
        }
    }

    /// Create a no-op handle that does nothing on drop.
    pub fn noop() -> Self {
        Self {
            abort_fn: std::sync::Mutex::new(None),
        }
    }

    /// Explicitly abort the spawned task without waiting for drop.
    pub fn abort(&self) {
        if let Some(f) = self
            .abort_fn
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
        {
            f();
        }
    }

    /// Detach the handle so the task is NOT aborted on drop.
    ///
    /// The spawned task will run until completion even if the parent scope
    /// is dropped. Use this for fire-and-forget tasks where cancellation
    /// is not desired.
    pub fn detach(self) {
        *self.abort_fn.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

impl Drop for AbortHandle {
    fn drop(&mut self) {
        self.abort();
    }
}

/// Publish-side owner of a shutdown notifier. Exposes `notify()` which sets
/// a sticky flag before waking listeners so a late subscriber still observes
/// the shutdown (event_listener notifications are edge-triggered).
pub struct ShutdownNotifier {
    inner: std::sync::Arc<ShutdownInner>,
}

struct ShutdownInner {
    // SeqCst ensures publishers always set `fired` before `event.notify` and
    // subscribers always register `listen` before loading `fired`; combined,
    // a listener either sees the flag or is guaranteed to be woken by notify.
    fired: std::sync::atomic::AtomicBool,
    event: event_listener::Event,
}

impl ShutdownNotifier {
    pub fn new() -> Self {
        Self {
            inner: std::sync::Arc::new(ShutdownInner {
                fired: std::sync::atomic::AtomicBool::new(false),
                event: event_listener::Event::new(),
            }),
        }
    }

    pub fn notify(&self) {
        self.inner
            .fired
            .store(true, std::sync::atomic::Ordering::SeqCst);
        self.inner.event.notify(usize::MAX);
    }

    fn is_fired(&self) -> bool {
        self.inner.fired.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Sticky-aware listener: registers the event listener BEFORE reading the
    /// flag so a notify that races this call either sets the flag we observe
    /// or wakes the listener we just registered. Returned future is 'static
    /// so it can be stored in `let` bindings and composed in `select!`.
    pub fn listen(&self) -> impl Future<Output = ()> + use<> {
        let listener = self.inner.event.listen();
        let fired = self.is_fired();
        async move {
            if fired {
                return;
            }
            listener.await;
        }
    }

    pub fn subscribe(&self) -> ShutdownSignal {
        ShutdownSignal {
            inner: Some(std::sync::Arc::clone(&self.inner)),
        }
    }
}

impl Default for ShutdownNotifier {
    fn default() -> Self {
        Self::new()
    }
}

/// Subscribe-side handle. Clone is cheap (atomic ref-count). Holds a strong
/// `Arc` to the notifier's inner so the sticky flag and event survive across
/// a publisher-side replacement (e.g. `Mutex<ShutdownNotifier>` swapped on
/// reconnect). Long-lived tasks must capture the signal once at startup; if
/// they re-subscribed each loop iteration a racing swap could strand them on
/// a fresh notifier that was never fired.
#[derive(Clone)]
pub struct ShutdownSignal {
    // None for `never()` — always Pending, always not-fired.
    inner: Option<std::sync::Arc<ShutdownInner>>,
}

impl ShutdownSignal {
    /// Inert handle whose listener never fires. Useful for tests or callers
    /// that don't wire a real notifier.
    pub fn never() -> Self {
        Self { inner: None }
    }

    /// Cheap synchronous probe without awaiting.
    pub fn is_fired(&self) -> bool {
        self.inner
            .as_ref()
            .is_some_and(|i| i.fired.load(std::sync::atomic::Ordering::SeqCst))
    }
}

/// Wait for shutdown, resolving when `ShutdownNotifier::notify` has been
/// called. Stays `Pending` for signals built via [`ShutdownSignal::never`];
/// pair with another exit condition in `futures::select!`.
///
/// The listener is registered BEFORE the sticky-flag load so a notify that
/// races the subscription either sets the flag we then observe or wakes the
/// listener we just registered. Call this directly inside the select arm, not
/// earlier in the function, to keep the race window closed.
pub fn wait_for_shutdown(signal: &ShutdownSignal) -> impl Future<Output = ()> + use<> {
    let (fired, listener) = match signal.inner.as_ref() {
        Some(inner) => {
            let listener = inner.event.listen();
            // Load AFTER listen so a notify that happens between the two
            // paths is caught — either the listener wakes or we read the
            // flag set by the publisher.
            let fired = inner.fired.load(std::sync::atomic::Ordering::SeqCst);
            (fired, Some(listener))
        }
        None => (false, None),
    };
    async move {
        if fired {
            return;
        }
        match listener {
            Some(l) => l.await,
            None => std::future::pending::<()>().await,
        }
    }
}

/// Error returned when an async operation times out.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("operation timed out")]
pub struct Elapsed;

/// Race a future against a timeout. Returns [`Elapsed`] if the duration
/// expires before the future completes.
pub async fn timeout<F, T>(rt: &dyn Runtime, duration: Duration, future: F) -> Result<T, Elapsed>
where
    F: Future<Output = T>,
{
    use futures::future::Either;

    futures::pin_mut!(future);
    let sleep = rt.sleep(duration);
    futures::pin_mut!(sleep);

    match futures::future::select(future, sleep).await {
        Either::Left((result, _)) => Ok(result),
        Either::Right(((), _)) => Err(Elapsed),
    }
}

/// Offload a blocking closure to a thread where blocking is acceptable,
/// returning its result.
///
/// Convenience wrapper around [`Runtime::spawn_blocking`] that uses
/// a oneshot channel to ferry the closure's return value back to the caller.
///
/// # Panics
///
/// Panics if the runtime drops the spawned task before it completes
/// (e.g. during runtime shutdown).
#[cfg(not(target_arch = "wasm32"))]
pub async fn blocking<T: Send + 'static>(
    rt: &dyn Runtime,
    f: impl FnOnce() -> T + Send + 'static,
) -> T {
    let (tx, rx) = futures::channel::oneshot::channel();
    rt.spawn_blocking(Box::new(move || {
        let _ = tx.send(f());
    }))
    .await;
    rx.await.unwrap_or_else(|_| {
        panic!("blocking task failed to complete (closure panic or runtime shutdown)")
    })
}

/// WASM variant — runs inline (single-threaded).
#[cfg(target_arch = "wasm32")]
pub async fn blocking<T: 'static>(_rt: &dyn Runtime, f: impl FnOnce() -> T + 'static) -> T {
    f()
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod abort_handle_tests {
    use super::AbortHandle;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn counting_handle() -> (AbortHandle, Arc<AtomicUsize>) {
        let calls = Arc::new(AtomicUsize::new(0));
        let handle = {
            let calls = Arc::clone(&calls);
            AbortHandle::new(move || {
                calls.fetch_add(1, Ordering::SeqCst);
            })
        };
        (handle, calls)
    }

    #[test]
    fn abort_runs_the_callback_exactly_once() {
        let (handle, calls) = counting_handle();

        handle.abort();
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        // The callback is FnOnce, so a second abort (and the Drop that follows)
        // must be a no-op rather than a double cancellation.
        handle.abort();
        drop(handle);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn drop_aborts_an_unaborted_handle() {
        let (handle, calls) = counting_handle();
        assert_eq!(calls.load(Ordering::SeqCst), 0);

        drop(handle);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn detach_disarms_the_abort_on_drop() {
        let (handle, calls) = counting_handle();

        handle.detach();
        assert_eq!(
            calls.load(Ordering::SeqCst),
            0,
            "a detached handle must leave the task running"
        );
    }

    #[test]
    fn noop_handle_is_inert() {
        let handle = AbortHandle::noop();
        handle.abort();
        drop(handle);
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tokio_runtime_tests {
    use super::{AbortHandle, Elapsed, Runtime, timeout};
    use std::future::Future;
    use std::pin::Pin;
    use std::time::Duration;

    /// Bound for every await below: a test that hangs should fail, not stall CI.
    const TEST_TIMEOUT: Duration = Duration::from_secs(5);

    struct TokioTestRuntime;

    #[async_trait::async_trait]
    impl Runtime for TokioTestRuntime {
        fn spawn(&self, future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>) -> AbortHandle {
            let handle = tokio::spawn(future);
            AbortHandle::new(move || handle.abort())
        }
        fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            Box::pin(tokio::time::sleep(duration))
        }
        fn spawn_blocking(
            &self,
            f: Box<dyn FnOnce() + Send + 'static>,
        ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            Box::pin(async move {
                let _ = tokio::task::spawn_blocking(f).await;
            })
        }
        fn yield_now(&self) -> Option<Pin<Box<dyn Future<Output = ()> + Send>>> {
            None
        }
    }

    /// Fires its channel when dropped, so a test can observe that a spawned
    /// future was cancelled without racing a sleep against the abort.
    struct DropSignal(Option<futures::channel::oneshot::Sender<()>>);

    impl Drop for DropSignal {
        fn drop(&mut self) {
            if let Some(tx) = self.0.take() {
                let _ = tx.send(());
            }
        }
    }

    /// Spawn a task that parks forever, returning its handle plus a receiver
    /// that resolves once the task's future has been dropped. The signal is
    /// built before the future so it fires even if the task is cancelled
    /// before its first poll.
    async fn spawn_parked_task() -> (AbortHandle, futures::channel::oneshot::Receiver<()>) {
        let (tx, rx) = futures::channel::oneshot::channel();
        let (started_tx, started_rx) = futures::channel::oneshot::channel();
        let dropped_on_cancel = DropSignal(Some(tx));
        let handle = TokioTestRuntime.spawn(Box::pin(async move {
            let _dropped_on_cancel = dropped_on_cancel;
            let _ = started_tx.send(());
            std::future::pending::<()>().await;
        }));
        // Wait for the task to report its first poll: a yield only requeues the
        // current task, so without this the abort could land before the spawned
        // future ever ran and the cancel-a-running-task path would go untested.
        tokio::time::timeout(TEST_TIMEOUT, started_rx)
            .await
            .expect("timed out waiting for the spawned task to start")
            .expect("the spawned task was cancelled before it started");
        (handle, rx)
    }

    async fn expect_cancelled(rx: futures::channel::oneshot::Receiver<()>) {
        tokio::time::timeout(TEST_TIMEOUT, rx)
            .await
            .expect("timed out waiting for the spawned task to be cancelled")
            .expect("the task's drop signal was lost");
    }

    #[tokio::test]
    async fn abort_cancels_a_spawned_task() {
        let (handle, rx) = spawn_parked_task().await;
        handle.abort();
        expect_cancelled(rx).await;
    }

    #[tokio::test]
    async fn dropping_the_handle_cancels_a_spawned_task() {
        let (handle, rx) = spawn_parked_task().await;
        drop(handle);
        expect_cancelled(rx).await;
    }

    #[tokio::test]
    async fn detached_task_survives_its_handle() {
        let (tx, rx) = futures::channel::oneshot::channel();
        TokioTestRuntime
            .spawn(Box::pin(async move {
                let _ = tx.send(7u32);
            }))
            .detach();

        let value = tokio::time::timeout(TEST_TIMEOUT, rx)
            .await
            .expect("timed out waiting for the detached task")
            .expect("the detached task was cancelled");
        assert_eq!(value, 7);
    }

    #[tokio::test]
    async fn timeout_returns_the_value_when_the_future_wins() {
        // A generous budget against an immediately-ready future: the only way
        // this fails is if timeout drops the result.
        let result = timeout(&TokioTestRuntime, Duration::from_secs(30), async { 42u32 }).await;
        assert_eq!(result, Ok(42));
    }

    #[tokio::test]
    async fn timeout_elapses_when_the_future_never_completes() {
        // `pending()` can never win the race, so the deadline is deterministic
        // and can be short.
        let result = timeout(
            &TokioTestRuntime,
            Duration::from_millis(5),
            std::future::pending::<()>(),
        )
        .await;
        assert_eq!(result, Err(Elapsed));
        assert_eq!(Elapsed.to_string(), "operation timed out");
    }

    #[tokio::test]
    async fn timeout_does_not_wait_out_the_duration_on_success() {
        // The sleep must be dropped as soon as the future resolves; a timeout
        // that awaited both would take a minute here.
        tokio::time::timeout(TEST_TIMEOUT, async {
            let result = timeout(&TokioTestRuntime, Duration::from_secs(60), async { "ok" }).await;
            assert_eq!(result, Ok("ok"));
        })
        .await
        .expect("timeout() must return as soon as the inner future completes");
    }

    #[tokio::test]
    async fn blocking_ferries_the_closure_result_back() {
        // Bounded so a `blocking` that never hands the result back fails here
        // instead of hanging the job.
        let value =
            tokio::time::timeout(TEST_TIMEOUT, super::blocking(&TokioTestRuntime, || 6 * 7))
                .await
                .expect("timed out waiting for the blocking closure");
        assert_eq!(value, 42);
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod shutdown_tests {
    use super::{ShutdownNotifier, ShutdownSignal, wait_for_shutdown};
    use futures::FutureExt;
    use futures::executor::block_on;

    // Regression guard against CodeRabbit's critical finding on PR #560:
    // event_listener notifications are edge-triggered, so a `notify()` fired
    // before a subscriber calls `listen()` would be lost without the sticky
    // flag. Verify that notify -> subscribe -> wait_for_shutdown still
    // resolves immediately.
    #[test]
    fn wait_for_shutdown_catches_notify_fired_before_subscribe() {
        let notifier = ShutdownNotifier::new();
        notifier.notify();

        let signal = notifier.subscribe();
        block_on(wait_for_shutdown(&signal));
    }

    // Same guard for the publisher-side listen() helper.
    #[test]
    fn notifier_listen_catches_notify_fired_before_listen() {
        let notifier = ShutdownNotifier::new();
        notifier.notify();

        block_on(notifier.listen());
    }

    // Guard the ordered path: listener registered first, notify after.
    // Must resolve through the normal event-listener wakeup (not the sticky
    // flag fast-path, which only fires when the flag is set before listen).
    #[test]
    fn wait_for_shutdown_wakes_on_notify_after_subscribe() {
        let notifier = ShutdownNotifier::new();
        let signal = notifier.subscribe();
        let fut = wait_for_shutdown(&signal);

        notifier.notify();
        block_on(fut);
    }

    // notify() wakes every registered listener, not just the first one:
    // event_listener's notify takes a count, so a wrong count here would strand
    // all but one of the tasks waiting on shutdown.
    //
    // Resolution is the wrong observable for that: the sticky flag makes a
    // re-polled waiter finish whether or not it was ever woken, so `block_on`
    // alone still passes with notify(1). Count the wakeups instead, and poll
    // each waiter to Pending first — registration happens on that first poll,
    // so a still-lazy future is not yet listening when notify fires.
    #[test]
    fn notify_wakes_every_registered_listener() {
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        struct CountingWaker(AtomicUsize);
        impl futures::task::ArcWake for CountingWaker {
            fn wake_by_ref(arc_self: &Arc<Self>) {
                arc_self.0.fetch_add(1, Ordering::Relaxed);
            }
        }

        let notifier = ShutdownNotifier::new();
        let signals: Vec<_> = (0..4).map(|_| notifier.subscribe()).collect();
        let mut waiters: Vec<_> = signals
            .iter()
            .map(|s| Box::pin(wait_for_shutdown(s).fuse()))
            .collect();
        let mut publisher_side = Box::pin(notifier.listen().fuse());

        // One waker per subscriber, plus one for the publisher-side listener.
        let wakers: Vec<_> = (0..signals.len() + 1)
            .map(|_| Arc::new(CountingWaker(AtomicUsize::new(0))))
            .collect();
        for (fut, waker) in waiters.iter_mut().zip(&wakers) {
            let waker = futures::task::waker_ref(waker);
            let mut ctx = futures::task::Context::from_waker(&waker);
            assert!(fut.as_mut().poll_unpin(&mut ctx).is_pending());
        }
        {
            let waker = futures::task::waker_ref(wakers.last().expect("wakers is non-empty"));
            let mut ctx = futures::task::Context::from_waker(&waker);
            assert!(publisher_side.as_mut().poll_unpin(&mut ctx).is_pending());
        }
        assert!(
            wakers.iter().all(|w| w.0.load(Ordering::Relaxed) == 0),
            "no listener may be woken before notify()"
        );

        notifier.notify();

        for (i, waker) in wakers.iter().enumerate() {
            assert!(
                waker.0.load(Ordering::Relaxed) > 0,
                "listener {i} of {} was never woken by notify()",
                wakers.len()
            );
        }

        block_on(futures::future::join_all(waiters));
        block_on(publisher_side);
        assert!(signals.iter().all(|s| s.is_fired()));
    }

    // Repeated notifies are harmless, and a subscriber that arrives after the
    // last one still sees the sticky flag.
    #[test]
    fn notify_is_idempotent_for_late_subscribers() {
        let notifier = ShutdownNotifier::new();
        let early = notifier.subscribe();
        notifier.notify();
        notifier.notify();

        let late = notifier.subscribe();
        assert!(early.is_fired());
        assert!(late.is_fired());
        block_on(futures::future::join(
            wait_for_shutdown(&early),
            wait_for_shutdown(&late),
        ));
    }

    // Clones share the notifier's inner state, so one notify covers all of them.
    #[test]
    fn cloned_signals_observe_the_same_notify() {
        let notifier = ShutdownNotifier::new();
        let signal = notifier.subscribe();
        let clone = signal.clone();

        assert!(!signal.is_fired());
        assert!(!clone.is_fired());

        notifier.notify();

        assert!(signal.is_fired());
        assert!(clone.is_fired());
        block_on(wait_for_shutdown(&clone));
    }

    // A fresh notifier must not look fired, and its listeners must stay pending
    // until someone notifies.
    #[test]
    fn unfired_notifier_leaves_listeners_pending() {
        let notifier = ShutdownNotifier::default();
        let signal = notifier.subscribe();
        assert!(!signal.is_fired());

        let mut fut = Box::pin(wait_for_shutdown(&signal).fuse());
        let mut ctx = futures::task::Context::from_waker(futures::task::noop_waker_ref());
        assert!(fut.as_mut().poll_unpin(&mut ctx).is_pending());
    }

    // never() must never resolve. Poll once manually and assert Pending.
    #[test]
    fn wait_for_shutdown_never_stays_pending() {
        let signal = ShutdownSignal::never();
        let mut fut = Box::pin(wait_for_shutdown(&signal).fuse());
        let mut ctx = futures::task::Context::from_waker(futures::task::noop_waker_ref());
        assert!(fut.as_mut().poll_unpin(&mut ctx).is_pending());
    }

    // Captured signal must survive the publisher being dropped — tasks that
    // hold the signal across a Mutex<ShutdownNotifier> swap need to still see
    // the notify that fired before the swap. With Weak<Inner> the Arc would
    // die on swap and subsequent wait_for_shutdown calls would pend forever.
    #[test]
    fn captured_signal_observes_fire_after_notifier_dropped() {
        let notifier = ShutdownNotifier::new();
        let signal = notifier.subscribe();
        notifier.notify();
        drop(notifier);

        assert!(
            signal.is_fired(),
            "Signal must remain fired after the publisher was dropped"
        );
        block_on(wait_for_shutdown(&signal));
    }
}

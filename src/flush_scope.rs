//! Tracks spawned tasks so a shutdown point can wait for them to complete.
//!
//! Used for outbound stanzas (delivery receipts, etc.) that must land before
//! the transport is torn down (issue #571). The decrement runs via a Drop
//! guard so the counter can't leak if the task future is cancelled mid-await.

use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use wacore::runtime::{Runtime, Spawnable};

/// Bit 0 of [`FlushScope::state`]: the scope is closed to new work.
const CLOSED: usize = 1;
/// One tracked unit of work, i.e. one live [`FlushGuard`]; the count lives in
/// bits 1.. of the same word. Packing the flag next to the counter is what
/// makes "refuse if closed, otherwise increment" a single read-modify-write,
/// and that in turn is what upholds the promise `close` makes: once it
/// returns, nothing further is tracked. Split across two atomics there would
/// be a window between the two operations, and that window is the whole bug.
const ONE_TRACKED: usize = 2;

pub struct FlushScope {
    state: AtomicUsize,
    idle: event_listener::Event,
    #[cfg(test)]
    track_rejections: AtomicUsize,
}

impl Default for FlushScope {
    fn default() -> Self {
        Self::new()
    }
}

impl FlushScope {
    pub fn new() -> Self {
        Self {
            state: AtomicUsize::new(0),
            idle: event_listener::Event::new(),
            #[cfg(test)]
            track_rejections: AtomicUsize::new(0),
        }
    }

    // Relaxed on both flag writes: they publish no data, and the exclusion
    // `try_track` relies on comes from every writer being a read-modify-write
    // on one location, which the per-location modification order already
    // totally orders regardless of the ordering argument.
    pub fn close(&self) {
        self.state.fetch_or(CLOSED, Ordering::Relaxed);
    }

    pub fn reopen(&self) {
        self.state.fetch_and(!CLOSED, Ordering::Relaxed);
    }

    /// Track a unit of outbound work without spawning a task (e.g. an item
    /// handed to a persistent worker). Returns `None` when the scope is
    /// closed — the work must be dropped, mirroring `spawn`. Dropping the
    /// guard marks the work as finished for `flush`.
    pub fn try_track(self: &Arc<Self>) -> Option<FlushGuard> {
        // `checked_add` rather than a bare add: a wrap would clobber the closed
        // flag in bit 0. It cannot be reached — the count only rises while a
        // guard is alive, and usize::MAX/2 live guards is more Arcs than the
        // address space holds — so refusing to track is a formality, handled
        // the same way as a closed scope.
        if self
            .state
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |state| {
                if state & CLOSED != 0 {
                    return None;
                }
                state.checked_add(ONE_TRACKED)
            })
            .is_err()
        {
            #[cfg(test)]
            self.track_rejections.fetch_add(1, Ordering::Relaxed);
            return None;
        }
        Some(FlushGuard {
            scope: Arc::clone(self),
        })
    }

    /// Spawn a tracked task. The counter decrements on completion OR if the
    /// future is dropped (e.g. aborted, or dropped before its first poll), so
    /// `flush` can never deadlock waiting on a cancelled task.
    // `Spawnable` (not a hardcoded `Send`) so this compiles on wasm, where the
    // single-threaded runtime accepts !Send futures (see `wacore::runtime`).
    pub fn spawn<F>(self: &Arc<Self>, rt: &dyn Runtime, fut: F)
    where
        F: Future<Output = ()> + Spawnable,
    {
        // Construct the guard outside the async block so it's captured as an
        // upvalue. This puts it in the future's state machine BEFORE the first
        // poll, so even if the runtime drops the future without polling it,
        // the guard's Drop runs and decrements the counter. Constructing the
        // guard inside the async body would delay it until the first poll,
        // which leaks the count if the future is dropped earlier.
        let Some(guard) = self.try_track() else {
            return;
        };
        rt.spawn(Box::pin(async move {
            let _guard = guard;
            fut.await;
        }))
        .detach();
    }

    /// Wait until every tracked task has finished or the timeout elapses.
    /// Emits a warn log on timeout with the number of leaked tasks.
    pub async fn flush(&self, rt: &dyn Runtime, timeout: Duration) {
        use wacore::time::Instant;

        let deadline = Instant::now() + timeout;
        loop {
            let listener = self.idle.listen();
            if self.tracked() == 0 {
                return;
            }

            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero()
                || wacore::runtime::timeout(rt, remaining, listener)
                    .await
                    .is_err()
            {
                log::warn!(
                    "FlushScope timed out with {} pending task(s)",
                    self.tracked()
                );
                return;
            }
        }
    }

    /// Live guards. Acquire pairs with the guards' release decrements, so a
    /// caller that tears the transport down once this reads zero has observed
    /// everything the tracked work did.
    fn tracked(&self) -> usize {
        self.state.load(Ordering::Acquire) >> 1
    }

    #[cfg(test)]
    pub fn pending(&self) -> usize {
        self.tracked()
    }

    /// Number of tasks parked inside [`Self::flush`]. `flush` registers its
    /// listener before checking the counter, so a listener here means a flusher
    /// really is waiting — what tests poll instead of sleeping.
    #[cfg(test)]
    pub fn flush_waiters(&self) -> usize {
        self.idle.total_listeners()
    }

    #[cfg(test)]
    pub fn track_rejections(&self) -> usize {
        self.track_rejections.load(Ordering::Relaxed)
    }
}

/// RAII guard for one tracked unit of outbound work. Obtained via
/// [`FlushScope::try_track`]; `spawn` embeds one in the task it creates.
pub struct FlushGuard {
    scope: Arc<FlushScope>,
}

impl Drop for FlushGuard {
    fn drop(&mut self) {
        // Release so the finished work is visible to whoever reads the counter
        // down to zero; the flusher's acquire load closes the pair.
        if self.scope.state.fetch_sub(ONE_TRACKED, Ordering::Release) >> 1 == 1 {
            self.scope.idle.notify(usize::MAX);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;
    use wacore::time::Instant;

    fn rt() -> Arc<dyn Runtime> {
        Arc::new(crate::runtime_impl::TokioRuntime)
    }

    #[tokio::test]
    async fn flush_returns_after_tasks_complete() {
        let scope = Arc::new(FlushScope::new());
        let runtime = rt();
        for _ in 0..5 {
            let r = Arc::clone(&runtime);
            scope.spawn(&*runtime, async move {
                r.sleep(Duration::from_millis(20)).await;
            });
        }
        assert_eq!(scope.pending(), 5);

        let start = Instant::now();
        scope.flush(&*runtime, Duration::from_secs(1)).await;
        assert!(start.elapsed() < Duration::from_millis(500));
        assert_eq!(scope.pending(), 0);
    }

    #[tokio::test]
    async fn flush_returns_immediately_when_idle() {
        let scope = Arc::new(FlushScope::new());
        let runtime = rt();

        let start = Instant::now();
        scope.flush(&*runtime, Duration::from_secs(5)).await;
        assert!(start.elapsed() < Duration::from_millis(10));
    }

    #[tokio::test]
    async fn close_rejects_new_tasks_until_reopened() {
        let scope = Arc::new(FlushScope::new());
        let runtime = rt();
        let ran = Arc::new(AtomicBool::new(false));

        scope.close();
        let ran_closed = Arc::clone(&ran);
        scope.spawn(&*runtime, async move {
            ran_closed.store(true, Ordering::Relaxed);
        });
        scope.flush(&*runtime, Duration::from_secs(1)).await;
        assert!(!ran.load(Ordering::Relaxed));

        scope.reopen();
        let ran_open = Arc::clone(&ran);
        scope.spawn(&*runtime, async move {
            ran_open.store(true, Ordering::Relaxed);
        });
        scope.flush(&*runtime, Duration::from_secs(1)).await;
        assert!(ran.load(Ordering::Relaxed));
    }

    #[tokio::test]
    async fn flush_honors_timeout_and_logs() {
        let scope = Arc::new(FlushScope::new());
        let runtime = rt();
        let r = Arc::clone(&runtime);
        scope.spawn(&*runtime, async move {
            r.sleep(Duration::from_secs(10)).await;
        });

        let start = Instant::now();
        scope.flush(&*runtime, Duration::from_millis(50)).await;
        let elapsed = start.elapsed();
        assert!(elapsed >= Duration::from_millis(50));
        assert!(elapsed < Duration::from_millis(500));
        assert_eq!(scope.pending(), 1);
    }

    /// Regression: if a tracked task's future is dropped after being polled
    /// (the normal abort path), the counter must still decrement via Drop.
    #[tokio::test]
    async fn decrement_runs_when_future_is_aborted_mid_flight() {
        use std::task::{Context, Poll};

        let scope = Arc::new(FlushScope::new());
        scope.state.fetch_add(ONE_TRACKED, Ordering::Relaxed);

        let scope_for_fut = Arc::clone(&scope);
        let mut fut: std::pin::Pin<Box<dyn Future<Output = ()> + Send>> = Box::pin(async move {
            let _guard = FlushGuard {
                scope: scope_for_fut,
            };
            futures::future::pending::<()>().await;
        });

        // Poll once so the guard is constructed and the future suspends on the
        // inner pending().
        let waker = futures::task::noop_waker();
        let mut cx = Context::from_waker(&waker);
        assert!(matches!(fut.as_mut().poll(&mut cx), Poll::Pending));
        assert_eq!(scope.pending(), 1);

        // Simulate abort: drop the in-flight boxed future.
        drop(fut);
        assert_eq!(
            scope.pending(),
            0,
            "FlushGuard must fire when the in-flight future is dropped"
        );
    }

    /// If the outer wrapping future is dropped *before its first poll* (e.g.
    /// the executor is shutting down), the guard must still be dropped so the
    /// counter decrements. Before the fix (guard constructed INSIDE the async
    /// body), this would leak the counter and cause `flush()` to wait its
    /// full timeout on every disconnect.
    #[tokio::test]
    async fn decrement_runs_when_future_is_dropped_before_first_poll() {
        let scope = Arc::new(FlushScope::new());

        // Emulate what spawn() does. The guard must be captured as an upvalue
        // so it's in the state machine from construction, not only after the
        // first poll.
        scope.state.fetch_add(ONE_TRACKED, Ordering::Relaxed);
        let guard = FlushGuard {
            scope: Arc::clone(&scope),
        };
        let never_polled_fut = async move {
            let _guard = guard;
            futures::future::pending::<()>().await;
        };
        assert_eq!(scope.pending(), 1);

        drop(never_polled_fut);

        assert_eq!(
            scope.pending(),
            0,
            "guard must drop with the never-polled future"
        );
    }

    #[tokio::test]
    async fn try_track_guard_blocks_flush_until_dropped() {
        let scope = Arc::new(FlushScope::new());
        let runtime = rt();

        let guard = scope.try_track().expect("open scope must track");
        assert_eq!(scope.pending(), 1);

        let flush_scope = Arc::clone(&scope);
        let flush_runtime = Arc::clone(&runtime);
        let flush_task = tokio::spawn(async move {
            flush_scope
                .flush(&*flush_runtime, Duration::from_secs(5))
                .await;
        });
        crate::test_utils::poll_until("the flusher to park on the scope", || {
            scope.flush_waiters() >= 1
        })
        .await;
        assert!(
            !flush_task.is_finished(),
            "flush must wait while a tracked guard is alive"
        );

        drop(guard);
        tokio::time::timeout(Duration::from_secs(1), flush_task)
            .await
            .expect("flush should finish once the guard drops")
            .expect("flush task should not panic");
        assert_eq!(scope.pending(), 0);
    }

    #[tokio::test]
    async fn try_track_rejected_when_closed() {
        let scope = Arc::new(FlushScope::new());
        scope.close();
        assert!(scope.try_track().is_none(), "closed scope must not track");
        assert_eq!(scope.pending(), 0);

        scope.reopen();
        let guard = scope.try_track();
        assert!(guard.is_some(), "reopened scope must track again");
        assert_eq!(scope.pending(), 1);
    }

    /// The guarantee that decides the packed layout: once `close()` has
    /// returned, no further guard is handed out. Exercised under a real race,
    /// because the failure mode is a check-then-increment window that a serial
    /// test cannot open.
    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn close_wins_against_concurrent_try_track() {
        for _ in 0..64 {
            let scope = Arc::new(FlushScope::new());
            let start = Arc::new(std::sync::Barrier::new(5));
            // Published by the closer the instant `close()` returns. Read
            // BEFORE each attempt: seeing it set means close already returned,
            // so a grant that then succeeds is a violation on its own, with no
            // shared count to launder it.
            let closed = Arc::new(AtomicBool::new(false));

            let trackers: Vec<_> = (0..4)
                .map(|_| {
                    let scope = Arc::clone(&scope);
                    let start = Arc::clone(&start);
                    let closed = Arc::clone(&closed);
                    std::thread::spawn(move || {
                        start.wait();
                        let mut late = 0usize;
                        // The guards are handed back rather than dropped here,
                        // so the count the closer sampled stays comparable.
                        let guards: Vec<_> = std::iter::from_fn(|| {
                            let seen_closed = closed.load(Ordering::SeqCst);
                            let guard = scope.try_track()?;
                            if seen_closed {
                                late += 1;
                            }
                            Some(guard)
                        })
                        .take(256)
                        .collect();
                        (guards, late)
                    })
                })
                .collect();

            start.wait();
            scope.close();
            // Sampled before the flag is published, so the two nets abut: a
            // grant later than this sample is caught by the count, an earlier
            // one by a racer that already saw the flag. Only a grant between
            // close() returning and this single load escapes both.
            let after_close = scope.pending();
            closed.store(true, Ordering::SeqCst);

            let results: Vec<_> = trackers
                .into_iter()
                .map(|t| t.join().expect("tracker thread"))
                .collect();
            let granted: usize = results.iter().map(|(guards, _)| guards.len()).sum();
            let late: usize = results.iter().map(|(_, late)| late).sum();
            assert_eq!(late, 0, "try_track granted a guard after close() returned");
            assert_eq!(
                granted, after_close,
                "try_track granted a guard after close() returned"
            );
        }
    }

    /// A saturated counter must refuse rather than wrap into the closed flag.
    #[tokio::test]
    async fn try_track_refuses_when_the_counter_would_overflow() {
        let scope = Arc::new(FlushScope::new());
        scope.state.store(usize::MAX & !CLOSED, Ordering::Relaxed);

        assert!(scope.try_track().is_none(), "saturated scope must refuse");
        assert_eq!(
            scope.state.load(Ordering::Relaxed) & CLOSED,
            0,
            "a refused track must not corrupt the closed flag"
        );

        // Room for exactly one more still tracks, so the refusal is the
        // saturation point and not an off-by-one earlier.
        scope
            .state
            .store((usize::MAX & !CLOSED) - ONE_TRACKED, Ordering::Relaxed);
        assert!(
            scope.try_track().is_some(),
            "one slot left must still track"
        );
    }

    #[tokio::test]
    async fn ran_bodies_observe_completion() {
        let scope = Arc::new(FlushScope::new());
        let runtime = rt();
        let done = Arc::new(AtomicBool::new(false));
        let done_clone = Arc::clone(&done);
        scope.spawn(&*runtime, async move {
            done_clone.store(true, Ordering::Relaxed);
        });
        scope.flush(&*runtime, Duration::from_secs(1)).await;
        assert!(done.load(Ordering::Relaxed));
    }
}

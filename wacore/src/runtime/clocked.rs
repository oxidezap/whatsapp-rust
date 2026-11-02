use super::{AbortHandle, BoxFuture, Runtime};
use crate::time::Clock;
use std::sync::Arc;
use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
type BlockingJob = Box<dyn FnOnce() + Send + 'static>;
#[cfg(target_arch = "wasm32")]
type BlockingJob = Box<dyn FnOnce() + 'static>;

/// Execution on the inner runtime with one selected clock.
/// Virtual clocks supply their own timers. Other clocks must advance with the inner executor.
#[derive(Clone)]
pub struct ClockedRuntime {
    inner: Arc<dyn Runtime>,
    clock: Arc<dyn Clock>,
}

impl ClockedRuntime {
    pub fn new(inner: Arc<dyn Runtime>, clock: Arc<dyn Clock>) -> Self {
        Self { inner, clock }
    }
}

impl Runtime for ClockedRuntime {
    fn clock(&self) -> Option<&Arc<dyn Clock>> {
        Some(&self.clock)
    }
    fn spawn(&self, future: BoxFuture<'static, ()>) -> AbortHandle {
        self.inner.spawn(future)
    }
    fn spawn_detached(&self, future: BoxFuture<'static, ()>) {
        self.inner.spawn_detached(future);
    }
    fn sleep(&self, duration: Duration) -> BoxFuture<'static, ()> {
        self.clock
            .clone()
            .sleep(duration)
            .unwrap_or_else(|| self.inner.sleep(duration))
    }
    fn spawn_blocking(&self, job: BlockingJob) -> BoxFuture<'static, ()> {
        self.inner.spawn_blocking(job)
    }
    fn yield_now(&self) -> Option<BoxFuture<'static, ()>> {
        self.inner.yield_now()
    }
    fn yield_frequency(&self) -> u32 {
        self.inner.yield_frequency()
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Default)]
    struct Calls {
        spawn: AtomicUsize,
        detached: AtomicUsize,
        sleep: AtomicUsize,
        blocking: AtomicUsize,
        yield_now: AtomicUsize,
    }
    struct RecordingRuntime(Arc<Calls>);
    impl Runtime for RecordingRuntime {
        fn spawn(&self, _future: BoxFuture<'static, ()>) -> AbortHandle {
            self.0.spawn.fetch_add(1, Ordering::Relaxed);
            AbortHandle::noop()
        }
        fn spawn_detached(&self, _future: BoxFuture<'static, ()>) {
            self.0.detached.fetch_add(1, Ordering::Relaxed);
        }
        fn sleep(&self, _duration: Duration) -> BoxFuture<'static, ()> {
            self.0.sleep.fetch_add(1, Ordering::Relaxed);
            Box::pin(async {})
        }
        fn spawn_blocking(&self, job: BlockingJob) -> BoxFuture<'static, ()> {
            self.0.blocking.fetch_add(1, Ordering::Relaxed);
            job();
            Box::pin(async {})
        }
        fn yield_now(&self) -> Option<BoxFuture<'static, ()>> {
            self.0.yield_now.fetch_add(1, Ordering::Relaxed);
            Some(Box::pin(async {}))
        }
        fn yield_frequency(&self) -> u32 {
            77
        }
    }

    #[tokio::test]
    async fn execution_ports_preserve_the_inner_entry_points() {
        let calls = Arc::new(Calls::default());
        let runtime = ClockedRuntime::new(
            Arc::new(RecordingRuntime(calls.clone())),
            Arc::new(crate::time::GlobalClock),
        );
        runtime.spawn(Box::pin(async {})).detach();
        runtime.spawn_detached(Box::pin(async {}));
        runtime.sleep(Duration::from_secs(1)).await;
        let completed = Arc::new(AtomicUsize::new(0));
        let job = completed.clone();
        runtime
            .spawn_blocking(Box::new(move || {
                job.fetch_add(1, Ordering::Relaxed);
            }))
            .await;
        runtime.yield_now().unwrap().await;
        assert_eq!(runtime.yield_frequency(), 77);
        assert_eq!(calls.spawn.load(Ordering::Relaxed), 1);
        assert_eq!(calls.detached.load(Ordering::Relaxed), 1);
        assert_eq!(calls.sleep.load(Ordering::Relaxed), 1);
        assert_eq!(calls.blocking.load(Ordering::Relaxed), 1);
        assert_eq!(calls.yield_now.load(Ordering::Relaxed), 1);
        assert_eq!(completed.load(Ordering::Relaxed), 1);
    }
}

use super::{AbortHandle, BoxFuture, Runtime};
use crate::time::Clock;
use std::sync::Arc;
use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
type BlockingJob = Box<dyn FnOnce() + Send + 'static>;
#[cfg(target_arch = "wasm32")]
type BlockingJob = Box<dyn FnOnce() + 'static>;

/// A clock selected by one host instance. The inner executor's timers must
/// advance consistently with the selected monotonic clock.
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
        self.inner.sleep(duration)
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

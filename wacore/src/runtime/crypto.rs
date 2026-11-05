//! Caller-owned symmetric crypto selection for a runtime instance.
use super::{AbortHandle, BoxFuture, Runtime};
use crate::libsignal::crypto::SymmetricCryptoProvider;
use std::{sync::Arc, time::Duration};

/// Decorate an executor with a symmetric backend for Signal and buffered media.
/// The provider belongs to this runtime; other clients keep their own selection.
pub struct CryptoRuntime {
    inner: Arc<dyn Runtime>,
    provider: Arc<dyn SymmetricCryptoProvider>,
}
impl CryptoRuntime {
    pub fn new(inner: Arc<dyn Runtime>, provider: Arc<dyn SymmetricCryptoProvider>) -> Self {
        Self { inner, provider }
    }
}
impl Runtime for CryptoRuntime {
    fn crypto_provider(&self) -> Option<Arc<dyn SymmetricCryptoProvider>> {
        Some(Arc::clone(&self.provider))
    }
    #[cfg(feature = "instance-clock")]
    fn clock(&self) -> Option<&Arc<dyn crate::time::Clock>> {
        self.inner.clock()
    }
    fn spawn(&self, future: BoxFuture<'static, ()>) -> AbortHandle {
        self.inner.spawn(future)
    }
    fn spawn_detached(&self, future: BoxFuture<'static, ()>) {
        self.inner.spawn_detached(future)
    }
    fn sleep(&self, duration: Duration) -> BoxFuture<'static, ()> {
        self.inner.sleep(duration)
    }
    #[cfg(not(target_arch = "wasm32"))]
    fn spawn_blocking(&self, work: Box<dyn FnOnce() + Send + 'static>) -> BoxFuture<'static, ()> {
        self.inner.spawn_blocking(work)
    }
    #[cfg(target_arch = "wasm32")]
    fn spawn_blocking(&self, work: Box<dyn FnOnce() + 'static>) -> BoxFuture<'static, ()> {
        self.inner.spawn_blocking(work)
    }
    fn yield_now(&self) -> Option<BoxFuture<'static, ()>> {
        self.inner.yield_now()
    }
    fn yield_frequency(&self) -> u32 {
        self.inner.yield_frequency()
    }
}

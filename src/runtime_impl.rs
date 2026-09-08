/// Tokio-based implementation of [`wacore::runtime::Runtime`]. Only available on native targets.
#[cfg(not(target_arch = "wasm32"))]
mod tokio_impl {
    use std::future::Future;
    use std::pin::Pin;
    use std::time::Duration;

    use async_trait::async_trait;
    use wacore::runtime::{AbortHandle, Runtime};

    /// Adapter for an existing Tokio executor, available with `tokio-runtime`
    /// on non-wasm32 targets. Does not create or keep an executor alive.
    ///
    /// Works with current-thread and multi-thread executors. `spawn` and
    /// `spawn_detached` use the current executor when called; `sleep` requires
    /// its timer driver to be enabled when the sleep future is constructed.
    /// `spawn_blocking` uses the current executor when its future is first polled.
    ///
    /// - `spawn` returns an [`AbortHandle`] that requests cancellation on drop.
    ///   Cancellation does not wait for destruction or preempt a running poll.
    /// - `spawn_detached` and [`AbortHandle::detach`] let a task outlive its
    ///   handle's scope, but do not guarantee completion across executor shutdown.
    /// - `spawn_blocking` submits nothing until polled. Once submitted, dropping
    ///   its future leaves queued or running work alive. Blocking work runs on
    ///   separate threads even with a current-thread executor; started work can
    ///   delay executor shutdown until it finishes.
    /// - `yield_now` is a no-op returning `None`, with inherited frequency 10,
    ///   on both executor flavors. It provides no scheduling point or fairness
    ///   guarantee. Tight loops must arrange their own cooperative yielding;
    ///   multiple workers do not eliminate that need.
    ///
    /// # Panics
    ///
    /// Missing Tokio context panics at the call or first poll described above.
    /// Constructing a sleep also panics if the timer driver is disabled.
    /// With unwinding and Tokio's default panic policy, async task panics are
    /// caught by Tokio and not returned by this adapter. `spawn_blocking` also
    /// discards its join error, so a closure panic resolves as `()`; the panic
    /// hook still runs. Use [`wacore::runtime::blocking`] to return a closure's
    /// result; it raises a new panic if the closure fails to deliver that result.
    /// With `panic = "abort"`, a panic terminates the process instead.
    ///
    /// # Example
    ///
    /// No `Client` is needed. Disable defaults to omit the bundled storage,
    /// transport and HTTP adapters; this still compiles the main crate and core.
    ///
    /// ```toml
    /// [dependencies]
    /// whatsapp-rust = { version = "0.7", default-features = false, features = ["tokio-runtime"] }
    /// tokio = { version = "1", features = ["rt", "time"] }
    /// ```
    ///
    /// ```rust
    /// use std::time::Duration;
    /// use whatsapp_rust::{Runtime, TokioRuntime};
    ///
    /// fn main() -> Result<(), Box<dyn std::error::Error>> {
    ///     let executor = tokio::runtime::Builder::new_current_thread()
    ///         .enable_time()
    ///         .build()?;
    ///     let runtime = TokioRuntime;
    ///     executor.block_on(async {
    ///         runtime.sleep(Duration::from_millis(1)).await;
    ///         let answer = whatsapp_rust::wacore::runtime::blocking(&runtime, || 6 * 7).await;
    ///         assert_eq!(answer, 42);
    ///     });
    ///     Ok(())
    /// }
    /// ```
    pub struct TokioRuntime;

    #[async_trait]
    impl Runtime for TokioRuntime {
        fn spawn(&self, future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>) -> AbortHandle {
            let handle = tokio::spawn(future);
            AbortHandle::new(move || handle.abort())
        }

        fn spawn_detached(&self, future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>) {
            tokio::spawn(future);
        }

        fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            Box::pin(tokio::time::sleep(duration))
        }

        fn spawn_blocking(
            &self,
            f: Box<dyn FnOnce() + Send + 'static>,
        ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            Box::pin(async {
                let _ = tokio::task::spawn_blocking(f).await;
            })
        }

        fn yield_now(&self) -> Option<Pin<Box<dyn Future<Output = ()> + Send>>> {
            None
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub use tokio_impl::TokioRuntime;

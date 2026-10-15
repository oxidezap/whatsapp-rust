//! Runtime and cancellation bounds remain usable outside the workspace.
#[cfg(any(test, target_arch = "wasm32"))]
use wacore::runtime::{AbortHandle, Runtime};

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn native_thread_bounds() {
    fn send_sync<T: Send + Sync + ?Sized>() {}
    send_sync::<dyn Runtime>();
    send_sync::<AbortHandle>();
    send_sync::<wacore::stats::InstrumentedRuntime>();
}

#[cfg(target_arch = "wasm32")]
mod local {
    use super::*;
    use std::{cell::Cell, future::Future, pin::Pin, rc::Rc, sync::Arc, time::Duration};
    use wacore::stats::{InstrumentedRuntime, TaskInstrument};

    struct LocalInstrument(Rc<Cell<usize>>);

    impl TaskInstrument for LocalInstrument {
        fn on_poll_start(&self) {
            self.0.set(self.0.get() + 1);
        }
        fn on_poll_end(&self) {}
    }

    pub struct LocalRuntime(pub Rc<Cell<usize>>);

    impl Runtime for LocalRuntime {
        fn spawn(&self, future: Pin<Box<dyn Future<Output = ()> + 'static>>) -> AbortHandle {
            // A host may retain a local task or JS cancellation object here.
            let calls = self.0.clone();
            AbortHandle::new(move || {
                drop(future);
                calls.set(calls.get() + 1);
            })
        }

        fn sleep(&self, _: Duration) -> Pin<Box<dyn Future<Output = ()>>> {
            Box::pin(async {})
        }

        fn spawn_blocking(
            &self,
            f: Box<dyn FnOnce() + 'static>,
        ) -> Pin<Box<dyn Future<Output = ()>>> {
            Box::pin(async move { f() })
        }

        fn yield_now(&self) -> Option<Pin<Box<dyn Future<Output = ()>>>> {
            None
        }
    }

    pub fn local_runtime_contract() {
        let calls = Rc::new(Cell::new(0));
        let runtime: Box<dyn Runtime> = Box::new(LocalRuntime(calls.clone()));
        let captured = calls.clone();
        let handle = runtime.spawn(Box::pin(async move { captured.set(100) }));
        handle.abort();
        handle.abort();
        drop(handle);
        assert_eq!(calls.get(), 1);
        runtime.spawn(Box::pin(async {})).detach();
        assert_eq!(calls.get(), 1);
        drop(runtime.spawn(Box::pin(async {})));
        assert_eq!(calls.get(), 2);
        let metered = InstrumentedRuntime::new(
            Arc::new(LocalRuntime(calls.clone())),
            Arc::new(LocalInstrument(Rc::new(Cell::new(0)))),
        );
        drop(metered.spawn(Box::pin(async {})));
        assert_eq!(calls.get(), 3);
    }
}

#[cfg(target_arch = "wasm32")]
pub use local::local_runtime_contract;

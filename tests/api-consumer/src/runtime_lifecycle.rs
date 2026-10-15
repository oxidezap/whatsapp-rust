//! Execution contracts exercised exclusively through downstream public APIs.

#[cfg(all(feature = "native", not(target_arch = "wasm32")))]
mod native {
    use futures::channel::oneshot;
    use std::{future::Future, pin::Pin, sync::Arc, time::Duration};
    use wacore::runtime::{AbortHandle, BoxFuture, Runtime, timeout};
    use whatsapp_rust::TokioRuntime;

    const BUDGET: Duration = Duration::from_secs(5);

    struct Dropped(Option<oneshot::Sender<()>>);
    impl Drop for Dropped {
        fn drop(&mut self) {
            if let Some(tx) = self.0.take() {
                let _ = tx.send(());
            }
        }
    }

    async fn received<T>(rx: oneshot::Receiver<T>) -> T {
        tokio::time::timeout(BUDGET, rx).await.unwrap().unwrap()
    }

    // An external implementer can delegate to the shipped executor while
    // retaining the trait's default spawn_detached implementation.
    struct ExternalRuntime;
    impl Runtime for ExternalRuntime {
        fn spawn(&self, future: BoxFuture<'static, ()>) -> AbortHandle {
            TokioRuntime.spawn(future)
        }
        fn sleep(&self, duration: Duration) -> BoxFuture<'static, ()> {
            TokioRuntime.sleep(duration)
        }
        fn spawn_blocking(&self, f: Box<dyn FnOnce() + Send + 'static>) -> BoxFuture<'static, ()> {
            TokioRuntime.spawn_blocking(f)
        }
        fn yield_now(&self) -> Option<Pin<Box<dyn Future<Output = ()> + Send>>> {
            None
        }
    }

    #[tokio::test]
    async fn default_and_optimized_detached_paths_execute() {
        for runtime in [&ExternalRuntime as &dyn Runtime, &TokioRuntime] {
            let (tx, rx) = oneshot::channel();
            runtime.spawn_detached(Box::pin(async move { tx.send(42).unwrap() }));
            assert_eq!(received(rx).await, 42);
        }
    }

    #[tokio::test]
    async fn timeout_cancels_owned_child_but_detached_child_survives() {
        for detach in [false, true] {
            let (started_tx, started_rx) = oneshot::channel();
            let (drop_tx, drop_rx) = oneshot::channel();
            let (release_tx, release_rx) = oneshot::channel::<()>();
            let dropped = Dropped(Some(drop_tx));
            let handle = TokioRuntime.spawn(Box::pin(async move {
                let _dropped = dropped;
                started_tx.send(()).unwrap();
                let _ = release_rx.await;
            }));
            received(started_rx).await;
            let parent = async move {
                let _owned = if detach {
                    handle.detach();
                    None
                } else {
                    Some(handle)
                };
                std::future::pending::<()>().await;
            };
            assert!(
                timeout(&TokioRuntime, Duration::from_millis(20), parent)
                    .await
                    .is_err()
            );
            let mut drop_rx = drop_rx;
            if detach {
                assert!(drop_rx.try_recv().unwrap().is_none());
                release_tx.send(()).unwrap();
            }
            received(drop_rx).await;
        }
    }

    #[test]
    fn executor_teardown_destroys_task_even_with_handle_retained() {
        for detach in [false, true] {
            let executor = tokio::runtime::Builder::new_current_thread()
                .enable_time()
                .build()
                .unwrap();
            let (drop_tx, mut drop_rx) = oneshot::channel();
            let dropped = Dropped(Some(drop_tx));
            let handle = executor.block_on(async {
                let (started_tx, started_rx) = oneshot::channel();
                let handle = TokioRuntime.spawn(Box::pin(async move {
                    let _dropped = dropped;
                    started_tx.send(()).unwrap();
                    std::future::pending::<()>().await;
                }));
                received(started_rx).await;
                handle
            });
            let retained = if detach {
                handle.detach();
                None
            } else {
                Some(handle)
            };
            drop(executor);
            assert_eq!(drop_rx.try_recv().unwrap(), Some(()));
            // Neither a retained handle nor detach keeps Tokio alive.
            drop(retained);
        }
    }

    #[tokio::test]
    async fn cancelling_blocking_waiter_does_not_cancel_started_work() {
        let (started_tx, started_rx) = oneshot::channel();
        let (finished_tx, finished_rx) = oneshot::channel();
        let (release_tx, release_rx) = std::sync::mpsc::channel();
        let runtime: Arc<dyn Runtime> = Arc::new(ExternalRuntime);
        let (cancelled_tx, cancelled_rx) = oneshot::channel();
        let waiter_dropped = Dropped(Some(cancelled_tx));
        let handle = runtime.spawn(Box::pin(async move {
            let _waiter_dropped = waiter_dropped;
            TokioRuntime
                .spawn_blocking(Box::new(move || {
                    started_tx.send(()).unwrap();
                    // Bounded even if an assertion on the async side fails.
                    release_rx.recv_timeout(BUDGET).unwrap();
                    finished_tx.send(()).unwrap();
                }))
                .await;
        }));
        received(started_rx).await;
        handle.abort();
        received(cancelled_rx).await;
        release_tx.send(()).unwrap();
        received(finished_rx).await;
    }
}

#[cfg(all(feature = "runtime-execution", target_arch = "wasm32"))]
mod local {
    use futures::{
        executor::{LocalPool, LocalSpawner},
        task::LocalSpawnExt,
    };
    use std::{
        cell::{Cell, RefCell},
        future::Future,
        pin::Pin,
        rc::Rc,
        time::Duration,
    };
    use wacore::runtime::{AbortHandle, Runtime, ShutdownNotifier, wait_for_shutdown};

    struct LocalRuntime {
        spawner: LocalSpawner,
        cancellations: Rc<Cell<usize>>,
    }
    impl Runtime for LocalRuntime {
        fn spawn(&self, future: Pin<Box<dyn Future<Output = ()> + 'static>>) -> AbortHandle {
            let (abort, registration) = futures::future::AbortHandle::new_pair();
            self.spawner
                .spawn_local(async move {
                    let _ = futures::future::Abortable::new(future, registration).await;
                })
                .unwrap();
            let cancellations = self.cancellations.clone();
            AbortHandle::new(move || {
                cancellations.set(cancellations.get() + 1);
                abort.abort();
            })
        }
        fn sleep(&self, _: Duration) -> Pin<Box<dyn Future<Output = ()>>> {
            Box::pin(std::future::pending())
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

    #[wasm_bindgen_test::wasm_bindgen_test]
    fn local_tasks_cancel_detach_and_observe_shutdown() {
        let mut pool = LocalPool::new();
        let calls = Rc::new(Cell::new(0));
        let runtime: Box<dyn Runtime> = Box::new(LocalRuntime {
            spawner: pool.spawner(),
            cancellations: calls.clone(),
        });
        let shutdown = ShutdownNotifier::new();
        let completed = Rc::new(RefCell::new(Vec::new()));
        let mut handles = Vec::new();
        for index in 0..3 {
            let signal = shutdown.subscribe();
            let completed = completed.clone();
            handles.push(runtime.spawn(Box::pin(async move {
                wait_for_shutdown(&signal).await;
                completed.borrow_mut().push(index);
            })));
        }
        pool.run_until_stalled();
        handles.pop().unwrap().detach();
        handles.pop().unwrap().abort();
        drop(handles);
        pool.run_until_stalled();
        assert_eq!(calls.get(), 2);
        shutdown.notify();
        drop(shutdown);
        drop(runtime);
        pool.run_until_stalled();
        assert_eq!(*completed.borrow(), vec![2]);
    }
}

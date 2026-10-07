use super::*;
use crate::store::traits::*;
use std::sync::atomic::{AtomicBool, Ordering};
use wacore::runtime::{AbortHandle, BoxFuture, Runtime};
use wacore::store::error::Result;

struct DropProbe {
    dropped: Arc<AtomicBool>,
    block: Option<(
        std::sync::mpsc::Sender<()>,
        std::sync::Mutex<std::sync::mpsc::Receiver<()>>,
    )>,
}
impl Drop for DropProbe {
    fn drop(&mut self) {
        if let Some((entered, gate)) = &self.block {
            entered.send(()).unwrap();
            gate.lock().unwrap().recv().unwrap();
        }
        self.dropped.store(true, Ordering::SeqCst);
    }
}

pub(crate) struct ProbeBackend {
    pub(crate) backend: Arc<dyn Backend>,
    _probe: DropProbe,
    pub(crate) gate: Option<async_channel::Receiver<()>>,
    pub(crate) entered: Option<async_channel::Sender<()>>,
    pub(crate) gate_method: &'static str,
    fail_save: bool,
    pub(crate) fail_maintenance: bool,
}

impl ProbeBackend {
    pub(crate) fn new() -> (Self, Arc<AtomicBool>) {
        let dropped = Arc::new(AtomicBool::new(false));
        (
            Self {
                backend: Arc::new(wacore::store::in_memory::InMemoryBackend::new()),
                _probe: DropProbe {
                    dropped: dropped.clone(),
                    block: None,
                },
                gate: None,
                entered: None,
                gate_method: "save",
                fail_save: false,
                fail_maintenance: false,
            },
            dropped,
        )
    }

    pub(crate) async fn before_call(&self, method: &str) {
        if method != self.gate_method {
            return;
        }
        if let Some(entered) = &self.entered {
            entered.send(()).await.expect("announce I/O");
        }
        if let Some(gate) = &self.gate {
            gate.recv().await.expect("unpark I/O");
        }
    }
}

#[async_trait::async_trait]
impl DeviceStore for ProbeBackend {
    async fn save(&self, device: &wacore::store::Device) -> Result<()> {
        self.before_call("save").await;
        if self.fail_save {
            return Err(wacore::store::error::StoreError::Io(std::io::Error::other(
                "synthetic save failure",
            )));
        }
        self.backend.save(device).await
    }
    async fn maintenance(&self) -> Result<()> {
        self.before_call("maintenance").await;
        if self.fail_maintenance {
            return Err(wacore::store::error::StoreError::Validation(
                "synthetic maintenance failure".into(),
            ));
        }
        self.backend.maintenance().await
    }
    async fn load(&self) -> Result<Option<wacore::store::Device>> {
        self.backend.load().await
    }
    async fn exists(&self) -> Result<bool> {
        self.backend.exists().await
    }
    async fn create(&self) -> Result<i32> {
        self.backend.create().await
    }
}

pub(crate) struct SaverRuntime {
    pub(crate) future: std::sync::Mutex<Option<BoxFuture<'static, ()>>>,
    pub(crate) aborted: Arc<AtomicBool>,
}
impl SaverRuntime {
    pub(crate) fn new() -> Self {
        Self {
            future: std::sync::Mutex::new(None),
            aborted: Arc::new(AtomicBool::new(false)),
        }
    }
}
#[async_trait::async_trait]
impl Runtime for SaverRuntime {
    fn spawn(&self, future: BoxFuture<'static, ()>) -> AbortHandle {
        assert!(self.future.lock().unwrap().replace(future).is_none());
        let aborted = self.aborted.clone();
        // Deliberately retain the future after abort request, like an executor
        // whose cancellation acknowledgement arrives later.
        AbortHandle::new(move || {
            aborted.store(true, Ordering::SeqCst);
        })
    }
    fn sleep(&self, duration: std::time::Duration) -> BoxFuture<'static, ()> {
        Box::pin(tokio::time::sleep(duration))
    }
    fn spawn_blocking(&self, f: Box<dyn FnOnce() + Send + 'static>) -> BoxFuture<'static, ()> {
        Box::pin(async move {
            tokio::task::spawn_blocking(f).await.unwrap();
        })
    }
    fn yield_now(&self) -> Option<BoxFuture<'static, ()>> {
        Some(Box::pin(tokio::task::yield_now()))
    }
}

#[tokio::test]
async fn shutdown_does_not_release_a_detached_client_owner() {
    let (backend, dropped) = ProbeBackend::new();
    let client = crate::test_utils::create_test_client_with_backend(Arc::new(backend)).await;
    let weak = Arc::downgrade(&client);
    let release = client.store_release();
    let mut waiter = Box::pin(release.wait());
    assert!(futures::poll!(waiter.as_mut()).is_pending());
    let (unpark, parked) = async_channel::bounded(1);
    let (finished, done) = async_channel::bounded(1);
    let detached = client.clone();
    client
        .runtime
        .spawn(Box::pin(async move {
            parked.recv().await.unwrap();
            drop(detached);
            finished.send(()).await.unwrap();
        }))
        .detach();

    let report = client.shutdown().await;
    assert!(report.device.is_ok());
    assert!(matches!(
        client.run().await,
        crate::RunCompletionReason::ShutdownRequested
    ));
    drop(client);
    assert!(
        weak.upgrade().is_some(),
        "shutdown/run returned with detached owner alive"
    );
    assert!(!dropped.load(Ordering::SeqCst));
    assert!(futures::poll!(waiter.as_mut()).is_pending());
    unpark.send(()).await.unwrap();
    done.recv().await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), waiter)
        .await
        .unwrap();
    assert!(dropped.load(Ordering::SeqCst));
    assert!(weak.upgrade().is_none());
    release.wait().await;
}

#[tokio::test]
async fn logout_retains_local_flush_failure_and_does_not_claim_backend_release() {
    use crate::store::commands::DeviceCommand;
    let (mut backend, dropped) = ProbeBackend::new();
    backend.fail_save = true;
    let client = crate::test_utils::create_test_client_with_backend(Arc::new(backend)).await;
    let weak = Arc::downgrade(&client);
    let release = client.store_release();
    let mut waiter = Box::pin(release.wait());
    client
        .persistence_manager
        .process_command(DeviceCommand::SetPushName("synthetic logout".into()))
        .await;
    let report = tokio::time::timeout(std::time::Duration::from_secs(5), client.logout())
        .await
        .expect("offline logout should finish despite failed persistence");
    assert!(matches!(
        report.deregistration,
        crate::DeregistrationOutcome::NotAttempted(crate::DeregistrationSkipReason::Offline)
    ));
    let Err(wacore::store::error::StoreError::Io(source)) = &report.shutdown.device else {
        panic!("local save failure must retain its original type and source");
    };
    assert_eq!(source.to_string(), "synthetic save failure");
    assert_eq!(report.shutdown.outbound, crate::DrainOutcome::Completed);
    assert!(matches!(
        client.run().await,
        crate::RunCompletionReason::ShutdownRequested
    ));
    assert!(futures::poll!(waiter.as_mut()).is_pending());
    assert!(
        !dropped.load(Ordering::SeqCst),
        "caller Arc still owns the backend"
    );
    drop(client);
    tokio::time::timeout(std::time::Duration::from_secs(5), waiter)
        .await
        .expect("backend lease should end after its owners drop");
    assert!(weak.upgrade().is_none());
    assert!(dropped.load(Ordering::SeqCst));
    // A held report/observer carries observations, not ownership of the store.
    assert!(report.shutdown.device.is_err());
    release.wait().await;
}

#[tokio::test]
async fn host_raw_handles_keep_their_identity_without_pinning_observation() {
    use crate::store::{Device, persistence_manager::PersistenceManager};
    let (backend, dropped) = ProbeBackend::new();
    let original: Arc<dyn Backend> = Arc::new(backend);
    let pm = PersistenceManager::new(original.clone()).await.unwrap();
    let snapshot = pm.get_device_snapshot();
    let second_snapshot = pm.get_device_snapshot();
    assert!(Arc::ptr_eq(&snapshot, &second_snapshot));
    let literal = Device {
        core: snapshot.core.clone(),
        backend: original.clone(),
    };
    assert!(Arc::ptr_eq(&original, &pm.backend()));
    assert!(Arc::ptr_eq(&original, &snapshot.backend));
    let count = Arc::strong_count(&original);
    let release = pm.store_release();
    let clone = release.clone();
    let unpolled = release.wait();
    let mut cancelled = Box::pin(clone.wait());
    assert!(futures::poll!(cancelled.as_mut()).is_pending());
    assert_eq!(Arc::strong_count(&original), count);
    drop(cancelled);
    drop(pm);
    unpolled.await;
    clone.wait().await;
    assert!(
        !dropped.load(Ordering::SeqCst),
        "host-owned handles remain outside the boundary"
    );
    drop((snapshot, second_snapshot, literal, original));
    assert!(dropped.load(Ordering::SeqCst));
}

#[tokio::test]
async fn observer_waits_until_the_actual_backend_destructor_returns() {
    use crate::store::persistence_manager::PersistenceManager;
    let (mut backend, dropped) = ProbeBackend::new();
    let (entered_tx, entered_rx) = std::sync::mpsc::channel();
    let (unpark_tx, unpark_rx) = std::sync::mpsc::channel();
    backend._probe.block = Some((entered_tx, std::sync::Mutex::new(unpark_rx)));
    let pm = PersistenceManager::new(Arc::new(backend)).await.unwrap();
    let mut waiter = Box::pin(pm.store_release().wait());
    let destroy = std::thread::spawn(move || drop(pm));
    entered_rx
        .recv_timeout(std::time::Duration::from_secs(5))
        .unwrap();
    assert!(!dropped.load(Ordering::SeqCst));
    assert!(
        futures::poll!(waiter.as_mut()).is_pending(),
        "Drop entry is not backend release"
    );
    unpark_tx.send(()).unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), waiter)
        .await
        .unwrap();
    assert!(dropped.load(Ordering::SeqCst));
    destroy.join().unwrap();
}

#[tokio::test]
async fn multiple_late_and_racing_waiters_never_lose_release() {
    for _ in 0..64 {
        let (backend, dropped) = ProbeBackend::new();
        let owner = BackendLease::new(Arc::new(backend));
        let release = owner.observer();
        let cancelled = release.wait();
        drop(cancelled);
        let first = release.wait();
        let second = release.clone().wait();
        let destroy = std::thread::spawn(move || drop(owner));
        let racing = release.wait();
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            futures::join!(first, second, racing);
        })
        .await
        .unwrap();
        assert!(dropped.load(Ordering::SeqCst));
        destroy.join().unwrap();
        release.wait().await;
    }
}

#[tokio::test]
async fn unpolled_backend_job_retains_ownership_until_destroyed() {
    use crate::store::persistence_manager::PersistenceManager;
    let (backend, dropped) = ProbeBackend::new();
    let pm = PersistenceManager::new(Arc::new(backend)).await.unwrap();
    let lease = pm.backend_lease();
    let mut waiter = Box::pin(pm.store_release().wait());
    let job = Box::pin(async move { lease.maintenance().await });
    drop(pm);
    assert!(futures::poll!(waiter.as_mut()).is_pending());
    assert!(!dropped.load(Ordering::SeqCst));
    drop(job);
    waiter.await;
    assert!(dropped.load(Ordering::SeqCst));
}

#[tokio::test]
async fn cancelled_save_driver_and_failed_flush_release_on_manager_drop() {
    use crate::store::{commands::DeviceCommand, persistence_manager::PersistenceManager};
    for fail in [false, true] {
        let (mut backend, dropped) = ProbeBackend::new();
        let (unpark, parked) = async_channel::bounded(1);
        let (entered, entry) = async_channel::bounded(1);
        backend.gate = Some(parked);
        backend.entered = Some(entered);
        backend.fail_save = fail;
        let pm = PersistenceManager::new(Arc::new(backend)).await.unwrap();
        let mut waiter = Box::pin(pm.store_release().wait());
        pm.process_command(DeviceCommand::SetPushName("synthetic".into()))
            .await;
        let mut flush = Box::pin(pm.flush());
        assert!(futures::poll!(flush.as_mut()).is_pending());
        entry.try_recv().unwrap();
        if fail {
            unpark.try_send(()).unwrap();
            let error = flush.await.unwrap_err();
            let wacore::store::error::StoreError::Io(source) = error else {
                panic!("original I/O variant lost")
            };
            assert_eq!(source.to_string(), "synthetic save failure");
        } else {
            drop(flush); // save remains in the manager, not the cancelled driver
            assert!(!unpark.is_closed());
        }
        assert!(futures::poll!(waiter.as_mut()).is_pending());
        drop(pm);
        waiter.await;
        assert!(dropped.load(Ordering::SeqCst));
    }
}

#[tokio::test]
async fn app_state_processor_keeps_its_manager_lease_after_client_drop() {
    let (backend, dropped) = ProbeBackend::new();
    let original: Arc<dyn Backend> = Arc::new(backend);
    let client = crate::test_utils::create_test_client_with_backend(original.clone()).await;
    let release = client.store_release();
    let processor = client.get_app_state_processor().clone();
    assert!(Arc::ptr_eq(&original, &processor.backend));
    drop(original);
    drop(client);
    let mut waiter = Box::pin(release.wait());
    assert!(futures::poll!(waiter.as_mut()).is_pending());
    assert!(!dropped.load(Ordering::SeqCst));
    drop(processor);
    tokio::time::timeout(std::time::Duration::from_secs(5), waiter)
        .await
        .unwrap();
    assert!(dropped.load(Ordering::SeqCst));
}

#[tokio::test]
async fn message_secret_worker_keeps_its_lease_after_client_drop() {
    let (mut backend, dropped) = ProbeBackend::new();
    let (unpark, parked) = async_channel::bounded(1);
    let (entered, entry) = async_channel::bounded(1);
    backend.gate_method = "put_msg_secrets";
    backend.gate = Some(parked);
    backend.entered = Some(entered);
    let client = crate::test_utils::create_test_client_with_backend(Arc::new(backend)).await;
    let weak = Arc::downgrade(&client);
    let release = client.store_release();
    client
        .msg_secret_buffer
        .queue_one(MsgSecretEntry {
            chat: Arc::from("15550000001@s.whatsapp.net"),
            sender: Arc::from("15550000002@s.whatsapp.net"),
            msg_id: Arc::from("synthetic-secret"),
            secret: [7; 32],
            expires_at: 0,
            message_ts: 1,
        })
        .await;
    tokio::time::timeout(std::time::Duration::from_secs(5), entry.recv())
        .await
        .unwrap()
        .unwrap();
    drop(client);
    crate::test_utils::poll_until("last client reference", || weak.upgrade().is_none()).await;
    let mut waiter = Box::pin(release.wait());
    assert!(futures::poll!(waiter.as_mut()).is_pending());
    assert!(!dropped.load(Ordering::SeqCst));
    unpark.send(()).await.unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(5), waiter)
        .await
        .unwrap();
    assert!(dropped.load(Ordering::SeqCst));
}

#[tokio::test]
async fn sent_message_backend_workers_keep_their_lease_after_client_drop() {
    for method in ["store_sent_message", "take_sent_message"] {
        let (mut backend, dropped) = ProbeBackend::new();
        let (unpark, parked) = async_channel::bounded(1);
        let (entered, entry) = async_channel::bounded(1);
        backend.gate_method = method;
        backend.gate = Some(parked);
        backend.entered = Some(entered);
        let pm = Arc::new(
            crate::store::persistence_manager::PersistenceManager::new(Arc::new(backend))
                .await
                .unwrap(),
        );
        let mut config = crate::cache_config::CacheConfig::default();
        config.recent_messages.capacity = 16;
        let (client, _rx) = crate::Client::builder()
            .with_runtime_arc(Arc::new(crate::runtime_impl::TokioRuntime))
            .with_persistence_manager(pm)
            .with_transport_factory_arc(Arc::new(
                crate::transport::mock::MockTransportFactory::new(),
            ))
            .with_http_client_arc(Arc::new(crate::test_utils::MockHttpClient))
            .with_cache_config(config)
            .build()
            .await
            .expect("test client should build")
            .into_parts();
        client.enter_live_mode_for_tests();
        assert!(client.cache_config.recent_messages_enabled);
        let weak = Arc::downgrade(&client);
        let release = client.store_release();
        let chat = crate::Jid::pn("15550000001");
        let message = waproto::whatsapp::Message {
            conversation: Some("synthetic".into()),
            ..Default::default()
        };
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            client.add_recent_message(&chat, "synthetic-sent", &message, None),
        )
        .await
        .unwrap();
        if method == "take_sent_message" {
            assert!(
                tokio::time::timeout(
                    std::time::Duration::from_secs(5),
                    client.take_recent_message(&chat, "synthetic-sent")
                )
                .await
                .unwrap()
                .is_some()
            );
        }
        tokio::time::timeout(std::time::Duration::from_secs(5), entry.recv())
            .await
            .unwrap()
            .unwrap();
        drop(client);
        crate::test_utils::poll_until("last client reference", || weak.upgrade().is_none()).await;
        let mut waiter = Box::pin(release.wait());
        assert!(futures::poll!(waiter.as_mut()).is_pending());
        assert!(!dropped.load(Ordering::SeqCst));
        unpark.send(()).await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(5), waiter)
            .await
            .unwrap();
        assert!(dropped.load(Ordering::SeqCst));
    }
}

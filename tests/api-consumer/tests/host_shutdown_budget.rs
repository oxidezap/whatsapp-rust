//! Host timeouts keep shutdown pinned while pending saves finish.
#![cfg(all(feature = "native", not(target_arch = "wasm32")))]

use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};
use std::time::Duration;
use wacore::store::{InMemoryBackend, error::Result, traits::DeviceStore};
use whatsapp_rust::store::{commands::DeviceCommand, persistence_manager::PersistenceManager};
use whatsapp_rust::{Client, DrainOutcome, TokioRuntime};

#[path = "support/forward_backend.rs"]
mod forwarding;
#[path = "support/offline.rs"]
mod offline;

struct PendingBackend {
    inner: InMemoryBackend,
    parked: AtomicBool,
    resume: async_channel::Receiver<()>,
    entered: async_channel::Sender<()>,
    commits: Mutex<Vec<String>>,
}

impl PendingBackend {
    fn new(
        parked: bool,
        capacity: usize,
    ) -> (
        Arc<Self>,
        async_channel::Sender<()>,
        async_channel::Receiver<()>,
    ) {
        let (resume, gate) = async_channel::bounded(capacity);
        let (entered, began) = async_channel::bounded(capacity);
        let backend = Arc::new(Self {
            inner: InMemoryBackend::new(),
            parked: AtomicBool::new(parked),
            resume: gate,
            entered,
            commits: Mutex::new(Vec::new()),
        });
        (backend, resume, began)
    }
}

#[async_trait::async_trait]
impl DeviceStore for PendingBackend {
    async fn save(&self, device: &wacore::store::Device) -> Result<()> {
        if self.parked.load(Ordering::SeqCst) {
            self.entered.send(()).await.unwrap();
            self.resume.recv().await.unwrap();
        }
        self.inner.save(device).await?;
        self.commits.lock().unwrap().push(device.push_name.clone());
        Ok(())
    }
    async fn load(&self) -> Result<Option<wacore::store::Device>> {
        self.inner.load().await
    }
    async fn exists(&self) -> Result<bool> {
        self.inner.exists().await
    }
    async fn create(&self) -> Result<i32> {
        self.inner.create().await
    }
}

#[tokio::test]
async fn host_deadline_keeps_shutdown_owned_until_save_and_release_finish() {
    let (backend, resume, began) = PendingBackend::new(false, 1);
    let manager = Arc::new(PersistenceManager::new(backend.clone()).await.unwrap());
    let client = Client::builder()
        .with_persistence_manager(manager.clone())
        .with_runtime(TokioRuntime)
        .with_http_client(offline::OfflineHttp)
        .with_transport_factory(offline::OfflineTransport)
        .with_version_override((2, 3000, 1))
        .build()
        .await
        .unwrap()
        .into_client();
    manager.flush().await.unwrap();
    manager
        .process_command(DeviceCommand::SetPushName("before deadline".into()))
        .await;
    backend.parked.store(true, Ordering::SeqCst);
    let release = client.store_release();
    let mut released = Box::pin(release.wait());
    let mut shutdown = Box::pin(client.shutdown());
    tokio::select! {
        started = began.recv() => started.unwrap(),
        _ = shutdown.as_mut() => panic!("shutdown completed before the save parked"),
        _ = tokio::time::sleep(Duration::from_secs(2)) => panic!("shutdown did not reach the save"),
    }
    // Borrow the pinned future so expiry stops waiting without cancelling cleanup.
    let observation = tokio::time::timeout(Duration::from_millis(10), shutdown.as_mut()).await;
    assert!(
        observation.is_err(),
        "an elapsed host budget means cleanup pending, with no durability verdict"
    );
    assert!(futures::poll!(released.as_mut()).is_pending());
    assert!(
        !backend
            .commits
            .lock()
            .unwrap()
            .iter()
            .any(|name| name == "before deadline"),
        "the pending save has not committed"
    );
    resume.send(()).await.unwrap();
    let report = tokio::time::timeout(Duration::from_secs(2), shutdown)
        .await
        .unwrap();
    assert!(report.device.is_ok());
    assert_eq!(report.inbound, DrainOutcome::Completed);
    assert_eq!(report.outbound, DrainOutcome::Completed);
    assert_eq!(report.signal_settle, DrainOutcome::Completed);
    assert_eq!(report.message_secrets.failed_batches, 0);
    assert_eq!(
        backend.commits.lock().unwrap().last().unwrap(),
        "before deadline"
    );
    assert!(
        futures::poll!(released.as_mut()).is_pending(),
        "host still owns client and manager"
    );
    drop(client);
    drop(manager);
    tokio::time::timeout(Duration::from_secs(2), released)
        .await
        .unwrap();
    // The raw backend is host-owned and outside StoreRelease's ownership fence.
    assert!(backend.load().await.unwrap().is_some());
}

#[tokio::test]
async fn cancelling_flush_driver_resumes_old_save_before_new_snapshot() {
    let (backend, resume, began) = PendingBackend::new(true, 2);
    let manager = Arc::new(PersistenceManager::new(backend.clone()).await.unwrap());
    let release = manager.store_release();
    let mut released = Box::pin(release.wait());
    manager
        .process_command(DeviceCommand::SetPushName("older".into()))
        .await;
    let mut driver = Box::pin(manager.flush());
    assert!(futures::poll!(driver.as_mut()).is_pending());
    tokio::time::timeout(Duration::from_secs(2), began.recv())
        .await
        .unwrap()
        .unwrap();
    drop(driver);
    manager
        .process_command(DeviceCommand::SetPushName("newer".into()))
        .await;
    let mut retry = Box::pin(manager.flush());
    assert!(futures::poll!(retry.as_mut()).is_pending());
    assert!(
        began.is_empty(),
        "a newer save must not start before the older save commits"
    );
    assert!(futures::poll!(released.as_mut()).is_pending());
    resume.send(()).await.unwrap();
    assert!(futures::poll!(retry.as_mut()).is_pending());
    tokio::time::timeout(Duration::from_secs(2), began.recv())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(*backend.commits.lock().unwrap(), ["older"]);
    assert!(futures::poll!(released.as_mut()).is_pending());
    resume.send(()).await.unwrap();
    retry.await.unwrap();
    assert_eq!(*backend.commits.lock().unwrap(), ["older", "newer"]);
    drop(manager);
    tokio::time::timeout(Duration::from_secs(2), release.wait())
        .await
        .unwrap();
}

#[tokio::test]
async fn store_release_waits_for_saver_worker_after_host_drops_manager() {
    let (backend, resume, began) = PendingBackend::new(true, 1);
    let manager = Arc::new(PersistenceManager::new(backend.clone()).await.unwrap());
    manager
        .process_command(DeviceCommand::SetPushName("worker owned".into()))
        .await;
    let release = manager.store_release();
    let mut released = Box::pin(release.wait());
    let stop = wacore::runtime::ShutdownNotifier::new();
    // Keep the abort handle alive until the durable save finishes.
    let saver = manager.clone().run_background_saver(
        Arc::new(TokioRuntime),
        Duration::from_secs(3600),
        stop.subscribe(),
    );
    tokio::time::timeout(Duration::from_secs(2), began.recv())
        .await
        .unwrap()
        .unwrap();
    drop(manager);
    stop.notify();
    assert!(futures::poll!(released.as_mut()).is_pending());
    assert!(backend.commits.lock().unwrap().is_empty());
    resume.send(()).await.unwrap();
    tokio::time::timeout(Duration::from_secs(2), released)
        .await
        .unwrap();
    assert_eq!(*backend.commits.lock().unwrap(), ["worker owned"]);
    drop(saver);
}

#[tokio::test]
async fn backend_decorator_preserves_bounded_group_reads() {
    use wacore::store::traits::{PendingInboundKey, PendingInboundLease, ProtocolStore};

    let (backend, _, _) = PendingBackend::new(false, 1);
    let chat = "120363000000000001@g.us";
    backend
        .store_pending_inbound(chat, "200:75@lid", "fixture", b"pending")
        .await
        .unwrap();
    let (sender, record) = backend
        .read_pending_inbound_group_bounded(
            PendingInboundKey {
                chat,
                sender: "200@lid",
                id: "fixture",
            },
            16,
            PendingInboundLease::new(Arc::new(())),
        )
        .await
        .unwrap()
        .unwrap();
    assert_eq!(sender, "200:75@lid");
    assert_eq!(record.bytes(), b"pending");
}

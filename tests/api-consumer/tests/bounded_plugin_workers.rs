//! A downstream admission policy and fixed worker pool. Endpoint capacities do
//! not limit total plugin tasks, bytes or endpoints. This adds no SDK scheduler.
#![cfg(all(feature = "native", feature = "plugins", not(target_arch = "wasm32")))]

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};
use std::time::Duration;
use wacore::runtime::wait_for_shutdown;
use whatsapp_rust::{
    Client, ClientPlugin, PluginCapability, PluginContext, PluginEventEndpointConfig,
    PluginEventOverflow, PluginEventPayloadEncoding, PluginEventTopic, PluginEvents, PluginFuture,
    PluginManifest, TokioRuntime, bytes::Bytes,
};

#[path = "support/offline.rs"]
mod offline;

const WORKERS: usize = 2;
const QUEUE: usize = 3;
const MAX_PAYLOAD: usize = 8;
const BYTE_BUDGET: usize = 24;

#[derive(Default)]
struct Counts {
    retained: AtomicUsize,
    copied: AtomicUsize,
    rejected: AtomicUsize,
    started: AtomicUsize,
    finished: AtomicUsize,
    processed: AtomicUsize,
}

struct Job {
    payload: Bytes,
    counts: Arc<Counts>,
}
impl Drop for Job {
    fn drop(&mut self) {
        self.counts
            .retained
            .fetch_sub(self.payload.len(), Ordering::SeqCst);
    }
}
struct Pool {
    queue: async_channel::Sender<Job>,
    counts: Arc<Counts>,
    events: PluginEvents,
}
impl Pool {
    /// DropNewest admission: reject before readiness, after sealing, or when
    /// oversized, before copying/retaining input.
    /// Charge queued AND in-flight jobs. A queued clone would need another lease.
    fn admit(&self, input: &[u8]) -> bool {
        if input.len() > MAX_PAYLOAD
            || self.queue.is_closed()
            || self.counts.started.load(Ordering::SeqCst) != WORKERS
        {
            self.counts.rejected.fetch_add(1, Ordering::SeqCst);
            return false;
        }
        if self
            .counts
            .retained
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |used| {
                used.checked_add(input.len())
                    .filter(|next| *next <= BYTE_BUDGET)
            })
            .is_err()
        {
            self.counts.rejected.fetch_add(1, Ordering::SeqCst);
            return false;
        }
        let job = Job {
            payload: Bytes::copy_from_slice(input),
            counts: self.counts.clone(),
        };
        self.counts.copied.fetch_add(1, Ordering::SeqCst);
        if self.queue.try_send(job).is_err() {
            self.counts.rejected.fetch_add(1, Ordering::SeqCst);
            return false;
        }
        true
    }
    /// A foreign adapter applies its wire ceiling before constructing Bytes.
    /// Endpoint queues share payload storage; endpoint counts remain a separate
    /// adapter policy. The worker byte budget does not cover router retention.
    fn publish_foreign(&self, input: &[u8]) -> anyhow::Result<()> {
        anyhow::ensure!(input.len() <= MAX_PAYLOAD, "foreign payload exceeds limit");
        let payload = Bytes::copy_from_slice(input);
        self.counts.copied.fetch_add(1, Ordering::SeqCst);
        self.events.publish(
            &PluginEventTopic::new("sample")?,
            1,
            PluginEventPayloadEncoding::Binary,
            payload,
        )?;
        Ok(())
    }
    fn close_admission(&self) {
        self.queue.close();
    }
}
struct WorkerPlugin;
impl ClientPlugin for WorkerPlugin {
    type Api = Pool;
    fn manifest(&self) -> PluginManifest {
        PluginManifest::new("fixture.workers", "1")
            .with_capability(PluginCapability::Tasks)
            .with_capability(PluginCapability::PluginEvents)
    }
    fn install(&self, context: PluginContext) -> PluginFuture<'_, anyhow::Result<Arc<Pool>>> {
        Box::pin(async move {
            let tasks = context.tasks().unwrap();
            let (queue, jobs) = async_channel::bounded::<Job>(QUEUE);
            let counts = Arc::new(Counts::default());
            for _ in 0..WORKERS {
                let jobs = jobs.clone();
                let counts = counts.clone();
                let shutdown = tasks.shutdown_signal();
                tasks.spawn_cooperative(async move {
                    counts.started.fetch_add(1, Ordering::SeqCst);
                    loop {
                        let next = futures::future::select(
                            Box::pin(wait_for_shutdown(&shutdown)),
                            Box::pin(jobs.recv()),
                        )
                        .await;
                        let job = match next {
                            futures::future::Either::Left(_) => {
                                // Seal admission, then drain the accepted prefix.
                                jobs.close();
                                match jobs.recv().await {
                                    Ok(job) => job,
                                    Err(_) => break,
                                }
                            }
                            futures::future::Either::Right((Ok(job), _)) => job,
                            futures::future::Either::Right((Err(_), _)) => break,
                        };
                        // Real work must cooperate with cancellation and have
                        // its own I/O deadline. Never block an inline callback.
                        tokio::task::yield_now().await;
                        counts.processed.fetch_add(1, Ordering::SeqCst);
                        drop(job);
                    }
                    counts.finished.fetch_add(1, Ordering::SeqCst);
                })?;
            }
            Ok(Arc::new(Pool {
                queue,
                counts,
                events: context.plugin_events().unwrap().clone(),
            }))
        })
    }
}
async fn fixture() -> (Arc<Client>, Arc<Pool>) {
    let manager = Arc::new(
        whatsapp_rust::store::persistence_manager::PersistenceManager::new(Arc::new(
            wacore::store::InMemoryBackend::new(),
        ))
        .await
        .unwrap(),
    );
    let client = Client::builder()
        .with_persistence_manager(manager)
        .with_runtime(TokioRuntime)
        .with_http_client(offline::OfflineHttp)
        .with_transport_factory(offline::OfflineTransport)
        .with_version_override((2, 3000, 1))
        .with_plugin(WorkerPlugin)
        .build()
        .await
        .unwrap()
        .into_client();
    let pool = client.plugin::<WorkerPlugin>().unwrap();
    tokio::time::timeout(Duration::from_secs(2), async {
        while pool.counts.started.load(Ordering::SeqCst) != WORKERS {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    (client, pool)
}

#[tokio::test]
async fn burst_admission_drops_excess_and_shutdown_drains_fixed_workers() {
    let (client, pool) = fixture().await;
    assert!(!pool.admit(&[0; MAX_PAYLOAD + 1]));
    assert_eq!(pool.counts.copied.load(Ordering::SeqCst), 0);
    let accepted = (0..100).filter(|_| pool.admit(b"eightbyt")).count();
    assert_eq!(accepted, QUEUE);
    assert_eq!(pool.counts.retained.load(Ordering::SeqCst), BYTE_BUDGET);
    assert_eq!(pool.counts.rejected.load(Ordering::SeqCst), 101 - accepted);
    pool.close_admission();
    assert!(!pool.admit(b"late"));
    let report = tokio::time::timeout(Duration::from_secs(2), client.shutdown())
        .await
        .unwrap();
    assert!(report.device.is_ok());
    assert_eq!(pool.counts.started.load(Ordering::SeqCst), WORKERS);
    assert_eq!(pool.counts.finished.load(Ordering::SeqCst), WORKERS);
    assert_eq!(pool.counts.processed.load(Ordering::SeqCst), accepted);
    assert_eq!(pool.counts.retained.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn cancellation_seals_admission_and_stops_idle_workers() {
    let (client, pool) = fixture().await;
    tokio::task::yield_now().await;
    tokio::time::timeout(Duration::from_secs(2), client.shutdown())
        .await
        .unwrap();
    assert!(!pool.admit(b"late"));
    assert_eq!(pool.counts.finished.load(Ordering::SeqCst), WORKERS);
}

#[tokio::test]
async fn foreign_size_gate_precedes_copy_and_endpoints_share_one_payload() {
    let (client, pool) = fixture().await;
    let topic = PluginEventTopic::new("sample").unwrap();
    let selector = pool.events.selector(&topic);
    let router = client.plugin_event_router().unwrap();
    let config = PluginEventEndpointConfig::new(1, PluginEventOverflow::DropNewest);
    // A foreign host must also cap the number of endpoints it permits.
    let first = router.subscribe([selector.clone()], config).unwrap();
    let second = router.subscribe([selector], config).unwrap();
    assert!(pool.publish_foreign(&[0; MAX_PAYLOAD + 1]).is_err());
    assert_eq!(pool.counts.copied.load(Ordering::SeqCst), 0);
    assert!(first.try_recv().is_err());
    assert!(second.try_recv().is_err());
    pool.publish_foreign(b"bounded").unwrap();
    assert_eq!(router.stats().queued_events, 1);
    assert_eq!(router.stats().queued_payload_bytes, 7);
    let a = first.try_recv().unwrap();
    let b = second.try_recv().unwrap();
    assert!(Arc::ptr_eq(&a, &b));
    assert_eq!(a.payload.as_ptr(), b.payload.as_ptr());
    assert_eq!(pool.counts.copied.load(Ordering::SeqCst), 1);
    client.shutdown().await;
}

#[tokio::test]
async fn full_queue_releases_rejected_jobs_byte_reservation() {
    let (client, pool) = fixture().await;
    for _ in 0..QUEUE {
        assert!(pool.admit(b"x"));
    }
    assert!(!pool.admit(b"y"));
    assert_eq!(pool.counts.retained.load(Ordering::SeqCst), QUEUE);
    assert_eq!(pool.counts.rejected.load(Ordering::SeqCst), 1);
    pool.close_admission();
    tokio::time::timeout(Duration::from_secs(2), client.shutdown())
        .await
        .unwrap();
    assert_eq!(pool.counts.retained.load(Ordering::SeqCst), 0);
}

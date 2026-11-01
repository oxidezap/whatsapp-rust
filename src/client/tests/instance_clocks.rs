use super::*;
use crate::{Clock, ClockedRuntime, VirtualClock};
use wacore::time::Instant;

struct CountedClock {
    inner: Arc<VirtualClock>,
    wall_reads: AtomicUsize,
}

impl Clock for CountedClock {
    fn wall_time_millis(&self) -> i64 {
        self.wall_reads.fetch_add(1, Ordering::Relaxed);
        self.inner.wall_time_millis()
    }

    fn monotonic_time(&self) -> Instant {
        self.inner.monotonic_time()
    }
}

async fn client(clock: Arc<dyn Clock>) -> Arc<Client> {
    let backend = Arc::new(wacore::store::InMemoryBackend::new().with_clock(clock.clone()));
    let pm = Arc::new(PersistenceManager::new(backend).await.unwrap());
    let runtime = ClockedRuntime::new(Arc::new(crate::TokioRuntime), clock);
    Client::builder()
        .with_runtime(runtime)
        .with_persistence_manager(pm)
        .with_transport_factory(crate::transport::mock::MockTransportFactory::new())
        .with_http_client(MockHttpClient)
        .build()
        .await
        .unwrap()
        .into_parts()
        .0
}

#[tokio::test]
async fn message_ids_read_only_the_selected_instance_clock() {
    let a = Arc::new(CountedClock {
        inner: Arc::new(VirtualClock::new(1_700_000_000_000, Instant::ZERO).unwrap()),
        wall_reads: AtomicUsize::new(0),
    });
    let b = Arc::new(CountedClock {
        inner: Arc::new(VirtualClock::new(1_800_000_000_000, Instant::ZERO).unwrap()),
        wall_reads: AtomicUsize::new(0),
    });
    let ca = client(a.clone()).await;
    let cb = client(b.clone()).await;
    let aa = a.wall_reads.load(Ordering::Relaxed);
    let bb = b.wall_reads.load(Ordering::Relaxed);
    let global = wacore::time::clock_reads::snapshot();
    let _ = ca.generate_message_id();
    assert_eq!(a.wall_reads.load(Ordering::Relaxed), aa + 1);
    assert_eq!(b.wall_reads.load(Ordering::Relaxed), bb);
    let _ = cb.generate_message_id();
    assert_eq!(b.wall_reads.load(Ordering::Relaxed), bb + 1);
    assert_eq!(wacore::time::clock_reads::since(global).wall, 0);
}

#[tokio::test]
async fn client_cache_expiry_follows_its_instance_clock() {
    let a = Arc::new(VirtualClock::new(1_700_000_000_000, Instant::ZERO).unwrap());
    let b = Arc::new(VirtualClock::new(1_800_000_000_000, Instant::ZERO).unwrap());
    let ca = client(a.clone()).await;
    let cb = client(b.clone()).await;
    let jid: Jid = "15550000101@s.whatsapp.net".parse().unwrap();
    ca.session_recreate_history
        .insert(jid.clone(), Instant::ZERO)
        .await;
    cb.session_recreate_history
        .insert(jid.clone(), Instant::ZERO)
        .await;
    a.advance(Duration::from_secs(7200)).unwrap();
    assert_eq!(ca.session_recreate_history.get(&jid).await, None);
    assert_eq!(
        cb.session_recreate_history.get(&jid).await,
        Some(Instant::ZERO)
    );
}

#[tokio::test]
async fn activity_at_the_clock_origin_remains_visible_in_wall_time() {
    let clock = Arc::new(VirtualClock::new(1_700_000_000_000, Instant::ZERO).unwrap());
    let client = client(clock.clone()).await;
    client
        .stats
        .mark_recv_activity_at(client.runtime.monotonic_time());
    clock.advance(Duration::from_secs(2)).unwrap();
    assert_eq!(client.stats().last_data_received_ms, 1_700_000_000_000);
}

#[cfg(feature = "sqlite-storage")]
#[tokio::test]
async fn storage_retention_uses_each_instance_clock_in_memory_and_sqlite() {
    use wacore::store::traits::Backend;
    for sqlite in [false, true] {
        let early = Arc::new(VirtualClock::new(1000, Instant::ZERO).unwrap());
        let late = Arc::new(VirtualClock::new(9000, Instant::ZERO).unwrap());
        let a: Arc<dyn Backend> = if sqlite {
            Arc::new(
                crate::store::SqliteStore::open(
                    "file:instance-clock-early?mode=memory&cache=shared",
                )
                .await
                .unwrap()
                .with_clock(early),
            )
        } else {
            Arc::new(wacore::store::InMemoryBackend::new().with_clock(early))
        };
        let b: Arc<dyn Backend> = if sqlite {
            Arc::new(
                crate::store::SqliteStore::open(
                    "file:instance-clock-late?mode=memory&cache=shared",
                )
                .await
                .unwrap()
                .with_clock(late),
            )
        } else {
            Arc::new(wacore::store::InMemoryBackend::new().with_clock(late))
        };
        let _a = PersistenceManager::new(a.clone()).await.unwrap();
        let _b = PersistenceManager::new(b.clone()).await.unwrap();
        for backend in [&a, &b] {
            backend
                .store_sent_message("chat", "id", b"fixture")
                .await
                .unwrap();
            backend
                .store_pending_inbound("chat", "sender", "id", b"fixture")
                .await
                .unwrap();
            backend
                .save_base_key("address", "id", b"fixture")
                .await
                .unwrap();
        }
        assert_eq!(a.delete_expired_sent_messages(2).await.unwrap(), 1);
        assert_eq!(a.delete_expired_pending_inbound(2).await.unwrap(), 1);
        assert_eq!(a.delete_expired_base_keys(2).await.unwrap(), 1);
        assert_eq!(b.delete_expired_sent_messages(2).await.unwrap(), 0);
        assert_eq!(b.delete_expired_pending_inbound(2).await.unwrap(), 0);
        assert_eq!(b.delete_expired_base_keys(2).await.unwrap(), 0);
    }
}

#[tokio::test]
async fn equal_clock_readings_do_not_make_different_clock_domains_interchangeable() {
    let runtime_clock = Arc::new(VirtualClock::new(1000, Instant::ZERO).unwrap());
    let storage_clock = Arc::new(VirtualClock::new(1000, Instant::ZERO).unwrap());
    let backend = Arc::new(wacore::store::InMemoryBackend::new().with_clock(storage_clock));
    let pm = Arc::new(PersistenceManager::new(backend).await.unwrap());
    let result = Client::builder()
        .with_runtime(ClockedRuntime::new(
            Arc::new(crate::TokioRuntime),
            runtime_clock,
        ))
        .with_persistence_manager(pm)
        .with_transport_factory(crate::transport::mock::MockTransportFactory::new())
        .with_http_client(MockHttpClient)
        .build()
        .await;
    assert!(matches!(
        result,
        Err(ClientBuilderError::InstanceClockMismatch)
    ));
}

use super::*;
use crate::{Clock, ClockedRuntime, VirtualClock};
use wacore::time::Instant;

struct CountedClock {
    inner: Arc<VirtualClock>,
    wall_reads: AtomicUsize,
}

impl Clock for CountedClock {
    fn sleep(
        self: Arc<Self>,
        duration: Duration,
    ) -> Option<wacore::runtime::BoxFuture<'static, ()>> {
        self.inner.clone().sleep(duration)
    }

    fn wall_time_millis(&self) -> i64 {
        self.wall_reads.fetch_add(1, Ordering::Relaxed);
        self.inner.wall_time_millis()
    }

    fn monotonic_time(&self) -> Instant {
        self.inner.monotonic_time()
    }
}

async fn client(clock: Arc<dyn Clock>) -> Arc<Client> {
    client_with_http(clock, Arc::new(MockHttpClient)).await
}

async fn client_with_http(
    clock: Arc<dyn Clock>,
    http: Arc<dyn crate::http::HttpClient>,
) -> Arc<Client> {
    let backend = Arc::new(wacore::store::InMemoryBackend::new().with_clock(clock.clone()));
    let pm = Arc::new(PersistenceManager::new(backend).await.unwrap());
    let runtime = ClockedRuntime::new(Arc::new(crate::TokioRuntime), clock);
    Client::builder()
        .with_runtime(runtime)
        .with_persistence_manager(pm)
        .with_transport_factory(crate::transport::mock::MockTransportFactory::new())
        .with_http_client_arc(http)
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

struct NoopInstrument;
impl wacore::stats::TaskInstrument for NoopInstrument {
    fn on_poll_start(&self) {}
    fn on_poll_end(&self) {}
}

#[tokio::test]
async fn instrumented_client_retains_its_instance_clock() {
    let clock: Arc<dyn Clock> =
        Arc::new(VirtualClock::new(1_700_000_000_000, Instant::ZERO).unwrap());
    let backend = Arc::new(wacore::store::InMemoryBackend::new().with_clock(clock.clone()));
    let pm = Arc::new(PersistenceManager::new(backend).await.unwrap());
    let client = Client::builder()
        .with_runtime(ClockedRuntime::new(
            Arc::new(crate::TokioRuntime),
            clock.clone(),
        ))
        .with_persistence_manager(pm)
        .with_transport_factory(crate::transport::mock::MockTransportFactory::new())
        .with_http_client(MockHttpClient)
        .with_task_instrument(Arc::new(NoopInstrument))
        .build()
        .await
        .unwrap()
        .into_parts()
        .0;
    assert!(
        client
            .runtime
            .clock()
            .is_some_and(|selected| Arc::ptr_eq(selected, &clock)),
        "instrumentation must retain the selected clock"
    );
}

#[tokio::test]
async fn advancing_one_instance_clock_wakes_only_its_timers() {
    use wacore::runtime::Runtime;
    let a = Arc::new(VirtualClock::new(1000, Instant::ZERO).unwrap());
    let b = Arc::new(VirtualClock::new(9000, Instant::ZERO).unwrap());
    let runtime_a = ClockedRuntime::new(Arc::new(crate::TokioRuntime), a.clone());
    let runtime_b = ClockedRuntime::new(Arc::new(crate::TokioRuntime), b.clone());
    let mut sleep_a = runtime_a.sleep(Duration::from_secs(5));
    let mut sleep_b = runtime_b.sleep(Duration::from_secs(5));
    assert!(futures::poll!(&mut sleep_a).is_pending());
    assert!(futures::poll!(&mut sleep_b).is_pending());
    a.advance(Duration::from_secs(5)).unwrap();
    assert!(
        futures::poll!(&mut sleep_a).is_ready(),
        "the selected virtual clock must wake its deadline"
    );
    assert!(futures::poll!(&mut sleep_b).is_pending());
    b.advance(Duration::from_secs(5)).unwrap();
    assert!(futures::poll!(&mut sleep_b).is_ready());
}

#[tokio::test]
async fn retry_rate_refill_uses_the_instance_clock() {
    let clock = Arc::new(VirtualClock::new(1000, Instant::ZERO).unwrap());
    let client = client(clock.clone()).await;
    let chat = "10001@g.us".parse().unwrap();
    client.resend_rate_limiter.set_rate(1, 60);
    assert!(
        client
            .resend_rate_limiter
            .try_acquire_with_clock(&chat, || client.runtime.monotonic_time())
            .await
    );
    assert!(
        !client
            .resend_rate_limiter
            .try_acquire_with_clock(&chat, || client.runtime.monotonic_time())
            .await
    );
    clock.advance(Duration::from_secs(1)).unwrap();
    assert!(
        client
            .resend_rate_limiter
            .try_acquire_with_clock(&chat, || client.runtime.monotonic_time())
            .await
    );
}

#[tokio::test]
async fn media_route_expiry_uses_the_instance_clock() {
    let clock = Arc::new(VirtualClock::new(1000, Instant::ZERO).unwrap());
    let client = client(clock.clone()).await;
    let conn = crate::mediaconn::MediaConn {
        auth: "fixture".into(),
        ttl: 1,
        auth_ttl: None,
        hosts: vec![crate::mediaconn::MediaConnHost::new(
            "media.example.com".into(),
        )],
        fetched_at: client.runtime.monotonic_time(),
    };
    *client.media_conn.write().await = Some(conn.clone());
    assert!(client.cached_download_route().await.unwrap().is_some());
    assert!(!conn.is_expired_at(client.runtime.monotonic_time()));
    clock.advance(Duration::from_secs(2)).unwrap();
    assert!(conn.is_expired_at(client.runtime.monotonic_time()));
    assert!(client.cached_download_route().await.unwrap().is_none());
}

#[tokio::test]
async fn virtual_clock_drives_runtime_timeout_and_preserves_an_unpolled_deadline() {
    use std::task::Poll;
    let clock = Arc::new(VirtualClock::new(1000, Instant::ZERO).unwrap());
    let runtime: Arc<dyn Runtime> = Arc::new(ClockedRuntime::new(
        Arc::new(crate::TokioRuntime),
        clock.clone(),
    ));
    let mut sleep = runtime.sleep(Duration::from_secs(1));
    clock.advance(Duration::from_secs(1)).unwrap();
    assert!(futures::poll!(&mut sleep).is_ready());
    let mut timeout = Box::pin(wacore::runtime::timeout(
        &*runtime,
        Duration::from_secs(2),
        std::future::pending::<()>(),
    ));
    assert!(futures::poll!(&mut timeout).is_pending());
    clock.advance(Duration::from_secs(2)).unwrap();
    assert!(matches!(futures::poll!(&mut timeout), Poll::Ready(Err(_))));
}

struct UploadHttp;
#[async_trait::async_trait]
impl crate::http::HttpClient for UploadHttp {
    async fn execute(
        &self,
        _request: crate::http::HttpRequest,
    ) -> Result<crate::http::HttpResponse> {
        Ok(crate::http::HttpResponse::new(
            200,
            br#"{"url":"https://media.example.com/fixture","direct_path":"/fixture"}"#.to_vec(),
        ))
    }
}

#[tokio::test]
async fn upload_timestamps_come_from_the_instance_clock() {
    let clock = Arc::new(VirtualClock::new(1_700_000_123_000, Instant::ZERO).unwrap());
    let client = client_with_http(clock, Arc::new(UploadHttp)).await;
    *client.media_conn.write().await = Some(crate::mediaconn::MediaConn {
        auth: "fixture".into(),
        ttl: 3600,
        auth_ttl: None,
        hosts: vec![crate::mediaconn::MediaConnHost::new(
            "media.example.com".into(),
        )],
        fetched_at: client.runtime.monotonic_time(),
    });
    let response = client
        .upload(
            b"clock fixture".to_vec(),
            wacore::download::MediaType::Image,
            Default::default(),
        )
        .await
        .unwrap();
    assert_eq!(response.media_key_timestamp, 1_700_000_123);
}

#[derive(Default)]
struct UndecryptableCount(AtomicUsize);
impl wacore::types::events::EventHandler for UndecryptableCount {
    fn handle_event(&self, event: Arc<Event>) {
        if matches!(&*event, Event::UndecryptableMessage(_)) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[tokio::test]
async fn expired_status_is_dropped_on_the_receive_path_using_its_instance_clock() {
    use wacore::store::commands::DeviceCommand;
    use wacore_binary::builder::NodeBuilder;
    let sent_at = wacore::time::now_secs();
    for expired in [true, false] {
        let wall = sent_at * 1000 + if expired { 48 * 60 * 60 * 1000 } else { 0 };
        let clock = Arc::new(VirtualClock::new(wall, Instant::ZERO).unwrap());
        let backend = Arc::new(wacore::store::InMemoryBackend::new().with_clock(clock.clone()));
        let runtime: Arc<dyn Runtime> =
            Arc::new(ClockedRuntime::new(Arc::new(crate::TokioRuntime), clock));
        let (client, _transport) =
            crate::test_utils::create_iq_test_client_with_runtime(backend, runtime).await;
        client
            .persistence_manager
            .process_command(DeviceCommand::SetId(Some(
                "10000@s.whatsapp.net".parse().unwrap(),
            )))
            .await;
        client
            .persistence_manager
            .process_command(DeviceCommand::SetLid(Some("20000@lid".parse().unwrap())))
            .await;
        let authentication = client.claim_session_authentication().unwrap();
        assert!(client.admit_session_authentication(authentication));
        let events = Arc::new(UndecryptableCount::default());
        let _subscription = client.subscribe_handler(events.clone());
        let mut retry = client.wait_for_sent_node(NodeFilter::tag("receipt").attr("type", "retry"));
        let mut rng = rand::rng();
        let key = wacore::libsignal::protocol::KeyPair::generate(&mut rng);
        let ciphertext = wacore::libsignal::protocol::SenderKeyMessage::new(
            3,
            9,
            0,
            vec![7; 16].into_boxed_slice(),
            &mut rng,
            &key.private_key,
        )
        .unwrap();
        let message = NodeBuilder::new("message")
            .attr("from", "status@broadcast")
            .attr("participant", "10001@lid")
            .attr("id", "CLOCK_STATUS")
            .attr("type", "text")
            .attr("t", sent_at.to_string())
            .children([NodeBuilder::new("enc")
                .attr("v", "2")
                .attr("type", "skmsg")
                .bytes(ciphertext.serialized().to_vec())
                .build()])
            .build();
        client
            .clone()
            .handle_incoming_message(crate::test_utils::node_to_owned_ref(&message))
            .await;
        if expired {
            assert_eq!(events.0.load(Ordering::Relaxed), 0);
            assert!(retry.try_recv().unwrap().is_none());
        } else {
            tokio::time::timeout(Duration::from_secs(5), retry)
                .await
                .expect("fresh status must request a retry")
                .unwrap();
            assert_eq!(events.0.load(Ordering::Relaxed), 1);
        }
        client.signal_shutdown_sync();
    }
}

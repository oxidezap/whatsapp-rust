use super::*;
use crate::test_utils::{create_test_client, node_to_owned_ref};

fn counts(client: &Client) -> (usize, usize) {
    (
        client.node_waiter_count.load(Ordering::Acquire),
        client.sent_node_waiter_count.load(Ordering::Acquire),
    )
}

#[tokio::test]
async fn drop_unregisters_without_traffic_or_retaining_client() {
    let client = create_test_client().await;
    for _ in 0..1000 {
        let incoming = client.wait_for_node(NodeFilter::tag("notification"));
        let outgoing = client.wait_for_sent_node(NodeFilter::tag("receipt"));
        assert_eq!(counts(&client), (1, 1));
        drop((incoming, outgoing));
        assert_eq!(counts(&client), (0, 0));
    }
    assert!(client.node_waiters.lock().unwrap().is_empty());
    assert!(client.sent_node_waiters.lock().unwrap().is_empty());
    let weak = Arc::downgrade(&client);
    let incoming = client.wait_for_node(NodeFilter::tag("notification"));
    let outgoing = client.wait_for_sent_node(NodeFilter::tag("receipt"));
    drop(client);
    crate::test_utils::poll_until("last client reference", || weak.upgrade().is_none()).await;
    assert!(incoming.await.is_err());
    assert!(outgoing.await.is_err());
}

#[tokio::test(start_paused = true)]
async fn timeout_and_send_failure_release_owned_waiters() {
    let client = create_test_client().await;
    assert!(
        tokio::time::timeout(
            Duration::from_secs(1),
            client.wait_for_node(NodeFilter::tag("ack"))
        )
        .await
        .is_err()
    );
    assert_eq!(counts(&client), (0, 0));
    async fn send(client: &Client) -> Result<(), ClientError> {
        let _waiter = client.wait_for_node(NodeFilter::tag("ack"));
        client.send_node(NodeBuilder::new("receipt").build()).await
    }
    assert!(send(&client).await.is_err());
    assert_eq!(counts(&client), (0, 0));
}

#[tokio::test]
async fn resolving_and_dropping_race_without_count_underflow() {
    let client = create_test_client().await;
    let node = NodeBuilder::new("receipt").build();
    let incoming = node_to_owned_ref(&node);
    let outgoing = Arc::new(node);
    for _ in 0..100 {
        let waiter = client.wait_for_node(NodeFilter::tag("receipt"));
        let sent = client.wait_for_sent_node(NodeFilter::tag("receipt"));
        std::thread::scope(|scope| {
            let barrier = Arc::new(std::sync::Barrier::new(2));
            let other = barrier.clone();
            scope.spawn(move || {
                other.wait();
                drop((waiter, sent));
            });
            barrier.wait();
            client.resolve_node_waiters(&incoming);
            client.resolve_sent_node_waiters(&outgoing);
        });
        assert_eq!(counts(&client), (0, 0));
    }
}

#[tokio::test]
async fn active_incoming_waiter_survives_both_reconnect_modes() {
    for immediate in [false, true] {
        let client = create_test_client().await;
        let incoming = client.wait_for_node(NodeFilter::tag("notification"));
        let sent = client.wait_for_sent_node(NodeFilter::tag("receipt"));
        if immediate {
            client.reconnect_immediately().await;
        } else {
            client.reconnect().await;
        }
        assert_eq!(
            client.expected_disconnect.load(Ordering::Relaxed),
            immediate
        );
        assert_eq!(
            client.intentional_reconnect.load(Ordering::Relaxed),
            !immediate
        );
        assert_eq!(
            client.backoff_reset_suppressed.load(Ordering::Relaxed),
            !immediate
        );
        if !immediate {
            assert_eq!(client.auto_reconnect_errors.load(Ordering::Relaxed), 4);
        }
        client.cleanup_connection_state().await;
        assert!(sent.await.is_err());
        assert_eq!(counts(&client), (1, 0));
        let node = node_to_owned_ref(&NodeBuilder::new("notification").build());
        client.resolve_node_waiters(&node);
        assert!(Arc::ptr_eq(&incoming.await.unwrap(), &node));
        assert_eq!(counts(&client), (0, 0));
    }
}

use crate::transport::mock::CapturingMockTransport;
use crate::{MediaRetryResult, MediaReuploadError, MediaReuploadRequest, MessageId, MessageRef};

#[test]
fn shared_reupload_errors_keep_the_original_typed_cause() {
    use std::error::Error;

    let error = MediaReuploadError::from(ClientError::NotConnected);
    let cloned = error.clone();
    let source = error
        .source()
        .unwrap()
        .downcast_ref::<ClientError>()
        .unwrap();
    let shared_source = cloned
        .source()
        .unwrap()
        .downcast_ref::<ClientError>()
        .unwrap();
    assert!(std::ptr::eq(source, shared_source));

    let error = MediaReuploadError::from(anyhow::Error::new(std::io::Error::other("fixture")));
    let cloned = error.clone();
    let source = error
        .source()
        .unwrap()
        .downcast_ref::<std::io::Error>()
        .unwrap();
    let shared_source = cloned
        .source()
        .unwrap()
        .downcast_ref::<std::io::Error>()
        .unwrap();
    assert!(std::ptr::eq(source, shared_source));
}

async fn reupload_fixture() -> (Arc<Client>, Arc<CapturingMockTransport>) {
    let client = create_test_client().await;
    client
        .persistence_manager
        .process_command(DeviceCommand::SetId(Some(Jid::pn_device("15550000001", 3))))
        .await;
    client
        .persistence_manager
        .process_command(DeviceCommand::SetLid(Some(Jid::lid_device(
            "10000000001",
            3,
        ))))
        .await;
    let transport = Arc::new(CapturingMockTransport::new());
    install_test_noise_socket(
        &client,
        transport.clone(),
        Arc::new(crate::runtime_impl::TokioRuntime),
    )
    .await;
    (client, transport)
}

async fn request(
    client: Arc<Client>,
    chat: Jid,
    id: &'static str,
    key: [u8; 32],
) -> Result<MediaRetryResult, MediaReuploadError> {
    let req = MediaReuploadRequest {
        target: MessageRef::new(&chat, MessageId::new(id).unwrap(), None, false).unwrap(),
        media_key: &key,
    };
    client.media_reupload().request(&req).await
}

fn ack(id: &str) -> Node {
    NodeBuilder::new("ack")
        .attr("id", id)
        .attr("class", "receipt")
        .attr("type", "server-error")
        .attr("from", Jid::lid("10000000001"))
        .build()
}

fn notification(id: &str) -> Node {
    NodeBuilder::new("notification")
        .attr("id", id)
        .attr("type", "mediaretry")
        .children([NodeBuilder::new("error").attr("code", "2").build()])
        .build()
}

fn deliver(client: &Client, node: Node) {
    client.resolve_node_waiters(&node_to_owned_ref(&node));
}

#[tokio::test]
async fn identical_reuploads_share_receipt_and_survive_one_cancellation() {
    let (client, transport) = reupload_fixture().await;
    let mut first = Box::pin(request(
        client.clone(),
        Jid::pn("15550000002"),
        "SAME",
        [1; 32],
    ));
    let mut second = Box::pin(request(
        client.clone(),
        Jid::pn("15550000002"),
        "SAME",
        [1; 32],
    ));
    let sent = client.wait_for_sent_node(NodeFilter::tag("receipt").attr("id", "SAME"));
    assert!(futures::poll!(&mut first).is_pending());
    assert!(futures::poll!(&mut second).is_pending());
    let receipt = sent.await.unwrap();
    assert_eq!(
        receipt.attrs().optional_jid("to").unwrap(),
        Jid::lid("10000000001")
    );
    assert_eq!(client.media_reuploads.lock().unwrap().len(), 1);
    assert_eq!(client.memory_report().await.media_reuploads, 1);
    assert_eq!(counts(&client).0, 2);
    drop(first);
    assert_eq!(
        counts(&client).0,
        2,
        "second caller still owns the operation"
    );
    // Even before send has finished, the notification is retained while ACK is pending.
    deliver(&client, notification("SAME"));
    assert!(futures::poll!(&mut second).is_pending());
    deliver(&client, ack("SAME"));
    assert!(matches!(second.await.unwrap(), MediaRetryResult::NotFound));
    assert_eq!(transport.sent().len(), 1);
    assert_eq!(counts(&client), (0, 0));
    assert!(client.media_reuploads.lock().unwrap().is_empty());
    assert_eq!(client.memory_report().await.media_reuploads, 0);
}

#[tokio::test]
async fn reupload_collisions_are_deterministic_and_last_drop_cleans_up() {
    let (client, _) = reupload_fixture().await;
    let mut first = Box::pin(request(
        client.clone(),
        Jid::pn("15550000002"),
        "COLLISION",
        [1; 32],
    ));
    assert!(futures::poll!(&mut first).is_pending());
    for (chat, key) in [
        (Jid::pn("15550000003"), [1; 32]),
        (Jid::pn("15550000002"), [2; 32]),
    ] {
        assert!(
            matches!(request(client.clone(), chat, "COLLISION", key).await, Err(MediaReuploadError::Conflict(id)) if id.as_str() == "COLLISION")
        );
    }
    drop(first);
    assert_eq!(counts(&client), (0, 0));
    assert!(client.media_reuploads.lock().unwrap().is_empty());
    let mut retry = Box::pin(request(
        client.clone(),
        Jid::pn("15550000003"),
        "COLLISION",
        [2; 32],
    ));
    assert!(
        futures::poll!(&mut retry).is_pending(),
        "released ID can be reused"
    );
    drop(retry);
    assert!(client.media_reuploads.lock().unwrap().is_empty());
}

#[tokio::test]
async fn correlated_rejection_wins_over_early_notification_and_wrong_ack_is_ignored() {
    let (client, _) = reupload_fixture().await;
    let mut future = Box::pin(request(
        client.clone(),
        Jid::pn("15550000002"),
        "NACK",
        [1; 32],
    ));
    assert!(futures::poll!(&mut future).is_pending());
    deliver(&client, notification("NACK"));
    for (key, value) in [
        ("id", "OTHER"),
        ("class", "message"),
        ("type", "delivery"),
        ("from", "15550000001@s.whatsapp.net"),
        ("participant", "15550000002@s.whatsapp.net"),
    ] {
        let mut wrong = ack("NACK");
        wrong.attrs.insert(key, value);
        deliver(&client, wrong);
        assert!(futures::poll!(&mut future).is_pending());
    }
    let mut rejection = ack("NACK");
    rejection.attrs.insert("error", "479");
    deliver(&client, rejection);
    assert!(matches!(future.await, Err(MediaReuploadError::Rejected(code)) if code == "479"));
    assert!(client.media_reuploads.lock().unwrap().is_empty());
    assert_eq!(counts(&client), (0, 0));
}

#[tokio::test(start_paused = true)]
async fn ack_and_notification_timeouts_release_operation_and_filters() {
    for with_ack in [false, true] {
        let (client, _) = reupload_fixture().await;
        let mut future = Box::pin(request(
            client.clone(),
            Jid::pn("15550000002"),
            "TIMEOUT",
            [1; 32],
        ));
        assert!(futures::poll!(&mut future).is_pending());
        if with_ack {
            deliver(&client, ack("TIMEOUT"));
        }
        let result = future.await;
        if with_ack {
            assert!(matches!(result, Err(MediaReuploadError::Timeout)));
        } else {
            assert!(matches!(result, Err(MediaReuploadError::AckTimeout)));
        }
        assert!(client.media_reuploads.lock().unwrap().is_empty());
        assert_eq!(counts(&client), (0, 0));
    }
}

#[tokio::test]
async fn missing_lid_and_failed_send_do_not_leave_subscriptions() {
    let client = create_test_client().await;
    assert!(matches!(
        request(client.clone(), Jid::pn("15550000002"), "FAIL", [1; 32]).await,
        Err(MediaReuploadError::NotLoggedIn)
    ));
    client
        .persistence_manager
        .process_command(DeviceCommand::SetLid(Some(Jid::lid("10000000001"))))
        .await;
    assert!(matches!(
        request(client.clone(), Jid::pn("15550000002"), "FAIL", [1; 32]).await,
        Err(MediaReuploadError::Client(_))
    ));
    assert!(client.media_reuploads.lock().unwrap().is_empty());
    assert_eq!(counts(&client), (0, 0));
}

#[tokio::test]
async fn batch_coalesces_duplicates_beyond_window_and_preserves_order() {
    let (client, transport) = reupload_fixture().await;
    let chat = Jid::pn("15550000002");
    let key = [1; 32];
    let other_key = [2; 32];
    let same = MediaReuploadRequest {
        target: MessageRef::new(&chat, MessageId::new("BATCH").unwrap(), None, false).unwrap(),
        media_key: &key,
    };
    let mut inputs = vec![same.clone(); 65];
    inputs[10].media_key = &other_key;
    let feature = client.media_reupload();
    let mut future = Box::pin(feature.request_many(&inputs));
    assert!(futures::poll!(&mut future).is_pending());
    deliver(&client, notification("BATCH"));
    deliver(&client, ack("BATCH"));
    let results = future.await;
    assert_eq!(results.len(), inputs.len());
    for (index, result) in results.into_iter().enumerate() {
        if index == 10 {
            assert!(matches!(result, Err(MediaReuploadError::Conflict(_))));
        } else {
            assert!(matches!(result.unwrap(), MediaRetryResult::NotFound));
        }
    }
    assert_eq!(transport.sent().len(), 1);
    assert!(client.media_reuploads.lock().unwrap().is_empty());
    assert_eq!(counts(&client), (0, 0));
}

#[tokio::test]
async fn reupload_chat_mapping_obeys_migration_and_keeps_group_participant() {
    for migrated in [false, true] {
        let (client, _) = reupload_fixture().await;
        client
            .add_lid_pn_mapping("10000000002", "15550000002", LearningSource::Pairing)
            .await
            .unwrap();
        client
            .persistence_manager
            .process_command(DeviceCommand::SetLidMigrated(migrated))
            .await;
        for (chat, participant, expected_chat) in [
            (
                Jid::pn("15550000002"),
                None,
                if migrated {
                    Jid::lid("10000000002")
                } else {
                    Jid::pn("15550000002")
                },
            ),
            (Jid::pn("15550000003"), None, Jid::pn("15550000003")),
            (Jid::lid("10000000002"), None, Jid::lid("10000000002")),
            (
                "123000@g.us".parse().unwrap(),
                Some(Jid::lid("10000000002")),
                "123000@g.us".parse().unwrap(),
            ),
            (
                "status@broadcast".parse().unwrap(),
                Some(Jid::pn("15550000002")),
                "status@broadcast".parse().unwrap(),
            ),
        ] {
            let req = MediaReuploadRequest {
                target: MessageRef::new(
                    &chat,
                    MessageId::new("ROUTE").unwrap(),
                    participant.as_ref(),
                    false,
                )
                .unwrap(),
                media_key: &[1; 32],
            };
            let feature = client.media_reupload();
            let mut future = Box::pin(feature.request(&req));
            let sent = client.wait_for_sent_node(NodeFilter::tag("receipt"));
            assert!(futures::poll!(&mut future).is_pending());
            let sent = tokio::time::timeout(Duration::from_secs(5), async {
                tokio::select! {
                    sent = sent => sent.unwrap(),
                    result = &mut future => panic!("reupload completed before the receipt: {result:?}"),
                }
            }).await.expect("receipt must be sent");
            let rmr = sent.get_optional_child_by_tag(&["rmr"]).unwrap();
            assert_eq!(rmr.attrs().optional_jid("jid").unwrap(), expected_chat);
            assert_eq!(rmr.attrs().optional_jid("participant"), participant);
            drop(future);
        }
    }
}

#[tokio::test]
async fn node_waiter_wakes_happen_after_registry_unlock() {
    struct Probe {
        client: std::sync::Weak<Client>,
        woke: AtomicBool,
        locked: AtomicBool,
    }
    impl std::task::Wake for Probe {
        fn wake(self: Arc<Self>) {
            let client = self.client.upgrade().unwrap();
            self.locked.store(
                client.node_waiters.try_lock().is_err()
                    || client.sent_node_waiters.try_lock().is_err(),
                Ordering::Relaxed,
            );
            self.woke.store(true, Ordering::Relaxed);
        }
    }
    let client = create_test_client().await;
    let probe = Arc::new(Probe {
        client: Arc::downgrade(&client),
        woke: AtomicBool::new(false),
        locked: AtomicBool::new(false),
    });
    let waker = std::task::Waker::from(probe.clone());
    let mut cx = std::task::Context::from_waker(&waker);
    let mut waiter = client.wait_for_node(NodeFilter::tag("notification"));
    assert!(std::pin::Pin::new(&mut waiter).poll(&mut cx).is_pending());
    deliver(&client, notification("WAKE"));
    assert!(probe.woke.load(Ordering::Relaxed));
    assert!(!probe.locked.load(Ordering::Relaxed));
    probe.woke.store(false, Ordering::Relaxed);
    let mut sent = client.wait_for_sent_node(NodeFilter::tag("receipt"));
    assert!(std::pin::Pin::new(&mut sent).poll(&mut cx).is_pending());
    client.clear_sent_node_waiters();
    assert!(probe.woke.load(Ordering::Relaxed));
    assert!(!probe.locked.load(Ordering::Relaxed));
}

#[tokio::test(start_paused = true)]
async fn reconnect_timeouts_still_close_transport_in_both_modes() {
    struct StalledClose(Arc<AtomicBool>);
    #[async_trait::async_trait]
    impl crate::transport::Transport for StalledClose {
        async fn send(&self, _data: bytes::Bytes) -> Result<()> {
            Ok(())
        }
        async fn disconnect(&self) {
            self.0.store(true, Ordering::Relaxed);
            std::future::pending::<()>().await;
        }
    }
    for immediate in [false, true] {
        let client = create_test_client().await;
        client.swap_message_semaphore(1);
        let inbound = client.acquire_message_processing_permit().await;
        let outbound = client.outbound_flush.try_track().unwrap();
        let close_called = Arc::new(AtomicBool::new(false));
        *client.transport.lock().await = Some(Arc::new(StalledClose(close_called.clone())));
        let started = tokio::time::Instant::now();
        tokio::time::timeout(Duration::from_secs(7), async {
            if immediate {
                client.reconnect_immediately().await;
            } else {
                client.reconnect().await;
            }
        })
        .await
        .expect("each teardown stage must remain bounded");
        assert!(close_called.load(Ordering::Relaxed));
        assert!(client.connection_shutdown_signal().is_fired());
        assert_eq!(started.elapsed(), Duration::from_secs(6));
        assert!(client.outbound_flush.try_track().is_none());
        drop((inbound, outbound));
        assert_eq!(client.outbound_flush.pending(), 0);
    }
}

#[tokio::test]
async fn dropping_all_reupload_subscribers_does_not_cycle_the_client() {
    let (client, _) = reupload_fixture().await;
    let weak = Arc::downgrade(&client);
    let mut first = Box::pin(request(
        client.clone(),
        Jid::pn("15550000002"),
        "DROP",
        [1; 32],
    ));
    let mut second = Box::pin(request(
        client.clone(),
        Jid::pn("15550000002"),
        "DROP",
        [1; 32],
    ));
    assert!(futures::poll!(&mut first).is_pending());
    assert!(futures::poll!(&mut second).is_pending());
    drop(client);
    drop(first);
    assert!(
        weak.upgrade().is_some(),
        "remaining subscriber owns the operation"
    );
    drop(second);
    crate::test_utils::poll_until("all coalesced client references released", || {
        weak.upgrade().is_none()
    })
    .await;
}

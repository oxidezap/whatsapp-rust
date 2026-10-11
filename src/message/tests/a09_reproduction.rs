//! Synthetic characterization of failure paths, not a stronger delivery contract.
use super::*;
use diesel::{Connection, RunQueryDsl, SqliteConnection};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use wacore::types::events::{BatchOrigin, ChannelEventHandler, InboundMessage};

#[derive(Default)]
struct Hook {
    fail: AtomicBool,
    pause_first: AtomicBool,
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
    attempts: AtomicUsize,
    committed: AtomicUsize,
}

#[async_trait::async_trait]
impl crate::types::durability_hook::InboundDurabilityHook for Hook {
    async fn on_messages(&self, _: Arc<Client>, items: &[InboundMessage]) -> anyhow::Result<()> {
        self.attempts.fetch_add(items.len(), Ordering::SeqCst);
        if self.pause_first.swap(false, Ordering::SeqCst) {
            self.entered.notify_one();
            self.release.notified().await;
        }
        anyhow::ensure!(
            !self.fail.load(Ordering::SeqCst),
            "synthetic consumer failure"
        );
        self.committed.fetch_add(items.len(), Ordering::SeqCst);
        Ok(())
    }
}

struct Fixture {
    client: Arc<Client>,
    transport: Arc<crate::transport::mock::CapturingMockTransport>,
    sql: SqliteConnection,
    hook: Arc<Hook>,
    events: async_channel::Receiver<Arc<Event>>,
    stanza: Arc<OwnedNodeRef>,
    info: MessageInfo,
}

impl Fixture {
    async fn new(name: &str, drain: bool) -> Self {
        use crate::socket::NoiseSocket;
        use crate::transport::mock::CapturingMockTransportFactory;
        use wacore::handshake::NoiseCipher;
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let db = format!(
            "file:a09_{}_{}_{}?mode=memory&cache=shared",
            name,
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        );
        let backend = Arc::new(SqliteStore::open(&db).await.unwrap());
        let sql = SqliteConnection::establish(&db).unwrap();
        let pm = Arc::new(PersistenceManager::new(backend).await.unwrap());
        let factory = CapturingMockTransportFactory::new();
        let transport = factory.transport();
        let (client, _) = Client::builder()
            .with_runtime_arc(Arc::new(crate::runtime_impl::TokioRuntime))
            .with_persistence_manager(pm)
            .with_transport_factory_arc(Arc::new(factory))
            .with_http_client_arc(Arc::new(MockHttpClient))
            .build()
            .await
            .unwrap()
            .into_parts();
        *client.noise_socket.lock().unwrap() = Some(Arc::new(NoiseSocket::new(
            client.runtime.clone(),
            transport.clone(),
            NoiseCipher::new(&[0; 32]).unwrap(),
            NoiseCipher::new(&[0; 32]).unwrap(),
        )));
        client.set_connected_for_test(true);
        seed_test_pn(&client).await;
        if !drain {
            client.enter_live_mode_for_tests();
        }
        let hook = Arc::new(Hook::default());
        client
            .inbound_durability_hook
            .set(hook.clone())
            .ok()
            .unwrap();
        let (handler, events) = ChannelEventHandler::new();
        client.core.event_bus.subscribe_handler(handler).detach();
        let (bundle, own) = bobs_prekey_bundle(&client).await;
        let mut peer = AlicePeer::new("12025550123:7@s.whatsapp.net").await;
        peer.install_bob_session(&own.to_protocol_address(), &bundle)
            .await;
        let mut message = wa::Message::default();
        message.conversation = Some("synthetic retained body".into());
        let ciphertext = peer
            .encrypt(
                &own.to_protocol_address(),
                &MessageUtils::encode_and_pad(&message),
            )
            .await;
        let enc = enc_payload_from_ciphertext(&ciphertext);
        let stanza = node_to_arc(
            NodeBuilder::new("message")
                .attr("from", &peer.jid)
                .attr("id", name)
                .attr("type", "text")
                .attr("t", wacore::time::now_secs().to_string())
                .children([NodeBuilder::new("enc")
                    .attr("type", enc.enc_type.as_wire_str())
                    .attr("v", "2")
                    .bytes(enc.ciphertext.to_vec())
                    .build()])
                .build(),
        );
        let info = client.parse_message_info(stanza.get()).await.unwrap();
        Self {
            client,
            transport,
            sql,
            hook,
            events,
            stanza,
            info,
        }
    }

    async fn restart(&mut self) {
        use crate::socket::NoiseSocket;
        use crate::transport::mock::CapturingMockTransportFactory;
        use wacore::handshake::NoiseCipher;
        let backend = self.client.persistence_manager.backend();
        let pm = Arc::new(PersistenceManager::new(backend).await.unwrap());
        let factory = CapturingMockTransportFactory::new();
        let transport = factory.transport();
        let (client, _) = Client::builder()
            .with_runtime_arc(Arc::new(crate::runtime_impl::TokioRuntime))
            .with_persistence_manager(pm)
            .with_transport_factory_arc(Arc::new(factory))
            .with_http_client_arc(Arc::new(MockHttpClient))
            .build()
            .await
            .unwrap()
            .into_parts();
        seed_test_pn(&client).await;
        client.enter_live_mode_for_tests();
        client.set_connected_for_test(true);
        *client.noise_socket.lock().unwrap() = Some(Arc::new(NoiseSocket::new(
            client.runtime.clone(),
            transport.clone(),
            NoiseCipher::new(&[0; 32]).unwrap(),
            NoiseCipher::new(&[0; 32]).unwrap(),
        )));
        client
            .inbound_durability_hook
            .set(self.hook.clone())
            .ok()
            .unwrap();
        let (handler, events) = ChannelEventHandler::new();
        client.core.event_bus.subscribe_handler(handler).detach();
        self.client = client;
        self.transport = transport;
        self.events = events;
        self.hook.attempts.store(0, Ordering::SeqCst);
    }

    fn buffer_failure(&mut self, enabled: bool) {
        let query = if enabled {
            "CREATE TRIGGER fail_pending BEFORE INSERT ON pending_inbound_messages BEGIN SELECT RAISE(FAIL, 'synthetic pending write failure'); END"
        } else {
            "DROP TRIGGER fail_pending"
        };
        diesel::sql_query(query).execute(&mut self.sql).unwrap();
    }

    async fn receive(&self) {
        self.client
            .clone()
            .handle_incoming_message(self.stanza.clone())
            .await;
        crate::test_utils::wait_for_outbound_tasks(&self.client).await;
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while self.client.signal_flush_worker_alive() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("coalesced Signal flush must settle");
    }

    async fn pending(&self) -> Option<Vec<u8>> {
        self.client
            .persistence_manager
            .backend()
            .get_pending_inbound(
                &self.info.source.chat.to_string(),
                &self.info.source.sender.to_string(),
                &self.info.id,
            )
            .await
            .unwrap()
    }

    fn receipts(&self) -> usize {
        let frames = self.transport.sent();
        assert_eq!(
            message_acks_for(&frames, &self.info.id),
            0,
            "ordinary DM/group delivery uses a receipt, not a transport ack"
        );
        delivery_receipts_for(&frames, &self.info.id)
    }
    fn published(&self) -> Vec<String> {
        message_texts_for_id(&self.events, &self.info.id)
    }
    async fn persisted_session(&self) -> bool {
        self.client
            .persistence_manager
            .backend()
            .get_session(self.info.source.sender.to_protocol_address().as_str())
            .await
            .unwrap()
            .is_some()
    }
}

#[tokio::test]
async fn a09_live_buffer_failure_then_exact_ciphertext_redelivery_recovers_body() {
    let mut f = Fixture::new("A09_LIVE_BUFFER", false).await;
    assert!(!f.persisted_session().await);
    f.buffer_failure(true);
    f.receive().await;
    assert_eq!(f.receipts(), 0);
    assert_eq!(f.hook.attempts.load(Ordering::SeqCst), 0);
    assert!(f.pending().await.is_none());
    assert!(f.published().is_empty());
    assert_eq!(f.client.inbound_commit_batch.pending_stats().0, 0);
    assert!(
        f.persisted_session().await,
        "live receive persists the advanced session despite missing buffer"
    );
    f.buffer_failure(false);
    f.receive().await;
    assert_eq!(
        f.receipts(),
        1,
        "the original plaintext is committed before its receipt"
    );
    assert_eq!(f.hook.committed.load(Ordering::SeqCst), 1);
    assert_eq!(f.published(), ["synthetic retained body"]);
}

#[tokio::test]
async fn a09_drain_buffer_failure_retains_body_then_recovers() {
    let mut f = Fixture::new("A09_DRAIN_BUFFER", true).await;
    f.buffer_failure(true);
    f.receive().await;
    assert!(
        !f.client
            .flush_inbound_commits_under_permit(true, None, None)
            .await
    );
    assert_eq!(f.receipts(), 0);
    assert_eq!(f.hook.attempts.load(Ordering::SeqCst), 0);
    assert!(f.pending().await.is_none());
    assert!(f.published().is_empty());
    assert_eq!(f.client.inbound_commit_batch.pending_stats().0, 1);
    assert!(
        !f.persisted_session().await,
        "drain must retain the ratchet in cache until the row exists"
    );
    f.buffer_failure(false);
    assert!(
        f.client
            .flush_inbound_commits_under_permit(true, None, None)
            .await
    );
    crate::test_utils::wait_for_outbound_tasks(&f.client).await;
    assert_eq!(f.receipts(), 1);
    assert_eq!(f.hook.committed.load(Ordering::SeqCst), 1);
    assert_eq!(f.published(), ["synthetic retained body"]);
    assert!(f.pending().await.is_none());
    assert!(f.persisted_session().await);
    assert!(!f.client.inbound_commit_batch.is_active());
    f.receive().await;
    assert_eq!(f.receipts(), 2);
    assert_eq!(f.hook.committed.load(Ordering::SeqCst), 1);
    assert!(f.published().is_empty());
}

async fn hook_failure_recovers(drain: bool, corrupt: bool) {
    let mut f = Fixture::new(
        if corrupt {
            "A09_CORRUPT"
        } else if drain {
            "A09_DRAIN_HOOK"
        } else {
            "A09_LIVE_HOOK"
        },
        drain,
    )
    .await;
    f.hook.fail.store(true, Ordering::SeqCst);
    f.receive().await;
    if drain {
        assert!(
            f.client
                .flush_inbound_commits_under_permit(true, None, None)
                .await
        );
    }
    crate::test_utils::wait_for_outbound_tasks(&f.client).await;
    assert_eq!(f.receipts(), 0);
    assert_eq!(f.hook.attempts.load(Ordering::SeqCst), 1);
    let original = f
        .pending()
        .await
        .expect("failed hook retains original bytes");
    assert!(f.persisted_session().await);
    assert!(f.published().is_empty());
    if corrupt {
        diesel::sql_query("UPDATE pending_inbound_messages SET message = X'FF'")
            .execute(&mut f.sql)
            .unwrap();
    }
    f.hook.fail.store(false, Ordering::SeqCst);
    if drain {
        assert!(!f.client.inbound_commit_batch.reset());
        f.client.swap_message_semaphore(1);
        f.client
            .offline_sync_completed
            .store(false, Ordering::Release);
    }
    f.receive().await;
    if drain {
        assert_eq!(f.receipts(), 0, "replay during drain waits for its batch");
        assert!(
            f.client
                .flush_inbound_commits_under_permit(true, None, None)
                .await
        );
        crate::test_utils::wait_for_outbound_tasks(&f.client).await;
    }
    if corrupt {
        assert_eq!(
            f.receipts(),
            0,
            "corruption cannot acknowledge an uncommitted message"
        );
        assert_eq!(
            f.pending().await.as_deref(),
            Some(&[0xff][..]),
            "retain exact bytes for diagnosis or repair"
        );
        assert_eq!(f.hook.attempts.load(Ordering::SeqCst), 1);
        assert_eq!(f.hook.committed.load(Ordering::SeqCst), 0);
        assert!(f.published().is_empty());
        f.client
            .persistence_manager
            .backend()
            .store_pending_inbound(
                &f.info.source.chat.to_string(),
                &f.info.source.sender.to_string(),
                &f.info.id,
                &original,
            )
            .await
            .unwrap();
        f.receive().await;
    }
    assert_eq!(f.receipts(), 1);
    assert!(f.pending().await.is_none());
    assert_eq!(f.hook.committed.load(Ordering::SeqCst), 1);
    assert_eq!(f.published(), ["synthetic retained body"]);
}

#[tokio::test]
async fn a09_live_hook_failure_replays_buffer() {
    hook_failure_recovers(false, false).await;
}
#[tokio::test]
async fn a09_drain_hook_failure_replays_buffer() {
    hook_failure_recovers(true, false).await;
}
#[tokio::test]
async fn a09_corrupt_replay_row_is_retained_until_repaired() {
    hook_failure_recovers(false, true).await;
}

#[tokio::test]
async fn a09_old_pending_row_survives_startup_and_regular_sweep() {
    let mut f = Fixture::new("A09_RETENTION", false).await;
    f.hook.fail.store(true, Ordering::SeqCst);
    f.receive().await;
    assert_eq!(f.receipts(), 0);
    assert!(f.pending().await.is_some());
    diesel::sql_query("UPDATE pending_inbound_messages SET inserted_at = inserted_at - 691200")
        .execute(&mut f.sql)
        .unwrap();
    f.client.run_startup_retention_cleanup().await;
    assert!(
        f.pending().await.is_some(),
        "startup gives old rows a replay opportunity"
    );
    f.client.run_retention_cleanup(0).await;
    assert!(
        f.pending().await.is_some(),
        "elapsed time cannot establish consumer commit"
    );
    f.hook.fail.store(false, Ordering::SeqCst);
    f.receive().await;
    assert_eq!(f.receipts(), 1);
    assert_eq!(f.hook.committed.load(Ordering::SeqCst), 1);
    assert_eq!(f.published(), ["synthetic retained body"]);
}

#[tokio::test]
async fn a09_pending_read_error_withholds_ack_and_recovers_original_body() {
    let mut f = Fixture::new("A09_READ_ERROR", false).await;
    f.hook.fail.store(true, Ordering::SeqCst);
    f.receive().await;
    assert_eq!(f.receipts(), 0);
    assert!(f.pending().await.is_some());
    diesel::sql_query("ALTER TABLE pending_inbound_messages RENAME TO saved_pending")
        .execute(&mut f.sql)
        .unwrap();
    f.hook.fail.store(false, Ordering::SeqCst);
    f.receive().await;
    assert_eq!(f.receipts(), 0);
    assert_eq!(f.hook.attempts.load(Ordering::SeqCst), 1);
    assert!(f.published().is_empty());
    diesel::sql_query("ALTER TABLE saved_pending RENAME TO pending_inbound_messages")
        .execute(&mut f.sql)
        .unwrap();
    f.receive().await;
    assert_eq!(f.receipts(), 1);
    assert_eq!(f.hook.committed.load(Ordering::SeqCst), 1);
    assert_eq!(f.published(), ["synthetic retained body"]);
}

async fn group_buffer_failure(drain: bool) {
    let mut f = Fixture::new(
        if drain {
            "A09_GROUP_DRAIN"
        } else {
            "A09_GROUP_LIVE"
        },
        drain,
    )
    .await;
    let group: Jid = "120363000000000091@g.us".parse().unwrap();
    let mut peer = joined_group_sender(&f.client, "777000000000091:7@lid", &group).await;
    let mut message = wa::Message::default();
    message.conversation = Some("synthetic retained group body".into());
    let ciphertext = peer
        .encrypt_group_message(&group, &MessageUtils::encode_and_pad(&message))
        .await;
    f.stanza = group_skmsg_stanza(&group, &peer.jid, &f.info.id, ciphertext);
    f.info = f.client.parse_message_info(f.stanza.get()).await.unwrap();
    f.buffer_failure(true);
    f.receive().await;
    if drain {
        assert!(
            !f.client
                .flush_inbound_commits_under_permit(true, None, None)
                .await
        );
    }
    assert_eq!(f.receipts(), 0);
    assert_eq!(f.hook.attempts.load(Ordering::SeqCst), 0);
    assert!(f.pending().await.is_none());
    assert!(f.published().is_empty());
    f.buffer_failure(false);
    if drain {
        assert!(
            f.client
                .flush_inbound_commits_under_permit(true, None, None)
                .await
        );
        crate::test_utils::wait_for_outbound_tasks(&f.client).await;
        assert_eq!(f.hook.committed.load(Ordering::SeqCst), 1);
        assert_eq!(f.published(), ["synthetic retained group body"]);
    } else {
        f.receive().await;
        assert_eq!(f.hook.committed.load(Ordering::SeqCst), 1);
        assert_eq!(f.published(), ["synthetic retained group body"]);
    }
    assert_eq!(f.receipts(), 1);
}

#[tokio::test]
async fn a09_group_live_buffer_failure_recovers_body_on_redelivery() {
    group_buffer_failure(false).await;
}
#[tokio::test]
async fn a09_group_drain_buffer_failure_retains_body_for_recovery() {
    group_buffer_failure(true).await;
}

#[tokio::test]
async fn retained_live_plaintext_survives_real_connection_cleanup() {
    let mut f = Fixture::new("RESET_BUFFER", false).await;
    f.buffer_failure(true);
    f.receive().await;
    assert_eq!(f.client.inbound_commit_batch.retention.stats().0, 1);
    f.client.cleanup_connection_state().await;
    assert_eq!(f.client.inbound_commit_batch.retention.stats().0, 1);
    assert_eq!(f.hook.committed.load(Ordering::SeqCst), 0);
    f.buffer_failure(false);
    f.receive().await;
    assert!(
        f.client
            .flush_inbound_commits_under_permit(true, None, None)
            .await
    );
    assert_eq!(f.hook.committed.load(Ordering::SeqCst), 1);
    assert_eq!(f.published(), ["synthetic retained body"]);
    assert_eq!(f.client.inbound_commit_batch.retention.stats().0, 0);
}

#[tokio::test]
async fn cancelling_receive_waiter_keeps_the_owned_hook_commit() {
    let f = Fixture::new("CANCEL_COMMIT", false).await;
    f.hook.pause_first.store(true, Ordering::SeqCst);
    let task = tokio::spawn(f.client.clone().handle_incoming_message(f.stanza.clone()));
    f.hook.entered.notified().await;
    task.abort();
    let _ = task.await;
    assert!(f.pending().await.is_some());
    assert_eq!(f.receipts(), 0);
    assert_eq!(f.client.inbound_commit_batch.retention.stats().0, 1);
    f.hook.release.notify_one();
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while f.client.inbound_commit_batch.retention.stats().0 != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    crate::test_utils::wait_for_outbound_tasks(&f.client).await;
    assert_eq!(f.hook.committed.load(Ordering::SeqCst), 1);
    assert_eq!(f.receipts(), 1);
    assert_eq!(f.published(), ["synthetic retained body"]);
}

#[tokio::test]
async fn shutdown_is_bounded_while_a_hook_owns_retained_plaintext() {
    let f = Fixture::new("SHUTDOWN_COMMIT", false).await;
    f.hook.pause_first.store(true, Ordering::SeqCst);
    let task = tokio::spawn(f.client.clone().handle_incoming_message(f.stanza.clone()));
    f.hook.entered.notified().await;
    tokio::time::timeout(std::time::Duration::from_secs(3), f.client.shutdown())
        .await
        .unwrap();
    assert!(f.pending().await.is_some());
    assert_eq!(f.client.inbound_commit_batch.retention.stats().0, 1);
    assert_eq!(f.hook.committed.load(Ordering::SeqCst), 0);
    f.hook.release.notify_one();
    task.await.unwrap();
    assert_eq!(f.hook.committed.load(Ordering::SeqCst), 1);
    assert_eq!(f.published(), ["synthetic retained body"]);
}

#[tokio::test]
async fn exhausted_admission_returns_before_decrypt_or_hook() {
    let f = Fixture::new("CAPACITY", false).await;
    let leases: Vec<_> = (0..400)
        .map(|_| f.client.inbound_commit_batch.retention.admit(1).unwrap())
        .collect();
    // Control responses do not acquire inbound message reservations.
    let (tx, rx) = futures::channel::oneshot::channel();
    f.client.response_waiters_guard().insert(
        "CONTROL_WHILE_FULL".to_owned(),
        crate::client::ResponseWaiter::Iq(tx),
    );
    let ack = node_to_arc(
        NodeBuilder::new("ack")
            .attr("id", "CONTROL_WHILE_FULL")
            .attr("from", "s.whatsapp.net")
            .build(),
    );
    assert!(f.client.handle_ack_response_arc(&ack));
    tokio::time::timeout(std::time::Duration::from_millis(100), rx)
        .await
        .unwrap()
        .unwrap();
    let mut cancelled = false;
    tokio::time::timeout(
        std::time::Duration::from_millis(100),
        crate::handlers::message::MessageHandler::handle_inline(
            f.client.clone(),
            f.stanza.clone(),
            &mut cancelled,
        ),
    )
    .await
    .unwrap();
    assert!(cancelled);
    assert!(f.client.connection_shutdown_signal().is_fired());
    assert!(!f.persisted_session().await);
    assert!(f.pending().await.is_none());
    assert_eq!(f.hook.attempts.load(Ordering::SeqCst), 0);
    assert_eq!(f.receipts(), 0);
    drop(leases);
    assert_eq!(f.client.inbound_commit_batch.retention.stats(), (0, 0));
}

async fn encrypted_parts(
    client: &Arc<Client>,
    sender: &str,
    id: &str,
    bodies: &[&str],
) -> Arc<OwnedNodeRef> {
    let (bundle, own) = bobs_prekey_bundle(client).await;
    let mut peer = AlicePeer::new(sender).await;
    peer.install_bob_session(&own.to_protocol_address(), &bundle)
        .await;
    let mut children = Vec::new();
    for body in bodies {
        let mut message = wa::Message::default();
        message.conversation = Some((*body).to_owned());
        let encrypted = peer
            .encrypt(
                &own.to_protocol_address(),
                &MessageUtils::encode_and_pad(&message),
            )
            .await;
        let enc = enc_payload_from_ciphertext(&encrypted);
        children.push(
            NodeBuilder::new("enc")
                .attr("type", enc.enc_type.as_wire_str())
                .attr("v", "2")
                .bytes(enc.ciphertext.to_vec())
                .build(),
        );
    }
    node_to_arc(
        NodeBuilder::new("message")
            .attr("from", &peer.jid)
            .attr("id", id)
            .attr("type", "text")
            .attr("t", wacore::time::now_secs().to_string())
            .children(children)
            .build(),
    )
}

#[tokio::test]
async fn multipart_stanza_commits_all_parts_before_one_receipt() {
    let mut f = Fixture::new("MULTIPART_RECEIVE", false).await;
    f.stanza = encrypted_parts(
        &f.client,
        "12025550125:7@s.whatsapp.net",
        &f.info.id,
        &["first part", "second part"],
    )
    .await;
    f.info = f.client.parse_message_info(f.stanza.get()).await.unwrap();
    f.hook.fail.store(true, Ordering::SeqCst);
    f.receive().await;
    assert_eq!(f.hook.attempts.load(Ordering::SeqCst), 2);
    assert_eq!(f.receipts(), 0);
    assert!(f.pending().await.is_some());
    f.hook.fail.store(false, Ordering::SeqCst);
    f.receive().await;
    assert_eq!(f.hook.committed.load(Ordering::SeqCst), 2);
    assert_eq!(f.published(), ["first part", "second part"]);
    assert_eq!(f.receipts(), 1);
    assert!(f.pending().await.is_none());
}

#[tokio::test]
async fn stalled_hook_does_not_block_an_unrelated_live_chat() {
    let f = Fixture::new("SLOW_CHAT", false).await;
    f.hook.pause_first.store(true, Ordering::SeqCst);
    let task = tokio::spawn(f.client.clone().handle_incoming_message(f.stanza.clone()));
    f.hook.entered.notified().await;
    let other = encrypted_parts(
        &f.client,
        "12025550126:7@s.whatsapp.net",
        "OTHER_CHAT",
        &["independent"],
    )
    .await;
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        f.client.clone().handle_incoming_message(other),
    )
    .await
    .unwrap();
    assert_eq!(f.hook.committed.load(Ordering::SeqCst), 1);
    assert_eq!(f.receipts(), 0, "the stalled stanza is still unconfirmed");
    assert_eq!(
        message_texts_for_id(&f.events, "OTHER_CHAT"),
        ["independent"]
    );
    f.hook.release.notify_one();
    task.await.unwrap();
    assert_eq!(f.hook.committed.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn resident_failures_recover_without_another_server_delivery() {
    for drain in [false, true] {
        for hook_failure in [false, true] {
            let mut f = Fixture::new("LOCAL_RECOVERY", drain).await;
            if hook_failure {
                f.hook.fail.store(true, Ordering::SeqCst);
            } else {
                f.buffer_failure(true);
            }
            f.receive().await;
            if drain {
                let durable = f
                    .client
                    .flush_inbound_commits_under_permit(false, None, None)
                    .await;
                assert_eq!(durable, hook_failure);
            }
            assert_eq!(f.receipts(), 0);
            assert_eq!(f.pending().await.is_some(), hook_failure);
            if hook_failure {
                f.hook.fail.store(false, Ordering::SeqCst);
            } else {
                f.buffer_failure(false);
            }
            let published = tokio::time::timeout(std::time::Duration::from_secs(12), async {
                loop {
                    let published = f.published();
                    if !published.is_empty() {
                        break published;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(1)).await;
                }
            })
            .await
            .unwrap();
            crate::test_utils::wait_for_outbound_tasks(&f.client).await;
            assert_eq!(published, ["synthetic retained body"]);
            assert_eq!(f.receipts(), 1);
            assert!(f.pending().await.is_none());
        }
    }
}

#[tokio::test]
async fn reconnect_waits_for_an_older_drain_commit_of_the_same_identity() {
    let f = Fixture::new("DRAIN_RESET_COMMIT", true).await;
    f.hook.pause_first.store(true, Ordering::SeqCst);
    f.receive().await;
    let client = f.client.clone();
    let old_commit = tokio::spawn(async move {
        client
            .flush_inbound_commits_under_permit(false, None, None)
            .await
    });
    f.hook.entered.notified().await;
    f.client.cleanup_connection_state().await;
    f.client.enter_live_mode_for_tests();
    let mut new_delivery = tokio::spawn(f.client.clone().handle_incoming_message(f.stanza.clone()));
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), &mut new_delivery,)
            .await
            .is_err(),
        "a new connection must not replace an in-progress commit"
    );
    assert_eq!(f.hook.attempts.load(Ordering::SeqCst), 1);
    assert!(f.published().is_empty());
    f.hook.release.notify_one();
    assert!(old_commit.await.unwrap());
    tokio::time::timeout(std::time::Duration::from_secs(2), new_delivery)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(f.hook.committed.load(Ordering::SeqCst), 1);
    assert_eq!(f.published(), ["synthetic retained body"]);
    assert_eq!(f.client.inbound_commit_batch.retention.stats().0, 0);
}

async fn p1_group_spelling_receipt(first_device: bool) {
    let mut f = Fixture::new("P1_GROUP_RECEIPT", false).await;
    let group: Jid = "120363000000000019@g.us".parse().unwrap();
    let mut peer = joined_group_sender(&f.client, "100000000000019:75@lid", &group).await;
    let bare = peer.jid.to_non_ad();
    let (first, second) = if first_device {
        (peer.jid.clone(), bare)
    } else {
        (bare, peer.jid.clone())
    };
    let ciphertext = encrypt_group_text(&mut peer, &group, "retained group part").await;
    f.stanza = group_skmsg_stanza(&group, &first, &f.info.id, ciphertext.clone());
    f.info = f.client.parse_message_info(f.stanza.get()).await.unwrap();
    assert_eq!(
        f.info.source.sender, first,
        "the fixture must preserve the participant spelling"
    );
    f.buffer_failure(true);
    f.receive().await;
    assert_eq!(f.receipts(), 0);
    f.stanza = group_skmsg_stanza(&group, &second, &f.info.id, ciphertext);
    assert_eq!(
        f.client
            .parse_message_info(f.stanza.get())
            .await
            .unwrap()
            .source
            .sender,
        second
    );
    f.receive().await;
    assert_eq!(f.hook.committed.load(Ordering::SeqCst), 0);
    assert_eq!(
        f.receipts(),
        0,
        "an alternate participant spelling cannot bypass the pending hook"
    );
    f.buffer_failure(false);
    let published = tokio::time::timeout(std::time::Duration::from_secs(9), async {
        loop {
            let bodies = f.published();
            if !bodies.is_empty() {
                break bodies;
            }
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(published, ["retained group part"]);
    assert_eq!(f.client.inbound_commit_batch.retention.stats(), (0, 0));
}
#[tokio::test]
async fn p1_group_device_to_bare_does_not_receipt_before_hook() {
    p1_group_spelling_receipt(true).await;
}
#[tokio::test]
async fn p1_group_bare_to_device_does_not_receipt_before_hook() {
    p1_group_spelling_receipt(false).await;
}

async fn p1_group_spelling_drain(first_device: bool) {
    let mut f = Fixture::new("P1_GROUP_DRAIN", true).await;
    let group: Jid = "120363000000000020@g.us".parse().unwrap();
    let mut peer = joined_group_sender(&f.client, "100000000000020:75@lid", &group).await;
    let bare = peer.jid.to_non_ad();
    let (first, second) = if first_device {
        (peer.jid.clone(), bare)
    } else {
        (bare, peer.jid.clone())
    };
    f.stanza = group_skmsg_stanza(
        &group,
        &first,
        &f.info.id,
        encrypt_group_text(&mut peer, &group, "one group body").await,
    );
    f.info = f.client.parse_message_info(f.stanza.get()).await.unwrap();
    assert_eq!(f.info.source.sender, first);
    f.receive().await;
    f.stanza = group_skmsg_stanza(
        &group,
        &second,
        &f.info.id,
        encrypt_group_text(&mut peer, &group, "one group body").await,
    );
    assert_eq!(
        f.client
            .parse_message_info(f.stanza.get())
            .await
            .unwrap()
            .source
            .sender,
        second
    );
    f.receive().await;
    assert!(
        f.client
            .flush_inbound_commits_under_permit(true, None, None)
            .await
    );
    assert_eq!(f.hook.committed.load(Ordering::SeqCst), 1);
    assert_eq!(f.published(), ["one group body"]);
    for sender in [first, second] {
        assert!(
            f.client
                .persistence_manager
                .backend()
                .get_pending_inbound(&group.to_string(), &sender.to_string(), &f.info.id)
                .await
                .unwrap()
                .is_none()
        );
    }
    assert_eq!(
        f.client.inbound_commit_batch.retention.stats(),
        (0, 0),
        "all physical arrivals must release their reservations"
    );
}
#[tokio::test]
async fn p1_group_device_to_bare_drain_releases_all_admissions() {
    p1_group_spelling_drain(true).await;
}
#[tokio::test]
async fn p1_group_bare_to_device_drain_releases_all_admissions() {
    p1_group_spelling_drain(false).await;
}

async fn p1_group_parts(
    peer: &mut AlicePeer,
    group: &Jid,
    id: &str,
    bodies: &[&str],
) -> Arc<OwnedNodeRef> {
    let mut children = Vec::new();
    for body in bodies {
        let mut wire = Vec::new();
        let mut message = wa::Message::default();
        message.conversation = Some((*body).to_owned());
        waproto::codec::message_encode_into(&message, &mut wire);
        wire.extend_from_slice(&[0xc0, 0x3e, 7]); // preserved future protobuf field
        let message = waproto::codec::message_decode(&wire).unwrap();
        let ciphertext = peer
            .encrypt_group_message(group, &MessageUtils::encode_and_pad(&message))
            .await;
        children.push(
            NodeBuilder::new("enc")
                .attr("type", "skmsg")
                .attr("v", "2")
                .bytes(ciphertext)
                .build(),
        );
    }
    node_to_arc(
        NodeBuilder::new("message")
            .attr("from", group.clone())
            .attr("participant", peer.jid.clone())
            .attr("id", id)
            .attr("t", wacore::time::now_secs().to_string())
            .attr("type", "text")
            .attr("addressing_mode", "lid")
            .children(children)
            .build(),
    )
}
async fn p1_restart_parts(bodies: &[&str]) {
    let mut f = Fixture::new("P1_RESTART", false).await;
    let group: Jid = "120363000000000021@g.us".parse().unwrap();
    let mut peer = joined_group_sender(&f.client, "100000000000021:75@lid", &group).await;
    f.stanza = p1_group_parts(&mut peer, &group, &f.info.id, bodies).await;
    f.info = f.client.parse_message_info(f.stanza.get()).await.unwrap();
    f.hook.fail.store(true, Ordering::SeqCst);
    f.receive().await;
    assert_eq!(f.hook.attempts.load(Ordering::SeqCst), bodies.len());
    let original = f.pending().await.unwrap();
    f.restart().await;
    f.stanza = p1_group_parts(&mut peer, &group, &f.info.id, bodies).await;
    f.receive().await;
    assert_eq!(
        f.hook.attempts.load(Ordering::SeqCst),
        bodies.len(),
        "a re-encrypted replay must retain original multiplicity"
    );
    assert_eq!(
        f.pending().await.unwrap(),
        original,
        "failed replay must leave opaque stored bytes unchanged"
    );
    f.hook.fail.store(false, Ordering::SeqCst);
    f.receive().await;
    assert_eq!(f.hook.committed.load(Ordering::SeqCst), bodies.len());
    assert_eq!(f.published(), bodies);
    assert!(f.pending().await.is_none());
}
#[tokio::test]
async fn p1_restart_reencrypted_single_part_does_not_grow() {
    p1_restart_parts(&["A"]).await;
}
#[tokio::test]
async fn p1_restart_reencrypted_repeated_parts_do_not_grow() {
    p1_restart_parts(&["A", "A"]).await;
}
#[tokio::test]
async fn p1_restart_reencrypted_mixed_parts_do_not_grow() {
    p1_restart_parts(&["A", "B", "B"]).await;
}

async fn p1_two_reconnects(cancel_old: bool) {
    let mut f = Fixture::new("P1_TWO_RECONNECTS", true).await;
    f.stanza = encrypted_parts(
        &f.client,
        "12025550129:7@s.whatsapp.net",
        &f.info.id,
        &["first", "second"],
    )
    .await;
    f.info = f.client.parse_message_info(f.stanza.get()).await.unwrap();
    f.hook.pause_first.store(true, Ordering::SeqCst);
    f.hook.fail.store(!cancel_old, Ordering::SeqCst);
    f.receive().await;
    let client = f.client.clone();
    let old = tokio::spawn(async move {
        client
            .flush_inbound_commits_under_permit(false, None, None)
            .await
    });
    f.hook.entered.notified().await;
    f.client.cleanup_connection_state().await;
    f.client.enter_live_mode_for_tests();
    let mut newer = tokio::spawn(f.client.clone().handle_incoming_message(f.stanza.clone()));
    assert!(
        tokio::time::timeout(std::time::Duration::from_millis(50), &mut newer)
            .await
            .is_err()
    );
    f.client.cleanup_connection_state().await;
    f.client.enter_live_mode_for_tests();
    if cancel_old {
        old.abort();
        assert!(old.await.unwrap_err().is_cancelled());
    } else {
        f.hook.release.notify_one();
        assert!(old.await.unwrap());
    }
    tokio::time::timeout(std::time::Duration::from_secs(2), newer)
        .await
        .unwrap()
        .unwrap();
    f.hook.fail.store(false, Ordering::SeqCst);
    let published = tokio::time::timeout(std::time::Duration::from_secs(9), async {
        loop {
            let bodies = f.published();
            if !bodies.is_empty() {
                break bodies;
            }
            tokio::time::sleep(std::time::Duration::from_millis(1)).await;
        }
    })
    .await
    .expect("retained parts must retry after the stale producer exits");
    assert_eq!(published, ["first", "second"]);
    assert!(f.pending().await.is_none());
    assert_eq!(f.client.inbound_commit_batch.retention.stats(), (0, 0));
}
#[tokio::test]
async fn p1_two_reconnects_after_hook_failure_keep_retrying() {
    p1_two_reconnects(false).await;
}
#[tokio::test]
async fn p1_two_reconnects_after_hook_cancellation_keep_retrying() {
    p1_two_reconnects(true).await;
}

fn with_participant(stanza: &OwnedNodeRef, participant: &Jid) -> Arc<OwnedNodeRef> {
    let mut node = stanza.to_owned_node();
    node.attrs.insert("participant", participant.clone());
    node_to_arc(node)
}

async fn restart_participant_alias(first_device: bool, multiple_rows: bool) {
    let mut f = Fixture::new("RESTART_ALIAS", false).await;
    let group: Jid = "120363000000000024@g.us".parse().unwrap();
    let mut peer = joined_group_sender(&f.client, "100000000000024:75@lid", &group).await;
    let bare = peer.jid.to_non_ad();
    let (first, second) = if first_device {
        (peer.jid.clone(), bare)
    } else {
        (bare, peer.jid.clone())
    };
    let original_stanza = p1_group_parts(&mut peer, &group, &f.info.id, &["A", "A", "B"]).await;
    f.stanza = with_participant(&original_stanza, &first);
    f.info = f.client.parse_message_info(f.stanza.get()).await.unwrap();
    f.hook.fail.store(true, Ordering::SeqCst);
    f.receive().await;
    let original = f.pending().await.unwrap();
    let backend = f.client.persistence_manager.backend();
    let chat = group.to_string();
    let id = f.info.id.to_string();
    let extra_sender = "100000000000024:76@lid";
    let foreign_sender = "100000000000024@s.whatsapp.net";
    let foreign_bytes = b"\0unsupported-foreign-envelope";
    backend
        .store_pending_inbound(&chat, foreign_sender, &id, foreign_bytes)
        .await
        .unwrap();
    if multiple_rows {
        backend
            .store_pending_inbound(&chat, &second.to_string(), &id, &original)
            .await
            .unwrap();
        let mut extra = wa::Message::default();
        extra.conversation = Some("C".into());
        let mut bytes = Vec::new();
        waproto::codec::message_encode_into(&extra, &mut bytes);
        backend
            .store_pending_inbound(&chat, extra_sender, &id, &bytes)
            .await
            .unwrap();
    }
    f.restart().await;
    f.stanza = with_participant(&original_stanza, &second); // identical ciphertext
    f.receive().await;
    let expected = if multiple_rows {
        vec!["A", "A", "B", "C"]
    } else {
        vec!["A", "A", "B"]
    };
    assert_eq!(
        f.receipts(),
        0,
        "a restart cannot acknowledge an uncommitted sender alias"
    );
    assert_eq!(f.hook.attempts.load(Ordering::SeqCst), expected.len());
    assert_eq!(
        backend
            .get_pending_inbound(&chat, &first.to_string(), &id)
            .await
            .unwrap()
            .unwrap(),
        original
    );
    f.hook.fail.store(false, Ordering::SeqCst);
    f.receive().await;
    assert_eq!(f.hook.committed.load(Ordering::SeqCst), expected.len());
    assert_eq!(f.published(), expected);
    assert!(f.receipts() > 0);
    for sender in [
        first.to_string(),
        second.to_string(),
        extra_sender.to_owned(),
    ] {
        assert!(
            backend
                .get_pending_inbound(&chat, &sender, &id)
                .await
                .unwrap()
                .is_none(),
            "committed alias must be settled: {sender}"
        );
    }
    assert_eq!(
        backend
            .get_pending_inbound(&chat, foreign_sender, &id)
            .await
            .unwrap()
            .unwrap(),
        foreign_bytes
    );
    assert_eq!(f.client.inbound_commit_batch.retention.stats(), (0, 0));
}
#[tokio::test]
async fn restart_alias_device_to_bare_replays_before_receipt() {
    restart_participant_alias(true, false).await;
}
#[tokio::test]
async fn restart_alias_bare_to_device_replays_before_receipt() {
    restart_participant_alias(false, false).await;
}
#[tokio::test]
async fn restart_alias_multiple_rows_preserve_parts_and_settle_all_keys() {
    restart_participant_alias(true, true).await;
}

#[tokio::test]
async fn restart_alias_corruption_withholds_receipt_until_matching_rows_are_repaired() {
    let mut f = Fixture::new("RESTART_CORRUPT_ALIAS", false).await;
    let group: Jid = "120363000000000025@g.us".parse().unwrap();
    let mut peer = joined_group_sender(&f.client, "100000000000025:75@lid", &group).await;
    f.stanza = p1_group_parts(&mut peer, &group, &f.info.id, &["A", "A"]).await;
    f.info = f.client.parse_message_info(f.stanza.get()).await.unwrap();
    f.hook.fail.store(true, Ordering::SeqCst);
    f.receive().await;
    let original = f.pending().await.unwrap();
    let backend = f.client.persistence_manager.backend();
    let chat = group.to_string();
    let id = f.info.id.to_string();
    let alias = peer.jid.to_non_ad().to_string();
    let corrupt = b"\0WAPI\x02future-version";
    backend
        .store_pending_inbound(&chat, &alias, &id, corrupt)
        .await
        .unwrap();
    f.restart().await;
    f.stanza = with_participant(&f.stanza, &peer.jid.to_non_ad());
    f.receive().await;
    assert_eq!(f.receipts(), 0);
    assert_eq!(f.hook.attempts.load(Ordering::SeqCst), 0);
    assert_eq!(f.pending().await.unwrap(), original);
    assert_eq!(
        backend
            .get_pending_inbound(&chat, &alias, &id)
            .await
            .unwrap()
            .unwrap(),
        corrupt
    );
    // Explicit fixture repair, never an SDK rewrite/delete of unknown bytes.
    backend
        .store_pending_inbound(&chat, &alias, &id, &original)
        .await
        .unwrap();
    f.hook.fail.store(false, Ordering::SeqCst);
    f.receive().await;
    assert_eq!(f.hook.committed.load(Ordering::SeqCst), 2);
    assert_eq!(f.published(), ["A", "A"]);
    assert_eq!(f.receipts(), 1);
    assert!(
        backend
            .get_pending_inbound_for_message(&chat, &id)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(f.client.inbound_commit_batch.retention.stats(), (0, 0));
}

#[tokio::test]
async fn restart_alias_conflicting_part_order_preserves_both_records() {
    let mut f = Fixture::new("RESTART_ORDER_ALIAS", false).await;
    let group: Jid = "120363000000000026@g.us".parse().unwrap();
    let mut peer = joined_group_sender(&f.client, "100000000000026:75@lid", &group).await;
    f.stanza = p1_group_parts(&mut peer, &group, &f.info.id, &["A", "B"]).await;
    f.info = f.client.parse_message_info(f.stanza.get()).await.unwrap();
    f.hook.fail.store(true, Ordering::SeqCst);
    f.receive().await;
    let original = f.pending().await.unwrap();
    let backend = f.client.persistence_manager.backend();
    let chat = group.to_string();
    let id = f.info.id.to_string();
    let alias = peer.jid.to_non_ad().to_string();
    let reversed: Vec<Vec<u8>> = durability::decode_pending_parts(&original)
        .unwrap()
        .into_iter()
        .rev()
        .map(|message| {
            let mut bytes = Vec::new();
            waproto::codec::message_encode_into(&message, &mut bytes);
            bytes
        })
        .collect();
    let reversed =
        durability::encode_pending_parts(&reversed.iter().map(Vec::as_slice).collect::<Vec<_>>());
    backend
        .store_pending_inbound(&chat, &alias, &id, &reversed)
        .await
        .unwrap();
    f.restart().await;
    f.stanza = with_participant(&f.stanza, &peer.jid.to_non_ad());
    f.receive().await;
    assert_eq!(f.receipts(), 0);
    assert_eq!(f.hook.attempts.load(Ordering::SeqCst), 0);
    assert_eq!(f.pending().await.unwrap(), original);
    assert_eq!(
        backend
            .get_pending_inbound(&chat, &alias, &id)
            .await
            .unwrap()
            .unwrap(),
        reversed
    );
    assert_eq!(f.client.inbound_commit_batch.retention.stats(), (0, 0));
}

#[tokio::test]
async fn pending_lookup_in_a_dm_does_not_merge_sender_devices() {
    let f = Fixture::new("DIRECT_DEVICE_BOUNDARY", false).await;
    let backend = f.client.persistence_manager.backend();
    let chat = f.info.source.chat.to_string();
    let sender = f.info.source.sender.to_string();
    let other = f.info.source.sender.to_non_ad().to_string();
    assert_ne!(sender, other);
    let mut message = wa::Message::default();
    message.conversation = Some("exact device only".into());
    let mut bytes = Vec::new();
    waproto::codec::message_encode_into(&message, &mut bytes);
    backend
        .store_pending_inbound(&chat, &sender, &f.info.id, &bytes)
        .await
        .unwrap();
    backend
        .store_pending_inbound(&chat, &other, &f.info.id, b"\0unreadable other device")
        .await
        .unwrap();
    let replay = f
        .client
        .load_pending_inbound(&Arc::new(f.info.clone()))
        .await
        .unwrap();
    assert_eq!(replay.items.len(), 1);
    assert_eq!(
        replay.items[0].message.conversation.as_deref(),
        Some("exact device only")
    );
    assert_eq!(replay.keys, [(chat.clone(), sender, f.info.id.to_string())]);
    assert_eq!(
        backend
            .get_pending_inbound(&chat, &other, &f.info.id)
            .await
            .unwrap()
            .unwrap(),
        b"\0unreadable other device"
    );
}

async fn restart_contained_alias(bodies: &[&str], exact: &[&str], extra_prefix: bool) {
    let mut f = Fixture::new("RESTART_CONTAINED_ALIAS", false).await;
    let group: Jid = "120363000000000029@g.us".parse().unwrap();
    let mut peer = joined_group_sender(&f.client, "100000000000029:75@lid", &group).await;
    f.stanza = p1_group_parts(&mut peer, &group, &f.info.id, bodies).await;
    f.info = f.client.parse_message_info(f.stanza.get()).await.unwrap();
    f.hook.fail.store(true, Ordering::SeqCst);
    f.receive().await;
    let original = f.pending().await.unwrap();
    let backend = f.client.persistence_manager.backend();
    let chat = group.to_string();
    let id = f.info.id.to_string();
    let alias = peer.jid.to_non_ad().to_string();
    let recorded = durability::decode_pending_parts(&original).unwrap();
    let encode = |parts: &[&str]| {
        let bytes: Vec<Vec<u8>> = parts
            .iter()
            .map(|body| {
                // Reuse every wire field, including the fixture's unknown field.
                let message = recorded
                    .iter()
                    .find(|message| message.conversation.as_deref() == Some(*body))
                    .unwrap();
                let mut bytes = Vec::new();
                waproto::codec::message_encode_into(message, &mut bytes);
                bytes
            })
            .collect();
        if bytes.len() == 1 {
            bytes[0].clone()
        } else {
            durability::encode_pending_parts(&bytes.iter().map(Vec::as_slice).collect::<Vec<_>>())
        }
    };
    let exact_bytes = encode(exact);
    backend
        .store_pending_inbound(&chat, &alias, &id, &exact_bytes)
        .await
        .unwrap();
    let extra = "100000000000029:76@lid";
    if extra_prefix {
        backend
            .store_pending_inbound(&chat, extra, &id, &encode(&bodies[..1]))
            .await
            .unwrap();
    }
    f.restart().await;
    f.stanza = with_participant(&f.stanza, &peer.jid.to_non_ad());
    f.receive().await;
    assert_eq!(
        f.hook.attempts.load(Ordering::SeqCst),
        bodies.len(),
        "a complete compatible alias must replay in its recorded order"
    );
    assert_eq!(f.receipts(), 0);
    assert_eq!(f.pending().await.unwrap(), original);
    let extended = backend
        .get_pending_inbound(&chat, &alias, &id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        durability::decode_pending_parts(&extended)
            .unwrap()
            .iter()
            .map(|message| message.conversation.as_deref().unwrap())
            .collect::<Vec<_>>(),
        bodies
    );
    f.hook.fail.store(false, Ordering::SeqCst);
    f.receive().await;
    assert_eq!(f.hook.committed.load(Ordering::SeqCst), bodies.len());
    assert_eq!(f.published(), bodies);
    assert_eq!(f.receipts(), 1);
    for sender in [peer.jid.to_string(), alias, extra.to_owned()] {
        assert!(
            backend
                .get_pending_inbound(&chat, &sender, &id)
                .await
                .unwrap()
                .is_none()
        );
    }
    assert_eq!(f.client.inbound_commit_batch.retention.stats(), (0, 0));
}
#[tokio::test]
async fn restart_contained_alias_exact_tail_uses_complete_recorded_sequence() {
    restart_contained_alias(&["A", "B"], &["B"], false).await;
}
#[tokio::test]
async fn restart_contained_alias_preserves_repeated_parts_and_all_keys() {
    restart_contained_alias(&["A", "A", "B"], &["A", "B"], true).await;
}

async fn retained_key_share_waits_for_consumer(cancel: bool) {
    let f = Fixture::new("KEY_SHARE_CONSUMER_COMMIT", false).await;
    f.receive().await;
    let mut info = f.info.clone();
    info.id = "KEY_SHARE_TICKET".into();
    let info = Arc::new(info);
    let mut request = wa::message::AppStateSyncKeyRequest::default();
    let mut key_id = wa::message::AppStateSyncKeyId::default();
    key_id.key_id = Some(vec![4, 3, 2, 1]);
    request.key_ids.push(key_id);
    let mut protocol = wa::message::ProtocolMessage::default();
    protocol.r#type = Some(wa::message::protocol_message::Type::AppStateSyncKeyRequest);
    protocol.app_state_sync_key_request = buffa::MessageField::some(request.clone());
    let mut message = wa::Message::default();
    message.protocol_message = buffa::MessageField::some(protocol);
    let items: Arc<[InboundMessage]> = Arc::from([InboundMessage::builder()
        .info(info.clone())
        .message(Arc::new(message))
        .build()]);
    let retention = &f.client.inbound_commit_batch.retention;
    retention.begin(&info, retention.admit(100)).await;
    let Some(InboundCommitState::Deferred(Some(ticket))) = retention.stage(&items, true) else {
        panic!("key request must have a tracked commit")
    };
    let (items, _) = retention.seal(&info, false);
    f.hook.pause_first.store(true, Ordering::SeqCst);
    f.hook.fail.store(!cancel, Ordering::SeqCst);
    let task = tokio::spawn({
        let client = f.client.clone();
        let items = items.clone();
        async move {
            client
                .commit_inbound_batch(items, BatchOrigin::Live, None)
                .await
        }
    });
    f.hook.entered.notified().await;
    assert_eq!(ticket.state(), InboundCommitTicketState::Pending);
    let sent_before = f.transport.sent_count();
    f.client.schedule_app_state_sync_key_share(
        info.source.sender.clone(),
        request,
        Some(ticket.clone()),
    );
    tokio::time::sleep(std::time::Duration::from_millis(30)).await;
    assert!(!has_message_to_after(
        &f.transport.sent(),
        sent_before,
        &info.source.sender.to_string()
    ));
    if cancel {
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
    } else {
        f.hook.release.notify_one();
        assert!(task.await.unwrap());
    }
    assert_eq!(ticket.state(), InboundCommitTicketState::Pending);
    assert!(!has_message_to_after(
        &f.transport.sent(),
        sent_before,
        &info.source.sender.to_string()
    ));
    f.hook.fail.store(false, Ordering::SeqCst);
    assert!(
        f.client
            .commit_inbound_batch(items, BatchOrigin::Live, None)
            .await
    );
    assert_eq!(ticket.state(), InboundCommitTicketState::Durable);
    // A live consumer commit must wake the job without an offline-sync event.
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        while !has_message_to_after(
            &f.transport.sent(),
            sent_before,
            &info.source.sender.to_string(),
        ) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("successful live hook must release the deferred key share");
}

#[tokio::test]
async fn retained_key_share_waits_after_hook_failure_and_wakes_on_retry() {
    retained_key_share_waits_for_consumer(false).await;
}

#[tokio::test]
async fn retained_key_share_waits_after_hook_cancellation_and_wakes_on_retry() {
    retained_key_share_waits_for_consumer(true).await;
}

async fn restart_merged_alias_sequence(first_device: bool, repeated: bool) {
    let mut f = Fixture::new("RESTART_MERGED_ALIAS_SEQUENCE", false).await;
    let group: Jid = "120363000000000030@g.us".parse().unwrap();
    let mut peer = joined_group_sender(&f.client, "100000000000030:75@lid", &group).await;
    let bare = peer.jid.to_non_ad();
    let (first, second) = if first_device {
        (peer.jid.clone(), bare)
    } else {
        (bare, peer.jid.clone())
    };
    let original = p1_group_parts(
        &mut peer,
        &group,
        &f.info.id,
        if repeated { &["A", "A"] } else { &["A"] },
    )
    .await;
    f.stanza = with_participant(&original, &first);
    f.info = f.client.parse_message_info(f.stanza.get()).await.unwrap();
    f.hook.fail.store(true, Ordering::SeqCst);
    f.receive().await;
    let appended = p1_group_parts(&mut peer, &group, &f.info.id, &["B"]).await;
    f.stanza = with_participant(&appended, &second);
    f.info = f.client.parse_message_info(f.stanza.get()).await.unwrap();
    f.receive().await;
    assert_eq!(f.receipts(), 0);
    f.restart().await;
    f.hook.fail.store(false, Ordering::SeqCst);
    f.receive().await;
    let expected = if repeated {
        vec!["A", "A", "B"]
    } else {
        vec!["A", "B"]
    };
    assert_eq!(
        f.published(),
        expected,
        "restart must preserve canonical arrival order and multiplicity across sender spellings"
    );
    assert_eq!(f.receipts(), 1);
    for sender in [&first, &second] {
        assert!(
            f.client
                .persistence_manager
                .backend()
                .get_pending_inbound(&group.to_string(), &sender.to_string(), &f.info.id)
                .await
                .unwrap()
                .is_none(),
            "settle each original sender key"
        );
    }
}

#[tokio::test]
async fn restart_merged_device_to_bare_preserves_part_order() {
    restart_merged_alias_sequence(true, false).await;
}

#[tokio::test]
async fn restart_merged_bare_to_device_preserves_repeated_parts() {
    restart_merged_alias_sequence(false, true).await;
}

#[tokio::test]
async fn recovered_middle_part_uses_retry_order_including_repeated_occurrences() {
    for repeated in [false, true] {
        let mut f = Fixture::new("RECOVERED_MIDDLE_PART", false).await;
        let group: Jid = "120363000000000031@g.us".parse().unwrap();
        let mut peer = joined_group_sender(&f.client, "100000000000031:75@lid", &group).await;
        let retained = if repeated {
            vec!["A", "A", "C"]
        } else {
            vec!["A", "C"]
        };
        let recovered = if repeated {
            vec!["A", "B", "A", "C"]
        } else {
            vec!["A", "B", "C"]
        };
        f.stanza = p1_group_parts(&mut peer, &group, &f.info.id, &retained).await;
        f.info = f.client.parse_message_info(f.stanza.get()).await.unwrap();
        f.hook.fail.store(true, Ordering::SeqCst);
        f.receive().await;
        assert_eq!(f.receipts(), 0);
        f.stanza = p1_group_parts(&mut peer, &group, &f.info.id, &recovered).await;
        f.hook.fail.store(false, Ordering::SeqCst);
        f.receive().await;
        assert_eq!(
            f.published(),
            recovered,
            "a recovered middle occurrence must precede its retained successor"
        );
        assert_eq!(f.receipts(), 1);
    }
}

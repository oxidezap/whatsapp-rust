use super::traits::StanzaHandler;
use crate::client::{ChatLane, Client, NotificationScope, QueuedChatMessage};
use async_trait::async_trait;
use log::warn;
use std::sync::Arc;
use wacore::stanza::wire_tags::StanzaTag;

/// WA Web: `WAWebMessageQueue` uses `promiseTimeout(r(), 2e4)` per queued handler.
const MAX_MESSAGE_DELAY_MS: u64 = 20_000;

/// How long a lane worker waits for its next message before exiting.
///
/// A worker awaits `handle_incoming_message_scoped` inline, so its task holds
/// that future's whole state machine (~9 KiB) for as long as the worker lives,
/// message or no message. Kept alive for the connection, that is one such
/// future per chat that ever spoke, bounded only by `chat_lanes_capacity`;
/// a client in a few thousand groups parked tens of MiB in idle workers. An
/// idle worker now exits and the next message for the chat spawns a fresh
/// one, so the cost is one task per burst of activity instead of per chat.
///
/// Long enough that a conversation in progress never respawns between
/// replies; short against the hours a connection stays up.
const LANE_IDLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(60);

/// Handler for `<message>` stanzas.
///
/// Messages are processed sequentially per-chat using a mailbox pattern to prevent
/// race conditions where a later message could be processed before the PreKey
/// message that establishes the Signal session.
#[derive(Default)]
pub(crate) struct MessageHandler;

impl MessageHandler {
    pub(crate) async fn handle_inline(
        client: Arc<Client>,
        node: Arc<wacore_binary::OwnedNodeRef>,
        cancelled: &mut bool,
    ) -> bool {
        Self::handle_inline_scoped(client, node, cancelled, None).await
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.recv.message_enqueue", level = "debug", skip_all)
    )]
    pub(crate) async fn handle_inline_scoped(
        client: Arc<Client>,
        node: Arc<wacore_binary::OwnedNodeRef>,
        cancelled: &mut bool,
        scope: Option<NotificationScope<'_>>,
    ) -> bool {
        let shutdown = scope.map_or_else(
            || client.connection_shutdown_signal(),
            NotificationScope::shutdown,
        );
        let generation = client
            .connection_generation
            .load(std::sync::atomic::Ordering::Acquire);
        if shutdown.is_fired()
            || scope.is_some_and(|scope| !scope.is_current(&client))
            || client
                .inbound_queues()
                .draining
                .load(std::sync::atomic::Ordering::Acquire)
        {
            *cancelled = true;
            return true;
        }
        let chat_jid = match node.attrs().optional_jid("from") {
            // Normalize AD metadata so the same chat always maps to one lane
            Some(jid) if jid.device > 0 || jid.agent > 0 => jid.to_non_ad(),
            Some(jid) => jid,
            None => {
                warn!("Message stanza missing required 'from' attribute");
                return false;
            }
        };

        // Single-flight: get_with_by_ref guarantees exactly one init runs per key,
        // preventing duplicate workers for the same chat (TOCTOU race).
        let lane = client
            .chat_lanes
            .get_with_by_ref(&chat_jid, async {
                let budget = client.inbound_queues().chat_budget(&chat_jid);
                create_chat_lane(
                    &client,
                    Arc::new(async_lock::Mutex::new(())),
                    Arc::new(async_lock::Mutex::new(())),
                    budget,
                )
            })
            .await;

        // Lock serializes enqueue order for this chat, the replacement
        // below included (see `create_chat_lane` for why it outlives the lane).
        let _guard = lane.enqueue_lock.lock().await;

        // A lane lookup or enqueue lock can resume after reconnection. Do not
        // admit an old notification to a worker created by the new connection.
        if shutdown.is_fired()
            || client
                .connection_generation
                .load(std::sync::atomic::Ordering::Acquire)
                != generation
            || scope.is_some_and(|scope| !scope.is_current(&client))
        {
            *cancelled = true;
            return true;
        }

        // An initializer can publish after cleanup cleared the cache. Refuse
        // fresh work on that retired worker and use the ordinary replacement path.
        if lane.generation != generation {
            lane.queue_tx.close();
        }

        let node = match lane
            .enqueue_scoped(node, shutdown.clone(), &*client.runtime)
            .await
        {
            Ok(()) => return true,
            // The worker went idle and closed its queue (see
            // `LANE_IDLE_TIMEOUT`). Replace the lane; the successor worker
            // starts only once the idle one has finished draining.
            Err(async_channel::TrySendError::Closed(queued)) => queued.node,
            Err(e) => {
                warn!("Failed to enqueue message for processing: {e}");
                // Cancel ack so server redelivers
                *cancelled = true;
                client.drain_overloaded_inbound(&shutdown);
                return true;
            }
        };

        // A caller that queued behind this lock on the same stale lane finds
        // the replacement already cached and joins it instead of replacing
        // it again. Nothing is enqueued before the lane is in the cache, so a
        // cancellation here leaves at worst an empty worker that idles out.
        let fresh = match client.chat_lanes.get(&chat_jid).await {
            Some(current) if !current.queue_tx.same_channel(&lane.queue_tx) => current,
            _ => {
                client.chat_lanes.invalidate(&chat_jid).await;
                client
                    .chat_lanes
                    .get_with_by_ref(&chat_jid, async {
                        create_chat_lane(
                            &client,
                            Arc::clone(&lane.enqueue_lock),
                            Arc::clone(&lane.worker_running),
                            Arc::clone(&lane.byte_budget),
                        )
                    })
                    .await
            }
        };
        if shutdown.is_fired()
            || client
                .connection_generation
                .load(std::sync::atomic::Ordering::Acquire)
                != generation
            || scope.is_some_and(|scope| !scope.is_current(&client))
        {
            *cancelled = true;
            return true;
        }
        if let Err(e) = fresh
            .enqueue_scoped(node, shutdown.clone(), &*client.runtime)
            .await
        {
            warn!("Failed to enqueue message for processing: {e}");
            *cancelled = true;
            client.drain_overloaded_inbound(&shutdown);
        }

        true
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl StanzaHandler for MessageHandler {
    fn tag(&self) -> &'static str {
        StanzaTag::Message.as_str()
    }

    async fn handle(
        &self,
        client: Arc<Client>,
        node: Arc<wacore_binary::OwnedNodeRef>,
        cancelled: &mut bool,
    ) -> bool {
        Self::handle_inline(client, node, cancelled).await
    }
}

/// Construct a ChatLane with a spawned worker task. Extracted to keep the
/// init closure passed to `get_with_by_ref` small.
///
/// Both locks belong to the chat rather than the lane, which is why they are
/// parameters: a lane that replaces an idle-exited one passes the
/// predecessor's, and only the chat's first lane mints new ones. Sharing
/// `enqueue_lock` keeps every enqueue for the chat in one total order
/// whichever lane generation a handler fetched, and the swap itself runs
/// under it; sharing `worker_running` makes the new worker wait for the old
/// one to finish draining, so the two never process the same chat at once.
fn create_chat_lane(
    client: &Arc<Client>,
    enqueue_lock: Arc<async_lock::Mutex<()>>,
    worker_running: Arc<async_lock::Mutex<()>>,
    byte_budget: Arc<crate::inbound_queue::ByteBudget>,
) -> ChatLane {
    let (tx, rx) =
        async_channel::bounded::<QueuedChatMessage>(crate::inbound_queue::CHAT_QUEUE_ITEMS);
    let rx = crate::inbound_queue::DrainingReceiver(rx);

    let client_for_worker = client.clone();
    let spawn_generation = client
        .connection_generation
        .load(std::sync::atomic::Ordering::Acquire);
    let running = Arc::clone(&worker_running);

    client
        .runtime
        .spawn(Box::pin(async move {
            // Queue behind the worker this lane replaced, if it is still
            // draining the messages that raced its idle exit.
            let _running = running.lock_arc().await;
            loop {
                // A burst is served straight off the queue; the idle timer is
                // only armed once the queue is empty, so it costs nothing per
                // message while the chat is busy.
                let next = match rx.try_recv() {
                    Ok(queued) => queued,
                    Err(async_channel::TryRecvError::Closed) => break,
                    Err(async_channel::TryRecvError::Empty) => {
                        match wacore::runtime::timeout(
                            &*client_for_worker.runtime,
                            LANE_IDLE_TIMEOUT,
                            rx.recv(),
                        )
                        .await
                        {
                            Ok(Ok(queued)) => queued,
                            Ok(Err(_)) => break,
                            Err(wacore::runtime::Elapsed) => {
                                // Stop accepting first, then drain whatever
                                // was enqueued before the close: an enqueue
                                // that lands after it is told `Closed` and
                                // replaces this lane.
                                rx.close();
                                while let Ok(queued) = rx.try_recv() {
                                    if !process_queued(&client_for_worker, queued, spawn_generation)
                                        .await
                                    {
                                        break;
                                    }
                                }
                                break;
                            }
                        }
                    }
                };
                if !process_queued(&client_for_worker, next, spawn_generation).await {
                    break;
                }
            }
        }))
        .detach();

    ChatLane {
        generation: spawn_generation,
        enqueue_lock,
        queue_tx: tx,
        byte_budget,
        global_budget: Arc::clone(&client.inbound_queues().chat),
        work: Arc::clone(&client.inbound_queues().work),
        worker_running,
    }
}

/// Process one queued message on its lane worker. Returns `false` when the
/// worker belongs to a torn-down connection and must stop.
async fn process_queued(
    client: &Arc<Client>,
    QueuedChatMessage {
        shutdown,
        node: msg_node,
        lane_liveness, // Prevents capacity eviction until processing finishes.
        work,
    }: QueuedChatMessage,
    spawn_generation: u64,
) -> bool {
    if client
        .connection_generation
        .load(std::sync::atomic::Ordering::Acquire)
        != spawn_generation
    {
        log::debug!(target: "MessageQueue", "Stale worker exiting; remaining messages will be redelivered by server");
        return false;
    }
    // Two clock reads per message, kept: sampling or gating on lane
    // backlog would stop reporting the single pathological message
    // this guard exists to catch.
    let start = wacore::time::Instant::now();
    // Classification can drop the parsed node before its heavy processing
    // finishes. Keep this admission charged until the whole handler completes;
    // escaped slices independently retain the same charge for longer.
    let _backing = msg_node.backing_bytes();
    if msg_node.tag() == StanzaTag::Notification.as_str() {
        client
            .process_queued_group_notification(msg_node, spawn_generation, shutdown)
            .await;
    } else {
        // Awaited inline (not boxed): the future lives in this
        // once-per-chat worker task instead of a fresh ~9 KB heap box
        // per message, which dominated per-message allocation churn.
        Arc::clone(client)
            .handle_incoming_message_scoped(msg_node, spawn_generation)
            .await;
    }
    let elapsed = start.elapsed();
    if elapsed.as_millis() as u64 > MAX_MESSAGE_DELAY_MS {
        warn!(
            target: "MessageQueue",
            "Message processing took {:.1}s (MAX_MESSAGE_DELAY is {}s)",
            elapsed.as_secs_f64(),
            MAX_MESSAGE_DELAY_MS / 1000
        );
    }
    drop(lane_liveness);
    drop(work);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::node_to_owned_ref;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::jid::Jid;

    #[tokio::test]
    async fn a_late_initializer_cannot_admit_fresh_work_to_a_retired_worker() {
        let client = crate::test_utils::create_test_client().await;
        let chat: Jid = "120363000000042@g.us".parse().unwrap();
        let running = Arc::new(async_lock::Mutex::new(()));
        let blocked = running.lock().await;
        let retired = create_chat_lane(
            &client,
            Arc::default(),
            Arc::clone(&running),
            Arc::new(crate::inbound_queue::ByteBudget::new(
                crate::inbound_queue::CHAT_BYTES,
            )),
        );
        client.notify_connection_shutdown();
        client.chat_lanes.clear().await;
        client
            .connection_generation
            .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
        client.reset_connection_shutdown();
        // Finish the old cache initializer after the reconnect's clear.
        client
            .chat_lanes
            .insert(chat.clone(), retired.clone())
            .await;
        let mut cancelled = false;
        MessageHandler::handle_inline(
            Arc::clone(&client),
            message_for(&chat, "fresh"),
            &mut cancelled,
        )
        .await;
        assert!(!cancelled);
        let fresh = client.chat_lanes.get(&chat).await.unwrap();
        assert!(!fresh.queue_tx.same_channel(&retired.queue_tx));
        assert!(retired.queue_tx.is_closed());
        assert_eq!(retired.queue_tx.len(), 0);
        assert_eq!(fresh.queue_tx.len(), 1);
        fresh.queue_tx.close();
        drop(blocked);
    }

    #[tokio::test]
    async fn a_real_chat_lane_bounds_a_burst_behind_a_blocked_worker() {
        let client = crate::test_utils::create_test_client().await;
        let running = Arc::new(async_lock::Mutex::new(()));
        let blocked = running.lock().await;
        let lane = create_chat_lane(
            &client,
            Arc::default(),
            Arc::clone(&running),
            Arc::new(crate::inbound_queue::ByteBudget::new(
                crate::inbound_queue::CHAT_BYTES,
            )),
        );
        let mut accepted = 0;
        for id in 0..128 {
            let node = node_to_owned_ref(
                &NodeBuilder::new("message")
                    .attr("from", "120363000000042@g.us")
                    .attr("id", id.to_string())
                    .bytes(vec![id as u8; 64 * 1024])
                    .build(),
            );
            if lane.try_enqueue(node).is_ok() {
                accepted += 1;
            }
        }
        assert_eq!(lane.queue_tx.capacity(), Some(64));
        assert_eq!(accepted, 64);
        assert_eq!(lane.queue_tx.len(), 64);
        let retained = 32 * 1024 * 1024 - client.inbound_queues().chat.available();
        assert!((4 * 1024 * 1024..5 * 1024 * 1024).contains(&retained));
        eprintln!("production lane: {accepted}/128 accepted, {retained} charged bytes");
        lane.queue_tx.close();
        drop(blocked);
    }

    fn message_for(chat: &Jid, id: &str) -> Arc<wacore_binary::OwnedNodeRef> {
        node_to_owned_ref(
            &NodeBuilder::new("message")
                .attr("from", chat.clone())
                .attr("id", id)
                .build(),
        )
    }

    /// An idle worker closes its queue and exits; the next message for the
    /// chat gets a fresh lane whose worker queues behind the old one.
    #[tokio::test(start_paused = true)]
    async fn an_idle_lane_worker_exits_and_the_next_message_respawns_it() {
        let client = crate::test_utils::create_test_client().await;
        let chat: Jid = "120363000000000031@g.us".parse().unwrap();

        let mut cancelled = false;
        assert!(
            MessageHandler::handle_inline(
                Arc::clone(&client),
                message_for(&chat, "A"),
                &mut cancelled
            )
            .await
        );
        assert!(!cancelled);
        let first = client.chat_lanes.get(&chat).await.expect("lane created");
        let first_tx = first.queue_tx.clone();
        assert!(!first_tx.is_closed());

        // Paused time: the sleep advances the clock instead of waiting.
        tokio::time::sleep(LANE_IDLE_TIMEOUT + Duration::from_secs(1)).await;
        for _ in 0..100 {
            if first_tx.is_closed() {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert!(first_tx.is_closed(), "idle worker must close its queue");
        assert!(
            first.worker_running.try_lock().is_some(),
            "an exited worker must release its running lock"
        );

        let mut cancelled = false;
        assert!(
            MessageHandler::handle_inline(
                Arc::clone(&client),
                message_for(&chat, "B"),
                &mut cancelled
            )
            .await
        );
        assert!(
            !cancelled,
            "a message after the idle exit is accepted, not redelivered"
        );
        let second = client.chat_lanes.get(&chat).await.expect("lane replaced");
        assert!(!second.queue_tx.same_channel(&first_tx));
        assert!(!second.queue_tx.is_closed());
        assert!(
            Arc::ptr_eq(&first.worker_running, &second.worker_running),
            "the successor serializes behind the predecessor's running lock"
        );
        assert!(
            Arc::ptr_eq(&first.enqueue_lock, &second.enqueue_lock),
            "the chat's enqueue lock survives the swap"
        );
    }

    /// A message enqueued in the window between the idle check and the close
    /// is still processed by the exiting worker, and a message after the close
    /// is not lost.
    #[tokio::test(start_paused = true)]
    async fn a_lane_that_stays_busy_never_exits() {
        let client = crate::test_utils::create_test_client().await;
        let chat: Jid = "120363000000000032@g.us".parse().unwrap();
        let mut cancelled = false;
        MessageHandler::handle_inline(Arc::clone(&client), message_for(&chat, "A"), &mut cancelled)
            .await;
        let lane = client.chat_lanes.get(&chat).await.expect("lane created");
        for i in 0..5 {
            tokio::time::sleep(LANE_IDLE_TIMEOUT / 2).await;
            MessageHandler::handle_inline(
                Arc::clone(&client),
                message_for(&chat, &format!("M{i}")),
                &mut cancelled,
            )
            .await;
            assert!(!cancelled);
        }
        let same = client.chat_lanes.get(&chat).await.expect("lane kept");
        assert!(same.queue_tx.same_channel(&lane.queue_tx));
        assert!(!lane.queue_tx.is_closed());
    }

    use std::future::Future;
    use std::pin::Pin;
    use std::time::Duration;

    struct SizingRuntime {
        inner: crate::runtime_impl::TokioRuntime,
        sizes: Arc<std::sync::Mutex<Vec<usize>>>,
    }

    #[async_trait::async_trait]
    impl wacore::runtime::Runtime for SizingRuntime {
        fn spawn(
            &self,
            future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
        ) -> wacore::runtime::AbortHandle {
            self.sizes
                .lock()
                .expect("sizes mutex")
                .push(size_of_val(&*future));
            self.inner.spawn(future)
        }

        fn spawn_detached(&self, future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>) {
            self.sizes
                .lock()
                .expect("sizes mutex")
                .push(size_of_val(&*future));
            self.inner.spawn_detached(future);
        }

        fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            self.inner.sleep(duration)
        }

        fn spawn_blocking(
            &self,
            f: Box<dyn FnOnce() + Send + 'static>,
        ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            self.inner.spawn_blocking(f)
        }

        fn yield_now(&self) -> Option<Pin<Box<dyn Future<Output = ()> + Send>>> {
            self.inner.yield_now()
        }
    }

    // Exact and width independent: node, shutdown, lane liveness and drain guard.
    // Rebaseline per [layout asserts](../../agent_docs/testing.md).
    #[test]
    fn queued_chat_message_keeps_four_handles() {
        assert_eq!(size_of::<QueuedChatMessage>(), 4 * size_of::<usize>());
    }

    #[cfg(feature = "bench-harness")]
    #[test]
    fn worker_throughput_fixture_drains_a_large_single_chat_burst_in_windows() {
        let harness = crate::bench_support::MultiLaneReceiveHarness::new(1);
        let before = harness.messages_delivered();
        harness.enqueue_and_drain(&harness.generate_burst(1, 256));
        assert_eq!(harness.messages_delivered() - before, 256);
        harness.close_lanes();
    }

    #[cfg(feature = "bench-harness")]
    #[test]
    fn isolated_cold_lane_enqueue_does_not_poll_workers() {
        let harness = crate::bench_support::MultiLaneReceiveHarness::new(256);
        for lanes in [1, 32, 256] {
            let batch = harness.generate_burst(lanes, lanes);
            let before = harness.messages_delivered();
            let target = harness.enqueue(&batch);
            assert_eq!(harness.messages_delivered(), before);
            assert_eq!(target, before + lanes as u64);
            harness.drain(target);
            assert_eq!(harness.messages_delivered(), target);
            harness.close_lanes();
            assert_eq!(harness.active_lanes(), 0);
        }
    }

    async fn client_with_sizing_runtime(sizes: Arc<std::sync::Mutex<Vec<usize>>>) -> Arc<Client> {
        let sizing_runtime = SizingRuntime {
            inner: crate::runtime_impl::TokioRuntime,
            sizes,
        };
        let persistence_manager = Arc::new(
            crate::store::persistence_manager::PersistenceManager::new(
                crate::test_utils::create_test_backend().await,
            )
            .await
            .expect("persistence manager"),
        );
        Client::builder()
            .with_runtime(sizing_runtime)
            .with_persistence_manager(persistence_manager)
            .with_transport_factory(crate::transport::mock::MockTransportFactory::new())
            .with_http_client(crate::test_utils::MockHttpClient)
            .build()
            .await
            .expect("client build")
            .into_client()
    }

    #[tokio::test]
    #[cfg(target_pointer_width = "64")]
    async fn chat_lane_future_stays_within_budget() {
        let sizes = Arc::new(std::sync::Mutex::new(Vec::new()));
        let client = client_with_sizing_runtime(Arc::clone(&sizes)).await;
        let lane = create_chat_lane(
            &client,
            Arc::new(async_lock::Mutex::new(())),
            Arc::new(async_lock::Mutex::new(())),
            Arc::new(crate::inbound_queue::ByteBudget::new(
                crate::inbound_queue::CHAT_BYTES,
            )),
        );
        let task_size = sizes.lock().unwrap().pop().unwrap();
        lane.queue_tx.close();

        // Each active chat retains this allocation even while its worker waits.
        // On the pinned nightly: 6504 bytes (8096 with tracing spans), versus
        // 7568 (8432 with tracing) when PDO recovery was inline in every lane.
        let budget = if cfg!(feature = "tracing") {
            8 * 1024
        } else {
            7 * 1024
        };
        assert!(
            task_size <= budget,
            "chat lane future grew to {task_size} bytes; inspect the receive layout audit"
        );
    }

    #[tokio::test]
    #[ignore = "layout diagnostic: run explicitly with --ignored --nocapture"]
    async fn audit_receive_future_and_struct_layouts() {
        let sizes = Arc::new(std::sync::Mutex::new(Vec::new()));
        let client = client_with_sizing_runtime(Arc::clone(&sizes)).await;

        let dummy_chat: Jid = "120363000000000031@g.us".parse().unwrap();
        let dummy_node = message_for(&dummy_chat, "test_msg");

        // 1. Concrete task future spawned in create_chat_lane
        let _lane = create_chat_lane(
            &client,
            Arc::new(async_lock::Mutex::new(())),
            Arc::new(async_lock::Mutex::new(())),
            Arc::new(crate::inbound_queue::ByteBudget::new(
                crate::inbound_queue::CHAT_BYTES,
            )),
        );
        let chat_lane_task_size = sizes.lock().unwrap().pop().unwrap();

        // 2. process_queued future
        let queued = QueuedChatMessage {
            shutdown: wacore::runtime::ShutdownSignal::never(),
            node: Arc::clone(&dummy_node),
            lane_liveness: Arc::new(async_lock::Mutex::new(())),
            work: None,
        };
        let process_queued_fut = process_queued(&client, queued, 0);
        let process_queued_size = size_of_val(&process_queued_fut);
        drop(process_queued_fut);

        // 3. handle_incoming_message_scoped future
        let handle_scoped_fut =
            Arc::clone(&client).handle_incoming_message_scoped(Arc::clone(&dummy_node), 0);
        let handle_scoped_size = size_of_val(&handle_scoped_fut);
        drop(handle_scoped_fut);

        // 4. handle_incoming_message future
        let handle_fut = Arc::clone(&client).handle_incoming_message(Arc::clone(&dummy_node));
        let handle_size = size_of_val(&handle_fut);
        drop(handle_fut);

        // 5. classify_incoming_message future
        let classify_fut = client.classify_incoming_message(&dummy_node);
        let classify_size = size_of_val(&classify_fut);
        drop(classify_fut);

        // 6. process_classified_message future
        let dummy_classified = crate::message::ClassifiedMessage {
            info: Arc::new(wacore::types::message::MessageInfo::default()),
            sender_encryption_jid: dummy_chat.clone(),
            session_payloads: Vec::new(),
            group_payloads: Vec::new(),
            bot_payloads: Vec::new(),
            max_sender_retry_count: 0,
            decrypt_fail_mode: crate::types::events::DecryptFailMode::default(),
            ack_recipient: None,
        };
        let process_classified_fut =
            Arc::clone(&client).process_classified_message(dummy_classified, 0);
        let process_classified_size = size_of_val(&process_classified_fut);
        drop(process_classified_fut);

        // 7. process_session_enc_batch future
        let dummy_info = Arc::new(wacore::types::message::MessageInfo::default());
        let session_batch_fut = Arc::clone(&client).process_session_enc_batch(
            Vec::new(),
            &dummy_info,
            &dummy_chat,
            crate::types::events::DecryptFailMode::default(),
        );
        let session_batch_size = size_of_val(&session_batch_fut);
        drop(session_batch_fut);

        // 8. handle_decrypted_plaintext future
        let handle_plaintext_fut = client.handle_decrypted_plaintext(
            "msg",
            Vec::new(),
            0,
            0,
            crate::message::EncNodeAnnotations {
                state: None,
                session_type: None,
            },
            &dummy_info,
        );
        let handle_plaintext_size = size_of_val(&handle_plaintext_fut);
        drop(handle_plaintext_fut);

        // 9. dispatch_parsed_message future
        let dispatch_fut = client.dispatch_parsed_message(
            waproto::whatsapp::Message::default(),
            &dummy_info,
            false,
        );
        let dispatch_size = size_of_val(&dispatch_fut);
        drop(dispatch_fut);
        let shared_dispatch_fut = client.dispatch_shared_message_with_decrypted(
            Arc::new(waproto::whatsapp::Message::default()),
            &dummy_info,
            false,
            None,
        );
        let shared_dispatch_size = size_of_val(&shared_dispatch_fut);
        drop(shared_dispatch_fut);

        // 10. harness receive async block future
        let harness_fut = async {
            Arc::clone(&client)
                .handle_incoming_message(Arc::clone(&dummy_node))
                .await;
            client
                .outbound_flush
                .flush(&*client.runtime, Duration::from_secs(5))
                .await;
        };
        let harness_fut_size = size_of_val(&harness_fut);
        drop(harness_fut);

        println!("=== SIZEOF REPORT ===");
        println!("chat_lane task future (pointee): {chat_lane_task_size} bytes");
        println!("process_queued future: {process_queued_size} bytes");
        println!("handle_incoming_message_scoped future: {handle_scoped_size} bytes");
        println!("handle_incoming_message future: {handle_size} bytes");
        println!("harness block_on future: {harness_fut_size} bytes");
        println!("classify_incoming_message future: {classify_size} bytes");
        println!("process_classified_message future: {process_classified_size} bytes");
        println!("process_session_enc_batch future: {session_batch_size} bytes");
        println!("handle_decrypted_plaintext future: {handle_plaintext_size} bytes");
        println!("dispatch_parsed_message future: {dispatch_size} bytes");
        println!("dispatch_shared_message_with_decrypted future: {shared_dispatch_size} bytes");

        println!(
            "waproto::whatsapp::Message: {} bytes",
            size_of::<waproto::whatsapp::Message>()
        );
        println!(
            "MessageInfo: {} bytes",
            size_of::<wacore::types::message::MessageInfo>()
        );
        println!(
            "ClassifiedMessage: {} bytes",
            size_of::<crate::message::ClassifiedMessage>()
        );
        println!(
            "QueuedChatMessage: {} bytes",
            size_of::<QueuedChatMessage>()
        );
        println!("ChatLane: {} bytes", size_of::<ChatLane>());
        println!(
            "EncPayload: {} bytes",
            size_of::<crate::message::EncPayload>()
        );
        println!(
            "SessionBatchOutcome: {} bytes",
            size_of::<crate::message::SessionBatchOutcome>()
        );
        println!(
            "PlaintextHandleOutcome: {} bytes",
            size_of::<crate::message::PlaintextHandleOutcome>()
        );
        println!(
            "InboundCommitState: {} bytes",
            size_of::<crate::message::InboundCommitState>()
        );
        let pdo = waproto::whatsapp::message::PeerDataOperationRequestResponseMessage::default();
        println!(
            "handle_pdo_response future: {} bytes",
            size_of_val(&client.handle_pdo_response(&pdo, &dummy_info))
        );
        println!(
            "probe_message_dispatch future: {} bytes",
            size_of_val(&client.probe_message_dispatch(
                &dummy_info,
                &waproto::whatsapp::Message::default(),
                None
            ))
        );
        println!("=== END SIZEOF REPORT ===");
    }

    #[test]
    #[cfg(feature = "bench-harness")]
    fn harness_receive_burst_and_worker_drain() {
        let harness = crate::bench_support::ReceiveHarness::new();
        let before = harness.messages_delivered();

        // Limit 2 control: receive_burst under single block_on
        let dm_batch: Vec<_> = (0..5).map(|_| harness.dm_stanza()).collect();
        harness.receive_burst(&dm_batch);
        assert_eq!(harness.messages_delivered() - before, 5);

        let grp_batch: Vec<_> = (0..5).map(|_| harness.group_stanza()).collect();
        harness.receive_burst(&grp_batch);
        assert_eq!(harness.messages_delivered() - before, 10);

        // Limit 3 production worker: enqueue_and_drain through chat lane
        let dm_lane_batch: Vec<_> = (0..5).map(|_| harness.dm_stanza()).collect();
        harness.enqueue_and_drain(&dm_lane_batch);
        assert_eq!(harness.messages_delivered() - before, 15);

        let grp_lane_batch: Vec<_> = (0..5).map(|_| harness.group_stanza()).collect();
        harness.enqueue_and_drain(&grp_lane_batch);
        assert_eq!(harness.messages_delivered() - before, 20);

        harness.close_lanes();
    }

    #[test]
    #[cfg(feature = "bench-harness")]
    fn multilane_harness_burst_and_worker_drain() {
        let harness = crate::bench_support::MultiLaneReceiveHarness::new(256);
        assert_eq!(harness.group_count(), 256);
        assert_eq!(harness.active_lanes(), 0);

        // 1 lane: 10 messages to group 0
        let stanzas_1 = harness.generate_burst(1, 10);
        harness.enqueue_and_drain(&stanzas_1);
        assert_eq!(harness.messages_delivered(), 10);
        assert_eq!(harness.active_lanes(), 1);
        harness.close_lanes();
        assert_eq!(harness.active_lanes(), 0);

        // 32 lanes: 64 messages across 32 groups
        let stanzas_32 = harness.generate_burst(32, 64);
        harness.enqueue_and_drain(&stanzas_32);
        assert_eq!(harness.messages_delivered(), 74);
        assert_eq!(harness.active_lanes(), 32);
        harness.close_lanes();
        assert_eq!(harness.active_lanes(), 0);

        // 256 lanes: 256 messages across 256 groups
        let stanzas_256 = harness.generate_burst(256, 256);
        harness.enqueue_and_drain(&stanzas_256);
        assert_eq!(harness.messages_delivered(), 330);
        assert_eq!(harness.active_lanes(), 256);
        harness.close_lanes();
        assert_eq!(harness.active_lanes(), 0);
    }
    #[tokio::test]
    async fn slow_group_capture_reports_lane_delay() {
        use crate::test_utils::log_capture;
        use portable_atomic::AtomicU64;
        use std::sync::atomic::Ordering;
        const TEST: &str = "handlers::message::tests::slow_group_capture_reports_lane_delay";
        if log_capture::delegated_to_child(TEST) {
            return;
        }
        struct Clock(Arc<AtomicU64>);
        impl wacore::time::MonotonicProvider for Clock {
            fn now_nanos(&self) -> u64 {
                self.0.load(Ordering::Acquire)
            }
        }
        struct SlowCapture(Arc<AtomicU64>);
        #[async_trait::async_trait]
        impl crate::GroupNotificationDurabilityHook for SlowCapture {
            async fn on_notification(
                &self,
                _: Arc<wacore_binary::OwnedNodeRef>,
            ) -> anyhow::Result<()> {
                self.0
                    .fetch_add((MAX_MESSAGE_DELAY_MS + 1) * 1_000_000, Ordering::AcqRel);
                anyhow::bail!("synthetic capture failure after a long wait");
            }
        }
        let clock = Arc::new(AtomicU64::new(0));
        wacore::time::set_monotonic_provider(Clock(clock.clone())).unwrap();
        let (client, transport) = crate::test_utils::create_iq_test_client().await;
        assert!(
            client
                .group_notification_durability_hook
                .set(Arc::new(SlowCapture(clock)))
                .is_ok()
        );
        let node = node_to_owned_ref(
            &NodeBuilder::new("notification")
                .attr("type", "w:gp2")
                .attr("from", "120363000000000000@g.us")
                .attr("id", "slow-capture")
                .build(),
        );
        let logs = log_capture::session();
        let generation = client.connection_generation.load(Ordering::Acquire);
        assert!(
            process_queued(
                &client,
                QueuedChatMessage {
                    node,
                    shutdown: client.connection_shutdown_signal(),
                    lane_liveness: Arc::new(async_lock::Mutex::new(())),
                    work: None,
                },
                generation
            )
            .await
        );
        assert!(
            logs.records_for("MessageQueue")
                .iter()
                .any(|(level, message)| *level == log::Level::Warn
                    && message.contains("Message processing took"))
        );
        assert!(transport.sent().is_empty());
    }
}

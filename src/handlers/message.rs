use super::traits::StanzaHandler;
use crate::client::{ChatLane, Client, QueuedChatMessage};
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
pub struct MessageHandler;

impl MessageHandler {
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.recv.message_enqueue", level = "debug", skip_all)
    )]
    pub(crate) async fn handle_inline(
        client: Arc<Client>,
        node: Arc<wacore_binary::OwnedNodeRef>,
        cancelled: &mut bool,
    ) -> bool {
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
                create_chat_lane(
                    &client,
                    Arc::new(async_lock::Mutex::new(())),
                    Arc::new(async_lock::Mutex::new(())),
                )
            })
            .await;

        // Lock serializes enqueue order for this chat, the replacement
        // below included (see `create_chat_lane` for why it outlives the lane).
        let _guard = lane.enqueue_lock.lock().await;

        let node = match lane.try_enqueue(node) {
            Ok(()) => return true,
            // The worker went idle and closed its queue (see
            // `LANE_IDLE_TIMEOUT`). Replace the lane; the successor worker
            // starts only once the idle one has finished draining.
            Err(async_channel::TrySendError::Closed(queued)) => queued.node,
            Err(e) => {
                warn!("Failed to enqueue message for processing: {e}");
                // Cancel ack so server redelivers
                *cancelled = true;
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
                        )
                    })
                    .await
            }
        };
        if let Err(e) = fresh.try_enqueue(node) {
            warn!("Failed to enqueue message for processing: {e}");
            *cancelled = true;
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
) -> ChatLane {
    let (tx, rx) = async_channel::unbounded::<QueuedChatMessage>();

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
        enqueue_lock,
        queue_tx: tx,
        worker_running,
    }
}

/// Process one queued message on its lane worker. Returns `false` when the
/// worker belongs to a torn-down connection and must stop.
async fn process_queued(
    client: &Arc<Client>,
    QueuedChatMessage {
        node: msg_node,
        lane_liveness, // Prevents capacity eviction until processing finishes.
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
    // Awaited inline (not boxed): the future lives in this
    // once-per-chat worker task instead of a fresh ~9 KB heap box
    // per message, which dominated per-message allocation churn.
    Arc::clone(client)
        .handle_incoming_message_scoped(msg_node, spawn_generation)
        .await;
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
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::node_to_owned_ref;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::jid::Jid;

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
        tokio::time::sleep(LANE_IDLE_TIMEOUT + std::time::Duration::from_secs(1)).await;
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
}

use super::{NotificationHandler, StanzaHandler};
use crate::GroupNotificationDurabilityHook;
use crate::test_utils::{TestEventCollector, create_iq_test_client, node_to_owned_ref};
use crate::types::events::Event;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::sync::Notify;
use wacore_binary::{Jid, OwnedNodeRef, builder::NodeBuilder};

const GROUP: &str = "120363000000000000@g.us";

struct FirstCaptureBarrier {
    entered: Notify,
    release: Notify,
}

#[async_trait::async_trait]
impl GroupNotificationDurabilityHook for FirstCaptureBarrier {
    async fn on_notification(&self, node: Arc<OwnedNodeRef>) -> anyhow::Result<()> {
        if node.get().attrs().optional_string("id").as_deref() == Some("first") {
            self.entered.notify_one();
            self.release.notified().await;
        }
        Ok(())
    }
}

fn participant_notification(id: &str, action: &'static str) -> OwnedNodeRef {
    participant_notification_for(GROUP, id, action)
}

fn participant_notification_for(group: &str, id: &str, action: &'static str) -> OwnedNodeRef {
    Arc::try_unwrap(node_to_owned_ref(
        &NodeBuilder::new("notification")
            .attr("type", "w:gp2")
            .attr("from", group)
            .attr("id", id)
            .attr("offline", "1")
            .children([NodeBuilder::new(action)
                .children([NodeBuilder::new("participant")
                    .attr("jid", "12025550102@s.whatsapp.net")
                    .build()])
                .build()])
            .build(),
    ))
    .unwrap()
}

#[tokio::test]
async fn queued_group_capture_keeps_remove_then_add_in_arrival_order() {
    use std::sync::atomic::Ordering;
    use wacore::client::context::GroupRoutingInfo;
    use wacore::types::message::AddressingMode;

    let (client, _) = create_iq_test_client().await;
    let collector = Arc::new(TestEventCollector::default());
    client.subscribe_handler(collector.clone()).detach();
    client
        .offline_sync_metrics
        .active
        .store(true, Ordering::Release);
    client
        .offline_sync_metrics
        .total_messages
        .store(3, Ordering::Release);
    let group: Jid = GROUP.parse().unwrap();
    let participant: Jid = "12025550102@s.whatsapp.net".parse().unwrap();
    client
        .get_group_cache()
        .insert(
            group.clone(),
            Arc::new(GroupRoutingInfo::new(
                vec![participant.clone()],
                AddressingMode::Pn,
            )),
        )
        .await;
    let hook = Arc::new(FirstCaptureBarrier {
        entered: Notify::new(),
        release: Notify::new(),
    });
    assert!(
        client
            .group_notification_durability_hook
            .set(hook.clone())
            .is_ok()
    );
    assert!(client.processes_inline(participant_notification("first", "remove").get()));
    let first = tokio::spawn({
        let client = client.clone();
        async move {
            client
                .process_decrypted_node(participant_notification("first", "remove"))
                .await
        }
    });
    tokio::time::timeout(Duration::from_secs(5), hook.entered.notified())
        .await
        .unwrap();
    client
        .process_decrypted_node(participant_notification_for(
            "120363000000000099@g.us",
            "other-group",
            "add",
        ))
        .await;
    crate::test_utils::poll_until("unrelated group makes progress during capture", || {
        group_events(&collector) == 1
    })
    .await;
    client
        .process_decrypted_node(participant_notification("second", "add"))
        .await;
    assert_eq!(
        client
            .offline_sync_metrics
            .processed_messages
            .load(Ordering::Acquire),
        3,
        "arrival accounting must not wait behind durable capture"
    );
    hook.release.notify_one();
    tokio::time::timeout(Duration::from_secs(5), first)
        .await
        .unwrap()
        .unwrap();
    crate::test_utils::poll_until("all group effects", || group_events(&collector) == 3).await;
    let ids: Vec<_> = collector
        .events()
        .iter()
        .filter_map(|event| match &**event {
            Event::GroupUpdate(update) if update.group_jid == group => {
                update.notification_id.clone()
            }
            _ => None,
        })
        .collect();
    assert_eq!(ids, ["first", "second"]);
    let info = client.get_group_cache().get(&group).await.unwrap();
    assert_eq!(info.participants, [participant]);
    assert_eq!(
        client
            .offline_sync_metrics
            .processed_messages
            .load(Ordering::Acquire),
        3,
        "the worker must not account for an arrival twice"
    );
}

#[tokio::test]
async fn retirement_while_waiting_for_group_metadata_preserves_current_state() {
    use std::sync::atomic::Ordering;
    use wacore::client::context::GroupRoutingInfo;
    use wacore::types::message::AddressingMode;

    for shutdown_only in [false, true] {
        let (client, transport) = create_iq_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.subscribe_handler(collector.clone()).detach();
        let group: Jid = GROUP.parse().unwrap();
        let participant: Jid = "12025550102@s.whatsapp.net".parse().unwrap();
        let snapshot = Arc::new(GroupRoutingInfo::new(
            vec![participant.clone()],
            AddressingMode::Pn,
        ));
        client
            .get_group_cache()
            .insert(group.clone(), snapshot.clone())
            .await;
        client
            .set_sender_key_status_for_devices(GROUP, &[participant], true, false)
            .await
            .unwrap();
        let metadata = client.lock_group_metadata(&group).await;
        let lock = client.group_distribution_locks.get(&group).await.unwrap();
        let baseline = Arc::strong_count(&lock);
        let hook = Arc::new(RecordingHook::new(false));
        hook.release.notify_one();
        assert!(client.group_notification_durability_hook.set(hook).is_ok());
        let task = tokio::spawn({
            let client = client.clone();
            async move { client.process_node(multi_action_node()).await }
        });
        crate::test_utils::wait_for_lock_waiter(&lock, baseline).await;
        if shutdown_only {
            client.notify_connection_shutdown();
        } else {
            client.connection_generation.fetch_add(1, Ordering::AcqRel);
        }
        drop(metadata);
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .unwrap()
            .unwrap();
        let current = client
            .get_group_cache()
            .get(&group)
            .await
            .expect("retired handler must not invalidate current routing");
        assert!(Arc::ptr_eq(&current, &snapshot));
        assert!(
            !client
                .persistence_manager
                .get_sender_key_devices(GROUP)
                .await
                .unwrap()
                .is_empty()
        );
        assert_eq!(group_events(&collector), 0);
        assert!(transport.sent().is_empty());
    }
}

#[tokio::test]
async fn retirement_from_group_event_stops_remaining_actions_and_ack() {
    struct Retire(Arc<crate::Client>);
    impl wacore::types::events::EventHandler for Retire {
        fn handle_event(&self, event: Arc<Event>) {
            if matches!(&*event, Event::GroupUpdate(_)) {
                self.0
                    .connection_generation
                    .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
            }
        }
    }
    let (client, transport) = create_iq_test_client().await;
    let collector = Arc::new(TestEventCollector::default());
    let _collector = client.subscribe_handler(collector.clone());
    let _retire = client.subscribe_handler(Arc::new(Retire(client.clone())));
    let hook = Arc::new(RecordingHook::new(false));
    hook.release.notify_one();
    assert!(client.group_notification_durability_hook.set(hook).is_ok());
    client.process_node(multi_action_node()).await;
    assert_eq!(group_events(&collector), 1);
    assert!(
        !collector
            .events()
            .iter()
            .any(|event| matches!(&**event, Event::Notification(_)))
    );
    assert!(transport.sent().is_empty());
}

struct RecordingHook {
    nodes: Mutex<Vec<Arc<OwnedNodeRef>>>,
    entered: Notify,
    release: Notify,
    fail: bool,
}

impl RecordingHook {
    /// Hold capture at an explicit barrier, then choose commit success or
    /// failure without depending on socket timing or a real storage backend.
    fn new(fail: bool) -> Self {
        Self {
            nodes: Mutex::new(Vec::new()),
            entered: Notify::new(),
            release: Notify::new(),
            fail,
        }
    }
}

#[async_trait::async_trait]
impl GroupNotificationDurabilityHook for RecordingHook {
    async fn on_notification(&self, node: Arc<OwnedNodeRef>) -> anyhow::Result<()> {
        self.nodes.lock().unwrap().push(node);
        self.entered.notify_one();
        self.release.notified().await;
        if self.fail {
            anyhow::bail!("durable commit rejected");
        }
        Ok(())
    }
}

/// One wire envelope with a sender-key invalidation and a visible rename:
/// capture must retain both before either independent effect becomes visible.
fn multi_action_node() -> Arc<OwnedNodeRef> {
    node_to_owned_ref(
        &NodeBuilder::new("notification")
            .attr("type", "w:gp2")
            .attr("from", GROUP)
            .attr("id", "group-durability-1")
            .attr("t", "1773519041")
            .children([
                NodeBuilder::new("modify")
                    .children([NodeBuilder::new("participant")
                        .attr("jid", "12025550102@s.whatsapp.net")
                        .build()])
                    .build(),
                NodeBuilder::new("subject")
                    .attr("subject", "renamed")
                    .build(),
            ])
            .build(),
    )
}

fn group_events(collector: &TestEventCollector) -> usize {
    collector
        .events()
        .iter()
        .filter(|event| matches!(&***event, Event::GroupUpdate(_)))
        .count()
}

#[tokio::test]
async fn failed_group_commit_withholds_wire_ack_and_preserves_sender_key_state() {
    let (client, transport) = create_iq_test_client().await;
    let collector = Arc::new(TestEventCollector::default());
    client.subscribe_handler(collector.clone()).detach();
    let participant: Jid = "12025550102@s.whatsapp.net".parse().unwrap();
    client
        .set_sender_key_status_for_devices(GROUP, &[participant], true, false)
        .await
        .unwrap();
    let before = client
        .persistence_manager
        .get_sender_key_devices(GROUP)
        .await
        .unwrap();
    assert!(!before.is_empty());
    let hook = Arc::new(RecordingHook::new(true));
    assert!(
        client
            .group_notification_durability_hook
            .set(hook.clone())
            .is_ok()
    );
    let node = multi_action_node();
    let processing = tokio::spawn({
        let client = client.clone();
        async move { client.process_node(node).await }
    });
    tokio::time::timeout(Duration::from_secs(5), hook.entered.notified())
        .await
        .unwrap();
    assert_eq!(group_events(&collector), 0);
    assert!(
        transport.sent().is_empty(),
        "no ACK while commit is pending"
    );
    hook.release.notify_one();
    tokio::time::timeout(Duration::from_secs(5), processing)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(group_events(&collector), 0);
    assert!(
        transport.sent().is_empty(),
        "failed commit must not queue ACK or NACK"
    );
    let after = client
        .persistence_manager
        .get_sender_key_devices(GROUP)
        .await
        .unwrap();
    assert_eq!(
        before.len(),
        after.len(),
        "modify must not clear sender-key tracking"
    );
    assert_eq!(hook.nodes.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn successful_multi_action_commit_is_once_before_events_and_wire_ack() {
    let (client, transport) = create_iq_test_client().await;
    let collector = Arc::new(TestEventCollector::default());
    client.subscribe_handler(collector.clone()).detach();
    let hook = Arc::new(RecordingHook::new(false));
    assert!(
        client
            .group_notification_durability_hook
            .set(hook.clone())
            .is_ok()
    );
    let node = multi_action_node();
    let original = node.clone();
    let processing = tokio::spawn({
        let client = client.clone();
        async move { client.process_node(node).await }
    });
    tokio::time::timeout(Duration::from_secs(5), hook.entered.notified())
        .await
        .unwrap();
    assert_eq!(group_events(&collector), 0);
    assert!(transport.sent().is_empty());
    {
        let recorded = hook.nodes.lock().unwrap();
        assert_eq!(
            recorded.len(),
            1,
            "one hook for the full multi-action envelope"
        );
        assert!(Arc::ptr_eq(&recorded[0], &original));
        assert_eq!(recorded[0].backing_bytes(), original.backing_bytes());
    }
    hook.release.notify_one();
    tokio::time::timeout(Duration::from_secs(5), processing)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(group_events(&collector), 2);
    tokio::time::timeout(Duration::from_secs(5), async {
        while transport.sent().is_empty() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    let plaintexts = crate::test_utils::decrypt_wire_frames(&transport.sent(), &[0; 32]);
    let unpacked = wacore_binary::util::unpack(&plaintexts[0]).unwrap();
    let ack = OwnedNodeRef::new(unpacked.into_owned()).unwrap();
    assert_eq!(ack.tag(), "ack");
    assert_eq!(
        ack.get().attrs().optional_string("id").as_deref(),
        Some("group-durability-1")
    );
    assert_eq!(hook.nodes.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn default_group_processing_dispatches_all_actions_and_keeps_ack_eligible() {
    let (client, _) = create_iq_test_client().await;
    let collector = Arc::new(TestEventCollector::default());
    client.subscribe_handler(collector.clone()).detach();
    let mut cancelled = false;
    assert!(
        NotificationHandler
            .handle(client, multi_action_node(), &mut cancelled)
            .await
    );
    assert!(!cancelled);
    assert_eq!(group_events(&collector), 2);
}

#[tokio::test]
async fn idless_group_notification_reaches_hook_without_synthetic_identity() {
    let (client, _) = create_iq_test_client().await;
    let hook = Arc::new(RecordingHook::new(false));
    hook.release.notify_one();
    assert!(
        client
            .group_notification_durability_hook
            .set(hook.clone())
            .is_ok()
    );
    let node = node_to_owned_ref(
        &NodeBuilder::new("notification")
            .attr("type", "w:gp2")
            .attr("from", GROUP)
            .children([NodeBuilder::new("subject").attr("subject", "same").build()])
            .build(),
    );
    let mut cancelled = false;
    assert!(
        NotificationHandler
            .handle(client, node.clone(), &mut cancelled)
            .await
    );
    assert!(!cancelled);
    let captured = hook.nodes.lock().unwrap();
    assert_eq!(captured.len(), 1);
    assert!(captured[0].get().attrs().optional_string("id").is_none());
    assert!(captured[0].get().attrs().optional_string("t").is_none());
    assert_eq!(captured[0].backing_bytes(), node.backing_bytes());
}

#[tokio::test]
async fn non_group_notification_does_not_invoke_group_hook() {
    let (client, _) = create_iq_test_client().await;
    let hook = Arc::new(RecordingHook::new(true));
    assert!(
        client
            .group_notification_durability_hook
            .set(hook.clone())
            .is_ok()
    );
    let node = node_to_owned_ref(
        &NodeBuilder::new("notification")
            .attr("type", "mediaretry")
            .attr("id", "other")
            .build(),
    );
    let mut cancelled = false;
    tokio::time::timeout(Duration::from_secs(5), async {
        assert!(
            NotificationHandler
                .handle(client, node, &mut cancelled)
                .await
        );
    })
    .await
    .unwrap();
    assert!(!cancelled);
    assert!(hook.nodes.lock().unwrap().is_empty());
}

#[tokio::test]
async fn groups_dirty_failure_is_claimed_and_cancels_ack_before_raw_dispatch() {
    let (client, _) = create_iq_test_client().await;
    let collector = Arc::new(TestEventCollector::default());
    client.subscribe_handler(collector.clone()).detach();
    let hook = Arc::new(RecordingHook::new(true));
    hook.release.notify_one();
    assert!(
        client
            .group_notification_durability_hook
            .set(hook.clone())
            .is_ok()
    );
    let node = node_to_owned_ref(
        &NodeBuilder::new("notification")
            .attr("type", "w:gp2")
            .attr("from", "s.whatsapp.net")
            .children([NodeBuilder::new("groups_dirty").build()])
            .build(),
    );
    let mut cancelled = false;
    assert!(
        NotificationHandler
            .handle(client, node, &mut cancelled)
            .await
    );
    assert!(cancelled);
    assert!(collector.events().is_empty());
    assert_eq!(hook.nodes.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn builder_group_hook_is_independent_and_last_registration_preserves_arc() {
    let hook: Arc<dyn GroupNotificationDurabilityHook> = Arc::new(RecordingHook::new(false));
    let bot = crate::bot::Bot::builder()
        .with_backend_arc(crate::test_utils::create_test_backend().await)
        .with_runtime(crate::TokioRuntime)
        .with_group_notification_durability_hook(RecordingHook::new(true))
        .with_group_notification_durability_hook_arc(hook.clone())
        .build()
        .await
        .unwrap();
    let client = bot.client();
    assert!(client.inbound_durability_hook.get().is_none());
    assert!(client.history_sync_capture_hook.get().is_none());
    assert!(Arc::ptr_eq(
        client.group_notification_durability_hook.get().unwrap(),
        &hook
    ));
}

#[tokio::test]
async fn canceled_pending_capture_has_no_group_effects_or_wire_ack() {
    let (client, transport) = create_iq_test_client().await;
    let collector = Arc::new(TestEventCollector::default());
    client.subscribe_handler(collector.clone()).detach();
    let hook = Arc::new(RecordingHook::new(false));
    assert!(
        client
            .group_notification_durability_hook
            .set(hook.clone())
            .is_ok()
    );
    let task = tokio::spawn({
        let client = client.clone();
        async move { client.process_node(multi_action_node()).await }
    });
    tokio::time::timeout(Duration::from_secs(5), hook.entered.notified())
        .await
        .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(group_events(&collector), 0);
    assert!(transport.sent().is_empty());
}

/// Exercise the claim path through the real dispatcher and encrypted wire
/// transport so an early or missing ACK cannot hide behind a handler mock.
async fn claimed_notification_waits_for_capture(fail: bool) {
    use crate::client::interceptor::Interception;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let (client, transport) = create_iq_test_client().await;
    let collector = Arc::new(TestEventCollector::default());
    client.subscribe_handler(collector.clone()).detach();
    let hook = Arc::new(RecordingHook::new(fail));
    assert!(
        client
            .group_notification_durability_hook
            .set(hook.clone())
            .is_ok()
    );
    let calls = Arc::new(AtomicUsize::new(0));
    let _interceptor = client.add_stanza_interceptor(Arc::new({
        let calls = calls.clone();
        move |_node: &OwnedNodeRef| {
            calls.fetch_add(1, Ordering::SeqCst);
            Interception::Handled
        }
    }));
    let task = tokio::spawn({
        let client = client.clone();
        async move { client.process_node(multi_action_node()).await }
    });
    tokio::time::timeout(Duration::from_secs(5), hook.entered.notified())
        .await
        .unwrap();
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "interceptors wait for durable capture"
    );
    assert!(transport.sent().is_empty());
    hook.release.notify_one();
    tokio::time::timeout(Duration::from_secs(5), task)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(hook.nodes.lock().unwrap().len(), 1);
    assert_eq!(
        group_events(&collector),
        0,
        "a claim skips built-in effects"
    );
    if fail {
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert!(
            transport.sent().is_empty(),
            "a failed capture cannot be ACKed by a claim"
        );
    } else {
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        tokio::time::timeout(Duration::from_secs(5), async {
            while transport.sent().is_empty() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        let plaintexts = crate::test_utils::decrypt_wire_frames(&transport.sent(), &[0; 32]);
        assert_eq!(plaintexts.len(), 1);
        let unpacked = wacore_binary::util::unpack(&plaintexts[0]).unwrap();
        let ack = OwnedNodeRef::new(unpacked.into_owned()).unwrap();
        assert_eq!(ack.tag(), "ack");
        assert_eq!(
            ack.get().attrs().optional_string("id").as_deref(),
            Some("group-durability-1")
        );
    }
}

#[tokio::test]
async fn interceptor_claim_cannot_bypass_failed_group_capture() {
    claimed_notification_waits_for_capture(true).await;
}

#[tokio::test]
async fn interceptor_claim_ack_follows_successful_group_capture() {
    claimed_notification_waits_for_capture(false).await;
}

#[tokio::test]
async fn passing_interceptor_does_not_capture_the_same_group_envelope_twice() {
    use crate::client::interceptor::Interception;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let (client, _) = create_iq_test_client().await;
    let collector = Arc::new(TestEventCollector::default());
    client.subscribe_handler(collector.clone()).detach();
    let hook = Arc::new(RecordingHook::new(false));
    hook.release.notify_one();
    assert!(
        client
            .group_notification_durability_hook
            .set(hook.clone())
            .is_ok()
    );
    let calls = Arc::new(AtomicUsize::new(0));
    let _interceptor = client.add_stanza_interceptor(Arc::new({
        let calls = calls.clone();
        move |_node: &OwnedNodeRef| {
            calls.fetch_add(1, Ordering::SeqCst);
            Interception::Pass
        }
    }));
    tokio::time::timeout(
        Duration::from_secs(5),
        client.process_node(multi_action_node()),
    )
    .await
    .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(hook.nodes.lock().unwrap().len(), 1);
    assert_eq!(group_events(&collector), 2);
}

#[tokio::test]
async fn retirement_during_group_capture_withholds_stale_effects_interceptors_and_ack() {
    use crate::client::interceptor::Interception;
    use std::sync::atomic::{AtomicUsize, Ordering};

    for (claim, shutdown_only) in [(false, false), (true, false), (false, true), (true, true)] {
        let (client, transport) = create_iq_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.subscribe_handler(collector.clone()).detach();
        let participant: Jid = "12025550102@s.whatsapp.net".parse().unwrap();
        client
            .set_sender_key_status_for_devices(GROUP, &[participant], true, false)
            .await
            .unwrap();
        let before = client
            .persistence_manager
            .get_sender_key_devices(GROUP)
            .await
            .unwrap();
        assert!(!before.is_empty());
        let hook = Arc::new(RecordingHook::new(false));
        assert!(
            client
                .group_notification_durability_hook
                .set(hook.clone())
                .is_ok()
        );
        let calls = Arc::new(AtomicUsize::new(0));
        let _interceptor = client.add_stanza_interceptor(Arc::new({
            let calls = calls.clone();
            move |_node: &OwnedNodeRef| {
                calls.fetch_add(1, Ordering::SeqCst);
                if claim {
                    Interception::Handled
                } else {
                    Interception::Pass
                }
            }
        }));
        let task = tokio::spawn({
            let client = client.clone();
            async move { client.process_node(multi_action_node()).await }
        });
        tokio::time::timeout(Duration::from_secs(5), hook.entered.notified())
            .await
            .unwrap();
        if shutdown_only {
            let generation = client.connection_generation.load(Ordering::Acquire);
            client.notify_connection_shutdown();
            assert_eq!(
                client.connection_generation.load(Ordering::Acquire),
                generation
            );
        } else {
            client.connection_generation.fetch_add(1, Ordering::AcqRel);
        }
        hook.release.notify_one();
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            hook.nodes.lock().unwrap().len(),
            1,
            "the durable commit still happened"
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(group_events(&collector), 0);
        assert!(transport.sent().is_empty());
        let after = client
            .persistence_manager
            .get_sender_key_devices(GROUP)
            .await
            .unwrap();
        assert_eq!(
            before.len(),
            after.len(),
            "old modify cannot clear current sender-key state"
        );
    }
}

#[tokio::test]
async fn interceptor_retiring_connection_after_capture_does_not_ack_old_notification() {
    use crate::client::interceptor::Interception;
    use std::sync::atomic::Ordering;

    let (client, transport) = create_iq_test_client().await;
    let hook = Arc::new(RecordingHook::new(false));
    hook.release.notify_one();
    assert!(
        client
            .group_notification_durability_hook
            .set(hook.clone())
            .is_ok()
    );
    let _interceptor = client.add_stanza_interceptor(Arc::new({
        let generation = client.connection_generation.clone();
        move |_node: &OwnedNodeRef| {
            generation.fetch_add(1, Ordering::AcqRel);
            Interception::Handled
        }
    }));
    tokio::time::timeout(
        Duration::from_secs(5),
        client.process_node(multi_action_node()),
    )
    .await
    .unwrap();
    assert_eq!(hook.nodes.lock().unwrap().len(), 1);
    assert!(transport.sent().is_empty());
}

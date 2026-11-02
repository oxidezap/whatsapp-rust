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
            client.advance_generation_for_test(1, Ordering::AcqRel);
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
                    .advance_generation_for_test(1, std::sync::atomic::Ordering::AcqRel);
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
            client.advance_generation_for_test(1, Ordering::AcqRel);
        }
        hook.release.notify_one();
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            hook.nodes.lock().unwrap().len(),
            1,
            "the original envelope reached capture once"
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

#[tokio::test]
async fn retired_queued_notification_keeps_enqueue_time_shutdown_after_reset() {
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
    let generation = client.connection_generation.load(Ordering::Acquire);
    let shutdown = client.connection_shutdown_signal();
    client.notify_connection_shutdown();
    client.reset_connection_shutdown();
    assert_eq!(
        client.connection_generation.load(Ordering::Acquire),
        generation
    );
    client
        .process_queued_group_notification(multi_action_node(), generation, shutdown)
        .await;
    assert!(hook.nodes.lock().unwrap().is_empty());
    assert!(transport.sent().is_empty());
}

#[tokio::test]
async fn metadata_commit_finishes_cache_publication_after_retirement() {
    use crate::store::release::tests::ProbeBackend;
    use crate::test_utils::create_iq_test_client_with_backend;
    use wacore::client::context::GroupRoutingInfo;
    use wacore::libsignal::protocol::{
        KeyPair, SENDERKEY_MESSAGE_CURRENT_VERSION, SenderKeyRecord,
    };
    use wacore::libsignal::store::sender_key_name::SenderKeyName;
    use wacore::types::jid::JidExt;
    use wacore::types::message::AddressingMode;
    for (shutdown_only, fail_response) in
        [(false, false), (true, false), (false, true), (true, true)]
    {
        let (mut probe, _) = ProbeBackend::new();
        let raw = probe.backend.clone();
        let (entered_tx, entered_rx) = async_channel::bounded(1);
        let (release_tx, release_rx) = async_channel::bounded(1);
        probe.gate_method = "after:put_group_metadata";
        probe.fail_after_method = fail_response.then_some("put_group_metadata");
        probe.entered = Some(entered_tx);
        probe.gate = Some(release_rx);
        let (client, transport) = create_iq_test_client_with_backend(Arc::new(probe)).await;
        let collector = Arc::new(TestEventCollector::default());
        client.subscribe_handler(collector.clone()).detach();
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
        let own_lid: Jid = "888000888000888:3@lid".parse().unwrap();
        let own_pn: Jid = "12025550101:3@s.whatsapp.net".parse().unwrap();
        client
            .persistence_manager
            .process_command(crate::store::commands::DeviceCommand::SetLid(Some(
                own_lid.clone(),
            )))
            .await;
        client
            .persistence_manager
            .process_command(crate::store::commands::DeviceCommand::SetId(Some(
                own_pn.clone(),
            )))
            .await;
        client
            .set_sender_key_status_for_devices(GROUP, &[participant], true, false)
            .await
            .unwrap();
        let mut names = Vec::new();
        for own in [own_lid, own_pn] {
            let name = SenderKeyName::from_parts(GROUP, own.to_protocol_address().as_str());
            let mut rng = rand::make_rng::<rand::rngs::StdRng>();
            let key_pair = KeyPair::generate(&mut rng);
            let mut record = SenderKeyRecord::new_empty();
            record
                .add_sender_key_state(
                    SENDERKEY_MESSAGE_CURRENT_VERSION,
                    7,
                    0,
                    &[9; 32],
                    key_pair.public_key,
                    Some(key_pair.private_key),
                )
                .unwrap();
            raw.put_sender_key(name.cache_key(), &record.serialize().unwrap())
                .await
                .unwrap();
            client.signal_cache.put_sender_key(&name, record).await;
            names.push(name);
        }
        let hook = Arc::new(RecordingHook::new(false));
        hook.release.notify_one();
        assert!(client.group_notification_durability_hook.set(hook).is_ok());
        let task = tokio::spawn({
            let client = client.clone();
            async move {
                client
                    .process_node(Arc::new(participant_notification(
                        "remove-commit",
                        "remove",
                    )))
                    .await
            }
        });
        tokio::time::timeout(Duration::from_secs(5), entered_rx.recv())
            .await
            .unwrap()
            .unwrap();
        let stored: GroupRoutingInfo =
            serde_json::from_slice(&raw.get_group_metadata(GROUP).await.unwrap().unwrap()).unwrap();
        assert!(
            stored.participants.is_empty(),
            "storage committed before retirement"
        );
        if shutdown_only {
            client.notify_connection_shutdown();
        } else {
            client.advance_generation_for_test(1, std::sync::atomic::Ordering::AcqRel);
        }
        release_tx.send(()).await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), task)
            .await
            .unwrap()
            .unwrap();
        assert!(
            client
                .get_group_cache()
                .get(&group)
                .await
                .unwrap()
                .participants
                .is_empty(),
            "routing must agree with the completed storage removal"
        );
        for name in names {
            assert!(
                raw.get_sender_key(name.cache_key())
                    .await
                    .unwrap()
                    .is_none(),
                "retired removal must durably delete both own identities"
            );
            assert!(
                client
                    .signal_cache
                    .get_sender_key(&name, &*raw)
                    .await
                    .unwrap()
                    .is_none()
            );
        }
        assert!(
            raw.get_sender_key_devices(GROUP)
                .await
                .unwrap()
                .iter()
                .all(|(_, warm)| !warm)
        );
        assert_eq!(group_events(&collector), 0);
        assert!(transport.sent().is_empty());
    }
}

#[tokio::test]
async fn redelivery_recaptures_original_bytes_and_repeats_non_durable_events() {
    let (client, _) = create_iq_test_client().await;
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
    for _ in 0..2 {
        hook.release.notify_one();
        client.process_node(node.clone()).await;
    }
    let records = hook.nodes.lock().unwrap();
    assert_eq!(records.len(), 2, "host idempotence must handle redelivery");
    assert_eq!(records[0].backing_bytes(), records[1].backing_bytes());
    assert_eq!(
        group_events(&collector),
        4,
        "typed events are not an exactly-once outbox"
    );
}

#[tokio::test]
async fn captured_generation_is_preserved_when_success_retires_it_without_shutdown() {
    use std::sync::atomic::Ordering;
    let (client, transport) = create_iq_test_client().await;
    let collector = Arc::new(TestEventCollector::default());
    client.subscribe_handler(collector.clone()).detach();
    let hook = Arc::new(RecordingHook::new(false));
    hook.release.notify_one();
    assert!(client.group_notification_durability_hook.set(hook).is_ok());
    let shutdown = client.connection_shutdown_signal();
    let node = multi_action_node();
    let captured = NotificationHandler::capture_group(
        &client,
        &node,
        &shutdown,
        client.connection_generation.load(Ordering::Acquire),
    )
    .await
    .unwrap();
    assert!(captured.is_some());
    client.advance_generation_for_test(1, Ordering::AcqRel);
    assert!(
        !shutdown.is_fired(),
        "success can advance generation on the same socket"
    );
    NotificationHandler::handle_scoped(&client, node, &shutdown, captured.unwrap()).await;
    assert_eq!(
        group_events(&collector),
        0,
        "effects must use the capture generation"
    );
    assert!(transport.sent().is_empty());
}

#[tokio::test]
async fn admitted_key_retirement_does_not_wait_for_the_offline_message_permit() {
    use wacore::libsignal::protocol::SenderKeyRecord;
    use wacore::libsignal::store::sender_key_name::SenderKeyName;
    use wacore::types::jid::JidExt;
    let (client, _) = create_iq_test_client().await;
    let own: Jid = "888000888000888:3@lid".parse().unwrap();
    client
        .persistence_manager
        .process_command(crate::store::commands::DeviceCommand::SetLid(Some(
            own.clone(),
        )))
        .await;
    let group: Jid = GROUP.parse().unwrap();
    let name = SenderKeyName::from_parts(GROUP, own.to_protocol_address().as_str());
    let record = SenderKeyRecord::new_empty();
    let backend = client.persistence_manager.backend();
    backend
        .put_sender_key(name.cache_key(), &record.serialize().unwrap())
        .await
        .unwrap();
    client.signal_cache.put_sender_key(&name, record).await;
    client.enter_drain_mode_for_tests();
    let permit = client.acquire_message_processing_permit().await;
    let metadata = client.lock_group_metadata(&group).await;
    tokio::time::timeout(
        Duration::from_secs(2),
        metadata.force_rotate_own_sender_key(),
    )
    .await
    .expect("own-key retirement must not wait for a message holder that can need this group lock")
    .expect("own-key retirement must commit");
    assert!(
        backend
            .get_sender_key(name.cache_key())
            .await
            .unwrap()
            .is_none()
    );
    drop(metadata);
    drop(permit);
}

#[tokio::test]
async fn sender_key_storage_error_withholds_group_events_and_ack() {
    use crate::store::release::tests::ProbeBackend;
    use crate::test_utils::create_iq_test_client_with_backend;
    use wacore::libsignal::protocol::SenderKeyRecord;
    use wacore::libsignal::store::sender_key_name::SenderKeyName;
    use wacore::types::jid::JidExt;
    let (mut probe, _) = ProbeBackend::new();
    let raw = probe.backend.clone();
    probe.fail_after_method = Some("delete_sender_key");
    let (client, transport) = create_iq_test_client_with_backend(Arc::new(probe)).await;
    let collector = Arc::new(TestEventCollector::default());
    client.subscribe_handler(collector.clone()).detach();
    let lid: Jid = "888000888000888:3@lid".parse().unwrap();
    let pn: Jid = "12025550101:3@s.whatsapp.net".parse().unwrap();
    client
        .persistence_manager
        .process_command(crate::store::commands::DeviceCommand::SetLid(Some(
            lid.clone(),
        )))
        .await;
    client
        .persistence_manager
        .process_command(crate::store::commands::DeviceCommand::SetId(Some(
            pn.clone(),
        )))
        .await;
    let mut names = Vec::new();
    for own in [lid, pn] {
        let name = SenderKeyName::from_parts(GROUP, own.to_protocol_address().as_str());
        let record = SenderKeyRecord::new_empty();
        raw.put_sender_key(name.cache_key(), &record.serialize().unwrap())
            .await
            .unwrap();
        client.signal_cache.put_sender_key(&name, record).await;
        names.push(name);
    }
    let hook = Arc::new(RecordingHook::new(false));
    hook.release.notify_one();
    assert!(
        client
            .group_notification_durability_hook
            .set(hook.clone())
            .is_ok()
    );
    client.process_node(multi_action_node()).await;
    assert_eq!(
        hook.nodes.lock().unwrap().len(),
        1,
        "raw capture succeeded before key storage failed"
    );
    for name in names {
        assert!(
            raw.get_sender_key(name.cache_key())
                .await
                .unwrap()
                .is_none(),
            "both deletes must be attempted despite one error"
        );
    }
    assert_eq!(group_events(&collector), 0);
    assert!(
        transport.sent().is_empty(),
        "an indeterminate key retirement must not be ACKed"
    );
}

#[tokio::test]
async fn capture_rejects_enqueue_generation_retired_before_shutdown_fires() {
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
    let generation = client.connection_generation.load(Ordering::Acquire);
    let shutdown = client.connection_shutdown_signal();
    client.advance_generation_for_test(1, Ordering::AcqRel);
    assert!(
        !shutdown.is_fired(),
        "cleanup can advance generation before firing shutdown"
    );
    assert!(
        NotificationHandler::capture_group(&client, &multi_action_node(), &shutdown, generation)
            .await
            .is_err()
    );
    assert!(
        hook.nodes.lock().unwrap().is_empty(),
        "an old queued envelope must not be labeled with the new generation"
    );
    assert!(transport.sent().is_empty());
}

#[tokio::test]
async fn failed_key_delete_preserves_remove_retry_admission() {
    use crate::store::release::tests::ProbeBackend;
    use crate::test_utils::create_iq_test_client_with_backend;
    use std::sync::atomic::{AtomicBool, Ordering};
    use wacore::libsignal::protocol::SenderKeyRecord;
    use wacore::libsignal::store::sender_key_name::SenderKeyName;
    use wacore::types::jid::JidExt;

    let (mut probe, _) = ProbeBackend::new();
    let raw = probe.backend.clone();
    let failing = Arc::new(AtomicBool::new(true));
    probe.fail_before_method = Some(("delete_sender_key", failing.clone()));
    let (client, _) = create_iq_test_client_with_backend(Arc::new(probe)).await;
    let own: Jid = "888000888000888:3@lid".parse().unwrap();
    client
        .persistence_manager
        .process_command(crate::store::commands::DeviceCommand::SetLid(Some(
            own.clone(),
        )))
        .await;
    let name = SenderKeyName::from_parts(GROUP, own.to_protocol_address().as_str());
    let record = SenderKeyRecord::new_empty();
    raw.put_sender_key(name.cache_key(), &record.serialize().unwrap())
        .await
        .unwrap();
    client.signal_cache.put_sender_key(&name, record).await;
    let participant: Jid = "12025550102:0@s.whatsapp.net".parse().unwrap();
    client
        .set_sender_key_status_for_devices(GROUP, &[participant], true, false)
        .await
        .unwrap();
    let group = GROUP.parse().unwrap();
    let guard = client.lock_group_metadata(&group).await;
    assert!(
        guard
            .rotate_sender_key_on_participant_remove(&["12025550102"])
            .await
            .is_err()
    );
    assert!(
        raw.get_sender_key(name.cache_key())
            .await
            .unwrap()
            .is_some(),
        "delete failed before commit"
    );
    assert!(
        client
            .persistence_manager
            .get_sender_key_devices(GROUP)
            .await
            .unwrap()
            .iter()
            .any(|(_, warm)| *warm),
        "failed delete must remain eligible for remove retry"
    );
    failing.store(false, Ordering::Relaxed);
    guard
        .rotate_sender_key_on_participant_remove(&["12025550102"])
        .await
        .unwrap();
    assert!(
        raw.get_sender_key(name.cache_key())
            .await
            .unwrap()
            .is_none(),
        "redelivery must retry persisted deletion despite cached tombstone"
    );
    assert!(
        client
            .persistence_manager
            .get_sender_key_devices(GROUP)
            .await
            .unwrap()
            .iter()
            .all(|(_, warm)| !warm)
    );
}

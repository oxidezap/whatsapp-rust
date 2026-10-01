use super::*;
use crate::test_utils::{TestEventCollector, create_test_client};
use serde_json::{Value, json};
use wacore::iq::mex_operations::fetch_reachout_timelock;
use wacore::types::events::{EventHandler, EventInterest, EventKind};
use wacore_binary::Node;
use wacore_binary::builder::NodeBuilder;

const OP: &str = "NotificationUserReachoutTimelockUpdate";

// Synthetic, sanitized fixtures, not captured traffic or live-server execution.
// WA Web 2.3000.1045368834: WAWebHandleMexNotification takes mexResponse.data;
// WAWebMexReachoutTimelockNotificationHandler reads the notify field and passes
// the same three fields as WAWebGetReachoutTimelockJob's pull update.
fn notification(op: &str, body: &str, bytes: bool) -> Node {
    let update = NodeBuilder::new("update").attr("op_name", op);
    let update = if bytes {
        update.bytes(body.as_bytes().to_vec())
    } else {
        update.string_content(body)
    };
    NodeBuilder::new("notification")
        .attr("type", "mex")
        .attr("from", "s.whatsapp.net")
        .attr("id", "reachout-update")
        .attr("offline", "1704067200")
        .children([update.build()])
        .build()
}

async fn deliver(client: Arc<Client>, node: Node) {
    let mut cancelled = false;
    assert!(
        NotificationHandler
            .handle(
                client,
                crate::test_utils::node_to_owned_ref(&node),
                &mut cancelled
            )
            .await
    );
    assert!(!cancelled);
}

#[tokio::test]
async fn reachout_timelock_raw_and_typed() {
    // Active, lifted and unrecognized/nullable enforcement retain the pull's
    // state, rather than inventing a deadline or mapping strings to an enum.
    for (state, bytes) in [
        (
            json!({"enforcement_type": "BIZ_QUALITY", "is_active": true,
            "time_enforcement_ends": "1704153600"}),
            false,
        ),
        (
            json!({"enforcement_type": "WEB_COMPANION_ONLY", "is_active": false,
            "time_enforcement_ends": null}),
            true,
        ),
        (
            json!({"enforcement_type": "FUTURE_ENFORCEMENT", "is_active": true,
            "time_enforcement_ends": "1704240000", "future_field": {"keep": true}}),
            true,
        ),
        (
            json!({"enforcement_type": null, "is_active": null,
            "time_enforcement_ends": null}),
            false,
        ),
        (json!({}), false),
    ] {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.subscribe_handler(collector.clone()).detach();
        let payload = json!({"data": {"xwa2_notify_account_reachout_timelock": state},
            "extensions": {"preserve": [1, 2, 3]}});
        deliver(client, notification(OP, &payload.to_string(), bytes)).await;
        let events = collector.events();
        assert_eq!(events.len(), 2, "typed update plus unchanged raw twin");
        let Event::ReachoutTimelockUpdate(update) = &*events[0] else {
            panic!("expected typed update before raw twin");
        };
        assert_eq!(events[0].kind(), EventKind::ReachoutTimelockUpdate);
        let pull: fetch_reachout_timelock::Response = serde_json::from_value(json!({
            "xwa2_fetch_account_reachout_timelock": state
        }))
        .unwrap();
        let pull = pull.xwa2_fetch_account_reachout_timelock.unwrap();
        // Same concrete DTO/Deserialize implementation, including optionality.
        let typed: &crate::ReachoutTimelock = &update.state;
        assert_eq!(typed.enforcement_type, pull.enforcement_type);
        assert_eq!(typed.is_active, pull.is_active);
        assert_eq!(typed.time_enforcement_ends, pull.time_enforcement_ends);
        let Event::MexNotification(raw) = &*events[1] else {
            panic!("expected raw twin");
        };
        assert_eq!(raw.op_name, OP);
        assert_eq!(raw.payload, payload);
        assert_eq!(raw.from.as_ref().unwrap().to_string(), "s.whatsapp.net");
        assert_eq!(raw.stanza_id.as_deref(), Some("reachout-update"));
        assert_eq!(raw.offline.as_deref(), Some("1704067200"));
        assert_eq!(update.from, raw.from);
        assert_eq!(update.stanza_id, raw.stanza_id);
        assert_eq!(update.offline, raw.offline);
    }
}

#[tokio::test]
async fn reachout_timelock_invalid_state_and_unrelated_ops_stay_raw() {
    let bodies = [
        json!({}),
        json!({"data": null}),
        json!({"data": {}}),
        json!({"data": {"xwa2_notify_account_reachout_timelock": null}}),
        json!({"data": {"xwa2_notify_account_reachout_timelock": []}}),
        json!({"data": {"xwa2_notify_account_reachout_timelock": [null, true, "1704153600"]}}),
        json!({"data": {"xwa2_notify_account_reachout_timelock": "invalid"}}),
        json!({"data": {"xwa2_notify_account_reachout_timelock": {"is_active": "false"}}}),
        json!({"data": {"xwa2_notify_account_reachout_timelock": {"enforcement_type": 42}}}),
        json!({"data": {"xwa2_notify_account_reachout_timelock": {"time_enforcement_ends": 42}}}),
        json!({"data": {"xwa2_fetch_account_reachout_timelock": {"is_active": true}}}),
        Value::Null,
    ];
    for payload in bodies {
        assert_raw_only(OP, payload).await;
    }
    let payload = json!({"data": {
        "xwa2_notify_account_reachout_timelock": {"is_active": true}
    }});
    assert_raw_only("UnrelatedNotification", payload.clone()).await;
    assert_raw_only("NotificationUserReachoutTimelockUpdateExtra", payload).await;
}

async fn assert_raw_only(op: &str, payload: Value) {
    let client = create_test_client().await;
    let collector = Arc::new(TestEventCollector::default());
    client.subscribe_handler(collector.clone()).detach();
    deliver(client, notification(op, &payload.to_string(), false)).await;
    let events = collector.events();
    assert_eq!(events.len(), 1);
    let Event::MexNotification(raw) = &*events[0] else {
        panic!("invalid state or unrelated op must not invent a typed event");
    };
    assert_eq!(raw.op_name, op);
    assert_eq!(raw.payload, payload);
}

#[tokio::test]
async fn reachout_timelock_malformed_envelope_emits_nothing() {
    for bytes in [false, true] {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.subscribe_handler(collector.clone()).detach();
        deliver(client, notification(OP, "{invalid", bytes)).await;
        assert!(collector.events().is_empty());
    }
    for child in [
        None,
        Some(NodeBuilder::new("update").build()),
        Some(NodeBuilder::new("update").attr("op_name", OP).build()),
    ] {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.subscribe_handler(collector.clone()).detach();
        let node = NodeBuilder::new("notification")
            .attr("type", "mex")
            .children(child)
            .build();
        deliver(client, node).await;
        assert!(collector.events().is_empty());
    }
}

struct BeforeAckCollector {
    transport: Arc<crate::transport::mock::CapturingMockTransport>,
    inner: TestEventCollector,
}

impl EventHandler for BeforeAckCollector {
    fn handle_event(&self, event: Arc<Event>) {
        assert_eq!(
            self.transport.sent_count(),
            0,
            "events precede transport ACK"
        );
        self.inner.handle_event(event);
    }
}

#[tokio::test]
async fn reachout_timelock_dispatch_preserves_deferred_ack() {
    for (op, body, expected_events) in [
        (
            OP,
            r#"{"data":{"xwa2_notify_account_reachout_timelock":{"is_active":true}}}"#,
            2,
        ),
        ("UnrelatedNotification", "{}", 1),
        (OP, "{invalid", 0),
    ] {
        let (client, transport) = crate::test_utils::create_iq_test_client().await;
        let collector = Arc::new(BeforeAckCollector {
            transport: transport.clone(),
            inner: TestEventCollector::default(),
        });
        client.subscribe_handler(collector.clone()).detach();
        let node = notification(op, body, false);
        client
            .process_node(crate::test_utils::node_to_owned_ref(&node))
            .await;
        assert_eq!(collector.inner.events().len(), expected_events);
        let ack = crate::test_utils::decode_sent_iq(&transport, 0).await;
        let ack = ack.get();
        assert_eq!(ack.tag, "ack");
        assert_eq!(
            ack.attrs().optional_string("class").as_deref(),
            Some("notification")
        );
        assert_eq!(ack.attrs().optional_string("type").as_deref(), Some("mex"));
        assert_eq!(
            ack.attrs().optional_string("id").as_deref(),
            Some("reachout-update")
        );
        assert_eq!(
            ack.attrs().optional_jid("to").unwrap().to_string(),
            "s.whatsapp.net"
        );
    }
}

struct InterestedCollector {
    kinds: EventInterest,
    inner: TestEventCollector,
}

impl EventHandler for InterestedCollector {
    fn interest(&self) -> EventInterest {
        self.kinds
    }
    fn handle_event(&self, event: Arc<Event>) {
        self.inner.handle_event(event);
    }
}

#[tokio::test]
async fn reachout_timelock_typed_and_raw_interests_are_independent() {
    for kind in [
        EventKind::ReachoutTimelockUpdate,
        EventKind::MexNotification,
    ] {
        let client = create_test_client().await;
        let collector = Arc::new(InterestedCollector {
            kinds: EventInterest::of(&[kind]),
            inner: TestEventCollector::default(),
        });
        client.subscribe_handler(collector.clone()).detach();
        // Missing source attributes remain None, not empty-string sentinels.
        let node = NodeBuilder::new("notification")
            .attr("type", "mex")
            .children([NodeBuilder::new("update")
                .attr("op_name", OP)
                .string_content(
                    r#"{"data":{"xwa2_notify_account_reachout_timelock":{"is_active":false}}}"#,
                )
                .build()])
            .build();
        deliver(client, node).await;
        let events = collector.inner.events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind(), kind);
        match &*events[0] {
            Event::ReachoutTimelockUpdate(update) => {
                assert_eq!(update.state.is_active, Some(false));
                assert!(
                    update.from.is_none() && update.stanza_id.is_none() && update.offline.is_none()
                );
            }
            Event::MexNotification(raw) => {
                assert!(raw.from.is_none() && raw.stanza_id.is_none() && raw.offline.is_none());
            }
            _ => panic!("unexpected event"),
        }
    }
}

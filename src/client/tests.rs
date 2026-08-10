//! Client integration and unit tests.

use super::*;
use crate::lid_pn_cache::LearningSource;
use crate::test_utils::MockHttpClient;
use futures::channel::oneshot;
use wacore_binary::SERVER_JID;

#[tokio::test]
async fn test_ack_behavior_for_incoming_stanzas() {
    let backend = crate::test_utils::create_test_backend().await;
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("persistence manager should initialize"),
    );
    let (client, _rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        Arc::new(crate::transport::mock::MockTransportFactory::new()),
        Arc::new(MockHttpClient),
        None,
    )
    .await;

    // --- Assertions ---

    // Verify that we still ack other critical stanzas (regression check).
    use wacore_binary::{Attrs, Node, NodeContent};

    let mut receipt_attrs = Attrs::new();
    receipt_attrs.insert("from".to_string(), "@s.whatsapp.net".to_string());
    receipt_attrs.insert("id".to_string(), "RCPT-1".to_string());
    let receipt_node = Node::new(
        "receipt",
        receipt_attrs,
        Some(NodeContent::String("test".into())),
    );

    let mut notification_attrs = Attrs::new();
    notification_attrs.insert("from".to_string(), "@s.whatsapp.net".to_string());
    notification_attrs.insert("id".to_string(), "NOTIF-1".to_string());
    let notification_node = Node::new(
        "notification",
        notification_attrs,
        Some(NodeContent::String("test".into())),
    );

    assert!(
        client.should_ack(&receipt_node.as_node_ref()),
        "should_ack must still return TRUE for <receipt> stanzas."
    );
    assert!(
        client.should_ack(&notification_node.as_node_ref()),
        "should_ack must still return TRUE for <notification> stanzas."
    );

    // Regular <message> stanzas (DM / group) are acked via the delivery
    // <receipt>, not a bare <ack class="message">. WA Web only emits
    // <ack class="message"> for newsletter deliveries.
    let mut dm_attrs = Attrs::new();
    dm_attrs.insert(
        "from".to_string(),
        "5511999999999@s.whatsapp.net".to_string(),
    );
    dm_attrs.insert("id".to_string(), "MSG-DM-1".to_string());
    let dm_message = Node::new("message", dm_attrs, None);
    assert!(
        !client.should_ack(&dm_message.as_node_ref()),
        "should_ack must return FALSE for regular DM <message> (delivery receipt covers it)."
    );

    let mut group_attrs = Attrs::new();
    group_attrs.insert("from".to_string(), "120363098765432100@g.us".to_string());
    group_attrs.insert("id".to_string(), "MSG-GROUP-1".to_string());
    let group_message = Node::new("message", group_attrs, None);
    assert!(
        !client.should_ack(&group_message.as_node_ref()),
        "should_ack must return FALSE for group <message>."
    );

    let mut newsletter_attrs = Attrs::new();
    newsletter_attrs.insert(
        "from".to_string(),
        "120363298765432100@newsletter".to_string(),
    );
    newsletter_attrs.insert("id".to_string(), "MSG-NL-1".to_string());
    let newsletter_message = Node::new("message", newsletter_attrs, None);
    assert!(
        client.should_ack(&newsletter_message.as_node_ref()),
        "should_ack must return TRUE for newsletter <message>."
    );

    // status@broadcast gets the transport <ack> as a fallback so that
    // drop paths in process_group_enc_batch (expired status, missing
    // sender key, decrypt error) don't leave the server retransmitting.
    // The success path also emits <receipt context="status">; the
    // duplicate is tolerated.
    let mut status_attrs = Attrs::new();
    status_attrs.insert("from".to_string(), "status@broadcast".to_string());
    status_attrs.insert("id".to_string(), "MSG-STATUS-1".to_string());
    let status_message = Node::new("message", status_attrs, None);
    assert!(
        client.should_ack(&status_message.as_node_ref()),
        "should_ack must return TRUE for status@broadcast <message> (fallback for drop paths)."
    );

    // A status update delivered as a top-level <status> stanza is owed the same
    // transport ack; the server recycles the stream until it arrives.
    let mut status_stanza_attrs = Attrs::new();
    status_stanza_attrs.insert("from".to_string(), "status@broadcast".to_string());
    status_stanza_attrs.insert("id".to_string(), "STATUS-STANZA-1".to_string());
    status_stanza_attrs.insert("participant".to_string(), "200725430796339@lid".to_string());
    status_stanza_attrs.insert("type".to_string(), "media".to_string());
    let status_stanza = Node::new("status", status_stanza_attrs, None);
    assert!(
        client.should_ack(&status_stanza.as_node_ref()),
        "should_ack must return TRUE for a top-level <status> stanza."
    );

    info!(
        "✅ test_ack_behavior_for_incoming_stanzas passed: Client correctly differentiates which stanzas to acknowledge."
    );
}

#[tokio::test]
async fn test_ack_waiter_resolves() {
    let backend = crate::test_utils::create_test_backend().await;
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("persistence manager should initialize"),
    );
    let (client, _rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        Arc::new(crate::transport::mock::MockTransportFactory::new()),
        Arc::new(MockHttpClient),
        None,
    )
    .await;

    // 1. Insert a waiter for a specific ID
    let test_id = "ack-test-123".to_string();
    let (tx, rx) = oneshot::channel();
    client
        .response_waiters_guard()
        .insert(test_id.clone(), ResponseWaiter::Iq(tx));
    assert!(
        client.response_waiters_guard().contains_key(&test_id),
        "Waiter should be inserted before handling ack"
    );

    // 2. Create a mock <ack/> node with the test ID
    let ack_node = NodeBuilder::new("ack")
        .attr("id", test_id.clone())
        .attr("from", SERVER_JID)
        .build();

    // 3. Handle the ack
    let handled = client.handle_ack_response_arc(&Arc::new(to_owned_node(&ack_node)));
    assert!(
        handled,
        "handle_ack_response should return true when waiter exists"
    );

    // 4. Await the receiver with a timeout
    match tokio::time::timeout(Duration::from_secs(1), rx).await {
        Ok(Ok(response_node)) => {
            assert!(
                response_node
                    .get()
                    .get_attr("id")
                    .is_some_and(|v| v.as_str() == test_id.as_str()),
                "Response node should have correct ID"
            );
        }
        Ok(Err(_)) => panic!("Receiver was dropped without being sent a value"),
        Err(_) => panic!("Test timed out waiting for ack response"),
    }

    // 5. Verify the waiter was removed
    assert!(
        !client.response_waiters_guard().contains_key(&test_id),
        "Waiter should be removed after handling"
    );

    info!("✅ test_ack_waiter_resolves passed: ACK response correctly resolves pending waiters");
}

#[tokio::test]
async fn test_ack_without_matching_waiter() {
    let backend = crate::test_utils::create_test_backend().await;
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("persistence manager should initialize"),
    );
    let (client, _rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        Arc::new(crate::transport::mock::MockTransportFactory::new()),
        Arc::new(MockHttpClient),
        None,
    )
    .await;

    // Create an ack without any matching waiter
    let ack_node = NodeBuilder::new("ack")
        .attr("id", "non-existent-id")
        .attr("from", SERVER_JID)
        .build();

    // Should return false since there's no waiter
    let handled = client.handle_ack_response_arc(&Arc::new(to_owned_node(&ack_node)));
    assert!(
        !handled,
        "handle_ack_response should return false when no waiter exists"
    );

    info!(
        "✅ test_ack_without_matching_waiter passed: ACK without matching waiter handled gracefully"
    );
}

/// Round-trip a built `Node` into the shape the receive path holds: node bytes,
/// as `unpack` hands them over once the format byte is off.
fn to_owned_node(node: &Node) -> OwnedNodeRef {
    let marshaled = wacore_binary::marshal::marshal_ref(&node.as_node_ref()).expect("valid node");
    let node_bytes = wacore_binary::util::unpack(&marshaled).expect("packed payload");
    OwnedNodeRef::new(node_bytes.into_owned()).expect("valid node")
}

fn owned_ack_node(id: &str) -> OwnedNodeRef {
    to_owned_node(
        &NodeBuilder::new("ack")
            .attr("id", id)
            .attr("from", SERVER_JID)
            .build(),
    )
}

/// The Arc entry point must hand the waiter the SAME allocation it was given
/// (no re-encode + re-parse round trip).
#[tokio::test]
async fn ack_arc_delivery_shares_allocation() {
    let client = crate::test_utils::create_test_client().await;

    let test_id = "ack-arc-456";
    let (tx, rx) = oneshot::channel();
    client
        .response_waiters_guard()
        .insert(test_id.to_string(), ResponseWaiter::Iq(tx));

    let node = Arc::new(owned_ack_node(test_id));
    assert!(client.handle_ack_response_arc(&node));

    let received = tokio::time::timeout(Duration::from_secs(1), rx)
        .await
        .expect("waiter should resolve")
        .expect("sender must not drop");
    assert!(
        Arc::ptr_eq(&received, &node),
        "waiter must receive the original allocation, not a re-encoded copy"
    );

    // No waiter: must report unhandled without consuming anything.
    assert!(!client.handle_ack_response_arc(&Arc::new(owned_ack_node("ack-arc-none"))));
}

/// The owned entry point (read-loop fast path) resolves the waiter from the
/// node it already owns.
#[tokio::test]
async fn ack_owned_delivery_resolves_waiter() {
    let client = crate::test_utils::create_test_client().await;

    let test_id = "ack-owned-789";
    let (tx, rx) = oneshot::channel();
    client
        .response_waiters_guard()
        .insert(test_id.to_string(), ResponseWaiter::Iq(tx));

    assert!(client.handle_ack_response_owned(owned_ack_node(test_id)));
    let received = tokio::time::timeout(Duration::from_secs(1), rx)
        .await
        .expect("waiter should resolve")
        .expect("sender must not drop");
    assert!(
        received
            .get()
            .get_attr("id")
            .is_some_and(|v| v.as_str() == test_id),
        "delivered node must carry the ack id"
    );

    assert!(!client.handle_ack_response_owned(owned_ack_node("ack-owned-none")));
}

/// Every server `<ack>` with an id dispatches an observe-only
/// `Event::ServerAck` carrying the ack's class/from/t, independent of
/// waiter state; a nack carries its error code. Lets consumers measure
/// send → server-accept latency and see nack codes programmatically
/// instead of scraping warn! logs.
#[tokio::test]
async fn test_ack_dispatches_server_ack_event() {
    use wacore::types::events::{Event, EventHandler};

    let client = crate::test_utils::create_test_client().await;
    let collector = Arc::new(crate::test_utils::TestEventCollector::default());
    client
        .subscribe_handler(collector.clone() as Arc<dyn EventHandler>)
        .detach();

    // Plain message ack (no waiter registered): event fires with the ack's
    // class, from and server timestamp; error is None.
    let ack_node = NodeBuilder::new("ack")
        .attr("id", "ack-evt-1")
        .attr("class", "message")
        .attr("from", "123456789@s.whatsapp.net")
        .attr("t", "1720000000")
        .build();
    client.handle_ack_response_arc(&Arc::new(to_owned_node(&ack_node)));
    assert!(
        collector.events().iter().any(|e| matches!(
            e.as_ref(),
            Event::ServerAck(ack)
                if ack.id == "ack-evt-1"
                    && ack.class.as_deref() == Some("message")
                    && ack.from.as_ref().is_some_and(|j| j.to_string() == "123456789@s.whatsapp.net")
                    && ack.timestamp.is_some_and(|t| t.timestamp() == 1_720_000_000)
                    && ack.error.is_none()
        )),
        "server <ack> should dispatch Event::ServerAck with class/from/t"
    );

    // Nack: the error code rides along; absent class/t stay None.
    let nack_node = NodeBuilder::new("ack")
        .attr("id", "ack-evt-2")
        .attr("error", "479")
        .attr("from", SERVER_JID)
        .build();
    client.handle_ack_response_arc(&Arc::new(to_owned_node(&nack_node)));
    assert!(
        collector.events().iter().any(|e| matches!(
            e.as_ref(),
            Event::ServerAck(ack)
                if ack.id == "ack-evt-2"
                    && ack.class.is_none()
                    && ack.timestamp.is_none()
                    && ack.error.as_deref() == Some("479")
        )),
        "server nack should dispatch Event::ServerAck carrying the error code"
    );

    // An ack without an id (e.g. non-message acks) dispatches nothing.
    let anon_ack = NodeBuilder::new("ack").attr("from", SERVER_JID).build();
    client.handle_ack_response_arc(&Arc::new(to_owned_node(&anon_ack)));
    assert_eq!(
        collector
            .events()
            .iter()
            .filter(|e| matches!(e.as_ref(), Event::ServerAck(_)))
            .count(),
        2,
        "an <ack> without an id must not dispatch Event::ServerAck"
    );

    // The headline guarantee: with a waiter registered for the same id, the
    // event STILL fires and the waiter STILL resolves — dispatch and waiter
    // resolution are independent.
    let (tx, rx) = oneshot::channel();
    client
        .response_waiters_guard()
        .insert("ack-evt-3".to_string(), ResponseWaiter::Iq(tx));
    let waited_ack = NodeBuilder::new("ack")
        .attr("id", "ack-evt-3")
        .attr("class", "message")
        .attr("from", SERVER_JID)
        .build();
    let handled = client.handle_ack_response_arc(&Arc::new(to_owned_node(&waited_ack)));
    assert!(handled, "waiter for the id should have been resolved");
    let resolved = tokio::time::timeout(Duration::from_secs(1), rx)
        .await
        .expect("timed out waiting for ack waiter")
        .expect("waiter sender was dropped");
    assert!(
        resolved
            .get()
            .get_attr("id")
            .is_some_and(|v| v.as_str() == "ack-evt-3"),
        "waiter should receive the ack node"
    );
    assert!(
        collector.events().iter().any(|e| matches!(
            e.as_ref(),
            Event::ServerAck(ack) if ack.id == "ack-evt-3"
        )),
        "Event::ServerAck should fire even when a waiter consumes the ack"
    );
}

/// `from` arrives as a JID token, so reading it must take the JID the decoder
/// already built rather than rendering and re-parsing it. Two things ride on
/// that: the render is pure churn on every acked message, and it silently drops
/// the interop `integrator`, which the wire does carry.
#[tokio::test]
async fn server_ack_from_preserves_the_wire_jid() {
    use wacore::types::events::{Event, EventHandler};

    let client = crate::test_utils::create_test_client().await;
    let collector = Arc::new(crate::test_utils::TestEventCollector::default());
    client
        .subscribe_handler(collector.clone() as Arc<dyn EventHandler>)
        .detach();

    // An addressed device JID: same value the render-and-reparse produced.
    let device: Jid = "551199990001:3@s.whatsapp.net".parse().expect("device jid");
    let ack = NodeBuilder::new("ack")
        .attr("id", "ack-from-ad")
        .attr("class", "message")
        .attr("from", device.clone())
        .build();
    client.handle_ack_response_arc(&Arc::new(to_owned_node(&ack)));
    assert!(
        collector.events().iter().any(|e| matches!(
            e.as_ref(),
            Event::ServerAck(a) if a.id == "ack-from-ad" && a.from.as_ref() == Some(&device)
        )),
        "an addressed `from` must reach the event unchanged"
    );

    // A `from` that reached the node as a plain string still parses, so the
    // switch does not narrow what is accepted.
    let string_ack = NodeBuilder::new("ack")
        .attr("id", "ack-from-str")
        .attr("from", "not-a-phone-user@g.us")
        .build();
    client.handle_ack_response_arc(&Arc::new(to_owned_node(&string_ack)));
    assert!(
        collector.events().iter().any(|e| matches!(
            e.as_ref(),
            Event::ServerAck(a)
                if a.id == "ack-from-str"
                    && a.from.as_ref().is_some_and(|j| j.to_string() == "not-a-phone-user@g.us")
        )),
        "a string-form `from` must still parse into the event"
    );
}

/// The allocation guard for the same call. `from` reaching the event is not by
/// itself evidence the display form is gone, since both spellings produce the
/// same JID for everything our own encoder can emit; the count is.
///
/// Baseline three: the event's owned `id` and `class` strings, and the `Arc`
/// `dispatch` wraps the event in. A user too long to sit inline in its
/// `CompactString` adds exactly one for that copy, and no more: it is owning the
/// JID that costs, not rendering it. Re-introducing the render moves both.
#[tokio::test]
async fn server_ack_event_build_does_not_render_the_from_jid() {
    use wacore::types::events::EventHandler;

    let client = crate::test_utils::create_test_client().await;
    let collector = Arc::new(crate::test_utils::TestEventCollector::default());
    client
        .subscribe_handler(collector.clone() as Arc<dyn EventHandler>)
        .detach();

    // Both arrive as JID tokens, which is how `from` comes off the wire: that is
    // the case where rendering it costs a String the parse then throws away. The
    // long one is a legacy group id, whose `<creator>-<timestamp>` user is the
    // real shape that outgrows the inline buffer.
    let ack_with_from = |from: Jid| {
        let node = Arc::new(to_owned_node(
            &NodeBuilder::new("ack")
                .attr("class", "message")
                .attr("from", from)
                .attr("id", "3EB0A1B2C3D4E5F60718")
                .attr("t", "1758000000")
                .build(),
        ));
        assert!(
            node.get()
                .get_attr("from")
                .is_some_and(|v| v.as_jid().is_some()),
            "the fixture must reach the handler as a JID token, not a string"
        );
        node
    };

    let inline_user = ack_with_from("551199990001@s.whatsapp.net".parse().expect("jid"));
    let heap_user = ack_with_from("5511999988887-1600000000123456@g.us".parse().expect("jid"));

    let inline_delta = crate::test_alloc::min_allocs(3, || {
        assert!(!client.handle_ack_response_arc(&inline_user));
    });
    assert_eq!(
        inline_delta, 3,
        "the ServerAck build must allocate only its two owned strings and the event Arc"
    );

    let heap_delta = crate::test_alloc::min_allocs(4, || {
        assert!(!client.handle_ack_response_arc(&heap_user));
    });
    assert_eq!(
        heap_delta,
        inline_delta + 1,
        "a heap-backed JID user must cost its own copy and nothing else"
    );
}

/// The other half of the reason `from` stopped going through the display form:
/// an interop JID's `integrator` is carried on the wire but is not part of the
/// rendered JID, so rendering and re-parsing silently dropped it.
#[test]
fn jid_attr_display_round_trip_drops_the_interop_integrator() {
    use wacore_binary::node::{NodeStr, ValueRef};
    use wacore_binary::{JidRef, Server};

    let value = ValueRef::Jid(JidRef {
        user: NodeStr::Borrowed("551199990002"),
        server: Server::Interop,
        agent: 0,
        device: 1,
        integrator: 7,
    });

    let via_display: Option<Jid> = value.as_str().parse().ok();
    assert_eq!(
        via_display.map(|j| j.integrator),
        Some(0),
        "the display form carries no integrator, which is what the old path lost"
    );
    assert_eq!(
        value.to_jid().map(|j| j.integrator),
        Some(7),
        "to_jid takes the decoded JID as-is"
    );
}

/// Failure shape: a `from` that is not a JID still yields `None` rather than a
/// partially-parsed value, and the ack is otherwise handled as before.
#[tokio::test]
async fn server_ack_from_stays_none_for_a_malformed_jid() {
    use wacore::types::events::{Event, EventHandler};

    let client = crate::test_utils::create_test_client().await;
    let collector = Arc::new(crate::test_utils::TestEventCollector::default());
    client
        .subscribe_handler(collector.clone() as Arc<dyn EventHandler>)
        .detach();

    let ack = NodeBuilder::new("ack")
        .attr("id", "ack-from-bad")
        .attr("class", "message")
        .attr("from", "not a jid at all")
        .build();
    client.handle_ack_response_arc(&Arc::new(to_owned_node(&ack)));
    assert!(
        collector.events().iter().any(|e| matches!(
            e.as_ref(),
            Event::ServerAck(a)
                if a.id == "ack-from-bad"
                    && a.from.is_none()
                    && a.class.as_deref() == Some("message")
        )),
        "an unparseable `from` must land as None without disturbing the rest"
    );
}

/// Test that the lid_pn_cache correctly stores and retrieves LID mappings.
///
/// This is critical for the LID-PN session mismatch fix. When we receive a message
/// with sender_lid, we cache the phone->LID mapping so that when sending replies,
/// we can reuse the existing LID session instead of creating a new PN session.
#[tokio::test]
async fn test_lid_pn_cache_basic_operations() {
    let backend = Arc::new(
        crate::store::SqliteStore::new("file:memdb_lid_cache_basic?mode=memory&cache=shared")
            .await
            .expect("Failed to create in-memory backend for test"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("persistence manager should initialize"),
    );
    let (client, _rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        Arc::new(crate::transport::mock::MockTransportFactory::new()),
        Arc::new(MockHttpClient),
        None,
    )
    .await;

    // Initially, the cache should be empty for a phone number
    let phone = "559980000001";
    let lid = "100000012345678";

    assert!(
        client.lid_pn_cache.get_current_lid(phone).await.is_none(),
        "Cache should be empty initially"
    );

    // Insert a phone->LID mapping using add_lid_pn_mapping
    client
        .add_lid_pn_mapping(lid, phone, LearningSource::Usync)
        .await
        .expect("Failed to persist LID-PN mapping in tests");

    // Verify we can retrieve it (phone -> LID lookup)
    let cached_lid = client.lid_pn_cache.get_current_lid(phone).await;
    assert!(cached_lid.is_some(), "Cache should contain the mapping");
    assert_eq!(
        cached_lid.expect("cache should have LID"),
        lid,
        "Cached LID should match what we inserted"
    );

    // Verify reverse lookup works (LID -> phone)
    let cached_phone = client.lid_pn_cache.get_phone_number(lid).await;
    assert!(cached_phone.is_some(), "Reverse lookup should work");
    assert_eq!(
        cached_phone.expect("reverse lookup should return phone"),
        phone,
        "Cached phone should match what we inserted"
    );

    // Verify a different phone number returns None
    assert!(
        client
            .lid_pn_cache
            .get_current_lid("559980000002")
            .await
            .is_none(),
        "Different phone number should not have a mapping"
    );

    info!("✅ test_lid_pn_cache_basic_operations passed: LID-PN cache works correctly");
}

/// Test that the lid_pn_cache respects timestamp-based conflict resolution.
///
/// When a phone number has multiple LIDs, the most recent one should be returned.
#[tokio::test]
async fn test_lid_pn_cache_timestamp_resolution() {
    let backend = Arc::new(
        crate::store::SqliteStore::new("file:memdb_lid_cache_timestamp?mode=memory&cache=shared")
            .await
            .expect("Failed to create in-memory backend for test"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("persistence manager should initialize"),
    );
    let (client, _rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        Arc::new(crate::transport::mock::MockTransportFactory::new()),
        Arc::new(MockHttpClient),
        None,
    )
    .await;

    let phone = "559980000001";
    let lid_old = "100000012345678";
    let lid_new = "100000087654321";

    // Insert initial mapping
    client
        .add_lid_pn_mapping(lid_old, phone, LearningSource::Usync)
        .await
        .expect("Failed to persist LID-PN mapping in tests");

    assert_eq!(
        client
            .lid_pn_cache
            .get_current_lid(phone)
            .await
            .expect("cache should have LID"),
        lid_old,
        "Initial LID should be stored"
    );

    // Small delay to ensure different timestamp
    tokio::time::sleep(Duration::from_millis(10)).await;

    // Add new mapping with newer timestamp
    client
        .add_lid_pn_mapping(lid_new, phone, LearningSource::PeerPnMessage)
        .await
        .expect("Failed to persist LID-PN mapping in tests");

    assert_eq!(
        client
            .lid_pn_cache
            .get_current_lid(phone)
            .await
            .expect("cache should have newer LID"),
        lid_new,
        "Newer LID should be returned for phone lookup"
    );

    // Both LIDs should still resolve to the same phone
    assert_eq!(
        client
            .lid_pn_cache
            .get_phone_number(lid_old)
            .await
            .expect("reverse lookup should return phone"),
        phone,
        "Old LID should still map to phone"
    );
    assert_eq!(
        client
            .lid_pn_cache
            .get_phone_number(lid_new)
            .await
            .expect("reverse lookup should return phone"),
        phone,
        "New LID should also map to phone"
    );

    info!(
        "✅ test_lid_pn_cache_timestamp_resolution passed: Timestamp-based resolution works correctly"
    );
}

/// Test that get_lid_for_phone (from SendContextResolver) returns the cached value.
///
/// This is the method used by wacore::send to look up LID mappings when encrypting.
#[tokio::test]
async fn test_get_lid_for_phone_via_send_context_resolver() {
    use wacore::client::context::SendContextResolver;

    let backend = Arc::new(
        crate::store::SqliteStore::new("file:memdb_get_lid_for_phone?mode=memory&cache=shared")
            .await
            .expect("Failed to create in-memory backend for test"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("persistence manager should initialize"),
    );
    let (client, _rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        Arc::new(crate::transport::mock::MockTransportFactory::new()),
        Arc::new(MockHttpClient),
        None,
    )
    .await;

    let phone = "559980000001";
    let lid = "100000012345678";

    // Before caching, should return None
    assert!(
        client.get_lid_for_phone(phone).await.is_none(),
        "get_lid_for_phone should return None before caching"
    );

    // Cache the mapping using add_lid_pn_mapping
    client
        .add_lid_pn_mapping(lid, phone, LearningSource::Usync)
        .await
        .expect("Failed to persist LID-PN mapping in tests");

    // Now it should return the LID
    let result = client.get_lid_for_phone(phone).await;
    assert!(
        result.is_some(),
        "get_lid_for_phone should return Some after caching"
    );
    assert_eq!(
        result.expect("get_lid_for_phone should return Some"),
        lid,
        "get_lid_for_phone should return the cached LID"
    );

    info!(
        "✅ test_get_lid_for_phone_via_send_context_resolver passed: SendContextResolver correctly returns cached LID"
    );
}

/// Test that wait_for_offline_delivery_end returns immediately when the flag is already set.
#[tokio::test]
async fn test_wait_for_offline_delivery_end_returns_immediately_when_flag_set() {
    let backend = Arc::new(
        crate::store::SqliteStore::new("file:memdb_offline_sync_flag_set?mode=memory&cache=shared")
            .await
            .expect("Failed to create in-memory backend for test"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("persistence manager should initialize"),
    );
    let (client, _rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        Arc::new(crate::transport::mock::MockTransportFactory::new()),
        Arc::new(MockHttpClient),
        None,
    )
    .await;

    // Set the flag to true (simulating offline sync completed)
    client.offline_sync_completed.store(true, Ordering::Relaxed);

    // This should return immediately (not wait 10 seconds)
    let start = wacore::time::Instant::now();
    client.wait_for_offline_delivery_end().await;
    let elapsed = start.elapsed();

    // Should complete in < 100ms (not 10 second timeout)
    assert!(
        elapsed.as_millis() < 100,
        "wait_for_offline_delivery_end should return immediately when flag is set, took {:?}",
        elapsed
    );

    info!("✅ test_wait_for_offline_delivery_end_returns_immediately_when_flag_set passed");
}

/// Test that wait_for_offline_delivery_end times out when the flag is NOT set.
/// This verifies the 10-second timeout is working.
#[tokio::test]
async fn test_wait_for_offline_delivery_end_times_out_when_flag_not_set() {
    let backend = Arc::new(
        crate::store::SqliteStore::new("file:memdb_offline_sync_timeout?mode=memory&cache=shared")
            .await
            .expect("Failed to create in-memory backend for test"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("persistence manager should initialize"),
    );
    let (client, _rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        Arc::new(crate::transport::mock::MockTransportFactory::new()),
        Arc::new(MockHttpClient),
        None,
    )
    .await;

    // Flag is false by default, so use a short timeout and verify the helper
    // marks the sync complete on timeout.
    let start = wacore::time::Instant::now();
    client
        .wait_for_offline_delivery_end_with_timeout(Duration::from_millis(50))
        .await;

    let elapsed = start.elapsed();
    // The drain finisher runs as a spawned task (off the read loop) and flips
    // the flag BEFORE swapping the semaphore, so neither the flag nor the
    // notifier alone proves the swap landed. Poll until the 64-permit
    // semaphore is observable (counting by non-blocking acquire).
    let mut permits = 0;
    for _ in 0..100 {
        let semaphore = match client.message_processing_semaphore.lock() {
            Ok(guard) => guard.clone(),
            Err(poisoned) => poisoned.into_inner().clone(),
        };
        let mut guards = Vec::new();
        while let Some(guard) = semaphore.try_acquire() {
            guards.push(guard);
        }
        permits = guards.len();
        drop(guards);
        if permits == 64 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }

    assert!(
        elapsed.as_millis() >= 45, // Allow small timing variance
        "Should have waited for the configured timeout duration, took {:?}",
        elapsed
    );
    assert!(
        client.offline_sync_completed.load(Ordering::Relaxed),
        "wait_for_offline_delivery_end should mark offline sync complete on timeout"
    );
    assert_eq!(
        permits, 64,
        "timeout completion should restore parallel permits"
    );

    info!("✅ test_wait_for_offline_delivery_end_times_out_when_flag_not_set passed");
}

/// Test that wait_for_offline_delivery_end returns when notified.
#[tokio::test]
async fn test_wait_for_offline_delivery_end_returns_on_notify() {
    let backend = Arc::new(
        crate::store::SqliteStore::new("file:memdb_offline_notify?mode=memory&cache=shared")
            .await
            .expect("Failed to create in-memory backend for test"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("persistence manager should initialize"),
    );
    let (client, _rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        Arc::new(crate::transport::mock::MockTransportFactory::new()),
        Arc::new(MockHttpClient),
        None,
    )
    .await;

    let client_clone = client.clone();

    // Spawn a task that will notify after 50ms
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(50)).await;
        client_clone.offline_sync_notifier.notify(usize::MAX);
    });

    let start = wacore::time::Instant::now();
    client.wait_for_offline_delivery_end().await;
    let elapsed = start.elapsed();

    // Should complete around 50ms (when notified), not 10 seconds
    assert!(
        elapsed.as_millis() < 200,
        "wait_for_offline_delivery_end should return when notified, took {:?}",
        elapsed
    );
    assert!(
        elapsed.as_millis() >= 45, // Should have waited for the notify
        "Should have waited for the notify, only took {:?}",
        elapsed
    );

    info!("✅ test_wait_for_offline_delivery_end_returns_on_notify passed");
}

/// Test that the offline_sync_completed flag starts as false.
#[tokio::test]
async fn test_offline_sync_flag_initially_false() {
    let backend = Arc::new(
        crate::store::SqliteStore::new("file:memdb_offline_flag_initial?mode=memory&cache=shared")
            .await
            .expect("Failed to create in-memory backend for test"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("persistence manager should initialize"),
    );
    let (client, _rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        Arc::new(crate::transport::mock::MockTransportFactory::new()),
        Arc::new(MockHttpClient),
        None,
    )
    .await;

    // The flag should be false initially
    assert!(
        !client.offline_sync_completed.load(Ordering::Relaxed),
        "offline_sync_completed should be false when Client is first created"
    );

    info!("✅ test_offline_sync_flag_initially_false passed");
}

/// Test the complete offline sync lifecycle:
/// 1. Flag starts false
/// 2. Flag is set true after IB offline stanza
/// 3. Notify is called
#[tokio::test]
async fn test_offline_sync_lifecycle() {
    use std::sync::atomic::Ordering;

    let backend = Arc::new(
        crate::store::SqliteStore::new("file:memdb_offline_lifecycle?mode=memory&cache=shared")
            .await
            .expect("Failed to create in-memory backend for test"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("persistence manager should initialize"),
    );
    let (client, _rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        Arc::new(crate::transport::mock::MockTransportFactory::new()),
        Arc::new(MockHttpClient),
        None,
    )
    .await;

    // 1. Initially false
    assert!(!client.offline_sync_completed.load(Ordering::Relaxed));

    // 2. Spawn a waiter
    let client_waiter = client.clone();
    let waiter_handle = tokio::spawn(async move {
        client_waiter.wait_for_offline_delivery_end().await;
        true // Return that we completed
    });

    // A registered listener proves the waiter reached its await point, so the
    // "still waiting" assertion below cannot pass just because the spawned task
    // never got scheduled.
    crate::test_utils::wait_for_notifier_listeners(&client.offline_sync_notifier, 1).await;

    // Verify waiter hasn't completed yet
    assert!(
        !waiter_handle.is_finished(),
        "Waiter should still be waiting"
    );

    // 3. Simulate IB handler behavior (set flag and notify)
    client.offline_sync_completed.store(true, Ordering::Relaxed);
    client.offline_sync_notifier.notify(usize::MAX);

    // 4. Waiter should complete
    let result = tokio::time::timeout(Duration::from_millis(100), waiter_handle)
        .await
        .expect("Waiter should complete after notify")
        .expect("Waiter task should not panic");

    assert!(result, "Waiter should have completed successfully");
    assert!(client.offline_sync_completed.load(Ordering::Relaxed));

    info!("✅ test_offline_sync_lifecycle passed");
}

/// Test that establish_primary_phone_session_immediate returns error when no PN is set.
/// This verifies the "not logged in" guard works.
#[tokio::test]
async fn test_establish_primary_phone_session_fails_without_pn() {
    let backend = Arc::new(
        crate::store::SqliteStore::new("file:memdb_no_pn?mode=memory&cache=shared")
            .await
            .expect("Failed to create in-memory backend for test"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("persistence manager should initialize"),
    );
    let (client, _rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        Arc::new(crate::transport::mock::MockTransportFactory::new()),
        Arc::new(MockHttpClient),
        None,
    )
    .await;

    // No PN set, so this should fail
    let result = client.establish_primary_phone_session_immediate().await;

    assert!(
        result.is_err(),
        "establish_primary_phone_session_immediate should fail when no PN is set"
    );

    let err = result.unwrap_err();
    assert!(
        err.downcast_ref::<ClientError>()
            .is_some_and(|e| matches!(e, ClientError::NotLoggedIn)),
        "Error should be ClientError::NotLoggedIn, got: {}",
        err
    );

    info!("✅ test_establish_primary_phone_session_fails_without_pn passed");
}

/// Test that ensure_e2e_sessions waits for offline sync to complete.
/// This is the CRITICAL difference between ensure_e2e_sessions and
/// establish_primary_phone_session_immediate.
#[tokio::test]
async fn test_ensure_e2e_sessions_waits_for_offline_sync() {
    use std::sync::atomic::Ordering;
    use wacore_binary::Jid;

    let backend = Arc::new(
        crate::store::SqliteStore::new("file:memdb_ensure_e2e_waits?mode=memory&cache=shared")
            .await
            .expect("Failed to create in-memory backend for test"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("persistence manager should initialize"),
    );
    let (client, _rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        Arc::new(crate::transport::mock::MockTransportFactory::new()),
        Arc::new(MockHttpClient),
        None,
    )
    .await;

    // Flag is false (offline sync not complete)
    assert!(!client.offline_sync_completed.load(Ordering::Relaxed));

    // Call ensure_e2e_sessions with an empty list (so it returns early after the wait)
    // This lets us test the waiting behavior without needing network
    let client_clone = client.clone();
    let ensure_handle = tokio::spawn(async move {
        // Start with some JIDs - but since we're testing the wait, we use empty
        // to avoid needing actual session establishment
        client_clone.ensure_e2e_sessions(&[]).await
    });

    // An empty list must not wait for offline sync: the flag is still false, so a
    // waiting call would hang until DEFAULT_OFFLINE_SYNC_TIMEOUT.
    tokio::time::timeout(Duration::from_secs(5), ensure_handle)
        .await
        .expect("ensure_e2e_sessions should return immediately for empty JID list")
        .expect("ensure_e2e_sessions task should not panic")
        .expect("empty JID list should succeed");

    // Now test with actual JIDs - it should wait for offline sync
    let client_clone = client.clone();
    let test_jid = Jid::pn("559999999999");
    let ensure_handle = tokio::spawn(async move {
        // This will wait for offline sync before proceeding
        let start = wacore::time::Instant::now();
        let _ = client_clone.ensure_e2e_sessions(&[test_jid]).await;
        start.elapsed()
    });

    // Registration is what makes the assertion below scheduler-independent: it
    // pins down that `ensure_e2e_sessions` is blocked on offline sync rather than
    // simply not started yet.
    crate::test_utils::wait_for_notifier_listeners(&client.offline_sync_notifier, 1).await;

    // It should still be waiting (offline sync not complete)
    assert!(
        !ensure_handle.is_finished(),
        "ensure_e2e_sessions should be waiting for offline sync"
    );

    // Now complete offline sync
    client.offline_sync_completed.store(true, Ordering::Relaxed);
    client.offline_sync_notifier.notify(usize::MAX);

    // Now it should complete (might fail on session establishment, but that's ok)
    let result = tokio::time::timeout(Duration::from_secs(2), ensure_handle).await;

    assert!(
        result.is_ok(),
        "ensure_e2e_sessions should complete after offline sync"
    );

    info!("✅ test_ensure_e2e_sessions_waits_for_offline_sync passed");
}

/// A warm session cache must satisfy the ensure without any network fetch:
/// the client here is disconnected, so reaching the usync fetch would error.
#[tokio::test]
async fn ensure_sessions_warm_cache_short_circuits() {
    use wacore::types::jid::JidExt;
    let client = crate::test_utils::create_test_client().await;
    let jid: Jid = "15550005555@s.whatsapp.net".parse().unwrap();

    // Cold cache and disconnected: the probe misses, so the fetch runs and
    // fails — proves the pre-filter does not silently skip unknown sessions.
    assert!(
        client
            .ensure_e2e_sessions_resolved(std::slice::from_ref(&jid))
            .await
            .is_err(),
        "unknown session must still attempt the fetch"
    );

    assert!(
        client
            .signal_cache
            .try_put_session(
                &jid.to_protocol_address(),
                wacore::libsignal::protocol::SessionRecord::new_fresh(),
            )
            .is_ok()
    );
    client
        .ensure_e2e_sessions_resolved(&[jid])
        .await
        .expect("cached session must satisfy ensure without network");
}

/// Integration test: Verify that the immediate session establishment does NOT
/// wait for offline sync. This is critical for PDO to work during offline sync.
///
/// The flow is:
/// 1. Login -> establish_primary_phone_session_immediate() is called
/// 2. This should NOT wait for offline sync (flag is false at this point)
/// 3. After session is established, offline messages arrive
/// 4. When decryption fails, PDO can immediately send to device 0
#[tokio::test]
async fn test_immediate_session_does_not_wait_for_offline_sync() {
    use std::sync::atomic::Ordering;
    use wacore_binary::Jid;

    let backend = Arc::new(
        crate::store::SqliteStore::new("file:memdb_immediate_no_wait?mode=memory&cache=shared")
            .await
            .expect("Failed to create in-memory backend for test"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend.clone())
            .await
            .expect("persistence manager should initialize"),
    );

    // Set a PN so establish_primary_phone_session_immediate doesn't fail early
    pm.modify_device(|device| {
        device.pn = Some(Jid::pn("559999999999"));
    })
    .await;

    let (client, _rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        Arc::new(crate::transport::mock::MockTransportFactory::new()),
        Arc::new(MockHttpClient),
        None,
    )
    .await;

    // Flag is false (offline sync not complete - simulating login state)
    assert!(!client.offline_sync_completed.load(Ordering::Relaxed));

    // Call establish_primary_phone_session_immediate
    // It should NOT wait for offline sync - it should proceed immediately
    let start = wacore::time::Instant::now();

    // Note: This will fail because we can't actually fetch prekeys in tests,
    // but the important thing is that it doesn't WAIT for offline sync
    let result = tokio::time::timeout(
        Duration::from_millis(500),
        client.establish_primary_phone_session_immediate(),
    )
    .await;

    let elapsed = start.elapsed();

    // The call should complete (or fail) quickly, NOT wait for 10 second timeout
    assert!(
        result.is_ok(),
        "establish_primary_phone_session_immediate should not wait for offline sync, timed out"
    );

    // It should complete in < 500ms (not 10 second wait)
    assert!(
        elapsed.as_millis() < 500,
        "establish_primary_phone_session_immediate should not wait, took {:?}",
        elapsed
    );

    // The actual result might be an error (no network), but that's fine
    // The important thing is it didn't wait for offline sync
    info!(
        "establish_primary_phone_session_immediate completed in {:?} (result: {:?})",
        elapsed,
        result.unwrap().is_ok()
    );

    info!("✅ test_immediate_session_does_not_wait_for_offline_sync passed");
}

/// Integration test: Verify that establish_primary_phone_session_immediate
/// skips establishment when a session already exists.
///
/// This is the CRITICAL fix for MAC verification failures:
/// - BUG (before fix): Called process_prekey_bundle() unconditionally,
///   replacing the existing session with a new one
/// - RESULT: Remote device still uses old session state, causing MAC failures
#[tokio::test]
async fn test_establish_session_skips_when_exists() {
    use wacore::libsignal::protocol::SessionRecord;
    use wacore::libsignal::store::SessionStore;
    use wacore::types::jid::JidExt;
    use wacore_binary::Jid;

    let backend = Arc::new(
        crate::store::SqliteStore::new("file:memdb_skip_existing?mode=memory&cache=shared")
            .await
            .expect("Failed to create in-memory backend for test"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend.clone())
            .await
            .expect("persistence manager should initialize"),
    );

    // Set a PN so the function doesn't fail early
    let own_pn = Jid::pn("559999999999");
    pm.modify_device(|device| {
        device.pn = Some(own_pn.clone());
    })
    .await;

    // Pre-populate a session for the primary phone JID (device 0)
    let primary_phone_jid = own_pn.with_device(0);
    let signal_addr = primary_phone_jid.to_protocol_address();

    // Create a dummy session record
    let dummy_session = SessionRecord::new_fresh();
    {
        let device_arc = pm.get_device_arc().await;
        let device = device_arc.read().await;
        device
            .store_session(&signal_addr, &dummy_session)
            .await
            .expect("Failed to store test session");

        // Verify session exists
        let exists = device
            .contains_session(&signal_addr)
            .await
            .expect("Failed to check session");
        assert!(exists, "Session should exist after store");
    }

    let (client, _rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm.clone(),
        Arc::new(crate::transport::mock::MockTransportFactory::new()),
        Arc::new(MockHttpClient),
        None,
    )
    .await;

    // Call establish_primary_phone_session_immediate
    // It should return Ok(()) immediately without fetching prekeys
    let result = client.establish_primary_phone_session_immediate().await;

    assert!(
        result.is_ok(),
        "establish_primary_phone_session_immediate should succeed when session exists"
    );

    // Verify the session was NOT replaced (still has the same record)
    // This is the critical assertion - if session was replaced, it would cause MAC failures
    {
        let device_arc = pm.get_device_arc().await;
        let device = device_arc.read().await;
        let exists = device
            .contains_session(&signal_addr)
            .await
            .expect("Failed to check session");
        assert!(exists, "Session should still exist after the call");
    }

    info!("✅ test_establish_session_skips_when_exists passed");
}

/// Integration test: Verify that the session check prevents MAC failures
/// by documenting the exact control flow that caused the bug.
#[test]
fn test_mac_failure_prevention_flow_documentation() {
    // Simulate the decision logic
    fn should_establish_session(check_result: Result<bool, &'static str>) -> Result<bool, String> {
        match check_result {
            Ok(true) => Ok(false), // Session exists → DON'T establish
            Ok(false) => Ok(true), // No session → establish
            Err(e) => Err(format!("Cannot verify session: {}", e)), // Fail-safe
        }
    }

    // Test Case 1: Session exists → skip (prevents MAC failure)
    let result = should_establish_session(Ok(true));
    assert_eq!(result, Ok(false), "Should skip when session exists");

    // Test Case 2: No session → establish
    let result = should_establish_session(Ok(false));
    assert_eq!(result, Ok(true), "Should establish when no session");

    // Test Case 3: Check fails → error (fail-safe)
    let result = should_establish_session(Err("database error"));
    assert!(result.is_err(), "Should fail when check fails");

    info!("✅ test_mac_failure_prevention_flow_documentation passed");
}

#[test]
fn test_unified_session_id_calculation() {
    // Test the mathematical calculation of the unified session ID.
    // Formula: (now_ms + server_offset_ms + 3_days_ms) % 7_days_ms

    const DAY_MS: i64 = 24 * 60 * 60 * 1000;
    const WEEK_MS: i64 = 7 * DAY_MS;
    const OFFSET_MS: i64 = 3 * DAY_MS;

    // Helper function matching the implementation
    fn calculate_session_id(now_ms: i64, server_offset_ms: i64) -> i64 {
        let adjusted_now = now_ms + server_offset_ms;
        (adjusted_now + OFFSET_MS) % WEEK_MS
    }

    // Test 1: Zero offset
    let now_ms = 1706000000000_i64; // Some arbitrary timestamp
    let id = calculate_session_id(now_ms, 0);
    assert!(
        (0..WEEK_MS).contains(&id),
        "Session ID should be in [0, WEEK_MS)"
    );

    // Test 2: Positive server offset (server is ahead)
    let id_with_positive_offset = calculate_session_id(now_ms, 5000);
    assert!(
        (0..WEEK_MS).contains(&id_with_positive_offset),
        "Session ID should be in [0, WEEK_MS)"
    );
    // The ID should be different from zero offset (unless wrap-around)
    // Not testing exact value as it depends on the offset

    // Test 3: Negative server offset (server is behind)
    let id_with_negative_offset = calculate_session_id(now_ms, -5000);
    assert!(
        (0..WEEK_MS).contains(&id_with_negative_offset),
        "Session ID should be in [0, WEEK_MS)"
    );

    // Test 4: Verify modulo wrap-around
    // If adjusted_now + OFFSET_MS >= WEEK_MS, it should wrap
    let wrap_test_now = WEEK_MS - OFFSET_MS + 1000; // Should produce small result
    let wrapped_id = calculate_session_id(wrap_test_now, 0);
    assert_eq!(wrapped_id, 1000, "Should wrap around correctly");

    // Test 5: Edge case - at exact boundary
    let boundary_now = WEEK_MS - OFFSET_MS;
    let boundary_id = calculate_session_id(boundary_now, 0);
    assert_eq!(boundary_id, 0, "At exact boundary should be 0");
}

#[tokio::test]
async fn test_server_time_offset_extraction() {
    use wacore_binary::builder::NodeBuilder;

    let backend = crate::test_utils::create_test_backend().await;
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("persistence manager should initialize"),
    );
    let (client, _rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        Arc::new(crate::transport::mock::MockTransportFactory::new()),
        Arc::new(MockHttpClient),
        None,
    )
    .await;

    // Initially, offset should be 0
    assert_eq!(
        client.unified_session.server_time_offset_ms(),
        0,
        "Initial offset should be 0"
    );

    // Create a node with a 't' attribute
    let server_time = wacore::time::now_secs() + 10; // Server is 10 seconds ahead
    let node = NodeBuilder::new("success").attr("t", server_time).build();

    // Update the offset
    client.update_server_time_offset(&node.as_node_ref());

    // The offset should be approximately 10 * 1000 = 10000 ms
    // Allow some tolerance for timing differences during the test
    let offset = client.unified_session.server_time_offset_ms();
    assert!(
        (offset - 10000).abs() < 1000, // Allow 1 second tolerance
        "Offset should be approximately 10000ms, got {}",
        offset
    );

    // Test with no 't' attribute - should not change offset
    let node_no_t = NodeBuilder::new("success").build();
    client.update_server_time_offset(&node_no_t.as_node_ref());
    let offset_after = client.unified_session.server_time_offset_ms();
    assert!(
        (offset_after - offset).abs() < 100, // Should be same (or very close)
        "Offset should not change when 't' is missing"
    );

    // Test with invalid 't' attribute - should not change offset
    let node_invalid = NodeBuilder::new("success")
        .attr("t", "not_a_number")
        .build();
    client.update_server_time_offset(&node_invalid.as_node_ref());
    let offset_after_invalid = client.unified_session.server_time_offset_ms();
    assert!(
        (offset_after_invalid - offset).abs() < 100,
        "Offset should not change when 't' is invalid"
    );

    // Test with negative/zero 't' - should not change offset
    let node_zero = NodeBuilder::new("success").attr("t", "0").build();
    client.update_server_time_offset(&node_zero.as_node_ref());
    let offset_after_zero = client.unified_session.server_time_offset_ms();
    assert!(
        (offset_after_zero - offset).abs() < 100,
        "Offset should not change when 't' is 0"
    );

    info!("✅ test_server_time_offset_extraction passed");
}

#[tokio::test]
async fn test_unified_session_manager_integration() {
    // Test the unified session manager through the client

    let backend = crate::test_utils::create_test_backend().await;
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("persistence manager should initialize"),
    );
    let (client, _rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        Arc::new(crate::transport::mock::MockTransportFactory::new()),
        Arc::new(MockHttpClient),
        None,
    )
    .await;

    // Initially, sequence should be 0
    assert_eq!(
        client.unified_session.sequence(),
        0,
        "Initial sequence should be 0"
    );

    // Duplicate prevention depends on the session ID staying the same between calls.
    // Since the session ID is millisecond-based, use a retry loop to handle
    // the rare case where we cross a millisecond boundary between calls.
    loop {
        client.unified_session.reset().await;

        let result = client.unified_session.prepare_send().await;
        assert!(result.is_some(), "First send should succeed");
        let (node, seq) = result.unwrap();
        assert_eq!(node.tag, "ib", "Should be an IB stanza");
        assert_eq!(seq, 1, "First sequence should be 1 (pre-increment)");
        assert_eq!(client.unified_session.sequence(), 1);

        let result2 = client.unified_session.prepare_send().await;
        if result2.is_none() {
            // Duplicate was prevented within the same millisecond
            assert_eq!(client.unified_session.sequence(), 1);
            break;
        }
        // Millisecond boundary crossed, retry
        tokio::task::yield_now().await;
    }

    // Clear last sent and try again - sequence resets on "new" session ID
    client.unified_session.clear_last_sent().await;
    let result3 = client.unified_session.prepare_send().await;
    assert!(result3.is_some(), "Should succeed after clearing");
    let (_, seq3) = result3.unwrap();
    assert_eq!(seq3, 1, "Sequence resets when session ID changes");
    assert_eq!(client.unified_session.sequence(), 1);

    info!("✅ test_unified_session_manager_integration passed");
}

#[test]
fn test_unified_session_protocol_node() {
    // Test the type-safe protocol node implementation
    use wacore::ib::{IbStanza, UnifiedSession};
    use wacore::protocol::ProtocolNode;

    // Create a unified session
    let session = UnifiedSession::new("123456789");
    assert_eq!(session.id, "123456789");
    assert_eq!(session.tag(), "unified_session");

    // Convert to node
    let node = session.into_node();
    assert_eq!(node.tag, "unified_session");
    assert!(node.attrs.get("id").is_some_and(|v| v == "123456789"));

    // Create an IB stanza
    let stanza = IbStanza::unified_session(UnifiedSession::new("987654321"));
    assert_eq!(stanza.tag(), "ib");

    // Convert to node and verify structure
    let ib_node = stanza.into_node();
    assert_eq!(ib_node.tag, "ib");
    let children = ib_node.children().expect("IB stanza should have children");
    assert_eq!(children.len(), 1);
    assert_eq!(children[0].tag, "unified_session");
    assert!(
        children[0]
            .attrs
            .get("id")
            .is_some_and(|v| v == "987654321")
    );

    info!("✅ test_unified_session_protocol_node passed");
}

fn node_to_owned_ref(node: Node) -> Arc<OwnedNodeRef> {
    crate::test_utils::node_to_owned_ref(&node)
}

/// Helper to create a test client for offline sync tests
async fn create_offline_sync_test_client() -> Arc<Client> {
    let backend = crate::test_utils::create_test_backend().await;
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("persistence manager should initialize"),
    );
    let (client, _rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        Arc::new(crate::transport::mock::MockTransportFactory::new()),
        Arc::new(MockHttpClient),
        None,
    )
    .await;
    client
}

/// Regression: a transport disconnect must flush dirty Signal state before
/// clearing the cache, or a just-advanced sender-key chain is lost (forcing
/// a full SKDM re-fanout on the next send).
#[tokio::test]
async fn cleanup_connection_state_flushes_dirty_signal_state() {
    use wacore::libsignal::protocol::ProtocolAddress;
    let client = create_offline_sync_test_client().await;

    // A dirty identity lives only in the write-back cache until flushed.
    let addr = ProtocolAddress::new("5550001000@s.whatsapp.net", 1u32.into());
    client.signal_cache.put_identity(&addr, &[7u8; 32]).await;

    client.cleanup_connection_state().await;

    // cleanup cleared the cache, so a hit now can only come from the DB,
    // proving the flush ran before the clear.
    let device = client.persistence_manager.get_device_arc().await;
    let guard = device.read().await;
    let persisted = client
        .signal_cache
        .get_identity(&addr, &*guard.backend)
        .await
        .expect("get_identity must not error");
    assert!(
        persisted.is_some(),
        "dirty Signal state must survive a transport disconnect (flush-before-clear)"
    );
}

/// Same guarantee on the sender-key store, which drives SKDM fanout.
#[tokio::test]
async fn cleanup_connection_state_flushes_dirty_sender_key() {
    use wacore::libsignal::protocol::SenderKeyRecord;
    use wacore::libsignal::store::sender_key_name::SenderKeyName;
    let client = create_offline_sync_test_client().await;

    let name = SenderKeyName::from_parts("group@g.us", "5550001000@s.whatsapp.net:1");
    client
        .signal_cache
        .put_sender_key(&name, SenderKeyRecord::new_empty())
        .await;

    client.cleanup_connection_state().await;

    let device = client.persistence_manager.get_device_arc().await;
    let guard = device.read().await;
    let persisted = client
        .signal_cache
        .get_sender_key(&name, &*guard.backend)
        .await
        .expect("get_sender_key must not error");
    assert!(
        persisted.is_some(),
        "dirty sender key must survive a transport disconnect (flush-before-clear)"
    );
}

#[tokio::test]
async fn cleanup_connection_state_does_not_burn_a_clean_sender_key_lease() {
    use wacore::libsignal::protocol::{KeyPair, SenderKeyRecord};
    use wacore::libsignal::store::sender_key_name::SenderKeyName;
    let client = create_offline_sync_test_client().await;
    let name = SenderKeyName::from_parts("group@g.us", "5550001001@s.whatsapp.net:1");
    let mut rng = rand::make_rng::<rand::rngs::StdRng>();
    let signing_key = KeyPair::generate(&mut rng);
    let mut record = SenderKeyRecord::new_empty();
    record
        .add_sender_key_state(
            3,
            12345,
            0,
            &[0x42; 32],
            signing_key.public_key,
            Some(signing_key.private_key),
        )
        .expect("sender key state");
    record.reserve_iterations(0);
    client.signal_cache.put_sender_key(&name, record).await;

    client.cleanup_connection_state().await;

    let device = client.persistence_manager.get_device_arc().await;
    let guard = device.read().await;
    let reloaded = client
        .signal_cache
        .get_sender_key(&name, &*guard.backend)
        .await
        .expect("sender key load")
        .expect("sender key");
    assert_eq!(
        reloaded
            .sender_key_state()
            .expect("sender key state")
            .sender_chain_key()
            .expect("sender chain")
            .iteration(),
        0
    );
}

/// When the flush itself fails, cleanup must NOT clear the cache, or it would
/// drop the very state the flush was meant to persist.
#[tokio::test]
async fn cleanup_connection_state_keeps_state_when_flush_fails() {
    use wacore::libsignal::protocol::{ProtocolAddress, SenderKeyRecord};
    use wacore::libsignal::store::sender_key_name::SenderKeyName;
    let client = create_offline_sync_test_client().await;

    // A malformed identity (not 32 bytes) makes flush() error out, standing
    // in for a transient backend write failure during cleanup.
    let bad = ProtocolAddress::new("5550002000@s.whatsapp.net", 1u32.into());
    client.signal_cache.put_identity(&bad, &[0u8; 16]).await;

    // A valid dirty sender key that must not be dropped when the flush fails.
    let name = SenderKeyName::from_parts("group@g.us", "5550001000@s.whatsapp.net:1");
    client
        .signal_cache
        .put_sender_key(&name, SenderKeyRecord::new_empty())
        .await;

    client.cleanup_connection_state().await;

    // flush() failed, so clear() was skipped; the unpersisted sender key
    // survives in the write-back cache instead of being dropped.
    let device = client.persistence_manager.get_device_arc().await;
    let guard = device.read().await;
    let persisted = client
        .signal_cache
        .get_sender_key(&name, &*guard.backend)
        .await
        .expect("get_sender_key must not error");
    assert!(
        persisted.is_some(),
        "a flush failure must not drop dirty Signal state"
    );
}

/// A 403 connect failure is WA Web's REASON_LOCKED: it must surface a logout
/// carrying AccountLocked and disable auto-reconnect (a lock is not transient).
#[tokio::test]
async fn connect_failure_403_dispatches_account_locked_logout() {
    use wacore::types::events::ChannelEventHandler;
    let client = create_offline_sync_test_client().await;
    let (handler, events) = ChannelEventHandler::new();
    client.subscribe_handler(handler).detach();

    // location="rva" is a region routing token and must not change the verdict.
    let failure = NodeBuilder::new("failure")
        .attr("reason", "403")
        .attr("location", "rva")
        .build();
    client.handle_connect_failure(&failure.as_node_ref()).await;

    let evt = events
        .try_recv()
        .expect("403 must dispatch a LoggedOut event");
    match &*evt {
        Event::LoggedOut(lo) => {
            assert!(lo.on_connect, "403 arrives as a failure-on-connect");
            assert_eq!(lo.reason, ConnectFailureReason::AccountLocked);
        }
        _ => panic!("expected Event::LoggedOut for reason=403"),
    }
    assert!(
        !client.enable_auto_reconnect.load(Ordering::Relaxed),
        "a server-side lock must not auto-reconnect"
    );
}

/// An account lock states its enforcement data exactly once: the appeal token is
/// the only route to contest the lock, and `violation_reason` / `vt` are the
/// only description of it. WA Web ignores those attributes (its appeal flow is
/// native), so nothing parses them here either — but dropping them makes them
/// unrecoverable, so the whole stanza rides on the event.
#[tokio::test]
async fn account_lock_logout_preserves_enforcement_attributes() {
    use wacore::types::events::ChannelEventHandler;
    let client = create_offline_sync_test_client().await;
    let (handler, events) = ChannelEventHandler::new();
    client.subscribe_handler(handler).detach();

    let failure = NodeBuilder::new("failure")
        .attr("reason", "403")
        .attr("location", "rva")
        .attr("violation_reason", "other_harm")
        .attr("vt", "1")
        .attr("appeal_token", "0aFICTITIOUSappealTOKEN00")
        .attr("logout_message_header", "Conta desconectada")
        .attr("logout_message_subtext", "Abra o WhatsApp no celular")
        .attr("logout_message_locale", "pt_BR")
        .build();
    client.handle_connect_failure(&failure.as_node_ref()).await;

    let evt = events.try_recv().expect("403 dispatches LoggedOut");
    match &*evt {
        Event::LoggedOut(lo) => {
            let raw = lo.raw.as_ref().expect("the <failure> stanza must survive");
            assert_eq!(
                raw.attrs.get("appeal_token").map(|v| v.as_str()).as_deref(),
                Some("0aFICTITIOUSappealTOKEN00"),
                "the one-time appeal token must reach the embedder"
            );
            assert_eq!(
                raw.attrs
                    .get("violation_reason")
                    .map(|v| v.as_str())
                    .as_deref(),
                Some("other_harm")
            );
            assert_eq!(
                raw.attrs.get("vt").map(|v| v.as_str()).as_deref(),
                Some("1")
            );

            // The localized copy is typed, the way WA Web parses it, with the
            // locale that decides whether it is safe to render.
            let msg = lo
                .logout_message
                .as_ref()
                .expect("logout_message_* must be surfaced");
            assert_eq!(msg.header.as_deref(), Some("Conta desconectada"));
            assert_eq!(msg.subtext.as_deref(), Some("Abra o WhatsApp no celular"));
            assert_eq!(msg.locale.as_deref(), Some("pt_BR"));
        }
        _ => panic!("expected Event::LoggedOut for reason=403"),
    }
}

/// WA Web hands `code`, `expire`, `message` and `url` to its temporary-ban UI —
/// `url` is the link it opens. All four must survive the dispatch.
#[tokio::test]
async fn temporary_ban_carries_message_url_and_stanza() {
    use wacore::types::events::{ChannelEventHandler, TempBanReason};
    let client = create_offline_sync_test_client().await;
    let (handler, events) = ChannelEventHandler::new();
    client.subscribe_handler(handler).detach();

    let failure = NodeBuilder::new("failure")
        .attr("reason", "402")
        .attr("code", "101")
        .attr("expire", "3600")
        .attr("message", "too many messages")
        .attr("url", "https://faq.example.invalid/ban")
        .build();
    client.handle_connect_failure(&failure.as_node_ref()).await;

    match &*events.try_recv().expect("402 dispatches TemporaryBan") {
        Event::TemporaryBan(ban) => {
            assert_eq!(ban.code, TempBanReason::SentToTooManyPeople);
            assert_eq!(ban.expire, chrono::Duration::seconds(3600));
            assert_eq!(ban.message.as_deref(), Some("too many messages"));
            assert_eq!(ban.url.as_deref(), Some("https://faq.example.invalid/ban"));
            assert!(ban.raw.is_some(), "the <failure> stanza must survive");
        }
        _ => panic!("expected Event::TemporaryBan for reason=402"),
    }
}

/// A 402 without `expire` is not a ban that lifted at the epoch. WA Web errors
/// out rather than reporting one, so we surface the raw failure instead of
/// fabricating a zero duration.
#[tokio::test]
async fn temporary_ban_without_expire_falls_back_to_connect_failure() {
    use wacore::types::events::ChannelEventHandler;
    let client = create_offline_sync_test_client().await;
    let (handler, events) = ChannelEventHandler::new();
    client.subscribe_handler(handler).detach();

    let failure = NodeBuilder::new("failure")
        .attr("reason", "402")
        .attr("code", "101")
        .build();
    client.handle_connect_failure(&failure.as_node_ref()).await;

    match &*events
        .try_recv()
        .expect("an incomplete 402 still dispatches")
    {
        Event::ConnectFailure(cf) => {
            assert_eq!(cf.reason, ConnectFailureReason::TempBanned);
            assert!(cf.raw.is_some(), "the <failure> stanza must survive");
        }
        other => panic!("expected Event::ConnectFailure, got {other:?}"),
    }
}

/// An `expire` that cannot be a `Duration` is no better than a missing one: it
/// must not reach a consumer as a ban of length zero.
#[tokio::test]
async fn temporary_ban_with_unrepresentable_expire_falls_back_to_connect_failure() {
    use wacore::types::events::ChannelEventHandler;
    let client = create_offline_sync_test_client().await;
    let (handler, events) = ChannelEventHandler::new();
    client.subscribe_handler(handler).detach();

    let failure = NodeBuilder::new("failure")
        .attr("reason", "402")
        .attr("code", "101")
        .attr("expire", u64::MAX.to_string())
        .build();
    client.handle_connect_failure(&failure.as_node_ref()).await;

    match &*events.try_recv().expect("a garbage 402 still dispatches") {
        Event::ConnectFailure(cf) => {
            assert_eq!(cf.reason, ConnectFailureReason::TempBanned);
            assert!(cf.raw.is_some(), "the <failure> stanza must survive");
        }
        other => panic!("expected Event::ConnectFailure, got {other:?}"),
    }
}

/// 405 is the one branch with nothing to parse, which is exactly why it used to
/// throw the stanza away; the client version the server rejected is in there.
#[tokio::test]
async fn client_outdated_carries_the_stanza() {
    use wacore::types::events::ChannelEventHandler;
    let client = create_offline_sync_test_client().await;
    let (handler, events) = ChannelEventHandler::new();
    client.subscribe_handler(handler).detach();

    let failure = NodeBuilder::new("failure")
        .attr("reason", "405")
        .attr("message", "client too old")
        .build();
    client.handle_connect_failure(&failure.as_node_ref()).await;

    match &*events.try_recv().expect("405 dispatches ClientOutdated") {
        Event::ClientOutdated(co) => {
            assert!(co.raw.is_some(), "the <failure> stanza must survive")
        }
        _ => panic!("expected Event::ClientOutdated for reason=405"),
    }
}

#[tokio::test]
async fn delivery_receipt_activity_state_machine() {
    let client = create_offline_sync_test_client().await;
    assert!(
        !client.receipts_are_active(),
        "default is inactive (background companion)"
    );
    client.mark_receipts_active_on_presence();
    assert!(client.receipts_are_active(), "presence available -> active");
    client.mark_receipts_inactive_on_presence();
    assert!(
        !client.receipts_are_active(),
        "presence unavailable -> inactive"
    );
    client.set_force_active_delivery_receipts(true);
    assert!(client.receipts_are_active(), "forced active");
    client.mark_receipts_inactive_on_presence();
    assert!(
        client.receipts_are_active(),
        "forced (2) survives a presence-unavailable CAS(1,0)"
    );
    client.set_force_active_delivery_receipts(false);
    assert!(!client.receipts_are_active());

    // Teardown resets presence-driven active (so it doesn't leak across
    // reconnects) but preserves a forced value.
    client.mark_receipts_active_on_presence();
    client.cleanup_connection_state().await;
    assert!(
        !client.receipts_are_active(),
        "teardown resets presence-driven active"
    );
    client.set_force_active_delivery_receipts(true);
    client.cleanup_connection_state().await;
    assert!(
        client.receipts_are_active(),
        "teardown preserves forced active"
    );
}

#[tokio::test]
async fn test_ib_thread_metadata_does_not_end_sync() {
    let client = create_offline_sync_test_client().await;
    client
        .offline_sync_metrics
        .active
        .store(true, Ordering::Release);

    let node = NodeBuilder::new("ib")
        .children([NodeBuilder::new("thread_metadata")
            .children([NodeBuilder::new("item").build()])
            .build()])
        .build();

    client.process_node(node_to_owned_ref(node)).await;
    assert!(
        client.offline_sync_metrics.active.load(Ordering::Acquire),
        "<ib><thread_metadata> should NOT end offline sync"
    );
}

#[tokio::test]
async fn test_ib_edge_routing_does_not_end_sync() {
    let client = create_offline_sync_test_client().await;
    client
        .offline_sync_metrics
        .active
        .store(true, Ordering::Release);

    let node = NodeBuilder::new("ib")
        .children([NodeBuilder::new("edge_routing")
            .children([NodeBuilder::new("routing_info")
                .bytes(vec![1, 2, 3])
                .build()])
            .build()])
        .build();

    client.process_node(node_to_owned_ref(node)).await;
    assert!(
        client.offline_sync_metrics.active.load(Ordering::Acquire),
        "<ib><edge_routing> should NOT end offline sync"
    );
}

#[tokio::test]
async fn test_ib_dirty_does_not_end_sync() {
    let client = create_offline_sync_test_client().await;
    client
        .offline_sync_metrics
        .active
        .store(true, Ordering::Release);

    let node = NodeBuilder::new("ib")
        .children([NodeBuilder::new("dirty")
            .attr("type", "groups")
            .attr("timestamp", "1234")
            .build()])
        .build();

    client.process_node(node_to_owned_ref(node)).await;
    assert!(
        client.offline_sync_metrics.active.load(Ordering::Acquire),
        "<ib><dirty> should NOT end offline sync"
    );
}

#[tokio::test]
async fn test_ib_offline_child_ends_sync() {
    let client = create_offline_sync_test_client().await;
    client
        .offline_sync_metrics
        .active
        .store(true, Ordering::Release);
    client
        .offline_sync_metrics
        .total_messages
        .store(301, Ordering::Release);

    let node = NodeBuilder::new("ib")
        .children([NodeBuilder::new("offline").attr("count", "301").build()])
        .build();

    client.process_node(node_to_owned_ref(node)).await;
    assert!(
        !client.offline_sync_metrics.active.load(Ordering::Acquire),
        "<ib><offline count='301'/> should end offline sync"
    );
}

#[tokio::test]
async fn test_ib_offline_preview_starts_sync() {
    let client = create_offline_sync_test_client().await;

    let node = NodeBuilder::new("ib")
        .children([NodeBuilder::new("offline_preview")
            .attr("count", "301")
            .attr("message", "168")
            .attr("notification", "62")
            .attr("receipt", "68")
            .attr("appdata", "0")
            .build()])
        .build();

    client.process_node(node_to_owned_ref(node)).await;
    assert!(
        client.offline_sync_metrics.active.load(Ordering::Acquire),
        "offline_preview with count>0 should activate sync"
    );
    assert_eq!(
        client
            .offline_sync_metrics
            .total_messages
            .load(Ordering::Acquire),
        301
    );
}

#[tokio::test]
async fn test_offline_message_increments_processed() {
    let client = create_offline_sync_test_client().await;
    client
        .offline_sync_metrics
        .active
        .store(true, Ordering::Release);
    client
        .offline_sync_metrics
        .total_messages
        .store(100, Ordering::Release);

    let node = NodeBuilder::new("message")
        .attr("offline", "1")
        .attr("from", "5551234567@s.whatsapp.net")
        .attr("id", "TEST123")
        .attr("t", "1772884671")
        .attr("type", "text")
        .build();

    client.process_node(node_to_owned_ref(node)).await;
    assert_eq!(
        client
            .offline_sync_metrics
            .processed_messages
            .load(Ordering::Acquire),
        1,
        "offline message should increment processed count"
    );
}

// ---------------------------------------------------------------
// Server-initiated ping detection tests
//
// The WhatsApp server can send pings in two formats:
//
// 1. Child-element format (legacy/whatsmeow style):
//    <iq type="get" from="s.whatsapp.net" id="...">
//      <ping/>
//    </iq>
//
// 2. xmlns-attribute format (real WhatsApp Web format):
//    <iq from="s.whatsapp.net" t="..." type="get" xmlns="urn:xmpp:ping"/>
//    This is a self-closing tag with NO child elements.
//    Verified against captured WhatsApp Web JS (WAWebCommsHandleStanza):
//      if (t.xmlns === "urn:xmpp:ping") return wap("iq", { type: "result", to: t.from });
//
// Both must be recognized and answered with a pong, otherwise the
// server considers the client dead and stops responding to keepalive
// pings — causing a timeout cascade and forced reconnect.
// ---------------------------------------------------------------

#[tokio::test]
async fn test_handle_iq_ping_with_child_element() {
    // Format 1: <iq type="get"><ping/></iq> — the legacy format with a <ping> child node.
    let backend = crate::test_utils::create_test_backend().await;
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("persistence manager should initialize"),
    );
    let (client, _rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        Arc::new(crate::transport::mock::MockTransportFactory::new()),
        Arc::new(MockHttpClient),
        None,
    )
    .await;

    let ping_node = NodeBuilder::new("iq")
        .attr("type", "get")
        .attr("from", SERVER_JID)
        .attr("id", "ping-child-1")
        .children([NodeBuilder::new("ping").build()])
        .build();

    let handled = client.handle_iq(&ping_node.as_node_ref()).await;
    assert!(
        handled,
        "handle_iq must recognize ping with <ping> child element"
    );
}

#[tokio::test]
async fn test_handle_iq_ping_with_xmlns_attribute() {
    // Format 2: <iq type="get" xmlns="urn:xmpp:ping"/> — the real WhatsApp Web format.
    // This is a self-closing IQ with NO children, only an xmlns attribute.
    // The server sends this format; failing to respond causes keepalive timeout cascade.
    let backend = crate::test_utils::create_test_backend().await;
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("persistence manager should initialize"),
    );
    let (client, _rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        Arc::new(crate::transport::mock::MockTransportFactory::new()),
        Arc::new(MockHttpClient),
        None,
    )
    .await;

    let ping_node = NodeBuilder::new("iq")
        .attr("type", "get")
        .attr("from", SERVER_JID)
        .attr("id", "ping-xmlns-1")
        .attr("xmlns", "urn:xmpp:ping")
        .build();

    let handled = client.handle_iq(&ping_node.as_node_ref()).await;
    assert!(
        handled,
        "handle_iq must recognize ping with xmlns=\"urn:xmpp:ping\" attribute (no children)"
    );
}

#[tokio::test]
async fn test_handle_iq_ping_with_both_child_and_xmlns() {
    // Edge case: node has BOTH a <ping> child AND xmlns="urn:xmpp:ping".
    // Should still be handled (OR condition).
    let backend = crate::test_utils::create_test_backend().await;
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("persistence manager should initialize"),
    );
    let (client, _rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        Arc::new(crate::transport::mock::MockTransportFactory::new()),
        Arc::new(MockHttpClient),
        None,
    )
    .await;

    let ping_node = NodeBuilder::new("iq")
        .attr("type", "get")
        .attr("from", SERVER_JID)
        .attr("id", "ping-both-1")
        .attr("xmlns", "urn:xmpp:ping")
        .children([NodeBuilder::new("ping").build()])
        .build();

    let handled = client.handle_iq(&ping_node.as_node_ref()).await;
    assert!(
        handled,
        "handle_iq must handle ping with both child and xmlns"
    );
}

#[tokio::test]
async fn test_handle_iq_ping_without_type_attr() {
    // WA Web pongs for any xmlns="urn:xmpp:ping" regardless of (or absent) type.
    // A ping with no type attr must still be answered, not dropped.
    let backend = crate::test_utils::create_test_backend().await;
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("persistence manager should initialize"),
    );
    let (client, _rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        Arc::new(crate::transport::mock::MockTransportFactory::new()),
        Arc::new(MockHttpClient),
        None,
    )
    .await;

    let ping_node = NodeBuilder::new("iq")
        .attr("from", SERVER_JID)
        .attr("id", "ping-notype-1")
        .attr("xmlns", "urn:xmpp:ping")
        .build();

    let handled = client.handle_iq(&ping_node.as_node_ref()).await;
    assert!(
        handled,
        "handle_iq must pong a urn:xmpp:ping IQ even without a type attribute"
    );
}

#[tokio::test]
async fn test_handle_iq_non_ping_returns_false() {
    // A type="get" IQ without ping child or xmlns should NOT be handled as ping.
    let backend = crate::test_utils::create_test_backend().await;
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("persistence manager should initialize"),
    );
    let (client, _rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        Arc::new(crate::transport::mock::MockTransportFactory::new()),
        Arc::new(MockHttpClient),
        None,
    )
    .await;

    let non_ping_node = NodeBuilder::new("iq")
        .attr("type", "get")
        .attr("from", SERVER_JID)
        .attr("id", "not-a-ping")
        .attr("xmlns", "some:other:namespace")
        .build();

    let handled = client.handle_iq(&non_ping_node.as_node_ref()).await;
    assert!(
        !handled,
        "handle_iq must NOT treat non-ping xmlns as a ping"
    );
}

#[tokio::test]
async fn test_handle_iq_ping_wrong_type_returns_false() {
    // xmlns="urn:xmpp:ping" but type="result" (not "get") — should NOT be handled as ping.
    let backend = crate::test_utils::create_test_backend().await;
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("persistence manager should initialize"),
    );
    let (client, _rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        Arc::new(crate::transport::mock::MockTransportFactory::new()),
        Arc::new(MockHttpClient),
        None,
    )
    .await;

    let result_node = NodeBuilder::new("iq")
        .attr("type", "result")
        .attr("from", SERVER_JID)
        .attr("id", "ping-result-1")
        .attr("xmlns", "urn:xmpp:ping")
        .build();

    let handled = client.handle_iq(&result_node.as_node_ref()).await;
    assert!(
        !handled,
        "handle_iq must NOT respond to type=\"result\" even with ping xmlns"
    );
}

// ── build_pong tests ──────────────────────────────────────────────

#[test]
fn test_build_pong_with_id() {
    let pong = build_pong("s.whatsapp.net".to_string(), Some("ping-123"));
    assert!(
        pong.attrs.get("id").is_some_and(|v| v == "ping-123"),
        "pong should include id when server ping has one"
    );
    assert!(pong.attrs.get("type").is_some_and(|v| v == "result"));
    assert!(pong.attrs.get("to").is_some_and(|v| v == "s.whatsapp.net"));
}

#[test]
fn test_build_pong_without_id() {
    let pong = build_pong("s.whatsapp.net".to_string(), None);
    assert!(
        !pong.attrs.contains_key("id"),
        "pong should NOT include id when server ping has none"
    );
    assert!(pong.attrs.get("type").is_some_and(|v| v == "result"));
}

#[test]
fn test_encrypt_identity_notification_omits_type() {
    let node = NodeBuilder::new("notification")
        .attr("from", "186303081611421@lid")
        .attr("id", "4128735301")
        .attr("type", "encrypt")
        .children([NodeBuilder::new("identity").build()])
        .build();

    assert!(
        is_encrypt_identity_notification(&node.as_node_ref()),
        "identity-change notification ACK must omit type to match WA Web"
    );
}

#[test]
fn test_device_notification_is_not_encrypt_identity() {
    let node = NodeBuilder::new("notification")
        .attr("from", "186303081611421@lid")
        .attr("id", "269488578")
        .attr("type", "devices")
        .children([NodeBuilder::new("remove").build()])
        .build();

    assert!(
        !is_encrypt_identity_notification(&node.as_node_ref()),
        "device notification is not an encrypt+identity notification"
    );
}

#[test]
fn test_build_ack_node_for_message_preserves_type_and_includes_from() {
    // Generic message acknowledgements echo the stanza type and identify the
    // local device in `from`.
    let incoming = NodeBuilder::new("message")
        .attr("from", "120363161500776365@g.us")
        .attr("id", "A5791A5392EF60E3FB0670098DE010D4")
        .attr("type", "text")
        .attr("participant", "181531758878822@lid")
        .build();
    let own_device_pn: Jid = "155500012345:48@s.whatsapp.net"
        .parse()
        .expect("own device PN JID should parse");

    let ack = build_ack_node(&incoming.as_node_ref(), Some(&own_device_pn))
        .expect("message ack should be buildable");

    assert_eq!(ack.tag, "ack");
    // Use PartialEq<str> on NodeValue — works for both String and Jid variants
    // without allocation, so tests don't depend on internal representation.
    assert!(ack.attrs.get("class").is_some_and(|v| v == "message"));
    assert!(
        ack.attrs
            .get("to")
            .is_some_and(|v| v == "120363161500776365@g.us")
    );
    assert!(
        ack.attrs
            .get("from")
            .is_some_and(|v| v == "155500012345:48@s.whatsapp.net")
    );
    assert!(
        ack.attrs
            .get("participant")
            .is_some_and(|v| v == "181531758878822@lid")
    );
    assert!(
        ack.attrs.get("type").is_some_and(|v| v == "text"),
        "message ACK must echo its explicit type"
    );
}

#[test]
fn test_build_ack_node_for_identity_change_omits_type_and_from() {
    let incoming = NodeBuilder::new("notification")
        .attr("from", "186303081611421@lid")
        .attr("id", "4128735301")
        .attr("type", "encrypt")
        .children([NodeBuilder::new("identity").build()])
        .build();
    let own_device_pn: Jid = "155500012345:48@s.whatsapp.net"
        .parse()
        .expect("own device PN JID should parse");

    let ack = build_ack_node(&incoming.as_node_ref(), Some(&own_device_pn))
        .expect("notification ack should be buildable");

    assert!(ack.attrs.get("class").is_some_and(|v| v == "notification"));
    assert!(
        !ack.attrs.contains_key("type"),
        "identity-change notification ACK must omit type"
    );
    assert!(
        !ack.attrs.contains_key("from"),
        "notification ACKs should not include our device PN"
    );
}

#[test]
fn test_build_ack_node_for_receipt_with_type_echoes_type() {
    // Receipt acks should echo the type attribute when present (e.g. "read", "played").
    let incoming = NodeBuilder::new("receipt")
        .attr("from", "156535032389744@lid")
        .attr("id", "RCPT-WITH-TYPE")
        .attr("type", "read")
        .build();
    let own_device_pn: Jid = "155500012345:48@s.whatsapp.net"
        .parse()
        .expect("own device PN JID should parse");

    let ack = build_ack_node(&incoming.as_node_ref(), Some(&own_device_pn))
        .expect("receipt ack should be buildable");

    assert!(ack.attrs.get("class").is_some_and(|v| v == "receipt"));
    assert!(
        ack.attrs.get("type").is_some_and(|v| v == "read"),
        "receipt ACK must echo the type attribute when present"
    );
    assert!(
        !ack.attrs.contains_key("from"),
        "receipt ACKs should not include our device PN"
    );
}

#[test]
fn test_build_ack_node_drops_participant_when_equal_to_from() {
    // WAWebReceiptAck: `participant: r && r !== e ? DEVICE_JID(r) : DROP_ATTR`.
    // When the incoming stanza carries participant == from (redundant),
    // the ack must not echo it.
    let incoming = NodeBuilder::new("receipt")
        .attr("from", "156535032389744@lid")
        .attr("participant", "156535032389744@lid")
        .attr("id", "RCPT-PARTICIPANT-EQ-FROM")
        .build();
    let own_device_pn: Jid = "155500012345:48@s.whatsapp.net".parse().unwrap();

    let ack =
        build_ack_node(&incoming.as_node_ref(), Some(&own_device_pn)).expect("ack should build");
    assert!(
        !ack.attrs.contains_key("participant"),
        "ack must drop participant when it duplicates `to` (the flipped from); got {:?}",
        ack.attrs.get("participant")
    );
}

#[test]
fn test_build_ack_node_keeps_participant_when_distinct_from_from() {
    // Group receipt: participant = sender (user), from = group jid; must be kept.
    let incoming = NodeBuilder::new("receipt")
        .attr("from", "120363098765432100@g.us")
        .attr("participant", "5511999999999@s.whatsapp.net")
        .attr("id", "RCPT-GROUP")
        .build();
    let own_device_pn: Jid = "155500012345:48@s.whatsapp.net".parse().unwrap();

    let ack =
        build_ack_node(&incoming.as_node_ref(), Some(&own_device_pn)).expect("ack should build");
    assert!(
        ack.attrs
            .get("participant")
            .is_some_and(|v| v == "5511999999999@s.whatsapp.net"),
        "ack must keep participant when it differs from `to`"
    );
}

#[test]
fn test_build_ack_node_for_receipt_without_type_omits_type() {
    // Delivery receipts have no type attribute — the ack must also omit it.
    // Sending type="delivery" in the ack causes stream:error disconnections.
    let incoming = NodeBuilder::new("receipt")
        .attr("from", "156535032389744@lid")
        .attr("id", "RCPT-NO-TYPE")
        .build();
    let own_device_pn: Jid = "155500012345:48@s.whatsapp.net"
        .parse()
        .expect("own device PN JID should parse");

    let ack = build_ack_node(&incoming.as_node_ref(), Some(&own_device_pn))
        .expect("receipt ack should be buildable");

    assert!(ack.attrs.get("class").is_some_and(|v| v == "receipt"));
    assert!(
        !ack.attrs.contains_key("type"),
        "receipt ACK must NOT contain type when the incoming receipt has no type attribute"
    );
    assert!(
        !ack.attrs.contains_key("from"),
        "receipt ACKs should not include our device PN"
    );
}

#[test]
fn test_build_ack_node_for_message_with_recipient_preserves_recipient() {
    // Peer / hosted-companion / LID-routed messages carry `recipient`.
    // The server uses it to route the ack back to the origin device;
    // without it the stream is torn down with <stream:error><ack/></stream:error>.
    let incoming = NodeBuilder::new("message")
        .attr("from", "166361967902821@lid")
        .attr("id", "2A32F960553696093D99")
        .attr("type", "text")
        .attr("recipient", "146991363395800@lid")
        .build();
    let own_device_pn: Jid = "155500012345:48@s.whatsapp.net"
        .parse()
        .expect("own device PN JID should parse");

    let ack = build_ack_node(&incoming.as_node_ref(), Some(&own_device_pn))
        .expect("message ack should be buildable");

    assert!(ack.attrs.get("class").is_some_and(|v| v == "message"));
    assert!(
        ack.attrs
            .get("recipient")
            .is_some_and(|v| v == "146991363395800@lid"),
        "message ACK must echo the incoming `recipient` attribute"
    );
}

#[test]
fn test_build_ack_node_for_receipt_with_recipient_preserves_recipient() {
    // Receipt acks must also echo `recipient` when the incoming carries it.
    let incoming = NodeBuilder::new("receipt")
        .attr("from", "120363098765432100@g.us")
        .attr("id", "RCPT-WITH-RECIPIENT")
        .attr("type", "read")
        .attr("recipient", "242395589390497@lid")
        .build();
    let own_device_pn: Jid = "155500012345:48@s.whatsapp.net"
        .parse()
        .expect("own device PN JID should parse");

    let ack = build_ack_node(&incoming.as_node_ref(), Some(&own_device_pn))
        .expect("receipt ack should be buildable");

    assert!(ack.attrs.get("class").is_some_and(|v| v == "receipt"));
    assert!(
        ack.attrs
            .get("recipient")
            .is_some_and(|v| v == "242395589390497@lid"),
        "receipt ACK must echo the incoming `recipient` attribute"
    );
}

#[test]
fn test_build_ack_node_for_message_without_recipient_omits_recipient() {
    // Regression guard: never synthesise a `recipient` field if the
    // incoming stanza did not carry one — server would reject the ack.
    let incoming = NodeBuilder::new("message")
        .attr("from", "120363161500776365@g.us")
        .attr("id", "A5791A5392EF60E3FB06")
        .attr("type", "text")
        .attr("participant", "181531758878822@lid")
        .build();
    let own_device_pn: Jid = "155500012345:48@s.whatsapp.net"
        .parse()
        .expect("own device PN JID should parse");

    let ack = build_ack_node(&incoming.as_node_ref(), Some(&own_device_pn))
        .expect("message ack should be buildable");

    assert!(
        !ack.attrs.contains_key("recipient"),
        "ACK must NOT add `recipient` when the incoming stanza has none"
    );
}

#[test]
fn test_encode_ack_bytes_roundtrip_recipient() {
    // Exercises the real wire encoder (`encode_ack_bytes`), not just the
    // `build_ack_node` test mirror: serialize, decode the bytes back, and
    // assert the parsed ACK echoes `recipient` when present and omits it
    // when absent. Guards against the two builders silently diverging.
    let own_device_pn: Jid = "155500012345:48@s.whatsapp.net"
        .parse()
        .expect("own device PN JID should parse");

    let with_recipient = NodeBuilder::new("message")
        .attr("from", "166361967902821@lid")
        .attr("id", "2A32F960553696093D99")
        .attr("type", "text")
        .attr("recipient", "146991363395800@lid")
        .build();
    let buf = encode_ack_bytes(
        &with_recipient.as_node_ref(),
        Some(&own_device_pn),
        AckParticipantPolicy::Preserve,
    )
    .expect("encode_ack_bytes should produce bytes");
    let decoded =
        wacore_binary::marshal::unmarshal_packed_ref(&buf).expect("encoded ack should decode");
    assert_eq!(decoded.tag, "ack");
    assert!(
        decoded
            .get_attr("class")
            .is_some_and(|v| v.as_str() == "message"),
        "decoded ack must have class=message"
    );
    assert!(
        decoded
            .get_attr("recipient")
            .is_some_and(|v| v.as_str() == "146991363395800@lid"),
        "encode_ack_bytes must echo `recipient` onto the wire"
    );
    assert!(
        decoded
            .get_attr("type")
            .is_some_and(|value| value.as_str() == "text"),
        "generic message ACK must echo its explicit type"
    );

    let without_recipient = NodeBuilder::new("message")
        .attr("from", "120363161500776365@g.us")
        .attr("id", "A5791A5392EF60E3FB06")
        .attr("type", "text")
        .attr("participant", "181531758878822@lid")
        .build();
    let buf = encode_ack_bytes(
        &without_recipient.as_node_ref(),
        Some(&own_device_pn),
        AckParticipantPolicy::Preserve,
    )
    .expect("encode_ack_bytes should produce bytes");
    let decoded =
        wacore_binary::marshal::unmarshal_packed_ref(&buf).expect("encoded ack should decode");
    assert!(
        decoded.get_attr("recipient").is_none(),
        "encode_ack_bytes must not synthesise `recipient` when absent"
    );
}

#[test]
fn test_encode_ack_bytes_requires_public_response_inputs() {
    let without_id = NodeBuilder::new("receipt")
        .attr("from", "12025550111@s.whatsapp.net")
        .build();
    assert!(matches!(
        encode_ack_bytes(
            &without_id.as_node_ref(),
            None,
            AckParticipantPolicy::Preserve,
        ),
        Err(crate::features::StanzaResponseError::MissingAttribute("id"))
    ));

    let empty_id = NodeBuilder::new("receipt")
        .attr("id", "")
        .attr("from", "12025550111@s.whatsapp.net")
        .build();
    assert!(matches!(
        encode_ack_bytes(
            &empty_id.as_node_ref(),
            None,
            AckParticipantPolicy::Preserve,
        ),
        Err(crate::features::StanzaResponseError::MissingAttribute("id"))
    ));

    let without_from = NodeBuilder::new("receipt")
        .attr("id", "MISSING-FROM")
        .build();
    assert!(matches!(
        encode_ack_bytes(
            &without_from.as_node_ref(),
            None,
            AckParticipantPolicy::Preserve,
        ),
        Err(crate::features::StanzaResponseError::MissingAttribute(
            "from"
        ))
    ));

    let empty_from = NodeBuilder::new("receipt")
        .attr("id", "EMPTY-FROM")
        .attr("from", "")
        .build();
    assert!(matches!(
        encode_ack_bytes(
            &empty_from.as_node_ref(),
            None,
            AckParticipantPolicy::Preserve,
        ),
        Err(crate::features::StanzaResponseError::MissingAttribute(
            "from"
        ))
    ));

    let message = NodeBuilder::new("message")
        .attr("id", "MISSING-IDENTITY")
        .attr("from", "12025550111@s.whatsapp.net")
        .build();
    assert!(matches!(
        encode_ack_bytes(&message.as_node_ref(), None, AckParticipantPolicy::Preserve,),
        Err(crate::features::StanzaResponseError::MissingLocalIdentity)
    ));
}

#[test]
fn test_encode_ack_bytes_preserves_specialized_receipt_rules() {
    let from: Jid = "12025550111@s.whatsapp.net".parse().unwrap();
    let receipt = NodeBuilder::new("receipt")
        .attr("id", "RECEIPT-ACK")
        .attr("from", &from)
        .attr("participant", "12025550111@s.whatsapp.net")
        .attr("type", "retry")
        .build();
    let bytes = encode_ack_bytes(
        &receipt.as_node_ref(),
        None,
        AckParticipantPolicy::OmitReceiptDestinationDuplicate,
    )
    .expect("complete receipt should produce an ack");
    let ack = wacore_binary::marshal::unmarshal_packed_ref(&bytes)
        .expect("encoded receipt ack should decode");

    assert!(
        ack.get_attr("class")
            .is_some_and(|value| value.as_str() == "receipt")
    );
    assert!(
        ack.get_attr("type")
            .is_some_and(|value| value.as_str() == "retry")
    );
    assert!(
        ack.get_attr("participant").is_none(),
        "receipt ack must omit a participant that duplicates its destination"
    );
    assert!(ack.get_attr("from").is_none());

    let group_receipt = NodeBuilder::new("receipt")
        .attr("id", "GROUP-RECEIPT-ACK")
        .attr("from", "120363098765432100@g.us")
        .attr("participant", "12025550111:7@s.whatsapp.net")
        .build();
    let bytes = encode_ack_bytes(
        &group_receipt.as_node_ref(),
        None,
        AckParticipantPolicy::OmitReceiptDestinationDuplicate,
    )
    .expect("group receipt should produce an ack");
    let ack = wacore_binary::marshal::unmarshal_packed_ref(&bytes)
        .expect("encoded group receipt ack should decode");
    assert!(
        ack.get_attr("participant")
            .is_some_and(|value| value.as_str() == "12025550111:7@s.whatsapp.net"),
        "receipt ack must preserve a participant distinct from its destination"
    );

    let generic = NodeBuilder::new("message")
        .attr("id", "MESSAGE-ACK")
        .attr("from", "12025550111@s.whatsapp.net")
        .attr("participant", &from)
        .build();
    let bytes = encode_ack_bytes(
        &generic.as_node_ref(),
        Some(&from),
        AckParticipantPolicy::Preserve,
    )
    .expect("complete message should produce an ack");
    let ack = wacore_binary::marshal::unmarshal_packed_ref(&bytes)
        .expect("encoded message ack should decode");
    assert!(
        ack.get_attr("participant")
            .is_some_and(|value| value.as_str() == "12025550111@s.whatsapp.net"),
        "generic ack must not inherit the receipt-only participant rule"
    );
}

#[test]
fn test_encode_ack_bytes_compares_jid_participants_by_display() {
    let from = Jid {
        user: "12025550111".into(),
        server: wacore_binary::Server::Hosted,
        agent: 1,
        device: 7,
        integrator: 0,
    };
    let participant = Jid {
        agent: 2,
        ..from.clone()
    };
    assert_eq!(from.to_string(), participant.to_string());

    let receipt = NodeBuilder::new("receipt")
        .attr("id", "DISPLAY-EQUIVALENT-PARTICIPANT")
        .attr("from", &from)
        .attr("participant", &participant)
        .build();
    let bytes = encode_ack_bytes(
        &receipt.as_node_ref(),
        None,
        AckParticipantPolicy::OmitReceiptDestinationDuplicate,
    )
    .expect("complete receipt should produce an ack");
    let ack = wacore_binary::marshal::unmarshal_packed_ref(&bytes)
        .expect("encoded receipt ack should decode");

    assert!(
        ack.get_attr("participant").is_none(),
        "receipt ack must omit display-equivalent participant JIDs"
    );
}

#[test]
fn test_encode_ack_bytes_drops_encrypt_identity_notification_type() {
    let notification = NodeBuilder::new("notification")
        .attr("id", "IDENTITY-NOTIFICATION")
        .attr("from", "12025550111@s.whatsapp.net")
        .attr("type", "encrypt")
        .children([NodeBuilder::new("identity").build()])
        .build();
    let bytes = encode_ack_bytes(
        &notification.as_node_ref(),
        None,
        AckParticipantPolicy::Preserve,
    )
    .expect("complete notification should produce an ack");
    let ack = wacore_binary::marshal::unmarshal_packed_ref(&bytes)
        .expect("encoded notification ack should decode");

    assert!(
        ack.get_attr("class")
            .is_some_and(|value| value.as_str() == "notification")
    );
    assert!(ack.get_attr("type").is_none());
    assert!(ack.get_attr("from").is_none());
}

#[test]
fn test_encode_ack_bytes_preserves_call_class_and_type() {
    let call = NodeBuilder::new("call")
        .attr("id", "CALL-ACK")
        .attr("from", "12025550111@s.whatsapp.net")
        .attr("type", "offer_notice")
        .build();
    let bytes = encode_ack_bytes(&call.as_node_ref(), None, AckParticipantPolicy::Preserve)
        .expect("complete call should produce an ack");
    let ack = wacore_binary::marshal::unmarshal_packed_ref(&bytes)
        .expect("encoded call ack should decode");

    assert!(
        ack.get_attr("class")
            .is_some_and(|value| value.as_str() == "call")
    );
    assert!(
        ack.get_attr("type")
            .is_some_and(|value| value.as_str() == "offer_notice")
    );
    assert!(ack.get_attr("from").is_none());
}

/// Own-account fan-out ack must address back to the original `from` (own
/// LID) echoing `recipient`, not to the chat. Guards against regressing to
/// the chat-addressed `build_nack_node` style.
#[test]
fn test_message_ack_source_node_own_device_addressing() {
    use crate::types::message::{MessageInfo, MessageSource};
    // Own-account branch: sender == `from` (device-qualified), chat is the
    // device-stripped recipient. `to` must come from sender, not chat.
    let info = MessageInfo {
        id: "AC055553E56A2C12DE592DAD6353C477".to_string(),
        source: MessageSource {
            sender: "236395184570386@lid".parse().expect("sender"),
            chat: "156535032389744@lid".parse().expect("chat"),
            recipient: Some("156535032389744@lid".parse().expect("recipient")),
            is_group: false,
            ..Default::default()
        },
        ..Default::default()
    };
    let own_device_pn: Jid = "559984726662:95@s.whatsapp.net"
        .parse()
        .expect("own device PN JID should parse");

    let source = message_ack_source_node(&info);
    let built = build_ack_node(&source.as_node_ref(), Some(&own_device_pn))
        .expect("message ack should be buildable");

    assert!(built.attrs.get("class").is_some_and(|v| v == "message"));
    assert!(
        built
            .attrs
            .get("to")
            .is_some_and(|v| v == "236395184570386@lid"),
        "ack `to` must be the original `from` (own LID), not the chat"
    );
    assert!(
        built
            .attrs
            .get("recipient")
            .is_some_and(|v| v == "156535032389744@lid"),
        "ack must echo `recipient` so the server can route/clear it"
    );
    assert!(
        !built.attrs.contains_key("type"),
        "message-class acks never carry a `type`"
    );
}

/// Common incoming DM from another user: `to` is the device-qualified
/// sender, with no `recipient`/`participant` synthesised.
#[test]
fn test_message_ack_source_node_incoming_dm_addressing() {
    use crate::types::message::{MessageInfo, MessageSource};
    let info = MessageInfo {
        id: "MSGID".to_string(),
        source: MessageSource {
            sender: "5511999998888:3@s.whatsapp.net".parse().expect("sender"),
            chat: "5511999998888@s.whatsapp.net".parse().expect("chat"),
            is_group: false,
            ..Default::default()
        },
        ..Default::default()
    };
    let own_device_pn: Jid = "559984726662:95@s.whatsapp.net"
        .parse()
        .expect("own device PN JID should parse");

    let source = message_ack_source_node(&info);
    let built = build_ack_node(&source.as_node_ref(), Some(&own_device_pn))
        .expect("dm ack should be buildable");

    assert!(
        built
            .attrs
            .get("to")
            .is_some_and(|v| v == "5511999998888:3@s.whatsapp.net"),
        "ack `to` must be the device-qualified sender (the original `from`)"
    );
    assert!(!built.attrs.contains_key("recipient"));
    assert!(!built.attrs.contains_key("participant"));
}

/// status@broadcast (is_group=true in the parser) addresses the ack to the
/// status chat, with the sender as participant, not to the sender.
#[test]
fn test_message_ack_source_node_status_addressing() {
    use crate::types::message::{MessageInfo, MessageSource};
    let info = MessageInfo {
        id: "STATUSMSG".to_string(),
        source: MessageSource {
            chat: "status@broadcast".parse().expect("status chat"),
            sender: "181531758878822@lid".parse().expect("participant"),
            is_group: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let own_device_pn: Jid = "559984726662:95@s.whatsapp.net"
        .parse()
        .expect("own device PN JID should parse");

    let source = message_ack_source_node(&info);
    let built = build_ack_node(&source.as_node_ref(), Some(&own_device_pn))
        .expect("status ack should be buildable");

    assert!(
        built
            .attrs
            .get("to")
            .is_some_and(|v| v == "status@broadcast"),
        "status ack `to` must be the status chat, not the sender"
    );
    assert!(
        built
            .attrs
            .get("participant")
            .is_some_and(|v| v == "181531758878822@lid"),
        "status ack must preserve the sending participant"
    );
}

/// Group failure ack: `to` is the group, `participant` is preserved.
#[test]
fn test_message_ack_source_node_group_addressing() {
    use crate::types::message::{MessageInfo, MessageSource};
    // Group branch: chat == group `from`, sender == participant.
    let info = MessageInfo {
        id: "GROUPMSGID".to_string(),
        source: MessageSource {
            chat: "120363011111111111@g.us".parse().expect("group"),
            sender: "181531758878822@lid".parse().expect("participant"),
            is_group: true,
            ..Default::default()
        },
        ..Default::default()
    };
    let own_device_pn: Jid = "559984726662:95@s.whatsapp.net"
        .parse()
        .expect("own device PN JID should parse");

    let source = message_ack_source_node(&info);
    let built = build_ack_node(&source.as_node_ref(), Some(&own_device_pn))
        .expect("group message ack should be buildable");

    assert!(
        built
            .attrs
            .get("to")
            .is_some_and(|v| v == "120363011111111111@g.us"),
        "group ack `to` must be the group JID"
    );
    assert!(
        built
            .attrs
            .get("participant")
            .is_some_and(|v| v == "181531758878822@lid"),
        "group ack must preserve the sending `participant`"
    );
}

/// Smoke test: server ping with xmlns but no id attribute is handled.
#[tokio::test]
async fn test_handle_iq_ping_without_id() {
    let backend = crate::test_utils::create_test_backend().await;
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("persistence manager should initialize"),
    );
    let (client, _rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        Arc::new(crate::transport::mock::MockTransportFactory::new()),
        Arc::new(MockHttpClient),
        None,
    )
    .await;

    // Server ping without id — real format observed in production logs
    let ping_node = NodeBuilder::new("iq")
        .attr("type", "get")
        .attr("from", SERVER_JID)
        .attr("xmlns", "urn:xmpp:ping")
        .build();

    let handled = client.handle_iq(&ping_node.as_node_ref()).await;
    assert!(
        handled,
        "handle_iq must recognize ping without id attribute"
    );
}

// ── fibonacci_backoff tests ────────────────────────────────────────

#[test]
fn test_fibonacci_backoff_sequence() {
    // WA Web: first=1000, second=1000 → 1,1,2,3,5,8,13,21,34,55,89,144...s
    // We test base values without jitter by checking the range (±10%).
    let expected_base_ms = [1000, 1000, 2000, 3000, 5000, 8000, 13000, 21000];
    for (attempt, &base) in expected_base_ms.iter().enumerate() {
        let delay = fibonacci_backoff(attempt as u32);
        let ms = delay.as_millis() as u64;
        let low = base - base / 10;
        let high = base + base / 10;
        assert!(
            ms >= low && ms <= high,
            "attempt {attempt}: expected {low}..={high}ms, got {ms}ms"
        );
    }
}

#[test]
fn test_fibonacci_backoff_max_900s() {
    // After many attempts, should cap at 900s (±10%)
    let delay = fibonacci_backoff(100);
    let ms = delay.as_millis() as u64;
    assert!(
        ms <= 990_000,
        "should never exceed 900s + 10% jitter, got {ms}ms"
    );
    assert!(
        ms >= 810_000,
        "should be at least 900s - 10% jitter, got {ms}ms"
    );
}

#[test]
fn test_fibonacci_backoff_first_attempt_is_1s() {
    let delay = fibonacci_backoff(0);
    let ms = delay.as_millis() as u64;
    assert!(
        (900..=1100).contains(&ms),
        "first attempt should be ~1s (±10%), got {ms}ms"
    );
}

// ── connection stability / backoff reset (WA Web resetDelay) ────────

#[test]
fn should_reset_backoff_requires_uptime_window_and_no_penalty() {
    let start = 1_000_000i64;
    let stable = start + Client::STABLE_CONNECTION_RESET_MS;
    // Never authenticated this cycle → not stable, whatever the clock says.
    assert!(!should_reset_backoff(0, 1_000_000, false));
    // Authenticated but dropped inside the 30s window → keep escalating.
    assert!(!should_reset_backoff(
        start,
        start + Client::STABLE_CONNECTION_RESET_MS - 1,
        false
    ));
    // Survived the full window with no penalty → reset the backoff.
    assert!(should_reset_backoff(start, stable, false));
    assert!(should_reset_backoff(start, start + 60_000, false));
    // An explicit penalty (429 / manual reconnect) survives even a stable
    // connection (WA Web cancelReset).
    assert!(!should_reset_backoff(start, stable, true));
    assert!(!should_reset_backoff(start, start + 60_000, true));
    // A backwards clock jump must not underflow into a spurious reset.
    assert!(!should_reset_backoff(start, start - 5_000, false));
}

// ── stream error tests ─────────────────────────────────────────────

#[tokio::test]
async fn test_stream_error_401_disables_reconnect() {
    let client = create_offline_sync_test_client().await;
    let node = NodeBuilder::new("stream:error").attr("code", "401").build();
    client.handle_stream_error(&node.as_node_ref()).await;
    assert!(
        !client.enable_auto_reconnect.load(Ordering::Relaxed),
        "401 should disable auto-reconnect"
    );
}

#[tokio::test]
async fn test_stream_error_409_disables_reconnect() {
    let client = create_offline_sync_test_client().await;
    let node = NodeBuilder::new("stream:error").attr("code", "409").build();
    client.handle_stream_error(&node.as_node_ref()).await;
    assert!(
        !client.enable_auto_reconnect.load(Ordering::Relaxed),
        "409 should disable auto-reconnect"
    );
}

#[tokio::test]
async fn test_stream_error_429_keeps_reconnect_with_backoff() {
    let client = create_offline_sync_test_client().await;
    client.is_logged_in.store(true, Ordering::Relaxed);
    let before = client.auto_reconnect_errors.load(Ordering::Relaxed);
    let node = NodeBuilder::new("stream:error").attr("code", "429").build();
    client.handle_stream_error(&node.as_node_ref()).await;
    assert!(
        client.enable_auto_reconnect.load(Ordering::Relaxed),
        "429 should keep auto-reconnect enabled"
    );
    assert!(
        !client.is_logged_in.load(Ordering::Relaxed),
        "429 must clear is_logged_in so sends bail before the server flags abuse"
    );
    assert!(
        !client.expected_disconnect.load(Ordering::Relaxed),
        "429 must not mark the disconnect as expected (auto-reconnect path)"
    );
    let after = client.auto_reconnect_errors.load(Ordering::Relaxed);
    assert_eq!(
        after,
        before + 5,
        "429 should increase backoff by exactly 5: before={before}, after={after}"
    );
}

/// A rate-limited session parks the client for minutes; without an event the
/// only trace is the missing connection. WA Web has no 429 arm to copy here
/// (only 500..600 is special-cased), so this is our own `StreamError` contract
/// applied consistently, and the neighbours must keep their own events.
#[tokio::test]
async fn test_stream_error_429_dispatches_stream_error_event() {
    use wacore::types::events::{Event, EventHandler};

    let client = create_offline_sync_test_client().await;
    client.is_logged_in.store(true, Ordering::Relaxed);
    let collector = Arc::new(crate::test_utils::TestEventCollector::default());
    client
        .subscribe_handler(collector.clone() as Arc<dyn EventHandler>)
        .detach();

    let node = NodeBuilder::new("stream:error")
        .attr("code", "429")
        .children([NodeBuilder::new("text")
            .attr("text", "rate-overlimit")
            .build()])
        .build();
    client.handle_stream_error(&node.as_node_ref()).await;

    let events = collector.events();
    let stream_errors: Vec<_> = events
        .iter()
        .filter_map(|event| match &**event {
            Event::StreamError(stream_error) => Some(stream_error),
            _ => None,
        })
        .collect();
    assert_eq!(
        stream_errors.len(),
        1,
        "429 must dispatch exactly one StreamError, got {events:?}"
    );
    assert_eq!(stream_errors[0].code, "429");
    let raw = stream_errors[0]
        .raw
        .as_ref()
        .expect("the stanza must ride along so the reason survives");
    assert_eq!(raw.tag, "stream:error");
    assert!(
        raw.get_optional_child("text").is_some(),
        "the raw stanza must keep the server's children, not just the code"
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(**event, Event::LoggedOut(_) | Event::StreamReplaced(_))),
        "429 is not a logout or a replacement"
    );
}

/// The branches either side of 429 keep dispatching what they always did — a
/// regression here would look like the 429 event working while 516/409 lost
/// theirs.
#[tokio::test]
async fn test_stream_error_neighbours_keep_their_events() {
    use wacore::types::events::{Event, EventHandler};

    for (code, expect_logged_out) in [("516", true), ("401", true), ("409", false)] {
        let client = create_offline_sync_test_client().await;
        let collector = Arc::new(crate::test_utils::TestEventCollector::default());
        client
            .subscribe_handler(collector.clone() as Arc<dyn EventHandler>)
            .detach();

        let node = NodeBuilder::new("stream:error").attr("code", code).build();
        client.handle_stream_error(&node.as_node_ref()).await;

        let events = collector.events();
        let matched = events.iter().any(|event| {
            if expect_logged_out {
                matches!(**event, Event::LoggedOut(_))
            } else {
                matches!(**event, Event::StreamReplaced(_))
            }
        });
        assert!(matched, "{code} lost its event, got {events:?}");
        assert!(
            !events
                .iter()
                .any(|event| matches!(**event, Event::StreamError(_))),
            "{code} must not also report as a generic StreamError"
        );
    }
}

#[tokio::test]
async fn test_stream_error_503_keeps_reconnect() {
    let client = create_offline_sync_test_client().await;
    client.is_logged_in.store(true, Ordering::Relaxed);
    let node = NodeBuilder::new("stream:error").attr("code", "503").build();
    client.handle_stream_error(&node.as_node_ref()).await;
    assert!(
        client.enable_auto_reconnect.load(Ordering::Relaxed),
        "503 should keep auto-reconnect enabled"
    );
    assert!(
        !client.is_logged_in.load(Ordering::Relaxed),
        "503 must clear is_logged_in so sends bail against the dying socket"
    );
    assert!(
        !client.expected_disconnect.load(Ordering::Relaxed),
        "503 must not mark the disconnect as expected (auto-reconnect path)"
    );
}

#[tokio::test]
async fn test_stream_error_unknown_keeps_connection_alive() {
    // Unknown stream:error (no `code` attribute) must mirror whatsmeow's
    // default branch: log + dispatch event, but NOT mark this as an
    // expected disconnect. Setting that flag silently swallows the next
    // real disconnect and races the read loop into shutdown.
    let client = create_offline_sync_test_client().await;
    // Simulate an authenticated session before the stream error arrives.
    client.is_logged_in.store(true, Ordering::Relaxed);
    let node = NodeBuilder::new("stream:error").build();
    client.handle_stream_error(&node.as_node_ref()).await;
    assert!(
        client.is_logged_in.load(Ordering::Relaxed),
        "unknown stream:error must NOT log the client out"
    );
    assert!(
        !client.expected_disconnect.load(Ordering::Relaxed),
        "unknown stream:error must not mark the disconnect as expected"
    );
    assert!(
        client.enable_auto_reconnect.load(Ordering::Relaxed),
        "unknown stream:error must keep auto-reconnect enabled"
    );
}

#[tokio::test]
async fn test_stream_error_ack_shaped_does_not_force_shutdown() {
    // Server wraps per-stanza routing failures in `<stream:error><ack/>`
    // with no `code` attribute. Treat as informational, not as a fatal
    // stream teardown.
    let client = create_offline_sync_test_client().await;
    client.is_logged_in.store(true, Ordering::Relaxed);
    let ack_child = NodeBuilder::new("ack")
        .attr("class", "message")
        .attr("type", "text")
        .attr("id", "2A32F960553696093D99")
        .build();
    let node = NodeBuilder::new("stream:error")
        .children([ack_child])
        .build();
    client.handle_stream_error(&node.as_node_ref()).await;
    assert!(
        client.is_logged_in.load(Ordering::Relaxed),
        "ack-shaped stream:error must NOT log the client out"
    );
    assert!(
        !client.expected_disconnect.load(Ordering::Relaxed),
        "ack-shaped stream:error must not mark the disconnect as expected"
    );
}

#[tokio::test]
async fn test_custom_cache_config_is_respected() {
    use crate::cache_config::{CacheConfig, CacheEntryConfig};
    use std::time::Duration;

    let backend = crate::test_utils::create_test_backend().await;
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("persistence manager should initialize"),
    );

    let custom_config = CacheConfig {
        group_cache: CacheEntryConfig::new(Some(Duration::from_secs(60)), 10),
        device_registry_cache: CacheEntryConfig::new(Some(Duration::from_secs(60)), 10),
        ..CacheConfig::default()
    };

    // Verify that constructing a client with a custom config does not panic
    // and the client is usable.
    let (client, _rx) = Client::new_with_cache_config(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        Arc::new(crate::transport::mock::MockTransportFactory::new()),
        Arc::new(MockHttpClient),
        None,
        custom_config,
    )
    .await;

    assert!(!client.is_logged_in());
}

#[tokio::test]
async fn held_group_distribution_lane_survives_capacity_pressure() {
    let config = CacheConfig {
        group_distribution_locks_capacity: 1,
        ..Default::default()
    };
    let client = crate::test_utils::create_test_client_with_config(
        "group_distribution_eviction",
        Arc::new(MockHttpClient),
        config,
    )
    .await;

    let first: Jid = "120363000000000011@g.us".parse().unwrap();
    let second: Jid = "120363000000000012@g.us".parse().unwrap();
    let third: Jid = "120363000000000013@g.us".parse().unwrap();
    let held = client.group_distribution_lock(&first).await;

    drop(client.group_distribution_lock(&second).await);
    drop(client.group_distribution_lock(&third).await);

    let first_again = client
        .group_distribution_locks
        .get(&first)
        .await
        .expect("held lane must remain cached");
    assert!(
        first_again.try_lock().is_none(),
        "capacity pressure must not mint a second live lane"
    );
    let report = client.memory_report().await;
    assert_eq!(report.group_distribution_locks, 2);
    assert_eq!(report.group_distribution_lock_evictions, 1);
    assert_eq!(report.group_distribution_lock_eviction_blocks, 2);
    drop(held);
    assert!(first_again.try_lock().is_some());
}

#[tokio::test]
async fn active_chat_lane_survives_capacity_pressure() {
    fn test_lane() -> (ChatLane, async_channel::Receiver<QueuedChatMessage>) {
        let (queue_tx, queue_rx) = async_channel::unbounded();
        (
            ChatLane {
                enqueue_lock: Arc::new(Mutex::new(())),
                queue_tx,
            },
            queue_rx,
        )
    }

    let config = CacheConfig {
        chat_lanes_capacity: 1,
        ..Default::default()
    };
    let client = crate::test_utils::create_test_client_with_config(
        "chat_lane_eviction",
        Arc::new(MockHttpClient),
        config,
    )
    .await;

    let first: Jid = "120363000000000021@g.us".parse().unwrap();
    let second: Jid = "120363000000000022@g.us".parse().unwrap();
    let (first_lane, first_rx) = test_lane();
    let first_tx_probe = first_lane.queue_tx.clone();
    client.chat_lanes.insert(first.clone(), first_lane).await;

    let first_lane = client.chat_lanes.get(&first).await.unwrap();
    let node = NodeBuilder::new("message")
        .attr("from", first.clone())
        .attr("id", "ACTIVE-LANE-1")
        .build();
    first_lane.try_enqueue(node_to_owned_ref(node)).unwrap();
    drop(first_lane);
    let active_message = first_rx.recv().await.unwrap();

    let (second_lane, _second_rx) = test_lane();
    client.chat_lanes.insert(second, second_lane).await;

    let first_again = client
        .chat_lanes
        .get(&first)
        .await
        .expect("an active lane must remain cached");
    assert!(first_again.queue_tx.same_channel(&first_tx_probe));

    let next_node = NodeBuilder::new("message")
        .attr("from", first.clone())
        .attr("id", "ACTIVE-LANE-2")
        .build();
    first_again
        .try_enqueue(node_to_owned_ref(next_node))
        .unwrap();
    drop(first_again);
    drop(active_message);
    let next_active_message = first_rx.recv().await.unwrap();

    let third: Jid = "120363000000000023@g.us".parse().unwrap();
    let (third_lane, _third_rx) = test_lane();
    client.chat_lanes.insert(third, third_lane).await;
    assert!(
        client.chat_lanes.get(&first).await.is_some(),
        "a lane with an in-flight message must not be evicted"
    );

    drop(next_active_message);
    let fourth: Jid = "120363000000000024@g.us".parse().unwrap();
    let (fourth_lane, _fourth_rx) = test_lane();
    client.chat_lanes.insert(fourth, fourth_lane).await;
    assert!(
        client.chat_lanes.get(&first).await.is_none(),
        "an idle lane must become evictable again"
    );
}

/// Proves that `is_connected()` no longer gives false negatives under mutex
/// contention. Before the fix, `try_lock()` would fail when another task held
/// the noise_socket mutex, causing `is_connected()` to return `false` even
/// though the connection was alive — silently dropping receipt acks.
///
/// This test sets up a real NoiseSocket (same as socket unit tests) so it
/// accurately models the pre-fix scenario: socket is Some + mutex is held
/// by another task = old is_connected() returned false.
#[tokio::test]
async fn test_is_connected_not_affected_by_mutex_contention() {
    use crate::socket::NoiseSocket;
    use wacore::handshake::NoiseCipher;

    let backend = crate::test_utils::create_test_backend().await;
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("persistence manager should initialize"),
    );
    let (client, _rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        Arc::new(crate::transport::mock::MockTransportFactory::new()),
        Arc::new(MockHttpClient),
        None,
    )
    .await;

    // Initially not connected
    assert!(!client.is_connected(), "should start disconnected");

    // Simulate a real connection: create a NoiseSocket and store it
    let transport: Arc<dyn crate::transport::Transport> =
        Arc::new(crate::transport::mock::MockTransport);
    let key = [0u8; 32];
    let write_key = NoiseCipher::new(&key).expect("valid key");
    let read_key = NoiseCipher::new(&key).expect("valid key");
    let noise_socket = NoiseSocket::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        transport,
        write_key,
        read_key,
    );
    *client.noise_socket.lock().unwrap() = Some(Arc::new(noise_socket));
    client.is_connected.store(true, Ordering::Release);

    assert!(client.is_connected(), "should report connected");

    // Hold the noise_socket mutex — this used to make is_connected() return
    // false via try_lock() even though the socket was Some(...)
    let _guard = client.noise_socket.lock().unwrap();
    assert!(
        client.is_connected(),
        "is_connected() must return true even while noise_socket mutex is held"
    );
}

#[tokio::test]
async fn disconnect_does_not_signal_connection_cleanup_before_outbound_flush() {
    use crate::socket::NoiseSocket;
    use async_trait::async_trait;
    use bytes::Bytes;
    use wacore::handshake::NoiseCipher;

    struct BlockingTransport {
        send_started: async_channel::Sender<()>,
        release_send: async_channel::Receiver<()>,
        send_done: Arc<AtomicBool>,
        disconnect_called: Arc<AtomicBool>,
        disconnect_before_send_done: Arc<AtomicBool>,
    }

    #[async_trait]
    impl crate::transport::Transport for BlockingTransport {
        async fn send(&self, _data: Bytes) -> Result<(), anyhow::Error> {
            let _ = self.send_started.try_send(());
            let _ = self.release_send.recv().await;
            self.send_done.store(true, Ordering::Release);
            Ok(())
        }

        async fn disconnect(&self) {
            if !self.send_done.load(Ordering::Acquire) {
                self.disconnect_before_send_done
                    .store(true, Ordering::Release);
            }
            self.disconnect_called.store(true, Ordering::Release);
        }
    }

    let client = crate::test_utils::create_test_client().await;
    let (send_started_tx, send_started_rx) = async_channel::bounded(1);
    let (release_send_tx, release_send_rx) = async_channel::bounded(1);
    let send_done = Arc::new(AtomicBool::new(false));
    let disconnect_called = Arc::new(AtomicBool::new(false));
    let disconnect_before_send_done = Arc::new(AtomicBool::new(false));

    let transport_impl = Arc::new(BlockingTransport {
        send_started: send_started_tx,
        release_send: release_send_rx,
        send_done: Arc::clone(&send_done),
        disconnect_called: Arc::clone(&disconnect_called),
        disconnect_before_send_done: Arc::clone(&disconnect_before_send_done),
    });
    let transport: Arc<dyn crate::transport::Transport> = transport_impl;

    let key = [0u8; 32];
    let write_key = NoiseCipher::new(&key).expect("valid key");
    let read_key = NoiseCipher::new(&key).expect("valid key");
    let noise_socket = NoiseSocket::new(
        client.runtime.clone(),
        Arc::clone(&transport),
        write_key,
        read_key,
    );

    *client.transport.lock().await = Some(transport);
    *client.noise_socket.lock().unwrap() = Some(Arc::new(noise_socket));
    client.is_connected.store(true, Ordering::Release);

    let cleanup_signal = client.connection_shutdown_signal();
    let cleanup_client = Arc::clone(&client);
    let cleanup_task = tokio::spawn(async move {
        wacore::runtime::wait_for_shutdown(&cleanup_signal).await;
        cleanup_client.cleanup_connection_state().await;
    });

    let send_client = Arc::clone(&client);
    client.outbound_flush.spawn(&*client.runtime, async move {
        let receipt = NodeBuilder::new("receipt")
            .attr("id", "TEST-FLUSH-ORDER")
            .attr("to", "1234567890@s.whatsapp.net")
            .build();
        let _ = send_client.send_node(receipt).await;
    });

    tokio::time::timeout(Duration::from_secs(1), send_started_rx.recv())
        .await
        .expect("tracked send should start")
        .expect("send_started sender should stay open");

    let disconnect_client = Arc::clone(&client);
    let disconnect_task = tokio::spawn(async move {
        disconnect_client.disconnect().await;
    });

    // disconnect() closes the scope and then parks in `outbound_flush.flush`; the
    // parked flusher is what proves it got that far and is stuck there.
    crate::test_utils::poll_until("disconnect to park on the outbound flush", || {
        client.outbound_flush.flush_waiters() >= 1
    })
    .await;
    assert!(
        !client.connection_shutdown_signal().is_fired(),
        "connection cleanup must not fire while outbound flush is blocked"
    );
    assert!(
        !disconnect_called.load(Ordering::Acquire),
        "transport must stay open while outbound flush is blocked"
    );

    release_send_tx
        .send(())
        .await
        .expect("blocked send should still be waiting");

    tokio::time::timeout(Duration::from_secs(1), disconnect_task)
        .await
        .expect("disconnect should finish")
        .expect("disconnect task should not panic");
    tokio::time::timeout(Duration::from_secs(1), cleanup_task)
        .await
        .expect("cleanup should finish")
        .expect("cleanup task should not panic");

    assert!(send_done.load(Ordering::Acquire));
    assert!(disconnect_called.load(Ordering::Acquire));
    assert!(
        !disconnect_before_send_done.load(Ordering::Acquire),
        "cleanup closed the transport before the tracked send completed"
    );
}

async fn install_test_noise_socket(
    client: &Arc<Client>,
    transport: Arc<dyn crate::transport::Transport>,
    runtime: Arc<dyn Runtime>,
) {
    use crate::socket::NoiseSocket;
    use wacore::handshake::NoiseCipher;

    let key = TEST_NOISE_KEY;
    // Wired to the client's own observers, like the real socket: a test that
    // watches its sends needs the same plumbing production has.
    let noise_socket = NoiseSocket::with_observers(
        runtime,
        transport,
        NoiseCipher::new(&key).expect("valid key"),
        NoiseCipher::new(&key).expect("valid key"),
        crate::socket::noise_socket::SendObservers::with_stats(client.stats.clone())
            .with_sent_frames(client.sent_frame_tap.clone()),
    );
    *client.noise_socket.lock().unwrap() = Some(Arc::new(noise_socket));
    client.set_connected_for_test(true);
}

/// The key `install_test_noise_socket` builds its socket with, so a test can
/// decrypt what the client wrote.
const TEST_NOISE_KEY: [u8; 32] = [0u8; 32];

/// Every distinct way a stanza leaves the client, driven end to end, with the
/// observer's view compared against the transport's.
///
/// `send_node` marshals and resolves sent-node waiters; the ack, receipt and
/// direct-encoded IQ paths hand pre-marshaled bytes straight to the socket and
/// were invisible to those waiters; the ack and receipt workers reach the wire
/// only through the burst. All of them cross the noise sender, which is why one
/// observation point covers the lot.
#[tokio::test]
async fn every_send_path_is_observed_exactly_as_it_reached_the_wire() {
    use crate::transport::mock::CapturingMockTransport;

    let client = crate::test_utils::create_test_client().await;
    let transport = Arc::new(CapturingMockTransport::new());
    install_test_noise_socket(
        &client,
        transport.clone(),
        Arc::new(crate::runtime_impl::TokioRuntime),
    )
    .await;

    let recorder = Arc::new(crate::test_utils::SentFrameRecorder::default());
    let _subscription = client.subscribe_handler(recorder.clone());
    let _lease = client.acquire_sent_frame_forwarding();

    // 1. send_node: the only path that builds a Node the caller could inspect.
    let presence = NodeBuilder::new("presence")
        .attr("type", "available")
        .attr("name", "observer")
        .build();
    client
        .send_node(presence.clone())
        .await
        .expect("presence must send");

    // 2. send_raw_bytes, through a real caller of it: the ack path documents
    //    that it bypasses node logging and sent-node waiters.
    let incoming = NodeBuilder::new("notification")
        .attr("id", "OBSERVED-1")
        .attr("type", "w:gp2")
        .attr("from", "5550000@g.us")
        .build();
    client
        .send_ack_for(&incoming.as_node_ref())
        .await
        .expect("ack must send");

    // 3. send_raw_bytes_burst, the shape the ack and receipt workers use: two
    //    frames coalesced into a single transport write.
    let mut frames = vec![
        wacore_binary::marshal::marshal_exact(
            &NodeBuilder::new("iq").attr("id", "BURST-1").build(),
        )
        .expect("marshal"),
        wacore_binary::marshal::marshal_exact(
            &NodeBuilder::new("iq").attr("id", "BURST-2").build(),
        )
        .expect("marshal"),
    ];
    let mut results = Vec::new();
    client
        .send_raw_bytes_burst(&mut frames, &mut results)
        .await
        .expect("burst must send");
    assert!(results.iter().all(|result| result.is_ok()));

    let wire = crate::test_utils::decrypt_wire_frames(&transport.sent(), &TEST_NOISE_KEY);
    assert_eq!(wire.len(), 4, "four frames must have reached the transport");
    let observed: Vec<Vec<u8>> = recorder
        .frames()
        .iter()
        .map(|frame| frame.to_vec())
        .collect();
    assert_eq!(
        observed, wire,
        "every send path must be observed, byte for byte and in wire order"
    );

    // The bytes are the stanza, not a rendering of it: the first frame decodes
    // back to the node that was sent.
    let decoded = wacore_binary::marshal::unmarshal_packed_ref(&observed[0])
        .expect("an observed frame must decode as the stanza it carried");
    assert_eq!(decoded.tag.as_ref(), "presence");
    assert_eq!(
        decoded.attrs().optional_string("name").as_deref(),
        Some("observer")
    );
}

/// While nobody holds a lease the send path publishes nothing and builds
/// nothing, and releasing the last lease puts it back to that state.
#[tokio::test]
async fn sends_are_unobserved_until_a_consumer_asks() {
    use crate::transport::mock::CapturingMockTransport;

    let client = crate::test_utils::create_test_client().await;
    let transport = Arc::new(CapturingMockTransport::new());
    install_test_noise_socket(
        &client,
        transport.clone(),
        Arc::new(crate::runtime_impl::TokioRuntime),
    )
    .await;

    let recorder = Arc::new(crate::test_utils::SentFrameRecorder::default());
    let _subscription = client.subscribe_handler(recorder.clone());

    assert!(
        !client.sent_frame_forwarding_enabled(),
        "forwarding must be off until a consumer acquires it"
    );
    client
        .send_node(NodeBuilder::new("presence").build())
        .await
        .expect("presence must send");
    assert_eq!(client.sent_frame_tap.published(), 0);
    assert!(recorder.frames().is_empty());

    let lease = client.acquire_sent_frame_forwarding();
    assert!(client.sent_frame_forwarding_enabled());
    client
        .send_node(NodeBuilder::new("presence").build())
        .await
        .expect("presence must send");
    assert_eq!(recorder.frames().len(), 1);

    drop(lease);
    assert!(
        !client.sent_frame_forwarding_enabled(),
        "the last lease dropping must disable forwarding again"
    );
    client
        .send_node(NodeBuilder::new("presence").build())
        .await
        .expect("presence must send");
    assert_eq!(
        recorder.frames().len(),
        1,
        "no frame may be observed after the lease is gone"
    );
    assert_eq!(client.sent_frame_tap.published(), 1);
}

/// An observer that panics must not cost the client its send path: the dispatch
/// runs on the noise sender task, and an unwinding panic there would end every
/// send on the connection.
#[tokio::test]
async fn a_panicking_observer_leaves_the_client_sending() {
    use crate::transport::mock::CapturingMockTransport;

    struct PanickingObserver;
    impl wacore::types::events::EventHandler for PanickingObserver {
        fn handle_event(&self, _event: Arc<Event>) {
            panic!("observer panics on every frame");
        }
        fn interest(&self) -> wacore::types::events::EventInterest {
            wacore::types::events::EventInterest::of(&[wacore::types::events::EventKind::SentFrame])
        }
    }

    let client = crate::test_utils::create_test_client().await;
    let transport = Arc::new(CapturingMockTransport::new());
    install_test_noise_socket(
        &client,
        transport.clone(),
        Arc::new(crate::runtime_impl::TokioRuntime),
    )
    .await;

    let _subscription = client.subscribe_handler(Arc::new(PanickingObserver));
    let _lease = client.acquire_sent_frame_forwarding();

    for attempt in 0..3 {
        client
            .send_node(NodeBuilder::new("presence").attr("t", attempt).build())
            .await
            .unwrap_or_else(|e| panic!("send {attempt} must survive the observer: {e:?}"));
    }
    assert_eq!(
        transport.sent_count(),
        3,
        "every stanza must still reach the wire"
    );
}

fn receipt_test_info(id: &str) -> Arc<crate::types::message::MessageInfo> {
    Arc::new(crate::types::message::MessageInfo {
        id: id.to_string(),
        source: crate::types::message::MessageSource {
            chat: "15550001111@s.whatsapp.net".parse().unwrap(),
            sender: "15550001111@s.whatsapp.net".parse().unwrap(),
            ..Default::default()
        },
        ..Default::default()
    })
}

#[derive(Debug)]
struct DropSpawnRuntime;

#[async_trait::async_trait]
impl Runtime for DropSpawnRuntime {
    fn spawn(
        &self,
        _future: std::pin::Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
    ) -> wacore::runtime::AbortHandle {
        // Dropping the sender future closes its receiver synchronously.
        wacore::runtime::AbortHandle::noop()
    }

    fn sleep(&self, _duration: Duration) -> std::pin::Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(async {})
    }

    fn spawn_blocking(
        &self,
        operation: Box<dyn FnOnce() + Send + 'static>,
    ) -> std::pin::Pin<Box<dyn Future<Output = ()> + Send>> {
        Box::pin(async move { operation() })
    }

    fn yield_now(&self) -> Option<std::pin::Pin<Box<dyn Future<Output = ()> + Send>>> {
        None
    }
}

#[tokio::test]
async fn raw_bytes_burst_drains_and_reuses_input_on_happy_paths() {
    use crate::transport::mock::CapturingMockTransport;

    let client = crate::test_utils::create_test_client().await;
    let transport = Arc::new(CapturingMockTransport::new());
    install_test_noise_socket(
        &client,
        transport.clone(),
        Arc::new(crate::runtime_impl::TokioRuntime),
    )
    .await;

    let mut frames = Vec::with_capacity(4);
    let retained_capacity = frames.capacity();
    frames.push(vec![0x11; 32]);
    // Sized for the largest burst below, so a reallocation here would mean the
    // callee replaced the buffer rather than filling it.
    let mut results = Vec::with_capacity(4);
    // Captured before the first call, not between the two: taken after it, a
    // replacement made on the single-frame path would already be the buffer
    // this compares against and would go unnoticed.
    let results_ptr = results.as_ptr();
    client
        .send_raw_bytes_burst(&mut frames, &mut results)
        .await
        .expect("installed socket");
    assert_eq!(results.len(), 1);
    assert!(results.iter().all(|result| result.is_ok()));
    assert_eq!(
        results.as_ptr(),
        results_ptr,
        "the single-frame path must fill the caller's buffer, not replace it"
    );
    assert!(frames.is_empty(), "the single-frame fast path must drain");
    assert_eq!(frames.capacity(), retained_capacity);

    frames.extend((0..4).map(|index| vec![index; 32]));
    client
        .send_raw_bytes_burst(&mut frames, &mut results)
        .await
        .expect("installed socket");
    assert_eq!(results.len(), 4);
    assert!(results.iter().all(|result| result.is_ok()));
    // Identity, not capacity: a fresh Vec of the same capacity would satisfy a
    // capacity check while defeating the whole point of the out-parameter. The
    // buffer is preallocated above so the second burst cannot legitimately
    // reallocate it.
    assert_eq!(
        results.as_ptr(),
        results_ptr,
        "the caller's results buffer must be the same allocation, not an equal one"
    );
    assert!(frames.is_empty(), "the joined path must drain");
    assert_eq!(frames.capacity(), retained_capacity);
    assert_eq!(transport.sent_count(), 5, "every frame must reach the wire");
    assert_eq!(
        transport.write_count(),
        2,
        "the four-frame call must remain one coalesced transport write"
    );
}

#[tokio::test]
async fn raw_bytes_burst_drains_input_when_disconnected() {
    let client = crate::test_utils::create_test_client().await;
    let mut frames = Vec::with_capacity(4);
    let retained_capacity = frames.capacity();
    frames.extend([vec![0x21; 32], vec![0x22; 32]]);

    let mut results = Vec::new();
    let result = client.send_raw_bytes_burst(&mut frames, &mut results).await;
    assert!(
        matches!(result, Err(ClientError::NotConnected)),
        "a missing socket must remain an outer NotConnected error: {result:?}"
    );
    assert!(frames.is_empty(), "the outer-error path must also drain");
    assert_eq!(frames.capacity(), retained_capacity);
}

#[tokio::test]
async fn raw_bytes_burst_surfaces_transport_then_poisoned_per_frame() {
    use crate::socket::error::EncryptSendErrorKind;
    use crate::transport::mock::CapturingMockTransport;

    let client = crate::test_utils::create_test_client().await;
    let transport = Arc::new(CapturingMockTransport::new());
    transport.fail_next_sends(1);
    install_test_noise_socket(
        &client,
        transport.clone(),
        Arc::new(crate::runtime_impl::TokioRuntime),
    )
    .await;

    let mut frames = Vec::with_capacity(4);
    let retained_capacity = frames.capacity();
    frames.push(vec![0x31; 32]);
    let mut results = Vec::new();
    client
        .send_raw_bytes_burst(&mut frames, &mut results)
        .await
        .expect("the socket lookup itself succeeds");
    let transport_error = results
        .pop()
        .expect("one result")
        .expect_err("the transport is configured to fail");
    assert!(matches!(
        transport_error.kind,
        EncryptSendErrorKind::Transport
    ));
    assert!(transport_error.is_transport_unavailable());
    assert!(frames.is_empty());
    assert_eq!(frames.capacity(), retained_capacity);

    frames.push(vec![0x32; 32]);
    client
        .send_raw_bytes_burst(&mut frames, &mut results)
        .await
        .expect("the installed socket remains reachable");
    let poisoned_error = results
        .pop()
        .expect("one result")
        .expect_err("the sender must reject work after an ambiguous write");
    assert!(matches!(
        poisoned_error.kind,
        EncryptSendErrorKind::Poisoned
    ));
    assert!(poisoned_error.is_transport_unavailable());
    assert!(frames.is_empty());
    assert_eq!(frames.capacity(), retained_capacity);
    assert_eq!(transport.failed_sends(), 1);
    assert_eq!(
        transport.write_count(),
        0,
        "a poisoned sender must not attempt another transport write"
    );
}

/// A burst hands every frame to the sender before awaiting any of them, which
/// is what lets them coalesce into one transport write. Awaiting each before
/// enqueueing the next would still deliver all four, in order, and produce four
/// writes instead of one -- so the write count is the assertion that separates
/// the two, and the ciphertext order is what pins the counter sequence the peer
/// will decrypt against.
#[tokio::test]
async fn a_multi_frame_burst_stays_one_ordered_write() {
    use crate::transport::mock::CapturingMockTransport;

    let client = crate::test_utils::create_test_client().await;
    let transport = Arc::new(CapturingMockTransport::new());
    install_test_noise_socket(
        &client,
        transport.clone(),
        Arc::new(crate::runtime_impl::TokioRuntime),
    )
    .await;

    // Distinct lengths, so a reordering is visible in the frame sizes even
    // though the payloads are encrypted on the way out.
    let mut frames: Vec<Vec<u8>> = (1..=4).map(|n| vec![n as u8; 16 * n]).collect();
    let mut results = Vec::new();
    client
        .send_raw_bytes_burst(&mut frames, &mut results)
        .await
        .expect("installed socket");

    assert_eq!(results.len(), 4);
    assert!(results.iter().all(|result| result.is_ok()));
    assert!(
        frames.is_empty(),
        "the burst must drain its input, which the workers rely on to refill it"
    );
    assert_eq!(
        transport.write_count(),
        1,
        "the whole burst must reach the transport as one write"
    );

    let sent = transport.sent();
    assert_eq!(sent.len(), 4, "every frame must reach the wire");
    // Each wire frame is its plaintext plus the AEAD tag and the length prefix,
    // a fixed function of the plaintext length, so the sizes identify which
    // plaintext landed where.
    const TAG_AND_PREFIX: usize = 16 + wacore::framing::FRAME_LENGTH_SIZE;
    let lengths: Vec<usize> = sent.iter().map(|frame| frame.len()).collect();
    assert_eq!(
        lengths,
        (1..=4)
            .map(|n| 16 * n + TAG_AND_PREFIX)
            .collect::<Vec<usize>>(),
        "frames must reach the wire in the order they were given"
    );
}

#[tokio::test]
async fn raw_bytes_burst_surfaces_a_closed_sender_per_frame() {
    use crate::socket::error::EncryptSendErrorKind;

    let client = crate::test_utils::create_test_client().await;
    install_test_noise_socket(
        &client,
        Arc::new(crate::transport::mock::MockTransport),
        Arc::new(DropSpawnRuntime),
    )
    .await;

    let mut frames = Vec::with_capacity(4);
    let retained_capacity = frames.capacity();
    frames.push(vec![0x41; 32]);
    let mut results = Vec::new();
    client
        .send_raw_bytes_burst(&mut frames, &mut results)
        .await
        .expect("the installed socket remains reachable");
    let error = results
        .pop()
        .expect("one result")
        .expect_err("the sender receiver was dropped at construction");
    assert!(matches!(error.kind, EncryptSendErrorKind::ChannelClosed));
    assert!(error.is_transport_unavailable());
    assert!(frames.is_empty());
    assert_eq!(frames.capacity(), retained_capacity);
}

/// Live delivery receipts flow through the persistent worker: the receipt
/// reaches the transport and the flush counter returns to zero afterwards.
#[tokio::test]
async fn delivery_receipt_worker_sends_and_releases_flush() {
    use crate::socket::NoiseSocket;
    use async_trait::async_trait;
    use bytes::Bytes;
    use wacore::handshake::NoiseCipher;

    struct CountingTransport {
        sends: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl crate::transport::Transport for CountingTransport {
        async fn send(&self, _data: Bytes) -> Result<(), anyhow::Error> {
            self.sends.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        async fn disconnect(&self) {}
    }

    let client = crate::test_utils::create_test_client().await;
    let sends = Arc::new(AtomicUsize::new(0));
    let transport: Arc<dyn crate::transport::Transport> = Arc::new(CountingTransport {
        sends: Arc::clone(&sends),
    });
    let key = [0u8; 32];
    let noise_socket = NoiseSocket::new(
        client.runtime.clone(),
        Arc::clone(&transport),
        NoiseCipher::new(&key).expect("valid key"),
        NoiseCipher::new(&key).expect("valid key"),
    );
    *client.transport.lock().await = Some(transport);
    *client.noise_socket.lock().unwrap() = Some(Arc::new(noise_socket));
    client.is_connected.store(true, Ordering::Release);

    client.ack_received_message(&receipt_test_info("RCPT-WORKER-1"));

    let deadline = wacore::time::Instant::now() + Duration::from_secs(2);
    while sends.load(Ordering::SeqCst) == 0 {
        assert!(
            wacore::time::Instant::now() < deadline,
            "delivery receipt was never sent by the worker"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(sends.load(Ordering::SeqCst), 1);

    client
        .outbound_flush
        .flush(&*client.runtime, Duration::from_secs(1))
        .await;
    assert_eq!(
        client.outbound_flush.pending(),
        0,
        "worker must release the flush guard after the send"
    );
}

/// Transport loss and the poisoned follow-up are reconnect signals, not
/// receipt-worker stalls: both must release their flush guards without a
/// second write attempt.
#[tokio::test]
async fn delivery_receipt_worker_releases_flush_after_transport_and_poisoned_failures() {
    use crate::transport::mock::CapturingMockTransport;

    let client = crate::test_utils::create_test_client().await;
    let transport = Arc::new(CapturingMockTransport::new());
    transport.fail_next_sends(1);
    install_test_noise_socket(
        &client,
        transport.clone(),
        Arc::new(crate::runtime_impl::TokioRuntime),
    )
    .await;

    client.ack_received_message(&receipt_test_info("RCPT-FAIL-1"));
    crate::test_utils::wait_for_outbound_tasks(&client).await;
    assert_eq!(client.outbound_flush.pending(), 0);
    assert_eq!(transport.failed_sends(), 1);

    client.ack_received_message(&receipt_test_info("RCPT-POISONED-2"));
    crate::test_utils::wait_for_outbound_tasks(&client).await;
    assert_eq!(client.outbound_flush.pending(), 0);
    assert_eq!(
        transport.failed_sends(),
        1,
        "the poisoned sender must reject locally instead of touching transport"
    );
    assert_eq!(transport.write_count(), 0);
}

/// A closed flush scope (disconnect in progress) drops live receipts without
/// leaking the flush counter — mirroring the previous spawn-per-receipt path.
#[tokio::test]
async fn delivery_receipt_dropped_when_flush_scope_closed() {
    let client = crate::test_utils::create_test_client().await;
    client.outbound_flush.close();

    client.ack_received_message(&receipt_test_info("RCPT-CLOSED-1"));

    assert_eq!(
        client.outbound_flush.pending(),
        0,
        "closed scope must not track new receipts"
    );
    // The drop happens before the queue: with the scope closed nothing may be
    // enqueued, so the lazy worker queue is never even created.
    assert!(
        client.delivery_receipt_queue.get().is_none(),
        "a dropped receipt must not reach the worker queue"
    );
    // Finishing well under the 5s flush timeout proves nothing was tracked;
    // the generous bound keeps this stable on oversubscribed CI runners.
    tokio::time::timeout(
        Duration::from_secs(2),
        client
            .outbound_flush
            .flush(&*client.runtime, Duration::from_secs(5)),
    )
    .await
    .expect("flush must not wait when nothing was queued");
}

/// Issue #571 under the worker model: `flush()` must wait for receipts that
/// are queued to the worker but not yet sent, including one queued behind an
/// in-flight blocked send.
#[tokio::test]
async fn flush_waits_for_queued_delivery_receipts() {
    use crate::socket::NoiseSocket;
    use async_trait::async_trait;
    use bytes::Bytes;
    use wacore::handshake::NoiseCipher;

    struct BlockingTransport {
        send_started: async_channel::Sender<()>,
        release_send: async_channel::Receiver<()>,
    }

    #[async_trait]
    impl crate::transport::Transport for BlockingTransport {
        async fn send(&self, _data: Bytes) -> Result<(), anyhow::Error> {
            let _ = self.send_started.try_send(());
            let _ = self.release_send.recv().await;
            Ok(())
        }
        async fn disconnect(&self) {}
    }

    let client = crate::test_utils::create_test_client().await;
    let (send_started_tx, send_started_rx) = async_channel::bounded(2);
    let (release_send_tx, release_send_rx) = async_channel::bounded(2);
    let transport: Arc<dyn crate::transport::Transport> = Arc::new(BlockingTransport {
        send_started: send_started_tx,
        release_send: release_send_rx,
    });
    let key = [0u8; 32];
    let noise_socket = NoiseSocket::new(
        client.runtime.clone(),
        Arc::clone(&transport),
        NoiseCipher::new(&key).expect("valid key"),
        NoiseCipher::new(&key).expect("valid key"),
    );
    *client.transport.lock().await = Some(transport);
    *client.noise_socket.lock().unwrap() = Some(Arc::new(noise_socket));
    client.is_connected.store(true, Ordering::Release);

    client.ack_received_message(&receipt_test_info("RCPT-QUEUE-1"));
    tokio::time::timeout(Duration::from_secs(1), send_started_rx.recv())
        .await
        .expect("first receipt send should start")
        .expect("send_started sender should stay open");

    // Second receipt queues behind the blocked one and must also be tracked.
    client.ack_received_message(&receipt_test_info("RCPT-QUEUE-2"));
    assert_eq!(
        client.outbound_flush.pending(),
        2,
        "both the in-flight and the queued receipt must hold flush guards"
    );

    let flush_client = Arc::clone(&client);
    let flush_task = tokio::spawn(async move {
        flush_client
            .outbound_flush
            .flush(&*flush_client.runtime, Duration::from_secs(5))
            .await;
    });
    crate::test_utils::poll_until("the flusher to park on the outbound scope", || {
        client.outbound_flush.flush_waiters() >= 1
    })
    .await;
    assert!(
        !flush_task.is_finished(),
        "flush must wait while receipts are queued or in flight"
    );

    release_send_tx.send(()).await.expect("release first send");
    release_send_tx.send(()).await.expect("release second send");

    tokio::time::timeout(Duration::from_secs(2), flush_task)
        .await
        .expect("flush should finish once the queue drains")
        .expect("flush task should not panic");
    assert_eq!(client.outbound_flush.pending(), 0);
}

/// Verifies that `send_ack_for` returns an error (not silent Ok) when
/// disconnected. This ensures the caller's `warn!` fires so dropped acks
/// are visible in logs.
#[tokio::test]
async fn test_send_ack_for_returns_error_when_disconnected() {
    let backend = crate::test_utils::create_test_backend().await;
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("persistence manager should initialize"),
    );
    let (client, _rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        Arc::new(crate::transport::mock::MockTransportFactory::new()),
        Arc::new(MockHttpClient),
        None,
    )
    .await;

    // Not connected — send_ack_for should return Err, not Ok
    let receipt = NodeBuilder::new("receipt")
        .attr("from", "120363040237990503@g.us")
        .attr("id", "TEST-RECEIPT-ID")
        .attr("participant", "236395184570386@lid")
        .build();

    let result = client.send_ack_for(&receipt.as_node_ref()).await;
    assert!(
        matches!(result, Err(ClientError::NotConnected)),
        "send_ack_for must return Err(NotConnected) when disconnected, got: {result:?}"
    );
}

/// The gate that `send_ack_for` applies per ack, and that the burst path
/// applies once per burst, must agree on what counts as teardown. A burst that
/// missed it would write stale acks into a socket that is being torn down and
/// hold the outbound flush open until its timeout.
#[tokio::test]
async fn outbound_teardown_gate_covers_both_disconnect_signals() {
    let client = crate::test_utils::create_test_client().await;

    client.set_connected_for_test(true);
    client.expected_disconnect.store(false, Ordering::Relaxed);
    assert!(
        !client.outbound_teardown_in_progress(),
        "a live connection must not be treated as tearing down"
    );

    client.expected_disconnect.store(true, Ordering::Relaxed);
    assert!(
        client.outbound_teardown_in_progress(),
        "an expected disconnect (an intentional close, or a 515) must gate sends"
    );

    client.expected_disconnect.store(false, Ordering::Relaxed);
    client.set_connected_for_test(false);
    assert!(
        client.outbound_teardown_in_progress(),
        "a disconnected client must gate sends even without the expected flag"
    );
}

/// Exercise the actual deferred-ack worker, not only its predicate. Dropped
/// teardown batches must release guards, and reusing the batch buffer must not
/// leak either dropped ack into the next live burst.
#[tokio::test]
async fn deferred_ack_worker_drops_teardown_batches_and_recovers_cleanly() {
    use crate::transport::mock::CapturingMockTransport;

    let client = crate::test_utils::create_test_client().await;
    let transport = Arc::new(CapturingMockTransport::new());
    install_test_noise_socket(
        &client,
        transport.clone(),
        Arc::new(crate::runtime_impl::TokioRuntime),
    )
    .await;
    let receipt = |id| {
        let node = NodeBuilder::new("receipt")
            .attr("from", "15550001111@s.whatsapp.net")
            .attr("id", id)
            .build();
        crate::test_utils::node_to_owned_ref(&node)
    };

    client.expected_disconnect.store(true, Ordering::Relaxed);
    client
        .process_node(receipt("ACK-EXPECTED-DISCONNECT"))
        .await;
    crate::test_utils::wait_for_outbound_tasks(&client).await;
    assert_eq!(transport.sent_count(), 0);

    client.expected_disconnect.store(false, Ordering::Relaxed);
    client.set_connected_for_test(false);
    client.process_node(receipt("ACK-DISCONNECTED")).await;
    crate::test_utils::wait_for_outbound_tasks(&client).await;
    assert_eq!(transport.sent_count(), 0);

    client.set_connected_for_test(true);
    client.process_node(receipt("ACK-LIVE")).await;
    crate::test_utils::wait_for_outbound_tasks(&client).await;
    assert_eq!(
        transport.sent_count(),
        1,
        "only the live ack may survive into the reusable batch"
    );
    assert_eq!(client.outbound_flush.pending(), 0);
}

/// Verifies that `send_ack_for` returns Ok when expected_disconnect is set,
/// since this is an intentional shutdown path.
#[tokio::test]
async fn test_send_ack_for_returns_ok_on_expected_disconnect() {
    let backend = crate::test_utils::create_test_backend().await;
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("persistence manager should initialize"),
    );
    let (client, _rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        Arc::new(crate::transport::mock::MockTransportFactory::new()),
        Arc::new(MockHttpClient),
        None,
    )
    .await;

    // Set expected disconnect — send_ack_for should gracefully return Ok
    client.expected_disconnect.store(true, Ordering::Relaxed);

    let receipt = NodeBuilder::new("receipt")
        .attr("from", "120363040237990503@g.us")
        .attr("id", "TEST-RECEIPT-ID")
        .build();

    let result = client.send_ack_for(&receipt.as_node_ref()).await;
    assert!(
        result.is_ok(),
        "send_ack_for should return Ok during expected disconnect"
    );
}

// Per-connection notify must NOT set the terminal sticky flag; if it did,
// every reconnect would instantly abort subscribers registered on the
// terminal signal. Regression guard for the CI breakage observed on PR #560.
#[tokio::test]
async fn per_connection_notify_leaves_terminal_signal_untouched() {
    let client = crate::test_utils::create_test_client().await;

    client.notify_connection_shutdown();

    assert!(
        !client.shutdown_signal().is_fired(),
        "terminal shutdown must stay clean when only per-connection fires"
    );
}

// Subscribers registered AFTER a reset must not see the previous
// notifier's fired state. This is the core property that makes reconnect
// work: after cleanup_connection_state notifies the per-connection
// signal, the next connection replaces it with a fresh one.
#[tokio::test]
async fn reset_gives_fresh_per_connection_notifier() {
    let client = crate::test_utils::create_test_client().await;

    client.notify_connection_shutdown();
    assert!(
        client.connection_shutdown_signal().is_fired(),
        "subscriber BEFORE reset sees the notify on the current notifier"
    );

    client.reset_connection_shutdown();

    assert!(
        !client.connection_shutdown_signal().is_fired(),
        "subscribers AFTER reset must NOT see the previous notifier's state"
    );
}

// Capture-once regression guard: a ShutdownSignal captured before a reset
// must keep observing the pre-reset fired state. Without this, a
// reconnect after the old notifier is replaced in the Mutex would
// strand long-lived tasks (e.g. keepalive) on a new notifier they
// never registered for. See keepalive_loop which captures its signal
// once at task startup.
#[tokio::test]
async fn captured_signal_keeps_observing_old_notifier_after_reset() {
    let client = crate::test_utils::create_test_client().await;

    let captured = client.connection_shutdown_signal();
    client.notify_connection_shutdown();
    client.reset_connection_shutdown();

    assert!(
        captured.is_fired(),
        "captured signal must retain the pre-reset notifier's fired state"
    );
}

// Terminal disconnect() must also wake per-connection subscribers via
// cleanup_connection_state, so keepalive/request/read loop exit promptly.
#[tokio::test]
async fn terminal_disconnect_propagates_to_per_connection_signal() {
    let client = crate::test_utils::create_test_client().await;
    let conn_signal = client.connection_shutdown_signal();

    client.disconnect().await;

    assert!(
        conn_signal.is_fired(),
        "disconnect must fire per-connection via cleanup_connection_state"
    );
    assert!(
        client.shutdown_signal().is_fired(),
        "disconnect must also fire terminal"
    );
}

/// Dropping the last owner must release persistence handles promptly.
#[tokio::test]
async fn dropping_fresh_client_releases_it_without_shutdown() {
    let client = crate::test_utils::create_test_client().await;
    let weak = Arc::downgrade(&client);

    drop(client);

    tokio::time::timeout(Duration::from_secs(5), async {
        while weak.strong_count() != 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap_or_else(|_| {
        panic!(
            "client is still retained by a background task (strong_count={})",
            weak.strong_count()
        )
    });
}

/// Locks the zero-allocation property of the ack miss path: id resolution and
/// the waiter probe must borrow from the node buffer. An `into_owned()` here
/// costs one String per received ack, which the e2e dhat profile caught live.
#[tokio::test]
async fn ack_miss_path_does_not_heap_allocate() {
    let client = crate::test_utils::create_test_client().await;

    let node = Arc::new(owned_ack_node("3EB0A9252A8F12B7E2"));

    // A per-call String shows up in every window, so the minimum only reaches 0
    // when the path is clean.
    let min_delta = crate::test_alloc::min_allocs(0, || {
        let handled = client.handle_ack_response_arc(&node);
        assert!(!handled, "no waiter is registered for this id");
    });
    assert_eq!(min_delta, 0, "ack miss path must not allocate");
}

/// The public stats snapshot must reflect the client's counters plus the
/// client-level fields (reconnect errors, throttled resends) it fills in.
#[tokio::test]
async fn stats_snapshot_reflects_counters() {
    let client = crate::test_utils::create_test_client().await;

    client.stats.record_frame_sent(150);
    client.stats.record_recv_batch(300, 2);
    client.stats.record_message_sent();
    client.auto_reconnect_errors.store(3, Ordering::Relaxed);

    let snap = client.stats();
    assert_eq!(snap.bytes_sent, 150);
    assert_eq!(snap.frames_sent, 1);
    assert_eq!(snap.bytes_received, 300);
    assert_eq!(snap.frames_received, 2);
    assert_eq!(snap.messages_sent, 1);
    assert_eq!(snap.reconnect_errors, 3);
    assert_eq!(snap.resends_throttled, 0);
}

/// A clock read leaves the module on wasm32/embedded, so the wire path owes a
/// budget: only the send that arms the dead-socket anchor may date itself, and
/// only one stamp may be spent per received transport event.
#[test]
fn wire_bookkeeping_reads_the_clock_only_where_a_value_is_used() {
    use wacore::time::clock_reads;

    let stats = wacore::stats::SessionStats::new();

    let arming = clock_reads::snapshot();
    stats.record_frame_sent(10);
    assert_eq!(
        clock_reads::since(arming).wall,
        1,
        "the send that arms the anchor dates it"
    );

    let armed = clock_reads::snapshot();
    for _ in 0..16 {
        stats.record_frame_sent(10);
    }
    assert_eq!(
        clock_reads::since(armed).wall,
        0,
        "sends under an already-armed anchor have nothing to date"
    );

    let recv = clock_reads::snapshot();
    stats.mark_recv_activity();
    stats.record_recv_batch(100, 1);
    assert_eq!(
        clock_reads::since(recv).wall,
        1,
        "a single-frame batch is stamped once, at arrival"
    );

    let long_batch = clock_reads::snapshot();
    stats.mark_recv_activity();
    stats.record_recv_batch(100, 4);
    assert_eq!(
        clock_reads::since(long_batch).wall,
        2,
        "a long batch re-stamps on completion so a slow drain is not read as silence"
    );

    let rearm = clock_reads::snapshot();
    stats.record_frame_sent(10);
    assert_eq!(
        clock_reads::since(rearm).wall,
        1,
        "the receive cancelled the anchor, so this send arms it again"
    );
}

/// Handling a received stanza must not ask for the time: the read loop already
/// stamped arrival, and nothing downstream of it dates anything.
#[tokio::test]
async fn received_stanza_handling_reads_no_clock() {
    use wacore::time::clock_reads;

    let client = crate::test_utils::create_test_client().await;
    let receipt = || {
        to_owned_node(
            &NodeBuilder::new("receipt")
                .attr("id", "3EB0AABBCCDDEEFF001122")
                .attr("from", "5511900000001@s.whatsapp.net")
                .attr("t", "1780000000")
                .build(),
        )
    };

    client.process_decrypted_node(receipt()).await;
    crate::test_utils::wait_for_outbound_tasks(&client).await;

    let base = clock_reads::snapshot();
    client.process_decrypted_node(receipt()).await;
    let reads = clock_reads::since(base);

    assert_eq!(reads.wall, 0, "receipt handling reads no wall clock");
    assert_eq!(
        reads.monotonic, 0,
        "receipt handling reads no monotonic clock"
    );
}

/// memory_report must be callable on a fresh client and internally
/// consistent: empty collections report zero entries and zero bytes.
/// The Display sections are sliced by hard-coded boundaries over
/// `collections()`, so adding a cache without moving the boundary silently
/// prints it under the wrong heading and drops the last one of the next
/// section. This pins the layout instead of the individual counts.
#[tokio::test]
async fn memory_report_display_sections_stay_aligned() {
    let client = crate::test_utils::create_test_client_with_name("memory_report_sections").await;
    let rendered = client.memory_report().await.to_string();

    let ttl_start = rendered
        .find("--- TTL-bounded caches ---")
        .expect("ttl section");
    let signal_start = rendered.find("--- Signal store").expect("signal section");
    let ttl_block = &rendered[ttl_start..signal_start];

    for name in [
        "group_cache:",
        "device_registry_cache:",
        "recent_messages:",
        "group_devices_memo:",
        "dm_devices_memo:",
    ] {
        assert!(
            ttl_block.contains(name),
            "{name} must render under the TTL-bounded heading, got:\n{rendered}"
        );
    }
    for name in [
        "signal_sessions:",
        "signal_identities:",
        "signal_sender_keys:",
    ] {
        assert!(
            rendered[signal_start..].contains(name),
            "{name} must render under the Signal heading, got:\n{rendered}"
        );
    }
}

#[tokio::test]
async fn memory_report_on_fresh_client() {
    // recent_messages is capacity-0 (disabled) by default; enable it so the
    // byte-attribution assertion below has a collection to land in.
    let mut cache_config = CacheConfig::default();
    cache_config.recent_messages =
        crate::cache_config::CacheEntryConfig::new(Some(Duration::from_secs(300)), 64);
    let client = crate::test_utils::create_test_client_with_config(
        "memory_report",
        Arc::new(MockHttpClient),
        cache_config,
    )
    .await;

    let report = client.memory_report().await;
    assert_eq!(report.recent_messages.entries, 0);
    assert_eq!(report.recent_messages.bytes, 0);
    assert_eq!(report.group_distribution_locks, 0);
    assert_eq!(report.group_distribution_lock_evictions, 0);
    assert_eq!(report.group_distribution_lock_eviction_blocks, 0);
    assert_eq!(report.signal_sessions.entries, 0);
    assert_eq!(report.response_waiters, 0);

    // Retained bytes must appear once something is cached.
    let key = ChatMessageId::new(
        "559980000001@s.whatsapp.net".parse().unwrap(),
        "3EB0TESTMSGID".to_string(),
    );
    client
        .recent_messages
        .insert(key, Arc::new(vec![0u8; 2048]))
        .await;
    let report = client.memory_report().await;
    assert_eq!(report.recent_messages.entries, 1);
    assert!(
        report.recent_messages.bytes >= 2048,
        "cached payload bytes must be attributed (got {})",
        report.recent_messages.bytes
    );
    assert!(report.total_estimated_bytes() >= 2048);
    // Display must render without panicking.
    let _ = report.to_string();
}

/// resource_report (workstream F) composes the client's own memory_report with
/// the out-of-client components, folds in an AllocMeter snapshot when installed,
/// and is Display-able.
#[tokio::test]
async fn resource_report_composes_client_and_out_of_client_components() {
    use wacore::stats::{AllocMeter, TaskInstrument};

    let client = crate::test_utils::create_test_client().await;

    let report = client.resource_report().await;

    // The client sub-report equals memory_report's total.
    let mem = client.memory_report().await;
    assert_eq!(
        report.client.total_estimated_bytes(),
        mem.total_estimated_bytes()
    );

    // The SQLite backend is intentionally best-effort and uses a non-blocking pool checkout.
    // A concurrently held single connection may therefore report no storage sample; when a sample
    // is available, its two SQLite-derived fields must remain coherent. The backend's dedicated
    // resource-report test deterministically verifies the concrete page-cache calculation.
    assert_eq!(
        report.storage.memory_bytes.is_some(),
        report.storage.pages.is_some()
    );

    // No transport is connected and the mock HTTP client reports nothing.
    assert!(report.transport.is_none());
    assert!(report.http.is_none());
    assert!(report.alloc.is_none(), "no alloc meter installed yet");

    // The total includes the storage estimate on top of client collections.
    assert!(report.total_estimated_bytes() >= report.storage.total_bytes());
    let _ = report.to_string();

    // Install an alloc meter and charge a known allocation inside a poll scope;
    // the next report folds in its snapshot.
    let meter = Arc::new(AllocMeter::new());
    meter.on_poll_start();
    AllocMeter::on_alloc(4096);
    meter.on_poll_end();
    let _ = client.alloc_meter.set(meter);

    let report = client.resource_report().await;
    let alloc = report
        .alloc
        .expect("alloc snapshot folded in once installed");
    assert_eq!(alloc.allocated_bytes, 4096);
    assert_eq!(alloc.allocations, 1);
}

/// InstrumentedRuntime must invoke the TaskInstrument around polls of spawned
/// futures and blocking closures, and CpuMeter must accumulate them.
#[tokio::test]
async fn instrumented_runtime_reports_to_cpu_meter() {
    use wacore::runtime::Runtime as _;
    use wacore::stats::{CpuMeter, InstrumentedRuntime};

    let meter = Arc::new(CpuMeter::new());
    let runtime =
        InstrumentedRuntime::new(Arc::new(crate::runtime_impl::TokioRuntime), meter.clone());

    let (tx, rx) = oneshot::channel::<()>();
    runtime
        .spawn(Box::pin(async move {
            let _ = tx.send(());
        }))
        .detach();
    rx.await.expect("spawned future ran");

    let after_spawn = meter.snapshot();
    assert!(after_spawn.polls >= 1, "spawned future polls are metered");

    runtime.spawn_blocking(Box::new(|| {})).await;
    let after_blocking = meter.snapshot();
    assert!(
        after_blocking.polls > after_spawn.polls,
        "blocking work is metered too"
    );
}

/// `spawn_detached` is the fire-and-forget path most of the read loop uses. The
/// decorator forwards it instead of falling back to `spawn`, so it must still
/// meter every poll of the task.
#[tokio::test]
async fn instrumented_runtime_meters_detached_spawns() {
    use wacore::runtime::Runtime as _;
    use wacore::stats::{CpuMeter, InstrumentedRuntime};

    let meter = Arc::new(CpuMeter::new());
    let runtime =
        InstrumentedRuntime::new(Arc::new(crate::runtime_impl::TokioRuntime), meter.clone());

    let (tx, rx) = oneshot::channel::<()>();
    runtime.spawn_detached(Box::pin(async move {
        tokio::task::yield_now().await;
        let _ = tx.send(());
    }));
    rx.await.expect("detached future ran");

    assert!(
        meter.snapshot().polls >= 2,
        "a detached task is metered on every poll, got {}",
        meter.snapshot().polls
    );
}

/// Cancellation through the decorator: aborting a metered task mid-flight must
/// leave the meter balanced, so work that runs afterwards is still attributed
/// to whoever actually did it.
#[tokio::test]
async fn aborting_a_metered_task_keeps_the_meter_balanced() {
    use wacore::runtime::Runtime as _;
    use wacore::stats::{AllocMeter, InstrumentedRuntime, TaskInstrument};

    let meter = AllocMeter::new();
    let instrument: Arc<dyn TaskInstrument> = Arc::new(meter.clone());
    let runtime = InstrumentedRuntime::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        Arc::clone(&instrument),
    );

    let (started_tx, started_rx) = oneshot::channel::<()>();
    let handle = runtime.spawn(Box::pin(async move {
        let _ = started_tx.send(());
        std::future::pending::<()>().await;
    }));
    started_rx.await.expect("task started");
    handle.abort();
    tokio::task::yield_now().await;

    // `#[tokio::test]` drives a current-thread runtime, so the aborted task ran
    // on this very thread: a scope it failed to close would charge this probe.
    let after_abort = meter.snapshot();
    AllocMeter::on_alloc(4096);
    assert_eq!(
        meter.snapshot().allocations,
        after_abort.allocations,
        "the aborted task must not still be the active scope"
    );

    // And the meter still attributes work that follows.
    let (tx, rx) = oneshot::channel::<()>();
    runtime.spawn_detached(Box::pin(async move {
        AllocMeter::on_alloc(512);
        let _ = tx.send(());
    }));
    rx.await.expect("follow-up task ran");
    assert!(
        meter.snapshot().allocated_bytes >= after_abort.allocated_bytes + 512,
        "the follow-up task is still charged"
    );
}

/// A status@broadcast stanza feeds the same per-chat queue a `<message>` does,
/// so its enqueue must keep the read loop's arrival order.
#[tokio::test]
async fn status_broadcast_stanzas_are_dispatched_inline() {
    use wacore_binary::builder::NodeBuilder;

    let client = create_offline_sync_test_client().await;

    let status = NodeBuilder::new("status")
        .attr("from", "status@broadcast")
        .attr("id", "INLINE-1")
        .build();
    assert!(
        client.processes_inline(&status.as_node_ref()),
        "a status@broadcast stanza must keep the read loop's arrival order"
    );

    let message = NodeBuilder::new("message")
        .attr("from", "status@broadcast")
        .attr("id", "INLINE-2")
        .build();
    assert!(
        client.processes_inline(&message.as_node_ref()),
        "the pre-existing <message> form is unchanged"
    );

    let newsletter_status = NodeBuilder::new("status")
        .attr("from", "120363298765432100@newsletter")
        .attr("id", "INLINE-3")
        .build();
    assert!(
        !client.processes_inline(&newsletter_status.as_node_ref()),
        "a newsletter <status> has no per-chat queue to order"
    );
}

/// The server counts status updates and calls separately, so a preview that
/// only reports messages/notifications/receipts leaves part of the backlog
/// unaccounted for.
#[tokio::test]
async fn offline_preview_reports_status_and_call_counts() {
    use wacore::types::events::{Event, EventHandler};

    #[derive(Default)]
    struct PreviewRecorder {
        previews: std::sync::Mutex<Vec<wacore::types::events::OfflineSyncPreview>>,
    }

    impl EventHandler for PreviewRecorder {
        fn handle_event(&self, event: Arc<Event>) {
            if let Event::OfflineSyncPreview(preview) = &*event {
                self.previews.lock().unwrap().push(preview.clone());
            }
        }
    }

    let client = create_offline_sync_test_client().await;
    let recorder = Arc::new(PreviewRecorder::default());
    client
        .core
        .event_bus
        .subscribe_handler(recorder.clone())
        .detach();

    let node = NodeBuilder::new("ib")
        .children([NodeBuilder::new("offline_preview")
            .attr("count", "9")
            .attr("message", "2")
            .attr("notification", "1")
            .attr("receipt", "1")
            .attr("appdata", "1")
            .attr("call", "1")
            .attr("status", "3")
            .build()])
        .build();

    client.process_node(node_to_owned_ref(node)).await;

    let previews = recorder.previews.lock().unwrap();
    let preview = previews
        .first()
        .expect("a preview event must be dispatched");
    assert_eq!(preview.total, 9);
    assert_eq!(preview.messages, 2);
    assert_eq!(preview.notifications, 1);
    assert_eq!(preview.receipts, 1);
    assert_eq!(preview.app_data_changes, 1);
    assert_eq!(preview.calls, 1);
    assert_eq!(preview.statuses, 3);
}

/// A preview from a server that never sends the newer counts still parses.
#[tokio::test]
async fn offline_preview_defaults_absent_counts_to_zero() {
    use wacore::types::events::{Event, EventHandler};

    #[derive(Default)]
    struct PreviewRecorder {
        previews: std::sync::Mutex<Vec<wacore::types::events::OfflineSyncPreview>>,
    }

    impl EventHandler for PreviewRecorder {
        fn handle_event(&self, event: Arc<Event>) {
            if let Event::OfflineSyncPreview(preview) = &*event {
                self.previews.lock().unwrap().push(preview.clone());
            }
        }
    }

    let client = create_offline_sync_test_client().await;
    let recorder = Arc::new(PreviewRecorder::default());
    client
        .core
        .event_bus
        .subscribe_handler(recorder.clone())
        .detach();

    let node = NodeBuilder::new("ib")
        .children([NodeBuilder::new("offline_preview")
            .attr("count", "1")
            .attr("message", "1")
            .build()])
        .build();

    client.process_node(node_to_owned_ref(node)).await;

    let previews = recorder.previews.lock().unwrap();
    let preview = previews
        .first()
        .expect("a preview event must be dispatched");
    assert_eq!(preview.total, 1);
    assert_eq!(preview.calls, 0);
    assert_eq!(preview.statuses, 0);
}

/// A phash waiter is resolved by an ack that may never arrive, and nothing
/// polls it. The sweep has to drop the stale one, or a non-empty map reads as
/// "IQ pending" and silences pings for the life of the connection.
#[test]
fn phash_waiter_sweep_drops_only_entries_that_lived_through_a_sweep() {
    use crate::client::{PhashWaiter, ResponseWaiter, ResponseWaiterMap};
    use futures::channel::oneshot;

    let mut map = ResponseWaiterMap::default();
    let waiter = |registered_epoch: u64| {
        ResponseWaiter::Phash(PhashWaiter {
            expected: wacore_binary::CompactString::from("hash"),
            jid: "13135550100@s.whatsapp.net".parse().expect("valid jid"),
            invalidate_group_cache: false,
            registered_epoch,
        })
    };

    let epoch = map.current_epoch();
    map.insert("first".to_string(), waiter(epoch));
    let (iq_tx, _iq_rx) = oneshot::channel();
    map.insert("iq".to_string(), ResponseWaiter::Iq(iq_tx));

    // One sweep is not enough: the waiter registered in the current epoch is
    // still within its window, so an ack in flight is not discarded early.
    map.drop_expired_phash();
    assert!(
        map.remove("first").is_some(),
        "a waiter must survive the sweep of the epoch it registered in"
    );

    // Registered before a sweep, then swept again: now it is stale.
    let epoch = map.current_epoch();
    map.insert("stale".to_string(), waiter(epoch));
    map.drop_expired_phash();
    map.drop_expired_phash();
    assert!(
        map.remove("stale").is_none(),
        "a waiter that lived through a full sweep must be dropped"
    );
    assert!(
        map.remove("iq").is_some(),
        "the sweep must never touch IQ waiters, which have their own cleanup"
    );
}

/// A non-reconnectable connect failure releases work parked in
/// `await_connection`, having decided the session is over.
///
/// What this pins is the outcome, and only that. It does **not** pin the order
/// of the stores and the notify inside `handle_connect_failure`, and no test at
/// this level can: nothing awaits between them, so the waiter is never
/// scheduled into the gap and the assertions below hold either way. Reordering
/// them keeps this test green.
///
/// That order is held by the comment at the notify, not from here. It is worth
/// holding because the announcement is what wakes the wait, and the wait
/// answers by reading state — announcing first offers it a client that has not
/// yet decided. Pinning it would mean a pause hook between the two, in
/// production code, to catch a race that `cleanup_connection_state` and the run
/// loop's exit both go on to correct. Not worth the hook.
#[tokio::test]
async fn a_terminal_connect_failure_releases_a_parked_wait() {
    let client = create_offline_sync_test_client().await;
    client.is_running.store(true, Ordering::Relaxed);

    let waiter = {
        let client = Arc::clone(&client);
        tokio::spawn(async move { client.await_connection().await })
    };
    crate::test_utils::poll_until("the waiter to park on the notifier", || {
        client.session_state_notifier.total_listeners() >= 1
    })
    .await;

    // 403 is REASON_LOCKED: not transient, so no replacement is coming.
    let failure = NodeBuilder::new("failure").attr("reason", "403").build();
    client.handle_connect_failure(&failure.as_node_ref()).await;

    assert!(
        client.is_terminal(),
        "the failure decided the session is over"
    );
    assert!(
        !tokio::time::timeout(Duration::from_secs(5), waiter)
            .await
            .expect("and the wait must end on that decision")
            .expect("the waiter should not panic"),
        "reporting that no connection arrived"
    );
}

/// The write-once cells (`group_cache`, `app_state_processor`) are read on the
/// send and app-state paths on the strength of never being rebuilt. These prove
/// the three ways that could break: a second call, a reconnect cleanup, and a
/// racing first call.
#[tokio::test]
async fn write_once_cells_return_the_same_instance_across_calls() {
    let client = crate::test_utils::create_test_client().await;

    let group_cache = client.get_group_cache().clone();
    let processor = client.get_app_state_processor().clone();

    assert!(
        Arc::ptr_eq(&group_cache, client.get_group_cache()),
        "the group cache must not be rebuilt on a second read"
    );
    assert!(
        Arc::ptr_eq(&processor, client.get_app_state_processor()),
        "the app-state processor must not be rebuilt on a second read"
    );
}

#[tokio::test]
async fn reconnect_cleanup_leaves_the_write_once_cells_installed() {
    let client = crate::test_utils::create_test_client().await;

    let group_cache = client.get_group_cache().clone();
    let processor = client.get_app_state_processor().clone();

    client.cleanup_connection_state().await;

    assert!(
        Arc::ptr_eq(&group_cache, client.get_group_cache()),
        "cleanup must not drop the group cache"
    );
    assert!(
        Arc::ptr_eq(&processor, client.get_app_state_processor()),
        "cleanup clears the processor's key cache in place, it does not replace it"
    );
    assert!(
        client.group_cache.get().is_some() && client.app_state_processor.get().is_some(),
        "and neither cell may fall back to uninitialized"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_first_readers_agree_on_one_instance() {
    let client = crate::test_utils::create_test_client().await;

    // OS threads and a blocking barrier, not tasks on a worker pool: the
    // getters are synchronous now, so every reader can be inside `get_or_init`
    // at once instead of at most `worker_threads` of them, and none can be
    // spawned late enough to find the cell already warm.
    const READERS: usize = 16;
    let results = tokio::task::spawn_blocking(move || {
        let start = std::sync::Barrier::new(READERS);
        std::thread::scope(|scope| {
            let readers: Vec<_> = (0..READERS)
                .map(|_| {
                    let client = &client;
                    let start = &start;
                    scope.spawn(move || {
                        start.wait();
                        (
                            client.get_group_cache().clone(),
                            client.get_app_state_processor().clone(),
                        )
                    })
                })
                .collect();
            readers
                .into_iter()
                .map(|reader| reader.join().expect("reader thread should not panic"))
                .collect::<Vec<_>>()
        })
    })
    .await
    .expect("blocking scope should not panic");

    let (first_cache, first_processor) = &results[0];
    for (cache, processor) in &results {
        assert!(
            Arc::ptr_eq(first_cache, cache),
            "every racing reader must observe the same group cache"
        );
        assert!(
            Arc::ptr_eq(first_processor, processor),
            "every racing reader must observe the same app-state processor"
        );
    }
}

fn test_chatstate_stanza() -> wacore::iq::chatstate::ChatstateStanza {
    use wacore::iq::chatstate::{ChatstateSource, ChatstateStanza, ReceivedChatState};

    ChatstateStanza {
        source: ChatstateSource::User {
            from: "15550001111@s.whatsapp.net".parse().expect("valid jid"),
        },
        state: ReceivedChatState::Typing,
    }
}

#[tokio::test]
async fn chatstate_dispatch_skips_the_event_build_with_no_handlers() {
    let client = crate::test_utils::create_test_client().await;

    client
        .dispatch_chatstate_event(test_chatstate_stanza())
        .await;

    assert_eq!(
        client.chatstate_events_built.load(Ordering::Acquire),
        0,
        "the default registers no handler, so nothing should read the event"
    );
}

#[tokio::test]
async fn chatstate_dispatch_reaches_every_registered_handler() {
    let client = crate::test_utils::create_test_client().await;
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));

    for tag in ["first", "second"] {
        let seen = seen.clone();
        client.register_chatstate_handler(Arc::new(move |event| {
            seen.lock()
                .unwrap_or_else(|p| p.into_inner())
                .push((tag, event.chat.to_string()));
        }));
    }

    client
        .dispatch_chatstate_event(test_chatstate_stanza())
        .await;

    crate::test_utils::poll_until("both chatstate handlers ran", || {
        seen.lock().unwrap_or_else(|p| p.into_inner()).len() == 2
    })
    .await;

    let mut seen = seen.lock().unwrap_or_else(|p| p.into_inner()).clone();
    seen.sort();
    assert_eq!(
        seen,
        vec![
            ("first", "15550001111@s.whatsapp.net".to_string()),
            ("second", "15550001111@s.whatsapp.net".to_string()),
        ]
    );
    assert_eq!(
        client.chatstate_events_built.load(Ordering::Acquire),
        1,
        "the event is built once and cloned per handler"
    );
}

// --- stanza interceptors ---------------------------------------------------

use crate::client::interceptor::{Interception, StanzaInterceptor};
use wacore_binary::OwnedNodeRef;

/// Builds a client with no transport, which is enough: interception happens
/// before anything is sent.
async fn create_interceptor_test_client() -> Arc<Client> {
    create_offline_sync_test_client().await
}

/// Records which stanzas it saw and claims the ones whose tag matches.
struct Recorder {
    claim: &'static str,
    seen: Arc<std::sync::Mutex<Vec<String>>>,
}

impl StanzaInterceptor for Recorder {
    fn intercept(&self, node: &OwnedNodeRef) -> Interception {
        self.seen
            .lock()
            .expect("recorder lock")
            .push(node.tag().to_string());
        if node.tag() == self.claim {
            Interception::Handled
        } else {
            Interception::Pass
        }
    }
}

fn recorder(claim: &'static str) -> (Arc<Recorder>, Arc<std::sync::Mutex<Vec<String>>>) {
    let seen = Arc::new(std::sync::Mutex::new(Vec::new()));
    (
        Arc::new(Recorder {
            claim,
            seen: Arc::clone(&seen),
        }),
        seen,
    )
}

#[tokio::test]
async fn interceptors_cost_nothing_until_one_is_registered() {
    let client = create_interceptor_test_client().await;
    assert!(!client.has_stanza_interceptors());

    let (interceptor, _seen) = recorder("nothing");
    let handle = client.add_stanza_interceptor(interceptor);
    assert!(client.has_stanza_interceptors());

    drop(handle);
    assert!(
        !client.has_stanza_interceptors(),
        "dropping the handle unregisters it"
    );
}

#[tokio::test]
async fn an_interceptor_sees_every_decoded_stanza() {
    let client = create_interceptor_test_client().await;
    let (interceptor, seen) = recorder("nothing-matches");
    let _handle = client.add_stanza_interceptor(interceptor);

    for tag in ["ib", "receipt", "notification"] {
        client
            .process_node(node_to_owned_ref(NodeBuilder::new(tag).build()))
            .await;
    }

    assert_eq!(
        *seen.lock().expect("recorder lock"),
        ["ib", "receipt", "notification"]
    );
}

/// A `<receipt>` the client dispatches, and the event that proves it did.
fn receipt_stanza() -> Node {
    NodeBuilder::new("receipt")
        .attr("from", "5511999998888@s.whatsapp.net")
        .attr("id", "RCPT-INTERCEPT")
        .build()
}

#[tokio::test]
async fn passing_leaves_the_stanza_to_the_client() {
    use wacore::types::events::ChannelEventHandler;
    let client = create_interceptor_test_client().await;
    let (handler, events) = ChannelEventHandler::new();
    client.subscribe_handler(handler).detach();

    let (interceptor, seen) = recorder("nothing-matches");
    let _handle = client.add_stanza_interceptor(interceptor);

    client
        .process_node(node_to_owned_ref(receipt_stanza()))
        .await;

    assert_eq!(seen.lock().expect("recorder lock").len(), 1, "it ran");
    assert!(
        events.try_recv().is_ok(),
        "and the built-in handler dispatched its event"
    );
}

#[tokio::test]
async fn claiming_a_stanza_skips_the_built_in_pipeline() {
    use wacore::types::events::ChannelEventHandler;
    let client = create_interceptor_test_client().await;
    let (handler, events) = ChannelEventHandler::new();
    client.subscribe_handler(handler).detach();

    let (interceptor, seen) = recorder("receipt");
    let _handle = client.add_stanza_interceptor(interceptor);

    client
        .process_node(node_to_owned_ref(receipt_stanza()))
        .await;

    assert_eq!(seen.lock().expect("recorder lock").len(), 1);
    assert!(
        events.try_recv().is_err(),
        "the built-in receipt handler must not have dispatched"
    );
}

#[tokio::test]
async fn interception_does_not_touch_connection_bookkeeping() {
    // Offline-sync tracking runs before dispatch, and must keep running: it is
    // what tells the client the drain finished. An interceptor exists to take
    // over *handling* a stanza, not to opt out of staying connected.
    let client = create_interceptor_test_client().await;
    client
        .offline_sync_metrics
        .active
        .store(true, Ordering::Release);

    let (interceptor, seen) = recorder("ib");
    let _handle = client.add_stanza_interceptor(interceptor);

    let node = NodeBuilder::new("ib")
        .children([NodeBuilder::new("offline").attr("count", "0").build()])
        .build();
    client.process_node(node_to_owned_ref(node)).await;

    assert_eq!(seen.lock().expect("recorder lock").len(), 1, "it ran");
    assert!(
        !client.offline_sync_metrics.active.load(Ordering::Acquire),
        "offline-sync tracking still ran, claimed or not"
    );
}

#[tokio::test]
async fn the_first_interceptor_to_claim_a_stanza_wins() {
    let client = create_interceptor_test_client().await;
    let (first, first_seen) = recorder("receipt");
    let (second, second_seen) = recorder("receipt");

    let _a = client.add_stanza_interceptor(first);
    let _b = client.add_stanza_interceptor(second);

    client
        .process_node(node_to_owned_ref(NodeBuilder::new("receipt").build()))
        .await;

    assert_eq!(first_seen.lock().expect("lock").len(), 1);
    assert!(
        second_seen.lock().expect("lock").is_empty(),
        "registration order is priority order"
    );
}

#[tokio::test]
async fn a_passing_interceptor_does_not_stop_the_next_one() {
    let client = create_interceptor_test_client().await;
    let (first, first_seen) = recorder("nothing");
    let (second, second_seen) = recorder("receipt");

    let _a = client.add_stanza_interceptor(first);
    let _b = client.add_stanza_interceptor(second);

    client
        .process_node(node_to_owned_ref(NodeBuilder::new("receipt").build()))
        .await;

    assert_eq!(first_seen.lock().expect("lock").len(), 1);
    assert_eq!(second_seen.lock().expect("lock").len(), 1);
}

#[tokio::test]
async fn an_unregistered_interceptor_stops_seeing_stanzas() {
    let client = create_interceptor_test_client().await;
    let (interceptor, seen) = recorder("nothing");
    let handle = client.add_stanza_interceptor(interceptor);

    client
        .process_node(node_to_owned_ref(NodeBuilder::new("receipt").build()))
        .await;
    assert_eq!(seen.lock().expect("lock").len(), 1);

    drop(handle);
    client
        .process_node(node_to_owned_ref(NodeBuilder::new("receipt").build()))
        .await;
    assert_eq!(
        seen.lock().expect("lock").len(),
        1,
        "no further stanzas after unregistering"
    );
}

#[tokio::test]
async fn a_claimed_unknown_stanza_is_acked_rather_than_nacked() {
    // The reason this exists. A tag the client does not model is nacked, which
    // tells the server this client cannot act on it. An interceptor that *can*
    // act on it says the opposite — but it must still say something: answering
    // nothing leaves the stanza in the offline queue and keeps the stream
    // recycling.
    let (client, transport) = crate::test_utils::create_iq_test_client().await;
    let (interceptor, seen) = recorder("vendor:thing");
    let _handle = client.add_stanza_interceptor(interceptor);

    let node = NodeBuilder::new("vendor:thing")
        .attr("id", "V-1")
        .attr("from", "s.whatsapp.net")
        .build();
    // `should_ack` covers only the tags the client models, so this stanza takes
    // the claimed-stanza ack path rather than the deferred one.
    assert!(
        !client.should_ack(&node.as_node_ref()),
        "fixture must exercise the path should_ack does not cover"
    );
    client.process_node(node_to_owned_ref(node)).await;
    assert_eq!(seen.lock().expect("lock").len(), 1, "the interceptor ran");

    // What the server actually receives, which is the whole point: an ack, and
    // not the nack this tag would otherwise have drawn.
    let sent = crate::test_utils::decode_sent_iq(&transport, 0).await;
    let sent = sent.get();
    assert_eq!(sent.tag.as_ref(), "ack");
    assert!(
        sent.get_attr("class")
            .is_some_and(|class| *class == "vendor:thing")
    );
    assert!(sent.get_attr("id").is_some_and(|id| *id == "V-1"));
    assert_eq!(
        transport.sent_count(),
        1,
        "one answer, so no nack followed the ack"
    );
}

#[tokio::test]
async fn claiming_a_stanza_the_client_answers_differently_sends_no_generic_ack() {
    // A direct <message> is answered with a delivery <receipt>, an <iq> with an
    // <iq type="result">. Neither is an <ack class="…">, so the claimed-stanza
    // path stays quiet rather than sending the server something it did not ask
    // for. The interceptor that took the stanza owes the reply.
    let (client, transport) = crate::test_utils::create_iq_test_client().await;
    let (interceptor, seen) = recorder("message");
    let _handle = client.add_stanza_interceptor(interceptor);

    let node = NodeBuilder::new("message")
        .attr("id", "M-1")
        .attr("from", "5511999998888@s.whatsapp.net")
        .build();
    assert!(
        !client.should_ack(&node.as_node_ref()),
        "a direct message is not ack-answered"
    );
    client.process_node(node_to_owned_ref(node)).await;

    assert_eq!(seen.lock().expect("lock").len(), 1, "it was claimed");
    crate::test_utils::wait_for_outbound_tasks(&client).await;
    assert_eq!(
        transport.sent_count(),
        0,
        "no invented answer for a tag the client models"
    );
}

#[tokio::test]
async fn a_claimed_stanza_without_identity_is_left_alone() {
    // An ack has nothing to address without `id` and `from`, which is the same
    // condition the nack path checks.
    let client = create_interceptor_test_client().await;
    let (interceptor, seen) = recorder("vendor:thing");
    let _handle = client.add_stanza_interceptor(interceptor);

    client
        .process_node(node_to_owned_ref(NodeBuilder::new("vendor:thing").build()))
        .await;

    assert_eq!(seen.lock().expect("lock").len(), 1);
}

#[tokio::test]
async fn connection_critical_stanzas_are_never_offered_to_an_interceptor() {
    // An interceptor exists to extend a client, not to leave it
    // authenticated-but-unaware, never reconnecting, or waiting forever on a
    // send that already completed. These four settle connection state, so they
    // do not reach an interceptor at all — an interceptor that tried to claim
    // one never gets the chance.
    let client = create_interceptor_test_client().await;
    let (interceptor, seen) = recorder("claims-everything-it-sees");
    let _handle = client.add_stanza_interceptor(interceptor);

    for tag in ["success", "failure", "stream:error", "ack"] {
        client
            .process_node(node_to_owned_ref(NodeBuilder::new(tag).build()))
            .await;
    }

    assert!(
        seen.lock().expect("lock").is_empty(),
        "a connection-critical stanza must not reach an interceptor"
    );

    // A neighbouring tag is still offered, so the guard is a list and not a
    // switch that turned interception off.
    client
        .process_node(node_to_owned_ref(NodeBuilder::new("notification").build()))
        .await;
    assert_eq!(seen.lock().expect("lock").len(), 1);
}

#[tokio::test]
async fn a_server_ping_is_never_offered_to_an_interceptor() {
    // A claimed ping is a pong never sent, and the server closes the connection
    // over it — the same class of harm as claiming `success` or `ack`, so the
    // same protection. Every other <iq> stays offered: that is the traffic an
    // interceptor exists to extend.
    let client = create_interceptor_test_client().await;
    let (interceptor, seen) = recorder("claims-everything-it-sees");
    let _handle = client.add_stanza_interceptor(interceptor);

    for node in [
        NodeBuilder::new("iq")
            .attr("from", "s.whatsapp.net")
            .attr("id", "PING-1")
            .attr("type", "get")
            .children([NodeBuilder::new("ping").build()])
            .build(),
        // WA Web's handleIq is type-agnostic, so an absent type is a ping too.
        NodeBuilder::new("iq")
            .attr("from", "s.whatsapp.net")
            .attr("id", "PING-2")
            .attr("xmlns", "urn:xmpp:ping")
            .build(),
    ] {
        client.process_node(node_to_owned_ref(node)).await;
    }
    assert!(
        seen.lock().expect("lock").is_empty(),
        "a server ping must not reach an interceptor"
    );

    // A ping *response* is ours, not the server's, so it carries no pong
    // obligation and stays offered.
    let response = NodeBuilder::new("iq")
        .attr("from", "s.whatsapp.net")
        .attr("id", "PING-3")
        .attr("type", "result")
        .children([NodeBuilder::new("ping").build()])
        .build();
    client.process_node(node_to_owned_ref(response)).await;

    let other = NodeBuilder::new("iq")
        .attr("from", "s.whatsapp.net")
        .attr("id", "IQ-1")
        .attr("type", "get")
        .children([NodeBuilder::new("query").build()])
        .build();
    client.process_node(node_to_owned_ref(other)).await;
    assert_eq!(
        seen.lock().expect("lock").len(),
        2,
        "a ping result and an ordinary <iq> are both still offered"
    );
}

#[tokio::test]
async fn a_closure_can_be_an_interceptor() {
    let client = create_interceptor_test_client().await;
    let calls = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&calls);

    let _handle = client.add_stanza_interceptor(Arc::new(move |_node: &OwnedNodeRef| {
        counter.fetch_add(1, Ordering::Relaxed);
        Interception::Pass
    }));

    client
        .process_node(node_to_owned_ref(NodeBuilder::new("receipt").build()))
        .await;
    assert_eq!(calls.load(Ordering::Relaxed), 1);
}

#[tokio::test]
async fn a_handle_outliving_its_client_does_not_keep_it_alive() {
    // The handle holds a weak reference, so a forgotten one cannot pin a
    // client — and dropping it afterwards must not panic either.
    let handle = {
        let client = create_interceptor_test_client().await;
        let (interceptor, _seen) = recorder("nothing");
        let owners = Arc::strong_count(&client);
        let handle = client.add_stanza_interceptor(interceptor);
        assert_eq!(
            Arc::strong_count(&client),
            owners,
            "registering must not make the handle an owner"
        );
        handle
    };
    drop(handle);
}

/// A stanza that arrived can be sent back out as it stands: pack what the
/// decoder holds and the bytes reaching the transport are the ones the marshal
/// produced, no re-encode involved. This is the round trip a forwarding or
/// replay consumer performs, asserted at the point the frame leaves.
#[tokio::test]
async fn a_received_stanza_forwards_as_the_bytes_it_arrived_as() {
    use wacore::handshake::NoiseCipher;

    let (client, transport) = crate::test_utils::create_iq_test_client().await;

    let node = NodeBuilder::new("message")
        .attr("to", "15551234567@s.whatsapp.net")
        .attr("id", "FORWARD-1")
        .children([NodeBuilder::new("enc")
            .attr("type", "msg")
            .bytes(vec![0xAB; 64])
            .build()])
        .build();
    let marshaled = wacore_binary::marshal::marshal(&node).expect("marshal");
    let received = to_owned_node(&node);

    client
        .send_raw_bytes(wacore_binary::util::pack(&received.backing_bytes()))
        .await
        .expect("installed socket");

    crate::test_utils::poll_until("the forwarded frame to reach the transport", || {
        !transport.sent().is_empty()
    })
    .await;
    let cipher = NoiseCipher::new(&[0u8; 32]).expect("32-byte key");
    let mut sent = transport.sent()[0][3..].to_vec();
    cipher
        .decrypt_in_place_with_counter(0, &mut sent)
        .expect("captured frame should decrypt");

    assert_eq!(
        sent, marshaled,
        "the forwarded frame must carry the original marshal output"
    );
}

/// The other half: node bytes handed to the send path are refused here rather
/// than accepted and answered by the server closing the connection.
#[tokio::test]
async fn send_raw_bytes_refuses_a_payload_without_its_format_byte() {
    let (client, transport) = crate::test_utils::create_iq_test_client().await;

    let node = NodeBuilder::new("iq").attr("id", "REJECT-1").build();
    let node_bytes = to_owned_node(&node).backing_bytes().to_vec();

    let error = client
        .send_raw_bytes(node_bytes.clone())
        .await
        .expect_err("node bytes are not a packed payload");
    assert!(
        matches!(
            error,
            ClientError::Socket(SocketError::Marshal(
                wacore_binary::BinaryError::UnexpectedFormatByte(byte)
            )) if byte == node_bytes[0]
        ),
        "unexpected error: {error:?}"
    );
    for empty in [Vec::new(), vec![wacore_binary::util::FORMAT_PLAIN]] {
        assert!(
            matches!(
                client.send_raw_bytes(empty).await,
                Err(ClientError::Socket(SocketError::Marshal(
                    wacore_binary::BinaryError::EmptyData
                )))
            ),
            "a payload with no stanza in it carries nothing to send"
        );
    }
    assert!(
        transport.sent().is_empty(),
        "a refused payload must not reach the transport"
    );

    // The same stanza, packed, goes out.
    client
        .send_raw_bytes(wacore_binary::util::pack(&node_bytes))
        .await
        .expect("installed socket");
    assert_eq!(transport.sent().len(), 1);
}

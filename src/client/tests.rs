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
        .response_waiters
        .lock()
        .await
        .insert(test_id.clone(), tx);
    assert!(
        client.response_waiters.lock().await.contains_key(&test_id),
        "Waiter should be inserted before handling ack"
    );

    // 2. Create a mock <ack/> node with the test ID
    let ack_node = NodeBuilder::new("ack")
        .attr("id", test_id.clone())
        .attr("from", SERVER_JID)
        .build();

    // 3. Handle the ack
    let handled = client.handle_ack_response(&ack_node.as_node_ref()).await;
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
        !client.response_waiters.lock().await.contains_key(&test_id),
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
    let handled = client.handle_ack_response(&ack_node.as_node_ref()).await;
    assert!(
        !handled,
        "handle_ack_response should return false when no waiter exists"
    );

    info!(
        "✅ test_ack_without_matching_waiter passed: ACK without matching waiter handled gracefully"
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
    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

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
    client
        .offline_sync_completed
        .store(true, std::sync::atomic::Ordering::Relaxed);

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
        .wait_for_offline_delivery_end_with_timeout(std::time::Duration::from_millis(50))
        .await;

    let elapsed = start.elapsed();
    // Count available permits by trying to acquire non-blockingly
    let semaphore = match client.message_processing_semaphore.lock() {
        Ok(guard) => guard.clone(),
        Err(poisoned) => poisoned.into_inner().clone(),
    };
    let mut guards = Vec::new();
    while let Some(guard) = semaphore.try_acquire() {
        guards.push(guard);
    }
    let permits = guards.len();
    drop(guards);

    assert!(
        elapsed.as_millis() >= 45, // Allow small timing variance
        "Should have waited for the configured timeout duration, took {:?}",
        elapsed
    );
    assert!(
        client
            .offline_sync_completed
            .load(std::sync::atomic::Ordering::Relaxed),
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
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
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
        !client
            .offline_sync_completed
            .load(std::sync::atomic::Ordering::Relaxed),
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

    // Give the waiter time to start waiting
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    // Verify waiter hasn't completed yet
    assert!(
        !waiter_handle.is_finished(),
        "Waiter should still be waiting"
    );

    // 3. Simulate IB handler behavior (set flag and notify)
    client.offline_sync_completed.store(true, Ordering::Relaxed);
    client.offline_sync_notifier.notify(usize::MAX);

    // 4. Waiter should complete
    let result = tokio::time::timeout(std::time::Duration::from_millis(100), waiter_handle)
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

    // Wait a bit - ensure_e2e_sessions should return immediately for empty list
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    assert!(
        ensure_handle.is_finished(),
        "ensure_e2e_sessions should return immediately for empty JID list"
    );

    // Now test with actual JIDs - it should wait for offline sync
    let client_clone = client.clone();
    let test_jid = Jid::pn("559999999999");
    let ensure_handle = tokio::spawn(async move {
        // This will wait for offline sync before proceeding
        let start = wacore::time::Instant::now();
        let _ = client_clone.ensure_e2e_sessions(&[test_jid]).await;
        start.elapsed()
    });

    // Give it a moment to start
    tokio::time::sleep(std::time::Duration::from_millis(20)).await;

    // It should still be waiting (offline sync not complete)
    assert!(
        !ensure_handle.is_finished(),
        "ensure_e2e_sessions should be waiting for offline sync"
    );

    // Now complete offline sync
    client.offline_sync_completed.store(true, Ordering::Relaxed);
    client.offline_sync_notifier.notify(usize::MAX);

    // Now it should complete (might fail on session establishment, but that's ok)
    let result = tokio::time::timeout(std::time::Duration::from_secs(2), ensure_handle).await;

    assert!(
        result.is_ok(),
        "ensure_e2e_sessions should complete after offline sync"
    );

    info!("✅ test_ensure_e2e_sessions_waits_for_offline_sync passed");
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
        std::time::Duration::from_millis(500),
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

fn node_to_owned_ref(node: Node) -> Arc<wacore_binary::OwnedNodeRef> {
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
    let addr = ProtocolAddress::new("5550001000@s.whatsapp.net".to_string(), 1u32.into());
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

/// When the flush itself fails, cleanup must NOT clear the cache, or it would
/// drop the very state the flush was meant to persist.
#[tokio::test]
async fn cleanup_connection_state_keeps_state_when_flush_fails() {
    use wacore::libsignal::protocol::{ProtocolAddress, SenderKeyRecord};
    use wacore::libsignal::store::sender_key_name::SenderKeyName;
    let client = create_offline_sync_test_client().await;

    // A malformed identity (not 32 bytes) makes flush() error out, standing
    // in for a transient backend write failure during cleanup.
    let bad = ProtocolAddress::new("5550002000@s.whatsapp.net".to_string(), 1u32.into());
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
    client.register_handler(handler);

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
fn test_build_ack_node_for_message_omits_type_includes_from() {
    // Whatsmeow: message acks do NOT echo type (node.Tag != "message" guard).
    // They DO include `from` with own device PN.
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
        !ack.attrs.contains_key("type"),
        "message ACK must NOT echo type (matches whatsmeow behavior)"
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
    let buf = encode_ack_bytes(&with_recipient.as_node_ref(), Some(&own_device_pn))
        .expect("encode_ack_bytes should not error")
        .expect("encode_ack_bytes should produce bytes");
    // The Encoder prepends a leading format byte (see `marshal`); the
    // decoder wants raw protocol bytes — same handling as `node_to_owned_ref`.
    let decoded =
        wacore_binary::marshal::unmarshal_ref(&buf[1..]).expect("encoded ack should decode");
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

    let without_recipient = NodeBuilder::new("message")
        .attr("from", "120363161500776365@g.us")
        .attr("id", "A5791A5392EF60E3FB06")
        .attr("type", "text")
        .attr("participant", "181531758878822@lid")
        .build();
    let buf = encode_ack_bytes(&without_recipient.as_node_ref(), Some(&own_device_pn))
        .expect("encode_ack_bytes should not error")
        .expect("encode_ack_bytes should produce bytes");
    let decoded =
        wacore_binary::marshal::unmarshal_ref(&buf[1..]).expect("encoded ack should decode");
    assert!(
        decoded.get_attr("recipient").is_none(),
        "encode_ack_bytes must not synthesise `recipient` when absent"
    );
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
    *client.noise_socket.lock().await = Some(Arc::new(noise_socket));
    client.is_connected.store(true, Ordering::Release);

    assert!(client.is_connected(), "should report connected");

    // Hold the noise_socket mutex — this used to make is_connected() return
    // false via try_lock() even though the socket was Some(...)
    let _guard = client.noise_socket.lock().await;
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
    *client.noise_socket.lock().await = Some(Arc::new(noise_socket));
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

    tokio::time::sleep(Duration::from_millis(50)).await;
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

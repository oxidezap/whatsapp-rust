// Included inside retry::tests to exercise the existing private orchestrator.
// The producer traces qualify Web call order. These checks qualify core state
// outcomes only; locks, persistence and cancellation remain handwritten.
#[tokio::test]
async fn captured_web_session_scenarios_match_core_repair_outcomes() {
    use sha2::Digest;
    let bytes = include_bytes!("../tests/fixtures/retry-conformance/session-cases.json");
    assert_eq!(
        hex::encode(sha2::Sha256::digest(bytes)),
        "aadc079a167af576f3ee7649d86cd91e4ac83e6029951c5f23ca5ab54205c9b9",
        "pinned retry oracle fixture drift"
    );
    let fixture: serde_json::Value = serde_json::from_slice(bytes).unwrap();
    let mut exercised = Vec::new();
    for case in fixture["cases"].as_array().unwrap() {
        let name = case["name"].as_str().unwrap();
        // These rows cross orchestration boundaries that this private method
        // does not own. The exclusions are explicit, not silently skipped.
        if matches!(
            name,
            "peer-mismatch"
                | "bundle-handled-mismatch"
                | "group"
                | "status-same-base"
                | "broadcast"
                | "missing-registration"
        ) {
            continue;
        }
        let client = retry_repair_client("retry_conformance_outcomes").await;
        let requester = Jid::lid_device("100000000000901".to_string(), 7);
        let address = requester.to_protocol_address();
        let message = "SYNTHETIC-RETRY";
        let base = vec![0x42; 32];
        let backend = client.persistence_manager.backend();
        backend
            .put_session(
                address.as_str(),
                &valid_serialized_session(
                    case["savedRegId"].as_u64().unwrap() as u32,
                    base.clone(),
                ),
            )
            .await
            .unwrap();
        if case["sameBaseKey"].as_bool().unwrap() {
            backend
                .save_base_key(address.as_str(), message, &base)
                .await
                .unwrap();
        }
        let node = build_retry_receipt_with_registration(7);
        client
            .reconcile_retry_session(
                &requester,
                message,
                case["retryCount"].as_u64().unwrap() as u8,
                &node.as_node_ref(),
            )
            .await;
        client.flush_signal_cache().await.unwrap();
        let expected_delete = case["effects"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e == "deleteSession");
        assert_eq!(
            backend
                .get_session(address.as_str())
                .await
                .unwrap()
                .is_none(),
            expected_delete,
            "{name}: session outcome differs from isolated Web oracle"
        );
        let stamped = backend
            .has_same_base_key(address.as_str(), message, &base)
            .await
            .unwrap();
        // Web calls save after a registration-mismatch delete at retry 2.
        // Core has no session to stamp and intentionally returns after delete.
        let expected_stamp = name == "save-at-two";
        assert_eq!(stamped, expected_stamp, "{name}: base-key stamp outcome");
        exercised.push(name);
    }
    assert_eq!(
        exercised,
        [
            "first",
            "save-at-two",
            "different-base",
            "same-base",
            "mismatch-continues-at-two"
        ]
    );
}

#[tokio::test]
async fn captured_retry_repair_cancelled_while_waiting_for_lock_has_no_effects() {
    let client = retry_repair_client("retry_conformance_cancel").await;
    let requester = Jid::lid_device("100000000000902".to_string(), 7);
    let address = requester.to_protocol_address();
    let base = vec![0x43; 32];
    let backend = client.persistence_manager.backend();
    let original = valid_serialized_session(8, base.clone());
    backend
        .put_session(address.as_str(), &original)
        .await
        .unwrap();
    let lock = client.session_lock_for(address.as_str()).await;
    let guard = lock.lock().await;
    let node = build_retry_receipt_with_registration(7);
    let node_ref = node.as_node_ref();
    let mut repair =
        Box::pin(client.reconcile_retry_session(&requester, "CANCELLED-RETRY", 2, &node_ref));
    // Poll deterministically into the held mutex, without relying on scheduling.
    assert!(
        std::future::poll_fn(|cx| std::task::Poll::Ready(Future::poll(repair.as_mut(), cx)))
            .await
            .is_pending()
    );
    drop(repair);
    drop(guard);
    client.flush_signal_cache().await.unwrap();
    assert_eq!(
        backend.get_session(address.as_str()).await.unwrap(),
        Some(original.into())
    );
    assert!(
        !backend
            .has_same_base_key(address.as_str(), "CANCELLED-RETRY", &base)
            .await
            .unwrap()
    );
}

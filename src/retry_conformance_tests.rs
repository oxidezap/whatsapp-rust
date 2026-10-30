// Included inside retry::tests to exercise the existing private orchestrator.
// The producer traces qualify Web call order. These checks qualify core state
// outcomes only; locks, persistence and cancellation remain handwritten.
#[tokio::test]
async fn captured_web_session_scenarios_match_core_repair_outcomes() {
    use sha2::Digest;
    let bytes = include_bytes!("../tests/fixtures/retry-conformance/session-cases.json");
    assert_eq!(
        hex::encode(sha2::Sha256::digest(bytes)),
        "dc9412eef9d9241262395948b80745c320922ce6b98a61405748dc4db0a6c0b6",
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

/// The official session generator awaits deleteRemoteSession before continuing.
/// Core adds a persistence boundary: cancellation after the backend committed
/// must retain the cache tombstone until a later flush acknowledges it.
#[tokio::test]
async fn captured_retry_cancel_after_durable_delete_keeps_tombstone_recoverable() {
    use crate::store::release::tests::ProbeBackend;
    let (mut probe, _) = ProbeBackend::new();
    let raw = probe.backend.clone();
    let (entered_tx, entered_rx) = async_channel::bounded(1);
    let (_release_tx, release_rx) = async_channel::bounded(1);
    probe.gate_method = "after:delete_sessions_batch";
    probe.entered = Some(entered_tx);
    probe.gate = Some(release_rx);
    let client = crate::test_utils::create_test_client_with_backend(Arc::new(probe)).await;
    let requester = Jid::lid_device("100000000000903".to_string(), 7);
    let address = requester.to_protocol_address();
    raw.put_session(
        address.as_str(),
        &valid_serialized_session(8, vec![0x44; 32]),
    )
    .await
    .unwrap();
    let task = {
        let client = Arc::clone(&client);
        let requester = requester.clone();
        tokio::spawn(async move {
            let receipt = build_retry_receipt_with_registration(7);
            client
                .reconcile_retry_session(&requester, "AFTER-DELETE", 2, &receipt.as_node_ref())
                .await;
        })
    };
    tokio::time::timeout(std::time::Duration::from_secs(5), entered_rx.recv())
        .await
        .expect("repair must reach post-delete boundary")
        .unwrap();
    assert!(
        raw.get_session(address.as_str()).await.unwrap().is_none(),
        "backend deletion committed before cancellation"
    );
    assert!(
        !client
            .signal_cache
            .has_session(&address, raw.as_ref())
            .await
            .unwrap(),
        "cache must not resurrect the deleted session"
    );
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    // Prove retention by replaying through the gated decorator again. Merely
    // observing an absent durable row would also pass if cancellation wrongly
    // discarded the pending tombstone after the first delete committed.
    let replay = {
        let client = Arc::clone(&client);
        tokio::spawn(async move { client.flush_signal_cache().await })
    };
    tokio::time::timeout(std::time::Duration::from_secs(5), entered_rx.recv())
        .await
        .expect("cancelled deletion must be replayed before acknowledgement")
        .unwrap();
    assert!(raw.get_session(address.as_str()).await.unwrap().is_none());
    replay.abort();
    assert!(replay.await.unwrap_err().is_cancelled());
    // Settle the retained tombstone against the same healthy backend.
    client.signal_cache.flush(raw.as_ref()).await.unwrap();
    assert!(
        !client
            .signal_cache
            .has_session(&address, raw.as_ref())
            .await
            .unwrap()
    );
    let mut settled = Box::pin(client.flush_signal_cache());
    let result =
        std::future::poll_fn(|cx| std::task::Poll::Ready(Future::poll(settled.as_mut(), cx))).await;
    assert!(
        matches!(result, std::task::Poll::Ready(Ok(()))),
        "acknowledged tombstone must not repeat gated delete I/O"
    );
}

#[tokio::test]
async fn captured_retry_failed_delete_flush_remains_recoverable_before_and_after_commit() {
    use crate::store::release::tests::ProbeBackend;
    for after_commit in [false, true] {
        let (mut probe, _) = ProbeBackend::new();
        let raw = probe.backend.clone();
        if after_commit {
            probe.fail_after_method = Some("delete_sessions_batch");
        } else {
            probe.fail_before_method = Some((
                "delete_sessions_batch",
                Arc::new(std::sync::atomic::AtomicBool::new(true)),
            ));
        }
        let client = crate::test_utils::create_test_client_with_backend(Arc::new(probe)).await;
        let requester = Jid::lid_device("100000000000904".to_string(), 7);
        let address = requester.to_protocol_address();
        raw.put_session(
            address.as_str(),
            &valid_serialized_session(8, vec![0x45; 32]),
        )
        .await
        .unwrap();
        let receipt = build_retry_receipt_with_registration(7);
        // This handwritten wrapper logs persistence failure. Its cache view
        // must remain deleted even when durable deletion did not commit.
        client
            .reconcile_retry_session(&requester, "DELETE-FAILURE", 2, &receipt.as_node_ref())
            .await;
        assert_eq!(
            raw.get_session(address.as_str()).await.unwrap().is_none(),
            after_commit
        );
        assert!(
            !client
                .signal_cache
                .has_session(&address, raw.as_ref())
                .await
                .unwrap()
        );
        let error = client
            .flush_signal_cache()
            .await
            .expect_err("pending tombstone must retry failed deletion");
        assert!(format!("{error:#}").contains(if after_commit {
            "synthetic failure after commit"
        } else {
            "synthetic failure before commit"
        }));
        client.signal_cache.flush(raw.as_ref()).await.unwrap();
        assert!(raw.get_session(address.as_str()).await.unwrap().is_none());
        assert!(
            !client
                .signal_cache
                .has_session(&address, raw.as_ref())
                .await
                .unwrap()
        );
    }
}

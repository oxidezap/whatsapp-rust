//! Signal session management e2e tests.

use e2e_tests::{TestClient, established_sessions, scan_sessions, send_and_expect_text};
use log::info;
use wacore::libsignal::protocol::SessionRecord;

/// Read the sender-chain counter of the first established session for `user`
/// straight from the backend (no cache, no settle) — the durable outbound
/// ratchet position.
async fn durable_sender_chain_index(
    backend: &dyn wacore::store::traits::SignalStore,
    user: &str,
    server: &str,
) -> anyhow::Result<Option<u32>> {
    for device_id in 0..=99u16 {
        let addr = if device_id == 0 {
            format!("{user}@{server}.0")
        } else {
            format!("{user}:{device_id}@{server}.0")
        };
        if let Some(data) = backend.get_session(&addr).await?
            && let Some(state) = SessionRecord::deserialize(&data)?.session_state()
            && let Ok(chain) = state.get_sender_chain_key()
        {
            return Ok(Some(chain.index()));
        }
    }
    Ok(None)
}

/// The durable snapshot must always be able to resume PAST every counter that
/// may have hit the wire: reusing an outbound counter reuses its message key +
/// IV. Counters are leased in batches (`SENDER_CHAIN_RESERVATION_BATCH`): the
/// send that raises the lease flushes synchronously before the wire, and every
/// lease-covered send may defer its advance to the coalesced write-behind
/// because `SessionRecord::deserialize` fast-forwards the reloaded chain past
/// the whole lease. This reads the backend IMMEDIATELY after `send_message`
/// (no delivery wait, no settle): the resume position — what a crash restore
/// would actually use — must already cover every counter spent so far.
#[tokio::test]
async fn test_durable_resume_position_always_covers_spent_counters() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let client_a = TestClient::connect("e2e_sig_durable_a").await?;
    let client_b = TestClient::connect("e2e_sig_durable_b").await?;
    let jid_b = client_b.jid().await;
    let lid_b = client_b.client.lid();

    // The first send raises the lease, so its flush is synchronous: the
    // durable resume position must already be past counter 0 the moment
    // send_message returns, with no settle.
    client_a
        .client
        .send_message(jid_b.clone(), e2e_tests::text_msg("establish"))
        .await?;

    let backend_a = client_a.client.persistence_manager().backend();
    let read_index = async |user: &str, server: &str| {
        durable_sender_chain_index(&*backend_a, user, server).await
    };

    // The session may be keyed under LID (modern) or PN.
    let (user, server) = match lid_b {
        Some(ref lid) if read_index(&lid.user, "lid").await?.is_some() => (lid.user.clone(), "lid"),
        _ => (jid_b.user.clone(), "c.us"),
    };

    // `durable_sender_chain_index` deserializes the stored record, which
    // applies the crash-restore fast-forward: this IS the resume position.
    let mut last = read_index(&user, server)
        .await?
        .expect("the lease raise must persist the session before the wire");
    let mut spent = 1u32; // counter 0 went out with "establish"
    assert!(
        last >= spent,
        "resume position {last} must cover the {spent} spent counter(s)"
    );

    // Lease-covered sends may leave the durable snapshot trailing (that is
    // the optimization), but the resume position must never fall behind the
    // wire and never regress.
    for i in 0..3 {
        client_a
            .client
            .send_message(jid_b.clone(), e2e_tests::text_msg(&format!("m{i}")))
            .await?;
        spent += 1;
        let now = read_index(&user, server)
            .await?
            .expect("session persists across sends");
        assert!(
            now >= spent,
            "send #{i}: resume position {now} fell behind the {spent} spent counter(s); \
             a crash here would re-derive a (key, IV) pair"
        );
        assert!(now >= last, "resume position must never regress");
        last = now;
    }

    client_a.disconnect().await;
    client_b.disconnect().await;
    Ok(())
}

/// A send that RAISES the counter lease gates the wire on persisting it: if
/// the flush fails, `send_message` returns `Err` and the peer receives
/// nothing. Otherwise a crash after a wire-committed send would leave the
/// lease only in memory and a reload would re-derive that counter's key + IV.
/// The first send on a fresh session always raises the lease, so the failure
/// is injected before it.
#[tokio::test]
async fn test_send_aborts_before_wire_when_lease_persist_fails() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let client_a = TestClient::connect("e2e_sig_abort_a").await?;
    let mut client_b = TestClient::connect("e2e_sig_abort_b").await?;
    let jid_b = client_b.jid().await;

    // Persisting the outbound lease now fails.
    client_a.backend.set_fail_session_writes(true);
    let writes_before = client_a.backend.session_batch_write_count();
    // `send_node` resolves this before marshaling the node, so a still-pending
    // waiter proves the send aborted before reaching the wire.
    let mut sent_waiter = client_a.next_sent_message_waiter();
    let result = client_a
        .client
        .send_message(
            jid_b.clone(),
            e2e_tests::text_msg("must not reach the wire"),
        )
        .await;
    assert!(
        result.is_err(),
        "send must fail when the raised lease cannot be persisted, got {result:?}"
    );
    // The send reached the (failing) persistence step, proving the lease flush
    // runs on the send path before the wire rather than being skipped or deferred.
    assert!(
        client_a.backend.session_batch_write_count() > writes_before,
        "the send path must attempt to persist the raised lease before the wire"
    );
    // Deterministic: no `message` node was ever marshaled, so `send_node` (and
    // thus the wire) was never reached. `Ok(None)` == pending, sender still alive.
    assert!(
        matches!(sent_waiter.try_recv(), Ok(None)),
        "the send must abort before send_node marshals the stanza for the wire"
    );

    // End-to-end corroboration: the stanza never went out, so B never sees it.
    client_b
        .assert_no_event(
            3,
            |e| {
                e.messages()
                    .any(|m| m.message.conversation.as_deref() == Some("must not reach the wire"))
            },
            "a send that failed to persist must not deliver",
        )
        .await?;

    // Recovery: once persistence works again, sends deliver normally, proving
    // the failure aborted cleanly rather than wedging the session.
    client_a.backend.set_fail_session_writes(false);
    send_and_expect_text(
        &client_a.client,
        &mut client_b,
        &jid_b,
        "after recovery",
        30,
    )
    .await?;

    client_a.disconnect().await;
    client_b.disconnect().await;
    Ok(())
}

/// The counterpart of the abort test: a send COVERED by an already-durable
/// lease does not depend on this flush for safety — a crash would reload the
/// durable snapshot and fast-forward past the whole lease, so its counter can
/// never be re-derived. Such a send must therefore succeed even while the
/// backend is refusing session writes (the advance lands later via the
/// coalescer's retry), instead of turning a storage hiccup into message loss.
#[tokio::test]
async fn test_lease_covered_send_survives_persist_failure() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let mut client_a = TestClient::connect("e2e_sig_covered_a").await?;
    let mut client_b = TestClient::connect("e2e_sig_covered_b").await?;
    let jid_a = client_a.jid().await;
    let jid_b = client_b.jid().await;

    // Establish both ways: the lease raise happens (and is persisted) here.
    send_and_expect_text(&client_a.client, &mut client_b, &jid_b, "establish", 30).await?;
    send_and_expect_text(&client_b.client, &mut client_a, &jid_a, "reply", 30).await?;

    // Storage starts refusing session writes; the next send is lease-covered,
    // so it must still deliver.
    client_a.backend.set_fail_session_writes(true);
    send_and_expect_text(
        &client_a.client,
        &mut client_b,
        &jid_b,
        "covered by the lease",
        30,
    )
    .await?;

    client_a.backend.set_fail_session_writes(false);
    client_a.disconnect().await;
    client_b.disconnect().await;
    Ok(())
}

/// Multiple sequential sends without a reply should all be delivered.
#[tokio::test]
async fn test_one_way_multiple_sends() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let client_a = TestClient::connect("e2e_sig_oneway_a").await?;
    let mut client_b = TestClient::connect("e2e_sig_oneway_b").await?;

    let jid_b = client_b.jid().await;

    let mut sent_ids = Vec::new();
    for i in 1..=5 {
        let text = format!("One-way message {i}");
        let msg_id =
            send_and_expect_text(&client_a.client, &mut client_b, &jid_b, &text, 30).await?;
        assert!(
            !sent_ids.contains(&msg_id),
            "Message IDs must be unique, got duplicate: {msg_id}"
        );
        sent_ids.push(msg_id);
        info!("Message {i}/5 delivered");
    }

    assert_eq!(sent_ids.len(), 5, "All 5 messages should have unique IDs");

    client_a.disconnect().await;
    client_b.disconnect().await;
    Ok(())
}

/// Bidirectional messaging: alternating sends between A and B.
#[tokio::test]
async fn test_bidirectional_exchange() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let mut client_a = TestClient::connect("e2e_sig_bidir_a").await?;
    let mut client_b = TestClient::connect("e2e_sig_bidir_b").await?;

    let jid_a = client_a.jid().await;
    let jid_b = client_b.jid().await;

    send_and_expect_text(&client_a.client, &mut client_b, &jid_b, "A1→B", 30).await?;
    info!("A→B delivered");

    send_and_expect_text(&client_b.client, &mut client_a, &jid_a, "B1→A", 30).await?;
    info!("B→A delivered");

    send_and_expect_text(&client_a.client, &mut client_b, &jid_b, "A2→B", 30).await?;
    info!("A→B (round 2) delivered");

    send_and_expect_text(&client_b.client, &mut client_a, &jid_a, "B2→A", 30).await?;
    info!("B→A (round 2) delivered");

    // Rapid alternating
    for i in 3..=5 {
        send_and_expect_text(
            &client_a.client,
            &mut client_b,
            &jid_b,
            &format!("A{i}→B"),
            30,
        )
        .await?;
        send_and_expect_text(
            &client_b.client,
            &mut client_a,
            &jid_a,
            &format!("B{i}→A"),
            30,
        )
        .await?;
    }
    info!("All 10 bidirectional messages delivered");

    client_a.disconnect().await;
    client_b.disconnect().await;
    Ok(())
}

/// After a roundtrip (A->B->A), the active LID session should have
/// `pending_pre_key` cleared.
#[tokio::test]
async fn test_session_state_after_roundtrip() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let mut client_a = TestClient::connect("e2e_sig_state_a").await?;
    let mut client_b = TestClient::connect("e2e_sig_state_b").await?;

    let jid_a = client_a.jid().await;
    let jid_b = client_b.jid().await;

    // Roundtrip: A→B, B→A
    send_and_expect_text(
        &client_a.client,
        &mut client_b,
        &jid_b,
        "Establishing session",
        30,
    )
    .await?;
    send_and_expect_text(&client_b.client, &mut client_a, &jid_a, "Session reply", 30).await?;
    info!("Roundtrip complete");

    // Settle A's write-behind Signal cache before inspecting it: A's last op
    // here is receiving B's reply, and the receive-path flush is coalesced, so
    // a plain read races the coalescing window.
    client_a.client.flush_pending_signal_state().await?;

    // Inspect session state
    let backend = client_a.client.persistence_manager().backend();

    // PN sessions: may exist with stale pending_pre_key (orphaned after LID migration)
    let pn_sessions = established_sessions(&scan_sessions(&*backend, &jid_b.user, "c.us").await?);
    for (addr, pending) in &pn_sessions {
        info!("PN session {addr}: pending_pre_key={pending}");
    }

    // LID sessions: the active sessions used by encrypt_for_devices
    let mut lid_sessions = Vec::new();
    if let Some(lid) = client_b.client.lid() {
        lid_sessions = established_sessions(&scan_sessions(&*backend, &lid.user, "lid").await?);
        for (addr, pending) in &lid_sessions {
            info!("LID session {addr}: pending_pre_key={pending}");
        }
    }

    // Must have at least one session (PN or LID) for B
    assert!(
        !pn_sessions.is_empty() || !lid_sessions.is_empty(),
        "Should have at least one session for B in A's store"
    );

    // If LID sessions exist (expected with modern mock server), the active session
    // used for messaging must have pending_pre_key cleared after the roundtrip. B is
    // a companion, so its session is device-suffixed (`<lid>:33@lid.0`) — there is no
    // bare device-0 "primary" session to key on; assert that at least one of B's LID
    // sessions is fully established.
    if !lid_sessions.is_empty() {
        let established = lid_sessions.iter().any(|(_, pending)| !pending);
        assert!(
            established,
            "At least one LID session should have pending_pre_key cleared after roundtrip. \
             LID sessions: {lid_sessions:?}"
        );
    }

    // Verify continued delivery works after session inspection
    for i in 1..=3 {
        send_and_expect_text(
            &client_a.client,
            &mut client_b,
            &jid_b,
            &format!("Post-inspection {i}"),
            30,
        )
        .await?;
    }
    info!("Post-inspection messages delivered");

    client_a.disconnect().await;
    client_b.disconnect().await;
    Ok(())
}

/// Sessions must be persisted to the SQLite backend after sending.
#[tokio::test]
async fn test_session_persistence() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let client_a = TestClient::connect("e2e_sig_persist_a").await?;
    let mut client_b = TestClient::connect("e2e_sig_persist_b").await?;

    let jid_b = client_b.jid().await;

    let backend = client_a.client.persistence_manager().backend();

    // No session should exist before first contact
    let pre_send = established_sessions(&scan_sessions(&*backend, &jid_b.user, "c.us").await?);
    assert!(
        pre_send.is_empty(),
        "No PN session should exist before first send, found: {pre_send:?}"
    );

    // First send creates and persists session
    let msg_id_1 = send_and_expect_text(
        &client_a.client,
        &mut client_b,
        &jid_b,
        "Persist test 1",
        30,
    )
    .await?;
    info!("First message sent: {msg_id_1}");

    // No settle: the send path flushes synchronously, so the session is durable
    // in the backend by the time send_message returned. (A missing session here
    // would catch a regression back to a coalesced/deferred send flush.)

    // Session may be under PN (c.us) or LID (lid) depending on whether
    // PN→LID mapping was resolved before encryption.
    let mut post_send = established_sessions(&scan_sessions(&*backend, &jid_b.user, "c.us").await?);
    if post_send.is_empty()
        && let Some(lid_b) = client_b.client.lid()
    {
        post_send = established_sessions(&scan_sessions(&*backend, &lid_b.user, "lid").await?);
    }
    assert!(
        !post_send.is_empty(),
        "At least one session (PN or LID) should be persisted after first send"
    );

    // All persisted sessions should have pending_pre_key=true (no reply yet)
    for (addr, pending) in &post_send {
        assert!(
            *pending,
            "Session {addr} should have pending_pre_key=true before any reply"
        );
    }
    info!(
        "Verified {} sessions persisted with pending_pre_key=true",
        post_send.len()
    );

    // Second send reuses sessions
    let msg_id_2 = send_and_expect_text(
        &client_a.client,
        &mut client_b,
        &jid_b,
        "Persist test 2",
        30,
    )
    .await?;
    assert_ne!(
        msg_id_1, msg_id_2,
        "Second message should have a different ID"
    );
    info!("Second message delivered with reused session");

    client_a.disconnect().await;
    client_b.disconnect().await;
    Ok(())
}

/// Sessions must survive a reconnect (loaded from SQLite after cache clear).
#[tokio::test]
async fn test_session_survives_reconnect() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let mut client_a = TestClient::connect("e2e_sig_reconnect_a").await?;
    let mut client_b = TestClient::connect("e2e_sig_reconnect_b").await?;

    let jid_a = client_a.jid().await;
    let jid_b = client_b.jid().await;

    // Establish sessions with a full roundtrip
    send_and_expect_text(
        &client_a.client,
        &mut client_b,
        &jid_b,
        "Pre-reconnect A→B",
        30,
    )
    .await?;
    send_and_expect_text(
        &client_b.client,
        &mut client_a,
        &jid_a,
        "Pre-reconnect B→A",
        30,
    )
    .await?;
    info!("Sessions established");

    // Reconnect A — clears in-memory cache, forces DB reload
    client_a.reconnect_and_wait().await?;
    info!("Client A reconnected (cache cleared)");

    // Send after reconnect — must load session from DB
    send_and_expect_text(
        &client_a.client,
        &mut client_b,
        &jid_b,
        "Post-reconnect 1",
        30,
    )
    .await?;
    info!("Post-reconnect message delivered");

    // Multiple messages to confirm session is stable after reload
    for i in 2..=4 {
        send_and_expect_text(
            &client_a.client,
            &mut client_b,
            &jid_b,
            &format!("Post-reconnect {i}"),
            30,
        )
        .await?;
    }
    info!("All post-reconnect messages delivered");

    // Also verify B→A still works after A's reconnect
    send_and_expect_text(
        &client_b.client,
        &mut client_a,
        &jid_a,
        "B→A after reconnect",
        30,
    )
    .await?;
    info!("B→A after A's reconnect delivered");

    client_a.disconnect().await;
    client_b.disconnect().await;
    Ok(())
}

/// Verify that MessageInfo fields are correctly populated on received messages.
#[tokio::test]
async fn test_message_info_fields() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let mut client_a = TestClient::connect("e2e_sig_info_a").await?;
    let mut client_b = TestClient::connect("e2e_sig_info_b").await?;

    let jid_a = client_a.jid().await;
    let jid_b = client_b.jid().await;

    // A→B: verify B's MessageInfo
    let text_ab = "Info check A→B";
    client_a
        .client
        .send_message(jid_b.clone(), e2e_tests::text_msg(text_ab))
        .await?;

    let event = client_b.wait_for_text(text_ab, 30).await?;

    if let Some(m) = event
        .messages()
        .find(|m| m.message.conversation.as_deref() == Some(text_ab))
    {
        let (msg, info) = (&m.message, &m.info);
        assert_eq!(msg.conversation.as_deref(), Some(text_ab));
        assert!(!info.id.is_empty(), "Message ID must not be empty");
        assert!(
            info.timestamp.timestamp() > 0,
            "Timestamp should be positive, got {}",
            info.timestamp
        );
        assert!(
            !info.source.is_from_me,
            "B should see A's message as not from_me"
        );
        assert!(!info.source.is_group, "DM should not be marked as group");
        // A 1:1 message is LID-addressed on the wire (compliant), so B sees A's LID
        // as the sender (with the PN carried in sender_pn). Accept either identity.
        let sender_user = info.source.sender.user.as_str();
        let a_lid = client_a.client.lid();
        assert!(
            sender_user == jid_a.user.as_str()
                || a_lid
                    .as_ref()
                    .is_some_and(|l| l.user.as_str() == sender_user),
            "Sender should be A (PN {} or LID {:?}), got {sender_user}",
            jid_a.user,
            a_lid.as_ref().map(|l| l.user.as_str()),
        );
        info!(
            "MessageInfo verified: id={}, sender={}, timestamp={}, from_me={}, is_group={}",
            info.id,
            info.source.sender,
            info.timestamp,
            info.source.is_from_me,
            info.source.is_group
        );
    }

    // B→A: verify A's MessageInfo
    let text_ba = "Info check B→A";
    client_b
        .client
        .send_message(jid_a.clone(), e2e_tests::text_msg(text_ba))
        .await?;

    let event = client_a.wait_for_text(text_ba, 30).await?;

    if let Some(m) = event
        .messages()
        .find(|m| m.message.conversation.as_deref() == Some(text_ba))
    {
        let info = &m.info;
        assert!(!info.source.is_from_me);
        assert!(!info.source.is_group);
        // LID-addressed 1:1 → A sees B's LID as the sender; accept PN or LID.
        let sender_user = info.source.sender.user.as_str();
        let b_lid = client_b.client.lid();
        assert!(
            sender_user == jid_b.user.as_str()
                || b_lid
                    .as_ref()
                    .is_some_and(|l| l.user.as_str() == sender_user),
            "Sender should be B (PN {} or LID {:?}), got {sender_user}",
            jid_b.user,
            b_lid.as_ref().map(|l| l.user.as_str()),
        );
        info!("Reverse direction MessageInfo verified");
    }

    client_a.disconnect().await;
    client_b.disconnect().await;
    Ok(())
}

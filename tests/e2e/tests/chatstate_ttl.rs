use e2e_tests::{TestClient, text_msg};
use log::info;
use wacore::types::events::Event;

/// Requires mock server with CHATSTATE_TTL_SECS=3 (so TTL expires before the ~5s reconnect).
#[tokio::test]
async fn test_expired_chatstate_not_delivered() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let client_a = TestClient::connect("e2e_off_chatstate_a").await?;
    let mut client_b = TestClient::connect("e2e_off_chatstate_b").await?;

    let jid_b = client_b.jid().await;

    info!("B={jid_b}");

    // reconnect() uses RECONNECT_BACKOFF_STEP to create a ~5s offline window.
    // With CHATSTATE_TTL_SECS=3, the chatstate expires at 3s and B reconnects
    // at ~5s, so the drain filters it out.
    client_b.client.reconnect().await;
    info!("B disconnected (will auto-reconnect after backoff)");
    client_b.wait_for_disconnected(5).await?;

    client_a.client.chatstate().send_composing(&jid_b).await?;
    info!("A sent typing indicator to offline B");

    // Queued behind the chatstate, and the barrier for the assertion below: the
    // server drains B's queue in order, so an unexpired chatstate would come out
    // of it before this message does. The old form waited fifteen seconds for
    // nothing and called the silence a result.
    let after_ttl = "sent after the chatstate expired";
    client_a
        .client
        .send_message(jid_b.clone(), text_msg(after_ttl))
        .await?;

    client_b
        .assert_no_event_before(
            30,
            |e| matches!(e, Event::ChatPresence(_)),
            |e| {
                e.messages()
                    .any(|m| m.message.conversation.as_deref() == Some(after_ttl))
            },
            "B should NOT receive a chatstate that expired while it was offline",
        )
        .await?;
    info!("Confirmed: expired chatstate was NOT delivered to B");

    client_a.disconnect().await;
    client_b.disconnect().await;

    Ok(())
}

/// Uses `reconnect_immediately()` instead of `reconnect()` to ensure B is offline
/// for less than the TTL (3s with CHATSTATE_TTL_SECS=3).
#[tokio::test]
async fn test_fresh_chatstate_delivered_on_reconnect() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let client_a = TestClient::connect("e2e_off_chatstate_fresh_a").await?;
    let mut client_b = TestClient::connect("e2e_off_chatstate_fresh_b").await?;

    let jid_b = client_b.jid().await;

    info!("B={jid_b}");

    // reconnect_immediately() causes near-instant reconnect, keeping B offline
    // well within the 3s TTL window
    client_b.client.reconnect_immediately().await;
    info!("B disconnected (will reconnect immediately)");
    client_b.wait_for_disconnected(5).await?;

    client_a.client.chatstate().send_composing(&jid_b).await?;
    info!("A sent typing indicator to offline B");

    let event = client_b
        .wait_for_event(15, |e| matches!(e, Event::ChatPresence(_)))
        .await
        .expect("B should receive fresh chatstate within TTL");

    info!("B received fresh chatstate: {:?}", event);

    client_a.disconnect().await;
    client_b.disconnect().await;

    Ok(())
}

use e2e_tests::{TestClient, text_msg};
use log::info;
use wacore::types::events::Event;

/// Requires mock server with CHATSTATE_TTL_SECS=3.
#[tokio::test]
async fn test_expired_chatstate_not_delivered() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let client_a = TestClient::connect("e2e_off_chatstate_a").await?;
    let mut client_b = TestClient::connect("e2e_off_chatstate_b").await?;

    let jid_b = client_b.jid().await;

    info!("B={jid_b}");

    client_b.go_offline().await?;

    client_a.client.chatstate().send_composing(&jid_b).await?;
    info!("A sent typing indicator to offline B");

    // Expiry is the behavior under test. Hold the pause past the mock's TTL;
    // production reconnect backoff must not determine this offline window.
    tokio::time::sleep(std::time::Duration::from_secs(4)).await;

    // Queued behind the chatstate, and the barrier for the assertion below: the
    // server drains B's queue in order, so an unexpired chatstate would come out
    // of it before this message does. The old form waited fifteen seconds for
    // nothing and called the silence a result.
    let after_ttl = "sent after the chatstate expired";
    client_a
        .client
        .send_message(jid_b.clone(), text_msg(after_ttl))
        .await?;

    client_b.come_back_online();

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

/// Confirms the server observed B going offline before queueing a fresh
/// chatstate, then reconnects within its TTL (CHATSTATE_TTL_SECS=3).
#[tokio::test]
async fn test_fresh_chatstate_delivered_on_reconnect() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let mut client_a = TestClient::connect("e2e_off_chatstate_fresh_a").await?;
    let mut client_b = TestClient::connect("e2e_off_chatstate_fresh_b").await?;

    let jid_b = client_b.jid().await;

    info!("B={jid_b}");

    // Establish a server-observed availability transition. `pause()` closes
    // the local socket but does not await the peer's Close acknowledgement;
    // its disconnected flag alone cannot prove the server retired B's route.
    client_b.client.presence().set_available().await?;
    client_a.client.presence().subscribe(&jid_b).await?;
    client_a
        .wait_for_event(5, |event| {
            matches!(event,
                Event::Presence(update) if update.from == jid_b
                    && update.status == wacore::types::presence::PresenceStatus::Available
            )
        })
        .await?;
    client_b.go_offline().await?;
    client_a
        .wait_for_event(5, |event| {
            matches!(event,
                Event::Presence(update) if update.from == jid_b
                    && update.status == wacore::types::presence::PresenceStatus::Unavailable
            )
        })
        .await?;
    info!("Server observed B offline before the chatstate send");

    client_a.client.chatstate().send_composing(&jid_b).await?;
    info!("A sent typing indicator to offline B");

    // Sending a chatstate only completes the local transport write. A reply
    // to a following IQ establishes a receive barrier on A's connection
    // before B reconnects and the server drains its offline queue.
    client_a
        .client
        .execute(wacore::iq::keepalive::KeepaliveSpec::with_timeout(
            std::time::Duration::from_secs(2),
        ))
        .await?;
    client_b.come_back_online();

    let event = client_b
        .wait_for_event(15, |e| matches!(e, Event::ChatPresence(_)))
        .await
        .expect("B should receive fresh chatstate within TTL");

    info!("B received fresh chatstate: {:?}", event);

    client_a.disconnect().await;
    client_b.disconnect().await;

    Ok(())
}

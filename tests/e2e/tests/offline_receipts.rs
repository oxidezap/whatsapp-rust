use e2e_tests::{TestClient, text_msg};
use log::info;
use wacore::types::events::Event;
use wacore::types::presence::ReceiptType;

#[tokio::test]
async fn test_deferred_delivery_receipt() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let mut client_a = TestClient::connect("e2e_offline_receipt_a").await?;
    let client_b = TestClient::connect("e2e_offline_receipt_b").await?;

    let jid_b = client_b.jid().await;

    client_b.take_primary_offline().await?;
    client_b.disconnect().await;
    info!("Client B fully disconnected");

    let text = "Hello offline B!";
    let requested_id = whatsapp_rust::MessageId::new("3EB0AABBCCDDEEFF00112233")?;
    let ack = client_a
        .client
        .wait_for_node(whatsapp_rust::NodeFilter::tag("ack").attr("id", requested_id.to_string()));
    let msg_id = client_a
        .client
        .send(
            whatsapp_rust::SendRequest::new(&jid_b, text_msg(text))
                .with_options(whatsapp_rust::SendOptions::default().with_message_id(requested_id)),
        )
        .await?
        .message_id;
    let ack = tokio::time::timeout(std::time::Duration::from_secs(10), ack).await??;
    assert!(
        ack.get_attr("class")
            .is_some_and(|value| value.as_str() == "message"),
        "a durably queued offline message must receive a successful server ACK"
    );
    assert!(
        ack.get_attr("error").is_none(),
        "a durably queued offline message must not receive an error ACK"
    );
    info!("Client A sent message to offline B: {msg_id}");

    // Sender receipt (type="sender") IS expected — only check that no Delivered arrives.
    client_a
        .assert_no_event(
            5,
            |e| {
                matches!(
                    e,
                    Event::Receipt(receipt)
                    if receipt.message_ids.iter().any(|id| *id == msg_id)
                        && receipt.r#type == ReceiptType::Delivered
                )
            },
            "Should NOT receive delivery receipt when recipient is offline",
        )
        .await?;

    info!("Confirmed: no delivery receipt for offline recipient (single checkmark)");

    client_a.disconnect().await;

    Ok(())
}

#[tokio::test]
async fn test_bidirectional_offline_receipt() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let mut client_a = TestClient::connect("e2e_off_bidir_a").await?;
    let mut client_b = TestClient::connect("e2e_off_bidir_b").await?;

    let jid_b = client_b.jid().await;

    info!("B={jid_b}");

    client_b.go_offline().await?;
    info!("B is offline");

    let text = "Bidirectional offline test";
    let msg_id = client_a
        .client
        .send_message(jid_b.clone(), text_msg(text))
        .await?
        .message_id;
    info!("A sent message to offline B: {msg_id}");

    client_a.go_offline().await?;
    info!("A is offline");

    // B comes back first, on purpose: the receipt A is owed can only be queued
    // once B has taken delivery, which the backoff schedule left to chance.
    client_b.come_back_online();
    client_b.wait_for_text(text, 30).await?;
    info!("B received offline message");

    client_a.come_back_online();
    client_a
        .wait_for_event(30, |e| {
            matches!(
                e,
                Event::Receipt(receipt)
                if receipt.message_ids.iter().any(|id| *id == msg_id)
                    && receipt.r#type == ReceiptType::Delivered
            )
        })
        .await
        .expect("A should receive deferred delivery receipt after reconnect");
    info!("A received deferred delivery receipt after reconnect");

    client_a.disconnect().await;
    client_b.disconnect().await;

    Ok(())
}

#[tokio::test]
async fn test_deferred_delivery_receipt_on_reconnect() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let mut client_a = TestClient::connect("e2e_off_def_rcpt_a").await?;
    let mut client_b = TestClient::connect("e2e_off_def_rcpt_b").await?;

    let jid_b = client_b.jid().await;

    info!("B={jid_b}");

    client_b.take_primary_offline().await?;
    client_b.go_offline().await?;
    info!("B is offline");

    let text = "Waiting for delivery receipt";
    let msg_id = client_a
        .client
        .send_message(jid_b.clone(), text_msg(text))
        .await?
        .message_id;
    info!("A sent message to offline B: {msg_id}");

    // B stays down for the whole window now, so this is a negative about a
    // recipient that is provably offline rather than one that probably still is.
    client_a
        .assert_no_event(
            3,
            |e| {
                matches!(
                    e,
                    Event::Receipt(receipt)
                    if receipt.message_ids.iter().any(|id| *id == msg_id)
                        && receipt.r#type == ReceiptType::Delivered
                )
            },
            "A should NOT get delivery receipt while B is offline",
        )
        .await?;
    info!("Confirmed: no early delivery receipt");

    // B reconnects and receives the message
    client_b.come_back_online();
    client_b.wait_for_text(text, 30).await?;
    info!("B received the offline message after reconnect");

    // A receives the deferred delivery receipt
    client_a
        .wait_for_event(30, |e| {
            matches!(
                e,
                Event::Receipt(receipt)
                if receipt.message_ids.iter().any(|id| *id == msg_id)
                    && receipt.r#type == ReceiptType::Delivered
            )
        })
        .await?;
    info!("A received deferred delivery receipt");

    client_a.disconnect().await;
    client_b.disconnect().await;

    Ok(())
}

#[tokio::test]
async fn test_offline_presence_coalescing() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let client_a = TestClient::connect("e2e_off_presence_a").await?;
    let mut client_b = TestClient::connect("e2e_off_presence_b").await?;

    let jid_a = client_a.jid().await;

    info!("A={jid_a}");

    client_b.client.presence().subscribe(&jid_a).await?;
    info!("B subscribed to A's presence");

    client_a.client.presence().set_available().await?;

    let _initial = client_b
        .wait_for_event(15, |e| matches!(e, Event::Presence(_)))
        .await?;
    info!("B received initial presence");

    client_b.go_offline().await?;
    info!("B is offline");

    // A changes presence multiple times while B is offline. Both stanzas go out
    // over A's single socket, so the server already sees them in send order —
    // no pacing needed for the coalescing check below.
    client_a.client.presence().set_unavailable().await?;
    info!("A set unavailable");

    client_a.client.presence().set_available().await?;
    info!("A set available again");

    // B reconnects — coalescing means only the latest state arrives, not all 3
    client_b.come_back_online();
    let presence_event = client_b
        .wait_for_event(30, |e| matches!(e, Event::Presence(_)))
        .await?;

    if let Event::Presence(presence) = &*presence_event {
        info!("B received coalesced presence: {:?}", presence);
    }

    // Drain extra presence events (re-subscribe response, initial delivery on connect).
    let mut extra_count = 0;
    while client_b
        .wait_for_event(2, |e| matches!(e, Event::Presence(_)))
        .await
        .is_ok()
    {
        extra_count += 1;
        if extra_count > 5 {
            panic!("Too many extra presence events ({extra_count}), likely a leak");
        }
    }
    info!("Drained {extra_count} extra presence event(s) after initial coalesced delivery");

    client_a.disconnect().await;
    client_b.disconnect().await;

    Ok(())
}

/// Pausing a companion alone does not stop the separate primary receiver.
#[tokio::test]
async fn test_online_primary_receipts_while_companion_is_offline() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();
    let mut sender = TestClient::connect("e2e_primary_online_sender").await?;
    let mut companion = TestClient::connect("e2e_primary_online_companion").await?;
    let target = companion.jid().await;

    // Only the companion goes offline; the primary remains independently online.
    companion.go_offline().await?;
    let text = "primary delivery preserves the companion copy";
    let id = sender
        .client
        .send_message(target, text_msg(text))
        .await?
        .message_id;
    sender
        .wait_for_event(10, |event| {
            matches!(event,
                Event::Receipt(receipt) if receipt.message_ids.iter().any(|message_id| *message_id == id)
                    && receipt.r#type == ReceiptType::Delivered
            )
        })
        .await?;
    assert!(!companion.client.is_socket_connected());

    companion.come_back_online();
    companion.wait_for_text(text, 30).await?;
    sender.disconnect().await;
    companion.disconnect().await;
    Ok(())
}

use e2e_tests::{TestClient, text_msg};
use log::info;

#[tokio::test]
async fn test_offline_message_delivery_on_reconnect() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let client_a = TestClient::connect("e2e_offline_recon_a").await?;
    let mut client_b = TestClient::connect("e2e_offline_recon_b").await?;

    let jid_b = client_b.jid().await;
    info!("Client B JID: {jid_b}");

    // Triggers auto-reconnect
    client_b.client.reconnect().await;
    info!("Client B connection dropped, will auto-reconnect");
    client_b.wait_for_disconnected(5).await?;

    let text = "Hello from offline queue!";
    let msg_id = client_a
        .client
        .send_message(jid_b.clone(), text_msg(text))
        .await?
        .message_id;
    info!("Client A sent message to reconnecting B: {msg_id}");

    // Message should arrive from the offline queue after reconnect
    client_b.wait_for_text(text, 30).await?;
    info!("Client B received offline message after reconnect");

    client_a.disconnect().await;
    client_b.disconnect().await;

    Ok(())
}

#[tokio::test]
async fn test_offline_message_ordering() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let client_a = TestClient::connect("e2e_offline_order_a").await?;
    let mut client_b = TestClient::connect("e2e_offline_order_b").await?;

    let jid_b = client_b.jid().await;

    client_b.client.reconnect().await;
    client_b.wait_for_disconnected(5).await?;

    let messages = vec!["first", "second", "third"];
    for text in &messages {
        client_a
            .client
            .send_message(jid_b.clone(), text_msg(text))
            .await?;
        info!("Sent: {text}");
    }

    // Verify messages arrive in send order. During offline drain a single
    // event can carry several messages, so iterate each batch.
    let mut received = Vec::new();
    while received.len() < messages.len() {
        let event = client_b
            .wait_for_event(30, |e| {
                e.messages().any(|m| m.message.conversation.is_some())
            })
            .await?;

        for m in event.messages() {
            if let Some(text) = &m.message.conversation {
                info!("Received: {text}");
                received.push(text.clone());
            }
        }
    }

    assert_eq!(
        received, messages,
        "Messages should arrive in the order they were sent"
    );

    client_a.disconnect().await;
    client_b.disconnect().await;

    Ok(())
}

#[tokio::test]
async fn test_message_delivery_when_online() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let client_a = TestClient::connect("e2e_offline_online_a").await?;
    let mut client_b = TestClient::connect("e2e_offline_online_b").await?;

    let jid_b = client_b.jid().await;

    let text = "Hello online B!";
    client_a
        .client
        .send_message(jid_b.clone(), text_msg(text))
        .await?;
    client_b.wait_for_text(text, 30).await?;

    client_a.disconnect().await;
    client_b.disconnect().await;

    Ok(())
}

/// Sender-side only — verifies the server accepts messages for an offline recipient.
#[tokio::test]
async fn test_server_accepts_messages_for_offline_recipient() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let client_a = TestClient::connect("e2e_offline_multi_a").await?;
    let client_b = TestClient::connect("e2e_offline_multi_b").await?;

    let jid_b = client_b.jid().await;

    client_b.disconnect().await;

    let mut msg_ids = Vec::new();
    for i in 1..=5 {
        let text = format!("Offline message {}", i);
        let msg_id = client_a
            .client
            .send_message(jid_b.clone(), text_msg(&text))
            .await?
            .message_id;
        info!("Sent message {} to offline B: {}", i, msg_id);
        msg_ids.push(msg_id);
    }

    assert_eq!(
        msg_ids.len(),
        5,
        "All 5 messages should be accepted by the server"
    );

    client_a.disconnect().await;

    Ok(())
}

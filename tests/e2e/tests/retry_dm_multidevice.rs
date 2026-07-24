//! DM retry recovery after session deletion.

use e2e_tests::{TestClient, send_and_expect_text, text_msg};
use log::info;
use wacore::types::events::Event;
use wacore_binary::JidExt as _;
use wacore_binary::node::Node;
use whatsapp_rust::{NodeFilter, SendOptions};

/// A non-empty `<participants>` on a DM retry would mean we regressed to
/// the fanout shape (server rejects with 479 SmaxInvalid).
fn participant_target_count(message_node: &Node) -> usize {
    message_node
        .get_optional_child("participants")
        .and_then(|participants| participants.children())
        .map(|children| children.iter().filter(|child| child.tag == "to").count())
        .unwrap_or_default()
}

fn retry_enc_count(message_node: &Node) -> Option<String> {
    let enc = message_node.get_optional_child("enc")?;
    enc.attrs().optional_string("count").map(|s| s.into_owned())
}

// The mock server's DM router delivers retry resends through the
// `<participants>` fanout shape. After the WAWebSendMsgCreateDeviceStanza
// alignment (direct-`<enc>` retry shape) the simple-DM route at
// `bartender::handlers::message::mod::route_to_client` no longer
// reaches the second test client. Re-enable once the mock server's
// route fans out the bare-`<enc>` retry shape end to end. The shape
// itself is pinned by `dm_retry_emits_enc_directly_under_message_with_recipient`
// in `wacore::send::tests`.
#[ignore]
#[tokio::test]
async fn test_dm_retry_recovers_after_session_deletion() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let mut client_a = TestClient::connect("e2e_retry_dm_a").await?;
    let mut client_b = TestClient::connect("e2e_retry_dm_b").await?;

    let jid_a = client_a.jid().await;
    let jid_b = client_b.jid().await;
    assert_ne!(
        jid_a.user, jid_b.user,
        "Clients must use different accounts"
    );

    send_and_expect_text(
        &client_a.client,
        &mut client_b,
        &jid_b,
        "retry-setup-a2b",
        30,
    )
    .await?;
    send_and_expect_text(
        &client_b.client,
        &mut client_a,
        &jid_a,
        "retry-setup-b2a",
        30,
    )
    .await?;
    info!("Baseline roundtrip established sessions");

    client_b
        .client
        .signal()
        .delete_sessions(std::slice::from_ref(&jid_a))
        .await?;

    let message_id = format!("E2ERETRY{}", uuid::Uuid::new_v4().simple());
    let initial_waiter = client_a
        .client
        .wait_for_sent_node(NodeFilter::tag("message").attr("id", &message_id));
    client_a
        .client
        .send_message_with_options(
            jid_b.clone(),
            text_msg("retry-recover"),
            SendOptions::default().with_message_id(message_id.clone()),
        )
        .await?;

    let initial_node = tokio::time::timeout(tokio::time::Duration::from_secs(10), initial_waiter)
        .await
        .map_err(|_| anyhow::anyhow!("Timed out waiting for initial DM send node"))?
        .map_err(|_| anyhow::anyhow!("initial DM send waiter was canceled"))?;
    assert!(
        retry_enc_count(&initial_node).is_none(),
        "Initial DM send should not carry a retry count"
    );

    let retry_waiter = client_a
        .client
        .wait_for_sent_node(NodeFilter::tag("message").attr("id", &message_id));

    client_b
        .wait_for_event(30, |e| matches!(e, Event::UndecryptableMessage(_)))
        .await?;
    client_b.wait_for_text("retry-recover", 30).await?;

    let retry_node = tokio::time::timeout(tokio::time::Duration::from_secs(10), retry_waiter)
        .await
        .map_err(|_| anyhow::anyhow!("Timed out waiting for retry DM send node"))?
        .map_err(|_| anyhow::anyhow!("retry DM send waiter was canceled"))?;
    assert_eq!(
        participant_target_count(&retry_node),
        0,
        "DM retry resend must not use the <participants><to> fanout shape"
    );
    assert!(
        retry_node.get_optional_child("enc").is_some(),
        "Retry resend should carry an <enc> directly under <message>"
    );
    assert_eq!(
        retry_enc_count(&retry_node).as_deref(),
        Some("1"),
        "Retry resend should mark the payload with count=1"
    );
    // WA Web uses the raw receipt `from` (with device suffix) as the stanza `to`.
    // The mock server may or may not include a device suffix depending on timing,
    // so check user part only.
    let retry_to = retry_node
        .attrs()
        .optional_jid("to")
        .expect("Retry resend should have a 'to' attribute");
    assert!(
        retry_to.is_same_user_as(&jid_b),
        "Retry resend 'to' should target the same user (got {retry_to}, expected user {})",
        jid_b
    );
    info!("Retry recovered after B deleted its session with A");

    send_and_expect_text(
        &client_a.client,
        &mut client_b,
        &jid_b,
        "retry-followup-a2b",
        15,
    )
    .await?;
    send_and_expect_text(
        &client_b.client,
        &mut client_a,
        &jid_a,
        "retry-followup-b2a",
        15,
    )
    .await?;
    info!("Messaging still works after retry recovery");

    client_a.disconnect().await;
    client_b.disconnect().await;
    Ok(())
}

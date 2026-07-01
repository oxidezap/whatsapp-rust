//! status@broadcast send: must go out as `<message>` (not `<status>`) with no
//! addressing_mode, matching WA Web. The gate is status-specific — groups keep it.

use e2e_tests::{TestClient, text_msg};
use wacore_binary::node::Node;
use whatsapp_rust::Jid;
use whatsapp_rust::features::{GroupCreateOptions, GroupParticipantOptions};

fn has_child(node: &Node, tag: &str) -> bool {
    node.children()
        .map(|children| children.iter().any(|child| child.tag == tag))
        .unwrap_or(false)
}

fn child_attr(node: &Node, child_tag: &str, attr: &str) -> Option<String> {
    node.children()?
        .iter()
        .find(|child| child.tag == child_tag)?
        .attrs
        .get(attr)
        .map(|value| value.to_string())
}

/// Happy path: a text status is a WA-Web-compliant `<message to="status@broadcast">`
/// with no addressing_mode, `skmsg` ciphertext, and a `<meta status_setting>`.
#[tokio::test]
async fn status_broadcast_send_is_wa_web_compliant() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let client_a = TestClient::connect("e2e_status_ok_a").await?;
    let client_b = TestClient::connect("e2e_status_ok_b").await?;
    let recipient = client_b
        .client
        .get_lid()
        .expect("recipient should have a LID after connect");

    let sent_waiter = client_a.next_sent_message_waiter();
    client_a
        .client
        .status()
        .send_text(
            "hello status",
            0xFF1E_6E4F,
            0,
            &[recipient],
            Default::default(),
        )
        .await?;

    let sent = tokio::time::timeout(std::time::Duration::from_secs(10), sent_waiter)
        .await
        .map_err(|_| anyhow::anyhow!("timed out waiting for the sent status node"))?
        .map_err(|_| anyhow::anyhow!("sent-message waiter was canceled"))?;

    assert_eq!(
        sent.tag, "message",
        "status must use <message>, not <status> (NACK 400)"
    );
    assert_eq!(
        sent.attrs.get("to").map(|v| v.to_string()).as_deref(),
        Some("status@broadcast"),
    );
    assert!(
        sent.attrs.get("addressing_mode").is_none(),
        "status must omit addressing_mode (NACK 479)"
    );
    assert_eq!(child_attr(&sent, "enc", "type").as_deref(), Some("skmsg"));
    assert!(child_attr(&sent, "meta", "status_setting").is_some());

    client_a.disconnect().await;
    client_b.disconnect().await;
    Ok(())
}

/// Empty recipients are rejected before anything hits the wire.
#[tokio::test]
async fn status_send_rejects_empty_recipients() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let client = TestClient::connect("e2e_status_empty").await?;

    let err = client
        .client
        .status()
        .send_text("no audience", 0xFF1E_6E4F, 0, &[], Default::default())
        .await
        .expect_err("status with no recipients must error");
    assert!(
        err.to_string().contains("no recipients"),
        "unexpected error: {err}"
    );

    client.disconnect().await;
    Ok(())
}

/// A non-user recipient (group JID) is rejected: status audiences are users.
#[tokio::test]
async fn status_send_rejects_non_user_recipient() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let client = TestClient::connect("e2e_status_bad_jid").await?;
    let group_jid: Jid = "120363111111111111@g.us"
        .parse()
        .expect("fictitious JID parses");

    let err = client
        .client
        .status()
        .send_text(
            "wrong audience",
            0xFF1E_6E4F,
            0,
            &[group_jid],
            Default::default(),
        )
        .await
        .expect_err("status to a group JID must error");
    assert!(
        err.to_string().contains("user JID"),
        "unexpected error: {err}"
    );

    client.disconnect().await;
    Ok(())
}

/// Regression: groups still carry addressing_mode (the gate is status-specific).
#[tokio::test]
async fn group_send_still_carries_addressing_mode() -> anyhow::Result<()> {
    let _ = env_logger::builder().is_test(true).try_init();

    let client_a = TestClient::connect("e2e_status_grp_a").await?;
    let client_b = TestClient::connect("e2e_status_grp_b").await?;
    let jid_b = client_b.jid().await;

    let group = client_a
        .client
        .groups()
        .create_group(GroupCreateOptions {
            subject: "addressing_mode regression".to_string(),
            participants: vec![GroupParticipantOptions::new(jid_b)],
            ..Default::default()
        })
        .await?;
    let group_jid = group.metadata.id;

    let sent_waiter = client_a.next_sent_message_waiter();
    client_a
        .client
        .send_message(group_jid.clone(), text_msg("group keeps addressing_mode"))
        .await?;

    let sent = tokio::time::timeout(std::time::Duration::from_secs(10), sent_waiter)
        .await
        .map_err(|_| anyhow::anyhow!("timed out waiting for the sent group node"))?
        .map_err(|_| anyhow::anyhow!("sent-message waiter was canceled"))?;

    assert_eq!(sent.tag, "message");
    assert_eq!(
        sent.attrs.get("to").map(|v| v.to_string()).as_deref(),
        Some(group_jid.to_string().as_str()),
    );
    assert!(
        sent.attrs.get("addressing_mode").is_some(),
        "groups must keep addressing_mode (only status@broadcast drops it)"
    );
    assert!(has_child(&sent, "enc"));

    client_a.disconnect().await;
    client_b.disconnect().await;
    Ok(())
}

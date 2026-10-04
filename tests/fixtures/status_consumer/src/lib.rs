//! Independent consumer: construction, async sends, and deliberate negative controls.
use std::{future::Future, pin::Pin};
use wa::waproto::whatsapp::{Message, message::extended_text_message::FontType};
use wa::{
    Client, Jid, MessageId, SendError, SendResult, StanzaId, StatusPrivacySetting,
    StatusSendOptions,
};

pub fn content_options(id: MessageId) -> StatusSendOptions {
    StatusSendOptions::default()
        .with_message_id(id)
        .with_privacy(StatusPrivacySetting::AllowList)
        .with_extra_stanza_nodes(Vec::new())
        .with_device_freshness(wa::cache::Freshness::Refresh)
}

#[cfg(not(target_arch = "wasm32"))]
pub type StatusFuture<'a> =
    Pin<Box<dyn Future<Output = Result<SendResult, SendError>> + Send + 'a>>;
#[cfg(target_arch = "wasm32")]
pub type StatusFuture<'a> = Pin<Box<dyn Future<Output = Result<SendResult, SendError>> + 'a>>;

/// Same explicit recipient list for posting and revoking. No network runs in tests.
pub fn post_and_revoke<'a>(client: &'a Client, recipients: &'a [Jid]) -> StatusFuture<'a> {
    Box::pin(async move {
        let status = client.status();
        let _observed_audience = status.audience();
        let sent = status
            .send_text(
                "hello",
                0xFF000000,
                FontType::SYSTEM,
                recipients,
                content_options(MessageId::new("STATUS-CONTENT")?),
            )
            .await?;
        status
            .revoke(
                sent.message_id,
                recipients,
                StatusSendOptions::default().with_stanza_id(StanzaId::new("REVOKE-OPERATION")?),
            )
            .await
    })
}

pub async fn raw_status(client: &Client, recipients: &[Jid]) -> Result<SendResult, SendError> {
    client
        .status()
        .send_raw(Message::default(), recipients, StatusSendOptions::default())
        .await
}

pub fn read_options(options: &StatusSendOptions) -> (&Option<MessageId>, &Option<StanzaId>) {
    let StatusSendOptions {
        message_id,
        stanza_id,
        ..
    } = options;
    (message_id, stanza_id)
}

// Each feature must fail for the specific obsolete or mismatched contract.
#[cfg(feature = "old-message-id-string")]
pub fn old_message_id() {
    let mut options = StatusSendOptions::default();
    options.message_id = Some(String::from("CONTENT"));
}

#[cfg(feature = "old-revoke-string")]
pub async fn old_revoke(client: &Client) {
    let _ = client
        .status()
        .revoke("CONTENT", &[], StatusSendOptions::default())
        .await;
}

#[cfg(feature = "wrong-message-id-domain")]
pub fn wrong_content_domain() {
    let _ =
        StatusSendOptions::default().with_message_id(StanzaId::new("OPERATION").expect("fixture"));
}

#[cfg(feature = "wrong-stanza-id-domain")]
pub fn wrong_operation_domain() {
    let _ =
        StatusSendOptions::default().with_stanza_id(MessageId::new("CONTENT").expect("fixture"));
}

#[cfg(feature = "old-options-literal")]
pub fn old_literal() {
    let _ = StatusSendOptions {
        privacy: StatusPrivacySetting::Contacts,
        message_id: None,
        stanza_id: None,
        extra_stanza_nodes: Vec::new(),
        device_freshness: wa::cache::Freshness::default(),
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn construction_preserves_domains_and_public_read_access() {
        let options = content_options(MessageId::new("CONTENT-ü").unwrap());
        assert_eq!(options.message_id.as_ref().unwrap().as_str(), "CONTENT-ü");
        assert!(options.stanza_id.is_none());
        assert_eq!(options.privacy, StatusPrivacySetting::AllowList);
        assert_eq!(options.device_freshness, wa::cache::Freshness::Refresh);
        assert!(options.extra_stanza_nodes.is_empty());
        let operation =
            StatusSendOptions::default().with_stanza_id(StanzaId::new("OPERATION").unwrap());
        let (content_id, operation_id) = read_options(&operation);
        assert!(content_id.is_none());
        assert_eq!(operation_id.as_ref().unwrap().as_str(), "OPERATION");
        assert!(MessageId::new("").is_err());
        assert!(StanzaId::new("").is_err());
        let _ = post_and_revoke;
        let _ = raw_status;
    }
}

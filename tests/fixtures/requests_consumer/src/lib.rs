//! A genuinely separate downstream crate; operations compile but do not send.
use std::{future::Future, pin::Pin};
use wa::prelude::MessageBuilderExt;
use wa::{
    Client, EditOptions, EditRequest, Jid, MessageId, MessageRef, MessageSecret, SendError,
    SendOptions, SendRequest, SendResult, StanzaId, async_trait, waproto::whatsapp::Message,
};

#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait Host: Send + Sync {
    async fn send_and_edit(
        &self,
        chat: &Jid,
        target: MessageRef<'_>,
        creator: &Jid,
        secret: &MessageSecret,
    ) -> Result<SendResult, SendError>;
}

#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
pub trait Host {
    async fn send_and_edit(
        &self,
        chat: &Jid,
        target: MessageRef<'_>,
        creator: &Jid,
        secret: &MessageSecret,
    ) -> Result<SendResult, SendError>;
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl Host for Client {
    async fn send_and_edit(
        &self,
        chat: &Jid,
        target: MessageRef<'_>,
        creator: &Jid,
        secret: &MessageSecret,
    ) -> Result<SendResult, SendError> {
        self.send(
            SendRequest::new(chat, Message::text("hello")).with_options(
                SendOptions::default()
                    .with_message_id(MessageId::new("CONTENT")?)
                    .with_ephemeral_expiration(86400),
            ),
        )
        .await?;
        self.edit_message(
            EditRequest::new(target, Message::text("edited"))
                .with_options(EditOptions::default().with_stanza_id(StanzaId::new("OPERATION")?))
                .with_secret(creator, secret),
        )
        .await
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub type BoxedSend<'a> = Pin<Box<dyn Future<Output = Result<SendResult, SendError>> + Send + 'a>>;
#[cfg(target_arch = "wasm32")]
pub type BoxedSend<'a> = Pin<Box<dyn Future<Output = Result<SendResult, SendError>> + 'a>>;

pub fn boxed<'a>(client: &'a Client, chat: &'a Jid) -> BoxedSend<'a> {
    #[cfg(feature = "requests")]
    let send = client.send(SendRequest::new(chat, Message::text("hello")));
    #[cfg(not(feature = "requests"))]
    let send = client.send_message(chat, Message::text("hello"));
    Box::pin(async move {
        let sent = send.await?;
        #[cfg(feature = "requests")]
        let edit = client.edit_message(EditRequest::new(
            sent.message_ref()?,
            Message::text("edited"),
        ));
        #[cfg(not(feature = "requests"))]
        let edit = client.edit_message_raw(
            chat,
            sent.message_id.as_str(),
            Message::text("edited"),
            EditOptions::default(),
        );
        edit.await?;

        // Retain secret-edit capability in BOTH measured roots via a real named
        // creation. Canonical edits use its captured creator; raw edits keep
        // their documented current-identity assumption.
        let created = client
            .events()
            .create(
                chat,
                wa::EventCreationParams {
                    name: "Launch".into(),
                    ..Default::default()
                },
            )
            .await?;
        let mut update = Message::default();
        update.event_message.get_or_insert_default().name = Some("Updated launch".into());
        #[cfg(feature = "requests")]
        let encrypted = client.edit_message(
            EditRequest::new(created.send_result().message_ref()?, update)
                .with_secret(created.creator(), created.secret()),
        );
        #[cfg(not(feature = "requests"))]
        let encrypted = client.edit_message_encrypted_raw(
            &created.send_result().to,
            created.send_result().message_id.as_str(),
            created.secret().as_bytes(),
            update,
        );
        encrypted.await
    })
}

/// Compilation checks the result contracts without manufacturing a sealed result.
pub fn result_ids(result: &SendResult) -> (&MessageId, StanzaId) {
    (&result.message_id, result.stanza_id())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn external_request_and_boxed_async_trait_compile() {
        fn assert_host<T: Host>() {}
        assert_host::<Client>();
        let _ = boxed;
        let _ = result_ids;
    }

    #[test]
    fn validated_domains_preserve_spelling_and_reject_absent_names() {
        assert!(MessageId::new("").is_err());
        assert!(StanzaId::new("").is_err());
        let id = MessageId::new("ünïcødé_✅").unwrap();
        let options = SendOptions::default().with_message_id(id.clone());
        assert_eq!(options.message_id.as_ref(), Some(&id));
        assert_eq!(StanzaId::from_message_id(&id).as_str(), id.as_str());
        let chat = Jid::pn("15550000001");
        let request = SendRequest::new(&chat, Message::text("hello"));
        assert!(std::mem::size_of_val(&request) <= 192);
        let target = MessageRef::new(&chat, id, None, true).unwrap();
        let edit = EditRequest::new(target, Message::text("edit"));
        assert!(std::mem::size_of_val(&edit) <= 192);
    }
}

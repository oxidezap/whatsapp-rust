//! Public imports, borrowed payloads and Send boxed futures for typed operations.
//! Result-taking functions are compile-only here; real returned-result ownership
//! is exercised by the library's synthetic transport send tests.
use std::{future::Future, pin::Pin, sync::Arc};
use whatsapp_rust::prelude::MessageBuilderExt;
use whatsapp_rust::wacore::types::events::InboundMessage;
use whatsapp_rust::wacore::types::message::{MessageInfo, MessageSource};
use whatsapp_rust::{
    Client, EditRequest, MessageId, MessageRef, MessageRefError, NewsletterMessageRef, PinDuration,
    SendResult, ServerMessageId, StanzaId, anyhow, async_trait, waproto::whatsapp as wa,
};

#[async_trait]
trait MessageOperations: Send + Sync {
    async fn update(
        &self,
        target: &MessageRef<'_>,
        post: &NewsletterMessageRef<'_>,
    ) -> anyhow::Result<()>;
}

#[async_trait]
impl MessageOperations for Client {
    async fn update(
        &self,
        target: &MessageRef<'_>,
        post: &NewsletterMessageRef<'_>,
    ) -> anyhow::Result<()> {
        self.edit_message(EditRequest::new(
            target.clone(),
            wa::Message::text("replacement"),
        ))
        .await?;
        self.revoke_message_ref(target).await?;
        self.send_reaction_ref(target, "👍").await?;
        self.pin_message_ref(target, PinDuration::Days7).await?;
        self.unpin_message_ref(target).await?;
        self.keep_message_ref(target, true).await?;
        self.mark_message_read(target).await?;
        self.mark_message_played(target).await?;
        let _: StanzaId = self.newsletter().send_reaction_ref(post, "👍").await?;
        let _: StanzaId = self
            .newsletter()
            .send_poll_vote_ref(post, &[[7; 32]])
            .await?;
        self.newsletter()
            .edit_message(post, wa::Message::text("replacement"))
            .await?;
        self.newsletter().revoke_message(post).await?;
        Ok(())
    }
}

fn boxed_operations<'a>(
    client: &'a Client,
    target: &'a MessageRef<'a>,
    post: &'a NewsletterMessageRef<'a>,
) -> Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'a>> {
    Box::pin(client.update(target, post))
}

fn boxed_inbound_reaction<'a>(
    client: &'a Client,
    inbound: &'a InboundMessage,
) -> Pin<Box<dyn Future<Output = anyhow::Result<SendResult>> + Send + 'a>> {
    Box::pin(async move {
        Ok(client
            .send_reaction_ref(&inbound.message_ref()?, "👍")
            .await?)
    })
}

// Callers acquire these sealed results through Client's send APIs.
fn returned_result_references<'a>(
    dm: &'a SendResult,
    channel: &'a SendResult,
) -> Result<(MessageRef<'a>, NewsletterMessageRef<'a>, StanzaId), MessageRefError> {
    Ok((dm.message_ref()?, channel.newsletter_ref()?, dm.stanza_id()))
}

#[test]
fn message_reference_public_imports_and_boxed_futures_compile() {
    let _ = boxed_operations;
    let _ = boxed_inbound_reaction;
    let _ = returned_result_references;
    fn assert_impl<T: MessageOperations>() {}
    assert_impl::<Client>();
}

#[test]
fn borrowed_public_inbound_reference_preserves_arc_and_origin() {
    let body = Arc::new(wa::Message::text("comment"));
    let info = Arc::new(MessageInfo {
        id: "INBOUND_CONTENT".into(),
        source: MessageSource {
            chat: "120363000000000001@g.us".parse().unwrap(),
            sender: "15550000001@s.whatsapp.net".parse().unwrap(),
            is_group: true,
            ..Default::default()
        },
        ..Default::default()
    });
    let inbound = InboundMessage::builder()
        .message(body.clone())
        .info(info.clone())
        .ephemeral_expiration(86400)
        .comment_target(Box::new(wa::MessageKey {
            id: Some("PARENT_POST".into()),
            ..Default::default()
        }))
        .build();
    let reference = inbound.message_ref().unwrap();
    assert_eq!(reference.id().as_str(), "INBOUND_CONTENT");
    assert!(!reference.from_me());
    assert_eq!(reference.sender(), Some(&info.source.sender));
    assert_eq!(
        reference.to_raw_key().participant,
        Some(info.source.sender.to_string())
    );
    assert!(std::ptr::eq(reference.chat(), &info.source.chat));
    assert!(std::ptr::eq(reference.source().unwrap(), &info.source));
    assert!(std::ptr::eq(
        reference.comment_target().unwrap(),
        inbound.comment_target.as_deref().unwrap()
    ));
    assert_eq!(reference.ephemeral_expiration(), Some(86400));
    assert!(Arc::ptr_eq(&body, &inbound.message));
    assert!(Arc::ptr_eq(&info, &inbound.info));
    assert_eq!(Arc::strong_count(&body), 2);
    assert_eq!(Arc::strong_count(&info), 2);
    let channel = "120363000000000001@newsletter".parse().unwrap();
    let post = NewsletterMessageRef::new(
        &channel,
        Some(MessageId::new("POST").unwrap()),
        Some(ServerMessageId::new(0)),
    )
    .unwrap();
    assert_eq!(post.server_id().unwrap().get(), 0);
}

#[test]
fn reference_errors_keep_their_typed_cause() {
    use std::error::Error;
    let send: whatsapp_rust::SendError = MessageRefError::MissingSender.into();
    assert_eq!(
        send.source().unwrap().downcast_ref::<MessageRefError>(),
        Some(&MessageRefError::MissingSender)
    );
    let channel: whatsapp_rust::NewsletterError = MessageRefError::MissingMessageId.into();
    assert_eq!(
        channel.source().unwrap().downcast_ref::<MessageRefError>(),
        Some(&MessageRefError::MissingMessageId)
    );
}

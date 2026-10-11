//! Independent consumer: no transport, session, or runtime is constructed.
use std::{future::Future, pin::Pin, sync::Arc};
use whatsapp_rust::bot::MessageContext;
use whatsapp_rust::prelude::MessageBuilderExt;
use whatsapp_rust::wacore::types::message::{MessageInfo, MessageSource};
use whatsapp_rust::{
    Client, EventCreationParams, MessageRef, NewsletterMessage, NewsletterMessageRef,
    NewsletterMessageType, NewsletterMetadata, NewsletterPollVote, NewsletterReactionCount,
    NewsletterState, NewsletterVerification, PinDuration, RevokeType, StanzaId, anyhow,
    async_trait, waproto::whatsapp as proto,
};

#[cfg(not(target_arch = "wasm32"))]
#[async_trait]
pub trait Host: Send + Sync {
    async fn actions(
        &self,
        target: &MessageRef<'_>,
        post: &NewsletterMessageRef<'_>,
    ) -> anyhow::Result<()>;
}

// Browser clients and their futures are local: ?Send alone does not relax
// the trait's supertraits, so retain Send + Sync only for the native host.
#[cfg(target_arch = "wasm32")]
#[async_trait(?Send)]
pub trait Host {
    async fn actions(
        &self,
        target: &MessageRef<'_>,
        post: &NewsletterMessageRef<'_>,
    ) -> anyhow::Result<()>;
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl Host for Client {
    async fn actions(
        &self,
        target: &MessageRef<'_>,
        post: &NewsletterMessageRef<'_>,
    ) -> anyhow::Result<()> {
        self.chat_actions().star_message(target).await?;
        self.chat_actions().unstar_message(target).await?;
        self.chat_actions()
            .delete_message_for_me(target, false, None)
            .await?;
        self.labels()
            .add_message_label("synthetic", target.chat(), target.id())
            .await?;
        self.labels()
            .remove_message_label("synthetic", target.chat(), target.id())
            .await?;
        self.send_reaction(target, "👍").await?;
        self.keep_message(target, true).await?;
        self.pin_message(target, PinDuration::Days7).await?;
        self.unpin_message(target).await?;
        self.revoke_message(target).await?;
        self.comments().send_text(target, "comment").await?;
        self.comments()
            .send_message(target, proto::Message::text("body"))
            .await?;
        let _: StanzaId = self.newsletter().send_reaction(post, "👍").await?;
        let _: StanzaId = self.newsletter().send_poll_vote(post, &[[7; 32]]).await?;
        self.newsletter()
            .edit_message(post, proto::Message::text("edit"))
            .await?;
        self.newsletter().revoke_message(post).await?;
        Ok(())
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub type BoxedAction<'a> = Pin<Box<dyn Future<Output = anyhow::Result<()>> + Send + 'a>>;
#[cfg(target_arch = "wasm32")]
pub type BoxedAction<'a> = Pin<Box<dyn Future<Output = anyhow::Result<()>> + 'a>>;

pub fn boxed<'a>(
    client: &'a Client,
    target: &'a MessageRef<'a>,
    post: &'a NewsletterMessageRef<'a>,
) -> BoxedAction<'a> {
    Box::pin(client.actions(target, post))
}

/// Raw interop remains explicit and shares the canonical implementation.
pub async fn raw(
    client: &Client,
    target: &MessageRef<'_>,
    post: &NewsletterMessageRef<'_>,
) -> anyhow::Result<()> {
    client
        .send_reaction_raw(target.chat(), target.to_raw_key(), "👍")
        .await?;
    client
        .keep_message_raw(target.chat(), target.to_raw_key(), true)
        .await?;
    client
        .pin_message_raw(target.chat(), target.to_raw_key(), PinDuration::Days7)
        .await?;
    client
        .unpin_message_raw(target.chat(), target.to_raw_key())
        .await?;
    client
        .revoke_message_raw(target.chat(), target.id().as_str(), RevokeType::Sender)
        .await?;
    client
        .comments()
        .send_text_raw(target.chat(), target.to_raw_key(), "comment")
        .await?;
    client
        .comments()
        .send_message_raw(
            target.chat(),
            target.to_raw_key(),
            proto::Message::text("body"),
        )
        .await?;
    let _: StanzaId = client
        .newsletter()
        .send_reaction_raw(post.chat(), post.require_server_id()?.get(), "👍")
        .await?;
    let _: StanzaId = client
        .newsletter()
        .send_poll_vote_raw(post.chat(), post.require_server_id()?.get(), &[[7; 32]])
        .await?;
    client
        .newsletter()
        .edit_message_raw(
            post.chat(),
            post.require_message_id()?.as_str(),
            proto::Message::text("edit"),
        )
        .await?;
    client
        .newsletter()
        .revoke_message_raw(post.chat(), post.require_message_id()?.as_str())
        .await?;
    Ok(())
}

pub fn mock_info() -> MessageInfo {
    MessageInfo {
        id: "COMMENT".into(),
        source: MessageSource {
            chat: "120363000000000001@g.us".parse().expect("fixture group"),
            sender: whatsapp_rust::Jid::lid("100000000000001"),
            is_group: true,
            ..Default::default()
        },
        ..Default::default()
    }
}

pub fn mock_context(client: Arc<Client>) -> MessageContext {
    MessageContext::builder()
        .client(client)
        .message(Arc::new(proto::Message::text("comment")))
        .info(mock_info())
        .ephemeral_expiration(86400)
        .comment_target(Box::new(proto::MessageKey {
            id: Some("PARENT".into()),
            ..Default::default()
        }))
        .build()
}

pub async fn context_actions(ctx: &MessageContext) -> anyhow::Result<()> {
    let target = ctx.message_ref()?;
    ctx.react("👍").await?;
    ctx.revoke_message(&target).await?;
    ctx.revoke_message_raw(target.id().as_str(), RevokeType::Sender)
        .await?;
    let MessageContext {
        message,
        ephemeral_expiration,
        ..
    } = ctx;
    let _ = (message, ephemeral_expiration);
    Ok(())
}

pub fn event_params() -> EventCreationParams {
    EventCreationParams::builder()
        .name("Launch".into())
        .description("Description".into())
        .start_time(1_700_000_000)
        .end_time(1_700_003_600)
        .join_link("https://example.invalid/call".into())
        .location(proto::message::LocationMessage::default())
        .is_scheduled_call(true)
        .extra_guests_allowed(false)
        .build()
}

pub fn mock_newsletter() -> NewsletterMetadata {
    NewsletterMetadata::builder()
        .jid(
            "120363000000000001@newsletter"
                .parse()
                .expect("fixture jid"),
        )
        .name("Channel".into())
        .subscriber_count(0)
        .verification(NewsletterVerification::Unverified)
        .state(NewsletterState::Active)
        .build()
}

pub fn mock_history() -> NewsletterMessage {
    NewsletterMessage::builder()
        .server_id(0)
        .timestamp(1_700_000_000)
        .message_type(NewsletterMessageType::Text)
        .edit(whatsapp_rust::wacore::types::message::EditAttribute::Empty)
        .is_sender(false)
        .is_wamo_sub(false)
        .reactions(vec![
            NewsletterReactionCount::builder()
                .code("👍".into())
                .count(2)
                .build(),
        ])
        .votes(vec![
            NewsletterPollVote::builder()
                .option_hash([7; 32])
                .count(1)
                .build(),
        ])
        .build()
}

pub mod construction;
pub mod removed;

/// Construct a detached range from addressing metadata alone, without a client.
pub fn range_from_metadata(
    info: &MessageInfo,
) -> anyhow::Result<whatsapp_rust::SyncActionMessageRange> {
    Ok(whatsapp_rust::message_range(
        42,
        Some(7),
        std::iter::once((MessageRef::from_info(info)?, 41)),
    ))
}

/// The context/result raw-key accessors remain available for explicit interop.
pub fn retained_key_accessors(ctx: &MessageContext, sent: &whatsapp_rust::SendResult) {
    let _: proto::MessageKey = ctx.message_key();
    let _: proto::MessageKey = sent.message_key();
}

#[cfg(all(test, feature = "contracts"))]
mod tests {
    use super::*;
    use whatsapp_rust::{
        NewsletterAdminInfo, NewsletterAdminProfile, NewsletterFollower, NewsletterMyAddOns,
        NewsletterMyPollVote, NewsletterMyReaction,
    };

    #[test]
    fn message_range_outlives_addressing_metadata() {
        let range = {
            let info = mock_info();
            range_from_metadata(&info).unwrap()
        };
        assert_eq!(range.last_message_timestamp, Some(42));
        assert_eq!(range.last_system_message_timestamp, Some(7));
        assert_eq!(range.messages.len(), 1);
        assert_eq!(range.messages[0].timestamp, Some(41));
        let key = range.messages[0].key.as_option().unwrap();
        assert_eq!(key.id.as_deref(), Some("COMMENT"));
        assert_eq!(key.remote_jid.as_deref(), Some("120363000000000001@g.us"));
        assert_eq!(key.participant.as_deref(), Some("100000000000001@lid"));
        assert_eq!(key.from_me, Some(false));
        assert!(
            whatsapp_rust::message_range(0, None, [])
                .messages
                .is_empty()
        );
    }

    #[test]
    fn host_futures_and_mock_construction_are_supported() {
        fn assert_host<T: Host>() {}
        assert_host::<Client>();
        let _ = (boxed, raw, mock_context, context_actions);
        let info = mock_info();
        let reference = MessageRef::from_info(&info).expect("mock is actionable");
        assert_eq!(reference.id().as_str(), "COMMENT");
        assert_eq!(reference.sender(), Some(&info.source.sender));
        let EventCreationParams {
            name, start_time, ..
        } = event_params();
        assert_eq!(name, "Launch");
        assert_eq!(start_time, Some(1_700_000_000));
        let NewsletterMetadata {
            name, description, ..
        } = mock_newsletter();
        assert_eq!(name, "Channel");
        assert!(description.is_none());
        let profile = NewsletterAdminProfile::builder()
            .name("Admin".into())
            .build();
        let NewsletterAdminProfile { name, .. } = &profile;
        assert_eq!(name, "Admin");
        let admin = NewsletterAdminInfo::builder()
            .admin_profile(profile)
            .build();
        let NewsletterAdminInfo { admin_count, .. } = admin;
        assert_eq!(admin_count, None);
        assert_eq!(NewsletterAdminInfo::default().admin_count, None);
        let follower = NewsletterFollower::builder()
            .jid(whatsapp_rust::Jid::lid("100000000000001"))
            .build();
        let NewsletterFollower { phone_jid, .. } = follower;
        assert_eq!(phone_jid, None);
        let NewsletterMessage {
            reactions, votes, ..
        } = mock_history();
        let NewsletterReactionCount { code, count, .. } = &reactions[0];
        assert_eq!((code.as_str(), *count), ("👍", 2));
        let NewsletterPollVote {
            option_hash, count, ..
        } = &votes[0];
        assert_eq!((*option_hash, *count), ([7; 32], 1));
        let mine = NewsletterMyAddOns::builder()
            .server_id(0)
            .reaction(
                NewsletterMyReaction::builder()
                    .code("👍".into())
                    .timestamp(1)
                    .build(),
            )
            .poll_vote(
                NewsletterMyPollVote::builder()
                    .timestamp(2)
                    .option_hashes(vec![])
                    .build(),
            )
            .build();
        let NewsletterMyAddOns {
            reaction,
            poll_vote,
            ..
        } = mine;
        assert_eq!(reaction.unwrap().timestamp, 1);
        let NewsletterMyPollVote { option_hashes, .. } = poll_vote.unwrap();
        assert!(option_hashes.is_empty());
    }
}

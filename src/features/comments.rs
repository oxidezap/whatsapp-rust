//! Encrypted channel comments (threaded replies under a Community
//! Announcement Group post).
//!
//! Mirrors WA Web `WAWebSendCommentMessageAction`: the comment body is a
//! regular `Message` (extended text), encrypted with the parent post's
//! `messageSecret` under the `"Enc Comment"` use-case, and shipped as a
//! top-level `enc_comment_message` envelope. The comment carries its own
//! fresh `messageSecret` so it can itself receive reactions.
//!
//! Incoming comments are decrypted transparently on the receive path and
//! dispatched as their inner body `Message`; the parent post key surfaces on
//! `InboundMessage::comment_target` (and `MessageContext::comment_target` for
//! a bot handler).

use wacore_binary::{Jid, JidExt};
use waproto::whatsapp as wa;

use crate::client::Client;
use crate::send::{SendError, SendResult};

pub struct Comments<'a> {
    client: &'a Client,
}

impl<'a> Comments<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Comment on a Community Announcement Group post with a text body.
    /// Requires the parent's captured `messageSecret`. The caller must supply
    /// a CAG post; a generic message reference does not attest group subtype.
    pub async fn send_text(
        &self,
        parent: &crate::MessageRef<'_>,
        text: &str,
    ) -> Result<SendResult, SendError> {
        self.send_text_raw(parent.chat(), self.parent_key(parent).await?, text)
            .await
    }

    /// Comment with an arbitrary body, preserving the parent's author scope.
    pub async fn send_message(
        &self,
        parent: &crate::MessageRef<'_>,
        body: wa::Message,
    ) -> Result<SendResult, SendError> {
        self.send_message_raw(parent.chat(), self.parent_key(parent).await?, body)
            .await
    }

    async fn parent_key(
        &self,
        parent: &crate::MessageRef<'_>,
    ) -> Result<wa::MessageKey, SendError> {
        parent.require_chat_operation()?;
        if !parent.chat().is_group() {
            return Err(crate::MessageRefError::UnsupportedOrigin.into());
        }
        // MessageRef carries no CAG subtype proof. Do not introduce metadata
        // queries/fallbacks into the captured-secret comment path to obtain one.
        self.client.message_ref_addon_key(parent).await
    }

    /// Explicit raw chat/key interop. `parent_key.participant` is the post
    /// author used for secret resolution and decryption, not the commenter.
    pub async fn send_text_raw(
        &self,
        chat: impl Into<Jid>,
        parent_key: wa::MessageKey,
        text: &str,
    ) -> Result<SendResult, SendError> {
        let chat = &chat.into();
        // WA Web encryptExtendedTextComment: the body is an extendedTextMessage.
        let body = wa::Message {
            extended_text_message: buffa::MessageField::some(wa::message::ExtendedTextMessage {
                text: Some(text.to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        self.send_message_raw(chat, parent_key, body).await
    }

    /// Explicit raw chat/key interop with an arbitrary comment body.
    pub async fn send_message_raw(
        &self,
        chat: impl Into<Jid>,
        mut parent_key: wa::MessageKey,
        body: wa::Message,
    ) -> Result<SendResult, SendError> {
        let chat = &chat.into();
        let client = self.client;
        let (author, secret) = client
            .resolve_outgoing_addon_parent(chat, &parent_key)
            .await?;
        let parent_id = parent_key
            .id
            .clone()
            .ok_or_else(|| SendError::InvalidRequest("parent message key missing id".into()))?;
        // WA Web comments are authored under the LID identity
        // (getMeLidUserOrThrow); fall back to PN only when no LID is known.
        let commenter = client
            .lid()
            .or_else(|| client.pn())
            .map(|j| j.to_non_ad())
            .ok_or(SendError::NotLoggedIn)?;

        let (enc_payload, iv) = wacore::comment::encrypt_comment_with_secret(
            &body,
            &secret,
            &parent_id,
            &author.to_non_ad_string(),
            &commenter.to_non_ad_string(),
        )?;

        // Receivers resolve the parent author from the envelope key, so it
        // must carry the same identity the HKDF was derived with.
        if parent_key.participant.is_none() {
            parent_key.participant = Some(author.to_non_ad_string());
        }

        // Fresh secret so the comment can itself receive encrypted add-ons.
        let comment_secret: [u8; 32] = {
            use rand::Rng;
            let mut secret = [0u8; 32];
            rand::rng().fill_bytes(&mut secret);
            secret
        };

        let message = wa::Message {
            enc_comment_message: buffa::MessageField::some(wa::message::EncCommentMessage {
                target_message_key: buffa::MessageField::some(parent_key),
                enc_payload: Some(enc_payload),
                enc_iv: Some(iv.to_vec()),
            }),
            message_context_info: buffa::MessageField::some(wa::MessageContextInfo {
                message_secret: Some(comment_secret.to_vec()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let result = client.send_message(chat, message).await?;

        // The send path only persists reporting-token secrets, so store the
        // comment's own secret here or we could never decrypt add-ons
        // targeting our own comment.
        client
            .persist_outbound_msg_secret(
                chat,
                &commenter,
                result.message_id.as_str(),
                &comment_secret,
                wacore::msg_secret::RetentionClass::Text,
                crate::send::SendInstant::now(),
            )
            .await;
        Ok(result)
    }
}

impl Client {
    pub fn comments(&self) -> Comments<'_> {
        Comments::new(self)
    }
}

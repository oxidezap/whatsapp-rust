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
//! `MessageInfo::comment_target`.

use anyhow::{Result, anyhow};
use wacore_binary::Jid;
use waproto::whatsapp as wa;

use crate::client::Client;
use crate::send::SendResult;

pub struct Comments<'a> {
    client: &'a Client,
}

impl<'a> Comments<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Comment on a channel post with a text body.
    ///
    /// `parent_key` references the post being commented on and must carry
    /// `participant` (the post author) so receivers can key the decryption.
    /// Requires the parent's `messageSecret` (captured when the post was
    /// received).
    pub async fn send_text(
        &self,
        chat: impl Into<Jid>,
        parent_key: wa::MessageKey,
        text: &str,
    ) -> Result<SendResult> {
        let chat = &chat.into();
        // WA Web encryptExtendedTextComment: the body is an extendedTextMessage.
        let body = wa::Message {
            extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
                text: Some(text.to_string()),
                ..Default::default()
            })),
            ..Default::default()
        };
        self.send_message(chat, parent_key, body).await
    }

    /// Comment on a channel post with an arbitrary body `Message`.
    pub async fn send_message(
        &self,
        chat: impl Into<Jid>,
        mut parent_key: wa::MessageKey,
        body: wa::Message,
    ) -> Result<SendResult> {
        let chat = &chat.into();
        let client = self.client;
        let (author, secret) = client
            .resolve_outgoing_addon_parent(chat, &parent_key)
            .await?;
        let parent_id = parent_key
            .id
            .clone()
            .ok_or_else(|| anyhow!("parent message key missing id"))?;
        // WA Web comments are authored under the LID identity
        // (getMeLidUserOrThrow); fall back to PN only when no LID is known.
        let commenter = client
            .get_lid()
            .or_else(|| client.get_pn())
            .map(|j| j.to_non_ad())
            .ok_or_else(|| anyhow!("not logged in"))?;

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
        let comment_secret: Vec<u8> = {
            use rand::Rng;
            let mut secret = vec![0u8; 32];
            rand::make_rng::<rand::rngs::StdRng>().fill_bytes(&mut secret);
            secret
        };

        let message = wa::Message {
            enc_comment_message: Some(Box::new(wa::message::EncCommentMessage {
                target_message_key: Some(parent_key),
                enc_payload: Some(enc_payload),
                enc_iv: Some(iv.to_vec()),
            })),
            message_context_info: Some(Box::new(wa::MessageContextInfo {
                message_secret: Some(comment_secret.clone()),
                ..Default::default()
            })),
            ..Default::default()
        };
        let result = client.send_message(chat, message).await?;

        // The send path only persists reporting-token secrets, so store the
        // comment's own secret here or we could never decrypt add-ons
        // targeting our own comment.
        let secret: [u8; 32] = comment_secret
            .as_slice()
            .try_into()
            .expect("comment secret is 32 bytes");
        client
            .persist_outbound_msg_secret(
                chat,
                &commenter,
                &result.message_id,
                &secret,
                wacore::msg_secret::RetentionClass::Text,
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

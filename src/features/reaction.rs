//! Sending reactions to DM/group/status messages.
//!
//! Newsletter reactions go through a different (plaintext) wire path; use
//! [`Client::newsletter`]'s `send_reaction` for channels.
//!
//! Community Announcement Groups never accept plaintext reactions: WA Web
//! (`WAWebReactionEncryptMsgData`) encrypts the reaction with the target
//! message's `messageSecret` and emits an `enc_reaction_message` envelope.
//! [`Client::send_reaction`] applies the same gate transparently.

use anyhow::anyhow;
use wacore_binary::{Jid, JidExt};
use waproto::whatsapp as wa;

use crate::client::Client;
use crate::send::SendResult;

impl Client {
    /// React to a DM, group, or status@broadcast message.
    ///
    /// `target_key` references the message being reacted to. For groups and
    /// status it must carry `participant` (the original sender) so the receipt
    /// can be attributed; [`crate::bot::MessageContext::react`] fills this in
    /// from the incoming message. An empty `emoji` removes a previous reaction
    /// (WA Web's empty-text reaction == sender-revoke).
    ///
    /// For a Community Announcement Group the reaction is encrypted with the
    /// target's `messageSecret` (captured when the message was received) and
    /// sent as `enc_reaction_message`; reacting to a message whose secret was
    /// never captured fails rather than emitting a plaintext reaction the
    /// channel would reject.
    ///
    /// status@broadcast reactions fan out to the status author's devices; the
    /// author is read from `target_key.participant` by the send path.
    pub async fn send_reaction(
        &self,
        chat: impl Into<Jid>,
        target_key: wa::MessageKey,
        emoji: &str,
    ) -> Result<SendResult, anyhow::Error> {
        let chat = &chat.into();
        if chat.is_group() && self.is_community_announce_group(chat).await? {
            return self.send_enc_reaction(chat, target_key, emoji).await;
        }
        let reaction = wacore::proto_helpers::build_reaction_message(
            target_key,
            emoji,
            wacore::time::now_millis(),
        );
        self.send_message(chat, reaction).await
    }

    /// Whether `chat` is a Community Announcement Group (WA Web `isCag`).
    ///
    /// Served from the cached/persisted group metadata; a blob persisted
    /// before the flag existed answers `None` and falls back to one full
    /// metadata query.
    pub(crate) async fn is_community_announce_group(
        &self,
        chat: &Jid,
    ) -> Result<bool, anyhow::Error> {
        if let Some(flag) = self.groups().query_info(chat).await?.is_community_announce {
            return Ok(flag);
        }
        Ok(self.groups().get_metadata(chat).await?.is_default_sub_group)
    }

    async fn send_enc_reaction(
        &self,
        chat: &Jid,
        mut target_key: wa::MessageKey,
        emoji: &str,
    ) -> Result<SendResult, anyhow::Error> {
        let (author, secret) = self
            .resolve_outgoing_addon_parent(chat, &target_key)
            .await?;
        let target_id = target_key
            .id
            .clone()
            .ok_or_else(|| anyhow!("target message key missing id"))?;
        // Receivers derive the addon key with the STANZA sender, which in a
        // CAG is our LID identity regardless of the parent author's namespace;
        // mirror the comment path (WA Web authors CAG addons under LID).
        let reactor = self
            .get_lid()
            .or_else(|| self.get_pn())
            .map(|j| j.to_non_ad())
            .ok_or_else(|| anyhow!("not logged in"))?;

        let (enc_payload, iv) = wacore::reaction::encrypt_reaction_with_secret(
            emoji,
            wacore::time::now_millis(),
            &secret,
            &target_id,
            &author.to_non_ad_string(),
            &reactor.to_non_ad_string(),
        )?;

        // Receivers resolve the parent author from the envelope key, so it
        // must carry the same identity the HKDF was derived with.
        if target_key.participant.is_none() {
            target_key.participant = Some(author.to_non_ad_string());
        }

        let message = wa::Message {
            enc_reaction_message: Some(wa::message::EncReactionMessage {
                target_message_key: Some(target_key),
                enc_payload: Some(enc_payload),
                enc_iv: Some(iv.to_vec()),
            }),
            ..Default::default()
        };
        self.send_message(chat, message).await
    }
}

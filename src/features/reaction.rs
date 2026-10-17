//! Sending reactions to DM/group/status messages.
//!
//! Newsletter reactions go through a different (plaintext) wire path; use
//! [`Client::newsletter`]'s `send_reaction` for channels.
//!
//! Community Announcement Groups never accept plaintext reactions: WA Web
//! (`WAWebReactionEncryptMsgData`) encrypts the reaction with the target
//! message's `messageSecret` and emits an `enc_reaction_message` envelope.
//! [`Client::send_reaction`] applies the same gate transparently.

use buffa::MessageField;
use wacore_binary::{Jid, JidExt};
use waproto::whatsapp as wa;

use crate::client::Client;
use crate::send::{SendError, SendRequest, SendResult};

impl Client {
    /// React using the message's chat/author scope. Newsletter reactions use
    /// [`crate::features::Newsletter::send_reaction`]; explicit chat/key interop
    /// is available through [`Client::send_reaction_raw`].
    pub async fn send_reaction(
        &self,
        target: &crate::MessageRef<'_>,
        emoji: &str,
    ) -> Result<SendResult, SendError> {
        // Status reactions have supported author fanout; broadcast lists do not.
        if target.chat().is_broadcast_list() {
            return Err(crate::MessageRefError::UnsupportedOrigin.into());
        }
        let mut key = self.message_ref_addon_key(target).await?;
        if target.chat().is_status_broadcast() && key.participant.is_none() {
            key.participant = Some(self.pn().ok_or(SendError::NotLoggedIn)?.to_non_ad_string());
        }
        self.send_reaction_raw(target.chat(), key, emoji).await
    }

    /// Explicit raw chat/key interop for DM, group, or status reactions.
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
    pub async fn send_reaction_raw(
        &self,
        chat: impl Into<Jid>,
        target_key: wa::MessageKey,
        emoji: &str,
    ) -> Result<SendResult, SendError> {
        let chat = &chat.into();
        let request = if chat.is_group() && self.is_community_announce_group(chat).await? {
            // Secret lookup and encrypted-envelope construction are CAG-only.
            // Keep their frame out of ordinary DM/group reaction futures.
            Box::pin(self.prepare_enc_reaction(chat, target_key, emoji)).await?
        } else {
            plain_reaction_request(chat, target_key, emoji)
        };
        self.send(request).await
    }

    /// Whether `chat` is a Community Announcement Group (WA Web `isCag`).
    ///
    /// Served from the cached/persisted group metadata; a blob persisted
    /// before the flag existed answers `None` and falls back to one full
    /// metadata query.
    pub(crate) async fn is_community_announce_group(
        &self,
        chat: &Jid,
    ) -> Result<bool, crate::features::GroupError> {
        if let Some(flag) = self
            .groups()
            .routing_info(chat)
            .await?
            .is_community_announce
        {
            return Ok(flag);
        }
        // The full metadata response/state is needed only when the routing
        // flag is absent. Keep that cold phase out of warm/DM reaction frames.
        Box::pin(async {
            Ok(self
                .groups()
                .fetch_metadata(chat)
                .await?
                .is_default_sub_group)
        })
        .await
    }

    async fn prepare_enc_reaction(
        &self,
        chat: &Jid,
        mut target_key: wa::MessageKey,
        emoji: &str,
    ) -> Result<SendRequest, SendError> {
        let (author, secret) = self
            .resolve_outgoing_addon_parent(chat, &target_key)
            .await?;
        let target_id = target_key
            .id
            .clone()
            .ok_or_else(|| SendError::InvalidRequest("target message key missing id".into()))?;
        // Receivers derive the addon key with the STANZA sender, which in a
        // CAG is our LID identity regardless of the parent author's namespace;
        // mirror the comment path (WA Web authors CAG addons under LID).
        let reactor = self
            .lid()
            .or_else(|| self.pn())
            .map(|j| j.to_non_ad())
            .ok_or(SendError::NotLoggedIn)?;

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

        Ok(encrypted_reaction_request(
            chat,
            target_key,
            enc_payload,
            &iv,
        ))
    }
}

// Construct full protobufs outside async bodies so their uninitialized slots
// do not remain in every suspended reaction future after sharing the content.
fn plain_reaction_request(chat: &Jid, key: wa::MessageKey, emoji: &str) -> SendRequest {
    let message =
        wacore::proto_helpers::build_reaction_message(key, emoji, wacore::time::now_millis());
    SendRequest::new(chat, message)
}

fn encrypted_reaction_request(
    chat: &Jid,
    key: wa::MessageKey,
    payload: Vec<u8>,
    iv: &[u8],
) -> SendRequest {
    let mut message = wa::Message::default();
    message.enc_reaction_message = MessageField::some({
        let mut reaction = wa::message::EncReactionMessage::default();
        reaction.target_message_key = MessageField::some(key);
        reaction.enc_payload = Some(payload);
        reaction.enc_iv = Some(iv.to_vec());
        reaction
    });
    SendRequest::new(chat, message)
}

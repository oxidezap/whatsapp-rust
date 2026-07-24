use super::*;

impl Client {
    /// Delete a message for everyone in the chat (revoke).
    ///
    /// This sends a revoke protocol message that removes the message for all participants.
    /// The message will show as "This message was deleted" for recipients.
    ///
    /// # Arguments
    /// * `to` - The chat JID (DM or group)
    /// * `message_id` - The ID of the message to delete
    /// * `revoke_type` - Use `RevokeType::Sender` to delete your own message,
    ///   or `RevokeType::Admin { original_sender }` to delete another user's message as group admin
    pub async fn revoke_message(
        &self,
        to: impl Into<Jid>,
        message_id: impl Into<String>,
        revoke_type: RevokeType,
    ) -> Result<(), SendError> {
        self.revoke_message_inner(to.into(), message_id.into(), revoke_type)
            .await
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.send.revoke", level = "debug", skip_all, fields(to = %to.observe()), err(Debug)))]
    async fn revoke_message_inner(
        &self,
        to: Jid,
        message_id: String,
        revoke_type: RevokeType,
    ) -> Result<(), SendError> {
        self.require_pn().map_err(SendError::from_anyhow)?;

        let (from_me, participant, edit_attr) = match &revoke_type {
            RevokeType::Sender => {
                // For sender revoke, participant is NOT set (from_me=true identifies it)
                // This matches whatsmeow's BuildMessageKey behavior
                (true, None, EditAttribute::SenderRevoke)
            }
            RevokeType::Admin { original_sender } => {
                // Admin revoke requires group context
                if !to.is_group() {
                    return Err(SendError::InvalidRequest(
                        "admin revoke is only valid for group chats".into(),
                    ));
                }
                // The protocolMessageKey.participant should match the original message's key exactly
                // Do NOT convert LID to PN - pass through unchanged like WhatsApp Web does
                let participant_str = original_sender.to_non_ad_string();
                log::debug!(
                    "Admin revoke: using participant {} for MessageKey",
                    participant_str
                );
                (false, Some(participant_str), EditAttribute::AdminRevoke)
            }
        };

        let revoke_message = build_revoke_message(&to, from_me, message_id, participant);

        // The revoke message stanza needs a NEW unique ID, not the message ID being revoked
        // The message_id being revoked is already in protocolMessage.key.id
        // Passing None generates a fresh stanza ID
        //
        // For admin revokes, force SKDM distribution to get the proper message structure
        // with phash, <participants>, and <device-identity> that WhatsApp Web uses
        let force_skdm = matches!(revoke_type, RevokeType::Admin { .. });
        self.send_message_impl(
            to,
            &revoke_message,
            SendPipelineOptions {
                force_key_distribution: force_skdm,
                edit: Some(edit_attr),
                ..Default::default()
            },
        )
        .await
        .map_err(SendError::from_anyhow)?;
        Ok(())
    }

    /// Keep (or un-keep) a message in a disappearing chat for everyone.
    ///
    /// Sends a `keepInChatMessage` add-on (WA Web `WAWebKeepInChatMsgAction`):
    /// `keep = true` requests `KEEP_FOR_ALL`, `keep = false` requests
    /// `UNDO_KEEP_FOR_ALL`. `key` is the target (kept) message's key; the keep
    /// message itself is sent with a fresh id. The send path classifies this as a
    /// text add-on and maps the undo case to a sender-revoke edit attribute.
    pub async fn keep_message(
        &self,
        chat: impl Into<Jid>,
        key: wa::MessageKey,
        keep: bool,
    ) -> Result<SendResult, SendError> {
        let chat = chat.into();
        let message = wacore::proto_helpers::build_keep_in_chat_message(
            key,
            keep,
            wacore::time::now_millis(),
        );
        self.send_message(chat, message).await
    }

    /// Pin a message in a chat for all participants.
    pub async fn pin_message(
        &self,
        chat: impl Into<Jid>,
        key: wa::MessageKey,
        duration: PinDuration,
    ) -> Result<(), SendError> {
        self.send_pin(
            chat.into(),
            key,
            wa::message::pin_in_chat_message::Type::PinForAll,
            duration.as_secs(),
        )
        .await
    }

    /// Unpin a previously pinned message.
    pub async fn unpin_message(
        &self,
        chat: impl Into<Jid>,
        key: wa::MessageKey,
    ) -> Result<(), SendError> {
        self.send_pin(
            chat.into(),
            key,
            wa::message::pin_in_chat_message::Type::UnpinForAll,
            0,
        )
        .await
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.send.pin", level = "debug", skip_all, fields(chat = %chat.observe()), err(Debug)))]
    async fn send_pin(
        &self,
        chat: Jid,
        key: wa::MessageKey,
        pin_type: wa::message::pin_in_chat_message::Type,
        duration_secs: u32,
    ) -> Result<(), SendError> {
        let message = wa::Message {
            pin_in_chat_message: buffa::MessageField::some(wa::message::PinInChatMessage {
                key: buffa::MessageField::some(key),
                r#type: Some(pin_type),
                sender_timestamp_ms: Some(wacore::time::now_millis()),
            }),
            message_context_info: buffa::MessageField::some(wa::MessageContextInfo {
                message_add_on_duration_in_secs: Some(duration_secs),
                ..Default::default()
            }),
            ..Default::default()
        };

        self.send_message_impl(
            chat,
            &message,
            SendPipelineOptions {
                edit: Some(EditAttribute::PinInChat),
                ..Default::default()
            },
        )
        .await
        .map_err(SendError::from_anyhow)?;
        Ok(())
    }
}

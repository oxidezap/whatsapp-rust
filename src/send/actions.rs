use super::*;

impl Client {
    pub(crate) async fn message_ref_addon_key(
        &self,
        target: &crate::MessageRef<'_>,
    ) -> Result<wa::MessageKey, SendError> {
        let mut key = target.to_raw_key();
        // Add-ons carry the original group author, not their own sender. A
        // borrowed own-send reference lacks that author; resolve it here without
        // the raw identity helper's fallback on unavailable routing/LID state.
        if target.chat().is_group() && target.from_me() && key.participant.is_none() {
            let routing = self.groups().routing_info(target.chat()).await?;
            let own = self.persistence_manager.get_device_snapshot();
            let author = match routing.addressing_mode {
                AddressingMode::Pn => own.pn.as_ref().ok_or(SendError::NotLoggedIn)?,
                AddressingMode::Lid => own
                    .lid
                    .as_ref()
                    .ok_or(crate::MessageRefError::MissingSender)?,
            };
            key.participant = Some(author.to_non_ad_string());
        }
        Ok(key)
    }

    /// Revoke using the original author/from-me scope. Own messages use a
    /// sender revoke (no participant); other authors require group admin
    /// rights, checked by the server. The reference grants no permissions.
    pub async fn revoke_message(
        &self,
        target: &crate::MessageRef<'_>,
    ) -> Result<SendResult, SendError> {
        target.require_chat_operation()?;
        let kind = if target.from_me() {
            RevokeType::Sender
        } else {
            if !target.chat().is_group() {
                return Err(crate::MessageRefError::UnsupportedOrigin.into());
            }
            RevokeType::Admin {
                original_sender: target
                    .sender()
                    .ok_or(crate::MessageRefError::MissingSender)?
                    .clone(),
            }
        };
        self.revoke_message_raw(target.chat(), target.id().as_str(), kind)
            .await
    }

    /// Keep/unkeep the addressed message. Use [`Client::keep_message_raw`]
    /// for explicit chat/key interop.
    pub async fn keep_message(
        &self,
        target: &crate::MessageRef<'_>,
        keep: bool,
    ) -> Result<SendResult, SendError> {
        target.require_chat_operation()?;
        self.keep_message_raw(
            target.chat(),
            self.message_ref_addon_key(target).await?,
            keep,
        )
        .await
    }

    /// Pin the addressed message with a fresh operation id.
    pub async fn pin_message(
        &self,
        target: &crate::MessageRef<'_>,
        duration: PinDuration,
    ) -> Result<SendResult, SendError> {
        target.require_chat_operation()?;
        self.pin_message_raw(
            target.chat(),
            self.message_ref_addon_key(target).await?,
            duration,
        )
        .await
    }

    /// Unpin the addressed message with a fresh operation id.
    pub async fn unpin_message(
        &self,
        target: &crate::MessageRef<'_>,
    ) -> Result<SendResult, SendError> {
        target.require_chat_operation()?;
        self.unpin_message_raw(target.chat(), self.message_ref_addon_key(target).await?)
            .await
    }

    /// Explicit raw addressing and revoke scope for advanced hosts.
    ///
    /// This sends a revoke protocol message that removes the message for all participants.
    /// The message will show as "This message was deleted" for recipients.
    ///
    /// # Arguments
    /// * `to` - The chat JID (DM or group)
    /// * `message_id` - The ID of the message to delete
    /// * `revoke_type` - Use `RevokeType::Sender` to delete your own message,
    ///   or `RevokeType::Admin { original_sender }` to delete another user's message as group admin
    ///
    /// The returned [`SendResult`] describes the revoke itself: `message_id`
    /// is the revoke stanza's own fresh id, and `message` the protocol message
    /// it carried, keyed by the message being deleted.
    pub async fn revoke_message_raw(
        &self,
        to: impl Into<Jid>,
        message_id: impl Into<String>,
        revoke_type: RevokeType,
    ) -> Result<SendResult, SendError> {
        self.revoke_message_inner(to.into(), message_id.into(), revoke_type)
            .await
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.send.revoke", level = "debug", skip_all, fields(to = %to.observe()), err(Debug)))]
    async fn revoke_message_inner(
        &self,
        to: Jid,
        message_id: String,
        revoke_type: RevokeType,
    ) -> Result<SendResult, SendError> {
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
        // A revoke is an ordinary group message: the sender key goes only to devices
        // that do not have it yet, like every other send.
        self.send_built_message(to, revoke_message, edit_attr, None)
            .await
    }

    /// Explicit raw chat/key interop to keep or unkeep a disappearing message.
    ///
    /// Sends a `keepInChatMessage` add-on (WA Web `WAWebKeepInChatMsgAction`):
    /// `keep = true` requests `KEEP_FOR_ALL`, `keep = false` requests
    /// `UNDO_KEEP_FOR_ALL`. `key` is the target (kept) message's key; the keep
    /// message itself is sent with a fresh id. The send path classifies this as a
    /// text add-on and maps the undo case to a sender-revoke edit attribute.
    pub async fn keep_message_raw(
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

    /// Explicit raw chat/key interop to pin a message for all participants.
    ///
    /// The returned [`SendResult`] describes the pin itself: its own fresh
    /// `message_id`, and in `message` the `pinInChatMessage` that was sent.
    pub async fn pin_message_raw(
        &self,
        chat: impl Into<Jid>,
        key: wa::MessageKey,
        duration: PinDuration,
    ) -> Result<SendResult, SendError> {
        self.send_pin(
            chat.into(),
            key,
            wa::message::pin_in_chat_message::Type::PinForAll,
            duration.as_secs(),
        )
        .await
    }

    /// Explicit raw chat/key interop to unpin a message. Returns the unpin's
    /// own [`SendResult`], like [`Client::pin_message`].
    pub async fn unpin_message_raw(
        &self,
        chat: impl Into<Jid>,
        key: wa::MessageKey,
    ) -> Result<SendResult, SendError> {
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
    ) -> Result<SendResult, SendError> {
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

        self.send_built_message(chat, message, EditAttribute::PinInChat, None)
            .await
    }
}

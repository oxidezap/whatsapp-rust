//! Outgoing send primitives, receipts, reactions, edits and chat-state events.

use super::*;

impl Client {
    /// Send pre-marshaled plaintext bytes through the noise socket.
    ///
    /// The bytes must be a valid WABinary-marshaled stanza (as produced by
    /// `wacore_binary::marshal::marshal_to`). Sending malformed data will
    /// cause the server to close the connection.
    ///
    /// This bypasses node logging and `sent_node_waiter` resolution — use
    /// [`send_node`](Client::send_node) for normal stanza sending.
    pub async fn send_raw_bytes(&self, plaintext: Vec<u8>) -> Result<(), ClientError> {
        let noise_socket = self.get_noise_socket().await?;
        noise_socket
            .encrypt_and_send(bytes::Bytes::from(plaintext))
            .await?;
        self.last_data_sent_ms
            .store(wacore::time::now_millis().max(0) as u64, Ordering::Relaxed);
        Ok(())
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.send.node", level = "debug", skip_all, fields(tag = %node.tag), err(Debug)))]
    pub async fn send_node(&self, node: Node) -> Result<(), ClientError> {
        debug!(target: "Client/Send", "{}", DisplayableNode(&node));
        if self.sent_node_waiter_count.load(Ordering::Acquire) > 0 {
            self.resolve_sent_node_waiters(&Arc::new(node.clone()));
        }

        let plaintext_buf = wacore_binary::marshal::marshal_auto(&node).map_err(|e| {
            error!("Failed to marshal node: {e:?}");
            SocketError::Marshal(e)
        })?;

        self.send_raw_bytes(plaintext_buf).await
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.send.unified_session", level = "debug", skip_all)
    )]
    pub(crate) async fn send_unified_session(&self) {
        if !self.is_connected() {
            debug!(target: "Client/UnifiedSession", "Skipping: not connected");
            return;
        }

        let Some((node, _sequence)) = self.unified_session.prepare_send().await else {
            return;
        };

        if let Err(e) = self.send_node(node).await {
            debug!(target: "Client/UnifiedSession", "Send failed: {e}");
            self.unified_session.clear_last_sent().await;
        }
    }

    pub async fn edit_message(
        &self,
        to: impl Into<Jid>,
        original_id: impl Into<String>,
        new_content: wa::Message,
    ) -> Result<String, anyhow::Error> {
        self.edit_message_inner(to.into(), original_id.into(), new_content)
            .await
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.send.edit", level = "debug", skip_all, fields(to = %to.observe()), err(Debug)))]
    async fn edit_message_inner(
        &self,
        to: Jid,
        original_id: String,
        new_content: wa::Message,
    ) -> Result<String, anyhow::Error> {
        // WhatsApp Web uses getMeUserLidOrJidForChat(chat, EditMessage) which
        // returns LID for LID-addressing groups and PN otherwise.
        let participant = if to.is_group() {
            Some(
                self.get_own_jid_for_group(&to)
                    .await?
                    .to_non_ad()
                    .to_string(),
            )
        } else {
            if self.get_pn().is_none() {
                return Err(anyhow::Error::from(ClientError::NotLoggedIn));
            }
            None
        };

        let edit_container_message = crate::send::build_edit_message(
            &to,
            original_id.clone(),
            participant,
            new_content,
            wacore::time::now_millis(),
        );

        // Use a new stanza ID instead of reusing the original message ID.
        // The original message ID is already embedded in protocolMessage.key.id
        // inside the encrypted payload. Reusing it as the outer stanza ID causes
        // the server to deduplicate against the original message and silently
        // drop the edit.
        self.send_message_impl(
            to,
            &edit_container_message,
            None,
            false,
            false,
            Some(crate::types::message::EditAttribute::MessageEdit),
            vec![],
            None,
        )
        .await?;

        Ok(original_id)
    }

    /// Edit a message via the message-secret encrypted path (`secret_encrypted_message`
    /// with `secret_enc_type = MESSAGE_EDIT`), instead of the plaintext protocolMessage
    /// edit. This is the form Community Announcement Group / channel edits require, and
    /// what WA Web sends when `message_edit_to_message_secret_sender_enabled` is on.
    ///
    /// `message_secret` is the *original* message's 32-byte secret (you generated it when
    /// you sent that message). You can only edit your own messages, so the original
    /// sender and the editor are both you.
    pub async fn edit_message_encrypted(
        &self,
        to: impl Into<Jid>,
        original_id: impl Into<String>,
        message_secret: &[u8],
        new_content: wa::Message,
    ) -> Result<String, anyhow::Error> {
        self.edit_message_encrypted_inner(
            to.into(),
            original_id.into(),
            message_secret,
            new_content,
        )
        .await
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.send.edit_encrypted", level = "debug", skip_all, fields(to = %to.observe()), err(Debug)))]
    async fn edit_message_encrypted_inner(
        &self,
        to: Jid,
        original_id: String,
        message_secret: &[u8],
        new_content: wa::Message,
    ) -> Result<String, anyhow::Error> {
        // Newsletters/channels are plaintext (no message-secret addon crypto) and the
        // E2E send path rejects them, so an encrypted edit can't apply there; fail with
        // a clear boundary error instead of the cryptic downstream rejection.
        anyhow::ensure!(
            !to.is_newsletter(),
            "edit_message_encrypted is not valid for newsletters/channels; use edit_message"
        );
        anyhow::ensure!(
            message_secret.len() == 32,
            "message_secret must be exactly 32 bytes, got {}",
            message_secret.len()
        );

        let self_jid = if to.is_group() {
            self.get_own_jid_for_group(&to).await?.to_non_ad()
        } else {
            self.get_pn()
                .ok_or_else(|| anyhow::Error::from(ClientError::NotLoggedIn))?
                .to_non_ad()
        };
        let participant = if to.is_group() {
            Some(self_jid.to_string())
        } else {
            None
        };

        let envelope = build_secret_message_edit(
            &to,
            &original_id,
            participant,
            &self_jid.to_string(),
            message_secret,
            new_content,
        )?;

        self.send_message_impl(
            to,
            &envelope,
            None,
            false,
            false,
            Some(crate::types::message::EditAttribute::MessageEdit),
            vec![],
            None,
        )
        .await?;

        Ok(original_id)
    }

    /// Send a server-side reaction (used by both newsletter and status reactions).
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.send.server_reaction", level = "debug", skip_all, fields(to = %to.observe()), err(Debug)))]
    pub(crate) async fn send_server_reaction(
        &self,
        to: &Jid,
        server_id: u64,
        reaction: &str,
    ) -> Result<(), anyhow::Error> {
        let request_id = self.generate_message_id();

        let stanza = NodeBuilder::new("message")
            .attr("to", to)
            .attr("type", "reaction")
            .attr("id", &request_id)
            .attr("server_id", server_id)
            .children([NodeBuilder::new("reaction").attr("code", reaction).build()])
            .build();

        self.send_node(stanza).await?;
        Ok(())
    }

    /// Register a oneshot waiter for a server ack by message ID.
    /// Returns the receiver — caller sends the node separately and awaits this in background.
    pub(crate) async fn register_ack_waiter(
        &self,
        message_id: &str,
    ) -> futures::channel::oneshot::Receiver<std::sync::Arc<wacore_binary::OwnedNodeRef>> {
        let (tx, rx) = futures::channel::oneshot::channel();
        self.response_waiters
            .lock()
            .await
            .insert(message_id.to_string(), tx);
        rx
    }

    /// Creates a normalized ChatMessageId by resolving PN to LID JIDs.
    pub(crate) async fn make_chat_message_id(&self, chat: &Jid, id: &str) -> ChatMessageId {
        // Resolve chat JID to LID if possible
        let chat = self.resolve_encryption_jid(chat).await;

        ChatMessageId {
            chat,
            id: id.to_owned(),
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.send.protocol_receipt", level = "debug", skip_all)
    )]
    pub(crate) async fn send_protocol_receipt(
        &self,
        id: String,
        receipt_type: crate::types::presence::ReceiptType,
    ) {
        if id.is_empty() {
            return;
        }
        let device_snapshot = self.persistence_manager.get_device_snapshot();
        if let Some(own_jid) = &device_snapshot.pn {
            // Single source of truth for the wire mapping (ReceiptType::Sent is a derived
            // incoming-only state and is never sent by us).
            let type_str = receipt_type.as_wire_str();

            // Borrow `id` for the attr so it stays available for the error log
            // below (the warn used to log self.unique_id, the client UUID, by
            // mistake). Separate .attr calls avoid cloning into a homogeneous map.
            let node = NodeBuilder::new("receipt")
                .attr("id", id.as_str())
                .attr("type", type_str)
                .attr("to", own_jid.to_non_ad_string())
                .build();

            if let Err(e) = self.send_node(node).await {
                warn!(
                    "Failed to send protocol receipt of type {:?} for message ID {}: {:?}",
                    receipt_type, id, e
                );
            }
        }
    }

    /// Register a chatstate handler which will be invoked when a `<chatstate>` stanza is received.
    ///
    /// The handler receives a `ChatStateEvent` with the parsed chat state information.
    pub async fn register_chatstate_handler(
        &self,
        handler: Arc<dyn Fn(ChatStateEvent) + Send + Sync>,
    ) {
        self.chatstate_handlers.write().await.push(handler);
    }

    /// Dispatch a parsed chatstate stanza to registered handlers.
    ///
    /// Called by `ChatstateHandler` after parsing the incoming stanza.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.notif.chatstate", level = "debug", skip_all)
    )]
    pub(crate) async fn dispatch_chatstate_event(
        &self,
        stanza: wacore::iq::chatstate::ChatstateStanza,
    ) {
        use wacore::iq::chatstate::{ChatstateSource, ReceivedChatState};
        use wacore::types::events::ChatPresenceUpdate;
        use wacore::types::message::MessageSource;
        use wacore::types::presence::{ChatPresence, ChatPresenceMedia};

        // Dispatch via event bus
        let (chat, sender, is_group) = match &stanza.source {
            ChatstateSource::User { from } => (from.clone(), from.clone(), false),
            ChatstateSource::Group { from, participant } => {
                (from.clone(), participant.clone(), true)
            }
        };

        let (state, media) = match stanza.state {
            ReceivedChatState::Typing => (ChatPresence::Composing, ChatPresenceMedia::Text),
            ReceivedChatState::RecordingAudio => {
                (ChatPresence::Composing, ChatPresenceMedia::Audio)
            }
            ReceivedChatState::Idle => (ChatPresence::Paused, ChatPresenceMedia::Text),
        };

        self.core
            .event_bus
            .dispatch(Event::ChatPresence(ChatPresenceUpdate {
                source: MessageSource {
                    chat,
                    sender,
                    is_from_me: false,
                    is_group,
                    addressing_mode: None,
                    sender_alt: None,
                    recipient_alt: None,
                    broadcast_list_owner: None,
                    recipient: None,
                },
                state,
                media,
            }));

        // Invoke legacy callback handlers
        let event = ChatStateEvent::from_stanza(stanza);
        let handlers = self.chatstate_handlers.read().await.clone();
        for handler in handlers {
            let event_clone = event.clone();
            self.runtime
                .spawn(Box::pin(async move {
                    (handler)(event_clone);
                }))
                .detach();
        }
    }

    /// Whether delivery receipts should be sent active (rendered as ticks) vs
    /// `type="inactive"`. Mirrors whatsmeow's `sendActiveReceipts != 0`.
    pub(crate) fn receipts_are_active(&self) -> bool {
        self.send_active_receipts.load(Ordering::Acquire) != 0
    }

    /// Force active delivery receipts even when offline (whatsmeow's
    /// `SetForceActiveDeliveryReceipts`); off restores the default.
    pub fn set_force_active_delivery_receipts(&self, active: bool) {
        self.send_active_receipts
            .store(if active { 2 } else { 0 }, Ordering::Release);
    }

    /// CAS so a forced value (2) is preserved (whatsmeow's `CompareAndSwap`).
    pub(crate) fn mark_receipts_active_on_presence(&self) {
        let _ =
            self.send_active_receipts
                .compare_exchange(0, 1, Ordering::AcqRel, Ordering::Acquire);
    }

    pub(crate) fn mark_receipts_inactive_on_presence(&self) {
        let _ =
            self.send_active_receipts
                .compare_exchange(1, 0, Ordering::AcqRel, Ordering::Acquire);
    }
}

/// Build the outgoing `secret_encrypted_message` (MESSAGE_EDIT) envelope: encrypt the
/// protocolMessage(MESSAGE_EDIT) under the original message's secret and wrap it with
/// `messageContextInfo.messageSecret`, matching WAWebGenerateSecretMessageEditProto.
fn build_secret_message_edit(
    to: &Jid,
    original_id: &str,
    participant: Option<String>,
    self_jid_str: &str,
    message_secret: &[u8],
    new_content: wa::Message,
) -> Result<wa::Message, anyhow::Error> {
    let inner = crate::send::build_edit_message(
        to,
        original_id.to_string(),
        participant.clone(),
        new_content,
        wacore::time::now_millis(),
    );

    // You can only edit your own message, so original-sender == editor == self.
    let ctx = wacore::message_edit::MessageEditContext {
        original_msg_id: original_id,
        original_sender_jid: self_jid_str,
        editor_jid: self_jid_str,
    };
    let (enc_payload, iv) =
        wacore::message_edit::encrypt_message_edit(&inner, message_secret, &ctx)?;

    Ok(wa::Message {
        secret_encrypted_message: Some(Box::new(wa::message::SecretEncryptedMessage {
            target_message_key: Some(wa::MessageKey {
                remote_jid: Some(to.to_string()),
                from_me: Some(true),
                id: Some(original_id.to_string()),
                participant,
            }),
            enc_payload: Some(enc_payload),
            enc_iv: Some(iv.to_vec()),
            secret_enc_type: Some(
                wa::message::secret_encrypted_message::SecretEncType::MessageEdit as i32,
            ),
            remote_key_id: None,
        })),
        message_context_info: Some(Box::new(wa::MessageContextInfo {
            message_secret: Some(message_secret.to_vec()),
            ..Default::default()
        })),
        ..Default::default()
    })
}

#[cfg(test)]
mod secret_message_edit_tests {
    use super::*;

    #[test]
    fn secret_message_edit_roundtrip() {
        let secret = [0x33u8; 32];
        let to: Jid = "5511777777777@s.whatsapp.net".parse().unwrap();
        let self_str = "5511999999999@s.whatsapp.net";
        let new_content = wa::Message {
            conversation: Some("edited!".into()),
            ..Default::default()
        };

        let envelope =
            build_secret_message_edit(&to, "ORIGID", None, self_str, &secret, new_content).unwrap();

        let sem = envelope.secret_encrypted_message.as_ref().unwrap();
        assert_eq!(
            sem.secret_enc_type,
            Some(wa::message::secret_encrypted_message::SecretEncType::MessageEdit as i32)
        );
        // The envelope carries the original secret (WAWebGenerateSecretMessageEditProto).
        assert_eq!(
            envelope
                .message_context_info
                .as_ref()
                .and_then(|c| c.message_secret.as_deref()),
            Some(&secret[..])
        );

        // The recipient decrypts with the original message's secret + same ctx.
        let ctx = wacore::message_edit::MessageEditContext {
            original_msg_id: "ORIGID",
            original_sender_jid: self_str,
            editor_jid: self_str,
        };
        let inner = wacore::message_edit::decrypt_message_edit(
            sem.enc_payload.as_deref().unwrap(),
            sem.enc_iv.as_deref().unwrap(),
            &secret,
            &ctx,
        )
        .unwrap();
        let edited = inner
            .protocol_message
            .and_then(|pm| pm.edited_message)
            .and_then(|m| m.conversation);
        assert_eq!(edited.as_deref(), Some("edited!"));
    }

    #[test]
    fn secret_message_edit_wrong_secret_fails_to_decrypt() {
        let secret = [0x33u8; 32];
        let to: Jid = "5511777777777@s.whatsapp.net".parse().unwrap();
        let self_str = "5511999999999@s.whatsapp.net";
        let envelope = build_secret_message_edit(
            &to,
            "ORIGID",
            None,
            self_str,
            &secret,
            wa::Message {
                conversation: Some("edited!".into()),
                ..Default::default()
            },
        )
        .unwrap();
        let sem = envelope.secret_encrypted_message.as_ref().unwrap();
        let ctx = wacore::message_edit::MessageEditContext {
            original_msg_id: "ORIGID",
            original_sender_jid: self_str,
            editor_jid: self_str,
        };
        assert!(
            wacore::message_edit::decrypt_message_edit(
                sem.enc_payload.as_deref().unwrap(),
                sem.enc_iv.as_deref().unwrap(),
                &[0x00u8; 32],
                &ctx,
            )
            .is_err()
        );
    }
}

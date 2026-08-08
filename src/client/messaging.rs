//! Outgoing send primitives, receipts, reactions, edits and chat-state events.

use super::*;

impl Client {
    /// Send a pre-marshaled stanza through the noise socket.
    ///
    /// The bytes must be a packed payload: the format byte followed by the node
    /// bytes, which is what every `wacore_binary::marshal::marshal*` function
    /// writes. A stanza that came off the wire is one byte short of that, since
    /// `OwnedNodeRef::backing_bytes()` returns node bytes only and the receive
    /// path stripped the format byte, so forward it through
    /// `wacore_binary::util::pack`. Anything else the server answers by closing
    /// the connection, which is why the format byte is checked here.
    ///
    /// This bypasses node logging and `sent_node_waiter` resolution — use
    /// [`send_node`](Client::send_node) for normal stanza sending.
    pub async fn send_raw_bytes(&self, plaintext: Vec<u8>) -> Result<(), ClientError> {
        wacore_binary::util::check_plain_payload(&plaintext).map_err(SocketError::Marshal)?;
        let noise_socket = self.get_noise_socket()?;
        // Wire bytes and the last-sent timestamp are recorded by the noise
        // sender task at the actual transport write.
        noise_socket
            .encrypt_and_send(bytes::Bytes::from(plaintext))
            .await?;
        Ok(())
    }

    /// Receivers a burst holds without allocating. Both callers cap their batch
    /// at 4 ([`MAX_ACK_BURST`](Self::MAX_ACK_BURST) and
    /// [`MAX_RECEIPT_BURST`](Self::MAX_RECEIPT_BURST)), so a real burst fits.
    pub(crate) const MAX_INLINE_BURST: usize = 4;

    /// Send several pre-marshaled stanzas as one burst, returning a result per
    /// stanza in the order given.
    ///
    /// The noise sender coalesces whatever is queued when it wakes, but a
    /// worker that awaits each send before starting the next never has two
    /// frames queued at once, so the coalescing it was built for never fires.
    /// Handing over the whole burst is what turns batching from incidental into
    /// the normal case.
    ///
    /// Order is preserved, which the ack worker depends on.
    ///
    /// Always drains `frames`, including when no socket is installed, while
    /// retaining its outer allocation for the persistent workers to reuse.
    ///
    /// Results land in `results`, which the caller owns and reuses too. A
    /// returned `Vec` would allocate once per burst, and the common burst is a
    /// single frame, so that allocation was the dominant cost of sending one.
    /// A multi-frame burst holds its receivers inline up to
    /// [`MAX_INLINE_BURST`](Self::MAX_INLINE_BURST), so it does not allocate
    /// either; beyond that the spill is one allocation, which is what the
    /// previous `join_all` cost every time.
    pub(crate) async fn send_raw_bytes_burst(
        &self,
        frames: &mut Vec<Vec<u8>>,
        results: &mut Vec<crate::socket::error::EncryptSendResult>,
    ) -> Result<(), ClientError> {
        results.clear();
        let noise_socket = match self.get_noise_socket() {
            Ok(socket) => socket,
            Err(error) => {
                frames.clear();
                return Err(error);
            }
        };
        if frames.len() == 1 {
            let plaintext = frames.pop().expect("length checked");
            results.push(
                noise_socket
                    .encrypt_and_send(bytes::Bytes::from(plaintext))
                    .await,
            );
            return Ok(());
        }
        // Every frame is enqueued before any is awaited, which is what lets the
        // sender coalesce them into one transport write; awaiting each before
        // enqueueing the next would hand them over one completion apart. The
        // receivers live inline for the burst sizes both callers cap at, so
        // unlike `join_all` this neither allocates storage for the futures nor
        // a `Vec` for their results.
        let mut receivers: smallvec::SmallVec<[_; Self::MAX_INLINE_BURST]> =
            smallvec::SmallVec::new();
        // An enqueue only fails once the sender task is gone, which no later
        // frame recovers from. Recording where it happened keeps `results`
        // aligned with the frames that were drained: a caller reporting a
        // failure against the wrong frame is worse than the failure.
        let mut frames_after_enqueue_failed = 0usize;
        for plaintext in frames.drain(..) {
            if frames_after_enqueue_failed > 0 {
                frames_after_enqueue_failed += 1;
                continue;
            }
            match noise_socket
                .enqueue_send(bytes::Bytes::from(plaintext))
                .await
            {
                Ok(receiver) => receivers.push(receiver),
                Err(_) => frames_after_enqueue_failed = 1,
            }
        }

        for receiver in receivers {
            results.push(NoiseSocket::await_send(receiver).await);
        }
        for _ in 0..frames_after_enqueue_failed {
            results.push(Err(EncryptSendError::channel_closed()));
        }
        Ok(())
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.send.node", level = "debug", skip_all, fields(tag = %node.tag), err(Debug)))]
    pub async fn send_node(&self, node: Node) -> Result<(), ClientError> {
        let plaintext_buf = self.marshal_node_for_send(node)?;
        self.send_raw_bytes(plaintext_buf).await
    }

    /// Everything [`send_node`](Client::send_node) does short of the send:
    /// logging, waiter resolution and marshalling. Split out so a burst can
    /// marshal its whole batch before touching the socket, which is what keeps
    /// the sends orderable.
    pub(crate) fn marshal_node_for_send(&self, node: Node) -> Result<Vec<u8>, ClientError> {
        debug!(target: "Client/Send", "{}", DisplayableNode(&node));
        if self.sent_node_waiter_count.load(Ordering::Acquire) > 0 {
            self.resolve_sent_node_waiters(&Arc::new(node.clone()));
        }

        // Exact two-pass sizing: typical stanzas are a few hundred bytes, so
        // the 1 KiB default reserve of the one-pass path mostly over-allocates.
        wacore_binary::marshal::marshal_exact(&node).map_err(|e| {
            error!("Failed to marshal node: {e:?}");
            SocketError::Marshal(e).into()
        })
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
    ) -> Result<String, crate::send::SendError> {
        self.edit_message_inner(to.into(), original_id.into(), new_content, None)
            .await
    }

    /// Edits a message you own (`original_id`) with caller-supplied
    /// [`crate::send::EditOptions`]. The edit-path counterpart of
    /// [`crate::send::SendOptions::message_id`] (which overrides the stanza id
    /// for plain sends): `stanza_id` lets callers control the outer stanza id —
    /// for example to collide it with an existing message so clients re-render
    /// that slot.
    ///
    /// When `stanza_id` is set, no id-keyed local state is bound to the borrowed
    /// id (the edit skips outbound-secret and retry-cache persistence, leaving
    /// the original message's state intact), and whether the collision is
    /// honored is server/client dependent — treat it as best-effort. See
    /// [`crate::send::EditOptions::stanza_id`].
    pub async fn edit_message_with_options(
        &self,
        to: impl Into<Jid>,
        original_id: impl Into<String>,
        new_content: wa::Message,
        options: crate::send::EditOptions,
    ) -> Result<String, crate::send::SendError> {
        self.edit_message_inner(
            to.into(),
            original_id.into(),
            new_content,
            options.stanza_id,
        )
        .await
    }

    /// Shared edit-send flow for [`Self::edit_message`] and
    /// [`Self::edit_message_with_options`]. `request_id` overrides the outer
    /// stanza id when `Some`; when `None` a fresh one is generated (the default,
    /// safe behavior — see below).
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.send.edit", level = "debug", skip_all, fields(to = %to.observe()), err(Debug)))]
    async fn edit_message_inner(
        &self,
        to: Jid,
        original_id: String,
        new_content: wa::Message,
        request_id: Option<String>,
    ) -> Result<String, crate::send::SendError> {
        // WhatsApp Web uses getMeUserLidOrJidForChat(chat, EditMessage) which
        // returns LID for LID-addressing groups and PN otherwise.
        let participant = if to.is_group() {
            Some(
                self.get_own_jid_for_group(&to)
                    .await
                    .map_err(crate::send::SendError::from_anyhow)?
                    .to_non_ad()
                    .to_string(),
            )
        } else {
            if self.pn().is_none() {
                return Err(crate::send::SendError::NotLoggedIn);
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

        // Default (`request_id = None`) uses a fresh stanza ID instead of
        // reusing the original message ID: the original ID is already embedded
        // in protocolMessage.key.id inside the encrypted payload, and reusing it
        // as the outer stanza ID makes the server deduplicate against the
        // original message and silently drop the edit. Callers that intentionally
        // want to pin the outer stanza id pass it via `request_id`; that id is
        // borrowed from another message, so id-keyed state (retry cache, outbound
        // secret) must not be bound to it.
        let borrowed_message_id = request_id.is_some();
        self.send_message_impl(
            to,
            &edit_container_message,
            crate::send::SendPipelineOptions {
                edit: Some(crate::types::message::EditAttribute::MessageEdit),
                request_id: request_id.as_deref(),
                borrowed_message_id,
                ..Default::default()
            },
        )
        .await
        .map_err(crate::send::SendError::from_anyhow)?;

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
    ) -> Result<String, crate::send::SendError> {
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
    ) -> Result<String, crate::send::SendError> {
        use crate::send::SendError;
        // Newsletters/channels are plaintext (no message-secret addon crypto) and the
        // E2E send path rejects them, so an encrypted edit can't apply there; fail with
        // a clear boundary error instead of the cryptic downstream rejection.
        if to.is_newsletter() {
            return Err(SendError::InvalidRequest(
                "edit_message_encrypted is not valid for newsletters/channels; use newsletter().edit_message"
                    .into(),
            ));
        }
        if message_secret.len() != 32 {
            return Err(SendError::InvalidRequest(format!(
                "message_secret must be exactly 32 bytes, got {}",
                message_secret.len()
            )));
        }

        let self_jid = if to.is_group() {
            self.get_own_jid_for_group(&to)
                .await
                .map_err(SendError::from_anyhow)?
                .to_non_ad()
        } else {
            self.pn().ok_or(SendError::NotLoggedIn)?.to_non_ad()
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
            crate::send::SendPipelineOptions {
                edit: Some(crate::types::message::EditAttribute::MessageEdit),
                ..Default::default()
            },
        )
        .await
        .map_err(SendError::from_anyhow)?;

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
    /// Sync: registration is just a `std::sync::Mutex` insert (no await).
    /// Register a waiter that receives the ack node itself.
    ///
    /// Used where the caller needs the response: the VoIP offer reads the relay
    /// out of its ack. A phash check does not, which is why that path uses
    /// [`Self::register_phash_waiter`] and pays no channel per message. Gated on
    /// the only consumer's feature, or it is dead code in a default build.
    #[cfg(feature = "voip-runtime")]
    pub(crate) fn register_ack_waiter(
        &self,
        message_id: &str,
    ) -> futures::channel::oneshot::Receiver<Arc<wacore_binary::OwnedNodeRef>> {
        let (tx, rx) = futures::channel::oneshot::channel();
        self.response_waiters_guard()
            .insert(message_id.to_string(), ResponseWaiter::Iq(tx));
        rx
    }

    /// Register the phash the server is expected to echo for this send.
    ///
    /// Nothing awaits the result: the read loop compares inline when the ack
    /// lands and only acts on a mismatch, so a send costs a map entry instead of
    /// a task, a oneshot and a timer.
    pub(crate) fn register_phash_waiter(
        &self,
        message_id: &str,
        expected: wacore_binary::CompactString,
        jid: Jid,
        invalidate_group_cache: bool,
    ) {
        let mut waiters = self.response_waiters_guard();
        // Stamped with the sweep epoch under the lock the insert already holds:
        // a deadline derived from the instant the send started would already be
        // stale here when preparation is slow, and a wall clock can jump.
        let registered_epoch = waiters.current_epoch();
        waiters.insert(
            message_id.to_string(),
            ResponseWaiter::Phash(PhashWaiter {
                expected,
                jid,
                invalidate_group_cache,
                registered_epoch,
            }),
        );
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
    pub fn register_chatstate_handler(&self, handler: Arc<dyn Fn(ChatStateEvent) + Send + Sync>) {
        let mut guard = self
            .chatstate_handlers
            .write()
            .unwrap_or_else(|p| p.into_inner());
        let mut handlers = Vec::with_capacity(guard.len() + 1);
        handlers.extend(guard.iter().cloned());
        handlers.push(handler);
        *guard = Arc::from(handlers);
        // Published after the snapshot is in place, so a reader that sees a
        // non-zero count always finds the handler behind it.
        self.chatstate_handler_count
            .store(guard.len(), Ordering::Release);
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

        self.core.event_bus.dispatch(Event::ChatPresence(
            ChatPresenceUpdate::builder()
                .source(MessageSource {
                    chat,
                    sender,
                    is_from_me: false,
                    is_group,
                    addressing_mode: None,
                    sender_alt: None,
                    recipient_alt: None,
                    broadcast_list_owner: None,
                    recipient: None,
                })
                .state(state)
                .media(media)
                .build(),
        ));

        // Invoke legacy callback handlers. Building the event is only worth it
        // once something reads it, and the default registers nothing.
        if self.chatstate_handler_count.load(Ordering::Acquire) == 0 {
            return;
        }
        #[cfg(test)]
        self.chatstate_events_built.fetch_add(1, Ordering::Release);
        let event = ChatStateEvent::from_stanza(stanza);
        let handlers = self
            .chatstate_handlers
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        for handler in handlers.iter().cloned() {
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
        secret_encrypted_message: buffa::MessageField::some(wa::message::SecretEncryptedMessage {
            target_message_key: buffa::MessageField::some(wa::MessageKey {
                remote_jid: Some(to.to_string()),
                from_me: Some(true),
                id: Some(original_id.to_string()),
                participant,
            }),
            enc_payload: Some(enc_payload),
            enc_iv: Some(iv.to_vec()),
            secret_enc_type: Some(
                wa::message::secret_encrypted_message::SecretEncType::MessageEdit,
            ),
            remote_key_id: None,
        }),
        message_context_info: buffa::MessageField::some(wa::MessageContextInfo {
            message_secret: Some(message_secret.to_vec()),
            ..Default::default()
        }),
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

        let sem = envelope.secret_encrypted_message.as_option().unwrap();
        assert_eq!(
            sem.secret_enc_type,
            Some(wa::message::secret_encrypted_message::SecretEncType::MessageEdit)
        );
        // The envelope carries the original secret (WAWebGenerateSecretMessageEditProto).
        assert_eq!(
            envelope
                .message_context_info
                .as_option()
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
            .into_option()
            .and_then(|pm| pm.edited_message.into_option())
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
        let sem = envelope.secret_encrypted_message.as_option().unwrap();
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

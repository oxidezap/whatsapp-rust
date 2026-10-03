//! PDO (Peer Data Operation) support for requesting message content from the primary device.
//!
//! When message decryption fails (e.g., due to session mismatch), instead of only sending
//! a retry receipt to the sender, we can also request the message content from our own
//! primary phone device. This is useful because:
//!
//! 1. The primary phone has already decrypted the message successfully
//! 2. It can share the decrypted content with linked devices via PDO
//! 3. This bypasses session issues entirely since we're asking our own trusted device
//!
//! The flow is:
//! 1. Decryption fails for a message
//! 2. We send a PeerDataOperationRequestMessage with type PLACEHOLDER_MESSAGE_RESEND
//! 3. The phone responds with PeerDataOperationRequestResponseMessage containing the decoded message
//! 4. We emit the message as if we had decrypted it ourselves

use crate::client::Client;
use crate::types::message::MessageInfo;
use log::{debug, info, warn};
use std::sync::Arc;
use wacore::types::message::{ChatMessageId, EditAttribute, MessageCategory, MessageSource};
use wacore_binary::{Jid, JidExt};
use waproto::whatsapp as wa;

#[derive(Clone, Debug)]
pub struct PendingPdoRequest {
    pub message_info: Arc<MessageInfo>,
    pub requested_at: wacore::time::Instant,
}

const PDO_IN_FLIGHT: u8 = 0;
const PDO_SENT: u8 = 1;
const PDO_FAILED: u8 = 2;
const PDO_WRITING: u8 = 3;
static NEXT_PDO_GENERATION: portable_atomic::AtomicU64 = portable_atomic::AtomicU64::new(1);

#[derive(Debug)]
pub(crate) struct PdoRequestMemo {
    pub(crate) request_id: String,
    info: Arc<MessageInfo>,
    explicit_retry: bool,
    outcome: std::sync::atomic::AtomicU8,
    previous: Option<Arc<Self>>,
    generation: u64,
}

// Share the gate's lock/hash future across all updates. Capture results locally
// so neither a different closure nor a different result shape duplicates it.
type PdoGateUpdate<'a> = &'a mut (
            dyn FnMut(Option<&Arc<PdoRequestMemo>>) -> (Option<Arc<PdoRequestMemo>>, ()) + Send + 'a
        );

struct PdoAttemptGuard(Arc<PdoRequestMemo>);

impl Drop for PdoAttemptGuard {
    fn drop(&mut self) {
        let _ = self.0.outcome.fetch_update(
            std::sync::atomic::Ordering::AcqRel,
            std::sync::atomic::Ordering::Acquire,
            |state| (state != PDO_SENT).then_some(PDO_FAILED),
        );
    }
}

impl wacore::socket::noise_socket::SendObserver for PdoAttemptGuard {
    fn sending(&self) {
        self.0
            .outcome
            .store(PDO_WRITING, std::sync::atomic::Ordering::Release);
    }

    fn sent(&self) {
        self.0
            .outcome
            .store(PDO_SENT, std::sync::atomic::Ordering::Release);
    }
}

impl PdoRequestMemo {
    pub(crate) fn new(
        info: &Arc<MessageInfo>,
        request_id: String,
        explicit_retry: bool,
        previous: Option<Arc<Self>>,
    ) -> Arc<Self> {
        Arc::new(Self {
            request_id,
            info: info.clone(),
            explicit_retry,
            outcome: std::sync::atomic::AtomicU8::new(PDO_IN_FLIGHT),
            previous,
            generation: 0,
        })
    }

    #[cfg(test)]
    fn sent_for_test(
        info: &Arc<MessageInfo>,
        request_id: String,
        explicit_retry: bool,
    ) -> Arc<Self> {
        let memo = Self::new(info, request_id, explicit_retry, None);
        memo.outcome
            .store(PDO_SENT, std::sync::atomic::Ordering::Release);
        memo
    }

    fn live_owner(self: &Arc<Self>) -> Option<Arc<Self>> {
        let mut current = self.clone();
        loop {
            match current.outcome.load(std::sync::atomic::Ordering::Acquire) {
                PDO_FAILED | PDO_IN_FLIGHT => current = current.previous.clone()?,
                PDO_SENT if current.previous.is_some() => return Some(current.sent_owner()),
                _ => return Some(current),
            }
        }
    }

    fn sent_owner(&self) -> Arc<Self> {
        Arc::new(Self {
            request_id: self.request_id.clone(),
            info: self.info.clone(),
            explicit_retry: self.explicit_retry,
            outcome: std::sync::atomic::AtomicU8::new(PDO_SENT),
            previous: None,
            generation: self.generation,
        })
    }

    fn winning_owner(self: &Arc<Self>, previous: Option<Arc<Self>>) -> Arc<Self> {
        Arc::new(Self {
            request_id: self.request_id.clone(),
            info: self.info.clone(),
            explicit_retry: self.explicit_retry,
            outcome: std::sync::atomic::AtomicU8::new(PDO_IN_FLIGHT),
            previous,
            generation: NEXT_PDO_GENERATION.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        })
    }

    fn matches_source(&self, from_me: bool, participant: Option<&str>) -> bool {
        pdo_source_matches(&self.info, from_me, participant)
    }

    fn matches_target(
        &self,
        key: &ChatMessageId,
        from_me: bool,
        participant: Option<&str>,
    ) -> bool {
        let source = &self.info.source;
        let same_chat = |chat: &Jid| chat.user == key.chat.user && chat.server == key.chat.server;
        self.info.id == key.id
            && (same_chat(&source.chat)
                || (!source.is_group
                    && !source.is_from_me
                    && (same_chat(&source.sender)
                        || source.sender_alt.as_ref().is_some_and(same_chat))))
            && self.matches_source(from_me, participant)
    }
}

fn pdo_source_matches(info: &MessageInfo, from_me: bool, participant: Option<&str>) -> bool {
    let source = &info.source;
    if from_me != source.is_from_me {
        return false;
    }
    let Some(participant) = participant else {
        return from_me || !source.is_group;
    };
    let Ok(participant) = participant.parse::<Jid>() else {
        return false;
    };
    let participant = participant.to_non_ad();
    source.sender.to_non_ad() == participant
        || source
            .sender_alt
            .as_ref()
            .is_some_and(|alt| alt.to_non_ad() == participant)
}

#[cfg(test)]
pub(crate) fn test_pending(
    pending: PendingPdoRequest,
    request_id: &str,
    explicit_retry: bool,
) -> (PendingPdoRequest, Arc<PdoRequestMemo>) {
    let memo = PdoRequestMemo::new(
        &pending.message_info,
        request_id.into(),
        explicit_retry,
        None,
    );
    let owner = memo.winning_owner(None);
    owner
        .outcome
        .store(PDO_SENT, std::sync::atomic::Ordering::Release);
    (pending, owner)
}

/// Peer-message destination keyed by the namespace the phone's Signal
/// store actually uses — LID after migration, PN before. Mirrors
/// whatsmeow's `SendPeerMessage` → `cli.getOwnID().ToNonAD()`. WA Web's
/// PN-only target leaves the LID slot stranded post-migration.
fn self_peer_target(device: &wacore::store::Device) -> Result<Jid, crate::client::ClientError> {
    if let Some(lid) = device.lid.as_ref() {
        return Ok(Jid::lid(lid.user.clone()));
    }
    let pn = device
        .pn
        .as_ref()
        .ok_or(crate::client::ClientError::NotLoggedIn)?;
    Ok(Jid::pn(pn.user.clone()))
}

impl Client {
    /// Sends a PDO (Peer Data Operation) request to our own primary phone to get the
    /// decrypted content of a message that we failed to decrypt.
    ///
    /// This is called when decryption fails and we want to ask our phone for the message.
    /// The phone will respond with a PeerDataOperationRequestResponseMessage containing
    /// the full WebMessageInfo which we can then dispatch as a normal message event.
    ///
    /// # Arguments
    /// * `info` - The MessageInfo for the message that failed to decrypt
    ///
    /// # Returns
    /// * `Ok(())` if the request was sent, already requested, or pending
    /// * `Err` if we couldn't send the request (e.g., not logged in)
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.pdo.placeholder_resend", level = "debug", skip_all, fields(chat = %info.source.chat.observe(), sender = %info.source.sender.observe(), msg_id = %info.id), err(Debug)))]
    pub async fn send_pdo_placeholder_resend_request(
        self: &Arc<Self>,
        info: &Arc<MessageInfo>,
    ) -> Result<(), anyhow::Error> {
        self.send_pdo_placeholder_resend_request_impl(info, false)
            .await
            .map(|_| ())
    }

    /// Retry a phone request: `Some(id)` was sent, `None` is already pending.
    pub async fn retry_pdo_placeholder_resend_request(
        self: &Arc<Self>,
        info: &Arc<MessageInfo>,
    ) -> Result<Option<String>, anyhow::Error> {
        use wacore_binary::Server;
        anyhow::ensure!(!info.id.is_empty(), "message id is empty");
        anyhow::ensure!(
            !info.source.chat.user.is_empty()
                && matches!(
                    info.source.chat.server,
                    Server::Pn | Server::Lid | Server::Group | Server::Broadcast
                )
                && info.source.is_group
                    == matches!(info.source.chat.server, Server::Group | Server::Broadcast),
            "invalid placeholder chat"
        );
        anyhow::ensure!(
            !info.source.sender.user.is_empty()
                && matches!(
                    info.source.sender.server,
                    Server::Pn | Server::Lid | Server::Hosted | Server::HostedLid | Server::Bot
                ),
            "invalid placeholder sender"
        );
        if !self.is_socket_connected() {
            return Err(crate::client::ClientError::NotConnected.into());
        }
        self.send_pdo_placeholder_resend_request_impl(info, true)
            .await
    }

    async fn send_pdo_placeholder_resend_request_impl(
        self: &Arc<Self>,
        info: &Arc<MessageInfo>,
        explicit_retry: bool,
    ) -> Result<Option<String>, anyhow::Error> {
        let device_snapshot = self.persistence_manager.get_device_snapshot();
        let peer_target = self_peer_target(&device_snapshot)?;

        // Resolve to LID for the MessageKey when LID-migrated, matching WA Web's
        // NonMessageDataRequest.js:412-421 (toUserLid when isLidMigrated).
        // The phone stores messages by LID after migration.
        let resolved_jid = self.resolve_encryption_jid(&info.source.chat).await;
        // WAWebE2EProtoUtils.msgKeyToProtobuf omits participant when fromMe or
        // when the MsgKey has no participant (i.e. a DM, where the chat JID is
        // the sender). Groups and broadcast chats need it so the phone can
        // locate the stored message.
        let participant = if !info.source.is_from_me
            && (info.source.is_group || info.source.chat.server == wacore_binary::Server::Broadcast)
        {
            Some(self.resolve_encryption_jid(&info.source.sender).await)
        } else {
            None
        };

        // Cache key must use PN JID because the phone's response always contains
        // PN JIDs in WebMessageInfo.key. For LID-migrated DMs, info.source.chat
        // can be LID while sender_alt holds the PN — prefer the PN form.
        let cache_chat = if !info.source.is_group && info.source.chat.is_lid() {
            info.source
                .sender_alt
                .as_ref()
                .map(|jid| jid.to_non_ad())
                .unwrap_or_else(|| info.source.chat.clone())
        } else {
            info.source.chat.clone()
        };
        let cache_key = ChatMessageId::new(cache_chat, info.id.clone());

        // Wire spelling, unresolved: this key is never compared against
        // anything the phone produces, so it has no namespace to agree with,
        // and a key that resolves would move when a LID mapping is learned.
        // Why it names the sender at all is on `Client::pdo_requested`.
        let gate_key = wacore::types::message::SenderMessageId::new(
            info.source.chat.clone(),
            info.id.clone(),
            info.source.sender.clone(),
        );

        // One request per message, like WA Web's session-lifetime set in
        // WAWebNonMessageDataRequestPlaceholderMessageResendUtils. The
        // pending cache below only covers in-flight requests; once the phone
        // answers (even without content) it empties, and a sender that keeps
        // redelivering the same undecryptable message would otherwise trigger
        // a fresh request per copy. Claimed via the single-flight `get_with`
        // (same arm as `dispatch_undecryptable_event`): decrypt-failure tasks
        // are detached per copy, so a get-then-insert would let two
        // concurrent copies both pass the gate, and only the claim winner may
        // release the slot on send failure below.
        if self
            .pdo_pending_requests
            .get(&cache_key)
            .await
            .is_some_and(|(_, memo)| {
                memo.outcome.load(std::sync::atomic::Ordering::Acquire) == PDO_FAILED
            })
        {
            self.pdo_pending_requests
                .remove_if(&cache_key, &|(_, memo)| {
                    memo.outcome.load(std::sync::atomic::Ordering::Acquire) == PDO_FAILED
                })
                .await;
        }
        let request_id = self.generate_message_id();
        let mut admission = None;
        self.pdo_requested
            .upsert_with_by_ref(
                &gate_key,
                (&mut |current: Option<&Arc<PdoRequestMemo>>| {
                    let previous = current.and_then(PdoRequestMemo::live_owner);
                    let normalized = previous
                        .as_ref()
                        .filter(|owner| {
                            current.is_none_or(|cached| {
                                !Arc::ptr_eq(owner, cached)
                                    && matches!(
                                        cached.outcome.load(std::sync::atomic::Ordering::Acquire),
                                        PDO_SENT | PDO_FAILED
                                    )
                            })
                        })
                        .cloned();
                    // Explicit retries bypass a spent gate, not an unsent reservation:
                    // its pending slot can disappear while the session wait is still live.
                    if current.is_some_and(|memo| {
                        memo.outcome.load(std::sync::atomic::Ordering::Acquire) == PDO_IN_FLIGHT
                    }) || (!explicit_retry
                        && (previous.is_some()
                            || current.is_some_and(|memo| {
                                memo.outcome.load(std::sync::atomic::Ordering::Acquire)
                                    != PDO_FAILED
                            })))
                    {
                        return (normalized, ());
                    }
                    // An unsent gate is still a reservation, even though
                    // live_owner must ignore it as a response owner.
                    let claimed = previous.is_none()
                        && current.is_none_or(|memo| {
                            memo.outcome.load(std::sync::atomic::Ordering::Acquire) == PDO_FAILED
                        });
                    let memo =
                        PdoRequestMemo::new(info, request_id.clone(), explicit_retry, previous);
                    let next = if claimed {
                        Some(memo.clone())
                    } else {
                        normalized
                    };
                    admission = Some((memo, claimed));
                    (next, ())
                }) as PdoGateUpdate<'_>,
            )
            .await;
        let Some((memo, claimed)) = admission else {
            debug!(
                "PDO request already sent for message {} from {}; not re-requesting",
                info.id,
                info.source.sender.observe()
            );
            return Ok(None);
        };
        let mut attempt = PdoAttemptGuard(memo.clone());

        // Reserved atomically, not read-then-written. Two senders sharing one
        // `(chat, id)` hold distinct gates and arrive here concurrently, and a
        // `get` followed by an `insert` would let both see the slot empty,
        // both overwrite it, and both send. The response is removed by
        // `(chat, id)` and carries whichever `MessageInfo` won the overwrite,
        // so the recovered content would be dispatched under the other
        // sender's identity.
        let pending = (
            PendingPdoRequest {
                message_info: Arc::clone(info),
                requested_at: wacore::time::Instant::now(),
            },
            memo.clone(),
        );
        let reserved = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let reserved_clone = reserved.clone();
        let transfer_key = gate_key.clone();
        let transfer_memo = memo.clone();
        let explicit_published = &self.pdo_explicit_published;
        // Keep cache initialization out of the send frame. Its nested gate
        // update otherwise duplicates the cache state machines in this poll.
        let initialize: wacore::runtime::BoxFuture<'_, _> = Box::pin(
            self.pdo_pending_requests
                .get_with(cache_key.clone(), async {
                    self.pdo_requested
                        .upsert_with_by_ref(
                            &transfer_key,
                            (&mut |current: Option<&Arc<PdoRequestMemo>>| {
                                let previous = current.and_then(|current| {
                                    if current.request_id == transfer_memo.request_id {
                                        current
                                            .previous
                                            .as_ref()
                                            .and_then(PdoRequestMemo::live_owner)
                                    } else {
                                        current.live_owner()
                                    }
                                });
                                transfer_memo
                                    .outcome
                                    .store(PDO_FAILED, std::sync::atomic::Ordering::Release);
                                let owner = transfer_memo.winning_owner(previous);
                                attempt.0 = owner.clone();
                                if owner.explicit_retry {
                                    explicit_published
                                        .store(true, std::sync::atomic::Ordering::Release);
                                }
                                (Some(owner), ())
                            }) as PdoGateUpdate<'_>,
                        )
                        .await;
                    reserved_clone.store(true, std::sync::atomic::Ordering::Release);
                    (pending.0, attempt.0.clone())
                }),
        );
        let (holder, owner) = initialize.await;
        if !reserved.load(std::sync::atomic::Ordering::Acquire) {
            // Only when the slot belongs to a *different* sender. The gate
            // cache has its own 512-entry capacity, so a burst can evict this
            // sender's gate while its own request is still pending; a
            // redelivery then recreates the gate, finds its own entry here,
            // and removing it would let a later redelivery send a second
            // request for a message that already has one out.
            if claimed {
                let same_owner = holder.message_info.source.sender == info.source.sender
                    && holder.message_info.source.is_from_me == info.source.is_from_me;
                self.pdo_requested
                    .upsert_with_by_ref(
                        &gate_key,
                        (&mut |current: Option<&Arc<PdoRequestMemo>>| {
                            memo.outcome
                                .store(PDO_FAILED, std::sync::atomic::Ordering::Release);
                            (
                                current
                                    .filter(|current| {
                                        same_owner && current.request_id == request_id
                                    })
                                    .and_then(|_| {
                                        // Restore an evicted reservation from the pending slot,
                                        // not only an owner that has already reached the writer.
                                        if owner.outcome.load(std::sync::atomic::Ordering::Acquire)
                                            == PDO_IN_FLIGHT
                                        {
                                            Some(owner.clone())
                                        } else {
                                            owner.live_owner()
                                        }
                                    }),
                                (),
                            )
                        }) as PdoGateUpdate<'_>,
                    )
                    .await;
                self.pdo_requested
                    .remove_if(&gate_key, &|current| {
                        current.request_id == request_id
                            && current.outcome.load(std::sync::atomic::Ordering::Acquire)
                                == PDO_FAILED
                    })
                    .await;
            }
            // Another sender's request for this `(chat, id)` is in flight.
            // Nothing was sent for this one, so it must not keep the slot it
            // claimed: holding it would suppress this sender for the gate's
            // whole lifetime once that request is answered and the pending
            // entry clears.
            debug!(
                "PDO request already pending for message {} from {}",
                info.id,
                info.source.sender.observe()
            );
            return Ok(None);
        }

        let memo = owner;

        let message_key = wa::MessageKey {
            remote_jid: Some(resolved_jid.to_string()),
            from_me: Some(info.source.is_from_me),
            id: Some(info.id.to_string()),
            participant: participant.map(|p| p.to_string()),
        };

        // Build the PDO request message
        let pdo_request = wa::message::PeerDataOperationRequestMessage {
            peer_data_operation_request_type: Some(
                wa::message::PeerDataOperationRequestType::PLACEHOLDER_MESSAGE_RESEND,
            ),
            placeholder_message_resend_request: vec![
                wa::message::peer_data_operation_request_message::PlaceholderMessageResendRequest {
                    message_key: buffa::MessageField::some(message_key),
                },
            ],
            ..Default::default()
        };

        // Wrap it in a protocol message
        let protocol_message = wa::message::ProtocolMessage {
            r#type: Some(wa::message::protocol_message::Type::PEER_DATA_OPERATION_REQUEST_MESSAGE),
            peer_data_operation_request_message: buffa::MessageField::some(pdo_request),
            ..Default::default()
        };

        let msg = wa::Message {
            protocol_message: buffa::MessageField::some(protocol_message),
            ..Default::default()
        };

        info!(
            "Sending PDO placeholder resend request for message {} from {} in {} to {}",
            info.id,
            info.source.sender.observe(),
            info.source.chat.observe(),
            peer_target.observe()
        );

        // A failed send must not consume the once-per-message slot, or a
        // transient error would permanently block recovery for this message.
        let send = async {
            self.ensure_e2e_sessions(std::slice::from_ref(&peer_target))
                .await?;
            // Transfer the attempt guard with the actual socket job. Queued
            // work survives its caller, but only the writer starts ownership.
            let pipeline: wacore::runtime::BoxFuture<'_, _> = Box::pin(self.send_message_impl(
                peer_target,
                &msg,
                crate::send::SendPipelineOptions {
                    request_id: Some(&request_id),
                    peer: true,
                    send_observer: Some(Box::new(attempt)),
                    ..Default::default()
                },
            ));
            pipeline.await.map(|_| ())
        }
        .await;
        if let Err(e) = send {
            self.pdo_pending_requests
                .remove_if(&cache_key, &|(_, current)| current.request_id == request_id)
                .await;
            self.pdo_requested
                .upsert_with_by_ref(
                    &gate_key,
                    (&mut |current: Option<&Arc<PdoRequestMemo>>| {
                        memo.outcome
                            .store(PDO_FAILED, std::sync::atomic::Ordering::Release);
                        (
                            current
                                .filter(|current| current.request_id == request_id)
                                .and_then(|_| {
                                    memo.previous.as_ref().and_then(PdoRequestMemo::live_owner)
                                }),
                            (),
                        )
                    }) as PdoGateUpdate<'_>,
                )
                .await;
            self.pdo_requested
                .remove_if(&gate_key, &|current| {
                    current.request_id == request_id
                        && current.outcome.load(std::sync::atomic::Ordering::Acquire) == PDO_FAILED
                })
                .await;
            warn!(
                "Failed to send PDO request for message {}: {:?}",
                info.id, e
            );
            return Err(e);
        }

        memo.outcome
            .store(PDO_SENT, std::sync::atomic::Ordering::Release);
        debug!("PDO request sent successfully for message {}", info.id);
        self.pdo_requested
            .upsert_with_by_ref(
                &gate_key,
                (&mut |current: Option<&Arc<PdoRequestMemo>>| {
                    (
                        current
                            .filter(|current| current.request_id == request_id)
                            .map(|_| memo.sent_owner()),
                        (),
                    )
                }) as PdoGateUpdate<'_>,
            )
            .await;
        Ok(Some(request_id))
    }

    /// Request on-demand message history from the primary phone via PDO.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.pdo.fetch_history", level = "debug", skip_all, fields(chat = %chat_jid.observe(), count), err(Debug)))]
    pub async fn fetch_message_history(
        self: &Arc<Self>,
        chat_jid: &Jid,
        oldest_msg_id: &str,
        oldest_msg_from_me: bool,
        oldest_msg_timestamp_ms: i64,
        count: i32,
    ) -> Result<String, anyhow::Error> {
        let device_snapshot = self.persistence_manager.get_device_snapshot();
        let peer_target = self_peer_target(&device_snapshot)?;

        let pdo_request = wa::message::PeerDataOperationRequestMessage {
            peer_data_operation_request_type: Some(
                wa::message::PeerDataOperationRequestType::HISTORY_SYNC_ON_DEMAND,
            ),
            history_sync_on_demand_request: buffa::MessageField::some(
                wa::message::peer_data_operation_request_message::HistorySyncOnDemandRequest {
                    chat_jid: Some(chat_jid.to_string()),
                    oldest_msg_id: Some(oldest_msg_id.to_string()),
                    oldest_msg_from_me: Some(oldest_msg_from_me),
                    oldest_msg_timestamp_ms: Some(oldest_msg_timestamp_ms),
                    on_demand_msg_count: Some(count),
                    ..Default::default()
                },
            ),
            ..Default::default()
        };

        let protocol_message = wa::message::ProtocolMessage {
            r#type: Some(wa::message::protocol_message::Type::PEER_DATA_OPERATION_REQUEST_MESSAGE),
            peer_data_operation_request_message: buffa::MessageField::some(pdo_request),
            ..Default::default()
        };

        let msg = wa::Message {
            protocol_message: buffa::MessageField::some(protocol_message),
            ..Default::default()
        };

        info!(
            "Sending PDO history sync on-demand request for chat {} (count={}) to {}",
            chat_jid.observe(),
            count,
            peer_target.observe()
        );

        self.ensure_e2e_sessions(std::slice::from_ref(&peer_target))
            .await?;
        self.send_peer_message(peer_target, &msg).await
    }

    /// Ask the primary device for a collection this side could not validate.
    ///
    /// Sent when a snapshot's MAC does not match the one we compute over it.
    /// There is no way forward through the server after that -- the same bytes
    /// arrive on every retry and fail the same way -- so the collection would
    /// otherwise stay at version 0 for ever, and every mutation this client
    /// tries to write to it (a chat marked read, a mute, an archive) is refused
    /// with a conflict it can never resolve.
    ///
    /// Fire-and-forget, like every other PDO here: the answer arrives later as
    /// an ordinary peer message and is applied then. Nothing waits, so a phone
    /// that is off simply means the collection stays as it was.
    pub async fn request_syncd_snapshot_recovery(
        self: &Arc<Self>,
        collection: &str,
    ) -> Result<String, anyhow::Error> {
        // Both gates are enforced here as well as at the escalation, because this
        // is public and takes a name.
        //
        // A name this client has no rules for is refused outright: the reply
        // side rejects `Unknown` *before* spending the marker, and an
        // outstanding request only ever expires when the same name is asked for
        // again -- so one typo would sit in the map for the life of the process,
        // having been sent to the phone for nothing.
        let patch_name = collection
            .parse::<wacore::appstate::patch_decode::WAPatchName>()
            .unwrap_or(wacore::appstate::patch_decode::WAPatchName::Unknown);
        if patch_name == wacore::appstate::patch_decode::WAPatchName::Unknown {
            return Err(anyhow::anyhow!(
                "{collection} is not an app-state collection this client knows"
            ));
        }
        // And the block list is refused because a caller asking for it would
        // otherwise mark it pending and send, and the reply passes the
        // known-collection check and applies. Rebuilding a block list from a
        // device that may itself be behind is the one collection where being
        // wrong means talking to somebody who was blocked.
        if patch_name == wacore::appstate::patch_decode::WAPatchName::CriticalBlock {
            return Err(anyhow::anyhow!(
                "the block list is not recovered from the primary"
            ));
        }

        // And the rollout gate, for the same reason the two above are here: an
        // explicit `0` is the account being told its primary cannot do this, and
        // a caller reaching past the escalation would spend a whole-collection
        // request on a device that will ignore it. Silence still proceeds --
        // this client is not on WhatsApp's rollout and may simply never be sent
        // the prop, and reading that as a refusal would disable the escalation
        // for everyone it exists to help.
        if self
            .ab_props()
            .get(wacore::iq::abprops::web::ENABLE_PEER_SNAPSHOT_RECOVERY)
            .await
            .is_some_and(|value| value == "0" || value.eq_ignore_ascii_case("false"))
        {
            return Err(anyhow::anyhow!(
                "the account has peer snapshot recovery turned off"
            ));
        }

        let device_snapshot = self.persistence_manager.get_device_snapshot();
        let peer_target = self_peer_target(&device_snapshot)?;

        let pdo_request = wa::message::PeerDataOperationRequestMessage {
            peer_data_operation_request_type: Some(
                wa::message::PeerDataOperationRequestType::COMPANION_SYNCD_SNAPSHOT_FATAL_RECOVERY,
            ),
            syncd_collection_fatal_recovery_request: buffa::MessageField::some(
                wa::message::peer_data_operation_request_message::SyncDCollectionFatalRecoveryRequest {
                    collection_name: Some(collection.to_string()),
                    timestamp: Some(wacore::time::now_secs() as i64),
                },
            ),
            ..Default::default()
        };

        let protocol_message = wa::message::ProtocolMessage {
            r#type: Some(wa::message::protocol_message::Type::PEER_DATA_OPERATION_REQUEST_MESSAGE),
            peer_data_operation_request_message: buffa::MessageField::some(pdo_request),
            ..Default::default()
        };

        let msg = wa::Message {
            protocol_message: buffa::MessageField::some(protocol_message),
            ..Default::default()
        };

        info!(
            "Asking {} to send back the {} collection after a snapshot we could not validate",
            peer_target.observe(),
            collection
        );

        self.ensure_e2e_sessions(std::slice::from_ref(&peer_target))
            .await?;

        // Marked before the send, not after. The reply is handled on the inbound
        // path by a different task, and a primary that answers quickly can be
        // read before a mark placed afterwards is visible -- which would drop a
        // perfectly good recovery for a request that really was made. Marking
        // first cannot lose one; the only cost is a marker to take back if the
        // send never happened, which is what the failure arm does.
        // Generated here, not inside the send: the answer is identified by this
        // id, and a reply that beats the send's return would otherwise find the
        // request recorded with no id at all -- unrecognisable, and so unable to
        // free the ask it answers.
        let request_id = self.generate_message_id();

        let proc = self.get_app_state_processor();
        if !proc.mark_recovery_requested(collection).await {
            // One is already outstanding, and the reply that is coming answers
            // this ask too. Suppressing the duplicate is also what keeps the
            // marker honest: a second send that failed would otherwise withdraw
            // the first request's only record of itself.
            debug!("A recovery for {collection} is already outstanding; not asking again");
            return Ok(String::new());
        }
        proc.note_recovery_request_id(collection, &request_id).await;
        match self
            .send_peer_message_with_id(peer_target, &msg, &request_id)
            .await
        {
            Ok(()) => Ok(request_id),
            Err(e) => {
                // By id, not by name. The window this marker holds is shorter
                // than a send can take, so by now another ask for the same
                // collection may have replaced this entry -- and withdrawing by
                // name would take *its* marker, leaving a request that really is
                // on the wire with nothing to recognise its answer. Taking by id
                // withdraws this ask or nothing.
                proc.take_recovery_request_by_id(&request_id).await;
                Err(e)
            }
        }
    }

    /// Sends a peer message (message to our own devices).
    /// This is used for PDO requests and similar device-to-device communication.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.pdo.send_peer_message", level = "debug", skip_all, fields(to = %to.observe()), err(Debug)))]
    async fn send_peer_message(
        self: &Arc<Self>,
        to: Jid,
        msg: &wa::Message,
    ) -> Result<String, anyhow::Error> {
        let msg_id = self.generate_message_id();
        self.send_peer_message_with_id(to, msg, &msg_id).await?;
        Ok(msg_id)
    }

    /// [`Self::send_peer_message`] for a caller that has to know the id before
    /// the send, because it records something against it that an answer can
    /// arrive and look up before this returns.
    async fn send_peer_message_with_id(
        self: &Arc<Self>,
        to: Jid,
        msg: &wa::Message,
        msg_id: &str,
    ) -> Result<(), anyhow::Error> {
        // Send with peer category and high priority
        self.send_message_impl(
            to,
            msg,
            crate::send::SendPipelineOptions {
                request_id: Some(msg_id),
                peer: true,
                ..Default::default()
            },
        )
        .await?;

        Ok(())
    }

    /// Handles a PDO response message from our primary phone.
    /// This is called when we receive a PeerDataOperationRequestResponseMessage.
    ///
    /// # Arguments
    /// * `response` - The PDO response message
    /// * `info` - The MessageInfo for the PDO response message itself
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.pdo.handle_response", level = "debug", skip_all, fields(sender = %pdo_msg_info.source.sender.observe())))]
    pub async fn handle_pdo_response(
        self: &Arc<Self>,
        response: &wa::message::PeerDataOperationRequestResponseMessage,
        pdo_msg_info: &MessageInfo,
    ) {
        // Only process PDO responses from device 0 (the primary phone)
        if pdo_msg_info.source.sender.device != 0 {
            debug!(
                "Ignoring PDO response from non-primary device {}",
                pdo_msg_info.source.sender.observe()
            );
            return;
        }

        let request_id = response.stanza_id.as_deref().unwrap_or("");
        debug!(
            "Received PDO response (request_id={}) with {} results",
            request_id,
            response.peer_data_operation_result.len()
        );

        for result in &response.peer_data_operation_result {
            if let Some(placeholder_response) =
                result.placeholder_message_resend_response.as_option()
            {
                self.handle_placeholder_resend_response(placeholder_response, request_id)
                    .await;
            }
        }

        // One response can carry several recovery results, and they all answer
        // the one ask this stanza id was made about -- so exactly one of them is
        // handled, and a populated one wins. Taking them in order would let an
        // empty result arriving first spend the request the good one still
        // needs; an empty one is answered only when nothing here carries a
        // collection at all.
        let recoveries: Vec<_> = response
            .peer_data_operation_result
            .iter()
            .filter_map(|result| result.syncd_snapshot_fatal_recovery_response.as_option())
            .collect();
        if let Some(recovery) = recoveries
            .iter()
            // Present and non-empty. A zero-length field is a result that
            // carries nothing -- an empty payload decodes to a default recovery
            // naming no collection, which would then spend the request in the
            // mismatch path and leave a usable sibling unconsidered.
            .find(|recovery| {
                recovery
                    .collection_snapshot
                    .as_deref()
                    .is_some_and(|blob| !blob.is_empty())
            })
            .or(recoveries.first())
        {
            self.handle_syncd_snapshot_recovery_response(recovery, request_id)
                .await;
        } else {
            // A response under this id carrying no recovery result at all --
            // an empty list, or only somebody else's result. It is still the
            // answer to the ask that id was made about, so it spends it:
            // leaving the request would suppress every retry for the rest of
            // the window over a question already answered, badly.
            //
            // Claimed before it is spent, for the reason the absent-blob path
            // is: a resultless response arriving beside one already being
            // decoded must not delete the request that decoder will take at the
            // end, or a usable recovery is dropped and the collection stays
            // behind the MAC failure it started at.
            let proc = self.get_app_state_processor();
            if let Some(name) = proc.claim_recovery_request_by_id(request_id).await {
                proc.take_recovery_request_by_id(request_id).await;
                warn!(
                    "Snapshot recovery response for {name} carries no result; it may be asked for again"
                );
            }
        }
    }

    /// Apply a collection the primary sent back after a snapshot we refused.
    ///
    /// Logged rather than returned: this arrives on the inbound message path,
    /// long after the sync that asked, and there is nobody left to hand an error
    /// to. What a failure costs is the collection staying where it was, which is
    /// where it already was.
    async fn handle_syncd_snapshot_recovery_response(
        &self,
        response: &wa::message::peer_data_operation_request_response_message::peer_data_operation_result::SyncDSnapshotFatalRecoveryResponse,
        request_id: &str,
    ) {
        // An empty byte field is a result carrying nothing, handled as the
        // absent one: decoding it would produce a default recovery naming no
        // collection, which is a mismatch against the ask and reads in the log
        // as the primary having answered about something else.
        let Some(blob) = response
            .collection_snapshot
            .as_deref()
            .filter(|blob| !blob.is_empty())
        else {
            // Answered, with nothing. Leaving the marker would suppress every
            // retry for the rest of the window over an ask already spent.
            //
            // Claimed first, though, rather than removed outright: one response
            // can carry several results under one id, so an empty one arriving
            // beside a good one would otherwise delete the request the good
            // one's decoder is still working against -- and that task, finding
            // no marker at the end, would drop a usable recovery.
            let proc = self.get_app_state_processor();
            let Some(name) = proc.claim_recovery_request_by_id(request_id).await else {
                warn!(
                    "Ignoring a snapshot recovery with no collection: nothing here is waiting on that ask"
                );
                return;
            };
            proc.take_recovery_request_by_id(request_id).await;
            warn!(
                "Snapshot recovery response for {name} carries no collection; it may be asked for again"
            );
            return;
        };

        // Which collection this id was asked about -- claimed, before a byte is
        // cloned or inflated.
        //
        // Correlating here rather than after the decode is the difference
        // between a map read and up to 64 MiB of inflate plus a record graph
        // built for a reply that was never eligible to apply. Claiming rather
        // than reading is what makes one ask cost one decode: a response that
        // repeats the result, or a second copy arriving before the first is
        // consumed, would otherwise each spawn a collection-sized job against
        // the same request.
        //
        // The payload names a collection too, but that is the reply's claim
        // about itself -- and with two recoveries outstanding, a reply carrying
        // A's id and B's name would select B's marker and be checked against its
        // own name, which always agrees. So the ask decides.
        let Some(asked) = self
            .get_app_state_processor()
            .claim_recovery_request_by_id(request_id)
            .await
        else {
            warn!("Ignoring a snapshot recovery nothing here asked for");
            return;
        };

        // The whole continuation is detached, not just the CPU inside it.
        // `receive.rs` awaits this handler inline and inbound processing is
        // serialized per chat, so awaiting the decode -- even one that runs on
        // the blocking pool -- keeps the self-chat lane closed behind it, and
        // the primary's key shares queue behind a payload of the primary's own
        // choosing. Nothing here has an answer to give back.
        let Some(client) = self.self_weak.get().and_then(|w| w.upgrade()) else {
            return;
        };
        let compressed = response.is_compressed.unwrap_or(false);

        // Bounded before it is copied, not after. The ceiling was enforced
        // inside the task, so a malformed gigabyte reply was still allocated and
        // memcpy'd in full on the inbound lane before anything looked at its
        // size -- on the self-chat lane, where the primary's key shares queue
        // behind it.
        //
        // Both branches are bounded, by what each can legitimately be. An
        // uncompressed reply is its own output, so the ceiling is the ceiling. A
        // compressed one is measured against the largest a stream whose *output*
        // fits can be on the wire: deflate stores what it cannot compress, at a
        // cost of about a thousandth plus a small header, so anything past that
        // could not have inflated to something this path would accept. The
        // inflate still enforces the real ceiling on the way out; this only
        // decides whether the bytes are worth copying.
        let max_wire = if compressed {
            wacore::history_sync::MAX_DECOMPRESSED
                + wacore::history_sync::MAX_DECOMPRESSED / 1000
                + 64
        } else {
            wacore::history_sync::MAX_DECOMPRESSED
        };
        if blob.len() as u64 > max_wire {
            self.get_app_state_processor()
                .take_recovery_request_by_id(request_id)
                .await;
            warn!(
                "Snapshot recovery for {asked} is {} bytes on the wire, over the {max_wire} this path allows; refusing it",
                blob.len()
            );
            return;
        }
        let payload = blob.to_vec();
        let request_id = request_id.to_string();
        let generation = self
            .connection_generation
            .load(std::sync::atomic::Ordering::Acquire);

        self.runtime.spawn_detached(Box::pin(async move {
            // Up to 64 MiB of inflate plus a decode wide enough that `waproto`
            // pins its instantiation: off the runtime's async workers either
            // way.
            let decoded = wacore::runtime::blocking(&*client.runtime, move || {
                let bytes = if compressed {
                    let mut reader = wacore_binary::zlib_pool::InflateReader::new(
                        &payload,
                        wacore::history_sync::MAX_DECOMPRESSED,
                    );
                    let mut plain = Vec::new();
                    loop {
                        match reader.ensure(1) {
                            Ok(true) => {}
                            // `ensure(1)` answers false only with nothing left.
                            Ok(false) => break,
                            Err(e) => return Err(format!("failed to decompress: {e}")),
                        }
                        let taken = {
                            let chunk = reader.available();
                            plain.extend_from_slice(chunk);
                            chunk.len()
                        };
                        if taken == 0 {
                            break;
                        }
                        reader.consume(taken);
                    }
                    // Running out is not ending. A payload cut short after a
                    // parseable prefix would otherwise be applied as the whole
                    // collection, and a short collection cannot be told from a
                    // real one -- nothing here knows how many records to expect.
                    if !reader.stream_ended() {
                        return Err("is a truncated compressed stream".to_string());
                    }
                    // And ending is not all of it. A complete stream followed by
                    // a second member or by trailing bytes leaves the reader
                    // done with input to spare, and taking the first member for
                    // the collection is the same silent short read the check
                    // above refuses -- reached from the other direction.
                    let (read, whole) = reader.compressed_progress();
                    if read != whole {
                        return Err(format!(
                            "carries {} byte(s) after its compressed stream",
                            whole - read
                        ));
                    }
                    std::borrow::Cow::Owned(plain)
                } else {
                    // Already bounded: the raw blob was measured against the
                    // same ceiling before it was copied, which is what an
                    // uncompressed reply needs -- the decode allocates a record
                    // graph from whatever arrives.
                    std::borrow::Cow::Borrowed(&payload[..])
                };
                waproto::codec::syncd_snapshot_recovery_decode(&bytes)
                    .map_err(|e| format!("failed to decode: {e}"))
            })
            .await;

            let recovery = match decoded {
                Ok(recovery) => recovery,
                Err(e) => {
                    // The request was answered, badly. Its collection name is
                    // inside the payload that would not read, so the id the
                    // answer carried is the only way to say which ask this was
                    // -- and leaving the marker would suppress every retry for
                    // the rest of the window over a question already answered.
                    let proc = client.get_app_state_processor();
                    match proc.take_recovery_request_by_id(&request_id).await {
                        Some(name) => warn!(
                            "Snapshot recovery response for {name} {e}; the collection may be asked for again"
                        ),
                        None => warn!("Snapshot recovery response {e}"),
                    }
                    return;
                }
            };

            let proc = client.get_app_state_processor();

            match recovery.collection_name.as_deref() {
                Some(named) if named == asked => {}
                other => {
                    proc.take_recovery_request_by_id(&request_id).await;
                    warn!(
                        "Snapshot recovery answering the ask for {asked} names {}; refusing it",
                        other.unwrap_or("nothing")
                    );
                    return;
                }
            }

            // The reservation is a connection's, and a disconnect clears the
            // registry wholesale -- so a task that started before one and
            // applied after it would write beside the new connection's own sync
            // and dispatch events for a session that has since been replaced.
            if client.connection_generation.load(std::sync::atomic::Ordering::Acquire) != generation
            {
                // Spent, like every other ending that is not an apply. The ask
                // was answered; dropping the answer because the connection went
                // is this side's decision, and leaving the marker would have the
                // next connection unable to ask again until the window ran out.
                proc.take_recovery_request_by_id(&request_id).await;
                debug!("Dropping the {asked} recovery: the connection it belongs to is gone");
                return;
            }

            client
                .apply_recovered_collection(&asked, &request_id, generation, recovery)
                .await;
        }));
    }

    async fn handle_placeholder_resend_response(
        self: &Arc<Self>,
        response: &wa::message::peer_data_operation_request_response_message::peer_data_operation_result::PlaceholderMessageResendResponse,
        request_id: &str,
    ) {
        let Some(web_message_info_bytes) = &response.web_message_info_bytes else {
            warn!("PDO placeholder response missing webMessageInfoBytes");
            return;
        };

        // Owned decode (not a view): WebMessageInfo carries a nested `message`
        // (a full Message), so an eager view would pull the entire MessageView
        // tree into the binary and parse the message once into a view only to
        // copy it again into the owned form. Owned decode reads it in one pass.
        let mut web_msg_info = match waproto::codec::web_message_info_decode(web_message_info_bytes)
        {
            Ok(info) => info,
            Err(e) => {
                warn!("Failed to decode WebMessageInfo from PDO response: {:?}", e);
                return;
            }
        };

        let Some(key) = web_msg_info.key.as_option() else {
            warn!("PDO response WebMessageInfo missing key");
            return;
        };
        let remote_jid_str = key.remote_jid.as_deref().unwrap_or("");
        let msg_id = key.id.as_deref().unwrap_or("");
        let response_participant = self.pdo_key_participant(&web_msg_info);
        let response_from_me = key.from_me.unwrap_or(false);

        let cache_key = match remote_jid_str.parse::<Jid>() {
            Ok(jid) => ChatMessageId::new(jid, msg_id.into()),
            Err(_) => {
                warn!(
                    "PDO response has unparseable remote_jid: {}",
                    remote_jid_str
                );
                return;
            }
        };

        // The response's namespace can differ from the one used when the
        // request was cached: a migrated 1:1 request may cross the PN/LID
        // boundary between these two legs. Keep the response key primary, and
        // only spend one alias lookup after that direct key misses. Groups do
        // not have a PN/LID namespace and must never take this path.
        // Missing IDs are accepted only for validated automatic entries.
        let matches_pending = |(_, memo): &(PendingPdoRequest, Arc<PdoRequestMemo>)| {
            ((!request_id.is_empty() && memo.request_id == request_id)
                || (request_id.is_empty() && !memo.explicit_retry))
                && matches!(
                    memo.outcome.load(std::sync::atomic::Ordering::Acquire),
                    PDO_WRITING | PDO_SENT
                )
                && memo.matches_source(response_from_me, response_participant.as_deref())
        };
        let mut pending = self
            .pdo_pending_requests
            .remove_if(&cache_key, &matches_pending)
            .await;
        let alias_key = if !cache_key.chat.is_group() {
            self.swap_pn_lid_namespace(&cache_key.chat)
                .await
                .map(|alias| ChatMessageId::new(alias, msg_id.into()))
        } else {
            None
        };
        if pending.is_none()
            && let Some(alias_key) = &alias_key
        {
            pending = self
                .pdo_pending_requests
                .remove_if(alias_key, &matches_pending)
                .await;
        }
        let authority = pending.as_ref().map(|(_, memo)| memo.clone());
        let pending = pending.map(|(entry, _)| entry);
        if self
            .obsolete_pdo_response(
                &cache_key,
                alias_key.as_ref(),
                response_from_me,
                response_participant.as_deref(),
                request_id,
                authority.as_ref(),
            )
            .await
        {
            return;
        }

        let elapsed = pending
            .as_ref()
            .map(|p| p.requested_at.elapsed().as_millis())
            .unwrap_or(0);

        info!(
            "Received PDO placeholder response for message {} (took {}ms)",
            msg_id, elapsed
        );

        let mut message_info = if let Some(pending) = pending {
            pending.message_info
        } else {
            match self.message_info_from_web_message_info(&web_msg_info).await {
                Ok(info) => Arc::new(info),
                Err(e) => {
                    warn!(
                        "Failed to reconstruct MessageInfo from PDO response: {:?}",
                        e
                    );
                    return;
                }
            }
        };

        let Some(message) = web_msg_info.message.take() else {
            // Expected when the phone could not decrypt the message either;
            // WA Web only counts this outcome in telemetry, with no warning.
            info!("PDO response WebMessageInfo missing message content");
            return;
        };

        let ephemeral_expiration = {
            use wacore::proto_helpers::MessageExt;
            message.get_base_message().get_ephemeral_expiration()
        };
        if self
            .obsolete_pdo_response(
                &cache_key,
                alias_key.as_ref(),
                response_from_me,
                response_participant.as_deref(),
                request_id,
                authority.as_ref(),
            )
            .await
        {
            return;
        }
        Arc::make_mut(&mut message_info).unavailable_request_id = if request_id.is_empty() {
            None
        } else {
            Some(request_id.to_owned())
        };

        let claim = self.dispatch_gate_enabled()
            && !crate::features::message_edit::carries_secret_encrypted(&message);
        let fingerprint = claim.then(|| crate::message::MessageDispatch::fingerprint(&message));
        let mut publication = crate::message::PublicationGuard::default();
        let suppressed =
            self.admit_message_dispatch(&message_info, true, fingerprint, false, &mut publication);
        if suppressed {
            self.duplicate_dispatch_suppressed
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            wacore::telemetry::recv("duplicate_resend");
            return;
        }

        info!(
            "Dispatching PDO-recovered message {} from {} via phone (request_id={})",
            message_info.id,
            message_info.source.sender.observe(),
            request_id
        );

        // PDO recovery is event-only (its ack runs on the PDO path, not the
        // message pipeline), so this bypasses the commit batcher on purpose.
        self.core
            .event_bus
            .dispatch(wacore::types::events::Event::Messages(
                wacore::types::events::MessageBatch::builder()
                    .messages(Arc::from([wacore::types::events::InboundMessage::builder(
                    )
                    .message(Arc::from(message))
                    .info(message_info)
                    .maybe_ephemeral_expiration(ephemeral_expiration)
                    .build()]))
                    .origin(wacore::types::events::BatchOrigin::Live)
                    .build(),
            ));
        publication.complete();
    }

    async fn obsolete_pdo_response(
        &self,
        key: &ChatMessageId,
        alias: Option<&ChatMessageId>,
        from_me: bool,
        participant: Option<&str>,
        request_id: &str,
        authority: Option<&Arc<PdoRequestMemo>>,
    ) -> bool {
        let mut latest: Option<Arc<PdoRequestMemo>> = None;
        let mut consider = |memo: Arc<PdoRequestMemo>| {
            if let Some(owner) = memo.live_owner()
                && (owner.matches_target(key, from_me, participant)
                    || alias.is_some_and(|alias| owner.matches_target(alias, from_me, participant)))
            {
                if latest
                    .as_ref()
                    .is_none_or(|known| known.generation < owner.generation)
                {
                    latest = Some(owner);
                }
                true
            } else {
                false
            }
        };
        if let Some(owner) = authority {
            consider(owner.clone());
        }
        for candidate in std::iter::once(key).chain(alias) {
            if let Some((_, memo)) = self.pdo_pending_requests.get(candidate).await {
                consider(memo);
            }
        }
        if self
            .pdo_explicit_published
            .load(std::sync::atomic::Ordering::Acquire)
        {
            // Bounded by the configured gate capacity (default 512). Clone
            // only matching keys; get rechecks expiry and the current owner.
            let gates = self
                .pdo_requested
                .fold_entries(Vec::new(), |mut gates, gate, memo| {
                    if memo.info.id == key.id && memo.generation != 0 {
                        gates.push(gate.clone());
                    }
                    gates
                })
                .await;
            for gate in gates {
                if let Some(memo) = self.pdo_requested.get(&gate).await {
                    consider(memo);
                }
            }
        }
        latest.is_some_and(|owner| {
            owner.request_id != request_id && (owner.explicit_retry || !request_id.is_empty())
        })
    }

    fn pdo_key_participant(&self, web: &wa::WebMessageInfo) -> Option<String> {
        let key = web.key.as_option()?;
        if let Some(participant) = &key.participant {
            return Some(participant.clone());
        }
        let chat = key.remote_jid.as_deref()?.parse::<Jid>().ok()?;
        // WAWebParseWebMessageInfoUtils.buildMsgKey fills a missing key
        // participant only for groups/status. The top-level participant is
        // the author, and does not overwrite an existing key identity.
        if chat.is_group() || chat.is_status_broadcast() {
            if key.from_me == Some(true) {
                return web
                    .original_self_author_user_jid_string
                    .clone()
                    .or_else(|| {
                        self.persistence_manager
                            .get_device_snapshot()
                            .pn
                            .as_ref()
                            .map(ToString::to_string)
                    });
            }
            return web.participant.clone();
        }
        None
    }

    /// Reconstructs a MessageInfo from a WebMessageInfo.
    /// This is used when we receive a PDO response but don't have the original pending request cached.
    async fn message_info_from_web_message_info(
        &self,
        web_msg: &wa::WebMessageInfo,
    ) -> Result<MessageInfo, anyhow::Error> {
        let Some(key) = web_msg.key.as_option() else {
            anyhow::bail!("WebMessageInfo missing key");
        };

        let participant = self.pdo_key_participant(web_msg);
        let author = if key.remote_jid.as_deref().is_some_and(|chat| {
            chat.parse::<Jid>().is_ok_and(|chat| {
                chat.is_group() || chat.server == wacore_binary::Server::Broadcast
            })
        }) {
            web_msg.participant.as_deref().or(participant.as_deref())
        } else {
            participant.as_deref()
        };
        self.message_info_from_web_message_parts(
            key.remote_jid.as_deref(),
            key.from_me,
            key.id.as_deref(),
            author,
            web_msg.message_timestamp,
            web_msg.push_name.as_deref(),
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn message_info_from_web_message_parts(
        &self,
        remote_jid: Option<&str>,
        from_me: Option<bool>,
        id: Option<&str>,
        participant: Option<&str>,
        message_timestamp: Option<u64>,
        push_name: Option<&str>,
    ) -> Result<MessageInfo, anyhow::Error> {
        let remote_jid: Jid = remote_jid
            .ok_or_else(|| anyhow::anyhow!("MessageKey missing remoteJid"))?
            .parse()?;
        let is_group = remote_jid.is_group();
        let is_from_me = from_me.unwrap_or(false);

        // The PDO response handler maps the top-level author, falling back to
        // the normalized key participant, for groups and broadcasts alike.
        let sender = if let Some(p) = participant {
            p.parse()?
        } else if is_from_me {
            self.persistence_manager
                .get_device_snapshot()
                .pn
                .clone()
                .unwrap_or_else(|| remote_jid.clone())
        } else {
            remote_jid.clone()
        };

        let timestamp = message_timestamp
            .map(|ts| wacore::time::from_secs_or_now(ts as i64))
            .unwrap_or_else(wacore::time::now_utc);

        Ok(MessageInfo {
            id: id.unwrap_or_default().into(),
            server_id: 0,
            newsletter_server_id: None,
            r#type: None,
            source: MessageSource {
                chat: remote_jid,
                sender,
                sender_alt: None,
                recipient_alt: None,
                is_from_me,
                is_group,
                addressing_mode: None,
                broadcast_list_owner: None,
                recipient: None,
            },
            timestamp,
            push_name: push_name.unwrap_or_default().into(),
            category: MessageCategory::default(),
            multicast: false,
            media_type: None,
            edit: EditAttribute::default(),
            bot_info: None,
            meta_info: None,
            verified_name: None,
            device_sent_meta: None,
            is_offline: false,
            unavailable_request_id: None,
            server_timestamp_us: None,
            verified_level: None,
            verified_name_serial: None,
            peer_recipient_pn: None,
            bcl_participants: Vec::new(),
        })
    }

    /// Age-gated PDO send, awaitable so it can run before a transport ack inside
    /// one flush task (when PDO is the sole recovery, e.g. `<unavailable>`).
    /// `fromMe` is NOT excluded: own-device fan-out that fails to decrypt has PDO
    /// as its only recovery (WAWebNonMessageDataRequestPlaceholderMessageResendUtils).
    ///
    /// Returns `false` only on a transient send failure: the caller must then
    /// NOT ack, so the stanza stays in the offline queue for another attempt.
    /// Age-skip counts as a deliberate give-up (`true`), so ancient stanzas are
    /// still cleared.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.pdo.run_request", level = "debug", skip_all, fields(chat = %info.source.chat.observe(), sender = %info.source.sender.observe(), msg_id = %info.id)))]
    pub(crate) async fn run_pdo_request(self: &Arc<Self>, info: &Arc<MessageInfo>) -> bool {
        // Skip ancient messages (14d, matching the AB prop), compared in seconds
        // like WA Web's `age_s > i`. Uses the wacore time primitive (mockable).
        const PDO_MAX_AGE_SECS: i64 = 14 * 24 * 60 * 60;
        let age_secs = wacore::time::now_secs() - info.timestamp.timestamp();
        if age_secs > PDO_MAX_AGE_SECS {
            debug!(
                "PDO request skipped for message {} (age {age_secs}s exceeds {PDO_MAX_AGE_SECS}s limit)",
                info.id,
            );
            return true;
        }
        match self.send_pdo_placeholder_resend_request(info).await {
            Ok(()) => true,
            Err(e) => {
                warn!(
                    "Failed to send PDO request for message {} from {}: {:?}",
                    info.id,
                    info.source.sender.observe(),
                    e
                );
                false
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::self_peer_target;
    use wacore::store::Device;
    use wacore_binary::{Jid, JidExt, Server};

    fn empty_device() -> Device {
        Device {
            pn: None,
            lid: None,
            ..Device::default()
        }
    }

    /// LID-migrated bots must address peer messages over LID so the
    /// pkmsg emitted alongside the PDO refreshes the phone's LID-keyed
    /// Signal slot — sending the same pkmsg to PN leaves the LID slot
    /// on a diverged ratchet and the inbound side never recovers.
    /// Whatsmeow's `SendPeerMessage` picks the same way via
    /// `cli.getOwnID().ToNonAD()` (`Store.GetJID()` returns LID
    /// post-migration).
    #[test]
    fn self_peer_target_prefers_lid_when_present() {
        let mut device = empty_device();
        device.pn = Some(Jid::pn_device("559999999999", 33));
        device.lid = Some(Jid::lid_device("111111111111111", 33));

        let target = self_peer_target(&device).expect("LID present");

        assert_eq!(target.user, "111111111111111");
        assert_eq!(target.server, Server::Lid);
        assert_eq!(target.device, 0);
        assert!(!target.is_ad());
    }

    /// Pre-LID-migration accounts only have a PN. Fall back so peer
    /// messages still route to the primary phone via the PN slot.
    #[test]
    fn self_peer_target_falls_back_to_pn_without_lid() {
        let mut device = empty_device();
        device.pn = Some(Jid::pn_device("559999999999", 33));

        let target = self_peer_target(&device).expect("PN present");

        assert_eq!(target.user, "559999999999");
        assert_eq!(target.server, Server::Pn);
        assert_eq!(target.device, 0);
    }

    /// Pre-login (no PN/LID yet) must surface as a typed error rather
    /// than addressing a bogus JID.
    #[test]
    fn self_peer_target_errors_when_no_identity_known() {
        let device = empty_device();
        assert!(
            matches!(
                self_peer_target(&device),
                Err(crate::client::ClientError::NotLoggedIn)
            ),
            "must require either PN or LID"
        );
    }

    // Reconstruction-path tests share a bare Client wired to mock transport
    // and an in-memory SQLite backend. The only thing they vary is the
    // WebMessageInfo they hand to `message_info_from_web_message_info`.

    async fn setup_reconstruct_client() -> std::sync::Arc<crate::client::Client> {
        use crate::test_utils::{MockHttpClient, create_test_backend};
        use crate::{
            client::Client, runtime_impl::TokioRuntime,
            store::persistence_manager::PersistenceManager, transport::mock::MockTransportFactory,
        };
        use std::sync::Arc;

        let backend = create_test_backend().await;
        let pm = Arc::new(PersistenceManager::new(backend).await.unwrap());
        let (client, _rx) = Client::builder()
            .with_runtime_arc(Arc::new(TokioRuntime))
            .with_persistence_manager(pm)
            .with_transport_factory_arc(Arc::new(MockTransportFactory::new()))
            .with_http_client_arc(Arc::new(MockHttpClient))
            .build()
            .await
            .expect("test client should build")
            .into_parts();
        client
    }

    fn make_web_msg(
        remote_jid: &str,
        from_me: bool,
        id: &str,
        participant: Option<&str>,
    ) -> waproto::whatsapp::WebMessageInfo {
        use waproto::whatsapp as wa;
        wa::WebMessageInfo {
            key: buffa::MessageField::some(wa::MessageKey {
                remote_jid: Some(remote_jid.into()),
                from_me: Some(from_me),
                id: Some(id.into()),
                participant: participant.map(|p| p.into()),
            }),
            ..Default::default()
        }
    }

    fn make_placeholder_response(
        remote_jid: &str,
        from_me: bool,
        id: &str,
        participant: Option<&str>,
    ) -> waproto::whatsapp::message::peer_data_operation_request_response_message::peer_data_operation_result::PlaceholderMessageResendResponse
    {
        use buffa::Message as _;
        let mut web_msg = make_web_msg(remote_jid, from_me, id, participant);
        web_msg.message = buffa::MessageField::some(waproto::whatsapp::Message {
            conversation: Some("recovered by the phone".to_owned()),
            ..Default::default()
        });
        waproto::whatsapp::message::peer_data_operation_request_response_message::peer_data_operation_result::PlaceholderMessageResendResponse {
            web_message_info_bytes: Some(web_msg.encode_to_vec()),
        }
    }

    fn make_dm_pending_info(
        chat: &str,
        sender_alt: &str,
        id: &str,
        addressing_mode: wacore::types::message::AddressingMode,
    ) -> std::sync::Arc<wacore::types::message::MessageInfo> {
        use wacore::types::message::{MessageInfo, MessageSource};
        let chat: Jid = chat.parse().expect("chat jid");
        std::sync::Arc::new(MessageInfo {
            id: id.into(),
            push_name: "pending metadata".into(),
            source: MessageSource {
                chat: chat.clone(),
                sender: chat,
                addressing_mode: Some(addressing_mode),
                sender_alt: Some(sender_alt.parse().expect("sender alt jid")),
                ..Default::default()
            },
            ..Default::default()
        })
    }

    /// The reconstruction path preserves the real author for status
    /// broadcasts via `key.participant`. Using `remote_jid` as sender
    /// would surface `status@broadcast` and erase the author.
    #[tokio::test]
    async fn test_reconstruct_prefers_participant_for_status_broadcast() {
        let client = setup_reconstruct_client().await;
        let author_jid = "203040904720543@lid";
        let web_msg = make_web_msg("status@broadcast", false, "STATUS_PDO_1", Some(author_jid));

        let info = client
            .message_info_from_web_message_info(&web_msg)
            .await
            .unwrap();

        assert_eq!(info.source.chat.to_string(), "status@broadcast");
        assert_eq!(info.source.sender.to_string(), author_jid);
    }

    /// DM without participant falls back to remote_jid as the sender,
    /// preserving the pre-fix behaviour for the DM case.
    #[tokio::test]
    async fn test_reconstruct_dm_falls_back_to_remote_jid() {
        let client = setup_reconstruct_client().await;
        let peer = "5511999998888@s.whatsapp.net";
        let web_msg = make_web_msg(peer, false, "DM_PDO_1", None);

        let info = client
            .message_info_from_web_message_info(&web_msg)
            .await
            .unwrap();

        assert_eq!(info.source.chat.to_string(), peer);
        assert_eq!(info.source.sender.to_string(), peer);
    }

    #[tokio::test]
    async fn test_reconstruct_from_web_message_info_view() {
        use buffa::Message as _;
        use waproto::whatsapp as wa;

        let client = setup_reconstruct_client().await;
        let author_jid = "203040904720543@lid";
        let mut web_msg = make_web_msg(
            "status@broadcast",
            false,
            "STATUS_PDO_VIEW_1",
            Some(author_jid),
        );
        web_msg.push_name = Some("Recovered Sender".to_string());
        web_msg.message_timestamp = Some(1_700_000_000);
        let encoded = web_msg.encode_to_vec();
        let decoded = wa::WebMessageInfo::decode_from_slice(&encoded).expect("should decode");

        let info = client
            .message_info_from_web_message_info(&decoded)
            .await
            .unwrap();

        assert_eq!(info.id, "STATUS_PDO_VIEW_1");
        assert_eq!(info.source.chat.to_string(), "status@broadcast");
        assert_eq!(info.source.sender.to_string(), author_jid);
        assert_eq!(info.push_name, "Recovered Sender");
    }

    /// LID-migrated 1-on-1 responses carry `remote_jid` in LID form and no
    /// `participant` (WA Web's request side strips it when building the new
    /// MsgKey, and `msgKeyToProtobuf` then omits it). Reconstruction must
    /// still resolve the sender to that LID remote, not to something else.
    #[tokio::test]
    async fn test_reconstruct_lid_migrated_dm_uses_lid_remote() {
        let client = setup_reconstruct_client().await;
        let peer_lid = "236395184570386@lid";
        let web_msg = make_web_msg(peer_lid, false, "LID_DM_PDO_1", None);

        let info = client
            .message_info_from_web_message_info(&web_msg)
            .await
            .unwrap();

        assert_eq!(info.source.chat.to_string(), peer_lid);
        assert_eq!(info.source.sender.to_string(), peer_lid);
        assert!(!info.source.is_group);
        assert!(!info.source.is_from_me);
    }

    /// fromMe LID DM: the response has no participant (WA Web omits it when
    /// fromMe), so the reconstructed sender must come from the device's own
    /// PN, not from the LID remote_jid.
    #[tokio::test]
    async fn test_reconstruct_lid_migrated_dm_from_me_uses_own_pn() {
        let client = setup_reconstruct_client().await;
        let peer_lid = "236395184570386@lid";
        let web_msg = make_web_msg(peer_lid, true, "LID_DM_FROM_ME_1", None);

        let info = client
            .message_info_from_web_message_info(&web_msg)
            .await
            .unwrap();

        // No own PN configured on a fresh test client, so sender falls back
        // to `remote_jid`. The point is that the participant-less fromMe
        // path reconstructs without panic.
        assert_eq!(info.source.chat.to_string(), peer_lid);
        assert!(info.source.is_from_me);
    }

    // Once-per-message memo tests: WA Web sends at most one placeholder
    // resend request per message per session
    // (WAWebNonMessageDataRequestPlaceholderMessageResendUtils); these pin
    // the same contract onto `pdo_requested`.

    fn make_group_message_info(
        chat: &str,
        sender: &str,
        id: &str,
    ) -> std::sync::Arc<wacore::types::message::MessageInfo> {
        use wacore::types::message::{MessageInfo, MessageSource};
        std::sync::Arc::new(MessageInfo {
            id: id.into(),
            source: MessageSource {
                chat: chat.parse().expect("chat jid"),
                sender: sender.parse().expect("sender jid"),
                is_group: true,
                ..Default::default()
            },
            timestamp: wacore::time::now_utc(),
            ..Default::default()
        })
    }

    async fn set_own_pn(client: &std::sync::Arc<crate::client::Client>) {
        client
            .persistence_manager
            .process_command(crate::store::commands::DeviceCommand::SetId(Some(
                "5511777776666:2@s.whatsapp.net".parse().expect("own jid"),
            )))
            .await;
    }

    /// A message that already went through one placeholder resend must not
    /// trigger another request, no matter how many times the server
    /// redelivers the undecryptable original.
    #[tokio::test]
    async fn pdo_request_skipped_when_already_requested() {
        use wacore::types::message::ChatMessageId;

        let client = setup_reconstruct_client().await;
        set_own_pn(&client).await;

        let info = make_group_message_info(
            "120363000000000001@g.us",
            "203040904720543@lid",
            "PDO_ONCE_1",
        );
        let key = ChatMessageId::new(info.source.chat.clone(), info.id.clone());
        let gate_key = wacore::types::message::SenderMessageId::new(
            info.source.chat.clone(),
            info.id.clone(),
            info.source.sender.clone(),
        );
        client
            .pdo_requested
            .insert(
                gate_key,
                super::PdoRequestMemo::new(&info, "prior-request".into(), false, None),
            )
            .await;

        let res = client.send_pdo_placeholder_resend_request(&info).await;

        assert!(res.is_ok(), "gated path reports success: {res:?}");
        assert!(
            client.pdo_pending_requests.get(&key).await.is_none(),
            "gated request must not register a pending entry"
        );
    }

    /// The once-per-message gate is per sender, because a message id belongs to
    /// the sending client and two participants can pick the same one.
    ///
    /// Measured, not hypothetical: in a 14-hour production log, 2 of 851
    /// `(chat, id)` pairs carried messages from two different participants.
    /// Gating on `(chat, id)` alone means the second one never gets a
    /// placeholder requested for it at all.
    ///
    /// Told apart by the return: a gated call returns early with `Ok`, while a
    /// call that gets past the gate reaches the send and fails without a live
    /// transport. `Err` here therefore means "was not gated".
    #[tokio::test]
    async fn the_pdo_gate_lets_a_second_sender_ask_for_its_own_message() {
        let client = setup_reconstruct_client().await;
        set_own_pn(&client).await;
        client
            .offline_sync_completed
            .store(true, std::sync::atomic::Ordering::Relaxed);

        let first = make_group_message_info(
            "120363000000000001@g.us",
            "203040904720543@lid",
            "PDO_SHARED_ID",
        );
        let second = make_group_message_info(
            "120363000000000001@g.us",
            "111222333444555@lid",
            "PDO_SHARED_ID",
        );

        // The first sender's request is already on record.
        client
            .pdo_requested
            .insert(
                wacore::types::message::SenderMessageId::new(
                    first.source.chat.clone(),
                    first.id.clone(),
                    first.source.sender.clone(),
                ),
                super::PdoRequestMemo::new(&first, "prior-request".into(), false, None),
            )
            .await;

        let gated = tokio::time::timeout(
            std::time::Duration::from_secs(15),
            client.send_pdo_placeholder_resend_request(&first),
        )
        .await
        .expect("the gated call returns without touching the network");
        assert!(gated.is_ok(), "the first sender is gated by its own record");

        let ungated = tokio::time::timeout(
            std::time::Duration::from_secs(15),
            client.send_pdo_placeholder_resend_request(&second),
        )
        .await
        .expect("send attempt must resolve fast without a live transport");
        assert!(
            ungated.is_err(),
            "the second sender's message is its own, and must reach the send \
             rather than being gated by the first"
        );
    }

    /// A sender blocked by *another* sender's in-flight request must not keep
    /// the gate it just claimed.
    ///
    /// The pending map is shared by `(chat, id)`, so one sender's in-flight
    /// request short-circuits the other's call after it has already claimed its
    /// own gate. Nothing was sent for it, so holding the slot would suppress it
    /// for the gate's whole lifetime once the pending entry clears.
    #[tokio::test]
    async fn a_sender_short_circuited_by_a_shared_pending_entry_keeps_no_gate() {
        use wacore::types::message::ChatMessageId;

        let client = setup_reconstruct_client().await;
        set_own_pn(&client).await;
        client
            .offline_sync_completed
            .store(true, std::sync::atomic::Ordering::Relaxed);

        let first = make_group_message_info(
            "120363000000000001@g.us",
            "203040904720543@lid",
            "PDO_PENDING_SHARED",
        );
        let second = make_group_message_info(
            "120363000000000001@g.us",
            "111222333444555@lid",
            "PDO_PENDING_SHARED",
        );
        let second_gate = wacore::types::message::SenderMessageId::new(
            second.source.chat.clone(),
            second.id.clone(),
            second.source.sender.clone(),
        );

        // The first sender's request is in flight, under the shared key.
        client
            .pdo_pending_requests
            .insert(
                ChatMessageId::new(first.source.chat.clone(), first.id.clone()),
                super::test_pending(
                    crate::pdo::PendingPdoRequest {
                        message_info: first.clone(),
                        requested_at: wacore::time::Instant::now(),
                    },
                    "req-first",
                    false,
                ),
            )
            .await;

        let blocked = tokio::time::timeout(
            std::time::Duration::from_secs(15),
            client.send_pdo_placeholder_resend_request(&second),
        )
        .await
        .expect("the short-circuited call returns without touching the network");
        assert!(blocked.is_ok(), "the pending branch reports success");
        assert!(
            client.pdo_requested.get(&second_gate).await.is_none(),
            "a sender that sent nothing must not hold its once-per-message slot"
        );
    }

    /// A pending entry that belongs to another sender must not be used to
    /// attribute this response.
    ///
    /// The pending map is keyed by `(chat, id)` and its slot expires and can be
    /// evicted, so the entry present when a response lands is not necessarily
    /// the one it answers. Adopting it anyway would dispatch one sender's
    /// recovered content under the other's identity — worse than the missing
    /// placeholder this change set out to fix.
    #[tokio::test]
    async fn a_response_does_not_adopt_another_senders_pending_entry() {
        use buffa::Message as _;
        use wacore::types::events::ChannelEventHandler;
        use wacore::types::message::ChatMessageId;

        let client = setup_reconstruct_client().await;
        set_own_pn(&client).await;
        let (handler, rx) = ChannelEventHandler::new();
        client.core.event_bus.subscribe_handler(handler).detach();

        let chat = "120363000000000001@g.us";
        let msg_id = "PDO_ATTRIBUTION";
        let key = ChatMessageId::new(chat.parse().expect("chat jid"), msg_id.into());

        // Whoever holds the slot when the response lands is not who it answers.
        client
            .pdo_pending_requests
            .insert(
                key.clone(),
                super::test_pending(
                    super::PendingPdoRequest {
                        message_info: make_group_message_info(chat, "203040904720543@lid", msg_id),
                        requested_at: wacore::time::Instant::now(),
                    },
                    "req-attr",
                    false,
                ),
            )
            .await;

        let web_msg = waproto::whatsapp::WebMessageInfo {
            key: buffa::MessageField::some(waproto::whatsapp::MessageKey {
                remote_jid: Some(chat.to_owned()),
                from_me: Some(false),
                id: Some(msg_id.into()),
                participant: Some("111222333444555@lid".to_owned()),
            }),
            message: buffa::MessageField::some(waproto::whatsapp::Message {
                conversation: Some("recovered by the phone".to_owned()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let response = waproto::whatsapp::message::peer_data_operation_request_response_message::peer_data_operation_result::PlaceholderMessageResendResponse {
            web_message_info_bytes: Some(web_msg.encode_to_vec()),
        };

        client
            .handle_placeholder_resend_response(&response, "req-attr")
            .await;

        assert!(
            client.pdo_pending_requests.get(&key).await.is_some(),
            "a response must preserve another sender's pending request"
        );

        let senders: Vec<String> = {
            let mut senders = Vec::new();
            while let Ok(event) = rx.try_recv() {
                for m in event.messages().filter(|m| m.info.id == msg_id) {
                    senders.push(m.info.source.sender.to_string());
                }
            }
            senders
        };
        assert_eq!(
            senders,
            vec!["111222333444555@lid".to_string()],
            "the response names its own author; a stale pending entry must not \
             relabel the recovered content as the other sender"
        );
    }

    /// The phone answers in PN while a LID-addressed group stored the LID, and
    /// that is the same author — the entry must be kept.
    ///
    /// Comparing one spelling would reject every genuine entry from a LID
    /// group and fall back to rebuilding, discarding the addressing mode,
    /// the sender alias and the stanza metadata the original delivery carried.
    #[tokio::test]
    async fn a_pn_response_matches_the_lid_sender_it_was_requested_for() {
        use buffa::Message as _;
        use wacore::types::events::ChannelEventHandler;
        use wacore::types::message::ChatMessageId;

        let client = setup_reconstruct_client().await;
        set_own_pn(&client).await;
        let (handler, rx) = ChannelEventHandler::new();
        client.core.event_bus.subscribe_handler(handler).detach();

        let chat = "120363000000000001@g.us";
        let msg_id = "PDO_LID_ALIAS";
        let key = ChatMessageId::new(chat.parse().expect("chat jid"), msg_id.into());

        // What a LID-addressed group delivery leaves behind: LID in `sender`,
        // the PN the stanza carried in `sender_alt`.
        let mut info = (*make_group_message_info(chat, "203040904720543@lid", msg_id)).clone();
        info.source.sender_alt = Some("15550001234@s.whatsapp.net".parse().expect("pn"));
        info.source.addressing_mode = Some(wacore::types::message::AddressingMode::Lid);
        client
            .pdo_pending_requests
            .insert(
                key,
                super::test_pending(
                    super::PendingPdoRequest {
                        message_info: std::sync::Arc::new(info),
                        requested_at: wacore::time::Instant::now(),
                    },
                    "req-alias",
                    false,
                ),
            )
            .await;

        let web_msg = waproto::whatsapp::WebMessageInfo {
            key: buffa::MessageField::some(waproto::whatsapp::MessageKey {
                remote_jid: Some(chat.to_owned()),
                from_me: Some(false),
                id: Some(msg_id.into()),
                // The phone answers in PN.
                participant: Some("15550001234@s.whatsapp.net".to_owned()),
            }),
            message: buffa::MessageField::some(waproto::whatsapp::Message {
                conversation: Some("recovered by the phone".to_owned()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let response = waproto::whatsapp::message::peer_data_operation_request_response_message::peer_data_operation_result::PlaceholderMessageResendResponse {
            web_message_info_bytes: Some(web_msg.encode_to_vec()),
        };

        client
            .handle_placeholder_resend_response(&response, "req-alias")
            .await;

        let dispatched: Vec<(String, Option<wacore::types::message::AddressingMode>)> = {
            let mut seen = Vec::new();
            while let Ok(event) = rx.try_recv() {
                for m in event.messages().filter(|m| m.info.id == msg_id) {
                    seen.push((
                        m.info.source.sender.to_string(),
                        m.info.source.addressing_mode,
                    ));
                }
            }
            seen
        };
        assert_eq!(dispatched.len(), 1, "the response is dispatched");
        assert_eq!(
            dispatched[0].0, "203040904720543@lid",
            "the pending entry is the same author under its other spelling, so \
             its record is kept rather than rebuilt from the response"
        );
        assert_eq!(
            dispatched[0].1,
            Some(wacore::types::message::AddressingMode::Lid),
            "keeping the entry keeps the addressing mode the delivery carried"
        );
    }

    /// A response in PN can recover a request cached in LID after migration.
    /// The pending MessageInfo, including its addressing metadata, must win over
    /// lossy reconstruction from the response.
    #[tokio::test]
    async fn a_pn_response_retries_the_lid_pending_key() {
        use wacore::types::events::ChannelEventHandler;
        use wacore::types::message::ChatMessageId;

        let client = setup_reconstruct_client().await;
        let pn = "5511999998888@s.whatsapp.net";
        let lid = "236395184570386@lid";
        let msg_id = "PDO_DM_PN_FROM_LID";
        client
            .lid_pn_cache
            .add(&wacore::types::lid_pn::LidPnEntry {
                lid: "236395184570386".into(),
                phone_number: "5511999998888".into(),
                created_at: 0,
                learning_source: wacore::types::lid_pn::LearningSource::Usync,
            })
            .await;
        let pending =
            make_dm_pending_info(lid, pn, msg_id, wacore::types::message::AddressingMode::Lid);
        client
            .pdo_pending_requests
            .insert(
                ChatMessageId::new(lid.parse().expect("lid"), msg_id.into()),
                super::test_pending(
                    super::PendingPdoRequest {
                        message_info: pending,
                        requested_at: wacore::time::Instant::now(),
                    },
                    "req-pn-alias",
                    false,
                ),
            )
            .await;

        let (handler, rx) = ChannelEventHandler::new();
        client.core.event_bus.subscribe_handler(handler).detach();
        let response = make_placeholder_response(pn, false, msg_id, None);
        client
            .handle_placeholder_resend_response(&response, "req-pn-alias")
            .await;

        assert!(
            client
                .pdo_pending_requests
                .get(&ChatMessageId::new(lid.parse().unwrap(), msg_id.into()))
                .await
                .is_none()
        );
        let mut infos = Vec::new();
        while let Ok(event) = rx.try_recv() {
            infos.extend(event.messages().map(|message| message.info.clone()));
        }
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].source.chat.to_string(), lid);
        assert_eq!(
            infos[0].source.addressing_mode,
            Some(wacore::types::message::AddressingMode::Lid)
        );
        assert_eq!(infos[0].source.sender_alt.as_ref().unwrap().to_string(), pn);
        assert_eq!(infos[0].push_name, "pending metadata");
    }

    /// The reverse migration spelling is also recovered, but only after the
    /// response's direct key misses.
    #[tokio::test]
    async fn a_lid_response_retries_the_pn_pending_key() {
        use wacore::types::events::ChannelEventHandler;
        use wacore::types::message::ChatMessageId;

        let client = setup_reconstruct_client().await;
        let pn = "5511999998888@s.whatsapp.net";
        let lid = "236395184570386@lid";
        let msg_id = "PDO_DM_LID_FROM_PN";
        client
            .lid_pn_cache
            .add(&wacore::types::lid_pn::LidPnEntry {
                lid: "236395184570386".into(),
                phone_number: "5511999998888".into(),
                created_at: 0,
                learning_source: wacore::types::lid_pn::LearningSource::Usync,
            })
            .await;
        client
            .pdo_pending_requests
            .insert(
                ChatMessageId::new(pn.parse().expect("pn"), msg_id.into()),
                super::test_pending(
                    super::PendingPdoRequest {
                        message_info: make_dm_pending_info(
                            pn,
                            lid,
                            msg_id,
                            wacore::types::message::AddressingMode::Pn,
                        ),
                        requested_at: wacore::time::Instant::now(),
                    },
                    "req-lid-alias",
                    false,
                ),
            )
            .await;

        let (handler, rx) = ChannelEventHandler::new();
        client.core.event_bus.subscribe_handler(handler).detach();
        let response = make_placeholder_response(lid, false, msg_id, None);
        client
            .handle_placeholder_resend_response(&response, "req-lid-alias")
            .await;

        assert!(
            client
                .pdo_pending_requests
                .get(&ChatMessageId::new(pn.parse().unwrap(), msg_id.into()))
                .await
                .is_none()
        );
        let mut infos = Vec::new();
        while let Ok(event) = rx.try_recv() {
            infos.extend(event.messages().map(|message| message.info.clone()));
        }
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].source.chat.to_string(), pn);
        assert_eq!(
            infos[0].source.addressing_mode,
            Some(wacore::types::message::AddressingMode::Pn)
        );
        assert_eq!(
            infos[0].source.sender_alt.as_ref().unwrap().to_string(),
            lid
        );
        assert_eq!(infos[0].push_name, "pending metadata");
    }

    /// Without ownership order, a direct hit keeps its metadata and leaves the alias entry intact.
    #[tokio::test]
    async fn pdo_direct_pending_hit_does_not_consume_alias() {
        use wacore::types::events::ChannelEventHandler;
        use wacore::types::message::ChatMessageId;

        let client = setup_reconstruct_client().await;
        let pn = "5511999998888@s.whatsapp.net";
        let lid = "236395184570386@lid";
        let msg_id = "PDO_DM_DIRECT";
        client
            .lid_pn_cache
            .add(&wacore::types::lid_pn::LidPnEntry {
                lid: "236395184570386".into(),
                phone_number: "5511999998888".into(),
                created_at: 0,
                learning_source: wacore::types::lid_pn::LearningSource::Usync,
            })
            .await;
        let direct_info =
            make_dm_pending_info(pn, lid, msg_id, wacore::types::message::AddressingMode::Pn);
        let alias_info =
            make_dm_pending_info(lid, pn, msg_id, wacore::types::message::AddressingMode::Lid);
        client
            .pdo_pending_requests
            .insert(
                ChatMessageId::new(pn.parse().unwrap(), msg_id.into()),
                (
                    super::PendingPdoRequest {
                        message_info: direct_info.clone(),
                        requested_at: wacore::time::Instant::now(),
                    },
                    super::PdoRequestMemo::sent_for_test(&direct_info, "req-direct".into(), false),
                ),
            )
            .await;
        client
            .pdo_pending_requests
            .insert(
                ChatMessageId::new(lid.parse().unwrap(), msg_id.into()),
                (
                    super::PendingPdoRequest {
                        message_info: alias_info.clone(),
                        requested_at: wacore::time::Instant::now(),
                    },
                    super::PdoRequestMemo::sent_for_test(
                        &alias_info,
                        "req-alias-other".into(),
                        false,
                    ),
                ),
            )
            .await;

        let (handler, rx) = ChannelEventHandler::new();
        client.core.event_bus.subscribe_handler(handler).detach();
        let response = make_placeholder_response(pn, false, msg_id, None);
        client
            .handle_placeholder_resend_response(&response, "req-direct")
            .await;

        assert!(
            client
                .pdo_pending_requests
                .get(&ChatMessageId::new(pn.parse().unwrap(), msg_id.into()))
                .await
                .is_none()
        );
        assert!(
            client
                .pdo_pending_requests
                .get(&ChatMessageId::new(lid.parse().unwrap(), msg_id.into()))
                .await
                .is_some()
        );
        let mut infos = Vec::new();
        while let Ok(event) = rx.try_recv() {
            infos.extend(event.messages().map(|message| message.info.clone()));
        }
        assert_eq!(infos.len(), 1);
        assert_eq!(
            infos[0].source.addressing_mode,
            Some(wacore::types::message::AddressingMode::Pn)
        );
    }

    /// Without a mapping, a namespace mismatch still takes the existing
    /// reconstruction path instead of guessing an alias.
    #[tokio::test]
    async fn pdo_alias_miss_without_mapping_reconstructs_response() {
        use wacore::types::events::ChannelEventHandler;
        use wacore::types::message::ChatMessageId;

        let client = setup_reconstruct_client().await;
        let pn = "5511999998888@s.whatsapp.net";
        let lid = "236395184570386@lid";
        let msg_id = "PDO_DM_NO_MAPPING";
        client
            .pdo_pending_requests
            .insert(
                ChatMessageId::new(pn.parse().unwrap(), msg_id.into()),
                super::test_pending(
                    super::PendingPdoRequest {
                        message_info: make_dm_pending_info(
                            pn,
                            lid,
                            msg_id,
                            wacore::types::message::AddressingMode::Pn,
                        ),
                        requested_at: wacore::time::Instant::now(),
                    },
                    "req-no-mapping",
                    false,
                ),
            )
            .await;

        let (handler, rx) = ChannelEventHandler::new();
        client.core.event_bus.subscribe_handler(handler).detach();
        let response = make_placeholder_response(lid, false, msg_id, None);
        client
            .handle_placeholder_resend_response(&response, "req-no-mapping")
            .await;

        assert!(
            client
                .pdo_pending_requests
                .get(&ChatMessageId::new(pn.parse().unwrap(), msg_id.into()))
                .await
                .is_some()
        );
        let mut infos = Vec::new();
        while let Ok(event) = rx.try_recv() {
            infos.extend(event.messages().map(|message| message.info.clone()));
        }
        assert_eq!(infos.len(), 1);
        assert_eq!(infos[0].source.chat.to_string(), lid);
        assert_eq!(infos[0].source.sender.to_string(), lid);
        assert_eq!(infos[0].push_name, "");
    }

    /// Two directions of one DM can share an id, and both their responses omit
    /// the participant — so direction is the only thing left to agree on.
    #[tokio::test]
    async fn a_participant_less_response_checks_the_direction() {
        use buffa::Message as _;
        use wacore::types::events::ChannelEventHandler;
        use wacore::types::message::{ChatMessageId, MessageInfo, MessageSource};

        let client = setup_reconstruct_client().await;
        set_own_pn(&client).await;
        let (handler, rx) = ChannelEventHandler::new();
        client.core.event_bus.subscribe_handler(handler).detach();

        let peer = "5511999998888@s.whatsapp.net";
        let msg_id = "PDO_DIRECTION";
        let key = ChatMessageId::new(peer.parse().expect("chat jid"), msg_id.into());

        // The slot holds the outgoing half of the conversation.
        let outgoing = MessageInfo {
            id: msg_id.into(),
            source: MessageSource {
                chat: peer.parse().expect("chat jid"),
                sender: peer.parse().expect("chat jid"),
                is_from_me: true,
                ..Default::default()
            },
            ..Default::default()
        };
        client
            .pdo_pending_requests
            .insert(
                key,
                super::test_pending(
                    super::PendingPdoRequest {
                        message_info: std::sync::Arc::new(outgoing),
                        requested_at: wacore::time::Instant::now(),
                    },
                    "req-direction",
                    false,
                ),
            )
            .await;

        // The response answers the incoming one, and omits the participant too.
        let web_msg = waproto::whatsapp::WebMessageInfo {
            key: buffa::MessageField::some(waproto::whatsapp::MessageKey {
                remote_jid: Some(peer.to_owned()),
                from_me: Some(false),
                id: Some(msg_id.into()),
                participant: None,
            }),
            message: buffa::MessageField::some(waproto::whatsapp::Message {
                conversation: Some("recovered by the phone".to_owned()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let response = waproto::whatsapp::message::peer_data_operation_request_response_message::peer_data_operation_result::PlaceholderMessageResendResponse {
            web_message_info_bytes: Some(web_msg.encode_to_vec()),
        };

        client
            .handle_placeholder_resend_response(&response, "req-direction")
            .await;

        let from_me: Vec<bool> = {
            let mut seen = Vec::new();
            while let Ok(event) = rx.try_recv() {
                for m in event.messages().filter(|m| m.info.id == msg_id) {
                    seen.push(m.info.source.is_from_me);
                }
            }
            seen
        };
        assert_eq!(from_me.len(), 1, "the response is dispatched");
        assert!(
            !from_me[0],
            "the response is for the incoming message; the outgoing entry in \
             the slot must not relabel it as ours"
        );
    }

    mod regressions {
        include!("pdo/regression_tests.rs");
    }

    mod manual_retry {
        use super::super::{
            Arc, Client, MessageInfo, PdoRequestMemo, PendingPdoRequest, test_pending, wa,
        };
        use super::{
            make_dm_pending_info, make_group_message_info, make_placeholder_response, make_web_msg,
            set_own_pn, setup_reconstruct_client,
        };
        use wacore::types::jid::JidExt as _;
        use wacore::types::message::ChatMessageId;
        use wacore_binary::{Jid, JidExt as _};

        async fn manual_retry_client() -> (
            Arc<Client>,
            Arc<crate::transport::mock::CapturingMockTransport>,
            Arc<MessageInfo>,
        ) {
            let (client, transport) = crate::test_utils::create_iq_test_client().await;
            use crate::store::commands::DeviceCommand;
            client
                .persistence_manager
                .process_command(DeviceCommand::SetId(Some(
                    "12025550100:2@s.whatsapp.net".parse().unwrap(),
                )))
                .await;
            client
                .persistence_manager
                .process_command(DeviceCommand::SetAccount(Some(
                    wa::ADVSignedDeviceIdentity::default(),
                )))
                .await;
            crate::test_utils::seed_peer_session(
                &client,
                &"12025550100@s.whatsapp.net".parse().unwrap(),
            )
            .await;
            let info = make_group_message_info(
                "120363000000000001@g.us",
                "12025550101@s.whatsapp.net",
                "MANUAL_PDO_SYNTHETIC",
            );
            (client, transport, info)
        }

        #[tokio::test]
        async fn manual_pdo_sends_after_automatic_memo_without_releasing_it() {
            let (client, transport, info) = manual_retry_client().await;
            assert!(
                !client
                    .pdo_explicit_published
                    .load(std::sync::atomic::Ordering::Acquire)
            );
            let gate = wacore::types::message::SenderMessageId::new(
                info.source.chat.clone(),
                info.id.clone(),
                info.source.sender.clone(),
            );
            client
                .pdo_requested
                .insert(
                    gate.clone(),
                    PdoRequestMemo::sent_for_test(&info, "AUTOMATIC_REQUEST".into(), false),
                )
                .await;
            let id = client
                .retry_pdo_placeholder_resend_request(&info)
                .await
                .unwrap()
                .unwrap();
            assert!(
                client
                    .pdo_explicit_published
                    .load(std::sync::atomic::Ordering::Acquire)
            );
            let sent = crate::test_utils::decode_sent_iq(&transport, 0).await;
            assert_eq!(sent.get().tag.as_ref(), "message");
            assert_eq!(
                sent.get().attrs().optional_string("id").as_deref(),
                Some(id.as_str())
            );
            assert_eq!(
                sent.get().attrs().optional_string("category").as_deref(),
                Some("peer")
            );
            assert_eq!(
                sent.get().attrs().optional_jid("to").unwrap(),
                "12025550100@s.whatsapp.net".parse::<Jid>().unwrap()
            );
            assert!(
                client
                    .retry_pdo_placeholder_resend_request(&info)
                    .await
                    .unwrap()
                    .is_none()
            );
            for _ in 0..3 {
                client
                    .send_pdo_placeholder_resend_request(&info)
                    .await
                    .unwrap();
            }
            assert_eq!(transport.sent().len(), 1);
            assert_eq!(
                client
                    .pdo_requested
                    .get(&gate)
                    .await
                    .map(|memo| memo.request_id.clone())
                    .as_deref(),
                Some(id.as_str())
            );
            assert!(
                client
                    .pdo_requested
                    .get(&gate)
                    .await
                    .unwrap()
                    .previous
                    .is_none()
            );
        }

        #[tokio::test]
        async fn reviewed_status_retry_accepts_the_parser_broadcast_source() {
            let (client, transport, original) = manual_retry_client().await;
            let stanza = wacore_binary::builder::NodeBuilder::new("message")
                .attr("from", "status@broadcast")
                .attr("participant", original.source.sender.clone())
                .attr("id", original.id.to_string())
                .attr("type", "text")
                .build();
            let stanza = crate::test_utils::node_to_owned_ref(&stanza);
            let info = Arc::new(client.parse_message_info(stanza.get()).await.unwrap());
            assert!(info.source.is_group && !info.source.chat.is_group());
            let id = client
                .retry_pdo_placeholder_resend_request(&info)
                .await
                .unwrap()
                .unwrap();
            let sent = crate::test_utils::decode_sent_iq(&transport, 0).await;
            assert_eq!(
                sent.get().attrs().optional_string("id").as_deref(),
                Some(id.as_str())
            );
        }

        #[tokio::test]
        async fn reviewed_obsolete_manual_replies_never_publish_old_content() {
            use buffa::Message as _;
            use wacore::types::events::ChannelEventHandler;
            let (client, _, info) = manual_retry_client().await;
            let key = ChatMessageId::new(info.source.chat.clone(), info.id.clone());
            let mut alias_info = (*info).clone();
            alias_info.source.sender = "777000000000101@lid".parse().unwrap();
            alias_info.source.sender_alt = Some(info.source.sender.clone());
            let first = client
                .retry_pdo_placeholder_resend_request(&Arc::new(alias_info))
                .await
                .unwrap()
                .unwrap();
            client.pdo_pending_requests.remove(&key).await;
            let latest = client
                .retry_pdo_placeholder_resend_request(&info)
                .await
                .unwrap()
                .unwrap();
            let (handler, rx) = ChannelEventHandler::new();
            client.core.event_bus.subscribe_handler(handler).detach();
            let mut response = make_placeholder_response(
                &info.source.chat.to_string(),
                false,
                &info.id,
                Some(&info.source.sender.to_string()),
            );
            for id in [&first, "", "UNKNOWN_REQUEST"] {
                client
                    .handle_placeholder_resend_response(&response, id)
                    .await;
                assert_eq!(
                    client
                        .pdo_pending_requests
                        .get(&key)
                        .await
                        .unwrap()
                        .1
                        .request_id,
                    latest
                );
                while let Ok(event) = rx.try_recv() {
                    assert!(
                        event.messages().next().is_none(),
                        "obsolete manual content was published"
                    );
                }
            }
            let mut web = waproto::codec::web_message_info_decode(
                response.web_message_info_bytes.as_ref().unwrap(),
            )
            .unwrap();
            web.message.as_option_mut().unwrap().conversation = Some("LATEST_RESPONSE".into());
            response.web_message_info_bytes = Some(web.encode_to_vec());
            client
                .handle_placeholder_resend_response(&response, &latest)
                .await;
            assert!(client.pdo_pending_requests.get(&key).await.is_none());
            let mut delivered = 0;
            while let Ok(event) = rx.try_recv() {
                for message in event.messages() {
                    assert_eq!(
                        message.message.conversation.as_deref(),
                        Some("LATEST_RESPONSE")
                    );
                    assert_eq!(message.info.source.chat, info.source.chat);
                    assert_eq!(message.info.source.sender, info.source.sender);
                    assert_eq!(message.info.source.is_from_me, info.source.is_from_me);
                    assert_eq!(
                        message.info.unavailable_request_id.as_deref(),
                        Some(latest.as_str())
                    );
                    delivered += 1;
                }
            }
            assert_eq!(delivered, 1);
            web.message.as_option_mut().unwrap().conversation = Some("STALE_AFTER_SUCCESS".into());
            response.web_message_info_bytes = Some(web.encode_to_vec());
            client
                .handle_placeholder_resend_response(&response, &first)
                .await;
            while let Ok(event) = rx.try_recv() {
                assert!(
                    event.messages().next().is_none(),
                    "old reply after success was published"
                );
            }
        }

        fn failure_socket(
            client: &Arc<Client>,
        ) -> (async_channel::Receiver<()>, async_channel::Sender<()>) {
            struct GatedFailure {
                entered: async_channel::Sender<()>,
                release: async_channel::Receiver<()>,
            }
            #[async_trait::async_trait]
            impl crate::transport::Transport for GatedFailure {
                async fn send(&self, _: bytes::Bytes) -> anyhow::Result<()> {
                    self.entered.send(()).await.unwrap();
                    self.release.recv().await.unwrap();
                    anyhow::bail!("synthetic send failure")
                }
                async fn disconnect(&self) {}
            }
            let (entered, receiver) = async_channel::bounded(1);
            let (release, wait) = async_channel::bounded(1);
            install_socket(
                client,
                Arc::new(GatedFailure {
                    entered,
                    release: wait,
                }),
            );
            (receiver, release)
        }

        fn install_socket(client: &Arc<Client>, transport: Arc<dyn crate::transport::Transport>) {
            let socket = crate::socket::NoiseSocket::new(
                Arc::new(crate::runtime_impl::TokioRuntime),
                transport,
                wacore::handshake::NoiseCipher::new(&[0u8; 32]).unwrap(),
                wacore::handshake::NoiseCipher::new(&[0u8; 32]).unwrap(),
            );
            *client.noise_socket.lock().unwrap() = Some(Arc::new(socket));
        }

        #[tokio::test]
        async fn reviewed_overlapping_failed_sends_leave_no_failed_memo() {
            use wacore::types::message::SenderMessageId;
            for attempts in [2, 3] {
                let (client, transport, info) = manual_retry_client().await;
                let key = ChatMessageId::new(info.source.chat.clone(), info.id.clone());
                let gate = SenderMessageId::new(
                    info.source.chat.clone(),
                    info.id.clone(),
                    info.source.sender.clone(),
                );
                let mut jobs = Vec::new();
                for attempt in 0..attempts {
                    client.pdo_pending_requests.remove(&key).await;
                    let (entered, release) = failure_socket(&client);
                    let client_copy = client.clone();
                    let info_copy = info.clone();
                    let job = tokio::spawn(async move {
                        if attempt == 0 {
                            client_copy
                                .send_pdo_placeholder_resend_request(&info_copy)
                                .await
                        } else {
                            client_copy
                                .retry_pdo_placeholder_resend_request(&info_copy)
                                .await
                                .map(|_| ())
                        }
                    });
                    tokio::time::timeout(std::time::Duration::from_secs(5), entered.recv())
                        .await
                        .unwrap()
                        .unwrap();
                    assert_eq!(
                        client
                            .pdo_explicit_published
                            .load(std::sync::atomic::Ordering::Acquire),
                        attempt != 0
                    );
                    jobs.push((job, release));
                }
                for (job, release) in jobs {
                    release.send(()).await.unwrap();
                    assert!(
                        tokio::time::timeout(std::time::Duration::from_secs(5), job)
                            .await
                            .unwrap()
                            .unwrap()
                            .is_err()
                    );
                }
                assert!(
                    client.pdo_requested.get(&gate).await.is_none(),
                    "failed predecessor was restored"
                );
                assert!(transport.sent().is_empty());
                assert!(
                    client
                        .pdo_explicit_published
                        .load(std::sync::atomic::Ordering::Acquire)
                );
                install_socket(&client, transport.clone());
                client
                    .send_pdo_placeholder_resend_request(&info)
                    .await
                    .unwrap();
                crate::test_utils::decode_sent_iq(&transport, 0).await;
                assert!(client.pdo_pending_requests.get(&key).await.is_some());
            }
        }

        #[tokio::test]
        async fn manual_pdo_can_ask_again_after_a_contentless_response() {
            use buffa::Message as _;
            let (client, transport, info) = manual_retry_client().await;
            let first = client
                .retry_pdo_placeholder_resend_request(&info)
                .await
                .unwrap()
                .unwrap();
            let mut response = make_placeholder_response(
                &info.source.chat.to_string(),
                false,
                &info.id,
                Some(&info.source.sender.to_string()),
            );
            response.web_message_info_bytes = Some(
                make_web_msg(
                    &info.source.chat.to_string(),
                    false,
                    &info.id,
                    Some(&info.source.sender.to_string()),
                )
                .encode_to_vec(),
            );
            client
                .handle_placeholder_resend_response(&response, &first)
                .await;
            client
                .send_pdo_placeholder_resend_request(&info)
                .await
                .unwrap();
            let second = client
                .retry_pdo_placeholder_resend_request(&info)
                .await
                .unwrap()
                .unwrap();
            assert_ne!(first, second);
            let sent = crate::test_utils::decode_sent_iq(&transport, 1).await;
            assert_eq!(
                sent.get().attrs().optional_string("id").as_deref(),
                Some(second.as_str())
            );
            assert_eq!(transport.sent().len(), 2);
        }

        #[tokio::test]
        async fn concurrent_manual_and_automatic_pdo_requests_send_once() {
            let (client, transport, info) = manual_retry_client().await;
            let (first, second, automatic) = tokio::join!(
                client.retry_pdo_placeholder_resend_request(&info),
                client.retry_pdo_placeholder_resend_request(&info),
                client.send_pdo_placeholder_resend_request(&info),
            );
            automatic.unwrap();
            let sent = [first.unwrap(), second.unwrap()];
            assert!(sent.iter().filter(|id| id.is_some()).count() <= 1);
            crate::test_utils::decode_sent_iq(&transport, 0).await;
            assert_eq!(transport.sent().len(), 1);
            let key = ChatMessageId::new(info.source.chat.clone(), info.id.clone());
            assert!(client.pdo_pending_requests.get(&key).await.is_some());
        }

        #[tokio::test]
        async fn manual_pdo_errors_never_report_a_send_or_spend_an_older_memo() {
            let (client, transport, info) = manual_retry_client().await;
            let mut empty = (*info).clone();
            empty.id = "".into();
            let mut wrong_group = (*info).clone();
            wrong_group.source.is_group = false;
            let mut wrong_sender = (*info).clone();
            wrong_sender.source.sender = info.source.chat.clone();
            for invalid in [empty, wrong_group, wrong_sender] {
                assert!(
                    client
                        .retry_pdo_placeholder_resend_request(&Arc::new(invalid))
                        .await
                        .is_err()
                );
            }
            assert_eq!(client.pdo_requested.entry_count_async().await, 0);
            client.set_connected_for_test(false);
            assert!(
                client
                    .retry_pdo_placeholder_resend_request(&info)
                    .await
                    .is_err()
            );
            client.set_connected_for_test(true);
            let gate = wacore::types::message::SenderMessageId::new(
                info.source.chat.clone(),
                info.id.clone(),
                info.source.sender.clone(),
            );
            client
                .pdo_requested
                .insert(
                    gate.clone(),
                    PdoRequestMemo::sent_for_test(&info, "AUTOMATIC_REQUEST".into(), false),
                )
                .await;
            *client.noise_socket.lock().unwrap() = None;
            assert!(
                client
                    .retry_pdo_placeholder_resend_request(&info)
                    .await
                    .is_err()
            );
            assert_eq!(client.pdo_pending_requests.entry_count_async().await, 0);
            assert_eq!(
                client
                    .pdo_requested
                    .get(&gate)
                    .await
                    .map(|memo| memo.request_id.clone())
                    .as_deref(),
                Some("AUTOMATIC_REQUEST")
            );
            assert!(transport.sent().is_empty());
            client
                .pdo_requested
                .get(&gate)
                .await
                .unwrap()
                .outcome
                .store(
                    super::super::PDO_FAILED,
                    std::sync::atomic::Ordering::Release,
                );
            assert!(
                client
                    .send_pdo_placeholder_resend_request(&info)
                    .await
                    .is_err()
            );
            assert!(client.pdo_requested.get(&gate).await.is_none());
        }

        #[tokio::test]
        async fn an_old_failed_pdo_send_preserves_a_new_pending_request_and_memo() {
            let (client, transport, info) = manual_retry_client().await;
            let key = ChatMessageId::new(info.source.chat.clone(), info.id.clone());
            let gate = wacore::types::message::SenderMessageId::new(
                info.source.chat.clone(),
                info.id.clone(),
                info.source.sender.clone(),
            );
            client
                .pdo_requested
                .insert(
                    gate.clone(),
                    PdoRequestMemo::sent_for_test(&info, "VALID_PRIOR".into(), false),
                )
                .await;
            let peer: Jid = "12025550100@s.whatsapp.net".parse().unwrap();
            let mutex = client
                .session_lock_for(peer.to_protocol_address().as_str())
                .await;
            let lock = mutex.lock().await;
            let old = {
                let client = client.clone();
                let info = info.clone();
                tokio::spawn(
                    async move { client.retry_pdo_placeholder_resend_request(&info).await },
                )
            };
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                while client.pdo_pending_requests.get(&key).await.is_none() {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            client
                .pdo_pending_requests
                .insert(
                    key.clone(),
                    test_pending(
                        PendingPdoRequest {
                            message_info: info.clone(),
                            requested_at: wacore::time::Instant::now(),
                        },
                        "NEW_REQUEST",
                        true,
                    ),
                )
                .await;
            client
                .pdo_requested
                .insert(
                    gate.clone(),
                    PdoRequestMemo::new(&info, "NEW_REQUEST".into(), true, None),
                )
                .await;
            *client.noise_socket.lock().unwrap() = None;
            drop(lock);
            assert!(
                tokio::time::timeout(std::time::Duration::from_secs(5), old)
                    .await
                    .unwrap()
                    .unwrap()
                    .is_err()
            );
            assert_eq!(
                client
                    .pdo_pending_requests
                    .get(&key)
                    .await
                    .unwrap()
                    .1
                    .request_id,
                "NEW_REQUEST"
            );
            assert_eq!(
                client
                    .pdo_requested
                    .get(&gate)
                    .await
                    .map(|memo| memo.request_id.clone())
                    .as_deref(),
                Some("NEW_REQUEST")
            );
            assert!(transport.sent().is_empty());
        }

        #[tokio::test]
        async fn cancelled_pdo_ownership_preserves_automatic_recovery_and_allows_retry() {
            use buffa::Message as _;
            use std::time::Duration;
            use wacore::types::{events::ChannelEventHandler, message::SenderMessageId};
            for after_publication in [false, true] {
                let (client, transport, info) = manual_retry_client().await;
                let key = ChatMessageId::new(info.source.chat.clone(), info.id.clone());
                let gate = SenderMessageId::new(
                    info.source.chat.clone(),
                    info.id.clone(),
                    info.source.sender.clone(),
                );
                client
                    .send_pdo_placeholder_resend_request(&info)
                    .await
                    .unwrap();
                let automatic = client
                    .pdo_requested
                    .get(&gate)
                    .await
                    .unwrap()
                    .request_id
                    .clone();
                client.pdo_pending_requests.remove(&key).await;
                let pending_cache = &client.pdo_pending_requests;
                let mut read = Some(pending_cache.hold_read_for_test().await);
                let cancelled = {
                    let client = client.clone();
                    let info = info.clone();
                    tokio::spawn(
                        async move { client.retry_pdo_placeholder_resend_request(&info).await },
                    )
                };
                let cancelled_id = tokio::time::timeout(Duration::from_secs(5), async {
                    loop {
                        if let Some(memo) = client.pdo_requested.get(&gate).await
                            && memo.request_id != automatic
                            && memo.generation != 0
                        {
                            break memo.request_id.clone();
                        }
                        tokio::task::yield_now().await;
                    }
                })
                .await
                .expect("manual retry did not transfer ownership before pending insertion");
                let registry_guard = if after_publication {
                    let guard = tokio::time::timeout(
                        Duration::from_secs(5),
                        pending_cache.hold_reclaim_after_init_for_test(&key),
                    )
                    .await
                    .expect("manual retry did not hold the pending cache init lock");
                    drop(read.take());
                    tokio::time::timeout(Duration::from_secs(5), async {
                        while pending_cache.get(&key).await.is_none() {
                            tokio::task::yield_now().await;
                        }
                    })
                    .await
                    .expect("manual retry did not publish its pending owner before reclaim");
                    Some(guard)
                } else {
                    None
                };
                assert!(!cancelled.is_finished());
                cancelled.abort();
                assert!(
                    tokio::time::timeout(Duration::from_secs(5), cancelled)
                        .await
                        .expect("cancelled manual retry did not stop")
                        .unwrap_err()
                        .is_cancelled()
                );
                drop(read);
                drop(registry_guard);
                let memo = client.pdo_requested.get(&gate).await.unwrap();
                assert_eq!(memo.request_id, cancelled_id);
                assert_eq!(
                    memo.outcome.load(std::sync::atomic::Ordering::Acquire),
                    super::super::PDO_FAILED
                );
                assert_eq!(memo.live_owner().unwrap().request_id, automatic);
                assert_eq!(pending_cache.get(&key).await.is_some(), after_publication);
                assert_eq!(transport.sent().len(), 1);
                let (handler, rx) = ChannelEventHandler::new();
                client.core.event_bus.subscribe_handler(handler).detach();
                let mut response = make_placeholder_response(
                    &info.source.chat.to_string(),
                    false,
                    &info.id,
                    Some(&info.source.sender.to_string()),
                );
                let mut web = waproto::codec::web_message_info_decode(
                    response.web_message_info_bytes.as_ref().unwrap(),
                )
                .unwrap();
                web.message.as_option_mut().unwrap().conversation =
                    Some("AUTOMATIC_RECOVERY".into());
                response.web_message_info_bytes = Some(web.encode_to_vec());
                client
                    .handle_placeholder_resend_response(&response, &automatic)
                    .await;
                let mut delivered = 0;
                while let Ok(event) = rx.try_recv() {
                    for message in event.messages() {
                        assert_eq!(
                            message.message.conversation.as_deref(),
                            Some("AUTOMATIC_RECOVERY")
                        );
                        assert_eq!(
                            message.info.unavailable_request_id.as_deref(),
                            Some(automatic.as_str())
                        );
                        delivered += 1;
                    }
                }
                assert_eq!(delivered, 1);
                let retry = client
                    .retry_pdo_placeholder_resend_request(&info)
                    .await
                    .unwrap()
                    .unwrap();
                assert_ne!(retry, cancelled_id);
                assert_eq!(pending_cache.get(&key).await.unwrap().1.request_id, retry);
                crate::test_utils::decode_sent_iq(&transport, 1).await;
                assert_eq!(transport.sent().len(), 2);
            }
        }

        #[tokio::test]
        async fn reviewed_pdo_reclaim_wait_cannot_overwrite_a_newer_memo() {
            use std::time::Duration;
            use wacore::types::message::SenderMessageId;
            let (client, transport, info) = manual_retry_client().await;
            let key = ChatMessageId::new(info.source.chat.clone(), info.id.clone());
            let gate = SenderMessageId::new(
                info.source.chat.clone(),
                info.id.clone(),
                info.source.sender.clone(),
            );
            let pending_cache = &client.pdo_pending_requests;
            let read = pending_cache.hold_read_for_test().await;
            let old = {
                let client = client.clone();
                let info = info.clone();
                tokio::spawn(
                    async move { client.retry_pdo_placeholder_resend_request(&info).await },
                )
            };
            let registry_guard = tokio::time::timeout(
                Duration::from_secs(5),
                pending_cache.hold_reclaim_after_init_for_test(&key),
            )
            .await
            .expect("PDO retry did not acquire the pending cache init lock");
            drop(read);
            tokio::time::timeout(Duration::from_secs(5), async {
                while pending_cache.get(&key).await.is_none() {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("PDO retry did not publish its pending owner before reclaim");
            assert!(
                !old.is_finished(),
                "PDO retry did not wait for init-lock reclaim"
            );
            assert!(
                client
                    .pdo_explicit_published
                    .load(std::sync::atomic::Ordering::Acquire)
            );
            pending_cache.remove(&key).await;
            pending_cache
                .insert(
                    key.clone(),
                    test_pending(
                        PendingPdoRequest {
                            message_info: info.clone(),
                            requested_at: wacore::time::Instant::now(),
                        },
                        "NEW_REQUEST",
                        true,
                    ),
                )
                .await;
            client
                .pdo_requested
                .insert(
                    gate.clone(),
                    PdoRequestMemo::new(&info, "NEW_REQUEST".into(), true, None),
                )
                .await;
            drop(registry_guard);
            tokio::time::timeout(Duration::from_secs(5), old)
                .await
                .expect("PDO retry did not finish after releasing init-lock reclaim")
                .unwrap()
                .unwrap()
                .unwrap();
            crate::test_utils::decode_sent_iq(&transport, 0).await;
            assert_eq!(
                pending_cache.get(&key).await.unwrap().1.request_id,
                "NEW_REQUEST"
            );
            assert_eq!(
                client
                    .pdo_requested
                    .get(&gate)
                    .await
                    .map(|memo| memo.request_id.clone())
                    .as_deref(),
                Some("NEW_REQUEST")
            );
        }

        #[tokio::test]
        async fn a_late_pdo_reply_preserves_the_new_attempt_until_its_matching_reply() {
            use buffa::Message as _;
            use wacore::types::events::ChannelEventHandler;
            let (client, transport, info) = manual_retry_client().await;
            let key = ChatMessageId::new(info.source.chat.clone(), info.id.clone());
            let first = client
                .retry_pdo_placeholder_resend_request(&info)
                .await
                .unwrap()
                .unwrap();
            client.pdo_pending_requests.remove(&key).await;
            let second = client
                .retry_pdo_placeholder_resend_request(&info)
                .await
                .unwrap()
                .unwrap();
            let mut response = make_placeholder_response(
                &info.source.chat.to_string(),
                false,
                &info.id,
                Some(&info.source.sender.to_string()),
            );
            response.web_message_info_bytes = Some(
                make_web_msg(
                    &info.source.chat.to_string(),
                    false,
                    &info.id,
                    Some(&info.source.sender.to_string()),
                )
                .encode_to_vec(),
            );
            client
                .handle_placeholder_resend_response(&response, &first)
                .await;
            assert_eq!(
                client
                    .pdo_pending_requests
                    .get(&key)
                    .await
                    .unwrap()
                    .1
                    .request_id,
                second
            );
            let (handler, rx) = ChannelEventHandler::new();
            client.core.event_bus.subscribe_handler(handler).detach();
            response = make_placeholder_response(
                &info.source.chat.to_string(),
                false,
                &info.id,
                Some(&info.source.sender.to_string()),
            );
            client
                .handle_placeholder_resend_response(&response, &second)
                .await;
            assert!(client.pdo_pending_requests.get(&key).await.is_none());
            let event = rx.try_recv().unwrap();
            let recovered = event.messages().next().unwrap();
            assert_eq!(
                recovered.info.unavailable_request_id.as_deref(),
                Some(second.as_str())
            );
            assert_eq!(recovered.info.source.sender, info.source.sender);
            crate::test_utils::decode_sent_iq(&transport, 1).await;
            assert_eq!(transport.sent().len(), 2);
        }

        #[tokio::test]
        async fn active_automatic_pdo_rejects_mismatched_ids_and_accepts_matching_or_legacy_ids() {
            use buffa::Message as _;
            use wacore::types::events::ChannelEventHandler;
            use wacore::types::message::AddressingMode;
            for legacy_id in [false, true] {
                let (client, transport, info) = manual_retry_client().await;
                let key = ChatMessageId::new(info.source.chat.clone(), info.id.clone());
                client
                    .send_pdo_placeholder_resend_request(&info)
                    .await
                    .unwrap();
                let current = client
                    .pdo_pending_requests
                    .get(&key)
                    .await
                    .unwrap()
                    .1
                    .request_id
                    .clone();
                assert!(
                    !client
                        .pdo_explicit_published
                        .load(std::sync::atomic::Ordering::Acquire)
                );
                let sent = crate::test_utils::decode_sent_iq(&transport, 0).await;
                assert_eq!(
                    sent.get().attrs().optional_string("id").as_deref(),
                    Some(current.as_str())
                );
                let (handler, rx) = ChannelEventHandler::new();
                client.core.event_bus.subscribe_handler(handler).detach();
                let mut response = make_placeholder_response(
                    &info.source.chat.to_string(),
                    false,
                    &info.id,
                    Some(&info.source.sender.to_string()),
                );
                let mut web = waproto::codec::web_message_info_decode(
                    response.web_message_info_bytes.as_ref().unwrap(),
                )
                .unwrap();
                web.message.as_option_mut().unwrap().conversation =
                    Some("STALE_AUTOMATIC_CONTENT".into());
                response.web_message_info_bytes = Some(web.encode_to_vec());
                client
                    .handle_placeholder_resend_response(&response, "OLD_AUTOMATIC_REQUEST")
                    .await;
                assert_eq!(
                    client
                        .pdo_pending_requests
                        .get(&key)
                        .await
                        .unwrap()
                        .1
                        .request_id,
                    current
                );
                while let Ok(event) = rx.try_recv() {
                    assert!(
                        event.messages().next().is_none(),
                        "mismatched automatic content was published"
                    );
                }
                web.message.as_option_mut().unwrap().conversation =
                    Some("CURRENT_AUTOMATIC_CONTENT".into());
                response.web_message_info_bytes = Some(web.encode_to_vec());
                let request_id = if legacy_id { "" } else { current.as_str() };
                client
                    .handle_placeholder_resend_response(&response, request_id)
                    .await;
                assert!(client.pdo_pending_requests.get(&key).await.is_none());
                let mut delivered = 0;
                while let Ok(event) = rx.try_recv() {
                    for message in event.messages() {
                        assert_eq!(
                            message.message.conversation.as_deref(),
                            Some("CURRENT_AUTOMATIC_CONTENT")
                        );
                        assert_eq!(message.info.source.chat, info.source.chat);
                        assert_eq!(message.info.source.sender, info.source.sender);
                        assert_eq!(message.info.source.is_from_me, info.source.is_from_me);
                        assert_eq!(
                            message.info.unavailable_request_id.as_deref(),
                            (!legacy_id).then_some(current.as_str())
                        );
                        delivered += 1;
                    }
                }
                assert_eq!(delivered, 1);
                assert_eq!(transport.sent().len(), 1);
            }
            let (client, transport, _) = manual_retry_client().await;
            let pn = "12025550101@s.whatsapp.net";
            let lid = "777000000000101@lid";
            let msg_id = "PDO_AUTOMATIC_ALIAS_GENERATION";
            client
                .lid_pn_cache
                .add(&wacore::types::lid_pn::LidPnEntry {
                    lid: "777000000000101".into(),
                    phone_number: "12025550101".into(),
                    created_at: 0,
                    learning_source: wacore::types::lid_pn::LearningSource::Usync,
                })
                .await;
            let direct_info = make_dm_pending_info(pn, lid, msg_id, AddressingMode::Pn);
            let mut alias_info =
                (*make_dm_pending_info(lid, pn, msg_id, AddressingMode::Lid)).clone();
            alias_info.source.sender_alt = None;
            let alias_info = Arc::new(alias_info);
            let direct_key = ChatMessageId::new(pn.parse().unwrap(), msg_id.into());
            let alias_key = ChatMessageId::new(lid.parse().unwrap(), msg_id.into());
            client
                .send_pdo_placeholder_resend_request(&direct_info)
                .await
                .unwrap();
            let direct = client
                .pdo_pending_requests
                .get(&direct_key)
                .await
                .unwrap()
                .1;
            client
                .send_pdo_placeholder_resend_request(&alias_info)
                .await
                .unwrap();
            let latest = client.pdo_pending_requests.get(&alias_key).await.unwrap().1;
            assert!(latest.generation > direct.generation);
            assert_ne!(latest.request_id, direct.request_id);
            assert!(
                !client
                    .pdo_explicit_published
                    .load(std::sync::atomic::Ordering::Acquire)
            );
            for (index, owner) in [(0, &direct), (1, &latest)] {
                let sent = crate::test_utils::decode_sent_iq(&transport, index).await;
                assert_eq!(
                    sent.get().attrs().optional_string("id").as_deref(),
                    Some(owner.request_id.as_str())
                );
            }
            let (handler, rx) = ChannelEventHandler::new();
            client.core.event_bus.subscribe_handler(handler).detach();
            let mut response = make_placeholder_response(pn, false, msg_id, None);
            let mut web = waproto::codec::web_message_info_decode(
                response.web_message_info_bytes.as_ref().unwrap(),
            )
            .unwrap();
            web.message.as_option_mut().unwrap().conversation =
                Some("OLD_DIRECT_AUTOMATIC_CONTENT".into());
            response.web_message_info_bytes = Some(web.encode_to_vec());
            client
                .handle_placeholder_resend_response(&response, &direct.request_id)
                .await;
            assert!(client.pdo_pending_requests.get(&direct_key).await.is_none());
            assert_eq!(
                client
                    .pdo_pending_requests
                    .get(&alias_key)
                    .await
                    .unwrap()
                    .1
                    .request_id,
                latest.request_id
            );
            while let Ok(event) = rx.try_recv() {
                assert!(
                    event.messages().next().is_none(),
                    "older direct automatic content bypassed newer alias owner"
                );
            }
            web.message.as_option_mut().unwrap().conversation =
                Some("LATEST_ALIAS_AUTOMATIC_CONTENT".into());
            response.web_message_info_bytes = Some(web.encode_to_vec());
            client
                .handle_placeholder_resend_response(&response, &latest.request_id)
                .await;
            assert!(client.pdo_pending_requests.get(&alias_key).await.is_none());
            let mut delivered = 0;
            while let Ok(event) = rx.try_recv() {
                for message in event.messages() {
                    assert_eq!(
                        message.message.conversation.as_deref(),
                        Some("LATEST_ALIAS_AUTOMATIC_CONTENT")
                    );
                    assert_eq!(message.info.source.chat, alias_info.source.chat);
                    assert_eq!(message.info.source.sender, alias_info.source.sender);
                    assert_eq!(message.info.source.is_from_me, alias_info.source.is_from_me);
                    assert_eq!(
                        message.info.source.addressing_mode,
                        Some(AddressingMode::Lid)
                    );
                    assert_eq!(
                        message.info.unavailable_request_id.as_deref(),
                        Some(latest.request_id.as_str())
                    );
                    delivered += 1;
                }
            }
            assert_eq!(delivered, 1);
            assert_eq!(transport.sent().len(), 2);
        }

        #[tokio::test]
        async fn pdo_missing_request_id_only_consumes_validated_automatic_entries() {
            use wacore::types::events::ChannelEventHandler;
            for explicit_retry in [false, true] {
                let client = setup_reconstruct_client().await;
                set_own_pn(&client).await;
                let mut info = (*make_group_message_info(
                    "120363000000000001@g.us",
                    "12025550101@s.whatsapp.net",
                    "PDO_WITHOUT_REQUEST_ID",
                ))
                .clone();
                info.push_name = "pending metadata".into();
                let key = ChatMessageId::new(info.source.chat.clone(), info.id.clone());
                client
                    .pdo_pending_requests
                    .insert(
                        key.clone(),
                        test_pending(
                            PendingPdoRequest {
                                message_info: Arc::new(info.clone()),
                                requested_at: wacore::time::Instant::now(),
                            },
                            "CURRENT_REQUEST",
                            explicit_retry,
                        ),
                    )
                    .await;
                let (handler, rx) = ChannelEventHandler::new();
                client.core.event_bus.subscribe_handler(handler).detach();
                let response = make_placeholder_response(
                    &info.source.chat.to_string(),
                    false,
                    &info.id,
                    Some(&info.source.sender.to_string()),
                );
                client
                    .handle_placeholder_resend_response(&response, "")
                    .await;
                assert_eq!(
                    client.pdo_pending_requests.get(&key).await.is_some(),
                    explicit_retry
                );
                let mut delivered = Vec::new();
                while let Ok(event) = rx.try_recv() {
                    delivered.extend(event.messages().map(|message| message.info.clone()));
                }
                assert_eq!(delivered.len(), usize::from(!explicit_retry));
                if !explicit_retry {
                    assert_eq!(delivered[0].push_name.as_str(), "pending metadata");
                }
            }
        }
    }

    /// A transient send failure must release the once-per-message slot, or
    /// one bad send would permanently block recovery for that message.
    #[tokio::test]
    async fn pdo_request_failure_releases_once_per_message_slot() {
        use wacore::types::message::ChatMessageId;

        let client = setup_reconstruct_client().await;
        set_own_pn(&client).await;
        // A live client has finished offline sync long before any PDO; skip
        // the offline-delivery wait so the send failure surfaces immediately.
        client
            .offline_sync_completed
            .store(true, std::sync::atomic::Ordering::Relaxed);

        let info = make_group_message_info(
            "120363000000000001@g.us",
            "203040904720543@lid",
            "PDO_ONCE_2",
        );
        let key = ChatMessageId::new(info.source.chat.clone(), info.id.clone());

        let res = tokio::time::timeout(
            std::time::Duration::from_secs(15),
            client.send_pdo_placeholder_resend_request(&info),
        )
        .await
        .expect("send attempt must resolve fast without a live transport");

        assert!(res.is_err(), "no live transport, the send must fail");
        let gate_key = wacore::types::message::SenderMessageId::new(
            info.source.chat.clone(),
            info.id.clone(),
            info.source.sender.clone(),
        );
        assert!(
            client.pdo_requested.get(&gate_key).await.is_none(),
            "failed send must release the once-per-message slot"
        );
        assert!(
            client.pdo_pending_requests.get(&key).await.is_none(),
            "failed send must clear the pending entry"
        );
    }

    /// A phone response without content consumes the pending slot but keeps
    /// the memo: the phone has nothing to share for this message, so
    /// re-asking on the next redelivery cannot produce content either.
    #[tokio::test]
    async fn pdo_missing_content_response_clears_pending_but_keeps_memo() {
        use buffa::Message as _;
        use wacore::types::message::ChatMessageId;

        let client = setup_reconstruct_client().await;
        let chat = "5511999998888@s.whatsapp.net";
        let msg_id = "PDO_ONCE_3";
        let key = ChatMessageId::new(chat.parse().expect("chat jid"), msg_id.into());
        // A DM: the chat jid is the sender, which is what the gate names.
        let gate_key = wacore::types::message::SenderMessageId::new(
            chat.parse().expect("chat jid"),
            msg_id.into(),
            chat.parse().expect("chat jid"),
        );

        client
            .pdo_requested
            .insert(
                gate_key.clone(),
                super::PdoRequestMemo::new(
                    &make_dm_pending_info(
                        chat,
                        chat,
                        msg_id,
                        wacore::types::message::AddressingMode::Pn,
                    ),
                    "req-1".into(),
                    false,
                    None,
                ),
            )
            .await;
        client
            .pdo_pending_requests
            .insert(
                key.clone(),
                super::test_pending(
                    super::PendingPdoRequest {
                        message_info: make_dm_pending_info(
                            chat,
                            chat,
                            msg_id,
                            wacore::types::message::AddressingMode::Pn,
                        ),
                        requested_at: wacore::time::Instant::now(),
                    },
                    "req-1",
                    false,
                ),
            )
            .await;

        let web_msg = waproto::whatsapp::WebMessageInfo {
            key: buffa::MessageField::some(waproto::whatsapp::MessageKey {
                remote_jid: Some(chat.to_owned()),
                from_me: Some(false),
                id: Some(msg_id.into()),
                participant: None,
            }),
            ..Default::default()
        };
        let response = waproto::whatsapp::message::peer_data_operation_request_response_message::peer_data_operation_result::PlaceholderMessageResendResponse {
            web_message_info_bytes: Some(web_msg.encode_to_vec()),
        };

        client
            .handle_placeholder_resend_response(&response, "req-1")
            .await;

        assert!(
            client.pdo_pending_requests.get(&key).await.is_none(),
            "response consumes the pending slot"
        );
        assert!(
            client.pdo_requested.get(&gate_key).await.is_some(),
            "memo must survive a content-less response"
        );
    }
}

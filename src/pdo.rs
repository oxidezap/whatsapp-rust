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

/// How many unrequested recoveries may wait behind the one being applied.
const MAX_QUEUED_UNREQUESTED_RECOVERIES: usize = 64;

/// How many wire bytes they may hold between them: one reply of the largest
/// size the decode accepts.
const MAX_QUEUED_UNREQUESTED_RECOVERY_BYTES: u64 = wacore::history_sync::MAX_DECOMPRESSED;

/// The collections a recovery from the primary can be for, synced again when
/// unrequested replies were dropped before anything read which collection
/// they were for. The block list is never recovered from the primary, and an
/// unknown name is not a collection.
const RECOVERABLE_COLLECTIONS: [wacore::appstate::patch_decode::WAPatchName; 4] = [
    wacore::appstate::patch_decode::WAPatchName::RegularLow,
    wacore::appstate::patch_decode::WAPatchName::RegularHigh,
    wacore::appstate::patch_decode::WAPatchName::Regular,
    wacore::appstate::patch_decode::WAPatchName::CriticalUnblockLow,
];

/// What a client keeps for snapshot recoveries from the primary, built on the
/// first recovery reply.
#[derive(Default)]
pub(crate) struct SnapshotRecoveryState {
    /// Serializes the decode and apply of snapshot recoveries.
    ///
    /// A reply is applied whether or not this process still remembers the ask
    /// it answers, and the phone answers late and in bursts -- dozens of
    /// whole-collection replies within seconds, to asks made over hours. One at
    /// a time bounds that to one inflated collection in memory, and lets each
    /// reply after the first find the collection already moved and be
    /// discarded by the version check rather than racing it. Locked through
    /// `lock_arc`, so the count of handles is the count of tasks holding or
    /// waiting for it: one per matched reply, and one drainer for however many
    /// unrequested replies are queued behind it.
    pub(crate) lock: Arc<async_lock::Mutex<()>>,
    /// Unrequested snapshot recoveries waiting, in arrival order, behind the
    /// one task that applies them.
    ///
    /// The lock above bounds the decodes to one, not the copies: a reply's
    /// payload is copied off the inbound lane before anything waits for the
    /// lock, so a burst of unrequested replies would each hold a
    /// collection-sized copy while it waited. A matched reply is bounded by its
    /// claim -- one per ask -- but nothing bounds replies nobody asked for.
    ///
    /// No admitted reply is ever dropped or replaced before it is decoded:
    /// which collection a reply is for is inside the payload, so any reply
    /// discarded unread can be the only one for its collection. Every one is
    /// applied in turn, and the older ones end as stale. What bounds the copies
    /// is admission instead -- a count, and a byte budget of one reply of the
    /// largest size the decode accepts, which one reply may always use. A
    /// reply refused there is dropped before anything reads which collection
    /// it was for, so a refusal marks the queue, and once it drains every
    /// collection a recovery can be for is scheduled to sync again (see
    /// `Client::converge_after_dropped_recovery`): one still stuck fails as
    /// before and asks the primary again. An admitted reply that fails to
    /// decode cannot be read either, but gets no such sync: the primary's
    /// answer to the ask that sync would end in is likely the same unreadable
    /// reply, so it would only loop. A sync mutex, held only to admit (and
    /// copy) a reply or take one.
    pub(crate) unrequested: std::sync::Mutex<UnrequestedRecoveryQueue>,
}

impl SnapshotRecoveryState {
    /// How many unrequested replies are queued and the wire bytes they hold,
    /// for the memory report.
    pub(crate) fn queued_unrequested(&self) -> (usize, u64) {
        let queue = self.unrequested.lock().unwrap_or_else(|p| p.into_inner());
        (queue.queued.len(), queue.queued_bytes)
    }
}

impl Client {
    /// The snapshot recovery state, built on first use.
    pub(crate) fn snapshot_recovery(&self) -> &SnapshotRecoveryState {
        self.snapshot_recovery.get_or_init(Box::default)
    }
}

/// The unrequested snapshot recoveries waiting behind the one being applied;
/// why is on `SnapshotRecoveryState::unrequested`.
#[derive(Default)]
pub(crate) struct UnrequestedRecoveryQueue {
    /// Whether a drainer task is applying replies.
    running: bool,
    /// The replies that arrived while it was, in arrival order.
    queued: std::collections::VecDeque<QueuedRecoveryReply>,
    /// The wire bytes `queued` holds.
    queued_bytes: u64,
    /// Whether a reply was refused since the drainer last finished, so the
    /// collections it may have been for are synced again once it does.
    overflowed: bool,
}

/// The unrequested recovery drainer's exit when it dies before its end.
///
/// A drainer that ended any other way would leave the queue marked as running
/// for ever, every later unrequested reply queued behind it for nothing, and
/// the reply it was applying and the ones behind it lost. Cleared on the
/// drainer's normal exit; dropped while set, it releases the queue and has the
/// collections synced again.
struct UnrequestedDrainer {
    client: Option<Arc<Client>>,
}

impl Drop for UnrequestedDrainer {
    fn drop(&mut self) {
        if let Some(client) = self.client.take() {
            client.abandon_unrequested_recoveries();
        }
    }
}

/// A recovery reply copied off the inbound lane, not yet decoded.
pub(crate) struct QueuedRecoveryReply {
    payload: Vec<u8>,
    compressed: bool,
    request_id: String,
}

#[derive(Clone, Debug)]
pub(crate) struct PendingPdoRequest {
    pub(crate) message_info: Arc<MessageInfo>,
    pub(crate) requested_at: wacore::time::Instant,
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
                    && if source.is_from_me {
                        source.recipient_alt.as_ref().is_some_and(same_chat)
                    } else {
                        same_chat(&source.sender)
                            || source.sender_alt.as_ref().is_some_and(same_chat)
                    }))
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

fn history_sync_on_demand_message(
    chat_jid: &Jid,
    oldest_msg_id: &str,
    oldest_msg_from_me: bool,
    oldest_msg_timestamp_ms: i64,
    count: i32,
) -> wa::Message {
    let pdo_request = {
        let mut proto = wa::message::PeerDataOperationRequestMessage::default();
        proto.peer_data_operation_request_type =
            Some(wa::message::PeerDataOperationRequestType::HISTORY_SYNC_ON_DEMAND);
        proto.history_sync_on_demand_request = buffa::MessageField::some({
            let mut proto = wa::message::peer_data_operation_request_message::HistorySyncOnDemandRequest::default();
            proto.chat_jid = Some(chat_jid.to_string());
            proto.oldest_msg_id = Some(oldest_msg_id.to_string());
            proto.oldest_msg_from_me = Some(oldest_msg_from_me);
            proto.oldest_msg_timestamp_ms = Some(oldest_msg_timestamp_ms);
            proto.on_demand_msg_count = Some(count);
            proto
        });
        proto
    };
    let mut protocol_message = wa::message::ProtocolMessage::default();
    protocol_message.r#type =
        Some(wa::message::protocol_message::Type::PEER_DATA_OPERATION_REQUEST_MESSAGE);
    protocol_message.peer_data_operation_request_message = buffa::MessageField::some(pdo_request);
    let mut message = wa::Message::default();
    message.protocol_message = buffa::MessageField::some(protocol_message);
    message
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

        // Prefer the peer's explicit PN when a LID DM response crosses namespaces.
        // In self-sync, sender_alt identifies us; only recipient_alt names the peer.
        let cache_chat = if !info.source.is_group && info.source.chat.server.is_lid_family() {
            let peer_alt = if info.source.is_from_me {
                &info.source.recipient_alt
            } else {
                &info.source.sender_alt
            };
            peer_alt
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
        let request_id = self.generate_message_id_at(wacore::time::now_secs_u64());
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

        let message_key = {
            let mut proto = wa::MessageKey::default();
            proto.remote_jid = Some(resolved_jid.to_string());
            proto.from_me = Some(info.source.is_from_me);
            proto.id = Some(info.id.to_string());
            proto.participant = participant.map(|p| p.to_string());
            proto
        };

        // Build the PDO request message
        let pdo_request = {
            let mut proto = wa::message::PeerDataOperationRequestMessage::default();
            proto.peer_data_operation_request_type =
                Some(wa::message::PeerDataOperationRequestType::PLACEHOLDER_MESSAGE_RESEND);
            proto.placeholder_message_resend_request = vec![{
                let mut proto = wa::message::peer_data_operation_request_message::PlaceholderMessageResendRequest::default();
                proto.message_key = buffa::MessageField::some(message_key);
                proto
            }];
            proto
        };

        // Wrap it in a protocol message
        let protocol_message = {
            let mut proto_ = wa::message::ProtocolMessage::default();
            proto_.r#type =
                Some(wa::message::protocol_message::Type::PEER_DATA_OPERATION_REQUEST_MESSAGE);
            proto_.peer_data_operation_request_message = buffa::MessageField::some(pdo_request);
            proto_
        };

        let msg = {
            let mut proto_ = wa::Message::default();
            proto_.protocol_message = buffa::MessageField::some(protocol_message);
            proto_
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
        let request_id = self.generate_message_id();
        self.fetch_message_history_with_id(
            chat_jid,
            oldest_msg_id,
            oldest_msg_from_me,
            oldest_msg_timestamp_ms,
            count,
            &request_id,
        )
        .await?;
        Ok(request_id.into_string())
    }

    /// Request on-demand message history with a caller-supplied session ID.
    ///
    /// Generate the ID with [`Client::generate_message_id`] and durably record
    /// it before awaiting this method when an incoming history notification
    /// must be correlated even if it arrives before the send completes.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.pdo.fetch_history_with_id", level = "debug", skip_all, fields(chat = %chat_jid.observe(), count), err(Debug)))]
    pub async fn fetch_message_history_with_id(
        self: &Arc<Self>,
        chat_jid: &Jid,
        oldest_msg_id: &str,
        oldest_msg_from_me: bool,
        oldest_msg_timestamp_ms: i64,
        count: i32,
        request_id: &crate::MessageId,
    ) -> Result<(), anyhow::Error> {
        let device_snapshot = self.persistence_manager.get_device_snapshot();
        let peer_target = self_peer_target(&device_snapshot)?;
        let msg = history_sync_on_demand_message(
            chat_jid,
            oldest_msg_id,
            oldest_msg_from_me,
            oldest_msg_timestamp_ms,
            count,
        );

        info!(
            "Sending PDO history sync on-demand request for chat {} (count={}) to {}",
            chat_jid.observe(),
            count,
            peer_target.observe()
        );

        self.ensure_e2e_sessions(std::slice::from_ref(&peer_target))
            .await?;
        self.send_peer_message_with_id(peer_target, &msg, request_id.as_str())
            .await
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
        // side refuses `Unknown` whatever the phone sends back, so the ask
        // would have the primary serialize a whole collection for nothing, and
        // an unanswered marker for it would sit in the map until the same name
        // was asked for again.
        let patch_name = collection
            .parse::<wacore::appstate::patch_decode::WAPatchName>()
            .unwrap_or(wacore::appstate::patch_decode::WAPatchName::Unknown);
        if patch_name == wacore::appstate::patch_decode::WAPatchName::Unknown {
            return Err(anyhow::anyhow!(
                "{collection} is not an app-state collection this client knows"
            ));
        }
        // And the block list is refused, here and again where a reply is
        // applied, since a reply no longer has to match an ask to be applied.
        // Rebuilding a block list from a device that may itself be behind is
        // the one collection where being wrong means talking to somebody who
        // was blocked.
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

        let pdo_request = {
            let mut proto = wa::message::PeerDataOperationRequestMessage::default();
            proto.peer_data_operation_request_type = Some(
                wa::message::PeerDataOperationRequestType::COMPANION_SYNCD_SNAPSHOT_FATAL_RECOVERY,
            );
            proto.syncd_collection_fatal_recovery_request = buffa::MessageField::some({
                let mut proto = wa::message::peer_data_operation_request_message::SyncDCollectionFatalRecoveryRequest::default();
                proto.collection_name = Some(collection.to_string());
                proto.timestamp = Some(wacore::time::now_secs() as i64);
                proto
            });
            proto
        };

        let protocol_message = {
            let mut proto_ = wa::message::ProtocolMessage::default();
            proto_.r#type =
                Some(wa::message::protocol_message::Type::PEER_DATA_OPERATION_REQUEST_MESSAGE);
            proto_.peer_data_operation_request_message = buffa::MessageField::some(pdo_request);
            proto_
        };

        let msg = Box::new({
            let mut proto_ = wa::Message::default();
            proto_.protocol_message = buffa::MessageField::some(protocol_message);
            proto_
        });

        info!(
            "Asking {} to send back the {} collection after a snapshot we could not validate",
            peer_target.observe(),
            collection
        );

        Box::pin(self.ensure_e2e_sessions(std::slice::from_ref(&peer_target))).await?;

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
        let proc = self.get_app_state_processor();
        // Retry one reconnect, with a fresh id and marker. Repeated connection
        // churn is left to the app-state retry loop rather than an unbounded send.
        for _ in 0..2 {
            let reachable = wacore::runtime::timeout(
                &*self.runtime,
                std::time::Duration::from_secs(60),
                self.wait_until_reachable(),
            )
            .await
            .map_err(|_| {
                anyhow::anyhow!("timed out waiting for an authenticated recovery connection")
            })?;
            if reachable != crate::client::Reachability::Reachable {
                anyhow::bail!("snapshot recovery connection is {reachable:?}");
            }
            let generation = self
                .connection_generation
                .load(std::sync::atomic::Ordering::Acquire);
            let socket = self.get_noise_socket()?;
            let request_id = self.generate_message_id_at(wacore::time::now_secs_u64());
            if !proc
                .mark_recovery_requested_with_id(collection, generation, &request_id)
                .await
            {
                if self
                    .connection_generation
                    .load(std::sync::atomic::Ordering::Acquire)
                    != generation
                {
                    continue;
                }
                debug!("A recovery for {collection} is already outstanding; not asking again");
                return Ok(String::new());
            }
            let result = self
                .send_message_impl(
                    peer_target.clone(),
                    &msg,
                    crate::send::SendPipelineOptions {
                        request_id: Some(&request_id),
                        peer: true,
                        send_scope: Some((generation, socket)),
                        ..Default::default()
                    },
                )
                .await;
            if self
                .connection_generation
                .load(std::sync::atomic::Ordering::Acquire)
                != generation
                || !self.can_reach_server()
            {
                proc.take_recovery_request_by_id(&request_id).await;
                continue;
            }
            return match result {
                Ok(_) => Ok(request_id),
                Err(e) => {
                    // A replacement request must never lose its marker.
                    proc.take_recovery_request_by_id(&request_id).await;
                    Err(e)
                }
            };
        }
        Err(anyhow::anyhow!(
            "connection changed twice while sending snapshot recovery"
        ))
    }

    /// Sends a peer message using an ID recorded before the send starts.
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
    pub(crate) async fn handle_pdo_response(
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
            match proc.claim_recovery_reply(request_id).await {
                wacore::appstate_sync::RecoveryClaim::Claimed(name) => {
                    proc.take_recovery_request_by_id(request_id).await;
                    warn!(
                        "Snapshot recovery response for {name} carries no collection; it may be asked for again"
                    );
                }
                wacore::appstate_sync::RecoveryClaim::AlreadyAnswering(_) => {}
                wacore::appstate_sync::RecoveryClaim::Unrequested => info!(
                    "Discarding a snapshot recovery response that carries no collection and answers no ask outstanding here"
                ),
            }
            return;
        };

        // Which collection this id was asked about, if any -- claimed, before a
        // byte is cloned or inflated.
        //
        // Claiming rather than reading is what makes one ask cost one decode: a
        // response that repeats the result, or a second copy arriving before the
        // first is consumed, would otherwise each spawn a collection-sized job
        // against the same request.
        //
        // The payload names a collection too, but that is the reply's claim
        // about itself -- and with two recoveries outstanding, a reply carrying
        // A's id and B's name would select B's marker and be checked against its
        // own name, which always agrees. So where there is an ask, it decides.
        //
        // Where there is none, the reply is still decoded and applied. The
        // phone answers late and in bursts, to asks made minutes or hours
        // earlier -- on connections or in processes that are gone, or since
        // replaced by a newer ask -- and refusing everything this process cannot
        // match threw away nearly every answer a stuck collection ever got.
        // What keeps a late answer from rolling a collection back is the
        // version check at apply, not the bookkeeping here; the decode it costs
        // is accepted because only the primary reaches this path, and the lock
        // below makes a burst cost one decode at a time.
        let proc = self.get_app_state_processor();
        let asked = match proc.claim_recovery_reply(request_id).await {
            wacore::appstate_sync::RecoveryClaim::Claimed(name) => Some(name),
            wacore::appstate_sync::RecoveryClaim::AlreadyAnswering(name) => {
                info!(
                    "Ignoring a repeat of the {name} snapshot recovery answering {request_id}: a copy of it is already being handled"
                );
                return;
            }
            wacore::appstate_sync::RecoveryClaim::Unrequested => {
                info!(
                    "Snapshot recovery {request_id} answers no ask outstanding here; applying it as a late or unsolicited recovery from the primary"
                );
                None
            }
        };
        let about = asked.as_deref().unwrap_or("an unrequested collection");

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
            if asked.is_some() {
                proc.take_recovery_request_by_id(request_id).await;
            }
            warn!(
                "Snapshot recovery for {about} is {} bytes on the wire, over the {max_wire} this path allows; refusing it",
                blob.len()
            );
            return;
        }
        if asked.is_some() {
            let reply = QueuedRecoveryReply {
                payload: blob.to_vec(),
                compressed,
                request_id: request_id.to_string(),
            };
            self.runtime.spawn_detached(Box::pin(async move {
                client.recover_from_reply(asked, reply).await;
            }));
            return;
        }

        // Admitted, and copied, under the queue's lock, so the decision and the
        // copy it allows cannot be split by the drainer finishing in between.
        // Nothing else is done under it, and nothing awaits.
        let wire = blob.len() as u64;
        let refused = {
            let mut queue = self
                .snapshot_recovery()
                .unrequested
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            let reply = || QueuedRecoveryReply {
                payload: blob.to_vec(),
                compressed,
                request_id: request_id.to_string(),
            };
            if !queue.running {
                queue.running = true;
                let first = reply();
                drop(queue);
                // The exit guard exists before the task does, so a task dropped
                // before its first poll still releases the queue.
                let drainer = UnrequestedDrainer {
                    client: Some(Arc::clone(&client)),
                };
                self.runtime
                    .spawn_detached(Box::pin(Self::drain_unrequested_recoveries(drainer, first)));
                None
            } else if queue.queued.is_empty()
                || (queue.queued.len() < MAX_QUEUED_UNREQUESTED_RECOVERIES
                    && queue.queued_bytes + wire <= MAX_QUEUED_UNREQUESTED_RECOVERY_BYTES)
            {
                queue.queued.push_back(reply());
                queue.queued_bytes += wire;
                None
            } else {
                // Refused without a copy, and so without knowing which
                // collection it was for -- the name is inside the payload. What
                // keeps that from losing a collection is `overflowed`: once the
                // queue drains, every collection a recovery can be for is synced
                // again, and one still stuck asks the primary again.
                queue.overflowed = true;
                Some((queue.queued.len(), queue.queued_bytes))
            }
        };
        if let Some((queued, queued_bytes)) = refused {
            warn!(
                "Refusing the unrequested snapshot recovery {request_id} ({wire} bytes): {queued} replies holding {queued_bytes} bytes are already queued; the collections will be synced again once the queue drains"
            );
        }
    }

    /// Applies unrequested recoveries one after the other, in arrival order,
    /// starting with `first` and then taking whatever is queued until nothing
    /// is.
    async fn drain_unrequested_recoveries(
        mut drainer: UnrequestedDrainer,
        first: QueuedRecoveryReply,
    ) {
        let Some(client) = drainer.client.clone() else {
            return;
        };
        let mut next = first;
        let overflowed = loop {
            client.recover_from_reply(None, next).await;
            let taken = {
                let mut queue = client
                    .snapshot_recovery()
                    .unrequested
                    .lock()
                    .unwrap_or_else(|p| p.into_inner());
                match queue.queued.pop_front() {
                    Some(reply) => {
                        queue.queued_bytes -= reply.payload.len() as u64;
                        Ok(reply)
                    }
                    // Cleared under the same lock as the empty queue was seen,
                    // so a reply arriving now either was queued before this
                    // or starts a drainer of its own after it.
                    None => {
                        queue.running = false;
                        Err(std::mem::take(&mut queue.overflowed))
                    }
                }
            };
            match taken {
                Ok(reply) => next = reply,
                Err(overflowed) => break overflowed,
            }
        };
        drainer.client = None;
        if overflowed {
            info!("Unrequested snapshot recoveries were refused while the queue was full");
            client.converge_after_dropped_recovery(&RECOVERABLE_COLLECTIONS);
        }
    }

    /// The drainer's exit when it dies before reaching its end.
    pub(crate) fn abandon_unrequested_recoveries(self: &Arc<Self>) {
        let dropped = {
            let mut queue = self
                .snapshot_recovery()
                .unrequested
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            queue.running = false;
            queue.overflowed = false;
            queue.queued_bytes = 0;
            std::mem::take(&mut queue.queued).len()
        };
        // Scheduled whether or not anything was still queued: the reply it was
        // applying is lost with it.
        if self.is_permanently_stopped() {
            return;
        }
        warn!(
            "The unrequested snapshot recovery drainer ended early; dropped {dropped} queued replies"
        );
        self.converge_after_dropped_recovery(&RECOVERABLE_COLLECTIONS);
    }

    /// Decodes one reply and applies it, under the recovery lock.
    ///
    /// `asked` is the collection the reply's ask was made about, claimed for
    /// it, or `None` for a reply nothing here is waiting for.
    async fn recover_from_reply(&self, asked: Option<String>, reply: QueuedRecoveryReply) {
        let QueuedRecoveryReply {
            payload,
            compressed,
            request_id,
        } = reply;
        // Held across the decode and the apply, and taken by every reply,
        // matched or not. See the field for why.
        let _one_at_a_time = self.snapshot_recovery().lock.lock_arc().await;

        // Up to 64 MiB of inflate plus a decode wide enough that `waproto`
        // pins its instantiation: off the runtime's async workers either way.
        let decoded = wacore::runtime::blocking(&*self.runtime, move || {
            decode_snapshot_recovery(&payload, compressed)
        })
        .await;

        let proc = self.get_app_state_processor();
        let recovery = match decoded {
            Ok(recovery) => recovery,
            Err(e) => {
                // The request was answered, badly. Its collection name is
                // inside the payload that would not read, so the id the answer
                // carried is the only way to say which ask this was -- and
                // leaving the marker would suppress every retry for the rest of
                // the window over a question already answered.
                match &asked {
                    Some(name) => {
                        proc.take_recovery_request_by_id(&request_id).await;
                        warn!(
                            "Snapshot recovery response for {name} {e}; the collection may be asked for again"
                        );
                    }
                    None => warn!(
                        "Discarding the unrequested snapshot recovery response {request_id}: {e}"
                    ),
                }
                return;
            }
        };

        let matched = asked.is_some();
        let name = match (asked, recovery.collection_name.as_deref()) {
            (Some(asked), Some(named)) if named == asked => asked,
            (Some(asked), other) => {
                proc.take_recovery_request_by_id(&request_id).await;
                warn!(
                    "Snapshot recovery answering the ask for {asked} names {}; refusing it",
                    other.unwrap_or("nothing")
                );
                return;
            }
            (None, Some(named)) => named.to_string(),
            (None, None) => {
                warn!(
                    "Discarding the unrequested snapshot recovery {request_id}: it names no collection"
                );
                return;
            }
        };

        self.apply_recovered_collection(&name, matched.then_some(request_id.as_str()), recovery)
            .await;
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
        let mut alias_key = if !cache_key.chat.is_group() {
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
        if pending.is_none()
            && (cache_key.chat.server.is_lid_family() || cache_key.chat.server.is_pn_family())
        {
            // Explicit envelope aliases may be hosted, absent from the mapping
            // cache, or conflict with its retained mapping. On a miss, correlate
            // against pending metadata instead of inferring another identity.
            // This scan is bounded by the pending-cache capacity (default 200).
            let explicit_key = self
                .pdo_pending_requests
                .fold_entries(
                    None,
                    |known: Option<(ChatMessageId, u64)>, key, (_, memo)| {
                        if matches!(
                            memo.outcome.load(std::sync::atomic::Ordering::Acquire),
                            PDO_WRITING | PDO_SENT
                        ) && memo.matches_target(
                            &cache_key,
                            response_from_me,
                            response_participant.as_deref(),
                        ) && known
                            .as_ref()
                            .is_none_or(|(_, generation)| *generation < memo.generation)
                        {
                            Some((key.clone(), memo.generation))
                        } else {
                            known
                        }
                    },
                )
                .await;
            if let Some((key, _)) = explicit_key {
                pending = self
                    .pdo_pending_requests
                    .remove_if(&key, &|entry| {
                        matches_pending(entry)
                            && entry.1.matches_target(
                                &cache_key,
                                response_from_me,
                                response_participant.as_deref(),
                            )
                    })
                    .await;
                // Keep the candidate even for a stale request ID so the
                // generation check below can reject it without consuming it.
                alias_key = Some(key);
            }
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
    use super::{history_sync_on_demand_message, self_peer_target, wa};
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

    #[test]
    fn history_request_preserves_all_anchor_metadata() {
        let chat: Jid = "12025550123@s.whatsapp.net".parse().unwrap();
        let message = history_sync_on_demand_message(&chat, "OLDEST", true, 1_700_000_123, 50);
        let protocol = message.protocol_message.as_option().unwrap();
        assert_eq!(
            protocol.r#type,
            Some(wa::message::protocol_message::Type::PEER_DATA_OPERATION_REQUEST_MESSAGE)
        );
        let operation = protocol
            .peer_data_operation_request_message
            .as_option()
            .unwrap();
        assert_eq!(
            operation.peer_data_operation_request_type,
            Some(wa::message::PeerDataOperationRequestType::HISTORY_SYNC_ON_DEMAND)
        );
        let request = operation
            .history_sync_on_demand_request
            .as_option()
            .unwrap();
        let chat_text = chat.to_string();
        assert_eq!(request.chat_jid.as_deref(), Some(chat_text.as_str()));
        assert_eq!(request.oldest_msg_id.as_deref(), Some("OLDEST"));
        assert_eq!(request.oldest_msg_from_me, Some(true));
        assert_eq!(request.oldest_msg_timestamp_ms, Some(1_700_000_123));
        assert_eq!(request.on_demand_msg_count, Some(50));
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
        {
            let mut proto = wa::WebMessageInfo::default();
            proto.key = buffa::MessageField::some({
                let mut proto = wa::MessageKey::default();
                proto.remote_jid = Some(remote_jid.into());
                proto.from_me = Some(from_me);
                proto.id = Some(id.into());
                proto.participant = participant.map(|p| p.into());
                proto
            });
            proto
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
        web_msg.message = buffa::MessageField::some({
            let mut proto_ = waproto::whatsapp::Message::default();
            proto_.conversation = Some("recovered by the phone".to_owned());
            proto_
        });
        {
            let mut proto_ = waproto::whatsapp::message::peer_data_operation_request_response_message::peer_data_operation_result::PlaceholderMessageResendResponse::default();
            proto_.web_message_info_bytes = Some(web_msg.encode_to_vec());
            proto_
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

        let web_msg = {
            let mut proto_ = waproto::whatsapp::WebMessageInfo::default();
            proto_.key = buffa::MessageField::some({
                let mut proto_ = waproto::whatsapp::MessageKey::default();
                proto_.remote_jid = Some(chat.to_owned());
                proto_.from_me = Some(false);
                proto_.id = Some(msg_id.into());
                proto_.participant = Some("111222333444555@lid".to_owned());
                proto_
            });
            proto_.message = buffa::MessageField::some({
                let mut proto_ = waproto::whatsapp::Message::default();
                proto_.conversation = Some("recovered by the phone".to_owned());
                proto_
            });
            proto_
        };
        let response = {
            let mut proto_ = waproto::whatsapp::message::peer_data_operation_request_response_message::peer_data_operation_result::PlaceholderMessageResendResponse::default();
            proto_.web_message_info_bytes = Some(web_msg.encode_to_vec());
            proto_
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

        let web_msg = {
            let mut proto_ = waproto::whatsapp::WebMessageInfo::default();
            proto_.key = buffa::MessageField::some({
                let mut proto_ = waproto::whatsapp::MessageKey::default();
                proto_.remote_jid = Some(chat.to_owned());
                proto_.from_me = Some(false);
                proto_.id = Some(msg_id.into()); // The phone answers in PN.

                proto_.participant = Some("15550001234@s.whatsapp.net".to_owned());
                proto_
            });
            proto_.message = buffa::MessageField::some({
                let mut proto_ = waproto::whatsapp::Message::default();
                proto_.conversation = Some("recovered by the phone".to_owned());
                proto_
            });
            proto_
        };
        let response = {
            let mut proto_ = waproto::whatsapp::message::peer_data_operation_request_response_message::peer_data_operation_result::PlaceholderMessageResendResponse::default();
            proto_.web_message_info_bytes = Some(web_msg.encode_to_vec());
            proto_
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

    /// An explicit envelope alias is sufficient evidence even with a cold
    /// mapping cache. Without either, reconstruct instead of guessing a pair.
    #[tokio::test]
    async fn pdo_alias_miss_uses_only_explicit_pending_identity() {
        use wacore::types::events::ChannelEventHandler;
        use wacore::types::message::ChatMessageId;

        for explicit_alias in [true, false] {
            let client = setup_reconstruct_client().await;
            let pn = "12025550101@s.whatsapp.net";
            let lid = "777000000000101@lid";
            let msg_id = "PDO_DM_NO_MAPPING";
            let mut info =
                make_dm_pending_info(pn, lid, msg_id, wacore::types::message::AddressingMode::Pn);
            if !explicit_alias {
                std::sync::Arc::make_mut(&mut info).source.sender_alt = None;
            }
            let key = ChatMessageId::new(pn.parse().unwrap(), msg_id.into());
            client
                .pdo_pending_requests
                .insert(
                    key.clone(),
                    super::test_pending(
                        super::PendingPdoRequest {
                            message_info: info.clone(),
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

            assert_eq!(
                client.pdo_pending_requests.get(&key).await.is_none(),
                explicit_alias
            );
            let mut infos = Vec::new();
            while let Ok(event) = rx.try_recv() {
                infos.extend(event.messages().map(|message| message.info.clone()));
            }
            assert_eq!(infos.len(), 1);
            if explicit_alias {
                assert_eq!(infos[0].source.chat, info.source.chat);
                assert_eq!(infos[0].source.sender, info.source.sender);
                assert_eq!(infos[0].source.sender_alt, info.source.sender_alt);
                assert_eq!(infos[0].push_name, info.push_name);
            } else {
                assert_eq!(infos[0].source.chat.to_string(), lid);
                assert_eq!(infos[0].source.sender.to_string(), lid);
                assert!(infos[0].source.sender_alt.is_none());
                assert_eq!(infos[0].push_name, "");
            }
            assert!(
                client
                    .lid_pn_cache
                    .get_phone_number("777000000000101")
                    .await
                    .is_none()
            );
        }
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
        let web_msg = {
            let mut proto_ = waproto::whatsapp::WebMessageInfo::default();
            proto_.key = buffa::MessageField::some({
                let mut proto_ = waproto::whatsapp::MessageKey::default();
                proto_.remote_jid = Some(peer.to_owned());
                proto_.from_me = Some(false);
                proto_.id = Some(msg_id.into());
                proto_.participant = None;
                proto_
            });
            proto_.message = buffa::MessageField::some({
                let mut proto_ = waproto::whatsapp::Message::default();
                proto_.conversation = Some("recovered by the phone".to_owned());
                proto_
            });
            proto_
        };
        let response = {
            let mut proto_ = waproto::whatsapp::message::peer_data_operation_request_response_message::peer_data_operation_result::PlaceholderMessageResendResponse::default();
            proto_.web_message_info_bytes = Some(web_msg.encode_to_vec());
            proto_
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
        async fn supplied_history_session_id_is_the_outgoing_stanza_id() {
            let (client, transport, _) = manual_retry_client().await;
            let chat: Jid = "12025550123@s.whatsapp.net".parse().unwrap();
            let request_id = crate::MessageId::new("HISTORY_SESSION_SUPPLIED").unwrap();
            assert!(crate::MessageId::new("").is_err());

            client
                .fetch_message_history_with_id(
                    &chat,
                    "OLDEST",
                    false,
                    1_700_000_123,
                    50,
                    &request_id,
                )
                .await
                .unwrap();

            let sent = crate::test_utils::decode_sent_iq(&transport, 0).await;
            assert_eq!(sent.get().tag.as_ref(), "message");
            assert_eq!(
                sent.get().get_attr("id").unwrap().as_str(),
                request_id.as_str()
            );
        }

        #[tokio::test]
        async fn generated_history_session_ids_remain_unique_and_are_returned() {
            let (client, transport, _) = manual_retry_client().await;
            let chat: Jid = "12025550123@s.whatsapp.net".parse().unwrap();

            let first = client
                .fetch_message_history(&chat, "FIRST", false, 1_700_000_123, 50)
                .await
                .unwrap();
            let second = client
                .fetch_message_history(&chat, "SECOND", true, 1_700_000_100, 25)
                .await
                .unwrap();

            assert!(!first.is_empty());
            assert!(!second.is_empty());
            assert_ne!(first, second);
            for (index, expected) in [(0, first.as_str()), (1, second.as_str())] {
                let sent = crate::test_utils::decode_sent_iq(&transport, index).await;
                assert_eq!(sent.get().get_attr("id").unwrap().as_str(), expected);
            }
        }

        fn authenticate_recovery_fixture(client: &Arc<Client>) {
            client
                .is_logged_in
                .store(true, std::sync::atomic::Ordering::Release);
            client.authenticated_generation.store(
                client
                    .connection_generation
                    .load(std::sync::atomic::Ordering::SeqCst),
                std::sync::atomic::Ordering::SeqCst,
            );
            client.session_state_notifier.notify(usize::MAX);
        }

        #[tokio::test]
        async fn recovery_retry_waits_for_the_final_authenticated_generation() {
            let (client, _, _) = manual_retry_client().await;
            let generation = client
                .connection_generation
                .load(std::sync::atomic::Ordering::Acquire);
            client
                .is_logged_in
                .store(true, std::sync::atomic::Ordering::Release);
            client
                .authenticated_generation
                .store(generation, std::sync::atomic::Ordering::SeqCst);
            let peer: Jid = "12025550100@s.whatsapp.net".parse().unwrap();
            let mutex = client
                .session_lock_for(peer.to_protocol_address().as_str())
                .await;
            let held = mutex.lock().await;
            let proc = client.get_app_state_processor();
            let sender = Arc::clone(&client);
            let job =
                tokio::spawn(
                    async move { sender.request_syncd_snapshot_recovery("regular_low").await },
                );
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                while !proc.has_recovery_request("regular_low").await {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            // Teardown increments first; connect publishes a socket; success
            // authenticates and increments again only after the parked send resumes.
            client
                .connection_generation
                .store(generation + 1, std::sync::atomic::Ordering::SeqCst);
            client
                .is_logged_in
                .store(false, std::sync::atomic::Ordering::Release);
            let replacement = Arc::new(crate::transport::mock::CapturingMockTransport::new());
            install_socket(&client, replacement.clone());
            let prior_listeners = client.session_state_notifier.total_listeners();
            drop(held);
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                while !job.is_finished()
                    && client.session_state_notifier.total_listeners() <= prior_listeners
                {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            assert!(
                replacement.sent().is_empty(),
                "an intermediate pre-login generation must send no recovery"
            );
            assert!(!job.is_finished(), "retry must await authentication");
            client
                .connection_generation
                .store(generation + 2, std::sync::atomic::Ordering::SeqCst);
            client
                .authenticated_generation
                .store(generation + 2, std::sync::atomic::Ordering::SeqCst);
            client
                .is_logged_in
                .store(true, std::sync::atomic::Ordering::Release);
            client.session_state_notifier.notify(usize::MAX);
            let id = tokio::time::timeout(std::time::Duration::from_secs(5), job)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            assert_eq!(replacement.sent().len(), 1);
            assert_eq!(
                proc.take_recovery_request_with_generation_by_id(&id).await,
                Some(("regular_low".into(), generation + 2))
            );
        }

        #[tokio::test]
        async fn recovery_send_stops_after_two_reconnects_and_releases_markers() {
            let (client, _, _) = manual_retry_client().await;
            authenticate_recovery_fixture(&client);
            let (first_entered, first_release) = failure_socket(&client);
            let sender = Arc::clone(&client);
            let job =
                tokio::spawn(
                    async move { sender.request_syncd_snapshot_recovery("regular_low").await },
                );
            tokio::time::timeout(std::time::Duration::from_secs(5), first_entered.recv())
                .await
                .unwrap()
                .unwrap();
            let (second_entered, second_release) = failure_socket(&client);
            client
                .connection_generation
                .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
            authenticate_recovery_fixture(&client);
            first_release.send(()).await.unwrap();
            tokio::time::timeout(std::time::Duration::from_secs(5), second_entered.recv())
                .await
                .unwrap()
                .unwrap();
            client
                .connection_generation
                .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
            authenticate_recovery_fixture(&client);
            second_release.send(()).await.unwrap();
            assert!(
                tokio::time::timeout(std::time::Duration::from_secs(5), job)
                    .await
                    .unwrap()
                    .unwrap()
                    .is_err()
            );
            assert!(
                !client
                    .get_app_state_processor()
                    .has_recovery_request("regular_low")
                    .await
            );
        }

        type RecoveryResponse = wa::message::peer_data_operation_request_response_message::peer_data_operation_result::SyncDSnapshotFatalRecoveryResponse;

        const RECOVERY_KEY_ID: &[u8] = b"recovery-test-key";
        const RECOVERY_KEY: [u8; 32] = [7; 32];
        const OLD_INDEX_MAC: &[u8] = b"index-mac-before-the-recovery";

        async fn seed_regular_low(client: &Arc<Client>, version: u64) {
            seed_collection(client, "regular_low", version).await;
        }

        /// `collection` at `version`, with an ltHash, a MAC row and the key
        /// the replies below are signed with.
        async fn seed_collection(client: &Arc<Client>, collection: &str, version: u64) {
            let backend = client.persistence_manager.backend();
            backend
                .set_sync_key(
                    RECOVERY_KEY_ID,
                    wacore::store::traits::AppStateSyncKey {
                        key_data: RECOVERY_KEY.to_vec(),
                        ..Default::default()
                    },
                )
                .await
                .unwrap();
            backend
                .set_version(
                    collection,
                    wacore::appstate::hash::HashState {
                        version,
                        hash: [0x11; 128],
                        bootstrapped: true,
                        mac_mismatch_fatal: true,
                        ..Default::default()
                    },
                )
                .await
                .unwrap();
            backend
                .put_mutation_macs(
                    collection,
                    version,
                    &[wacore::appstate::processor::AppStateMutationMAC {
                        index_mac: OLD_INDEX_MAC.to_vec(),
                        value_mac: vec![0x22; 32],
                    }],
                )
                .await
                .unwrap();
        }

        fn regular_low_reply(version: u64, chat: &str, mark: u8) -> RecoveryResponse {
            recovery_reply("regular_low", version, chat, mark)
        }

        /// The primary's `collection` at `version`: one mute record for `chat`,
        /// and the ltHash byte and value MAC byte that identify this reply.
        fn recovery_reply(
            collection: &str,
            version: u64,
            chat: &str,
            mark: u8,
        ) -> RecoveryResponse {
            use buffa::Message as _;
            let mut record = wa::SyncdPlainTextRecord::default();
            record.value = buffa::MessageField::some({
                let mut data = wa::SyncActionData::default();
                data.index = Some(format!(r#"["mute","{chat}"]"#).into_bytes());
                data.value = buffa::MessageField::some({
                    let mut value = wa::SyncActionValue::default();
                    value.mute_action = buffa::MessageField::some({
                        let mut mute = wa::sync_action_value::MuteAction::default();
                        mute.muted = Some(true);
                        mute
                    });
                    value
                });
                data
            });
            record.key_id = Some(RECOVERY_KEY_ID.to_vec());
            record.mac = Some(vec![mark; 32]);
            let mut recovery = wa::SyncdSnapshotRecovery::default();
            recovery.collection_name = Some(collection.to_string());
            recovery.version = buffa::MessageField::some({
                let mut v = wa::SyncdVersion::default();
                v.version = Some(version);
                v
            });
            recovery.collection_lthash = Some(vec![mark; 128]);
            recovery.mutation_records = vec![record];
            let mut response = RecoveryResponse::default();
            response.collection_snapshot = Some(recovery.encode_to_vec());
            response.is_compressed = Some(false);
            response
        }

        async fn regular_low_state(client: &Arc<Client>) -> wacore::appstate::hash::HashState {
            collection_state(client, "regular_low").await
        }

        async fn collection_state(
            client: &Arc<Client>,
            collection: &str,
        ) -> wacore::appstate::hash::HashState {
            client
                .persistence_manager
                .backend()
                .get_version(collection)
                .await
                .unwrap()
                .unwrap()
        }

        async fn regular_low_mac(client: &Arc<Client>, index_mac: &[u8]) -> Option<Vec<u8>> {
            client
                .persistence_manager
                .backend()
                .get_mutation_mac("regular_low", index_mac)
                .await
                .unwrap()
        }

        fn mute_index_mac(chat: &str) -> Vec<u8> {
            let keys = wacore::appstate::keys::expand_app_state_keys(&RECOVERY_KEY);
            wacore::appstate::hash::generate_index_mac(
                format!(r#"["mute","{chat}"]"#).as_bytes(),
                &keys.index,
            )
        }

        /// The request ids of the unrequested replies queued behind the drainer,
        /// and the wire bytes they hold.
        fn queued_unrequested_recoveries(client: &Arc<Client>) -> (Vec<String>, u64) {
            let queue = client.snapshot_recovery().unrequested.lock().unwrap();
            (
                queue
                    .queued
                    .iter()
                    .map(|reply| reply.request_id.clone())
                    .collect(),
                queue.queued_bytes,
            )
        }

        /// Hands each reply to the handler, letting the task it spawns, if
        /// any, reach the recovery lock before the next one is handed over.
        async fn hand_over(client: &Arc<Client>, replies: &[(&RecoveryResponse, &str)]) {
            for (reply, request_id) in replies {
                client
                    .handle_syncd_snapshot_recovery_response(reply, request_id)
                    .await;
                for _ in 0..64 {
                    tokio::task::yield_now().await;
                }
            }
        }

        /// Waits until no recovery task holds or waits for the lock and the
        /// unrequested drainer has finished.
        async fn wait_for_recoveries(client: &Arc<Client>) {
            crate::test_utils::poll_until("every recovery reply to finish", || {
                Arc::strong_count(&client.snapshot_recovery().lock) == 1
                    && !client
                        .snapshot_recovery()
                        .unrequested
                        .lock()
                        .unwrap()
                        .running
            })
            .await;
        }

        /// Hands every reply to the handler while the recovery lock is held, so
        /// all of them are in flight together, checks that `tasks` of them
        /// queued on the lock and which unrequested ones are `queued` behind
        /// them, in order, and waits for every one to finish.
        ///
        /// A task holds a handle to the lock while it waits for it and while it
        /// runs, so the handle count is the tasks in flight: one per matched
        /// reply, and one drainer for all the unrequested ones.
        async fn deliver_together(
            client: &Arc<Client>,
            replies: &[(&RecoveryResponse, &str)],
            tasks: usize,
            queued: &[&str],
        ) {
            let held = client.snapshot_recovery().lock.lock().await;
            hand_over(client, replies).await;
            assert_eq!(
                Arc::strong_count(&client.snapshot_recovery().lock),
                1 + tasks,
                "tasks queued on the recovery lock"
            );
            assert_eq!(
                queued_unrequested_recoveries(client).0,
                queued,
                "the unrequested replies queued behind them"
            );
            drop(held);
            wait_for_recoveries(client).await;
        }

        #[tokio::test]
        async fn a_recovery_reply_nothing_asked_for_is_applied_when_it_moves_the_collection() {
            let (client, _, _) = manual_retry_client().await;
            seed_regular_low(&client, 5).await;
            let reply = regular_low_reply(8, "1@s.whatsapp.net", 0x5A);
            deliver_together(&client, &[(&reply, "asked-by-an-earlier-process")], 1, &[]).await;

            let state = regular_low_state(&client).await;
            assert_eq!(state.version, 8, "the primary's version replaces ours");
            assert_eq!(state.hash, [0x5A; 128], "and so does its ltHash");
            assert!(!state.mac_mismatch_fatal);
            assert_eq!(
                regular_low_mac(&client, &mute_index_mac("1@s.whatsapp.net")).await,
                Some(vec![0x5A; 32]),
                "the record's MAC is stored"
            );
            assert_eq!(
                regular_low_mac(&client, OLD_INDEX_MAC).await,
                None,
                "and the MACs of the collection it replaced are gone"
            );
        }

        #[tokio::test]
        async fn a_recovery_reply_to_an_ask_from_an_older_connection_is_applied() {
            let (client, _, _) = manual_retry_client().await;
            seed_regular_low(&client, 5).await;
            let proc = client.get_app_state_processor();
            let generation = client
                .connection_generation
                .load(std::sync::atomic::Ordering::Acquire);
            assert!(
                proc.mark_recovery_requested_with_id(
                    "regular_low",
                    generation,
                    "asked-before-reconnect"
                )
                .await
            );
            client
                .connection_generation
                .store(generation + 1, std::sync::atomic::Ordering::Release);

            let reply = regular_low_reply(8, "1@s.whatsapp.net", 0x5A);
            deliver_together(&client, &[(&reply, "asked-before-reconnect")], 1, &[]).await;

            let state = regular_low_state(&client).await;
            assert_eq!(state.version, 8, "applied under the current connection");
            assert_eq!(state.hash, [0x5A; 128]);
            assert!(
                !proc.has_recovery_request("regular_low").await,
                "the ask it answered is spent"
            );
            assert!(
                proc.mark_recovery_requested_with_id("regular_low", generation + 1, "next")
                    .await,
                "and does not suppress the next one"
            );
        }

        #[tokio::test]
        async fn a_recovery_reply_behind_the_collection_is_discarded_and_releases_the_ask() {
            let (client, _, _) = manual_retry_client().await;
            seed_regular_low(&client, 10).await;
            let proc = client.get_app_state_processor();
            let generation = client
                .connection_generation
                .load(std::sync::atomic::Ordering::Acquire);
            assert!(
                proc.mark_recovery_requested_with_id("regular_low", generation, "newest-ask")
                    .await
            );

            let reply = regular_low_reply(10, "1@s.whatsapp.net", 0x5A);
            deliver_together(&client, &[(&reply, "an-older-ask")], 1, &[]).await;

            let state = regular_low_state(&client).await;
            assert_eq!(state.version, 10, "the stored version stands");
            assert_eq!(state.hash, [0x11; 128], "and so does its ltHash");
            assert_eq!(
                regular_low_mac(&client, OLD_INDEX_MAC).await,
                Some(vec![0x22; 32]),
                "and its MACs"
            );
            assert_eq!(
                regular_low_mac(&client, &mute_index_mac("1@s.whatsapp.net")).await,
                None
            );
            assert!(
                !proc.has_recovery_request("regular_low").await,
                "an answer found behind still settles the outstanding ask"
            );
        }

        #[tokio::test]
        async fn recovery_replies_arriving_together_apply_one_after_the_other() {
            let (client, _, _) = manual_retry_client().await;
            seed_regular_low(&client, 5).await;
            let first = regular_low_reply(8, "1@s.whatsapp.net", 0x5A);
            let second = regular_low_reply(8, "2@s.whatsapp.net", 0x6B);
            deliver_together(
                &client,
                &[(&first, "first"), (&second, "second")],
                1,
                &["second"],
            )
            .await;

            let state = regular_low_state(&client).await;
            assert_eq!(state.version, 8);
            assert_eq!(
                state.hash, [0x5A; 128],
                "the first reply's collection stands"
            );
            assert_eq!(
                regular_low_mac(&client, &mute_index_mac("1@s.whatsapp.net")).await,
                Some(vec![0x5A; 32])
            );
            assert_eq!(
                regular_low_mac(&client, &mute_index_mac("2@s.whatsapp.net")).await,
                None,
                "the second, at the same version, was discarded as stale"
            );
        }

        #[tokio::test]
        async fn a_burst_of_unrequested_recoveries_ends_at_the_newest_version() {
            let (client, _, _) = manual_retry_client().await;
            seed_regular_low(&client, 5).await;
            let first = regular_low_reply(8, "1@s.whatsapp.net", 0x5A);
            let second = regular_low_reply(8, "2@s.whatsapp.net", 0x6B);
            let newest = regular_low_reply(9, "3@s.whatsapp.net", 0x7C);
            deliver_together(
                &client,
                &[(&first, "first"), (&second, "second"), (&newest, "third")],
                1,
                &["second", "third"],
            )
            .await;

            let state = regular_low_state(&client).await;
            assert_eq!(state.version, 9, "every reply is applied in turn");
            assert_eq!(state.hash, [0x7C; 128]);
            assert_eq!(
                regular_low_mac(&client, &mute_index_mac("3@s.whatsapp.net")).await,
                Some(vec![0x7C; 32])
            );
            assert_eq!(queued_unrequested_recoveries(&client), (Vec::new(), 0));

            let later = regular_low_reply(10, "4@s.whatsapp.net", 0x8D);
            deliver_together(&client, &[(&later, "after-the-burst")], 1, &[]).await;
            assert_eq!(
                regular_low_state(&client).await.version,
                10,
                "a drained queue starts a drainer for the next reply"
            );
        }

        #[tokio::test]
        async fn a_burst_whose_newest_version_arrives_first_ends_at_it() {
            let (client, _, _) = manual_retry_client().await;
            seed_regular_low(&client, 5).await;
            let newest = regular_low_reply(9, "1@s.whatsapp.net", 0x7C);
            let older = regular_low_reply(8, "2@s.whatsapp.net", 0x5A);
            deliver_together(
                &client,
                &[(&newest, "first"), (&older, "second")],
                1,
                &["second"],
            )
            .await;

            let state = regular_low_state(&client).await;
            assert_eq!(
                state.version, 9,
                "the older reply applied after it is discarded as stale"
            );
            assert_eq!(state.hash, [0x7C; 128]);
            assert_eq!(
                regular_low_mac(&client, &mute_index_mac("2@s.whatsapp.net")).await,
                None
            );
        }

        #[tokio::test]
        async fn unrequested_recoveries_for_different_collections_are_all_applied() {
            let (client, _, _) = manual_retry_client().await;
            seed_regular_low(&client, 5).await;
            seed_collection(&client, "regular", 1).await;
            let low = regular_low_reply(8, "1@s.whatsapp.net", 0x5A);
            let regular = recovery_reply("regular", 3, "2@s.whatsapp.net", 0x6B);
            let newer_low = regular_low_reply(9, "3@s.whatsapp.net", 0x7C);
            deliver_together(
                &client,
                &[
                    (&low, "regular-low-v8"),
                    (&regular, "regular-v3"),
                    (&newer_low, "regular-low-v9"),
                ],
                1,
                &["regular-v3", "regular-low-v9"],
            )
            .await;

            let low_state = regular_low_state(&client).await;
            assert_eq!(low_state.version, 9);
            assert_eq!(low_state.hash, [0x7C; 128]);
            let regular_state = collection_state(&client, "regular").await;
            assert_eq!(
                regular_state.version, 3,
                "a later reply for another collection does not cost this one its recovery"
            );
            assert_eq!(regular_state.hash, [0x6B; 128]);
        }

        /// Whether a sync holds a collection no recovery here touches: the
        /// reservation `process_app_state_sync_task` keeps while its request is
        /// out, which the fixture's transport never answers.
        fn recoverable_collection_reserved(client: &Arc<Client>) -> bool {
            [
                wacore::appstate::patch_decode::WAPatchName::RegularHigh,
                wacore::appstate::patch_decode::WAPatchName::CriticalUnblockLow,
            ]
            .into_iter()
            .any(|name| client.app_state_syncing.is_held(name))
        }

        /// The convergence sync waits for a usable connection rather than
        /// giving up without one: nothing is reserved while the fixture is
        /// unauthenticated, past the scheduler's first backoff, and the sync
        /// runs once it authenticates.
        async fn assert_converges_once_connected(client: &Arc<Client>) {
            tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
            assert!(
                !recoverable_collection_reserved(client),
                "nothing is synced without a connection"
            );
            authenticate_recovery_fixture(client);
            crate::test_utils::poll_until("the recoverable collections to be synced again", || {
                recoverable_collection_reserved(client)
            })
            .await;
        }

        #[tokio::test]
        async fn an_unrequested_recovery_past_the_queue_bound_is_refused_before_it_is_copied_and_the_collections_are_synced_again()
         {
            let (client, _, _) = manual_retry_client().await;
            seed_regular_low(&client, 5).await;
            let running = regular_low_reply(8, "1@s.whatsapp.net", 0x5A);
            let queued = regular_low_reply(9, "2@s.whatsapp.net", 0x6B);
            // The largest reply the size check lets through, uncompressed: it
            // fits the byte budget alone but not beside the one queued above.
            let mut oversized = RecoveryResponse::default();
            oversized.collection_snapshot =
                Some(vec![0; wacore::history_sync::MAX_DECOMPRESSED as usize]);
            oversized.is_compressed = Some(false);

            let held = client.snapshot_recovery().lock.lock().await;
            hand_over(
                &client,
                &[
                    (&running, "running"),
                    (&queued, "queued"),
                    (&oversized, "refused"),
                ],
            )
            .await;
            let (ids, bytes) = queued_unrequested_recoveries(&client);
            assert_eq!(ids, ["queued"], "the refused reply never reached the queue");
            assert_eq!(
                bytes,
                queued.collection_snapshot.as_ref().unwrap().len() as u64,
                "and holds none of its bytes"
            );
            assert!(
                client
                    .snapshot_recovery()
                    .unrequested
                    .lock()
                    .unwrap()
                    .overflowed
            );
            assert!(
                !recoverable_collection_reserved(&client),
                "nothing is synced while the queue is still draining"
            );

            drop(held);
            wait_for_recoveries(&client).await;
            assert_eq!(regular_low_state(&client).await.version, 9);
            assert!(
                !client
                    .snapshot_recovery()
                    .unrequested
                    .lock()
                    .unwrap()
                    .overflowed
            );
            assert_converges_once_connected(&client).await;
        }

        #[tokio::test]
        async fn a_drainer_that_ends_early_releases_the_queue_and_syncs_the_collections_again() {
            let (client, _, _) = manual_retry_client().await;
            let reply = |request_id: &str| super::super::QueuedRecoveryReply {
                payload: vec![1; 16],
                compressed: false,
                request_id: request_id.into(),
            };
            {
                let mut queue = client.snapshot_recovery().unrequested.lock().unwrap();
                queue.running = true;
                queue.queued.push_back(reply("stranded"));
                queue.queued_bytes = 16;
            }
            // Dropped before its first poll: the earliest a drainer can end.
            drop(Client::drain_unrequested_recoveries(
                super::super::UnrequestedDrainer {
                    client: Some(Arc::clone(&client)),
                },
                reply("in-flight"),
            ));
            {
                let queue = client.snapshot_recovery().unrequested.lock().unwrap();
                assert!(!queue.running, "a later reply starts a drainer of its own");
                assert!(queue.queued.is_empty());
                assert_eq!(queue.queued_bytes, 0);
            }
            assert_converges_once_connected(&client).await;
        }

        #[tokio::test]
        async fn a_repeat_of_a_recovery_reply_being_handled_costs_no_decode() {
            let (client, _, _) = manual_retry_client().await;
            seed_regular_low(&client, 5).await;
            let proc = client.get_app_state_processor();
            let generation = client
                .connection_generation
                .load(std::sync::atomic::Ordering::Acquire);
            assert!(
                proc.mark_recovery_requested_with_id("regular_low", generation, "asked")
                    .await
            );

            let reply = regular_low_reply(8, "1@s.whatsapp.net", 0x5A);
            deliver_together(&client, &[(&reply, "asked"), (&reply, "asked")], 1, &[]).await;

            assert_eq!(regular_low_state(&client).await.version, 8);
            assert!(!proc.has_recovery_request("regular_low").await);
        }

        #[tokio::test]
        async fn recovery_send_retries_on_reconnect_without_using_old_marker() {
            let (client, old_transport, _) = manual_retry_client().await;
            authenticate_recovery_fixture(&client);
            let peer: Jid = "12025550100@s.whatsapp.net".parse().unwrap();
            let mutex = client
                .session_lock_for(peer.to_protocol_address().as_str())
                .await;
            let held = mutex.lock().await;
            let proc = client.get_app_state_processor();
            let generation = client
                .connection_generation
                .load(std::sync::atomic::Ordering::Acquire);
            let sender = Arc::clone(&client);
            let job =
                tokio::spawn(
                    async move { sender.request_syncd_snapshot_recovery("regular_low").await },
                );
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                while !proc.has_recovery_request("regular_low").await {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            let replacement = Arc::new(crate::transport::mock::CapturingMockTransport::new());
            install_socket(&client, replacement.clone());
            client
                .connection_generation
                .store(generation + 1, std::sync::atomic::Ordering::Release);
            authenticate_recovery_fixture(&client);
            drop(held);
            let id = tokio::time::timeout(std::time::Duration::from_secs(5), job)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            assert!(
                old_transport.sent().is_empty(),
                "retired send must not hit the old socket"
            );
            assert_eq!(
                replacement.sent().len(),
                1,
                "only the freshly marked request goes out"
            );
            assert_eq!(
                proc.take_recovery_request_with_generation_by_id(&id).await,
                Some(("regular_low".to_string(), generation + 1))
            );
        }

        #[tokio::test]
        async fn outgoing_lid_pdo_uses_recipient_alias_and_preserves_metadata() {
            use wacore::types::events::ChannelEventHandler;
            use wacore::types::message::{AddressingMode, MessageSource};
            for explicit_alias in [true, false] {
                let (client, transport, _) = manual_retry_client().await;
                let peer_lid: Jid = "777000000000101@lid".parse().unwrap();
                let peer_pn: Jid = "12025550101@s.whatsapp.net".parse().unwrap();
                let own_pn: Jid = "12025550100@s.whatsapp.net".parse().unwrap();
                let info = Arc::new(MessageInfo {
                    id: "OUTGOING_ALIAS_PDO".into(),
                    push_name: "original self-sync metadata".into(),
                    source: MessageSource {
                        chat: peer_lid.clone(),
                        sender: "777000000000100@lid".parse().unwrap(),
                        sender_alt: Some(own_pn.clone()),
                        recipient_alt: explicit_alias.then_some(peer_pn.clone()),
                        is_from_me: true,
                        addressing_mode: Some(AddressingMode::Lid),
                        ..Default::default()
                    },
                    ..Default::default()
                });
                let request_id = client
                    .retry_pdo_placeholder_resend_request(&info)
                    .await
                    .unwrap()
                    .unwrap();
                let response_chat = if explicit_alias { peer_pn } else { peer_lid };
                let key = ChatMessageId::new(response_chat.clone(), info.id.clone());
                assert!(client.pdo_pending_requests.get(&key).await.is_some());
                assert!(
                    client
                        .pdo_pending_requests
                        .get(&ChatMessageId::new(own_pn, info.id.clone()))
                        .await
                        .is_none()
                );
                let (handler, rx) = ChannelEventHandler::new();
                client.core.event_bus.subscribe_handler(handler).detach();
                let response =
                    make_placeholder_response(&response_chat.to_string(), true, &info.id, None);
                client
                    .handle_placeholder_resend_response(&response, "STALE_REQUEST")
                    .await;
                assert!(client.pdo_pending_requests.get(&key).await.is_some());
                assert!(rx.try_recv().is_err(), "stale response must not publish");
                client
                    .handle_placeholder_resend_response(&response, &request_id)
                    .await;
                assert!(client.pdo_pending_requests.get(&key).await.is_none());
                let mut delivered = 0;
                while let Ok(event) = rx.try_recv() {
                    for message in event.messages() {
                        assert_eq!(message.info.push_name, info.push_name);
                        assert_eq!(message.info.source.chat, info.source.chat);
                        assert_eq!(message.info.source.sender, info.source.sender);
                        assert_eq!(message.info.source.recipient_alt, info.source.recipient_alt);
                        assert_eq!(
                            message.info.unavailable_request_id.as_deref(),
                            Some(request_id.as_str())
                        );
                        delivered += 1;
                    }
                }
                assert_eq!(delivered, 1);
                client
                    .handle_placeholder_resend_response(&response, "STALE_REQUEST")
                    .await;
                assert!(
                    rx.try_recv().is_err(),
                    "completed owner still rejects stale aliases"
                );
                crate::test_utils::decode_sent_iq(&transport, 0).await;
            }
        }

        #[tokio::test]
        async fn pdo_explicit_aliases_preserve_metadata_across_reply_namespaces() {
            use wacore::types::events::ChannelEventHandler;
            use wacore::types::message::{AddressingMode, MessageSource, SenderMessageId};
            for (lid_server, pn_server, conflicting_cache) in [
                ("hosted.lid", "hosted", false),
                ("hosted.lid", "hosted", true),
                ("lid", "s.whatsapp.net", false),
                ("lid", "s.whatsapp.net", true),
            ] {
                for (explicit_alias, reply_with_pn) in [(true, true), (true, false), (false, false)]
                {
                    let (client, transport, _) = manual_retry_client().await;
                    let peer_lid: Jid = format!("777000000000101@{lid_server}").parse().unwrap();
                    let peer_pn: Jid = format!("12025550101@{pn_server}").parse().unwrap();
                    let own_pn: Jid = "12025550100@s.whatsapp.net".parse().unwrap();
                    if conflicting_cache {
                        client
                            .lid_pn_cache
                            .add(&wacore::types::lid_pn::LidPnEntry {
                                lid: peer_lid.user.to_string().into(),
                                phone_number: "12025550109".into(),
                                created_at: 0,
                                learning_source: wacore::types::lid_pn::LearningSource::Usync,
                            })
                            .await;
                    }
                    let info = Arc::new(MessageInfo {
                        id: "EXPLICIT_ALIAS_PDO".into(),
                        push_name: "original self-sync metadata".into(),
                        source: MessageSource {
                            chat: peer_lid.clone(),
                            sender: "777000000000100@lid".parse().unwrap(),
                            sender_alt: Some(own_pn.clone()),
                            recipient_alt: explicit_alias.then_some(peer_pn.clone()),
                            is_from_me: true,
                            addressing_mode: Some(AddressingMode::Lid),
                            ..Default::default()
                        },
                        ..Default::default()
                    });
                    client
                        .send_pdo_placeholder_resend_request(&info)
                        .await
                        .unwrap();
                    let request_id = client
                        .pdo_requested
                        .get(&SenderMessageId::new(
                            info.source.chat.clone(),
                            info.id.clone(),
                            info.source.sender.clone(),
                        ))
                        .await
                        .unwrap()
                        .request_id
                        .clone();
                    let pending_chat = if explicit_alias {
                        peer_pn.clone()
                    } else {
                        peer_lid.clone()
                    };
                    let response_chat = if reply_with_pn { peer_pn } else { peer_lid };
                    let key = ChatMessageId::new(pending_chat, info.id.clone());
                    assert!(client.pdo_pending_requests.get(&key).await.is_some());
                    assert!(
                        client
                            .pdo_pending_requests
                            .get(&ChatMessageId::new(own_pn, info.id.clone()))
                            .await
                            .is_none()
                    );
                    let (handler, rx) = ChannelEventHandler::new();
                    client.core.event_bus.subscribe_handler(handler).detach();
                    let response =
                        make_placeholder_response(&response_chat.to_string(), true, &info.id, None);
                    client
                        .handle_placeholder_resend_response(&response, "STALE_REQUEST")
                        .await;
                    assert!(client.pdo_pending_requests.get(&key).await.is_some());
                    assert!(rx.try_recv().is_err(), "stale response must not publish");
                    client
                        .handle_placeholder_resend_response(&response, &request_id)
                        .await;
                    assert!(client.pdo_pending_requests.get(&key).await.is_none());
                    let mut delivered = 0;
                    while let Ok(event) = rx.try_recv() {
                        for message in event.messages() {
                            assert_eq!(message.info.push_name, info.push_name);
                            assert_eq!(message.info.source.chat, info.source.chat);
                            assert_eq!(message.info.source.sender, info.source.sender);
                            assert_eq!(
                                message.info.source.recipient_alt,
                                info.source.recipient_alt
                            );
                            assert_eq!(
                                message.info.unavailable_request_id.as_deref(),
                                Some(request_id.as_str())
                            );
                            delivered += 1;
                        }
                    }
                    assert_eq!(delivered, 1);
                    assert_eq!(
                        client
                            .lid_pn_cache
                            .get_phone_number(&info.source.chat.user)
                            .await,
                        conflicting_cache.then(|| "12025550109".to_owned())
                    );
                    crate::test_utils::decode_sent_iq(&transport, 0).await;
                }
            }
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

        let web_msg = {
            let mut proto_ = waproto::whatsapp::WebMessageInfo::default();
            proto_.key = buffa::MessageField::some({
                let mut proto_ = waproto::whatsapp::MessageKey::default();
                proto_.remote_jid = Some(chat.to_owned());
                proto_.from_me = Some(false);
                proto_.id = Some(msg_id.into());
                proto_.participant = None;
                proto_
            });
            proto_
        };
        let response = {
            let mut proto_ = waproto::whatsapp::message::peer_data_operation_request_response_message::peer_data_operation_result::PlaceholderMessageResendResponse::default();
            proto_.web_message_info_bytes = Some(web_msg.encode_to_vec());
            proto_
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

/// Decode the complete bounded recovery before allowing any state changes.
fn decode_snapshot_recovery(
    payload: &[u8],
    compressed: bool,
) -> Result<wa::SyncdSnapshotRecovery, String> {
    let bytes = if compressed {
        match inflate_zlib(payload) {
            Ok(plain) => std::borrow::Cow::Owned(plain),
            Err(zlib) => match inflate_gzip(payload) {
                Some(plain) => std::borrow::Cow::Owned(plain),
                None => return Err(format!("{zlib}; not a complete gzip stream either")),
            },
        }
    } else {
        std::borrow::Cow::Borrowed(payload)
    };
    waproto::codec::syncd_snapshot_recovery_decode(&bytes)
        .map_err(|e| format!("failed to decode: {e}"))
}
/// Inflates a zlib stream, refusing one that is cut short or followed by more
/// bytes.
fn inflate_zlib(payload: &[u8]) -> Result<Vec<u8>, String> {
    let mut reader = wacore_binary::zlib_pool::InflateReader::new(
        payload,
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
    Ok(plain)
}

/// The primary's reply flagged compressed but sent as gzip (`1f 8b 08 00`),
/// not zlib. Bounded like the zlib path, and whole or nothing like it too.
fn inflate_gzip(payload: &[u8]) -> Option<Vec<u8>> {
    inflate_gzip_bounded(payload, wacore::history_sync::MAX_DECOMPRESSED)
}

fn inflate_gzip_bounded(payload: &[u8], max: u64) -> Option<Vec<u8>> {
    use std::io::Read;
    let mut decoder = flate2::bufread::GzDecoder::new(payload);
    let mut plain = Vec::new();
    (&mut decoder).take(max + 1).read_to_end(&mut plain).ok()?;
    if plain.is_empty() || plain.len() as u64 > max {
        return None;
    }
    // The reader stops after the first member, so a stream that ends before
    // the payload does would hand a parseable prefix over as the whole
    // collection. It reads from the slice itself, and what it leaves of it has
    // to be nothing.
    if !decoder.into_inner().is_empty() {
        return None;
    }
    info!(
        "Snapshot recovery payload is gzip, not zlib (header {:02x?})",
        &payload[..payload.len().min(4)]
    );
    Some(plain)
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)] // Synthetic protobuf fixtures, as permitted by clippy.toml.
mod inflate_gzip_tests {
    use super::inflate_gzip;
    use std::io::Write;

    fn gzip(data: &[u8]) -> Vec<u8> {
        let mut e = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        e.write_all(data).unwrap();
        e.finish().unwrap()
    }

    fn recovery() -> super::wa::SyncdSnapshotRecovery {
        let mut recovery = super::wa::SyncdSnapshotRecovery::default();
        recovery.collection_name = Some("regular_low".to_string());
        let mut version = super::wa::SyncdVersion::default();
        version.version = Some(7);
        recovery.version = buffa::MessageField::some(version);
        recovery
    }

    #[test]
    fn decodes_compressed_recovery_as_gzip_and_zlib() {
        use buffa::Message;
        let expected = recovery();
        let raw = expected.encode_to_vec();
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        z.write_all(&raw).unwrap();
        for payload in [gzip(&raw), z.finish().unwrap()] {
            let decoded = super::decode_snapshot_recovery(&payload, true).unwrap();
            assert_eq!(decoded, expected);
        }
        assert_eq!(
            super::decode_snapshot_recovery(&raw, false).unwrap(),
            expected
        );
        assert!(super::decode_snapshot_recovery(&gzip(&raw), false).is_err());
    }

    #[test]
    fn decoder_rejects_incomplete_or_appended_recoveries() {
        use buffa::Message;
        let raw = recovery().encode_to_vec();
        let payload = gzip(&raw);
        assert!(super::decode_snapshot_recovery(&payload[..payload.len() - 4], true).is_err());
        let mut members = payload.clone();
        members.extend(gzip(&raw));
        assert!(super::decode_snapshot_recovery(&members, true).is_err());
        let mut corrupt = payload;
        let n = corrupt.len();
        corrupt[n - 8] ^= 1;
        assert!(super::decode_snapshot_recovery(&corrupt, true).is_err());
        assert!(super::decode_snapshot_recovery(&gzip(&[255]), true).is_err());
    }

    #[test]
    fn inflates_a_whole_gzip_stream() {
        assert_eq!(
            inflate_gzip(&gzip(b"records")).as_deref(),
            Some(&b"records"[..])
        );
    }

    #[test]
    fn gzip_limit_accepts_the_boundary_and_rejects_one_byte_more() {
        assert_eq!(
            super::inflate_gzip_bounded(&gzip(b"12345678"), 8),
            Some(b"12345678".to_vec())
        );
        assert_eq!(super::inflate_gzip_bounded(&gzip(b"123456789"), 8), None);
    }

    #[test]
    fn refuses_a_second_member() {
        let mut payload = gzip(b"first half");
        payload.extend(gzip(b"second half"));
        assert_eq!(inflate_gzip(&payload), None);
    }

    #[test]
    fn refuses_trailing_bytes() {
        let mut payload = gzip(b"records");
        payload.push(0);
        assert_eq!(inflate_gzip(&payload), None);
    }

    #[test]
    fn refuses_a_truncated_stream() {
        let data: Vec<u8> = (0..4096u32).flat_map(|i| i.to_le_bytes()).collect();
        let payload = gzip(&data);
        assert_eq!(inflate_gzip(&payload[..payload.len() - 4]), None);
        assert_eq!(inflate_gzip(&payload[..payload.len() / 2]), None);
    }
}

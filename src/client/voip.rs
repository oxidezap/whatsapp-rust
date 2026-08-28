//! Call-control accessor. Reject/terminate are always available since their stanza builders live in
//! core; the high-level call/accept flows, including their signaling, need the `voip` feature.

#[cfg(feature = "voip-runtime")]
use std::mem::size_of;
#[cfg(feature = "voip-runtime")]
use std::sync::Arc;
#[cfg(feature = "voip-runtime")]
use std::time::Duration;

#[cfg(feature = "voip-runtime")]
use log::warn;
#[cfg(feature = "voip-runtime")]
use wacore::stanza::call::build_mute_v2;
use wacore::stanza::call::{TerminateParams, build_reject, build_terminate};
#[cfg(feature = "voip-runtime")]
use wacore::stanza::group_call::{
    build_active_group_accept, build_active_group_preaccept, build_call_link_create,
    build_call_link_join_with_capability, build_call_link_query, build_raise_hand,
    build_screen_share, build_waiting_room_admit, build_waiting_room_deny,
    build_waiting_room_heartbeat, build_waiting_room_toggle, parse_call_link_create_ack,
    parse_call_link_join_ack, parse_call_link_join_call_id, parse_call_link_query_ack,
    parse_waiting_room_admit_ack, parse_waiting_room_deny_ack, parse_waiting_room_toggle_ack,
};
use wacore::types::call::IncomingCall;
#[cfg(feature = "voip-runtime")]
use wacore::types::call::{CallAction, VideoState};
#[cfg(feature = "voip-runtime")]
use wacore::types::group_call::{
    CallLink, CallLinkJoin, CallLinkMedia, CallLinkPreview, GroupCallUpdate, ScreenShare,
    ScreenShareState, WaitingRoom,
};
#[cfg(feature = "voip-runtime")]
use wacore::voip::{AudioFormat, CallEvent, CallPhase, CallSession, VideoControl};
use wacore_binary::Jid;
#[cfg(feature = "voip-runtime")]
use wacore_binary::Node;
#[cfg(feature = "voip-runtime")]
use wacore_binary::Server;
#[cfg(feature = "voip-runtime")]
use zeroize::Zeroizing;

#[cfg(feature = "voip-runtime")]
use super::ResponseWaiter;
use super::{Client, ClientError};

#[cfg(feature = "voip-runtime")]
const CALL_SERVICE_REQUEST_TIMEOUT: Duration = Duration::from_secs(10);
#[cfg(feature = "voip-runtime")]
const WAITING_ROOM_HEARTBEAT_INTERVAL: Duration = Duration::from_secs(10);
#[cfg(feature = "voip-runtime")]
const WAITING_ROOM_HEARTBEAT_MAX_CONSECUTIVE_FAILURES: u8 = 3;
#[cfg(feature = "voip-runtime")]
const MAX_PENDING_CALL_LINK_TRANSITIONS: usize = 32;
#[cfg(feature = "voip-runtime")]
const MAX_PENDING_CALL_LINK_TRANSITION_BYTES: usize = 1024 * 1024;
#[cfg(feature = "voip-runtime")]
const MAX_PENDING_CALL_LINK_SATURATION_FINGERPRINTS: usize = 32;

/// Opaque call-control handle obtained via [`Client::voip`]. Borrows the client;
/// kept as a newtype so the surface can grow without breaking callers.
pub struct Voip<'a> {
    client: &'a Client,
}

#[cfg(feature = "voip-runtime")]
struct CallLinkRegistrationGuard {
    client: std::sync::Weak<Client>,
    registry: Arc<wacore::voip::CallRegistry>,
    call_id: String,
    call_creator: Jid,
    generation: u64,
    armed: bool,
}

#[cfg(feature = "voip-runtime")]
pub(crate) struct CallLinkJoinRegistration {
    pub(crate) join: CallLinkJoin,
    pub(crate) generation: u64,
}

#[cfg(feature = "voip-runtime")]
#[derive(Clone, Copy)]
enum WaitingRoomUserAction {
    Admit,
    Deny,
}

#[cfg(feature = "voip-runtime")]
enum PendingCallLinkTransition {
    Group(Box<GroupCallUpdate>),
    WaitingRoom(WaitingRoom),
    RawEpoch {
        call_creator: Jid,
        sender: Jid,
        transaction_id: u32,
        raw_epoch: Zeroizing<Vec<u8>>,
    },
    Terminated {
        call_creator: Jid,
        sender: Jid,
    },
    Saturated,
}

#[cfg(feature = "voip-runtime")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PendingCallLinkBuffer {
    NotPending,
    Buffered,
    Saturated,
}

#[cfg(feature = "voip-runtime")]
impl PendingCallLinkBuffer {
    pub(crate) fn suppresses_dispatch(self) -> bool {
        self != Self::NotPending
    }
}

#[cfg(feature = "voip-runtime")]
impl PendingCallLinkTransition {
    fn group_heap_bytes(update: &GroupCallUpdate) -> usize {
        use wacore::stats::HeapSize;

        size_of::<GroupCallUpdate>() + update.heap_bytes()
    }

    fn waiting_room_heap_bytes(room: &WaitingRoom) -> usize {
        use wacore::stats::HeapSize;

        room.heap_bytes()
    }

    fn heap_bytes(&self) -> usize {
        use wacore::stats::HeapSize;

        match self {
            Self::Group(update) => Self::group_heap_bytes(update),
            Self::WaitingRoom(room) => Self::waiting_room_heap_bytes(room),
            Self::RawEpoch {
                call_creator,
                sender,
                raw_epoch,
                ..
            } => call_creator
                .heap_bytes()
                .saturating_add(sender.heap_bytes())
                .saturating_add(raw_epoch.capacity()),
            Self::Terminated {
                call_creator,
                sender,
            } => call_creator
                .heap_bytes()
                .saturating_add(sender.heap_bytes()),
            Self::Saturated => 0,
        }
    }
}

#[cfg(feature = "voip-runtime")]
#[derive(Default)]
pub(crate) struct PendingCallLinkJoins {
    active: usize,
    bound_call_id: Option<String>,
    transitions: std::collections::HashMap<String, Vec<PendingCallLinkTransition>>,
    saturation_fingerprints: Vec<u64>,
    saturation_hash_builder: std::collections::hash_map::RandomState,
    untracked_saturation: bool,
}

#[cfg(feature = "voip-runtime")]
impl PendingCallLinkJoins {
    fn accepts(&self, call_id: &str) -> bool {
        self.bound_call_id
            .as_deref()
            .is_none_or(|bound| bound == call_id)
    }

    fn can_buffer_transition(&self, call_id: &str, payload_bytes: usize) -> bool {
        use wacore::stats::HeapSize;

        let entries = self.transitions.values().map(Vec::len).sum::<usize>();
        if entries >= MAX_PENDING_CALL_LINK_TRANSITIONS {
            return false;
        }
        let new_key_bytes = if self.transitions.contains_key(call_id) {
            0
        } else {
            size_of::<String>() + call_id.heap_bytes()
        };
        let structural_reserve = MAX_PENDING_CALL_LINK_TRANSITIONS
            .saturating_mul(size_of::<PendingCallLinkTransition>());
        self.memory_stats()
            .bytes
            .saturating_add(payload_bytes.try_into().unwrap_or(u64::MAX))
            .saturating_add(new_key_bytes.try_into().unwrap_or(u64::MAX))
            .saturating_add(structural_reserve.try_into().unwrap_or(u64::MAX))
            <= MAX_PENDING_CALL_LINK_TRANSITION_BYTES as u64
    }

    fn bind_call_id(&mut self, call_id: &str) {
        let fingerprint = self.call_id_fingerprint(call_id);
        self.bound_call_id = Some(call_id.to_string());
        self.transitions.retain(|retained, _| retained == call_id);
        self.saturation_fingerprints
            .retain(|retained| *retained == fingerprint);
    }

    fn prepare_bound_retry(&mut self, call_id: &str) -> bool {
        if !self.untracked_saturation || self.bound_call_id.as_deref() != Some(call_id) {
            return false;
        }
        // The first ACK gives the provisional buffer an exact identity. When unrelated traffic
        // exhausted even the overflow fingerprints, retry the join from that bound state instead
        // of either failing the valid call or silently ignoring a possibly dropped transition.
        // The refreshed ACK is the new authoritative floor; controls racing the retry are retained
        // only for this call id and replayed after it.
        self.transitions.clear();
        self.saturation_fingerprints.clear();
        self.untracked_saturation = false;
        true
    }

    fn is_saturated(&self, call_id: &str) -> bool {
        self.untracked_saturation
            || self
                .saturation_fingerprints
                .contains(&self.call_id_fingerprint(call_id))
            || self.transitions.get(call_id).is_some_and(|transitions| {
                transitions
                    .iter()
                    .any(|transition| matches!(transition, PendingCallLinkTransition::Saturated))
            })
    }

    fn call_id_fingerprint(&self, call_id: &str) -> u64 {
        use std::hash::BuildHasher;

        self.saturation_hash_builder.hash_one(call_id)
    }

    fn mark_saturated(&mut self, call_id: &str) {
        // Saturation belongs to the call whose transition could not be retained. An unrelated
        // creator-authenticated control must not poison the one unknown call-link join that owns
        // this bounded buffer.
        self.transitions.remove(call_id);
        if self.can_buffer_transition(call_id, 0) {
            self.transitions.insert(
                call_id.to_string(),
                vec![PendingCallLinkTransition::Saturated],
            );
            return;
        }
        let fingerprint = self.call_id_fingerprint(call_id);
        if self.saturation_fingerprints.contains(&fingerprint) {
            return;
        }
        if self.saturation_fingerprints.len() < MAX_PENDING_CALL_LINK_SATURATION_FINGERPRINTS {
            // Keep the failure identity in fixed-size metadata even when unrelated payloads have
            // consumed every transition slot. Binding the ACK can then fail exactly the join whose
            // admission state was dropped without letting unrelated saturation poison it.
            self.saturation_fingerprints.push(fingerprint);
        } else {
            // If even the bounded fingerprint reserve is exhausted, remember that exact
            // membership is ambiguous. Once the ACK binds a call id, the join path refreshes its
            // authoritative state before registration instead of guessing or failing another id.
            self.untracked_saturation = true;
        }
    }

    pub(crate) fn memory_stats(&self) -> wacore::stats::CollectionStats {
        use wacore::stats::HeapSize;

        let transition_bytes = self
            .transitions
            .iter()
            .map(|(call_id, transitions)| {
                size_of::<String>()
                    + call_id.heap_bytes()
                    + transitions.capacity() * size_of::<PendingCallLinkTransition>()
                    + transitions
                        .iter()
                        .map(PendingCallLinkTransition::heap_bytes)
                        .sum::<usize>()
            })
            .sum::<usize>();
        let bytes = transition_bytes
            .saturating_add(self.saturation_fingerprints.capacity() * size_of::<u64>());
        wacore::stats::CollectionStats::new(
            self.transitions
                .values()
                .map(Vec::len)
                .sum::<usize>()
                .saturating_add(self.saturation_fingerprints.len())
                .saturating_add(usize::from(self.untracked_saturation))
                .try_into()
                .unwrap_or(u64::MAX),
            bytes.try_into().unwrap_or(u64::MAX),
        )
    }
}

#[cfg(feature = "voip-runtime")]
struct PendingCallLinkJoinGuard {
    state: Arc<std::sync::Mutex<PendingCallLinkJoins>>,
}

#[cfg(feature = "voip-runtime")]
impl Drop for PendingCallLinkJoinGuard {
    fn drop(&mut self) {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.active = state.active.saturating_sub(1);
        if state.active == 0 {
            state.bound_call_id = None;
            state.transitions.clear();
            state.saturation_fingerprints.clear();
            state.untracked_saturation = false;
        }
    }
}

#[cfg(feature = "voip-runtime")]
impl CallLinkRegistrationGuard {
    fn new(
        client: &Client,
        registry: Arc<wacore::voip::CallRegistry>,
        call_id: &str,
        call_creator: Jid,
        generation: u64,
    ) -> Self {
        Self {
            client: client.self_weak.get().cloned().unwrap_or_default(),
            registry,
            call_id: call_id.to_string(),
            call_creator,
            generation,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

#[cfg(feature = "voip-runtime")]
impl Drop for CallLinkRegistrationGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let Some(client) = self.client.upgrade() else {
            self.registry
                .remove_if_current(&self.call_id, self.generation);
            return;
        };
        let registry = self.registry.clone();
        let call_id = self.call_id.clone();
        let call_creator = self.call_creator.clone();
        let generation = self.generation;
        let runtime = client.runtime.clone();
        runtime
            .spawn(Box::pin(async move {
                // A cancelled admitted join is still live on the call service. Claim this exact
                // generation under the replacement lane before deciding whether a wire terminate
                // is required; waiting-room cancellation remains local-only.
                let _transition = client.lock_answer_transition(&call_id).await;
                let Some(phase) = registry.remove_if_current_with_phase(&call_id, generation)
                else {
                    return;
                };
                if phase == CallPhase::WaitingRoom {
                    return;
                }
                let target = Jid::new(&call_id, Server::Call);
                crate::voip::facade::send_answer_terminate(
                    &client,
                    &call_id,
                    &target,
                    &call_creator,
                )
                .await;
            }))
            .detach();
    }
}

impl Client {
    /// Call control: reject/terminate are always available; media (call/accept)
    /// needs the `voip` feature.
    pub fn voip(&self) -> Voip<'_> {
        Voip { client: self }
    }

    /// The per-call media registry the `voip` facade registers active calls in. `pub(crate)` so the
    /// facade and the connection-cleanup teardown share one instance.
    #[cfg(feature = "voip-runtime")]
    pub(crate) fn call_registry(&self) -> Arc<wacore::voip::CallRegistry> {
        self.voip_state().call_registry.clone()
    }

    #[cfg(feature = "voip-runtime")]
    fn begin_call_link_join(&self) -> PendingCallLinkJoinGuard {
        let mut state = self
            .voip_state()
            .pending_call_link_joins
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.active == 0 {
            state.bound_call_id = None;
            state.transitions.clear();
            state.saturation_fingerprints.clear();
            state.untracked_saturation = false;
        }
        state.active = state.active.saturating_add(1);
        drop(state);
        PendingCallLinkJoinGuard {
            state: self.voip_state().pending_call_link_joins.clone(),
        }
    }

    #[cfg(feature = "voip-runtime")]
    pub(crate) fn pending_call_link_control_candidate(
        &self,
        call_id: &str,
        call_creator: &Jid,
        sender: &Jid,
    ) -> bool {
        let state = self
            .voip_state()
            .pending_call_link_joins
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.active != 0
            && !call_id.is_empty()
            && state.accepts(call_id)
            && sender.to_non_ad() == call_creator.to_non_ad()
            && self
                .voip_state()
                .call_registry
                .generation_of(call_id)
                .is_none()
    }

    /// Bind the one serialized pending link join to the ACK's exact call id before the read loop
    /// wakes the request task. Controls for unrelated unknown calls can no longer consume its
    /// retained-entry or byte budget during the ACK/registration race.
    #[cfg(feature = "voip-runtime")]
    pub(crate) fn bind_pending_call_link_join_ack(&self, response: &wacore_binary::NodeRef<'_>) {
        let Ok(call_id) = parse_call_link_join_call_id(response) else {
            return;
        };
        let mut state = self
            .voip_state()
            .pending_call_link_joins
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.active != 0 {
            state.bind_call_id(&call_id);
        }
    }

    #[cfg(feature = "voip-runtime")]
    fn prepare_pending_call_link_join_retry(&self, call_id: &str) -> bool {
        let mut state = self
            .voip_state()
            .pending_call_link_joins
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.prepare_bound_retry(call_id)
    }

    /// Buffer a creator-authenticated admission snapshot while its link-join ACK is being
    /// registered. The pending-state lock is shared with registration, closing both orderings of
    /// the ACK/update race without accepting arbitrary unknown calls.
    #[cfg(feature = "voip-runtime")]
    pub(crate) fn buffer_pending_call_link_update(
        &self,
        update: &GroupCallUpdate,
        sender: &Jid,
    ) -> PendingCallLinkBuffer {
        let mut state = self
            .voip_state()
            .pending_call_link_joins
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.active == 0
            || update.call_id.is_empty()
            || !state.accepts(&update.call_id)
            || sender.to_non_ad() != update.call_creator.to_non_ad()
            || self
                .voip_state()
                .call_registry
                .generation_of(&update.call_id)
                .is_some()
        {
            return PendingCallLinkBuffer::NotPending;
        }
        if state.is_saturated(&update.call_id) {
            return PendingCallLinkBuffer::Saturated;
        }
        if state
            .transitions
            .get(&update.call_id)
            .into_iter()
            .flatten()
            .rev()
            .find_map(|transition| match transition {
                PendingCallLinkTransition::Group(update) => Some(update.transaction_id),
                PendingCallLinkTransition::WaitingRoom(_)
                | PendingCallLinkTransition::RawEpoch { .. }
                | PendingCallLinkTransition::Terminated { .. }
                | PendingCallLinkTransition::Saturated => None,
            })
            .is_some_and(|transaction_id| transaction_id >= update.transaction_id)
        {
            return PendingCallLinkBuffer::Buffered;
        }
        if !state.can_buffer_transition(
            &update.call_id,
            PendingCallLinkTransition::group_heap_bytes(update),
        ) {
            state.mark_saturated(&update.call_id);
            return PendingCallLinkBuffer::Saturated;
        }
        state
            .transitions
            .entry(update.call_id.clone())
            .or_default()
            .push(PendingCallLinkTransition::Group(Box::new(update.clone())));
        PendingCallLinkBuffer::Buffered
    }

    /// Retain a creator-authenticated epoch that overtook publication of the call-link generation.
    #[cfg(feature = "voip-runtime")]
    pub(crate) fn buffer_pending_call_link_epoch(
        &self,
        call_id: &str,
        call_creator: &Jid,
        sender: &Jid,
        transaction_id: u32,
        raw_epoch: &[u8],
    ) -> PendingCallLinkBuffer {
        let mut state = self
            .voip_state()
            .pending_call_link_joins
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.active == 0
            || call_id.is_empty()
            || !state.accepts(call_id)
            || sender.to_non_ad() != call_creator.to_non_ad()
            || self
                .voip_state()
                .call_registry
                .generation_of(call_id)
                .is_some()
        {
            return PendingCallLinkBuffer::NotPending;
        }
        if state.is_saturated(call_id) {
            return PendingCallLinkBuffer::Saturated;
        }
        if state
            .transitions
            .get(call_id)
            .into_iter()
            .flatten()
            .rev()
            .find_map(|transition| match transition {
                PendingCallLinkTransition::RawEpoch {
                    call_creator: retained_creator,
                    sender: retained_sender,
                    transaction_id,
                    ..
                } if retained_creator == call_creator && retained_sender == sender => {
                    Some(*transaction_id)
                }
                _ => None,
            })
            .is_some_and(|retained| retained >= transaction_id)
        {
            return PendingCallLinkBuffer::Buffered;
        }
        if !state.can_buffer_transition(call_id, raw_epoch.len()) {
            state.mark_saturated(call_id);
            return PendingCallLinkBuffer::Saturated;
        }
        state
            .transitions
            .entry(call_id.to_string())
            .or_default()
            .push(PendingCallLinkTransition::RawEpoch {
                call_creator: call_creator.clone(),
                sender: sender.clone(),
                transaction_id,
                raw_epoch: Zeroizing::new(raw_epoch.to_vec()),
            });
        PendingCallLinkBuffer::Buffered
    }

    /// Mark a creator-authenticated call-link generation as ended before its ACK is registered.
    #[cfg(feature = "voip-runtime")]
    pub(crate) fn buffer_pending_call_link_terminate(
        &self,
        call_id: &str,
        call_creator: &Jid,
        sender: &Jid,
    ) -> PendingCallLinkBuffer {
        let mut state = self
            .voip_state()
            .pending_call_link_joins
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if state.active == 0
            || call_id.is_empty()
            || !state.accepts(call_id)
            || sender.to_non_ad() != call_creator.to_non_ad()
            || self
                .voip_state()
                .call_registry
                .generation_of(call_id)
                .is_some()
        {
            return PendingCallLinkBuffer::NotPending;
        }
        if state.is_saturated(call_id) {
            return PendingCallLinkBuffer::Saturated;
        }
        if state
            .transitions
            .get(call_id)
            .into_iter()
            .flatten()
            .any(|transition| {
                matches!(
                    transition,
                    PendingCallLinkTransition::Terminated {
                        call_creator: retained_creator,
                        sender: retained_sender,
                    } if retained_creator == call_creator && retained_sender == sender
                )
            })
        {
            return PendingCallLinkBuffer::Buffered;
        }
        if !state.can_buffer_transition(call_id, 0) {
            state.mark_saturated(call_id);
            return PendingCallLinkBuffer::Saturated;
        }
        state
            .transitions
            .entry(call_id.to_string())
            .or_default()
            .push(PendingCallLinkTransition::Terminated {
                call_creator: call_creator.clone(),
                sender: sender.clone(),
            });
        PendingCallLinkBuffer::Buffered
    }

    /// Serialize a terminal control with publication of the call-link generation it targets.
    #[cfg(feature = "voip-runtime")]
    pub(crate) async fn retain_or_apply_pending_call_link_terminate(
        &self,
        call_id: &str,
        call_creator: &Jid,
        sender: &Jid,
    ) -> bool {
        let _answer_transition = self.lock_answer_transition(call_id).await;
        let buffered = self.buffer_pending_call_link_terminate(call_id, call_creator, sender);
        if buffered.suppresses_dispatch() {
            return true;
        }
        let Some(generation) = self.voip_state().call_registry.generation_of(call_id) else {
            return false;
        };
        if !self
            .voip_state()
            .call_registry
            .group_creator_authorized_if_current(call_id, generation, call_creator, sender)
        {
            return false;
        }
        self.voip_state()
            .call_registry
            .remove_if_current(call_id, generation)
    }

    /// Buffer a creator-authenticated waiting-room snapshot in the same ordered call-link
    /// transition stream as admission rosters.
    #[cfg(feature = "voip-runtime")]
    pub(crate) fn buffer_pending_call_link_waiting_room(
        &self,
        room: &WaitingRoom,
        sender: &Jid,
    ) -> PendingCallLinkBuffer {
        let mut state = self
            .voip_state()
            .pending_call_link_joins
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if self
            .voip_state()
            .call_registry
            .generation_of(&room.call_id)
            .is_some()
            || state.active == 0
            || room.call_id.is_empty()
            || !state.accepts(&room.call_id)
            || room.link_token.is_empty()
            || sender.to_non_ad() != room.call_creator.to_non_ad()
        {
            return PendingCallLinkBuffer::NotPending;
        }
        if state.is_saturated(&room.call_id) {
            return PendingCallLinkBuffer::Saturated;
        }
        if !state.can_buffer_transition(
            &room.call_id,
            PendingCallLinkTransition::waiting_room_heap_bytes(room),
        ) {
            state.mark_saturated(&room.call_id);
            return PendingCallLinkBuffer::Saturated;
        }
        state
            .transitions
            .entry(room.call_id.clone())
            .or_default()
            .push(PendingCallLinkTransition::WaitingRoom(room.clone()));
        PendingCallLinkBuffer::Buffered
    }

    #[cfg(feature = "voip-runtime")]
    async fn register_call_link_session(
        &self,
        session: CallSession,
        waiting_room: Option<WaitingRoom>,
        expected_media: CallLinkMedia,
        expected_token: &str,
    ) -> Result<u64, wacore::voip::GroupStateApply> {
        let voip = self.voip_state();
        let call_id = session.call_id.clone();
        let call_creator = session.call_creator.clone();
        let mut rekey_pending = session
            .group
            .as_ref()
            .is_some_and(|update| update.rekey_requested);
        // Share the stable call-id lane with every competing call registration. Once this join
        // inserts its generation, no re-offer can replace it until all staged admission state has
        // either committed to that generation or caused registration to fail.
        let _answer_transition = self.lock_answer_transition(&call_id).await;
        let mut state = self
            .voip_state()
            .pending_call_link_joins
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        state.bind_call_id(&call_id);
        let saturated = state.is_saturated(&call_id);
        let staged = state.transitions.remove(&call_id).unwrap_or_default();
        if saturated {
            return Err(wacore::voip::GroupStateApply::InvalidSnapshot);
        }
        let generation = self
            .voip_state()
            .call_registry
            .insert_call_link_checked(session)?;
        if let Some(room) = waiting_room {
            let applied = self
                .voip_state()
                .call_registry
                .apply_waiting_room_if_current(room, generation);
            if applied != wacore::voip::GroupStateApply::Applied {
                voip.call_registry.remove_if_current(&call_id, generation);
                return Err(applied);
            }
        }
        for transition in staged {
            match transition {
                PendingCallLinkTransition::Group(update)
                    if update.call_creator == call_creator
                        && update.media == expected_media.as_str() =>
                {
                    let mut update = *update;
                    update.rekey_requested |= rekey_pending;
                    let staged_rekey = update.rekey_requested;
                    match self.apply_pending_call_link_update(update, generation) {
                        wacore::voip::GroupStateApply::Applied => {
                            rekey_pending = staged_rekey;
                        }
                        wacore::voip::GroupStateApply::Stale => {}
                        rejected => {
                            voip.call_registry.remove_if_current(&call_id, generation);
                            return Err(rejected);
                        }
                    }
                }
                PendingCallLinkTransition::WaitingRoom(room)
                    if room.call_creator == call_creator
                        && room.media == expected_media
                        && room.link_token == expected_token =>
                {
                    let applied = self
                        .voip_state()
                        .call_registry
                        .apply_waiting_room_if_current(room, generation);
                    if !matches!(
                        applied,
                        wacore::voip::GroupStateApply::Applied
                            | wacore::voip::GroupStateApply::Stale
                    ) {
                        voip.call_registry.remove_if_current(&call_id, generation);
                        return Err(applied);
                    }
                }
                PendingCallLinkTransition::RawEpoch {
                    call_creator: staged_creator,
                    sender,
                    transaction_id,
                    raw_epoch,
                } => {
                    if !self
                        .voip_state()
                        .call_registry
                        .group_sender_authorized_if_current(
                            &call_id,
                            generation,
                            &staged_creator,
                            &sender,
                        )
                    {
                        continue;
                    }
                    if !voip.call_registry.send_group_epoch_if_current(
                        &call_id,
                        generation,
                        transaction_id,
                        raw_epoch.to_vec(),
                    ) {
                        voip.call_registry.remove_if_current(&call_id, generation);
                        return Err(wacore::voip::GroupStateApply::UnknownCall);
                    }
                }
                PendingCallLinkTransition::Terminated {
                    call_creator: staged_creator,
                    sender,
                } => {
                    if !self
                        .voip_state()
                        .call_registry
                        .group_creator_authorized_if_current(
                            &call_id,
                            generation,
                            &staged_creator,
                            &sender,
                        )
                    {
                        continue;
                    }
                    voip.call_registry.remove_if_current(&call_id, generation);
                    return Err(wacore::voip::GroupStateApply::InvalidSnapshot);
                }
                PendingCallLinkTransition::Saturated => {
                    voip.call_registry.remove_if_current(&call_id, generation);
                    return Err(wacore::voip::GroupStateApply::InvalidSnapshot);
                }
                _ => {}
            }
        }
        Ok(generation)
    }

    #[cfg(feature = "voip-runtime")]
    fn apply_pending_call_link_update(
        &self,
        update: GroupCallUpdate,
        generation: u64,
    ) -> wacore::voip::GroupStateApply {
        self.voip_state()
            .call_registry
            .apply_group_update_if_current(update, generation)
    }

    /// Lock the striped answer-transition lane for `call_id`. Incoming answer registration and
    /// answer teardown both use this, preventing a replacement generation from being installed
    /// after the old one is claimed but before its terminal stanza reaches the wire.
    #[cfg(feature = "voip-runtime")]
    pub(crate) fn answer_transition_lock(&self, call_id: &str) -> Arc<async_lock::Mutex<()>> {
        use std::hash::{Hash, Hasher};

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        call_id.hash(&mut hasher);
        let lane = hasher.finish() as usize % self.voip_state().answer_transition_locks.len();
        self.voip_state().answer_transition_locks[lane].clone()
    }

    #[cfg(feature = "voip-runtime")]
    pub(crate) async fn lock_answer_transition(
        &self,
        call_id: &str,
    ) -> async_lock::MutexGuardArc<()> {
        self.answer_transition_lock(call_id).lock_arc().await
    }
}

/// Errors from call-control operations. `#[non_exhaustive]` so new variants stay
/// non-breaking after 1.0.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CallError {
    #[error("{0}")]
    Send(#[from] ClientError),
    #[error("call_id cannot be empty")]
    EmptyCallId,
    /// `accept` was called with an `IncomingCall` that is not an `<offer>` (nothing to answer).
    #[cfg(feature = "voip-runtime")]
    #[error("not an incoming call offer")]
    NotAnOffer,
    /// `accept().start()` was called without PCM or encoded audio endpoints.
    #[cfg(feature = "voip-runtime")]
    #[error("accept() requires audio(...) or encoded_audio(...) before start()")]
    MissingAudio,
    /// The selected media profile was not present in the incoming offer.
    #[cfg(feature = "voip-runtime")]
    #[error("incoming offer does not advertise the selected audio rate {0}")]
    AudioFormatNotOffered(u32),
    /// Video endpoints were supplied for an offer that only advertised audio.
    #[cfg(feature = "voip-runtime")]
    #[error("incoming offer did not advertise video; use start_video() after answering")]
    VideoNotOffered,
    /// The peer ended or superseded the call while the answer was being prepared.
    #[cfg(feature = "voip-runtime")]
    #[error("call ended during answer setup")]
    CallEndedDuringSetup,
    /// Decrypting the offer's encrypted callKey failed.
    #[cfg(feature = "voip-runtime")]
    #[error("callKey decrypt failed: {0}")]
    Decrypt(String),
    /// Assembling the call config from the offer's relay block failed.
    #[cfg(feature = "voip-runtime")]
    #[error("call setup failed: {0}")]
    Setup(String),
    /// Connecting the relay media transport (UDP/DTLS/SCTP) failed.
    #[cfg(feature = "voip-runtime")]
    #[error("relay connect failed: {0}")]
    Connect(String),
    /// The offer was missing media material (no `<enc>`/`<relay>`, no callKey, no own LID, etc.).
    #[cfg(feature = "voip-runtime")]
    #[error("media offer error: {0}")]
    Media(&'static str),
    /// The peer cancelled or replaced the upgrade before its video source became ready.
    #[cfg(feature = "voip-runtime")]
    #[error("video upgrade request is no longer current")]
    VideoUpgradeExpired,
    /// `call(peer)` resolved zero devices for the peer (nothing to address an offer to).
    #[cfg(feature = "voip-runtime")]
    #[error("peer has no resolvable devices")]
    NoDevices,
    /// An outgoing offer would emit a pkmsg `<enc>` but we hold no ADV account, so the peer could
    /// not validate the pre-key message. Refused before send to avoid advancing the sender chain
    /// (mirrors the peer-send path's `<device-identity>` requirement).
    #[cfg(feature = "voip-runtime")]
    #[error("offer pkmsg requires <device-identity> (account is None)")]
    MissingDeviceIdentity,
    /// A call-service response was malformed or rejected.
    #[cfg(feature = "voip-runtime")]
    #[error("call service response failed: {0}")]
    Response(String),
    /// The call service did not answer within its bounded request window.
    #[cfg(feature = "voip-runtime")]
    #[error("call service request timed out")]
    ResponseTimeout,
}

impl Voip<'_> {
    /// Reject an incoming call. Fire-and-forget — no server response is expected.
    pub async fn reject(&self, incoming: &IncomingCall) -> Result<(), CallError> {
        self.reject_call_inner(
            incoming.action.call_id(),
            &incoming.from,
            incoming.action.call_creator(),
            incoming.ringing_generation(),
        )
        .await
    }

    /// Reject a call when its signaling identifiers are already available.
    /// `peer` is the outer `<call to>` target, while `call_creator` is the
    /// action's `call-creator` attribute; preserve them separately because
    /// they may differ for companion-device signaling.
    /// Fire-and-forget — no server response is expected.
    pub async fn reject_call(
        &self,
        call_id: &str,
        peer: &Jid,
        call_creator: &Jid,
    ) -> Result<(), CallError> {
        self.reject_call_inner(call_id, peer, call_creator, None)
            .await
    }

    async fn reject_call_inner(
        &self,
        call_id: &str,
        peer: &Jid,
        call_creator: &Jid,
        _ringing_generation: Option<u64>,
    ) -> Result<(), CallError> {
        if call_id.is_empty() {
            return Err(CallError::EmptyCallId);
        }
        let id = self.client.generate_request_id();
        let stanza = build_reject(call_id, peer, call_creator, &id);
        // Consume the ringing flag BEFORE the async send: a caller <terminate> processed while we await
        // the send would otherwise hit take_ringing first and surface a phantom missed call for a call
        // we already declined (WA Web deletes it from _ringingCalls on reject). No-op if never ringing.
        #[cfg(feature = "voip-runtime")]
        {
            let registry = self.client.call_registry();
            if let Some(generation) = _ringing_generation {
                if !registry.reject_ringing_if_current(call_id, generation) {
                    return Err(CallError::CallEndedDuringSetup);
                }
            } else {
                let generation = registry.ringing_group_generation(call_id, call_creator);
                registry.take_ringing(call_id);
                if let Some(generation) = generation {
                    registry.remove_if_current(call_id, generation);
                }
            }
        }
        self.client.send_node(stanza).await?;
        Ok(())
    }

    /// Begin answering an incoming call: returns a builder; call `.audio(source, sink)` then
    /// `.start().await` to send `<preaccept>`, decrypt the callKey, send `<accept>`, connect the relay,
    /// and drive the call, yielding a [`CallHandle`](crate::voip::CallHandle). Requires
    /// `voip-runtime` or a profile that enables it: `voip`, `voip-encoded`, `voip-mlow`, or
    /// `voip-libopus`.
    #[cfg(feature = "voip-runtime")]
    pub fn accept<'b>(&'b self, incoming: &'b IncomingCall) -> crate::voip::AcceptCall<'b> {
        crate::voip::facade::AcceptCall::new(self.client, incoming)
    }

    /// Begin placing an outgoing 1:1 call to `peer`: returns a builder; call `.audio(source, sink)`
    /// then `.start().await` to generate the callKey, encrypt it per peer device, send the `<offer>`,
    /// and register the call, yielding a [`CallHandle`](crate::voip::CallHandle). The media engine
    /// only attaches once the server hands back the relay for our call-id (live), so the returned
    /// handle is dormant until then. Requires `voip-runtime` or a profile that enables it: `voip`,
    /// `voip-encoded`, `voip-mlow`, or `voip-libopus`.
    #[cfg(feature = "voip-runtime")]
    pub fn call<'b>(&'b self, peer: &'b Jid) -> crate::voip::OutgoingCall<'b> {
        crate::voip::facade::OutgoingCall::new(self.client, peer)
    }

    /// Begin a native group call to two or more selected users.
    #[cfg(feature = "voip-runtime")]
    pub fn group_call<'b>(&'b self, targets: &'b [Jid]) -> crate::voip::OutgoingGroupCall<'b> {
        crate::voip::facade::OutgoingGroupCall::new(self.client, targets)
    }

    /// Begin a native call bound to an existing group. The current roster is resolved at
    /// [`start`](crate::voip::GroupBoundCall::start), with this account excluded automatically.
    #[cfg(feature = "voip-runtime")]
    pub fn group_call_by_id<'b>(&'b self, group_jid: &'b Jid) -> crate::voip::GroupBoundCall<'b> {
        crate::voip::facade::GroupBoundCall::new(self.client, group_jid)
    }

    /// Join a reusable call link and attach group media after admission.
    #[cfg(feature = "voip-runtime")]
    pub fn call_link<'b>(
        &'b self,
        token_or_url: &'b str,
        media: CallLinkMedia,
    ) -> crate::voip::CallLinkCall<'b> {
        crate::voip::facade::CallLinkCall::new(self.client, token_or_url, media)
    }

    /// Send the eager preparation response for an active group-call invitation.
    #[cfg(feature = "voip-runtime")]
    pub async fn preaccept_group_invite(&self, incoming: &IncomingCall) -> Result<(), CallError> {
        let CallAction::Offer {
            call_id,
            call_creator,
            is_video,
            ..
        } = &incoming.action
        else {
            return Err(CallError::NotAnOffer);
        };
        if incoming.group.is_none() {
            return Err(CallError::Media("offer is not an active group invitation"));
        }
        let registry = self.client.call_registry();
        let Some(retained_generation) = incoming.ringing_generation() else {
            return Err(CallError::CallEndedDuringSetup);
        };
        let generation = registry
            .ringing_group_generation(call_id, call_creator)
            .ok_or(CallError::CallEndedDuringSetup)?;
        if generation != retained_generation {
            return Err(CallError::CallEndedDuringSetup);
        }
        let node = build_active_group_preaccept(
            call_id,
            call_creator,
            &self.client.generate_request_id(),
            *is_video,
        )
        .map_err(|error| CallError::Response(error.to_string()))?;
        self.client.send_node(node).await?;
        if registry.ringing_group_generation(call_id, call_creator) != Some(generation) {
            return Err(CallError::CallEndedDuringSetup);
        }
        Ok(())
    }

    /// Send an early call-scoped accept for an active group invitation.
    ///
    /// The retained offer remains ringing so [`accept`](Self::accept) can subsequently attach the
    /// application's media endpoints to the exact same generation.
    #[cfg(feature = "voip-runtime")]
    pub async fn accept_group_invite(&self, incoming: &IncomingCall) -> Result<(), CallError> {
        let CallAction::Offer {
            call_id,
            call_creator,
            is_video,
            ..
        } = &incoming.action
        else {
            return Err(CallError::NotAnOffer);
        };
        if incoming.group.is_none() {
            return Err(CallError::Media("offer is not an active group invitation"));
        }
        let registry = self.client.call_registry();
        let Some(retained_generation) = incoming.ringing_generation() else {
            return Err(CallError::CallEndedDuringSetup);
        };
        let generation = registry
            .ringing_group_generation(call_id, call_creator)
            .ok_or(CallError::CallEndedDuringSetup)?;
        if generation != retained_generation {
            return Err(CallError::CallEndedDuringSetup);
        }
        let node = build_active_group_accept(
            call_id,
            call_creator,
            &self.client.generate_request_id(),
            *is_video,
        )
        .map_err(|error| CallError::Response(error.to_string()))?;
        self.client.send_node(node).await?;
        if registry.ringing_group_generation(call_id, call_creator) != Some(generation) {
            return Err(CallError::CallEndedDuringSetup);
        }
        Ok(())
    }

    /// Create a reusable audio or video call link.
    #[cfg(feature = "voip-runtime")]
    pub async fn create_call_link(&self, media: CallLinkMedia) -> Result<CallLink, CallError> {
        let request_id = self.client.generate_request_id();
        let request = build_call_link_create(media, &request_id)
            .map_err(|error| CallError::Response(error.to_string()))?;
        let link = execute_call_service_request(
            self.client,
            &request_id,
            request,
            parse_call_link_create_ack,
        )
        .await?;
        if link.media != media {
            return Err(CallError::Response(
                "call-link creation changed the requested media mode".to_string(),
            ));
        }
        Ok(link)
    }

    /// Inspect a call link without joining it.
    #[cfg(feature = "voip-runtime")]
    pub async fn preview_call_link(
        &self,
        token_or_url: &str,
        media: CallLinkMedia,
    ) -> Result<CallLinkPreview, CallError> {
        let token = normalize_call_link_token(token_or_url, media)?;
        let request_id = self.client.generate_request_id();
        let request = build_call_link_query(&token, media, &request_id)
            .map_err(|error| CallError::Response(error.to_string()))?;
        let preview = execute_call_service_request(
            self.client,
            &request_id,
            request,
            parse_call_link_query_ack,
        )
        .await?;
        if preview.token != token || preview.media != media {
            return Err(CallError::Response(
                "call-link preview changed the requested link identity".to_string(),
            ));
        }
        Ok(preview)
    }

    /// Join a call link. The result explicitly reports whether this endpoint was admitted or placed
    /// in the waiting room; media starts only after an admitted authoritative group snapshot.
    #[cfg(feature = "voip-runtime")]
    pub async fn join_call_link(
        &self,
        token_or_url: &str,
        media: CallLinkMedia,
    ) -> Result<CallLinkJoin, CallError> {
        self.join_call_link_with_audio(token_or_url, media, AudioFormat::MLOW_16KHZ_60MS)
            .await
    }

    #[cfg(feature = "voip-runtime")]
    pub(crate) async fn join_call_link_with_audio(
        &self,
        token_or_url: &str,
        media: CallLinkMedia,
        audio_format: AudioFormat,
    ) -> Result<CallLinkJoin, CallError> {
        Ok(self
            .join_call_link_registration_with_audio(token_or_url, media, audio_format)
            .await?
            .join)
    }

    #[cfg(feature = "voip-runtime")]
    pub(crate) async fn join_call_link_registration_with_audio(
        &self,
        token_or_url: &str,
        media: CallLinkMedia,
        audio_format: AudioFormat,
    ) -> Result<CallLinkJoinRegistration, CallError> {
        // Before the ACK arrives, creator-authenticated admission traffic has no trusted call id
        // to associate with this request. Keep one such request active at a time so bounded-buffer
        // saturation can fail only its owning join; other joins wait here and start with clean
        // staging state.
        let pending_join_lane = self
            .client
            .voip_state()
            .pending_call_link_join_lane
            .lock()
            .await;
        let own_lid = self.client.lid().ok_or(CallError::Media("no own LID"))?;
        let token = normalize_call_link_token(token_or_url, media)?;
        let capability =
            crate::voip::facade::offer_capability(media == CallLinkMedia::Video, audio_format);
        let pending_join = self.client.begin_call_link_join();
        let mut join =
            execute_call_link_join_request(self.client, &token, media, capability).await?;
        if join.media != media {
            return Err(CallError::Response(
                "call-link response changed the requested media mode".to_string(),
            ));
        }
        if join.call_id.is_empty() {
            return Err(CallError::EmptyCallId);
        }
        if self
            .client
            .prepare_pending_call_link_join_retry(&join.call_id)
        {
            let first_call_id = join.call_id.clone();
            let first_call_creator = join.call_creator.clone();
            let refreshed =
                execute_call_link_join_request(self.client, &token, media, capability).await?;
            if refreshed.media != media
                || refreshed.call_id != first_call_id
                || refreshed.call_creator != first_call_creator
            {
                return Err(CallError::Response(
                    "call-link identity changed while refreshing admission state".to_string(),
                ));
            }
            join = refreshed;
        }

        let mut session = CallSession::new_outgoing(
            &join.call_id,
            Jid::new(&join.call_id, Server::Call),
            join.call_creator.clone(),
        );
        session.audio_format = Some(audio_format);
        session.is_video = media == CallLinkMedia::Video;
        session.group = join.group.clone();
        let _ = session.transition_to(CallPhase::Calling);
        let _ = session.transition_to(if join.in_waiting_room {
            CallPhase::WaitingRoom
        } else {
            CallPhase::Connecting
        });
        let registry = self.client.call_registry();
        let generation = self
            .client
            .register_call_link_session(session, join.waiting_room.clone(), media, &token)
            .await
            .map_err(|_| {
                CallError::Response("call-link admission snapshot was rejected".to_string())
            })?;
        // Publication transfers admission controls to the generation-scoped registry. Clear the
        // provisional binding before another serialized unknown-id join starts with a clean buffer.
        drop(pending_join);
        drop(pending_join_lane);
        let mut registration = CallLinkRegistrationGuard::new(
            self.client,
            registry.clone(),
            &join.call_id,
            join.call_creator.clone(),
            generation,
        );

        if join.in_waiting_room && join.waiting_room.is_none() {
            return Err(CallError::Response(
                "call-link join omitted its waiting-room state".to_string(),
            ));
        }

        registry.set_group_invite_self_device(
            &join.call_id,
            generation,
            wacore::types::group_call::GroupCallDevice::new(own_lid).with_capability(1, capability),
        );
        let rekey_required = join
            .group
            .as_ref()
            .is_some_and(|update| update.rekey_requested);
        let mut still_waiting = self
            .synchronize_call_link_admission(&mut join, generation, rekey_required)
            .await?;
        if still_waiting {
            let heartbeat = self
                .waiting_room_heartbeat(&join.call_id, &join.call_creator)
                .await;
            // The heartbeat crosses an unbounded transport await. Admission may have committed
            // while it was in flight, so re-read the generation before publishing the result or
            // starting a task that now belongs to an admitted call.
            still_waiting = self
                .synchronize_call_link_admission(&mut join, generation, rekey_required)
                .await?;
            if still_waiting {
                heartbeat?;
            }
        }
        if still_waiting {
            self.start_waiting_room_heartbeat(
                join.call_id.clone(),
                join.call_creator.clone(),
                generation,
            );
        }

        registration.disarm();
        Ok(CallLinkJoinRegistration { join, generation })
    }

    #[cfg(feature = "voip-runtime")]
    async fn synchronize_call_link_admission(
        &self,
        join: &mut CallLinkJoin,
        generation: u64,
        rekey_required: bool,
    ) -> Result<bool, CallError> {
        let registry = self.client.call_registry();
        let transition_lock = registry
            .group_transition_lock(&join.call_id, generation)
            .ok_or(CallError::CallEndedDuringSetup)?;
        let _transition_guard = transition_lock.lock().await;
        let state = registry
            .group_state_if_current(&join.call_id, generation)
            .ok_or(CallError::CallEndedDuringSetup)?;
        if let Some(room) = state.waiting_room().cloned() {
            join.waiting_room_enabled = room.enabled;
            join.is_admin = room.is_admin;
            join.waiting_room = Some(room);
        }
        let phase = registry
            .phase_if_current(&join.call_id, generation)
            .ok_or(CallError::CallEndedDuringSetup)?;
        if phase == CallPhase::WaitingRoom {
            join.in_waiting_room = true;
            return Ok(true);
        }

        let update = state.snapshot().cloned().ok_or(CallError::Media(
            "admitted call link has no authoritative group snapshot",
        ))?;
        join.in_waiting_room = false;
        join.group = Some(update.clone());
        let retained_epoch =
            registry.pending_group_epoch_transaction_if_current(&join.call_id, generation);
        if (rekey_required || update.rekey_requested)
            && retained_epoch.is_none_or(|transaction| transaction < update.transaction_id)
        {
            // The shared transition lane keeps roster selection, fan-out, and publication on the
            // same transaction even if a post-registration update tries to overtake the ACK.
            crate::voip::facade::fanout_group_epoch(self.client, &update)
                .await?
                .commit(|epoch| {
                    registry
                        .send_group_epoch_if_current(
                            &join.call_id,
                            generation,
                            update.transaction_id,
                            epoch.to_vec(),
                        )
                        .then_some(())
                        .ok_or(CallError::Media(
                            "call-link group epoch could not be retained",
                        ))
                })?;
        }
        Ok(false)
    }

    /// Enable or disable approval for a live call-link waiting room.
    #[cfg(feature = "voip-runtime")]
    pub async fn set_approval_required(
        &self,
        call_id: &str,
        call_creator: &Jid,
        enabled: bool,
    ) -> Result<(), CallError> {
        let generation = self
            .client
            .call_registry()
            .generation_of(call_id)
            .ok_or(CallError::Media("call is no longer active"))?;
        self.set_approval_required_for_generation(call_id, call_creator, generation, enabled)
            .await
    }

    #[cfg(feature = "voip-runtime")]
    pub(crate) async fn set_approval_required_for_generation(
        &self,
        call_id: &str,
        call_creator: &Jid,
        generation: u64,
        enabled: bool,
    ) -> Result<(), CallError> {
        let registry = self.client.call_registry();
        let transition_lock = registry
            .group_transition_lock(call_id, generation)
            .ok_or(CallError::Media("call is no longer active"))?;
        let _transition_guard = transition_lock.lock().await;
        self.ensure_waiting_room_admin_if_current(call_id, generation)?;
        let request_id = self.client.generate_request_id();
        execute_call_service_request(
            self.client,
            &request_id,
            build_waiting_room_toggle(call_id, call_creator, enabled, &request_id)
                .map_err(|error| CallError::Response(error.to_string()))?,
            parse_waiting_room_toggle_ack,
        )
        .await?;
        if registry.set_waiting_room_enabled_if_current(call_id, generation, enabled) {
            Ok(())
        } else {
            Err(CallError::Media(
                "call was replaced while applying group control",
            ))
        }
    }

    /// Keep a pending call-link admission alive.
    #[cfg(feature = "voip-runtime")]
    pub async fn waiting_room_heartbeat(
        &self,
        call_id: &str,
        call_creator: &Jid,
    ) -> Result<(), CallError> {
        self.send_group_control(
            call_id,
            build_waiting_room_heartbeat(call_id, call_creator, &self.client.generate_request_id())
                .map_err(|error| CallError::Response(error.to_string()))?,
        )
        .await
    }

    /// Admit one user from a call-link waiting room.
    #[cfg(feature = "voip-runtime")]
    pub async fn admit_waiting_user(
        &self,
        call_id: &str,
        call_creator: &Jid,
        user: &Jid,
    ) -> Result<(), CallError> {
        let generation = self
            .client
            .call_registry()
            .generation_of(call_id)
            .ok_or(CallError::Media("call is no longer active"))?;
        self.admit_waiting_user_for_generation(call_id, call_creator, generation, user)
            .await
    }

    #[cfg(feature = "voip-runtime")]
    pub(crate) async fn admit_waiting_user_for_generation(
        &self,
        call_id: &str,
        call_creator: &Jid,
        generation: u64,
        user: &Jid,
    ) -> Result<(), CallError> {
        self.waiting_room_user_action_for_generation(
            call_id,
            call_creator,
            generation,
            user,
            WaitingRoomUserAction::Admit,
        )
        .await
    }

    /// Deny one user from a call-link waiting room.
    #[cfg(feature = "voip-runtime")]
    pub async fn deny_waiting_user(
        &self,
        call_id: &str,
        call_creator: &Jid,
        user: &Jid,
    ) -> Result<(), CallError> {
        let generation = self
            .client
            .call_registry()
            .generation_of(call_id)
            .ok_or(CallError::Media("call is no longer active"))?;
        self.deny_waiting_user_for_generation(call_id, call_creator, generation, user)
            .await
    }

    #[cfg(feature = "voip-runtime")]
    pub(crate) async fn deny_waiting_user_for_generation(
        &self,
        call_id: &str,
        call_creator: &Jid,
        generation: u64,
        user: &Jid,
    ) -> Result<(), CallError> {
        self.waiting_room_user_action_for_generation(
            call_id,
            call_creator,
            generation,
            user,
            WaitingRoomUserAction::Deny,
        )
        .await
    }

    #[cfg(feature = "voip-runtime")]
    async fn waiting_room_user_action_for_generation(
        &self,
        call_id: &str,
        call_creator: &Jid,
        generation: u64,
        user: &Jid,
        action: WaitingRoomUserAction,
    ) -> Result<(), CallError> {
        self.ensure_waiting_room_admin_if_current(call_id, generation)?;
        let request_id = self.client.generate_request_id();
        let (request, parse) = match action {
            WaitingRoomUserAction::Admit => (
                build_waiting_room_admit(call_id, call_creator, user, &request_id),
                parse_waiting_room_admit_ack
                    as fn(&wacore_binary::NodeRef<'_>) -> anyhow::Result<()>,
            ),
            WaitingRoomUserAction::Deny => (
                build_waiting_room_deny(call_id, call_creator, user, &request_id),
                parse_waiting_room_deny_ack
                    as fn(&wacore_binary::NodeRef<'_>) -> anyhow::Result<()>,
            ),
        };
        execute_call_service_request(
            self.client,
            &request_id,
            request.map_err(|error| CallError::Response(error.to_string()))?,
            parse,
        )
        .await?;
        if self.client.call_registry().is_current(call_id, generation) {
            Ok(())
        } else {
            Err(CallError::Media(
                "call was replaced while applying group control",
            ))
        }
    }

    /// Announce this side's microphone state, as `<mute_v2 mute-state>`.
    ///
    /// `peer` is where the state is published: the call scope for a group or call-link call, whose
    /// roster shows who is muted, and the peer device for a direct call, the same address the rest of
    /// its signaling uses. From a `CallHandle` prefer its own `set_muted()`, which applies the local
    /// mute and resolves `peer` for both shapes.
    #[cfg(feature = "voip-runtime")]
    pub async fn announce_muted(
        &self,
        call_id: &str,
        peer: &Jid,
        call_creator: &Jid,
        muted: bool,
    ) -> Result<(), CallError> {
        let generation = self
            .client
            .call_registry()
            .generation_of(call_id)
            .ok_or(CallError::Media("call is no longer active"))?;
        // The answer-transition lane is the lock a replacement generation is installed under; a
        // per-entry lock belongs to the entry being replaced and so cannot close that window.
        let _transition = self.client.lock_answer_transition(call_id).await;
        self.announce_muted_locked(call_id, peer, call_creator, generation, muted)
            .await
    }

    /// [`announce_muted`](Self::announce_muted) for a caller that already holds the call's
    /// answer-transition lane and its own generation.
    #[cfg(feature = "voip-runtime")]
    pub(crate) async fn announce_muted_locked(
        &self,
        call_id: &str,
        peer: &Jid,
        call_creator: &Jid,
        generation: u64,
        muted: bool,
    ) -> Result<(), CallError> {
        // A handle superseded while it waited must not announce a state for the replacement that now
        // owns this call-id.
        if self.client.call_registry().generation_of(call_id) != Some(generation) {
            return Err(CallError::Media("call is no longer active"));
        }
        self.send_group_control(
            call_id,
            build_mute_v2(
                call_id,
                peer,
                call_creator,
                &self.client.generate_request_id(),
                muted,
            ),
        )
        .await
    }

    /// Publish the local persistent raise/lower-hand state.
    #[cfg(feature = "voip-runtime")]
    pub async fn set_hand_raised(
        &self,
        call_id: &str,
        call_creator: &Jid,
        raised: bool,
    ) -> Result<(), CallError> {
        let generation = self
            .client
            .call_registry()
            .generation_of(call_id)
            .ok_or(CallError::Media("call is no longer active"))?;
        self.set_hand_raised_for_generation(call_id, call_creator, generation, raised)
            .await
    }

    #[cfg(feature = "voip-runtime")]
    pub(crate) async fn set_hand_raised_for_generation(
        &self,
        call_id: &str,
        call_creator: &Jid,
        generation: u64,
        raised: bool,
    ) -> Result<(), CallError> {
        let registry = self.client.call_registry();
        let transition_lock = registry
            .group_transition_lock(call_id, generation)
            .ok_or(CallError::Media("call is no longer active"))?;
        let _transition_guard = transition_lock.lock().await;
        if !registry.group_creator_matches_if_current(call_id, generation, call_creator) {
            return Err(CallError::Media(
                "call creator does not match the active group call",
            ));
        }
        let participant = self
            .client
            .lid()
            .ok_or(CallError::Media("no own LID"))?
            .to_non_ad();
        let target = Jid::new(call_id, Server::Call);
        self.send_group_control(
            call_id,
            build_raise_hand(
                call_id,
                &target,
                call_creator,
                &self.client.generate_request_id(),
                raised,
            )
            .map_err(|error| CallError::Response(error.to_string()))?,
        )
        .await?;
        if registry.set_raised_hand_if_current(call_id, generation, &participant, raised) {
            registry.send_call_event_if_current(
                call_id,
                generation,
                CallEvent::HandRaised {
                    participant,
                    raised,
                },
            );
            Ok(())
        } else {
            Err(CallError::Media(
                "call was replaced while applying group control",
            ))
        }
    }

    /// Publish a screen-share start/stop transition.
    #[cfg(feature = "voip-runtime")]
    pub async fn set_screen_share(
        &self,
        call_id: &str,
        call_creator: &Jid,
        state: ScreenShareState,
        screen_share_id: Option<u32>,
    ) -> Result<(), CallError> {
        let generation = self
            .client
            .call_registry()
            .generation_of(call_id)
            .ok_or(CallError::Media("call is no longer active"))?;
        self.set_screen_share_for_generation(
            call_id,
            call_creator,
            generation,
            state,
            screen_share_id,
        )
        .await
    }

    #[cfg(feature = "voip-runtime")]
    pub(crate) async fn set_screen_share_for_generation(
        &self,
        call_id: &str,
        call_creator: &Jid,
        generation: u64,
        state: ScreenShareState,
        screen_share_id: Option<u32>,
    ) -> Result<(), CallError> {
        let registry = self.client.call_registry();
        let transition_lock = registry
            .group_transition_lock(call_id, generation)
            .ok_or(CallError::Media("call is no longer active"))?;
        let _transition_guard = transition_lock.lock().await;
        if !registry.group_creator_matches_if_current(call_id, generation, call_creator) {
            return Err(CallError::Media(
                "call creator does not match the active group call",
            ));
        }
        let group = registry
            .group_state_if_current(call_id, generation)
            .ok_or(CallError::Media("call is not an active group call"))?;
        if state == ScreenShareState::Started
            && (group
                .snapshot()
                .is_none_or(|snapshot| snapshot.media != "video")
                || !matches!(
                    registry.video_states(call_id, generation),
                    Some((VideoState::Enabled, _))
                ))
        {
            return Err(CallError::Media(
                "screen sharing requires an active local video plane",
            ));
        }
        let participant = self
            .client
            .lid()
            .ok_or(CallError::Media("no own LID"))?
            .to_non_ad();
        let target = Jid::new(call_id, Server::Call);
        self.send_group_control(
            call_id,
            build_screen_share(
                call_id,
                &target,
                call_creator,
                &self.client.generate_request_id(),
                state,
                screen_share_id,
            )
            .map_err(|error| CallError::Response(error.to_string()))?,
        )
        .await?;
        let screen_share = ScreenShare::new(state, screen_share_id);
        if registry.set_screen_share_if_current(
            call_id,
            generation,
            &participant,
            screen_share.clone(),
        ) {
            registry.send_call_event_if_current(
                call_id,
                generation,
                CallEvent::ScreenShareChanged {
                    participant,
                    screen_share,
                },
            );
        } else {
            return Err(CallError::Media(
                "call was replaced while applying group control",
            ));
        }
        // Both directions swap the encoder source, so the peer needs an IDR before either stream
        // can safely resume.
        registry.send_video_ctl(call_id, generation, VideoControl::RequireKeyframe);
        Ok(())
    }

    #[cfg(feature = "voip-runtime")]
    async fn send_group_control(&self, call_id: &str, node: Node) -> Result<(), CallError> {
        if call_id.is_empty() {
            return Err(CallError::EmptyCallId);
        }
        self.client.send_node(node).await?;
        Ok(())
    }

    #[cfg(feature = "voip-runtime")]
    fn ensure_waiting_room_admin_if_current(
        &self,
        call_id: &str,
        generation: u64,
    ) -> Result<(), CallError> {
        let room = self
            .client
            .call_registry()
            .group_state_if_current(call_id, generation)
            .and_then(|state| state.waiting_room().cloned())
            .ok_or(CallError::Media("call has no waiting-room state"))?;
        if !room.is_admin {
            return Err(CallError::Media(
                "waiting-room control requires an administrator",
            ));
        }
        Ok(())
    }

    #[cfg(feature = "voip-runtime")]
    fn start_waiting_room_heartbeat(&self, call_id: String, call_creator: Jid, generation: u64) {
        let weak_client = self.client.self_weak.get().cloned().unwrap_or_default();
        let runtime = self.client.runtime.clone();
        let sleeper = runtime.clone();
        let heartbeat_call_id = call_id.clone();
        let task = runtime.spawn(Box::pin(async move {
            let mut consecutive_failures = 0;
            loop {
                sleeper.sleep(WAITING_ROOM_HEARTBEAT_INTERVAL).await;
                let Some(client) = weak_client.upgrade() else {
                    break;
                };
                if client
                    .call_registry()
                    .phase_if_current(&heartbeat_call_id, generation)
                    != Some(CallPhase::WaitingRoom)
                {
                    break;
                }
                let request_id = client.generate_request_id();
                let heartbeat = match build_waiting_room_heartbeat(
                    &heartbeat_call_id,
                    &call_creator,
                    &request_id,
                ) {
                    Ok(heartbeat) => heartbeat,
                    Err(error) => {
                        warn!(
                            "voip: invalid waiting-room heartbeat for call {}: {error}",
                            heartbeat_call_id
                        );
                        break;
                    }
                };
                if let Err(error) = client
                    .send_node(heartbeat)
                    .await
                {
                    consecutive_failures += 1;
                    warn!(
                        "voip: waiting-room heartbeat failed for call {} ({consecutive_failures}/{WAITING_ROOM_HEARTBEAT_MAX_CONSECUTIVE_FAILURES}): {error}",
                        heartbeat_call_id,
                    );
                    if consecutive_failures >= WAITING_ROOM_HEARTBEAT_MAX_CONSECUTIVE_FAILURES {
                        client.call_registry().send_call_event_if_current(
                            &heartbeat_call_id,
                            generation,
                            CallEvent::WaitingRoomHeartbeatFailed,
                        );
                        client
                            .call_registry()
                            .remove_if_current(&heartbeat_call_id, generation);
                        break;
                    }
                    continue;
                }
                consecutive_failures = 0;
            }
        }));
        self.client
            .call_registry()
            .set_waiting_room_task(&call_id, generation, task);
    }

    /// Terminate an active call: `<terminate>` to `peer`, then the local teardown.
    ///
    /// Holding a `CallHandle`, prefer its own `terminate()`: it knows all three identifiers, resolves
    /// `peer` to the device that answered, and reaches every device a still-ringing call rang.
    pub async fn terminate(
        &self,
        call_id: &str,
        peer: &Jid,
        call_creator: &Jid,
    ) -> Result<(), CallError> {
        #[cfg(feature = "voip-runtime")]
        {
            let pinned = self.client.call_registry().generation_of(call_id);
            // Armed before the lane, since waiting for it is cancellable too, and pinned so it can
            // only ever end the call this terminate started on. Nothing registered under this call-id
            // means there is no local call of ours to end, so this path stays teardown-free.
            let _teardown = pinned.map(|generation| LocalTeardown {
                client: self.client,
                call_id,
                generation,
            });
            // Held across the send: a replacement installed in that window would be ended by a
            // `<terminate>` the peer cannot tell apart from ours, since both carry the same call-id.
            let _transition = self.client.lock_answer_transition(call_id).await;
            // Any change since entry -- a replacement, a removal, or a first registration under a
            // call-id that had none -- belongs to a different call than the one asked to end.
            if self.client.call_registry().generation_of(call_id) != pinned {
                return Err(CallError::Media("call is no longer active"));
            }
            return self
                .terminate_inner(call_id, std::slice::from_ref(peer), call_creator, pinned)
                .await
                .1;
        }
        #[cfg(not(feature = "voip-runtime"))]
        self.terminate_inner(call_id, std::slice::from_ref(peer), call_creator, None)
            .await
            .1
    }

    /// [`terminate`](Self::terminate) restricted to one registry generation and addressed at every
    /// device that must be told, so a handle superseded by a same-call-id replacement tears down its
    /// own call instead of the replacement's.
    #[cfg(feature = "voip-runtime")]
    pub(crate) async fn terminate_for_generation(
        &self,
        call_id: &str,
        peers: &[Jid],
        call_creator: &Jid,
        generation: u64,
    ) -> TerminateDelivery {
        let (notified, result) = self
            .terminate_inner(call_id, peers, call_creator, Some(generation))
            .await;
        TerminateDelivery {
            notified,
            failure: result.err(),
        }
    }

    async fn terminate_inner(
        &self,
        call_id: &str,
        peers: &[Jid],
        call_creator: &Jid,
        _generation: Option<u64>,
    ) -> (usize, Result<(), CallError>) {
        if call_id.is_empty() {
            return (0, Err(CallError::EmptyCallId));
        }
        // Tear the local call down whatever happens to the stanzas: the app asked to hang up, and
        // neither a failed send nor a caller that drops this future mid-send (a timeout, a `select!`)
        // may leave the media task capturing or a dormant outgoing call free to attach on a late
        // relay ack. A drop guard is what survives the cancellation; it reuses the same teardown the
        // peer's `<terminate>` triggers.
        // Pinned by the caller, never resolved here: a teardown that read the registry at drop time
        // would find a same-call-id replacement and end that instead of the call the caller asked to
        // hang up. `None` is a call-id with nothing of ours registered under it, which has no local
        // side to tear down.
        #[cfg(feature = "voip-runtime")]
        let _teardown = _generation.map(|generation| LocalTeardown {
            client: self.client,
            call_id,
            generation,
        });
        let mut notified = 0usize;
        let mut result = Ok(());
        for peer in peers {
            let id = self.client.generate_request_id();
            let stanza = build_terminate(&TerminateParams {
                call_id,
                to: peer,
                id: Some(&id),
                call_creator,
                reason: None,
            });
            match self.client.send_node(stanza).await {
                Ok(()) => notified += 1,
                Err(error) if result.is_ok() => result = Err(error.into()),
                Err(_) => {}
            }
        }
        (notified, result)
    }
}

/// How much of a multi-target `<terminate>` fan-out was confirmed sent. A still-ringing call is told
/// per device, so "some devices, not all" is a real outcome and not the same as telling nobody. A
/// send that errors is unconfirmed rather than known-undelivered: the transport can fail after
/// putting bytes on the wire.
#[cfg(feature = "voip-runtime")]
pub(crate) struct TerminateDelivery {
    pub(crate) notified: usize,
    pub(crate) failure: Option<CallError>,
}

/// Local call teardown that runs even when the terminate future is cancelled mid-send.
#[cfg(feature = "voip-runtime")]
pub(crate) struct LocalTeardown<'a> {
    pub(crate) client: &'a Client,
    pub(crate) call_id: &'a str,
    /// The generation this teardown owns, pinned when the terminate started, so it can only ever end
    /// that call and never a replacement registered under the same call-id afterwards.
    pub(crate) generation: u64,
}

#[cfg(feature = "voip-runtime")]
impl Drop for LocalTeardown<'_> {
    fn drop(&mut self) {
        crate::voip::facade::terminate_call_if_current(self.client, self.call_id, self.generation);
    }
}

#[cfg(feature = "voip-runtime")]
fn normalize_call_link_token(
    token_or_url: &str,
    expected_media: CallLinkMedia,
) -> Result<String, CallError> {
    let value = token_or_url.trim();
    if value.is_empty() {
        return Err(CallError::Response(
            "call-link token is required".to_string(),
        ));
    }
    const PREFIX: &str = "https://call.whatsapp.com/";
    if let Some(path) = value.strip_prefix(PREFIX) {
        let path = path.split_once(['?', '#']).map_or(path, |(path, _)| path);
        let mut parts = path.split('/');
        let media = parts.next();
        let Some(token) = parts.next().filter(|token| !token.is_empty()) else {
            return Err(CallError::Response(
                "invalid call-link URL or media mode".to_string(),
            ));
        };
        if parts.next().is_some() || media != Some(expected_media.as_str()) {
            return Err(CallError::Response(
                "invalid call-link URL or media mode".to_string(),
            ));
        }
        return Ok(token.to_string());
    }
    if value.contains("://") || value.contains('/') {
        return Err(CallError::Response("invalid call-link token".to_string()));
    }
    Ok(value.to_string())
}

#[cfg(feature = "voip-runtime")]
#[inline(never)]
async fn execute_call_link_join_request(
    client: &Client,
    token: &str,
    media: CallLinkMedia,
    capability: &[u8],
) -> Result<CallLinkJoin, CallError> {
    let request_id = client.generate_request_id();
    let request = build_call_link_join_with_capability(token, media, &request_id, capability)
        .map_err(|error| CallError::Response(error.to_string()))?;
    execute_call_service_request(client, &request_id, request, |response| {
        parse_call_link_join_ack(response, token)
    })
    .await
}

#[cfg(feature = "voip-runtime")]
async fn execute_call_service_request<T>(
    client: &Client,
    request_id: &str,
    request: Node,
    parse: impl FnOnce(&wacore_binary::NodeRef<'_>) -> anyhow::Result<T>,
) -> Result<T, CallError> {
    let (tx, response) = futures::channel::oneshot::channel();
    let cleanup_generation = client
        .response_waiters_guard()
        .try_insert_guarded(request_id.to_string(), ResponseWaiter::Iq(tx))
        .ok_or_else(|| CallError::Response("duplicate call-service request id".to_string()))?;
    let _waiter_guard = crate::request::ResponseWaiterGuard::new(
        client.response_waiters.clone(),
        request_id.to_string(),
        cleanup_generation,
    );
    client.send_node(request).await?;
    let response =
        match wacore::runtime::timeout(&*client.runtime, CALL_SERVICE_REQUEST_TIMEOUT, response)
            .await
        {
            Ok(Ok(response)) => response,
            Ok(Err(_)) => return Err(CallError::Response("response channel closed".to_string())),
            Err(_) => return Err(CallError::ResponseTimeout),
        };
    parse(response.get()).map_err(|error| CallError::Response(error.to_string()))
}

#[cfg(test)]
mod tests {
    /// The admission snapshots the report attributes to this subsystem.
    #[cfg(feature = "voip-runtime")]
    async fn pending_link_updates(client: &Client) -> wacore::stats::CollectionStats {
        client
            .memory_report()
            .await
            .subsystem(crate::voip::collections::PENDING_LINK_UPDATES)
            .expect("the voip subsystem is attached in this build")
    }

    #[cfg(feature = "voip-runtime")]
    use super::PendingCallLinkBuffer;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[cfg(feature = "voip-runtime")]
    use std::time::Duration;

    use async_trait::async_trait;
    use bytes::Bytes;
    use wacore::handshake::NoiseCipher;
    use wacore::types::call::{CallAction, IncomingCall};
    #[cfg(feature = "voip-runtime")]
    use wacore::types::group_call::{
        CallLinkMedia, GroupCallDevice, GroupCallParticipant, GroupCallRelay,
        GroupCallRelayEndpoint, GroupCallUpdate, ScreenShareState, WaitingRoom,
    };
    #[cfg(feature = "voip-runtime")]
    use wacore::voip::{
        AudioFormat, CallEvent, CallPhase, CallSession, VideoControl, video_control_channel,
    };
    #[cfg(feature = "voip-runtime")]
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::{Jid, Server};

    #[cfg(feature = "voip-runtime")]
    use super::{
        MAX_PENDING_CALL_LINK_SATURATION_FINGERPRINTS, MAX_PENDING_CALL_LINK_TRANSITION_BYTES,
        MAX_PENDING_CALL_LINK_TRANSITIONS, WaitingRoomUserAction,
    };
    use crate::client::Client;
    #[cfg(feature = "voip-runtime")]
    use crate::client::{CallError, ResponseWaiter};

    #[cfg(feature = "voip-runtime")]
    #[test]
    fn call_link_urls_strip_query_and_fragment_without_relaxing_validation() {
        assert_eq!(
            super::normalize_call_link_token(
                "https://call.whatsapp.com/video/TEST-TOKEN?utm_source=test#join",
                CallLinkMedia::Video,
            )
            .unwrap(),
            "TEST-TOKEN"
        );
        assert!(
            super::normalize_call_link_token(
                "https://call.whatsapp.com/audio/TEST-TOKEN?x=1",
                CallLinkMedia::Video,
            )
            .is_err()
        );
        assert!(
            super::normalize_call_link_token(
                "https://call.whatsapp.com/video/?x=1",
                CallLinkMedia::Video,
            )
            .is_err()
        );
    }

    struct CountingTransport {
        count: Arc<AtomicUsize>,
    }

    #[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
    impl crate::transport::Transport for CountingTransport {
        async fn send(&self, _data: Bytes) -> Result<(), anyhow::Error> {
            self.count.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }
        async fn disconnect(&self) {}
    }

    async fn make_client_with_count() -> (Arc<Client>, Arc<AtomicUsize>) {
        let client = crate::test_utils::create_test_client().await;

        let count = Arc::new(AtomicUsize::new(0));
        let socket_transport: Arc<dyn crate::transport::Transport> = Arc::new(CountingTransport {
            count: count.clone(),
        });
        let key = [0u8; 32];
        let noise_socket = crate::socket::NoiseSocket::new(
            Arc::new(crate::runtime_impl::TokioRuntime),
            socket_transport,
            NoiseCipher::new(&key).expect("valid key"),
            NoiseCipher::new(&key).expect("valid key"),
        );
        *client.noise_socket.lock().unwrap() = Some(Arc::new(noise_socket));
        (client, count)
    }

    #[cfg(feature = "voip-runtime")]
    struct FailingTransport;

    #[cfg(feature = "voip-runtime")]
    #[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
    impl crate::transport::Transport for FailingTransport {
        async fn send(&self, _data: Bytes) -> Result<(), anyhow::Error> {
            Err(anyhow::anyhow!("transport down"))
        }
        async fn disconnect(&self) {}
    }

    #[cfg(feature = "voip-runtime")]
    async fn make_client_failing() -> Arc<Client> {
        let client = crate::test_utils::create_test_client().await;
        let socket_transport: Arc<dyn crate::transport::Transport> = Arc::new(FailingTransport);
        let key = [0u8; 32];
        let noise_socket = crate::socket::NoiseSocket::new(
            Arc::new(crate::runtime_impl::TokioRuntime),
            socket_transport,
            NoiseCipher::new(&key).expect("valid key"),
            NoiseCipher::new(&key).expect("valid key"),
        );
        *client.noise_socket.lock().unwrap() = Some(Arc::new(noise_socket));
        client
    }

    fn caller() -> Jid {
        Jid::new("111111111111111", Server::Lid)
    }

    fn call_creator() -> Jid {
        Jid::new("222222222222222", Server::Lid)
    }

    fn incoming_reject() -> IncomingCall {
        IncomingCall::new_for_test(
            caller(),
            "STANZA-ID-0001".into(),
            wacore::time::from_secs(1_766_847_151_i64).expect("valid ts"),
            CallAction::Offer {
                call_id: "CALL-ID-0001".into(),
                call_creator: caller(),
                caller_pn: None,
                caller_country_code: None,
                device_class: None,
                joinable: false,
                is_video: false,
                audio: Vec::new(),
                group_jid: None,
            },
        )
    }

    #[tokio::test]
    async fn reject_sends_stanza() {
        let (client, count) = make_client_with_count().await;
        client
            .voip()
            .reject(&incoming_reject())
            .await
            .expect("reject should send");
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn reject_call_sends_stanza_without_event_context() {
        let (client, count) = make_client_with_count().await;
        let waiter = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        let peer = caller();
        let creator = call_creator();
        client
            .voip()
            .reject_call("CALL-ID-0001", &peer, &creator)
            .await
            .expect("reject should send");
        assert_eq!(count.load(Ordering::SeqCst), 1);

        let sent = waiter.await.expect("reject stanza should be observable");
        let call = sent.as_node_ref();
        assert_eq!(
            call.attrs().optional_string("to").as_deref(),
            Some(peer.to_string().as_str())
        );
        let reject = &call.children().expect("call action")[0];
        assert_eq!(reject.tag, "reject");
        assert_eq!(
            reject.attrs().optional_string("call-id").as_deref(),
            Some("CALL-ID-0001")
        );
        assert_eq!(
            reject.attrs().optional_string("call-creator").as_deref(),
            Some(creator.to_string().as_str())
        );
        assert_eq!(
            reject.attrs().optional_string("count").as_deref(),
            Some("0")
        );
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn rejecting_an_incoming_group_offer_removes_its_ringing_generation() {
        let (client, _count) = make_client_with_count().await;
        let creator = caller();
        let call_id = "INCOMING-GROUP-CALL";
        let mut session = CallSession::new_incoming(call_id, creator.clone(), creator.clone());
        session.group = Some(
            GroupCallUpdate::builder()
                .call_id(call_id.to_string())
                .call_creator(creator.clone())
                .transaction_id(1)
                .media("audio".to_string())
                .connected_limit(32)
                .joinable(true)
                .av_upgradable(true)
                .rekey_requested(false)
                .participants(Vec::new())
                .build(),
        );
        let generation = client
            .call_registry()
            .insert_ringing_group_if_inactive(session)
            .expect("valid group snapshot")
            .expect("ringing generation");

        client
            .voip()
            .reject_call(call_id, &creator, &creator)
            .await
            .expect("reject");

        assert_ne!(
            client.call_registry().generation_of(call_id),
            Some(generation),
            "reject must reap the exact eagerly registered group offer"
        );
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn rejecting_a_stale_group_offer_event_preserves_the_replacement_generation() {
        let (client, count) = make_client_with_count().await;
        let creator = caller();
        let call_id = "REPLACED-INCOMING-GROUP-CALL";
        let update = GroupCallUpdate::builder()
            .call_id(call_id.to_string())
            .call_creator(creator.clone())
            .transaction_id(1)
            .media("audio".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(false)
            .participants(Vec::new())
            .build();
        let mut session = CallSession::new_incoming(call_id, creator.clone(), creator.clone());
        session.group = Some(update.clone());
        let stale = client
            .call_registry()
            .insert_ringing_group_if_inactive(session)
            .expect("valid group snapshot")
            .expect("ringing generation");
        let mut incoming = IncomingCall::new_for_test(
            creator.clone(),
            "STALE-GROUP-OFFER".to_string(),
            wacore::time::from_secs(1_766_847_151_i64).expect("valid ts"),
            CallAction::Offer {
                call_id: call_id.to_string(),
                call_creator: creator.clone(),
                caller_pn: None,
                caller_country_code: None,
                device_class: None,
                joinable: true,
                is_video: false,
                audio: Vec::new(),
                group_jid: None,
            },
        );
        incoming.group = Some(Box::new(update.clone()));
        incoming.set_ringing_generation(stale);

        let mut replacement = CallSession::new_incoming(call_id, creator.clone(), creator.clone());
        replacement.group = Some(update);
        let replacement = client.call_registry().insert_ringing_group(replacement);

        assert!(matches!(
            client.voip().reject(&incoming).await,
            Err(CallError::CallEndedDuringSetup)
        ));
        assert_eq!(
            count.load(Ordering::SeqCst),
            0,
            "a stale application event must not reject the replacement on the wire"
        );

        assert_eq!(
            client.call_registry().generation_of(call_id),
            Some(replacement),
            "a retained application event must not reap a newer same-id generation"
        );
        assert_eq!(
            client
                .call_registry()
                .ringing_group_generation(call_id, &creator),
            Some(replacement),
            "the newer offer must remain available for the application to answer or reject"
        );
        client
            .call_registry()
            .remove_if_current(call_id, replacement);
        client.call_registry().take_ringing(call_id);
    }

    #[tokio::test]
    async fn terminate_sends_stanza() {
        let (client, count) = make_client_with_count().await;
        client
            .voip()
            .terminate("CALL-ID-0001", &caller(), &caller())
            .await
            .expect("terminate should send");
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn terminate_aborts_the_local_call() {
        use wacore::voip::CallSession;
        let (client, _count) = make_client_with_count().await;
        let reg = client.call_registry();
        reg.insert(CallSession::new_outgoing(
            "CALL-ID-0001",
            caller(),
            caller(),
        ));
        assert_eq!(reg.active_count(), 1);
        client
            .voip()
            .terminate("CALL-ID-0001", &caller(), &caller())
            .await
            .expect("terminate should send");
        assert_eq!(
            reg.active_count(),
            0,
            "terminate must tear the local call down, not just signal the peer"
        );
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn terminate_tears_down_local_even_when_send_fails() {
        use wacore::voip::CallSession;
        let client = make_client_failing().await;
        let reg = client.call_registry();
        reg.insert(CallSession::new_outgoing(
            "CALL-ID-0001",
            caller(),
            caller(),
        ));
        assert_eq!(reg.active_count(), 1);
        let res = client
            .voip()
            .terminate("CALL-ID-0001", &caller(), &caller())
            .await;
        assert!(
            res.is_err(),
            "a failed signaling send must surface the error"
        );
        assert_eq!(
            reg.active_count(),
            0,
            "a failed signaling send must still tear the local media task down"
        );
    }

    #[tokio::test]
    async fn reject_empty_call_id_errors() {
        let (client, _count) = make_client_with_count().await;
        let mut call = incoming_reject();
        call.action = CallAction::Reject {
            call_id: String::new(),
            call_creator: caller(),
            reason: None,
        };
        assert!(client.voip().reject(&call).await.is_err());
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn local_group_controls_commit_state_events_and_screen_keyframe_gate() {
        let (client, transport) = crate::test_utils::create_iq_test_client().await;
        let own_device = Jid::new("111111111111111", Server::Lid).with_device(1);
        client
            .persistence_manager()
            .process_command(crate::store::commands::DeviceCommand::SetLid(Some(
                own_device,
            )))
            .await;
        let participant = Jid::new("111111111111111", Server::Lid);
        let creator = participant.clone();
        let call_id = "TEST-GROUP-CONTROLS";
        let registry = client.call_registry();
        let mut session =
            CallSession::new_outgoing(call_id, Jid::new(call_id, Server::Call), creator.clone());
        session.group = Some(
            GroupCallUpdate::builder()
                .call_id(call_id.to_string())
                .call_creator(creator.clone())
                .transaction_id(1)
                .media("video".to_string())
                .connected_limit(32)
                .joinable(true)
                .av_upgradable(true)
                .rekey_requested(false)
                .participants(vec![GroupCallParticipant::new(
                    participant.clone(),
                    vec![GroupCallDevice::new(participant.clone().with_device(1))],
                )])
                .build(),
        );
        let generation = registry.insert(session);
        let (event_tx, event_rx) = async_channel::bounded(4);
        let (video_tx, video_rx) = video_control_channel();
        registry.set_video_channels(call_id, generation, event_tx, video_tx, Box::new(|| {}));

        client
            .voip()
            .set_hand_raised(call_id, &creator, true)
            .await
            .expect("raise hand");
        assert!(
            registry
                .group_state(call_id)
                .expect("group state")
                .raised_hands()
                .contains(&participant)
        );
        assert!(matches!(
            event_rx.try_recv(),
            Ok(CallEvent::HandRaised {
                participant: event_participant,
                raised: true,
            }) if event_participant == participant
        ));

        assert!(
            client
                .voip()
                .set_screen_share(call_id, &creator, ScreenShareState::Started, Some(7))
                .await
                .is_err(),
            "a call without a local video plane must not advertise an unsendable screen share"
        );
        assert_eq!(
            transport.sent_count(),
            1,
            "the rejected screen-share transition must stay off the wire"
        );
        assert!(registry.set_is_video(call_id, generation, true));

        client
            .voip()
            .set_screen_share(call_id, &creator, ScreenShareState::Started, Some(7))
            .await
            .expect("start screen share");
        let share = registry
            .group_state(call_id)
            .expect("group state")
            .screen_shares()
            .get(&participant)
            .cloned()
            .expect("local screen share");
        assert_eq!(share.state, ScreenShareState::Started);
        assert_eq!(share.version, 2);
        assert_eq!(share.screen_share_id, Some(7));
        assert!(matches!(
            event_rx.try_recv(),
            Ok(CallEvent::ScreenShareChanged {
                participant: event_participant,
                screen_share,
            }) if event_participant == participant && screen_share == share
        ));
        assert_eq!(
            video_rx.try_recv(),
            Ok(VideoControl::RequireKeyframe),
            "starting a replacement screen source must re-arm the H.264 recovery gate"
        );

        client
            .voip()
            .set_screen_share(call_id, &creator, ScreenShareState::Stopped, None)
            .await
            .expect("stop screen share");
        assert!(
            registry
                .group_state(call_id)
                .expect("group state")
                .screen_shares()
                .is_empty()
        );
        assert!(matches!(
            event_rx.try_recv(),
            Ok(CallEvent::ScreenShareChanged {
                participant: event_participant,
                screen_share,
            }) if event_participant == participant
                && screen_share.state == ScreenShareState::Stopped
        ));
        assert_eq!(
            video_rx.try_recv(),
            Ok(VideoControl::RequireKeyframe),
            "returning to the camera must re-arm the H.264 recovery gate"
        );
        assert_eq!(transport.sent_count(), 3);

        let mut audio_only = registry
            .group_state_if_current(call_id, generation)
            .and_then(|state| state.snapshot().cloned())
            .expect("authoritative roster");
        audio_only.transaction_id = 2;
        audio_only.media = "audio".to_string();
        assert_eq!(
            registry.apply_group_update_if_current(audio_only, generation),
            wacore::voip::GroupStateApply::Applied
        );
        assert!(
            client
                .voip()
                .set_screen_share(call_id, &creator, ScreenShareState::Started, Some(8))
                .await
                .is_err(),
            "an authoritative audio downgrade must disable screen sharing even if local video was negotiated"
        );
        assert_eq!(
            transport.sent_count(),
            3,
            "the rejected post-downgrade transition must stay off the wire"
        );

        let replacement_creator = Jid::new("222222222222222", Server::Lid);
        let mut replacement = CallSession::new_outgoing(
            call_id,
            Jid::new(call_id, Server::Call),
            replacement_creator.clone(),
        );
        replacement.group = Some(
            GroupCallUpdate::builder()
                .call_id(call_id.to_string())
                .call_creator(replacement_creator)
                .transaction_id(1)
                .media("video".to_string())
                .connected_limit(32)
                .joinable(true)
                .av_upgradable(true)
                .rekey_requested(false)
                .participants(vec![GroupCallParticipant::new(
                    participant,
                    vec![GroupCallDevice::new(
                        Jid::new("111111111111111", Server::Lid).with_device(1),
                    )],
                )])
                .build(),
        );
        let replacement_generation = registry.insert(replacement);
        assert!(
            client
                .voip()
                .set_hand_raised(call_id, &creator, true)
                .await
                .is_err(),
            "stale creator metadata cannot mutate a replacement generation"
        );
        assert_eq!(
            transport.sent_count(),
            3,
            "a stale group identity must be rejected before signaling"
        );
        registry.remove_if_current(call_id, replacement_generation);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn local_group_controls_wait_for_the_authoritative_transition_lane() {
        let (client, transport) = crate::test_utils::create_iq_test_client().await;
        let own_device = Jid::new("111111111111111", Server::Lid).with_device(1);
        client
            .persistence_manager()
            .process_command(crate::store::commands::DeviceCommand::SetLid(Some(
                own_device,
            )))
            .await;
        let participant = Jid::new("111111111111111", Server::Lid);
        let creator = participant.clone();
        let call_id = "TEST-GROUP-CONTROL-SERIALIZATION";
        let registry = client.call_registry();
        let mut session =
            CallSession::new_outgoing(call_id, Jid::new(call_id, Server::Call), creator.clone());
        session.group = Some(
            GroupCallUpdate::builder()
                .call_id(call_id.to_string())
                .call_creator(creator.clone())
                .transaction_id(1)
                .media("video".to_string())
                .connected_limit(32)
                .joinable(true)
                .av_upgradable(true)
                .rekey_requested(false)
                .participants(vec![GroupCallParticipant::new(
                    participant,
                    vec![GroupCallDevice::new(
                        Jid::new("111111111111111", Server::Lid).with_device(1),
                    )],
                )])
                .build(),
        );
        let generation = registry.insert(session);
        assert!(registry.set_is_video(call_id, generation, true));
        let transition_lock = registry
            .group_transition_lock(call_id, generation)
            .expect("group transition lane");

        let guard = transition_lock.lock().await;
        let hand_client = client.clone();
        let hand_creator = creator.clone();
        let hand = tokio::spawn(async move {
            hand_client
                .voip()
                .set_hand_raised_for_generation(call_id, &hand_creator, generation, true)
                .await
        });
        tokio::task::yield_now().await;
        assert_eq!(
            transport.sent_count(),
            0,
            "raise-hand signaling must wait for an authoritative transition"
        );
        drop(guard);
        hand.await
            .expect("raise-hand task")
            .expect("raise-hand transition");

        let guard = transition_lock.lock().await;
        let screen_client = client.clone();
        let screen = tokio::spawn(async move {
            screen_client
                .voip()
                .set_screen_share_for_generation(
                    call_id,
                    &creator,
                    generation,
                    ScreenShareState::Started,
                    Some(7),
                )
                .await
        });
        tokio::task::yield_now().await;
        assert_eq!(
            transport.sent_count(),
            1,
            "screen-share signaling must wait for an authoritative transition"
        );
        drop(guard);
        screen
            .await
            .expect("screen-share task")
            .expect("screen-share transition");
        assert_eq!(transport.sent_count(), 2);
        registry.remove_if_current(call_id, generation);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn direct_calls_reject_group_controls_before_sending() {
        let (client, transport) = crate::test_utils::create_iq_test_client().await;
        let own_device = Jid::new("111111111111111", Server::Lid).with_device(1);
        client
            .persistence_manager()
            .process_command(crate::store::commands::DeviceCommand::SetLid(Some(
                own_device,
            )))
            .await;
        let call_id = "TEST-DIRECT-CONTROLS";
        let creator = Jid::new("111111111111111", Server::Lid);
        let generation = client.call_registry().insert(CallSession::new_outgoing(
            call_id,
            Jid::new("222222222222222", Server::Lid),
            creator.clone(),
        ));

        assert!(
            client
                .voip()
                .set_hand_raised(call_id, &creator, true)
                .await
                .is_err()
        );
        assert!(
            client
                .voip()
                .set_screen_share(call_id, &creator, ScreenShareState::Started, Some(7))
                .await
                .is_err()
        );
        assert_eq!(
            transport.sent_count(),
            0,
            "group-only controls must not be emitted for a direct call"
        );
        assert!(client.call_registry().group_state(call_id).is_none());
        client
            .call_registry()
            .remove_if_current(call_id, generation);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test(start_paused = true)]
    async fn call_link_requests_round_trip_through_bounded_response_waiters() {
        async fn wait_for_frames(
            transport: &crate::transport::mock::CapturingMockTransport,
            expected: usize,
        ) {
            for _ in 0..10_000 {
                if transport.sent_count() >= expected {
                    return;
                }
                tokio::task::yield_now().await;
            }
            panic!("timed out waiting for {expected} captured call frames");
        }

        let (client, transport) = crate::test_utils::create_iq_test_client().await;
        let own_lid = Jid::new("111111111111111", Server::Lid).with_device(1);
        client
            .persistence_manager()
            .process_command(crate::store::commands::DeviceCommand::SetLid(Some(
                own_lid.clone(),
            )))
            .await;
        let creator = Jid::new("333333333333333", Server::Lid);

        let sent = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        let create_client = client.clone();
        let create = tokio::spawn(async move {
            create_client
                .voip()
                .create_call_link(CallLinkMedia::Video)
                .await
        });
        let request = sent.await.expect("link_create request");
        let request_id = request
            .as_node_ref()
            .attrs()
            .optional_string("id")
            .expect("request id")
            .into_owned();
        crate::test_utils::answer_iq(
            &client,
            &request_id,
            &NodeBuilder::new("ack")
                .attr("class", "call")
                .attr("type", "link_create")
                .attr("id", request_id.as_str())
                .children([NodeBuilder::new("link_create")
                    .attr("token", "TEST-CALL-LINK")
                    .attr("media", "video")
                    .build()])
                .build(),
        )
        .await;
        let link = create.await.expect("create task").expect("create response");
        assert_eq!(link.token, "TEST-CALL-LINK");
        assert_eq!(link.media, CallLinkMedia::Video);

        let sent = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        let preview_client = client.clone();
        let preview = tokio::spawn(async move {
            preview_client
                .voip()
                .preview_call_link("TEST-CALL-LINK", CallLinkMedia::Video)
                .await
        });
        let request = sent.await.expect("link_query request");
        let request_id = request
            .as_node_ref()
            .attrs()
            .optional_string("id")
            .expect("request id")
            .into_owned();
        crate::test_utils::answer_iq(
            &client,
            &request_id,
            &NodeBuilder::new("ack")
                .attr("class", "call")
                .attr("type", "link_query")
                .attr("id", request_id.as_str())
                .children([NodeBuilder::new("link_query")
                    .attr("token", "TEST-CALL-LINK")
                    .attr("media", "video")
                    .attr("link_creator", creator.clone())
                    .children([NodeBuilder::new("waiting_room")
                        .attr("enabled", "1")
                        .attr("is_admin", "0")
                        .build()])
                    .build()])
                .build(),
        )
        .await;
        let preview = preview
            .await
            .expect("preview task")
            .expect("preview response");
        assert_eq!(preview.creator, creator);
        assert!(preview.waiting_room_enabled);
        assert!(!preview.is_admin);

        let sent = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        let join_client = client.clone();
        let join = tokio::spawn(async move {
            join_client
                .voip()
                .join_call_link_with_audio(
                    "TEST-CALL-LINK",
                    CallLinkMedia::Video,
                    AudioFormat::OPUS_16KHZ_60MS,
                )
                .await
        });
        let request = sent.await.expect("link_join request");
        let request_ref = request.as_node_ref();
        let action = &request_ref.children().expect("join action children")[0];
        assert_eq!(
            action
                .get_optional_child("capability")
                .expect("join capability")
                .content_bytes(),
            Some(wacore::stanza::call::CAPABILITY_STANDARD_OPUS_VIDEO_OFFER.as_slice())
        );
        let request_id = request
            .as_node_ref()
            .attrs()
            .optional_string("id")
            .expect("request id")
            .into_owned();
        crate::test_utils::answer_iq(
            &client,
            &request_id,
            &NodeBuilder::new("ack")
                .attr("class", "call")
                .attr("type", "link_join")
                .attr("id", request_id.as_str())
                .children([NodeBuilder::new("waiting_room")
                    .attr("call-id", "TEST-CALL-ID")
                    .attr("call-creator", creator.clone())
                    .attr("link-token", "TEST-CALL-LINK")
                    .attr("media", "video")
                    .attr("enabled", "1")
                    .attr("is_admin", "0")
                    .attr("transaction-id", "7")
                    .children([NodeBuilder::new("user")
                        .attr("jid", Jid::new("444444444444444", Server::Lid))
                        .attr("state", "pending")
                        .build()])
                    .build()])
                .build(),
        )
        .await;
        let join = join.await.expect("join task").expect("join response");
        assert!(join.in_waiting_room);
        assert!(join.waiting_room_enabled);
        assert_eq!(join.call_id, "TEST-CALL-ID");
        assert!(join.group.is_none());
        assert_eq!(
            client.call_registry().phase("TEST-CALL-ID"),
            Some(CallPhase::WaitingRoom)
        );
        let room = client
            .call_registry()
            .group_state("TEST-CALL-ID")
            .and_then(|state| state.waiting_room().cloned())
            .expect("waiting-room state retained");
        assert_eq!(room.transaction_id, Some(7));
        assert_eq!(room.users.len(), 1);

        wait_for_frames(&transport, 4).await;
        let immediate = crate::test_utils::decode_sent_iq(&transport, 3).await;
        let heartbeat = &immediate.get().children().expect("heartbeat action")[0];
        assert_eq!(heartbeat.tag, "heartbeat");
        assert_eq!(
            heartbeat.attrs().optional_string("type").as_deref(),
            Some("waiting_room")
        );

        tokio::time::advance(Duration::from_secs(10)).await;
        wait_for_frames(&transport, 5).await;
        let scheduled = crate::test_utils::decode_sent_iq(&transport, 4).await;
        assert_eq!(
            scheduled.get().children().expect("heartbeat action")[0].tag,
            "heartbeat"
        );

        let admitted = NodeBuilder::new("group_update")
            .attr("call-id", "TEST-CALL-ID")
            .attr("call-creator", creator)
            .children([NodeBuilder::new("group_info")
                .attr("transaction-id", "8")
                .attr("connected-limit", "32")
                .attr("media", "video")
                .children([NodeBuilder::new("user")
                    .attr("jid", own_lid.to_non_ad())
                    .attr("state", "connected")
                    .children([NodeBuilder::new("device").attr("jid", own_lid).build()])
                    .build()])
                .build()])
            .build();
        let update = wacore::stanza::group_call::parse_group_update(&admitted.as_node_ref())
            .expect("admitted group snapshot");
        assert_eq!(
            client.call_registry().apply_group_update(update),
            wacore::voip::GroupStateApply::Applied
        );
        assert_eq!(
            client.call_registry().phase("TEST-CALL-ID"),
            Some(CallPhase::Connecting)
        );
        let heartbeat_count = transport.sent_count();
        tokio::time::advance(Duration::from_secs(20)).await;
        tokio::task::yield_now().await;
        assert_eq!(
            transport.sent_count(),
            heartbeat_count,
            "admission must cancel the repeating heartbeat"
        );
        let generation = client
            .call_registry()
            .generation_of("TEST-CALL-ID")
            .expect("registered call-link generation");
        client
            .call_registry()
            .remove_if_current("TEST-CALL-ID", generation);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn call_link_preview_rejects_a_changed_token_or_media() {
        let (client, _transport) = crate::test_utils::create_iq_test_client().await;
        let creator = Jid::new("333333333333333", Server::Lid);
        for (response_token, response_media) in
            [("OTHER-CALL-LINK", "video"), ("TEST-CALL-LINK", "audio")]
        {
            let sent = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
            let preview_client = client.clone();
            let preview = tokio::spawn(async move {
                preview_client
                    .voip()
                    .preview_call_link("TEST-CALL-LINK", CallLinkMedia::Video)
                    .await
            });
            let request = sent.await.expect("link_query request");
            let request_id = request
                .as_node_ref()
                .attrs()
                .optional_string("id")
                .expect("request id")
                .into_owned();
            crate::test_utils::answer_iq(
                &client,
                &request_id,
                &NodeBuilder::new("ack")
                    .attr("class", "call")
                    .attr("type", "link_query")
                    .attr("id", request_id.as_str())
                    .children([NodeBuilder::new("link_query")
                        .attr("token", response_token)
                        .attr("media", response_media)
                        .attr("link_creator", creator.clone())
                        .build()])
                    .build(),
            )
            .await;
            assert!(matches!(
                preview.await.expect("preview task"),
                Err(CallError::Response(message))
                    if message == "call-link preview changed the requested link identity"
            ));
        }
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn call_link_creation_rejects_a_changed_media_mode() {
        let (client, _transport) = crate::test_utils::create_iq_test_client().await;
        let sent = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        let create_client = client.clone();
        let create = tokio::spawn(async move {
            create_client
                .voip()
                .create_call_link(CallLinkMedia::Video)
                .await
        });
        let request = sent.await.expect("link_create request");
        let request_id = request
            .as_node_ref()
            .attrs()
            .optional_string("id")
            .expect("request id")
            .into_owned();
        crate::test_utils::answer_iq(
            &client,
            &request_id,
            &NodeBuilder::new("ack")
                .attr("class", "call")
                .attr("type", "link_create")
                .attr("id", request_id.as_str())
                .children([NodeBuilder::new("link_create")
                    .attr("token", "TEST-CALL-LINK")
                    .attr("media", "audio")
                    .build()])
                .build(),
        )
        .await;
        assert!(matches!(
            create.await.expect("create task"),
            Err(CallError::Response(message))
                if message == "call-link creation changed the requested media mode"
        ));
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn approval_ack_cannot_commit_to_a_replacement_generation() {
        let (client, _transport) = crate::test_utils::create_iq_test_client().await;
        let call_id = "TEST-APPROVAL-GENERATION";
        let creator = Jid::new("333333333333333", Server::Lid);
        let registry = client.call_registry();
        let first = registry
            .insert_call_link_checked(CallSession::new_outgoing(
                call_id,
                Jid::new(call_id, Server::Call),
                creator.clone(),
            ))
            .expect("valid call-link session");
        assert_eq!(
            registry.apply_waiting_room(
                WaitingRoom::builder()
                    .call_id(call_id.to_string())
                    .call_creator(creator.clone())
                    .link_token("TEST-CALL-LINK".to_string())
                    .media(CallLinkMedia::Audio)
                    .enabled(false)
                    .is_admin(true)
                    .transaction_id(1)
                    .users(Vec::new())
                    .build(),
            ),
            wacore::voip::GroupStateApply::Applied
        );

        let sent = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        let request_client = client.clone();
        let request_creator = creator.clone();
        let toggle = tokio::spawn(async move {
            request_client
                .voip()
                .set_approval_required(call_id, &request_creator, true)
                .await
        });
        let request = sent.await.expect("waiting-room toggle request");
        let request_id = request
            .as_node_ref()
            .attrs()
            .optional_string("id")
            .expect("request id")
            .into_owned();

        let replacement = registry.insert(CallSession::new_outgoing(
            call_id,
            Jid::new(call_id, Server::Call),
            creator,
        ));
        assert_ne!(replacement, first);
        crate::test_utils::answer_iq(
            &client,
            &request_id,
            &NodeBuilder::new("ack")
                .attr("class", "call")
                .attr("type", "waiting_room_toggle")
                .attr("id", request_id.as_str())
                .build(),
        )
        .await;

        assert!(matches!(
            toggle.await.expect("toggle task"),
            Err(CallError::Media(
                "call was replaced while applying group control"
            ))
        ));
        assert!(
            registry.group_state(call_id).is_none(),
            "the stale ACK must not synthesize waiting-room state on the replacement"
        );
        registry.remove_if_current(call_id, replacement);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn approval_toggle_serializes_with_authoritative_waiting_room_updates() {
        let (client, _transport) = crate::test_utils::create_iq_test_client().await;
        let call_id = "TEST-APPROVAL-SERIALIZATION";
        let creator = Jid::new("333333333333333", Server::Lid);
        let registry = client.call_registry();
        let generation = registry
            .insert_call_link_checked(CallSession::new_outgoing(
                call_id,
                Jid::new(call_id, Server::Call),
                creator.clone(),
            ))
            .expect("valid call-link session");
        let room = |transaction_id, enabled| {
            WaitingRoom::builder()
                .call_id(call_id.to_string())
                .call_creator(creator.clone())
                .link_token("TEST-CALL-LINK".to_string())
                .media(CallLinkMedia::Audio)
                .enabled(enabled)
                .is_admin(true)
                .transaction_id(transaction_id)
                .users(Vec::new())
                .build()
        };
        assert_eq!(
            registry.apply_waiting_room(room(1, false)),
            wacore::voip::GroupStateApply::Applied
        );
        let transition_lock = registry
            .group_transition_lock(call_id, generation)
            .expect("active group transition");

        let sent = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        let request_client = client.clone();
        let request_creator = creator.clone();
        let toggle = tokio::spawn(async move {
            request_client
                .voip()
                .set_approval_required_for_generation(call_id, &request_creator, generation, true)
                .await
        });
        let request = sent.await.expect("waiting-room toggle request");
        let request_id = request
            .as_node_ref()
            .attrs()
            .optional_string("id")
            .expect("request id")
            .into_owned();

        let update_registry = registry.clone();
        let update = room(2, false);
        let authoritative = tokio::spawn(async move {
            let _guard = transition_lock.lock().await;
            update_registry.apply_waiting_room_if_current(update, generation)
        });
        tokio::task::yield_now().await;
        assert!(
            !authoritative.is_finished(),
            "the authoritative snapshot must wait for the toggle ACK and local commit"
        );

        crate::test_utils::answer_iq(
            &client,
            &request_id,
            &NodeBuilder::new("ack")
                .attr("class", "call")
                .attr("type", "waiting_room_toggle")
                .attr("id", request_id.as_str())
                .build(),
        )
        .await;
        toggle.await.expect("toggle task").expect("toggle response");
        assert_eq!(
            authoritative.await.expect("authoritative update task"),
            wacore::voip::GroupStateApply::Applied
        );
        assert!(
            registry
                .group_state_if_current(call_id, generation)
                .and_then(|state| state.waiting_room().cloned())
                .is_some_and(|room| room.transaction_id == Some(2) && !room.enabled),
            "the newer authoritative snapshot must win after the serialized local toggle"
        );
        registry.remove_if_current(call_id, generation);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn waiting_room_user_acks_are_bound_to_the_originating_generation() {
        let user = Jid::new("444444444444444", Server::Lid);
        for (index, action) in [WaitingRoomUserAction::Admit, WaitingRoomUserAction::Deny]
            .into_iter()
            .enumerate()
        {
            let (client, _transport) = crate::test_utils::create_iq_test_client().await;
            let call_id = format!("TEST-WAITING-ACTION-{index}");
            let creator = Jid::new("333333333333333", Server::Lid);
            let registry = client.call_registry();
            let first = registry
                .insert_call_link_checked(CallSession::new_outgoing(
                    &call_id,
                    Jid::new(&call_id, Server::Call),
                    creator.clone(),
                ))
                .expect("valid call-link session");
            assert_eq!(
                registry.apply_waiting_room(
                    WaitingRoom::builder()
                        .call_id(call_id.clone())
                        .call_creator(creator.clone())
                        .link_token("TEST-CALL-LINK".to_string())
                        .media(CallLinkMedia::Audio)
                        .enabled(true)
                        .is_admin(true)
                        .transaction_id(1)
                        .users(Vec::new())
                        .build(),
                ),
                wacore::voip::GroupStateApply::Applied
            );

            let sent = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
            let request_client = client.clone();
            let request_call_id = call_id.clone();
            let request_creator = creator.clone();
            let request_user = user.clone();
            let control = tokio::spawn(async move {
                match action {
                    WaitingRoomUserAction::Admit => {
                        request_client
                            .voip()
                            .admit_waiting_user_for_generation(
                                &request_call_id,
                                &request_creator,
                                first,
                                &request_user,
                            )
                            .await
                    }
                    WaitingRoomUserAction::Deny => {
                        request_client
                            .voip()
                            .deny_waiting_user_for_generation(
                                &request_call_id,
                                &request_creator,
                                first,
                                &request_user,
                            )
                            .await
                    }
                }
            });
            let request = sent.await.expect("waiting-room user request");
            let request_id = request
                .as_node_ref()
                .attrs()
                .optional_string("id")
                .expect("request id")
                .into_owned();

            let replacement = registry.insert(CallSession::new_outgoing(
                &call_id,
                Jid::new(&call_id, Server::Call),
                creator,
            ));
            assert_ne!(replacement, first);
            let action_type = match action {
                WaitingRoomUserAction::Admit => "waiting_room_admit",
                WaitingRoomUserAction::Deny => "waiting_room_deny",
            };
            crate::test_utils::answer_iq(
                &client,
                &request_id,
                &NodeBuilder::new("ack")
                    .attr("class", "call")
                    .attr("type", action_type)
                    .attr("id", request_id.as_str())
                    .build(),
            )
            .await;

            assert!(matches!(
                control.await.expect("waiting-room control task"),
                Err(CallError::Media(
                    "call was replaced while applying group control"
                ))
            ));
            registry.remove_if_current(&call_id, replacement);
        }
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn call_link_admission_is_buffered_until_ack_registration() {
        let (client, _transport) = crate::test_utils::create_iq_test_client().await;
        let creator = Jid::new("333333333333333", Server::Lid);
        let call_id = "BUFFERED-ADMISSION";
        let local_device = Jid::new("111111111111111", Server::Lid).with_device(1);
        let mut participant = GroupCallParticipant::new(
            local_device.to_non_ad(),
            vec![GroupCallDevice::new(local_device.clone())],
        );
        participant.state = Some("connected".to_string());
        let update = GroupCallUpdate::builder()
            .call_id(call_id.to_string())
            .call_creator(creator.clone())
            .transaction_id(8)
            .media("audio".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(false)
            .participants(vec![participant])
            .build();
        let creator_sender = creator.clone().with_device(1);

        let pending = client.begin_call_link_join();
        assert_eq!(
            client.buffer_pending_call_link_update(&update, &creator_sender),
            PendingCallLinkBuffer::Buffered,
            "the creator's admission update must survive until the ACK registers its call id"
        );
        assert_eq!(
            client.buffer_pending_call_link_update(
                &update,
                &Jid::new("999999999999999", Server::Lid)
            ),
            PendingCallLinkBuffer::NotPending,
            "an unrelated sender cannot populate the pre-registration buffer"
        );
        let pending_memory = pending_link_updates(&client).await;
        assert_eq!(pending_memory.entries, 1);
        assert!(pending_memory.bytes > 0);

        let mut session =
            CallSession::new_outgoing(call_id, Jid::new(call_id, Server::Call), creator);
        let _ = session.transition_to(CallPhase::Calling);
        let _ = session.transition_to(CallPhase::WaitingRoom);
        let generation = client
            .register_call_link_session(session, None, CallLinkMedia::Audio, "TEST-CALL-LINK")
            .await
            .expect("valid buffered admission");
        assert!(client.call_registry().set_group_invite_self_device(
            call_id,
            generation,
            GroupCallDevice::new(local_device).with_capability(1, [1]),
        ));
        assert_eq!(
            client.call_registry().phase_if_current(call_id, generation),
            Some(CallPhase::Connecting),
            "consuming the buffered admission must perform the waiting-room transition"
        );
        assert_eq!(
            client
                .call_registry()
                .group_state_if_current(call_id, generation)
                .and_then(|state| { state.snapshot().map(|snapshot| snapshot.transaction_id) }),
            Some(8)
        );
        assert_eq!(pending_link_updates(&client).await.entries, 0);
        let mut later = update;
        later.transaction_id = 9;
        assert_eq!(
            client.buffer_pending_call_link_update(&later, &creator_sender),
            PendingCallLinkBuffer::NotPending,
            "an already registered generation must dispatch instead of entering an orphan buffer"
        );

        drop(pending);
        client
            .call_registry()
            .remove_if_current(call_id, generation);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn call_link_termination_before_registration_rejects_the_join() {
        let (client, _transport) = crate::test_utils::create_iq_test_client().await;
        let creator = Jid::new("333333333333333", Server::Lid);
        let call_id = "TERMINATED-CALL-LINK";
        let _pending = client.begin_call_link_join();
        assert!(
            client
                .retain_or_apply_pending_call_link_terminate(
                    call_id,
                    &creator,
                    &creator.clone().with_device(1),
                )
                .await
        );

        let mut session =
            CallSession::new_outgoing(call_id, Jid::new(call_id, Server::Call), creator);
        let _ = session.transition_to(CallPhase::Calling);
        let _ = session.transition_to(CallPhase::WaitingRoom);
        assert_eq!(
            client
                .register_call_link_session(session, None, CallLinkMedia::Audio, "TEST-CALL-LINK")
                .await,
            Err(wacore::voip::GroupStateApply::InvalidSnapshot)
        );
        assert_eq!(
            client.call_registry().generation_of(call_id),
            None,
            "a terminal control that overtakes registration must prevent publication"
        );
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn call_link_termination_removes_a_generation_that_won_registration() {
        let (client, _transport) = crate::test_utils::create_iq_test_client().await;
        let creator = Jid::new("333333333333333", Server::Lid);
        let call_id = "REGISTERED-THEN-TERMINATED-CALL-LINK";
        let _pending = client.begin_call_link_join();
        let mut session =
            CallSession::new_outgoing(call_id, Jid::new(call_id, Server::Call), creator.clone());
        let _ = session.transition_to(CallPhase::Calling);
        let _ = session.transition_to(CallPhase::WaitingRoom);
        let generation = client
            .register_call_link_session(session, None, CallLinkMedia::Audio, "TEST-CALL-LINK")
            .await
            .expect("registration wins the answer-transition lane");

        assert!(
            client
                .retain_or_apply_pending_call_link_terminate(
                    call_id,
                    &creator,
                    &creator.with_device(1),
                )
                .await
        );
        assert_eq!(
            client.call_registry().generation_of(call_id),
            None,
            "the terminal control must remove the just-published generation"
        );
        assert!(
            !client.call_registry().is_current(call_id, generation),
            "the removed generation cannot remain active"
        );
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn call_link_epoch_before_registration_is_replayed_to_the_generation() {
        let (client, _transport) = crate::test_utils::create_iq_test_client().await;
        let creator = Jid::new("333333333333333", Server::Lid);
        let call_id = "EPOCH-CALL-LINK";
        let _pending = client.begin_call_link_join();
        assert_eq!(
            client.buffer_pending_call_link_epoch(
                call_id,
                &creator,
                &creator.clone().with_device(1),
                7,
                &[7; 32],
            ),
            PendingCallLinkBuffer::Buffered
        );

        let mut session =
            CallSession::new_outgoing(call_id, Jid::new(call_id, Server::Call), creator);
        let _ = session.transition_to(CallPhase::Calling);
        let _ = session.transition_to(CallPhase::Connecting);
        let generation = client
            .register_call_link_session(session, None, CallLinkMedia::Audio, "TEST-CALL-LINK")
            .await
            .expect("valid call-link generation");
        assert_eq!(
            client
                .call_registry()
                .pending_group_epoch_transaction_if_current(call_id, generation),
            Some(7),
            "the decrypted epoch must survive until the media driver attaches"
        );
        client
            .call_registry()
            .remove_if_current(call_id, generation);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn staged_call_link_epoch_and_termination_revalidate_provenance() {
        let (client, _transport) = crate::test_utils::create_iq_test_client().await;
        let call_id = "PROVENANCE-CALL-LINK";
        let creator = Jid::new("333333333333333", Server::Lid);
        let asserted_creator = Jid::new("999999999999999", Server::Lid);
        let asserted_sender = asserted_creator.clone().with_device(7);
        let _pending = client.begin_call_link_join();

        assert_eq!(
            client.buffer_pending_call_link_epoch(
                call_id,
                &asserted_creator,
                &asserted_sender,
                7,
                &[7; 32],
            ),
            PendingCallLinkBuffer::Buffered
        );
        assert_eq!(
            client
                .buffer_pending_call_link_terminate(call_id, &asserted_creator, &asserted_sender,),
            PendingCallLinkBuffer::Buffered
        );

        let mut session =
            CallSession::new_outgoing(call_id, Jid::new(call_id, Server::Call), creator);
        let _ = session.transition_to(CallPhase::Calling);
        let _ = session.transition_to(CallPhase::Connecting);
        let generation = client
            .register_call_link_session(session, None, CallLinkMedia::Audio, "TEST-CALL-LINK")
            .await
            .expect("unauthorized staged controls must not abort the legitimate join");
        assert!(
            client.call_registry().is_current(call_id, generation),
            "the unauthorized terminal marker must be ignored after registration"
        );
        assert_eq!(
            client
                .call_registry()
                .pending_group_epoch_transaction_if_current(call_id, generation),
            None,
            "the unauthorized epoch must not enter the registered generation"
        );
        client
            .call_registry()
            .remove_if_current(call_id, generation);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn concurrent_call_link_join_waits_for_the_unknown_call_id_lane() {
        use std::time::Duration;

        let (client, _transport) = crate::test_utils::create_iq_test_client().await;
        client
            .persistence_manager()
            .process_command(crate::store::commands::DeviceCommand::SetLid(Some(
                Jid::new("111111111111111", Server::Lid).with_device(1),
            )))
            .await;

        let first_request = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        let first_client = client.clone();
        let first = tokio::spawn(async move {
            first_client
                .voip()
                .join_call_link_registration_with_audio(
                    "FIRST-CALL-LINK",
                    CallLinkMedia::Audio,
                    AudioFormat::OPUS_16KHZ_60MS,
                )
                .await
        });
        first_request.await.expect("first link_join request");

        let second_request = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        let second_client = client.clone();
        let second = tokio::spawn(async move {
            second_client
                .voip()
                .join_call_link_registration_with_audio(
                    "SECOND-CALL-LINK",
                    CallLinkMedia::Audio,
                    AudioFormat::OPUS_16KHZ_60MS,
                )
                .await
        });
        assert!(
            tokio::time::timeout(Duration::from_millis(25), second_request)
                .await
                .is_err(),
            "a second unknown-call-id join must wait instead of sharing the first join's buffer"
        );

        let released_request = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        first.abort();
        let _ = first.await;
        tokio::time::timeout(Duration::from_secs(1), released_request)
            .await
            .expect("the second join lane should be released")
            .expect("second link_join request");
        second.abort();
        let _ = second.await;
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn registered_call_link_releases_the_unknown_id_lane_before_heartbeat() {
        use std::time::Duration;
        use wacore::handshake::NoiseCipher;

        struct GatedTransport {
            started: async_channel::Sender<()>,
            release: async_channel::Receiver<()>,
            gate_next_send: std::sync::atomic::AtomicBool,
        }

        #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
        #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
        impl crate::transport::Transport for GatedTransport {
            async fn send(&self, _data: Bytes) -> Result<(), anyhow::Error> {
                if self.gate_next_send.swap(false, Ordering::AcqRel) {
                    self.started
                        .send(())
                        .await
                        .map_err(|_| anyhow::anyhow!("heartbeat observer closed"))?;
                    self.release
                        .recv()
                        .await
                        .map_err(|_| anyhow::anyhow!("heartbeat gate closed"))?;
                }
                Ok(())
            }

            async fn disconnect(&self) {}
        }

        let (client, _transport) = crate::test_utils::create_iq_test_client().await;
        client
            .persistence_manager()
            .process_command(crate::store::commands::DeviceCommand::SetLid(Some(
                Jid::new("111111111111111", Server::Lid).with_device(1),
            )))
            .await;
        let creator = Jid::new("333333333333333", Server::Lid);
        let first_request = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        let first_client = client.clone();
        let first = tokio::spawn(async move {
            first_client
                .voip()
                .join_call_link_registration_with_audio(
                    "FIRST-CALL-LINK",
                    CallLinkMedia::Audio,
                    AudioFormat::OPUS_16KHZ_60MS,
                )
                .await
        });
        let request = first_request.await.expect("first link_join request");
        let request_id = request
            .as_node_ref()
            .attrs()
            .optional_string("id")
            .expect("request id")
            .into_owned();
        let (started_tx, started_rx) = async_channel::bounded(1);
        let (release_tx, release_rx) = async_channel::bounded(1);
        let gated_socket = crate::socket::NoiseSocket::new(
            Arc::new(crate::runtime_impl::TokioRuntime),
            Arc::new(GatedTransport {
                started: started_tx,
                release: release_rx,
                gate_next_send: std::sync::atomic::AtomicBool::new(true),
            }),
            NoiseCipher::new(&[0u8; 32]).expect("valid key"),
            NoiseCipher::new(&[0u8; 32]).expect("valid key"),
        );
        *client.noise_socket.lock().unwrap() = Some(Arc::new(gated_socket));
        crate::test_utils::answer_iq(
            &client,
            &request_id,
            &NodeBuilder::new("ack")
                .attr("class", "call")
                .attr("type", "link_join")
                .attr("id", request_id.as_str())
                .children([NodeBuilder::new("waiting_room")
                    .attr("call-id", "FIRST-CALL-ID")
                    .attr("call-creator", creator.clone())
                    .attr("link-token", "FIRST-CALL-LINK")
                    .attr("media", "audio")
                    .attr("enabled", "1")
                    .attr("is_admin", "0")
                    .attr("transaction-id", "1")
                    .build()])
                .build(),
        )
        .await;
        started_rx
            .recv()
            .await
            .expect("first waiting-room heartbeat entered the gated transport");

        let second_request = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        let second_client = client.clone();
        let second = tokio::spawn(async move {
            second_client
                .voip()
                .join_call_link_registration_with_audio(
                    "SECOND-CALL-LINK",
                    CallLinkMedia::Audio,
                    AudioFormat::OPUS_16KHZ_60MS,
                )
                .await
        });
        let request = tokio::time::timeout(Duration::from_secs(1), second_request)
            .await
            .expect("registration must release the lane while the heartbeat remains gated")
            .expect("second link_join request");
        assert_eq!(
            request
                .as_node_ref()
                .children()
                .expect("second request action")[0]
                .tag,
            "link_join"
        );
        let second_sender = creator.clone().with_device(1);
        let second_update = GroupCallUpdate::builder()
            .call_id("SECOND-CALL-ID".to_string())
            .call_creator(creator)
            .transaction_id(1)
            .media("audio".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(false)
            .participants(Vec::new())
            .build();
        assert_eq!(
            client.buffer_pending_call_link_update(&second_update, &second_sender),
            PendingCallLinkBuffer::Buffered,
            "the second join must not inherit the first call id's provisional binding"
        );

        release_tx.send(()).await.expect("release heartbeat send");
        second.abort();
        let _ = second.await;
        let registration = first
            .await
            .expect("first join task")
            .expect("first waiting-room registration");
        drop(registration);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn pending_call_link_transitions_are_bounded_by_retained_bytes() {
        let (client, _transport) = crate::test_utils::create_iq_test_client().await;
        let creator = Jid::new("333333333333333", Server::Lid);
        let sender = creator.clone().with_device(1);
        let _pending = client.begin_call_link_join();
        let chunk = MAX_PENDING_CALL_LINK_TRANSITION_BYTES / 3;
        let mut accepted = 0;
        for index in 0..4 {
            let participant = GroupCallParticipant::new(
                creator.clone(),
                vec![GroupCallDevice::new(sender.clone()).with_capability(1, vec![7; chunk])],
            );
            let update = GroupCallUpdate::builder()
                .call_id(format!("BUFFERED-BYTES-{index}"))
                .call_creator(creator.clone())
                .transaction_id(1)
                .media("audio".to_string())
                .connected_limit(32)
                .joinable(true)
                .av_upgradable(true)
                .rekey_requested(false)
                .participants(vec![participant])
                .build();
            accepted += usize::from(
                client.buffer_pending_call_link_update(&update, &sender)
                    == PendingCallLinkBuffer::Buffered,
            );
        }
        let stats = pending_link_updates(&client).await;
        assert!(
            accepted < 4,
            "the aggregate byte budget must reject excess staged snapshots"
        );
        assert!(
            stats.bytes <= MAX_PENDING_CALL_LINK_TRANSITION_BYTES as u64,
            "retained staged snapshots must stay within the aggregate byte budget"
        );

        let oversized = GroupCallUpdate::builder()
            .call_id("BUFFERED-OVERSIZED".to_string())
            .call_creator(creator.clone())
            .transaction_id(1)
            .media("audio".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(false)
            .participants(vec![GroupCallParticipant::new(
                creator,
                vec![
                    GroupCallDevice::new(sender.clone())
                        .with_capability(1, vec![7; MAX_PENDING_CALL_LINK_TRANSITION_BYTES]),
                ],
            )])
            .build();
        assert_eq!(
            client.buffer_pending_call_link_update(&oversized, &sender),
            PendingCallLinkBuffer::Saturated,
            "one staged snapshot cannot consume the entire retained-byte budget"
        );
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn saturated_call_link_admission_fails_instead_of_falling_back() {
        let (client, _transport) = crate::test_utils::create_iq_test_client().await;
        let creator = Jid::new("333333333333333", Server::Lid);
        let sender = creator.clone().with_device(1);
        let call_id = "SATURATED-ADMISSION";
        let _pending = client.begin_call_link_join();
        let update = GroupCallUpdate::builder()
            .call_id(call_id.to_string())
            .call_creator(creator.clone())
            .transaction_id(1)
            .media("audio".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(false)
            .participants(vec![GroupCallParticipant::new(
                creator.clone(),
                vec![
                    GroupCallDevice::new(sender.clone())
                        .with_capability(1, vec![7; MAX_PENDING_CALL_LINK_TRANSITION_BYTES]),
                ],
            )])
            .build();
        assert_eq!(
            client.buffer_pending_call_link_update(&update, &sender),
            PendingCallLinkBuffer::Saturated,
            "the admission is handled locally even when it exceeds the staging budget"
        );

        let mut session =
            CallSession::new_outgoing(call_id, Jid::new(call_id, Server::Call), creator);
        let _ = session.transition_to(CallPhase::Calling);
        let _ = session.transition_to(CallPhase::WaitingRoom);
        assert_eq!(
            client
                .register_call_link_session(session, None, CallLinkMedia::Audio, "TEST-CALL-LINK",)
                .await,
            Err(wacore::voip::GroupStateApply::InvalidSnapshot),
            "a saturated join must fail rather than wait forever for a discarded admission"
        );
        assert_eq!(client.call_registry().generation_of(call_id), None);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn saturated_admission_is_retained_when_unrelated_call_ids_fill_the_payload_budget() {
        let (client, _transport) = crate::test_utils::create_iq_test_client().await;
        let creator = Jid::new("333333333333333", Server::Lid);
        let sender = creator.clone().with_device(1);
        let _pending = client.begin_call_link_join();
        for index in 0..MAX_PENDING_CALL_LINK_TRANSITIONS {
            let unrelated_creator = Jid::new(format!("55555555555{index:04}"), Server::Lid);
            let unrelated_sender = unrelated_creator.clone().with_device(1);
            assert_eq!(
                client.buffer_pending_call_link_terminate(
                    &format!("UNRELATED-{index}"),
                    &unrelated_creator,
                    &unrelated_sender,
                ),
                PendingCallLinkBuffer::Buffered
            );
        }

        let call_id = "SATURATED-AFTER-UNRELATED";
        let oversized = GroupCallUpdate::builder()
            .call_id(call_id.to_string())
            .call_creator(creator.clone())
            .transaction_id(1)
            .media("audio".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(false)
            .participants(vec![GroupCallParticipant::new(
                creator.clone(),
                vec![
                    GroupCallDevice::new(sender.clone())
                        .with_capability(1, vec![7; MAX_PENDING_CALL_LINK_TRANSITION_BYTES]),
                ],
            )])
            .build();
        assert_eq!(
            client.buffer_pending_call_link_update(&oversized, &sender),
            PendingCallLinkBuffer::Saturated
        );

        let mut session =
            CallSession::new_outgoing(call_id, Jid::new(call_id, Server::Call), creator);
        let _ = session.transition_to(CallPhase::Calling);
        let _ = session.transition_to(CallPhase::WaitingRoom);
        assert_eq!(
            client
                .register_call_link_session(session, None, CallLinkMedia::Audio, "TEST-CALL-LINK",)
                .await,
            Err(wacore::voip::GroupStateApply::InvalidSnapshot),
            "binding the ACK must retain the exact overflow identity outside the full payload map"
        );
        assert_eq!(client.call_registry().generation_of(call_id), None);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn unrelated_saturation_does_not_reject_the_valid_call_link_join() {
        let (client, _transport) = crate::test_utils::create_iq_test_client().await;
        let unrelated_creator = Jid::new("333333333333333", Server::Lid);
        let unrelated_sender = unrelated_creator.clone().with_device(1);
        let _pending = client.begin_call_link_join();
        for index in 0..MAX_PENDING_CALL_LINK_TRANSITIONS {
            assert!(
                client
                    .buffer_pending_call_link_terminate(
                        &format!("UNRELATED-{index}"),
                        &unrelated_creator,
                        &unrelated_sender,
                    )
                    .suppresses_dispatch()
            );
        }
        let mut oversized = GroupCallUpdate::builder()
            .call_id("UNRELATED-SATURATED-CALL-0".to_string())
            .call_creator(unrelated_creator.clone())
            .transaction_id(1)
            .media("audio".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(false)
            .participants(vec![GroupCallParticipant::new(
                unrelated_creator,
                vec![
                    GroupCallDevice::new(unrelated_sender.clone())
                        .with_capability(1, vec![7; MAX_PENDING_CALL_LINK_TRANSITION_BYTES]),
                ],
            )])
            .build();
        for index in 0..=MAX_PENDING_CALL_LINK_SATURATION_FINGERPRINTS {
            oversized.call_id = format!("UNRELATED-SATURATED-CALL-{index}");
            assert_eq!(
                client.buffer_pending_call_link_update(&oversized, &unrelated_sender),
                PendingCallLinkBuffer::Saturated,
                "every oversized unrelated identity must remain handled locally"
            );
        }

        let call_id = "VALID-CALL-LINK";
        let call_creator = Jid::new("444444444444444", Server::Lid);
        let ack = NodeBuilder::new("ack")
            .attr("class", "call")
            .attr("type", "link_join")
            .children([NodeBuilder::new("waiting_room")
                .attr("call-id", call_id)
                .build()])
            .build();
        client.bind_pending_call_link_join_ack(&ack.as_node_ref());
        assert!(
            client.prepare_pending_call_link_join_retry(call_id),
            "exhausted pre-ACK identity metadata requires one exact-call refresh"
        );
        assert_eq!(
            pending_link_updates(&client).await.entries,
            0,
            "binding the ACK must discard every unrelated candidate bucket"
        );
        assert_eq!(
            client.buffer_pending_call_link_terminate(
                "LATE-UNRELATED",
                &unrelated_sender.to_non_ad(),
                &unrelated_sender,
            ),
            PendingCallLinkBuffer::NotPending,
            "later unrelated controls cannot consume the bound join's budget"
        );
        let mut participant = GroupCallParticipant::new(
            call_creator.clone(),
            vec![GroupCallDevice::new(call_creator.clone().with_device(1))],
        );
        participant.state = Some("connected".to_string());
        let admitted = GroupCallUpdate::builder()
            .call_id(call_id.to_string())
            .call_creator(call_creator.clone())
            .transaction_id(1)
            .media("audio".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(false)
            .participants(vec![participant])
            .build();
        assert_eq!(
            client
                .buffer_pending_call_link_update(&admitted, &call_creator.clone().with_device(1),),
            PendingCallLinkBuffer::Buffered
        );
        let mut session =
            CallSession::new_outgoing(call_id, Jid::new(call_id, Server::Call), call_creator);
        let _ = session.transition_to(CallPhase::Calling);
        let _ = session.transition_to(CallPhase::WaitingRoom);
        let generation = client
            .register_call_link_session(session, None, CallLinkMedia::Audio, "VALID-CALL-LINK")
            .await
            .expect("an unrelated saturated control cannot abort the ACK's actual call id");
        assert!(
            client.call_registry().is_current(call_id, generation),
            "the legitimate call-link generation must remain registered"
        );
        assert_eq!(
            client
                .call_registry()
                .group_state_if_current(call_id, generation)
                .and_then(|state| state.snapshot().map(|snapshot| snapshot.transaction_id)),
            Some(1)
        );
        client
            .call_registry()
            .remove_if_current(call_id, generation);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn ambiguous_pre_ack_saturation_retries_after_binding_the_exact_call_id() {
        let (client, _transport) = crate::test_utils::create_iq_test_client().await;
        client
            .persistence_manager()
            .process_command(crate::store::commands::DeviceCommand::SetLid(Some(
                Jid::new("111111111111111", Server::Lid).with_device(1),
            )))
            .await;
        let creator = Jid::new("333333333333333", Server::Lid);
        let sender = creator.clone().with_device(1);
        let sent = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        let join_client = client.clone();
        let join = tokio::spawn(async move {
            join_client
                .voip()
                .join_call_link_registration_with_audio(
                    "RETRIED-CALL-LINK",
                    CallLinkMedia::Audio,
                    AudioFormat::OPUS_16KHZ_60MS,
                )
                .await
        });
        let first_request = sent.await.expect("initial link_join request");

        for index in 0..MAX_PENDING_CALL_LINK_TRANSITIONS {
            assert_eq!(
                client.buffer_pending_call_link_terminate(
                    &format!("UNRELATED-FILLED-{index}"),
                    &creator,
                    &sender,
                ),
                PendingCallLinkBuffer::Buffered
            );
        }
        let mut oversized = GroupCallUpdate::builder()
            .call_id("UNRELATED-OVERFLOW-0".to_string())
            .call_creator(creator.clone())
            .transaction_id(1)
            .media("audio".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(false)
            .participants(vec![GroupCallParticipant::new(
                creator.clone(),
                vec![
                    GroupCallDevice::new(sender.clone())
                        .with_capability(1, vec![7; MAX_PENDING_CALL_LINK_TRANSITION_BYTES]),
                ],
            )])
            .build();
        for index in 0..=MAX_PENDING_CALL_LINK_SATURATION_FINGERPRINTS {
            oversized.call_id = format!("UNRELATED-OVERFLOW-{index}");
            assert_eq!(
                client.buffer_pending_call_link_update(&oversized, &sender),
                PendingCallLinkBuffer::Saturated
            );
        }

        let first_request_id = first_request
            .as_node_ref()
            .attrs()
            .optional_string("id")
            .expect("initial request id")
            .into_owned();
        let refreshed_sent = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        crate::test_utils::answer_iq(
            &client,
            &first_request_id,
            &NodeBuilder::new("ack")
                .attr("class", "call")
                .attr("type", "link_join")
                .attr("id", first_request_id.as_str())
                .children([NodeBuilder::new("waiting_room")
                    .attr("call-id", "RETRIED-CALL-ID")
                    .attr("call-creator", creator.clone())
                    .attr("link-token", "RETRIED-CALL-LINK")
                    .attr("media", "audio")
                    .attr("enabled", "1")
                    .attr("is_admin", "0")
                    .attr("transaction-id", "1")
                    .build()])
                .build(),
        )
        .await;
        let refreshed_request = refreshed_sent.await.expect("refreshed link_join request");
        assert_eq!(
            refreshed_request
                .as_node_ref()
                .children()
                .expect("refreshed request action")[0]
                .tag,
            "link_join"
        );
        let refreshed_request_id = refreshed_request
            .as_node_ref()
            .attrs()
            .optional_string("id")
            .expect("refreshed request id")
            .into_owned();
        crate::test_utils::answer_iq(
            &client,
            &refreshed_request_id,
            &NodeBuilder::new("ack")
                .attr("class", "call")
                .attr("type", "link_join")
                .attr("id", refreshed_request_id.as_str())
                .children([NodeBuilder::new("waiting_room")
                    .attr("call-id", "RETRIED-CALL-ID")
                    .attr("call-creator", creator)
                    .attr("link-token", "RETRIED-CALL-LINK")
                    .attr("media", "audio")
                    .attr("enabled", "1")
                    .attr("is_admin", "0")
                    .attr("transaction-id", "2")
                    .build()])
                .build(),
        )
        .await;
        let registration = join
            .await
            .expect("join task")
            .expect("an unrelated overflow must recover through the bound retry");
        assert_eq!(registration.join.call_id, "RETRIED-CALL-ID");
        assert!(
            client
                .call_registry()
                .is_current("RETRIED-CALL-ID", registration.generation)
        );
        client
            .call_registry()
            .remove_if_current("RETRIED-CALL-ID", registration.generation);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn call_link_ack_paths_bind_before_waking_the_waiter() {
        for owned_fast_path in [false, true] {
            let (client, _transport) = crate::test_utils::create_iq_test_client().await;
            let unrelated_creator = Jid::new("333333333333333", Server::Lid);
            let unrelated_sender = unrelated_creator.clone().with_device(1);
            let _pending = client.begin_call_link_join();
            assert_eq!(
                client.buffer_pending_call_link_terminate(
                    "UNRELATED-CALL-LINK",
                    &unrelated_creator,
                    &unrelated_sender,
                ),
                PendingCallLinkBuffer::Buffered
            );

            let request_id = if owned_fast_path {
                "OWNED-LINK-JOIN-ACK"
            } else {
                "SHARED-LINK-JOIN-ACK"
            };
            let (sender, receiver) = futures::channel::oneshot::channel();
            client
                .response_waiters_guard()
                .insert(request_id.to_string(), ResponseWaiter::Iq(sender));
            let ack = NodeBuilder::new("ack")
                .attr("id", request_id)
                .attr("class", "call")
                .attr("type", "link_join")
                .children([NodeBuilder::new("waiting_room")
                    .attr("call-id", "ACTUAL-CALL-LINK")
                    .build()])
                .build();
            let node = crate::test_utils::node_to_owned_ref(&ack);
            let handled = if owned_fast_path {
                let node = Arc::try_unwrap(node)
                    .unwrap_or_else(|_| panic!("the test owns the ACK allocation"));
                client.handle_ack_response_owned(node)
            } else {
                client.handle_ack_response_arc(&node)
            };
            assert!(handled, "the ACK must resolve its registered waiter");
            assert_eq!(
                pending_link_updates(&client).await.entries,
                0,
                "the ACK call id must be bound before the waiter can observe the response"
            );
            assert_eq!(
                client.buffer_pending_call_link_terminate(
                    "LATE-UNRELATED-CALL",
                    &unrelated_creator,
                    &unrelated_sender,
                ),
                PendingCallLinkBuffer::NotPending,
                "later unrelated controls cannot consume the bound join's budget"
            );
            let response = receiver.await.expect("the ACK waiter should be woken");
            assert!(
                response
                    .get()
                    .get_attr("id")
                    .is_some_and(|value| value.as_str() == request_id)
            );
        }
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn call_link_registration_replays_staged_transitions_in_order() {
        let (client, _transport) = crate::test_utils::create_iq_test_client().await;
        let creator = Jid::new("333333333333333", Server::Lid);
        let call_id = "ORDERED-CALL-LINK";
        let local_device = Jid::new("111111111111111", Server::Lid).with_device(1);
        let mut participant = GroupCallParticipant::new(
            local_device.to_non_ad(),
            vec![GroupCallDevice::new(local_device.clone())],
        );
        participant.state = Some("connected".to_string());
        let relay = GroupCallRelay::builder()
            .transaction_id(8)
            .self_pid(1)
            .uuid("TEST-RELAY".to_string())
            .participant_uuid("TEST-PARTICIPANT".to_string())
            .attribute_padding(false)
            .warp_mi_tag_len(4)
            .key(vec![7; 32])
            .tokens(vec![vec![9; 16]])
            .endpoints(vec![
                GroupCallRelayEndpoint::builder()
                    .relay_id(1)
                    .token_id(0)
                    .auth_token_id(0)
                    .relay_name("test-relay".to_string())
                    .is_fna(false)
                    .ipv4("203.0.113.7".to_string())
                    .port(3478)
                    .build(),
            ])
            .build();
        let first = GroupCallUpdate::builder()
            .call_id(call_id.to_string())
            .call_creator(creator.clone())
            .transaction_id(8)
            .media("audio".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(true)
            .participants(vec![participant.clone()])
            .relay(relay)
            .build();
        let second = GroupCallUpdate::builder()
            .call_id(call_id.to_string())
            .call_creator(creator.clone())
            .transaction_id(9)
            .media("audio".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(false)
            .participants(vec![participant])
            .build();
        let initial_room = WaitingRoom::builder()
            .call_id(call_id.to_string())
            .call_creator(creator.clone())
            .link_token("TEST-CALL-LINK".to_string())
            .media(CallLinkMedia::Audio)
            .enabled(true)
            .is_admin(false)
            .transaction_id(1)
            .users(Vec::new())
            .build();
        let newer_room = WaitingRoom::builder()
            .call_id(call_id.to_string())
            .call_creator(creator.clone())
            .link_token("TEST-CALL-LINK".to_string())
            .media(CallLinkMedia::Audio)
            .enabled(true)
            .is_admin(true)
            .transaction_id(2)
            .users(Vec::new())
            .build();

        let pending = client.begin_call_link_join();
        let sender = creator.clone().with_device(1);
        assert_eq!(
            client.buffer_pending_call_link_update(&first, &sender),
            PendingCallLinkBuffer::Buffered
        );
        assert_eq!(
            client.buffer_pending_call_link_waiting_room(&newer_room, &sender),
            PendingCallLinkBuffer::Buffered
        );
        assert_eq!(
            client.buffer_pending_call_link_update(&second, &sender),
            PendingCallLinkBuffer::Buffered
        );
        assert_eq!(pending_link_updates(&client).await.entries, 3);

        let mut session =
            CallSession::new_outgoing(call_id, Jid::new(call_id, Server::Call), creator);
        let _ = session.transition_to(CallPhase::Calling);
        let _ = session.transition_to(CallPhase::WaitingRoom);
        let registration_lane = client.lock_answer_transition(call_id).await;
        let register_client = client.clone();
        let registration = tokio::spawn(async move {
            register_client
                .register_call_link_session(
                    session,
                    Some(initial_room),
                    CallLinkMedia::Audio,
                    "TEST-CALL-LINK",
                )
                .await
        });
        tokio::task::yield_now().await;
        assert_eq!(
            client.call_registry().generation_of(call_id),
            None,
            "call-link insertion must share the call-id registration lane"
        );
        drop(registration_lane);
        let generation = registration
            .await
            .expect("registration task")
            .expect("valid staged transitions");
        assert!(client.call_registry().set_group_invite_self_device(
            call_id,
            generation,
            GroupCallDevice::new(local_device).with_capability(1, [1]),
        ));
        let state = client
            .call_registry()
            .group_state_if_current(call_id, generation)
            .expect("registered group state");
        let snapshot = state.snapshot().expect("latest admission roster");
        assert_eq!(snapshot.transaction_id, 9);
        assert!(snapshot.relay.is_some(), "roster-only update retains relay");
        assert!(
            snapshot.rekey_requested,
            "the earlier unfulfilled rekey obligation survives the roster-only update"
        );
        assert!(
            state
                .waiting_room()
                .is_some_and(|room| room.transaction_id == Some(2) && room.is_admin)
        );
        assert_eq!(
            client.call_registry().phase_if_current(call_id, generation),
            Some(CallPhase::Connecting)
        );
        assert_eq!(pending_link_updates(&client).await.entries, 0);

        drop(pending);
        client
            .call_registry()
            .remove_if_current(call_id, generation);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn call_link_rekey_targets_the_latest_post_registration_roster() {
        let (client, _transport) = crate::test_utils::create_iq_test_client().await;
        let own_lid = Jid::new("111111111111111", Server::Lid).with_device(1);
        client
            .persistence_manager()
            .process_command(crate::store::commands::DeviceCommand::SetLid(Some(
                own_lid.clone(),
            )))
            .await;
        let creator = Jid::new("333333333333333", Server::Lid);
        let call_id = "POST-REGISTRATION-REKEY";
        let mut participant =
            GroupCallParticipant::new(own_lid.to_non_ad(), vec![GroupCallDevice::new(own_lid)]);
        participant.state = Some("connected".to_string());
        let initial = GroupCallUpdate::builder()
            .call_id(call_id.to_string())
            .call_creator(creator.clone())
            .transaction_id(1)
            .media("audio".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(true)
            .participants(vec![participant.clone()])
            .build();
        let mut session =
            CallSession::new_outgoing(call_id, Jid::new(call_id, Server::Call), creator.clone());
        session.group = Some(initial.clone());
        let _ = session.transition_to(CallPhase::Calling);
        let _ = session.transition_to(CallPhase::Connecting);
        let generation = client
            .register_call_link_session(session, None, CallLinkMedia::Audio, "TEST-CALL-LINK")
            .await
            .expect("registered admitted call link");

        let mut current = initial.clone();
        current.transaction_id = 2;
        current.rekey_requested = false;
        assert_eq!(
            client
                .call_registry()
                .apply_group_update_if_current(current, generation),
            wacore::voip::GroupStateApply::Applied
        );
        let mut join = wacore::types::group_call::CallLinkJoin::builder()
            .token("TEST-CALL-LINK".to_string())
            .media(CallLinkMedia::Audio)
            .call_id(call_id.to_string())
            .call_creator(creator)
            .waiting_room_enabled(false)
            .in_waiting_room(false)
            .is_admin(false)
            .group(initial)
            .build();

        assert!(
            !client
                .voip()
                .synchronize_call_link_admission(&mut join, generation, true)
                .await
                .expect("latest admission state")
        );
        assert_eq!(
            join.group.as_ref().map(|update| update.transaction_id),
            Some(2)
        );
        assert_eq!(
            client
                .call_registry()
                .pending_group_epoch_transaction_if_current(call_id, generation),
            Some(2),
            "the ACK rekey obligation must publish against the latest serialized roster"
        );
        client
            .call_registry()
            .remove_if_current(call_id, generation);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn invalid_admitted_call_link_snapshot_is_not_registered() {
        let (client, _transport) = crate::test_utils::create_iq_test_client().await;
        let creator = Jid::new("333333333333333", Server::Lid);
        let call_id = "INVALID-CALL-LINK";
        let mut invalid = GroupCallUpdate::builder()
            .call_id(call_id.to_string())
            .call_creator(creator.clone())
            .transaction_id(1)
            .media("audio".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(false)
            .participants(Vec::new())
            .build();
        invalid.connected_limit = 0;
        let mut session =
            CallSession::new_outgoing(call_id, Jid::new(call_id, Server::Call), creator);
        session.group = Some(invalid);

        assert_eq!(
            client
                .register_call_link_session(session, None, CallLinkMedia::Audio, "TEST-CALL-LINK",)
                .await,
            Err(wacore::voip::GroupStateApply::InvalidSnapshot)
        );
        assert_eq!(client.call_registry().generation_of(call_id), None);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn buffered_call_link_admission_cannot_cross_generations() {
        let (client, _transport) = crate::test_utils::create_iq_test_client().await;
        let creator = Jid::new("333333333333333", Server::Lid);
        let call_id = "BUFFERED-ADMISSION-GENERATION";
        let mut participant = GroupCallParticipant::new(
            creator.clone(),
            vec![GroupCallDevice::new(creator.clone().with_device(1))],
        );
        participant.state = Some("connected".to_string());
        let update = GroupCallUpdate::builder()
            .call_id(call_id.to_string())
            .call_creator(creator.clone())
            .transaction_id(8)
            .media("audio".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(false)
            .participants(vec![participant])
            .build();

        let registry = client.call_registry();
        let stale = registry.insert(CallSession::new_outgoing(
            call_id,
            Jid::new(call_id, Server::Call),
            creator.clone(),
        ));
        let replacement = registry.insert(CallSession::new_outgoing(
            call_id,
            Jid::new(call_id, Server::Call),
            creator,
        ));
        assert_ne!(stale, replacement);
        assert_eq!(
            client.apply_pending_call_link_update(update, stale),
            wacore::voip::GroupStateApply::UnknownCall
        );
        assert!(
            registry
                .group_state_if_current(call_id, replacement)
                .is_none(),
            "a buffered snapshot from the joining generation must not mutate its replacement"
        );
        registry.remove_if_current(call_id, replacement);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn call_link_waiting_room_cannot_cross_generations() {
        let (client, _transport) = crate::test_utils::create_iq_test_client().await;
        let creator = Jid::new("333333333333333", Server::Lid);
        let call_id = "WAITING-ROOM-GENERATION";
        let registry = client.call_registry();
        let stale = registry.insert(CallSession::new_outgoing(
            call_id,
            Jid::new(call_id, Server::Call),
            creator.clone(),
        ));
        let replacement = registry.insert(CallSession::new_outgoing(
            call_id,
            Jid::new(call_id, Server::Call),
            creator.clone(),
        ));
        let room = WaitingRoom::builder()
            .call_id(call_id.to_string())
            .call_creator(creator)
            .link_token("TEST-CALL-LINK".to_string())
            .media(CallLinkMedia::Audio)
            .enabled(true)
            .is_admin(true)
            .transaction_id(1)
            .users(Vec::new())
            .build();

        assert_eq!(
            registry.apply_waiting_room_if_current(room, stale),
            wacore::voip::GroupStateApply::UnknownCall
        );
        assert!(
            registry
                .group_state_if_current(call_id, replacement)
                .and_then(|state| state.waiting_room().cloned())
                .is_none(),
            "a stale join cannot grant waiting-room admin state to its replacement"
        );
        registry.remove_if_current(call_id, replacement);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn early_group_invite_accept_preserves_media_attachment_generation() {
        let (client, _transport) = crate::test_utils::create_iq_test_client().await;
        let creator = call_creator();
        let call_id = "ATTACHABLE-GROUP-INVITE";
        let update = GroupCallUpdate::builder()
            .call_id(call_id.to_string())
            .call_creator(creator.clone())
            .transaction_id(1)
            .media("audio".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(false)
            .participants(Vec::new())
            .build();
        let mut incoming = IncomingCall::new_for_test(
            creator.clone(),
            "ATTACHABLE-GROUP-INVITE-STANZA".to_string(),
            wacore::time::from_secs(1_766_847_151_i64).expect("valid ts"),
            CallAction::Offer {
                call_id: call_id.to_string(),
                call_creator: creator.clone(),
                caller_pn: None,
                caller_country_code: None,
                device_class: None,
                joinable: true,
                is_video: false,
                audio: Vec::new(),
                group_jid: None,
            },
        );
        incoming.group = Some(Box::new(update.clone()));
        let mut ringing = CallSession::new_incoming(call_id, creator.clone(), creator.clone());
        ringing.group = Some(update);
        let generation = client
            .call_registry()
            .insert_ringing_group_if_inactive(ringing)
            .expect("valid group snapshot")
            .expect("ringing invitation");
        incoming.set_ringing_generation(generation);

        client
            .voip()
            .accept_group_invite(&incoming)
            .await
            .expect("early group invitation accept");

        assert_eq!(
            client
                .call_registry()
                .ringing_group_generation(call_id, &creator),
            Some(generation),
            "the media accept builder must still be able to claim the exact ringing generation"
        );
        assert_eq!(
            client.call_registry().phase_if_current(call_id, generation),
            Some(CallPhase::Ringing)
        );
        assert!(client.call_registry().take_ringing(call_id));
        client
            .call_registry()
            .remove_if_current(call_id, generation);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn group_invite_preaccept_and_accept_are_bound_to_the_retained_offer_generation() {
        let client = crate::test_utils::create_test_client().await;
        let retained_creator = call_creator();
        let replacement_creator = retained_creator.clone();
        let call_id = "REPLACED-GROUP-INVITE";
        let group_update = |creator: &Jid| {
            GroupCallUpdate::builder()
                .call_id(call_id.to_string())
                .call_creator(creator.clone())
                .transaction_id(1)
                .media("audio".to_string())
                .connected_limit(32)
                .joinable(true)
                .av_upgradable(true)
                .rekey_requested(false)
                .participants(Vec::new())
                .build()
        };
        let retained_update = group_update(&retained_creator);
        let mut incoming = IncomingCall::new_for_test(
            retained_creator.clone(),
            "RETAINED-GROUP-INVITE".to_string(),
            wacore::time::from_secs(1_766_847_151_i64).expect("valid ts"),
            CallAction::Offer {
                call_id: call_id.to_string(),
                call_creator: retained_creator.clone(),
                caller_pn: None,
                caller_country_code: None,
                device_class: None,
                joinable: true,
                is_video: false,
                audio: Vec::new(),
                group_jid: None,
            },
        );
        incoming.group = Some(Box::new(retained_update.clone()));
        let mut retained =
            CallSession::new_incoming(call_id, retained_creator.clone(), retained_creator);
        retained.group = Some(retained_update);
        let stale = client.call_registry().insert_ringing_group(retained);
        incoming.set_ringing_generation(stale);

        let replacement_update = group_update(&replacement_creator);
        let mut replacement =
            CallSession::new_incoming(call_id, replacement_creator.clone(), replacement_creator);
        replacement.group = Some(replacement_update);
        let current = client.call_registry().insert_ringing_group(replacement);

        assert!(matches!(
            client.voip().preaccept_group_invite(&incoming).await,
            Err(CallError::CallEndedDuringSetup)
        ));
        assert!(matches!(
            client.voip().accept_group_invite(&incoming).await,
            Err(CallError::CallEndedDuringSetup)
        ));
        assert_eq!(client.call_registry().generation_of(call_id), Some(current));
        assert_eq!(
            client.call_registry().phase_if_current(call_id, current),
            Some(CallPhase::Ringing)
        );
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn group_invite_accept_does_not_consume_a_replacement_generation() {
        struct GatedTransport {
            started: async_channel::Sender<()>,
            release: async_channel::Receiver<()>,
        }

        #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
        #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
        impl crate::transport::Transport for GatedTransport {
            async fn send(&self, _data: Bytes) -> Result<(), anyhow::Error> {
                let _ = self.started.try_send(());
                self.release.recv().await?;
                Ok(())
            }

            async fn disconnect(&self) {}
        }

        let client = crate::test_utils::create_test_client().await;
        let creator = call_creator();
        let call_id = "ACTIVE-GROUP-INVITE";
        let update = GroupCallUpdate::builder()
            .call_id(call_id.to_string())
            .call_creator(creator.clone())
            .transaction_id(1)
            .media("audio".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(false)
            .participants(Vec::new())
            .build();
        let mut incoming = IncomingCall::new_for_test(
            creator.clone(),
            "GROUP-INVITE-STANZA".to_string(),
            wacore::time::from_secs(1_766_847_151_i64).expect("valid ts"),
            CallAction::Offer {
                call_id: call_id.to_string(),
                call_creator: creator.clone(),
                caller_pn: None,
                caller_country_code: None,
                device_class: None,
                joinable: true,
                is_video: false,
                audio: Vec::new(),
                group_jid: None,
            },
        );
        incoming.group = Some(Box::new(update.clone()));
        let mut ringing = CallSession::new_incoming(call_id, creator.clone(), creator.clone());
        ringing.group = Some(update.clone());
        let stale = client
            .call_registry()
            .insert_ringing_group_if_inactive(ringing)
            .expect("valid group snapshot")
            .expect("ringing invitation");
        incoming.set_ringing_generation(stale);

        let (started_tx, started_rx) = async_channel::bounded(1);
        let (release_tx, release_rx) = async_channel::bounded(1);
        let noise_socket = crate::socket::NoiseSocket::new(
            Arc::new(crate::runtime_impl::TokioRuntime),
            Arc::new(GatedTransport {
                started: started_tx,
                release: release_rx,
            }),
            NoiseCipher::new(&[0u8; 32]).expect("valid key"),
            NoiseCipher::new(&[0u8; 32]).expect("valid key"),
        );
        *client.noise_socket.lock().unwrap() = Some(Arc::new(noise_socket));

        let accept = tokio::spawn({
            let client = client.clone();
            let incoming = incoming.clone();
            async move { client.voip().accept_group_invite(&incoming).await }
        });
        started_rx.recv().await.expect("accept send entered");
        let mut replacement = CallSession::new_incoming(call_id, creator.clone(), creator);
        replacement.group = Some(update);
        let current = client.call_registry().insert_ringing_group(replacement);
        assert_ne!(current, stale);
        release_tx.send(()).await.expect("release accept send");

        assert!(matches!(
            accept.await.expect("accept task"),
            Err(CallError::CallEndedDuringSetup)
        ));
        assert_eq!(client.call_registry().generation_of(call_id), Some(current));
        assert_eq!(
            client.call_registry().phase_if_current(call_id, current),
            Some(CallPhase::Ringing)
        );
        assert!(
            client.call_registry().take_ringing(call_id),
            "the stale accept must leave the replacement ringing"
        );
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn cancelling_call_link_request_removes_response_waiter() {
        let (client, _transport) = crate::test_utils::create_iq_test_client().await;
        let sent = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        let request_client = client.clone();
        let request = tokio::spawn(async move {
            request_client
                .voip()
                .create_call_link(CallLinkMedia::Audio)
                .await
        });
        let node = sent.await.expect("link_create request");
        let request_id = node
            .as_node_ref()
            .attrs()
            .optional_string("id")
            .expect("request id")
            .into_owned();
        assert!(
            client.response_waiters_guard().contains_key(&request_id),
            "the request must register its ACK waiter before sending"
        );

        request.abort();
        assert!(
            request
                .await
                .expect_err("request should be cancelled")
                .is_cancelled()
        );
        tokio::task::yield_now().await;
        assert!(
            !client.response_waiters_guard().contains_key(&request_id),
            "cancelling a call-service request must not leak its waiter"
        );
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn cancelling_registered_call_link_join_removes_its_generation() {
        use wacore::handshake::NoiseCipher;

        struct BlockingTransport {
            started: async_channel::Sender<()>,
        }

        #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
        #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
        impl crate::transport::Transport for BlockingTransport {
            async fn send(&self, _data: Bytes) -> Result<(), anyhow::Error> {
                let _ = self.started.try_send(());
                futures::future::pending().await
            }

            async fn disconnect(&self) {}
        }

        let (client, _transport) = crate::test_utils::create_iq_test_client().await;
        client
            .persistence_manager()
            .process_command(crate::store::commands::DeviceCommand::SetLid(Some(
                Jid::new("111111111111111", Server::Lid).with_device(1),
            )))
            .await;
        let creator = Jid::new("333333333333333", Server::Lid);
        let join_sent = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        let join_client = client.clone();
        let join = tokio::spawn(async move {
            join_client
                .voip()
                .join_call_link_with_audio(
                    "CANCELLED-CALL-LINK",
                    CallLinkMedia::Audio,
                    AudioFormat::OPUS_16KHZ_60MS,
                )
                .await
        });
        let request = join_sent.await.expect("link_join request");
        let request_id = request
            .as_node_ref()
            .attrs()
            .optional_string("id")
            .expect("request id")
            .into_owned();
        let (started_tx, started_rx) = async_channel::bounded(1);
        let blocking_socket = crate::socket::NoiseSocket::new(
            Arc::new(crate::runtime_impl::TokioRuntime),
            Arc::new(BlockingTransport {
                started: started_tx,
            }),
            NoiseCipher::new(&[0u8; 32]).expect("valid key"),
            NoiseCipher::new(&[0u8; 32]).expect("valid key"),
        );
        *client.noise_socket.lock().unwrap() = Some(Arc::new(blocking_socket));
        crate::test_utils::answer_iq(
            &client,
            &request_id,
            &NodeBuilder::new("ack")
                .attr("class", "call")
                .attr("type", "link_join")
                .attr("id", request_id.as_str())
                .children([NodeBuilder::new("waiting_room")
                    .attr("call-id", "CANCELLED-CALL-ID")
                    .attr("call-creator", creator)
                    .attr("link-token", "CANCELLED-CALL-LINK")
                    .attr("media", "audio")
                    .attr("enabled", "1")
                    .attr("is_admin", "0")
                    .attr("transaction-id", "1")
                    .build()])
                .build(),
        )
        .await;
        started_rx.recv().await.expect("heartbeat send must start");
        assert!(
            client
                .call_registry()
                .generation_of("CANCELLED-CALL-ID")
                .is_some(),
            "the join must register before its heartbeat completes"
        );

        join.abort();
        assert!(
            join.await
                .expect_err("join should be cancelled")
                .is_cancelled()
        );
        tokio::task::yield_now().await;
        assert_eq!(
            client.call_registry().generation_of("CANCELLED-CALL-ID"),
            None,
            "cancelling after registration must reap only that generation"
        );
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn cancelling_an_admitted_call_link_registration_sends_terminate() {
        let (client, sends) = make_client_with_count().await;
        let call_id = "CANCELLED-ADMITTED-CALL";
        let creator = Jid::new("333333333333333", Server::Lid);
        let mut session =
            CallSession::new_outgoing(call_id, Jid::new(call_id, Server::Call), creator.clone());
        let _ = session.transition_to(CallPhase::Calling);
        let _ = session.transition_to(CallPhase::Connecting);
        let registry = client.call_registry();
        let generation = registry.insert(session);
        let registration = super::CallLinkRegistrationGuard::new(
            &client,
            registry.clone(),
            call_id,
            creator,
            generation,
        );

        drop(registration);

        tokio::time::timeout(Duration::from_secs(2), async {
            while sends.load(Ordering::SeqCst) == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("admitted cancellation must send a call-scoped terminate");
        assert_eq!(sends.load(Ordering::SeqCst), 1);
        assert_eq!(registry.generation_of(call_id), None);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn call_link_join_reports_admission_committed_during_heartbeat() {
        use wacore::handshake::NoiseCipher;

        struct GatedTransport {
            started: async_channel::Sender<()>,
            release: async_channel::Receiver<()>,
        }

        #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
        #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
        impl crate::transport::Transport for GatedTransport {
            async fn send(&self, _data: Bytes) -> Result<(), anyhow::Error> {
                self.started
                    .send(())
                    .await
                    .map_err(|_| anyhow::anyhow!("heartbeat observer closed"))?;
                self.release
                    .recv()
                    .await
                    .map_err(|_| anyhow::anyhow!("heartbeat gate closed"))?;
                Ok(())
            }

            async fn disconnect(&self) {}
        }

        let (client, _transport) = crate::test_utils::create_iq_test_client().await;
        let own_lid = Jid::new("111111111111111", Server::Lid).with_device(1);
        client
            .persistence_manager()
            .process_command(crate::store::commands::DeviceCommand::SetLid(Some(
                own_lid.clone(),
            )))
            .await;
        let creator = Jid::new("333333333333333", Server::Lid);
        let sent = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        let join_client = client.clone();
        let join = tokio::spawn(async move {
            join_client
                .voip()
                .join_call_link_registration_with_audio(
                    "ADMISSION-RACE-LINK",
                    CallLinkMedia::Audio,
                    AudioFormat::OPUS_16KHZ_60MS,
                )
                .await
        });
        let request = sent.await.expect("link_join request");
        let request_id = request
            .as_node_ref()
            .attrs()
            .optional_string("id")
            .expect("request id")
            .into_owned();
        let (started_tx, started_rx) = async_channel::bounded(1);
        let (release_tx, release_rx) = async_channel::bounded(1);
        let gated_socket = crate::socket::NoiseSocket::new(
            Arc::new(crate::runtime_impl::TokioRuntime),
            Arc::new(GatedTransport {
                started: started_tx,
                release: release_rx,
            }),
            NoiseCipher::new(&[0u8; 32]).expect("valid key"),
            NoiseCipher::new(&[0u8; 32]).expect("valid key"),
        );
        *client.noise_socket.lock().unwrap() = Some(Arc::new(gated_socket));
        crate::test_utils::answer_iq(
            &client,
            &request_id,
            &NodeBuilder::new("ack")
                .attr("class", "call")
                .attr("type", "link_join")
                .attr("id", request_id.as_str())
                .children([NodeBuilder::new("waiting_room")
                    .attr("call-id", "ADMISSION-RACE-CALL")
                    .attr("call-creator", creator.clone())
                    .attr("link-token", "ADMISSION-RACE-LINK")
                    .attr("media", "audio")
                    .attr("enabled", "1")
                    .attr("is_admin", "0")
                    .attr("transaction-id", "1")
                    .build()])
                .build(),
        )
        .await;
        started_rx.recv().await.expect("heartbeat send started");

        let registry = client.call_registry();
        let generation = registry
            .generation_of("ADMISSION-RACE-CALL")
            .expect("registered waiting-room generation");
        let mut participant =
            GroupCallParticipant::new(own_lid.to_non_ad(), vec![GroupCallDevice::new(own_lid)]);
        participant.state = Some("connected".to_string());
        let admitted = GroupCallUpdate::builder()
            .call_id("ADMISSION-RACE-CALL".to_string())
            .call_creator(creator)
            .transaction_id(2)
            .media("audio".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(false)
            .participants(vec![participant])
            .build();
        let transition_lock = registry
            .group_transition_lock("ADMISSION-RACE-CALL", generation)
            .expect("group transition lane");
        let transition_guard = transition_lock.lock().await;
        assert_eq!(
            registry.apply_group_update_if_current(admitted, generation),
            wacore::voip::GroupStateApply::Applied
        );
        assert_eq!(
            registry.phase_if_current("ADMISSION-RACE-CALL", generation),
            Some(CallPhase::Connecting)
        );
        drop(transition_guard);
        release_tx.send(()).await.expect("release heartbeat send");

        let registration = join.await.expect("join task").expect("join response");
        assert_eq!(registration.generation, generation);
        assert!(!registration.join.in_waiting_room);
        assert_eq!(
            registration
                .join
                .group
                .as_ref()
                .map(|update| update.transaction_id),
            Some(2),
            "the public result must report admission committed during the heartbeat"
        );
        registry.remove_if_current("ADMISSION-RACE-CALL", generation);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn immediately_admitted_call_link_preserves_token_and_origin_generation() {
        let (client, _transport) = crate::test_utils::create_iq_test_client().await;
        client
            .persistence_manager()
            .process_command(crate::store::commands::DeviceCommand::SetLid(Some(
                Jid::new("111111111111111", Server::Lid).with_device(1),
            )))
            .await;
        let creator = Jid::new("333333333333333", Server::Lid);
        let sent = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        let join_client = client.clone();
        let join = tokio::spawn(async move {
            join_client
                .voip()
                .join_call_link_registration_with_audio(
                    "REQUESTED-CALL-LINK",
                    CallLinkMedia::Video,
                    AudioFormat::OPUS_16KHZ_60MS,
                )
                .await
        });
        let request = sent.await.expect("link_join request");
        let request_id = request
            .as_node_ref()
            .attrs()
            .optional_string("id")
            .expect("request id")
            .into_owned();
        crate::test_utils::answer_iq(
            &client,
            &request_id,
            &NodeBuilder::new("ack")
                .attr("class", "call")
                .attr("type", "link_join")
                .attr("id", request_id.as_str())
                .children([
                    NodeBuilder::new("waiting_room")
                        .attr("call-id", "ADMITTED-CALL-ID")
                        .attr("call-creator", creator.clone())
                        .attr("link-token", "REQUESTED-CALL-LINK")
                        .attr("media", "video")
                        .attr("enabled", "1")
                        .attr("is_admin", "1")
                        .attr("transaction-id", "1")
                        .build(),
                    NodeBuilder::new("group_info")
                        .attr("call-id", "ADMITTED-CALL-ID")
                        .attr("call-creator", creator)
                        .attr("transaction-id", "1")
                        .attr("connected-limit", "32")
                        .attr("media", "video")
                        .build(),
                ])
                .build(),
        )
        .await;

        let admitted_registration = join.await.expect("join task").expect("join response");
        let admitted = admitted_registration.join;
        assert_eq!(admitted.token, "REQUESTED-CALL-LINK");
        assert!(!admitted.in_waiting_room);
        let generation = client
            .call_registry()
            .generation_of("ADMITTED-CALL-ID")
            .expect("registered admitted call");
        assert_eq!(
            admitted_registration.generation, generation,
            "the join result must retain the generation it created"
        );
        assert!(
            client
                .call_registry()
                .group_state("ADMITTED-CALL-ID")
                .and_then(|state| state.waiting_room().cloned())
                .is_some_and(|room| room.is_admin && room.enabled),
            "admitted joins must retain waiting-room admin state from the ACK"
        );
        let replacement = client.call_registry().insert(CallSession::new_outgoing(
            "ADMITTED-CALL-ID",
            Jid::new("ADMITTED-CALL-ID", Server::Call),
            Jid::new("333333333333333", Server::Lid),
        ));
        assert_ne!(replacement, admitted_registration.generation);
        assert!(
            client
                .call_registry()
                .snapshot_if_current("ADMITTED-CALL-ID", admitted_registration.generation)
                .is_none(),
            "a stale starter must not attach through a replacement generation"
        );
        client
            .call_registry()
            .remove_if_current("ADMITTED-CALL-ID", replacement);
    }
}

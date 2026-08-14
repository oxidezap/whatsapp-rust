//! Per-call registry: tracks active [`CallSession`]s and their media-task abort handles so a
//! connection teardown can stop every in-flight call. [`CallRegistry::abort_all`] is the teardown
//! primitive, but it is NOT yet wired into the client's connection cleanup; the integrator owns a
//! `CallRegistry` and must call `abort_all` from their own disconnect/reconnect path.
//!
//! The abort handle is [`crate::runtime::AbortHandle`] (runtime-agnostic), so the same registry
//! drives the Tokio driver task, a wasm `spawn_local` task, or any other runtime without coupling
//! the portable core to a specific executor.

use std::collections::{HashMap, HashSet, VecDeque};
use std::mem::{size_of, size_of_val};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_lock::Mutex as AsyncMutex;
use portable_atomic::{AtomicBool, AtomicU64, AtomicUsize};

use crate::runtime::AbortHandle;
use crate::types::call::{CallAction, IncomingCall, VideoState};
use crate::types::group_call::{
    GroupCallDevice, GroupCallParticipant, GroupCallUpdate, ScreenShare, WaitingRoom,
};
use crate::voip::driver::{GroupControl, GroupRawEpoch, VideoControl, VideoControlSender};
use crate::voip::engine::CallEvent;
use crate::voip::group::{GroupCallState, GroupStateApply};
use crate::voip::group_media::group_device_is_local;
use crate::voip::session::{CallPhase, CallSession};
use wacore_binary::Jid;

const MAX_PENDING_INITIAL_GROUP_CONTROLS: usize = 64;
/// Bootstrap controls only bridge the short offer-registration race. One MiB leaves ample room for
/// a legitimate full roster plus rekey while bounding unauthenticated call-id retention.
const MAX_PENDING_INITIAL_GROUP_CONTROL_BYTES: usize = 1024 * 1024;
/// Match the call-service offer ACK horizon so abandoned prospective identities cannot reserve the
/// global buffer indefinitely, while controls from an in-flight offer still survive reordering.
const PENDING_INITIAL_GROUP_CONTROL_TTL: Duration = Duration::from_secs(10);
const MAX_CALL_EVENT_QUEUE_BYTES: usize = 1024 * 1024;
const MAX_GROUP_CONTROL_QUEUE_BYTES: usize = 1024 * 1024;
const DEFAULT_CALL_EVENT_QUEUE_CAPACITY: usize = 64;
const MAX_RINGING_GROUP_CALLS: usize = 64;
const MAX_RINGING_GROUP_CALL_BYTES: usize = 1024 * 1024;

/// Identifies one peer video-upgrade request within one call generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VideoUpgradeToken {
    generation: u64,
    epoch: u64,
}

impl VideoUpgradeToken {
    pub fn generation(self) -> u64 {
        self.generation
    }
}

/// Result of applying a committed peer video-state transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PeerVideoTransition {
    Ignored,
    UpgradeRequested(VideoUpgradeToken),
    Applied {
        enable_plane: bool,
        teardown_local: bool,
        answer_upgrade: bool,
    },
}

#[derive(Debug, Clone, Copy)]
struct VideoNegotiation {
    self_state: VideoState,
    peer_state: VideoState,
    peer_request_epoch: u64,
    pending_peer_request: Option<u64>,
    self_request_epoch: u64,
    pending_self_request: Option<u64>,
}

impl VideoNegotiation {
    fn new(is_video: bool) -> Self {
        let state = if is_video {
            VideoState::Enabled
        } else {
            VideoState::Disabled
        };
        Self {
            self_state: state,
            peer_state: state,
            peer_request_epoch: 0,
            pending_peer_request: None,
            self_request_epoch: 0,
            pending_self_request: None,
        }
    }

    fn is_video(self) -> bool {
        !self.self_state.is_inactive_for_call_mode() || !self.peer_state.is_inactive_for_call_mode()
    }

    fn next_peer_request(&mut self, generation: u64) -> VideoUpgradeToken {
        self.peer_request_epoch = self.peer_request_epoch.wrapping_add(1).max(1);
        self.pending_peer_request = Some(self.peer_request_epoch);
        VideoUpgradeToken {
            generation,
            epoch: self.peer_request_epoch,
        }
    }

    fn next_self_request(&mut self) -> u64 {
        self.self_request_epoch = self.self_request_epoch.wrapping_add(1).max(1);
        self.pending_self_request = Some(self.self_request_epoch);
        self.self_request_epoch
    }
}

/// Runs its closure when dropped. Stored on a [`CallEntry`] to wake the call's `wait_ended()` waiter
/// whenever the entry is removed (terminal stanza, disconnect, supersession) -- including in the
/// window after registration but before a media task exists to carry the notify on its own teardown.
/// Every entry-drop is a terminal event for that generation, and the wake (a sticky flag) is
/// idempotent with the media task's own drop-guard, so firing it on every removal is safe.
struct EndedNotify(Option<Box<dyn FnOnce() + Send>>);

impl Drop for EndedNotify {
    fn drop(&mut self) {
        if let Some(f) = self.0.take() {
            f();
        }
    }
}

#[derive(Clone)]
struct CallEventQueue {
    tx: async_channel::Sender<CallEvent>,
    max_payload_bytes: Arc<AtomicUsize>,
}

impl CallEventQueue {
    fn new(tx: async_channel::Sender<CallEvent>) -> Self {
        Self {
            tx,
            max_payload_bytes: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn force_send(&self, event: CallEvent) -> bool {
        let payload_bytes = event.heap_bytes();
        let queue_capacity = self
            .tx
            .capacity()
            .unwrap_or(DEFAULT_CALL_EVENT_QUEUE_CAPACITY)
            .max(1);
        let max_event_bytes = MAX_CALL_EVENT_QUEUE_BYTES / queue_capacity;
        if size_of::<CallEvent>().saturating_add(payload_bytes) > max_event_bytes {
            return false;
        }
        self.max_payload_bytes
            .fetch_max(payload_bytes, Ordering::Relaxed);
        self.tx.force_send(event).is_ok()
    }

    fn retained_bytes(&self) -> usize {
        self.tx.len().saturating_mul(
            size_of::<CallEvent>().saturating_add(self.max_payload_bytes.load(Ordering::Relaxed)),
        )
    }
}

#[derive(Clone)]
struct GroupControlQueue {
    tx: async_channel::Sender<GroupControl>,
    max_payload_bytes: Arc<AtomicUsize>,
}

impl GroupControlQueue {
    fn new(tx: async_channel::Sender<GroupControl>) -> Self {
        Self {
            tx,
            max_payload_bytes: Arc::new(AtomicUsize::new(0)),
        }
    }

    fn retained_bytes(&self) -> usize {
        self.tx.len().saturating_mul(
            size_of::<GroupControl>()
                .saturating_add(self.max_payload_bytes.load(Ordering::Relaxed)),
        )
    }

    fn accepts(&self, control: &GroupControl) -> bool {
        Self::accepts_with_capacity(control, self.tx.capacity().unwrap_or(1))
    }

    fn accepts_with_capacity(control: &GroupControl, queue_capacity: usize) -> bool {
        let queue_capacity = queue_capacity.max(1);
        let max_control_bytes = MAX_GROUP_CONTROL_QUEUE_BYTES / queue_capacity;
        size_of::<GroupControl>().saturating_add(control.heap_bytes()) <= max_control_bytes
    }

    fn try_send(&self, control: GroupControl) -> bool {
        self.try_send_recover(control).is_ok()
    }

    fn try_send_recover(&self, control: GroupControl) -> Result<(), GroupControl> {
        if !self.accepts(&control) {
            return Err(control);
        }
        let payload_bytes = control.heap_bytes();
        self.tx
            .try_send(control)
            .map_err(|error| error.into_inner())?;
        self.max_payload_bytes
            .fetch_max(payload_bytes, Ordering::Relaxed);
        Ok(())
    }
}

fn retained_epoch(control: GroupControl) -> Option<GroupRawEpoch> {
    match control {
        GroupControl::Transition { epoch, .. } | GroupControl::RawEpoch(epoch) => Some(epoch),
        GroupControl::Update(_) | GroupControl::Reaction(_) => None,
    }
}

struct CallEntry {
    session: CallSession,
    media_task: Option<AbortHandle>,
    /// Keeps a pending call-link admission alive. Cleared on admission and aborted with the entry.
    waiting_room_task: Option<AbortHandle>,
    /// Monotonic token distinguishing this registration from a later same-call-id replacement, so a
    /// finishing task only reaps its OWN entry (the ABA hazard).
    generation: u64,
    /// Caller-only, one-shot: delivers the answering device LID to the drive loop so it can rekey
    /// recv. Taken on first use (a duplicate `<accept>` finds `None`); dropped with the entry.
    rekey_tx: Option<async_channel::Sender<String>>,
    /// The call's consumer-facing event queue (same channel `CallHandle::events()` reads), so the
    /// SIGNALING handler can surface `<video state>` changes next to the engine's events.
    event_tx: Option<CallEventQueue>,
    /// Mid-call video-plane control into the drive loop (enable/disable/orientation).
    video_ctl_tx: Option<VideoControlSender>,
    /// Lossless group roster/key transitions into the media driver.
    group_ctl_tx: Option<GroupControlQueue>,
    /// WARP authentication-tag width baked into the attached media pipelines. A relay refresh
    /// cannot change this packet boundary without rebuilding every sender and receiver atomically.
    group_warp_mi_tag_len: Option<usize>,
    /// An epoch may arrive after call-scoped accept but before relay media attaches. Replacing or
    /// dropping this command erases its key bytes through [`GroupRawEpoch`]'s `Drop`.
    pending_group_epoch: Option<GroupRawEpoch>,
    /// Fully release the local video endpoints. Stored here so refusal and terminal paths share the
    /// same teardown without keeping codec resources alive through lingering handles.
    video_teardown: Option<Box<dyn Fn() + Send + Sync>>,
    /// Prevents concurrent video transitions from overtaking each other while an ack is in flight.
    event_publication_reserved: Arc<AtomicBool>,
    /// Mirrors WA's stream mutex for user actions, peer signaling, and timeout callbacks.
    video_transition_lock: Arc<AsyncMutex<()>>,
    /// Serializes participant controls and waiting-room state through their typed ACK.
    group_transition_lock: Arc<AsyncMutex<()>>,
    /// Wakes a pending call-link media builder on admission, replacement, or terminal teardown.
    group_update_event: Arc<event_listener::Event>,
    video: VideoNegotiation,
    /// The rotation this side announces, as `<video device_orientation>`, in
    /// `0..=3`.
    ///
    /// Ours, not the peer's: theirs arrives on their `<video>` and travels the
    /// other way, into `Engine::set_peer_video_orientation`, where it stamps the
    /// frames they send. The two are independent facts about two devices.
    ///
    /// Beside the negotiation rather than inside it, because a camera does not
    /// un-rotate when a negotiation restarts: six paths rebuild
    /// `VideoNegotiation` (a group downgrade, a re-offer, glare), and a rotation
    /// has to survive all of them or the next `<video>` announces a camera that
    /// is not the one pointing at the user.
    self_video_orientation: u8,
    /// Explicit group identity exists before the first authoritative roster arrives.
    is_group_call: bool,
    /// This generation originated from a reusable call-link join and may accept waiting-room state.
    is_call_link: bool,
    group: Option<GroupCallState>,
    /// Active 1:1 devices retained so an established call can become an ad-hoc group call.
    group_invite_self_device: Option<GroupCallDevice>,
    group_invite_peer_device: Option<GroupCallDevice>,
    /// Once an `<accept>` selects a device, delayed sibling preaccepts cannot replace it.
    group_invite_peer_selected: bool,
    /// Wakes this call's `wait_ended()` waiter on removal, even before a media task exists. Fires from
    /// `EndedNotify`'s Drop whenever the entry leaves the map.
    on_terminal: Option<EndedNotify>,
}

impl CallEntry {
    fn group_mut(&mut self) -> &mut GroupCallState {
        self.is_group_call = true;
        let call_id = self.session.call_id.clone();
        let call_creator = self.session.call_creator.clone();
        self.group
            .get_or_insert_with(|| GroupCallState::new(call_id, call_creator))
    }

    fn heap_bytes(&self) -> usize {
        use crate::stats::HeapSize;

        let queued_bytes = self
            .rekey_tx
            .as_ref()
            .map_or(0, |tx| tx.len().saturating_mul(size_of::<String>()))
            .saturating_add(
                self.event_tx
                    .as_ref()
                    .map_or(0, CallEventQueue::retained_bytes),
            )
            .saturating_add(
                self.video_ctl_tx
                    .as_ref()
                    .map_or(0, VideoControlSender::retained_bytes),
            )
            .saturating_add(
                self.group_ctl_tx
                    .as_ref()
                    .map_or(0, GroupControlQueue::retained_bytes),
            );
        self.session.heap_bytes()
            + self.group.as_ref().map_or(0, HeapSize::heap_bytes)
            + self
                .pending_group_epoch
                .as_ref()
                .map_or(0, GroupRawEpoch::heap_bytes)
            + self
                .group_invite_self_device
                .as_ref()
                .map_or(0, HeapSize::heap_bytes)
            + self
                .group_invite_peer_device
                .as_ref()
                .map_or(0, HeapSize::heap_bytes)
            + size_of::<AsyncMutex<()>>() * 2
            + size_of::<event_listener::Event>()
            + queued_bytes
    }

    fn retained_bytes(&self, call_id: &str) -> usize {
        use crate::stats::HeapSize;

        size_of::<String>()
            .saturating_add(call_id.heap_bytes())
            .saturating_add(size_of::<CallEntry>())
            .saturating_add(self.heap_bytes())
    }
}

/// Exclusive publication right for an actionable signaling event awaiting its typed ack.
pub struct CallEventPermit {
    tx: CallEventQueue,
    reserved: Arc<AtomicBool>,
    generation: u64,
}

impl CallEventPermit {
    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn send(&self, event: CallEvent) -> bool {
        // The latest committed state must remain observable even when its consumer is behind.
        self.tx.force_send(event)
    }
}

impl Drop for CallEventPermit {
    fn drop(&mut self) {
        self.reserved.store(false, Ordering::Release);
    }
}

impl Drop for CallEntry {
    fn drop(&mut self) {
        self.group_update_event.notify(usize::MAX);
        if let Some(teardown) = self.video_teardown.take() {
            teardown();
        }
    }
}

fn routed_call_sender(call: &IncomingCall) -> &Jid {
    call.participant.as_ref().unwrap_or(&call.from)
}

struct PendingInitialGroupControl {
    call: IncomingCall,
    expires_at: crate::time::Instant,
}

fn pending_initial_group_control_heap_bytes(call: &IncomingCall) -> usize {
    use crate::stats::HeapSize;

    let envelope = call.from.heap_bytes()
        + call.stanza_id.heap_bytes()
        + call.notify.as_ref().map_or(0, HeapSize::heap_bytes)
        + call.platform.as_ref().map_or(0, HeapSize::heap_bytes)
        + call.version.as_ref().map_or(0, HeapSize::heap_bytes)
        + call.participant.as_ref().map_or(0, HeapSize::heap_bytes)
        + call.recipient.as_ref().map_or(0, HeapSize::heap_bytes);
    envelope
        + match &call.action {
            CallAction::GroupUpdate { update } => {
                size_of::<GroupCallUpdate>() + update.heap_bytes()
            }
            CallAction::EncRekey { rekey } => {
                size_of_val(rekey.as_ref())
                    + rekey.call_id.heap_bytes()
                    + rekey.call_creator.heap_bytes()
                    + rekey.encryption_type.heap_bytes()
                    + rekey.ciphertext.heap_bytes()
            }
            _ => 0,
        }
}

fn pending_initial_group_control_retained_bytes(call: &IncomingCall) -> usize {
    size_of::<PendingInitialGroupControl>()
        .saturating_add(pending_initial_group_control_heap_bytes(call))
}

fn pending_initial_group_control_matches(
    call: &IncomingCall,
    call_id: &str,
    call_creator: &Jid,
) -> bool {
    call.action.call_id() == call_id
        && call.action.call_creator().to_non_ad() == call_creator.to_non_ad()
}

/// Thread-safe map of active calls keyed by call-id.
#[derive(Default)]
pub struct CallRegistry {
    inner: Mutex<HashMap<String, CallEntry>>,
    next_gen: AtomicU64,
    /// Creator-authenticated controls that overtook their initial group offer. A bounded value
    /// queue avoids retaining one task per fabricated call id while preserving the real offer race.
    pending_initial_group_controls: Mutex<VecDeque<PendingInitialGroupControl>>,
    /// Wakes controls that arrived just before the corresponding offer registered its generation.
    registration_event: Arc<event_listener::Event>,
    /// Incoming offers we've rung but not yet answered, keyed by call-id. Mirrors WA Web's
    /// `_ringingCalls`: it is the ONLY signal that distinguishes a genuine missed call (a `<terminate>`
    /// for an offer still ringing) from a `<terminate>` for an answered or outgoing call. Active-call
    /// absence cannot tell them apart, since an answered call leaves the map on teardown too.
    ringing: Mutex<HashSet<String>>,
}

impl CallRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// The active-call map, recovering the guard from a poisoned mutex instead of panicking.
    ///
    /// Poisoning would be permanent and process-wide: one panic under any of these guards would
    /// turn every later VoIP call into a panic, including [`abort_all`](Self::abort_all), the one
    /// operation a caller needs after a failure -- and it would not undo whatever the panic left
    /// behind. Recovery is sound because the damage an unwind can do is bounded to the one call it
    /// was already failing: the map has no cross-entry invariant, entries are only ever removed
    /// whole, and no individual field is ever left torn. A multi-field update can still stop
    /// partway -- `set_video_channels` writes three fields, and a superseded value panicking in
    /// `Drop` between them leaves that call's own update incomplete -- but every entry stays
    /// individually well-formed, and reachable, which poisoning would not have preserved.
    fn active_calls(&self) -> std::sync::MutexGuard<'_, HashMap<String, CallEntry>> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The ringing-offer set. Poison policy as in [`active_calls`](Self::active_calls).
    fn ringing_calls(&self) -> std::sync::MutexGuard<'_, HashSet<String>> {
        self.ringing
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// The pre-offer control buffer. Poison policy as in [`active_calls`](Self::active_calls).
    fn pending_controls(&self) -> std::sync::MutexGuard<'_, VecDeque<PendingInitialGroupControl>> {
        self.pending_initial_group_controls
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn ringing_group_update_fits(
        ringing: &HashSet<String>,
        map: &HashMap<String, CallEntry>,
        call_id: &str,
        entry: &CallEntry,
        preview: &GroupCallState,
        preview_session: &CallSession,
    ) -> bool {
        use crate::stats::HeapSize;

        if !ringing.contains(call_id)
            || !entry.is_group_call
            || entry.session.phase() != CallPhase::Ringing
        {
            return true;
        }
        let current_state_bytes = entry
            .group
            .as_ref()
            .map_or(0, HeapSize::heap_bytes)
            .saturating_add(entry.session.heap_bytes());
        let preview_state_bytes = preview
            .heap_bytes()
            .saturating_add(preview_session.heap_bytes());
        let prospective_entry_bytes = entry
            .retained_bytes(call_id)
            .saturating_sub(current_state_bytes)
            .saturating_add(preview_state_bytes);
        let ringing_group_bytes = map
            .iter()
            .filter(|(candidate_call_id, candidate)| {
                ringing.contains(*candidate_call_id)
                    && candidate.is_group_call
                    && candidate.session.phase() == CallPhase::Ringing
            })
            .fold(0usize, |bytes, (candidate_call_id, candidate)| {
                bytes.saturating_add(candidate.retained_bytes(candidate_call_id))
            });
        ringing_group_bytes
            .saturating_sub(entry.retained_bytes(call_id))
            .saturating_add(prospective_entry_bytes)
            <= MAX_RINGING_GROUP_CALL_BYTES
    }

    /// Snapshot active registry retention without cloning call state or exposing identifiers.
    pub fn memory_stats(&self) -> crate::stats::CollectionStats {
        use crate::stats::HeapSize;

        let (active_entries, active_bytes) = {
            let map = self.active_calls();
            let bytes = map
                .iter()
                .map(|(call_id, entry)| entry.retained_bytes(call_id))
                .sum::<usize>();
            (map.len(), bytes)
        };
        let (pending_entries, pending_bytes) = {
            let mut pending = self.pending_controls();
            let now = crate::time::Instant::now();
            pending.retain(|queued| queued.expires_at > now);
            let bytes = pending
                .iter()
                .map(|queued| pending_initial_group_control_retained_bytes(&queued.call))
                .sum::<usize>();
            (pending.len(), bytes)
        };
        let (ringing_entries, ringing_bytes) = {
            let ringing = self.ringing_calls();
            let bytes = ringing
                .iter()
                .map(|call_id| size_of::<String>() + call_id.heap_bytes())
                .sum::<usize>();
            (ringing.len(), bytes)
        };
        crate::stats::CollectionStats::new(
            active_entries
                .saturating_add(pending_entries)
                .saturating_add(ringing_entries)
                .try_into()
                .unwrap_or(u64::MAX),
            active_bytes
                .saturating_add(pending_bytes)
                .saturating_add(ringing_bytes)
                .try_into()
                .unwrap_or(u64::MAX),
        )
    }

    /// Retain an initial roster/key control until its matching group offer registers.
    ///
    /// The pending lock is acquired before checking the active map. The offer path inserts first
    /// and drains second, so either ordering leaves the control with exactly one consumer.
    pub fn buffer_initial_group_control(&self, call: IncomingCall) -> bool {
        if !matches!(
            call.action,
            CallAction::GroupUpdate { .. } | CallAction::EncRekey { .. }
        ) || call.action.call_id().is_empty()
            || routed_call_sender(&call).to_non_ad() != call.action.call_creator().to_non_ad()
        {
            return false;
        }

        let mut pending = self.pending_controls();
        let now = crate::time::Instant::now();
        pending.retain(|queued| queued.expires_at > now);
        if self.active_calls().contains_key(call.action.call_id()) {
            return false;
        }
        if !call.stanza_id.is_empty()
            && pending.iter().any(|queued| {
                queued.call.stanza_id == call.stanza_id
                    && pending_initial_group_control_matches(
                        &queued.call,
                        call.action.call_id(),
                        call.action.call_creator(),
                    )
            })
        {
            return true;
        }
        let retained_bytes = pending_initial_group_control_retained_bytes(&call);
        if retained_bytes > MAX_PENDING_INITIAL_GROUP_CONTROL_BYTES {
            return false;
        }
        let mut pending_bytes = pending
            .iter()
            .map(|queued| pending_initial_group_control_retained_bytes(&queued.call))
            .sum::<usize>();
        // A control may replace older state for its own prospective call, but unrelated traffic
        // cannot make room by discarding a roster or epoch that already won the bounded race.
        let (matching_entries, matching_bytes) = pending
            .iter()
            .filter(|queued| {
                pending_initial_group_control_matches(
                    &queued.call,
                    call.action.call_id(),
                    call.action.call_creator(),
                )
            })
            .fold((0usize, 0usize), |(entries, bytes), queued| {
                (
                    entries.saturating_add(1),
                    bytes
                        .saturating_add(pending_initial_group_control_retained_bytes(&queued.call)),
                )
            });
        if pending.len().saturating_sub(matching_entries) >= MAX_PENDING_INITIAL_GROUP_CONTROLS
            || pending_bytes
                .saturating_sub(matching_bytes)
                .saturating_add(retained_bytes)
                > MAX_PENDING_INITIAL_GROUP_CONTROL_BYTES
        {
            return false;
        }
        while pending.len() >= MAX_PENDING_INITIAL_GROUP_CONTROLS
            || pending_bytes.saturating_add(retained_bytes)
                > MAX_PENDING_INITIAL_GROUP_CONTROL_BYTES
        {
            let index = pending
                .iter()
                .position(|queued| {
                    pending_initial_group_control_matches(
                        &queued.call,
                        call.action.call_id(),
                        call.action.call_creator(),
                    )
                })
                .expect("preflight proved a matching control can be evicted");
            let evicted = pending
                .remove(index)
                .expect("matching pending control must remain present");
            pending_bytes = pending_bytes
                .saturating_sub(pending_initial_group_control_retained_bytes(&evicted.call));
        }
        pending.push_back(PendingInitialGroupControl {
            call,
            expires_at: now + PENDING_INITIAL_GROUP_CONTROL_TTL,
        });
        true
    }

    /// Consume controls buffered before this exact group identity registered, preserving wire order.
    pub fn take_initial_group_controls(
        &self,
        call_id: &str,
        call_creator: &Jid,
    ) -> Vec<IncomingCall> {
        let creator = call_creator.to_non_ad();
        let mut pending = self.pending_controls();
        let now = crate::time::Instant::now();
        pending.retain(|queued| queued.expires_at > now);
        let mut retained = VecDeque::with_capacity(pending.len());
        let mut matched = Vec::new();
        while let Some(queued) = pending.pop_front() {
            if queued.call.action.call_id() == call_id
                && queued.call.action.call_creator().to_non_ad() == creator
            {
                matched.push(queued.call);
            } else {
                retained.push_back(queued);
            }
        }
        *pending = retained;
        matched
    }

    /// Listen for any call registration, then recheck the desired call id.
    pub fn listen_registration(&self) -> event_listener::EventListener {
        self.registration_event.listen()
    }

    /// Deliver a signaling event through the call's consumer queue.
    pub fn send_call_event(&self, call_id: &str, event: CallEvent) -> bool {
        let tx = self
            .active_calls()
            .get(call_id)
            .and_then(|entry| entry.event_tx.clone());
        tx.is_some_and(|tx| tx.force_send(event))
    }

    /// Deliver an event only while `generation` still owns this call-id.
    pub fn send_call_event_if_current(
        &self,
        call_id: &str,
        generation: u64,
        event: CallEvent,
    ) -> bool {
        let tx = self
            .active_calls()
            .get(call_id)
            .filter(|entry| entry.generation == generation)
            .and_then(|entry| entry.event_tx.clone());
        tx.is_some_and(|tx| tx.force_send(event))
    }

    /// Atomically apply a newer authoritative group snapshot to an active call.
    pub fn apply_group_update(&self, update: GroupCallUpdate) -> GroupStateApply {
        self.apply_group_update_inner(update, None)
    }

    /// Apply a group snapshot only while `generation` still owns this call-id.
    pub fn apply_group_update_if_current(
        &self,
        update: GroupCallUpdate,
        generation: u64,
    ) -> GroupStateApply {
        self.apply_group_update_inner(update, Some(generation))
    }

    fn apply_group_update_inner(
        &self,
        update: GroupCallUpdate,
        generation: Option<u64>,
    ) -> GroupStateApply {
        if super::engine::validate_group_relay_update(&update).is_err() {
            return GroupStateApply::InvalidSnapshot;
        }
        // Held so the commit lookup survives `update` being moved into the preview.
        let call_id = update.call_id.clone();
        let (applied, waiting_room_task, video_teardown, event) = {
            let ringing = self.ringing_calls();
            let mut map = self.active_calls();
            let Some(entry) = map
                .get(&call_id)
                .filter(|entry| generation.is_none_or(|current| entry.generation == current))
            else {
                return GroupStateApply::UnknownCall;
            };
            if let (Some(established), Some(relay)) =
                (entry.group_warp_mi_tag_len, update.relay.as_ref())
                && relay.warp_mi_tag_len.unwrap_or(4) as usize != established
            {
                // Reject before GroupCallState consumes the transaction. The media pipelines retain
                // their original packet boundary, so a corrected same-transaction refresh must
                // remain admissible instead of splitting registry and driver state.
                return GroupStateApply::InvalidSnapshot;
            }
            let downgrades_video = update.media == "audio"
                && (entry.session.is_video
                    || entry
                        .group
                        .as_ref()
                        .and_then(GroupCallState::snapshot)
                        .is_some_and(|snapshot| snapshot.media == "video"));
            let local_member = entry
                .group_invite_self_device
                .as_ref()
                .is_some_and(|device| {
                    Self::group_update_contains_connected_device(&update, &device.jid)
                });
            // One preview both decides admission and becomes the committed state. `apply_update`
            // mutates nothing on a rejected transaction, so a preview that did not apply is still
            // byte-identical to the entry's own state and can be stored back unconditionally.
            let mut preview = entry.group.clone().unwrap_or_else(|| {
                GroupCallState::new(
                    entry.session.call_id.clone(),
                    entry.session.call_creator.clone(),
                )
            });
            let applied = preview.apply_update(update);
            if applied == GroupStateApply::Applied {
                if entry.is_group_call && entry.session.phase() == CallPhase::Ringing {
                    let mut preview_session = entry.session.clone();
                    preview_session.group = preview.snapshot().cloned();
                    if !Self::ringing_group_update_fits(
                        &ringing,
                        &map,
                        &call_id,
                        entry,
                        &preview,
                        &preview_session,
                    ) {
                        // Preserve the transaction watermark so a corrected same-transaction
                        // snapshot can still be accepted.
                        return GroupStateApply::InvalidSnapshot;
                    }
                }
                let control = GroupControl::Update(Box::new(
                    preview
                        .snapshot()
                        .expect("an applied preview owns a snapshot")
                        .clone(),
                ));
                let fits = entry.group_ctl_tx.as_ref().map_or(
                    !entry.is_call_link || {
                        GroupControlQueue::accepts_with_capacity(
                            &control,
                            DEFAULT_CALL_EVENT_QUEUE_CAPACITY,
                        )
                    },
                    |group_ctl_tx| group_ctl_tx.accepts(&control),
                );
                if !fits {
                    // Do not consume the authoritative transaction when its committed form cannot
                    // fit one production media-driver slot, including before the sender attaches.
                    // A corrected same-transaction redelivery must remain admissible.
                    return GroupStateApply::InvalidSnapshot;
                }
            }
            let entry = map.get_mut(&call_id).expect("entry presence checked");
            // `group_mut()`'s side effect: any update claims the call as group media, applied or not.
            entry.is_group_call = true;
            entry.group = Some(preview);
            let admitted = applied == GroupStateApply::Applied
                && entry.is_call_link
                && entry.session.phase() == CallPhase::WaitingRoom
                && local_member;
            if admitted {
                let _ = entry.session.transition_to(CallPhase::Connecting);
            }
            let waiting_room_task = if admitted {
                entry.waiting_room_task.take()
            } else {
                None
            };
            let video_teardown = if applied == GroupStateApply::Applied && downgrades_video {
                entry.video = VideoNegotiation::new(false);
                entry.session.is_video = false;
                entry.video_teardown.take()
            } else {
                None
            };
            (
                applied,
                waiting_room_task,
                video_teardown,
                (applied == GroupStateApply::Applied).then(|| entry.group_update_event.clone()),
            )
        };
        // Abort the heartbeat outside the registry lock: executor cancellation is an external
        // callback and must never be able to re-enter while the map is borrowed.
        drop(waiting_room_task);
        if let Some(video_teardown) = video_teardown {
            video_teardown();
        }
        if let Some(event) = event {
            event.notify(usize::MAX);
        }
        applied
    }

    fn group_update_contains_connected_device(update: &GroupCallUpdate, device: &Jid) -> bool {
        update.participants.iter().any(|participant| {
            participant.is_connected()
                && participant
                    .devices
                    .iter()
                    .any(|candidate| group_device_is_local(participant, candidate, device))
        })
    }

    /// Atomically apply a newer waiting-room snapshot to an active call-link call.
    pub fn apply_waiting_room(&self, room: WaitingRoom) -> GroupStateApply {
        self.apply_waiting_room_inner(room, None)
    }

    /// Apply a waiting-room snapshot only while `generation` still owns this call-id.
    pub fn apply_waiting_room_if_current(
        &self,
        room: WaitingRoom,
        generation: u64,
    ) -> GroupStateApply {
        self.apply_waiting_room_inner(room, Some(generation))
    }

    fn apply_waiting_room_inner(
        &self,
        room: WaitingRoom,
        generation: Option<u64>,
    ) -> GroupStateApply {
        let mut map = self.active_calls();
        let Some(entry) = map
            .get_mut(&room.call_id)
            .filter(|entry| generation.is_none_or(|current| entry.generation == current))
        else {
            return GroupStateApply::UnknownCall;
        };
        if !entry.is_call_link
            && !entry
                .group
                .as_ref()
                .is_some_and(|group| group.waiting_room().is_some())
        {
            return GroupStateApply::InvalidSnapshot;
        }
        entry.group_mut().apply_waiting_room(room)
    }

    /// Commit a local waiting-room toggle only while `generation` owns this call-id.
    pub fn set_waiting_room_enabled_if_current(
        &self,
        call_id: &str,
        generation: u64,
        enabled: bool,
    ) -> bool {
        self.active_calls()
            .get_mut(call_id)
            .filter(|entry| entry.generation == generation)
            .and_then(|entry| entry.group.as_mut())
            .is_some_and(|group| group.set_waiting_room_enabled(enabled))
    }

    /// Update one participant's persistent hand state for an active group call.
    pub fn set_raised_hand(&self, call_id: &str, participant: &Jid, raised: bool) -> bool {
        let mut map = self.active_calls();
        let Some(group) = map.get_mut(call_id).and_then(|entry| entry.group.as_mut()) else {
            return false;
        };
        group.set_raised_hand(participant, raised);
        true
    }

    /// Update local hand state only while `generation` still owns this call-id.
    pub fn set_raised_hand_if_current(
        &self,
        call_id: &str,
        generation: u64,
        participant: &Jid,
        raised: bool,
    ) -> bool {
        let mut map = self.active_calls();
        let Some(group) = map
            .get_mut(call_id)
            .filter(|entry| entry.generation == generation)
            .and_then(|entry| entry.group.as_mut())
        else {
            return false;
        };
        group.set_raised_hand(participant, raised);
        true
    }

    /// Update one participant's screen-share state for an active group call.
    pub fn set_screen_share(
        &self,
        call_id: &str,
        participant: &Jid,
        screen_share: ScreenShare,
    ) -> bool {
        let mut map = self.active_calls();
        let Some(group) = map.get_mut(call_id).and_then(|entry| entry.group.as_mut()) else {
            return false;
        };
        group.set_screen_share(participant, screen_share);
        true
    }

    /// Update local screen-share state only while `generation` still owns this call-id.
    pub fn set_screen_share_if_current(
        &self,
        call_id: &str,
        generation: u64,
        participant: &Jid,
        screen_share: ScreenShare,
    ) -> bool {
        let mut map = self.active_calls();
        let Some(group) = map
            .get_mut(call_id)
            .filter(|entry| entry.generation == generation)
            .and_then(|entry| entry.group.as_mut())
        else {
            return false;
        };
        group.set_screen_share(participant, screen_share);
        true
    }

    /// Read a clone of the current group-call state.
    pub fn group_state(&self, call_id: &str) -> Option<GroupCallState> {
        self.active_calls()
            .get(call_id)
            .and_then(|entry| entry.group.clone())
    }

    /// Whether the registered call-id belongs to a group-call generation.
    pub fn is_group_call(&self, call_id: &str) -> bool {
        self.active_calls()
            .get(call_id)
            .is_some_and(|entry| entry.is_group_call)
    }

    /// Whether one exact call generation is registered as group media.
    pub fn is_group_call_if_current(&self, call_id: &str, generation: u64) -> bool {
        self.active_calls()
            .get(call_id)
            .is_some_and(|entry| entry.generation == generation && entry.is_group_call)
    }

    /// The exact ringing group generation matching its signaling creator.
    pub fn ringing_group_generation(&self, call_id: &str, call_creator: &Jid) -> Option<u64> {
        self.active_calls()
            .get(call_id)
            .filter(|entry| {
                entry.is_group_call
                    && entry.session.phase() == CallPhase::Ringing
                    && entry.session.call_creator == *call_creator
            })
            .map(|entry| entry.generation)
    }

    /// Read group state only when `generation` still owns this call-id.
    pub fn group_state_if_current(&self, call_id: &str, generation: u64) -> Option<GroupCallState> {
        self.active_calls()
            .get(call_id)
            .filter(|entry| entry.generation == generation)
            .and_then(|entry| entry.group.clone())
    }

    /// Whether one exact active group generation carries the supplied signaling creator.
    pub fn group_creator_matches_if_current(
        &self,
        call_id: &str,
        generation: u64,
        call_creator: &Jid,
    ) -> bool {
        self.active_calls().get(call_id).is_some_and(|entry| {
            entry.generation == generation
                && entry.is_group_call
                && entry.session.call_creator == *call_creator
        })
    }

    /// Whether a routed signaling sender is the creator or belongs to this exact active group call.
    ///
    /// The creator is the only trusted bootstrap sender before a call-link admission installs its
    /// first authoritative roster. Once a roster exists, its participants are authorized too.
    pub fn group_sender_authorized(&self, call_id: &str, call_creator: &Jid, sender: &Jid) -> bool {
        if sender.to_non_ad() == call_creator.to_non_ad()
            && self
                .active_calls()
                .get(call_id)
                .is_some_and(|entry| entry.session.call_creator == *call_creator)
        {
            return true;
        }
        self.canonical_group_participant(call_id, call_creator, sender)
            .is_some()
    }

    /// Whether a routed sender belongs to one exact active call generation.
    pub fn group_sender_authorized_if_current(
        &self,
        call_id: &str,
        generation: u64,
        call_creator: &Jid,
        sender: &Jid,
    ) -> bool {
        if sender.to_non_ad() == call_creator.to_non_ad()
            && self.active_calls().get(call_id).is_some_and(|entry| {
                entry.generation == generation && entry.session.call_creator == *call_creator
            })
        {
            return true;
        }
        self.canonical_group_participant_if_current(call_id, generation, call_creator, sender)
            .is_some()
    }

    /// Whether the routed sender is the creator of this exact active group-call generation.
    pub fn group_creator_authorized_if_current(
        &self,
        call_id: &str,
        generation: u64,
        call_creator: &Jid,
        sender: &Jid,
    ) -> bool {
        self.active_calls().get(call_id).is_some_and(|entry| {
            entry.generation == generation
                && entry.session.call_creator == *call_creator
                && sender.to_non_ad() == call_creator.to_non_ad()
        })
    }

    /// Resolve an authenticated routed sender to the roster's stable participant JID.
    ///
    /// Bare PN aliases are accepted by signaling but must not become persistent control keys:
    /// roster and media state are keyed by the authoritative participant LID.
    pub fn canonical_group_participant(
        &self,
        call_id: &str,
        call_creator: &Jid,
        sender: &Jid,
    ) -> Option<Jid> {
        let map = self.active_calls();
        let entry = map
            .get(call_id)
            .filter(|entry| entry.session.call_creator == *call_creator)?;
        Self::canonical_group_participant_for_entry(entry, sender)
    }

    /// Resolve a sender only while `generation` still owns this call-id.
    pub fn canonical_group_participant_if_current(
        &self,
        call_id: &str,
        generation: u64,
        call_creator: &Jid,
        sender: &Jid,
    ) -> Option<Jid> {
        let map = self.active_calls();
        let entry = map.get(call_id).filter(|entry| {
            entry.generation == generation && entry.session.call_creator == *call_creator
        })?;
        Self::canonical_group_participant_for_entry(entry, sender)
    }

    /// Resolve a routed device to the exact device JID retained by the authoritative roster.
    pub fn canonical_group_device_if_current(
        &self,
        call_id: &str,
        generation: u64,
        call_creator: &Jid,
        sender: &Jid,
    ) -> Option<Jid> {
        let map = self.active_calls();
        let entry = map.get(call_id).filter(|entry| {
            entry.generation == generation && entry.session.call_creator == *call_creator
        })?;
        let snapshot = entry.group.as_ref().and_then(GroupCallState::snapshot)?;
        snapshot
            .participants
            .iter()
            .filter(|participant| participant.is_connected())
            .flat_map(|participant| participant.devices.iter())
            .find(|device| device.jid.device_eq(sender))
            .map(|device| device.jid.clone())
    }

    fn canonical_group_participant_for_entry(entry: &CallEntry, sender: &Jid) -> Option<Jid> {
        let snapshot = entry.group.as_ref().and_then(GroupCallState::snapshot)?;
        let sender_user = sender.to_non_ad();
        snapshot.participants.iter().find_map(|participant| {
            if !participant.is_connected() {
                return None;
            }
            let authorized = if sender.device == 0 {
                participant.jid.to_non_ad() == sender_user
                    || participant
                        .pn
                        .as_ref()
                        .is_some_and(|pn| pn.to_non_ad() == sender_user)
            } else {
                participant
                    .devices
                    .iter()
                    .any(|device| device.jid.device_eq(sender))
            };
            authorized.then(|| participant.jid.to_non_ad())
        })
    }

    /// Register a call, returning its generation token. A same-call-id re-offer (retry/glare)
    /// REPLACES the prior registration, aborting its media task; the returned generation
    /// distinguishes the new call from the old. Pass it to [`set_media_task`](Self::set_media_task)
    /// and [`remove_if_current`](Self::remove_if_current) so a finishing task only ever reaps its
    /// OWN entry, never a newer replacement.
    pub fn insert(&self, session: CallSession) -> u64 {
        self.insert_inner(session, true, false, false)
    }

    /// Register a group call before its first authoritative roster is available.
    pub fn insert_group(&self, session: CallSession) -> u64 {
        self.insert_inner(session, true, true, false)
    }

    /// Register a group call only after validating any initial authoritative snapshot.
    ///
    /// Waiting-room call links may not have a roster yet, so an absent snapshot remains valid.
    pub fn insert_group_checked(&self, session: CallSession) -> Result<u64, GroupStateApply> {
        self.insert_group_checked_inner(session, false)
    }

    /// Register a call-link generation after validating any immediately admitted group snapshot.
    pub fn insert_call_link_checked(&self, session: CallSession) -> Result<u64, GroupStateApply> {
        self.insert_group_checked_inner(session, true)
    }

    fn insert_group_checked_inner(
        &self,
        session: CallSession,
        is_call_link: bool,
    ) -> Result<u64, GroupStateApply> {
        if let Some(initial_update) = session.group.as_ref() {
            if super::engine::validate_group_relay_update(initial_update).is_err() {
                return Err(GroupStateApply::InvalidSnapshot);
            }
            let mut initial_state =
                GroupCallState::new(session.call_id.clone(), session.call_creator.clone());
            let applied = initial_state.apply_update(initial_update.clone());
            if applied != GroupStateApply::Applied {
                return Err(applied);
            }
            if is_call_link {
                let committed = initial_state
                    .snapshot()
                    .expect("an applied initial state owns a snapshot")
                    .clone();
                if !GroupControlQueue::accepts_with_capacity(
                    &GroupControl::Update(Box::new(committed)),
                    DEFAULT_CALL_EVENT_QUEUE_CAPACITY,
                ) {
                    return Err(GroupStateApply::InvalidSnapshot);
                }
            }
        }
        Ok(self.insert_inner(session, true, true, is_call_link))
    }

    /// Register an active-group invitation without marking it answered.
    ///
    /// Roster and epoch updates can land while the user decides, while the separate ringing set
    /// still classifies a remote timeout as a missed call.
    pub fn insert_ringing_group(&self, session: CallSession) -> u64 {
        self.insert_inner(session, false, true, false)
    }

    /// Register an incoming group offer unless this call id already belongs to an active call.
    ///
    /// The ringing marker and entry replacement are committed under the same lock pair so a
    /// redelivered offer cannot race an acceptance and supersede the accepted generation.
    pub fn insert_ringing_group_if_inactive(
        &self,
        mut session: CallSession,
    ) -> Result<Option<u64>, GroupStateApply> {
        let Some(initial_update) = session.group.as_ref() else {
            return Err(GroupStateApply::InvalidSnapshot);
        };
        if super::engine::validate_group_relay_update(initial_update).is_err() {
            return Err(GroupStateApply::InvalidSnapshot);
        }
        let mut initial_state =
            GroupCallState::new(session.call_id.clone(), session.call_creator.clone());
        let initial_apply = initial_state.apply_update(initial_update.clone());
        if initial_apply != GroupStateApply::Applied {
            return Err(initial_apply);
        }
        let call_id = session.call_id.clone();
        let generation = {
            let mut ringing = self.ringing_calls();
            let mut map = self.active_calls();
            if let Some(entry) = map.get(&call_id) {
                if entry.session.phase() != CallPhase::Ringing {
                    return Ok(None);
                }
                if entry.session.call_creator != session.call_creator {
                    return Err(GroupStateApply::IdentityMismatch);
                }
                let preview = if let Some(update) = session.group.take() {
                    let mut preview = entry.group.clone().unwrap_or_else(|| {
                        GroupCallState::new(
                            entry.session.call_id.clone(),
                            entry.session.call_creator.clone(),
                        )
                    });
                    match preview.apply_update(update) {
                        GroupStateApply::Applied => {
                            let mut preview_session = entry.session.clone();
                            preview_session.group = preview.snapshot().cloned();
                            if !Self::ringing_group_update_fits(
                                &ringing,
                                &map,
                                &call_id,
                                entry,
                                &preview,
                                &preview_session,
                            ) {
                                return Err(GroupStateApply::InvalidSnapshot);
                            }
                            Some((preview, preview_session.group))
                        }
                        GroupStateApply::Stale => None,
                        reason => return Err(reason),
                    }
                } else {
                    None
                };
                let entry = map.get_mut(&call_id).expect("entry presence checked");
                if let Some((preview, preview_group)) = preview {
                    entry.session.group = preview_group;
                    entry.group = Some(preview);
                }
                session.group = entry
                    .group
                    .as_ref()
                    .and_then(GroupCallState::snapshot)
                    .cloned();
                entry.video = VideoNegotiation::new(session.is_video);
                entry.session = session;
                ringing.insert(call_id);
                entry.generation
            } else {
                // Only a brand-new registration is charged against the aggregate, so the walk over
                // every ringing group call belongs here rather than ahead of the update branch.
                let (ringing_group_entries, ringing_group_bytes) = map
                    .iter()
                    .filter(|(call_id, entry)| {
                        ringing.contains(*call_id)
                            && entry.is_group_call
                            && entry.session.phase() == CallPhase::Ringing
                    })
                    .fold((0usize, 0usize), |(entries, bytes), (call_id, entry)| {
                        (
                            entries.saturating_add(1),
                            bytes.saturating_add(entry.retained_bytes(call_id)),
                        )
                    });
                let generation = self.next_gen.fetch_add(1, Ordering::Relaxed);
                let entry = Self::new_entry(session, generation, true, false);
                if ringing_group_entries >= MAX_RINGING_GROUP_CALLS
                    || ringing_group_bytes.saturating_add(entry.retained_bytes(&call_id))
                        > MAX_RINGING_GROUP_CALL_BYTES
                {
                    return Err(GroupStateApply::InvalidSnapshot);
                }
                ringing.insert(call_id.clone());
                map.insert(call_id, entry);
                generation
            }
        };
        self.registration_event.notify(usize::MAX);
        Ok(Some(generation))
    }

    fn insert_inner(
        &self,
        session: CallSession,
        consume_ringing: bool,
        force_group: bool,
        is_call_link: bool,
    ) -> u64 {
        let generation = self.next_gen.fetch_add(1, Ordering::Relaxed);
        // Registering a call as active answers (accept) or places (outgoing) it: it is no longer
        // merely ringing. A no-op for an outgoing call (never ringing); for an accepted incoming
        // offer this clears the ringing flag so a later `<terminate>` reads as ended, not missed.
        if consume_ringing {
            self.take_ringing(&session.call_id);
        }
        let prev = {
            let mut map = self.active_calls();
            map.insert(
                session.call_id.clone(),
                Self::new_entry(session, generation, force_group, is_call_link),
            )
        };
        // The superseded entry drops here, OUTSIDE the lock: its media-task AbortHandle aborts and its
        // on_terminal hook fires (the old generation ended). Running those closures off-lock keeps them
        // from re-entering or poisoning the registry mutex.
        drop(prev);
        self.registration_event.notify(usize::MAX);
        generation
    }

    /// Accept exactly the ringing generation observed before the signaling await.
    pub fn accept_ringing_if_current(&self, call_id: &str, generation: u64) -> bool {
        let mut ringing = self.ringing_calls();
        let mut map = self.active_calls();
        let Some(entry) = map
            .get_mut(call_id)
            .filter(|entry| entry.generation == generation)
        else {
            return false;
        };
        if !entry.session.transition_to(CallPhase::Connecting) {
            return false;
        }
        ringing.remove(call_id);
        true
    }

    /// Remove exactly the ringing generation carried by a dispatched offer.
    pub fn reject_ringing_if_current(&self, call_id: &str, generation: u64) -> bool {
        let removed = {
            let mut ringing = self.ringing_calls();
            let mut map = self.active_calls();
            if map.get(call_id).is_some_and(|entry| {
                entry.generation == generation && entry.session.phase() == CallPhase::Ringing
            }) {
                ringing.remove(call_id);
                map.remove(call_id)
            } else {
                None
            }
        };
        let rejected = removed.is_some();
        drop(removed);
        rejected
    }

    fn new_entry(
        session: CallSession,
        generation: u64,
        force_group: bool,
        is_call_link: bool,
    ) -> CallEntry {
        let video = VideoNegotiation::new(session.is_video);
        let is_group_call = force_group || session.group.is_some();
        let group = session.group.as_ref().map(|update| {
            let mut state =
                GroupCallState::new(session.call_id.clone(), session.call_creator.clone());
            let _ = state.apply_update(update.clone());
            state
        });
        CallEntry {
            // A fresh call starts upright; the app announces a rotation when it
            // has one, and it then outlives every negotiation rebuild.
            self_video_orientation: 0,
            session,
            media_task: None,
            waiting_room_task: None,
            generation,
            rekey_tx: None,
            event_tx: None,
            video_ctl_tx: None,
            group_ctl_tx: None,
            group_warp_mi_tag_len: None,
            pending_group_epoch: None,
            video_teardown: None,
            event_publication_reserved: Arc::new(AtomicBool::new(false)),
            video_transition_lock: Arc::new(AsyncMutex::new(())),
            group_transition_lock: Arc::new(AsyncMutex::new(())),
            group_update_event: Arc::new(event_listener::Event::new()),
            video,
            is_group_call,
            is_call_link,
            group,
            group_invite_self_device: None,
            group_invite_peer_device: None,
            group_invite_peer_selected: false,
            on_terminal: None,
        }
    }

    /// Promote a previously registered active-group invitation into media setup without replacing
    /// its entry. This preserves any newer roster and buffered epoch that arrived during acceptance.
    pub fn promote_ringing_group(&self, mut session: CallSession) -> Option<u64> {
        let call_id = session.call_id.clone();
        let generation = {
            let mut map = self.active_calls();
            let entry = map.get_mut(&call_id)?;
            if !matches!(
                entry.session.phase(),
                CallPhase::Ringing | CallPhase::Connecting
            ) || entry.session.call_creator != session.call_creator
                || entry.group.is_none()
            {
                return None;
            }
            if let Some(latest) = entry
                .group
                .as_ref()
                .and_then(GroupCallState::snapshot)
                .cloned()
            {
                session.group = Some(latest);
            }
            entry.video = VideoNegotiation::new(session.is_video);
            entry.session = session;
            entry.generation
        };
        self.take_ringing(&call_id);
        Some(generation)
    }

    /// Promote only the exact ringing group generation captured for an incoming offer.
    pub fn promote_ringing_group_if_current(
        &self,
        mut session: CallSession,
        generation: u64,
    ) -> bool {
        let call_id = session.call_id.clone();
        let mut ringing = self.ringing_calls();
        let mut map = self.active_calls();
        let Some(entry) = map.get_mut(&call_id).filter(|entry| {
            entry.generation == generation
                && matches!(
                    entry.session.phase(),
                    CallPhase::Ringing | CallPhase::Connecting
                )
                && entry.session.call_creator == session.call_creator
                && entry.group.is_some()
        }) else {
            return false;
        };
        if let Some(latest) = entry
            .group
            .as_ref()
            .and_then(GroupCallState::snapshot)
            .cloned()
        {
            session.group = Some(latest);
        }
        entry.video = VideoNegotiation::new(session.is_video);
        entry.session = session;
        ringing.remove(&call_id);
        true
    }

    /// Attach (or replace) the media task for the call registered under `generation`. If the call
    /// was removed or superseded by a newer generation, the handle is aborted immediately so its
    /// task can't outlive the call.
    pub fn set_media_task(&self, call_id: &str, generation: u64, handle: AbortHandle) {
        let aborted = {
            let mut map = self.active_calls();
            match map.get_mut(call_id) {
                Some(entry) if entry.generation == generation => entry.media_task.replace(handle),
                _ => Some(handle),
            }
        };
        // Both exits abort off-lock, for the reason spelled out at `insert_inner`.
        drop(aborted);
    }

    /// Attach the repeating waiting-room heartbeat to one call generation.
    pub fn set_waiting_room_task(&self, call_id: &str, generation: u64, handle: AbortHandle) {
        let replaced = {
            let mut map = self.active_calls();
            let Some(entry) = map.get_mut(call_id).filter(|entry| {
                entry.generation == generation && entry.session.phase() == CallPhase::WaitingRoom
            }) else {
                drop(map);
                drop(handle);
                return;
            };
            entry.waiting_room_task.replace(handle)
        };
        drop(replaced);
    }

    /// Listen for a committed group update or terminal replacement of this generation.
    pub fn listen_group_update(
        &self,
        call_id: &str,
        generation: u64,
    ) -> Option<event_listener::EventListener> {
        self.active_calls()
            .get(call_id)
            .filter(|entry| entry.generation == generation)
            .map(|entry| entry.group_update_event.listen())
    }

    /// Seed the local active device used for group media and call-link admission.
    pub fn set_group_invite_self_device(
        &self,
        call_id: &str,
        generation: u64,
        device: GroupCallDevice,
    ) -> bool {
        let (waiting_room_task, event) = {
            let mut map = self.active_calls();
            let Some(entry) = map
                .get_mut(call_id)
                .filter(|entry| entry.generation == generation)
            else {
                return false;
            };
            let admitted = entry.is_call_link
                && entry.session.phase() == CallPhase::WaitingRoom
                && entry.group.as_ref().is_some_and(|state| {
                    state.snapshot().is_some_and(|update| {
                        Self::group_update_contains_connected_device(update, &device.jid)
                    })
                });
            entry.group_invite_self_device = Some(device);
            if admitted {
                let _ = entry.session.transition_to(CallPhase::Connecting);
            }
            (
                if admitted {
                    entry.waiting_room_task.take()
                } else {
                    None
                },
                admitted.then(|| entry.group_update_event.clone()),
            )
        };
        drop(waiting_room_task);
        if let Some(event) = event {
            event.notify(usize::MAX);
        }
        true
    }

    /// Record the peer device and capability selected by preaccept/accept signaling.
    pub fn set_group_invite_peer_device(
        &self,
        call_id: &str,
        generation: u64,
        device: GroupCallDevice,
    ) -> bool {
        self.active_calls()
            .get_mut(call_id)
            .filter(|entry| entry.generation == generation)
            .is_some_and(|entry| {
                if entry.group_invite_peer_selected {
                    return false;
                }
                entry.group_invite_peer_device = Some(device);
                true
            })
    }

    /// Select the device that actually accepted the 1:1 call for later group promotion.
    ///
    /// The first accept wins. It replaces (or clears) capability learned from an earlier sibling
    /// preaccept, and subsequent accepts or delayed preaccepts cannot replace that decision.
    pub fn select_group_invite_peer_device(
        &self,
        call_id: &str,
        generation: u64,
        device: Option<GroupCallDevice>,
    ) -> bool {
        self.active_calls()
            .get_mut(call_id)
            .filter(|entry| entry.generation == generation)
            .is_some_and(|entry| {
                if entry.group_invite_peer_selected {
                    return false;
                }
                entry.group_invite_peer_device = device;
                entry.group_invite_peer_selected = true;
                true
            })
    }

    /// Select an accepting device that omitted capability signaling.
    ///
    /// Capability retained from a preaccept belongs to the accepting device only when their exact
    /// device JIDs match. A sibling accept clears it so promotion cannot borrow another device's
    /// keys or feature advertisement.
    pub fn select_group_invite_peer_without_capability(
        &self,
        call_id: &str,
        generation: u64,
        accepting_device: &Jid,
    ) -> bool {
        self.active_calls()
            .get_mut(call_id)
            .filter(|entry| entry.generation == generation)
            .is_some_and(|entry| {
                if entry.group_invite_peer_selected {
                    return false;
                }
                if !entry
                    .group_invite_peer_device
                    .as_ref()
                    .is_some_and(|device| device.jid.device_eq(accepting_device))
                {
                    entry.group_invite_peer_device = None;
                }
                entry.group_invite_peer_selected = true;
                true
            })
    }

    /// Build the connected two-user roster needed to add the first ad-hoc participant.
    pub fn group_invite_fallback_roster(
        &self,
        call_id: &str,
        generation: u64,
    ) -> Option<Vec<GroupCallParticipant>> {
        let map = self.active_calls();
        let entry = map
            .get(call_id)
            .filter(|entry| entry.generation == generation)?;
        let self_device = entry.group_invite_self_device.clone()?;
        let peer_device = entry.group_invite_peer_device.clone()?;
        Some(vec![
            GroupCallParticipant::builder()
                .jid(self_device.jid.to_non_ad())
                .state("connected".to_string())
                .devices(vec![self_device])
                .build(),
            GroupCallParticipant::builder()
                .jid(peer_device.jid.to_non_ad())
                .state("connected".to_string())
                .devices(vec![peer_device])
                .build(),
        ])
    }

    /// Attach the wake-on-removal hook for the call under `generation`: when the entry is removed
    /// (terminal stanza / disconnect / supersession), `notify` runs to wake a parked `wait_ended()`,
    /// even if no media task was attached yet. Generation-guarded and ignored if removed/superseded.
    pub fn set_ended_notify(
        &self,
        call_id: &str,
        generation: u64,
        notify: impl FnOnce() + Send + 'static,
    ) {
        if let Some(entry) = self.active_calls()
            .get_mut(call_id)
            && entry.generation == generation
            // Set-once: a second call for the same generation would otherwise drop (and fire) the
            // existing hook in place, a false terminal notification. The first hook wins.
            && entry.on_terminal.is_none()
        {
            entry.on_terminal = Some(EndedNotify(Some(Box::new(notify))));
        }
    }

    /// Store the per-call recv-rekey sender (the drive loop holds the matching receiver). Generation-
    /// guarded and ignored if the call was removed or superseded, so a stale sender can't outlive its
    /// call. Caller side only.
    pub fn set_rekey_sender(
        &self,
        call_id: &str,
        generation: u64,
        tx: async_channel::Sender<String>,
    ) {
        if let Some(entry) = self.active_calls().get_mut(call_id)
            && entry.generation == generation
        {
            entry.rekey_tx = Some(tx);
        }
    }

    /// Store the call's consumer-facing event sender, video-control sender, and the local-video
    /// teardown hook, so the signaling handler can surface `<video state>` changes, steer the video
    /// plane mid-call, and fully release the endpoints on a refused upgrade. Generation-guarded like
    /// the rekey sender.
    pub fn set_video_channels(
        &self,
        call_id: &str,
        generation: u64,
        event_tx: async_channel::Sender<CallEvent>,
        video_ctl_tx: VideoControlSender,
        video_teardown: Box<dyn Fn() + Send + Sync>,
    ) {
        if let Some(entry) = self.active_calls().get_mut(call_id)
            && entry.generation == generation
        {
            entry.event_tx = Some(CallEventQueue::new(event_tx));
            entry.video_ctl_tx = Some(video_ctl_tx);
            entry.video_teardown = Some(video_teardown);
        }
    }

    /// Attach the group-media control sender for this call generation.
    pub fn set_group_control_sender(
        &self,
        call_id: &str,
        generation: u64,
        warp_mi_tag_len: Option<usize>,
        tx: async_channel::Sender<GroupControl>,
    ) -> bool {
        // Publish and flush under one guard: a concurrent epoch router that observes the sender
        // cannot enqueue a newer epoch before the buffered startup epoch.
        let mut map = self.active_calls();
        let Some(entry) = map
            .get_mut(call_id)
            .filter(|entry| entry.generation == generation)
        else {
            return false;
        };
        if let (Some(established), Some(relay)) = (
            warp_mi_tag_len,
            entry
                .group
                .as_ref()
                .and_then(GroupCallState::snapshot)
                .and_then(|snapshot| snapshot.relay.as_ref()),
        ) && relay.warp_mi_tag_len.unwrap_or(4) as usize != established
        {
            return false;
        }
        let tx = GroupControlQueue::new(tx);
        let update = entry
            .group
            .as_ref()
            .and_then(GroupCallState::snapshot)
            .cloned();
        let pending_epoch = entry.pending_group_epoch.take();
        let mut unqueued_epoch = None;
        let queued = match (update, pending_epoch) {
            (Some(update), Some(epoch)) => {
                let transition = GroupControl::Transition {
                    update: Box::new(update),
                    epoch,
                };
                if tx.accepts(&transition) {
                    match tx.try_send_recover(transition) {
                        Ok(()) => true,
                        Err(control) => {
                            unqueued_epoch = retained_epoch(control);
                            false
                        }
                    }
                } else {
                    // The engine was initialized from the retained roster before this sender is
                    // attached. If adding the epoch tips their indivisible replay over one slot's
                    // byte budget, replay the same roster and epoch in order instead.
                    match transition {
                        GroupControl::Transition { update, epoch } => {
                            if !tx.try_send(GroupControl::Update(update)) {
                                unqueued_epoch = Some(epoch);
                                false
                            } else {
                                match tx.try_send_recover(GroupControl::RawEpoch(epoch)) {
                                    Ok(()) => true,
                                    Err(control) => {
                                        unqueued_epoch = retained_epoch(control);
                                        false
                                    }
                                }
                            }
                        }
                        GroupControl::Update(_)
                        | GroupControl::RawEpoch(_)
                        | GroupControl::Reaction(_) => false,
                    }
                }
            }
            (Some(update), None) => tx.try_send(GroupControl::Update(Box::new(update))),
            (None, Some(epoch)) => match tx.try_send_recover(GroupControl::RawEpoch(epoch)) {
                Ok(()) => true,
                Err(control) => {
                    unqueued_epoch = retained_epoch(control);
                    false
                }
            },
            (None, None) => true,
        };
        if !queued {
            entry.pending_group_epoch = unqueued_epoch;
            return false;
        }
        entry.group_ctl_tx = Some(tx);
        entry.group_warp_mi_tag_len = warp_mi_tag_len;
        true
    }

    /// Deliver an authoritative roster only while `generation` owns this call-id. Before media
    /// attaches, the committed registry snapshot is itself the retained delivery; attaching the
    /// group-control sender replays that latest snapshot.
    pub fn send_group_update_if_current(
        &self,
        call_id: &str,
        generation: u64,
        update: GroupCallUpdate,
    ) -> bool {
        let delivery = {
            let map = self.active_calls();
            let Some(entry) = map
                .get(call_id)
                .filter(|entry| entry.generation == generation)
            else {
                return false;
            };
            let Some(tx) = entry.group_ctl_tx.clone() else {
                return true;
            };
            // `apply_group_update_if_current` may have inherited relay material omitted by this
            // wire update. Route that committed snapshot so mailbox coalescing cannot replace a
            // relay refresh with a later roster-only payload that drops it.
            let update = entry
                .group
                .as_ref()
                .and_then(GroupCallState::snapshot)
                .filter(|committed| committed.transaction_id >= update.transaction_id)
                .cloned()
                .unwrap_or(update);
            (tx, update)
        };
        Self::force_send_preserving_epoch(&delivery.0, GroupControl::Update(Box::new(delivery.1)))
    }

    /// Deliver one decrypted shared epoch only while `generation` owns this call-id.
    pub fn send_group_epoch_if_current(
        &self,
        call_id: &str,
        generation: u64,
        transaction_id: u32,
        raw_epoch: Vec<u8>,
    ) -> bool {
        let epoch = GroupRawEpoch::new(transaction_id, raw_epoch);
        let delivery = {
            let mut map = self.active_calls();
            let Some(entry) = map
                .get_mut(call_id)
                .filter(|entry| entry.generation == generation)
            else {
                return false;
            };
            Self::retain_or_route_group_epoch(entry, epoch)
        };
        if let Some((tx, update, epoch)) = delivery {
            let command = match update {
                Some(update) => GroupControl::Transition {
                    update: Box::new(update),
                    epoch,
                },
                None => GroupControl::RawEpoch(epoch),
            };
            Self::force_send_preserving_epoch(&tx, command)
        } else {
            true
        }
    }

    /// The retained epoch transaction for one call generation before its media driver attaches.
    pub fn pending_group_epoch_transaction_if_current(
        &self,
        call_id: &str,
        generation: u64,
    ) -> Option<u32> {
        self.active_calls()
            .get(call_id)
            .filter(|entry| entry.generation == generation)
            .and_then(|entry| entry.pending_group_epoch.as_ref())
            .map(|epoch| epoch.transaction_id)
    }

    /// Keep the newest epoch resident when a bounded group-control mailbox sheds older commands.
    /// Roster updates may be coalesced under backpressure; losing the only pending epoch would leave
    /// the media engine permanently unable to decrypt the latest generation.
    fn force_send_preserving_epoch(tx: &GroupControlQueue, mut command: GroupControl) -> bool {
        if !tx.accepts(&command) {
            return false;
        }
        let latest_update_transaction = match &command {
            GroupControl::Update(update) | GroupControl::Transition { update, .. } => {
                Some(update.transaction_id)
            }
            GroupControl::RawEpoch(_) | GroupControl::Reaction(_) => None,
        };
        loop {
            let queued_epoch = command.epoch_transaction_id();
            tx.max_payload_bytes
                .fetch_max(command.heap_bytes(), Ordering::Relaxed);
            // Each retry keeps the newer of the queued and evicted epochs. If the rotation reaches
            // the roster this delivery just inserted, put that roster back once and shed the next
            // epoch instead. Existing Transition pairs remain indivisible, while an epoch-only full
            // mailbox still reserves one slot for the newest authoritative snapshot.
            match tx.tx.force_send(command) {
                Ok(Some(evicted)) => {
                    let evicted_latest_update = latest_update_transaction.is_some_and(
                        |latest_transaction| match &evicted {
                            GroupControl::Update(update)
                            | GroupControl::Transition { update, .. } => {
                                update.transaction_id == latest_transaction
                            }
                            GroupControl::RawEpoch(_) | GroupControl::Reaction(_) => false,
                        },
                    );
                    if evicted_latest_update {
                        tx.max_payload_bytes
                            .fetch_max(evicted.heap_bytes(), Ordering::Relaxed);
                        return tx.tx.force_send(evicted).is_ok();
                    }
                    if evicted
                        .epoch_transaction_id()
                        .is_some_and(|evicted_transaction| {
                            queued_epoch.is_none_or(|queued| evicted_transaction > queued)
                        })
                    {
                        command = evicted;
                    } else {
                        return true;
                    }
                }
                Ok(None) => return true,
                Err(_) => return false,
            }
        }
    }

    fn retain_or_route_group_epoch(
        entry: &mut CallEntry,
        epoch: GroupRawEpoch,
    ) -> Option<(GroupControlQueue, Option<GroupCallUpdate>, GroupRawEpoch)> {
        if let Some(tx) = entry.group_ctl_tx.clone() {
            let update = entry
                .group
                .as_ref()
                .and_then(GroupCallState::snapshot)
                .cloned();
            return Some((tx, update, epoch));
        }
        let replace = entry
            .pending_group_epoch
            .as_ref()
            .is_none_or(|pending| epoch.transaction_id > pending.transaction_id);
        if replace {
            entry.pending_group_epoch = Some(epoch);
        }
        None
    }

    /// Queue one RTC reaction on the active group media stream.
    pub fn send_group_reaction(&self, call_id: &str, emoji: String) -> bool {
        if crate::voip::app_data::encode_reaction(1, &emoji).is_err() {
            return false;
        }
        let tx = self
            .active_calls()
            .get(call_id)
            .filter(|entry| entry.group.is_some())
            .and_then(|entry| entry.group_ctl_tx.clone());
        tx.is_some_and(|tx| tx.try_send(GroupControl::Reaction(emoji)))
    }

    /// Queue one validated reaction only while `generation` owns an active group call.
    pub fn send_group_reaction_if_current(
        &self,
        call_id: &str,
        generation: u64,
        emoji: String,
    ) -> bool {
        if crate::voip::app_data::encode_reaction(1, &emoji).is_err() {
            return false;
        }
        let tx = self
            .active_calls()
            .get(call_id)
            .filter(|entry| entry.generation == generation && entry.group.is_some())
            .and_then(|entry| entry.group_ctl_tx.clone());
        tx.is_some_and(|tx| tx.try_send(GroupControl::Reaction(emoji)))
    }

    /// Replace the one-shot endpoint teardown for the current call generation.
    pub fn set_video_teardown(
        &self,
        call_id: &str,
        generation: u64,
        video_teardown: Box<dyn Fn() + Send + Sync>,
    ) -> bool {
        let mut map = self.active_calls();
        let Some(entry) = map
            .get_mut(call_id)
            .filter(|entry| entry.generation == generation)
        else {
            return false;
        };
        entry.video_teardown = Some(video_teardown);
        true
    }

    /// Release the local video endpoints once for the current call generation.
    pub fn run_video_teardown(&self, call_id: &str, generation: u64) -> bool {
        let hook = self
            .active_calls()
            .get_mut(call_id)
            .filter(|entry| entry.generation == generation)
            .and_then(|entry| entry.video_teardown.take());
        if let Some(hook) = hook {
            hook();
            true
        } else {
            false
        }
    }

    /// Serialize an actionable signaling event across its typed ack. The permit force-inserts the
    /// committed transition, so queue pressure cannot hide peer-visible state from the consumer.
    pub fn reserve_call_event(&self, call_id: &str) -> Option<CallEventPermit> {
        let (tx, reserved, generation) = {
            let map = self.active_calls();
            let entry = map.get(call_id)?;
            (
                entry.event_tx.clone()?,
                entry.event_publication_reserved.clone(),
                entry.generation,
            )
        };
        if tx.tx.is_closed()
            || reserved
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
        {
            return None;
        }
        Some(CallEventPermit {
            tx,
            reserved,
            generation,
        })
    }

    /// The current call generation and its shared video-transition lock.
    pub fn current_video_transition(&self, call_id: &str) -> Option<(u64, Arc<AsyncMutex<()>>)> {
        self.active_calls()
            .get(call_id)
            .map(|entry| (entry.generation, entry.video_transition_lock.clone()))
    }

    /// The current call generation and its shared group-transition lock.
    pub fn current_group_transition(&self, call_id: &str) -> Option<(u64, Arc<AsyncMutex<()>>)> {
        self.active_calls()
            .get(call_id)
            .map(|entry| (entry.generation, entry.group_transition_lock.clone()))
    }

    /// The group-transition lock for one known call generation.
    pub fn group_transition_lock(
        &self,
        call_id: &str,
        generation: u64,
    ) -> Option<Arc<AsyncMutex<()>>> {
        self.active_calls()
            .get(call_id)
            .filter(|entry| entry.generation == generation)
            .map(|entry| entry.group_transition_lock.clone())
    }

    /// The video-transition lock for one known call generation.
    pub fn video_transition_lock(
        &self,
        call_id: &str,
        generation: u64,
    ) -> Option<Arc<AsyncMutex<()>>> {
        self.active_calls()
            .get(call_id)
            .filter(|entry| entry.generation == generation)
            .map(|entry| entry.video_transition_lock.clone())
    }

    /// Apply a peer state while the caller holds this generation's video-transition lock.
    pub fn apply_peer_video_state(
        &self,
        call_id: &str,
        generation: u64,
        state: VideoState,
    ) -> PeerVideoTransition {
        let mut map = self.active_calls();
        let Some(entry) = map
            .get_mut(call_id)
            .filter(|entry| entry.generation == generation)
        else {
            return PeerVideoTransition::Ignored;
        };

        let outcome = match state {
            VideoState::UpgradeRequest | VideoState::UpgradeRequestV2 => {
                if entry.video.peer_state.is_upgrade_request()
                    && let Some(epoch) = entry.video.pending_peer_request
                {
                    PeerVideoTransition::UpgradeRequested(VideoUpgradeToken { generation, epoch })
                } else if entry.video.self_state.is_upgrade_request()
                    && entry.video.pending_self_request.is_some()
                {
                    entry.video.self_state = VideoState::Enabled;
                    entry.video.peer_state = VideoState::Enabled;
                    entry.video.pending_self_request = None;
                    entry.video.pending_peer_request = None;
                    PeerVideoTransition::Applied {
                        enable_plane: true,
                        teardown_local: false,
                        answer_upgrade: true,
                    }
                } else if !entry.video.self_state.is_inactive_for_call_mode() {
                    PeerVideoTransition::Ignored
                } else {
                    entry.video.peer_state = state;
                    let token = entry.video.next_peer_request(generation);
                    PeerVideoTransition::UpgradeRequested(token)
                }
            }
            VideoState::UpgradeAccept => {
                if entry.video.self_state.is_upgrade_request()
                    && entry.video.pending_self_request.is_some()
                {
                    entry.video.self_state = VideoState::Enabled;
                    entry.video.peer_state = VideoState::Enabled;
                    entry.video.pending_self_request = None;
                    PeerVideoTransition::Applied {
                        enable_plane: true,
                        teardown_local: false,
                        answer_upgrade: false,
                    }
                } else {
                    PeerVideoTransition::Ignored
                }
            }
            VideoState::Enabled => {
                if entry.video.self_state.is_upgrade_request() {
                    entry.video.self_state = VideoState::Enabled;
                    entry.video.pending_self_request = None;
                }
                entry.video.peer_state = VideoState::Enabled;
                entry.video.pending_peer_request = None;
                PeerVideoTransition::Applied {
                    enable_plane: true,
                    teardown_local: false,
                    answer_upgrade: false,
                }
            }
            VideoState::UpgradeReject | VideoState::UpgradeRejectByTimeout => {
                if entry.video.self_state.is_upgrade_request()
                    && entry.video.pending_self_request.is_some()
                {
                    entry.video.self_state = VideoState::Disabled;
                    entry.video.peer_state = state;
                    entry.video.pending_self_request = None;
                    PeerVideoTransition::Applied {
                        enable_plane: false,
                        teardown_local: true,
                        answer_upgrade: false,
                    }
                } else {
                    PeerVideoTransition::Ignored
                }
            }
            VideoState::Disabled
            | VideoState::UpgradeCancel
            | VideoState::UpgradeCancelByTimeout
            | VideoState::Error => {
                entry.video.self_state = VideoState::Disabled;
                entry.video.peer_state = state;
                entry.video.pending_self_request = None;
                entry.video.pending_peer_request = None;
                PeerVideoTransition::Applied {
                    enable_plane: false,
                    teardown_local: true,
                    answer_upgrade: false,
                }
            }
            VideoState::Stopped => {
                entry.video.peer_state = VideoState::Stopped;
                entry.video.pending_peer_request = None;
                PeerVideoTransition::Applied {
                    enable_plane: false,
                    teardown_local: false,
                    answer_upgrade: false,
                }
            }
            VideoState::Paused | VideoState::UnknownPeer => {
                entry.video.peer_state = state;
                PeerVideoTransition::Applied {
                    enable_plane: false,
                    teardown_local: false,
                    answer_upgrade: false,
                }
            }
            VideoState::Unknown(_) => PeerVideoTransition::Ignored,
        };
        entry.session.is_video = entry.video.is_video();
        outcome
    }

    /// Begin a local upgrade and return its timeout epoch.
    pub fn begin_local_video_request(&self, call_id: &str, generation: u64) -> Option<u64> {
        let mut map = self.active_calls();
        let entry = map
            .get_mut(call_id)
            .filter(|entry| entry.generation == generation)?;
        if !entry.video.self_state.is_inactive_for_call_mode()
            || entry.video.pending_peer_request.is_some()
        {
            return None;
        }
        entry.video.self_state = VideoState::UpgradeRequestV2;
        let epoch = entry.video.next_self_request();
        entry.session.is_video = entry.video.is_video();
        Some(epoch)
    }

    /// Complete a peer request only when the same request is still pending.
    pub fn complete_peer_video_request(&self, call_id: &str, token: VideoUpgradeToken) -> bool {
        let mut map = self.active_calls();
        let Some(entry) = map
            .get_mut(call_id)
            .filter(|entry| entry.generation == token.generation)
        else {
            return false;
        };
        if entry.video.pending_peer_request != Some(token.epoch)
            || !entry.video.peer_state.is_upgrade_request()
        {
            return false;
        }
        entry.video.self_state = VideoState::Enabled;
        entry.video.peer_state = VideoState::Enabled;
        entry.video.pending_peer_request = None;
        entry.session.is_video = entry.video.is_video();
        true
    }

    pub fn peer_video_request_is_current(&self, call_id: &str, token: VideoUpgradeToken) -> bool {
        self.active_calls()
            .get(call_id)
            .filter(|entry| entry.generation == token.generation)
            .is_some_and(|entry| {
                entry.video.pending_peer_request == Some(token.epoch)
                    && entry.video.peer_state.is_upgrade_request()
            })
    }

    /// Roll back or expire one local request without touching a newer request.
    pub fn end_local_video_request(&self, call_id: &str, generation: u64, epoch: u64) -> bool {
        let mut map = self.active_calls();
        let Some(entry) = map
            .get_mut(call_id)
            .filter(|entry| entry.generation == generation)
        else {
            return false;
        };
        if entry.video.pending_self_request != Some(epoch)
            || !entry.video.self_state.is_upgrade_request()
        {
            return false;
        }
        entry.video.self_state = VideoState::Disabled;
        entry.video.peer_state = VideoState::Disabled;
        entry.video.pending_self_request = None;
        entry.video.pending_peer_request = None;
        entry.session.is_video = entry.video.is_video();
        true
    }

    /// Clear both directions after a failed handshake or full downgrade.
    pub fn reset_video(&self, call_id: &str, generation: u64) -> bool {
        let mut map = self.active_calls();
        let Some(entry) = map
            .get_mut(call_id)
            .filter(|entry| entry.generation == generation)
        else {
            return false;
        };
        entry.video.self_state = VideoState::Disabled;
        entry.video.peer_state = VideoState::Disabled;
        entry.video.pending_self_request = None;
        entry.video.pending_peer_request = None;
        entry.session.is_video = false;
        true
    }

    /// Mark only our direction stopped; the peer may keep sending video.
    pub fn stop_local_video(&self, call_id: &str, generation: u64) -> bool {
        let mut map = self.active_calls();
        let Some(entry) = map
            .get_mut(call_id)
            .filter(|entry| entry.generation == generation)
        else {
            return false;
        };
        entry.video.self_state = VideoState::Stopped;
        entry.video.pending_self_request = None;
        entry.session.is_video = entry.video.is_video();
        true
    }

    pub fn video_states(&self, call_id: &str, generation: u64) -> Option<(VideoState, VideoState)> {
        self.active_calls()
            .get(call_id)
            .filter(|entry| entry.generation == generation)
            .map(|entry| (entry.video.self_state, entry.video.peer_state))
    }

    /// The rotation this side announces on `<video>`, or `None` when the call is
    /// gone. Always in `0..=3`.
    pub fn local_video_orientation(&self, call_id: &str, generation: u64) -> Option<u8> {
        self.active_calls()
            .get(call_id)
            .filter(|entry| entry.generation == generation)
            .map(|entry| entry.self_video_orientation)
    }

    /// Record the rotation to announce, rejecting a value the wire has no room
    /// for rather than folding it into range: `4` is a caller that counted
    /// something other than quarter turns, and answering it with `0` would turn
    /// that into an upright picture that stays wrong for the rest of the call.
    ///
    /// `false` when the value is out of range or the call is gone — the two the
    /// caller has to tell apart anyway, and it knows which it asked about.
    pub fn set_local_video_orientation(
        &self,
        call_id: &str,
        generation: u64,
        orientation: u8,
    ) -> bool {
        if orientation > 3 {
            return false;
        }
        self.active_calls()
            .get_mut(call_id)
            .filter(|entry| entry.generation == generation)
            .map(|entry| entry.self_video_orientation = orientation)
            .is_some()
    }

    /// Send a mid-call video-plane command to the current drive loop.
    pub fn send_video_ctl(&self, call_id: &str, generation: u64, ctl: VideoControl) {
        let tx = self
            .active_calls()
            .get(call_id)
            .filter(|entry| entry.generation == generation)
            .and_then(|e| e.video_ctl_tx.clone());
        if let Some(tx) = tx {
            let _ = tx.send(ctl);
        }
    }

    /// Track whether the call currently has video negotiated (offer `<video>`, upgrade accepted, or
    /// downgraded back). No-op for an unknown call.
    pub fn set_is_video(&self, call_id: &str, generation: u64, is_video: bool) -> bool {
        if let Some(entry) = self.active_calls().get_mut(call_id)
            && entry.generation == generation
        {
            entry.session.is_video = is_video;
            entry.video = VideoNegotiation::new(is_video);
            true
        } else {
            false
        }
    }

    pub fn is_current(&self, call_id: &str, generation: u64) -> bool {
        self.active_calls()
            .get(call_id)
            .is_some_and(|entry| entry.generation == generation)
    }

    /// Caller side: rekey recv to the device that answered. One-shot — the sender is TAKEN, so a
    /// duplicate/late `<accept>` from another device is a no-op (first answerer wins, matching WA Web).
    /// Silently ignored when absent (no engine yet, an incoming call, or the call is torn down).
    pub fn send_rekey(&self, call_id: &str, answering_lid: String) {
        let tx = self
            .active_calls()
            .get_mut(call_id)
            .and_then(|e| e.rekey_tx.take());
        if let Some(tx) = tx {
            let _ = tx.try_send(answering_lid);
        }
    }

    /// Caller side: record the callee device that answered (the inbound `<accept>`'s `call.from`), so a
    /// later `<terminate>` can target it instead of the bare peer the offer rang. Set-once -- the first
    /// answerer wins, matching the rekey and WA Web's accepted-elsewhere handling. No-op if the call is
    /// unknown or a device was already recorded.
    pub fn set_answering_device(&self, call_id: &str, device: Jid) {
        if let Some(entry) = self.active_calls().get_mut(call_id)
            && entry.session.answering_device.is_none()
        {
            entry.session.answering_device = Some(device);
        }
    }

    /// The callee device that answered the call registered under `generation`, if one has, for
    /// addressing a `<terminate>`. Generation-guarded so a stale handle (superseded by a same-call-id
    /// replacement) can't read the newer call's device; it falls back to its own bare peer instead.
    pub fn answering_device_if_current(&self, call_id: &str, generation: u64) -> Option<Jid> {
        self.active_calls()
            .get(call_id)
            .filter(|e| e.generation == generation)
            .and_then(|e| e.session.answering_device.clone())
    }

    /// The current generation token registered under `call_id`, or `None` if unknown. Lets a caller
    /// confirm its registration still owns the call (not superseded/removed) before attaching to it.
    pub fn generation_of(&self, call_id: &str) -> Option<u64> {
        self.active_calls().get(call_id).map(|e| e.generation)
    }

    pub fn phase(&self, call_id: &str) -> Option<CallPhase> {
        self.active_calls().get(call_id).map(|e| e.session.phase())
    }

    /// Read phase only while `generation` still owns this call-id.
    pub fn phase_if_current(&self, call_id: &str, generation: u64) -> Option<CallPhase> {
        self.active_calls()
            .get(call_id)
            .filter(|entry| entry.generation == generation)
            .map(|entry| entry.session.phase())
    }

    /// Advance a call's phase; returns false if the call is unknown or the transition is illegal.
    pub fn transition(&self, call_id: &str, next: CallPhase) -> bool {
        self.active_calls()
            .get_mut(call_id)
            .is_some_and(|e| e.session.transition_to(next))
    }

    /// Advance a call phase only while `generation` still owns this call-id.
    pub fn transition_if_current(&self, call_id: &str, generation: u64, next: CallPhase) -> bool {
        self.active_calls()
            .get_mut(call_id)
            .filter(|entry| entry.generation == generation)
            .is_some_and(|entry| entry.session.transition_to(next))
    }

    /// Read a clone of a call's session snapshot.
    pub fn snapshot(&self, call_id: &str) -> Option<CallSession> {
        self.active_calls().get(call_id).map(|e| e.session.clone())
    }

    /// Read a call session only when `generation` still owns this call-id.
    pub fn snapshot_if_current(&self, call_id: &str, generation: u64) -> Option<CallSession> {
        self.active_calls()
            .get(call_id)
            .filter(|entry| entry.generation == generation)
            .map(|entry| entry.session.clone())
    }

    /// Take an outgoing call's sibling-dismiss targets: `(call_creator, rung_device_jids)`, leaving
    /// the session's `ring_devices` empty so a duplicate accept/reject can't re-dismiss. Returns
    /// `None` when the call is unknown or has no devices to dismiss (already taken, single-device, or
    /// incoming). The device list is consumed here, but the entry stays -- an accepted call is live.
    /// Each device JID already names its user, so the bare peer is not returned (the dismiss
    /// terminate is addressed per device JID, not to the bare peer).
    pub fn take_dismiss_targets(&self, call_id: &str) -> Option<(Jid, Vec<Jid>)> {
        let mut map = self.active_calls();
        let entry = map.get_mut(call_id)?;
        if entry.session.ring_devices.is_empty() {
            return None;
        }
        let devices = std::mem::take(&mut entry.session.ring_devices);
        Some((entry.session.call_creator.clone(), devices))
    }

    pub fn active_count(&self) -> usize {
        self.active_calls().len()
    }

    /// Record an incoming offer as ringing (not yet answered) so a later `<terminate>` for it can be
    /// surfaced as a missed call. Idempotent; the flag is consumed by [`take_ringing`](Self::take_ringing)
    /// on answer or terminate. Do not call for an offline-queued offer: that one is already surfaced
    /// as missed-offline and must not double-fire.
    pub fn mark_incoming_ringing(&self, call_id: &str) {
        self.ringing_calls().insert(call_id.to_string());
    }

    /// Consume the ringing flag for `call_id`, returning whether it was still ringing. True means a
    /// genuine missed call (an unanswered incoming offer the peer gave up on); false means the call
    /// was answered, was outgoing, or was already resolved (so a duplicate `<terminate>` is ended,
    /// never a second missed). One-shot.
    pub fn take_ringing(&self, call_id: &str) -> bool {
        self.ringing_calls().remove(call_id)
    }

    /// Remove a call, aborting its media task. Returns true if it existed.
    pub fn remove(&self, call_id: &str) -> bool {
        let removed = self.active_calls().remove(call_id);
        // `removed` drops here, after the lock guard: the media-task abort and on_terminal hook run
        // off-lock.
        removed.is_some()
    }

    /// Remove a call only if it is still on `generation` -- the safe self-cleanup for a finishing
    /// media task. A newer same-call-id replacement (different generation) is left untouched, so a
    /// task that ended after being superseded can't reap the live replacement. Returns true if this
    /// generation was the current entry and was removed.
    pub fn remove_if_current(&self, call_id: &str, generation: u64) -> bool {
        self.remove_if_current_with_phase(call_id, generation)
            .is_some()
    }

    /// Remove exactly one call generation and return the phase it had at the atomic claim.
    ///
    /// Call-link setup uses the returned phase to distinguish local waiting-room cancellation from
    /// cancellation after server-side admission, which additionally requires a terminal stanza.
    pub fn remove_if_current_with_phase(
        &self,
        call_id: &str,
        generation: u64,
    ) -> Option<CallPhase> {
        let removed = {
            let mut map = self.active_calls();
            if map.get(call_id).is_some_and(|e| e.generation == generation) {
                map.remove(call_id)
            } else {
                None
            }
        };
        // `removed` drops here, off-lock: the media-task abort and on_terminal hook run without the
        // registry mutex held.
        removed.map(|entry| entry.session.phase())
    }

    /// Abort every call's media task and clear the registry. Returns the number cleared.
    /// Call this from your own disconnect/reconnect teardown; it is not wired into the client.
    pub fn abort_all(&self) -> usize {
        self.ringing_calls().clear();
        self.pending_controls().clear();
        let drained: Vec<CallEntry> = {
            let mut map = self.active_calls();
            map.drain().map(|(_, entry)| entry).collect()
        };
        let n = drained.len();
        // `drained` drops here, off-lock: every entry aborts its media task and fires on_terminal.
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::group_call::{
        CallLinkMedia, GroupCallDevice, GroupCallEncRekey, GroupCallParticipant, GroupCallRelay,
        GroupCallRelayEndpoint, GroupCallUpdate, ScreenShareState, WaitingRoom,
    };
    use crate::voip::driver::video_control_channel;
    use futures::FutureExt;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use wacore_binary::{Jid, Server};

    fn session(id: &str) -> CallSession {
        CallSession::new_outgoing(
            id,
            Jid::new("222222222222222", Server::Lid),
            Jid::new("111111111111111", Server::Lid),
        )
    }

    fn group_update(transaction_id: u32) -> GroupCallUpdate {
        GroupCallUpdate {
            call_id: "GROUP-CALL".to_string(),
            call_creator: Jid::new("111111111111111", Server::Lid),
            group_jid: None,
            transaction_id,
            media: "audio".to_string(),
            connected_limit: 32,
            joinable: true,
            av_upgradable: true,
            rekey_requested: false,
            participants: Vec::new(),
            relay: None,
        }
    }

    fn initial_group_update(call_id: &str, stanza_id: &str, sender: Jid) -> IncomingCall {
        let mut update = group_update(1);
        update.call_id = call_id.to_string();
        IncomingCall::new_for_test(
            sender,
            stanza_id.to_string(),
            crate::time::from_secs(1).expect("valid timestamp"),
            CallAction::GroupUpdate {
                update: Box::new(update),
            },
        )
    }

    fn initial_group_rekey(
        call_id: &str,
        stanza_id: &str,
        sender: Jid,
        ciphertext_len: usize,
    ) -> IncomingCall {
        IncomingCall::new_for_test(
            sender,
            stanza_id.to_string(),
            crate::time::from_secs(1).expect("valid timestamp"),
            CallAction::EncRekey {
                rekey: Box::new(
                    GroupCallEncRekey::builder()
                        .call_id(call_id.to_string())
                        .call_creator(Jid::new("111111111111111", Server::Lid))
                        .transaction_id(1)
                        .key_generation(1)
                        .encryption_type("msg".to_string())
                        .encryption_version(2)
                        .ciphertext(vec![7; ciphertext_len])
                        .build(),
                ),
            },
        )
    }

    #[test]
    fn initial_group_control_buffer_is_authenticated_bounded_and_drained_by_identity() {
        let reg = CallRegistry::new();
        let creator = Jid::new("111111111111111", Server::Lid);
        assert!(!reg.buffer_initial_group_control(initial_group_update(
            "UNAUTHORIZED",
            "UNAUTHORIZED-STANZA",
            Jid::new("999999999999999", Server::Lid),
        )));

        for index in 0..MAX_PENDING_INITIAL_GROUP_CONTROLS {
            assert!(reg.buffer_initial_group_control(initial_group_update(
                &format!("GROUP-{index}"),
                &format!("STANZA-{index}"),
                creator.clone().with_device(1),
            )));
        }
        assert_eq!(
            reg.memory_stats().entries,
            MAX_PENDING_INITIAL_GROUP_CONTROLS as u64,
            "fabricated call IDs must not grow the pending-control buffer without bound"
        );
        assert!(
            !reg.buffer_initial_group_control(initial_group_update(
                "OVER-CAPACITY",
                "OVER-CAPACITY-STANZA",
                creator.clone().with_device(1),
            )),
            "a new prospective identity cannot evict an already-retained call"
        );
        assert!(reg.buffer_initial_group_control(initial_group_update(
            "GROUP-0",
            "REPLACEMENT-STANZA",
            creator.clone().with_device(1),
        )));
        let latest = reg.take_initial_group_controls("GROUP-0", &creator);
        assert_eq!(latest.len(), 1, "only the same identity may evict itself");
        assert_eq!(
            latest[0].stanza_id, "REPLACEMENT-STANZA",
            "same-identity replacement should retain the newest control"
        );
        reg.abort_all();
        assert_eq!(reg.memory_stats().entries, 0);
    }

    #[test]
    fn initial_group_control_buffer_preserves_update_and_epoch_order() {
        let reg = CallRegistry::new();
        let creator = Jid::new("111111111111111", Server::Lid);
        assert!(reg.buffer_initial_group_control(initial_group_update(
            "GROUP-CALL",
            "UPDATE",
            creator.clone().with_device(1),
        )));
        assert!(
            reg.buffer_initial_group_control(IncomingCall::new_for_test(
                creator.clone().with_device(1),
                "EPOCH".to_string(),
                crate::time::from_secs(2).expect("valid timestamp"),
                CallAction::EncRekey {
                    rekey: Box::new(
                        GroupCallEncRekey::builder()
                            .call_id("GROUP-CALL".to_string())
                            .call_creator(creator.clone())
                            .transaction_id(2)
                            .key_generation(1)
                            .encryption_type("msg".to_string())
                            .encryption_version(2)
                            .ciphertext(vec![7; 32])
                            .build(),
                    ),
                },
            ))
        );

        let drained = reg.take_initial_group_controls("GROUP-CALL", &creator);
        assert_eq!(drained.len(), 2);
        assert!(matches!(drained[0].action, CallAction::GroupUpdate { .. }));
        assert!(matches!(drained[1].action, CallAction::EncRekey { .. }));
    }

    #[test]
    fn initial_group_control_buffer_is_bounded_by_retained_bytes() {
        let reg = CallRegistry::new();
        let creator = Jid::new("111111111111111", Server::Lid);
        let chunk = MAX_PENDING_INITIAL_GROUP_CONTROL_BYTES / 3;
        let mut accepted = 0;
        for index in 0..4 {
            accepted += usize::from(reg.buffer_initial_group_control(initial_group_rekey(
                &format!("GROUP-{index}"),
                &format!("EPOCH-{index}"),
                creator.clone().with_device(1),
                chunk,
            )));
        }
        let stats = reg.memory_stats();
        assert!(accepted < 4, "the byte cap must reject excess identities");
        assert!(
            stats.bytes <= MAX_PENDING_INITIAL_GROUP_CONTROL_BYTES as u64,
            "retained bootstrap controls must stay within the aggregate byte budget"
        );
        assert!(
            !reg.take_initial_group_controls("GROUP-0", &creator)
                .is_empty(),
            "byte pressure from unrelated identities must preserve the first control"
        );

        assert!(
            !reg.buffer_initial_group_control(initial_group_rekey(
                "OVERSIZED",
                "OVERSIZED-EPOCH",
                creator.with_device(1),
                MAX_PENDING_INITIAL_GROUP_CONTROL_BYTES,
            )),
            "one oversized control cannot consume the entire bootstrap budget"
        );
    }

    #[test]
    fn unrelated_pre_offer_controls_cannot_evict_a_retained_epoch() {
        let reg = CallRegistry::new();
        let creator = Jid::new("111111111111111", Server::Lid);
        assert!(reg.buffer_initial_group_control(initial_group_rekey(
            "PROTECTED-GROUP",
            "PROTECTED-EPOCH",
            creator.clone().with_device(1),
            32,
        )));
        for index in 1..MAX_PENDING_INITIAL_GROUP_CONTROLS {
            assert!(reg.buffer_initial_group_control(initial_group_update(
                &format!("UNRELATED-GROUP-{index}"),
                &format!("UNRELATED-STANZA-{index}"),
                creator.clone().with_device(1),
            )));
        }
        assert!(
            !reg.buffer_initial_group_control(initial_group_update(
                "OVERFLOW-GROUP",
                "OVERFLOW-STANZA",
                creator.clone().with_device(1),
            )),
            "a new identity must be dropped once the global bound is occupied"
        );

        let retained = reg.take_initial_group_controls("PROTECTED-GROUP", &creator);
        assert_eq!(retained.len(), 1);
        assert!(matches!(retained[0].action, CallAction::EncRekey { .. }));
    }

    #[test]
    fn expired_pre_offer_controls_release_capacity_for_new_identity() {
        let reg = CallRegistry::new();
        let creator = Jid::new("111111111111111", Server::Lid);
        for index in 0..MAX_PENDING_INITIAL_GROUP_CONTROLS {
            assert!(reg.buffer_initial_group_control(initial_group_update(
                &format!("STALE-GROUP-{index}"),
                &format!("STALE-STANZA-{index}"),
                creator.clone().with_device(1),
            )));
        }
        reg.pending_controls()
            .front_mut()
            .expect("one retained control")
            .expires_at = crate::time::Instant::ZERO;

        assert!(reg.buffer_initial_group_control(initial_group_update(
            "LEGITIMATE-GROUP",
            "LEGITIMATE-STANZA",
            creator.clone().with_device(1),
        )));
        assert!(
            reg.take_initial_group_controls("STALE-GROUP-0", &creator)
                .is_empty(),
            "expired reservations must be removed before capacity is evaluated"
        );
        assert_eq!(
            reg.take_initial_group_controls("LEGITIMATE-GROUP", &creator)
                .len(),
            1
        );
    }

    fn group_relay(transaction_id: u32) -> GroupCallRelay {
        GroupCallRelay::builder()
            .transaction_id(transaction_id)
            .self_pid(1)
            .uuid("TEST-RELAY".to_string())
            .participant_uuid("TEST-PARTICIPANT".to_string())
            .attribute_padding(false)
            .warp_mi_tag_len(4)
            .key(vec![7; 32])
            .tokens(vec![vec![9; 16]])
            .endpoints(vec![GroupCallRelayEndpoint {
                relay_id: 1,
                token_id: 0,
                auth_token_id: 0,
                relay_name: "test-relay".to_string(),
                domain_name: None,
                rtt_ms: None,
                is_fna: false,
                address: Vec::new(),
                ipv4: Some("203.0.113.7".to_string()),
                port: Some(3478),
            }])
            .build()
    }

    #[test]
    fn duplicate_group_offer_cannot_replace_an_active_generation() {
        let reg = CallRegistry::new();
        let incoming = || {
            let mut session = CallSession::new_incoming(
                "GROUP-CALL",
                Jid::new("111111111111111", Server::Lid),
                Jid::new("111111111111111", Server::Lid),
            );
            session.group = Some(group_update(1));
            session
        };
        let generation = reg
            .insert_ringing_group_if_inactive(incoming())
            .expect("valid group snapshot")
            .expect("first offer registers");
        assert!(reg.transition("GROUP-CALL", CallPhase::Connecting));
        assert!(reg.take_ringing("GROUP-CALL"));

        assert_eq!(reg.insert_ringing_group_if_inactive(incoming()), Ok(None));
        assert_eq!(reg.generation_of("GROUP-CALL"), Some(generation));
        assert!(
            !reg.take_ringing("GROUP-CALL"),
            "the duplicate must not mark an active call as ringing"
        );
    }

    #[test]
    fn ringing_group_offer_cannot_change_the_call_creator() {
        let reg = CallRegistry::new();
        let incoming = |creator: Jid| {
            let mut session =
                CallSession::new_incoming("GROUP-CALL", creator.clone(), creator.clone());
            let mut update = group_update(1);
            update.call_creator = creator;
            session.group = Some(update);
            session
        };
        let original_creator = Jid::new("111111111111111", Server::Lid);
        let replacement_creator = Jid::new("222222222222222", Server::Lid);
        let generation = reg
            .insert_ringing_group_if_inactive(incoming(original_creator.clone()))
            .expect("valid initial offer")
            .expect("first offer registers");

        assert_eq!(
            reg.insert_ringing_group_if_inactive(incoming(replacement_creator)),
            Err(GroupStateApply::IdentityMismatch)
        );
        assert_eq!(reg.generation_of("GROUP-CALL"), Some(generation));
        assert_eq!(
            reg.snapshot("GROUP-CALL")
                .map(|session| session.call_creator),
            Some(original_creator),
            "a conflicting redelivery must leave the original signaling/media identity intact"
        );
    }

    #[test]
    fn ringing_group_redelivery_rejects_conflicting_group_jid() {
        let reg = CallRegistry::new();
        let incoming = |transaction_id, group_jid: &str| {
            let creator = Jid::new("111111111111111", Server::Lid);
            let mut session = CallSession::new_incoming("GROUP-CALL", creator.clone(), creator);
            let mut update = group_update(transaction_id);
            update.group_jid = Some(Jid::new(group_jid, Server::Group));
            session.group = Some(update);
            session
        };
        let generation = reg
            .insert_ringing_group_if_inactive(incoming(1, "120363000000000001"))
            .expect("valid initial offer")
            .expect("first offer registers");

        assert_eq!(
            reg.insert_ringing_group_if_inactive(incoming(2, "120363000000000002")),
            Err(GroupStateApply::IdentityMismatch)
        );
        assert_eq!(reg.generation_of("GROUP-CALL"), Some(generation));
        assert_eq!(
            reg.group_state("GROUP-CALL")
                .and_then(|state| state.snapshot().and_then(|update| update.group_jid.clone())),
            Some(Jid::new("120363000000000001", Server::Group)),
            "a conflicting redelivery must leave the original group identity intact"
        );
    }

    #[test]
    fn ringing_group_registrations_are_bounded_by_count_and_bytes() {
        let incoming = |call_id: &str| {
            let creator = Jid::new("111111111111111", Server::Lid);
            let mut session = CallSession::new_incoming(call_id, creator.clone(), creator.clone());
            let mut update = group_update(1);
            update.call_id = call_id.to_string();
            update.call_creator = creator;
            session.group = Some(update);
            session
        };
        let reg = CallRegistry::new();
        let first_generation = (0..MAX_RINGING_GROUP_CALLS).fold(None, |first, index| {
            let call_id = format!("GROUP-CALL-{index}");
            let generation = reg
                .insert_ringing_group_if_inactive(incoming(&call_id))
                .expect("bounded offer")
                .expect("offer registers");
            first.or(Some(generation))
        });
        assert_eq!(
            reg.insert_ringing_group_if_inactive(incoming("GROUP-CALL-OVERFLOW")),
            Err(GroupStateApply::InvalidSnapshot)
        );
        assert_eq!(reg.active_count(), MAX_RINGING_GROUP_CALLS);

        assert!(reg.take_ringing("GROUP-CALL-0"));
        assert!(reg.remove_if_current("GROUP-CALL-0", first_generation.expect("first generation")));
        assert!(
            reg.insert_ringing_group_if_inactive(incoming("GROUP-CALL-RECOVERED"))
                .expect("recovered capacity")
                .is_some()
        );

        let oversized = CallRegistry::new();
        let mut session = incoming("GROUP-CALL-OVERSIZED");
        let mut relay = group_relay(1);
        relay.tokens = vec![vec![9; MAX_RINGING_GROUP_CALL_BYTES]];
        session.group.as_mut().expect("group snapshot").relay = Some(relay);
        assert_eq!(
            oversized.insert_ringing_group_if_inactive(session),
            Err(GroupStateApply::InvalidSnapshot)
        );
        assert_eq!(oversized.active_count(), 0);
        assert!(!oversized.take_ringing("GROUP-CALL-OVERSIZED"));

        let growing = CallRegistry::new();
        let generation = growing
            .insert_ringing_group_if_inactive(incoming("GROUP-CALL-GROWING"))
            .expect("valid initial offer")
            .expect("offer registers");
        let mut redelivery = incoming("GROUP-CALL-GROWING");
        redelivery
            .group
            .as_mut()
            .expect("group snapshot")
            .transaction_id = 2;
        let mut relay = group_relay(2);
        relay.tokens = vec![vec![9; MAX_RINGING_GROUP_CALL_BYTES]];
        redelivery.group.as_mut().expect("group snapshot").relay = Some(relay);
        assert_eq!(
            growing.insert_ringing_group_if_inactive(redelivery),
            Err(GroupStateApply::InvalidSnapshot)
        );
        assert_eq!(
            growing
                .group_state("GROUP-CALL-GROWING")
                .and_then(|state| state.snapshot().map(|update| update.transaction_id)),
            Some(1),
            "an oversized redelivery must not consume the authoritative transaction"
        );
        let mut corrected = incoming("GROUP-CALL-GROWING");
        corrected
            .group
            .as_mut()
            .expect("group snapshot")
            .transaction_id = 2;
        assert_eq!(
            growing.insert_ringing_group_if_inactive(corrected),
            Ok(Some(generation)),
            "a corrected same-transaction redelivery must remain admissible"
        );

        let generic = CallRegistry::new();
        let generic_generation = generic
            .insert_ringing_group_if_inactive(incoming("GROUP-CALL-GENERIC"))
            .expect("valid initial offer")
            .expect("offer registers");
        let mut unmarked = incoming("GROUP-CALL-UNMARKED");
        let mut relay = group_relay(1);
        relay.tokens = vec![vec![9; MAX_RINGING_GROUP_CALL_BYTES]];
        unmarked.group.as_mut().expect("group snapshot").relay = Some(relay);
        generic.insert_ringing_group(unmarked);
        let mut oversized_update = incoming("GROUP-CALL-GENERIC")
            .group
            .expect("group snapshot");
        oversized_update.transaction_id = 2;
        let mut relay = group_relay(2);
        relay.tokens = vec![vec![9; MAX_RINGING_GROUP_CALL_BYTES]];
        oversized_update.relay = Some(relay);
        assert_eq!(
            generic.apply_group_update_if_current(oversized_update, generic_generation),
            GroupStateApply::InvalidSnapshot,
            "generic signaling updates must not bypass the ringing aggregate byte budget"
        );
        assert_eq!(
            generic
                .group_state("GROUP-CALL-GENERIC")
                .and_then(|state| state.snapshot().map(|update| update.transaction_id)),
            Some(1)
        );
        let mut corrected = incoming("GROUP-CALL-GENERIC")
            .group
            .expect("group snapshot");
        corrected.transaction_id = 2;
        assert_eq!(
            generic.apply_group_update_if_current(corrected, generic_generation),
            GroupStateApply::Applied,
            "a corrected same-transaction update must ignore group entries not in the ringing set"
        );
    }

    /// The single preview that decides admission is the one that gets committed. Pins the admission
    /// boundary and the accounted bytes on both sides of it, plus the claim-as-group-media side
    /// effect a rejected transaction still carries.
    #[test]
    fn group_update_admission_boundary_and_accounting_are_unchanged_by_the_shared_preview() {
        let creator = Jid::new("111111111111111", Server::Lid);
        let ringing_offer = || {
            let mut session =
                CallSession::new_incoming("GROUP-CALL", creator.clone(), creator.clone());
            session.group = Some(group_update(1));
            session
        };

        let reg = CallRegistry::new();
        let generation = reg
            .insert_ringing_group_if_inactive(ringing_offer())
            .expect("valid initial offer")
            .expect("offer registers");
        let baseline_bytes = reg.memory_stats().bytes;

        let mut oversized = group_update(2);
        let mut relay = group_relay(2);
        relay.tokens = vec![vec![9; MAX_RINGING_GROUP_CALL_BYTES]];
        oversized.relay = Some(relay);
        assert_eq!(
            reg.apply_group_update_if_current(oversized, generation),
            GroupStateApply::InvalidSnapshot
        );
        assert_eq!(
            reg.group_state("GROUP-CALL")
                .and_then(|state| state.snapshot().map(|update| update.transaction_id)),
            Some(1),
            "a rejected transaction must not be consumed"
        );
        assert_eq!(
            reg.memory_stats().bytes,
            baseline_bytes,
            "a rejected preview must not be accounted"
        );

        assert_eq!(
            reg.apply_group_update_if_current(group_update(2), generation),
            GroupStateApply::Applied,
            "a corrected same-transaction update stays admissible"
        );
        assert_eq!(
            reg.group_state("GROUP-CALL")
                .and_then(|state| state.snapshot().map(|update| update.transaction_id)),
            Some(2)
        );
        assert_eq!(
            reg.apply_group_update_if_current(group_update(2), generation),
            GroupStateApply::Stale
        );

        // A non-applying update still claims the call as group media, exactly as routing the update
        // through `group_mut()` did.
        let plain = CallRegistry::new();
        let plain_generation = plain.insert(session("CID"));
        let mut foreign = group_update(1);
        foreign.call_id = "CID".to_string();
        foreign.call_creator = Jid::new("999999999999999", Server::Lid);
        assert_eq!(
            plain.apply_group_update_if_current(foreign, plain_generation),
            GroupStateApply::IdentityMismatch
        );
        assert!(plain.is_group_call("CID"));
        assert!(
            plain
                .group_state("CID")
                .is_some_and(|state| state.snapshot().is_none()),
            "the rejected transaction leaves an empty authoritative state behind"
        );
    }

    #[test]
    fn outgoing_group_identity_exists_before_the_first_roster() {
        let reg = CallRegistry::new();
        let generation = reg.insert_group(session("GROUP-CALL"));

        assert!(reg.is_group_call("GROUP-CALL"));
        assert!(reg.group_state("GROUP-CALL").is_none());
        assert!(reg.remove_if_current("GROUP-CALL", generation));
    }

    #[test]
    fn invalid_initial_group_snapshot_is_not_registered() {
        let reg = CallRegistry::new();
        let mut incoming = CallSession::new_incoming(
            "GROUP-CALL",
            Jid::new("111111111111111", Server::Lid),
            Jid::new("111111111111111", Server::Lid),
        );
        let mut invalid = group_update(1);
        invalid.connected_limit = 0;
        incoming.group = Some(invalid);

        assert_eq!(
            reg.insert_ringing_group_if_inactive(incoming),
            Err(GroupStateApply::InvalidSnapshot)
        );
        assert_eq!(reg.active_count(), 0);
        assert!(!reg.take_ringing("GROUP-CALL"));
    }

    #[test]
    fn redelivered_ringing_group_preserves_roster_epoch_and_generation() {
        let reg = CallRegistry::new();
        let incoming = |transaction_id| {
            let mut session = CallSession::new_incoming(
                "GROUP-CALL",
                Jid::new("111111111111111", Server::Lid),
                Jid::new("111111111111111", Server::Lid),
            );
            session.group = Some(group_update(transaction_id));
            session
        };
        let generation = reg
            .insert_ringing_group_if_inactive(incoming(1))
            .expect("valid group snapshot")
            .expect("first offer registers");
        assert_eq!(
            reg.apply_group_update(group_update(2)),
            GroupStateApply::Applied
        );
        assert!(reg.send_group_epoch_if_current("GROUP-CALL", generation, 2, vec![2; 32],));

        assert_eq!(
            reg.insert_ringing_group_if_inactive(incoming(1)),
            Ok(Some(generation)),
            "redelivery refreshes the ringing entry instead of replacing it"
        );
        assert_eq!(
            reg.group_state("GROUP-CALL")
                .and_then(|state| state.snapshot().map(|update| update.transaction_id)),
            Some(2),
            "the stale redelivery must not roll back the accumulated roster"
        );

        let (tx, rx) = async_channel::bounded(2);
        reg.set_group_control_sender("GROUP-CALL", generation, Some(4), tx);
        assert!(matches!(
            rx.try_recv(),
            Ok(GroupControl::Transition { update, epoch })
                if update.transaction_id == 2 && epoch.transaction_id == 2
        ));
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn accepting_a_stale_ringing_generation_leaves_replacement_untouched() {
        let reg = CallRegistry::new();
        let incoming = || {
            CallSession::new_incoming(
                "GROUP-CALL",
                Jid::new("222222222222222", Server::Lid),
                Jid::new("111111111111111", Server::Lid),
            )
        };
        let stale = reg.insert_ringing_group(incoming());
        let current = reg.insert_ringing_group(incoming());
        reg.mark_incoming_ringing("GROUP-CALL");

        assert!(!reg.accept_ringing_if_current("GROUP-CALL", stale));
        assert_eq!(reg.generation_of("GROUP-CALL"), Some(current));
        assert_eq!(reg.phase("GROUP-CALL"), Some(CallPhase::Ringing));
        assert!(
            reg.ringing
                .lock()
                .expect("registry lock")
                .contains("GROUP-CALL"),
            "a stale accept must not consume the replacement's ringing marker"
        );
    }

    #[test]
    fn rejecting_a_stale_ringing_generation_leaves_replacement_untouched() {
        let reg = CallRegistry::new();
        let incoming = || {
            CallSession::new_incoming(
                "GROUP-CALL",
                Jid::new("222222222222222", Server::Lid),
                Jid::new("111111111111111", Server::Lid),
            )
        };
        let stale = reg.insert_ringing_group(incoming());
        let current = reg.insert_ringing_group(incoming());
        reg.mark_incoming_ringing("GROUP-CALL");

        assert!(!reg.reject_ringing_if_current("GROUP-CALL", stale));
        assert_eq!(reg.generation_of("GROUP-CALL"), Some(current));
        assert_eq!(reg.phase("GROUP-CALL"), Some(CallPhase::Ringing));
        assert!(
            reg.ringing
                .lock()
                .expect("registry lock")
                .contains("GROUP-CALL"),
            "a stale reject must not consume the replacement's ringing marker"
        );
    }

    #[test]
    fn ringing_group_promotion_preserves_newer_roster_and_latest_pending_epoch() {
        let reg = CallRegistry::new();
        reg.mark_incoming_ringing("GROUP-CALL");
        let mut ringing = CallSession::new_incoming(
            "GROUP-CALL",
            Jid::new("111111111111111", Server::Lid),
            Jid::new("111111111111111", Server::Lid),
        );
        ringing.group = Some(group_update(1));
        let generation = reg.insert_ringing_group(ringing);
        assert!(
            reg.ringing
                .lock()
                .expect("registry lock")
                .contains("GROUP-CALL"),
            "registering the enriched offer must not mark it answered"
        );

        assert_eq!(
            reg.apply_group_update(group_update(2)),
            GroupStateApply::Applied
        );
        assert!(reg.send_group_epoch_if_current("GROUP-CALL", generation, 2, vec![2; 32]));
        assert!(reg.send_group_epoch_if_current("GROUP-CALL", generation, 3, vec![3; 32]));
        assert!(reg.send_group_epoch_if_current("GROUP-CALL", generation, 3, vec![9; 32]));
        assert!(reg.send_group_epoch_if_current("GROUP-CALL", generation, 2, vec![9; 32]));
        assert!(reg.transition("GROUP-CALL", CallPhase::Connecting));

        let mut accepted = CallSession::new_incoming(
            "GROUP-CALL",
            Jid::new("111111111111111", Server::Lid),
            Jid::new("111111111111111", Server::Lid),
        );
        accepted.group = Some(group_update(1));
        assert_eq!(reg.promote_ringing_group(accepted), Some(generation));
        assert!(!reg.take_ringing("GROUP-CALL"));
        assert_eq!(
            reg.group_state("GROUP-CALL")
                .and_then(|state| state.snapshot().map(|update| update.transaction_id)),
            Some(2)
        );

        let (tx, rx) = async_channel::unbounded();
        reg.set_group_control_sender("GROUP-CALL", generation, Some(4), tx);
        match rx.try_recv().expect("latest roster and epoch") {
            GroupControl::Transition { update, epoch } => {
                assert_eq!(update.transaction_id, 2);
                assert_eq!(epoch.transaction_id, 3);
                assert_eq!(
                    epoch.as_bytes(),
                    [3; 32],
                    "a duplicate transaction must preserve the first authenticated epoch"
                );
            }
            _ => panic!("expected buffered group transition"),
        }
        assert!(reg.send_group_epoch_if_current("GROUP-CALL", generation, 4, vec![4; 32],));
        match rx
            .try_recv()
            .expect("new epoch routed after buffered epoch")
        {
            GroupControl::Transition { update, epoch } => {
                assert_eq!(update.transaction_id, 2);
                assert_eq!(epoch.transaction_id, 4);
            }
            _ => panic!("expected live group transition"),
        }
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn call_link_admission_transitions_wakes_and_releases_heartbeat() {
        let reg = CallRegistry::new();
        let mut waiting = session("GROUP-CALL");
        assert!(waiting.transition_to(CallPhase::Calling));
        assert!(waiting.transition_to(CallPhase::WaitingRoom));
        let generation = reg
            .insert_call_link_checked(waiting)
            .expect("valid call-link registration");

        let heartbeat_aborted = Arc::new(AtomicBool::new(false));
        reg.set_waiting_room_task("GROUP-CALL", generation, flag_handle(&heartbeat_aborted));
        let stale_aborted = Arc::new(AtomicBool::new(false));
        reg.set_waiting_room_task("GROUP-CALL", generation + 1, flag_handle(&stale_aborted));
        assert!(
            stale_aborted.load(Ordering::SeqCst),
            "a stale generation cannot retain a heartbeat"
        );

        let local_device = Jid::new("222222222222222", Server::Lid).with_device(1);
        let mut without_local = group_update(1);
        let mut creator = GroupCallParticipant::new(
            Jid::new("111111111111111", Server::Lid),
            vec![GroupCallDevice::new(
                Jid::new("111111111111111", Server::Lid).with_device(1),
            )],
        );
        creator.state = Some("connected".to_string());
        without_local.participants = vec![creator];
        assert_eq!(
            reg.apply_group_update_if_current(without_local, generation),
            GroupStateApply::Applied
        );
        assert_eq!(reg.phase("GROUP-CALL"), Some(CallPhase::WaitingRoom));
        assert!(
            !heartbeat_aborted.load(Ordering::SeqCst),
            "a roster that omits the local device cannot admit the call link"
        );
        assert!(reg.set_group_invite_self_device(
            "GROUP-CALL",
            generation,
            GroupCallDevice::new(local_device.clone()).with_capability(1, [1]),
        ));
        assert_eq!(
            reg.phase("GROUP-CALL"),
            Some(CallPhase::WaitingRoom),
            "recording a local device absent from the roster must not admit it"
        );

        let mut wrong_device = group_update(2);
        let mut local_participant = GroupCallParticipant::new(
            local_device.to_non_ad(),
            vec![GroupCallDevice::new(local_device.clone().with_device(2))],
        );
        local_participant.state = Some("connected".to_string());
        wrong_device.participants = vec![local_participant];
        assert_eq!(
            reg.apply_group_update_if_current(wrong_device, generation),
            GroupStateApply::Applied
        );
        assert_eq!(
            reg.phase("GROUP-CALL"),
            Some(CallPhase::WaitingRoom),
            "a sibling device is not proof that this device was admitted"
        );

        let listener = reg
            .listen_group_update("GROUP-CALL", generation)
            .expect("live listener");
        let mut admitted = group_update(3);
        let mut local_participant = GroupCallParticipant::new(
            local_device.to_non_ad(),
            vec![GroupCallDevice::new(local_device)],
        );
        local_participant.state = Some("connected".to_string());
        admitted.participants = vec![local_participant];
        assert_eq!(
            reg.apply_group_update_if_current(admitted, generation),
            GroupStateApply::Applied
        );
        assert_eq!(reg.phase("GROUP-CALL"), Some(CallPhase::Connecting));
        assert!(
            heartbeat_aborted.load(Ordering::SeqCst),
            "admission releases the waiting-room heartbeat"
        );
        assert!(
            listener.now_or_never().is_some(),
            "admission wakes the media attachment waiter"
        );
    }

    #[test]
    fn call_link_admission_accepts_the_local_device_through_its_pn_alias() {
        let reg = CallRegistry::new();
        let mut waiting = session("GROUP-CALL");
        assert!(waiting.transition_to(CallPhase::Calling));
        assert!(waiting.transition_to(CallPhase::WaitingRoom));
        let generation = reg
            .insert_call_link_checked(waiting)
            .expect("valid call-link registration");

        let local_lid = Jid::new("222222222222222", Server::Lid).with_device(1);
        let local_pn = Jid::new("12025550123", Server::Pn).with_device(1);
        assert!(reg.set_group_invite_self_device(
            "GROUP-CALL",
            generation,
            GroupCallDevice::new(local_lid),
        ));

        let mut admitted = group_update(1);
        let mut local_participant = GroupCallParticipant::new(
            Jid::new("222222222222222", Server::Lid),
            vec![GroupCallDevice::new(local_pn)],
        );
        local_participant.pn = Some(Jid::new("12025550123", Server::Pn));
        local_participant.state = Some("connected".to_string());
        admitted.participants = vec![local_participant];

        assert_eq!(
            reg.apply_group_update_if_current(admitted, generation),
            GroupStateApply::Applied
        );
        assert_eq!(
            reg.phase("GROUP-CALL"),
            Some(CallPhase::Connecting),
            "the accepted PN alias represents the exact local LID device"
        );
    }

    #[test]
    fn recording_the_local_device_rechecks_a_retained_admission_roster() {
        let reg = CallRegistry::new();
        let mut waiting = session("GROUP-CALL");
        assert!(waiting.transition_to(CallPhase::Calling));
        assert!(waiting.transition_to(CallPhase::WaitingRoom));
        let generation = reg
            .insert_call_link_checked(waiting)
            .expect("valid call-link registration");
        let local_device = Jid::new("222222222222222", Server::Lid).with_device(1);
        let mut local_participant = GroupCallParticipant::new(
            local_device.to_non_ad(),
            vec![GroupCallDevice::new(local_device.clone())],
        );
        local_participant.state = Some("connected".to_string());
        let mut update = group_update(1);
        update.participants = vec![local_participant];

        assert_eq!(
            reg.apply_group_update_if_current(update, generation),
            GroupStateApply::Applied
        );
        assert_eq!(
            reg.phase("GROUP-CALL"),
            Some(CallPhase::WaitingRoom),
            "the registry cannot trust membership before the local device is known"
        );
        assert!(reg.set_group_invite_self_device(
            "GROUP-CALL",
            generation,
            GroupCallDevice::new(local_device).with_capability(1, [1]),
        ));
        assert_eq!(
            reg.phase("GROUP-CALL"),
            Some(CallPhase::Connecting),
            "recording the exact retained roster member completes admission"
        );
    }

    #[test]
    fn memory_stats_include_active_group_state_and_clear_on_removal() {
        let reg = CallRegistry::new();
        let mut active = session("GROUP-CALL");
        active.group = Some(group_update(1));
        let generation = reg.insert(active);
        assert!(reg.send_group_epoch_if_current("GROUP-CALL", generation, 1, vec![7; 32],));

        let stats = reg.memory_stats();
        assert_eq!(stats.entries, 1);
        assert!(stats.bytes > 32, "session, roster and epoch are retained");

        assert!(reg.remove_if_current("GROUP-CALL", generation));
        let cleared = reg.memory_stats();
        assert_eq!(cleared.entries, 0);
        assert_eq!(cleared.bytes, 0);
    }

    #[test]
    fn memory_stats_include_queued_group_payload_allocations() {
        use crate::stats::HeapSize;

        let reg = CallRegistry::new();
        let mut active = session("GROUP-CALL");
        active.group = Some(group_update(1));
        let generation = reg.insert(active);
        let (event_tx, _event_rx) = async_channel::bounded(2);
        let (video_tx, _video_rx) = video_control_channel();
        reg.set_video_channels(
            "GROUP-CALL",
            generation,
            event_tx,
            video_tx,
            Box::new(|| {}),
        );
        let (group_tx, group_rx) = async_channel::bounded(2);
        reg.set_group_control_sender("GROUP-CALL", generation, Some(4), group_tx);
        group_rx.try_recv().expect("initial roster");

        let baseline = reg.memory_stats().bytes;
        let update = group_update(2);
        let payload_bytes = update.heap_bytes() as u64;
        assert!(reg.send_group_update_if_current("GROUP-CALL", generation, update.clone()));
        assert!(reg.send_call_event_if_current(
            "GROUP-CALL",
            generation,
            CallEvent::GroupUpdated(Box::new(update))
        ));

        assert!(
            reg.memory_stats().bytes >= baseline + payload_bytes.saturating_mul(2),
            "both queued boxes must include their retained roster allocations"
        );
    }

    #[test]
    fn call_event_queue_rejects_payloads_that_break_its_total_byte_budget() {
        let (event_tx, event_rx) = async_channel::bounded(DEFAULT_CALL_EVENT_QUEUE_CAPACITY);
        let queue = CallEventQueue::new(event_tx);
        let mut update = group_update(1);
        update.participants.push(GroupCallParticipant {
            jid: Jid::new("222222222222222", Server::Lid),
            pn: None,
            state: Some("connected".to_string()),
            participant_type: None,
            devices: vec![GroupCallDevice {
                jid: Jid::new("222222222222222", Server::Lid).with_device(1),
                platform: Some("web".to_string()),
                pid: Some(2),
                capability_version: Some(1),
                capability: vec![7; MAX_CALL_EVENT_QUEUE_BYTES],
            }],
        });

        assert!(!queue.force_send(CallEvent::GroupUpdated(Box::new(update))));
        assert!(event_rx.is_empty());
        assert_eq!(queue.retained_bytes(), 0);
        assert!(
            queue.force_send(CallEvent::RelayAllocated),
            "rejecting an oversized snapshot must leave capacity for lifecycle events"
        );
    }

    #[test]
    fn group_control_queue_rejects_payloads_that_break_its_total_byte_budget() {
        let (control_tx, control_rx) = async_channel::bounded(DEFAULT_CALL_EVENT_QUEUE_CAPACITY);
        let queue = GroupControlQueue::new(control_tx);
        let mut update = group_update(1);
        update.participants.push(GroupCallParticipant {
            jid: Jid::new("222222222222222", Server::Lid),
            pn: None,
            state: Some("connected".to_string()),
            participant_type: None,
            devices: vec![GroupCallDevice {
                jid: Jid::new("222222222222222", Server::Lid).with_device(1),
                platform: Some("web".to_string()),
                pid: Some(2),
                capability_version: Some(1),
                capability: vec![7; MAX_GROUP_CONTROL_QUEUE_BYTES],
            }],
        });

        assert!(!CallRegistry::force_send_preserving_epoch(
            &queue,
            GroupControl::Update(Box::new(update)),
        ));
        assert!(control_rx.is_empty());
        assert_eq!(queue.retained_bytes(), 0);
        assert!(
            CallRegistry::force_send_preserving_epoch(
                &queue,
                GroupControl::RawEpoch(GroupRawEpoch::new(1, vec![7; 32])),
            ),
            "rejecting an oversized roster must leave capacity for an epoch"
        );
    }

    #[test]
    fn group_media_attachment_splits_an_oversized_startup_transition() {
        let reg = CallRegistry::new();
        let generation = reg.insert(session("GROUP-CALL"));
        let mut update = group_update(1);
        update.participants.push(GroupCallParticipant {
            jid: Jid::new("222222222222222", Server::Lid),
            pn: None,
            state: Some("connected".to_string()),
            participant_type: None,
            devices: vec![GroupCallDevice {
                jid: Jid::new("222222222222222", Server::Lid).with_device(1),
                platform: Some("web".to_string()),
                pid: Some(2),
                capability_version: Some(1),
                capability: Vec::new(),
            }],
        });
        let (control_tx, control_rx) = async_channel::bounded(DEFAULT_CALL_EVENT_QUEUE_CAPACITY);
        let queue = GroupControlQueue::new(control_tx.clone());
        let base = GroupControl::Update(Box::new(update.clone()));
        let max_control_bytes = MAX_GROUP_CONTROL_QUEUE_BYTES / DEFAULT_CALL_EVENT_QUEUE_CAPACITY;
        let padding = max_control_bytes
            .checked_sub(size_of::<GroupControl>() + base.heap_bytes())
            .expect("fixture leaves room for capability padding");
        update.participants[0].devices[0].capability = vec![7; padding];

        let standalone = GroupControl::Update(Box::new(update.clone()));
        assert!(queue.accepts(&standalone));
        let transition = GroupControl::Transition {
            update: Box::new(update.clone()),
            epoch: GroupRawEpoch::new(1, vec![1; 32]),
        };
        assert!(
            !queue.accepts(&transition),
            "the epoch must be what tips the retained roster over one slot's budget"
        );
        drop(queue);

        assert_eq!(
            reg.apply_group_update_if_current(update, generation),
            GroupStateApply::Applied
        );
        assert!(reg.send_group_epoch_if_current("GROUP-CALL", generation, 1, vec![1; 32]));
        assert!(
            reg.set_group_control_sender("GROUP-CALL", generation, Some(4), control_tx),
            "attachment must replay the already-configured roster and its epoch separately"
        );
        assert!(matches!(
            control_rx.try_recv(),
            Ok(GroupControl::Update(update)) if update.transaction_id == 1
        ));
        assert!(matches!(
            control_rx.try_recv(),
            Ok(GroupControl::RawEpoch(epoch)) if epoch.transaction_id == 1
        ));
        assert!(control_rx.try_recv().is_err());
    }

    #[test]
    fn failed_group_media_attachment_retains_the_pending_epoch() {
        let reg = CallRegistry::new();
        let generation = reg.insert(session("GROUP-CALL"));
        assert_eq!(
            reg.apply_group_update_if_current(group_update(1), generation),
            GroupStateApply::Applied
        );
        assert!(reg.send_group_epoch_if_current("GROUP-CALL", generation, 1, vec![1; 32]));

        let (closed_tx, closed_rx) = async_channel::bounded(2);
        drop(closed_rx);
        assert!(
            !reg.set_group_control_sender("GROUP-CALL", generation, Some(4), closed_tx),
            "a closed driver mailbox must reject attachment"
        );
        assert_eq!(
            reg.pending_group_epoch_transaction_if_current("GROUP-CALL", generation),
            Some(1),
            "a failed handoff must leave the epoch available for a later attachment"
        );

        let (retry_tx, retry_rx) = async_channel::bounded(2);
        assert!(reg.set_group_control_sender("GROUP-CALL", generation, Some(4), retry_tx));
        assert!(matches!(
            retry_rx.try_recv(),
            Ok(GroupControl::Transition { update, epoch })
                if update.transaction_id == 1 && epoch.transaction_id == 1
        ));
    }

    #[test]
    fn partial_split_replay_retains_the_pending_epoch() {
        let reg = CallRegistry::new();
        let generation = reg.insert(session("GROUP-CALL"));
        let mut update = group_update(1);
        update.participants.push(GroupCallParticipant {
            jid: Jid::new("222222222222222", Server::Lid),
            pn: None,
            state: Some("connected".to_string()),
            participant_type: None,
            devices: vec![GroupCallDevice {
                jid: Jid::new("222222222222222", Server::Lid).with_device(1),
                platform: Some("web".to_string()),
                pid: Some(2),
                capability_version: Some(1),
                capability: Vec::new(),
            }],
        });
        let (control_tx, control_rx) = async_channel::bounded(1);
        let queue = GroupControlQueue::new(control_tx.clone());
        let base = GroupControl::Update(Box::new(update.clone()));
        let padding = MAX_GROUP_CONTROL_QUEUE_BYTES
            .checked_sub(size_of::<GroupControl>() + base.heap_bytes())
            .expect("fixture leaves room for capability padding");
        update.participants[0].devices[0].capability = vec![7; padding];
        assert!(queue.accepts(&GroupControl::Update(Box::new(update.clone()))));
        assert!(!queue.accepts(&GroupControl::Transition {
            update: Box::new(update.clone()),
            epoch: GroupRawEpoch::new(1, vec![1; 32]),
        }));
        drop(queue);

        assert_eq!(
            reg.apply_group_update_if_current(update, generation),
            GroupStateApply::Applied
        );
        assert!(reg.send_group_epoch_if_current("GROUP-CALL", generation, 1, vec![1; 32]));
        assert!(
            !reg.set_group_control_sender("GROUP-CALL", generation, Some(4), control_tx),
            "a one-slot mailbox cannot accept both halves of a split replay"
        );
        assert!(matches!(
            control_rx.try_recv(),
            Ok(GroupControl::Update(update)) if update.transaction_id == 1
        ));
        assert_eq!(
            reg.pending_group_epoch_transaction_if_current("GROUP-CALL", generation),
            Some(1)
        );

        let (retry_tx, retry_rx) = async_channel::unbounded();
        assert!(reg.set_group_control_sender("GROUP-CALL", generation, Some(4), retry_tx));
        assert!(matches!(
            retry_rx.try_recv(),
            Ok(GroupControl::Update(update)) if update.transaction_id == 1
        ));
        assert!(matches!(
            retry_rx.try_recv(),
            Ok(GroupControl::RawEpoch(epoch)) if epoch.transaction_id == 1
        ));
    }

    #[test]
    fn oversized_group_update_does_not_consume_the_registry_transaction() {
        let reg = CallRegistry::new();
        let generation = reg.insert(session("GROUP-CALL"));
        assert_eq!(
            reg.apply_group_update_if_current(group_update(1), generation),
            GroupStateApply::Applied
        );
        let (control_tx, control_rx) = async_channel::bounded(DEFAULT_CALL_EVENT_QUEUE_CAPACITY);
        reg.set_group_control_sender("GROUP-CALL", generation, Some(4), control_tx);
        control_rx.try_recv().expect("initial roster");

        let mut oversized = group_update(2);
        oversized.participants.push(GroupCallParticipant {
            jid: Jid::new("222222222222222", Server::Lid),
            pn: None,
            state: Some("connected".to_string()),
            participant_type: None,
            devices: vec![GroupCallDevice {
                jid: Jid::new("222222222222222", Server::Lid).with_device(1),
                platform: Some("web".to_string()),
                pid: Some(2),
                capability_version: Some(1),
                capability: vec![7; MAX_GROUP_CONTROL_QUEUE_BYTES],
            }],
        });
        assert_eq!(
            reg.apply_group_update_if_current(oversized, generation),
            GroupStateApply::InvalidSnapshot,
            "a snapshot that cannot fit one driver slot must be rejected before commit"
        );
        assert_eq!(
            reg.group_state_if_current("GROUP-CALL", generation)
                .and_then(|state| state.snapshot().map(|snapshot| snapshot.transaction_id)),
            Some(1)
        );
        assert!(control_rx.is_empty());

        let corrected = group_update(2);
        assert_eq!(
            reg.apply_group_update_if_current(corrected.clone(), generation),
            GroupStateApply::Applied,
            "a corrected redelivery at the same transaction must remain admissible"
        );
        assert!(reg.send_group_update_if_current("GROUP-CALL", generation, corrected));
        assert!(matches!(
            control_rx.try_recv(),
            Ok(GroupControl::Update(update)) if update.transaction_id == 2
        ));
    }

    #[test]
    fn oversized_pre_attachment_group_update_does_not_consume_the_transaction() {
        let reg = CallRegistry::new();
        let generation = reg
            .insert_call_link_checked(session("GROUP-CALL"))
            .expect("call-link registration without media");
        assert_eq!(
            reg.apply_group_update_if_current(group_update(1), generation),
            GroupStateApply::Applied
        );

        let mut oversized = group_update(2);
        oversized.participants.push(GroupCallParticipant {
            jid: Jid::new("222222222222222", Server::Lid),
            pn: None,
            state: Some("connected".to_string()),
            participant_type: None,
            devices: vec![GroupCallDevice {
                jid: Jid::new("222222222222222", Server::Lid).with_device(1),
                platform: Some("web".to_string()),
                pid: Some(2),
                capability_version: Some(1),
                capability: vec![7; MAX_GROUP_CONTROL_QUEUE_BYTES],
            }],
        });
        assert_eq!(
            reg.apply_group_update_if_current(oversized, generation),
            GroupStateApply::InvalidSnapshot,
            "a pre-attachment generation must not retain a snapshot too large for its future queue"
        );
        assert_eq!(
            reg.group_state_if_current("GROUP-CALL", generation)
                .and_then(|state| state.snapshot().map(|snapshot| snapshot.transaction_id)),
            Some(1)
        );

        assert_eq!(
            reg.apply_group_update_if_current(group_update(2), generation),
            GroupStateApply::Applied,
            "a corrected same-transaction redelivery must remain admissible"
        );
    }

    #[test]
    fn registration_listener_closes_offer_control_race() {
        let reg = CallRegistry::new();
        let listener = reg.listen_registration();
        assert!(
            listener.now_or_never().is_none(),
            "listener must remain pending before registration"
        );

        let listener = reg.listen_registration();
        let generation = reg.insert_ringing_group(session("GROUP-CALL"));
        assert!(
            listener.now_or_never().is_some(),
            "control waiters must wake when the offer generation registers"
        );
        assert_eq!(reg.generation_of("GROUP-CALL"), Some(generation));
    }

    #[test]
    fn bounded_group_mailbox_preserves_epoch_across_roster_bursts() {
        let (raw_tx, rx) = async_channel::bounded(2);
        let tx = GroupControlQueue::new(raw_tx);
        assert!(CallRegistry::force_send_preserving_epoch(
            &tx,
            GroupControl::Update(Box::new(group_update(7))),
        ));
        assert!(CallRegistry::force_send_preserving_epoch(
            &tx,
            GroupControl::Reaction("queued".to_string()),
        ));
        assert!(CallRegistry::force_send_preserving_epoch(
            &tx,
            GroupControl::Transition {
                update: Box::new(group_update(7)),
                epoch: GroupRawEpoch::new(7, vec![7; 32]),
            },
        ));
        for transaction_id in 8..=9 {
            assert!(CallRegistry::force_send_preserving_epoch(
                &tx,
                GroupControl::Update(Box::new(group_update(transaction_id))),
            ));
        }

        let controls = [rx.try_recv().unwrap(), rx.try_recv().unwrap()];
        assert!(
            controls.iter().any(|control| matches!(
                control,
                GroupControl::Transition { update, epoch }
                    if update.transaction_id == 7 && epoch.transaction_id == 7
            )),
            "the epoch and the roster required to install it must remain indivisible"
        );
        assert!(
            controls.iter().any(
                |control| matches!(control, GroupControl::Update(update) if update.transaction_id == 9)
            ),
            "roster coalescing must retain the newest independent update"
        );
    }

    #[test]
    fn bounded_group_mailbox_preserves_roster_when_every_slot_has_an_epoch() {
        let (raw_tx, rx) = async_channel::bounded(2);
        let tx = GroupControlQueue::new(raw_tx);
        for transaction_id in 8..=9 {
            assert!(CallRegistry::force_send_preserving_epoch(
                &tx,
                GroupControl::RawEpoch(GroupRawEpoch::new(
                    transaction_id,
                    vec![transaction_id as u8; 32],
                )),
            ));
        }

        assert!(CallRegistry::force_send_preserving_epoch(
            &tx,
            GroupControl::Update(Box::new(group_update(10))),
        ));

        let controls = [rx.try_recv().unwrap(), rx.try_recv().unwrap()];
        assert!(
            controls.iter().any(|control| matches!(
                control,
                GroupControl::Update(update) | GroupControl::Transition { update, .. }
                    if update.transaction_id == 10
            )),
            "epoch preservation must not evict the newest authoritative roster"
        );
        assert!(
            controls
                .iter()
                .filter_map(GroupControl::epoch_transaction_id)
                .max()
                .is_some_and(|transaction_id| transaction_id == 9),
            "the newest queued epoch must remain resident with the roster"
        );
    }

    #[test]
    fn group_sender_authorization_is_bound_to_creator_and_active_roster() {
        let reg = CallRegistry::new();
        reg.insert(session("GROUP-CALL"));
        let creator = Jid::new("111111111111111", Server::Lid);
        assert!(
            reg.group_sender_authorized("GROUP-CALL", &creator, &creator.clone().with_device(7)),
            "the creator must be able to install the first call-link admission snapshot"
        );
        assert_eq!(
            reg.canonical_group_participant(
                "GROUP-CALL",
                &creator,
                &creator.clone().with_device(7)
            ),
            None,
            "creator bootstrap must not authorize participant-scoped controls before a roster"
        );
        assert!(!reg.group_sender_authorized(
            "GROUP-CALL",
            &creator,
            &Jid::new("333333333333333", Server::Lid)
        ));

        let participant = Jid::new("333333333333333", Server::Lid);
        let device = participant.clone().with_device(3);
        let mut update = group_update(1);
        let mut roster_participant = GroupCallParticipant::new(
            participant.clone(),
            vec![GroupCallDevice::new(device.clone())],
        );
        let participant_pn = Jid::new("12025550113", Server::Pn);
        roster_participant.pn = Some(participant_pn.clone());
        roster_participant.state = Some("connected".to_string());
        update.participants = vec![
            {
                let mut creator = GroupCallParticipant::new(
                    update.call_creator.clone(),
                    vec![GroupCallDevice::new(
                        update.call_creator.clone().with_device(1),
                    )],
                );
                creator.state = Some("connected".to_string());
                creator
            },
            roster_participant,
        ];
        assert_eq!(reg.apply_group_update(update), GroupStateApply::Applied);

        assert!(reg.group_sender_authorized("GROUP-CALL", &creator, &device));
        assert!(reg.group_sender_authorized("GROUP-CALL", &creator, &participant));
        assert_eq!(
            reg.canonical_group_participant("GROUP-CALL", &creator, &participant_pn),
            Some(participant.clone())
        );
        assert!(!reg.group_sender_authorized("GROUP-CALL", &creator, &participant.with_device(4)));
        assert!(!reg.group_sender_authorized(
            "GROUP-CALL",
            &creator,
            &Jid::new("444444444444444", Server::Lid).with_device(4)
        ));
        assert!(!reg.group_sender_authorized(
            "GROUP-CALL",
            &Jid::new("555555555555555", Server::Lid),
            &device
        ));
        assert!(!reg.group_sender_authorized("OTHER-CALL", &creator, &device));

        let mut departed = group_update(2);
        let mut departed_participant = GroupCallParticipant::new(
            participant.clone(),
            vec![GroupCallDevice::new(device.clone())],
        );
        departed_participant.pn = Some(participant_pn.clone());
        departed_participant.state = Some("disconnected".to_string());
        departed.participants = vec![departed_participant];
        assert_eq!(reg.apply_group_update(departed), GroupStateApply::Applied);
        assert!(
            !reg.group_sender_authorized("GROUP-CALL", &creator, &device),
            "a retained but disconnected device must lose control authorization"
        );
        assert!(
            !reg.group_sender_authorized("GROUP-CALL", &creator, &participant_pn),
            "a disconnected participant's PN alias must lose control authorization"
        );
        assert!(
            reg.group_sender_authorized("GROUP-CALL", &creator, &creator.clone().with_device(7)),
            "the explicit creator bootstrap exception remains valid"
        );
        assert_eq!(
            reg.canonical_group_participant(
                "GROUP-CALL",
                &creator,
                &creator.clone().with_device(7)
            ),
            None,
            "a departed creator must not regain participant-scoped control authorization"
        );
    }

    #[test]
    fn invalid_relay_does_not_consume_the_registry_transaction() {
        let reg = CallRegistry::new();
        reg.insert(session("GROUP-CALL"));
        assert_eq!(
            reg.apply_group_update(group_update(1)),
            GroupStateApply::Applied
        );

        let mut invalid = group_update(2);
        let mut relay = group_relay(2);
        relay.key.clear();
        invalid.relay = Some(relay);
        assert_eq!(
            reg.apply_group_update(invalid),
            GroupStateApply::InvalidSnapshot
        );
        assert_eq!(
            reg.group_state("GROUP-CALL")
                .and_then(|state| state.snapshot().map(|update| update.transaction_id)),
            Some(1)
        );

        let mut corrected = group_update(2);
        corrected.relay = Some(group_relay(2));
        assert_eq!(
            reg.apply_group_update(corrected),
            GroupStateApply::Applied,
            "the corrected resend with the same transaction must remain retryable"
        );
    }

    #[test]
    fn relay_tag_change_does_not_consume_the_registry_transaction() {
        let reg = CallRegistry::new();
        let generation = reg.insert(session("GROUP-CALL"));
        let mut initial = group_update(1);
        initial.relay = Some(group_relay(1));
        assert_eq!(reg.apply_group_update(initial), GroupStateApply::Applied);
        let (tx, rx) = async_channel::bounded(4);
        assert!(reg.set_group_control_sender("GROUP-CALL", generation, Some(4), tx));
        let _ = rx.try_recv().expect("initial roster");

        let mut incompatible = group_update(2);
        let mut relay = group_relay(2);
        relay.warp_mi_tag_len = Some(6);
        incompatible.relay = Some(relay);
        assert_eq!(
            reg.apply_group_update_if_current(incompatible, generation),
            GroupStateApply::InvalidSnapshot
        );
        assert_eq!(
            reg.group_state_if_current("GROUP-CALL", generation)
                .and_then(|state| state.snapshot().map(|update| update.transaction_id)),
            Some(1),
            "an unsupported media-boundary change must not advance registry state"
        );

        let mut corrected = group_update(2);
        corrected.relay = Some(group_relay(2));
        assert_eq!(
            reg.apply_group_update_if_current(corrected, generation),
            GroupStateApply::Applied,
            "the corrected same-transaction refresh must remain retryable"
        );
    }

    #[test]
    fn group_media_attachment_rejects_a_retained_relay_tag_mismatch() {
        let reg = CallRegistry::new();
        let generation = reg.insert(session("GROUP-CALL"));
        let mut update = group_update(1);
        let mut relay = group_relay(1);
        relay.warp_mi_tag_len = Some(6);
        update.relay = Some(relay);
        assert_eq!(reg.apply_group_update(update), GroupStateApply::Applied);

        let (tx, _rx) = async_channel::bounded(4);
        assert!(
            !reg.set_group_control_sender("GROUP-CALL", generation, Some(4), tx),
            "a snapshot that overtook media attachment cannot be replayed into incompatible pipelines"
        );
    }

    #[test]
    fn session_and_group_snapshots_are_generation_scoped() {
        let reg = CallRegistry::new();
        let first = reg.insert(session("GROUP-CALL"));
        assert_eq!(
            reg.apply_group_update(group_update(1)),
            GroupStateApply::Applied
        );
        assert!(
            reg.snapshot_if_current("GROUP-CALL", first).is_some()
                && reg.group_state_if_current("GROUP-CALL", first).is_some()
        );

        let replacement = reg.insert(session("GROUP-CALL"));
        assert_ne!(replacement, first);
        assert!(reg.snapshot_if_current("GROUP-CALL", first).is_none());
        assert!(
            reg.group_state_if_current("GROUP-CALL", first).is_none(),
            "a stale invite handle must not read the replacement roster"
        );
        assert!(reg.snapshot_if_current("GROUP-CALL", replacement).is_some());
    }

    #[test]
    fn group_controls_and_invite_devices_reject_stale_generations() {
        let reg = CallRegistry::new();
        let stale = reg.insert(session("GROUP-CALL"));
        let current = reg.insert(session("GROUP-CALL"));
        assert_eq!(
            reg.apply_group_update(group_update(1)),
            GroupStateApply::Applied
        );
        let participant = Jid::new("111111111111111", Server::Lid);
        assert!(!reg.set_raised_hand_if_current("GROUP-CALL", stale, &participant, true,));
        assert!(reg.set_raised_hand_if_current("GROUP-CALL", current, &participant, true,));
        assert!(
            reg.group_state_if_current("GROUP-CALL", current)
                .expect("current group state")
                .raised_hands()
                .contains(&participant)
        );

        let self_device =
            GroupCallDevice::new(participant.clone().with_device(1)).with_capability(1, [1]);
        let peer_device =
            GroupCallDevice::new(Jid::new("222222222222222", Server::Lid).with_device(2))
                .with_capability(1, [2]);
        assert!(reg.set_group_invite_self_device("GROUP-CALL", current, self_device,));
        assert!(!reg.set_group_invite_peer_device("GROUP-CALL", stale, peer_device.clone(),));
        assert!(
            reg.group_invite_fallback_roster("GROUP-CALL", current)
                .is_none()
        );
        assert!(reg.set_group_invite_peer_device("GROUP-CALL", current, peer_device,));
        assert_eq!(
            reg.group_invite_fallback_roster("GROUP-CALL", current)
                .expect("current invite roster")
                .len(),
            2
        );
    }

    #[test]
    fn accepted_peer_capability_survives_delayed_sibling_preaccept() {
        let reg = CallRegistry::new();
        let generation = reg.insert(session("GROUP-CALL"));
        let self_device =
            GroupCallDevice::new(Jid::new("111111111111111", Server::Lid).with_device(1))
                .with_capability(1, [1]);
        assert!(reg.set_group_invite_self_device("GROUP-CALL", generation, self_device,));

        let early_sibling =
            GroupCallDevice::new(Jid::new("222222222222222", Server::Lid).with_device(2))
                .with_capability(1, [2]);
        assert!(reg.set_group_invite_peer_device("GROUP-CALL", generation, early_sibling,));

        let accepted =
            GroupCallDevice::new(Jid::new("222222222222222", Server::Lid).with_device(3))
                .with_capability(1, [3]);
        assert!(reg.select_group_invite_peer_device(
            "GROUP-CALL",
            generation,
            Some(accepted.clone()),
        ));

        let delayed_sibling =
            GroupCallDevice::new(Jid::new("222222222222222", Server::Lid).with_device(4))
                .with_capability(1, [4]);
        assert!(!reg.set_group_invite_peer_device("GROUP-CALL", generation, delayed_sibling,));
        assert!(!reg.select_group_invite_peer_device(
            "GROUP-CALL",
            generation,
            Some(GroupCallDevice::new(
                Jid::new("222222222222222", Server::Lid).with_device(5),
            )),
        ));

        let roster = reg
            .group_invite_fallback_roster("GROUP-CALL", generation)
            .expect("promotion roster");
        assert_eq!(roster[1].devices, vec![accepted]);
    }

    #[test]
    fn capabilityless_accept_retains_only_the_same_devices_preaccept() {
        let reg = CallRegistry::new();
        let self_device =
            GroupCallDevice::new(Jid::new("111111111111111", Server::Lid).with_device(1))
                .with_capability(1, [1]);
        let preaccepted =
            GroupCallDevice::new(Jid::new("222222222222222", Server::Lid).with_device(2))
                .with_capability(7, [2, 3]);

        let same_generation = reg.insert(session("SAME-DEVICE"));
        assert!(reg.set_group_invite_self_device(
            "SAME-DEVICE",
            same_generation,
            self_device.clone(),
        ));
        assert!(reg.set_group_invite_peer_device(
            "SAME-DEVICE",
            same_generation,
            preaccepted.clone(),
        ));
        assert!(reg.select_group_invite_peer_without_capability(
            "SAME-DEVICE",
            same_generation,
            &preaccepted.jid,
        ));
        assert_eq!(
            reg.group_invite_fallback_roster("SAME-DEVICE", same_generation)
                .expect("same-device preaccept remains usable")[1]
                .devices,
            vec![preaccepted.clone()]
        );

        let sibling_generation = reg.insert(session("SIBLING-DEVICE"));
        assert!(reg.set_group_invite_self_device(
            "SIBLING-DEVICE",
            sibling_generation,
            self_device,
        ));
        assert!(reg.set_group_invite_peer_device(
            "SIBLING-DEVICE",
            sibling_generation,
            preaccepted,
        ));
        assert!(reg.select_group_invite_peer_without_capability(
            "SIBLING-DEVICE",
            sibling_generation,
            &Jid::new("222222222222222", Server::Lid).with_device(3),
        ));
        assert!(
            reg.group_invite_fallback_roster("SIBLING-DEVICE", sibling_generation)
                .is_none(),
            "a sibling accept cannot borrow another device's capability"
        );
    }

    #[test]
    fn direct_calls_reject_group_state_mutations() {
        let reg = CallRegistry::new();
        let generation = reg.insert(session("DIRECT-CALL"));
        let participant = Jid::new("111111111111111", Server::Lid);
        assert!(!reg.set_raised_hand("DIRECT-CALL", &participant, true));
        assert!(!reg.set_raised_hand_if_current("DIRECT-CALL", generation, &participant, true,));
        assert!(!reg.set_screen_share(
            "DIRECT-CALL",
            &participant,
            ScreenShare::new(ScreenShareState::Started, Some(7)),
        ));
        assert!(!reg.set_screen_share_if_current(
            "DIRECT-CALL",
            generation,
            &participant,
            ScreenShare::new(ScreenShareState::Started, Some(7)),
        ));
        assert!(reg.group_state("DIRECT-CALL").is_none());
    }

    #[test]
    fn direct_calls_reject_waiting_room_promotion() {
        let reg = CallRegistry::new();
        reg.insert(session("GROUP-CALL"));
        assert_eq!(
            reg.apply_waiting_room(
                WaitingRoom::builder()
                    .call_id("GROUP-CALL".to_string())
                    .call_creator(Jid::new("111111111111111", Server::Lid))
                    .link_token("TEST-CALL-LINK".to_string())
                    .media(CallLinkMedia::Audio)
                    .enabled(true)
                    .is_admin(false)
                    .transaction_id(1)
                    .users(Vec::new())
                    .build(),
            ),
            GroupStateApply::InvalidSnapshot
        );
        assert!(!reg.is_group_call("GROUP-CALL"));
        assert!(reg.group_state("GROUP-CALL").is_none());
    }

    #[test]
    fn local_group_commits_reject_stale_generations() {
        let reg = CallRegistry::new();
        let stale = reg
            .insert_call_link_checked(session("GROUP-CALL"))
            .expect("valid stale call link");
        let current = reg
            .insert_call_link_checked(session("GROUP-CALL"))
            .expect("valid current call link");
        assert_eq!(
            reg.apply_group_update(group_update(1)),
            GroupStateApply::Applied
        );
        assert_eq!(
            reg.apply_waiting_room(
                WaitingRoom::builder()
                    .call_id("GROUP-CALL".to_string())
                    .call_creator(Jid::new("111111111111111", Server::Lid))
                    .link_token("TEST-CALL-LINK".to_string())
                    .media(CallLinkMedia::Audio)
                    .enabled(false)
                    .is_admin(true)
                    .transaction_id(1)
                    .users(Vec::new())
                    .build(),
            ),
            GroupStateApply::Applied
        );
        assert!(!reg.set_waiting_room_enabled_if_current("GROUP-CALL", stale, true));
        assert!(reg.set_waiting_room_enabled_if_current("GROUP-CALL", current, true));

        let (tx, rx) = async_channel::bounded(4);
        reg.set_group_control_sender("GROUP-CALL", current, Some(4), tx);
        let _ = rx.try_recv(); // initial authoritative snapshot
        assert!(!reg.send_group_update_if_current("GROUP-CALL", stale, group_update(2),));
        assert!(!reg.send_group_epoch_if_current("GROUP-CALL", stale, 2, vec![2; 32],));
        assert!(rx.try_recv().is_err());
        assert_eq!(
            reg.apply_group_update_if_current(group_update(2), current),
            GroupStateApply::Applied
        );
        assert!(reg.send_group_update_if_current("GROUP-CALL", current, group_update(2),));
        assert!(reg.send_group_epoch_if_current("GROUP-CALL", current, 2, vec![2; 32],));
        assert!(matches!(
            rx.try_recv(),
            Ok(GroupControl::Update(update)) if update.transaction_id == 2
        ));
        assert!(matches!(
            rx.try_recv(),
            Ok(GroupControl::Transition { update, epoch })
                if update.transaction_id == 2 && epoch.transaction_id == 2
        ));
    }

    #[test]
    fn reaction_enqueue_rejects_direct_calls_and_oversized_payloads() {
        let reg = CallRegistry::new();
        let generation = reg.insert(session("GROUP-CALL"));
        let (tx, rx) = async_channel::bounded(2);
        reg.set_group_control_sender("GROUP-CALL", generation, Some(4), tx);
        assert!(!reg.send_group_reaction_if_current("GROUP-CALL", generation, "👍".to_string(),));

        assert_eq!(
            reg.apply_group_update(group_update(1)),
            GroupStateApply::Applied
        );
        assert!(reg.send_group_reaction_if_current("GROUP-CALL", generation, "👍".to_string(),));
        assert!(matches!(
            rx.try_recv(),
            Ok(GroupControl::Reaction(emoji)) if emoji == "👍"
        ));
        assert!(!reg.send_group_reaction_if_current("GROUP-CALL", generation, "x".repeat(257),));
    }

    #[test]
    fn newest_group_state_survives_control_queue_backpressure() {
        let reg = CallRegistry::new();
        let generation = reg.insert(session("GROUP-CALL"));
        assert_eq!(
            reg.apply_group_update(group_update(1)),
            GroupStateApply::Applied
        );
        let (tx, rx) = async_channel::bounded(2);
        reg.set_group_control_sender("GROUP-CALL", generation, Some(4), tx);
        assert!(matches!(
            rx.try_recv(),
            Ok(GroupControl::Update(update)) if update.transaction_id == 1
        ));

        assert_eq!(
            reg.apply_group_update(group_update(2)),
            GroupStateApply::Applied
        );
        assert!(reg.send_group_update_if_current("GROUP-CALL", generation, group_update(2)));
        assert!(matches!(
            rx.try_recv(),
            Ok(GroupControl::Update(update)) if update.transaction_id == 2
        ));

        assert_eq!(
            reg.apply_group_update(group_update(3)),
            GroupStateApply::Applied
        );
        assert!(reg.send_group_update_if_current("GROUP-CALL", generation, group_update(3)));
        assert!(reg.send_group_epoch_if_current("GROUP-CALL", generation, 3, vec![3; 32]));
        assert!(matches!(
            rx.try_recv(),
            Ok(GroupControl::Update(update)) if update.transaction_id == 3
        ));
        assert!(matches!(
            rx.try_recv(),
            Ok(GroupControl::Transition { update, epoch })
                if update.transaction_id == 3 && epoch.transaction_id == 3
        ));
    }

    #[test]
    fn roster_only_update_preserves_committed_relay_under_backpressure() {
        let reg = CallRegistry::new();
        let generation = reg.insert(session("GROUP-CALL"));
        let mut initial = group_update(1);
        initial.relay = Some(group_relay(1));
        assert_eq!(reg.apply_group_update(initial), GroupStateApply::Applied);
        let (tx, rx) = async_channel::bounded(1);
        reg.set_group_control_sender("GROUP-CALL", generation, Some(4), tx);
        let _ = rx.try_recv().expect("initial authoritative snapshot");

        let mut relay_refresh = group_update(2);
        relay_refresh.relay = Some(group_relay(2));
        assert_eq!(
            reg.apply_group_update(relay_refresh.clone()),
            GroupStateApply::Applied
        );
        assert!(reg.send_group_update_if_current("GROUP-CALL", generation, relay_refresh));

        let roster_only = group_update(3);
        assert_eq!(
            reg.apply_group_update(roster_only.clone()),
            GroupStateApply::Applied
        );
        assert!(reg.send_group_update_if_current("GROUP-CALL", generation, roster_only));

        assert!(matches!(
            rx.try_recv(),
            Ok(GroupControl::Update(update))
                if update.transaction_id == 3
                    && update.relay.as_ref().and_then(|relay| relay.transaction_id) == Some(2)
        ));
        assert!(
            rx.try_recv().is_err(),
            "the later committed snapshot must coalesce the relay refresh into one update"
        );
    }

    #[test]
    fn answering_device_is_set_once_and_generation_guarded() {
        let reg = CallRegistry::new();
        assert_eq!(reg.answering_device_if_current("CID", 0), None, "unknown");
        let g = reg.insert(session("CID"));
        assert_eq!(
            reg.answering_device_if_current("CID", g),
            None,
            "none until an accept"
        );
        let dev = Jid::new("222222222222222", Server::Lid).with_device(2);
        reg.set_answering_device("CID", dev.clone());
        assert_eq!(reg.answering_device_if_current("CID", g), Some(dev.clone()));
        // First answerer wins: a later accept from another device is ignored.
        let other = Jid::new("222222222222222", Server::Lid).with_device(5);
        reg.set_answering_device("CID", other);
        assert_eq!(reg.answering_device_if_current("CID", g), Some(dev.clone()));
        // A replacement under a new generation isolates the device: the stale generation reads None.
        let g2 = reg.insert(session("CID"));
        assert_ne!(g, g2);
        assert_eq!(
            reg.answering_device_if_current("CID", g),
            None,
            "a stale generation must not read the replacement's device"
        );
        assert_eq!(reg.answering_device_if_current("CID", g2), None);
    }

    #[test]
    fn signaling_event_commit_survives_queue_backpressure() {
        let reg = CallRegistry::new();
        let generation = reg.insert(session("CID"));
        let (event_tx, event_rx) = async_channel::bounded(1);
        let (ctl_tx, _ctl_rx) = video_control_channel();
        reg.set_video_channels("CID", generation, event_tx.clone(), ctl_tx, Box::new(|| {}));

        let event = || CallEvent::VideoStateChanged {
            state: VideoState::Enabled,
            orientation: None,
            upgrade_token: None,
        };
        let permit = reg.reserve_call_event("CID").expect("first reservation");
        assert!(
            reg.reserve_call_event("CID").is_none(),
            "only one typed ack may own the signaling slot"
        );
        assert!(
            event_rx.is_empty(),
            "reservation must not publish the event"
        );
        event_tx
            .try_send(CallEvent::RelayAllocated)
            .expect("fill the queue while the typed ack is in flight");
        assert!(permit.send(event()));
        assert!(matches!(
            event_rx.try_recv(),
            Ok(CallEvent::VideoStateChanged { .. })
        ));
        assert!(
            reg.reserve_call_event("CID").is_none(),
            "publishing must not release the transition before its effects finish"
        );
        drop(permit);
        assert!(reg.reserve_call_event("CID").is_some());
        assert!(reg.reserve_call_event("UNKNOWN").is_none());
    }

    #[test]
    fn peer_upgrade_tokens_reject_cancel_request_aba() {
        let reg = CallRegistry::new();
        let generation = reg.insert(session("CID"));
        let first =
            match reg.apply_peer_video_state("CID", generation, VideoState::UpgradeRequestV2) {
                PeerVideoTransition::UpgradeRequested(token) => token,
                transition => panic!("unexpected transition: {transition:?}"),
            };
        assert!(reg.peer_video_request_is_current("CID", first));
        assert!(matches!(
            reg.apply_peer_video_state("CID", generation, VideoState::UpgradeCancel),
            PeerVideoTransition::Applied {
                teardown_local: true,
                ..
            }
        ));

        let second =
            match reg.apply_peer_video_state("CID", generation, VideoState::UpgradeRequestV2) {
                PeerVideoTransition::UpgradeRequested(token) => token,
                transition => panic!("unexpected transition: {transition:?}"),
            };
        assert_ne!(first, second);
        assert!(!reg.peer_video_request_is_current("CID", first));
        assert!(reg.peer_video_request_is_current("CID", second));
        assert!(!reg.complete_peer_video_request("CID", first));
        assert!(reg.peer_video_request_is_current("CID", second));
        assert!(reg.complete_peer_video_request("CID", second));
        assert_eq!(
            reg.video_states("CID", generation),
            Some((VideoState::Enabled, VideoState::Enabled))
        );
    }

    #[test]
    fn duplicate_peer_request_keeps_the_same_token() {
        let reg = CallRegistry::new();
        let generation = reg.insert(session("CID"));
        let request = |state| match reg.apply_peer_video_state("CID", generation, state) {
            PeerVideoTransition::UpgradeRequested(token) => token,
            transition => panic!("unexpected transition: {transition:?}"),
        };
        assert_eq!(
            request(VideoState::UpgradeRequest),
            request(VideoState::UpgradeRequestV2)
        );
    }

    #[test]
    fn stopped_is_directional_but_disabled_is_a_full_downgrade() {
        let reg = CallRegistry::new();
        let generation = reg.insert(session("CID"));
        let token =
            match reg.apply_peer_video_state("CID", generation, VideoState::UpgradeRequestV2) {
                PeerVideoTransition::UpgradeRequested(token) => token,
                transition => panic!("unexpected transition: {transition:?}"),
            };
        assert!(reg.complete_peer_video_request("CID", token));

        assert!(matches!(
            reg.apply_peer_video_state("CID", generation, VideoState::Stopped),
            PeerVideoTransition::Applied {
                teardown_local: false,
                ..
            }
        ));
        assert_eq!(
            reg.video_states("CID", generation),
            Some((VideoState::Enabled, VideoState::Stopped))
        );
        assert!(reg.snapshot("CID").expect("session").is_video);

        assert!(matches!(
            reg.apply_peer_video_state("CID", generation, VideoState::Disabled),
            PeerVideoTransition::Applied {
                teardown_local: true,
                ..
            }
        ));
        assert_eq!(
            reg.video_states("CID", generation),
            Some((VideoState::Disabled, VideoState::Disabled))
        );
        assert!(!reg.snapshot("CID").expect("session").is_video);
    }

    #[test]
    fn local_request_timeout_epoch_cannot_end_a_newer_request() {
        let reg = CallRegistry::new();
        let generation = reg.insert(session("CID"));
        let first = reg
            .begin_local_video_request("CID", generation)
            .expect("first request");
        assert!(reg.end_local_video_request("CID", generation, first));
        let second = reg
            .begin_local_video_request("CID", generation)
            .expect("second request");
        assert_ne!(first, second);
        assert!(!reg.end_local_video_request("CID", generation, first));
        assert_eq!(
            reg.video_states("CID", generation),
            Some((VideoState::UpgradeRequestV2, VideoState::Disabled))
        );
        assert!(reg.end_local_video_request("CID", generation, second));
    }

    #[test]
    fn cancelling_a_local_request_is_a_full_native_downgrade() {
        let reg = CallRegistry::new();
        let mut video_session = session("CID");
        video_session.is_video = true;
        let generation = reg.insert(video_session);
        assert!(reg.stop_local_video("CID", generation));
        let epoch = reg
            .begin_local_video_request("CID", generation)
            .expect("local request");

        assert!(reg.end_local_video_request("CID", generation, epoch));
        assert_eq!(
            reg.video_states("CID", generation),
            Some((VideoState::Disabled, VideoState::Disabled))
        );
        assert!(!reg.snapshot("CID").expect("session").is_video);
    }

    #[test]
    fn video_teardown_runs_after_registry_unlock() {
        let reg = Arc::new(CallRegistry::new());
        let generation = reg.insert(session("CID"));
        let (event_tx, _event_rx) = async_channel::bounded(1);
        let (ctl_tx, _ctl_rx) = video_control_channel();
        let lock_was_free = Arc::new(AtomicBool::new(false));
        reg.set_video_channels("CID", generation, event_tx, ctl_tx, {
            let reg = reg.clone();
            let lock_was_free = lock_was_free.clone();
            Box::new(move || {
                lock_was_free.store(reg.inner.try_lock().is_ok(), Ordering::SeqCst);
            })
        });

        assert!(reg.run_video_teardown("CID", generation));
        assert!(lock_was_free.load(Ordering::SeqCst));
        assert!(!reg.run_video_teardown("CID", generation));
    }

    #[test]
    fn authoritative_group_audio_downgrade_runs_video_teardown_after_unlock() {
        let reg = Arc::new(CallRegistry::new());
        let mut active = session("GROUP-CALL");
        active.is_video = true;
        let generation = reg.insert_group(active);
        let mut video = group_update(1);
        video.media = "video".to_string();
        assert_eq!(
            reg.apply_group_update_if_current(video, generation),
            GroupStateApply::Applied
        );

        let (event_tx, _event_rx) = async_channel::bounded(1);
        let (ctl_tx, _ctl_rx) = video_control_channel();
        let teardown_calls = Arc::new(AtomicU64::new(0));
        reg.set_video_channels("GROUP-CALL", generation, event_tx, ctl_tx, {
            let reg = reg.clone();
            let teardown_calls = teardown_calls.clone();
            Box::new(move || {
                assert!(
                    reg.inner.try_lock().is_ok(),
                    "video teardown must run outside the registry lock"
                );
                teardown_calls.fetch_add(1, Ordering::SeqCst);
            })
        });

        let audio = group_update(2);
        assert_eq!(
            reg.apply_group_update_if_current(audio, generation),
            GroupStateApply::Applied
        );
        assert_eq!(
            reg.video_states("GROUP-CALL", generation),
            Some((VideoState::Disabled, VideoState::Disabled))
        );
        assert!(!reg.snapshot("GROUP-CALL").expect("active session").is_video);
        assert_eq!(teardown_calls.load(Ordering::SeqCst), 1);
        assert!(
            !reg.run_video_teardown("GROUP-CALL", generation),
            "the downgrade hook is one-shot until video endpoints are reattached"
        );
    }

    #[test]
    fn video_teardown_is_one_shot_until_rearmed() {
        let reg = CallRegistry::new();
        let generation = reg.insert(session("CID"));
        let (event_tx, _event_rx) = async_channel::bounded(1);
        let (ctl_tx, _ctl_rx) = video_control_channel();
        let calls = Arc::new(AtomicU64::new(0));
        let hook = |calls: &Arc<AtomicU64>| {
            let calls = calls.clone();
            Box::new(move || {
                calls.fetch_add(1, Ordering::SeqCst);
            }) as Box<dyn Fn() + Send + Sync>
        };
        reg.set_video_channels("CID", generation, event_tx, ctl_tx, hook(&calls));

        assert!(reg.run_video_teardown("CID", generation));
        assert!(!reg.run_video_teardown("CID", generation));
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        assert!(reg.set_video_teardown("CID", generation, hook(&calls)));
        assert!(reg.run_video_teardown("CID", generation));
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert!(reg.remove_if_current("CID", generation));
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn video_mutations_are_generation_guarded_and_orientation_is_advisory() {
        let reg = CallRegistry::new();
        let stale = reg.insert(session("CID"));
        let current = reg.insert(session("CID"));
        let (event_tx, _event_rx) = async_channel::bounded(1);
        let (ctl_tx, ctl_rx) = video_control_channel();
        let teardown_calls = Arc::new(AtomicU64::new(0));
        reg.set_video_channels("CID", current, event_tx, ctl_tx, {
            let teardown_calls = teardown_calls.clone();
            Box::new(move || {
                teardown_calls.fetch_add(1, Ordering::SeqCst);
            })
        });

        assert!(reg.set_is_video("CID", current, true));
        assert!(!reg.set_is_video("CID", stale, false));
        assert!(reg.snapshot("CID").expect("session").is_video);
        assert!(!reg.run_video_teardown("CID", stale));
        assert_eq!(teardown_calls.load(Ordering::SeqCst), 0);

        reg.send_video_ctl("CID", current, VideoControl::Disable);
        for orientation in 0..100u8 {
            reg.send_video_ctl(
                "CID",
                current,
                VideoControl::SetOrientation(orientation % 4),
            );
        }
        reg.send_video_ctl("CID", current, VideoControl::Enable);
        reg.send_video_ctl("CID", current, VideoControl::RequireKeyframe);
        assert_eq!(ctl_rx.try_recv(), Ok(VideoControl::Disable));
        assert_eq!(ctl_rx.try_recv(), Ok(VideoControl::Enable));
        assert_eq!(ctl_rx.try_recv(), Ok(VideoControl::RequireKeyframe));
        assert_eq!(ctl_rx.try_recv(), Ok(VideoControl::SetOrientation(3)));

        reg.send_video_ctl("CID", stale, VideoControl::Disable);
        assert!(ctl_rx.try_recv().is_err());
    }

    #[test]
    fn removal_releases_video_endpoints_after_registry_unlock() {
        let reg = Arc::new(CallRegistry::new());
        let generation = reg.insert(session("CID"));
        let (event_tx, _event_rx) = async_channel::bounded(1);
        let (ctl_tx, _ctl_rx) = video_control_channel();
        let calls = Arc::new(AtomicU64::new(0));
        reg.set_video_channels("CID", generation, event_tx, ctl_tx, {
            let reg = reg.clone();
            let calls = calls.clone();
            Box::new(move || {
                assert!(reg.inner.try_lock().is_ok());
                calls.fetch_add(1, Ordering::SeqCst);
            })
        });

        assert!(reg.remove_if_current("CID", generation));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn ended_notify_fires_on_removal_even_without_a_media_task() {
        let reg = CallRegistry::new();
        let fired = Arc::new(AtomicBool::new(false));
        let g = reg.insert(session("CID"));
        reg.set_ended_notify("CID", g, {
            let fired = fired.clone();
            move || fired.store(true, Ordering::SeqCst)
        });
        // No media task attached (the connect-window case).
        assert!(reg.remove_if_current("CID", g));
        assert!(
            fired.load(Ordering::SeqCst),
            "removing a task-less entry must wake its wait_ended() via the on_terminal hook"
        );
    }

    #[test]
    fn ended_notify_is_generation_guarded_and_fires_via_abort_all() {
        let reg = CallRegistry::new();
        // A stale generation must not attach the hook.
        let stale = Arc::new(AtomicBool::new(false));
        let g = reg.insert(session("CID"));
        reg.set_ended_notify("CID", g + 99, {
            let stale = stale.clone();
            move || stale.store(true, Ordering::SeqCst)
        });
        // The live generation attaches it; abort_all (disconnect) fires it.
        let fired = Arc::new(AtomicBool::new(false));
        reg.set_ended_notify("CID", g, {
            let fired = fired.clone();
            move || fired.store(true, Ordering::SeqCst)
        });
        reg.abort_all();
        assert!(
            fired.load(Ordering::SeqCst),
            "abort_all must fire on_terminal"
        );
        assert!(
            !stale.load(Ordering::SeqCst),
            "a stale-generation hook must never have been attached"
        );
    }

    /// An abort handle that flips a shared flag, so a test can assert the registry actually aborts
    /// the stored handle (the runtime-agnostic analog of asserting a tokio task was cancelled).
    fn flag_handle(flag: &Arc<AtomicBool>) -> AbortHandle {
        let flag = flag.clone();
        AbortHandle::new(move || flag.store(true, Ordering::SeqCst))
    }

    #[test]
    fn send_rekey_is_one_shot_and_generation_guarded() {
        let reg = CallRegistry::new();
        let g = reg.insert(session("CID"));
        let (tx, rx) = async_channel::bounded::<String>(1);
        // A stale generation is ignored (no sender stored).
        reg.set_rekey_sender("CID", g + 99, tx.clone());
        reg.send_rekey("CID", "x".into());
        assert!(
            rx.try_recv().is_err(),
            "stale-generation sender must not fire"
        );
        // The live generation stores it; the first send fires, the second is a no-op (taken).
        reg.set_rekey_sender("CID", g, tx);
        reg.send_rekey("CID", "222222222222222:2@lid".into());
        assert_eq!(rx.try_recv().ok().as_deref(), Some("222222222222222:2@lid"));
        reg.send_rekey("CID", "again".into());
        assert!(rx.try_recv().is_err(), "rekey sender is one-shot");
    }

    #[test]
    fn ringing_is_one_shot_and_distinguishes_missed_from_ended() {
        let reg = CallRegistry::new();
        // An unanswered incoming offer: marked ringing, then a <terminate> consumes it as missed.
        reg.mark_incoming_ringing("RING");
        assert!(reg.take_ringing("RING"), "an unanswered offer is missed");
        assert!(
            !reg.take_ringing("RING"),
            "one-shot: a duplicate <terminate> is ended, not a second missed"
        );
        // A call we never rang (outgoing, or a terminate with no preceding offer) is never missed.
        assert!(!reg.take_ringing("NEVER"));
    }

    #[test]
    fn memory_stats_include_ringing_entries() {
        let reg = CallRegistry::new();
        reg.mark_incoming_ringing("FIRST-RING");
        reg.mark_incoming_ringing("SECOND-RING");
        let stats = reg.memory_stats();
        assert_eq!(stats.entries, 2);
        assert!(stats.bytes > 0);

        assert!(reg.take_ringing("FIRST-RING"));
        assert_eq!(reg.memory_stats().entries, 1);
        reg.abort_all();
        assert_eq!(reg.memory_stats().entries, 0);
    }

    #[test]
    fn answering_an_incoming_offer_clears_its_ringing_flag() {
        let reg = CallRegistry::new();
        reg.mark_incoming_ringing("CID");
        // Accepting the call registers it as active (insert), which clears the ringing flag so a
        // later <terminate> reads as ended, not missed.
        let _g = reg.insert(session("CID"));
        assert!(
            !reg.take_ringing("CID"),
            "an answered call must not surface a missed call on terminate"
        );
    }

    #[test]
    fn abort_all_clears_ringing() {
        let reg = CallRegistry::new();
        reg.mark_incoming_ringing("CID");
        reg.abort_all();
        assert!(
            !reg.take_ringing("CID"),
            "a disconnect must drop stale ringing state so it can't surface after reconnect"
        );
    }

    #[test]
    fn insert_transition_remove() {
        let reg = CallRegistry::new();
        let _g = reg.insert(session("CID"));
        assert_eq!(reg.phase("CID"), Some(CallPhase::Idle));
        assert!(reg.transition("CID", CallPhase::Calling));
        assert_eq!(reg.phase("CID"), Some(CallPhase::Calling));
        assert!(!reg.transition("UNKNOWN", CallPhase::Calling));
        assert!(reg.remove("CID"));
        assert!(!reg.remove("CID"));
        assert_eq!(reg.active_count(), 0);
    }

    #[test]
    fn take_dismiss_targets_one_shot_and_dropped_on_remove() {
        let reg = CallRegistry::new();
        let peer = Jid::new("222222222222222", Server::Lid);
        let devs = vec![peer.with_device(1), peer.with_device(2)];

        let mut s = session("CID");
        s.ring_devices = devs.clone();
        let _g = reg.insert(s);

        // First take returns the targets; a second is None (one-shot, so a duplicate accept/reject
        // can't re-dismiss).
        let (got_creator, taken) = reg.take_dismiss_targets("CID").expect("first take");
        assert_eq!(got_creator, Jid::new("111111111111111", Server::Lid));
        assert_eq!(taken, devs);
        assert!(reg.take_dismiss_targets("CID").is_none(), "one-shot");

        // The device list dies with the entry: insert, remove, then take finds nothing.
        let mut s2 = session("CID2");
        s2.ring_devices = devs.clone();
        let _g2 = reg.insert(s2);
        assert!(reg.remove("CID2"));
        assert!(
            reg.take_dismiss_targets("CID2").is_none(),
            "removed entry leaves no tracking to leak"
        );

        assert!(reg.take_dismiss_targets("UNKNOWN").is_none());
    }

    #[test]
    fn remove_aborts_media_task() {
        let reg = CallRegistry::new();
        let g = reg.insert(session("A"));
        let flag = Arc::new(AtomicBool::new(false));
        reg.set_media_task("A", g, flag_handle(&flag));
        assert!(reg.remove("A"));
        assert!(
            flag.load(Ordering::SeqCst),
            "removing a call must abort its media task"
        );
    }

    #[test]
    fn abort_all_aborts_media_tasks() {
        let reg = CallRegistry::new();
        let flags: Vec<Arc<AtomicBool>> = ["A", "B"]
            .iter()
            .map(|id| {
                let g = reg.insert(session(id));
                let flag = Arc::new(AtomicBool::new(false));
                reg.set_media_task(id, g, flag_handle(&flag));
                flag
            })
            .collect();
        assert_eq!(reg.active_count(), 2);
        assert_eq!(reg.abort_all(), 2);
        assert_eq!(reg.active_count(), 0);
        assert!(
            flags.iter().all(|f| f.load(Ordering::SeqCst)),
            "abort_all must abort every media task"
        );
    }

    #[test]
    fn replace_aborts_the_old_media_task() {
        let reg = CallRegistry::new();
        let g = reg.insert(session("A"));
        let old = Arc::new(AtomicBool::new(false));
        let new = Arc::new(AtomicBool::new(false));
        reg.set_media_task("A", g, flag_handle(&old));
        // Replacing the handle for a live call (same generation) aborts the old one, not the new.
        reg.set_media_task("A", g, flag_handle(&new));
        assert!(old.load(Ordering::SeqCst), "replaced task must be aborted");
        assert!(!new.load(Ordering::SeqCst), "the replacement stays live");
        // Cleanup: removing the call aborts the replacement too.
        reg.remove("A");
        assert!(new.load(Ordering::SeqCst), "replacement aborted on remove");
    }

    /// A handle whose abort reads the registry back, modelling a `Runtime` that drops the task
    /// synchronously so the task's own cleanup re-enters. `std::sync::Mutex` is not reentrant, so
    /// any abort still holding the registry guard deadlocks instead of returning.
    fn reentrant_handle(reg: &Arc<CallRegistry>) -> AbortHandle {
        let reg = Arc::downgrade(reg);
        AbortHandle::new(move || {
            if let Some(reg) = reg.upgrade() {
                let _ = reg.active_count();
            }
        })
    }

    /// Runs `body` on a worker thread and fails instead of hanging the suite if it deadlocks.
    ///
    /// Only a timeout means deadlock. A dropped sender means `body` panicked, so the worker is
    /// joined and its payload re-raised rather than reported as a hang.
    fn within_five_seconds(name: &str, body: impl FnOnce() + Send + 'static) {
        use std::sync::mpsc::RecvTimeoutError;

        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let worker = std::thread::spawn(move || {
            body();
            let _ = done_tx.send(());
        });
        if done_rx.recv_timeout(Duration::from_secs(5)) == Err(RecvTimeoutError::Timeout) {
            panic!("{name} deadlocked: the abort ran while the registry lock was held");
        }
        // Either the body signalled or it unwound and dropped the sender; joining covers both and
        // re-raises the payload so a failed assertion inside the body is what the suite reports.
        if let Err(payload) = worker.join() {
            std::panic::resume_unwind(payload);
        }
    }

    #[test]
    fn replacing_a_media_task_does_not_abort_under_the_registry_lock() {
        within_five_seconds("replacing a media task", || {
            let reg = Arc::new(CallRegistry::new());
            let generation = reg.insert(session("CID"));
            reg.set_media_task("CID", generation, reentrant_handle(&reg));
            let installed = Arc::new(AtomicBool::new(false));
            reg.set_media_task("CID", generation, flag_handle(&installed));
            assert!(
                !installed.load(Ordering::SeqCst),
                "the replacement must stay live"
            );
            assert!(reg.remove_if_current("CID", generation));
            assert!(installed.load(Ordering::SeqCst));
        });
    }

    #[test]
    fn rejecting_a_stale_media_task_does_not_abort_under_the_registry_lock() {
        within_five_seconds("rejecting a stale media task", || {
            let reg = Arc::new(CallRegistry::new());
            let stale = reg.insert(session("CID"));
            let current = reg.insert(session("CID"));
            assert_ne!(stale, current);
            // The generation-mismatch exit aborts the handle it was given; so does the unknown-call
            // exit. Both must run the abort after the guard is gone.
            reg.set_media_task("CID", stale, reentrant_handle(&reg));
            reg.set_media_task("GONE", 0, reentrant_handle(&reg));
            assert_eq!(reg.active_count(), 1);
        });
    }

    /// The registry recovers a poisoned guard, so one panic under the lock cannot disable VoIP for
    /// the rest of the process -- including `abort_all`, the teardown a caller needs after a panic.
    #[test]
    fn a_panic_under_the_registry_lock_leaves_the_registry_usable() {
        let reg = CallRegistry::new();
        let generation = reg.insert(session("CID"));
        reg.mark_incoming_ringing("RINGING");

        // The panic hook is left alone: silencing it is a process-wide mutation that would swallow
        // a genuine panic from any test running in parallel, so this one announces itself instead.
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _active = reg.active_calls();
            let _ringing = reg.ringing_calls();
            panic!(
                "intentional panic under both registry guards; this test asserts on the recovery"
            );
        }));

        assert!(outcome.is_err());
        assert!(reg.inner.is_poisoned() && reg.ringing.is_poisoned());
        assert_eq!(reg.active_count(), 1);
        assert!(reg.is_current("CID", generation));
        assert!(reg.take_ringing("RINGING"));
        assert_eq!(reg.abort_all(), 1);
    }

    /// Attaching a media task to an already-removed call must abort the handle immediately so the
    /// task can't outlive the call.
    #[test]
    fn set_media_task_on_unknown_call_aborts_immediately() {
        let reg = CallRegistry::new();
        let flag = Arc::new(AtomicBool::new(false));
        reg.set_media_task("GONE", 0, flag_handle(&flag));
        assert!(
            flag.load(Ordering::SeqCst),
            "an orphan media task must be aborted immediately"
        );
    }

    /// A same-call-id re-offer (retry/glare) replaces the prior call: the old media task is aborted,
    /// and the old generation can no longer reap the replacement. Guards the ABA hazard the example
    /// hit -- a finishing task removing a newer call's handle.
    #[test]
    fn replacement_supersedes_and_old_generation_cannot_reap_it() {
        let reg = CallRegistry::new();
        let g1 = reg.insert(session("CID"));
        let a = Arc::new(AtomicBool::new(false));
        reg.set_media_task("CID", g1, flag_handle(&a));

        // Re-offer with the same id supersedes: aborts task A, fresh generation.
        let g2 = reg.insert(session("CID"));
        assert_ne!(g1, g2);
        assert!(
            a.load(Ordering::SeqCst),
            "the superseded call's task must be aborted on replacement"
        );
        let b = Arc::new(AtomicBool::new(false));
        reg.set_media_task("CID", g2, flag_handle(&b));

        // Task A's stale self-cleanup (old generation) must NOT reap the live replacement.
        assert!(
            !reg.remove_if_current("CID", g1),
            "the old generation must not reap the replacement"
        );
        assert!(!b.load(Ordering::SeqCst), "the replacement task stays live");
        assert_eq!(reg.active_count(), 1);

        // Attaching under the stale generation aborts it immediately (it is for a dead call).
        let stale = Arc::new(AtomicBool::new(false));
        reg.set_media_task("CID", g1, flag_handle(&stale));
        assert!(
            stale.load(Ordering::SeqCst),
            "a stale-generation media task must be aborted"
        );
        assert!(
            !b.load(Ordering::SeqCst),
            "the live replacement is untouched"
        );

        // The current generation reaps correctly.
        assert!(reg.remove_if_current("CID", g2));
        assert!(
            b.load(Ordering::SeqCst),
            "the current generation reap aborts the live task"
        );
        assert_eq!(reg.active_count(), 0);
    }
}

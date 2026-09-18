//! The control vocabulary a backend and the control plane share.
//!
//! These are the command shapes a call sends into its media plane: group roster/epoch transitions,
//! video-plane controls, the recv-rekey answer, and the mailbox types that carry them. They used to
//! live in the `voip` driver, which forced the registry to depend on the engine to name a video
//! command. Moved here so the control plane can build and deliver every command with the engine off;
//! `crate::voip::driver` re-exports all of them.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};

use futures::FutureExt;
use portable_atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};
use zeroize::Zeroize;

use crate::types::group_call::{GROUP_CALL_MAX_PARTICIPANTS, GroupCallUpdate};
use crate::voip_control::{MediaAudioCodec, MediaKeyframeUrgency};

/// Lossless, ordered signaling mutations consumed by the sans-I/O group-media engine.
pub enum GroupControl {
    Update(Box<GroupCallUpdate>),
    /// One roster snapshot and its decrypted epoch, kept indivisible under mailbox backpressure.
    Transition {
        update: Box<GroupCallUpdate>,
        epoch: GroupRawEpoch,
    },
    RawEpoch(GroupRawEpoch),
    Reaction(String),
}

impl GroupControl {
    /// The transaction this control carries key material for, if any.
    pub(crate) fn epoch_transaction_id(&self) -> Option<u32> {
        match self {
            Self::Transition { epoch, .. } | Self::RawEpoch(epoch) => Some(epoch.transaction_id),
            Self::Update(_) | Self::Reaction(_) => None,
        }
    }

    pub(crate) fn heap_bytes(&self) -> usize {
        use core::mem::size_of;

        use crate::stats::HeapSize;

        match self {
            Self::Update(update) => size_of::<GroupCallUpdate>() + update.heap_bytes(),
            Self::Transition { update, epoch } => {
                size_of::<GroupCallUpdate>() + update.heap_bytes() + epoch.heap_bytes()
            }
            Self::RawEpoch(epoch) => epoch.heap_bytes(),
            Self::Reaction(emoji) => emoji.capacity(),
        }
    }
}

/// A group control split into whether it carries an epoch, so the driver can apply the roster and
/// the key in the right order without re-matching the outer enum.
pub enum NormalizedGroupControl {
    Update {
        update: Box<GroupCallUpdate>,
        paired_epoch: Option<GroupRawEpoch>,
    },
    RawEpoch(GroupRawEpoch),
    Reaction(String),
}

impl From<GroupControl> for NormalizedGroupControl {
    fn from(control: GroupControl) -> Self {
        match control {
            GroupControl::Update(update) => Self::Update {
                update,
                paired_epoch: None,
            },
            GroupControl::Transition { update, epoch } => Self::Update {
                update,
                paired_epoch: Some(epoch),
            },
            GroupControl::RawEpoch(epoch) => Self::RawEpoch(epoch),
            GroupControl::Reaction(emoji) => Self::Reaction(emoji),
        }
    }
}

/// One decrypted keygen-v2 epoch. Debug output is deliberately redacted and the bytes are erased
/// when the command leaves the driver, regardless of whether the engine accepted it.
pub struct GroupRawEpoch {
    pub transaction_id: u32,
    raw_epoch: Vec<u8>,
}

impl GroupRawEpoch {
    pub fn new(transaction_id: u32, raw_epoch: Vec<u8>) -> Self {
        Self {
            transaction_id,
            raw_epoch,
        }
    }

    pub(crate) fn as_bytes(&self) -> &[u8] {
        &self.raw_epoch
    }

    pub(crate) fn heap_bytes(&self) -> usize {
        self.raw_epoch.capacity()
    }
}

impl core::fmt::Debug for GroupRawEpoch {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("GroupRawEpoch")
            .field("transaction_id", &self.transaction_id)
            .field("raw_epoch", &"[redacted]")
            .finish()
    }
}

impl Drop for GroupRawEpoch {
    fn drop(&mut self) {
        self.raw_epoch.zeroize();
    }
}

/// Mid-call video-plane commands from the shell (upgrade / downgrade / peer orientation). Kept out
/// of the engine so it stays sans-IO; the drive loop translates each into an engine method call.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum VideoControl {
    /// Select the source generation accepted by the timestamped input queue.
    SetInputGeneration(u64),
    /// RTP clock increment for each access unit. Sent before attaching a source whose cadence is
    /// different from the 15 fps compatibility default.
    SetTimestampStride(u32),
    /// Bring the video plane up with outbound video ALLOWED (a from-start call, an accept, or the
    /// initiator once the peer accepted the upgrade).
    Enable,
    /// Bring the video plane up but hold outbound video off the wire until the peer accepts (the
    /// initiator of an upgrade). Inbound still decodes. A later `Enable` ungates it.
    EnableAwaitingAccept,
    /// Tear the video plane down (downgrade to audio).
    Disable,
    /// Gate outbound video off the wire while inbound keeps decoding (our camera stopped; the
    /// peer is still sending). `Enable` later ungates it, like an accepted upgrade.
    DisableOutbound,
    /// Tear the video plane down while retaining queued legacy AUs for a legacy reattach.
    DisableKeepLegacy,
    /// Require the next outbound access unit to be an IDR frame after changing its source role.
    RequireKeyframe,
    /// Ask the PEER for a keyframe, by RTCP PLI: the mirror of `RequireKeyframe`. See
    /// It is the caller that decides what becomes of one; the engine throttles by urgency.
    RequestPeerKeyframe(MediaKeyframeUrgency),
    /// The peer's device orientation (0..3, ×90°) from a `<video>` stanza.
    SetOrientation(u8),
    /// One routed group participant's device orientation.
    SetParticipantOrientation {
        participant: wacore_binary::Jid,
        orientation: u8,
    },
}

/// State changes stay FIFO so `Disable` performs its purge before a later `Enable`; only the latest
/// orientation matters while the driver is busy.
enum VideoControlMessage {
    State(VideoControl),
    ParticipantOrientationsReady,
    /// A peer-keyframe request is pending; the urgency travels beside it. Every other state
    /// change here is a discrete consumer action, so an unbounded queue costs nothing -- but a
    /// sink is invited to ask on every dropped access unit, which is a run this queue would
    /// otherwise grow to hold while the drive loop is busy. The engine's throttle only runs
    /// after the dequeue, and so cannot bound what waits ahead of it.
    PeerKeyframeRequested,
}

/// [`PendingPeerKeyframe`] slots, ordered so `fetch_max` raises urgency and never lowers it: a
/// decoder that failed after asking politely still needs the throttle bypassed, and the request
/// it is joining was queued at the lower one.
const PEER_KEYFRAME_NONE: u8 = 0;
const PEER_KEYFRAME_COALESCED: u8 = 1;
const PEER_KEYFRAME_IMMEDIATE: u8 = 2;

fn peer_keyframe_slot(urgency: MediaKeyframeUrgency) -> u8 {
    match urgency {
        MediaKeyframeUrgency::Coalesced => PEER_KEYFRAME_COALESCED,
        MediaKeyframeUrgency::Immediate => PEER_KEYFRAME_IMMEDIATE,
    }
}

#[derive(Default)]
struct PendingParticipantOrientations {
    values: Mutex<HashMap<wacore_binary::Jid, u8>>,
    /// `values.len()`, written inside its critical section. The drive loop consults it once per
    /// iteration and the map is empty in every call that never routes a group video participant, so
    /// the read must not cost a lock.
    len: AtomicUsize,
    marker_queued: AtomicBool,
}

#[derive(Clone)]
pub struct VideoControlSender {
    state: async_channel::Sender<VideoControlMessage>,
    orientation: async_channel::Sender<u8>,
    participant_orientations: Arc<PendingParticipantOrientations>,
    peer_keyframe: Arc<AtomicU8>,
}

/// Receiving half of [`video_control_channel`].
pub struct VideoControlReceiver {
    state: async_channel::Receiver<VideoControlMessage>,
    orientation: async_channel::Receiver<u8>,
    participant_orientations: Arc<PendingParticipantOrientations>,
    peer_keyframe: Arc<AtomicU8>,
    ready_participant_orientations: Mutex<VecDeque<(wacore_binary::Jid, u8)>>,
    /// `ready_participant_orientations.len()`, same role as [`PendingParticipantOrientations::len`].
    ready_participant_orientations_len: AtomicUsize,
}

/// Build the control mailbox used by one call driver.
pub fn video_control_channel() -> (VideoControlSender, VideoControlReceiver) {
    let (state_tx, state_rx) = async_channel::unbounded();
    let (orientation_tx, orientation_rx) = async_channel::bounded(1);
    let participant_orientations = Arc::new(PendingParticipantOrientations::default());
    let peer_keyframe = Arc::new(AtomicU8::new(PEER_KEYFRAME_NONE));
    (
        VideoControlSender {
            state: state_tx,
            orientation: orientation_tx,
            participant_orientations: participant_orientations.clone(),
            peer_keyframe: peer_keyframe.clone(),
        },
        VideoControlReceiver {
            state: state_rx,
            orientation: orientation_rx,
            participant_orientations,
            peer_keyframe,
            ready_participant_orientations: Mutex::new(VecDeque::new()),
            ready_participant_orientations_len: AtomicUsize::new(0),
        },
    )
}

impl VideoControlSender {
    /// Queue a state change, or replace the pending orientation with the newest value.
    pub fn send(&self, control: VideoControl) -> bool {
        match control {
            VideoControl::SetOrientation(orientation) => {
                self.orientation.force_send(orientation).is_ok()
            }
            VideoControl::SetParticipantOrientation {
                participant,
                orientation,
            } => {
                let needs_marker = {
                    let mut pending = self
                        .participant_orientations
                        .values
                        .lock()
                        .expect("participant orientation lock poisoned");
                    if !pending.contains_key(&participant)
                        && pending.len() == GROUP_CALL_MAX_PARTICIPANTS
                        && let Some(evicted) = pending.keys().next().cloned()
                    {
                        pending.remove(&evicted);
                    }
                    pending.insert(participant, orientation);
                    self.participant_orientations
                        .len
                        .store(pending.len(), Ordering::Relaxed);
                    !self
                        .participant_orientations
                        .marker_queued
                        .swap(true, Ordering::Relaxed)
                };
                if !needs_marker {
                    return true;
                }
                if self
                    .state
                    .try_send(VideoControlMessage::ParticipantOrientationsReady)
                    .is_ok()
                {
                    true
                } else {
                    let mut pending = self
                        .participant_orientations
                        .values
                        .lock()
                        .expect("participant orientation lock poisoned");
                    pending.clear();
                    self.participant_orientations
                        .len
                        .store(0, Ordering::Relaxed);
                    self.participant_orientations
                        .marker_queued
                        .store(false, Ordering::Relaxed);
                    false
                }
            }
            VideoControl::RequestPeerKeyframe(urgency) => {
                let previous = self
                    .peer_keyframe
                    .fetch_max(peer_keyframe_slot(urgency), Ordering::Relaxed);
                // Not once the receiver is gone: the marker that would carry this
                // raise can no longer be consumed, so the slot would stay set and
                // every later send would report a queued request that does not
                // exist. Falling through clears it on the failed send instead.
                if previous != PEER_KEYFRAME_NONE && !self.state.is_closed() {
                    return true;
                }
                if self
                    .state
                    .try_send(VideoControlMessage::PeerKeyframeRequested)
                    .is_ok()
                {
                    true
                } else {
                    self.peer_keyframe
                        .store(PEER_KEYFRAME_NONE, Ordering::Relaxed);
                    false
                }
            }
            state => self
                .state
                .try_send(VideoControlMessage::State(state))
                .is_ok(),
        }
    }

    #[cfg(all(test, feature = "voip-mlow"))]
    pub(crate) fn retained_len(&self) -> usize {
        self.state
            .len()
            .saturating_add(self.orientation.len())
            .saturating_add(
                self.participant_orientations
                    .values
                    .lock()
                    .expect("participant orientation lock poisoned")
                    .len(),
            )
    }

    pub(crate) fn retained_bytes(&self) -> usize {
        use core::mem::size_of;

        use crate::stats::HeapSize;

        let pending = self
            .participant_orientations
            .values
            .lock()
            .expect("participant orientation lock poisoned");
        self.state
            .len()
            .saturating_mul(size_of::<VideoControlMessage>())
            .saturating_add(self.orientation.len().saturating_mul(size_of::<u8>()))
            .saturating_add(crate::stats::hash_table_bytes(
                pending.capacity(),
                size_of::<(wacore_binary::Jid, u8)>(),
            ))
            .saturating_add(pending.keys().map(HeapSize::heap_bytes).sum::<usize>())
    }
}

impl VideoControlReceiver {
    /// Whether both halves have lost every sender.
    pub fn is_closed(&self) -> bool {
        self.state.is_closed() && self.orientation.is_closed()
    }

    fn take_peer_keyframe_request(&self) -> Option<VideoControl> {
        match self
            .peer_keyframe
            .swap(PEER_KEYFRAME_NONE, Ordering::Relaxed)
        {
            PEER_KEYFRAME_NONE => None,
            PEER_KEYFRAME_IMMEDIATE => Some(VideoControl::RequestPeerKeyframe(
                MediaKeyframeUrgency::Immediate,
            )),
            _ => Some(VideoControl::RequestPeerKeyframe(
                MediaKeyframeUrgency::Coalesced,
            )),
        }
    }

    fn take_participant_orientation(&self) -> Option<VideoControl> {
        // Both queues empty is the steady state (always, for a call with no routed group video), and
        // this runs on every drive-loop iteration. A stale zero cannot swallow an orientation: the
        // sender fills the map before publishing its marker, so the marker message wakes the loop
        // again and the counter is visible by then.
        if self
            .ready_participant_orientations_len
            .load(Ordering::Relaxed)
            == 0
            && self.participant_orientations.len.load(Ordering::Relaxed) == 0
        {
            return None;
        }
        let mut ready = self
            .ready_participant_orientations
            .lock()
            .expect("participant orientation lock poisoned");
        if ready.is_empty() {
            let mut pending = self
                .participant_orientations
                .values
                .lock()
                .expect("participant orientation lock poisoned");
            ready.extend(pending.drain());
            self.participant_orientations
                .len
                .store(0, Ordering::Relaxed);
            self.participant_orientations
                .marker_queued
                .store(false, Ordering::Relaxed);
        }
        let taken = ready.pop_front();
        self.ready_participant_orientations_len
            .store(ready.len(), Ordering::Relaxed);
        taken.map(
            |(participant, orientation)| VideoControl::SetParticipantOrientation {
                participant,
                orientation,
            },
        )
    }

    /// Receive a ready state first, otherwise the latest orientation.
    pub fn try_recv(&self) -> Result<VideoControl, async_channel::TryRecvError> {
        let state_error = loop {
            match self.state.try_recv() {
                Ok(VideoControlMessage::State(state)) => return Ok(state),
                Ok(VideoControlMessage::ParticipantOrientationsReady) => {
                    if let Some(orientation) = self.take_participant_orientation() {
                        return Ok(orientation);
                    }
                }
                Ok(VideoControlMessage::PeerKeyframeRequested) => {
                    if let Some(request) = self.take_peer_keyframe_request() {
                        return Ok(request);
                    }
                }
                Err(error) => break error,
            }
        };
        if let Some(orientation) = self.take_participant_orientation() {
            return Ok(orientation);
        }
        match self.orientation.try_recv() {
            Ok(orientation) => Ok(VideoControl::SetOrientation(orientation)),
            Err(async_channel::TryRecvError::Closed)
                if state_error == async_channel::TryRecvError::Closed =>
            {
                Err(async_channel::TryRecvError::Closed)
            }
            Err(_) => Err(async_channel::TryRecvError::Empty),
        }
    }

    async fn recv_state(&self) -> Result<VideoControl, async_channel::RecvError> {
        loop {
            match self.state.recv().await? {
                VideoControlMessage::State(state) => return Ok(state),
                VideoControlMessage::ParticipantOrientationsReady => {
                    if let Some(orientation) = self.take_participant_orientation() {
                        return Ok(orientation);
                    }
                }
                VideoControlMessage::PeerKeyframeRequested => {
                    if let Some(request) = self.take_peer_keyframe_request() {
                        return Ok(request);
                    }
                }
            }
        }
    }

    /// Wait for a state or orientation until every sender is gone.
    pub async fn recv(&self) -> Result<VideoControl, async_channel::RecvError> {
        loop {
            match self.try_recv() {
                Ok(control) => return Ok(control),
                Err(async_channel::TryRecvError::Closed) => return self.recv_state().await,
                Err(async_channel::TryRecvError::Empty) => {}
            }

            match (self.state.is_closed(), self.orientation.is_closed()) {
                (false, true) => return self.recv_state().await,
                (true, false) => {
                    return self
                        .orientation
                        .recv()
                        .await
                        .map(VideoControl::SetOrientation);
                }
                (true, true) => return self.recv_state().await,
                (false, false) => {
                    let state = self.state.recv().fuse();
                    let orientation = self.orientation.recv().fuse();
                    futures::pin_mut!(state, orientation);
                    futures::select_biased! {
                        state = state => match state {
                            Ok(VideoControlMessage::State(state)) => return Ok(state),
                            Ok(VideoControlMessage::ParticipantOrientationsReady) => {
                                if let Some(orientation) = self.take_participant_orientation() {
                                    return Ok(orientation);
                                }
                            }
                            Ok(VideoControlMessage::PeerKeyframeRequested) => {
                                if let Some(request) = self.take_peer_keyframe_request() {
                                    return Ok(request);
                                }
                            }
                            Err(_) => continue,
                        },
                        orientation = orientation => match orientation {
                            Ok(orientation) => return Ok(VideoControl::SetOrientation(orientation)),
                            Err(_) => continue,
                        },
                    }
                }
            }
        }
    }
}

/// What the caller learned from the callee's `<accept>`, as one message.
///
/// Both facts land at the same instant and both must be applied before the first inbound packet, so
/// they travel together rather than racing down two channels. The callee needs no equivalent: its
/// peer's capability arrives in the `<offer>`, before the engine exists, so it simply starts with
/// the right format.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeerAnswer {
    /// The answering device's LID. Recv keys are re-derived from it.
    pub answering_lid: String,
    /// The audio codec the peer's capability selects, or `None` when it announced nothing and the
    /// negotiated choice stands.
    pub audio_codec: Option<MediaAudioCodec>,
}

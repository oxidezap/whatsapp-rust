//! Signaling/control call state, independent of the media engine.
//!
//! These types describe a call's identity, direction and lifecycle. They used to live in the
//! `voip`-gated session module, so the control plane could not name them without the engine. They
//! carry no engine type now: the one piece of media state a call needs at signaling time is its
//! negotiated audio format, and that is the neutral [`MediaAudioFormat`]. The `voip` session module
//! re-exports every one of these, so the historical paths keep resolving.

use wacore_binary::Jid;

use crate::types::group_call::GroupCallUpdate;
use crate::voip_control::MediaAudioFormat;

/// Call direction, kept neutral so signaling compiles without the engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CallDirection {
    Outgoing,
    Incoming,
}

/// Lifecycle phase of a call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CallPhase {
    Idle,
    Calling,
    Ringing,
    /// A call-link join is alive but still awaiting administrator admission.
    WaitingRoom,
    Connecting,
    Active,
    Ended,
}

/// Per-call signaling state. Transitions are validated so an out-of-order server message
/// can't silently advance a torn-down call.
#[derive(Debug, Clone)]
pub struct CallSession {
    pub call_id: String,
    pub peer_jid: Jid,
    pub call_creator: Jid,
    pub direction: CallDirection,
    pub is_video: bool,
    /// The single media profile selected for this call, as the neutral flat format.
    pub audio_format: Option<MediaAudioFormat>,
    /// For an OUTGOING call: the callee device JIDs the offer rang, so when one accepts/rejects the
    /// caller can dismiss the rest (`accepted_elsewhere`). Empty for incoming calls and single-device
    /// callees. Lives on the session so it is dropped automatically whenever the call deregisters --
    /// no separate per-call map to clean up across the many call-end paths.
    pub ring_devices: Vec<Jid>,
    /// For an OUTGOING call: the callee device (`call.from` of the inbound `<accept>`) that actually
    /// answered, learned after the offer rang the bare LID. Call signaling other than the offer is
    /// addressed per device (WA Web `WAWebVoipSendSignalingXmpp` coerces the peer to a device JID), so
    /// a `<terminate>` must target this device, not the bare peer, or it can miss the companion that
    /// answered. `None` until the first `<accept>`; set-once (first answerer wins, like the rekey).
    pub answering_device: Option<Jid>,
    /// Initial group snapshot for a native group call or active-call invitation.
    pub group: Option<GroupCallUpdate>,
    /// The rotation the peer announced on the `<offer>`'s `<video>` child, with
    /// the device that announced it -- a group call stamps rotation per sending
    /// device, so the bare user JID would hand the same rotation to every
    /// sibling. Rides the session for the same reason [`Self::is_video`] does:
    /// both are facts the offer states and the engine needs. What happens to it
    /// afterwards is `CallEntry::peer_video_orientations`, which holds one such
    /// pair per announcer.
    pub peer_video_orientation: Option<(Jid, u8)>,
    phase: CallPhase,
}

impl CallSession {
    pub fn new_outgoing(call_id: impl Into<String>, peer_jid: Jid, call_creator: Jid) -> Self {
        Self {
            call_id: call_id.into(),
            peer_jid,
            call_creator,
            direction: CallDirection::Outgoing,
            is_video: false,
            audio_format: None,
            ring_devices: Vec::new(),
            answering_device: None,
            group: None,
            peer_video_orientation: None,
            phase: CallPhase::Idle,
        }
    }

    pub fn new_incoming(call_id: impl Into<String>, peer_jid: Jid, call_creator: Jid) -> Self {
        Self {
            call_id: call_id.into(),
            peer_jid,
            call_creator,
            direction: CallDirection::Incoming,
            is_video: false,
            audio_format: None,
            ring_devices: Vec::new(),
            answering_device: None,
            group: None,
            peer_video_orientation: None,
            phase: CallPhase::Ringing,
        }
    }

    pub fn phase(&self) -> CallPhase {
        self.phase
    }

    pub fn is_active(&self) -> bool {
        self.phase == CallPhase::Active
    }

    pub fn is_ended(&self) -> bool {
        self.phase == CallPhase::Ended
    }

    /// Attempt a phase transition; returns false (no-op) if it is not legal from the current phase.
    ///
    /// The lifecycle order is `Idle → Calling → Ringing/WaitingRoom → Connecting → Active`.
    /// Forward progress is allowed and MAY skip intermediate phases: an accepted outgoing call
    /// commonly goes `Calling → Connecting` with no observed `Ringing`, and an immediate accept can
    /// reach `Active` directly. Backward moves are rejected. `Idle` leaves only to `Calling`
    /// (outgoing) or `Ended`. `Ended` is a sink reachable from any live phase (`Ended → Ended` is a
    /// no-op `false`).
    /// Self-transitions on a live phase are idempotent.
    pub fn transition_to(&mut self, next: CallPhase) -> bool {
        use CallPhase::*;
        let ok = match (self.phase, next) {
            (Ended, _) => false,
            (_, Ended) => true,
            (a, b) if a == b => true,
            (Idle, Calling) => self.direction == CallDirection::Outgoing,
            (Idle, _) => false,
            (from, to) => phase_rank(to) > phase_rank(from),
        };
        if ok {
            self.phase = next;
        }
        ok
    }
}

impl crate::stats::HeapSize for CallSession {
    fn heap_bytes(&self) -> usize {
        use core::mem::size_of;

        use crate::stats::HeapSize;

        self.call_id.heap_bytes()
            + self.peer_jid.heap_bytes()
            + self.call_creator.heap_bytes()
            + self.ring_devices.capacity() * size_of::<Jid>()
            + self
                .ring_devices
                .iter()
                .map(HeapSize::heap_bytes)
                .sum::<usize>()
            + self
                .answering_device
                .as_ref()
                .map_or(0, HeapSize::heap_bytes)
            + self
                .peer_video_orientation
                .as_ref()
                .map_or(0, |(announcer, _)| announcer.heap_bytes())
            + self.group.as_ref().map_or(0, HeapSize::heap_bytes)
    }
}

/// Lifecycle ordinal for the forward-progress check in [`CallSession::transition_to`] (higher =
/// later in the call). `Ended` is handled separately, so its rank is never compared.
fn phase_rank(p: CallPhase) -> u8 {
    match p {
        CallPhase::Idle => 0,
        CallPhase::Calling => 1,
        CallPhase::Ringing => 2,
        CallPhase::WaitingRoom => 2,
        CallPhase::Connecting => 3,
        CallPhase::Active => 4,
        CallPhase::Ended => 5,
    }
}

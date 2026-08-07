//! The portable async driver: one loop that drives a [`CallEngine`] over an injected
//! [`Runtime`] and [`RelayTransport`].
//! It owns no concrete socket, clock, or executor; native injects the Tokio runtime + the webrtc-rs
//! DataChannel, the WASM bridge injects its single-threaded runtime + a `node:dgram` transport. The
//! esp32 control plane does not use this (it has no relay/media).
//!
//! The loop is the str0m drive contract: wait for one input (a relay packet, a mic frame, or the
//! timer), apply it with `handle_input(now, ..)`, then drain `poll_output()` running each intent,
//! and arm the next timer from `poll_timeout()`. The monotonic clock is `crate::time::Instant`
//! (native `std::time::Instant`; wasm `performance.now`), so no wall clock leaks into the engine.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bytes::Bytes;
use futures::FutureExt;
use futures::future::{Fuse, FusedFuture};
use portable_atomic::{AtomicBool, AtomicUsize, Ordering};
use zeroize::Zeroize;

use crate::runtime::{BoxFuture, Runtime};
use crate::time::Instant;
use crate::types::group_call::{GROUP_CALL_MAX_PARTICIPANTS, GroupCallUpdate};
use crate::voip::audio::EncodedAudioFrame;
use crate::voip::demux::{RelayPacketKind, classify_relay_packet};
use crate::voip::engine::{self, CallEngine, CallEvent, Input, Output};
use crate::voip::group_media::GroupMediaError;
use crate::voip::h264::VideoFrame;
use crate::voip::rtp::{RTP_PAYLOAD_TYPE_H264, VIDEO_MEDIA_FRAME_INFO_IDR, parse_rtp_header};
use crate::voip::transport::{RelayTransport, RelayTransportEvent};

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

enum NormalizedGroupControl {
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
    /// Require the next outbound access unit to be an IDR frame after changing its source role.
    RequireKeyframe,
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
}

/// Receiving half of [`video_control_channel`].
pub struct VideoControlReceiver {
    state: async_channel::Receiver<VideoControlMessage>,
    orientation: async_channel::Receiver<u8>,
    participant_orientations: Arc<PendingParticipantOrientations>,
    ready_participant_orientations: Mutex<VecDeque<(wacore_binary::Jid, u8)>>,
    /// `ready_participant_orientations.len()`, same role as [`PendingParticipantOrientations::len`].
    ready_participant_orientations_len: AtomicUsize,
}

/// Build the control mailbox used by one call driver.
pub fn video_control_channel() -> (VideoControlSender, VideoControlReceiver) {
    let (state_tx, state_rx) = async_channel::unbounded();
    let (orientation_tx, orientation_rx) = async_channel::bounded(1);
    let participant_orientations = Arc::new(PendingParticipantOrientations::default());
    (
        VideoControlSender {
            state: state_tx,
            orientation: orientation_tx,
            participant_orientations: participant_orientations.clone(),
        },
        VideoControlReceiver {
            state: state_rx,
            orientation: orientation_rx,
            participant_orientations,
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
            .saturating_add(
                pending
                    .capacity()
                    .saturating_mul(size_of::<(wacore_binary::Jid, u8)>()),
            )
            .saturating_add(pending.keys().map(HeapSize::heap_bytes).sum::<usize>())
    }
}

impl VideoControlReceiver {
    /// Whether both halves have lost every sender.
    pub fn is_closed(&self) -> bool {
        self.state.is_closed() && self.orientation.is_closed()
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

/// The audio + video + event channels the driver bridges to the platform. PCM and encoded audio
/// channels are both present; the call's [`super::audio::AudioIo`] selects which pair is active.
/// Media outputs shed frames on sink overflow, while lifecycle events use their own bounded queue.
pub struct CallChannels {
    pub mic: async_channel::Receiver<Vec<i16>>,
    pub speaker: async_channel::Sender<Vec<i16>>,
    pub encoded_audio_in: async_channel::Receiver<Bytes>,
    pub encoded_audio_out: async_channel::Sender<EncodedAudioFrame>,
    pub events: async_channel::Sender<CallEvent>,
    /// Caller-only: the answering device's LID, delivered once the callee's `<accept>` is received so
    /// the drive loop can rekey the recv path before media flows. `None` on the callee side and esp32.
    pub rekey: Option<async_channel::Receiver<String>>,
    /// Outbound video: one pre-encoded H.264 Annex-B access unit per item.
    pub video_in: async_channel::Receiver<Vec<u8>>,
    /// Inbound video: reassembled peer access units (dropped on sink overflow, like the speaker).
    pub video_out: async_channel::Sender<VideoFrame>,
    /// Mid-call video-plane control (lossless state, coalesced orientation).
    pub video_ctl: VideoControlReceiver,
    /// Group roster and decrypted epoch transitions. `None` for a 1:1 call.
    pub group_ctl: Option<async_channel::Receiver<GroupControl>>,
}

/// Bound slow relay writes without truncating a complete video access unit.
const SEND_QUEUE_BATCH_CAP: usize = 64;
const SEND_QUEUE_BYTE_CAP: usize = 2 * 1024 * 1024;
const RELAY_RECONNECT_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SendBatchKind {
    Control,
    Media,
    Video,
}

struct SendBatch {
    packets: VecDeque<Bytes>,
    bytes: usize,
    kind: SendBatchKind,
    started: bool,
    video_keyframe: bool,
}

impl SendBatch {
    fn packet(data: Bytes) -> Self {
        let kind = if classify_relay_packet(&data) == RelayPacketKind::Stun {
            SendBatchKind::Control
        } else {
            SendBatchKind::Media
        };
        Self {
            bytes: data.len(),
            packets: VecDeque::from([data]),
            kind,
            started: false,
            video_keyframe: false,
        }
    }

    fn video(packets: Vec<Bytes>) -> Self {
        let video_keyframe = packets
            .first()
            .and_then(|packet| parse_rtp_header(packet))
            .and_then(|header| header.video_extension)
            .is_some_and(|extension| extension.media_frame_info == VIDEO_MEDIA_FRAME_INFO_IDR);
        Self {
            bytes: packets.iter().map(Bytes::len).sum(),
            packets: packets.into(),
            kind: SendBatchKind::Video,
            started: false,
            video_keyframe,
        }
    }
}

#[derive(Default)]
struct DroppedMedia {
    video_access_units: u32,
    packets: u32,
}

type RelaySendFuture = Fuse<BoxFuture<'static, anyhow::Result<()>>>;

/// The drive loop's engine-deadline timer. `Runtime::sleep` hands back an owned `'static` future, so
/// the armed sleep outlives the iteration that armed it.
type DeadlineTimer = Fuse<BoxFuture<'static, ()>>;

struct InFlightSend {
    future: RelaySendFuture,
    kind: Option<SendBatchKind>,
}

impl Default for InFlightSend {
    fn default() -> Self {
        Self {
            future: Fuse::terminated(),
            kind: None,
        }
    }
}

fn cancel_in_flight_group_media(
    sending: &mut InFlightSend,
    epoch_advanced: bool,
    audio_only: bool,
) -> DroppedMedia {
    let Some(kind) = sending.kind else {
        return DroppedMedia::default();
    };
    let cancel = (epoch_advanced && kind != SendBatchKind::Control)
        || (audio_only && kind == SendBatchKind::Video);
    if !cancel {
        return DroppedMedia::default();
    }
    // Dropping the owned send future cancels a relay write that is still pending. Its packet was
    // already removed from `send_queue`, so queue purging alone cannot retire this old-key media.
    *sending = InFlightSend::default();
    DroppedMedia {
        video_access_units: u32::from(kind == SendBatchKind::Video),
        packets: 1,
    }
}

fn record_drop(dropped: &mut DroppedMedia, batch: &SendBatch) {
    dropped.packets = dropped
        .packets
        .saturating_add(batch.packets.len().try_into().unwrap_or(u32::MAX));
    if batch.kind == SendBatchKind::Video {
        dropped.video_access_units = dropped.video_access_units.saturating_add(1);
    }
}

fn purge_unstarted_video(
    queue: &mut VecDeque<SendBatch>,
    awaiting_video_keyframe: &mut bool,
) -> DroppedMedia {
    let mut dropped = DroppedMedia::default();
    queue.retain(|batch| {
        let discard = !batch.started && batch.kind == SendBatchKind::Video;
        if discard {
            record_drop(&mut dropped, batch);
        }
        !discard
    });
    if dropped.video_access_units != 0 {
        *awaiting_video_keyframe = true;
    }
    dropped
}

fn purge_queued(
    queue: &mut VecDeque<SendBatch>,
    pending_video: &mut Vec<Bytes>,
    awaiting_video_keyframe: &mut bool,
    discard: impl Fn(&SendBatch) -> bool,
) -> DroppedMedia {
    let mut dropped = DroppedMedia::default();
    queue.retain(|batch| {
        let drop_batch = discard(batch);
        if drop_batch {
            record_drop(&mut dropped, batch);
        }
        !drop_batch
    });
    if !pending_video.is_empty() {
        dropped.video_access_units = dropped.video_access_units.saturating_add(1);
        dropped.packets = dropped
            .packets
            .saturating_add(pending_video.len().try_into().unwrap_or(u32::MAX));
        pending_video.clear();
    }
    if dropped.video_access_units != 0 {
        *awaiting_video_keyframe = true;
    }
    dropped
}

fn purge_group_transition_media(
    queue: &mut VecDeque<SendBatch>,
    pending_video: &mut Vec<Bytes>,
    awaiting_video_keyframe: &mut bool,
    epoch_advanced: bool,
    audio_only: bool,
) -> DroppedMedia {
    if !epoch_advanced && !audio_only {
        return DroppedMedia::default();
    }
    purge_queued(queue, pending_video, awaiting_video_keyframe, |batch| {
        (epoch_advanced && batch.kind != SendBatchKind::Control)
            || (audio_only && batch.kind == SendBatchKind::Video)
    })
}

fn apply_group_epoch_control(
    engine: &mut CallEngine,
    epoch: GroupRawEpoch,
    send_queue: &mut VecDeque<SendBatch>,
    pending_video: &mut Vec<Bytes>,
    awaiting_video_keyframe: &mut bool,
    sending: &mut InFlightSend,
    events: &async_channel::Sender<CallEvent>,
) {
    match engine.apply_group_raw_epoch(epoch.transaction_id, epoch.as_bytes()) {
        Ok(crate::voip::GroupEpochApply::Installed) => {
            let mut dropped = purge_queued(
                send_queue,
                pending_video,
                awaiting_video_keyframe,
                |batch| batch.kind != SendBatchKind::Control,
            );
            let in_flight = cancel_in_flight_group_media(sending, true, false);
            dropped.video_access_units = dropped
                .video_access_units
                .saturating_add(in_flight.video_access_units);
            dropped.packets = dropped.packets.saturating_add(in_flight.packets);
            if dropped.packets != 0 {
                let _ = events.try_send(CallEvent::OutboundMediaDropped {
                    video_access_units: dropped.video_access_units,
                    packets: dropped.packets,
                });
            }
        }
        Ok(_) => {}
        Err(_) => {
            // A decrypted epoch may overtake initial roster configuration. Rejecting the control
            // keeps the driver alive; registry-originated epochs are paired with their roster and
            // therefore retry through the transition path instead of relying on this fallback.
            let _ = events.try_send(CallEvent::GroupControlRejected {
                control: engine::GroupControlKind::Epoch,
            });
        }
    }
}

fn publish_engine_event(events: &async_channel::Sender<CallEvent>, event: CallEvent) {
    if matches!(
        &event,
        CallEvent::RelayAllocated
            | CallEvent::RelayAllocateFailed(_)
            | CallEvent::RelayAllocateTimedOut
            | CallEvent::RelayReconnectTimedOut
    ) {
        let _ = events.force_send(event);
    } else {
        let _ = events.try_send(event);
    }
}

async fn disconnect_relay_bounded(rt: &dyn Runtime, transport: &dyn RelayTransport) {
    let disconnect = transport.disconnect().fuse();
    let timeout = rt.sleep(RELAY_RECONNECT_TIMEOUT).fuse();
    futures::pin_mut!(disconnect, timeout);
    futures::select_biased! {
        () = disconnect => {},
        () = timeout => {},
    }
}

fn is_fatal_group_update_error(error: &engine::EngineError) -> bool {
    matches!(
        error,
        engine::EngineError::GroupMedia(GroupMediaError::LocalParticipantRemoved)
    ) || !matches!(error, engine::EngineError::GroupMedia(_))
}

fn prepare_relay_reconnect(
    queue: &mut VecDeque<SendBatch>,
    pending_video: &mut Vec<Bytes>,
    awaiting_video_keyframe: &mut bool,
) {
    queue.clear();
    pending_video.clear();
    // Discarding access units leaves remote decoders with a hole. Reopen the replacement path on
    // an IDR rather than forwarding a delta that references frames lost with the retired relay.
    *awaiting_video_keyframe = true;
}

fn discard_video_until_keyframe(
    queue: &mut VecDeque<SendBatch>,
    dropped: &mut DroppedMedia,
) -> bool {
    loop {
        let Some(index) = queue
            .iter()
            .position(|batch| !batch.started && batch.kind == SendBatchKind::Video)
        else {
            return true;
        };
        if queue[index].video_keyframe {
            return false;
        }
        if let Some(batch) = queue.remove(index) {
            record_drop(dropped, &batch);
        }
    }
}

fn shed_to_cap(
    queue: &mut VecDeque<SendBatch>,
    awaiting_video_keyframe: &mut bool,
) -> DroppedMedia {
    let mut dropped = DroppedMedia::default();
    loop {
        let bytes: usize = queue.iter().map(|batch| batch.bytes).sum();
        if queue.len() <= SEND_QUEUE_BATCH_CAP && bytes <= SEND_QUEUE_BYTE_CAP {
            break;
        }
        // Never cut an AU after one of its fragments has entered the transport.
        let victim = queue
            .iter()
            .position(|batch| !batch.started && batch.kind == SendBatchKind::Video)
            .or_else(|| {
                queue
                    .iter()
                    .position(|batch| !batch.started && batch.kind == SendBatchKind::Media)
            })
            .or_else(|| queue.iter().position(|batch| !batch.started));
        let Some(victim) = victim else {
            break;
        };
        let Some(batch) = queue.remove(victim) else {
            break;
        };
        let dropped_video = batch.kind == SendBatchKind::Video;
        record_drop(&mut dropped, &batch);
        if dropped_video {
            *awaiting_video_keyframe = discard_video_until_keyframe(queue, &mut dropped);
        }
    }
    dropped
}

fn enqueue_batch(
    queue: &mut VecDeque<SendBatch>,
    awaiting_video_keyframe: &mut bool,
    batch: SendBatch,
) -> DroppedMedia {
    if batch.kind == SendBatchKind::Video && *awaiting_video_keyframe {
        if !batch.video_keyframe {
            let mut dropped = DroppedMedia::default();
            record_drop(&mut dropped, &batch);
            return dropped;
        }
        *awaiting_video_keyframe = false;
    }
    queue.push_back(batch);
    shed_to_cap(queue, awaiting_video_keyframe)
}

/// Coalesce every PT-97 packet through the marker into one queue unit, so backpressure keeps or
/// drops a complete H.264 access unit instead of silently truncating an IDR.
fn queue_transmit(
    queue: &mut VecDeque<SendBatch>,
    pending_video: &mut Vec<Bytes>,
    awaiting_video_keyframe: &mut bool,
    data: Bytes,
) -> DroppedMedia {
    if let Some(header) = parse_rtp_header(&data)
        && header.payload_type == RTP_PAYLOAD_TYPE_H264
    {
        pending_video.push(data);
        if header.marker {
            return enqueue_batch(
                queue,
                awaiting_video_keyframe,
                SendBatch::video(std::mem::take(pending_video)),
            );
        }
        return DroppedMedia::default();
    }
    let mut dropped = DroppedMedia::default();
    if !pending_video.is_empty() {
        dropped = enqueue_batch(
            queue,
            awaiting_video_keyframe,
            SendBatch::video(std::mem::take(pending_video)),
        );
    }
    let more = enqueue_batch(queue, awaiting_video_keyframe, SendBatch::packet(data));
    dropped.video_access_units = dropped
        .video_access_units
        .saturating_add(more.video_access_units);
    dropped.packets = dropped.packets.saturating_add(more.packets);
    dropped
}

fn pop_next_packet(queue: &mut VecDeque<SendBatch>) -> Option<(Bytes, SendBatchKind)> {
    let index = queue
        .iter()
        .position(|batch| batch.kind == SendBatchKind::Control)
        .unwrap_or(0);
    let batch = queue.get_mut(index)?;
    batch.started = true;
    let kind = batch.kind;
    let packet = batch.packets.pop_front()?;
    batch.bytes = batch.bytes.saturating_sub(packet.len());
    if batch.packets.is_empty() {
        queue.remove(index);
    }
    Some((packet, kind))
}

/// Drive one call to completion: returns when the relay channel disconnects, a send fails, or the
/// relay-event stream closes. On exit it calls `transport.disconnect()` so the platform's relay read
/// pump is released rather than left parked in `recv()`. The caller spawns this on its runtime and
/// stores the [`AbortHandle`](crate::runtime::AbortHandle) (e.g. in a
/// [`CallRegistry`](super::registry::CallRegistry)) to tear the call down; aborting the task drops
/// this future, which drops the transport `Arc` and closes the channel as well.
pub async fn run_call(
    rt: Arc<dyn Runtime>,
    transport: Arc<dyn RelayTransport>,
    relay_events: async_channel::Receiver<RelayTransportEvent>,
    channels: CallChannels,
    eng: CallEngine,
) {
    let epoch = Instant::now();
    let wallclock_ms = crate::time::now_millis().max(0) as u64;
    run_call_with_clock_and_wallclock(
        rt,
        transport,
        relay_events,
        channels,
        eng,
        move || epoch.elapsed().as_millis() as u64,
        wallclock_ms,
    )
    .await;
}

/// [`run_call`] with an injectable monotonic clock, so tests can drive the keepalive/playout timers
/// deterministically without real-time sleeps. Native calls use [`crate::time::Instant`].
// Lifecycle span over the whole drive. The large channels/transport/clock args are skipped; only the
// non-PII call_id is recorded.
#[cfg_attr(
    feature = "tracing",
    tracing::instrument(
        name = "wa.voip.run_call",
        level = "debug",
        skip_all,
        fields(call_id = %eng.call_id())
    )
)]
#[cfg(all(test, feature = "voip-mlow"))]
async fn run_call_with_clock(
    rt: Arc<dyn Runtime>,
    transport: Arc<dyn RelayTransport>,
    relay_events: async_channel::Receiver<RelayTransportEvent>,
    channels: CallChannels,
    eng: CallEngine,
    now_ms: impl Fn() -> engine::Millis,
) {
    run_call_with_clock_and_wallclock(
        rt,
        transport,
        relay_events,
        channels,
        eng,
        now_ms,
        1_700_000_000_000,
    )
    .await;
}

async fn run_call_with_clock_and_wallclock(
    rt: Arc<dyn Runtime>,
    mut transport: Arc<dyn RelayTransport>,
    mut relay_events: async_channel::Receiver<RelayTransportEvent>,
    channels: CallChannels,
    mut eng: CallEngine,
    now_ms: impl Fn() -> engine::Millis,
    wallclock_ms: u64,
) {
    eng.start(now_ms(), wallclock_ms);

    #[cfg(feature = "tracing")]
    let call_id = eng.call_id().to_string();

    // The mic feeds outgoing audio only; its liveness must not gate the call. Flipped false when the
    // mic channel closes so we stop polling it without tearing the call down (see the mic arm).
    let mut mic_open = true;
    let mut encoded_audio_open = true;

    // The recv-rekey is one-shot (the first callee `<accept>` picks the answering device). Flipped
    // false after the first event -- a rekey or the sender closing -- so the closed channel's
    // always-ready `Err` doesn't busy-spin the select (the mic arm has the same guard).
    let mut rekey_open = true;
    let mut group_ctl_open = true;

    // Same closed-channel guards for the video arms: a call that never wires a video source/control
    // sender must not busy-spin on their always-ready `Err`.
    let mut video_in_open = true;
    let mut video_ctl_open = true;
    // Set by a `Disable` to drain the video input queue after the select block (it can't be drained
    // inside, where the arm futures borrow the channel).
    let mut drain_video_in = false;

    // Decouple sending from receive/playout. Outbound packets are queued and the in-flight send is
    // polled as one arm of the select, CONCURRENTLY with the relay/mic/timer arms -- so a slow or
    // stalled relay write (SCTP cwnd, a congested link, a loaded host) is simply pending here and never
    // freezes the inbound jitter buffer or the playout tick. Awaiting `transport.send()` inline coupled
    // the two: a slow send parked the loop, inbound packets queued then arrived in a burst, and the
    // playout tick fired late -- so the jitter buffer overflowed then underran (audible glitching,
    // worst whatsapp-rust<->whatsapp-rust where both ends stalled; the official client decouples them).
    // Video is queued per access unit so overload never leaves half an IDR on the wire.
    let mut send_queue: VecDeque<SendBatch> = VecDeque::new();
    let mut awaiting_video_keyframe = false;
    // Idle sentinel: a terminated `Fuse` is safe to re-select every iteration and never fires until a
    // real send replaces it; on completion it terminates itself, so no manual reset / re-poll hazard.
    // `BoxFuture` is `Send` natively but `?Send` on wasm (the transport is single-threaded there).
    let mut sending = InFlightSend::default();

    let mut timer: DeadlineTimer = Fuse::terminated();
    let mut armed_deadline: Option<engine::Millis> = None;

    'drive: loop {
        // Drain every intent the last mutation produced; stop at the terminal Timeout.
        let mut pending_video = Vec::new();
        let mut reconnect_to = None;
        loop {
            match eng.poll_output() {
                // Queue for the in-flight send arm; never await the write in this loop.
                Output::Transmit(data) => {
                    let dropped = queue_transmit(
                        &mut send_queue,
                        &mut pending_video,
                        &mut awaiting_video_keyframe,
                        data,
                    );
                    if dropped.packets != 0 {
                        let _ = channels.events.try_send(CallEvent::OutboundMediaDropped {
                            video_access_units: dropped.video_access_units,
                            packets: dropped.packets,
                        });
                    }
                }
                // Loss tolerant: drop the frame if the speaker can't keep up.
                Output::Playout(pcm) => {
                    let _ = channels.speaker.try_send(pcm);
                }
                Output::EncodedAudio(frame) => {
                    let _ = channels.encoded_audio_out.try_send(frame);
                }
                // Same policy for video: a stalled sink sheds frames, never the drive loop.
                Output::VideoPlayout(frame) => {
                    let _ = channels.video_out.try_send(frame);
                }
                Output::Event(ev) => {
                    publish_engine_event(&channels.events, ev);
                }
                Output::ReconnectRelay(endpoint) => {
                    // Anything queued before this intent targets the retired relay. Later outputs in
                    // the same drain include the fresh Allocate and are retained for the new channel.
                    prepare_relay_reconnect(
                        &mut send_queue,
                        &mut pending_video,
                        &mut awaiting_video_keyframe,
                    );
                    reconnect_to = Some(endpoint);
                }
                Output::Timeout(_) => {
                    if !pending_video.is_empty() {
                        let dropped = enqueue_batch(
                            &mut send_queue,
                            &mut awaiting_video_keyframe,
                            SendBatch::video(std::mem::take(&mut pending_video)),
                        );
                        if dropped.packets != 0 {
                            let _ = channels.events.try_send(CallEvent::OutboundMediaDropped {
                                video_access_units: dropped.video_access_units,
                                packets: dropped.packets,
                            });
                        }
                    }
                    break;
                }
            }
        }

        // The terminal event above must reach the consumer before transport teardown.
        if eng.is_terminated() {
            break 'drive;
        }

        if let Some(endpoint) = reconnect_to {
            // An in-flight write belongs to the retired channel and is delivery-ambiguous. Drop it;
            // the fresh Allocate emitted after ReconnectRelay establishes the replacement path.
            sending = InFlightSend::default();
            let current_transport = transport.clone();
            let reconnect = current_transport.reconnect(endpoint).fuse();
            let timeout = rt.sleep(RELAY_RECONNECT_TIMEOUT).fuse();
            futures::pin_mut!(reconnect, timeout);
            let reconnect_result = futures::select_biased! {
                result = reconnect => result,
                () = timeout => {
                    publish_engine_event(&channels.events, CallEvent::RelayReconnectTimedOut);
                    break 'drive;
                },
            };
            let Ok((replacement, replacement_events)) = reconnect_result else {
                break 'drive;
            };
            let retired = std::mem::replace(&mut transport, replacement);
            relay_events = replacement_events;
            disconnect_relay_bounded(&*rt, &*retired).await;
            eng.relay_reconnected(now_ms());
            // The reconnect restarts every deadline from `now`, so an armed sleep still points at the
            // retired relay's schedule. Drop it and let the arming step below build the new one.
            timer = Fuse::terminated();
            armed_deadline = None;
        }

        // Start the next queued send when none is in flight. The future owns an Arc clone, so it is
        // `'static` (no borrow of the loop's `transport`).
        if sending.future.is_terminated()
            && let Some((data, kind)) = pop_next_packet(&mut send_queue)
        {
            let t = transport.clone();
            let fut: BoxFuture<'static, anyhow::Result<()>> =
                Box::pin(async move { t.send(data).await });
            sending.future = fut.fuse();
            sending.kind = Some(kind);
        }

        // Deadlines are absolute, so a sleep already armed for this one still expires at the right
        // instant and is reused rather than rebuilt (a rebuild is a boxed future plus a
        // register/deregister in the runtime's timer wheel). Rearm when the engine moved the deadline,
        // and after a fire: a spent `Fuse` never resolves again, which would leave the call with no
        // keepalive and no playout.
        let deadline = eng.poll_timeout().filter(|at| *at != engine::NEVER);
        if deadline != armed_deadline || (deadline.is_some() && timer.is_terminated()) {
            timer = match deadline {
                Some(at) => rt
                    .sleep(Duration::from_millis(at.saturating_sub(now_ms())))
                    .fuse(),
                None => Fuse::terminated(),
            };
            armed_deadline = deadline;
        }

        // Poll the mic only while its channel is open. A closed mic must NOT end the call: OS mute can
        // make the mic source (e.g. `pw-record`) EOF, closing this channel, and the keepalive + playout
        // have to keep running or the relay drops us after ~4s of no traffic and the peer reconnects.
        // On close we disable the arm (a closed async_channel is always-ready `Err`, which would
        // otherwise busy-spin the select and starve the timer) and keep driving with a pending mic.
        let mic = &channels.mic;
        let mic_fut = async move {
            if mic_open {
                mic.recv().await
            } else {
                std::future::pending().await
            }
        }
        .fuse();
        futures::pin_mut!(mic_fut);

        let encoded_audio = &channels.encoded_audio_in;
        let encoded_audio_fut = async move {
            if encoded_audio_open {
                encoded_audio.recv().await
            } else {
                std::future::pending().await
            }
        }
        .fuse();
        futures::pin_mut!(encoded_audio_fut);

        // Caller-only recv-rekey: the answering device's LID. Parked (pending) when there is no rekey
        // channel (callee/esp32) or once consumed, mirroring the mic arm so a closed channel can't spin.
        let rekey_live = rekey_open && channels.rekey.is_some();
        let rekey_ch = channels.rekey.as_ref();
        let rekey_fut = async move {
            if rekey_live {
                rekey_ch.expect("rekey_live implies Some").recv().await.ok()
            } else {
                std::future::pending().await
            }
        }
        .fuse();
        futures::pin_mut!(rekey_fut);

        let group_ctl_live = group_ctl_open && channels.group_ctl.is_some();
        let group_ctl_ch = channels.group_ctl.as_ref();
        let group_ctl_fut = async move {
            if group_ctl_live {
                group_ctl_ch
                    .expect("group_ctl_live implies Some")
                    .recv()
                    .await
                    .ok()
            } else {
                std::future::pending().await
            }
        }
        .fuse();
        futures::pin_mut!(group_ctl_fut);

        // Video source and control arms, guarded like the mic: a closed channel disables the arm
        // (a video source EOF must not end the call — audio keeps running after a downgrade).
        let video_in = &channels.video_in;
        let video_in_fut = async move {
            if video_in_open {
                video_in.recv().await
            } else {
                std::future::pending().await
            }
        }
        .fuse();
        futures::pin_mut!(video_in_fut);

        let video_ctl = &channels.video_ctl;
        let video_ctl_fut = async move {
            if video_ctl_open {
                video_ctl.recv().await
            } else {
                std::future::pending().await
            }
        }
        .fuse();
        futures::pin_mut!(video_ctl_fut);

        // Wait for exactly one input, then apply it. A dropped (unready) recv future loses nothing:
        // async_channel only dequeues on a ready poll.
        // Biased: the in-flight send first so it always makes progress (drain the queue, surface a send
        // failure) -- a slow send is pending here and yields to the arms below, so it can't stall them.
        // Then the recv-rekey BEFORE relay: if the answering device's LID and its first media packet are
        // both ready, apply the rekey first so that packet decrypts under the right keys (no startup
        // garbage frame). Then relay before the timer: drain a ready inbound packet into the jitter buffer
        // BEFORE the playout tick (no phase-slip underrun), firing an overdue timer in-line so a relay/mic
        // flood can't starve the keepalive/playout.
        futures::select_biased! {
            // The in-flight send completed. A failure tears the call down (the old inline behavior).
            res = &mut sending.future => {
                sending.kind = None;
                if res.is_err() {
                    break 'drive;
                }
            },
            // Rekey recv to the device that answered, before its media reaches the relay arm below.
            lid = rekey_fut => {
                rekey_open = false; // one-shot: a LID or the sender closing both disable the arm
                if let Some(lid) = lid
                    && !eng.rekey_recv(&lid)
                {
                    break 'drive; // malformed stored call_key (a setup invariant violated)
                }
            },
            group = group_ctl_fut => {
                match group.map(NormalizedGroupControl::from) {
                    Some(NormalizedGroupControl::Update {
                        update,
                        paired_epoch,
                    }) => {
                        let previous_epoch = eng.group_epoch_transaction();
                        let update_accepted = match eng.apply_group_update(now_ms(), &update) {
                            Ok(crate::voip::GroupRosterApply::Applied) => {
                                let epoch_advanced = update.rekey_requested
                                    || eng.group_epoch_transaction() != previous_epoch;
                                let mut dropped = purge_group_transition_media(
                                    &mut send_queue,
                                    &mut pending_video,
                                    &mut awaiting_video_keyframe,
                                    epoch_advanced,
                                    update.media == "audio",
                                );
                                let in_flight = cancel_in_flight_group_media(
                                    &mut sending,
                                    epoch_advanced,
                                    update.media == "audio",
                                );
                                dropped.video_access_units = dropped
                                    .video_access_units
                                    .saturating_add(in_flight.video_access_units);
                                dropped.packets =
                                    dropped.packets.saturating_add(in_flight.packets);
                                if dropped.packets != 0 {
                                    let _ = channels.events.try_send(
                                        CallEvent::OutboundMediaDropped {
                                            video_access_units: dropped.video_access_units,
                                            packets: dropped.packets,
                                        },
                                    );
                                }
                                true
                            }
                            Ok(_) => true,
                            Err(error) => {
                                if is_fatal_group_update_error(&error) {
                                    break 'drive;
                                }
                                let _ = channels.events.try_send(
                                    CallEvent::GroupControlRejected {
                                        control: engine::GroupControlKind::Update,
                                    },
                                );
                                false
                            }
                        };
                        if update_accepted
                            && let Some(epoch) = paired_epoch
                        {
                            apply_group_epoch_control(
                                &mut eng,
                                epoch,
                                &mut send_queue,
                                &mut pending_video,
                                &mut awaiting_video_keyframe,
                                &mut sending,
                                &channels.events,
                            );
                        } else if paired_epoch.is_some() {
                            // A transition is indivisible: rejecting its roster also discards the
                            // paired key, so surface both halves instead of hiding the lost epoch.
                            let _ = channels.events.try_send(CallEvent::GroupControlRejected {
                                control: engine::GroupControlKind::Epoch,
                            });
                        }
                    }
                    Some(NormalizedGroupControl::RawEpoch(epoch)) => {
                        apply_group_epoch_control(
                            &mut eng,
                            epoch,
                            &mut send_queue,
                            &mut pending_video,
                            &mut awaiting_video_keyframe,
                            &mut sending,
                            &channels.events,
                        );
                    }
                    Some(NormalizedGroupControl::Reaction(emoji)) => {
                        if eng.send_group_reaction(now_ms(), &emoji).is_err() {
                            let _ = channels.events.try_send(CallEvent::GroupControlRejected {
                                control: engine::GroupControlKind::Reaction,
                            });
                        }
                    }
                    None => group_ctl_open = false,
                }
                let now = now_ms();
                if let Some(at) = eng.poll_timeout()
                    && at != engine::NEVER
                    && now >= at
                {
                    eng.handle_input(now, Input::Timeout);
                }
            },
            // State control before orientation and media, so a Disable always purges before a later
            // Enable can admit frames from the replacement source.
            ctl = video_ctl_fut => {
                match ctl {
                    Ok(VideoControl::SetTimestampStride(ts_stride)) => {
                        let _ = eng.set_video_timestamp_stride(ts_stride);
                    }
                    Ok(VideoControl::Enable) => {
                        // False only for a control-only engine or a malformed stored callKey; the
                        // audio plane already validated the key, so treat it as a no-op not fatal.
                        let _ = eng.enable_video();
                    }
                    Ok(VideoControl::EnableAwaitingAccept) => {
                        let _ = eng.enable_video_gated();
                    }
                    Ok(VideoControl::Disable) => {
                        eng.disable_video();
                        let dropped = purge_unstarted_video(
                            &mut send_queue,
                            &mut awaiting_video_keyframe,
                        );
                        if dropped.packets != 0 {
                            let _ = channels.events.try_send(CallEvent::OutboundMediaDropped {
                                video_access_units: dropped.video_access_units,
                                packets: dropped.packets,
                            });
                        }
                        // Discard any AUs still queued from the (now-detached) source, so a quick
                        // re-Enable can't transmit stale frames from the previous session under the
                        // new negotiation. Drained after the select block (the futures borrow the
                        // channel).
                        drain_video_in = true;
                    }
                    Ok(VideoControl::RequireKeyframe) => eng.require_video_keyframe(),
                    Ok(VideoControl::SetOrientation(o)) => eng.set_peer_video_orientation(o),
                    Ok(VideoControl::SetParticipantOrientation {
                        participant,
                        orientation,
                    }) => eng.set_participant_video_orientation(participant, orientation),
                    Err(_) => video_ctl_open = false,
                }
                // Fire an overdue timer like the other ready arms so a stream of control messages
                // cannot keep this arm hot and defer the keepalive.
                let now = now_ms();
                if let Some(at) = eng.poll_timeout()
                    && at != engine::NEVER
                    && now >= at
                {
                    eng.handle_input(now, Input::Timeout);
                }
            },
            ev = relay_events.recv().fuse() => match ev {
                Ok(RelayTransportEvent::PacketReceived(data)) => {
                    eng.handle_input(now_ms(), Input::RelayPacket(&data));
                    let now = now_ms();
                    if let Some(at) = eng.poll_timeout()
                        && at != engine::NEVER
                        && now >= at
                    {
                        eng.handle_input(now, Input::Timeout);
                    }
                }
                // The channel is already open by the time we run; Connected is a redundant confirm.
                Ok(RelayTransportEvent::Connected) => {}
                Ok(RelayTransportEvent::Disconnected(_)) | Err(_) => break 'drive,
            },
            frame = mic_fut => match frame {
                Ok(pcm) => {
                    eng.handle_input(now_ms(), Input::MicFrame(&pcm));
                    let now = now_ms();
                    if let Some(at) = eng.poll_timeout()
                        && at != engine::NEVER
                        && now >= at
                    {
                        eng.handle_input(now, Input::Timeout);
                    }
                }
                // Mic source gone (e.g. muted -> pw-record EOF). Stop polling it but keep the call
                // alive: muting the mic must not hang up the call (see the comment above).
                Err(_) => mic_open = false,
            },
            frame = encoded_audio_fut => match frame {
                Ok(encoded) => {
                    eng.handle_input(now_ms(), Input::EncodedAudio(&encoded));
                    let now = now_ms();
                    if let Some(at) = eng.poll_timeout()
                        && at != engine::NEVER
                        && now >= at
                    {
                        eng.handle_input(now, Input::Timeout);
                    }
                }
                Err(_) => encoded_audio_open = false,
            },
            au = video_in_fut => match au {
                Ok(au) => {
                    eng.handle_input(now_ms(), Input::VideoFrame(&au));
                    let now = now_ms();
                    if let Some(at) = eng.poll_timeout()
                        && at != engine::NEVER
                        && now >= at
                    {
                        eng.handle_input(now, Input::Timeout);
                    }
                }
                // Video source gone (encoder EOF / downgrade released it): disable the arm but keep
                // the call alive, exactly like the mic.
                Err(_) => video_in_open = false,
            },
            _ = &mut timer => eng.handle_input(now_ms(), Input::Timeout),
        }

        // Post-select: the arm futures (which borrow the channels) have been dropped, so it is now
        // safe to drain the video input queue requested by a Disable above.
        if drain_video_in {
            drain_video_in = false;
            while channels.video_in.try_recv().is_ok() {}
        }
    }

    // Any local exit (relay disconnect or send failure -- not a closed mic, which only disables its
    // arm) tears down the transport so the platform's relay read pump -- which may be parked in recv()
    // with no packet coming -- sees the channel close, returns, and releases its task and socket.
    #[cfg(feature = "tracing")]
    tracing::debug!(call_id = %call_id, "voip call drive ended");
    transport.disconnect().await;
}

#[cfg(all(test, feature = "voip-mlow"))]
mod tests {
    use super::*;
    use crate::runtime::AbortHandle;
    use crate::types::group_call::{
        GroupCallDevice, GroupCallParticipant, GroupCallRelay, GroupCallRelayEndpoint,
        GroupCallUpdate,
    };
    use crate::voip::demux::{RelayPacketKind, classify_relay_packet};
    use crate::voip::engine::{CallConfig, GroupEngineConfig, SequentialTxIds};
    use crate::voip::mlow::MlowEncoder;
    use crate::voip::session::{CallDirection, MediaPipeline, MediaPipelineParams};
    use crate::voip::{RelayDisconnectReason, stun};
    use async_trait::async_trait;
    use bytes::Bytes;
    use portable_atomic::AtomicU64;
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use wacore_binary::{Jid, Server};

    #[test]
    fn local_roster_removal_is_the_only_fatal_group_media_update_error() {
        assert!(is_fatal_group_update_error(
            &engine::EngineError::GroupMedia(GroupMediaError::LocalParticipantRemoved)
        ));
        assert!(!is_fatal_group_update_error(
            &engine::EngineError::GroupMedia(GroupMediaError::Pipeline)
        ));
    }

    /// Runtime whose `sleep` never resolves, so the driver is exercised purely by the relay-event
    /// stream (the timer arm stays pending). `spawn` is unused: the shell spawns `run_call`, not the
    /// loop itself.
    struct PendingSleepRuntime;
    #[async_trait]
    impl Runtime for PendingSleepRuntime {
        fn spawn(&self, _f: Pin<Box<dyn Future<Output = ()> + Send + 'static>>) -> AbortHandle {
            AbortHandle::noop()
        }
        fn sleep(&self, _d: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            Box::pin(futures::future::pending())
        }
        fn spawn_blocking(
            &self,
            _f: Box<dyn FnOnce() + Send + 'static>,
        ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            Box::pin(async {})
        }
        fn yield_now(&self) -> Option<Pin<Box<dyn Future<Output = ()> + Send>>> {
            None
        }
    }

    #[derive(Default)]
    struct RecordingTransport {
        sent: Mutex<Vec<Bytes>>,
    }
    #[async_trait]
    impl RelayTransport for RecordingTransport {
        async fn send(&self, data: Bytes) -> anyhow::Result<()> {
            self.sent.lock().unwrap().push(data);
            Ok(())
        }
        async fn disconnect(&self) {}
    }

    type ReplacementRelay = (
        Arc<dyn RelayTransport>,
        async_channel::Receiver<RelayTransportEvent>,
    );

    struct ReconnectTransport {
        sent: Mutex<Vec<Bytes>>,
        reconnects: Mutex<Vec<std::net::SocketAddr>>,
        replacement: Mutex<Option<ReplacementRelay>>,
    }

    #[derive(Default)]
    struct HangingReconnectTransport {
        reconnects: AtomicUsize,
        disconnects: AtomicUsize,
    }

    #[async_trait]
    impl RelayTransport for HangingReconnectTransport {
        async fn send(&self, _data: Bytes) -> anyhow::Result<()> {
            Ok(())
        }

        async fn disconnect(&self) {
            self.disconnects.fetch_add(1, Ordering::Relaxed);
        }

        async fn reconnect(
            &self,
            _endpoint: std::net::SocketAddr,
        ) -> anyhow::Result<(
            Arc<dyn RelayTransport>,
            async_channel::Receiver<RelayTransportEvent>,
        )> {
            self.reconnects.fetch_add(1, Ordering::Relaxed);
            futures::future::pending().await
        }
    }

    struct ReconnectTimeoutRuntime;

    #[async_trait]
    impl Runtime for ReconnectTimeoutRuntime {
        fn spawn(&self, _f: Pin<Box<dyn Future<Output = ()> + Send + 'static>>) -> AbortHandle {
            AbortHandle::noop()
        }

        fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            if duration == RELAY_RECONNECT_TIMEOUT {
                Box::pin(async {})
            } else {
                Box::pin(futures::future::pending())
            }
        }

        fn spawn_blocking(
            &self,
            _f: Box<dyn FnOnce() + Send + 'static>,
        ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            Box::pin(async {})
        }

        fn yield_now(&self) -> Option<Pin<Box<dyn Future<Output = ()> + Send>>> {
            None
        }
    }

    #[derive(Default)]
    struct HangingDisconnectTransport {
        disconnects: AtomicUsize,
    }

    #[async_trait]
    impl RelayTransport for HangingDisconnectTransport {
        async fn send(&self, _data: Bytes) -> anyhow::Result<()> {
            Ok(())
        }

        async fn disconnect(&self) {
            self.disconnects.fetch_add(1, Ordering::Relaxed);
            futures::future::pending().await
        }
    }

    #[async_trait]
    impl RelayTransport for ReconnectTransport {
        async fn send(&self, data: Bytes) -> anyhow::Result<()> {
            self.sent.lock().unwrap().push(data);
            Ok(())
        }

        async fn disconnect(&self) {}

        async fn reconnect(
            &self,
            endpoint: std::net::SocketAddr,
        ) -> anyhow::Result<(
            Arc<dyn RelayTransport>,
            async_channel::Receiver<RelayTransportEvent>,
        )> {
            self.reconnects.lock().unwrap().push(endpoint);
            self.replacement
                .lock()
                .unwrap()
                .take()
                .ok_or_else(|| anyhow::anyhow!("replacement already consumed"))
        }
    }

    #[test]
    fn video_control_channel_preserves_state_and_coalesces_orientation() {
        let (tx, rx) = video_control_channel();
        assert!(tx.send(VideoControl::Disable));
        for orientation in 0..100u8 {
            assert!(tx.send(VideoControl::SetOrientation(orientation % 4)));
        }
        assert!(tx.send(VideoControl::Enable));
        assert!(tx.send(VideoControl::RequireKeyframe));

        assert_eq!(rx.try_recv(), Ok(VideoControl::Disable));
        assert_eq!(rx.try_recv(), Ok(VideoControl::Enable));
        assert_eq!(rx.try_recv(), Ok(VideoControl::RequireKeyframe));
        assert_eq!(rx.try_recv(), Ok(VideoControl::SetOrientation(3)));
        assert_eq!(rx.try_recv(), Err(async_channel::TryRecvError::Empty));
    }

    #[test]
    fn relay_lifecycle_events_replace_saturated_diagnostics() {
        for lifecycle in [
            CallEvent::RelayAllocated,
            CallEvent::RelayAllocateFailed(486),
            CallEvent::RelayAllocateTimedOut,
            CallEvent::RelayReconnectTimedOut,
        ] {
            let (tx, rx) = async_channel::bounded(1);
            tx.try_send(CallEvent::GroupControlRejected {
                control: engine::GroupControlKind::Update,
            })
            .expect("diagnostic fills the event queue");

            publish_engine_event(&tx, lifecycle.clone());

            assert_eq!(rx.try_recv(), Ok(lifecycle));
            assert_eq!(rx.try_recv(), Err(async_channel::TryRecvError::Empty));
        }
    }

    #[test]
    fn video_control_channel_coalesces_group_orientation_per_participant() {
        let (tx, rx) = video_control_channel();
        let participant = Jid::new("200002", Server::Lid).with_device(3);
        for orientation in 0..100u8 {
            assert!(tx.send(VideoControl::SetParticipantOrientation {
                participant: participant.clone(),
                orientation: orientation % 4,
            }));
        }
        assert!(
            tx.retained_len() <= 2,
            "one participant must retain at most one value and one wake marker"
        );
        assert_eq!(
            rx.try_recv(),
            Ok(VideoControl::SetParticipantOrientation {
                participant,
                orientation: 3,
            })
        );
        assert_eq!(rx.try_recv(), Err(async_channel::TryRecvError::Empty));
    }

    #[test]
    fn group_transitions_purge_only_packets_protected_under_stale_state() {
        let batch = |kind, packets: usize, started| SendBatch {
            packets: (0..packets)
                .map(|_| Bytes::from_static(b"packet"))
                .collect(),
            bytes: packets * 6,
            kind,
            started,
            video_keyframe: false,
        };

        let mut downgrade_queue = VecDeque::from([
            batch(SendBatchKind::Control, 1, false),
            batch(SendBatchKind::Media, 1, false),
            batch(SendBatchKind::Video, 2, true),
        ]);
        let mut pending_video = vec![Bytes::from_static(b"fragment")];
        let mut awaiting_keyframe = false;
        let dropped = purge_group_transition_media(
            &mut downgrade_queue,
            &mut pending_video,
            &mut awaiting_keyframe,
            false,
            true,
        );
        assert_eq!(dropped.video_access_units, 2);
        assert_eq!(dropped.packets, 3);
        assert_eq!(downgrade_queue.len(), 2);
        assert!(
            downgrade_queue
                .iter()
                .all(|batch| batch.kind != SendBatchKind::Video)
        );
        assert!(awaiting_keyframe);

        downgrade_queue.push_back(batch(SendBatchKind::Video, 2, false));
        pending_video.push(Bytes::from_static(b"fragment"));
        let dropped = purge_group_transition_media(
            &mut downgrade_queue,
            &mut pending_video,
            &mut awaiting_keyframe,
            true,
            false,
        );
        assert_eq!(dropped.video_access_units, 2);
        assert_eq!(dropped.packets, 4);
        assert_eq!(downgrade_queue.len(), 1);
        assert_eq!(downgrade_queue[0].kind, SendBatchKind::Control);
        assert!(
            pending_video.is_empty(),
            "a new epoch must not leave a partial old-key access unit"
        );
    }

    #[test]
    fn relay_reconnect_drops_deltas_until_a_fresh_keyframe() {
        let video = |keyframe| SendBatch {
            packets: VecDeque::from([Bytes::from_static(b"video")]),
            bytes: 5,
            kind: SendBatchKind::Video,
            started: false,
            video_keyframe: keyframe,
        };
        let mut queue = VecDeque::from([video(false)]);
        let mut pending_video = vec![Bytes::from_static(b"fragment")];
        let mut awaiting_keyframe = false;

        prepare_relay_reconnect(&mut queue, &mut pending_video, &mut awaiting_keyframe);
        assert!(queue.is_empty());
        assert!(pending_video.is_empty());
        assert!(awaiting_keyframe);

        let dropped = enqueue_batch(&mut queue, &mut awaiting_keyframe, video(false));
        assert_eq!(dropped.video_access_units, 1);
        assert!(queue.is_empty(), "delta frame must not enter the new path");

        let dropped = enqueue_batch(&mut queue, &mut awaiting_keyframe, video(true));
        assert_eq!(dropped.video_access_units, 0);
        assert_eq!(queue.len(), 1);
        assert!(!awaiting_keyframe);
    }

    /// CallChannels with idle video plumbing (senders/receivers dropped immediately), for the
    /// audio-only driver tests: the closed-channel guards must keep those arms inert.
    fn test_channels(
        mic: async_channel::Receiver<Vec<i16>>,
        speaker: async_channel::Sender<Vec<i16>>,
        events: async_channel::Sender<CallEvent>,
    ) -> CallChannels {
        let (_vin_tx, vin_rx) = async_channel::unbounded::<Vec<u8>>();
        let (vout_tx, _vout_rx) = async_channel::unbounded::<VideoFrame>();
        let (_vctl_tx, vctl_rx) = video_control_channel();
        let (_encoded_tx, encoded_rx) = async_channel::unbounded::<Bytes>();
        let (encoded_out_tx, _encoded_out_rx) = async_channel::unbounded::<EncodedAudioFrame>();
        CallChannels {
            mic,
            speaker,
            encoded_audio_in: encoded_rx,
            encoded_audio_out: encoded_out_tx,
            events,
            rekey: None,
            video_in: vin_rx,
            video_out: vout_tx,
            video_ctl: vctl_rx,
            group_ctl: None,
        }
    }

    fn config() -> CallConfig {
        CallConfig {
            call_id: "CID".into(),
            direction: CallDirection::Incoming,
            self_lid: "111111111111111:0@lid".into(),
            peer_lid: "222222222222222:0@lid".into(),
            call_key: (0u8..32).collect(),
            ssrc: 0x5741_0001,
            audio: crate::voip::AudioConfig::MLOW_PCM,
            relay_token: vec![0xAB; 16],
            relay_ip: "203.0.113.7".into(),
            relay_port: 3478,
            integrity_key: b"relay-key".to_vec(),
            warp_mi_tag_len: 4,
            enable_media: true,
            enable_video: false,
            enable_sframe: false,
        }
    }

    fn group_relay(transaction_id: u32, ip: &str, port: u16) -> GroupCallRelay {
        GroupCallRelay::builder()
            .transaction_id(transaction_id)
            .self_pid(1)
            .uuid("relay".to_string())
            .participant_uuid("participant".to_string())
            .attribute_padding(false)
            .warp_mi_tag_len(4)
            .key(b"relay-key".to_vec())
            .tokens(vec![vec![0x47]])
            .auth_tokens(vec![vec![0x57]])
            .endpoints(vec![GroupCallRelayEndpoint {
                relay_id: 1,
                token_id: 0,
                auth_token_id: 0,
                relay_name: "relay-1".to_string(),
                domain_name: None,
                rtt_ms: None,
                is_fna: false,
                address: Vec::new(),
                ipv4: Some(ip.to_string()),
                port: Some(port),
            }])
            .build()
    }

    fn group_update(transaction_id: u32, ip: &str, port: u16) -> GroupCallUpdate {
        let self_jid = Jid::new("111111111111111", Server::Lid);
        let mut self_device = GroupCallDevice::new(self_jid.clone());
        self_device.pid = Some(1);
        let mut participant = GroupCallParticipant::new(self_jid.to_non_ad(), vec![self_device]);
        participant.state = Some("connected".to_string());
        GroupCallUpdate::builder()
            .call_id("00abcdef0123456789abcdef01234567".to_string())
            .call_creator(self_jid)
            .transaction_id(transaction_id)
            .media("audio".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(false)
            .participants(vec![participant])
            .relay(group_relay(transaction_id, ip, port))
            .build()
    }

    fn group_engine() -> CallEngine {
        let update = group_update(7, "203.0.113.7", 3478);
        let mut config = CallConfig::for_group(
            CallDirection::Outgoing,
            &update.call_id,
            "111111111111111@lid",
            "111111111111111@lid",
            update.relay.as_ref().expect("relay"),
        )
        .expect("group config");
        config.audio = crate::voip::AudioConfig::MLOW_PCM;
        let mut engine =
            CallEngine::new(config, Box::new(SequentialTxIds::new())).expect("group engine");
        engine
            .configure_group(GroupEngineConfig {
                call_creator: update.call_creator.clone(),
                self_jid: update.call_creator.clone(),
                initial_update: update,
                direct_peer: None,
            })
            .expect("configure group");
        engine
    }

    #[test]
    fn group_relay_migration_redials_transport_before_reallocation() {
        let rt: Arc<dyn Runtime> = Arc::new(PendingSleepRuntime);
        let replacement = Arc::new(RecordingTransport::default());
        let (replacement_tx, replacement_rx) = async_channel::unbounded();
        replacement_tx
            .try_send(RelayTransportEvent::Disconnected(
                RelayDisconnectReason::Closed,
            ))
            .unwrap();
        let transport = Arc::new(ReconnectTransport {
            sent: Mutex::new(Vec::new()),
            reconnects: Mutex::new(Vec::new()),
            replacement: Mutex::new(Some((
                replacement.clone() as Arc<dyn RelayTransport>,
                replacement_rx,
            ))),
        });
        let (_initial_tx, initial_rx) = async_channel::unbounded();
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, _spk_rx) = async_channel::unbounded();
        let (ev_tx, _ev_rx) = async_channel::unbounded();
        let (group_tx, group_rx) = async_channel::bounded(1);
        group_tx
            .try_send(GroupControl::Update(Box::new(group_update(
                8,
                "203.0.113.8",
                3481,
            ))))
            .unwrap();
        let mut channels = test_channels(mic_rx, spk_tx, ev_tx);
        channels.group_ctl = Some(group_rx);

        futures::executor::block_on(run_call(
            rt,
            transport.clone() as Arc<dyn RelayTransport>,
            initial_rx,
            channels,
            group_engine(),
        ));

        assert_eq!(
            *transport.reconnects.lock().unwrap(),
            ["203.0.113.8:3481".parse().unwrap()]
        );
        assert!(
            replacement
                .sent
                .lock()
                .unwrap()
                .iter()
                .any(|packet| stun::stun_message_type(packet) == Some(stun::MSG_ALLOCATE_REQUEST)),
            "the replacement transport must carry the fresh allocate"
        );
    }

    #[test]
    fn group_relay_migration_bounds_a_hung_reconnect() {
        let transport = Arc::new(HangingReconnectTransport::default());
        let (_initial_tx, initial_rx) = async_channel::unbounded();
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, _spk_rx) = async_channel::unbounded();
        let (ev_tx, ev_rx) = async_channel::unbounded();
        let (group_tx, group_rx) = async_channel::bounded(1);
        group_tx
            .try_send(GroupControl::Update(Box::new(group_update(
                8,
                "203.0.113.8",
                3481,
            ))))
            .unwrap();
        let mut channels = test_channels(mic_rx, spk_tx, ev_tx);
        channels.group_ctl = Some(group_rx);

        futures::executor::block_on(run_call(
            Arc::new(ReconnectTimeoutRuntime),
            transport.clone(),
            initial_rx,
            channels,
            group_engine(),
        ));

        assert_eq!(transport.reconnects.load(Ordering::Relaxed), 1);
        assert_eq!(
            ev_rx.try_recv(),
            Ok(CallEvent::RelayReconnectTimedOut),
            "the application must learn why relay migration ended the call"
        );
        assert_eq!(
            transport.disconnects.load(Ordering::Relaxed),
            1,
            "the timed-out driver must tear down the original transport"
        );
    }

    #[test]
    fn group_relay_migration_bounds_retired_transport_disconnect() {
        let transport = HangingDisconnectTransport::default();
        futures::executor::block_on(disconnect_relay_bounded(
            &ReconnectTimeoutRuntime,
            &transport,
        ));
        assert_eq!(
            transport.disconnects.load(Ordering::Relaxed),
            1,
            "the retired transport cleanup must be attempted without parking the driver"
        );
    }

    #[test]
    fn rejected_group_control_does_not_terminate_the_driver() {
        let rt: Arc<dyn Runtime> = Arc::new(PendingSleepRuntime);
        let transport = Arc::new(RecordingTransport::default());
        let (relay_tx, relay_rx) = async_channel::unbounded();
        let binding =
            stun::encode_stun_request(stun::MSG_BINDING_REQUEST, &[9u8; 12], &[], None, false);
        relay_tx
            .try_send(RelayTransportEvent::PacketReceived(Bytes::from(binding)))
            .unwrap();
        relay_tx
            .try_send(RelayTransportEvent::Disconnected(
                RelayDisconnectReason::Closed,
            ))
            .unwrap();
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, _spk_rx) = async_channel::unbounded();
        let (ev_tx, ev_rx) = async_channel::unbounded();
        let (group_tx, group_rx) = async_channel::bounded(1);
        group_tx
            .try_send(GroupControl::Reaction("x".repeat(1024)))
            .unwrap();
        let mut channels = test_channels(mic_rx, spk_tx, ev_tx);
        channels.group_ctl = Some(group_rx);

        futures::executor::block_on(run_call(
            rt,
            transport.clone() as Arc<dyn RelayTransport>,
            relay_rx,
            channels,
            group_engine(),
        ));

        assert!(
            transport
                .sent
                .lock()
                .unwrap()
                .iter()
                .any(|packet| stun::stun_message_type(packet) == Some(stun::MSG_BINDING_SUCCESS)),
            "the driver must keep processing relay traffic after rejecting a control"
        );
        assert!(
            std::iter::from_fn(|| ev_rx.try_recv().ok()).any(|event| matches!(
                event,
                CallEvent::GroupControlRejected {
                    control: engine::GroupControlKind::Reaction
                }
            ))
        );
    }

    #[test]
    fn epoch_before_group_configuration_does_not_terminate_the_driver() {
        let rt: Arc<dyn Runtime> = Arc::new(PendingSleepRuntime);
        let transport = Arc::new(RecordingTransport::default());
        let (relay_tx, relay_rx) = async_channel::unbounded();
        let binding =
            stun::encode_stun_request(stun::MSG_BINDING_REQUEST, &[8u8; 12], &[], None, false);
        relay_tx
            .try_send(RelayTransportEvent::PacketReceived(Bytes::from(binding)))
            .unwrap();
        relay_tx
            .try_send(RelayTransportEvent::Disconnected(
                RelayDisconnectReason::Closed,
            ))
            .unwrap();
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, _spk_rx) = async_channel::unbounded();
        let (ev_tx, ev_rx) = async_channel::unbounded();
        let (group_tx, group_rx) = async_channel::bounded(1);
        group_tx
            .try_send(GroupControl::RawEpoch(GroupRawEpoch::new(1, vec![1; 32])))
            .unwrap();
        let mut channels = test_channels(mic_rx, spk_tx, ev_tx);
        channels.group_ctl = Some(group_rx);
        let engine =
            CallEngine::new(config(), Box::new(SequentialTxIds::new())).expect("direct engine");

        futures::executor::block_on(run_call(
            rt,
            transport.clone() as Arc<dyn RelayTransport>,
            relay_rx,
            channels,
            engine,
        ));

        assert!(
            transport
                .sent
                .lock()
                .unwrap()
                .iter()
                .any(|packet| stun::stun_message_type(packet) == Some(stun::MSG_BINDING_SUCCESS)),
            "a racing epoch must not stop subsequent relay processing"
        );
        assert!(
            std::iter::from_fn(|| ev_rx.try_recv().ok()).any(|event| matches!(
                event,
                CallEvent::GroupControlRejected {
                    control: engine::GroupControlKind::Epoch
                }
            )),
            "the pre-roster epoch is rejected explicitly instead of terminating the call"
        );
    }

    #[test]
    fn group_update_pipeline_error_does_not_terminate_the_driver() {
        let rt: Arc<dyn Runtime> = Arc::new(PendingSleepRuntime);
        let transport = Arc::new(RecordingTransport::default());
        let (relay_tx, relay_rx) = async_channel::unbounded();
        let binding =
            stun::encode_stun_request(stun::MSG_BINDING_REQUEST, &[7u8; 12], &[], None, false);
        relay_tx
            .try_send(RelayTransportEvent::PacketReceived(Bytes::from(binding)))
            .unwrap();
        relay_tx
            .try_send(RelayTransportEvent::Disconnected(
                RelayDisconnectReason::Closed,
            ))
            .unwrap();
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, _spk_rx) = async_channel::unbounded();
        let (ev_tx, ev_rx) = async_channel::unbounded();
        let (group_tx, group_rx) = async_channel::bounded(1);
        let update = group_update(1, "203.0.113.7", 3478);
        group_tx
            .try_send(GroupControl::Update(Box::new(update.clone())))
            .unwrap();
        let mut channels = test_channels(mic_rx, spk_tx, ev_tx);
        channels.group_ctl = Some(group_rx);
        let mut direct_config = config();
        direct_config.enable_media = false;
        let mut engine = CallEngine::new(direct_config, Box::new(SequentialTxIds::new()))
            .expect("control-only engine");
        assert!(matches!(
            engine.apply_group_update(0, &update),
            Err(engine::EngineError::GroupMedia(GroupMediaError::Pipeline))
        ));

        futures::executor::block_on(run_call(
            rt,
            transport.clone() as Arc<dyn RelayTransport>,
            relay_rx,
            channels,
            engine,
        ));

        assert!(
            transport
                .sent
                .lock()
                .unwrap()
                .iter()
                .any(|packet| stun::stun_message_type(packet) == Some(stun::MSG_BINDING_SUCCESS)),
            "a failed roster refresh must not stop subsequent relay processing"
        );
        assert!(
            std::iter::from_fn(|| ev_rx.try_recv().ok()).any(|event| matches!(
                event,
                CallEvent::GroupControlRejected {
                    control: engine::GroupControlKind::Update
                }
            )),
            "the non-fatal engine error must be surfaced as a rejected update"
        );
    }

    #[test]
    fn rejected_group_transition_surfaces_its_paired_epoch() {
        let rt: Arc<dyn Runtime> = Arc::new(PendingSleepRuntime);
        let transport = Arc::new(RecordingTransport::default());
        let (relay_tx, relay_rx) = async_channel::unbounded();
        relay_tx
            .try_send(RelayTransportEvent::Disconnected(
                RelayDisconnectReason::Closed,
            ))
            .unwrap();
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, _spk_rx) = async_channel::unbounded();
        let (ev_tx, ev_rx) = async_channel::unbounded();
        let (group_tx, group_rx) = async_channel::bounded(1);
        let mut invalid = group_update(2, "203.0.113.7", 3478);
        invalid.call_id = "WRONG-CALL".to_string();
        group_tx
            .try_send(GroupControl::Transition {
                update: Box::new(invalid),
                epoch: GroupRawEpoch::new(2, vec![2; 32]),
            })
            .unwrap();
        let mut channels = test_channels(mic_rx, spk_tx, ev_tx);
        channels.group_ctl = Some(group_rx);

        futures::executor::block_on(run_call(
            rt,
            transport as Arc<dyn RelayTransport>,
            relay_rx,
            channels,
            group_engine(),
        ));

        let rejected = std::iter::from_fn(|| ev_rx.try_recv().ok())
            .filter_map(|event| match event {
                CallEvent::GroupControlRejected { control } => Some(control),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(rejected.contains(&engine::GroupControlKind::Update));
        assert!(rejected.contains(&engine::GroupControlKind::Epoch));
    }

    // The driver wiring: start sends the allocate, an inbound binding request gets a binding-success
    // reply, and a Disconnected event ends the loop. Timer-pending so this is deterministic.
    #[test]
    fn run_call_emits_allocate_and_answers_binding_request() {
        let rt: Arc<dyn Runtime> = Arc::new(PendingSleepRuntime);
        let transport = Arc::new(RecordingTransport::default());
        let (relay_tx, relay_rx) = async_channel::unbounded();
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, _spk_rx) = async_channel::unbounded();
        let (ev_tx, _ev_rx) = async_channel::unbounded();

        let req =
            stun::encode_stun_request(stun::MSG_BINDING_REQUEST, &[9u8; 12], &[], None, false);
        relay_tx
            .try_send(RelayTransportEvent::PacketReceived(Bytes::from(req)))
            .unwrap();
        relay_tx
            .try_send(RelayTransportEvent::Disconnected(
                RelayDisconnectReason::Closed,
            ))
            .unwrap();

        let eng = CallEngine::new(config(), Box::new(SequentialTxIds::new())).unwrap();
        futures::executor::block_on(run_call(
            rt,
            transport.clone() as Arc<dyn RelayTransport>,
            relay_rx,
            test_channels(mic_rx, spk_tx, ev_tx),
            eng,
        ));

        let sent = transport.sent.lock().unwrap();
        assert!(
            sent.iter()
                .any(|b| stun::stun_message_type(b) == Some(stun::MSG_ALLOCATE_REQUEST)),
            "start must emit the STUN allocate"
        );
        assert!(
            sent.iter()
                .any(|b| stun::stun_message_type(b) == Some(stun::MSG_BINDING_SUCCESS)),
            "a binding request must be answered with a binding success"
        );
    }

    /// Runtime with an instant `sleep` that closes `relay_tx` once the `close_after`-th sleep has
    /// *elapsed*, so a driver loop that survives a closed mic still terminates deterministically -- via
    /// the relay path, never the mic. `sleeps` counts the arms so a test can assert the loop kept
    /// arming the keepalive/playout timer after the mic went away. The close is deferred to the
    /// returned future rather than done in `sleep()` itself: a real runtime's `sleep` has no
    /// construction-time effect, and the driver arms its timer before it parks on the select.
    struct CloseRelayOnSleepRuntime {
        sleeps: Arc<AtomicUsize>,
        relay_tx: async_channel::Sender<RelayTransportEvent>,
        close_after: usize,
    }
    #[async_trait]
    impl Runtime for CloseRelayOnSleepRuntime {
        fn spawn(&self, _f: Pin<Box<dyn Future<Output = ()> + Send + 'static>>) -> AbortHandle {
            AbortHandle::noop()
        }
        fn sleep(&self, _d: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            let close = self.sleeps.fetch_add(1, Ordering::Relaxed) + 1 >= self.close_after;
            let relay_tx = self.relay_tx.clone();
            Box::pin(async move {
                if close {
                    relay_tx.close();
                }
            })
        }
        fn spawn_blocking(
            &self,
            _f: Box<dyn FnOnce() + Send + 'static>,
        ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            Box::pin(async {})
        }
        fn yield_now(&self) -> Option<Pin<Box<dyn Future<Output = ()> + Send>>> {
            None
        }
    }

    // Regression: a closed mic must NOT end the call. OS mute can make the mic source (pw-record) EOF,
    // closing the mic channel; if that tore the loop down, the relay would lose its 1s keepalive, drop
    // us after ~4s, and the peer would reconnect -- the mute/unmute reconnect loop. The loop must
    // instead disable the mic and keep arming the keepalive/playout timer. We pre-close the mic, then
    // let the runtime end the call via the relay after a couple of timer arms; had the closed mic
    // ended the call, the loop would break before arming any timer and `sleeps` would stay 0.
    #[test]
    fn closed_mic_does_not_end_the_call() {
        let sleeps = Arc::new(AtomicUsize::new(0));
        let (relay_tx, relay_rx) = async_channel::unbounded();
        let rt: Arc<dyn Runtime> = Arc::new(CloseRelayOnSleepRuntime {
            sleeps: sleeps.clone(),
            relay_tx,
            close_after: 2,
        });
        let transport = Arc::new(RecordingTransport::default());
        let (mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        mic_tx.close(); // mic source gone before the call even starts
        let (spk_tx, _spk_rx) = async_channel::unbounded();
        let (ev_tx, _ev_rx) = async_channel::unbounded();

        let eng = CallEngine::new(config(), Box::new(SequentialTxIds::new())).unwrap();
        futures::executor::block_on(run_call(
            rt,
            transport.clone() as Arc<dyn RelayTransport>,
            relay_rx,
            test_channels(mic_rx, spk_tx, ev_tx),
            eng,
        ));

        assert!(
            sleeps.load(Ordering::Relaxed) >= 1,
            "a closed mic must not end the call: the loop has to keep arming the keepalive timer"
        );
        assert!(
            transport
                .sent
                .lock()
                .unwrap()
                .iter()
                .any(|b| stun::stun_message_type(b) == Some(stun::MSG_ALLOCATE_REQUEST)),
            "the call must still start (emit the allocate) with a dead mic"
        );
    }

    // A continuously-ready relay must not starve the keepalive/playout timers. `select_biased!`
    // always prefers the relay arm, so without the in-line overdue-timer fire a relay flood would
    // defer Timeout forever (no keepalive -> relay drops us -> reconnect). The clock races forward so
    // every deadline is overdue; the relay is pre-filled (the flood) then closed. With the fix, each
    // drained packet fires the overdue timer, so playout frames keep flowing during the flood.
    #[test]
    fn relay_flood_does_not_starve_the_timer() {
        let rt: Arc<dyn Runtime> = Arc::new(PendingSleepRuntime); // the timer arm never resolves itself
        let transport = Arc::new(RecordingTransport::default());
        let (relay_tx, relay_rx) = async_channel::unbounded();
        for _ in 0..40 {
            relay_tx
                .try_send(RelayTransportEvent::PacketReceived(Bytes::from_static(
                    b"\0\0\0\0",
                )))
                .unwrap();
        }
        relay_tx
            .try_send(RelayTransportEvent::Disconnected(
                RelayDisconnectReason::Closed,
            ))
            .unwrap();
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, spk_rx) = async_channel::unbounded();
        let (ev_tx, _ev_rx) = async_channel::unbounded();

        // Each clock read jumps a full second, so the 1s keepalive and 20ms playout are always overdue.
        let clock = Arc::new(AtomicU64::new(0));
        let clk = clock.clone();
        let eng = CallEngine::new(config(), Box::new(SequentialTxIds::new())).unwrap();
        futures::executor::block_on(run_call_with_clock(
            rt,
            transport.clone() as Arc<dyn RelayTransport>,
            relay_rx,
            test_channels(mic_rx, spk_tx, ev_tx),
            eng,
            move || clk.fetch_add(1000, Ordering::Relaxed),
        ));

        let playouts = std::iter::from_fn(|| spk_rx.try_recv().ok()).count();
        assert!(
            playouts > 0,
            "the playout timer must keep firing during a relay flood (not starved); got {playouts}"
        );
    }

    /// Records sends and fails after `fail_after` of them, so a drive terminates deterministically via
    /// the send-failure `break 'drive`. A mic-starvation test needs this: a relay event would be
    /// biased ahead of the mic arm and pre-empt the very backlog under test, so the terminator must
    /// not come from the relay.
    struct FailAfterTransport {
        sent: Mutex<Vec<Bytes>>,
        fail_after: usize,
    }
    #[async_trait]
    impl RelayTransport for FailAfterTransport {
        async fn send(&self, data: Bytes) -> anyhow::Result<()> {
            let mut s = self.sent.lock().unwrap();
            s.push(data);
            if s.len() > self.fail_after {
                anyhow::bail!("send failure (test terminator)");
            }
            Ok(())
        }
        async fn disconnect(&self) {}
    }

    // The mic side of the same starvation hazard as `relay_flood_does_not_starve_the_timer`:
    // `select_biased!` prefers the mic arm over the timer, so without the in-line overdue-timer fire
    // (mirroring the relay arm) a continuously-ready mic -- a custom producer on an unbounded channel,
    // or a built-up backlog -- would defer the keepalive/playout `Timeout` indefinitely. The relay
    // stays empty+open so the mic arm wins every iteration; the clock races forward so every deadline
    // is overdue; a send-failure terminator ends the drive (a relay event would win the bias). With
    // the fix, each drained mic frame fires the overdue timer, so playout frames keep flowing.
    #[test]
    fn mic_flood_does_not_starve_the_timer() {
        let rt: Arc<dyn Runtime> = Arc::new(PendingSleepRuntime); // the timer arm never resolves itself
        let transport = Arc::new(FailAfterTransport {
            sent: Mutex::new(Vec::new()),
            fail_after: 60,
        });
        // Relay empty but OPEN: `_relay_tx` keeps the channel from closing (a closed/Err relay would
        // win the bias and break immediately), so `recv()` just pends and the mic arm always wins.
        let (_relay_tx, relay_rx) = async_channel::unbounded::<RelayTransportEvent>();
        // A deep backlog of non-silent frames, so the mic arm is continuously ready (never drains
        // before the send-failure terminator fires).
        let (mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let tone: Vec<i16> = (0..960i32).map(|i| (i % 200) as i16 - 99).collect();
        for _ in 0..300 {
            mic_tx.try_send(tone.clone()).unwrap();
        }
        let (spk_tx, spk_rx) = async_channel::unbounded();
        let (ev_tx, _ev_rx) = async_channel::unbounded();

        // Each clock read jumps a full second, so the 1s keepalive and 20ms playout are always overdue.
        let clock = Arc::new(AtomicU64::new(0));
        let clk = clock.clone();
        let eng = CallEngine::new(config(), Box::new(SequentialTxIds::new())).unwrap();
        futures::executor::block_on(run_call_with_clock(
            rt,
            transport.clone() as Arc<dyn RelayTransport>,
            relay_rx,
            test_channels(mic_rx, spk_tx, ev_tx),
            eng,
            move || clk.fetch_add(1000, Ordering::Relaxed),
        ));

        let playouts = std::iter::from_fn(|| spk_rx.try_recv().ok()).count();
        assert!(
            playouts > 0,
            "the playout timer must keep firing during a mic flood (not starved); got {playouts}"
        );
    }

    // A terminal allocate-error must end run_call: the driver breaks after forwarding the terminal
    // event, so the call tears down instead of keepaliving a dead relay forever. The relay event
    // stream is never closed (only the allocate error is pushed); if the engine did not terminate
    // the loop, block_on would deadlock on the pending timer/relay arms.
    #[test]
    fn allocate_error_ends_run_call() {
        let rt: Arc<dyn Runtime> = Arc::new(PendingSleepRuntime);
        let transport = Arc::new(RecordingTransport::default());
        let (relay_tx, relay_rx) = async_channel::unbounded();
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, _spk_rx) = async_channel::unbounded();
        let (ev_tx, ev_rx) = async_channel::unbounded();

        let mut eng = CallEngine::new(config(), Box::new(SequentialTxIds::new())).unwrap();
        eng.start(0, 0);
        let allocation = match eng.poll_output() {
            Output::Transmit(packet) => packet,
            other => panic!("expected initial allocation, got {other:?}"),
        };
        let transaction_id: [u8; 12] = stun::stun_transaction_id(&allocation)
            .expect("allocation transaction id")
            .try_into()
            .expect("STUN transaction IDs are 12 bytes");

        // Raw Allocate-error STUN packet carrying ERROR-CODE 486 (class 4, number 86).
        let err_attr = [0x00, 0x09, 0x00, 0x04, 0x00, 0x00, 4u8, 86u8];
        let err = stun::encode_stun_request(
            stun::MSG_ALLOCATE_ERROR,
            &transaction_id,
            &err_attr,
            None,
            false,
        );
        relay_tx
            .try_send(RelayTransportEvent::PacketReceived(Bytes::from(err)))
            .unwrap();
        // Note: the relay stream is intentionally NOT closed; the engine termination must end the loop.

        futures::executor::block_on(run_call(
            rt,
            transport.clone() as Arc<dyn RelayTransport>,
            relay_rx,
            test_channels(mic_rx, spk_tx, ev_tx),
            eng,
        ));

        // The terminal event reached the consumer before the loop broke.
        let events: Vec<CallEvent> = std::iter::from_fn(|| ev_rx.try_recv().ok()).collect();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, CallEvent::RelayAllocateFailed(486))),
            "the terminal RelayAllocateFailed must be delivered before teardown"
        );
    }

    /// A runtime with virtual time: `sleep(d)` advances the shared clock by `d` and resolves at once,
    /// so a `block_on` drive steps deterministically through the keepalive/playout deadlines with no
    /// real waiting. Pair with a `now_ms` closure that reads the same clock.
    struct VirtualTimeRuntime {
        clock: Arc<AtomicU64>,
    }
    #[async_trait]
    impl Runtime for VirtualTimeRuntime {
        fn spawn(&self, _f: Pin<Box<dyn Future<Output = ()> + Send + 'static>>) -> AbortHandle {
            AbortHandle::noop()
        }
        fn sleep(&self, d: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            self.clock
                .fetch_add(d.as_millis() as u64, Ordering::Relaxed);
            Box::pin(async {})
        }
        fn spawn_blocking(
            &self,
            _f: Box<dyn FnOnce() + Send + 'static>,
        ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            Box::pin(async {})
        }
        fn yield_now(&self) -> Option<Pin<Box<dyn Future<Output = ()> + Send>>> {
            None
        }
    }

    /// An in-memory relay that closes the media loop end-to-end without a real UDP/DTLS/SCTP socket.
    /// It records what the engine transmits and reacts like a real relay+peer: it accepts the first
    /// STUN allocate (so the engine's media path goes live) and then streams two MLow tone frames
    /// back as a mirrored peer. After `stop_after_allocates` allocates (one keepalive cycle) it pushes
    /// Disconnected to end the call.
    struct FakeRelay {
        events: async_channel::Sender<RelayTransportEvent>,
        sent: Mutex<Vec<Bytes>>,
        peer: Mutex<PeerSim>,
    }
    struct PeerSim {
        pipe: MediaPipeline,
        enc: MlowEncoder,
        allocates: usize,
        stop_after_allocates: usize,
    }
    #[async_trait]
    impl RelayTransport for FakeRelay {
        async fn send(&self, data: Bytes) -> anyhow::Result<()> {
            self.sent.lock().unwrap().push(data.clone());
            if stun::stun_message_type(&data) != Some(stun::MSG_ALLOCATE_REQUEST) {
                return Ok(());
            }
            let mut peer = self.peer.lock().unwrap();
            peer.allocates += 1;
            if peer.allocates == 1 {
                // The relay accepts the allocate, then the mirrored peer streams two MLow tone frames.
                let transaction_id: [u8; 12] = stun::stun_transaction_id(&data)
                    .expect("allocate transaction id")
                    .try_into()
                    .expect("STUN transaction IDs are 12 bytes");
                let ok = stun::encode_stun_request(
                    stun::MSG_ALLOCATE_SUCCESS,
                    &transaction_id,
                    &[],
                    None,
                    false,
                );
                let _ = self
                    .events
                    .try_send(RelayTransportEvent::PacketReceived(Bytes::from(ok)));
                for n in 0..2u32 {
                    let tone: Vec<f32> = (0..960usize)
                        .map(|i| 0.3 * ((i as f32 + (n * 960) as f32) * 0.07).sin())
                        .collect();
                    let frame = peer.enc.encode(&tone).expect("mlow encode");
                    let pkt = peer.pipe.protect_audio(&frame);
                    let _ = self
                        .events
                        .try_send(RelayTransportEvent::PacketReceived(Bytes::from(pkt)));
                }
            } else if peer.allocates >= peer.stop_after_allocates {
                let _ = self.events.try_send(RelayTransportEvent::Disconnected(
                    RelayDisconnectReason::Closed,
                ));
            }
            Ok(())
        }
        async fn disconnect(&self) {}
    }

    // End-to-end over the in-memory FakeRelay: allocate handshake -> RelayAllocated, mic tone ->
    // outbound RTP, peer RTP -> audible playout, a keepalive over (virtual) time, then teardown. This
    // drives the real run_call + engine + RelayTransport seam; only the webrtc-rs socket is mocked,
    // closing the "media path not exercised end-to-end" gap.
    #[test]
    fn full_media_path_over_fake_relay() {
        let clock = Arc::new(AtomicU64::new(0));
        let rt: Arc<dyn Runtime> = Arc::new(VirtualTimeRuntime {
            clock: clock.clone(),
        });

        let (relay_tx, relay_rx) = async_channel::unbounded();
        let cfg = config();
        // Mirror: the peer's self LID is our peer LID, so its protect keys match the engine's unprotect.
        let peer_pipe = MediaPipeline::new(&MediaPipelineParams {
            call_key: &cfg.call_key,
            self_lid: &cfg.peer_lid,
            peer_lid: &cfg.self_lid,
            ssrc: cfg.ssrc,
            samples_per_packet: cfg.audio.format.rtp_timestamp_step,
            warp_mi_tag_len: cfg.warp_mi_tag_len,
        })
        .unwrap();
        let relay = Arc::new(FakeRelay {
            events: relay_tx,
            sent: Mutex::new(Vec::new()),
            peer: Mutex::new(PeerSim {
                pipe: peer_pipe,
                enc: MlowEncoder::new(),
                allocates: 0,
                stop_after_allocates: 2,
            }),
        });

        let (mic_tx, mic_rx) = async_channel::unbounded();
        let tone: Vec<i16> = (0..cfg.audio.format.samples_per_frame as usize)
            .map(|i| (8000.0 * (i as f32 * 0.1).sin()) as i16)
            .collect();
        mic_tx.try_send(tone).unwrap();
        let (spk_tx, spk_rx) = async_channel::unbounded();
        let (ev_tx, ev_rx) = async_channel::unbounded();

        let eng = CallEngine::new(cfg, Box::new(SequentialTxIds::new())).unwrap();
        let clk = clock.clone();
        futures::executor::block_on(run_call_with_clock(
            rt,
            relay.clone() as Arc<dyn RelayTransport>,
            relay_rx,
            test_channels(mic_rx, spk_tx, ev_tx),
            eng,
            move || clk.load(Ordering::Relaxed),
        ));
        // mic_tx stays alive until here so the mic channel never closes during the drive.
        drop(mic_tx);

        // 1. The allocate handshake surfaced RelayAllocated to the shell.
        let events: Vec<CallEvent> = std::iter::from_fn(|| ev_rx.try_recv().ok()).collect();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, CallEvent::RelayAllocated)),
            "the allocate handshake must surface RelayAllocated"
        );

        // 2. Outbound: an initial allocate, the mic tone's RTP, and a keepalive re-allocate.
        let sent = relay.sent.lock().unwrap();
        let allocates = sent
            .iter()
            .filter(|b| stun::stun_message_type(b) == Some(stun::MSG_ALLOCATE_REQUEST))
            .count();
        assert!(
            allocates >= 2,
            "initial allocate + at least one keepalive re-allocate; got {allocates}"
        );
        let rtp = sent
            .iter()
            .filter(|b| matches!(classify_relay_packet(b), RelayPacketKind::Rtp))
            .count();
        assert!(
            rtp >= 1,
            "the mic tone must produce at least one outbound RTP packet"
        );

        // 3. Inbound: the peer's RTP decoded to non-silent playout at the speaker.
        let peak = std::iter::from_fn(|| spk_rx.try_recv().ok())
            .flatten()
            .map(|s| s.abs())
            .max()
            .unwrap_or(0);
        assert!(
            peak > 0,
            "peer RTP must decode to audible playout end-to-end"
        );
    }

    /// Runtime for the decoupling test: each `sleep` advances a shared virtual clock (so the playout
    /// timer actually fires) and, once the clock passes `close_at_ms`, closes the relay channel to end
    /// the call deterministically -- independent of `transport.send()`, which is wedged in the test.
    struct DrivingRuntime {
        clock: Arc<AtomicU64>,
        relay_tx: async_channel::Sender<RelayTransportEvent>,
        close_at_ms: u64,
    }
    #[async_trait]
    impl Runtime for DrivingRuntime {
        fn spawn(&self, _f: Pin<Box<dyn Future<Output = ()> + Send + 'static>>) -> AbortHandle {
            AbortHandle::noop()
        }
        fn sleep(&self, d: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            let now = self
                .clock
                .fetch_add(d.as_millis() as u64, Ordering::Relaxed)
                + d.as_millis() as u64;
            if now >= self.close_at_ms {
                self.relay_tx.close();
            }
            Box::pin(async {})
        }
        fn spawn_blocking(
            &self,
            _f: Box<dyn FnOnce() + Send + 'static>,
        ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            Box::pin(async {})
        }
        fn yield_now(&self) -> Option<Pin<Box<dyn Future<Output = ()> + Send>>> {
            None
        }
    }

    // Regression: a wedged relay write must NOT freeze the receive/playout path. This reproduces the
    // root cause of the whatsapp-rust<->whatsapp-rust glitching: with the old inline
    // `transport.send().await`, the first send (the STUN allocate) blocked the whole loop, so inbound
    // packets never decoded and the speaker starved (silent/choppy audio). Now the send is decoupled,
    // so injected peer RTP still decodes to audible playout while the send is stuck forever.
    #[test]
    fn wedged_send_does_not_freeze_inbound_playout() {
        struct WedgedSend;
        #[async_trait]
        impl RelayTransport for WedgedSend {
            async fn send(&self, _data: Bytes) -> anyhow::Result<()> {
                // A congested SCTP / dead link: this write never completes.
                futures::future::pending().await
            }
            async fn disconnect(&self) {}
        }

        let cfg = config();
        // Mirror the peer's pipeline (its self LID is our peer LID) so the RTP it "sends" decrypts and
        // decodes on our side.
        let mut peer_pipe = MediaPipeline::new(&MediaPipelineParams {
            call_key: &cfg.call_key,
            self_lid: &cfg.peer_lid,
            peer_lid: &cfg.self_lid,
            ssrc: cfg.ssrc,
            samples_per_packet: cfg.audio.format.rtp_timestamp_step,
            warp_mi_tag_len: cfg.warp_mi_tag_len,
        })
        .unwrap();
        let mut enc = MlowEncoder::new();

        let (relay_tx, relay_rx) = async_channel::unbounded();
        // Several non-silent peer frames (enough to clear the playout prebuffer), fed directly so the
        // inbound path does NOT depend on our send reaching the relay.
        for n in 0..6u32 {
            let tone: Vec<f32> = (0..960usize)
                .map(|i| 0.3 * ((i as f32 + (n * 960) as f32) * 0.05).sin())
                .collect();
            let frame = enc.encode(&tone).expect("mlow encode");
            let pkt = peer_pipe.protect_audio(&frame);
            relay_tx
                .try_send(RelayTransportEvent::PacketReceived(Bytes::from(pkt)))
                .unwrap();
        }

        let clock = Arc::new(AtomicU64::new(0));
        let rt: Arc<dyn Runtime> = Arc::new(DrivingRuntime {
            clock: clock.clone(),
            relay_tx: relay_tx.clone(),
            // ~25 playout ticks: long enough to drain the injected frames before the relay closes.
            close_at_ms: 500,
        });
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, spk_rx) = async_channel::unbounded();
        let (ev_tx, _ev_rx) = async_channel::unbounded();

        let clk = clock.clone();
        let eng = CallEngine::new(cfg, Box::new(SequentialTxIds::new())).unwrap();
        // Run on a worker thread with a wall-clock bound: the OLD inline-send loop DEADLOCKS here (the
        // wedged allocate send freezes the loop forever), so without a bound a regression would hang the
        // whole test binary instead of failing. The fixed loop terminates in microseconds.
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            futures::executor::block_on(run_call_with_clock(
                rt,
                Arc::new(WedgedSend) as Arc<dyn RelayTransport>,
                relay_rx,
                test_channels(mic_rx, spk_tx, ev_tx),
                eng,
                move || clk.load(Ordering::Relaxed),
            ));
            // Despite `send()` being wedged forever, the injected peer RTP must have decoded to audible
            // playout; report its peak amplitude back to the test.
            let peak = std::iter::from_fn(|| spk_rx.try_recv().ok())
                .flatten()
                .map(|s| s.abs())
                .max()
                .unwrap_or(0);
            let _ = done_tx.send(peak);
        });

        let peak = done_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("run_call must terminate: a wedged transport.send() must not freeze the loop");
        assert!(
            peak > 0,
            "inbound audio must play out while transport.send() is wedged (send/receive decoupled)"
        );
    }

    // Video through the real drive loop: Enable + orientation via video_ctl (biased ahead of the
    // media arms), mirrored peer video packets via the relay (reassemble to video_out with the
    // orientation stamped), one of our own AUs via video_in (fans out to PT-97 FU-A transmits).
    // Deterministic: queues drain by bias order, then the first timer arm closes the relay.
    #[test]
    fn drive_loop_routes_video_both_ways() {
        use crate::voip::rtp::{RTP_PAYLOAD_TYPE_H264, VIDEO_TS_STRIDE_15FPS, parse_rtp_header};
        use crate::voip::session::{VideoPipeline, VideoPipelineParams};
        use crate::voip::ssrc;

        let sleeps = Arc::new(AtomicUsize::new(0));
        let (relay_tx, relay_rx) = async_channel::unbounded();
        // The timer arm is only polled once every queue is idle; its first sleep closes the relay,
        // ending the loop deterministically after all the video work is done.
        let rt: Arc<dyn Runtime> = Arc::new(CloseRelayOnSleepRuntime {
            sleeps,
            relay_tx: relay_tx.clone(),
            close_after: 1,
        });
        let transport = Arc::new(RecordingTransport::default());
        let cfg = config();

        // Mirrored peer video pipe (its self LID = our peer LID).
        let mut peer_video = VideoPipeline::new(&VideoPipelineParams {
            call_key: &cfg.call_key,
            self_lid: &cfg.peer_lid,
            peer_lid: &cfg.self_lid,
            ssrc: ssrc::derive_video_participant_ssrc(
                &cfg.call_id,
                &ssrc::format_e2e_srtp_participant_id(&cfg.peer_lid),
            ),
            ts_stride: VIDEO_TS_STRIDE_15FPS,
            warp_mi_tag_len: cfg.warp_mi_tag_len,
        })
        .unwrap();
        let make_au = |len: usize| -> Vec<u8> {
            let mut au = vec![0, 0, 0, 1, 0x65];
            au.extend((0..len).map(|i| (i % 251) as u8));
            au
        };
        let peer_au = make_au(2000);
        for p in peer_video.protect_video(&peer_au) {
            relay_tx
                .try_send(RelayTransportEvent::PacketReceived(Bytes::from(p)))
                .unwrap();
        }

        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, _spk_rx) = async_channel::unbounded();
        let (ev_tx, _ev_rx) = async_channel::unbounded();
        let (vin_tx, vin_rx) = async_channel::unbounded::<Vec<u8>>();
        let (vout_tx, vout_rx) = async_channel::unbounded::<VideoFrame>();
        let (vctl_tx, vctl_rx) = video_control_channel();

        // Control drains first (bias), so cadence, Enable, and orientation land before any AU.
        assert!(vctl_tx.send(VideoControl::SetTimestampStride(4500)));
        assert!(vctl_tx.send(VideoControl::Enable));
        assert!(vctl_tx.send(VideoControl::SetOrientation(1)));
        let our_au = make_au(3000);
        vin_tx.try_send(our_au.clone()).unwrap();
        vin_tx.try_send(our_au).unwrap();

        let eng = CallEngine::new(cfg, Box::new(SequentialTxIds::new())).unwrap();
        futures::executor::block_on(run_call(
            rt,
            transport.clone() as Arc<dyn RelayTransport>,
            relay_rx,
            CallChannels {
                mic: mic_rx,
                speaker: spk_tx,
                encoded_audio_in: async_channel::unbounded::<Bytes>().1,
                encoded_audio_out: async_channel::unbounded::<EncodedAudioFrame>().0,
                events: ev_tx,
                rekey: None,
                video_in: vin_rx,
                video_out: vout_tx,
                video_ctl: vctl_rx,
                group_ctl: None,
            },
            eng,
        ));

        // Inbound: the peer AU reassembled to the sink, orientation stamped from the control arm.
        let frames: Vec<VideoFrame> = std::iter::from_fn(|| vout_rx.try_recv().ok()).collect();
        assert_eq!(frames.len(), 1, "peer AU must reach video_out exactly once");
        assert_eq!(frames[0].data, peer_au);
        assert!(frames[0].keyframe);
        assert_eq!(
            frames[0].orientation, 1,
            "SetOrientation must apply before the inbound AU reassembles"
        );

        // Outbound: each 3KB AU fans out to four PT-97 packets and the 20 fps stride applies.
        let sent = transport.sent.lock().unwrap();
        let video_headers = sent
            .iter()
            .filter_map(|packet| {
                parse_rtp_header(packet).filter(|h| h.payload_type == RTP_PAYLOAD_TYPE_H264)
            })
            .collect::<Vec<_>>();
        assert_eq!(video_headers.len(), 8);
        assert_eq!(
            video_headers
                .iter()
                .filter(|header| header.marker)
                .map(|header| header.timestamp)
                .collect::<Vec<_>>(),
            [0, 4500]
        );
    }

    // A relay stall backs the queue up past cap: the overflow policy must shed media, never the STUN
    // control (keepalive / consent Binding Success) sharing the queue, or relay consent fails.
    #[test]
    fn overflow_sheds_media_and_spares_control() {
        // version 2 + extension bit -> 0x90, classified as Rtp media.
        let media = |seq: u8| Bytes::from(vec![0x90, seq]);
        // Top two bits zero -> STUN control.
        let control = || Bytes::from(vec![0x00, 0x01]);

        let mut q: VecDeque<SendBatch> = VecDeque::new();
        let mut awaiting_keyframe = false;
        q.push_back(SendBatch::packet(control())); // oldest, must survive
        for n in 0..SEND_QUEUE_BATCH_CAP as u8 {
            q.push_back(SendBatch::packet(media(n)));
            let _ = shed_to_cap(&mut q, &mut awaiting_keyframe);
        }

        assert_eq!(q.len(), SEND_QUEUE_BATCH_CAP);
        assert_eq!(
            q[0].kind,
            SendBatchKind::Control,
            "the queued control packet must not be evicted by media backpressure"
        );
        // The oldest media (seq 0) is the one shed, not the control at the front.
        assert_eq!(&q[1].packets[0][..], &[0x90, 1]);
    }

    // Pathological: an all-control queue still has to honor the bound, so it falls back to dropping
    // the oldest.
    #[test]
    fn overflow_all_control_drops_oldest() {
        let mut q: VecDeque<SendBatch> = (0..=SEND_QUEUE_BATCH_CAP as u8)
            .map(|n| SendBatch::packet(Bytes::from(vec![0x00, n])))
            .collect();
        let mut awaiting_keyframe = false;
        let _ = shed_to_cap(&mut q, &mut awaiting_keyframe);
        assert_eq!(q.len(), SEND_QUEUE_BATCH_CAP);
        assert_eq!(
            &q[0].packets[0][..],
            &[0x00, 1],
            "oldest control dropped to keep bound"
        );
    }

    #[test]
    fn large_video_au_is_queued_atomically() {
        fn video_packet(seq: u16, marker: bool) -> Bytes {
            let mut packet = vec![0u8; 16];
            packet[0] = 0x90; // V=2, X=1
            packet[1] = ((marker as u8) << 7) | RTP_PAYLOAD_TYPE_H264;
            packet[2..4].copy_from_slice(&seq.to_be_bytes());
            packet[12..14].copy_from_slice(&0xdebeu16.to_be_bytes());
            Bytes::from(packet)
        }

        // The old 32-datagram queue truncated this AU before its marker.
        let mut queue = VecDeque::new();
        let mut pending = Vec::new();
        let mut awaiting_keyframe = false;
        for seq in 0..40u16 {
            let dropped = queue_transmit(
                &mut queue,
                &mut pending,
                &mut awaiting_keyframe,
                video_packet(seq, seq == 39),
            );
            assert_eq!(dropped.packets, 0);
        }
        assert!(pending.is_empty());
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0].kind, SendBatchKind::Video);
        assert_eq!(queue[0].packets.len(), 40);

        let sent: Vec<u16> = std::iter::from_fn(|| pop_next_packet(&mut queue))
            .map(|(packet, _)| parse_rtp_header(&packet).unwrap().sequence_number)
            .collect();
        assert_eq!(sent, (0..40u16).collect::<Vec<_>>());
    }

    #[test]
    fn epoch_transition_cancels_media_already_removed_from_the_send_queue() {
        struct DropProbe(Arc<AtomicBool>);
        impl Drop for DropProbe {
            fn drop(&mut self) {
                self.0.store(true, Ordering::SeqCst);
            }
        }

        let cancelled = Arc::new(AtomicBool::new(false));
        let probe = DropProbe(cancelled.clone());
        let future: BoxFuture<'static, anyhow::Result<()>> = Box::pin(async move {
            let _probe = probe;
            futures::future::pending().await
        });
        let mut sending = InFlightSend {
            future: future.fuse(),
            kind: Some(SendBatchKind::Media),
        };

        let dropped = cancel_in_flight_group_media(&mut sending, true, false);

        assert!(sending.future.is_terminated());
        assert_eq!(sending.kind, None);
        assert!(cancelled.load(Ordering::SeqCst));
        assert_eq!(dropped.packets, 1);
        assert_eq!(dropped.video_access_units, 0);
    }

    #[test]
    fn oversized_video_is_dropped_as_a_whole_au() {
        let packets = (0..40)
            .map(|_| Bytes::from(vec![0x90; 64 * 1024]))
            .collect();
        let mut queue = VecDeque::new();
        let mut awaiting_keyframe = false;
        let dropped = enqueue_batch(
            &mut queue,
            &mut awaiting_keyframe,
            SendBatch::video(packets),
        );
        assert_eq!(dropped.video_access_units, 1);
        assert_eq!(dropped.packets, 40);
        assert!(queue.is_empty(), "no partial AU may remain queued");
        assert!(awaiting_keyframe);
    }

    #[test]
    fn disabling_purges_only_video_batches_that_have_not_started() {
        let mut started = SendBatch::video(vec![
            Bytes::from_static(b"started-1"),
            Bytes::from_static(b"started-2"),
        ]);
        started.started = true;
        let mut queue = VecDeque::from([
            SendBatch::packet(Bytes::from_static(&[0x90, 0x01])),
            started,
            SendBatch::video(vec![Bytes::from_static(b"stale")]),
            SendBatch::packet(Bytes::from_static(&[0x00, 0x01])),
        ]);
        let mut awaiting_keyframe = false;

        let dropped = purge_unstarted_video(&mut queue, &mut awaiting_keyframe);

        assert_eq!(dropped.video_access_units, 1);
        assert_eq!(dropped.packets, 1);
        assert!(awaiting_keyframe);
        assert_eq!(queue.len(), 3);
        assert!(queue.iter().any(|batch| {
            batch.kind == SendBatchKind::Video && batch.started && batch.packets.len() == 2
        }));
        assert!(
            queue
                .iter()
                .all(|batch| batch.kind != SendBatchKind::Video || batch.started)
        );
    }

    #[test]
    fn video_backpressure_recovers_at_the_next_keyframe() {
        fn video_packet(seq: u16, keyframe: bool) -> Bytes {
            let mut packet = Vec::new();
            crate::voip::rtp::encode_rtp_header_into(
                &crate::voip::rtp::RtpHeader {
                    marker: true,
                    payload_type: RTP_PAYLOAD_TYPE_H264,
                    sequence_number: seq,
                    timestamp: u32::from(seq) * 4500,
                    ssrc: 0x1122_3344,
                    extension_word: None,
                    video_extension: Some(crate::voip::rtp::VideoRtpExtension {
                        media_frame_info: if keyframe {
                            VIDEO_MEDIA_FRAME_INFO_IDR
                        } else {
                            crate::voip::rtp::VIDEO_MEDIA_FRAME_INFO_DELTA
                        },
                        initial_bandwidth: 20_000,
                        short_offset: 0,
                        transport_sequence: seq,
                    }),
                },
                &mut packet,
            );
            Bytes::from(packet)
        }

        let mut queue = VecDeque::new();
        let mut awaiting_keyframe = true;

        let dropped = enqueue_batch(
            &mut queue,
            &mut awaiting_keyframe,
            SendBatch::video(vec![video_packet(1, false)]),
        );
        assert_eq!(dropped.video_access_units, 1);
        assert!(queue.is_empty());

        let dropped = enqueue_batch(
            &mut queue,
            &mut awaiting_keyframe,
            SendBatch::video(vec![video_packet(2, true)]),
        );
        assert_eq!(dropped.video_access_units, 0);
        assert!(!awaiting_keyframe);

        let dropped = enqueue_batch(
            &mut queue,
            &mut awaiting_keyframe,
            SendBatch::video(vec![video_packet(3, false)]),
        );
        assert_eq!(dropped.video_access_units, 0);
        assert_eq!(queue.len(), 2);
        assert!(queue[0].video_keyframe);
        assert!(!queue[1].video_keyframe);

        let mut queued: VecDeque<_> = (0..=SEND_QUEUE_BATCH_CAP as u16)
            .map(|seq| SendBatch::video(vec![video_packet(seq, seq == 10)]))
            .collect();
        let mut awaiting_keyframe = false;
        let dropped = shed_to_cap(&mut queued, &mut awaiting_keyframe);
        assert_eq!(dropped.video_access_units, 10);
        assert!(queued.front().is_some_and(|batch| batch.video_keyframe));
        assert!(!awaiting_keyframe, "a queued IDR is a valid recovery point");
    }

    /// Virtual-time runtime for the deadline-timer tests. `sleep` only records the arm; the clock
    /// advances when the returned future is awaited, exactly like a real runtime, so an armed timer
    /// that never fires cannot move time on its own. Once a sleep elapses at or past `horizon_ms` it
    /// closes the relay, ending the drive loop deterministically.
    struct ScheduleRuntime {
        clock: Arc<AtomicU64>,
        arms: Arc<Mutex<Vec<(u64, u64)>>>,
        relay_tx: async_channel::Sender<RelayTransportEvent>,
        horizon_ms: u64,
    }

    #[async_trait]
    impl Runtime for ScheduleRuntime {
        fn spawn(&self, _f: Pin<Box<dyn Future<Output = ()> + Send + 'static>>) -> AbortHandle {
            AbortHandle::noop()
        }
        fn sleep(&self, d: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            let duration = d.as_millis() as u64;
            let armed_at = self.clock.load(Ordering::Relaxed);
            self.arms.lock().unwrap().push((armed_at, duration));
            let clock = self.clock.clone();
            let relay_tx = self.relay_tx.clone();
            let horizon_ms = self.horizon_ms;
            Box::pin(async move {
                let fired_at = armed_at.saturating_add(duration);
                clock.fetch_max(fired_at, Ordering::Relaxed);
                if fired_at >= horizon_ms {
                    relay_tx.close();
                }
            })
        }
        fn spawn_blocking(
            &self,
            _f: Box<dyn FnOnce() + Send + 'static>,
        ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            Box::pin(async {})
        }
        fn yield_now(&self) -> Option<Pin<Box<dyn Future<Output = ()> + Send>>> {
            None
        }
    }

    /// The distinct deadlines the loop armed for, in order. Rearming for the deadline already armed is
    /// the work this suite's subject removes, so the schedule has to be compared with those collapsed.
    fn armed_deadlines(arms: &[(u64, u64)]) -> Vec<u64> {
        let mut deadlines: Vec<u64> = Vec::new();
        for (armed_at, duration) in arms {
            let deadline = armed_at.saturating_add(*duration);
            if deadlines.last() != Some(&deadline) {
                deadlines.push(deadline);
            }
        }
        deadlines
    }

    /// Relay that timestamps every transmission against the virtual clock and behaves like the real
    /// one: it accepts the first allocate (bringing the media plane and RTCP up) and then probes us
    /// once for consent freshness, which the loop must answer with a Binding Success.
    struct ScheduleRelay {
        clock: Arc<AtomicU64>,
        events: async_channel::Sender<RelayTransportEvent>,
        sent: Mutex<Vec<(u64, Bytes)>>,
        allocates: AtomicUsize,
    }

    #[async_trait]
    impl RelayTransport for ScheduleRelay {
        async fn send(&self, data: Bytes) -> anyhow::Result<()> {
            self.sent
                .lock()
                .unwrap()
                .push((self.clock.load(Ordering::Relaxed), data.clone()));
            if stun::stun_message_type(&data) != Some(stun::MSG_ALLOCATE_REQUEST)
                || self.allocates.fetch_add(1, Ordering::Relaxed) != 0
            {
                return Ok(());
            }
            let transaction_id: [u8; 12] = stun::stun_transaction_id(&data)
                .expect("allocate transaction id")
                .try_into()
                .expect("STUN transaction IDs are 12 bytes");
            let ok = stun::encode_stun_request(
                stun::MSG_ALLOCATE_SUCCESS,
                &transaction_id,
                &[],
                None,
                false,
            );
            let _ = self
                .events
                .try_send(RelayTransportEvent::PacketReceived(Bytes::from(ok)));
            let probe =
                stun::encode_stun_request(stun::MSG_BINDING_REQUEST, &[7u8; 12], &[], None, false);
            let _ = self
                .events
                .try_send(RelayTransportEvent::PacketReceived(Bytes::from(probe)));
            Ok(())
        }
        async fn disconnect(&self) {}
    }

    fn packet_kind(packet: &Bytes) -> &'static str {
        match classify_relay_packet(packet) {
            RelayPacketKind::Rtcp => "rtcp",
            RelayPacketKind::Rtp => "rtp",
            _ => match stun::stun_message_type(packet) {
                Some(stun::MSG_ALLOCATE_REQUEST) => "allocate",
                Some(stun::MSG_BINDING_SUCCESS) => "binding-success",
                _ => "keepalive-ping",
            },
        }
    }

    /// Run a drive loop off the test thread and fail instead of hanging when it never ends: a timer
    /// that stops rearming parks the loop on inputs that never arrive, and a hung test reports
    /// nothing. The timeout is a watchdog, not a pacing device -- the loop runs on virtual time and
    /// finishes in microseconds.
    fn drive_bounded(drive: impl FnOnce() + Send + 'static) {
        let (done_tx, done_rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            drive();
            let _ = done_tx.send(());
        });
        done_rx
            .recv_timeout(Duration::from_secs(30))
            .expect("the drive loop never ended: its deadline timer stopped rearming");
    }

    struct ScheduleHarness {
        clock: Arc<AtomicU64>,
        arms: Arc<Mutex<Vec<(u64, u64)>>>,
        relay: Arc<ScheduleRelay>,
        speaker: async_channel::Receiver<Vec<i16>>,
        video_out: async_channel::Receiver<VideoFrame>,
    }

    /// Drive one audio-only call over virtual time until `horizon_ms`, with the video plane wired but
    /// never enabled.
    fn drive_schedule(horizon_ms: u64) -> ScheduleHarness {
        let clock = Arc::new(AtomicU64::new(0));
        let arms = Arc::new(Mutex::new(Vec::new()));
        let (relay_tx, relay_rx) = async_channel::unbounded();
        let rt: Arc<dyn Runtime> = Arc::new(ScheduleRuntime {
            clock: clock.clone(),
            arms: arms.clone(),
            relay_tx: relay_tx.clone(),
            horizon_ms,
        });
        let relay = Arc::new(ScheduleRelay {
            clock: clock.clone(),
            events: relay_tx,
            sent: Mutex::new(Vec::new()),
            allocates: AtomicUsize::new(0),
        });
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, spk_rx) = async_channel::unbounded();
        let (ev_tx, _ev_rx) = async_channel::unbounded();
        let (vout_tx, vout_rx) = async_channel::unbounded::<VideoFrame>();
        let mut channels = test_channels(mic_rx, spk_tx, ev_tx);
        channels.video_out = vout_tx;

        let eng = CallEngine::new(config(), Box::new(SequentialTxIds::new())).unwrap();
        let drive_relay = relay.clone();
        let drive_clock = clock.clone();
        drive_bounded(move || {
            futures::executor::block_on(run_call_with_clock(
                rt,
                drive_relay as Arc<dyn RelayTransport>,
                relay_rx,
                channels,
                eng,
                move || drive_clock.load(Ordering::Relaxed),
            ));
        });

        ScheduleHarness {
            clock,
            arms,
            relay,
            speaker: spk_rx,
            video_out: vout_rx,
        }
    }

    // The happy path for the hoisted deadline timer: an armed sleep that survives across iterations
    // must not shift a single tick. Both sequences below are invariant to how often the loop rearms --
    // the deadlines it arms for and the stanzas the peer sees at each instant -- so they pin the
    // schedule (playout every 20ms, keepalive every 1s, RTCP every 1.5s, consent answered inline)
    // rather than the arming policy.
    #[test]
    fn hoisted_timer_keeps_the_call_schedule() {
        const HORIZON_MS: u64 = 2_600;
        let harness = drive_schedule(HORIZON_MS);

        let deadlines = armed_deadlines(&harness.arms.lock().unwrap());
        let playout_ticks: Vec<u64> = (1..=HORIZON_MS / engine::PLAYOUT_MS)
            .map(|n| n * engine::PLAYOUT_MS)
            .collect();
        assert!(
            deadlines.starts_with(&playout_ticks),
            "every 20ms playout tick must be armed for, once, in order; got {deadlines:?}"
        );
        // Teardown may add one arm past the horizon: the fire that closed the relay rearms before the
        // next select observes the closed channel.
        assert!(deadlines.len() <= playout_ticks.len() + 1);

        let stanzas: Vec<(u64, &'static str)> = harness
            .relay
            .sent
            .lock()
            .unwrap()
            .iter()
            .map(|(at, packet)| (*at, packet_kind(packet)))
            .collect();
        assert_eq!(
            stanzas,
            [
                (0, "allocate"),
                (0, "rtcp"),
                (0, "binding-success"),
                (1000, "allocate"),
                (1000, "keepalive-ping"),
                (1500, "rtcp"),
                (2000, "allocate"),
                (2000, "keepalive-ping"),
            ],
            "the peer-visible schedule must not move"
        );

        assert_eq!(
            std::iter::from_fn(|| harness.speaker.try_recv().ok()).count() as u64,
            HORIZON_MS / engine::PLAYOUT_MS,
            "one playout frame per 20ms tick"
        );
        assert_eq!(harness.clock.load(Ordering::Relaxed), HORIZON_MS);
    }

    // A call with no video must stay entirely on the audio plane: nothing reaches the video sink and
    // no H.264 packet is transmitted, while the audio schedule keeps running. The video-control fast
    // path runs on every iteration of this call, so this is also its empty-queue case.
    #[test]
    fn audio_only_call_never_touches_the_video_plane() {
        let harness = drive_schedule(1_200);

        assert!(
            harness.video_out.try_recv().is_err(),
            "an audio-only call must not produce video frames"
        );
        let sent = harness.relay.sent.lock().unwrap();
        assert!(
            !sent.iter().any(|(_, packet)| parse_rtp_header(packet)
                .is_some_and(|header| header.payload_type == RTP_PAYLOAD_TYPE_H264)),
            "an audio-only call must not transmit H.264"
        );
        assert!(
            sent.iter()
                .filter(|(_, packet)| stun::stun_message_type(packet)
                    == Some(stun::MSG_ALLOCATE_REQUEST))
                .count()
                >= 2,
            "the keepalive must keep running on an audio-only call"
        );
    }

    // Regression: the timer has to rearm after it fires. A `Fuse` that completed never resolves again,
    // so a loop that reuses the armed sleep without rearming goes silent -- no playout, no keepalive,
    // no consent -- and the relay drops the call. Two consecutive keepalive deadlines a second apart
    // prove the rearm; with the bug the drive never reaches the second one at all.
    #[test]
    fn timer_rearms_after_every_fire() {
        let harness = drive_schedule(2_200);

        let deadlines = armed_deadlines(&harness.arms.lock().unwrap());
        let keepalives: Vec<u64> = deadlines
            .iter()
            .copied()
            .filter(|deadline| deadline % 1_000 == 0)
            .collect();
        assert_eq!(
            keepalives,
            [1_000, 2_000],
            "the timer must rearm past its first deadline and keep the keepalive cadence"
        );
        assert_eq!(
            harness
                .relay
                .sent
                .lock()
                .unwrap()
                .iter()
                .filter(|(_, packet)| stun::stun_message_type(packet)
                    == Some(stun::MSG_ALLOCATE_REQUEST))
                .map(|(at, _)| *at)
                .collect::<Vec<_>>(),
            [0, 1_000, 2_000],
            "each keepalive deadline must put its allocate on the wire"
        );
    }

    /// Relay whose reconnect hands back a replacement that immediately reports Disconnected. It
    /// records the virtual instant of the migration and how many timers had been armed by then, so
    /// the test can look only at what the loop armed *after* it.
    struct ClockedReconnectTransport {
        clock: Arc<AtomicU64>,
        arms: Arc<Mutex<Vec<(u64, u64)>>>,
        migration: Mutex<Option<(u64, usize)>>,
        replacement: Mutex<Option<ReplacementRelay>>,
    }

    #[async_trait]
    impl RelayTransport for ClockedReconnectTransport {
        async fn send(&self, _data: Bytes) -> anyhow::Result<()> {
            Ok(())
        }
        async fn disconnect(&self) {}
        async fn reconnect(
            &self,
            _endpoint: std::net::SocketAddr,
        ) -> anyhow::Result<ReplacementRelay> {
            *self.migration.lock().unwrap() = Some((
                self.clock.load(Ordering::Relaxed),
                self.arms.lock().unwrap().len(),
            ));
            self.replacement
                .lock()
                .unwrap()
                .take()
                .ok_or_else(|| anyhow::anyhow!("one reconnect only"))
        }
    }

    /// Runtime for the migration test: virtual time as above, plus it hands the driver its relay
    /// migration once the clock has passed `migrate_at_ms`, so the reconnect lands mid-call with a
    /// timer already armed for the old schedule.
    struct MigrationRuntime {
        clock: Arc<AtomicU64>,
        arms: Arc<Mutex<Vec<(u64, u64)>>>,
        group_tx: async_channel::Sender<GroupControl>,
        migrate_at_ms: u64,
        migrated: Arc<AtomicBool>,
    }

    #[async_trait]
    impl Runtime for MigrationRuntime {
        fn spawn(&self, _f: Pin<Box<dyn Future<Output = ()> + Send + 'static>>) -> AbortHandle {
            AbortHandle::noop()
        }
        fn sleep(&self, d: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            let duration = d.as_millis() as u64;
            let armed_at = self.clock.load(Ordering::Relaxed);
            self.arms.lock().unwrap().push((armed_at, duration));
            let clock = self.clock.clone();
            let group_tx = self.group_tx.clone();
            let migrate_at_ms = self.migrate_at_ms;
            let migrated = self.migrated.clone();
            Box::pin(async move {
                let fired_at = armed_at.saturating_add(duration);
                clock.fetch_max(fired_at, Ordering::Relaxed);
                if fired_at >= migrate_at_ms && !migrated.swap(true, Ordering::Relaxed) {
                    let _ = group_tx.try_send(GroupControl::Update(Box::new(group_update(
                        8,
                        "203.0.113.8",
                        3481,
                    ))));
                }
            })
        }
        fn spawn_blocking(
            &self,
            _f: Box<dyn FnOnce() + Send + 'static>,
        ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
            Box::pin(async {})
        }
        fn yield_now(&self) -> Option<Pin<Box<dyn Future<Output = ()> + Send>>> {
            None
        }
    }

    // Regression: a mid-call relay migration restarts every deadline from the reconnect instant, so an
    // armed sleep still pointing at the retired relay's schedule has to go. If the loop kept it, the
    // next fire would land on a deadline the engine no longer holds and the fresh keepalive/playout
    // cadence would be off by however long the old timer had left.
    #[test]
    fn relay_reconnect_rearms_the_timer_from_the_new_schedule() {
        const MIGRATE_AT_MS: u64 = 500;
        let clock = Arc::new(AtomicU64::new(0));
        let arms = Arc::new(Mutex::new(Vec::new()));
        let (group_tx, group_rx) = async_channel::bounded(1);
        let rt: Arc<dyn Runtime> = Arc::new(MigrationRuntime {
            clock: clock.clone(),
            arms: arms.clone(),
            group_tx,
            migrate_at_ms: MIGRATE_AT_MS,
            migrated: Arc::new(AtomicBool::new(false)),
        });

        let replacement = Arc::new(RecordingTransport::default());
        let (replacement_tx, replacement_rx) = async_channel::unbounded();
        replacement_tx
            .try_send(RelayTransportEvent::Disconnected(
                RelayDisconnectReason::Closed,
            ))
            .unwrap();
        let transport = Arc::new(ClockedReconnectTransport {
            clock: clock.clone(),
            arms: arms.clone(),
            migration: Mutex::new(None),
            replacement: Mutex::new(Some((
                replacement as Arc<dyn RelayTransport>,
                replacement_rx,
            ))),
        });

        let (_relay_tx, relay_rx) = async_channel::unbounded();
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, _spk_rx) = async_channel::unbounded();
        let (ev_tx, _ev_rx) = async_channel::unbounded();
        let mut channels = test_channels(mic_rx, spk_tx, ev_tx);
        channels.group_ctl = Some(group_rx);

        let drive_transport = transport.clone();
        let drive_clock = clock.clone();
        drive_bounded(move || {
            futures::executor::block_on(run_call_with_clock(
                rt,
                drive_transport as Arc<dyn RelayTransport>,
                relay_rx,
                channels,
                group_engine(),
                move || drive_clock.load(Ordering::Relaxed),
            ));
        });

        let (reconnect_at, arms_before) = transport
            .migration
            .lock()
            .unwrap()
            .expect("the group update must migrate the relay");
        assert_eq!(reconnect_at, MIGRATE_AT_MS);
        let after_migration = arms
            .lock()
            .unwrap()
            .iter()
            .skip(arms_before)
            .copied()
            // The migration also arms its own bounded reconnect/disconnect timeouts; they are not the
            // engine's deadline timer.
            .find(|(_, duration)| *duration != RELAY_RECONNECT_TIMEOUT.as_millis() as u64)
            .expect("the loop must arm the deadline timer again after the migration");
        assert_eq!(
            after_migration,
            (reconnect_at, engine::PLAYOUT_MS),
            "the first timer after a migration must be a full interval measured from the reconnect"
        );
    }

    // Item 2's failure case: the lock-free empty check must never swallow an orientation that is
    // actually queued, including one enqueued concurrently with the `try_recv` sitting in that check.
    // The sender fills the map before publishing its wake marker, so the reader either sees the
    // counter or is woken by the marker; a reader that trusted a stale zero forever would drop the
    // participant's rotation for the rest of the call.
    #[test]
    fn queued_participant_orientation_survives_the_lock_free_empty_check() {
        const ROUNDS: u8 = 64;
        let participants: Vec<Jid> = (0..4)
            .map(|n| Jid::new(format!("20000{n}"), Server::Lid).with_device(3))
            .collect();

        let (tx, rx) = video_control_channel();
        let sender_participants = participants.clone();
        let sender = std::thread::spawn(move || {
            for round in 0..ROUNDS {
                for participant in &sender_participants {
                    assert!(tx.send(VideoControl::SetParticipantOrientation {
                        participant: participant.clone(),
                        orientation: round % 4,
                    }));
                }
            }
        });

        // Race the sender: most of these calls land in the empty fast path, which is the point.
        let mut observed: HashMap<Jid, u8> = HashMap::new();
        let deadline = Instant::now() + Duration::from_secs(30);
        loop {
            match rx.try_recv() {
                Ok(VideoControl::SetParticipantOrientation {
                    participant,
                    orientation,
                }) => {
                    observed.insert(participant, orientation);
                }
                Ok(other) => panic!("unexpected control {other:?}"),
                Err(_) => {
                    if sender.is_finished() && observed.len() == participants.len() {
                        break;
                    }
                    assert!(
                        Instant::now() < deadline,
                        "an orientation was swallowed by the empty fast path: saw {observed:?}"
                    );
                    std::thread::yield_now();
                }
            }
        }
        sender.join().expect("sender thread");

        // Drain whatever the last round left behind, then every participant must carry its last value.
        while let Ok(VideoControl::SetParticipantOrientation {
            participant,
            orientation,
        }) = rx.try_recv()
        {
            observed.insert(participant, orientation);
        }
        let last = (ROUNDS - 1) % 4;
        for participant in &participants {
            assert_eq!(
                observed.get(participant),
                Some(&last),
                "the final orientation for {participant} must reach the drive loop"
            );
        }
    }
}

//! The resident media session: the control plane's handle over one running call.
//!
//! `run_call` (in the engine half) owns the media engine for the life of a call, on
//! one task, so the control plane never shares the engine. It holds bounded mailboxes into that
//! task, and this type is the neutral front for them: it implements [`VoipMediaSession`] in terms
//! of [`MediaCommand`] and
//! [`MediaStats`], translating each command back into the driver
//! control the mailboxes already carry.
//!
//! That is the answer to the `Send`-but-not-`Sync` engine. `CallEngine` holds
//! `Box<dyn ForeignAudioCodec>`, whose trait is bounded `MaybeSend` and deliberately not `Sync`
//! (libopus's decoder is `Send` but not `Sync`). Presenting the engine as a shared session object
//! would force a `Mutex<CallEngine>` around a value that is already single-owner. Instead the
//! engine keeps its exclusive owner -- the drive task -- and the session is a handle into it, so
//! the seam requirement stays `MaybeSendSync` without a lock on the media path. A foreign backend
//! that reuses the codec traits inherits the same bound inside its own process, where it can make
//! the same choice.
//!
//! The registry's media command fields are absorbed here: the recv-rekey and video-control
//! mailboxes, the lossless group-control queue with its byte-budgeted coalescing, the epoch
//! retained before media attaches, the consumer-facing event stream, the counters cell, and the
//! media-task abort handle. The control plane keeps what is its own -- signaling state, close
//! reasons, generation guards, and the teardown hook.

use std::mem::size_of;
use std::sync::{Arc, Mutex};

use crate::types::group_call::GroupCallUpdate;
use crate::voip_control::control::VideoControlSender;
use crate::voip_control::control::{DEFAULT_CALL_EVENT_QUEUE_CAPACITY, GroupControlQueue};
use crate::voip_control::control::{GroupControl, GroupRawEpoch, PeerAnswer, VideoControl};
#[cfg(any(test, feature = "test-util"))]
use crate::voip_control::control::{VideoControlReceiver, video_control_channel};
use crate::voip_control::media_stats::MediaStatsCell;
use crate::voip_control::{
    CallDirection, MediaCommand, MediaEvent, MediaKeyframeUrgency, MediaSessionKey,
    MediaSessionSpec, MediaSetupError, MediaStats, VoipMediaBackend, VoipMediaSession,
};

/// The core group control a neutral command carries, when it carries one.
pub(crate) fn group_control_of(command: &MediaCommand) -> Option<GroupControl> {
    match command {
        MediaCommand::ApplyGroupUpdate(update) => Some(GroupControl::Update(update.clone())),
        MediaCommand::ApplyGroupTransition(transition) => Some(GroupControl::Transition {
            update: transition.update.clone(),
            epoch: GroupRawEpoch::new(
                transition.transaction_id,
                transition.raw_epoch.as_bytes().to_vec(),
            ),
        }),
        MediaCommand::ApplyGroupEpoch {
            transaction_id,
            raw_epoch,
        } => Some(GroupControl::RawEpoch(GroupRawEpoch::new(
            *transaction_id,
            raw_epoch.as_bytes().to_vec(),
        ))),
        MediaCommand::SendGroupReaction(emoji) => Some(GroupControl::Reaction(emoji.clone())),
        _ => None,
    }
}

fn retained_epoch(control: GroupControl) -> Option<GroupRawEpoch> {
    match control {
        GroupControl::Transition { epoch, .. } | GroupControl::RawEpoch(epoch) => Some(epoch),
        GroupControl::Update(_) | GroupControl::Reaction(_) => None,
    }
}

/// The neutral command for a driver-level video control.
///
/// The registry builds a [`VideoControl`] because the participant-vs-plane decision lives on the
/// entry; this keeps the seam from growing a second parallel vocabulary for the same states.
pub(crate) fn video_control_to_command(control: VideoControl) -> MediaCommand {
    match control {
        VideoControl::SetInputGeneration(generation) => {
            MediaCommand::SetVideoInputGeneration(generation)
        }
        VideoControl::SetTimestampStride(stride) => MediaCommand::SetVideoTimestampStride(stride),
        VideoControl::Enable => MediaCommand::EnableVideo {
            awaiting_accept: false,
        },
        VideoControl::EnableAwaitingAccept => MediaCommand::EnableVideo {
            awaiting_accept: true,
        },
        VideoControl::Disable => MediaCommand::DisableVideo { keep_legacy: false },
        VideoControl::DisableKeepLegacy => MediaCommand::DisableVideo { keep_legacy: true },
        VideoControl::DisableOutbound => MediaCommand::DisableVideoOutbound,
        VideoControl::RequireKeyframe => MediaCommand::RequireVideoKeyframe,
        VideoControl::RequestPeerKeyframe(urgency) => {
            MediaCommand::RequestPeerKeyframe(match urgency {
                MediaKeyframeUrgency::Coalesced => MediaKeyframeUrgency::Coalesced,
                MediaKeyframeUrgency::Immediate => MediaKeyframeUrgency::Immediate,
            })
        }
        VideoControl::SetOrientation(orientation) => MediaCommand::SetVideoOrientation {
            participant: None,
            orientation,
        },
        VideoControl::SetParticipantOrientation {
            participant,
            orientation,
        } => MediaCommand::SetVideoOrientation {
            participant: Some(participant),
            orientation,
        },
        // A new driver-level video control must be given a neutral spelling here, or the seam
        // would silently drop it. The exhaustive match is the guard.
    }
}

/// Mailboxes into one running drive task, attached incrementally as a call is set up.
#[derive(Default)]
struct Mailboxes {
    video: Option<VideoControlSender>,
    group: Option<GroupControlQueue>,
    // The drive task's abort handle, if this session was handed one. `close` aborts it so a
    // terminal registry entry ends the media task without the registry holding a parallel handle.
    media_task: Option<crate::runtime::AbortHandle>,
    /// A decrypted epoch that arrived before relay media attached. Replacing or dropping this
    /// erases its key bytes through [`GroupRawEpoch`]'s `Drop`.
    pending_group_epoch: Option<GroupRawEpoch>,
}

/// One call's media session, resident in the same process as the drive task.
pub struct ResidentMediaSession {
    mailboxes: Mutex<Mailboxes>,
    stats: Mutex<Arc<MediaStatsCell>>,
    /// The public event stream for this call: the sender the drive loop and the control plane
    /// publish through, and the receiver every [`subscribe`](VoipMediaSession::subscribe) clones.
    /// One session has one ordered stream a consumer (and the `CallHandle`) reads, whether it is the
    /// resident engine or a foreign one. Interior-mutable so tests can swap the sender before they
    /// subscribe; production keeps the reservation-time pair for the life of the call.
    events: Mutex<(
        async_channel::Sender<MediaEvent>,
        async_channel::Receiver<MediaEvent>,
    )>,
    /// The one-shot recv-rekey channel, created at reservation so a callee `<accept>` that races
    /// ahead of the relay is buffered (bounded(1)) rather than lost. The receiver is handed to the
    /// drive loop through [`take_rekey_receiver`](Self::take_rekey_receiver).
    rekey_tx: async_channel::Sender<PeerAnswer>,
    rekey_rx: Mutex<Option<async_channel::Receiver<PeerAnswer>>>,
    /// Test-only weak self, so the registry's test accessor can hand an owned handle back without
    /// production code holding the concrete type.
    #[cfg(any(test, feature = "test-util"))]
    self_weak: std::sync::OnceLock<std::sync::Weak<Self>>,
}

impl ResidentMediaSession {
    /// A session that publishes through a fresh counter cell.
    #[must_use]
    pub fn new() -> Arc<Self> {
        Self::with_stats(Arc::new(MediaStatsCell::default()))
    }

    /// A session that publishes through `stats`, shared with the `CallHandle`.
    #[must_use]
    fn with_stats(stats: Arc<MediaStatsCell>) -> Arc<Self> {
        let (events_tx, events_rx) = async_channel::bounded(DEFAULT_CALL_EVENT_QUEUE_CAPACITY);
        let (rekey_tx, rekey_rx) = async_channel::bounded(1);
        let session = Self {
            mailboxes: Mutex::new(Mailboxes::default()),
            stats: Mutex::new(stats),
            events: Mutex::new((events_tx, events_rx)),
            rekey_tx,
            rekey_rx: Mutex::new(Some(rekey_rx)),
            #[cfg(any(test, feature = "test-util"))]
            self_weak: std::sync::OnceLock::new(),
        };
        let session = Arc::new(session);
        #[cfg(any(test, feature = "test-util"))]
        let _ = session.self_weak.set(Arc::downgrade(&session));
        session
    }

    /// Test-only: an owned handle to this session.
    #[cfg(any(test, feature = "test-util"))]
    #[must_use]
    pub fn resident_arc(&self) -> Option<Arc<Self>> {
        self.self_weak.get().and_then(|weak| weak.upgrade())
    }

    /// The event sender the drive loop publishes through.
    #[must_use]
    pub fn event_sender(&self) -> async_channel::Sender<MediaEvent> {
        self.events().0.clone()
    }

    fn events(
        &self,
    ) -> std::sync::MutexGuard<
        '_,
        (
            async_channel::Sender<MediaEvent>,
            async_channel::Receiver<MediaEvent>,
        ),
    > {
        self.events
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// Test-only: reached through the test-gated registry setters.
    #[cfg(any(test, feature = "test-util"))]
    /// Create and install this session's video-control mailbox, returning the drive-loop half.
    ///
    /// Concrete (not on the trait) because only the resident backend wires a `run_call` loop; a
    /// foreign backend owns its own video path. The session keeps the sender so `submit` reaches the
    /// loop; the returned receiver is handed to `CallChannels`.
    #[must_use]
    pub fn install_video_channel(&self) -> VideoControlReceiver {
        let (tx, rx) = video_control_channel();
        self.set_video_sender(tx);
        rx
    }

    /// Create and install this session's group-control mailbox, returning the drive-loop half.
    ///
    /// The committed roster and any retained epoch are replayed when the sender is installed, and a
    /// `warp_mi_tag_len` that changed under an established relay is refused (`None`), the same policy
    /// the registry's attach-time replay applied.
    #[must_use]
    pub fn install_group_channel(
        &self,
        warp_mi_tag_len: Option<usize>,
        committed: Option<GroupCallUpdate>,
        established_warp_mi_tag_len: Option<usize>,
    ) -> Option<async_channel::Receiver<GroupControl>> {
        let (tx, rx) = async_channel::bounded(DEFAULT_CALL_EVENT_QUEUE_CAPACITY);
        self.set_group_sender(tx, warp_mi_tag_len, committed, established_warp_mi_tag_len)
            .then_some(rx)
    }

    /// A fresh counter cell, installed on this session and returned for the `CallHandle` to hold.
    #[must_use]
    pub fn install_fresh_stats_cell(&self) -> Arc<MediaStatsCell> {
        let cell = Arc::new(MediaStatsCell::default());
        self.set_stats_cell(cell.clone());
        cell
    }

    /// Take ownership of the drive task's abort handle.
    ///
    /// Concrete, not on the trait: the backend that spawned the loop owns the task, and `close`
    /// aborts it. The control plane never sees this handle (F6/F14).
    pub fn install_drive_task(&self, handle: crate::runtime::AbortHandle) {
        self.mailboxes().media_task = Some(handle);
    }

    /// Test-only: reached through the test-gated registry setters.
    #[cfg(any(test, feature = "test-util"))]
    /// Swap the public stream's sender, for tests that install their own sink before subscribing.
    /// Production never calls this: the handle subscribes to the reservation-time stream, and a
    /// swap after that would orphan its receiver.
    pub fn install_event_sender(&self, tx: async_channel::Sender<MediaEvent>) {
        self.events().0 = tx;
    }

    /// Take the one-shot recv-rekey receiver for the drive loop.
    #[must_use]
    pub fn take_rekey_receiver(&self) -> Option<async_channel::Receiver<PeerAnswer>> {
        self.rekey_rx
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take()
    }

    /// Install the video-control sender the shell steers the plane through.
    pub fn install_video_sender(&self, tx: VideoControlSender) {
        self.set_video_sender(tx);
    }

    /// Test-only: reached through the test-gated registry setters.
    #[cfg(any(test, feature = "test-util"))]
    /// Install the shared counter cell the drive loop publishes into.
    pub fn install_stats_cell(&self, cell: Arc<MediaStatsCell>) {
        self.set_stats_cell(cell);
    }

    /// Test-only: reached through the test-gated registry setters.
    #[cfg(any(test, feature = "test-util"))]
    /// Install the group-control mailbox and replay the retained startup state.
    pub fn install_group_sender(
        &self,
        tx: async_channel::Sender<GroupControl>,
        warp_mi_tag_len: Option<usize>,
        committed: Option<GroupCallUpdate>,
        established_warp_mi_tag_len: Option<usize>,
    ) -> bool {
        self.set_group_sender(tx, warp_mi_tag_len, committed, established_warp_mi_tag_len)
    }

    /// Test-only: reached through the test-gated registry setters.
    #[cfg(any(test, feature = "test-util"))]
    /// Install the drive task's abort handle (alias of [`install_drive_task`](Self::install_drive_task)).
    pub fn install_media_task(&self, handle: crate::runtime::AbortHandle) {
        self.install_drive_task(handle);
    }

    fn mailboxes(&self) -> std::sync::MutexGuard<'_, Mailboxes> {
        self.mailboxes
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn stats_cell(&self) -> Arc<MediaStatsCell> {
        self.stats
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    /// Attach the video-control mailbox.
    pub(crate) fn set_video_sender(&self, tx: VideoControlSender) {
        self.mailboxes().video = Some(tx);
    }

    /// Attach the group-control mailbox and replay the retained startup state.
    ///
    /// This is the old registry `set_group_control_sender` policy, moved with the mailbox it guards:
    /// a `warp_mi_tag_len` that changed under an established relay is rejected, the latest
    /// committed roster plus a retained epoch replay as one indivisible transition when both exist,
    /// and a trailing epoch is retained when the queue cannot take it.
    pub(crate) fn set_group_sender(
        &self,
        tx: async_channel::Sender<GroupControl>,
        warp_mi_tag_len: Option<usize>,
        committed: Option<GroupCallUpdate>,
        established_warp_mi_tag_len: Option<usize>,
    ) -> bool {
        if let (Some(established), Some(relay_len)) = (warp_mi_tag_len, established_warp_mi_tag_len)
            && relay_len != established
        {
            return false;
        }
        let tx = GroupControlQueue::new(tx);
        let mut mailboxes = self.mailboxes();
        let pending_epoch = mailboxes.pending_group_epoch.take();
        let mut unqueued_epoch = None;
        let queued = match (committed, pending_epoch) {
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
            mailboxes.pending_group_epoch = unqueued_epoch;
            return false;
        }
        mailboxes.group = Some(tx);
        true
    }

    /// Install the counter cell the drive loop publishes into.
    ///
    /// The cell is created by the control plane and shared with the `CallHandle`; the session reads
    /// the same one so `VoipMediaSession::stats` reports the live call rather than a private zero.
    pub(crate) fn set_stats_cell(&self, cell: Arc<MediaStatsCell>) {
        *self
            .stats
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner()) = cell;
    }

    fn send_video(&self, control: VideoControl) -> bool {
        self.mailboxes()
            .video
            .as_ref()
            .is_some_and(|tx| tx.send(control))
    }
}

impl VoipMediaSession for ResidentMediaSession {
    fn submit(&self, command: MediaCommand) -> bool {
        if let Some(control) = group_control_of(&command) {
            let mailboxes = self.mailboxes();
            return mailboxes
                .group
                .as_ref()
                .is_some_and(|tx| tx.try_send(control));
        }
        match command {
            MediaCommand::EnableVideo { awaiting_accept } => self.send_video(if awaiting_accept {
                VideoControl::EnableAwaitingAccept
            } else {
                VideoControl::Enable
            }),
            MediaCommand::DisableVideo { keep_legacy } => self.send_video(if keep_legacy {
                VideoControl::DisableKeepLegacy
            } else {
                VideoControl::Disable
            }),
            MediaCommand::DisableVideoOutbound => self.send_video(VideoControl::DisableOutbound),
            MediaCommand::RequireVideoKeyframe => self.send_video(VideoControl::RequireKeyframe),
            MediaCommand::RequestPeerKeyframe(urgency) => {
                self.send_video(VideoControl::RequestPeerKeyframe(urgency))
            }
            MediaCommand::SetVideoOrientation {
                participant,
                orientation,
            } => match participant {
                Some(participant) => self.send_video(VideoControl::SetParticipantOrientation {
                    participant,
                    orientation,
                }),
                None => self.send_video(VideoControl::SetOrientation(orientation)),
            },
            MediaCommand::SetVideoInputGeneration(generation) => {
                self.send_video(VideoControl::SetInputGeneration(generation))
            }
            MediaCommand::SetVideoTimestampStride(stride) => {
                self.send_video(VideoControl::SetTimestampStride(stride))
            }
            MediaCommand::RekeyRecv {
                answering_lid,
                audio_codec,
            } => {
                // First answerer wins: a duplicate or late `<accept>` from another device finds the
                // receiver already taken by the drive loop and is a no-op. Buffered by bounded(1)
                // when media has not attached yet.
                self.rekey_tx
                    .try_send(PeerAnswer {
                        answering_lid,
                        audio_codec,
                    })
                    .is_ok()
            }
            MediaCommand::ApplyGroupUpdate(_)
            | MediaCommand::ApplyGroupTransition(_)
            | MediaCommand::ApplyGroupEpoch { .. }
            | MediaCommand::SendGroupReaction(_) => unreachable!("group commands handled above"),
        }
    }

    fn submit_lossless(&self, command: MediaCommand) -> bool {
        let Some(control) = group_control_of(&command) else {
            return self.submit(command);
        };
        let mailboxes = self.mailboxes();
        let Some(tx) = mailboxes.group.as_ref() else {
            return false;
        };
        tx.force_send_preserving_epoch(control)
    }

    fn deliver_group_update(&self, update: Box<GroupCallUpdate>) -> bool {
        let mailboxes = self.mailboxes();
        let Some(tx) = mailboxes.group.as_ref() else {
            // No media attached yet: the registry's own committed entry is the retained delivery,
            // and `set_group_sender` replays it at attach. Report success so signaling does not
            // discard the transaction.
            return true;
        };
        tx.force_send_preserving_epoch(GroupControl::Update(update))
    }

    fn deliver_group_epoch(
        &self,
        transaction_id: u32,
        raw_epoch: crate::voip_control::MediaGroupEpoch,
        committed: Option<GroupCallUpdate>,
    ) -> bool {
        let epoch = GroupRawEpoch::new(transaction_id, raw_epoch.into_bytes());
        let mut mailboxes = self.mailboxes();
        let Some(tx) = mailboxes.group.clone() else {
            // Retain the newest epoch until media attaches; `set_group_sender` pairs it with the
            // committed roster and replays both as one indivisible transition.
            let replace = mailboxes
                .pending_group_epoch
                .as_ref()
                .is_none_or(|pending| epoch.transaction_id > pending.transaction_id);
            if replace {
                mailboxes.pending_group_epoch = Some(epoch);
            }
            return true;
        };
        drop(mailboxes);
        let command = match committed {
            Some(update) => GroupControl::Transition {
                update: Box::new(update),
                epoch,
            },
            None => GroupControl::RawEpoch(epoch),
        };
        tx.force_send_preserving_epoch(command)
    }

    fn group_update_fits(&self, update: &GroupCallUpdate, is_call_link: bool) -> bool {
        let control = GroupControl::Update(Box::new(update.clone()));
        let mailboxes = self.mailboxes();
        match mailboxes.group.as_ref() {
            Some(tx) => tx.accepts(&control),
            None => {
                !is_call_link
                    || GroupControlQueue::accepts_with_capacity(
                        &control,
                        DEFAULT_CALL_EVENT_QUEUE_CAPACITY,
                    )
            }
        }
    }

    fn pending_group_epoch(&self) -> Option<u32> {
        self.mailboxes()
            .pending_group_epoch
            .as_ref()
            .map(|epoch| epoch.transaction_id)
    }

    fn retained_bytes(&self) -> usize {
        let mailboxes = self.mailboxes();
        self.rekey_tx
            .len()
            .saturating_mul(size_of::<PeerAnswer>())
            .saturating_add(
                mailboxes
                    .video
                    .as_ref()
                    .map_or(0, VideoControlSender::retained_bytes),
            )
            .saturating_add(
                mailboxes
                    .group
                    .as_ref()
                    .map_or(0, GroupControlQueue::retained_bytes),
            )
            .saturating_add(
                mailboxes
                    .pending_group_epoch
                    .as_ref()
                    .map_or(0, GroupRawEpoch::heap_bytes),
            )
    }

    fn stats(&self) -> MediaStats {
        self.stats_cell().snapshot()
    }

    fn subscribe(&self) -> async_channel::Receiver<MediaEvent> {
        // A clone over the same bounded stream: every subscriber sees the published events, and the
        // drive loop publishes through `event_sender` into this one.
        self.events().1.clone()
    }

    fn publish(&self, event: MediaEvent) -> bool {
        // The control plane surfaces a signaling event (a peer video-state change, a group-control
        // answer) on the same ordered stream the backend's media events travel.
        crate::voip_control::registry::publish_call_event(&self.events().0, event)
    }

    #[cfg(any(test, feature = "test-util"))]
    fn as_any(&self) -> Option<&dyn core::any::Any> {
        Some(self)
    }

    fn close(&self, _reason: crate::voip_control::MediaCloseReason) {
        // Take and abort the drive task, off-lock: aborting drops the future, which drops the
        // transport. Idempotent because the handle is taken once.
        let task = self.mailboxes().media_task.take();
        drop(task);
        // Release every mailbox the drive loop may still hold the far half of: dropping the
        // senders closes the channels, so a parked loop wakes and ends instead of lingering on a
        // dead call. Dropping the retained epoch erases its key bytes through `GroupRawEpoch`'s
        // `Drop`. Idempotent: each handle is taken once.
        let mut mailboxes = self.mailboxes();
        mailboxes.video.take();
        mailboxes.group.take();
        mailboxes.pending_group_epoch.take();
        drop(mailboxes);
        // Drop the rekey receiver nobody will now take, and close the sender so a late answer
        // fails instead of queueing key material for a dead call.
        self.rekey_rx
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .take();
        self.rekey_tx.close();
        // Then close the public stream. The entry-owned queue this session replaced closed when
        // the entry dropped, so a lingering handle's `recv` ends instead of parking: buffered
        // events still drain, and a publish racing the close fails cleanly like a closed queue.
        self.events().0.close();
    }
}

/// The backend a `CallRegistry` carries when none is injected.
///
/// It reserves a [`ResidentMediaSession`] so commands that arrive before a real backend is installed
/// still have somewhere to land, but it carries no engine: [`open`](VoipMediaBackend::open) returns
/// the typed [`MediaSetupError::NoBackend`]. That is the honest answer for a `voip-control`-only
/// build -- the call flow compiles, and starting media without an injected backend fails with a
/// clear error rather than a panic or a silent no-op. A `whatsapp-rust` client installs
/// [`WacoreVoipMediaBackend`](crate::voip_control) instead.
#[derive(Default)]
pub struct NoMediaBackend;

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl VoipMediaBackend for NoMediaBackend {
    fn reserve(
        &self,
        _key: &MediaSessionKey,
        _direction: CallDirection,
    ) -> Arc<dyn VoipMediaSession> {
        ResidentMediaSession::new()
    }

    async fn open(
        &self,
        _spec: MediaSessionSpec,
        _ctx: crate::voip_control::MediaOpenContext,
    ) -> Result<(), MediaSetupError> {
        // The registry's fallback carries no engine, so it cannot start media. `whatsapp-rust`
        // injects `WacoreVoipMediaBackend`, which owns the runtime and transport and does the real
        // open; reaching this one on a live call is the typed `NoBackend` refusal the caller sees.
        Err(MediaSetupError::NoBackend)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voip_control::control::video_control_channel;

    #[tokio::test]
    async fn the_default_backend_refuses_to_open_with_a_typed_error() {
        // F12: a `voip-control`-only build has no engine, so starting media without an injected
        // backend must be this typed refusal, never a panic or a silent no-op.
        use crate::voip_control::{
            CallDirection, MediaAudioFormat, MediaAudioIo, MediaAudioSpec, MediaOpenContext,
            MediaSessionKey, MediaSessionSpec, VoipMediaBackend,
        };
        let backend = NoMediaBackend;
        let key = MediaSessionKey::builder()
            .call_id("NO-BACKEND".into())
            .generation(1)
            .build();
        let spec = MediaSessionSpec::builder()
            .key(key)
            .direction(CallDirection::Outgoing)
            .self_lid("1:0@lid".into())
            .peer_lid("2:0@lid".into())
            .call_key(vec![0u8; 32])
            .ssrc(1)
            .audio(
                MediaAudioSpec::builder()
                    .format(MediaAudioFormat::MLOW_16KHZ_60MS)
                    .io(MediaAudioIo::Pcm)
                    .build(),
            )
            .relay_token(vec![])
            .auth_token(vec![])
            .relay_ip("127.0.0.1".into())
            .relay_port(3478)
            .integrity_key(vec![])
            .warp_mi_tag_len(4)
            .enable_media(false)
            .enable_video(false)
            .enable_sframe(false)
            .build();
        assert_eq!(
            backend.open(spec, MediaOpenContext::for_test()).await,
            Err(MediaSetupError::NoBackend)
        );
    }

    #[test]
    fn subscribe_delivers_events_published_on_the_same_session() {
        // F7: the resident subscribe is a real stream, not a closed receiver. The drive loop holds
        // the sender from `event_sender`; a consumer holding the receiver sees what it publishes.
        let session = ResidentMediaSession::new();
        let events = session.subscribe();
        assert!(
            session
                .event_sender()
                .try_send(MediaEvent::RelayAllocated)
                .is_ok()
        );
        assert_eq!(events.try_recv(), Ok(MediaEvent::RelayAllocated));
    }

    #[test]
    fn a_video_command_lands_on_the_drive_mailbox() {
        let session = ResidentMediaSession::new();
        let (tx, rx) = video_control_channel();
        session.set_video_sender(tx);
        assert!(session.submit(MediaCommand::RequireVideoKeyframe));
        assert!(matches!(rx.try_recv(), Ok(VideoControl::RequireKeyframe)));
    }

    #[test]
    fn a_rekey_is_one_shot() {
        let session = ResidentMediaSession::new();
        let rx = session
            .take_rekey_receiver()
            .expect("the session owns a rekey receiver");
        assert!(session.submit(MediaCommand::RekeyRecv {
            answering_lid: "2:0@lid".into(),
            audio_codec: None,
        }));
        assert!(rx.try_recv().is_ok());
        // The receiver is one-shot: once the drive loop has taken it, a second answer stays queued
        // for nobody, and a further `take` returns `None`.
        assert!(session.take_rekey_receiver().is_none());
    }

    #[test]
    fn an_epoch_before_media_is_retained_for_attach() {
        let session = ResidentMediaSession::new();
        let update = GroupCallUpdate::builder()
            .call_id("G".to_string())
            .call_creator(wacore_binary::Jid::new("1", wacore_binary::Server::Lid))
            .transaction_id(3)
            .media("audio".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(false)
            .participants(Vec::new())
            .build();
        assert!(session.deliver_group_epoch(
            3,
            crate::voip_control::MediaGroupEpoch::new(vec![9; 32]),
            None
        ));
        assert_eq!(session.pending_group_epoch(), Some(3));
        // Attach replays the retained epoch rather than dropping the key.
        let (tx, rx) = async_channel::bounded(4);
        assert!(session.set_group_sender(tx, Some(4), Some(update), None));
        assert!(matches!(rx.try_recv(), Ok(GroupControl::Transition { .. })));
        assert_eq!(session.pending_group_epoch(), None);
    }

    #[test]
    fn close_releases_every_mailbox_and_ends_the_stream() {
        // Item 3: a closed session must not hold the drive loop's mailboxes, a retained epoch's
        // key bytes, or a parked event stream.
        let session = ResidentMediaSession::new();
        let _video_rx = session.install_video_channel();
        assert!(session.submit(MediaCommand::RequireVideoKeyframe));
        assert!(session.deliver_group_epoch(
            3,
            crate::voip_control::MediaGroupEpoch::new(vec![9; 32]),
            None
        ));
        assert_eq!(session.pending_group_epoch(), Some(3));
        let events = session.subscribe();
        session.close(crate::voip_control::MediaCloseReason::Local);
        assert_eq!(session.pending_group_epoch(), None);
        assert_eq!(session.retained_bytes(), 0);
        assert!(!session.submit(MediaCommand::RequireVideoKeyframe));
        // A late answer fails instead of queueing key material for a dead call.
        assert!(!session.submit(MediaCommand::RekeyRecv {
            answering_lid: "2:0@lid".into(),
            audio_codec: None,
        }));
        // The public stream ends instead of parking a lingering handle.
        assert!(matches!(
            events.try_recv(),
            Err(async_channel::TryRecvError::Closed)
        ));
    }

    #[test]
    fn a_neutral_group_update_maps_to_the_core_group_control() {
        let update = GroupCallUpdate::builder()
            .call_id("G".to_string())
            .call_creator(wacore_binary::Jid::new("1", wacore_binary::Server::Lid))
            .transaction_id(7)
            .media("audio".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(false)
            .participants(Vec::new())
            .build();
        let command = MediaCommand::ApplyGroupUpdate(Box::new(update));
        let control = group_control_of(&command).expect("the command carries a roster");
        assert!(matches!(control, GroupControl::Update(_)));
    }
}

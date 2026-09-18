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
//! mailboxes, the lossless group-control queue with its byte-budgeted coalescing, and the epoch
//! retained before media attaches. The control plane keeps what is its own -- the consumer-facing
//! `CallEvent` queue, the counters cell, the media-task abort handle, and the teardown hook.

use std::mem::size_of;
use std::sync::{Arc, Mutex};

use crate::types::group_call::GroupCallUpdate;
use crate::voip_control::control::{DEFAULT_CALL_EVENT_QUEUE_CAPACITY, GroupControlQueue};
use crate::voip_control::control::{
    GroupControl, GroupRawEpoch, PeerAnswer, VideoControl, VideoControlSender,
};
use crate::voip_control::media_stats::{CallMediaStats, MediaStatsCell};
use crate::voip_control::{
    CallDirection, MediaAudioCodec, MediaCommand, MediaEvent, MediaKeyframeUrgency,
    MediaSessionKey, MediaSessionSpec, MediaSetupError, MediaStats, VoipMediaBackend,
    VoipMediaSession,
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

fn codec_to_core(codec: MediaAudioCodec) -> MediaAudioCodec {
    codec
}

/// The neutral codec for an engine one.
pub(crate) fn codec_to_neutral(codec: MediaAudioCodec) -> MediaAudioCodec {
    codec
}

fn urgency_to_core(urgency: MediaKeyframeUrgency) -> MediaKeyframeUrgency {
    urgency
}

/// The neutral counters for an engine snapshot.
///
/// `CallMediaStats` is the seam's [`MediaStats`] under its historical engine name, so this is the
/// identity and exists only so the resident adapter's call sites read the same as they did when the
/// two were distinct structs.
#[must_use]
pub fn stats_to_neutral(stats: CallMediaStats) -> MediaStats {
    stats
}

/// Mailboxes into one running drive task, attached incrementally as a call is set up.
#[derive(Default)]
struct Mailboxes {
    video: Option<VideoControlSender>,
    group: Option<GroupControlQueue>,
    rekey: Option<async_channel::Sender<PeerAnswer>>,
    /// A decrypted epoch that arrived before relay media attached. Replacing or dropping this
    /// erases its key bytes through [`GroupRawEpoch`]'s `Drop`.
    pending_group_epoch: Option<GroupRawEpoch>,
}

/// One call's media session, resident in the same process as the drive task.
pub struct ResidentMediaSession {
    mailboxes: Mutex<Mailboxes>,
    stats: Mutex<Arc<MediaStatsCell>>,
}

impl ResidentMediaSession {
    /// A session that publishes through a fresh counter cell.
    #[must_use]
    pub fn new() -> Arc<Self> {
        Self::with_stats(Arc::new(MediaStatsCell::default()))
    }

    /// A session that publishes through `stats`, shared with the `CallHandle`.
    #[must_use]
    pub fn with_stats(stats: Arc<MediaStatsCell>) -> Arc<Self> {
        Arc::new(Self {
            mailboxes: Mutex::new(Mailboxes::default()),
            stats: Mutex::new(stats),
        })
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

    /// Attach the caller-only recv-rekey mailbox. Caller side only.
    pub(crate) fn set_rekey_sender(&self, tx: async_channel::Sender<PeerAnswer>) {
        self.mailboxes().rekey = Some(tx);
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

    /// A snapshot in the engine's own counter type.
    #[must_use]
    pub fn core_stats(&self) -> CallMediaStats {
        self.stats_cell().snapshot()
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

    /// The counter cell, for the control plane's `CallHandle` path.
    #[must_use]
    pub fn stats_cell_arc(&self) -> Arc<MediaStatsCell> {
        self.stats_cell()
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
            // The seam does not own mute yet: the live client applies it through `MuteFeed`, which
            // gates the PCM the drive loop reads, and that path is unchanged. Reporting `true` here
            // would tell a direct seam caller its mute landed when nothing moved, so the honest
            // answer is `false` until this session owns the mute state.
            MediaCommand::SetMuted(_) => false,
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
                self.send_video(VideoControl::RequestPeerKeyframe(urgency_to_core(urgency)))
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
                // Take the sender: a duplicate or late `<accept>` from another device is a no-op,
                // because the first answerer wins. That is the old registry `send_rekey` contract.
                let mut mailboxes = self.mailboxes();
                let Some(tx) = mailboxes.rekey.take() else {
                    return false;
                };
                tx.try_send(PeerAnswer {
                    answering_lid,
                    audio_codec: audio_codec.map(codec_to_core),
                })
                .is_ok()
            }
            MediaCommand::SwitchAudioCodec { .. } => {
                // Codec selection is applied at engine construction or through the rekey, which
                // carries the peer's capability. A live swap without one is not wired through the
                // seam, and answering `false` is how the caller learns that.
                false
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
        mailboxes
            .rekey
            .as_ref()
            .map_or(0, |tx| tx.len().saturating_mul(size_of::<PeerAnswer>()))
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

    fn as_any(&self) -> Option<&dyn core::any::Any> {
        Some(self)
    }

    fn stats(&self) -> MediaStats {
        stats_to_neutral(self.stats_cell().snapshot())
    }

    fn subscribe(&self) -> async_channel::Receiver<MediaEvent> {
        // The resident session raises no events of its own: the drive loop publishes straight into
        // the control plane's `CallEvent` queue, which is the public handle stream. A foreign
        // backend that owns its engine raises `MediaEvent`s here.
        let (_tx, rx) = async_channel::unbounded();
        rx
    }

    fn close(&self, _reason: crate::voip_control::MediaCloseReason) {
        // The drive task owns teardown; dropping the session drops the mailboxes. The registry
        // entry's media-task abort ends the call.
    }
}

/// The in-process resident backend: reserves a [`ResidentMediaSession`] per call.
///
/// This is the default a `CallRegistry` carries when no backend is injected, so registry-only
/// builds and unit tests keep the exact pre-injection behavior. `whatsapp-rust` injects its own
/// [`WacoreVoipMediaBackend`](crate::voip_control) instead, which owns the engine and the drive
/// task; this one exists so the registry never has to name `ResidentMediaSession` itself.
#[derive(Default)]
pub struct ResidentMediaBackend;

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl VoipMediaBackend for ResidentMediaBackend {
    fn reserve(
        &self,
        _key: &MediaSessionKey,
        _direction: CallDirection,
    ) -> Arc<dyn VoipMediaSession> {
        ResidentMediaSession::new()
    }

    async fn open(
        &self,
        _session: &Arc<dyn VoipMediaSession>,
        spec: MediaSessionSpec,
    ) -> Result<(), MediaSetupError> {
        // The registry's own fallback validates the projection and builds no task: the live call
        // path drives the engine through `whatsapp-rust`'s backend, which owns the runtime and
        // transport. A spec the engine refuses is refused here too.
        let _ = crate::voip_control::engine_bridge::into_engine_parts(spec)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voip_control::control::video_control_channel;

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
        let (tx, rx) = async_channel::bounded(1);
        session.set_rekey_sender(tx);
        assert!(session.submit(MediaCommand::RekeyRecv {
            answering_lid: "2:0@lid".into(),
            audio_codec: None,
        }));
        assert!(rx.try_recv().is_ok());
        // The first answerer wins; a duplicate finds no sender.
        assert!(!session.submit(MediaCommand::RekeyRecv {
            answering_lid: "3:0@lid".into(),
            audio_codec: None,
        }));
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

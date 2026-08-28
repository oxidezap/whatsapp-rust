//! Participant-indexed group media receive state.
//!
//! Packets are routed by deterministic SSRC before authentication. A shared
//! keygen-v2 epoch is then derived with the authenticated sender's device id;
//! keys are never tried across participants.

use std::collections::{BTreeMap, HashMap, HashSet};

use subtle::ConstantTimeEq;
use wacore_binary::{Jid, JidExt};
use zeroize::Zeroize;

use crate::types::group_call::{
    GROUP_CALL_MAX_PARTICIPANTS, GroupCallDevice, GroupCallParticipant, GroupCallUpdate,
};
use crate::voip::app_data::APP_DATA_RTP_TIMESTAMP_STRIDE;
use crate::voip::e2e_srtp::{derive_e2e_keys_from_raw, derive_srtcp_keys_from_raw};
use crate::voip::rtp::RTP_PAYLOAD_TYPE_APP_DATA;
use crate::voip::rtp::{RtpHeader, parse_rtp_header};
use crate::voip::session::{
    MediaPipeline, MediaPipelineParams, VideoPipeline, VideoPipelineParams,
};
use crate::voip::ssrc::{
    APP_DATA_SSRC_SLOT_WORD, VIDEO_SSRC_SLOT_WORD, derive_video_participant_ssrc,
    derive_wasm_participant_ssrc, format_e2e_srtp_participant_id,
};

const MAX_BUFFERED_EPOCHS: usize = 8;
const RELAY_STREAM_SLOT_COUNT: u32 = 9;

struct EpochKey(Vec<u8>);

impl EpochKey {
    fn new(value: &[u8]) -> Option<Self> {
        (value.len() == 32).then(|| Self(value.to_vec()))
    }

    fn as_slice(&self) -> &[u8] {
        &self.0
    }

    fn equals(&self, other: &[u8]) -> bool {
        other.len() == 32 && bool::from(self.0.ct_eq(other))
    }
}

impl Drop for EpochKey {
    fn drop(&mut self) {
        self.0.zeroize();
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum GroupRosterApply {
    Applied,
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum GroupEpochApply {
    Installed,
    Buffered,
    Stale,
    Duplicate,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum GroupMediaError {
    #[error("group media call identity mismatch")]
    IdentityMismatch,
    #[error("group media snapshot is invalid")]
    InvalidSnapshot,
    #[error("group media epoch must contain exactly 32 bytes")]
    InvalidEpoch,
    #[error("group media epoch conflicts with an existing transaction")]
    ConflictingEpoch,
    #[error("the local participant is no longer connected to the group call")]
    LocalParticipantRemoved,
    #[error("group media pipeline could not be constructed")]
    Pipeline,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ParticipantMedia {
    pub participant_id: String,
    pub user_jid: Jid,
    pub device_jid: Jid,
    pub pid: Option<u32>,
    pub header: RtpHeader,
    pub payload: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ParticipantVideo {
    pub participant_id: String,
    pub user_jid: Jid,
    pub device_jid: Jid,
    pub pid: Option<u32>,
    pub header: RtpHeader,
    pub access_units: Vec<Vec<u8>>,
}

struct ParticipantReceiver {
    participant_id: String,
    user_jid: Jid,
    device_jid: Jid,
    pid: Option<u32>,
    audio_ssrc: u32,
    video_ssrc: u32,
    app_data_ssrc: u32,
    video_enabled: bool,
    audio: Option<MediaPipeline>,
    app_data: Option<MediaPipeline>,
    video: Option<VideoPipeline>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GroupMediaStream {
    Audio,
    Video,
}

/// Why an inbound group audio packet produced nothing.
///
/// Two different things, and only one of them is a fault: a packet whose SSRC is not in the roster
/// never reaches SRTP at all, so calling it an authentication failure reports a key problem for a
/// packet no key was tried on. A straggler from a participant an authoritative update just removed
/// is the ordinary way that happens.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GroupAudioReject {
    /// No route for this SSRC: not addressed to anyone this call still knows.
    Unroutable,
    /// Routed, and the tag did not authenticate.
    Unprotect,
}

/// Per-call participant receiver registry with transaction-ordered shared epochs.
pub struct GroupMediaRegistry {
    call_id: String,
    call_creator: Jid,
    self_lid: String,
    self_device: Jid,
    samples_per_packet: u32,
    warp_mi_tag_len: usize,
    video_ts_stride: u32,
    roster_transaction: Option<u32>,
    receivers: HashMap<String, ParticipantReceiver>,
    audio_routes: HashMap<u32, String>,
    video_routes: HashMap<u32, String>,
    app_data_routes: HashMap<u32, String>,
    rtcp_routes: HashMap<u32, String>,
    installed_epoch: Option<(u32, EpochKey)>,
    pending_epochs: BTreeMap<u32, EpochKey>,
}

impl GroupMediaRegistry {
    pub fn new(
        call_id: impl Into<String>,
        call_creator: Jid,
        self_lid: &Jid,
        samples_per_packet: u32,
        warp_mi_tag_len: usize,
        video_ts_stride: u32,
    ) -> Result<Self, GroupMediaError> {
        if samples_per_packet == 0 || video_ts_stride == 0 || !(1..=20).contains(&warp_mi_tag_len) {
            return Err(GroupMediaError::Pipeline);
        }
        Ok(Self {
            call_id: call_id.into(),
            call_creator,
            self_lid: format_e2e_srtp_participant_id(&self_lid.to_string()),
            self_device: self_lid.clone(),
            samples_per_packet,
            warp_mi_tag_len,
            video_ts_stride,
            roster_transaction: None,
            receivers: HashMap::new(),
            audio_routes: HashMap::new(),
            video_routes: HashMap::new(),
            app_data_routes: HashMap::new(),
            rtcp_routes: HashMap::new(),
            installed_epoch: None,
            pending_epochs: BTreeMap::new(),
        })
    }

    /// Seed the original direct peer so an in-place add-person transition keeps
    /// its authenticated receiver until a PID-bearing group roster is ready.
    pub fn seed_direct_peer(
        &mut self,
        call_key: &[u8],
        user_jid: &Jid,
        device_jid: &Jid,
        video: bool,
    ) -> Result<(), GroupMediaError> {
        let receiver = self.build_receiver(call_key, user_jid, device_jid, None, video)?;
        self.receivers
            .insert(receiver.participant_id.clone(), receiver);
        self.rebuild_routes()?;
        Ok(())
    }

    pub fn roster_transaction(&self) -> Option<u32> {
        self.roster_transaction
    }

    pub fn installed_epoch_transaction(&self) -> Option<u32> {
        self.installed_epoch
            .as_ref()
            .map(|(transaction, _)| *transaction)
    }

    pub(crate) fn installed_epoch(&self) -> Option<(u32, &[u8])> {
        self.installed_epoch
            .as_ref()
            .map(|(transaction, epoch)| (*transaction, epoch.as_slice()))
    }

    pub fn active_participant_ids(&self) -> Vec<String> {
        let mut ids = self
            .receivers
            .values()
            .filter(|receiver| receiver.audio.is_some())
            .map(|receiver| receiver.participant_id.clone())
            .collect::<Vec<_>>();
        ids.sort();
        ids
    }

    pub(crate) fn active_pids(&self) -> Vec<u32> {
        let mut pids = self
            .receivers
            .values()
            .filter_map(|receiver| receiver.pid)
            .collect::<Vec<_>>();
        pids.sort_unstable();
        pids.dedup();
        pids
    }

    pub(crate) fn active_device_ids(&self) -> Vec<String> {
        let mut ids = self.receivers.keys().cloned().collect::<Vec<_>>();
        ids.sort();
        ids
    }

    pub(crate) fn participants_with_pid_changes(&self, update: &GroupCallUpdate) -> Vec<String> {
        active_devices(update, &self.self_device)
            .into_iter()
            .filter_map(|(_, device)| {
                let participant_id = format_e2e_srtp_participant_id(&device.jid.to_string());
                self.receivers
                    .get(&participant_id)
                    .filter(|receiver| pid_migrated(receiver.pid, device.pid))
                    .map(|_| participant_id)
            })
            .collect()
    }

    pub(crate) fn sender_report_stream(
        &self,
        participant_id: &str,
        sender_ssrc: u32,
    ) -> Option<GroupMediaStream> {
        let receiver = self.receivers.get(participant_id)?;
        if receiver.audio.is_some() && receiver.audio_ssrc == sender_ssrc {
            Some(GroupMediaStream::Audio)
        } else if receiver.video_enabled
            && receiver.video.is_some()
            && receiver.video_ssrc == sender_ssrc
        {
            Some(GroupMediaStream::Video)
        } else {
            None
        }
    }

    /// Replace the connected PID-bearing roster. Existing device receivers are
    /// reused so ROC and depacketization state survive a participant update.
    pub fn apply_group_update(
        &mut self,
        update: &GroupCallUpdate,
    ) -> Result<GroupRosterApply, GroupMediaError> {
        self.validate_update(update)?;
        if self
            .roster_transaction
            .is_some_and(|current| update.transaction_id <= current)
        {
            return Ok(GroupRosterApply::Stale);
        }

        let active = active_devices(update, &self.self_device);
        if active.is_empty() {
            // Even the first authoritative roster supersedes the direct-call fallback. Retaining
            // its seeded receiver when every remote device has already left would keep accepting
            // media under a participant identity the group roster no longer authorizes.
            self.receivers.clear();
            self.rebuild_routes()?;
            self.roster_transaction = Some(update.transaction_id);
            self.install_best_pending_epoch()?;
            return Ok(GroupRosterApply::Applied);
        }

        // Route derivation is fallible protocol validation. Complete it before moving any existing
        // receiver or advancing the roster transaction so a corrected resend remains admissible.
        let mut previous = std::mem::take(&mut self.receivers);
        let mut next = HashMap::with_capacity(active.len());
        for (participant, device) in active {
            let participant_id = format_e2e_srtp_participant_id(&device.jid.to_string());
            let mut receiver = if let Some(mut existing) = previous.remove(&participant_id) {
                if pid_migrated(existing.pid, device.pid) {
                    // A PID identifies one relay media session. Reusing its authenticated ROC,
                    // replay, and depacketizer state across a PID migration can reject the new
                    // stream's first packets or combine fragments from two different sessions.
                    if let Some((_, epoch)) = self.installed_epoch.as_ref() {
                        self.build_receiver(
                            epoch.as_slice(),
                            &participant.jid,
                            &device.jid,
                            device.pid,
                            update.media == "video",
                        )?
                    } else {
                        self.receiver_without_keys(&participant.jid, &device.jid, device.pid)
                    }
                } else if update.media == "video"
                    && existing.video.is_none()
                    && let Some((_, epoch)) = self.installed_epoch.as_ref()
                {
                    let mut rebuilt = self.build_receiver(
                        epoch.as_slice(),
                        &participant.jid,
                        &device.jid,
                        device.pid,
                        true,
                    )?;
                    rebuilt.audio = existing.audio.take();
                    rebuilt.app_data = existing.app_data.take();
                    rebuilt
                } else {
                    existing
                }
            } else if let Some((_, epoch)) = self.installed_epoch.as_ref() {
                self.build_receiver(
                    epoch.as_slice(),
                    &participant.jid,
                    &device.jid,
                    device.pid,
                    update.media == "video",
                )?
            } else {
                self.receiver_without_keys(&participant.jid, &device.jid, device.pid)
            };
            receiver.user_jid = participant.jid.clone();
            receiver.device_jid = device.jid.clone();
            receiver.pid = device.pid;
            if update.media == "audio"
                && receiver.video_enabled
                && let Some(video) = receiver.video.as_mut()
            {
                // Preserve SRTP replay state for a later reactivation, but discard incomplete
                // pre-downgrade access units so they cannot be emitted under the new roster.
                video.reset_depacketizer();
            }
            receiver.video_enabled = update.media == "video";
            next.insert(participant_id, receiver);
        }
        self.receivers = next;
        self.roster_transaction = Some(update.transaction_id);
        self.install_best_pending_epoch()?;
        self.rebuild_routes()?;
        Ok(GroupRosterApply::Applied)
    }

    /// Install immediately when the corresponding roster is known, otherwise
    /// buffer a bounded future epoch.
    pub fn apply_raw_epoch(
        &mut self,
        transaction_id: u32,
        raw_epoch: &[u8],
    ) -> Result<GroupEpochApply, GroupMediaError> {
        if transaction_id == 0 || raw_epoch.len() != 32 {
            return Err(GroupMediaError::InvalidEpoch);
        }
        if let Some((installed_transaction, installed)) = self.installed_epoch.as_ref() {
            if transaction_id < *installed_transaction {
                return Ok(GroupEpochApply::Stale);
            }
            if transaction_id == *installed_transaction {
                return if installed.equals(raw_epoch) {
                    Ok(GroupEpochApply::Duplicate)
                } else {
                    Err(GroupMediaError::ConflictingEpoch)
                };
            }
        }
        if let Some(pending) = self.pending_epochs.get(&transaction_id) {
            return if pending.equals(raw_epoch) {
                Ok(GroupEpochApply::Duplicate)
            } else {
                Err(GroupMediaError::ConflictingEpoch)
            };
        }

        if self
            .roster_transaction
            .is_none_or(|roster| transaction_id > roster)
        {
            self.pending_epochs.insert(
                transaction_id,
                EpochKey::new(raw_epoch).ok_or(GroupMediaError::InvalidEpoch)?,
            );
            if self.pending_epochs.len() > MAX_BUFFERED_EPOCHS {
                let Some(farthest) = self.pending_epochs.keys().next_back().copied() else {
                    return Err(GroupMediaError::InvalidEpoch);
                };
                self.pending_epochs.remove(&farthest);
            }
            return Ok(GroupEpochApply::Buffered);
        }

        self.install_epoch(transaction_id, raw_epoch)?;
        Ok(GroupEpochApply::Installed)
    }

    pub fn unprotect_audio(&mut self, packet: &[u8]) -> Result<ParticipantMedia, GroupAudioReject> {
        let ssrc = parse_rtp_header(packet)
            .ok_or(GroupAudioReject::Unroutable)?
            .ssrc;
        let participant_id = self
            .audio_routes
            .get(&ssrc)
            .ok_or(GroupAudioReject::Unroutable)?
            .clone();
        let receiver = self
            .receivers
            .get_mut(&participant_id)
            .ok_or(GroupAudioReject::Unroutable)?;
        let (header, payload) = receiver
            .audio
            .as_mut()
            .ok_or(GroupAudioReject::Unroutable)?
            .unprotect_audio(packet)
            .ok_or(GroupAudioReject::Unprotect)?;
        Ok(ParticipantMedia {
            participant_id,
            user_jid: receiver.user_jid.clone(),
            device_jid: receiver.device_jid.clone(),
            pid: receiver.pid,
            header,
            payload,
        })
    }

    pub fn unprotect_video(&mut self, packet: &[u8]) -> Option<ParticipantVideo> {
        let ssrc = parse_rtp_header(packet)?.ssrc;
        let participant_id = self.video_routes.get(&ssrc)?.clone();
        let receiver = self.receivers.get_mut(&participant_id)?;
        if !receiver.video_enabled {
            return None;
        }
        let (header, access_units) = receiver.video.as_mut()?.unprotect_video_packet(packet)?;
        Some(ParticipantVideo {
            participant_id,
            user_jid: receiver.user_jid.clone(),
            device_jid: receiver.device_jid.clone(),
            pid: receiver.pid,
            header,
            access_units,
        })
    }

    pub fn unprotect_app_data(&mut self, packet: &[u8]) -> Option<ParticipantMedia> {
        let ssrc = parse_rtp_header(packet)?.ssrc;
        let participant_id = self.app_data_routes.get(&ssrc)?.clone();
        let receiver = self.receivers.get_mut(&participant_id)?;
        let (header, payload) = receiver.app_data.as_mut()?.unprotect_audio(packet)?;
        (header.payload_type == RTP_PAYLOAD_TYPE_APP_DATA).then(|| ParticipantMedia {
            participant_id,
            user_jid: receiver.user_jid.clone(),
            device_jid: receiver.device_jid.clone(),
            pid: receiver.pid,
            header,
            payload,
        })
    }

    pub fn unprotect_rtcp(&mut self, packet: &[u8]) -> Option<ParticipantMedia> {
        let ssrc = crate::voip::rtcp::parse_rtcp_sender_ssrc(packet)?;
        let participant_id = self.rtcp_routes.get(&ssrc)?.clone();
        let receiver = self.receivers.get_mut(&participant_id)?;
        let payload = receiver.audio.as_mut()?.unprotect_rtcp(packet)?;
        Some(ParticipantMedia {
            participant_id,
            user_jid: receiver.user_jid.clone(),
            device_jid: receiver.device_jid.clone(),
            pid: receiver.pid,
            header: RtpHeader {
                marker: false,
                payload_type: 0,
                sequence_number: 0,
                timestamp: 0,
                ssrc,
                extension_word: None,
                video_extension: None,
            },
            payload,
        })
    }

    fn install_best_pending_epoch(&mut self) -> Result<(), GroupMediaError> {
        let Some(roster) = self.roster_transaction else {
            return Ok(());
        };
        let candidate = self
            .pending_epochs
            .range(..=roster)
            .next_back()
            .map(|(transaction, _)| *transaction);
        let Some(transaction) = candidate else {
            return Ok(());
        };
        let epoch = self
            .pending_epochs
            .remove(&transaction)
            .ok_or(GroupMediaError::InvalidEpoch)?;
        self.install_epoch(transaction, epoch.as_slice())?;
        self.pending_epochs
            .retain(|pending_transaction, _| *pending_transaction > transaction);
        Ok(())
    }

    fn install_epoch(
        &mut self,
        transaction_id: u32,
        raw_epoch: &[u8],
    ) -> Result<(), GroupMediaError> {
        let epoch = EpochKey::new(raw_epoch).ok_or(GroupMediaError::InvalidEpoch)?;

        // Complete every fallible derivation before mutating a working receiver.
        for receiver in self.receivers.values() {
            derive_e2e_keys_from_raw(epoch.as_slice(), &receiver.participant_id)
                .ok_or(GroupMediaError::InvalidEpoch)?;
            derive_srtcp_keys_from_raw(epoch.as_slice(), &receiver.participant_id)
                .ok_or(GroupMediaError::InvalidEpoch)?;
        }

        let receiver_ids = self.receivers.keys().cloned().collect::<Vec<_>>();
        for participant_id in receiver_ids {
            let needs_audio = self
                .receivers
                .get(&participant_id)
                .is_some_and(|receiver| receiver.audio.is_none());
            if needs_audio {
                let receiver = self
                    .receivers
                    .get(&participant_id)
                    .ok_or(GroupMediaError::Pipeline)?;
                let rebuilt = self.build_receiver(
                    epoch.as_slice(),
                    &receiver.user_jid,
                    &receiver.device_jid,
                    receiver.pid,
                    receiver.video_enabled,
                )?;
                self.receivers.insert(participant_id, rebuilt);
                continue;
            }
            let receiver = self
                .receivers
                .get_mut(&participant_id)
                .ok_or(GroupMediaError::Pipeline)?;
            if !receiver.audio.as_mut().is_some_and(|audio| {
                audio.rekey_recv_from_raw_preserving_roc(epoch.as_slice(), &receiver.participant_id)
            }) {
                return Err(GroupMediaError::Pipeline);
            }
            if !receiver.app_data.as_mut().is_some_and(|app_data| {
                app_data
                    .rekey_recv_from_raw_preserving_roc(epoch.as_slice(), &receiver.participant_id)
            }) {
                return Err(GroupMediaError::Pipeline);
            }
            if let Some(video) = receiver.video.as_mut()
                && !video
                    .rekey_recv_from_raw_preserving_roc(epoch.as_slice(), &receiver.participant_id)
            {
                return Err(GroupMediaError::Pipeline);
            }
        }
        self.installed_epoch = Some((transaction_id, epoch));
        self.rebuild_routes()?;
        Ok(())
    }

    fn build_receiver(
        &self,
        key: &[u8],
        user_jid: &Jid,
        device_jid: &Jid,
        pid: Option<u32>,
        video: bool,
    ) -> Result<ParticipantReceiver, GroupMediaError> {
        let participant_id = format_e2e_srtp_participant_id(&device_jid.to_string());
        let audio_ssrc = derive_wasm_participant_ssrc(&self.call_id, &participant_id, 0);
        let video_ssrc = derive_video_participant_ssrc(&self.call_id, &participant_id);
        let app_data_ssrc =
            derive_wasm_participant_ssrc(&self.call_id, &participant_id, APP_DATA_SSRC_SLOT_WORD);
        let audio = MediaPipeline::new(&MediaPipelineParams {
            call_key: key,
            self_lid: &self.self_lid,
            peer_lid: &participant_id,
            ssrc: audio_ssrc,
            samples_per_packet: self.samples_per_packet,
            warp_mi_tag_len: self.warp_mi_tag_len,
        })
        .ok_or(GroupMediaError::Pipeline)?;
        let mut app_data = MediaPipeline::new(&MediaPipelineParams {
            call_key: key,
            self_lid: &self.self_lid,
            peer_lid: &participant_id,
            ssrc: app_data_ssrc,
            samples_per_packet: APP_DATA_RTP_TIMESTAMP_STRIDE,
            warp_mi_tag_len: self.warp_mi_tag_len,
        })
        .ok_or(GroupMediaError::Pipeline)?;
        if !app_data.set_audio_payload_type(RTP_PAYLOAD_TYPE_APP_DATA) {
            return Err(GroupMediaError::Pipeline);
        }
        app_data.set_audio_mlow_profile(false);
        let video = if video {
            Some(
                VideoPipeline::new(&VideoPipelineParams {
                    call_key: key,
                    self_lid: &self.self_lid,
                    peer_lid: &participant_id,
                    ssrc: video_ssrc,
                    ts_stride: self.video_ts_stride,
                    warp_mi_tag_len: self.warp_mi_tag_len,
                })
                .ok_or(GroupMediaError::Pipeline)?,
            )
        } else {
            None
        };
        Ok(ParticipantReceiver {
            participant_id,
            user_jid: user_jid.clone(),
            device_jid: device_jid.clone(),
            pid,
            audio_ssrc,
            video_ssrc,
            app_data_ssrc,
            video_enabled: video.is_some(),
            audio: Some(audio),
            app_data: Some(app_data),
            video,
        })
    }

    fn receiver_without_keys(
        &self,
        user_jid: &Jid,
        device_jid: &Jid,
        pid: Option<u32>,
    ) -> ParticipantReceiver {
        let participant_id = format_e2e_srtp_participant_id(&device_jid.to_string());
        ParticipantReceiver {
            audio_ssrc: derive_wasm_participant_ssrc(&self.call_id, &participant_id, 0),
            video_ssrc: derive_video_participant_ssrc(&self.call_id, &participant_id),
            app_data_ssrc: derive_wasm_participant_ssrc(
                &self.call_id,
                &participant_id,
                APP_DATA_SSRC_SLOT_WORD,
            ),
            participant_id,
            user_jid: user_jid.clone(),
            device_jid: device_jid.clone(),
            pid,
            video_enabled: false,
            audio: None,
            app_data: None,
            video: None,
        }
    }

    fn rebuild_routes(&mut self) -> Result<(), GroupMediaError> {
        let mut audio_routes = HashMap::new();
        let mut video_routes = HashMap::new();
        let mut app_data_routes = HashMap::new();
        let mut rtcp_routes = HashMap::new();
        for receiver in self.receivers.values() {
            if receiver.audio.is_some()
                && audio_routes
                    .insert(receiver.audio_ssrc, receiver.participant_id.clone())
                    .is_some()
            {
                return Err(GroupMediaError::InvalidSnapshot);
            }
            if receiver.video_enabled
                && receiver.video.is_some()
                && video_routes
                    .insert(receiver.video_ssrc, receiver.participant_id.clone())
                    .is_some()
            {
                return Err(GroupMediaError::InvalidSnapshot);
            }
            if receiver.app_data.is_some()
                && app_data_routes
                    .insert(receiver.app_data_ssrc, receiver.participant_id.clone())
                    .is_some()
            {
                return Err(GroupMediaError::InvalidSnapshot);
            }
            if receiver.audio.is_some() {
                for slot in 0..RELAY_STREAM_SLOT_COUNT {
                    let ssrc =
                        derive_wasm_participant_ssrc(&self.call_id, &receiver.participant_id, slot);
                    if rtcp_routes
                        .insert(ssrc, receiver.participant_id.clone())
                        .is_some()
                    {
                        return Err(GroupMediaError::InvalidSnapshot);
                    }
                }
            }
        }
        self.audio_routes = audio_routes;
        self.video_routes = video_routes;
        self.app_data_routes = app_data_routes;
        self.rtcp_routes = rtcp_routes;
        Ok(())
    }

    fn validate_update(&self, update: &GroupCallUpdate) -> Result<(), GroupMediaError> {
        if update.call_id != self.call_id || update.call_creator != self.call_creator {
            return Err(GroupMediaError::IdentityMismatch);
        }
        if update.transaction_id == 0
            || update.participants.len() > GROUP_CALL_MAX_PARTICIPANTS
            || !matches!(update.media.as_str(), "audio" | "video")
        {
            return Err(GroupMediaError::InvalidSnapshot);
        }
        validate_group_media_snapshot(update)
    }
}

fn pid_migrated(existing: Option<u32>, incoming: Option<u32>) -> bool {
    existing.is_some() && existing != incoming
}

pub(crate) fn validate_group_media_snapshot(
    update: &GroupCallUpdate,
) -> Result<(), GroupMediaError> {
    let mut pids = HashSet::new();
    let mut devices = HashSet::new();
    let mut audio = HashSet::new();
    let mut video = HashSet::new();
    let mut app_data = HashSet::new();
    let mut rtcp = HashSet::new();
    for device in update
        .participants
        .iter()
        .filter(|participant| participant.state.as_deref() == Some("connected"))
        .flat_map(|participant| &participant.devices)
    {
        let Some(pid) = device.pid else {
            continue;
        };
        let participant_id = format_e2e_srtp_participant_id(&device.jid.to_string());
        if pid == 0 || !pids.insert(pid) || !devices.insert(participant_id.clone()) {
            return Err(GroupMediaError::InvalidSnapshot);
        }
        let audio_ssrc = derive_wasm_participant_ssrc(&update.call_id, &participant_id, 0);
        let video_ssrc =
            derive_wasm_participant_ssrc(&update.call_id, &participant_id, VIDEO_SSRC_SLOT_WORD);
        let app_data_ssrc =
            derive_wasm_participant_ssrc(&update.call_id, &participant_id, APP_DATA_SSRC_SLOT_WORD);
        if !audio.insert(audio_ssrc) || !video.insert(video_ssrc) || !app_data.insert(app_data_ssrc)
        {
            return Err(GroupMediaError::InvalidSnapshot);
        }
        for slot_word in 0..RELAY_STREAM_SLOT_COUNT {
            let rtcp_ssrc =
                derive_wasm_participant_ssrc(&update.call_id, &participant_id, slot_word);
            if !rtcp.insert(rtcp_ssrc) {
                return Err(GroupMediaError::InvalidSnapshot);
            }
        }
    }
    Ok(())
}

pub(crate) fn group_device_is_local(
    participant: &GroupCallParticipant,
    device: &GroupCallDevice,
    local_device: &Jid,
) -> bool {
    let owns_local_user = participant.jid.is_same_user_as(local_device)
        || participant
            .pn
            .as_ref()
            .is_some_and(|pn| pn.is_same_user_as(local_device));
    owns_local_user
        && device.jid.device == local_device.device
        && (device.jid.is_same_user_as(&participant.jid)
            || participant
                .pn
                .as_ref()
                .is_some_and(|pn| device.jid.is_same_user_as(pn)))
}

fn active_devices<'a>(
    update: &'a GroupCallUpdate,
    local_device: &Jid,
) -> Vec<(&'a GroupCallParticipant, &'a GroupCallDevice)> {
    update
        .participants
        .iter()
        .filter(|participant| participant.is_connected())
        .flat_map(|participant| {
            participant
                .devices
                .iter()
                .filter(|device| device.pid.is_some())
                .map(move |device| (participant, device))
        })
        .filter(|(participant, device)| !group_device_is_local(participant, device, local_device))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voip::rtp::VIDEO_TS_STRIDE_15FPS;
    use crate::voip::warp::WARP_MI_TAG_LEN;
    use wacore_binary::Server;

    fn device(user: &str, device: u16, pid: u32) -> GroupCallParticipant {
        GroupCallParticipant {
            jid: Jid::new(user, Server::Lid),
            pn: None,
            state: Some("connected".to_string()),
            participant_type: None,
            devices: vec![GroupCallDevice {
                jid: Jid::new(user, Server::Lid).with_device(device),
                platform: None,
                pid: Some(pid),
                capability_version: None,
                capability: Vec::new(),
            }],
        }
    }

    fn update(transaction: u32, participants: Vec<GroupCallParticipant>) -> GroupCallUpdate {
        GroupCallUpdate {
            call_id: "CALL".to_string(),
            call_creator: Jid::new("100001", Server::Lid).with_device(1),
            group_jid: None,
            transaction_id: transaction,
            media: "video".to_string(),
            connected_limit: 32,
            joinable: true,
            av_upgradable: true,
            rekey_requested: false,
            participants,
            relay: None,
        }
    }

    fn registry() -> GroupMediaRegistry {
        GroupMediaRegistry::new(
            "CALL",
            Jid::new("100001", Server::Lid).with_device(1),
            &Jid::new("100001", Server::Lid).with_device(1),
            960,
            WARP_MI_TAG_LEN,
            VIDEO_TS_STRIDE_15FPS,
        )
        .unwrap()
    }

    fn peer_sender(key: &[u8], peer: &Jid) -> MediaPipeline {
        let participant = format_e2e_srtp_participant_id(&peer.to_string());
        MediaPipeline::new(&MediaPipelineParams {
            call_key: key,
            self_lid: &participant,
            peer_lid: "100001:1@lid",
            ssrc: derive_wasm_participant_ssrc("CALL", &participant, 0),
            samples_per_packet: 960,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap()
    }

    fn peer_video_sender(key: &[u8], peer: &Jid) -> VideoPipeline {
        let participant = format_e2e_srtp_participant_id(&peer.to_string());
        VideoPipeline::new(&VideoPipelineParams {
            call_key: key,
            self_lid: &peer.to_string(),
            peer_lid: "100001:1@lid",
            ssrc: derive_video_participant_ssrc("CALL", &participant),
            ts_stride: VIDEO_TS_STRIDE_15FPS,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap()
    }

    #[test]
    fn future_epoch_activates_when_roster_arrives_and_routes_by_ssrc() {
        let epoch = [0x42; 32];
        let alice = Jid::new("200002", Server::Lid).with_device(2);
        let bob = Jid::new("300003", Server::Lid).with_device(3);
        let mut registry = registry();
        assert_eq!(
            registry.apply_raw_epoch(7, &epoch).unwrap(),
            GroupEpochApply::Buffered
        );
        assert_eq!(
            registry
                .apply_group_update(&update(
                    7,
                    vec![
                        device("100001", 1, 1),
                        device("200002", 2, 2),
                        device("300003", 3, 3),
                    ],
                ))
                .unwrap(),
            GroupRosterApply::Applied
        );
        assert_eq!(registry.installed_epoch_transaction(), Some(7));

        let payload = [0x50, 1, 2, 3, 4, 5, 6, 7];
        for peer in [&alice, &bob] {
            let packet = peer_sender(&epoch, peer).protect_audio(&payload);
            let decoded = registry.unprotect_audio(&packet).unwrap();
            assert_eq!(decoded.device_jid, *peer);
            assert_eq!(decoded.payload, payload);
        }
        let mut unknown = peer_sender(&epoch, &Jid::new("400004", Server::Lid).with_device(4));
        assert!(
            registry
                .unprotect_audio(&unknown.protect_audio(&payload))
                .is_err()
        );
    }

    #[test]
    fn first_empty_group_roster_removes_the_seeded_direct_peer() {
        let epoch = [0x42; 32];
        let peer = Jid::new("200002", Server::Lid).with_device(2);
        let mut sender = peer_sender(&epoch, &peer);
        let mut registry = registry();
        registry
            .seed_direct_peer(&epoch, &peer.to_non_ad(), &peer, true)
            .expect("direct peer");
        assert!(
            registry
                .unprotect_audio(&sender.protect_audio(&[0x50; 20]))
                .is_ok(),
            "the direct fallback is active before the first roster"
        );

        registry
            .apply_group_update(&update(1, vec![device("100001", 1, 1)]))
            .expect("authoritative empty remote roster");
        assert!(registry.active_participant_ids().is_empty());
        assert!(
            registry
                .unprotect_audio(&sender.protect_audio(&[0x51; 20]))
                .is_err(),
            "the first authoritative roster must revoke a departed direct peer"
        );
    }

    #[test]
    fn seeded_direct_peer_keeps_keys_when_adopting_its_first_pid() {
        let epoch = [0x43; 32];
        let peer = Jid::new("200002", Server::Lid).with_device(2);
        let mut sender = peer_sender(&epoch, &peer);
        let mut registry = registry();
        registry
            .seed_direct_peer(&epoch, &peer.to_non_ad(), &peer, true)
            .expect("direct peer");
        assert!(
            registry
                .unprotect_audio(&sender.protect_audio(&[0x50; 20]))
                .is_ok()
        );

        registry
            .apply_group_update(&update(
                1,
                vec![device("100001", 1, 1), device("200002", 2, 2)],
            ))
            .expect("first PID-bearing roster");
        assert!(
            registry
                .unprotect_audio(&sender.protect_audio(&[0x51; 20]))
                .is_ok(),
            "None-to-Some PID adoption must retain the authenticated direct receiver"
        );
    }

    #[test]
    fn colliding_routes_do_not_advance_or_replace_the_roster() {
        let mut registry = registry();
        registry
            .apply_group_update(&update(
                1,
                vec![device("100001", 1, 1), device("200002", 2, 2)],
            ))
            .expect("initial roster");
        registry
            .apply_raw_epoch(1, &[0x42; 32])
            .expect("initial epoch");
        let previous_participants = registry.active_participant_ids();

        let collisions = [
            ("audio", 0, "37774", "53838"),
            ("video", VIDEO_SSRC_SLOT_WORD, "52371", "59782"),
            ("app-data", APP_DATA_SSRC_SLOT_WORD, "19671", "43135"),
            ("RTCP", 1, "25851", "41568"),
        ];
        for (route, slot_word, first, second) in collisions {
            let first_id = format_e2e_srtp_participant_id(
                &Jid::new(first, Server::Lid).with_device(1).to_string(),
            );
            let second_id = format_e2e_srtp_participant_id(
                &Jid::new(second, Server::Lid).with_device(1).to_string(),
            );
            assert_eq!(
                derive_wasm_participant_ssrc("CALL", &first_id, slot_word),
                derive_wasm_participant_ssrc("CALL", &second_id, slot_word),
                "{route} collision fixture no longer exercises the intended route"
            );

            let colliding = update(
                2,
                vec![
                    device("100001", 1, 1),
                    device(first, 1, 2),
                    device(second, 1, 3),
                ],
            );
            assert_eq!(
                registry.apply_group_update(&colliding),
                Err(GroupMediaError::InvalidSnapshot),
                "{route} route collision must reject the snapshot"
            );
            assert_eq!(registry.roster_transaction(), Some(1));
            assert_eq!(
                registry.active_participant_ids(),
                previous_participants,
                "{route} route collision must preserve the active roster"
            );
        }
    }

    #[test]
    fn same_device_receiver_survives_roster_update_and_departure_removes_route() {
        let epoch = [0x24; 32];
        let alice = Jid::new("200002", Server::Lid).with_device(2);
        let mut sender = peer_sender(&epoch, &alice);
        let mut registry = registry();
        registry
            .apply_group_update(&update(
                2,
                vec![device("100001", 1, 1), device("200002", 2, 2)],
            ))
            .unwrap();
        registry.apply_raw_epoch(2, &epoch).unwrap();
        let first = sender.protect_audio(&[0x50; 20]);
        assert!(registry.unprotect_audio(&first).is_ok());

        registry
            .apply_group_update(&update(
                3,
                vec![
                    device("100001", 1, 1),
                    device("200002", 2, 2),
                    device("300003", 3, 3),
                ],
            ))
            .unwrap();
        let second = sender.protect_audio(&[0x51; 20]);
        assert!(registry.unprotect_audio(&second).is_ok());

        registry
            .apply_group_update(&update(4, vec![device("100001", 1, 1)]))
            .unwrap();
        assert!(
            registry
                .unprotect_audio(&sender.protect_audio(&[0x52; 20]))
                .is_err()
        );
    }

    #[test]
    fn device_pid_change_rebuilds_its_receive_session() {
        let epoch = [0x25; 32];
        let alice = Jid::new("200002", Server::Lid).with_device(2);
        let participants = vec![device("100001", 1, 1), device("200002", 2, 2)];
        let mut registry = registry();
        registry
            .apply_group_update(&update(1, participants.clone()))
            .expect("initial roster");
        registry.apply_raw_epoch(1, &epoch).expect("initial epoch");

        let mut first_session = peer_sender(&epoch, &alice);
        assert!(
            registry
                .unprotect_audio(&first_session.protect_audio(&[0x50; 20]))
                .is_ok()
        );

        let mut migrated = participants;
        migrated[1].devices[0].pid = Some(9);
        registry
            .apply_group_update(&update(2, migrated))
            .expect("PID migration");

        let mut replacement_session = peer_sender(&epoch, &alice);
        assert!(
            registry
                .unprotect_audio(&replacement_session.protect_audio(&[0x51; 20]))
                .is_ok(),
            "a new PID must start with fresh ROC, replay, and depacketization state"
        );
    }

    #[test]
    fn audio_only_roster_update_disables_retained_video_receiver() {
        let epoch = [0x35; 32];
        let alice = Jid::new("200002", Server::Lid).with_device(2);
        let participants = vec![device("100001", 1, 1), device("200002", 2, 2)];
        let mut registry = registry();
        registry
            .apply_group_update(&update(1, participants.clone()))
            .unwrap();
        registry.apply_raw_epoch(1, &epoch).unwrap();
        let mut sender = peer_video_sender(&epoch, &alice);
        let access_unit = [0, 0, 0, 1, 0x65, 0x88, 0x84, 0x21];
        let initial = sender.protect_video(&access_unit);
        assert!(
            initial
                .iter()
                .any(|packet| registry.unprotect_video(packet).is_some())
        );

        let mut audio_only = update(2, participants.clone());
        audio_only.media = "audio".to_string();
        registry.apply_group_update(&audio_only).unwrap();
        assert!(
            sender
                .protect_video(&access_unit)
                .iter()
                .all(|packet| registry.unprotect_video(packet).is_none()),
            "PT-97 must be dropped while the authoritative media mode is audio"
        );

        registry
            .apply_group_update(&update(3, participants))
            .unwrap();
        assert!(
            sender
                .protect_video(&access_unit)
                .iter()
                .any(|packet| registry.unprotect_video(packet).is_some()),
            "a later video roster may reactivate the retained receiver"
        );
    }

    #[test]
    fn audio_downgrade_discards_partial_video_access_units() {
        let epoch = [0x35; 32];
        let alice = Jid::new("200002", Server::Lid).with_device(2);
        let participants = vec![device("100001", 1, 1), device("200002", 2, 2)];
        let mut registry = registry();
        registry
            .apply_group_update(&update(1, participants.clone()))
            .unwrap();
        registry.apply_raw_epoch(1, &epoch).unwrap();
        let mut sender = peer_video_sender(&epoch, &alice);
        let mut fragmented = vec![0, 0, 0, 1, 0x65];
        fragmented.resize(3_005, 0x88);
        let packets = sender.protect_video(&fragmented);
        assert!(packets.len() > 1, "fixture must create a fragmented AU");
        for packet in &packets[..packets.len() - 1] {
            assert!(
                registry
                    .unprotect_video(packet)
                    .is_some_and(|video| video.access_units.is_empty())
            );
        }

        let mut audio_only = update(2, participants.clone());
        audio_only.media = "audio".to_string();
        registry.apply_group_update(&audio_only).unwrap();
        registry
            .apply_group_update(&update(3, participants))
            .unwrap();
        assert!(
            registry
                .unprotect_video(packets.last().unwrap())
                .is_some_and(|video| video.access_units.is_empty()),
            "reactivation must not complete an access unit retained before the downgrade"
        );

        let fresh = [0, 0, 0, 1, 0x65, 0x88, 0x84, 0x21];
        assert!(
            sender.protect_video(&fresh).iter().any(|packet| registry
                .unprotect_video(packet)
                .is_some_and(|video| !video.access_units.is_empty())),
            "resetting the depacketizer must preserve the receiver's SRTP state"
        );
    }

    #[test]
    fn duplicate_and_conflicting_epochs_are_distinguished() {
        let mut registry = registry();
        registry
            .apply_group_update(&update(5, vec![device("200002", 2, 2)]))
            .unwrap();
        assert_eq!(
            registry.apply_raw_epoch(5, &[0x11; 32]).unwrap(),
            GroupEpochApply::Installed
        );
        assert_eq!(
            registry.apply_raw_epoch(5, &[0x11; 32]).unwrap(),
            GroupEpochApply::Duplicate
        );
        assert_eq!(
            registry.apply_raw_epoch(5, &[0x22; 32]),
            Err(GroupMediaError::ConflictingEpoch)
        );
        assert_eq!(
            registry.apply_raw_epoch(4, &[0x33; 32]).unwrap(),
            GroupEpochApply::Stale
        );
    }

    #[test]
    fn future_epoch_buffer_preserves_the_nearest_usable_transaction() {
        let mut registry = registry();
        for transaction in 100..100 + MAX_BUFFERED_EPOCHS as u32 {
            assert_eq!(
                registry
                    .apply_raw_epoch(transaction, &[transaction as u8; 32])
                    .unwrap(),
                GroupEpochApply::Buffered
            );
        }
        assert_eq!(
            registry.apply_raw_epoch(1, &[0x42; 32]).unwrap(),
            GroupEpochApply::Buffered
        );
        assert_eq!(registry.pending_epochs.len(), MAX_BUFFERED_EPOCHS);
        assert!(
            registry.pending_epochs.contains_key(&1),
            "a legitimate next epoch must displace the farthest speculative key"
        );
        assert!(
            !registry
                .pending_epochs
                .contains_key(&(100 + MAX_BUFFERED_EPOCHS as u32 - 1))
        );
        registry
            .apply_group_update(&update(1, vec![device("200002", 2, 2)]))
            .expect("next roster");
        assert_eq!(
            registry.installed_epoch_transaction(),
            Some(1),
            "far-future epochs must not suppress the next authoritative key"
        );
    }

    #[test]
    fn video_routes_are_participant_specific_for_multiple_devices() {
        let epoch = [0x57; 32];
        let alice = Jid::new("200002", Server::Lid).with_device(2);
        let bob = Jid::new("300003", Server::Lid).with_device(3);
        let mut registry = registry();
        registry
            .apply_group_update(&update(
                1,
                vec![
                    device("100001", 1, 1),
                    device("200002", 2, 2),
                    device("300003", 3, 3),
                ],
            ))
            .unwrap();
        registry.apply_raw_epoch(1, &epoch).unwrap();

        let access_unit = [0, 0, 0, 1, 0x65, 0x88, 0x84, 0x21];
        for peer in [&alice, &bob] {
            let packets = peer_video_sender(&epoch, peer).protect_video(&access_unit);
            let decoded = packets
                .iter()
                .find_map(|packet| registry.unprotect_video(packet))
                .expect("participant video packet");
            assert_eq!(decoded.device_jid, *peer);
            assert_eq!(decoded.access_units, [access_unit.to_vec()]);
        }
    }

    #[test]
    fn pid_migration_rule_is_shared_by_receiver_and_mixer_updates() {
        assert!(!pid_migrated(None, None));
        assert!(!pid_migrated(None, Some(7)));
        assert!(!pid_migrated(Some(7), Some(7)));
        assert!(pid_migrated(Some(7), Some(8)));
        assert!(pid_migrated(Some(7), None));
    }
}

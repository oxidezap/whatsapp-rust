//! Call state machine and the media pipeline composition (Opus payload to RTP WARP header to
//! E2E SRTP protect, and the reverse). The byte-level crypto/framing lives in the sibling
//! `wacore::voip` modules; this stitches it together. Pure logic: no socket, clock, or runtime.

use super::audio::AudioFormat;
use super::e2e_srtp::{
    E2eSrtpKeys, RecvRocTracker, RocTracker, append_warp_mi_tag_in_place, crypt_payload,
    crypt_payload_in_place, derive_e2e_keys, derive_e2e_keys_from_raw, derive_srtcp_keys,
    derive_srtcp_keys_from_raw, protect_srtcp, unprotect_srtcp, verify_warp_mi_tag,
};
use super::h264::{H264_MAX_AU_BYTES, H264Depacketizer, PacketizedAu, au_has_idr, packetize_au};
use super::rtcp::{
    RtcpReceptionReport, RtcpSenderStats, WHATSAPP_RTCP_CNAME_LEN,
    build_whatsapp_picture_loss_indication, build_whatsapp_rtcp_cname,
    build_whatsapp_sender_report_with_sdes, build_whatsapp_source_description,
    parse_rtcp_sender_ssrc,
};
use super::rtp::{
    RTP_FIXED_HEADER_LEN, RtpHeader, RtpStream, VIDEO_MEDIA_FRAME_INFO_DELTA,
    VIDEO_MEDIA_FRAME_INFO_IDR, VideoRtpStream, encode_rtp_header_into, parse_rtp_header,
    rtp_header_byte_length,
};
use super::ssrc::format_e2e_srtp_participant_id;
use crate::types::group_call::GroupCallUpdate;
use wacore_binary::Jid;

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
    /// The single media profile selected for this call.
    pub audio_format: Option<AudioFormat>,
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

const SRTCP_INDEX_MASK: u32 = 0x7fff_ffff;
const SRTCP_INDEX_HALF_RANGE: u32 = 1 << 30;
const SRTCP_REPLAY_WINDOW_BITS: u32 = 64;
const SRTCP_REPLAY_STREAM_CAP: usize = 16;

#[derive(Default)]
struct SrtcpReplayWindow {
    highest: Option<u32>,
    seen: u64,
}

impl SrtcpReplayWindow {
    fn accept(&mut self, index: u32) -> bool {
        let index = index & SRTCP_INDEX_MASK;
        let Some(highest) = self.highest else {
            self.highest = Some(index);
            self.seen = 1;
            return true;
        };
        let forward = index.wrapping_sub(highest) & SRTCP_INDEX_MASK;
        if forward == 0 {
            return false;
        }
        if forward < SRTCP_INDEX_HALF_RANGE {
            self.seen = if forward >= SRTCP_REPLAY_WINDOW_BITS {
                1
            } else {
                (self.seen << forward) | 1
            };
            self.highest = Some(index);
            return true;
        }
        let behind = highest.wrapping_sub(index) & SRTCP_INDEX_MASK;
        if behind >= SRTCP_REPLAY_WINDOW_BITS {
            return false;
        }
        let bit = 1u64 << behind;
        if self.seen & bit != 0 {
            return false;
        }
        self.seen |= bit;
        true
    }
}

#[derive(Default)]
struct SrtcpReplayState {
    streams: Vec<(u32, SrtcpReplayWindow)>,
}

impl SrtcpReplayState {
    fn accept(&mut self, sender_ssrc: u32, index: u32) -> bool {
        if let Some((_, window)) = self
            .streams
            .iter_mut()
            .find(|(ssrc, _)| *ssrc == sender_ssrc)
        {
            return window.accept(index);
        }
        if self.streams.len() >= SRTCP_REPLAY_STREAM_CAP {
            return false;
        }
        let mut window = SrtcpReplayWindow::default();
        let accepted = window.accept(index);
        self.streams.push((sender_ssrc, window));
        accepted
    }
}

/// How long a just-retired video SSRC stays ignorable, in authenticated packets of either stream.
///
/// The window a renumbering's stragglers arrive in is a network reordering window -- milliseconds --
/// and 15fps video puts a handful of packets in one. Sized well above that and far below any real
/// gap, so it covers the overlap without outliving it: a stream still arriving after this many
/// packets is not a straggler, it is the peer's current stream, and it takes the depacketizer back.
const RETIRED_SSRC_GRACE_PACKETS: u32 = 64;

/// Consecutive packets a retired SSRC must deliver, once its grace has expired, before it takes the
/// depacketizer back.
///
/// Expiring the grace must not turn ONE very late packet into a commitment to its stream: reclaiming
/// on a single straggler makes the peer's actual current stream the retired one, and it is then
/// ignored for a whole fresh grace window -- a video freeze caused by the straggler the grace exists
/// to absorb. A resumed stream keeps arriving and clears this in a few packets; a lone latecomer
/// never does. Any packet from the stream in possession resets the count, so only an uninterrupted
/// run counts.
/// How many previously-left SSRCs are remembered; see `retired_ssrcs`.
const RETIRED_SSRC_MEMORY: usize = 4;

const RETIRED_SSRC_RESUME_PACKETS: u32 = 3;

const SRTP_REPLAY_WINDOW_BITS: u64 = 64;
/// Concurrent inbound RTP streams tracked per pipeline, matching [`SRTCP_REPLAY_STREAM_CAP`].
///
/// One is the norm. A peer that renumbers its SSRC mid-call adds a second, and the bound keeps a
/// peer that renumbers on every packet from growing this without limit.
const SRTP_REPLAY_STREAM_CAP: usize = 16;

/// Per-SSRC inbound RTP state: rollover counter and replay window.
///
/// Both are indexed by sequence number, and a sequence number only means something within one
/// stream. Sharing them across SSRCs is what the RTCP side already avoids: two interleaved streams
/// would each look to the other like a huge jump backwards, so roughly half the packets would fail
/// their tag or be rejected as replays. Silently, and only for a peer that happens to use two
/// SSRCs.
#[derive(Default)]
struct SrtpRecvStreams {
    /// The first stream, held inline.
    ///
    /// A 1:1 call has exactly one inbound SSRC, so this is the case that runs on every packet of
    /// every call. Spilling it to the heap to serve a second stream that usually never arrives puts
    /// an allocation on the first packet of every call, and measured, that allocation was the entire
    /// cost of making this per-SSRC in the first place.
    primary: Option<(u32, RecvRocTracker, SrtpReplayWindow)>,
    /// Streams past the first, allocated only if a peer really does renumber or use several SSRCs.
    overflow: Vec<(u32, RecvRocTracker, SrtpReplayWindow)>,
}

impl SrtpRecvStreams {
    /// The rollover counter to authenticate `seq` against, WITHOUT allocating anything.
    ///
    /// A stream never seen before estimates from a fresh counter, which is what one would answer
    /// anyway. Nothing is allocated here on purpose; see [`Self::commit_mut`].
    fn estimate_roc(&self, ssrc: u32, seq: u16) -> u32 {
        self.primary
            .iter()
            .chain(self.overflow.iter())
            .find(|(known, _, _)| *known == ssrc)
            .map_or_else(
                || RecvRocTracker::default().estimate_roc(seq),
                |(_, roc, _)| roc.estimate_roc(seq),
            )
    }

    /// Borrow the state for an AUTHENTICATED packet's SSRC, creating it on first sight.
    ///
    /// Called only after the WARP MI tag verifies, which is the point: the SSRC comes from the
    /// unauthenticated RTP header, so allocating on sight would let anyone able to inject datagrams
    /// spend the whole table on forged SSRCs before the peer's first real packet and leave the call
    /// permanently deaf.
    ///
    /// `None` once the stream cap is reached, which drops the packet rather than evicting a live
    /// stream: evicting would reset a rollover counter that a real stream is still using. Reaching
    /// the cap now requires that many distinct SSRCs to have each produced a packet with a valid tag.
    fn commit_mut(&mut self, ssrc: u32) -> Option<(&mut RecvRocTracker, &mut SrtpReplayWindow)> {
        // `matches!` before the borrow: taking `&mut self.primary` inside the condition would hold
        // it across the fallthrough and the borrow checker would reject the overflow path below.
        if self.primary.is_none() || matches!(self.primary, Some((known, _, _)) if known == ssrc) {
            let (_, roc, replay) = self.primary.get_or_insert((
                ssrc,
                RecvRocTracker::default(),
                SrtpReplayWindow::default(),
            ));
            return Some((roc, replay));
        }
        if let Some(index) = self
            .overflow
            .iter()
            .position(|(known, _, _)| *known == ssrc)
        {
            let (_, roc, replay) = &mut self.overflow[index];
            return Some((roc, replay));
        }
        // The cap counts every tracked stream, the inline one included.
        if self.overflow.len() + 1 >= SRTP_REPLAY_STREAM_CAP {
            return None;
        }
        self.overflow
            .push((ssrc, RecvRocTracker::default(), SrtpReplayWindow::default()));
        let (_, roc, replay) = self.overflow.last_mut().expect("just pushed");
        Some((roc, replay))
    }
}

#[derive(Default)]
struct SrtpReplayWindow {
    highest: Option<u64>,
    seen: u64,
}

impl SrtpReplayWindow {
    fn accept(&mut self, index: u64) -> bool {
        let Some(highest) = self.highest else {
            self.highest = Some(index);
            self.seen = 1;
            return true;
        };
        if index > highest {
            let forward = index - highest;
            self.seen = if forward >= SRTP_REPLAY_WINDOW_BITS {
                1
            } else {
                (self.seen << forward) | 1
            };
            self.highest = Some(index);
            return true;
        }
        let behind = highest - index;
        if behind >= SRTP_REPLAY_WINDOW_BITS {
            return false;
        }
        let bit = 1u64 << behind;
        if self.seen & bit != 0 {
            return false;
        }
        self.seen |= bit;
        true
    }
}

/// Per-stream RTCP Sender-Report state.
struct SrtcpSender {
    keys: E2eSrtpKeys,
    cname: [u8; WHATSAPP_RTCP_CNAME_LEN],
    index: u32,
    packets_sent: u32,
    octets_sent: u32,
    profile_extension: bool,
}

impl SrtcpSender {
    fn new(
        call_key: &[u8],
        self_lid: &str,
        cname: [u8; WHATSAPP_RTCP_CNAME_LEN],
        profile_extension: bool,
    ) -> Option<Self> {
        Some(Self::from_keys(
            derive_srtcp_keys(call_key, &format_e2e_srtp_participant_id(self_lid))?,
            cname,
            profile_extension,
        ))
    }

    fn from_keys(
        keys: E2eSrtpKeys,
        cname: [u8; WHATSAPP_RTCP_CNAME_LEN],
        profile_extension: bool,
    ) -> Self {
        Self {
            keys,
            cname,
            // libSRTP increments its SRTCP sender index before protecting the first packet.
            index: 1,
            packets_sent: 0,
            octets_sent: 0,
            profile_extension,
        }
    }

    fn record(&mut self, packets: u32, octets: usize) {
        self.packets_sent = self.packets_sent.wrapping_add(packets);
        self.octets_sent = self.octets_sent.wrapping_add(octets as u32);
    }

    fn replace_keys(&mut self, keys: E2eSrtpKeys) {
        self.keys = keys;
    }

    fn protect(&mut self, ssrc: u32, plain: &[u8]) -> Vec<u8> {
        let out = protect_srtcp(&self.keys, ssrc, self.index, plain);
        self.index = self.index.wrapping_add(1);
        out
    }

    /// Build and SRTCP-protect a Picture Loss Indication naming `media_ssrc`.
    ///
    /// On this sender's own profile, like every other report it emits: the bit
    /// is a property of the session, not of the packet kind.
    fn picture_loss_indication(&mut self, ssrc: u32, media_ssrc: u32) -> Vec<u8> {
        self.protect(
            ssrc,
            &build_whatsapp_picture_loss_indication(ssrc, media_ssrc, self.profile_extension),
        )
    }

    fn source_description(&mut self, ssrc: u32) -> Vec<u8> {
        self.protect(
            ssrc,
            &build_whatsapp_source_description(ssrc, &self.cname, self.profile_extension),
        )
    }

    /// Build and SRTCP-protect a Sender Report for `ssrc` at wall-clock `now_ms`.
    fn sender_report(
        &mut self,
        ssrc: u32,
        rtp_timestamp: u32,
        now_ms: u64,
        report: Option<&RtcpReceptionReport>,
    ) -> Vec<u8> {
        let stats = RtcpSenderStats {
            packets_sent: self.packets_sent,
            octets_sent: self.octets_sent,
            rtp_timestamp,
        };
        let plain = build_whatsapp_sender_report_with_sdes(
            ssrc,
            &stats,
            now_ms,
            &self.cname,
            report,
            self.profile_extension,
        );
        self.protect(ssrc, &plain)
    }
}

/// Composes the outbound (protect) and inbound (unprotect) media pipeline for E2E 1:1.
/// SFrame is omitted (default-off on send; plain Opus inside WAHKDF SRTP).
pub struct MediaPipeline {
    send_keys: E2eSrtpKeys,
    recv_keys: E2eSrtpKeys,
    warp_mi_tag_len: usize,
    rtp: RtpStream,
    send_roc: RocTracker,
    recv_streams: SrtpRecvStreams,
    srtcp: SrtcpSender,
    recv_srtcp_keys: E2eSrtpKeys,
    recv_srtcp_replay: SrtcpReplayState,
}

pub(crate) struct SendRekey {
    rtp: E2eSrtpKeys,
    rtcp: E2eSrtpKeys,
}

/// Borrowed inputs for [`MediaPipeline::new`]. `self_lid`/`peer_lid` are the E2E-SRTP
/// participant JIDs (normalized inside `new`).
#[derive(Clone, Copy)]
pub struct MediaPipelineParams<'a> {
    pub call_key: &'a [u8],
    pub self_lid: &'a str,
    pub peer_lid: &'a str,
    pub ssrc: u32,
    pub samples_per_packet: u32,
    pub warp_mi_tag_len: usize,
}

impl MediaPipeline {
    /// Derive both directions from the 32-byte callKey. The HKDF `info` is the *sender's* own
    /// participant id, so send keys come from our self LID and recv keys from the peer LID
    /// (SFrame uses the opposite convention). JIDs are normalized with the E2E-SRTP
    /// participant-id rule (keep an existing `:device`, bare `@lid` becomes `:0@lid`), which
    /// must match the form the peer derives our SSRC from.
    /// Returns `None` when `call_key` is shorter than 32 bytes (a malformed peer callKey).
    pub fn new(p: &MediaPipelineParams<'_>) -> Option<Self> {
        let mut entropy = [0u8; 12];
        entropy[6..10].copy_from_slice(&p.ssrc.to_be_bytes());
        if let Some(call_prefix) = p.call_key.get(..2) {
            entropy[10..].copy_from_slice(call_prefix);
        }
        Self::new_with_rtcp_cname(p, build_whatsapp_rtcp_cname(&entropy))
    }

    pub(crate) fn new_with_rtcp_cname(
        p: &MediaPipelineParams<'_>,
        rtcp_cname: [u8; WHATSAPP_RTCP_CNAME_LEN],
    ) -> Option<Self> {
        // The WARP MI tag is sliced from the 20-byte HMAC-SHA1 digest; a relay-advertised length
        // above 20 (or zero) would panic on the first packet, so reject it at setup instead.
        if !(1..=20).contains(&p.warp_mi_tag_len) {
            return None;
        }
        Some(Self {
            send_keys: derive_e2e_keys(p.call_key, &format_e2e_srtp_participant_id(p.self_lid))?,
            recv_keys: derive_e2e_keys(p.call_key, &format_e2e_srtp_participant_id(p.peer_lid))?,
            warp_mi_tag_len: p.warp_mi_tag_len,
            rtp: RtpStream::new(p.ssrc, p.samples_per_packet, false),
            send_roc: RocTracker::default(),
            recv_streams: SrtpRecvStreams::default(),
            srtcp: SrtcpSender::new(p.call_key, p.self_lid, rtcp_cname, false)?,
            recv_srtcp_keys: derive_srtcp_keys(
                p.call_key,
                &format_e2e_srtp_participant_id(p.peer_lid),
            )?,
            recv_srtcp_replay: SrtcpReplayState::default(),
        })
    }

    pub fn send_ssrc(&self) -> u32 {
        self.rtp.ssrc
    }

    pub(crate) fn set_send_ssrc(&mut self, ssrc: u32) {
        self.rtp.ssrc = ssrc;
    }

    /// Select the negotiated audio RTP payload type without resetting sequence/timestamp state.
    pub fn set_audio_payload_type(&mut self, payload_type: u8) -> bool {
        self.rtp.set_payload_type(payload_type)
    }

    /// Select MLOW's TOC-aware DTX and marker behavior independently from the shared payload type.
    pub fn set_audio_mlow_profile(&mut self, enabled: bool) {
        self.rtp.set_mlow_profile(enabled);
    }

    /// An SRTCP-protected Sender Report for the audio stream (our send SSRC), or the accumulated
    /// packet/octet totals since the call began. Emitted periodically by the engine.
    pub(crate) fn audio_sender_report(
        &mut self,
        now_ms: u64,
        report: Option<&RtcpReceptionReport>,
    ) -> Vec<u8> {
        self.srtcp
            .sender_report(self.rtp.ssrc, self.rtp.rtp_timestamp(), now_ms, report)
    }

    /// The native client sends this once when the audio RTCP session is associated.
    pub fn audio_source_description(&mut self) -> Vec<u8> {
        self.srtcp.source_description(self.rtp.ssrc)
    }

    /// Caller-side: re-derive the recv keys for the device that actually answered. We dial the base
    /// callee LID, but a multi-device callee answers from one device (e.g. `:2`) and encrypts under
    /// its OWN participant id; keeping the base-LID recv keys decrypts every inbound frame to garbage.
    /// Send keys are untouched (they key on our self LID). The recv ROC resets: the answerer's RTP
    /// stream is fresh, so a stale `s_l` would mis-guess the index of its first packets. Returns
    /// `false` only on a malformed `call_key` (a setup invariant already checked in [`new`](Self::new)).
    pub fn rekey_recv(&mut self, call_key: &[u8], answering_peer_lid: &str) -> bool {
        let participant_id = format_e2e_srtp_participant_id(answering_peer_lid);
        let Some(keys) = derive_e2e_keys(call_key, &participant_id) else {
            return false;
        };
        let Some(srtcp_keys) = derive_srtcp_keys(call_key, &participant_id) else {
            return false;
        };
        self.recv_keys = keys;
        self.recv_srtcp_keys = srtcp_keys;
        self.recv_streams = SrtpRecvStreams::default();
        self.recv_srtcp_replay = SrtcpReplayState::default();
        true
    }

    /// Install a transaction-wide group epoch for outbound RTP and SRTCP.
    ///
    /// RTP sequence/timestamp/ROC and SRTCP index/CNAME/statistics are preserved.
    pub fn rekey_send_from_raw(&mut self, raw_epoch: &[u8], self_lid: &str) -> bool {
        let Some(rekey) = Self::prepare_send_rekey(raw_epoch, self_lid) else {
            return false;
        };
        self.commit_send_rekey(rekey);
        true
    }

    pub(crate) fn prepare_send_rekey(raw_epoch: &[u8], self_lid: &str) -> Option<SendRekey> {
        let participant_id = format_e2e_srtp_participant_id(self_lid);
        Some(SendRekey {
            rtp: derive_e2e_keys_from_raw(raw_epoch, &participant_id)?,
            rtcp: derive_srtcp_keys_from_raw(raw_epoch, &participant_id)?,
        })
    }

    pub(crate) fn commit_send_rekey(&mut self, rekey: SendRekey) {
        self.send_keys = rekey.rtp;
        self.srtcp.replace_keys(rekey.rtcp);
    }

    /// Install a transaction-wide group epoch for one peer without resetting
    /// its authenticated RTP ROC or SRTCP replay windows.
    pub fn rekey_recv_from_raw_preserving_roc(&mut self, raw_epoch: &[u8], peer_lid: &str) -> bool {
        let participant_id = format_e2e_srtp_participant_id(peer_lid);
        let Some(recv_keys) = derive_e2e_keys_from_raw(raw_epoch, &participant_id) else {
            return false;
        };
        let Some(srtcp_keys) = derive_srtcp_keys_from_raw(raw_epoch, &participant_id) else {
            return false;
        };
        self.recv_keys = recv_keys;
        self.recv_srtcp_keys = srtcp_keys;
        true
    }

    /// Outbound: wrap an Opus payload in an RTP WARP header, E2E-SRTP encrypt, append the WARP MI tag.
    pub fn protect_audio(&mut self, opus_payload: &[u8]) -> Vec<u8> {
        let header = self.rtp.next_packet(opus_payload, false);
        let roc = self.send_roc.advance(header.sequence_number);
        self.srtcp.record(1, opus_payload.len());
        protect_srtp_packet(
            &self.send_keys,
            &header,
            roc,
            self.warp_mi_tag_len,
            opus_payload,
        )
    }

    /// Inbound: verify the WARP MI tag, parse the header, decrypt the payload.
    /// The ROC is derived per-packet from the recv tracker (RFC 3711 guess-index), so the keystream
    /// stays aligned with the sender's across 16-bit seq wraps even under reorder/loss.
    ///
    /// The tag is authenticated (constant-time) against the *estimated* ROC BEFORE
    /// that ROC is committed, so an on-path relay can't fold unauthenticated packets
    /// into the rollover counter and permanently desync the receiver (RFC 3711
    /// §3.3.1 requires the index update to follow authentication).
    pub fn unprotect_audio(&mut self, packet: &[u8]) -> Option<(RtpHeader, Vec<u8>)> {
        unprotect_srtp_packet(
            &self.recv_keys,
            &mut self.recv_streams,
            self.warp_mi_tag_len,
            packet,
        )
    }

    /// Authenticate and decrypt peer SRTCP using the sender SSRC left clear on the wire.
    pub fn unprotect_rtcp(&mut self, packet: &[u8]) -> Option<Vec<u8>> {
        let sender_ssrc = parse_rtcp_sender_ssrc(packet)?;
        let (plain, index) = unprotect_srtcp(&self.recv_srtcp_keys, sender_ssrc, packet)?;
        self.recv_srtcp_replay
            .accept(sender_ssrc, index)
            .then_some(plain)
    }
}

/// Shared outbound SRTP step for the audio and video pipelines: build the whole
/// protected packet -- RTP header, AES-CTR ciphertext, WARP MI tag -- inside one
/// allocation sized exactly for it. The payload is copied in once and encrypted where
/// it lands, so a send costs one `Vec` per packet rather than one per stage, which is
/// what the video path (~40 packets per frame, ~1200/s at 30 fps) actually pays.
fn protect_srtp_packet(
    send_keys: &E2eSrtpKeys,
    header: &RtpHeader,
    roc: u32,
    warp_mi_tag_len: usize,
    payload: &[u8],
) -> Vec<u8> {
    let mut packet = Vec::with_capacity(header.byte_size() + payload.len() + warp_mi_tag_len);
    encode_rtp_header_into(header, &mut packet);
    let payload_start = packet.len();
    packet.extend_from_slice(payload);
    crypt_payload_in_place(
        send_keys,
        header.ssrc,
        header.sequence_number,
        roc,
        &mut packet[payload_start..],
    );
    append_warp_mi_tag_in_place(&send_keys.auth_key, &mut packet, roc, warp_mi_tag_len);
    packet
}

/// Shared inbound SRTP step for the audio and video pipelines: verify the WARP
/// MI tag against the *estimated* ROC BEFORE committing it, then decrypt. The
/// order is load-bearing (RFC 3711 §3.3.1): an on-path relay must not be able
/// to fold unauthenticated packets into the rollover counter and permanently
/// desync the receiver.
fn unprotect_srtp_packet(
    recv_keys: &E2eSrtpKeys,
    recv_streams: &mut SrtpRecvStreams,
    warp_mi_tag_len: usize,
    packet: &[u8],
) -> Option<(RtpHeader, Vec<u8>)> {
    if packet.len() < RTP_FIXED_HEADER_LEN + warp_mi_tag_len {
        return None;
    }
    let split = packet.len() - warp_mi_tag_len;
    let without_tag = &packet[..split];
    let received_tag = &packet[split..];
    let header = parse_rtp_header(without_tag)?;
    let header_len = rtp_header_byte_length(without_tag)?;
    if without_tag.len() <= header_len {
        return None;
    }
    // Estimate against this SSRC's own counter without allocating for it. The SSRC is read from the
    // unauthenticated header, so committing state before the tag verifies would let anyone able to
    // inject datagrams fill the stream table with forged SSRCs and leave the call deaf.
    let roc = recv_streams.estimate_roc(header.ssrc, header.sequence_number);
    if !verify_warp_mi_tag(
        &recv_keys.auth_key,
        without_tag,
        roc,
        warp_mi_tag_len,
        received_tag,
    ) {
        return None;
    }
    // Authenticated: only now is it safe to allocate state for this SSRC and advance its counter.
    let (recv_roc, recv_replay) = recv_streams.commit_mut(header.ssrc)?;
    let index = (u64::from(roc) << 16) | u64::from(header.sequence_number);
    if !recv_replay.accept(index) {
        return None;
    }
    recv_roc.commit_roc(roc, header.sequence_number);
    let cipher = &without_tag[header_len..];
    let plain = crypt_payload(recv_keys, header.ssrc, header.sequence_number, roc, cipher);
    Some((header, plain))
}

/// Video sibling of [`MediaPipeline`]: same E2E-SRTP keys (per participant, not
/// per media type) and WARP MI tag, but H.264 packetization on top and its own
/// SSRC/sequencer. One access unit fans out to N RTP packets on send and is
/// reassembled from them on receive.
pub struct VideoPipeline {
    send_keys: E2eSrtpKeys,
    recv_keys: E2eSrtpKeys,
    warp_mi_tag_len: usize,
    rtp: VideoRtpStream,
    send_roc: RocTracker,
    recv_streams: SrtpRecvStreams,
    depacketizer: H264Depacketizer,
    /// SSRC whose fragments the depacketizer currently holds, once one has authenticated.
    ///
    /// The receive table tracks several SSRCs so a renumbering peer keeps its own rollover counter
    /// and replay window, but reassembly is one state machine keyed on sequence number and
    /// timestamp -- neither of which means anything across a stream boundary. Without this, a
    /// renumbered stream's restarted timestamps read as an old frame and its fragments splice onto
    /// the previous stream's.
    depacketizer_ssrc: Option<u32>,
    /// Every stream this one has left, newest last.
    ///
    /// A renumbering is not instantaneous on the wire: packets from the old SSRC keep arriving for
    /// a few milliseconds after the first packet of the new one. Each of those looks like another
    /// stream commitment, so without this they take the depacketizer back, discard whatever the new
    /// stream has half-assembled, and lose it again on the next new-SSRC fragment -- valid frames
    /// dropped for the whole overlap.
    ///
    /// Every one of them, not just the last: a peer that changes SSRC twice has two older streams
    /// that can still deliver a straggler, and one from the OLDER of them used to bypass both
    /// guards below -- the grace and the run -- because each only ever asked about the most recent.
    ///
    /// Bounded rather than permanent, in both senses. A late packet is late by milliseconds, so a
    /// short grace is enough and after it a stream may claim the depacketizer again by sustained
    /// delivery; retiring one forever would leave a peer that legitimately returns to a previous
    /// SSRC with no video at all. And the list itself is capped, so a peer that churns SSRCs cannot
    /// grow it without limit -- [`RETIRED_SSRC_MEMORY`] is well past what a call plausibly cycles
    /// through, and the oldest is dropped.
    retired_ssrcs: Vec<u32>,
    /// The retired SSRC the current run belongs to, so a run cannot be assembled out of packets
    /// from two different old streams.
    contender_ssrc: Option<u32>,
    packets_since_stream_change: u32,
    /// Consecutive packets from `contender_ssrc` since the last one from the stream in possession.
    retired_ssrc_run: u32,
    pkt_scratch: PacketizedAu,
    srtcp: SrtcpSender,
}

/// Borrowed inputs for [`VideoPipeline::new`]. No `samples_per_packet`: the
/// video timestamp advances by a fixed per-AU stride instead.
#[derive(Clone, Copy)]
pub struct VideoPipelineParams<'a> {
    pub call_key: &'a [u8],
    pub self_lid: &'a str,
    pub peer_lid: &'a str,
    pub ssrc: u32,
    pub ts_stride: u32,
    pub warp_mi_tag_len: usize,
}

impl VideoPipeline {
    pub fn new(p: &VideoPipelineParams<'_>) -> Option<Self> {
        let mut entropy = [0u8; 12];
        entropy[6..10].copy_from_slice(&p.ssrc.to_be_bytes());
        if let Some(call_prefix) = p.call_key.get(..2) {
            entropy[10..].copy_from_slice(call_prefix);
        }
        Self::new_with_rtcp_cname(p, build_whatsapp_rtcp_cname(&entropy))
    }

    pub(crate) fn new_with_rtcp_cname(
        p: &VideoPipelineParams<'_>,
        rtcp_cname: [u8; WHATSAPP_RTCP_CNAME_LEN],
    ) -> Option<Self> {
        // A zero stride would leave every AU at timestamp 0 (an unusable stream); reject it at
        // setup rather than emit a frozen clock.
        if !(1..=20).contains(&p.warp_mi_tag_len) || p.ts_stride == 0 {
            return None;
        }
        Some(Self {
            send_keys: derive_e2e_keys(p.call_key, &format_e2e_srtp_participant_id(p.self_lid))?,
            recv_keys: derive_e2e_keys(p.call_key, &format_e2e_srtp_participant_id(p.peer_lid))?,
            warp_mi_tag_len: p.warp_mi_tag_len,
            rtp: VideoRtpStream::new(p.ssrc, p.ts_stride)?,
            send_roc: RocTracker::default(),
            recv_streams: SrtpRecvStreams::default(),
            depacketizer: H264Depacketizer::default(),
            depacketizer_ssrc: None,
            retired_ssrcs: Vec::new(),
            contender_ssrc: None,
            packets_since_stream_change: 0,
            retired_ssrc_run: 0,
            pkt_scratch: PacketizedAu::default(),
            srtcp: SrtcpSender::new(p.call_key, p.self_lid, rtcp_cname, true)?,
        })
    }

    /// An SRTCP-protected Sender Report for the video stream.
    pub(crate) fn video_sender_report(
        &mut self,
        now_ms: u64,
        report: Option<&RtcpReceptionReport>,
    ) -> Vec<u8> {
        self.srtcp
            .sender_report(self.rtp.ssrc, self.rtp.rtp_timestamp(), now_ms, report)
    }

    pub fn send_ssrc(&self) -> u32 {
        self.rtp.ssrc
    }

    /// The peer stream this pipeline is reassembling, once one has authenticated.
    ///
    /// What a PLI names, and therefore what the rate at which we complain about
    /// it belongs to: a caller holding both can tell a second complaint about
    /// one picture from the first about a new one.
    pub(crate) fn inbound_ssrc(&self) -> Option<u32> {
        self.depacketizer_ssrc
    }

    /// Ask the peer for a keyframe, or `None` when there is nobody to ask.
    ///
    /// Addressed to `depacketizer_ssrc`, the peer stream this pipeline is
    /// actually reassembling, rather than to any SSRC that has ever
    /// authenticated: a PLI naming a stream the peer has renumbered away from
    /// asks for a reset of something it no longer sends. Absent until the
    /// first packet authenticates, which is the honest answer -- before that
    /// there is no inbound stream to have lost.
    ///
    /// On the native video profile, and protected under our own SSRC, like
    /// every other report this sender emits.
    pub(crate) fn picture_loss_indication(&mut self) -> Option<Vec<u8>> {
        let media_ssrc = self.depacketizer_ssrc?;
        let ours = self.rtp.ssrc;
        Some(self.srtcp.picture_loss_indication(ours, media_ssrc))
    }

    pub(crate) fn set_send_ssrc(&mut self, ssrc: u32) {
        self.rtp.ssrc = ssrc;
    }

    pub(crate) fn set_timestamp_stride(&mut self, ts_stride: u32) -> bool {
        self.rtp.set_timestamp_stride(ts_stride)
    }

    pub(crate) fn set_video_timestamp(&mut self, timestamp: u32) -> bool {
        self.rtp.set_timestamp(timestamp)
    }

    /// Same answering-device rekey as [`MediaPipeline::rekey_recv`]; the video
    /// recv keys are derived from the identical participant id, so they go
    /// stale together with the audio ones. The in-flight reassembly state is
    /// dropped: pre-rekey fragments decrypted to garbage anyway.
    pub fn rekey_recv(&mut self, call_key: &[u8], answering_peer_lid: &str) -> bool {
        let Some(keys) = derive_e2e_keys(
            call_key,
            &format_e2e_srtp_participant_id(answering_peer_lid),
        ) else {
            return false;
        };
        self.recv_keys = keys;
        self.recv_streams = SrtpRecvStreams::default();
        self.reset_depacketizer();
        true
    }

    /// Rotate outbound video RTP/SRTCP keys while preserving stream counters.
    pub fn rekey_send_from_raw(&mut self, raw_epoch: &[u8], self_lid: &str) -> bool {
        let Some(rekey) = Self::prepare_send_rekey(raw_epoch, self_lid) else {
            return false;
        };
        self.commit_send_rekey(rekey);
        true
    }

    pub(crate) fn prepare_send_rekey(raw_epoch: &[u8], self_lid: &str) -> Option<SendRekey> {
        let participant_id = format_e2e_srtp_participant_id(self_lid);
        Some(SendRekey {
            rtp: derive_e2e_keys_from_raw(raw_epoch, &participant_id)?,
            rtcp: derive_srtcp_keys_from_raw(raw_epoch, &participant_id)?,
        })
    }

    pub(crate) fn commit_send_rekey(&mut self, rekey: SendRekey) {
        self.send_keys = rekey.rtp;
        self.srtcp.replace_keys(rekey.rtcp);
    }

    /// Rotate inbound video keys while preserving the peer's RTP ROC and
    /// in-flight depacketization state.
    pub fn rekey_recv_from_raw_preserving_roc(&mut self, raw_epoch: &[u8], peer_lid: &str) -> bool {
        let Some(recv_keys) =
            derive_e2e_keys_from_raw(raw_epoch, &format_e2e_srtp_participant_id(peer_lid))
        else {
            return false;
        };
        self.recv_keys = recv_keys;
        true
    }

    /// Drop what the depacketizer holds without forgetting which streams have
    /// left.
    ///
    /// For a change on *our* side -- a local video downgrade and resume -- where
    /// the peer is still the peer: [`Self::reset_depacketizer`] would also clear
    /// `retired_ssrcs`, and a straggler from a stream the peer renumbered away
    /// from would then take possession of a plane that has just come back, so
    /// anything addressed to the stream being reassembled would name a stream
    /// nobody is sending.
    /// The retired list is the only thing kept: it is a fact about the peer,
    /// which did not change. The run and grace counters are evidence about
    /// packets that arrived before the pause, and a reclaim part-way through one
    /// would otherwise be completed by fewer stragglers than it takes to earn.
    pub(crate) fn reset_reassembly(&mut self) {
        self.depacketizer.reset();
        self.depacketizer_ssrc = None;
        self.contender_ssrc = None;
        self.packets_since_stream_change = 0;
        self.retired_ssrc_run = 0;
    }

    pub(crate) fn reset_depacketizer(&mut self) {
        self.depacketizer.reset();
        self.depacketizer_ssrc = None;
        self.retired_ssrcs.clear();
        self.contender_ssrc = None;
        self.packets_since_stream_change = 0;
        self.retired_ssrc_run = 0;
    }

    /// Outbound: packetize one Annex-B access unit and protect each RTP packet.
    pub fn protect_video(&mut self, au: &[u8]) -> Vec<Vec<u8>> {
        if au.len() > H264_MAX_AU_BYTES {
            return Vec::new();
        }
        // Taken out and put back so the fragment buffer's allocation lives for the whole
        // call while the loop below still has `&mut self` for the sequencer.
        let mut payloads = std::mem::take(&mut self.pkt_scratch);
        packetize_au(au, &mut payloads);
        let media_frame_info = if au_has_idr(au) {
            VIDEO_MEDIA_FRAME_INFO_IDR
        } else {
            VIDEO_MEDIA_FRAME_INFO_DELTA
        };
        let mut packets = Vec::with_capacity(payloads.len());
        let last = payloads.len().saturating_sub(1);
        for (i, payload) in payloads.iter().enumerate() {
            let header = self.rtp.next_video_packet(i == last, media_frame_info);
            let roc = self.send_roc.advance(header.sequence_number);
            self.srtcp.record(1, payload.len());
            packets.push(protect_srtp_packet(
                &self.send_keys,
                &header,
                roc,
                self.warp_mi_tag_len,
                payload,
            ));
        }
        self.pkt_scratch = payloads;
        packets
    }

    /// Inbound: authenticate+decrypt one RTP packet and feed the depacketizer;
    /// the reassembled access unit is returned on the AU's marker packet.
    pub fn unprotect_video(&mut self, packet: &[u8]) -> Option<Vec<Vec<u8>>> {
        let completed = self.unprotect_video_packet(packet)?.1;
        (!completed.is_empty()).then_some(completed)
    }

    pub(crate) fn unprotect_video_packet(
        &mut self,
        packet: &[u8],
    ) -> Option<(RtpHeader, Vec<Vec<u8>>)> {
        let (header, payload) = unprotect_srtp_packet(
            &self.recv_keys,
            &mut self.recv_streams,
            self.warp_mi_tag_len,
            packet,
        )?;
        // Counted BEFORE the grace check, and for every authenticated packet including the ones that
        // check ignores. Counting only the stream that replaced the retired one would never expire
        // the grace in the case that matters: a peer that sends one packet on a new SSRC and then
        // goes back to the old one delivers nothing but ignored packets, so the window would stay
        // open and its video would be frozen for the rest of the call -- the permanent failure the
        // bound exists to avoid, arrived at from the other side.
        self.packets_since_stream_change = self.packets_since_stream_change.saturating_add(1);
        // Committing to a new stream discards what the previous one left half-assembled. Dropping a
        // partial access unit is the correct trade: the alternative is emitting one spliced from two
        // encoders' fragments, which decodes to garbage rather than to nothing.
        if self.depacketizer_ssrc == Some(header.ssrc) {
            // The stream in possession is still speaking, so whatever run the retired one had built
            // is not a resumption.
            self.retired_ssrc_run = 0;
        } else {
            // A straggler from the stream we just left is not a commitment to it -- see
            // `retired_ssrcs`. Its own access unit was discarded when we switched, so there is
            // nothing it can complete; it is counted as received and otherwise ignored.
            //
            // Past the grace it still is not a commitment on its own: reclaiming on one late packet
            // would make the peer's ACTUAL stream the retired one and freeze its video for a whole
            // new window. Only an uninterrupted run reclaims, which a resumed stream produces in a
            // few packets and a lone latecomer never does.
            //
            // Asked of EVERY stream this one has left, not only the last: with two older streams a
            // straggler from the older one met neither guard and took reassembly on its own. A
            // never-seen SSRC is not a straggler but a genuine stream change, and still commits at
            // once -- that is how streams change at all.
            if self.retired_ssrcs.contains(&header.ssrc) {
                if self.contender_ssrc != Some(header.ssrc) {
                    // A different old stream: it starts its own run rather than inheriting one.
                    self.contender_ssrc = Some(header.ssrc);
                    self.retired_ssrc_run = 0;
                }
                self.retired_ssrc_run = self.retired_ssrc_run.saturating_add(1);
                if self.packets_since_stream_change <= RETIRED_SSRC_GRACE_PACKETS
                    || self.retired_ssrc_run < RETIRED_SSRC_RESUME_PACKETS
                {
                    return Some((header, Vec::new()));
                }
            }
            if self.depacketizer_ssrc.is_some() {
                self.depacketizer.reset();
            }
            if let Some(left) = self.depacketizer_ssrc {
                self.retired_ssrcs.retain(|ssrc| *ssrc != left);
                if self.retired_ssrcs.len() == RETIRED_SSRC_MEMORY {
                    self.retired_ssrcs.remove(0);
                }
                self.retired_ssrcs.push(left);
            }
            // The stream taking possession is no longer retired, whatever it was before.
            self.retired_ssrcs.retain(|ssrc| *ssrc != header.ssrc);
            self.depacketizer_ssrc = Some(header.ssrc);
            self.packets_since_stream_change = 0;
            self.retired_ssrc_run = 0;
            self.contender_ssrc = None;
        }
        let first = self.depacketizer.push(
            header.sequence_number,
            header.timestamp,
            &payload,
            header.marker,
        );
        let mut completed = Vec::with_capacity(if first.is_some() { 2 } else { 0 });
        if let Some(au) = first {
            completed.push(au);
        }
        while let Some(au) = self.depacketizer.pop_ready() {
            completed.push(au);
        }
        Some((header, completed))
    }
}

#[cfg(test)]
mod replay_stream_tests {
    use super::*;

    // A sequence number only means something inside one stream. With a shared window two
    // interleaved SSRCs each look to the other like a huge jump, and roughly half the packets are
    // rejected as replays -- silently, and only for a peer that happens to use two SSRCs.
    #[test]
    fn interleaved_ssrcs_do_not_reject_each_other() {
        let mut streams = SrtpRecvStreams::default();
        for seq in 0..64u64 {
            for ssrc in [0xAAAA_0001u32, 0xBBBB_0002] {
                let (_, replay) = streams.commit_mut(ssrc).expect("within the cap");
                assert!(
                    replay.accept(seq),
                    "ssrc {ssrc:#x} seq {seq} must be accepted on its own timeline"
                );
            }
        }
    }

    #[test]
    fn a_replay_within_one_stream_is_still_rejected() {
        let mut streams = SrtpRecvStreams::default();
        let (_, replay) = streams.commit_mut(1).expect("first stream");
        assert!(replay.accept(10));
        assert!(!replay.accept(10), "a repeat is a replay");
        assert!(replay.accept(11));
    }

    // Dropping past the cap rather than evicting: eviction would reset a rollover counter a live
    // stream is still using, turning a bounded resource into a correctness bug.
    #[test]
    fn the_stream_table_is_bounded_and_refuses_rather_than_evicting() {
        let mut streams = SrtpRecvStreams::default();
        for ssrc in 0..SRTP_REPLAY_STREAM_CAP as u32 {
            assert!(streams.commit_mut(ssrc).is_some());
        }
        assert!(
            streams.commit_mut(9999).is_none(),
            "past the cap the packet is dropped"
        );
        assert!(
            streams.commit_mut(0).is_some(),
            "an established stream keeps its state"
        );
    }

    // Each stream keeps its own rollover counter, so one stream wrapping cannot move another's.
    #[test]
    fn each_stream_keeps_its_own_rollover_counter() {
        let mut streams = SrtpRecvStreams::default();
        let (roc_a, _) = streams.commit_mut(1).expect("stream a");
        roc_a.commit_roc(0, 0xffff);
        let (roc_a, _) = streams.commit_mut(1).expect("stream a again");
        assert_eq!(roc_a.estimate_roc(0x0001), 1, "stream a wrapped");
        let (roc_b, _) = streams.commit_mut(2).expect("stream b");
        assert_eq!(
            roc_b.estimate_roc(0x0001),
            0,
            "stream b must not inherit another stream's wrap"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voip::e2e_srtp::SRTCP_AUTH_TAG_LEN;
    use crate::voip::warp::WARP_MI_TAG_LEN;
    use wacore_binary::Server;

    fn peer() -> Jid {
        Jid::new("222222222222222", Server::Lid)
    }
    fn creator() -> Jid {
        Jid::new("111111111111111", Server::Lid).with_device(1)
    }

    #[test]
    fn outgoing_lifecycle() {
        let mut s = CallSession::new_outgoing("CID", peer(), creator());
        assert_eq!(s.phase(), CallPhase::Idle);
        assert!(s.transition_to(CallPhase::Calling));
        assert!(s.transition_to(CallPhase::Ringing));
        assert!(s.transition_to(CallPhase::Connecting));
        assert!(s.transition_to(CallPhase::Active));
        assert!(s.is_active());
        // Illegal jump is rejected.
        assert!(!s.transition_to(CallPhase::Calling));
        assert!(s.transition_to(CallPhase::Ended));
        assert!(s.is_ended());
        // Nothing advances after Ended.
        assert!(!s.transition_to(CallPhase::Active));
    }

    #[test]
    fn incoming_starts_ringing_and_cannot_call() {
        let mut s = CallSession::new_incoming("CID", peer(), creator());
        assert_eq!(s.phase(), CallPhase::Ringing);
        // Incoming can't go to Calling.
        assert!(!s.transition_to(CallPhase::Calling));
        assert!(s.transition_to(CallPhase::Connecting));
        assert!(s.transition_to(CallPhase::Active));
    }

    #[test]
    fn call_link_waiting_room_advances_to_connecting_but_not_back() {
        let mut session = CallSession::new_outgoing("LINK", peer(), creator());
        assert!(session.transition_to(CallPhase::Calling));
        assert!(session.transition_to(CallPhase::WaitingRoom));
        assert!(session.transition_to(CallPhase::Connecting));
        assert!(!session.transition_to(CallPhase::WaitingRoom));
    }

    #[test]
    fn forward_progress_may_skip_phases_but_not_go_backward() {
        // An accepted outgoing call commonly skips Ringing (Calling -> Connecting) and an immediate
        // accept can reach Active directly. Both are forward progress and must be allowed.
        let mut s = CallSession::new_outgoing("CID", peer(), creator());
        assert!(s.transition_to(CallPhase::Calling));
        assert!(
            s.transition_to(CallPhase::Connecting),
            "Calling->Connecting (ringing skipped) must be allowed"
        );
        assert!(s.transition_to(CallPhase::Active));

        let mut s2 = CallSession::new_outgoing("CID2", peer(), creator());
        assert!(s2.transition_to(CallPhase::Calling));
        assert!(
            s2.transition_to(CallPhase::Active),
            "Calling->Active (immediate accept) must be allowed"
        );

        // Idle still leaves only to Calling (outgoing), and backward moves stay rejected.
        let mut s3 = CallSession::new_outgoing("CID3", peer(), creator());
        assert!(
            !s3.transition_to(CallPhase::Connecting),
            "Idle cannot skip straight to Connecting"
        );
        assert!(s3.transition_to(CallPhase::Calling));
        assert!(s3.transition_to(CallPhase::Active));
        assert!(
            !s3.transition_to(CallPhase::Connecting),
            "no backward Active->Connecting"
        );
        assert!(
            !s3.transition_to(CallPhase::Ringing),
            "no backward Active->Ringing"
        );
    }

    #[test]
    fn media_pipeline_round_trips_composition() {
        // Same LID both directions so the loopback exercises header+crypt+tag stitching. This
        // cannot catch a send/recv direction inversion (the scheme is symmetric between two
        // equally-configured peers); `protect_uses_self_lid_for_send` guards that.
        let call_key: Vec<u8> = (0u8..32).collect();
        let lid = "222222222222222:0@lid";
        let params = MediaPipelineParams {
            call_key: &call_key,
            self_lid: lid,
            peer_lid: lid,
            ssrc: 0x12345678,
            samples_per_packet: 960,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        };
        let mut tx = MediaPipeline::new(&params).unwrap();
        let mut rx = MediaPipeline::new(&params).unwrap();

        let opus = vec![0x48u8, 0x11, 0x22, 0x33, 0x44, 0x55];
        let packet = tx.protect_audio(&opus);
        // First packet: seq=1 gives roc=0.
        let (header, payload) = rx.unprotect_audio(&packet).unwrap();
        assert_eq!(header.sequence_number, 1);
        assert_eq!(header.ssrc, 0x12345678);
        assert_eq!(header.payload_type, 120);
        assert_eq!(payload, opus);
    }

    #[test]
    fn protect_uses_self_lid_for_send() {
        // The outbound keystream must be keyed by our *self* LID (the sender's id) so a real
        // WhatsApp peer, which derives its recv keys from our LID, can decrypt us. An inversion
        // back to the peer LID would re-key this body and break interop (was the garbled-audio /
        // reconnect bug). Round-trip tests can't see this; pinning the ciphertext can.
        let call_key: Vec<u8> = (0u8..32).collect();
        let self_lid = "111111111111111:0@lid";
        let peer_lid = "222222222222222:0@lid";
        let ssrc = 0x12345678u32;
        let mut pipe = MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid,
            peer_lid,
            ssrc,
            samples_per_packet: 960,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap();
        let opus = vec![0x10u8, 0x21, 0x32, 0x43];
        let packet = pipe.protect_audio(&opus);

        let without_tag = &packet[..packet.len() - WARP_MI_TAG_LEN];
        let header_len = rtp_header_byte_length(without_tag).unwrap();
        let body = &without_tag[header_len..];
        // First packet is seq=1, roc=0.
        let expect = crypt_payload(
            &derive_e2e_keys(&call_key, self_lid).unwrap(),
            ssrc,
            1,
            0,
            &opus,
        );
        assert_eq!(
            body,
            expect.as_slice(),
            "send must encrypt under the self LID"
        );
        // And NOT under the peer LID (the inverted form).
        let inverted = crypt_payload(
            &derive_e2e_keys(&call_key, peer_lid).unwrap(),
            ssrc,
            1,
            0,
            &opus,
        );
        assert_ne!(body, inverted.as_slice());
    }

    #[test]
    fn recv_uses_peer_lid_for_recv() {
        // The recv keystream must be keyed by the PEER's LID: a real peer encrypts under its own
        // (self) LID, which is our peer LID. A round-trip test can't catch a recv-direction key
        // inversion because the scheme is symmetric; this pins the direction.
        let call_key: Vec<u8> = (0u8..32).collect();
        let self_lid = "111111111111111:0@lid";
        let peer_lid = "222222222222222:0@lid";
        let ssrc = 0x12345678u32;

        // Our recv pipe (keys self=self_lid / peer=peer_lid).
        let mut us = MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid,
            peer_lid,
            ssrc,
            samples_per_packet: 960,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap();
        // The peer's send pipe is OUR mirror: its self LID is our peer LID.
        let mut peer_tx = MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid: peer_lid,
            peer_lid: self_lid,
            ssrc,
            samples_per_packet: 960,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap();

        let opus = vec![0x48u8, 0x01, 0x02, 0x03, 0x04, 0x05];
        let from_peer = peer_tx.protect_audio(&opus);
        let (_, recovered) = us
            .unprotect_audio(&from_peer)
            .expect("peer packet must decrypt under our recv (peer-LID) keys");
        assert_eq!(recovered, opus, "recv must use the peer-LID keystream");

        // A packet a mis-keyed peer would send under OUR self LID must NOT recover: that proves the
        // recv side is not silently keyed by the self LID.
        let mut self_keyed_tx = MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid,
            peer_lid,
            ssrc,
            samples_per_packet: 960,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap();
        let wrong = self_keyed_tx.protect_audio(&opus);
        let mut us2 = MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid,
            peer_lid,
            ssrc,
            samples_per_packet: 960,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap();
        assert!(
            us2.unprotect_audio(&wrong).is_none(),
            "recv must reject a self-LID-keyed packet (its MI tag fails to authenticate)"
        );
    }

    // The recv keystream is HKDF'd over the FULL peer LID INCLUDING the device suffix, so the caller
    // must key its recv path to the device that ANSWERS, not the base/dialed LID. A multi-device callee
    // whose companion (e.g. `:2`) answers sends under `derive(self=...:2)`; if the caller still keys
    // recv from the base `...:0` (the dialed LID), every inbound frame decrypts to garbage. This is the
    // rust↔rust "choppy audio" root cause (an Android phone answers from `:0`, so it never tripped).
    #[test]
    fn recv_keys_must_match_the_answering_device_lid() {
        let call_key: Vec<u8> = (0u8..32).collect();
        let caller = "111111111111111:0@lid";
        let callee_base = "222222222222222:0@lid";
        let callee_answering = "222222222222222:2@lid"; // the companion that actually answered
        let ssrc = 0x12345678;
        let opus = vec![0x50u8, 0x11, 0x22, 0x33, 0x44, 0x55];

        // The answering companion's send pipe: self = its OWN device LID `:2`.
        let mut answerer_tx = MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid: callee_answering,
            peer_lid: caller,
            ssrc,
            samples_per_packet: 960,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap();
        let from_answerer = answerer_tx.protect_audio(&opus);

        // Wrong keying: caller keys recv from the dialed BASE LID `:0`. The frame's
        // MI tag (keyed by the answering device) fails to authenticate, so it's rejected.
        let mut caller_base = MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid: caller,
            peer_lid: callee_base,
            ssrc,
            samples_per_packet: 960,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap();
        assert!(
            caller_base.unprotect_audio(&from_answerer).is_none(),
            "base-LID recv keys must reject a companion-device-keyed frame"
        );

        // FIX: caller keys recv from the ANSWERING device LID `:2` -> recovers cleanly.
        let mut caller_fixed = MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid: caller,
            peer_lid: callee_answering,
            ssrc,
            samples_per_packet: 960,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap();
        let (_, recovered) = caller_fixed.unprotect_audio(&from_answerer).unwrap();
        assert_eq!(
            recovered, opus,
            "recv keys derived from the answering device LID must recover the frame"
        );
    }

    // The fix: `rekey_recv` switches the caller's recv keys from the base LID to the answering device,
    // so a companion's frames that decrypted to garbage start decrypting cleanly. Also asserts send keys
    // are untouched (the round-trip the OTHER way still works after a rekey).
    #[test]
    fn rekey_recv_recovers_companion_keyed_frame() {
        let call_key: Vec<u8> = (0u8..32).collect();
        let caller = "111111111111111:0@lid";
        let callee_base = "222222222222222:0@lid";
        let callee_answering = "222222222222222:2@lid";
        let ssrc = 0x12345678;
        let opus = vec![0x50u8, 0x11, 0x22, 0x33, 0x44, 0x55];

        // The companion that answers keys its send by its OWN device LID.
        let mut answerer_tx = MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid: callee_answering,
            peer_lid: caller,
            ssrc,
            samples_per_packet: 960,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap();

        // Caller starts keyed to the dialed BASE LID (the bug state): the companion
        // frame's MI tag doesn't authenticate under the base-LID recv keys -> rejected.
        let mut caller_pipe = MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid: caller,
            peer_lid: callee_base,
            ssrc,
            samples_per_packet: 960,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap();
        let frame1 = answerer_tx.protect_audio(&opus);
        assert!(
            caller_pipe.unprotect_audio(&frame1).is_none(),
            "pre-rekey: a companion-keyed frame is rejected"
        );

        // Rekey to the answering device; a subsequent frame now recovers cleanly.
        assert!(caller_pipe.rekey_recv(&call_key, callee_answering));
        let frame2 = answerer_tx.protect_audio(&opus);
        let (_, recovered) = caller_pipe.unprotect_audio(&frame2).unwrap();
        assert_eq!(
            recovered, opus,
            "post-rekey: the companion's frames decrypt"
        );

        // Send keys were not touched: our outbound still round-trips to a peer keyed on us.
        let mut peer_rx = MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid: callee_answering,
            peer_lid: caller,
            ssrc,
            samples_per_packet: 960,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap();
        let ours = caller_pipe.protect_audio(&opus);
        let (_, got) = peer_rx.unprotect_audio(&ours).unwrap();
        assert_eq!(got, opus, "rekey_recv must not disturb send keys");
    }

    // An unauthenticated packet (bad WARP MI tag — an on-path relay can't forge it
    // without the SRTP auth key) is rejected and must NOT fold the recv rollover
    // counter, so a following legit frame still decrypts (RFC 3711 §3.3.1).
    #[test]
    fn forged_packet_is_rejected_and_does_not_desync_roc() {
        let call_key: Vec<u8> = (0u8..32).collect();
        let a = "111111111111111:0@lid";
        let b = "222222222222222:0@lid";
        let ssrc = 0x0BADF00D;
        let opus = vec![0x50u8, 1, 2, 3, 4, 5, 6, 7];

        let params = |self_lid, peer_lid| MediaPipelineParams {
            call_key: &call_key,
            self_lid,
            peer_lid,
            ssrc,
            samples_per_packet: 960,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        };
        let mut tx = MediaPipeline::new(&params(a, b)).unwrap();
        let mut rx = MediaPipeline::new(&params(b, a)).unwrap();

        // Legit frame seeds the recv tracker and decrypts cleanly.
        let f0 = tx.protect_audio(&opus);
        assert_eq!(rx.unprotect_audio(&f0).unwrap().1, opus);
        let base_seq = u16::from_be_bytes([f0[2], f0[3]]);

        // Forge a far-AHEAD packet: rewrite the RTP sequence field (bytes 2..4) to a
        // forward jump the pre-fix guess_roc would have folded into s_l. The rewrite
        // also invalidates the MI tag, so authentication rejects the packet.
        let mut forged = tx.protect_audio(&opus);
        forged[2..4].copy_from_slice(&base_seq.wrapping_add(0x4000).to_be_bytes());
        assert!(
            rx.unprotect_audio(&forged).is_none(),
            "an unauthenticated far-ahead packet must be rejected, not fold the ROC"
        );

        // The rejected packet left the recv tracker untouched, so a subsequent legit
        // frame still decrypts. (The exact roc-bump staircase is pinned by
        // e2e_srtp::unauthenticated_staircase_cannot_advance_roc_without_commit.)
        let f2 = tx.protect_audio(&opus);
        assert_eq!(
            rx.unprotect_audio(&f2).unwrap().1,
            opus,
            "recv keystream survives an injected forged packet"
        );
    }

    // A relay-advertised non-4 WARP MI tag length must round-trip: the tag the sender appends and the
    // bytes the receiver strips must agree, or all inbound media silently fails to decode. Threads a
    // configurable length through both pipelines and proves a payload survives at 6 and at the default 4.
    #[test]
    fn rejects_out_of_range_warp_mi_tag_len() {
        let call_key: Vec<u8> = (0u8..32).collect();
        let lid = "222222222222222:0@lid";
        let params = |tag_len| MediaPipelineParams {
            call_key: &call_key,
            self_lid: lid,
            peer_lid: lid,
            ssrc: 0x12345678,
            samples_per_packet: 960,
            warp_mi_tag_len: tag_len,
        };
        // The tag is a prefix of a 20-byte HMAC-SHA1 digest, so a longer one cannot exist and a
        // zero-length one would authenticate every packet. Rejecting both here is what keeps
        // `append_warp_mi_tag_in_place`'s clamp unreachable from a built pipeline.
        assert!(MediaPipeline::new(&params(21)).is_none());
        assert!(MediaPipeline::new(&params(0)).is_none());
        assert!(MediaPipeline::new(&params(WARP_MI_TAG_LEN)).is_some());
        assert!(MediaPipeline::new(&params(20)).is_some());

        // The video pipeline shares the tag helpers, so it has to reject the same range --
        // otherwise the audio guard alone would leave the send-side clamp reachable.
        let video = |tag_len| VideoPipelineParams {
            warp_mi_tag_len: tag_len,
            ..video_params(&call_key, lid, lid)
        };
        assert!(VideoPipeline::new(&video(21)).is_none());
        assert!(VideoPipeline::new(&video(0)).is_none());
        assert!(VideoPipeline::new(&video(WARP_MI_TAG_LEN)).is_some());
        assert!(VideoPipeline::new(&video(20)).is_some());
    }

    #[test]
    fn non_default_warp_mi_tag_len_round_trips() {
        let call_key: Vec<u8> = (0u8..32).collect();
        let lid = "222222222222222:0@lid";
        let opus = vec![0x48u8, 0x11, 0x22, 0x33, 0x44, 0x55];

        for tag_len in [WARP_MI_TAG_LEN, 6] {
            let params = MediaPipelineParams {
                call_key: &call_key,
                self_lid: lid,
                peer_lid: lid,
                ssrc: 0x12345678,
                samples_per_packet: 960,
                warp_mi_tag_len: tag_len,
            };
            let mut tx = MediaPipeline::new(&params).unwrap();
            let mut rx = MediaPipeline::new(&params).unwrap();
            let packet = tx.protect_audio(&opus);
            let (_, payload) = rx
                .unprotect_audio(&packet)
                .unwrap_or_else(|| panic!("tag_len {tag_len} must round-trip"));
            assert_eq!(payload, opus, "tag_len {tag_len} payload must survive");
        }

        // A mismatched recv tag length strips the wrong byte count and corrupts the payload, which is
        // exactly the failure this config plumbing prevents.
        let base = MediaPipelineParams {
            call_key: &call_key,
            self_lid: lid,
            peer_lid: lid,
            ssrc: 0x12345678,
            samples_per_packet: 960,
            warp_mi_tag_len: 6,
        };
        let mut tx = MediaPipeline::new(&base).unwrap();
        let mut rx = MediaPipeline::new(&MediaPipelineParams {
            warp_mi_tag_len: 4,
            ..base
        })
        .unwrap();
        let packet = tx.protect_audio(&opus);
        let mismatched = rx.unprotect_audio(&packet).map(|(_, p)| p);
        assert_ne!(
            mismatched.as_deref(),
            Some(opus.as_slice()),
            "a recv/send tag-length mismatch must NOT recover the payload"
        );
    }

    #[test]
    fn srtcp_recv_rekeys_to_the_answering_device() {
        use crate::voip::rtcp::build_compact_rtcp_208;

        let call_key: Vec<u8> = (0u8..32).collect();
        let caller = "111111111111111:0@lid";
        let callee_base = "222222222222222:0@lid";
        let callee_answering = "222222222222222:2@lid";
        let params = MediaPipelineParams {
            call_key: &call_key,
            self_lid: caller,
            peer_lid: callee_base,
            ssrc: 0x0102_0304,
            samples_per_packet: 960,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        };
        let mut caller_rx = MediaPipeline::new(&params).unwrap();
        let peer_ssrc = 0x1122_3344;
        let plain = build_compact_rtcp_208(peer_ssrc, params.ssrc);
        let peer_keys = derive_srtcp_keys(&call_key, callee_answering).unwrap();
        let protected = protect_srtcp(&peer_keys, peer_ssrc, 0, &plain);

        assert!(caller_rx.unprotect_rtcp(&protected).is_none());
        assert!(caller_rx.rekey_recv(&call_key, callee_answering));
        assert_eq!(
            caller_rx.unprotect_rtcp(&protected).as_deref(),
            Some(plain.as_slice())
        );
    }

    #[test]
    fn group_epoch_rotates_rtp_and_srtcp_without_resetting_stream_counters() {
        let old_epoch = [0x11; 32];
        let new_epoch = [0x22; 32];
        let alice = "100001:1@lid";
        let bob = "200002:2@lid";
        let ssrc = 0x1234_5678;
        let params = |key, self_lid, peer_lid| MediaPipelineParams {
            call_key: key,
            self_lid,
            peer_lid,
            ssrc,
            samples_per_packet: 960,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        };
        let mut tx = MediaPipeline::new(&params(&old_epoch, alice, bob)).unwrap();
        let mut rx = MediaPipeline::new(&params(&old_epoch, bob, alice)).unwrap();
        let mut old_rx = MediaPipeline::new(&params(&old_epoch, bob, alice)).unwrap();
        let payload = [0x50, 1, 2, 3, 4, 5, 6, 7];

        let before = tx.protect_audio(&payload);
        let before_header = parse_rtp_header(&before).unwrap();
        assert_eq!(rx.unprotect_audio(&before).unwrap().1, payload);
        assert!(
            rx.unprotect_audio(&before).is_none(),
            "an authenticated RTP packet must be delivered only once"
        );
        let first_rtcp = tx.audio_sender_report(1_000, None);
        assert!(rx.unprotect_rtcp(&first_rtcp).is_some());
        let first_index = u32::from_be_bytes(
            first_rtcp
                [first_rtcp.len() - SRTCP_AUTH_TAG_LEN - 4..first_rtcp.len() - SRTCP_AUTH_TAG_LEN]
                .try_into()
                .unwrap(),
        ) & 0x7fff_ffff;

        assert!(tx.rekey_send_from_raw(&new_epoch, alice));
        assert!(rx.rekey_recv_from_raw_preserving_roc(&new_epoch, alice));
        let mut reset_sender = MediaPipeline::new(&params(&new_epoch, alice, bob)).unwrap();
        assert!(
            rx.unprotect_audio(&reset_sender.protect_audio(&[0x51; 8]))
                .is_none(),
            "an ordinary epoch rekey must preserve the authenticated RTP replay window"
        );
        let after = tx.protect_audio(&payload);
        let after_header = parse_rtp_header(&after).unwrap();
        assert_eq!(
            after_header.sequence_number,
            before_header.sequence_number.wrapping_add(1)
        );
        assert_eq!(
            after_header.timestamp,
            before_header.timestamp.wrapping_add(960)
        );
        assert_eq!(rx.unprotect_audio(&after).unwrap().1, payload);
        assert!(old_rx.unprotect_audio(&after).is_none());

        let second_rtcp = tx.audio_sender_report(2_000, None);
        assert!(rx.unprotect_rtcp(&second_rtcp).is_some());
        let second_index = u32::from_be_bytes(
            second_rtcp[second_rtcp.len() - SRTCP_AUTH_TAG_LEN - 4
                ..second_rtcp.len() - SRTCP_AUTH_TAG_LEN]
                .try_into()
                .unwrap(),
        ) & 0x7fff_ffff;
        assert_eq!(second_index, first_index.wrapping_add(1));
    }

    #[test]
    fn srtcp_replay_window_handles_reorder_and_index_wrap() {
        let mut window = SrtcpReplayWindow::default();
        assert!(window.accept(100));
        assert!(window.accept(102));
        assert!(window.accept(101));
        assert!(
            !window.accept(101),
            "a reordered packet is accepted only once"
        );
        assert!(window.accept(40), "the oldest in-window packet is accepted");
        assert!(
            !window.accept(38),
            "packets outside the 64-index window are stale"
        );

        let mut wrapping = SrtcpReplayWindow::default();
        assert!(wrapping.accept(0x7fff_fffe));
        assert!(wrapping.accept(0x7fff_ffff));
        assert!(wrapping.accept(0));
        assert!(wrapping.accept(1));
        assert!(!wrapping.accept(0x7fff_ffff));
    }

    #[test]
    fn srtcp_replay_is_rejected_only_after_authentication() {
        use crate::voip::rtcp::build_compact_rtcp_208;

        let call_key: Vec<u8> = (0u8..32).collect();
        let caller = "111111111111111:0@lid";
        let peer = "222222222222222:0@lid";
        let params = MediaPipelineParams {
            call_key: &call_key,
            self_lid: caller,
            peer_lid: peer,
            ssrc: 0x0102_0304,
            samples_per_packet: 960,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        };
        let mut receiver = MediaPipeline::new(&params).unwrap();
        let peer_keys = derive_srtcp_keys(&call_key, peer).unwrap();
        let sender = 0x1122_3344;
        let plain = build_compact_rtcp_208(sender, params.ssrc);
        let packet = |index| protect_srtcp(&peer_keys, sender, index, &plain);

        assert_eq!(
            receiver.unprotect_rtcp(&packet(5)).as_deref(),
            Some(&plain[..])
        );
        assert!(receiver.unprotect_rtcp(&packet(5)).is_none());

        let mut forged = packet(6);
        *forged.last_mut().unwrap() ^= 1;
        assert!(receiver.unprotect_rtcp(&forged).is_none());
        assert_eq!(
            receiver.unprotect_rtcp(&packet(6)).as_deref(),
            Some(&plain[..]),
            "a forged packet must not consume the authenticated index"
        );

        assert_eq!(
            receiver.unprotect_rtcp(&packet(8)).as_deref(),
            Some(&plain[..])
        );
        assert_eq!(
            receiver.unprotect_rtcp(&packet(7)).as_deref(),
            Some(&plain[..])
        );
        assert!(receiver.unprotect_rtcp(&packet(7)).is_none());
        assert_eq!(
            receiver.unprotect_rtcp(&packet(80)).as_deref(),
            Some(&plain[..])
        );
        assert!(receiver.unprotect_rtcp(&packet(10)).is_none());

        let other_sender = 0x5566_7788;
        let other_plain = build_compact_rtcp_208(other_sender, params.ssrc);
        let other = protect_srtcp(&peer_keys, other_sender, 5, &other_plain);
        assert_eq!(
            receiver.unprotect_rtcp(&other).as_deref(),
            Some(&other_plain[..]),
            "replay state is independent per sender SSRC"
        );
    }

    fn video_params<'a>(
        call_key: &'a [u8],
        self_lid: &'a str,
        peer_lid: &'a str,
    ) -> VideoPipelineParams<'a> {
        VideoPipelineParams {
            call_key,
            self_lid,
            peer_lid,
            ssrc: 0x0055_AA33,
            ts_stride: crate::voip::rtp::VIDEO_TS_STRIDE_15FPS,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        }
    }

    /// A synthetic Annex-B AU big enough to force FU-A fragmentation.
    fn video_au(nal_len: usize) -> Vec<u8> {
        let mut au = vec![0, 0, 0, 1, 0x65];
        au.extend((0..nal_len).map(|i| (i % 251) as u8));
        au
    }

    #[test]
    fn video_pipeline_round_trips_multi_packet_au() {
        let call_key: Vec<u8> = (0u8..32).collect();
        let a = "111111111111111:0@lid";
        let b = "222222222222222:0@lid";
        let mut tx = VideoPipeline::new(&video_params(&call_key, a, b)).unwrap();
        let mut rx = VideoPipeline::new(&video_params(&call_key, b, a)).unwrap();

        let au = video_au(3000);
        let packets = tx.protect_video(&au);
        assert!(packets.len() >= 4, "3KB AU must fragment into FU-A packets");
        let mut got = None;
        for (i, p) in packets.iter().enumerate() {
            let out = rx.unprotect_video(p);
            if i < packets.len() - 1 {
                assert!(out.is_none(), "AU must only complete on the marker packet");
            } else {
                got = out;
            }
        }
        assert_eq!(got, Some(vec![au]), "AU must reassemble byte-identical");
        assert!(
            rx.unprotect_video(packets.last().unwrap()).is_none(),
            "replaying the marker packet must not redeliver the completed access unit"
        );

        // Second AU keeps flowing (sequencer + ROC state stay consistent).
        let au2 = video_au(100);
        let packets2 = tx.protect_video(&au2);
        assert_eq!(packets2.len(), 1);
        assert_eq!(rx.unprotect_video(&packets2[0]), Some(vec![au2]));
    }

    // The receive table authenticates a renumbered stream on its own rollover counter and replay
    // window, but reassembly is keyed on the RTP timestamp, which restarts with the stream. Without
    // committing the depacketizer to one SSRC at a time, the replacement stream's first frames read
    // as reordered packets of the old one and the video freezes for as long as the peer keeps its
    // new numbering -- which is forever.
    #[test]
    fn video_renumbering_peer_is_reassembled_on_its_own_timeline() {
        let call_key: Vec<u8> = (0u8..32).collect();
        let a = "111111111111111:0@lid";
        let b = "222222222222222:0@lid";
        let mut tx = VideoPipeline::new(&video_params(&call_key, a, b)).unwrap();
        let mut renumbered = {
            let mut params = video_params(&call_key, a, b);
            params.ssrc ^= 0x0F0F_0F0F;
            VideoPipeline::new(&params).unwrap()
        };
        let mut rx = VideoPipeline::new(&video_params(&call_key, b, a)).unwrap();

        // Three AUs on the original stream carry its clock well past zero.
        for _ in 0..3 {
            let au = video_au(60);
            let packet = tx.protect_video(&au).pop().expect("one packet");
            assert_eq!(rx.unprotect_video(&packet), Some(vec![au]));
        }
        // The replacement stream starts its own clock at zero.
        let au = video_au(60);
        let packet = renumbered.protect_video(&au).pop().expect("one packet");
        assert_eq!(
            rx.unprotect_video(&packet),
            Some(vec![au]),
            "a renumbered stream's first AU must not read as a reordered packet of the old one"
        );
    }

    // A renumbering is not instantaneous: packets from the old SSRC keep arriving while the new
    // stream is already sending. Each straggler used to look like another stream commitment, taking
    // the depacketizer back and discarding the fragments the new stream had assembled -- so the
    // frames spanning the overlap were lost, on a stream whose packets all authenticated.
    #[test]
    fn a_straggler_from_the_retired_stream_does_not_discard_the_new_one() {
        let call_key: Vec<u8> = (0u8..32).collect();
        let a = "111111111111111:0@lid";
        let b = "222222222222222:0@lid";
        let mut old = VideoPipeline::new(&video_params(&call_key, a, b)).unwrap();
        let mut new = {
            let mut params = video_params(&call_key, a, b);
            params.ssrc ^= 0x0F0F_0F0F;
            VideoPipeline::new(&params).unwrap()
        };
        let mut rx = VideoPipeline::new(&video_params(&call_key, b, a)).unwrap();

        // The old stream is established, and has one AU still in flight on the wire.
        let established = video_au(60);
        let packet = old.protect_video(&established).pop().expect("one packet");
        assert_eq!(rx.unprotect_video(&packet), Some(vec![established]));
        let straggler = old
            .protect_video(&video_au(60))
            .pop()
            .expect("the packet still in flight when the peer renumbers");

        // The new stream sends an AU large enough to span several packets.
        let au = video_au(4_000);
        let fragments = new.protect_video(&au);
        assert!(
            fragments.len() > 2,
            "the AU must span packets for the overlap to be observable"
        );

        // Its first fragments arrive, then the straggler, then the rest.
        for fragment in &fragments[..fragments.len() - 1] {
            assert_eq!(rx.unprotect_video(fragment), None, "still assembling");
        }
        assert_eq!(
            rx.unprotect_video(&straggler),
            None,
            "the straggler completes nothing: its own AU went with the stream it belonged to"
        );
        assert_eq!(
            rx.unprotect_video(fragments.last().expect("marker packet")),
            Some(vec![au]),
            "the new stream's access unit must survive the overlap intact"
        );
    }

    // The grace has to end even when nothing but the retired stream arrives. A peer that sends one
    // packet on a new SSRC and then goes back to the old one delivers only ignored packets, so a
    // window counted in packets of the REPLACEMENT stream would never expire and that peer's video
    // would be frozen for the rest of the call -- the permanent failure the bound exists to avoid.
    #[test]
    fn a_stream_that_resumes_past_the_grace_reclaims_reassembly() {
        let call_key: Vec<u8> = (0u8..32).collect();
        let a = "111111111111111:0@lid";
        let b = "222222222222222:0@lid";
        let mut original = VideoPipeline::new(&video_params(&call_key, a, b)).unwrap();
        let mut replacement = {
            let mut params = video_params(&call_key, a, b);
            params.ssrc ^= 0x0F0F_0F0F;
            VideoPipeline::new(&params).unwrap()
        };
        let mut rx = VideoPipeline::new(&video_params(&call_key, b, a)).unwrap();

        let au = video_au(60);
        let packet = original.protect_video(&au).pop().expect("one packet");
        assert_eq!(rx.unprotect_video(&packet), Some(vec![au]));

        // One packet on the replacement SSRC, then the peer goes back to the original stream.
        let au = video_au(60);
        let packet = replacement.protect_video(&au).pop().expect("one packet");
        assert_eq!(rx.unprotect_video(&packet), Some(vec![au]));

        let mut delivered = 0;
        for _ in 0..(RETIRED_SSRC_GRACE_PACKETS + 4) {
            let au = video_au(60);
            let packet = original.protect_video(&au).pop().expect("one packet");
            if rx.unprotect_video(&packet) == Some(vec![au]) {
                delivered += 1;
            }
        }
        assert!(
            delivered >= 3,
            "the resumed stream must take reassembly back rather than stay ignored forever,              got {delivered} of {} delivered",
            RETIRED_SSRC_GRACE_PACKETS + 4
        );
    }

    // Expiring the grace must not turn ONE very late packet into a commitment to its stream. It
    // would make the peer's actual stream the retired one, and that stream is then ignored for a
    // whole fresh window -- a freeze caused by exactly the straggler the grace exists to absorb,
    // arrived at from a third side.
    #[test]
    fn a_lone_straggler_past_the_grace_does_not_take_reassembly_from_the_live_stream() {
        let call_key: Vec<u8> = (0u8..32).collect();
        let a = "111111111111111:0@lid";
        let b = "222222222222222:0@lid";
        let mut original = VideoPipeline::new(&video_params(&call_key, a, b)).unwrap();
        let mut replacement = {
            let mut params = video_params(&call_key, a, b);
            params.ssrc ^= 0x0F0F_0F0F;
            VideoPipeline::new(&params).unwrap()
        };
        let mut rx = VideoPipeline::new(&video_params(&call_key, b, a)).unwrap();

        let au = video_au(60);
        let packet = original.protect_video(&au).pop().expect("one packet");
        assert_eq!(rx.unprotect_video(&packet), Some(vec![au]));

        // The peer renumbers and stays there, well past the grace.
        for _ in 0..(RETIRED_SSRC_GRACE_PACKETS + 4) {
            let au = video_au(60);
            let packet = replacement.protect_video(&au).pop().expect("one packet");
            let _ = rx.unprotect_video(&packet);
        }

        // One very late packet from the retired stream, then the live stream continues.
        let au = video_au(60);
        let straggler = original.protect_video(&au).pop().expect("one packet");
        let _ = rx.unprotect_video(&straggler);

        let mut delivered = 0;
        for _ in 0..8 {
            let au = video_au(60);
            let packet = replacement.protect_video(&au).pop().expect("one packet");
            if rx.unprotect_video(&packet) == Some(vec![au]) {
                delivered += 1;
            }
        }
        assert_eq!(
            delivered, 8,
            "the live stream must keep reassembly; one latecomer is not a resumption"
        );
    }

    // A local downgrade and resume drops possession, which is what the run being built was counted
    // against. Carried across the pause, two stragglers from a stream the peer had already
    // renumbered away from -- one either side of the reset -- completed a single reclaim between
    // them, and the resumed plane went to the stream nobody is sending.
    //
    // The live stream takes it back on its next packet, because the wrongly seated straggler
    // retired nothing on the way in, so this is one discarded access unit rather than a freeze.
    // The window matters anyway: a keyframe request made inside it names the wrong stream, which
    // is the one thing this feature exists to get right.
    #[test]
    fn a_resumed_plane_does_not_complete_a_reclaim_begun_before_it() {
        let call_key: Vec<u8> = (0u8..32).collect();
        let a = "111111111111111:0@lid";
        let b = "222222222222222:0@lid";
        let mut original = VideoPipeline::new(&video_params(&call_key, a, b)).unwrap();
        let mut replacement = {
            let mut params = video_params(&call_key, a, b);
            params.ssrc ^= 0x0F0F_0F0F;
            VideoPipeline::new(&params).unwrap()
        };
        let mut rx = VideoPipeline::new(&video_params(&call_key, b, a)).unwrap();

        let au = video_au(60);
        let packet = original.protect_video(&au).pop().expect("one packet");
        assert_eq!(rx.unprotect_video(&packet), Some(vec![au]));

        // The peer renumbers and stays there, well past the grace.
        for _ in 0..(RETIRED_SSRC_GRACE_PACKETS + 4) {
            let au = video_au(60);
            let packet = replacement.protect_video(&au).pop().expect("one packet");
            let _ = rx.unprotect_video(&packet);
        }

        // Part of a run from the retired stream, one short of reclaiming.
        for _ in 0..(RETIRED_SSRC_RESUME_PACKETS - 1) {
            let au = video_au(60);
            let straggler = original.protect_video(&au).pop().expect("one packet");
            let _ = rx.unprotect_video(&straggler);
        }

        rx.reset_reassembly();

        // One more straggler must not finish what the pause interrupted: seating it would deliver
        // its access unit, which is what taking the plane looks like from here.
        let au = video_au(60);
        let straggler = original.protect_video(&au).pop().expect("one packet");
        assert_eq!(
            rx.unprotect_video(&straggler),
            None,
            "a straggler must not take the resumed plane on a run built before the reset"
        );

        let mut delivered = 0;
        for _ in 0..8 {
            let au = video_au(60);
            let packet = replacement.protect_video(&au).pop().expect("one packet");
            if rx.unprotect_video(&packet) == Some(vec![au]) {
                delivered += 1;
            }
        }
        assert_eq!(
            delivered, 8,
            "the stream the peer is actually sending must take the resumed plane"
        );
    }

    // The guard asked only about the MOST RECENTLY retired SSRC, so a peer that renumbered twice had
    // an older stream that met neither the grace nor the run: one straggler from it took reassembly
    // outright, and the live stream then froze for a whole fresh grace window.
    #[test]
    fn a_straggler_from_an_older_stream_does_not_take_reassembly_either() {
        let call_key: Vec<u8> = (0u8..32).collect();
        let a = "111111111111111:0@lid";
        let b = "222222222222222:0@lid";
        let mut first = VideoPipeline::new(&video_params(&call_key, a, b)).unwrap();
        let mut second = {
            let mut params = video_params(&call_key, a, b);
            params.ssrc ^= 0x0F0F_0F0F;
            VideoPipeline::new(&params).unwrap()
        };
        let mut third = {
            let mut params = video_params(&call_key, a, b);
            params.ssrc ^= 0x00FF_00FF;
            VideoPipeline::new(&params).unwrap()
        };
        let mut rx = VideoPipeline::new(&video_params(&call_key, b, a)).unwrap();

        let au = video_au(60);
        let packet = first.protect_video(&au).pop().expect("one packet");
        assert_eq!(rx.unprotect_video(&packet), Some(vec![au]));

        // Renumber once, then again, so `first` is now two streams back.
        for _ in 0..4 {
            let au = video_au(60);
            let packet = second.protect_video(&au).pop().expect("one packet");
            let _ = rx.unprotect_video(&packet);
        }
        for _ in 0..(RETIRED_SSRC_GRACE_PACKETS + 4) {
            let au = video_au(60);
            let packet = third.protect_video(&au).pop().expect("one packet");
            let _ = rx.unprotect_video(&packet);
        }

        // One very late packet from the OLDEST stream.
        let au = video_au(60);
        let straggler = first.protect_video(&au).pop().expect("one packet");
        let _ = rx.unprotect_video(&straggler);

        let mut delivered = 0;
        for _ in 0..8 {
            let au = video_au(60);
            let packet = third.protect_video(&au).pop().expect("one packet");
            if rx.unprotect_video(&packet) == Some(vec![au]) {
                delivered += 1;
            }
        }
        assert_eq!(
            delivered, 8,
            "an older stream's latecomer is no more a resumption than the last one's"
        );
    }

    #[test]
    fn video_pipeline_rejects_oversized_au_before_packetization() {
        let call_key: Vec<u8> = (0u8..32).collect();
        let mut pipe = VideoPipeline::new(&video_params(
            &call_key,
            "111111111111111:0@lid",
            "222222222222222:0@lid",
        ))
        .unwrap();
        let oversized = vec![0u8; H264_MAX_AU_BYTES + 1];
        assert!(pipe.protect_video(&oversized).is_empty());

        let packet = pipe.protect_video(&video_au(10)).pop().unwrap();
        assert_eq!(parse_rtp_header(&packet).unwrap().sequence_number, 0);
    }

    #[test]
    fn video_pipeline_keeps_parameter_sets_first_on_the_wire() {
        let call_key: Vec<u8> = (0u8..32).collect();
        let a = "111111111111111:0@lid";
        let b = "222222222222222:0@lid";
        let mut tx = VideoPipeline::new(&video_params(&call_key, a, b)).unwrap();
        let mut rx = VideoPipeline::new(&video_params(&call_key, b, a)).unwrap();
        let au = [
            &[0, 0, 0, 1, 0x69, 0xf0][..],
            &[0, 0, 0, 1, 0x67, 0x42, 0x00, 0x1f][..],
            &[0, 0, 0, 1, 0x68, 0xce, 0x06, 0xe2][..],
            &[0, 0, 0, 1, 0x65, 1, 2, 3][..],
        ]
        .concat();

        let packets = tx.protect_video(&au);
        let mut received = None;
        for packet in &packets {
            if let Some(frame) = rx.unprotect_video(packet) {
                received = Some(frame);
            }
        }

        let mut received = received.expect("marker packet completes the access unit");
        assert_eq!(received.len(), 1);
        let received = received.remove(0);
        assert_eq!(
            crate::voip::h264::split_annexb(&received)
                .map(crate::voip::h264::nal_unit_type)
                .collect::<Vec<_>>(),
            [7, 8, 5]
        );
    }

    #[test]
    fn video_protect_uses_self_lid_and_video_headers() {
        let call_key: Vec<u8> = (0u8..32).collect();
        let self_lid = "111111111111111:0@lid";
        let peer_lid = "222222222222222:0@lid";
        let mut pipe = VideoPipeline::new(&video_params(&call_key, self_lid, peer_lid)).unwrap();
        let au = video_au(10);
        let packets = pipe.protect_video(&au);
        assert_eq!(packets.len(), 1);
        let packet = &packets[0];
        let without_tag = &packet[..packet.len() - WARP_MI_TAG_LEN];
        let header = parse_rtp_header(without_tag).unwrap();
        assert_eq!(header.payload_type, crate::voip::rtp::RTP_PAYLOAD_TYPE_H264);
        assert!(header.marker, "single-packet AU carries the marker");
        assert_eq!(header.sequence_number, 0, "video seq starts at 0");
        assert_eq!(
            header.video_extension.unwrap().media_frame_info,
            VIDEO_MEDIA_FRAME_INFO_IDR,
            "an IDR AU carries WhatsApp's keyframe and IDR bits"
        );

        // Pin the send keystream to the SELF lid (same inversion guard as audio).
        let header_len = rtp_header_byte_length(without_tag).unwrap();
        let body = &without_tag[header_len..];
        let nal = &au[4..];
        let expect = crypt_payload(
            &derive_e2e_keys(&call_key, self_lid).unwrap(),
            header.ssrc,
            0,
            0,
            nal,
        );
        assert_eq!(body, expect.as_slice(), "video send must key on self LID");
    }

    #[test]
    fn video_frame_info_is_constant_across_every_au_fragment() {
        let call_key: Vec<u8> = (0u8..32).collect();
        let mut pipe = VideoPipeline::new(&video_params(
            &call_key,
            "111111111111111:0@lid",
            "222222222222222:0@lid",
        ))
        .unwrap();

        let idr = video_au(3_000);
        let idr_packets = pipe.protect_video(&idr);
        assert!(idr_packets.len() > 1);
        assert!(idr_packets.iter().all(|packet| {
            parse_rtp_header(packet)
                .and_then(|header| header.video_extension)
                .is_some_and(|extension| extension.media_frame_info == VIDEO_MEDIA_FRAME_INFO_IDR)
        }));

        let mut delta = vec![0, 0, 0, 1, 0x41];
        delta.extend((0..3_000).map(|i| (i % 251) as u8));
        let delta_packets = pipe.protect_video(&delta);
        assert!(delta_packets.len() > 1);
        assert!(delta_packets.iter().all(|packet| {
            parse_rtp_header(packet)
                .and_then(|header| header.video_extension)
                .is_some_and(|extension| extension.media_frame_info == VIDEO_MEDIA_FRAME_INFO_DELTA)
        }));
    }

    #[test]
    fn video_forged_tag_rejected_and_stream_survives() {
        let call_key: Vec<u8> = (0u8..32).collect();
        let a = "111111111111111:0@lid";
        let b = "222222222222222:0@lid";
        let mut tx = VideoPipeline::new(&video_params(&call_key, a, b)).unwrap();
        let mut rx = VideoPipeline::new(&video_params(&call_key, b, a)).unwrap();

        let au = video_au(50);
        let packets = tx.protect_video(&au);
        let mut forged = packets[0].clone();
        let seq = u16::from_be_bytes([forged[2], forged[3]]);
        forged[2..4].copy_from_slice(&seq.wrapping_add(0x4000).to_be_bytes());
        assert!(
            rx.unprotect_video(&forged).is_none(),
            "tampered video packet must fail authentication"
        );
        assert_eq!(
            rx.unprotect_video(&packets[0]),
            Some(vec![au]),
            "legit packet still decrypts after the forgery"
        );
        // Garbage never panics.
        assert!(rx.unprotect_video(&[]).is_none());
        assert!(rx.unprotect_video(&[0xff; 9]).is_none());
    }

    #[test]
    fn video_rekey_recv_switches_to_answering_device() {
        let call_key: Vec<u8> = (0u8..32).collect();
        let caller = "111111111111111:0@lid";
        let callee_base = "222222222222222:0@lid";
        let callee_answering = "222222222222222:2@lid";

        let mut answerer_tx =
            VideoPipeline::new(&video_params(&call_key, callee_answering, caller)).unwrap();
        let mut caller_rx =
            VideoPipeline::new(&video_params(&call_key, caller, callee_base)).unwrap();

        let au = video_au(60);
        let f1 = answerer_tx.protect_video(&au);
        assert!(
            caller_rx.unprotect_video(&f1[0]).is_none(),
            "base-LID keys must reject the companion's video"
        );
        assert!(caller_rx.rekey_recv(&call_key, callee_answering));
        let f2 = answerer_tx.protect_video(&au);
        assert_eq!(caller_rx.unprotect_video(&f2[0]), Some(vec![au]));
        // Malformed key refuses without clobbering state.
        assert!(!caller_rx.rekey_recv(&[0u8; 4], callee_answering));
    }

    #[test]
    fn video_pipeline_rejects_bad_setup() {
        let call_key: Vec<u8> = (0u8..32).collect();
        let lid = "222222222222222:0@lid";
        let mut p = video_params(&call_key, lid, lid);
        p.warp_mi_tag_len = 0;
        assert!(VideoPipeline::new(&p).is_none());
        p.warp_mi_tag_len = 21;
        assert!(VideoPipeline::new(&p).is_none());
        let mut zero_stride = video_params(&call_key, lid, lid);
        zero_stride.ts_stride = 0;
        assert!(
            VideoPipeline::new(&zero_stride).is_none(),
            "a zero timestamp stride must be rejected"
        );
        let mut short = video_params(&[0u8; 8], lid, lid);
        short.warp_mi_tag_len = WARP_MI_TAG_LEN;
        assert!(
            VideoPipeline::new(&short).is_none(),
            "short callKey must be rejected"
        );
        // Empty AU produces no packets rather than a marker-only ghost.
        let mut ok = VideoPipeline::new(&video_params(&call_key, lid, lid)).unwrap();
        assert!(ok.protect_video(&[]).is_empty());
    }

    #[test]
    fn timed_video_input_preserves_a_capture_gap_in_rtp() {
        let call_key: Vec<u8> = (0u8..32).collect();
        let lid = "222222222222222:0@lid";
        let stride = 6_000;
        let mut pipe = VideoPipeline::new(&VideoPipelineParams {
            call_key: &call_key,
            self_lid: lid,
            peer_lid: lid,
            ssrc: 0x1234_5678,
            ts_stride: stride,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap();
        let au = [0, 0, 0, 1, 0x65, 1, 2, 3];
        let first = parse_rtp_header(&pipe.protect_video(&au).pop().unwrap()).unwrap();
        assert_eq!(first.timestamp, 0);

        // The source captured one AU at the missing timestamp. Supplying the next capture clock
        // value keeps the RTP timeline at 3 * stride even though only two AUs reach this pipeline.
        pipe.set_video_timestamp(stride * 3);
        let after_gap = parse_rtp_header(&pipe.protect_video(&au).pop().unwrap()).unwrap();
        assert_eq!(after_gap.timestamp, stride * 3);
    }

    // The esp32 control/crypto plane. An embedded consumer with no UDP, no codec, and no audio
    // drives exactly this much of the call stack over its main WebSocket connection: the signaling
    // state machine plus E2E-SRTP key derivation. It never constructs the media engine and never
    // runs MLow. This pins that surface as pure sync logic (no runtime, no FFI), which is all the
    // esp32-S3 can do today; running the codec there is out of scope.
    #[test]
    fn esp32_control_plane_signaling_and_crypto_without_media() {
        // Signaling: drive an incoming call through its lifecycle.
        let peer = Jid::new("222222222222222", Server::Lid);
        let mut call = CallSession::new_incoming("CID", peer.clone(), peer);
        assert_eq!(call.phase(), CallPhase::Ringing);
        assert!(call.transition_to(CallPhase::Connecting));
        assert!(call.transition_to(CallPhase::Active));
        assert!(call.transition_to(CallPhase::Ended));

        // Crypto: derive the E2E-SRTP keys from the callKey. This is HKDF only (no codec, no FFI),
        // so it is viable on the esp32; building the pipeline does not encode or decode any audio.
        let call_key: Vec<u8> = (0u8..32).collect();
        let pipeline = MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid: "111@lid",
            peer_lid: "222@lid",
            ssrc: 0x1234,
            samples_per_packet: 960,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        });
        assert!(
            pipeline.is_some(),
            "key derivation must succeed on the control plane"
        );
    }
}

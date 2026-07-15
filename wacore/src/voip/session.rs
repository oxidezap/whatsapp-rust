//! Call state machine and the media pipeline composition (Opus payload to RTP WARP header to
//! E2E SRTP protect, and the reverse). The byte-level crypto/framing lives in the sibling
//! `wacore::voip` modules; this stitches it together. Pure logic: no socket, clock, or runtime.

use super::e2e_srtp::{
    E2eSrtpKeys, RecvRocTracker, RocTracker, append_warp_mi_tag, crypt_payload, derive_e2e_keys,
    derive_srtcp_keys, protect_srtcp, unprotect_srtcp, verify_warp_mi_tag,
};
use super::h264::{H264_MAX_AU_BYTES, H264Depacketizer, au_has_idr, packetize_au};
use super::rtcp::{
    RtcpReceptionReport, RtcpSenderStats, WHATSAPP_RTCP_CNAME_LEN, build_whatsapp_rtcp_cname,
    build_whatsapp_sender_report_with_sdes, build_whatsapp_source_description,
    parse_rtcp_sender_ssrc,
};
use super::rtp::{
    RTP_FIXED_HEADER_LEN, RtpHeader, RtpStream, VIDEO_MEDIA_FRAME_INFO_DELTA,
    VIDEO_MEDIA_FRAME_INFO_IDR, VideoRtpStream, encode_rtp_header_into, parse_rtp_header,
    rtp_header_byte_length,
};
use super::ssrc::format_e2e_srtp_participant_id;
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
            ring_devices: Vec::new(),
            answering_device: None,
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
            ring_devices: Vec::new(),
            answering_device: None,
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
    /// The lifecycle order is `Idle → Calling → Ringing → Connecting → Active`. Forward progress is
    /// allowed and MAY skip intermediate phases: an accepted outgoing call commonly goes
    /// `Calling → Connecting` with no observed `Ringing`, and an immediate accept can reach `Active`
    /// directly. Backward moves are rejected. `Idle` leaves only to `Calling` (outgoing) or `Ended`.
    /// `Ended` is a sink reachable from any live phase (`Ended → Ended` is a no-op `false`).
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

/// Lifecycle ordinal for the forward-progress check in [`CallSession::transition_to`] (higher =
/// later in the call). `Ended` is handled separately, so its rank is never compared.
fn phase_rank(p: CallPhase) -> u8 {
    match p {
        CallPhase::Idle => 0,
        CallPhase::Calling => 1,
        CallPhase::Ringing => 2,
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

    fn protect(&mut self, ssrc: u32, plain: &[u8]) -> Vec<u8> {
        let out = protect_srtcp(&self.keys, ssrc, self.index, plain);
        self.index = self.index.wrapping_add(1);
        out
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
    recv_roc: RecvRocTracker,
    srtcp: SrtcpSender,
    recv_srtcp_keys: E2eSrtpKeys,
    recv_srtcp_replay: SrtcpReplayState,
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
            recv_roc: RecvRocTracker::default(),
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
        self.recv_roc = RecvRocTracker::default();
        self.recv_srtcp_replay = SrtcpReplayState::default();
        true
    }

    /// Outbound: wrap an Opus payload in an RTP WARP header, E2E-SRTP encrypt, append the WARP MI tag.
    pub fn protect_audio(&mut self, opus_payload: &[u8]) -> Vec<u8> {
        let header = self.rtp.next_packet(opus_payload, false);
        let roc = self.send_roc.advance(header.sequence_number);
        let encrypted = crypt_payload(
            &self.send_keys,
            header.ssrc,
            header.sequence_number,
            roc,
            opus_payload,
        );
        // One buffer sized for header + ciphertext: the header writes straight
        // into it (no throwaway Vec), and the exact capacity avoids the growth
        // realloc the extend would otherwise trigger.
        let mut packet = Vec::with_capacity(header.byte_size() + encrypted.len());
        encode_rtp_header_into(&header, &mut packet);
        packet.extend_from_slice(&encrypted);
        self.srtcp.record(1, opus_payload.len());
        append_warp_mi_tag(&self.send_keys.auth_key, &packet, roc, self.warp_mi_tag_len)
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
            &mut self.recv_roc,
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

/// Shared inbound SRTP step for the audio and video pipelines: verify the WARP
/// MI tag against the *estimated* ROC BEFORE committing it, then decrypt. The
/// order is load-bearing (RFC 3711 §3.3.1): an on-path relay must not be able
/// to fold unauthenticated packets into the rollover counter and permanently
/// desync the receiver.
fn unprotect_srtp_packet(
    recv_keys: &E2eSrtpKeys,
    recv_roc: &mut RecvRocTracker,
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
    let roc = recv_roc.estimate_roc(header.sequence_number);
    if !verify_warp_mi_tag(
        &recv_keys.auth_key,
        without_tag,
        roc,
        warp_mi_tag_len,
        received_tag,
    ) {
        return None;
    }
    // Authenticated: now it's safe to advance the rollover counter.
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
    recv_roc: RecvRocTracker,
    depacketizer: H264Depacketizer,
    pkt_scratch: Vec<Vec<u8>>,
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
            recv_roc: RecvRocTracker::default(),
            depacketizer: H264Depacketizer::default(),
            pkt_scratch: Vec::new(),
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

    pub(crate) fn set_timestamp_stride(&mut self, ts_stride: u32) -> bool {
        self.rtp.set_timestamp_stride(ts_stride)
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
        self.recv_roc = RecvRocTracker::default();
        self.depacketizer.reset();
        true
    }

    /// Outbound: packetize one Annex-B access unit and protect each RTP packet.
    pub fn protect_video(&mut self, au: &[u8]) -> Vec<Vec<u8>> {
        if au.len() > H264_MAX_AU_BYTES {
            return Vec::new();
        }
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
            let encrypted = crypt_payload(
                &self.send_keys,
                header.ssrc,
                header.sequence_number,
                roc,
                payload,
            );
            let mut packet = Vec::with_capacity(header.byte_size() + encrypted.len());
            encode_rtp_header_into(&header, &mut packet);
            packet.extend_from_slice(&encrypted);
            self.srtcp.record(1, payload.len());
            packets.push(append_warp_mi_tag(
                &self.send_keys.auth_key,
                &packet,
                roc,
                self.warp_mi_tag_len,
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
            &mut self.recv_roc,
            self.warp_mi_tag_len,
            packet,
        )?;
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
mod tests {
    use super::*;
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
        // Above the 20-byte HMAC digest (or zero) would panic when slicing the tag, so reject it.
        assert!(MediaPipeline::new(&params(21)).is_none());
        assert!(MediaPipeline::new(&params(0)).is_none());
        assert!(MediaPipeline::new(&params(WARP_MI_TAG_LEN)).is_some());
        assert!(MediaPipeline::new(&params(20)).is_some());
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

        // Second AU keeps flowing (sequencer + ROC state stay consistent).
        let au2 = video_au(100);
        let packets2 = tx.protect_video(&au2);
        assert_eq!(packets2.len(), 1);
        assert_eq!(rx.unprotect_video(&packets2[0]), Some(vec![au2]));
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

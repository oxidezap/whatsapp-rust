//! The sans-IO call engine: a str0m-shaped state machine that owns the relay control plane (STUN
//! allocate, 1s keepalive, consent-freshness replies) and, optionally, the media plane (MLow
//! encode/decode + E2E-SRTP + SFrame + a 20ms playout jitter buffer). It owns no socket, no clock,
//! and no thread. The shell performs a single mutation (`handle_input`), drains `poll_output()`
//! until it yields `Output::Timeout`, executes each intent, and arms one timer for that deadline.
//!
//! Time is monotonic milliseconds supplied by the shell; the engine never reads a clock. The only
//! non-deterministic input is the STUN transaction id, injected via [`TxIdSource`], so the whole
//! engine is deterministically testable.
//!
//! This is the portable orchestration the example's `run_media` task did by hand, lifted into pure
//! logic so the Tokio driver, the WASM bridge, and (for the control plane) embedded consumers all
//! drive one implementation.

use std::collections::VecDeque;

use bytes::Bytes;

use super::demux::{RelayPacketKind, classify_relay_packet};
use super::h264::{VideoFrame, au_has_idr, au_is_keyframe};
use super::rtcp::{
    RTCP_PT_PSFB, RtcpFeedback, RtcpReportBlock, RtpReceptionStats, build_whatsapp_rtcp_cname,
    parse_sender_report_timing, summarize_rtcp,
};
use super::rtp::{
    RTP_PAYLOAD_TYPE_H264, VIDEO_CLOCK_RATE, VIDEO_TS_STRIDE_15FPS, parse_rtp_header,
};
use super::session::{
    CallDirection, MediaPipeline, MediaPipelineParams, VideoPipeline, VideoPipelineParams,
};
use super::sframe::{SframeIn, SframeSession};
use super::{is_standard_opus_frame, mlow, ssrc, stun};

/// Monotonic milliseconds. The shell supplies it; the engine never reads a clock.
pub type Millis = u64;

/// Sentinel deadline meaning "no timer pending"; the shell waits only on I/O until the next input.
pub const NEVER: Millis = u64::MAX;

/// Relay consent-freshness cadence: re-send the STUN allocate + a WA ping every second. The relay
/// drops the client after ~4s without traffic, which is what makes the peer reconnect/terminate.
const KEEPALIVE_MS: Millis = 1000;
/// RTCP Sender-Report cadence. WhatsApp's `voip_settings` advertises `rtcp_interval_ms=1500`.
const RTCP_MS: Millis = 1500;
/// Deadline for the relay to ack the allocate. Past this with no success the relay is wedged
/// (silently dropping the allocate), so surface a terminal timeout instead of keepaliving forever.
const ALLOCATE_TIMEOUT_MS: Millis = 10_000;
/// Playout drain cadence: hand the speaker a fixed slice every 20ms so it stays fed at 16kHz.
const PLAYOUT_MS: Millis = 20;
/// 20ms @ 16kHz: samples drained to the speaker per playout tick.
const PLAYOUT_DRAIN: usize = 320;
/// ~150ms latency ceiling; a burst past this resyncs (drops oldest) instead of lagging.
const PLAYOUT_CAP: usize = 2400;
/// Prebuffer target: prime playout until the jitter buffer holds two 60ms peer frames, so the
/// steady-state buffer never drains below one frame (a 60ms cushion that absorbs the relay's
/// inter-arrival jitter). Priming to a single frame is a zero cushion: that one frame drains away
/// over its own 60ms cycle, so the buffer returns to empty before the next packet and any late
/// arrival underruns. The cushion has to be one frame above what the per-cycle drain consumes.
const PLAYOUT_TARGET: usize = 1920;
/// Bound on how long playout primes before flushing a partial buffer: if the peer sends one frame
/// then goes DTX the jitter buffer never reaches `PLAYOUT_TARGET`, so after this many 20ms ticks
/// (~200ms) drain whatever is queued instead of holding it (silent) forever. Comfortably above the
/// few ticks a normal jittered second-frame arrival takes, so it never trips in steady operation.
const MAX_PRIME_TICKS: u32 = 10;
/// One-byte mlow DTX comfort-noise token sent on a muted (exact-zero) mic frame so the media stream
/// never gaps; protect_audio frames it with the DTX RTP header and the peer decodes it to silence.
const MLOW_DTX_CNG: [u8; 1] = [0x90];
/// One 60ms MLow frame at 16kHz. `Input::MicFrame` must carry exactly this; a wrong-length buffer is
/// dropped (the encoder requires it), never sent.
const MIC_FRAME_SAMPLES: usize = 960;
const AUDIO_CLOCK_RATE: u32 = 16_000;

/// Supplies STUN transaction ids. Injected so the core stays RNG-free and deterministically
/// testable. Production shells MUST back this with a real RNG (the ids gate consent freshness);
/// [`SequentialTxIds`] is for tests and deterministic drives only.
pub trait TxIdSource: crate::sync_marker::MaybeSendSync {
    fn next_tx_id(&mut self) -> [u8; 12];
}

/// Deterministic counter ids for tests / deterministic drives. NOT for production: predictable
/// transaction ids weaken consent freshness. Doc-hidden and kept off the `voip` facade so a
/// consumer never reaches for it; production shells default to an OS-RNG `TxIdSource` (`RandTxIds`).
#[doc(hidden)]
#[derive(Default)]
pub struct SequentialTxIds(u64);

impl SequentialTxIds {
    pub fn new() -> Self {
        Self::default()
    }
}

impl TxIdSource for SequentialTxIds {
    fn next_tx_id(&mut self) -> [u8; 12] {
        self.0 = self.0.wrapping_add(1);
        let mut id = [0u8; 12];
        id[..8].copy_from_slice(&self.0.to_be_bytes());
        id
    }
}

/// Everything the engine needs to be self-contained for one call. The relay fields come from the
/// parsed `<relay>` stanza; the crypto fields from the decrypted callKey and our/our-peer LIDs.
/// Build it via [`for_incoming`](Self::for_incoming) / [`for_outgoing`](Self::for_outgoing), which
/// validate the relay block.
#[derive(Clone)]
pub struct CallConfig {
    pub call_id: String,
    pub direction: CallDirection,
    /// Our own participant LID (the E2E-SRTP send keys are derived from this).
    pub self_lid: String,
    /// The peer's participant LID (the E2E-SRTP recv keys are derived from this).
    pub peer_lid: String,
    /// The 32-byte callKey.
    pub call_key: Vec<u8>,
    pub ssrc: u32,
    /// RTP timestamp stride per packet. NOTE: the MLow encoder requires exactly 960-sample (60ms @
    /// 16kHz) frames, so `Input::MicFrame` must carry 960 samples regardless of this value (a
    /// wrong-length frame is dropped, no RTP sent). Set to 960 unless the codec changes.
    pub samples_per_packet: u32,
    /// Relay endpoint allocate inputs.
    pub relay_token: Vec<u8>,
    pub relay_ip: String,
    pub relay_port: u16,
    /// The relay `<key>` (ASCII) used as the STUN MESSAGE-INTEGRITY key.
    pub integrity_key: Vec<u8>,
    /// The relay `<warp_mi_tag_len>` (default 4); a non-4 length must not desync the WARP MI tag.
    pub warp_mi_tag_len: usize,
    /// Run the media plane (MLow + playout). Off for the esp32 control plane.
    pub enable_media: bool,
    /// Build the video plane at engine construction (a video-from-the-start call). An audio call
    /// upgrades later via [`CallEngine::enable_video`]; both paths build the same pipeline.
    pub enable_video: bool,
    /// Decrypt inbound SFrame, with a plaintext fallback (the Android peer may GCM-wrap its
    /// Opus/MLow). Recv-side only by design: outbound stays plain codec inside WAHKDF SRTP, which
    /// the peer accepts, matching the pre-refactor pipeline (`MediaPipeline`: "SFrame is omitted,
    /// default-off on send"). Send-side SFrame is intentionally not wired.
    pub enable_sframe: bool,
}

// Manual Debug so a stray `{:?}` can't leak the SRTP callKey, the STUN integrity key, or the relay
// token (all live call credentials), matching the redaction the sibling key structs already apply
// (E2eSrtpKeys, SrtpKeyingMaterial).
impl core::fmt::Debug for CallConfig {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CallConfig")
            .field("call_id", &self.call_id)
            .field("direction", &self.direction)
            .field("self_lid", &self.self_lid)
            .field("peer_lid", &self.peer_lid)
            .field("call_key", &"[redacted]")
            .field("ssrc", &self.ssrc)
            .field("samples_per_packet", &self.samples_per_packet)
            .field("relay_token", &"[redacted]")
            .field("relay_ip", &self.relay_ip)
            .field("relay_port", &self.relay_port)
            .field("integrity_key", &"[redacted]")
            .field("warp_mi_tag_len", &self.warp_mi_tag_len)
            .field("enable_media", &self.enable_media)
            .field("enable_video", &self.enable_video)
            .field("enable_sframe", &self.enable_sframe)
            .finish()
    }
}

/// Why the engine could not be constructed.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum EngineError {
    #[error("callKey too short for E2E keys (need 32 bytes)")]
    BadCallKey,
    #[error("relay endpoint is not a valid IPv4 address")]
    BadEndpoint,
}

/// Why an incoming call's [`CallConfig`] could not be assembled from the offer's relay block.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum SetupError {
    #[error("relay has no endpoints")]
    NoRelayEndpoint,
    #[error("relay endpoint has no IPv4 address")]
    NoRelayIpv4,
    #[error("relay has no token #{0}")]
    NoRelayToken(u32),
    #[error("relay has no <key> (STUN integrity key)")]
    NoIntegrityKey,
    /// The relay advertised a WARP MI tag length the SRTP layer can't honor (the tag is sliced from a
    /// 20-byte HMAC-SHA1 digest, so 1..=20 is the only valid range).
    #[error("relay advertised an unsupported WARP MI tag length: {0}")]
    BadWarpMiTagLen(usize),
}

impl CallConfig {
    /// Assemble the engine config from the callKey and the parsed `<relay>`. Pure: derives our
    /// participant SSRC (E2E HKDF over our self LID) and pulls the relay endpoint / token /
    /// integrity-key out of `relay`, so the whole media-config build is offline testable. The only
    /// thing that differs by direction is the `direction` field; everything else is identical.
    /// `enable_sframe` is on (the Android peer may GCM-wrap its codec; recv-decrypt only).
    fn from_relay(
        direction: CallDirection,
        call_id: &str,
        self_lid: &str,
        peer_lid: &str,
        call_key: Vec<u8>,
        relay: &super::relay_parse::RelayData,
    ) -> Result<Self, SetupError> {
        use super::{relay_parse, ssrc};

        let ep = relay_parse::get_media_relay_endpoint(relay).ok_or(SetupError::NoRelayEndpoint)?;
        let (relay_ip, relay_port) =
            relay_parse::get_primary_ipv4_address(ep).ok_or(SetupError::NoRelayIpv4)?;
        // A padded-empty slot (a sparse token block) is a missing token, not a zero-length one: reject
        // it here so the nothing-usable fallback surfaces a precise NoRelayToken instead of dialing the
        // relay with an empty token and failing at the allocate.
        let relay_token = relay
            .relay_tokens
            .get(ep.token_id as usize)
            .filter(|t| !t.is_empty())
            .cloned()
            .ok_or(SetupError::NoRelayToken(ep.token_id))?;
        // The relay <key> is the STUN MESSAGE-INTEGRITY key; without it the allocate/binding-success
        // we sign can't authenticate, so fail here rather than dial with an empty key. Sign with the
        // base64 TEXT of <key> (relay_key_ascii), NOT its decoded bytes (relay.relay_key): the relay
        // HMACs against the ASCII key material, so decoding first fails the allocate (verified against
        // the WhatsApp client; raw decoded bytes were the original bug).
        let integrity_key = relay
            .relay_key_ascii
            .clone()
            .ok_or(SetupError::NoIntegrityKey)?;

        let our_ssrc = ssrc::derive_wasm_participant_ssrc(
            call_id,
            &ssrc::format_e2e_srtp_participant_id(self_lid),
            0,
        );

        // Default to 4 when absent; reject an out-of-range relay value here (a distinct relay-protocol
        // error) rather than letting it collapse into BadCallKey when the SRTP layer rejects it.
        let warp_mi_tag_len = relay
            .warp_mi_tag_len
            .map(|n| n as usize)
            .unwrap_or(super::warp::WARP_MI_TAG_LEN);
        if !(1..=20).contains(&warp_mi_tag_len) {
            return Err(SetupError::BadWarpMiTagLen(warp_mi_tag_len));
        }

        Ok(CallConfig {
            call_id: call_id.to_string(),
            direction,
            self_lid: self_lid.to_string(),
            peer_lid: peer_lid.to_string(),
            call_key,
            ssrc: our_ssrc,
            // The MLow encoder requires exactly 960-sample frames.
            samples_per_packet: 960,
            relay_token,
            relay_ip,
            relay_port,
            integrity_key,
            warp_mi_tag_len,
            enable_media: true,
            enable_video: false,
            enable_sframe: true,
        })
    }

    /// Engine config for an INCOMING call: the callKey was decrypted from the peer's offer.
    pub fn for_incoming(
        call_id: &str,
        self_lid: &str,
        peer_lid: &str,
        call_key: Vec<u8>,
        relay: &super::relay_parse::RelayData,
    ) -> Result<Self, SetupError> {
        Self::from_relay(
            CallDirection::Incoming,
            call_id,
            self_lid,
            peer_lid,
            call_key,
            relay,
        )
    }

    /// Engine config for an OUTGOING call: the callKey is the one WE generated, and the relay block is
    /// the one the server hands back after the offer.
    pub fn for_outgoing(
        call_id: &str,
        self_lid: &str,
        peer_lid: &str,
        call_key: Vec<u8>,
        relay: &super::relay_parse::RelayData,
    ) -> Result<Self, SetupError> {
        Self::from_relay(
            CallDirection::Outgoing,
            call_id,
            self_lid,
            peer_lid,
            call_key,
            relay,
        )
    }
}

/// One input to the engine, applied with the current monotonic timestamp.
pub enum Input<'a> {
    /// A packet arrived on the relay channel (one DataChannel/datagram message).
    RelayPacket(&'a [u8]),
    /// A 60ms PCM frame captured from the local mic (16kHz mono). Must be exactly 960 samples (the
    /// MLow frame size); a wrong-length frame is silently dropped by the encoder (no RTP sent).
    MicFrame(&'a [i16]),
    /// One pre-encoded H.264 Annex-B access unit to send. Dropped while the video plane is off
    /// (audio-only call, or after a downgrade).
    VideoFrame(&'a [u8]),
    /// The deadline that `poll_output`/`poll_timeout` last reported has fired.
    Timeout,
}

/// One intent emitted by the engine, drained via `poll_output` until `Timeout`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum Output {
    /// Send these bytes over the relay channel.
    Transmit(Bytes),
    /// Decoded PCM for the speaker (16kHz mono).
    Playout(Vec<i16>),
    /// A reassembled peer access unit for the video sink. Dedicated output (not a CallEvent) for
    /// the same reason audio uses `Playout`: the event channel sheds on overflow, media must not.
    VideoPlayout(VideoFrame),
    /// A call lifecycle / diagnostic event.
    Event(CallEvent),
    /// Drained: arm a timer for this monotonic-ms deadline ([`NEVER`] = no timer).
    Timeout(Millis),
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CallEvent {
    /// The relay accepted our allocate (an allocate/binding success arrived); media path is live.
    RelayAllocated,
    /// An inbound audio payload the core won't decode itself (a standard-Opus frame, not MLow).
    /// The shell may decode it with a platform codec (libopus). Carries the decrypted payload.
    ForeignAudio(Bytes),
    /// The relay rejected our allocate. Terminal; carries the STUN error code (class*100 + number).
    RelayAllocateFailed(u16),
    /// The relay never acked the allocate within the deadline (wedged relay). Terminal.
    RelayAllocateTimedOut,
    /// The peer's `<video state=N>` signaling arrived (upgrade requested/accepted, stopped, ...).
    /// Pushed by the signaling handler, not the engine; surfaced here so one event stream carries
    /// the whole call. `UpgradeRequestV2` is the peer asking for video — answer with
    /// `accept_video` or ignore to decline.
    VideoStateChanged {
        state: crate::types::call::VideoState,
        orientation: Option<u8>,
    },
    /// Authenticated peer RTCP. A referenced local video SSRC proves the peer built a receiver for
    /// our outbound stream; RR, NACK, PLI and FIR are all represented here.
    RtcpReceived {
        packet_types: Vec<u8>,
        sender_ssrc: u32,
        referenced_ssrcs: Vec<u32>,
        reports_audio: bool,
        reports_video: bool,
        report_blocks: Vec<RtcpReportBlock>,
        feedback: Vec<RtcpFeedback>,
    },
    /// Relay-send backpressure discarded complete media units before transmission.
    OutboundMediaDropped {
        video_access_units: u32,
        packets: u32,
    },
}

/// The optional media plane: the SRTP pipeline, the MLow codec, an optional SFrame session, and
/// the playout jitter buffer. One `MediaPipeline` serves both directions: protect uses its send
/// keys/ROC/RTP state, unprotect its recv keys/ROC, and those fields are disjoint.
struct MediaState {
    pipe: MediaPipeline,
    audio_reception: RtpReceptionStats,
    /// Retained so the caller can re-derive the recv keys once the answering device is known (the
    /// callee's `<accept>` carries its device LID). See [`CallEngine::rekey_recv`].
    call_key: Vec<u8>,
    /// Retained for a mid-call [`CallEngine::enable_video`]: the video pipeline derives its own
    /// SSRC and send keys from these on demand.
    self_lid: String,
    /// The peer LID the recv keys are CURRENTLY derived from — starts as the dialed base LID and
    /// moves to the answering device on [`CallEngine::rekey_recv`]. A video plane enabled after
    /// that rekey must key its recv path from this, not the stale config LID.
    recv_peer_lid: String,
    warp_mi_tag_len: usize,
    video_ts_stride: u32,
    /// The video plane, present while video is enabled (from the start or via upgrade).
    video: Option<VideoPlaneState>,
    audio_rtcp_announced: bool,
    sframe: Option<SframeSession>,
    encoder: mlow::MlowEncoder,
    decoder: mlow::MlowDecoder,
    /// Reused per outbound frame to hold the i16->f32 conversion, so the encode hot path doesn't
    /// allocate a fresh Vec each frame.
    scratch: Vec<f32>,
    jitter: VecDeque<i16>,
    playout_deadline: Millis,
    /// Playout emits silence (without draining) while the jitter buffer fills to `PLAYOUT_TARGET`, so
    /// a late packet costs one re-prime instead of a silence gap every 20ms tick. Re-armed on underrun.
    priming: bool,
    /// Consecutive playout ticks spent priming; bounds the wait so a partial buffer (the peer sent one
    /// frame then went DTX) is flushed after `MAX_PRIME_TICKS` instead of being held silent forever.
    priming_ticks: u32,
}

/// The video half of the media plane. No jitter buffer or playout tick: an AU is handed to the
/// sink the moment its marker packet reassembles it (the consumer's decoder does its own pacing).
///
/// The pipeline is built once per call and OUTLIVES a downgrade: `active` gates transmit/decode
/// while the pipe (its SRTP send seq + ROC) is preserved. Rebuilding it on a re-upgrade would reset
/// the packet index to zero under the same key+SSRC and repeat the AES-CTR keystream (a two-time
/// pad), so a downgrade must not drop it.
struct VideoPlaneState {
    pipe: VideoPipeline,
    reception: RtpReceptionStats,
    active: bool,
    /// When set, inbound video still decodes but OUTBOUND AUs are dropped: the initiator of an
    /// upgrade holds its camera off the wire until the peer accepts (a `<video>` request the peer
    /// ignores must not leak our video). Cleared on the peer's UpgradeAccept/Enabled.
    send_gated: bool,
    /// PLI/FIR means dependent frames only prolong the peer's undecodable jitter-buffer state.
    keyframe_required: bool,
}

fn requests_keyframe(feedback: &[RtcpFeedback], video_ssrc: u32) -> bool {
    let target = video_ssrc.to_be_bytes();
    feedback.iter().any(|item| {
        if item.packet_type != RTCP_PT_PSFB {
            return false;
        }
        match item.fmt {
            1 => item.media_ssrc == video_ssrc,
            4 => {
                item.media_ssrc == video_ssrc
                    || item
                        .fci
                        .chunks_exact(8)
                        .any(|row| row.get(..4) == Some(target.as_slice()))
            }
            _ => false,
        }
    })
}

/// Build the video pipeline for `self_lid` sending / `recv_peer_lid` receiving. `None` on a
/// malformed callKey (a setup invariant the audio plane already validated).
fn make_video_plane(
    call_id: &str,
    call_key: &[u8],
    self_lid: &str,
    recv_peer_lid: &str,
    warp_mi_tag_len: usize,
    ts_stride: u32,
    rtcp_cname: [u8; super::rtcp::WHATSAPP_RTCP_CNAME_LEN],
) -> Option<VideoPlaneState> {
    let video_ssrc = ssrc::derive_video_participant_ssrc(
        call_id,
        &ssrc::format_e2e_srtp_participant_id(self_lid),
    );
    let pipe = VideoPipeline::new_with_rtcp_cname(
        &VideoPipelineParams {
            call_key,
            self_lid,
            peer_lid: recv_peer_lid,
            ssrc: video_ssrc,
            ts_stride,
            warp_mi_tag_len,
        },
        rtcp_cname,
    )?;
    Some(VideoPlaneState {
        pipe,
        reception: RtpReceptionStats::default(),
        active: true,
        send_gated: false,
        keyframe_required: true,
    })
}

/// The sans-IO call engine. See the module docs for the drive contract.
pub struct CallEngine {
    call_id: String,
    direction: CallDirection,
    // Control plane.
    relay_token: Vec<u8>,
    endpoint_xor: [u8; 6],
    integrity_key: Vec<u8>,
    allocate: Bytes,
    tx_ids: Box<dyn TxIdSource>,
    keepalive_deadline: Millis,
    /// Next RTCP Sender-Report tick (NEVER until the relay allocates).
    rtcp_deadline: Millis,
    /// Mapping injected at start so SR NTP timestamps use wall time while scheduling stays monotonic.
    rtcp_monotonic_origin: Millis,
    rtcp_wallclock_origin_ms: u64,
    /// Deadline by which the allocate must be acked; NEVER once it is (or after firing the timeout).
    allocate_deadline: Millis,
    allocated: bool,
    started: bool,
    /// A terminal relay-allocate failure was surfaced; the engine goes inert (no keepalive, no
    /// timer, no further transmits) so the driver tears the call down instead of keepaliving a
    /// dead relay forever.
    terminated: bool,
    /// Our SRTP participant id (LID normalized to `<user>:<device>@lid`), the HKDF input for every
    /// stream SSRC. Retained so the STUN allocate announces this call's live SSRCs.
    self_participant_id: String,
    // Media plane (None = control plane only, e.g. esp32).
    media: Option<MediaState>,
    /// Peer device orientation (0..3, ×90°) from the last `<video device_orientation>`; stamped on
    /// every reassembled inbound AU so the sink can rotate.
    peer_video_orientation: u8,
    outbox: VecDeque<Output>,
}

impl CallEngine {
    /// Build the engine. Derives the E2E-SRTP keys and the XOR relay endpoint up front so a
    /// malformed callKey or relay address fails here rather than mid-call. Does not touch the
    /// timestamp or the tx-id source; call [`start`](Self::start) once the relay channel is open.
    // Lifecycle span only. The LID and callKey fields are PII/secret, so the config is skipped and
    // only the non-sensitive call_id/direction/media-flag are recorded.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "wa.voip.engine_new",
            level = "debug",
            skip_all,
            fields(call_id = %config.call_id, dir = ?config.direction, media = config.enable_media),
            err(Debug)
        )
    )]
    pub fn new(config: CallConfig, mut tx_ids: Box<dyn TxIdSource>) -> Result<Self, EngineError> {
        let endpoint_xor = stun::encode_xor_relay_endpoint(&config.relay_ip, config.relay_port)
            .ok_or(EngineError::BadEndpoint)?;

        let media = if config.enable_media {
            let audio_rtcp_cname = build_whatsapp_rtcp_cname(&tx_ids.next_tx_id());
            let pipe = MediaPipeline::new_with_rtcp_cname(
                &MediaPipelineParams {
                    call_key: &config.call_key,
                    self_lid: &config.self_lid,
                    peer_lid: &config.peer_lid,
                    ssrc: config.ssrc,
                    samples_per_packet: config.samples_per_packet,
                    warp_mi_tag_len: config.warp_mi_tag_len,
                },
                audio_rtcp_cname,
            )
            .ok_or(EngineError::BadCallKey)?;
            let sframe = if config.enable_sframe {
                SframeSession::new(&config.call_key, &config.self_lid, &config.peer_lid)
            } else {
                None
            };
            let video = if config.enable_video {
                let video_rtcp_cname = build_whatsapp_rtcp_cname(&tx_ids.next_tx_id());
                Some(
                    make_video_plane(
                        &config.call_id,
                        &config.call_key,
                        &config.self_lid,
                        &config.peer_lid,
                        config.warp_mi_tag_len,
                        VIDEO_TS_STRIDE_15FPS,
                        video_rtcp_cname,
                    )
                    .ok_or(EngineError::BadCallKey)?,
                )
            } else {
                None
            };
            Some(MediaState {
                pipe,
                audio_reception: RtpReceptionStats::default(),
                call_key: config.call_key.clone(),
                self_lid: config.self_lid.clone(),
                recv_peer_lid: config.peer_lid.clone(),
                warp_mi_tag_len: config.warp_mi_tag_len,
                video_ts_stride: VIDEO_TS_STRIDE_15FPS,
                video,
                audio_rtcp_announced: false,
                sframe,
                encoder: mlow::MlowEncoder::new(),
                decoder: mlow::MlowDecoder::new(),
                scratch: Vec::with_capacity(config.samples_per_packet as usize),
                jitter: VecDeque::new(),
                playout_deadline: 0,
                priming: true,
                priming_ticks: 0,
            })
        } else {
            None
        };

        Ok(Self {
            call_id: config.call_id,
            direction: config.direction,
            relay_token: config.relay_token,
            endpoint_xor,
            integrity_key: config.integrity_key,
            allocate: Bytes::new(),
            tx_ids,
            keepalive_deadline: 0,
            rtcp_deadline: NEVER,
            rtcp_monotonic_origin: 0,
            rtcp_wallclock_origin_ms: 0,
            allocate_deadline: 0,
            allocated: false,
            started: false,
            terminated: false,
            self_participant_id: ssrc::format_e2e_srtp_participant_id(&config.self_lid),
            media,
            peer_video_orientation: 0,
            outbox: VecDeque::new(),
        })
    }

    pub fn call_id(&self) -> &str {
        &self.call_id
    }

    pub fn direction(&self) -> CallDirection {
        self.direction
    }

    /// Whether the relay has acknowledged our allocate.
    pub fn is_allocated(&self) -> bool {
        self.allocated
    }

    /// Whether a terminal relay-allocate failure has been surfaced. Once true the engine is inert
    /// (emits nothing further); the driver breaks its loop and tears the call down.
    pub fn is_terminated(&self) -> bool {
        self.terminated
    }

    /// Caller-side: rekey the recv path to the device that ANSWERED (its LID arrives in the callee's
    /// `<accept>`). The dialed base callee LID is wrong once a companion device answers — without this
    /// every inbound frame decrypts to garbage. No-op (`true`) for a control-only engine (no media).
    /// `false` means the stored call_key is malformed (a setup invariant), so the driver ends the call.
    pub fn rekey_recv(&mut self, answering_peer_lid: &str) -> bool {
        let Some(m) = self.media.as_mut() else {
            return true;
        };
        if !m.pipe.rekey_recv(&m.call_key, answering_peer_lid) {
            return false;
        }
        // The video recv keys derive from the same participant id and go stale together with the
        // audio ones; a video plane enabled after this rekey must also start from the new LID.
        m.recv_peer_lid = answering_peer_lid.to_string();
        if let Some(v) = m.video.as_mut() {
            return v.pipe.rekey_recv(&m.call_key, answering_peer_lid);
        }
        true
    }

    /// Whether the video plane is currently up (sending is possible, inbound PT-97 decodes).
    pub fn is_video_enabled(&self) -> bool {
        self.media
            .as_ref()
            .and_then(|m| m.video.as_ref())
            .is_some_and(|v| v.active)
    }

    /// Set the nominal RTP cadence for subsequent video access units. The source must pace access
    /// units at the same rate; changing this never resets the SRTP or RTP sequence state.
    pub fn set_video_timestamp_stride(&mut self, ts_stride: u32) -> bool {
        if ts_stride == 0 {
            return false;
        }
        let Some(media) = self.media.as_mut() else {
            return false;
        };
        media.video_ts_stride = ts_stride;
        if let Some(video) = media.video.as_mut() {
            video.pipe.set_timestamp_stride(ts_stride);
        }
        true
    }

    /// Bring the video plane up (from-start call, or an accepted upgrade) with OUTBOUND allowed.
    /// Idempotent; also ungates a previously send-gated plane. See [`enable_video_gated`].
    pub fn enable_video(&mut self) -> bool {
        self.enable_video_inner(false)
    }

    /// Bring the video plane up but hold OUTBOUND video off the wire (the initiator of an upgrade,
    /// before the peer accepts). Inbound still decodes. [`enable_video`] later ungates it.
    pub fn enable_video_gated(&mut self) -> bool {
        self.enable_video_inner(true)
    }

    /// `false` when there is no media plane (control-only engine) or the stored callKey is malformed.
    /// A plane built by an earlier upgrade is REACTIVATED, not rebuilt, so its SRTP send seq/ROC
    /// continue (rebuilding would repeat the keystream under the same key+SSRC).
    fn enable_video_inner(&mut self, send_gated: bool) -> bool {
        let Some(m) = self.media.as_mut() else {
            return false;
        };
        if let Some(v) = m.video.as_mut() {
            let needs_recovery = !v.active || (v.send_gated && !send_gated);
            v.active = true;
            v.send_gated = send_gated;
            v.keyframe_required |= needs_recovery;
            return true;
        }
        let rtcp_cname = build_whatsapp_rtcp_cname(&self.tx_ids.next_tx_id());
        let call_id = &self.call_id;
        match make_video_plane(
            call_id,
            &m.call_key,
            &m.self_lid,
            &m.recv_peer_lid,
            m.warp_mi_tag_len,
            m.video_ts_stride,
            rtcp_cname,
        ) {
            Some(mut v) => {
                v.send_gated = send_gated;
                m.video = Some(v);
                true
            }
            None => false,
        }
    }

    /// Deactivate the video plane (downgrade): outbound AUs drop, inbound PT-97 is ignored. The
    /// pipeline (and its SRTP send seq/ROC) is PRESERVED so a later re-upgrade continues the
    /// keystream instead of resetting the packet index. The audio plane is untouched. Idempotent.
    pub fn disable_video(&mut self) {
        if let Some(v) = self.media.as_mut().and_then(|m| m.video.as_mut()) {
            v.active = false;
        }
    }

    /// Record the peer's device orientation (0..3, ×90°) from a `<video>` stanza; stamped on every
    /// subsequently reassembled inbound AU.
    pub fn set_peer_video_orientation(&mut self, orientation: u8) {
        self.peer_video_orientation = orientation & 0x03;
    }

    /// Begin the media session with separate monotonic and Unix clocks. RTCP scheduling must not
    /// leak wall-clock jumps, while Sender Reports require a real NTP epoch. Idempotent.
    pub fn start(&mut self, now: Millis, wallclock_ms: u64) {
        if self.started {
            return;
        }
        self.started = true;
        self.rtcp_monotonic_origin = now;
        self.rtcp_wallclock_origin_ms = wallclock_ms;
        let tx = self.tx_ids.next_tx_id();
        // Built once here; the 1s keepalive re-sends it, so store it as Bytes and refcount-clone
        // rather than re-allocating the buffer every tick.
        self.allocate = Bytes::from(stun::build_wasm_stun_allocate_request(
            &tx,
            &self.relay_token,
            &self.endpoint_xor,
            &self.integrity_key,
            &self.call_id,
            &self.self_participant_id,
        ));
        self.outbox
            .push_back(Output::Transmit(self.allocate.clone()));
        self.keepalive_deadline = now + KEEPALIVE_MS;
        self.allocate_deadline = now + ALLOCATE_TIMEOUT_MS;
        if let Some(m) = &mut self.media {
            m.playout_deadline = now + PLAYOUT_MS;
        }
    }

    /// Apply one input at time `now`.
    pub fn handle_input(&mut self, now: Millis, input: Input<'_>) {
        // Inert after a terminal failure: emit no further intents (the driver is tearing down).
        if self.terminated {
            return;
        }
        match input {
            Input::Timeout => self.on_timeout(now),
            Input::RelayPacket(pkt) => self.on_packet(now, pkt),
            Input::MicFrame(pcm) => self.on_mic(pcm),
            Input::VideoFrame(au) => self.on_video(au),
        }
    }

    /// Drain one intent. Returns `Output::Timeout(deadline)` once the queue is empty; the shell
    /// stops draining there and arms a timer for `deadline` ([`NEVER`] = none).
    pub fn poll_output(&mut self) -> Output {
        self.outbox
            .pop_front()
            .unwrap_or(Output::Timeout(self.poll_timeout().unwrap_or(NEVER)))
    }

    /// The next deadline (the nearer of the keepalive and, if media is on, the playout tick), or
    /// `None` before `start`. Computed on demand from the two deadline fields.
    pub fn poll_timeout(&self) -> Option<Millis> {
        if !self.started || self.terminated {
            return None;
        }
        let mut next = self.keepalive_deadline;
        // The allocate timeout only matters while the allocate is still outstanding.
        if !self.allocated && self.allocate_deadline != NEVER {
            next = next.min(self.allocate_deadline);
        }
        if let Some(m) = &self.media {
            next = next.min(m.playout_deadline);
            next = next.min(self.rtcp_deadline);
        }
        Some(next)
    }

    /// Current playout jitter-buffer depth in samples. Test-only: lets coverage assert the
    /// feed-side bound without exposing the media plane.
    #[cfg(test)]
    pub(crate) fn jitter_len(&self) -> usize {
        self.media.as_ref().map_or(0, |m| m.jitter.len())
    }

    fn on_timeout(&mut self, now: Millis) {
        // The relay never acked the allocate: surface a terminal timeout exactly once, then go inert.
        if !self.allocated
            && self.started
            && self.allocate_deadline != NEVER
            && now >= self.allocate_deadline
        {
            self.allocate_deadline = NEVER;
            self.terminated = true;
            #[cfg(feature = "tracing")]
            tracing::debug!(call_id = %self.call_id, "voip relay allocate timed out");
            self.outbox
                .push_back(Output::Event(CallEvent::RelayAllocateTimedOut));
            return;
        }
        if self.started && now >= self.keepalive_deadline {
            // Re-send the same allocate (consent freshness) plus a fresh-id WA ping.
            self.outbox
                .push_back(Output::Transmit(self.allocate.clone()));
            let tx = self.tx_ids.next_tx_id();
            let ping = stun::build_whatsapp_ping(&tx);
            self.outbox
                .push_back(Output::Transmit(Bytes::copy_from_slice(&ping)));
            self.keepalive_deadline = next_tick(self.keepalive_deadline, now, KEEPALIVE_MS);
        }
        if let Some(m) = self.media.as_mut()
            && self.started
            && now >= m.playout_deadline
        {
            let frame = drain_playout(&mut m.jitter, &mut m.priming, &mut m.priming_ticks);
            m.playout_deadline = next_tick(m.playout_deadline, now, PLAYOUT_MS);
            self.outbox.push_back(Output::Playout(frame));
        }
        if self.started && self.allocated && self.media.is_some() && now >= self.rtcp_deadline {
            self.emit_sender_reports(now, self.rtcp_wallclock_at(now));
            self.rtcp_deadline = next_tick(self.rtcp_deadline, now, RTCP_MS);
        }
    }

    fn rtcp_wallclock_at(&self, monotonic_now: Millis) -> u64 {
        self.rtcp_wallclock_origin_ms
            .saturating_add(monotonic_now.saturating_sub(self.rtcp_monotonic_origin))
    }

    fn announce_audio_rtcp_session(&mut self) {
        let Some(m) = self.media.as_mut().filter(|m| !m.audio_rtcp_announced) else {
            return;
        };
        let packet = m.pipe.audio_source_description();
        m.audio_rtcp_announced = true;
        self.outbox.push_back(Output::Transmit(Bytes::from(packet)));
    }

    /// Audio and video share one wall-clock sample for lip sync.
    fn emit_sender_reports(&mut self, monotonic_ms: Millis, wallclock_ms: u64) {
        let Some(m) = self.media.as_mut() else {
            return;
        };
        let audio_report = m.audio_reception.report(monotonic_ms);
        let audio_sr = m
            .pipe
            .audio_sender_report(wallclock_ms, audio_report.as_ref());
        self.outbox
            .push_back(Output::Transmit(Bytes::from(audio_sr)));
        if let Some(v) = m.video.as_mut().filter(|v| v.active && !v.send_gated) {
            let video_report = v.reception.report(monotonic_ms);
            let video_sr = v
                .pipe
                .video_sender_report(wallclock_ms, video_report.as_ref());
            self.outbox
                .push_back(Output::Transmit(Bytes::from(video_sr)));
        }
    }

    fn on_packet(&mut self, now: Millis, pkt: &[u8]) {
        match classify_relay_packet(pkt) {
            RelayPacketKind::Stun => self.on_stun(now, pkt),
            RelayPacketKind::Rtp => self.on_rtp(now, pkt),
            RelayPacketKind::Rtcp => self.on_rtcp(now, pkt),
            RelayPacketKind::Other => {}
        }
    }

    fn on_rtcp(&mut self, now: Millis, pkt: &[u8]) {
        let event = {
            let Some(m) = self.media.as_mut() else {
                return;
            };
            let audio_ssrc = m.pipe.send_ssrc();
            let video_ssrc = m.video.as_ref().map(|v| v.pipe.send_ssrc());
            let Some(plain) = m.pipe.unprotect_rtcp(pkt) else {
                return;
            };
            let Some(summary) = summarize_rtcp(&plain) else {
                return;
            };
            if let Some(video_ssrc) = video_ssrc
                && requests_keyframe(&summary.feedback, video_ssrc)
                && let Some(video) = m.video.as_mut()
            {
                video.keyframe_required = true;
            }
            if let Some((sender, ntp_seconds, ntp_fraction)) = parse_sender_report_timing(&plain) {
                m.audio_reception
                    .observe_sender_report(sender, ntp_seconds, ntp_fraction, now);
                if let Some(v) = m.video.as_mut() {
                    v.reception
                        .observe_sender_report(sender, ntp_seconds, ntp_fraction, now);
                }
            }
            CallEvent::RtcpReceived {
                reports_audio: summary.referenced_ssrcs.contains(&audio_ssrc),
                reports_video: video_ssrc
                    .is_some_and(|ssrc| summary.referenced_ssrcs.contains(&ssrc)),
                packet_types: summary.packet_types,
                sender_ssrc: summary.sender_ssrc,
                referenced_ssrcs: summary.referenced_ssrcs,
                report_blocks: summary.report_blocks,
                feedback: summary.feedback,
            }
        };
        self.outbox.push_back(Output::Event(event));
    }

    fn on_stun(&mut self, now: Millis, pkt: &[u8]) {
        // Consent freshness (RFC 7675): answer a binding request with a binding success.
        if stun::stun_message_type(pkt) == Some(stun::MSG_BINDING_REQUEST)
            && let Some(req_tx) = stun::stun_transaction_id(pkt)
            && req_tx.len() == 12
        {
            let mut tx12 = [0u8; 12];
            tx12.copy_from_slice(req_tx);
            let resp = stun::encode_stun_request(
                stun::MSG_BINDING_SUCCESS,
                &tx12,
                &[],
                Some(&self.integrity_key),
                true,
            );
            self.outbox.push_back(Output::Transmit(Bytes::from(resp)));
        }
        // The relay acknowledged our allocate; surface it once and stop the allocate timer.
        if !self.allocated && stun::is_allocate_or_binding_success(pkt) {
            self.allocated = true;
            self.allocate_deadline = NEVER;
            #[cfg(feature = "tracing")]
            tracing::debug!(call_id = %self.call_id, "voip relay allocated");
            self.outbox
                .push_back(Output::Event(CallEvent::RelayAllocated));
            if self.media.is_some() {
                self.rtcp_deadline = now + RTCP_MS;
                self.announce_audio_rtcp_session();
            }
            return;
        }
        // A complete allocate-error (a parsed ERROR-CODE) terminates the call; STUN-typed garbage
        // whose error code cannot be parsed is ignored rather than hanging up.
        if !self.allocated
            && self.allocate_deadline != NEVER
            && stun::is_allocate_error(pkt)
            && let Some(code) = stun::parse_stun_error_code(pkt)
        {
            self.allocate_deadline = NEVER;
            self.terminated = true;
            #[cfg(feature = "tracing")]
            tracing::debug!(call_id = %self.call_id, code, "voip relay allocate failed");
            self.outbox
                .push_back(Output::Event(CallEvent::RelayAllocateFailed(code)));
        }
    }

    fn on_rtp(&mut self, now: Millis, pkt: &[u8]) {
        let Some(m) = self.media.as_mut() else {
            return;
        };
        // Demux by payload type BEFORE unprotect: audio and video share E2E keys but have
        // distinct SSRCs/ROC trackers, so feeding a video packet through the audio pipeline
        // would fail its MI tag at best and desync at worst.
        if parse_rtp_header(pkt).is_some_and(|h| h.payload_type == RTP_PAYLOAD_TYPE_H264) {
            // A PT-97 packet with no ACTIVE video plane (not negotiated, or after a downgrade) is
            // dropped. The pipe still advances its recv ROC on drop-free packets it never sees, but
            // an inactive plane simply ignores them.
            if let Some(v) = m.video.as_mut().filter(|v| v.active)
                && let Some((header, completed)) = v.pipe.unprotect_video_packet(pkt)
            {
                v.reception.observe(
                    header.ssrc,
                    header.sequence_number,
                    header.timestamp,
                    now,
                    VIDEO_CLOCK_RATE,
                );
                for au in completed {
                    let keyframe = au_is_keyframe(&au);
                    self.outbox.push_back(Output::VideoPlayout(VideoFrame {
                        data: au,
                        keyframe,
                        orientation: self.peer_video_orientation,
                    }));
                }
            }
            return;
        }
        let Some((header, payload)) = m.pipe.unprotect_audio(pkt) else {
            return;
        };
        m.audio_reception.observe(
            header.ssrc,
            header.sequence_number,
            header.timestamp,
            now,
            AUDIO_CLOCK_RATE,
        );
        // SFrame on: use the GCM-decrypted bytes; otherwise the SRTP payload is already plain Opus.
        let opus = match m.sframe.as_ref().map(|s| s.decrypt(&payload)) {
            Some(SframeIn::Decrypted(plain)) => plain,
            _ => payload,
        };
        let first = opus.first().copied().unwrap_or(0);
        if is_standard_opus_frame(first) {
            // Not portably decodable (libopus is FFI); hand it to the shell.
            self.outbox
                .push_back(Output::Event(CallEvent::ForeignAudio(Bytes::from(opus))));
            return;
        }
        // MLow decode (f32 [-1,1]) -> i16, appended to the playout buffer.
        for s in m.decoder.decode(&opus) {
            m.jitter
                .push_back((s * 32767.0).clamp(-32768.0, 32767.0) as i16);
        }
        // Bound the buffer on the feed side too: a burst of inbound packets arriving between two 20ms
        // playout ticks must not grow `jitter` without limit (drain_playout's cap only runs on a
        // tick). Drop oldest past the same ceiling the drain path uses.
        if m.jitter.len() > PLAYOUT_CAP {
            let drop_n = m.jitter.len() - PLAYOUT_CAP;
            m.jitter.drain(..drop_n);
        }
    }

    fn on_mic(&mut self, pcm: &[i16]) {
        let Some(m) = self.media.as_mut() else {
            return;
        };
        // Drop a wrong-length frame before any send: the encoder needs exactly one 60ms frame, and a
        // mis-sized buffer must not reach the DTX fast-path (which would emit an off-cadence packet).
        if pcm.len() != MIC_FRAME_SAMPLES {
            return;
        }
        // OS mic-mute delivers an exactly all-zero frame; genuine quiet speech carries LSB noise.
        // Don't gap the wire on mute: send a cheap cached DTX comfort-noise frame so the peer's
        // media-liveness timer stays fed (no codec CPU) and it doesn't re-negotiate the transport.
        if pcm.iter().all(|&s| s == 0) {
            let packet = m.pipe.protect_audio(&MLOW_DTX_CNG);
            self.outbox.push_back(Output::Transmit(Bytes::from(packet)));
            return;
        }
        m.scratch.clear();
        m.scratch.extend(pcm.iter().map(|&s| s as f32 / 32768.0));
        // A transient encode failure drops just this frame; the next one resyncs.
        let Ok(coded) = m.encoder.encode(&m.scratch) else {
            return;
        };
        // No SFrame on send by design: the encoded frame goes plain into WAHKDF SRTP, which the peer
        // accepts. `enable_sframe` is recv-decrypt-only (see CallConfig). This matches the
        // pre-refactor send path; send-side SFrame is intentionally not wired.
        let packet = m.pipe.protect_audio(&coded);
        self.outbox.push_back(Output::Transmit(Bytes::from(packet)));
    }

    fn on_video(&mut self, au: &[u8]) {
        // Drop unless the plane is active AND ungated: an inactive plane (audio-only / post-
        // downgrade) or a send-gated one (upgrade requested but not yet accepted) must not put video
        // on the wire. Either way the pipe's send seq/ROC stay frozen for a later resume.
        let Some(v) = self
            .media
            .as_mut()
            .and_then(|m| m.video.as_mut())
            .filter(|v| v.active && !v.send_gated)
        else {
            return;
        };
        if v.keyframe_required && !au_has_idr(au) {
            return;
        }
        let packets = v.pipe.protect_video(au);
        if packets.is_empty() {
            return;
        }
        v.keyframe_required = false;
        for packet in packets {
            self.outbox.push_back(Output::Transmit(Bytes::from(packet)));
        }
    }
}

/// Advance a periodic deadline past `now`. Normally one interval; if the shell fell far behind
/// (more than one interval late) resync to `now + interval` so we emit one tick, not a backlog.
fn next_tick(deadline: Millis, now: Millis, interval: Millis) -> Millis {
    let stepped = deadline + interval;
    if stepped <= now {
        now + interval
    } else {
        stepped
    }
}

/// Drain one 20ms playout slice. Caps the buffer at the latency ceiling, then while priming holds
/// silence WITHOUT draining until the cushion reaches `PLAYOUT_TARGET`; once primed it takes up to
/// `PLAYOUT_DRAIN` samples and re-arms priming on an underrun, so a late packet costs one clean
/// re-prime rather than a silence pad every tick. Priming also gives up after `MAX_PRIME_TICKS` if
/// the buffer holds some audio but never reaches the target (the peer sent one frame then went DTX),
/// flushing it instead of stalling silent forever.
fn drain_playout(
    jitter: &mut VecDeque<i16>,
    priming: &mut bool,
    priming_ticks: &mut u32,
) -> Vec<i16> {
    if jitter.len() > PLAYOUT_CAP {
        let drop_n = jitter.len() - PLAYOUT_CAP;
        jitter.drain(..drop_n);
    }
    if *priming {
        let reached_target = jitter.len() >= PLAYOUT_TARGET;
        // Bounded wait: a partial buffer that never reaches the target (peer DTX after one frame) is
        // flushed rather than held silent forever / replayed stale when a much later packet arrives.
        let timed_out = *priming_ticks >= MAX_PRIME_TICKS && !jitter.is_empty();
        if reached_target || timed_out {
            *priming = false;
            *priming_ticks = 0;
        } else {
            // Age the timeout only while a partial buffer is actually waiting to fill. An empty
            // buffer (call start, or a DTX gap) doesn't count, so the first real frame still gets the
            // full prebuffer cushion instead of flushing instantly on a counter left high by silence.
            *priming_ticks = if jitter.is_empty() {
                0
            } else {
                *priming_ticks + 1
            };
            return vec![0; PLAYOUT_DRAIN];
        }
    }
    let take = jitter.len().min(PLAYOUT_DRAIN);
    let mut frame: Vec<i16> = jitter.drain(..take).collect();
    if frame.len() < PLAYOUT_DRAIN {
        *priming = true;
        *priming_ticks = 0;
        frame.resize(PLAYOUT_DRAIN, 0);
    }
    frame
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voip::e2e_srtp::SRTCP_AUTH_TAG_LEN;
    use crate::voip::mlow::MlowEncoder;
    use crate::voip::rtcp::parse_rtcp_sender_ssrc;
    use crate::voip::warp::WARP_MI_TAG_LEN;

    const SELF_LID: &str = "111111111111111:0@lid";
    const PEER_LID: &str = "222222222222222:0@lid";
    const SSRC: u32 = 0x5741_0001;
    const SAMPLES: u32 = 960;

    fn config(enable_media: bool) -> CallConfig {
        CallConfig {
            call_id: "CID".into(),
            direction: CallDirection::Incoming,
            self_lid: SELF_LID.into(),
            peer_lid: PEER_LID.into(),
            call_key: (0u8..32).collect(),
            ssrc: SSRC,
            samples_per_packet: SAMPLES,
            relay_token: vec![0xAB; 16],
            relay_ip: "203.0.113.7".into(),
            relay_port: 3478,
            integrity_key: b"relay-key".to_vec(),
            warp_mi_tag_len: 4,
            enable_media,
            enable_video: false,
            enable_sframe: false,
        }
    }

    // The SRTP callKey and the STUN integrity key must never reach a `{:?}` dump, matching the
    // redaction on the sibling key structs. Pins the manual Debug against a `#[derive(Debug)]` regression.
    #[test]
    fn call_config_debug_redacts_key_material() {
        let cfg = config(true);
        let dbg = format!("{cfg:?}");
        assert!(
            dbg.contains("call_key: \"[redacted]\""),
            "callKey not redacted"
        );
        assert!(
            dbg.contains("integrity_key: \"[redacted]\""),
            "integrity_key not redacted"
        );
        assert!(
            dbg.contains("relay_token: \"[redacted]\""),
            "relay_token not redacted"
        );
        // The 0..32 callKey bytes, the b"relay-key" integrity key, and the 0xAB relay-token bytes
        // must not appear.
        assert!(!dbg.contains("[0, 1, 2, 3"), "callKey bytes leaked");
        assert!(!dbg.contains("114, 101, 108"), "integrity_key bytes leaked");
        assert!(!dbg.contains("[171, 171"), "relay_token bytes leaked");
        // Non-secret fields stay visible for diagnostics.
        assert!(dbg.contains("call_id: \"CID\""));
    }

    fn engine(enable_media: bool) -> CallEngine {
        CallEngine::new(config(enable_media), Box::new(SequentialTxIds::new())).unwrap()
    }

    // CallConfig::for_incoming pulls the relay endpoint/token/integrity-key out of a parsed RelayData
    // and derives our participant SSRC, so the media-config build is offline testable end to end.
    #[test]
    fn for_incoming_builds_config_from_relay() {
        use crate::voip::relay_parse::{RelayAddress, RelayData, RelayEndpoint};
        let relay = RelayData {
            relay_key_ascii: Some(b"relay-key".to_vec()),
            warp_mi_tag_len: Some(4),
            relay_tokens: vec![vec![0xAB; 16]],
            endpoints: vec![RelayEndpoint {
                relay_id: 1,
                relay_name: "gru1c02".into(),
                token_id: 0,
                auth_token_id: 1,
                addresses: vec![RelayAddress {
                    protocol: 0,
                    ipv4: Some("203.0.113.7".into()),
                    ipv6: None,
                    port: 3478,
                }],
                ..Default::default()
            }],
            ..Default::default()
        };
        let cfg = CallConfig::for_incoming("CID", SELF_LID, PEER_LID, (0u8..32).collect(), &relay)
            .expect("config builds from a complete relay");
        assert_eq!(cfg.relay_ip, "203.0.113.7");
        assert_eq!(cfg.relay_port, 3478);
        assert_eq!(cfg.relay_token, vec![0xAB; 16]);
        assert_eq!(cfg.integrity_key, b"relay-key");
        assert_eq!(cfg.direction, CallDirection::Incoming);
        assert!(cfg.enable_media && cfg.enable_sframe);
        // SSRC is the deterministic E2E derivation over our self LID.
        assert_eq!(
            cfg.ssrc,
            crate::voip::ssrc::derive_wasm_participant_ssrc(
                "CID",
                &crate::voip::ssrc::format_e2e_srtp_participant_id(SELF_LID),
                0
            )
        );
        // A relay with no <key> is rejected (no STUN integrity key to sign the allocate).
        let mut no_key = relay.clone();
        no_key.relay_key_ascii = None;
        assert!(matches!(
            CallConfig::for_incoming("CID", SELF_LID, PEER_LID, (0u8..32).collect(), &no_key),
            Err(SetupError::NoIntegrityKey)
        ));
        // No endpoints -> NoRelayEndpoint.
        let mut no_ep = relay.clone();
        no_ep.endpoints.clear();
        assert!(matches!(
            CallConfig::for_incoming("CID", SELF_LID, PEER_LID, (0u8..32).collect(), &no_ep),
            Err(SetupError::NoRelayEndpoint)
        ));
        // A padded-empty token slot (sparse token block) is a missing token, not a zero-length one.
        let mut empty_token = relay.clone();
        empty_token.relay_tokens = vec![Vec::new()];
        assert!(matches!(
            CallConfig::for_incoming("CID", SELF_LID, PEER_LID, (0u8..32).collect(), &empty_token),
            Err(SetupError::NoRelayToken(0))
        ));
    }

    // for_outgoing mirrors for_incoming (same relay parse + SSRC derivation) but sets Outgoing and
    // takes the locally-generated callKey.
    #[test]
    fn for_outgoing_builds_config_from_relay() {
        use crate::voip::relay_parse::{RelayAddress, RelayData, RelayEndpoint};
        let relay = RelayData {
            relay_key_ascii: Some(b"relay-key".to_vec()),
            warp_mi_tag_len: Some(4),
            relay_tokens: vec![vec![0xAB; 16]],
            endpoints: vec![RelayEndpoint {
                relay_id: 1,
                relay_name: "gru1c02".into(),
                token_id: 0,
                auth_token_id: 1,
                addresses: vec![RelayAddress {
                    protocol: 0,
                    ipv4: Some("203.0.113.7".into()),
                    ipv6: None,
                    port: 3478,
                }],
                ..Default::default()
            }],
            ..Default::default()
        };
        let cfg = CallConfig::for_outgoing("CID", SELF_LID, PEER_LID, (0u8..32).collect(), &relay)
            .expect("config builds from a complete relay");
        assert_eq!(cfg.direction, CallDirection::Outgoing);
        assert_eq!(cfg.relay_ip, "203.0.113.7");
        assert_eq!(cfg.relay_port, 3478);
        assert_eq!(cfg.relay_token, vec![0xAB; 16]);
        assert_eq!(cfg.integrity_key, b"relay-key");
        assert_eq!(cfg.call_key, (0u8..32).collect::<Vec<u8>>());
        assert!(cfg.enable_media && cfg.enable_sframe);
        assert_eq!(
            cfg.ssrc,
            crate::voip::ssrc::derive_wasm_participant_ssrc(
                "CID",
                &crate::voip::ssrc::format_e2e_srtp_participant_id(SELF_LID),
                0
            )
        );
        // The same relay-completeness errors apply.
        let mut no_key = relay.clone();
        no_key.relay_key_ascii = None;
        assert!(matches!(
            CallConfig::for_outgoing("CID", SELF_LID, PEER_LID, (0u8..32).collect(), &no_key),
            Err(SetupError::NoIntegrityKey)
        ));
    }

    /// Drain the engine fully, returning every intent up to (and excluding) the terminal Timeout,
    /// plus that deadline.
    fn drain(eng: &mut CallEngine) -> (Vec<Output>, Millis) {
        let mut out = Vec::new();
        loop {
            match eng.poll_output() {
                Output::Timeout(t) => return (out, t),
                other => out.push(other),
            }
        }
    }

    /// Build an Allocate-error STUN packet carrying an ERROR-CODE attr for `code` (class*100+num).
    fn allocate_error(code: u16) -> Vec<u8> {
        let class = (code / 100) as u8;
        let number = (code % 100) as u8;
        // Raw ERROR-CODE (0x0009) TLV: type, len=4, value (2 reserved bytes, class, number).
        let err_attr = [0x00, 0x09, 0x00, 0x04, 0x00, 0x00, class, number];
        stun::encode_stun_request(stun::MSG_ALLOCATE_ERROR, &[3u8; 12], &err_attr, None, false)
    }

    fn count_transmits(outs: &[Output]) -> usize {
        outs.iter()
            .filter(|o| matches!(o, Output::Transmit(_)))
            .count()
    }

    fn feed_frame(b: &mut VecDeque<i16>) {
        // One 60ms peer frame of nonzero samples, so a real slice is distinguishable from a pad.
        b.extend((0..960i32).map(|i| (i % 200) as i16 - 99));
    }

    #[test]
    fn playout_primes_to_target_before_audio() {
        let mut buf: VecDeque<i16> = VecDeque::new();
        let mut priming = true;
        let mut priming_ticks = 0u32;
        // One frame is below PLAYOUT_TARGET (two frames): playout holds silence without draining.
        feed_frame(&mut buf);
        assert!(
            drain_playout(&mut buf, &mut priming, &mut priming_ticks)
                .iter()
                .all(|&s| s == 0),
            "below the prebuffer target playout primes with silence"
        );
        assert_eq!(buf.len(), 960, "priming must not consume the buffer");
        // The second frame reaches the target; playout now produces real audio.
        feed_frame(&mut buf);
        assert!(
            drain_playout(&mut buf, &mut priming, &mut priming_ticks)
                .iter()
                .any(|&s| s != 0),
            "at the prebuffer target playout starts real audio"
        );
    }

    #[test]
    fn playout_prebuffer_absorbs_inter_arrival_jitter() {
        // Packets (one 60ms peer frame) arrive at a jittered cadence around every 3rd 20ms tick, with
        // gaps up to 4 ticks that stay within the 60ms cushion. The primed buffer must emit no
        // mid-stream silence; the old floor-riding drain (no prebuffer) underruns on the same schedule.
        let arrivals = [0usize, 3, 7, 9, 12, 16, 18, 21, 25, 27, 30];
        let ticks = 34;
        let feed = |buf: &mut VecDeque<i16>, t: usize| {
            if arrivals.contains(&t) {
                feed_frame(buf);
            }
        };
        let midstream_silence = |frames: &[bool]| -> usize {
            match (
                frames.iter().position(|&r| r),
                frames.iter().rposition(|&r| r),
            ) {
                (Some(a), Some(b)) => (a..=b).filter(|&t| !frames[t]).count(),
                _ => 0,
            }
        };

        // The pre-fix drain: drains the floor immediately and silence-pads underruns.
        fn floor_drain(jitter: &mut VecDeque<i16>) -> Vec<i16> {
            let take = jitter.len().min(PLAYOUT_DRAIN);
            let mut f: Vec<i16> = jitter.drain(..take).collect();
            f.resize(PLAYOUT_DRAIN, 0);
            f
        }
        let mut old_buf = VecDeque::new();
        let old_real: Vec<bool> = (0..ticks)
            .map(|t| {
                feed(&mut old_buf, t);
                floor_drain(&mut old_buf).iter().any(|&s| s != 0)
            })
            .collect();
        assert!(
            midstream_silence(&old_real) > 0,
            "schedule must stress the buffer: the floor-riding drain should underrun"
        );

        let mut buf = VecDeque::new();
        let mut priming = true;
        let mut priming_ticks = 0u32;
        let real: Vec<bool> = (0..ticks)
            .map(|t| {
                feed(&mut buf, t);
                drain_playout(&mut buf, &mut priming, &mut priming_ticks)
                    .iter()
                    .any(|&s| s != 0)
            })
            .collect();
        assert_eq!(
            midstream_silence(&real),
            0,
            "prebuffer must absorb the inter-arrival jitter with no mid-stream silence"
        );
    }

    #[test]
    fn bad_endpoint_rejected() {
        let mut cfg = config(true);
        cfg.relay_ip = "not-an-ip".into();
        assert!(matches!(
            CallEngine::new(cfg, Box::new(SequentialTxIds::new())),
            Err(EngineError::BadEndpoint)
        ));
    }

    #[test]
    fn short_call_key_rejected() {
        let mut cfg = config(true);
        cfg.call_key = vec![0u8; 16];
        assert!(matches!(
            CallEngine::new(cfg, Box::new(SequentialTxIds::new())),
            Err(EngineError::BadCallKey)
        ));
    }

    #[test]
    fn start_emits_allocate_and_arms_playout_first() {
        let mut eng = engine(true);
        assert_eq!(eng.poll_timeout(), None);
        eng.start(0, 0);
        let (outs, deadline) = drain(&mut eng);
        // The initial allocate is the only transmit; playout (20ms) is the nearer deadline.
        assert_eq!(count_transmits(&outs), 1);
        assert!(matches!(outs[0], Output::Transmit(_)));
        assert_eq!(deadline, PLAYOUT_MS);
        assert_eq!(eng.poll_timeout(), Some(PLAYOUT_MS));
    }

    #[test]
    fn control_plane_only_arms_keepalive_no_playout() {
        // esp32-style: no media. The only deadline is the 1s keepalive, and mic frames are ignored.
        let mut eng = engine(false);
        eng.start(0, 0);
        let (outs, deadline) = drain(&mut eng);
        assert_eq!(count_transmits(&outs), 1); // allocate
        assert_eq!(deadline, KEEPALIVE_MS);
        // A mic frame produces nothing without a media plane.
        eng.handle_input(5, Input::MicFrame(&[1234i16; SAMPLES as usize]));
        let (outs, _) = drain(&mut eng);
        assert_eq!(count_transmits(&outs), 0);
    }

    #[test]
    fn keepalive_fires_allocate_and_ping() {
        let mut eng = engine(false);
        eng.start(0, 0);
        let _ = drain(&mut eng);
        eng.handle_input(KEEPALIVE_MS, Input::Timeout);
        let (outs, deadline) = drain(&mut eng);
        // allocate + ping.
        assert_eq!(count_transmits(&outs), 2);
        assert_eq!(deadline, 2 * KEEPALIVE_MS);
    }

    #[test]
    fn playout_emits_silence_every_tick() {
        let mut eng = engine(true);
        eng.start(0, 0);
        let _ = drain(&mut eng);
        eng.handle_input(PLAYOUT_MS, Input::Timeout);
        let (outs, deadline) = drain(&mut eng);
        match outs.as_slice() {
            [Output::Playout(frame)] => {
                assert_eq!(frame.len(), PLAYOUT_DRAIN);
                assert!(frame.iter().all(|&s| s == 0), "no audio fed yet -> silence");
            }
            other => panic!("expected one Playout, got {other:?}"),
        }
        assert_eq!(deadline, 2 * PLAYOUT_MS);
    }

    #[test]
    fn binding_request_gets_binding_success() {
        let mut eng = engine(true);
        eng.start(0, 0);
        let _ = drain(&mut eng);
        let req =
            stun::encode_stun_request(stun::MSG_BINDING_REQUEST, &[7u8; 12], &[], None, false);
        eng.handle_input(1, Input::RelayPacket(&req));
        let (outs, _) = drain(&mut eng);
        let transmits: Vec<&Output> = outs
            .iter()
            .filter(|o| matches!(o, Output::Transmit(_)))
            .collect();
        assert_eq!(transmits.len(), 1, "exactly one binding-success reply");
        if let Output::Transmit(b) = transmits[0] {
            assert_eq!(stun::stun_message_type(b), Some(stun::MSG_BINDING_SUCCESS));
        }
    }

    #[test]
    fn allocate_success_emits_event_once() {
        let mut eng = engine(true);
        eng.start(0, 0);
        let _ = drain(&mut eng);
        let ok =
            stun::encode_stun_request(stun::MSG_ALLOCATE_SUCCESS, &[1u8; 12], &[], None, false);
        eng.handle_input(1, Input::RelayPacket(&ok));
        let (outs, _) = drain(&mut eng);
        assert_eq!(
            outs.iter()
                .filter(|o| matches!(o, Output::Event(CallEvent::RelayAllocated)))
                .count(),
            1
        );
        assert!(eng.is_allocated());
        // A second success does not re-emit.
        eng.handle_input(2, Input::RelayPacket(&ok));
        let (outs, _) = drain(&mut eng);
        assert_eq!(
            outs.iter()
                .filter(|o| matches!(o, Output::Event(_)))
                .count(),
            0
        );
    }

    #[test]
    fn mic_drops_wrong_length_frames() {
        let mut eng = engine(true);
        eng.start(0, 0);
        let _ = drain(&mut eng);
        // A wrong-length all-zero buffer must not reach the DTX fast-path: it is dropped, not sent.
        eng.handle_input(1, Input::MicFrame(&[0i16; 480]));
        let (outs, _) = drain(&mut eng);
        assert_eq!(
            count_transmits(&outs),
            0,
            "a short muted frame must be dropped"
        );
        eng.handle_input(2, Input::MicFrame(&[]));
        let (outs, _) = drain(&mut eng);
        assert_eq!(count_transmits(&outs), 0, "an empty frame must be dropped");
        // A correctly-sized all-zero frame still emits one DTX packet.
        eng.handle_input(3, Input::MicFrame(&[0i16; SAMPLES as usize]));
        let (outs, _) = drain(&mut eng);
        assert_eq!(
            count_transmits(&outs),
            1,
            "a 960-sample muted frame transmits DTX"
        );
    }

    #[test]
    fn mic_mute_emits_dtx_keepalive_not_a_gap() {
        let mut eng = engine(true);
        eng.start(0, 0);
        let _ = drain(&mut eng);
        // OS mute delivers exact-zero frames; each must still transmit a DTX comfort-noise frame so
        // the peer's media-liveness timer stays fed (a gap makes the peer re-negotiate the transport).
        let call_key: Vec<u8> = (0u8..32).collect();
        let mut peer = MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid: PEER_LID,
            peer_lid: SELF_LID,
            ssrc: SSRC,
            samples_per_packet: SAMPLES,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap();
        for k in 1..=5u64 {
            eng.handle_input(k, Input::MicFrame(&[0i16; SAMPLES as usize]));
            let (outs, _) = drain(&mut eng);
            assert_eq!(
                count_transmits(&outs),
                1,
                "muted tick {k} must transmit DTX, not skip"
            );
            let pkt = outs
                .iter()
                .find_map(|o| match o {
                    Output::Transmit(b) => Some(b.clone()),
                    _ => None,
                })
                .expect("a transmit");
            let (_, payload) = peer
                .unprotect_audio(&pkt)
                .expect("muted DTX packet must decrypt");
            assert_eq!(payload.len(), 1, "DTX is one byte");
            assert_eq!(
                payload[0], 0x90,
                "muted frame payload is the mlow DTX token"
            );
        }
        // A real tone still encodes + protects to one RTP transmit.
        let tone: Vec<i16> = (0..SAMPLES as usize)
            .map(|i| (8000.0 * (i as f32 * 0.1).sin()) as i16)
            .collect();
        eng.handle_input(6, Input::MicFrame(&tone));
        let (outs, _) = drain(&mut eng);
        assert_eq!(count_transmits(&outs), 1);
    }

    #[test]
    fn inbound_rtp_decodes_into_playout() {
        // A mirrored peer (its self LID is our peer LID) encrypts real MLow tone frames; the engine
        // must SRTP-decrypt, MLow-decode, and drain them to the speaker as non-silent audio. Two
        // frames are sent so the playout prebuffer reaches PLAYOUT_TARGET and starts draining.
        let mut eng = engine(true);
        eng.start(0, 0);
        let _ = drain(&mut eng);

        let call_key: Vec<u8> = (0u8..32).collect();
        let mut peer_tx = MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid: PEER_LID,
            peer_lid: SELF_LID,
            ssrc: SSRC,
            samples_per_packet: SAMPLES,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap();
        let mut peer_enc = MlowEncoder::new();
        for n in 0..2u32 {
            let tone: Vec<f32> = (0..SAMPLES as usize)
                .map(|i| 0.3 * ((i as f32 + (n * SAMPLES) as f32) * 0.07).sin())
                .collect();
            let frame = peer_enc.encode(&tone).expect("mlow encode");
            let packet = peer_tx.protect_audio(&frame);
            eng.handle_input(1, Input::RelayPacket(&packet));
        }
        // Drain enough playout ticks to pass priming and pull the decoded ~1920 samples (320/tick).
        let mut peak = 0i16;
        for k in 1..=8 {
            eng.handle_input(k * PLAYOUT_MS, Input::Timeout);
            let (outs, _) = drain(&mut eng);
            for o in outs {
                if let Output::Playout(frame) = o {
                    peak = peak.max(frame.iter().map(|s| s.abs()).max().unwrap_or(0));
                }
            }
        }
        assert!(peak > 0, "decoded peer audio must reach the playout buffer");
    }

    // End-to-end of the device-mismatch fix: an engine built for the dialed base callee LID receives
    // garbage from the device that actually answered (a companion `:2`), until `rekey_recv` re-keys
    // the recv path to that device — after which its audio decodes and reaches playout.
    #[test]
    fn rekey_recv_switches_inbound_to_answering_device() {
        let mut eng = engine(true); // recv keyed to PEER_LID = "222...:0@lid" (the dialed base)
        eng.start(0, 0);
        let _ = drain(&mut eng);

        let call_key: Vec<u8> = (0u8..32).collect();
        let answering = "222222222222222:2@lid"; // a companion, NOT the dialed base device
        let mut answerer_tx = MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid: answering,
            peer_lid: SELF_LID,
            ssrc: SSRC,
            samples_per_packet: SAMPLES,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap();
        let mut enc = MlowEncoder::new();
        let tone = |n: u32| -> Vec<f32> {
            (0..SAMPLES as usize)
                .map(|i| 0.3 * ((i as f32 + (n * SAMPLES) as f32) * 0.07).sin())
                .collect()
        };

        // Before rekey: recv keyed to the base, so the companion's frames don't decode (garbage).
        for n in 0..2u32 {
            let packet = answerer_tx.protect_audio(&enc.encode(&tone(n)).unwrap());
            eng.handle_input(1, Input::RelayPacket(&packet));
            let _ = drain(&mut eng);
        }

        assert!(eng.rekey_recv(answering));

        // After rekey: the companion's frames decode to real audio that reaches playout.
        for n in 2..4u32 {
            let packet = answerer_tx.protect_audio(&enc.encode(&tone(n)).unwrap());
            eng.handle_input(1, Input::RelayPacket(&packet));
            let _ = drain(&mut eng);
        }
        let mut peak = 0i16;
        for k in 1..=8 {
            eng.handle_input(k * PLAYOUT_MS, Input::Timeout);
            let (outs, _) = drain(&mut eng);
            for o in outs {
                if let Output::Playout(frame) = o {
                    peak = peak.max(frame.iter().map(|s| s.abs()).max().unwrap_or(0));
                }
            }
        }
        assert!(
            peak > 0,
            "after rekey the answering device's audio must reach playout"
        );
    }

    #[test]
    fn merged_deadline_is_the_nearer_of_keepalive_and_playout() {
        let mut eng = engine(true);
        eng.start(0, 0);
        let _ = drain(&mut eng);
        // Playout (20) is nearer than keepalive (1000) right after start.
        assert_eq!(eng.poll_timeout(), Some(PLAYOUT_MS));
    }

    // A burst of inbound frames arriving between two playout ticks must not grow the jitter buffer
    // without bound: on_rtp caps it at the same ceiling drain_playout uses. Regression for the
    // feed-side unbounded-growth path (no Timeout is interleaved, so the drain-time cap never runs).
    #[test]
    fn inbound_burst_keeps_jitter_bounded() {
        let mut eng = engine(true);
        eng.start(0, 0);
        let _ = drain(&mut eng);
        let call_key: Vec<u8> = (0u8..32).collect();
        let mut peer_tx = MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid: PEER_LID,
            peer_lid: SELF_LID,
            ssrc: SSRC,
            samples_per_packet: SAMPLES,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap();
        let mut peer_enc = MlowEncoder::new();
        for n in 0..200u32 {
            let tone: Vec<f32> = (0..SAMPLES as usize)
                .map(|i| 0.3 * ((i as f32 + (n * SAMPLES) as f32) * 0.05).sin())
                .collect();
            let frame = peer_enc.encode(&tone).expect("mlow encode");
            let packet = peer_tx.protect_audio(&frame);
            eng.handle_input(1, Input::RelayPacket(&packet));
            let _ = drain(&mut eng);
        }
        assert!(
            eng.jitter_len() <= PLAYOUT_CAP,
            "feed-side jitter must stay <= PLAYOUT_CAP, got {}",
            eng.jitter_len()
        );
    }

    // A decrypted payload whose first byte marks standard Opus/CELT ((b & 0xC0) == 0xC0) is surfaced
    // as CallEvent::ForeignAudio for the shell to decode, NOT pushed into the MLow playout buffer.
    #[test]
    fn standard_opus_payload_routes_to_foreign_audio() {
        let mut eng = engine(true);
        eng.start(0, 0);
        let _ = drain(&mut eng);
        let call_key: Vec<u8> = (0u8..32).collect();
        let mut peer_tx = MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid: PEER_LID,
            peer_lid: SELF_LID,
            ssrc: SSRC,
            samples_per_packet: SAMPLES,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap();
        let packet = peer_tx.protect_audio(&[0xC8u8, 1, 2, 3, 4, 5]); // 0xC8 & 0xC0 == 0xC0
        eng.handle_input(1, Input::RelayPacket(&packet));
        let (outs, _) = drain(&mut eng);
        assert_eq!(
            outs.iter()
                .filter(|o| matches!(o, Output::Event(CallEvent::ForeignAudio(_))))
                .count(),
            1,
            "standard-Opus payload must surface exactly one ForeignAudio event"
        );
        assert_eq!(
            eng.jitter_len(),
            0,
            "foreign audio must not enter the MLow playout buffer"
        );
    }

    // The inbound SFrame decrypt branch end-to-end: a mirrored peer GCM-wraps an MLow frame (its
    // encrypt key == our decrypt key), SRTP-protects it, and the engine must SRTP-decrypt, SFrame-
    // decrypt, MLow-decode, and play it. All other engine tests run enable_sframe = false.
    #[test]
    fn sframe_wrapped_inbound_decrypts_and_plays() {
        let mut cfg = config(true);
        cfg.enable_sframe = true;
        let mut eng = CallEngine::new(cfg, Box::new(SequentialTxIds::new())).unwrap();
        eng.start(0, 0);
        let _ = drain(&mut eng);
        let call_key: Vec<u8> = (0u8..32).collect();
        let mut peer_tx = MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid: PEER_LID,
            peer_lid: SELF_LID,
            ssrc: SSRC,
            samples_per_packet: SAMPLES,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap();
        // Mirror: peer's self = our peer, peer's peer = our self -> peer.encrypt key == our decrypt key.
        let mut peer_sframe = SframeSession::new(&call_key, PEER_LID, SELF_LID).unwrap();
        let mut peer_enc = MlowEncoder::new();
        for n in 0..2u32 {
            let tone: Vec<f32> = (0..SAMPLES as usize)
                .map(|i| 0.3 * ((i as f32 + (n * SAMPLES) as f32) * 0.07).sin())
                .collect();
            let frame = peer_enc.encode(&tone).expect("mlow encode");
            let wrapped = peer_sframe.encrypt(&frame);
            let packet = peer_tx.protect_audio(&wrapped);
            eng.handle_input(1, Input::RelayPacket(&packet));
        }
        let mut peak = 0i16;
        for k in 1..=8 {
            eng.handle_input(k * PLAYOUT_MS, Input::Timeout);
            let (outs, _) = drain(&mut eng);
            for o in outs {
                if let Output::Playout(frame) = o {
                    peak = peak.max(frame.iter().map(|s| s.abs()).max().unwrap_or(0));
                }
            }
        }
        assert!(
            peak > 0,
            "SFrame-wrapped peer audio must decrypt, MLow-decode, and reach playout"
        );
    }

    // At t = 1000 the keepalive and playout deadlines coincide; one timeout must fire both.
    #[test]
    fn coincident_keepalive_and_playout_both_fire() {
        let mut eng = engine(true);
        eng.start(0, 0);
        let _ = drain(&mut eng);
        eng.handle_input(KEEPALIVE_MS, Input::Timeout);
        let (outs, _) = drain(&mut eng);
        assert_eq!(count_transmits(&outs), 2, "keepalive allocate + ping");
        assert_eq!(
            outs.iter()
                .filter(|o| matches!(o, Output::Playout(_)))
                .count(),
            1,
            "exactly one playout frame on the coincident tick"
        );
    }

    // Native parity: audio announces SDES once after transport association; video has no initial
    // packet. The first periodic tick then sends independent audio/video SR+SDES compounds.
    #[test]
    fn rtcp_sender_reports_emitted_for_audio_and_video() {
        use crate::voip::e2e_srtp::{derive_srtcp_keys, unprotect_srtcp};
        use crate::voip::rtcp::{RTCP_PT_SDES, RTCP_PT_SR, WHATSAPP_RTCP_CNAME_LEN};

        let mut cfg = config(true);
        cfg.enable_video = true;
        let call_key = cfg.call_key.clone();
        let audio_ssrc = cfg.ssrc;
        let video_ssrc = ssrc::derive_video_participant_ssrc(
            &cfg.call_id,
            &ssrc::format_e2e_srtp_participant_id(&cfg.self_lid),
        );
        let mut eng = CallEngine::new(cfg, Box::new(SequentialTxIds::new())).unwrap();
        eng.start(0, 0);
        let _ = drain(&mut eng);

        // RTCP cannot precede a usable transport.
        eng.handle_input(RTCP_MS, Input::Timeout);
        let (outs, _) = drain(&mut eng);
        assert!(outs.iter().all(|output| !matches!(
            output,
            Output::Transmit(packet)
                if classify_relay_packet(packet) == RelayPacketKind::Rtcp
        )));

        let allocate =
            stun::encode_stun_request(stun::MSG_ALLOCATE_SUCCESS, &[1u8; 12], &[], None, false);
        let allocated_at = RTCP_MS + 1;
        eng.handle_input(allocated_at, Input::RelayPacket(&allocate));
        let (outs, _) = drain(&mut eng);
        let rtcp: Vec<&Bytes> = outs
            .iter()
            .filter_map(|o| match o {
                Output::Transmit(b) if classify_relay_packet(b) == RelayPacketKind::Rtcp => Some(b),
                _ => None,
            })
            .collect();
        assert_eq!(rtcp.len(), 1, "only audio sends an initial SDES");
        assert_eq!(parse_rtcp_sender_ssrc(rtcp[0]), Some(audio_ssrc));
        assert_eq!(rtcp[0].len(), 32 + 4 + SRTCP_AUTH_TAG_LEN);
        let transport = derive_srtcp_keys(&call_key, SELF_LID).unwrap();
        let (plain, _) = unprotect_srtcp(&transport, audio_ssrc, rtcp[0]).unwrap();
        let summary = summarize_rtcp(&plain).unwrap();
        assert_eq!(summary.packet_types, [RTCP_PT_SDES]);
        assert_eq!(summary.sdes_cname_lengths, [WHATSAPP_RTCP_CNAME_LEN]);

        eng.handle_input(allocated_at + RTCP_MS, Input::Timeout);
        let (outs, _) = drain(&mut eng);
        let rtcp: Vec<&Bytes> = outs
            .iter()
            .filter_map(|output| match output {
                Output::Transmit(packet)
                    if classify_relay_packet(packet) == RelayPacketKind::Rtcp =>
                {
                    Some(packet)
                }
                _ => None,
            })
            .collect();
        assert_eq!(rtcp.len(), 2, "one periodic compound per active stream");
        for (ssrc, expected_index) in [(audio_ssrc, 2), (video_ssrc, 1)] {
            let packet = rtcp
                .iter()
                .copied()
                .find(|packet| parse_rtcp_sender_ssrc(packet) == Some(ssrc))
                .expect("stream RTCP packet");
            assert_eq!(packet.len(), 60 + 4 + SRTCP_AUTH_TAG_LEN);
            let index_at = packet.len() - SRTCP_AUTH_TAG_LEN - 4;
            let index = u32::from_be_bytes(packet[index_at..index_at + 4].try_into().unwrap())
                & 0x7fff_ffff;
            assert_eq!(index, expected_index);
            let (plain, _) = unprotect_srtcp(&transport, ssrc, packet).unwrap();
            let summary = summarize_rtcp(&plain).unwrap();
            assert_eq!(summary.packet_types, [RTCP_PT_SR, RTCP_PT_SDES]);
            assert_eq!(summary.sdes_cname_lengths, [WHATSAPP_RTCP_CNAME_LEN]);
        }
    }

    #[test]
    fn video_sender_report_carries_native_video_reception_block() {
        use crate::voip::e2e_srtp::{derive_srtcp_keys, unprotect_srtcp};
        use crate::voip::rtcp::{RTCP_PT_SDES, RTCP_PT_SR};

        let mut cfg = config(true);
        cfg.enable_video = true;
        let call_key = cfg.call_key.clone();
        let local_video_ssrc = ssrc::derive_video_participant_ssrc(
            &cfg.call_id,
            &ssrc::format_e2e_srtp_participant_id(&cfg.self_lid),
        );
        let peer_audio_ssrc = ssrc::derive_wasm_participant_ssrc(
            &cfg.call_id,
            &ssrc::format_e2e_srtp_participant_id(&cfg.peer_lid),
            0,
        );
        let peer_video_ssrc = ssrc::derive_video_participant_ssrc(
            &cfg.call_id,
            &ssrc::format_e2e_srtp_participant_id(&cfg.peer_lid),
        );
        let mut eng = CallEngine::new(cfg, Box::new(SequentialTxIds::new())).unwrap();
        eng.start(0, 1_700_000_000_000);
        let _ = drain(&mut eng);
        let allocate =
            stun::encode_stun_request(stun::MSG_ALLOCATE_SUCCESS, &[1u8; 12], &[], None, false);
        eng.handle_input(1, Input::RelayPacket(&allocate));
        let _ = drain(&mut eng);

        let mut peer_audio = MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid: PEER_LID,
            peer_lid: SELF_LID,
            ssrc: peer_audio_ssrc,
            samples_per_packet: SAMPLES,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap();
        let audio = peer_audio.protect_audio(&[0xf8, 0xff, 0xfe]);
        eng.handle_input(100, Input::RelayPacket(&audio));
        let _ = drain(&mut eng);

        let mut peer_video = peer_video_pipe();
        let video = peer_video
            .protect_video(&video_au(100))
            .pop()
            .expect("one-packet video AU");
        eng.handle_input(101, Input::RelayPacket(&video));
        let _ = drain(&mut eng);

        eng.handle_input(1 + RTCP_MS, Input::Timeout);
        let (outs, _) = drain(&mut eng);
        let protected = outs
            .iter()
            .find_map(|output| match output {
                Output::Transmit(packet)
                    if parse_rtcp_sender_ssrc(packet) == Some(local_video_ssrc) =>
                {
                    Some(packet)
                }
                _ => None,
            })
            .expect("video Sender Report");

        assert_eq!(protected.len(), 76 + 32 + 4 + SRTCP_AUTH_TAG_LEN);
        let transport = derive_srtcp_keys(&call_key, SELF_LID).unwrap();
        let (plain, _) = unprotect_srtcp(&transport, local_video_ssrc, protected).unwrap();
        assert_eq!(&plain[..4], &[0x91, RTCP_PT_SR, 0, 18]);
        assert_eq!(&plain[28..32], &peer_video_ssrc.to_be_bytes());
        assert_eq!(&plain[52..76], &[0; 24]);
        assert_eq!(&plain[76..80], &[0x91, RTCP_PT_SDES, 0, 7]);
        let summary = summarize_rtcp(&plain).unwrap();
        assert_eq!(summary.packet_types, [RTCP_PT_SR, RTCP_PT_SDES]);
        assert_eq!(summary.referenced_ssrcs, [peer_video_ssrc]);
        assert_eq!(summary.report_blocks.len(), 1);
        assert_eq!(summary.report_blocks[0].profile_extension, [0; 24]);
        assert!(summary.uses_whatsapp_profile_extension);
    }

    #[test]
    fn sender_report_uses_unix_wallclock_not_monotonic_time() {
        use crate::voip::e2e_srtp::{derive_srtcp_keys, unprotect_srtcp};

        const UNIX_START_MS: u64 = 1_700_000_000_250;
        let call_key: Vec<u8> = (0u8..32).collect();
        let mut eng = engine(true);
        eng.start(100, UNIX_START_MS);
        let _ = drain(&mut eng);
        let allocate =
            stun::encode_stun_request(stun::MSG_ALLOCATE_SUCCESS, &[1u8; 12], &[], None, false);
        eng.handle_input(100, Input::RelayPacket(&allocate));
        let _ = drain(&mut eng);
        eng.handle_input(100 + RTCP_MS, Input::Timeout);
        let (outs, _) = drain(&mut eng);
        let protected = outs
            .iter()
            .find_map(|output| match output {
                Output::Transmit(packet)
                    if classify_relay_packet(packet) == RelayPacketKind::Rtcp =>
                {
                    Some(packet)
                }
                _ => None,
            })
            .expect("RTCP tick emits an audio Sender Report");
        let sender_ssrc = parse_rtcp_sender_ssrc(protected).unwrap();
        let keys = derive_srtcp_keys(&call_key, SELF_LID).unwrap();
        let (plain, _) = unprotect_srtcp(&keys, sender_ssrc, protected).unwrap();
        let ntp_seconds = u32::from_be_bytes(plain[8..12].try_into().unwrap());
        assert_eq!(
            ntp_seconds,
            (2_208_988_800 + (UNIX_START_MS + RTCP_MS) / 1000) as u32
        );
    }

    #[test]
    fn authenticated_receiver_report_identifies_local_video_ssrc() {
        use crate::voip::e2e_srtp::{derive_srtcp_keys, protect_srtcp};
        use crate::voip::rtcp::RTCP_PT_RR;

        let mut cfg = config(true);
        cfg.enable_video = true;
        let call_key = cfg.call_key.clone();
        let audio_ssrc = cfg.ssrc;
        let video_ssrc = ssrc::derive_video_participant_ssrc(
            &cfg.call_id,
            &ssrc::format_e2e_srtp_participant_id(&cfg.self_lid),
        );
        let peer_ssrc = ssrc::derive_wasm_participant_ssrc(
            &cfg.call_id,
            &ssrc::format_e2e_srtp_participant_id(&cfg.peer_lid),
            0,
        );

        // Receiver Report with one block for audio and one for video, followed by a video PLI.
        let mut rr = vec![0x82, RTCP_PT_RR, 0, 13];
        rr.extend_from_slice(&peer_ssrc.to_be_bytes());
        for reported in [audio_ssrc, video_ssrc] {
            rr.extend_from_slice(&reported.to_be_bytes());
            rr.extend_from_slice(&[0; 20]);
        }
        rr.extend_from_slice(&[0x81, crate::voip::rtcp::RTCP_PT_PSFB, 0, 2]);
        rr.extend_from_slice(&peer_ssrc.to_be_bytes());
        rr.extend_from_slice(&video_ssrc.to_be_bytes());
        let peer_keys = derive_srtcp_keys(&call_key, PEER_LID).unwrap();
        let protected = protect_srtcp(&peer_keys, peer_ssrc, 0, &rr);

        let mut eng = CallEngine::new(cfg, Box::new(SequentialTxIds::new())).unwrap();
        eng.start(0, 0);
        let _ = drain(&mut eng);
        eng.handle_input(1, Input::RelayPacket(&protected));
        let (outs, _) = drain(&mut eng);
        assert!(outs.iter().any(|output| matches!(
            output,
            Output::Event(CallEvent::RtcpReceived {
                packet_types,
                sender_ssrc,
                referenced_ssrcs,
                feedback,
                reports_audio: true,
                reports_video: true,
                ..
            }) if packet_types == &[RTCP_PT_RR, crate::voip::rtcp::RTCP_PT_PSFB]
                && *sender_ssrc == peer_ssrc
                && referenced_ssrcs.contains(&audio_ssrc)
                && referenced_ssrcs.contains(&video_ssrc)
                && feedback.iter().any(|item| item.packet_type == crate::voip::rtcp::RTCP_PT_PSFB
                    && item.fmt == 1
                    && item.media_ssrc == video_ssrc
                    && item.fci.is_empty())
        )));

        eng.handle_input(2, Input::VideoFrame(&video_delta_au(200)));
        assert_eq!(
            count_transmits(&drain(&mut eng).0),
            0,
            "PLI must suppress dependent AUs until recovery"
        );
        eng.handle_input(3, Input::VideoFrame(&video_au(200)));
        assert_eq!(
            count_transmits(&drain(&mut eng).0),
            1,
            "the next IDR must recover transmission"
        );

        let mut forged = protected;
        *forged.last_mut().unwrap() ^= 1;
        eng.handle_input(4, Input::RelayPacket(&forged));
        let (outs, _) = drain(&mut eng);
        assert!(
            !outs
                .iter()
                .any(|output| matches!(output, Output::Event(CallEvent::RtcpReceived { .. }))),
            "forged SRTCP must be dropped"
        );
        eng.handle_input(5, Input::VideoFrame(&video_delta_au(200)));
        assert_eq!(
            count_transmits(&drain(&mut eng).0),
            1,
            "forged feedback must not re-arm keyframe recovery"
        );
    }

    #[test]
    fn keyframe_feedback_must_target_the_local_video_ssrc() {
        let video_ssrc = 0x1122_3344;
        let other_ssrc = 0x5566_7788;
        let feedback = |packet_type, fmt, media_ssrc, fci| RtcpFeedback {
            packet_type,
            fmt,
            sender_ssrc: 0x99aa_bbcc,
            media_ssrc,
            fci,
        };

        assert!(requests_keyframe(
            &[feedback(RTCP_PT_PSFB, 1, video_ssrc, Vec::new())],
            video_ssrc
        ));
        assert!(!requests_keyframe(
            &[feedback(RTCP_PT_PSFB, 1, other_ssrc, Vec::new())],
            video_ssrc
        ));
        assert!(!requests_keyframe(
            &[feedback(205, 1, video_ssrc, Vec::new())],
            video_ssrc
        ));

        let fir = [video_ssrc.to_be_bytes().as_slice(), &[7, 0, 0, 0]].concat();
        assert!(requests_keyframe(
            &[feedback(RTCP_PT_PSFB, 4, 0, fir)],
            video_ssrc
        ));
        let other_fir = [other_ssrc.to_be_bytes().as_slice(), &[8, 0, 0, 0]].concat();
        assert!(!requests_keyframe(
            &[feedback(RTCP_PT_PSFB, 4, 0, other_fir)],
            video_ssrc
        ));
    }

    #[test]
    fn authenticated_malformed_rtcp_is_dropped() {
        use crate::voip::e2e_srtp::{derive_srtcp_keys, protect_srtcp};
        use crate::voip::rtcp::RTCP_PT_SDES;

        let cfg = config(true);
        let call_key = cfg.call_key.clone();
        let peer_ssrc = ssrc::derive_wasm_participant_ssrc(
            &cfg.call_id,
            &ssrc::format_e2e_srtp_participant_id(&cfg.peer_lid),
            0,
        );
        let mut malformed = vec![0x81, RTCP_PT_SDES, 0, 2];
        malformed.extend_from_slice(&peer_ssrc.to_be_bytes());
        malformed.extend_from_slice(&[1, 18, 0, 0]);
        let peer_keys = derive_srtcp_keys(&call_key, PEER_LID).unwrap();
        let protected = protect_srtcp(&peer_keys, peer_ssrc, 3, &malformed);

        let mut eng = CallEngine::new(cfg, Box::new(SequentialTxIds::new())).unwrap();
        eng.start(0, 0);
        let _ = drain(&mut eng);
        eng.handle_input(1, Input::RelayPacket(&protected));
        let (outs, _) = drain(&mut eng);
        assert!(
            !outs
                .iter()
                .any(|output| matches!(output, Output::Event(CallEvent::RtcpReceived { .. }))),
            "malformed RTCP must not surface as authenticated feedback"
        );
    }

    #[test]
    fn sender_reports_use_transport_srtcp_for_audio_and_video() {
        use crate::voip::e2e_srtp::{derive_srtcp_keys, unprotect_srtcp};

        let mut cfg = config(true);
        cfg.enable_video = true;
        let call_key = cfg.call_key.clone();
        let audio_ssrc = cfg.ssrc;
        let video_ssrc = ssrc::derive_video_participant_ssrc(
            &cfg.call_id,
            &ssrc::format_e2e_srtp_participant_id(&cfg.self_lid),
        );
        let mut eng = CallEngine::new(cfg, Box::new(SequentialTxIds::new())).unwrap();
        eng.start(0, 1_700_000_000_000);
        let _ = drain(&mut eng);
        let allocate =
            stun::encode_stun_request(stun::MSG_ALLOCATE_SUCCESS, &[1u8; 12], &[], None, false);
        eng.handle_input(0, Input::RelayPacket(&allocate));
        let _ = drain(&mut eng);
        eng.handle_input(1, Input::VideoFrame(&video_au(200)));
        let _ = drain(&mut eng);
        eng.handle_input(RTCP_MS, Input::Timeout);
        let (outs, _) = drain(&mut eng);

        let find_sr = |ssrc| {
            outs.iter().find_map(|output| match output {
                Output::Transmit(packet) if parse_rtcp_sender_ssrc(packet) == Some(ssrc) => {
                    Some(packet)
                }
                _ => None,
            })
        };
        let audio = find_sr(audio_ssrc).expect("audio SR");
        let video = find_sr(video_ssrc).expect("video SR");
        let transport = derive_srtcp_keys(&call_key, SELF_LID).unwrap();

        assert!(unprotect_srtcp(&transport, audio_ssrc, audio).is_some());
        assert!(unprotect_srtcp(&transport, video_ssrc, video).is_some());
    }

    // The MLow encoder requires exactly 960 samples; a wrong-length mic frame is dropped (no RTP,
    // no panic), not partially sent. Pins the samples_per_packet contract (see CallConfig doc).
    #[test]
    fn wrong_length_mic_frame_is_dropped() {
        let mut eng = engine(true);
        eng.start(0, 0);
        let _ = drain(&mut eng);
        let short: Vec<i16> = (0..480i32).map(|i| (i % 50) as i16 + 1).collect();
        eng.handle_input(1, Input::MicFrame(&short));
        let (outs, _) = drain(&mut eng);
        assert_eq!(
            count_transmits(&outs),
            0,
            "a non-960 mic frame must be dropped"
        );
    }

    // A Timeout fired before any deadline (the shell woke early) emits nothing and leaves the next
    // deadline unchanged -- no spurious keepalive/playout, no deadline drift, no busy-spin.
    #[test]
    fn early_timeout_is_a_noop() {
        let mut eng = engine(true);
        eng.start(0, 0);
        let _ = drain(&mut eng);
        assert_eq!(eng.poll_timeout(), Some(PLAYOUT_MS));
        eng.handle_input(5, Input::Timeout); // before the 20ms playout deadline
        let (outs, deadline) = drain(&mut eng);
        assert!(outs.is_empty(), "early timeout must emit nothing");
        assert_eq!(deadline, PLAYOUT_MS, "deadline must be unchanged");
    }

    // Characterizes playout under HARSH inter-arrival jitter (gaps beyond the cushion, which the
    // prebuffer test deliberately avoids). It pins two invariants the re-prime path must keep: buffer
    // occupancy (latency) never exceeds PLAYOUT_CAP, and the stream recovers to real audio once
    // arrivals stabilize. (The ~120ms re-prime pause per underrun is a known tuning cost of the
    // 2-frame prebuffer target -- a separate, deliberate audio trade-off, not asserted here.)
    #[test]
    fn playout_under_harsh_jitter_stays_bounded_and_recovers() {
        let mut buf: VecDeque<i16> = VecDeque::new();
        let mut priming = true;
        let mut priming_ticks = 0u32;
        let mut max_occupancy = 0usize;
        // Phase 1: sparse arrivals with gaps up to ~6 ticks (well beyond the cushion) -> underruns.
        let arrivals = [0usize, 6, 13, 20, 27];
        for t in 0..32 {
            if arrivals.contains(&t) {
                feed_frame(&mut buf);
            }
            let _ = drain_playout(&mut buf, &mut priming, &mut priming_ticks);
            max_occupancy = max_occupancy.max(buf.len());
        }
        assert!(
            max_occupancy <= PLAYOUT_CAP,
            "latency must stay bounded by the cap; peaked at {max_occupancy}"
        );
        // Phase 2: steady arrivals every 3rd tick -> playout must recover to real (non-silent) audio.
        let mut recovered = false;
        for t in 0..30 {
            if t % 3 == 0 {
                feed_frame(&mut buf);
            }
            if drain_playout(&mut buf, &mut priming, &mut priming_ticks)
                .iter()
                .any(|&s| s != 0)
            {
                recovered = true;
            }
        }
        assert!(
            recovered,
            "playout must recover to real audio once arrivals stabilize"
        );
    }

    // Bounded re-prime flush: after priming re-arms, if the peer sends a single 60ms frame and then
    // goes DTX, the buffer stalls below PLAYOUT_TARGET. Playout must flush that frame after
    // MAX_PRIME_TICKS instead of holding it (silent) forever or replaying it stale much later.
    #[test]
    fn priming_flushes_partial_buffer_after_bounded_wait() {
        let mut buf: VecDeque<i16> = VecDeque::new();
        let mut priming = true;
        let mut priming_ticks = 0u32;
        feed_frame(&mut buf); // one 60ms frame (960) < PLAYOUT_TARGET (1920), then nothing (DTX)
        // Up to MAX_PRIME_TICKS the partial buffer is held: silence, no drain.
        for _ in 0..MAX_PRIME_TICKS {
            let f = drain_playout(&mut buf, &mut priming, &mut priming_ticks);
            assert!(f.iter().all(|&s| s == 0), "still priming -> silence");
            assert_eq!(
                buf.len(),
                960,
                "the partial frame is held while priming, not drained"
            );
        }
        // The next tick hits the bound and flushes the held frame as real audio.
        let flushed = drain_playout(&mut buf, &mut priming, &mut priming_ticks);
        assert!(
            flushed.iter().any(|&s| s != 0),
            "the partial buffer must flush to real audio after the bounded wait"
        );
        assert!(
            buf.len() < 960,
            "the held frame was drained, not stalled forever"
        );
    }

    // The priming timeout must NOT age during initial silence / a DTX gap (empty buffer), or the
    // first frame after a long silence would flush instantly with no cushion. After 2*MAX ticks of
    // empty-buffer priming, one frame must still wait for the cushion instead of flushing.
    #[test]
    fn priming_timeout_does_not_age_on_an_empty_buffer() {
        let mut buf: VecDeque<i16> = VecDeque::new();
        let mut priming = true;
        let mut priming_ticks = 0u32;
        for _ in 0..(MAX_PRIME_TICKS * 2) {
            let f = drain_playout(&mut buf, &mut priming, &mut priming_ticks);
            assert!(f.iter().all(|&s| s == 0), "empty buffer -> silence");
        }
        // First frame arrives: must NOT flush instantly -- the counter didn't age while empty.
        feed_frame(&mut buf);
        let f = drain_playout(&mut buf, &mut priming, &mut priming_ticks);
        assert!(
            f.iter().all(|&s| s == 0),
            "one frame is below the target -> still priming, no instant flush"
        );
        assert_eq!(buf.len(), 960, "the first frame is held for the cushion");
        // The second frame reaches the target -> real audio drains.
        feed_frame(&mut buf);
        let f = drain_playout(&mut buf, &mut priming, &mut priming_ticks);
        assert!(
            f.iter().any(|&s| s != 0),
            "at the target playout starts real audio"
        );
    }

    /// A mirrored peer engine's video plane (its self LID = our peer LID), used to craft real
    /// inbound video packets for demux tests.
    fn peer_video_pipe() -> crate::voip::session::VideoPipeline {
        use crate::voip::session::{VideoPipeline, VideoPipelineParams};
        let call_key: Vec<u8> = (0u8..32).collect();
        VideoPipeline::new(&VideoPipelineParams {
            call_key: &call_key,
            self_lid: PEER_LID,
            peer_lid: SELF_LID,
            ssrc: ssrc::derive_video_participant_ssrc(
                "CID",
                &ssrc::format_e2e_srtp_participant_id(PEER_LID),
            ),
            ts_stride: VIDEO_TS_STRIDE_15FPS,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap()
    }

    /// A synthetic Annex-B AU with one large IDR NAL (forces FU-A fragmentation).
    fn video_au(nal_len: usize) -> Vec<u8> {
        let mut au = vec![0, 0, 0, 1, 0x65];
        au.extend((0..nal_len).map(|i| (i % 251) as u8));
        au
    }

    fn video_delta_au(nal_len: usize) -> Vec<u8> {
        let mut au = vec![0, 0, 0, 1, 0x41];
        au.extend((0..nal_len).map(|i| (i % 251) as u8));
        au
    }

    #[test]
    fn video_frame_dropped_when_video_disabled() {
        let mut eng = engine(true); // enable_video: false
        eng.start(0, 0);
        let _ = drain(&mut eng);
        assert!(!eng.is_video_enabled());
        eng.handle_input(1, Input::VideoFrame(&video_au(100)));
        let (outs, _) = drain(&mut eng);
        assert_eq!(
            count_transmits(&outs),
            0,
            "an AU with video off must not transmit"
        );
    }

    #[test]
    fn video_from_start_transmits_pt97_packets() {
        let mut cfg = config(true);
        cfg.enable_video = true;
        let mut eng = CallEngine::new(cfg, Box::new(SequentialTxIds::new())).unwrap();
        assert!(eng.is_video_enabled());
        eng.start(0, 0);
        let _ = drain(&mut eng);
        eng.handle_input(1, Input::VideoFrame(&video_delta_au(300)));
        assert_eq!(
            count_transmits(&drain(&mut eng).0),
            0,
            "from-start video must wait for a decoder-safe IDR"
        );
        eng.handle_input(1, Input::VideoFrame(&video_au(3000)));
        let (outs, _) = drain(&mut eng);
        let transmits: Vec<&Bytes> = outs
            .iter()
            .filter_map(|o| match o {
                Output::Transmit(b) => Some(b),
                _ => None,
            })
            .collect();
        assert!(
            transmits.len() >= 4,
            "a 3KB AU must fan out into FU-A packets"
        );
        for b in &transmits {
            let h = parse_rtp_header(b).expect("valid RTP header");
            assert_eq!(h.payload_type, RTP_PAYLOAD_TYPE_H264);
        }
    }

    #[test]
    fn enable_video_mid_call_then_disable() {
        let mut eng = engine(true);
        eng.start(0, 0);
        let _ = drain(&mut eng);
        let au = video_au(200);
        // Off: dropped.
        eng.handle_input(1, Input::VideoFrame(&au));
        assert_eq!(count_transmits(&drain(&mut eng).0), 0);
        // Upgrade: dependent frames wait for the first IDR.
        assert!(eng.enable_video());
        assert!(eng.enable_video(), "enable_video must be idempotent");
        eng.handle_input(2, Input::VideoFrame(&video_delta_au(200)));
        assert_eq!(count_transmits(&drain(&mut eng).0), 0);
        eng.handle_input(2, Input::VideoFrame(&au));
        assert_eq!(count_transmits(&drain(&mut eng).0), 1);
        // Downgrade: dropped again, audio untouched.
        eng.disable_video();
        assert!(!eng.is_video_enabled());
        eng.handle_input(3, Input::VideoFrame(&au));
        assert_eq!(count_transmits(&drain(&mut eng).0), 0);
        eng.handle_input(4, Input::MicFrame(&[0i16; SAMPLES as usize]));
        assert_eq!(
            count_transmits(&drain(&mut eng).0),
            1,
            "audio DTX must survive a video downgrade"
        );
    }

    // SECURITY: a downgrade must PRESERVE the video SRTP send state, so a re-upgrade never repeats
    // an (SSRC, ROC, seq) triple under the same key — that would repeat the AES-CTR keystream (a
    // two-time pad). Pin it by checking the RTP sequence number strictly increases across the
    // disable→enable cycle instead of resetting to 0.
    #[test]
    fn re_enabling_video_does_not_reset_the_srtp_packet_index() {
        let mut eng = engine(true);
        assert!(eng.enable_video());
        eng.start(0, 0);
        let _ = drain(&mut eng);

        let seq_of = |outs: &[Output]| -> Vec<u16> {
            outs.iter()
                .filter_map(|o| match o {
                    Output::Transmit(b) => parse_rtp_header(b)
                        .filter(|h| h.payload_type == RTP_PAYLOAD_TYPE_H264)
                        .map(|h| h.sequence_number),
                    _ => None,
                })
                .collect()
        };

        // First video AU: single NAL -> one packet at seq 0.
        eng.handle_input(1, Input::VideoFrame(&video_au(100)));
        let (outputs, _) = drain(&mut eng);
        let first = seq_of(&outputs);
        assert_eq!(first, vec![0]);

        // Downgrade then re-upgrade: the plane is preserved, so the next packet's seq CONTINUES.
        eng.disable_video();
        assert!(!eng.is_video_enabled());
        eng.handle_input(2, Input::VideoFrame(&video_au(100))); // dropped while inactive
        assert!(seq_of(&drain(&mut eng).0).is_empty());
        assert!(eng.enable_video());
        eng.handle_input(3, Input::VideoFrame(&video_delta_au(100)));
        assert!(seq_of(&drain(&mut eng).0).is_empty());
        eng.handle_input(3, Input::VideoFrame(&video_au(100)));
        let after = seq_of(&drain(&mut eng).0);
        assert_eq!(
            after,
            vec![1],
            "re-enabled video must continue the sequence, not reset to 0 (keystream reuse)"
        );
    }

    #[test]
    fn rejected_idr_packetization_keeps_the_recovery_gate_armed() {
        let mut eng = engine(true);
        assert!(eng.enable_video());
        eng.start(0, 0);
        let _ = drain(&mut eng);

        let oversized = video_au(crate::voip::h264::H264_MAX_AU_BYTES);
        eng.handle_input(1, Input::VideoFrame(&oversized));
        assert_eq!(count_transmits(&drain(&mut eng).0), 0);

        eng.handle_input(2, Input::VideoFrame(&video_delta_au(100)));
        assert_eq!(
            count_transmits(&drain(&mut eng).0),
            0,
            "a rejected IDR must not admit dependent frames"
        );

        eng.handle_input(3, Input::VideoFrame(&video_au(100)));
        assert_eq!(count_transmits(&drain(&mut eng).0), 1);
    }

    // The upgrade initiator holds outbound video until the peer accepts: a send-gated plane decodes
    // inbound but transmits nothing, and a later ungating `enable_video` starts transmission.
    #[test]
    fn send_gated_video_plane_holds_outbound_until_ungated() {
        let mut eng = engine(true);
        assert!(eng.enable_video_gated());
        assert!(eng.is_video_enabled(), "a gated plane is still 'enabled'");
        eng.start(0, 0);
        let _ = drain(&mut eng);

        // Gated: local AUs are dropped (no PT-97 on the wire).
        eng.handle_input(1, Input::VideoFrame(&video_au(200)));
        assert_eq!(
            count_transmits(&drain(&mut eng).0),
            0,
            "a send-gated plane must not transmit our video"
        );
        // Inbound still decodes while gated.
        let mut peer = peer_video_pipe();
        for p in peer.protect_video(&video_au(120)) {
            eng.handle_input(1, Input::RelayPacket(&p));
        }
        assert!(
            drain(&mut eng)
                .0
                .iter()
                .any(|o| matches!(o, Output::VideoPlayout(_))),
            "a gated plane must still decode inbound video"
        );

        // Peer accepted -> deltas remain withheld until a decoder-safe IDR.
        assert!(eng.enable_video());
        eng.handle_input(2, Input::VideoFrame(&video_delta_au(200)));
        assert_eq!(
            count_transmits(&drain(&mut eng).0),
            0,
            "ungating must not start with a delta whose references were gated"
        );
        eng.handle_input(3, Input::VideoFrame(&video_au(200)));
        assert_eq!(
            count_transmits(&drain(&mut eng).0),
            1,
            "the next IDR resumes outbound video"
        );
    }

    #[test]
    fn enable_video_fails_without_media_plane() {
        let mut eng = engine(false); // control-plane only
        assert!(!eng.enable_video(), "no media plane -> no video plane");
        assert!(!eng.is_video_enabled());
    }

    #[test]
    fn inbound_video_reassembles_into_video_playout() {
        let mut eng = engine(true);
        assert!(eng.enable_video());
        eng.start(0, 0);
        let _ = drain(&mut eng);
        eng.set_peer_video_orientation(2);

        let mut peer = peer_video_pipe();
        let au = video_au(3000);
        let packets = peer.protect_video(&au);
        assert!(packets.len() >= 4);
        let mut frames = Vec::new();
        for p in &packets {
            eng.handle_input(1, Input::RelayPacket(p));
            let (outs, _) = drain(&mut eng);
            frames.extend(outs.into_iter().filter_map(|o| match o {
                Output::VideoPlayout(f) => Some(f),
                _ => None,
            }));
        }
        assert_eq!(frames.len(), 1, "N packets must reassemble into 1 AU");
        assert_eq!(frames[0].data, au);
        assert!(frames[0].keyframe, "IDR AU must be flagged as keyframe");
        assert_eq!(frames[0].orientation, 2);
        assert_eq!(
            eng.jitter_len(),
            0,
            "video must not leak into the audio jitter buffer"
        );
    }

    #[test]
    fn inbound_video_rejects_forged_warp_tag() {
        let mut eng = engine(true);
        assert!(eng.enable_video());
        eng.start(0, 0);
        let _ = drain(&mut eng);

        let mut peer = peer_video_pipe();
        let packet = peer
            .protect_video(&video_au(100))
            .pop()
            .expect("single-packet video AU");

        let mut forged = packet.clone();
        *forged.last_mut().expect("WARP tag") ^= 1;
        eng.handle_input(1, Input::RelayPacket(&forged));
        assert!(
            !drain(&mut eng)
                .0
                .iter()
                .any(|output| matches!(output, Output::VideoPlayout(_))),
            "unauthenticated RTP must not reach playout"
        );

        eng.handle_input(2, Input::RelayPacket(&packet));
        assert!(
            drain(&mut eng)
                .0
                .iter()
                .any(|output| matches!(output, Output::VideoPlayout(_))),
            "authenticated RTP must reach playout"
        );
    }

    #[test]
    fn inbound_video_dropped_when_video_disabled_and_audio_unaffected() {
        let mut eng = engine(true); // video off
        eng.start(0, 0);
        let _ = drain(&mut eng);
        let mut peer = peer_video_pipe();
        for p in peer.protect_video(&video_au(500)) {
            eng.handle_input(1, Input::RelayPacket(&p));
        }
        let (outs, _) = drain(&mut eng);
        assert!(
            !outs
                .iter()
                .any(|o| matches!(o, Output::VideoPlayout(_) | Output::Event(_))),
            "PT-97 with video off must be silently dropped"
        );
        // Audio still decodes (demux must not eat Opus packets).
        let call_key: Vec<u8> = (0u8..32).collect();
        let mut peer_audio = MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid: PEER_LID,
            peer_lid: SELF_LID,
            ssrc: SSRC,
            samples_per_packet: SAMPLES,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap();
        let mut enc = MlowEncoder::new();
        let tone: Vec<f32> = (0..SAMPLES as usize)
            .map(|i| 0.3 * (i as f32 * 0.07).sin())
            .collect();
        for _ in 0..2 {
            let pkt = peer_audio.protect_audio(&enc.encode(&tone).unwrap());
            eng.handle_input(2, Input::RelayPacket(&pkt));
        }
        let _ = drain(&mut eng);
        assert!(eng.jitter_len() > 0, "audio path must keep decoding");
    }

    #[test]
    fn rekey_recv_also_rekeys_the_video_plane() {
        let mut eng = engine(true);
        assert!(eng.enable_video());
        eng.start(0, 0);
        let _ = drain(&mut eng);

        let call_key: Vec<u8> = (0u8..32).collect();
        let answering = "222222222222222:2@lid";
        let mut answerer =
            crate::voip::session::VideoPipeline::new(&crate::voip::session::VideoPipelineParams {
                call_key: &call_key,
                self_lid: answering,
                peer_lid: SELF_LID,
                ssrc: ssrc::derive_video_participant_ssrc(
                    "CID",
                    &ssrc::format_e2e_srtp_participant_id(answering),
                ),
                ts_stride: VIDEO_TS_STRIDE_15FPS,
                warp_mi_tag_len: WARP_MI_TAG_LEN,
            })
            .unwrap();

        let au = video_au(120);
        for p in answerer.protect_video(&au) {
            eng.handle_input(1, Input::RelayPacket(&p));
        }
        let (outs, _) = drain(&mut eng);
        assert!(
            !outs.iter().any(|o| matches!(o, Output::VideoPlayout(_))),
            "pre-rekey: companion-keyed video must not decode"
        );

        assert!(eng.rekey_recv(answering));
        for p in answerer.protect_video(&au) {
            eng.handle_input(2, Input::RelayPacket(&p));
        }
        let (outs, _) = drain(&mut eng);
        assert!(
            outs.iter()
                .any(|o| matches!(o, Output::VideoPlayout(f) if f.data == au)),
            "post-rekey: the answering device's video must decode"
        );
    }

    #[test]
    fn video_enabled_after_rekey_keys_recv_from_answering_device() {
        // Upgrade AFTER the answering device is known: the late-built video pipeline must key its
        // recv path from the CURRENT (rekeyed) peer LID, not the stale dialed base.
        let mut eng = engine(true);
        eng.start(0, 0);
        let _ = drain(&mut eng);
        let answering = "222222222222222:2@lid";
        assert!(eng.rekey_recv(answering));
        assert!(eng.enable_video(), "upgrade after rekey");

        let call_key: Vec<u8> = (0u8..32).collect();
        let mut answerer =
            crate::voip::session::VideoPipeline::new(&crate::voip::session::VideoPipelineParams {
                call_key: &call_key,
                self_lid: answering,
                peer_lid: SELF_LID,
                ssrc: ssrc::derive_video_participant_ssrc(
                    "CID",
                    &ssrc::format_e2e_srtp_participant_id(answering),
                ),
                ts_stride: VIDEO_TS_STRIDE_15FPS,
                warp_mi_tag_len: WARP_MI_TAG_LEN,
            })
            .unwrap();
        let au = video_au(80);
        for p in answerer.protect_video(&au) {
            eng.handle_input(1, Input::RelayPacket(&p));
        }
        let (outs, _) = drain(&mut eng);
        assert!(
            outs.iter()
                .any(|o| matches!(o, Output::VideoPlayout(f) if f.data == au)),
            "a video plane built after rekey must decode the answering device"
        );
    }

    // An inbound Allocate-error must surface exactly one terminal RelayAllocateFailed carrying the
    // STUN error code, and not mark the call allocated.
    #[test]
    fn allocate_error_emits_failed_event_with_code() {
        let mut eng = engine(true);
        eng.start(0, 0);
        let _ = drain(&mut eng);
        let err = allocate_error(486); // class 4, number 86
        eng.handle_input(1, Input::RelayPacket(&err));
        let (outs, _) = drain(&mut eng);
        assert_eq!(
            outs.iter()
                .filter(|o| matches!(o, Output::Event(CallEvent::RelayAllocateFailed(486))))
                .count(),
            1,
            "one RelayAllocateFailed carrying the error code"
        );
        assert!(!eng.is_allocated(), "a rejected allocate is not allocated");
    }

    // An allocate-error is terminal: the engine goes inert, so a subsequent Timeout far past the
    // keepalive deadline produces ZERO further transmits (the keepalive stopped, not a dead-relay
    // keepalive forever) and poll_timeout reports no timer.
    #[test]
    fn malformed_stun_success_does_not_cancel_the_allocate_timeout() {
        let mut eng = engine(true);
        eng.start(0, 0);
        let _ = drain(&mut eng);
        // A success-typed packet without the STUN magic cookie must not mark us allocated or cancel
        // the allocate timeout, else a garbage packet keeps a wedged relay in indefinite keepalive.
        let mut garbage =
            stun::encode_stun_request(stun::MSG_BINDING_SUCCESS, &[3u8; 12], &[], None, false);
        garbage[4] ^= 0xff; // corrupt the magic cookie
        eng.handle_input(1, Input::RelayPacket(&garbage));
        let (outs, _) = drain(&mut eng);
        assert!(
            !eng.is_allocated(),
            "a malformed success must not mark allocated"
        );
        assert!(
            !outs
                .iter()
                .any(|o| matches!(o, Output::Event(CallEvent::RelayAllocated))),
            "a malformed success must not emit RelayAllocated"
        );
        // The allocate timeout safety net is intact.
        eng.handle_input(ALLOCATE_TIMEOUT_MS + 1, Input::Timeout);
        let (outs, _) = drain(&mut eng);
        assert!(
            outs.iter()
                .any(|o| matches!(o, Output::Event(CallEvent::RelayAllocateTimedOut))),
            "the allocate timeout must still fire after a malformed success"
        );
    }

    #[test]
    fn garbage_stun_does_not_terminate_the_call() {
        let mut eng = engine(true);
        eng.start(0, 0);
        let _ = drain(&mut eng);
        // Dropping the ERROR-CODE TLV leaves the body-length header still claiming the full body, so
        // the packet is rejected as INCOMPLETE (fails is_complete_stun), not as a parseable error. A
        // garbage relay packet must not be treated as a terminal failure that hangs up the call.
        let full = allocate_error(486);
        let garbage = &full[..full.len() - 8]; // drop the ERROR-CODE TLV, keep the message type
        eng.handle_input(1, Input::RelayPacket(garbage));
        let (outs, _) = drain(&mut eng);
        assert!(
            !eng.is_terminated(),
            "garbage STUN must not terminate the call"
        );
        assert!(
            !outs
                .iter()
                .any(|o| matches!(o, Output::Event(CallEvent::RelayAllocateFailed(_)))),
            "garbage STUN must not emit RelayAllocateFailed"
        );
    }

    #[test]
    fn allocate_error_terminates_and_stops_keepalive() {
        let mut eng = engine(true);
        eng.start(0, 0);
        let _ = drain(&mut eng);
        eng.handle_input(1, Input::RelayPacket(&allocate_error(486)));
        let _ = drain(&mut eng);
        assert!(eng.is_terminated(), "an allocate-error is terminal");
        assert_eq!(eng.poll_timeout(), None, "no timer once terminated");
        // Far past every deadline: the keepalive must not fire.
        eng.handle_input(100 * KEEPALIVE_MS, Input::Timeout);
        let (outs, _) = drain(&mut eng);
        assert_eq!(
            count_transmits(&outs),
            0,
            "a terminated engine must emit no further transmits"
        );
    }

    // Same for the allocate-timeout path: once it fires the engine is terminal, so a later Timeout
    // emits no keepalive.
    #[test]
    fn allocate_timeout_terminates_and_stops_keepalive() {
        let mut eng = engine(true);
        eng.start(0, 0);
        let _ = drain(&mut eng);
        eng.handle_input(ALLOCATE_TIMEOUT_MS, Input::Timeout);
        let (outs, _) = drain(&mut eng);
        assert_eq!(
            outs.iter()
                .filter(|o| matches!(o, Output::Event(CallEvent::RelayAllocateTimedOut)))
                .count(),
            1,
            "the terminal timeout event is delivered before going inert"
        );
        assert!(eng.is_terminated(), "the allocate-timeout is terminal");
        assert_eq!(eng.poll_timeout(), None, "no timer once terminated");
        eng.handle_input(ALLOCATE_TIMEOUT_MS + 100 * KEEPALIVE_MS, Input::Timeout);
        let (outs, _) = drain(&mut eng);
        assert_eq!(
            count_transmits(&outs),
            0,
            "a terminated engine must emit no further transmits"
        );
    }

    // With no allocate ack, driving Timeouts past ALLOCATE_TIMEOUT_MS must emit exactly ONE
    // RelayAllocateTimedOut and none after (the deadline fires once, then is cleared).
    #[test]
    fn allocate_timeout_fires_exactly_once() {
        let mut eng = engine(true);
        eng.start(0, 0);
        let _ = drain(&mut eng);
        eng.handle_input(ALLOCATE_TIMEOUT_MS, Input::Timeout);
        let (outs, _) = drain(&mut eng);
        assert_eq!(
            outs.iter()
                .filter(|o| matches!(o, Output::Event(CallEvent::RelayAllocateTimedOut)))
                .count(),
            1,
            "one terminal timeout at the deadline"
        );
        // Drive well past the deadline again: no second timeout event.
        eng.handle_input(ALLOCATE_TIMEOUT_MS + 5 * KEEPALIVE_MS, Input::Timeout);
        let (outs, _) = drain(&mut eng);
        assert_eq!(
            outs.iter()
                .filter(|o| matches!(o, Output::Event(CallEvent::RelayAllocateTimedOut)))
                .count(),
            0,
            "the timeout must not re-fire"
        );
    }

    // A successful allocate before the deadline emits RelayAllocated and stops the timer, so driving
    // Timeouts past ALLOCATE_TIMEOUT_MS yields no RelayAllocateTimedOut.
    #[test]
    fn allocate_success_cancels_the_timeout() {
        let mut eng = engine(true);
        eng.start(0, 0);
        let _ = drain(&mut eng);
        let ok =
            stun::encode_stun_request(stun::MSG_ALLOCATE_SUCCESS, &[1u8; 12], &[], None, false);
        eng.handle_input(1, Input::RelayPacket(&ok));
        let (outs, _) = drain(&mut eng);
        assert_eq!(
            outs.iter()
                .filter(|o| matches!(o, Output::Event(CallEvent::RelayAllocated)))
                .count(),
            1
        );
        assert!(eng.is_allocated());
        // Past the deadline: no timeout, the success already stopped the timer.
        eng.handle_input(ALLOCATE_TIMEOUT_MS + KEEPALIVE_MS, Input::Timeout);
        let (outs, _) = drain(&mut eng);
        assert_eq!(
            outs.iter()
                .filter(|o| matches!(o, Output::Event(CallEvent::RelayAllocateTimedOut)))
                .count(),
            0,
            "a successful allocate must cancel the timeout"
        );
    }
}

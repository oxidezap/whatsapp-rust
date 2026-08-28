//! The sans-IO call engine: a str0m-shaped state machine that owns the relay control plane (STUN
//! allocate, 1s keepalive, consent-freshness replies) and, optionally, the media plane (encoded
//! audio or PCM MLOW + E2E-SRTP + SFrame + a playout jitter buffer). It owns no socket, no clock,
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

use std::collections::{HashMap, HashSet, VecDeque};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use bytes::Bytes;

use super::app_data;
use super::audio::{
    AudioCodec, AudioConfig, AudioFormat, AudioIo, AudioRtpProfile, EncodedAudioFrame,
    ForeignAudioCodec, ForeignAudioCodecFactory, depacketize_opus_from_mlow,
};
use super::codec_probe::InboundCodecProbe;
use super::demux::{RelayPacketKind, classify_relay_packet, unwrap_group_forwarding_packet};
use super::group_audio::ParticipantAudioMixer;
use super::group_media::{
    GroupAudioReject, GroupEpochApply, GroupMediaError, GroupMediaRegistry, GroupMediaStream,
    GroupRosterApply, group_device_is_local,
};
use super::h264::{VideoFrame, au_has_idr, au_is_keyframe};
use super::media_stats::{
    AudioHealthAlarm, AudioHealthWatch, AudioSilenceReason, CODEC_FLAP_LIMIT, CallMediaStats,
};
#[cfg(feature = "voip-mlow")]
use super::mlow;
use super::rtcp::{
    RTCP_PT_PSFB, RtcpFeedback, RtcpReportBlock, RtpReceptionStats, build_whatsapp_rtcp_cname,
    parse_sender_report_timing, summarize_rtcp,
};
#[cfg(feature = "voip-mlow")]
use super::rtp::RTP_PAYLOAD_TYPE_MLOW_RED;
use super::rtp::{
    RTP_PAYLOAD_TYPE_APP_DATA, RTP_PAYLOAD_TYPE_H264, VIDEO_CLOCK_RATE, VIDEO_TS_STRIDE_15FPS,
    parse_rtp_header,
};
use super::session::{
    CallDirection, MediaPipeline, MediaPipelineParams, VideoPipeline, VideoPipelineParams,
};
use super::sframe::{SframeIn, SframeSession};
use super::{ssrc, stun};
use crate::types::group_call::{GroupCallRelay, GroupCallUpdate, ScreenShare, WaitingRoom};
use wacore_binary::Jid;
use zeroize::Zeroize;

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
pub(crate) const PLAYOUT_MS: Millis = 20;
const APP_DATA_RETRANSMIT_MS: Millis = 50;
const APP_DATA_RETRANSMIT_COUNT: u8 = 10;
const MAX_PENDING_REACTIONS: usize = 64;
/// 20ms @ 16kHz: samples drained to the speaker per playout tick.
const PLAYOUT_DRAIN: usize = 320;
/// 60ms @ 16kHz: the peer packet size the playout constants were written for, and the assumption
/// used until the first decode reports what the peer actually sends.
const OPUS_FRAME_SAMPS_60MS: usize = 960;
/// ~150ms latency ceiling for a 60ms peer frame; a burst past this resyncs (drops oldest) instead
/// of lagging. The floor for [`playout_bounds`], which scales it to the peer's packet.
const PLAYOUT_CAP: usize = 2400;
/// Prebuffer target: prime playout until the jitter buffer holds two 60ms peer frames, so the
/// steady-state buffer never drains below one frame (a 60ms cushion that absorbs the relay's
/// inter-arrival jitter). Priming to a single frame is a zero cushion: that one frame drains away
/// over its own 60ms cycle, so the buffer returns to empty before the next packet and any late
/// arrival underruns. The cushion has to be one frame above what the per-cycle drain consumes.
const PLAYOUT_TARGET: usize = 1920;

/// Prime target and latency ceiling for a peer sending `packet_samps`-sample packets.
///
/// The constants above assume a 60ms peer frame, which held while the decoder only produced those.
/// A 120ms packet is a full [`PLAYOUT_TARGET`] on its own, so priming would end on the first one
/// with no cushion at all, and two in flight would exceed [`PLAYOUT_CAP`] and be trimmed on arrival.
/// Keep the same shape instead: prime to two packets so the steady-state buffer never drains below
/// one, and let the ceiling hold that cushion plus a drain slice.
fn playout_bounds(packet_samps: usize) -> (usize, usize) {
    let target = PLAYOUT_TARGET.max(packet_samps.saturating_mul(2));
    (target, PLAYOUT_CAP.max(target + PLAYOUT_DRAIN))
}

/// The ceiling to enforce now, given the one in force, the peer's current packet and what is queued.
///
/// It rises with the packet immediately. Falling is deliberately gradual: a stream dropping to a
/// shorter packet (a genuine switch, or the SID that DTX canonicalizes to) would otherwise trim, in
/// one go, audio that was legally queued under the previous bound and has not been played. The
/// ceiling exists to bound latency under a burst, not to punish a change of packet size.
///
/// Gradual, though, not conditional on the backlog draining by itself -- it does not. In a steady
/// stream each packet adds exactly what playout removes, so a backlog left above the new ceiling
/// stays there and the old one (up to ~260 ms of pure latency at 120 ms packets) would hold for the
/// rest of the call. Instead it gives up one packet's worth per packet until it reaches the target:
/// the trim that follows discards at most one packet of the OLDEST queued audio at a time, and a
/// few packets later the call is back to the latency its current cadence asks for.
fn effective_playout_cap(current: usize, packet_samps: usize, queued: usize) -> usize {
    let want = playout_bounds(packet_samps).1;
    if want >= current || queued <= want {
        want
    } else {
        current.saturating_sub(packet_samps.max(1)).max(want)
    }
}
/// Bound on how long playout primes before flushing a partial buffer: if the peer sends one frame
/// then goes DTX the jitter buffer never reaches `PLAYOUT_TARGET`, so after this many 20ms ticks
/// (~200ms) drain whatever is queued instead of holding it (silent) forever. Comfortably above the
/// few ticks a normal jittered second-frame arrival takes, so it never trips in steady operation.
const MAX_PRIME_TICKS: u32 = 10;
/// One-byte mlow DTX comfort-noise token sent on a muted (exact-zero) mic frame so the media stream
/// never gaps; protect_audio frames it with the DTX RTP header and the peer decodes it to silence.
#[cfg(feature = "voip-mlow")]
const MLOW_DTX_CNG: [u8; 1] = [0x90];

/// One 60ms frame at 16kHz. `Input::MicFrame` must carry exactly this; a wrong-length buffer is
/// dropped (either encoder requires it), never sent. Both halves of the swappable pair share the
/// cadence, which is what lets a call switch between them without re-signalling.
const MIC_FRAME_SAMPLES: usize = 960;
#[cfg(feature = "voip-mlow")]
const MLOW_ENCODED_CAPACITY: usize = 513;
const MAX_INVALID_AUDIO_WARNINGS: u8 = 3;

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
    /// Codec, timing, and whether the engine sees PCM or complete encoded payloads.
    pub audio: AudioConfig,
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

/// Group-media inputs layered onto a regular engine before it starts.
pub struct GroupEngineConfig {
    pub call_creator: Jid,
    pub self_jid: Jid,
    pub initial_update: GroupCallUpdate,
    /// Present when a live direct call is upgraded in place. The existing
    /// authenticated receiver stays active until a PID-bearing roster arrives.
    pub direct_peer: Option<DirectPeer>,
}

/// Authenticated direct-call participant retained during an in-place group promotion.
pub struct DirectPeer {
    pub user_jid: Jid,
    pub device_jid: Jid,
    pub call_key: Vec<u8>,
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
            .field("audio", &self.audio)
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
    #[error("audio format contains a zero timing or channel value")]
    BadAudioFormat,
    #[error("PCM audio is supported only for mono 16 kHz / 60 ms MLOW or standard Opus")]
    UnsupportedPcmAudio,
    #[error("PCM MLOW audio requires the `voip-mlow` feature")]
    MlowUnavailable,
    #[error("group media setup failed: {0}")]
    GroupMedia(#[from] GroupMediaError),
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
            audio: AudioConfig::MLOW_PCM,
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

    /// Build a native group-call engine before its shared keygen-v2 epoch arrives. Media is gated
    /// until [`CallEngine::apply_group_raw_epoch`] installs an authenticated epoch, so the zeroed
    /// bootstrap key is never permitted onto the wire.
    pub fn for_group(
        direction: CallDirection,
        call_id: &str,
        self_lid: &str,
        call_creator: &str,
        relay: &GroupCallRelay,
    ) -> Result<Self, SetupError> {
        let endpoint = get_group_media_relay_endpoint(relay).ok_or(SetupError::NoRelayEndpoint)?;
        let relay_ip = endpoint.ipv4.clone().ok_or(SetupError::NoRelayIpv4)?;
        let relay_port = endpoint.port.ok_or(SetupError::NoRelayEndpoint)?;
        let relay_token = relay
            .tokens
            .get(endpoint.token_id as usize)
            .filter(|token| !token.is_empty())
            .cloned()
            .ok_or(SetupError::NoRelayToken(endpoint.token_id))?;
        if relay.key.is_empty() {
            return Err(SetupError::NoIntegrityKey);
        }
        let warp_mi_tag_len = relay
            .warp_mi_tag_len
            .map(|value| value as usize)
            .unwrap_or(super::warp::WARP_MI_TAG_LEN);
        if !(1..=20).contains(&warp_mi_tag_len) {
            return Err(SetupError::BadWarpMiTagLen(warp_mi_tag_len));
        }
        Ok(Self {
            call_id: call_id.to_string(),
            direction,
            self_lid: self_lid.to_string(),
            peer_lid: call_creator.to_string(),
            call_key: vec![0; 32],
            ssrc: ssrc::derive_wasm_participant_ssrc(
                call_id,
                &ssrc::format_e2e_srtp_participant_id(self_lid),
                0,
            ),
            audio: AudioConfig::MLOW_PCM,
            relay_token,
            relay_ip,
            relay_port,
            integrity_key: relay.key.clone(),
            warp_mi_tag_len,
            enable_media: true,
            enable_video: false,
            enable_sframe: false,
        })
    }
}

/// One input to the engine, applied with the current monotonic timestamp.
pub enum Input<'a> {
    /// A packet arrived on the relay channel (one DataChannel/datagram message).
    RelayPacket(&'a [u8]),
    /// A 60ms PCM frame captured from the local mic (16kHz mono). Must be exactly 960 samples (the
    /// MLow frame size); a wrong-length frame is silently dropped by the encoder (no RTP sent).
    MicFrame(&'a [i16]),
    /// One complete payload produced by the codec selected in [`CallConfig::audio`]. The engine
    /// adds RTP, SRTP, and WARP framing without inspecting or transcoding it.
    EncodedAudio(&'a [u8]),
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
    /// A decrypted codec payload for an encoded audio sink.
    EncodedAudio(EncodedAudioFrame),
    /// A reassembled peer access unit for the video sink. Dedicated output (not a CallEvent) for
    /// the same reason audio uses `Playout`: the event channel sheds on overflow, media must not.
    VideoPlayout(VideoFrame),
    /// A call lifecycle / diagnostic event.
    Event(CallEvent),
    /// Redial media transport before sending subsequent allocation/media packets.
    ReconnectRelay(SocketAddr),
    /// Drained: arm a timer for this monotonic-ms deadline ([`NEVER`] = no timer).
    Timeout(Millis),
}

/// What decided a codec switch, so a consumer can tell parity from a rescue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CodecDecisionSource {
    /// The peer's `<capability>` said so. The normative source; this is what the official client
    /// uses and the only thing it uses.
    Negotiated,
    /// The bytes on the wire said so, and they disagreed with the negotiation. Worth surfacing:
    /// it means our model of the peer is wrong, not just that the audio was rescued.
    Content,
}

/// Why a requested codec switch was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CodecSwitchError {
    #[error("the call has no media plane")]
    NoMedia,
    #[error("only the MLOW/Opus pair that shares one RTP timing can be swapped mid-call")]
    NotASiblingFormat,
    #[error("the codec changed too many times; the decision is latched")]
    Latched,
}

/// Group-control command rejected without terminating the media driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum GroupControlKind {
    Update,
    Epoch,
    Reaction,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CallEvent {
    /// The relay accepted our allocate (an allocate/binding success arrived); media path is live.
    RelayAllocated,
    /// A standard Opus packet carried through MLOW's in-profile escape while PCM/MLOW I/O is
    /// selected. Shells with an Opus decoder can play it; codec selection still follows signaling.
    ForeignAudio(Bytes),
    /// A standard Opus fallback packet received in PCM/MLOW mode from one authenticated group
    /// participant. The participant metadata lets consumers keep one stateful decoder per sender.
    ForeignGroupAudio(EncodedAudioFrame),
    /// The peer selected signaling rates incompatible with the single profile offered locally.
    AudioFormatMismatch {
        expected_rate: u32,
        received_rates: Vec<u32>,
    },
    /// The relay rejected our allocate. Terminal; carries the STUN error code (class*100 + number).
    RelayAllocateFailed(u16),
    /// The relay never acked the allocate within the deadline (wedged relay). Terminal.
    RelayAllocateTimedOut,
    /// Replacing a migrated relay transport did not finish within the reconnect deadline.
    RelayReconnectTimedOut,
    /// The peer's `<video state=N>` signaling arrived (upgrade requested/accepted, stopped, ...).
    /// Pushed by the signaling handler, not the engine; surfaced here so one event stream carries
    /// the whole call. For an upgrade request, pass `upgrade_token` to `accept_video`; a cancelled
    /// or superseded token cannot attach video endpoints.
    VideoStateChanged {
        state: crate::types::call::VideoState,
        orientation: Option<u8>,
        /// Accepting requires this exact token. `None` means simultaneous local and peer requests
        /// were already resolved by the signaling state machine.
        upgrade_token: Option<super::VideoUpgradeToken>,
    },
    /// Outbound video needs an IDR before anything can go on the wire, and the
    /// engine cannot make one — it transports encoded access units and never
    /// touches pixels, so only the application's encoder can.
    ///
    /// Raised the moment the requirement appears, not once per dropped frame:
    /// an upgrade being ungated, a group epoch commit, a source switch, or the
    /// peer's own RTCP asking for one. Until an access unit carrying an IDR
    /// arrives, every frame handed to the engine is dropped — so a consumer
    /// that ignores this event sends nothing until its encoder's own keyframe
    /// period comes round, which for a mid-call upgrade is the difference
    /// between a picture appearing at once and appearing seconds later.
    ///
    /// The shipped client does the same thing at the same moments
    /// (`pjmedia_vid_stream_request_keyframe`, and its "requesting keyframe
    /// after dropped frames" / "will request keyframe on resume" paths).
    VideoKeyframeNeeded,
    /// A newer authoritative group membership/relay snapshot was committed.
    GroupUpdated(Box<GroupCallUpdate>),
    /// A newer authoritative call-link admission snapshot was committed.
    WaitingRoomUpdated(Box<WaitingRoom>),
    /// Repeated waiting-room heartbeats failed and the pending call-link admission was abandoned.
    WaitingRoomHeartbeatFailed,
    /// One signaling/app-data control was rejected while the call itself remained healthy.
    GroupControlRejected { control: GroupControlKind },
    /// A server-requested shared epoch could not be distributed or committed locally.
    GroupRekeyFailed,
    /// One participant raised or lowered their hand.
    HandRaised { participant: Jid, raised: bool },
    /// One participant started or stopped screen sharing.
    ScreenShareChanged {
        participant: Jid,
        screen_share: ScreenShare,
    },
    /// One authenticated, participant-attributed RTC reaction.
    Reaction {
        participant: Jid,
        device: Jid,
        pid: Option<u32>,
        /// `None` removes the participant's previous reaction.
        emoji: Option<String>,
        removed: bool,
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
    /// Audio RTP keeps arriving and none of it is becoming sound.
    ///
    /// The consumer-visible answer to the failure mode behind issue #1105: a call that connects,
    /// stays connected, and carries silence used to be indistinguishable from a peer who is not
    /// speaking. Re-emitted on a fixed cadence while the condition holds, with a monotonic
    /// `silent_for_ms`, so a truncated log still catches it. Diagnostic, never terminal.
    AudioSilent {
        silent_for_ms: Millis,
        /// Packets counted in the window that produced this alarm, not for the whole call.
        rtp_received: u32,
        frames_produced: u32,
        dominant_reason: AudioSilenceReason,
    },
    /// The payload grammar in use changed inside the negotiated RTP timing.
    ///
    /// `source: Content` means the peer's bytes contradicted its signaling, which is a statement
    /// about our model of the peer and not only about this call.
    AudioCodecSwitched {
        from: AudioCodec,
        to: AudioCodec,
        source: CodecDecisionSource,
        /// Audio packets seen before the switch, so a late switch is distinguishable from an
        /// immediate one without correlating timestamps.
        packets_observed: u32,
    },
    /// The peer speaks one codec and this call's encoded source emits another, and neither can
    /// move: the source was built by the application and carries no per-frame codec, so the engine
    /// cannot re-point it the way it re-points its own encoder.
    ///
    /// The mid-call twin of [`CallError::EncodedAudioCodecNotNegotiated`][enc], which refuses the
    /// same mismatch at answer time. Nothing is switched -- switching would keep taking the
    /// application's bytes and send them under a profile that accepts any nonempty payload, so the
    /// peer would hear noise with nothing on this side noticing. Only the application can pick a
    /// codec, so only it can end or rebuild this call.
    ///
    /// [enc]: https://docs.rs/whatsapp-rust/latest/whatsapp_rust/voip/enum.CallError.html
    AudioCodecSourceIsFixed {
        /// What the application's source emits, and what this call keeps sending.
        sending: AudioCodec,
        /// What the peer says it speaks.
        peer_expects: AudioCodec,
        /// Whether the peer said so in signaling or in its packets.
        source: CodecDecisionSource,
    },
    /// Audio RTP has stopped arriving: either none ever did since the relay allocated, or the
    /// peer's media stopped mid-call. `silent_for_ms` measures from the last packet, or from the
    /// allocate when there has been none.
    ///
    /// Deliberately distinct from [`Self::AudioSilent`]: this one is a transport problem and that
    /// one is a codec problem, and conflating them is exactly how #1105 was mis-triaged for months.
    /// Emitted once per stall -- reception recovering re-arms it, so a call that drops out twice
    /// says so twice.
    AudioReceptionStalled { silent_for_ms: Millis },
}

impl CallEvent {
    pub(crate) fn heap_bytes(&self) -> usize {
        use core::mem::size_of;

        use crate::stats::HeapSize;

        match self {
            // A unit variant: no heap behind it.
            Self::VideoKeyframeNeeded => 0,
            Self::ForeignAudio(data) => data.len(),
            Self::ForeignGroupAudio(frame) => {
                frame.data.len()
                    + frame.sender.as_ref().map_or(0, HeapSize::heap_bytes)
                    + frame.device.as_ref().map_or(0, HeapSize::heap_bytes)
            }
            Self::AudioFormatMismatch { received_rates, .. } => {
                received_rates.capacity() * size_of::<u32>()
            }
            Self::GroupUpdated(update) => size_of::<GroupCallUpdate>() + update.heap_bytes(),
            Self::WaitingRoomUpdated(room) => size_of::<WaitingRoom>() + room.heap_bytes(),
            Self::HandRaised { participant, .. } | Self::ScreenShareChanged { participant, .. } => {
                participant.heap_bytes()
            }
            Self::Reaction {
                participant,
                device,
                emoji,
                ..
            } => {
                participant.heap_bytes()
                    + device.heap_bytes()
                    + emoji.as_ref().map_or(0, String::capacity)
            }
            Self::RtcpReceived {
                packet_types,
                referenced_ssrcs,
                report_blocks,
                feedback,
                ..
            } => {
                packet_types.capacity()
                    + referenced_ssrcs.capacity() * size_of::<u32>()
                    + report_blocks.capacity() * size_of::<RtcpReportBlock>()
                    + report_blocks
                        .iter()
                        .map(|report| report.profile_extension.capacity())
                        .sum::<usize>()
                    + feedback.capacity() * size_of::<RtcpFeedback>()
                    + feedback
                        .iter()
                        .map(|item| item.fci.capacity())
                        .sum::<usize>()
            }
            Self::RelayAllocated
            | Self::RelayAllocateFailed(_)
            | Self::RelayAllocateTimedOut
            | Self::RelayReconnectTimedOut
            | Self::VideoStateChanged { .. }
            | Self::WaitingRoomHeartbeatFailed
            | Self::GroupControlRejected { .. }
            | Self::GroupRekeyFailed
            | Self::OutboundMediaDropped { .. }
            | Self::AudioSilent { .. }
            | Self::AudioCodecSwitched { .. }
            | Self::AudioCodecSourceIsFixed { .. }
            | Self::AudioReceptionStalled { .. } => 0,
        }
    }
}

/// Restore an encoded source's payload for an active format it was not configured for.
///
/// `Some` only for the one pair that is translatable at all: a source fixed to MLOW's CELT escape
/// feeding a call that has since downgraded to native Opus, where the fix is the TOC rewrite the
/// escape applied in the first place. `None` means the frame cannot honestly go on the wire.
///
/// The escape's SID token has no RFC Opus spelling, so it translates to nothing -- and that is
/// correct rather than lossy: a SID says the speaker is silent, and a native Opus peer reads a gap
/// in the stream the same way. It is reported as a drop all the same, because a source stuck in DTX
/// against a peer that never hears comfort noise is worth seeing in the counters.
fn translate_encoded_for_active_format(m: &MediaState, payload: &[u8]) -> Option<Vec<u8>> {
    let escape_source = m.audio.format.codec == AudioCodec::Opus
        && m.audio.format.rtp_profile == AudioRtpProfile::Mlow;
    if !escape_source || m.active_format != m.audio.format.sibling_for(AudioCodec::Opus)? {
        return None;
    }
    let mut translated = payload.to_vec();
    depacketize_opus_from_mlow(&mut translated).ok()?;
    Some(translated)
}

/// Ask the content probe whether this packet's own two statements contradict the negotiation.
///
/// Free function rather than a method because both callers hold `&mut MediaState` while the switch
/// it may return needs the whole engine; the verdict is applied once that borrow ends.
fn observe_codec_content(m: &mut MediaState, payload: &[u8]) -> Option<AudioCodec> {
    m.codec_probe.observe(
        payload,
        m.active_format.codec,
        m.audio_reception.frame_span(),
        m.active_format.rtp_clock_rate,
        m.audio.format.rtp_timestamp_step,
    )
}

/// Ask one participant's content probe whether its bytes contradict the call's negotiation.
///
/// The group twin of [`observe_codec_content`]. A verdict is remembered in `foreign_participants`,
/// because native Opus carries no marker: nothing in the NEXT packet would say so again.
fn observe_group_codec_content(
    group: &mut GroupEngineState,
    participant: &crate::voip::group_media::ParticipantMedia,
    format: AudioFormat,
    classified: AudioCodec,
) -> Option<AudioCodec> {
    let span = group
        .audio_reception
        .get(&participant.participant_id)
        .and_then(RtpReceptionStats::frame_span);
    let verdict = group
        .codec_probes
        .entry(participant.participant_id.clone())
        .or_default()
        .observe(
            &participant.payload,
            // The packet's OWN classification, never a hard-coded MLOW. The probe abstains unless
            // the grammar in force is MLow, and that guard is the only thing standing between it
            // and a valid escape: an escape's payload IS an Opus packet, so at the negotiated
            // cadence it agrees with itself, and the probe would promote the participant and then
            // hand the sink a rewritten TOC labelled native Opus -- undecodable, for the rest of
            // the call. The direct path is safe because its active codec is call-wide; in a group
            // the escape is per packet, so the per-packet answer is what the guard needs.
            classified,
            span,
            format.rtp_clock_rate,
            format.rtp_timestamp_step,
        )?;
    group
        .foreign_participants
        .insert(participant.participant_id.clone());
    Some(verdict)
}

/// The optional media plane: the SRTP pipeline, selected audio mode, an optional SFrame session,
/// and the PCM playout jitter buffer. One `MediaPipeline` serves both directions: protect uses its send
/// keys/ROC/RTP state, unprotect its recv keys/ROC, and those fields are disjoint.
struct MediaState {
    pipe: MediaPipeline,
    audio: AudioConfig,
    /// The payload grammar in use right now, which is not always the one negotiated at setup.
    ///
    /// `audio.format` stays immutable and describes the negotiated RTP timing. This is its sibling
    /// within that timing: [`AudioFormat::MLOW_16KHZ_60MS`] and [`AudioFormat::OPUS_16KHZ_60MS`]
    /// agree on payload type, clock rate, timestamp step and samples per frame, and differ only in
    /// `codec` and `rtp_profile`. Swapping between them therefore changes no RTP header byte, no
    /// SSRC and no timestamp continuity, which is why this is a field rather than a reconfiguration.
    active_format: AudioFormat,
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
    /// Watches inbound payloads for a peer whose bytes contradict its signaling. The stream it can
    /// rescue is one that negotiated MLOW -- which an encoded call does too, so this is not gated
    /// on the built-in decoder.
    codec_probe: InboundCodecProbe,
    /// The platform's decoder for a codec the core cannot implement. `None` on wasm32/ESP32 and on
    /// any build without the libopus adapter; the engine then reports silence rather than faking it.
    foreign_audio: Option<Box<dyn ForeignAudioCodec>>,
    /// Reused across packets so the foreign decode path does not allocate per frame.
    foreign_pcm: Vec<i16>,
    /// The send-side twin of `foreign_pcm`, used by the standard-Opus PCM send path.
    foreign_encoded: Vec<u8>,
    audio_rtcp_announced: bool,
    audio_tx_invalid_streak: u8,
    sframe: Option<SframeSession>,
    /// Whether any inbound frame has ever authenticated as SFrame.
    ///
    /// The gate on counting a failed tag. `SframeSession::decrypt` reads the wrapping from the
    /// frame's own trailing bytes, and a plain codec frame can end in bytes that parse as a header
    /// by coincidence -- roughly a percent of them do -- so a failed tag alone does not mean the
    /// peer wraps. One frame that authenticates does mean it, and from then on a failure is real.
    /// Until then the two are genuinely indistinguishable and the silence alarm is what reports a
    /// call whose keys are wrong from end to end.
    sframe_authenticated: bool,
    /// Playout state for a PCM call. Not gated on the built-in codec: an injected `Opus` decoder
    /// feeds this very buffer, and without it the samples it produced would have nowhere to go.
    pcm: Option<PcmAudioState>,
    /// `NEVER` for encoded I/O, which has no core-side playout timer.
    playout_deadline: Millis,
}

struct PcmAudioState {
    #[cfg(feature = "voip-mlow")]
    encoder: mlow::MlowEncoder,
    #[cfg(feature = "voip-mlow")]
    decoder: mlow::MlowDecoder,
    /// Reused per outbound frame to hold the i16->f32 conversion, so the encode hot path doesn't
    /// allocate a fresh Vec each frame.
    #[cfg(feature = "voip-mlow")]
    scratch: Vec<f32>,
    /// Reused codec output before SRTP copies it into the protected packet.
    #[cfg(feature = "voip-mlow")]
    encoded: Vec<u8>,
    jitter: VecDeque<i16>,
    /// Playout emits silence (without draining) while the jitter buffer fills to `PLAYOUT_TARGET`, so
    /// a late packet costs one re-prime instead of a silence gap every 20ms tick. Re-armed on underrun.
    priming: bool,
    /// Whether the most recent packet became audio rather than concealment.
    ///
    /// The content-switch purge was written when the probe only ever ran after packets STOPPED
    /// becoming audio, so everything queued was manufactured silence and clearing it lost nothing.
    /// Native CELT breaks that: it decodes correctly and is queued as real speech while the probe
    /// corroborates it, so purging throws away the corroboration window of what the peer said.
    ///
    /// The MOST RECENT packet, not "any so far", because the queue is what the last few packets put
    /// there: a call that decoded fine an hour ago and is concealing now still has a queue full of
    /// concealment, and "any so far" would keep it forever.
    last_packet_decoded: bool,
    /// Consecutive playout ticks spent priming; bounds the wait so a partial buffer (the peer sent one
    /// frame then went DTX) is flushed after `MAX_PRIME_TICKS` instead of being held silent forever.
    priming_ticks: u32,
    /// Samples in the peer's most recent packet, the input to [`playout_bounds`]. Starts at the
    /// 60ms default until the first decode reports otherwise.
    packet_samps: usize,
    /// Latency ceiling in force, tracked rather than recomputed so it can lag a shrinking packet
    /// until the backlog drains; see [`effective_playout_cap`].
    playout_cap: usize,
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
    /// Whether the application has been told about the current requirement.
    ///
    /// Separate from the requirement itself because the two end at different
    /// moments: the requirement ends when an IDR reaches the wire, the request
    /// ends when it is made. Asking again for a requirement already announced
    /// would raise one event per dropped frame; not tracking it at all would
    /// leave a plane that was born requiring an IDR -- every plane is --
    /// waiting for a request nobody ever made.
    keyframe_announced: bool,
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
        keyframe_announced: false,
    })
}

struct GroupEngineState {
    registry: GroupMediaRegistry,
    local_device: Jid,
    local_epoch_transaction: Option<u32>,
    required_epoch_transaction: Option<u32>,
    direct_fallback_active: bool,
    stream_ssrcs: [u32; 9],
    app_data_ssrc: u32,
    hbh_fec_ssrcs: [u32; 2],
    app_data: MediaPipeline,
    reaction_transaction: u64,
    reaction_last_seen: HashMap<String, ReactionWatermark>,
    pending_reactions: VecDeque<PendingReaction>,
    mixer: ParticipantAudioMixer,
    video_orientations: HashMap<Jid, u8>,
    audio_reception: HashMap<String, RtpReceptionStats>,
    video_reception: HashMap<String, RtpReceptionStats>,
    #[cfg(feature = "voip-mlow")]
    decoders: HashMap<String, mlow::MlowDecoder>,
    /// One injected decoder per participant speaking standard Opus. Separate instances because
    /// these codecs carry inter-frame state: one shared across speakers corrupts every one of them.
    foreign_decoders: HashMap<String, Box<dyn ForeignAudioCodec>>,
    /// Per participant, because in a group the question is per participant: the call negotiates one
    /// format, and any one member may be outside the MLOW rollout and sending Opus under it. Not
    /// gated on the built-in codec, for the same reason the direct path's is not: an encoded group
    /// call asks the same question and never decodes anything itself.
    codec_probes: HashMap<String, InboundCodecProbe>,
    /// Participants whose own bytes contradicted the negotiation. Native Opus carries no escape
    /// marker, so nothing about the packet says so -- only the accumulated evidence does, and it
    /// has to be remembered or every packet after the verdict would be classified MLOW again.
    foreign_participants: HashSet<String>,
}

struct ReactionWatermark {
    pid: Option<u32>,
    transaction_id: u64,
}

struct PendingReaction {
    payload: Vec<u8>,
    remaining: u8,
    next_at: Millis,
}

/// The sans-IO call engine. See the module docs for the drive contract.
pub struct CallEngine {
    call_id: String,
    direction: CallDirection,
    // Control plane.
    relay_token: Vec<u8>,
    endpoint_xor: [u8; 6],
    relay_addr: SocketAddr,
    integrity_key: Vec<u8>,
    allocate: Bytes,
    allocate_transaction_id: Option<[u8; 12]>,
    tx_ids: Box<dyn TxIdSource>,
    keepalive_deadline: Millis,
    /// Next RTCP Sender-Report tick (NEVER until the relay allocates).
    rtcp_deadline: Millis,
    /// Mapping injected at start so SR NTP timestamps use wall time while scheduling stays monotonic.
    rtcp_monotonic_origin: Millis,
    rtcp_wallclock_origin_ms: u64,
    /// Deadline by which the allocate must be acked; NEVER once it is (or after firing the timeout).
    allocate_deadline: Millis,
    /// The current Allocate transaction still requires a matching success or error response.
    /// Subscription-only refreshes keep `allocated` true so established media remains live.
    allocate_pending: bool,
    allocated: bool,
    /// Whether the peer has picked up. The watchdog needs this AND [`Self::allocated`], in whichever
    /// order they arrive: inbound media cannot flow before the relay answers our allocate, so arming
    /// on the accept alone reports the allocation interval itself as lost reception.
    peer_has_answered: bool,
    started: bool,
    /// A terminal relay-allocate failure was surfaced; the engine goes inert (no keepalive, no
    /// timer, no further transmits) so the driver tears the call down instead of keepaliving a
    /// dead relay forever.
    terminated: bool,
    /// Our SRTP participant id (LID normalized to `<user>:<device>@lid`), the HKDF input for every
    /// stream SSRC. Retained so the STUN allocate announces this call's live SSRCs.
    self_participant_id: String,
    group: Option<GroupEngineState>,
    /// Mints the per-participant decoders the group path needs. Held on the engine rather than in
    /// `GroupEngineState` because it outlives any one group session and is installed at
    /// construction, before there is one.
    foreign_audio_factory: Option<Box<dyn ForeignAudioCodecFactory>>,
    // Media plane (None = control plane only, e.g. esp32).
    media: Option<MediaState>,
    /// Peer device orientation (0..3, ×90°) from the last `<video device_orientation>`; stamped on
    /// every reassembled inbound AU so the sink can rotate.
    peer_video_orientation: u8,
    /// Media counters and the audio-health watchdog. On the engine rather than `MediaState` so a
    /// call that never gets a media plane still reports zeroes instead of nothing.
    media_stats: CallMediaStats,
    health: AudioHealthWatch,
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
        if config.enable_media && !config.audio.format.is_valid() {
            return Err(EngineError::BadAudioFormat);
        }
        // PCM I/O accepts the one swappable pair and nothing else. Opus is admitted because a peer
        // outside the MLow rollout selects it during signaling, before the engine is built, and
        // refusing here would turn "the peer speaks Opus" into a call that never starts. A build
        // with no decoder for it does not pretend: it reports `CallEvent::AudioSilent` with
        // `NoDecoderForNegotiatedCodec` once media is flowing. Anything outside the pair has a
        // different RTP timing and is still refused.
        if config.enable_media
            && config.audio.io == AudioIo::Pcm
            && config.audio.format != AudioFormat::MLOW_16KHZ_60MS
            && config.audio.format != AudioFormat::OPUS_16KHZ_60MS
        {
            return Err(EngineError::UnsupportedPcmAudio);
        }
        // Only the MLOW half of the swappable pair needs the built-in codec. Standard Opus PCM is
        // what `voip-libopus` is for, and a peer outside the rollout selects it during signaling --
        // refusing it here would make that feature combination reject every call it exists to serve.
        #[cfg(not(feature = "voip-mlow"))]
        if config.enable_media
            && config.audio.io == AudioIo::Pcm
            && config.audio.format.codec == AudioCodec::Mlow
        {
            return Err(EngineError::MlowUnavailable);
        }
        let endpoint_xor = stun::encode_xor_relay_endpoint(&config.relay_ip, config.relay_port)
            .ok_or(EngineError::BadEndpoint)?;
        let relay_ip = config
            .relay_ip
            .parse::<Ipv4Addr>()
            .map_err(|_| EngineError::BadEndpoint)?;
        let relay_addr = SocketAddr::new(IpAddr::V4(relay_ip), config.relay_port);

        let media = if config.enable_media {
            let audio_rtcp_cname = build_whatsapp_rtcp_cname(&tx_ids.next_tx_id());
            let mut pipe = MediaPipeline::new_with_rtcp_cname(
                &MediaPipelineParams {
                    call_key: &config.call_key,
                    self_lid: &config.self_lid,
                    peer_lid: &config.peer_lid,
                    ssrc: config.ssrc,
                    samples_per_packet: config.audio.format.rtp_timestamp_step,
                    warp_mi_tag_len: config.warp_mi_tag_len,
                },
                audio_rtcp_cname,
            )
            .ok_or(EngineError::BadCallKey)?;
            if !pipe.set_audio_payload_type(config.audio.format.rtp_payload_type) {
                return Err(EngineError::BadAudioFormat);
            }
            pipe.set_audio_mlow_profile(matches!(
                config.audio.format.rtp_profile,
                AudioRtpProfile::Mlow
            ));
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
                audio: config.audio,
                active_format: config.audio.format,
                audio_reception: RtpReceptionStats::default(),
                call_key: config.call_key.clone(),
                self_lid: config.self_lid.clone(),
                recv_peer_lid: config.peer_lid.clone(),
                warp_mi_tag_len: config.warp_mi_tag_len,
                video_ts_stride: VIDEO_TS_STRIDE_15FPS,
                video,
                codec_probe: InboundCodecProbe::default(),
                foreign_audio: None,
                foreign_pcm: Vec::new(),
                foreign_encoded: Vec::new(),
                audio_rtcp_announced: false,
                audio_tx_invalid_streak: 0,
                sframe,
                sframe_authenticated: false,
                pcm: (config.audio.io == AudioIo::Pcm).then(|| PcmAudioState {
                    #[cfg(feature = "voip-mlow")]
                    encoder: mlow::MlowEncoder::new(),
                    #[cfg(feature = "voip-mlow")]
                    decoder: mlow::MlowDecoder::new(),
                    #[cfg(feature = "voip-mlow")]
                    scratch: Vec::with_capacity(config.audio.format.samples_per_frame as usize),
                    #[cfg(feature = "voip-mlow")]
                    encoded: Vec::with_capacity(MLOW_ENCODED_CAPACITY),
                    jitter: VecDeque::new(),
                    priming: true,
                    last_packet_decoded: false,
                    priming_ticks: 0,
                    packet_samps: OPUS_FRAME_SAMPS_60MS,
                    playout_cap: playout_bounds(OPUS_FRAME_SAMPS_60MS).1,
                }),
                playout_deadline: NEVER,
            })
        } else {
            None
        };

        let mut engine = Self {
            call_id: config.call_id,
            direction: config.direction,
            relay_token: config.relay_token,
            endpoint_xor,
            relay_addr,
            integrity_key: config.integrity_key,
            allocate: Bytes::new(),
            allocate_transaction_id: None,
            tx_ids,
            keepalive_deadline: 0,
            rtcp_deadline: NEVER,
            rtcp_monotonic_origin: 0,
            rtcp_wallclock_origin_ms: 0,
            allocate_deadline: 0,
            allocate_pending: false,
            allocated: false,
            peer_has_answered: false,
            started: false,
            terminated: false,
            self_participant_id: ssrc::format_e2e_srtp_participant_id(&config.self_lid),
            foreign_audio_factory: None,
            group: None,
            media,
            peer_video_orientation: 0,
            media_stats: CallMediaStats::default(),
            health: AudioHealthWatch::default(),
            outbox: VecDeque::new(),
        };
        // A video-from-start call builds its plane here, already requiring an
        // IDR, and the application may never call `enable_video` on a plane that
        // is up before it asks. Announced now so the request does not depend on
        // one arriving; the flag makes a later `enable_video` a no-op.
        engine.announce_video_keyframe();
        Ok(engine)
    }

    pub fn call_id(&self) -> &str {
        &self.call_id
    }

    /// WARP authentication-tag width baked into the active media pipelines.
    pub fn media_warp_mi_tag_len(&self) -> Option<usize> {
        self.media.as_ref().map(|media| media.warp_mi_tag_len)
    }

    pub fn direction(&self) -> CallDirection {
        self.direction
    }

    /// Whether participant-indexed group media has been configured.
    pub fn is_group(&self) -> bool {
        self.group.is_some()
    }

    pub(crate) fn group_epoch_transaction(&self) -> Option<u32> {
        self.group
            .as_ref()
            .and_then(|group| group.local_epoch_transaction)
    }

    /// Enable participant-indexed media before [`start`](Self::start).
    pub fn configure_group(&mut self, config: GroupEngineConfig) -> Result<(), EngineError> {
        if self.started {
            return Err(GroupMediaError::Pipeline.into());
        }
        self.configure_group_at(0, config)
    }

    fn configure_group_at(
        &mut self,
        now: Millis,
        config: GroupEngineConfig,
    ) -> Result<(), EngineError> {
        let local_sender = group_roster_local_device(&config.initial_update, &config.self_jid)
            .ok_or(GroupMediaError::LocalParticipantRemoved)?
            .jid
            .clone();
        // Validate relay material before constructing or publishing group state. In particular, a
        // failed direct-to-group promotion must remain retryable with the same roster transaction.
        let relay_refresh = prepare_group_relay_refresh(&config.initial_update)?;
        let audio = self.media.as_ref().ok_or(GroupMediaError::Pipeline)?.audio;
        let mut registry = GroupMediaRegistry::new(
            self.call_id.clone(),
            config.call_creator,
            &config.self_jid,
            audio.format.rtp_timestamp_step,
            self.media
                .as_ref()
                .ok_or(GroupMediaError::Pipeline)?
                .warp_mi_tag_len,
            VIDEO_TS_STRIDE_15FPS,
        )?;
        let direct_fallback_active = config.direct_peer.is_some();
        if let Some(DirectPeer {
            user_jid,
            device_jid,
            mut call_key,
        }) = config.direct_peer
        {
            registry.seed_direct_peer(
                &call_key,
                &user_jid,
                &device_jid,
                self.media
                    .as_ref()
                    .and_then(|media| media.video.as_ref())
                    .is_some(),
            )?;
            call_key.zeroize();
        }
        registry.apply_group_update(&config.initial_update)?;

        let live_sender = self.started || self.allocated || self.allocate_pending;
        let local_sender = if live_sender {
            self.media
                .as_ref()
                .ok_or(GroupMediaError::Pipeline)?
                .self_lid
                .clone()
        } else {
            local_sender.to_string()
        };
        let local_participant_id = ssrc::format_e2e_srtp_participant_id(&local_sender);
        let derived_stream_ssrcs =
            ssrc::derive_wasm_relay_stream_ssrcs(&self.call_id, &local_participant_id);
        if live_sender {
            let media = self.media.as_ref().ok_or(GroupMediaError::Pipeline)?;
            let sender_is_unchanged = self.self_participant_id == local_participant_id
                && media.pipe.send_ssrc() == derived_stream_ssrcs[0]
                && media
                    .video
                    .as_ref()
                    .is_none_or(|video| video.pipe.send_ssrc() == derived_stream_ssrcs[3]);
            if !sender_is_unchanged {
                // A live promotion may add the group receive/control planes, but it cannot rotate
                // an already allocated sender identity or RTP stream underneath in-flight media.
                return Err(GroupMediaError::Pipeline.into());
            }
        }
        let app_data_ssrc = ssrc::derive_wasm_participant_ssrc(
            &self.call_id,
            &local_participant_id,
            ssrc::APP_DATA_SSRC_SLOT_WORD,
        );
        let stream_ssrcs = self.prepare_group_stream_ssrcs(&local_participant_id, app_data_ssrc)?;
        let media = self.media.as_ref().ok_or(GroupMediaError::Pipeline)?;
        let mut app_data = MediaPipeline::new(&MediaPipelineParams {
            call_key: &media.call_key,
            self_lid: &local_sender,
            peer_lid: &media.recv_peer_lid,
            ssrc: app_data_ssrc,
            samples_per_packet: app_data::APP_DATA_RTP_TIMESTAMP_STRIDE,
            warp_mi_tag_len: media.warp_mi_tag_len,
        })
        .ok_or(GroupMediaError::Pipeline)?;
        if !app_data.set_audio_payload_type(RTP_PAYLOAD_TYPE_APP_DATA) {
            return Err(GroupMediaError::Pipeline.into());
        }
        app_data.set_audio_mlow_profile(false);
        let hbh_fec_ssrcs = [
            ssrc::derive_wasm_participant_ssrc(
                &self.call_id,
                &local_participant_id,
                ssrc::HBH_FEC_TX_SSRC_SLOT_WORD,
            ),
            ssrc::derive_wasm_participant_ssrc(
                &self.call_id,
                &local_participant_id,
                ssrc::HBH_FEC_RX_SSRC_SLOT_WORD,
            ),
        ];
        self.self_participant_id = local_participant_id;
        let media = self.media.as_mut().ok_or(GroupMediaError::Pipeline)?;
        media.self_lid = local_sender;
        media.pipe.set_send_ssrc(stream_ssrcs[0]);
        if let Some(video) = media.video.as_mut() {
            video.pipe.set_send_ssrc(stream_ssrcs[3]);
        }
        let mut group = GroupEngineState {
            registry,
            local_device: config.self_jid,
            local_epoch_transaction: None,
            required_epoch_transaction: config
                .initial_update
                .rekey_requested
                .then_some(config.initial_update.transaction_id),
            direct_fallback_active,
            stream_ssrcs,
            app_data_ssrc,
            hbh_fec_ssrcs,
            app_data,
            reaction_transaction: 0,
            reaction_last_seen: HashMap::new(),
            pending_reactions: VecDeque::new(),
            mixer: ParticipantAudioMixer::new(),
            video_orientations: HashMap::new(),
            audio_reception: HashMap::new(),
            video_reception: HashMap::new(),
            #[cfg(feature = "voip-mlow")]
            decoders: HashMap::new(),
            foreign_decoders: HashMap::new(),
            codec_probes: HashMap::new(),
            foreign_participants: HashSet::new(),
        };
        group.mixer.retain(group.registry.active_participant_ids());
        self.group = Some(group);
        self.commit_group_allocate(now, &config.initial_update, relay_refresh, true)?;
        self.sync_group_epoch()?;
        Ok(())
    }

    pub fn apply_group_update(
        &mut self,
        now: Millis,
        update: &GroupCallUpdate,
    ) -> Result<GroupRosterApply, EngineError> {
        if self.group.is_none() {
            let (self_jid, peer_device, call_key) = {
                let media = self.media.as_ref().ok_or(GroupMediaError::Pipeline)?;
                let self_jid = media
                    .self_lid
                    .parse::<Jid>()
                    .map_err(|_| GroupMediaError::Pipeline)?;
                let peer_device = media
                    .recv_peer_lid
                    .parse::<Jid>()
                    .map_err(|_| GroupMediaError::Pipeline)?;
                (self_jid, peer_device, media.call_key.clone())
            };
            self.configure_group_at(
                now,
                GroupEngineConfig {
                    call_creator: update.call_creator.clone(),
                    self_jid,
                    initial_update: update.clone(),
                    direct_peer: Some(DirectPeer {
                        user_jid: peer_device.to_non_ad(),
                        device_jid: peer_device,
                        call_key,
                    }),
                },
            )?;
            return Ok(GroupRosterApply::Applied);
        }
        // Validate fallible relay material before advancing the roster transaction. Otherwise a
        // malformed relay could partially commit the roster and make a corrected resend look stale.
        let relay_refresh = prepare_group_relay_refresh(update)?;
        let established_warp_mi_tag_len = self
            .media
            .as_ref()
            .ok_or(GroupMediaError::Pipeline)?
            .warp_mi_tag_len;
        if relay_refresh
            .as_ref()
            .is_some_and(|refresh| refresh.warp_mi_tag_len != established_warp_mi_tag_len)
        {
            // Every existing send/receive pipeline was constructed with the established WARP tag
            // length. A roster refresh cannot change that packet boundary atomically today, so
            // reject it before the authoritative transaction commits.
            return Err(GroupMediaError::Pipeline.into());
        }
        let previous_pids = self
            .group
            .as_ref()
            .ok_or(GroupMediaError::Pipeline)?
            .registry
            .active_pids();
        let previous_devices = self
            .group
            .as_ref()
            .ok_or(GroupMediaError::Pipeline)?
            .registry
            .active_device_ids();
        let changed_pid_participants = self
            .group
            .as_ref()
            .ok_or(GroupMediaError::Pipeline)?
            .registry
            .participants_with_pid_changes(update);
        let result = self
            .group
            .as_mut()
            .ok_or(GroupMediaError::Pipeline)?
            .registry
            .apply_group_update(update)?;
        if result == GroupRosterApply::Applied {
            let local_device = &self
                .group
                .as_ref()
                .ok_or(GroupMediaError::Pipeline)?
                .local_device;
            if !group_roster_contains_participant(update, local_device) {
                // A full replacement roster that removes this endpoint is authoritative teardown.
                // Mark the engine inert before returning the fatal control result so no caller that
                // observes the error can continue capturing or protecting media.
                self.terminated = true;
                return Err(GroupMediaError::LocalParticipantRemoved.into());
            }
            let group = self.group.as_mut().ok_or(GroupMediaError::Pipeline)?;
            if update.rekey_requested {
                group.required_epoch_transaction = Some(
                    group
                        .required_epoch_transaction
                        .map_or(update.transaction_id, |current| {
                            current.max(update.transaction_id)
                        }),
                );
                // Retransmissions were protected under the old app-data key. Discarding them is
                // safer than leaking removed membership across the epoch boundary, and avoids a
                // due retry keeping the timer hot while group media is gated.
                group.pending_reactions.clear();
            }
            let active = group.registry.active_participant_ids();
            for participant in changed_pid_participants {
                // A new PID is a new media session for the same participant, which is why the mixer
                // is reset here. Every other piece of per-participant decode state is in the same
                // position: a stateful decoder carries predictor and synthesis history from the
                // retired session into the replacement's first packets, and the codec verdict was
                // reached about a stream that no longer exists.
                group.mixer.reset(&participant);
                #[cfg(feature = "voip-mlow")]
                group.decoders.remove(&participant);
                group.foreign_decoders.remove(&participant);
                group.codec_probes.remove(&participant);
                group.foreign_participants.remove(&participant);
                // The reception stats too, and this one is not merely stale but actively harmful.
                // A group SSRC is derived from the device identity, not the PID, so the replacement
                // session keeps it and its sequence numbers can restart BELOW the retired session's
                // maximum. Every one of those packets then reads as reordered, which clears the
                // frame span rather than measuring one -- and the probe abstains without a span. A
                // participant that needs probing to be heard at all would stay silent until the new
                // sequence climbed past the old maximum.
                group.audio_reception.remove(&participant);
                group.video_reception.remove(&participant);
            }
            group.mixer.retain(active.iter().cloned());
            group
                .audio_reception
                .retain(|participant, _| active.contains(participant));
            group
                .video_reception
                .retain(|participant, _| active.contains(participant));
            group
                .reaction_last_seen
                .retain(|participant, _| active.contains(participant));
            group.video_orientations.retain(|jid, _| {
                update.participants.iter().any(|participant| {
                    participant.is_connected()
                        && (participant.jid == *jid
                            || participant.devices.iter().any(|device| device.jid == *jid))
                })
            });
            #[cfg(feature = "voip-mlow")]
            group
                .decoders
                .retain(|participant, _| active.contains(participant));
            group
                .foreign_decoders
                .retain(|participant, _| active.contains(participant));
            group
                .codec_probes
                .retain(|participant, _| active.contains(participant));
            group
                .foreign_participants
                .retain(|participant| active.contains(participant));
            let current_pids = group.registry.active_pids();
            let subscriptions_changed = current_pids != previous_pids;
            let current_devices = group.registry.active_device_ids();
            let participant_added = update.media == "video"
                && current_devices
                    .iter()
                    .any(|device| !previous_devices.contains(device));
            if update.media == "audio" {
                self.disable_video();
                self.purge_queued_video_outputs();
            } else if participant_added {
                // A newly subscribed receiver has no decoder reference state. Start its stream on
                // an IDR even when no locally queued video happened to require purging.
                self.require_video_keyframe();
            }
            self.commit_group_allocate(now, update, relay_refresh, subscriptions_changed)?;
            self.sync_group_epoch()?;
        }
        Ok(result)
    }

    pub fn apply_group_raw_epoch(
        &mut self,
        transaction_id: u32,
        raw_epoch: &[u8],
    ) -> Result<GroupEpochApply, GroupMediaError> {
        let result = self
            .group
            .as_mut()
            .ok_or(GroupMediaError::Pipeline)?
            .registry
            .apply_raw_epoch(transaction_id, raw_epoch)?;
        self.sync_group_epoch()?;
        Ok(result)
    }

    /// Queue a reaction on the authenticated RTC app-data stream. The captured sender repeats each
    /// transaction ten times at 50 ms intervals; receivers suppress duplicates per participant.
    pub fn send_group_reaction(&mut self, now: Millis, emoji: &str) -> Result<(), GroupMediaError> {
        if !self.group_epoch_ready() {
            return Err(GroupMediaError::Pipeline);
        }
        let group = self.group.as_mut().ok_or(GroupMediaError::Pipeline)?;
        group.reaction_transaction = group.reaction_transaction.wrapping_add(1).max(1);
        let payload = app_data::encode_reaction(group.reaction_transaction, emoji)
            .map_err(|_| GroupMediaError::Pipeline)?;
        let packet = group.app_data.protect_audio(&payload);
        self.outbox.push_back(Output::Transmit(Bytes::from(packet)));
        if group.pending_reactions.len() == MAX_PENDING_REACTIONS {
            group.pending_reactions.pop_front();
        }
        group.pending_reactions.push_back(PendingReaction {
            payload,
            remaining: APP_DATA_RETRANSMIT_COUNT - 1,
            next_at: now + APP_DATA_RETRANSMIT_MS,
        });
        Ok(())
    }

    fn prepare_group_stream_ssrcs(
        &mut self,
        participant_id: &str,
        app_data_ssrc: u32,
    ) -> Result<[u32; 9], EngineError> {
        let mut stream_ssrcs = ssrc::derive_wasm_relay_stream_ssrcs(&self.call_id, participant_id);
        let mut used = stream_ssrcs[..6].iter().copied().collect::<HashSet<_>>();
        used.insert(app_data_ssrc);
        for stream_ssrc in &mut stream_ssrcs[6..] {
            let mut selected = None;
            for _ in 0..64 {
                let random = self.tx_ids.next_tx_id();
                let candidate = u32::from_be_bytes([random[4], random[5], random[6], random[7]]);
                if candidate != 0 && used.insert(candidate) {
                    selected = Some(candidate);
                    break;
                }
            }
            *stream_ssrc = selected.ok_or(GroupMediaError::Pipeline)?;
        }
        Ok(stream_ssrcs)
    }

    fn commit_group_allocate(
        &mut self,
        now: Millis,
        update: &GroupCallUpdate,
        relay_refresh: Option<GroupRelayRefresh>,
        subscriptions_changed: bool,
    ) -> Result<(), GroupMediaError> {
        let mut reconnect = None;
        let relay_material_changed = relay_refresh.as_ref().is_some_and(|refresh| {
            refresh.relay_addr != self.relay_addr
                || refresh.relay_token != self.relay_token
                || refresh.endpoint_xor != self.endpoint_xor
                || refresh.integrity_key != self.integrity_key
        });
        if !relay_material_changed && !subscriptions_changed {
            return Ok(());
        }
        if let Some(refresh) = relay_refresh {
            if refresh.relay_addr != self.relay_addr {
                self.relay_addr = refresh.relay_addr;
                reconnect = Some(refresh.relay_addr);
            }
            self.relay_token = refresh.relay_token;
            self.endpoint_xor = refresh.endpoint_xor;
            self.integrity_key = refresh.integrity_key;
        }
        if relay_material_changed {
            // New credentials invalidate the allocation even on the same endpoint; address-only
            // checks would leave the fresh token/key without an acknowledgement or timeout.
            self.allocated = false;
        }
        self.allocate_pending = true;
        if self.started && reconnect.is_none() {
            self.allocate_deadline = now + ALLOCATE_TIMEOUT_MS;
        } else if reconnect.is_some() {
            // The shell cannot send the replacement Allocate until transport reconnection
            // completes. Starting its deadline here would charge the network handshake against the
            // relay response budget.
            self.allocate_deadline = NEVER;
        }
        let group = self.group.as_ref().ok_or(GroupMediaError::Pipeline)?;
        let pids = remote_group_pids(update, &group.local_device);
        let transaction_id = self.tx_ids.next_tx_id();
        let allocate = Bytes::from(stun::build_wasm_group_stun_allocate_request(
            &stun::WasmGroupStunAllocateRequest {
                transaction_id: &transaction_id,
                relay_token: &self.relay_token,
                endpoint_xor: &self.endpoint_xor,
                integrity_key: &self.integrity_key,
                stream_ssrcs: &group.stream_ssrcs,
                app_data_ssrc: group.app_data_ssrc,
                hbh_fec_ssrcs: &group.hbh_fec_ssrcs,
                participant_pids: &pids,
            },
        ));
        self.allocate = allocate.clone();
        self.allocate_transaction_id = Some(transaction_id);
        if self.started {
            if let Some(relay_addr) = reconnect {
                self.outbox.push_back(Output::ReconnectRelay(relay_addr));
            }
            self.outbox.push_back(Output::Transmit(allocate));
        }
        Ok(())
    }

    fn purge_queued_video_outputs(&mut self) {
        self.outbox.retain(|output| {
            !matches!(
                output,
                Output::Transmit(packet)
                    if parse_rtp_header(packet)
                        .is_some_and(|header| header.payload_type == RTP_PAYLOAD_TYPE_H264)
            )
        });
    }

    fn sync_group_epoch(&mut self) -> Result<(), GroupMediaError> {
        let Some((transaction_id, mut epoch)) = self
            .group
            .as_ref()
            .and_then(|group| group.registry.installed_epoch())
            .map(|(transaction, epoch)| (transaction, epoch.to_vec()))
        else {
            return Ok(());
        };
        if self
            .group
            .as_ref()
            .and_then(|group| group.local_epoch_transaction)
            .is_some_and(|current| transaction_id <= current)
        {
            epoch.zeroize();
            return Ok(());
        }
        let (Some(media), Some(group)) = (&mut self.media, &mut self.group) else {
            epoch.zeroize();
            return Err(GroupMediaError::Pipeline);
        };
        let Some(audio_rekey) = MediaPipeline::prepare_send_rekey(&epoch, &media.self_lid) else {
            epoch.zeroize();
            return Err(GroupMediaError::InvalidEpoch);
        };
        let video_rekey = match media.video.as_ref() {
            Some(_) => {
                let Some(rekey) = VideoPipeline::prepare_send_rekey(&epoch, &media.self_lid) else {
                    epoch.zeroize();
                    return Err(GroupMediaError::InvalidEpoch);
                };
                Some(rekey)
            }
            None => None,
        };
        let Some(app_data_rekey) = MediaPipeline::prepare_send_rekey(&epoch, &media.self_lid)
        else {
            epoch.zeroize();
            return Err(GroupMediaError::InvalidEpoch);
        };
        // All derivations are complete before any live send pipeline changes epoch.
        media.pipe.commit_send_rekey(audio_rekey);
        if let (Some(video), Some(rekey)) = (media.video.as_mut(), video_rekey) {
            video.pipe.commit_send_rekey(rekey);
            // A new group epoch may be the first decryptable media for a recently admitted
            // participant, so never begin that epoch with a dependent frame.
            video.keyframe_required = true;
        }
        group.app_data.commit_send_rekey(app_data_rekey);
        media.call_key.zeroize();
        media.call_key.clear();
        media.call_key.extend_from_slice(&epoch);
        group.local_epoch_transaction = Some(transaction_id);
        group.direct_fallback_active = false;
        // The roster arrives before its authenticated epoch during normal startup. Installing that
        // epoch creates the receiver pipelines, so refresh the allowlist at the same commit point.
        group.mixer.retain(group.registry.active_participant_ids());
        epoch.zeroize();
        // After the borrows above end. A rekey that began before this epoch was
        // installable had its IDR discarded by the gate in `on_video`, which
        // cleared the announcement so this asks again for the one request that
        // can finally be served.
        self.announce_video_keyframe();
        if self.allocated {
            self.announce_audio_rtcp_session();
        }
        Ok(())
    }

    fn group_epoch_ready(&self) -> bool {
        self.group
            .as_ref()
            .is_none_or(|group| match group.required_epoch_transaction {
                Some(required) => group
                    .local_epoch_transaction
                    .is_some_and(|installed| installed >= required),
                None => group.local_epoch_transaction.is_some() || group.direct_fallback_active,
            })
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

    /// Whether outbound video could reach the wire at all right now: a plane
    /// that is up and not gated.
    ///
    /// The driver's send queue can come to require an IDR on a call that has no
    /// picture to unblock -- a relay reconnect sets that requirement whether or
    /// not video was ever enabled -- and asking an audio-only application for a
    /// keyframe is asking for something it has no way to produce.
    pub(crate) fn video_send_active(&self) -> bool {
        self.media
            .as_ref()
            .and_then(|m| m.video.as_ref())
            .is_some_and(|v| v.active && !v.send_gated)
    }

    /// Whether outbound video is still dropping access units for want of an IDR.
    ///
    /// The driver reads this before retrying a keyframe request a saturated
    /// consumer queue refused: the encoder's own periodic IDR can settle the
    /// requirement in the meantime, and a request nobody is waiting on would
    /// cost the application a keyframe for nothing.
    ///
    /// Sendability is part of the answer, matching what
    /// [`Self::announce_video_keyframe`] is willing to ask for: an IDR made for
    /// a plane that has since been downgraded or re-gated is one the engine
    /// would drop, and whatever re-enables the plane asks again anyway.
    pub(crate) fn video_keyframe_required(&self) -> bool {
        self.media
            .as_ref()
            .and_then(|m| m.video.as_ref())
            .is_some_and(|v| v.active && !v.send_gated && v.keyframe_required)
    }

    /// The peer picked up.
    ///
    /// Separate from [`rekey_recv`](Self::rekey_recv), which the caller also does on an `<accept>`,
    /// because they are different facts: that one re-keys the receive path to the answering device,
    /// this one is the first instant the peer's silence means anything. An outgoing call allocates
    /// its relay when the SERVER acks the offer, so the health watchdog cannot arm there without
    /// reporting a stall three seconds into every ordinary ring. Idempotent, and a no-op for a
    /// callee, which was answered before its media plane existed and armed at allocate.
    pub fn peer_answered(&mut self, now: Millis) {
        self.peer_has_answered = true;
        // Both facts, not just this one. An outgoing peer can accept before the relay finishes
        // allocating -- a buffered accept consumed as soon as a delayed attachment starts makes it
        // ordinary rather than rare -- and inbound media cannot flow until the allocate response
        // arrives. Arming here alone reports a slow allocation as a reception stall, blaming the
        // peer for silence the relay had not yet made possible. Allocation arms the other side of
        // this, so whichever lands second does the arming.
        if self.group.is_none() && self.allocated {
            self.health.media_started(now);
        }
    }

    /// Whether the video plane is currently up (sending is possible, inbound PT-97 decodes).
    pub fn is_video_enabled(&self) -> bool {
        self.media
            .as_ref()
            .and_then(|m| m.video.as_ref())
            .is_some_and(|v| v.active)
    }

    /// Ask the application for the IDR the outbound plane is waiting on, once
    /// per requirement.
    ///
    /// Every site that sets `keyframe_required` ends here, because the engine
    /// never produces a frame itself: until the application sends an IDR,
    /// `on_video` drops everything, and the request is the only thing that
    /// tells it so. Silent while the plane cannot send one anyway (inactive, or
    /// send-gated behind an upgrade the peer has not accepted yet) -- the
    /// moment it can, the ungate calls this again.
    fn announce_video_keyframe(&mut self) {
        let Some(video) = self.media.as_mut().and_then(|media| media.video.as_mut()) else {
            return;
        };
        if !video.keyframe_required || video.keyframe_announced || !video.active || video.send_gated
        {
            return;
        }
        video.keyframe_announced = true;
        self.outbox
            .push_back(Output::Event(CallEvent::VideoKeyframeNeeded));
    }

    /// Re-arm outbound H.264 recovery after the application switches camera/screen sources.
    pub fn require_video_keyframe(&mut self) {
        if let Some(video) = self.media.as_mut().and_then(|media| media.video.as_mut()) {
            // A new requirement, not the standing one: the frames already asked
            // for are from the retired source, so this asks again.
            video.keyframe_required = true;
            video.keyframe_announced = false;
        }
        self.announce_video_keyframe();
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
    /// Idempotent; also ungates a previously send-gated plane. See
    /// [`enable_video_gated`](Self::enable_video_gated).
    pub fn enable_video(&mut self) -> bool {
        self.enable_video_inner(false)
    }

    /// Bring the video plane up but hold OUTBOUND video off the wire (the initiator of an upgrade,
    /// before the peer accepts). Inbound still decodes. [`enable_video`](Self::enable_video) later
    /// ungates it.
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
            // Resuming a plane is a fresh requirement: the peer's decoder lost
            // whatever it had while this one was off or gated.
            let needs_recovery = !v.active || (v.send_gated && !send_gated);
            v.active = true;
            v.send_gated = send_gated;
            if needs_recovery {
                v.keyframe_required = true;
                v.keyframe_announced = false;
            }
            // Unconditional, because this is also where a plane the constructor
            // built for a video-from-start call first becomes announceable: it
            // is already active and already requires an IDR, so `needs_recovery`
            // is false and nothing else would ever ask on its behalf.
            self.announce_video_keyframe();
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
                self.announce_video_keyframe();
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

    /// Record orientation for one authenticated group participant/device.
    pub fn set_participant_video_orientation(&mut self, participant: Jid, orientation: u8) {
        let Some(group) = self.group.as_mut() else {
            return;
        };
        group
            .video_orientations
            .insert(participant, orientation & 0x03);
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
        // `configure_group` may already have committed a group Allocate containing stream SSRCs
        // and participant subscriptions. Never replace it with the 1:1 request below.
        if self.allocate.is_empty() {
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
            self.allocate_transaction_id = Some(tx);
        }
        self.outbox
            .push_back(Output::Transmit(self.allocate.clone()));
        self.keepalive_deadline = now + KEEPALIVE_MS;
        self.allocate_pending = true;
        self.allocate_deadline = now + ALLOCATE_TIMEOUT_MS;
        if let Some(m) = &mut self.media
            && m.audio.io == AudioIo::Pcm
        {
            m.playout_deadline = now + PLAYOUT_MS;
        }
    }

    /// Start the deferred allocation-response budget once a replacement relay transport is ready.
    pub(crate) fn relay_reconnected(&mut self, now: Millis) {
        if self.started && self.allocate_pending && self.allocate_deadline == NEVER {
            self.allocate_deadline = now + ALLOCATE_TIMEOUT_MS;
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
            Input::EncodedAudio(payload) => self.on_encoded_audio(payload),
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
        if self.allocate_pending && self.allocate_deadline != NEVER {
            next = next.min(self.allocate_deadline);
        }
        if let Some(m) = &self.media {
            next = next.min(m.playout_deadline);
            next = next.min(self.rtcp_deadline);
            // The health watchdog needs its own tick: its whole job is to fire when nothing else
            // is happening, so it cannot ride on a timer that inbound media drives.
            next = next.min(self.health.deadline());
        }
        if let Some(deadline) = self
            .group
            .as_ref()
            .and_then(|group| group.pending_reactions.front())
            .map(|reaction| reaction.next_at)
        {
            next = next.min(deadline);
        }
        Some(next)
    }

    /// Current playout jitter-buffer depth in samples. Test-only: lets coverage assert the
    /// feed-side bound without exposing the media plane.
    #[cfg(all(test, feature = "voip-mlow"))]
    pub(crate) fn jitter_len(&self) -> usize {
        self.media
            .as_ref()
            .and_then(|m| m.pcm.as_ref())
            .map_or(0, |pcm| pcm.jitter.len())
    }

    fn on_timeout(&mut self, now: Millis) {
        // The relay never acked the allocate: surface a terminal timeout exactly once, then go inert.
        if self.allocate_pending
            && self.started
            && self.allocate_deadline != NEVER
            && now >= self.allocate_deadline
        {
            self.allocate_deadline = NEVER;
            self.allocate_pending = false;
            self.allocated = false;
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
        let playout_due = self.media.as_ref().is_some_and(|media| {
            self.started && media.audio.io == AudioIo::Pcm && now >= media.playout_deadline
        });
        if playout_due {
            // Not gated on the built-in codec: the mixer holds PCM, and since the group path gained
            // per-participant foreign decoders it is no longer only MLOW that fills it. Gated, a
            // `voip-libopus` group call decoded every participant into a mixer nothing ever drained
            // and then played the empty direct-call buffer instead -- silence, from the one build
            // whose whole purpose is that codec.
            let group_frame = self.group.as_mut().map(|group| {
                let mut frame = Vec::with_capacity(PLAYOUT_DRAIN);
                for _ in 0..2 {
                    if let Some(chunk) = group.mixer.mix_chunk() {
                        frame.extend(chunk);
                    } else {
                        frame.resize(frame.len() + PLAYOUT_DRAIN / 2, 0);
                    }
                }
                frame
            });
            if let Some(m) = self.media.as_mut() {
                let frame = if let Some(frame) = group_frame {
                    frame
                } else if let Some(pcm) = m.pcm.as_mut() {
                    // NOT recomputed here. The ceiling gives up one packet's worth per PACKET --
                    // its own contract -- and the arriving-packet sites are what maintain it.
                    // Recomputing on the 20ms tick as well is a second caller of a per-packet
                    // decision, which is inconsistent whether or not a fixture can catch it.
                    drain_playout(
                        &mut pcm.jitter,
                        &mut pcm.priming,
                        &mut pcm.priming_ticks,
                        pcm.packet_samps,
                        pcm.playout_cap,
                    )
                } else {
                    Vec::new()
                };
                m.playout_deadline = next_tick(m.playout_deadline, now, PLAYOUT_MS);
                self.outbox.push_back(Output::Playout(frame));
            }
        }
        if self.started && self.allocated && self.media.is_some() && now >= self.rtcp_deadline {
            self.emit_sender_reports(now, self.rtcp_wallclock_at(now));
            self.rtcp_deadline = next_tick(self.rtcp_deadline, now, RTCP_MS);
        }
        self.poll_audio_health(now);
        self.retransmit_group_reactions(now);
    }

    /// Media counters for this call. Additive for the call's life; sample twice for a rate.
    pub fn media_stats(&self) -> CallMediaStats {
        self.media_stats
    }

    /// Supply a decoder for a codec the core cannot implement.
    ///
    /// Consuming rather than a setter because it belongs at construction: installing a codec after
    /// media has flowed would mean the packets before it were silently discarded, and the honest
    /// answer to "this build has no decoder" is [`CallEvent::AudioSilent`], not a late rescue.
    ///
    /// Without one, a call whose peer turns out to speak standard Opus reports itself silent
    /// instead of pretending; with one, the same call is rescued and keeps a single playout
    /// schedule, because the decoded samples go into the very same jitter buffer.
    #[must_use]
    pub fn with_foreign_audio_codec(mut self, codec: Box<dyn ForeignAudioCodec>) -> Self {
        if let Some(m) = self.media.as_mut() {
            m.foreign_audio = Some(codec);
        }
        self
    }

    /// Supply a source of decoders for a group call, where one instance cannot serve everyone.
    ///
    /// [`Self::with_foreign_audio_codec`] hands over a single decoder, which is right for a 1:1
    /// call and impossible for a group: these codecs carry inter-frame state, so each participant
    /// needs their own. A runtime installs both -- the instance decodes the direct path, the
    /// factory mints one per participant as they are first heard.
    #[must_use]
    pub fn with_foreign_audio_codec_factory(
        mut self,
        factory: Box<dyn ForeignAudioCodecFactory>,
    ) -> Self {
        self.foreign_audio_factory = Some(factory);
        self
    }

    /// Swap the audio payload grammar without touching the negotiated RTP timing.
    ///
    /// Accepts only the MLow/Opus pair at 16 kHz, 60 ms and payload type 120, because that is the
    /// only pair whose [`AudioFormat`]s agree on every timing field: the swap changes no RTP header
    /// byte, so there is no discontinuity for the peer to recover from and nothing to re-signal.
    ///
    /// Idempotent, and latched after a small number of changes: evidence that keeps reversing is
    /// evidence that is wrong, and thrashing the decoder for a whole call is worse than picking one
    /// and reporting that the call is unhealthy.
    pub fn switch_audio_codec(
        &mut self,
        to: AudioCodec,
        source: CodecDecisionSource,
    ) -> Result<(), CodecSwitchError> {
        let packets_observed = self.media_stats.rtp_received;
        let m = self.media.as_mut().ok_or(CodecSwitchError::NoMedia)?;
        let from = m.active_format.codec;
        let Some(target) = m.audio.format.sibling_for(to) else {
            // Nothing to swap to. Asking for the codec already in use is a no-op either way, so it
            // is not an error just because this format has no sibling.
            return if from == to {
                Ok(())
            } else {
                Err(CodecSwitchError::NotASiblingFormat)
            };
        };
        // Compared as FORMATS, not codecs. MLOW's escape profile carries standard Opus inside
        // MLOW's container, so `Opus -> Opus` is a real change there: it takes the call off a
        // container the peer only parses if it speaks MLOW. Keyed on the codec, that switch would
        // read as idempotent and the call would keep sending an escape the peer cannot decode.
        if m.active_format == target {
            return Ok(());
        }
        // An encoded call's outbound bytes come from a source the APPLICATION built, fixed to one
        // codec for the life of the call. The switch still happens, because the RECEIVE side is
        // what it is for -- the peer's packets have to be decoded and labelled as the grammar they
        // are. The send side cannot follow: the engine can re-point its own encoder, not the
        // application's source. `on_encoded_audio` stops transmitting those bytes rather than
        // sending them under a profile that accepts any nonempty payload (the peer would hear
        // noise), and this event is how the application learns that only it can fix the call.
        let outbound_stranded = m.audio.io == AudioIo::Encoded && m.audio.format.codec != to;
        if self.media_stats.codec_switches >= CODEC_FLAP_LIMIT {
            return Err(CodecSwitchError::Latched);
        }
        m.active_format = target;
        // The MLow RTP profile also selects the marker/DTX framing on the send path, so it has to
        // follow the grammar rather than the negotiated profile.
        m.pipe
            .set_audio_mlow_profile(matches!(target.rtp_profile, AudioRtpProfile::Mlow));
        if let Some(pcm) = m.pcm.as_mut() {
            // The MLow decoder carries cross-frame predictor and synthesis history. Whatever it
            // built from the other codec's bytes is not a starting point for this one.
            #[cfg(feature = "voip-mlow")]
            pcm.decoder.reset();
            // A switch the CONTENT forced is one the probe only asks for after packets stopped
            // becoming audio, so what is queued is the concealment those packets produced -- up to
            // the playout cap of it, a quarter second of manufactured silence sitting in front of
            // the first correctly decoded frame. Rescuing a call and then making it wait out its
            // own failure is not a rescue. A negotiated switch keeps its buffer: nothing there
            // failed, and it is real audio the peer sent.
            if source == CodecDecisionSource::Content && !pcm.last_packet_decoded {
                pcm.jitter.clear();
                pcm.priming = true;
                pcm.priming_ticks = 0;
            }
        }
        self.media_stats.codec_switches = self.media_stats.codec_switches.saturating_add(1);
        #[cfg(feature = "tracing")]
        tracing::info!(call_id = %self.call_id, ?from, ?to, ?source, "voip audio codec switched");
        self.outbox
            .push_back(Output::Event(CallEvent::AudioCodecSwitched {
                from,
                to,
                source,
                packets_observed,
            }));
        if outbound_stranded {
            self.outbox
                .push_back(Output::Event(CallEvent::AudioCodecSourceIsFixed {
                    sending: self.media.as_ref().map_or(from, |m| m.audio.format.codec),
                    peer_expects: to,
                    source,
                }));
        }
        Ok(())
    }

    /// The codec currently decoding and encoding this call's audio.
    pub fn active_audio_codec(&self) -> Option<AudioCodec> {
        self.media.as_ref().map(|m| m.active_format.codec)
    }

    /// The full format currently on the wire, container included.
    ///
    /// Not the same question as [`Self::active_audio_codec`]: MLOW's escape and native Opus are
    /// both codec `Opus`, so a caller deciding whether the grammar changed has to ask this one.
    pub fn active_audio_format(&self) -> Option<AudioFormat> {
        self.media.as_ref().map(|m| m.active_format)
    }

    /// Fold in playout the consumer's sink refused after the engine produced it.
    ///
    /// The drop happens one layer out, in the drive loop that owns the application's channels, so
    /// the engine cannot observe it. Counting it keeps "the application heard nothing" separable
    /// from "the call carried nothing", which are different problems with different owners.
    pub fn note_audio_sink_dropped(&mut self, frames: u32) {
        self.media_stats.audio_sink_dropped =
            self.media_stats.audio_sink_dropped.saturating_add(frames);
    }

    /// Fold in inbound media the transport dropped before the engine could see it.
    ///
    /// The drop happens one crate out, in the relay read pump, so the engine cannot observe it
    /// directly. Reporting it keeps every discard on the receive path attributable to exactly one
    /// counter, which is the invariant that makes a silent call diagnosable.
    pub fn note_inbound_dropped(&mut self, packets: u32) {
        self.media_stats.inbound_pipe_dropped = self
            .media_stats
            .inbound_pipe_dropped
            .saturating_add(packets);
    }

    fn poll_audio_health(&mut self, now: Millis) {
        let Some(alarm) = self.health.poll(now, &self.media_stats) else {
            return;
        };
        let event = match alarm {
            AudioHealthAlarm::Silent {
                silent_for_ms,
                rtp_received,
                frames_produced,
                dominant_reason,
            } => CallEvent::AudioSilent {
                silent_for_ms,
                rtp_received,
                frames_produced,
                dominant_reason,
            },
            AudioHealthAlarm::Stalled { silent_for_ms } => {
                CallEvent::AudioReceptionStalled { silent_for_ms }
            }
        };
        #[cfg(feature = "tracing")]
        tracing::warn!(call_id = %self.call_id, ?event, "voip audio health");
        self.outbox.push_back(Output::Event(event));
    }

    fn retransmit_group_reactions(&mut self, now: Millis) {
        loop {
            let due = self
                .group
                .as_ref()
                .and_then(|group| group.pending_reactions.front())
                .is_some_and(|reaction| now >= reaction.next_at);
            if !due {
                break;
            }
            let Some(mut pending) = self
                .group
                .as_mut()
                .and_then(|group| group.pending_reactions.pop_front())
            else {
                break;
            };
            let Some(group) = self.group.as_mut() else {
                break;
            };
            let packet = group.app_data.protect_audio(&pending.payload);
            self.outbox.push_back(Output::Transmit(Bytes::from(packet)));
            pending.remaining = pending.remaining.saturating_sub(1);
            if pending.remaining != 0 {
                pending.next_at = next_tick(pending.next_at, now, APP_DATA_RETRANSMIT_MS);
                group.pending_reactions.push_back(pending);
            }
        }
    }

    fn rtcp_wallclock_at(&self, monotonic_now: Millis) -> u64 {
        self.rtcp_wallclock_origin_ms
            .saturating_add(monotonic_now.saturating_sub(self.rtcp_monotonic_origin))
    }

    fn announce_audio_rtcp_session(&mut self) {
        if !self.group_epoch_ready() {
            return;
        }
        let Some(m) = self.media.as_mut().filter(|m| !m.audio_rtcp_announced) else {
            return;
        };
        let packet = m.pipe.audio_source_description();
        m.audio_rtcp_announced = true;
        self.outbox.push_back(Output::Transmit(Bytes::from(packet)));
    }

    /// Audio and video share one wall-clock sample for lip sync.
    fn emit_sender_reports(&mut self, monotonic_ms: Millis, wallclock_ms: u64) {
        if !self.group_epoch_ready() {
            return;
        }
        let group_reports = self.group.as_mut().map(|group| {
            let audio = group
                .audio_reception
                .values_mut()
                .filter_map(|stats| stats.report(monotonic_ms))
                .collect::<Vec<_>>();
            let video = group
                .video_reception
                .values_mut()
                .filter_map(|stats| stats.report(monotonic_ms))
                .collect::<Vec<_>>();
            (audio, video)
        });
        let Some(m) = self.media.as_mut() else {
            return;
        };
        if let Some((audio_reports, video_reports)) = group_reports {
            if audio_reports.is_empty() {
                let report = m.pipe.audio_sender_report(wallclock_ms, None);
                self.outbox.push_back(Output::Transmit(Bytes::from(report)));
            } else {
                for reception in &audio_reports {
                    let report = m.pipe.audio_sender_report(wallclock_ms, Some(reception));
                    self.outbox.push_back(Output::Transmit(Bytes::from(report)));
                }
            }
            if let Some(v) = m.video.as_mut().filter(|v| v.active && !v.send_gated) {
                if video_reports.is_empty() {
                    let report = v.pipe.video_sender_report(wallclock_ms, None);
                    self.outbox.push_back(Output::Transmit(Bytes::from(report)));
                } else {
                    for reception in &video_reports {
                        let report = v.pipe.video_sender_report(wallclock_ms, Some(reception));
                        self.outbox.push_back(Output::Transmit(Bytes::from(report)));
                    }
                }
            }
            return;
        }
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
        let Ok(pkt) = unwrap_group_forwarding_packet(pkt) else {
            self.media_stats.forwarding_envelope_rejected = self
                .media_stats
                .forwarding_envelope_rejected
                .saturating_add(1);
            return;
        };
        let pkt = pkt.payload;
        match classify_relay_packet(pkt) {
            RelayPacketKind::Stun => self.on_stun(now, pkt),
            RelayPacketKind::Rtp => self.on_rtp(now, pkt),
            RelayPacketKind::Rtcp => self.on_rtcp(now, pkt),
            // Nothing the media plane speaks. Counted rather than ignored: a relay that starts
            // wrapping packets differently would otherwise present as a call with no media at all.
            RelayPacketKind::Other => {
                self.media_stats.relay_packet_unclassified =
                    self.media_stats.relay_packet_unclassified.saturating_add(1);
            }
        }
    }

    fn on_rtcp(&mut self, now: Millis, pkt: &[u8]) {
        if self.group.is_some() {
            if !self.group_epoch_ready() {
                // A requested epoch retires the old shared SRTCP keys together with SRTP. Besides
                // forged feedback, accepting a high old-key index here would advance the replay
                // window that survives rekey and suppress legitimate packets under the new epoch.
                return;
            }
            let (audio_ssrc, video_ssrc) = match self.media.as_ref() {
                Some(media) => (
                    media.pipe.send_ssrc(),
                    media.video.as_ref().map(|video| video.pipe.send_ssrc()),
                ),
                None => return,
            };
            let participant = match self
                .group
                .as_mut()
                .and_then(|group| group.registry.unprotect_rtcp(pkt))
            {
                Some(media) => media,
                None => return,
            };
            let Some(summary) = summarize_rtcp(&participant.payload) else {
                return;
            };
            if let Some(video_ssrc) = video_ssrc
                && requests_keyframe(&summary.feedback, video_ssrc)
                && let Some(video) = self.media.as_mut().and_then(|media| media.video.as_mut())
            {
                video.keyframe_required = true;
                self.announce_video_keyframe();
            }
            if let Some((sender, ntp_seconds, ntp_fraction)) =
                parse_sender_report_timing(&participant.payload)
                && let Some(group) = self.group.as_mut()
            {
                match group
                    .registry
                    .sender_report_stream(&participant.participant_id, sender)
                {
                    Some(GroupMediaStream::Audio) => group
                        .audio_reception
                        .entry(participant.participant_id.clone())
                        .or_default()
                        .observe_sender_report(sender, ntp_seconds, ntp_fraction, now),
                    Some(GroupMediaStream::Video) => group
                        .video_reception
                        .entry(participant.participant_id.clone())
                        .or_default()
                        .observe_sender_report(sender, ntp_seconds, ntp_fraction, now),
                    None => {}
                }
            }
            self.outbox
                .push_back(Output::Event(CallEvent::RtcpReceived {
                    reports_audio: summary.referenced_ssrcs.contains(&audio_ssrc),
                    reports_video: video_ssrc
                        .is_some_and(|ssrc| summary.referenced_ssrcs.contains(&ssrc)),
                    packet_types: summary.packet_types,
                    sender_ssrc: summary.sender_ssrc,
                    referenced_ssrcs: summary.referenced_ssrcs,
                    report_blocks: summary.report_blocks,
                    feedback: summary.feedback,
                }));
            return;
        }
        let mut newly_needs_keyframe = false;
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
                newly_needs_keyframe = true;
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
        // After the media borrow ends: the peer's RTCP asked for a keyframe, and
        // only the application's encoder can make one.
        if newly_needs_keyframe {
            self.announce_video_keyframe();
        }
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
        let matches_allocation = self
            .allocate_transaction_id
            .as_ref()
            .is_some_and(|expected| stun::stun_transaction_id(pkt) == Some(expected.as_slice()));
        // The relay acknowledged the current allocate; stale replies from a superseded credential
        // generation must not cancel its timeout or terminate the call.
        if self.allocate_pending && matches_allocation && stun::is_allocate_or_binding_success(pkt)
        {
            let was_allocated = self.allocated;
            self.allocate_pending = false;
            self.allocated = true;
            self.allocate_deadline = NEVER;
            #[cfg(feature = "tracing")]
            tracing::debug!(call_id = %self.call_id, "voip relay allocated");
            if !was_allocated {
                self.outbox
                    .push_back(Output::Event(CallEvent::RelayAllocated));
                if self.media.is_some() {
                    self.rtcp_deadline = now + RTCP_MS;
                    // Inbound media only becomes possible here, so this is the earliest instant at
                    // which "no audio has arrived" means anything -- for a call that has been
                    // ANSWERED. An outgoing call allocates its relay when the server acks the
                    // offer, while the callee is still ringing and owes us nothing, so arming here
                    // would report a stall three seconds into every normal ring. The caller arms on
                    // the `<accept>` instead (`rekey_recv`), or on its first inbound packet if
                    // media somehow arrives without one, so the watchdog can never be left
                    // permanently disarmed.
                    //
                    // Direct calls only either way: a group call returns through `on_group_rtp`,
                    // which moves the same media counters but feeds neither the arrival nor the
                    // production side of the watchdog -- a mixer with several participants is a
                    // different question from "is this stream carrying audio" -- so arming it there
                    // would report every healthy group call as stalled.
                    // An incoming call was answered before its media plane existed, so allocation
                    // is the whole condition. An outgoing one arms here too when the accept already
                    // arrived -- see `peer_answered`, which handles the other order.
                    if self.group.is_none()
                        && (self.direction == CallDirection::Incoming || self.peer_has_answered)
                    {
                        self.health.media_started(now);
                    }
                    self.announce_audio_rtcp_session();
                }
            }
            return;
        }
        // A complete allocate-error (a parsed ERROR-CODE) terminates the call; STUN-typed garbage
        // whose error code cannot be parsed is ignored rather than hanging up.
        if self.allocate_pending
            && matches_allocation
            && self.allocate_deadline != NEVER
            && stun::is_allocate_error(pkt)
            && let Some(code) = stun::parse_stun_error_code(pkt)
        {
            self.allocate_deadline = NEVER;
            self.allocate_pending = false;
            self.allocated = false;
            self.terminated = true;
            #[cfg(feature = "tracing")]
            tracing::debug!(call_id = %self.call_id, code, "voip relay allocate failed");
            self.outbox
                .push_back(Output::Event(CallEvent::RelayAllocateFailed(code)));
        }
    }

    fn on_rtp(&mut self, now: Millis, pkt: &[u8]) {
        // Demux by payload type BEFORE unprotect: audio and video share E2E keys but have
        // distinct SSRCs/ROC trackers, so feeding a video packet through the audio pipeline
        // would fail its MI tag at best and desync at worst.
        let Some(wire_header) = parse_rtp_header(pkt) else {
            // Version bits and two bytes are enough for `classify_relay_packet` to call this RTP,
            // which is not enough to read a header from. Counted where an unreadable datagram is
            // counted: uncounted, a stream of these leaves every discard counter at zero and the
            // watchdog reports a reception that never started.
            self.media_stats.relay_packet_unclassified =
                self.media_stats.relay_packet_unclassified.saturating_add(1);
            return;
        };
        if self.group.is_some() {
            self.on_group_rtp(now, pkt, wire_header);
            return;
        }
        // Set by the content probe below and applied once the media borrow has ended: the switch
        // needs the whole engine, and holding both borrows would be a borrow-checker fight for no
        // behavioural gain. The encoded path keeps its own, since it returns before the tail. Not
        // gated on the built-in codec: the standard-Opus PCM path asks too, and it exists in a build
        // that has no MLOW decoder at all.
        let mut probe_verdict: Option<AudioCodec> = None;
        let Some(m) = self.media.as_mut() else {
            return;
        };
        if wire_header.payload_type == RTP_PAYLOAD_TYPE_H264 {
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
                        sender: None,
                        device: None,
                        pid: None,
                    }));
                }
            }
            return;
        }
        // The watchdog counts ARRIVALS, not authenticated packets, and that distinction is the
        // whole point: wrong recv keys make every packet fail below, and a watchdog fed after that
        // point would see an empty window and conclude the peer is simply not speaking. This is the
        // deafest failure the receive path has, so it is the one the alarm must reach. Counted
        // BEFORE the payload-type gate for the same reason: a peer that switched profiles is
        // sending audio RTP, so reading it as "nothing arrived" would report the transport alarm
        // and bury the one reason the counters can actually name. Video has already returned above.
        // A caller whose media arrives without a `<accept>` ever reaching the drive loop would
        // otherwise carry a disarmed watchdog for the whole call. Idempotent.
        if self.group.is_none() {
            self.health.media_started(now);
        }
        self.health.on_rtp(now);
        // Gated on the ACTIVE format, which is what every step below decodes with. Read from the
        // negotiated one, an MLow call switched to standard Opus would keep admitting PT 121 --
        // MLow's redundancy type, which standard Opus does not have -- and hand the RED wrapper to
        // the Opus decoder as if it were an Opus frame.
        if !m
            .active_format
            .accepts_rtp_payload_type(wire_header.payload_type)
        {
            // A payload type outside the active profile is indistinguishable from a peer who
            // stopped talking unless it is counted: this discard is how a profile mismatch hides.
            self.media_stats.rtp_payload_type_unexpected = self
                .media_stats
                .rtp_payload_type_unexpected
                .saturating_add(1);
            return;
        }
        let Some((header, payload)) = m.pipe.unprotect_audio(pkt) else {
            // Wrong recv keys, wrong peer LID or a desynced ROC all land here, and every one of
            // them makes the call totally deaf with no other symptom.
            self.media_stats.srtp_unprotect_failed =
                self.media_stats.srtp_unprotect_failed.saturating_add(1);
            return;
        };
        self.media_stats.rtp_received = self.media_stats.rtp_received.saturating_add(1);
        // A renumbered stream restarts the timestamp sequence, so differences across the change are
        // not comparable and the probe's streak has to start over. Its decision does not: a peer
        // must not be able to reopen the codec question by changing its SSRC.
        if m.audio_reception.ssrc() != Some(header.ssrc) {
            m.codec_probe.stream_restarted();
        }
        m.audio_reception.observe(
            header.ssrc,
            header.sequence_number,
            header.timestamp,
            now,
            m.audio.format.rtp_clock_rate,
        );
        // Set when the payload reaching the codec is ciphertext whose tag did not authenticate. Such
        // bytes are passed through by contract but they are NOT evidence about anything: they are
        // whatever the wrong key produced, and three of them structurally resembling 60ms Opus would
        // latch a permanent codec switch that outlives the authentication failure itself.
        let mut unauthenticated = false;
        // SFrame on: use the GCM-decrypted bytes; otherwise the SRTP payload is already plain codec.
        let encoded = match m.sframe.as_ref().map(|s| s.decrypt(&payload)) {
            Some(SframeIn::Decrypted(plain)) => {
                m.sframe_authenticated = true;
                plain
            }
            Some(SframeIn::AuthFailed) if m.sframe_authenticated => {
                // The frame WAS SFrame-wrapped and its tag did not authenticate. Passing the
                // payload through is the documented contract, but doing it silently would hand
                // ciphertext to the codec and call the result a codec problem.
                self.media_stats.sframe_decrypt_failed =
                    self.media_stats.sframe_decrypt_failed.saturating_add(1);
                unauthenticated = true;
                payload
            }
            // A peer that does not SFrame-wrap at all is a supported mode, not a failure: counting
            // it would make every packet of a healthy call report one. `AuthFailed` before anything
            // has authenticated lands here too -- see `sframe_authenticated`.
            Some(SframeIn::AuthFailed | SframeIn::Plaintext) | None => payload,
        };
        let codec = m.active_format.inbound_codec(header.payload_type, &encoded);
        // Asked HERE, before the Opus branch returns. That branch decodes inbound audio perfectly
        // well without the call-wide format ever moving, which leaves `on_mic` encoding MLOW at a
        // peer that speaks native Opus: inbound fine, outbound silent, and no counter describing it
        // because nothing failed. Native CELT reaching this point is confidently native -- the
        // escape discriminator just said so -- and it agrees with the negotiated cadence, which is
        // exactly the corroboration the probe wants. Unauthenticated bytes are excluded as
        // everywhere else: they are not evidence.
        //
        // PCM only: the encoded branch below asks for itself, and the probe must be asked ONCE per
        // packet. Asked twice, three consecutive agreements are reached in two packets and the whole
        // requirement is halved.
        if m.audio.io == AudioIo::Pcm
            && !unauthenticated
            && codec == AudioCodec::Opus
            && m.active_format.codec == AudioCodec::Mlow
            && !m.active_format.payload_is_mlow_escape(&encoded)
        {
            probe_verdict = observe_codec_content(m, &encoded);
        }
        if m.audio.io == AudioIo::Pcm && codec == AudioCodec::Opus {
            // The peer speaks a codec the core does not implement. With a platform decoder the
            // samples join the SAME jitter buffer the MLow path feeds, so there is one playout
            // schedule regardless of which grammar produced the audio.
            if let Some(foreign) = m.foreign_audio.as_mut() {
                m.foreign_pcm.clear();
                // Two different things reach here and only one of them is an RFC 6716 packet.
                //
                // Inside the MLOW profile, `(b & 0xC0) == 0xC0` is MLOW's in-profile escape, and its
                // first byte is NOT an Opus TOC: this crate writes it as
                // `0xC0 | mode << 2 | stereo << 1 | multi` (see `packetize_opus_for_mlow`). Handing
                // it to a stock decoder reads the duration, the stereo flag and the frame count out
                // of the wrong bits. Restore the RFC header first, exactly as the consumer-side
                // `decode_mlow_escape` used to before the engine took this over.
                let mut encoded = encoded;
                if m.active_format.payload_is_mlow_escape(&encoded)
                    && let Err(e) = depacketize_opus_from_mlow(&mut encoded)
                {
                    log::debug!("voip: malformed MLOW Opus escape: {e}");
                    self.media_stats.audio_frames_concealed =
                        self.media_stats.audio_frames_concealed.saturating_add(1);
                    return;
                }
                let samples = super::opus_packet_shape(&encoded)
                    .and_then(|shape| shape.total_samples(m.active_format.rtp_clock_rate))
                    .unwrap_or(m.active_format.samples_per_frame)
                    as usize;
                let mut decoded_ok = false;
                match foreign.decode(&encoded, &mut m.foreign_pcm) {
                    Ok(()) => {
                        decoded_ok = true;
                        // Not credited when the tag did not authenticate, for the reason the other
                        // production sites are not: these bytes are whatever the wrong key produced,
                        // and a decoder succeeding on them says nothing about the peer. The samples
                        // still reach playout -- silencing them is a behaviour change this finding
                        // did not ask for -- but they must not reset the silence window or count
                        // toward `audio_produced()`, which is what let a sustained tag failure
                        // suppress the alarm that names it.
                        if !unauthenticated {
                            self.media_stats.foreign_frames_decoded =
                                self.media_stats.foreign_frames_decoded.saturating_add(1);
                            self.health.on_audio_produced();
                        }
                    }
                    Err(e) => {
                        log::debug!("voip: foreign audio decode failed: {e}");
                        m.foreign_pcm.clear();
                        foreign.conceal(samples, &mut m.foreign_pcm);
                        self.media_stats.audio_frames_concealed =
                            self.media_stats.audio_frames_concealed.saturating_add(1);
                    }
                }
                if let Some(pcm) = m.pcm.as_mut() {
                    pcm.packet_samps = samples.max(1);
                    pcm.last_packet_decoded = decoded_ok;
                    pcm.jitter.extend(m.foreign_pcm.iter().copied());
                    // Same feed-side ceiling the MLOW path enforces below. A rescued stream is
                    // still a stream: a burst arriving between two playout ticks must not grow the
                    // buffer without bound just because the samples came from another codec.
                    pcm.playout_cap =
                        effective_playout_cap(pcm.playout_cap, pcm.packet_samps, pcm.jitter.len());
                    if pcm.jitter.len() > pcm.playout_cap {
                        let drop_n = pcm.jitter.len() - pcm.playout_cap;
                        pcm.jitter.drain(..drop_n);
                        self.media_stats.playout_trimmed_samples = self
                            .media_stats
                            .playout_trimmed_samples
                            .saturating_add(drop_n as u32);
                    }
                }
                // This branch returns before the tail that normally applies it, and a verdict
                // dropped here is the outbound half of the call staying on the wrong codec.
                self.apply_codec_verdict(probe_verdict);
                return;
            }
            // No decoder for it. Surface the payload so a shell that has one can play it, and let
            // the health watchdog report the call as silent rather than have it look like a peer
            // who is not speaking.
            self.media_stats.audio_frames_without_decoder = self
                .media_stats
                .audio_frames_without_decoder
                .saturating_add(1);
            self.outbox
                .push_back(Output::Event(CallEvent::ForeignAudio(Bytes::from(encoded))));
            self.apply_codec_verdict(probe_verdict);
            return;
        }
        if m.audio.io == AudioIo::Encoded {
            // An encoded call never decodes anything here, so "nothing became audio" -- the PCM
            // path's trigger for asking -- is true of every packet and cannot be the condition. Ask
            // whenever the call is still on MLOW: the peer's capability can be absent, or can lose
            // the race with its first packets, and then the sink would be handed native Opus
            // labelled `Mlow` with nothing to notice it. The arithmetic that makes the probe safe
            // is about the packets, not about who decodes them.
            // The switch itself runs after this borrow ends, so `active_format` is still the old
            // one here. Label the frame with what the verdict implies rather than what has not been
            // applied yet: a sink that depacketizes by `format.rtp_profile` would otherwise treat
            // this one valid transition packet as an MLOW escape and corrupt it. Only a verdict with
            // a sibling relabels -- one without is a switch that will be refused anyway.
            let verdict = (!unauthenticated)
                .then(|| observe_codec_content(m, &encoded))
                .flatten();
            // A packet the discriminator proves is native gets the native label immediately, before
            // any verdict: the probe needs three of them, and labelling the first two with MLOW's
            // container tells the sink to undo an escape that is not there. Waiting for
            // corroboration is right for the call-wide SWITCH, which changes what we send; it is
            // wrong for describing a packet whose grammar this one packet already settles.
            let native_now =
                codec == AudioCodec::Opus && !m.active_format.payload_is_mlow_escape(&encoded);
            let (codec, active_format) = match verdict
                .or(native_now.then_some(AudioCodec::Opus))
                .and_then(|c| m.active_format.sibling_for(c).map(|format| (c, format)))
            {
                Some(pair) => pair,
                None => (codec, m.active_format),
            };
            // Counted where the engine hands the frame over, which is the last point it can see. A
            // sink that refuses it is reported separately, through `note_audio_sink_dropped`.
            // Both of these, for one reason: `audio_produced()` sums `audio_frames_delivered`, so
            // counting a failed tag's ciphertext there reports it as produced audio in the public
            // statistics even once the watchdog stops believing it. The frame IS handed over -- the
            // encoded API promises that -- and `sframe_decrypt_failed` is the counter that says so.
            // Not for bytes whose tag did not authenticate. Handing them over is the encoded API's
            // contract, but they are not codec plaintext and calling them produced audio keeps
            // `window_produced` nonzero through a run of failures -- so `AudioSilent` never fires
            // and `AuthenticationFailing`, the one reason that names the real cause, can never be
            // reached. The sink still gets the bytes; the watchdog just stops being told they are
            // audio.
            if !unauthenticated {
                self.media_stats.audio_frames_delivered =
                    self.media_stats.audio_frames_delivered.saturating_add(1);
                self.health.on_audio_produced();
            }
            self.outbox
                .push_back(Output::EncodedAudio(EncodedAudioFrame {
                    format: active_format,
                    codec,
                    data: Bytes::from(encoded),
                    payload_type: header.payload_type,
                    sequence_number: header.sequence_number,
                    timestamp: header.timestamp,
                    marker: header.marker,
                    sender: None,
                    device: None,
                    pid: None,
                }));
            // Applied here rather than at the tail below, which only the PCM path reaches -- and in
            // a build without the built-in decoder is the only path there is.
            self.apply_codec_verdict(verdict);
            return;
        }
        debug_assert_eq!(m.active_format.codec, AudioCodec::Mlow);
        #[cfg(feature = "voip-mlow")]
        let Some(pcm) = m.pcm.as_mut() else {
            return;
        };
        #[cfg(not(feature = "voip-mlow"))]
        return;
        // MLow decode (f32 [-1,1]) -> i16, appended to the playout buffer.
        #[cfg(feature = "voip-mlow")]
        pcm.decoder
            .set_redundancy(i32::from(header.payload_type == RTP_PAYLOAD_TYPE_MLOW_RED));
        #[cfg(feature = "voip-mlow")]
        let decode_report = {
            let decoded = pcm.decoder.decode(&encoded);
            // Declared, not decoded; see `MlowDecoder::last_packet_samps`.
            pcm.packet_samps = pcm.decoder.last_packet_samps();
            for s in decoded {
                pcm.jitter
                    .push_back((s * 32767.0).clamp(-32768.0, 32767.0) as i16);
            }
            pcm.decoder.take_frame_report()
        };
        #[cfg(feature = "voip-mlow")]
        {
            // Not for bytes whose tag did not authenticate, matching the gate the encoded path puts
            // on `audio_frames_delivered` and the watchdog credit below: `audio_produced()` sums
            // all three, so an ungated one lets the public snapshot vouch for frames the watchdog
            // has already stopped believing. Unlike its two siblings this one is a contract kept
            // rather than a leak observed -- measured over 7840 unauthenticated frames, MLow never
            // once reported `decoded`, because what reaches it is the SFrame framing rather than a
            // TOC and it answers off-point, SID or concealment. That is the decoder's behaviour,
            // not this function's guarantee, so the gate states the guarantee here.
            if !unauthenticated {
                self.media_stats.audio_frames_decoded = self
                    .media_stats
                    .audio_frames_decoded
                    .saturating_add(u32::from(decode_report.decoded));
            }
            self.media_stats.audio_frames_concealed = self
                .media_stats
                .audio_frames_concealed
                .saturating_add(u32::from(decode_report.concealed));
            self.media_stats.mlow_off_point_dropped = self
                .media_stats
                .mlow_off_point_dropped
                .saturating_add(u32::from(decode_report.off_point));
            self.media_stats.mlow_inactive_or_sid = self
                .media_stats
                .mlow_inactive_or_sid
                .saturating_add(u32::from(decode_report.inactive_or_sid));
            // A SID is the peer TELLING us it is silent, and the decoder handled it: a muted peer
            // is a healthy stream, not a call that cannot turn packets into sound. Counted as
            // production so the silence alarm does not fire through a long mute -- the failures
            // this alarm exists for report `off_point` or concealment, never `inactive_or_sid`.
            // `!unauthenticated` for the reason the encoded path has it: random ciphertext can be
            // classified as a SID or even decode to something, and crediting that resets the silence
            // window, so a sustained tag failure would suppress the very alarm that names it.
            if !unauthenticated && (decode_report.decoded > 0 || decode_report.inactive_or_sid > 0)
            {
                self.health.on_audio_produced();
            }
            // A SID counts as decoded here too: the peer told us it is silent and the decoder
            // honoured it, so the queue holds what the peer meant rather than a failure's residue.
            pcm.last_packet_decoded =
                decode_report.decoded > 0 || decode_report.inactive_or_sid > 0;
        }
        // Bound the buffer on the feed side too: a burst of inbound packets arriving between two 20ms
        // playout ticks must not grow `jitter` without limit (drain_playout's cap only runs on a
        // tick). Drop oldest past the same ceiling the drain path uses.
        #[cfg(feature = "voip-mlow")]
        {
            pcm.playout_cap =
                effective_playout_cap(pcm.playout_cap, pcm.packet_samps, pcm.jitter.len());
            if pcm.jitter.len() > pcm.playout_cap {
                let drop_n = pcm.jitter.len() - pcm.playout_cap;
                pcm.jitter.drain(..drop_n);
                // Trimming discards speech that was legally queued. Counted so a call that sounds
                // chopped can be told apart from one that never decoded anything.
                self.media_stats.playout_trimmed_samples = self
                    .media_stats
                    .playout_trimmed_samples
                    .saturating_add(drop_n as u32);
            }
        }
        // Nothing became audio. Ask whether the peer's own two statements about this packet -- the
        // duration its Opus header would declare, and the step its RTP timestamps actually advance
        // by -- agree with each other. They cannot agree for any packet this decoder accepts (see
        // `codec_probe`), so a run of agreements is evidence of a grammar we are not speaking.
        #[cfg(feature = "voip-mlow")]
        if decode_report.decoded == 0
            && !unauthenticated
            && let Some(m) = self.media.as_mut()
        {
            probe_verdict = observe_codec_content(m, &encoded);
        }
        #[cfg(feature = "voip-mlow")]
        self.apply_codec_verdict(probe_verdict);
    }

    /// Act on what the content probe concluded, once the media borrow it needed has ended.
    ///
    /// Content contradicting negotiation is a statement about our model of the peer, not just about
    /// this call, which is why the event names the source.
    fn apply_codec_verdict(&mut self, verdict: Option<AudioCodec>) {
        if let Some(codec) = verdict
            && let Err(e) = self.switch_audio_codec(codec, CodecDecisionSource::Content)
        {
            log::debug!("voip: inbound bytes indicate {codec:?}, not switching: {e}");
        }
    }

    fn on_group_rtp(&mut self, now: Millis, pkt: &[u8], wire_header: crate::voip::rtp::RtpHeader) {
        let Some(audio) = self.media.as_ref().map(|media| media.audio) else {
            return;
        };
        if !self.group_epoch_ready() {
            // An authoritative rekey request retires the old shared epoch immediately. Until the
            // requested transaction installs, accepting inbound RTP would let a removed member
            // forge audio, video, or reactions with the still-resident receiver pipelines.
            return;
        }
        let Some(group) = self.group.as_mut() else {
            return;
        };
        if wire_header.payload_type == RTP_PAYLOAD_TYPE_H264 {
            let Some(video) = group.registry.unprotect_video(pkt) else {
                return;
            };
            group
                .video_reception
                .entry(video.participant_id.clone())
                .or_default()
                .observe(
                    video.header.ssrc,
                    video.header.sequence_number,
                    video.header.timestamp,
                    now,
                    VIDEO_CLOCK_RATE,
                );
            let orientation = group
                .video_orientations
                .get(&video.device_jid)
                .or_else(|| group.video_orientations.get(&video.user_jid))
                .copied()
                .unwrap_or_default();
            for access_unit in video.access_units {
                let keyframe = au_is_keyframe(&access_unit);
                self.outbox.push_back(Output::VideoPlayout(VideoFrame {
                    data: access_unit,
                    keyframe,
                    orientation,
                    sender: Some(video.user_jid.clone()),
                    device: Some(video.device_jid.clone()),
                    pid: video.pid,
                }));
            }
            return;
        }
        if wire_header.payload_type == RTP_PAYLOAD_TYPE_APP_DATA {
            let Some(participant) = group.registry.unprotect_app_data(pkt) else {
                return;
            };
            let Ok(reactions) = app_data::decode_reactions(&participant.payload) else {
                return;
            };
            let last_seen = group
                .reaction_last_seen
                .entry(participant.participant_id)
                .or_insert(ReactionWatermark {
                    pid: participant.pid,
                    transaction_id: 0,
                });
            if last_seen.pid != participant.pid {
                last_seen.pid = participant.pid;
                last_seen.transaction_id = 0;
            }
            for reaction in reactions {
                if reaction.transaction_id <= last_seen.transaction_id {
                    continue;
                }
                last_seen.transaction_id = reaction.transaction_id;
                let emoji = (!reaction.emoji.is_empty()).then_some(reaction.emoji);
                self.outbox.push_back(Output::Event(CallEvent::Reaction {
                    participant: participant.user_jid.clone(),
                    device: participant.device_jid.clone(),
                    pid: participant.pid,
                    removed: emoji.is_none(),
                    emoji,
                }));
            }
            return;
        }
        // The group receive path carries the same counters as the direct one. Without them
        // `media_stats()` reports a healthy group call as having received nothing at all, while
        // group-specific counters like `forwarding_envelope_rejected` do move -- a snapshot that
        // contradicts itself, and no way to attribute a group call's silence.
        if !audio
            .format
            .accepts_rtp_payload_type(wire_header.payload_type)
        {
            self.media_stats.rtp_payload_type_unexpected = self
                .media_stats
                .rtp_payload_type_unexpected
                .saturating_add(1);
            return;
        }
        // `unprotect_audio` answers `None` for two different things, and only one of them is a
        // failing tag: a packet from an SSRC absent from the roster -- a straggler from a
        // participant an authoritative update just removed -- is turned away at the route lookup
        // before SRTP is asked anything. Counting that as `srtp_unprotect_failed` reports a key
        // problem for a packet no key was ever tried on, and now that the silence reason weighs
        // that counter against `rtp_received`, a departing participant's tail could name the whole
        // window an authentication failure.
        let participant = match group.registry.unprotect_audio(pkt) {
            Ok(participant) => participant,
            Err(GroupAudioReject::Unprotect) => {
                self.media_stats.srtp_unprotect_failed =
                    self.media_stats.srtp_unprotect_failed.saturating_add(1);
                return;
            }
            // Deliberately uncounted rather than given a counter of its own. A packet with no route
            // is the ordinary tail of a participant an authoritative update just removed -- an
            // expected event, not a fault -- and the finding here is that it must not be reported as
            // a failing tag, which it no longer is. A counter for expected drops is its own change.
            Err(GroupAudioReject::Unroutable) => {
                log::trace!("voip: group audio for an SSRC no longer on the roster");
                return;
            }
        };
        self.media_stats.rtp_received = self.media_stats.rtp_received.saturating_add(1);
        // A stream that restarts is a new stream, and the probe's three-packet requirement means
        // nothing if agreements from the replacement can finish a streak the retired one began. The
        // direct path retires its evidence the same way, for the same reason.
        if group
            .audio_reception
            .get(&participant.participant_id)
            .and_then(RtpReceptionStats::ssrc)
            .is_some_and(|ssrc| ssrc != participant.header.ssrc)
            && let Some(probe) = group.codec_probes.get_mut(&participant.participant_id)
        {
            probe.stream_restarted();
        }
        group
            .audio_reception
            .entry(participant.participant_id.clone())
            .or_default()
            .observe(
                participant.header.ssrc,
                participant.header.sequence_number,
                participant.header.timestamp,
                now,
                audio.format.rtp_clock_rate,
            );
        // The latched promotion is consulted FIRST, because the escape marker cannot tell the two
        // apart: `is_mlow_embedded_opus` tests the top two bits, and every native Opus CELT config
        // (24..=31) sets them -- a native 60ms CELT packet starts 0xC3. Read marker-first, a
        // promoted participant's native packet is called an escape and has a TOC that was never
        // rewritten rewritten again, which is a decode failure rather than a mislabel.
        let promoted = group
            .foreign_participants
            .contains(&participant.participant_id);
        let codec = if promoted {
            AudioCodec::Opus
        } else {
            audio
                .format
                .inbound_codec(participant.header.payload_type, &participant.payload)
        };
        // What says the TOC was rewritten and has to be restored. Never true for a promoted
        // participant -- it was promoted precisely because its bytes are native -- and decided for
        // everyone else by `payload_is_mlow_escape`, because the marker alone cannot tell an escape
        // from native CELT and calling native CELT an escape rewrites a TOC that was never
        // rewritten. A participant sending native CELT from its very first packet is never
        // classified MLOW, so the probe would never see it: without this it would be corrupted for
        // the whole call with nothing able to notice.
        let escaped = !promoted && audio.format.payload_is_mlow_escape(&participant.payload);
        // A packet the discriminator proved native latches the participant immediately, without the
        // probe: the probe abstains on it (its grammar is already Opus) and would never latch, so
        // this participant's FIRST SILK packet -- carrying no CELT marker, classified MLOW -- would
        // be dropped, and an encoder alternating modes would keep re-losing it. An escape latches
        // nothing: it is MLOW's container and says nothing about what the peer negotiated.
        if codec == AudioCodec::Opus
            && !escaped
            && group
                .foreign_participants
                .insert(participant.participant_id.clone())
        {
            // Newly latched, so the same cleanup the probe's verdict does. A native-Opus
            // participant whose first packets happen to be SILK was classified MLOW until this
            // moment, and those packets left two things behind: a decoder holding predictor state
            // built from another codec's bytes, and their concealment queued in the mixer. Without
            // this, the CELT audio that settled the question plays out behind that manufactured
            // silence -- the participant is rescued and then made to wait out its own failure.
            #[cfg(feature = "voip-mlow")]
            group.decoders.remove(&participant.participant_id);
            group.mixer.reset(&participant.participant_id);
        }
        // A promoted participant sends NATIVE Opus, so the frame must not be described by the
        // container the call negotiated: a sink that depacketizes by `format.rtp_profile` would
        // read the untouched TOC as an MLOW escape and corrupt it. The escape keeps `audio.format`,
        // because for it the MLOW profile is the truth.
        let frame_format = if codec == AudioCodec::Opus && !escaped {
            audio
                .format
                .sibling_for(AudioCodec::Opus)
                .unwrap_or(audio.format)
        } else {
            audio.format
        };
        if audio.io == AudioIo::Encoded {
            // An encoded group call decodes nothing here, so "the MLow decoder produced nothing" --
            // the condition the PCM path probes under -- is unreachable, and the branch returns
            // before it besides. Without asking here, a participant sending native Opus is labelled
            // MLOW to the sink forever: the set that would correct it is only ever populated below.
            let (codec, frame_format) = if promoted {
                (codec, frame_format)
            } else {
                match observe_group_codec_content(group, &participant, audio.format, codec) {
                    Some(verdict) => (
                        verdict,
                        audio.format.sibling_for(verdict).unwrap_or(audio.format),
                    ),
                    None => (codec, frame_format),
                }
            };
            self.media_stats.audio_frames_delivered =
                self.media_stats.audio_frames_delivered.saturating_add(1);
            self.outbox
                .push_back(Output::EncodedAudio(EncodedAudioFrame {
                    format: frame_format,
                    codec,
                    data: Bytes::from(participant.payload),
                    payload_type: participant.header.payload_type,
                    sequence_number: participant.header.sequence_number,
                    timestamp: participant.header.timestamp,
                    marker: participant.header.marker,
                    sender: Some(participant.user_jid),
                    device: Some(participant.device_jid),
                    pid: participant.pid,
                }));
            return;
        }
        if codec == AudioCodec::Opus {
            // Decoded with this participant's OWN injected decoder, not the direct path's single
            // instance: these codecs carry inter-frame state and two speakers through one corrupts
            // both. Only when no decoder can be made does the frame go out as an event -- the same
            // honest answer `ForeignAudio` gives on the direct path. Reporting that while a codec
            // WAS installed left the speaker silent for that participant with the counter naming a
            // cause that was not true.
            let participant_id = participant.participant_id.clone();
            if !group.foreign_decoders.contains_key(&participant_id)
                && let Some(factory) = self.foreign_audio_factory.as_ref()
                && let Some(codec) = factory.create()
            {
                group.foreign_decoders.insert(participant_id.clone(), codec);
            }
            let Some(decoder) = group.foreign_decoders.get_mut(&participant_id) else {
                self.media_stats.audio_frames_without_decoder = self
                    .media_stats
                    .audio_frames_without_decoder
                    .saturating_add(1);
                self.outbox
                    .push_back(Output::Event(CallEvent::ForeignGroupAudio(
                        EncodedAudioFrame {
                            format: frame_format,
                            codec,
                            data: Bytes::from(participant.payload),
                            payload_type: participant.header.payload_type,
                            sequence_number: participant.header.sequence_number,
                            timestamp: participant.header.timestamp,
                            marker: participant.header.marker,
                            sender: Some(participant.user_jid),
                            device: Some(participant.device_jid),
                            pid: participant.pid,
                        },
                    )));
                return;
            };
            // The escape's first byte is not an RFC TOC; restore it before a stock decoder sees it,
            // exactly as the direct path does.
            let mut encoded = participant.payload;
            if escaped && let Err(e) = depacketize_opus_from_mlow(&mut encoded) {
                log::debug!("voip: malformed MLOW Opus escape from a participant: {e}");
                self.media_stats.audio_frames_concealed =
                    self.media_stats.audio_frames_concealed.saturating_add(1);
                return;
            }
            let mut pcm = Vec::new();
            match decoder.decode(&encoded, &mut pcm) {
                Ok(()) => {
                    self.media_stats.foreign_frames_decoded =
                        self.media_stats.foreign_frames_decoded.saturating_add(1);
                }
                Err(e) => {
                    log::debug!("voip: participant foreign audio decode failed: {e}");
                    pcm.clear();
                    decoder.conceal(audio.format.samples_per_frame as usize, &mut pcm);
                    self.media_stats.audio_frames_concealed =
                        self.media_stats.audio_frames_concealed.saturating_add(1);
                }
            }
            group.mixer.push(&participant_id, &pcm);
            // Not a `return`: without the built-in codec nothing follows, and clippy denies the
            // dead one. The MLOW branch below is the whole remainder either way.
        } else {
            #[cfg(feature = "voip-mlow")]
            {
                let decoder = group
                    .decoders
                    .entry(participant.participant_id.clone())
                    .or_insert_with(mlow::MlowDecoder::new);
                decoder.set_redundancy(i32::from(
                    participant.header.payload_type == RTP_PAYLOAD_TYPE_MLOW_RED,
                ));
                let pcm = decoder
                    .decode(&participant.payload)
                    .iter()
                    .map(|sample| (sample * 32767.0).clamp(-32768.0, 32767.0) as i16)
                    .collect::<Vec<_>>();
                let report = decoder.take_frame_report();
                self.media_stats.audio_frames_decoded = self
                    .media_stats
                    .audio_frames_decoded
                    .saturating_add(u32::from(report.decoded));
                self.media_stats.audio_frames_concealed = self
                    .media_stats
                    .audio_frames_concealed
                    .saturating_add(u32::from(report.concealed));
                self.media_stats.mlow_off_point_dropped = self
                    .media_stats
                    .mlow_off_point_dropped
                    .saturating_add(u32::from(report.off_point));
                self.media_stats.mlow_inactive_or_sid = self
                    .media_stats
                    .mlow_inactive_or_sid
                    .saturating_add(u32::from(report.inactive_or_sid));
                // Nothing became audio, so ask this participant's own two statements about the packet
                // whether the negotiation describes it -- the same question the direct path asks, and
                // the only way to catch NATIVE Opus here: it carries no escape marker, so classification
                // alone will call it MLOW forever and the participant stays silent with a decoder
                // installed and idle.
                let mut pcm = pcm;
                if report.decoded == 0
                    && observe_group_codec_content(group, &participant, audio.format, codec)
                        == Some(AudioCodec::Opus)
                {
                    // Whatever this decoder built out of the other codec's bytes is not a starting
                    // point for anything, and the participant will not come back to it.
                    group.decoders.remove(&participant.participant_id);
                    // Nor is what it already queued: every one of those samples is concealment for
                    // a packet that failed, and up to the mixer's capacity of it would play out in
                    // front of the rescued audio. Rescuing a participant and then making them wait
                    // out their own failure is not a rescue; the direct path clears for this too.
                    group.mixer.reset(&participant.participant_id);
                    pcm.clear();
                }
                group.mixer.push(&participant.participant_id, &pcm);
            }
        }
    }

    fn on_mic(&mut self, pcm: &[i16]) {
        if !self.group_epoch_ready() {
            return;
        }
        let Some(m) = self.media.as_mut() else {
            return;
        };
        if m.audio.io != AudioIo::Pcm {
            return;
        }
        // Drop a wrong-length frame before any send: the encoder needs exactly one 60ms frame, and a
        // mis-sized buffer must not reach the DTX fast-path (which would emit an off-cadence packet).
        if pcm.len() != MIC_FRAME_SAMPLES {
            return;
        }
        // The gate is mutual, so a peer that selected standard Opus decodes only standard Opus.
        // Sending MLow at it is the other half of the silence, and no amount of receive-side rescue
        // fixes it: the peer has to hear us too.
        if m.active_format.codec == AudioCodec::Opus {
            let Some(foreign) = m.foreign_audio.as_mut() else {
                // Nothing to encode with, so nothing goes out at all -- not even the comfort-noise
                // frame that keeps the peer's media-liveness timer fed. Counted rather than dropped
                // in silence: an outbound side that has gone mute is the other half of #1105, and
                // it is invisible from every other counter here, which describe reception.
                self.media_stats.outbound_frames_without_encoder = self
                    .media_stats
                    .outbound_frames_without_encoder
                    .saturating_add(1);
                return;
            };
            let mut encoded = core::mem::take(&mut m.foreign_encoded);
            encoded.clear();
            let sent = foreign.encode(pcm, &mut encoded);
            if let Err(e) = &sent {
                // Counted like the missing encoder above, and for the same reason: an encoder that
                // refuses a well-formed frame stops the peer hearing us, and every other counter
                // here watches the inbound direction.
                log::debug!("voip: foreign audio encode failed: {e}");
                self.media_stats.outbound_frames_without_encoder = self
                    .media_stats
                    .outbound_frames_without_encoder
                    .saturating_add(1);
            }
            let packet = sent.is_ok().then(|| m.pipe.protect_audio(&encoded));
            m.foreign_encoded = encoded;
            if let Some(packet) = packet {
                self.outbox.push_back(Output::Transmit(Bytes::from(packet)));
            }
            return;
        }
        // Past this point the frame has to become MLOW, which only the built-in codec can do. A
        // build without it counts the frame the same way the Opus branch counts a missing encoder:
        // an outbound side gone mute is invisible from every other counter here.
        #[cfg(not(feature = "voip-mlow"))]
        {
            self.media_stats.outbound_frames_without_encoder = self
                .media_stats
                .outbound_frames_without_encoder
                .saturating_add(1);
        }
        #[cfg(feature = "voip-mlow")]
        {
            self.encode_mlow_frame(pcm);
        }
    }

    /// The MLOW half of [`Self::on_mic`], split out so the standard-Opus half compiles without the
    /// built-in codec.
    #[cfg(feature = "voip-mlow")]
    fn encode_mlow_frame(&mut self, pcm: &[i16]) {
        let Some(m) = self.media.as_mut() else {
            return;
        };
        let Some(pcm_state) = m.pcm.as_mut() else {
            return;
        };
        // OS mic-mute delivers an exactly all-zero frame; genuine quiet speech carries LSB noise.
        // Don't gap the wire on mute: send a cheap cached DTX comfort-noise frame so the peer's
        // media-liveness timer stays fed (no codec CPU) and it doesn't re-negotiate the transport.
        if pcm.iter().all(|&s| s == 0) {
            let packet = m.pipe.protect_audio(&MLOW_DTX_CNG);
            self.outbox.push_back(Output::Transmit(Bytes::from(packet)));
            return;
        }
        pcm_state.scratch.clear();
        pcm_state
            .scratch
            .extend(pcm.iter().map(|&s| s as f32 / 32768.0));
        // A transient encode failure drops just this frame; the next one resyncs. Counted the same
        // way the foreign encoder's refusal is: a run of them stops outbound RTP, and every other
        // counter here watches the inbound direction, so without this the peer stops hearing us
        // while `media_stats()` reports a healthy call.
        if pcm_state
            .encoder
            .encode_into(&pcm_state.scratch, &mut pcm_state.encoded)
            .is_err()
        {
            self.media_stats.outbound_frames_without_encoder = self
                .media_stats
                .outbound_frames_without_encoder
                .saturating_add(1);
            return;
        }
        // No SFrame on send by design: the encoded frame goes plain into WAHKDF SRTP, which the peer
        // accepts. `enable_sframe` is recv-decrypt-only (see CallConfig). This matches the
        // pre-refactor send path; send-side SFrame is intentionally not wired.
        let packet = m.pipe.protect_audio(&pcm_state.encoded);
        self.outbox.push_back(Output::Transmit(Bytes::from(packet)));
    }

    fn on_encoded_audio(&mut self, payload: &[u8]) {
        if !self.group_epoch_ready() {
            return;
        }
        let Some(m) = self
            .media
            .as_mut()
            .filter(|media| media.audio.io == AudioIo::Encoded)
        else {
            return;
        };
        // The source is fixed to the FORMAT the call was built with, not just its codec: MLOW's
        // escape and native Opus are both codec `Opus`, and the source supplies the escape's
        // rewritten TOC (which is exactly what `accepts_encoded_payload` demands of it). Compared on
        // the codec alone, a call downgraded off the escape kept handing those rewritten TOCs to a
        // peer expecting RFC Opus, under a profile that accepts any nonempty payload.
        //
        // That one pair is also the one the engine can repair itself: the escape's payload IS an
        // RFC Opus packet with one byte rewritten, so restoring the TOC costs a byte and no
        // transcode. Any other divergence has no such translation and is dropped, counted where a
        // call that cannot encode is already counted and announced once by `switch_audio_codec`.
        let translated = if m.audio.format == m.active_format {
            None
        } else if let Some(payload) = translate_encoded_for_active_format(m, payload) {
            Some(payload)
        } else {
            self.media_stats.outbound_frames_without_encoder = self
                .media_stats
                .outbound_frames_without_encoder
                .saturating_add(1);
            return;
        };
        let payload = translated.as_deref().unwrap_or(payload);
        if !m.active_format.accepts_encoded_payload(payload) {
            if m.audio_tx_invalid_streak < MAX_INVALID_AUDIO_WARNINGS {
                log::warn!(
                    "voip dropping encoded audio incompatible with the negotiated RTP profile call_id={} codec={:?} profile={:?} payload_len={} toc={:?}",
                    self.call_id,
                    m.active_format.codec,
                    m.active_format.rtp_profile,
                    payload.len(),
                    payload.first().copied(),
                );
                m.audio_tx_invalid_streak += 1;
            }
            return;
        }
        m.audio_tx_invalid_streak = 0;
        let packet = m.pipe.protect_audio(payload);
        self.outbox.push_back(Output::Transmit(Bytes::from(packet)));
    }

    fn on_video(&mut self, au: &[u8]) {
        if !self.group_epoch_ready() {
            // Everything here is discarded, an IDR included -- so a request the
            // application has already answered has to be made again once the
            // epoch installs, or the requirement outlives every request for it.
            // Only for an IDR: a delta was never an answer, so discarding one
            // costs nothing that has to be asked for a second time.
            if let Some(video) = self.media.as_mut().and_then(|media| media.video.as_mut())
                && video.keyframe_required
                && au_has_idr(au)
            {
                video.keyframe_announced = false;
            }
            return;
        }
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
        v.keyframe_announced = false;
        for packet in packets {
            self.outbox.push_back(Output::Transmit(Bytes::from(packet)));
        }
    }
}

type GroupRelayAllocateMaterial = (Vec<u8>, [u8; 6], Vec<u8>);

struct GroupRelayRefresh {
    relay_addr: SocketAddr,
    relay_token: Vec<u8>,
    endpoint_xor: [u8; 6],
    integrity_key: Vec<u8>,
    warp_mi_tag_len: usize,
}

fn prepare_group_relay_refresh(
    update: &GroupCallUpdate,
) -> Result<Option<GroupRelayRefresh>, GroupMediaError> {
    let Some(relay) = update.relay.as_ref() else {
        return Ok(None);
    };
    let warp_mi_tag_len = relay.warp_mi_tag_len.unwrap_or(4) as usize;
    if !(1..=20).contains(&warp_mi_tag_len) {
        return Err(GroupMediaError::Pipeline);
    }
    let (relay_token, endpoint_xor, integrity_key) = group_relay_allocate_material(relay)?;
    Ok(Some(GroupRelayRefresh {
        relay_addr: group_relay_socket_addr(relay)?,
        relay_token,
        endpoint_xor,
        integrity_key,
        warp_mi_tag_len,
    }))
}

pub(crate) fn validate_group_relay_update(update: &GroupCallUpdate) -> Result<(), GroupMediaError> {
    prepare_group_relay_refresh(update).map(drop)
}

fn get_group_media_relay_endpoint(
    relay: &GroupCallRelay,
) -> Option<&crate::types::group_call::GroupCallRelayEndpoint> {
    let usable = |endpoint: &&crate::types::group_call::GroupCallRelayEndpoint| {
        !endpoint.is_fna
            && endpoint.ipv4.is_some()
            && endpoint.port.is_some_and(|port| port != 0)
            && relay
                .tokens
                .get(endpoint.token_id as usize)
                .is_some_and(|token| !token.is_empty())
    };
    relay
        .endpoints
        .iter()
        .filter(usable)
        .find(|endpoint| endpoint.port == Some(super::relay_parse::WEB_CLIENT_RELAY_PORT))
        .or_else(|| relay.endpoints.iter().find(usable))
}

fn group_relay_allocate_material(
    relay: &GroupCallRelay,
) -> Result<GroupRelayAllocateMaterial, GroupMediaError> {
    if relay.key.is_empty() {
        return Err(GroupMediaError::InvalidSnapshot);
    }
    if let Some(endpoint) = get_group_media_relay_endpoint(relay) {
        let (Some(ipv4), Some(port)) = (endpoint.ipv4.as_deref(), endpoint.port) else {
            return Err(GroupMediaError::InvalidSnapshot);
        };
        let Some(token) = relay
            .tokens
            .get(endpoint.token_id as usize)
            .filter(|token| !token.is_empty())
        else {
            return Err(GroupMediaError::InvalidSnapshot);
        };
        let Some(endpoint_xor) = stun::encode_xor_relay_endpoint(ipv4, port) else {
            return Err(GroupMediaError::InvalidSnapshot);
        };
        return Ok((token.clone(), endpoint_xor, relay.key.clone()));
    }
    Err(GroupMediaError::InvalidSnapshot)
}

fn group_relay_socket_addr(relay: &GroupCallRelay) -> Result<SocketAddr, GroupMediaError> {
    let endpoint = get_group_media_relay_endpoint(relay).ok_or(GroupMediaError::InvalidSnapshot)?;
    let ip = endpoint
        .ipv4
        .as_deref()
        .ok_or(GroupMediaError::InvalidSnapshot)?
        .parse::<Ipv4Addr>()
        .map_err(|_| GroupMediaError::InvalidSnapshot)?;
    let port = endpoint.port.ok_or(GroupMediaError::InvalidSnapshot)?;
    Ok(SocketAddr::new(IpAddr::V4(ip), port))
}

fn remote_group_pids(update: &GroupCallUpdate, local_device: &Jid) -> Vec<u32> {
    let mut pids = update
        .participants
        .iter()
        .filter(|participant| participant.is_connected())
        .flat_map(|participant| {
            participant
                .devices
                .iter()
                .filter(|device| !group_device_is_local(participant, device, local_device))
        })
        .filter_map(|device| device.pid)
        .collect::<Vec<_>>();
    pids.sort_unstable();
    pids.dedup();
    pids
}

fn group_roster_contains_participant(update: &GroupCallUpdate, local_device: &Jid) -> bool {
    group_roster_local_device(update, local_device).is_some()
}

fn group_roster_local_device<'a>(
    update: &'a GroupCallUpdate,
    local_device: &Jid,
) -> Option<&'a crate::types::group_call::GroupCallDevice> {
    update
        .participants
        .iter()
        .filter(|participant| participant.is_connected())
        .find_map(|participant| {
            participant
                .devices
                .iter()
                .find(|device| group_device_is_local(participant, device, local_device))
        })
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
    packet_samps: usize,
    cap: usize,
) -> Vec<i16> {
    let target = playout_bounds(packet_samps).0;
    if jitter.len() > cap {
        let drop_n = jitter.len() - cap;
        jitter.drain(..drop_n);
    }
    if *priming {
        let reached_target = jitter.len() >= target;
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
mod encoded_tests {
    use super::*;
    use crate::types::group_call::{
        GroupCallDevice, GroupCallParticipant, GroupCallRelay, GroupCallRelayEndpoint,
    };
    use wacore_binary::Server;

    const SELF_LID: &str = "15550001111:0@lid";
    const PEER_LID: &str = "15550002222:0@lid";

    fn config() -> CallConfig {
        CallConfig {
            call_id: "ENCODED-AUDIO-TEST".into(),
            direction: CallDirection::Incoming,
            self_lid: SELF_LID.into(),
            peer_lid: PEER_LID.into(),
            call_key: (0u8..32).collect(),
            ssrc: 0x5741_0001,
            audio: AudioConfig::encoded(AudioFormat::OPUS_16KHZ_60MS),
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

    fn drain(engine: &mut CallEngine) -> Vec<Output> {
        let mut outputs = Vec::new();
        loop {
            match engine.poll_output() {
                Output::Timeout(_) => return outputs,
                output => outputs.push(output),
            }
        }
    }

    fn allocation_success(engine: &CallEngine) -> Vec<u8> {
        let transaction_id = engine
            .allocate_transaction_id
            .expect("current allocation transaction");
        stun::encode_stun_request(
            stun::MSG_ALLOCATE_SUCCESS,
            &transaction_id,
            &[],
            None,
            false,
        )
    }

    fn allocation_error(transaction_id: &[u8; 12], code: u16) -> Vec<u8> {
        let class = (code / 100) as u8;
        let number = (code % 100) as u8;
        let error = [0x00, 0x09, 0x00, 0x04, 0x00, 0x00, class, number];
        stun::encode_stun_request(
            stun::MSG_ALLOCATE_ERROR,
            transaction_id,
            &error,
            None,
            false,
        )
    }

    fn group_update() -> GroupCallUpdate {
        let creator = Jid::new("15550001111", Server::Lid);
        let peer = Jid::new("15550002222", Server::Lid);
        GroupCallUpdate {
            call_id: "ENCODED-AUDIO-TEST".to_string(),
            call_creator: creator.clone(),
            group_jid: None,
            transaction_id: 7,
            media: "audio".to_string(),
            connected_limit: 32,
            joinable: true,
            av_upgradable: true,
            rekey_requested: false,
            participants: vec![
                GroupCallParticipant {
                    jid: creator.clone(),
                    pn: None,
                    state: Some("connected".to_string()),
                    participant_type: None,
                    devices: vec![GroupCallDevice {
                        jid: creator,
                        platform: None,
                        pid: Some(1),
                        capability_version: None,
                        capability: Vec::new(),
                    }],
                },
                GroupCallParticipant {
                    jid: peer.clone(),
                    pn: None,
                    state: Some("connected".to_string()),
                    participant_type: None,
                    devices: vec![GroupCallDevice {
                        jid: peer,
                        platform: None,
                        pid: Some(2),
                        capability_version: None,
                        capability: Vec::new(),
                    }],
                },
            ],
            relay: None,
        }
    }

    fn group_relay() -> GroupCallRelay {
        GroupCallRelay::builder()
            .transaction_id(7)
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
                ipv4: Some("203.0.113.7".to_string()),
                port: Some(3480),
            }])
            .build()
    }

    fn group_engine() -> CallEngine {
        let relay = group_relay();
        let mut update = group_update();
        update.media = "video".to_string();
        update.relay = Some(relay.clone());
        let mut config = CallConfig::for_group(
            CallDirection::Outgoing,
            &update.call_id,
            SELF_LID,
            SELF_LID,
            &relay,
        )
        .expect("group config");
        config.audio = AudioConfig::encoded(AudioFormat::OPUS_16KHZ_60MS);
        config.enable_video = true;
        let mut engine =
            CallEngine::new(config, Box::new(SequentialTxIds::new())).expect("group engine");
        engine
            .configure_group(GroupEngineConfig {
                call_creator: update.call_creator.clone(),
                self_jid: Jid::new("15550001111", Server::Lid),
                initial_update: update,
                direct_peer: None,
            })
            .expect("configure group");
        engine
    }

    /// Hands each group participant its own [`DecodeToConstant`].
    #[cfg(not(feature = "voip-mlow"))]
    struct DecodeToConstantFactory(i16);

    #[cfg(not(feature = "voip-mlow"))]
    impl ForeignAudioCodecFactory for DecodeToConstantFactory {
        fn create(&self) -> Option<Box<dyn ForeignAudioCodec>> {
            Some(Box::new(DecodeToConstant(self.0)))
        }
    }

    /// A PCM group call with an installed epoch and one remote participant, for the build without
    /// the built-in codec. The MLOW test module has richer group helpers, but they are gated on the
    /// very feature these tests exist to exercise the absence of.
    #[cfg(not(feature = "voip-mlow"))]
    fn pcm_group_engine() -> (CallEngine, [u8; 32], MediaPipeline) {
        let relay = group_relay();
        let mut update = group_update();
        update.relay = Some(relay.clone());
        let mut cfg = CallConfig::for_group(
            CallDirection::Outgoing,
            &update.call_id,
            SELF_LID,
            SELF_LID,
            &relay,
        )
        .expect("group config");
        cfg.audio = AudioConfig::OPUS_PCM;
        let mut engine = CallEngine::new(cfg, Box::new(SequentialTxIds::new()))
            .expect("standard Opus PCM does not need the MLOW codec")
            .with_foreign_audio_codec_factory(Box::new(DecodeToConstantFactory(4321)));
        engine
            .configure_group(GroupEngineConfig {
                call_creator: update.call_creator.clone(),
                self_jid: Jid::new("15550001111", Server::Lid),
                initial_update: update,
                direct_peer: None,
            })
            .expect("configure group");
        let epoch = [0x42u8; 32];
        engine
            .apply_group_raw_epoch(7, &epoch)
            .expect("install epoch");
        let peer_id = ssrc::format_e2e_srtp_participant_id(PEER_LID);
        let peer = MediaPipeline::new(&MediaPipelineParams {
            call_key: &epoch,
            self_lid: PEER_LID,
            peer_lid: SELF_LID,
            ssrc: ssrc::derive_wasm_participant_ssrc("ENCODED-AUDIO-TEST", &peer_id, 0),
            samples_per_packet: AudioFormat::OPUS_16KHZ_60MS.rtp_timestamp_step,
            warp_mi_tag_len: 4,
        })
        .expect("peer audio pipeline");
        (engine, epoch, peer)
    }

    // The mixer holds PCM, so what filled it is irrelevant to draining it -- but the drain was gated
    // on the built-in codec, so a `voip-libopus` group call decoded every participant into a mixer
    // nothing emptied and then played the empty direct-call buffer instead. Silence, from the one
    // build whose whole purpose is that codec.
    #[cfg(not(feature = "voip-mlow"))]
    #[test]
    fn a_group_call_mixes_and_plays_without_the_built_in_codec() {
        let (mut eng, _epoch, mut peer) = pcm_group_engine();
        eng.start(0, 1_700_000_000_000);
        let _ = drain(&mut eng);
        let allocate = allocation_success(&eng);
        eng.handle_input(1, Input::RelayPacket(&allocate));
        let _ = drain(&mut eng);

        let mut heard = Vec::new();
        for n in 0..8u64 {
            let packet = peer.protect_audio(&[0xE8u8, 0x11, 0x22, 0x33]);
            eng.handle_input(100 + n * 60, Input::RelayPacket(&packet));
            for tick in 0..3u64 {
                eng.handle_input(100 + n * 60 + tick * 20, Input::Timeout);
                for output in drain(&mut eng) {
                    if let Output::Playout(frame) = output {
                        heard.extend(frame);
                    }
                }
            }
        }

        assert!(
            eng.media_stats().foreign_frames_decoded > 0,
            "each participant is decoded by the injected factory"
        );
        assert!(
            heard.contains(&4321),
            "and the mixer has to be drained into playout, whatever filled it"
        );
    }

    #[test]
    fn initial_group_roster_requires_the_local_device() {
        let mut engine =
            CallEngine::new(config(), Box::new(SequentialTxIds::new())).expect("direct engine");
        let mut update = group_update();
        update.participants.remove(0);

        assert!(matches!(
            engine.configure_group(GroupEngineConfig {
                call_creator: update.call_creator.clone(),
                self_jid: Jid::new("15550001111", Server::Lid),
                initial_update: update,
                direct_peer: None,
            }),
            Err(EngineError::GroupMedia(
                GroupMediaError::LocalParticipantRemoved
            ))
        ));
        assert!(
            !engine.is_group(),
            "a roster that never admitted this device cannot publish group media"
        );
    }

    #[test]
    fn initial_group_roster_accepts_the_local_device_through_its_pn_alias() {
        let relay = group_relay();
        let mut update = group_update();
        let local_pn = Jid::new("12025550111", Server::Pn);
        update.participants[0].pn = Some(local_pn.clone());
        update.participants[0].devices[0].jid = local_pn.clone();
        update.relay = Some(relay.clone());
        let config = CallConfig::for_group(
            CallDirection::Outgoing,
            &update.call_id,
            SELF_LID,
            SELF_LID,
            &relay,
        )
        .expect("group config");
        let mut config = config;
        config.audio = AudioConfig::encoded(AudioFormat::OPUS_16KHZ_60MS);
        let mut engine = CallEngine::new(config, Box::new(SequentialTxIds::new())).expect("engine");

        engine
            .configure_group(GroupEngineConfig {
                call_creator: update.call_creator.clone(),
                self_jid: Jid::new("15550001111", Server::Lid),
                initial_update: update.clone(),
                direct_peer: None,
            })
            .expect("the local PN device belongs to the local LID participant");
        let group = engine.group.as_ref().expect("group state");
        assert_eq!(
            group.registry.active_pids(),
            vec![2],
            "the local PN device must not create a media receiver"
        );
        assert_eq!(
            remote_group_pids(&update, &Jid::new("15550001111", Server::Lid),),
            vec![2],
            "the local PN device must not be subscribed through the relay"
        );
        let local_participant_id = ssrc::format_e2e_srtp_participant_id(&local_pn.to_string());
        assert_eq!(engine.self_participant_id, local_participant_id);
        let local_ssrc =
            ssrc::derive_wasm_participant_ssrc(&update.call_id, &local_participant_id, 0);
        let group = engine.group.as_ref().expect("group state");
        assert_eq!(
            group.stream_ssrcs[3],
            ssrc::derive_video_participant_ssrc(&update.call_id, &local_participant_id)
        );
        assert_eq!(
            group.app_data_ssrc,
            ssrc::derive_wasm_participant_ssrc(
                &update.call_id,
                &local_participant_id,
                ssrc::APP_DATA_SSRC_SLOT_WORD,
            )
        );
        assert_eq!(
            engine.media.as_ref().expect("group media").self_lid,
            local_pn.to_string()
        );
        assert_eq!(
            engine.media.as_ref().expect("group media").pipe.send_ssrc(),
            local_ssrc,
            "outbound media must use the admitted roster device identity"
        );

        let epoch = [0x42; 32];
        assert_eq!(
            engine
                .apply_group_raw_epoch(update.transaction_id, &epoch)
                .expect("install group epoch"),
            GroupEpochApply::Installed
        );
        engine.handle_input(1, Input::EncodedAudio(&[0x08, 1, 2]));
        let packet = drain(&mut engine)
            .into_iter()
            .find_map(|output| match output {
                Output::Transmit(packet) => Some(packet),
                _ => None,
            })
            .expect("authenticated outbound audio");
        let peer = update.participants[1].devices[0].jid.clone();
        let mut receiver = MediaPipeline::new(&MediaPipelineParams {
            call_key: &epoch,
            self_lid: &peer.to_string(),
            peer_lid: &local_pn.to_string(),
            ssrc: 1,
            samples_per_packet: AudioFormat::OPUS_16KHZ_60MS.rtp_timestamp_step,
            warp_mi_tag_len: 4,
        })
        .expect("peer receiver");
        let (header, payload) = receiver
            .unprotect_audio(&packet)
            .expect("the peer derives the local PN sender key");
        assert_eq!(header.ssrc, local_ssrc);
        assert_eq!(payload, [0x08, 1, 2]);
    }

    #[test]
    fn configure_group_rejects_a_started_engine() {
        let mut engine =
            CallEngine::new(config(), Box::new(SequentialTxIds::new())).expect("direct engine");
        engine.start(1, 1_700_000_000_000);
        let mut update = group_update();
        update.relay = Some(group_relay());
        assert!(matches!(
            engine.configure_group(GroupEngineConfig {
                call_creator: update.call_creator.clone(),
                self_jid: Jid::new("15550001111", Server::Lid),
                initial_update: update,
                direct_peer: None,
            }),
            Err(EngineError::GroupMedia(GroupMediaError::Pipeline))
        ));
    }

    /// The registry restates a retained rotation on the video-control channel
    /// and the roster travels on the group-control channel, so the driver can
    /// install the rotation before applying the roster. That is safe because
    /// the restated key is the roster's own name for the announcer: the prune
    /// below reads the same snapshot the key came from. Only a participant the
    /// snapshot no longer carries loses its rotation.
    #[test]
    fn a_rotation_installed_before_its_roster_survives_the_prune() {
        let mut engine = group_engine();
        let peer = group_update().participants[1].jid.clone();
        let departed = Jid::new("15550003333", Server::Lid);
        engine.set_participant_video_orientation(peer.clone(), 3);
        engine.set_participant_video_orientation(departed.clone(), 2);

        let mut update = group_update();
        update.media = "video".to_string();
        update.relay = Some(group_relay());
        update.transaction_id = 8;
        assert_eq!(
            engine.apply_group_update(1, &update).unwrap(),
            GroupRosterApply::Applied
        );

        let orientations = &engine
            .group
            .as_ref()
            .expect("group state")
            .video_orientations;
        assert_eq!(
            orientations.get(&peer),
            Some(&3),
            "a rotation keyed by the roster survives the roster landing after it"
        );
        assert!(
            !orientations.contains_key(&departed),
            "a rotation the snapshot cannot name is still retired"
        );
    }

    #[test]
    fn participant_pid_change_discards_queued_audio_from_the_old_session() {
        let mut engine = group_engine();
        let mut update = group_update();
        engine
            .apply_group_raw_epoch(7, &[0x42; 32])
            .expect("install group epoch");
        let participant_id = ssrc::format_e2e_srtp_participant_id(
            &update.participants[1].devices[0].jid.to_string(),
        );
        let group = engine.group.as_mut().expect("group state");
        assert!(group.mixer.push(
            &participant_id,
            &vec![7; crate::voip::group_audio::GROUP_MIX_PREFILL_SAMPLES]
        ));

        update.transaction_id = 8;
        update.participants[1].devices[0].pid = Some(9);
        assert_eq!(
            engine.apply_group_update(1, &update).unwrap(),
            GroupRosterApply::Applied
        );
        assert!(
            engine
                .group
                .as_mut()
                .expect("group state")
                .mixer
                .mix_chunk()
                .is_none(),
            "queued PCM from the retired PID must not play in the replacement session"
        );
    }

    #[test]
    fn group_media_prefers_the_web_relay_port() {
        let endpoint = |relay_id, token_id, port| GroupCallRelayEndpoint {
            relay_id,
            token_id,
            auth_token_id: token_id,
            relay_name: format!("relay-{relay_id}"),
            domain_name: None,
            rtt_ms: None,
            is_fna: false,
            address: Vec::new(),
            ipv4: Some("203.0.113.7".to_string()),
            port: Some(port),
        };
        let relay = GroupCallRelay::builder()
            .transaction_id(1)
            .self_pid(1)
            .uuid("relay".to_string())
            .participant_uuid("participant".to_string())
            .attribute_padding(false)
            .warp_mi_tag_len(4)
            .key(b"relay-key".to_vec())
            .tokens(vec![vec![0x47], vec![0x48]])
            .auth_tokens(vec![vec![0x57], vec![0x58]])
            .endpoints(vec![endpoint(1, 0, 3478), endpoint(2, 1, 3480)])
            .build();

        let selected = get_group_media_relay_endpoint(&relay).expect("usable relay");
        assert_eq!(selected.port, Some(3480));
        let config = CallConfig::for_group(
            CallDirection::Outgoing,
            "GROUP-CALL",
            SELF_LID,
            SELF_LID,
            &relay,
        )
        .expect("group config");
        assert_eq!(config.relay_port, 3480);
        assert_eq!(config.relay_token, vec![0x48]);
        assert_eq!(
            group_relay_allocate_material(&relay)
                .expect("allocate material")
                .0,
            vec![0x48]
        );
    }

    #[test]
    fn group_media_does_not_require_latency_probe_auth_tokens() {
        let mut relay = group_relay();
        relay.auth_tokens.clear();
        relay.endpoints[0].auth_token_id = 0;

        assert_eq!(
            get_group_media_relay_endpoint(&relay).and_then(|endpoint| endpoint.port),
            Some(3480)
        );
        assert!(
            CallConfig::for_group(
                CallDirection::Outgoing,
                "GROUP-CALL",
                SELF_LID,
                SELF_LID,
                &relay,
            )
            .is_ok()
        );
        assert!(group_relay_allocate_material(&relay).is_ok());
    }

    #[test]
    fn direct_engine_promotes_when_group_roster_arrives() {
        let mut engine =
            CallEngine::new(config(), Box::new(SequentialTxIds::new())).expect("direct engine");
        assert!(!engine.is_group());

        assert_eq!(
            engine
                .apply_group_update(0, &group_update())
                .expect("promote to group"),
            GroupRosterApply::Applied
        );
        assert!(engine.is_group());
        assert_eq!(
            engine
                .apply_group_raw_epoch(7, &[0x42; 32])
                .expect("install group epoch"),
            GroupEpochApply::Installed
        );
    }

    #[test]
    fn live_direct_engine_rejects_group_promotion_that_changes_sender_ssrc() {
        let mut engine =
            CallEngine::new(config(), Box::new(SequentialTxIds::new())).expect("direct engine");
        let original_participant_id = engine.self_participant_id.clone();
        let original_ssrc = engine
            .media
            .as_ref()
            .expect("direct media")
            .pipe
            .send_ssrc();
        engine.start(1, 1_700_000_000_000);
        let _ = drain(&mut engine);

        assert!(matches!(
            engine.apply_group_update(2, &group_update()),
            Err(EngineError::GroupMedia(GroupMediaError::Pipeline))
        ));
        assert!(!engine.is_group());
        assert_eq!(engine.self_participant_id, original_participant_id);
        assert_eq!(
            engine
                .media
                .as_ref()
                .expect("direct media")
                .pipe
                .send_ssrc(),
            original_ssrc
        );
    }

    #[test]
    fn live_direct_engine_promotes_when_sender_identity_and_ssrc_are_unchanged() {
        let mut config = config();
        let participant_id = ssrc::format_e2e_srtp_participant_id(&config.self_lid);
        config.ssrc = ssrc::derive_wasm_participant_ssrc(&config.call_id, &participant_id, 0);
        let original_ssrc = config.ssrc;
        let mut engine =
            CallEngine::new(config, Box::new(SequentialTxIds::new())).expect("direct engine");
        engine.start(1, 1_700_000_000_000);
        let _ = drain(&mut engine);

        let mut update = group_update();
        let local_pn = Jid::new("12025550111", Server::Pn);
        update.participants[0].pn = Some(local_pn.clone());
        update.participants[0].devices[0].jid = local_pn;
        assert_eq!(
            engine
                .apply_group_update(2, &update)
                .expect("PN-alias roster promotion preserving the live sender"),
            GroupRosterApply::Applied
        );
        assert!(engine.is_group());
        assert_eq!(engine.self_participant_id, participant_id);
        assert_eq!(
            engine.media.as_ref().expect("group media").pipe.send_ssrc(),
            original_ssrc
        );
    }

    #[test]
    fn matching_roster_installs_a_buffered_epoch_into_the_send_pipeline() {
        let mut engine = group_engine();
        assert_eq!(engine.group_epoch_transaction(), None);
        assert_eq!(
            engine
                .apply_group_raw_epoch(8, &[0x48; 32])
                .expect("buffer future epoch"),
            GroupEpochApply::Buffered
        );
        let mut update = group_update();
        update.transaction_id = 8;
        update.media = "video".to_string();
        assert_eq!(
            engine
                .apply_group_update(1, &update)
                .expect("apply matching roster"),
            GroupRosterApply::Applied
        );
        assert_eq!(
            engine.group_epoch_transaction(),
            Some(8),
            "the driver must be able to observe and purge on the send-key transition"
        );
    }

    #[test]
    fn group_roster_additions_and_epoch_changes_require_an_idr() {
        let mut engine = group_engine();
        assert_eq!(
            engine.apply_group_raw_epoch(7, &[0x42; 32]).unwrap(),
            GroupEpochApply::Installed
        );
        engine.start(0, 1_700_000_000_000);
        let _ = drain(&mut engine);

        let idr = [0, 0, 0, 1, 0x65, 1, 2, 3];
        let delta = [0, 0, 0, 1, 0x41, 4, 5, 6];
        engine.handle_input(1, Input::VideoFrame(&idr));
        assert!(drain(&mut engine).iter().any(|output| matches!(
            output,
            Output::Transmit(packet)
                if parse_rtp_header(packet)
                    .is_some_and(|header| header.payload_type == RTP_PAYLOAD_TYPE_H264)
        )));
        engine.handle_input(2, Input::VideoFrame(&delta));
        let _ = drain(&mut engine);

        let mut expanded = group_update();
        expanded.transaction_id = 8;
        expanded.media = "video".to_string();
        expanded.relay = Some(group_relay());
        let participant = Jid::new("15550003333", Server::Lid);
        expanded.participants.push(GroupCallParticipant {
            jid: participant.clone(),
            pn: None,
            state: Some("connected".to_string()),
            participant_type: None,
            devices: vec![GroupCallDevice {
                jid: participant,
                platform: None,
                pid: Some(3),
                capability_version: None,
                capability: Vec::new(),
            }],
        });
        assert_eq!(
            engine.apply_group_update(3, &expanded).unwrap(),
            GroupRosterApply::Applied
        );
        let _ = drain(&mut engine);

        engine.handle_input(4, Input::VideoFrame(&delta));
        assert!(
            drain(&mut engine).iter().all(|output| !matches!(
                output,
                Output::Transmit(packet)
                    if parse_rtp_header(packet)
                        .is_some_and(|header| header.payload_type == RTP_PAYLOAD_TYPE_H264)
            )),
            "a newly admitted participant must not receive a dependent frame first"
        );
        engine.handle_input(5, Input::VideoFrame(&idr));
        let _ = drain(&mut engine);

        assert_eq!(
            engine.apply_group_raw_epoch(8, &[0x48; 32]).unwrap(),
            GroupEpochApply::Installed
        );
        engine.handle_input(6, Input::VideoFrame(&delta));
        assert!(
            drain(&mut engine).iter().all(|output| !matches!(
                output,
                Output::Transmit(packet)
                    if parse_rtp_header(packet)
                        .is_some_and(|header| header.payload_type == RTP_PAYLOAD_TYPE_H264)
            )),
            "the first video frame under a new group epoch must be an IDR"
        );
    }

    /// The gate discards deltas too, and a delta was never an answer to the
    /// standing request. Treating one as if it were would ask a second time for
    /// a requirement the application is already working on.
    #[test]
    fn a_gated_delta_does_not_re_ask_for_a_keyframe_already_requested() {
        let mut engine = group_engine();
        assert_eq!(
            engine.apply_group_raw_epoch(7, &[0x42; 32]).unwrap(),
            GroupEpochApply::Installed
        );
        engine.start(0, 1_700_000_000_000);
        let idr = [0, 0, 0, 1, 0x65, 1, 2, 3];
        let delta = [0, 0, 0, 1, 0x41, 4, 5, 6];
        engine.handle_input(1, Input::VideoFrame(&idr));
        let _ = drain(&mut engine);

        let mut expanded = group_update();
        expanded.transaction_id = 8;
        expanded.media = "video".to_string();
        expanded.relay = Some(group_relay());
        expanded.rekey_requested = true;
        let participant = Jid::new("15550003333", Server::Lid);
        expanded.participants.push(GroupCallParticipant {
            jid: participant.clone(),
            pn: None,
            state: Some("connected".to_string()),
            participant_type: None,
            devices: vec![GroupCallDevice {
                jid: participant,
                platform: None,
                pid: Some(3),
                capability_version: None,
                capability: Vec::new(),
            }],
        });
        assert_eq!(
            engine.apply_group_update(3, &expanded).unwrap(),
            GroupRosterApply::Applied
        );
        assert_eq!(
            keyframe_requests(&drain(&mut engine)),
            1,
            "the rekey asks once for the IDR it needs"
        );

        // Only deltas arrive while the epoch is pending, and a delta was never
        // an answer to that request.
        engine.handle_input(4, Input::VideoFrame(&delta));
        assert_eq!(keyframe_requests(&drain(&mut engine)), 0);
        assert_eq!(
            engine.apply_group_raw_epoch(8, &[0x48; 32]).unwrap(),
            GroupEpochApply::Installed
        );
        assert_eq!(
            keyframe_requests(&drain(&mut engine)),
            0,
            "the request already standing is not made a second time"
        );

        // Still owed, though: the plane keeps dropping until it arrives.
        engine.handle_input(5, Input::VideoFrame(&delta));
        assert!(drain(&mut engine).iter().all(|output| !matches!(
            output,
            Output::Transmit(packet)
                if parse_rtp_header(packet)
                    .is_some_and(|header| header.payload_type == RTP_PAYLOAD_TYPE_H264)
        )));
        engine.handle_input(6, Input::VideoFrame(&idr));
        assert!(drain(&mut engine).iter().any(|output| matches!(
            output,
            Output::Transmit(packet)
                if parse_rtp_header(packet)
                    .is_some_and(|header| header.payload_type == RTP_PAYLOAD_TYPE_H264)
        )));
    }

    fn keyframe_requests(outputs: &[Output]) -> usize {
        outputs
            .iter()
            .filter(|output| matches!(output, Output::Event(CallEvent::VideoKeyframeNeeded)))
            .count()
    }

    /// The keyframe an application produces for a rekey it cannot yet encrypt
    /// under is dropped by the epoch gate, and dropping it does not clear the
    /// requirement. Installing the epoch must therefore ask again even though
    /// the flag was already set, or the requirement outlives every request for
    /// it and outbound video stays dark until the encoder's own IDR interval.
    #[test]
    fn installing_a_gated_epoch_asks_again_for_the_keyframe_it_swallowed() {
        let mut engine = group_engine();
        assert_eq!(
            engine.apply_group_raw_epoch(7, &[0x42; 32]).unwrap(),
            GroupEpochApply::Installed
        );
        engine.start(0, 1_700_000_000_000);
        let idr = [0, 0, 0, 1, 0x65, 1, 2, 3];
        engine.handle_input(1, Input::VideoFrame(&idr));
        let _ = drain(&mut engine);

        let mut expanded = group_update();
        expanded.transaction_id = 8;
        expanded.media = "video".to_string();
        expanded.relay = Some(group_relay());
        expanded.rekey_requested = true;
        let participant = Jid::new("15550003333", Server::Lid);
        expanded.participants.push(GroupCallParticipant {
            jid: participant.clone(),
            pn: None,
            state: Some("connected".to_string()),
            participant_type: None,
            devices: vec![GroupCallDevice {
                jid: participant,
                platform: None,
                pid: Some(3),
                capability_version: None,
                capability: Vec::new(),
            }],
        });
        assert_eq!(
            engine.apply_group_update(3, &expanded).unwrap(),
            GroupRosterApply::Applied
        );
        let _ = drain(&mut engine);

        // The application answers the rekey with an IDR the gate discards.
        engine.handle_input(4, Input::VideoFrame(&idr));
        let gated = drain(&mut engine);
        assert!(
            gated.iter().all(|output| !matches!(
                output,
                Output::Transmit(packet)
                    if parse_rtp_header(packet)
                        .is_some_and(|header| header.payload_type == RTP_PAYLOAD_TYPE_H264)
            )),
            "media under a requested-but-uninstalled epoch stays off the wire"
        );

        assert_eq!(
            engine.apply_group_raw_epoch(8, &[0x48; 32]).unwrap(),
            GroupEpochApply::Installed
        );
        assert!(
            drain(&mut engine)
                .iter()
                .any(|output| matches!(output, Output::Event(CallEvent::VideoKeyframeNeeded))),
            "the request the gate swallowed has to be raised again on install"
        );

        engine.handle_input(5, Input::VideoFrame(&idr));
        assert!(
            drain(&mut engine).iter().any(|output| matches!(
                output,
                Output::Transmit(packet)
                    if parse_rtp_header(packet)
                        .is_some_and(|header| header.payload_type == RTP_PAYLOAD_TYPE_H264)
            )),
            "and the IDR that answers it resumes video"
        );
    }

    #[test]
    fn group_device_replacement_reuses_pid_but_still_requires_an_idr() {
        let mut engine = group_engine();
        assert_eq!(
            engine.apply_group_raw_epoch(7, &[0x42; 32]).unwrap(),
            GroupEpochApply::Installed
        );
        engine.start(0, 1_700_000_000_000);
        let _ = drain(&mut engine);

        let idr = [0, 0, 0, 1, 0x65, 1, 2, 3];
        let delta = [0, 0, 0, 1, 0x41, 4, 5, 6];
        engine.handle_input(1, Input::VideoFrame(&idr));
        let _ = drain(&mut engine);

        let mut replaced = group_update();
        replaced.transaction_id = 8;
        replaced.media = "video".to_string();
        replaced.relay = Some(group_relay());
        let replacement = Jid::new("15550004444", Server::Lid);
        replaced.participants[1].jid = replacement.clone();
        replaced.participants[1].devices[0].jid = replacement;
        assert_eq!(
            engine.apply_group_update(2, &replaced).unwrap(),
            GroupRosterApply::Applied
        );
        let _ = drain(&mut engine);

        engine.handle_input(3, Input::VideoFrame(&delta));
        assert!(
            drain(&mut engine).iter().all(|output| !matches!(
                output,
                Output::Transmit(packet)
                    if parse_rtp_header(packet)
                        .is_some_and(|header| header.payload_type == RTP_PAYLOAD_TYPE_H264)
            )),
            "a different device on the same relay PID must receive an IDR first"
        );
    }

    #[test]
    fn authoritative_roster_removal_terminates_local_group_media() {
        let mut engine = group_engine();
        assert_eq!(
            engine.apply_group_raw_epoch(7, &[0x42; 32]).unwrap(),
            GroupEpochApply::Installed
        );
        engine.start(0, 1_700_000_000_000);
        let _ = drain(&mut engine);

        let mut removed = group_update();
        removed.transaction_id = 8;
        removed.participants.remove(0);
        assert!(matches!(
            engine.apply_group_update(1, &removed),
            Err(EngineError::GroupMedia(
                GroupMediaError::LocalParticipantRemoved
            ))
        ));
        assert!(
            engine.is_terminated(),
            "an authoritative roster removal must make the local media engine inert"
        );

        engine.handle_input(2, Input::EncodedAudio(&[1, 2, 3]));
        assert!(
            drain(&mut engine)
                .iter()
                .all(|output| !matches!(output, Output::Transmit(_))),
            "a removed participant must not emit any later media"
        );
    }

    #[test]
    fn invalid_relay_update_does_not_advance_the_roster_transaction() {
        let mut engine = group_engine();
        let mut invalid = group_update();
        invalid.transaction_id = 8;
        invalid.media = "video".to_string();
        let mut relay = group_relay();
        relay.tokens.clear();
        invalid.relay = Some(relay);

        assert!(matches!(
            engine.apply_group_update(1, &invalid),
            Err(EngineError::GroupMedia(GroupMediaError::InvalidSnapshot))
        ));
        assert_eq!(
            engine
                .group
                .as_ref()
                .map(|group| group.registry.roster_transaction()),
            Some(Some(7)),
            "a rejected relay must leave the committed roster untouched"
        );

        invalid.relay = Some(group_relay());
        assert_eq!(
            engine.apply_group_update(2, &invalid).unwrap(),
            GroupRosterApply::Applied,
            "a corrected resend with the same transaction must still apply"
        );
        assert_eq!(
            engine
                .group
                .as_ref()
                .map(|group| group.registry.roster_transaction()),
            Some(Some(8))
        );
    }

    #[test]
    fn invalid_initial_relay_does_not_commit_direct_promotion() {
        let mut engine =
            CallEngine::new(config(), Box::new(SequentialTxIds::new())).expect("direct engine");
        let mut update = group_update();
        let mut relay = group_relay();
        relay.tokens.clear();
        update.relay = Some(relay);

        assert!(matches!(
            engine.apply_group_update(1, &update),
            Err(EngineError::GroupMedia(GroupMediaError::InvalidSnapshot))
        ));
        assert!(
            !engine.is_group(),
            "a rejected initial relay must not partially promote the direct call"
        );

        update.relay = Some(group_relay());
        assert_eq!(
            engine.apply_group_update(2, &update).unwrap(),
            GroupRosterApply::Applied,
            "a corrected resend with the same transaction must still promote"
        );
        assert!(engine.is_group());
    }

    #[test]
    fn audio_only_group_update_disables_and_purges_outbound_video() {
        let mut engine = group_engine();
        assert_eq!(
            engine.apply_group_raw_epoch(7, &[0x42; 32]).unwrap(),
            GroupEpochApply::Installed
        );
        engine.start(0, 1_700_000_000_000);
        let _ = drain(&mut engine);

        engine.handle_input(1, Input::VideoFrame(&[0, 0, 0, 1, 0x65, 1, 2, 3]));
        assert!(
            engine.outbox.iter().any(|output| {
                matches!(
                    output,
                    Output::Transmit(packet)
                        if parse_rtp_header(packet).is_some_and(
                            |header| header.payload_type == RTP_PAYLOAD_TYPE_H264
                        )
                )
            }),
            "the active video plane must have queued an encrypted packet"
        );

        let mut audio_only = group_update();
        audio_only.transaction_id = 8;
        assert_eq!(
            engine.apply_group_update(2, &audio_only).unwrap(),
            GroupRosterApply::Applied
        );
        assert!(!engine.is_video_enabled());
        assert!(
            drain(&mut engine).iter().all(|output| {
                !matches!(
                    output,
                    Output::Transmit(packet)
                        if parse_rtp_header(packet).is_some_and(
                            |header| header.payload_type == RTP_PAYLOAD_TYPE_H264
                        )
                )
            }),
            "video protected under the old roster mode must be purged"
        );

        engine.handle_input(3, Input::VideoFrame(&[0, 0, 0, 1, 0x65, 4, 5, 6]));
        assert!(
            drain(&mut engine).iter().all(|output| {
                !matches!(
                    output,
                    Output::Transmit(packet)
                        if parse_rtp_header(packet).is_some_and(
                            |header| header.payload_type == RTP_PAYLOAD_TYPE_H264
                        )
                )
            }),
            "future video must stay gated after the authoritative audio downgrade"
        );
    }

    #[test]
    fn group_media_is_gated_until_an_authenticated_epoch_installs() {
        let mut engine = group_engine();
        engine.start(0, 1_700_000_000_000);
        let _ = drain(&mut engine);

        engine.handle_input(1, Input::EncodedAudio(&[0x11; 20]));
        engine.handle_input(2, Input::VideoFrame(&[0, 0, 0, 1, 0x65, 1, 2, 3]));
        assert!(
            drain(&mut engine)
                .iter()
                .all(|output| !matches!(output, Output::Transmit(_))),
            "the zero bootstrap key must never reach the relay"
        );

        assert_eq!(
            engine
                .apply_group_raw_epoch(7, &[0x42; 32])
                .expect("authenticated group epoch"),
            GroupEpochApply::Installed
        );
        engine.handle_input(3, Input::VideoFrame(&[0, 0, 0, 1, 0x65, 4, 5, 6]));
        assert!(
            drain(&mut engine)
                .iter()
                .any(|output| matches!(output, Output::Transmit(_))),
            "media should resume once an authenticated epoch is installed"
        );
    }

    #[test]
    fn requested_group_rekey_gates_media_until_the_matching_epoch_installs() {
        let mut engine = group_engine();
        assert_eq!(
            engine.apply_group_raw_epoch(7, &[0x42; 32]).unwrap(),
            GroupEpochApply::Installed
        );
        engine.start(0, 1_700_000_000_000);
        let _ = drain(&mut engine);

        engine.handle_input(1, Input::EncodedAudio(&[0x11; 20]));
        assert!(
            drain(&mut engine)
                .iter()
                .any(|output| matches!(output, Output::Transmit(_))),
            "the installed epoch must initially admit media"
        );

        let mut rekey = group_update();
        rekey.transaction_id = 8;
        rekey.rekey_requested = true;
        assert_eq!(
            engine.apply_group_update(2, &rekey).unwrap(),
            GroupRosterApply::Applied
        );
        let _ = drain(&mut engine);
        engine.handle_input(3, Input::EncodedAudio(&[0x22; 20]));
        assert!(
            drain(&mut engine)
                .iter()
                .all(|output| !matches!(output, Output::Transmit(_))),
            "media protected by the old key must stop once a newer epoch is requested"
        );

        assert_eq!(
            engine.apply_group_raw_epoch(8, &[0x48; 32]).unwrap(),
            GroupEpochApply::Installed
        );
        engine.handle_input(4, Input::EncodedAudio(&[0x33; 20]));
        assert!(
            drain(&mut engine)
                .iter()
                .any(|output| matches!(output, Output::Transmit(_))),
            "media must resume only after the requested epoch is installed"
        );
    }

    #[test]
    fn requested_group_rekey_gates_inbound_media_from_the_retired_epoch() {
        let old_epoch = [0x42; 32];
        let new_epoch = [0x48; 32];
        let update = group_update();
        let call_id = update.call_id.clone();
        let self_jid = update.participants[0].devices[0].jid.clone();
        let peer_jid = update.participants[1].devices[0].jid.clone();
        let peer_id = ssrc::format_e2e_srtp_participant_id(&peer_jid.to_string());
        let sender = |epoch: &[u8]| {
            let mut pipe = MediaPipeline::new(&MediaPipelineParams {
                call_key: epoch,
                self_lid: &peer_jid.to_string(),
                peer_lid: &self_jid.to_string(),
                ssrc: ssrc::derive_wasm_participant_ssrc(
                    &call_id,
                    &peer_id,
                    ssrc::WASM_RELAY_STREAM_SLOT_WORDS[0],
                ),
                samples_per_packet: AudioFormat::OPUS_16KHZ_60MS.rtp_timestamp_step,
                warp_mi_tag_len: 4,
            })
            .expect("peer group audio pipeline");
            assert!(pipe.set_audio_payload_type(AudioFormat::OPUS_16KHZ_60MS.rtp_payload_type));
            pipe
        };
        let mut old_sender = sender(&old_epoch);
        let mut engine = group_engine();
        assert_eq!(
            engine.apply_group_raw_epoch(7, &old_epoch).unwrap(),
            GroupEpochApply::Installed
        );

        let before = old_sender.protect_audio(&[0x08, 1, 2, 3]);
        engine.handle_input(1, Input::RelayPacket(&before));
        assert!(
            drain(&mut engine)
                .iter()
                .any(|output| matches!(output, Output::EncodedAudio(_))),
            "the installed epoch must initially admit inbound media"
        );

        let mut rekey = update;
        rekey.transaction_id = 8;
        rekey.rekey_requested = true;
        assert_eq!(
            engine.apply_group_update(2, &rekey).unwrap(),
            GroupRosterApply::Applied
        );
        let retired = old_sender.protect_audio(&[0x08, 4, 5, 6]);
        engine.handle_input(3, Input::RelayPacket(&retired));
        assert!(
            drain(&mut engine)
                .iter()
                .all(|output| !matches!(output, Output::EncodedAudio(_))),
            "old-key inbound media must remain gated while the requested epoch is pending"
        );

        assert_eq!(
            engine.apply_group_raw_epoch(8, &new_epoch).unwrap(),
            GroupEpochApply::Installed
        );
        assert!(old_sender.rekey_send_from_raw(&new_epoch, &peer_jid.to_string()));
        let resumed = old_sender.protect_audio(&[0x08, 7, 8, 9]);
        engine.handle_input(4, Input::RelayPacket(&resumed));
        assert!(
            drain(&mut engine)
                .iter()
                .any(|output| matches!(output, Output::EncodedAudio(_))),
            "inbound media must resume under the requested epoch"
        );
    }

    #[test]
    fn requested_group_rekey_gates_inbound_rtcp_and_its_replay_index() {
        use crate::voip::e2e_srtp::{derive_srtcp_keys_from_raw, protect_srtcp};
        use crate::voip::rtcp::RTCP_PT_RR;

        let old_epoch = [0x42; 32];
        let new_epoch = [0x48; 32];
        let update = group_update();
        let call_id = update.call_id.clone();
        let peer_jid = update.participants[1].devices[0].jid.clone();
        let peer_id = ssrc::format_e2e_srtp_participant_id(&peer_jid.to_string());
        let peer_ssrc = ssrc::derive_wasm_participant_ssrc(
            &call_id,
            &peer_id,
            ssrc::WASM_RELAY_STREAM_SLOT_WORDS[0],
        );
        let mut engine = group_engine();
        assert_eq!(
            engine.apply_group_raw_epoch(7, &old_epoch).unwrap(),
            GroupEpochApply::Installed
        );
        let local_ssrc = engine.media.as_ref().expect("group media").pipe.send_ssrc();
        let protect = |epoch: &[u8], index| {
            let mut rr = vec![0x81, RTCP_PT_RR, 0, 7];
            rr.extend_from_slice(&peer_ssrc.to_be_bytes());
            rr.extend_from_slice(&local_ssrc.to_be_bytes());
            rr.extend_from_slice(&[0; 20]);
            let keys = derive_srtcp_keys_from_raw(epoch, &peer_id).expect("peer group SRTCP keys");
            protect_srtcp(&keys, peer_ssrc, index, &rr)
        };

        engine.handle_input(1, Input::RelayPacket(&protect(&old_epoch, 0)));
        assert!(
            drain(&mut engine)
                .iter()
                .any(|output| matches!(output, Output::Event(CallEvent::RtcpReceived { .. }))),
            "the installed epoch must initially admit authenticated RTCP"
        );

        let mut rekey = update;
        rekey.transaction_id = 8;
        rekey.rekey_requested = true;
        assert_eq!(
            engine.apply_group_update(2, &rekey).unwrap(),
            GroupRosterApply::Applied
        );
        engine.handle_input(3, Input::RelayPacket(&protect(&old_epoch, 10_000)));
        assert!(
            drain(&mut engine)
                .iter()
                .all(|output| !matches!(output, Output::Event(CallEvent::RtcpReceived { .. }))),
            "old-key RTCP must not advance replay state while the requested epoch is pending"
        );

        assert_eq!(
            engine.apply_group_raw_epoch(8, &new_epoch).unwrap(),
            GroupEpochApply::Installed
        );
        engine.handle_input(4, Input::RelayPacket(&protect(&new_epoch, 1)));
        assert!(
            drain(&mut engine)
                .iter()
                .any(|output| matches!(output, Output::Event(CallEvent::RtcpReceived { .. }))),
            "the low legitimate index must remain admissible after the requested rekey"
        );
    }

    #[test]
    fn roster_only_group_update_rebuilds_cached_relay_subscriptions() {
        let mut engine =
            CallEngine::new(config(), Box::new(SequentialTxIds::new())).expect("direct engine");
        engine
            .apply_group_update(0, &group_update())
            .expect("promote to group");
        engine.start(0, 1_700_000_000_000);
        let _ = drain(&mut engine);

        let mut update = group_update();
        update.transaction_id = 8;
        let participant = Jid::new("15550003333", Server::Lid);
        update.participants.push(GroupCallParticipant {
            jid: participant.clone(),
            pn: None,
            state: Some("connected".to_string()),
            participant_type: None,
            devices: vec![GroupCallDevice {
                jid: participant,
                platform: None,
                pid: Some(3),
                capability_version: None,
                capability: Vec::new(),
            }],
        });
        assert!(
            update.relay.is_none(),
            "the update intentionally reuses relay state"
        );

        engine
            .apply_group_update(1, &update)
            .expect("apply roster-only update");
        let allocation = drain(&mut engine)
            .into_iter()
            .find_map(|output| match output {
                Output::Transmit(packet) => Some(packet),
                _ => None,
            })
            .expect("updated allocation");
        let subscriptions = stun::create_wasm_group_receiver_subscriptions(&[2, 3]);
        assert!(
            allocation
                .windows(subscriptions.len())
                .any(|window| window == subscriptions),
            "the refreshed allocation must subscribe to the complete current PID roster"
        );
    }

    // A group call never reaches the direct receive path: `on_group_rtp` returns before anything
    // the watchdog counts, so arming it there would report every healthy group call as stalled
    // three seconds in -- a false alarm on the one event that is supposed to mean something.
    #[test]
    fn a_group_call_never_reports_a_direct_audio_stall() {
        let mut engine = group_engine();
        engine.start(0, 1_700_000_000_000);
        let _ = drain(&mut engine);
        let allocate_success = allocation_success(&engine);
        engine.handle_input(1, Input::RelayPacket(&allocate_success));
        let _ = drain(&mut engine);
        for tick in 1..=40u64 {
            engine.handle_input(tick * 500, Input::Timeout);
            for output in drain(&mut engine) {
                assert!(
                    !matches!(
                        output,
                        Output::Event(CallEvent::AudioReceptionStalled { .. })
                            | Output::Event(CallEvent::AudioSilent { .. })
                    ),
                    "a group call must not raise the direct-audio alarms, got {output:?}"
                );
            }
        }
    }

    #[test]
    fn group_relay_endpoint_change_requests_reconnect_before_allocate() {
        let mut engine = group_engine();
        engine.start(0, 1_700_000_000_000);
        let _ = drain(&mut engine);
        let allocate_success = allocation_success(&engine);
        engine.handle_input(1, Input::RelayPacket(&allocate_success));
        assert!(
            drain(&mut engine)
                .iter()
                .any(|output| matches!(output, Output::Event(CallEvent::RelayAllocated)))
        );

        let mut update = group_update();
        update.transaction_id = 8;
        let mut relay = group_relay();
        relay.transaction_id = Some(8);
        relay.endpoints[0].ipv4 = Some("203.0.113.8".to_string());
        relay.endpoints[0].port = Some(3481);
        update.relay = Some(relay);
        engine
            .apply_group_update(2_000, &update)
            .expect("relay migration update");

        let outputs = drain(&mut engine);
        let expected = "203.0.113.8:3481".parse().unwrap();
        let reconnect_index = outputs
            .iter()
            .position(|output| {
                matches!(output, Output::ReconnectRelay(endpoint) if *endpoint == expected)
            })
            .expect("reconnect intent");
        let allocate_index = outputs
            .iter()
            .position(|output| matches!(output, Output::Transmit(_)))
            .expect("replacement allocate");
        assert!(
            reconnect_index < allocate_index,
            "the shell must redial before sending the replacement allocate"
        );

        assert_eq!(
            engine.allocate_deadline, NEVER,
            "the reconnect handshake must not consume the allocation response budget"
        );
        engine.relay_reconnected(14_000);
        engine.handle_input(23_999, Input::Timeout);
        assert!(
            drain(&mut engine)
                .iter()
                .all(|output| !matches!(output, Output::Event(CallEvent::RelayAllocateTimedOut))),
            "the replacement allocation retains its full response budget after reconnect"
        );
        engine.handle_input(24_000, Input::Timeout);
        assert!(
            drain(&mut engine)
                .iter()
                .any(|output| matches!(output, Output::Event(CallEvent::RelayAllocateTimedOut))),
            "a replacement relay must retain the initial allocation timeout safety net"
        );
    }

    #[test]
    fn group_relay_credential_refresh_rearms_allocation_on_the_same_endpoint() {
        let mut engine = group_engine();
        engine.start(0, 1_700_000_000_000);
        let _ = drain(&mut engine);
        let allocate_success = allocation_success(&engine);
        engine.handle_input(1, Input::RelayPacket(&allocate_success));
        let _ = drain(&mut engine);
        assert!(engine.is_allocated());

        let mut update = group_update();
        update.transaction_id = 8;
        let mut relay = group_relay();
        relay.transaction_id = Some(8);
        relay.key = b"rotated-relay-key".to_vec();
        relay.tokens[0] = vec![0x48];
        relay.auth_tokens[0] = vec![0x58];
        update.relay = Some(relay);
        engine
            .apply_group_update(2_000, &update)
            .expect("credential refresh");

        assert!(
            !engine.is_allocated(),
            "new credentials require a fresh allocation acknowledgement"
        );
        let outputs = drain(&mut engine);
        assert!(
            outputs
                .iter()
                .any(|output| matches!(output, Output::Transmit(_))),
            "the credential refresh must emit a replacement allocation"
        );
        assert!(
            !outputs
                .iter()
                .any(|output| matches!(output, Output::ReconnectRelay(_))),
            "unchanged relay coordinates must not reconnect the socket"
        );

        engine.handle_input(2_000 + ALLOCATE_TIMEOUT_MS, Input::Timeout);
        assert!(
            drain(&mut engine)
                .iter()
                .any(|output| matches!(output, Output::Event(CallEvent::RelayAllocateTimedOut))),
            "the replacement allocation must retain the timeout safety net"
        );
    }

    #[test]
    fn group_relay_tag_length_refresh_is_rejected_before_roster_commit() {
        let mut engine = group_engine();
        let mut update = group_update();
        update.transaction_id = 8;
        let mut relay = group_relay();
        relay.transaction_id = Some(8);
        relay.warp_mi_tag_len = Some(6);
        update.relay = Some(relay);

        assert!(matches!(
            engine.apply_group_update(2_000, &update),
            Err(EngineError::GroupMedia(GroupMediaError::Pipeline))
        ));
        assert_eq!(
            engine
                .group
                .as_ref()
                .and_then(|group| group.registry.roster_transaction()),
            Some(7),
            "an unsupported tag-boundary transition cannot consume the roster transaction"
        );
    }

    #[test]
    fn unchanged_relay_and_subscriptions_keep_the_healthy_allocation() {
        let mut engine = group_engine();
        engine.start(0, 1_700_000_000_000);
        let _ = drain(&mut engine);
        let success = allocation_success(&engine);
        engine.handle_input(1, Input::RelayPacket(&success));
        let _ = drain(&mut engine);
        assert!(engine.is_allocated());

        let mut update = group_update();
        update.transaction_id = 8;
        let mut relay = group_relay();
        relay.transaction_id = Some(8);
        update.relay = Some(relay);
        engine
            .apply_group_update(2_000, &update)
            .expect("idempotent relay snapshot");
        assert!(engine.is_allocated());
        assert!(
            drain(&mut engine)
                .iter()
                .all(|output| !matches!(output, Output::Transmit(_))),
            "unchanged relay material and PID subscriptions require no replacement allocate"
        );

        engine.handle_input(2_000 + ALLOCATE_TIMEOUT_MS, Input::Timeout);
        assert!(
            drain(&mut engine)
                .iter()
                .all(|output| !matches!(output, Output::Event(CallEvent::RelayAllocateTimedOut))),
            "an idempotent roster refresh cannot arm a fatal allocation timeout"
        );
    }

    #[test]
    fn roster_only_pid_change_tracks_subscription_refresh_ack_and_timeout() {
        let mut engine = group_engine();
        engine.start(0, 1_700_000_000_000);
        let _ = drain(&mut engine);
        let success = allocation_success(&engine);
        engine.handle_input(1, Input::RelayPacket(&success));
        let _ = drain(&mut engine);
        assert!(engine.is_allocated());

        let mut update = group_update();
        update.transaction_id = 8;
        let peer = Jid::new("15550003333", Server::Lid);
        update.participants.push(GroupCallParticipant {
            jid: peer.clone(),
            pn: None,
            state: Some("connected".to_string()),
            participant_type: None,
            devices: vec![GroupCallDevice {
                jid: peer,
                platform: None,
                pid: Some(3),
                capability_version: None,
                capability: Vec::new(),
            }],
        });
        let mut relay = group_relay();
        relay.transaction_id = Some(8);
        update.relay = Some(relay);
        engine
            .apply_group_update(2_000, &update)
            .expect("roster-only PID refresh");

        assert!(
            engine.is_allocated(),
            "subscription refreshes must not gate an already healthy media allocation"
        );
        assert!(
            drain(&mut engine)
                .iter()
                .any(|output| matches!(output, Output::Transmit(_))),
            "new remote PIDs still require an updated group Allocate"
        );
        let refresh_success = allocation_success(&engine);
        engine.handle_input(2_001, Input::RelayPacket(&refresh_success));
        assert!(
            drain(&mut engine)
                .iter()
                .all(|output| !matches!(output, Output::Event(CallEvent::RelayAllocated))),
            "a subscription-only ACK must not re-announce the already active allocation"
        );
        engine.handle_input(2_000 + ALLOCATE_TIMEOUT_MS, Input::Timeout);
        assert!(
            drain(&mut engine)
                .iter()
                .all(|output| !matches!(output, Output::Event(CallEvent::RelayAllocateTimedOut))),
            "the matching subscription refresh ACK clears its deadline"
        );

        let mut unacked = update;
        unacked.transaction_id = 9;
        let next_peer = Jid::new("15550004444", Server::Lid);
        unacked.participants.push(GroupCallParticipant {
            jid: next_peer.clone(),
            pn: None,
            state: Some("connected".to_string()),
            participant_type: None,
            devices: vec![GroupCallDevice {
                jid: next_peer,
                platform: None,
                pid: Some(4),
                capability_version: None,
                capability: Vec::new(),
            }],
        });
        unacked
            .relay
            .as_mut()
            .expect("relay snapshot")
            .transaction_id = Some(9);
        engine
            .apply_group_update(3_000, &unacked)
            .expect("second PID refresh");
        let _ = drain(&mut engine);
        engine.handle_input(3_000 + ALLOCATE_TIMEOUT_MS, Input::Timeout);
        assert!(
            drain(&mut engine)
                .iter()
                .any(|output| matches!(output, Output::Event(CallEvent::RelayAllocateTimedOut))),
            "a lost subscription refresh must retain the allocation timeout safety net"
        );
    }

    #[test]
    fn group_relay_refresh_ignores_stale_allocation_responses() {
        let mut engine = group_engine();
        engine.start(0, 1_700_000_000_000);
        let _ = drain(&mut engine);
        let stale_transaction = engine
            .allocate_transaction_id
            .expect("initial allocation transaction");
        let initial_success = allocation_success(&engine);
        engine.handle_input(1, Input::RelayPacket(&initial_success));
        let _ = drain(&mut engine);
        assert!(engine.is_allocated());

        let mut update = group_update();
        update.transaction_id = 8;
        let mut relay = group_relay();
        relay.transaction_id = Some(8);
        relay.key = b"rotated-relay-key".to_vec();
        relay.tokens[0] = vec![0x48];
        relay.auth_tokens[0] = vec![0x58];
        update.relay = Some(relay);
        engine
            .apply_group_update(2_000, &update)
            .expect("credential refresh");
        let current_transaction = engine
            .allocate_transaction_id
            .expect("replacement allocation transaction");
        assert_ne!(stale_transaction, current_transaction);
        let _ = drain(&mut engine);

        let stale_success = stun::encode_stun_request(
            stun::MSG_ALLOCATE_SUCCESS,
            &stale_transaction,
            &[],
            None,
            false,
        );
        engine.handle_input(2_001, Input::RelayPacket(&stale_success));
        engine.handle_input(
            2_002,
            Input::RelayPacket(&allocation_error(&stale_transaction, 486)),
        );
        assert!(
            !engine.is_allocated() && !engine.is_terminated(),
            "a previous allocation generation cannot complete or fail the refreshed one"
        );
        assert!(
            drain(&mut engine).iter().all(|output| !matches!(
                output,
                Output::Event(CallEvent::RelayAllocated | CallEvent::RelayAllocateFailed(_))
            )),
            "stale allocation responses must be silent"
        );

        let current_success = allocation_success(&engine);
        engine.handle_input(2_003, Input::RelayPacket(&current_success));
        assert!(engine.is_allocated());
        assert!(
            drain(&mut engine)
                .iter()
                .any(|output| matches!(output, Output::Event(CallEvent::RelayAllocated))),
            "only the current allocation transaction may complete the refresh"
        );
    }

    #[test]
    fn group_reaction_uses_pt119_retransmits_and_deduplicates_per_sender() {
        let epoch = [0x42; 32];
        let update = group_update();
        let self_jid = update.participants[0].devices[0].jid.clone();
        let peer_jid = update.participants[1].devices[0].jid.clone();
        let mut engine =
            CallEngine::new(config(), Box::new(SequentialTxIds::new())).expect("engine");
        engine
            .configure_group(GroupEngineConfig {
                call_creator: update.call_creator.clone(),
                self_jid: self_jid.clone(),
                initial_update: update,
                direct_peer: None,
            })
            .unwrap();
        engine.apply_group_raw_epoch(7, &epoch).unwrap();
        engine.start(0, 1_700_000_000_000);
        let _ = drain(&mut engine);

        engine.send_group_reaction(10, "👍").unwrap();
        let first = drain(&mut engine)
            .into_iter()
            .find_map(|output| match output {
                Output::Transmit(packet) => Some(packet),
                _ => None,
            })
            .expect("initial reaction packet");
        let mut receiver = MediaPipeline::new(&MediaPipelineParams {
            call_key: &epoch,
            self_lid: &peer_jid.to_string(),
            peer_lid: &self_jid.to_string(),
            ssrc: 1,
            samples_per_packet: 50,
            warp_mi_tag_len: 4,
        })
        .unwrap();
        let (header, payload) = receiver.unprotect_audio(&first).unwrap();
        assert_eq!(header.payload_type, RTP_PAYLOAD_TYPE_APP_DATA);
        assert_eq!(app_data::decode_reactions(&payload).unwrap()[0].emoji, "👍");
        for retransmit in 1..APP_DATA_RETRANSMIT_COUNT {
            engine.handle_input(
                10 + u64::from(retransmit) * APP_DATA_RETRANSMIT_MS,
                Input::Timeout,
            );
            assert_eq!(
                drain(&mut engine)
                    .iter()
                    .filter(|output| matches!(output, Output::Transmit(_)))
                    .count(),
                1
            );
        }

        let peer_id = ssrc::format_e2e_srtp_participant_id(&peer_jid.to_string());
        let mut sender = MediaPipeline::new(&MediaPipelineParams {
            call_key: &epoch,
            self_lid: &peer_jid.to_string(),
            peer_lid: &self_jid.to_string(),
            ssrc: ssrc::derive_wasm_participant_ssrc(
                "ENCODED-AUDIO-TEST",
                &peer_id,
                ssrc::APP_DATA_SSRC_SLOT_WORD,
            ),
            samples_per_packet: 50,
            warp_mi_tag_len: 4,
        })
        .unwrap();
        assert!(sender.set_audio_payload_type(RTP_PAYLOAD_TYPE_APP_DATA));
        sender.set_audio_mlow_profile(false);
        let inbound = sender.protect_audio(&app_data::encode_reaction(9, "👏").unwrap());
        engine.handle_input(500, Input::RelayPacket(&inbound));
        assert!(drain(&mut engine).iter().any(|output| matches!(
            output,
            Output::Event(CallEvent::Reaction {
                participant,
                emoji,
                removed: false,
                ..
            }) if *participant == peer_jid.to_non_ad() && emoji.as_deref() == Some("👏")
        )));
        engine.handle_input(501, Input::RelayPacket(&inbound));
        assert!(
            !drain(&mut engine)
                .iter()
                .any(|output| matches!(output, Output::Event(CallEvent::Reaction { .. })))
        );
        let removal = sender.protect_audio(&app_data::encode_reaction(10, "").unwrap());
        engine.handle_input(502, Input::RelayPacket(&removal));
        assert!(drain(&mut engine).iter().any(|output| matches!(
            output,
            Output::Event(CallEvent::Reaction {
                participant,
                emoji: None,
                removed: true,
                ..
            }) if *participant == peer_jid.to_non_ad()
        )));

        let mut migrated = group_update();
        migrated.transaction_id = 8;

        migrated.participants[1].devices[0].pid = Some(9);
        engine
            .apply_group_update(1, &migrated)
            .expect("replace the participant media session");
        let restarted = sender.protect_audio(&app_data::encode_reaction(1, "🔄").unwrap());
        engine.handle_input(503, Input::RelayPacket(&restarted));
        assert!(drain(&mut engine).iter().any(|output| matches!(
            output,
            Output::Event(CallEvent::Reaction {
                participant,
                pid: Some(9),
                emoji,
                removed: false,
                ..
            }) if *participant == peer_jid.to_non_ad() && emoji.as_deref() == Some("🔄")
        )));

        let mut departed = group_update();
        departed.transaction_id = 9;
        departed.participants.truncate(1);
        engine
            .apply_group_update(2, &departed)
            .expect("remove participant");
        let mut rejoined = group_update();
        rejoined.transaction_id = 10;
        engine
            .apply_group_update(3, &rejoined)
            .expect("rejoin participant");
        let restarted = sender.protect_audio(&app_data::encode_reaction(1, "✅").unwrap());
        engine.handle_input(504, Input::RelayPacket(&restarted));
        assert!(drain(&mut engine).iter().any(|output| matches!(
            output,
            Output::Event(CallEvent::Reaction {
                participant,
                emoji,
                removed: false,
                ..
            }) if *participant == peer_jid.to_non_ad() && emoji.as_deref() == Some("✅")
        )));
    }

    #[test]
    fn pending_group_reaction_retransmissions_are_bounded() {
        let epoch = [0x42; 32];
        let update = group_update();
        let self_jid = update.participants[0].devices[0].jid.clone();
        let mut engine =
            CallEngine::new(config(), Box::new(SequentialTxIds::new())).expect("engine");
        engine
            .configure_group(GroupEngineConfig {
                call_creator: update.call_creator.clone(),
                self_jid,
                initial_update: update,
                direct_peer: None,
            })
            .unwrap();
        engine.apply_group_raw_epoch(7, &epoch).unwrap();
        engine.start(0, 1_700_000_000_000);
        let _ = drain(&mut engine);

        for index in 0..=MAX_PENDING_REACTIONS {
            engine
                .send_group_reaction(10, &format!("reaction-{index}"))
                .unwrap();
            let _ = drain(&mut engine);
        }

        let pending = &engine.group.as_ref().unwrap().pending_reactions;
        assert_eq!(pending.len(), MAX_PENDING_REACTIONS);
        assert_eq!(
            app_data::decode_reactions(&pending.front().unwrap().payload).unwrap()[0]
                .transaction_id,
            2,
            "the newest bounded retry window must supersede the oldest pending reaction"
        );
    }

    #[test]
    fn group_rekey_discards_reaction_retries_protected_with_the_old_epoch() {
        let epoch = [0x42; 32];
        let update = group_update();
        let self_jid = update.participants[0].devices[0].jid.clone();
        let mut engine =
            CallEngine::new(config(), Box::new(SequentialTxIds::new())).expect("engine");
        engine
            .configure_group(GroupEngineConfig {
                call_creator: update.call_creator.clone(),
                self_jid,
                initial_update: update,
                direct_peer: None,
            })
            .unwrap();
        engine.apply_group_raw_epoch(7, &epoch).unwrap();
        engine.start(0, 1_700_000_000_000);
        let _ = drain(&mut engine);
        engine.send_group_reaction(10, "👍").unwrap();
        let _ = drain(&mut engine);

        let mut rekey = group_update();
        rekey.transaction_id = 8;
        rekey.rekey_requested = true;
        assert_eq!(
            engine.apply_group_update(20, &rekey).unwrap(),
            GroupRosterApply::Applied
        );
        assert!(
            engine
                .group
                .as_ref()
                .is_some_and(|group| group.pending_reactions.is_empty()),
            "retries protected with the old app-data key must not cross the epoch boundary"
        );
        let _ = drain(&mut engine);

        engine.handle_input(10 + APP_DATA_RETRANSMIT_MS, Input::Timeout);
        assert!(
            drain(&mut engine)
                .iter()
                .all(|output| !matches!(output, Output::Transmit(_))),
            "the due old-epoch retry must not be transmitted while rekey is pending"
        );
    }

    #[test]
    fn encoded_opus_round_trips_without_a_core_codec() {
        let cfg = config();
        let mut engine =
            CallEngine::new(cfg.clone(), Box::new(SequentialTxIds::new())).expect("engine");
        engine.start(0, 1_700_000_000_000);
        let _ = drain(&mut engine);

        let mut peer = MediaPipeline::new(&MediaPipelineParams {
            call_key: &cfg.call_key,
            self_lid: PEER_LID,
            peer_lid: SELF_LID,
            ssrc: cfg.ssrc,
            samples_per_packet: AudioFormat::OPUS_16KHZ_60MS.rtp_timestamp_step,
            warp_mi_tag_len: cfg.warp_mi_tag_len,
        })
        .expect("peer pipeline");
        assert!(peer.set_audio_payload_type(AudioFormat::OPUS_16KHZ_60MS.rtp_payload_type));

        let outbound = [0x08, 0x11, 0x22, 0x33];
        engine.handle_input(1, Input::EncodedAudio(&outbound));
        let protected = drain(&mut engine)
            .into_iter()
            .find_map(|output| match output {
                Output::Transmit(packet) => Some(packet),
                _ => None,
            })
            .expect("protected audio");
        let (header, recovered) = peer
            .unprotect_audio(&protected)
            .expect("peer decrypts outbound audio");
        assert_eq!(header.payload_type, 120);
        assert!(!header.marker);
        assert_eq!(recovered, outbound);

        let inbound = [0x08, 0x44, 0x55, 0x66];
        let protected = peer.protect_audio(&inbound);
        engine.handle_input(2, Input::RelayPacket(&protected));
        let frame = drain(&mut engine)
            .into_iter()
            .find_map(|output| match output {
                Output::EncodedAudio(frame) => Some(frame),
                _ => None,
            })
            .expect("encoded output");
        assert_eq!(frame.data.as_ref(), inbound);
        assert_eq!(frame.payload_type, 120);
        assert_eq!(frame.codec, AudioCodec::Opus);
        assert_eq!(frame.format, AudioFormat::OPUS_16KHZ_60MS);
    }

    // A source fixed to the escape keeps sending escape bytes after the call downgrades off it --
    // it is immutable by contract. Compared on the codec alone both formats read `Opus`, so the
    // guard passed and rewritten TOCs went to a peer that registered RFC Opus: the outbound half of
    // #1105, from a fix meant to cure it. The engine restores the byte the escape rewrote.
    #[test]
    fn an_encoded_escape_source_is_translated_after_a_downgrade_to_native_opus() {
        let mut cfg = config();
        cfg.audio = AudioConfig::encoded(AudioFormat::OPUS_MLOW_16KHZ_60MS);
        let mut engine =
            CallEngine::new(cfg.clone(), Box::new(SequentialTxIds::new())).expect("engine");
        engine.start(0, 1_700_000_000_000);
        let _ = drain(&mut engine);

        let mut peer = MediaPipeline::new(&MediaPipelineParams {
            call_key: &cfg.call_key,
            self_lid: PEER_LID,
            peer_lid: SELF_LID,
            ssrc: cfg.ssrc,
            samples_per_packet: AudioFormat::OPUS_16KHZ_60MS.rtp_timestamp_step,
            warp_mi_tag_len: cfg.warp_mi_tag_len,
        })
        .expect("peer pipeline");
        assert!(peer.set_audio_payload_type(AudioFormat::OPUS_16KHZ_60MS.rtp_payload_type));

        // The same packet in both spellings: TOC 0xDD is what the escape puts on the wire for the
        // RFC code-3 packet 0xEB, and only the first byte differs.
        let escaped = [0xDDu8, 0x03, 0x11, 0x22, 0x33];
        let mut rfc = escaped;
        depacketize_opus_from_mlow(&mut rfc).expect("the escape is translatable");
        assert_ne!(rfc[0], escaped[0]);

        engine
            .switch_audio_codec(AudioCodec::Opus, CodecDecisionSource::Negotiated)
            .expect("the peer cleared the capability");
        assert_eq!(engine.active_audio_codec(), Some(AudioCodec::Opus));

        engine.handle_input(2, Input::EncodedAudio(&escaped));
        let protected: Vec<_> = drain(&mut engine)
            .into_iter()
            .filter_map(|output| match output {
                Output::Transmit(packet) => Some(packet),
                _ => None,
            })
            .collect();
        assert_eq!(protected.len(), 1, "the frame still goes out");
        let (_, payload) = peer
            .unprotect_audio(&protected[0])
            .expect("peer decrypts outbound audio");
        assert_eq!(
            payload, rfc,
            "and carries the RFC TOC the peer's decoder expects"
        );
        assert_eq!(engine.media_stats().outbound_frames_without_encoder, 0);

        // The escape's SID token has no RFC spelling. Dropped rather than sent as garbage, and
        // counted so a source stuck in DTX is visible.
        engine.handle_input(3, Input::EncodedAudio(&[0x90]));
        assert!(
            drain(&mut engine)
                .iter()
                .all(|output| !matches!(output, Output::Transmit(_)))
        );
        assert_eq!(engine.media_stats().outbound_frames_without_encoder, 1);
    }

    /// A decoder that fills every packet with one recognisable sample value, so a test can follow
    /// decoded audio all the way to playout without pulling libopus into `wacore`.
    #[cfg(not(feature = "voip-mlow"))]
    struct DecodeToConstant(i16);

    #[cfg(not(feature = "voip-mlow"))]
    impl ForeignAudioCodec for DecodeToConstant {
        fn decode(
            &mut self,
            _payload: &[u8],
            out: &mut Vec<i16>,
        ) -> Result<(), crate::voip::audio::ForeignCodecError> {
            out.extend(core::iter::repeat_n(self.0, MIC_FRAME_SAMPLES));
            Ok(())
        }

        fn conceal(&mut self, samples: usize, out: &mut Vec<i16>) {
            out.resize(out.len() + samples, 0);
        }

        fn encode(
            &mut self,
            _pcm: &[i16],
            _out: &mut Vec<u8>,
        ) -> Result<(), crate::voip::audio::ForeignCodecError> {
            Err(crate::voip::audio::ForeignCodecError::InvalidPayload)
        }
    }

    /// An encoder that answers with a fixed payload, so the send path can be proven without
    /// pulling libopus into `wacore`.
    #[cfg(not(feature = "voip-mlow"))]
    struct EncodeOnlyCodec;

    #[cfg(not(feature = "voip-mlow"))]
    impl ForeignAudioCodec for EncodeOnlyCodec {
        fn decode(
            &mut self,
            _payload: &[u8],
            _out: &mut Vec<i16>,
        ) -> Result<(), crate::voip::audio::ForeignCodecError> {
            Err(crate::voip::audio::ForeignCodecError::InvalidPayload)
        }

        fn conceal(&mut self, samples: usize, out: &mut Vec<i16>) {
            out.resize(out.len() + samples, 0);
        }

        fn encode(
            &mut self,
            pcm: &[i16],
            out: &mut Vec<u8>,
        ) -> Result<(), crate::voip::audio::ForeignCodecError> {
            out.extend(core::iter::repeat_n(0x58, pcm.len() / 16));
            Ok(())
        }
    }

    // `voip-libopus` is advertised as the standard-Opus PCM adapter, and standard Opus is what a
    // peer outside the MLOW rollout selects during signaling. Gated on PCM I/O rather than on the
    // MLOW format, the availability check refused every call that feature exists to serve, and the
    // send path was a stub besides -- so the combination could neither start such a call nor speak
    // on one. Runs only where the gap was: a build without the built-in codec.
    #[cfg(not(feature = "voip-mlow"))]
    #[test]
    fn standard_opus_pcm_works_without_the_built_in_codec() {
        let mut cfg = config();
        cfg.audio = AudioConfig::OPUS_PCM;
        let mut eng = CallEngine::new(cfg, Box::new(SequentialTxIds::new()))
            .expect("standard Opus PCM does not need the MLOW codec")
            .with_foreign_audio_codec(Box::new(EncodeOnlyCodec));
        eng.start(0, 0);
        let _ = drain(&mut eng);
        let success = allocation_success(&eng);
        eng.handle_input(0, Input::RelayPacket(&success));
        let _ = drain(&mut eng);

        eng.handle_input(1, Input::MicFrame(&[64i16; MIC_FRAME_SAMPLES]));
        let sent = drain(&mut eng)
            .into_iter()
            .filter(|output| matches!(output, Output::Transmit(_)))
            .count();
        assert_eq!(sent, 1, "the mic frame has to reach the wire");
        assert_eq!(eng.media_stats().outbound_frames_without_encoder, 0);
    }

    // Sending is only half of it. The injected decoder feeds the SAME jitter buffer the MLOW path
    // feeds, and both that transfer and the playout tick that drains it were gated on the built-in
    // codec -- so this configuration decoded every packet, reported `foreign_frames_decoded`, and
    // played nothing: the next packet cleared the samples. Silence with the counters saying
    // otherwise is the exact shape of #1105.
    #[cfg(not(feature = "voip-mlow"))]
    #[test]
    fn decoded_opus_reaches_playout_without_the_built_in_codec() {
        let mut cfg = config();
        cfg.audio = AudioConfig::OPUS_PCM;
        let mut eng = CallEngine::new(cfg.clone(), Box::new(SequentialTxIds::new()))
            .expect("standard Opus PCM does not need the MLOW codec")
            .with_foreign_audio_codec(Box::new(DecodeToConstant(4321)));
        eng.start(0, 0);
        let _ = drain(&mut eng);
        let success = allocation_success(&eng);
        eng.handle_input(0, Input::RelayPacket(&success));
        let _ = drain(&mut eng);

        let mut peer = MediaPipeline::new(&MediaPipelineParams {
            call_key: &cfg.call_key,
            self_lid: PEER_LID,
            peer_lid: SELF_LID,
            ssrc: cfg.ssrc,
            samples_per_packet: AudioFormat::OPUS_16KHZ_60MS.rtp_timestamp_step,
            warp_mi_tag_len: cfg.warp_mi_tag_len,
        })
        .expect("peer pipeline");
        assert!(peer.set_audio_payload_type(AudioFormat::OPUS_16KHZ_60MS.rtp_payload_type));

        let mut heard = Vec::new();
        for n in 0..8u64 {
            let packet = peer.protect_audio(&[0xE8, 0x11, 0x22, 0x33]);
            eng.handle_input(1 + n * 60, Input::RelayPacket(&packet));
            for tick in 0..3u64 {
                eng.handle_input(1 + n * 60 + tick * 20, Input::Timeout);
                for output in drain(&mut eng) {
                    if let Output::Playout(frame) = output {
                        heard.extend(frame);
                    }
                }
            }
        }

        assert!(eng.media_stats().foreign_frames_decoded > 0, "it decodes");
        assert!(
            heard.contains(&4321),
            "and what it decoded has to be what the consumer hears"
        );
    }

    #[test]
    fn opus_mlow_escape_uses_pt120_and_rejects_ambiguous_toc() {
        let mut cfg = config();
        cfg.audio = AudioConfig::encoded(AudioFormat::OPUS_MLOW_16KHZ_60MS);
        let mut engine =
            CallEngine::new(cfg.clone(), Box::new(SequentialTxIds::new())).expect("engine");
        engine.start(0, 1_700_000_000_000);
        let _ = drain(&mut engine);

        engine.handle_input(1, Input::EncodedAudio(&[0x08, 1, 2]));
        assert!(
            drain(&mut engine)
                .iter()
                .all(|output| !matches!(output, Output::Transmit(_)))
        );

        let mut peer = MediaPipeline::new(&MediaPipelineParams {
            call_key: &cfg.call_key,
            self_lid: PEER_LID,
            peer_lid: SELF_LID,
            ssrc: cfg.ssrc,
            samples_per_packet: AudioFormat::OPUS_MLOW_16KHZ_60MS.rtp_timestamp_step,
            warp_mi_tag_len: cfg.warp_mi_tag_len,
        })
        .expect("peer pipeline");
        assert!(peer.set_audio_payload_type(AudioFormat::OPUS_MLOW_16KHZ_60MS.rtp_payload_type));

        let outbound = [0xDD, 0x03, 0x11, 0x22, 0x33];
        engine.handle_input(2, Input::EncodedAudio(&outbound));
        let protected: Vec<_> = drain(&mut engine)
            .into_iter()
            .filter_map(|output| match output {
                Output::Transmit(packet) => Some(packet),
                _ => None,
            })
            .collect();
        assert_eq!(protected.len(), 1);
        let (header, payload) = peer
            .unprotect_audio(&protected[0])
            .expect("peer decrypts initial outbound audio");
        assert_eq!(
            header.payload_type,
            AudioFormat::OPUS_MLOW_16KHZ_60MS.rtp_payload_type
        );
        assert_eq!(payload, outbound);
        assert_eq!(
            (header.sequence_number, header.timestamp, header.marker),
            (1, 0, true)
        );

        let inbound_mlow = [0x50, 0x44, 0x55, 0x66];
        let protected = peer.protect_audio(&inbound_mlow);
        engine.handle_input(3, Input::RelayPacket(&protected));
        let frame = drain(&mut engine)
            .into_iter()
            .find_map(|output| match output {
                Output::EncodedAudio(frame) => Some(frame),
                _ => None,
            })
            .expect("encoded output");
        assert_eq!(frame.codec, AudioCodec::Mlow);
        assert_eq!(frame.data.as_ref(), inbound_mlow);
    }
}

#[cfg(all(test, feature = "voip-mlow"))]
mod tests {
    use super::*;
    use crate::types::group_call::{GroupCallDevice, GroupCallParticipant};
    use crate::voip::e2e_srtp::SRTCP_AUTH_TAG_LEN;
    use crate::voip::mlow::MlowEncoder;
    use crate::voip::rtcp::parse_rtcp_sender_ssrc;
    use crate::voip::warp::WARP_MI_TAG_LEN;
    use wacore_binary::Server;

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
            audio: AudioConfig::MLOW_PCM,
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

    fn group_update(media: &str) -> GroupCallUpdate {
        let self_user = Jid::new("111111111111111", Server::Lid);
        let peer_user = Jid::new("222222222222222", Server::Lid);
        let self_device = SELF_LID.parse::<Jid>().expect("self JID");
        let peer_device = PEER_LID.parse::<Jid>().expect("peer JID");
        GroupCallUpdate {
            call_id: "CID".to_string(),
            call_creator: self_user.clone(),
            group_jid: None,
            transaction_id: 7,
            media: media.to_string(),
            connected_limit: 32,
            joinable: true,
            av_upgradable: true,
            rekey_requested: false,
            participants: vec![
                GroupCallParticipant {
                    jid: self_user,
                    pn: None,
                    state: Some("connected".to_string()),
                    participant_type: None,
                    devices: vec![GroupCallDevice {
                        jid: self_device,
                        platform: None,
                        pid: Some(1),
                        capability_version: None,
                        capability: Vec::new(),
                    }],
                },
                GroupCallParticipant {
                    jid: peer_user,
                    pn: None,
                    state: Some("connected".to_string()),
                    participant_type: None,
                    devices: vec![GroupCallDevice {
                        jid: peer_device,
                        platform: None,
                        pid: Some(2),
                        capability_version: None,
                        capability: Vec::new(),
                    }],
                },
            ],
            relay: None,
        }
    }

    fn group_engine(video: bool) -> (CallEngine, [u8; 32]) {
        let mut cfg = config(true);
        cfg.enable_video = video;
        let mut engine =
            CallEngine::new(cfg, Box::new(SequentialTxIds::new())).expect("group engine");
        let update = group_update(if video { "video" } else { "audio" });
        engine
            .configure_group(GroupEngineConfig {
                call_creator: update.call_creator.clone(),
                self_jid: SELF_LID.parse().expect("self JID"),
                initial_update: update,
                direct_peer: None,
            })
            .expect("configure group");
        let epoch = [0x42; 32];
        assert_eq!(
            engine
                .apply_group_raw_epoch(7, &epoch)
                .expect("install epoch"),
            GroupEpochApply::Installed
        );
        (engine, epoch)
    }

    fn group_peer_audio(epoch: &[u8]) -> MediaPipeline {
        let peer_id = ssrc::format_e2e_srtp_participant_id(PEER_LID);
        MediaPipeline::new(&MediaPipelineParams {
            call_key: epoch,
            self_lid: PEER_LID,
            peer_lid: SELF_LID,
            ssrc: ssrc::derive_wasm_participant_ssrc("CID", &peer_id, 0),
            samples_per_packet: SAMPLES,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .expect("peer audio pipeline")
    }

    fn group_peer_video(epoch: &[u8]) -> VideoPipeline {
        use crate::voip::session::{VideoPipeline, VideoPipelineParams};
        let peer_id = ssrc::format_e2e_srtp_participant_id(PEER_LID);
        VideoPipeline::new(&VideoPipelineParams {
            call_key: epoch,
            self_lid: PEER_LID,
            peer_lid: SELF_LID,
            ssrc: ssrc::derive_video_participant_ssrc("CID", &peer_id),
            ts_stride: VIDEO_TS_STRIDE_15FPS,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .expect("peer video pipeline")
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
            ssrc::derive_wasm_participant_ssrc(
                "CID",
                &ssrc::format_e2e_srtp_participant_id(SELF_LID),
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
            ssrc::derive_wasm_participant_ssrc(
                "CID",
                &ssrc::format_e2e_srtp_participant_id(SELF_LID),
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
    fn allocate_success(eng: &CallEngine) -> Vec<u8> {
        let transaction_id = eng
            .allocate_transaction_id
            .expect("current allocation transaction");
        stun::encode_stun_request(
            stun::MSG_ALLOCATE_SUCCESS,
            &transaction_id,
            &[],
            None,
            false,
        )
    }

    fn allocate_error(eng: &CallEngine, code: u16) -> Vec<u8> {
        let class = (code / 100) as u8;
        let number = (code % 100) as u8;
        // Raw ERROR-CODE (0x0009) TLV: type, len=4, value (2 reserved bytes, class, number).
        let err_attr = [0x00, 0x09, 0x00, 0x04, 0x00, 0x00, class, number];
        let transaction_id = eng
            .allocate_transaction_id
            .expect("current allocation transaction");
        stun::encode_stun_request(
            stun::MSG_ALLOCATE_ERROR,
            &transaction_id,
            &err_attr,
            None,
            false,
        )
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
            drain_playout(
                &mut buf,
                &mut priming,
                &mut priming_ticks,
                OPUS_FRAME_SAMPS_60MS,
                playout_bounds(OPUS_FRAME_SAMPS_60MS).1
            )
            .iter()
            .all(|&s| s == 0),
            "below the prebuffer target playout primes with silence"
        );
        assert_eq!(buf.len(), 960, "priming must not consume the buffer");
        // The second frame reaches the target; playout now produces real audio.
        feed_frame(&mut buf);
        assert!(
            drain_playout(
                &mut buf,
                &mut priming,
                &mut priming_ticks,
                OPUS_FRAME_SAMPS_60MS,
                playout_bounds(OPUS_FRAME_SAMPS_60MS).1
            )
            .iter()
            .any(|&s| s != 0),
            "at the prebuffer target playout starts real audio"
        );
    }

    /// Shrinking the ceiling must never discard audio that is already queued. A 120 ms stream that
    /// drops to a shorter packet -- a genuine switch, or the `0x90` SID that `packetize_opus_for_mlow`
    /// canonicalizes DTX to, which declares 60 ms -- would otherwise trim the backlog built under the
    /// larger bound, clipping the tail of the utterance that is still playing out.
    #[test]
    fn a_smaller_packet_does_not_trim_the_existing_backlog() {
        const BIG: usize = 1920; // 120 ms
        const SMALL: usize = 960; // 60 ms, e.g. the canonical SID
        let (big_cap, small_cap) = (playout_bounds(BIG).1, playout_bounds(SMALL).1);
        assert!(
            small_cap < big_cap,
            "the premise: the ceiling really does shrink"
        );

        // A primed 120 ms stream carrying more than the smaller ceiling would allow.
        let mut cap = big_cap;
        let queued = small_cap + 480;
        assert!(
            queued <= big_cap,
            "the premise: legal under the bound it was built with"
        );

        // The shorter packet arrives: the ceiling must not drop below what is already queued.
        cap = effective_playout_cap(cap, SMALL, queued);
        assert!(
            cap >= queued,
            "shrinking to {cap} would discard {} queued samples",
            queued - cap
        );

        // Once the backlog has drained under the smaller bound, the ceiling follows it down.
        cap = effective_playout_cap(cap, SMALL, small_cap - 320);
        assert_eq!(cap, small_cap, "the ceiling must not stay large forever");
    }

    /// A peer sending 120 ms packets (WhatsApp Desktop) delivers 1920 samples at a time. The
    /// prebuffer and the latency ceiling were both sized around a 60 ms peer frame, so with the
    /// larger packet the cushion collapses to zero (one packet already meets the target) and, worse,
    /// two in flight exceed the cap and get trimmed — dropping audio on every arrival. Both have to
    /// scale with the packet the peer actually sends.
    #[test]
    fn playout_scales_its_cushion_to_the_peer_packet() {
        const P: usize = 1920; // 120 ms @ 16 kHz
        let feed_120 =
            |b: &mut VecDeque<i16>| b.extend((0..P as i32).map(|i| (i % 200) as i16 - 99));

        // One packet must NOT end priming: draining it takes its own 120 ms, so the buffer would be
        // empty again exactly when the next one is due, leaving nothing for a late arrival.
        let (mut buf, mut priming, mut ticks) = (VecDeque::new(), true, 0u32);
        feed_120(&mut buf);
        let first = drain_playout(&mut buf, &mut priming, &mut ticks, P, playout_bounds(P).1);
        assert!(
            first.iter().all(|&s| s == 0),
            "a single 120ms packet is a zero cushion; playout must keep priming"
        );

        // Two in flight must survive: the ceiling has to hold the cushion it just asked for.
        feed_120(&mut buf);
        let before = buf.len();
        let _ = drain_playout(&mut buf, &mut priming, &mut ticks, P, playout_bounds(P).1);
        assert!(
            buf.len() + PLAYOUT_DRAIN >= before,
            "the latency ceiling trimmed the 120ms cushion: {before} -> {} samples",
            buf.len()
        );
        assert!(!priming, "two packets is the cushion; playout must start");
    }

    // A stream that drops from 120 ms packets to 60 ms leaves a backlog above the new ceiling, and
    // in steady state the backlog does not fall on its own: each packet adds what playout removes.
    // Waiting for it to fit meant the 120 ms ceiling -- and its latency -- outlived the 120 ms
    // stream by the whole rest of the call.
    #[test]
    fn a_shrinking_packet_cadence_gives_the_latency_back() {
        const BIG: usize = 1920; // 120 ms @ 16 kHz
        const SMALL: usize = 960; // 60 ms
        let big_cap = playout_bounds(BIG).1;
        let small_cap = playout_bounds(SMALL).1;
        assert!(small_cap < big_cap, "the shorter packet asks for less");

        // The backlog a 120 ms stream primes to, which no 60 ms steady state will drain below.
        let queued = big_cap;
        let mut cap = big_cap;
        let mut steps = 0;
        while cap > small_cap {
            let next = effective_playout_cap(cap, SMALL, queued);
            assert!(
                next < cap,
                "the ceiling has to keep falling, stuck at {cap}"
            );
            assert!(
                cap - next <= SMALL,
                "and fall by at most one packet at a time, so the trim is never a chunk"
            );
            cap = next;
            steps += 1;
            assert!(steps < 100, "convergence must be quick, not eventual");
        }
        assert_eq!(cap, small_cap, "it settles exactly on the new target");

        // The other direction is immediate: a longer packet needs its cushion on arrival.
        assert_eq!(effective_playout_cap(small_cap, BIG, queued), big_cap);
        // And a backlog that already fits takes the new ceiling at once, with nothing to trim.
        assert_eq!(effective_playout_cap(big_cap, SMALL, 0), small_cap);
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
                drain_playout(
                    &mut buf,
                    &mut priming,
                    &mut priming_ticks,
                    OPUS_FRAME_SAMPS_60MS,
                    playout_bounds(OPUS_FRAME_SAMPS_60MS).1,
                )
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
        let ok = allocate_success(&eng);
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

    /// A peer pipeline keyed the mirror of `engine(true)`, so its packets authenticate here.
    fn peer_pipeline() -> MediaPipeline {
        let call_key: Vec<u8> = (0u8..32).collect();
        MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid: PEER_LID,
            peer_lid: SELF_LID,
            ssrc: SSRC,
            samples_per_packet: SAMPLES,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap()
    }

    /// Started, allocated, and with the health watchdog armed: the state a live call is in.
    fn allocated_engine() -> CallEngine {
        let mut eng = engine(true);
        eng.start(0, 0);
        let _ = drain(&mut eng);
        let success = allocate_success(&eng);
        eng.handle_input(0, Input::RelayPacket(&success));
        let _ = drain(&mut eng);
        eng
    }

    // A peer that ships plain codec bytes inside E2E-SRTP without SFrame-wrapping is a SUPPORTED
    // mode, not a failure. Counting the pass-through made every packet of such a call report an
    // authentication failure and pointed whoever read the counters at keys that were fine.
    #[test]
    fn a_peer_that_does_not_sframe_wrap_is_not_an_authentication_failure() {
        let mut cfg = config(true);
        cfg.enable_sframe = true;
        let mut eng = CallEngine::new(cfg, Box::new(SequentialTxIds::new())).unwrap();
        eng.start(0, 0);
        let _ = drain(&mut eng);
        let success = allocate_success(&eng);
        eng.handle_input(0, Input::RelayPacket(&success));
        let _ = drain(&mut eng);

        let mut peer_tx = peer_pipeline();
        for n in 0..5 {
            // Shorter than a GCM tag plus a header, so it carries no SFrame framing at all.
            let packet = peer_tx.protect_audio(&[0x50, 1, 2, n]);
            eng.handle_input(u64::from(n) + 1, Input::RelayPacket(&packet));
            let _ = drain(&mut eng);
        }
        assert_eq!(
            eng.media_stats().rtp_received,
            5,
            "the packets authenticated"
        );
        assert_eq!(
            eng.media_stats().sframe_decrypt_failed,
            0,
            "an unwrapped frame is not a tag that failed"
        );
    }

    // `classify_relay_packet` needs two bytes and the RTP version bits to call a datagram RTP;
    // reading a header needs twelve. The gap used to `return` with no counter, so a stream of
    // RTP-shaped garbage left every discard counter at zero and the watchdog reported a reception
    // that never started -- the exact ambiguity this PR exists to remove.
    #[test]
    fn an_rtp_shaped_packet_too_short_to_parse_is_counted() {
        let mut eng = allocated_engine();
        // Version 2, payload type 120, and nothing else: classified as RTP, unparseable as RTP.
        for n in 0..5u8 {
            eng.handle_input(u64::from(n) + 1, Input::RelayPacket(&[0x80, 0x78, n]));
            let _ = drain(&mut eng);
        }
        let stats = eng.media_stats();
        assert_eq!(
            stats.relay_packet_unclassified, 5,
            "each one leaves a trace"
        );
        assert_eq!(stats.rtp_received, 0, "none of them was a packet we read");
    }

    // Every discard on the receive path has to leave a trace. A payload type outside the negotiated
    // profile used to `return` with no log and no counter, which is indistinguishable from a peer
    // who stopped sending -- the ambiguity that kept issue #1105 open.
    #[test]
    fn an_unexpected_payload_type_is_counted_rather_than_silently_dropped() {
        let mut eng = allocated_engine();
        let mut peer_tx = peer_pipeline();
        let mut packet = peer_tx.protect_audio(&[0x50, 1, 2, 3]);
        // Rewrite the payload type in place; the WARP tag is over the packet, so this also makes it
        // fail authentication. Assert only on the PT counter, which is checked first.
        packet[1] = (packet[1] & 0x80) | 99;
        eng.handle_input(1, Input::RelayPacket(&packet));
        let _ = drain(&mut eng);
        let stats = eng.media_stats();
        assert_eq!(stats.rtp_payload_type_unexpected, 1);
        assert_eq!(stats.rtp_received, 0);
    }

    // A peer that switched RTP profiles under us IS sending audio RTP, so the arrival has to be
    // counted before the profile gate rejects it. Counted after, the watchdog would see an empty
    // window, report the transport alarm, and bury the one reason the counters can name exactly.
    #[test]
    fn a_stream_on_an_unexpected_payload_type_reports_that_reason_not_a_stall() {
        let mut eng = allocated_engine();
        let mut peer_tx = peer_pipeline();
        let mut now = 1;
        let mut events = Vec::new();
        for _ in 0..80 {
            let mut packet = peer_tx.protect_audio(&[0x50, 1, 2, 3]);
            packet[1] = (packet[1] & 0x80) | 99;
            eng.handle_input(now, Input::RelayPacket(&packet));
            let (outputs, _) = drain(&mut eng);
            events.extend(outputs);
            now += 60;
            eng.handle_input(now, Input::Timeout);
            let (outputs, _) = drain(&mut eng);
            events.extend(outputs);
        }
        assert!(
            !events
                .iter()
                .any(|o| matches!(o, Output::Event(CallEvent::AudioReceptionStalled { .. }))),
            "packets are arriving, so this is not a stalled reception"
        );
        let reason = events
            .iter()
            .find_map(|o| match o {
                Output::Event(CallEvent::AudioSilent {
                    dominant_reason, ..
                }) => Some(*dominant_reason),
                _ => None,
            })
            .expect("a stream on the wrong payload type must report itself silent");
        assert_eq!(reason, AudioSilenceReason::UnexpectedPayloadType);
    }

    // The most dangerous discard in the file: wrong recv keys make a call totally deaf, and before
    // this counter existed there was no observation at all distinguishing it from silence.
    #[test]
    fn a_packet_that_fails_its_tag_is_counted() {
        let mut eng = allocated_engine();
        let mut peer_tx = peer_pipeline();
        let mut packet = peer_tx.protect_audio(&[0x50, 1, 2, 3]);
        let last = packet.len() - 1;
        packet[last] ^= 0xff;
        eng.handle_input(1, Input::RelayPacket(&packet));
        let _ = drain(&mut eng);
        let stats = eng.media_stats();
        assert_eq!(stats.srtp_unprotect_failed, 1);
        assert_eq!(stats.rtp_received, 0);
    }

    // A relay datagram the media plane does not speak is counted, so a relay that changes its
    // framing presents as "unclassified packets" instead of as a call with no media.
    #[test]
    fn an_unclassifiable_relay_datagram_is_counted() {
        let mut eng = allocated_engine();
        // Version bits 01: not STUN (which is `b0 & 0xc0 == 0`), and not RTP/RTCP, which require
        // version 2. Nothing the media plane speaks.
        eng.handle_input(1, Input::RelayPacket(&[0x40, 0x00, 0x00, 0x00, 0xaa]));
        let _ = drain(&mut eng);
        assert_eq!(eng.media_stats().relay_packet_unclassified, 1);
    }

    // The consumer-visible half of the #1105 fix: packets keep arriving, none of them becomes
    // sound, and the call says so instead of looking like a peer who is not speaking.
    #[test]
    fn a_call_that_receives_packets_and_produces_no_audio_reports_itself_silent() {
        let mut eng = allocated_engine();
        let mut peer_tx = peer_pipeline();
        // TOC 0x58 is what a WhatsApp Desktop peer outside the MLow rollout actually sends: an Opus
        // SILK wideband frame that the MLow decoder reads as a 120 ms packet and cannot decode.
        let body: Vec<u8> = core::iter::once(0x58u8).chain(0..80u8).collect();
        let mut now = 1;
        let mut events = Vec::new();
        for _ in 0..80 {
            let packet = peer_tx.protect_audio(&body);
            eng.handle_input(now, Input::RelayPacket(&packet));
            let (outputs, _) = drain(&mut eng);
            events.extend(outputs.into_iter().filter_map(|o| match o {
                Output::Event(ev) => Some(ev),
                _ => None,
            }));
            now += 60;
            eng.handle_input(now, Input::Timeout);
            let (outputs, _) = drain(&mut eng);
            events.extend(outputs.into_iter().filter_map(|o| match o {
                Output::Event(ev) => Some(ev),
                _ => None,
            }));
        }
        let silent = events
            .iter()
            .find_map(|ev| match ev {
                CallEvent::AudioSilent {
                    rtp_received,
                    frames_produced,
                    dominant_reason,
                    ..
                } => Some((*rtp_received, *frames_produced, *dominant_reason)),
                _ => None,
            })
            .expect("a call receiving packets and decoding none must report itself silent");
        assert!(silent.0 >= 20, "window must carry enough packets to judge");
        assert_eq!(silent.1, 0, "no audio was produced");
        // Four packets go down the MLOW path before the probe has enough evidence -- one to give the
        // timestamps a difference, then three agreeing -- and every one after that is a packet this
        // build has no decoder for.
        assert_eq!(
            eng.media_stats().audio_frames_without_decoder,
            76,
            "every packet after the switch is one this build could not decode"
        );
        // The probe recognises the grammar from the bytes and the engine says exactly why the call
        // is silent: not "the codec refused it", but "this build has no decoder for what the peer
        // negotiated", which is the one reason a consumer can act on.
        assert_eq!(
            silent.2,
            AudioSilenceReason::NoDecoderForNegotiatedCodec,
            "an engine with no Opus decoder must name that as the reason"
        );
        let switched = events
            .iter()
            .find_map(|ev| match ev {
                CallEvent::AudioCodecSwitched { to, source, .. } => Some((*to, *source)),
                _ => None,
            })
            .expect("the bytes contradict the negotiation and that must be surfaced");
        assert_eq!(switched, (AudioCodec::Opus, CodecDecisionSource::Content));
        let stats = eng.media_stats();
        assert_eq!(stats.rtp_received, 80);
        assert_eq!(stats.audio_frames_decoded, 0);
    }

    /// A decoder that turns any payload into a fixed run of samples, so a test can prove the
    /// rescue path without pulling libopus into `wacore`.
    struct StubForeignCodec {
        samples: usize,
    }

    impl ForeignAudioCodec for StubForeignCodec {
        fn decode(
            &mut self,
            payload: &[u8],
            out: &mut Vec<i16>,
        ) -> Result<(), super::super::audio::ForeignCodecError> {
            if payload.is_empty() {
                return Err(super::super::audio::ForeignCodecError::InvalidPayload);
            }
            out.extend(core::iter::repeat_n(1234, self.samples));
            Ok(())
        }

        fn conceal(&mut self, samples: usize, out: &mut Vec<i16>) {
            out.resize(out.len() + samples, 0);
        }

        fn encode(
            &mut self,
            pcm: &[i16],
            out: &mut Vec<u8>,
        ) -> Result<(), super::super::audio::ForeignCodecError> {
            out.extend(core::iter::repeat_n(0x58, pcm.len() / 16));
            Ok(())
        }
    }

    // The whole point of issue #1105, end to end: a peer sending standard Opus on the MLow payload
    // type is recognised from its own two statements, the codec is switched, and the call carries
    // audio instead of silence.
    #[test]
    fn a_peer_sending_opus_on_the_mlow_profile_is_rescued_and_the_call_carries_audio() {
        let mut eng = engine(true).with_foreign_audio_codec(Box::new(StubForeignCodec {
            samples: SAMPLES as usize,
        }));
        eng.start(0, 0);
        let _ = drain(&mut eng);
        let success = allocate_success(&eng);
        eng.handle_input(0, Input::RelayPacket(&success));
        let _ = drain(&mut eng);

        let mut peer_tx = peer_pipeline();
        let body: Vec<u8> = core::iter::once(0x58u8).chain(0..80u8).collect();
        for n in 0..10u64 {
            let packet = peer_tx.protect_audio(&body);
            eng.handle_input(1 + n * 60, Input::RelayPacket(&packet));
            let _ = drain(&mut eng);
        }
        assert_eq!(eng.active_audio_codec(), Some(AudioCodec::Opus));
        let stats = eng.media_stats();
        // Exact, not "greater than zero": a rescue that recovered one packet in ten would satisfy a
        // loose assertion and sound exactly as broken as no rescue at all.
        //
        // Four packets are spent before the switch -- one to give the timestamps a difference to
        // measure, then three agreeing ones -- and every packet after it decodes. Ten in, four
        // concealed, six rescued, and the two numbers have to add up to the packets received.
        assert_eq!(stats.rtp_received, 10);
        assert_eq!(
            (stats.audio_frames_concealed, stats.foreign_frames_decoded),
            (4, 6),
            "every packet after the switch must decode, got {stats:?}"
        );
        assert_eq!(stats.codec_switches, 1);
        assert_eq!(
            stats.audio_frames_without_decoder, 0,
            "a rescued call never reports a missing decoder"
        );
        // The rescued samples go into the SAME buffer the MLOW path feeds, under the same ceiling:
        // this test pushes ten packets with no playout tick in between, so the feed-side bound is
        // what keeps it finite.
        assert!(eng.jitter_len() > 0, "decoded samples reach playout");
        // The ceiling in force is the one the MLOW reading left behind: those first four packets
        // declared 120 ms (TOC 0x58 under that grammar), so the cushion grew to fit them, and it
        // only comes back down once the backlog fits underneath -- otherwise dropping to a shorter
        // packet would trim audio that was legally queued under the previous bound.
        let ceiling = playout_bounds(2 * SAMPLES as usize).1;
        assert!(
            eng.jitter_len() <= ceiling,
            "the rescued path is bounded by the ceiling in force ({ceiling}), got {}",
            eng.jitter_len()
        );
        assert!(
            stats.playout_trimmed_samples > 0,
            "and a trim under that ceiling is counted rather than silent"
        );
    }

    // Rescuing a call and then making it wait out its own failure is not a rescue. The probe only
    // asks for a switch after packets have stopped becoming audio, so what is queued at that moment
    // is the concealment those packets produced -- up to a quarter second of manufactured silence
    // sitting in front of the first correctly decoded frame.
    #[test]
    fn a_rescued_call_does_not_play_out_the_silence_that_preceded_the_rescue() {
        let mut eng = engine(true).with_foreign_audio_codec(Box::new(StubForeignCodec {
            samples: SAMPLES as usize,
        }));
        eng.start(0, 0);
        let _ = drain(&mut eng);
        let success = allocate_success(&eng);
        eng.handle_input(0, Input::RelayPacket(&success));
        let _ = drain(&mut eng);

        let mut peer_tx = peer_pipeline();
        let body: Vec<u8> = core::iter::once(0x58u8).chain(0..80u8).collect();
        // Exactly the four packets the switch costs, then two that decode.
        for n in 0..6u64 {
            let packet = peer_tx.protect_audio(&body);
            eng.handle_input(1 + n * 60, Input::RelayPacket(&packet));
            let _ = drain(&mut eng);
        }
        assert_eq!(eng.active_audio_codec(), Some(AudioCodec::Opus), "rescued");

        // The very first sample the consumer hears has to be rescued audio, not the concealment
        // that led to the rescue. The stub decodes every payload to a constant, so anything the
        // MLow path produced for those first four packets is distinguishable from it.
        let mut first = None;
        for tick in 1..=8u64 {
            eng.handle_input(400 + tick * PLAYOUT_MS, Input::Timeout);
            let (outputs, _) = drain(&mut eng);
            for output in outputs {
                if let Output::Playout(frame) = output
                    && first.is_none()
                {
                    first = frame.first().copied();
                }
            }
        }
        assert_eq!(
            first,
            Some(1234),
            "the first frame played after the rescue must be the rescued audio"
        );
    }

    // A call carrying real audio must never alarm; without this the watchdog is a false-positive
    // generator and the first thing a consumer does is ignore it.
    #[test]
    fn a_healthy_call_never_reports_itself_silent() {
        let mut eng = allocated_engine();
        let mut peer_tx = peer_pipeline();
        let mut peer_enc = MlowEncoder::new();
        let mut now = 1;
        for n in 0..80u32 {
            let tone: Vec<f32> = (0..SAMPLES as usize)
                .map(|i| 0.3 * ((i as f32 + (n * SAMPLES) as f32) * 0.05).sin())
                .collect();
            let frame = peer_enc.encode(&tone).expect("mlow encode");
            let packet = peer_tx.protect_audio(&frame);
            eng.handle_input(now, Input::RelayPacket(&packet));
            let _ = drain(&mut eng);
            now += 60;
            eng.handle_input(now, Input::Timeout);
            let (outputs, _) = drain(&mut eng);
            for output in outputs {
                assert!(
                    !matches!(
                        output,
                        Output::Event(CallEvent::AudioSilent { .. })
                            | Output::Event(CallEvent::AudioReceptionStalled { .. })
                    ),
                    "a call decoding audio must not alarm, got {output:?}"
                );
            }
        }
        assert!(eng.media_stats().audio_frames_decoded > 0);
    }

    // A muted peer sends SID/DTX, and the decoder handling one is the stream working exactly as
    // designed. Counting only DECODED frames as production made a mute look identical to a codec
    // that cannot decode anything: `AudioSilent` every two seconds, re-alarming for as long as the
    // peer stayed quiet, with no reason to name because nothing was actually wrong.
    #[test]
    fn a_muted_peer_sending_sid_is_not_a_silent_call() {
        let mut eng = allocated_engine();
        let mut peer_tx = peer_pipeline();
        let mut now = 1;
        // Twelve seconds of comfort noise: past the 2 s window, the 3 s stall bound, and one full
        // re-alarm cadence, so a false alarm has every chance to fire.
        for _ in 0..200u32 {
            // TOC 0x80: a SID, silenced through the comfort-noise path without opening the range
            // coder (see `MlowDecoder::decode_frame`).
            let packet = peer_tx.protect_audio(&[0x80, 0xAA, 0xBB, 0xCC]);
            eng.handle_input(now, Input::RelayPacket(&packet));
            let _ = drain(&mut eng);
            now += 60;
            eng.handle_input(now, Input::Timeout);
            let (outputs, _) = drain(&mut eng);
            for output in outputs {
                assert!(
                    !matches!(
                        output,
                        Output::Event(CallEvent::AudioSilent { .. })
                            | Output::Event(CallEvent::AudioReceptionStalled { .. })
                    ),
                    "a peer that is telling us it is silent is not a call that cannot hear, got {output:?}"
                );
            }
        }
        let stats = eng.media_stats();
        assert!(stats.mlow_inactive_or_sid > 0, "the SIDs were recognised");
        assert_eq!(stats.audio_frames_decoded, 0, "and none of them was speech");
    }

    // An outgoing call allocates its relay when the SERVER acks the offer, which is long before the
    // callee touches the phone. Arming the watchdog there reported a stalled reception three
    // seconds into every normal ring -- an alarm on the single most ordinary thing a call does.
    #[test]
    fn a_ringing_outgoing_call_is_not_a_stalled_reception() {
        let mut cfg = config(true);
        cfg.direction = CallDirection::Outgoing;
        let mut eng = CallEngine::new(cfg, Box::new(SequentialTxIds::new())).unwrap();
        eng.start(0, 0);
        let _ = drain(&mut eng);
        let success = allocate_success(&eng);
        eng.handle_input(0, Input::RelayPacket(&success));
        let _ = drain(&mut eng);

        // Twenty seconds of ringing: nobody has answered, so nobody owes us audio.
        for tick in 1..=40u64 {
            eng.handle_input(tick * 500, Input::Timeout);
            let (outputs, _) = drain(&mut eng);
            assert!(
                !outputs
                    .iter()
                    .any(|o| matches!(o, Output::Event(CallEvent::AudioReceptionStalled { .. }))),
                "a ringing call is not a stalled one, got {outputs:?}"
            );
        }

        // The callee answers. From here its silence means something, and the alarm must come.
        eng.peer_answered(20_000);
        let mut stalls = 0;
        for tick in 41..=60u64 {
            eng.handle_input(tick * 500, Input::Timeout);
            let (outputs, _) = drain(&mut eng);
            stalls += outputs
                .iter()
                .filter(|o| matches!(o, Output::Event(CallEvent::AudioReceptionStalled { .. })))
                .count();
        }
        assert_eq!(
            stalls, 1,
            "once answered, a call that carries nothing is exactly what this alarm is for"
        );
    }

    // The accept can land BEFORE the relay answers our allocate -- a buffered accept consumed as
    // soon as a delayed attachment starts makes that ordinary. Inbound media cannot flow until the
    // allocation completes, so arming on the accept alone reports the allocation interval itself as
    // lost reception, and the alarm arrives before `RelayAllocated` does.
    #[test]
    fn an_accept_before_allocation_does_not_start_the_stall_clock() {
        let mut eng = CallEngine::new(config(true), Box::new(SequentialTxIds::new())).unwrap();
        eng.start(0, 0);
        let _ = drain(&mut eng);
        assert!(!eng.is_allocated(), "the relay has not answered yet");

        // The peer picks up first, then the relay takes its time.
        eng.peer_answered(0);
        for tick in 1..=12u64 {
            eng.handle_input(tick * 500, Input::Timeout);
            let (outputs, _) = drain(&mut eng);
            assert!(
                !outputs
                    .iter()
                    .any(|o| matches!(o, Output::Event(CallEvent::AudioReceptionStalled { .. }))),
                "nothing can arrive before the relay allocates, so nothing is late, got {outputs:?}"
            );
        }

        // Allocation completes: only now does the peer's silence mean anything.
        let success = allocate_success(&eng);
        eng.handle_input(6_000, Input::RelayPacket(&success));
        let _ = drain(&mut eng);
        let mut stalls = 0;
        for tick in 13..=32u64 {
            eng.handle_input(6_000 + tick * 500, Input::Timeout);
            let (outputs, _) = drain(&mut eng);
            stalls += outputs
                .iter()
                .filter(|o| matches!(o, Output::Event(CallEvent::AudioReceptionStalled { .. })))
                .count();
        }
        assert_eq!(
            stalls, 1,
            "and once both have happened the alarm still works, it is only deferred"
        );
    }

    // Reception that never starts is a transport problem, and it gets its own event: conflating it
    // with a codec problem is how #1105 was mis-triaged.
    #[test]
    fn a_call_that_never_receives_rtp_reports_a_stall_exactly_once() {
        let mut eng = allocated_engine();
        let mut stalls = 0;
        for tick in 1..=40u64 {
            eng.handle_input(tick * 500, Input::Timeout);
            let (outputs, _) = drain(&mut eng);
            stalls += outputs
                .iter()
                .filter(|o| matches!(o, Output::Event(CallEvent::AudioReceptionStalled { .. })))
                .count();
        }
        assert_eq!(stalls, 1, "a stalled call does not become more stalled");
    }

    // The whole reason the switch is a field swap and not a reconfiguration: the two formats agree
    // on every timing field, so nothing on the wire moves. If that ever stops being true the swap
    // becomes a renegotiation the peer cannot learn about, and this test is what says so.
    #[test]
    fn the_swappable_pair_agrees_on_every_rtp_timing_field() {
        let mlow = AudioFormat::MLOW_16KHZ_60MS;
        let opus = AudioFormat::OPUS_16KHZ_60MS;
        assert_eq!(mlow.rtp_payload_type, opus.rtp_payload_type);
        assert_eq!(mlow.rtp_clock_rate, opus.rtp_clock_rate);
        assert_eq!(mlow.rtp_timestamp_step, opus.rtp_timestamp_step);
        assert_eq!(mlow.samples_per_frame, opus.samples_per_frame);
        assert_eq!(mlow.sample_rate, opus.sample_rate);
        assert_eq!(mlow.channels, opus.channels);
        assert_eq!(mlow.sibling_for(AudioCodec::Opus), Some(opus));
        assert_eq!(opus.sibling_for(AudioCodec::Mlow), Some(mlow));
        // A profile on a different clock is NOT swappable, and must refuse rather than silently
        // change the RTP timing under a live stream.
        assert_eq!(
            AudioFormat::OPUS_RFC7587_16KHZ_60MS.sibling_for(AudioCodec::Mlow),
            None
        );
    }

    #[test]
    fn switching_to_the_sibling_codec_changes_the_grammar_and_reports_it() {
        let mut eng = allocated_engine();
        assert_eq!(eng.active_audio_codec(), Some(AudioCodec::Mlow));
        eng.switch_audio_codec(AudioCodec::Opus, CodecDecisionSource::Negotiated)
            .expect("the pair is swappable");
        assert_eq!(eng.active_audio_codec(), Some(AudioCodec::Opus));
        let (outputs, _) = drain(&mut eng);
        let switched = outputs
            .iter()
            .find_map(|o| match o {
                Output::Event(CallEvent::AudioCodecSwitched {
                    from, to, source, ..
                }) => Some((*from, *to, *source)),
                _ => None,
            })
            .expect("a codec switch is consumer-visible");
        assert_eq!(
            switched,
            (
                AudioCodec::Mlow,
                AudioCodec::Opus,
                CodecDecisionSource::Negotiated
            )
        );
        assert_eq!(eng.media_stats().codec_switches, 1);
    }

    #[test]
    fn switching_to_the_codec_already_in_use_is_a_silent_no_op() {
        let mut eng = allocated_engine();
        eng.switch_audio_codec(AudioCodec::Mlow, CodecDecisionSource::Negotiated)
            .expect("idempotent");
        let (outputs, _) = drain(&mut eng);
        assert!(
            !outputs
                .iter()
                .any(|o| matches!(o, Output::Event(CallEvent::AudioCodecSwitched { .. }))),
            "an idempotent switch must not emit an event"
        );
        assert_eq!(eng.media_stats().codec_switches, 0);
    }

    // Evidence that keeps reversing is evidence that is wrong. Thrashing the decoder for a whole
    // call is worse than picking one grammar and letting the health watchdog say the call is sick.
    #[test]
    fn a_codec_that_keeps_flapping_latches() {
        let mut eng = allocated_engine();
        let mut codec = AudioCodec::Opus;
        for _ in 0..CODEC_FLAP_LIMIT {
            eng.switch_audio_codec(codec, CodecDecisionSource::Content)
                .expect("within the flap budget");
            codec = match codec {
                AudioCodec::Opus => AudioCodec::Mlow,
                _ => AudioCodec::Opus,
            };
        }
        assert_eq!(
            eng.switch_audio_codec(codec, CodecDecisionSource::Content),
            Err(CodecSwitchError::Latched)
        );
        assert_eq!(eng.media_stats().codec_switches, CODEC_FLAP_LIMIT);
    }

    // The engine must refuse a swap that would change the RTP timing under a live stream, rather
    // than accept it and leave the peer decoding against a clock that silently moved.
    #[test]
    fn a_switch_that_would_change_the_rtp_timing_is_refused() {
        let mut cfg = config(true);
        cfg.audio = AudioConfig::encoded(AudioFormat::OPUS_RFC7587_16KHZ_60MS);
        let mut eng = CallEngine::new(cfg, Box::new(SequentialTxIds::new())).unwrap();
        eng.start(0, 0);
        let _ = drain(&mut eng);
        assert_eq!(
            eng.switch_audio_codec(AudioCodec::Mlow, CodecDecisionSource::Negotiated),
            Err(CodecSwitchError::NotASiblingFormat)
        );
    }

    // After the switch the inbound classifier has to follow the new grammar, otherwise the call
    // announces a codec change and keeps decoding the old way.
    #[test]
    fn after_switching_to_opus_inbound_packets_are_classified_as_opus() {
        let mut cfg = config(true);
        cfg.audio = AudioConfig::encoded(AudioFormat::MLOW_16KHZ_60MS);
        let mut eng = CallEngine::new(cfg, Box::new(SequentialTxIds::new())).unwrap();
        eng.start(0, 0);
        let _ = drain(&mut eng);
        eng.switch_audio_codec(AudioCodec::Opus, CodecDecisionSource::Negotiated)
            .expect("swappable");
        let _ = drain(&mut eng);
        let mut peer_tx = peer_pipeline();
        // TOC 0x58: Opus SILK wideband, 60 ms. Under the MLow grammar the same byte reads as a
        // 120 ms packet, which is exactly the collision behind issue #1105.
        let body: Vec<u8> = core::iter::once(0x58u8).chain(0..40u8).collect();
        let packet = peer_tx.protect_audio(&body);
        eng.handle_input(1, Input::RelayPacket(&packet));
        let (outputs, _) = drain(&mut eng);
        let codec = outputs
            .iter()
            .find_map(|o| match o {
                Output::EncodedAudio(frame) => Some(frame.codec),
                _ => None,
            })
            .expect("the payload must reach the encoded sink");
        assert_eq!(codec, AudioCodec::Opus);
    }

    // An `EncodedAudioSource` emits one codec for the life of the call: the application built it,
    // it carries no per-frame codec, and the engine cannot re-point it the way it re-points its own
    // encoder. When the peer turns out to speak the other grammar, the switch still has to happen
    // for the RECEIVE side -- but sending the source's bytes under the new profile would put MLow
    // on the wire labelled Opus, and that profile accepts any nonempty payload, so nothing would
    // catch it and the peer would hear noise.
    #[test]
    fn a_fixed_encoded_source_stops_sending_rather_than_mislabelling_its_bytes() {
        let mut cfg = config(true);
        cfg.audio = AudioConfig::encoded(AudioFormat::MLOW_16KHZ_60MS);
        let mut eng = CallEngine::new(cfg, Box::new(SequentialTxIds::new())).unwrap();
        eng.start(0, 0);
        let _ = drain(&mut eng);

        // While the call is on the source's own grammar, its bytes go out.
        let mlow_frame: Vec<u8> = core::iter::once(0x08u8).chain(0..40u8).collect();
        eng.handle_input(1, Input::EncodedAudio(&mlow_frame));
        let (outputs, _) = drain(&mut eng);
        assert!(
            outputs.iter().any(|o| matches!(o, Output::Transmit(_))),
            "the source's own codec is what this call sends"
        );

        // The peer's capability says Opus. The receive side follows it; the source cannot.
        eng.switch_audio_codec(AudioCodec::Opus, CodecDecisionSource::Negotiated)
            .expect("the inbound side must still switch");
        let (outputs, _) = drain(&mut eng);
        let announced = outputs.iter().any(|o| {
            matches!(
                o,
                Output::Event(CallEvent::AudioCodecSourceIsFixed {
                    sending: AudioCodec::Mlow,
                    peer_expects: AudioCodec::Opus,
                    ..
                })
            )
        });
        assert!(
            announced,
            "the application has to learn that only it can fix this call, got {outputs:?}"
        );

        eng.handle_input(2, Input::EncodedAudio(&mlow_frame));
        let (outputs, _) = drain(&mut eng);
        assert!(
            !outputs.iter().any(|o| matches!(o, Output::Transmit(_))),
            "bytes the peer cannot decode must not go out labelled as bytes it can"
        );
        assert_eq!(
            eng.media_stats().outbound_frames_without_encoder,
            1,
            "and the frames that stay behind are counted"
        );
    }

    // The payload-type gate has to move with the switch. MLow carries a redundancy type (PT 121)
    // that standard Opus does not have, so a gate reading the NEGOTIATED format kept admitting it
    // after a switch to Opus -- and the classifier below, reading the active one, called the RED
    // wrapper an Opus frame and handed it to the Opus decoder.
    #[test]
    fn a_redundancy_payload_type_is_refused_after_switching_away_from_mlow() {
        let mut cfg = config(true);
        cfg.audio = AudioConfig::encoded(AudioFormat::MLOW_16KHZ_60MS);
        let mut eng = CallEngine::new(cfg, Box::new(SequentialTxIds::new())).unwrap();
        eng.start(0, 0);
        let _ = drain(&mut eng);
        let mut peer_tx = peer_pipeline();
        let body: Vec<u8> = core::iter::once(0x58u8).chain(0..40u8).collect();

        // While the call is MLow, PT 121 is part of its profile and is delivered.
        let mut packet = peer_tx.protect_audio(&body);
        packet[1] = (packet[1] & 0x80) | RTP_PAYLOAD_TYPE_MLOW_RED;
        eng.handle_input(1, Input::RelayPacket(&packet));
        let _ = drain(&mut eng);
        assert_eq!(
            eng.media_stats().rtp_payload_type_unexpected,
            0,
            "MLow's own redundancy type belongs to an MLow call"
        );

        eng.switch_audio_codec(AudioCodec::Opus, CodecDecisionSource::Negotiated)
            .expect("swappable");
        let _ = drain(&mut eng);

        // Standard Opus has no such type: a delayed or reordered one must be refused, not decoded.
        let mut packet = peer_tx.protect_audio(&body);
        packet[1] = (packet[1] & 0x80) | RTP_PAYLOAD_TYPE_MLOW_RED;
        eng.handle_input(2, Input::RelayPacket(&packet));
        let (outputs, _) = drain(&mut eng);
        assert_eq!(
            eng.media_stats().rtp_payload_type_unexpected,
            1,
            "the redundancy type is outside the active profile and is counted as such"
        );
        assert!(
            !outputs.iter().any(|o| matches!(o, Output::EncodedAudio(_))),
            "and a RED wrapper must never reach a consumer labelled as an Opus frame"
        );
    }

    // The peer is the one that must be able to read the frame, so a format the engine has not
    // switched to yet is the wrong label for the packet that triggers the switch.
    #[test]
    fn the_frame_that_triggers_the_content_switch_is_labelled_with_its_own_format() {
        let mut cfg = config(true);
        cfg.audio = AudioConfig::encoded(AudioFormat::MLOW_16KHZ_60MS);
        let mut eng = CallEngine::new(cfg, Box::new(SequentialTxIds::new())).unwrap();
        eng.start(0, 0);
        let _ = drain(&mut eng);
        let success = allocate_success(&eng);
        eng.handle_input(0, Input::RelayPacket(&success));
        let _ = drain(&mut eng);

        let body: Vec<u8> = core::iter::once(0x58u8).chain(0..80u8).collect();
        let mut peer_tx = peer_pipeline();
        let mut frames = Vec::new();
        for n in 0..6u64 {
            let packet = peer_tx.protect_audio(&body);
            eng.handle_input(1 + n * 60, Input::RelayPacket(&packet));
            let (outputs, _) = drain(&mut eng);
            for output in outputs {
                if let Output::EncodedAudio(frame) = output {
                    frames.push((frame.codec, frame.format));
                }
            }
        }

        let transition = frames
            .iter()
            .position(|(codec, _)| *codec == AudioCodec::Opus)
            .expect("the probe fires");
        assert_eq!(
            frames[transition].1,
            AudioFormat::OPUS_16KHZ_60MS,
            "the frame carrying the verdict is labelled by it, not by the format still installed"
        );
        assert!(
            frames[transition..]
                .iter()
                .all(|(codec, format)| *codec == AudioCodec::Opus
                    && *format == AudioFormat::OPUS_16KHZ_60MS),
            "and every frame after it agrees"
        );
    }

    // The probe needs three packets before the call-wide switch, but the grammar of any ONE native
    // packet is already settled by the discriminator. Labelling the first two with MLOW's container
    // tells the sink to undo an escape that is not there, and it loses them.
    #[test]
    fn an_encoded_call_labels_native_celt_natively_before_the_verdict() {
        let mut cfg = config(true);
        cfg.audio = AudioConfig::encoded(AudioFormat::MLOW_16KHZ_60MS);
        let mut eng = CallEngine::new(cfg, Box::new(SequentialTxIds::new())).unwrap();
        eng.start(0, 0);
        let _ = drain(&mut eng);
        let success = allocate_success(&eng);
        eng.handle_input(0, Input::RelayPacket(&success));
        let _ = drain(&mut eng);

        let mut native: Vec<u8> = vec![27 << 3 | 3, 3];
        native.extend(core::iter::repeat_n(0x11u8, 60));
        let mut peer_tx = peer_pipeline();
        let mut delivered = Vec::new();
        for n in 0..3u64 {
            let packet = peer_tx.protect_audio(&native);
            eng.handle_input(1 + n * 60, Input::RelayPacket(&packet));
            let (outputs, _) = drain(&mut eng);
            for output in outputs {
                if let Output::EncodedAudio(frame) = output {
                    delivered.push((frame.codec, frame.format));
                }
            }
        }

        assert_eq!(delivered.len(), 3);
        assert!(
            delivered
                .iter()
                .all(|(codec, format)| *codec == AudioCodec::Opus
                    && *format == AudioFormat::OPUS_16KHZ_60MS),
            "every one, including the two before the verdict, got {delivered:?}"
        );
    }

    // The purge exists for a queue full of concealment, which is what the probe used to imply. It
    // does not imply that any more: native CELT decodes correctly while the probe corroborates it,
    // so the queue holds the corroboration window of real speech and purging loses all of it.
    #[test]
    fn the_corroboration_window_of_decoded_celt_survives_the_switch() {
        let mut eng = CallEngine::new(config(true), Box::new(SequentialTxIds::new()))
            .unwrap()
            .with_foreign_audio_codec(Box::new(StubForeignCodec { samples: 960 }));
        eng.start(0, 0);
        let _ = drain(&mut eng);
        let success = allocate_success(&eng);
        eng.handle_input(0, Input::RelayPacket(&success));
        let _ = drain(&mut eng);

        let mut native: Vec<u8> = vec![27 << 3 | 3, 3];
        native.extend(core::iter::repeat_n(0x11u8, 60));
        let mut peer_tx = peer_pipeline();
        for n in 0..6u64 {
            let packet = peer_tx.protect_audio(&native);
            eng.handle_input(1 + n * 60, Input::RelayPacket(&packet));
            let _ = drain(&mut eng);
        }
        assert_eq!(eng.active_audio_codec(), Some(AudioCodec::Opus));

        let mut heard = Vec::new();
        for tick in 0..24u64 {
            eng.handle_input(400 + tick * 20, Input::Timeout);
            let (outputs, _) = drain(&mut eng);
            for output in outputs {
                if let Output::Playout(frame) = output {
                    heard.extend(frame);
                }
            }
        }
        // The number is the assertion because the loss is a prefix of the buffer, not a failure:
        // purging unconditionally gives 1920 here, keeping what decoded gives 2400. Two 60ms packets
        // of real speech in this fixture -- less than an unconditional purge costs a call whose
        // buffer is fuller, and enough to fail if the condition is dropped.
        let decoded = heard.iter().filter(|s| **s == 1234).count();
        assert_eq!(
            decoded, 2400,
            "the packets decoded before the verdict have to survive it"
        );
    }

    // Inbound native CELT decodes perfectly well without the call-wide format ever moving, and then
    // `on_mic` goes on encoding MLOW at a peer that speaks native Opus: inbound fine, outbound
    // silent, and no counter describing it because nothing failed. The probe has to be asked before
    // that branch returns, and its verdict applied there too.
    #[test]
    fn native_celt_on_the_pcm_path_moves_the_call_so_the_outbound_half_follows() {
        let mut eng = CallEngine::new(config(true), Box::new(SequentialTxIds::new()))
            .unwrap()
            .with_foreign_audio_codec(Box::new(StubForeignCodec { samples: 960 }));
        eng.start(0, 0);
        let _ = drain(&mut eng);
        let success = allocate_success(&eng);
        eng.handle_input(0, Input::RelayPacket(&success));
        let _ = drain(&mut eng);
        assert_eq!(eng.active_audio_codec(), Some(AudioCodec::Mlow));

        // Config 27, code 3, three 20ms CELT frames: native Opus at the negotiated 60ms cadence,
        // with a TOC in the escape's bit class.
        let mut native: Vec<u8> = vec![27 << 3 | 3, 3];
        native.extend(core::iter::repeat_n(0x11u8, 60));
        let mut peer_tx = peer_pipeline();
        for n in 0..6u64 {
            let packet = peer_tx.protect_audio(&native);
            eng.handle_input(1 + n * 60, Input::RelayPacket(&packet));
            let _ = drain(&mut eng);
        }

        assert_eq!(
            eng.active_audio_codec(),
            Some(AudioCodec::Opus),
            "the call has to move, or everything it sends stays MLOW"
        );
        assert_eq!(
            eng.active_audio_format(),
            Some(AudioFormat::OPUS_16KHZ_60MS),
            "and onto the native format, since these bytes are not the escape"
        );
    }

    // An encoded call never decodes anything in the core, so "nothing became audio" is true of
    // every packet and cannot be what triggers the probe. Without asking on this path, a peer whose
    // capability was absent or lost the race with its media had its native Opus handed to the sink
    // labelled `Mlow`, with no switch and no event -- and the fixed source kept sending MLow at a
    // peer that cannot decode it, so both directions were broken with nothing to notice.
    #[test]
    fn an_encoded_call_probes_the_content_and_relabels_what_it_delivers() {
        let mut cfg = config(true);
        cfg.audio = AudioConfig::encoded(AudioFormat::MLOW_16KHZ_60MS);
        let mut eng = CallEngine::new(cfg, Box::new(SequentialTxIds::new())).unwrap();
        eng.start(0, 0);
        let _ = drain(&mut eng);
        let success = allocate_success(&eng);
        eng.handle_input(0, Input::RelayPacket(&success));
        let _ = drain(&mut eng);

        // TOC 0x58: 60 ms of SILK wideband under the Opus grammar, 120 ms under MLow's -- the
        // collision behind #1105. The peer paces at the negotiated 960-sample step, so its two
        // statements agree and only the Opus reading can explain that.
        let body: Vec<u8> = core::iter::once(0x58u8).chain(0..80u8).collect();
        let mut peer_tx = peer_pipeline();
        let mut delivered = Vec::new();
        for n in 0..6u64 {
            let packet = peer_tx.protect_audio(&body);
            eng.handle_input(1 + n * 60, Input::RelayPacket(&packet));
            let (outputs, _) = drain(&mut eng);
            for output in outputs {
                if let Output::EncodedAudio(frame) = output {
                    delivered.push(frame.codec);
                }
            }
        }

        assert_eq!(
            eng.active_audio_codec(),
            Some(AudioCodec::Opus),
            "the encoded path has to reach the probe too"
        );
        assert_eq!(delivered.len(), 6, "every packet still reaches the sink");
        assert_eq!(
            delivered.last(),
            Some(&AudioCodec::Opus),
            "and after the switch it is labelled as what it is"
        );
        assert_eq!(eng.media_stats().codec_switches, 1);
    }

    // Encoded routing follows negotiation, not an ambiguous TOC-byte heuristic.
    #[test]
    fn negotiated_opus_payload_routes_to_encoded_output() {
        let mut cfg = config(true);
        cfg.audio = AudioConfig::encoded(AudioFormat::OPUS_16KHZ_60MS);
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
        assert!(peer_tx.set_audio_payload_type(AudioFormat::OPUS_16KHZ_60MS.rtp_payload_type));
        let encoded = [0x08u8, 1, 2, 3, 4, 5];
        let packet = peer_tx.protect_audio(&encoded);
        eng.handle_input(1, Input::RelayPacket(&packet));
        let (outs, _) = drain(&mut eng);
        let frames: Vec<_> = outs
            .iter()
            .filter_map(|output| match output {
                Output::EncodedAudio(frame) => Some(frame),
                _ => None,
            })
            .collect();
        assert_eq!(frames.len(), 1);
        assert_eq!(frames[0].data.as_ref(), encoded);
        assert_eq!(frames[0].format, AudioFormat::OPUS_16KHZ_60MS);
        assert_eq!(
            eng.jitter_len(),
            0,
            "encoded audio must not enter the PCM playout buffer"
        );
    }

    #[test]
    fn negotiated_mlow_surfaces_its_embedded_opus_escape() {
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
        let embedded_opus = [0xF8u8, 0xFF, 0xFE];
        let packet = peer_tx.protect_audio(&embedded_opus);
        eng.handle_input(1, Input::RelayPacket(&packet));
        let (outputs, _) = drain(&mut eng);

        assert!(outputs.iter().any(|output| matches!(
            output,
            Output::Event(CallEvent::ForeignAudio(payload)) if payload.as_ref() == embedded_opus
        )));
        assert_eq!(eng.jitter_len(), 0);
    }

    /// Hands out a fresh [`StubForeignCodec`] per participant, standing in for the libopus factory.
    struct StubCodecFactory {
        samples: usize,
    }

    impl ForeignAudioCodecFactory for StubCodecFactory {
        fn create(&self) -> Option<Box<dyn ForeignAudioCodec>> {
            Some(Box::new(StubForeignCodec {
                samples: self.samples,
            }))
        }
    }

    // A group participant outside the MLOW rollout speaks the escape, exactly as a 1:1 peer does.
    // The direct path decodes that with the installed adapter; the group path announced it as
    // undecodable and mixed nothing, so the speaker stayed silent for that participant while
    // `audio_frames_without_decoder` named a cause that was not true -- a decoder WAS installed.
    #[test]
    fn a_group_participant_on_opus_is_decoded_by_the_installed_factory() {
        let (mut eng, epoch) = group_engine(false);
        eng = eng.with_foreign_audio_codec_factory(Box::new(StubCodecFactory { samples: 960 }));
        eng.start(0, 1_700_000_000_000);
        let _ = drain(&mut eng);
        let allocate = allocate_success(&eng);
        eng.handle_input(1, Input::RelayPacket(&allocate));
        let _ = drain(&mut eng);

        let mut peer = group_peer_audio(&epoch);
        let mut heard = Vec::new();
        for n in 0..6u64 {
            let packet = peer.protect_audio(&[0xF8u8, 0xFF, 0xFE]);
            eng.handle_input(100 + n * 60, Input::RelayPacket(&packet));
            for tick in 0..3u64 {
                eng.handle_input(100 + n * 60 + tick * 20, Input::Timeout);
                let (outputs, _) = drain(&mut eng);
                for output in outputs {
                    match output {
                        Output::Playout(frame) => heard.extend(frame),
                        Output::Event(CallEvent::ForeignGroupAudio(_)) => {
                            panic!("a decoder was installed, so nothing should be handed back")
                        }
                        _ => {}
                    }
                }
            }
        }

        assert!(eng.media_stats().foreign_frames_decoded > 0, "it decodes");
        assert_eq!(
            eng.media_stats().audio_frames_without_decoder,
            0,
            "and does not claim a missing decoder while holding one"
        );
        assert!(
            heard.contains(&1234),
            "what it decoded has to reach the mixer and the speaker"
        );
    }

    // NATIVE Opus from a group participant carries no escape marker, so classification alone calls
    // it MLOW forever: it went to the MLow decoder, decoded to nothing, and that participant stayed
    // silent with an Opus decoder installed and idle. The direct path has the content probe for
    // exactly this; the group needs it per participant, because the call negotiates one format and
    // any one member may be the one outside the rollout.
    #[test]
    fn a_group_participant_sending_native_opus_is_rescued_by_the_probe() {
        let (mut eng, epoch) = group_engine(false);
        eng = eng.with_foreign_audio_codec_factory(Box::new(StubCodecFactory { samples: 960 }));
        eng.start(0, 1_700_000_000_000);
        let _ = drain(&mut eng);
        let allocate = allocate_success(&eng);
        eng.handle_input(1, Input::RelayPacket(&allocate));
        let _ = drain(&mut eng);

        // TOC 0x58 with no MLOW escape marker: 60 ms of SILK wideband read as Opus, paced at the
        // negotiated 960-sample step so the peer's two statements about the packet agree.
        let body: Vec<u8> = core::iter::once(0x58u8).chain(0..80u8).collect();
        let mut peer = group_peer_audio(&epoch);
        let mut heard = Vec::new();
        for n in 0..8u64 {
            let packet = peer.protect_audio(&body);
            eng.handle_input(100 + n * 60, Input::RelayPacket(&packet));
            for tick in 0..3u64 {
                eng.handle_input(100 + n * 60 + tick * 20, Input::Timeout);
                let (outputs, _) = drain(&mut eng);
                for output in outputs {
                    if let Output::Playout(frame) = output {
                        heard.extend(frame);
                    }
                }
            }
        }

        assert!(
            eng.media_stats().foreign_frames_decoded > 0,
            "the probe has to reach the same verdict it reaches on the direct path"
        );
        assert!(
            heard.contains(&1234),
            "and the rescued participant has to become audible"
        );
    }

    // The failed MLow decodes that convinced the probe also queued their concealment, and up to the
    // mixer's capacity of it would play out in front of the rescued audio. Rescuing a participant
    // and then making them wait out their own failure is not a rescue.
    #[test]
    fn a_rescued_group_participant_does_not_play_out_its_own_failure() {
        let (mut eng, epoch) = group_engine(false);
        eng = eng.with_foreign_audio_codec_factory(Box::new(StubCodecFactory { samples: 960 }));
        eng.start(0, 1_700_000_000_000);
        let _ = drain(&mut eng);
        let allocate = allocate_success(&eng);
        eng.handle_input(1, Input::RelayPacket(&allocate));
        let _ = drain(&mut eng);

        let body: Vec<u8> = core::iter::once(0x58u8).chain(0..80u8).collect();
        let mut peer = group_peer_audio(&epoch);
        let mut heard = Vec::new();
        for n in 0..8u64 {
            let packet = peer.protect_audio(&body);
            eng.handle_input(100 + n * 60, Input::RelayPacket(&packet));
            for tick in 0..3u64 {
                eng.handle_input(100 + n * 60 + tick * 20, Input::Timeout);
                let (outputs, _) = drain(&mut eng);
                for output in outputs {
                    if let Output::Playout(frame) = output {
                        heard.extend(frame);
                    }
                }
            }
        }

        let first_audible = heard
            .iter()
            .position(|sample| *sample != 0)
            .expect("the rescue produces audio");
        assert_eq!(
            heard[first_audible], 1234,
            "what is finally heard is decoded audio"
        );
        // The number is the point, since concealment and priming are both silence and only their
        // LENGTH tells them apart. Without the purge this is 5760 -- one 60ms packet of concealment
        // from the failed decodes, queued in front of the rescue and played before it.
        assert_eq!(
            first_audible, 4800,
            "the failed decodes' concealment must not be queued ahead of the rescued audio"
        );
    }

    // The encoded group path decodes nothing, so the PCM path's trigger ("the MLow decoder produced
    // nothing") is unreachable there and the branch returns before it anyway. Without its own ask,
    // the set that would correct the label is never populated for an encoded call, and the sink is
    // told MLOW about native Opus for the rest of the call.
    #[test]
    fn an_encoded_group_call_probes_each_participant_and_relabels_what_it_delivers() {
        let mut cfg = config(true);
        cfg.audio = AudioConfig::encoded(AudioFormat::MLOW_16KHZ_60MS);
        let mut eng = CallEngine::new(cfg, Box::new(SequentialTxIds::new())).expect("engine");
        let update = group_update("audio");
        eng.configure_group(GroupEngineConfig {
            call_creator: update.call_creator.clone(),
            self_jid: SELF_LID.parse().expect("self JID"),
            initial_update: update,
            direct_peer: None,
        })
        .expect("configure group");
        let epoch = [0x42; 32];
        assert_eq!(
            eng.apply_group_raw_epoch(7, &epoch).expect("install epoch"),
            GroupEpochApply::Installed
        );
        eng.start(0, 1_700_000_000_000);
        let _ = drain(&mut eng);
        let allocate = allocate_success(&eng);
        eng.handle_input(1, Input::RelayPacket(&allocate));
        let _ = drain(&mut eng);

        let body: Vec<u8> = core::iter::once(0x58u8).chain(0..80u8).collect();
        let mut peer = group_peer_audio(&epoch);
        let mut delivered = Vec::new();
        for n in 0..6u64 {
            let packet = peer.protect_audio(&body);
            eng.handle_input(100 + n * 60, Input::RelayPacket(&packet));
            let (outputs, _) = drain(&mut eng);
            for output in outputs {
                if let Output::EncodedAudio(frame) = output {
                    delivered.push((frame.codec, frame.format));
                }
            }
        }

        assert_eq!(delivered.len(), 6, "every packet still reaches the sink");
        let (codec, format) = *delivered.last().expect("frames");
        assert_eq!(codec, AudioCodec::Opus, "the probe has to run here too");
        assert_eq!(
            format,
            AudioFormat::OPUS_16KHZ_60MS,
            "and native Opus must not be described by the container it is not in"
        );
    }

    // A native-Opus participant whose first packets happen to be SILK is classified MLOW until its
    // first CELT packet settles the question. Those packets leave a decoder full of another codec's
    // predictor state and their concealment queued in the mixer, so without the same cleanup the
    // probe's verdict does, the CELT audio plays out behind that manufactured silence.
    #[test]
    fn a_group_participant_latched_late_does_not_wait_out_its_own_concealment() {
        let (mut eng, epoch) = group_engine(false);
        eng = eng.with_foreign_audio_codec_factory(Box::new(StubCodecFactory { samples: 960 }));
        eng.start(0, 1_700_000_000_000);
        let _ = drain(&mut eng);
        let allocate = allocate_success(&eng);
        eng.handle_input(1, Input::RelayPacket(&allocate));
        let _ = drain(&mut eng);

        let mut peer = group_peer_audio(&epoch);
        // Two SILK packets first: no marker, so classified MLOW and concealed into the mixer.
        let silk: Vec<u8> = core::iter::once(0x58u8).chain(0..80u8).collect();
        for n in 0..2u64 {
            let packet = peer.protect_audio(&silk);
            eng.handle_input(100 + n * 60, Input::RelayPacket(&packet));
            let _ = drain(&mut eng);
        }
        // Then CELT, which settles the participant's grammar outright.
        let mut celt: Vec<u8> = vec![27 << 3 | 3, 3];
        celt.extend(core::iter::repeat_n(0x11u8, 60));
        let mut heard = Vec::new();
        for n in 0..6u64 {
            let packet = peer.protect_audio(&celt);
            eng.handle_input(220 + n * 60, Input::RelayPacket(&packet));
            for tick in 0..3u64 {
                eng.handle_input(220 + n * 60 + tick * 20, Input::Timeout);
                let (outputs, _) = drain(&mut eng);
                for output in outputs {
                    if let Output::Playout(frame) = output {
                        heard.extend(frame);
                    }
                }
            }
        }

        let first_audible = heard
            .iter()
            .position(|sample| *sample != 0)
            .expect("the latched participant becomes audible");
        assert_eq!(
            heard[first_audible], 1234,
            "what is heard is decoded audio, not the concealment the MLOW attempts queued"
        );
        // The number is the assertion for the same reason as the direct path's: concealment and
        // playout priming are both silence, and only their length tells them apart.
        assert_eq!(
            first_audible, 960,
            "the concealment from before the latch must not sit in front of the rescued audio"
        );
    }

    // A group SSRC comes from the device identity, not the PID, so a replacement session keeps it
    // and its sequence numbers restart BELOW the retired session's maximum. Reception stats left
    // behind then read every replacement packet as reordered, which CLEARS the frame span rather
    // than measuring one -- and the probe abstains without a span. A participant that needs probing
    // to be heard stays silent until the new sequence climbs past the old maximum.
    #[test]
    fn a_pid_migration_clears_the_reception_stats_so_the_probe_can_still_measure() {
        let (mut eng, epoch) = group_engine(false);
        eng = eng.with_foreign_audio_codec_factory(Box::new(StubCodecFactory { samples: 960 }));
        eng.start(0, 1_700_000_000_000);
        let _ = drain(&mut eng);
        let allocate = allocate_success(&eng);
        eng.handle_input(1, Input::RelayPacket(&allocate));
        let _ = drain(&mut eng);

        // The retired session climbs to a high sequence number.
        let silk: Vec<u8> = core::iter::once(0x58u8).chain(0..80u8).collect();
        let mut retired = group_peer_audio(&epoch);
        for n in 0..40u64 {
            let packet = retired.protect_audio(&silk);
            eng.handle_input(100 + n * 60, Input::RelayPacket(&packet));
            let _ = drain(&mut eng);
        }

        // The participant comes back under a new PID.
        let mut migrated = group_update("audio");
        migrated.transaction_id = 8;
        migrated.participants[1].devices[0].pid = Some(99);
        eng.apply_group_update(3_000, &migrated)
            .expect("the roster applies");
        let _ = drain(&mut eng);

        // A fresh pipeline: same SSRC (it comes from the LID), sequence numbers from the start.
        let mut replacement = group_peer_audio(&epoch);
        let mut heard = Vec::new();
        for n in 0..8u64 {
            let packet = replacement.protect_audio(&silk);
            eng.handle_input(4_000 + n * 60, Input::RelayPacket(&packet));
            for tick in 0..3u64 {
                eng.handle_input(4_000 + n * 60 + tick * 20, Input::Timeout);
                let (outputs, _) = drain(&mut eng);
                for output in outputs {
                    if let Output::Playout(frame) = output {
                        heard.extend(frame);
                    }
                }
            }
        }

        assert!(
            heard.contains(&1234),
            "the replacement session has to be probed and heard, not wait out the old sequence"
        );
    }

    // A standard Opus encoder switches modes with the signal: CELT for music-like frames, SILK for
    // speech. Only the CELT ones carry the top bits the discriminator reads, so without latching the
    // participant, its first SILK packet is classified MLOW and dropped -- and an encoder that
    // alternates keeps re-losing them, since the probe abstains on the CELT ones and never latches.
    #[test]
    fn a_group_participant_latched_by_native_celt_keeps_its_grammar_through_silk() {
        let mut cfg = config(true);
        cfg.audio = AudioConfig::encoded(AudioFormat::MLOW_16KHZ_60MS);
        let mut eng = CallEngine::new(cfg, Box::new(SequentialTxIds::new())).expect("engine");
        let update = group_update("audio");
        eng.configure_group(GroupEngineConfig {
            call_creator: update.call_creator.clone(),
            self_jid: SELF_LID.parse().expect("self JID"),
            initial_update: update,
            direct_peer: None,
        })
        .expect("configure group");
        let epoch = [0x42; 32];
        assert_eq!(
            eng.apply_group_raw_epoch(7, &epoch).expect("install epoch"),
            GroupEpochApply::Installed
        );
        eng.start(0, 1_700_000_000_000);
        let _ = drain(&mut eng);
        let allocate = allocate_success(&eng);
        eng.handle_input(1, Input::RelayPacket(&allocate));
        let _ = drain(&mut eng);

        let mut peer = group_peer_audio(&epoch);
        let mut celt: Vec<u8> = vec![27 << 3 | 3, 3];
        celt.extend(core::iter::repeat_n(0x11u8, 60));
        let packet = peer.protect_audio(&celt);
        eng.handle_input(100, Input::RelayPacket(&packet));
        let _ = drain(&mut eng);

        // The same participant's next frame is SILK: no marker, so classification alone says MLOW.
        let silk: Vec<u8> = core::iter::once(0x58u8).chain(0..80u8).collect();
        assert_ne!(silk[0] & 0xC0, 0xC0, "SILK carries no marker at all");
        let packet = peer.protect_audio(&silk);
        eng.handle_input(160, Input::RelayPacket(&packet));
        let (outputs, _) = drain(&mut eng);
        let frame = outputs
            .iter()
            .find_map(|output| match output {
                Output::EncodedAudio(frame) => Some(frame),
                _ => None,
            })
            .expect("the SILK packet reaches the sink");
        assert_eq!(
            frame.codec,
            AudioCodec::Opus,
            "one native packet settles the participant's grammar; the next need not re-prove it"
        );
        assert_eq!(frame.format, AudioFormat::OPUS_16KHZ_60MS);
        assert_eq!(frame.data.as_ref(), silk.as_slice(), "and is untouched");
    }

    // A participant whose native Opus stream is CELT from its FIRST packet is never classified MLOW,
    // so the probe never sees it: the marker calls it an escape, its untouched TOC is rewritten, and
    // it is corrupted for the whole call with nothing able to notice. The promotion fix does not
    // reach this -- there is nothing to promote it. What separates the two is arithmetic: an escape
    // cannot parse as Opus at the negotiated cadence and native CELT at that cadence parses exactly.
    #[test]
    fn native_celt_from_an_unpromoted_participant_is_not_read_as_an_escape() {
        let mut cfg = config(true);
        cfg.audio = AudioConfig::encoded(AudioFormat::MLOW_16KHZ_60MS);
        let mut eng = CallEngine::new(cfg, Box::new(SequentialTxIds::new())).expect("engine");
        let update = group_update("audio");
        eng.configure_group(GroupEngineConfig {
            call_creator: update.call_creator.clone(),
            self_jid: SELF_LID.parse().expect("self JID"),
            initial_update: update,
            direct_peer: None,
        })
        .expect("configure group");
        let epoch = [0x42; 32];
        assert_eq!(
            eng.apply_group_raw_epoch(7, &epoch).expect("install epoch"),
            GroupEpochApply::Installed
        );
        eng.start(0, 1_700_000_000_000);
        let _ = drain(&mut eng);
        let allocate = allocate_success(&eng);
        eng.handle_input(1, Input::RelayPacket(&allocate));
        let _ = drain(&mut eng);

        // Config 27, code 3, three 20ms CELT frames: 60ms, the negotiated cadence. Its TOC 0xDB is
        // in the escape's bit class, which is the whole difficulty.
        let mut native: Vec<u8> = vec![27 << 3 | 3, 3];
        native.extend(core::iter::repeat_n(0x11u8, 60));
        assert_eq!(native[0] & 0xC0, 0xC0, "and so looks like an escape");

        let mut peer = group_peer_audio(&epoch);
        let packet = peer.protect_audio(&native);
        eng.handle_input(100, Input::RelayPacket(&packet));
        let (outputs, _) = drain(&mut eng);
        let frame = outputs
            .iter()
            .find_map(|output| match output {
                Output::EncodedAudio(frame) => Some(frame),
                _ => None,
            })
            .expect("the packet reaches the sink");
        assert_eq!(frame.codec, AudioCodec::Opus);
        assert_eq!(
            frame.format,
            AudioFormat::OPUS_16KHZ_60MS,
            "native CELT is native Opus, not MLOW's container, from the first packet"
        );

        // And the escape of that same packet is still an escape: the rule separates them, it does
        // not simply stop believing the marker.
        let mut escape = native.clone();
        crate::voip::audio::packetize_opus_for_mlow(&mut escape).expect("a valid escape");
        assert!(
            AudioFormat::MLOW_16KHZ_60MS.payload_is_mlow_escape(&escape),
            "the escape of the same packet must still read as one"
        );
        assert!(
            !AudioFormat::MLOW_16KHZ_60MS.payload_is_mlow_escape(&native),
            "and the native packet must not"
        );
    }

    // `is_mlow_embedded_opus` tests the top two bits, and EVERY native Opus CELT config (24..=31)
    // sets them: a native 60ms CELT packet starts 0xC3. So the escape marker cannot tell a rewritten
    // TOC from a native CELT one, and reading it first meant a promoted participant's native packet
    // was called an escape and had its untouched TOC rewritten anyway -- a decode failure rather
    // than a mislabel. The latched promotion is the only thing that knows better, so it goes first.
    #[test]
    fn a_promoted_participant_sending_native_celt_is_not_read_as_an_escape() {
        let mut cfg = config(true);
        cfg.audio = AudioConfig::encoded(AudioFormat::MLOW_16KHZ_60MS);
        let mut eng = CallEngine::new(cfg, Box::new(SequentialTxIds::new())).expect("engine");
        let update = group_update("audio");
        eng.configure_group(GroupEngineConfig {
            call_creator: update.call_creator.clone(),
            self_jid: SELF_LID.parse().expect("self JID"),
            initial_update: update,
            direct_peer: None,
        })
        .expect("configure group");
        let epoch = [0x42; 32];
        assert_eq!(
            eng.apply_group_raw_epoch(7, &epoch).expect("install epoch"),
            GroupEpochApply::Installed
        );
        eng.start(0, 1_700_000_000_000);
        let _ = drain(&mut eng);
        let allocate = allocate_success(&eng);
        eng.handle_input(1, Input::RelayPacket(&allocate));
        let _ = drain(&mut eng);

        // Promote the participant with unmarked SILK packets, as the probe does.
        let silk: Vec<u8> = core::iter::once(0x58u8).chain(0..80u8).collect();
        let mut peer = group_peer_audio(&epoch);
        for n in 0..6u64 {
            let packet = peer.protect_audio(&silk);
            eng.handle_input(100 + n * 60, Input::RelayPacket(&packet));
            let _ = drain(&mut eng);
        }

        // Now the same participant sends native CELT, whose TOC is in the escape's bit class.
        let native_celt: Vec<u8> = core::iter::once(0xC3u8).chain(0..40u8).collect();
        assert_eq!(native_celt[0] & 0xC0, 0xC0, "and so looks like an escape");
        let packet = peer.protect_audio(&native_celt);
        eng.handle_input(500, Input::RelayPacket(&packet));
        let (outputs, _) = drain(&mut eng);
        let frame = outputs
            .iter()
            .find_map(|output| match output {
                Output::EncodedAudio(frame) => Some(frame),
                _ => None,
            })
            .expect("the packet reaches the sink");
        assert_eq!(frame.codec, AudioCodec::Opus);
        assert_eq!(
            frame.format,
            AudioFormat::OPUS_16KHZ_60MS,
            "a promoted participant's bytes are native, whatever the top bits look like"
        );
        assert_eq!(
            frame.data.as_ref(),
            native_celt.as_slice(),
            "and must reach the sink untouched"
        );
    }

    // Promoting an escape would be a disaster -- the sink would get a rewritten TOC labelled native
    // Opus and could not decode that participant again -- and two independent things prevent it.
    //
    // The one this asserts is local: the probe is asked about the packet's OWN classification, and
    // an escape already answers Opus, so there is nothing to corroborate and it abstains.
    //
    // The other is arithmetic, and is why this test passes even with that argument reverted: an
    // escape's low TOC bit means "multiple frames", not an Opus frame count, so read as Opus it is
    // code 0 or code 1 -- one or two frames. The escape's configs are all CELT (2.5/5/10/20 ms), so
    // the most it can claim is 40 ms, and it can never total the 960 samples the probe requires.
    // Depending on that is depending on a coincidence between two modules; the classification makes
    // the guarantee local, which is why the change stands on its own.
    // A valid MLOW escape is an Opus packet with one byte rewritten, so at the negotiated cadence
    // it agrees with itself and satisfies the probe. Promoting on that evidence relabels a payload
    // whose TOC IS rewritten as native Opus, and the sink cannot decode that participant for the
    // rest of the call -- the probe turned a working stream into a broken one. The packet's own
    // classification is what stops it: it already says Opus, so there is nothing to corroborate.
    #[test]
    fn diag_escape_shapes() {
        for (config, code, frames, nbody) in [
            (24u8, 3u8, 3u8, 60usize),
            (25, 3, 3, 60),
            (26, 3, 3, 60),
            (27, 3, 3, 60),
            (28, 3, 3, 60),
            (29, 3, 3, 60),
            (30, 3, 3, 60),
            (31, 3, 3, 60),
            (24, 3, 6, 60),
            (25, 3, 6, 60),
            (26, 3, 2, 60),
            (27, 3, 2, 60),
        ] {
            let mut p: Vec<u8> = vec![config << 3 | code, frames];
            p.extend(core::iter::repeat_n(0x11u8, nbody));
            let before = p.clone();
            if crate::voip::audio::packetize_opus_for_mlow(&mut p).is_err() {
                continue;
            }
            let shape = crate::voip::opus_packet_shape(&p);
            let total = shape.and_then(|s| s.total_samples(16_000));
            eprintln!(
                "cfg={config} code={code} frames={frames} rfc_toc={:#04x} escape_toc={:#04x} total_at_16k={total:?}",
                before[0], p[0]
            );
        }
    }

    #[test]
    fn a_valid_group_escape_is_never_promoted_to_native_opus() {
        let mut cfg = config(true);
        cfg.audio = AudioConfig::encoded(AudioFormat::MLOW_16KHZ_60MS);
        let mut eng = CallEngine::new(cfg, Box::new(SequentialTxIds::new())).expect("engine");
        let update = group_update("audio");
        eng.configure_group(GroupEngineConfig {
            call_creator: update.call_creator.clone(),
            self_jid: SELF_LID.parse().expect("self JID"),
            initial_update: update,
            direct_peer: None,
        })
        .expect("configure group");
        let epoch = [0x42; 32];
        assert_eq!(
            eng.apply_group_raw_epoch(7, &epoch).expect("install epoch"),
            GroupEpochApply::Installed
        );
        eng.start(0, 1_700_000_000_000);
        let _ = drain(&mut eng);
        let allocate = allocate_success(&eng);
        eng.handle_input(1, Input::RelayPacket(&allocate));
        let _ = drain(&mut eng);

        // A genuine escape, built by the function that writes them: an RFC code-3 CELT packet of
        // three 20ms frames -- 60ms, the negotiated cadence, which is what makes it agree.
        let mut escape: Vec<u8> = vec![0xC3, 0x03];
        escape.extend(core::iter::repeat_n(0x11u8, 60));
        crate::voip::audio::packetize_opus_for_mlow(&mut escape).expect("a valid escape");
        assert_eq!(escape[0] & 0xC0, 0xC0, "the fixture really is an escape");

        let mut peer = group_peer_audio(&epoch);
        let mut delivered = Vec::new();
        for n in 0..8u64 {
            let packet = peer.protect_audio(&escape);
            eng.handle_input(100 + n * 60, Input::RelayPacket(&packet));
            let (outputs, _) = drain(&mut eng);
            for output in outputs {
                if let Output::EncodedAudio(frame) = output {
                    delivered.push((frame.codec, frame.format, frame.data.clone()));
                }
            }
        }

        assert_eq!(delivered.len(), 8);
        for (codec, format, data) in &delivered {
            assert_eq!(*codec, AudioCodec::Opus, "an escape carries Opus");
            assert_eq!(
                *format,
                AudioFormat::MLOW_16KHZ_60MS,
                "but in MLOW's container, which is what the sink needs to know to undo it"
            );
            assert_eq!(data.as_ref(), escape.as_slice(), "delivered unchanged");
        }
    }

    #[test]
    fn group_foreign_opus_keeps_sender_metadata_and_updates_rtcp_reception() {
        use crate::voip::e2e_srtp::{derive_srtcp_keys_from_raw, unprotect_srtcp};

        let (mut eng, epoch) = group_engine(false);
        eng.start(0, 1_700_000_000_000);
        let _ = drain(&mut eng);
        let allocate = allocate_success(&eng);
        eng.handle_input(1, Input::RelayPacket(&allocate));
        let _ = drain(&mut eng);

        let mut peer = group_peer_audio(&epoch);
        let embedded_opus = [0xF8u8, 0xFF, 0xFE];
        let packet = peer.protect_audio(&embedded_opus);
        let peer_ssrc = parse_rtp_header(&packet).expect("RTP header").ssrc;
        eng.handle_input(100, Input::RelayPacket(&packet));
        let (outputs, _) = drain(&mut eng);
        let frame = outputs
            .iter()
            .find_map(|output| match output {
                Output::Event(CallEvent::ForeignGroupAudio(frame)) => Some(frame),
                _ => None,
            })
            .unwrap_or_else(|| panic!("missing foreign group audio event: {outputs:?}"));
        assert_eq!(frame.data.as_ref(), embedded_opus);
        assert_eq!(
            frame.sender.as_ref().map(ToString::to_string).as_deref(),
            Some("222222222222222@lid")
        );
        let expected_device = PEER_LID.parse::<Jid>().expect("peer JID").to_string();
        assert_eq!(
            frame.device.as_ref().map(ToString::to_string).as_deref(),
            Some(expected_device.as_str())
        );
        assert_eq!(frame.pid, Some(2));

        eng.handle_input(1 + RTCP_MS, Input::Timeout);
        let (outputs, _) = drain(&mut eng);
        let protected = outputs
            .iter()
            .find_map(|output| match output {
                Output::Transmit(packet)
                    if classify_relay_packet(packet) == RelayPacketKind::Rtcp =>
                {
                    Some(packet)
                }
                _ => None,
            })
            .expect("group audio Sender Report");
        let sender_ssrc = parse_rtcp_sender_ssrc(protected).expect("sender SSRC");
        let self_id = ssrc::format_e2e_srtp_participant_id(SELF_LID);
        let keys = derive_srtcp_keys_from_raw(&epoch, &self_id).expect("group SRTCP keys");
        let (plain, _) = unprotect_srtcp(&keys, sender_ssrc, protected).expect("group SRTCP");
        let summary = summarize_rtcp(&plain).expect("RTCP summary");
        assert_eq!(summary.referenced_ssrcs, [peer_ssrc]);
        assert_eq!(summary.report_blocks.len(), 1);
    }

    #[test]
    fn group_sender_reports_update_only_the_matching_reception_stream() {
        let (mut eng, epoch) = group_engine(true);
        let peer_id = ssrc::format_e2e_srtp_participant_id(PEER_LID);
        let mut audio = group_peer_audio(&epoch);
        let mut video = group_peer_video(&epoch);

        let audio_packet = audio.protect_audio(&[0xF8, 0xFF, 0xFE]);
        eng.handle_input(1, Input::RelayPacket(&audio_packet));
        for packet in video.protect_video(&video_au(100)) {
            eng.handle_input(2, Input::RelayPacket(&packet));
        }
        let _ = drain(&mut eng);

        let audio_sr = audio.audio_sender_report(1_700_000_000_000, None);
        eng.handle_input(100, Input::RelayPacket(&audio_sr));
        let _ = drain(&mut eng);
        let audio_lsr = {
            let group = eng.group.as_mut().expect("group engine");
            let audio_report = group
                .audio_reception
                .get_mut(&peer_id)
                .and_then(|stats| stats.report(101))
                .expect("audio reception report");
            let video_report = group
                .video_reception
                .get_mut(&peer_id)
                .and_then(|stats| stats.report(101))
                .expect("video reception report");
            assert_ne!(audio_report.last_sender_report, 0);
            assert_eq!(
                video_report.last_sender_report, 0,
                "an audio sender report must not overwrite video timing"
            );
            audio_report.last_sender_report
        };

        let video_sr = video.video_sender_report(1_700_000_100_000, None);
        eng.handle_input(200, Input::RelayPacket(&video_sr));
        let _ = drain(&mut eng);
        let group = eng.group.as_mut().expect("group engine");
        let audio_report = group
            .audio_reception
            .get_mut(&peer_id)
            .and_then(|stats| stats.report(201))
            .expect("audio reception report");
        let video_report = group
            .video_reception
            .get_mut(&peer_id)
            .and_then(|stats| stats.report(201))
            .expect("video reception report");
        assert_eq!(
            audio_report.last_sender_report, audio_lsr,
            "a video sender report must not overwrite audio timing"
        );
        assert_ne!(video_report.last_sender_report, 0);
        assert_ne!(video_report.last_sender_report, audio_lsr);
    }

    #[test]
    fn installing_first_group_epoch_admits_roster_audio_to_mixer() {
        let (mut eng, epoch) = group_engine(false);
        eng.start(0, 0);
        let _ = drain(&mut eng);
        let mut peer = group_peer_audio(&epoch);
        let mut encoder = MlowEncoder::new();

        for frame in 0..2u32 {
            let tone = (0..SAMPLES as usize)
                .map(|sample| 0.3 * ((sample as f32 + (frame * SAMPLES) as f32) * 0.07).sin())
                .collect::<Vec<_>>();
            let packet = peer.protect_audio(&encoder.encode(&tone).expect("MLOW frame"));
            eng.handle_input(1, Input::RelayPacket(&packet));
            let _ = drain(&mut eng);
        }

        eng.handle_input(PLAYOUT_MS, Input::Timeout);
        let (outputs, _) = drain(&mut eng);
        assert!(outputs.iter().any(|output| {
            matches!(output, Output::Playout(frame) if frame.iter().any(|sample| *sample != 0))
        }));
    }

    // A group call's receive path is the same question as a direct one's: what arrived, what was
    // discarded, and where. It reported none of it -- a healthy group call read as having received
    // nothing at all, while `forwarding_envelope_rejected` moved in the same snapshot.
    #[test]
    fn group_audio_moves_the_same_counters_as_a_direct_call() {
        let (mut eng, epoch) = group_engine(false);
        eng.start(0, 0);
        let _ = drain(&mut eng);
        let mut peer = group_peer_audio(&epoch);
        let mut encoder = MlowEncoder::new();

        for frame in 0..3u32 {
            let tone = (0..SAMPLES as usize)
                .map(|sample| 0.3 * ((sample as f32 + (frame * SAMPLES) as f32) * 0.07).sin())
                .collect::<Vec<_>>();
            let packet = peer.protect_audio(&encoder.encode(&tone).expect("MLOW frame"));
            eng.handle_input(1, Input::RelayPacket(&packet));
            let _ = drain(&mut eng);
        }
        let stats = eng.media_stats();
        assert_eq!(stats.rtp_received, 3, "the packets that authenticated");
        assert!(stats.audio_frames_decoded > 0, "and became audio");

        // A payload type outside the profile, and a packet that cannot authenticate, each leave
        // their own trace rather than looking like a peer who stopped speaking.
        let mut wrong_pt = peer.protect_audio(&[0x08, 1, 2, 3]);
        wrong_pt[1] = (wrong_pt[1] & 0x80) | 99;
        eng.handle_input(2, Input::RelayPacket(&wrong_pt));
        let _ = drain(&mut eng);
        assert_eq!(eng.media_stats().rtp_payload_type_unexpected, 1);

        let mut forged = peer.protect_audio(&[0x08, 4, 5, 6]);
        let last = forged.len() - 1;
        forged[last] ^= 0xff;
        eng.handle_input(3, Input::RelayPacket(&forged));
        let _ = drain(&mut eng);
        assert_eq!(eng.media_stats().srtp_unprotect_failed, 1);
        assert_eq!(
            eng.media_stats().rtp_received,
            3,
            "neither of those two was a packet we read"
        );
    }

    #[test]
    fn group_video_uses_per_participant_orientation() {
        let (mut eng, epoch) = group_engine(true);
        let peer_device = PEER_LID.parse::<Jid>().expect("peer JID");
        eng.set_participant_video_orientation(peer_device.clone(), 2);
        let mut peer = group_peer_video(&epoch);
        let packet = peer
            .protect_video(&video_au(100))
            .pop()
            .expect("one-packet video");
        eng.handle_input(1, Input::RelayPacket(&packet));
        let (outputs, _) = drain(&mut eng);
        let peer_user = Jid::new("222222222222222", Server::Lid);
        assert!(outputs.iter().any(|output| matches!(
            output,
            Output::VideoPlayout(VideoFrame {
                orientation: 2,
                sender: Some(sender),
                device: Some(device),
                pid: Some(2),
                ..
            }) if *sender == peer_user && *device == peer_device
        )));
    }

    #[test]
    fn group_video_orientation_keeps_sibling_devices_distinct() {
        let (mut eng, _epoch) = group_engine(true);
        let first = Jid::new("222222222222222", Server::Lid).with_device(2);
        let second = first.to_non_ad().with_device(3);
        eng.set_participant_video_orientation(first.clone(), 1);
        eng.set_participant_video_orientation(second.clone(), 3);

        let orientations = &eng.group.as_ref().expect("group state").video_orientations;
        assert_eq!(orientations.get(&first), Some(&1));
        assert_eq!(orientations.get(&second), Some(&3));
        assert!(
            !orientations.contains_key(&first.to_non_ad()),
            "device controls must not overwrite a user-wide fallback"
        );
    }

    #[test]
    fn group_video_does_not_inherit_the_direct_peer_orientation() {
        let (mut eng, epoch) = group_engine(true);
        eng.set_peer_video_orientation(3);
        let mut peer = group_peer_video(&epoch);
        let packet = peer
            .protect_video(&video_au(100))
            .pop()
            .expect("one-packet video");
        eng.handle_input(1, Input::RelayPacket(&packet));
        assert!(drain(&mut eng).0.iter().any(|output| matches!(
            output,
            Output::VideoPlayout(VideoFrame { orientation: 0, .. })
        )));
    }

    #[test]
    fn mlow_red_payload_type_is_depacketized_before_toc_dispatch() {
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
        assert!(peer_tx.set_audio_payload_type(RTP_PAYLOAD_TYPE_MLOW_RED));
        let tone = (0..SAMPLES as usize)
            .map(|sample| 0.3 * (sample as f32 * 0.05).sin())
            .collect::<Vec<_>>();
        let main = MlowEncoder::new().encode(&tone).expect("MLOW frame");
        // 0xC0 is a RED header here, not MLOW's embedded-Opus escape.
        let mut red = vec![0xC0, 1, 0, 0x90];
        red.extend_from_slice(&main);
        let packet = peer_tx.protect_audio(&red);
        eng.handle_input(1, Input::RelayPacket(&packet));
        let (outputs, _) = drain(&mut eng);

        assert!(
            outputs
                .iter()
                .all(|output| !matches!(output, Output::Event(CallEvent::ForeignAudio(_))))
        );
        assert_eq!(eng.jitter_len(), SAMPLES as usize);
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

    // An encoded call hands the ciphertext of a failed tag to its sink by contract -- but calling
    // that produced audio keeps `window_produced` nonzero through a whole run of failures, so
    // `AudioSilent` never fires and `AuthenticationFailing`, the one reason naming the real cause,
    // is unreachable. The sink still gets the bytes; the watchdog stops being told they are audio.
    #[test]
    fn failed_sframe_ciphertext_is_delivered_but_is_not_produced_audio() {
        let mut cfg = config(true);
        cfg.enable_sframe = true;
        cfg.audio = AudioConfig::encoded(AudioFormat::MLOW_16KHZ_60MS);
        let mut eng = CallEngine::new(cfg, Box::new(SequentialTxIds::new())).unwrap();
        eng.start(0, 0);
        let _ = drain(&mut eng);
        let success = allocate_success(&eng);
        eng.handle_input(0, Input::RelayPacket(&success));
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
        let mut peer_sframe = SframeSession::new(&call_key, PEER_LID, SELF_LID).unwrap();

        // One frame that authenticates: the proof the peer wraps, without which a failure is not
        // attributable to authentication at all.
        let wrapped = peer_sframe.encrypt(&[0x50, 1, 2, 3]);
        let packet = peer_tx.protect_audio(&wrapped);
        eng.handle_input(1, Input::RelayPacket(&packet));
        let _ = drain(&mut eng);

        // Then a run whose tags do not: same wrapping, one byte of ciphertext flipped. Sent as a
        // real stream would be -- packets arriving THROUGH the health windows, since a window with
        // no arrivals at all is a reception stall rather than a call that receives and stays silent.
        let mut delivered = 0;
        let mut reasons = Vec::new();
        for n in 0..160u64 {
            let mut wrapped = peer_sframe.encrypt(&[0x50, 4, 5, 6]);
            wrapped[0] ^= 0xFF;
            let packet = peer_tx.protect_audio(&wrapped);
            let now = 100 + n * 60;
            eng.handle_input(now, Input::RelayPacket(&packet));
            eng.handle_input(now, Input::Timeout);
            let (outputs, _) = drain(&mut eng);
            for output in outputs {
                match output {
                    Output::EncodedAudio(_) => delivered += 1,
                    Output::Event(CallEvent::AudioSilent {
                        dominant_reason, ..
                    }) => reasons.push(dominant_reason),
                    _ => {}
                }
            }
        }
        assert_eq!(delivered, 160, "the contract still hands the bytes over");
        assert!(
            eng.media_stats().sframe_decrypt_failed >= 160,
            "and every one is counted as a tag that failed"
        );
        assert!(
            reasons.contains(&AudioSilenceReason::AuthenticationFailing),
            "the alarm has to name authentication, got {reasons:?}"
        );
        // And the PUBLIC statistics have to agree with the watchdog. `audio_produced()` sums
        // `audio_frames_delivered`, so counting the ciphertext there reported 161 frames of produced
        // audio for a call that produced one -- the watchdog disbelieving the frames while the
        // number a consumer reads still vouched for them.
        assert_eq!(
            eng.media_stats().audio_produced(),
            1,
            "only the frame that authenticated was audio"
        );
    }

    // The PCM twin of the encoded case, and the third site that credited unauthenticated bytes as
    // audio. Random ciphertext reaching the MLow decoder can be classified as a SID -- the peer
    // telling us it is silent -- which counts as production and resets the silence window, so a
    // sustained tag failure suppresses the very alarm that would name it.
    #[test]
    fn failed_sframe_ciphertext_does_not_credit_the_pcm_path_with_audio() {
        let mut cfg = config(true);
        cfg.enable_sframe = true;
        let call_key = cfg.call_key.clone();
        // A decoder is installed so the ciphertext that happens to classify as Opus cannot raise
        // `NoDecoderForNegotiatedCodec`, which outranks every other reason and would answer a
        // different question than this test asks.
        let mut eng = CallEngine::new(cfg, Box::new(SequentialTxIds::new()))
            .unwrap()
            .with_foreign_audio_codec(Box::new(StubForeignCodec { samples: 960 }));
        eng.start(0, 0);
        let _ = drain(&mut eng);
        let success = allocate_success(&eng);
        eng.handle_input(0, Input::RelayPacket(&success));
        let _ = drain(&mut eng);

        let mut peer_tx = peer_pipeline();
        let mut peer_sframe = SframeSession::new(&call_key, PEER_LID, SELF_LID).unwrap();

        // One frame that authenticates, without which a failure is not attributable at all.
        let wrapped = peer_sframe.encrypt(&[0x90; 20]);
        let packet = peer_tx.protect_audio(&wrapped);
        eng.handle_input(1, Input::RelayPacket(&packet));
        let _ = drain(&mut eng);
        // Whatever that one authenticated frame produced is the whole of the audio this call ever
        // received. Taken as a baseline rather than pinned to a literal, because it depends on what
        // the MLow decoder makes of the frame -- the claim under test is that the 160 that follow
        // add nothing to it, not what this particular frame decodes to.
        let produced_when_authenticated = eng.media_stats().audio_produced();

        let mut reasons = Vec::new();
        for n in 0..160u64 {
            // A byte in the middle of the ciphertext. Not the first -- flipping that makes
            // classification read the frame as Opus and the alarm names the missing decoder, a
            // different finding -- and not the last, which is the SFrame trailer the parser needs to
            // recognise the frame as wrapped at all. The grammar has to stay MLOW and the frame has
            // to stay recognisably SFrame for this to be about authentication.
            let mut wrapped = peer_sframe.encrypt(&[0x90; 20]);
            let middle = wrapped.len() / 2;
            wrapped[middle] ^= 0xFF;
            let packet = peer_tx.protect_audio(&wrapped);
            let now = 100 + n * 60;
            eng.handle_input(now, Input::RelayPacket(&packet));
            eng.handle_input(now, Input::Timeout);
            let (outputs, _) = drain(&mut eng);
            for output in outputs {
                if let Output::Event(CallEvent::AudioSilent {
                    dominant_reason, ..
                }) = output
                {
                    reasons.push(dominant_reason);
                }
            }
        }

        assert!(
            eng.media_stats().sframe_decrypt_failed >= 160,
            "every one is counted as a tag that failed"
        );
        assert!(
            reasons.contains(&AudioSilenceReason::AuthenticationFailing),
            "and the alarm has to fire and name authentication, got {reasons:?}"
        );
        // And the public snapshot has to agree with the watchdog: `audio_produced()` sums
        // `audio_frames_decoded`, so an ungated increment would let a consumer's number climb
        // through frames the watchdog above has already stopped believing. This assertion does not
        // discriminate the gate today -- MLow answers off-point, SID or concealment for these
        // frames and never `decoded` -- so it guards the engine's side of a contract whose other
        // side is the decoder's behaviour. It is here to fail if that behaviour ever changes.
        assert_eq!(
            eng.media_stats().audio_produced(),
            produced_when_authenticated,
            "not one of the 160 unauthenticated frames may count as produced audio"
        );
    }

    // `SframeSession::decrypt` reads the wrapping off the frame's own trailing bytes, and a plain
    // codec frame whose last bytes happen to parse as a header fails GCM exactly like a corrupted
    // wrapped one. Counting a failed tag on its own therefore reports authentication failures on a
    // healthy unwrapped call. One frame that DOES authenticate is the proof the peer wraps, and
    // only after it is a failure attributable.
    #[test]
    fn a_failed_tag_counts_only_once_the_peer_is_known_to_wrap() {
        let mut cfg = config(true);
        cfg.enable_sframe = true;
        let mut eng = CallEngine::new(cfg, Box::new(SequentialTxIds::new())).unwrap();
        eng.start(0, 0);
        let _ = drain(&mut eng);
        let success = allocate_success(&eng);
        eng.handle_input(0, Input::RelayPacket(&success));
        let _ = drain(&mut eng);

        let call_key: Vec<u8> = (0u8..32).collect();
        let mut peer_tx = peer_pipeline();
        // Ends in `00 00 03`: a 3-byte header whose two varints parse, so this unwrapped frame is
        // read as SFrame-framed and its (absent) tag cannot authenticate.
        let mut looks_wrapped = vec![0x50u8; 24];
        looks_wrapped.extend_from_slice(&[0x00, 0x00, 0x03]);
        eng.handle_input(
            1,
            Input::RelayPacket(&peer_tx.protect_audio(&looks_wrapped)),
        );
        let _ = drain(&mut eng);
        assert_eq!(
            eng.media_stats().sframe_decrypt_failed,
            0,
            "nothing has authenticated yet, so this is indistinguishable from a peer that does not wrap"
        );

        // A genuinely wrapped frame settles the question: this peer wraps.
        let mut peer_sframe = SframeSession::new(&call_key, PEER_LID, SELF_LID).unwrap();
        let mut peer_enc = MlowEncoder::new();
        let tone: Vec<f32> = (0..SAMPLES as usize)
            .map(|i| 0.3 * (i as f32 * 0.07).sin())
            .collect();
        let frame = peer_enc.encode(&tone).expect("mlow encode");
        let wrapped = peer_sframe.encrypt(&frame);
        eng.handle_input(2, Input::RelayPacket(&peer_tx.protect_audio(&wrapped)));
        let _ = drain(&mut eng);
        assert_eq!(
            eng.media_stats().sframe_decrypt_failed,
            0,
            "it authenticated"
        );

        // Now the same failing frame IS a failure, because we know what this peer sends.
        eng.handle_input(
            3,
            Input::RelayPacket(&peer_tx.protect_audio(&looks_wrapped)),
        );
        let _ = drain(&mut eng);
        assert_eq!(
            eng.media_stats().sframe_decrypt_failed,
            1,
            "a tag that fails after the peer has proven it wraps is a real failure"
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

        let allocate = allocate_success(&eng);
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
        let allocate = allocate_success(&eng);
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
        let allocate = allocate_success(&eng);
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
        rr.extend_from_slice(&[0x81, RTCP_PT_PSFB, 0, 2]);
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
            }) if packet_types == &[RTCP_PT_RR, RTCP_PT_PSFB]
                && *sender_ssrc == peer_ssrc
                && referenced_ssrcs.contains(&audio_ssrc)
                && referenced_ssrcs.contains(&video_ssrc)
                && feedback.iter().any(|item| item.packet_type == RTCP_PT_PSFB
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
    fn authenticated_group_pli_requires_a_new_idr() {
        use crate::voip::e2e_srtp::{derive_srtcp_keys_from_raw, protect_srtcp};

        let (mut eng, epoch) = group_engine(true);
        eng.handle_input(1, Input::VideoFrame(&video_au(100)));
        assert_eq!(
            count_transmits(&drain(&mut eng).0),
            1,
            "the initial IDR opens group video transmission"
        );
        eng.handle_input(2, Input::VideoFrame(&video_delta_au(100)));
        assert_eq!(count_transmits(&drain(&mut eng).0), 1);

        let self_id = ssrc::format_e2e_srtp_participant_id(SELF_LID);
        let peer_id = ssrc::format_e2e_srtp_participant_id(PEER_LID);
        let local_video_ssrc = ssrc::derive_video_participant_ssrc("CID", &self_id);
        let peer_audio_ssrc = ssrc::derive_wasm_participant_ssrc("CID", &peer_id, 0);
        let mut pli = vec![0x81, RTCP_PT_PSFB, 0, 2];
        pli.extend_from_slice(&peer_audio_ssrc.to_be_bytes());
        pli.extend_from_slice(&local_video_ssrc.to_be_bytes());
        let peer_keys =
            derive_srtcp_keys_from_raw(&epoch, &peer_id).expect("peer group SRTCP keys");
        let protected = protect_srtcp(&peer_keys, peer_audio_ssrc, 0, &pli);
        eng.handle_input(3, Input::RelayPacket(&protected));
        let _ = drain(&mut eng);

        eng.handle_input(4, Input::VideoFrame(&video_delta_au(100)));
        assert_eq!(
            count_transmits(&drain(&mut eng).0),
            0,
            "PLI must suppress dependent group AUs"
        );
        eng.handle_input(5, Input::VideoFrame(&video_au(100)));
        assert_eq!(
            count_transmits(&drain(&mut eng).0),
            1,
            "the next IDR must recover group transmission"
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
        let allocate = allocate_success(&eng);
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
            let _ = drain_playout(
                &mut buf,
                &mut priming,
                &mut priming_ticks,
                OPUS_FRAME_SAMPS_60MS,
                playout_bounds(OPUS_FRAME_SAMPS_60MS).1,
            );
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
            if drain_playout(
                &mut buf,
                &mut priming,
                &mut priming_ticks,
                OPUS_FRAME_SAMPS_60MS,
                playout_bounds(OPUS_FRAME_SAMPS_60MS).1,
            )
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
            let f = drain_playout(
                &mut buf,
                &mut priming,
                &mut priming_ticks,
                OPUS_FRAME_SAMPS_60MS,
                playout_bounds(OPUS_FRAME_SAMPS_60MS).1,
            );
            assert!(f.iter().all(|&s| s == 0), "still priming -> silence");
            assert_eq!(
                buf.len(),
                960,
                "the partial frame is held while priming, not drained"
            );
        }
        // The next tick hits the bound and flushes the held frame as real audio.
        let flushed = drain_playout(
            &mut buf,
            &mut priming,
            &mut priming_ticks,
            OPUS_FRAME_SAMPS_60MS,
            playout_bounds(OPUS_FRAME_SAMPS_60MS).1,
        );
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
            let f = drain_playout(
                &mut buf,
                &mut priming,
                &mut priming_ticks,
                OPUS_FRAME_SAMPS_60MS,
                playout_bounds(OPUS_FRAME_SAMPS_60MS).1,
            );
            assert!(f.iter().all(|&s| s == 0), "empty buffer -> silence");
        }
        // First frame arrives: must NOT flush instantly -- the counter didn't age while empty.
        feed_frame(&mut buf);
        let f = drain_playout(
            &mut buf,
            &mut priming,
            &mut priming_ticks,
            OPUS_FRAME_SAMPS_60MS,
            playout_bounds(OPUS_FRAME_SAMPS_60MS).1,
        );
        assert!(
            f.iter().all(|&s| s == 0),
            "one frame is below the target -> still priming, no instant flush"
        );
        assert_eq!(buf.len(), 960, "the first frame is held for the cushion");
        // The second frame reaches the target -> real audio drains.
        feed_frame(&mut buf);
        let f = drain_playout(
            &mut buf,
            &mut priming,
            &mut priming_ticks,
            OPUS_FRAME_SAMPS_60MS,
            playout_bounds(OPUS_FRAME_SAMPS_60MS).1,
        );
        assert!(
            f.iter().any(|&s| s != 0),
            "at the target playout starts real audio"
        );
    }

    /// A mirrored peer engine's video plane (its self LID = our peer LID), used to craft real
    /// inbound video packets for demux tests.
    fn peer_video_pipe() -> VideoPipeline {
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
    fn source_role_change_rearms_the_outbound_keyframe_gate() {
        let mut cfg = config(true);
        cfg.enable_video = true;
        let mut eng = CallEngine::new(cfg, Box::new(SequentialTxIds::new())).unwrap();
        eng.start(0, 0);
        let _ = drain(&mut eng);

        eng.handle_input(1, Input::VideoFrame(&video_au(100)));
        assert_eq!(count_transmits(&drain(&mut eng).0), 1);
        eng.handle_input(2, Input::VideoFrame(&video_delta_au(100)));
        assert_eq!(count_transmits(&drain(&mut eng).0), 1);

        eng.require_video_keyframe();
        eng.handle_input(3, Input::VideoFrame(&video_delta_au(100)));
        assert_eq!(
            count_transmits(&drain(&mut eng).0),
            0,
            "a replacement camera/screen source must not start on a dependent frame"
        );
        eng.handle_input(4, Input::VideoFrame(&video_au(100)));
        assert_eq!(
            count_transmits(&drain(&mut eng).0),
            1,
            "an IDR must release the replacement source"
        );
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

    /// Ungating an upgrade drops every frame until an IDR arrives, and the
    /// engine cannot make one — it never touches pixels. Saying so is the
    /// difference between the peer's picture appearing at once and appearing a
    /// keyframe period later, which for a three-second GOP is most of a short
    /// call. The shipped client requests one at the same moment.
    #[test]
    fn ungating_an_upgrade_asks_the_application_for_a_keyframe() {
        let mut eng = engine(true);
        assert!(eng.enable_video_gated());
        eng.start(0, 0);
        let _ = drain(&mut eng);

        assert!(eng.enable_video());
        let events = drain(&mut eng).0;
        assert!(
            events
                .iter()
                .any(|o| matches!(o, Output::Event(CallEvent::VideoKeyframeNeeded))),
            "ungating must ask for the IDR it is about to start dropping frames for"
        );

        // Raised once for the requirement, not once per dropped frame: a
        // consumer that reacts by asking its encoder must not be asked again
        // for every delta that arrives before the IDR does.
        eng.handle_input(2, Input::VideoFrame(&video_delta_au(200)));
        assert!(
            !drain(&mut eng)
                .0
                .iter()
                .any(|o| matches!(o, Output::Event(CallEvent::VideoKeyframeNeeded))),
            "a dropped delta must not re-raise the request"
        );

        // Satisfied, and the next requirement raises it again.
        eng.handle_input(3, Input::VideoFrame(&video_au(200)));
        let _ = drain(&mut eng);
        eng.require_video_keyframe();
        assert!(
            drain(&mut eng)
                .0
                .iter()
                .any(|o| matches!(o, Output::Event(CallEvent::VideoKeyframeNeeded))),
            "a fresh requirement is a fresh request"
        );
    }

    /// A video-from-start call brings the plane up ungated and immediately
    /// drops every access unit until an IDR arrives, and nothing on that path
    /// would otherwise ask: the plane is built by the constructor, so it is
    /// already active by the time `enable_video` runs and the resume arm sees
    /// nothing to recover. The caller's picture would appear a keyframe period
    /// late.
    #[test]
    fn a_video_plane_that_starts_ungated_asks_for_its_first_keyframe() {
        let mut cfg = config(true);
        cfg.enable_video = true;
        let mut eng = CallEngine::new(cfg, Box::new(SequentialTxIds::new())).expect("engine");
        assert!(
            drain(&mut eng)
                .0
                .iter()
                .any(|o| matches!(o, Output::Event(CallEvent::VideoKeyframeNeeded))),
            "a plane born requiring an IDR must say so, without waiting to be enabled"
        );

        // Satisfying the requirement ends the request; a plane already asked
        // for and still waiting is not asked again.
        assert!(eng.enable_video());
        assert!(
            !drain(&mut eng)
                .0
                .iter()
                .any(|o| matches!(o, Output::Event(CallEvent::VideoKeyframeNeeded))),
            "one request per requirement, not one per call"
        );

        // A gated plane has nowhere to put one, so it stays quiet until the
        // ungate, which asks on its own.
        let mut gated = engine(true);
        assert!(gated.enable_video_gated());
        assert!(
            !drain(&mut gated)
                .0
                .iter()
                .any(|o| matches!(o, Output::Event(CallEvent::VideoKeyframeNeeded))),
            "an upgrade initiator cannot send video yet, so it must not ask"
        );
        assert!(gated.enable_video());
        assert!(
            drain(&mut gated)
                .0
                .iter()
                .any(|o| matches!(o, Output::Event(CallEvent::VideoKeyframeNeeded)))
        );
    }

    /// The driver retries a refused request only while the engine could still
    /// use the answer. A plane that was downgraded or re-gated in the meantime
    /// would drop the IDR it asked for, and its own re-enable asks again.
    #[test]
    fn a_plane_that_cannot_send_reports_no_keyframe_requirement() {
        let mut eng = engine(true);
        assert!(eng.enable_video());
        assert!(eng.video_keyframe_required(), "a fresh plane is waiting");

        eng.disable_video();
        assert!(
            !eng.video_keyframe_required(),
            "a downgraded plane sends nothing"
        );

        assert!(eng.enable_video_gated());
        assert!(
            !eng.video_keyframe_required(),
            "an upgrade the peer has not accepted sends nothing either"
        );

        assert!(eng.enable_video());
        assert!(
            eng.video_keyframe_required(),
            "the ungate is a fresh requirement"
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
        let mut answerer = VideoPipeline::new(&VideoPipelineParams {
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
        let mut answerer = VideoPipeline::new(&VideoPipelineParams {
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
        let err = allocate_error(&eng, 486); // class 4, number 86
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
        let full = allocate_error(&eng, 486);
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
        let error = allocate_error(&eng, 486);
        eng.handle_input(1, Input::RelayPacket(&error));
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
        let ok = allocate_success(&eng);
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

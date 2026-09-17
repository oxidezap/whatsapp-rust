//! The neutral control-plane media seam.
//!
//! This is the boundary the call control plane uses to reach a media engine. Its API names no type
//! from the `crate::voip` module -- only primitives, `String`, `Vec<u8>`, `Bytes`, the out-of-voip
//! `wacore` types (`wacore_binary::Jid`, `crate::types::call::VideoState`,
//! `crate::types::group_call::{GroupCallUpdate, WaitingRoom, ScreenShare}`) and the flat enums and
//! structs defined here. That is what lets a build enable this module and not `voip`: the compiler
//! names the leak if a draft reaches for an engine type.
//!
//! The engine-facing half is intentionally not here. `whatsapp-rust`'s resident backend implements
//! [`VoipMediaBackend`] on top of `wacore::voip::CallEngine` and translates commands, events and
//! counters across this boundary. `agent_docs/subsystem_boundary.md` records why the byte cut (a
//! facade that compiles with `voip` off) is a later phase: the registry and facade still name
//! signaling types that live in `crate::voip`.

use std::sync::Arc;

use bytes::Bytes;
use wacore_binary::Jid;
use zeroize::Zeroizing;

use crate::sync_marker::MaybeSendSync;
use crate::types::call::VideoState;
use crate::types::group_call::{GroupCallUpdate, ScreenShare, WaitingRoom};

// The one place the neutral contract meets the engine: the consuming `TryFrom` conversions. Kept
// out of this file so the compiler enforces that everything above stays free of `crate::voip`.
#[cfg(feature = "voip")]
mod engine_bridge;

/// One decrypted keygen-v2 epoch, kept as secret material.
///
/// The engine's `GroupRawEpoch` is in `crate::voip::driver`, which is gated by the very feature this
/// contract exists to compile without, so it cannot be named here. This is the neutral twin: it
/// holds the bytes in [`Zeroizing`], so they are erased when the value drops, and its `Debug` prints
/// `[redacted]`, so a stray `{:?}` in a log cannot leak the decrypted key. Both properties matter
/// and are why a bare `Vec<u8>` is not the type: a manual `Debug` alone leaves the bytes in memory
/// after drop, and `Clone` alone leaves a second copy that never gets erased.
#[derive(Clone, PartialEq, Eq)]
pub struct MediaGroupEpoch(Zeroizing<Vec<u8>>);

impl MediaGroupEpoch {
    #[must_use]
    pub fn new(raw_epoch: Vec<u8>) -> Self {
        Self(Zeroizing::new(raw_epoch))
    }

    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_slice()
    }

    /// Take the bytes out, leaving an empty buffer whose allocation is erased on drop.
    ///
    /// Crate-private on purpose: an external consumer leaving with a bare `Vec<u8>` would escape
    /// the erasure this type promises. A consumer that needs the bytes without taking ownership
    /// uses [`as_bytes`](Self::as_bytes).
    #[must_use]
    pub(crate) fn into_bytes(mut self) -> Vec<u8> {
        std::mem::take(&mut *self.0)
    }
}

impl core::fmt::Debug for MediaGroupEpoch {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("MediaGroupEpoch([redacted])")
    }
}

/// The generational identity of one media session.
///
/// A `call_id` alone is not an identity. The registry distinguishes `call ABC gen 12` from a later
/// `call ABC gen 13` so that a finishing task only reaps its OWN registration (the ABA hazard), and
/// a foreign backend keyed only on the call-id could deliver a late message from the old generation
/// into the new session. The generation is the same monotonic token the control plane assigns per
/// registration; a session that never reuses a call-id still carries one.
#[derive(Debug, Clone, PartialEq, Eq, Hash, bon::Builder)]
#[non_exhaustive]
pub struct MediaSessionKey {
    pub call_id: String,
    pub generation: u64,
}

/// Call direction, without the engine's `CallDirection`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MediaDirection {
    Outgoing,
    Incoming,
}

/// Audio codec carried inside the RTP payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MediaAudioCodec {
    Mlow,
    Opus,
}

/// RTP payload family, independent of the codec bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MediaAudioRtpProfile {
    Mlow,
    StandardOpus,
}

/// Where encoding and decoding happen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MediaAudioIo {
    Pcm,
    Encoded,
}

/// Flat audio timing and format. Mirrors `crate::voip::audio::AudioFormat` field for field without
/// depending on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, bon::Builder)]
#[non_exhaustive]
pub struct MediaAudioFormat {
    pub codec: MediaAudioCodec,
    pub rtp_profile: MediaAudioRtpProfile,
    pub signaling_rate: u32,
    pub sample_rate: u32,
    pub channels: u8,
    pub samples_per_frame: u32,
    pub rtp_clock_rate: u32,
    pub rtp_timestamp_step: u32,
    pub rtp_payload_type: u8,
}

impl MediaAudioFormat {
    /// The implemented MLOW operating point: mono, 16 kHz, 60 ms.
    pub const MLOW_16KHZ_60MS: Self = Self {
        codec: MediaAudioCodec::Mlow,
        rtp_profile: MediaAudioRtpProfile::Mlow,
        signaling_rate: 16_000,
        sample_rate: 16_000,
        channels: 1,
        samples_per_frame: 960,
        rtp_clock_rate: 16_000,
        rtp_timestamp_step: 960,
        rtp_payload_type: 120,
    };

    /// Native Opus timing at PT 120 and a 16 kHz RTP clock.
    pub const OPUS_16KHZ_60MS: Self = Self {
        codec: MediaAudioCodec::Opus,
        rtp_profile: MediaAudioRtpProfile::StandardOpus,
        signaling_rate: 16_000,
        sample_rate: 16_000,
        channels: 1,
        samples_per_frame: 960,
        rtp_clock_rate: 16_000,
        rtp_timestamp_step: 960,
        rtp_payload_type: 120,
    };
}

/// Format plus I/O selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, bon::Builder)]
#[non_exhaustive]
pub struct MediaAudioSpec {
    pub format: MediaAudioFormat,
    pub io: MediaAudioIo,
}

/// Keyframe urgency for a peer-keyframe request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MediaKeyframeUrgency {
    Coalesced,
    Immediate,
}

/// Identity of one peer video-upgrade request. Replaces the engine's `VideoUpgradeToken`, whose
/// fields are private, by a neutral `(generation, epoch)` pair an implementation can build and
/// compare. Both fields are needed: `epoch` is what distinguishes two requests in one generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MediaVideoUpgradeToken {
    pub generation: u64,
    pub epoch: u64,
}

/// A roster snapshot plus its decrypted epoch, kept indivisible. Replaces the engine's
/// `GroupControl::Transition`, whose whole reason for existing is that the pair must not separate
/// under mailbox backpressure. The epoch stays secret through [`MediaGroupEpoch`].
#[derive(Debug, Clone, PartialEq)]
pub struct MediaGroupTransition {
    pub update: Box<GroupCallUpdate>,
    pub transaction_id: u32,
    pub raw_epoch: MediaGroupEpoch,
}

/// What decided a codec switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MediaCodecDecisionSource {
    Negotiated,
    Content,
}

/// Why a call is carrying no audio.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MediaSilenceReason {
    NoDecoderForNegotiatedCodec,
    AuthenticationFailing,
    UnexpectedPayloadType,
    CodecRejectingFrames,
    CodecFlapping,
    Unknown,
}

/// One decrypted codec payload from the peer, flattened. Replaces the engine's
/// `EncodedAudioFrame`.
#[derive(Debug, Clone, PartialEq, Eq, bon::Builder)]
#[non_exhaustive]
pub struct MediaEncodedFrame {
    pub format: MediaAudioFormat,
    pub codec: MediaAudioCodec,
    pub data: Bytes,
    pub payload_type: u8,
    pub sequence_number: u16,
    pub timestamp: u32,
    pub marker: bool,
    pub sender: Option<Jid>,
    pub device: Option<Jid>,
    pub pid: Option<u32>,
}

/// One RTCP report block, flattened from the engine's `RtcpReportBlock`.
#[derive(Debug, Clone, PartialEq, Eq, bon::Builder)]
#[non_exhaustive]
pub struct MediaRtcpReportBlock {
    pub ssrc: u32,
    pub fraction_lost: u8,
    pub cumulative_lost: i32,
    pub extended_highest_sequence: u32,
    pub jitter: u32,
    pub last_sender_report: u32,
    pub delay_since_last_sender_report: u32,
    pub profile_extension: Vec<u8>,
}

/// One RTCP feedback packet, flattened from the engine's `RtcpFeedback`.
#[derive(Debug, Clone, PartialEq, Eq, bon::Builder)]
#[non_exhaustive]
pub struct MediaRtcpFeedback {
    pub packet_type: u8,
    pub fmt: u8,
    pub sender_ssrc: u32,
    pub media_ssrc: u32,
    pub fci: Vec<u8>,
}

/// A flat media-session spec. Replaces the engine's `CallConfig`.
///
/// All relay and key material rides here as plain data (`Vec<u8>`, `String`): the engine needs it to
/// derive SRTP keys and sign the STUN allocate, and a foreign backend needs it to build its own
/// transport. It is deliberately its own struct so [`Debug`] can redact the secrets in one place
/// rather than depending on the engine's redaction. The secret fields are `call_key`, `relay_token`,
/// `auth_token` and `integrity_key`; a `Debug` of this struct never prints them.
///
/// This is everything a backend needs to open the session, group media included: [`key`](Self::key)
/// carries the generational identity and `group` the optional group media inputs. A backend never
/// receives those as separate arguments.
#[derive(Clone, bon::Builder)]
#[non_exhaustive]
pub struct MediaSessionSpec {
    /// The generational identity of this session, not a bare call-id.
    pub key: MediaSessionKey,
    pub direction: MediaDirection,
    pub self_lid: String,
    pub peer_lid: String,
    /// The 32-byte callKey. Secret.
    pub call_key: Vec<u8>,
    pub ssrc: u32,
    pub audio: MediaAudioSpec,
    /// The STUN `RELAY-TOKEN` attribute. Secret.
    pub relay_token: Vec<u8>,
    /// The `<auth_token>` used to build a synthetic SDP `ice-ufrag`. Secret.
    pub auth_token: Vec<u8>,
    pub relay_ip: String,
    pub relay_port: u16,
    /// The relay `<key>` (ASCII) used as the STUN MESSAGE-INTEGRITY key. Secret.
    pub integrity_key: Vec<u8>,
    pub warp_mi_tag_len: usize,
    pub enable_media: bool,
    pub enable_video: bool,
    pub enable_sframe: bool,
    /// Group-media inputs, present only for a group call.
    pub group: Option<MediaGroupSpec>,
}

impl core::fmt::Debug for MediaSessionSpec {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MediaSessionSpec")
            .field("key", &self.key)
            .field("direction", &self.direction)
            .field("self_lid", &self.self_lid)
            .field("peer_lid", &self.peer_lid)
            .field("call_key", &"[redacted]")
            .field("ssrc", &self.ssrc)
            .field("audio", &self.audio)
            .field("relay_token", &"[redacted]")
            .field("auth_token", &"[redacted]")
            .field("relay_ip", &self.relay_ip)
            .field("relay_port", &self.relay_port)
            .field("integrity_key", &"[redacted]")
            .field("warp_mi_tag_len", &self.warp_mi_tag_len)
            .field("enable_media", &self.enable_media)
            .field("enable_video", &self.enable_video)
            .field("enable_sframe", &self.enable_sframe)
            .field("group", &self.group)
            .finish()
    }
}

/// Authenticated direct-call participant retained during an in-place group promotion.
#[derive(Clone, bon::Builder)]
#[non_exhaustive]
pub struct MediaDirectPeer {
    pub user_jid: Jid,
    pub device_jid: Jid,
    /// The direct-call callKey. Secret.
    pub call_key: Vec<u8>,
}

impl core::fmt::Debug for MediaDirectPeer {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("MediaDirectPeer")
            .field("user_jid", &self.user_jid)
            .field("device_jid", &self.device_jid)
            .field("call_key", &"[redacted]")
            .finish()
    }
}

/// Group-media inputs layered onto a regular session.
#[derive(Clone, Debug, bon::Builder)]
#[non_exhaustive]
pub struct MediaGroupSpec {
    pub call_creator: Jid,
    pub self_jid: Jid,
    pub initial_update: GroupCallUpdate,
    pub direct_peer: Option<MediaDirectPeer>,
}

/// One intent the control plane sends into the media plane. Covers the engine's `VideoControl` and
/// `GroupControl` plus the loose engine methods (`set_muted`, `rekey_recv`,
/// `switch_audio_codec`, `request_peer_keyframe`, `send_group_reaction`).
#[derive(Debug, Clone, PartialEq)]
#[non_exhaustive]
pub enum MediaCommand {
    /// Local microphone mute. The `<mute>` stanza itself stays in signaling.
    SetMuted(bool),
    /// Bring the video plane up. `awaiting_accept` gates outbound until the peer accepts.
    EnableVideo { awaiting_accept: bool },
    /// Tear the video plane down, optionally retaining legacy AUs for a reattach.
    DisableVideo { keep_legacy: bool },
    /// Stop sending video while keeping inbound decoding.
    DisableVideoOutbound,
    /// Require the next outbound access unit to be an IDR.
    RequireVideoKeyframe,
    /// Ask the peer for a keyframe by RTCP PLI.
    RequestPeerKeyframe(MediaKeyframeUrgency),
    /// The peer device's rotation (0..=3). `participant` is set for a group sender.
    SetVideoOrientation {
        participant: Option<Jid>,
        orientation: u8,
    },
    /// Select the source generation accepted by the timestamped input queue.
    SetVideoInputGeneration(u64),
    /// RTP clock increment per access unit.
    SetVideoTimestampStride(u32),
    /// Caller-only: rekey the recv path to the device that answered, and apply the audio codec its
    /// capability selected in the same step. The two travel together because rekeying decides which
    /// keys decrypt the next packet, and the codec decides how it is decoded.
    RekeyRecv {
        answering_lid: String,
        audio_codec: Option<MediaAudioCodec>,
    },
    /// Swap the audio payload grammar within the negotiated timing.
    SwitchAudioCodec {
        to: MediaAudioCodec,
        source: MediaCodecDecisionSource,
    },
    /// A newer authoritative group roster/relay snapshot.
    ApplyGroupUpdate(Box<GroupCallUpdate>),
    /// One roster snapshot and its decrypted epoch, indivisible.
    ApplyGroupTransition(MediaGroupTransition),
    /// A decrypted keygen-v2 epoch.
    ApplyGroupEpoch {
        transaction_id: u32,
        raw_epoch: MediaGroupEpoch,
    },
    /// One authenticated group reaction to broadcast.
    SendGroupReaction(String),
}

/// One event the media plane raises to the control plane. Replaces the engine-owned subset of the
/// engine's `CallEvent`.
///
/// Signaling-born events (`VideoStateChanged`, `GroupUpdated`, `WaitingRoomUpdated`, `HandRaised`,
/// `ScreenShareChanged`, `Reaction`) are named here only where they originate in the media plane:
/// the registry and handler keep producing their signaling copies.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MediaEvent {
    RelayAllocated,
    RelayAllocateFailed(u16),
    RelayAllocateTimedOut,
    RelayReconnectTimedOut,
    MediaSetupFailed(String),
    AudioSilent {
        silent_for_ms: u64,
        rtp_received: u32,
        frames_produced: u32,
        dominant_reason: MediaSilenceReason,
    },
    AudioReceptionStalled {
        silent_for_ms: u64,
    },
    AudioCodecSwitched {
        from: MediaAudioCodec,
        to: MediaAudioCodec,
        source: MediaCodecDecisionSource,
        packets_observed: u32,
    },
    AudioCodecSourceIsFixed {
        sending: MediaAudioCodec,
        peer_expects: MediaAudioCodec,
        source: MediaCodecDecisionSource,
    },
    AudioFormatMismatch {
        expected_rate: u32,
        received_rates: Vec<u32>,
    },
    OutboundMediaDropped {
        video_access_units: u32,
        packets: u32,
    },
    VideoKeyframeNeeded,
    RtcpReceived {
        packet_types: Vec<u8>,
        sender_ssrc: u32,
        referenced_ssrcs: Vec<u32>,
        reports_audio: bool,
        reports_video: bool,
        report_blocks: Vec<MediaRtcpReportBlock>,
        feedback: Vec<MediaRtcpFeedback>,
    },
    /// A decrypted standard-Opus packet a shell with its own decoder can play.
    ForeignAudio(Bytes),
    /// The same, attributed to a group participant.
    ForeignGroupAudio(MediaEncodedFrame),
    /// A committed peer video-state notification.
    PeerVideoStateChanged {
        source: Jid,
        call_creator: Jid,
        state: VideoState,
        orientation: Option<u8>,
        /// The token to pass back to accept an upgrade. `None` when signaling already resolved
        /// simultaneous requests.
        upgrade_token: Option<MediaVideoUpgradeToken>,
    },
    /// A newer authoritative group membership/relay snapshot was committed.
    GroupUpdated(Box<GroupCallUpdate>),
    /// A newer authoritative call-link admission snapshot was committed.
    WaitingRoomUpdated(Box<WaitingRoom>),
    /// Repeated waiting-room heartbeats failed.
    WaitingRoomHeartbeatFailed,
    /// One signaling/app-data control was rejected while the call stayed healthy.
    GroupControlRejected(MediaGroupControlKind),
    /// A server-requested shared epoch could not be distributed or committed locally.
    GroupRekeyFailed,
    /// One participant raised or lowered their hand.
    HandRaised {
        participant: Jid,
        raised: bool,
    },
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
        emoji: Option<String>,
        removed: bool,
    },
    /// The session closed, with why.
    Closed(MediaCloseReason),
}

/// Which group control was rejected. Replaces the engine's `GroupControlKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MediaGroupControlKind {
    Update,
    Epoch,
    Reaction,
}

/// Why a media session ended.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum MediaCloseReason {
    /// The control plane asked (hangup, terminate, reconnect).
    Local,
    /// The relay dropped.
    RelayDisconnected,
    /// A relay write failed terminally.
    SendFailed(String),
    /// Media never came up.
    SetupFailed(String),
}

/// Why a media session could not be opened.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum MediaSetupError {
    #[error("callKey too short for E2E keys (need 32 bytes)")]
    BadCallKey,
    #[error("relay endpoint is not a valid IPv4 address")]
    BadEndpoint,
    #[error("audio format contains a zero timing or channel value")]
    BadAudioFormat,
    #[error("PCM audio is supported only for mono 16 kHz / 60 ms MLOW or standard Opus")]
    UnsupportedPcmAudio,
    #[error("PCM MLOW audio requires the built-in codec")]
    MlowUnavailable,
    #[error("media session setup failed: {0}")]
    Backend(String),
}

/// Per-call media counters. Replaces the engine's `CallMediaStats`, field for field.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, bon::Builder)]
#[non_exhaustive]
pub struct MediaStats {
    #[builder(default)]
    pub rtp_received: u32,
    #[builder(default)]
    pub rtp_payload_type_unexpected: u32,
    #[builder(default)]
    pub srtp_unprotect_failed: u32,
    #[builder(default)]
    pub sframe_decrypt_failed: u32,
    #[builder(default)]
    pub audio_frames_decoded: u32,
    #[builder(default)]
    pub audio_frames_delivered: u32,
    #[builder(default)]
    pub audio_frames_concealed: u32,
    #[builder(default)]
    pub mlow_off_point_dropped: u32,
    #[builder(default)]
    pub mlow_inactive_or_sid: u32,
    #[builder(default)]
    pub foreign_frames_decoded: u32,
    #[builder(default)]
    pub audio_frames_without_decoder: u32,
    #[builder(default)]
    pub outbound_frames_without_encoder: u32,
    #[builder(default)]
    pub playout_trimmed_samples: u32,
    #[builder(default)]
    pub inbound_pipe_dropped: u32,
    #[builder(default)]
    pub audio_sink_dropped: u32,
    #[builder(default)]
    pub video_sink_dropped: u32,
    #[builder(default)]
    pub peer_keyframe_requests: u32,
    #[builder(default)]
    pub relay_packet_unclassified: u32,
    #[builder(default)]
    pub forwarding_envelope_rejected: u32,
    #[builder(default)]
    pub codec_switches: u16,
}

impl MediaStats {
    /// Audio units that actually reached a consumer, whichever I/O mode is in use.
    #[must_use]
    pub const fn audio_produced(&self) -> u32 {
        self.audio_frames_decoded
            .saturating_add(self.audio_frames_delivered)
            .saturating_add(self.foreign_frames_decoded)
    }
}

/// The boundary a media engine implements.
///
/// The executor and the relay transport are constructor state of an implementation, never fields of
/// [`MediaSessionSpec`]: they are Rust trait objects that cannot cross a process or wasm module
/// boundary, which is exactly why they cannot be part of the neutral contract.
///
/// **This contract does not yet own the session lifecycle.** In this phase the resident backend's
/// [`open`](Self::open) validates the spec against what the engine accepts and returns the error it
/// would; the live call path constructs the engine through the backend's
/// `build_engine_from_config` and keeps owning the drive task, because the socket and the runtime
/// are the facade's. Until the next phase splits `wacore::voip` into a signaling half and an engine
/// half, `reserve`/`open` are not the path a real call takes. Their doc comments say exactly what
/// they currently do, not what a future version will.
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait VoipMediaBackend: MaybeSendSync {
    /// Hand back a session for one call.
    ///
    /// The session owns the media command mailboxes for its call. The resident implementation
    /// returns a fresh, unwired session; wiring a driver's mailboxes into it is backend-specific and
    /// happens when the engine attaches. A foreign backend returns whatever session type its
    /// adapter drives.
    fn reserve(&self, call_id: &str, direction: MediaDirection) -> Arc<dyn VoipMediaSession>;

    /// Build the engine for `spec`.
    ///
    /// The resident implementation validates the spec against the engine's constructors and returns
    /// the same [`MediaSetupError`] the engine would; it does not start a drive task or attach the
    /// reserved `session`, because the caller owns the runtime and transport. See the trait doc for
    /// why the full lifecycle is the next phase. A foreign backend that owns its executor may start
    /// driving here.
    async fn open(
        &self,
        session: &Arc<dyn VoipMediaSession>,
        spec: MediaSessionSpec,
    ) -> Result<(), MediaSetupError>;
}

/// One live media session, owned by the control plane through this trait object.
///
/// The control plane no longer holds the driver's per-media mailboxes itself: the session owns them
/// and this trait is the only surface the registry and facade use. That is what makes the stored
/// field substitutable, and it is why every accessor the control plane needs is declared here
/// rather than reached through the concrete resident type.
pub trait VoipMediaSession: MaybeSendSync + 'static {
    /// Apply one control intent, returning whether the implementation accepted it.
    ///
    /// `false` is not an error: it is the same backpressure answer the driver's bounded command
    /// queues give today, and the control plane uses it to decide whether a committed signaling
    /// transition reached media. Idempotent where the engine method it replaces is.
    fn submit(&self, command: MediaCommand) -> bool;

    /// Apply a control intent that must not be lost to backpressure.
    ///
    /// Group rosters and their decrypted epochs arrive on a bounded mailbox and may be coalesced;
    /// losing the newest epoch would leave the engine unable to decrypt the latest generation. The
    /// resident implementation sheds the oldest queued entry instead, the same policy the registry
    /// applied to its own group queue. Defaults to [`Self::submit`] for an implementation with no
    /// such queue.
    fn submit_lossless(&self, command: MediaCommand) -> bool {
        self.submit(command)
    }

    /// Deliver a committed roster, retaining it for replay if media has not attached yet.
    ///
    /// `true` means the roster is either queued for the engine or safely held for the replay at
    /// attach; `false` means it could not be accepted and the committed signaling state must not be
    /// consumed.
    fn deliver_group_update(&self, update: Box<GroupCallUpdate>) -> bool {
        self.submit_lossless(MediaCommand::ApplyGroupUpdate(update))
    }

    /// Deliver a decrypted epoch, paired with the committed roster when one exists.
    ///
    /// The pairing matters: the engine rebuilds its participant key map from the roster and its
    /// epoch together, so an epoch that arrives without the roster it belongs to must not be
    /// delivered alone. A resident session with no media attached retains the epoch and reports
    /// success, and the attach-time replay pairs it with the committed roster then. Returns the
    /// actual submission result, so the control plane does not consume a signaling transaction the
    /// media plane refused.
    fn deliver_group_epoch(
        &self,
        transaction_id: u32,
        raw_epoch: MediaGroupEpoch,
        committed: Option<GroupCallUpdate>,
    ) -> bool {
        match committed {
            Some(update) => {
                self.submit_lossless(MediaCommand::ApplyGroupTransition(MediaGroupTransition {
                    update: Box::new(update),
                    transaction_id,
                    raw_epoch,
                }))
            }
            None => self.submit_lossless(MediaCommand::ApplyGroupEpoch {
                transaction_id,
                raw_epoch,
            }),
        }
    }

    /// Whether a committed roster would fit the media mailbox under the same budget the delivery
    /// uses. `is_call_link` charges an unattached call-link admission against the default slot
    /// count, matching the pre-attach reservation.
    ///
    /// This is the one non-consuming preflight the control plane needs: it commits group state and
    /// media routing together, so it has to ask the question before it commits rather than after a
    /// rejected submit. There is deliberately no generic `accepts(&MediaCommand)`: a command's
    /// readiness depends on mailbox wiring the session may install later, so advertising readiness
    /// for the whole catalogue would be a promise [`submit`](Self::submit) cannot keep.
    fn group_update_fits(&self, update: &GroupCallUpdate, is_call_link: bool) -> bool;

    /// The retained epoch transaction waiting for media to attach, if any.
    ///
    /// A decrypted epoch can land between call-scoped accept and relay attach; the control plane
    /// reads this so a later roster delivery can be paired with it instead of dropping the key.
    fn pending_group_epoch(&self) -> Option<u32> {
        None
    }

    /// Bytes of media mailbox retained, for `Client::memory_report`.
    ///
    /// A session that keeps no buffers reports zero, which is the honest answer and not a missing
    /// measurement: the resident session over-counts itself, a foreign one reports its own.
    fn retained_bytes(&self) -> usize {
        0
    }

    /// The concrete implementation, so the backend that built this session can install its own
    /// driver mailboxes into it.
    ///
    /// The control plane holds the session behind this trait, but wiring a driver's mailboxes is
    /// backend-specific. This is the one narrow downcast the resident wiring path needs; a foreign
    /// backend returns `None` and wires through its own adapter.
    fn as_any(&self) -> Option<&dyn core::any::Any> {
        None
    }

    /// Snapshot of the counters the control plane republishes for `CallHandle`.
    fn stats(&self) -> MediaStats;

    /// One subscription per call. Event delivery competes with itself the same way
    /// `CallHandle::events()` does today.
    fn subscribe(&self) -> async_channel::Receiver<MediaEvent>;

    /// Idempotent close; releases the transport.
    fn close(&self, reason: MediaCloseReason);
}
/// Blanket impls so a session behind an `Arc` is usable as one session.
impl<T: VoipMediaSession + ?Sized> VoipMediaSession for Arc<T> {
    fn submit(&self, command: MediaCommand) -> bool {
        (**self).submit(command)
    }

    fn submit_lossless(&self, command: MediaCommand) -> bool {
        (**self).submit_lossless(command)
    }

    fn deliver_group_update(&self, update: Box<GroupCallUpdate>) -> bool {
        (**self).deliver_group_update(update)
    }

    fn deliver_group_epoch(
        &self,
        transaction_id: u32,
        raw_epoch: MediaGroupEpoch,
        committed: Option<GroupCallUpdate>,
    ) -> bool {
        (**self).deliver_group_epoch(transaction_id, raw_epoch, committed)
    }

    fn group_update_fits(&self, update: &GroupCallUpdate, is_call_link: bool) -> bool {
        (**self).group_update_fits(update, is_call_link)
    }

    fn pending_group_epoch(&self) -> Option<u32> {
        (**self).pending_group_epoch()
    }

    fn retained_bytes(&self) -> usize {
        (**self).retained_bytes()
    }

    fn as_any(&self) -> Option<&dyn core::any::Any> {
        (**self).as_any()
    }

    fn stats(&self) -> MediaStats {
        (**self).stats()
    }

    fn subscribe(&self) -> async_channel::Receiver<MediaEvent> {
        (**self).subscribe()
    }

    fn close(&self, reason: MediaCloseReason) {
        (**self).close(reason)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audio_produced_covers_every_io_mode() {
        let pcm = MediaStats {
            audio_frames_decoded: 7,
            ..MediaStats::default()
        };
        let encoded = MediaStats {
            audio_frames_delivered: 9,
            ..MediaStats::default()
        };
        let rescued = MediaStats {
            foreign_frames_decoded: 4,
            ..MediaStats::default()
        };
        assert_eq!(pcm.audio_produced(), 7);
        assert_eq!(encoded.audio_produced(), 9);
        assert_eq!(rescued.audio_produced(), 4);
    }

    #[test]
    fn spec_debug_redacts_call_secrets() {
        let spec = MediaSessionSpec::builder()
            .key(
                MediaSessionKey::builder()
                    .call_id("cid".into())
                    .generation(3)
                    .build(),
            )
            .direction(MediaDirection::Incoming)
            .self_lid("1:0@lid".into())
            .peer_lid("2:0@lid".into())
            .call_key(vec![0xAB; 32])
            .ssrc(1)
            .audio(
                MediaAudioSpec::builder()
                    .format(MediaAudioFormat::MLOW_16KHZ_60MS)
                    .io(MediaAudioIo::Pcm)
                    .build(),
            )
            .relay_token(vec![0xCD; 8])
            .auth_token(vec![0xEF; 8])
            .relay_ip("127.0.0.1".into())
            .relay_port(3478)
            .integrity_key(vec![b'k'; 20])
            .warp_mi_tag_len(4)
            .enable_media(true)
            .enable_video(false)
            .enable_sframe(true)
            .build();
        let rendered = format!("{spec:?}");
        assert!(rendered.contains("[redacted]"));
        assert!(!rendered.contains("171"));
        assert!(!rendered.contains("205"));
    }

    #[test]
    fn the_session_key_is_part_of_the_identity() {
        // The call-id alone is not the identity: the registry separates same-call-id generations,
        // and the neutral key must carry that separation across the seam.
        let key = MediaSessionKey::builder()
            .call_id("ABC".into())
            .generation(12)
            .build();
        assert_eq!(key.call_id, "ABC");
        assert_eq!(key.generation, 12);
        assert_ne!(
            key,
            MediaSessionKey::builder()
                .call_id("ABC".into())
                .generation(13)
                .build()
        );
    }

    #[test]
    fn a_direct_peer_debug_redacts_its_call_key() {
        let peer = MediaDirectPeer::builder()
            .user_jid(Jid::new("1", wacore_binary::Server::Lid))
            .device_jid(Jid::new("1", wacore_binary::Server::Lid))
            .call_key(vec![0x11; 32])
            .build();
        let rendered = format!("{peer:?}");
        assert!(rendered.contains("[redacted]"));
        assert!(!rendered.contains("17"));
    }

    #[test]
    fn the_neutral_epoch_redacts_its_bytes() {
        // A decrypted group epoch is key material. It must not print, and it must not survive as a
        // plain `Vec<u8>` a log could reach; `MediaGroupEpoch` guarantees both.
        let epoch = MediaGroupEpoch::new(vec![0x5A; 32]);
        let rendered = format!("{epoch:?}");
        assert_eq!(rendered, "MediaGroupEpoch([redacted])");
        assert!(!rendered.contains("90"));
        assert_eq!(epoch.as_bytes(), &[0x5A; 32]);
    }
}

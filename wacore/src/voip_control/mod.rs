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
use crate::types::group_call::GroupCallUpdate;

// The one place the neutral contract meets the engine: the consuming conversions between the spec
// and the engine config. Kept out of this file so the compiler enforces that everything above stays
// free of `crate::voip`.
#[cfg(feature = "voip")]
pub mod engine_bridge;

// The relay-transport seam: a dumb packet pipe the platform implements. It belongs to the contract,
// not the engine, because a foreign backend reaches its own relay through it -- a `voip-control`
// build names these traits to supply a transport, and none of them touches `crate::voip`.
pub mod transport;

// A fake backend that implements only this contract, for the architectural gate. Gated so a
// shipping build carries no test scaffolding.
#[cfg(any(test, feature = "test-util"))]
pub mod fake_backend;

// Signaling/control call state -- identity, direction, lifecycle -- that carries no engine type, so
// the call flow can name a session with the engine off. `crate::voip::session` re-exports these.
pub mod signaling;

// The public, ordered call event stream. Moved out of the engine so the public API no longer names
// an engine enum; the engine re-exports it as `crate::voip::CallEvent`.
pub mod events;
pub use events::CallEvent;

// The relay `<relay>` parser: pure signaling metadata (endpoints, tokens, keys) that the control
// plane reads before any engine exists. Names only `NodeRef` and `base64`, so it belongs here.
pub mod relay_parse;

// Fundamental audio format types, moved out of the `voip`-gated audio module so the contract names
// one type and the engine re-exports it. The `voip::audio` module keeps the payload-inspecting
// helpers as a second inherent impl on `AudioFormat`.
pub mod audio_format;

// The resident media session: the neutral handle over a running call's command mailboxes. It names
// no engine type, so registry-only builds store it as `Arc<dyn VoipMediaSession>`.
pub mod resident_session;
// The call registry: active calls, generations, and the media session behind each. Names no engine
// type, so the call flow compiles with the engine off.
pub mod registry;
// The media endpoint ports (audio/video source and sink) and the video frame type, so a backend can
// be handed the platform's endpoints without the engine.
pub mod ports;
pub use ports::{AudioSink, AudioSource, EncodedAudioSink, EncodedAudioSource};
pub use ports::{TimedVideoFrame, VideoFrame, VideoSink, VideoSource};
// RTC app-data payload encoding (reactions), pure and engine-free.
pub mod app_data;
// The pure KDF/JID/varint helpers, moved here so the registry names them without the engine.
pub(crate) mod kdf;
// SSRC derivation and participant-id formatting, engine-free.
pub mod ssrc;
// Group-call membership/control state, engine-free: the server owns the roster and this is the
// transaction-ordered client view of it.
pub mod group;
// The control vocabulary a call sends into its media plane: group roster/epoch transitions,
// video-plane controls, and the recv-rekey answer, with the mailbox types that carry them.
pub mod control;

// Per-call media counters and the audio-health watchdog. Neutral: the counters are the seam's
// [`MediaStats`], and the watchdog reads a clock the shell supplies. `crate::voip::media_stats`
// re-exports this module, so the engine and a foreign backend count the same fields.
pub mod media_stats;

pub use signaling::{CallDirection, CallPhase, CallSession};

/// One decrypted keygen-v2 epoch, kept as secret material.
///
/// The engine's `GroupRawEpoch` lives in the neutral `control` module and takes these bytes; this
/// type is the public spelling, holding them in [`Zeroizing`] so they are erased when the value
/// drops, and printing `[redacted]`, so a stray `{:?}` in a log cannot leak the decrypted key. Both
/// properties matter and are why a bare `Vec<u8>` is not the type: a manual `Debug` alone leaves the
/// bytes in memory after drop, and `Clone` alone leaves a second copy that never gets erased.
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
    /// uses [`as_bytes`](Self::as_bytes). Only the resident adapter consumes it.
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

// `CallDirection` is gone: `signaling::CallDirection` is the one direction enum, and it lives on
// the neutral side already, so a separate spelling would only be a second thing to keep in sync.

// The audio format types are the moved originals (`audio_format`), not twins. The `MediaAudio*`
// names are kept as aliases for source compatibility with the already-published surface; there is
// exactly one type behind each, so the two can no longer drift.
pub use audio_format::{AudioCodec as MediaAudioCodec, AudioFormat as MediaAudioFormat};
pub use audio_format::{AudioIo as MediaAudioIo, AudioRtpProfile as MediaAudioRtpProfile};

/// Format plus I/O selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, bon::Builder)]
#[non_exhaustive]
pub struct MediaAudioSpec {
    pub format: MediaAudioFormat,
    pub io: MediaAudioIo,
}

/// Keyframe urgency for a peer-keyframe request.
///
/// The neutral vocabulary for what the engine calls `KeyframeUrgency`; the engine re-exports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MediaKeyframeUrgency {
    Coalesced,
    Immediate,
}

/// Identity of one peer video-upgrade request. Replaces the engine's `VideoUpgradeToken`, whose
/// fields are private, by a neutral `(generation, epoch)` pair an implementation can build and
/// compare. Both fields are needed: `epoch` is what distinguishes two requests in one generation.
///
/// A payload of [`MediaEvent::PeerVideoStateChanged`], therefore sealed: `#[non_exhaustive]` plus a
/// builder, so a later field does not break a consumer that destructures it. The fields stay public
/// for reading; a consumer needs both to accept an upgrade.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, bon::Builder)]
#[non_exhaustive]
pub struct MediaVideoUpgradeToken {
    pub generation: u64,
    pub epoch: u64,
}

impl MediaVideoUpgradeToken {
    /// The call generation this request belongs to.
    #[must_use]
    pub fn generation(self) -> u64 {
        self.generation
    }

    /// The per-generation request sequence number.
    ///
    /// The rotation is what distinguishes two requests in one generation: a peer can cancel and
    /// re-request, and accepting the older token would attach video for a request the peer withdrew.
    #[must_use]
    pub fn epoch(self) -> u64 {
        self.epoch
    }

    /// Rebuild the token where the fields are not directly nameable, e.g. from `(generation, epoch)`
    /// carried as a pair across a seam.
    #[must_use]
    pub fn from_parts(generation: u64, epoch: u64) -> Self {
        Self { generation, epoch }
    }
}

/// A roster snapshot plus its decrypted epoch, kept indivisible. Replaces the engine's
/// `GroupControl::Transition`, whose whole reason for existing is that the pair must not separate
/// under mailbox backpressure. The epoch stays secret through [`MediaGroupEpoch`].
///
/// A payload of [`MediaCommand::ApplyGroupTransition`], therefore sealed like every other DTO here:
/// `#[non_exhaustive]` plus a builder.
#[derive(Debug, Clone, PartialEq, bon::Builder)]
#[non_exhaustive]
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
    pub direction: CallDirection,
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

/// One event the media plane raises to the control plane.
///
/// This is the same [`CallEvent`] the public handle stream carries, not a parallel vocabulary: a
/// backend at the seam and a consumer of the call read one enum, so an event cannot mean two things.
pub use events::CallEvent as MediaEvent;

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
    /// No media backend was injected. A `voip-control`-only build compiles the call flow but ships
    /// no engine; starting media without a backend is this typed refusal rather than a panic or a
    /// silent no-op.
    #[error("no VoIP media backend is installed for this client")]
    NoBackend,
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
    /// Hand back a session for `key`.
    ///
    /// The session owns the media command mailboxes for its call. The resident implementation
    /// returns a fresh, unwired session; wiring a driver's mailboxes into it is backend-specific and
    /// happens when the engine attaches. A foreign backend returns whatever session type its
    /// adapter drives.
    ///
    /// The key, not a bare call-id: the generational identity has to be present from the first
    /// step, or a command that arrives before the engine attaches from a superseded generation of
    /// the same call-id could reach the new session. [`MediaSessionSpec::key`] carries the same
    /// key for the `open` half.
    fn reserve(&self, key: &MediaSessionKey, direction: CallDirection)
    -> Arc<dyn VoipMediaSession>;

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
            Some(update) => self.submit_lossless(MediaCommand::ApplyGroupTransition(
                MediaGroupTransition::builder()
                    .update(Box::new(update))
                    .transaction_id(transaction_id)
                    .raw_epoch(raw_epoch)
                    .build(),
            )),
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
            .direction(CallDirection::Incoming)
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
    fn group_relay_credentials_never_print() {
        // `MediaGroupSpec` and `MediaSessionSpec` derive/format `Debug`, and a group roster carries
        // a relay whose key, tokens and auth tokens are credentials. Those fields live on
        // `GroupCallRelay`, which has its own redacting `Debug`; this test pins that the redaction
        // survives the nesting so a `{:?}` of either public type cannot leak them.
        use crate::types::group_call::{
            GroupCallDevice, GroupCallParticipant, GroupCallRelay, GroupCallRelayEndpoint,
            GroupCallUpdate,
        };
        let relay = GroupCallRelay::builder()
            .uuid("uuid".to_string())
            .participant_uuid("participant".to_string())
            .attribute_padding(false)
            .key(vec![0xAB; 8])
            .tokens(vec![vec![0xCD; 8]])
            .auth_tokens(vec![vec![0xEF; 8]])
            .endpoints(vec![
                GroupCallRelayEndpoint::builder()
                    .relay_id(1)
                    .token_id(0)
                    .auth_token_id(0)
                    .relay_name("relay".to_string())
                    .is_fna(false)
                    .build(),
            ])
            .build();
        let jid = Jid::new("1", wacore_binary::Server::Lid);
        let update = GroupCallUpdate::builder()
            .call_id("cid".to_string())
            .call_creator(jid.clone())
            .transaction_id(1)
            .media("audio".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(false)
            .participants(vec![GroupCallParticipant::new(
                jid.clone(),
                vec![GroupCallDevice::new(jid.clone())],
            )])
            .relay(relay)
            .build();
        let group = MediaGroupSpec::builder()
            .call_creator(jid.clone())
            .self_jid(jid)
            .initial_update(update)
            .build();
        let rendered = format!("{group:?}");
        assert!(rendered.contains("[redacted]"));
        for secret in ["171", "205", "239"] {
            assert!(
                !rendered.contains(secret),
                "relay credential leaked: {secret}"
            );
        }
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

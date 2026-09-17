//! The resident media backend: builds the `wacore` engine from the neutral seam spec.
//!
//! This is the other half of the seam. `wacore::voip_control` declares
//! [`VoipMediaBackend`]/[`VoipMediaSession`] with no engine type in the API; this module implements
//! them on top of [`wacore::voip::CallEngine`], translating the flat [`MediaSessionSpec`] into the
//! engine's [`CallConfig`] and the engine's [`CallEvent`]s back into neutral [`MediaEvent`]s.
//!
//! The executor and the relay transport are constructor state here, never fields of the spec:
//! they are trait objects that cannot cross a process boundary, which is why the neutral contract
//! cannot carry them. The command mailboxes into a running drive task live on the resident session
//! ([`ResidentMediaSession`]), which the call registry stores.

use std::sync::Arc;

use wacore::voip::audio::{AudioCodec, AudioConfig, AudioFormat, AudioRtpProfile};
use wacore::voip::engine::{
    CallConfig, CallEngine, CallEvent, CodecDecisionSource, GroupControlKind,
};
use wacore::voip::media_session::{ResidentMediaSession, stats_to_neutral};
use wacore::voip::media_stats::AudioSilenceReason;
use wacore::voip::rtcp::{RtcpFeedback, RtcpReportBlock};
use wacore::voip::session::CallDirection;
use wacore::voip::transport::RelayEndpointParams;
use wacore::voip_control::{
    MediaAudioCodec, MediaAudioFormat, MediaAudioRtpProfile, MediaAudioSpec, MediaCloseReason,
    MediaCodecDecisionSource, MediaDirection, MediaEncodedFrame, MediaEvent, MediaGroupControlKind,
    MediaGroupSpec, MediaSetupError, MediaSilenceReason, MediaVideoUpgradeToken, VoipMediaBackend,
    VoipMediaSession,
};

/// Build the engine's [`AudioConfig`] from the neutral flat spec.
fn audio_config(spec: MediaAudioSpec) -> Result<AudioConfig, MediaSetupError> {
    AudioConfig::from_neutral(spec).ok_or(MediaSetupError::BadAudioFormat)
}

/// Project the neutral spec onto the engine config.
///
/// [`CallConfig`] is the one engine struct that is not `#[non_exhaustive]`, so this can be a field
/// assignment and the neutral spec is a direct projection of it.
pub fn call_config(
    spec: &wacore::voip_control::MediaSessionSpec,
    group: Option<&MediaGroupSpec>,
) -> Result<CallConfig, MediaSetupError> {
    let audio = audio_config(spec.audio)?;
    if spec.relay_ip.parse::<std::net::Ipv4Addr>().is_err() {
        return Err(MediaSetupError::BadEndpoint);
    }
    if spec.call_key.len() < 32 {
        return Err(MediaSetupError::BadCallKey);
    }
    let peer_lid = group
        .map(|group| group.call_creator.to_string())
        .unwrap_or_else(|| spec.peer_lid.clone());
    Ok(CallConfig {
        call_id: spec.call_id.clone(),
        direction: match spec.direction {
            MediaDirection::Outgoing => CallDirection::Outgoing,
            MediaDirection::Incoming => CallDirection::Incoming,
            // `#[non_exhaustive]`: a new direction the engine does not name cannot be dialed, and
            // refusing is the honest answer rather than guessing a side.
            _ => {
                return Err(MediaSetupError::Backend(
                    "unsupported call direction".into(),
                ));
            }
        },
        self_lid: spec.self_lid.clone(),
        peer_lid,
        call_key: spec.call_key.clone(),
        ssrc: spec.ssrc,
        audio,
        relay_token: spec.relay_token.clone(),
        auth_token: spec.auth_token.clone(),
        relay_ip: spec.relay_ip.clone(),
        relay_port: spec.relay_port,
        integrity_key: spec.integrity_key.clone(),
        warp_mi_tag_len: spec.warp_mi_tag_len,
        enable_media: spec.enable_media,
        enable_video: spec.enable_video,
        enable_sframe: spec.enable_sframe,
    })
}

/// The relay endpoint a platform transport dials, read off the engine config the relay walk
/// already resolved.
pub fn relay_endpoint(config: &CallConfig) -> Result<RelayEndpointParams, MediaSetupError> {
    let addr = format!("{}:{}", config.relay_ip, config.relay_port)
        .parse()
        .map_err(|_| MediaSetupError::BadEndpoint)?;
    Ok(RelayEndpointParams {
        addr,
        ice_ufrag: wacore::voip::relay_parse::token_to_ice_ufrag(&config.auth_token),
        ice_pwd: String::from_utf8_lossy(&config.integrity_key).into_owned(),
    })
}

/// Project an engine config onto the neutral spec, so a platform that already parsed a `<relay>`
/// can build its engine through the seam.
#[must_use]
pub fn spec_from_config(config: &CallConfig) -> wacore::voip_control::MediaSessionSpec {
    wacore::voip_control::MediaSessionSpec {
        call_id: config.call_id.clone(),
        direction: match config.direction {
            CallDirection::Outgoing => MediaDirection::Outgoing,
            CallDirection::Incoming => MediaDirection::Incoming,
            _ => MediaDirection::Incoming,
        },
        self_lid: config.self_lid.clone(),
        peer_lid: config.peer_lid.clone(),
        call_key: config.call_key.clone(),
        ssrc: config.ssrc,
        audio: config.audio.to_neutral(),
        relay_token: config.relay_token.clone(),
        auth_token: config.auth_token.clone(),
        relay_ip: config.relay_ip.clone(),
        relay_port: config.relay_port,
        integrity_key: config.integrity_key.clone(),
        warp_mi_tag_len: config.warp_mi_tag_len,
        enable_media: config.enable_media,
        enable_video: config.enable_video,
        enable_sframe: config.enable_sframe,
    }
}

/// Build the engine from an already-parsed [`CallConfig`], through the neutral spec.
///
/// This is the resident backend's entry point on the live path: the facade parses the `<relay>` and
/// decrypts the callKey exactly as before, then hands the resulting config across the seam instead
/// of reaching for `CallEngine::new` itself. The round trip is behavior-preserving because the
/// neutral spec is a field-for-field projection of [`CallConfig`].
pub fn build_engine_from_config(
    config: CallConfig,
    group: Option<MediaGroupSpec>,
    tx_ids: Box<dyn wacore::voip::engine::TxIdSource>,
) -> Result<CallEngine, MediaSetupError> {
    let spec = spec_from_config(&config);
    build_engine(&spec, group.as_ref(), tx_ids)
}

/// Give the engine the platform's standard-Opus codec, when this build has one.
///
/// A no-op without `voip-libopus`, and that is the honest outcome: the call still runs, and if the
/// peer turns out to speak Opus the engine reports `AudioSilence` with `NoDecoderForNegotiatedCodec`.
pub fn with_platform_audio_codec(engine: CallEngine) -> CallEngine {
    #[cfg(feature = "voip-libopus")]
    {
        let engine = engine
            .with_foreign_audio_codec_factory(Box::new(crate::voip::audio::LibopusCodecFactory));
        match crate::voip::audio::LibopusAudioCodec::new() {
            Ok(codec) => engine.with_foreign_audio_codec(Box::new(codec)),
            Err(e) => {
                log::warn!("voip: libopus unavailable for this call, Opus will not decode: {e}");
                engine
            }
        }
    }
    #[cfg(not(feature = "voip-libopus"))]
    engine
}

/// Build the engine from the neutral spec, optionally layering group media.
pub fn build_engine(
    spec: &wacore::voip_control::MediaSessionSpec,
    group: Option<&MediaGroupSpec>,
    tx_ids: Box<dyn wacore::voip::engine::TxIdSource>,
) -> Result<CallEngine, MediaSetupError> {
    let config = call_config(spec, group)?;
    let engine = CallEngine::new(config, tx_ids)
        .map(with_platform_audio_codec)
        .map_err(|error| MediaSetupError::Backend(error.to_string()))?;
    let Some(group) = group else {
        return Ok(engine);
    };
    let mut engine = engine;
    engine
        .configure_group(wacore::voip::engine::GroupEngineConfig {
            call_creator: group.call_creator.clone(),
            self_jid: group.self_jid.clone(),
            initial_update: group.initial_update.clone(),
            direct_peer: group
                .direct_peer
                .as_ref()
                .map(|peer| wacore::voip::engine::DirectPeer {
                    user_jid: peer.user_jid.clone(),
                    device_jid: peer.device_jid.clone(),
                    call_key: peer.call_key.clone(),
                }),
        })
        .map_err(|error| MediaSetupError::Backend(error.to_string()))?;
    Ok(engine)
}

fn codec_to_neutral(codec: AudioCodec) -> MediaAudioCodec {
    match codec {
        AudioCodec::Mlow => MediaAudioCodec::Mlow,
        AudioCodec::Opus => MediaAudioCodec::Opus,
        // `#[non_exhaustive]`: a codec added upstream maps to the closest neutral spelling.
        _ => MediaAudioCodec::Opus,
    }
}

fn rtp_profile_to_neutral(profile: AudioRtpProfile) -> MediaAudioRtpProfile {
    match profile {
        AudioRtpProfile::Mlow => MediaAudioRtpProfile::Mlow,
        AudioRtpProfile::StandardOpus => MediaAudioRtpProfile::StandardOpus,
        _ => MediaAudioRtpProfile::Mlow,
    }
}

fn format_to_neutral(format: AudioFormat) -> MediaAudioFormat {
    MediaAudioFormat {
        codec: codec_to_neutral(format.codec),
        rtp_profile: rtp_profile_to_neutral(format.rtp_profile),
        signaling_rate: format.signaling_rate,
        sample_rate: format.sample_rate,
        channels: format.channels,
        samples_per_frame: format.samples_per_frame,
        rtp_clock_rate: format.rtp_clock_rate,
        rtp_timestamp_step: format.rtp_timestamp_step,
        rtp_payload_type: format.rtp_payload_type,
    }
}

fn decision_to_neutral(source: CodecDecisionSource) -> MediaCodecDecisionSource {
    match source {
        CodecDecisionSource::Negotiated => MediaCodecDecisionSource::Negotiated,
        CodecDecisionSource::Content => MediaCodecDecisionSource::Content,
        _ => MediaCodecDecisionSource::Negotiated,
    }
}

fn silence_to_neutral(reason: AudioSilenceReason) -> MediaSilenceReason {
    match reason {
        AudioSilenceReason::NoDecoderForNegotiatedCodec => {
            MediaSilenceReason::NoDecoderForNegotiatedCodec
        }
        AudioSilenceReason::AuthenticationFailing => MediaSilenceReason::AuthenticationFailing,
        AudioSilenceReason::UnexpectedPayloadType => MediaSilenceReason::UnexpectedPayloadType,
        AudioSilenceReason::CodecRejectingFrames => MediaSilenceReason::CodecRejectingFrames,
        AudioSilenceReason::CodecFlapping => MediaSilenceReason::CodecFlapping,
        // `#[non_exhaustive]`: a reason added upstream must still translate.
        _ => MediaSilenceReason::Unknown,
    }
}

fn group_kind_to_neutral(control: GroupControlKind) -> MediaGroupControlKind {
    match control {
        GroupControlKind::Update => MediaGroupControlKind::Update,
        GroupControlKind::Epoch => MediaGroupControlKind::Epoch,
        GroupControlKind::Reaction => MediaGroupControlKind::Reaction,
        // `#[non_exhaustive]`: a control added upstream maps to the closest neutral spelling.
        _ => MediaGroupControlKind::Update,
    }
}

fn rtcp_block_to_neutral(block: RtcpReportBlock) -> wacore::voip_control::MediaRtcpReportBlock {
    wacore::voip_control::MediaRtcpReportBlock {
        ssrc: block.ssrc,
        fraction_lost: block.fraction_lost,
        cumulative_lost: block.cumulative_lost,
        extended_highest_sequence: block.extended_highest_sequence,
        jitter: block.jitter,
        last_sender_report: block.last_sender_report,
        delay_since_last_sender_report: block.delay_since_last_sender_report,
        profile_extension: block.profile_extension,
    }
}

fn rtcp_feedback_to_neutral(feedback: RtcpFeedback) -> wacore::voip_control::MediaRtcpFeedback {
    wacore::voip_control::MediaRtcpFeedback {
        packet_type: feedback.packet_type,
        fmt: feedback.fmt,
        sender_ssrc: feedback.sender_ssrc,
        media_ssrc: feedback.media_ssrc,
        fci: feedback.fci,
    }
}

fn frame_to_neutral(frame: wacore::voip::EncodedAudioFrame) -> MediaEncodedFrame {
    MediaEncodedFrame {
        format: format_to_neutral(frame.format),
        codec: codec_to_neutral(frame.codec),
        data: frame.data,
        payload_type: frame.payload_type,
        sequence_number: frame.sequence_number,
        timestamp: frame.timestamp,
        marker: frame.marker,
        sender: frame.sender,
        device: frame.device,
        pid: frame.pid,
    }
}

/// Total translation of the engine's event enum into the neutral one.
///
/// The engine enum is `#[non_exhaustive]`, so the trailing arm is required by the compiler even
/// with every current variant handled. It does not swallow a future variant silently: it surfaces
/// as a terminal [`MediaEvent::Closed`], so an untranslated event is observable rather than lost.
pub fn translate_event(event: CallEvent) -> MediaEvent {
    match event {
        CallEvent::RelayAllocated => MediaEvent::RelayAllocated,
        CallEvent::RelayAllocateFailed(code) => MediaEvent::RelayAllocateFailed(code),
        CallEvent::RelayAllocateTimedOut => MediaEvent::RelayAllocateTimedOut,
        CallEvent::RelayReconnectTimedOut => MediaEvent::RelayReconnectTimedOut,
        CallEvent::MediaSetupFailed(reason) => MediaEvent::MediaSetupFailed(reason),
        CallEvent::AudioSilent {
            silent_for_ms,
            rtp_received,
            frames_produced,
            dominant_reason,
        } => MediaEvent::AudioSilent {
            silent_for_ms,
            rtp_received,
            frames_produced,
            dominant_reason: silence_to_neutral(dominant_reason),
        },
        CallEvent::AudioReceptionStalled { silent_for_ms } => {
            MediaEvent::AudioReceptionStalled { silent_for_ms }
        }
        CallEvent::AudioCodecSwitched {
            from,
            to,
            source,
            packets_observed,
        } => MediaEvent::AudioCodecSwitched {
            from: codec_to_neutral(from),
            to: codec_to_neutral(to),
            source: decision_to_neutral(source),
            packets_observed,
        },
        CallEvent::AudioCodecSourceIsFixed {
            sending,
            peer_expects,
            source,
        } => MediaEvent::AudioCodecSourceIsFixed {
            sending: codec_to_neutral(sending),
            peer_expects: codec_to_neutral(peer_expects),
            source: decision_to_neutral(source),
        },
        CallEvent::AudioFormatMismatch {
            expected_rate,
            received_rates,
        } => MediaEvent::AudioFormatMismatch {
            expected_rate,
            received_rates,
        },
        CallEvent::OutboundMediaDropped {
            video_access_units,
            packets,
        } => MediaEvent::OutboundMediaDropped {
            video_access_units,
            packets,
        },
        CallEvent::VideoKeyframeNeeded => MediaEvent::VideoKeyframeNeeded,
        CallEvent::RtcpReceived {
            packet_types,
            sender_ssrc,
            referenced_ssrcs,
            reports_audio,
            reports_video,
            report_blocks,
            feedback,
        } => MediaEvent::RtcpReceived {
            packet_types,
            sender_ssrc,
            referenced_ssrcs,
            reports_audio,
            reports_video,
            report_blocks: report_blocks
                .into_iter()
                .map(rtcp_block_to_neutral)
                .collect(),
            feedback: feedback.into_iter().map(rtcp_feedback_to_neutral).collect(),
        },
        CallEvent::ForeignAudio(data) => MediaEvent::ForeignAudio(data),
        CallEvent::ForeignGroupAudio(frame) => {
            MediaEvent::ForeignGroupAudio(frame_to_neutral(frame))
        }
        CallEvent::PeerVideoStateChanged {
            source,
            call_creator,
            state,
            orientation,
            upgrade_token,
        } => MediaEvent::PeerVideoStateChanged {
            source,
            call_creator,
            state,
            orientation,
            // `epoch` is what distinguishes two requests in one generation, and
            // [`VideoUpgradeToken::epoch`] is now public, so the neutral token is complete.
            upgrade_token: upgrade_token.map(|token| MediaVideoUpgradeToken {
                generation: token.generation(),
                epoch: token.epoch(),
            }),
        },
        CallEvent::GroupUpdated(update) => MediaEvent::GroupUpdated(update),
        CallEvent::WaitingRoomUpdated(room) => MediaEvent::WaitingRoomUpdated(room),
        CallEvent::WaitingRoomHeartbeatFailed => MediaEvent::WaitingRoomHeartbeatFailed,
        CallEvent::GroupControlRejected { control } => {
            MediaEvent::GroupControlRejected(group_kind_to_neutral(control))
        }
        CallEvent::GroupRekeyFailed => MediaEvent::GroupRekeyFailed,
        CallEvent::HandRaised {
            participant,
            raised,
        } => MediaEvent::HandRaised {
            participant,
            raised,
        },
        CallEvent::ScreenShareChanged {
            participant,
            screen_share,
        } => MediaEvent::ScreenShareChanged {
            participant,
            screen_share,
        },
        CallEvent::Reaction {
            participant,
            device,
            pid,
            emoji,
            removed,
        } => MediaEvent::Reaction {
            participant,
            device,
            pid,
            emoji,
            removed,
        },
        // The compatibility spelling of `PeerVideoStateChanged`; the neutral seam carries only the
        // identity-aware one.
        CallEvent::VideoStateChanged { .. } => MediaEvent::VideoKeyframeNeeded,
        other => MediaEvent::Closed(MediaCloseReason::SetupFailed(format!(
            "untranslated engine event: {other:?}"
        ))),
    }
}

/// The resident backend.
///
/// It owns no per-call state: a session is reserved per call and carries the engine's mailboxes.
/// The executor and transport belong to the facade that drives `run_call`, which is why
/// [`open`](VoipMediaBackend::open) here only builds the engine from the spec and hands it back
/// through the session's own wiring; the drive loop stays where the socket and runtime are.
#[derive(Default)]
pub struct WacoreVoipMediaBackend;

impl WacoreVoipMediaBackend {
    #[must_use]
    pub fn new() -> Self {
        Self
    }

    /// Reserve a resident session with a fresh counters cell.
    #[must_use]
    pub fn reserve_session(&self) -> Arc<ResidentMediaSession> {
        ResidentMediaSession::new()
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl VoipMediaBackend for WacoreVoipMediaBackend {
    fn reserve(&self, _call_id: &str, _direction: MediaDirection) -> Arc<dyn VoipMediaSession> {
        ResidentMediaSession::new()
    }

    /// Validate the spec against the engine's constructors.
    ///
    /// The engine itself already exists on a real call: the facade builds it and owns the drive
    /// loop, which is where the socket and runtime are. This checks that the spec the seam carries
    /// is one the engine accepts, and fails with the same [`MediaSetupError`] the engine would.
    async fn open(
        &self,
        _session: &Arc<dyn VoipMediaSession>,
        spec: wacore::voip_control::MediaSessionSpec,
    ) -> Result<(), MediaSetupError> {
        build_engine(
            &spec,
            None,
            Box::new(wacore::voip::engine::SequentialTxIds::new()),
        )
        .map(|_| ())
    }
}

/// The neutral counters for an engine snapshot, re-exported so the facade can publish them.
#[must_use]
pub fn neutral_stats(stats: wacore::voip::CallMediaStats) -> wacore::voip_control::MediaStats {
    stats_to_neutral(stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wacore::voip_control::{MediaAudioIo, MediaSessionSpec};

    fn spec() -> MediaSessionSpec {
        MediaSessionSpec {
            call_id: "SEAM-1".into(),
            direction: MediaDirection::Incoming,
            self_lid: "1:0@lid".into(),
            peer_lid: "2:0@lid".into(),
            call_key: vec![0u8; 32],
            ssrc: 7,
            audio: MediaAudioSpec {
                format: MediaAudioFormat::MLOW_16KHZ_60MS,
                io: MediaAudioIo::Pcm,
            },
            relay_token: vec![1, 2, 3],
            auth_token: vec![],
            relay_ip: "127.0.0.1".into(),
            relay_port: 3478,
            integrity_key: b"key".into(),
            warp_mi_tag_len: 4,
            enable_media: false,
            enable_video: false,
            enable_sframe: false,
        }
    }

    #[test]
    fn the_engine_builds_from_the_neutral_spec() {
        let engine = build_engine(
            &spec(),
            None,
            Box::new(wacore::voip::engine::SequentialTxIds::new()),
        )
        .expect("the neutral spec builds an engine");
        assert_eq!(engine.call_id(), "SEAM-1");
    }

    #[test]
    fn a_short_call_key_is_refused_without_building() {
        let mut bad = spec();
        bad.call_key = vec![0u8; 8];
        assert_eq!(
            build_engine(
                &bad,
                None,
                Box::new(wacore::voip::engine::SequentialTxIds::new())
            )
            .err(),
            Some(MediaSetupError::BadCallKey)
        );
    }

    #[test]
    fn a_zero_timing_format_is_refused() {
        let mut bad = spec();
        bad.audio.format.sample_rate = 0;
        assert_eq!(
            build_engine(
                &bad,
                None,
                Box::new(wacore::voip::engine::SequentialTxIds::new())
            )
            .err(),
            Some(MediaSetupError::BadAudioFormat)
        );
    }

    #[test]
    fn every_engine_event_shape_translates() {
        // A representative from each shape the engine emits. The match is total, and the trailing
        // arm surfaces an unhandled upstream variant as a terminal event rather than a silent drop.
        assert_eq!(
            translate_event(CallEvent::RelayAllocated),
            MediaEvent::RelayAllocated
        );
        assert_eq!(
            translate_event(CallEvent::AudioSilent {
                silent_for_ms: 1,
                rtp_received: 2,
                frames_produced: 3,
                dominant_reason: AudioSilenceReason::Unknown,
            }),
            MediaEvent::AudioSilent {
                silent_for_ms: 1,
                rtp_received: 2,
                frames_produced: 3,
                dominant_reason: MediaSilenceReason::Unknown,
            }
        );
        assert!(matches!(
            translate_event(CallEvent::RelayAllocateFailed(486)),
            MediaEvent::RelayAllocateFailed(486)
        ));
        assert_eq!(
            translate_event(CallEvent::VideoKeyframeNeeded),
            MediaEvent::VideoKeyframeNeeded
        );
    }
}

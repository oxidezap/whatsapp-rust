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

use wacore::voip::audio::{AudioCodec, AudioFormat, AudioRtpProfile};
use wacore::voip::engine::{
    CallConfig, CallEngine, CallEvent, CodecDecisionSource, GroupControlKind,
};
use wacore::voip::media_session::{ResidentMediaSession, stats_to_neutral};
use wacore::voip::media_stats::AudioSilenceReason;
use wacore::voip::rtcp::{RtcpFeedback, RtcpReportBlock};
use wacore::voip::transport::RelayEndpointParams;
use wacore::voip_control::{
    MediaAudioCodec, MediaAudioFormat, MediaAudioRtpProfile, MediaCodecDecisionSource,
    MediaDirection, MediaEncodedFrame, MediaEvent, MediaGroupControlKind, MediaGroupSpec,
    MediaSessionSpec, MediaSetupError, MediaSilenceReason, MediaVideoUpgradeToken,
    VoipMediaBackend, VoipMediaSession,
};

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

/// Build the engine from an already-parsed [`CallConfig`], through the neutral spec.
///
/// This is the resident backend's entry point on the live path: the facade parses the `<relay>` and
/// decrypts the callKey exactly as before, then hands the resulting config across the seam instead
/// of reaching for `CallEngine::new` itself. `generation` is the registration token the call
/// registry assigned, carried into the neutral [`MediaSessionSpec::key`] so a foreign backend can
/// reject a message from a superseded generation.
pub fn build_engine_from_config(
    config: CallConfig,
    generation: u64,
    group: Option<MediaGroupSpec>,
    tx_ids: Box<dyn wacore::voip::engine::TxIdSource>,
) -> Result<CallEngine, MediaSetupError> {
    let mut spec = MediaSessionSpec::try_from((config, generation))?;
    spec.group = group;
    build_engine(spec, tx_ids)
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

/// Build the engine from the neutral spec, applying the group media the spec carries.
pub fn build_engine(
    mut spec: MediaSessionSpec,
    tx_ids: Box<dyn wacore::voip::engine::TxIdSource>,
) -> Result<CallEngine, MediaSetupError> {
    // Take the group before the config projection consumes the spec; it is applied to the engine
    // after construction, not carried by `CallConfig`.
    let group = spec.group.take();
    let config = CallConfig::try_from(spec)?;
    let engine = CallEngine::new(config, tx_ids)
        .map(with_platform_audio_codec)
        .map_err(|error| MediaSetupError::Backend(error.to_string()))?;
    let Some(group) = group else {
        return Ok(engine);
    };
    let mut engine = engine;
    engine
        .configure_group(wacore::voip::engine::GroupEngineConfig {
            call_creator: group.call_creator,
            self_jid: group.self_jid,
            initial_update: group.initial_update,
            direct_peer: group
                .direct_peer
                .map(|peer| wacore::voip::engine::DirectPeer {
                    user_jid: peer.user_jid,
                    device_jid: peer.device_jid,
                    call_key: peer.call_key,
                }),
        })
        .map_err(|error| MediaSetupError::Backend(error.to_string()))?;
    Ok(engine)
}

/// Translate an engine codec, or `None` when the neutral seam does not name it.
///
/// Both enums are `#[non_exhaustive]`. A codec added upstream has no neutral spelling, and picking
/// the nearest one would claim a decode the seam cannot describe; refusing lets the caller drop the
/// event instead.
fn codec_to_neutral(codec: AudioCodec) -> Option<MediaAudioCodec> {
    match codec {
        AudioCodec::Mlow => Some(MediaAudioCodec::Mlow),
        AudioCodec::Opus => Some(MediaAudioCodec::Opus),
        _ => None,
    }
}

fn rtp_profile_to_neutral(profile: AudioRtpProfile) -> Option<MediaAudioRtpProfile> {
    match profile {
        AudioRtpProfile::Mlow => Some(MediaAudioRtpProfile::Mlow),
        AudioRtpProfile::StandardOpus => Some(MediaAudioRtpProfile::StandardOpus),
        _ => None,
    }
}

fn format_to_neutral(format: AudioFormat) -> Option<MediaAudioFormat> {
    Some(
        MediaAudioFormat::builder()
            .codec(codec_to_neutral(format.codec)?)
            .rtp_profile(rtp_profile_to_neutral(format.rtp_profile)?)
            .signaling_rate(format.signaling_rate)
            .sample_rate(format.sample_rate)
            .channels(format.channels)
            .samples_per_frame(format.samples_per_frame)
            .rtp_clock_rate(format.rtp_clock_rate)
            .rtp_timestamp_step(format.rtp_timestamp_step)
            .rtp_payload_type(format.rtp_payload_type)
            .build(),
    )
}

fn decision_to_neutral(source: CodecDecisionSource) -> Option<MediaCodecDecisionSource> {
    match source {
        CodecDecisionSource::Negotiated => Some(MediaCodecDecisionSource::Negotiated),
        CodecDecisionSource::Content => Some(MediaCodecDecisionSource::Content),
        _ => None,
    }
}

fn silence_to_neutral(reason: AudioSilenceReason) -> Option<MediaSilenceReason> {
    match reason {
        AudioSilenceReason::NoDecoderForNegotiatedCodec => {
            Some(MediaSilenceReason::NoDecoderForNegotiatedCodec)
        }
        AudioSilenceReason::AuthenticationFailing => {
            Some(MediaSilenceReason::AuthenticationFailing)
        }
        AudioSilenceReason::UnexpectedPayloadType => {
            Some(MediaSilenceReason::UnexpectedPayloadType)
        }
        AudioSilenceReason::CodecRejectingFrames => Some(MediaSilenceReason::CodecRejectingFrames),
        AudioSilenceReason::CodecFlapping => Some(MediaSilenceReason::CodecFlapping),
        AudioSilenceReason::Unknown => Some(MediaSilenceReason::Unknown),
        _ => None,
    }
}

fn group_kind_to_neutral(control: GroupControlKind) -> Option<MediaGroupControlKind> {
    match control {
        GroupControlKind::Update => Some(MediaGroupControlKind::Update),
        GroupControlKind::Epoch => Some(MediaGroupControlKind::Epoch),
        GroupControlKind::Reaction => Some(MediaGroupControlKind::Reaction),
        _ => None,
    }
}

fn rtcp_block_to_neutral(block: RtcpReportBlock) -> wacore::voip_control::MediaRtcpReportBlock {
    wacore::voip_control::MediaRtcpReportBlock::builder()
        .ssrc(block.ssrc)
        .fraction_lost(block.fraction_lost)
        .cumulative_lost(block.cumulative_lost)
        .extended_highest_sequence(block.extended_highest_sequence)
        .jitter(block.jitter)
        .last_sender_report(block.last_sender_report)
        .delay_since_last_sender_report(block.delay_since_last_sender_report)
        .profile_extension(block.profile_extension)
        .build()
}

fn rtcp_feedback_to_neutral(feedback: RtcpFeedback) -> wacore::voip_control::MediaRtcpFeedback {
    wacore::voip_control::MediaRtcpFeedback::builder()
        .packet_type(feedback.packet_type)
        .fmt(feedback.fmt)
        .sender_ssrc(feedback.sender_ssrc)
        .media_ssrc(feedback.media_ssrc)
        .fci(feedback.fci)
        .build()
}

fn frame_to_neutral(frame: wacore::voip::EncodedAudioFrame) -> Option<MediaEncodedFrame> {
    Some(
        MediaEncodedFrame::builder()
            .format(format_to_neutral(frame.format)?)
            .codec(codec_to_neutral(frame.codec)?)
            .data(frame.data)
            .payload_type(frame.payload_type)
            .sequence_number(frame.sequence_number)
            .timestamp(frame.timestamp)
            .marker(frame.marker)
            .maybe_sender(frame.sender)
            .maybe_device(frame.device)
            .maybe_pid(frame.pid)
            .build(),
    )
}

/// Translate an engine event into the neutral one, or `None` when the seam does not carry it.
///
/// The engine enum is `#[non_exhaustive]`, so a trailing arm is required by the compiler even with
/// every current variant handled. Three things return `None`, and none is fatal to a healthy call:
///
/// - A future variant the compiler has not seen. Logged and dropped, never surfaced as a terminal
///   `Closed`, because an event the adapter does not model is not a reason to tear down media.
/// - A known variant whose payload names a known-unknown enum value (a codec, silence reason, or
///   group-control kind added upstream). Inventing the nearest neutral spelling would claim a fact
///   the seam cannot describe, so the event is dropped.
/// - The compatibility spelling `VideoStateChanged`, which the seam deliberately does not carry.
pub fn translate_event(event: CallEvent) -> Option<MediaEvent> {
    let neutral = match event {
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
            dominant_reason: silence_to_neutral(dominant_reason)?,
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
            from: codec_to_neutral(from)?,
            to: codec_to_neutral(to)?,
            source: decision_to_neutral(source)?,
            packets_observed,
        },
        CallEvent::AudioCodecSourceIsFixed {
            sending,
            peer_expects,
            source,
        } => MediaEvent::AudioCodecSourceIsFixed {
            sending: codec_to_neutral(sending)?,
            peer_expects: codec_to_neutral(peer_expects)?,
            source: decision_to_neutral(source)?,
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
            MediaEvent::ForeignGroupAudio(frame_to_neutral(frame)?)
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
            upgrade_token: upgrade_token.map(|token| {
                MediaVideoUpgradeToken::builder()
                    .generation(token.generation())
                    .epoch(token.epoch())
                    .build()
            }),
        },
        CallEvent::GroupUpdated(update) => MediaEvent::GroupUpdated(update),
        CallEvent::WaitingRoomUpdated(room) => MediaEvent::WaitingRoomUpdated(room),
        CallEvent::WaitingRoomHeartbeatFailed => MediaEvent::WaitingRoomHeartbeatFailed,
        CallEvent::GroupControlRejected { control } => {
            MediaEvent::GroupControlRejected(group_kind_to_neutral(control)?)
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
        // The compatibility spelling of `PeerVideoStateChanged`. The neutral seam carries only the
        // identity-aware event, so this one is dropped rather than mapped onto something else.
        // Mapping it to `VideoKeyframeNeeded` (as an earlier draft did) fabricates an outbound IDR
        // request from a peer-state notification, which are unrelated facts: the peer toggling
        // video is not our encoder needing a keyframe.
        CallEvent::VideoStateChanged { .. } => return None,
        other => {
            // `#[non_exhaustive]`: an engine event the seam does not model yet. Log it and drop it;
            // never end a healthy call because the adapter is behind the engine.
            log::debug!("voip: engine event has no neutral spelling, ignored: {other:?}");
            return None;
        }
    };
    Some(neutral)
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
    /// This is not the live path yet. On a real call the facade parses the `<relay>`, builds the
    /// engine through [`build_engine_from_config`], and owns the drive task, because the socket and
    /// the runtime are the facade's to supply. What this does is prove the neutral spec is one the
    /// engine accepts, failing with the same [`MediaSetupError`] the engine would, so a foreign
    /// backend author has a checked example. Wiring `reserve`/`open` into a full lifecycle is the
    /// next phase, when `wacore::voip` splits into a signaling half and an engine half.
    async fn open(
        &self,
        _session: &Arc<dyn VoipMediaSession>,
        spec: MediaSessionSpec,
    ) -> Result<(), MediaSetupError> {
        build_engine(spec, Box::new(wacore::voip::engine::SequentialTxIds::new())).map(|_| ())
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
    use wacore::voip_control::{MediaAudioIo, MediaAudioSpec, MediaSessionKey};

    fn spec() -> MediaSessionSpec {
        MediaSessionSpec::builder()
            .key(
                MediaSessionKey::builder()
                    .call_id("SEAM-1".into())
                    .generation(1)
                    .build(),
            )
            .direction(MediaDirection::Incoming)
            .self_lid("1:0@lid".into())
            .peer_lid("2:0@lid".into())
            .call_key(vec![0u8; 32])
            .ssrc(7)
            .audio(
                MediaAudioSpec::builder()
                    .format(MediaAudioFormat::MLOW_16KHZ_60MS)
                    .io(MediaAudioIo::Pcm)
                    .build(),
            )
            .relay_token(vec![1, 2, 3])
            .auth_token(vec![])
            .relay_ip("127.0.0.1".into())
            .relay_port(3478)
            .integrity_key(b"key".into())
            .warp_mi_tag_len(4)
            .enable_media(false)
            .enable_video(false)
            .enable_sframe(false)
            .build()
    }

    fn tx_ids() -> Box<dyn wacore::voip::engine::TxIdSource> {
        Box::new(wacore::voip::engine::SequentialTxIds::new())
    }

    #[test]
    fn the_engine_builds_from_the_neutral_spec() {
        let engine = build_engine(spec(), tx_ids()).expect("the neutral spec builds an engine");
        assert_eq!(engine.call_id(), "SEAM-1");
    }

    #[test]
    fn the_spec_carries_the_generational_identity() {
        // The registry distinguishes `call ABC gen 12` from `gen 13`; the neutral spec must too, or
        // a foreign backend keyed on the call-id alone could route a stale generation into the new
        // session. The key survives the config projection.
        let spec = spec();
        let key = spec.key.clone();
        assert_eq!(key.call_id, "SEAM-1");
        assert_eq!(key.generation, 1);
        let config = CallConfig::try_from(spec).expect("the spec projects onto a config");
        assert_eq!(config.call_id, "SEAM-1");
    }

    #[test]
    fn a_group_spec_rides_the_session_spec_and_configures_the_engine() {
        // Item 3: a foreign backend cannot open a group call without the group inputs, so they ride
        // the spec rather than a separate `open` parameter. The roster carries a relay and the local
        // participant is connected, so `configure_group` accepts it and the engine reports a group.
        let group = MediaGroupSpec::builder()
            .call_creator(wacore_binary::Jid::new("1", wacore_binary::Server::Lid))
            .self_jid(wacore_binary::Jid::new("1", wacore_binary::Server::Lid))
            .initial_update(group_update())
            .build();
        let mut spec = spec();
        spec.enable_media = true;
        spec.group = Some(group);
        let engine = build_engine(spec, tx_ids()).expect("a group spec builds a group engine");
        assert!(engine.is_group());
    }

    fn group_update() -> wacore::types::group_call::GroupCallUpdate {
        use wacore::types::group_call::{
            GroupCallDevice, GroupCallParticipant, GroupCallRelay, GroupCallRelayEndpoint,
            GroupCallUpdate,
        };
        let jid = wacore_binary::Jid::new("1", wacore_binary::Server::Lid);
        let mut local =
            GroupCallParticipant::new(jid.clone(), vec![GroupCallDevice::new(jid.clone())]);
        local.state = Some("connected".to_string());
        let relay = GroupCallRelay::builder()
            .transaction_id(1)
            .self_pid(1)
            .uuid("relay".to_string())
            .participant_uuid("participant".to_string())
            .attribute_padding(false)
            .warp_mi_tag_len(4)
            .key(b"relay-key".to_vec())
            .tokens(vec![vec![0x47]])
            .auth_tokens(vec![vec![0x57]])
            .endpoints(vec![
                GroupCallRelayEndpoint::builder()
                    .relay_id(1)
                    .token_id(0)
                    .auth_token_id(0)
                    .relay_name("relay-1".to_string())
                    .is_fna(false)
                    .ipv4("203.0.113.7".to_string())
                    .port(3480)
                    .build(),
            ])
            .build();
        GroupCallUpdate::builder()
            .call_id("SEAM-1".to_string())
            .call_creator(jid)
            .transaction_id(1)
            .media("audio".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(false)
            .participants(vec![local])
            .relay(relay)
            .build()
    }

    #[test]
    fn a_short_call_key_is_refused_without_building() {
        let mut bad = spec();
        bad.call_key = vec![0u8; 8];
        assert_eq!(
            build_engine(bad, tx_ids()).err(),
            Some(MediaSetupError::BadCallKey)
        );
    }

    #[test]
    fn a_zero_timing_format_is_refused() {
        let mut bad = spec();
        bad.audio.format.sample_rate = 0;
        assert_eq!(
            build_engine(bad, tx_ids()).err(),
            Some(MediaSetupError::BadAudioFormat)
        );
    }

    #[test]
    fn every_engine_event_shape_translates() {
        // A representative from each shape the engine emits.
        assert_eq!(
            translate_event(CallEvent::RelayAllocated),
            Some(MediaEvent::RelayAllocated)
        );
        assert_eq!(
            translate_event(CallEvent::AudioSilent {
                silent_for_ms: 1,
                rtp_received: 2,
                frames_produced: 3,
                dominant_reason: AudioSilenceReason::Unknown,
            }),
            Some(MediaEvent::AudioSilent {
                silent_for_ms: 1,
                rtp_received: 2,
                frames_produced: 3,
                dominant_reason: MediaSilenceReason::Unknown,
            })
        );
        assert!(matches!(
            translate_event(CallEvent::RelayAllocateFailed(486)),
            Some(MediaEvent::RelayAllocateFailed(486))
        ));
        assert_eq!(
            translate_event(CallEvent::VideoKeyframeNeeded),
            Some(MediaEvent::VideoKeyframeNeeded)
        );
    }

    #[test]
    fn the_video_state_compatibility_event_is_dropped_not_mapped_to_a_keyframe() {
        // A peer toggling video is not our encoder needing an IDR. The seam carries only the
        // identity-aware `PeerVideoStateChanged`, so the compatibility spelling is dropped.
        assert_eq!(
            translate_event(CallEvent::VideoStateChanged {
                state: wacore::types::call::VideoState::Enabled,
                orientation: None,
                upgrade_token: None,
            }),
            None
        );
    }
}

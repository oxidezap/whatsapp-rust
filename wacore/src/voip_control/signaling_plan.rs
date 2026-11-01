//! Shared call signaling decisions, independent of the authenticated client and media engine.

use super::{
    MediaAudioCodec as AudioCodec, MediaAudioFormat as AudioFormat,
    MediaAudioRtpProfile as AudioRtpProfile,
};
use crate::stanza::call::{
    AcceptParams, CAPABILITY_OFFER, CAPABILITY_PREACCEPT, CAPABILITY_STANDARD_OPUS_OFFER,
    CAPABILITY_STANDARD_OPUS_PREACCEPT, CAPABILITY_STANDARD_OPUS_VIDEO_OFFER,
    CAPABILITY_VIDEO_OFFER, build_accept, build_preaccept_with_capability,
    standard_opus_voip_settings,
};
use crate::types::call::{CallAction, IncomingCall};
use wacore_binary::{Jid, Server};

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum AudioPlanError {
    #[error("encoded audio codec {configured:?} cannot satisfy negotiated codec {selected:?}")]
    EncodedAudioCodecNotNegotiated {
        configured: AudioCodec,
        selected: AudioCodec,
    },
}

pub fn offer_capability(video: bool, audio: AudioFormat) -> &'static [u8] {
    let standard_opus = matches!(audio.rtp_profile, AudioRtpProfile::StandardOpus);
    match (video, standard_opus) {
        (true, true) => &CAPABILITY_STANDARD_OPUS_VIDEO_OFFER,
        (false, true) => &CAPABILITY_STANDARD_OPUS_OFFER,
        (true, false) => &CAPABILITY_VIDEO_OFFER,
        (false, false) => &CAPABILITY_OFFER,
    }
}

/// Keep the application's encoded source grammar distinct from the negotiated wire format.
/// PCM endpoints can change codec; encoded endpoints can only change compatible containers.
#[derive(Debug, PartialEq, Eq)]
pub struct NegotiatedAudioPlan {
    /// The source format the engine receives from the application.
    pub engine_format: AudioFormat,
    /// What the peer is told, and what the wire actually carries.
    pub wire_format: AudioFormat,
    /// Set when the engine must be moved off the format it was built with, before media flows.
    pub engine_switch: Option<AudioCodec>,
}

pub fn negotiated_audio_plan(
    configured: AudioFormat,
    encoded_endpoint: bool,
    selected: Option<AudioCodec>,
) -> Result<NegotiatedAudioPlan, AudioPlanError> {
    let unchanged = NegotiatedAudioPlan {
        engine_format: configured,
        wire_format: configured,
        engine_switch: None,
    };
    let Some(codec) = selected else {
        return Ok(unchanged);
    };
    // A codec with no sibling at this timing cannot be reached without re-signalling, so the call
    // stays as configured rather than silently changing what the peer was told.
    let Some(format) = configured.sibling_for(codec) else {
        return Ok(unchanged);
    };
    // Only a change of CODEC is unanswerable: the source emits one grammar and cannot be told to
    // emit another, so the application has to pick again. A change of container is answerable --
    // the escape profile and native Opus carry the same Opus bytes, and the engine is what stops
    // rewriting their TOCs.
    if encoded_endpoint && configured.codec != codec {
        return Err(AudioPlanError::EncodedAudioCodecNotNegotiated {
            configured: configured.codec,
            selected: codec,
        });
    }
    Ok(NegotiatedAudioPlan {
        engine_format: if encoded_endpoint { configured } else { format },
        wire_format: format,
        engine_switch: encoded_endpoint.then_some(codec),
    })
}

pub fn peer_selected_codec_from_offer(
    incoming: &IncomingCall,
    format: AudioFormat,
) -> Option<AudioCodec> {
    use crate::stanza::call::{CAPABILITY_INDEX_MLOW_V1, CapabilityBit, capability_bit};
    use crate::voip_control::{MediaAudioCodec as AudioCodec, MediaAudioFormat as AudioFormat};

    // 1:1 only, and the guard lives here rather than at the call site because the question itself
    // does not have a call-wide answer for a group. A group offer carries the capability of the ONE
    // device that invited us, while the engine applies the call's format to every participant: read
    // as a decision, it decodes the whole roster under one member's grammar and every member still
    // on negotiated MLOW goes silent. A group answers per participant instead, which the engine's
    // own classification and probe provide.
    if incoming.group.is_some() {
        return None;
    }
    if format != AudioFormat::MLOW_16KHZ_60MS
        && format != AudioFormat::OPUS_16KHZ_60MS
        && format != AudioFormat::OPUS_MLOW_16KHZ_60MS
    {
        return None;
    }
    let peer = incoming
        .media()
        .and_then(|media| media.peer_device.as_ref())
        .map_or(CapabilityBit::Unknown, |device| {
            capability_bit(
                device.capability_version,
                device.capability(),
                CAPABILITY_INDEX_MLOW_V1,
            )
        });
    // The capability gates MLOW's CONTAINER, not the codec inside it -- the escape profile carries
    // standard Opus in MLOW's framing, and a peer that cleared the bit cannot parse that either.
    let local_mlow = format.rtp_profile == AudioRtpProfile::Mlow;
    let effective_mlow = crate::stanza::call::mlow_after_peer_capability(local_mlow, peer);
    (effective_mlow != local_mlow).then_some(if effective_mlow {
        AudioCodec::Mlow
    } else {
        AudioCodec::Opus
    })
}

fn answer_preaccept_capability(video: bool, audio: AudioFormat) -> &'static [u8] {
    let standard_opus = matches!(audio.rtp_profile, AudioRtpProfile::StandardOpus);
    match (video, standard_opus) {
        (true, true) => &CAPABILITY_STANDARD_OPUS_OFFER,
        (false, true) => &CAPABILITY_STANDARD_OPUS_PREACCEPT,
        (true, false) => &CAPABILITY_OFFER,
        (false, false) => &CAPABILITY_PREACCEPT,
    }
}

pub fn build_answer_signaling(
    incoming: &IncomingCall,
    audio: AudioFormat,
    video: bool,
    is_group: bool,
    preaccept_id: &str,
    accept_id: &str,
) -> Option<(wacore_binary::Node, wacore_binary::Node)> {
    let CallAction::Offer {
        call_id,
        call_creator,
        ..
    } = &incoming.action
    else {
        return None;
    };
    let audio_rate = audio.signaling_rate.to_string();
    let audio_rates = [audio_rate.as_str()];
    let standard_opus = matches!(audio.rtp_profile, AudioRtpProfile::StandardOpus);
    let target = if is_group {
        Jid::new(call_id, Server::Call)
    } else {
        incoming.from.clone()
    };
    let preaccept = build_preaccept_with_capability(
        call_id,
        &target,
        call_creator,
        preaccept_id,
        &audio_rates,
        answer_preaccept_capability(video, audio),
        video,
    );
    // Captured video accepts mirror the offer's peer experiment metadata; audio accepts omit it.
    let metadata = if video { incoming.media() } else { None };
    let accept = build_accept(&AcceptParams {
        call_id,
        to: &target,
        id: accept_id,
        call_creator,
        audio_rates: &audio_rates,
        relay_te: None,
        rte: None,
        voip_settings: standard_opus
            .then(|| standard_opus_voip_settings(audio.rtp_clock_rate == 48_000)),
        // A captured from-start video accept carries no capability child; audio answers advertise
        // the selected MLOW/standard-Opus capability.
        capability: if video {
            None
        } else if standard_opus {
            Some(&CAPABILITY_STANDARD_OPUS_OFFER)
        } else {
            Some(&CAPABILITY_OFFER)
        },
        video,
        peer_abtest_bucket: metadata.and_then(|offer| offer.peer_abtest_bucket.as_deref()),
        peer_abtest_bucket_id_list: metadata
            .and_then(|offer| offer.peer_abtest_bucket_id_list.as_deref()),
    });
    Some((preaccept, accept))
}

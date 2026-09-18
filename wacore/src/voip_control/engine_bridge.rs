//! The consuming conversion from the neutral spec to the engine config.
//!
//! This lives here, not in the `whatsapp-rust` backend, because it splits crate-local types on
//! both sides. The module is gated on `voip`: the neutral contract must still compile with the
//! engine off, and this conversion is the one place the two meet.
//!
//! Consuming on purpose. The split used to clone `call_key`, `relay_token`, `auth_token` and
//! `integrity_key`; the callKey and relay credentials are the largest secret buffers on the path,
//! and there is no reason to hold two copies of each just to cross a boundary.

use crate::voip::audio::AudioConfig;
use crate::voip::engine::CallConfig;
use crate::voip_control::{MediaGroupSpec, MediaSessionKey, MediaSessionSpec, MediaSetupError};

/// The engine-side pieces of a neutral spec, with nothing dropped.
///
/// Internal adapter API: this module is `#[doc(hidden)]` because it exists only to cross the
/// crate boundary for the resident backend. A foreign backend never touches it; it implements
/// [`VoipMediaBackend`](super::VoipMediaBackend) directly against the neutral contract.
///
/// [`MediaSessionSpec`] carries the generational [`MediaSessionKey`] and an optional
/// [`MediaGroupSpec`], and [`CallConfig`] represents neither. A bare `TryFrom<MediaSessionSpec>`
/// would therefore be a lossy public conversion: a caller could consume a spec and silently lose
/// the group and the identity. This bundle keeps all three halves together, so the only public way
/// across the boundary hands back everything.
#[derive(Debug)]
#[non_exhaustive]
pub struct EngineParts {
    pub key: MediaSessionKey,
    pub config: CallConfig,
    pub group: Option<MediaGroupSpec>,
}

/// Split a neutral spec into everything the engine needs, consuming the spec.
///
/// Internal adapter API, called only by the resident backend: a foreign backend implements
/// [`VoipMediaBackend`](super::VoipMediaBackend) without crossing this bridge.
///
/// The config is validated and moved, while the group and the key come back beside it rather than
/// being folded into `CallConfig`, which cannot hold them. Callers that only need the engine pass
/// the `group` to `configure_group`; callers that need the identity read `key`.
pub fn into_engine_parts(spec: MediaSessionSpec) -> Result<EngineParts, MediaSetupError> {
    let audio = AudioConfig::from_neutral(spec.audio).ok_or(MediaSetupError::BadAudioFormat)?;
    if spec.relay_ip.parse::<std::net::Ipv4Addr>().is_err() {
        return Err(MediaSetupError::BadEndpoint);
    }
    if spec.call_key.len() < 32 {
        return Err(MediaSetupError::BadCallKey);
    }
    let MediaSessionSpec {
        key,
        direction,
        self_lid,
        peer_lid,
        call_key,
        ssrc,
        relay_token,
        auth_token,
        relay_ip,
        relay_port,
        integrity_key,
        warp_mi_tag_len,
        enable_media,
        enable_video,
        enable_sframe,
        group,
        // `audio` is validated above; the format is rebuilt from the engine's own type.
        audio: _,
    } = spec;
    let config = CallConfig {
        call_id: key.call_id.clone(),
        // The engine's `CallConfig.direction` is this same neutral enum, so no translation.
        direction,
        self_lid,
        peer_lid,
        call_key,
        ssrc,
        audio,
        relay_token,
        auth_token,
        relay_ip,
        relay_port,
        integrity_key,
        warp_mi_tag_len,
        enable_media,
        enable_video,
        enable_sframe,
    };
    Ok(EngineParts { key, config, group })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voip_control::CallDirection;

    fn spec(generation: u64, call_key: Vec<u8>) -> MediaSessionSpec {
        use crate::voip_control::{MediaAudioFormat, MediaAudioIo, MediaAudioSpec};
        MediaSessionSpec::builder()
            .key(MediaSessionKey {
                call_id: "CID".into(),
                generation,
            })
            .direction(CallDirection::Incoming)
            .self_lid("1:0@lid".into())
            .peer_lid("2:0@lid".into())
            .call_key(call_key)
            .ssrc(1)
            .audio(
                MediaAudioSpec::builder()
                    .format(MediaAudioFormat::MLOW_16KHZ_60MS)
                    .io(MediaAudioIo::Pcm)
                    .build(),
            )
            .relay_token(vec![])
            .auth_token(vec![])
            .relay_ip("127.0.0.1".into())
            .relay_port(3478)
            .integrity_key(vec![])
            .warp_mi_tag_len(4)
            .enable_media(false)
            .enable_video(false)
            .enable_sframe(false)
            .build()
    }

    #[test]
    fn the_split_preserves_the_key_material_and_identity() {
        let call_key: Vec<u8> = (0u8..32).collect();
        let parts = into_engine_parts(spec(42, call_key.clone())).expect("the spec splits");
        assert_eq!(parts.key.call_id, "CID");
        assert_eq!(parts.key.generation, 42);
        assert!(parts.group.is_none());
        assert_eq!(parts.config.call_id, "CID");
        assert_eq!(parts.config.call_key, call_key);
        assert_eq!(parts.config.direction, CallDirection::Incoming);
    }

    #[test]
    fn a_short_call_key_is_refused_on_the_way_in() {
        assert_eq!(
            into_engine_parts(spec(1, vec![0u8; 8])).err(),
            Some(MediaSetupError::BadCallKey)
        );
    }

    #[test]
    fn a_zero_timing_format_is_refused_on_the_way_in() {
        let mut spec = spec(1, (0u8..32).collect());
        spec.audio.format.sample_rate = 0;
        assert_eq!(
            into_engine_parts(spec).err(),
            Some(MediaSetupError::BadAudioFormat)
        );
    }
}

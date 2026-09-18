//! Consuming conversions between the neutral spec and the engine config.
//!
//! These live here, not in the `whatsapp-rust` backend, because the orphan rule keeps a `TryFrom`
//! impl next to a type it mentions, and `MediaSessionSpec` is defined in this crate's
//! [`voip_control`](crate::voip_control) while `CallConfig` is defined in the `voip`-gated
//! [`engine`](crate::voip::engine). The module is gated on `voip`: the neutral contract must still
//! compile with the engine off, and these conversions are the one place the two meet.
//!
//! Consuming on purpose. The setup round trip used to clone `call_key`, `relay_token`, `auth_token`
//! and `integrity_key`; the callKey and relay credentials are the largest secret buffers on the
//! path, and there is no reason to hold two copies of each just to cross a boundary.

use crate::voip::audio::AudioConfig;
use crate::voip::engine::CallConfig;
use crate::voip::session::CallDirection;
use crate::voip_control::{
    MediaDirection, MediaGroupSpec, MediaSessionKey, MediaSessionSpec, MediaSetupError,
};

impl TryFrom<(CallConfig, u64)> for MediaSessionSpec {
    type Error = MediaSetupError;

    /// Project an engine config onto the neutral spec, consuming the config.
    ///
    /// `generation` is the control plane's monotonic registration token, not a field of
    /// [`CallConfig`]: it names *this* session among same-call-id replacements, so it rides the
    /// neutral [`MediaSessionKey`] rather than the engine config.
    fn try_from((config, generation): (CallConfig, u64)) -> Result<Self, Self::Error> {
        // Exhaustive on purpose: this lives in `wacore`, the enum's own crate, so a new direction
        // fails to compile here rather than being silently projected onto the wrong side.
        let direction = match config.direction {
            CallDirection::Outgoing => MediaDirection::Outgoing,
            CallDirection::Incoming => MediaDirection::Incoming,
        };
        Ok(Self {
            key: MediaSessionKey {
                call_id: config.call_id,
                generation,
            },
            direction,
            self_lid: config.self_lid,
            peer_lid: config.peer_lid,
            call_key: config.call_key,
            ssrc: config.ssrc,
            audio: config.audio.to_neutral(),
            relay_token: config.relay_token,
            auth_token: config.auth_token,
            relay_ip: config.relay_ip,
            relay_port: config.relay_port,
            integrity_key: config.integrity_key,
            warp_mi_tag_len: config.warp_mi_tag_len,
            enable_media: config.enable_media,
            enable_video: config.enable_video,
            enable_sframe: config.enable_sframe,
            group: None,
        })
    }
}

/// The engine-side pieces of a neutral spec, with nothing dropped.
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
/// This is the non-lossy inverse of [`TryFrom<(CallConfig, u64)>`](MediaSessionSpec): the config is
/// validated and moved, while the group and the key come back beside it rather than being folded
/// into `CallConfig`, which cannot hold them. Callers that only need the engine pass the `group`
/// to `configure_group`; callers that need the identity read `key`.
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
        // Exhaustive on purpose: a new direction fails to compile here rather than being silently
        // dialed as the wrong side.
        direction: match direction {
            MediaDirection::Outgoing => CallDirection::Outgoing,
            MediaDirection::Incoming => CallDirection::Incoming,
        },
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
    use crate::voip::relay_parse::{RelayAddress, RelayData, RelayEndpoint};

    fn relay() -> RelayData {
        RelayData {
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
        }
    }

    #[test]
    fn the_round_trip_preserves_the_key_material_and_identity() {
        let config =
            CallConfig::for_incoming("CID", "1:0@lid", "2:0@lid", (0u8..32).collect(), &relay())
                .expect("config builds from a complete relay");
        let call_key = config.call_key.clone();
        let spec = MediaSessionSpec::try_from((config, 42)).expect("the config projects");
        assert_eq!(spec.key.call_id, "CID");
        assert_eq!(spec.key.generation, 42);
        assert_eq!(spec.call_key, call_key);
        let parts = into_engine_parts(spec).expect("the spec splits");
        assert_eq!(parts.key.call_id, "CID");
        assert_eq!(parts.key.generation, 42);
        assert!(parts.group.is_none());
        assert_eq!(parts.config.call_id, "CID");
        assert_eq!(parts.config.call_key, call_key);
        assert_eq!(parts.config.direction, CallDirection::Incoming);
    }

    #[test]
    fn a_short_call_key_is_refused_on_the_way_in() {
        let config = CallConfig {
            call_key: vec![0u8; 8],
            ..CallConfig::for_incoming("CID", "1:0@lid", "2:0@lid", (0u8..32).collect(), &relay())
                .expect("config builds")
        };
        let spec = MediaSessionSpec::try_from((config, 1)).expect("projection does not validate");
        assert_eq!(
            into_engine_parts(spec).err(),
            Some(MediaSetupError::BadCallKey)
        );
    }

    #[test]
    fn a_zero_timing_format_is_refused_on_the_way_in() {
        let config =
            CallConfig::for_incoming("CID", "1:0@lid", "2:0@lid", (0u8..32).collect(), &relay())
                .expect("config builds");
        let mut spec = MediaSessionSpec::try_from((config, 1)).expect("the config projects");
        spec.audio.format.sample_rate = 0;
        assert_eq!(
            into_engine_parts(spec).err(),
            Some(MediaSetupError::BadAudioFormat)
        );
    }
}

//! Building a [`MediaSessionSpec`] from a parsed relay block.
//!
//! The 1:1 offer and the outgoing ack both carry a `<relay>`; this projects it onto the neutral spec
//! the backend opens, deriving our participant SSRC and selecting the endpoint, token and integrity
//! key. It is pure, so the whole media-config build is offline testable, and it lives on the
//! control plane so the facade can build a spec without the engine.

use super::relay_parse::{
    RelayData, get_media_relay_endpoint, get_primary_ipv4_address, select_auth_token,
};
use super::{
    CallDirection, MediaAudioIo, MediaAudioSpec, MediaSessionKey, MediaSessionSpec, MediaSetupError,
};

/// Default WARP authentication-tag width when the relay omits `warp_mi_tag_len`.
const WARP_MI_TAG_LEN: usize = 4;

/// Select the endpoint a group relay's media should use.
fn group_media_relay_endpoint(
    relay: &crate::types::group_call::GroupCallRelay,
) -> Option<&crate::types::group_call::GroupCallRelayEndpoint> {
    use crate::types::group_call::GroupCallRelayEndpoint;
    let usable = |endpoint: &&GroupCallRelayEndpoint| {
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

impl MediaSessionSpec {
    /// Build a native group-call spec before its shared keygen-v2 epoch arrives.
    ///
    /// The callKey is zeroed: media is gated until an authenticated epoch installs the real key, so
    /// the bootstrap value is never permitted onto the wire. Mirrors the 1:1 constructor but pulls
    /// the endpoint and token from the group relay block, which indexes its own token sets.
    pub fn for_group(
        direction: CallDirection,
        key: MediaSessionKey,
        self_lid: &str,
        call_creator: &str,
        relay: &crate::types::group_call::GroupCallRelay,
    ) -> Result<Self, MediaSetupError> {
        let endpoint = group_media_relay_endpoint(relay)
            .ok_or_else(|| MediaSetupError::Relay("group relay has no endpoint".into()))?;
        let relay_ip = endpoint
            .ipv4
            .clone()
            .ok_or_else(|| MediaSetupError::Relay("group relay endpoint has no IPv4".into()))?;
        let relay_port = endpoint
            .port
            .ok_or_else(|| MediaSetupError::Relay("group relay endpoint has no port".into()))?;
        let relay_token = relay
            .tokens
            .get(endpoint.token_id as usize)
            .filter(|token| !token.is_empty())
            .cloned()
            .ok_or_else(|| {
                MediaSetupError::Relay(format!("group relay has no token #{}", endpoint.token_id))
            })?;
        if relay.key.is_empty() {
            return Err(MediaSetupError::Relay(
                "group relay has no <key> (STUN integrity key)".into(),
            ));
        }
        let warp_mi_tag_len = relay
            .warp_mi_tag_len
            .map(|value| value as usize)
            .unwrap_or(WARP_MI_TAG_LEN);
        if !(1..=20).contains(&warp_mi_tag_len) {
            return Err(MediaSetupError::Relay(format!(
                "group relay advertised an unsupported WARP MI tag length: {warp_mi_tag_len}"
            )));
        }
        let our_ssrc = super::ssrc::derive_wasm_participant_ssrc(
            &key.call_id,
            &super::ssrc::format_e2e_srtp_participant_id(self_lid),
            0,
        );
        Ok(Self {
            key,
            direction,
            self_lid: self_lid.to_string(),
            peer_lid: call_creator.to_string(),
            call_key: vec![0; 32],
            ssrc: our_ssrc,
            audio: MediaAudioSpec {
                format: super::MediaAudioFormat::MLOW_16KHZ_60MS,
                io: MediaAudioIo::Pcm,
            },
            relay_token,
            auth_token: select_auth_token(&relay.auth_tokens, endpoint.auth_token_id),
            relay_ip,
            relay_port,
            integrity_key: relay.key.clone(),
            warp_mi_tag_len,
            enable_media: true,
            enable_video: false,
            enable_sframe: false,
            group: None,
        })
    }
}

impl MediaSessionSpec {
    /// Build the spec from the callKey and the parsed `<relay>`.
    ///
    /// The only thing that differs by direction is [`direction`](Self::direction); everything else is
    /// identical. `enable_sframe` is on because the peer may GCM-wrap its codec (recv-decrypt only).
    /// The caller sets `audio`, `enable_video` and `group` afterwards.
    pub fn from_relay(
        direction: CallDirection,
        key: MediaSessionKey,
        self_lid: &str,
        peer_lid: &str,
        call_key: Vec<u8>,
        relay: &RelayData,
    ) -> Result<Self, MediaSetupError> {
        if call_key.len() < 32 {
            return Err(MediaSetupError::BadCallKey);
        }
        let ep = get_media_relay_endpoint(relay)
            .ok_or_else(|| MediaSetupError::Relay("relay has no endpoints".into()))?;
        let (relay_ip, relay_port) = get_primary_ipv4_address(ep)
            .ok_or_else(|| MediaSetupError::Relay("relay endpoint has no IPv4 address".into()))?;
        // A padded-empty slot is a missing token, not a zero-length one.
        let relay_token = relay
            .relay_tokens
            .get(ep.token_id as usize)
            .filter(|token| !token.is_empty())
            .cloned()
            .ok_or_else(|| {
                MediaSetupError::Relay(format!("relay has no token #{}", ep.token_id))
            })?;
        let auth_token = select_auth_token(&relay.auth_tokens, ep.auth_token_id);
        // Signed with the base64 TEXT of `<key>`, not its decoded bytes: the relay HMACs against the
        // ASCII key material.
        let integrity_key = relay.relay_key_ascii.clone().ok_or_else(|| {
            MediaSetupError::Relay("relay has no <key> (STUN integrity key)".into())
        })?;

        let our_ssrc = super::ssrc::derive_wasm_participant_ssrc(
            &key.call_id,
            &super::ssrc::format_e2e_srtp_participant_id(self_lid),
            0,
        );

        let warp_mi_tag_len = relay
            .warp_mi_tag_len
            .map(|n| n as usize)
            .unwrap_or(WARP_MI_TAG_LEN);
        if !(1..=20).contains(&warp_mi_tag_len) {
            return Err(MediaSetupError::Relay(format!(
                "relay advertised an unsupported WARP MI tag length: {warp_mi_tag_len}"
            )));
        }

        Ok(Self {
            key,
            direction,
            self_lid: self_lid.to_string(),
            peer_lid: peer_lid.to_string(),
            call_key,
            ssrc: our_ssrc,
            audio: MediaAudioSpec {
                format: super::MediaAudioFormat::MLOW_16KHZ_60MS,
                io: MediaAudioIo::Pcm,
            },
            relay_token,
            auth_token,
            relay_ip,
            relay_port,
            integrity_key,
            warp_mi_tag_len,
            enable_media: true,
            enable_video: false,
            enable_sframe: true,
            group: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voip_control::relay_parse::{RelayAddress, RelayEndpoint};

    fn relay() -> RelayData {
        RelayData {
            relay_key_ascii: Some(b"relay-key".to_vec()),
            warp_mi_tag_len: Some(4),
            relay_tokens: vec![vec![0xAB; 16]],
            auth_tokens: vec![vec![0xCD; 8]],
            endpoints: vec![RelayEndpoint {
                relay_id: 1,
                relay_name: "gru1c02".into(),
                token_id: 0,
                auth_token_id: 0,
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

    fn key() -> MediaSessionKey {
        MediaSessionKey {
            call_id: "CID".into(),
            generation: 42,
        }
    }

    #[test]
    fn the_spec_projects_the_relay_and_derives_the_ssrc() {
        let spec = MediaSessionSpec::from_relay(
            CallDirection::Incoming,
            key(),
            "1:0@lid",
            "2:0@lid",
            (0u8..32).collect(),
            &relay(),
        )
        .expect("a complete relay builds a spec");
        assert_eq!(spec.key.call_id, "CID");
        assert_eq!(spec.key.generation, 42);
        assert_eq!(spec.direction, CallDirection::Incoming);
        assert_eq!(spec.relay_ip, "203.0.113.7");
        assert_eq!(spec.relay_port, 3478);
        assert_eq!(spec.relay_token, vec![0xAB; 16]);
        assert_eq!(spec.auth_token, vec![0xCD; 8]);
        assert_eq!(spec.integrity_key, b"relay-key");
        assert_ne!(spec.ssrc, 0, "our participant SSRC is derived");
    }

    #[test]
    fn xpop_selection_keeps_credentials_bound_through_media_setup() {
        use crate::voip_control::relay_parse::parse_relay_data;
        use wacore_binary::builder::NodeBuilder;
        for normal_available in [false, true] {
            let mut children = vec![
                NodeBuilder::new("key")
                    .bytes(b"synthetic-key".to_vec())
                    .build(),
                NodeBuilder::new("auth_token")
                    .attr("id", "0")
                    .bytes(vec![0x33; 8])
                    .build(),
                NodeBuilder::new("token")
                    .attr("id", "2")
                    .bytes(vec![0x22; 16])
                    .build(),
                NodeBuilder::new("te2")
                    .attr("relay_id", "2")
                    .attr("token_id", "1")
                    .attr("xtoken_id", "2")
                    .attr("auth_token_id", "0")
                    .bytes(vec![192, 0, 2, 1, 0x0d, 0x96])
                    .build(),
            ];
            if normal_available {
                children.push(
                    NodeBuilder::new("token")
                        .attr("id", "1")
                        .bytes(vec![0x11; 16])
                        .build(),
                );
            }
            let node = NodeBuilder::new("relay").children(children).build();
            let relay = parse_relay_data(&node.as_node_ref()).unwrap();
            let spec = MediaSessionSpec::from_relay(
                CallDirection::Outgoing,
                key(),
                "1:0@lid",
                "2:0@lid",
                vec![0; 32],
                &relay,
            )
            .unwrap();
            let expected_token = vec![if normal_available { 0x11 } else { 0x22 }; 16];
            assert_eq!(spec.relay_token, expected_token);
            assert_eq!(spec.auth_token, vec![0x33; 8]);
            assert_eq!(
                (spec.relay_ip.as_str(), spec.relay_port),
                ("192.0.2.1", 3478)
            );
            #[cfg(feature = "voip")]
            {
                use crate::voip::engine::{CallEngine, Output, SequentialTxIds};
                use crate::voip::stun::parse_stun_attributes;
                let mut parts =
                    crate::voip_control::engine_bridge::into_engine_parts(spec).unwrap();
                // Allocate is control-plane traffic: credential binding must also be
                // checked with `voip` alone, without requiring a PCM/MLOW decoder.
                parts.config.enable_media = false;
                let mut engine =
                    CallEngine::new(parts.config, Box::new(SequentialTxIds::new())).unwrap();
                engine.start(0, 0);
                let Output::Transmit(packet) = engine.poll_output() else {
                    panic!("missing Allocate")
                };
                let attrs = parse_stun_attributes(&packet);
                assert_eq!(
                    attrs.iter().find(|a| a.attr_type == 0x4000).unwrap().value,
                    expected_token
                );
            }
        }
    }

    #[test]
    fn captured_xpop_token_permutations_keep_credentials_with_both_address_families() {
        use crate::voip_control::relay_parse::{
            RelayCandidateKind, parse_relay_candidates, parse_relay_data,
        };
        use wacore_binary::builder::NodeBuilder;
        for x_token in 0..3u32 {
            let mut children = vec![
                NodeBuilder::new("key")
                    .bytes(b"synthetic-key".to_vec())
                    .build(),
            ];
            for token in 0..3u32 {
                children.push(
                    NodeBuilder::new("token")
                        .attr("id", token.to_string())
                        .bytes(vec![0x10 + token as u8; 12])
                        .build(),
                );
            }
            for auth in 0..2u32 {
                children.push(
                    NodeBuilder::new("auth_token")
                        .attr("id", auth.to_string())
                        .bytes(vec![0xa0 + auth as u8; 8])
                        .build(),
                );
            }
            for relay_id in 0..3u32 {
                let (attr, token, auth) = if relay_id == 2 {
                    ("xtoken_id", x_token, "1")
                } else {
                    ("token_id", (x_token + relay_id + 1) % 3, "0")
                };
                let mut v6 = vec![0x20, 0x01, 0x0d, 0xb8];
                v6.extend([0; 11]);
                v6.extend([relay_id as u8 + 1, 13, 150]);
                for address in [vec![192, 0, 2, relay_id as u8 + 1, 13, 150], v6] {
                    children.push(
                        NodeBuilder::new("te2")
                            .attr("relay_id", relay_id.to_string())
                            .attr("relay_name", format!("synthetic-{relay_id}"))
                            .attr(attr, token.to_string())
                            .attr("auth_token_id", auth)
                            .attr("xrtt_ms", "52")
                            .attr("c2r_rtt", "7")
                            .bytes(address)
                            .build(),
                    );
                }
            }
            let node = NodeBuilder::new("relay").children(children).build();
            let nr = node.as_node_ref();
            let data = parse_relay_data(&nr).unwrap();
            let candidates = parse_relay_candidates(&nr).unwrap();
            assert_eq!(candidates.len(), 3);
            assert_eq!(candidates[2].kind, RelayCandidateKind::Xpop);
            assert_eq!(candidates[2].xrtt_ms, Some(52));
            assert!(data.endpoints.iter().all(|e| e.addresses.len() == 2));
            let spec = MediaSessionSpec::from_relay(
                CallDirection::Outgoing,
                key(),
                "1:0@lid",
                "2:0@lid",
                vec![0; 32],
                &data,
            )
            .unwrap();
            assert_eq!(spec.relay_token, vec![0x10 + x_token as u8; 12]);
            assert_eq!(spec.auth_token, vec![0xa1; 8]);
            assert_eq!(spec.relay_ip, "192.0.2.3");
            assert_eq!(spec.relay_port, 3478);
            #[cfg(feature = "voip")]
            {
                use crate::voip::engine::{CallEngine, Output, SequentialTxIds};
                let mut parts =
                    crate::voip_control::engine_bridge::into_engine_parts(spec).unwrap();
                parts.config.enable_media = false;
                let mut engine =
                    CallEngine::new(parts.config, Box::new(SequentialTxIds::new())).unwrap();
                engine.start(0, 0);
                let Output::Transmit(packet) = engine.poll_output() else {
                    panic!("missing Allocate")
                };
                assert_eq!(
                    crate::voip::stun::parse_stun_attributes(&packet)
                        .iter()
                        .find(|attr| attr.attr_type == 0x4000)
                        .unwrap()
                        .value,
                    vec![0x10 + x_token as u8; 12]
                );
            }
        }
    }

    #[test]
    fn a_short_call_key_is_refused() {
        let error = MediaSessionSpec::from_relay(
            CallDirection::Outgoing,
            key(),
            "1:0@lid",
            "2:0@lid",
            vec![0u8; 8],
            &relay(),
        )
        .err();
        assert_eq!(error, Some(MediaSetupError::BadCallKey));
    }

    #[test]
    fn a_relay_without_a_key_is_refused() {
        let mut relay = relay();
        relay.relay_key_ascii = None;
        let error = MediaSessionSpec::from_relay(
            CallDirection::Incoming,
            key(),
            "1:0@lid",
            "2:0@lid",
            vec![0u8; 32],
            &relay,
        )
        .err();
        assert!(matches!(error, Some(MediaSetupError::Relay(_))));
    }

    #[test]
    fn a_group_relay_builds_a_zero_key_spec() {
        use crate::types::group_call::{GroupCallRelay, GroupCallRelayEndpoint};
        let relay = GroupCallRelay::builder()
            .uuid("relay".into())
            .participant_uuid("participant".into())
            .attribute_padding(false)
            .warp_mi_tag_len(4)
            .key(b"relay-key".to_vec())
            .tokens(vec![vec![0xAB; 16]])
            .auth_tokens(vec![vec![0xCD; 8]])
            .endpoints(vec![
                GroupCallRelayEndpoint::builder()
                    .relay_id(1)
                    .token_id(0)
                    .auth_token_id(0)
                    .relay_name("relay-1".into())
                    .is_fna(false)
                    .ipv4("203.0.113.9".into())
                    .port(3480)
                    .build(),
            ])
            .build();
        let spec = MediaSessionSpec::for_group(
            CallDirection::Outgoing,
            key(),
            "1:0@lid",
            "2:0@lid",
            &relay,
        )
        .expect("a complete group relay builds a spec");
        assert_eq!(spec.relay_ip, "203.0.113.9");
        assert_eq!(spec.relay_port, 3480);
        assert_eq!(spec.relay_token, vec![0xAB; 16]);
        assert_eq!(spec.auth_token, vec![0xCD; 8]);
        assert_eq!(spec.call_key, vec![0u8; 32], "bootstrap key is zeroed");
        assert!(!spec.enable_sframe);
    }
}

//! Parse the `<relay>` block from a call ack into [`RelayData`] (hbh_key, relay key,
//! indexed tokens, te2 endpoints) and select outbound relay candidates.
//!
//! wacrg spec: relay-candidates (REL-01), stun-relay (REL-02).

use crate::voip::hbh_srtp::HBH_KEY_LEN;
use base64::prelude::*;
use std::collections::HashMap;
use wacore_binary::NodeRef;

/// Default relay port from a te2 endpoint (0x0D96).
pub const WHATSAPP_RELAY_PORT: u16 = 3478;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RelayAddress {
    pub protocol: u8,
    pub ipv4: Option<String>,
    pub ipv6: Option<String>,
    pub port: u16,
}

#[derive(Clone, Debug, Default)]
pub struct RelayEndpoint {
    pub relay_id: u32,
    pub relay_name: String,
    pub token_id: u32,
    pub auth_token_id: u32,
    pub is_fna: bool,
    /// Raw 6-byte IPv4:port from the first matching te2; used verbatim in relaylatency.
    pub ipv4_te2_bytes: Option<[u8; 6]>,
    pub addresses: Vec<RelayAddress>,
    pub c2r_rtt_ms: Option<u32>,
}

#[derive(Clone, Default)]
pub struct RelayData {
    pub hbh_key: Option<Vec<u8>>,
    /// Raw `<hbh_key>` content (ASCII base64); used as the STUN MESSAGE-INTEGRITY key.
    pub hbh_key_ascii: Option<Vec<u8>>,
    pub relay_key: Option<Vec<u8>>,
    /// Raw `<key>` content before base64 decode; the STUN MESSAGE-INTEGRITY key material.
    pub relay_key_ascii: Option<Vec<u8>>,
    pub warp_mi_tag_len: Option<u32>,
    pub uuid: Option<String>,
    pub self_pid: Option<u32>,
    pub peer_pid: Option<u32>,
    pub relay_tokens: Vec<Vec<u8>>,
    pub auth_tokens: Vec<Vec<u8>>,
    pub endpoints: Vec<RelayEndpoint>,
}

// Manual Debug so a stray `{:?}` (e.g. on Event::IncomingCall, which carries this) can't leak the
// SRTP master key, the STUN MESSAGE-INTEGRITY key, or the relay/auth tokens. Matches the redaction
// the sibling key structs already apply (E2eSrtpKeys, SrtpKeyingMaterial, CallConfig).
impl core::fmt::Debug for RelayData {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let redact = |o: &Option<Vec<u8>>| o.as_ref().map(|_| "[redacted]");
        f.debug_struct("RelayData")
            .field("hbh_key", &redact(&self.hbh_key))
            .field("hbh_key_ascii", &redact(&self.hbh_key_ascii))
            .field("relay_key", &redact(&self.relay_key))
            .field("relay_key_ascii", &redact(&self.relay_key_ascii))
            .field("warp_mi_tag_len", &self.warp_mi_tag_len)
            .field("uuid", &self.uuid)
            .field("self_pid", &self.self_pid)
            .field("peer_pid", &self.peer_pid)
            .field(
                "relay_tokens",
                &format_args!("[{} redacted]", self.relay_tokens.len()),
            )
            .field(
                "auth_tokens",
                &format_args!("[{} redacted]", self.auth_tokens.len()),
            )
            .field("endpoints", &self.endpoints)
            .finish()
    }
}

fn looks_like_base64(txt: &str) -> bool {
    txt.len() >= 4
        && txt
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'+' || c == b'/' || c == b'=')
}

fn try_decode_base64(bytes: &[u8]) -> Option<Vec<u8>> {
    let txt = std::str::from_utf8(bytes).ok()?;
    if !looks_like_base64(txt) {
        return None;
    }
    BASE64_STANDARD.decode(txt).ok().or_else(|| {
        BASE64_STANDARD_NO_PAD
            .decode(txt.trim_end_matches('='))
            .ok()
    })
}

/// Bytes content of a node, accepting either Bytes or String (UTF-8) content.
fn node_content_bytes(node: &NodeRef<'_>) -> Option<Vec<u8>> {
    if let Some(b) = node.content_bytes() {
        return Some(b.to_vec());
    }
    node.content_str().map(|s| s.as_bytes().to_vec())
}

/// Decode `<hbh_key>` to 30 bytes (16B master_key + 14B master_salt); handles double-base64.
pub fn decode_hbh_key(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.is_empty() {
        return None;
    }
    let mut decoded = try_decode_base64(bytes).unwrap_or_else(|| bytes.to_vec());
    if decoded.len() != HBH_KEY_LEN
        && let Some(inner) = try_decode_base64(&decoded)
        && inner.len() == HBH_KEY_LEN
    {
        decoded = inner;
    }
    (decoded.len() == HBH_KEY_LEN).then_some(decoded)
}

/// Decode `<key>` to its raw bytes (16B for STUN MI), falling back to the input.
pub fn decode_relay_key_content(bytes: &[u8]) -> Vec<u8> {
    try_decode_base64(bytes).unwrap_or_else(|| bytes.to_vec())
}

/// Decode `<raw_e2e>` (keygen v2): base64, min 32 bytes for the E2E SRTP IKM.
pub fn decode_raw_e2e_content(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.is_empty() {
        return None;
    }
    let decoded = try_decode_base64(bytes).unwrap_or_else(|| bytes.to_vec());
    (decoded.len() >= 32).then_some(decoded)
}

/// Bound on a relay-supplied `<token id=...>` index. The relay is untrusted, so an unbounded
/// id would let it force an arbitrarily large allocation.
const MAX_RELAY_TOKENS: usize = 64;

fn parse_indexed_tokens(children: &[NodeRef<'_>], tag: &str) -> Vec<Vec<u8>> {
    let mut tokens: Vec<Vec<u8>> = Vec::new();
    for node in children.iter().filter(|c| c.tag.as_ref() == tag) {
        let Some(bytes) = node_content_bytes(node) else {
            continue;
        };
        let id = node
            .attrs()
            .optional_string("id")
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(tokens.len());
        if id >= MAX_RELAY_TOKENS {
            continue;
        }
        while tokens.len() <= id {
            tokens.push(Vec::new());
        }
        tokens[id] = bytes;
    }
    tokens
}

fn parse_te2_address(bytes: &[u8], protocol: u8) -> Option<RelayAddress> {
    match bytes.len() {
        6 => Some(RelayAddress {
            protocol,
            ipv4: Some(format!(
                "{}.{}.{}.{}",
                bytes[0], bytes[1], bytes[2], bytes[3]
            )),
            ipv6: None,
            port: ((bytes[4] as u16) << 8) | bytes[5] as u16,
        }),
        18 => {
            let mut parts = Vec::with_capacity(8);
            for i in (0..16).step_by(2) {
                parts.push(format!(
                    "{:x}",
                    ((bytes[i] as u16) << 8) | bytes[i + 1] as u16
                ));
            }
            Some(RelayAddress {
                protocol,
                ipv4: None,
                ipv6: Some(parts.join(":")),
                port: ((bytes[16] as u16) << 8) | bytes[17] as u16,
            })
        }
        _ => None,
    }
}

/// Parse a `<relay>` node into [`RelayData`].
pub fn parse_relay_data(relay_node: &NodeRef<'_>) -> Option<RelayData> {
    let children = relay_node.children().unwrap_or_default();
    let find_bytes = |tag: &str| {
        children
            .iter()
            .find(|c| c.tag.as_ref() == tag)
            .and_then(node_content_bytes)
    };

    let key_bytes = find_bytes("key");
    let hbh_key_bytes = find_bytes("hbh_key");
    let warp_mi_tag_len = find_bytes("warp_mi_tag_len")
        .and_then(|b| String::from_utf8(b).ok())
        .and_then(|s| s.trim().parse::<u32>().ok())
        .filter(|&n| n > 0);

    let relay_tokens = parse_indexed_tokens(children, "token");
    let auth_tokens = parse_indexed_tokens(children, "auth_token");

    let mut endpoints: Vec<RelayEndpoint> = Vec::new();
    let mut index_by_key: HashMap<String, usize> = HashMap::new();

    for te2 in children.iter().filter(|c| c.tag.as_ref() == "te2") {
        let Some(addr_bytes) = node_content_bytes(te2) else {
            continue;
        };
        let mut a = te2.attrs();
        let relay_id = a
            .optional_string("relay_id")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let relay_name = a
            .optional_string("relay_name")
            .map(|s| s.into_owned())
            .unwrap_or_default();
        let token_id = a
            .optional_string("token_id")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let auth_token_id = a
            .optional_string("auth_token_id")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let is_fna = a.optional_string("is_fna").as_deref() == Some("1");
        let protocol = a
            .optional_string("protocol")
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let c2r_rtt_ms = a
            .optional_string("c2r_rtt")
            .and_then(|s| s.parse::<u32>().ok());

        let Some(address) = parse_te2_address(&addr_bytes, protocol) else {
            continue;
        };

        let key = format!("{relay_id}:{relay_name}");
        let idx = *index_by_key.entry(key).or_insert_with(|| {
            endpoints.push(RelayEndpoint {
                relay_id,
                relay_name: relay_name.clone(),
                token_id,
                auth_token_id,
                is_fna,
                ipv4_te2_bytes: None,
                addresses: Vec::new(),
                c2r_rtt_ms,
            });
            endpoints.len() - 1
        });
        let endpoint = &mut endpoints[idx];
        endpoint.addresses.push(address);
        if let Some(rtt) = c2r_rtt_ms {
            endpoint.c2r_rtt_ms = Some(rtt);
        }
        if addr_bytes.len() == 6 && endpoint.ipv4_te2_bytes.is_none() {
            let mut six = [0u8; 6];
            six.copy_from_slice(&addr_bytes);
            endpoint.ipv4_te2_bytes = Some(six);
        }
    }

    let mut attrs = relay_node.attrs();
    Some(RelayData {
        hbh_key: hbh_key_bytes.as_deref().and_then(decode_hbh_key),
        hbh_key_ascii: hbh_key_bytes,
        relay_key: key_bytes.as_deref().map(decode_relay_key_content),
        relay_key_ascii: key_bytes,
        warp_mi_tag_len,
        uuid: attrs.optional_string("uuid").map(|s| s.into_owned()),
        self_pid: attrs
            .optional_string("self_pid")
            .and_then(|s| s.parse().ok()),
        peer_pid: attrs
            .optional_string("peer_pid")
            .and_then(|s| s.parse().ok()),
        relay_tokens,
        auth_tokens,
        endpoints,
    })
}

/// Find and parse the `<relay>` child of an ack node.
pub fn parse_relay_data_from_ack(ack_node: &NodeRef<'_>) -> Option<RelayData> {
    let relay = ack_node
        .children()?
        .iter()
        .find(|c| c.tag.as_ref() == "relay")?;
    parse_relay_data(relay)
}

/// Merge a patch (e.g. the accept ack's hbh_key) over an existing relay block.
pub fn merge_relay_data(base: RelayData, patch: RelayData) -> RelayData {
    RelayData {
        hbh_key: patch.hbh_key.or(base.hbh_key),
        hbh_key_ascii: patch.hbh_key_ascii.or(base.hbh_key_ascii),
        relay_key: patch.relay_key.or(base.relay_key),
        relay_key_ascii: patch.relay_key_ascii.or(base.relay_key_ascii),
        warp_mi_tag_len: patch.warp_mi_tag_len.or(base.warp_mi_tag_len),
        uuid: patch.uuid.or(base.uuid),
        self_pid: patch.self_pid.or(base.self_pid),
        peer_pid: patch.peer_pid.or(base.peer_pid),
        relay_tokens: if patch.relay_tokens.is_empty() {
            base.relay_tokens
        } else {
            patch.relay_tokens
        },
        auth_tokens: if patch.auth_tokens.is_empty() {
            base.auth_tokens
        } else {
            patch.auth_tokens
        },
        endpoints: if patch.endpoints.is_empty() {
            base.endpoints
        } else {
            patch.endpoints
        },
    }
}

/// FNA relays (is_fna=1, auth_token_id=0) are inbound-only; not for outbound relaylatency.
pub fn is_outbound_relay_candidate(endpoint: &RelayEndpoint) -> bool {
    !endpoint.is_fna && endpoint.auth_token_id != 0
}

/// Outbound relay endpoints, deduped by name and sorted by relay_id.
pub fn get_outbound_relay_endpoints(relay_data: &RelayData) -> Vec<RelayEndpoint> {
    let mut seen = std::collections::HashSet::new();
    let mut out: Vec<RelayEndpoint> = relay_data
        .endpoints
        .iter()
        .filter(|ep| is_outbound_relay_candidate(ep))
        .filter(|ep| seen.insert(ep.relay_name.clone()))
        .cloned()
        .collect();
    out.sort_by_key(|ep| ep.relay_id);
    out
}

/// The relay endpoint to connect the MEDIA transport to. `auth_token_id` only gates relaylatency
/// probes, so an offer that carries every endpoint with `auth_token_id=0` has no relaylatency
/// candidate yet must still connect for media. Prefer a relaylatency candidate, else any non-FNA
/// endpoint, else the first; otherwise the call is dropped with no media.
///
/// At each tier prefer a USABLE endpoint -- one with an IPv4 address to dial AND a token we hold --
/// so a preferred-but-unusable endpoint doesn't drop a call that a later fallback endpoint in the
/// same relay block could carry. If nothing is usable, fall back to the prior selection so
/// `CallConfig::from_relay` still surfaces a precise NoRelayIpv4/NoRelayToken error.
pub fn get_media_relay_endpoint(relay_data: &RelayData) -> Option<&RelayEndpoint> {
    // A sparse `token id=...` block pads the missing lower indices with empty Vecs (parse_indexed_tokens),
    // so an endpoint referencing a never-sent token_id has a present-but-empty slot. Treat that as no
    // token, or a padded slot would mask a later endpoint whose token the relay actually provided.
    let usable = |e: &RelayEndpoint| {
        get_primary_ipv4_address(e).is_some()
            && relay_data
                .relay_tokens
                .get(e.token_id as usize)
                .is_some_and(|t| !t.is_empty())
    };
    let pick = |usable_only: bool| {
        relay_data
            .endpoints
            .iter()
            .find(|e| is_outbound_relay_candidate(e) && (!usable_only || usable(e)))
            .or_else(|| {
                relay_data
                    .endpoints
                    .iter()
                    .find(|e| !e.is_fna && (!usable_only || usable(e)))
            })
            .or_else(|| {
                relay_data
                    .endpoints
                    .iter()
                    .find(|e| !usable_only || usable(e))
            })
    };
    pick(true).or_else(|| pick(false))
}

/// Raw 6-byte IPv4:port for relaylatency (prefers the verbatim te2 bytes).
pub fn get_ipv4_address_bytes(endpoint: &RelayEndpoint) -> Option<[u8; 6]> {
    if let Some(bytes) = endpoint.ipv4_te2_bytes {
        return Some(bytes);
    }
    for addr in &endpoint.addresses {
        let Some(ipv4) = &addr.ipv4 else { continue };
        let octets: Vec<u8> = ipv4.split('.').filter_map(|n| n.parse().ok()).collect();
        if octets.len() != 4 {
            continue;
        }
        let mut buf = [0u8; 6];
        buf[..4].copy_from_slice(&octets);
        buf[4] = (addr.port >> 8) as u8;
        buf[5] = addr.port as u8;
        return Some(buf);
    }
    None
}

pub fn get_primary_ipv4_address(endpoint: &RelayEndpoint) -> Option<(String, u16)> {
    endpoint
        .addresses
        .iter()
        .find_map(|a| a.ipv4.clone().map(|ip| (ip, a.port)))
}

/// ice-ufrag for the synthetic SDP: base64 of the raw auth_token bytes.
pub fn token_to_ice_ufrag(token_bytes: &[u8]) -> String {
    if token_bytes.is_empty() {
        return String::new();
    }
    BASE64_STANDARD.encode(token_bytes)
}

/// ice-pwd for the synthetic SDP: base64 of the relay key.
pub fn get_relay_key_for_sdp(relay_data: &RelayData) -> String {
    match &relay_data.relay_key {
        Some(k) if !k.is_empty() => BASE64_STANDARD.encode(k),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wacore_binary::Node;
    use wacore_binary::builder::NodeBuilder;

    fn as_ref(n: &Node) -> NodeRef<'_> {
        n.as_node_ref()
    }

    fn sample_relay() -> Node {
        let hbh30: Vec<u8> = (0x40u8..0x5e).collect(); // 30 bytes
        let key16: Vec<u8> = (0x10u8..0x20).collect(); // 16 bytes
        NodeBuilder::new("relay")
            .attr("uuid", "relay-uuid")
            .attr("self_pid", "11")
            .attr("peer_pid", "22")
            .children([
                NodeBuilder::new("key")
                    .bytes(BASE64_STANDARD.encode(&key16).into_bytes())
                    .build(),
                NodeBuilder::new("hbh_key")
                    .bytes(BASE64_STANDARD.encode(&hbh30).into_bytes())
                    .build(),
                NodeBuilder::new("warp_mi_tag_len")
                    .bytes(b"4".to_vec())
                    .build(),
                NodeBuilder::new("token")
                    .attr("id", "0")
                    .bytes(vec![0xaa, 0xbb, 0xcc])
                    .build(),
                NodeBuilder::new("auth_token")
                    .attr("id", "1")
                    .bytes(vec![0x11, 0x22])
                    .build(),
                NodeBuilder::new("te2")
                    .attr("relay_id", "1")
                    .attr("relay_name", "gru1c02")
                    .attr("token_id", "0")
                    .attr("auth_token_id", "1")
                    .attr("c2r_rtt", "33")
                    .bytes(vec![157, 240, 226, 133, 0x0d, 0x96])
                    .build(),
                // FNA inbound-only endpoint (auth_token_id=0).
                NodeBuilder::new("te2")
                    .attr("relay_id", "2")
                    .attr("relay_name", "fldb1")
                    .attr("auth_token_id", "0")
                    .attr("is_fna", "1")
                    .bytes(vec![10, 0, 0, 1, 0x0d, 0x96])
                    .build(),
            ])
            .build()
    }

    #[test]
    fn parses_keys_tokens_and_endpoints() {
        let node = sample_relay();
        let rd = parse_relay_data(&as_ref(&node)).unwrap();

        let hbh30: Vec<u8> = (0x40u8..0x5e).collect();
        let key16: Vec<u8> = (0x10u8..0x20).collect();
        assert_eq!(rd.hbh_key.as_deref(), Some(hbh30.as_slice()));
        assert_eq!(rd.relay_key.as_deref(), Some(key16.as_slice()));
        assert_eq!(rd.warp_mi_tag_len, Some(4));
        assert_eq!(rd.uuid.as_deref(), Some("relay-uuid"));
        assert_eq!(rd.self_pid, Some(11));
        assert_eq!(rd.peer_pid, Some(22));
        assert_eq!(rd.relay_tokens[0], vec![0xaa, 0xbb, 0xcc]);
        assert_eq!(rd.auth_tokens[1], vec![0x11, 0x22]);

        assert_eq!(rd.endpoints.len(), 2);
        let edge = &rd.endpoints[0];
        assert_eq!(edge.relay_name, "gru1c02");
        assert_eq!(edge.c2r_rtt_ms, Some(33));
        assert_eq!(edge.ipv4_te2_bytes, Some([157, 240, 226, 133, 0x0d, 0x96]));
        let addr = &edge.addresses[0];
        assert_eq!(addr.ipv4.as_deref(), Some("157.240.226.133"));
        assert_eq!(addr.port, 3478);
    }

    #[test]
    fn outbound_excludes_fna() {
        let node = sample_relay();
        let rd = parse_relay_data(&as_ref(&node)).unwrap();
        let outbound = get_outbound_relay_endpoints(&rd);
        assert_eq!(outbound.len(), 1);
        assert_eq!(outbound[0].relay_name, "gru1c02");
        assert!(is_outbound_relay_candidate(&rd.endpoints[0]));
        assert!(!is_outbound_relay_candidate(&rd.endpoints[1]));
    }

    #[test]
    fn media_relay_falls_back_when_no_relaylatency_candidate() {
        // Offer where every endpoint is non-FNA but auth_token_id=0 (only gates relaylatency):
        // get_outbound_relay_endpoints is empty, yet media must still connect to one.
        let node = NodeBuilder::new("relay")
            .children([
                NodeBuilder::new("auth_token")
                    .attr("id", "0")
                    .bytes(vec![0x01])
                    .build(),
                NodeBuilder::new("te2")
                    .attr("relay_id", "0")
                    .attr("relay_name", "for2c01")
                    .attr("token_id", "1")
                    .attr("auth_token_id", "0")
                    .bytes(vec![57, 144, 129, 57, 0x0d, 0x96])
                    .build(),
                NodeBuilder::new("te2")
                    .attr("relay_id", "1")
                    .attr("relay_name", "fra5c02")
                    .attr("token_id", "0")
                    .attr("auth_token_id", "0")
                    .bytes(vec![157, 240, 253, 133, 0x0d, 0x96])
                    .build(),
            ])
            .build();
        let rd = parse_relay_data(&as_ref(&node)).unwrap();
        assert!(get_outbound_relay_endpoints(&rd).is_empty());
        let media = get_media_relay_endpoint(&rd).expect("media relay must be selectable");
        assert_eq!(media.relay_name, "for2c01");
        assert!(!media.is_fna);
    }

    #[test]
    fn media_relay_skips_unusable_endpoint_for_a_usable_fallback() {
        let addr = |ip: &str| RelayAddress {
            protocol: 0,
            ipv4: Some(ip.into()),
            ipv6: None,
            port: 3478,
        };
        let ep = |name: &str, token_id: u32, ip: &str| RelayEndpoint {
            relay_name: name.into(),
            token_id,
            auth_token_id: 1, // outbound candidate
            addresses: vec![addr(ip)],
            ..RelayEndpoint::default()
        };
        // Only token index 0 exists; the preferred (first) endpoint references a missing token, so it
        // is unusable -- selection must skip it for the usable fallback rather than dropping the call.
        let rd = RelayData {
            relay_tokens: vec![vec![0xAA]],
            endpoints: vec![ep("unusable", 5, "1.2.3.4"), ep("usable", 0, "5.6.7.8")],
            ..RelayData::default()
        };
        let selected = get_media_relay_endpoint(&rd).expect("a usable endpoint must be selectable");
        assert_eq!(selected.relay_name, "usable");
    }

    #[test]
    fn media_relay_treats_padded_empty_token_slot_as_missing() {
        let addr = |ip: &str| RelayAddress {
            protocol: 0,
            ipv4: Some(ip.into()),
            ipv6: None,
            port: 3478,
        };
        let ep = |name: &str, token_id: u32, ip: &str| RelayEndpoint {
            relay_name: name.into(),
            token_id,
            auth_token_id: 1,
            addresses: vec![addr(ip)],
            ..RelayEndpoint::default()
        };
        // Only `token id="1"` was provided; parse_indexed_tokens pads index 0 with an empty Vec. The
        // preferred endpoint references that padded slot, so it must be skipped for the real token at 1.
        let rd = RelayData {
            relay_tokens: vec![Vec::new(), vec![0xAA]],
            endpoints: vec![ep("padded", 0, "1.2.3.4"), ep("real", 1, "5.6.7.8")],
            ..RelayData::default()
        };
        let selected =
            get_media_relay_endpoint(&rd).expect("the real-token endpoint must be picked");
        assert_eq!(selected.relay_name, "real");
    }

    #[test]
    fn ipv4_bytes_and_ufrag_and_pwd() {
        let node = sample_relay();
        let rd = parse_relay_data(&as_ref(&node)).unwrap();
        assert_eq!(
            get_ipv4_address_bytes(&rd.endpoints[0]),
            Some([157, 240, 226, 133, 0x0d, 0x96])
        );
        assert_eq!(
            get_primary_ipv4_address(&rd.endpoints[0]),
            Some(("157.240.226.133".to_string(), 3478))
        );
        assert_eq!(
            token_to_ice_ufrag(&[0xaa, 0xbb, 0xcc]),
            BASE64_STANDARD.encode([0xaa, 0xbb, 0xcc])
        );
        let key16: Vec<u8> = (0x10u8..0x20).collect();
        assert_eq!(get_relay_key_for_sdp(&rd), BASE64_STANDARD.encode(&key16));
    }

    #[test]
    fn te2_ipv6_and_raw_e2e() {
        let mut v6 = vec![0u8; 18];
        v6[0] = 0x20;
        v6[1] = 0x01;
        v6[16] = 0x0d;
        v6[17] = 0x96;
        let addr = parse_te2_address(&v6, 0).unwrap();
        assert_eq!(addr.ipv6.as_deref(), Some("2001:0:0:0:0:0:0:0"));
        assert_eq!(addr.port, 3478);
        assert!(parse_te2_address(&[1, 2, 3], 0).is_none());

        let raw: Vec<u8> = (0u8..40).collect();
        let b64 = BASE64_STANDARD.encode(&raw);
        assert_eq!(
            decode_raw_e2e_content(b64.as_bytes()).as_deref(),
            Some(raw.as_slice())
        );
        assert!(decode_raw_e2e_content(BASE64_STANDARD.encode([0u8; 8]).as_bytes()).is_none());
    }

    #[test]
    fn rejects_out_of_bound_token_id() {
        // A malicious relay sending a huge `id` must not force a giant allocation; the token is
        // ignored and a valid in-range token still parses.
        let node = NodeBuilder::new("relay")
            .children([
                NodeBuilder::new("token")
                    .attr("id", "4000000000")
                    .bytes(vec![0xde, 0xad])
                    .build(),
                NodeBuilder::new("token")
                    .attr("id", "0")
                    .bytes(vec![0xaa, 0xbb])
                    .build(),
            ])
            .build();
        let rd = parse_relay_data(&as_ref(&node)).unwrap();
        assert!(rd.relay_tokens.len() <= MAX_RELAY_TOKENS);
        assert_eq!(rd.relay_tokens[0], vec![0xaa, 0xbb]);
    }

    #[test]
    fn merge_prefers_patch() {
        let node = sample_relay();
        let base = parse_relay_data(&as_ref(&node)).unwrap();
        let patch = RelayData {
            hbh_key: Some(vec![9u8; 30]),
            ..Default::default()
        };
        let merged = merge_relay_data(base.clone(), patch);
        assert_eq!(merged.hbh_key, Some(vec![9u8; 30]));
        // Untouched fields fall back to base.
        assert_eq!(merged.relay_key, base.relay_key);
        assert_eq!(merged.endpoints.len(), base.endpoints.len());
    }
}

//! WhatsApp VoIP media-plane crypto: SRTP (E2E 1:1 + HBH), SFrame, WAHKDF, SSRC.
//!
//! Pure, no-Tokio, wasm-safe. Every primitive is pinned byte-for-byte against captured
//! vectors (testdata/kats.json) because the recv path has no authentication: round-trip
//! tests pass even when both directions are wrong identically, so known-answer vectors are
//! the only real guard.

pub mod demux;
pub mod driver;
pub mod e2e_srtp;
pub mod engine;
pub mod hbh_srtp;
pub mod mlow;
pub mod registry;
pub mod relay_parse;
pub mod rtcp;
pub mod rtp;
pub mod session;
pub mod sframe;
pub mod ssrc;
pub mod stun;
// Internal packet-capture facility. Now `pub(crate)` and wired by no non-test consumer, so its
// only live callers are its own tests; retained as a debugging primitive rather than deleted.
#[allow(dead_code)]
pub mod tap;
pub mod transport;
pub mod warp;

// Curated facade: the headline entry points a consumer reaches for, hoisted to `voip::`.
// The sub-modules stay `pub` for fine-grained wire helpers (parsers, attr builders), but
// these are the ones worth surfacing by name.
pub use demux::{RelayPacketKind, classify_relay_packet};
pub use driver::{CallChannels, run_call};
pub use engine::{
    CallConfig, CallEngine, CallEvent, EngineError, Input, Millis, NEVER, Output, SetupError,
    TxIdSource,
};
pub use mlow::{MlowDecoder, MlowEncoder};
// Internal: the inbound router's standard-Opus vs mlow discriminator (engine reaches it via `super`).
pub(crate) use mlow::is_standard_opus_frame;
pub use registry::CallRegistry;
pub use session::{CallDirection, CallPhase, CallSession, MediaPipeline, MediaPipelineParams};
pub use transport::{
    RelayDisconnectReason, RelayTransport, RelayTransportEvent, RelayTransportFactory,
};

// Internal crypto/framing primitives (`crypt_payload`, `derive_e2e_keys`, `SframeSession`,
// `SequentialTxIds`) are intentionally NOT re-exported here: a consumer composing its own SRTP
// from these would silently produce a broken (or insecure) stack. They stay reachable only as
// `#[doc(hidden)]` in their source modules so the in-tree benchmark crate can drive them.

use hkdf::Hkdf;
use sha2::Sha256;

/// HKDF-SHA256 (extract with `salt`, expand with `info`): the one KDF shape all of
/// WhatsApp's VoIP key derivations reduce to.
pub(crate) fn hkdf_sha256(salt: &[u8], ikm: &[u8], info: &[u8], len: usize) -> Vec<u8> {
    debug_assert!(len <= 255 * 32, "HKDF-SHA256 max output is 8160 bytes");
    let hk = Hkdf::<Sha256>::new(Some(salt), ikm);
    let mut okm = vec![0u8; len];
    hk.expand(info, &mut okm)
        .expect("HKDF length within bounds");
    okm
}

/// Device-qualified participant id used as HKDF `info` for both E2E-SRTP and SFrame: strip the
/// resource, keep an existing `:N@lid` device suffix, give bare `@lid` an implicit `:0`, and pass
/// everything else through unchanged.
pub(crate) fn format_participant_id(jid: &str) -> String {
    let bare = jid.split('/').next().unwrap_or(jid).trim();
    let Some(at) = bare.rfind('@') else {
        return bare.to_string();
    };
    if at == 0 {
        return bare.to_string();
    }
    let user = &bare[..at];
    let domain = &bare[at + 1..];
    if domain == "lid" && !user.contains(':') {
        return format!("{user}:0@{domain}");
    }
    bare.to_string()
}

/// LEB128 varint append (`SFrame` header + DC STUN attributes use the same encoding).
pub(crate) fn encode_varint(out: &mut Vec<u8>, value: u64) {
    let mut v = value;
    while v > 0x7f {
        out.push(((v & 0x7f) | 0x80) as u8);
        v >>= 7;
    }
    out.push((v & 0xff) as u8);
}

#[cfg(test)]
pub(crate) mod testkat {
    use serde_json::Value;

    /// Parsed known-answer-test vectors captured from a reference VoIP crypto stack.
    pub fn kats() -> Value {
        serde_json::from_str(include_str!("testdata/kats.json")).expect("kats.json must parse")
    }

    /// Hex-decode a string field reached by `keys`, e.g. `hexd(&k, &["sframe", "peerKey32"])`.
    pub fn hexd(v: &Value, keys: &[&str]) -> Vec<u8> {
        let mut cur = v;
        for k in keys {
            cur = &cur[*k];
        }
        let s = cur
            .as_str()
            .unwrap_or_else(|| panic!("kats path {keys:?} is not a string"));
        hex::decode(s).expect("kat field must be valid hex")
    }
}

#[cfg(test)]
mod fuzz_tests {
    use crate::voip::{relay_parse, rtcp, rtp, stun};
    use std::panic::catch_unwind;
    use wacore_binary::builder::NodeBuilder;

    /// Every byte-level parser must reject garbage by returning (never panicking): truncations,
    /// all-0xFF, and length-field lies that an off-by-one bounds check would index past. Guards the
    /// whole bounds-check class touched by the relay token-id bound and the STUN/RTP/RTCP parsers.
    fn garbage_buffers() -> Vec<Vec<u8>> {
        let seed: Vec<u8> = vec![
            0x80, 0xc8, 0x00, 0x06, 0xde, 0xad, 0xbe, 0xef, 0x21, 0x12, 0xa4, 0x42, 0x01, 0x02,
            0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x00, 0x09, 0x00, 0x08, 0x00, 0x00, 0x04, 0x01,
        ];
        let mut bufs: Vec<Vec<u8>> = Vec::new();
        // Every prefix truncation of the seed (0..=len bytes).
        for i in 0..=seed.len() {
            bufs.push(seed[..i].to_vec());
        }
        // All-0xFF buffers of assorted lengths (max-out every length field).
        for len in [0usize, 1, 2, 3, 7, 8, 11, 12, 19, 20, 21, 64, 255] {
            bufs.push(vec![0xFF; len]);
        }
        // Length-field lies: a valid-looking STUN/RTP header whose declared body/attr length runs
        // past the real buffer end.
        bufs.push(vec![
            0x00, 0x01, 0xff, 0xff, 0x21, 0x12, 0xa4, 0x42, 1, 2, 3, 4, 5, 6, 7, 8,
        ]); // STUN msg_len lie
        bufs.push(vec![
            0x00, 0x01, 0x00, 0x10, 0x21, 0x12, 0xa4, 0x42, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12,
            0x40, 0x00, 0xff, 0xff, 0xaa, // attr len 0xffff but no body
        ]);
        bufs.push(vec![
            0x90, 0x78, 0x12, 0x34, 0, 0, 0, 0, 0, 0, 0, 0, 0xde, 0xbe, 0xff, 0xff,
        ]); // RTP ext_words lie
        bufs.push(vec![0x90, 0x78, 0x12, 0x34, 0, 0, 0, 0, 0, 0, 0, 0, 0x10]); // X=1, CSRC overrun
        bufs
    }

    #[test]
    fn parsers_dont_panic_on_garbage() {
        for buf in garbage_buffers() {
            let b = buf.clone();
            let res = catch_unwind(move || {
                // STUN
                let _ = stun::is_stun_packet(&b);
                let _ = stun::stun_message_type(&b);
                let _ = stun::stun_transaction_id(&b);
                let _ = stun::parse_stun_attributes(&b);
                let _ = stun::parse_stun_error_code(&b);
                let _ = stun::is_allocate_or_binding_success(&b);
                let _ = stun::is_allocate_error(&b);
                let _ = stun::is_whatsapp_pong(&b, None);
                // RTP
                let _ = rtp::is_rtp_version2(&b);
                let _ = rtp::rtp_header_byte_length(&b);
                let _ = rtp::parse_rtp_header(&b);
                // RTCP
                let _ = rtcp::is_rtcp_packet(&b);
                let _ = rtcp::rtcp_payload_type(&b);
                let _ = rtcp::parse_rtcp_sender_ssrc(&b);
            });
            assert!(res.is_ok(), "parser panicked on garbage {:02x?}", buf);
        }
    }

    #[test]
    fn relay_parse_does_not_panic_on_garbage() {
        // Garbage `<relay>` nodes exercise parse_indexed_tokens (huge / non-numeric ids), the te2
        // address parse (wrong byte lengths), and non-UTF-8 key content; none may panic.
        let nodes = [
            // Out-of-range and non-numeric token ids + non-UTF-8 content.
            NodeBuilder::new("relay")
                .children([
                    NodeBuilder::new("token")
                        .attr("id", "999999999999999999")
                        .bytes(vec![0xff, 0xfe])
                        .build(),
                    NodeBuilder::new("token")
                        .attr("id", "not-a-number")
                        .bytes(vec![0x80, 0x81])
                        .build(),
                    NodeBuilder::new("auth_token")
                        .attr("id", "-1")
                        .bytes(vec![0xc0])
                        .build(),
                    NodeBuilder::new("key")
                        .bytes(vec![0xff, 0xff, 0xff])
                        .build(),
                    NodeBuilder::new("hbh_key").bytes(vec![0x00; 5]).build(),
                    NodeBuilder::new("warp_mi_tag_len")
                        .bytes(vec![0xff, 0x01])
                        .build(),
                    // te2 with a malformed (non 6/18) address length and garbage attrs.
                    NodeBuilder::new("te2")
                        .attr("relay_id", "abc")
                        .attr("protocol", "999")
                        .bytes(vec![0x01, 0x02, 0x03])
                        .build(),
                ])
                .build(),
            // Empty relay node.
            NodeBuilder::new("relay").build(),
        ];
        for node in &nodes {
            let nr = node.as_node_ref();
            let res = catch_unwind(|| {
                let _ = relay_parse::parse_relay_data(&nr);
                let _ = relay_parse::decode_hbh_key(&[0xff; 8]);
                let _ = relay_parse::decode_relay_key_content(&[0xff; 8]);
                let _ = relay_parse::decode_raw_e2e_content(&[0xff; 8]);
            });
            assert!(res.is_ok(), "relay parser panicked on garbage node");
        }
    }
}

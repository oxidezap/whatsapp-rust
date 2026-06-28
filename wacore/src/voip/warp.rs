//! WARP RTP extension constants and the WARP MESSAGE-INTEGRITY tag.
//!
//! wacrg spec: warp-crypto (CRY-07), warp relay framing (REL-03). The MI tag is keyed
//! by the per-participant SRTP auth key (KAT-pinned), NOT a separate callKey-derived
//! "warp auth key" as the spec's `derive_warp_auth_key` documents; don't "fix" it
//! toward the spec without re-checking the vectors.

use hmac::{Hmac, KeyInit, Mac};
use sha1::Sha1;

type HmacSha1 = Hmac<Sha1>;

pub const WARP_AUDIO_PIGGYBACK_EXT: [u8; 4] = [0x30, 0x01, 0x00, 0x00];
pub const WARP_MI_TAG_LEN: usize = 4;
/// Packets #1-2 carry an empty extension; #3+ (0-based index >= 2) piggyback.
pub const WARP_PIGGYBACK_START_PACKET: usize = 2;

/// Audio piggyback extension word for `packet_index`, or `None` for the first packets.
pub fn audio_piggyback_extension_for(
    packet_index: usize,
    enabled: bool,
    start_packet: usize,
) -> Option<u32> {
    if !enabled || packet_index < start_packet {
        return None;
    }
    Some(u32::from_be_bytes(WARP_AUDIO_PIGGYBACK_EXT))
}

/// WARP MI tag = first `tag_len` bytes of HMAC-SHA1(auth_key, packet || roc_be32).
pub fn compute_warp_mi_tag(
    auth_key: &[u8],
    packet_without_tag: &[u8],
    roc: u32,
    tag_len: usize,
) -> Vec<u8> {
    let mut mac = HmacSha1::new_from_slice(auth_key).expect("HMAC accepts any key length");
    mac.update(packet_without_tag);
    mac.update(&roc.to_be_bytes());
    let full = mac.finalize().into_bytes();
    full[..tag_len].to_vec()
}

/// Append the WARP MI tag to a protected packet.
pub fn append_warp_mi_tag(
    auth_key: &[u8],
    packet_without_tag: &[u8],
    roc: u32,
    tag_len: usize,
) -> Vec<u8> {
    let tag = compute_warp_mi_tag(auth_key, packet_without_tag, roc, tag_len);
    let mut out = Vec::with_capacity(packet_without_tag.len() + tag.len());
    out.extend_from_slice(packet_without_tag);
    out.extend_from_slice(&tag);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voip::testkat::{hexd, kats};

    #[test]
    fn warp_mi_tag_matches_kat() {
        let k = kats();
        let auth_key = hexd(&k, &["e2e_srtp", "peer_authKey"]);
        let packet = hexd(&k, &["inputs", "samplePacket"]);
        let roc = k["inputs"]["roc"].as_u64().unwrap() as u32;
        let tag = compute_warp_mi_tag(&auth_key, &packet, roc, WARP_MI_TAG_LEN);
        assert_eq!(
            hex::encode(tag),
            k["e2e_srtp"]["warp_mi_tag4"].as_str().unwrap()
        );
    }
}

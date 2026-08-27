//! WARP RTP extension constants and the WARP MESSAGE-INTEGRITY tag.
//!
//! wacrg spec: warp-crypto (CRY-07), warp relay framing (REL-03). The MI tag is keyed
//! by the per-participant SRTP auth key (KAT-pinned), NOT a separate callKey-derived
//! "warp auth key" as the spec's `derive_warp_auth_key` documents; don't "fix" it
//! toward the spec without re-checking the vectors.

use hmac::{Hmac, KeyInit, Mac};
use sha1::Sha1;
use subtle::ConstantTimeEq;

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

/// HMAC-SHA1's digest size, and so the largest WARP MI tag that can exist. Tags are
/// computed into a buffer this size on the stack and truncated to the negotiated
/// `tag_len` (4 on every live call), which keeps the per-packet send and recv paths
/// off the allocator.
pub const WARP_MI_TAG_MAX_LEN: usize = 20;

/// WARP MI tag material: `HMAC-SHA1(auth_key, packet || roc_be32)`. The tag on the wire is
/// the first `tag_len` bytes of the returned digest; returning the whole thing by value lets
/// both the send and the recv path slice it in place instead of allocating for four bytes.
pub fn compute_warp_mi_tag(
    auth_key: &[u8],
    packet_without_tag: &[u8],
    roc: u32,
) -> [u8; WARP_MI_TAG_MAX_LEN] {
    let mut mac = HmacSha1::new_from_slice(auth_key).expect("HMAC accepts any key length");
    mac.update(packet_without_tag);
    mac.update(&roc.to_be_bytes());
    let mut tag = [0u8; WARP_MI_TAG_MAX_LEN];
    tag.copy_from_slice(&mac.finalize().into_bytes());
    tag
}

/// Constant-time-verify a received WARP MI tag against the one we compute for
/// `roc`. Callers must reject a packet whose tag fails BEFORE folding recv ROC
/// state, so an unauthenticated packet can't desync the rollover counter
/// (RFC 3711 §3.3.1: update the index only after authentication).
pub fn verify_warp_mi_tag(
    auth_key: &[u8],
    packet_without_tag: &[u8],
    roc: u32,
    tag_len: usize,
    received_tag: &[u8],
) -> bool {
    // A length mismatch is a rejection, not a panic: `tag_len` comes off the negotiated
    // call config and `received_tag` off the wire.
    if tag_len > WARP_MI_TAG_MAX_LEN || received_tag.len() != tag_len {
        return false;
    }
    let expected = compute_warp_mi_tag(auth_key, packet_without_tag, roc);
    expected[..tag_len].ct_eq(received_tag).into()
}

/// Append the WARP MI tag over everything already in `packet`. The tag covers the bytes
/// on hand, so this must run last, once the header and ciphertext are both written --
/// which is also what lets the whole protected packet live in one allocation.
pub fn append_warp_mi_tag_in_place(
    auth_key: &[u8],
    packet: &mut Vec<u8>,
    roc: u32,
    tag_len: usize,
) {
    // Both media pipelines reject a `tag_len` outside 1..=20 in their constructors, so a built
    // pipeline can never reach the clamp below. It stays as a release-mode floor because this
    // runs per packet on the send path, where a panic would take the call down; the assert is
    // what surfaces the misuse to a direct caller, since a send/recv length disagreement
    // otherwise shows up only as every inbound packet failing to authenticate.
    debug_assert!(
        (1..=WARP_MI_TAG_MAX_LEN).contains(&tag_len),
        "WARP MI tag_len must be 1..=20, got {tag_len}"
    );
    let tag = compute_warp_mi_tag(auth_key, packet, roc);
    packet.extend_from_slice(&tag[..tag_len.clamp(1, WARP_MI_TAG_MAX_LEN)]);
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
        let tag = compute_warp_mi_tag(&auth_key, &packet, roc);
        assert_eq!(
            hex::encode(&tag[..WARP_MI_TAG_LEN]),
            k["e2e_srtp"]["warp_mi_tag4"].as_str().unwrap()
        );
    }

    #[test]
    fn verify_rejects_forged_and_misshapen_tags() {
        let k = kats();
        let auth_key = hexd(&k, &["e2e_srtp", "peer_authKey"]);
        let packet = hexd(&k, &["inputs", "samplePacket"]);
        let roc = k["inputs"]["roc"].as_u64().unwrap() as u32;
        let tag = compute_warp_mi_tag(&auth_key, &packet, roc);
        let good = &tag[..WARP_MI_TAG_LEN];
        assert!(verify_warp_mi_tag(
            &auth_key,
            &packet,
            roc,
            WARP_MI_TAG_LEN,
            good
        ));

        let mut forged = good.to_vec();
        forged[0] ^= 1;
        assert!(!verify_warp_mi_tag(
            &auth_key,
            &packet,
            roc,
            WARP_MI_TAG_LEN,
            &forged
        ));
        // A different ROC is a different tag, which is what stops a relay replaying
        // a packet across a rollover.
        assert!(!verify_warp_mi_tag(
            &auth_key,
            &packet,
            roc.wrapping_add(1),
            WARP_MI_TAG_LEN,
            good
        ));
        // A short/long tag from the wire, or a `tag_len` past the digest, is a rejection
        // rather than a panic.
        assert!(!verify_warp_mi_tag(
            &auth_key,
            &packet,
            roc,
            WARP_MI_TAG_LEN,
            &good[..3]
        ));
        assert!(!verify_warp_mi_tag(
            &auth_key,
            &packet,
            roc,
            WARP_MI_TAG_MAX_LEN + 1,
            &tag
        ));
    }

    #[test]
    fn append_in_place_writes_exactly_the_tag() {
        let k = kats();
        let auth_key = hexd(&k, &["e2e_srtp", "peer_authKey"]);
        let packet = hexd(&k, &["inputs", "samplePacket"]);
        let roc = k["inputs"]["roc"].as_u64().unwrap() as u32;
        let mut framed = packet.clone();
        append_warp_mi_tag_in_place(&auth_key, &mut framed, roc, WARP_MI_TAG_LEN);
        assert_eq!(framed.len(), packet.len() + WARP_MI_TAG_LEN);
        assert_eq!(&framed[..packet.len()], &packet[..]);
        assert!(verify_warp_mi_tag(
            &auth_key,
            &packet,
            roc,
            WARP_MI_TAG_LEN,
            &framed[packet.len()..]
        ));
    }
}

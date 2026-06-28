//! E2E 1:1 SRTP, the primary working path. Keys derive from `callKey` (32B) +
//! participant LID via HKDF-SHA256, then an AES-CM PRF; payloads use AES-128-CTR
//! with a 4-byte WARP MESSAGE-INTEGRITY tag (HMAC-SHA1, not verified on recv).
//!
//! wacrg spec: srtp-e2e (CRY-05), srtp-master-key (CRY-02), call-key (CRY-01).

use aes::Aes128;
use ctr::Ctr128BE;
use ctr::cipher::{KeyIvInit, StreamCipher};

use crate::voip::hkdf_sha256;
pub use crate::voip::warp::{WARP_MI_TAG_LEN, append_warp_mi_tag, compute_warp_mi_tag};

type AesCtr = Ctr128BE<Aes128>;

/// Session keys for the E2E 1:1 SRTP cipher. Doc-hidden: an internal primitive, surfaced only so
/// the in-tree benchmark crate can drive `crypt_payload`/`derive_e2e_keys`.
#[doc(hidden)]
#[derive(Clone)]
pub struct E2eSrtpKeys {
    pub cipher_key: [u8; 16],
    pub salt: [u8; 14],
    pub auth_key: [u8; 20],
}

// Manual Debug so a stray `{:?}` can't leak the session keys.
impl core::fmt::Debug for E2eSrtpKeys {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("E2eSrtpKeys([redacted])")
    }
}

/// AES-CM PRF (libsrtp KDF): IV = master_salt (14B) with `label` XORed into byte 7,
/// zero-padded to 16, then AES-128-CTR keystream over `len` zero bytes.
fn aes_cm_kdf(master_key: &[u8], master_salt: &[u8], label: u8, len: usize) -> Vec<u8> {
    let mut iv = [0u8; 16];
    iv[..14].copy_from_slice(&master_salt[..14]);
    iv[7] ^= label;
    let mut out = vec![0u8; len];
    let mut cipher = AesCtr::new_from_slices(master_key, &iv).expect("16-byte key/iv");
    cipher.apply_keystream(&mut out);
    out
}

fn derive_session_keys_from_master(master: &[u8]) -> E2eSrtpKeys {
    let master_key = &master[0..16];
    let master_salt = &master[16..30];
    let mut keys = E2eSrtpKeys {
        cipher_key: [0u8; 16],
        salt: [0u8; 14],
        auth_key: [0u8; 20],
    };
    keys.cipher_key
        .copy_from_slice(&aes_cm_kdf(master_key, master_salt, 0x00, 16));
    keys.auth_key
        .copy_from_slice(&aes_cm_kdf(master_key, master_salt, 0x01, 20));
    keys.salt
        .copy_from_slice(&aes_cm_kdf(master_key, master_salt, 0x02, 14));
    keys
}

/// E2E 1:1 keys from `call_key` (>= 32B) and a participant LID (HKDF `info`).
/// The `info` is the *sender's* own participant id, so a caller derives the send keys from the
/// self LID and the recv keys from the peer LID (note SFrame uses the opposite convention).
/// Returns `None` when `call_key` is shorter than 32 bytes (a malformed peer callKey).
#[doc(hidden)]
pub fn derive_e2e_keys(call_key: &[u8], participant_lid: &str) -> Option<E2eSrtpKeys> {
    if call_key.len() < 32 {
        return None;
    }
    let master = hkdf_sha256(&[0u8; 32], &call_key[..32], participant_lid.as_bytes(), 46);
    Some(derive_session_keys_from_master(&master))
}

/// E2E RTP IV: salt right-aligned into 16 bytes, then SSRC XORed at bytes 4-7 and the
/// 48-bit packet index (ROC<<16 | seq) XORed at bytes 8-13.
pub fn build_e2e_rtp_iv(salt: &[u8], ssrc: u32, roc: u32, seq: u16) -> [u8; 16] {
    let mut iv = [0u8; 16];
    // Salt right-aligns into bytes [0..14]. Clamp so an oversized salt can't underflow `off` or panic
    // the slice; the only production caller passes the 14-byte E2eSrtpKeys.salt (n = 14, off = 0).
    let n = salt.len().min(14);
    let off = 14 - n;
    iv[off..off + n].copy_from_slice(&salt[..n]);
    iv[4] ^= (ssrc >> 24) as u8;
    iv[5] ^= (ssrc >> 16) as u8;
    iv[6] ^= (ssrc >> 8) as u8;
    iv[7] ^= ssrc as u8;
    let packet_index = (roc as u64) * 0x1_0000 + (seq as u64);
    let hi16 = ((packet_index >> 32) & 0xffff) as u16;
    let lo32 = (packet_index & 0xffff_ffff) as u32;
    iv[8] ^= (hi16 >> 8) as u8;
    iv[9] ^= hi16 as u8;
    iv[10] ^= (lo32 >> 24) as u8;
    iv[11] ^= (lo32 >> 16) as u8;
    iv[12] ^= (lo32 >> 8) as u8;
    iv[13] ^= lo32 as u8;
    iv
}

/// AES-128-CTR encrypt/decrypt of an RTP payload (the cipher is symmetric).
#[doc(hidden)]
pub fn crypt_payload(keys: &E2eSrtpKeys, ssrc: u32, seq: u16, roc: u32, payload: &[u8]) -> Vec<u8> {
    let iv = build_e2e_rtp_iv(&keys.salt, ssrc, roc, seq);
    let mut out = payload.to_vec();
    let mut cipher = AesCtr::new_from_slices(&keys.cipher_key, &iv).expect("16-byte key/iv");
    cipher.apply_keystream(&mut out);
    out
}

/// Send-side ROC tracker for monotonic 16-bit sequence numbers.
#[derive(Default)]
pub(crate) struct RocTracker {
    roc: u32,
    last_seq: u16,
    initialized: bool,
}

impl RocTracker {
    pub fn advance(&mut self, seq: u16) -> u32 {
        if !self.initialized {
            self.last_seq = seq;
            self.initialized = true;
            return self.roc;
        }
        // A signed 16-bit gap below -32768 is the wrap (seq jumped backward past the half-range).
        if (seq as i32 - self.last_seq as i32) < -32768 {
            self.roc = self.roc.wrapping_add(1);
        }
        self.last_seq = seq;
        self.roc
    }
}

/// Recv-side ROC estimator (RFC 3711 §3.3.1 guess-index). Unlike the monotonic send tracker it
/// tolerates reorder/loss: each packet's ROC is guessed from the highest seq seen, so a late
/// packet straddling a wrap decrypts under the right (lower) ROC without poisoning the state.
#[derive(Default)]
pub(crate) struct RecvRocTracker {
    roc: u32,
    s_l: u16,
    initialized: bool,
}

impl RecvRocTracker {
    /// Guess the ROC for `seq` and fold it into the state. Seeds from the first packet (roc=0).
    pub fn guess_roc(&mut self, seq: u16) -> u32 {
        if !self.initialized {
            self.s_l = seq;
            self.initialized = true;
            return self.roc;
        }
        // Pick v in {roc-1, roc, roc+1} so 2^16*v+seq is closest to 2^16*roc+s_l. The signed 16-bit
        // gap (not a modular wrapping_sub) is what distinguishes "next-but-reordered" from "wrapped".
        let v = if self.s_l < 0x8000 {
            if (seq as i32 - self.s_l as i32) > 0x8000 {
                self.roc.wrapping_sub(1) // old packet from before the origin (roc-1)
            } else {
                self.roc
            }
        } else if (self.s_l as i32 - seq as i32) > 0x8000 {
            self.roc.wrapping_add(1) // forward wrap into roc+1
        } else {
            self.roc
        };
        if v == self.roc {
            if seq > self.s_l {
                self.s_l = seq;
            }
        } else if v == self.roc.wrapping_add(1) {
            self.roc = v;
            self.s_l = seq;
        }
        // v == roc-1 (reordered late packet): return the lower ROC, leave state untouched.
        v
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voip::testkat::{hexd, kats};

    fn keys_from(k: &serde_json::Value, who: &str) -> E2eSrtpKeys {
        let mut keys = E2eSrtpKeys {
            cipher_key: [0u8; 16],
            salt: [0u8; 14],
            auth_key: [0u8; 20],
        };
        keys.cipher_key
            .copy_from_slice(&hexd(k, &["e2e_srtp", &format!("{who}_cipherKey")]));
        keys.salt
            .copy_from_slice(&hexd(k, &["e2e_srtp", &format!("{who}_salt")]));
        keys.auth_key
            .copy_from_slice(&hexd(k, &["e2e_srtp", &format!("{who}_authKey")]));
        keys
    }

    #[test]
    fn derive_e2e_keys_matches_kat() {
        let k = kats();
        let call_key = hexd(&k, &["inputs", "callKey"]);
        let peer = derive_e2e_keys(&call_key, k["inputs"]["peerLid"].as_str().unwrap()).unwrap();
        let expect = keys_from(&k, "peer");
        assert_eq!(peer.cipher_key, expect.cipher_key, "peer cipher_key");
        assert_eq!(peer.salt, expect.salt, "peer salt");
        assert_eq!(peer.auth_key, expect.auth_key, "peer auth_key");

        let self_keys =
            derive_e2e_keys(&call_key, k["inputs"]["selfLid"].as_str().unwrap()).unwrap();
        let expect_self = keys_from(&k, "self");
        assert_eq!(
            self_keys.cipher_key, expect_self.cipher_key,
            "self cipher_key"
        );
        assert_eq!(self_keys.auth_key, expect_self.auth_key, "self auth_key");
    }

    #[test]
    fn rtp_iv_matches_kat() {
        let k = kats();
        let peer = keys_from(&k, "peer");
        let ssrc = k["inputs"]["ssrc"].as_u64().unwrap() as u32;
        let seq = k["inputs"]["seq"].as_u64().unwrap() as u16;
        let roc = k["inputs"]["roc"].as_u64().unwrap() as u32;
        let iv = build_e2e_rtp_iv(&peer.salt, ssrc, roc, seq);
        assert_eq!(hex::encode(iv), k["e2e_srtp"]["rtpIv"].as_str().unwrap());
    }

    #[test]
    fn roc_tracker_wraps() {
        // --- send-side monotonic tracker ---
        let mut tx = RocTracker::default();
        assert_eq!(tx.advance(0xFFFE), 0); // seed
        assert_eq!(tx.advance(0xFFFF), 0);
        assert_eq!(tx.advance(0x0000), 1, "0xFFFF→0x0000 bumps ROC");
        assert_eq!(tx.advance(0x0001), 1);
        // Small out-of-order dip must NOT bump.
        assert_eq!(tx.advance(0x0000), 1, "a backward dip does not bump ROC");
        assert_eq!(tx.advance(0x0001), 1);
        // Walk to a second wrap → ROC=2.
        for s in [0x7000u16, 0xE000, 0xFFFF] {
            tx.advance(s);
        }
        assert_eq!(tx.advance(0x0000), 2, "second wrap gives ROC=2");

        // --- recv-side guess tracker ---
        let mut rx = RecvRocTracker::default();
        assert_eq!(rx.guess_roc(0xFFFE), 0); // seed (roc=0, s_l=0xFFFE)
        assert_eq!(rx.guess_roc(0xFFFF), 0);
        assert_eq!(rx.guess_roc(0x0000), 1, "0xFFFF→0x0000 bumps ROC");
        assert_eq!(rx.guess_roc(0x0001), 1);
        // Reordered small dip in the same ROC must NOT bump, and must not corrupt s_l.
        assert_eq!(
            rx.guess_roc(0x0000),
            1,
            "a reordered dip stays in the same ROC"
        );
        assert_eq!(rx.guess_roc(0x0002), 1, "state intact after the dip");
        // Walk forward (< 2^15 steps) to the high range, then wrap again → ROC=2.
        for s in [0x7000u16, 0xE000, 0xFFFF] {
            assert_eq!(rx.guess_roc(s), 1);
        }
        assert_eq!(rx.guess_roc(0x0000), 2, "second wrap gives ROC=2");
        // A late packet from just before the last wrap returns the LOWER ROC without corrupting
        // state: the next in-order packet still guesses ROC=2.
        assert_eq!(
            rx.guess_roc(0xFFF0),
            1,
            "reordered late packet returns the lower ROC"
        );
        assert_eq!(
            rx.guess_roc(0x0001),
            2,
            "state not corrupted by the late packet"
        );
    }

    #[test]
    fn crypt_payload_matches_kat() {
        let k = kats();
        let peer = keys_from(&k, "peer");
        let ssrc = k["inputs"]["ssrc"].as_u64().unwrap() as u32;
        let seq = k["inputs"]["seq"].as_u64().unwrap() as u16;
        let roc = k["inputs"]["roc"].as_u64().unwrap() as u32;
        let payload = hexd(&k, &["inputs", "payload"]);
        let ct = crypt_payload(&peer, ssrc, seq, roc, &payload);
        assert_eq!(
            hex::encode(&ct),
            k["e2e_srtp"]["cipher_out"].as_str().unwrap()
        );
        // Symmetric: decrypt round-trips.
        let pt = crypt_payload(&peer, ssrc, seq, roc, &ct);
        assert_eq!(pt, payload);
    }

    #[test]
    fn build_iv_tolerates_oversized_salt() {
        // Latent-panic guard: a salt longer than 14 bytes must not underflow `off` or panic the
        // slice. (The only production caller passes the 14-byte E2eSrtpKeys.salt.)
        let _ = build_e2e_rtp_iv(&[0u8; 16], 0x0102_0304, 0, 0);
        let _ = build_e2e_rtp_iv(&[0xABu8; 32], 0xdead_beef, 7, 0xFFFF);
        // The valid 14-byte path is unchanged: the salt lands left-aligned at bytes [0..14].
        let iv = build_e2e_rtp_iv(&[0x11u8; 14], 0, 0, 0);
        assert_eq!(&iv[0..14], &[0x11u8; 14]);
        assert_eq!(&iv[14..16], &[0u8; 2]);
    }

    #[test]
    fn crypt_payload_roundtrips_across_seq_wrap() {
        // The keystream stays aligned across a 16-bit sequence wrap: packets straddling 0xFFFF→0x0000
        // each decrypt under the ROC the recv estimator guesses, even with a small post-wrap reorder.
        // This is the send/recv ROC contract MediaPipeline depends on, exercised through the cipher.
        let keys = E2eSrtpKeys {
            cipher_key: [7u8; 16],
            salt: [9u8; 14],
            auth_key: [0u8; 20],
        };
        let ssrc = 0x5741_0001u32;
        let seqs = [0xFFFEu16, 0xFFFF, 0x0000, 0x0001];
        let plaintexts: Vec<Vec<u8>> = (0..4u8).map(|i| vec![i.wrapping_mul(37); 40]).collect();

        // Sender: monotonic seqs; the send ROC bumps at the wrap.
        let mut send_roc = RocTracker::default();
        let sent: Vec<(u16, Vec<u8>)> = seqs
            .iter()
            .zip(&plaintexts)
            .map(|(&seq, pt)| {
                let roc = send_roc.advance(seq);
                (seq, crypt_payload(&keys, ssrc, seq, roc, pt))
            })
            .collect();

        // Receiver sees a small post-wrap reorder (0x0000 and 0x0001 swapped); guess-index recovers
        // the right ROC for each, and the symmetric cipher must return the original plaintext.
        let mut recv_roc = RecvRocTracker::default();
        for &i in &[0usize, 1, 3, 2] {
            let (seq, ct) = &sent[i];
            let roc = recv_roc.guess_roc(*seq);
            let recovered = crypt_payload(&keys, ssrc, *seq, roc, ct);
            assert_eq!(
                &recovered, &plaintexts[i],
                "seq {seq:#06x} must decrypt across the wrap"
            );
        }
    }
}

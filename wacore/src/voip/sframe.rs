//! SFrame end-to-end media encryption (AES-128-GCM) over the relay.
//!
//! Three byte-level traps that silently fail if done the obvious way (all pinned by KATs):
//!   1. GCM nonce = 8 zero bytes || counter as u64 LITTLE-endian (16-byte nonce, GHASH-derived J0).
//!   2. The SFrame header is appended AFTER the ciphertext+tag and is NOT GCM AAD.
//!   3. Wire layout is `[ciphertext || 16-byte tag || varint-header]`.
//!
//! wacrg spec: sframe-media (CRY-04).

use aes_gcm::aes::Aes128;
use aes_gcm::aes::cipher::consts::U16;
use aes_gcm::{AesGcm, KeyInit, Nonce, aead::Aead};

use crate::voip::{encode_varint, hkdf_sha256};

/// AES-128-GCM with WhatsApp's non-standard 16-byte nonce.
type Aes128Gcm16 = AesGcm<Aes128, U16>;

pub const KDF_LABEL_E2E_SFRAME: &str = "e2e sframe key";
const GCM_TAG_LEN: usize = 16;
const AES_KEY_LEN: usize = 16;

fn split_call_key(call_key: &[u8]) -> Option<(&[u8], &[u8])> {
    if call_key.len() != 32 {
        return None;
    }
    Some((&call_key[0..16], &call_key[16..32]))
}

/// Android appends the participant JID to the label: `e2e sframe key<id>`.
/// Primary `@lid` without a device suffix uses `:0`. Intentionally a separate protocol surface from
/// E2E-SRTP's variant; they coincide today, so both delegate to one helper. Un-shim here if SFrame
/// ever needs to diverge.
pub fn format_sframe_participant_id(jid: &str) -> String {
    crate::voip::format_participant_id(jid)
}

pub fn sframe_info_label(participant_id: &str) -> String {
    format!("{KDF_LABEL_E2E_SFRAME}{participant_id}")
}

/// Per-participant SFrame key (Android apk / mbedtls path). `None` unless callKey is 32B.
pub fn derive_e2e_sframe_key_for_participant(
    call_key: &[u8],
    participant_id: &str,
) -> Option<Vec<u8>> {
    let (salt, ikm) = split_call_key(call_key)?;
    // Android derive_sframe_key == standard HKDF-SHA256 expand with the label as `info`.
    Some(hkdf_sha256(
        salt,
        ikm,
        sframe_info_label(participant_id).as_bytes(),
        32,
    ))
}

fn decode_varint(data: &[u8], offset: usize) -> Option<(u64, usize)> {
    let mut value: u64 = 0;
    let mut shift: u32 = 0;
    let mut i = offset;
    while i < data.len() {
        let b = data[i];
        i += 1;
        value |= ((b & 0x7f) as u64) << shift;
        if b & 0x80 == 0 {
            return Some((value, i));
        }
        shift += 7;
        if shift > 63 {
            return None;
        }
    }
    None
}

/// GCM nonce: 16 bytes = 8 zero bytes followed by `counter` as little-endian u64.
fn counter_to_iv(counter: u64) -> [u8; 16] {
    let mut iv = [0u8; 16];
    iv[8..16].copy_from_slice(&counter.to_le_bytes());
    iv
}

fn build_sframe_header(counter: u64, key_id: u64) -> Vec<u8> {
    let mut header = Vec::new();
    encode_varint(&mut header, counter);
    encode_varint(&mut header, key_id);
    let total_len = header.len() + 1;
    header.push(total_len as u8);
    header
}

fn parse_sframe_header(header: &[u8]) -> Option<(u64, u64)> {
    if header.len() < 2 {
        return None;
    }
    let total_len = *header.last().unwrap() as usize;
    if total_len != header.len() {
        return None;
    }
    let body = &header[..header.len() - 1];
    let (counter, next) = decode_varint(body, 0)?;
    let (key_id, _) = decode_varint(body, next)?;
    Some((counter, key_id))
}

fn gcm_encrypt(key: &[u8], nonce16: &[u8; 16], plaintext: &[u8]) -> Vec<u8> {
    let cipher = Aes128Gcm16::new_from_slice(&key[..AES_KEY_LEN]).expect("16-byte key");
    let nonce = Nonce::<U16>::from(*nonce16);
    cipher
        .encrypt(&nonce, plaintext)
        .expect("AES-GCM encrypt is infallible for valid key/nonce")
}

fn gcm_decrypt(key: &[u8], nonce16: &[u8; 16], ciphertext_with_tag: &[u8]) -> Option<Vec<u8>> {
    let cipher = Aes128Gcm16::new_from_slice(&key[..AES_KEY_LEN]).ok()?;
    let nonce = Nonce::<U16>::from(*nonce16);
    cipher.decrypt(&nonce, ciphertext_with_tag).ok()
}

/// Outcome of [`SframeSession::decrypt`]. Discrimination is by GCM authentication, not a payload
/// heuristic: a frame that parses as a valid SFrame header AND authenticates is `Decrypted`;
/// anything else is `Plaintext`: the peer ships plain Opus inside E2E-SRTP without SFrame-wrapping,
/// so those bytes are the payload and must be used verbatim.
#[doc(hidden)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SframeIn {
    /// The frame was a real SFrame frame; GCM auth passed. Inner is the recovered plaintext.
    Decrypted(Vec<u8>),
    /// Not an authenticatable SFrame frame; the raw frame bytes are the plaintext payload.
    Plaintext,
}

/// SFrame send/recv with the per-direction keys (encrypt for peer, decrypt for self).
/// Doc-hidden: an internal primitive, surfaced only for the in-tree benchmark crate.
#[doc(hidden)]
pub struct SframeSession {
    encrypt_key: [u8; AES_KEY_LEN],
    decrypt_key: [u8; AES_KEY_LEN],
    tx_counter: u64,
    pub self_participant_id: String,
    pub peer_participant_id: String,
}

impl SframeSession {
    /// Build from `call_key` and the self/peer JIDs. `None` unless callKey is 32B.
    pub fn new(call_key: &[u8], self_jid: &str, peer_jid: &str) -> Option<Self> {
        let self_id = format_sframe_participant_id(self_jid);
        let peer_id = format_sframe_participant_id(peer_jid);
        let send_key = derive_e2e_sframe_key_for_participant(call_key, &peer_id)?;
        let recv_key = derive_e2e_sframe_key_for_participant(call_key, &self_id)?;
        let mut encrypt_key = [0u8; AES_KEY_LEN];
        let mut decrypt_key = [0u8; AES_KEY_LEN];
        encrypt_key.copy_from_slice(&send_key[..AES_KEY_LEN]);
        decrypt_key.copy_from_slice(&recv_key[..AES_KEY_LEN]);
        Some(Self {
            encrypt_key,
            decrypt_key,
            tx_counter: 0,
            self_participant_id: self_id,
            peer_participant_id: peer_id,
        })
    }

    /// Encrypt one frame: `[ciphertext || tag || varint-header]`.
    pub fn encrypt(&mut self, plaintext: &[u8]) -> Vec<u8> {
        let counter = self.tx_counter;
        self.tx_counter += 1;
        let header = build_sframe_header(counter, 0);
        let iv = counter_to_iv(counter);
        let encrypted = gcm_encrypt(&self.encrypt_key, &iv, plaintext);
        let mut out = Vec::with_capacity(encrypted.len() + header.len());
        out.extend_from_slice(&encrypted);
        out.extend_from_slice(&header);
        out
    }

    /// Decrypt one frame. Returns [`SframeIn::Decrypted`] only when the trailing SFrame header parses
    /// and the GCM tag authenticates; otherwise [`SframeIn::Plaintext`], the frame is plain Opus to
    /// be used as-is. The caller branches on intent rather than guessing from the payload bytes.
    pub fn decrypt(&self, frame: &[u8]) -> SframeIn {
        // Too small to hold ciphertext + tag + a 3-byte header: it's plaintext Opus.
        if frame.len() < GCM_TAG_LEN + 3 {
            return SframeIn::Plaintext;
        }
        let header_len = *frame.last().unwrap() as usize;
        if header_len < 3 || header_len > frame.len() {
            return SframeIn::Plaintext;
        }
        let header_start = frame.len() - header_len;
        let header = &frame[header_start..];
        let ciphertext = &frame[..header_start];
        if ciphertext.len() < GCM_TAG_LEN + 1 {
            return SframeIn::Plaintext;
        }
        let Some((counter, _key_id)) = parse_sframe_header(header) else {
            return SframeIn::Plaintext;
        };
        let iv = counter_to_iv(counter);
        // GCM auth is the sole discriminator: a forged/plain frame fails the tag → pass-through.
        match gcm_decrypt(&self.decrypt_key, &iv, ciphertext) {
            Some(plain) => SframeIn::Decrypted(plain),
            None => SframeIn::Plaintext,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voip::testkat::{hexd, kats};

    #[test]
    fn participant_key_and_label_match_kat() {
        let k = kats();
        let call_key = hexd(&k, &["inputs", "callKey"]);
        let peer_id = format_sframe_participant_id(k["inputs"]["peerLid"].as_str().unwrap());
        assert_eq!(peer_id, k["sframe"]["participantPeerId"].as_str().unwrap());
        assert_eq!(
            sframe_info_label(&peer_id),
            k["sframe"]["infoLabelPeer"].as_str().unwrap()
        );
        let key = derive_e2e_sframe_key_for_participant(&call_key, &peer_id).unwrap();
        assert_eq!(
            hex::encode(&key),
            k["sframe"]["peerKey32"].as_str().unwrap()
        );
    }

    #[test]
    fn counter_iv_and_header_match_kat() {
        let k = kats();
        assert_eq!(
            hex::encode(counter_to_iv(5)),
            k["sframe"]["counterToIv_5"].as_str().unwrap()
        );
        assert_eq!(
            hex::encode(build_sframe_header(5, 0)),
            k["sframe"]["header_5_0"].as_str().unwrap()
        );
        // Header round-trips.
        assert_eq!(
            parse_sframe_header(&build_sframe_header(5, 0)),
            Some((5, 0))
        );
    }

    #[test]
    fn encrypt_matches_kat() {
        let k = kats();
        let call_key = hexd(&k, &["inputs", "callKey"]);
        // The KAT captures encrypt with encryptKey = peerKey[0:16] at a fixed counter of 5,
        // so seed tx_counter to 5 and encrypt once.
        let mut s = SframeSession::new(
            &call_key,
            k["inputs"]["selfLid"].as_str().unwrap(),
            k["inputs"]["peerLid"].as_str().unwrap(),
        )
        .unwrap();
        s.tx_counter = k["inputs"]["sframeCounter"].as_u64().unwrap();
        let payload = hexd(&k, &["inputs", "payload"]);
        let out = s.encrypt(&payload);
        assert_eq!(
            hex::encode(&out),
            k["sframe"]["encrypt_out"].as_str().unwrap()
        );
    }

    #[test]
    fn encrypt_decrypt_round_trip() {
        let k = kats();
        let call_key = hexd(&k, &["inputs", "callKey"]);
        // Sender encrypts for the peer; the peer (self/peer swapped) decrypts.
        let self_lid = k["inputs"]["selfLid"].as_str().unwrap();
        let peer_lid = k["inputs"]["peerLid"].as_str().unwrap();
        let mut sender = SframeSession::new(&call_key, self_lid, peer_lid).unwrap();
        let receiver = SframeSession::new(&call_key, peer_lid, self_lid).unwrap();
        let payload = b"hello sframe payload";
        let frame = sender.encrypt(payload);
        assert_eq!(
            receiver.decrypt(&frame),
            SframeIn::Decrypted(payload.to_vec())
        );
    }

    /// A real SFrame frame decrypted under the WRONG key must NOT yield the plaintext: the GCM tag
    /// authenticates the ciphertext, so a key/LID-direction mismatch fails closed instead of
    /// emitting forged "plaintext". (Refutes the "recv has no auth" hazard: the `aes_gcm` crate
    /// verifies the tag in `decrypt`.)
    #[test]
    fn wrong_key_does_not_forge_plaintext() {
        let k = kats();
        let call_key = hexd(&k, &["inputs", "callKey"]);
        let self_lid = k["inputs"]["selfLid"].as_str().unwrap();
        let peer_lid = k["inputs"]["peerLid"].as_str().unwrap();
        let mut sender = SframeSession::new(&call_key, self_lid, peer_lid).unwrap();
        // A non-Opus-looking payload so the looks_like_opus fallback can't mask the auth check.
        let payload = [0xaau8; 24];
        let frame = sender.encrypt(&payload);
        // Receiver with a DIFFERENT callKey → wrong decrypt key.
        let mut other = call_key.clone();
        other[0] ^= 0xff;
        let receiver = SframeSession::new(&other, peer_lid, self_lid).unwrap();
        // GCM auth rejects → fail-closed to Plaintext (never a forged Decrypted plaintext).
        assert_eq!(
            receiver.decrypt(&frame),
            SframeIn::Plaintext,
            "wrong key must not recover the plaintext (GCM auth must reject)"
        );
    }

    /// The peer ships plain Opus inside E2E-SRTP (it does NOT SFrame-wrap), so recv `decrypt` must
    /// report `Plaintext` so the caller uses the raw bytes UNCHANGED, never partially GCM-mangle
    /// them. Captured-call evidence: inbound frames repeat byte-for-byte (Opus DTX comfort noise),
    /// which fresh GCM ciphertext can never do. These are TOC families seen on the wire.
    #[test]
    fn plain_opus_passes_through_unchanged() {
        let k = kats();
        let call_key = hexd(&k, &["inputs", "callKey"]);
        let self_lid = k["inputs"]["selfLid"].as_str().unwrap();
        let peer_lid = k["inputs"]["peerLid"].as_str().unwrap();
        let receiver = SframeSession::new(&call_key, self_lid, peer_lid).unwrap();

        // Byte-exact frames lifted from a real inbound call dump (post-E2E-SRTP payloads). Each must
        // classify as Plaintext: SFrame decrypt either skips (too short / varint/header mismatch) or
        // the GCM auth fails; both fail-closed to Plaintext so the caller passes the raw bytes on.
        let plain_opus_frames: &[&[u8]] = &[
            &[0x00],                                                       // 1-byte DTX silence
            &hex::decode("90b81414c4").unwrap(), // 0x90 comfort noise (x85)
            &hex::decode("12101a759d3399bbaefb874fd75a004af7c0").unwrap(), // 0x12 code-2 DTX (x5)
            &hex::decode("9036ba6ffa40").unwrap(),
            &hex::decode("1236262b4ac920b1206166637b5af2").unwrap(), // a real 0x12 frame
        ];
        for f in plain_opus_frames {
            assert_eq!(
                receiver.decrypt(f),
                SframeIn::Plaintext,
                "plain Opus frame {} must classify as Plaintext (caller uses raw bytes)",
                hex::encode(f)
            );
        }
    }

    #[test]
    fn decode_varint_rejects_shift_overflow() {
        // 10 continuation bytes (high bit set) push the shift past 63 bits; the parser must return
        // None rather than panic or silently overflow the u64 accumulator.
        assert_eq!(decode_varint(&[0xFFu8; 10], 0), None);
    }

    #[test]
    fn recv_path_never_panics_on_truncation_or_garbage() {
        // The untrusted recv entry points (decrypt + the trailing-header parse) must never panic --
        // on any prefix of a real frame or on arbitrary garbage -- and fail closed (Plaintext/None).
        let k = kats();
        let call_key = hexd(&k, &["inputs", "callKey"]);
        let self_lid = k["inputs"]["selfLid"].as_str().unwrap();
        let peer_lid = k["inputs"]["peerLid"].as_str().unwrap();
        let mut sender = SframeSession::new(&call_key, self_lid, peer_lid).unwrap();
        let receiver = SframeSession::new(&call_key, peer_lid, self_lid).unwrap();
        let frame = sender.encrypt(b"truncation fuzz payload");
        for n in 0..=frame.len() {
            let _ = receiver.decrypt(&frame[..n]);
            let _ = parse_sframe_header(&frame[..n]);
        }
        let junk: &[&[u8]] = &[
            &[],
            &[0u8],
            &[0xFFu8; 1],
            &[0xFFu8; 16],
            &[0x02, 0xFF],
            &[0u8; 64],
            &[0xFFu8; 64],
        ];
        for j in junk {
            let _ = receiver.decrypt(j);
            let _ = parse_sframe_header(j);
        }
    }
}

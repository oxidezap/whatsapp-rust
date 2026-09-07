//! Runtime-agnostic cryptographic primitives shared across protocol features.

use hkdf::Hkdf;
use sha2::Sha256;

use crate::libsignal::protocol::{CurveError, KeyPair, PrivateKey};

const HKDF_SHA256_MAX_OUTPUT_LENGTH: usize = 255 * 32;

/// Errors returned by the shared cryptographic helpers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum CryptoError {
    /// The requested HKDF output exceeds the SHA-256 expansion limit.
    #[error("HKDF-SHA256 output length is invalid")]
    InvalidHkdfLength,
}

/// Legacy protocol fingerprint; this must not be used as a security hash.
pub fn md5_digest(input: &[u8]) -> [u8; 16] {
    let mut ctx = Md5::new();
    ctx.consume(input);
    ctx.finalize()
}

/// Suffix WA Web mixes into the contact hash (`WAWebApiContact`).
const CONTACT_NOTIFICATION_HASH_SUFFIX: &[u8] = b"WA_ADD_NOTIF";

/// The contact identifier carried by `<notification type="devices">
/// <update hash="..."/></notification>`, base64-encoded on the wire. `user` is
/// the contact's bare user part. Fed incrementally to avoid a concat buffer.
pub fn contact_notification_hash(user: &str) -> [u8; 3] {
    let mut context = Md5::new();
    context.consume(user.as_bytes());
    context.consume(CONTACT_NOTIFICATION_HASH_SUFFIX);
    let digest = context.finalize();
    [digest[0], digest[1], digest[2]]
}

/// Minimal RFC 1321 MD5 hasher for legacy protocol identifiers.
///
/// This exists solely to compute WhatsApp's legacy non-cryptographic fingerprints
/// (`contact_notification_hash`, registration `build_hash`) without pulling in an
/// external MD5 crate with 11 KB of unrolled loop tables.
#[derive(Clone, Debug)]
pub struct Md5 {
    state: [u32; 4],
    buffer: [u8; 64],
    buf_len: usize,
    total_len: u64,
}

const MD5_SHIFTS: [u32; 16] = [7, 12, 17, 22, 5, 9, 14, 20, 4, 11, 16, 23, 6, 10, 15, 21];

const MD5_K: [u32; 64] = [
    0xd76a_a478,
    0xe8c7_b756,
    0x2420_70db,
    0xc1bd_ceee,
    0xf57c_0faf,
    0x4787_c62a,
    0xa830_4613,
    0xfd46_9501,
    0x6980_98d8,
    0x8b44_f7af,
    0xffff_5bb1,
    0x895c_d7be,
    0x6b90_1122,
    0xfd98_7193,
    0xa679_438e,
    0x49b4_0821,
    0xf61e_2562,
    0xc040_b340,
    0x265e_5a51,
    0xe9b6_c7aa,
    0xd62f_105d,
    0x0244_1453,
    0xd8a1_e681,
    0xe7d3_fbc8,
    0x21e1_cde6,
    0xc337_07d6,
    0xf4d5_0d87,
    0x455a_14ed,
    0xa9e3_e905,
    0xfcef_a3f8,
    0x676f_02d9,
    0x8d2a_4c8a,
    0xfffa_3942,
    0x8771_f681,
    0x6d9d_6122,
    0xfde5_380c,
    0xa4be_ea44,
    0x4bde_cfa9,
    0xf6bb_4b60,
    0xbebf_bc70,
    0x289b_7ec6,
    0xeaa1_27fa,
    0xd4ef_3085,
    0x0488_1d05,
    0xd9d4_d039,
    0xe6db_99e5,
    0x1fa2_7cf8,
    0xc4ac_5665,
    0xf429_2244,
    0x432a_ff97,
    0xab94_23a7,
    0xfc93_a039,
    0x655b_59c3,
    0x8f0c_cc92,
    0xffef_f47d,
    0x8584_5dd1,
    0x6fa8_7e4f,
    0xfe2c_e6e0,
    0xa301_4314,
    0x4e08_11a1,
    0xf753_7e82,
    0xbd3a_f235,
    0x2ad7_d2bb,
    0xeb86_d391,
];

impl Default for Md5 {
    fn default() -> Self {
        Self::new()
    }
}

impl Md5 {
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: [0x6745_2301, 0xefcd_ab89, 0x98ba_dcfe, 0x1032_5476],
            buffer: [0u8; 64],
            buf_len: 0,
            total_len: 0,
        }
    }

    pub fn consume(&mut self, mut data: &[u8]) {
        self.total_len += data.len() as u64;
        if self.buf_len > 0 {
            let to_fill = 64 - self.buf_len;
            if data.len() >= to_fill {
                self.buffer[self.buf_len..64].copy_from_slice(&data[..to_fill]);
                Self::process_block(&mut self.state, &self.buffer);
                self.buf_len = 0;
                data = &data[to_fill..];
            } else {
                self.buffer[self.buf_len..self.buf_len + data.len()].copy_from_slice(data);
                self.buf_len += data.len();
                return;
            }
        }
        while data.len() >= 64 {
            Self::process_block(&mut self.state, &data[..64]);
            data = &data[64..];
        }
        if !data.is_empty() {
            self.buffer[..data.len()].copy_from_slice(data);
            self.buf_len = data.len();
        }
    }

    #[must_use]
    pub fn finalize(mut self) -> [u8; 16] {
        let bit_len = self.total_len * 8;
        self.buffer[self.buf_len] = 0x80;
        self.buf_len += 1;
        if self.buf_len > 56 {
            self.buffer[self.buf_len..64].fill(0);
            Self::process_block(&mut self.state, &self.buffer);
            self.buf_len = 0;
        }
        self.buffer[self.buf_len..56].fill(0);
        self.buffer[56..64].copy_from_slice(&bit_len.to_le_bytes());
        Self::process_block(&mut self.state, &self.buffer);

        let mut out = [0u8; 16];
        for (chunk, val) in out.chunks_exact_mut(4).zip(self.state) {
            chunk.copy_from_slice(&val.to_le_bytes());
        }
        out
    }

    fn process_block(state: &mut [u32; 4], block: &[u8]) {
        let mut m = [0u32; 16];
        for (slot, chunk) in m.iter_mut().zip(block.chunks_exact(4)) {
            if let Ok(b) = <[u8; 4]>::try_from(chunk) {
                *slot = u32::from_le_bytes(b);
            }
        }
        let [mut a, mut b, mut c, mut d] = *state;

        for i in 0..64 {
            let (f, g) = match i {
                0..=15 => ((b & c) | (!b & d), i),
                16..=31 => ((d & b) | (!d & c), (5 * i + 1) % 16),
                32..=47 => (b ^ c ^ d, (3 * i + 5) % 16),
                _ => (c ^ (b | !d), (7 * i) % 16),
            };
            let shift = MD5_SHIFTS[(i / 16) * 4 + (i % 4)];
            let temp = d;
            d = c;
            c = b;
            b = b.wrapping_add(
                a.wrapping_add(f)
                    .wrapping_add(MD5_K[i])
                    .wrapping_add(m[g])
                    .rotate_left(shift),
            );
            a = temp;
        }

        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
    }
}

/// Decode the wire form of [`contact_notification_hash`]. Rejects anything that
/// is not exactly one base64 group, so a malformed attribute cannot alias a
/// real contact.
pub fn parse_contact_notification_hash(wire: &str) -> Option<[u8; 3]> {
    use base64::Engine as _;

    let mut decoded = [0u8; 3];
    let written = base64::engine::general_purpose::STANDARD_NO_PAD
        .decode_slice(wire.as_bytes(), &mut decoded)
        .ok()?;
    (written == decoded.len()).then_some(decoded)
}

/// Rejects output beyond HKDF's 255-block expansion limit before allocating.
pub fn hkdf_sha256(
    input_key_material: &[u8],
    expanded_length: usize,
    salt: Option<&[u8]>,
    info: &[u8],
) -> Result<Vec<u8>, CryptoError> {
    if expanded_length > HKDF_SHA256_MAX_OUTPUT_LENGTH {
        return Err(CryptoError::InvalidHkdfLength);
    }
    let mut output = vec![0; expanded_length];
    hkdf_sha256_into(input_key_material, salt, info, &mut output)?;
    Ok(output)
}

/// Caller-owned output avoids an allocation in fixed-size derivation paths.
pub fn hkdf_sha256_into(
    input_key_material: &[u8],
    salt: Option<&[u8]>,
    info: &[u8],
    output: &mut [u8],
) -> Result<(), CryptoError> {
    Hkdf::<Sha256>::new(salt, input_key_material)
        .expand(info, output)
        .map_err(|_| CryptoError::InvalidHkdfLength)
}

/// Uses the crate-wide secure random source so key generation follows one policy.
pub fn generate_curve_key_pair() -> KeyPair {
    KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>())
}

/// Uses the same secure random policy as key generation for randomized signatures.
pub fn calculate_curve_signature(
    private_key: &PrivateKey,
    message: &[u8],
) -> Result<[u8; 64], CurveError> {
    private_key.calculate_signature(message, &mut rand::make_rng::<rand::rngs::StdRng>())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Confirmed against a live stream, then re-derived from fictitious LIDs.
    #[test]
    fn contact_notification_hash_matches_wire_vectors() {
        use base64::Engine as _;
        for (user, wire) in [
            ("100000000000001", "s7oK"),
            ("100000000000002", "P2DY"),
            ("100000000000003", "4ZhY"),
        ] {
            let encoded =
                base64::engine::general_purpose::STANDARD.encode(contact_notification_hash(user));
            assert_eq!(encoded, wire, "contact hash for {user}");
            assert_eq!(
                parse_contact_notification_hash(wire),
                Some(contact_notification_hash(user)),
                "wire hash for {user} must round-trip"
            );
        }
    }

    #[test]
    fn malformed_contact_hashes_are_rejected() {
        for wire in ["", "s7o", "s7oKX", "s7oK==", "!!!!", "s7oKs7oK"] {
            assert_eq!(
                parse_contact_notification_hash(wire),
                None,
                "{wire:?} is not a single base64 group"
            );
        }
    }

    #[test]
    fn md5_rfc1321_vectors() {
        let cases: &[(&[u8], &str)] = &[
            (b"", "d41d8cd98f00b204e9800998ecf8427e"),
            (b"a", "0cc175b9c0f1b6a831c399e269772661"),
            (b"abc", "900150983cd24fb0d6963f7d28e17f72"),
            (b"message digest", "f96b697d7cb7938d525a2f31aaf161d0"),
            (
                b"abcdefghijklmnopqrstuvwxyz",
                "c3fcd3d76192e4007dfb496cca67e13b",
            ),
            (
                b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789",
                "d174ab98d277d9f5a5611c2c9f419d9f",
            ),
            (
                b"12345678901234567890123456789012345678901234567890123456789012345678901234567890",
                "57edf4a22be3c955ac49da2e2107b67a",
            ),
        ];
        for (input, expected) in cases {
            assert_eq!(hex::encode(md5_digest(input)), *expected);
        }
    }

    #[test]
    fn hashes_and_expands_known_vectors() {
        assert_eq!(
            hex::encode(md5_digest(b"abc")),
            "900150983cd24fb0d6963f7d28e17f72"
        );

        let ikm = [0x0bu8; 22];
        let salt = hex::decode("000102030405060708090a0b0c").unwrap();
        let info = hex::decode("f0f1f2f3f4f5f6f7f8f9").unwrap();
        let output = hkdf_sha256(&ikm, 42, Some(&salt), &info).unwrap();
        assert_eq!(
            hex::encode(output),
            "3cb25f25faacd57a90434f64d0362f2a\
             2d2d0a90cf1a5a4c5db02d56ecc4c5bf\
             34007208d5b887185865"
                .replace(char::is_whitespace, "")
        );
    }

    #[test]
    fn writes_into_a_caller_owned_buffer() {
        let mut output = [0u8; 32];
        hkdf_sha256_into(b"input", None, b"info", &mut output).unwrap();
        assert_eq!(
            output.as_slice(),
            hkdf_sha256(b"input", 32, None, b"info").unwrap()
        );
    }

    #[test]
    fn rejects_output_beyond_sha256_limit() {
        assert_eq!(
            hkdf_sha256(b"input", HKDF_SHA256_MAX_OUTPUT_LENGTH + 1, None, b"info"),
            Err(CryptoError::InvalidHkdfLength)
        );

        let mut output = vec![0; HKDF_SHA256_MAX_OUTPUT_LENGTH + 1];
        assert_eq!(
            hkdf_sha256_into(b"input", None, b"info", &mut output),
            Err(CryptoError::InvalidHkdfLength)
        );
    }

    #[test]
    fn generated_keys_sign_with_the_shared_signal_implementation() {
        let pair = generate_curve_key_pair();
        let signature = calculate_curve_signature(&pair.private_key, b"message").unwrap();
        assert!(pair.public_key.verify_signature(b"message", &signature));
        assert!(!pair.public_key.verify_signature(b"tampered", &signature));
    }
}

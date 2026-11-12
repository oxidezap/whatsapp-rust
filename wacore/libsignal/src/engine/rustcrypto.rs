// SPDX-License-Identifier: MIT
//! RustCrypto implementation using RFC 7748, RFC 5869 and the public XEdDSA
//! specification, <https://signal.org/docs/specifications/xeddsa/> sections 2–5.
//! Verification also accepts an Edwards sign bit transported in signature bit 511.
use crate::curve::{edwards::EdwardsPoint, montgomery::MontgomeryPoint, scalar::Scalar};
use crate::engine::{CryptoProvider, Error, RandomSource, Result};
use aes::cipher::{BlockModeDecrypt, BlockModeEncrypt, KeyIvInit, block_padding::Pkcs7};
use hmac::{Hmac, HmacReset, KeyInit, Mac};
use sha2::digest::{block_api::CoreProxy, common::hazmat::SerializableState};
use sha2::{Digest, Sha256, Sha512};
use subtle::{Choice, ConditionallySelectable, ConstantTimeEq};
use zeroize::Zeroizing;

#[derive(Clone, Copy, Debug, Default)]
pub struct RustCrypto;

// SHA-256 states after RFC 2104's public zero-key ipad/opad blocks. These
// constants avoid global initialization; tests regenerate the serialized states
// through the hash API to detect any dependency state-format change.
const ZERO_SALT_IPAD: [u8; 40] = [
    0xad, 0xde, 0x54, 0xf4, 0x4f, 0x21, 0x25, 0x97, 0xa0, 0xf2, 0xda, 0x90, 0xea, 0x28, 0x12, 0xdf,
    0x0f, 0x75, 0xe5, 0x64, 0x81, 0x41, 0x92, 0xa3, 0x2b, 0x93, 0x4a, 0x82, 0x32, 0x4e, 0xe0, 0xf8,
    1, 0, 0, 0, 0, 0, 0, 0,
];
const ZERO_SALT_OPAD: [u8; 40] = [
    0x0f, 0x48, 0x85, 0xd3, 0x77, 0x64, 0xbb, 0x7a, 0x38, 0xc5, 0xc9, 0x37, 0x67, 0x24, 0xd8, 0x5d,
    0x72, 0x3a, 0x04, 0x8e, 0xb0, 0x34, 0x34, 0x75, 0x18, 0x28, 0xb8, 0xde, 0xa6, 0x45, 0x1d, 0x36,
    1, 0, 0, 0, 0, 0, 0, 0,
];

fn public_sha256_prefix(state: &[u8; 40]) -> Sha256 {
    let core = <Sha256 as CoreProxy>::Core::deserialize(&(*state).into())
        .expect("fixed public SHA-256 prefix state");
    Sha256::compose(core, Default::default())
}

impl RustCrypto {
    pub(crate) fn hmac_sha256_array(&self, key: &[u8], data: &[u8]) -> [u8; 32] {
        self.hmac_sha256_parts_array(key, &[data])
    }
    pub(crate) fn hmac_sha256_parts_array(&self, key: &[u8], parts: &[&[u8]]) -> [u8; 32] {
        let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(key)
            .expect("HMAC accepts keys of any length");
        for part in parts {
            mac.update(part);
        }
        mac.finalize().into_bytes().into()
    }
}

fn scalar_hash(hash: Sha512) -> Scalar {
    let digest = Zeroizing::new(<[u8; 64]>::from(hash.finalize()));
    Scalar::from_bytes_mod_order_wide(&digest)
}
fn challenge(r: &[u8], public: &[u8], message: &[u8]) -> Scalar {
    let mut hash = Sha512::new();
    hash.update(r);
    hash.update(public);
    hash.update(message);
    scalar_hash(hash)
}
fn canonical_montgomery(public: &[u8; 32]) -> bool {
    // p = 2^255 - 19, encoded little endian. Comparison concerns public data.
    let mut p = [255; 32];
    p[0] = 237;
    p[31] = 127;
    public.iter().rev().cmp(p.iter().rev()).is_lt()
}
impl CryptoProvider for RustCrypto {
    fn public_key(&self, private: &[u8; 32]) -> Result<[u8; 32]> {
        Ok(MontgomeryPoint::mul_base_clamped(*private).to_bytes())
    }
    fn agreement(&self, private: &[u8; 32], public: &[u8; 32]) -> Result<[u8; 32]> {
        let shared = MontgomeryPoint(*public).mul_clamped(*private).to_bytes();
        if bool::from(shared.ct_eq(&[0; 32])) {
            return Err(Error::Crypto);
        }
        Ok(shared)
    }
    fn sign(
        &self,
        private: &[u8; 32],
        message: &[u8],
        random: &mut dyn RandomSource,
    ) -> Result<[u8; 64]> {
        let mut randomness = Zeroizing::new([0; 64]);
        random.fill(&mut *randomness)?;
        let mut clamped = Zeroizing::new(*private);
        clamped[0] &= 248;
        clamped[31] = (clamped[31] & 127) | 64;
        let scalar = Zeroizing::new(Scalar::from_bytes_mod_order(*clamped));
        let mut public = EdwardsPoint::mul_base(&scalar).compress().to_bytes();
        let sign = Choice::from(public[31] >> 7);
        let a = Zeroizing::new(Scalar::conditional_select(&scalar, &-*scalar, sign));
        public[31] &= 127;
        let mut domain = [255; 32];
        domain[0] = 254;
        let mut hash = Sha512::new();
        hash.update(domain);
        hash.update(a.as_bytes());
        hash.update(message);
        hash.update(randomness.as_slice());
        let nonce = Zeroizing::new(scalar_hash(hash));
        let r = EdwardsPoint::mul_base(&nonce).compress().to_bytes();
        let h = challenge(&r, &public, message);
        let s = *nonce + h * *a;
        let mut signature = [0; 64];
        signature[..32].copy_from_slice(&r);
        signature[32..].copy_from_slice(s.as_bytes());
        Ok(signature)
    }
    fn verify(&self, public: &[u8; 32], message: &[u8], signature: &[u8; 64]) -> Result<()> {
        if !canonical_montgomery(public) {
            return Err(Error::Authentication);
        }
        let point = MontgomeryPoint(*public)
            .to_edwards(signature[63] >> 7)
            .ok_or(Error::Authentication)?;
        if point.is_small_order() {
            return Err(Error::Authentication);
        }
        let mut encoded_s = [0; 32];
        encoded_s.copy_from_slice(&signature[32..]);
        encoded_s[31] &= 127;
        // XEdDSA permits an unreduced s below 2^253, see specification section 5.
        if encoded_s[31] & 224 != 0 {
            return Err(Error::Authentication);
        }
        let s = Scalar::from_bytes_mod_order(encoded_s);
        let h = challenge(&signature[..32], point.compress().as_bytes(), message);
        let check = EdwardsPoint::vartime_double_scalar_mul_basepoint(&-h, &point, &s).compress();
        if bool::from(check.as_bytes().ct_eq(&signature[..32])) {
            Ok(())
        } else {
            Err(Error::Authentication)
        }
    }
    fn sha256(&self, data: &[u8]) -> [u8; 32] {
        Sha256::digest(data).into()
    }
    fn hmac_sha256(&self, key: &[u8], parts: &[&[u8]]) -> Result<[u8; 32]> {
        Ok(RustCrypto::hmac_sha256_parts_array(self, key, parts))
    }

    fn chain_step(&self, key: &[u8; 32]) -> Result<([u8; 32], [u8; 32])> {
        let mut mac = <HmacReset<Sha256> as KeyInit>::new_from_slice(key)
            .expect("HMAC accepts keys of any length");
        mac.update(&[1]);
        let seed = mac.finalize_reset().into_bytes().into();
        mac.update(&[2]);
        Ok((seed, mac.finalize().into_bytes().into()))
    }

    fn hkdf_sha256(&self, salt: &[u8], input: &[u8], info: &[u8], output: &mut [u8]) -> Result<()> {
        hkdf::Hkdf::<Sha256>::new(Some(salt), input)
            .expand(info, output)
            .map_err(|_| Error::Crypto)
    }
    fn hkdf_sha256_zero_salt(&self, input: &[u8], info: &[u8], output: &mut [u8]) -> Result<()> {
        let mut inner = public_sha256_prefix(&ZERO_SALT_IPAD);
        inner.update(input);
        let inner_digest = Zeroizing::new(<[u8; 32]>::from(inner.finalize()));
        let mut outer = public_sha256_prefix(&ZERO_SALT_OPAD);
        outer.update(inner_digest.as_slice());
        let prk = Zeroizing::new(<[u8; 32]>::from(outer.finalize()));
        hkdf::Hkdf::<Sha256>::from_prk(prk.as_ref())
            .map_err(|_| Error::Crypto)?
            .expand(info, output)
            .map_err(|_| Error::Crypto)
    }
    fn encrypt_cbc(&self, key: &[u8; 32], iv: &[u8; 16], plaintext: &[u8]) -> Result<Vec<u8>> {
        Ok(cbc::Encryptor::<aes::Aes256>::new(key.into(), iv.into())
            .encrypt_padded_vec::<Pkcs7>(plaintext))
    }
    fn encrypt_cbc_into(
        &self,
        key: &[u8; 32],
        iv: &[u8; 16],
        plaintext: &[u8],
        output: &mut Vec<u8>,
    ) -> Result<()> {
        output.reserve(super::crypto::cbc_ciphertext_len(plaintext.len())?);
        crate::crypto::aes_256_cbc_encrypt_into(plaintext, key, iv, output)
            .map_err(|_| Error::Crypto)
    }
    fn decrypt_cbc(&self, key: &[u8; 32], iv: &[u8; 16], ciphertext: &[u8]) -> Result<Vec<u8>> {
        cbc::Decryptor::<aes::Aes256>::new(key.into(), iv.into())
            .decrypt_padded_vec::<Pkcs7>(ciphertext)
            .map_err(|_| Error::InvalidCiphertext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cbc_append_preserves_prefix_allocation_and_standard_ciphertext() {
        for len in [0, 1, 15, 16, 17, 31, 32, 33, 127, 128, 129, 1024] {
            let plaintext = vec![len as u8; len];
            let expected = RustCrypto
                .encrypt_cbc(&[3; 32], &[4; 16], &plaintext)
                .unwrap();
            for prefix_len in [0, 1, 15, 16, 47, 64, 129] {
                let prefix = vec![9; prefix_len];
                let mut output = Vec::with_capacity(prefix_len + expected.len() + 8);
                output.extend_from_slice(&prefix);
                let pointer = output.as_ptr();
                RustCrypto
                    .encrypt_cbc_into(&[3; 32], &[4; 16], &plaintext, &mut output)
                    .unwrap();
                assert_eq!(&output[..prefix_len], prefix);
                assert_eq!(&output[prefix_len..], expected);
                assert_eq!(output.as_ptr(), pointer);
            }
        }
        assert!(super::super::crypto::cbc_ciphertext_len(usize::MAX).is_err());
    }
    #[test]
    fn public_zero_salt_prefixes_match_the_hash_state_format() {
        for (pad, expected) in [(0x36, ZERO_SALT_IPAD), (0x5c, ZERO_SALT_OPAD)] {
            let mut hash = Sha256::new();
            hash.update([pad; 64]);
            let (core, buffer) = hash.decompose();
            assert_eq!(buffer.get_pos(), 0);
            assert_eq!(core.serialize().as_slice(), expected);
        }
    }

    #[test]
    fn zero_salt_hkdf_matches_rfc5869_and_generic_boundaries() {
        let mut rfc = [0; 42];
        RustCrypto
            .hkdf_sha256_zero_salt(&[11; 22], b"", &mut rfc)
            .unwrap();
        assert_eq!(
            rfc,
            hex(
                "8da4e775a563c18f715f802a063c5a31b8a11f5c5ee1879ec3454e5f3c738d2d9d201395faa4b61a96c8"
            )
        );
        for input_len in [0, 1, 31, 32, 33, 63, 64, 65, 127, 128, 1024] {
            let input: Vec<_> = (0..input_len).map(|index| index as u8).collect();
            for info in [b"".as_slice(), b"WhisperMessageKeys", b"WhisperGroup"] {
                for output_len in [0, 1, 31, 32, 33, 48, 80, 96] {
                    let mut expected = vec![0xa5; output_len];
                    let mut actual = expected.clone();
                    RustCrypto
                        .hkdf_sha256(&[0; 32], &input, info, &mut expected)
                        .unwrap();
                    RustCrypto
                        .hkdf_sha256_zero_salt(&input, info, &mut actual)
                        .unwrap();
                    assert_eq!(actual, expected);
                }
            }
        }
        for output_len in [255 * 32, 255 * 32 + 1] {
            let mut expected = vec![0xa5; output_len];
            let mut actual = expected.clone();
            assert_eq!(
                RustCrypto.hkdf_sha256(&[0; 32], b"synthetic", b"", &mut expected),
                RustCrypto.hkdf_sha256_zero_salt(b"synthetic", b"", &mut actual)
            );
            assert_eq!(actual, expected);
        }
    }

    #[test]
    fn paired_chain_hmac_matches_independent_messages() {
        for seed in 0u8..32 {
            let key = std::array::from_fn(|index| seed.wrapping_mul(17) ^ index as u8);
            assert_eq!(
                RustCrypto.chain_step(&key).unwrap(),
                (
                    RustCrypto.hmac_sha256_array(&key, &[1]),
                    RustCrypto.hmac_sha256_array(&key, &[2]),
                )
            );
        }
    }

    #[test]
    fn rfc7748_low_order_aliases_reject_and_valid_high_bit_alias_agrees() {
        // RFC 7748 sections 5 and 6.1 define masking, field reduction and the
        // all-zero shared-secret check. Torsion points are public Dalek constants.
        let private = hex("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a");
        let mut p = [255; 32];
        p[0] = 237;
        p[31] = 127;
        let mut p_plus_one = p;
        p_plus_one[0] += 1;
        let mut minus_one = p;
        minus_one[0] -= 1;
        let mut one = [0; 32];
        one[0] = 1;
        let mut low_order = vec![[0; 32], one, minus_one, p, p_plus_one];
        low_order.extend(
            crate::curve::EIGHT_TORSION
                .iter()
                .map(|point| point.to_montgomery().to_bytes()),
        );
        for public in low_order {
            let mut alias = public;
            alias[31] |= 128;
            for encoded in [public, alias] {
                assert!(
                    RustCrypto.agreement(&private, &encoded).is_err(),
                    "{encoded:02x?}"
                );
            }
        }
        let public = hex("de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f");
        let mut alias = public;
        alias[31] |= 128;
        let expected = hex("4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742");
        assert_eq!(RustCrypto.agreement(&private, &public).unwrap(), expected);
        assert_eq!(RustCrypto.agreement(&private, &alias).unwrap(), expected);
    }

    fn hex<const N: usize>(s: &str) -> [u8; N] {
        assert_eq!(s.len(), N * 2);
        core::array::from_fn(|i| u8::from_str_radix(&s[i * 2..i * 2 + 2], 16).unwrap())
    }
    struct Fixed;
    impl RandomSource for Fixed {
        fn fill(&mut self, out: &mut [u8]) -> Result<()> {
            out.fill(42);
            Ok(())
        }
    }
    #[test]
    fn rfc7748_alice_bob() {
        let alice = hex("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a");
        let bob = hex("5dab087e624a8a4b79e17f8b83800ee66f3bb1292618b6fd1c2f8b27ff88e0eb");
        let a = RustCrypto.public_key(&alice).unwrap();
        let b = RustCrypto.public_key(&bob).unwrap();
        assert_eq!(
            a,
            hex("8520f0098930a754748b7ddcb43ef75a0dbf3a0d26381af4eba4a98eaa9b4e6a")
        );
        assert_eq!(
            b,
            hex("de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f")
        );
        let shared = hex("4a5d9d5ba4ce2de1728e3bf480350f25e07e21c947d19e3376f09b3c1e161742");
        assert_eq!(RustCrypto.agreement(&alice, &b).unwrap(), shared);
        assert_eq!(RustCrypto.agreement(&bob, &a).unwrap(), shared);
        assert!(RustCrypto.agreement(&alice, &[0; 32]).is_err());
        let mut one = [0; 32];
        one[0] = 1;
        assert!(RustCrypto.agreement(&alice, &one).is_err());
    }
    #[test]
    fn rfc5869_case_one() {
        let mut out = [0; 42];
        RustCrypto
            .hkdf_sha256(
                &hex::<13>("000102030405060708090a0b0c"),
                &[11; 22],
                &hex::<10>("f0f1f2f3f4f5f6f7f8f9"),
                &mut out,
            )
            .unwrap();
        assert_eq!(
            out,
            hex(
                "3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865"
            )
        );
    }
    #[test]
    fn rfc4231_hmac_and_sha256() {
        assert_eq!(
            RustCrypto.hmac_sha256_array(&[11; 20], b"Hi There"),
            hex("b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7")
        );
        assert_eq!(
            RustCrypto.sha256(b"abc"),
            hex("ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad")
        );
    }
    #[test]
    fn rfc8032_signature_with_montgomery_public_key() {
        // RFC 8032 section 7.1, test 1. XEdDSA uses the same verification equation.
        let ed = hex("d75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a");
        let public = crate::curve::edwards::CompressedEdwardsY(ed)
            .decompress()
            .unwrap()
            .to_montgomery()
            .to_bytes();
        let signature = hex(
            "e5564300c360ac729086e2cc806e828a84877f1eb8e5d974d873e065224901555fb8821590a33bacc61e39701cf9b46bd25bf5f0595bbe24655141438e7a100b",
        );
        RustCrypto.verify(&public, b"", &signature).unwrap();
    }
    #[test]
    fn nist_sp800_38a_aes256_cbc_first_block() {
        let key = hex("603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4");
        let iv = hex("000102030405060708090a0b0c0d0e0f");
        let input = hex::<16>("6bc1bee22e409f96e93d7e117393172a");
        let encrypted = RustCrypto.encrypt_cbc(&key, &iv, &input).unwrap();
        assert_eq!(
            &encrypted[..16],
            &hex::<16>("f58c4c04d6e5f1ba779eabfb5f7bfbd6")
        );
    }
    #[test]
    fn signatures_and_rejections() {
        for byte in 0..32 {
            let private = [byte; 32];
            let public = RustCrypto.public_key(&private).unwrap();
            let signature = RustCrypto
                .sign(&private, b"synthetic message", &mut Fixed)
                .unwrap();
            RustCrypto
                .verify(&public, b"synthetic message", &signature)
                .unwrap();
            assert!(
                RustCrypto
                    .verify(&public, b"changed message", &signature)
                    .is_err()
            );
            let mut corrupted = signature;
            corrupted[0] ^= 1;
            assert!(
                RustCrypto
                    .verify(&public, b"synthetic message", &corrupted)
                    .is_err()
            );
        }
        assert!(RustCrypto.verify(&[0; 32], b"", &[0; 64]).is_err());
    }
    #[test]
    fn cbc_padding_boundaries() {
        for len in [0, 1, 15, 16, 17, 31, 32, 255] {
            let input = vec![7; len];
            let encrypted = RustCrypto.encrypt_cbc(&[3; 32], &[4; 16], &input).unwrap();
            assert_eq!(encrypted.len(), (len / 16 + 1) * 16);
            assert_eq!(
                RustCrypto
                    .decrypt_cbc(&[3; 32], &[4; 16], &encrypted)
                    .unwrap(),
                input
            );
        }
        assert!(RustCrypto.decrypt_cbc(&[3; 32], &[4; 16], &[]).is_err());
        assert!(
            RustCrypto
                .decrypt_cbc(&[3; 32], &[4; 16], &[0; 15])
                .is_err()
        );
    }
}

// SPDX-License-Identifier: MIT
//! Curve25519 key encodings and cached signing follow RFC 7748 and the public
//! XEdDSA specification, <https://signal.org/docs/specifications/xeddsa/>.
use crate::engine::{CryptoProvider, RandomSource, rustcrypto::RustCrypto};
use rand::{CryptoRng, Rng};
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use zeroize::Zeroize;

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum CurveError {
    #[error("missing public key type identifier")]
    NoKeyTypeIdentifier,
    #[error("invalid Curve25519 key encoding")]
    InvalidKey,
    #[error("Curve25519 cryptographic operation failed")]
    Crypto,
}
type Result<T> = std::result::Result<T, CurveError>;
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PublicKey([u8; 32]);
impl PublicKey {
    pub const RAW_KEY_LEN: usize = 32;
    pub const SERIALIZED_KEY_LEN: usize = 33;
    pub fn from_djb_public_key_bytes(bytes: &[u8]) -> Result<Self> {
        Ok(Self(bytes.try_into().map_err(|_| CurveError::InvalidKey)?))
    }
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        if bytes.is_empty() {
            return Err(CurveError::NoKeyTypeIdentifier);
        }
        if bytes.len() != 33 || bytes[0] != 5 {
            return Err(CurveError::InvalidKey);
        }
        Self::from_djb_public_key_bytes(&bytes[1..])
    }
    pub fn public_key_bytes(&self) -> &[u8; 32] {
        &self.0
    }
    pub fn serialize(&self) -> [u8; 33] {
        let mut out = [0; 33];
        out[0] = 5;
        out[1..].copy_from_slice(&self.0);
        out
    }
    pub fn verify_signature(&self, message: &[u8], signature: &[u8]) -> bool {
        signature
            .try_into()
            .is_ok_and(|s| RustCrypto.verify(&self.0, message, s).is_ok())
    }
    pub fn prepare_verifying_key(&self) -> Result<PreparedVerifyingKey> {
        PreparedVerifyingKey::new(self)
    }
}
/// Raw scalar with a lazy inline signing context. Equality and serialization
/// depend only on the scalar, including after signing or cloning a warm key.
#[derive(Clone)]
pub struct PrivateKey {
    key: [u8; 32],
    signing: OnceLock<PreparedSigningKey>,
}
impl PartialEq for PrivateKey {
    fn eq(&self, other: &Self) -> bool {
        self.key == other.key
    }
}
impl Eq for PrivateKey {}
impl std::fmt::Debug for PrivateKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PrivateKey([redacted])")
    }
}
impl Drop for PrivateKey {
    fn drop(&mut self) {
        self.key.zeroize();
    }
}
// Persisted key serde uses tagged public keys and raw private scalars. Arrays
// would change both the human-readable representation and binary token kind.
fn serialize_key<S: serde::Serializer>(
    bytes: &[u8],
    serializer: S,
) -> std::result::Result<S::Ok, S::Error> {
    if serializer.is_human_readable() {
        let text = zeroize::Zeroizing::new(hex::encode(bytes));
        serializer.serialize_str(text.as_str())
    } else {
        serializer.serialize_bytes(bytes)
    }
}
fn deserialize_key<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
    expected: usize,
) -> std::result::Result<zeroize::Zeroizing<Vec<u8>>, D::Error> {
    use serde::de::Error as _;
    if deserializer.is_human_readable() {
        let text = zeroize::Zeroizing::new(String::deserialize(deserializer)?);
        if text.len() != expected * 2 {
            return Err(D::Error::custom("invalid key length"));
        }
        return hex::decode(text.as_str())
            .map(zeroize::Zeroizing::new)
            .map_err(D::Error::custom);
    }
    struct KeyBytes(usize);
    impl<'de> serde::de::Visitor<'de> for KeyBytes {
        type Value = zeroize::Zeroizing<Vec<u8>>;
        fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "a {}-byte key", self.0)
        }
        fn visit_bytes<E: serde::de::Error>(
            self,
            bytes: &[u8],
        ) -> std::result::Result<Self::Value, E> {
            if bytes.len() != self.0 {
                return Err(E::custom("invalid key length"));
            }
            Ok(zeroize::Zeroizing::new(bytes.to_vec()))
        }
        fn visit_byte_buf<E: serde::de::Error>(
            self,
            bytes: Vec<u8>,
        ) -> std::result::Result<Self::Value, E> {
            let bytes = zeroize::Zeroizing::new(bytes);
            if bytes.len() != self.0 {
                return Err(E::custom("invalid key length"));
            }
            Ok(bytes)
        }
        fn visit_seq<A: serde::de::SeqAccess<'de>>(
            self,
            mut sequence: A,
        ) -> std::result::Result<Self::Value, A::Error> {
            let mut bytes = zeroize::Zeroizing::new(Vec::with_capacity(self.0));
            while let Some(byte) = sequence.next_element::<u8>()? {
                if bytes.len() == self.0 {
                    return Err(A::Error::custom("invalid key length"));
                }
                bytes.push(byte);
            }
            if bytes.len() != self.0 {
                return Err(A::Error::custom("invalid key length"));
            }
            Ok(bytes)
        }
    }
    deserializer.deserialize_bytes(KeyBytes(expected))
}
impl Serialize for PublicKey {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serialize_key(&self.serialize(), serializer)
    }
}
impl<'de> Deserialize<'de> for PublicKey {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        let bytes = deserialize_key(deserializer, Self::SERIALIZED_KEY_LEN)?;
        Self::deserialize(&bytes).map_err(serde::de::Error::custom)
    }
}
impl Serialize for PrivateKey {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serialize_key(self.serialize(), serializer)
    }
}
impl<'de> Deserialize<'de> for PrivateKey {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        let bytes = deserialize_key(deserializer, 32)?;
        Self::deserialize(&bytes).map_err(serde::de::Error::custom)
    }
}
struct RandomAdapter<'a, R>(&'a mut R);
impl<R: Rng + CryptoRng> RandomSource for RandomAdapter<'_, R> {
    fn fill(&mut self, out: &mut [u8]) -> crate::engine::Result<()> {
        self.0.fill_bytes(out);
        Ok(())
    }
}
impl PrivateKey {
    fn from_raw(key: [u8; 32]) -> Self {
        Self {
            key,
            signing: OnceLock::new(),
        }
    }
    pub fn deserialize(bytes: &[u8]) -> Result<Self> {
        Ok(Self::from_raw(
            bytes.try_into().map_err(|_| CurveError::InvalidKey)?,
        ))
    }
    pub fn serialize(&self) -> &[u8; 32] {
        &self.key
    }
    pub fn public_key(&self) -> Result<PublicKey> {
        RustCrypto
            .public_key(&self.key)
            .map(PublicKey)
            .map_err(|_| CurveError::Crypto)
    }
    pub fn calculate_agreement(&self, public: &PublicKey) -> Result<[u8; 32]> {
        RustCrypto
            .agreement(&self.key, &public.0)
            .map_err(|_| CurveError::Crypto)
    }
    pub fn calculate_signature<R: Rng + CryptoRng>(
        &self,
        message: &[u8],
        rng: &mut R,
    ) -> Result<[u8; 64]> {
        self.signing
            .get_or_init(|| PreparedSigningKey::prepare(&self.key))
            .calculate_signature(message, rng)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyPair {
    pub public_key: PublicKey,
    pub private_key: PrivateKey,
}
impl KeyPair {
    pub fn new(public_key: PublicKey, private_key: PrivateKey) -> Self {
        Self {
            public_key,
            private_key,
        }
    }
    pub fn generate<R: Rng + CryptoRng>(rng: &mut R) -> Self {
        let mut raw = zeroize::Zeroizing::new([0; 32]);
        rng.fill_bytes(raw.as_mut_slice());
        raw[0] &= 248;
        raw[31] = (raw[31] & 127) | 64;
        let public =
            curve25519_dalek::montgomery::MontgomeryPoint::mul_base_clamped(*raw).to_bytes();
        Self::new(PublicKey(public), PrivateKey::from_raw(*raw))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct IdentityKey(PublicKey);
impl IdentityKey {
    pub fn new(public: PublicKey) -> Self {
        Self(public)
    }
    pub fn decode(bytes: &[u8]) -> Result<Self> {
        PublicKey::deserialize(bytes).map(Self)
    }
    pub fn public_key(&self) -> &PublicKey {
        &self.0
    }
    pub fn serialize(&self) -> [u8; 33] {
        self.0.serialize()
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentityKeyPair {
    identity_key: IdentityKey,
    private_key: PrivateKey,
}
impl IdentityKeyPair {
    pub fn new(identity_key: IdentityKey, private_key: PrivateKey) -> Self {
        Self {
            identity_key,
            private_key,
        }
    }
    pub fn generate<R: Rng + CryptoRng>(rng: &mut R) -> Self {
        let pair = KeyPair::generate(rng);
        Self::new(IdentityKey::new(pair.public_key), pair.private_key)
    }
    pub fn identity_key(&self) -> &IdentityKey {
        &self.identity_key
    }
    pub fn public_key(&self) -> &PublicKey {
        self.identity_key.public_key()
    }
    pub fn private_key(&self) -> &PrivateKey {
        &self.private_key
    }
}
/// Decompresses the public point once for repeated XEdDSA checks.
#[derive(Clone, Debug)]
pub struct PreparedVerifyingKey {
    point: curve25519_dalek::edwards::EdwardsPoint,
    encoding: [u8; 32],
}
impl PreparedVerifyingKey {
    pub fn new(public: &PublicKey) -> Result<Self> {
        use curve25519_dalek::montgomery::MontgomeryPoint;
        let point = MontgomeryPoint(public.0);
        let p0 = point.to_edwards(0).ok_or(CurveError::InvalidKey)?;
        if p0.is_small_order() {
            return Err(CurveError::InvalidKey);
        }
        let mut prime = [255; 32];
        prime[0] = 237;
        prime[31] = 127;
        if public.0.iter().rev().cmp(prime.iter().rev()).is_ge() {
            return Err(CurveError::InvalidKey);
        }
        Ok(Self {
            point: p0,
            encoding: p0.compress().to_bytes(),
        })
    }
    pub fn verify_signature(&self, message: &[u8], signature: &[u8]) -> bool {
        use curve25519_dalek::{edwards::EdwardsPoint, scalar::Scalar};
        use sha2::{Digest, Sha512};
        use subtle::ConstantTimeEq;
        let Ok(sig) = <&[u8; 64]>::try_from(signature) else {
            return false;
        };
        let mut s = [0; 32];
        s.copy_from_slice(&sig[32..]);
        s[31] &= 127;
        if s[31] & 224 != 0 {
            return false;
        }
        let negative;
        let point = if sig[63] & 128 != 0 {
            negative = -self.point;
            &negative
        } else {
            &self.point
        };
        let mut encoding = self.encoding;
        encoding[31] |= sig[63] & 128;
        let mut h = Sha512::new();
        h.update(&sig[..32]);
        h.update(encoding);
        h.update(message);
        let h = Scalar::from_bytes_mod_order_wide(&h.finalize().into());
        let r = EdwardsPoint::vartime_double_scalar_mul_basepoint(
            &-h,
            point,
            &Scalar::from_bytes_mod_order(s),
        )
        .compress();
        bool::from(r.as_bytes().ct_eq(&sig[..32]))
    }
}
/// Keeps the normalized scalar and Edwards encoding for repeated signing.
#[derive(Clone)]
pub struct PreparedSigningKey {
    scalar: curve25519_dalek::scalar::Scalar,
    public: [u8; 32],
}
impl Drop for PreparedSigningKey {
    fn drop(&mut self) {
        self.scalar.zeroize()
    }
}
impl std::fmt::Debug for PreparedSigningKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PreparedSigningKey([redacted])")
    }
}
impl PreparedSigningKey {
    pub fn new(private: &PrivateKey) -> Self {
        Self::prepare(private.serialize())
    }
    #[inline]
    fn prepare(private: &[u8; 32]) -> Self {
        use curve25519_dalek::{edwards::EdwardsPoint, scalar::Scalar};
        use subtle::{Choice, ConditionallySelectable};
        let mut raw = zeroize::Zeroizing::new(*private);
        raw[0] &= 248;
        raw[31] = (raw[31] & 127) | 64;
        let scalar = zeroize::Zeroizing::new(Scalar::from_bytes_mod_order(*raw));
        let point = EdwardsPoint::mul_base(&scalar);
        let mut public = point.compress().to_bytes();
        let scalar = Scalar::conditional_select(&scalar, &-*scalar, Choice::from(public[31] >> 7));
        public[31] &= 127;
        Self { scalar, public }
    }
    pub fn calculate_signature<R: Rng + CryptoRng>(
        &self,
        message: &[u8],
        rng: &mut R,
    ) -> Result<[u8; 64]> {
        self.calculate_signature_with_random_source(message, &mut RandomAdapter(rng))
            .map_err(|_| CurveError::Crypto)
    }
    pub fn calculate_signature_with_random_source(
        &self,
        message: &[u8],
        rng: &mut dyn RandomSource,
    ) -> crate::engine::Result<[u8; 64]> {
        use curve25519_dalek::{edwards::EdwardsPoint, scalar::Scalar};
        use sha2::{Digest, Sha512};
        let mut random = zeroize::Zeroizing::new([0; 64]);
        rng.fill(random.as_mut_slice())?;
        let mut domain = [255; 32];
        domain[0] = 254;
        let mut h = Sha512::new();
        h.update(domain);
        h.update(self.scalar.as_bytes());
        h.update(message);
        h.update(random.as_slice());
        let digest = zeroize::Zeroizing::new(<[u8; 64]>::from(h.finalize()));
        let nonce = zeroize::Zeroizing::new(Scalar::from_bytes_mod_order_wide(&digest));
        let r = EdwardsPoint::mul_base(&nonce).compress();
        let mut h = Sha512::new();
        h.update(r.as_bytes());
        h.update(self.public);
        h.update(message);
        let challenge = Scalar::from_bytes_mod_order_wide(&h.finalize().into());
        let s = *nonce + challenge * self.scalar;
        let mut signature = [0; 64];
        signature[..32].copy_from_slice(r.as_bytes());
        signature[32..].copy_from_slice(s.as_bytes());
        Ok(signature)
    }
}
impl PrivateKey {
    pub fn prepare_signing_key(&self) -> PreparedSigningKey {
        PreparedSigningKey::new(self)
    }
}
impl TryFrom<&[u8]> for IdentityKey {
    type Error = CurveError;
    fn try_from(bytes: &[u8]) -> Result<Self> {
        Self::decode(bytes)
    }
}
impl From<PublicKey> for IdentityKey {
    fn from(public: PublicKey) -> Self {
        Self::new(public)
    }
}
impl From<KeyPair> for IdentityKeyPair {
    fn from(pair: KeyPair) -> Self {
        Self::new(pair.public_key.into(), pair.private_key)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn signed_fixture() -> (PublicKey, [u8; 64]) {
        use rand::SeedableRng;
        let private = PrivateKey::deserialize(&[7; 32]).unwrap();
        let public = private.public_key().unwrap();
        let signature = private
            .calculate_signature(b"synthetic", &mut rand::rngs::StdRng::from_seed([9; 32]))
            .unwrap();
        (public, signature)
    }
    fn field_modulus() -> [u8; 32] {
        let mut p = [255; 32];
        p[0] = 237;
        p[31] = 127;
        p
    }
    #[test]
    fn ordinary_and_prepared_reject_noncanonical_and_low_order_public_keys() {
        let (public, signature) = signed_fixture();
        assert!(public.verify_signature(b"synthetic", &signature));
        let p = field_modulus();
        let mut p_plus_one = p;
        p_plus_one[0] += 1;
        let mut minus_one = p;
        minus_one[0] -= 1;
        let mut one = [0; 32];
        one[0] = 1;
        let mut high_bit = *public.public_key_bytes();
        high_bit[31] |= 128;
        let mut invalid = vec![p, p_plus_one, high_bit, [0; 32], one, minus_one];
        invalid.extend(
            curve25519_dalek::constants::EIGHT_TORSION
                .iter()
                .map(|point| point.to_montgomery().to_bytes()),
        );
        for raw in invalid {
            let public = PublicKey::from_djb_public_key_bytes(&raw).unwrap();
            assert!(
                !public.verify_signature(b"synthetic", &signature),
                "{raw:02x?}"
            );
            assert!(PreparedVerifyingKey::new(&public).is_err(), "{raw:02x?}");
        }
    }
    #[test]
    fn ordinary_and_prepared_signature_boundaries_preserve_xeddsa_unreduced_s() {
        let (public, signature) = signed_fixture();
        let prepared = public.prepare_verifying_key().unwrap();
        assert!(prepared.verify_signature(b"synthetic", &signature));
        let p = field_modulus();
        let mut p_plus_one = p;
        p_plus_one[0] += 1;
        let mut invalid = Vec::new();
        for y in [p, p_plus_one] {
            let mut changed = signature;
            changed[..32].copy_from_slice(&y);
            invalid.push(changed);
        }
        for bit in [0x20, 0x40, 0x80] {
            let mut changed = signature;
            changed[63] ^= bit;
            invalid.push(changed);
        }
        for changed in invalid {
            assert!(!public.verify_signature(b"synthetic", &changed));
            assert!(!prepared.verify_signature(b"synthetic", &changed));
        }
        // XEdDSA section 5 permits unreduced S below 2^253. L is the
        // Ed25519 subgroup order specified in RFC 8032 section 5.1.
        let order = hex::decode("edd3f55c1a631258d69cf7a2def9de1400000000000000000000000000000010")
            .unwrap();
        let mut unreduced = signature;
        let mut carry = 0u16;
        for (byte, addend) in unreduced[32..].iter_mut().zip(order) {
            let sum = u16::from(*byte) + u16::from(addend) + carry;
            *byte = sum as u8;
            carry = sum >> 8;
        }
        assert_eq!(carry, 0);
        assert_eq!(unreduced[63] & 0xe0, 0);
        assert!(public.verify_signature(b"synthetic", &unreduced));
        assert!(prepared.verify_signature(b"synthetic", &unreduced));
    }

    #[test]
    fn signing_preparation_preserves_public_keys_and_signature_bytes() {
        use rand::SeedableRng;
        let mut random = rand::rngs::StdRng::from_seed([21; 32]);
        for _ in 0..32 {
            let pair = KeyPair::generate(&mut random);
            let prepared = PreparedSigningKey::new(&pair.private_key);
            let mut first = rand::rngs::StdRng::from_seed([22; 32]);
            let mut second = rand::rngs::StdRng::from_seed([22; 32]);
            let signature = prepared
                .calculate_signature(b"synthetic matching pair", &mut first)
                .unwrap();
            assert_eq!(
                signature,
                pair.private_key
                    .calculate_signature(b"synthetic matching pair", &mut second)
                    .unwrap()
            );
            assert!(
                pair.public_key
                    .verify_signature(b"synthetic matching pair", &signature)
            );
            let other = KeyPair::generate(&mut random);
            assert!(
                !other
                    .public_key
                    .verify_signature(b"synthetic matching pair", &signature)
            );
        }
    }

    #[test]
    fn retained_private_keys_match_uncached_signing_without_changing_storage() {
        use rand::SeedableRng;
        for byte in [0, 1, 37, 128, 255] {
            let private = PrivateKey::deserialize(&[byte; 32]).unwrap();
            let cold_copy = private.clone();
            let encoded = serde_json::to_vec(&private).unwrap();
            let mut binary = Vec::new();
            ciborium::ser::into_writer(&private, &mut binary).unwrap();
            let mut cached_random = rand::rngs::StdRng::from_seed([byte; 32]);
            let mut direct_random = rand::rngs::StdRng::from_seed([byte; 32]);
            let mut copy_random = rand::rngs::StdRng::from_seed([byte; 32]);
            for message in [&b""[..], &b"synthetic"[..], &[0; 129][..]] {
                let signature = private
                    .calculate_signature(message, &mut cached_random)
                    .unwrap();
                let direct = RustCrypto
                    .sign(
                        private.serialize(),
                        message,
                        &mut RandomAdapter(&mut direct_random),
                    )
                    .unwrap();
                assert_eq!(signature, direct);
                assert_eq!(
                    private
                        .clone()
                        .calculate_signature(message, &mut copy_random)
                        .unwrap(),
                    direct
                );
                assert_eq!(private, cold_copy);
                assert_eq!(private.serialize(), &[byte; 32]);
                assert_eq!(serde_json::to_vec(&private).unwrap(), encoded);
                let mut warm_binary = Vec::new();
                ciborium::ser::into_writer(&private, &mut warm_binary).unwrap();
                assert_eq!(warm_binary, binary);
                assert!(
                    private
                        .public_key()
                        .unwrap()
                        .verify_signature(message, &signature)
                );
            }
        }
    }

    #[test]
    fn key_encodings_and_cached_signatures() {
        assert_eq!(
            PublicKey::deserialize(&[]),
            Err(CurveError::NoKeyTypeIdentifier)
        );
        assert_eq!(
            IdentityKey::try_from(&[][..]),
            Err(CurveError::NoKeyTypeIdentifier)
        );
        assert!(IdentityKey::try_from(&[5; 32][..]).is_err());
        assert!(IdentityKey::try_from(&[6; 33][..]).is_err());
        let tagged = PublicKey::from_djb_public_key_bytes(&[7; 32])
            .unwrap()
            .serialize();
        assert_eq!(
            IdentityKey::try_from(tagged.as_slice())
                .unwrap()
                .serialize(),
            tagged
        );

        use rand::SeedableRng;
        let mut rng = rand::rngs::StdRng::from_seed([7; 32]);
        let pair = KeyPair::generate(&mut rng);
        assert_eq!(pair.public_key.serialize()[0], 5);
        assert_eq!(
            PublicKey::deserialize(&pair.public_key.serialize()).unwrap(),
            pair.public_key
        );
        assert_eq!(
            PrivateKey::deserialize(pair.private_key.serialize()).unwrap(),
            pair.private_key
        );
        assert_eq!(pair.private_key.public_key().unwrap(), pair.public_key);
        let cached = pair.private_key.prepare_signing_key();
        let verifier = pair.public_key.prepare_verifying_key().unwrap();
        let mut first = rand::rngs::StdRng::from_seed([8; 32]);
        let mut second = rand::rngs::StdRng::from_seed([8; 32]);
        let signature = cached
            .calculate_signature(b"synthetic", &mut first)
            .unwrap();
        assert_eq!(
            signature,
            pair.private_key
                .calculate_signature(b"synthetic", &mut second)
                .unwrap()
        );
        assert!(verifier.verify_signature(b"synthetic", &signature));
        assert!(!verifier.verify_signature(b"modified", &signature));
        assert!(pair.public_key.verify_signature(b"synthetic", &signature));
        let mut transported = cached.clone();
        transported.scalar = -transported.scalar;
        transported.public[31] |= 128;
        let mut signature = transported
            .calculate_signature(b"sign bit", &mut rng)
            .unwrap();
        signature[63] |= 128;
        assert!(verifier.verify_signature(b"sign bit", &signature));
        assert!(pair.public_key.verify_signature(b"sign bit", &signature));

        assert!(PublicKey::deserialize(pair.public_key.public_key_bytes()).is_err());
    }
}

#[cfg(test)]
mod serde_tests {
    use super::*;
    struct BinaryBytes<'de>(&'de [u8]);
    impl<'de> serde::Deserializer<'de> for BinaryBytes<'de> {
        type Error = serde::de::value::Error;
        fn is_human_readable(&self) -> bool {
            false
        }
        fn deserialize_any<V: serde::de::Visitor<'de>>(
            self,
            visitor: V,
        ) -> std::result::Result<V::Value, Self::Error> {
            visitor.visit_borrowed_bytes(self.0)
        }
        serde::forward_to_deserialize_any! {bool i8 i16 i32 i64 u8 u16 u32 u64 f32 f64 char str string bytes byte_buf option unit unit_struct newtype_struct seq tuple tuple_struct map struct enum identifier ignored_any}
    }
    #[test]
    fn binary_key_bytes_decode_tagged_public_and_raw_private() {
        let public = PublicKey::from_djb_public_key_bytes(&[0xab; 32]).unwrap();
        let encoded = public.serialize();
        assert_eq!(
            <PublicKey as Deserialize>::deserialize(BinaryBytes(&encoded)).unwrap(),
            public
        );
        let raw = [0xcd; 32];
        assert_eq!(
            <PrivateKey as Deserialize>::deserialize(BinaryBytes(&raw))
                .unwrap()
                .serialize(),
            &raw
        );
        assert!(<PublicKey as Deserialize>::deserialize(BinaryBytes(&raw)).is_err());
        assert!(<PrivateKey as Deserialize>::deserialize(BinaryBytes(&encoded)).is_err());
    }

    #[test]
    fn historical_key_json_encodings_and_field_names() {
        let public = PublicKey::from_djb_public_key_bytes(&[0xab; 32]).unwrap();
        let private = PrivateKey::deserialize(&[0xcd; 32]).unwrap();
        let public_text = format!("05{}", "ab".repeat(32));
        let private_text = "cd".repeat(32);
        assert_eq!(
            serde_json::to_string(&public).unwrap(),
            format!("\"{public_text}\"")
        );
        assert_eq!(
            serde_json::to_string(&private).unwrap(),
            format!("\"{private_text}\"")
        );
        let pair = KeyPair::new(public, private);
        let expected =
            format!("{{\"public_key\":\"{public_text}\",\"private_key\":\"{private_text}\"}}");
        assert_eq!(serde_json::to_string(&pair).unwrap(), expected);
        assert_eq!(serde_json::from_str::<KeyPair>(&expected).unwrap(), pair);
        let identity = IdentityKey::new(public);
        assert_eq!(
            serde_json::to_string(&identity).unwrap(),
            format!("\"{public_text}\"")
        );
        let identity_pair = IdentityKeyPair::from(pair);
        let expected =
            format!("{{\"identity_key\":\"{public_text}\",\"private_key\":\"{private_text}\"}}");
        assert_eq!(serde_json::to_string(&identity_pair).unwrap(), expected);
        assert_eq!(
            serde_json::from_str::<IdentityKeyPair>(&expected).unwrap(),
            identity_pair
        );
    }
    #[test]
    fn serde_rejects_invalid_key_lengths_tags_and_hex() {
        for value in [
            "",
            "05",
            &"00".repeat(32),
            &"06".repeat(33),
            &"gg".repeat(33),
        ] {
            let json = format!("\"{value}\"");
            assert!(serde_json::from_str::<PublicKey>(&json).is_err());
        }
        for value in [
            "",
            "00",
            &"cd".repeat(31),
            &"cd".repeat(33),
            &"gg".repeat(32),
        ] {
            let json = format!("\"{value}\"");
            assert!(serde_json::from_str::<PrivateKey>(&json).is_err());
        }
        assert!(
            serde_json::from_str::<PublicKey>(&format!("[{}]", vec!["0"; 32].join(","))).is_err()
        );
    }
}

// SPDX-License-Identifier: MIT
use super::{edwards::EdwardsPoint, scalar::Scalar};
use sha2::{Digest, Sha512};
use zeroize::Zeroizing;

pub(crate) fn scalar_hash(hash: Sha512) -> Scalar {
    let digest = Zeroizing::new(<[u8; 64]>::from(hash.finalize()));
    Scalar::from_bytes_mod_order_wide(&digest)
}

// The scalar and encoding form the caller's normalized signing key. Sharing
// this core avoids duplicating nonce generation and the signature equation.
#[inline(never)]
pub(crate) fn sign(
    scalar: &Scalar,
    public: &[u8; 32],
    message: &[u8],
    randomness: &[u8; 64],
) -> [u8; 64] {
    let mut domain = [255; 32];
    domain[0] = 254;
    let mut hash = Sha512::new();
    hash.update(domain);
    hash.update(scalar.as_bytes());
    hash.update(message);
    hash.update(randomness);
    let nonce = Zeroizing::new(scalar_hash(hash));
    let r = EdwardsPoint::mul_base(&nonce).compress();
    let mut hash = Sha512::new();
    hash.update(r.as_bytes());
    hash.update(public);
    hash.update(message);
    let challenge = scalar_hash(hash);
    let s = *nonce + challenge * *scalar;
    let mut signature = [0; 64];
    signature[..32].copy_from_slice(r.as_bytes());
    signature[32..].copy_from_slice(s.as_bytes());
    signature
}

pub(crate) struct ParsedSignature<'a> {
    bytes: &'a [u8; 64],
    scalar: Scalar,
}

impl<'a> ParsedSignature<'a> {
    pub(crate) fn parse(bytes: &'a [u8]) -> Option<Self> {
        let bytes: &[u8; 64] = bytes.try_into().ok()?;
        let mut encoded = [0; 32];
        encoded.copy_from_slice(&bytes[32..]);
        let scalar = Scalar::from_signature_bytes(encoded)?;
        Some(Self { bytes, scalar })
    }

    pub(crate) fn sign(&self) -> u8 {
        self.bytes[63] >> 7
    }
}

// Callers validate the public key and pass its point and matching encoding
// with the transported sign. Parsing binds the checked scalar to these bytes.
#[inline(never)]
pub(crate) fn verify(
    point: &EdwardsPoint,
    encoding: &[u8; 32],
    message: &[u8],
    signature: &ParsedSignature<'_>,
) -> bool {
    use subtle::ConstantTimeEq;
    let mut hash = Sha512::new();
    hash.update(&signature.bytes[..32]);
    hash.update(encoding);
    hash.update(message);
    let challenge = scalar_hash(hash);
    let r =
        EdwardsPoint::vartime_double_scalar_mul_basepoint(&-challenge, point, &signature.scalar)
            .compress();
    bool::from(r.as_bytes().ct_eq(&signature.bytes[..32]))
}

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

/// A validated verifying point and the precomputed tables its backend uses
/// for repeated verification.
#[derive(Clone)]
pub(crate) struct PreparedPoint {
    point: EdwardsPoint,
    #[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
    segments: Option<Box<super::avx2::SegmentTables>>,
}

impl core::fmt::Debug for PreparedPoint {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("PreparedPoint")
            .field("point", &self.point)
            .finish_non_exhaustive()
    }
}

impl PreparedPoint {
    pub(crate) fn new(point: EdwardsPoint) -> Self {
        Self {
            point,
            #[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
            segments: super::avx2::prepare(&point),
        }
    }

    /// Heap owned by this point, excluding the process-wide basepoint tables.
    pub(crate) fn heap_bytes(&self) -> usize {
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
        if self.segments.is_some() {
            return size_of::<super::avx2::SegmentTables>();
        }
        0
    }
}

fn challenge(encoding: &[u8; 32], message: &[u8], signature: &ParsedSignature<'_>) -> Scalar {
    let mut hash = Sha512::new();
    hash.update(&signature.bytes[..32]);
    hash.update(encoding);
    hash.update(message);
    scalar_hash(hash)
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
    let challenge = challenge(encoding, message, signature);
    let r =
        EdwardsPoint::vartime_double_scalar_mul_basepoint(&-challenge, point, &signature.scalar)
            .compress();
    bool::from(r.as_bytes().ct_eq(&signature.bytes[..32]))
}

// `prepared` holds the positive point; the transported sign selects -A.
// `encoding` already carries that sign.
#[inline(never)]
pub(crate) fn verify_prepared(
    prepared: &PreparedPoint,
    encoding: &[u8; 32],
    message: &[u8],
    signature: &ParsedSignature<'_>,
) -> bool {
    use subtle::ConstantTimeEq;
    #[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
    if let Some(segments) = &prepared.segments {
        let challenge = challenge(encoding, message, signature);
        // [-h](-A) = [h]A, so the sign flips the scalar instead of the tables.
        let a = if signature.sign() != 0 {
            challenge
        } else {
            -challenge
        };
        let r = super::avx2::mul_prepared(&a, segments, &signature.scalar).compress();
        return bool::from(r.as_bytes().ct_eq(&signature.bytes[..32]));
    }
    let point = if signature.sign() != 0 {
        -prepared.point
    } else {
        prepared.point
    };
    verify(&point, encoding, message, signature)
}

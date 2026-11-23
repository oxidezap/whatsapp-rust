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
        let challenge = -challenge(encoding, message, signature);
        // Scalars are reduced, so `verify` multiplies -A by the integer l - h.
        // [l - h](-A) + [s]B = -([l - h]A + [l - s]B) holds for mixed-order A
        // too, because only B is known to have order l. [h]A + [s]B would
        // differ by [l]A for keys with a small-order component.
        let r = if signature.sign() != 0 {
            -super::avx2::mul_prepared(&challenge, segments, &-signature.scalar)
        } else {
            super::avx2::mul_prepared(&challenge, segments, &signature.scalar)
        };
        return bool::from(r.compress().as_bytes().ct_eq(&signature.bytes[..32]));
    }
    let point = if signature.sign() != 0 {
        -prepared.point
    } else {
        prepared.point
    };
    verify(&point, encoding, message, signature)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prepared_verification_matches_for_mixed_order_keys() {
        let mut one = [0; 32];
        one[0] = 1;
        let one = Scalar::from_bytes_mod_order(one);
        let mut accepted = [0usize; 2];
        for seed in 0u32..256 {
            let mut wide: [u8; 64] = Sha512::digest(seed.to_le_bytes()).into();
            let key = Scalar::from_bytes_mod_order_wide(&wide);
            wide.reverse();
            let nonce = Scalar::from_bytes_mod_order_wide(&wide);
            // A = [key]B + T with T of order 2, 4 or 8, so [l]A != identity.
            let point = EdwardsPoint::vartime_double_scalar_mul_basepoint(
                &one,
                &super::super::EIGHT_TORSION[1 + seed as usize % 7],
                &key,
            );
            let prepared = PreparedPoint::new(point);
            let message = seed.to_be_bytes();
            for sign in [0u8, 1] {
                let signer = if sign != 0 { -point } else { point };
                let mut encoding = point.compress().to_bytes();
                encoding[31] |= sign << 7;
                let mut bytes = [0; 64];
                bytes[..32].copy_from_slice(EdwardsPoint::mul_base(&nonce).compress().as_bytes());
                let h = challenge(
                    &encoding,
                    &message,
                    &ParsedSignature::parse(&bytes).unwrap(),
                );
                // `verify` accepts iff the torsion part [l - h](±T) vanishes,
                // which holds only for some challenges.
                let signer_key = if sign != 0 { -key } else { key };
                let s = nonce + h * signer_key;
                bytes[32..].copy_from_slice(s.as_bytes());
                bytes[63] |= sign << 7;
                let signature = ParsedSignature::parse(&bytes).unwrap();
                let ordinary = verify(&signer, &encoding, &message, &signature);
                assert_eq!(
                    verify_prepared(&prepared, &encoding, &message, &signature),
                    ordinary,
                    "seed {seed} sign {sign}"
                );
                accepted[usize::from(sign)] += usize::from(ordinary);
            }
        }
        assert!(accepted.iter().all(|&count| count > 0), "{accepted:?}");
    }
}

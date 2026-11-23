//! Fixed-base Edwards scalar multiplication on `FieldElement64`, for x86_64
//! with BMI2 only. Same algorithm, digits and table points as
//! `EdwardsPoint::mul_base`; only the field representation differs.

#![allow(non_snake_case)]

use super::models::AffineNielsPoint;
use super::{
    edwards::EdwardsPoint, field64::FieldElement64 as Fe, scalar::Scalar, tables::BASEPOINT_TABLE,
    window::LookupTable,
};
use subtle::{Choice, ConditionallySelectable};

/// `repr(C)`: `select` reads each entry as six 16-byte vectors.
#[derive(Clone, Copy)]
#[repr(C)]
struct AffineNiels64 {
    y_plus_x: Fe,
    y_minus_x: Fe,
    xy2d: Fe,
}

#[derive(Clone, Copy)]
struct Extended64 {
    X: Fe,
    Y: Fe,
    Z: Fe,
    T: Fe,
}

struct Completed64 {
    X: Fe,
    Y: Fe,
    Z: Fe,
    T: Fe,
}

const fn convert_table(table: &LookupTable<AffineNielsPoint>) -> [AffineNiels64; 8] {
    let mut out = [AffineNiels64 {
        y_plus_x: Fe::ZERO,
        y_minus_x: Fe::ZERO,
        xy2d: Fe::ZERO,
    }; 8];
    let mut i = 0;
    while i < 8 {
        let point = &table.0[i];
        out[i] = AffineNiels64 {
            y_plus_x: Fe::from_radix51(point.y_plus_x.0),
            y_minus_x: Fe::from_radix51(point.y_minus_x.0),
            xy2d: Fe::from_radix51(point.xy2d.0),
        };
        i += 1;
    }
    out
}

/// `BASEPOINT_TABLE` converted to radix 2^64 during constant evaluation.
static BASEPOINT_TABLE64: [[AffineNiels64; 8]; 32] = {
    let mut out = [[AffineNiels64 {
        y_plus_x: Fe::ZERO,
        y_minus_x: Fe::ZERO,
        xy2d: Fe::ZERO,
    }; 8]; 32];
    let mut i = 0;
    while i < 32 {
        out[i] = convert_table(&BASEPOINT_TABLE[i]);
        i += 1;
    }
    out
};

/// Constant-time `x * P_j` lookup from a table of `[P, 2P, ..., 8P]`, for
/// `x` in `-8..=8`, like `LookupTable::select`.
///
/// Every entry is read and masked with SSE2 (part of the x86_64 baseline):
/// an entry is six 16-byte vectors, so the scan takes 6 loads per entry
/// instead of 12, keeps its accumulators in registers, and derives the
/// one-hot masks with vector compares instead of a `subtle` barrier per
/// entry.
#[inline(always)]
fn select(table: &[AffineNiels64; 8], x: i8) -> AffineNiels64 {
    use core::arch::x86_64::{
        __m128i, _mm_and_si128, _mm_cmpeq_epi32, _mm_loadu_si128, _mm_or_si128, _mm_set_epi64x,
        _mm_set1_epi32, _mm_setzero_si128, _mm_storeu_si128,
    };

    let mask = (x as i16) >> 7;
    let abs = ((x as i16) + mask) ^ mask;

    let mut out = AffineNiels64 {
        y_plus_x: Fe::ZERO,
        y_minus_x: Fe::ZERO,
        xy2d: Fe::ZERO,
    };
    // Layout the vector loads and stores below rely on.
    const _: () = assert!(size_of::<AffineNiels64>() == 96);
    // SAFETY: SSE2 is enabled on every x86_64 target. `AffineNiels64` is
    // `repr(C)` with three `repr(transparent)` `[u64; 4]` fields and no
    // padding, so the 96 bytes of each table entry and of `out` are six
    // unaligned 16-byte vectors.
    #[allow(unsafe_code)]
    unsafe {
        // All 32-bit lanes are equal, so a lane-wise compare yields an
        // all-ones or all-zeros vector.
        let wanted = _mm_set1_epi32(i32::from(abs));
        // `abs == 0` selects the identity (1, 1, 0).
        let identity = _mm_and_si128(
            _mm_cmpeq_epi32(wanted, _mm_setzero_si128()),
            _mm_set_epi64x(0, 1),
        );
        let mut acc = [_mm_setzero_si128(); 6];
        acc[0] = identity;
        acc[2] = identity;
        for (j, point) in table.iter().enumerate() {
            let m = _mm_cmpeq_epi32(wanted, _mm_set1_epi32(j as i32 + 1));
            let lanes = (point as *const AffineNiels64).cast::<__m128i>();
            for (k, acc) in acc.iter_mut().enumerate() {
                *acc = _mm_or_si128(*acc, _mm_and_si128(m, _mm_loadu_si128(lanes.add(k))));
            }
        }
        let dst = (&mut out as *mut AffineNiels64).cast::<__m128i>();
        for (k, acc) in acc.iter().enumerate() {
            _mm_storeu_si128(dst.add(k), *acc);
        }
    }

    // -(y + x, y - x, 2dxy) = (y - x, y + x, -2dxy)
    let negative = Choice::from((mask & 1) as u8);
    Fe::conditional_swap(&mut out.y_plus_x, &mut out.y_minus_x, negative);
    let neg_xy2d = Fe::ZERO.sub(&out.xy2d);
    for k in 0..4 {
        out.xy2d.0[k] = u64::conditional_select(&out.xy2d.0[k], &neg_xy2d.0[k], negative);
    }
    out
}

impl Extended64 {
    #[inline(always)]
    fn add_affine(&self, other: &AffineNiels64) -> Completed64 {
        let Y_plus_X = self.Y.add(&self.X);
        let Y_minus_X = self.Y.sub(&self.X);
        let PP = Y_plus_X.mul(&other.y_plus_x);
        let MM = Y_minus_X.mul(&other.y_minus_x);
        let Txy2d = self.T.mul(&other.xy2d);
        let Z2 = self.Z.add(&self.Z);
        Completed64 {
            X: PP.sub(&MM),
            Y: PP.add(&MM),
            Z: Z2.add(&Txy2d),
            T: Z2.sub(&Txy2d),
        }
    }

    /// Doubling on the projective part (`T` is not read), as
    /// `ProjectivePoint::double`.
    #[inline(always)]
    fn double(&self) -> Completed64 {
        let XX = self.X.square();
        let YY = self.Y.square();
        let ZZ = self.Z.square();
        let ZZ2 = ZZ.add(&ZZ);
        let X_plus_Y_sq = self.X.add(&self.Y).square();
        let YY_plus_XX = YY.add(&XX);
        let YY_minus_XX = YY.sub(&XX);
        Completed64 {
            X: X_plus_Y_sq.sub(&YY_plus_XX),
            Y: YY_plus_XX,
            Z: YY_minus_XX,
            T: ZZ2.sub(&YY_minus_XX),
        }
    }
}

impl Completed64 {
    #[inline(always)]
    fn as_extended(&self) -> Extended64 {
        Extended64 {
            X: self.X.mul(&self.T),
            Y: self.Y.mul(&self.Z),
            Z: self.Z.mul(&self.T),
            T: self.X.mul(&self.Y),
        }
    }

    /// Projective coordinates only; `T` is left at zero and must not be read.
    #[inline(always)]
    fn as_projective(&self) -> Extended64 {
        Extended64 {
            X: self.X.mul(&self.T),
            Y: self.Y.mul(&self.Z),
            Z: self.Z.mul(&self.T),
            T: Fe::ZERO,
        }
    }
}

/// `EdwardsPoint::mul_base` on radix-2^64 limbs.
#[target_feature(enable = "bmi2")]
pub(super) fn mul_base_bmi2(scalar: &Scalar) -> EdwardsPoint {
    let digits = scalar.as_radix_16();
    let mut point = Extended64 {
        X: Fe::ZERO,
        Y: Fe::ONE,
        Z: Fe::ONE,
        T: Fe::ZERO,
    };
    for i in (1..64).step_by(2) {
        point = point
            .add_affine(&select(&BASEPOINT_TABLE64[i / 2], digits[i]))
            .as_extended();
    }
    // Multiply by 16: three projective doublings, then one to extended.
    for _ in 0..3 {
        point = point.double().as_projective();
    }
    point = point.double().as_extended();
    for i in (0..64).step_by(2) {
        point = point
            .add_affine(&select(&BASEPOINT_TABLE64[i / 2], digits[i]))
            .as_extended();
    }
    EdwardsPoint {
        X: point.X.to_field51(),
        Y: point.Y.to_field51(),
        Z: point.Z.to_field51(),
        T: point.T.to_field51(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha512};

    #[test]
    fn select_matches_portable_lookup_for_every_digit() {
        for (table, table64) in BASEPOINT_TABLE.iter().zip(&BASEPOINT_TABLE64) {
            for x in -8i8..=8 {
                let got = select(table64, x);
                let want = table.select(x);
                for (fe64, fe) in [
                    (got.y_plus_x, want.y_plus_x),
                    (got.y_minus_x, want.y_minus_x),
                    (got.xy2d, want.xy2d),
                ] {
                    assert_eq!(fe64.to_field51().to_bytes(), fe.to_bytes(), "digit {x}");
                }
            }
        }
    }

    #[test]
    fn bmi2_mul_base_matches_portable() {
        if !std::is_x86_feature_detected!("bmi2") {
            return;
        }
        let mut scalars = vec![Scalar::from_bytes_mod_order([0; 32])];
        let mut one = [0u8; 32];
        one[0] = 1;
        scalars.push(Scalar::from_bytes_mod_order(one));
        for seed in 0u32..256 {
            let wide: [u8; 64] = Sha512::digest(seed.to_le_bytes()).into();
            let mut bytes: [u8; 32] = wide[..32].try_into().unwrap();
            if seed % 2 == 0 {
                // X25519 clamping, as in the key-generation callers.
                bytes[0] &= 248;
                bytes[31] = (bytes[31] & 127) | 64;
            }
            scalars.push(Scalar::from_bytes_mod_order(bytes));
        }
        for (i, scalar) in scalars.iter().enumerate() {
            // SAFETY: BMI2 support is checked above.
            #[allow(unsafe_code)]
            let fast = unsafe { mul_base_bmi2(scalar) };
            let portable = EdwardsPoint::mul_base_generic(scalar);
            assert_eq!(
                fast.compress().to_bytes(),
                portable.compress().to_bytes(),
                "scalar {i}"
            );
            assert_eq!(
                fast.to_montgomery().to_bytes(),
                portable.to_montgomery().to_bytes(),
                "scalar {i}"
            );
            // T must stay consistent with X, Y and Z for later additions.
            assert!(&fast.T * &fast.Z == &fast.X * &fast.Y, "scalar {i}");
        }
    }
}

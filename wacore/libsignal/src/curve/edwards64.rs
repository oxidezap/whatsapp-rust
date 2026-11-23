//! Fixed-base Edwards scalar multiplication on `FieldElement64`, for x86_64
//! with BMI2 only. Same algorithm, digits and table points as
//! `EdwardsPoint::mul_base`; only the field representation differs.

#![allow(non_snake_case)]

use super::models::AffineNielsPoint;
use super::{
    edwards::EdwardsPoint, field64::FieldElement64 as Fe, scalar::Scalar, tables::BASEPOINT_TABLE,
    window::LookupTable,
};
use subtle::{Choice, ConditionallySelectable, ConstantTimeEq};

#[derive(Clone, Copy)]
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
#[inline(always)]
fn select(table: &[AffineNiels64; 8], x: i8) -> AffineNiels64 {
    let mask = (x as i16) >> 7;
    let abs = ((x as i16) + mask) ^ mask;
    let is_zero = (abs as u16).ct_eq(&0);
    let mut out = AffineNiels64 {
        y_plus_x: Fe([is_zero.unwrap_u8() as u64, 0, 0, 0]),
        y_minus_x: Fe([is_zero.unwrap_u8() as u64, 0, 0, 0]),
        xy2d: Fe::ZERO,
    };
    for (j, point) in table.iter().enumerate() {
        // The choices are one-hot, so OR-ing the masked entries selects one.
        let m = ((abs as u16).ct_eq(&(j as u16 + 1)).unwrap_u8() as u64).wrapping_neg();
        for k in 0..4 {
            out.y_plus_x.0[k] |= point.y_plus_x.0[k] & m;
            out.y_minus_x.0[k] |= point.y_minus_x.0[k] & m;
            out.xy2d.0[k] |= point.xy2d.0[k] & m;
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

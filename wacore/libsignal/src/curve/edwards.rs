// Portions derived from curve25519-dalek 5.0.0. See LICENSE.
#![allow(non_snake_case)]
use super::{
    constants,
    field::FieldElement,
    models::{CompletedPoint, ProjectiveNielsPoint, ProjectivePoint},
    montgomery::MontgomeryPoint,
    scalar::Scalar,
    traits::Identity,
};
use core::ops::Neg;
use subtle::ConstantTimeEq;
#[cfg(test)]
use subtle::{Choice, ConditionallyNegatable};
#[derive(Clone, Copy, Debug)]
pub(crate) struct EdwardsPoint {
    pub(super) X: FieldElement,
    pub(super) Y: FieldElement,
    pub(super) Z: FieldElement,
    pub(super) T: FieldElement,
}
#[derive(Clone, Copy)]
pub(crate) struct CompressedEdwardsY(pub(crate) [u8; 32]);
impl CompressedEdwardsY {
    pub(crate) fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
    pub(crate) fn to_bytes(self) -> [u8; 32] {
        self.0
    }
    #[cfg(test)]
    pub(crate) fn decompress(&self) -> Option<EdwardsPoint> {
        let Y = FieldElement::from_bytes(&self.0);
        let yy = Y.square();
        let (valid, mut X) = FieldElement::sqrt_ratio(
            &(&yy - &FieldElement::ONE),
            &(&(&yy * &constants::EDWARDS_D) + &FieldElement::ONE),
        );
        if !bool::from(valid) {
            return None;
        }
        X.conditional_negate(Choice::from(self.0[31] >> 7));
        Some(EdwardsPoint {
            X,
            Y,
            Z: FieldElement::ONE,
            T: &X * &Y,
        })
    }
}
impl Identity for EdwardsPoint {
    fn identity() -> Self {
        Self {
            X: FieldElement::ZERO,
            Y: FieldElement::ONE,
            Z: FieldElement::ONE,
            T: FieldElement::ZERO,
        }
    }
}
impl EdwardsPoint {
    pub(super) fn as_projective_niels(&self) -> ProjectiveNielsPoint {
        ProjectiveNielsPoint {
            Y_plus_X: &self.Y + &self.X,
            Y_minus_X: &self.Y - &self.X,
            Z: self.Z,
            T2d: &self.T * &constants::EDWARDS_D2,
        }
    }
    pub(super) const fn as_projective(&self) -> ProjectivePoint {
        ProjectivePoint {
            X: self.X,
            Y: self.Y,
            Z: self.Z,
        }
    }
    pub(crate) fn to_montgomery(self) -> MontgomeryPoint {
        // We have u = (1+y)/(1-y) = (Z+Y)/(Z-Y).
        //
        // The denominator is zero only when y=1, the identity point of
        // the Edwards curve.  Since 0.invert() = 0, in this case we
        // compute the 2-torsion point (0,0).
        let U = &self.Z + &self.Y;
        let W = &self.Z - &self.Y;
        let u = &U * &W.invert();
        MontgomeryPoint(u.to_bytes())
    }
    pub(super) fn mul_by_pow_2(&self, k: u32) -> EdwardsPoint {
        debug_assert!(k > 0);
        let mut r: CompletedPoint;
        let mut s = self.as_projective();
        for _ in 0..(k - 1) {
            r = s.double();
            s = r.as_projective();
        }
        // Unroll last iteration so we can go directly as_extended()
        s.double().as_extended()
    }
    pub(crate) fn compress(&self) -> CompressedEdwardsY {
        let recip = self.Z.invert();
        let x = &self.X * &recip;
        let y = &self.Y * &recip;
        let mut bytes = y.to_bytes();
        bytes[31] |= x.is_negative().unwrap_u8() << 7;
        CompressedEdwardsY(bytes)
    }
    pub(crate) fn is_small_order(&self) -> bool {
        let p = self.mul_by_pow_2(3);
        bool::from(p.X.is_zero() & p.Y.ct_eq(&p.Z))
    }
    pub(crate) fn mul_base(scalar: &Scalar) -> Self {
        let digits = scalar.as_radix_16();
        let table = &constants::BASEPOINT_TABLE;
        let mut point = Self::identity();
        for i in (1..64).step_by(2) {
            point = (&point + &table[i / 2].select(digits[i])).as_extended();
        }
        point = point.mul_by_pow_2(4);
        for i in (0..64).step_by(2) {
            point = (&point + &table[i / 2].select(digits[i])).as_extended();
        }
        point
    }
    pub(crate) fn vartime_double_scalar_mul_basepoint(a: &Scalar, A: &Self, b: &Scalar) -> Self {
        super::vartime::mul(a, A, b)
    }
}
impl Neg for &EdwardsPoint {
    type Output = EdwardsPoint;

    fn neg(self) -> EdwardsPoint {
        EdwardsPoint {
            X: -(&self.X),
            Y: self.Y,
            Z: self.Z,
            T: -(&self.T),
        }
    }
}

impl Neg for EdwardsPoint {
    type Output = EdwardsPoint;

    fn neg(self) -> EdwardsPoint {
        -&self
    }
}

// Portions derived from curve25519-dalek 5.0.0. See LICENSE.
#![allow(non_snake_case)]

use core::ops::{Add, Neg, Sub};

use subtle::Choice;
use subtle::ConditionallySelectable;

use super::edwards::EdwardsPoint;
use super::field::FieldElement;

// ------------------------------------------------------------------------
// Internal point representations
// ------------------------------------------------------------------------

#[allow(missing_docs)]
#[derive(Copy, Clone)]
pub(super) struct ProjectivePoint {
    pub X: FieldElement,
    pub Y: FieldElement,
    pub Z: FieldElement,
}

#[derive(Copy, Clone)]
#[allow(missing_docs)]
pub(super) struct CompletedPoint {
    pub X: FieldElement,
    pub Y: FieldElement,
    pub Z: FieldElement,
    pub T: FieldElement,
}

// Safe to derive Eq because affine coordinates.
#[derive(Copy, Clone)]
#[allow(missing_docs)]
pub(super) struct AffineNielsPoint {
    pub y_plus_x: FieldElement,
    pub y_minus_x: FieldElement,
    pub xy2d: FieldElement,
}

#[derive(Copy, Clone)]
#[allow(missing_docs)]
pub(super) struct ProjectiveNielsPoint {
    pub Y_plus_X: FieldElement,
    pub Y_minus_X: FieldElement,
    pub Z: FieldElement,
    pub T2d: FieldElement,
}

// ------------------------------------------------------------------------
// Constructors
// ------------------------------------------------------------------------

use super::traits::Identity;

impl Identity for ProjectivePoint {
    fn identity() -> ProjectivePoint {
        ProjectivePoint {
            X: FieldElement::ZERO,
            Y: FieldElement::ONE,
            Z: FieldElement::ONE,
        }
    }
}

impl Identity for AffineNielsPoint {
    fn identity() -> AffineNielsPoint {
        AffineNielsPoint {
            y_plus_x: FieldElement::ONE,
            y_minus_x: FieldElement::ONE,
            xy2d: FieldElement::ZERO,
        }
    }
}

// ------------------------------------------------------------------------
// Validity checks (for debugging, not CT)
// ------------------------------------------------------------------------

impl ConditionallySelectable for AffineNielsPoint {
    fn conditional_select(a: &Self, b: &Self, choice: Choice) -> Self {
        AffineNielsPoint {
            y_plus_x: FieldElement::conditional_select(&a.y_plus_x, &b.y_plus_x, choice),
            y_minus_x: FieldElement::conditional_select(&a.y_minus_x, &b.y_minus_x, choice),
            xy2d: FieldElement::conditional_select(&a.xy2d, &b.xy2d, choice),
        }
    }

    fn conditional_assign(&mut self, other: &Self, choice: Choice) {
        self.y_plus_x.conditional_assign(&other.y_plus_x, choice);
        self.y_minus_x.conditional_assign(&other.y_minus_x, choice);
        self.xy2d.conditional_assign(&other.xy2d, choice);
    }
}

// ------------------------------------------------------------------------
// Point conversions
// ------------------------------------------------------------------------

impl ProjectivePoint {
    pub(super) fn as_extended(&self) -> EdwardsPoint {
        EdwardsPoint {
            X: &self.X * &self.Z,
            Y: &self.Y * &self.Z,
            Z: self.Z.square(),
            T: &self.X * &self.Y,
        }
    }
}

impl CompletedPoint {
    pub(super) fn as_projective(&self) -> ProjectivePoint {
        ProjectivePoint {
            X: &self.X * &self.T,
            Y: &self.Y * &self.Z,
            Z: &self.Z * &self.T,
        }
    }

    pub(super) fn as_extended(&self) -> EdwardsPoint {
        EdwardsPoint {
            X: &self.X * &self.T,
            Y: &self.Y * &self.Z,
            Z: &self.Z * &self.T,
            T: &self.X * &self.Y,
        }
    }
}

// ------------------------------------------------------------------------
// Doubling
// ------------------------------------------------------------------------

impl ProjectivePoint {
    pub(super) fn double(&self) -> CompletedPoint {
        // Double()
        let XX = self.X.square();
        let YY = self.Y.square();
        let ZZ2 = self.Z.square2();
        let X_plus_Y = &self.X + &self.Y;
        let X_plus_Y_sq = X_plus_Y.square();
        let YY_plus_XX = &YY + &XX;
        let YY_minus_XX = &YY - &XX;

        CompletedPoint {
            X: &X_plus_Y_sq - &YY_plus_XX,
            Y: YY_plus_XX,
            Z: YY_minus_XX,
            T: &ZZ2 - &YY_minus_XX,
        }
    }
}

// ------------------------------------------------------------------------
// Addition and Subtraction
// ------------------------------------------------------------------------

// XXX(hdevalence) These were doc(hidden) so they don't appear in the
// public API docs.
// However, that prevents them being used with --document-private-items,
// so comment out the doc(hidden) for now until this is resolved
//
// upstream rust issue: https://github.com/rust-lang/rust/issues/46380
//#[doc(hidden)]
impl<'a> Add<&'a ProjectiveNielsPoint> for &EdwardsPoint {
    type Output = CompletedPoint;

    fn add(self, other: &'a ProjectiveNielsPoint) -> CompletedPoint {
        let Y_plus_X = &self.Y + &self.X;
        let Y_minus_X = &self.Y - &self.X;
        let PP = &Y_plus_X * &other.Y_plus_X;
        let MM = &Y_minus_X * &other.Y_minus_X;
        let TT2d = &self.T * &other.T2d;
        let ZZ = &self.Z * &other.Z;
        let ZZ2 = &ZZ + &ZZ;

        CompletedPoint {
            X: &PP - &MM,
            Y: &PP + &MM,
            Z: &ZZ2 + &TT2d,
            T: &ZZ2 - &TT2d,
        }
    }
}

//#[doc(hidden)]
impl<'a> Sub<&'a ProjectiveNielsPoint> for &EdwardsPoint {
    type Output = CompletedPoint;

    fn sub(self, other: &'a ProjectiveNielsPoint) -> CompletedPoint {
        let Y_plus_X = &self.Y + &self.X;
        let Y_minus_X = &self.Y - &self.X;
        let PM = &Y_plus_X * &other.Y_minus_X;
        let MP = &Y_minus_X * &other.Y_plus_X;
        let TT2d = &self.T * &other.T2d;
        let ZZ = &self.Z * &other.Z;
        let ZZ2 = &ZZ + &ZZ;

        CompletedPoint {
            X: &PM - &MP,
            Y: &PM + &MP,
            Z: &ZZ2 - &TT2d,
            T: &ZZ2 + &TT2d,
        }
    }
}

//#[doc(hidden)]
impl<'a> Add<&'a AffineNielsPoint> for &EdwardsPoint {
    type Output = CompletedPoint;

    fn add(self, other: &'a AffineNielsPoint) -> CompletedPoint {
        let Y_plus_X = &self.Y + &self.X;
        let Y_minus_X = &self.Y - &self.X;
        let PP = &Y_plus_X * &other.y_plus_x;
        let MM = &Y_minus_X * &other.y_minus_x;
        let Txy2d = &self.T * &other.xy2d;
        let Z2 = &self.Z + &self.Z;

        CompletedPoint {
            X: &PP - &MM,
            Y: &PP + &MM,
            Z: &Z2 + &Txy2d,
            T: &Z2 - &Txy2d,
        }
    }
}

//#[doc(hidden)]
impl<'a> Sub<&'a AffineNielsPoint> for &EdwardsPoint {
    type Output = CompletedPoint;

    fn sub(self, other: &'a AffineNielsPoint) -> CompletedPoint {
        let Y_plus_X = &self.Y + &self.X;
        let Y_minus_X = &self.Y - &self.X;
        let PM = &Y_plus_X * &other.y_minus_x;
        let MP = &Y_minus_X * &other.y_plus_x;
        let Txy2d = &self.T * &other.xy2d;
        let Z2 = &self.Z + &self.Z;

        CompletedPoint {
            X: &PM - &MP,
            Y: &PM + &MP,
            Z: &Z2 - &Txy2d,
            T: &Z2 + &Txy2d,
        }
    }
}

// ------------------------------------------------------------------------
// Negation
// ------------------------------------------------------------------------

impl Neg for &AffineNielsPoint {
    type Output = AffineNielsPoint;

    fn neg(self) -> AffineNielsPoint {
        AffineNielsPoint {
            y_plus_x: self.y_minus_x,
            y_minus_x: self.y_plus_x,
            xy2d: -(&self.xy2d),
        }
    }
}

// ------------------------------------------------------------------------
// Debug traits
// ------------------------------------------------------------------------

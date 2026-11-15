// -*- mode: rust; -*-
//
// This file is part of curve25519-dalek.
// Copyright (c) 2016-2021 isis lovecruft
// Copyright (c) 2016-2019 Henry de Valence
// See LICENSE for licensing information.
//
// Authors:
// - isis agora lovecruft <isis@patternsinthevoid.net>
// - Henry de Valence <hdevalence@hdevalence.ca>

//! Parallel Edwards Arithmetic for Curve25519.
//!
//! This module currently has two point types:
//!
//! * `ExtendedPoint`: a point stored in vector-friendly format, with
//!   vectorized doubling and addition;
//!
//! * `CachedPoint`: used for readdition.
//!
//! Details on the formulas can be found in the documentation for the
//! parent `avx2` module.
//!
//! This API is designed to be safe: vectorized points can only be
//! created from serial points (which do validation on decompression),
//! and operations on valid points return valid points, so invalid
//! point states should be unrepresentable.
//!
//! This design goal is met, with one exception: the `Neg`
//! implementation for the `CachedPoint` performs a lazy negation, so
//! that subtraction can be efficiently implemented as a negation and
//! an addition.  Repeatedly negating a `CachedPoint` will cause its
//! coefficients to grow and eventually overflow.  Repeatedly negating
//! a point should not be necessary anyways.

#![allow(non_snake_case)]

use core::ops::{Add, Neg, Sub};

use super::super::edwards;
use super::super::window::NafLookupTable5;

use super::super::traits::Identity;

use super::constants;
use super::field::{FieldElement2625x4, Lanes, Shuffle};

/// A point on Curve25519, using parallel Edwards formulas for curve
/// operations.
///
/// # Invariant
///
/// The coefficients of an `ExtendedPoint` are bounded with
/// \\( b < 0.007 \\).
#[derive(Copy, Clone)]
pub(super) struct ExtendedPoint(pub(super) FieldElement2625x4);

impl From<edwards::EdwardsPoint> for ExtendedPoint {
    #[inline(always)]
    fn from(P: edwards::EdwardsPoint) -> ExtendedPoint {
        ExtendedPoint(FieldElement2625x4::new(&P.X, &P.Y, &P.Z, &P.T))
    }
}

impl From<ExtendedPoint> for edwards::EdwardsPoint {
    #[inline(always)]
    fn from(P: ExtendedPoint) -> edwards::EdwardsPoint {
        let tmp = P.0.split();
        edwards::EdwardsPoint {
            X: tmp[0],
            Y: tmp[1],
            Z: tmp[2],
            T: tmp[3],
        }
    }
}

impl Identity for ExtendedPoint {
    #[inline(always)]
    fn identity() -> ExtendedPoint {
        constants::EXTENDEDPOINT_IDENTITY
    }
}

impl ExtendedPoint {
    /// Compute the double of this point.
    #[inline(always)]
    pub(super) fn double(&self) -> ExtendedPoint {
        // Want to compute (X1 Y1 Z1 X1+Y1).
        // Not sure how to do this less expensively than computing
        // (X1 Y1 Z1 T1) --(256bit shuffle)--> (X1 Y1 X1 Y1)
        // (X1 Y1 X1 Y1) --(2x128b shuffle)--> (Y1 X1 Y1 X1)
        // and then adding.

        // Set tmp0 = (X1 Y1 X1 Y1)
        let mut tmp0 = self.0.shuffle(Shuffle::ABAB);

        // Set tmp1 = (Y1 X1 Y1 X1)
        let mut tmp1 = tmp0.shuffle(Shuffle::BADC);

        // Set tmp0 = (X1 Y1 Z1 X1+Y1)
        tmp0 = self.0.blend(tmp0 + tmp1, Lanes::D);

        // Set tmp1 = tmp0^2, negating the D values
        tmp1 = tmp0.square_and_negate_D();
        // Now tmp1 = (S1 S2 S3 -S4) with b < 0.007

        // See discussion of bounds in the module-level documentation.
        // We want to compute
        //
        //    + | S1 | S1 | S1 | S1 |
        //    + | S2 |    |    | S2 |
        //    + |    |    | S3 |    |
        //    + |    |    | S3 |    |
        //    + |    |    |    |-S4 |
        //    + |    | 2p | 2p |    |
        //    - |    | S2 | S2 |    |
        //    =======================
        //        S5   S6   S8   S9

        let zero = FieldElement2625x4::ZERO;
        let S_1 = tmp1.shuffle(Shuffle::AAAA);
        let S_2 = tmp1.shuffle(Shuffle::BBBB);

        tmp0 = zero.blend(tmp1 + tmp1, Lanes::C);
        // tmp0 = (0, 0,  2S_3, 0)
        tmp0 = tmp0.blend(tmp1, Lanes::D);
        // tmp0 = (0, 0,  2S_3, -S_4)
        tmp0 = tmp0 + S_1;
        // tmp0 = (  S_1,   S_1, S_1 + 2S_3, S_1 - S_4)
        tmp0 = tmp0 + zero.blend(S_2, Lanes::AD);
        // tmp0 = (S_1 + S_2,   S_1, S_1 + 2S_3, S_1 + S_2 - S_4)
        tmp0 = tmp0 + zero.blend(S_2.negate_lazy(), Lanes::BC);
        // tmp0 = (S_1 + S_2, S_1 - S_2, S_1 - S_2 + 2S_3, S_1 + S_2 - S_4)
        //    b < (     1.01,       1.6,             2.33,             1.6)
        // Now tmp0 = (S_5, S_6, S_8, S_9)

        // Set tmp1 = ( S_9,  S_6,  S_6,  S_9)
        //        b < ( 1.6,  1.6,  1.6,  1.6)
        tmp1 = tmp0.shuffle(Shuffle::DBBD);
        // Set tmp0 = ( S_8,  S_5,  S_8,  S_5)
        //        b < (2.33, 1.01, 2.33, 1.01)
        tmp0 = tmp0.shuffle(Shuffle::CACA);

        // Bounds on (tmp0, tmp1) are (2.33, 1.6) < (2.5, 1.75).
        ExtendedPoint(&tmp0 * &tmp1)
    }
}

/// A cached point with some precomputed variables used for readdition.
///
/// # Warning
///
/// It is not safe to negate this point more than once.
///
/// # Invariant
///
/// As long as the `CachedPoint` is not repeatedly negated, its
/// coefficients will be bounded with \\( b < 1.0 \\).
#[derive(Copy, Clone)]
pub(super) struct CachedPoint(pub(super) FieldElement2625x4);

impl From<ExtendedPoint> for CachedPoint {
    #[inline(always)]
    fn from(P: ExtendedPoint) -> CachedPoint {
        let mut x = P.0;

        x = x.blend(x.diff_sum(), Lanes::AB);
        // x = (Y2 - X2, Y2 + X2, Z2, T2) = (S2 S3 Z2 T2)

        x = x * (121666, 121666, 2 * 121666, 2 * 121665);
        // x = (121666*S2 121666*S3 2*121666*Z2 2*121665*T2)

        x = x.blend(-x, Lanes::D);
        // x = (121666*S2 121666*S3 2*121666*Z2 -2*121665*T2)

        // The coefficients of the output are bounded with b < 0.007.
        CachedPoint(x)
    }
}

impl Neg for &CachedPoint {
    type Output = CachedPoint;
    /// Lazily negate the point.
    ///
    /// # Warning
    ///
    /// Because this method does not perform a reduction, it is not
    /// safe to repeatedly negate a point.
    #[inline(always)]
    fn neg(self) -> CachedPoint {
        let swapped = self.0.shuffle(Shuffle::BACD);
        CachedPoint(swapped.blend(swapped.negate_lazy(), Lanes::D))
    }
}

impl Add<&CachedPoint> for &ExtendedPoint {
    type Output = ExtendedPoint;

    /// Add an `ExtendedPoint` and a `CachedPoint`.
    #[inline(always)]
    fn add(self, other: &CachedPoint) -> ExtendedPoint {
        // The coefficients of an `ExtendedPoint` are reduced after
        // every operation.  If the `CachedPoint` was negated, its
        // coefficients grow by one bit.  So on input, `self` is
        // bounded with `b < 0.007` and `other` is bounded with
        // `b < 1.0`.

        let mut tmp = self.0;

        tmp = tmp.blend(tmp.diff_sum(), Lanes::AB);
        // tmp = (Y1-X1 Y1+X1 Z1 T1) = (S0 S1 Z1 T1) with b < 1.6

        // (tmp, other) bounded with b < (1.6, 1.0) < (2.5, 1.75).
        tmp = &tmp * &other.0;
        // tmp = (S0*S2' S1*S3' Z1*Z2' T1*T2') = (S8 S9 S10 S11)

        tmp = tmp.shuffle(Shuffle::ABDC);
        // tmp = (S8 S9 S11 S10)

        tmp = tmp.diff_sum();
        // tmp = (S9-S8 S9+S8 S10-S11 S10+S11) = (S12 S13 S14 S15)

        let t0 = tmp.shuffle(Shuffle::ADDA);
        // t0 = (S12 S15 S15 S12)
        let t1 = tmp.shuffle(Shuffle::CBCB);
        // t1 = (S14 S13 S14 S13)

        // All coefficients of t0, t1 are bounded with b < 1.6.
        // Return (S12*S14 S15*S13 S15*S14 S12*S13) = (X3 Y3 Z3 T3)
        ExtendedPoint(&t0 * &t1)
    }
}

impl Sub<&CachedPoint> for &ExtendedPoint {
    type Output = ExtendedPoint;

    /// Implement subtraction by negating the point and adding.
    ///
    /// Empirically, this seems about the same cost as a custom
    /// subtraction impl (maybe because the benefit is cancelled by
    /// increased code size?)
    #[inline(always)]
    fn sub(self, other: &CachedPoint) -> ExtendedPoint {
        self + &(-other)
    }
}

impl From<&edwards::EdwardsPoint> for NafLookupTable5<CachedPoint> {
    #[inline(always)]
    fn from(point: &edwards::EdwardsPoint) -> Self {
        let A = ExtendedPoint::from(*point);
        let mut Ai = [CachedPoint::from(A); 8];
        let A2 = A.double();
        for i in 0..7 {
            Ai[i + 1] = (&A2 + &Ai[i]).into();
        }
        // Now Ai = [A, 3A, 5A, 7A, 9A, 11A, 13A, 15A]
        NafLookupTable5(Ai)
    }
}

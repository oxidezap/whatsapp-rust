// Portions derived from curve25519-dalek 5.0.0. See LICENSE.
use super::{
    edwards::EdwardsPoint,
    models::{AffineNielsPoint, ProjectiveNielsPoint},
    traits::Identity,
};
use subtle::{Choice, ConditionallyNegatable, ConditionallySelectable, ConstantTimeEq};
#[derive(Clone, Copy)]
pub(super) struct LookupTable<T>(pub(super) [T; 8]);
impl LookupTable<AffineNielsPoint> {
    pub(super) fn select(&self, x: i8) -> AffineNielsPoint {
        debug_assert!((-8..=8).contains(&x));
        let mask = (x as i16) >> 7;
        let abs = ((x as i16) + mask) ^ mask;
        let mut result = AffineNielsPoint::identity();
        for j in 1..=8 {
            result.conditional_assign(&self.0[j - 1], (abs as u16).ct_eq(&(j as u16)));
        }
        result.conditional_negate(Choice::from((mask & 1) as u8));
        result
    }
}
pub(super) struct NafLookupTable5<T>(pub(super) [T; 8]);
pub(super) struct NafLookupTable8<T>(pub(super) [T; 64]);
impl<T: Copy> NafLookupTable5<T> {
    pub(super) fn select(&self, x: usize) -> T {
        debug_assert!(x & 1 == 1 && x < 16);
        self.0[x / 2]
    }
}
impl<T: Copy> NafLookupTable8<T> {
    pub(super) fn select(&self, x: usize) -> T {
        debug_assert!(x & 1 == 1 && x < 128);
        self.0[x / 2]
    }
}
impl<'a> From<&'a EdwardsPoint> for NafLookupTable5<ProjectiveNielsPoint> {
    fn from(point: &'a EdwardsPoint) -> Self {
        let mut points = [point.as_projective_niels(); 8];
        let doubled = point.mul_by_pow_2(1);
        for i in 0..7 {
            points[i + 1] = (&doubled + &points[i]).as_extended().as_projective_niels();
        }
        // Now points = [point, 3A, 5A, 7A, 9A, 11A, 13A, 15A]
        NafLookupTable5(points)
    }
}

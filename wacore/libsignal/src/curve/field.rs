// Portions derived from curve25519-dalek 5.0.0. See LICENSE.
use super::constants;
#[cfg(target_pointer_width = "64")]
pub(super) use super::field51::FieldElement51 as FieldElement;
#[cfg(not(target_pointer_width = "64"))]
pub(super) use super::field2625::FieldElement2625 as FieldElement;
use subtle::{Choice, ConditionallyNegatable, ConditionallySelectable, ConstantTimeEq};
impl PartialEq for FieldElement {
    fn eq(&self, other: &Self) -> bool {
        self.ct_eq(other).into()
    }
}
impl ConstantTimeEq for FieldElement {
    fn ct_eq(&self, other: &Self) -> Choice {
        self.to_bytes().ct_eq(&other.to_bytes())
    }
}
impl FieldElement {
    pub(crate) fn is_negative(&self) -> Choice {
        let bytes = self.to_bytes();
        (bytes[0] & 1).into()
    }
    pub(crate) fn is_zero(&self) -> Choice {
        let zero = [0u8; 32];
        let bytes = self.to_bytes();

        bytes.ct_eq(&zero)
    }
    fn pow22501(&self) -> (FieldElement, FieldElement) {
        // Instead of managing which temporary variables are used
        // for what, we define as many as we need and leave stack
        // allocation to the compiler
        //
        // Each temporary variable t_i is of the form (self)^e_i.
        // Squaring t_i corresponds to multiplying e_i by 2,
        // so the pow2k function shifts e_i left by k places.
        // Multiplying t_i and t_j corresponds to adding e_i + e_j.
        //
        // Temporary t_i                      Nonzero bits of e_i
        //
        let t0 = self.square(); // 1         e_0 = 2^1
        let t1 = t0.square().square(); // 3         e_1 = 2^3
        let t2 = self * &t1; // 3,0       e_2 = 2^3 + 2^0
        let t3 = &t0 * &t2; // 3,1,0
        let t4 = t3.square(); // 4,2,1
        let t5 = &t2 * &t4; // 4,3,2,1,0
        let t6 = t5.pow2k(5); // 9,8,7,6,5
        let t7 = &t6 * &t5; // 9,8,7,6,5,4,3,2,1,0
        let t8 = t7.pow2k(10); // 19..10
        let t9 = &t8 * &t7; // 19..0
        let t10 = t9.pow2k(20); // 39..20
        let t11 = &t10 * &t9; // 39..0
        let t12 = t11.pow2k(10); // 49..10
        let t13 = &t12 * &t7; // 49..0
        let t14 = t13.pow2k(50); // 99..50
        let t15 = &t14 * &t13; // 99..0
        let t16 = t15.pow2k(100); // 199..100
        let t17 = &t16 * &t15; // 199..0
        let t18 = t17.pow2k(50); // 249..50
        let t19 = &t18 * &t13; // 249..0

        (t19, t3)
    }
    pub(crate) fn invert(&self) -> FieldElement {
        // The bits of p-2 = 2^255 -19 -2 are 11010111111...11.
        //
        //                                 nonzero bits of exponent
        let (t19, t3) = self.pow22501(); // t19: 249..0 ; t3: 3,1,0
        let t20 = t19.pow2k(5); // 254..5
        let t21 = &t20 * &t3; // 254..5,3,1,0

        t21
    }
    pub(crate) fn pow_p58(&self) -> FieldElement {
        // The bits of (p-5)/8 are 101111.....11.
        //
        //                                 nonzero bits of exponent
        let (t19, _) = self.pow22501(); // 249..0
        let t20 = t19.pow2k(2); // 251..2
        let t21 = self * &t20; // 251..2,0

        t21
    }
    pub(crate) fn sqrt_ratio(u: &FieldElement, v: &FieldElement) -> (Choice, FieldElement) {
        // r = (uv³)(uv⁷)^((p-5)/8). A square ratio gives v*r² = u or -u.
        // Callers discard r on failure, so no root of i*u/v is computed.
        let v3 = &v.square() * v;
        let v7 = &v3.square() * v;
        let mut r = &(u * &v3) * &(u * &v7).pow_p58();
        let check = v * &r.square();

        let correct_sign_sqrt = check.ct_eq(u);
        let flipped_sign_sqrt = check.ct_eq(&(-u));

        let r_prime = &constants::SQRT_M1 * &r;
        r.conditional_assign(&r_prime, flipped_sign_sqrt);

        // Choose the nonnegative square root.
        let r_is_negative = r.is_negative();
        r.conditional_negate(r_is_negative);

        let was_nonzero_square = correct_sign_sqrt | flipped_sign_sqrt;

        (was_nonzero_square, r)
    }
}

#[cfg(target_pointer_width = "64")]
pub(super) const fn from_radix51(limbs: [u64; 5]) -> FieldElement {
    FieldElement::from_limbs(limbs)
}
#[cfg(not(target_pointer_width = "64"))]
pub(super) const fn from_radix51(mut limbs: [u64; 5]) -> FieldElement {
    // Tables contain unreduced limbs. Carry before splitting into 26/25-bit limbs.
    let mask = (1u64 << 51) - 1;
    let mut round = 0;
    while round < 2 {
        let mut i = 0;
        while i < 4 {
            let carry = limbs[i] >> 51;
            limbs[i] &= mask;
            limbs[i + 1] += carry;
            i += 1;
        }
        let carry = limbs[4] >> 51;
        limbs[4] &= mask;
        limbs[0] += 19 * carry;
        round += 1;
    }
    let mut bytes = [0u8; 32];
    let mut bit = 0;
    while bit < 255 {
        bytes[bit / 8] |= (((limbs[bit / 51] >> (bit % 51)) & 1) as u8) << (bit % 8);
        bit += 1;
    }
    FieldElement::from_bytes(&bytes)
}

#[cfg(not(target_pointer_width = "64"))]
impl FieldElement {
    pub(super) fn mul121666(&self) -> Self {
        const FACTOR: FieldElement = from_radix51([121666, 0, 0, 0, 0]);
        self * &FACTOR
    }
}

//! Field arithmetic modulo \\(p = 2\^{255} - 19\\) on four saturated 64-bit
//! limbs, for the x86_64 BMI2 Montgomery ladder only.
//!
//! An element is any integer below \\(2\^{256}\\) in its residue class: the
//! operations fold carries out of bit 256 with \\(2\^{256} \equiv 38\\) and
//! never canonicalize. Compared with the radix-\\(2\^{51}\\) field, a product
//! needs 16 limb multiplications instead of 25 and holds four limbs per
//! operand instead of five, so the ladder step fits in far fewer registers.
//! All functions are branch-free and `#[inline(always)]`, so they compile
//! for the BMI2 caller's target features.

use super::field::FieldElement;
use subtle::{Choice, ConditionallySelectable};

#[derive(Clone, Copy)]
pub(super) struct FieldElement64(pub(super) [u64; 4]);

/// `a + b + carry`, returning the low word and the carry out. Written like
/// `u64::carrying_add` (unstable) so that LLVM emits an `adc` chain.
#[inline(always)]
fn adc(a: u64, b: u64, carry: bool) -> (u64, bool) {
    let (s, c1) = a.overflowing_add(b);
    let (s, c2) = s.overflowing_add(carry as u64);
    (s, c1 | c2)
}

/// `a - b - borrow`, returning the low word and the borrow out.
#[inline(always)]
fn sbb(a: u64, b: u64, borrow: bool) -> (u64, bool) {
    let (d, b1) = a.overflowing_sub(b);
    let (d, b2) = d.overflowing_sub(borrow as u64);
    (d, b1 | b2)
}

impl FieldElement64 {
    pub(super) const ZERO: Self = Self([0, 0, 0, 0]);
    pub(super) const ONE: Self = Self([1, 0, 0, 0]);

    /// Decodes an X25519 u-coordinate, ignoring bit 255 as RFC 7748 requires.
    #[inline(always)]
    pub(super) fn from_bytes(bytes: &[u8; 32]) -> Self {
        let mut limbs: [u64; 4] = core::array::from_fn(|i| {
            u64::from_le_bytes(core::array::from_fn(|k| bytes[8 * i + k]))
        });
        limbs[3] &= (1 << 63) - 1;
        Self(limbs)
    }

    /// Splits into radix-2^51 limbs. The top limb takes bits 204..256, so it
    /// stays below 2^52, inside the 2^54 input bound of the 51-bit field.
    #[inline(always)]
    pub(super) fn to_field51(self) -> FieldElement {
        const MASK: u64 = (1 << 51) - 1;
        let [l0, l1, l2, l3] = self.0;
        FieldElement::from_limbs([
            l0 & MASK,
            ((l0 >> 51) | (l1 << 13)) & MASK,
            ((l1 >> 38) | (l2 << 26)) & MASK,
            ((l2 >> 25) | (l3 << 39)) & MASK,
            l3 >> 12,
        ])
    }

    /// Adds `38 * carry` to a 256-bit value, where `carry` counts the
    /// multiples of 2^256 that were dropped. `carry * 38` must fit in a u64.
    #[inline(always)]
    fn fold_carry(limbs: [u64; 4], carry: u64) -> Self {
        let (r0, c) = adc(limbs[0], carry * 38, false);
        let (r1, c) = adc(limbs[1], 0, c);
        let (r2, c) = adc(limbs[2], 0, c);
        let (r3, c) = adc(limbs[3], 0, c);
        // A carry out here means the sum wrapped to a value below
        // `carry * 38`, so adding 38 to the low limb cannot carry again.
        Self([r0 + (c as u64) * 38, r1, r2, r3])
    }

    #[inline(always)]
    pub(super) fn add(&self, rhs: &Self) -> Self {
        let (s0, c) = adc(self.0[0], rhs.0[0], false);
        let (s1, c) = adc(self.0[1], rhs.0[1], c);
        let (s2, c) = adc(self.0[2], rhs.0[2], c);
        let (s3, c) = adc(self.0[3], rhs.0[3], c);
        Self::fold_carry([s0, s1, s2, s3], c as u64)
    }

    #[inline(always)]
    pub(super) fn sub(&self, rhs: &Self) -> Self {
        let (d0, b) = sbb(self.0[0], rhs.0[0], false);
        let (d1, b) = sbb(self.0[1], rhs.0[1], b);
        let (d2, b) = sbb(self.0[2], rhs.0[2], b);
        let (d3, b) = sbb(self.0[3], rhs.0[3], b);
        // A borrow added 2^256 = 38 (mod p); take the 38 back.
        let (d0, b) = sbb(d0, (b as u64) * 38, false);
        let (d1, b) = sbb(d1, 0, b);
        let (d2, b) = sbb(d2, 0, b);
        let (d3, b) = sbb(d3, 0, b);
        // A second borrow means the value was below 38 and wrapped to at
        // least 2^256 - 38, so the low limb is at least 2^64 - 38.
        Self([d0 - (b as u64) * 38, d1, d2, d3])
    }

    /// Reduces a 512-bit product with 2^256 = 38 (mod p).
    #[inline(always)]
    fn reduce_wide(r: [u64; 8]) -> Self {
        let mut out = [0u64; 4];
        let mut carry = 0u64;
        for i in 0..4 {
            // 38 * (2^64 - 1) + 2 * (2^64 - 1) < 2^128, and carry < 39.
            let t = (r[i + 4] as u128) * 38 + (r[i] as u128) + (carry as u128);
            out[i] = t as u64;
            carry = (t >> 64) as u64;
        }
        Self::fold_carry(out, carry)
    }

    #[inline(always)]
    pub(super) fn mul(&self, rhs: &Self) -> Self {
        let a = &self.0;
        let b = &rhs.0;
        let mut r = [0u64; 8];
        for i in 0..4 {
            let mut carry = 0u64;
            for j in 0..4 {
                // (2^64 - 1)^2 + 2 * (2^64 - 1) = 2^128 - 1: no overflow.
                let t = (a[i] as u128) * (b[j] as u128) + (r[i + j] as u128) + (carry as u128);
                r[i + j] = t as u64;
                carry = (t >> 64) as u64;
            }
            r[i + 4] = carry;
        }
        Self::reduce_wide(r)
    }

    /// Computes the six cross products `a[i] * a[j]` (`i < j`) once, doubles
    /// them and adds the four squares: 10 limb products instead of 16.
    #[inline(always)]
    pub(super) fn square(&self) -> Self {
        #[inline(always)]
        fn m(x: u64, y: u64) -> u128 {
            (x as u128) * (y as u128)
        }
        let a = &self.0;

        // Cross products. Each `t` stays below 2^128, as in `mul`.
        let t = m(a[0], a[1]);
        let r1 = t as u64;
        let t = m(a[0], a[2]) + (t >> 64);
        let r2 = t as u64;
        let t = m(a[0], a[3]) + (t >> 64);
        let r3 = t as u64;
        let r4 = (t >> 64) as u64;

        let t = m(a[1], a[2]) + (r3 as u128);
        let r3 = t as u64;
        let t = m(a[1], a[3]) + (r4 as u128) + (t >> 64);
        let r4 = t as u64;
        let r5 = (t >> 64) as u64;

        let t = m(a[2], a[3]) + (r5 as u128);
        let r5 = t as u64;
        let r6 = (t >> 64) as u64;

        // Double. The cross products sum to less than a^2 / 2 < 2^511, so no
        // bit is lost.
        let r7 = r6 >> 63;
        let r6 = (r6 << 1) | (r5 >> 63);
        let r5 = (r5 << 1) | (r4 >> 63);
        let r4 = (r4 << 1) | (r3 >> 63);
        let r3 = (r3 << 1) | (r2 >> 63);
        let r2 = (r2 << 1) | (r1 >> 63);
        let r1 = r1 << 1;

        // Add the squares. The total is a^2 < 2^512, so the chain ends
        // without a carry.
        let s0 = m(a[0], a[0]);
        let s1 = m(a[1], a[1]);
        let s2 = m(a[2], a[2]);
        let s3 = m(a[3], a[3]);
        let (r1, c) = adc(r1, (s0 >> 64) as u64, false);
        let (r2, c) = adc(r2, s1 as u64, c);
        let (r3, c) = adc(r3, (s1 >> 64) as u64, c);
        let (r4, c) = adc(r4, s2 as u64, c);
        let (r5, c) = adc(r5, (s2 >> 64) as u64, c);
        let (r6, c) = adc(r6, s3 as u64, c);
        let r7 = r7 + (s3 >> 64) as u64 + c as u64;

        Self::reduce_wide([s0 as u64, r1, r2, r3, r4, r5, r6, r7])
    }

    /// Computes `self * k + addend` for a constant `k` below 2^32.
    #[inline(always)]
    pub(super) fn mul_small_add(&self, k: u32, addend: &Self) -> Self {
        let mut out = [0u64; 4];
        let mut carry = 0u64;
        for i in 0..4 {
            // (2^64 - 1) * (2^32 - 1) + 2 * (2^64 - 1) < 2^128.
            let t = (self.0[i] as u128) * (k as u128) + (addend.0[i] as u128) + (carry as u128);
            out[i] = t as u64;
            carry = (t >> 64) as u64;
        }
        // carry <= 2^32, so carry * 38 fits in a u64.
        Self::fold_carry(out, carry)
    }

    #[inline(always)]
    pub(super) fn conditional_swap(a: &mut Self, b: &mut Self, choice: Choice) {
        for i in 0..4 {
            u64::conditional_swap(&mut a.0[i], &mut b.0[i], choice);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha512};

    fn to_fe51(x: &FieldElement64) -> FieldElement {
        x.to_field51()
    }

    fn encoded(x: &FieldElement64) -> [u8; 32] {
        to_fe51(x).to_bytes()
    }

    /// Edge values (0, 1, p - 1, p, 2^255 - 1, 2^256 - 1, ...) and
    /// pseudo-random ones, all as unreduced 256-bit integers.
    fn samples() -> Vec<FieldElement64> {
        const M: u64 = u64::MAX;
        let mut out = vec![
            FieldElement64([0, 0, 0, 0]),
            FieldElement64([1, 0, 0, 0]),
            FieldElement64([37, 0, 0, 0]),
            FieldElement64([38, 0, 0, 0]),
            FieldElement64([M - 19, M, M, M >> 1]),
            FieldElement64([M - 18, M, M, M >> 1]),
            FieldElement64([M, M, M, M >> 1]),
            FieldElement64([M - 37, M, M, M]),
            FieldElement64([M, M, M, M]),
            FieldElement64([0, 0, 0, 1 << 63]),
            FieldElement64([M, 0, M, 0]),
        ];
        for seed in 0u32..64 {
            let wide: [u8; 64] = Sha512::digest(seed.to_le_bytes()).into();
            let (chunks, _) = wide.as_chunks::<8>();
            out.push(FieldElement64(core::array::from_fn(|i| {
                u64::from_le_bytes(chunks[i])
            })));
        }
        out
    }

    #[test]
    fn operations_match_radix_51_field() {
        let samples = samples();
        for x in &samples {
            let fx = to_fe51(x);
            assert_eq!(encoded(&x.square()), fx.square().to_bytes());

            for y in &samples {
                let fy = to_fe51(y);
                assert_eq!(encoded(&x.mul(y)), (&fx * &fy).to_bytes());
                assert_eq!(encoded(&x.add(y)), (&fx + &fy).to_bytes());
                assert_eq!(encoded(&x.sub(y)), (&fx - &fy).to_bytes());
                assert_eq!(
                    encoded(&x.mul_small_add(121666, y)),
                    (&fx.mul121666() + &fy).to_bytes()
                );
            }
        }
    }

    #[test]
    fn from_bytes_ignores_bit_255() {
        let mut bytes = [0xffu8; 32];
        let masked = FieldElement64::from_bytes(&bytes);
        bytes[31] &= 0x7f;
        assert_eq!(masked.0, FieldElement64::from_bytes(&bytes).0);
        assert_eq!(
            encoded(&masked),
            FieldElement::from_bytes(&bytes).to_bytes()
        );
    }
}

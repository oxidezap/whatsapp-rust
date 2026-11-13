// Portions derived from curve25519-dalek 5.0.0. See LICENSE.

use core::ops::{Index, IndexMut};
use subtle::{Choice, ConditionallySelectable};

use super::constants;

#[derive(Copy, Clone)]
pub(super) struct Scalar52(pub(super) [u64; 5]);

impl Index<usize> for Scalar52 {
    type Output = u64;
    fn index(&self, _index: usize) -> &u64 {
        &(self.0[_index])
    }
}

impl IndexMut<usize> for Scalar52 {
    fn index_mut(&mut self, _index: usize) -> &mut u64 {
        &mut (self.0[_index])
    }
}

#[inline(always)]
fn m(x: u64, y: u64) -> u128 {
    (x as u128) * (y as u128)
}

impl Scalar52 {
    pub(super) const ZERO: Scalar52 = Scalar52([0, 0, 0, 0, 0]);

    #[rustfmt::skip] // keep alignment of s[*] calculations
    pub(super) fn from_bytes(bytes: &[u8; 32]) -> Scalar52 {
        let mut words = [0u64; 4];
        for i in 0..4 {
            for j in 0..8 {
                words[i] |= (bytes[(i * 8) + j] as u64) << (j * 8);
            }
        }

        let mask = (1u64 << 52) - 1;
        let top_mask = (1u64 << 48) - 1;
        let mut s = Scalar52::ZERO;

        s[0] =   words[0]                            & mask;
        s[1] = ((words[0] >> 52) | (words[1] << 12)) & mask;
        s[2] = ((words[1] >> 40) | (words[2] << 24)) & mask;
        s[3] = ((words[2] >> 28) | (words[3] << 36)) & mask;
        s[4] =  (words[3] >> 16)                     & top_mask;

        s
    }

    #[rustfmt::skip] // keep alignment of lo[*] and hi[*] calculations
    pub(super) fn from_bytes_wide(bytes: &[u8; 64]) -> Scalar52 {
        let mut words = [0u64; 8];
        for i in 0..8 {
            for j in 0..8 {
                words[i] |= (bytes[(i * 8) + j] as u64) << (j * 8);
            }
        }

        let mask = (1u64 << 52) - 1;
        let mut lo = Scalar52::ZERO;
        let mut hi = Scalar52::ZERO;

        lo[0] =   words[0]                             & mask;
        lo[1] = ((words[0] >> 52) | (words[ 1] << 12)) & mask;
        lo[2] = ((words[1] >> 40) | (words[ 2] << 24)) & mask;
        lo[3] = ((words[2] >> 28) | (words[ 3] << 36)) & mask;
        lo[4] = ((words[3] >> 16) | (words[ 4] << 48)) & mask;
        hi[0] =  (words[4] >>  4)                      & mask;
        hi[1] = ((words[4] >> 56) | (words[ 5] <<  8)) & mask;
        hi[2] = ((words[5] >> 44) | (words[ 6] << 20)) & mask;
        hi[3] = ((words[6] >> 32) | (words[ 7] << 32)) & mask;
        hi[4] =   words[7] >> 20                             ;

        lo = Scalar52::montgomery_mul(&lo, &constants::R);  // (lo * R) / R = lo
        hi = Scalar52::montgomery_mul(&hi, &constants::RR); // (hi * R^2) / R = hi * R

        Scalar52::add(&hi, &lo)
    }

    #[rustfmt::skip] // keep alignment of s[*] calculations
    #[allow(clippy::identity_op)]
    pub(super) fn to_bytes(self) -> [u8; 32] {
        let mut s = [0u8; 32];

        s[ 0] =  (self.0[ 0] >>  0)                      as u8;
        s[ 1] =  (self.0[ 0] >>  8)                      as u8;
        s[ 2] =  (self.0[ 0] >> 16)                      as u8;
        s[ 3] =  (self.0[ 0] >> 24)                      as u8;
        s[ 4] =  (self.0[ 0] >> 32)                      as u8;
        s[ 5] =  (self.0[ 0] >> 40)                      as u8;
        s[ 6] = ((self.0[ 0] >> 48) | (self.0[ 1] << 4)) as u8;
        s[ 7] =  (self.0[ 1] >>  4)                      as u8;
        s[ 8] =  (self.0[ 1] >> 12)                      as u8;
        s[ 9] =  (self.0[ 1] >> 20)                      as u8;
        s[10] =  (self.0[ 1] >> 28)                      as u8;
        s[11] =  (self.0[ 1] >> 36)                      as u8;
        s[12] =  (self.0[ 1] >> 44)                      as u8;
        s[13] =  (self.0[ 2] >>  0)                      as u8;
        s[14] =  (self.0[ 2] >>  8)                      as u8;
        s[15] =  (self.0[ 2] >> 16)                      as u8;
        s[16] =  (self.0[ 2] >> 24)                      as u8;
        s[17] =  (self.0[ 2] >> 32)                      as u8;
        s[18] =  (self.0[ 2] >> 40)                      as u8;
        s[19] = ((self.0[ 2] >> 48) | (self.0[ 3] << 4)) as u8;
        s[20] =  (self.0[ 3] >>  4)                      as u8;
        s[21] =  (self.0[ 3] >> 12)                      as u8;
        s[22] =  (self.0[ 3] >> 20)                      as u8;
        s[23] =  (self.0[ 3] >> 28)                      as u8;
        s[24] =  (self.0[ 3] >> 36)                      as u8;
        s[25] =  (self.0[ 3] >> 44)                      as u8;
        s[26] =  (self.0[ 4] >>  0)                      as u8;
        s[27] =  (self.0[ 4] >>  8)                      as u8;
        s[28] =  (self.0[ 4] >> 16)                      as u8;
        s[29] =  (self.0[ 4] >> 24)                      as u8;
        s[30] =  (self.0[ 4] >> 32)                      as u8;
        s[31] =  (self.0[ 4] >> 40)                      as u8;

        s
    }

    pub(super) fn add(a: &Scalar52, b: &Scalar52) -> Scalar52 {
        let mut sum = Scalar52::ZERO;
        let mask = (1u64 << 52) - 1;

        // a + b
        let mut carry: u64 = 0;
        for i in 0..5 {
            carry = a[i] + b[i] + (carry >> 52);
            sum[i] = carry & mask;
        }

        // subtract l if the sum is >= l
        Scalar52::sub(&sum, &constants::L)
    }

    pub(super) fn conditional_add_l(&mut self, condition: Choice) -> u64 {
        let mut carry: u64 = 0;
        let mask = (1u64 << 52) - 1;

        for i in 0..5 {
            let addend = u64::conditional_select(&0, &constants::L[i], condition);
            carry = (carry >> 52) + self[i] + addend;
            self[i] = carry & mask;
        }

        carry
    }
    pub(super) fn sub(a: &Scalar52, b: &Scalar52) -> Scalar52 {
        let mut difference = Scalar52::ZERO;
        let mask = (1u64 << 52) - 1;

        // a - b
        let mut borrow: u64 = 0;
        for i in 0..5 {
            borrow = a[i].wrapping_sub(b[i] + (borrow >> 63));
            difference[i] = borrow & mask;
        }

        // conditionally add l if the difference is negative
        difference.conditional_add_l(Choice::from((borrow >> 63) as u8));
        difference
    }

    #[rustfmt::skip] // keep alignment of z[*] calculations
    pub(super) fn mul_internal(a: &Scalar52, b: &Scalar52) -> [u128; 9] {
        let mut z = [0u128; 9];

        z[0] = m(a[0], b[0]);
        z[1] = m(a[0], b[1]) + m(a[1], b[0]);
        z[2] = m(a[0], b[2]) + m(a[1], b[1]) + m(a[2], b[0]);
        z[3] = m(a[0], b[3]) + m(a[1], b[2]) + m(a[2], b[1]) + m(a[3], b[0]);
        z[4] = m(a[0], b[4]) + m(a[1], b[3]) + m(a[2], b[2]) + m(a[3], b[1]) + m(a[4], b[0]);
        z[5] =                 m(a[1], b[4]) + m(a[2], b[3]) + m(a[3], b[2]) + m(a[4], b[1]);
        z[6] =                                 m(a[2], b[4]) + m(a[3], b[3]) + m(a[4], b[2]);
        z[7] =                                                 m(a[3], b[4]) + m(a[4], b[3]);
        z[8] =                                                                 m(a[4], b[4]);

        z
    }

    #[rustfmt::skip] // keep alignment of n* and r* calculations
    pub(super) fn montgomery_reduce(limbs: &[u128; 9]) -> Scalar52 {

        #[inline(always)]
        fn part1(sum: u128) -> (u128, u64) {
            let p = (sum as u64).wrapping_mul(constants::LFACTOR) & ((1u64 << 52) - 1);
            ((sum + m(p, constants::L[0])) >> 52, p)
        }

        #[inline(always)]
        fn part2(sum: u128) -> (u128, u64) {
            let w = (sum as u64) & ((1u64 << 52) - 1);
            (sum >> 52, w)
        }

        // note: l[3] is zero, so its multiples can be skipped
        let l = &constants::L;

        // the first half computes the Montgomery adjustment factor n, and begins adding n*l to make limbs divisible by R
        let (carry, n0) = part1(        limbs[0]);
        let (carry, n1) = part1(carry + limbs[1] + m(n0, l[1]));
        let (carry, n2) = part1(carry + limbs[2] + m(n0, l[2]) + m(n1, l[1]));
        let (carry, n3) = part1(carry + limbs[3]               + m(n1, l[2]) + m(n2, l[1]));
        let (carry, n4) = part1(carry + limbs[4] + m(n0, l[4])               + m(n2, l[2]) + m(n3, l[1]));

        // limbs is divisible by R now, so we can divide by R by simply storing the upper half as the result
        let (carry, r0) = part2(carry + limbs[5]               + m(n1, l[4])               + m(n3, l[2])   + m(n4, l[1]));
        let (carry, r1) = part2(carry + limbs[6]                             + m(n2,l[4])                  + m(n4, l[2]));
        let (carry, r2) = part2(carry + limbs[7]                                           + m(n3, l[4])                );
        let (carry, r3) = part2(carry + limbs[8]                                                           + m(n4, l[4]));
        let         r4 = carry as u64;

        // result may be >= l, so attempt to subtract l
        Scalar52::sub(&Scalar52([r0, r1, r2, r3, r4]), l)
    }

    pub(super) fn mul(a: &Scalar52, b: &Scalar52) -> Scalar52 {
        let ab = Scalar52::montgomery_reduce(&Scalar52::mul_internal(a, b));
        Scalar52::montgomery_reduce(&Scalar52::mul_internal(&ab, &constants::RR))
    }

    pub(super) fn montgomery_mul(a: &Scalar52, b: &Scalar52) -> Scalar52 {
        Scalar52::montgomery_reduce(&Scalar52::mul_internal(a, b))
    }
}

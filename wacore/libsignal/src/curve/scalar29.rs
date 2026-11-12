// Portions derived from curve25519-dalek 5.0.0. See LICENSE.

use core::ops::{Index, IndexMut};
use subtle::{Choice, ConditionallySelectable};

use super::constants;

#[derive(Copy, Clone)]
pub(super) struct Scalar29(pub(super) [u32; 9]);

impl Index<usize> for Scalar29 {
    type Output = u32;
    fn index(&self, _index: usize) -> &u32 {
        &(self.0[_index])
    }
}

impl IndexMut<usize> for Scalar29 {
    fn index_mut(&mut self, _index: usize) -> &mut u32 {
        &mut (self.0[_index])
    }
}

#[inline(always)]
fn m(x: u32, y: u32) -> u64 {
    (x as u64) * (y as u64)
}

impl Scalar29 {
    pub(super) const ZERO: Scalar29 = Scalar29([0, 0, 0, 0, 0, 0, 0, 0, 0]);

    #[rustfmt::skip] // keep alignment of s[*] calculations
    pub(super) fn from_bytes(bytes: &[u8; 32]) -> Scalar29 {
        let mut words = [0u32; 8];
        for i in 0..8 {
            for j in 0..4 {
                words[i] |= (bytes[(i * 4) + j] as u32) << (j * 8);
            }
        }

        let mask = (1u32 << 29) - 1;
        let top_mask = (1u32 << 24) - 1;
        let mut s = Scalar29::ZERO;

        s[0] =   words[0]                            & mask;
        s[1] = ((words[0] >> 29) | (words[1] <<  3)) & mask;
        s[2] = ((words[1] >> 26) | (words[2] <<  6)) & mask;
        s[3] = ((words[2] >> 23) | (words[3] <<  9)) & mask;
        s[4] = ((words[3] >> 20) | (words[4] << 12)) & mask;
        s[5] = ((words[4] >> 17) | (words[5] << 15)) & mask;
        s[6] = ((words[5] >> 14) | (words[6] << 18)) & mask;
        s[7] = ((words[6] >> 11) | (words[7] << 21)) & mask;
        s[8] =  (words[7] >>  8)                     & top_mask;

        s
    }

    #[rustfmt::skip] // keep alignment of lo[*] calculations
    pub(super) fn from_bytes_wide(bytes: &[u8; 64]) -> Scalar29 {
        let mut words = [0u32; 16];
        for i in 0..16 {
            for j in 0..4 {
                words[i] |= (bytes[(i * 4) + j] as u32) << (j * 8);
            }
        }

        let mask = (1u32 << 29) - 1;
        let mut lo = Scalar29::ZERO;
        let mut hi = Scalar29::ZERO;

        lo[0] =   words[ 0]                             & mask;
        lo[1] = ((words[ 0] >> 29) | (words[ 1] <<  3)) & mask;
        lo[2] = ((words[ 1] >> 26) | (words[ 2] <<  6)) & mask;
        lo[3] = ((words[ 2] >> 23) | (words[ 3] <<  9)) & mask;
        lo[4] = ((words[ 3] >> 20) | (words[ 4] << 12)) & mask;
        lo[5] = ((words[ 4] >> 17) | (words[ 5] << 15)) & mask;
        lo[6] = ((words[ 5] >> 14) | (words[ 6] << 18)) & mask;
        lo[7] = ((words[ 6] >> 11) | (words[ 7] << 21)) & mask;
        lo[8] = ((words[ 7] >>  8) | (words[ 8] << 24)) & mask;
        hi[0] = ((words[ 8] >>  5) | (words[ 9] << 27)) & mask;
        hi[1] =  (words[ 9] >>  2)                      & mask;
        hi[2] = ((words[ 9] >> 31) | (words[10] <<  1)) & mask;
        hi[3] = ((words[10] >> 28) | (words[11] <<  4)) & mask;
        hi[4] = ((words[11] >> 25) | (words[12] <<  7)) & mask;
        hi[5] = ((words[12] >> 22) | (words[13] << 10)) & mask;
        hi[6] = ((words[13] >> 19) | (words[14] << 13)) & mask;
        hi[7] = ((words[14] >> 16) | (words[15] << 16)) & mask;
        hi[8] =   words[15] >> 13                             ;

        lo = Scalar29::montgomery_mul(&lo, &constants::R);  // (lo * R) / R = lo
        hi = Scalar29::montgomery_mul(&hi, &constants::RR); // (hi * R^2) / R = hi * R

        Scalar29::add(&hi, &lo) // (hi * R) + lo
    }

    #[rustfmt::skip] // keep alignment of s[*] calculations
    #[allow(clippy::identity_op)]
    pub(super) fn to_bytes(self) -> [u8; 32] {
        let mut s = [0u8; 32];

        s[ 0] =  (self.0[0] >>  0)                      as u8;
        s[ 1] =  (self.0[0] >>  8)                      as u8;
        s[ 2] =  (self.0[0] >> 16)                      as u8;
        s[ 3] = ((self.0[0] >> 24) | (self.0[1] << 5))  as u8;
        s[ 4] =  (self.0[1] >>  3)                      as u8;
        s[ 5] =  (self.0[1] >> 11)                      as u8;
        s[ 6] =  (self.0[1] >> 19)                      as u8;
        s[ 7] = ((self.0[1] >> 27) | (self.0[2] << 2))  as u8;
        s[ 8] =  (self.0[2] >>  6)                      as u8;
        s[ 9] =  (self.0[2] >> 14)                      as u8;
        s[10] = ((self.0[2] >> 22) | (self.0[3] << 7))  as u8;
        s[11] =  (self.0[3] >>  1)                      as u8;
        s[12] =  (self.0[3] >>  9)                      as u8;
        s[13] =  (self.0[3] >> 17)                      as u8;
        s[14] = ((self.0[3] >> 25) | (self.0[4] << 4))  as u8;
        s[15] =  (self.0[4] >>  4)                      as u8;
        s[16] =  (self.0[4] >> 12)                      as u8;
        s[17] =  (self.0[4] >> 20)                      as u8;
        s[18] = ((self.0[4] >> 28) | (self.0[5] << 1))  as u8;
        s[19] =  (self.0[5] >>  7)                      as u8;
        s[20] =  (self.0[5] >> 15)                      as u8;
        s[21] = ((self.0[5] >> 23) | (self.0[6] << 6))  as u8;
        s[22] =  (self.0[6] >>  2)                      as u8;
        s[23] =  (self.0[6] >> 10)                      as u8;
        s[24] =  (self.0[6] >> 18)                      as u8;
        s[25] = ((self.0[6] >> 26) | (self.0[7] << 3))  as u8;
        s[26] =  (self.0[7] >>  5)                      as u8;
        s[27] =  (self.0[7] >> 13)                      as u8;
        s[28] =  (self.0[7] >> 21)                      as u8;
        s[29] =  (self.0[8] >>  0)                      as u8;
        s[30] =  (self.0[8] >>  8)                      as u8;
        s[31] =  (self.0[8] >> 16)                      as u8;

        s
    }

    pub(super) fn add(a: &Scalar29, b: &Scalar29) -> Scalar29 {
        let mut sum = Scalar29::ZERO;
        let mask = (1u32 << 29) - 1;

        // a + b
        let mut carry: u32 = 0;
        for i in 0..9 {
            carry = a[i] + b[i] + (carry >> 29);
            sum[i] = carry & mask;
        }

        // subtract l if the sum is >= l
        Scalar29::sub(&sum, &constants::L)
    }

    pub(super) fn conditional_add_l(&mut self, condition: Choice) -> u32 {
        let mut carry: u32 = 0;
        let mask = (1u32 << 29) - 1;

        for i in 0..9 {
            let addend = u32::conditional_select(&0, &constants::L[i], condition);
            carry = (carry >> 29) + self[i] + addend;
            self[i] = carry & mask;
        }
        carry
    }
    pub(super) fn sub(a: &Scalar29, b: &Scalar29) -> Scalar29 {
        let mut difference = Scalar29::ZERO;
        let mask = (1u32 << 29) - 1;

        // a - b
        let mut borrow: u32 = 0;
        for i in 0..9 {
            borrow = a[i].wrapping_sub(b[i] + (borrow >> 31));
            difference[i] = borrow & mask;
        }

        // conditionally add l if the difference is negative
        difference.conditional_add_l(Choice::from((borrow >> 31) as u8));
        difference
    }

    #[rustfmt::skip] // keep alignment of z[*] calculations
    #[inline(always)]
    pub(super) fn mul_internal(a: &Scalar29, b: &Scalar29) -> [u64; 17] {
        let mut z = [0u64; 17];

        z[0] = m(a[0], b[0]);                                                                 // c00
        z[1] = m(a[0], b[1]) + m(a[1], b[0]);                                                 // c01
        z[2] = m(a[0], b[2]) + m(a[1], b[1]) + m(a[2], b[0]);                                 // c02
        z[3] = m(a[0], b[3]) + m(a[1], b[2]) + m(a[2], b[1]) + m(a[3], b[0]);                 // c03
        z[4] = m(a[0], b[4]) + m(a[1], b[3]) + m(a[2], b[2]) + m(a[3], b[1]) + m(a[4], b[0]); // c04
        z[5] =                 m(a[1], b[4]) + m(a[2], b[3]) + m(a[3], b[2]) + m(a[4], b[1]); // c05
        z[6] =                                 m(a[2], b[4]) + m(a[3], b[3]) + m(a[4], b[2]); // c06
        z[7] =                                                 m(a[3], b[4]) + m(a[4], b[3]); // c07
        z[8] =                                                                (m(a[4], b[4])).wrapping_sub(z[3]); // c08 - c03

        z[10] = z[5].wrapping_sub(m(a[5], b[5]));                                                // c05mc10
        z[11] = z[6].wrapping_sub(m(a[5], b[6]) + m(a[6], b[5]));                                // c06mc11
        z[12] = z[7].wrapping_sub(m(a[5], b[7]) + m(a[6], b[6]) + m(a[7], b[5]));                // c07mc12
        z[13] =                   m(a[5], b[8]) + m(a[6], b[7]) + m(a[7], b[6]) + m(a[8], b[5]); // c13
        z[14] =                                   m(a[6], b[8]) + m(a[7], b[7]) + m(a[8], b[6]); // c14
        z[15] =                                                   m(a[7], b[8]) + m(a[8], b[7]); // c15
        z[16] =                                                                   m(a[8], b[8]); // c16

        z[ 5] = z[10].wrapping_sub(z[ 0]); // c05mc10 - c00
        z[ 6] = z[11].wrapping_sub(z[ 1]); // c06mc11 - c01
        z[ 7] = z[12].wrapping_sub(z[ 2]); // c07mc12 - c02
        z[ 8] = z[ 8].wrapping_sub(z[13]); // c08mc13 - c03
        z[ 9] = z[14].wrapping_add(z[ 4]); // c14 + c04
        z[10] = z[15].wrapping_add(z[10]); // c15 + c05mc10
        z[11] = z[16].wrapping_add(z[11]); // c16 + c06mc11

        let aa = [
            a[0] + a[5],
            a[1] + a[6],
            a[2] + a[7],
            a[3] + a[8]
        ];

        let bb = [
            b[0] + b[5],
            b[1] + b[6],
            b[2] + b[7],
            b[3] + b[8]
        ];

        z[ 5] = (m(aa[0], bb[0]))                                                                       .wrapping_add(z[ 5]); // c20 + c05mc10 - c00
        z[ 6] = (m(aa[0], bb[1]) + m(aa[1], bb[0]))                                                     .wrapping_add(z[ 6]); // c21 + c06mc11 - c01
        z[ 7] = (m(aa[0], bb[2]) + m(aa[1], bb[1]) + m(aa[2], bb[0]))                                   .wrapping_add(z[ 7]); // c22 + c07mc12 - c02
        z[ 8] = (m(aa[0], bb[3]) + m(aa[1], bb[2]) + m(aa[2], bb[1]) + m(aa[3], bb[0]))                 .wrapping_add(z[ 8]); // c23 + c08mc13 - c03
        z[ 9] = (m(aa[0],  b[4]) + m(aa[1], bb[3]) + m(aa[2], bb[2]) + m(aa[3], bb[1]) + m(a[4], bb[0])).wrapping_sub(z[ 9]); // c24 - c14 - c04
        z[10] = (                  m(aa[1],  b[4]) + m(aa[2], bb[3]) + m(aa[3], bb[2]) + m(a[4], bb[1])).wrapping_sub(z[10]); // c25 - c15 - c05mc10
        z[11] = (                                    m(aa[2],  b[4]) + m(aa[3], bb[3]) + m(a[4], bb[2])).wrapping_sub(z[11]); // c26 - c16 - c06mc11
        z[12] = (                                                      m(aa[3],  b[4]) + m(a[4], bb[3])).wrapping_sub(z[12]); // c27 - c07mc12

        z
    }

    #[rustfmt::skip] // keep alignment of calculations
    #[rustfmt::skip] // keep alignment of part1() and part2() computations
    #[inline(always)]
    pub(super) fn montgomery_reduce(limbs: &[u64; 17]) -> Scalar29 {

        #[inline(always)]
        fn part1(sum: u64) -> (u64, u32) {
            let p = (sum as u32).wrapping_mul(constants::LFACTOR) & ((1u32 << 29) - 1);
            ((sum + m(p,constants::L[0])) >> 29, p)
        }

        #[inline(always)]
        fn part2(sum: u64) -> (u64, u32) {
            let w = (sum as u32) & ((1u32 << 29) - 1);
            (sum >> 29, w)
        }

        // note: l5,l6,l7 are zero, so their multiplies can be skipped
        let l = &constants::L;

        // the first half computes the Montgomery adjustment factor n, and begins adding n*l to make limbs divisible by R
        let (carry, n0) = part1(        limbs[ 0]);
        let (carry, n1) = part1(carry + limbs[ 1] + m(n0,l[1]));
        let (carry, n2) = part1(carry + limbs[ 2] + m(n0,l[2]) + m(n1,l[1]));
        let (carry, n3) = part1(carry + limbs[ 3] + m(n0,l[3]) + m(n1,l[2]) + m(n2,l[1]));
        let (carry, n4) = part1(carry + limbs[ 4] + m(n0,l[4]) + m(n1,l[3]) + m(n2,l[2]) + m(n3,l[1]));
        let (carry, n5) = part1(carry + limbs[ 5]              + m(n1,l[4]) + m(n2,l[3]) + m(n3,l[2]) + m(n4,l[1]));
        let (carry, n6) = part1(carry + limbs[ 6]                           + m(n2,l[4]) + m(n3,l[3]) + m(n4,l[2]) + m(n5,l[1]));
        let (carry, n7) = part1(carry + limbs[ 7]                                        + m(n3,l[4]) + m(n4,l[3]) + m(n5,l[2]) + m(n6,l[1]));
        let (carry, n8) = part1(carry + limbs[ 8] + m(n0,l[8])                                        + m(n4,l[4]) + m(n5,l[3]) + m(n6,l[2]) + m(n7,l[1]));

        // limbs is divisible by R now, so we can divide by R by simply storing the upper half as the result
        let (carry, r0) = part2(carry + limbs[ 9]              + m(n1,l[8])                                        + m(n5,l[4]) + m(n6,l[3]) + m(n7,l[2]) + m(n8,l[1]));
        let (carry, r1) = part2(carry + limbs[10]                           + m(n2,l[8])                                        + m(n6,l[4]) + m(n7,l[3]) + m(n8,l[2]));
        let (carry, r2) = part2(carry + limbs[11]                                        + m(n3,l[8])                                        + m(n7,l[4]) + m(n8,l[3]));
        let (carry, r3) = part2(carry + limbs[12]                                                     + m(n4,l[8])                                        + m(n8,l[4]));
        let (carry, r4) = part2(carry + limbs[13]                                                                  + m(n5,l[8])                                       );
        let (carry, r5) = part2(carry + limbs[14]                                                                               + m(n6,l[8])                          );
        let (carry, r6) = part2(carry + limbs[15]                                                                                            + m(n7,l[8])             );
        let (carry, r7) = part2(carry + limbs[16]                                                                                                         + m(n8,l[8]));
        let         r8 = carry as u32;

        // result may be >= l, so attempt to subtract l
        Scalar29::sub(&Scalar29([r0,r1,r2,r3,r4,r5,r6,r7,r8]), l)
    }

    pub(super) fn mul(a: &Scalar29, b: &Scalar29) -> Scalar29 {
        let ab = Scalar29::montgomery_reduce(&Scalar29::mul_internal(a, b));
        Scalar29::montgomery_reduce(&Scalar29::mul_internal(&ab, &constants::RR))
    }

    pub(super) fn montgomery_mul(a: &Scalar29, b: &Scalar29) -> Scalar29 {
        Scalar29::montgomery_reduce(&Scalar29::mul_internal(a, b))
    }
}

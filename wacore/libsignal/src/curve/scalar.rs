// Portions derived from curve25519-dalek 5.0.0. See LICENSE.
#[cfg(not(target_pointer_width = "64"))]
use super::scalar29::Scalar29 as Unpacked;
#[cfg(target_pointer_width = "64")]
use super::scalar52::Scalar52 as Unpacked;
use subtle::{Choice, ConditionallySelectable};
use zeroize::Zeroize;
#[derive(Clone, Copy)]
pub(crate) struct Scalar {
    bytes: [u8; 32],
}
impl Zeroize for Scalar {
    fn zeroize(&mut self) {
        self.bytes.zeroize();
    }
}
impl Scalar {
    pub(crate) fn from_bytes_mod_order(bytes: [u8; 32]) -> Self {
        // Montgomery reduction computes bytes * R / R modulo the group order.
        Self {
            bytes: Unpacked::montgomery_mul(&Unpacked::from_bytes(&bytes), &super::constants::R)
                .to_bytes(),
        }
    }
    pub(crate) fn from_bytes_mod_order_wide(bytes: &[u8; 64]) -> Self {
        Self {
            bytes: Unpacked::from_bytes_wide(bytes).to_bytes(),
        }
    }
    pub(crate) fn as_bytes(&self) -> &[u8; 32] {
        &self.bytes
    }
    pub(crate) fn non_adjacent_form(&self, w: usize) -> [i8; 256] {
        // required by the NAF definition
        debug_assert!(w >= 2);
        // required so that the NAF digits fit in i8
        debug_assert!(w <= 8);

        let mut naf = [0i8; 256];

        let mut x_u64 = [0u64; 5];
        read_le_u64_into(&self.bytes, &mut x_u64[0..4]);

        let width = 1 << w;
        let window_mask = width - 1;

        let mut pos = 0;
        let mut carry = 0;
        while pos < 256 {
            // Construct a buffer of bits of the scalar, starting at bit `pos`
            let u64_idx = pos / 64;
            let bit_idx = pos % 64;
            let bit_buf: u64 = if bit_idx < 64 - w {
                // This window's bits are contained in a single u64
                x_u64[u64_idx] >> bit_idx
            } else {
                // Combine the current u64's bits with the bits from the next u64
                (x_u64[u64_idx] >> bit_idx) | (x_u64[1 + u64_idx] << (64 - bit_idx))
            };

            // Add the carry into the current window
            let window = carry + (bit_buf & window_mask);

            if window & 1 == 0 {
                // If the window value is even, preserve the carry and continue.
                // Why is the carry preserved?
                // If carry == 0 and window & 1 == 0, then the next carry should be 0
                // If carry == 1 and window & 1 == 0, then bit_buf & 1 == 1 so the next carry should be 1
                pos += 1;
                continue;
            }

            if window < width / 2 {
                carry = 0;
                naf[pos] = window as i8;
            } else {
                carry = 1;
                naf[pos] = (window as i8).wrapping_sub(width as i8);
            }

            pos += w;
        }

        naf
    }
    pub(crate) fn as_radix_16(&self) -> [i8; 64] {
        debug_assert!(self.bytes[31] <= 127);
        let mut output = [0i8; 64];

        // Step 1: change radix.
        // Convert from radix 256 (bytes) to radix 16 (nibbles)
        #[allow(clippy::identity_op)]
        #[inline(always)]
        fn bot_half(x: u8) -> u8 {
            (x >> 0) & 15
        }
        #[inline(always)]
        fn top_half(x: u8) -> u8 {
            (x >> 4) & 15
        }

        for i in 0..32 {
            output[2 * i] = bot_half(self.bytes[i]) as i8;
            output[2 * i + 1] = top_half(self.bytes[i]) as i8;
        }
        // Precondition note: since self.bytes[31] <= 127, output[63] <= 7

        // Step 2: recenter coefficients from [0,16) to [-8,8)
        for i in 0..63 {
            let carry = (output[i] + 8) >> 4;
            output[i] -= carry << 4;
            output[i + 1] += carry;
        }
        // Precondition note: output[63] is not recentered.  It
        // increases by carry <= 1.  Thus output[63] <= 8.

        output
    }
}
impl ConditionallySelectable for Scalar {
    fn conditional_select(a: &Self, b: &Self, choice: Choice) -> Self {
        Self {
            bytes: core::array::from_fn(|i| {
                u8::conditional_select(&a.bytes[i], &b.bytes[i], choice)
            }),
        }
    }
}
impl core::ops::Add for Scalar {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self {
            bytes: Unpacked::add(
                &Unpacked::from_bytes(&self.bytes),
                &Unpacked::from_bytes(&rhs.bytes),
            )
            .to_bytes(),
        }
    }
}
impl core::ops::Mul for Scalar {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        Self {
            bytes: Unpacked::mul(
                &Unpacked::from_bytes(&self.bytes),
                &Unpacked::from_bytes(&rhs.bytes),
            )
            .to_bytes(),
        }
    }
}
impl core::ops::Neg for Scalar {
    type Output = Self;
    fn neg(self) -> Self {
        Self {
            bytes: Unpacked::sub(&Unpacked::ZERO, &Unpacked::from_bytes(&self.bytes)).to_bytes(),
        }
    }
}
fn read_le_u64_into(src: &[u8], dst: &mut [u64]) {
    for (input, output) in src.chunks_exact(8).zip(dst) {
        *output = u64::from_le_bytes(input.try_into().expect("eight-byte chunk"));
    }
}

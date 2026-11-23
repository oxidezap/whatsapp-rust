// Portions derived from curve25519-dalek 5.0.0. See LICENSE.
#![allow(non_snake_case)]
use super::{edwards::EdwardsPoint, field::FieldElement, scalar::Scalar, traits::Identity};
use subtle::{Choice, ConditionallyNegatable, ConditionallySelectable};
use zeroize::{Zeroize, Zeroizing};
#[derive(Clone, Copy)]
pub(crate) struct MontgomeryPoint(pub(crate) [u8; 32]);
impl MontgomeryPoint {
    pub(crate) fn to_bytes(self) -> [u8; 32] {
        self.0
    }
    pub(crate) fn mul_base_clamped(bytes: [u8; 32]) -> Self {
        Self::mul_base_edwards(bytes).to_montgomery()
    }
    pub(crate) fn mul_base_clamped_batch<const N: usize>(scalars: &[[u8; 32]; N]) -> [Self; N] {
        let mut numerators = Zeroizing::new([FieldElement::ZERO; N]);
        let mut denominators = Zeroizing::new([FieldElement::ZERO; N]);
        for (i, scalar) in scalars.iter().enumerate() {
            (numerators[i], denominators[i]) =
                Self::mul_base_edwards(*scalar).montgomery_coordinates();
        }
        FieldElement::invert_batch(&mut denominators);
        core::array::from_fn(|i| Self((&numerators[i] * &denominators[i]).to_bytes()))
    }
    /// Returns `mul_base_clamped(base)` and `point.mul_clamped(scalar)`, sharing
    /// one field inversion between the two results.
    pub(crate) fn mul_base_clamped_and_mul_clamped(
        base: [u8; 32],
        point: Self,
        scalar: [u8; 32],
    ) -> (Self, Self) {
        let (base_u, base_w) = Self::mul_base_edwards(base).montgomery_coordinates();
        let product = point.mul_clamped_projective(scalar);
        let mut denominators = Zeroizing::new([base_w, product.W]);
        FieldElement::invert_batch(&mut denominators);
        (
            Self((&base_u * &denominators[0]).to_bytes()),
            Self((&product.U * &denominators[1]).to_bytes()),
        )
    }
    fn mul_base_edwards(mut bytes: [u8; 32]) -> EdwardsPoint {
        bytes[0] &= 248;
        bytes[31] = (bytes[31] & 127) | 64;
        EdwardsPoint::mul_base(&Zeroizing::new(Scalar::from_bytes_mod_order(bytes)))
    }
    pub(crate) fn mul_clamped(self, bytes: [u8; 32]) -> Self {
        self.mul_clamped_projective(bytes).as_affine()
    }
    fn mul_clamped_projective(self, mut bytes: [u8; 32]) -> ProjectivePoint {
        bytes[0] &= 248;
        bytes[31] = (bytes[31] & 127) | 64;
        let bytes = Zeroizing::new(bytes);
        self.mul_bits_be(
            (0..255)
                .rev()
                .map(|bit| (bytes[bit / 8] >> (bit % 8)) & 1 != 0),
        )
    }
    fn mul_bits_be(&self, bits: impl Iterator<Item = bool>) -> ProjectivePoint {
        #[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
        if std::is_x86_feature_detected!("bmi2") {
            // SAFETY: the CPU supports BMI2, checked just above.
            #[allow(unsafe_code)]
            return unsafe { self.mul_bits_be_bmi2(bits) };
        }
        self.mul_bits_be_generic(bits)
    }

    // Baseline x86-64 `mul` writes its product to the fixed `rdx:rax` pair;
    // BMI2 `mulx` takes any destination registers. With it, the ladder runs on
    // four saturated 64-bit limbs (`FieldElement64`): 16 limb products per
    // multiplication instead of 25, and fewer live registers, so the step
    // spills much less to the stack than the radix-2^51 one. The field
    // operations are `#[inline(always)]`, so they are compiled for BMI2 here.
    #[cfg(all(target_arch = "x86_64", target_pointer_width = "64"))]
    #[target_feature(enable = "bmi2")]
    fn mul_bits_be_bmi2(&self, bits: impl Iterator<Item = bool>) -> ProjectivePoint {
        use super::field64::FieldElement64 as Fe;

        let affine_u = Fe::from_bytes(&self.0);
        let mut x0 = (Fe::ONE, Fe::ZERO);
        let mut x1 = (affine_u, Fe::ONE);

        let mut prev_bit = false;
        for cur_bit in bits {
            let choice = Choice::from((prev_bit ^ cur_bit) as u8);
            Fe::conditional_swap(&mut x0.0, &mut x1.0, choice);
            Fe::conditional_swap(&mut x0.1, &mut x1.1, choice);

            // Same formulas as `differential_add_and_double`.
            let (pu, pw) = x0;
            let (qu, qw) = x1;
            let t0 = pu.add(&pw);
            let t1 = pu.sub(&pw);
            let t2 = qu.add(&qw);
            let t3 = qu.sub(&qw);
            let t4 = t0.square();
            let t5 = t1.square();
            let t6 = t4.sub(&t5);
            let t7 = t0.mul(&t3);
            let t8 = t1.mul(&t2);
            let t9 = t7.add(&t8);
            let t10 = t7.sub(&t8);
            let t15 = t6.mul_small_add(121666, &t5);
            x0 = (t4.mul(&t5), t6.mul(&t15));
            x1 = (t9.square(), affine_u.mul(&t10.square()));

            prev_bit = cur_bit;
        }
        let choice = Choice::from(prev_bit as u8);
        Fe::conditional_swap(&mut x0.0, &mut x1.0, choice);
        Fe::conditional_swap(&mut x0.1, &mut x1.1, choice);
        prev_bit.zeroize();

        ProjectivePoint {
            U: x0.0.to_field51(),
            W: x0.1.to_field51(),
        }
    }

    #[inline(always)]
    fn mul_bits_be_generic(&self, bits: impl Iterator<Item = bool>) -> ProjectivePoint {
        // Algorithm 8 of Costello-Smith 2017
        let affine_u = FieldElement::from_bytes(&self.0);
        let mut x0 = ProjectivePoint::identity();
        let mut x1 = ProjectivePoint {
            U: affine_u,
            W: FieldElement::ONE,
        };

        // Go through the bits from most to least significant, using a sliding window of 2
        let mut prev_bit = false;
        for cur_bit in bits {
            let choice: u8 = (prev_bit ^ cur_bit) as u8;

            debug_assert!(choice == 0 || choice == 1);

            ProjectivePoint::conditional_swap(&mut x0, &mut x1, choice.into());
            differential_add_and_double(&mut x0, &mut x1, &affine_u);

            prev_bit = cur_bit;
        }
        // The final value of prev_bit above is scalar.bits()[0], i.e., the LSB of scalar
        ProjectivePoint::conditional_swap(&mut x0, &mut x1, Choice::from(prev_bit as u8));
        // Don't leave the bit in the stack
        prev_bit.zeroize();

        x0
    }
    pub(crate) fn to_edwards(self, sign: u8) -> Option<EdwardsPoint> {
        // The birational map y = (u-1)/(u+1) is undefined at u = -1,
        // which lies on the twist and must be rejected.
        let u = FieldElement::from_bytes(&self.0);

        if u == FieldElement::MINUS_ONE {
            return None;
        }

        let one = FieldElement::ONE;

        // With y = yn/yd, x² = (yn²-yd²)/(d*yn²+yd²). Keep Y and Z
        // projective to avoid a separate inversion before the square root.
        // The denominator cannot vanish for yd != 0 because -1/d is nonsquare.
        let yn = &u - &one;
        let yd = &u + &one;
        let yn2 = yn.square();
        let yd2 = yd.square();

        let (is_valid_y_coord, mut X) = FieldElement::sqrt_ratio(
            &(&yn2 - &yd2),
            &(&(&yn2 * &super::constants::EDWARDS_D) + &yd2),
        );

        if !bool::from(is_valid_y_coord) {
            return None;
        }

        // sqrt_ratio returns the nonnegative root. Preserve the former
        // encoding's low-bit sign semantics before constructing a Choice.
        X.conditional_negate(Choice::from(sign & 1));

        // (x, yn/yd) in projective coordinates, with T satisfying T*Z = X*Y.
        Some(EdwardsPoint {
            X: &X * &yd,
            Y: yn,
            Z: yd,
            T: &X * &yn,
        })
    }
}
#[derive(Copy, Clone)]
struct ProjectivePoint {
    pub U: FieldElement,
    pub W: FieldElement,
}

impl Identity for ProjectivePoint {
    fn identity() -> ProjectivePoint {
        ProjectivePoint {
            U: FieldElement::ONE,
            W: FieldElement::ZERO,
        }
    }
}

impl ConditionallySelectable for ProjectivePoint {
    fn conditional_select(
        a: &ProjectivePoint,
        b: &ProjectivePoint,
        choice: Choice,
    ) -> ProjectivePoint {
        ProjectivePoint {
            U: FieldElement::conditional_select(&a.U, &b.U, choice),
            W: FieldElement::conditional_select(&a.W, &b.W, choice),
        }
    }

    #[inline]
    fn conditional_swap(a: &mut Self, b: &mut Self, choice: Choice) {
        FieldElement::conditional_swap(&mut a.U, &mut b.U, choice);
        FieldElement::conditional_swap(&mut a.W, &mut b.W, choice);
    }
}

impl ProjectivePoint {
    fn as_affine(&self) -> MontgomeryPoint {
        let u = &self.U * &self.W.invert();
        MontgomeryPoint(u.to_bytes())
    }
}

#[rustfmt::skip] // keep alignment of explanatory comments
#[inline(always)]
fn differential_add_and_double(
    P: &mut ProjectivePoint,
    Q: &mut ProjectivePoint,
    affine_PmQ: &FieldElement,
) {
    let t0 = &P.U + &P.W;
    // Every difference in this step only feeds a multiplication or squaring,
    // and every input is the output of a field carry chain or `from_bytes`,
    // so the differences can skip the carry pass of `Sub` (see `sub_ladder`).
    let t1 = P.U.sub_ladder(&P.W);
    let t2 = &Q.U + &Q.W;
    let t3 = Q.U.sub_ladder(&Q.W);

    let t4 = t0.square();   // (U_P + W_P)^2 = U_P^2 + 2 U_P W_P + W_P^2
    let t5 = t1.square();   // (U_P - W_P)^2 = U_P^2 - 2 U_P W_P + W_P^2

    let t6 = t4.sub_ladder(&t5); // 4 U_P W_P

    let t7 = t0.mul_ladder(&t3);     // (U_P + W_P) (U_Q - W_Q) = U_P U_Q + W_P U_Q - U_P W_Q - W_P W_Q
    let t8 = t1.mul_ladder(&t2);     // (U_P - W_P) (U_Q + W_Q) = U_P U_Q - W_P U_Q + U_P W_Q - W_P W_Q

    let t9  = &t7 + &t8;    // 2 (U_P U_Q - W_P W_Q)
    let t10 = t7.sub_ladder(&t8); // 2 (W_P U_Q - U_P W_Q)

    let t11 =  t9.square(); // 4 (U_P U_Q - W_P W_Q)^2
    let t12 = t10.square(); // 4 (W_P U_Q - U_P W_Q)^2

    let t13 = t6.mul121666(); // (A + 2) U_P U_Q

    let t14 = t4.mul_ladder(&t5);    // ((U_P + W_P)(U_P - W_P))^2 = (U_P^2 - W_P^2)^2
    let t15 = &t13 + &t5;   // (U_P - W_P)^2 + (A + 2) U_P W_P

    let t16 = t6.mul_ladder(&t15);   // 4 (U_P W_P) ((U_P - W_P)^2 + (A + 2) U_P W_P)

    let t17 = affine_PmQ.mul_ladder(&t12); // U_D * 4 (W_P U_Q - U_P W_Q)^2
    let t18 = t11;               // W_D * 4 (U_P U_Q - W_P W_Q)^2

    P.U = t14;  // U_{P'} = (U_P + W_P)^2 (U_P - W_P)^2
    P.W = t16;  // W_{P'} = (4 U_P W_P) ((U_P - W_P)^2 + ((A + 2)/4) 4 U_P W_P)
    Q.U = t18;  // U_{Q'} = W_D * 4 (U_P U_Q - W_P W_Q)^2
    Q.W = t17;  // W_{Q'} = U_D * 4 (W_P U_Q - U_P W_Q)^2
}

#[cfg(all(test, target_arch = "x86_64", target_pointer_width = "64"))]
mod tests {
    use super::*;
    use sha2::{Digest, Sha512};

    #[test]
    fn bmi2_ladder_matches_portable_ladder() {
        if !std::is_x86_feature_detected!("bmi2") {
            return;
        }
        for seed in 0u32..256 {
            let wide: [u8; 64] = Sha512::digest(seed.to_le_bytes()).into();
            let point = MontgomeryPoint(wide[..32].try_into().unwrap());
            let scalar: [u8; 32] = wide[32..].try_into().unwrap();
            let bits = || {
                (0..256)
                    .rev()
                    .map(|bit| (scalar[bit / 8] >> (bit % 8)) & 1 != 0)
            };
            // SAFETY: BMI2 support is checked above.
            #[allow(unsafe_code)]
            let bmi2 = unsafe { point.mul_bits_be_bmi2(bits()) }.as_affine();
            let portable = point.mul_bits_be_generic(bits()).as_affine();
            assert_eq!(bmi2.0, portable.0, "seed {seed}");
        }
    }
}

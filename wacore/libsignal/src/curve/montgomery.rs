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
    pub(crate) fn mul_base_clamped(mut bytes: [u8; 32]) -> Self {
        bytes[0] &= 248;
        bytes[31] = (bytes[31] & 127) | 64;
        EdwardsPoint::mul_base(&Zeroizing::new(Scalar::from_bytes_mod_order(bytes))).to_montgomery()
    }
    pub(crate) fn mul_clamped(self, mut bytes: [u8; 32]) -> Self {
        bytes[0] &= 248;
        bytes[31] = (bytes[31] & 127) | 64;
        let bytes = Zeroizing::new(bytes);
        self.mul_bits_be(
            (0..255)
                .rev()
                .map(|bit| (bytes[bit / 8] >> (bit % 8)) & 1 != 0),
        )
    }
    fn mul_bits_be(&self, bits: impl Iterator<Item = bool>) -> MontgomeryPoint {
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

        x0.as_affine()
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
}

impl ProjectivePoint {
    fn as_affine(&self) -> MontgomeryPoint {
        let u = &self.U * &self.W.invert();
        MontgomeryPoint(u.to_bytes())
    }
}

#[rustfmt::skip] // keep alignment of explanatory comments
fn differential_add_and_double(
    P: &mut ProjectivePoint,
    Q: &mut ProjectivePoint,
    affine_PmQ: &FieldElement,
) {
    let t0 = &P.U + &P.W;
    let t1 = &P.U - &P.W;
    let t2 = &Q.U + &Q.W;
    let t3 = &Q.U - &Q.W;

    let t4 = t0.square();   // (U_P + W_P)^2 = U_P^2 + 2 U_P W_P + W_P^2
    let t5 = t1.square();   // (U_P - W_P)^2 = U_P^2 - 2 U_P W_P + W_P^2

    let t6 = &t4 - &t5;     // 4 U_P W_P

    let t7 = &t0 * &t3;     // (U_P + W_P) (U_Q - W_Q) = U_P U_Q + W_P U_Q - U_P W_Q - W_P W_Q
    let t8 = &t1 * &t2;     // (U_P - W_P) (U_Q + W_Q) = U_P U_Q - W_P U_Q + U_P W_Q - W_P W_Q

    let t9  = &t7 + &t8;    // 2 (U_P U_Q - W_P W_Q)
    let t10 = &t7 - &t8;    // 2 (W_P U_Q - U_P W_Q)

    let t11 =  t9.square(); // 4 (U_P U_Q - W_P W_Q)^2
    let t12 = t10.square(); // 4 (W_P U_Q - U_P W_Q)^2

    let t13 = t6.mul121666(); // (A + 2) U_P U_Q

    let t14 = &t4 * &t5;    // ((U_P + W_P)(U_P - W_P))^2 = (U_P^2 - W_P^2)^2
    let t15 = &t13 + &t5;   // (U_P - W_P)^2 + (A + 2) U_P W_P

    let t16 = &t6 * &t15;   // 4 (U_P W_P) ((U_P - W_P)^2 + (A + 2) U_P W_P)

    let t17 = affine_PmQ * &t12; // U_D * 4 (W_P U_Q - U_P W_Q)^2
    let t18 = t11;               // W_D * 4 (U_P U_Q - W_P W_Q)^2

    P.U = t14;  // U_{P'} = (U_P + W_P)^2 (U_P - W_P)^2
    P.W = t16;  // W_{P'} = (4 U_P W_P) ((U_P - W_P)^2 + ((A + 2)/4) 4 U_P W_P)
    Q.U = t18;  // U_{Q'} = W_D * 4 (U_P U_Q - W_P W_Q)^2
    Q.W = t17;  // W_{Q'} = U_D * 4 (W_P U_Q - U_P W_Q)^2
}

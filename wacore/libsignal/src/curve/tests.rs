// Transcript recorded against crates.io curve25519-dalek 5.0.0.
use super::{edwards::EdwardsPoint, montgomery::MontgomeryPoint, scalar::Scalar};
use sha2::{Digest, Sha512};

#[test]
fn primitive_transcript_matches_dalek_5_0_0() {
    let mut transcript = Sha512::new();
    let mut accepted = 0;
    let mut rejected = 0;
    for seed in 0u32..512 {
        let mut wide: [u8; 64] = Sha512::digest(seed.to_le_bytes()).into();
        match seed {
            0 => wide.fill(0),
            1 => {
                wide.fill(0);
                wide[0] = 1;
            }
            2 => wide.fill(255),
            3 => {
                wide.fill(255);
                wide[31] = 127;
            }
            _ => {}
        }
        let raw: [u8; 32] = wide[..32].try_into().unwrap();
        let a = Scalar::from_bytes_mod_order(raw);
        let b = Scalar::from_bytes_mod_order_wide(&wide);
        for scalar in [a, b, a + b, a * b, -a] {
            transcript.update(scalar.as_bytes());
            transcript.update(EdwardsPoint::mul_base(&scalar).compress().as_bytes());
        }
        let point = MontgomeryPoint(raw);
        transcript.update(point.mul_clamped([seed as u8; 32]).to_bytes());
        for sign in [0, 1] {
            if let Some(point) = point.to_edwards(sign) {
                accepted += 1;
                transcript.update([1]);
                transcript.update(point.compress().as_bytes());
                transcript.update(point.to_montgomery().to_bytes());
                transcript.update([u8::from(point.is_small_order())]);
                transcript.update(
                    EdwardsPoint::vartime_double_scalar_mul_basepoint(&a, &point, &b)
                        .compress()
                        .as_bytes(),
                );
            } else {
                rejected += 1;
                transcript.update([0]);
            }
        }
    }
    assert!(accepted > 0 && rejected > 0);
    assert_eq!(
        hex::encode(transcript.finalize()),
        "c5bdf4504707b4cd22c98de694e098dc959c7e352992f5ecff7228b6b604723a4a85376a7f3845de1533fcb030c6a1cf7c82baa25925f29e83aeb4f6d9939685"
    );
}

#[cfg(target_pointer_width = "64")]
#[test]
fn squaring_matches_multiplication_at_limb_bounds() {
    use super::field51::FieldElement51;
    for bits in [51, 52, 53, 54] {
        let max = (1u64 << bits) - 1;
        for index in 0..5 {
            for value in [0, 1, max - 1, max] {
                let mut limbs = [max; 5];
                limbs[index] = value;
                let a = FieldElement51::from_limbs(limbs);
                assert_eq!(a.square().to_bytes(), (&a * &a).to_bytes());
                let factor = FieldElement51::from_limbs([121666, 0, 0, 0, 0]);
                assert_eq!(a.mul121666().to_bytes(), (&a * &factor).to_bytes());
                let square = a.square();
                assert_eq!(a.pow2k(2).to_bytes(), (&square * &square).to_bytes());
            }
        }
    }
}

#[test]
fn field_predicates_match_canonical_encoding() {
    use super::field::FieldElement;
    use subtle::ConstantTimeEq;
    let mut modulus = [255; 32];
    modulus[0] = 237;
    modulus[31] = 127;
    let mut cases = vec![
        FieldElement::ZERO,
        FieldElement::ONE,
        FieldElement::MINUS_ONE,
        FieldElement::from_bytes(&modulus),
    ];
    for seed in 0u32..512 {
        let bytes: [u8; 32] = Sha512::digest(seed.to_le_bytes())[..32].try_into().unwrap();
        let a = FieldElement::from_bytes(&bytes);
        cases.extend([a, &a + &a, &(&a + &FieldElement::ONE) - &FieldElement::ONE]);
    }
    for a in &cases {
        let bytes = a.to_bytes();
        assert_eq!(a.is_negative().unwrap_u8(), bytes[0] & 1);
        assert_eq!(bool::from(a.is_zero()), bytes == [0; 32]);
        for b in [a, &FieldElement::ZERO, &FieldElement::ONE, &cases[3]] {
            assert_eq!(bool::from(a.ct_eq(b)), bytes == b.to_bytes());
        }
    }
}

const fn pack_radix51(mut limbs: [u64; 5]) -> [u64; 4] {
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
    [
        limbs[0] | (limbs[1] << 51),
        (limbs[1] >> 13) | (limbs[2] << 38),
        (limbs[2] >> 26) | (limbs[3] << 25),
        (limbs[3] >> 39) | (limbs[4] << 12),
    ]
}

#[test]
fn radix51_constants_match_byte_decoding() {
    use super::field::{FieldElement, from_radix51};
    for bits in [51, 52, 53, 54] {
        let max = (1u64 << bits) - 1;
        for index in 0..5 {
            for value in [0, 1, max - 1, max] {
                let mut limbs = [max; 5];
                limbs[index] = value;
                let packed = pack_radix51(limbs);
                let mut bytes = [0; 32];
                for (word, chunk) in packed.iter().zip(bytes.chunks_exact_mut(8)) {
                    chunk.copy_from_slice(&word.to_le_bytes());
                }
                assert_eq!(
                    FieldElement::from_bytes(&bytes).to_bytes(),
                    from_radix51(limbs).to_bytes()
                );
            }
        }
    }
}

#[test]
fn signature_scalar_reduction_preserves_xeddsa_boundaries() {
    let order = super::constants::L.to_bytes();
    let mut below_order = order;
    below_order[0] -= 1;
    let mut above_order = order;
    above_order[0] += 1;
    let mut limit = [255; 32];
    limit[31] = 31;
    for mut bytes in [[0; 32], below_order, order, above_order, limit] {
        let expected = Scalar::from_bytes_mod_order(bytes);
        for sign in [0, 128] {
            bytes[31] = (bytes[31] & 127) | sign;
            assert_eq!(
                Scalar::from_signature_bytes(bytes).unwrap().as_bytes(),
                expected.as_bytes()
            );
        }
    }

    for seed in 0u32..512 {
        let mut bytes: [u8; 32] = Sha512::digest(seed.to_le_bytes())[..32].try_into().unwrap();
        for top in [0, 0x0f, 0x10, 0x1f, 0x20, 0x40, 0x60, 0x80, 0x9f, 0xff] {
            bytes[31] = top;
            let scalar = Scalar::from_signature_bytes(bytes);
            bytes[31] &= 127;
            assert_eq!(scalar.is_some(), bytes[31] < 32);
            if let Some(scalar) = scalar {
                assert_eq!(
                    scalar.as_bytes(),
                    Scalar::from_bytes_mod_order(bytes).as_bytes()
                );
            }
        }
    }
}

#[test]
fn fixed_base_selection_preserves_every_signed_digit() {
    use super::{
        field::from_radix51, models::AffineNielsPoint, traits::Identity, window::LookupTable,
    };
    let table = LookupTable(core::array::from_fn(|index| {
        let seed = index as u64 + 1;
        AffineNielsPoint {
            y_plus_x: from_radix51([seed, seed << 30, seed << 40, seed << 45, seed << 47]),
            y_minus_x: from_radix51([seed << 5, seed << 25, seed << 35, seed << 40, seed << 45]),
            xy2d: from_radix51([seed << 2, seed << 20, seed << 32, seed << 42, seed << 48]),
        }
    }));
    for digit in -8i8..=8 {
        let found = table.select(digit);
        let mut expected = if digit == 0 {
            AffineNielsPoint::identity()
        } else {
            table.0[digit.unsigned_abs() as usize - 1]
        };
        if digit < 0 {
            expected = -&expected;
        }
        for (a, b) in [
            (found.y_plus_x, expected.y_plus_x),
            (found.y_minus_x, expected.y_minus_x),
            (found.xy2d, expected.xy2d),
        ] {
            assert_eq!(a.to_bytes(), b.to_bytes());
        }
    }
}

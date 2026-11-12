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

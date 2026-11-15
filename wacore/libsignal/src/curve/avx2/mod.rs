//! AVX2 verification only. All vector types and intrinsics stay below the
//! checked entry point, which enables AVX2 before any vector arithmetic runs.
//! Intrinsics only operate on registers; the constant constructor transmutes
//! equal-sized types with no invalid bit patterns. No raw pointer access is used.
mod constants;
mod edwards;
mod field;
mod packed;

use super::{edwards::EdwardsPoint, scalar::Scalar, traits::Identity, window::NafLookupTable5};
use edwards::{CachedPoint, ExtendedPoint};

pub(super) fn mul(a: &Scalar, point: &EdwardsPoint, b: &Scalar) -> Option<EdwardsPoint> {
    if !std::is_x86_feature_detected!("avx2") {
        return None;
    }
    // SAFETY: Runtime detection includes the OS support required for AVX2.
    Some(unsafe { mul_avx2(a, point, b) })
}

#[target_feature(enable = "avx2")]
unsafe fn mul_avx2(a: &Scalar, point: &EdwardsPoint, b: &Scalar) -> EdwardsPoint {
    let a_naf = a.non_adjacent_form(5);
    let b_naf = b.non_adjacent_form(8);
    let table_a = NafLookupTable5::<CachedPoint>::from(point);
    let table_b = &constants::BASEPOINT_ODD_LOOKUP_TABLE;
    let mut result = ExtendedPoint::identity();
    let top = (0..256)
        .rfind(|&i| a_naf[i] != 0 || b_naf[i] != 0)
        .unwrap_or(0);
    for i in (0..=top).rev() {
        result = result.double();
        match a_naf[i].cmp(&0) {
            core::cmp::Ordering::Greater => result = &result + &table_a.select(a_naf[i] as usize),
            core::cmp::Ordering::Less => result = &result - &table_a.select(-a_naf[i] as usize),
            core::cmp::Ordering::Equal => {}
        }
        match b_naf[i].cmp(&0) {
            core::cmp::Ordering::Greater => result = &result + &table_b.select(b_naf[i] as usize),
            core::cmp::Ordering::Less => result = &result - &table_b.select(-b_naf[i] as usize),
            core::cmp::Ordering::Equal => {}
        }
    }
    result.into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha512};

    #[test]
    fn vector_matches_serial_for_arbitrary_points_and_scalar_boundaries() {
        if !std::is_x86_feature_detected!("avx2") {
            return;
        }
        for seed in 0u32..512 {
            let mut wide: [u8; 64] = Sha512::digest(seed.to_le_bytes()).into();
            match seed {
                0 => wide.fill(0),
                1 => {
                    wide.fill(0);
                    wide[0] = 1;
                }
                2 => wide.fill(255),
                _ => {}
            }
            let a = Scalar::from_bytes_mod_order_wide(&wide);
            wide.reverse();
            let b = Scalar::from_bytes_mod_order_wide(&wide);
            let raw: [u8; 32] = wide[..32].try_into().unwrap();
            for sign in [0, 1] {
                let Some(point) = super::super::montgomery::MontgomeryPoint(raw).to_edwards(sign)
                else {
                    continue;
                };
                let vector = mul(&a, &point, &b).unwrap();
                let serial = super::super::vartime::mul_serial(&a, &point, &b);
                assert_eq!(vector.compress().as_bytes(), serial.compress().as_bytes());
            }
        }
    }

    #[test]
    fn vector_matches_serial_for_every_basepoint_table_entry() {
        if !std::is_x86_feature_detected!("avx2") {
            return;
        }
        let zero = Scalar::from_bytes_mod_order([0; 32]);
        let point = EdwardsPoint::identity();
        for odd in (1..128).step_by(2) {
            let mut bytes = [0; 32];
            bytes[0] = odd;
            let scalar = Scalar::from_bytes_mod_order(bytes);
            let vector = mul(&zero, &point, &scalar).unwrap();
            assert_eq!(
                vector.compress().as_bytes(),
                EdwardsPoint::mul_base(&scalar).compress().as_bytes()
            );
        }
    }
}

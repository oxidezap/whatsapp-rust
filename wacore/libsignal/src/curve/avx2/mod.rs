//! AVX2 verification only. All vector types and intrinsics stay below the
//! checked entry point, which enables AVX2 before any vector arithmetic runs.
//! Intrinsics only operate on registers; the constant constructor transmutes
//! equal-sized types with no invalid bit patterns. No raw pointer access is used.
mod constants;
mod edwards;
mod field;
mod packed;

use super::{
    edwards::EdwardsPoint,
    scalar::Scalar,
    traits::Identity,
    window::{NafLookupTable5, NafLookupTable8},
};
use edwards::{CachedPoint, ExtendedPoint};
use std::sync::OnceLock;

const SEGMENTS: usize = 4;
const SEGMENT_BITS: usize = 256 / SEGMENTS;

/// Odd multiples of `2^(64 j) A` for each segment `j` of a verifying key.
///
/// Splitting both NAFs into four 64-digit segments lets one verification
/// share 64 doublings across eight table lookups per digit position, instead
/// of doubling 253 times. Constructed only after AVX2 detection.
#[derive(Clone)]
pub(super) struct SegmentTables {
    point: [NafLookupTable5<CachedPoint>; SEGMENTS],
    basepoint: &'static [NafLookupTable8<CachedPoint>; SEGMENTS - 1],
}

/// Odd multiples of `2^(64 j) B` for segments 1..4; segment 0 is
/// `BASEPOINT_ODD_LOOKUP_TABLE`.
static BASEPOINT_SEGMENTS: OnceLock<Box<[NafLookupTable8<CachedPoint>; SEGMENTS - 1]>> =
    OnceLock::new();

pub(super) fn prepare(point: &EdwardsPoint) -> Option<Box<SegmentTables>> {
    if !std::is_x86_feature_detected!("avx2") {
        return None;
    }
    // SAFETY: Runtime detection includes the OS support required for AVX2.
    Some(unsafe { prepare_avx2(point) })
}

#[target_feature(enable = "avx2")]
unsafe fn prepare_avx2(point: &EdwardsPoint) -> Box<SegmentTables> {
    // The closure inherits this function's AVX2 target feature, so the
    // tables are built only by the thread that initializes the cell.
    let basepoint = &**BASEPOINT_SEGMENTS.get_or_init(|| basepoint_segments_avx2());
    let mut segment = ExtendedPoint::from(*point);
    let mut tables = [const { None }; SEGMENTS];
    for (j, table) in tables.iter_mut().enumerate() {
        if j > 0 {
            segment = shift_segment(segment);
        }
        let mut multiples = [CachedPoint::from(segment); 8];
        odd_multiples(segment, &mut multiples);
        *table = Some(NafLookupTable5(multiples));
    }
    Box::new(SegmentTables {
        point: tables.map(|table| table.expect("every segment table is built")),
        basepoint,
    })
}

// Table construction runs once per key or once per process, so these helpers
// stay out of line to keep the inlined point arithmetic out of the binary.
#[target_feature(enable = "avx2")]
#[inline(never)]
fn basepoint_segments_avx2() -> Box<[NafLookupTable8<CachedPoint>; SEGMENTS - 1]> {
    let mut one = [0; 32];
    one[0] = 1;
    let mut segment =
        ExtendedPoint::from(EdwardsPoint::mul_base(&Scalar::from_bytes_mod_order(one)));
    let mut tables = [const { None }; SEGMENTS - 1];
    for table in &mut tables {
        segment = shift_segment(segment);
        let mut multiples = [CachedPoint::from(segment); 64];
        odd_multiples(segment, &mut multiples);
        *table = Some(NafLookupTable8(multiples));
    }
    Box::new(tables.map(|table| table.expect("every segment table is built")))
}

/// Fills `multiples` with `first`, `3 first`, `5 first`, ...
#[target_feature(enable = "avx2")]
#[inline(never)]
fn odd_multiples(first: ExtendedPoint, multiples: &mut [CachedPoint]) {
    let double = first.double();
    multiples[0] = CachedPoint::from(first);
    for i in 0..multiples.len() - 1 {
        multiples[i + 1] = CachedPoint::from(&double + &multiples[i]);
    }
}

#[target_feature(enable = "avx2")]
#[inline(never)]
fn shift_segment(mut point: ExtendedPoint) -> ExtendedPoint {
    for _ in 0..SEGMENT_BITS {
        point = point.double();
    }
    point
}

/// Computes `a A + b B` with the segment tables of `A`.
pub(super) fn mul_prepared(a: &Scalar, tables: &SegmentTables, b: &Scalar) -> EdwardsPoint {
    // SAFETY: `SegmentTables` is only built by `prepare` after AVX2 detection.
    unsafe { mul_prepared_avx2(a, tables, b) }
}

#[target_feature(enable = "avx2")]
unsafe fn mul_prepared_avx2(a: &Scalar, tables: &SegmentTables, b: &Scalar) -> EdwardsPoint {
    let a_naf = a.non_adjacent_form(5);
    let b_naf = b.non_adjacent_form(8);
    let mut result = ExtendedPoint::identity();
    let top = (0..SEGMENT_BITS)
        .rfind(|&i| {
            (0..SEGMENTS)
                .any(|j| a_naf[j * SEGMENT_BITS + i] != 0 || b_naf[j * SEGMENT_BITS + i] != 0)
        })
        .unwrap_or(0);
    for i in (0..=top).rev() {
        result = result.double();
        for j in 0..SEGMENTS {
            let digit = a_naf[j * SEGMENT_BITS + i];
            if digit != 0 {
                result = &result + &signed_multiple(&tables.point[j].0, digit);
            }
            let digit = b_naf[j * SEGMENT_BITS + i];
            if digit != 0 {
                let table = match j {
                    0 => &constants::BASEPOINT_ODD_LOOKUP_TABLE,
                    _ => &tables.basepoint[j - 1],
                };
                result = &result + &signed_multiple(&table.0, digit);
            }
        }
    }
    result.into()
}

/// Selects `digit * P` from the odd multiples of `P`. A select instead of a
/// branch keeps LLVM from copying each addition, and its inlined field
/// multiplications, into separate add and subtract paths.
#[inline(always)]
fn signed_multiple(multiples: &[CachedPoint], digit: i8) -> CachedPoint {
    let point = &multiples[usize::from(digit.unsigned_abs()) / 2];
    core::hint::select_unpredictable(digit < 0, -point, *point)
}

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
    fn segmented_matches_serial_for_arbitrary_points_and_scalar_boundaries() {
        if !std::is_x86_feature_detected!("avx2") {
            return;
        }
        // l - 1, the largest reduced scalar, and 2^253 - 1, the largest
        // unreduced XEdDSA s, reach the top NAF segment.
        let mut order_minus_one = [
            0xec, 0xd3, 0xf5, 0x5c, 0x1a, 0x63, 0x12, 0x58, 0xd6, 0x9c, 0xf7, 0xa2, 0xde, 0xf9,
            0xde, 0x14, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0x10,
        ];
        let mut top = [255; 32];
        top[31] = 0x1f;
        for seed in 0u32..256 {
            let mut wide: [u8; 64] = Sha512::digest(seed.to_le_bytes()).into();
            let raw: [u8; 32] = wide[..32].try_into().unwrap();
            let (a, b) = match seed {
                0 => (
                    Scalar::from_bytes_mod_order([0; 32]),
                    Scalar::from_bytes_mod_order([0; 32]),
                ),
                1 => (
                    Scalar::from_bytes_mod_order(order_minus_one),
                    Scalar::from_signature_bytes(top).unwrap(),
                ),
                2 => {
                    order_minus_one[0] = 1;
                    (
                        Scalar::from_bytes_mod_order(order_minus_one),
                        Scalar::from_bytes_mod_order([0; 32]),
                    )
                }
                _ => {
                    let a = Scalar::from_bytes_mod_order_wide(&wide);
                    wide.reverse();
                    (a, Scalar::from_bytes_mod_order_wide(&wide))
                }
            };
            for sign in [0, 1] {
                let Some(point) = super::super::montgomery::MontgomeryPoint(raw).to_edwards(sign)
                else {
                    continue;
                };
                let tables = prepare(&point).unwrap();
                let segmented = mul_prepared(&a, &tables, &b);
                let serial = super::super::vartime::mul_serial(&a, &point, &b);
                assert_eq!(
                    segmented.compress().as_bytes(),
                    serial.compress().as_bytes()
                );
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

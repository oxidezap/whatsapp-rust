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
    field::FieldElement,
    scalar::Scalar,
    traits::Identity,
    window::{NafLookupTable5, NafLookupTable8},
};
// This module only builds for x86_64, which has native 64-bit atomics.
#[allow(clippy::disallowed_types)]
use core::sync::atomic::{AtomicU64, Ordering};
use edwards::{CachedPoint, ExtendedPoint};
use std::sync::OnceLock;

const SEGMENTS: usize = 3;
const SEGMENT_BITS: usize = 86;

/// Affine `2^86 A` and `2^172 A` of a verifying key `A`, as the little-endian
/// words of their canonical `x` and `y`, filled by its second verification.
///
/// Splitting both NAFs into three 86-digit segments lets one verification
/// share 86 doublings across six table lookups per digit position, instead
/// of doubling 253 times. Only these two points are kept per key, and the
/// odd-multiple tables are rebuilt on each verification, so a prepared key
/// is no larger than the extended point it replaces.
#[allow(clippy::disallowed_types)]
pub(super) struct SegmentPoints([AtomicU64; 16]);

/// Set in the top bit of the last word, which a canonical `y < 2^255` leaves
/// clear, once every other word is stored.
const READY: u64 = 1 << 63;

/// Set in the top bit of the first point's `y` by the first verification,
/// which runs unsegmented, so a key that verifies a single signature costs
/// the same as an unprepared one.
const SEEN: u64 = 1 << 63;

#[allow(clippy::disallowed_types)]
impl SegmentPoints {
    pub(super) const fn new() -> Self {
        Self([const { AtomicU64::new(0) }; 16])
    }

    /// Returns whether an earlier verification already marked this key.
    fn mark_seen(&self) -> bool {
        self.0[7].fetch_or(SEEN, Ordering::Relaxed) & SEEN != 0
    }

    // Out of line, with `store`, to keep the serial field arithmetic out of
    // `mul_prepared_avx2`.
    #[inline(never)]
    fn load(&self) -> Option<[EdwardsPoint; SEGMENTS - 1]> {
        let last = self.0[15].load(Ordering::Acquire);
        if last & READY == 0 {
            return None;
        }
        let words: [u64; 16] = core::array::from_fn(|i| self.0[i].load(Ordering::Relaxed));
        Some(core::array::from_fn(|k| {
            let coordinate = |offset: usize| {
                let mut bytes = [0; 32];
                for i in 0..4 {
                    bytes[8 * i..8 * i + 8].copy_from_slice(&words[offset + i].to_le_bytes());
                }
                // `from_bytes` ignores bit 255, which holds `READY` or `SEEN`.
                FieldElement::from_bytes(&bytes)
            };
            EdwardsPoint::from_affine(coordinate(8 * k), coordinate(8 * k + 4))
        }))
    }

    /// Concurrent verifications may both store; they store the same
    /// words, and `READY` is published last.
    #[inline(never)]
    fn store(&self, points: &[EdwardsPoint; SEGMENTS - 1]) {
        let mut inverses = points.map(|point| point.Z);
        FieldElement::invert_batch(&mut inverses);
        let mut words = [0; 16];
        for (k, (point, inverse)) in points.iter().zip(&inverses).enumerate() {
            let x = (&point.X * inverse).to_bytes();
            let y = (&point.Y * inverse).to_bytes();
            for i in 0..4 {
                words[8 * k + i] = u64::from_le_bytes(
                    x[8 * i..8 * i + 8]
                        .try_into()
                        .expect("coordinate slice has fixed eight-byte length"),
                );
                words[8 * k + 4 + i] = u64::from_le_bytes(
                    y[8 * i..8 * i + 8]
                        .try_into()
                        .expect("coordinate slice has fixed eight-byte length"),
                );
            }
        }
        for (slot, word) in self.0[..15].iter().zip(words) {
            slot.store(word, Ordering::Relaxed);
        }
        self.0[15].store(words[15] | READY, Ordering::Release);
    }
}

impl Clone for SegmentPoints {
    fn clone(&self) -> Self {
        let clone = Self::new();
        if let Some(points) = self.load() {
            clone.store(&points);
        }
        clone
    }
}

/// Odd multiples of `2^86 B` and `2^172 B`; segment 0 is
/// `BASEPOINT_ODD_LOOKUP_TABLE`. Held inline in the static, not on the heap.
static BASEPOINT_SEGMENTS: OnceLock<[NafLookupTable8<CachedPoint>; SEGMENTS - 1]> = OnceLock::new();

/// Builds the process-wide basepoint tables ahead of the first verification.
pub(super) fn prepare_basepoint() {
    if BASEPOINT_SEGMENTS.get().is_none() && std::is_x86_feature_detected!("avx2") {
        // SAFETY: Runtime detection includes the OS support required for AVX2.
        unsafe { basepoint_avx2() };
    }
}

#[target_feature(enable = "avx2")]
unsafe fn basepoint_avx2() -> &'static [NafLookupTable8<CachedPoint>; SEGMENTS - 1] {
    // The closure inherits this function's AVX2 target feature, so the
    // tables are built only by the thread that initializes the cell.
    BASEPOINT_SEGMENTS.get_or_init(|| basepoint_segments_avx2())
}

// Table construction runs once per process, and the key's shifted points
// once per key, so these helpers stay out of line to keep the inlined point
// arithmetic out of the binary.
#[target_feature(enable = "avx2")]
#[inline(never)]
fn basepoint_segments_avx2() -> [NafLookupTable8<CachedPoint>; SEGMENTS - 1] {
    let mut one = [0; 32];
    one[0] = 1;
    let mut segment =
        ExtendedPoint::from(EdwardsPoint::mul_base(&Scalar::from_bytes_mod_order(one)));
    core::array::from_fn(|_| {
        segment = shift_segment(segment);
        let mut multiples = [CachedPoint::from(segment); 64];
        odd_multiples(segment, &mut multiples);
        NafLookupTable8(multiples)
    })
}

#[target_feature(enable = "avx2")]
#[inline(never)]
fn shifted_points(point: &EdwardsPoint) -> [EdwardsPoint; SEGMENTS - 1] {
    let mut segment = ExtendedPoint::from(*point);
    core::array::from_fn(|_| {
        segment = shift_segment(segment);
        EdwardsPoint::from(segment)
    })
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

/// Computes `a A + b B`, filling the shifted points of `A` on second use, or
/// returns `None` without AVX2.
pub(super) fn mul_prepared(
    a: &Scalar,
    point: &EdwardsPoint,
    segments: &SegmentPoints,
    b: &Scalar,
) -> Option<EdwardsPoint> {
    if !std::is_x86_feature_detected!("avx2") {
        return None;
    }
    // SAFETY: Runtime detection includes the OS support required for AVX2.
    Some(unsafe { mul_prepared_avx2(a, point, segments, b) })
}

#[target_feature(enable = "avx2")]
unsafe fn mul_prepared_avx2(
    a: &Scalar,
    point: &EdwardsPoint,
    segments: &SegmentPoints,
    b: &Scalar,
) -> EdwardsPoint {
    // SAFETY: This function only runs with AVX2 enabled.
    let basepoint = unsafe { basepoint_avx2() };
    let shifted = match segments.load() {
        Some(shifted) => shifted,
        // SAFETY: This function only runs with AVX2 enabled.
        None if !segments.mark_seen() => return unsafe { mul_avx2(a, point, b) },
        None => {
            let shifted = shifted_points(point);
            segments.store(&shifted);
            shifted
        }
    };
    let tables = [point, &shifted[0], &shifted[1]].map(|point| {
        let first = ExtendedPoint::from(*point);
        let mut multiples = [CachedPoint::from(first); 8];
        odd_multiples(first, &mut multiples);
        NafLookupTable5(multiples)
    });
    // Both NAFs are at most 254 digits; the padding keeps the last segment's
    // indices in bounds.
    let mut a_naf = [0; SEGMENTS * SEGMENT_BITS];
    a_naf[..256].copy_from_slice(&a.non_adjacent_form(5));
    let mut b_naf = [0; SEGMENTS * SEGMENT_BITS];
    b_naf[..256].copy_from_slice(&b.non_adjacent_form(8));
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
                result = &result + &signed_multiple(&tables[j].0, digit);
            }
            let digit = b_naf[j * SEGMENT_BITS + i];
            if digit != 0 {
                let table = match j {
                    0 => &constants::BASEPOINT_ODD_LOOKUP_TABLE,
                    _ => &basepoint[j - 1],
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
                let segments = SegmentPoints::new();
                let serial = super::super::vartime::mul_serial(&a, &point, &b);
                // The first call runs unsegmented, the second fills the
                // shifted points, and later calls and clones load them.
                for call in 0..4 {
                    let clone;
                    let segments = if call == 3 {
                        clone = segments.clone();
                        assert!(clone.load().is_some());
                        &clone
                    } else {
                        &segments
                    };
                    let segmented = mul_prepared(&a, &point, segments, &b).unwrap();
                    assert_eq!(
                        segmented.compress().as_bytes(),
                        serial.compress().as_bytes()
                    );
                }
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

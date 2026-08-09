use hkdf::Hkdf;
use hmac::digest::KeyInit;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use std::sync::LazyLock;

/// HKDF-extract with no salt keys HMAC with a zero block, identical for every
/// input, so the key-schedule compressions run once and each derivation
/// clones the keyed state.
static EXTRACT_HMAC: LazyLock<Hmac<Sha256>> =
    LazyLock::new(|| Hmac::<Sha256>::new_from_slice(&[0u8; 32]).expect("32-byte HMAC key"));

#[derive(Clone, Debug)]
pub struct LTHash {
    pub hkdf_info: &'static [u8],
    pub hkdf_size: u8,
}

pub const WAPATCH_INTEGRITY_INFO: &str = "WhatsApp Patch Integrity";
pub const WAPATCH_INTEGRITY: LTHash = LTHash {
    hkdf_info: WAPATCH_INTEGRITY_INFO.as_bytes(),
    hkdf_size: 128,
};

impl LTHash {
    pub fn subtract_then_add<S: AsRef<[u8]>, A: AsRef<[u8]>>(
        &self,
        base: &[u8],
        subtract: &[S],
        add: &[A],
    ) -> Vec<u8> {
        let mut output = base.to_vec();
        self.subtract_then_add_in_place(&mut output, subtract, add);
        output
    }

    pub fn subtract_then_add_in_place<S: AsRef<[u8]>, A: AsRef<[u8]>>(
        &self,
        base: &mut [u8],
        subtract: &[S],
        add: &[A],
    ) {
        self.multiple_op(base, subtract, true);
        self.multiple_op(base, add, false);
    }

    fn multiple_op<T: AsRef<[u8]>>(&self, base: &mut [u8], input: &[T], subtract: bool) {
        // Reuse one stack buffer instead of a per-operand HKDF heap alloc.
        let mut derived = [0u8; u8::MAX as usize + 1];
        let derived = &mut derived[..self.hkdf_size as usize];
        for item in input {
            hkdf_sha256_into(item.as_ref(), self.hkdf_info, derived);
            perform_pointwise_with_overflow(base, derived, subtract);
        }
    }
}

/// Deliberately scalar. A hand-vectorised version of this loop lived here
/// until it was measured: over an 812-MAC batch it moved the total by 0.28%,
/// because HKDF above it dominates and LLVM already auto-vectorizes this loop
/// about as well as the intrinsics did.
fn perform_pointwise_with_overflow(base: &mut [u8], input: &[u8], subtract: bool) {
    assert_eq!(base.len(), input.len(), "length mismatch");
    assert!(base.len().is_multiple_of(2), "slice lengths must be even");

    // WA Web treats the accumulator as little-endian u16 lanes
    // (`new DataView(...).getUint16(off, true)` in WA/Crypto/LtHash.js).
    // Snapshot/patch MACs are HMACs over the accumulator bytes, so the lane
    // endianness is part of the wire spec.
    for (base_pair, input_pair) in base.chunks_exact_mut(2).zip(input.chunks_exact(2)) {
        let x = u16::from_le_bytes([base_pair[0], base_pair[1]]);
        let y = u16::from_le_bytes([input_pair[0], input_pair[1]]);

        let result = if subtract {
            x.wrapping_sub(y)
        } else {
            x.wrapping_add(y)
        };
        base_pair.copy_from_slice(&result.to_le_bytes());
    }
}

fn hkdf_sha256_into(key: &[u8], info: &[u8], out: &mut [u8]) {
    let mut extract = EXTRACT_HMAC.clone();
    extract.update(key);
    let prk = extract.finalize().into_bytes();
    let hk = Hkdf::<Sha256>::from_prk(&prk).expect("PRK is hash-sized");
    hk.expand(info, out).expect("hkdf expand");
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMPTY: &[Vec<u8>] = &[];

    /// The pre-keyed extract must stay byte-identical to plain
    /// `Hkdf::new(None, key)`; any drift would corrupt every ltHash.
    #[test]
    fn pre_keyed_extract_matches_plain_hkdf() {
        let mut keys: Vec<Vec<u8>> = (0..16u8).map(|i| vec![i.wrapping_mul(17); 32]).collect();
        keys.push(Vec::new());
        keys.push(vec![0xAB; 3]);

        let mut ours = [0u8; 128];
        let mut reference = [0u8; 128];
        for key in &keys {
            hkdf_sha256_into(key, WAPATCH_INTEGRITY_INFO.as_bytes(), &mut ours);
            Hkdf::<Sha256>::new(None, key)
                .expand(WAPATCH_INTEGRITY_INFO.as_bytes(), &mut reference)
                .expect("hkdf expand");
            assert_eq!(ours, reference);
        }
    }

    #[test]
    fn pointwise_add_and_subtract() {
        let mut base = vec![0u8; 128];
        let item = vec![1u8, 2, 3];
        let lth = WAPATCH_INTEGRITY;
        lth.subtract_then_add_in_place(&mut base, EMPTY, std::slice::from_ref(&item));
        let after_add = base.clone();
        assert_ne!(after_add, vec![0u8; 128]);
        lth.subtract_then_add_in_place(&mut base, &[item], EMPTY);
        assert_eq!(base, vec![0u8; 128]);
    }

    #[test]
    fn add_then_subtract_returns_to_zero_across_sizes() {
        let test_sizes = [2, 4, 8, 16, 18, 32, 64, 128, 256];

        for &size in &test_sizes {
            let mut base = vec![0u8; size];
            let input = vec![1u8; size];

            perform_pointwise_with_overflow(&mut base, &input, false);
            perform_pointwise_with_overflow(&mut base, &input, true);
            assert_eq!(base, vec![0u8; size], "size {size}");
        }
    }

    /// Reference for the test below. It reaches the same answer by a
    /// different route than the implementation: lanes are assembled by hand
    /// from byte positions and the arithmetic is done in `u32` and masked, so
    /// it shares neither `from_le_bytes` nor `wrapping_*` with the code under
    /// test. A reference that mirrors the implementation proves nothing.
    fn reference_pointwise(base: &mut [u8], input: &[u8], subtract: bool) {
        for i in (0..base.len()).step_by(2) {
            let x = base[i] as u32 | ((base[i + 1] as u32) << 8);
            let y = input[i] as u32 | ((input[i + 1] as u32) << 8);
            let r = if subtract {
                x.wrapping_sub(y) & 0xFFFF
            } else {
                (x + y) & 0xFFFF
            };
            base[i] = (r & 0xFF) as u8;
            base[i + 1] = (r >> 8) as u8;
        }
    }

    /// Sizes straddle the 16-byte boundary a vectorised implementation would
    /// chunk on, so the coverage still holds if one ever comes back. Inputs
    /// are seeded onto the wrap boundaries in both directions, which is where
    /// a lane-width or endianness mistake shows up rather than in round data.
    #[test]
    fn pointwise_matches_independent_reference() {
        let sizes = [0usize, 2, 14, 16, 18, 32, 34, 128, 130, 256];
        // Deterministic LCG: reproducible failures, no dev-dependency.
        let mut seed = 0x2545_F491u32;
        let mut next = move || {
            seed = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            (seed >> 24) as u8
        };

        for size in sizes {
            for subtract in [false, true] {
                for edge in [0u8, 0xFF, 0x01] {
                    let base: Vec<u8> = (0..size)
                        .map(|i| if i % 3 == 0 { edge } else { next() })
                        .collect();
                    let input: Vec<u8> = (0..size)
                        .map(|i| if i % 5 == 0 { edge } else { next() })
                        .collect();

                    let mut actual = base.clone();
                    let mut expected = base.clone();
                    perform_pointwise_with_overflow(&mut actual, &input, subtract);
                    reference_pointwise(&mut expected, &input, subtract);

                    assert_eq!(
                        actual, expected,
                        "size {size}, subtract {subtract}, edge {edge:#04x}"
                    );
                }
            }
        }
    }

    #[test]
    fn test_overflow_underflow() {
        let mut base = vec![255u8, 255, 0, 0];
        let input = vec![1u8, 0, 1, 0];

        perform_pointwise_with_overflow(&mut base, &input, false);
        assert_eq!(base, vec![0, 0, 1, 0]);

        perform_pointwise_with_overflow(&mut base, &input, true);
        assert_eq!(base, vec![255, 255, 0, 0]);
    }

    #[test]
    fn test_multiple_operations() {
        let mut base = vec![0u8; 128];
        let lth = WAPATCH_INTEGRITY;

        let items = vec![
            vec![1u8, 2, 3, 4],
            vec![5u8, 6, 7, 8],
            vec![9u8, 10, 11, 12],
        ];

        lth.subtract_then_add_in_place(&mut base, EMPTY, &items);
        let after_add = base.clone();
        assert_ne!(after_add, vec![0u8; 128]);

        let mut reverse_items = items.clone();
        reverse_items.reverse();
        lth.subtract_then_add_in_place(&mut base, &reverse_items, EMPTY);
        assert_eq!(base, vec![0u8; 128]);
    }

    #[test]
    fn test_different_buffer_sizes() {
        let lth = WAPATCH_INTEGRITY;

        let base = vec![0u8; 128];
        let items = vec![vec![42u8; 1], vec![42u8; 10], vec![42u8; 32]];

        for item in items {
            let mut test_base = base.clone();
            lth.subtract_then_add_in_place(&mut test_base, EMPTY, std::slice::from_ref(&item));
            assert_ne!(test_base, vec![0u8; 128]);

            lth.subtract_then_add_in_place(&mut test_base, &[item], EMPTY);
            assert_eq!(test_base, vec![0u8; 128]);
        }
    }

    #[test]
    fn test_round_trip_complex() {
        let mut base = vec![100u8; 128];
        let original = base.clone();
        let lth = WAPATCH_INTEGRITY;

        let add_items = vec![vec![1u8, 2, 3], vec![4u8, 5], vec![6u8, 7, 8, 9]];

        let subtract_items = vec![vec![1u8, 2, 3], vec![4u8, 5], vec![6u8, 7, 8, 9]];

        lth.subtract_then_add_in_place(&mut base, EMPTY, &add_items);
        assert_ne!(base, original);

        lth.subtract_then_add_in_place(&mut base, &subtract_items, EMPTY);
        assert_eq!(base, original);
    }

    #[test]
    fn test_empty_operations() {
        let mut base = vec![42u8; 128];
        let original = base.clone();
        let lth = WAPATCH_INTEGRITY;

        lth.subtract_then_add_in_place::<Vec<u8>, Vec<u8>>(&mut base, &[], &[]);
        assert_eq!(base, original);
    }

    #[test]
    fn test_single_byte_operations() {
        let mut base = vec![0u8; 2];
        let input = vec![255u8, 254];

        perform_pointwise_with_overflow(&mut base, &input, false);
        assert_eq!(base, vec![255, 254]);

        perform_pointwise_with_overflow(&mut base, &input, true);
        assert_eq!(base, vec![0, 0]);
    }
}

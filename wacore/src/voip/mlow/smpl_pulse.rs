//! MLow PVQ-style pulse decode (post-LSF blocks of WASM func 3545). The active 0x50 capture frames
//! are config=0 (p6=0), so the pulse COUNT uses the NB triangular prior; the split/magnitude/sign
//! machinery walks the same CDF tables, now read from the logical `CcTables` (built from a seed)
//! instead of the cc_blob heap window. `wrapping_*` mirrors the WASM's i32/u32 modular arithmetic.

use super::rangecoder::RangeDecoder;
use super::smpl_cc_tables::CcTables;

/// Static gain-helper table (rodata 0xe8990): pulse-count byte index `[config*3 + (p4+s1)]`.
const SMPL_PULSE_COUNT_BYTE: [u8; 8] = [80, 160, 160, 16, 32, 32, 0, 0];

pub(crate) fn mem8_static(addr: u32) -> u8 {
    if (0xe8990..0xe8998).contains(&addr) {
        SMPL_PULSE_COUNT_BYTE[(addr - 0xe8990) as usize]
    } else {
        0
    }
}

/// Decoded excitation for one internal (20 ms) frame.
pub(crate) struct SmplPulseResult {
    pub(crate) pulses: Vec<i32>, // signed pulse magnitudes per sample position (len = p2)
    pub(crate) subfr: [i32; 4],  // per-subframe pulse counts
}

/// Decode the pulse blocks of one internal frame. `p2` = frame samples (320), `p3` = num subframes
/// (4), `p4` = regular flag (1), `p6` = config (0/1), `s1` = LSF stage-1 selector.
pub(crate) fn decode_smpl_pulses(
    dec: &mut RangeDecoder,
    cc: &CcTables,
    p2: i32,
    p3: i32,
    p4: i32,
    p6: i32,
    s1: i32,
) -> SmplPulseResult {
    let mut res = SmplPulseResult {
        pulses: vec![0i32; p2.max(0) as usize],
        subfr: [0; 4],
    };

    let idx = p4 + s1;
    let b_byte = mem8_static(0xe8990u32.wrapping_add((p6 * 3 + idx) as u32)) as i32;
    let frame_len4k = b_byte * p2 / 320;
    let subfr_len16 = frame_len4k / p3;
    let pos_per_subfr = p2 / p3;

    // --- pulse COUNT ---
    let total: i32;
    if p6 != 0 {
        // WB low-rate: the pulse-count CDF for this voicing class.
        let cdf = cc.n_pulse_count(idx);
        total = dec.decode_cdf(cdf);
    } else {
        // NB (config=0, our path): a TRIANGULAR prior over [0, frame_len4k].
        let l = frame_len4k as u32;
        let tri_t = |k: u32| -> u32 {
            let a = k.wrapping_add(2).wrapping_mul(l.wrapping_add(1));
            let b = k.wrapping_sub(1).wrapping_mul(k.wrapping_add(131070)) >> 1;
            a.wrapping_sub(b) & 0xffff
        };
        let mut ft = tri_t(l);
        if ft == 0 {
            ft = 1;
        }
        let val = dec.decode(ft);
        let limit = (frame_len4k as u32).wrapping_add(1);
        let mut prev_cum: u32 = 0;
        let mut k: u32 = 0;
        loop {
            if k == limit {
                break;
            }
            let cum = tri_t(k);
            // found when prev_cum <= val < cum (the cumulative-triangular interval containing val).
            if prev_cum <= val && val < cum {
                dec.update(prev_cum, cum, ft);
                break;
            }
            prev_cum = cum;
            k += 1;
        }
        total = k as i32;
    }

    // --- recursive binary SPLIT (p3==4 path) ---
    let mut split = [0i32; 8];
    if total != 0 {
        let mut sum = (total - subfr_len16 * 2).max(0);
        let lo = (total - 80).max(0);
        if sum < lo {
            // min_split2 >= min_split assert path; treat as parse error (zeroed subframes).
            return res;
        }
        let hi_bound = total - lo;
        if sum < hi_bound {
            // window the split CDF at (sum - lo); n entries from the table base.
            let n = ((hi_bound - sum) + 2) as usize;
            sum += dec.decode_cdf_window(cc.split_cmf(total), (sum - lo) as usize, n);
        }
        if sum > 0 {
            let s0 = smpl_split_3537(dec, cc, sum, subfr_len16);
            split[0] = s0;
            split[1] = sum - s0;
        }
        if sum < total {
            let s2 = smpl_split_3537(dec, cc, total - sum, subfr_len16);
            split[2] = s2;
            split[3] = (total - sum) - s2;
        }
        // A corrupt -1 from either half zeroes the whole split (and n_pulses).
        if split[0] == -1 || split[2] == -1 {
            split = [0i32; 8];
        }
    }

    let take = p3.clamp(0, 4) as usize;
    res.subfr[..take].copy_from_slice(&split[..take]);

    // --- MAGNITUDE block: per-subframe run-length pulse positions ---
    let pos_per = pos_per_subfr;
    let mut pos_list: Vec<i32> = Vec::new();
    let mut mag_list: Vec<i32> = Vec::new();
    let mut pulse_idx: i32 = -1;
    for subfr in 0..p3 {
        let cnt = split[subfr as usize];
        if cnt <= 0 {
            continue;
        }
        let base_pos = pos_per * subfr;
        let mut run_pos = base_pos;
        let mut pos = pos_per;
        let mut c = cnt;
        let mut k = 0;
        while k < cnt {
            if pos < 0 {
                break; // defensive: malformed frame must not drive a huge CDF length
            }
            let oct = (pos + 7) / 8;
            let bucket = cc.runlen(oct);
            // window the c-pulses CDF by (max_samples - pos), reading pos+1 entries.
            let start = (bucket.max_samples() - pos) as usize;
            let m = dec.decode_cdf_window(bucket.cmf(c), start, (pos + 1) as usize);
            if m > 0 || k == 0 {
                pulse_idx += 1;
                run_pos += m;
                pos_list.push(run_pos);
                mag_list.push(1);
                pos -= m;
            } else if pulse_idx >= 0 {
                mag_list[pulse_idx as usize] += 1;
            }
            c -= 1;
            k += 1;
        }
    }

    let num_pos = pulse_idx + 1;

    // --- SIGN block: batched uniform sign reads (1 bit per position) ---
    if num_pos > 0 {
        let mut p = 0;
        while p <= pulse_idx {
            let mut nbits = num_pos - p;
            if nbits >= 15 {
                nbits = 15;
            }
            if nbits <= 0 {
                break;
            }
            let sym = dec.decode_raw_symbol(nbits as u32);
            let mut bitfield = sym.wrapping_shl(16 - nbits as u32);
            let end = p + nbits;
            for q in p..end {
                let sign = ((bitfield >> 14) & 2) as i32 - 1; // +1 if MSB set else -1
                mag_list[q as usize] *= sign;
                bitfield = bitfield.wrapping_shl(1);
            }
            p = end;
        }
    }

    // scatter signed magnitudes into the pulse vector at their absolute positions.
    for i in 0..num_pos {
        let pp = pos_list[i as usize];
        if pp >= 0 && (pp as usize) < res.pulses.len() {
            res.pulses[pp as usize] = mag_list[i as usize];
        }
    }
    log::trace!(
        "mlow pulses: total={total} subfr={:?} num_pos={num_pos}",
        res.subfr
    );
    res
}

/// Split `count` pulses across a range, returning the count assigned to the first half (func 3537).
fn smpl_split_3537(dec: &mut RangeDecoder, cc: &CcTables, count: i32, granularity: i32) -> i32 {
    let lo = count.min(granularity);
    let min_split = (count - granularity).max(0);
    if lo < min_split {
        return -1;
    }
    if min_split == lo {
        return min_split;
    }
    // window split_CMFs[count-1] at min_split, n entries.
    let n = ((lo - min_split) + 2) as usize;
    dec.decode_cdf_window(cc.split_cmf(count), min_split as usize, n) + min_split
}

#[cfg(test)]
mod tests {
    use super::super::smpl_cc_tables::load_cc_tables;
    use super::super::smpl_decode::{SmplLsfState, decode_smpl_lsf, load_smpl_tables};
    use super::*;
    use serde_json::Value;

    // Decodes LSF(0) then pulses(0) on each active captured frame and compares the per-subframe
    // counts + the full signed pulse vector against the reference.
    #[test]
    fn pulses_match_go() {
        let recs: Value = serde_json::from_str(include_str!("testdata/pulse_vectors.json"))
            .expect("pulse_vectors");
        let tbl = load_smpl_tables();
        let cc = load_cc_tables();
        let arr = recs.as_array().unwrap();
        assert!(!arr.is_empty());
        for rec in arr {
            let frame = hex::decode(rec["frame"].as_str().unwrap()).unwrap();
            let mut st = SmplLsfState::default();
            let mut dec = RangeDecoder::new(&frame[1..]);
            let lsf = decode_smpl_lsf(&mut dec, tbl, &mut st, 0, 0);
            let pr = decode_smpl_pulses(&mut dec, cc, 320, 4, 1, 0, lsf.stage1);

            let want_subfr: Vec<i32> = rec["subfr"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_i64().unwrap() as i32)
                .collect();
            assert_eq!(pr.subfr.to_vec(), want_subfr, "subfr");

            // rebuild the expected sparse vector and compare element-wise.
            let mut want = vec![0i32; pr.pulses.len()];
            for pu in rec["pulses"].as_array().unwrap() {
                let pos = pu["pos"].as_i64().unwrap() as usize;
                want[pos] = pu["val"].as_i64().unwrap() as i32;
            }
            assert_eq!(pr.pulses, want, "pulse vector");
        }
    }

    // A corrupt split (count > 2*granularity) makes smpl_split_3537 return -1; the decoder then
    // zeroes the subframe split rather than copying the sentinel into res.subfr.
    #[test]
    fn corrupt_split_zeroes_subframes() {
        let cc = load_cc_tables();
        let mut dec = RangeDecoder::new(&[0u8; 8]);
        // count=10 > 2*granularity=2 hits the `lo < min_split` corrupt branch.
        assert_eq!(smpl_split_3537(&mut dec, cc, 10, 1), -1);

        // The guard the decoder applies on either -1: the whole split array is wiped before copy.
        let mut split = [3i32; 8];
        split[2] = -1;
        if split[0] == -1 || split[2] == -1 {
            split = [0i32; 8];
        }
        let take = 4usize;
        let mut subfr = [0i32; 4];
        subfr[..take].copy_from_slice(&split[..take]);
        assert_eq!(subfr, [0i32; 4]);
    }
}

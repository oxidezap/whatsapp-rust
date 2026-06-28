//! MLow pitch/LTP decode, run when the LSF stage-1 selector is 1 (voiced), which is every active
//! 0x50 capture frame. Two parts: LTP gains (per-subframe gain index + optional 35-sym filter index,
//! served from the logical `CcTables`) and the lag block (primary lag absolute/delta, contour-map
//! search, optional 64-sym fine read, per-segment fractional loop) which reads the contour window
//! built from the pitch seed.

use super::rangecoder::RangeDecoder;
use super::smpl_cc_tables::CcTables;
use super::smpl_decode::SmplLsfState;
use super::smpl_mem::SmplMem;

/// Decoded LTP/pitch parameters for one internal frame.
#[derive(Default)]
pub(crate) struct SmplPitchResult {
    pub(crate) gain_idx: [i32; 4],
    pub(crate) filt_idx: [i32; 4],
    pub(crate) lag: i32,
    pub(crate) contour: i32,
    /// Per-segment reconstructed pitch lag in Q6 (1/64-sample).
    pub(crate) sample_lag_q6: [i32; 8],
    pub(crate) num_seg: i32,
    /// Per-subframe pitch lag in Q6 (the LTP lag table). Kept at 4 entries to match the
    /// `pitch_match_go` vectors; the 8 per-40-block lags the ACB/LTP synthesis actually consumes live
    /// in `block_lags`.
    pub(crate) int_lag_q6: [i32; 4],
    /// The per-40-sample-block `laginds` (8 per 20 ms frame), in the same index domain as
    /// `int_lag_q6`. A single 80-sample subframe spans two blocks that can carry different fractional
    /// lags; synthesis maps each as `lag = block_lags*0.5 + SMPL_MIN_PITCH_LAG`.
    pub(crate) block_lags: [i32; 8],
    pub(crate) num_subfr: i32,
}

/// `decode_cdf` on the n-entry CDF at heap address `addr`. Reads the region bytes in place when the
/// window fits one region (the common case), falling back to the zero-filling `cdf_at` copy only at
/// a region boundary, where the out-of-range entries must read 0.
#[inline]
fn decode_cdf_mem(dec: &mut RangeDecoder, mem: &SmplMem, addr: u32, n: usize) -> i32 {
    match mem.cdf_bytes(addr, n) {
        Some(bytes) => dec.decode_cdf_le16(bytes),
        None => dec.decode_cdf(&mem.cdf_at(addr, n)),
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn decode_smpl_pitch(
    dec: &mut RangeDecoder,
    mem: &SmplMem,
    cc: &CcTables,
    st: &mut SmplLsfState,
    _p2: i32,
    p3: i32,
    p6: i32,
    subfr_counts: [i32; 4],
) -> SmplPitchResult {
    let mut res = SmplPitchResult {
        filt_idx: [-1; 4],
        ..Default::default()
    };
    // LTP gains loop: both selects key on p6 (active WB path is p6==0 HR, the p6!=0 LR variant), both
    // served from the logical `CcTables`. The filter CDFs are shared across p6.
    let mut gain_accum: i32 = 0;
    for (sf, &cnt) in subfr_counts.iter().enumerate().take((p3 as usize).min(4)) {
        let gi = if p6 != 0 {
            dec.decode_cdf(cc.acbgain_row_lr(st.prev_gain_idx))
        } else {
            dec.decode_cdf(cc.acbgain_row(st.prev_gain_idx))
        };
        res.gain_idx[sf] = gi;
        st.prev_gain_idx = gi;

        let (w0, w2) = if p6 != 0 {
            cc.acbgain_weights_lr(gi)
        } else {
            cc.acbgain_weights(gi)
        };
        gain_accum += w0 + 2 * w2;

        if cnt > 0 {
            let fi = if st.prev_filt_idx == -1 {
                dec.decode_cdf(cc.fcbgain_v())
            } else {
                dec.decode_cdf(cc.fcbgain_v_delta(st.prev_filt_idx))
            };
            res.filt_idx[sf] = fi;
            st.prev_filt_idx = fi;
        }
    }
    let avg_gain = gain_accum / p3; // drives the fractional-lag segment select

    // --- Lag block ---
    let pcfg = mem.g_clk.wrapping_add(0x5704);
    let num_contours = mem.u32(pcfg.wrapping_add(22240)) as i32;
    let lag_cdf = mem.u32(pcfg.wrapping_add(22248));
    let contour_map = mem.u32(pcfg.wrapping_add(22244));
    let frac_base = mem.u32(pcfg.wrapping_add(22252));
    let delta_cdf = mem.u32(pcfg.wrapping_add(22268));

    // primary lag:
    let lag: i32 = if st.prev_lag < 0 {
        decode_cdf_mem(dec, mem, lag_cdf, (num_contours + 1).max(0) as usize)
    } else {
        let di = decode_cdf_mem(
            dec,
            mem,
            delta_cdf.wrapping_add((st.prev_lag as u32) * 20),
            10,
        );
        let lo = mem.u8(0xe7ef0u32.wrapping_add((di as u32) * 2)) as i32;
        let hi = mem.u8(0xe7ef0u32.wrapping_add((di as u32) * 2 + 1)) as i32;
        let r_n = (hi - lo) + 2;
        if r_n < 2 {
            res.lag = -1;
            return res; // malformed delta interval
        }
        let sym = decode_cdf_mem(
            dec,
            mem,
            lag_cdf.wrapping_add((lo as u32) * 2),
            r_n as usize,
        );
        sym + lo
    };

    // contour-map search: find index where contour_map[i] == lag+1.
    let target = lag + 1;
    let mut contour: i32 = -1;
    for i in 0..217 {
        if mem.u8(contour_map.wrapping_add(i as u32)) as i32 == target {
            contour = i;
            break;
        }
    }
    res.lag = lag;
    res.contour = contour;
    if contour < 0 || contour >= num_contours {
        return res; // out-of-range; stop consuming pitch bits
    }

    let ctr_base = pcfg.wrapping_add((contour as u32).wrapping_mul(0x44));
    let base_lag = mem.i32(ctr_base.wrapping_add(0x1d38)); // contour base lag

    // (a) 64-symbol fine lag; read UNLESS prev_lag>=0 && -1 <= (base_lag-prev_lag) < 3.
    let mut cur_lag2 = base_lag;
    let mut read_fine = true;
    if st.prev_lag >= 0 {
        let delta = base_lag - st.prev_lag;
        if (-1..3).contains(&delta) {
            read_fine = false;
        }
    }
    let mut subfr_w: i32 = 0;
    if read_fine {
        let sym = dec.decode_64_fine_sym();
        cur_lag2 = (base_lag << 6) + sym;
        st.prev_frac_lag = cur_lag2;
        st.prev_lag = base_lag;
        let seg_len0 = mem.i32(ctr_base.wrapping_add(0x1d58));
        for _ in 0..seg_len0 {
            if (subfr_w as usize) < 4 {
                res.int_lag_q6[subfr_w as usize] = cur_lag2;
            }
            if (subfr_w as usize) < 8 {
                res.block_lags[subfr_w as usize] = cur_lag2;
            }
            subfr_w += 1;
        }
        if (subfr_w as usize) < 4 {
            res.int_lag_q6[subfr_w as usize] = cur_lag2; // trailing write, subfr_w not incremented
        }
        if (subfr_w as usize) < 8 {
            res.block_lags[subfr_w as usize] = cur_lag2;
        }
    }

    // (b) fractional per-segment loop:
    let cnt2 = mem.i32(ctr_base.wrapping_add(0x1d78));
    let seg_sel = if avg_gain >= 10007 {
        if avg_gain < 14085 { 1 } else { 2 }
    } else {
        0
    };
    let frac_seg_base = frac_base.wrapping_add((seg_sel as u32) * 0x280);
    let mut l3 = st.prev_frac_lag;
    let mut l2 = cur_lag2;
    let start_seg = if read_fine { 1 } else { 0 };
    res.num_seg = cnt2;
    for seg in start_seg..cnt2 {
        let seg_lag = mem.i32(ctr_base.wrapping_add(0x1d38).wrapping_add((seg as u32) * 4));
        let nl2 = ((l2 << 6) - l3) + ((seg_lag - l2) << 6);
        let off = frac_seg_base
            .wrapping_add((nl2 * 2) as u32)
            .wrapping_add(0xfe);
        let sym = decode_cdf_mem(dec, mem, off, 65);
        l3 = sym + st.prev_frac_lag + nl2;
        if (seg as usize) < 8 {
            res.sample_lag_q6[seg as usize] = l3;
        }
        let seg_len = mem.i32(ctr_base.wrapping_add(0x1d58).wrapping_add((seg as u32) * 4));
        for _ in 0..seg_len {
            if (subfr_w as usize) < 4 {
                res.int_lag_q6[subfr_w as usize] = l3;
            }
            if (subfr_w as usize) < 8 {
                res.block_lags[subfr_w as usize] = l3;
            }
            subfr_w += 1;
        }
        l2 = seg_lag;
        st.prev_frac_lag = l3;
        st.prev_lag = seg_lag;
    }
    res.num_subfr = subfr_w;
    log::trace!(
        "mlow pitch: lag={lag} contour={contour} avg_gain={avg_gain} int_lag_q6={:?} num_subfr={subfr_w}",
        res.int_lag_q6
    );
    res
}

#[cfg(test)]
mod tests {
    use super::super::smpl_cc_tables::load_cc_tables;
    use super::super::smpl_decode::{SmplLsfState, decode_smpl_lsf, load_smpl_tables};
    use super::super::smpl_mem::load_smpl_mem;
    use super::super::smpl_pulse::decode_smpl_pulses;
    use super::*;
    use serde_json::Value;

    // Decodes LSF(0) -> pulses(0) -> pitch(0) on each active captured frame and compares the pitch
    // result (gains, filters, lag, contour, per-subframe int_lag_q6) against the reference vectors.
    #[test]
    fn pitch_match_go() {
        let recs: Value = serde_json::from_str(include_str!("testdata/pitch_vectors.json"))
            .expect("pitch_vectors");
        let tbl = load_smpl_tables();
        let mem = load_smpl_mem();
        let cc = load_cc_tables();
        let arr = recs.as_array().unwrap();
        assert!(!arr.is_empty());
        let as_i32 = |v: &Value| -> Vec<i32> {
            v.as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_i64().unwrap() as i32)
                .collect()
        };
        for rec in arr {
            let frame = hex::decode(rec["frame"].as_str().unwrap()).unwrap();
            let mut st = SmplLsfState::default();
            let mut dec = RangeDecoder::new(&frame[1..]);
            let lsf = decode_smpl_lsf(&mut dec, tbl, &mut st, 0, 0);
            let pulses = decode_smpl_pulses(&mut dec, cc, 320, 4, 1, 0, lsf.stage1);
            let pr = decode_smpl_pitch(&mut dec, mem, cc, &mut st, 320, 4, 0, pulses.subfr);

            assert_eq!(pr.lag, rec["lag"].as_i64().unwrap() as i32, "lag");
            assert_eq!(
                pr.contour,
                rec["contour"].as_i64().unwrap() as i32,
                "contour"
            );
            assert_eq!(pr.gain_idx.to_vec(), as_i32(&rec["gain_idx"]), "gain_idx");
            assert_eq!(pr.filt_idx.to_vec(), as_i32(&rec["filt_idx"]), "filt_idx");
            assert_eq!(
                pr.int_lag_q6.to_vec(),
                as_i32(&rec["int_lag_q6"]),
                "int_lag_q6"
            );
            assert_eq!(dec.err, 0, "no decode error");
        }
    }
}

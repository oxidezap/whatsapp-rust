//! MLow gains + nrgres decode (func 3545 GAINS block), run for UNVOICED (stage-1 selector == 0)
//! internal frames; mutually exclusive with the pitch block. The config=0 active capture is all
//! voiced, so this never runs on real audio (the synth uses gainQ=0 for voiced frames); the test
//! below validates the arithmetic byte-exact by force-running it on a voiced frame's post-pulse
//! decoder state.

use super::rangecoder::RangeDecoder;
use super::smpl_cc_tables::CcTables;

pub(crate) struct SmplGainResult {
    /// Per-subframe quantized log-gain (Q-domain).
    pub(crate) gain_q: [i32; 4],
    /// Per-subframe energy-residual symbol (only subframes with pulses are read).
    pub(crate) nrg_res: [i32; 4],
}

/// Two-subframe (low-rate) gain reconstruction codebook, indexed `sf + 2 * gain_delta`.
const GAIN_RECON_2: [i16; 46] = [
    -9672, 9672, -4437, 4437, -907, 907, 3610, -3610, -2857, 2857, 9345, -9345, -2220, 2220, 420,
    -420, -3565, 3565, -1577, 1577, -184, 184, 2973, -2973, 2389, -2389, 1774, -1774, -7315, 7315,
    4405, -4405, -5630, 5630, -12927, 12927, -17376, 17376, 1107, -1107, 7058, -7058, 5481, -5481,
    0, 0,
];
/// Energy step per main-gain index, by gain-table row.
const NRG_STEPS: [i32; 3] = [16384, 19843, 16686];

/// Decode the gains+nrgres reads for `p3` subframes (2 at low rate, 4 otherwise). `subfr_counts` are
/// the per-subframe pulse counts.
pub(crate) fn decode_smpl_gains(
    dec: &mut RangeDecoder,
    cc: &CcTables,
    p3: i32,
    subfr_counts: [i32; 4],
) -> SmplGainResult {
    let mut res = SmplGainResult {
        gain_q: [0; 4],
        nrg_res: [0; 4],
    };

    // main gain (n=85) + delta gain (n=99)
    let (gain_main, gain_delta) = if p3 == 2 {
        (
            dec.decode_cdf(cc.nrgres_gain2()),
            dec.decode_cdf(cc.nrgres_shape2()),
        )
    } else {
        (
            dec.decode_cdf(cc.nrgres_gain4()),
            dec.decode_cdf(cc.nrgres_shape4()),
        )
    };
    let cfg_sel = 2i32;

    // gain reconstruction. The index sf + p3*gain_delta is NOT bounded to the visible 64-entry array
    // (gain_delta up to 98); the WASM reads adjacent rodata, which the seed carries verbatim.
    let off6 = p3 * gain_delta;
    // Two subframes use the second energy step; four keep the configured one.
    let step = if p3 == 2 {
        NRG_STEPS[1]
    } else {
        cc.nrg_step(cfg_sel)
    };
    let base7 = gain_main * step - 0x154000;
    for (sf, gain) in res.gain_q.iter_mut().enumerate().take((p3 as usize).min(4)) {
        let cbv = if p3 == 2 {
            GAIN_RECON_2
                .get((sf as i32 + off6) as usize)
                .map_or(0, |v| i32::from(*v))
        } else {
            cc.gain_recon(p3 == 4, sf as i32 + off6)
        };
        *gain = base7 + (cbv << 4);
    }
    // Two subframes read their energy residuals from the second offset table.
    let table_ix = if p3 == 2 { 1 } else { cfg_sel as usize };

    // nrgres: per-subframe bucketed CDF (n=92) with a gain-derived slice shift.
    for (sf, &cnt) in subfr_counts.iter().enumerate().take((p3 as usize).min(4)) {
        if cnt <= 0 {
            continue;
        }
        let bucket = if cnt >= 30 { 3 } else { (cnt & 0xffff) / 10 };
        // g = clamp((gainQ[sf]+8192)>>14, floor -85); min_offset = -neg_part (forward entry shift).
        let mut g = (res.gain_q[sf] + 8192) >> 14;
        if g < -85 {
            g = -85;
        }
        let neg_part = (g >> 31) & g;
        let min_offset = (-neg_part) as usize;
        res.nrg_res[sf] = dec.decode_cdf(cc.fcbg_offset(table_ix, bucket as usize, min_offset));
    }
    log::trace!(
        "mlow gains: main={gain_main} delta={gain_delta} gain_q={:?} nrg_res={:?}",
        res.gain_q,
        res.nrg_res
    );
    res
}

#[cfg(test)]
mod tests {
    use super::super::smpl_cc_tables::load_cc_tables;
    use super::super::smpl_decode::{SmplLsfState, decode_smpl_lsf, load_smpl_tables};
    use super::super::smpl_pulse::decode_smpl_pulses;
    use super::*;
    use serde_json::Value;

    // Force-runs gains on each active frame's post-pulse decoder state (semantically a voiced frame,
    // so the bits aren't "really" gains, but the decode is deterministic and must match the reference
    // exactly, validating the gains arithmetic + memory reads byte-for-byte).
    #[test]
    fn gains_match_go() {
        let recs: Value = serde_json::from_str(include_str!("testdata/gains_vectors.json"))
            .expect("gains_vectors");
        let tbl = load_smpl_tables();
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
            let lsf = decode_smpl_lsf(&mut dec, tbl, &mut st, 0, 0, true);
            let pulses = decode_smpl_pulses(&mut dec, cc, 320, 4, 1, 0, lsf.stage1);
            let g = decode_smpl_gains(&mut dec, cc, 4, pulses.subfr);
            assert_eq!(g.gain_q.to_vec(), as_i32(&rec["gain_q"]), "gain_q");
            assert_eq!(g.nrg_res.to_vec(), as_i32(&rec["nrg_res"]), "nrg_res");
        }
    }
}

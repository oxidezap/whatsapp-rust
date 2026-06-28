//! MLow per-frame LSF parameter decode ("smpl_core_decode_indices", LSF block) plus the runtime CDF
//! tables it reads.
//!
//! The smpl LSF coding is Meta-specific (NOT stock SILK CB1): a 2-way stage-1 codebook *selector*,
//! a stage-1 *grid* index, then 16 stage-2 residuals from `g_lsf[stage1][config][grid][coeff]`. The
//! stage-2/gain CDFs are RUNTIME-built (not static rodata) and were captured into `smpl_tables.json`.
//! The entropy primitive is `decodeCDF` (u16 cumulative CDFs), not the ICDF path.

use super::rangecoder::RangeDecoder;

pub(crate) struct LsfGrid {
    pub(crate) match1: Vec<u16>,
    pub(crate) match1_alt: Vec<u16>,
    pub(crate) match0: Vec<u16>,
    pub(crate) match0_alt: Vec<u16>,
}

/// Runtime CDF tables (the LSF block of func 3559's output), expanded from the LSF seed ROM.
pub(crate) struct SmplTables {
    pub(crate) lsf_sel: Vec<Vec<u16>>,
    pub(crate) lsf_grid: LsfGrid,
    /// `[stage1][config][grid][coeff]` -> cumulative CDF.
    pub(crate) lsf_stage2: Vec<Vec<Vec<Vec<Vec<u16>>>>>,
    pub(crate) lsf_extra: Vec<u16>,
    // The gain CDFs the decoder uses come from the heap window (smpl_mem g_nrg), not these table
    // fields, so the JSON's gain_main/gain_delta are intentionally not deserialized here.
}

pub(crate) fn load_smpl_tables() -> &'static SmplTables {
    &super::smpl_lsf_seed::lsf_built().tables
}

/// Cross-internal-frame decoder state. The LSF block RESETS the pitch/LTP predictor fields to -1
/// whenever the stage-1 selector does not match the previous internal frame.
#[derive(Default, Clone)]
pub(crate) struct SmplLsfState {
    pub(crate) prev_stage1: i32,
    pub(crate) prev_match: bool,
    pub(crate) have_prev: bool,
    pub(crate) prev_gain_idx: i32,
    pub(crate) prev_filt_idx: i32,
    pub(crate) prev_lag: i32,
    pub(crate) prev_frac_lag: i32,
    /// Encoder-only: previous internal frame's chosen pitch lag (samples) for the pitch-search
    /// continuity bias. Unused by the decoder.
    pub(crate) prev_lag_samples: f32,
    /// Encoder-only: the `ParamsEncoder` lag predictor (`prev_lagblk`/`prev_lagidx`) that
    /// `smpl_encode_lags` threads to pick the abs-vs-block-transition first lag and the per-block
    /// delta-lag CMFs. Reset to -1 on a no-match (`cond_coding == FALSE`). Unused by the decoder
    /// (it rebuilds lags from the wire via its own contour predictor).
    pub(crate) prev_lagblk: i32,
    pub(crate) prev_lagidx: i32,
}

/// Advance the LSF predictor mirror exactly as `encode_smpl_lsf`/`decode_smpl_lsf` does for an
/// internal frame with the given stage-1 selector: on a no-match (intf 0, or stage1 differs) reset
/// the pitch/LTP predictor to -1, then record prev_stage1/prev_match. The encoder analysis runs this
/// so its `prev_lag` tracks what the entropy encoder will compute (driving the abs-vs-delta lag pick).
pub(crate) fn smpl_advance_lsf_state(st: &mut SmplLsfState, intf: usize, stage1: i32) {
    let m = intf != 0 && stage1 == st.prev_stage1;
    if !m {
        st.prev_gain_idx = -1;
        st.prev_filt_idx = -1;
        st.prev_lag = -1;
        st.prev_frac_lag = -1;
        st.prev_lagblk = -1;
        st.prev_lagidx = -1;
    }
    st.prev_stage1 = stage1;
    st.prev_match = m;
    st.have_prev = true;
}

/// Decoded per-internal-frame LSF index set.
pub(crate) struct SmplLsfIndices {
    pub(crate) stage1: i32,
    pub(crate) grid: i32,
    pub(crate) stage2: [i32; 16],
    pub(crate) extra: i32,
}

/// Decode the LSF block of one internal frame (the first block of the core decode). `config` is the
/// smpl config (0/1), `intf` the internal-frame index (0,1,2) within the 60 ms packet. Mutates `st`.
pub(crate) fn decode_smpl_lsf(
    dec: &mut RangeDecoder,
    t: &SmplTables,
    st: &mut SmplLsfState,
    config: usize,
    intf: usize,
) -> SmplLsfIndices {
    let mut idx = SmplLsfIndices {
        stage1: 0,
        grid: 0,
        stage2: [0; 16],
        extra: 0,
    };

    // Read 1: stage-1 selector. The first internal frame uses the dedicated row 0; later frames
    // pick row 1/2 by the previous frame's stage-1 result.
    let sel = if intf == 0 {
        0
    } else if st.prev_stage1 != 0 {
        2
    } else {
        1
    };
    let stage1 = dec.decode_cdf(&t.lsf_sel[sel]);
    idx.stage1 = stage1;

    // match := enter_match && stage1 == prev_stage1. enter_match is false for the first internal
    // frame and true afterwards (reset to 1 after each synthesis). On no-match the pitch/LTP
    // predictor resets to -1.
    let enter_match = intf != 0;
    let m = enter_match && (stage1 == st.prev_stage1);
    if !m {
        st.prev_gain_idx = -1;
        st.prev_filt_idx = -1;
        st.prev_lag = -1;
        st.prev_frac_lag = -1;
    }
    st.prev_stage1 = stage1;

    // Read 2: stage-1 grid. Inner select keys on the CURRENT stage1, outer select on match.
    let grid_cdf: &[u16] = if m {
        if stage1 != 0 {
            &t.lsf_grid.match1
        } else {
            &t.lsf_grid.match1_alt
        }
    } else if stage1 != 0 {
        &t.lsf_grid.match0_alt
    } else {
        &t.lsf_grid.match0
    };
    let grid = dec.decode_cdf(grid_cdf);
    idx.grid = grid;
    st.prev_match = m;
    st.have_prev = true;

    // Read 3: 16 stage-2 residuals, each coeff k from its own CDF g_lsf[stage1][config][grid][k].
    let st2 = &t.lsf_stage2[stage1 as usize][config][grid as usize];
    for (k, c) in st2.iter().enumerate().take(16) {
        idx.stage2[k] = dec.decode_cdf(c);
    }

    // "Extra" LSF read: a 3-symbol static CDF, always fires for our path (p4=1, num_subfr>=2).
    idx.extra = dec.decode_cdf(&t.lsf_extra);

    log::trace!(
        "mlow LSF intf={intf} sel={sel} m={m}: stage1={stage1} grid={grid} extra={} stage2={:?}",
        idx.extra,
        idx.stage2
    );
    idx
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    // Validates the first-internal-frame LSF parse (range coder at body start, the reliable
    // validation point) against the captured vectors over every active frame.
    #[test]
    fn lsf_frame0_matches_go() {
        let recs: Value =
            serde_json::from_str(include_str!("testdata/lsf_vectors.json")).expect("lsf_vectors");
        let t = load_smpl_tables();
        let arr = recs.as_array().unwrap();
        assert!(!arr.is_empty());
        for rec in arr {
            let frame = hex::decode(rec["frame"].as_str().unwrap()).unwrap();
            let mut st = SmplLsfState::default();
            let mut dec = RangeDecoder::new(&frame[1..]);
            let idx = decode_smpl_lsf(&mut dec, t, &mut st, 0, 0);
            assert_eq!(idx.stage1, rec["stage1"].as_i64().unwrap() as i32, "stage1");
            assert_eq!(idx.grid, rec["grid"].as_i64().unwrap() as i32, "grid");
            assert_eq!(idx.extra, rec["extra"].as_i64().unwrap() as i32, "extra");
            let want2: Vec<i32> = rec["stage2"]
                .as_array()
                .unwrap()
                .iter()
                .map(|x| x.as_i64().unwrap() as i32)
                .collect();
            assert_eq!(idx.stage2.to_vec(), want2, "stage2");
            assert_eq!(dec.err, 0, "no decode error");
        }
    }
}

//! SILK-style multi-stage pitch estimator. It HP-filters and 2x-downsamples the perceptually-weighted
//! `ltp_buf`, runs an open-loop block-track survivor search at the coarse (16 kHz upsampled from
//! 8 kHz) resolution, refines per-block at full resolution around the survivors, and folds in
//! rate/prev-lag/spectral-harmonicity biases. The outputs (`pitchcorr`, per-subframe `lags[8]`,
//! `avg_lag`, `harm_strength`) feed the bit-exact `smpl_get_signal_mode` classifier.
//!
//! The constant tables (blocksegs/blocktracks/CMFs, decoded from a packed bitstream via the range
//! decoder + `dcmf_to_cmf` at load time) are loaded from a committed seed blob rather than re-porting
//! the decoders, since they are immutable. Only the 20 ms / 8-subframe config is supported (the active
//! MLow 1:1 path); 10 ms frames never occur here.
#![allow(clippy::needless_range_loop)]

use super::smpl_signal_mode::{build_f2w, harm_strength_at};
use std::sync::OnceLock;

// codec constants
const FS_KHZ: i32 = 16;
const STAGE1_FS_KHZ: i32 = 8;
const COARSE_FS_KHZ: i32 = 16;
const TOT_INTERP_DELAY: i32 = 6;
pub(crate) const NUM_SUBFRAMES: usize = 8;
const MINPITCH_MS: i32 = 2;
const MAXPITCH_MS: i32 = 20;
const MINPITCH_LEN: i32 = MINPITCH_MS * FS_KHZ; // 32
const MAXPITCH_LEN: i32 = MAXPITCH_MS * FS_KHZ; // 320
const MINPITCH_STAGE1: i32 = MINPITCH_MS * STAGE1_FS_KHZ - TOT_INTERP_DELAY; // 10
const MAXPITCH_STAGE1: i32 = MAXPITCH_MS * STAGE1_FS_KHZ + TOT_INTERP_DELAY; // 166
const PITCH_DELTAWGHT: f32 = 0.1439;
const PITCH_SHORTWGHT1: f32 = 0.04;
const SPEC_HARM_BIAS: f32 = 2.5;
const PREVWGHT: f32 = 0.7981;
const PREVWGHT_SPAN: f32 = 0.15;
const RATEWGHT_HR: f32 = 0.022;
const LAG_SUBFRLEN: i32 = 40;
const LAG_SUBFRLEN_STAGE1: i32 = STAGE1_FS_KHZ * LAG_SUBFRLEN / FS_KHZ; // 20
const PITCHBLOCK_MS: i32 = 2;
const PITCH_LOOKAHEAD_LEN: usize = 7;
pub(crate) const MAX_LTP_BUF_LEN: usize = 659;
const F_LEN: usize = 257;

const PITCH_DOWNSAMP_DELAY: usize = 7;
const PITCH_INTERPOL_DELAY_C: usize = 4;

const PITCH_NUM_BLOCKS: usize = ((MAXPITCH_MS - MINPITCH_MS) / PITCHBLOCK_MS) as usize; // 9
const PITCHBLOCK: usize = (PITCHBLOCK_MS * FS_KHZ) as usize; // 32
const NUM_LAGS_STAGE1: usize = (MAXPITCH_STAGE1 - MINPITCH_STAGE1 + 1) as usize; // 157
const NUMLAGS_COARSE: usize = (COARSE_FS_KHZ * (MAXPITCH_MS - MINPITCH_MS)) as usize; // 288
const NUMLAGS_FS: usize = (FS_KHZ * (MAXPITCH_MS - MINPITCH_MS)) as usize; // 288

/// `numstates1` block-track survivors at complexity 5-8 (`pitch_numstates1 = 24`); overrides the
/// per-stream init default of 8.
const NUMSTATES1: usize = 24;
/// complexity-8 is NOT low-complexity (numstates1 > 4), so `low_complexity_mode == false`.
const LOW_COMPLEXITY: bool = false;
/// 20 kbps is the HIGH-rate path (`low_rate == false`).
const LOW_RATE: bool = false;

// decoded constant tables (expanded once from the pitch seed ROM)
pub(crate) struct BlockSeg {
    pub(crate) nblocks: usize,
    pub(crate) blocks: Vec<usize>,
    pub(crate) seglens: Vec<usize>,
}
pub(crate) struct BlockTrack {
    pub(crate) track: [usize; NUM_SUBFRAMES],
    pub(crate) meanblock: f32,
    pub(crate) trackdeltas: f32,
}
pub(crate) struct PitchTables {
    blocksegs: Vec<BlockSeg>,
    blocktracks: Vec<BlockTrack>,
    blocksegs2idx: Vec<usize>,
    blockseg_idx_cmf: Vec<u32>,
    delta_lag_cmfs: Vec<Vec<u32>>,
    blocksegs_ix: Vec<[usize; 2]>,
    firstblock_range: Vec<[usize; 2]>,
    block_transition_cmf: Vec<Vec<u32>>,
}

static TABLES: OnceLock<PitchTables> = OnceLock::new();

pub(crate) fn load_pitch_tables() -> &'static PitchTables {
    TABLES.get_or_init(|| {
        let seed: super::smpl_pitch_seed::PitchSeed =
            super::smpl_tables_blob::load_blob_prost(include_bytes!("testdata/pitch_seed.bin"));
        seed.build()
    })
}

impl PitchTables {
    /// Assemble the runtime tables from the expanded parts (the pitch seed builder calls this).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_parts(
        blocksegs: Vec<BlockSeg>,
        blocktracks: Vec<BlockTrack>,
        blocksegs2idx: Vec<usize>,
        blockseg_idx_cmf: Vec<u32>,
        delta_lag_cmfs: Vec<Vec<u32>>,
        blocksegs_ix: Vec<[usize; 2]>,
        firstblock_range: Vec<[usize; 2]>,
        block_transition_cmf: Vec<Vec<u32>>,
    ) -> PitchTables {
        PitchTables {
            blocksegs,
            blocktracks,
            blocksegs2idx,
            blockseg_idx_cmf,
            delta_lag_cmfs,
            blocksegs_ix,
            firstblock_range,
            block_transition_cmf,
        }
    }

    /// FNV-1a digest over every field (order-deterministic), for the seed-build tripwire.
    #[cfg(test)]
    pub(crate) fn debug_checksum(&self) -> u64 {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut feed = |x: u64| {
            h ^= x;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        };
        for s in &self.blocksegs {
            feed(s.nblocks as u64);
            for &b in &s.blocks {
                feed(b as u64);
            }
            for &l in &s.seglens {
                feed(l as u64);
            }
        }
        for t in &self.blocktracks {
            for &v in &t.track {
                feed(v as u64);
            }
            feed(t.meanblock.to_bits() as u64);
            feed(t.trackdeltas.to_bits() as u64);
        }
        for &v in &self.blocksegs2idx {
            feed(v as u64);
        }
        for &v in &self.blockseg_idx_cmf {
            feed(v as u64);
        }
        for row in &self.delta_lag_cmfs {
            for &v in row {
                feed(v as u64);
            }
        }
        for p in &self.blocksegs_ix {
            feed(p[0] as u64);
            feed(p[1] as u64);
        }
        for p in &self.firstblock_range {
            feed(p[0] as u64);
            feed(p[1] as u64);
        }
        for row in &self.block_transition_cmf {
            for &v in row {
                feed(v as u64);
            }
        }
        h
    }
}

/// Per-stream estimator state (the `PitchEstimator` non-scratch fields). `prev_lagblk/prev_lagidx`
/// are reset to -1 at frame boundaries by the encoder (`reset_cond`).
#[derive(Clone)]
pub(crate) struct PitchEstState {
    pub prev_lag: f32,
    pub prev_pitch_corr: f32,
    pub prev_lagblk: i32,
    pub prev_lagidx: i32,
}

impl Default for PitchEstState {
    fn default() -> Self {
        PitchEstState {
            prev_lag: 0.0,
            prev_pitch_corr: 0.0,
            prev_lagblk: -1,
            prev_lagidx: -1,
        }
    }
}

impl PitchEstState {
    /// Clear the cross-frame lag-block predictor (called after the last frame of a packet and after
    /// any unvoiced frame, so cond-coding restarts).
    pub fn reset_cond(&mut self) {
        self.prev_lagblk = -1;
        self.prev_lagidx = -1;
    }
}

/// Pitch estimator result for one internal frame. `laginds`/`blockseg_idx` are the estimator's chosen
/// per-block lag indices and contour; the classifier consumes `pitchcorr`/`lags`/`avg_lag`/`harm`,
/// while the wire pitch encoder (downstream) can use the contour to carry the per-block lags.
pub(crate) struct PitchResult {
    pub pitchcorr: f32,
    pub lags: [f32; NUM_SUBFRAMES],
    #[allow(dead_code)]
    pub laginds: [i32; NUM_SUBFRAMES],
    pub avg_lag: f32,
    pub harm_strength: f32,
    #[allow(dead_code)]
    pub blockseg_idx: usize,
}

// filters / DSP helpers

/// ARMA1 with `pitch_hp_b={1,-1}`, `pitch_hp_a={1,-0.96}`, zero state at call start. MA1
/// (`ma[n] = x[n] - x[n-1]`, `ma[0] = x[0]`) then AR1 in a 5-sample unrolled form using precomputed
/// powers of `ar1 = 0.96`, so the float accumulation order is stable.
fn pitch_hp_filter(x: &[f32], out: &mut [f32]) {
    let n = x.len();
    // MA1 into `out` (state_ma = x[-1] = 0).
    let mut state_ma = 0.0f32;
    for i in 0..n {
        out[i] = x[i] - state_ma;
        state_ma = x[i];
    }
    // AR1: y[n] = ma[n] + 0.96*y[n-1], unrolled by 5.
    let ar1 = 0.96f32;
    let ar1_2 = ar1 * ar1;
    let ar1_3 = ar1 * ar1_2;
    let ar1_4 = ar1 * ar1_3;
    let ar1_5 = ar1 * ar1_4;
    let mut ytmp = 0.0f32; // y[-1]
    let mut idx = 0usize;
    while idx + 4 < n {
        let x0 = out[idx];
        let x1 = out[idx + 1];
        let x2 = out[idx + 2];
        let x3 = out[idx + 3];
        let x4 = out[idx + 4];
        out[idx + 4] = x4 + ar1 * x3 + ar1_2 * x2 + ar1_3 * x1 + ar1_4 * x0 + ar1_5 * ytmp;
        out[idx] = x0 + ar1 * ytmp;
        out[idx + 1] = x1 + ar1 * x0 + ar1_2 * ytmp;
        out[idx + 2] = x2 + ar1 * x1 + ar1_2 * x0 + ar1_3 * ytmp;
        out[idx + 3] = x3 + ar1 * x2 + ar1_2 * x1 + ar1_3 * x0 + ar1_4 * ytmp;
        ytmp = out[idx + 4];
        idx += 5;
    }
    while idx < n {
        ytmp = out[idx] + ytmp * ar1;
        out[idx] = ytmp;
        idx += 1;
    }
}

const DOWNSAMP_FILT: [f32; 2 * PITCH_DOWNSAMP_DELAY + 1] = [
    -0.045472838,
    0.0,
    0.06366198,
    0.0,
    -0.10610329,
    0.0,
    0.31830987,
    0.5,
    0.31830987,
    0.0,
    -0.10610329,
    0.0,
    0.06366198,
    0.0,
    -0.045472838,
];

/// 2x decimating FIR. `ptr_in` has `PITCH_DOWNSAMP_DELAY` lead samples (offset) already written into
/// `ptr_out[0..offset]`; output length is `(L - 2*delay)/2`.
fn pitch_downsample(ptr_in: &[f32], l: usize, ptr_out: &mut [f32]) -> usize {
    let d = PITCH_DOWNSAMP_DELAY;
    let n = (l - 2 * d) / 2;
    for j in 0..n {
        let mut tmp = ptr_in[2 * j + d] * DOWNSAMP_FILT[d];
        let mut i = 0;
        while i < d {
            tmp += (ptr_in[2 * j + i] + ptr_in[2 * j + 2 * d - i]) * DOWNSAMP_FILT[i];
            i += 2;
        }
        ptr_out[j] = tmp;
    }
    n
}

const INTERPOL_FILT_C: [f32; 2 * PITCH_INTERPOL_DELAY_C] = [
    -0.0024414062,
    0.023925781,
    -0.119628906,
    0.59814453,
    0.59814453,
    -0.119628906,
    0.023925783,
    -0.0024414062,
];

/// Writes `2*len` samples backwards from `y` using `x` (read backwards). Even taps copy `x`, odd taps
/// average adjacent. `y_end`/`x_end` are the indices of the LAST written/read.
fn upsamp_e_core(buf: &mut [f32], x_end: usize, y_end: usize, len: usize) {
    let mut xi = x_end as isize;
    let mut yi = y_end as isize;
    for _ in 0..len {
        let v = (buf[xi as usize] + buf[(xi + 1) as usize]) * 0.5;
        buf[yi as usize] = v;
        yi -= 1;
        buf[yi as usize] = buf[xi as usize];
        yi -= 1;
        xi -= 1;
    }
}

/// Like `upsamp_e_core` but the interpolated sample uses the 8-tap `INTERPOL_FILT_C`.
fn upsamp_c_core(buf: &mut [f32], x_end: usize, y_end: usize, len: usize) {
    let mut xi = x_end as isize;
    let mut yi = y_end as isize;
    for _ in 0..len {
        let mut tmp = 0.0f32;
        for j in 0..PITCH_INTERPOL_DELAY_C {
            let a = buf[(xi + j as isize - (PITCH_INTERPOL_DELAY_C as isize - 1)) as usize];
            let b = buf[(xi + PITCH_INTERPOL_DELAY_C as isize - j as isize) as usize];
            tmp += (a + b) * INTERPOL_FILT_C[j];
        }
        buf[yi as usize] = tmp;
        yi -= 1;
        buf[yi as usize] = buf[xi as usize];
        yi -= 1;
        xi -= 1;
    }
}

#[inline]
fn smpl_nrg(x: &[f32]) -> f32 {
    x.iter().map(|&v| v * v).sum()
}

/// Argmax; ties resolve to the FIRST index. A simple strict-`>` scan (lowest index wins) matches the
/// reference tree-reduction (validated by a tie-probe harness on this data).
fn get_maxi(x: &[f32]) -> usize {
    let mut bi = 0usize;
    let mut best = x[0];
    for n in 1..x.len() {
        if x[n] > best {
            best = x[n];
            bi = n;
        }
    }
    bi
}

/// K highest-value indices. The ascending masked-max scan (strict `>`, lowest-index-wins) is the
/// validated equivalent of the reference tree selection. Returns them in selection order (descending
/// value).
fn get_maxi_k(x: &[f32], k: usize) -> Vec<usize> {
    let mut taken = vec![false; x.len()];
    let mut out = Vec::with_capacity(k);
    for _ in 0..k {
        let mut bi: isize = -1;
        let mut best = f32::MIN;
        for n in 0..x.len() {
            if !taken[n] && (bi < 0 || x[n] > best) {
                best = x[n];
                bi = n as isize;
            }
        }
        if bi < 0 {
            break;
        }
        taken[bi as usize] = true;
        out.push(bi as usize);
    }
    out
}

// E1 / C / E2 computation

/// Running energy of `lag_subfrlen`-length windows ending just before lag `minpitch`, for each of
/// `numlags` lags. `t` is the window-start anchor in `ltpbuf`.
fn calc_e1_inner(
    e1: &mut [f32],
    ltpbuf: &[f32],
    t: usize,
    minpitch: i32,
    maxpitch: i32,
    lag_subfrlen: usize,
) {
    let numlags = (maxpitch - minpitch + 1) as usize;
    let base = (t as i32 - minpitch) as usize; // &ltpbuf[t - minpitch]
    let reg = &ltpbuf[base - (numlags - 1)..]; // reg[-i] for i in 0..numlags valid
    // reg points at ltpbuf[t - minpitch]; we index reg[0], reg[-i], reg[lag_subfrlen - i].
    let reg0 = base; // absolute index of reg[0]
    e1[0] = smpl_nrg(&ltpbuf[reg0..reg0 + lag_subfrlen]).max(1e-9);
    for i in 1..numlags {
        let rm = ltpbuf[reg0 - i];
        let rs = ltpbuf[reg0 + lag_subfrlen - i];
        e1[i] = (e1[i - 1] + rm * rm - rs * rs).max(1e-9);
    }
    let _ = reg;
}

/// Per-subframe E1 by computing an extended E1_ once then offsetting per subframe.
fn calc_e1(
    e1: &mut [f32],
    ltpbuf: &[f32],
    ltpbuf_len: usize,
    numsubfrs: usize,
    minpitch: i32,
    maxpitch: i32,
    lag_subfrlen: usize,
) {
    let numlags = (maxpitch - minpitch + 1) as usize;
    let maxpitch_ = maxpitch + (numsubfrs as i32 - 1) * lag_subfrlen as i32;
    let numlags_ = (maxpitch_ - minpitch + 1) as usize;
    let t = ltpbuf_len - lag_subfrlen;
    let mut e1_ext = vec![0.0f32; numlags_];
    calc_e1_inner(&mut e1_ext, ltpbuf, t, minpitch, maxpitch_, lag_subfrlen);
    let mut offset = (numlags_ - numlags) as isize;
    for sf in 0..numsubfrs {
        for i in 0..numlags {
            e1[sf * numlags + i] = e1_ext[(offset + i as isize) as usize];
        }
        offset -= lag_subfrlen as isize;
    }
}

fn dot_prod(a: &[f32], b: &[f32], n: usize) -> f32 {
    let mut r = 0.0f32;
    for i in 0..n {
        r += a[i] * b[i];
    }
    r
}

/// Stage-1 cross-correlation `C` (8-sample dot, NUM_LAGS_STAGE1 lags/subframe) and per-subframe target
/// energy `E2`.
fn calc_c_e2(c: &mut [f32], e2: &mut [f32], ltpbuf: &[f32], ltpbuf_len: usize, numsubfrs: usize) {
    let mut t = ltpbuf_len - LAG_SUBFRLEN_STAGE1 as usize * numsubfrs;
    for sf in 0..numsubfrs {
        let tgt = &ltpbuf[t..t + 20];
        let reg0 = (t as i32 - MINPITCH_STAGE1) as usize;
        for i in 0..NUM_LAGS_STAGE1 {
            // dot_prod_20(tgt, &reg[-i]) where reg=&ltpbuf[reg0]
            let r = &ltpbuf[reg0 - i..reg0 - i + 20];
            c[sf * NUM_LAGS_STAGE1 + i] = dot_prod(tgt, r, 20);
        }
        t += LAG_SUBFRLEN_STAGE1 as usize;
        e2[sf] = dot_prod(tgt, tgt, 20).max(1e-9);
    }
}

/// In-place 2x upsample of a per-subframe E array, high subframe first.
fn upsamp_e_fast(buf: &mut [f32], numsubfrs: usize, minpitch: &mut i32, numlags: &mut usize) {
    let nin = *numlags;
    let nout = (nin - 1) * 2;
    for sf in (0..numsubfrs).rev() {
        let x_end = sf * nin + nin - 2;
        let y_end = sf * nout + nout - 1;
        upsamp_e_core(buf, x_end, y_end, nin - 1);
    }
    *numlags = nout;
    *minpitch *= 2;
}

/// In-place 2x upsample of a per-subframe C array using the interpolation filter.
fn upsamp_c_fast(buf: &mut [f32], numsubfrs: usize, minpitch: &mut i32, numlags: &mut usize) {
    let nin = *numlags;
    let nout = (nin - PITCH_INTERPOL_DELAY_C) * 2;
    for sf in (0..numsubfrs).rev() {
        let x_end = sf * nin + nin - 1 - PITCH_INTERPOL_DELAY_C;
        let y_end = sf * nout + nout - 1;
        upsamp_c_core(buf, x_end, y_end, nin - (PITCH_INTERPOL_DELAY_C * 2 - 1));
    }
    *numlags = nout;
    *minpitch *= 2;
}

fn dot_prod_40(a: &[f32], b: &[f32]) -> f32 {
    let mut r = 0.0f32;
    for i in 0..40 {
        r += a[i] * b[i];
    }
    r
}

fn sumdeltas(laginds: &[i32], numsubfrs: usize) -> i32 {
    let mut ret = 0;
    for i in 1..numsubfrs {
        ret += (laginds[i] - laginds[i - 1]).abs();
    }
    ret
}

/// The rate (bits) the lag indices would cost, used as a survivor bias. Accumulates n_bits with no
/// entropy-coding side-effects (the cost-only path).
fn encode_lags_bits(
    tab: &PitchTables,
    blocksegs_ix: usize,
    laginds: &[i32],
    prev_lagblk: i32,
    prev_lagidx: i32,
    mode: usize,
) -> f32 {
    let mut n_bits = 0.0f32;
    let ix_julia = tab.blocksegs2idx[blocksegs_ix] as i32;
    let blocksize = PITCHBLOCK_MS * FS_KHZ * 2; // 64
    let pblockseg = &tab.blocksegs[blocksegs_ix];
    let mut prev_lagblk = prev_lagblk;
    let mut prev_lagidx = prev_lagidx;

    if prev_lagblk < 0 {
        let cmf = &tab.blockseg_idx_cmf;
        n_bits += ec_encode_bits(
            cmf[(ix_julia - 1) as usize],
            cmf[ix_julia as usize],
            cmf[tab.blocksegs.len()],
        );
    } else {
        let cmf = &tab.block_transition_cmf[prev_lagblk as usize];
        let b0 = pblockseg.blocks[0];
        n_bits += ec_encode_bits(cmf[b0], cmf[b0 + 1], cmf[PITCH_NUM_BLOCKS]);
        let start_ix = tab.firstblock_range[b0][0] as i32;
        let cmf_len = (tab.firstblock_range[b0][1] - tab.firstblock_range[b0][0] + 1) as i32;
        let cmf = &tab.blockseg_idx_cmf[start_ix as usize..];
        let lo = (ix_julia - start_ix - 1) as usize;
        let hi = (ix_julia - start_ix) as usize;
        n_bits += ec_encode_bits(
            cmf[lo] - cmf[0],
            cmf[hi] - cmf[0],
            cmf[cmf_len as usize] - cmf[0],
        );
    }

    let mut blk = pblockseg.blocks[0] as i32;
    let mut delta_blk = blk - prev_lagblk;
    let mut start_seg = 0usize;
    let mut laginds_ix = 0usize;
    if !((prev_lagblk > -1) && (-1..=2).contains(&delta_blk)) {
        n_bits += 6.0; // uniform first-lag cost (log2 blocksize)
        prev_lagblk = blk;
        prev_lagidx = laginds[laginds_ix];
        laginds_ix += pblockseg.seglens[0];
        start_seg = 1;
    }
    let delta_lag_cmf = &tab.delta_lag_cmfs[mode];
    for k in start_seg..pblockseg.nblocks {
        blk = pblockseg.blocks[k] as i32;
        let idx = laginds[laginds_ix];
        laginds_ix += pblockseg.seglens[k];
        delta_blk = blk - prev_lagblk;
        let delta_idx = idx - prev_lagidx;
        let prev_lagidx_mod = prev_lagidx - prev_lagblk * blocksize;
        let delta_range_start = -prev_lagidx_mod + delta_blk * blocksize;
        let cmf_base = (delta_range_start + 2 * blocksize - 1) as usize;
        let ix = (delta_idx - delta_range_start) as usize;
        let p0 = delta_lag_cmf[cmf_base];
        n_bits += ec_encode_bits(
            delta_lag_cmf[cmf_base + ix] - p0,
            delta_lag_cmf[cmf_base + ix + 1] - p0,
            delta_lag_cmf[cmf_base + blocksize as usize] - p0,
        );
        prev_lagblk = blk;
        prev_lagidx = idx;
    }
    n_bits
}

/// Returns the symbol's bit cost `-log2((fh-fl)/ft)` without coding it.
fn ec_encode_bits(fl: u32, fh: u32, ft: u32) -> f32 {
    let p = (fh as f32 - fl as f32) / ft as f32;
    if p <= 0.0 { 0.0 } else { -p.log2() }
}

/// Write the blockseg selector + the per-40-block lag indices (`laginds`) to the range stream. This
/// IS the voiced lag wire encode, the inverse of `decode_smpl_pitch`'s contour reconstruction.
/// `prev_lagblk`/`prev_lagidx` are the lag predictor (-1 on the first frame of a packet / after a
/// no-match); `mode` selects the delta-lag CMF (0/1/2 by the mean ACB gain). The decoder rebuilds the
/// exact per-block contour from these bits, so its voiced ACB basis matches the encoder's.
pub(crate) fn smpl_encode_lags_wire(
    tab: &PitchTables,
    enc: &mut super::rangecoder::RangeEncoder,
    blocksegs_ix: usize,
    laginds: &[i32; NUM_SUBFRAMES],
    prev_lagblk: i32,
    prev_lagidx: i32,
    mode: usize,
) {
    let ix_julia = tab.blocksegs2idx[blocksegs_ix] as i32;
    let blocksize = PITCHBLOCK_MS * FS_KHZ * 2; // 64
    let pblockseg = &tab.blocksegs[blocksegs_ix];
    let mut prev_lagblk = prev_lagblk;
    let mut prev_lagidx = prev_lagidx;

    // Blockseg selector: absolute (CMF over all blocksegs) when no predictor, else block-transition
    // CMF for blocks[0] followed by the first-block-range CMF.
    if prev_lagblk < 0 {
        let cmf = &tab.blockseg_idx_cmf;
        enc.encode(
            cmf[(ix_julia - 1) as usize],
            cmf[ix_julia as usize],
            cmf[tab.blocksegs.len()],
        );
    } else {
        let cmf = &tab.block_transition_cmf[prev_lagblk as usize];
        let b0 = pblockseg.blocks[0];
        enc.encode(cmf[b0], cmf[b0 + 1], cmf[PITCH_NUM_BLOCKS]);
        let start_ix = tab.firstblock_range[b0][0] as i32;
        let cmf_len = (tab.firstblock_range[b0][1] - tab.firstblock_range[b0][0] + 1) as i32;
        let cmf = &tab.blockseg_idx_cmf[start_ix as usize..];
        let lo = (ix_julia - start_ix - 1) as usize;
        let hi = (ix_julia - start_ix) as usize;
        enc.encode(
            cmf[lo] - cmf[0],
            cmf[hi] - cmf[0],
            cmf[cmf_len as usize] - cmf[0],
        );
    }

    let mut blk = pblockseg.blocks[0] as i32;
    let mut delta_blk = blk - prev_lagblk;
    let mut start_seg = 0usize;
    let mut laginds_ix = 0usize;
    if !((prev_lagblk > -1) && (-1..=2).contains(&delta_blk)) {
        // First lag uniform-coded over `blocksize`.
        let idx_mod = (laginds[laginds_ix] - blk * blocksize) as u32;
        enc.encode(idx_mod, idx_mod + 1, blocksize as u32);
        prev_lagblk = blk;
        prev_lagidx = laginds[laginds_ix];
        laginds_ix += pblockseg.seglens[0];
        start_seg = 1;
    }
    let delta_lag_cmf = &tab.delta_lag_cmfs[mode];
    for k in start_seg..pblockseg.nblocks {
        blk = pblockseg.blocks[k] as i32;
        let idx = laginds[laginds_ix];
        laginds_ix += pblockseg.seglens[k];
        delta_blk = blk - prev_lagblk;
        let delta_idx = idx - prev_lagidx;
        let prev_lagidx_mod = prev_lagidx - prev_lagblk * blocksize;
        let delta_range_start = -prev_lagidx_mod + delta_blk * blocksize;
        let cmf_base = (delta_range_start + 2 * blocksize - 1) as usize;
        let ix = (delta_idx - delta_range_start) as usize;
        let p0 = delta_lag_cmf[cmf_base];
        enc.encode(
            delta_lag_cmf[cmf_base + ix] - p0,
            delta_lag_cmf[cmf_base + ix + 1] - p0,
            delta_lag_cmf[cmf_base + blocksize as usize] - p0,
        );
        prev_lagblk = blk;
        prev_lagidx = idx;
    }
}

/// The lag predictor after a voiced encode: `prev_lagblk = blocks[nblocks-1]`,
/// `prev_lagidx = laginds[numsubfrs-1]`. Exposed so the analysis advances its mirror identically.
pub(crate) fn smpl_lags_predictor_after(
    tab: &PitchTables,
    blocksegs_ix: usize,
    laginds: &[i32; NUM_SUBFRAMES],
) -> (i32, i32) {
    let pblockseg = &tab.blocksegs[blocksegs_ix];
    let last_blk = pblockseg.blocks[pblockseg.nblocks - 1] as i32;
    (last_blk, laginds[NUM_SUBFRAMES - 1])
}

/// Spectral harmonicity with a per-survivor cache (keyed by harmonic bin). Reuses the classifier's
/// recompute via `harm_strength_at` for a single value; the loop here threads the cache per survivor.
fn spectral_harmonicity_cached(
    avg_lag: f32,
    f2w: &[f32; F_LEN],
    cache: &mut [f32],
    reset: bool,
) -> f32 {
    const HARM_UNDEF: f32 = -10000.0;
    if reset {
        for c in cache.iter_mut() {
            *c = HARM_UNDEF;
        }
    }
    let inv_f2_step_hz = 2.0 * (F_LEN - 1) as f32 / 16000.0;
    let harm_hz = 16000.0 / avg_lag;
    let harm_ix = (harm_hz * 2.0 * inv_f2_step_hz).round() as i32;
    // The classifier's recompute is the single source of truth: an out-of-range bin only arises from a
    // degenerate lag and is handled there by a clamped recompute.
    if harm_ix < 0 || harm_ix as usize >= cache.len() {
        return harm_strength_at(avg_lag, f2w);
    }
    if cache[harm_ix as usize] > HARM_UNDEF {
        return cache[harm_ix as usize];
    }
    let hs = harm_strength_at(avg_lag, f2w);
    cache[harm_ix as usize] = hs;
    hs
}

/// The full estimator. `ltp_buf` is the perceptually-weighted speech of length `MAX_LTP_BUF_LEN` (the
/// last `PITCH_LOOKAHEAD_LEN` samples are lookahead). `f2` is the LPC power spectrum.
/// `coded_as_active_voice` gates the search (false -> unvoiced defaults). Mutates the cross-frame
/// predictor in `st`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn smpl_pitch(
    st: &mut PitchEstState,
    ltp_buf: &[f32],
    f2: &[f32; F_LEN],
    coded_as_active_voice: bool,
) -> PitchResult {
    let tab = load_pitch_tables();
    let numsubfrs = NUM_SUBFRAMES;
    let l = MAX_LTP_BUF_LEN;
    let look = PITCH_LOOKAHEAD_LEN;

    if !coded_as_active_voice {
        let min_lag = (MINPITCH_MS * FS_KHZ) as f32;
        st.prev_lag = 0.0;
        st.prev_pitch_corr = 0.0;
        st.prev_lagblk = -1;
        st.prev_lagidx = -1;
        return PitchResult {
            pitchcorr: 0.0,
            lags: [min_lag; NUM_SUBFRAMES],
            laginds: [0; NUM_SUBFRAMES],
            avg_lag: min_lag,
            harm_strength: 0.0,
            blockseg_idx: 0,
        };
    }

    // HP filter into ltp_buf_stage1[offset..], where offset = PITCH_DOWNSAMP_DELAY leading zeros.
    let offset = PITCH_DOWNSAMP_DELAY;
    let mut stage1 = vec![0.0f32; l + offset + 64]; // small slack
    pitch_hp_filter(ltp_buf, &mut stage1[offset..offset + l]);
    // ltp_buf_hp = stage1[offset .. offset + (L - look)]
    let hp_len = l - look;
    let ltp_buf_hp: Vec<f32> = stage1[offset..offset + hp_len].to_vec();

    // Downsample stage1[0 .. L+offset] -> stage1_ds. We keep the HP signal already copied out, so a
    // separate output buffer is equivalent to the reference in-place write.
    let mut stage1_ds = vec![0.0f32; (l + offset) / 2 + 8];
    let stage1_len = pitch_downsample(&stage1, l + offset, &mut stage1_ds);

    let numlags0 = NUM_LAGS_STAGE1;
    let mut e1 = vec![0.0f32; numlags0 * numsubfrs + 16];
    calc_e1(
        &mut e1,
        &stage1_ds,
        stage1_len,
        numsubfrs,
        MINPITCH_STAGE1,
        MAXPITCH_STAGE1,
        LAG_SUBFRLEN_STAGE1 as usize,
    );
    let mut e2 = vec![0.0f32; numsubfrs];
    // C / E arrays are over-allocated: the upsample stages expand them in place to full-res widths.
    let cap = (2 * FS_KHZ / STAGE1_FS_KHZ) as usize * NUM_LAGS_STAGE1 * numsubfrs + 64;
    let mut c = vec![0.0f32; cap];
    let mut e = vec![0.0f32; cap];
    let mut c_stage1 = vec![0.0f32; numlags0 * numsubfrs];
    calc_c_e2(&mut c_stage1, &mut e2, &stage1_ds, stage1_len, numsubfrs);
    c[..numlags0 * numsubfrs].copy_from_slice(&c_stage1);

    // E from sqrt-energy blend (stage 1).
    let numlags = numlags0;
    for sf in 0..numsubfrs {
        let mut sqrt_e1 = vec![0.0f32; numlags];
        for i in 0..numlags {
            sqrt_e1[i] = (e1[sf * numlags + i] + 1e-30).sqrt();
        }
        let sqrt_e2 = (e2[sf] + 1e-30).sqrt();
        for i in 0..numlags {
            let tmp = 0.5 * (sqrt_e1[i] + sqrt_e2);
            e[sf * numlags + i] = tmp * tmp;
        }
    }

    // Upsample to coarse (16 kHz) resolution.
    let mut minpitch_c = MINPITCH_STAGE1;
    let mut numlags_c = numlags;
    let mut minpitch_e = MINPITCH_STAGE1;
    let mut numlags_e = numlags;
    if LOW_COMPLEXITY {
        upsamp_e_fast(&mut c, numsubfrs, &mut minpitch_c, &mut numlags_c);
    } else {
        upsamp_c_fast(&mut c, numsubfrs, &mut minpitch_c, &mut numlags_c);
    }
    upsamp_e_fast(&mut e, numsubfrs, &mut minpitch_e, &mut numlags_e);

    let minpitch_coarse = COARSE_FS_KHZ * MINPITCH_MS;
    let numlags_coarse = NUMLAGS_COARSE;
    let offset_c0 = (minpitch_coarse - minpitch_c) as usize;
    let offset_e0 = (minpitch_coarse - minpitch_e) as usize;

    // H (coarse) and coarse copies.
    let mut h = vec![0.0f32; numlags_coarse * numsubfrs * 2 + 64];
    let mut h_coarse = vec![0.0f32; numlags_coarse * numsubfrs];
    let mut c_coarse = vec![0.0f32; numlags_coarse * numsubfrs];
    let mut e_coarse = vec![0.0f32; numlags_coarse * numsubfrs];
    for sf in 0..numsubfrs {
        for i in 0..numlags_coarse {
            let cv = c[sf * numlags_c + offset_c0 + i];
            let ev = e[sf * numlags_e + offset_e0 + i];
            h[sf * numlags_coarse + i] = cv / ev;
        }
        h_coarse[sf * numlags_coarse..(sf + 1) * numlags_coarse]
            .copy_from_slice(&h[sf * numlags_coarse..sf * numlags_coarse + numlags_coarse]);
        for i in 0..numlags_coarse {
            c_coarse[sf * numlags_coarse + i] = c[sf * numlags_c + offset_c0 + i];
            e_coarse[sf * numlags_coarse + i] = e[sf * numlags_e + offset_e0 + i];
        }
    }

    // Per-block coarse maxima -> Hblk.
    let pitchblock_coarse = (PITCHBLOCK_MS * COARSE_FS_KHZ) as usize; // 32
    let mut hblk = [[0.0f32; PITCH_NUM_BLOCKS]; NUM_SUBFRAMES];
    for sf in 0..numsubfrs {
        for block in 0..PITCH_NUM_BLOCKS {
            let base = sf * numlags_coarse + block * pitchblock_coarse;
            hblk[sf][block] = smpl_maximum(&h[base..base + pitchblock_coarse]);
        }
    }

    // Block-track survivor selection.
    let blocksize_fs = PITCHBLOCK * 2; // BLOCKSIZE = 64
    let reduction_factor = 0.7f32;
    let pitch_deltawght = PITCH_DELTAWGHT / blocksize_fs as f32;
    let mut sf_wght = [0.0f32; NUM_SUBFRAMES];
    {
        let sum_e2: f32 = e2.iter().take(numsubfrs).sum();
        for sf in 0..numsubfrs {
            sf_wght[sf] = e2[sf] / sum_e2;
        }
    }
    let num_blocktracks = tab.blocktracks.len();
    let mut utils = vec![0.0f32; num_blocktracks];
    for i in 0..num_blocktracks {
        let bt = &tab.blocktracks[i];
        let mut corr = 0.0f32;
        for sf in 0..numsubfrs {
            corr += hblk[sf][bt.track[sf]] * sf_wght[sf];
        }
        let shortlagbias1 = (MAXPITCH_LEN as f32 / ((bt.meanblock + 1.5) * PITCHBLOCK as f32)
            - 1.0)
            * PITCH_SHORTWGHT1;
        utils[i] = 1.0 / (1.1 - corr)
            - reduction_factor * PITCHBLOCK as f32 * pitch_deltawght * bt.trackdeltas
            + shortlagbias1;
    }
    let track_idx = get_maxi_k(&utils, NUMSTATES1);

    // Recompute full-res E1 over the HP signal.
    let mut e1_fs = vec![0.0f32; numlags_e * numsubfrs + 16];
    calc_e1(
        &mut e1_fs,
        &ltp_buf_hp,
        l - look,
        numsubfrs,
        minpitch_e,
        minpitch_e + numlags_e as i32 - 1,
        LAG_SUBFRLEN as usize,
    );

    // uniqueblocks bitmask per subframe from the survivor tracks.
    let mut uniqueblocks = [0u16; NUM_SUBFRAMES];
    for &ti in &track_idx {
        let track = &tab.blocktracks[ti].track;
        for sf in 0..numsubfrs {
            uniqueblocks[sf] |= 1 << track[sf];
        }
    }

    let h_thres = if LOW_COMPLEXITY { 0.0 } else { 0.25 };
    let offset_c = (MINPITCH_MS * FS_KHZ - minpitch_c) as usize;
    let offset_e = (MINPITCH_MS * FS_KHZ - minpitch_e) as usize;
    // Update C and E around survivor block peaks at full resolution.
    for sf in 0..numsubfrs {
        let mut mask = 1u16;
        let c_ptr = offset_c + sf * numlags_c;
        let e_ptr = offset_e + sf * numlags_e;
        let e1_ptr = offset_e + sf * numlags_e;
        let h_ptr = sf * NUMLAGS_FS;
        // ltp_buf_ptr = &ltp_buf_hp[L - look + (sf - numsubfrs)*LAG_SUBFRLEN]
        let ltp_off = ((l - look) as i32 + (sf as i32 - numsubfrs as i32) * LAG_SUBFRLEN) as usize;
        let e2_sf = dot_prod_40(&ltp_buf_hp[ltp_off..], &ltp_buf_hp[ltp_off..]).max(1e-9);
        e2[sf] = e2_sf;
        let sqrt_e2 = (e2_sf + 1e-30).sqrt();
        for block in 0..PITCH_NUM_BLOCKS {
            if uniqueblocks[sf] & mask != 0 {
                let mut sqrt_e1 = [0.0f32; PITCHBLOCK + 1];
                for i in 0..PITCHBLOCK + 1 {
                    sqrt_e1[i] = (e1_fs[e1_ptr + block * PITCHBLOCK + i] + 1e-30).sqrt();
                }
                for i in 0..PITCHBLOCK + 1 {
                    let tmp = 0.5 * (sqrt_e1[i] + sqrt_e2);
                    e[e_ptr + block * PITCHBLOCK + i] = 0.5 * tmp * tmp;
                }
                for i in 0..PITCHBLOCK {
                    if h[h_ptr + block * PITCHBLOCK + i] > h_thres {
                        let lag = (MINPITCH_LEN as usize) + block * PITCHBLOCK + i;
                        let a = &ltp_buf_hp[ltp_off..];
                        let b = &ltp_buf_hp[ltp_off - lag..];
                        c[c_ptr + block * PITCHBLOCK + i] = 0.5 * dot_prod_40(a, b);
                    }
                }
            }
            mask <<= 1;
        }
    }

    // Upsample C and E around survivor peaks to half-sample resolution and compute H (high to low).
    let stride_c = PITCH_NUM_BLOCKS * 2 * PITCHBLOCK + offset_c; // per-subframe frac stride
    let stride_e = PITCH_NUM_BLOCKS * 2 * PITCHBLOCK + offset_e;
    for sf in (0..numsubfrs).rev() {
        let c_ptr = offset_c + sf * numlags_c;
        let c_ptr_frac = offset_c + sf * stride_c;
        let e_ptr = offset_e + sf * numlags_e;
        let e_ptr_frac = offset_e + sf * stride_e;
        let h_ptr = sf * 2 * PITCHBLOCK * PITCH_NUM_BLOCKS;
        let mut mask = 1u16 << (PITCH_NUM_BLOCKS - 1);
        for block in (0..PITCH_NUM_BLOCKS).rev() {
            if uniqueblocks[sf] & mask != 0 {
                let ein = e_ptr + block * PITCHBLOCK;
                let eout = e_ptr_frac + block * 2 * PITCHBLOCK;
                upsamp_e_core(
                    &mut e,
                    ein + PITCHBLOCK - 1,
                    eout + 2 * PITCHBLOCK - 1,
                    PITCHBLOCK,
                );
                let cin = c_ptr + block * PITCHBLOCK;
                let cout = c_ptr_frac + block * 2 * PITCHBLOCK;
                if LOW_COMPLEXITY {
                    upsamp_e_core(
                        &mut c,
                        cin + PITCHBLOCK - 1,
                        cout + 2 * PITCHBLOCK - 1,
                        PITCHBLOCK,
                    );
                } else {
                    upsamp_c_core(
                        &mut c,
                        cin + PITCHBLOCK - 1,
                        cout + 2 * PITCHBLOCK - 1,
                        PITCHBLOCK,
                    );
                }
                for i in 0..2 * PITCHBLOCK {
                    h[h_ptr + block * 2 * PITCHBLOCK + i] = c[cout + i] / e[eout + i];
                }
            }
            mask >>= 1;
        }
    }

    // Fine search: per survivor, per blockseg, per block: combine H over the seg's subframes, argmax.
    let mut laginds_surv: Vec<[i32; NUM_SUBFRAMES]> = Vec::new();
    let mut blocksegs_ix_list: Vec<usize> = Vec::new();
    let mut h_comb = vec![0.0f32; 2 * PITCHBLOCK];
    let mut lagind_cache: std::collections::HashMap<i32, i32> = std::collections::HashMap::new();
    for &idx in &track_idx {
        let range = tab.blocksegs_ix[idx];
        for j in 0..range[1] {
            let bsx = range[0] + j;
            let pblockseg = &tab.blocksegs[bsx];
            let mut laginds_row = [0i32; NUM_SUBFRAMES];
            let mut start_sf = 0usize;
            for n in 0..pblockseg.nblocks {
                let lookup_key = (((start_sf as i32) << 3) + pblockseg.seglens[n] as i32) << 4
                    | pblockseg.blocks[n] as i32;
                let best_i = if let Some(&v) = lagind_cache.get(&lookup_key) {
                    v
                } else {
                    for v in h_comb.iter_mut() {
                        *v = 0.0;
                    }
                    for sf in start_sf..start_sf + pblockseg.seglens[n] {
                        let h_ptr = sf * 2 * PITCHBLOCK * PITCH_NUM_BLOCKS
                            + pblockseg.blocks[n] * 2 * PITCHBLOCK;
                        for i in 0..2 * PITCHBLOCK {
                            h_comb[i] += h[h_ptr + i] * e2[sf];
                        }
                    }
                    let bi = get_maxi(&h_comb) as i32;
                    lagind_cache.insert(lookup_key, bi);
                    bi
                };
                for sf in start_sf..start_sf + pblockseg.seglens[n] {
                    laginds_row[sf] = best_i + (pblockseg.blocks[n] * 2 * PITCHBLOCK) as i32;
                }
                start_sf += pblockseg.seglens[n];
            }
            laginds_surv.push(laginds_row);
            blocksegs_ix_list.push(bsx);
        }
    }
    let nlaginds = laginds_surv.len();

    // Final search.
    let pitch_ratewght = if LOW_RATE { 0.028 } else { RATEWGHT_HR };
    let f2w = build_f2w(f2);
    let max_ix = get_maxi(&sf_wght[..numsubfrs]);
    let mut spectral_harm_cache = vec![0.0f32; 50];

    let mut best_util = 0.0f32;
    let mut best_pitchcorr = 0.0f32;
    let mut best_surv = 0usize;
    let pitch_deltawght_fs = PITCH_DELTAWGHT / blocksize_fs as f32;

    for surv in 0..nlaginds {
        let mut sum_c = 0.0f32;
        let mut sum_e = 0.0f32;
        for sf in 0..numsubfrs {
            let c_base = offset_c + sf * stride_c;
            let e_base = offset_e + sf * stride_e;
            let li = laginds_surv[surv][sf] as usize;
            sum_c += c[c_base + li];
            sum_e += e[e_base + li];
        }
        let rate_bias = encode_lags_bits(
            tab,
            blocksegs_ix_list[surv],
            &laginds_surv[surv],
            st.prev_lagblk,
            st.prev_lagidx,
            1,
        ) * pitch_ratewght;
        let mean_lag = laginds_surv[surv][max_ix] as f32 * 0.5 + MINPITCH_LEN as f32;
        let pitchcorr = sum_c / sum_e;
        let first_lag = 0.5 * laginds_surv[surv][0] as f32 + MINPITCH_LEN as f32;
        let prev_lag_bias = get_prev_lag_bias(st, first_lag);
        let spectral_harm_bias = SPEC_HARM_BIAS
            * spectral_harmonicity_cached(mean_lag, &f2w, &mut spectral_harm_cache, surv == 0);
        let util = 1.0 / (1.1 - pitchcorr)
            - pitch_deltawght_fs * sumdeltas(&laginds_surv[surv], numsubfrs) as f32
            + spectral_harm_bias
            + prev_lag_bias
            - rate_bias;
        if surv == 0 || util > best_util {
            best_util = util;
            best_surv = surv;
        }
        if surv == 0 || pitchcorr > best_pitchcorr {
            best_pitchcorr = pitchcorr;
        }
    }

    let mut lags = [0.0f32; NUM_SUBFRAMES];
    let mut laginds_out = [0i32; NUM_SUBFRAMES];
    for sf in 0..numsubfrs {
        lags[sf] = laginds_surv[best_surv][sf] as f32 * 0.5 + MINPITCH_LEN as f32;
        laginds_out[sf] = laginds_surv[best_surv][sf];
    }
    let avg_lag = laginds_surv[best_surv][max_ix] as f32 * 0.5 + MINPITCH_LEN as f32;
    // With SMPL_PITCH_SPEC_HARM_BIAS defined, the final harmonicity reuses the survivor-loop cache.
    let harm_strength = spectral_harmonicity_cached(avg_lag, &f2w, &mut spectral_harm_cache, false);

    st.prev_lag = lags[numsubfrs - 1];
    st.prev_pitch_corr = best_pitchcorr;
    st.prev_lagidx = laginds_surv[best_surv][numsubfrs - 1];
    st.prev_lagblk = st.prev_lagidx / (2 * PITCHBLOCK as i32);

    PitchResult {
        pitchcorr: best_pitchcorr,
        lags,
        laginds: laginds_out,
        avg_lag,
        harm_strength,
        blockseg_idx: blocksegs_ix_list[best_surv],
    }
}

fn smpl_maximum(x: &[f32]) -> f32 {
    let mut m = x[0];
    for &v in &x[1..] {
        if v > m {
            m = v;
        }
    }
    m
}

fn get_prev_lag_bias(st: &PitchEstState, lag: f32) -> f32 {
    let lag_diff = (lag - st.prev_lag).abs();
    let diff_thres = PREVWGHT_SPAN * st.prev_lag;
    if lag_diff < diff_thres {
        st.prev_pitch_corr * (1.0 - lag_diff / diff_thres) * PREVWGHT
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    // Feed the reference encoder's exact per-frame ltp_buf + F2 into the estimator (threading the
    // cross-frame predictor + frame-boundary reset) and require the outputs to converge to the
    // reference: pitchcorr (tight float tol), avg_lag, per-subframe laginds, blockseg_idx,
    // harm_strength (cache-aliasing tol). This is the rigorous proof the estimator is faithful.
    #[test]
    fn pitch_estimator_matches_c_ground_truth() {
        let recs: Value =
            serde_json::from_str(include_str!("testdata/pitchio_ground_truth.json")).unwrap();
        let arr = recs.as_array().unwrap();
        assert!(arr.len() >= 30, "expected >=30 records, got {}", arr.len());

        // Thread prev_lag/prev_pitch_corr across frames (the estimator carries these), but seed
        // prev_lagblk/prev_lagidx from the reference dump per frame so the rate-bias survivor selection
        // uses the exact predictor (its reset timing depends on the voiced decision, out of scope for
        // the isolated estimator test).
        let mut st = PitchEstState::default();
        let mut max_pc_err = 0.0f32;
        let mut max_avglag_err = 0.0f32;
        let mut max_harm_err = 0.0f32;
        let mut lagind_mismatch = 0usize;
        let mut bsx_mismatch = 0usize;
        let mut checked = 0usize;
        for rec in arr {
            let _frame = rec["frame"].as_i64().unwrap();
            let cav = rec["cav"].as_i64().unwrap() != 0;
            st.prev_lagblk = rec["prev_lagblk"].as_i64().unwrap() as i32;
            st.prev_lagidx = rec["prev_lagidx"].as_i64().unwrap() as i32;
            let ltp_buf: Vec<f32> = rec["ltp_buf"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap() as f32)
                .collect();
            assert_eq!(ltp_buf.len(), MAX_LTP_BUF_LEN);
            let f2v: Vec<f32> = rec["F2"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap() as f32)
                .collect();
            let mut f2 = [0.0f32; F_LEN];
            f2.copy_from_slice(&f2v);

            let res = smpl_pitch(&mut st, &ltp_buf, &f2, cav);

            if cav {
                let pc_c = rec["pitchcorr"].as_f64().unwrap() as f32;
                let avg_c = rec["avg_lag"].as_f64().unwrap() as f32;
                let harm_c = rec["harm"].as_f64().unwrap() as f32;
                let bsx_c = rec["blockseg_idx"].as_i64().unwrap() as usize;
                let laginds_c: Vec<i32> = rec["laginds"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_i64().unwrap() as i32)
                    .collect();
                max_pc_err = max_pc_err.max((res.pitchcorr - pc_c).abs());
                max_avglag_err = max_avglag_err.max((res.avg_lag - avg_c).abs());
                max_harm_err = max_harm_err.max((res.harm_strength - harm_c).abs());
                let lag_mm = (0..NUM_SUBFRAMES).any(|sf| res.laginds[sf] != laginds_c[sf]);
                if res.blockseg_idx != bsx_c {
                    bsx_mismatch += 1;
                }
                if lag_mm {
                    lagind_mismatch += 1;
                }
                checked += 1;
            }
        }
        assert!(checked >= 20, "too few active frames checked: {checked}");
        assert!(
            max_pc_err < 1e-3,
            "pitchcorr diverges from reference: max_err={max_pc_err}"
        );
        assert!(
            max_avglag_err < 1e-3,
            "avg_lag diverges from reference: max_err={max_avglag_err}"
        );
        assert_eq!(
            lagind_mismatch, 0,
            "per-subframe laginds diverge from reference"
        );
        assert_eq!(bsx_mismatch, 0, "blockseg_idx diverges from reference");
        assert!(
            max_harm_err < 0.05,
            "harm_strength diverges beyond cache-aliasing tol: {max_harm_err}"
        );
    }
}

//! Decoder-side CELP synthesis in the codec's native float domain: the per-subframe loop
//! (excitation, CELP/ACB decode, gen_noise, LPC synthesis) plus the LSF interpolation and the FCB
//! gain tables. Output is float in [-1, 1].
//!
//! Self-contained on purpose (mirrors `smpl_celp.rs`): the leaf helpers are local so the decoder
//! path is decoupled from the encoder.
#![allow(clippy::excessive_precision)]
#![allow(clippy::needless_range_loop)]
#![allow(clippy::too_many_arguments)]

use super::smpl_gennoise::{
    NoiseGenerator, smpl_celp_gen_noise, smpl_decode_resnrg, smpl_get_normalized_bitrate,
};
use std::sync::OnceLock;

const SMPL_LPC_ORDER: usize = 16;
const SMPL_SUBFR_LEN: usize = 80; // 5 ms at 16 kHz, num_subframes==4
const SMPL_NUM_SUBFR: usize = 4;
const SMPL_FRAME_LEN: usize = 320; // 20 ms
const SMPL_LAG_SUBFRLEN: usize = 40;
const SMPL_LTP_INTERPOL_DELAY: usize = 8;
const SMPL_MAX_PITCH_LAG: usize = 320;
const SMPL_ACBG_M: usize = 2;
const SMPL_PITCH_SHARPENING_COEF: f32 = 0.9881;

const SMPL_FCBG_V_N: usize = 34;
const SMPL_UV_GAIN_IDX_LEN: usize = 90;
const SMPL_V_GAIN_MIN_DB: f32 = -100.0;
const SMPL_V_GAIN_STEP_DB: f32 = 3.0;
const SMPL_UV_GAIN_MIN_DB: f32 = -90.0;
const SMPL_UV_GAIN_STEP_DB: f32 = 1.0;

/// ACB high-boost endpoints.
const SMPL_DEC_ACB_HIGH_BOOST: [f32; 2] = [0.35, 0.18];

/// LSF->LPC interpolation factors per subframe, `[lsf_interpol_idx][sf]`.
const SMPL_LSF_INTERPOL_4: [[f32; 4]; 2] = [[0.55, 0.88, 1.0, 1.0], [0.3, 0.65, 0.95, 1.0]];

/// 16-tap symmetric LTP interpolation kernel.
#[rustfmt::skip]
const SMPL_INTERPOL_KERNEL: [f32; 2 * SMPL_LTP_INTERPOL_DELAY] = [
    -6.3925986e-6, 0.00011064114, -0.0009153038, 0.00484772, -0.018698348, 0.05759091, -0.15997477, 0.6170455,
    0.61704546, -0.15997475, 0.057590906, -0.018698348, 0.00484772, -0.0009153038, 0.000110641144, -6.392598e-6,
];

/// Per-subframe ACB-gain codebook (Q14), low-rate and high-rate. Only the high-rate table is
/// exercised by this capture (low_rate==0).
fn acbgains_cb_hr() -> &'static [i16] {
    super::smpl_celp::cb_acbgains_hr_q14()
}
fn acbgains_cb_lr() -> &'static [i16] {
    super::smpl_celp::cb_acbgains_lr_q14()
}

struct FcbGains {
    uv: [f32; SMPL_UV_GAIN_IDX_LEN + 1],
    v: [f32; SMPL_FCBG_V_N],
}

fn fcb_gains() -> &'static FcbGains {
    static T: OnceLock<FcbGains> = OnceLock::new();
    T.get_or_init(|| {
        let mut uv = [0.0f32; SMPL_UV_GAIN_IDX_LEN + 1];
        let mut v = [0.0f32; SMPL_FCBG_V_N];
        for ix in 0..=SMPL_UV_GAIN_IDX_LEN {
            uv[ix] = 10.0f32.powf(0.05 * (ix as f32 * SMPL_UV_GAIN_STEP_DB + SMPL_UV_GAIN_MIN_DB));
        }
        for ix in 0..SMPL_FCBG_V_N {
            v[ix] = 10.0f32.powf(0.05 * (ix as f32 * SMPL_V_GAIN_STEP_DB + SMPL_V_GAIN_MIN_DB));
        }
        FcbGains { uv, v }
    })
}

#[inline]
fn smpl_dot_prod(a: &[f32], b: &[f32], l: usize) -> f32 {
    let mut r = 0.0f32;
    for i in 0..l {
        r += a[i] * b[i];
    }
    r
}

/// Order-16 NLSF (radians) -> LPC `a[0..16]`, a[0]=1.
fn nlsf2a(nlsf: &[f32]) -> [f32; SMPL_LPC_ORDER + 1] {
    let order = SMPL_LPC_ORDER;
    let half = order / 2;
    let cosv: Vec<f64> = nlsf.iter().map(|&x| (x as f64).cos()).collect();
    let mut p = [0f64; SMPL_LPC_ORDER / 2 + 1];
    let mut q = [0f64; SMPL_LPC_ORDER / 2 + 1];
    nlsf_poly(&mut p, &cosv, half, 0);
    nlsf_poly(&mut q, &cosv, half, 1);
    let mut a = [0f32; SMPL_LPC_ORDER + 1];
    a[0] = 1.0;
    for k in 0..half {
        let pt = p[k + 1] + p[k];
        let qt = q[k + 1] - q[k];
        a[k + 1] = (0.5 * (pt + qt)) as f32;
        a[order - k] = (0.5 * (pt - qt)) as f32;
    }
    a
}

fn nlsf_poly(out: &mut [f64], cosv: &[f64], half: usize, parity: usize) {
    out[0] = 1.0;
    out[1] = -2.0 * cosv[parity];
    for k in 1..half {
        let c = -2.0 * cosv[2 * k + parity];
        out[k + 1] = 2.0 * out[k - 1] + c * out[k];
        let mut n = k;
        while n > 1 {
            out[n] += out[n - 2] + c * out[n - 1];
            n -= 1;
        }
        out[1] += c;
    }
}

/// Per-subframe interpolation of the LSF between `prev_lsf` and `lsf`, then NLSF->A. Returns the
/// per-subframe A coefficients and writes the per-subframe interpolated LSF. Mutates `prev_lsf` to
/// the last interpolated LSF (codec carries it across frames).
fn lpc_interpol(
    lsf: &[f32],
    prev_lsf: &mut [f32; SMPL_LPC_ORDER],
    interpol: &[f32; 4],
    a_out: &mut [[f32; SMPL_LPC_ORDER + 1]; SMPL_NUM_SUBFR],
    lsfs_out: &mut [[f32; SMPL_LPC_ORDER]; SMPL_NUM_SUBFR],
) {
    if prev_lsf[SMPL_LPC_ORDER - 1] == 0.0 {
        prev_lsf[..SMPL_LPC_ORDER].copy_from_slice(&lsf[..SMPL_LPC_ORDER]);
    }
    let mut ilsf = [0.0f32; SMPL_LPC_ORDER];
    let mut prev_factor = -1.0f32;
    for j in 0..SMPL_NUM_SUBFR {
        if interpol[j] == prev_factor {
            a_out[j] = a_out[j - 1];
        } else {
            if interpol[j] == 1.0 {
                ilsf.copy_from_slice(&lsf[..SMPL_LPC_ORDER]);
            } else {
                for k in 0..SMPL_LPC_ORDER {
                    ilsf[k] = prev_lsf[k] * (1.0 - interpol[j]) + lsf[k] * interpol[j];
                }
            }
            a_out[j] = nlsf2a(&ilsf);
        }
        prev_factor = interpol[j];
        lsfs_out[j] = ilsf;
    }
    prev_lsf.copy_from_slice(&ilsf);
}

#[inline]
fn acb_dequant(low_rate: bool, acb_idx: i32, acb_g: &mut [f32; SMPL_ACBG_M]) {
    let cb = if low_rate {
        acbgains_cb_lr()
    } else {
        acbgains_cb_hr()
    };
    let sc = 1.0f32 / ((1i32 << 14) as f32);
    for m in 0..SMPL_ACBG_M {
        acb_g[m] = cb[acb_idx as usize * SMPL_ACBG_M + m] as f32 * sc;
    }
}

/// Adjust the ACB gains, then 3-tap symmetric ACB synthesis with high-boost applied.
fn acb_synthesize(
    fcb_subfrlen: usize,
    acb_basis: &[f32],
    acb_g_in: &[f32; SMPL_ACBG_M],
    high_boost: f32,
    acb: &mut [f32],
) {
    let mut acb_g = *acb_g_in;
    if high_boost != 0.0 {
        let f0 = acb_g[0] + 2.0 * acb_g[1];
        let f1 = acb_g[0] - acb_g[1];
        let abs_f2new = (f1.abs() + high_boost).min(f0.abs());
        let f1 = f1 * (abs_f2new / (f1.abs() + 1e-12));
        acb_g[0] = (f0 + 2.0 * f1) / 3.0;
        acb_g[1] = (f0 - f1) / 3.0;
    }
    for i in 0..fcb_subfrlen {
        acb[i] = acb_g[0] * acb_basis[i];
    }
    for i in 0..fcb_subfrlen {
        acb[i] += acb_g[1] * acb_basis[fcb_subfrlen + i];
    }
}

#[inline]
fn pitch_sharp(x: &mut [f32], lag: usize, l: usize) {
    for i in lag..l {
        x[i] += x[i - lag] * SMPL_PITCH_SHARPENING_COEF;
    }
}

/// Build the ACB basis from the excitation history; mutates `state` forward. `state` is the full ACB
/// state; the logical start is `state[state_len - n_lags*40]`.
fn syn_ltp_basis(
    lags: &[f32],
    n_lags: usize,
    state: &mut [f32],
    state_len: usize,
    acb_basis: &mut [f32],
) {
    let mut p = state_len - n_lags * SMPL_LAG_SUBFRLEN;
    for subfr in 0..n_lags {
        let i_lag = lags[subfr].floor() as i32;
        if (i_lag as f32) == lags[subfr] {
            let il = i_lag as usize;
            for i in 0..SMPL_LAG_SUBFRLEN {
                state[p + i] = state[(p + i) - il];
            }
            for i in 0..SMPL_LAG_SUBFRLEN {
                acb_basis[subfr * SMPL_LAG_SUBFRLEN + i] = state[p + i];
            }
            for i in 0..SMPL_LAG_SUBFRLEN {
                let a = state[(p + i) - il - 1];
                let b = state[(p + i) - il + 1];
                acb_basis[(n_lags + subfr) * SMPL_LAG_SUBFRLEN + i] = a + b;
            }
        } else {
            let il = i_lag;
            let base_first = (p as i32) + (-1 - il - SMPL_LTP_INTERPOL_DELAY as i32);
            let first = smpl_dot_prod(
                &state[base_first as usize..],
                &SMPL_INTERPOL_KERNEL,
                2 * SMPL_LTP_INTERPOL_DELAY,
            );
            {
                let src_base = (p as i32) + (-il - SMPL_LTP_INTERPOL_DELAY as i32);
                for nn in 0..SMPL_LAG_SUBFRLEN {
                    let mut ret = 0.0f32;
                    for i in 0..8 {
                        let s0 = state[(src_base + nn as i32 + i as i32) as usize];
                        let s1 = state[(src_base + nn as i32 + 15 - i as i32) as usize];
                        ret += (s0 + s1) * SMPL_INTERPOL_KERNEL[i];
                    }
                    state[p + nn] = ret;
                }
            }
            // The index runs -1 -> 0 -> +SMPL_LAG_SUBFRLEN here, so the tap base for the last sample
            // is p + (SMPL_LAG_SUBFRLEN - i_lag - delay), not -1 of that.
            let base_last =
                (p as i32) + (SMPL_LAG_SUBFRLEN as i32 - il - SMPL_LTP_INTERPOL_DELAY as i32);
            let last = smpl_dot_prod(
                &state[base_last as usize..],
                &SMPL_INTERPOL_KERNEL,
                2 * SMPL_LTP_INTERPOL_DELAY,
            );
            for i in 0..SMPL_LAG_SUBFRLEN {
                acb_basis[subfr * SMPL_LAG_SUBFRLEN + i] = state[p + i];
            }
            let b1 = (n_lags + subfr) * SMPL_LAG_SUBFRLEN;
            acb_basis[b1] = first + state[p + 1];
            for i in 0..SMPL_LAG_SUBFRLEN - 2 {
                acb_basis[b1 + 1 + i] = state[p + i] + state[p + i + 2];
            }
            let i_last = SMPL_LAG_SUBFRLEN - 1;
            acb_basis[b1 + i_last] = state[p + i_last - 1] + last;
        }
        p += SMPL_LAG_SUBFRLEN;
    }
}

/// Voiced branch: add the ACB (LTP) contribution into `lpc_res`, then push the subframe into the ACB
/// state. `acb_state_len = subfrlen + 2*MAX_PITCH_LAG + LTP_INTERPOL_DELAY`.
fn celp_decode(
    acb_state: &mut [f32],
    acb_state_len: usize,
    voiced: bool,
    acb_gain_idx: i32,
    lags: &[f32],
    num_lags: usize,
    subfrlen: usize,
    low_rate: bool,
    normalized_bitrate: f32,
    lpc_res: &mut [f32],
) {
    if voiced {
        let high_boost = SMPL_DEC_ACB_HIGH_BOOST[0]
            + (SMPL_DEC_ACB_HIGH_BOOST[1] - SMPL_DEC_ACB_HIGH_BOOST[0]) * normalized_bitrate;
        let i_lag = lags[num_lags - 1] as i32;
        if low_rate {
            pitch_sharp(lpc_res, i_lag as usize, subfrlen);
        }
        let mut acb_basis = vec![0.0f32; subfrlen * SMPL_ACBG_M];
        let mut acb = vec![0.0f32; subfrlen];
        syn_ltp_basis(lags, num_lags, acb_state, acb_state_len, &mut acb_basis);
        let mut acb_gain = [0.0f32; SMPL_ACBG_M];
        acb_dequant(low_rate, acb_gain_idx, &mut acb_gain);
        acb_synthesize(subfrlen, &acb_basis, &acb_gain, high_boost, &mut acb);
        for i in 0..subfrlen {
            lpc_res[i] += acb[i];
        }
    }
    // Update ACB state: shift left by subfrlen, append this subframe's excitation.
    acb_state.copy_within(subfrlen..acb_state_len - subfrlen, 0);
    acb_state[acb_state_len - 2 * subfrlen..acb_state_len - subfrlen]
        .copy_from_slice(&lpc_res[..subfrlen]);
}

/// AR(16) over one subframe: `y[n] = x[n] - sum_{i} coef[16-i]*y[n-16+i]`. `ybuf` holds a 16-sample
/// history prefix at `ybuf[base-16 .. base]`, so synthesis flows contiguously across the frame
/// (cross-subframe and cross-frame history is the same buffer).
fn filt_ar16(x: &[f32], a: &[f32; SMPL_LPC_ORDER + 1], ybuf: &mut [f32], base: usize, n: usize) {
    for nn in 0..n {
        let mut res = x[nn];
        for i in 0..SMPL_LPC_ORDER {
            res -= a[SMPL_LPC_ORDER - i] * ybuf[base + nn - SMPL_LPC_ORDER + i];
        }
        ybuf[base + nn] = res;
    }
}

/// Per-subframe decoded params the synthesis consumes.
pub(crate) struct CelpDecParams {
    pub(crate) voiced: bool,
    pub(crate) sf_pulses: [i32; SMPL_NUM_SUBFR],
    pub(crate) fcbg_idx: [i32; SMPL_NUM_SUBFR],
    pub(crate) nrgres_dbq_q14: [i32; SMPL_NUM_SUBFR],
    /// ACB gain index per subframe (voiced only).
    pub(crate) acbg_idx: [i32; SMPL_NUM_SUBFR],
    /// Per-40-block pitch lag (codec units: float), 8 per frame, 0 for unvoiced. Synthesis hands the
    /// two blocks of subframe `sf` (`block_lags[2*sf]`, `block_lags[2*sf+1]`) to the ACB/LTP basis, so
    /// fractional intra-subframe lag changes are preserved (lags_per_subframe == 2).
    pub(crate) block_lags: [f32; 2 * SMPL_NUM_SUBFR],
    pub(crate) total_pulses: i32,
}

/// Persistent decoder synthesis state (float domain).
pub(crate) struct CelpDecState {
    noise: NoiseGenerator,
    acb_state: Vec<f32>,
    acb_state_len: usize,
    lpc_synth_mem: [f32; SMPL_LPC_ORDER],
    lsf_prev: [f32; SMPL_LPC_ORDER],
    prev_nrgres: f32,
    /// Post-LPC HP (pitch-harmonic) postfilter state, persistent across the stream.
    hp: super::smpl_harmcomb::HpPostfilterState,
    /// Test-only capture of the per-subframe pre-noise excitation (`exc_pre`), 80/subframe.
    #[cfg(test)]
    pub(crate) dbg_exc_pre: Vec<f32>,
}

impl Default for CelpDecState {
    fn default() -> Self {
        let acb_state_len = SMPL_SUBFR_LEN + 2 * SMPL_MAX_PITCH_LAG + SMPL_LTP_INTERPOL_DELAY;
        CelpDecState {
            noise: NoiseGenerator::default(),
            acb_state: vec![0.0; acb_state_len],
            acb_state_len,
            lpc_synth_mem: [0.0; SMPL_LPC_ORDER],
            lsf_prev: [0.0; SMPL_LPC_ORDER],
            prev_nrgres: 0.0,
            hp: super::smpl_harmcomb::HpPostfilterState::default(),
            #[cfg(test)]
            dbg_exc_pre: Vec::new(),
        }
    }
}

impl CelpDecState {
    /// Synthesize one 20 ms internal frame (4 subframes) into 320 float samples in [-1, 1] via the
    /// subframe loop. `nlsf` is the reconstructed order-16 NLSF (radians); `pulses` are the signed FCB
    /// pulse magnitudes (320 positions). `low_rate` is the TOC bit.
    pub(crate) fn synth_frame(
        &mut self,
        nlsf: &[f32],
        lsf_interpol_idx: usize,
        pulses: &[i32],
        params: &CelpDecParams,
        low_rate: bool,
        frame_length_16: i32,
        out: &mut [f32],
    ) {
        let gains = fcb_gains();
        // Per-subframe LPC interpolation.
        let mut a = [[0.0f32; SMPL_LPC_ORDER + 1]; SMPL_NUM_SUBFR];
        let mut lsfs = [[0.0f32; SMPL_LPC_ORDER]; SMPL_NUM_SUBFR];
        let interpol = &SMPL_LSF_INTERPOL_4[lsf_interpol_idx.min(1)];
        lpc_interpol(nlsf, &mut self.lsf_prev, interpol, &mut a, &mut lsfs);

        let normalized_bitrate = smpl_get_normalized_bitrate(params.total_pulses, frame_length_16);

        // Excitation: sparse FCB pulses scaled by the per-subframe FCB gain.
        let mut lpc_res = [0.0f32; SMPL_FRAME_LEN];
        let gain_tab: &[f32] = if params.voiced { &gains.v } else { &gains.uv };
        for pos in 0..SMPL_FRAME_LEN {
            if pulses[pos] != 0 {
                let sf = pos / SMPL_SUBFR_LEN;
                lpc_res[pos] = pulses[pos] as f32 * gain_tab[params.fcbg_idx[sf] as usize];
            }
        }

        let lags_per_subfr = 2; // 80-sample subframe / 40-sample lag subframe
        // Contiguous synthesis buffer: 16-sample history prefix + 320 frame samples.
        let mut ybuf = [0.0f32; SMPL_LPC_ORDER + SMPL_FRAME_LEN];
        ybuf[..SMPL_LPC_ORDER].copy_from_slice(&self.lpc_synth_mem);
        for sf in 0..SMPL_NUM_SUBFR {
            let base = sf * SMPL_SUBFR_LEN;
            // CELP (ACB/LTP) decode: adds the voiced adaptive-codebook contribution + updates state.
            // With lags_per_subframe == 2 the two 40-blocks of this subframe carry independent lags.
            let sf_lags = [params.block_lags[2 * sf], params.block_lags[2 * sf + 1]];
            celp_decode(
                &mut self.acb_state,
                self.acb_state_len,
                params.voiced,
                params.acbg_idx[sf],
                &sf_lags,
                lags_per_subfr,
                SMPL_SUBFR_LEN,
                low_rate,
                normalized_bitrate,
                &mut lpc_res[base..base + SMPL_SUBFR_LEN],
            );

            #[cfg(test)]
            self.dbg_exc_pre
                .extend_from_slice(&lpc_res[base..base + SMPL_SUBFR_LEN]);

            // Residual noise energy + shaped noise (the dominant unvoiced fix).
            let nrgres = smpl_decode_resnrg(params.nrgres_dbq_q14[sf], SMPL_SUBFR_LEN as i32);
            if !params.voiced {
                self.prev_nrgres = nrgres;
            }

            let mut noise = [0.0f32; 160];
            smpl_celp_gen_noise(
                &mut self.noise,
                &lpc_res[base..base + SMPL_SUBFR_LEN],
                SMPL_SUBFR_LEN,
                params.voiced,
                params.sf_pulses[sf],
                nrgres,
                params.fcbg_idx[sf],
                &lsfs[sf],
                normalized_bitrate,
                &gains.uv,
                &mut noise,
            );
            for i in 0..SMPL_SUBFR_LEN {
                lpc_res[base + i] += noise[i];
            }

            // LPC synthesis (contiguous history across subframes/frames).
            filt_ar16(
                &lpc_res[base..base + SMPL_SUBFR_LEN],
                &a[sf],
                &mut ybuf,
                SMPL_LPC_ORDER + base,
                SMPL_SUBFR_LEN,
            );
        }
        out[..SMPL_FRAME_LEN].copy_from_slice(&ybuf[SMPL_LPC_ORDER..]);
        self.lpc_synth_mem
            .copy_from_slice(&ybuf[SMPL_LPC_ORDER + SMPL_FRAME_LEN - SMPL_LPC_ORDER..]);

        // Post-LPC HP (pitch-harmonic) postfilter (LPC postfilter is off on this stream, tilt
        // postfilter is low_rate-only). The comb lag is the energy-weighted mean of the 8 per-40-block
        // lags (0 -> the default fixed-corner curve, unvoiced).
        let lag = if params.voiced {
            let (mut sl, mut sll) = (0f32, 0f32);
            for &l in &params.block_lags {
                sl += l;
                sll += l * l;
            }
            if sl > 0.0 { sll / sl } else { 0.0 }
        } else {
            0.0
        };
        let mut hp_out = [0f32; SMPL_FRAME_LEN];
        super::smpl_harmcomb::smpl_hp_postfilter(
            &mut self.hp,
            &out[..SMPL_FRAME_LEN],
            SMPL_FRAME_LEN,
            lag,
            &mut hp_out,
        );
        out[..SMPL_FRAME_LEN].copy_from_slice(&hp_out);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voip::mlow::smpl_cc_tables::load_cc_tables;
    use crate::voip::mlow::smpl_decode::{SmplLsfState, decode_smpl_lsf, load_smpl_tables};
    use crate::voip::mlow::smpl_gains::decode_smpl_gains;
    use crate::voip::mlow::smpl_mem::load_smpl_mem;
    use crate::voip::mlow::smpl_pitch::decode_smpl_pitch;
    use crate::voip::mlow::smpl_pulse::decode_smpl_pulses;
    use crate::voip::mlow::smpl_synth::{load_smpl_synth_tables, smpl_reconstruct_nlsf};
    use serde_json::Value;

    /// Validate the pre-noise excitation (FCB pulses * gain + voiced ACB) against the reference
    /// `exc_pre` dump, per subframe. This proves the excitation domain (`fcbgains_uv/v[fcbg_idx]`) and
    /// the voiced ACB/LTP synthesis are faithful, independent of the PRNG-driven noise.
    #[test]
    fn exc_pre_matches_c() {
        let recs: Value = serde_json::from_str(include_str!("testdata/exc_pre_lags.json")).unwrap();
        let carr = recs.as_array().unwrap();

        // Key the reference exc_pre by (packet, frame, sf).
        use std::collections::HashMap;
        let mut cmap: HashMap<(i64, i64, i64), &Value> = HashMap::new();
        for c in carr {
            cmap.insert(
                (
                    c["packet"].as_i64().unwrap(),
                    c["frame"].as_i64().unwrap(),
                    c["sf"].as_i64().unwrap(),
                ),
                c,
            );
        }

        let frames: Vec<String> =
            serde_json::from_str(include_str!("testdata/inbound_capture_frames.json")).unwrap();
        let tbl = load_smpl_tables();
        let synth_t = load_smpl_synth_tables();
        let mem = load_smpl_mem();
        let cc = load_cc_tables();
        let mut lstate = SmplLsfState::default();
        let mut celp = CelpDecState::default();
        let mut prev_nlsf: Vec<f32> = Vec::new();

        let (mut uv_ok, mut uv_bad, mut v_ok, mut v_bad) = (0, 0, 0, 0);
        let mut worst = 0f32;
        for (packet, hex_frame) in frames.iter().enumerate() {
            let frame = hex::decode(hex_frame).unwrap();
            if frame.is_empty() {
                continue;
            }
            let toc = crate::voip::mlow::toc::parse_mlow_toc(frame[0]);
            if toc.std_opus || toc.sid || !toc.active {
                continue;
            }
            let config = (frame[0] >> 2) as usize & 1;
            let low_rate = (frame[0] >> 2) & 1 != 0;
            let mut dec = crate::voip::mlow::rangecoder::RangeDecoder::new(&frame[1..]);
            for f in 0..3 {
                let lsf = decode_smpl_lsf(&mut dec, tbl, &mut lstate, config, f);
                let pulses = decode_smpl_pulses(&mut dec, cc, 320, 4, 1, config as i32, lsf.stage1);
                let voiced = lsf.stage1 == 1;
                let mut params = CelpDecParams {
                    voiced,
                    sf_pulses: pulses.subfr,
                    fcbg_idx: [0; 4],
                    nrgres_dbq_q14: [0; 4],
                    acbg_idx: [0; 4],
                    block_lags: [0.0; 8],
                    total_pulses: pulses.subfr.iter().sum(),
                };
                if voiced {
                    let pr = decode_smpl_pitch(
                        &mut dec,
                        mem,
                        cc,
                        &mut lstate,
                        320,
                        4,
                        config as i32,
                        pulses.subfr,
                    );
                    for b in 0..8 {
                        params.block_lags[b] =
                            ((pr.block_lags[b] as f64 * 0.5 + 32.0).min(320.0)) as f32;
                    }
                    for sf in 0..4 {
                        params.acbg_idx[sf] = pr.gain_idx[sf];
                        params.fcbg_idx[sf] = pr.filt_idx[sf].max(0);
                    }
                } else {
                    let g = decode_smpl_gains(&mut dec, cc, 4, pulses.subfr);
                    params.nrgres_dbq_q14 = g.gain_q;
                    params.fcbg_idx = g.nrg_res;
                }
                let nlsf = smpl_reconstruct_nlsf(
                    synth_t,
                    lsf.stage1 as usize,
                    config,
                    lsf.grid as usize,
                    &lsf.stage2,
                    &prev_nlsf,
                );
                celp.dbg_exc_pre.clear();
                let mut sig = [0f32; SMPL_FRAME_LEN];
                celp.synth_frame(
                    &nlsf,
                    lsf.extra as usize,
                    &pulses.pulses,
                    &params,
                    low_rate,
                    320,
                    &mut sig,
                );
                prev_nlsf = nlsf;
                // Compare each subframe's exc_pre to the reference.
                for sf in 0..4 {
                    let Some(c) = cmap.get(&(packet as i64, f as i64, sf as i64)) else {
                        continue;
                    };
                    // Cross-check that our reconstructed per-block lags equal the reference dump's two
                    // lags for this subframe (the decode that drives the ACB/LTP basis).
                    if voiced {
                        let clags = c["lags"].as_array().unwrap();
                        let c0 = clags[0].as_f64().unwrap() as f32;
                        let c1 = clags[1].as_f64().unwrap() as f32;
                        assert_eq!(
                            (params.block_lags[2 * sf], params.block_lags[2 * sf + 1]),
                            (c0, c1),
                            "per-block lags diverge at pkt={packet} f={f} sf={sf}"
                        );
                    }
                    // Reconstruct the dense reference exc_pre from the sparse nonzero list.
                    let mut cexc = [0f32; SMPL_SUBFR_LEN];
                    for pair in c["nz"].as_array().unwrap() {
                        let p = pair.as_array().unwrap();
                        let idx = p[0].as_u64().unwrap() as usize;
                        cexc[idx] = p[1].as_f64().unwrap() as f32;
                    }
                    let base = sf * SMPL_SUBFR_LEN;
                    let mut bad = false;
                    for i in 0..SMPL_SUBFR_LEN {
                        let d = (celp.dbg_exc_pre[base + i] - cexc[i]).abs();
                        worst = worst.max(d);
                        // Excitation amplitudes are ~1e-4; a tight absolute tolerance.
                        if d > 2e-5 {
                            bad = true;
                        }
                    }
                    if voiced {
                        if bad { v_bad += 1 } else { v_ok += 1 }
                    } else if bad {
                        uv_bad += 1
                    } else {
                        uv_ok += 1
                    }
                }
            }
        }
        eprintln!(
            "exc_pre vs reference: unvoiced ok={uv_ok} bad={uv_bad}; voiced ok={v_ok} bad={v_bad}; worst abs diff={worst:.2e}"
        );
        // Unvoiced excitation is deterministic (pulses * fcbgains_uv), so it must match.
        assert_eq!(
            uv_bad, 0,
            "unvoiced exc_pre diverges from reference ({uv_bad} subframes)"
        );
        // Voiced excitation (FCB pulses * gain + ACB/LTP per-block-lag synthesis) is also deterministic.
        assert_eq!(
            v_bad, 0,
            "voiced exc_pre diverges from reference ({v_bad} subframes)"
        );
    }
}

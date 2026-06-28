//! MLow (smpl_audio_codec) SYNTHESIS LB-core: turns LB parameters (LSF, pulses, LTP, gains) into
//! 16 kHz PCM. Implements WASM func 3597's low-band core (NLSF reconstruct, NLSF2A, pulse excitation
//! times gain, fractional LTP, order-16 LPC synthesis). `synth_internal_frame` is the
//! encoder's analysis-by-synthesis pass (driven from `analysis.rs`), not the decoder's playout. The
//! decoder's real HP pitch-harmonic postfilter is the ungated `smpl_hp_postfilter` call in
//! `smpl_celpdec.rs`; the per-subframe excitation comb here is unused at the current operating point.
//!
//! The float constants below are byte-exact copies of the reference f32 literals (the extra digits
//! round to the same f32 the WASM uses), so excessive-precision is allowed module-wide.
#![allow(clippy::excessive_precision)]

pub(crate) const SMPL_ORDER: usize = 16;
pub(crate) const SMPL_SUBFR_LEN: usize = 80; // 5 ms @ 16 kHz
pub(crate) const SMPL_INTF_LEN: usize = 320; // 20 ms internal frame
pub(crate) const SMPL_SUBFR_COUNT: usize = 4;
pub(crate) const SMPL_LTP_HIST: usize = 728;
const SMPL_FRAC_STATE_LEN: i32 = 728;
pub(crate) const SMPL_VOICED_NORM_GAIN: f64 = 1.0;
const LTP_HIST_LEN: usize = SMPL_LTP_HIST + SMPL_INTF_LEN + 64;

const SMPL_NLSF_WEIGHT_W_MAX: f32 = 999.9999;
const SMPL_NLSF_WEIGHT_EPS: f32 = 0.0009999999;
const SMPL_PI_F32: f32 = 3.1415927410125;
const SMPL_STABILIZE_MAX_LOOPS: i32 = 1000;
const SMPL_STABILIZE_EPS: f32 = 9.5367431640625e-07;

/// 16-tap symmetric fractional-delay interpolation FIR (WASM mem 0xe8780, func 3523/3507).
const SMPL_FIR16: [f32; 16] = [
    -0.000006392598606907995,
    0.00011064113641623408,
    -0.0009153038263320923,
    0.0048477197997272015,
    -0.018698347732424736,
    0.05759090930223465,
    -0.15997476875782013,
    0.617045521736145,
    0.6170454621315002,
    -0.15997475385665894,
    0.05759090557694435,
    -0.018698347732424736,
    0.0048477197997272015,
    -0.0009153038263320923,
    0.00011064114369219169,
    -0.0000063925981521606445,
];

pub(crate) struct SmplSynthTables {
    /// `[stage1][config][grid][coeff]` -> per-symbol NLSF residual values.
    pub(crate) valtables: Vec<Vec<Vec<Vec<Vec<f32>>>>>,
    /// `[stage1][grid]` -> 16 base NLSF (radians, half-scale).
    pub(crate) centroids: Vec<Vec<Vec<f32>>>,
    /// `[stage1][grid]` -> 16x16 decorrelation matrix (`mat[row][col]`).
    pub(crate) matrices: Vec<Vec<Vec<Vec<f32>>>>,
    /// `[stage1]` -> 17 NLSF stabilize minimum spacings.
    pub(crate) min_spacing: Vec<Vec<f32>>,
    /// grid==16 base NLSF tables (selected INVERTED by signal type).
    pub(crate) grid16_w: Vec<Vec<f32>>,
    pub(crate) grid16_alpha: Vec<f32>,
    /// `[sig][config]` -> 256-entry flat column-major grid==16 decorrelation matrix.
    pub(crate) grid16_matrices: Vec<Vec<Vec<f32>>>,
}

pub(crate) fn load_smpl_synth_tables() -> &'static SmplSynthTables {
    &super::smpl_lsf_seed::lsf_built().synth
}

/// func 3530 (silk_NLSF_VQ_weights_laroia): inverse-gap weights w[k] = invgap[k] + invgap[k+1].
fn smpl_nlsf_laroia_weights(nlsf: &[f32], out: &mut [f32]) {
    let mut inv = [0f32; SMPL_ORDER + 1];
    let clamp = |gap: f32| -> f32 {
        if gap > SMPL_NLSF_WEIGHT_EPS {
            1.0 / gap
        } else {
            SMPL_NLSF_WEIGHT_W_MAX
        }
    };
    inv[0] = clamp(nlsf[0]);
    let mut prev = nlsf[0];
    for k in 1..SMPL_ORDER {
        let gap = nlsf[k] - prev;
        inv[k] = clamp(gap);
        prev = nlsf[k];
    }
    inv[SMPL_ORDER] = clamp(SMPL_PI_F32 - nlsf[SMPL_ORDER - 1]);
    for k in 0..SMPL_ORDER {
        out[k] = inv[k] + inv[k + 1];
    }
}

/// func 3485 (decorrelation-matrix apply): out[r] = sum_c mat[c*16 + r] * vec[c] (column-major mat).
fn smpl_nlsf_decorr(mat: &[f32], vec: &[f32], out: &mut [f32]) {
    let mut scr = [0f32; SMPL_ORDER];
    let v0 = vec[0];
    for r in 0..SMPL_ORDER {
        scr[r] = v0 * mat[r];
    }
    for (c, &v) in vec.iter().enumerate().take(SMPL_ORDER).skip(1) {
        let base = c * SMPL_ORDER;
        for r in 0..SMPL_ORDER {
            scr[r] += mat[base + r] * v;
        }
    }
    out[..SMPL_ORDER].copy_from_slice(&scr);
}

/// Reconstruct the order-16 NLSF (radians) from the decoded LSF indices (f3597 1437..2002).
pub(crate) fn smpl_reconstruct_nlsf(
    t: &SmplSynthTables,
    stage1: usize,
    config: usize,
    grid: usize,
    stage2: &[i32; 16],
    prev_nlsf: &[f32],
) -> Vec<f32> {
    let val = &t.valtables[stage1][config][grid];
    let mut resid = [0f32; SMPL_ORDER];
    for (k, r) in resid.iter_mut().enumerate() {
        let sym = stage2[k];
        if sym >= 0 && (sym as usize) < val[k].len() {
            *r = val[k][sym as usize];
        }
    }

    let mut out = vec![0f32; SMPL_ORDER];
    if grid == 16 {
        // grid==16: interpolate base between prevNLSF and the inverted grid16 base table.
        let mut base = [0f32; SMPL_ORDER];
        let base_tbl = &t.grid16_w[1 - stage1];
        let alpha = t.grid16_alpha[stage1];
        for k in 0..SMPL_ORDER {
            let pv = if k < prev_nlsf.len() {
                prev_nlsf[k]
            } else {
                0.0
            };
            base[k] = pv + alpha * (base_tbl[k] - pv);
        }
        let mut w = [0f32; SMPL_ORDER];
        smpl_nlsf_laroia_weights(&base, &mut w);
        for wk in w.iter_mut() {
            *wk = (*wk as f64).sqrt() as f32;
        }
        let mut decorr = [0f32; SMPL_ORDER];
        let mat16 = &t.grid16_matrices[stage1][config];
        smpl_nlsf_decorr(mat16, &resid, &mut decorr);
        for k in 0..SMPL_ORDER {
            out[k] = base[k] + decorr[k] / w[k];
        }
        smpl_stabilize_nlsf(&mut out, &t.min_spacing[stage1]);
        return out;
    }

    // matrix case (grid < 16): NLSF[r] = 2*centroid[r] + sum_c mat[c][r]*resid[c].
    let cent = &t.centroids[stage1][grid];
    let mat = &t.matrices[stage1][grid];
    for r in 0..SMPL_ORDER {
        let mut acc = 2.0 * cent[r];
        for c in 0..SMPL_ORDER {
            acc += mat[c][r] * resid[c];
        }
        out[r] = acc;
    }
    smpl_stabilize_nlsf(&mut out, &t.min_spacing[stage1]);
    out
}

/// func 3533 (silk_NLSF_stabilize): enforce minimum spacing + ordering in the margin domain.
fn smpl_stabilize_nlsf(nlsf: &mut [f32], min_spacing: &[f32]) {
    const PI: f32 = SMPL_PI_F32;
    const L: usize = SMPL_ORDER;
    let mut marg = [0f32; L + 1];
    marg[0] = nlsf[0] - min_spacing[0];
    for i in 1..L {
        marg[i] = nlsf[i] - nlsf[i - 1] - min_spacing[i];
    }
    marg[L] = PI - nlsf[L - 1] - min_spacing[L];

    let argmin = |marg: &[f32; L + 1]| -> (f32, usize) {
        let mut m = marg[0];
        let mut idx = 0;
        for (i, &v) in marg.iter().enumerate().take(L + 1).skip(1) {
            if v < m {
                m = v;
                idx = i;
            }
        }
        (m, idx)
    };
    let (mut min, mut sel) = argmin(&marg);
    let mut loop_n = 0i32;
    while min < 0.0 {
        let d = loop_n as f32 * SMPL_STABILIZE_EPS - min;
        if sel == 0 {
            marg[0] += d;
            marg[1] -= d;
        } else if sel == L {
            marg[L] += d;
            marg[L - 1] -= d;
        } else {
            marg[sel] += d;
            let half = d * 0.5;
            marg[sel - 1] -= half;
            marg[sel + 1] -= half;
        }
        let (m, s) = argmin(&marg);
        min = m;
        sel = s;
        if min < 0.0 {
            loop_n += 1;
            if loop_n == SMPL_STABILIZE_MAX_LOOPS {
                break;
            }
        }
    }
    nlsf[0] = min_spacing[0] + marg[0];
    let mut run = nlsf[0];
    for i in 1..L {
        run = run + marg[i] + min_spacing[i];
        nlsf[i] = run;
    }
}

/// func 3513 (silk_NLSF2A): order-16 NLSF (radians) -> LPC coefficients a[0..16], a[0]=1.0.
pub(crate) fn smpl_nlsf2a(nlsf: &[f32]) -> Vec<f32> {
    let order = nlsf.len();
    let half = order / 2;
    let cosv: Vec<f64> = nlsf.iter().map(|&x| (x as f64).cos()).collect();
    let mut p = vec![0f64; half + 1];
    let mut q = vec![0f64; half + 1];
    smpl_nlsf_poly(&mut p, &cosv, half, 0);
    smpl_nlsf_poly(&mut q, &cosv, half, 1);

    let mut a = vec![0f32; order + 1];
    a[0] = 1.0;
    for k in 0..half {
        let pt = p[k + 1] + p[k];
        let qt = q[k + 1] - q[k];
        a[k + 1] = (0.5 * (pt + qt)) as f32;
        a[order - k] = (0.5 * (pt - qt)) as f32;
    }
    a
}

fn smpl_nlsf_poly(out: &mut [f64], cosv: &[f64], half: usize, parity: usize) {
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

/// func 3503 (order-16 LPC synthesis): out[n] = ex[n] - sum_{j=1..16} a[j]*out[n-j]. `state` holds
/// the previous `order` outputs (carried across subframes/frames), updated in place.
fn smpl_lpc_synthesis(ex: &[f32], a: &[f32], out: &mut [f32], state: &mut [f32]) {
    let order = SMPL_ORDER;
    for n in 0..ex.len() {
        let mut acc = ex[n] as f64;
        for j in 1..=order {
            let prev = if n >= j {
                out[n - j] as f64
            } else {
                state[order + n - j] as f64
            };
            acc -= a[j] as f64 * prev;
        }
        out[n] = acc as f32;
    }
    if out.len() >= order {
        state[..order].copy_from_slice(&out[out.len() - order..]);
    }
}

/// func 3597 gain: quantized log-gain -> linear gain (fast pow2 bit-cast).
pub(crate) fn smpl_gain_lin(gain_q: i32) -> f64 {
    let y = gain_q as f32 * 6.103515625e-05 * 0.10000000149011612 * 27749388.0 + 1064866816.0;
    let i: i32 = if y < 2147483648.0 && y > -2147483648.0 {
        y as i32
    } else {
        -2147483648
    };
    let mut f = f32::from_bits(i as u32) - 3.1622775509276835e-09;
    if f < 0.0 {
        f = 0.0;
    }
    f as f64
}

fn smpl_floor_f32(x: f32) -> f32 {
    let mut i = x as i32;
    if i as f32 > x {
        i -= 1;
    }
    i as f32
}

fn abs_f32(x: f32) -> f32 {
    if x < 0.0 { -x } else { x }
}

/// func 3507: 8-tap symmetric FIR16 application, IN-PLACE over `sig` (the WASM passes in==out, so
/// overlapping read/write regions must see prior writes). f32 accumulation order matches the WASM.
fn smpl_fir8(sig: &mut [f32], in_base: i32, out_base: i32, cnt: i32) {
    for jj in 0..cnt {
        let mut acc = 0f32;
        for i in 0..8 {
            acc += (sig[(in_base + jj + i) as usize] + sig[(in_base + jj + 15 - i) as usize])
                * SMPL_FIR16[i as usize];
        }
        sig[(out_base + jj) as usize] = acc;
    }
}

/// func 3523 (fractional LTP + interpolation). Reads `sig` backward from `sig_end` and writes two
/// regions per subframe into `out` (len 2*num_subfr*40); also mutates `sig` in place.
fn smpl_frac_ltp(
    lag: &[f32],
    num_subfr: i32,
    sig: &mut [f32],
    sig_end: i32,
    state_len: i32,
    out: &mut [f32],
) {
    let mut lb = sig_end - (40 * num_subfr - state_len);
    for sf in 0..num_subfr {
        let fl = smpl_floor_f32(lag[sf as usize]);
        let int_lag = fl as i32;
        if int_lag as f32 == lag[sf as usize] {
            for k in 0..40 {
                sig[(lb + k) as usize] = sig[(lb + k - int_lag) as usize];
            }
            for k in 0..40 {
                out[(sf * 40 + k) as usize] = sig[(lb + k) as usize];
                out[((num_subfr + sf) * 40 + k) as usize] =
                    sig[(lb + k - int_lag - 1) as usize] + sig[(lb + k - int_lag + 1) as usize];
            }
        } else {
            let b = (num_subfr + sf) * 40;
            for k in 0..40 {
                out[(b + k) as usize] =
                    sig[(lb - int_lag - 1 + k) as usize] + sig[(lb - int_lag + 1 + k) as usize];
            }
            let mut l10 = 0f32;
            for j in 0..16 {
                l10 += sig[(lb - 9 - int_lag + j) as usize] * SMPL_FIR16[j as usize];
            }
            smpl_fir8(sig, lb - int_lag - 8, lb, 40);
            let mut l11 = 0f32;
            for j in 0..16 {
                l11 += sig[(lb + 32 - int_lag + j) as usize] * SMPL_FIR16[j as usize];
            }
            for k in 0..40 {
                out[(sf * 40 + k) as usize] = sig[(lb + k) as usize];
            }
            out[b as usize] = l10 + sig[(lb + 1) as usize];
            for k in 0..38 {
                out[(b + 1 + k) as usize] = sig[(lb + k) as usize] + sig[(lb + 2 + k) as usize];
            }
            out[(b + 39) as usize] = l11 + sig[(lb + 38) as usize];
        }
        lb += 40;
    }
}

/// func 3522 (per-subframe LTP gain-apply) state, reset per internal frame.
#[derive(Default, Clone, Copy)]
pub(crate) struct SmplExcGainState {
    s0: f32,
    s1: f32,
}

fn smpl_exc_gain_apply(
    sub_len: usize,
    input: &[f32],
    st: &mut SmplExcGainState,
    out: &mut [f32],
    gain: f32,
) {
    if gain != 0.0 {
        let s5 = st.s1;
        let s6 = (s5 + s5) + st.s0;
        let d = st.s0 - s5;
        let abs_d = abs_f32(d);
        let abs_s6 = abs_f32(s6);
        let mut mn = abs_d + gain;
        if abs_s6 < mn {
            mn = abs_s6;
        }
        let t = d * mn / (abs_d + 1e-12);
        st.s1 = (s6 - t) / 3.0;
        st.s0 = (2.0 * t + s6) / 3.0;
    }
    if sub_len == 0 {
        return;
    }
    let s0 = st.s0;
    for n in 0..sub_len {
        out[n] = s0 * input[n];
    }
    let s1 = st.s1;
    for n in 0..sub_len {
        out[n] += s1 * input[sub_len + n];
    }
}

/// func 3597 offset 4342: fractional-LTP gain = normGain*-0.17 + 0.35.
pub(crate) fn smpl_ltp_frac_gain(norm_gain: f64) -> f32 {
    norm_gain as f32 * -0.16999998688697815 + 0.3499999940395355
}

/// One 80-sample subframe's fractional-LTP prediction (func 3523 + func 3522).
pub(crate) fn smpl_ltp_subframe_pred(
    hist: &mut [f32],
    hist_pos: i32,
    lag_f: f32,
    gain_frac: f32,
    gst: &mut SmplExcGainState,
    pred_out: &mut [f32],
) {
    let mut frac_out = [0f32; 2 * 2 * 40];
    let lags = [lag_f, lag_f];
    smpl_frac_ltp(
        &lags,
        2,
        hist,
        hist_pos - 648,
        SMPL_FRAC_STATE_LEN,
        &mut frac_out,
    );
    smpl_exc_gain_apply(SMPL_SUBFR_LEN, &frac_out, gst, pred_out, gain_frac);
}

/// LTP parameters the synthesis needs for one internal frame.
#[derive(Clone)]
pub(crate) struct SmplPitchSynth {
    pub(crate) voiced: bool,
    pub(crate) lag_subfr: [f64; 4], // per-subframe func-3523 lag = intLagQ6[sf]*0.5 + 32
    pub(crate) norm_gain: f64,
}

/// Cross-internal-frame synthesis state (LPC + LTP/excitation history + the post-LPC band tail).
#[derive(Clone)]
pub(crate) struct SmplFrameSynth {
    lpc_state: [f32; SMPL_ORDER],
    ltp_hist: Vec<f32>,
    gst: SmplExcGainState,
}

impl Default for SmplFrameSynth {
    fn default() -> Self {
        SmplFrameSynth {
            lpc_state: [0.0; SMPL_ORDER],
            ltp_hist: vec![0.0; LTP_HIST_LEN],
            gst: SmplExcGainState::default(),
        }
    }
}

/// Turn one 20 ms internal frame's decoded parameters into 320 LB PCM samples (float, ~int16-scaled).
/// Returns (signal, nlsf); `nlsf` becomes the next frame's `prev_nlsf`.
#[allow(clippy::too_many_arguments)]
pub(crate) fn synth_internal_frame(
    t: &SmplSynthTables,
    st: &mut SmplFrameSynth,
    stage1: usize,
    config: usize,
    grid: usize,
    stage2: &[i32; 16],
    prev_nlsf: &[f32],
    pulses: &[i32],
    gain_q: &[i32; 4],
    pitch: &SmplPitchSynth,
) -> (Vec<f32>, Vec<f32>) {
    let nlsf = smpl_reconstruct_nlsf(t, stage1, config, grid, stage2, prev_nlsf);
    let a = smpl_nlsf2a(&nlsf);

    let sub_gain = |sf: usize| -> f64 {
        let gq = if sf < gain_q.len() { gain_q[sf] } else { 0 };
        smpl_gain_lin(gq) * SMPL_SUBFR_LEN as f64
    };

    let mut ex = vec![0f32; SMPL_INTF_LEN];
    for n in 0..SMPL_INTF_LEN {
        ex[n] = (pulses[n] as f64 * sub_gain(n / SMPL_SUBFR_LEN)) as f32;
    }
    let hist = &mut st.ltp_hist;

    if pitch.voiced {
        const G_LTP: f32 = 0.949999988079071;
        let gain_frac = smpl_ltp_frac_gain(pitch.norm_gain);
        let mut pred_out = vec![0f32; SMPL_SUBFR_LEN];
        st.gst = SmplExcGainState::default();
        for sf in 0..SMPL_SUBFR_COUNT {
            let lag_f = pitch.lag_subfr[sf] as f32;
            let int_lag = lag_f as i32;
            if int_lag <= 0 {
                let from = sf * SMPL_SUBFR_LEN;
                let to = (sf + 1) * SMPL_SUBFR_LEN;
                hist[SMPL_LTP_HIST + from..SMPL_LTP_HIST + to].copy_from_slice(&ex[from..to]);
                continue;
            }
            let ex_base = sf * SMPL_SUBFR_LEN;
            let hist_pos = (SMPL_LTP_HIST + ex_base) as i32;
            if int_lag > 0 && (int_lag as usize) < SMPL_SUBFR_LEN {
                for n in (int_lag as usize)..SMPL_SUBFR_LEN {
                    ex[ex_base + n] += G_LTP * ex[ex_base + n - int_lag as usize];
                }
            }
            smpl_ltp_subframe_pred(hist, hist_pos, lag_f, gain_frac, &mut st.gst, &mut pred_out);
            for n in 0..SMPL_SUBFR_LEN {
                ex[ex_base + n] += pred_out[n];
            }
            hist[hist_pos as usize..hist_pos as usize + SMPL_SUBFR_LEN]
                .copy_from_slice(&ex[ex_base..ex_base + SMPL_SUBFR_LEN]);
        }
    } else {
        hist[SMPL_LTP_HIST..SMPL_LTP_HIST + SMPL_INTF_LEN].copy_from_slice(&ex);
    }

    let mut out = vec![0f32; SMPL_INTF_LEN];
    smpl_lpc_synthesis(&ex, &a, &mut out, &mut st.lpc_state);

    // roll the LTP history forward by one internal frame; clear the forward margin.
    hist.copy_within(SMPL_INTF_LEN..SMPL_LTP_HIST + SMPL_INTF_LEN, 0);
    for v in hist
        .iter_mut()
        .take(LTP_HIST_LEN)
        .skip(SMPL_LTP_HIST + SMPL_INTF_LEN)
    {
        *v = 0.0;
    }
    (out, nlsf)
}

/// Cross-frame decoder state (the persistent LSF/pitch predictor, prev NLSF, CELP synthesis).
#[derive(Default)]
pub(crate) struct SmplDecoderState {
    pub(crate) lstate: super::smpl_decode::SmplLsfState,
    pub(crate) prev_nlsf: Vec<f32>,
    /// C-float-domain CELP synthesis state (excitation + ACB + gen_noise + LPC synth).
    pub(crate) celp: super::smpl_celpdec::CelpDecState,
    /// Per-packet harmonic postfilter state (runs once per packet after all internal frames).
    pub(crate) harm: super::smpl_harm_postfilter::HarmPostfilterState,
}

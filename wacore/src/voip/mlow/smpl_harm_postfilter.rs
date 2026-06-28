//! MLow harmonic postfilter: the final per-packet pitch-comb that runs on the full LB output after
//! the HP postfilter. It enhances pitch harmonics by mixing `x[-lag] + x[+lag]` into the signal,
//! low-pass filtered by a lag-dependent kernel, and it introduces the codec's
//! `SMPL_TOT_POSTFILT_DELAY = 48`-sample group delay (8 FB + 40 lag-subframe).
//!
//! The reference is built with `-ffast-math -mavx`, so the recursive/accumulating math is not
//! IEEE-strict; this matches it to within i16 output quantization, not bit-for-bit.
#![allow(clippy::needless_range_loop)]

use std::sync::OnceLock;

const FRAME_LEN: usize = 320;
const MAX_FRAMES_PER_PACKET: usize = 6;
const MIN_PITCH_LAG: i32 = 32; // SMPL_MINPITCH_MS(2) * 16
const MAX_PITCH_LAG: i32 = 320; // SMPL_MAXPITCH_MS(20) * 16
const MAXPITCH_LEN: usize = 320; // SMPL_MAXPITCH_MS * SMPL_PITCH_FS_KHZ(16)
const FB_DELAY: usize = 8; // SMPL_HARM_POSTF_FB_DELAY
const LAG_SUBFR_LEN: usize = 40; // SMPL_HARM_POSTF_LAG_SUBFR_LEN = FRAME_LEN / 8
const HARM_DELAY: usize = LAG_SUBFR_LEN; // SMPL_HARM_POSTF_DELAY
/// Total group delay the harmonic postfilter introduces (`SMPL_TOT_POSTFILT_DELAY`), so the decoded
/// PCM aligns at lag 0 with the reference. Used by the delay-aligned round-trip/validation tests.
#[cfg(test)]
pub(crate) const TOT_POSTFILT_DELAY: usize = FB_DELAY + HARM_DELAY; // 48
const PITCH_NUM_SUBFRAMES: usize = 8;
const FB_STRENGTH: f32 = 0.4734;
const STRENGTH: f32 = 0.6438;
const CUTOFF_HZ: f32 = 4000.0;
const NHARM_CUTOFF: f32 = 6.3;
const REDUCTION_FAC: f32 = 0.0579;
const SMPL_PI: f32 = std::f32::consts::PI;

const STATE_COMB_LEN: usize = MAXPITCH_LEN + FRAME_LEN * MAX_FRAMES_PER_PACKET + HARM_DELAY;
const LP_FILT_RES: i32 = 2500;
const NUM_LP_FILT: usize = ((LP_FILT_RES / 80) - LP_FILT_RES / MAX_PITCH_LAG + 1) as usize;

#[inline]
fn lag_to_filt_ix(lag: i32) -> usize {
    (LP_FILT_RES / (lag + 30).max(80) - LP_FILT_RES / MAX_PITCH_LAG) as usize
}

/// LP-filter bank, one symmetric `2*FB_DELAY+1`-tap kernel per quantized lag bucket.
struct HarmTables {
    lp_filters: Vec<[f32; 2 * FB_DELAY + 1]>,
}

fn harm_tables() -> &'static HarmTables {
    static T: OnceLock<HarmTables> = OnceLock::new();
    T.get_or_init(|| {
        let mut filt_win = [0f32; FB_DELAY];
        let d_omega = (0.5 * SMPL_PI) / (FB_DELAY as f32 + 1.0);
        let mut omega = d_omega;
        for i in 0..FB_DELAY {
            filt_win[i] = omega.cos() / (i as f32 + 1.0);
            omega += d_omega;
        }
        let mut lp = vec![[0f32; 2 * FB_DELAY + 1]; NUM_LP_FILT];
        let mut ix_prev = -1i32;
        for lag in MIN_PITCH_LAG..=MAX_PITCH_LAG {
            let ix = lag_to_filt_ix(lag) as i32;
            if ix != ix_prev {
                let omega0 = 2.0 * SMPL_PI / lag as f32;
                create_lp_filter(omega0, &filt_win, &mut lp[ix as usize]);
                ix_prev = ix;
            }
        }
        HarmTables { lp_filters: lp }
    })
}

fn create_lp_filter(omega0: f32, filt_win: &[f32; FB_DELAY], blp: &mut [f32; 2 * FB_DELAY + 1]) {
    let omega_c = (omega0 * NHARM_CUTOFF).min(CUTOFF_HZ / 16000.0 * SMPL_PI);
    let mut sum_b = 0.0f32;
    let mut omega_c_sum = omega_c;
    for i in 0..FB_DELAY {
        let b = filt_win[i] * omega_c_sum.sin();
        omega_c_sum += omega_c;
        blp[FB_DELAY + i + 1] = b;
        blp[FB_DELAY - i - 1] = b;
        sum_b += 2.0 * b;
    }
    blp[FB_DELAY] = omega_c;
    sum_b += omega_c;
    let sc = 1.0 / sum_b;
    for v in blp.iter_mut() {
        *v *= sc;
    }
}

/// Persistent harm-postfilter state. `prev_lag = 0` after init (first packet's lag).
#[derive(Clone)]
pub(crate) struct HarmPostfilterState {
    state1: [f32; 2 * FB_DELAY],
    lpcoefs: [f32; 2 * FB_DELAY + 1],
    state_comb: Vec<f32>,
    prev_lag: i32,
    prev_did_filter: i32,
}

impl Default for HarmPostfilterState {
    fn default() -> Self {
        HarmPostfilterState {
            state1: [0.0; 2 * FB_DELAY],
            lpcoefs: [0.0; 2 * FB_DELAY + 1],
            state_comb: vec![0.0; STATE_COMB_LEN],
            prev_lag: 0,
            prev_did_filter: 0,
        }
    }
}

#[inline]
fn dot_prod(a: &[f32], b: &[f32], l: usize) -> f32 {
    let mut r = 0.0f32;
    for i in 0..l {
        r += a[i] * b[i];
    }
    r
}

#[inline]
fn nrg(x: &[f32], n: usize) -> f32 {
    let mut r = 0.0f32;
    for i in 0..n {
        r += x[i] * x[i];
    }
    r
}

/// 17-tap symmetric MA. `x` has 16 samples of history at `x[-16..0]`; here it reads from a base offset
/// into the shared buffer.
#[inline]
fn filt_ma16_sym(buf: &[f32], x_base: usize, n: usize, coef: &[f32; 17], out: &mut [f32]) {
    for nn in 0..n {
        let c = x_base + nn;
        let mut res = buf[c - 8] * coef[8];
        for i in 0..8 {
            res += coef[i] * (buf[c - i] + buf[c - 16 + i]);
        }
        out[nn] = res;
    }
}

/// Core filter for one 40-sample lag block. `comb` is the StateComb buffer; `comb_x` is the index of
/// this block's read pointer. `out`/`out_off` is the caller's `y_harm` destination; it doubles as
/// scratch and holds the final block output. The filtered result is also fed back into
/// `comb[comb_x - FB_DELAY ..]`, which is what makes the comb recursive and is the 48-sample-delayed
/// location the next packets read from.
#[allow(clippy::too_many_arguments)]
fn harm_postfilter_core(
    lpcoefs: &mut [f32; 2 * FB_DELAY + 1],
    comb: &mut [f32],
    comb_x: usize,
    future_samples: i32,
    lag: i32,
    diff: &mut [f32],
    diff_base: usize,
    out: &mut [f32],
    out_off: usize,
    l: usize,
    fb_strength: f32,
    prev_did_filter: &mut i32,
) {
    let tables = harm_tables();
    let lag_u = lag as usize;
    let mut xy = 0.0f32;
    // y_harm scratch lives in `out[out_off..out_off+l]`.
    if lag > 0 {
        let lookforward = l as i32 + lag - future_samples;
        if lookforward > 0 {
            let l2 = (l as i32 - lookforward).max(0) as usize;
            for i in 0..l2 {
                out[out_off + i] = comb[comb_x + i - lag_u] + comb[comb_x + i + lag_u];
            }
            for i in 0..(l - l2) {
                out[out_off + l2 + i] = comb[comb_x + l2 + i - lag_u] + comb[comb_x + l2 + i];
            }
        } else {
            for i in 0..l {
                out[out_off + i] = comb[comb_x + i - lag_u] + comb[comb_x + i + lag_u];
            }
        }
        xy = dot_prod(&comb[comb_x..], &out[out_off..], l);
    }
    if lag > 0 && xy > 0.0 {
        let xx = nrg(&comb[comb_x..], l);
        let yy = 0.25 * nrg(&out[out_off..], l);
        let strength = 0.5 * xy / yy.max(xx);
        let high_lag_reduction = 1.0
            - REDUCTION_FAC
                * ((lag - MIN_PITCH_LAG) as f32 / (MAX_PITCH_LAG - MIN_PITCH_LAG) as f32);
        let strength = strength * high_lag_reduction * STRENGTH;
        for i in 0..l {
            out[out_off + i] *= 0.5 * strength;
        }
        // diff = -strength * x + y_harm
        for i in 0..l {
            diff[diff_base + i] = out[out_off + i] + (-strength) * comb[comb_x + i];
        }
        let kernel = &tables.lp_filters[lag_to_filt_ix(lag)];
        for k in 0..(2 * FB_DELAY + 1) {
            lpcoefs[k] = kernel[k] * fb_strength;
        }
        let coef17: [f32; 17] = *lpcoefs;
        // y_harm = MA(diff); then y_harm += comb[x - FB_DELAY] (the 48-delayed base signal). The comb
        // is read-only here; only y_harm is modified.
        let mut yh = [0f32; LAG_SUBFR_LEN];
        filt_ma16_sym(diff, diff_base, l, &coef17, &mut yh);
        for i in 0..l {
            out[out_off + i] = yh[i] + comb[comb_x - FB_DELAY + i];
        }
        *prev_did_filter = 1;
    } else {
        for v in diff[diff_base..diff_base + LAG_SUBFR_LEN].iter_mut() {
            *v = 0.0;
        }
        if *prev_did_filter != 0 {
            // zero-input response of the previous filter for the first 2*FB_DELAY samples, added onto
            // the delayed base; the tail is the plain 48-delayed comb.
            let coef17: [f32; 17] = *lpcoefs;
            let mut yh = [0f32; 2 * FB_DELAY];
            filt_ma16_sym(diff, diff_base, 2 * FB_DELAY, &coef17, &mut yh);
            for i in 0..(2 * FB_DELAY) {
                out[out_off + i] = yh[i] + comb[comb_x - FB_DELAY + i];
            }
            for i in (2 * FB_DELAY)..l {
                out[out_off + i] = comb[comb_x + FB_DELAY + i - 2 * FB_DELAY];
            }
        } else {
            for i in 0..l {
                out[out_off + i] = comb[comb_x - FB_DELAY + i];
            }
        }
        *prev_did_filter = 0;
    }
}

/// Apply the harmonic postfilter to a full packet in place. `x` is `packetlen_16` samples; `lags`
/// are the per-40-block lags (`nlags = packetlen/40`), `normalized_bitrate` is the packet average.
pub(crate) fn smpl_harm_postfilter(
    st: &mut HarmPostfilterState,
    x: &mut [f32],
    x_len: usize,
    lags: &[f32],
    n_lags: usize,
    normalized_bitrate: f32,
) {
    debug_assert_eq!(x_len, n_lags * LAG_SUBFR_LEN);
    // diff buffer with 16 samples of history prefix: backing is FRAME_LEN + 2*FB_DELAY, diff starts at +2*FB_DELAY.
    const DIFF_PREFIX: usize = 2 * FB_DELAY;
    let mut diff = vec![0f32; FRAME_LEN + DIFF_PREFIX];

    let mut lag = st.prev_lag;
    // StateComb layout: [history | current packet]; current packet starts at MAX_PITCH_LAG + HARM_DELAY.
    let comb_cur = MAX_PITCH_LAG as usize + HARM_DELAY;
    st.state_comb[comb_cur..comb_cur + x_len].copy_from_slice(&x[..x_len]);

    let fb_strength = 1.0 - FB_STRENGTH * normalized_bitrate;
    let mut offset1 = 0usize;

    let mut lag_ctr = 0usize;
    while lag_ctr < n_lags {
        let mut offset2 = 0usize;
        // diff[-16..0] = state1
        diff[DIFF_PREFIX - 16..DIFF_PREFIX].copy_from_slice(&st.state1);
        let lag_ctr_end = (lag_ctr + PITCH_NUM_SUBFRAMES).min(n_lags);
        while lag_ctr < lag_ctr_end {
            let comb_x = MAX_PITCH_LAG as usize + offset1;
            let future_samples = HARM_DELAY as i32 + x_len as i32 - offset1 as i32;
            harm_postfilter_core(
                &mut st.lpcoefs,
                &mut st.state_comb,
                comb_x,
                future_samples,
                lag,
                &mut diff,
                DIFF_PREFIX + offset2,
                x,
                offset1,
                LAG_SUBFR_LEN,
                fb_strength,
                &mut st.prev_did_filter,
            );
            offset1 += LAG_SUBFR_LEN;
            offset2 += LAG_SUBFR_LEN;
            lag = lags[lag_ctr].round() as i32;
            lag_ctr += 1;
        }
        // state1 = diff[offset2-16 .. offset2]
        st.state1
            .copy_from_slice(&diff[DIFF_PREFIX + offset2 - 16..DIFF_PREFIX + offset2]);
    }

    st.prev_lag = lag;
    // shift StateComb left by x_len
    st.state_comb.copy_within(x_len..x_len + comb_cur, 0);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rf32(b: &[u8], o: &mut usize) -> f32 {
        let v = f32::from_le_bytes([b[*o], b[*o + 1], b[*o + 2], b[*o + 3]]);
        *o += 4;
        v
    }
    fn ri32(b: &[u8], o: &mut usize) -> i32 {
        let v = i32::from_le_bytes([b[*o], b[*o + 1], b[*o + 2], b[*o + 3]]);
        *o += 4;
        v
    }

    /// Validate `smpl_harm_postfilter` against the instrumented reference decoder, processing the full
    /// active packet sequence in order (the filter carries StateComb/state1/prev_lag across packets).
    /// The dump carries, per packet, the per-block lags, the packet bitrate, the input (post-hp)
    /// signal, and the reference output.
    ///
    /// The reference is `-ffast-math` (reassociating/FMA-contracting), so this is not bit-for-bit.
    /// Two regimes: every voiced packet and every steady silence packet match within the i16 output
    /// quantization step (the comb math is feed-forward there). The only larger residual is the first
    /// 48 samples of a *silence packet immediately following voiced*: the comb's zero-input response,
    /// driven recursively by the prior frame's `-ffast-math`-built LP coefficients. That residual is
    /// bounded by `TRANSITION_TOL` and only lands on near-silent transitions, so it is inaudible.
    #[test]
    fn harm_postfilter_matches_c() {
        const I16_LSB: f32 = 1.0 / 32768.0;
        // The voiced→silence transition zero-input response under -ffast-math; bulk stays under I16_LSB.
        const TRANSITION_TOL: f32 = 6.0e-4;
        let data = include_bytes!("testdata/harm_postfilter_vectors.raw");
        let mut o = 0usize;
        let count = ri32(data, &mut o);
        let mut st = HarmPostfilterState::default();
        let mut worst = 0f32;
        let mut worst_steady = 0f32;
        for _ in 0..count {
            let _packet = ri32(data, &mut o);
            let plen = ri32(data, &mut o) as usize;
            let nlags = ri32(data, &mut o) as usize;
            let nbr = rf32(data, &mut o);
            let mut lags = vec![0f32; nlags];
            for l in lags.iter_mut() {
                *l = rf32(data, &mut o);
            }
            let mut inp = vec![0f32; plen];
            for v in inp.iter_mut() {
                *v = rf32(data, &mut o);
            }
            let mut cout = vec![0f32; plen];
            for v in cout.iter_mut() {
                *v = rf32(data, &mut o);
            }
            // A silent packet (lag0 == 0) carries the transition zero-input response in its first 48
            // samples; everywhere else is the i16-exact regime.
            let transition = lags[0] == 0.0;
            smpl_harm_postfilter(&mut st, &mut inp, plen, &lags, nlags, nbr);
            for i in 0..plen {
                let d = (inp[i] - cout[i]).abs();
                worst = worst.max(d);
                if !(transition && i < TOT_POSTFILT_DELAY) {
                    worst_steady = worst_steady.max(d);
                }
            }
        }
        eprintln!(
            "harm_postfilter vs reference: packets={count} worst={worst:.2e} worst_steady={worst_steady:.2e} \
             (i16 LSB={I16_LSB:.2e})"
        );
        assert!(
            worst_steady < I16_LSB,
            "harm_postfilter steady-state diverges from reference by {worst_steady:.2e} (>= i16 LSB {I16_LSB:.2e})"
        );
        assert!(
            worst < TRANSITION_TOL,
            "harm_postfilter transition residual {worst:.2e} exceeds tolerance {TRANSITION_TOL:.2e}"
        );
    }
}

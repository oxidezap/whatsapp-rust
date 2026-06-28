//! Voiced/unvoiced classifier and the spectral-harmonicity measure (`spectral_harmonicity`) it shares
//! with the pitch estimator. The classifier folds five strengths (pitch correlation, VAD, spectral
//! tilt, harmonicity, lag) plus a per-stream hysteresis into a single `voicing_strength`; the encoder
//! codes a frame voiced when that is positive and the packet is coded-as-active-voice.
#![allow(clippy::needless_range_loop)]

use super::smpl_lpc::SMPL_F_LEN;

/// Weights on corrs, vad, tilt, harmonicity, short lags. The reference declares 6 entries but only
/// sums the first 5, so this holds 5.
const SMPL_VUV_WEIGHTS: [f32; 5] = [1.0, 0.5, 0.5, 0.7, 0.3];
const SMPL_VUV_BIAS: f32 = -0.1038;
const SMPL_VUV_HYST: f32 = 0.05;
/// `SMPL_F_LEN / 3` (the transition index between the low/high spectral-tilt bands).
const TRANSITION_IX: usize = SMPL_F_LEN / 3;
const HARMONICITY_UNDEF: f32 = -10000.0;

#[inline]
fn smpl_sigmoid(x: f32) -> f32 {
    if x > 80.0 {
        return 1.0;
    }
    if x < -80.0 {
        return 0.0;
    }
    1.0 / (1.0 + (-x).exp())
}

#[inline]
fn smpl_inv_sigmoid(x: f32) -> f32 {
    -((1.0 / x) - 1.0).ln()
}

#[inline]
fn smpl_dot_prod(a: &[f32], b: &[f32], l: usize) -> f32 {
    let mut s = 0.0f32;
    for i in 0..l {
        s += a[i] * b[i];
    }
    s
}

#[inline]
fn smpl_sum_vec(x: &[f32], l: usize) -> f32 {
    let mut s = 0.0f32;
    for &v in x.iter().take(l) {
        s += v;
    }
    s
}

/// Per-stream voicing hysteresis + spectral-tilt background tracker. The encoder threads one instance
/// across the whole stream.
#[derive(Clone)]
pub(crate) struct VuvMode {
    nrg_lo_bgn: f32,
    nrg_hi_bgn: f32,
    voicing_prev: f32,
    last_lag_prev: f32,
}

impl Default for VuvMode {
    fn default() -> Self {
        // All fields start at 0.
        VuvMode {
            nrg_lo_bgn: 0.0,
            nrg_hi_bgn: 0.0,
            voicing_prev: 0.0,
            last_lag_prev: 0.0,
        }
    }
}

/// Harmonic peak/valley energy ratio at low frequencies, from the per-bin weighted power spectrum
/// `f2w` (= `F2[i] * (i+3)` with `f2w[0,1]=0`). `cache` is a per-call harmonicity memo keyed by
/// harmonic bin; `reset` clears it.
fn spectral_harmonicity(avg_lag: f32, f2w: &[f32], cache: &mut [f32], reset: bool) -> f32 {
    if reset {
        for c in cache.iter_mut() {
            *c = HARMONICITY_UNDEF;
        }
    }
    let inv_f2_step_hz = 2.0 * (SMPL_F_LEN - 1) as f32 / 16000.0;
    let harm_hz = 16000.0 / avg_lag;
    let harm_ix = (harm_hz * 2.0 * inv_f2_step_hz).round() as i32;
    debug_assert!(harm_ix >= 0);
    let cache_len = cache.len() as i32;
    if harm_ix >= cache_len {
        // The reference asserts this never happens; guard defensively and fall through to recompute.
        return recompute_harmonicity(harm_hz, inv_f2_step_hz, f2w);
    }
    if cache[harm_ix as usize] > HARMONICITY_UNDEF {
        return cache[harm_ix as usize];
    }
    let hs = recompute_harmonicity(harm_hz, inv_f2_step_hz, f2w);
    cache[harm_ix as usize] = hs;
    hs
}

const NUM_HARMS: usize = 4;

fn recompute_harmonicity(harm_hz: f32, inv_f2_step_hz: f32, f2w: &[f32]) -> f32 {
    let harm_width = harm_hz * inv_f2_step_hz;
    let mut harm_strength = 0.1f32;
    if harm_width > 1.97 {
        let mut peak_valley_mags = [0.0f32; 2 * NUM_HARMS + 1];
        for (num_harm, pvm) in peak_valley_mags.iter_mut().enumerate() {
            let ix_start = 0.5 * num_harm as f32 * harm_width;
            let ix_end = ix_start + harm_width;
            let idx_start = ix_start.ceil() as i32;
            let idx_end = ix_end.floor() as i32;
            let weights_len = (idx_end - idx_start + 1).max(0) as usize;
            let mut weights = [0.0f32; 20];
            let inv_harm_width = 1.0 / harm_width;
            for (i, w) in weights.iter_mut().take(weights_len).enumerate() {
                let mut tmp = (idx_start as f32 - ix_start + i as f32) * inv_harm_width;
                tmp -= tmp * tmp;
                *w = tmp * tmp;
            }
            let base = (idx_start.max(0) as usize).min(f2w.len());
            // The reference assumes the harmonic window stays within F2w; clamp defensively so a
            // degenerate (too-short) lag can't read past the spectrum.
            let avail = (f2w.len() - base).min(weights_len);
            let peak_valley_nrg =
                smpl_dot_prod(&f2w[base..], &weights, avail) / smpl_sum_vec(&weights, weights_len);
            *pvm = (peak_valley_nrg + 1e-30).sqrt();
        }
        let mut mag_ratios_log = [0.0f32; NUM_HARMS];
        let mut mag_weights = [0.0f32; NUM_HARMS];
        const MAG_PEAK_WEIGHTS: [f32; 3] = [1.0, 10.0, 1.0];
        const MAG_VALLEY_WEIGHTS: [f32; 3] = [5.0, 2.0, 5.0];
        for num_harm in 0..NUM_HARMS {
            let mag_peak = MAG_PEAK_WEIGHTS[0] * peak_valley_mags[2 * num_harm]
                + MAG_PEAK_WEIGHTS[1] * peak_valley_mags[2 * num_harm + 1]
                + MAG_PEAK_WEIGHTS[2] * peak_valley_mags[2 * num_harm + 2];
            let mag_valley = MAG_VALLEY_WEIGHTS[0] * peak_valley_mags[2 * num_harm]
                + MAG_VALLEY_WEIGHTS[1] * peak_valley_mags[2 * num_harm + 1]
                + MAG_VALLEY_WEIGHTS[2] * peak_valley_mags[2 * num_harm + 2];
            mag_ratios_log[num_harm] = (mag_peak / mag_valley).ln();
            mag_weights[num_harm] = (mag_peak + mag_valley + 1e-30).sqrt();
        }
        harm_strength = smpl_dot_prod(&mag_weights, &mag_ratios_log, NUM_HARMS)
            / smpl_sum_vec(&mag_weights, NUM_HARMS);
    }
    harm_strength
}

/// Build `f2w` (`F2[i] * (i+3)`, with `f2w[0]=f2w[1]=0`) consumed by `spectral_harmonicity`.
pub(crate) fn build_f2w(f2: &[f32; SMPL_F_LEN]) -> [f32; SMPL_F_LEN] {
    let mut f2w = [0.0f32; SMPL_F_LEN];
    for i in 2..SMPL_F_LEN {
        f2w[i] = f2[i] * (i + 3) as f32;
    }
    f2w
}

/// Harmonicity at `avg_lag` (computed right after the pitch search, fresh cache). Reused by the pitch
/// estimator so its `harm_strength` matches the value fed to `smpl_get_signal_mode`.
pub(crate) fn harm_strength_at(avg_lag: f32, f2w: &[f32; SMPL_F_LEN]) -> f32 {
    let mut cache = [0.0f32; 50];
    spectral_harmonicity(avg_lag, f2w, &mut cache, true)
}

/// Combine the five voicing strengths + hysteresis into `voicing_strength`. `lags` is the
/// per-lag-subframe pitch lag in samples (`framelen / SMPL_LAG_SUBFRLEN` entries); `f2` is the power
/// spectrum `F2[0..256]`. Mutates `vuv` (background tilt + hysteresis state).
pub(crate) fn smpl_get_signal_mode(
    pitchcorr: f32,
    lags: &[f32],
    avg_lag: f32,
    harm_strength: f32,
    f2: &[f32; SMPL_F_LEN],
    sp_act_prob: f32,
    vuv: &mut VuvMode,
) -> f32 {
    let corr_strength = smpl_inv_sigmoid(0.1 + 0.75 * pitchcorr.clamp(0.0, 1.0)); // -1.4 .. 1.4
    let vad_strength = 0.04 * (1.0 - 1.04 / (sp_act_prob + 0.04)); // -1 .. 0

    // spectral tilt
    let mut nrg_lo = 0.0f32;
    for i in 2..TRANSITION_IX {
        let tmp = f2[i] * (i + 3) as f32;
        nrg_lo += tmp * (TRANSITION_IX - i) as f32;
    }
    let mut nrg_hi = 0.0f32;
    for i in TRANSITION_IX..SMPL_F_LEN {
        let tmp = f2[i] * (i + 3) as f32;
        nrg_hi += tmp * (i - TRANSITION_IX) as f32;
    }
    if vad_strength < -0.1 {
        let smth_coef = -0.5 * vad_strength;
        vuv.nrg_lo_bgn += smth_coef * (nrg_lo - vuv.nrg_lo_bgn);
        vuv.nrg_hi_bgn += smth_coef * (nrg_hi - vuv.nrg_hi_bgn);
    }
    let tilt_lin = ((nrg_lo - vuv.nrg_lo_bgn).max(0.0) - (nrg_hi - vuv.nrg_hi_bgn).max(0.0))
        / (nrg_lo + nrg_hi + 1e-9);
    let tilt_strength = tilt_lin * tilt_lin * tilt_lin; // cubed to make the measure less binary
    let lag_strength = -smpl_sigmoid(0.25 * (38.0 - avg_lag));

    let mut voicing_strength = (SMPL_VUV_WEIGHTS[0] * corr_strength
        + SMPL_VUV_WEIGHTS[1] * vad_strength
        + SMPL_VUV_WEIGHTS[2] * tilt_strength
        + SMPL_VUV_WEIGHTS[3] * harm_strength
        + SMPL_VUV_WEIGHTS[4] * lag_strength)
        / smpl_sum_vec(&SMPL_VUV_WEIGHTS, 5)
        + SMPL_VUV_BIAS;

    // hysteresis
    if vuv.last_lag_prev > 0.0 {
        let mut tmp = (lags[0] / vuv.last_lag_prev).log2();
        if tmp > 0.0 {
            tmp *= 0.5;
        }
        vuv.voicing_prev /= 0.4 + tmp * tmp;
    }
    voicing_strength += vuv.voicing_prev * SMPL_VUV_HYST;
    vuv.voicing_prev = (3.0 * voicing_strength).tanh();
    vuv.last_lag_prev = lags[lags.len() - 1];

    voicing_strength
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    // Feed the reference encoder's exact per-frame pitchcorr/avg_lag/harm/lags/F2/sp_act_prob (in
    // stream order, threading one VuvMode) and require our voicing_strength + voiced decision to match
    // the reference output. Isolates the classifier from the pitch estimator.
    #[test]
    fn signal_mode_matches_c_ground_truth() {
        let recs: Value =
            serde_json::from_str(include_str!("testdata/sigmode_ground_truth.json")).unwrap();
        let arr = recs.as_array().unwrap();
        assert!(arr.len() >= 12);
        let mut vuv = VuvMode::default();
        let mut max_err = 0.0f32;
        let mut max_harm_err = 0.0f32;
        for rec in arr {
            let pitchcorr = rec["pitchcorr"].as_f64().unwrap() as f32;
            let avg_lag = rec["avg_lag"].as_f64().unwrap() as f32;
            let harm = rec["harm"].as_f64().unwrap() as f32;
            let sp = rec["sp_act_prob"].as_f64().unwrap() as f32;
            let vstr_c = rec["vstr"].as_f64().unwrap() as f32;
            let voiced_c = rec["voiced"].as_i64().unwrap() != 0;
            let lags: Vec<f32> = rec["lags"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap() as f32)
                .collect();
            let f2v: Vec<f32> = rec["F2"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_f64().unwrap() as f32)
                .collect();
            let mut f2 = [0.0f32; SMPL_F_LEN];
            f2.copy_from_slice(&f2v);

            // Validate harm_strength_at against the reference harm on frames where the pitch search
            // ran. On inactive frames the pitch search early-returns (lag clamped to the 32-sample
            // floor) and never computes harmonicity, leaving harm at its 0.0 init, not a recompute
            // target.
            if avg_lag > 33.0 {
                let f2w = build_f2w(&f2);
                let harm_rs = harm_strength_at(avg_lag, &f2w);
                max_harm_err = max_harm_err.max((harm_rs - harm).abs());
            }

            let vstr_rs = smpl_get_signal_mode(pitchcorr, &lags, avg_lag, harm, &f2, sp, &mut vuv);
            max_err = max_err.max((vstr_rs - vstr_c).abs());
            // Voiced decision (all dump frames are coded_as_active_voice).
            assert_eq!(
                vstr_rs > 0.0,
                voiced_c,
                "voiced flip frame vstr_rs={vstr_rs} vstr_c={vstr_c}"
            );
        }
        assert!(
            max_err < 1e-4,
            "voicing_strength diverges from reference: max_err={max_err}"
        );
        // harm_strength_at is exact PER call, but the reference reuses a survivor-loop cache keyed by
        // a quantized harm bin, so its FINAL value can be one computed at a different (bin-sharing)
        // survivor lag. A fresh-cache recompute is therefore close but not bit-exact without the full
        // pitch survivor sequence; bound the residual.
        assert!(
            max_harm_err < 0.05,
            "harm_strength diverges from reference beyond cache-aliasing tolerance: {max_harm_err}"
        );
    }
}

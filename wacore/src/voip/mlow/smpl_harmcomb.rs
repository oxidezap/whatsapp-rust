//! MLow HP (harmonic/pitch) postfilter: the post-LPC-synthesis comb that resonates the output at the
//! PITCH frequency. Structure per frame:
//!   de-emphasis (AR1 leaky integrator {1,-0.995}) -> ARMA2 comb (MA2 numerator + AR2 denominator,
//!   coefficients derived from the pitch lag f=1/lag) -> companion pre-emphasis (MA1 differentiator).
//!
//! The comb keys on the PITCH LAG (f=1/lag), not an energy ratio, and the AR denominator radius factor
//! uses the `arr` curve (negative -> stable pole), not `arf` (positive -> unstable).
#![allow(clippy::needless_range_loop, clippy::excessive_precision)]

use std::sync::OnceLock;

/// Low-emphasis coef pair {1.0, -0.995}: AR1 = de-emphasis (leaky integrator), MA1 = companion
/// pre-emphasis (differentiator). The comb is bracketed by these.
const LO_EMPH: [f32; 2] = [1.0, -0.995];

/// 1.2 dB peak voiced pitch-comb curve: maf, arf (cos angle), arr (radius).
const HP_PITCH_MAF: f32 = 0.1;
const HP_PITCH_ARF: [f32; 2] = [0.608057355, 0.070939485];
const HP_PITCH_ARR: [f32; 2] = [-2.187380512, 2.291030664];
/// Default (lag<=0) curve, corner 50 Hz -> f = 50/16000.
const HP_DEF_MAF: f32 = 0.1;
const HP_DEF_ARF: [f32; 2] = [0.728508218, 0.476039848];
const HP_DEF_ARR: [f32; 2] = [-4.363803713, 8.441854006];
const HP_DEF_FCORNER_HZ: f32 = 50.0;

const SMPL_PI: f32 = 3.1415927410125;
const LAG_CHANGE_THRESHOLD: f32 = 1.25;
const FRAME_LEN: usize = 320;
const HP_POSTF_TRANSITION_SPEED: f32 = 2.0;

/// Persistent HP-postfilter state. `lag_old < 0` marks a fresh/reset filter.
/// `scratch_*` are per-frame working buffers hoisted off the hot path; each is fully overwritten
/// (`[..n]`) before it is read, so they carry no state between frames.
#[derive(Clone)]
pub(crate) struct HpPostfilterState {
    state_lo_emph1: f32,
    state_lo_emph2: f32,
    state_hp: [f32; 4], // [ma2 x[-1], ma2 x[-2], ar2 y[-1], ar2 y[-2]]
    lag_old: f32,
    x_old: [f32; FRAME_LEN],
    coef_ma: [f32; 3],
    coef_ar: [f32; 3],
    scratch_x: [f32; FRAME_LEN],
    scratch_y_old: [f32; FRAME_LEN],
    scratch_y_tmp: [f32; FRAME_LEN],
    scratch_dummy: [f32; FRAME_LEN],
}

impl Default for HpPostfilterState {
    fn default() -> Self {
        Self {
            state_lo_emph1: 0.0,
            state_lo_emph2: 0.0,
            state_hp: [0.0; 4],
            lag_old: -1.0,
            x_old: [0.0; FRAME_LEN],
            coef_ma: [0.0; 3],
            coef_ar: [0.0; 3],
            scratch_x: [0.0; FRAME_LEN],
            scratch_y_old: [0.0; FRAME_LEN],
            scratch_y_tmp: [0.0; FRAME_LEN],
            scratch_dummy: [0.0; FRAME_LEN],
        }
    }
}

/// Small-angle cosine `cos_approx(x) = 1 - 0.5*x^2`.
#[inline]
fn cos_approx(x: f32) -> f32 {
    1.0 - 0.5 * x * x
}

/// 3-tap FIR with carried 2-sample input history (general/monic). Also reused by the excitation
/// postfilter (comb #1).
pub(crate) fn smpl_pf_fir3(
    input: &[f32],
    n: usize,
    coef: [f32; 3],
    state: &mut [f32; 2],
    out: &mut [f32],
) {
    let xm1 = state[0];
    let xm2 = state[1];
    for i in 0..n {
        let p1 = if i >= 1 { input[i - 1] } else { xm1 };
        let p2 = if i >= 2 {
            input[i - 2]
        } else if i == 1 {
            xm1
        } else {
            xm2
        };
        out[i] = coef[0] * input[i] + coef[1] * p1 + coef[2] * p2;
    }
    if n >= 2 {
        state[0] = input[n - 1];
        state[1] = input[n - 2];
    } else if n == 1 {
        state[1] = xm1;
        state[0] = input[0];
    }
}

/// 2nd-order all-pole `y[n] = in[n] - c1*y[n-1] - c2*y[n-2]` (monic), state {y[-1],y[-2]}. Uses the
/// scalar 4-wide unrolled block (precomputed coefficient powers) so the floating-point rounding is
/// bit-exact with the scalar reference (the codec was built without NEON).
fn smpl_filt_ar2(input: &[f32], n: usize, c1: f32, c2: f32, state: &mut [f32; 2], out: &mut [f32]) {
    let mut ytmp0 = state[1];
    let mut ytmp1 = state[0];
    let ar1 = -c1;
    let ar2 = -c2;
    let ar1_2 = ar1 * ar1;
    let ar1_3 = ar1 * ar1_2;
    let ar1_4 = ar1 * ar1_3;
    let imp1 = ar1;
    let imp2 = ar1_2 + ar2;
    let imp3 = ar1_3 + 2.0 * ar1 * ar2;
    let imp4 = ar1_4 + ar2 * ar2 + 3.0 * ar1_2 * ar2;
    let ymp1 = ar2;
    let ymp2 = ar2 * imp1;
    let ymp3 = ar2 * imp2;
    let ymp4 = ar2 * imp3;
    let mut nn = 0usize;
    while nn + 3 < n {
        let xtmp0 = input[nn];
        let xtmp1 = input[nn + 1];
        let xtmp2 = input[nn + 2];
        out[nn + 2] = xtmp2 + imp1 * xtmp1 + imp2 * xtmp0 + imp3 * ytmp1 + ymp3 * ytmp0;
        let xtmp3 = input[nn + 3];
        out[nn + 3] =
            xtmp3 + imp1 * xtmp2 + imp2 * xtmp1 + imp3 * xtmp0 + imp4 * ytmp1 + ymp4 * ytmp0;
        out[nn] = xtmp0 + imp1 * ytmp1 + ymp1 * ytmp0;
        out[nn + 1] = xtmp1 + imp1 * xtmp0 + imp2 * ytmp1 + ymp2 * ytmp0;
        ytmp0 = out[nn + 2];
        ytmp1 = out[nn + 3];
        nn += 4;
    }
    while nn < n {
        out[nn] = input[nn] + ar1 * ytmp1 + ar2 * ytmp0;
        ytmp0 = ytmp1;
        ytmp1 = out[nn];
        nn += 1;
    }
    state[1] = ytmp0;
    state[0] = ytmp1;
}

/// AR1 leaky integrator `y[n] = x[n] - c1*y[n-1]` (here the de-emphasis {1,-0.995}). Uses the scalar
/// 5-wide unrolled block (precomputed `ar1` powers) for bit-exact rounding.
fn smpl_filt_ar1(input: &[f32], n: usize, c1: f32, state: &mut f32, out: &mut [f32]) {
    let ar1 = -c1;
    let ar1_2 = ar1 * ar1;
    let ar1_3 = ar1 * ar1_2;
    let ar1_4 = ar1 * ar1_3;
    let ar1_5 = ar1 * ar1_4;
    let mut ytmp = *state;
    let mut nn = 0usize;
    while nn + 4 < n {
        let xtmp0 = input[nn];
        let xtmp1 = input[nn + 1];
        let xtmp2 = input[nn + 2];
        let xtmp3 = input[nn + 3];
        let xtmp4 = input[nn + 4];
        out[nn + 4] =
            xtmp4 + ar1 * xtmp3 + ar1_2 * xtmp2 + ar1_3 * xtmp1 + ar1_4 * xtmp0 + ar1_5 * ytmp;
        out[nn] = xtmp0 + ar1 * ytmp;
        out[nn + 1] = xtmp1 + ar1 * xtmp0 + ar1_2 * ytmp;
        out[nn + 2] = xtmp2 + ar1 * xtmp1 + ar1_2 * xtmp0 + ar1_3 * ytmp;
        out[nn + 3] = xtmp3 + ar1 * xtmp2 + ar1_2 * xtmp1 + ar1_3 * xtmp0 + ar1_4 * ytmp;
        ytmp = out[nn + 4];
        nn += 5;
    }
    while nn < n {
        ytmp = input[nn] + ytmp * ar1;
        out[nn] = ytmp;
        nn += 1;
    }
    *state = ytmp;
}

/// MA1 `y[n] = x[n] + c1*x[n-1]` (here the companion pre-emphasis {1,-0.995}).
fn smpl_filt_ma1(input: &[f32], n: usize, c1: f32, state: &mut f32, out: &mut [f32]) {
    let prev = *state;
    for i in (1..n).rev() {
        out[i] = input[i] + c1 * input[i - 1];
    }
    if n > 0 {
        out[0] = input[0] + c1 * prev;
        *state = input[n - 1];
    }
}

/// The default fixed-corner ARMA2 biquad (also the encoder's input high-pass). `fcorner_hz` clamped to
/// [5, 1500]; `f = fcorner/16000`.
pub(crate) fn smpl_get_hp_coefs(fcorner_hz: f32) -> ([f32; 3], [f32; 3]) {
    let fc = fcorner_hz.clamp(5.0, 1500.0);
    smpl_calc_hp_coefs(HP_DEF_MAF, HP_DEF_ARF, HP_DEF_ARR, fc / 16000.0)
}

/// MA2 numerator then AR2 denominator, shared 4-wide state {ma x[-1],x[-2], ar y[-1],y[-2]}.
pub(crate) fn smpl_filt_arma2(
    input: &[f32],
    n: usize,
    coef_ma: [f32; 3],
    coef_ar: [f32; 3],
    state: &mut [f32; 4],
    out: &mut [f32],
) {
    let mut tmp = vec![0f32; n];
    let mut ma_st = [state[0], state[1]];
    smpl_pf_fir3(input, n, coef_ma, &mut ma_st, &mut tmp);
    state[0] = ma_st[0];
    state[1] = ma_st[1];
    let mut ar_st = [state[2], state[3]];
    smpl_filt_ar2(&tmp, n, coef_ar[1], coef_ar[2], &mut ar_st, out);
    state[2] = ar_st[0];
    state[3] = ar_st[1];
}

/// Build the unity-numerator-DC comb biquad. The AR denominator is a resonance at the pitch angle
/// `2*pi*arf*f` with radius `1 + arr*f` (arr negative -> stable), then the MA numerator is scaled so
/// the comb has unity DC gain.
fn smpl_calc_hp_coefs(maf: f32, arf: [f32; 2], arr: [f32; 2], f: f32) -> ([f32; 3], [f32; 3]) {
    let mut coef_ma = [1.0f32, -2.0 * cos_approx(2.0 * SMPL_PI * maf * f), 1.0];
    let far_ = arf[0] * f + arf[1] * f * f;
    let rar_ = arr[0] * f + arr[1] * f * f;
    let coef_ar = [
        1.0,
        -2.0 * cos_approx(2.0 * SMPL_PI * far_) * (1.0 + rar_),
        1.0 + (2.0 * rar_ + rar_ * rar_),
    ];
    let sc = (1.0 - coef_ar[1] + coef_ar[2]) / (1.0 - coef_ma[1] + coef_ma[2]);
    for c in coef_ma.iter_mut() {
        *c *= sc;
    }
    (coef_ma, coef_ar)
}

/// Voiced pitch curve when lag>0 (f=1/lag), else the default 50 Hz-corner curve.
fn new_coefs_into(coef_ma: &mut [f32; 3], coef_ar: &mut [f32; 3], lag: f32) {
    let (ma, ar) = if lag > 0.0 {
        let f = 1.0 / lag;
        smpl_calc_hp_coefs(HP_PITCH_MAF, HP_PITCH_ARF, HP_PITCH_ARR, f)
    } else {
        let fc = HP_DEF_FCORNER_HZ.clamp(5.0, 1500.0);
        smpl_calc_hp_coefs(HP_DEF_MAF, HP_DEF_ARF, HP_DEF_ARR, fc / 16000.0)
    };
    *coef_ma = ma;
    *coef_ar = ar;
}

/// `cos(omega)^2` down-ramp for the lag-change overlap-add. `omega` is accumulated by repeated addition
/// (not `d_omega * i`) to stay bit-exact with the reference table.
fn ramp_dn(len: usize) -> &'static Vec<f32> {
    static RAMP: OnceLock<Vec<f32>> = OnceLock::new();
    let ramp = RAMP.get_or_init(|| {
        let d_omega = SMPL_PI / (2.0 * (FRAME_LEN as f32 + 1.0));
        let mut omega = d_omega;
        let mut v = Vec::with_capacity(FRAME_LEN);
        for _ in 0..FRAME_LEN {
            v.push(omega.cos().powf(HP_POSTF_TRANSITION_SPEED));
            omega += d_omega;
        }
        v
    });
    debug_assert_eq!(len, FRAME_LEN);
    ramp
}

/// Apply the HP (pitch-harmonic) postfilter to one frame's post-LPC output. `lag` is the frame's
/// average pitch lag (`sum(l^2)/sum(l)` over the subframe lags), 0 for unvoiced.
pub(crate) fn smpl_hp_postfilter(
    st: &mut HpPostfilterState,
    x_in: &[f32],
    n: usize,
    lag: f32,
    out: &mut [f32],
) {
    // de-emphasis (AR1) into the x scratch (disjoint field borrows let the input/output/state
    // alias-free across the arma2 calls below; each scratch is fully written before any read).
    let HpPostfilterState {
        state_lo_emph1,
        state_lo_emph2,
        state_hp,
        x_old,
        coef_ma,
        coef_ar,
        scratch_x,
        scratch_y_old,
        scratch_y_tmp,
        scratch_dummy,
        lag_old,
    } = st;
    smpl_filt_ar1(x_in, n, LO_EMPH[1], state_lo_emph1, &mut scratch_x[..n]);

    let mut overlap = false;
    if *lag_old < 0.0 {
        new_coefs_into(coef_ma, coef_ar, lag);
        *lag_old = lag;
    } else if lag > LAG_CHANGE_THRESHOLD * *lag_old || LAG_CHANGE_THRESHOLD * lag < *lag_old {
        overlap = true;
        smpl_filt_arma2(
            &scratch_x[..n],
            n,
            *coef_ma,
            *coef_ar,
            state_hp,
            &mut scratch_y_old[..n],
        );
        new_coefs_into(coef_ma, coef_ar, lag);
        *lag_old = lag;
        smpl_filt_arma2(
            &x_old[..n],
            n,
            *coef_ma,
            *coef_ar,
            state_hp,
            &mut scratch_dummy[..n],
        );
    } else if lag != *lag_old {
        new_coefs_into(coef_ma, coef_ar, lag);
        *lag_old = lag;
    }
    x_old[..n].copy_from_slice(&scratch_x[..n]);

    smpl_filt_arma2(
        &scratch_x[..n],
        n,
        *coef_ma,
        *coef_ar,
        state_hp,
        &mut scratch_y_tmp[..n],
    );

    if overlap {
        let ramp = ramp_dn(n);
        for i in 0..n {
            scratch_y_tmp[i] += (scratch_y_old[i] - scratch_y_tmp[i]) * ramp[i];
        }
    }

    // companion pre-emphasis (MA1).
    smpl_filt_ma1(&scratch_y_tmp[..n], n, LO_EMPH[1], state_lo_emph2, out);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Seed a state from a snapshot (test-only; mirrors the dumped field order).
    #[allow(clippy::too_many_arguments)]
    fn seed_state(
        lo1: f32,
        lo2: f32,
        hp: [f32; 4],
        lag_old: f32,
        x_old: [f32; FRAME_LEN],
        coef_ma: [f32; 3],
        coef_ar: [f32; 3],
    ) -> HpPostfilterState {
        HpPostfilterState {
            state_lo_emph1: lo1,
            state_lo_emph2: lo2,
            state_hp: hp,
            lag_old,
            x_old,
            coef_ma,
            coef_ar,
            ..Default::default()
        }
    }

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

    /// Validate `smpl_hp_postfilter` against the instrumented reference decoder. Each active frame is
    /// self-contained: the dump carries the postfilter state in, the 8 per-40-block lags, the pre-hp
    /// signal, and the post-hp signal; we seed, run, and compare sample-for-sample. The frame lag is
    /// the energy-weighted mean `sum(l^2)/sum(l)` over the 8 lags (0 -> default 50 Hz curve).
    ///
    /// The reference is built with `-ffast-math -mavx`, which reassociates the recursive AR/MA
    /// accumulations (and may contract to FMA). Straightforward IEEE-strict Rust therefore cannot
    /// reproduce its output to the last bit through the near-unit-circle pitch-comb feedback: a 1-ULP
    /// de-emphasis difference drifts to ~1.5e-5 across the resonant pole. We assert the error stays
    /// well under the i16 output quantization step (1/32768 ~= 3.05e-5), i.e. inaudible and identical
    /// once written to the 16-bit PCM the codec emits.
    #[test]
    fn hp_postfilter_matches_c() {
        const I16_LSB: f32 = 1.0 / 32768.0;
        let data = include_bytes!("testdata/hp_postfilter_vectors.raw");
        let mut o = 0usize;
        let count = ri32(data, &mut o);
        let mut worst = 0f32;
        for _ in 0..count {
            let _packet = ri32(data, &mut o);
            let _frame = ri32(data, &mut o);
            let mut lags = [0f32; 8];
            for l in lags.iter_mut() {
                *l = rf32(data, &mut o);
            }
            let lo1 = rf32(data, &mut o);
            let lo2 = rf32(data, &mut o);
            let mut hp = [0f32; 4];
            for h in hp.iter_mut() {
                *h = rf32(data, &mut o);
            }
            let lag_old = rf32(data, &mut o);
            let mut x_old = [0f32; FRAME_LEN];
            for x in x_old.iter_mut() {
                *x = rf32(data, &mut o);
            }
            let mut coef_ma = [0f32; 3];
            for c in coef_ma.iter_mut() {
                *c = rf32(data, &mut o);
            }
            let mut coef_ar = [0f32; 3];
            for c in coef_ar.iter_mut() {
                *c = rf32(data, &mut o);
            }
            let mut y_pre = vec![0f32; FRAME_LEN];
            for y in y_pre.iter_mut() {
                *y = rf32(data, &mut o);
            }
            let mut y_post = vec![0f32; FRAME_LEN];
            for y in y_post.iter_mut() {
                *y = rf32(data, &mut o);
            }

            let lag = if lags[0] > 0.0 {
                let (mut sl, mut sll) = (0f32, 0f32);
                for &l in &lags {
                    sl += l;
                    sll += l * l;
                }
                sll / sl
            } else {
                0.0
            };

            let mut st = seed_state(lo1, lo2, hp, lag_old, x_old, coef_ma, coef_ar);
            let mut out = vec![0f32; FRAME_LEN];
            smpl_hp_postfilter(&mut st, &y_pre, FRAME_LEN, lag, &mut out);
            for i in 0..FRAME_LEN {
                worst = worst.max((out[i] - y_post[i]).abs());
            }
        }
        eprintln!(
            "hp_postfilter vs reference: frames={count} worst_abs_diff={worst:.2e} (i16 LSB={I16_LSB:.2e})"
        );
        assert!(
            worst < I16_LSB,
            "hp_postfilter diverges from reference by {worst:.2e} (>= i16 LSB {I16_LSB:.2e})"
        );
    }
}

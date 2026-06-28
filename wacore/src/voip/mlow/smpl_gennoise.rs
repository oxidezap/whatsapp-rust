//! CELP noise generator (`smpl_celp_gen_noise` + `add_noise_uv`) and its helpers (`smpl_RAND`,
//! `smpl_gen_rand_pulses`, `smpl_get_env`/`get_env0`, `smpl_sigmoid`, `smpl_spec_fact2`, the noise
//! DCT) plus `smpl_decode_resnrg` and `smpl_get_normalized_bitrate`.
//!
//! The decoder runs this per subframe: it derives the shaped residual noise that the real codec
//! adds into the LPC excitation before synthesis. Without it the excitation is bare FCB pulses
//! (buzzy/robotic, starved highs). The unvoiced branch is the dominant path; the voiced branch
//! shapes a small high-band noise floor.
//!
//! The float constants are byte-exact copies of the reference f32 literals.
#![allow(clippy::excessive_precision)]
#![allow(clippy::needless_range_loop)]
// SMPL_PI is the verbatim literal `3.1415926535897f`; keep it for bit-exactness.
#![allow(clippy::approx_constant)]

const SMPL_MAX_SF_LEN: usize = 160;
const SMPL_NOISE_CORR_ORDER: usize = 2;
const SMPL_NOISE_DCT_ORDER: usize = 16;
const SMPL_CELP_FS_KHZ: usize = 16;
const SMPL_PI: f32 = 3.1415926535897f32;

const SMPL_DEC_NOISE_V_NOISE_GAIN: f32 = 0.35;
const SMPL_DEC_NOISE_UV_NOISE_GAIN: f32 = 0.8;
const SMPL_DEC_NOISE_UV_FCORNER_HZ: f32 = 800.0;
const SMPL_ENV_SMTH_COEF_V: f32 = 0.95;
const SMPL_ENV_SMTH_COEF_UV: f32 = 0.995;
const SMPL_ENV_SMTH_COEF_UV_V: f32 = 0.99;

const SMPL_RES_NRG_BIAS: f32 = 3.1622776e-9;

/// `smpl_RAND`: LCG, wrapping i32 arithmetic (C uses u32 mul + add on the i32 seed).
#[inline]
fn smpl_rand(seed: i32) -> i32 {
    // smpl_MLA_ovflw(907633515, seed, 196314165) = 907633515 + (u32)seed * 196314165
    (907633515i32).wrapping_add((seed as u32).wrapping_mul(196314165) as i32)
}

/// `smpl_sigmoid` with the same +-80 clamp as C (avoids inf/denormal under fast-math).
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
fn smpl_nrg(x: &[f32]) -> f32 {
    let mut nrg = 0.0f32;
    for &v in x {
        nrg += v * v;
    }
    nrg
}

#[inline]
fn smpl_sum(x: &[f32]) -> f32 {
    let mut s = 0.0f32;
    for &v in x {
        s += v;
    }
    s
}

#[inline]
fn smpl_maximum(x: &[f32]) -> f32 {
    let mut m = x[0];
    for &v in &x[1..] {
        if v > m {
            m = v;
        }
    }
    m
}

/// `smpl_gen_rand_pulses` (live version): 4-at-a-time bit-rotated white pulses scaled by 8.1e-10.
fn smpl_gen_rand_pulses(noise: &mut [f32], l: usize, seed: &mut i32) {
    const SC: f32 = 8.1e-10;
    let mut i = 0;
    while i + 3 < l {
        *seed = smpl_rand(*seed);
        let s = *seed as u32;
        noise[i] = SC * (*seed as f32);
        noise[i + 1] = SC * ((s << 8) as i32 as f32);
        noise[i + 2] = SC * ((s << 16) as i32 as f32);
        noise[i + 3] = SC * ((s << 24) as i32 as f32);
        i += 4;
    }
    while i < l {
        *seed = smpl_rand(*seed);
        noise[i] = SC * (*seed as f32);
        i += 1;
    }
}

/// `smpl_get_env`: squared-signal smoothing envelope (4-wide; the accumulation order is load-bearing
/// for bit-exactness).
fn smpl_get_env(
    exc: &[f32],
    len: usize,
    mut smth_coef: f32,
    smth_state: &mut f32,
    env: &mut [f32],
) {
    smth_coef *= smth_coef; // operate on squared signal
    let mut state = *smth_state + 1e-8;
    state *= state;
    let gain_coef = 1.0 - smth_coef;
    let smth_coef2 = smth_coef * smth_coef;
    let gain_smth_coef = gain_coef * smth_coef;
    let mut i = 0;
    while i + 3 < len {
        let tmp0 = exc[i] * exc[i] + exc[i + 1] * exc[i + 1];
        let tmp1 = exc[i + 2] * exc[i + 2] + exc[i + 3] * exc[i + 3];
        let y1 = gain_coef * tmp1 + gain_smth_coef * tmp0 + smth_coef2 * state;
        let y0 = gain_coef * tmp0 + smth_coef * state;
        env[i] = y0.sqrt();
        env[i + 1] = env[i];
        env[i + 2] = y1.sqrt();
        env[i + 3] = env[i + 2];
        state = y1;
        i += 4;
    }
    *smth_state = env[len - 1];
}

/// `smpl_get_env0`: decaying envelope when there is no excitation to seed from.
fn smpl_get_env0(len: usize, smth_coef: f32, smth_state: &mut f32, env: &mut [f32]) {
    let smth_coef2 = smth_coef * smth_coef;
    env[0] = (*smth_state + 1e-8) * smth_coef;
    env[1] = env[0];
    let mut i = 2;
    while i + 2 < len {
        env[i + 2] = env[i - 1] * smth_coef2;
        env[i + 3] = env[i + 2];
        env[i] = env[i - 1] * smth_coef;
        env[i + 1] = env[i];
        i += 4;
    }
    env[len - 2] = env[len - 3] * smth_coef;
    env[len - 1] = env[len - 2];
    *smth_state = env[len - 1];
}

// Filters.

/// `smpl_filt_ma1` (coef_len=2, state_len=1). `x != y`.
fn smpl_filt_ma1(x: &[f32], n: usize, coef: &[f32; 2], state: &mut [f32; 1], y: &mut [f32]) {
    if coef[0] == 1.0 {
        for k in 1..n {
            y[k] = x[k] + coef[1] * x[k - 1];
        }
    } else {
        for k in 0..n {
            y[k] = coef[0] * x[k];
        }
        for k in 1..n {
            y[k] += coef[1] * x[k - 1];
        }
    }
    y[0] = coef[0] * x[0] + coef[1] * state[0];
    state[0] = x[n - 1];
}

/// `smpl_filt_ar1` (coef_len=2, state_len=1, coef[0]==1).
fn smpl_filt_ar1(x: &[f32], n: usize, coef: &[f32; 2], state: &mut [f32; 1], y: &mut [f32]) {
    let ar1 = -coef[1];
    let mut ytmp = state[0];
    for nn in 0..n {
        ytmp = x[nn] + ytmp * ar1;
        y[nn] = ytmp;
    }
    state[0] = ytmp;
}

/// `smpl_filt_arma1`: MA1 then AR1, state {ma, ar}. Supports in-place (x==y) via a temp buffer.
fn smpl_filt_arma1(
    x: &[f32],
    n: usize,
    coef_ma: &[f32; 2],
    coef_ar: &[f32; 2],
    state: &mut [f32; 2],
    y: &mut [f32],
) {
    let mut tmp = [0.0f32; SMPL_MAX_SF_LEN];
    let mut ma_state = [state[0]];
    smpl_filt_ma1(x, n, coef_ma, &mut ma_state, &mut tmp);
    state[0] = ma_state[0];
    let mut ar_state = [state[1]];
    smpl_filt_ar1(&tmp, n, coef_ar, &mut ar_state, y);
    state[1] = ar_state[0];
}

/// `smpl_filt_ma2` (coef_len=3, state_len=2). `x != y`.
fn smpl_filt_ma2(x: &[f32], n: usize, coef: &[f32; 3], state: &mut [f32; 2], y: &mut [f32]) {
    if coef[0] == 1.0 {
        for i in 1..n {
            y[i] = x[i] + coef[1] * x[i - 1];
        }
    } else {
        for i in 0..n {
            y[i] = coef[0] * x[i];
        }
        for i in 1..n {
            y[i] += coef[1] * x[i - 1];
        }
    }
    for i in 2..n {
        y[i] += coef[2] * x[i - 2];
    }
    y[0] = coef[0] * x[0] + coef[1] * state[0] + coef[2] * state[1];
    y[1] += coef[2] * state[0];
    state[0] = x[n - 1];
    state[1] = x[n - 2];
}

/// `smpl_spec_fact2`: spectral factorization of a 3-tap autocorrelation into a 3-tap MA.
fn smpl_spec_fact2(c_in: &[f32; 3], a: &mut [f32; 3]) {
    let mut c = *c_in;
    c[0] += 1e-30;
    let inv_c0 = 1.0 / c[0];
    let mut r2 = c[2] * inv_c0;
    let mut r1 = c[1] / (c[0] * (1.0 + r2));
    for _ in 0..2 {
        let v0 = 1.0 + r1 * r1 + r2 * r2;
        let v1 = r1 + r1 * r2;
        let mut s = -2.0 / v0;
        let da0 = s * r1;
        let da1 = s * r2;
        s = v0 * inv_c0;
        let e1 = s * c[1] - v1;
        let e2 = s * c[2] - r2;
        let r0 = 2.0 * r1 + v0 * da0;
        let r3 = 2.0 * r2 + v0 * da1;
        let mut rr00 = r0 * r0;
        let mut rr01 = r0 * r3;
        let mut rr11 = r3 * r3;
        let rcap1 = 1.0 + r2 + v1 * da0;
        let r4 = r1 + v1 * da1;
        rr00 += rcap1 * rcap1;
        rr01 += rcap1 * r4;
        rr11 += r4 * r4;
        let mut re0 = rcap1 * e1;
        let mut re1 = r4 * e1;
        let r2c = r2 * da0;
        let r5 = 1.0 + r2 * da1;
        rr00 += r2c * r2c;
        rr01 += r2c * r5;
        rr11 += r5 * r5;
        re0 += r2c * e2;
        re1 += r5 * e2;
        s = rr00 * rr11 - rr01 * rr01;
        if s < 1e-4 {
            break;
        }
        s = 1.0 / s;
        r1 += (rr11 * re0 - rr01 * re1) * s;
        r2 += (-rr01 * re0 + rr00 * re1) * s;
    }
    let sc = (c[0] / (1.0 + r1 * r1 + r2 * r2)).sqrt();
    a[0] = sc;
    a[1] = sc * r1;
    a[2] = sc * r2;
}

/// The noise DCT (`dct_mat_t[NOISE_CORR_ORDER+1][NOISE_DCT_ORDER]`).
fn noise_dct() -> &'static [[f32; SMPL_NOISE_DCT_ORDER]; SMPL_NOISE_CORR_ORDER + 1] {
    use std::sync::OnceLock;
    static DCT: OnceLock<[[f32; SMPL_NOISE_DCT_ORDER]; SMPL_NOISE_CORR_ORDER + 1]> =
        OnceLock::new();
    DCT.get_or_init(|| {
        let mut m = [[0.0f32; SMPL_NOISE_DCT_ORDER]; SMPL_NOISE_CORR_ORDER + 1];
        let sc = 1.0f32 / (SMPL_NOISE_DCT_ORDER as f32).sqrt();
        for i in 0..SMPL_NOISE_DCT_ORDER {
            let d_omega = ((0.5 + i as f32) * SMPL_PI) / SMPL_NOISE_DCT_ORDER as f32;
            let mut omega = 0.0f32;
            for j in 0..SMPL_NOISE_CORR_ORDER + 1 {
                m[j][i] = omega.cos() * sc;
                omega += d_omega;
            }
        }
        m
    })
}

/// `smpl_matrix_mult_transp_16`: y[0..16] = sum_j C[j*16 + i] * x[j].
fn matrix_mult_transp_16(
    c: &[[f32; SMPL_NOISE_DCT_ORDER]; SMPL_NOISE_CORR_ORDER + 1],
    x: &[f32],
    y: &mut [f32],
    len_x: usize,
) {
    let mut yt = [0.0f32; SMPL_NOISE_DCT_ORDER];
    let xtmp = x[0];
    for i in 0..SMPL_NOISE_DCT_ORDER {
        yt[i] = c[0][i] * xtmp;
    }
    for j in 1..len_x {
        let xtmp = x[j];
        for i in 0..SMPL_NOISE_DCT_ORDER {
            yt[i] += c[j][i] * xtmp;
        }
    }
    y[..SMPL_NOISE_DCT_ORDER].copy_from_slice(&yt);
}

/// `smpl_matrix_mult`: y[i] = dot(C[i*len_x ..], x, len_x). C here is dct_mat_t flattened by [j][i].
fn matrix_mult(
    c: &[[f32; SMPL_NOISE_DCT_ORDER]; SMPL_NOISE_CORR_ORDER + 1],
    x: &[f32],
    y: &mut [f32],
) {
    // C is treated row-major as C[i][k], but dct_mat_t is [k][i]. The call reads dct_mat_t flat with
    // len_y=CORR+1, len_x=DCT_ORDER, so row i is the flat offset i*DCT_ORDER, i.e. dct_mat_t[i].
    for i in 0..SMPL_NOISE_CORR_ORDER + 1 {
        let mut acc = 0.0f32;
        for k in 0..SMPL_NOISE_DCT_ORDER {
            acc += c[i][k] * x[k];
        }
        y[i] = acc;
    }
}

/// `smpl_get_normalized_bitrate` (`smpl_helpers.c`).
pub(crate) fn smpl_get_normalized_bitrate(num_pulses: i32, frame_length_16: i32) -> f32 {
    let pulses_per_20ms = (num_pulses * frame_length_16) as f32 / (20.0 * 16.0);
    smpl_sigmoid(1.4 * (pulses_per_20ms + 1.0).log2() - 6.5)
}

/// `smpl_decode_resnrg` (`smpl_quant_nrg_res.c`). `nrgres_frame_dbq_Q14` is the per-subframe Q14
/// residual-energy value; returns the linear residual energy scaled by the subframe length.
pub(crate) fn smpl_decode_resnrg(nrgres_frame_dbq_q14: i32, fcb_subfrlen: i32) -> f32 {
    let exp = 0.1 * (nrgres_frame_dbq_q14 as f32 / ((1i32 << 14) as f32));
    let mut resnrg = 10.0f32.powf(exp) - SMPL_RES_NRG_BIAS;
    if resnrg < 0.0 {
        resnrg = 0.0;
    }
    resnrg * fcb_subfrlen as f32
}

/// Persistent decoder-side noise generator state (mirrors C `NoiseGenerator`).
#[derive(Clone)]
pub(crate) struct NoiseGenerator {
    pub(crate) env_smth: f32,
    pub(crate) env_last: f32,
    pub(crate) out_state_uv: [f32; 2],
    pub(crate) out_state_v: [f32; 2],
    pub(crate) corr_smth: [f32; SMPL_NOISE_CORR_ORDER + 1],
    pub(crate) shape_state: [f32; SMPL_NOISE_CORR_ORDER],
    pub(crate) prev_voiced: bool,
    pub(crate) since_unvoiced: i32,
    pub(crate) rand_seed: i32,
}

impl Default for NoiseGenerator {
    fn default() -> Self {
        // The whole struct is zero-initialized at decode init.
        NoiseGenerator {
            env_smth: 0.0,
            env_last: 0.0,
            out_state_uv: [0.0; 2],
            out_state_v: [0.0; 2],
            corr_smth: [0.0; SMPL_NOISE_CORR_ORDER + 1],
            shape_state: [0.0; SMPL_NOISE_CORR_ORDER],
            prev_voiced: false,
            since_unvoiced: 0,
            rand_seed: 0,
        }
    }
}

const COEF_MA_V: [f32; 3] = [0.25, -0.496, 0.25];

/// `add_noise_uv`: HP-shape the unvoiced noise (corner climbs when LSF[0..1] are close) and add it
/// into `noise`. `fcbgains_uv` is the codec's UV gain table (only `lsf[0..1]` are read).
fn add_noise_uv(
    ng: &mut NoiseGenerator,
    exc_noise_uv: &mut [f32],
    l: usize,
    lsf: &[f32],
    nrg_ratio: f32,
    noise: &mut [f32],
) {
    let lsf_hz = 16000.0 * (lsf[0] + lsf[1]) / (4.0 * SMPL_PI);
    let min_uv_fcorner_hz = lsf_hz * 3.0 * smpl_sigmoid(0.2 / (lsf[1] - lsf[0] + 1e-30) - 3.0);
    let mut uv_fcorner_hz = SMPL_DEC_NOISE_UV_FCORNER_HZ * (0.6f32 + 0.4 * nrg_ratio).min(1.0);
    uv_fcorner_hz = uv_fcorner_hz.max(min_uv_fcorner_hz);
    uv_fcorner_hz = uv_fcorner_hz.min(1500.0);
    let coef_tmp = 6.0 * uv_fcorner_hz / 16000.0;
    let coef_ma_uv = [
        (1.0 - 0.5 * coef_tmp) * SMPL_DEC_NOISE_UV_NOISE_GAIN,
        -((1.0 - 0.5 * coef_tmp) * SMPL_DEC_NOISE_UV_NOISE_GAIN),
    ];
    let coef_ar_uv = [1.0, -1.0 + coef_tmp];
    let mut filtered = [0.0f32; SMPL_MAX_SF_LEN];
    smpl_filt_arma1(
        exc_noise_uv,
        l,
        &coef_ma_uv,
        &coef_ar_uv,
        &mut ng.out_state_uv,
        &mut filtered,
    );
    exc_noise_uv[..l].copy_from_slice(&filtered[..l]);
    for i in 0..l {
        noise[i] += exc_noise_uv[i];
    }
}

/// `smpl_celp_gen_noise`: build the shaped residual noise for one subframe (faithful port).
#[allow(clippy::too_many_arguments)]
pub(crate) fn smpl_celp_gen_noise(
    ng: &mut NoiseGenerator,
    exc_lpc: &[f32],
    l: usize,
    voiced: bool,
    num_pulses: i32,
    nrgres: f32,
    fcbg_idx: i32,
    lsf: &[f32],
    normalized_bitrate: f32,
    fcbgains_uv: &[f32],
    noise: &mut [f32],
) {
    let mut nrg_ratio = 1.0f32;
    let mut noise_uv = [0.0f32; SMPL_MAX_SF_LEN];
    let mut noise_v = [0.0f32; SMPL_MAX_SF_LEN];
    let mut noise_v2 = [0.0f32; SMPL_MAX_SF_LEN];
    let mut env = [0.0f32; SMPL_MAX_SF_LEN];

    if voiced {
        let mut corrs = [0.0f32; SMPL_NOISE_CORR_ORDER + 1];
        let mut c = [0.0f32; SMPL_NOISE_CORR_ORDER + 1];
        let mut ctgt = [0.0f32; SMPL_NOISE_CORR_ORDER + 1];
        for i in 0..SMPL_NOISE_CORR_ORDER + 1 {
            let mut acc = 0.0f32;
            for k in 0..l - i {
                acc += exc_lpc[k] * exc_lpc[k + i];
            }
            corrs[i] = acc;
        }
        corrs[0] += 1e-12;
        let corr_smth_coef = if l == SMPL_CELP_FS_KHZ * 10 {
            0.4
        } else {
            0.16
        };
        for i in 0..SMPL_NOISE_CORR_ORDER + 1 {
            ng.corr_smth[i] += corr_smth_coef * (corrs[i] - ng.corr_smth[i]);
        }
        let scale =
            SMPL_DEC_NOISE_V_NOISE_GAIN * SMPL_DEC_NOISE_V_NOISE_GAIN * corrs[0] / ng.corr_smth[0];
        for i in 0..SMPL_NOISE_CORR_ORDER + 1 {
            c[i] = ng.corr_smth[i] * scale;
        }
        c[1] *= 2.0;
        c[2] *= 2.0;

        let dct = noise_dct();
        let mut f2 = [0.0f32; SMPL_NOISE_DCT_ORDER];
        let mut f2_tgt = [0.0f32; SMPL_NOISE_DCT_ORDER];
        matrix_mult_transp_16(dct, &c, &mut f2, SMPL_NOISE_CORR_ORDER + 1);
        let m = smpl_maximum(&f2[..SMPL_NOISE_DCT_ORDER]) * 1.5;
        for i in 0..SMPL_NOISE_DCT_ORDER {
            f2_tgt[i] = m - f2[i];
        }
        matrix_mult(dct, &f2_tgt, &mut ctgt);
        smpl_gen_rand_pulses(&mut noise_v, l, &mut ng.rand_seed);
        if !ng.prev_voiced {
            ng.env_smth = ng.env_last;
        }
        smpl_get_env(exc_lpc, l, SMPL_ENV_SMTH_COEF_V, &mut ng.env_smth, &mut env);
        for i in 0..l {
            noise_v[i] *= env[i];
        }
        let nrg_noise = smpl_nrg(&noise_v[..l]);
        let inv = 1.0 / (nrg_noise + 1e-12);
        for i in 0..SMPL_NOISE_CORR_ORDER + 1 {
            ctgt[i] *= inv;
        }
        let mut coef_ma = [0.0f32; SMPL_NOISE_CORR_ORDER + 1];
        smpl_spec_fact2(&ctgt, &mut coef_ma);
        smpl_filt_ma2(&noise_v, l, &coef_ma, &mut ng.shape_state, &mut noise_v2);

        if !ng.prev_voiced {
            smpl_gen_rand_pulses(&mut noise_uv, l, &mut ng.rand_seed);
            let mut env_val = ng.env_last * SMPL_ENV_SMTH_COEF_UV_V;
            let mut i = 0;
            while i < l {
                noise_uv[i] *= env_val;
                noise_uv[i + 1] *= env_val * SMPL_ENV_SMTH_COEF_UV_V;
                env_val *= SMPL_ENV_SMTH_COEF_UV_V * SMPL_ENV_SMTH_COEF_UV_V;
                i += 2;
            }
        } else if ng.since_unvoiced < 2 {
            for v in noise_uv.iter_mut().take(l) {
                *v = 0.0;
            }
        }
        ng.env_last = env[l - 1];
    } else {
        for v in ng.corr_smth.iter_mut() {
            *v = 0.0;
        }
        for v in ng.shape_state.iter_mut() {
            *v = 0.0;
        }
        for v in noise_v2.iter_mut().take(l) {
            *v = 0.0;
        }

        let nrg_tgt;
        if num_pulses > 0 {
            nrg_ratio = smpl_nrg(&exc_lpc[..l]) / (nrgres + 1e-20);
            let hardness = 10.0 + 20.0 * normalized_bitrate;
            nrg_tgt = nrgres * ((hardness * (1.0 - nrg_ratio)).exp() + 1.0).ln() / hardness;
            smpl_get_env(
                exc_lpc,
                l,
                SMPL_ENV_SMTH_COEF_UV,
                &mut ng.env_smth,
                &mut env,
            );
        } else {
            nrg_ratio = 0.0;
            nrg_tgt = nrgres;
            smpl_get_env0(l, SMPL_ENV_SMTH_COEF_UV, &mut ng.env_smth, &mut env);
        }

        let mut scale = 1.0 / l as f32;
        let nrg_tgt = nrg_tgt * scale + 1e-30;
        let nrg_env = smpl_nrg(&env[..l]) * scale;
        let mut f = nrg_tgt.sqrt();
        let mut g = (nrg_tgt / nrg_env).sqrt();
        let ge = g * env[0];
        let env_last = ng.env_last;
        if env_last < f.min(ge) {
            if f < ge {
                g = 0.0;
            } else {
                f = 0.0;
            }
        } else if env_last > f.max(ge) {
            if f > ge {
                g = 0.0;
            } else {
                f = 0.0;
            }
        } else {
            let sum_env = smpl_sum(&env[..l]) * scale;
            let a = nrg_env + env[0] * env[0] - 2.0 * sum_env * env[0];
            let b = 2.0 * env_last * (sum_env - env[0]);
            let cc = env_last * env_last - nrg_tgt;
            let mut tmp = b * b - 4.0 * a * cc;
            if tmp < 1e-35 || a < 1e-25 {
                f = 0.0;
                g = 0.0;
            } else {
                tmp = tmp.sqrt();
                scale = 0.5 / a;
                g = (-b + tmp) * scale;
                f = env_last - env[0] * g;
                if f < 0.0 {
                    g = (-b - tmp) * scale;
                    f = env_last - env[0] * g;
                }
            }
        }

        smpl_gen_rand_pulses(&mut noise_uv, l, &mut ng.rand_seed);
        if num_pulses > 0 {
            let max_val = fcbgains_uv[fcbg_idx as usize] * 0.5;
            for i in 0..l {
                if exc_lpc[i] == 0.0 {
                    noise_uv[i] *= (f + g * env[i]).min(max_val);
                } else {
                    noise_uv[i] = 0.0;
                }
            }
            ng.env_last = (f + g * env[l - 1]).min(max_val);
        } else {
            for i in 0..l {
                noise_uv[i] *= f + g * env[i];
            }
            ng.env_last = f + g * env[l - 1];
        }
    }

    if ng.prev_voiced || voiced {
        smpl_filt_ma2(&noise_v2, l, &COEF_MA_V, &mut ng.out_state_v, noise);
    } else {
        for v in noise.iter_mut().take(l) {
            *v = 0.0;
        }
    }
    if ng.since_unvoiced < 2 || !voiced {
        add_noise_uv(ng, &mut noise_uv, l, lsf, nrg_ratio, noise);
    } else {
        ng.out_state_uv = [0.0, 0.0];
    }
    ng.prev_voiced = voiced;
    if voiced {
        ng.since_unvoiced += 1;
    } else {
        ng.since_unvoiced = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn f32v(v: &Value) -> Vec<f32> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_f64().unwrap() as f32)
            .collect()
    }

    fn ng_from(v: &Value) -> NoiseGenerator {
        let g = |k: &str| v[k].as_f64().unwrap() as f32;
        let arr2 = |k: &str| {
            let a = f32v(&v[k]);
            [a[0], a[1]]
        };
        let arr3 = |k: &str| {
            let a = f32v(&v[k]);
            [a[0], a[1], a[2]]
        };
        NoiseGenerator {
            env_smth: g("env_smth"),
            env_last: g("env_last"),
            out_state_uv: arr2("out_state_uv"),
            out_state_v: arr2("out_state_v"),
            corr_smth: arr3("corr_smth"),
            shape_state: arr2("shape_state"),
            prev_voiced: v["prev_voiced"].as_i64().unwrap() != 0,
            since_unvoiced: v["since_unvoiced"].as_i64().unwrap() as i32,
            rand_seed: v["rand_seed"].as_i64().unwrap() as i32,
        }
    }

    /// T2: gen_noise is PRNG- and state-driven, so the only deterministic validation is
    /// bit-exactness against the reference vectors. Each vector carries the full input
    /// `NoiseGenerator` state, the excitation, params, lsf, and the expected `noise[80]` plus the
    /// output `NoiseGenerator` state (`ng_out`). Seeding the generator with the captured `ng_in` and
    /// running our implementation must reproduce both bit-for-bit across voiced and unvoiced paths.
    #[test]
    fn gen_noise_matches_c() {
        // fcbgains_uv[ix] = 10^(0.05*(ix-90)), ix in 0..=90.
        let fcbgains_uv: Vec<f32> = (0..=90)
            .map(|ix| 10.0f32.powf(0.05 * (ix as f32 - 90.0)))
            .collect();

        let recs: Value =
            serde_json::from_str(include_str!("testdata/gennoise_vectors.json")).unwrap();
        let arr = recs.as_array().unwrap();
        assert!(!arr.is_empty());

        let (mut v_checked, mut uv0_checked, mut uvp_checked) = (0, 0, 0);
        for rec in arr {
            let voiced = rec["voiced"].as_i64().unwrap() == 1;
            let exc = f32v(&rec["exc_pre"]);
            let lsf = f32v(&rec["lsf"]);
            let noise_expected = f32v(&rec["noise"]);
            let nrgres = rec["nrgres"].as_f64().unwrap() as f32;
            let fcbg = rec["fcbg_idx"].as_i64().unwrap() as i32;
            let np = rec["sf_pulses"].as_i64().unwrap() as i32;
            let nbr = rec["norm_br"].as_f64().unwrap() as f32;
            let seed_out = rec["seed_out"].as_i64().unwrap() as i32;

            let mut ng = ng_from(&rec["ng_in"]);
            let ng_out_expected = ng_from(&rec["ng_out"]);
            let mut noise = [0.0f32; SMPL_MAX_SF_LEN];
            smpl_celp_gen_noise(
                &mut ng,
                &exc,
                80,
                voiced,
                np,
                nrgres,
                fcbg,
                &lsf,
                nbr,
                &fcbgains_uv,
                &mut noise,
            );

            // Seed transition is bit-exact (proves the PRNG + gen_rand_pulses call count).
            assert_eq!(
                ng.rand_seed, seed_out,
                "seed_out mismatch (voiced={voiced} np={np})"
            );
            assert_eq!(ng.rand_seed, ng_out_expected.rand_seed, "ng_out.rand_seed");

            // noise[80] bit-exact (tiny tolerance for f32 transcendental last-ULP across libm).
            for i in 0..80 {
                assert!(
                    (noise[i] - noise_expected[i]).abs() < 1e-6,
                    "noise[{i}] mismatch (voiced={voiced} np={np}): rust={} ref={}",
                    noise[i],
                    noise_expected[i]
                );
            }
            // Output generator state bit-exact (env_last / out states drive the next subframe).
            assert!(
                (ng.env_last - ng_out_expected.env_last).abs() < 1e-6,
                "ng_out.env_last (voiced={voiced} np={np}): rust={} ref={}",
                ng.env_last,
                ng_out_expected.env_last
            );
            for k in 0..2 {
                assert!(
                    (ng.out_state_uv[k] - ng_out_expected.out_state_uv[k]).abs() < 1e-6,
                    "ng_out.out_state_uv[{k}]"
                );
                assert!(
                    (ng.out_state_v[k] - ng_out_expected.out_state_v[k]).abs() < 1e-6,
                    "ng_out.out_state_v[{k}]"
                );
            }

            if voiced {
                v_checked += 1;
            } else if np == 0 {
                uv0_checked += 1;
            } else {
                uvp_checked += 1;
            }
        }
        assert!(
            v_checked > 0 && uv0_checked > 0 && uvp_checked > 0,
            "vectors must exercise all three paths: voiced={v_checked} uv0={uv0_checked} uvp={uvp_checked}"
        );
    }
}

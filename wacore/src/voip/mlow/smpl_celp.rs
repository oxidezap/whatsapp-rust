//! MLow (smpl_audio_codec) CELP excitation encoder.
//!
//! This is the correctness-critical per-subframe analyzer. It builds a perceptually weighted impulse
//! response (`Phi`), an LTP/ACB basis for voiced frames, runs the FCB pulse search (greedy +
//! delayed-decision beam), and RD-quantizes the adaptive- and fixed-codebook gains against entropy
//! tables built once at init. `smpl_dcmf_to_cmf` keeps integer truncating division because the RD
//! winner depends on it bit-exactly.
//!
//! Self-contained on purpose: small leaf helpers are duplicated locally rather than shared with
//! `smpl_synth.rs` / `smpl_harmcomb.rs`, so the pipeline integration can be wired separately without
//! coupling.
// The encode path is not yet fully wired: a duplicated leaf vector op (smpl_sub_vec_inplace) and the
// CelpSubframeOut.n_pulses/exc_lpc outputs the consumer doesn't read yet are scaffolding, so
// dead_code is allowed module-wide.
#![allow(dead_code)]
#![allow(clippy::needless_range_loop)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::excessive_precision)]
// SMPL_PI is the exact literal `3.1415926535897f` the codec uses; keep it, do not swap for std PI.
#![allow(clippy::approx_constant)]
// The clamp is written as min(max(x, lo), hi); the `.max().min()` form preserves that operation order.
#![allow(clippy::manual_clamp)]

use std::sync::OnceLock;

// Tier 0: constants.

const SMPL_PI: f32 = 3.1415926535897;

const SMPL_MAX_SF_LEN: usize = 160;
const SMPL_MAX_L_RESP: usize = 33; // 32 + 1
const SMPL_PERC_RESP_LEN: usize = 32;
const SMPL_LPC_ORDER: usize = 16;
const SMPL_LTP_INTERPOL_DELAY: usize = 8;
const SMPL_LAG_SUBFRLEN: usize = 40;
const SMPL_MAXPITCH_LEN: usize = 320; // 20ms * 16kHz
const SMPL_MAX_PITCH_LAG: usize = 320;
const SMPL_MAX_PULSES_PER_SF: usize = 40;

const SMPL_ACBG_N: usize = 16;
const SMPL_ACBG_M: usize = 2;
const SMPL_G_ACB_RD_MU: f32 = 0.014999999664723873;

const SMPL_FCBG_V_N: usize = 34;
const SMPL_FCBG_V_DELTA_N: usize = 67;
const SMPL_UV_GAIN_IDX_LEN: usize = 90; // (0 - (-90)) / 1

const SMPL_V_GAIN_MIN_DB: f32 = -100.0;
const SMPL_V_GAIN_MAX_DB: f32 = 0.0;
const SMPL_V_GAIN_STEP_DB: f32 = 3.0;
const SMPL_UV_GAIN_MIN_DB: f32 = -90.0;
const SMPL_UV_GAIN_MAX_DB: f32 = 0.0;
const SMPL_UV_GAIN_STEP_DB: f32 = 1.0;

const SMPL_RATE_ACB_SCALE: f32 = 0.9;
const SMPL_PITCH_SHARPENING_COEF: f32 = 0.9881;
const SMPL_FCB_SRV_MAX: i32 = 4;
const SMPL_CELP_MAX_NUMSURV: usize = 8;

const SMPL_CELP_IDX_FEC: usize = 0;
const SMPL_CELP_IDX_MAIN: usize = 1;
const SMPL_CELP_MAX_RATES: usize = 2;

const N_GAIN_STEPS: usize = 2;

// Raw constant codec tables.

#[rustfmt::skip]
pub(super) const SMPL_CB_ACBGAINS_LR_Q14: [i16; SMPL_ACBG_N * SMPL_ACBG_M] = [
    2812, 2484,
    0, 0,
    -362, 2465,
    -337, 703,
    3033, 1474,
    13536, 220,
    -2630, 9226,
    6032, 3499,
    -220, 441,
    7661, 4243,
    11521, 0,
    1430, 779,
    4495, 2724,
    15535, 343,
    -779, 1559,
    480, 481,
];

#[rustfmt::skip]
const SMPL_CB_ACBGAINS_HR_Q14: [i16; SMPL_ACBG_N * SMPL_ACBG_M] = [
    16039, 91,
    0, 0,
    4310, 4930,
    -1431, 2862,
    2893, 0,
    8009, 4075,
    2754, 4223,
    8367, 354,
    4640, 1254,
    -176, 2734,
    -1222, 5017,
    -476, 1506,
    11351, 567,
    1243, 0,
    10601, 22,
    14088, 108,
];

#[rustfmt::skip]
pub(super) const SMPL_ACBGAINS_DCMF_LR: [u8; (SMPL_ACBG_N + 1) * SMPL_ACBG_N] = [
    103, 70, 48, 3, 122, 135, 47, 192, 2, 255, 99, 96, 186, 194, 4, 28,
    161, 90, 76, 3, 181, 60, 37, 219, 2, 132, 81, 146, 255, 43, 3, 36,
    114, 222, 55, 6, 203, 34, 42, 154, 6, 255, 33, 209, 225, 78, 6, 45,
    198, 161, 110, 8, 239, 26, 35, 162, 4, 117, 42, 214, 255, 33, 6, 72,
    55, 255, 124, 55, 124, 55, 55, 55, 55, 78, 55, 215, 111, 55, 55, 167,
    154, 136, 77, 4, 220, 33, 38, 166, 2, 144, 50, 196, 255, 43, 4, 41,
    56, 21, 19, 3, 48, 255, 38, 220, 2, 225, 107, 31, 122, 227, 2, 11,
    63, 38, 23, 4, 77, 85, 58, 190, 4, 255, 53, 53, 145, 138, 4, 14,
    95, 47, 33, 2, 110, 146, 53, 255, 2, 219, 79, 73, 198, 122, 2, 15,
    84, 255, 84, 84, 147, 84, 84, 84, 84, 120, 84, 120, 84, 84, 84, 84,
    73, 58, 25, 1, 95, 99, 52, 175, 1, 255, 48, 69, 151, 184, 1, 15,
    105, 32, 43, 2, 84, 225, 34, 255, 2, 156, 129, 49, 189, 124, 3, 19,
    152, 230, 89, 6, 253, 28, 40, 153, 2, 195, 31, 255, 249, 58, 5, 61,
    138, 84, 54, 3, 173, 96, 45, 247, 2, 176, 83, 128, 255, 69, 2, 26,
    22, 17, 8, 1, 23, 106, 26, 88, 1, 182, 37, 18, 50, 255, 1, 6,
    218, 174, 228, 65, 186, 65, 65, 92, 65, 65, 65, 255, 174, 65, 65, 174,
    117, 255, 101, 16, 180, 20, 33, 94, 10, 131, 20, 222, 143, 38, 15, 105,
];

#[rustfmt::skip]
const SMPL_ACBGAINS_DCMF_HR: [u8; (SMPL_ACBG_N + 1) * SMPL_ACBG_N] = [
    254, 105, 212, 26, 110, 255, 202, 93, 152, 121, 110, 43, 150, 20, 81, 176,
    255, 28, 100, 5, 26, 184, 61, 29, 36, 26, 28, 9, 61, 4, 27, 116,
    121, 255, 161, 39, 195, 215, 191, 75, 186, 178, 119, 82, 68, 41, 43, 56,
    188, 65, 243, 15, 74, 255, 205, 79, 123, 84, 95, 26, 139, 13, 67, 154,
    81, 219, 173, 70, 219, 165, 234, 102, 231, 255, 191, 119, 87, 60, 62, 59,
    106, 255, 182, 49, 242, 196, 233, 95, 247, 228, 152, 96, 81, 45, 54, 61,
    236, 55, 178, 10, 56, 255, 131, 54, 85, 58, 59, 18, 93, 9, 43, 133,
    123, 95, 224, 24, 113, 202, 255, 105, 186, 134, 135, 38, 141, 18, 82, 111,
    126, 97, 204, 34, 126, 186, 255, 141, 210, 147, 149, 46, 165, 22, 113, 122,
    96, 156, 185, 42, 188, 178, 255, 116, 248, 199, 157, 66, 109, 29, 69, 75,
    102, 207, 194, 57, 224, 193, 255, 107, 253, 242, 180, 95, 97, 44, 60, 64,
    105, 119, 202, 39, 140, 189, 255, 110, 207, 173, 165, 54, 119, 24, 75, 85,
    74, 255, 142, 59, 214, 150, 182, 76, 194, 215, 138, 122, 61, 56, 41, 45,
    200, 53, 255, 17, 66, 238, 222, 109, 129, 78, 101, 21, 227, 11, 110, 243,
    74, 255, 128, 50, 187, 149, 154, 63, 165, 184, 115, 101, 52, 47, 37, 34,
    159, 66, 232, 26, 86, 196, 255, 146, 171, 113, 134, 31, 245, 16, 145, 190,
    255, 29, 182, 7, 33, 235, 115, 55, 59, 37, 47, 11, 139, 6, 60, 234,
];

#[rustfmt::skip]
const SMPL_FCBG_V_DCMF: [u8; SMPL_FCBG_V_N] = [
    107, 12, 17, 25, 31, 41, 52, 65, 83, 103, 122, 146, 169, 191, 210, 227,
    240, 249, 255, 253, 246, 229, 200, 161, 120, 82, 51, 29, 14, 6, 2, 2,
    2, 2,
];

#[rustfmt::skip]
const SMPL_FCBG_V_DELTA_DCMF: [u8; SMPL_FCBG_V_DELTA_N] = [
    1, 1, 1, 1, 1, 1, 1, 1, 2, 3, 4, 6, 8, 10, 12, 12,
    12, 13, 14, 14, 14, 13, 12, 11, 10, 9, 8, 9, 15, 33, 65, 119,
    196, 255, 220, 144, 90, 57, 36, 23, 17, 14, 12, 12, 12, 13, 12, 12,
    12, 12, 12, 11, 11, 10, 9, 7, 6, 4, 3, 2, 1, 1, 1, 1,
    1, 1, 1,
];

#[rustfmt::skip]
const SMPL_INTERPOL_KERNEL: [f32; 2 * SMPL_LTP_INTERPOL_DELAY] = [
    -6.3925986e-6, 0.00011064114, -0.0009153038, 0.00484772, -0.018698348, 0.05759091, -0.15997477, 0.6170455,
    0.61704546, -0.15997475, 0.057590906, -0.018698348, 0.00484772, -0.0009153038, 0.000110641144, -6.392598e-6,
];

// Tier 1: leaf math helpers.

#[inline]
fn smpl_dot_prod(a: &[f32], b: &[f32], l: usize) -> f32 {
    let mut ret = 0.0f32;
    for i in 0..l {
        ret += a[i] * b[i];
    }
    ret
}

#[inline]
fn smpl_nrg(x: &[f32], n: usize) -> f32 {
    let mut nrg = 0.0f32;
    for k in 0..n {
        nrg += x[k] * x[k];
    }
    nrg
}

#[inline]
fn smpl_reverse(x: &mut [f32], l: usize) {
    for i in 0..l / 2 {
        x.swap(i, l - i - 1);
    }
}

// x -= y
#[inline]
fn smpl_sub_vec_inplace(y: &[f32], x: &mut [f32], l: usize) {
    for i in 0..l {
        x[i] -= y[i];
    }
}

// x = y - z
#[inline]
fn smpl_sub_vec(y: &[f32], z: &[f32], x: &mut [f32], l: usize) {
    for i in 0..l {
        x[i] = y[i] - z[i];
    }
}

// x += y
#[inline]
fn smpl_add_vec_inplace(y: &[f32], x: &mut [f32], l: usize) {
    for i in 0..l {
        x[i] += y[i];
    }
}

#[inline]
fn smpl_scale_vec_inplace(x: &mut [f32], l: usize, g: f32) {
    for i in 0..l {
        x[i] *= g;
    }
}

#[inline]
fn smpl_scale_vec(x: &[f32], y: &mut [f32], l: usize, g: f32) {
    for i in 0..l {
        y[i] = x[i] * g;
    }
}

// y += g * x
#[inline]
fn smpl_add_scale_vec_inplace(x: &[f32], y: &mut [f32], l: usize, g: f32) {
    for i in 0..l {
        y[i] += g * x[i];
    }
}

// y = x0 + g * x1
#[inline]
fn smpl_add_scale_vec(x0: &[f32], x1: &[f32], y: &mut [f32], l: usize, g: f32) {
    for i in 0..l {
        y[i] = x0[i] + g * x1[i];
    }
}

#[inline]
fn smpl_mul_vec_inplace(x: &[f32], y: &mut [f32], l: usize) {
    for i in 0..l {
        y[i] *= x[i];
    }
}

#[inline]
fn smpl_celp_q(num: &[f32], den: &[f32], l: usize, q: &mut [f32]) {
    for i in 0..l {
        q[i] = (num[i] * num[i]) / den[i];
    }
}

/// `y[n] = sum C[.] * x[.]` for a symmetric Toeplitz multiply. `C` must carry the trailing zero at
/// index `2*L_resp-1` and `x` must be readable up to `N + L_resp` (zero padded past `N`): the inner
/// loop deliberately reads one extra zero element for SIMD friendliness.
fn smpl_mult_symtoepl2(c: &[f32], l_resp: usize, x: &[f32], y: &mut [f32], n: usize) {
    debug_assert!(2 * l_resp <= n);
    debug_assert!(c[2 * l_resp - 1] == 0.0);

    let mut len = l_resp;
    let mut nn = 0usize;
    while nn < l_resp - 1 {
        y[nn] = smpl_dot_prod(&c[l_resp - 1 - nn..], &x[0..], len);
        len += 1;
        nn += 1;
    }
    len = 2 * l_resp;
    while nn < n - l_resp {
        // nn >= l_resp-1 here, so (nn + 1 - l_resp) avoids the usize underflow of nn - l_resp + 1.
        y[nn] = smpl_dot_prod(&c[0..], &x[nn + 1 - l_resp..], len);
        nn += 1;
    }
    while nn < n {
        len -= 1;
        y[nn] = smpl_dot_prod(&c[0..], &x[nn + 1 - l_resp..], len);
        nn += 1;
    }
}

/// 16th-order AR filter. The 16-sample state sits in `y[base-16 .. base]`; `x`/`y` are slices whose
/// index 0 is the logical sample 0 (so callers pass an offset slice for the history).
fn smpl_filt_ar16(x: &[f32], n: usize, coef: &[f32], y_base: usize, y: &mut [f32]) {
    debug_assert!(coef[0] == 1.0);
    for nn in 0..n {
        let mut res = x[nn];
        for i in 0..16 {
            res -= coef[16 - i] * y[y_base + nn - 16 + i];
        }
        y[y_base + nn] = res;
    }
}

/// MA filter: the `(coef_len-1)` history samples sit before `x[0]` (caller passes an offset). `x != y`.
fn smpl_filt_ma(x: &[f32], x_base: usize, n: usize, coef: &[f32], coef_len: usize, y: &mut [f32]) {
    let mut i;
    if coef[0] == 1.0 {
        // y[k] = x[k] + coef[1]*x[k-1]
        for k in 0..n {
            y[k] = x[x_base + k] + coef[1] * x[x_base + k - 1];
        }
        i = 2;
    } else {
        for k in 0..n {
            y[k] = coef[0] * x[x_base + k];
        }
        i = 1;
    }
    while i < coef_len {
        for k in 0..n {
            y[k] += coef[i] * x[x_base + k - i];
        }
        i += 1;
    }
}

/// 9th-order MA. The 9-sample history sits before `x[0]` (caller passes an offset).
fn smpl_filt_ma9(
    x: &[f32],
    x_base: usize,
    n: usize,
    coef: &[f32],
    _coef_len: usize,
    y: &mut [f32],
) {
    for nn in 0..n {
        let mut res = 0.0f32;
        for i in 0..10 {
            res += coef[i] * x[x_base + nn - i];
        }
        y[nn] = res;
    }
}

/// `smpl_dcmf_to_cmf` returning the cumulative u16 CDF (len `dcmf.len()+1`).
pub(super) fn dcmf_to_cmf(dcmf: &[u8]) -> Vec<u16> {
    let mut cmf = vec![0u16; dcmf.len() + 1];
    smpl_dcmf_to_cmf(dcmf, dcmf.len(), &mut cmf);
    cmf
}

/// INTEGER, bit-exact: `cmf[n+1] = min((dcmf[n]+1)^2, 65535)`, then truncating-int normalize. Note the
/// `(32767 - dcmf_len)` uses the dcmf length, not the cmf length; this is intentional, not a typo.
fn smpl_dcmf_to_cmf(dcmf: &[u8], dcmf_len: usize, cmf: &mut [u16]) {
    let mut sum: i32 = 0;
    for n in 0..dcmf_len {
        let mut tmp: i32 = dcmf[n] as i32;
        tmp += 1;
        tmp *= tmp;
        if tmp > 65535 {
            tmp = 65535;
        }
        cmf[n + 1] = tmp as u16;
        sum += tmp;
    }
    cmf[0] = 0;
    for n in 1..dcmf_len + 1 {
        cmf[n] = cmf[n - 1] + ((cmf[n] as i32 * (32767 - dcmf_len as i32)) / sum) as u16 + 1;
    }
}

fn smpl_cmf_to_bits(cmf: &[u16], cmf_len: usize, bits: &mut [f32]) {
    for i in 0..cmf_len - 1 {
        bits[i] = -((cmf[i + 1] - cmf[i]) as f32 / cmf[cmf_len - 1] as f32).log2();
    }
}

/// argmax (lowest index on tie). The reference resolves ties to the lowest index; a forward scan that
/// only replaces on strict `>` is equivalent.
#[inline]
fn smpl_get_maxi(x: &[f32], x_len: usize) -> usize {
    let mut i = 0usize;
    let mut maxtmp = x[0];
    for n in 1..x_len {
        if x[n] > maxtmp {
            maxtmp = x[n];
            i = n;
        }
    }
    i
}

/// Top-K indices, descending by value: the K largest in descending order (the deldec search relies on
/// `idx[0]` being the best). Ties resolve to the lowest index, matching the reference for distinct
/// floats.
fn smpl_get_maxi_k(x: &[f32], idx: &mut [i32], x_len: usize, k: usize) {
    // Partial selection: repeatedly take the current max, masking taken indices.
    // Stack-scratch the mask: x_len is bounded by the subframe length and the
    // candidate count (NUMSURV^2 = 64), both <= SMPL_MAX_SF_LEN, so this avoids
    // a heap alloc on the hottest allocation site in the encoder (~500x/frame).
    debug_assert!(x_len <= SMPL_MAX_SF_LEN);
    let mut taken_buf = [false; SMPL_MAX_SF_LEN];
    let taken = &mut taken_buf[..x_len];
    for kk in 0..k {
        let mut best = -f32::MAX;
        let mut bi = 0usize;
        let mut found = false;
        for n in 0..x_len {
            if !taken[n] && (!found || x[n] > best) {
                best = x[n];
                bi = n;
                found = true;
            }
        }
        taken[bi] = true;
        idx[kk] = bi as i32;
    }
}

// Tier 2: entropy/gain table builder.

pub(crate) struct CelpTables {
    acbgains_cmf_lr: [u16; (SMPL_ACBG_N + 1) * (SMPL_ACBG_N + 1)],
    acbgains_cmf_hr: [u16; (SMPL_ACBG_N + 1) * (SMPL_ACBG_N + 1)],
    acbg_inv_prob_lr: [f32; (SMPL_ACBG_N + 1) * SMPL_ACBG_N],
    acbg_inv_prob_hr: [f32; (SMPL_ACBG_N + 1) * SMPL_ACBG_N],
    fcbgains_v_cmf: [u16; SMPL_FCBG_V_N + 1],
    fcbgains_v_delta_cmf: [u16; SMPL_FCBG_V_DELTA_N + 1],
    fcbgains_v: [f32; SMPL_FCBG_V_N],
    fcbgains_uv: [f32; SMPL_UV_GAIN_IDX_LEN + 1],
    fcbg_v_inv_prob: [f32; SMPL_FCBG_V_N],
    fcbg_v_delta_inv_prob: [f32; SMPL_FCBG_V_DELTA_N],
}

static CELP_TABLES: OnceLock<CelpTables> = OnceLock::new();

fn celp_tables() -> &'static CelpTables {
    CELP_TABLES.get_or_init(build_celp_tables)
}

fn build_celp_tables() -> CelpTables {
    let mut t = CelpTables {
        acbgains_cmf_lr: [0; (SMPL_ACBG_N + 1) * (SMPL_ACBG_N + 1)],
        acbgains_cmf_hr: [0; (SMPL_ACBG_N + 1) * (SMPL_ACBG_N + 1)],
        acbg_inv_prob_lr: [0.0; (SMPL_ACBG_N + 1) * SMPL_ACBG_N],
        acbg_inv_prob_hr: [0.0; (SMPL_ACBG_N + 1) * SMPL_ACBG_N],
        fcbgains_v_cmf: [0; SMPL_FCBG_V_N + 1],
        fcbgains_v_delta_cmf: [0; SMPL_FCBG_V_DELTA_N + 1],
        fcbgains_v: [0.0; SMPL_FCBG_V_N],
        fcbgains_uv: [0.0; SMPL_UV_GAIN_IDX_LEN + 1],
        fcbg_v_inv_prob: [0.0; SMPL_FCBG_V_N],
        fcbg_v_delta_inv_prob: [0.0; SMPL_FCBG_V_DELTA_N],
    };

    // (a) acbgains cmf, 17 rows of 16-dcmf -> 17-cmf
    for i in 0..SMPL_ACBG_N + 1 {
        smpl_dcmf_to_cmf(
            &SMPL_ACBGAINS_DCMF_LR[i * SMPL_ACBG_N..],
            SMPL_ACBG_N,
            &mut t.acbgains_cmf_lr[i * (SMPL_ACBG_N + 1)..],
        );
        smpl_dcmf_to_cmf(
            &SMPL_ACBGAINS_DCMF_HR[i * SMPL_ACBG_N..],
            SMPL_ACBG_N,
            &mut t.acbgains_cmf_hr[i * (SMPL_ACBG_N + 1)..],
        );
    }

    // (b) acbg inv-prob: per row, cmf_to_bits(17) -> bits[16], then 2^(bits*mu)
    for i in 0..SMPL_ACBG_N + 1 {
        smpl_cmf_to_bits(
            &t.acbgains_cmf_lr[i * (SMPL_ACBG_N + 1)..],
            SMPL_ACBG_N + 1,
            &mut t.acbg_inv_prob_lr[i * SMPL_ACBG_N..],
        );
        smpl_cmf_to_bits(
            &t.acbgains_cmf_hr[i * (SMPL_ACBG_N + 1)..],
            SMPL_ACBG_N + 1,
            &mut t.acbg_inv_prob_hr[i * SMPL_ACBG_N..],
        );
        for j in 0..SMPL_ACBG_N {
            t.acbg_inv_prob_lr[i * SMPL_ACBG_N + j] =
                2.0f32.powf(t.acbg_inv_prob_lr[i * SMPL_ACBG_N + j] * SMPL_G_ACB_RD_MU);
            t.acbg_inv_prob_hr[i * SMPL_ACBG_N + j] =
                2.0f32.powf(t.acbg_inv_prob_hr[i * SMPL_ACBG_N + j] * SMPL_G_ACB_RD_MU);
        }
    }

    // (c) fcb voiced gain cmfs + inv-prob
    smpl_dcmf_to_cmf(&SMPL_FCBG_V_DCMF, SMPL_FCBG_V_N, &mut t.fcbgains_v_cmf);
    smpl_dcmf_to_cmf(
        &SMPL_FCBG_V_DELTA_DCMF,
        SMPL_FCBG_V_DELTA_N,
        &mut t.fcbgains_v_delta_cmf,
    );
    smpl_cmf_to_bits(&t.fcbgains_v_cmf, SMPL_FCBG_V_N + 1, &mut t.fcbg_v_inv_prob);
    for i in 0..SMPL_FCBG_V_N {
        t.fcbg_v_inv_prob[i] = 2.0f32.powf(t.fcbg_v_inv_prob[i] * SMPL_G_ACB_RD_MU);
    }
    smpl_cmf_to_bits(
        &t.fcbgains_v_delta_cmf,
        SMPL_FCBG_V_DELTA_N + 1,
        &mut t.fcbg_v_delta_inv_prob,
    );
    for i in 0..SMPL_FCBG_V_DELTA_N {
        t.fcbg_v_delta_inv_prob[i] = 2.0f32.powf(t.fcbg_v_delta_inv_prob[i] * SMPL_G_ACB_RD_MU);
    }

    // (d) gain magnitude tables
    for ix in 0..SMPL_FCBG_V_N {
        let fcb_gain_db = ix as f32 * SMPL_V_GAIN_STEP_DB + SMPL_V_GAIN_MIN_DB;
        t.fcbgains_v[ix] = 10.0f32.powf(0.05 * fcb_gain_db);
    }
    for ix in 0..=SMPL_UV_GAIN_IDX_LEN {
        let fcb_gain_db = ix as f32 * SMPL_UV_GAIN_STEP_DB + SMPL_UV_GAIN_MIN_DB;
        t.fcbgains_uv[ix] = 10.0f32.powf(0.05 * fcb_gain_db);
    }

    t
}

// Tier 3: LTP / ACB synthesis.

#[inline]
fn acb_dequant(low_rate: bool, acb_idx: i32, acb_g: &mut [f32; SMPL_ACBG_M]) {
    let cb: &[i16] = if low_rate {
        &SMPL_CB_ACBGAINS_LR_Q14
    } else {
        &SMPL_CB_ACBGAINS_HR_Q14
    };
    let sc_q14 = 1.0f32 / ((1i32) << 14) as f32;
    for m in 0..SMPL_ACBG_M {
        acb_g[m] = cb[acb_idx as usize * SMPL_ACBG_M + m] as f32 * sc_q14;
    }
}

/// Gain adjustment with `high_boost==0` is a no-op (the encoder always passes 0), so the synth is just
/// the 3-tap symmetric basis combination.
fn acb_synthesize(
    fcb_subfrlen: usize,
    acb_basis: &[f32],
    acb_g: &[f32; SMPL_ACBG_M],
    acb: &mut [f32],
) {
    smpl_scale_vec(acb_basis, acb, fcb_subfrlen, acb_g[0]);
    smpl_add_scale_vec_inplace(&acb_basis[fcb_subfrlen..], acb, fcb_subfrlen, acb_g[1]);
}

#[inline]
fn smpl_pitch_sharp(x: &mut [f32], lag: usize, l: usize) {
    for i in lag..l {
        x[i] += x[i - lag] * SMPL_PITCH_SHARPENING_COEF;
    }
}

/// Builds the LTP basis (basis0 = pitch-extended excitation, basis1 = its 3-tap symmetric neighbor
/// sum) per 40-sample sub-block, and MUTATES `state` in place by extending the excitation forward.
/// `state` is the full `acb_state`; `state_off` is `&state[state_len - n_lags*40]`.
fn smpl_syn_ltp_basis(
    lags: &[f32],
    n_lags: usize,
    state: &mut [f32],
    state_len: usize,
    acb_basis: &mut [f32],
) {
    debug_assert!(state_len > 0);
    let mut p = state_len - n_lags * SMPL_LAG_SUBFRLEN; // index into `state` at the excitation tail
    for subfr in 0..n_lags {
        let i_lag = lags[subfr].floor() as i32;
        if (i_lag as f32) == lags[subfr] {
            let il = i_lag as usize;
            for i in 0..SMPL_LAG_SUBFRLEN {
                // p[i] = p[i - i_lag]; i_lag <= i offset so source already written/available
                state[p + i] = state[(p + i) - il];
            }
            for i in 0..SMPL_LAG_SUBFRLEN {
                acb_basis[subfr * SMPL_LAG_SUBFRLEN + i] = state[p + i];
            }
            // basis1[i] = p[i - i_lag - 1] + p[i - i_lag + 1]
            for i in 0..SMPL_LAG_SUBFRLEN {
                let a = state[(p + i) - il - 1];
                let b = state[(p + i) - il + 1];
                acb_basis[(n_lags + subfr) * SMPL_LAG_SUBFRLEN + i] = a + b;
            }
        } else {
            // Fractional lag.
            // first = dot(p[-i_lag-8 ..], kernel, 16)
            let il = i_lag; // may be used in signed arithmetic
            let base_first = (p as i32) + (-1 - il - SMPL_LTP_INTERPOL_DELAY as i32);
            let first = smpl_dot_prod(
                &state[base_first as usize..],
                &SMPL_INTERPOL_KERNEL,
                2 * SMPL_LTP_INTERPOL_DELAY,
            );
            // smpl_interpol(p - i_lag - 8, p, 40)  (writes p[0..40] from the history before it)
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
            // last = dot(p[SMPL_LAG_SUBFRLEN - i_lag - 8 ..], kernel, 16). No -1: matches the C
            // reference (smpl_celp_util.c uses i==SMPL_LAG_SUBFRLEN) and our own decoder basis.
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
            // basis1[1..39] = p[i] + p[i+2] for i in 0..38 -> dst index i+1
            for i in 0..SMPL_LAG_SUBFRLEN - 2 {
                acb_basis[b1 + 1 + i] = state[p + i] + state[p + i + 2];
            }
            let i_last = SMPL_LAG_SUBFRLEN - 1;
            acb_basis[b1 + i_last] = state[p + i_last - 1] + last;
        }
        p += SMPL_LAG_SUBFRLEN;
    }
}

// Tier 4: FCB search.

#[derive(Clone)]
struct Fcb {
    wnrg: f32,
    n_pulses: i32,
    pos_new: i32,
    sign_new: f32,
    sgntr: u64,
    fcb_state_idx: usize,
}

impl Default for Fcb {
    fn default() -> Self {
        Fcb {
            wnrg: 0.0,
            n_pulses: 0,
            pos_new: 0,
            sign_new: 0.0,
            sgntr: 0,
            fcb_state_idx: 0,
        }
    }
}

#[derive(Clone)]
struct FcbState {
    // Fixed-length (SMPL_MAX_SF_LEN) scratch indexed by subframe position; never resized. Inline
    // arrays so the per-subframe `new()` and `clone_from` in the FCB survivor search are memcpys
    // instead of heap alloc/free (~300x/frame on the encoder's hottest remaining alloc site).
    pulse_positions: [i32; SMPL_MAX_SF_LEN],
    pulse_signs: [f32; SMPL_MAX_SF_LEN],
    num: [f32; SMPL_MAX_SF_LEN],
    den: [f32; SMPL_MAX_SF_LEN],
}

impl FcbState {
    fn new() -> Self {
        FcbState {
            pulse_positions: [0; SMPL_MAX_SF_LEN],
            pulse_signs: [0.0; SMPL_MAX_SF_LEN],
            num: [0.0; SMPL_MAX_SF_LEN],
            den: [0.0; SMPL_MAX_SF_LEN],
        }
    }
}

fn calc_d_abs_and_sign(d: &[f32], l: usize, d_abs: &mut [f32], d_sign: &mut [f32]) {
    for i in 0..l {
        if d[i] > 0.0 {
            d_abs[i] = d[i];
            d_sign[i] = 1.0;
        } else {
            d_abs[i] = -d[i];
            d_sign[i] = -1.0;
        }
    }
}

fn check_if_better(wnrg: f32, nrg_thr: &mut f32, wnrg_per_pulse: f32) -> bool {
    *nrg_thr += wnrg_per_pulse;
    if wnrg > *nrg_thr {
        *nrg_thr = wnrg;
        true
    } else {
        false
    }
}

/// `PhiFlip` column for `col`: the column starts at `PhiFlip[SMPL_MAX_SF_LEN - col]` and is read at
/// arbitrary non-negative indices, so this returns the start offset into `phi_flip`.
#[inline]
fn phi_col_offset(col: i32) -> i32 {
    SMPL_MAX_SF_LEN as i32 - col
}

#[inline]
fn non_zero_range(col: i32, perc_resp_len: usize, fcb_subfrlen: usize) -> (usize, usize) {
    let lo = (col - perc_resp_len as i32 + 1).max(0) as usize;
    let hi = (col + perc_resp_len as i32).min(fcb_subfrlen as i32) as usize;
    (lo, hi)
}

// Public output of the per-subframe encoder.

pub(crate) struct CelpSubframeOut {
    pub pulses: [Vec<i16>; SMPL_CELP_MAX_RATES],
    pub n_pulses: [i16; SMPL_CELP_MAX_RATES],
    pub acb_idx: [i16; SMPL_CELP_MAX_RATES],
    pub gain_idx: [i16; SMPL_CELP_MAX_RATES],
    pub exc_lpc: Vec<f32>,
}

// ACBG params, carried within a single encode_subframe call.

struct AcbgParams {
    werr_in: f32,
    phi_acb: [f32; SMPL_ACBG_M * SMPL_ACBG_M],
    d_acb_lpc: [f32; SMPL_ACBG_M],
    acb_basis_phi: Vec<f32>, // SMPL_ACBG_M * fcb_subfrlen
}

// Persistent encoder state.

pub(crate) struct CelpEncoder {
    // state_wght has SMPL_LPC_ORDER samples of history before the "logical" start; logical index 0
    // == state_wght_buf[SMPL_LPC_ORDER].
    state_wght_buf: Vec<f32>, // SMPL_MAX_SF_LEN + SMPL_LPC_ORDER
    state_err_lpc_syn: [f32; SMPL_LPC_ORDER],
    hanning_win: Vec<f32>, // perc_resp_len
    sgntrs: Vec<u64>,      // SMPL_MAX_SF_LEN
    acb_state: Vec<f32>,   // SMPL_MAX_PITCH_LAG + SMPL_MAX_SF_LEN + SMPL_LTP_INTERPOL_DELAY
    acb_state_len: usize,
    prev_acb_idx: [i32; SMPL_CELP_MAX_RATES],
    prev_fcb_idx: [i32; SMPL_CELP_MAX_RATES],
    subfr_cnt: i32,
    subfr_per_packet: i32,
    fcb_subfrlen: usize,
    perc_resp_len: usize,
    low_rate: bool,
    ignore_zir: bool,
    fcbgain: f32,
    use_ma9: bool,

    // Scratch reused across calls.
    imp_lpc_buf: Vec<f32>, // SMPL_MAX_SF_LEN + SMPL_LPC_ORDER, logical start at SMPL_LPC_ORDER
    phi: Vec<f32>,         // SMPL_MAX_SF_LEN
    phi_flip: Vec<f32>,    // 2 * SMPL_MAX_SF_LEN
    // Greedy-FCB-search scratch (per subframe + per pulse), each fully rewritten over [0..fcb_subfrlen]
    // before it is read, so it carries no state between calls.
    fcb_greedy: FcbGreedyScratch,
    // Delayed-decision beam-search scratch (the heavier path). `mem::take`n for each search and put
    // back after, so the per-subframe deldec pays no allocation. `reset()` clears its bookkeeping.
    fcb_search: FcbSearchScratch,
    // Per-subframe working buffers for `encode_subframe`, `mem::take`n at the start of each call and
    // put back at the end so the subframe analyzer reuses them instead of allocating the function-level
    // scratch `Vec`s every subframe. Each is `clear()`+`resize(.., 0.0)`d to exactly match a fresh
    // `vec![0.0; N]`; a missed put-back only loses the reuse (the next take re-allocates), never
    // changes output.
    sf: SubframeScratch,
}

/// Pooled per-`encode_subframe` scratch (see [`CelpEncoder::sf`]). Starts empty; each buffer is sized
/// on first use and reused thereafter.
#[derive(Default)]
struct SubframeScratch {
    imp_lpc_rev: Vec<f32>,
    res_lpc_pad: Vec<f32>,
    d_lpc: Vec<f32>,
    zir_lpc: Vec<f32>,
    acb_basis: Vec<f32>,
    acb: Vec<f32>,
    d_ltp: Vec<f32>,
    wtgt_tmp: Vec<f32>,
    wtgt: Vec<f32>,
    exc_fcb: Vec<f32>,
}

impl SubframeScratch {
    /// Take `self.<field>`, then `clear()`+`resize(n, 0.0)` it so it holds exactly `n` zeros -- the
    /// drop-in replacement for `vec![0.0f32; n]` that reuses the pooled allocation.
    fn zeroed(buf: &mut Vec<f32>, n: usize) -> Vec<f32> {
        let mut v = std::mem::take(buf);
        v.clear();
        v.resize(n, 0.0);
        v
    }
}

/// Per-call working buffers for `smpl_fcb_search`, hoisted off the per-frame/per-pulse hot path.
struct FcbGreedyScratch {
    d_abs: [f32; SMPL_MAX_SF_LEN],
    d_sign: [f32; SMPL_MAX_SF_LEN],
    num: [f32; SMPL_MAX_SF_LEN],
    den: [f32; SMPL_MAX_SF_LEN],
    q: [f32; SMPL_MAX_SF_LEN],
}

impl Default for FcbGreedyScratch {
    fn default() -> Self {
        FcbGreedyScratch {
            d_abs: [0.0; SMPL_MAX_SF_LEN],
            d_sign: [0.0; SMPL_MAX_SF_LEN],
            num: [0.0; SMPL_MAX_SF_LEN],
            den: [0.0; SMPL_MAX_SF_LEN],
            q: [0.0; SMPL_MAX_SF_LEN],
        }
    }
}

impl CelpEncoder {
    pub(crate) fn new(
        low_rate: bool,
        perc_resp_len: usize,
        fcb_subfrlen: usize,
        subfr_per_packet: usize,
    ) -> Self {
        debug_assert!(perc_resp_len <= SMPL_MAX_L_RESP);
        debug_assert!(fcb_subfrlen <= SMPL_MAX_SF_LEN);
        // Force table build at first construction.
        let _ = celp_tables();

        let acb_state_len = fcb_subfrlen + SMPL_MAXPITCH_LEN + SMPL_LTP_INTERPOL_DELAY;

        // `sgntrs` are random u64; only used to dedup identical pulse SETS in the deldec beam. A
        // deterministic distinct-per-position sequence is correctness-equivalent (a different beam
        // tie-break can pick a different but equally valid candidate; bit-exact reproduction of the
        // reference's RNG is not required for a correct encoder). Use a fixed LCG so the result is
        // reproducible.
        let mut sgntrs = vec![0u64; SMPL_MAX_SF_LEN];
        let mut s: u64 = 0x9E3779B97F4A7C15;
        for v in sgntrs.iter_mut() {
            s = s
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            *v = s;
        }

        let mut hanning_win = vec![0.0f32; perc_resp_len];
        let scale = 1.0f32 / (2 * SMPL_PERC_RESP_LEN + 1) as f32;
        for i in 0..perc_resp_len {
            hanning_win[i] = (SMPL_PI * (perc_resp_len + i + 1) as f32 * scale).sin();
        }
        let use_ma9 = perc_resp_len == 10;

        CelpEncoder {
            state_wght_buf: vec![0.0; SMPL_MAX_SF_LEN + SMPL_LPC_ORDER],
            state_err_lpc_syn: [0.0; SMPL_LPC_ORDER],
            hanning_win,
            sgntrs,
            acb_state: vec![0.0; SMPL_MAX_PITCH_LAG + SMPL_MAX_SF_LEN + SMPL_LTP_INTERPOL_DELAY],
            acb_state_len,
            prev_acb_idx: [-1; SMPL_CELP_MAX_RATES],
            prev_fcb_idx: [-1; SMPL_CELP_MAX_RATES],
            subfr_cnt: 0,
            subfr_per_packet: subfr_per_packet as i32,
            fcb_subfrlen,
            perc_resp_len,
            low_rate,
            ignore_zir: false,
            fcbgain: 0.0,
            use_ma9,
            imp_lpc_buf: vec![0.0; SMPL_MAX_SF_LEN + SMPL_LPC_ORDER],
            phi: vec![0.0; SMPL_MAX_SF_LEN],
            phi_flip: vec![0.0; 2 * SMPL_MAX_SF_LEN],
            fcb_greedy: FcbGreedyScratch::default(),
            fcb_search: FcbSearchScratch::new(),
            sf: SubframeScratch::default(),
        }
    }

    #[inline]
    fn perc_filt_ma(
        &self,
        x: &[f32],
        x_base: usize,
        n: usize,
        coef: &[f32],
        coef_len: usize,
        y: &mut [f32],
    ) {
        if self.use_ma9 {
            smpl_filt_ma9(x, x_base, n, coef, coef_len, y);
        } else {
            smpl_filt_ma(x, x_base, n, coef, coef_len, y);
        }
    }
}

// FCB search scratch: the deldec-relevant working buffers.

// `Default` is the empty (alloc-free) sentinel: the encoder holds one populated instance (built by
// `new()`) and `mem::take`s it for the duration of a search, swapping this empty placeholder in. The
// search then pays no per-call allocation.
#[derive(Default)]
struct FcbSearchScratch {
    // Double-buffered candidate states (read/write ping-pong).
    fcb_states: [Vec<FcbState>; 2], // each SMPL_CELP_MAX_NUMSURV
    read_idx: usize,
    write_idx: usize,
    fcbs: Vec<Fcb>, // SMPL_CELP_MAX_NUMSURV
    fcbs_size: usize,
    fcb_candidates: Vec<Fcb>, // up to NUMSURV*NUMSURV
    fcb_candidates_size: usize,
    unique_sgntr: Vec<u64>,
    unique_sgntr_size: usize,
    // Per-`add_pulse` working buffers (each call is on the innermost beam loop). `q` is fully written
    // by `smpl_celp_q` before it is read; `dd_den` accumulates so it is zeroed per call. Held here so
    // the beam search does not allocate two `Vec`s per candidate.
    q: Vec<f32>,      // SMPL_MAX_SF_LEN
    dd_den: Vec<f32>, // SMPL_MAX_SF_LEN
    // The pitch-sharpened deldec target. Fully overwritten (`copy_from_slice` then accumulation) over
    // the live region before it is read. (`d_abs`/`d_sign` stay deldec locals: they are passed to
    // `add_pulse` alongside the `&mut sc`, so they cannot also be borrowed from `sc`.)
    d_new: Vec<f32>, // SMPL_MAX_SF_LEN
}

impl FcbSearchScratch {
    fn new() -> Self {
        let mk = || {
            (0..SMPL_CELP_MAX_NUMSURV)
                .map(|_| FcbState::new())
                .collect::<Vec<_>>()
        };
        FcbSearchScratch {
            fcb_states: [mk(), mk()],
            read_idx: 0,
            write_idx: 1,
            fcbs: vec![Fcb::default(); SMPL_CELP_MAX_NUMSURV],
            fcbs_size: 0,
            fcb_candidates: vec![Fcb::default(); SMPL_CELP_MAX_NUMSURV * SMPL_CELP_MAX_NUMSURV],
            fcb_candidates_size: 0,
            unique_sgntr: vec![0; SMPL_CELP_MAX_NUMSURV * SMPL_CELP_MAX_NUMSURV],
            unique_sgntr_size: 0,
            q: vec![0.0; SMPL_MAX_SF_LEN],
            dd_den: vec![0.0; SMPL_MAX_SF_LEN],
            d_new: vec![0.0; SMPL_MAX_SF_LEN],
        }
    }

    /// Reset the bookkeeping to a fresh-search state (the buffers are overwritten before they are
    /// read, so only the indices/sizes need clearing). Lets one pooled instance serve every search.
    fn reset(&mut self) {
        self.read_idx = 0;
        self.write_idx = 1;
        self.fcbs_size = 0;
        self.fcb_candidates_size = 0;
        self.unique_sgntr_size = 0;
    }

    fn swap_rw(&mut self) {
        std::mem::swap(&mut self.read_idx, &mut self.write_idx);
    }

    fn is_unique(&self, sgntr: u64) -> bool {
        for i in 0..self.unique_sgntr_size {
            if self.unique_sgntr[i] == sgntr {
                return false;
            }
        }
        true
    }
}

impl CelpEncoder {
    // Greedy FCB search.
    fn smpl_fcb_search(
        &mut self,
        d: &[f32],
        wnrg_per_pulse: &[f32; SMPL_CELP_MAX_RATES],
        fcb_pulses_max: &[i16; SMPL_CELP_MAX_RATES],
        pulses: &mut [[i16; SMPL_MAX_PULSES_PER_SF]; SMPL_CELP_MAX_RATES],
        n_pulses: &mut [i16; SMPL_CELP_MAX_RATES],
        wnrg: &mut [f32; SMPL_CELP_MAX_RATES],
        gain_from_search: &mut [f32; SMPL_CELP_MAX_RATES],
        fcb_wnrg: &mut [f32; SMPL_CELP_MAX_RATES],
    ) {
        let fcb_subfrlen = self.fcb_subfrlen;
        let perc_resp_len = self.perc_resp_len;
        *n_pulses = [0; SMPL_CELP_MAX_RATES];

        // Split-borrow: the read-only impulse-response (`phi`/`phi_flip`) and the pooled scratch are
        // disjoint encoder fields, so the search keeps both without re-allocating.
        let phi = &self.phi;
        let phi_flip = &self.phi_flip;
        let FcbGreedyScratch {
            d_abs,
            d_sign,
            num,
            den,
            q,
        } = &mut self.fcb_greedy;

        let mut positions = [0i32; SMPL_MAX_PULSES_PER_SF];
        let phi0 = phi[0];
        calc_d_abs_and_sign(d, fcb_subfrlen, d_abs, d_sign);

        for i in 0..fcb_subfrlen {
            den[i] = phi0 + 1e-16;
        }
        num[..fcb_subfrlen].copy_from_slice(&d_abs[..fcb_subfrlen]);
        positions[0] = smpl_get_maxi(num, fcb_subfrlen) as i32;
        let mut nrg_thr = [0.0f32; SMPL_CELP_MAX_RATES];
        let p0 = positions[0] as usize;
        let ratio = num[p0] / den[p0];
        let wnrg_ = num[p0] * ratio;
        if check_if_better(
            wnrg_,
            &mut nrg_thr[SMPL_CELP_IDX_MAIN],
            wnrg_per_pulse[SMPL_CELP_IDX_MAIN],
        ) {
            n_pulses[SMPL_CELP_IDX_MAIN] = 1;
            wnrg[SMPL_CELP_IDX_MAIN] = wnrg_;
            wnrg[SMPL_CELP_IDX_FEC] = wnrg_;
            gain_from_search[SMPL_CELP_IDX_MAIN] = ratio;
            gain_from_search[SMPL_CELP_IDX_FEC] = ratio;
            fcb_wnrg[SMPL_CELP_IDX_MAIN] = den[p0];
            fcb_wnrg[SMPL_CELP_IDX_FEC] = den[p0];
            if fcb_pulses_max[SMPL_CELP_IDX_FEC] > 0 {
                n_pulses[SMPL_CELP_IDX_FEC] = n_pulses[SMPL_CELP_IDX_MAIN];
                wnrg[SMPL_CELP_IDX_FEC] = wnrg[SMPL_CELP_IDX_MAIN];
                gain_from_search[SMPL_CELP_IDX_FEC] = gain_from_search[SMPL_CELP_IDX_MAIN];
                fcb_wnrg[SMPL_CELP_IDX_FEC] = fcb_wnrg[SMPL_CELP_IDX_MAIN];
            }
        }

        for pulse_nr in 1..fcb_pulses_max[SMPL_CELP_IDX_MAIN] as usize {
            let position = positions[pulse_nr - 1];
            let sgn = d_sign[position as usize];
            for i in 0..fcb_subfrlen {
                num[i] += d_abs[position as usize];
            }
            let (nz0, nz1) = non_zero_range(position, perc_resp_len, fcb_subfrlen);
            let col_off = phi_col_offset(position);
            let mut d_den = 0.0f32;
            for i in 0..pulse_nr - 1 {
                let pi = positions[i] as usize;
                d_den += phi_flip[(col_off + pi as i32) as usize] * d_sign[pi];
            }
            d_den *= 2.0 * sgn;
            d_den += phi_flip[(col_off + position) as usize];
            for i in 0..fcb_subfrlen {
                den[i] += d_den;
            }
            for i in nz0..nz1 {
                den[i] += 2.0 * sgn * d_sign[i] * phi_flip[(col_off + i as i32) as usize];
            }
            smpl_celp_q(num, den, fcb_subfrlen, q);
            positions[pulse_nr] = smpl_get_maxi(q, fcb_subfrlen) as i32;
            let pp = positions[pulse_nr] as usize;
            if check_if_better(
                q[pp],
                &mut nrg_thr[SMPL_CELP_IDX_MAIN],
                wnrg_per_pulse[SMPL_CELP_IDX_MAIN],
            ) {
                n_pulses[SMPL_CELP_IDX_MAIN] = (pulse_nr + 1) as i16;
                wnrg[SMPL_CELP_IDX_MAIN] = q[pp];
                gain_from_search[SMPL_CELP_IDX_MAIN] = num[pp] / den[pp];
                fcb_wnrg[SMPL_CELP_IDX_MAIN] = den[pp];
            }
            if fcb_pulses_max[SMPL_CELP_IDX_FEC] as usize >= pulse_nr
                && check_if_better(
                    q[pp],
                    &mut nrg_thr[SMPL_CELP_IDX_FEC],
                    wnrg_per_pulse[SMPL_CELP_IDX_FEC],
                )
            {
                n_pulses[SMPL_CELP_IDX_FEC] = (pulse_nr + 1) as i16;
                wnrg[SMPL_CELP_IDX_FEC] = q[pp];
                gain_from_search[SMPL_CELP_IDX_FEC] = num[pp] / den[pp];
                fcb_wnrg[SMPL_CELP_IDX_FEC] = den[pp];
            }
        }

        for r in SMPL_CELP_IDX_FEC..SMPL_CELP_IDX_MAIN + 1 {
            if nrg_thr[r] > 0.0 {
                for i in 0..n_pulses[r] as usize {
                    let position = positions[i];
                    pulses[r][i] = if d_sign[position as usize] > 0.0 {
                        1 + position as i16
                    } else {
                        -(1 + position as i16)
                    };
                }
            } else {
                wnrg[r] = 0.0;
                gain_from_search[r] = 0.0;
                fcb_wnrg[r] = 0.0;
                n_pulses[r] = 0;
            }
        }
    }
}

impl CelpEncoder {
    // add_pulse: deldec beam helper.
    fn add_pulse(
        &self,
        sc: &mut FcbSearchScratch,
        fcb_idx_in: usize, // index of `fcb` within sc.fcbs
        d_abs: &[f32],
        d_sign: &[f32],
        numsurv: usize,
        idx: usize,
        lag: i32,
        pitch_sharp: f32,
    ) {
        let fcb_subfrlen = self.fcb_subfrlen;
        let perc_resp_len = self.perc_resp_len;

        // Snapshot the read-side fcb (the `fcb` arg is a mutable copy in sc.fcbs[fcb_idx_in]).
        let fcb_pos_new = sc.fcbs[fcb_idx_in].pos_new;
        let fcb_sign_new = sc.fcbs[fcb_idx_in].sign_new;
        let fcb_n_pulses = sc.fcbs[fcb_idx_in].n_pulses;
        let fcb_state_idx = sc.fcbs[fcb_idx_in].fcb_state_idx;
        let fcb_sgntr_base = sc.fcbs[fcb_idx_in].sgntr;

        let read_idx = sc.read_idx;
        let write_idx = sc.write_idx;

        // num_w[i] = num_r[i] + d_abs[pos_new]
        let add = d_abs[fcb_pos_new as usize];
        for i in 0..fcb_subfrlen {
            let v = sc.fcb_states[read_idx][fcb_state_idx].num[i] + add;
            sc.fcb_states[write_idx][idx].num[i] = v;
        }
        // den_w = copy(den_r)
        for i in 0..fcb_subfrlen {
            sc.fcb_states[write_idx][idx].den[i] = sc.fcb_states[read_idx][fcb_state_idx].den[i];
        }

        if pitch_sharp == 0.0 {
            let (nz0, nz1) = non_zero_range(fcb_pos_new, perc_resp_len, fcb_subfrlen);
            let col_off = phi_col_offset(fcb_pos_new);
            let mut d_den = 0.0f32;
            for i in 0..fcb_n_pulses as usize {
                let pos = sc.fcb_states[read_idx][fcb_state_idx].pulse_positions[i];
                let sgn = sc.fcb_states[read_idx][fcb_state_idx].pulse_signs[i];
                d_den += self.phi_flip[(col_off + pos) as usize] * sgn;
            }
            d_den *= 2.0 * fcb_sign_new;
            d_den += self.phi_flip[(col_off + fcb_pos_new) as usize];
            for i in 0..fcb_subfrlen {
                sc.fcb_states[write_idx][idx].den[i] += d_den;
            }
            for i in nz0..nz1 {
                sc.fcb_states[write_idx][idx].den[i] +=
                    2.0 * fcb_sign_new * d_sign[i] * self.phi_flip[(col_off + i as i32) as usize];
            }
        } else {
            // Pitch-sharpened cross terms.
            let mut g1;
            let mut d_den = 0.0f32;
            // cross term: new pulse train vs each previous pulse train
            {
                g1 = 1.0f32;
                let mut pos = fcb_pos_new;
                while pos < fcb_subfrlen as i32 {
                    let col_off = phi_col_offset(pos);
                    for i in 0..fcb_n_pulses as usize {
                        let mut g2 = g1;
                        let pulse_pos = sc.fcb_states[read_idx][fcb_state_idx].pulse_positions[i];
                        let pulse_sgn = sc.fcb_states[read_idx][fcb_state_idx].pulse_signs[i];
                        let mut pos_ = pulse_pos;
                        while pos_ < fcb_subfrlen as i32 {
                            d_den += g2 * self.phi_flip[(col_off + pos_) as usize] * pulse_sgn;
                            g2 *= pitch_sharp;
                            pos_ += lag;
                        }
                    }
                    g1 *= pitch_sharp;
                    pos += lag;
                }
            }
            d_den *= 2.0 * fcb_sign_new;
            // self term: new pulse train vs itself
            {
                g1 = 1.0f32;
                let mut pos1 = fcb_pos_new;
                while pos1 < fcb_subfrlen as i32 {
                    let col_off = phi_col_offset(pos1);
                    let mut g2 = g1;
                    let mut pos2 = fcb_pos_new;
                    while pos2 < fcb_subfrlen as i32 {
                        d_den += g2 * self.phi_flip[(col_off + pos2) as usize];
                        g2 *= pitch_sharp;
                        pos2 += lag;
                    }
                    g1 *= pitch_sharp;
                    pos1 += lag;
                }
            }
            for i in 0..fcb_subfrlen {
                sc.fcb_states[write_idx][idx].den[i] += d_den;
            }
            // dd_den (accumulated, so zero the live region first).
            sc.dd_den[..fcb_subfrlen].fill(0.0);
            g1 = 1.0f32;
            let mut pos = fcb_pos_new;
            while pos < fcb_subfrlen as i32 {
                let (nz0, nz1) = non_zero_range(pos, perc_resp_len, fcb_subfrlen);
                let col_off = phi_col_offset(pos);
                let mut g2 = g1;
                let mut k = 0i32;
                while k < fcb_subfrlen as i32 {
                    let start_i = (nz0 as i32 - k).max(0);
                    let end_i = (fcb_subfrlen as i32 - k).min(nz1 as i32 - k);
                    let mut i = start_i;
                    while i < end_i {
                        sc.dd_den[i as usize] += g2 * self.phi_flip[(col_off + i + k) as usize];
                        i += 1;
                    }
                    g2 *= pitch_sharp;
                    k += lag;
                }
                g1 *= pitch_sharp;
                pos += lag;
            }
            for i in 0..fcb_subfrlen {
                sc.fcb_states[write_idx][idx].den[i] +=
                    2.0 * fcb_sign_new * d_sign[i] * sc.dd_den[i];
            }
        }

        // Append the new pulse to the state.
        for i in 0..fcb_n_pulses as usize {
            sc.fcb_states[write_idx][idx].pulse_positions[i] =
                sc.fcb_states[read_idx][fcb_state_idx].pulse_positions[i];
            sc.fcb_states[write_idx][idx].pulse_signs[i] =
                sc.fcb_states[read_idx][fcb_state_idx].pulse_signs[i];
        }
        sc.fcb_states[write_idx][idx].pulse_positions[fcb_n_pulses as usize] = fcb_pos_new;
        sc.fcb_states[write_idx][idx].pulse_signs[fcb_n_pulses as usize] = fcb_sign_new;

        // fcb->n_pulses++, fcb->fcb_state_idx = idx (mutate the read copy)
        let new_n_pulses = fcb_n_pulses + 1;

        // Q = num^2/den; top-numsurv -> candidates with unique-signature dedup.
        smpl_celp_q(
            &sc.fcb_states[write_idx][idx].num,
            &sc.fcb_states[write_idx][idx].den,
            fcb_subfrlen,
            &mut sc.q,
        );
        let mut sort_ix = [0i32; SMPL_CELP_MAX_NUMSURV];
        smpl_get_maxi_k(&sc.q, &mut sort_ix, fcb_subfrlen, numsurv);

        for i in 0..numsurv {
            let pos = sort_ix[i] as usize;
            let sgntr = fcb_sgntr_base.wrapping_add(self.sgntrs[pos]);
            if sc.is_unique(sgntr) {
                let cand = Fcb {
                    wnrg: sc.q[pos],
                    n_pulses: new_n_pulses,
                    pos_new: pos as i32,
                    sign_new: d_sign[pos],
                    sgntr,
                    fcb_state_idx: idx,
                };
                let cs = sc.fcb_candidates_size;
                sc.fcb_candidates[cs] = cand;
                sc.fcb_candidates_size += 1;
                let us = sc.unique_sgntr_size;
                sc.unique_sgntr[us] = sgntr;
                sc.unique_sgntr_size += 1;
            }
        }
    }

    // Delayed-decision beam FCB search.
    fn smpl_fcb_search_deldec(
        &self,
        sc: &mut FcbSearchScratch,
        d: &[f32],
        mut pitch_sharp: f32,
        lag: i32,
        wnrg_per_pulse: &[f32; SMPL_CELP_MAX_RATES],
        fcb_pulses_max: &[i16; SMPL_CELP_MAX_RATES],
        surv: &[i16],
        pulses: &mut [[i16; SMPL_MAX_PULSES_PER_SF]; SMPL_CELP_MAX_RATES],
        n_pulses: &mut [i16; SMPL_CELP_MAX_RATES],
        wnrg: &mut [f32; SMPL_CELP_MAX_RATES],
        gain_from_search: &mut [f32; SMPL_CELP_MAX_RATES],
        fcb_wnrg: &mut [f32; SMPL_CELP_MAX_RATES],
    ) {
        let fcb_subfrlen = self.fcb_subfrlen;
        sc.reset();

        // `d_abs`/`d_sign` are locals (passed to `add_pulse` next to `&mut sc`); `d_new` is pooled.
        let mut d_abs = vec![0.0f32; SMPL_MAX_SF_LEN];
        let mut d_sign = vec![0.0f32; SMPL_MAX_SF_LEN];
        let phi0 = self.phi[0];

        if pitch_sharp != 0.0 && lag > 0 && lag < fcb_subfrlen as i32 {
            sc.d_new[..fcb_subfrlen].copy_from_slice(&d[..fcb_subfrlen]);
            for j in 0..fcb_subfrlen {
                let mut g = pitch_sharp;
                let mut i = lag + j as i32;
                while i < fcb_subfrlen as i32 {
                    sc.d_new[j] += g * d[i as usize];
                    g *= pitch_sharp;
                    i += lag;
                }
            }
            calc_d_abs_and_sign(&sc.d_new, fcb_subfrlen, &mut d_abs, &mut d_sign);
        } else {
            calc_d_abs_and_sign(d, fcb_subfrlen, &mut d_abs, &mut d_sign);
            pitch_sharp = 0.0;
        }

        // (read_idx/write_idx already set by `sc.reset()` above.)
        let mut best_fcb: [Fcb; SMPL_CELP_MAX_RATES] = [Fcb::default(), Fcb::default()];
        let mut best_fcb_state: [FcbState; SMPL_CELP_MAX_RATES] =
            [FcbState::new(), FcbState::new()];
        let mut nrg_thr = [0.0f32; SMPL_CELP_MAX_RATES];

        // Initialize the first state buffer (write_idx slot 0).
        {
            let wi = sc.write_idx;
            sc.fcb_states[wi][0].num[..fcb_subfrlen].copy_from_slice(&d_abs[..fcb_subfrlen]);
            if pitch_sharp == 0.0 {
                for i in 0..fcb_subfrlen {
                    sc.fcb_states[wi][0].den[i] = phi0 + 1e-16;
                }
            } else {
                let mut offset = fcb_subfrlen as i32 - 1;
                let mut i = fcb_subfrlen as i32 - 1;
                while i >= 0 {
                    let mut res = 1e-16f32;
                    let mut g_1 = 1.0f32;
                    let mut j = i;
                    while j < fcb_subfrlen as i32 {
                        let col_off = phi_col_offset(j);
                        let mut g_2 = 1.0f32;
                        let mut k = i;
                        while k < fcb_subfrlen as i32 {
                            res += g_1 * g_2 * self.phi_flip[(col_off + k) as usize];
                            g_2 *= pitch_sharp;
                            k += lag;
                        }
                        g_1 *= pitch_sharp;
                        j += lag;
                    }
                    let len = lag.min(offset + 1);
                    for jj in 0..len {
                        sc.fcb_states[wi][0].den[(offset - jj) as usize] = res;
                    }
                    offset -= len;
                    i -= lag;
                }
            }
        }

        sc.swap_rw();
        {
            // fcb_state is the slot just written: after the swap, read_idx points back at
            // fcb_states[old write_idx][0].
            let ri = sc.read_idx; // after swap, this equals old write_idx
            // Split-borrow the read-side state and `q` (disjoint fields) so the copy/Q stays alloc-free.
            let FcbSearchScratch { fcb_states, q, .. } = &mut *sc;
            let st = &fcb_states[ri][0];
            if pitch_sharp == 0.0 {
                q[..fcb_subfrlen].copy_from_slice(&st.num[..fcb_subfrlen]);
            } else {
                smpl_celp_q(&st.num, &st.den, fcb_subfrlen, q);
            }
        }

        let mut sort_ix = [0i32; SMPL_CELP_MAX_NUMSURV];
        smpl_get_maxi_k(&sc.q, &mut sort_ix, fcb_subfrlen, surv[0] as usize);
        sc.fcbs_size = 0;
        {
            let ri = sc.read_idx;
            for i in 0..surv[0] as usize {
                let pos = sort_ix[i] as usize;
                let fcb = Fcb {
                    sgntr: self.sgntrs[pos],
                    pos_new: pos as i32,
                    sign_new: d_sign[pos],
                    wnrg: (sc.fcb_states[ri][0].num[pos] * sc.fcb_states[ri][0].num[pos])
                        / sc.fcb_states[ri][0].den[pos],
                    n_pulses: 0,
                    fcb_state_idx: 0,
                };
                let s = sc.fcbs_size;
                sc.fcbs[s] = fcb;
                sc.fcbs_size += 1;
            }
        }

        self.check_if_better_deldec(
            &*sc,
            0,
            &mut best_fcb[SMPL_CELP_IDX_MAIN],
            &mut best_fcb_state[SMPL_CELP_IDX_MAIN],
            &mut nrg_thr[SMPL_CELP_IDX_MAIN],
            wnrg_per_pulse[SMPL_CELP_IDX_MAIN],
        );
        if fcb_pulses_max[SMPL_CELP_IDX_FEC] > 0 {
            self.check_if_better_deldec(
                &*sc,
                0,
                &mut best_fcb[SMPL_CELP_IDX_FEC],
                &mut best_fcb_state[SMPL_CELP_IDX_FEC],
                &mut nrg_thr[SMPL_CELP_IDX_FEC],
                wnrg_per_pulse[SMPL_CELP_IDX_FEC],
            );
        }

        if fcb_pulses_max[SMPL_CELP_IDX_MAIN] > 1 {
            for pulse_nr in 2..fcb_pulses_max[SMPL_CELP_IDX_MAIN] as usize {
                sc.fcb_candidates_size = 0;
                sc.unique_sgntr_size = 0;
                let fcbs_size = sc.fcbs_size;
                // `idx` increments lockstep with the loop counter, so idx == i.
                for i in 0..fcbs_size {
                    self.add_pulse(
                        &mut *sc,
                        i,
                        &d_abs,
                        &d_sign,
                        surv[pulse_nr - 1] as usize,
                        i,
                        lag,
                        pitch_sharp,
                    );
                }
                sc.swap_rw();
                // Sort candidates by wnrg.
                let cand_size = sc.fcb_candidates_size;
                for i in 0..cand_size {
                    sc.q[i] = sc.fcb_candidates[i].wnrg;
                }
                smpl_get_maxi_k(&sc.q, &mut sort_ix, cand_size, surv[pulse_nr - 1] as usize);
                sc.fcbs_size = 0;
                for i in 0..surv[pulse_nr - 1] as usize {
                    let c = sc.fcb_candidates[sort_ix[i] as usize].clone();
                    let s = sc.fcbs_size;
                    sc.fcbs[s] = c;
                    sc.fcbs_size += 1;
                }
                self.check_if_better_deldec(
                    &*sc,
                    0,
                    &mut best_fcb[SMPL_CELP_IDX_MAIN],
                    &mut best_fcb_state[SMPL_CELP_IDX_MAIN],
                    &mut nrg_thr[SMPL_CELP_IDX_MAIN],
                    wnrg_per_pulse[SMPL_CELP_IDX_MAIN],
                );
                if fcb_pulses_max[SMPL_CELP_IDX_FEC] as usize >= pulse_nr {
                    self.check_if_better_deldec(
                        &*sc,
                        0,
                        &mut best_fcb[SMPL_CELP_IDX_FEC],
                        &mut best_fcb_state[SMPL_CELP_IDX_FEC],
                        &mut nrg_thr[SMPL_CELP_IDX_FEC],
                        wnrg_per_pulse[SMPL_CELP_IDX_FEC],
                    );
                }
            }
            // Last pulse (surv=1, MAIN only).
            sc.fcb_candidates_size = 0;
            sc.unique_sgntr_size = 0;
            let fcbs_size = sc.fcbs_size;
            for i in 0..fcbs_size {
                self.add_pulse(&mut *sc, i, &d_abs, &d_sign, 1, i, lag, pitch_sharp);
            }
            sc.swap_rw();
            let mut best_idx = 0usize;
            let mut max_wnrg = sc.fcb_candidates[0].wnrg;
            for i in 1..sc.fcb_candidates_size {
                if sc.fcb_candidates[i].wnrg > max_wnrg {
                    max_wnrg = sc.fcb_candidates[i].wnrg;
                    best_idx = i;
                }
            }
            // check_if_better_deldec on a candidate (not in fcbs); its fcb_state_idx points into the
            // current read buffer.
            self.check_if_better_deldec_cand(
                &*sc,
                best_idx,
                &mut best_fcb[SMPL_CELP_IDX_MAIN],
                &mut best_fcb_state[SMPL_CELP_IDX_MAIN],
                &mut nrg_thr[SMPL_CELP_IDX_MAIN],
                wnrg_per_pulse[SMPL_CELP_IDX_MAIN],
            );
        }

        for r in SMPL_CELP_IDX_FEC..SMPL_CELP_IDX_MAIN + 1 {
            for i in 0..best_fcb[r].n_pulses as usize {
                pulses[r][i] = if best_fcb_state[r].pulse_signs[i] > 0.0 {
                    1 + best_fcb_state[r].pulse_positions[i] as i16
                } else {
                    -(1 + best_fcb_state[r].pulse_positions[i] as i16)
                };
            }
            pulses[r][best_fcb[r].n_pulses as usize] = if best_fcb[r].sign_new > 0.0 {
                1 + best_fcb[r].pos_new as i16
            } else {
                -(1 + best_fcb[r].pos_new as i16)
            };

            if best_fcb[r].wnrg > 0.0 {
                wnrg[r] = best_fcb[r].wnrg;
                let pn = best_fcb[r].pos_new as usize;
                gain_from_search[r] = best_fcb_state[r].num[pn] / best_fcb_state[r].den[pn];
                fcb_wnrg[r] = best_fcb_state[r].den[pn];
                n_pulses[r] = best_fcb[r].n_pulses as i16 + 1;
            } else {
                wnrg[r] = 0.0;
                gain_from_search[r] = 0.0;
                fcb_wnrg[r] = 0.0;
                n_pulses[r] = 0;
            }
        }
    }

    // check_if_better_deldec where `fcb` is sc.fcbs[fcbs_idx]; copies the read-side state.
    fn check_if_better_deldec(
        &self,
        sc: &FcbSearchScratch,
        fcbs_idx: usize,
        best_fcb: &mut Fcb,
        best_fcb_state: &mut FcbState,
        nrg_thr: &mut f32,
        wnrg_per_pulse: f32,
    ) {
        *nrg_thr += wnrg_per_pulse;
        let fcb = &sc.fcbs[fcbs_idx];
        if fcb.wnrg > *nrg_thr {
            *nrg_thr = fcb.wnrg;
            *best_fcb = fcb.clone();
            best_fcb_state.clone_from(&sc.fcb_states[sc.read_idx][fcb.fcb_state_idx]);
        }
    }

    // Variant where `fcb` is a candidate in sc.fcb_candidates.
    fn check_if_better_deldec_cand(
        &self,
        sc: &FcbSearchScratch,
        cand_idx: usize,
        best_fcb: &mut Fcb,
        best_fcb_state: &mut FcbState,
        nrg_thr: &mut f32,
        wnrg_per_pulse: f32,
    ) {
        *nrg_thr += wnrg_per_pulse;
        let fcb = &sc.fcb_candidates[cand_idx];
        if fcb.wnrg > *nrg_thr {
            *nrg_thr = fcb.wnrg;
            *best_fcb = fcb.clone();
            best_fcb_state.clone_from(&sc.fcb_states[sc.read_idx][fcb.fcb_state_idx]);
        }
    }
}

// Tier 5: gain quantization.

#[inline]
fn smpl_wnrg2(c: &[f32], x: &[f32]) -> f32 {
    x[0] * (c[0] * x[0] + c[1] * x[1]) + x[1] * (c[2] * x[0] + c[3] * x[1])
}

#[inline]
fn smpl_wnrg3(c: &[f32], x: &[f32]) -> f32 {
    x[0] * (c[0] * x[0] + c[1] * x[1] + c[2] * x[2])
        + x[1] * (c[3] * x[0] + c[4] * x[1] + c[5] * x[2])
        + x[2] * (c[6] * x[0] + c[7] * x[1] + c[8] * x[2])
}

#[inline]
fn quant_gain_uv(gain_from_search: f32) -> i16 {
    let mut gain_db = 20.0 * (gain_from_search + 1.0e-16).log10();
    gain_db = gain_db.max(SMPL_UV_GAIN_MIN_DB).min(SMPL_UV_GAIN_MAX_DB);
    ((gain_db - SMPL_UV_GAIN_MIN_DB) / SMPL_UV_GAIN_STEP_DB).round() as i16
}

fn fcb_synthesize(fcb_subfrlen: usize, pulses: &[i16], n_pulses: usize, fcb: &mut [f32]) {
    for v in fcb.iter_mut().take(fcb_subfrlen) {
        *v = 0.0;
    }
    for n in 0..n_pulses {
        // sign = 1 + 2*(p>>15); pos = p*sign - 1 (arithmetic shift on the signed pulse).
        let sign = 1i32 + 2 * ((pulses[n] as i32) >> 15);
        let pos = (pulses[n] as i32 * sign) - 1;
        fcb[pos as usize] += sign as f32;
    }
}

impl CelpEncoder {
    // calc_acb_gain (returns best acbg idx; fills acbg_params and d_ltp).
    fn calc_acb_gain(
        &self,
        l_resp: usize,
        acb_basis: &[f32],
        d_lpc: &[f32],
        acbg: &mut AcbgParams,
        d_ltp: &mut [f32],
    ) -> i32 {
        let tbl = celp_tables();
        let fcb_subfrlen = self.fcb_subfrlen;

        for m in 0..SMPL_ACBG_M {
            // acb_basis_phi[m] = symtoepl2(PhiFlip + MAX_SF_LEN - L_resp + 1, ...)
            let c_off = SMPL_MAX_SF_LEN - l_resp + 1;
            let mut tmp = vec![0.0f32; fcb_subfrlen];
            smpl_mult_symtoepl2(
                &self.phi_flip[c_off..],
                l_resp,
                &acb_basis[m * fcb_subfrlen..],
                &mut tmp,
                fcb_subfrlen,
            );
            acbg.acb_basis_phi[m * fcb_subfrlen..m * fcb_subfrlen + fcb_subfrlen]
                .copy_from_slice(&tmp);
            for i in 0..SMPL_ACBG_M {
                acbg.phi_acb[m * SMPL_ACBG_M + i] = smpl_dot_prod(
                    &acb_basis[i * fcb_subfrlen..],
                    &acbg.acb_basis_phi[m * fcb_subfrlen..],
                    fcb_subfrlen,
                );
            }
            acbg.d_acb_lpc[m] = smpl_dot_prod(&acb_basis[m * fcb_subfrlen..], d_lpc, fcb_subfrlen);
        }

        let mut best_rd = 1e30f32;
        let mut best_acbg_idx = 0i32;
        let transition_idx = if self.prev_acb_idx[SMPL_CELP_IDX_MAIN] == -1 {
            0
        } else {
            self.prev_acb_idx[SMPL_CELP_IDX_MAIN] + 1
        };
        let acbg_inv_prob_full: &[f32] = if self.low_rate {
            &tbl.acbg_inv_prob_lr
        } else {
            &tbl.acbg_inv_prob_hr
        };
        let acbg_inv_prob = &acbg_inv_prob_full[transition_idx as usize * SMPL_ACBG_N..];
        let cb: &[i16] = if self.low_rate {
            &SMPL_CB_ACBGAINS_LR_Q14
        } else {
            &SMPL_CB_ACBGAINS_HR_Q14
        };
        let sc_q14 = 1.0f32 / ((1i32) << 14) as f32;
        let mut acb_gains = [0.0f32; SMPL_ACBG_M];
        for n in 0..SMPL_ACBG_N {
            for m in 0..SMPL_ACBG_M {
                acb_gains[m] = cb[n * SMPL_ACBG_M + m] as f32 * sc_q14;
            }
            let werr_out = acbg.werr_in + smpl_wnrg2(&acbg.phi_acb, &acb_gains)
                - 2.0 * (acbg.d_acb_lpc[0] * acb_gains[0] + acbg.d_acb_lpc[1] * acb_gains[1]);
            let rd = werr_out * acbg_inv_prob[n];
            if rd < best_rd {
                best_rd = rd;
                best_acbg_idx = n as i32;
            }
        }

        // fcb target signal
        let g0 = -cb[best_acbg_idx as usize * SMPL_ACBG_M] as f32 * sc_q14;
        smpl_add_scale_vec(d_lpc, &acbg.acb_basis_phi, d_ltp, fcb_subfrlen, g0);
        let g1 = -cb[best_acbg_idx as usize * SMPL_ACBG_M + 1] as f32 * sc_q14;
        smpl_add_scale_vec_inplace(&acbg.acb_basis_phi[fcb_subfrlen..], d_ltp, fcb_subfrlen, g1);

        best_acbg_idx
    }

    // calc_gains_v (g_acb_rd_mu > 0 always -> RD branch). Returns the chosen voiced fcb gain.
    fn calc_gains_v(
        &self,
        fcb_wnrg: f32,
        gain_from_search: f32,
        exc_fcb: &[f32],
        d_lpc: &[f32],
        acbg: &AcbgParams,
        rate_idx: usize,
        acb_idx: &mut [i16; SMPL_CELP_MAX_RATES],
        fcb_idx: &mut [i16; SMPL_CELP_MAX_RATES],
    ) -> f32 {
        let tbl = celp_tables();
        let fcb_subfrlen = self.fcb_subfrlen;

        let fcbgain = gain_from_search.max(0.0);
        let mut gain_db = 20.0 * (fcbgain + 1.0e-16).log10();
        gain_db = gain_db.max(SMPL_V_GAIN_MIN_DB).min(SMPL_V_GAIN_MAX_DB);
        let max_gain_idx =
            ((SMPL_V_GAIN_MAX_DB - SMPL_V_GAIN_MIN_DB) / SMPL_V_GAIN_STEP_DB).round() as i32;

        // g_acb_rd_mu > 0 -> always RD branch
        let mut best_acbg_idx = 0i32;
        let mut best_fcbg_idx = 0i32;

        let mut acb_fcb = [0.0f32; SMPL_ACBG_M];
        for i in 0..SMPL_ACBG_M {
            acb_fcb[i] = smpl_dot_prod(
                &acbg.acb_basis_phi[i * fcb_subfrlen..],
                exc_fcb,
                fcb_subfrlen,
            );
        }
        // Phi_all is (M+1)x(M+1) row-major.
        let mut phi_all = [0.0f32; (SMPL_ACBG_M + 1) * (SMPL_ACBG_M + 1)];
        let stride = SMPL_ACBG_M + 1;
        for i in 0..SMPL_ACBG_M {
            for j in 0..SMPL_ACBG_M {
                phi_all[i * stride + j] = acbg.phi_acb[i * SMPL_ACBG_M + j];
            }
        }
        for i in 0..SMPL_ACBG_M {
            phi_all[i * stride + SMPL_ACBG_M] = acb_fcb[i];
            phi_all[SMPL_ACBG_M * stride + i] = acb_fcb[i];
        }
        phi_all[SMPL_ACBG_M * stride + SMPL_ACBG_M] = fcb_wnrg;

        let mut dall = [0.0f32; SMPL_ACBG_M + 1];
        dall[..SMPL_ACBG_M].copy_from_slice(&acbg.d_acb_lpc);
        dall[SMPL_ACBG_M] = smpl_dot_prod(d_lpc, exc_fcb, fcb_subfrlen);

        let mut gain_idxs = [0i32; N_GAIN_STEPS];
        let mut fcbgains = [0.0f32; N_GAIN_STEPS];
        let mut fcbg_inv_prob = [0.0f32; N_GAIN_STEPS];
        let mut first_gain_idx = (((gain_db - SMPL_V_GAIN_MIN_DB) / SMPL_V_GAIN_STEP_DB).floor()
            as i32
            - (N_GAIN_STEPS as i32 - 1) / 2)
            .max(0);
        first_gain_idx = first_gain_idx.min(max_gain_idx - 1);
        let offset =
            ((SMPL_V_GAIN_MIN_DB - SMPL_V_GAIN_MAX_DB) / SMPL_V_GAIN_STEP_DB).floor() as i32;
        for i in 0..N_GAIN_STEPS {
            gain_idxs[i] = first_gain_idx + i as i32;
            fcbgains[i] = tbl.fcbgains_v[gain_idxs[i] as usize];
            if self.prev_fcb_idx[rate_idx] == -1 {
                fcbg_inv_prob[i] = tbl.fcbg_v_inv_prob[gain_idxs[i] as usize];
            } else {
                let delta = self.prev_fcb_idx[rate_idx] - gain_idxs[i];
                let cmf_idx = delta - offset;
                fcbg_inv_prob[i] = tbl.fcbg_v_delta_inv_prob[cmf_idx as usize];
            }
        }

        let mut best_rd = 1e30f32;
        let transition_idx = if self.prev_acb_idx[rate_idx] == -1 {
            0
        } else {
            self.prev_acb_idx[rate_idx] + 1
        };
        let cb: &[i16] = if self.low_rate {
            &SMPL_CB_ACBGAINS_LR_Q14
        } else {
            &SMPL_CB_ACBGAINS_HR_Q14
        };
        let acbg_inv_prob_full: &[f32] = if self.low_rate {
            &tbl.acbg_inv_prob_lr
        } else {
            &tbl.acbg_inv_prob_hr
        };
        let acbg_inv_prob = &acbg_inv_prob_full[transition_idx as usize * SMPL_ACBG_N..];
        let sc_q14 = 1.0f32 / ((1i32) << 14) as f32;
        for n in 0..SMPL_ACBG_N {
            let mut gains = [0.0f32; SMPL_ACBG_M + 1];
            for m in 0..SMPL_ACBG_M {
                gains[m] = cb[n * SMPL_ACBG_M + m] as f32 * sc_q14;
            }
            for i in 0..N_GAIN_STEPS {
                gains[SMPL_ACBG_M] = fcbgains[i];
                let werr_out = acbg.werr_in + smpl_wnrg3(&phi_all, &gains)
                    - 2.0 * (dall[0] * gains[0] + dall[1] * gains[1] + dall[2] * gains[2]);
                let rd = werr_out * fcbg_inv_prob[i] * acbg_inv_prob[n];
                if rd < best_rd {
                    best_rd = rd;
                    best_acbg_idx = n as i32;
                    best_fcbg_idx = gain_idxs[i];
                }
            }
        }
        acb_idx[rate_idx] = best_acbg_idx as i16;
        fcb_idx[rate_idx] = best_fcbg_idx as i16;

        fcb_idx[rate_idx] = fcb_idx[rate_idx].max(0).min(max_gain_idx as i16);
        tbl.fcbgains_v[fcb_idx[rate_idx] as usize]
    }
}

// Tier 6: main per-subframe encoder.

impl CelpEncoder {
    pub(crate) fn encode_subframe(
        &mut self,
        res_lpc: &[f32],
        predcoef: &[f32; 17],
        perc_wght_resp: &[f32],
        lags: &[f32],
        subfr_importance: [f32; SMPL_CELP_MAX_RATES],
        fcb_pulses_max: [i16; SMPL_CELP_MAX_RATES],
        surv: &[i16],
    ) -> CelpSubframeOut {
        let l_resp = self.perc_resp_len;
        let fcb_subfrlen = self.fcb_subfrlen;
        let voiced = lags[1] > 0.0;

        // imp_lpc = filt_ar16(perc_wght_resp, L_resp, predcoef) * hanning_win.
        // The 16-sample AR lead (imp_lpc_buf[0..SMPL_LPC_ORDER]) is zeroed once at construction and
        // never written by the filter (it only writes indices >= SMPL_LPC_ORDER), so it stays zero
        // across calls, matching the reference's one-time zero of imp_lpc_.
        smpl_filt_ar16(
            perc_wght_resp,
            l_resp,
            predcoef,
            SMPL_LPC_ORDER,
            &mut self.imp_lpc_buf,
        );
        {
            let imp = &mut self.imp_lpc_buf[SMPL_LPC_ORDER..];
            smpl_mul_vec_inplace(&self.hanning_win, imp, l_resp);
        }

        // imp_lpc_rev: buffer of 2*MAX_L_RESP-1, logical ptr at MAX_L_RESP-1.
        let mut imp_lpc_rev =
            SubframeScratch::zeroed(&mut self.sf.imp_lpc_rev, 2 * SMPL_MAX_L_RESP - 1);
        let rev_base = SMPL_MAX_L_RESP - 1;
        {
            let imp = &self.imp_lpc_buf[SMPL_LPC_ORDER..];
            // reverse_into(imp, &imp_lpc_rev[rev_base..], L_resp)
            for i in 0..l_resp {
                imp_lpc_rev[rev_base + i] = imp[l_resp - i - 1];
            }
        }
        // Zero the (L_resp-1) lead before rev_base; already zero, kept explicit for clarity.
        for i in 0..l_resp - 1 {
            imp_lpc_rev[rev_base - (l_resp - 1) + i] = 0.0;
        }
        // Phi = perc_filt_ma(imp_lpc_rev, L_resp, imp_lpc, L_resp)
        {
            let mut phi = vec![0.0f32; SMPL_MAX_SF_LEN];
            // Pass the (finalized) impulse-response slice straight in -- it was a defensive copy.
            self.perc_filt_ma(
                &imp_lpc_rev,
                rev_base,
                l_resp,
                &self.imp_lpc_buf[SMPL_LPC_ORDER..SMPL_LPC_ORDER + l_resp],
                l_resp,
                &mut phi,
            );
            smpl_reverse(&mut phi, l_resp);
            for v in phi.iter_mut().take(fcb_subfrlen).skip(l_resp) {
                *v = 0.0;
            }
            self.phi.copy_from_slice(&phi);
        }
        // PhiFlip
        for v in self.phi_flip.iter_mut() {
            *v = 0.0;
        }
        self.phi_flip[SMPL_MAX_SF_LEN] = self.phi[0];
        for i in 0..l_resp + 1 {
            self.phi_flip[SMPL_MAX_SF_LEN - i] = self.phi[i];
            self.phi_flip[SMPL_MAX_SF_LEN + i] = self.phi[i];
        }

        // d_lpc = symtoepl2(PhiFlip + MAX_SF_LEN - L_resp + 1, L_resp, res_lpc, fcb_subfrlen)
        // res_lpc must be readable up to fcb_subfrlen + L_resp (zero padded). Build a padded copy.
        let mut res_lpc_pad =
            SubframeScratch::zeroed(&mut self.sf.res_lpc_pad, fcb_subfrlen + l_resp + 1);
        res_lpc_pad[..fcb_subfrlen].copy_from_slice(&res_lpc[..fcb_subfrlen]);
        let mut d_lpc = SubframeScratch::zeroed(&mut self.sf.d_lpc, SMPL_MAX_SF_LEN);
        {
            let c_off = SMPL_MAX_SF_LEN - l_resp + 1;
            smpl_mult_symtoepl2(
                &self.phi_flip[c_off..],
                l_resp,
                &res_lpc_pad,
                &mut d_lpc,
                fcb_subfrlen,
            );
        }

        let mut acbg = AcbgParams {
            werr_in: 0.0,
            phi_acb: [0.0; SMPL_ACBG_M * SMPL_ACBG_M],
            d_acb_lpc: [0.0; SMPL_ACBG_M],
            acb_basis_phi: vec![0.0; SMPL_ACBG_M * fcb_subfrlen],
        };
        let mut zir_lpc = SubframeScratch::zeroed(&mut self.sf.zir_lpc, SMPL_MAX_SF_LEN);

        if !self.ignore_zir {
            // zir_lpc_tmp buffer: MAX_SF_LEN + MAX_L_RESP - 1, logical ptr at MAX_L_RESP-1
            let mut zir_tmp = vec![0.0f32; SMPL_MAX_SF_LEN + SMPL_MAX_L_RESP - 1];
            let zt = SMPL_MAX_L_RESP - 1;
            // Ht_zir buffer: 2*MAX_L_RESP-1, ptr at MAX_L_RESP-1
            let mut ht_zir = vec![0.0f32; 2 * SMPL_MAX_L_RESP - 1];
            let ht = SMPL_MAX_L_RESP - 1;

            // zir_lpc_tmp[0..L_resp]=0 (already)
            let state_len = SMPL_LPC_ORDER.max(l_resp - 1);
            // copy state_wght[fcb_subfrlen - state_len .. fcb_subfrlen] into the state_len samples
            // before zir_lpc_tmp[0]. state_wght logical start at state_wght_buf[SMPL_LPC_ORDER].
            for i in 0..state_len {
                zir_tmp[zt - state_len + i] =
                    self.state_wght_buf[SMPL_LPC_ORDER + (fcb_subfrlen - state_len) + i];
            }
            // In-place AR over zir_tmp at base zt (x==y). The input term x[n] for n in 0..L_resp is
            // the zeroed region, read before being overwritten, while y[n-16..n-1] supply the
            // just-copied history; writing y[n] after reading x[n] keeps the aliasing safe.
            {
                for nn in 0..l_resp {
                    let mut res = zir_tmp[zt + nn]; // x[nn] (currently 0 in [0..L_resp])
                    for i in 0..16 {
                        res -= predcoef[16 - i] * zir_tmp[zt + nn - 16 + i];
                    }
                    zir_tmp[zt + nn] = res;
                }
            }
            // zir_lpc = perc_filt_ma(zir_lpc_tmp, L_resp, perc_wght_resp, L_resp)
            self.perc_filt_ma(&zir_tmp, zt, l_resp, perc_wght_resp, l_resp, &mut zir_lpc);

            // reverse_into(zir_lpc, zir_lpc_tmp, L_resp)
            for i in 0..l_resp {
                zir_tmp[zt + i] = zir_lpc[l_resp - i - 1];
            }
            for i in 0..l_resp - 1 {
                zir_tmp[zt - (l_resp - 1) + i] = 0.0;
            }
            // Ht_zir = perc_filt_ma(zir_lpc_tmp, L_resp, imp_lpc, L_resp)
            {
                self.perc_filt_ma(
                    &zir_tmp,
                    zt,
                    l_resp,
                    &self.imp_lpc_buf[SMPL_LPC_ORDER..SMPL_LPC_ORDER + l_resp],
                    l_resp,
                    &mut ht_zir[ht..],
                );
            }
            smpl_reverse(&mut ht_zir[ht..], l_resp);

            acbg.werr_in = if voiced {
                smpl_dot_prod(&d_lpc, res_lpc, fcb_subfrlen)
                    + 2.0 * smpl_dot_prod(&ht_zir[ht..], res_lpc, l_resp)
                    + smpl_nrg(&zir_lpc, l_resp)
            } else {
                0.0
            };
            // d_lpc[0..L_resp] += Ht_zir
            for i in 0..l_resp {
                d_lpc[i] += ht_zir[ht + i];
            }
        } else {
            for v in zir_lpc.iter_mut().take(l_resp) {
                *v = 0.0;
            }
            acbg.werr_in = if voiced {
                smpl_dot_prod(&d_lpc, res_lpc, fcb_subfrlen)
            } else {
                0.0
            };
        }

        // ACB
        let mut acb_basis =
            SubframeScratch::zeroed(&mut self.sf.acb_basis, SMPL_MAX_SF_LEN * SMPL_ACBG_M);
        let mut acb = SubframeScratch::zeroed(&mut self.sf.acb, SMPL_MAX_SF_LEN);
        let mut d_ltp = SubframeScratch::zeroed(&mut self.sf.d_ltp, SMPL_MAX_SF_LEN);
        let mut acb_idx = [-1i16; SMPL_CELP_MAX_RATES];

        if voiced {
            smpl_syn_ltp_basis(
                lags,
                fcb_subfrlen / SMPL_LAG_SUBFRLEN,
                &mut self.acb_state,
                self.acb_state_len,
                &mut acb_basis,
            );
            let idx = self.calc_acb_gain(l_resp, &acb_basis, &d_lpc, &mut acbg, &mut d_ltp);
            acb_idx[SMPL_CELP_IDX_MAIN] = idx as i16;
            let mut acb_gain = [0.0f32; SMPL_ACBG_M];
            acb_dequant(
                self.low_rate,
                acb_idx[SMPL_CELP_IDX_MAIN] as i32,
                &mut acb_gain,
            );
            acb_synthesize(fcb_subfrlen, &acb_basis, &acb_gain, &mut acb);
            acb_idx[SMPL_CELP_IDX_FEC] = acb_idx[SMPL_CELP_IDX_MAIN];
        }

        // Weighted target + thresholds.
        // wtgt_tmp buffer: MAX_SF_LEN + 2*MAX_L_RESP - 1, ptr at MAX_L_RESP-1
        let mut wtgt_tmp = SubframeScratch::zeroed(
            &mut self.sf.wtgt_tmp,
            SMPL_MAX_SF_LEN + 2 * SMPL_MAX_L_RESP - 1,
        );
        let wt = SMPL_MAX_L_RESP - 1;
        let mut wtgt =
            SubframeScratch::zeroed(&mut self.sf.wtgt, SMPL_MAX_SF_LEN + SMPL_MAX_L_RESP);
        wtgt_tmp[wt..wt + fcb_subfrlen].copy_from_slice(&res_lpc[..fcb_subfrlen]);
        if voiced {
            // wtgt_tmp += -RATE_ACB_SCALE * acb
            for i in 0..fcb_subfrlen {
                wtgt_tmp[wt + i] += -SMPL_RATE_ACB_SCALE * acb[i];
            }
        }
        for i in 0..l_resp {
            wtgt_tmp[wt + fcb_subfrlen + i] = 0.0;
        }
        for i in 0..l_resp - 1 {
            wtgt_tmp[wt - (l_resp - 1) + i] = 0.0;
        }
        {
            self.perc_filt_ma(
                &wtgt_tmp,
                wt,
                fcb_subfrlen + l_resp,
                &self.imp_lpc_buf[SMPL_LPC_ORDER..SMPL_LPC_ORDER + l_resp],
                l_resp,
                &mut wtgt,
            );
        }
        for i in 0..l_resp {
            wtgt[i] += zir_lpc[i];
        }
        let nrg_wtgt = smpl_nrg(&wtgt, fcb_subfrlen + l_resp);
        let mut wnrg_per_pulse = [0.0f32; SMPL_CELP_MAX_RATES];
        for r in 0..SMPL_CELP_MAX_RATES {
            wnrg_per_pulse[r] = nrg_wtgt / (subfr_importance[r] + 1.0e-3);
        }
        let i_lag = lags[(fcb_subfrlen / SMPL_LAG_SUBFRLEN) - 1] as i32;

        let mut n_pulses = [0i16; SMPL_CELP_MAX_RATES];
        let mut gain_from_search = [0.0f32; SMPL_CELP_MAX_RATES];
        let mut fcb_wnrg = [0.0f32; SMPL_CELP_MAX_RATES];
        let mut wnrg = [0.0f32; SMPL_CELP_MAX_RATES];
        let mut pulses = [[0i16; SMPL_MAX_PULSES_PER_SF]; SMPL_CELP_MAX_RATES];

        if fcb_pulses_max[SMPL_CELP_IDX_MAIN] > 0 {
            let target: &[f32] = if voiced { &d_ltp } else { &d_lpc };
            let use_greedy = fcb_pulses_max[SMPL_CELP_IDX_MAIN] - 1 > 0
                && surv[(fcb_pulses_max[SMPL_CELP_IDX_MAIN] - 2) as usize] == 1
                && !self.low_rate;
            if use_greedy {
                self.smpl_fcb_search(
                    target,
                    &wnrg_per_pulse,
                    &fcb_pulses_max,
                    &mut pulses,
                    &mut n_pulses,
                    &mut wnrg,
                    &mut gain_from_search,
                    &mut fcb_wnrg,
                );
            } else {
                let ps = SMPL_PITCH_SHARPENING_COEF * if self.low_rate { 1.0 } else { 0.0 };
                // Borrow the pooled beam scratch out for the search (leaving the empty `Default`
                // sentinel), then put it back -- so `deldec` keeps `&self` while owning a `&mut sc`.
                let mut sc = std::mem::take(&mut self.fcb_search);
                self.smpl_fcb_search_deldec(
                    &mut sc,
                    target,
                    ps,
                    i_lag,
                    &wnrg_per_pulse,
                    &fcb_pulses_max,
                    surv,
                    &mut pulses,
                    &mut n_pulses,
                    &mut wnrg,
                    &mut gain_from_search,
                    &mut fcb_wnrg,
                );
                self.fcb_search = sc;
            }
        }

        // Per-rate gain quant + exc_fcb. fcbgain carries the last assigned (MAIN) value.
        let mut gain_idx = [-1i16; SMPL_CELP_MAX_RATES];
        let mut fcbgain = 0.0f32;
        let mut exc_fcb = SubframeScratch::zeroed(&mut self.sf.exc_fcb, SMPL_MAX_SF_LEN);
        let tbl = celp_tables();
        for r in 0..SMPL_CELP_MAX_RATES {
            let mut exc_fcb_raw = vec![0.0f32; SMPL_MAX_SF_LEN];
            fcb_synthesize(
                fcb_subfrlen,
                &pulses[r],
                n_pulses[r] as usize,
                &mut exc_fcb_raw,
            );
            exc_fcb[..fcb_subfrlen].copy_from_slice(&exc_fcb_raw[..fcb_subfrlen]);
            if n_pulses[r] > 0 {
                if voiced {
                    if self.low_rate {
                        smpl_pitch_sharp(&mut exc_fcb, i_lag as usize, fcb_subfrlen);
                    }
                    fcbgain = self.calc_gains_v(
                        fcb_wnrg[r],
                        gain_from_search[r],
                        &exc_fcb,
                        &d_lpc,
                        &acbg,
                        r,
                        &mut acb_idx,
                        &mut gain_idx,
                    );
                } else {
                    gain_idx[r] = quant_gain_uv(gain_from_search[r]);
                    fcbgain = tbl.fcbgains_uv[gain_idx[r] as usize];
                }
                smpl_scale_vec_inplace(&mut exc_fcb, fcb_subfrlen, fcbgain);
            }
        }

        // exc_lpc = exc_fcb (last r = MAIN); for voiced add the ACB contribution. The reference's
        // trailing `smpl_sub_vec_inplace(acb, res_ltp)` mutates only the throwaway plotting buffer
        // `res_ltp`, NOT exc_lpc, so the excitation fed back into the ACB state is exactly
        // exc_fcb + acb.
        let mut exc_lpc = vec![0.0f32; fcb_subfrlen];
        exc_lpc.copy_from_slice(&exc_fcb[..fcb_subfrlen]);
        if voiced {
            let mut acb_gain = [0.0f32; SMPL_ACBG_M];
            acb_dequant(
                self.low_rate,
                acb_idx[SMPL_CELP_IDX_MAIN] as i32,
                &mut acb_gain,
            );
            acb_synthesize(fcb_subfrlen, &acb_basis, &acb_gain, &mut acb);
            smpl_add_vec_inplace(&acb, &mut exc_lpc, fcb_subfrlen);
        }

        // Update adaptive codebook state: shift down by fcb_subfrlen (keeping acb_state_len -
        // 2*fcb_subfrlen elements) and write exc_lpc at acb_state_len - 2*fcb_subfrlen, leaving the
        // trailing fcb_subfrlen untouched (an in-place shift, not an append-at-end).
        self.acb_state
            .copy_within(fcb_subfrlen..self.acb_state_len - fcb_subfrlen, 0);
        let write_off = self.acb_state_len - 2 * fcb_subfrlen;
        self.acb_state[write_off..write_off + fcb_subfrlen]
            .copy_from_slice(&exc_lpc[..fcb_subfrlen]);

        // Update ZIR state.
        if !self.ignore_zir {
            let mut lpc_res_err = vec![0.0f32; SMPL_MAX_SF_LEN];
            smpl_sub_vec(res_lpc, &exc_lpc, &mut lpc_res_err, fcb_subfrlen);
            // state_wght[-16..0] = state_err_lpc_syn  (i.e. state_wght_buf[0..16])
            for i in 0..SMPL_LPC_ORDER {
                self.state_wght_buf[i] = self.state_err_lpc_syn[i];
            }
            // filt_ar16(lpc_res_err, fcb_subfrlen, predcoef, state_wght) with y_base=SMPL_LPC_ORDER
            smpl_filt_ar16(
                &lpc_res_err,
                fcb_subfrlen,
                predcoef,
                SMPL_LPC_ORDER,
                &mut self.state_wght_buf,
            );
            // state_err_lpc_syn = state_wght[fcb_subfrlen-16 .. fcb_subfrlen]
            for i in 0..SMPL_LPC_ORDER {
                self.state_err_lpc_syn[i] =
                    self.state_wght_buf[SMPL_LPC_ORDER + (fcb_subfrlen - SMPL_LPC_ORDER) + i];
            }
        }

        // Packet-boundary / prev-idx state.
        self.subfr_cnt += 1;
        if self.subfr_cnt == self.subfr_per_packet {
            for r in 0..SMPL_CELP_MAX_RATES {
                self.prev_acb_idx[r] = -1;
                self.prev_fcb_idx[r] = -1;
            }
            self.subfr_cnt = 0;
        } else {
            for r in 0..SMPL_CELP_MAX_RATES {
                self.prev_acb_idx[r] = if voiced { acb_idx[r] as i32 } else { -1 };
                self.prev_fcb_idx[r] = if voiced { gain_idx[r] as i32 } else { -1 };
            }
        }
        self.fcbgain = fcbgain;

        // Materialize pulses Vecs trimmed to n_pulses.
        let pulses_fec: Vec<i16> =
            pulses[SMPL_CELP_IDX_FEC][..n_pulses[SMPL_CELP_IDX_FEC].max(0) as usize].to_vec();
        let pulses_main: Vec<i16> =
            pulses[SMPL_CELP_IDX_MAIN][..n_pulses[SMPL_CELP_IDX_MAIN].max(0) as usize].to_vec();

        // Return the pooled subframe buffers for reuse next call.
        self.sf.imp_lpc_rev = imp_lpc_rev;
        self.sf.res_lpc_pad = res_lpc_pad;
        self.sf.d_lpc = d_lpc;
        self.sf.zir_lpc = zir_lpc;
        self.sf.acb_basis = acb_basis;
        self.sf.acb = acb;
        self.sf.d_ltp = d_ltp;
        self.sf.wtgt_tmp = wtgt_tmp;
        self.sf.wtgt = wtgt;
        self.sf.exc_fcb = exc_fcb;

        CelpSubframeOut {
            pulses: [pulses_fec, pulses_main],
            n_pulses,
            acb_idx,
            gain_idx,
            exc_lpc,
        }
    }
}

// Tier 7: survivor distribution.

pub(crate) fn smpl_distribute_fcb_surv(numsurv: &mut [i16], max_pulses: i32, tot_surv: i32) {
    debug_assert!(max_pulses <= 256);
    if max_pulses <= 1 {
        numsurv[0] = 1;
        return;
    }
    for i in 0..max_pulses as usize {
        numsurv[i] = 1;
    }
    let mut sum_surv = max_pulses;
    let extra_surv = tot_surv - max_pulses;
    let extra = (extra_surv / (max_pulses - 1)).min(SMPL_FCB_SRV_MAX - 1);
    for i in 0..(max_pulses - 1) as usize {
        numsurv[i] += extra as i16;
    }
    sum_surv += extra * (max_pulses - 1);
    let mut ix = max_pulses - 2;
    while sum_surv < tot_surv {
        if (numsurv[ix as usize] as i32) < SMPL_FCB_SRV_MAX {
            numsurv[ix as usize] += 1;
            sum_surv += 1;
        }
        ix -= 1;
        if ix < 0 {
            break;
        }
    }
}

/// The high-rate ACB gain codebook (`smpl_cb_acbgains_hr_Q14`), for the decoder ACB synthesis.
pub(crate) fn cb_acbgains_hr_q14() -> &'static [i16] {
    &SMPL_CB_ACBGAINS_HR_Q14
}

/// The low-rate ACB gain codebook (`smpl_cb_acbgains_lr_Q14`).
pub(crate) fn cb_acbgains_lr_q14() -> &'static [i16] {
    &SMPL_CB_ACBGAINS_LR_Q14
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tables_build_and_have_expected_shapes() {
        let t = celp_tables();
        // fcbgains_v[0] = 10^(0.05 * -100) and step 3dB.
        assert!((t.fcbgains_v[0] - 10f32.powf(0.05 * -100.0)).abs() < 1e-9);
        assert!((t.fcbgains_v[33] - 10f32.powf(0.05 * (33.0 * 3.0 - 100.0))).abs() < 1e-6);
        // fcbgains_uv spans -90..0 dB in 1dB steps; index 90 == 0 dB == 1.0.
        assert!((t.fcbgains_uv[90] - 1.0).abs() < 1e-6);
        assert!((t.fcbgains_uv[0] - 10f32.powf(0.05 * -90.0)).abs() < 1e-9);
        // inv-prob tables are positive (2^bits).
        assert!(t.acbg_inv_prob_lr.iter().all(|&v| v > 0.0));
        assert!(t.fcbg_v_inv_prob.iter().all(|&v| v > 0.0));
    }

    #[test]
    fn dcmf_to_cmf_is_integer_exact() {
        // Reproduce the first acbgains_lr row through the integer path; the result must be a strictly
        // increasing u16 sequence starting at 0 (cumulative).
        let mut cmf = [0u16; SMPL_ACBG_N + 1];
        smpl_dcmf_to_cmf(&SMPL_ACBGAINS_DCMF_LR[..SMPL_ACBG_N], SMPL_ACBG_N, &mut cmf);
        assert_eq!(cmf[0], 0);
        for w in cmf.windows(2) {
            assert!(w[1] > w[0]);
        }
    }

    #[test]
    fn encode_unvoiced_runs() {
        let perc_resp_len = 32usize;
        let fcb_subfrlen = 80usize;
        let mut enc = CelpEncoder::new(false, perc_resp_len, fcb_subfrlen, 4);
        let res_lpc: Vec<f32> = (0..fcb_subfrlen)
            .map(|i| ((i as f32 * 0.3).sin()) * 0.1)
            .collect();
        let mut predcoef = [0.0f32; 17];
        predcoef[0] = 1.0;
        predcoef[1] = -0.5;
        let perc_wght_resp: Vec<f32> = (0..perc_resp_len)
            .map(|i| if i == 0 { 1.0 } else { 0.0 })
            .collect();
        // Unvoiced: lags[1] <= 0.
        let lags = [0.0f32, 0.0, 0.0];
        let surv = [1i16; SMPL_MAX_PULSES_PER_SF];
        let out = enc.encode_subframe(
            &res_lpc,
            &predcoef,
            &perc_wght_resp,
            &lags,
            [1.0, 1.0],
            [8, 8],
            &surv,
        );
        assert_eq!(out.acb_idx[SMPL_CELP_IDX_MAIN], -1);
        assert_eq!(out.exc_lpc.len(), fcb_subfrlen);
        assert!(out.n_pulses[SMPL_CELP_IDX_MAIN] >= 0);
    }

    #[test]
    fn encode_voiced_runs() {
        let perc_resp_len = 32usize;
        let fcb_subfrlen = 80usize;
        let mut enc = CelpEncoder::new(false, perc_resp_len, fcb_subfrlen, 4);
        // Prime acb_state with a periodic signal so the LTP basis is meaningful.
        for (i, v) in enc.acb_state.iter_mut().enumerate() {
            *v = ((i as f32) * 0.2).sin();
        }
        let res_lpc: Vec<f32> = (0..fcb_subfrlen)
            .map(|i| ((i as f32 * 0.25).sin()) * 0.2)
            .collect();
        let mut predcoef = [0.0f32; 17];
        predcoef[0] = 1.0;
        predcoef[1] = -0.4;
        let perc_wght_resp: Vec<f32> = (0..perc_resp_len)
            .map(|i| if i == 0 { 1.0 } else { 0.0 })
            .collect();
        // Voiced: integer lag 60 for both 40-sample sub-blocks (fcb_subfrlen/40 = 2).
        let lags = [60.0f32, 60.0, 60.0];
        let surv = [2i16; SMPL_MAX_PULSES_PER_SF];
        let out = enc.encode_subframe(
            &res_lpc,
            &predcoef,
            &perc_wght_resp,
            &lags,
            [1.0, 1.0],
            [6, 6],
            &surv,
        );
        assert!(out.acb_idx[SMPL_CELP_IDX_MAIN] >= 0);
        assert_eq!(out.exc_lpc.len(), fcb_subfrlen);
    }

    #[test]
    fn encode_voiced_fractional_lag_greedy_runs() {
        // High-rate (low_rate=false) + surv[max-2]==1 triggers the greedy search path; a fractional
        // lag exercises the interpolation branch of smpl_syn_ltp_basis.
        let perc_resp_len = 32usize;
        let fcb_subfrlen = 80usize;
        let mut enc = CelpEncoder::new(false, perc_resp_len, fcb_subfrlen, 4);
        for (i, v) in enc.acb_state.iter_mut().enumerate() {
            *v = ((i as f32) * 0.17).sin();
        }
        let res_lpc: Vec<f32> = (0..fcb_subfrlen)
            .map(|i| ((i as f32 * 0.25).sin()) * 0.2)
            .collect();
        let mut predcoef = [0.0f32; 17];
        predcoef[0] = 1.0;
        predcoef[1] = -0.4;
        let perc_wght_resp: Vec<f32> = (0..perc_resp_len)
            .map(|i| if i == 0 { 1.0 } else { 0.0 })
            .collect();
        let lags = [55.5f32, 55.5, 55.5]; // fractional
        let surv = [1i16; SMPL_MAX_PULSES_PER_SF];
        let out = enc.encode_subframe(
            &res_lpc,
            &predcoef,
            &perc_wght_resp,
            &lags,
            [1.0, 1.0],
            [4, 4],
            &surv,
        );
        assert!(out.acb_idx[SMPL_CELP_IDX_MAIN] >= 0);
        assert_eq!(out.exc_lpc.len(), fcb_subfrlen);
    }
}

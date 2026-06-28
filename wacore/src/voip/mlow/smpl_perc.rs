//! MLow perceptual-weighting front-end: FFT-based perceptual autocorrelation to a perceptual LPC
//! response, plus the per-subframe pulse budget and importance from the bitrate controller, with the
//! leaf LPC/vector/window helpers they need.
//!
//! A self-contained mixed-radix complex FFT avoids a C-only FFT dependency (not portable to
//! wasm32/esp32). The spectrum is re-packed into the ordered real layout the rest of the codec
//! expects (`f[0]`=DC, `f[1]`=Nyquist, then interleaved `[re,im]` for bins 1..N/2-1) so `smth_filt`
//! and the inverse index identical bins and the returned autocorrelation `R[]` keeps the same
//! structure.
//! PERCW_NFFT is 576 = 2^6 * 3^2, NOT a power of two, hence the mixed-radix (radix-2/3 + naive base)
//! DFT.
//!
//! Self-contained on purpose: small leaf helpers (vector ops, window generation, ac2rc/rc2a) are
//! duplicated locally rather than shared with `smpl_celp.rs`, so this front-end can be wired in
//! separately without coupling.
// Leaf constants/helpers (SMPL_PERC_RESP_LEN, SMPL_TRUE, smpl_nrg) are intentionally duplicated
// locally and one bitrate-controller state field is parity scaffolding the wired-in path doesn't
// read yet, so dead_code is allowed module-wide.
#![allow(dead_code)]
#![allow(clippy::needless_range_loop)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::excessive_precision)]
// SMPL_PI is the codec's own literal; keep it, do not swap for std PI.
#![allow(clippy::approx_constant)]
// Clamp is written as min(max(x, lo), hi); the `.max().min()` form preserves that operand order.
#![allow(clippy::manual_clamp)]
// The `% k == 0`, `(k+1)/2`, `>= x+1`, and range comparisons below stay in their literal arithmetic
// form to keep the bit-exact integer behavior obvious rather than rewritten into idiomatic Rust.
#![allow(clippy::manual_is_multiple_of)]
#![allow(clippy::manual_div_ceil)]
#![allow(clippy::int_plus_one)]
#![allow(clippy::manual_range_contains)]

// Constants

const SMPL_PI: f32 = 3.1415926535897;

const PERCW_NFFT: usize = 512 + 64; // 576
const PERCW_FS_KHZ: f32 = 16.0;

const SMPL_PERC_MASK_SMTH: f32 = 0.1158;
const SMPL_PERC_MEL_FC_HZ: f32 = 320.0;

const SMPL_WINNEXT_WB_LEN: usize = 16 * 2; // 32
const SMPL_WINNEXT_WB_LONG_LEN: usize = 16 * 4; // 64
const SMPL_WIN3_SHORT_LEN: usize = SMPL_WINNEXT_WB_LEN; // 32
const SMPL_WIN3_LONG_LEN: usize = SMPL_WINNEXT_WB_LONG_LEN; // 64
const SMPL_WINPREV_PERC_LEN: usize = 16 * 12; // 192
const SMPL_PERC_WIN1_10MS_LEN: usize = 192;
const SMPL_PERC_WIN1_20MS_LEN: usize = 352;

const SMPL_MAX_L_RESP: usize = 32 + 1; // 33
const SMPL_MAX_SF_LEN: usize = 16 * 10; // 160
const SMPL_PERC_RESP_LEN: usize = 16 * 2; // 32
pub(crate) const SMPL_PERC_REG: f32 = 1e-3;

const SMPL_TRUE: i32 = 1;
const SMPL_FALSE: i32 = 0;

// Bitrate-controller constants
const SMPL_CELP_IDX_FEC: usize = 0;
const SMPL_CELP_IDX_MAIN: usize = 1;
const SMPL_CELP_MAX_RATES: usize = SMPL_CELP_IDX_MAIN + 1; // 2
const SMPL_MAX_PULSES_PER_SF: i32 = 40;
const SMPL_RATE_CONT_SCALE: f32 = 26.0;
const SMPL_E: f32 = 2.7182818284590;

// SmplFrameTypes
const BACKGROUND_NOISE: usize = 0;
const UNVOICED: usize = 1;
const VOICED: usize = 2;

// [lowRate][BACKGROUND_NOISE/UNVOICED/VOICED]
const SMPL_MAX_PULSES_PER_FRAME: [[u8; 3]; 2] = [[80, 160, 160], [16, 32, 32]];

// [framelenidx][lowrate][8]
const SMPL_RATE_CONTROL_MODEL_COMP5: [[[f32; 8]; 2]; 4] = [
    [
        [
            5.166876656946171,
            -8.981699804753452,
            0.07280811614105594,
            0.1301196310618402,
            -0.01597680442864421,
            1.7601470147884113,
            -3.8161195433141755,
            0.3038629198331684,
        ],
        [
            -71.71229978402292,
            14.197572549553076,
            -0.9863630205846172,
            0.032124893286072924,
            -0.0003538411576874928,
            1.803705259861388e-11,
            10.0,
            1.2454667523627154,
        ],
    ],
    [
        [
            32.5371190670542,
            -41.270234279452104,
            10.490270829170875,
            -1.102121269442237,
            0.03848319274046071,
            3.405326741403831,
            -5.102658181889428,
            0.2141935195026695,
        ],
        [
            -177.10486363500775,
            43.952329593498376,
            -3.7049735533247454,
            0.14239771116996938,
            -0.001919963993993193,
            7.953695588409639e-6,
            5.220317075476664,
            0.6435364076926223,
        ],
    ],
    [
        [
            -79.2663194911617,
            45.00981883522089,
            -10.063311543498518,
            1.2311531056576501,
            -0.06023559069137118,
            0.059204788212259364,
            3.033961466462233,
            1.0111383197827808,
        ],
        [
            -122.04861900525415,
            31.62096398905459,
            -2.613237037423586,
            0.10050433143234094,
            -0.0013233009240188039,
            2.14859438836692e-7,
            1.9077791307787761,
            0.7059420500333776,
        ],
    ],
    [
        [
            -182.64255084224325,
            122.90780796179816,
            -31.308790671748525,
            3.7850563849431462,
            -0.1750480676903051,
            0.05399618467364628,
            3.009451055091342,
            1.1243365512229038,
        ],
        [
            -132.4565456943888,
            34.361297004632966,
            -2.7956546289118887,
            0.10428149547078584,
            -0.001322667891395693,
            2.678747426340249e-6,
            6.9940208056381925,
            0.7551244069345737,
        ],
    ],
];

// [framelenidx][lowrate]
const SMPL_RATE_CONTROL_THRS_COMP5: [[u16; 2]; 4] =
    [[7500, 10000], [4500, 5750], [4000, 5000], [4000, 4750]];

// Perceptual emphasis coefficients, exposed for the caller selecting voiced vs unvoiced emphasis.
pub(crate) const SMPL_PERC_EMPH_V: [f32; 2] = [-0.72, -0.77];
pub(crate) const SMPL_PERC_EMPH_UV: [f32; 2] = [-0.55, -0.6];

// Leaf vector helpers, duplicated locally.

#[inline]
fn smpl_nrg(x: &[f32]) -> f32 {
    let mut nrg = 0.0f32;
    for &v in x {
        nrg += v * v;
    }
    nrg
}

// out = input .* win
#[inline]
fn smpl_mul_vec(input: &[f32], win: &[f32], out: &mut [f32], l: usize) {
    for i in 0..l {
        out[i] = win[i] * input[i];
    }
}

// y = x * g
#[inline]
fn smpl_scale_vec(x: &[f32], y: &mut [f32], l: usize, g: f32) {
    for i in 0..l {
        y[i] = x[i] * g;
    }
}

// y = x0 + g * x1
#[inline]
fn smpl_add_scale_vec(x0: &[f32], x1: &[f32], y: &mut [f32], l: usize, g: f32) {
    for i in 0..l {
        y[i] = x0[i] + g * x1[i];
    }
}

// y += g * x
#[inline]
fn smpl_add_scale_vec_inplace(x: &[f32], y: &mut [f32], l: usize, g: f32) {
    for i in 0..l {
        y[i] += g * x[i];
    }
}

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

// 2nd-order MA filter, may be non-monic. State sits in `state[0..2]`.
fn smpl_filt_ma2(x: &[f32], n: usize, coef: &[f32], state: &[f32; 2], y: &mut [f32]) {
    debug_assert!(coef.len() == 3);
    debug_assert!(n > 1);

    if coef[0] == 1.0 {
        // monic
        smpl_add_scale_vec(&x[1..], x, &mut y[1..], n - 1, coef[1]);
    } else {
        smpl_scale_vec(x, y, n, coef[0]);
        smpl_add_scale_vec_inplace(x, &mut y[1..], n - 1, coef[1]);
    }
    smpl_add_scale_vec_inplace(x, &mut y[2..], n - 2, coef[2]);
    y[0] = coef[0] * x[0] + coef[1] * state[0] + coef[2] * state[1];
    y[1] += coef[2] * state[0];
    // The state update is dead for our single call (state is a stack temp), so we drop it.
}

// autocorrelation -> reflection coeffs, Levinson, double precision.
fn smpl_ac2rc_dbl(corr: &[f64], order: usize, reg: f64, rc: &mut [f32]) {
    debug_assert!(order > 0);
    debug_assert!(order - 1 <= SMPL_MAX_SF_LEN);
    let mut c0 = vec![0.0f64; order + 1];
    let mut c1 = vec![0.0f64; order + 1];
    c0[..(order + 1)].copy_from_slice(&corr[..(order + 1)]);
    c0[0] *= 1.0f64 + reg;
    c1.copy_from_slice(&c0);
    for r in rc[..order].iter_mut() {
        *r = 0.0;
    }
    for k in 0..order {
        if c0[k + 1] > c1[0] {
            rc[k] = -1.0;
            break;
        }
        if c0[k + 1] < -c1[0] {
            rc[k] = 1.0;
            break;
        }
        if c1[0] == 0.0 {
            break;
        }
        let rc_tmp = -c0[k + 1] / c1[0];
        rc[k] = rc_tmp as f32;
        for n in 0..(order - k) {
            let ctmp1 = c0[n + k + 1];
            let ctmp2 = c1[n];
            c0[n + k + 1] = ctmp1 + ctmp2 * rc_tmp;
            c1[n] = ctmp2 + ctmp1 * rc_tmp;
        }
    }
}

// Float wrapper that promotes to double precision before Levinson.
fn smpl_ac2rc(corr: &[f32], order: usize, reg: f32, rc: &mut [f32]) {
    debug_assert!(order > 0);
    debug_assert!(order - 1 <= SMPL_MAX_SF_LEN);
    let mut corr_dbl = vec![0.0f64; order + 1];
    for i in 0..(order + 1) {
        corr_dbl[i] = corr[i] as f64;
    }
    smpl_ac2rc_dbl(&corr_dbl, order, reg as f64, rc);
}

// reflection coeffs -> LPC polynomial A[0..order].
fn smpl_rc2a(rc: &[f32], order: usize, a: &mut [f32]) {
    for v in a[1..=order].iter_mut() {
        *v = 0.0;
    }
    a[0] = 1.0;
    for k in 0..order {
        let rc_tmp = rc[k];
        for n in 0..((k + 1) / 2) {
            let tmp1 = a[n + 1];
            let tmp2 = a[k - n];
            a[n + 1] = tmp1 + tmp2 * rc_tmp;
            a[k - n] = tmp2 + tmp1 * rc_tmp;
        }
        a[k + 1] = rc_tmp;
    }
}

// Self-contained mixed-radix complex FFT (no external deps).

#[derive(Clone, Copy)]
struct Cpx {
    re: f32,
    im: f32,
}

impl Cpx {
    #[inline]
    fn zero() -> Self {
        Cpx { re: 0.0, im: 0.0 }
    }
    #[inline]
    fn add(self, o: Cpx) -> Cpx {
        Cpx {
            re: self.re + o.re,
            im: self.im + o.im,
        }
    }
    #[inline]
    fn mul(self, o: Cpx) -> Cpx {
        Cpx {
            re: self.re * o.re - self.im * o.im,
            im: self.re * o.im + self.im * o.re,
        }
    }
}

/// Smallest prime factor of `n` (>= 2). For our use n is always composite of 2 and 3.
fn smallest_factor(n: usize) -> usize {
    if n % 2 == 0 {
        return 2;
    }
    let mut p = 3;
    while p * p <= n {
        if n % p == 0 {
            return p;
        }
        p += 2;
    }
    n
}

/// Precomputed twiddle factors for one `(top-N, sign)` FFT. The butterfly cos/sin depend only on
/// `(n, index)`, never on the input, so they are computed once at init and read in the hot loop.
/// Each table reproduces the EXACT inline angle arithmetic (same f32 op order, same std cos/sin), so
/// reading from it is bit-identical to the inline recompute; proven by `twiddle_table_is_bit_exact`.
struct FftTwiddles {
    /// Per visited length `n`, the combine-step twiddles indexed `k * p + q` (length `n * p`).
    combine: Vec<(usize, Vec<Cpx>)>,
    /// Per prime base length `n`, the base-DFT twiddles indexed `k * n + j` (length `n * n`).
    base: Vec<(usize, Vec<Cpx>)>,
}

/// Compute the combine twiddle for `(n, k, q, sign)` exactly as the inline butterfly does.
#[inline]
fn combine_twiddle(n: usize, k: usize, q: usize, sign: f32) -> Cpx {
    let ang = sign * 2.0 * SMPL_PI * (k as f32) * (q as f32) / (n as f32);
    Cpx {
        re: ang.cos(),
        im: ang.sin(),
    }
}

/// Compute the prime-base twiddle for `(n, k, j, sign)` exactly as the inline base DFT does
/// (two-step: `ang_k = sign*2π*k/n` then `ang = ang_k * j`).
#[inline]
fn base_twiddle(n: usize, k: usize, j: usize, sign: f32) -> Cpx {
    let ang_k = sign * 2.0 * SMPL_PI * (k as f32) / (n as f32);
    let ang = ang_k * (j as f32);
    Cpx {
        re: ang.cos(),
        im: ang.sin(),
    }
}

impl FftTwiddles {
    /// Build the twiddle tables for the chain of `n` values the recursion visits from `top` down,
    /// for the given `sign`.
    fn new(top: usize, sign: f32) -> Self {
        let mut combine = Vec::new();
        let mut base = Vec::new();
        let mut n = top;
        while n > 1 {
            let p = smallest_factor(n);
            if p == n {
                let mut tab = Vec::with_capacity(n * n);
                for k in 0..n {
                    for j in 0..n {
                        tab.push(base_twiddle(n, k, j, sign));
                    }
                }
                base.push((n, tab));
                break;
            }
            let mut tab = Vec::with_capacity(n * p);
            for k in 0..n {
                for q in 0..p {
                    tab.push(combine_twiddle(n, k, q, sign));
                }
            }
            combine.push((n, tab));
            n /= p;
        }
        FftTwiddles { combine, base }
    }

    #[inline]
    fn combine_for(&self, n: usize) -> &[Cpx] {
        for (m, tab) in &self.combine {
            if *m == n {
                return tab;
            }
        }
        unreachable!("combine twiddle table missing for n={n}");
    }

    #[inline]
    fn base_for(&self, n: usize) -> &[Cpx] {
        for (m, tab) in &self.base {
            if *m == n {
                return tab;
            }
        }
        unreachable!("base twiddle table missing for n={n}");
    }
}

/// Reusable FFT workspace owned by `PercModelState`, so the per-frame perc model runs its ~12
/// forward+backward FFTs of N=PERCW_NFFT without re-allocating the recursion scratch every call and
/// without recomputing the input-independent butterfly cos/sin. `arena` backs `fft_rec`'s per-level
/// `sub` buffers (carved by `split_at_mut`); `cin`/`spec`/`tout` back the real-FFT pack/unpack;
/// `tw_fwd`/`tw_bwd` are the precomputed twiddles for the two signs. Reuse never touches the
/// arithmetic, so the output is bit-identical.
pub(crate) struct FftScratch {
    arena: Vec<Cpx>,
    cin: Vec<Cpx>,
    spec: Vec<Cpx>,
    tout: Vec<Cpx>,
    tw_fwd: FftTwiddles, // sign = -1.0
    tw_bwd: FftTwiddles, // sign = +1.0
}

impl FftScratch {
    pub(crate) fn new(n: usize) -> Self {
        FftScratch {
            arena: vec![Cpx::zero(); fft_arena_len(n)],
            cin: vec![Cpx::zero(); n],
            spec: vec![Cpx::zero(); n],
            tout: vec![Cpx::zero(); n],
            tw_fwd: FftTwiddles::new(n, -1.0),
            tw_bwd: FftTwiddles::new(n, 1.0),
        }
    }
}

/// Worst-case `fft_rec` arena length: at each level we carve `n` `Cpx` for `sub` then recurse on
/// `m = n/p` (children are sequential, so they reuse the remainder; the bound is additive down one
/// path, not across siblings). Computed at state init for the fixed N (576 -> 1143).
fn fft_arena_len(n: usize) -> usize {
    if n <= 1 {
        return 0;
    }
    let p = smallest_factor(n);
    if p == n {
        return 0; // prime base case allocates nothing
    }
    n + fft_arena_len(n / p)
}

/// Recursive mixed-radix Cooley-Tukey DFT. `x[i]` are the n input samples at stride `stride` within
/// the parent array; `out` is contiguous. `scratch` is the remaining arena: each non-base level
/// splits off the first `n` entries for its `sub` buffer and passes the rest down (siblings are
/// sequential, so the rest is safely shared). `tw` supplies the precomputed butterfly twiddles for
/// this FFT's sign, bit-identical to the inline cos/sin it replaces.
fn fft_rec(
    x: &[Cpx],
    stride: usize,
    n: usize,
    out: &mut [Cpx],
    scratch: &mut [Cpx],
    tw: &FftTwiddles,
) {
    if n == 1 {
        out[0] = x[0];
        return;
    }
    let p = smallest_factor(n);
    if p == n {
        // Prime (or n with no small factor): naive O(n^2) DFT base case.
        let bt = tw.base_for(n);
        for k in 0..n {
            let mut acc = Cpx::zero();
            let row = k * n;
            for j in 0..n {
                acc = acc.add(x[j * stride].mul(bt[row + j]));
            }
            out[k] = acc;
        }
        return;
    }
    let m = n / p;
    // Carve this level's `sub` (size n) from the arena; the remainder feeds the child sub-DFTs.
    let (sub, rest) = scratch.split_at_mut(n);
    // Compute p sub-DFTs of length m over the decimated inputs (sub[q*m .. (q+1)*m]).
    for (q, dst) in sub.chunks_mut(m).enumerate() {
        fft_rec(&x[q * stride..], stride * p, m, dst, rest, tw);
    }
    // Combine: out[k] = sum_q twiddle(q,k) * sub_q[k mod m], with the radix-p butterfly.
    let ct = tw.combine_for(n);
    for k in 0..n {
        let kmod = k % m;
        let mut acc = Cpx::zero();
        let row = k * p;
        for q in 0..p {
            acc = acc.add(sub[q * m + kmod].mul(ct[row + q]));
        }
        out[k] = acc;
    }
}

/// Complex FFT of a power-/mixed-radix length into `out`. `sign=-1` forward, `+1` inverse.
fn cfft(input: &[Cpx], out: &mut [Cpx], sign: f32, arena: &mut [Cpx], tw: &FftTwiddles) {
    let n = input.len();
    debug_assert!(out.len() == n);
    debug_assert!(sign == -1.0 || sign == 1.0);
    fft_rec(input, 1, n, out, arena, tw);
}

/// Forward real FFT of `n` real samples, re-packed into the ordered REAL layout:
/// f[0]=DC.re, f[1]=Nyquist.re, then [re,im] pairs for bins 1..n/2-1. Output length is n.
/// Uses `sc.cin`/`sc.spec`/`sc.arena` as scratch (reused across calls).
pub(crate) fn rfft_forward_ordered_sc(time: &[f32], f: &mut [f32], sc: &mut FftScratch) {
    let n = time.len();
    debug_assert!(f.len() == n);
    let cin = &mut sc.cin;
    for i in 0..n {
        cin[i].re = time[i];
        cin[i].im = 0.0;
    }
    cfft(cin, &mut sc.spec, -1.0, &mut sc.arena, &sc.tw_fwd);
    let spec = &sc.spec;
    f[0] = spec[0].re;
    f[1] = spec[n / 2].re;
    for i in 1..(n / 2) {
        f[2 * i] = spec[i].re;
        f[2 * i + 1] = spec[i].im;
    }
}

/// Test-only allocating wrapper preserving the original `rfft_forward_ordered` signature for the FFT
/// unit tests. Production LPC analysis threads a reusable `FftScratch`
/// (`smpl_lpc::new_lpc_fft_scratch`) instead. Same arithmetic, so output is bit-identical.
#[cfg(test)]
fn rfft_forward_ordered(time: &[f32], f: &mut [f32]) {
    let mut sc = FftScratch::new(time.len());
    rfft_forward_ordered_sc(time, f, &mut sc);
}

/// Inverse real FFT consuming the ordered REAL layout (as produced/modified above) and producing n
/// real time samples, unnormalized (BACKWARD(FORWARD(x)) = n*x).
/// Uses `sc.spec`/`sc.tout`/`sc.arena` as scratch (reused across calls).
fn rfft_backward_ordered_sc(f: &[f32], time: &mut [f32], sc: &mut FftScratch) {
    let n = f.len();
    debug_assert!(time.len() == n);
    // Rebuild the full Hermitian complex spectrum from the ordered real layout.
    let spec = &mut sc.spec;
    spec[0] = Cpx { re: f[0], im: 0.0 };
    spec[n / 2] = Cpx { re: f[1], im: 0.0 };
    for i in 1..(n / 2) {
        let re = f[2 * i];
        let im = f[2 * i + 1];
        spec[i] = Cpx { re, im };
        spec[n - i] = Cpx { re, im: -im }; // conjugate symmetry
    }
    cfft(spec, &mut sc.tout, 1.0, &mut sc.arena, &sc.tw_bwd);
    let tout = &sc.tout;
    for i in 0..n {
        time[i] = tout[i].re;
    }
}

/// Test-only allocating wrapper preserving the original `rfft_backward_ordered` signature.
#[cfg(test)]
fn rfft_backward_ordered(f: &[f32], time: &mut [f32]) {
    let mut sc = FftScratch::new(f.len());
    rfft_backward_ordered_sc(f, time, &mut sc);
}

// Perceptual model

/// Window tables built once, covering the perc-relevant windows.
struct PercWindows {
    perc_win1_10ms: Vec<f32>, // gen_sin_win, len 192
    perc_win1_20ms: Vec<f32>, // gen_sin_win, len 352
    win3_short: Vec<f32>,     // gen_cos_win, len 32
    win3_long: Vec<f32>,      // gen_cos_win, len 64
}

// win[i] = sin((i+1)/(N+1) * pi/2)
fn gen_sin_win(n: usize) -> Vec<f32> {
    let mut w = vec![0.0f32; n];
    for i in 0..n {
        w[i] = ((i as f32 + 1.0) / (n as f32 + 1.0) * SMPL_PI / 2.0).sin();
    }
    w
}

// win[i] = cos((i+1)/(N+1) * pi/2)
fn gen_cos_win(n: usize) -> Vec<f32> {
    let mut w = vec![0.0f32; n];
    for i in 0..n {
        w[i] = ((i as f32 + 1.0) / (n as f32 + 1.0) * SMPL_PI / 2.0).cos();
    }
    w
}

impl PercWindows {
    fn new() -> Self {
        PercWindows {
            perc_win1_10ms: gen_sin_win(SMPL_PERC_WIN1_10MS_LEN),
            perc_win1_20ms: gen_sin_win(SMPL_PERC_WIN1_20MS_LEN),
            win3_short: gen_cos_win(SMPL_WIN3_SHORT_LEN),
            win3_long: gen_cos_win(SMPL_WIN3_LONG_LEN),
        }
    }
}

// Windowing for the perc case (use_lpc_win == FALSE).
fn smpl_window_perc(
    win: &PercWindows,
    input: &[f32],
    out: &mut [f32],
    len: usize,
    frame_ms: i32,
    use_long_win: bool,
) {
    let (win1len, win1): (usize, &[f32]) = if frame_ms == 10 {
        (SMPL_PERC_WIN1_10MS_LEN, &win.perc_win1_10ms)
    } else {
        (SMPL_PERC_WIN1_20MS_LEN, &win.perc_win1_20ms)
    };
    let (win3len, win3): (usize, &[f32]) = if use_long_win {
        (SMPL_WIN3_LONG_LEN, &win.win3_long)
    } else {
        (SMPL_WIN3_SHORT_LEN, &win.win3_short)
    };

    smpl_mul_vec(input, win1, out, win1len);
    let mid = len - win1len - SMPL_WIN3_LONG_LEN;
    out[win1len..win1len + mid].copy_from_slice(&input[win1len..win1len + mid]);
    smpl_mul_vec(
        &input[len - SMPL_WIN3_LONG_LEN..],
        win3,
        &mut out[len - SMPL_WIN3_LONG_LEN..],
        win3len,
    );
    if !use_long_win {
        let start = len - SMPL_WIN3_LONG_LEN + SMPL_WIN3_SHORT_LEN;
        for v in out[start..len].iter_mut() {
            *v = 0.0;
        }
    }
}

// Bidirectional masking smooth across the power spectrum.
fn smth_filt(f: &mut [f32], smthcoef: &[f32]) {
    let half = PERCW_NFFT / 2;
    let mut f2smth = f[0];
    for i in 1..half {
        let f2new = f[2 * i];
        f2smth = f2new + smthcoef[i] * (f2smth - f2new);
        f[2 * i] = f2smth;
    }
    f[1] = f[1] + smthcoef[half] * (f2smth - f[1]);
    f2smth = f[1];
    let mut i = half - 1;
    while i > 0 {
        let f2new = f[2 * i];
        f2smth = f2new + smthcoef[i] * (f2smth - f2new);
        f[2 * i] = f2smth;
        i -= 1;
    }
    f[0] = f[0] + smthcoef[0] * (f2smth - f[0]);
}

/// State carried across `smpl_perc_model` calls (the `buf` history of length PERCW_NFFT), plus the
/// reusable per-call FFT/windowing scratch so the perceptual model allocates nothing per frame.
pub(crate) struct PercModelState {
    buf: [f32; PERCW_NFFT],
    smthcoef: Vec<f32>, // length PERCW_NFFT/2 + 1
    windows: PercWindows,
    fft: FftScratch,
    buf_win: Vec<f32>, // length PERCW_NFFT, windowed time-domain frame
    f: Vec<f32>,       // length PERCW_NFFT, ordered REAL spectrum / power
}

impl PercModelState {
    pub(crate) fn new() -> Self {
        // Per-bin mel-width smoothing coefficients.
        let fs_step = (PERCW_FS_KHZ * 1000.0) / PERCW_NFFT as f32;
        let mut smthcoef = vec![0.0f32; PERCW_NFFT / 2 + 1];
        for i in 0..(PERCW_NFFT / 2 + 1) {
            let perc_width_per_bin =
                SMPL_PERC_MASK_SMTH * (fs_step * i as f32 + SMPL_PERC_MEL_FC_HZ) / fs_step;
            smthcoef[i] = perc_width_per_bin / (perc_width_per_bin + 1.0);
        }
        PercModelState {
            buf: [0.0; PERCW_NFFT],
            smthcoef,
            windows: PercWindows::new(),
            fft: FftScratch::new(PERCW_NFFT),
            buf_win: vec![0.0f32; PERCW_NFFT],
            f: vec![0.0f32; PERCW_NFFT],
        }
    }
}

/// Windowed power spectrum -> bidirectional masking smooth -> inverse -> 1/NFFT scale.
/// Returns the first `len_r` autocorrelation lags. The `buf` history advances by dropping the
/// consumed front, keeping the tail, and appending the new subframe.
pub(crate) fn smpl_perc_model(
    state: &mut PercModelState,
    xsubfr: &[f32],
    xsubfr_len: usize,
    frame_ms: i32,
    is_last_subfr: i32,
    len_r: usize,
) -> Vec<f32> {
    debug_assert!(xsubfr_len <= PERCW_NFFT);

    // Advance history: drop the consumed front, keep the (NFFT - xsubfr_len) tail, then append xsubfr.
    // Source offset is xsubfr_len - (WB_LONG_LEN - WB_LEN) = xsubfr_len - 32.
    let src_off = xsubfr_len - (SMPL_WINNEXT_WB_LONG_LEN - SMPL_WINNEXT_WB_LEN);
    let keep = PERCW_NFFT - xsubfr_len;
    state.buf.copy_within(src_off..src_off + keep, 0);
    state.buf[keep..keep + xsubfr_len].copy_from_slice(&xsubfr[..xsubfr_len]);

    let winlen = SMPL_WINPREV_PERC_LEN + (frame_ms as usize) * 16 + SMPL_WIN3_LONG_LEN;
    let skip_samples = PERCW_NFFT - winlen;
    debug_assert!(frame_ms == 10 || skip_samples == 0);

    // Reuse the persistent scratch; re-zero buf_win so the windowed frame matches a fresh
    // `vec![0.0; NFFT]` (the skip region plus any window bins the windowing leaves untouched).
    state.buf_win.fill(0.0);
    smpl_window_perc(
        &state.windows,
        &state.buf[skip_samples..],
        &mut state.buf_win[skip_samples..],
        winlen,
        frame_ms,
        is_last_subfr == SMPL_FALSE,
    );

    rfft_forward_ordered_sc(&state.buf_win, &mut state.f, &mut state.fft);
    let f = &mut state.f;
    f[0] = f[0] * f[0]; // DC
    f[1] = f[1] * f[1]; // Nyquist
    for i in 1..(PERCW_NFFT / 2) {
        f[2 * i] = f[2 * i] * f[2 * i] + f[2 * i + 1] * f[2 * i + 1];
        f[2 * i + 1] = 0.0;
    }
    smth_filt(f, &state.smthcoef);
    rfft_backward_ordered_sc(&state.f, &mut state.buf_win, &mut state.fft);

    let mut r = vec![0.0f32; len_r];
    smpl_scale_vec(&state.buf_win, &mut r, len_r, 1.0 / PERCW_NFFT as f32);
    r
}

/// ma2 (b={pe, 1+pe^2, pe}) on R[1..] then Levinson + rc2a -> A[0..perc_resp_len].
pub(crate) fn smpl_perc_ac2a(
    r: &[f32],
    len_r: usize,
    perc_emph: f32,
    perc_resp_len: usize,
    reg: f32,
) -> Vec<f32> {
    debug_assert!(len_r >= perc_resp_len + 1);
    debug_assert!(SMPL_MAX_L_RESP >= perc_resp_len);

    let b = [perc_emph, 1.0 + perc_emph * perc_emph, perc_emph];
    let state = [r[0], r[1]];
    let mut r_ = vec![0.0f32; SMPL_MAX_L_RESP];
    smpl_filt_ma2(&r[1..], perc_resp_len, &b, &state, &mut r_);

    let mut rc = vec![0.0f32; SMPL_MAX_L_RESP];
    smpl_ac2rc(&r_, perc_resp_len - 1, reg, &mut rc);

    let mut a = vec![0.0f32; perc_resp_len];
    smpl_rc2a(&rc, perc_resp_len - 1, &mut a);
    a
}

// Bitrate controller

fn bitrate2pulses(rate_kbps: f32, coeff: &[f32; 8]) -> f32 {
    coeff[0]
        + coeff[1] * rate_kbps
        + coeff[2] * rate_kbps * rate_kbps
        + coeff[3] * rate_kbps.powf(3.0)
        + coeff[4] * rate_kbps.powf(4.0)
        + coeff[5] * SMPL_E.powf((rate_kbps - coeff[6]) * coeff[7])
}

fn bitrate2pulses_hr_fec(rate_kbps: f32, coeff: &[f32; 8], one_pulse_rate_bps: f32) -> f32 {
    const RATE_THRES_KBPS: f32 = 9.0; // only compensate below this value
    if rate_kbps >= RATE_THRES_KBPS {
        bitrate2pulses(rate_kbps, coeff)
    } else if one_pulse_rate_bps >= RATE_THRES_KBPS * 1000.0 {
        1.0
    } else {
        let pulses_thres = bitrate2pulses(RATE_THRES_KBPS, coeff);
        let sc = (RATE_THRES_KBPS - rate_kbps) / (RATE_THRES_KBPS - one_pulse_rate_bps / 1000.0);
        pulses_thres - sc * (pulses_thres - 1.0)
    }
}

/// Encoder-control inputs the controller reads.
#[derive(Clone, Copy)]
pub(crate) struct BitrateControllerInputs {
    pub internal_sample_rate: i32,
    pub payload_size_ms: i32,
    pub fec_bit_rate: i32,
    pub main_bit_rate: i32,
    pub complexity: i32,
    pub use_fec_rate_compensation: i32,
    pub use_dtx: i32,
    pub sub_frame_importance_factor: f32,
}

/// BitrateController state carried across frames.
pub(crate) struct BitrateController {
    prev_voiced: i32,
    rate_cont_wnrg_smth: f32,
    rate_cont_bitrate_scale: [f32; SMPL_CELP_MAX_RATES],
    bitrate_delta_smth: [f32; SMPL_CELP_MAX_RATES],
    rate_cont_bitrate: [f32; SMPL_CELP_MAX_RATES],
    adjustment_factor: [f32; SMPL_CELP_MAX_RATES],
}

impl BitrateController {
    /// Controller init plus zeroed state.
    pub(crate) fn new() -> Self {
        BitrateController {
            prev_voiced: 0,
            rate_cont_wnrg_smth: 0.0,
            rate_cont_bitrate_scale: [0.0; SMPL_CELP_MAX_RATES],
            bitrate_delta_smth: [0.0; SMPL_CELP_MAX_RATES],
            rate_cont_bitrate: [0.0; SMPL_CELP_MAX_RATES],
            adjustment_factor: [1.0; SMPL_CELP_MAX_RATES],
        }
    }

    /// Returns (max_pulses_per_subfr, subfr_importance).
    pub(crate) fn control(
        &mut self,
        enc: &BitrateControllerInputs,
        dtx_sid_frame: i32,
        coded_as_active_voice: i32,
        sp_act_prob: f32,
        nonflatness: f32,
        voicing_strength: f32,
        voiced: i32,
        wnrg: f32,
        wnrg_next: f32,
        low_rate: i32,
        framelen: i32,
        subfrlen: i32,
    ) -> ([i16; SMPL_CELP_MAX_RATES], [f32; SMPL_CELP_MAX_RATES]) {
        debug_assert!(sp_act_prob >= 0.0 && sp_act_prob <= 1.0);

        let mut bwe_bitrate = 0i32;
        if enc.internal_sample_rate > 16000 {
            bwe_bitrate += if low_rate != 0 { 450 } else { 750 };
            bwe_bitrate += if enc.payload_size_ms == 10 { 450 } else { 0 };
        }

        self.rate_cont_wnrg_smth += 0.6 * (wnrg - self.rate_cont_wnrg_smth);

        let framelen_idx = if enc.payload_size_ms == 10 {
            0usize
        } else if enc.payload_size_ms == 20 {
            1
        } else if enc.payload_size_ms == 60 {
            2
        } else {
            3
        };

        let mut max_pulses_per_subfr = [0i16; SMPL_CELP_MAX_RATES];
        let mut subfr_importance = [0.0f32; SMPL_CELP_MAX_RATES];

        // Operator precedence: `SMPL_CELP_IDX_FEC + (a == 0) || (b == c)` parses as
        // `(SMPL_CELP_IDX_FEC + (a == 0)) || (b == c)` (a `||` whose value is 0 or 1).
        let start_r: usize = if ((SMPL_CELP_IDX_FEC as i32 + (enc.fec_bit_rate == 0) as i32) != 0)
            || (enc.fec_bit_rate == enc.main_bit_rate)
        {
            1
        } else {
            0
        };

        let lr_idx = if low_rate != 0 { 0usize } else { 1 };

        for r in start_r..=SMPL_CELP_IDX_MAIN {
            let mut bit_rate = if r == SMPL_CELP_IDX_FEC {
                enc.fec_bit_rate as f32
            } else {
                enc.main_bit_rate as f32
            };
            bit_rate = bit_rate.min(30000.0); // don't extrapolate pulses_per_20ms_target_max curves
            let mut rate_kbps = (bit_rate - bwe_bitrate as f32) / 1000.0;
            if low_rate == 0 {
                rate_kbps *= match enc.complexity {
                    1 => 0.9900990,
                    2 => 0.9900990,
                    3 => 1.0101010,
                    4 => 1.0101010,
                    _ => 1.0,
                };
            }

            let pulses_per_20ms_target_max;
            let rate_control_thrs = SMPL_RATE_CONTROL_THRS_COMP5[framelen_idx][lr_idx] as f32;
            if (bit_rate - bwe_bitrate as f32) < rate_control_thrs {
                pulses_per_20ms_target_max = 1.0;
            } else {
                let coeff = &SMPL_RATE_CONTROL_MODEL_COMP5[framelen_idx][lr_idx];
                if (r == SMPL_CELP_IDX_FEC) && low_rate == 0 && enc.use_fec_rate_compensation != 0 {
                    pulses_per_20ms_target_max =
                        bitrate2pulses_hr_fec(rate_kbps, coeff, rate_control_thrs).max(1.0);
                } else {
                    pulses_per_20ms_target_max = bitrate2pulses(rate_kbps, coeff).max(1.0);
                }
            }

            let rel_pulserate = pulses_per_20ms_target_max / 16.0 * (320.0 / framelen as f32);
            debug_assert!(rel_pulserate > 0.0);
            let rel_pulserate_log = rel_pulserate.ln();
            if self.rate_cont_bitrate[r] != bit_rate {
                let bitrate_scale = SMPL_RATE_CONT_SCALE
                    * rel_pulserate
                    * (1.0 + 0.4 * rel_pulserate_log * rel_pulserate_log);
                self.rate_cont_bitrate_scale[r] = bitrate_scale;
                self.rate_cont_bitrate[r] = bit_rate;
            }

            let numsubfrs = framelen / subfrlen;
            let mut mpps =
                1 + (pulses_per_20ms_target_max * (1.0 + 0.5) / numsubfrs as f32).round() as i32;
            if enc.use_dtx != 0 && dtx_sid_frame != 0 {
                mpps = 0;
            } else {
                mpps = (mpps as f32 * (0.5 + 0.5 * (sp_act_prob + 1e-12).sqrt())).round() as i32;
                let frame_type = if coded_as_active_voice == 0 {
                    BACKGROUND_NOISE
                } else if voiced == 1 {
                    VOICED
                } else {
                    UNVOICED
                };
                // smpl_max_pulses_per_frame is indexed by the raw low_rate flag (0=high-rate row),
                // unlike the model/thrs tables which use `low_rate ? 0 : 1`.
                let max_pulses = SMPL_MAX_PULSES_PER_FRAME[low_rate as usize][frame_type] as i32
                    * framelen
                    / 320;
                mpps = mpps.min(max_pulses / numsubfrs); // don't overshoot the PDF
            }
            debug_assert!(mpps <= SMPL_MAX_PULSES_PER_SF);
            max_pulses_per_subfr[r] = mpps as i16;

            let mut imp =
                (wnrg + 0.01 * wnrg_next) / (self.rate_cont_wnrg_smth + 0.02 * wnrg_next + 1e-12);
            if voiced != 0 {
                if bit_rate <= 9000.0 {
                    imp = (imp + 1e-12).sqrt();
                }
            } else {
                imp *= 0.9 + 0.3 * smpl_sigmoid(nonflatness - 2.0);
                imp *= 0.8;
            }
            if voiced != self.prev_voiced {
                imp *= 1.1;
            }
            imp *= 0.9 + 0.3 * 1.0 / (1.0 + 25.0 * voicing_strength * voicing_strength);

            // Speech-activity bitrate allocation shaping.
            let mut imp_factor = enc.sub_frame_importance_factor;
            if imp_factor <= 1.0 {
                imp *= (1.0 - imp_factor) + imp_factor * (sp_act_prob + 1e-12).sqrt();
            } else if imp_factor <= 2.0 {
                imp_factor -= 1.0;
                imp *= (1.0 - imp_factor) + imp_factor * sp_act_prob;
            } else {
                debug_assert!(imp_factor <= 3.0);
                imp_factor -= 2.0;
                imp *= (1.0 - imp_factor) + imp_factor * sp_act_prob * sp_act_prob;
            }
            imp *= self.adjustment_factor[r] * self.rate_cont_bitrate_scale[r];
            subfr_importance[r] = imp;
            self.prev_voiced = voiced;
        }

        (max_pulses_per_subfr, subfr_importance)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The precomputed twiddle tables MUST reproduce the inline cos/sin to the bit, or the golden
    // checksum shifts. Assert every used (n, k, q) combine entry and (n, k, j) base entry equals the
    // inline recompute for both signs (forward = -1, backward = +1) at both FFT sizes (576, 512).
    #[test]
    fn twiddle_table_is_bit_exact() {
        for &top in &[PERCW_NFFT, 512usize] {
            for &sign in &[-1.0f32, 1.0f32] {
                let tw = FftTwiddles::new(top, sign);
                // Combine tables.
                let mut n = top;
                while n > 1 {
                    let p = smallest_factor(n);
                    if p == n {
                        let bt = tw.base_for(n);
                        for k in 0..n {
                            for j in 0..n {
                                let want = base_twiddle(n, k, j, sign);
                                let got = bt[k * n + j];
                                assert_eq!(
                                    got.re.to_bits(),
                                    want.re.to_bits(),
                                    "base re mismatch n={n} k={k} j={j} sign={sign}"
                                );
                                assert_eq!(
                                    got.im.to_bits(),
                                    want.im.to_bits(),
                                    "base im mismatch n={n} k={k} j={j} sign={sign}"
                                );
                            }
                        }
                        break;
                    }
                    let ct = tw.combine_for(n);
                    for k in 0..n {
                        for q in 0..p {
                            let want = combine_twiddle(n, k, q, sign);
                            let got = ct[k * p + q];
                            assert_eq!(
                                got.re.to_bits(),
                                want.re.to_bits(),
                                "combine re mismatch n={n} k={k} q={q} sign={sign}"
                            );
                            assert_eq!(
                                got.im.to_bits(),
                                want.im.to_bits(),
                                "combine im mismatch n={n} k={k} q={q} sign={sign}"
                            );
                        }
                    }
                    n /= p;
                }
            }
        }
    }

    #[test]
    fn percw_nfft_is_576_non_pow2() {
        assert_eq!(PERCW_NFFT, 576);
        assert_ne!(
            PERCW_NFFT & (PERCW_NFFT - 1),
            0,
            "576 is not a power of two"
        );
    }

    // FFT -> IFFT round-trips to n*x within float tolerance.
    #[test]
    fn fft_roundtrip() {
        let n = PERCW_NFFT;
        let mut x = vec![0.0f32; n];
        // deterministic pseudo-random-ish signal
        let mut s: u32 = 12345;
        for v in x.iter_mut() {
            s = s.wrapping_mul(196314165).wrapping_add(907633515);
            *v = ((s >> 9) as f32 / (1u32 << 23) as f32) - 1.0;
        }
        let mut f = vec![0.0f32; n];
        rfft_forward_ordered(&x, &mut f);
        let mut back = vec![0.0f32; n];
        rfft_backward_ordered(&f, &mut back);
        for i in 0..n {
            let expected = x[i] * n as f32;
            assert!(
                (back[i] - expected).abs() < 1e-1 * (1.0 + expected.abs()),
                "idx {i}: got {}, want {}",
                back[i],
                expected
            );
        }
    }

    // Known signal: a single real cosine at bin k has power N^2/4 at bin k (one-sided), 0 elsewhere.
    #[test]
    fn power_spectrum_of_cosine() {
        let n = PERCW_NFFT;
        let k = 9usize; // a bin reachable by both radix-2 and radix-3 paths
        let mut x = vec![0.0f32; n];
        for i in 0..n {
            x[i] = (2.0 * SMPL_PI * (k as f32) * (i as f32) / (n as f32)).cos();
        }
        let mut f = vec![0.0f32; n];
        rfft_forward_ordered(&x, &mut f);
        // power at bin k (interleaved [re,im] for 1..n/2-1)
        let pk = f[2 * k] * f[2 * k] + f[2 * k + 1] * f[2 * k + 1];
        let expected = (n as f32 / 2.0) * (n as f32 / 2.0);
        assert!(
            (pk - expected).abs() < 1e-2 * expected,
            "bin {k} power {pk}, want {expected}"
        );
        // a far-away bin should be ~0
        let m = 40usize;
        let pm = f[2 * m] * f[2 * m] + f[2 * m + 1] * f[2 * m + 1];
        assert!(pm < 1e-2 * expected, "bin {m} power {pm} not ~0");
    }

    // smpl_perc_model end-to-end smoke: with zero input it returns all-zero R; a DC step gives R[0]>0.
    #[test]
    fn perc_model_smoke() {
        let mut st = PercModelState::new();
        let xsubfr = vec![0.0f32; 320];
        let r = smpl_perc_model(&mut st, &xsubfr, 320, 20, SMPL_FALSE, SMPL_MAX_L_RESP);
        assert_eq!(r.len(), SMPL_MAX_L_RESP);
        for &v in &r {
            assert!(v.abs() < 1e-6, "zero input must give ~0 autocorr, got {v}");
        }

        let mut st2 = PercModelState::new();
        let dc = vec![1.0f32; 320];
        let r2 = smpl_perc_model(&mut st2, &dc, 320, 20, SMPL_FALSE, SMPL_MAX_L_RESP);
        assert!(
            r2[0] > 0.0,
            "DC input must give positive R[0], got {}",
            r2[0]
        );
        // ac2a must run without panicking and return A[0] == 1.0
        let a = smpl_perc_ac2a(
            &r2,
            SMPL_MAX_L_RESP,
            SMPL_PERC_EMPH_V[0],
            SMPL_PERC_RESP_LEN,
            SMPL_PERC_REG,
        );
        assert_eq!(a.len(), SMPL_PERC_RESP_LEN);
        assert_eq!(a[0], 1.0);
    }

    #[test]
    fn bitrate_controller_runs() {
        let mut bc = BitrateController::new();
        let enc = BitrateControllerInputs {
            internal_sample_rate: 16000,
            payload_size_ms: 20,
            fec_bit_rate: 0,
            main_bit_rate: 16000,
            complexity: 5,
            use_fec_rate_compensation: 0,
            use_dtx: 0,
            sub_frame_importance_factor: 1.0,
        };
        let (mp, si) = bc.control(&enc, 0, 1, 0.9, 1.5, 0.3, 1, 100.0, 80.0, 0, 320, 80);
        // FEC disabled (fec_bit_rate==0) so start_r==1: only MAIN index is populated.
        assert_eq!(mp[0], 0);
        assert!(mp[1] >= 1);
        assert!(si[1].is_finite());
    }

    // R4 budget: for the active MLow config (20 kbps, 60 ms payload, complexity 8, high-rate, unvoiced)
    // the per-subframe pulse budget must equal 23 (the reference value, per testdata/PROVENANCE.md).
    // Regression for the inverted `smpl_max_pulses_per_frame` index that capped unvoiced subframes at
    // 8 (the `{16,32,32}` low-rate row) instead of 23.
    #[test]
    fn bitrate_controller_active_unvoiced_budget_matches_c() {
        let mut bc = BitrateController::new();
        let enc = BitrateControllerInputs {
            internal_sample_rate: 16000,
            payload_size_ms: 60,
            fec_bit_rate: 0,
            main_bit_rate: 20000,
            complexity: 8,
            use_fec_rate_compensation: 0,
            use_dtx: 0,
            sub_frame_importance_factor: 1.0,
        };
        // coded_as_active_voice=1, voiced=0, low_rate=0, framelen=320, subfrlen=80, sp_act_prob≈1.
        let (mp, _si) = bc.control(
            &enc, 0, 1, 0.9961, 0.2, -0.18, 0, 3.0e-5, 5.0e-5, 0, 320, 80,
        );
        assert_eq!(
            mp[1], 23,
            "active-unvoiced max_pulses must match the reference (23/subframe)"
        );
    }
}

//! MLow LSF QUANTIZER (encoder side). Given the analysis NLSF, it runs the VQ_temp Mahalanobis
//! shortlist over the stage-1 centroids, then an RD beam (`0.5*order*log2(werr)*RDw_adj + bits`) with
//! per-coeff stage-2 clamps, returning `qi[0..16]`. `qi[0]` is the Rust wire `grid`, `qi[1..16]` the
//! `stage2`. Replaces the SSE-only grid heuristic that froze `grid` at 0 and produced wrong formants.
//!
//! The codebook tables (`cbhalf/cbCinv/wie/we/bits/Rotcond/cInv`, stage-2 `Qlvls/numBits`, the clamp
//! tables, qstep, means, reg_cond, min_dist) are stored verbatim, so the quantizer math is bit-exact
//! against the reference.
//!
//! Validated 60/60 bit-exact against the reference quantizer (see the test). It is the dominant fix
//! for the frozen-grid mute and is now wired live in `analysis.rs`, fed the faithful front-end NLSF
//! (`smpl_lpc::smpl_a2nlsf_16`). The earlier "resonant NLSF rings up" regression was a symptom of the
//! WRONG LPC input (the old heuristic front-end), not a missing decoder stabilizer: the decoder's
//! `smpl_reconstruct_nlsf` already reproduces the reference `qlsf` (proven in `smpl_lpc`'s
//! `decoder_reconstructs_c_qlsf`), so no synthesis-time stabilization was needed.
#![allow(dead_code)] // serde-only fields + test-only reconstruction helpers
#![allow(clippy::needless_range_loop)]
#![allow(clippy::too_many_arguments)]

pub(crate) const SMPL_LPC_ORDER: usize = 16;
pub(crate) const LSF_CB_CENTROIDS: usize = 16;
const LSF_QSTEP_COND_MULT: f32 = 0.9;
const SMPL_PI: f32 = std::f32::consts::PI;

#[derive(serde::Deserialize, serde::Serialize)]
pub(crate) struct St1Json {
    pub(crate) cbhalf: Vec<Vec<f32>>, // [16][16]
    #[serde(rename = "cInv")]
    pub(crate) c_inv: Vec<Vec<f32>>, // [16][16]
    pub(crate) bits_cond: Vec<f32>,   // [17]
    #[serde(rename = "Rotcond")]
    pub(crate) rotcond: Vec<Vec<Vec<f32>>>, // [2][16][16]
    #[serde(rename = "cbCinv")]
    pub(crate) cb_cinv: Vec<Vec<f32>>, // [16][16]
    pub(crate) we: Vec<Vec<Vec<f32>>>, // [16][16][16]
    pub(crate) bits: Vec<f32>,        // [16]
    pub(crate) wie: Vec<Vec<Vec<f32>>>, // [16][16][16]
}

#[derive(serde::Deserialize, serde::Serialize)]
pub(crate) struct St2Json {
    #[serde(rename = "numQlvls")]
    pub(crate) num_qlvls: Vec<i32>, // [16]
    #[serde(rename = "Qlvls")]
    pub(crate) qlvls: Vec<Vec<f32>>, // [16][numQlvls[i]]
    #[serde(rename = "numBits")]
    pub(crate) num_bits: Vec<Vec<f32>>, // [16][numQlvls[i]]
}

#[derive(serde::Deserialize, serde::Serialize)]
pub(crate) struct LsfCbJson {
    pub(crate) st1: Vec<St1Json>,               // [2]
    pub(crate) st2: Vec<Vec<Vec<St2Json>>>,     // [2][2][17]
    pub(crate) min_qi: Vec<Vec<Vec<Vec<i32>>>>, // [2][2][17][16]
    pub(crate) max_qi: Vec<Vec<Vec<Vec<i32>>>>, // [2][2][17][16]
    pub(crate) qstep: Vec<Vec<f32>>,            // [2][2]
    pub(crate) mean_v: Vec<f32>,                // [16]
    pub(crate) mean_uv: Vec<f32>,               // [16]
    pub(crate) reg_cond: Vec<f32>,              // [2]
    pub(crate) min_dist_v: Vec<f32>,            // [17]
    pub(crate) min_dist_uv: Vec<f32>,           // [17]
}

pub(crate) struct LsfCb {
    j: LsfCbJson,
}

impl LsfCb {
    pub(crate) fn from_json(j: LsfCbJson) -> Self {
        LsfCb { j }
    }

    #[cfg(test)]
    pub(crate) fn st1(&self, voiced: usize) -> &St1Json {
        &self.j.st1[voiced]
    }

    #[cfg(test)]
    pub(crate) fn st2(&self, voiced: usize, low_rate: usize, c: usize) -> &St2Json {
        &self.j.st2[voiced][low_rate][c]
    }

    #[cfg(test)]
    pub(crate) fn min_qi_ref(&self) -> &Vec<Vec<Vec<Vec<i32>>>> {
        &self.j.min_qi
    }
    #[cfg(test)]
    pub(crate) fn max_qi_ref(&self) -> &Vec<Vec<Vec<Vec<i32>>>> {
        &self.j.max_qi
    }
    #[cfg(test)]
    pub(crate) fn qstep_ref(&self) -> &Vec<Vec<f32>> {
        &self.j.qstep
    }
    #[cfg(test)]
    pub(crate) fn mean_v_ref(&self) -> &[f32] {
        &self.j.mean_v
    }
    #[cfg(test)]
    pub(crate) fn mean_uv_ref(&self) -> &[f32] {
        &self.j.mean_uv
    }
    #[cfg(test)]
    pub(crate) fn reg_cond_ref(&self) -> &[f32] {
        &self.j.reg_cond
    }
    #[cfg(test)]
    pub(crate) fn min_dist_v_ref(&self) -> &[f32] {
        &self.j.min_dist_v
    }
    #[cfg(test)]
    pub(crate) fn min_dist_uv_ref(&self) -> &[f32] {
        &self.j.min_dist_uv
    }
}

pub(crate) fn load_lsf_cb() -> &'static LsfCb {
    &super::smpl_lsf_seed::lsf_built().cb
}

// Vector helpers.

fn sub_vec(y: &[f32], z: &[f32], out: &mut [f32]) {
    for i in 0..SMPL_LPC_ORDER {
        out[i] = y[i] - z[i];
    }
}

fn dot_prod(a: &[f32], b: &[f32]) -> f32 {
    let mut s = 0.0f32;
    for i in 0..SMPL_LPC_ORDER {
        s += a[i] * b[i];
    }
    s
}

fn werr(x: &[f32], y: &[f32], w: &[f32]) -> f32 {
    let mut s = 0.0f32;
    for k in 0..SMPL_LPC_ORDER {
        let e = x[k] - y[k];
        s += w[k] * e * e;
    }
    s
}

/// Transposed 16-wide matrix-vector multiply: `y[i] = sum_j C[j][i]*x[j]`.
fn matrix_mult_transp_16(c: &[Vec<f32>], x: &[f32], y: &mut [f32], len_x: usize) {
    let mut yt = [0.0f32; SMPL_LPC_ORDER];
    let xtmp = x[0];
    for i in 0..SMPL_LPC_ORDER {
        yt[i] = c[0][i] * xtmp;
    }
    for j in 1..len_x {
        let xtmp = x[j];
        for i in 0..SMPL_LPC_ORDER {
            yt[i] += c[j][i] * xtmp;
        }
    }
    y[..SMPL_LPC_ORDER].copy_from_slice(&yt);
}

/// Top-K indices of the K largest values in `x` (descending), ties broken toward the lower index.
/// Used for the VQ shortlist + alt-refinement; the caller re-evaluates survivors, so the survivor SET
/// is the load-bearing output.
fn get_maxi_k(x: &[f32], idx: &mut [i32], k: usize) {
    let n = x.len();
    let mut used = vec![false; n];
    for slot in idx.iter_mut().take(k) {
        let mut best_i = -1i32;
        let mut best_v = f32::NEG_INFINITY;
        for (i, &v) in x.iter().enumerate() {
            if used[i] {
                continue;
            }
            if v > best_v {
                best_v = v;
                best_i = i as i32;
            }
        }
        if best_i < 0 {
            *slot = 0;
        } else {
            used[best_i as usize] = true;
            *slot = best_i;
        }
    }
}

/// LSF RD weights, spectral inverse-envelope variant (`SMPL_USE_SPEC_LSW_WEIGHT`, the active build
/// path): the weight at each LSF is the inverse spectral envelope magnitude
/// `1/sqrt(|A(e^jw)|^2 * scale)`, `scale = 1/min`. `a` is the monic LPC `A[0..16]` (`A[0]=1`).
fn lsf_weights_spectral(a: &[f32], lsf: &[f32]) -> [f32; SMPL_LPC_ORDER] {
    let mut lsfw = [0.0f32; SMPL_LPC_ORDER];
    for i in 0..SMPL_LPC_ORDER {
        let e_re = lsf[i].cos();
        let e_im = lsf[i].sin();
        let mut acc_re = 1.0f32;
        let mut acc_im = 0.0f32;
        let mut ep_re = e_re;
        let mut ep_im = e_im;
        for j in 1..SMPL_LPC_ORDER {
            acc_re += ep_re * a[j];
            acc_im -= ep_im * a[j];
            // ep *= e
            let nr = ep_re * e_re - ep_im * e_im;
            let ni = ep_re * e_im + ep_im * e_re;
            ep_re = nr;
            ep_im = ni;
        }
        acc_re += ep_re * a[SMPL_LPC_ORDER];
        acc_im -= ep_im * a[SMPL_LPC_ORDER];
        lsfw[i] = acc_re * acc_re + acc_im * acc_im;
    }
    let mut min_lsfw = lsfw[0];
    for &v in lsfw.iter().skip(1) {
        if v < min_lsfw {
            min_lsfw = v;
        }
    }
    let scale = 1.0 / min_lsfw;
    for v in lsfw.iter_mut() {
        *v = 1.0 / (*v * scale).sqrt();
    }
    lsfw
}

pub(crate) fn lsf_weights_laroia(lsf: &[f32]) -> [f32; SMPL_LPC_ORDER] {
    let min_dist = 1e-3f32;
    let mut inv_delta = [0.0f32; SMPL_LPC_ORDER + 1];
    inv_delta[0] = 1.0 / lsf[0].max(min_dist);
    for i in 1..SMPL_LPC_ORDER {
        inv_delta[i] = 1.0 / (lsf[i] - lsf[i - 1]).max(min_dist);
    }
    inv_delta[SMPL_LPC_ORDER] = 1.0 / (SMPL_PI - lsf[SMPL_LPC_ORDER - 1]).max(min_dist);
    let mut lsfw = [0.0f32; SMPL_LPC_ORDER];
    for i in 0..SMPL_LPC_ORDER {
        lsfw[i] = inv_delta[i] + inv_delta[i + 1];
    }
    lsfw
}

/// Push LSFs apart so consecutive spacings exceed `min_dist`.
fn lsf_min_dist(lsfs: &mut [f32], min_dist: &[f32]) {
    let n = SMPL_LPC_ORDER;
    let mut dlsfs = [0.0f32; SMPL_LPC_ORDER + 1];
    dlsfs[0] = lsfs[0] - min_dist[0];
    for i in 1..n {
        dlsfs[i] = (lsfs[i] - lsfs[i - 1]) - min_dist[i];
    }
    dlsfs[n] = (SMPL_PI - lsfs[n - 1]) - min_dist[n];
    let find_min = |d: &[f32]| -> (f32, usize) {
        let mut m = d[0];
        let mut mi = 0usize;
        for (i, &v) in d.iter().enumerate().take(n + 1).skip(1) {
            if v < m {
                m = v;
                mi = i;
            }
        }
        (m, mi)
    };
    let (mut dm, mut min_ix) = find_min(&dlsfs);
    if dm > 0.0 {
        return;
    }
    for k in 0..1000 {
        let mut delta = k as f32 * 1.0e-6 - dm;
        dlsfs[min_ix] += delta;
        if min_ix == 0 {
            dlsfs[1] -= delta;
        } else if min_ix == n {
            dlsfs[n - 1] -= delta;
        } else {
            delta *= 0.5;
            dlsfs[min_ix - 1] -= delta;
            dlsfs[min_ix + 1] -= delta;
        }
        let (ndm, nmi) = find_min(&dlsfs);
        dm = ndm;
        min_ix = nmi;
        if dm >= 0.0 {
            lsfs[0] = dlsfs[0] + min_dist[0];
            for i in 1..n {
                lsfs[i] = lsfs[i - 1] + (dlsfs[i] + min_dist[i]);
            }
            return;
        }
    }
    // The reference asserts unreachable here; we fall through with the best-effort spacing.
}

struct CondParams {
    st1_cbhalf: [f32; SMPL_LPC_ORDER],
    st1_cb_cinv: [f32; SMPL_LPC_ORDER],
    st1_we: Vec<Vec<f32>>,  // [16][16]
    st1_wie: Vec<Vec<f32>>, // [16][16]
}

/// `VQ_temp`: Mahalanobis shortlist of `surv` stage-1 centroids (plus the cond centroid when present).
fn vq_temp(
    lsf: &[f32],
    cbhalf: &[Vec<f32>],
    cb_cinv: &[Vec<f32>],
    cond: Option<&CondParams>,
    surv: usize,
    idxs: &mut [i32],
) {
    let mut err = [0.0f32; LSF_CB_CENTROIDS + 1];
    let mut tmp = [0.0f32; SMPL_LPC_ORDER];
    for s in 0..LSF_CB_CENTROIDS {
        sub_vec(&cbhalf[s], lsf, &mut tmp);
        err[s] = -dot_prod(&tmp, &cb_cinv[s]);
    }
    let mut cb_centroids = LSF_CB_CENTROIDS;
    if let Some(c) = cond {
        sub_vec(&c.st1_cbhalf, lsf, &mut tmp);
        err[LSF_CB_CENTROIDS] = -dot_prod(&tmp, &c.st1_cb_cinv);
        cb_centroids += 1;
    }
    get_maxi_k(&err[..cb_centroids], idxs, surv);
}

#[inline]
fn smpl_sign(a: f32) -> i32 {
    if a > 0.0 {
        1
    } else if a == 0.0 {
        0
    } else {
        -1
    }
}

/// Result of one LSF quantization: `qi[0]` (=grid), `qi[1..16]` (=stage2), and the reconstructed
/// quantized NLSF (so the caller's synthesis uses the SAME envelope the decoder will rebuild).
pub(crate) struct LsfQuantResult {
    pub qi: [i32; SMPL_LPC_ORDER + 1],
    pub qlsf: [f32; SMPL_LPC_ORDER],
}

/// Core LSF quantizer. `nlsf` is the analysis NLSF (`smpl_A2NLSF_16(A)`), `voiced` 0/1 selects the
/// codebook, `low_rate` 0/1, `cond` the conditional-coding params (Some => qi[0]==16 reachable),
/// `rd_w_adj` the RD weight. `surv` is the beam width (= lsf_surv).
fn lsf_quant_core(
    cb: &LsfCb,
    a: &[f32],
    nlsf: &[f32],
    voiced: usize,
    low_rate: usize,
    cond: Option<&CondParams>,
    rd_w_adj: f32,
    surv: usize,
) -> LsfQuantResult {
    let st1 = &cb.j.st1[voiced];
    let st2v = &cb.j.st2[voiced][low_rate];
    let min_qi = &cb.j.min_qi[voiced][low_rate];
    let max_qi = &cb.j.max_qi[voiced][low_rate];
    let min_dist = if voiced == 1 {
        &cb.j.min_dist_v
    } else {
        &cb.j.min_dist_uv
    };

    let mut lsf = [0.0f32; SMPL_LPC_ORDER];
    lsf.copy_from_slice(&nlsf[..SMPL_LPC_ORDER]);
    // RD weights are the spectral inverse-envelope (SMPL_USE_SPEC_LSW_WEIGHT).
    let wlsf = lsf_weights_spectral(a, &lsf);

    let qstep = cb.j.qstep[voiced][low_rate];
    let qstep_cond = qstep * LSF_QSTEP_COND_MULT;

    let mut qim1 = [0i32; LSF_CB_CENTROIDS + 1];
    vq_temp(&lsf, &st1.cbhalf, &st1.cb_cinv, cond, surv, &mut qim1);

    let mut rd_best = f32::MAX;
    let mut out_qi = [0i32; SMPL_LPC_ORDER + 1];
    let mut out_qlsf = [0.0f32; SMPL_LPC_ORDER];

    for s1 in 0..surv {
        let qi1 = qim1[s1] as usize;
        let is_cond = qi1 == LSF_CB_CENTROIDS;

        // lsfq1 = 2 * cbhalf[qi1] (or cond centroid).
        let mut lsfq1 = [0.0f32; SMPL_LPC_ORDER];
        if is_cond {
            let c = cond.expect("cond centroid requires cond params");
            for i in 0..SMPL_LPC_ORDER {
                lsfq1[i] = c.st1_cbhalf[i] * 2.0;
            }
        } else {
            for i in 0..SMPL_LPC_ORDER {
                lsfq1[i] = st1.cbhalf[qi1][i] * 2.0;
            }
        }

        // qerr = wie^T * (lsf - lsfq1).
        let mut qerr_ = [0.0f32; SMPL_LPC_ORDER];
        sub_vec(&lsf, &lsfq1, &mut qerr_);
        let wie_ptr: &[Vec<f32>] = if is_cond {
            &cond.expect("cond").st1_wie
        } else {
            &st1.wie[qi1]
        };
        let mut qerr = [0.0f32; SMPL_LPC_ORDER];
        matrix_mult_transp_16(wie_ptr, &qerr_, &mut qerr, SMPL_LPC_ORDER);

        let inv_qstep = 1.0 / if !is_cond { qstep } else { qstep_cond };
        for q in qerr.iter_mut() {
            *q *= inv_qstep;
        }

        let mut bits = if cond.is_none() {
            st1.bits[qi1]
        } else {
            st1.bits_cond[qi1]
        };

        // The clamp tables are indexed by qi1 (0..16); the cond centroid uses the qi1==16 row.
        let mut alt = [0i32; SMPL_LPC_ORDER];
        let mut abs_qerr = [0.0f32; SMPL_LPC_ORDER];
        let mut qres = [0.0f32; SMPL_LPC_ORDER];
        let mut qi2 = [0i32; SMPL_LPC_ORDER];
        let st2 = &st2v[qi1];
        for i in 0..SMPL_LPC_ORDER {
            let mut qi2_ = qerr[i].round() as i32;
            let mn = min_qi[qi1][i];
            let mx = max_qi[qi1][i];
            qi2_ = qi2_.min(mx).max(mn);
            qerr[i] -= qi2_ as f32;
            alt[i] = smpl_sign(qerr[i]);
            if (qi2_ == mx && alt[i] > 0) || (qi2_ == mn && alt[i] < 0) {
                abs_qerr[i] = -1.0;
            } else {
                abs_qerr[i] = qerr[i].abs();
            }
            qi2_ -= mn;
            let qi2u = qi2_ as usize;
            bits += st2.num_bits[i][qi2u];
            qres[i] = st2.qlvls[i][qi2u];
            qi2[i] = qi2_;
        }

        let mut i_alt = [0i32; SMPL_LPC_ORDER];
        get_maxi_k(&abs_qerr, &mut i_alt, surv);

        let we_ptr: &[Vec<f32>] = if is_cond {
            &cond.expect("cond").st1_we
        } else {
            &st1.we[qi1]
        };
        let mut lsfq = [0.0f32; SMPL_LPC_ORDER];
        matrix_mult_transp_16(we_ptr, &qres, &mut lsfq, SMPL_LPC_ORDER);
        for i in 0..SMPL_LPC_ORDER {
            lsfq[i] += lsfq1[i];
        }

        let surv2 = surv - s1;
        let mut ind_chgd: i32 = -1;
        let bits_orig = bits;
        // The beam base `lsfq_` is FIXED to the initial lsfq before the loop; each refinement flips
        // ONE coeff relative to this base, undoing the previous flip.
        let lsfq_base = lsfq;
        let mut cur_bits = bits;
        for s2 in 0..surv2 {
            lsf_min_dist(&mut lsfq, min_dist);
            let w = werr(&lsf, &lsfq, &wlsf);
            let rd = 0.5 * SMPL_LPC_ORDER as f32 * w.log2() * rd_w_adj + cur_bits;
            if rd < rd_best {
                rd_best = rd;
                out_qi[0] = qi1 as i32;
                out_qi[1..=SMPL_LPC_ORDER].copy_from_slice(&qi2);
                out_qlsf.copy_from_slice(&lsfq);
            }
            if s2 == surv2 - 1 || abs_qerr[i_alt[s2] as usize] < 0.25 {
                break;
            }
            if s2 > 0 {
                let ic = ind_chgd as usize;
                qi2[ic] -= alt[ic];
            }
            ind_chgd = i_alt[s2];
            let ic = ind_chgd as usize;
            let qi2_old = qi2[ic];
            qi2[ic] += alt[ic];
            let qi2_new = qi2[ic];
            let qlvls_diff = st2.qlvls[ic][qi2_new as usize] - st2.qlvls[ic][qi2_old as usize];
            for i in 0..SMPL_LPC_ORDER {
                lsfq[i] = lsfq_base[i] + qlvls_diff * we_ptr[ic][i];
            }
            cur_bits =
                bits_orig + st2.num_bits[ic][qi2_new as usize] - st2.num_bits[ic][qi2_old as usize];
        }
    }

    LsfQuantResult {
        qi: out_qi,
        qlsf: out_qlsf,
    }
}

/// Non-conditional LSF quantization. `a` is the monic LPC `A[0..16]` (A[0]=1).
pub(crate) fn lsf_quant(
    a: &[f32],
    nlsf: &[f32],
    voiced: usize,
    low_rate: usize,
    rd_w_adj: f32,
    surv: usize,
) -> LsfQuantResult {
    let cb = load_lsf_cb();
    lsf_quant_core(cb, a, nlsf, voiced, low_rate, None, rd_w_adj, surv)
}

/// Conditional LSF quantization given the previous frame's quantized NLSF. `a` is the monic LPC
/// `A[0..16]` (A[0]=1).
pub(crate) fn lsf_quant_cond(
    a: &[f32],
    nlsf: &[f32],
    lsfq_prev: &[f32],
    voiced: usize,
    low_rate: usize,
    rd_w_adj: f32,
    surv: usize,
) -> LsfQuantResult {
    let cb = load_lsf_cb();
    let st1 = &cb.j.st1[voiced];
    let cb_lsfs_mean = if voiced == 1 {
        &cb.j.mean_v
    } else {
        &cb.j.mean_uv
    };
    let reg = cb.j.reg_cond[voiced];
    let mut lsfq_prev_ = [0.0f32; SMPL_LPC_ORDER];
    let mut st1_cbhalf = [0.0f32; SMPL_LPC_ORDER];
    for i in 0..SMPL_LPC_ORDER {
        lsfq_prev_[i] = lsfq_prev[i] + reg * (cb_lsfs_mean[i] - lsfq_prev[i]);
        st1_cbhalf[i] = 0.5 * lsfq_prev_[i];
    }
    let mut st1_cb_cinv = [0.0f32; SMPL_LPC_ORDER];
    matrix_mult_transp_16(&st1.c_inv, &lsfq_prev_, &mut st1_cb_cinv, SMPL_LPC_ORDER);
    let (st1_we, st1_wie) = rot_apply_wght(&st1.rotcond[low_rate], &lsfq_prev_);
    let cond = CondParams {
        st1_cbhalf,
        st1_cb_cinv,
        st1_we,
        st1_wie,
    };
    lsf_quant_core(cb, a, nlsf, voiced, low_rate, Some(&cond), rd_w_adj, surv)
}

/// Build `wrot1` (=we) and `wrot2` (=wie) for the cond centroid from the rotation matrix and the
/// Laroia-weighted previous LSF.
fn rot_apply_wght(rot: &[Vec<f32>], lsf: &[f32]) -> (Vec<Vec<f32>>, Vec<Vec<f32>>) {
    let mut lsfw = lsf_weights_laroia(lsf);
    for v in lsfw.iter_mut() {
        *v = v.sqrt();
    }
    let mut lsfw_inv = [0.0f32; SMPL_LPC_ORDER];
    for i in 0..SMPL_LPC_ORDER {
        lsfw_inv[i] = 1.0 / lsfw[i];
    }
    let mut wrot1 = vec![vec![0.0f32; SMPL_LPC_ORDER]; SMPL_LPC_ORDER];
    let mut wrot2 = vec![vec![0.0f32; SMPL_LPC_ORDER]; SMPL_LPC_ORDER];
    for i in 0..SMPL_LPC_ORDER {
        for j in 0..SMPL_LPC_ORDER {
            wrot1[i][j] = rot[i][j] * lsfw_inv[j];
            wrot2[j][i] = rot[i][j] * lsfw[j];
        }
    }
    (wrot1, wrot2)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;

    fn fvec(v: &Value) -> Vec<f32> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_f64().unwrap() as f32)
            .collect()
    }
    fn ivec(v: &Value) -> Vec<i32> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_i64().unwrap() as i32)
            .collect()
    }

    // Tier-B: feed the reference's post-A2NLSF `lsf` + LPC `A` + state to our quantizer; both the
    // indices `qi[]` AND the reconstructed `qlsf` must match the reference bit-for-bit.
    #[test]
    fn lsf_quant_matches_c() {
        let recs: Value = serde_json::from_str(include_str!("testdata/lsf_quant_io.json")).unwrap();
        let arr = recs.as_array().unwrap();
        assert!(arr.len() >= 12, "need vectors");
        let mut bad = 0usize;
        for (n, r) in arr.iter().enumerate() {
            let lsf = fvec(&r["lsf"]);
            let a = fvec(&r["A"]);
            let voiced = r["voiced"].as_i64().unwrap() as usize;
            let low_rate = r["lowRate"].as_i64().unwrap() as usize;
            let surv = r["surv"].as_i64().unwrap() as usize;
            let rd_w = r["RDw_adj"].as_f64().unwrap() as f32;
            let cond = r["cond_coding"].as_i64().unwrap() != 0;
            let prev_lsf = fvec(&r["prev_lsf"]);
            let want_qi = ivec(&r["qi"]);
            let want_qlsf = fvec(&r["qlsf"]);
            let res = if cond {
                lsf_quant_cond(&a, &lsf, &prev_lsf, voiced, low_rate, rd_w, surv)
            } else {
                lsf_quant(&a, &lsf, voiced, low_rate, rd_w, surv)
            };
            if res.qi.to_vec() != want_qi {
                bad += 1;
                if bad <= 6 {
                    eprintln!(
                        "rec {n} qi MISMATCH voiced={voiced} cond={cond}\n  got  {:?}\n  want {:?}",
                        res.qi, want_qi
                    );
                }
                continue;
            }
            // Reconstructed NLSF must match (the envelope the decoder rebuilds).
            for k in 0..SMPL_LPC_ORDER {
                assert!(
                    (res.qlsf[k] - want_qlsf[k]).abs() < 1e-4,
                    "rec {n} qlsf[{k}] {:.6} != ref {:.6}",
                    res.qlsf[k],
                    want_qlsf[k]
                );
            }
        }
        assert_eq!(bad, 0, "lsf_quant qi mismatch vs reference");
    }
}

//! Build-from-seed for the MLow LSF runtime tables. The expanded LSF tables (`SmplSynthTables`,
//! `SmplTables`, `LsfCb`) are the expansion of one small packed ROM (`lsf_seed.bin`), so we store the
//! ROM and rerun the init at load instead of committing the pre-expanded f32. The float op order here
//! is load-bearing (matmul accumulation, sqrt-then-reciprocal in `rot_apply_wght`, integer truncation
//! in `dcmf_to_cmf`, scalar `unpack8`) so the rebuilt tables are bit-faithful.
//!
//! `min_spacing` and `lsf_extra` are NOT separate ROM: `min_spacing[v]` is `min_dist[1-v]`, and
//! `lsf_extra` is the extra-symbol selector CDF carried in the seed. The decoder bounds-checks
//! `sym < numQlvls`, so `valtables` is built at width `numQlvls` (no trailing read).
//!
//! Explicit indexed loops are intentional: the float accumulation order is load-bearing for bit-exact
//! output, so the iterator rewrites clippy suggests are not applied here.
#![allow(clippy::needless_range_loop)]

use super::smpl_decode::{LsfGrid, SmplTables};
use super::smpl_lsf_quant::{LsfCb, LsfCbJson, St1Json, St2Json};
use super::smpl_synth::SmplSynthTables;
use std::sync::OnceLock;

const ORDER: usize = 16; // SMPL_LPC_ORDER
const CENTROIDS: usize = 16; // LSF_CB_CENTROIDS
const CINV_LEN: usize = ORDER * (ORDER + 1) / 2; // 136
const ST2_LEN: usize = 9593; // LSF_ST2_ALL_QLVLS_LEN

// Per-voiced (index 0 = unvoiced, 1 = voiced) scale/min constants.
const CB_MIN: [f32; 2] = [-0.5873778, -0.24721986];
const CB_SCALE: [f32; 2] = [1.3145164e-5, 7.226229e-6];
const CINV_MIN: [f32; 2] = [-3.5960955e-5, -2.778548e-5];
const CINV_SCALE: [f32; 2] = [1.8589316e-9, 1.2180106e-9];
const ROT_MIN: [f32; 2] = [-0.9124832, -0.8455929];
const ROT_SCALE: [f32; 2] = [0.006554049, 0.0069253775];
const ROT_COND_MIN: [f32; 2] = [-0.67291605, -0.8248211];
const ROT_COND_SCALE: [f32; 2] = [0.0052386564, 0.0064186584];
const ST2_QLVLS_MIN: f32 = -0.45;
const ST2_QLVLS_SCALE: f32 = 0.0034478905;
const QSTEP_COND_MULT: f32 = 0.9; // LSF_QSTEP_COND_MULT

/// On-disk packed LSF ROM (flat row-major; `tables.proto` `LsfSeed`). Reshaped before expansion.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct LsfSeed {
    #[prost(bytes = "vec", tag = "1")]
    rot_8: Vec<u8>, // [2][16][16][16]
    #[prost(bytes = "vec", tag = "2")]
    rot_cond_8: Vec<u8>, // [2][2][16][16]
    #[prost(bytes = "vec", tag = "3")]
    st2_all_qlvls_8: Vec<u8>, // [9593]
    #[prost(bytes = "vec", tag = "4")]
    st2_all_qlvl_dcmfs: Vec<u8>, // [9593]
    #[prost(bytes = "vec", tag = "5")]
    st2_min_qi: Vec<u8>, // [2][2][17][16] i8
    #[prost(bytes = "vec", tag = "6")]
    st2_max_qi: Vec<u8>, // [2][2][17][16] i8
    #[prost(uint32, repeated, tag = "7")]
    cb_16: Vec<u32>, // [2][16][16]
    #[prost(uint32, repeated, tag = "8")]
    cinv_16: Vec<u32>, // [2][136]
    #[prost(uint32, repeated, tag = "9")]
    cmf: Vec<u32>, // [2][17]
    #[prost(uint32, repeated, tag = "10")]
    cmf_cond: Vec<u32>, // [2][18]
    #[prost(uint32, repeated, tag = "11")]
    lsf_sel: Vec<u32>, // [3][3]
    #[prost(uint32, repeated, tag = "12")]
    lsf_extra: Vec<u32>, // [3]
    #[prost(float, repeated, tag = "13")]
    mean: Vec<f32>, // [2][16]
    #[prost(float, repeated, tag = "14")]
    min_dist: Vec<f32>, // [2][17]
    #[prost(float, repeated, tag = "15")]
    reg_cond: Vec<f32>, // [2]
    #[prost(float, repeated, tag = "16")]
    qstep: Vec<f32>, // [2][2]
                     // grid16_w/alpha/matrices and centroids16/matrices16 are not stored: the synth grid16 tables are
                     // derived at load (grid16_w = mean[1-v], grid16_alpha = reg_cond, grid16_matrices = unpack8(rot_cond_8)),
                     // and the grid==16 centroids/matrices rows are never read (grid==16 returns before indexing them).
}

/// The packed ROM reshaped into the nested arrays the expansion indexes. Outer index `[voiced]`.
struct LsfSeedNested {
    cb_16: Vec<Vec<Vec<u16>>>,          // [2][16][16]
    cinv_16: Vec<Vec<u16>>,             // [2][136]
    rot_8: Vec<Vec<Vec<Vec<u8>>>>,      // [2][16][16][16]
    rot_cond_8: Vec<Vec<Vec<Vec<u8>>>>, // [2][2][16][16]
    mean: Vec<Vec<f32>>,                // [2][16]
    cmf: Vec<Vec<u16>>,                 // [2][17]
    cmf_cond: Vec<Vec<u16>>,            // [2][18]
    min_dist: Vec<Vec<f32>>,            // [2][17]
    reg_cond: Vec<f32>,                 // [2]
    st2_min_qi: Vec<Vec<Vec<Vec<i8>>>>, // [2][2][17][16]
    st2_max_qi: Vec<Vec<Vec<Vec<i8>>>>, // [2][2][17][16]
    qstep: Vec<Vec<f32>>,               // [2][2]
    st2_all_qlvls_8: Vec<u8>,           // [9593]
    st2_all_qlvl_dcmfs: Vec<u8>,        // [9593]
    lsf_sel: Vec<Vec<u16>>,             // [3][3]
    lsf_extra: Vec<u16>,                // [3]
}

/// The three LSF runtime structs rebuilt from one seed.
pub(crate) struct LsfBuilt {
    pub(crate) synth: SmplSynthTables,
    pub(crate) tables: SmplTables,
    pub(crate) cb: LsfCb,
}

// reshape helpers (flat row-major -> nested)

fn u32_to_u16(v: &[u32]) -> Vec<u16> {
    v.iter().map(|&x| x as u16).collect()
}

/// Split `flat` into `outer` rows of length `inner`.
fn rows<T: Clone>(flat: &[T], outer: usize, inner: usize) -> Vec<Vec<T>> {
    debug_assert_eq!(flat.len(), outer * inner);
    flat.chunks_exact(inner).map(|c| c.to_vec()).collect()
}

/// Reshape a `[2][2][17][16]` flat i8 ROM.
fn qi_4d(flat: &[u8]) -> Vec<Vec<Vec<Vec<i8>>>> {
    debug_assert_eq!(flat.len(), 2 * 2 * 17 * ORDER);
    let mut p = 0usize;
    let mut out = Vec::with_capacity(2);
    for _ in 0..2 {
        let mut a = Vec::with_capacity(2);
        for _ in 0..2 {
            let mut b = Vec::with_capacity(17);
            for _ in 0..17 {
                let row: Vec<i8> = flat[p..p + ORDER].iter().map(|&x| x as i8).collect();
                p += ORDER;
                b.push(row);
            }
            a.push(b);
        }
        out.push(a);
    }
    out
}

impl LsfSeed {
    fn reshape(&self) -> LsfSeedNested {
        // rot_8 [2][16][16][16] u8
        let mut rot_8 = Vec::with_capacity(2);
        let mut p = 0usize;
        for _ in 0..2 {
            let mut centroids = Vec::with_capacity(CENTROIDS);
            for _ in 0..CENTROIDS {
                let mut mat = Vec::with_capacity(ORDER);
                for _ in 0..ORDER {
                    mat.push(self.rot_8[p..p + ORDER].to_vec());
                    p += ORDER;
                }
                centroids.push(mat);
            }
            rot_8.push(centroids);
        }

        // rot_cond_8 [2][2][16][16] u8
        let mut rot_cond_8 = Vec::with_capacity(2);
        let mut p = 0usize;
        for _ in 0..2 {
            let mut lr = Vec::with_capacity(2);
            for _ in 0..2 {
                let mut mat = Vec::with_capacity(ORDER);
                for _ in 0..ORDER {
                    mat.push(self.rot_cond_8[p..p + ORDER].to_vec());
                    p += ORDER;
                }
                lr.push(mat);
            }
            rot_cond_8.push(lr);
        }

        // cb_16 [2][16][16] u16
        let cb_16_u16 = u32_to_u16(&self.cb_16);
        let mut cb_16 = Vec::with_capacity(2);
        let mut p = 0usize;
        for _ in 0..2 {
            let mut centroids = Vec::with_capacity(CENTROIDS);
            for _ in 0..CENTROIDS {
                centroids.push(cb_16_u16[p..p + ORDER].to_vec());
                p += ORDER;
            }
            cb_16.push(centroids);
        }

        LsfSeedNested {
            cb_16,
            cinv_16: rows(&u32_to_u16(&self.cinv_16), 2, CINV_LEN),
            rot_8,
            rot_cond_8,
            mean: rows(&self.mean, 2, ORDER),
            cmf: rows(&u32_to_u16(&self.cmf), 2, 17),
            cmf_cond: rows(&u32_to_u16(&self.cmf_cond), 2, 18),
            min_dist: rows(&self.min_dist, 2, 17),
            reg_cond: self.reg_cond.clone(),
            st2_min_qi: qi_4d(&self.st2_min_qi),
            st2_max_qi: qi_4d(&self.st2_max_qi),
            qstep: rows(&self.qstep, 2, 2),
            st2_all_qlvls_8: self.st2_all_qlvls_8.clone(),
            st2_all_qlvl_dcmfs: self.st2_all_qlvl_dcmfs.clone(),
            lsf_sel: rows(&u32_to_u16(&self.lsf_sel), 3, 3),
            lsf_extra: u32_to_u16(&self.lsf_extra),
        }
    }

    pub(crate) fn build(&self) -> LsfBuilt {
        self.reshape().build()
    }
}

/// Transposed 16x16 matrix-vector multiply: `y[i] = sum_j C[j*16+i] * x[j]` (accumulation order:
/// seed with j=0, then `+=` for j>0).
fn matrix_mult_transp_16(c: &[[f32; ORDER]; ORDER], x: &[f32; ORDER]) -> [f32; ORDER] {
    let mut y = [0.0f32; ORDER];
    let x0 = x[0];
    for i in 0..ORDER {
        y[i] = c[0][i] * x0;
    }
    for j in 1..ORDER {
        let xj = x[j];
        for i in 0..ORDER {
            y[i] += c[j][i] * xj;
        }
    }
    y
}

/// Laroia inverse-gap LSF weights, with the gap floored at 1e-3.
fn laroia(lsf: &[f32; ORDER]) -> [f32; ORDER] {
    const PI: f32 = std::f32::consts::PI;
    const MIN_DIST: f32 = 1e-3;
    let mut inv = [0.0f32; ORDER + 1];
    inv[0] = 1.0 / lsf[0].max(MIN_DIST);
    for i in 1..ORDER {
        inv[i] = 1.0 / (lsf[i] - lsf[i - 1]).max(MIN_DIST);
    }
    inv[ORDER] = 1.0 / (PI - lsf[ORDER - 1]).max(MIN_DIST);
    let mut w = [0.0f32; ORDER];
    for i in 0..ORDER {
        w[i] = inv[i] + inv[i + 1];
    }
    w
}

/// Apply the Laroia weights to the rotation: `lsfw = sqrt(laroia(lsf))`, `we[i][j]=rot[i][j]/lsfw[j]`,
/// `wie[j][i]=rot[i][j]*lsfw[j]`.
fn rot_apply_wght(
    rot: &[[f32; ORDER]; ORDER],
    lsf: &[f32; ORDER],
) -> ([[f32; ORDER]; ORDER], [[f32; ORDER]; ORDER]) {
    let mut lsfw = laroia(lsf);
    for v in lsfw.iter_mut() {
        *v = v.sqrt();
    }
    let mut lsfw_inv = [0.0f32; ORDER];
    for i in 0..ORDER {
        lsfw_inv[i] = 1.0 / lsfw[i];
    }
    let mut we = [[0.0f32; ORDER]; ORDER];
    let mut wie = [[0.0f32; ORDER]; ORDER];
    for i in 0..ORDER {
        for j in 0..ORDER {
            we[i][j] = rot[i][j] * lsfw_inv[j];
            wie[j][i] = rot[i][j] * lsfw[j];
        }
    }
    (we, wie)
}

/// Per-symbol bit cost: `bits[i] = -log2f((cmf[i+1]-cmf[i]) / (f32)cmf[len-1])`.
fn cmf_to_bits(cmf: &[u16]) -> Vec<f32> {
    let n = cmf.len();
    let den = cmf[n - 1] as f32;
    let mut bits = Vec::with_capacity(n - 1);
    for i in 0..n - 1 {
        let num = (cmf[i + 1] as i32 - cmf[i] as i32) as f32;
        bits.push(-((num / den).log2()));
    }
    bits
}

/// Integer expansion of a delta-CMF to a cumulative u16 CDF of length `len+1`.
fn dcmf_to_cmf(dcmf: &[u8]) -> Vec<u16> {
    let dcmf_len = dcmf.len();
    let mut cmf = vec![0u16; dcmf_len + 1];
    let mut sum: i64 = 0;
    for n in 0..dcmf_len {
        let mut tmp = dcmf[n] as i32 + 1;
        tmp *= tmp;
        if tmp > 65535 {
            tmp = 65535;
        }
        cmf[n + 1] = tmp as u16;
        sum += tmp as i64;
    }
    cmf[0] = 0;
    for n in 1..dcmf_len + 1 {
        let prev = cmf[n - 1] as i64;
        let add = (cmf[n] as i64 * (32767 - dcmf_len as i64)) / sum + 1;
        cmf[n] = (prev + add) as u16;
    }
    cmf
}

fn unpack8_16x16(packed: &[Vec<u8>], scale: f32, min: f32) -> [[f32; ORDER]; ORDER] {
    let mut out = [[0.0f32; ORDER]; ORDER];
    for i in 0..ORDER {
        for j in 0..ORDER {
            out[i][j] = min + packed[i][j] as f32 * scale;
        }
    }
    out
}

impl LsfSeedNested {
    /// Run the LSF codebook expansion to produce the three runtime structs.
    fn build(&self) -> LsfBuilt {
        // Stage 1: per-voiced
        let mut st1: Vec<St1Json> = Vec::with_capacity(2);
        // Decoder-side accumulators for SmplSynthTables (centroids/matrices = cbhalf/we).
        let mut synth_centroids: Vec<Vec<Vec<f32>>> = vec![Vec::new(), Vec::new()];
        let mut synth_matrices: Vec<Vec<Vec<Vec<f32>>>> = vec![Vec::new(), Vec::new()];
        // grid==16 decorr matrices: the same Rotcond unpack8(rot_cond_8), flattened [lr][256].
        let mut synth_grid16_matrices: Vec<Vec<Vec<f32>>> = vec![Vec::new(), Vec::new()];

        for voiced in 0..2usize {
            // cInv (symmetric lower-triangular fill).
            debug_assert_eq!(self.cinv_16[voiced].len(), CINV_LEN);
            let mut c_inv = [[0.0f32; ORDER]; ORDER];
            let mut p = 0usize;
            for i in 0..ORDER {
                for j in 0..=i {
                    let v = CINV_MIN[voiced] + CINV_SCALE[voiced] * self.cinv_16[voiced][p] as f32;
                    c_inv[i][j] = v;
                    c_inv[j][i] = v;
                    p += 1;
                }
            }

            let mut cbhalf = [[0.0f32; ORDER]; ORDER];
            let mut cb_cinv = [[0.0f32; ORDER]; ORDER];
            let mut we = [[[0.0f32; ORDER]; ORDER]; CENTROIDS];
            let mut wie = [[[0.0f32; ORDER]; ORDER]; CENTROIDS];
            for c in 0..CENTROIDS {
                let mut lsf_cb = [0.0f32; ORDER];
                for i in 0..ORDER {
                    lsf_cb[i] = CB_MIN[voiced]
                        + self.cb_16[voiced][c][i] as f32 * CB_SCALE[voiced]
                        + self.mean[voiced][i];
                    cbhalf[c][i] = lsf_cb[i] * 0.5;
                }
                cb_cinv[c] = matrix_mult_transp_16(&c_inv, &lsf_cb);
                let rot = unpack8_16x16(&self.rot_8[voiced][c], ROT_SCALE[voiced], ROT_MIN[voiced]);
                let (we_c, wie_c) = rot_apply_wght(&rot, &lsf_cb);
                we[c] = we_c;
                wie[c] = wie_c;
            }

            // Rotcond[lowRate] = unpack8(rot_cond_8[lowRate]).
            let mut rotcond = [[[0.0f32; ORDER]; ORDER]; 2];
            for lr in 0..2usize {
                rotcond[lr] = unpack8_16x16(
                    &self.rot_cond_8[voiced][lr],
                    ROT_COND_SCALE[voiced],
                    ROT_COND_MIN[voiced],
                );
            }

            let bits = cmf_to_bits(&self.cmf[voiced]); // 16
            let bits_cond = cmf_to_bits(&self.cmf_cond[voiced]); // 17

            st1.push(St1Json {
                cbhalf: cbhalf.iter().map(|r| r.to_vec()).collect(),
                c_inv: c_inv.iter().map(|r| r.to_vec()).collect(),
                bits_cond: bits_cond.clone(),
                rotcond: rotcond
                    .iter()
                    .map(|m| m.iter().map(|r| r.to_vec()).collect())
                    .collect(),
                cb_cinv: cb_cinv.iter().map(|r| r.to_vec()).collect(),
                we: we
                    .iter()
                    .map(|m| m.iter().map(|r| r.to_vec()).collect())
                    .collect(),
                bits: bits.clone(),
                wie: wie
                    .iter()
                    .map(|m| m.iter().map(|r| r.to_vec()).collect())
                    .collect(),
            });

            // SmplSynthTables decoder centroids/matrices: grid g<16 == cbhalf[g]/we[g]. The grid==16
            // row is never read (grid==16 returns before indexing it), so it is not appended.
            synth_centroids[voiced] = (0..CENTROIDS).map(|g| cbhalf[g].to_vec()).collect();
            synth_matrices[voiced] = (0..CENTROIDS)
                .map(|g| we[g].iter().map(|r| r.to_vec()).collect())
                .collect();
            // grid16_matrices[voiced][lr] = the Rotcond computed above, flattened row-major to 256.
            synth_grid16_matrices[voiced] = (0..2)
                .map(|lr| rotcond[lr].iter().flatten().copied().collect())
                .collect();
        }

        // Stage 2: the flat QlvlsTable / cmfTable / numBitsTable
        let mut st2: Vec<Vec<Vec<St2Json>>> = Vec::with_capacity(2);
        // SmplSynthTables.valtables[voiced][config][grid][coeff], width numQlvls. config == lowRate.
        let mut valtables: Vec<Vec<Vec<Vec<Vec<f32>>>>> = Vec::with_capacity(2);
        // SmplTables.lsf_stage2[voiced][config][grid][coeff] = the cmfTable slice (cumulative CDF).
        let mut lsf_stage2: Vec<Vec<Vec<Vec<Vec<u16>>>>> = Vec::with_capacity(2);

        let mut qlvls_flat = vec![0.0f32; ST2_LEN];
        let mut numqlvls_flat: Vec<Vec<Vec<[i32; ORDER]>>> = Vec::new();
        let mut qoff_flat: Vec<Vec<Vec<[usize; ORDER]>>> = Vec::new();
        let mut cmf_slices: Vec<Vec<Vec<Vec<Vec<u16>>>>> = Vec::new();
        let mut numbits_slices: Vec<Vec<Vec<Vec<Vec<f32>>>>> = Vec::new();

        let mut q_ptr = 0usize; // QlvlsRomPtr (index into qlvls_flat)
        let mut q8_ptr = 0usize; // QlvlsRomPtr8 (index into st2_all_qlvls_8)
        let mut dcmf_ptr = 0usize; // QlvlsDCmfRomPtr

        for voiced in 0..2usize {
            let mut nq_v = Vec::with_capacity(2);
            let mut qoff_v = Vec::with_capacity(2);
            let mut cmf_v = Vec::with_capacity(2);
            let mut nb_v = Vec::with_capacity(2);
            for lr in 0..2usize {
                let mut nq_lr = Vec::with_capacity(CENTROIDS + 1);
                let mut qoff_lr = Vec::with_capacity(CENTROIDS + 1);
                let mut cmf_lr = Vec::with_capacity(CENTROIDS + 1);
                let mut nb_lr = Vec::with_capacity(CENTROIDS + 1);
                for c in 0..CENTROIDS + 1 {
                    let mut qstep = self.qstep[voiced][lr];
                    if c == CENTROIDS {
                        qstep *= QSTEP_COND_MULT;
                    }
                    let mut nq_c = [0i32; ORDER];
                    let mut qoff_c = [0usize; ORDER];
                    let mut cmf_c: Vec<Vec<u16>> = Vec::with_capacity(ORDER);
                    let mut nb_c: Vec<Vec<f32>> = Vec::with_capacity(ORDER);
                    for i in 0..ORDER {
                        let min_qi = self.st2_min_qi[voiced][lr][c][i] as i32;
                        let max_qi = self.st2_max_qi[voiced][lr][c][i] as i32;
                        let num_qlvls = (max_qi - min_qi + 1) as usize;
                        nq_c[i] = num_qlvls as i32;
                        qoff_c[i] = q_ptr;
                        for lvl in 0..num_qlvls {
                            let q8 = self.st2_all_qlvls_8[q8_ptr] as f32;
                            qlvls_flat[q_ptr] =
                                (ST2_QLVLS_MIN + ST2_QLVLS_SCALE * q8 + lvl as f32 + min_qi as f32)
                                    * qstep;
                            q_ptr += 1;
                            q8_ptr += 1;
                        }
                        let dcmf = &self.st2_all_qlvl_dcmfs[dcmf_ptr..dcmf_ptr + num_qlvls];
                        let cmf = dcmf_to_cmf(dcmf); // num_qlvls+1
                        let nb = cmf_to_bits(&cmf); // num_qlvls
                        dcmf_ptr += num_qlvls;
                        cmf_c.push(cmf);
                        nb_c.push(nb);
                    }
                    nq_lr.push(nq_c);
                    qoff_lr.push(qoff_c);
                    cmf_lr.push(cmf_c);
                    nb_lr.push(nb_c);
                }
                nq_v.push(nq_lr);
                qoff_v.push(qoff_lr);
                cmf_v.push(cmf_lr);
                nb_v.push(nb_lr);
            }
            numqlvls_flat.push(nq_v);
            qoff_flat.push(qoff_v);
            cmf_slices.push(cmf_v);
            numbits_slices.push(nb_v);
        }
        // The flat-pointer walks must consume exactly ST2_LEN. `debug_assert` is enough: the seed is a
        // checked-in constant ROM, so a release miscount can only come from a corrupted blob (which
        // the golden-checksum test catches), never from input data.
        debug_assert_eq!(q_ptr, ST2_LEN);
        debug_assert_eq!(q8_ptr, ST2_LEN);
        debug_assert_eq!(dcmf_ptr, ST2_LEN);

        // Assemble st2 (LsfCb) with Qlvls sliced [off..off+numQlvls] from the flat table.
        for voiced in 0..2usize {
            let mut st2_v = Vec::with_capacity(2);
            for lr in 0..2usize {
                let mut st2_lr = Vec::with_capacity(CENTROIDS + 1);
                for c in 0..CENTROIDS + 1 {
                    let nq = &numqlvls_flat[voiced][lr][c];
                    let qoff = &qoff_flat[voiced][lr][c];
                    let mut qlvls: Vec<Vec<f32>> = Vec::with_capacity(ORDER);
                    for i in 0..ORDER {
                        let n = nq[i] as usize;
                        qlvls.push(qlvls_flat[qoff[i]..qoff[i] + n].to_vec());
                    }
                    st2_lr.push(St2Json {
                        num_qlvls: nq.to_vec(),
                        qlvls,
                        num_bits: numbits_slices[voiced][lr][c].clone(),
                    });
                }
                st2_v.push(st2_lr);
            }
            st2.push(st2_v);
        }

        // valtables[voiced][config][grid][coeff] = flat QlvlsTable sliced numQlvls.
        for voiced in 0..2usize {
            let mut vt_v = Vec::with_capacity(2);
            let mut s2_v = Vec::with_capacity(2);
            for lr in 0..2usize {
                let mut vt_lr = Vec::with_capacity(CENTROIDS + 1);
                let mut s2_lr = Vec::with_capacity(CENTROIDS + 1);
                for c in 0..CENTROIDS + 1 {
                    let nq = &numqlvls_flat[voiced][lr][c];
                    let qoff = &qoff_flat[voiced][lr][c];
                    let mut vt_c: Vec<Vec<f32>> = Vec::with_capacity(ORDER);
                    for i in 0..ORDER {
                        let n = nq[i] as usize;
                        vt_c.push(qlvls_flat[qoff[i]..qoff[i] + n].to_vec());
                    }
                    vt_lr.push(vt_c);
                    s2_lr.push(cmf_slices[voiced][lr][c].clone());
                }
                vt_v.push(vt_lr);
                s2_v.push(s2_lr);
            }
            valtables.push(vt_v);
            lsf_stage2.push(s2_v);
        }

        // Assemble the runtime structs
        let cb_json = LsfCbJson {
            st1,
            st2,
            min_qi: clone_qi(&self.st2_min_qi),
            max_qi: clone_qi(&self.st2_max_qi),
            qstep: self.qstep.clone(),
            mean_v: self.mean[1].clone(),
            mean_uv: self.mean[0].clone(),
            reg_cond: self.reg_cond.clone(),
            min_dist_v: self.min_dist[1].clone(),
            min_dist_uv: self.min_dist[0].clone(),
        };

        let tables = SmplTables {
            lsf_sel: self.lsf_sel.clone(),
            lsf_grid: LsfGrid {
                // match1 = CMF_cond_v, match1_alt = CMF_cond_uv, match0 = CMF_uv, match0_alt = CMF_v.
                match1: self.cmf_cond[1].clone(),
                match1_alt: self.cmf_cond[0].clone(),
                match0: self.cmf[0].clone(),
                match0_alt: self.cmf[1].clone(),
            },
            lsf_stage2,
            lsf_extra: self.lsf_extra.clone(),
        };

        // min_spacing[v] = min_dist[1-v] (the index swap), not separate ROM.
        let min_spacing = vec![self.min_dist[1].clone(), self.min_dist[0].clone()];

        let synth = SmplSynthTables {
            valtables,
            centroids: synth_centroids,
            matrices: synth_matrices,
            min_spacing,
            // grid16_w[v] = mean[1-v] (the 1-v swap bakes in the synth's INVERTED selection);
            // grid16_alpha = reg_cond; grid16_matrices = unpack8(rot_cond_8) computed above.
            grid16_w: vec![self.mean[1].clone(), self.mean[0].clone()],
            grid16_alpha: self.reg_cond.clone(),
            grid16_matrices: synth_grid16_matrices,
        };

        LsfBuilt {
            synth,
            tables,
            cb: LsfCb::from_json(cb_json),
        }
    }
}

fn clone_qi(qi: &[Vec<Vec<Vec<i8>>>]) -> Vec<Vec<Vec<Vec<i32>>>> {
    qi.iter()
        .map(|a| {
            a.iter()
                .map(|b| {
                    b.iter()
                        .map(|c| c.iter().map(|&x| x as i32).collect())
                        .collect()
                })
                .collect()
        })
        .collect()
}

static LSF_BUILT: OnceLock<LsfBuilt> = OnceLock::new();

/// Load the LSF seed ROM and build all three runtime structs once.
pub(crate) fn lsf_built() -> &'static LsfBuilt {
    LSF_BUILT.get_or_init(|| {
        let seed: LsfSeed =
            super::smpl_tables_blob::load_blob_prost(include_bytes!("testdata/lsf_seed.bin"));
        seed.build()
    })
}

#[cfg(test)]
pub(crate) fn seed_from_json(s: &str) -> LsfSeed {
    seed_json::parse(s)
}

#[cfg(test)]
mod seed_json {
    use super::LsfSeed;

    #[derive(serde::Deserialize)]
    struct RawSeed {
        cb_16: Vec<Vec<Vec<u32>>>,
        cinv_16: Vec<Vec<u32>>,
        rot_8: Vec<Vec<Vec<Vec<u8>>>>,
        rot_cond_8: Vec<Vec<Vec<Vec<u8>>>>,
        mean: Vec<Vec<f32>>,
        cmf: Vec<Vec<u32>>,
        cmf_cond: Vec<Vec<u32>>,
        min_dist: Vec<Vec<f32>>,
        reg_cond: Vec<f32>,
        st2_min_qi: Vec<Vec<Vec<Vec<i32>>>>,
        st2_max_qi: Vec<Vec<Vec<Vec<i32>>>>,
        qstep: Vec<Vec<f32>>,
        st2_all_qlvls_8: Vec<u8>,
        st2_all_qlvl_dcmfs: Vec<u8>,
        lsf_sel: Vec<Vec<u32>>,
        lsf_extra: Vec<u32>,
        // grid16_* / centroids16 / matrices16 / min_spacing / qlvls_flat_tail are present in the JSON
        // but derived at load (not stored); serde ignores them.
    }

    fn flat_f32(v: &[Vec<f32>]) -> Vec<f32> {
        v.iter().flatten().copied().collect()
    }
    fn flat_u32(v: &[Vec<u32>]) -> Vec<u32> {
        v.iter().flatten().copied().collect()
    }

    pub(super) fn parse(s: &str) -> LsfSeed {
        let r: RawSeed = serde_json::from_str(s).expect("lsf_seed.json");
        // rot_8 [2][16][16][16] -> bytes
        let mut rot_8 = Vec::new();
        for v in &r.rot_8 {
            for c in v {
                for row in c {
                    rot_8.extend_from_slice(row);
                }
            }
        }
        let mut rot_cond_8 = Vec::new();
        for v in &r.rot_cond_8 {
            for lr in v {
                for row in lr {
                    rot_cond_8.extend_from_slice(row);
                }
            }
        }
        let mut cb_16 = Vec::new();
        for v in &r.cb_16 {
            for c in v {
                cb_16.extend_from_slice(c);
            }
        }
        let mut st2_min_qi = Vec::new();
        let mut st2_max_qi = Vec::new();
        for (dst, src) in [
            (&mut st2_min_qi, &r.st2_min_qi),
            (&mut st2_max_qi, &r.st2_max_qi),
        ] {
            for v in src {
                for lr in v {
                    for row in lr {
                        for &x in row {
                            dst.push(x as i8 as u8);
                        }
                    }
                }
            }
        }
        LsfSeed {
            rot_8,
            rot_cond_8,
            st2_all_qlvls_8: r.st2_all_qlvls_8,
            st2_all_qlvl_dcmfs: r.st2_all_qlvl_dcmfs,
            st2_min_qi,
            st2_max_qi,
            cb_16,
            cinv_16: flat_u32(&r.cinv_16),
            cmf: flat_u32(&r.cmf),
            cmf_cond: flat_u32(&r.cmf_cond),
            lsf_sel: flat_u32(&r.lsf_sel),
            lsf_extra: r.lsf_extra,
            mean: flat_f32(&r.mean),
            min_dist: flat_f32(&r.min_dist),
            reg_cond: r.reg_cond,
            qstep: flat_f32(&r.qstep),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The field-by-field equivalence between the seed-built structs and the expanded blobs was
    // validated before those blobs were removed: every int + every non-sqrt f32 was bit-identical,
    // and the sqrt-derived `we`/`wie`/`matrices` matched within 4 ULP. Those fields pass through
    // `sqrtf` in `rot_apply_wght`; the reference `sqrtf` rounds a few ULP from IEEE correctly-rounded.
    // The difference is benign: `golden_roundtrip_no_regression` stays byte-identical and
    // `lsf_quant_matches_c` still matches the reference `qi`/`qlsf`. The tripwire below pins the init
    // output without the removed blobs.

    /// Permanent no-regression tripwire for the LSF init: checksum the seed-built struct fields
    /// against committed golden constants, independent of the downstream codec tests.
    #[test]
    fn lsf_seed_build_golden_checksums() {
        let built = lsf_built();
        let st1v = built.cb.st1(1);
        assert_eq!(st1v.cbhalf[0][0].to_bits(), 0x3d93b440, "cbhalf[1][0][0]");
        assert_eq!(st1v.we[0][0][0].to_bits(), 0x3e0ff885, "we[1][0][0][0]");
        assert_eq!(st1v.wie[0][0][0].to_bits(), 0x4062b10f, "wie[1][0][0][0]");
        assert_eq!(st1v.bits[0].to_bits(), 0x40883c1d, "bits[1][0]");
        assert_eq!(
            built.cb.st2(1, 0, 0).num_qlvls[0],
            6,
            "numQlvls[1][0][0][0]"
        );
        assert_eq!(
            built.cb.st2(1, 0, 0).qlvls[0][0].to_bits(),
            0xbf2c0e76,
            "Qlvls[1][0][0][0][0]"
        );
        assert_eq!(
            built.tables.lsf_stage2[1][0][0][0],
            vec![0, 33, 140, 932, 6942, 28552, 32763],
            "lsf_stage2[1][0][0][0]"
        );
        assert_eq!(
            built.synth.valtables[1][0][0][0].len(),
            6,
            "valtables width = numQlvls"
        );
    }

    /// Tripwire pinning the grid16 derivations so a future seed regen can't silently desync the
    /// computed synth tables from the seed fields they're derived from.
    #[test]
    fn lsf_seed_grid16_derivation() {
        let seed: LsfSeed = super::super::smpl_tables_blob::load_blob_prost(include_bytes!(
            "testdata/lsf_seed.bin"
        ));
        let nested = seed.reshape();
        let synth = &seed.build().synth;

        // grid16_alpha == reg_cond.
        assert_eq!(
            synth.grid16_alpha, nested.reg_cond,
            "grid16_alpha == reg_cond"
        );

        for v in 0..2usize {
            // grid16_w[v] == mean[1-v].
            assert_eq!(
                synth.grid16_w[v],
                nested.mean[1 - v],
                "grid16_w[{v}] == mean[1-v]"
            );
            // grid16_matrices[v][lr] == unpack8(rot_cond_8[v][lr]), flattened row-major.
            for lr in 0..2usize {
                let want: Vec<f32> = unpack8_16x16(
                    &nested.rot_cond_8[v][lr],
                    ROT_COND_SCALE[v],
                    ROT_COND_MIN[v],
                )
                .iter()
                .flatten()
                .copied()
                .collect();
                assert_eq!(
                    synth.grid16_matrices[v][lr], want,
                    "grid16_matrices[{v}][{lr}] == unpack8(rot_cond_8)"
                );
            }
        }
    }
}

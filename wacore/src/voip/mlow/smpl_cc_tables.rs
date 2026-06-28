//! Logical seed-built tables for the nrgres/gains (Group A/E) and LTP gain (Group C) decode, built
//! from a small DCMF seed instead of read by absolute pointer off the cc_blob heap window. The CDFs
//! are the integer `dcmf_to_cmf` expansion; the gain-reconstruction rodata is carried verbatim. The
//! pulse (Group B) and pitch lag/contour (Group D) reads still use the heap window (`SmplMem`).

use std::sync::OnceLock;

use super::smpl_celp::{SMPL_ACBGAINS_DCMF_LR, SMPL_CB_ACBGAINS_LR_Q14, dcmf_to_cmf};

// SILK fixed-point primitives (zero-ULP integer lin2log/log2lin/sigm_Q15).

/// silk_SMULBB: (int16)a * (int16)b (32-bit wrapping).
#[inline]
fn smulbb(a: i32, b: i32) -> i32 {
    (a as i16 as i32).wrapping_mul(b as i16 as i32)
}

/// silk_SMLAWB: a + ((b * (int16)c) >> 16), 64-bit intermediate.
#[inline]
fn smlawb(a: i32, b: i32, c: i32) -> i32 {
    (a as i64 + ((b as i64 * (c as i16 as i64)) >> 16)) as i32
}

/// silk_CLZ_FRAC: leading-zero count + Q7 fractional mantissa.
#[inline]
fn clz_frac(inp: i32) -> (i32, i32) {
    let lz = (inp as u32).leading_zeros() as i32;
    let frac_q7 = (inp as u32).rotate_right(((24 - lz) & 31) as u32) as i32 & 0x7f;
    (lz, frac_q7)
}

/// silk_lin2log: approximation of 128 * log2().
#[inline]
fn lin2log(in_lin: i32) -> i32 {
    let (lz, frac_q7) = clz_frac(in_lin);
    smlawb(frac_q7, frac_q7.wrapping_mul(128 - frac_q7), 179).wrapping_add((31 - lz) << 7)
}

/// silk_log2lin: approximation of 2^() (inverse of lin2log).
#[inline]
fn log2lin(in_log_q7: i32) -> i32 {
    if in_log_q7 < 0 {
        return 0;
    }
    if in_log_q7 >= 3967 {
        return i32::MAX;
    }
    let mut out = 1i32 << (in_log_q7 >> 7);
    let frac_q7 = in_log_q7 & 0x7f;
    let inner = smlawb(frac_q7, smulbb(frac_q7, 128 - frac_q7), -174);
    if in_log_q7 < 2048 {
        out = out.wrapping_add((out.wrapping_mul(inner)) >> 7);
    } else {
        out = out.wrapping_add((out >> 7).wrapping_mul(inner));
    }
    out
}

/// silk_sigm_Q15: piecewise-linear sigmoid.
#[inline]
fn sigm_q15(in_q5: i32) -> i32 {
    const SLOPE: [i32; 6] = [237, 153, 73, 30, 12, 7];
    const POS: [i32; 6] = [16384, 23955, 28861, 31213, 32178, 32548];
    const NEG: [i32; 6] = [16384, 8812, 3906, 1554, 589, 219];
    if in_q5 < 0 {
        let in_q5 = -in_q5;
        if in_q5 >= 6 * 32 {
            0
        } else {
            let ind = (in_q5 >> 5) as usize;
            NEG[ind] - smulbb(SLOPE[ind], in_q5 & 0x1f)
        }
    } else if in_q5 >= 6 * 32 {
        32767
    } else {
        let ind = (in_q5 >> 5) as usize;
        POS[ind] + smulbb(SLOPE[ind], in_q5 & 0x1f)
    }
}

// Pulse-coding table builders, all integer/deterministic.

/// `smpl_pdf_to_CMF` (maxval == -1 path): truncating-int normalize into a cumulative u16 CDF of
/// length `pdf.len() + 1`. The PDF entries here are always non-negative.
fn pdf_to_cmf(pdf: &[i32]) -> Vec<u16> {
    let n = pdf.len() as i64;
    let maxval: i64 = 32767;
    let sump: i64 = pdf.iter().map(|&x| x as i64).sum();
    debug_assert!(sump > 0);
    let mut cmf = vec![0u16; pdf.len() + 1];
    for i in 0..pdf.len() {
        let p = ((pdf[i] as i64 * (maxval - n)) / sump) + 1;
        cmf[i + 1] = (cmf[i] as i32).wrapping_add(p as i32) as u16;
    }
    cmf
}

const LOG2_EXP1_Q15: i32 = 47274;
const LOG2_2PI_Q14: i32 = 43442;

/// Stirling-approximation log-factorial term used by the split-probability model.
fn stirling(n: i32) -> i32 {
    if n == 0 {
        return 0;
    }
    let mut ret = ((n << 1) + 1).wrapping_mul(lin2log(n) << 7);
    ret = ret.wrapping_sub(LOG2_EXP1_Q15.wrapping_mul(n));
    ret = ret.wrapping_add(LOG2_2PI_Q14);
    ret.wrapping_add(LOG2_EXP1_Q15 / (12 * n))
}

/// `smpl_prob_split_fast`: 2^(stirling(N) - stirling(k) - stirling(N-k) - N), as a Q-scaled int.
fn prob_split_fast(k: i32, n: i32) -> i32 {
    let tmp = stirling(n)
        .wrapping_sub(stirling(k))
        .wrapping_sub(stirling(n - k))
        .wrapping_sub(n.wrapping_mul(1 << 15));
    if tmp == 0 {
        return 1 << 30;
    }
    let ret = log2lin((-tmp) >> 8);
    (1 << 30) / ret
}

/// `create_split_CMFs`: split_CMFs[num_pulses-1] for num_pulses in 1..SMPL_MAX_PULSES_PER_SF*4.
fn create_split_cmfs() -> Vec<Vec<u16>> {
    (1..=SPLIT_NUM_TABLES as i32)
        .map(|num_pulses| {
            let min_split = (num_pulses - SMPL_MAX_PULSES_PER_SF as i32 * 2).max(0);
            let max_split = num_pulses - min_split;
            let p: Vec<i32> = (min_split..=max_split)
                .map(|k| prob_split_fast(k, num_pulses))
                .collect();
            pdf_to_cmf(&p)
        })
        .collect()
}

const ONE_Q31: i64 = 1 << 31;

/// `create_runlen_table` for one octave bucket: the per-`nump` magnitude CDFs (full length
/// `max_samples + 1`), Q31 sigmoid/log2lin geometric model.
fn create_runlen_table(max_samples: i32) -> RunlenCmfs {
    let ms = max_samples;
    let cmfs = (1..=SMPL_MAX_PULSES_PER_SF as i32)
        .map(|nump| {
            let mut plonger_q31: i64 = ONE_Q31;
            let mut p = vec![0i32; ms as usize];
            for nums in 1..=ms {
                let tmp = ONE_Q31 - (ONE_Q31 / (ms - nums + 1) as i64);
                let mut p1_q31 = tmp;
                for _ in 0..(nump - 1) {
                    p1_q31 = (p1_q31 * tmp) >> 31;
                }
                p1_q31 = ONE_Q31 - p1_q31;
                p1_q31 = p1_q31.min(2147376274); // 0.99995
                let log_out_q7 = if nump > ms {
                    lin2log((nump << 10) / ms) - 10 * 128
                } else {
                    -(lin2log((ms << 10) / nump) - 10 * 128)
                };
                const SIGM_BIAS_Q5: i32 = 146;
                const SCALE_MAX_Q15: i32 = 36000;
                const SCALE_MIN_Q15: i32 = 26000;
                let scale_fac_q15 = SCALE_MAX_Q15
                    - (((SCALE_MAX_Q15 - SCALE_MIN_Q15)
                        * sigm_q15((log_out_q7 >> 2) + SIGM_BIAS_Q5))
                        >> 15);
                p1_q31 = ONE_Q31
                    - log2lin(
                        ((scale_fac_q15 * (lin2log((ONE_Q31 - p1_q31) as i32) - 31 * 128)) >> 15)
                            + 31 * 128,
                    ) as i64;
                p1_q31 = p1_q31.min(2147376274);
                p[(nums - 1) as usize] = ((plonger_q31 * p1_q31) >> 31) as i32;
                plonger_q31 = (plonger_q31 * (ONE_Q31 - p1_q31)) >> 31;
            }
            pdf_to_cmf(&p)
        })
        .collect();
    RunlenCmfs {
        max_samples: ms,
        cmfs,
    }
}

const NRGRES_GAIN4_N: usize = 84;
const NRGRES_SHAPE4_N: usize = 98;
const FCBG_OFFSET_STEPS: usize = 176; // dcmf len; cmf row len is +1
const FCBG_OFFSET_BUCKETS: usize = 4;
const ACBG_N: usize = 16; // acbgain cmf row len is +1
const ACBG_ROWS: usize = ACBG_N + 1;
const FCBG_V_N: usize = 34; // fcbgains_v cmf len is +1
const FCBG_V_DELTA_N: usize = 67; // fcbgains_v_delta cmf len is +1

// Pulse-coding (Group B) shape constants.
const SMPL_MAX_PULSES_PER_SF: usize = 40;
const RUNLENGTH_STEP: usize = 8;
const NUM_RUNLEN_CMFS: usize = 20; // SMPL_MAX_SF_LEN(160) / RUNLENGTH_STEP
const SPLIT_NUM_TABLES: usize = SMPL_MAX_PULSES_PER_SF * 4 - 1; // num_pulses 1..160

/// On-disk packed seed (`tables.proto` `CcSeed`). `bytes` fields reshape row-major at build.
#[derive(Clone, PartialEq, prost::Message)]
pub(crate) struct CcSeed {
    #[prost(bytes = "vec", tag = "1")]
    nrgres_gain4_dcmf: Vec<u8>, // [84]
    #[prost(bytes = "vec", tag = "2")]
    nrgres_shape4_dcmf: Vec<u8>, // [98]
    #[prost(bytes = "vec", tag = "3")]
    fcbg_offset_dcmf: Vec<u8>, // [3][4][176]
    #[prost(bytes = "vec", tag = "4")]
    acbgains_hr_dcmf: Vec<u8>, // [17][16]
    #[prost(bytes = "vec", tag = "5")]
    fcbgains_v_dcmf: Vec<u8>, // [34]
    #[prost(bytes = "vec", tag = "6")]
    fcbgains_v_delta_dcmf: Vec<u8>, // [67]
    #[prost(sint32, repeated, tag = "7")]
    acbgains_cb_hr_q14: Vec<i32>, // [16][2]
    #[prost(uint32, tag = "8")]
    gain_recon_base: u32,
    #[prost(bytes = "vec", tag = "9")]
    gain_recon: Vec<u8>, // gain-reconstruction int16 rodata
    #[prost(bytes = "vec", tag = "10")]
    n_pulses_dcmf_bgn: Vec<u8>, // [17]
    #[prost(bytes = "vec", tag = "11")]
    n_pulses_dcmf_uv: Vec<u8>, // [33]
    #[prost(bytes = "vec", tag = "12")]
    n_pulses_dcmf_v: Vec<u8>, // [33]
}

/// Runtime tables. CDFs are `u16` (the integer cmf fits 16 bits) to feed `decode_cdf`/`encode_cdf`
/// directly, byte-identical to the old heap u16 reads. The accessor takes the LOGICAL index.
pub(crate) struct CcTables {
    nrgres_gain4: Vec<u16>,       // [85]
    nrgres_shape4: Vec<u16>,      // [99]
    fcbg_offset: Vec<Vec<u16>>,   // [3*4][177]
    acbgains_hr: Vec<u16>,        // [17*17] row-major rows of 17
    acbgains_lr: Vec<u16>,        // [17*17] LR variant (p6!=0), built from the LR DCMF const
    fcbgains_v: Vec<u16>,         // [35]
    fcbgains_v_delta: Vec<u16>,   // [68]
    acbgains_cb_hr_q14: Vec<i16>, // [16*2]
    acbgains_cb_lr_q14: Vec<i16>, // [16*2] LR weight table (p6!=0)
    gain_recon: Vec<i16>,         // rodata, addressed from gain_recon_base
    gain_recon_base: u32,
    // Group B (pulses): pulse-count CDFs [bgn,uv,v]; split CDFs by num_pulses-1; run-length CDFs by oct-1.
    n_pulse_cmfs: [Vec<u16>; 3],
    split_cmfs: Vec<Vec<u16>>, // [SPLIT_NUM_TABLES]; row i = split_CMFs[i] (num_pulses = i+1)
    runlen: Vec<RunlenCmfs>,   // [NUM_RUNLEN_CMFS]; row i = oct i+1
}

/// Run-length CDFs for one octave bucket (`max_samples = oct * RUNLENGTH_STEP`). `cmfs[c-1]` is the
/// full cumulative CDF for `c` pulses-left, length `max_samples + 1`.
pub(crate) struct RunlenCmfs {
    max_samples: i32,
    cmfs: Vec<Vec<u16>>, // [SMPL_MAX_PULSES_PER_SF]
}

impl CcSeed {
    fn build(&self) -> CcTables {
        let nrgres_gain4 = dcmf_to_cmf(&self.nrgres_gain4_dcmf);
        let nrgres_shape4 = dcmf_to_cmf(&self.nrgres_shape4_dcmf);
        let fcbg_offset: Vec<Vec<u16>> = self
            .fcbg_offset_dcmf
            .chunks_exact(FCBG_OFFSET_STEPS)
            .map(dcmf_to_cmf)
            .collect();
        let acbgains_hr: Vec<u16> = self
            .acbgains_hr_dcmf
            .chunks_exact(ACBG_N)
            .flat_map(dcmf_to_cmf)
            .collect();
        // LR (p6!=0) variant: same expansion from the LR DCMF const (no seed bytes).
        let acbgains_lr: Vec<u16> = SMPL_ACBGAINS_DCMF_LR
            .chunks_exact(ACBG_N)
            .flat_map(dcmf_to_cmf)
            .collect();
        let fcbgains_v = dcmf_to_cmf(&self.fcbgains_v_dcmf);
        let fcbgains_v_delta = dcmf_to_cmf(&self.fcbgains_v_delta_dcmf);
        let acbgains_cb_hr_q14: Vec<i16> =
            self.acbgains_cb_hr_q14.iter().map(|&x| x as i16).collect();
        let acbgains_cb_lr_q14: Vec<i16> = SMPL_CB_ACBGAINS_LR_Q14.to_vec();
        let gain_recon: Vec<i16> = self
            .gain_recon
            .chunks_exact(2)
            .map(|b| i16::from_le_bytes([b[0], b[1]]))
            .collect();

        debug_assert_eq!(nrgres_gain4.len(), NRGRES_GAIN4_N + 1);
        debug_assert_eq!(nrgres_shape4.len(), NRGRES_SHAPE4_N + 1);
        debug_assert_eq!(fcbg_offset.len(), 3 * FCBG_OFFSET_BUCKETS);
        debug_assert_eq!(acbgains_hr.len(), ACBG_ROWS * (ACBG_N + 1));
        debug_assert_eq!(acbgains_lr.len(), ACBG_ROWS * (ACBG_N + 1));
        debug_assert_eq!(fcbgains_v.len(), FCBG_V_N + 1);
        debug_assert_eq!(fcbgains_v_delta.len(), FCBG_V_DELTA_N + 1);

        // Group B: pulse-count from the DCMF seed, split/runlen computed from consts (no seed).
        let n_pulse_cmfs = [
            dcmf_to_cmf(&self.n_pulses_dcmf_bgn),
            dcmf_to_cmf(&self.n_pulses_dcmf_uv),
            dcmf_to_cmf(&self.n_pulses_dcmf_v),
        ];
        let split_cmfs = create_split_cmfs();
        let runlen: Vec<RunlenCmfs> = (1..=NUM_RUNLEN_CMFS as i32)
            .map(|oct| create_runlen_table(oct * RUNLENGTH_STEP as i32))
            .collect();

        debug_assert_eq!(split_cmfs.len(), SPLIT_NUM_TABLES);
        debug_assert_eq!(runlen.len(), NUM_RUNLEN_CMFS);

        CcTables {
            nrgres_gain4,
            nrgres_shape4,
            fcbg_offset,
            acbgains_hr,
            acbgains_lr,
            fcbgains_v,
            fcbgains_v_delta,
            acbgains_cb_hr_q14,
            acbgains_cb_lr_q14,
            gain_recon,
            gain_recon_base: self.gain_recon_base,
            n_pulse_cmfs,
            split_cmfs,
            runlen,
        }
    }
}

static TABLES: OnceLock<CcTables> = OnceLock::new();

pub(crate) fn load_cc_tables() -> &'static CcTables {
    TABLES.get_or_init(|| {
        let seed: CcSeed =
            super::smpl_tables_blob::load_blob_prost(include_bytes!("testdata/cc_seed.bin"));
        seed.build()
    })
}

impl CcTables {
    /// nrgres main-gain CDF (n=85).
    pub(crate) fn nrgres_gain4(&self) -> &[u16] {
        &self.nrgres_gain4
    }

    /// nrgres delta-gain / shape CDF (n=99).
    pub(crate) fn nrgres_shape4(&self) -> &[u16] {
        &self.nrgres_shape4
    }

    /// fcbg-offset CDF row `[table_ix][bucket]` sliced at `min_offset` (n=92). `min_offset` is the
    /// non-negative entry shift the old `-2*neg_part` pointer offset encoded.
    pub(crate) fn fcbg_offset(&self, table_ix: usize, bucket: usize, min_offset: usize) -> &[u16] {
        let row = &self.fcbg_offset[table_ix * FCBG_OFFSET_BUCKETS + bucket];
        &row[min_offset..min_offset + 92]
    }

    /// HR ACB-gain CDF for predictor `prev` (row `prev+1`), n=17.
    pub(crate) fn acbgain_row(&self, prev: i32) -> &[u16] {
        let r = (prev + 1) as usize;
        let base = r * (ACBG_N + 1);
        &self.acbgains_hr[base..base + ACBG_N + 1]
    }

    /// LR ACB-gain CDF for predictor `prev` (the p6!=0 variant of `acbgain_row`), n=17.
    pub(crate) fn acbgain_row_lr(&self, prev: i32) -> &[u16] {
        let r = (prev + 1) as usize;
        let base = r * (ACBG_N + 1);
        &self.acbgains_lr[base..base + ACBG_N + 1]
    }

    /// LR ACB-gain weights (w0, w2) for gain index `gi` (the p6!=0 variant).
    pub(crate) fn acbgain_weights_lr(&self, gi: i32) -> (i32, i32) {
        let i = gi as usize * 2;
        (
            self.acbgains_cb_lr_q14[i] as i32,
            self.acbgains_cb_lr_q14[i + 1] as i32,
        )
    }

    /// FCB voiced-gain CDF (prev_filt == -1), n=35.
    pub(crate) fn fcbgain_v(&self) -> &[u16] {
        &self.fcbgains_v
    }

    /// FCB voiced-gain delta CDF sliced at `[FCBG_V_N-1 - prev_filt]` (n=35).
    pub(crate) fn fcbgain_v_delta(&self, prev_filt: i32) -> &[u16] {
        let start = (FCBG_V_N as i32 - 1 - prev_filt) as usize;
        &self.fcbgains_v_delta[start..start + 35]
    }

    /// HR ACB-gain weights (w0, w2) for gain index `gi`.
    pub(crate) fn acbgain_weights(&self, gi: i32) -> (i32, i32) {
        let i = gi as usize * 2;
        (
            self.acbgains_cb_hr_q14[i] as i32,
            self.acbgains_cb_hr_q14[i + 1] as i32,
        )
    }

    /// nrg-step const for config `cfg` (gain-reconstruction base scale).
    pub(crate) fn nrg_step(&self, cfg: i32) -> i32 {
        self.gain_recon_at(self.gain_recon_base.wrapping_add((cfg as u32) * 2))
    }

    /// Unvoiced gain-reconstruction codebook entry. `p4` selects the p3==4 table; `idx` is the
    /// (possibly out-of-the-visible-array) rodata index the WASM reads as adjacent constants.
    pub(crate) fn gain_recon(&self, p4: bool, idx: i32) -> i32 {
        let base: u32 = if p4 { 0xf35f0 } else { 0xf3970 };
        self.gain_recon_at(base.wrapping_add((idx as u32) * 2))
    }

    /// Read an int16 from the gain-reconstruction rodata at WASM address `addr`. Out-of-region reads
    /// fall back to 0, matching the old heap window.
    fn gain_recon_at(&self, addr: u32) -> i32 {
        let off = addr.wrapping_sub(self.gain_recon_base) as usize;
        if off / 2 < self.gain_recon.len() && off.is_multiple_of(2) {
            self.gain_recon[off / 2] as i32
        } else {
            0
        }
    }

    /// Pulse-count CDF for table `idx` (0 = background, 1 = unvoiced, 2 = voiced). The whole CDF is
    /// the decode interval (the old heap read used the stored `cmfLen`).
    pub(crate) fn n_pulse_count(&self, idx: i32) -> &[u16] {
        &self.n_pulse_cmfs[idx as usize]
    }

    /// Split CDF base for `total` pulses (the heap `split_CMFs[total-1].cmf`). The caller windows it
    /// at its `min_split` offset; this returns the full row so out-of-range slices fall back to 0.
    pub(crate) fn split_cmf(&self, total: i32) -> &[u16] {
        let i = (total - 1) as usize;
        self.split_cmfs.get(i).map_or(&[], |v| v.as_slice())
    }

    /// Run-length CDF bucket for octave `oct` (`max_samples = oct * RUNLENGTH_STEP`).
    pub(crate) fn runlen(&self, oct: i32) -> &RunlenCmfs {
        &self.runlen[(oct - 1) as usize]
    }
}

impl RunlenCmfs {
    /// `max_samples` for this bucket (the heap stored this as the per-row base value the call site
    /// uses to shift into `cmf`).
    pub(crate) fn max_samples(&self) -> i32 {
        self.max_samples
    }

    /// Full magnitude CDF for `c` pulses-left (the heap `cmfs[c-1]`, length `max_samples + 1`). The
    /// caller windows it by `(max_samples - pos)` and reads `pos + 1` entries.
    pub(crate) fn cmf(&self, c: i32) -> &[u16] {
        &self.cmfs[(c - 1) as usize]
    }
}

#[cfg(test)]
pub(crate) fn seed_from_json(s: &str) -> CcSeed {
    #[derive(serde::Deserialize)]
    struct RawSeed {
        nrgres_gain4_dcmf: Vec<u8>,
        nrgres_shape4_dcmf: Vec<u8>,
        fcbg_offset_dcmf: Vec<u8>,
        acbgains_hr_dcmf: Vec<u8>,
        fcbgains_v_dcmf: Vec<u8>,
        fcbgains_v_delta_dcmf: Vec<u8>,
        acbgains_cb_hr_q14: Vec<i32>,
        gain_recon_base: u32,
        gain_recon: Vec<u8>,
        n_pulses_dcmf_bgn: Vec<u8>,
        n_pulses_dcmf_uv: Vec<u8>,
        n_pulses_dcmf_v: Vec<u8>,
    }
    let r: RawSeed = serde_json::from_str(s).expect("cc_seed.json");
    CcSeed {
        nrgres_gain4_dcmf: r.nrgres_gain4_dcmf,
        nrgres_shape4_dcmf: r.nrgres_shape4_dcmf,
        fcbg_offset_dcmf: r.fcbg_offset_dcmf,
        acbgains_hr_dcmf: r.acbgains_hr_dcmf,
        fcbgains_v_dcmf: r.fcbgains_v_dcmf,
        fcbgains_v_delta_dcmf: r.fcbgains_v_delta_dcmf,
        acbgains_cb_hr_q14: r.acbgains_cb_hr_q14,
        gain_recon_base: r.gain_recon_base,
        gain_recon: r.gain_recon,
        n_pulses_dcmf_bgn: r.n_pulses_dcmf_bgn,
        n_pulses_dcmf_uv: r.n_pulses_dcmf_uv,
        n_pulses_dcmf_v: r.n_pulses_dcmf_v,
    }
}

#[cfg(test)]
mod tests {
    use super::super::smpl_mem::try_load_full_heap;
    use super::*;

    /// Transitional gate: every migrated logical table must equal the old `mem.cdf_at`/`i16` heap read
    /// byte-identical. The full `smpl_cc_blob.json` dump is the oracle (the carved `.bin` only holds
    /// Group D now); skipped when the JSON is absent (CI).
    ///
    /// DOES NOT RUN IN CI (oracle gitignored); the CI guard for these tables is
    /// `lsf_seed_build_golden_checksums` + `golden_roundtrip`. A green CI does not imply this ran.
    #[test]
    fn cc_tables_byte_identical_to_heap() {
        let cc = load_cc_tables();
        let Some(mem) = try_load_full_heap() else {
            println!("smpl_cc_blob.json absent; skipping byte-identical gate");
            return;
        };
        let mem = &mem;
        let g_nrg = mem.g_nrg;
        let g_pitch = mem.g_pitch;

        // Group A: nrgres_gain4 / nrgres_shape4.
        assert_eq!(
            cc.nrgres_gain4(),
            mem.cdf_at(g_nrg.wrapping_add(0x1362), 85)
        );
        assert_eq!(
            cc.nrgres_shape4(),
            mem.cdf_at(g_nrg.wrapping_add(0x1098), 99)
        );

        // Group A: fcbg_offset[2][bucket] over all valid min_offset shifts.
        let nrg_base = g_nrg.wrapping_add(2 * 0x588);
        for bucket in 0..4 {
            let cdfp = nrg_base.wrapping_add((bucket as u32) * 0x162);
            for min_off in 0..=85usize {
                let off = cdfp.wrapping_add((min_off as u32) << 1);
                assert_eq!(
                    cc.fcbg_offset(2, bucket, min_off),
                    mem.cdf_at(off, 92),
                    "fcbg_offset bucket={bucket} min_off={min_off}"
                );
            }
        }

        // Group C: HR ACB-gain rows (the active p6==0 path reads gp+0x302).
        for prev in -1i32..16 {
            let row = (g_pitch + 0x302)
                .wrapping_add(prev.wrapping_mul(0x22) as u32)
                .wrapping_add(0x22);
            assert_eq!(
                cc.acbgain_row(prev),
                mem.cdf_at(row, 17),
                "acbgain_row {prev}"
            );
        }
        // Group C: HR ACB-gain weights at 0xe85b0.
        for gi in 0..16 {
            let w0 = mem.i16(0xe85b0u32.wrapping_add((gi as u32) * 4)) as i32;
            let w2 = mem.i16(0xe85b0u32.wrapping_add((gi as u32) * 4 + 2)) as i32;
            assert_eq!(cc.acbgain_weights(gi), (w0, w2), "acbgain_weights {gi}");
        }
        // Group C: fcbgains_v + delta. (The p6!=0 LR filter path reads these same shared tables.)
        assert_eq!(cc.fcbgain_v(), mem.cdf_at(g_pitch + 0xdc4, 35));
        for prev_filt in 0..34 {
            assert_eq!(
                cc.fcbgain_v_delta(prev_filt),
                mem.cdf_at((g_pitch + 0xe4c).wrapping_sub((prev_filt as u32) * 2), 35),
                "fcbgain_v_delta {prev_filt}"
            );
        }

        // Group C LR (p6!=0): ACB-gain rows at gp+0xc0 and weights at 0xe8460. This is the gate for
        // the low-rate pitch path, which the golden vectors may not exercise.
        for prev in -1i32..16 {
            let row = (g_pitch + 0xc0)
                .wrapping_add(prev.wrapping_mul(0x22) as u32)
                .wrapping_add(0x22);
            assert_eq!(
                cc.acbgain_row_lr(prev),
                mem.cdf_at(row, 17),
                "acbgain_row_lr {prev}"
            );
        }
        for gi in 0..16 {
            let w0 = mem.i16(0xe8460u32.wrapping_add((gi as u32) * 4)) as i32;
            let w2 = mem.i16(0xe8460u32.wrapping_add((gi as u32) * 4 + 2)) as i32;
            assert_eq!(
                cc.acbgain_weights_lr(gi),
                (w0, w2),
                "acbgain_weights_lr {gi}"
            );
        }

        // Group E: nrg-step + gain-reconstruction codebook.
        for cfg in 0..3 {
            assert_eq!(
                cc.nrg_step(cfg),
                mem.i16(0xf35e0u32.wrapping_add((cfg as u32) * 2)) as i32,
                "nrg_step {cfg}"
            );
        }
        for p4 in [true, false] {
            let base: u32 = if p4 { 0xf35f0 } else { 0xf3970 };
            for idx in 0..400 {
                assert_eq!(
                    cc.gain_recon(p4, idx),
                    mem.i16(base.wrapping_add((idx as u32) * 2)) as i32,
                    "gain_recon p4={p4} idx={idx}"
                );
            }
        }

        let g_cc = mem.g_cc;

        // Group B: pulse-count CDFs (the full stored CmfVec at g_cc+idx*8+0x11d8 -> ptr, +0x11dc -> len).
        for idx in 0..3 {
            let ent = g_cc.wrapping_add((idx as u32) * 8);
            let ptr = mem.u32(ent.wrapping_add(0x11d8));
            let n = mem.u32(ent.wrapping_add(0x11dc)) as usize;
            assert_eq!(
                cc.n_pulse_count(idx),
                mem.cdf_at(ptr, n),
                "n_pulse_count {idx}"
            );
        }

        // Group B: split CDFs split_CMFs[total-1] over all num_pulses, full row.
        for total in 1..=SPLIT_NUM_TABLES as i32 {
            let ptr = mem.u32(g_cc.wrapping_add((total as u32) * 8).wrapping_add(0xcd0));
            let min_split = (total - SMPL_MAX_PULSES_PER_SF as i32 * 2).max(0);
            let max_split = total - min_split;
            let row_len = (max_split - min_split + 2) as usize;
            assert_eq!(
                cc.split_cmf(total),
                mem.cdf_at(ptr, row_len),
                "split_cmf {total}"
            );
        }

        // Group B: run-length CDFs per (oct, c), full row + max_samples base value.
        for oct in 1..=NUM_RUNLEN_CMFS as i32 {
            let bucket = cc.runlen(oct);
            let c_base_off = mem.u32(g_cc.wrapping_add((oct as u32) * 0xa4)) as i32;
            assert_eq!(
                bucket.max_samples(),
                c_base_off,
                "runlen max_samples oct={oct}"
            );
            let row_len = (c_base_off + 1) as usize;
            for c in 1..=SMPL_MAX_PULSES_PER_SF as i32 {
                let ptr = mem.u32(
                    g_cc.wrapping_add((oct as u32) * 0xa4)
                        .wrapping_add(((c - 1) as u32) * 4)
                        .wrapping_sub(0xa0),
                );
                assert_eq!(
                    bucket.cmf(c),
                    mem.cdf_at(ptr, row_len),
                    "runlen cmf oct={oct} c={c}"
                );
            }
        }
    }
}

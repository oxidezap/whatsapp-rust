//! MLow ENTROPY ENCODER: the exact inverse of the byte-exact decoder. Given the analyzed
//! `SmplFrameParams`, it reproduces the same range-coder symbol stream the decoder consumes, against
//! the same config=0 runtime tables in the same field order. Targets the active config=0 path
//! (0x50 frames), p3=4, p4=1. Internal frames are voiced (LTP pitch block) or unvoiced (gains
//! block) per the analysis.

use super::analysis::{SmplEncoderState, smpl_analyze_frame_st};
use super::params::{
    SmplFrameParams, SmplGainParams, SmplLsfParams, SmplPitchParams, SmplPulseParams,
};
use super::rangecoder::RangeEncoder;
use super::smpl_cc_tables::{CcTables, load_cc_tables};
use super::smpl_decode::{SmplLsfState, SmplTables, load_smpl_tables};
use super::smpl_mem::{SmplMem, load_smpl_mem};
use super::smpl_pulse::mem8_static;

const SMPL_ENCODE_BUF_BYTES: usize = 512;
const OPUS_FRAME_SAMPS: usize = 960; // 60 ms @ 16 kHz

/// Why an MLow encode call failed. A consumer branches on this rather than parsing a string:
/// `FrameLength` is a caller bug (wrong PCM chunk size), `BufferOverflow` is an internal limit
/// the next frame may not hit.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum MlowError {
    /// PCM was not exactly one 60 ms frame (960 samples @ 16 kHz).
    #[error("mlow encode: expected {expected} samples (60 ms @16 kHz), got {got}")]
    FrameLength { expected: usize, got: usize },
    /// The range-coder's output buffer overflowed encoding this frame.
    #[error("mlow encode: range-encoder buffer overflow")]
    BufferOverflow,
}

/// Stateful pure-Rust MLow encoder: 60 ms PCM (960 f32 @16 kHz, ~[-1,1]) -> a wire MLow frame the
/// WhatsApp peer decodes. Emits active config=0 (`0x50`) frames, choosing voiced (LTP) or unvoiced
/// per internal frame via analysis-by-synthesis.
pub struct MlowEncoder {
    state: SmplEncoderState,
}

impl Default for MlowEncoder {
    fn default() -> Self {
        Self::new()
    }
}

impl MlowEncoder {
    pub fn new() -> Self {
        MlowEncoder {
            state: SmplEncoderState::default(),
        }
    }

    /// Clear the cross-frame analysis history (call at a stream discontinuity).
    pub fn reset(&mut self) {
        self.state = SmplEncoderState::default();
    }

    /// Encode one 60 ms frame. Expects exactly 960 samples.
    pub fn encode(&mut self, pcm: &[f32]) -> Result<Vec<u8>, MlowError> {
        if pcm.len() != OPUS_FRAME_SAMPS {
            return Err(MlowError::FrameLength {
                expected: OPUS_FRAME_SAMPS,
                got: pcm.len(),
            });
        }
        // Sanitize: NaN → 0, clamp to [-1,1] (the LPC analysis degenerates on non-finite input).
        let clean: Vec<f32> = pcm
            .iter()
            .map(|&s| if s.is_nan() { 0.0 } else { s.clamp(-1.0, 1.0) })
            .collect();
        let fp = smpl_analyze_frame_st(&mut self.state, &clean);
        encode_smpl_frame(&fp)
    }
}

/// Encode one 60 ms MLow frame from its parameters → `[TOC || range-coded body]`.
pub(crate) fn encode_smpl_frame(fp: &SmplFrameParams) -> Result<Vec<u8>, MlowError> {
    let (p2, p3, p4) = (320i32, 4i32, 1i32);
    let p6 = fp.config as i32;
    let tbl = load_smpl_tables();
    let mem = load_smpl_mem();
    let cc = load_cc_tables();
    let mut enc = RangeEncoder::new(1 + SMPL_ENCODE_BUF_BYTES);
    let mut st = SmplLsfState::default();
    for f in 0..3 {
        let ip = &fp.internal[f];
        encode_smpl_lsf(&mut enc, tbl, &mut st, fp.config, f, &ip.lsf);
        encode_smpl_pulses(&mut enc, cc, p2, p3, p4, p6, ip.lsf.stage1, &ip.pulses);
        // Voiced internal frames emit a pitch block; unvoiced emit a gains block (never both).
        if ip.lsf.stage1 == 1 {
            encode_smpl_pitch(
                &mut enc,
                mem,
                cc,
                &mut st,
                p2,
                p3,
                p6,
                ip.pulses.subfr,
                &ip.pitch,
            );
        } else {
            encode_smpl_gains(&mut enc, cc, p3, ip.pulses.subfr, &ip.gains);
        }
    }
    enc.done();
    if enc.err() != 0 {
        return Err(MlowError::BufferOverflow);
    }
    let n = enc.consumed_len();
    let body = enc.bytes();
    let mut out = Vec::with_capacity(1 + n);
    out.push(fp.toc);
    out.extend_from_slice(&body[..n]);
    Ok(out)
}

/// Inverse of `decode_smpl_lsf`: mirror the selector/grid/16-residual/extra reads, mutating `st`.
fn encode_smpl_lsf(
    enc: &mut RangeEncoder,
    t: &SmplTables,
    st: &mut SmplLsfState,
    config: usize,
    intf: usize,
    lsf: &SmplLsfParams,
) {
    let sel = if intf == 0 {
        0
    } else if st.prev_stage1 != 0 {
        2
    } else {
        1
    };
    let stage1 = lsf.stage1;
    enc.encode_cdf(stage1, &t.lsf_sel[sel]);

    let enter_match = intf != 0;
    let m = enter_match && (stage1 == st.prev_stage1);
    if !m {
        st.prev_gain_idx = -1;
        st.prev_filt_idx = -1;
        st.prev_lag = -1;
        st.prev_frac_lag = -1;
        st.prev_lagblk = -1;
        st.prev_lagidx = -1;
    }
    st.prev_stage1 = stage1;

    let grid_cdf: &[u16] = if m {
        if stage1 != 0 {
            &t.lsf_grid.match1
        } else {
            &t.lsf_grid.match1_alt
        }
    } else if stage1 != 0 {
        &t.lsf_grid.match0_alt
    } else {
        &t.lsf_grid.match0
    };
    enc.encode_cdf(lsf.grid, grid_cdf);
    st.prev_match = m;
    st.have_prev = true;

    let st2 = &t.lsf_stage2[stage1 as usize][config][lsf.grid as usize];
    for (k, c) in st2.iter().enumerate().take(16) {
        enc.encode_cdf(lsf.stage2[k], c);
    }
    enc.encode_cdf(lsf.extra, &t.lsf_extra);
}

/// Inverse of `decode_smpl_pulses` (config=0 NB count, p3=4): re-derive the count interval and the
/// split symbols from the per-subframe counts, then replay the recorded magnitude/sign symbols.
#[allow(clippy::too_many_arguments)]
fn encode_smpl_pulses(
    enc: &mut RangeEncoder,
    cc: &CcTables,
    p2: i32,
    p3: i32,
    p4: i32,
    p6: i32,
    s1: i32,
    pp: &SmplPulseParams,
) {
    let idx = p4 + s1;
    let b_byte = mem8_static(0xe8990u32.wrapping_add((p6 * 3 + idx) as u32)) as i32;
    let frame_len4k = b_byte * p2 / 320;
    let subfr_len16 = frame_len4k / p3;
    let total = pp.total;

    // pulse COUNT (NB triangular; config=0)
    let l = frame_len4k as u32;
    let tri_t = |k: u32| -> u32 {
        let a = k.wrapping_add(2).wrapping_mul(l.wrapping_add(1));
        let b = k.wrapping_sub(1).wrapping_mul(k.wrapping_add(131070)) >> 1;
        a.wrapping_sub(b) & 0xffff
    };
    let mut ft = tri_t(l);
    if ft == 0 {
        ft = 1;
    }
    let fl = if total > 0 {
        tri_t((total - 1) as u32)
    } else {
        0
    };
    let fh = tri_t(total as u32);
    enc.encode(fl, fh, ft);

    if total == 0 {
        return;
    }

    // recursive binary SPLIT
    let final_sum = pp.subfr[0] + pp.subfr[1];
    let init_sum = (total - subfr_len16 * 2).max(0);
    let lo = (total - 80).max(0);
    if init_sum < lo {
        return;
    }
    let hi_bound = total - lo;
    if init_sum < hi_bound {
        let n = ((hi_bound - init_sum) + 2) as usize;
        enc.encode_cdf_window(
            final_sum - init_sum,
            cc.split_cmf(total),
            (init_sum - lo) as usize,
            n,
        );
    }
    if final_sum > 0 {
        encode_split_3537(enc, cc, final_sum, subfr_len16, pp.subfr[0]);
    }
    if final_sum < total {
        encode_split_3537(enc, cc, total - final_sum, subfr_len16, pp.subfr[2]);
    }

    // MAGNITUDE block: replay recorded run-length symbols through the same loop
    let pos_per = p2 / p3;
    let mut mag_idx = 0usize;
    for subfr in 0..p3 {
        let cnt = pp.subfr[subfr as usize];
        if cnt <= 0 {
            continue;
        }
        let mut pos = pos_per;
        let mut c = cnt;
        let mut k = 0;
        while k < cnt {
            let oct = (pos + 7) / 8;
            let bucket = cc.runlen(oct);
            let start = (bucket.max_samples() - pos) as usize;
            let m = pp.mag_runs[mag_idx];
            mag_idx += 1;
            enc.encode_cdf_window(m, bucket.cmf(c), start, (pos + 1) as usize);
            if m > 0 || k == 0 {
                pos -= m;
            }
            c -= 1;
            k += 1;
        }
    }

    // SIGN block: replay recorded raw sign symbols
    for rs in &pp.sign_syms {
        enc.encode_raw_symbol(rs.sym, rs.nbits);
    }
}

/// Inverse of `smpl_split_3537`: encode the first-half count `s0` against the same CDF.
fn encode_split_3537(enc: &mut RangeEncoder, cc: &CcTables, count: i32, granularity: i32, s0: i32) {
    let lo = count.min(granularity);
    let min_split = (count - granularity).max(0);
    if lo < min_split || min_split == lo {
        return;
    }
    let n = ((lo - min_split) + 2) as usize;
    enc.encode_cdf_window(s0 - min_split, cc.split_cmf(count), min_split as usize, n);
}

/// Inverse of `decode_smpl_gains`: encode main/delta gain, then per-subframe nrgres with the same
/// gain-derived address shift.
fn encode_smpl_gains(
    enc: &mut RangeEncoder,
    cc: &CcTables,
    p3: i32,
    subfr_counts: [i32; 4],
    gp: &SmplGainParams,
) {
    let gain_main = gp.gain_main;
    let gain_delta = gp.gain_delta;
    enc.encode_cdf(gain_main, cc.nrgres_gain4());
    enc.encode_cdf(gain_delta, cc.nrgres_shape4());
    let cfg_sel = 2i32;

    let off6 = p3 * gain_delta;
    let base7 = gain_main * cc.nrg_step(cfg_sel) - 0x154000;
    let mut gain_q = [0i32; 4];
    for (sf, gq) in gain_q.iter_mut().enumerate().take(p3 as usize) {
        let cbv = cc.gain_recon(p3 == 4, sf as i32 + off6);
        *gq = base7 + (cbv << 4);
    }

    for (sf, &cnt) in subfr_counts.iter().enumerate().take(p3 as usize) {
        if cnt <= 0 {
            continue;
        }
        let bucket = if cnt >= 30 { 3 } else { (cnt & 0xffff) / 10 };
        let mut g = (gain_q[sf] + 8192) >> 14;
        if g < -85 {
            g = -85;
        }
        let neg_part = (g >> 31) & g;
        let min_offset = (-neg_part) as usize;
        enc.encode_cdf(
            gp.nrg_res[sf],
            cc.fcbg_offset(cfg_sel as usize, bucket as usize, min_offset),
        );
    }
}

/// Inverse of `decode_smpl_pitch`: encode the LTP gains, primary lag (abs/delta), optional 64-fine,
/// and the per-segment fractional symbols, mutating the predictor state identically.
#[allow(clippy::too_many_arguments)]
fn encode_smpl_pitch(
    enc: &mut RangeEncoder,
    mem: &SmplMem,
    cc: &CcTables,
    st: &mut SmplLsfState,
    _p2: i32,
    p3: i32,
    p6: i32,
    subfr_counts: [i32; 4],
    pp: &SmplPitchParams,
) {
    let gp = mem.g_pitch;
    // The active WB path is p6==0 (HR tables, served by the logical seed); the p6!=0 LR variant
    // stays on the heap window.
    let weight_tab: u32 = if p6 != 0 { 0xe8460 } else { 0xe85b0 };
    let gain_cdf_base = if p6 != 0 { gp + 0xc0 } else { gp + 0x302 };
    let filt_cdf0 = gp + 0xdc4;
    let filt_cdf1 = gp + 0xe4c;

    let mut gain_accum: i32 = 0;
    for (sf, &cnt) in subfr_counts.iter().enumerate().take((p3 as usize).min(4)) {
        let gi = pp.gain_idx[sf];
        if p6 != 0 {
            let row = gain_cdf_base
                .wrapping_add(st.prev_gain_idx.wrapping_mul(0x22) as u32)
                .wrapping_add(0x22);
            enc.encode_cdf(gi, &mem.cdf_at(row, 17));
        } else {
            enc.encode_cdf(gi, cc.acbgain_row(st.prev_gain_idx));
        }
        st.prev_gain_idx = gi;
        let (w0, w2) = if p6 != 0 {
            (
                mem.i16(weight_tab.wrapping_add((gi as u32) * 4)) as i32,
                mem.i16(weight_tab.wrapping_add((gi as u32) * 4 + 2)) as i32,
            )
        } else {
            cc.acbgain_weights(gi)
        };
        gain_accum += w0 + 2 * w2;
        if cnt > 0 {
            let fi = pp.filt_idx[sf];
            if p6 != 0 {
                if st.prev_filt_idx == -1 {
                    enc.encode_cdf(fi, &mem.cdf_at(filt_cdf0, 35));
                } else {
                    enc.encode_cdf(
                        fi,
                        &mem.cdf_at(filt_cdf1.wrapping_sub((st.prev_filt_idx as u32) * 2), 35),
                    );
                }
            } else if st.prev_filt_idx == -1 {
                enc.encode_cdf(fi, cc.fcbgain_v());
            } else {
                enc.encode_cdf(fi, cc.fcbgain_v_delta(st.prev_filt_idx));
            }
            st.prev_filt_idx = fi;
        }
    }
    let avg_gain = gain_accum / p3;

    // Lag block: write the estimator's chosen contour (`blockseg_idx`) + per-40-block lag indices
    // (`laginds`) via `smpl_encode_lags_wire`, NOT a single-lag contour-map flattening. The delta-lag
    // CMF `mode` is selected by the mean-ACB-gain thresholds below.
    let mode = if avg_gain < 10007 {
        0
    } else if avg_gain < 14085 {
        1
    } else {
        2
    };
    let tab = super::smpl_pitch_enc::load_pitch_tables();
    super::smpl_pitch_enc::smpl_encode_lags_wire(
        tab,
        enc,
        pp.blockseg_idx,
        &pp.laginds,
        st.prev_lagblk,
        st.prev_lagidx,
        mode,
    );
    let (nblk, nidx) =
        super::smpl_pitch_enc::smpl_lags_predictor_after(tab, pp.blockseg_idx, &pp.laginds);
    st.prev_lagblk = nblk;
    st.prev_lagidx = nidx;
}

#[cfg(test)]
mod tests {
    use super::super::decoder::MlowDecoder;
    use super::*;

    // Isolated voiced pitch-block round-trip: encode the gains + the estimator contour
    // (`blockseg_idx`/`laginds`) then decode them back; the decoder's `block_lags` must equal the
    // encoded `laginds`, proving the wire encode is the inverse of `decode_smpl_pitch`.
    #[test]
    fn pitch_block_round_trips_contour() {
        let mem = load_smpl_mem();
        let cc = load_cc_tables();
        let cases: &[(usize, [i32; 8], [i32; 4])] = &[
            (142, [128, 129, 129, 118, 118, 121, 121, 123], [5, 2, 2, 2]),
            (142, [128, 129, 129, 118, 118, 121, 121, 123], [5, 6, 2, 2]),
            (59, [123, 123, 123, 123, 128, 128, 132, 132], [2, 6, 6, 6]),
        ];
        for &(bsx, laginds, gains) in cases {
            let mut pp = SmplPitchParams {
                gain_idx: gains,
                filt_idx: [0; 4],
                blockseg_idx: bsx,
                laginds,
            };
            // subfr_counts > 0 so the filt_idx path fires (as in the real voiced encode).
            let subfr = [1i32; 4];
            for sf in 0..4 {
                pp.filt_idx[sf] = 0;
            }
            let mut enc = RangeEncoder::new(64);
            let mut est = SmplLsfState {
                prev_lag: -1,
                prev_frac_lag: -1,
                prev_lagblk: -1,
                prev_lagidx: -1,
                ..Default::default()
            };
            encode_smpl_pitch(&mut enc, mem, cc, &mut est, 320, 4, 0, subfr, &pp);
            enc.done();
            let n = enc.consumed_len();
            let bytes = enc.bytes()[..n].to_vec();
            let mut dec = super::super::rangecoder::RangeDecoder::new(&bytes);
            let mut dst = SmplLsfState {
                prev_lag: -1,
                prev_frac_lag: -1,
                ..Default::default()
            };
            let pr = super::super::smpl_pitch::decode_smpl_pitch(
                &mut dec, mem, cc, &mut dst, 320, 4, 0, subfr,
            );
            assert_eq!(
                pr.block_lags.to_vec(),
                laginds.to_vec(),
                "bsx={bsx}: decoded block_lags != encoded laginds"
            );
        }
    }

    fn corr(a: &[f32], b: &[f32]) -> f64 {
        let (mut sxy, mut sxx, mut syy) = (0f64, 0f64, 0f64);
        for i in 0..a.len() {
            let (x, y) = (a[i] as f64, b[i] as f64);
            sxy += x * y;
            sxx += x * x;
            syy += y * y;
        }
        if sxx < 1e-12 || syy < 1e-12 {
            return 0.0;
        }
        sxy / (sxx * syy).sqrt()
    }

    // Closed-loop: encode a tone and decode it back with the (byte-exact) decoder; the LB-core
    // reconstruction must track the input waveform shape (correlation). Proves the analysis →
    // entropy-encode → decode chain produces a frame that reconstructs the input audio.
    #[test]
    fn encode_round_trips_a_tone() {
        let mut enc = MlowEncoder::new();
        let mut dec = MlowDecoder::new();
        let mut best = 0f64;
        for f in 0..8 {
            let pcm: Vec<f32> = (0..960)
                .map(|i| {
                    let t = (f * 960 + i) as f64 / 16000.0;
                    (0.5 * (2.0 * std::f64::consts::PI * 550.0 * t).sin()) as f32
                })
                .collect();
            let frame = enc.encode(&pcm).expect("encode");
            assert_eq!(frame[0], 0x50, "active frame TOC");
            let out = dec.decode(&frame);
            // The decoder's harmonic postfilter adds 48 samples of group delay; align before correlating.
            const HARM_DELAY: usize = 48;
            best = best.max(corr(&pcm[..pcm.len() - HARM_DELAY], &out[HARM_DELAY..]));
        }
        assert!(
            best > 0.7,
            "encode→decode round-trip correlation too low: {best}"
        );
    }

    // Deterministic synthetic signal cycling voiced tone / unvoiced noise / voiced+noise / silence,
    // so encode tests exercise both the voiced (LTP) and unvoiced (gains) paths from code alone. Kept
    // local to this module so the encoder's self-contained guards need no data file.
    fn synth_input() -> Vec<f32> {
        use std::f32::consts::PI;
        const FRAME: usize = 960;
        const PACKETS: usize = 24;
        let mut out = Vec::with_capacity(PACKETS * FRAME);
        let mut seed: u32 = 0x9e37_79b9;
        for i in 0..PACKETS * FRAME {
            let t = i as f32 / 16000.0;
            seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
            let noise = (seed >> 8) as f32 / 8_388_608.0 - 1.0;
            let v = match (i / FRAME) % 4 {
                0 => 0.30 * (2.0 * PI * 150.0 * t).sin() + 0.10 * (2.0 * PI * 300.0 * t).sin(),
                1 => 0.18 * noise,
                2 => 0.25 * (2.0 * PI * 120.0 * t).sin() + 0.05 * noise,
                _ => 0.0,
            };
            out.push(v);
        }
        out
    }

    fn encode_all(enc: &mut MlowEncoder, input: &[f32]) -> Vec<Vec<u8>> {
        input
            .chunks(960)
            .filter(|c| c.len() == 960)
            .map(|c| enc.encode(c).expect("encode"))
            .collect()
    }

    // R4: two fresh encoders fed the same input must emit byte-identical frames. Catches any
    // nondeterminism (hashmap iteration order, uninitialized state, time/RNG seeding).
    #[test]
    fn encode_is_deterministic_across_fresh_instances() {
        let input = synth_input();
        let a = encode_all(&mut MlowEncoder::new(), &input);
        let b = encode_all(&mut MlowEncoder::new(), &input);
        assert_eq!(a, b, "two fresh encoders produced different frames");
    }

    // R5 (encoder): reset() must return the encoder to a fresh state. After N frames, reset then
    // re-encode the same input; output must equal a fresh instance's. Guards a reset() that forgets a
    // SmplEncoderState field (the analysis predictor history).
    #[test]
    fn encoder_reset_returns_to_fresh_state() {
        let input = synth_input();
        let fresh = encode_all(&mut MlowEncoder::new(), &input);

        let mut enc = MlowEncoder::new();
        let _ = encode_all(&mut enc, &input); // run N frames to populate predictor state
        enc.reset();
        let after_reset = encode_all(&mut enc, &input);
        assert_eq!(
            after_reset, fresh,
            "encoder reset() did not restore fresh state"
        );
    }

    // R5 (decoder): reset() must return the decoder to a fresh state. Guards a reset() that forgets
    // prev_nlsf / harm / lstate (the cross-frame predictor and postfilter history).
    #[test]
    fn decoder_reset_returns_to_fresh_state() {
        let input = synth_input();
        let frames = encode_all(&mut MlowEncoder::new(), &input);

        let decode_all = |dec: &mut MlowDecoder| -> Vec<f32> {
            frames
                .iter()
                .flat_map(|f| dec.decode(f))
                .collect::<Vec<_>>()
        };

        let fresh = decode_all(&mut MlowDecoder::new());

        let mut dec = MlowDecoder::new();
        let _ = decode_all(&mut dec); // run N frames to populate predictor/postfilter state
        dec.reset();
        let after_reset = decode_all(&mut dec);
        assert_eq!(
            after_reset, fresh,
            "decoder reset() did not restore fresh state"
        );
    }

    // R9: the encoder is config-0 / no-DTX, so every emitted frame is an active mlow TOC (0x50) and
    // never a standard Opus/CELT TOC. Makes the implicit contract explicit; a future DTX/SID path
    // would trip this.
    #[test]
    fn encoder_emits_only_active_mlow_toc() {
        use super::super::is_standard_opus_frame;
        let input = synth_input();
        for frame in encode_all(&mut MlowEncoder::new(), &input) {
            let toc = frame[0];
            assert!(
                !is_standard_opus_frame(toc),
                "encoder emitted a standard-opus TOC 0x{toc:02x}"
            );
            assert_eq!(toc, 0x50, "encoder emitted non-config-0 TOC 0x{toc:02x}");
        }
    }

    // Dev oracle: decode hex frames (MLOW_HEX) through `decode_smpl_pitch`, dumping each voiced
    // frame's reconstructed `block_lags` (the `laginds` domain) to MLOW_PITCH_DUMP. Used to prove
    // the decoder reconstructs `laginds` from reference-encoded bytes (representation equivalence).
    #[test]
    fn dump_decoded_pitch_from_hex() {
        let Ok(hexpath) = std::env::var("MLOW_HEX_PITCH") else {
            return;
        };
        let out = std::env::var("MLOW_PITCH_DUMP").expect("MLOW_PITCH_DUMP path");
        let text = std::fs::read_to_string(&hexpath).expect("read hex");
        let mem = load_smpl_mem();
        let cc = load_cc_tables();
        let tbl = load_smpl_tables();
        let mut recs: Vec<String> = Vec::new();
        for (pkt, line) in text.lines().enumerate() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let frame: Vec<u8> = (0..line.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&line[i..i + 2], 16).expect("hex"))
                .collect();
            if frame.first() != Some(&0x50) {
                continue;
            }
            let config = (frame[0] >> 2) as usize & 1;
            let mut dec = super::super::rangecoder::RangeDecoder::new(&frame[1..]);
            let mut lstate = SmplLsfState::default();
            for f in 0..3 {
                let lsf = super::super::smpl_decode::decode_smpl_lsf(
                    &mut dec,
                    tbl,
                    &mut lstate,
                    config,
                    f,
                );
                let pulses = super::super::smpl_pulse::decode_smpl_pulses(
                    &mut dec,
                    cc,
                    320,
                    4,
                    1,
                    config as i32,
                    lsf.stage1,
                );
                if lsf.stage1 == 1 {
                    let pr = super::super::smpl_pitch::decode_smpl_pitch(
                        &mut dec,
                        mem,
                        cc,
                        &mut lstate,
                        320,
                        4,
                        config as i32,
                        pulses.subfr,
                    );
                    recs.push(format!(
                        "{{\"pkt\":{pkt},\"frame\":{f},\"voiced\":1,\"block_lags\":{:?}}}",
                        pr.block_lags.to_vec()
                    ));
                } else {
                    super::super::smpl_gains::decode_smpl_gains(&mut dec, cc, 4, pulses.subfr);
                    recs.push(format!("{{\"pkt\":{pkt},\"frame\":{f},\"voiced\":0}}"));
                }
            }
        }
        std::fs::write(&out, format!("[{}]", recs.join(","))).expect("write dump");
    }

    // Dev harness: encode an i16 mono 16 kHz raw file (env MLOW_MIC) into 60 ms MLow frames and write
    // them as hex (one per line) to MLOW_OUT, so the peer's reference decoder can round-trip them
    // against the mic. Gated on env vars so the normal suite never touches the (large, machine-local)
    // mic clip.
    #[test]
    fn encode_mic_dump_hex() {
        let Ok(mic) = std::env::var("MLOW_MIC") else {
            return;
        };
        let out = std::env::var("MLOW_OUT").expect("MLOW_OUT path");
        let bytes = std::fs::read(&mic).expect("read mic");
        let samples: Vec<i16> = bytes
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect();
        let mut enc = MlowEncoder::new();
        let mut lines = String::new();
        for chunk in samples.chunks(OPUS_FRAME_SAMPS) {
            if chunk.len() < OPUS_FRAME_SAMPS {
                break;
            }
            let pcm: Vec<f32> = chunk.iter().map(|&s| s as f32 / 32768.0).collect();
            let frame = enc.encode(&pcm).expect("encode");
            for b in &frame {
                lines.push_str(&format!("{b:02x}"));
            }
            lines.push('\n');
        }
        std::fs::write(&out, lines).expect("write frames");
    }

    // Dev harness: encode the mic (MLOW_MIC) and decode it back through OUR own MlowDecoder, writing
    // the reconstruction as i16 raw to MLOW_SELFDEC_OUT. Lets the codec round-trip be measured against
    // our decoder independently of the peer's reference decoder.
    #[test]
    fn encode_mic_selfdecode_raw() {
        let Ok(mic) = std::env::var("MLOW_MIC") else {
            return;
        };
        let out = std::env::var("MLOW_SELFDEC_OUT").expect("MLOW_SELFDEC_OUT path");
        let bytes = std::fs::read(&mic).expect("read mic");
        let samples: Vec<i16> = bytes
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect();
        let mut enc = MlowEncoder::new();
        let mut dec = MlowDecoder::new();
        let mut pcm_out: Vec<i16> = Vec::new();
        for chunk in samples.chunks(OPUS_FRAME_SAMPS) {
            if chunk.len() < OPUS_FRAME_SAMPS {
                break;
            }
            let pcm: Vec<f32> = chunk.iter().map(|&s| s as f32 / 32768.0).collect();
            let frame = enc.encode(&pcm).expect("encode");
            for s in dec.decode(&frame) {
                pcm_out.push((s * 32768.0).clamp(-32768.0, 32767.0) as i16);
            }
        }
        let mut buf = Vec::with_capacity(pcm_out.len() * 2);
        for s in &pcm_out {
            buf.extend_from_slice(&s.to_le_bytes());
        }
        std::fs::write(&out, buf).expect("write selfdec");
    }

    // Dev oracle: decode hex frames (MLOW_HEX, one per line) through OUR MlowDecoder, write i16 raw to
    // MLOW_DEC_OUT. Used to confirm our decoder reconstructs reference-encoded frames, isolating the
    // decoder from the encoder.
    #[test]
    fn decode_hex_frames_raw() {
        let Ok(hexpath) = std::env::var("MLOW_HEX") else {
            return;
        };
        let out = std::env::var("MLOW_DEC_OUT").expect("MLOW_DEC_OUT path");
        let text = std::fs::read_to_string(&hexpath).expect("read hex");
        let mut dec = MlowDecoder::new();
        let mut pcm_out: Vec<i16> = Vec::new();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let frame: Vec<u8> = (0..line.len())
                .step_by(2)
                .map(|i| u8::from_str_radix(&line[i..i + 2], 16).expect("hex"))
                .collect();
            for s in dec.decode(&frame) {
                pcm_out.push((s * 32768.0).clamp(-32768.0, 32767.0) as i16);
            }
        }
        let mut buf = Vec::with_capacity(pcm_out.len() * 2);
        for s in &pcm_out {
            buf.extend_from_slice(&s.to_le_bytes());
        }
        std::fs::write(&out, buf).expect("write dec");
    }
}

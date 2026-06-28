//! SILK VAD: per-internal-frame speech-activity probability and the `coded_as_active_voice` flag the
//! bitrate controller and voiced/unvoiced classifier read. Fixed-point implementation of the SA_Q8
//! measure plus the noise-level tracker, the 2-band allpass filterbank, and the per-packet hangover.
//! Runs on the raw int16 input PCM (no encoder HP), 320 samples per internal frame at 16 kHz.

// SILK fixed-point primitives.

const SILK_INT32_MAX: i32 = 0x7FFF_FFFF;
const SILK_INT16_MAX: i32 = 0x7FFF;
const SILK_INT16_MIN: i32 = -0x8000;
const SILK_UINT8_MAX: i32 = 0xFF;

#[inline]
fn sat16(a: i32) -> i32 {
    a.clamp(SILK_INT16_MIN, SILK_INT16_MAX)
}

/// silk_SMULWB: (a * (int16)b) >> 16, 64-bit intermediate.
#[inline]
fn smulwb(a: i32, b: i32) -> i32 {
    ((a as i64 * (b as i16 as i64)) >> 16) as i32
}

/// silk_SMLAWB: a + ((b * (int16)c) >> 16).
#[inline]
fn smlawb(a: i32, b: i32, c: i32) -> i32 {
    (a as i64 + ((b as i64 * (c as i16 as i64)) >> 16)) as i32
}

/// silk_SMULWW: (a * b) >> 16, 64-bit intermediate.
#[inline]
fn smulww(a: i32, b: i32) -> i32 {
    ((a as i64 * b as i64) >> 16) as i32
}

/// silk_SMULBB: (int16)a * (int16)b.
#[inline]
fn smulbb(a: i32, b: i32) -> i32 {
    (a as i16 as i32).wrapping_mul(b as i16 as i32)
}

/// silk_SMLABB: a + (int16)b * (int16)c.
#[inline]
fn smlabb(a: i32, b: i32, c: i32) -> i32 {
    a.wrapping_add((b as i16 as i32).wrapping_mul(c as i16 as i32))
}

/// silk_ADD_POS_SAT32: saturating add of two non-negative values.
#[inline]
fn add_pos_sat32(a: i32, b: i32) -> i32 {
    if ((a as u32).wrapping_add(b as u32) & 0x8000_0000) != 0 {
        SILK_INT32_MAX
    } else {
        a.wrapping_add(b)
    }
}

#[inline]
fn div32(a: i32, b: i32) -> i32 {
    a / b
}

/// silk_CLZ32: leading zeros (silk returns 32 for 0).
#[inline]
fn clz32(x: i32) -> i32 {
    (x as u32).leading_zeros() as i32
}

/// silk_ROR32: rotate right by `rot` (negative `rot` rotates left). `rot & 31` folds both directions
/// onto a single rotate_right (rotate_right by -5 == rotate_right by 27).
#[inline]
fn ror32(a32: i32, rot: i32) -> i32 {
    (a32 as u32).rotate_right((rot & 31) as u32) as i32
}

/// silk_CLZ_FRAC.
#[inline]
fn clz_frac(inp: i32) -> (i32, i32) {
    let lz = clz32(inp);
    let frac_q7 = ror32(inp, 24 - lz) & 0x7f;
    (lz, frac_q7)
}

/// silk_lin2log: approximation of 128 * log2().
#[inline]
fn lin2log(in_lin: i32) -> i32 {
    let (lz, frac_q7) = clz_frac(in_lin);
    smlawb(frac_q7, frac_q7.wrapping_mul(128 - frac_q7), 179) + ((31 - lz) << 7)
}

/// silk_SQRT_APPROX.
#[inline]
fn sqrt_approx(x: i32) -> i32 {
    if x <= 0 {
        return 0;
    }
    let (lz, frac_q7) = clz_frac(x);
    let mut y = if lz & 1 != 0 { 32768 } else { 46214 };
    y >>= lz >> 1;
    smlawb(y, y, smulbb(213, frac_q7))
}

/// silk_sigm_Q15: piecewise-linear sigmoid approximation.
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
            NEG[ind] - smulbb(SLOPE[ind], in_q5 & 0x1F)
        }
    } else if in_q5 >= 6 * 32 {
        32767
    } else {
        let ind = (in_q5 >> 5) as usize;
        POS[ind] + smulbb(SLOPE[ind], in_q5 & 0x1F)
    }
}

// VAD constants.
const VAD_N_BANDS: usize = 4;
const VAD_INTERNAL_SUBFRAMES_LOG2: i32 = 2;
const VAD_INTERNAL_SUBFRAMES: usize = 1 << VAD_INTERNAL_SUBFRAMES_LOG2;
const VAD_NOISE_LEVEL_SMOOTH_COEF_Q16: i32 = 1024;
const VAD_NOISE_LEVELS_BIAS: i32 = 50;
const VAD_NEGATIVE_OFFSET_Q5: i32 = 128;
const VAD_SNR_FACTOR_Q16: i32 = 45000;

const TILT_WEIGHTS: [i32; VAD_N_BANDS] = [30000, 6000, -12000, -12000];

const A_FB1_20: i32 = 3894 << 1; // i16
const A_FB1_21: i32 = -29322; // i16

const SPEECH_ACTIVITY_DTX_THRES_Q8: i32 = (0.05 * 256.0) as i32; // SILK_FIX_CONST(0.05, 8) = 12

/// Persistent SILK VAD state, carried across packets.
pub(crate) struct SmplVadState {
    ana_state: [i32; 2],
    ana_state1: [i32; 2],
    ana_state2: [i32; 2],
    xnrg_subfr: [i32; VAD_N_BANDS],
    nl: [i32; VAD_N_BANDS],
    inv_nl: [i32; VAD_N_BANDS],
    noise_level_bias: [i32; VAD_N_BANDS],
    counter: i32,
    hp_state: i32,
    // VAD tuning params (default 0 in this config).
    noise_lvl_update_speed: i32,
    non_binariness: i32,
    highpass_sharpness: i32,
    // DTX hangover (per-packet).
    remaining_dtx_hangover: i32,
    hangover_ms: i32,
}

/// Per-internal-frame VAD type used by the hangover logic.
#[derive(Clone, Copy, PartialEq, Eq)]
enum VadType {
    Active,
    Inactive,
    Hangover,
}

/// VAD output for one 60 ms packet: per-internal-frame speech-activity probability and the
/// packet-level `coded_as_active_voice` flag.
pub(crate) struct VadPacketResult {
    pub vad_results: [f32; 3],
    pub coded_as_active_voice: bool,
}

impl SmplVadState {
    /// Initialize the VAD state.
    pub(crate) fn new() -> Self {
        let mut noise_level_bias = [0i32; VAD_N_BANDS];
        let mut nl = [0i32; VAD_N_BANDS];
        let mut inv_nl = [0i32; VAD_N_BANDS];
        for b in 0..VAD_N_BANDS {
            noise_level_bias[b] = (VAD_NOISE_LEVELS_BIAS / (b as i32 + 1)).max(1);
            nl[b] = 100 * noise_level_bias[b];
            inv_nl[b] = SILK_INT32_MAX / nl[b];
        }
        SmplVadState {
            ana_state: [0; 2],
            ana_state1: [0; 2],
            ana_state2: [0; 2],
            xnrg_subfr: [0; VAD_N_BANDS],
            nl,
            inv_nl,
            noise_level_bias,
            counter: 15,
            hp_state: 0,
            noise_lvl_update_speed: 0,
            non_binariness: 0,
            highpass_sharpness: 0,
            remaining_dtx_hangover: 60, // SMPL_DEFAULT_HANGOVER_MS
            hangover_ms: 60,
        }
    }

    /// First-order ARMA HP filter with zero at DC, in place over `len` samples.
    fn filt_hp(&mut self, x: &mut [i32], b_q16: i32, a_neg_q16: i32, len: usize) {
        for xi in x.iter_mut().take(len) {
            let inval = smulwb(b_q16, *xi);
            let outval = sat16(self.hp_state - inval);
            self.hp_state = smlawb(inval, a_neg_q16, outval);
            *xi = outval;
        }
    }

    /// 2-band split via first-order allpass filters. Writes low band to `out_l[0..N/2]` and high band
    /// to `out_h[0..N/2]`. `s` is the carried 2-element state.
    fn ana_filt_bank_1(
        inp: &[i32],
        s: &mut [i32; 2],
        out_l: &mut [i32],
        out_h: &mut [i32],
        n: usize,
    ) {
        let n2 = n >> 1;
        for k in 0..n2 {
            let in32 = inp[2 * k] << 10;
            let y = in32 - s[0];
            let x = smlawb(y, y, A_FB1_21);
            let out_1 = s[0] + x;
            s[0] = in32 + x;

            let in32 = inp[2 * k + 1] << 10;
            let y = in32 - s[1];
            let x = smulwb(y, A_FB1_20);
            let out_2 = s[1] + x;
            s[1] = in32 + x;

            out_l[k] = sat16(rshift_round(out_2 + out_1, 11));
            out_h[k] = sat16(rshift_round(out_2 - out_1, 11));
        }
    }

    /// Smooth the per-band noise-level estimate toward the current band energies.
    fn get_noise_levels(&mut self, p_x: &[i32; VAD_N_BANDS]) {
        let min_coef = if self.counter < 1000 {
            let mc = div32(SILK_INT16_MAX, (self.counter >> 4) + 1);
            self.counter += 1;
            mc
        } else {
            0
        };
        for (((&px, bias), inv_nl), nl_out) in p_x
            .iter()
            .zip(self.noise_level_bias.iter())
            .zip(self.inv_nl.iter_mut())
            .zip(self.nl.iter_mut())
        {
            let nl = *nl_out;
            let nrg = add_pos_sat32(px, *bias);
            let inv_nrg = div32(SILK_INT32_MAX, nrg);
            let mut coef = if nrg > (nl << 3) {
                VAD_NOISE_LEVEL_SMOOTH_COEF_Q16 >> 3
            } else if nrg < nl {
                VAD_NOISE_LEVEL_SMOOTH_COEF_Q16
            } else {
                smulwb(smulww(inv_nrg, nl), VAD_NOISE_LEVEL_SMOOTH_COEF_Q16 << 1)
            };
            coef = (coef * (100 + self.noise_lvl_update_speed)) / 100;
            coef = coef.max(min_coef);
            *inv_nl = smlawb(*inv_nl, inv_nrg - *inv_nl, coef);
            *nl_out = div32(SILK_INT32_MAX, *inv_nl).min(0x00FF_FFFF);
        }
    }

    /// Returns speech_activity_Q8 for one `framelen`-sample int16 frame.
    fn get_sa_q8(&mut self, p_in: &[i32], framelen: usize) -> i32 {
        let dec_fl1 = framelen >> 1;
        let dec_fl2 = framelen >> 2;
        let dec_fl3 = framelen >> 3;

        let mut x_offset = [0usize; VAD_N_BANDS];
        x_offset[0] = 0;
        x_offset[1] = dec_fl3 + dec_fl2;
        x_offset[2] = x_offset[1] + dec_fl3;
        x_offset[3] = x_offset[2] + dec_fl2;
        let x_total = x_offset[3] + dec_fl1;
        let mut x = vec![0i32; x_total];

        // 0-8 kHz -> 0-4 kHz (X[0..]) and 4-8 kHz (X[offset3..])
        {
            let (lo, hi) = x.split_at_mut(x_offset[3]);
            let mut s = self.ana_state;
            Self::ana_filt_bank_1(p_in, &mut s, lo, hi, framelen);
            self.ana_state = s;
        }
        // 0-4 kHz -> 0-2 kHz (X[0..]) and 2-4 kHz (X[offset2..]); reads/writes within X[0..]
        {
            let mut s = self.ana_state1;
            Self::ana_filt_bank_1_inplace(&mut x, x_offset[2], &mut s, dec_fl1);
            self.ana_state1 = s;
        }
        // 0-2 kHz -> 0-1 kHz (X[0..]) and 1-2 kHz (X[offset1..])
        {
            let mut s = self.ana_state2;
            Self::ana_filt_bank_1_inplace(&mut x, x_offset[1], &mut s, dec_fl2);
            self.ana_state2 = s;
        }

        // HP filter on the lowest band, -3 dB @ 66 Hz.
        let mut a_neg_q16 = 53084i32;
        a_neg_q16 = (a_neg_q16 * (100 - self.highpass_sharpness)) / 100;
        let b_q16 = (65536 + a_neg_q16) / 2;
        {
            let mut lo = x[..dec_fl3].to_vec();
            self.filt_hp(&mut lo, b_q16, a_neg_q16, dec_fl3);
            x[..dec_fl3].copy_from_slice(&lo);
        }

        // Energy in each band.
        let mut xnrg = [0i32; VAD_N_BANDS];
        for b in 0..VAD_N_BANDS {
            let dec = framelen >> ((VAD_N_BANDS - b).min(VAD_N_BANDS - 1));
            let dec_subfr_len = dec >> VAD_INTERNAL_SUBFRAMES_LOG2;
            let mut dec_subfr_offset = 0usize;
            xnrg[b] = self.xnrg_subfr[b];
            let mut sum_squared = 0i32;
            for s in 0..VAD_INTERNAL_SUBFRAMES {
                sum_squared = 0;
                for i in 0..dec_subfr_len {
                    let x_tmp = x[x_offset[b] + i + dec_subfr_offset] >> 3;
                    sum_squared = smlabb(sum_squared, x_tmp, x_tmp);
                }
                if s < VAD_INTERNAL_SUBFRAMES - 1 {
                    xnrg[b] = add_pos_sat32(xnrg[b], sum_squared);
                } else {
                    xnrg[b] = add_pos_sat32(xnrg[b], sum_squared >> 1);
                }
                dec_subfr_offset += dec_subfr_len;
            }
            self.xnrg_subfr[b] = sum_squared;
        }

        self.get_noise_levels(&xnrg);

        // Signal-plus-noise to noise ratio.
        let mut sum_squared = 0i32;
        let mut input_tilt = 0i32;
        for b in 0..VAD_N_BANDS {
            let speech_nrg = xnrg[b] - self.nl[b];
            if speech_nrg > 0 {
                let ratio_q8 = if (xnrg[b] & 0xFF80_0000u32 as i32) == 0 {
                    div32(xnrg[b] << 8, self.nl[b] + 1)
                } else {
                    div32(xnrg[b], (self.nl[b] >> 8) + 1)
                };
                let mut snr_q7 = lin2log(ratio_q8) - 8 * 128;
                sum_squared = smlabb(sum_squared, snr_q7, snr_q7);
                if speech_nrg < (1 << 20) {
                    snr_q7 = smulwb(sqrt_approx(speech_nrg) << 6, snr_q7);
                }
                input_tilt = smlawb(input_tilt, TILT_WEIGHTS[b], snr_q7);
            }
        }
        sum_squared = div32(sum_squared, VAD_N_BANDS as i32);
        let p_snr_db_q7 = (3 * sqrt_approx(sum_squared)) as i16 as i32;

        let vad_snr_factor_q16 = (VAD_SNR_FACTOR_Q16 * (150 - self.non_binariness)) / 150;
        let sa_q15 = sigm_q15(smulwb(vad_snr_factor_q16, p_snr_db_q7) - VAD_NEGATIVE_OFFSET_Q5);

        let _ = input_tilt; // input_tilt_Q15 unused downstream here

        (sa_q15 >> 7).min(SILK_UINT8_MAX)
    }

    /// In-place 2-band split: reads X[0..n], writes low band to X[0..n/2] and high band to
    /// X[hi_off..hi_off+n/2]. Input and low-out overlap; `in[2k]/in[2k+1]` are read before `out[k]`
    /// is written, so a forward scan is safe since k <= 2k.
    fn ana_filt_bank_1_inplace(x: &mut [i32], hi_off: usize, s: &mut [i32; 2], n: usize) {
        let n2 = n >> 1;
        for k in 0..n2 {
            let in32 = x[2 * k] << 10;
            let y = in32 - s[0];
            let xx = smlawb(y, y, A_FB1_21);
            let out_1 = s[0] + xx;
            s[0] = in32 + xx;

            let in32 = x[2 * k + 1] << 10;
            let y = in32 - s[1];
            let xx = smulwb(y, A_FB1_20);
            let out_2 = s[1] + xx;
            s[1] = in32 + xx;

            x[hi_off + k] = sat16(rshift_round(out_2 - out_1, 11));
            x[k] = sat16(rshift_round(out_2 + out_1, 11));
        }
    }

    /// Process one 60 ms packet (3 internal frames of `framelen` int16 samples). `activity` mirrors the
    /// Opus VAD decision; we use NO_DECISION (-1), so the type is threshold-driven, then hangover runs.
    pub(crate) fn process_packet(&mut self, pcm_i16: &[i16], framelen: usize) -> VadPacketResult {
        const FRAMES_PER_PACKET: usize = 3;
        const PACKET_MS: i32 = 60;
        let mut vad_results = [0f32; 3];
        // Reject short packets up front so the frame loop's fixed-stride indexing can't panic.
        let expected_len = FRAMES_PER_PACKET * framelen;
        if pcm_i16.len() < expected_len {
            return VadPacketResult {
                vad_results,
                coded_as_active_voice: false,
            };
        }
        let mut vad_type = [VadType::Inactive; 3];
        for i in 0..FRAMES_PER_PACKET {
            let t = i * framelen;
            let frame: Vec<i32> = pcm_i16[t..t + framelen].iter().map(|&s| s as i32).collect();
            let sa_q8 = self.get_sa_q8(&frame, framelen);
            vad_results[i] = sa_q8 as f32 / 256.0;
            // activity == NO_DECISION path.
            vad_type[i] = if sa_q8 > SPEECH_ACTIVITY_DTX_THRES_Q8 {
                VadType::Active
            } else {
                VadType::Inactive
            };
        }

        let mut coded_as_active_voice = false;
        for ty in vad_type.iter_mut() {
            if *ty == VadType::Active {
                self.remaining_dtx_hangover = self.hangover_ms;
            } else if self.remaining_dtx_hangover > 0 {
                *ty = VadType::Hangover;
                self.remaining_dtx_hangover -= PACKET_MS / FRAMES_PER_PACKET as i32;
            }
            if *ty != VadType::Inactive {
                coded_as_active_voice = true;
            }
        }

        VadPacketResult {
            vad_results,
            coded_as_active_voice,
        }
    }
}

/// silk_RSHIFT_ROUND.
#[inline]
fn rshift_round(a: i32, shift: i32) -> i32 {
    if shift == 1 {
        (a >> 1) + (a & 1)
    } else {
        ((a >> (shift - 1)) + 1) >> 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn silence_gives_low_activity() {
        let mut vad = SmplVadState::new();
        let pcm = vec![0i16; 960];
        let r = vad.process_packet(&pcm, 320);
        for v in r.vad_results {
            assert!(v <= 0.05, "silence activity {v}");
        }
    }

    #[test]
    fn onset_of_speechlike_signal_is_active() {
        // The SILK VAD adapts its noise floor toward a sustained signal, so a fair test settles the
        // floor on quiet input, then presents a loud broadband (speech-like) onset and checks the
        // activity rises on that onset before the floor catches up.
        let mut vad = SmplVadState::new();
        let quiet: Vec<i16> = vec![0i16; 960];
        for _ in 0..30 {
            vad.process_packet(&quiet, 320);
        }
        let mut seed = 0x1234_5678u32;
        let loud: Vec<i16> = (0..960)
            .map(|i| {
                let t = i as f32 / 16000.0;
                let tone = (2.0 * std::f32::consts::PI * 200.0 * t).sin()
                    + (2.0 * std::f32::consts::PI * 900.0 * t).sin()
                    + (2.0 * std::f32::consts::PI * 2500.0 * t).sin();
                seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
                let noise = (seed >> 16) as f32 / 65536.0 - 0.5;
                (6000.0 * (tone / 3.0 + 0.2 * noise)) as i16
            })
            .collect();
        let r = vad.process_packet(&loud, 320);
        let onset = r.vad_results[0].max(r.vad_results[1]).max(r.vad_results[2]);
        assert!(onset > 0.5, "speech-like onset activity {onset}");
    }
}

#[cfg(test)]
mod ground_truth_tests {
    use super::*;
    use serde_json::Value;

    // The SILK VAD is fixed-point, so its per-frame speech-activity probability and the packet-level
    // coded_as_active_voice flag must match the reference bit-exactly. `vad_ground_truth.json` is the
    // ground-truth SA_Q8 dump on the synthetic mic (`synth_mic.raw`), over the packets where the
    // carried noise-level state stays bit-exact.
    #[test]
    fn vad_matches_c_ground_truth() {
        let raw = include_bytes!("testdata/synth_mic.raw");
        let samples: Vec<i16> = raw
            .chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]))
            .collect();
        let gt: Value = serde_json::from_str(include_str!("testdata/vad_ground_truth.json"))
            .expect("vad_ground_truth.json");
        let gt = gt.as_array().expect("array");

        let mut vad = SmplVadState::new();
        let mut results: Vec<(f32, i32)> = Vec::new();
        for chunk in samples.chunks(960) {
            if chunk.len() < 960 {
                break;
            }
            let r = vad.process_packet(chunk, 320);
            for f in 0..3 {
                results.push((r.vad_results[f], r.coded_as_active_voice as i32));
            }
        }

        for rec in gt {
            let pkt = rec["pkt"].as_u64().unwrap() as usize;
            let frame = rec["frame"].as_u64().unwrap() as usize;
            let idx = pkt * 3 + frame;
            let (spact, cav) = results[idx];
            let c_spact = rec["spact"].as_f64().unwrap() as f32;
            let c_cav = rec["cav"].as_i64().unwrap() as i32;
            assert!(
                (spact - c_spact).abs() < 1e-4,
                "pkt {pkt} frame {frame}: spact {spact} != ref {c_spact}"
            );
            assert_eq!(cav, c_cav, "pkt {pkt} frame {frame}: cav mismatch");
        }
    }
}

//! Audio-quality regression tests for the MLow encoder and decoder.
//!
//! The shipped encoder picks voiced (LTP) or unvoiced per internal frame via analysis-by-synthesis,
//! so a voiced input is reconstructed with its pitch periodicity, and an aperiodic input stays
//! broadband. These tests pin that behaviour in both directions:
//!
//! OUTBOUND (encode → decode round-trip):
//! - Voiced signals (harmonic-rich, fundamental + harmonics) reconstruct with periodicity intact
//!   (autocorrelation peak at the pitch lag well above the noise floor).
//! - Pure-tone signals reconstruct with spectral concentration (Goertzel ratio well above 1).
//! - Silence / near-silence produces no spurious energy.
//! - Unvoiced noise reconstructs as broadband (NOT pitched): the voiced path is not applied to an
//!   aperiodic signal.
//!
//! INBOUND (decoder energy-envelope tracking):
//! - The decoder tracks RMS energy of the synthetic frames in e2e_vectors.json: silence frames stay
//!   near zero, active frames produce measurable output, confirming inbound audio is not silent.
//!
//! The byte-exact wire-format guarantee lives in the decoder round-trip vectors and the golden test,
//! not here; these tests guard perceptual/structural properties of the audio.

#![cfg(test)]

use super::decoder::MlowDecoder;
use super::encode::MlowEncoder;

// Signal generators

/// Pure cosine tone: n samples @16 kHz, amplitude amp in [-1,1].
fn gen_tone(freq_hz: f64, n: usize, amp: f32) -> Vec<f32> {
    (0..n)
        .map(|i| {
            let t = i as f64 / 16000.0;
            (amp as f64 * (2.0 * std::f64::consts::PI * freq_hz * t).cos()) as f32
        })
        .collect()
}

/// Harmonic-rich voiced signal: fundamental f0 + 5 harmonics with 1/k amplitude (band-limited
/// sawtooth). The archetypal voiced-speech spectral shape: periodic AND multi-harmonic, so the
/// encoder selects its voiced (LTP) path for these frames.
fn gen_voiced_harmonic(f0_hz: f64, n: usize, amp: f32) -> Vec<f32> {
    const N_HARM: usize = 6;
    (0..n)
        .map(|i| {
            let mut s = 0f64;
            for k in 1..=N_HARM {
                let t = i as f64 / 16000.0;
                s += (1.0 / k as f64) * (2.0 * std::f64::consts::PI * k as f64 * f0_hz * t).cos();
            }
            // /2 to keep peak within ±1
            (amp as f64 * s / 2.0) as f32
        })
        .collect()
}

/// White noise, amplitude-bounded. Deterministic (LCG) so the test is reproducible.
fn gen_white_noise(n: usize, amp: f32) -> Vec<f32> {
    let mut state = 0x12345678u32;
    (0..n)
        .map(|_| {
            state = state.wrapping_mul(1664525).wrapping_add(1013904223);
            let v = (state as i32 as f64) / (i32::MAX as f64);
            (amp as f64 * v) as f32
        })
        .collect()
}

/// Silence (all zeros).
fn gen_silence(n: usize) -> Vec<f32> {
    vec![0.0f32; n]
}

/// Frequency chirp sweeping from f_lo to f_hi over n samples @16 kHz, amplitude amp.
fn gen_chirp(f_lo: f64, f_hi: f64, n: usize, amp: f32) -> Vec<f32> {
    let mut phase = 0.0f64;
    (0..n)
        .map(|i| {
            let f = f_lo + (f_hi - f_lo) * (i as f64 / n as f64);
            phase += 2.0 * std::f64::consts::PI * f / 16000.0;
            (amp as f64 * phase.cos()) as f32
        })
        .collect()
}

// synth_mic.raw generator

/// Reproduces the committed `testdata/synth_mic.raw` (s16le, 16 kHz, mono, 110 frames) byte-for-byte
/// as i16 samples. This is the in-repo recipe for the synthetic mic input: a deterministic sequence
/// of speech-like archetypes (formant-shaped voiced harmonics, unvoiced noise, voiced+noise, silence)
/// chosen so the VAD / pitch / LSF / gennoise / pulse / gains paths are all exercised and the encode
/// produces config-1/config-2 frames as well as active config-0 frames.
///
/// `synth_mic.raw` is the oracle INPUT that the captured encoder/decoder fixtures were derived from
/// (see PROVENANCE.md), so this generator must stay byte-exact: `synth_mic_raw_matches_generator`
/// asserts equality, and the env-gated `regen_synth_mic_raw` rewrites the file from it.
pub(crate) fn synth_mic_pcm() -> Vec<i16> {
    const SR: f64 = 16000.0;
    const FRAME: usize = 960;
    const SEG: usize = 10; // frames per archetype block

    // Deterministic LCG (same constants as `gen_white_noise`), threaded across all segments so the
    // whole stream is one reproducible sequence.
    struct Lcg(u32);
    impl Lcg {
        fn next_unit(&mut self) -> f64 {
            self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223);
            (self.0 as i32 as f64) / (i32::MAX as f64)
        }
    }

    // Formant-shaped voiced block: f0 + 5 harmonics, 1/k^1.3 tilt with a soft bump near 700 Hz so the
    // spectrum has width (not a pure line). `t0` is the absolute sample offset for phase continuity.
    fn voiced(out: &mut Vec<f32>, f0: f64, amp: f64, noise: f64, lcg: &mut Lcg, t0: usize) {
        for i in 0..SEG * FRAME {
            let t = (t0 + i) as f64 / SR;
            let mut s = 0f64;
            for k in 1..=5 {
                let fk = k as f64 * f0;
                let g = (1.0 / (k as f64).powf(1.3))
                    * (0.6 + 0.5 * (-((fk - 700.0) / 600.0).powi(2)).exp());
                s += g * (2.0 * std::f64::consts::PI * fk * t).cos();
            }
            let n = lcg.next_unit();
            out.push(((amp * s / 2.0) + noise * n) as f32);
        }
    }
    fn noise_block(out: &mut Vec<f32>, amp: f64, lcg: &mut Lcg) {
        for _ in 0..SEG * FRAME {
            out.push((amp * lcg.next_unit()) as f32);
        }
    }
    fn silence_block(out: &mut Vec<f32>) {
        out.resize(out.len() + SEG * FRAME, 0.0);
    }

    let mut out: Vec<f32> = Vec::with_capacity(11 * SEG * FRAME);
    let mut lcg = Lcg(0x1234_5678);
    // Segment order is load-bearing (the captured fixtures consume this exact byte stream).
    {
        let t0 = out.len();
        voiced(&mut out, 150.0, 0.30, 0.03, &mut lcg, t0);
    }
    {
        let t0 = out.len();
        voiced(&mut out, 130.0, 0.30, 0.03, &mut lcg, t0);
    }
    noise_block(&mut out, 0.12, &mut lcg);
    {
        let t0 = out.len();
        voiced(&mut out, 150.0, 0.22, 0.04, &mut lcg, t0);
    }
    {
        let t0 = out.len();
        voiced(&mut out, 130.0, 0.30, 0.03, &mut lcg, t0);
    }
    {
        let t0 = out.len();
        voiced(&mut out, 150.0, 0.30, 0.03, &mut lcg, t0);
    }
    silence_block(&mut out);
    noise_block(&mut out, 0.06, &mut lcg);
    {
        let t0 = out.len();
        voiced(&mut out, 130.0, 0.30, 0.03, &mut lcg, t0);
    }
    noise_block(&mut out, 0.02, &mut lcg);
    {
        let t0 = out.len();
        voiced(&mut out, 150.0, 0.30, 0.03, &mut lcg, t0);
    }

    out.iter()
        .map(|&v| (v * 32767.0).clamp(-32768.0, 32767.0) as i16)
        .collect()
}

/// s16le little-endian byte serialization of [`synth_mic_pcm`], matching the on-disk `.raw` layout.
fn synth_mic_bytes() -> Vec<u8> {
    let pcm = synth_mic_pcm();
    let mut bytes = Vec::with_capacity(pcm.len() * 2);
    for &s in &pcm {
        bytes.extend_from_slice(&s.to_le_bytes());
    }
    bytes
}

/// The in-repo generator must reproduce the committed `synth_mic.raw` byte-for-byte. The committed
/// file is the oracle input for the captured fixtures (PROVENANCE.md); if this drifts, those fixtures
/// no longer correspond to a reproducible input.
#[test]
fn synth_mic_raw_matches_generator() {
    let committed = include_bytes!("testdata/synth_mic.raw");
    let generated = synth_mic_bytes();
    assert_eq!(
        generated.len(),
        committed.len(),
        "synth_mic.raw length drifted: generator {} bytes vs committed {} bytes",
        generated.len(),
        committed.len()
    );
    assert!(
        generated == committed,
        "synth_mic_pcm() no longer matches committed testdata/synth_mic.raw byte-for-byte"
    );
}

/// Rewrites `testdata/synth_mic.raw` from [`synth_mic_pcm`] when `MLOW_GEN_SYNTH=1` (mirrors the
/// `VOIP_GEN_TABLES` pattern). Regenerating invalidates every captured fixture derived from this
/// input (PROVENANCE.md), so it is opt-in and otherwise a no-op.
#[test]
fn regen_synth_mic_raw() {
    if std::env::var("MLOW_GEN_SYNTH").as_deref() != Ok("1") {
        return;
    }
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/src/voip/mlow/testdata/synth_mic.raw"
    );
    std::fs::write(path, synth_mic_bytes()).expect("write synth_mic.raw");
    eprintln!("wrote {path}");
}

// Measurement helpers

/// Pearson correlation coefficient between two equal-length slices.
fn corr(a: &[f32], b: &[f32]) -> f64 {
    let (mut sxy, mut sxx, mut syy) = (0f64, 0f64, 0f64);
    for (&x, &y) in a.iter().zip(b.iter()) {
        let (x, y) = (x as f64, y as f64);
        sxy += x * y;
        sxx += x * x;
        syy += y * y;
    }
    if sxx < 1e-12 || syy < 1e-12 {
        return 0.0;
    }
    sxy / (sxx * syy).sqrt()
}

/// RMS of a signal.
fn rms(x: &[f32]) -> f64 {
    let ss: f64 = x.iter().map(|&v| (v as f64) * (v as f64)).sum();
    (ss / x.len() as f64).sqrt()
}

/// Goertzel power at `freq_hz` (single-bin DFT). Returns unnormalized power.
fn goertzel(x: &[f32], freq_hz: f64, sr: f64) -> f64 {
    let w = 2.0 * std::f64::consts::PI * freq_hz / sr;
    let c = 2.0 * w.cos();
    let (mut s1, mut s2) = (0f64, 0f64);
    for &v in x {
        let s0 = v as f64 + c * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    s1 * s1 + s2 * s2 - c * s1 * s2
}

/// Pitch-periodicity check: normalized autocorrelation at lag `lag_samples`.
/// For a perfectly periodic signal this is 1.0; for white noise it is ≈ 0.
fn normalized_autocorr(x: &[f32], lag: usize) -> f64 {
    if lag >= x.len() {
        return 0.0;
    }
    let (mut num, mut e1, mut e2) = (0f64, 0f64, 0f64);
    for i in lag..x.len() {
        let (a, b) = (x[i] as f64, x[i - lag] as f64);
        num += a * b;
        e1 += a * a;
        e2 += b * b;
    }
    if e1 < 1e-12 || e2 < 1e-12 {
        return 0.0;
    }
    num / (e1 * e2).sqrt()
}

/// Peak normalized autocorrelation over a range of lags [lag_lo, lag_hi] (inclusive).
fn peak_autocorr(x: &[f32], lag_lo: usize, lag_hi: usize) -> f64 {
    (lag_lo..=lag_hi)
        .map(|lag| normalized_autocorr(x, lag).abs())
        .fold(0f64, f64::max)
}

// Shared encode-decode helper

/// Encode `pcm` as a stream of 60 ms MLow frames and decode them back. Returns the decoded PCM
/// (same length as input). The first 60 ms (960 samples) are the cold-start onset (LPC history is
/// zero); quality measurements should skip it.
fn round_trip(pcm: &[f32]) -> Vec<f32> {
    const FRAME: usize = 960;
    let mut enc = MlowEncoder::new();
    let mut dec = MlowDecoder::new();
    let mut out = Vec::with_capacity(pcm.len());
    let mut i = 0;
    while i + FRAME <= pcm.len() {
        let frame = enc.encode(&pcm[i..i + FRAME]).expect("encode");
        let decoded = dec.decode(&frame);
        out.extend_from_slice(&decoded);
        i += FRAME;
    }
    out
}

/// Short-time RMS envelope of `x` over `win`-sample blocks.
fn envelope(x: &[f32], win: usize) -> Vec<f32> {
    (0..x.len() / win)
        .map(|i| {
            let s: f32 = x[i * win..(i + 1) * win].iter().map(|&v| v * v).sum();
            (s / win as f32).sqrt()
        })
        .collect()
}

/// Speech-like round-trip regression: the deterministic synthetic mic (`synth_mic.raw`, reproduced
/// in-repo by [`synth_mic_pcm`]: voiced harmonics + noise + silence) encoded by `MlowEncoder` and
/// decoded by `MlowDecoder` must reconstruct the spectral ENERGY CONTOUR of the input (short-time
/// envelope correlation) and not collapse to silence or constant noise. The waveform-phase
/// (sample-level) match is bounded by the excitation fine-structure not reproduced here, so the guard
/// is on the perceptually-dominant envelope.
#[test]
fn speech_energy_contour_tracks_input() {
    let raw = include_bytes!("testdata/synth_mic.raw");
    let pcm: Vec<f32> = raw
        .chunks_exact(2)
        .map(|c| i16::from_le_bytes([c[0], c[1]]) as f32 / 32768.0)
        .collect();
    let out = round_trip(&pcm);

    // The decoder's harmonic postfilter delays the output ~48 samples; align before correlating.
    const DELAY: usize = 48;
    const WIN: usize = 320;
    let ea = envelope(&pcm, WIN);
    let eb = envelope(&out[DELAY.min(out.len())..], WIN);
    let m = ea.len().min(eb.len());
    let ec = corr(&ea[..m], &eb[..m]);
    assert!(
        ec > 0.45,
        "real-speech envelope correlation {ec:.3} too low (outbound mute regression)"
    );

    // Level must be in the right ballpark (not silent, not blown up).
    let (ri, ro) = (rms(&pcm), rms(&out));
    assert!(
        ro > 0.3 * ri && ro < 3.0 * ri,
        "decoded RMS {ro:.4} not within [0.3x,3x] of input RMS {ri:.4}"
    );
}

// OUTBOUND: encode→decode quality tests

/// A 550 Hz pure tone must reconstruct with frequency concentration (Goertzel signal/control >> 1).
/// The existing test in encode.rs requires corr > 0.5 after 8 frames; this test adds a tighter
/// spectral-concentration check that would also fail for the unvoiced-only encoder on voiced inputs.
#[test]
fn tone_spectral_concentration() {
    const FREQ: f64 = 550.0;
    const N_FRAMES: usize = 5;
    const FRAME: usize = 960;
    let pcm = gen_tone(FREQ, FRAME * N_FRAMES, 0.4);
    let out = round_trip(&pcm);
    // Skip the cold-start onset frame.
    let steady = &out[FRAME..];
    let signal_power = goertzel(steady, FREQ, 16000.0);
    // Control bands: inter-harmonic neighbours that only noise occupies for a pure tone.
    let controls = [
        goertzel(steady, FREQ / 2.0, 16000.0),
        goertzel(steady, FREQ * 1.7, 16000.0),
        goertzel(steady, FREQ * 2.0 + 50.0, 16000.0),
        goertzel(steady, 1500.0, 16000.0),
    ];
    let control_power = controls.iter().cloned().fold(0f64, f64::max);
    let ratio = if control_power > 0.0 {
        signal_power / control_power
    } else {
        f64::INFINITY
    };
    assert!(
        ratio > 3.0,
        "pure tone {FREQ} Hz: spectral concentration ratio {ratio:.1}x < 3x (constant noise would fail this)"
    );
}

/// Silence must produce near-zero output energy. Catches a bug where an encoder always emits
/// non-zero pulses even for a zero-signal input.
#[test]
fn silence_stays_silent() {
    const N_FRAMES: usize = 4;
    const FRAME: usize = 960;
    let pcm = gen_silence(FRAME * N_FRAMES);
    let out = round_trip(&pcm);
    let steady = &out[FRAME..];
    let r = rms(steady);
    assert!(
        r < 0.01,
        "silence: decoded RMS {r:.6} is non-negligible (encoder should not inject energy into silence)"
    );
}

/// Regression floor: voiced speech (harmonic-rich, fundamental + harmonics) reconstructs with PITCH
/// PERIODICITY intact: the decoded autocorrelation at the pitch lag stays above 0.4. On this clean
/// synthetic signal the LPC poles alone already reach well past it, so 0.4 is a wide floor; any
/// future change that drops it below 0.4 here signals a serious encoder defect stripping
/// periodicity. The tighter spectral guard is `voiced_fundamental_survives_in_spectrum`.
#[test]
fn voiced_harmonic_has_pitch_periodicity() {
    const N_FRAMES: usize = 6;
    const FRAME: usize = 960;
    let sr = 16000.0f64;
    // Three fundamental frequencies in the voiced range (lag 80..143 samples @ 16 kHz):
    // 150 Hz -> lag ~107, 130 Hz -> lag ~123, 120 Hz -> lag ~133.
    for f0 in [150.0f64, 130.0, 120.0] {
        let pcm = gen_voiced_harmonic(f0, FRAME * N_FRAMES, 0.3);
        let out = round_trip(&pcm);
        // Skip the onset frame; measure steady-state.
        let steady = &out[FRAME..];

        let lag = (sr / f0).round() as usize;
        let autocorr_at_lag = normalized_autocorr(steady, lag).abs();

        // Sanity: the INPUT itself should be strongly periodic at this lag.
        let input_steady = &pcm[FRAME..];
        let input_autocorr = normalized_autocorr(input_steady, lag).abs();
        assert!(
            input_autocorr > 0.5,
            "f0={f0}Hz: input autocorr at lag {lag} is {input_autocorr:.3}; test signal is not voiced enough"
        );

        assert!(
            autocorr_at_lag > 0.4,
            "f0={f0}Hz (lag={lag}): decoded autocorr at pitch lag = {autocorr_at_lag:.3} < 0.4; \
             the encoder is stripping pitch periodicity"
        );
    }
}

/// Corollary: white noise must NOT have a significant autocorrelation peak in the pitched range,
/// so the encoder should not spuriously apply LTP. This tests that the voiced path is not triggered
/// for an aperiodic signal.
#[test]
fn noise_does_not_produce_pitch_periodicity() {
    const N_FRAMES: usize = 4;
    const FRAME: usize = 960;
    let pcm = gen_white_noise(FRAME * N_FRAMES, 0.3);
    let out = round_trip(&pcm);
    let steady = &out[FRAME..];

    // Peak autocorrelation over the voiced lag range (80..143 samples).
    let peak = peak_autocorr(steady, 80, 143);
    assert!(
        peak < 0.8,
        "noise: peak autocorr in voiced range = {peak:.3} >= 0.8; encoder is spuriously applying LTP"
    );
    // Must still produce some energy (not silence).
    let r = rms(steady);
    assert!(
        r > 0.001,
        "noise: decoded RMS {r:.6} is too low; encoder is discarding signal energy"
    );
}

/// A chirp sweeping 300 Hz → 800 Hz must track the input: the dominant frequency of the second
/// half of the decoded output must be higher than the first half, confirming the spectral envelope
/// adapts frame by frame.
#[test]
fn chirp_tracks_frequency() {
    const N_FRAMES: usize = 8;
    const FRAME: usize = 960;
    let pcm = gen_chirp(300.0, 800.0, FRAME * N_FRAMES, 0.3);
    let out = round_trip(&pcm);

    let find_dom_freq = |seg: &[f32]| -> f64 {
        let (mut best_power, mut best_f) = (0f64, 0f64);
        let mut f = 100.0f64;
        while f < 1500.0 {
            let p = goertzel(seg, f, 16000.0);
            if p > best_power {
                best_power = p;
                best_f = f;
            }
            f += 5.0;
        }
        best_f
    };

    // early = frames 1..3 (skip cold onset), late = last 3 frames
    let early_start = FRAME;
    let early_end = 3 * FRAME;
    let late_start = (N_FRAMES - 3) * FRAME;
    let late_end = N_FRAMES * FRAME;

    assert!(
        out.len() >= late_end,
        "decoded output too short ({} < {late_end}); the chirp decode regressed",
        out.len()
    );
    let dom_early = find_dom_freq(&out[early_start..early_end]);
    let dom_late = find_dom_freq(&out[late_start..late_end]);

    assert!(
        dom_late > dom_early,
        "chirp: dominant frequency did not rise (early={dom_early:.0}Hz late={dom_late:.0}Hz); \
         encoder is not tracking the time-varying spectral envelope"
    );
}

/// Energy tracking: the decoded RMS must roughly follow the input RMS across a range of signal
/// levels. A constant-noise output would have nearly constant RMS regardless of input level,
/// which this test catches by checking that a louder input produces louder output.
///
/// Uses broadband white noise (the unvoiced/fricative case the codec's `nrgres` level path targets):
/// its LPC residual scales directly with input amplitude. A single sinusoid or a few discrete
/// harmonics instead whiten almost entirely to the LPC's numerical floor, so their residual energy
/// (hence `nrgres`) does NOT track amplitude; carrying their level needs the per-pulse FCB gain
/// precision (the excitation refinement not implemented here), so they are not a valid
/// level-tracking probe.
#[test]
fn energy_tracks_input_level() {
    const N_FRAMES: usize = 4;
    const FRAME: usize = 960;
    let mut prev_rms = 0.0f64;
    for &amp in &[0.05f32, 0.15, 0.35] {
        let pcm = gen_white_noise(FRAME * N_FRAMES, amp);
        let out = round_trip(&pcm);
        let steady = &out[FRAME..];
        let r = rms(steady);
        // A louder input must produce louder output (monotone, not necessarily proportional).
        assert!(
            r > prev_rms,
            "energy not monotone: amp={amp} decoded RMS={r:.4} not above prev {prev_rms:.4}"
        );
        prev_rms = r;
    }
    // A silent input must produce near-silence.
    let silent_pcm = gen_silence(FRAME * N_FRAMES);
    let silent_out = round_trip(&silent_pcm);
    let silent_steady = &silent_out[FRAME..];
    assert!(
        rms(silent_steady) < prev_rms / 5.0,
        "energy not quashed for silence: silent decoded RMS {:.4} is too close to loud {prev_rms:.4}",
        rms(silent_steady)
    );
}

/// Voiced harmonic signal: the Goertzel power at the fundamental must dominate inter-harmonic bands
/// in the decoded output. This is an ALTERNATIVE formulation of
/// `voiced_harmonic_has_pitch_periodicity` that uses spectral evidence instead of time-domain
/// autocorrelation.
#[test]
fn voiced_fundamental_survives_in_spectrum() {
    const N_FRAMES: usize = 6;
    const FRAME: usize = 960;
    for f0 in [150.0f64, 130.0, 120.0] {
        let pcm = gen_voiced_harmonic(f0, FRAME * N_FRAMES, 0.3);
        let out = round_trip(&pcm);
        let steady = &out[FRAME..];

        let e_f0 = goertzel(steady, f0, 16000.0);
        // Control: inter-harmonic bands that are NOT f0 or its harmonics.
        let controls = [
            goertzel(steady, f0 * 0.5, 16000.0),
            goertzel(steady, f0 * 1.5, 16000.0),
            goertzel(steady, f0 * 2.5, 16000.0),
        ];
        let control_max = controls.iter().cloned().fold(0f64, f64::max);
        let ratio = if control_max > 0.0 {
            e_f0 / control_max
        } else {
            f64::INFINITY
        };
        assert!(
            ratio > 5.0,
            "f0={f0}Hz: fundamental/inter-harmonic ratio {ratio:.1}x < 5x in spectrum; \
             voiced signal is not reconstructing the fundamental (unvoiced noise smears it)"
        );
    }
}

// INBOUND: decoder energy-envelope tracking from captured frames

/// The decoder must reproduce meaningful energy from active frames (not silence). This is a guard
/// against a regression where the decoder always returns zeros for all active frames.
///
/// Uses e2e_vectors.json (see PROVENANCE.md): synthetic frames paired with a reference PCM decode,
/// with inactive-TOC frames zeroed to match the decoder's own DTX routing. The test checks that
/// active frames (reference RMS > 0.005) produce Rust-decoded RMS in the same order of magnitude
/// (within 10x), confirming energy-envelope tracking.
#[test]
fn decoder_tracks_energy_envelope() {
    let recs: serde_json::Value =
        serde_json::from_str(include_str!("testdata/e2e_vectors.json")).expect("e2e_vectors");
    let arr = recs.as_array().unwrap();
    let mut dec = MlowDecoder::new();
    let mut active_pairs: Vec<(f64, f64)> = Vec::new(); // (ref_rms, rust_rms)

    for rec in arr {
        let frame = hex::decode(rec["frame"].as_str().unwrap()).unwrap();
        let want: Vec<f32> = rec["pcm"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_f64().unwrap() as f32)
            .collect();
        let got = dec.decode(&frame);
        let ref_r = rms(&want);
        let rust_r = rms(&got);
        if ref_r > 0.005 {
            active_pairs.push((ref_r, rust_r));
        }
    }

    assert!(
        !active_pairs.is_empty(),
        "no active frames found in e2e_vectors.json; capture may be DTX-only"
    );

    // For every active frame, the Rust RMS must be within 10x of the reference RMS (either
    // direction). A regression to silence would give Rust RMS near 0 while the reference RMS > 0.
    let mut failures = 0;
    for (i, &(ref_r, rust_r)) in active_pairs.iter().enumerate() {
        if rust_r < ref_r / 10.0 {
            eprintln!(
                "  active frame {i}: ref RMS={ref_r:.4} Rust RMS={rust_r:.6} (Rust {:.1}x quieter)",
                ref_r / rust_r.max(1e-9)
            );
            failures += 1;
        } else if rust_r > ref_r * 10.0 {
            eprintln!(
                "  active frame {i}: ref RMS={ref_r:.4} Rust RMS={rust_r:.6} (Rust {:.1}x louder)",
                rust_r / ref_r.max(1e-9)
            );
            failures += 1;
        }
    }
    assert!(
        failures == 0,
        "{failures}/{} active frames: Rust decoder RMS is >10x off the reference (either direction)",
        active_pairs.len()
    );
}

/// The decoder must produce ZERO output for silence frames. Confirms that DTX/SID frames don't
/// inject energy.
#[test]
fn decoder_silence_frames_produce_zero() {
    let recs: serde_json::Value =
        serde_json::from_str(include_str!("testdata/e2e_vectors.json")).expect("e2e_vectors");
    let arr = recs.as_array().unwrap();
    let mut dec = MlowDecoder::new();

    for (i, rec) in arr.iter().enumerate() {
        let frame = hex::decode(rec["frame"].as_str().unwrap()).unwrap();
        let want: Vec<f32> = rec["pcm"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_f64().unwrap() as f32)
            .collect();
        let ref_r = rms(&want);
        let got = dec.decode(&frame);
        let rust_r = rms(&got);
        // A frame the reference decodes to near-silence must also be near-silence in Rust.
        if ref_r < 0.001 {
            assert!(
                rust_r < 0.001,
                "frame {i}: reference is silence (RMS={ref_r:.6}) but Rust produced RMS={rust_r:.6}"
            );
        }
    }
}

/// Voiced harmonic signal at LOW amplitude: the voiced path's LTP prediction carries energy that an
/// unvoiced-only path (constrained by its fixed gain step, placing few pulses at low amplitude)
/// could not, so the reconstruction stays correlated with the input even when the pulse budget is
/// thin. Guards that the voiced path keeps reconstructing the signal at amp 0.05.
#[test]
fn voiced_harmonic_low_amplitude_tracks_input() {
    const N_FRAMES: usize = 8;
    const FRAME: usize = 960;
    // Low amplitude: 0.05 (int16-scaled ~1638 peak), where the pulse budget is thin.
    let pcm = gen_voiced_harmonic(130.0, FRAME * N_FRAMES, 0.05);
    let out = round_trip(&pcm);
    // The decoder's harmonic postfilter both delays the output (~48 samples group delay) and resonates
    // it at the pitch period, so for this synthetic 130 Hz tone the reconstruction is a phase-shifted
    // (and partly inverted) copy. Search the best phase alignment over one pitch period and assert the
    // magnitude tracks; this guards the encoder voiced path, not the exact decoder phase.
    let steady_in = &pcm[FRAME..];
    let mut best = 0f64;
    for d in 0..128 {
        if FRAME + d >= out.len() {
            break;
        }
        let n = steady_in.len().min(out.len() - FRAME - d);
        best = best.max(corr(&steady_in[..n], &out[FRAME + d..FRAME + d + n]).abs());
    }
    assert!(
        best > 0.7,
        "low-amplitude harmonic: best phase-aligned |corr|={best:.3} < 0.7; the voiced path is not \
         reconstructing the signal"
    );
}

/// Encoding a multi-frame harmonic stream produces valid output for every frame: each frame carries
/// the active config=0 TOC (`0x50`) and decodes to a full 960-sample block, so the encode→decode
/// chain stays total across frame boundaries.
#[test]
fn voiced_harmonic_multiple_frames_no_panic() {
    const N_FRAMES: usize = 6;
    const FRAME: usize = 960;
    let pcm = gen_voiced_harmonic(130.0, FRAME * N_FRAMES, 0.3);
    // Just confirm encoding all frames produces valid output without panicking.
    let mut enc = MlowEncoder::new();
    let mut dec = MlowDecoder::new();
    let mut total_decoded = 0usize;
    for i in 0..N_FRAMES {
        let frame = enc
            .encode(&pcm[i * FRAME..(i + 1) * FRAME])
            .expect("encode");
        assert_eq!(frame[0], 0x50, "expected active config=0 TOC");
        let decoded = dec.decode(&frame);
        total_decoded += decoded.len();
    }
    assert_eq!(
        total_decoded,
        FRAME * N_FRAMES,
        "expected {FRAME}*{N_FRAMES} decoded samples"
    );
}

// The encoder's voiced/unvoiced decision is pitch/VAD-driven, so there is no byte-exact-encoder gate
// here: the wire format stays pinned by the byte-exact decoder round-trip vectors and the golden test.

/// Inbound-routing regression: an mlow stream (`inbound_capture_frames.json`) must decode to clean,
/// voice-level audio through `MlowDecoder`, and EVERY frame must be classified as mlow
/// (`!is_standard_opus_frame`) so the inbound router never hands one to the standard Opus decoder.
///
/// This guards the bug where the inbound path defaulted to the standard Opus decoder, which
/// mis-decodes mlow into energy-correct clipping noise (and rejects the config-2 TOC-0x12 frames
/// outright). Routing by the TOC byte to `MlowDecoder` instead yields voice-level audio with no
/// clipping. The stream is a captured-encoder fixture (see PROVENANCE.md): not Rust-reproducible
/// because we ship no encoder matching the peer's exact wire bytes, so it is committed and pinned by
/// the tripwire test below.
#[test]
fn captured_inbound_routes_to_mlow_and_decodes_clean() {
    use super::is_standard_opus_frame;

    let frames: Vec<String> =
        serde_json::from_str(include_str!("testdata/inbound_capture_frames.json"))
            .expect("inbound_capture_frames.json");
    assert!(!frames.is_empty(), "capture is empty");

    let mut dec = MlowDecoder::new();
    let mut all: Vec<f32> = Vec::new();
    let mut std_opus_frames = 0usize;
    let mut active_frames = 0usize;

    for hex_frame in &frames {
        let frame = hex::decode(hex_frame).unwrap();
        let toc = frame.first().copied().unwrap_or(0);
        if is_standard_opus_frame(toc) {
            std_opus_frames += 1;
        }
        let pcm = dec.decode(&frame);
        // `decode` is total: even a TOC the standard Opus decoder would reject yields valid PCM.
        // Active frames are 960 samples (60 ms); DTX/CN frames return a shorter block, never empty.
        assert!(!pcm.is_empty(), "every mlow frame decodes to non-empty PCM");
        if rms(&pcm) > 0.005 {
            active_frames += 1;
        }
        all.extend_from_slice(&pcm);
    }

    assert_eq!(
        std_opus_frames, 0,
        "inbound stream is all mlow; routing any of it to the standard Opus decoder garbles it"
    );

    // Clean decode is voice-level and never saturates; the mis-routed path would clip heavily.
    let overall_rms = rms(&all);
    let peak = all.iter().fold(0.0f32, |m, &x| m.max(x.abs()));
    let clip = all.iter().filter(|&&x| x.abs() >= 0.999).count();
    let clip_pct = 100.0 * clip as f64 / all.len() as f64;

    assert!(
        (0.01..0.20).contains(&overall_rms),
        "inbound RMS {overall_rms:.4} outside voice range (mis-routed decode would be louder)"
    );
    assert!(
        peak < 0.95,
        "inbound peak {peak:.4} near full-scale; clipping noise, not clean voice"
    );
    assert!(
        clip_pct < 0.5,
        "inbound {clip_pct:.2}% samples clipped (mis-routed decode clips heavily)"
    );
    assert!(
        active_frames > frames.len() / 10,
        "only {active_frames}/{} active frames; stream decoded to silence",
        frames.len()
    );
}

/// Branch-coverage tripwire for `inbound_capture_frames.json`: the committed stream must keep at
/// least one config-1 (TOC `0x10`) AND one config-2 (TOC `0x12`) frame, alongside the active
/// config-0 (`0x50`) frames. The clean-decode test above exercises the per-TOC decode branches, so a
/// regenerated fixture that lost the `0x12` frames would silently stop covering the config-2 path.
///
/// This fixture is NOT Rust-reproducible: it is the captured mlow encode of `synth_mic.raw` (see
/// PROVENANCE.md), and this crate ships no encoder producing those exact wire bytes. The tripwire is
/// what lets the committed bytes be trusted without an in-repo regenerator.
#[test]
fn inbound_capture_frames_cover_config1_and_config2_tocs() {
    let frames: Vec<String> =
        serde_json::from_str(include_str!("testdata/inbound_capture_frames.json"))
            .expect("inbound_capture_frames.json");
    let toc = |hex_frame: &str| -> Option<u8> {
        hex::decode(hex_frame).ok().and_then(|b| b.first().copied())
    };
    let has = |want: u8| frames.iter().any(|f| toc(f) == Some(want));
    assert!(
        has(0x10),
        "fixture lost config-1 (0x10) frames: config-1 decode branch no longer covered"
    );
    assert!(
        has(0x12),
        "fixture lost config-2 (0x12) frames: config-2 decode branch no longer covered"
    );
    assert!(
        has(0x50),
        "fixture lost active config-0 (0x50) frames: capture is not a normal call stream"
    );
}

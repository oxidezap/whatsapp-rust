//! Self-contained no-regression guard for the mlow codec; proves the encode->decode pipeline has
//! not drifted WITHOUT any reference fixture file. A deterministic synthetic signal (voiced tone,
//! unvoiced noise, silence) runs through the full round-trip and a compact digest, human-readable
//! scalars plus an exact byte checksum, is asserted against committed golden constants.
//!
//! This is a regression tripwire, not a correctness oracle: it locks in the validated codec output,
//! so any drift flips the checksum while the scalars stay eyeball-checkable. Bit-exactness is proven
//! by the separate per-stage suites; the point here is that the anti-regression guard survives
//! without the multi-MB ground-truth dumps.
//!
//! Regenerate after an intentional codec change:
//!   VOIP_GOLDEN=1 cargo test -p wacore --features voip golden_roundtrip -- --nocapture
#![cfg(test)]

use super::{MlowDecoder, MlowEncoder};

const FRAME: usize = 960;
const PACKETS: usize = 24;

/// Deterministic input cycling voiced tone / unvoiced noise / voiced+noise / silence, so the
/// round-trip exercises the voiced and unvoiced encode paths from code alone (no data file). The
/// silence cycle is encoded as a low-energy active frame, not DTX (the encoder has no DTX/SID path).
fn synth_input() -> Vec<f32> {
    use std::f32::consts::PI;
    let mut out = Vec::with_capacity(PACKETS * FRAME);
    let mut seed: u32 = 0x9e37_79b9;
    for i in 0..PACKETS * FRAME {
        let t = i as f32 / 16000.0;
        seed = seed.wrapping_mul(1664525).wrapping_add(1013904223);
        let noise = (seed >> 8) as f32 / 8_388_608.0 - 1.0; // [-1, 1)
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

/// `synth_input` with the low mantissa bit of about half the samples flipped: the smallest change
/// an f32 signal can carry. Which samples are hit is a pure function of `(index, seed)`, so each
/// seed is a different, reproducible one-ULP neighbour of the same signal.
///
/// One quarter of the corpus is exact-zero silence, so about an eighth of it goes from `0.0` to the
/// smallest subnormal rather than taking an ordinary mantissa step. That is still one ULP, and it
/// is not what drives the divergence the test measures: the silence frames are the ones that stay
/// closest to the unperturbed output (70-86 dB), while the frames that move are tone and noise,
/// where every flip is a mantissa step on a normal float.
fn perturb_one_ulp(x: &[f32], seed: u32) -> Vec<f32> {
    x.iter()
        .enumerate()
        .map(|(i, v)| {
            let mut h = (i as u32).wrapping_mul(0x9e37_79b9) ^ seed.wrapping_mul(0x85eb_ca6b);
            h ^= h >> 15;
            h = h.wrapping_mul(0xc2b2_ae35);
            h ^= h >> 13;
            let bits = v.to_bits();
            f32::from_bits(if h & 1 == 0 { bits ^ 1 } else { bits })
        })
        .collect()
}

fn fnv1a(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

struct Digest {
    samples: usize,
    rms: f32,
    peak: f32,
    clip_pct: f32,
    checksum: u64,
}

fn digest(out: &[i16]) -> Digest {
    let n = out.len().max(1);
    let mut sq = 0f64;
    let mut peak: i32 = 0;
    let mut clip = 0usize;
    let mut bytes = Vec::with_capacity(out.len() * 2);
    for &s in out {
        let a = (s as i32).abs();
        sq += (s as f64) * (s as f64);
        peak = peak.max(a);
        if a >= 32767 {
            clip += 1;
        }
        bytes.extend_from_slice(&s.to_le_bytes());
    }
    Digest {
        samples: out.len(),
        rms: ((sq / n as f64).sqrt() / 32768.0) as f32,
        peak: peak as f32 / 32768.0,
        clip_pct: 100.0 * clip as f32 / n as f32,
        checksum: fnv1a(&bytes),
    }
}

// Golden digest of the validated codec (regenerate with VOIP_GOLDEN=1; see module docs). The
// scalars are tolerant and platform-robust; the checksum is exact and stable on the CI target.
const GOLDEN_SAMPLES: usize = 23040;
const GOLDEN_CLIP: f32 = 0.0;

// The scalars are tolerant and platform-robust; the checksums are exact, and exact means exact for
// one build: the encoder resolves ties on f32 comparisons, so the bitstream is a function of the
// arithmetic the compiler emits, not only of the source. Changing `-C target-cpu` alone moves these
// constants. They are a drift tripwire on the CI target, not a portable identity of the codec.
const GOLDEN_RMS: f32 = 0.156401;
const GOLDEN_PEAK: f32 = 0.524200;
const GOLDEN_CHECKSUM: u64 = 0x4302_cced_6791_52b4;
// Pins the ENCODER bitstream independently of the decoder: fnv1a over the concatenation of every
// emitted packet's wire bytes. Drift here means the encoder changed even if decode output did not.
const GOLDEN_FRAMES_CHECKSUM: u64 = 0x5583_7f25_b89a_0621;

/// Encode then decode the whole corpus, returning the decoded i16 output and the concatenated
/// packet bytes.
fn roundtrip(input: &[f32]) -> (Vec<i16>, Vec<u8>) {
    let mut enc = MlowEncoder::new();
    let mut dec = MlowDecoder::new();
    let mut out: Vec<i16> = Vec::new();
    let mut pkt_bytes: Vec<u8> = Vec::new();
    for frame in input.chunks(FRAME) {
        if frame.len() < FRAME {
            break;
        }
        let pkt = enc.encode(frame).expect("encode");
        pkt_bytes.extend_from_slice(&pkt);
        for s in dec.decode(&pkt) {
            out.push((s * 32767.0).clamp(-32768.0, 32767.0) as i16);
        }
        // The clean synthetic corpus must never make the range decoder raise its error flag.
        assert!(
            !dec.had_error(),
            "clean corpus raised the range decoder error flag"
        );
    }
    (out, pkt_bytes)
}

/// Signal-to-difference ratio in dB, treating `a` as the signal and `a - b` as the error.
fn snr_db(a: &[i16], b: &[i16]) -> f64 {
    let mut sig = 0.0f64;
    let mut err = 0.0f64;
    for (x, y) in a.iter().zip(b) {
        sig += (*x as f64) * (*x as f64);
        let d = (*x as f64) - (*y as f64);
        err += d * d;
    }
    if err == 0.0 {
        return f64::INFINITY;
    }
    10.0 * (sig / err).log10()
}

#[test]
fn golden_roundtrip_no_regression() {
    let input = synth_input();
    let (out, pkt_bytes) = roundtrip(&input);
    let d = digest(&out);
    let frames_checksum = fnv1a(&pkt_bytes);

    if std::env::var_os("VOIP_GOLDEN").is_some() {
        println!(
            "GOLDEN_SAMPLES={} GOLDEN_RMS={:.6} GOLDEN_PEAK={:.6} GOLDEN_CLIP={:.4} GOLDEN_CHECKSUM={:#018x} GOLDEN_FRAMES_CHECKSUM={:#018x}",
            d.samples, d.rms, d.peak, d.clip_pct, d.checksum, frames_checksum
        );
        return;
    }

    assert_eq!(d.samples, GOLDEN_SAMPLES, "output length drifted");
    assert!(
        (d.rms - GOLDEN_RMS).abs() < 1e-3,
        "RMS drifted {:.6} vs golden {:.6}",
        d.rms,
        GOLDEN_RMS
    );
    assert!(
        (d.peak - GOLDEN_PEAK).abs() < 1e-3,
        "peak drifted {:.6} vs golden {:.6}",
        d.peak,
        GOLDEN_PEAK
    );
    assert!(
        (d.clip_pct - GOLDEN_CLIP).abs() < 0.1,
        "clip drifted {:.4} vs golden {:.4}",
        d.clip_pct,
        GOLDEN_CLIP
    );
    assert_eq!(
        d.checksum, GOLDEN_CHECKSUM,
        "output bytes drifted (codec regression). scalars rms={:.6} peak={:.6} clip={:.4}. If intentional, regenerate the golden with VOIP_GOLDEN=1.",
        d.rms, d.peak, d.clip_pct
    );
    assert_eq!(
        frames_checksum, GOLDEN_FRAMES_CHECKSUM,
        "encoder bitstream drifted (encoder regression). If intentional, regenerate the golden with VOIP_GOLDEN=1.",
    );
}

/// The encoder resolves discrete choices -- pulse positions, codebook indices, voicing -- on f32
/// comparisons that routinely sit on ties, so the last bit of any intermediate decides which branch
/// wins for a whole subframe. Perturbing the INPUT by one ULP, with the encoder byte-for-byte
/// unchanged, is enough: the decoded output then lands 21-27 dB from the unperturbed one, with
/// individual frames as low as ~10 dB, while still being a valid encoding of the same signal.
///
/// This is what the exact checksums above do and do not mean. They pin one arithmetic path
/// exactly, which is what makes them a useful tripwire; they say nothing about whether a different
/// path is worse. Two encodings that land far apart on this measure can be equally good encodings
/// of the same signal -- which is why a change of arithmetic is judged against the SOURCE, by
/// perceptual measurement, and not against the previous bitstream.
///
/// Asserting an upper bound on the SNR looks backwards, but it is the load-bearing half: if a
/// one-ULP input change ever stopped moving the output, the tie-breaking would have changed and
/// the reasoning above would no longer hold.
#[test]
fn encoder_output_is_last_bit_chaotic() {
    let input = synth_input();
    let (base, _) = roundtrip(&input);
    for seed in 1..=4u32 {
        let (near, _) = roundtrip(&perturb_one_ulp(&input, seed));
        assert_eq!(near.len(), base.len(), "seed {seed}: output length drifted");
        let snr = snr_db(&base, &near);
        assert!(
            (10.0..40.0).contains(&snr),
            "seed {seed}: one-ULP input change gave {snr:.2} dB against the unperturbed output; \
             outside 10-40 dB the encoder is either no longer tie-sensitive or genuinely broken"
        );
    }
}

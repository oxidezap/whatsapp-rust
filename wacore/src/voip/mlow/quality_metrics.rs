//! Decoder-quality regression harness: decode the captured inbound stream through [`MlowDecoder`]
//! and measure it against the reference decode (`ref_usesmpl_expected.raw`; see PROVENANCE.md).
//! These metrics are the objective signal that the pure-Rust decoder reproduces the codec's
//! excitation/noise/spectral shape, not just its energy envelope.
//!
//! The reference is the byte-exact decode of the same 373 frames. It applies a 48-sample group
//! delay in its harmonic postfilter, so the harness searches a small reference delay before
//! correlating.

#![cfg(test)]

use super::decoder::MlowDecoder;

const SR: f64 = 16000.0;

/// Decode every captured frame and concatenate the PCM (float [-1,1]).
fn decode_capture() -> Vec<f32> {
    let frames: Vec<String> =
        serde_json::from_str(include_str!("testdata/inbound_capture_frames.json"))
            .expect("inbound_capture_frames.json");
    let mut dec = MlowDecoder::new();
    let mut out = Vec::new();
    for hex_frame in &frames {
        let frame = hex::decode(hex_frame).unwrap();
        out.extend_from_slice(&dec.decode(&frame));
    }
    out
}

/// Load the decoder reference (s16le @ 16 kHz) as float [-1,1].
fn load_ref() -> Vec<f32> {
    let raw = include_bytes!("testdata/ref_usesmpl_expected.raw");
    raw.chunks_exact(2)
        .map(|b| i16::from_le_bytes([b[0], b[1]]) as f32 / 32768.0)
        .collect()
}

fn rms(x: &[f32]) -> f64 {
    if x.is_empty() {
        return 0.0;
    }
    let ss: f64 = x.iter().map(|&v| (v as f64) * (v as f64)).sum();
    (ss / x.len() as f64).sqrt()
}

/// Zero-mean Pearson correlation over the overlap, with the reference shifted by `lag` samples
/// (ref[i+lag] vs out[i]).
fn corr_at_lag(refp: &[f32], out: &[f32], lag: usize) -> f64 {
    let n = refp.len().saturating_sub(lag).min(out.len());
    if n < 16 {
        return 0.0;
    }
    let r = &refp[lag..lag + n];
    let o = &out[..n];
    let mr: f64 = r.iter().map(|&v| v as f64).sum::<f64>() / n as f64;
    let mo: f64 = o.iter().map(|&v| v as f64).sum::<f64>() / n as f64;
    let (mut sxy, mut sxx, mut syy) = (0f64, 0f64, 0f64);
    for i in 0..n {
        let dr = r[i] as f64 - mr;
        let dz = o[i] as f64 - mo;
        sxy += dr * dz;
        sxx += dr * dr;
        syy += dz * dz;
    }
    if sxx < 1e-12 || syy < 1e-12 {
        return 0.0;
    }
    sxy / (sxx * syy).sqrt()
}

/// Best delay-aligned correlation: search reference delay 0..=max_lag, return (best_lag, corr).
fn best_delay_corr(refp: &[f32], out: &[f32], max_lag: usize) -> (usize, f64) {
    let mut best = (0usize, f64::NEG_INFINITY);
    for lag in 0..=max_lag {
        let c = corr_at_lag(refp, out, lag);
        if c > best.1 {
            best = (lag, c);
        }
    }
    best
}

/// Fraction of near-zero samples (|x| < 1e-4) over the active (RMS-bearing) regions. Buzzy/robotic
/// excitation leaves long dead gaps between FCB pulses; broadband noise fill reduces this.
fn near_zero_fraction(x: &[f32]) -> f64 {
    const W: usize = 960;
    let (mut zeros, mut total) = (0usize, 0usize);
    for chunk in x.chunks(W) {
        if rms(chunk) < 0.003 {
            continue;
        }
        for &v in chunk {
            if v.abs() < 1e-4 {
                zeros += 1;
            }
            total += 1;
        }
    }
    if total == 0 {
        return 0.0;
    }
    zeros as f64 / total as f64
}

/// Goertzel power at `freq_hz` over the whole signal (unnormalized).
fn goertzel(x: &[f32], freq_hz: f64) -> f64 {
    let w = 2.0 * std::f64::consts::PI * freq_hz / SR;
    let c = 2.0 * w.cos();
    let (mut s1, mut s2) = (0f64, 0f64);
    for &v in x {
        let s0 = v as f64 + c * s1 - s2;
        s2 = s1;
        s1 = s0;
    }
    s1 * s1 + s2 * s2 - c * s1 * s2
}

/// 3-band energy via a bank of Goertzel bins: <700, 700-4000, >4000 Hz.
fn band_energy(x: &[f32]) -> (f64, f64, f64) {
    let (mut lo, mut mid, mut hi) = (0f64, 0f64, 0f64);
    let mut f = 100.0;
    while f < 7900.0 {
        let p = goertzel(x, f);
        if f < 700.0 {
            lo += p;
        } else if f < 4000.0 {
            mid += p;
        } else {
            hi += p;
        }
        f += 100.0;
    }
    (lo, mid, hi)
}

/// Print the full metric battery (used to track each change's before/after).
fn report(out: &[f32], refp: &[f32]) {
    let n = out.len().min(refp.len());
    let (lag, c) = best_delay_corr(refp, out, 128);
    let (lo, mid, hi) = band_energy(&out[..n]);
    let (rlo, rmid, rhi) = band_energy(&refp[..n]);
    eprintln!(
        "METRICS len={} rms={:.5} ref_rms={:.5} best_lag={} corr={:.4} gap={:.4} \
         band(lo/mid/hi)={:.3e}/{:.3e}/{:.3e} ref_band={:.3e}/{:.3e}/{:.3e} \
         hi_ratio={:.3} mid_ratio={:.3} lo_ratio={:.3}",
        out.len(),
        rms(out),
        rms(refp),
        lag,
        c,
        near_zero_fraction(out),
        lo,
        mid,
        hi,
        rlo,
        rmid,
        rhi,
        hi / rhi.max(1e-30),
        mid / rmid.max(1e-30),
        lo / rlo.max(1e-30),
    );
}

/// Baseline / progress probe; prints metrics, never asserts. Run with
/// `cargo test -p wacore --features voip decode_metrics_report -- --nocapture --ignored`.
#[test]
#[ignore = "diagnostic: prints metrics, used while iterating on the decoder"]
fn decode_metrics_report() {
    let out = decode_capture();
    let refp = load_ref();
    report(&out, &refp);
}

/// Invariant: the decode reproduces the reference's spectral signature, not just its energy. Band
/// ratios, inter-pulse gap fraction, RMS, and delay-aligned correlation all stay within the
/// thresholds that prove broadband noise fill and excitation shape are correct.
#[test]
fn decode_matches_ref_usesmpl() {
    let out = decode_capture();
    let refp = load_ref();
    assert_eq!(
        out.len(),
        refp.len(),
        "decode length must match the reference"
    );

    let (lo, mid, hi) = band_energy(&out);
    let (rlo, _rmid, rhi) = band_energy(&refp);
    let our_rms = rms(&out);
    let ref_rms = rms(&refp);
    let gap = near_zero_fraction(&out);
    let (_lag, c) = best_delay_corr(&refp, &out, 128);
    let hi_ratio = hi / rhi.max(1e-30);
    let lo_ratio = lo / rlo.max(1e-30);

    assert!(
        (0.35..3.0).contains(&hi_ratio),
        "hi-band ratio {hi_ratio:.3} out of [0.35, 3.0): out={hi:.3e} ref={rhi:.3e}, mid={mid:.3e}"
    );
    assert!(
        lo_ratio < 2.5,
        "low-band ratio {lo_ratio:.3} too boomy: out={lo:.3e} ref={rlo:.3e}"
    );
    assert!(
        gap < 0.32,
        "inter-pulse gap fraction {gap:.4} >= 0.32, noise fill missing"
    );
    assert!(
        (ref_rms * 0.4..ref_rms * 2.2).contains(&our_rms),
        "rms {our_rms:.4} vs ref {ref_rms:.4} out of range"
    );
    assert!(
        c > 0.55,
        "delay-aligned corr {c:.4} <= 0.55, excitation shape still wrong"
    );
}

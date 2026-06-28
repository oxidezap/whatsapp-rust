//! Pure-Rust codec for Meta's proprietary "smpl_audio_codec" (MLow), the Opus-forked
//! split-band CELP codec WhatsApp 1:1 calls use by default. Pinned against captured WASM decode
//! vectors (provenance in `testdata/PROVENANCE.md`).
//!
//! Both directions are implemented: the DECODER (inbound peer audio is mlow; stock Opus
//! mis-decodes it into energy-correct noise) and the ENCODER (outbound must be mlow too, since the
//! peer won't play standard Opus). The encoder analyzes PCM into voiced (LTP) or unvoiced frames and
//! entropy-encodes them. Every layer is validated byte-for-byte against captured vectors, the only
//! real guard since a wrong-but-consistent codec would otherwise pass round-trips.
//!
mod analysis;
mod decoder;
mod encode;
mod golden;
mod param_decode_match;
mod params;
mod quality_metrics;
mod quality_tests;
mod rangecoder;
mod red;
mod smpl_cc_tables;
mod smpl_celp;
mod smpl_celpdec;
mod smpl_decode;
mod smpl_gains;
mod smpl_gennoise;
mod smpl_harm_postfilter;
mod smpl_harmcomb;
mod smpl_lpc;
mod smpl_lsf_quant;
mod smpl_lsf_seed;
mod smpl_mem;
mod smpl_nrgres;
mod smpl_perc;
mod smpl_pitch;
mod smpl_pitch_enc;
mod smpl_pitch_seed;
mod smpl_pulse;
mod smpl_signal_mode;
mod smpl_synth;
mod smpl_tables_blob;
mod smpl_vad;
mod toc;

pub use decoder::MlowDecoder;
pub use encode::{MlowEncoder, MlowError};

/// Whether the first payload byte is a STANDARD Opus/CELT TOC (`(b & 0xC0) == 0xC0`) rather than a
/// Meta mlow "smpl" TOC. The inbound path routes standard-Opus frames to stock Opus and mlow
/// frames to [`MlowDecoder`].
pub(crate) fn is_standard_opus_frame(first_byte: u8) -> bool {
    first_byte & 0xC0 == 0xC0
}

#[cfg(test)]
mod tests {
    use super::is_standard_opus_frame;

    // The inbound router splits on exactly bit-pattern 0xC0: every 0xC0..=0xFF byte is stock
    // Opus/CELT, and the mlow "smpl" TOCs we actually emit/receive (0x10, 0x12, 0x50) are not. This
    // pins the boundary so a router-predicate edit can't silently send mlow to stock Opus.
    #[test]
    fn opus_split_boundary_table() {
        for b in 0xC0u8..=0xFF {
            assert!(is_standard_opus_frame(b), "0x{b:02x} must be standard opus");
        }
        for &mlow_toc in &[0x10u8, 0x12, 0x50] {
            assert!(
                !is_standard_opus_frame(mlow_toc),
                "mlow TOC 0x{mlow_toc:02x} must not route to stock Opus"
            );
        }
    }
}

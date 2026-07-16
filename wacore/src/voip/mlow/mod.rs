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

/// MLOW's in-profile escape for a standard Opus/CELT packet. This is meaningful only after call
/// negotiation selected MLOW; it must never be used to choose the call's media profile.
pub(crate) fn is_embedded_opus(payload: &[u8]) -> bool {
    payload.first().is_some_and(|byte| byte & 0xC0 == 0xC0)
}

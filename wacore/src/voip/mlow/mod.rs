//! Pure-Rust codec for Meta's proprietary "smpl_audio_codec" (MLow), the Opus-forked
//! split-band CELP codec WhatsApp 1:1 calls use by default. Pinned against captured WASM decode
//! vectors (provenance in `testdata/PROVENANCE.md`).
//!
//! Both proprietary directions are implemented: the DECODER (stock Opus mis-decodes MLOW into
//! energy-correct noise) and the ENCODER. Compatible standard Opus/CELT can instead use MLOW's
//! in-profile escape. Every layer is validated byte-for-byte against captured vectors, the only real
//! guard since a wrong-but-consistent codec would otherwise pass round-trips.
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

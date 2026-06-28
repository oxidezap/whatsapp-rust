# MLow test-fixture provenance

Every fixture in this directory is either reproducible in-repo or derived through an external oracle
from the in-repo synthetic input `synth_mic.raw`. This file maps each fixture to its oracle and the
exact recipe to regenerate it, so the irreducibly-external vectors are honestly reproducible WITHOUT
vendoring the oracle toolchains.

There is no real captured call audio here: `synth_mic.raw` is fully synthetic and deterministic.

## The root input: `synth_mic.raw`

- Oracle: none (it is generated IN-REPO).
- Recipe: `synth_mic_pcm()` in `quality_tests.rs`. `synth_mic_raw_matches_generator` asserts the
  committed bytes equal the generator output; `MLOW_GEN_SYNTH=1 cargo test -p wacore --features voip
  regen_synth_mic_raw` rewrites the file. It is s16le / 16 kHz / mono / 110 frames of 960 samples:
  a deterministic sequence of formant-shaped voiced harmonics, unvoiced noise, voiced+noise, and
  silence chosen to exercise the VAD / pitch / LSF / gennoise / pulse / gains paths.

Every external fixture below is derived FROM `synth_mic.raw` (or the frames encoded from it). If
`synth_mic.raw` changes, all of them must be regenerated together through their oracles.

## Runtime tables (not test vectors)

`lsf_seed.bin`, `pitch_seed.bin`, `cc_seed.bin` are the codec's runtime constant tables, dumped from
the `smpl` C reference. See `README.md`; regenerate with `VOIP_GEN_TABLES=1 cargo test -p wacore
--features voip gen_runtime_tables`. Their human-readable `.json` sources are gitignored.

## Encoder-side ground truth (oracle: `smpl` C reference dump tools)

These pin the Rust encoder front-end against the C reference, run on `synth_mic.raw`. Each is a JSON
dump emitted by a small C harness linked against the `smpl` reference; the Rust test parses it and
compares field-by-field (tight float tolerance where the C uses PFFFT and we use a portable FFT).

| fixture | consumer / test | C dump tool (flags: input output [limits]) |
| --- | --- | --- |
| `fe_dump.json` | `smpl_lpc.rs::front_end_a_matches_c` | `fe_dump_harness synth_mic.raw fe_dump.json 40 40` |
| `lsf_quant_io.json` | `smpl_lsf_quant.rs::lsf_quant_matches_c`, `smpl_lpc.rs` | `lsf_quant_io_harness synth_mic.raw lsf_quant_io.json 60 40` |
| `pitchio_ground_truth.json` | `smpl_pitch_enc.rs::pitch_estimator_matches_c_ground_truth` | `pitchio_harness synth_mic.raw pitchio_ground_truth.json 20000 8 40` |
| `sigmode_ground_truth.json` | `smpl_signal_mode.rs::signal_mode_matches_c_ground_truth` | `sigmode_harness_full synth_mic.raw sigmode_ground_truth.json 20000 8` |
| `vad_ground_truth.json` | `smpl_vad.rs::vad_matches_c_ground_truth` | the VAD dump harness on `synth_mic.raw`; the committed fixture is truncated to the bit-exact prefix where the carried fixed-point state stays in lockstep with C |
| `gennoise_vectors.json` | `smpl_gennoise.rs::gen_noise_matches_c` | the gennoise dump harness on `synth_mic.raw` |
| `gennoise_params_dump.json` | `param_decode_match.rs::nrgres_fcbg_match_c_reference` | the decode-param (`dec_param_harness`) dump |

## Decoder-side reference (oracle: the `smpl` C `useSmpl` decode and libopus)

The frames decoded here are the external mlow encode of `synth_mic.raw` (the same frames that the
inbound test consumes). The reference PCM is what the faithful codec produces.

| fixture | consumer / test | oracle recipe |
| --- | --- | --- |
| `ref_usesmpl_expected.raw` | `decoder.rs`, `quality_metrics.rs::decode_matches_ref_usesmpl` | libopus built with `useSmpl`, decoding the frames encoded from `synth_mic.raw`; s16le @ 16 kHz |
| `e2e_vectors.json` | `decoder.rs`, `quality_tests.rs` (energy-envelope tests) | each record = an mlow frame + the libopus useSmpl reference PCM for it; inactive-TOC frames zeroed to match DTX routing |
| `harm_postfilter_vectors.raw` | `smpl_harm_postfilter.rs::harm_postfilter_matches_c` | C harmonic-postfilter dump (`dump_harness_harm`) |
| `hp_postfilter_vectors.raw` | `smpl_harmcomb.rs::hp_postfilter_matches_c` | C high-pass postfilter dump (`dump_harness_hp`) |
| `exc_pre_lags.json` | `smpl_celpdec.rs::exc_pre_matches_c` | C pre-noise excitation dump from the decode of the encoded frames |

## Decoder cross-check vectors (oracle: the reference decoder)

These pin the bit-exact wire decode. They are the encoded frames of `synth_mic.raw` decoded through
the reference decoder, one record per frame, compared byte-for-byte by the Rust decoder.

| fixture | consumer / test |
| --- | --- |
| `lsf_vectors.json` | `smpl_decode.rs` |
| `pitch_vectors.json` | `smpl_pitch.rs` |
| `pulse_vectors.json` | `smpl_pulse.rs` |
| `gains_vectors.json` | `smpl_gains.rs` |
| `rc_vectors.json` | `rangecoder.rs::range_decoder_matches_*` |
| `toc_vectors.json` | `toc.rs::toc_matches_*` (full 256-TOC table, input-independent) |

## External-encoder frames

| fixture | consumer / test | oracle recipe |
| --- | --- | --- |
| `inbound_capture_frames.json` | `quality_tests.rs::captured_inbound_routes_to_mlow_and_decodes_clean` + `inbound_capture_frames_cover_config1_and_config2_tocs` | the external `smpl` mlow encoder run over `synth_mic.raw`, hex frames |

`inbound_capture_frames.json` is NOT Rust-reproducible: this crate ships no encoder that emits those
exact wire bytes (config-1 `0x10` and config-2 `0x12` frames included). The tripwire test asserts the
committed stream still carries `0x10`, `0x12`, and `0x50` TOCs so the per-config decode branches stay
covered; regenerating it requires the external encoder above on `synth_mic.raw`.

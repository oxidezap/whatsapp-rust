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
| `toc_vectors.json` | `toc.rs::toc_matches_*` (256-TOC table, input-independent; see the caveat below) |

### `toc_vectors.json`: the escape range is stale, deliberately

The `ms` column is authoritative for the 192 in-profile bytes and **obsolete for the 64 escape bytes**
(`0xC0..=0xFF`). It records the RFC 6716 reading (`config = b >> 3`), which is not the layout MLOW's
in-profile CELT escape uses: that is `0xC0 | mode << 2 | stereo << 1 | multi`, as
`packetize_opus_for_mlow` writes it and `opus_smpl_decode_TOC` in the shipped module reads it. The
two disagree on the duration for 56 of the 64.

The fixture was **not** edited to agree with the corrected parser. `toc_matches_go_all_256` skips the
`ms` assertion over that range and names why, and `escape_durations_agree_with_the_escape_writer`
covers it instead by round-tripping every CELT configuration through this crate's own escape writer —
an independent statement rather than the parser checking itself.

Editing an oracle until it agrees with the code is the most expensive failure mode this directory
has: a silence fixture in here had been zeroed to match a buggy decoder, and the test over it passed
for months. Regenerate from the pinned oracle or narrow the assertion and say so; never split the
difference.

## External-encoder frames

| fixture | consumer / test | oracle recipe |
| --- | --- | --- |
| `inbound_capture_frames.json` | `quality_tests.rs::captured_inbound_routes_to_mlow_and_decodes_clean` + `inbound_capture_frames_cover_config1_and_config2_tocs` | the external `smpl` mlow encoder run over `synth_mic.raw`, hex frames |

`inbound_capture_frames.json` is NOT Rust-reproducible: this crate ships no encoder that emits those
exact wire bytes (config-1 `0x10` and config-2 `0x12` frames included). The tripwire test asserts the
committed stream still carries `0x10`, `0x12`, and `0x50` TOCs so the per-config decode branches stay
covered; regenerating it requires the external encoder above on `synth_mic.raw`.

## Multi-frame (120 ms) packets — regenerable in one command

| fixture | consumer / test | oracle recipe |
| --- | --- | --- |
| `mlow_120ms_frames.json` | `decoder.rs::multi_frame_decode_matches_the_reference` | `scripts/regenerate-mlow-vectors.sh` |
| `ref_120ms_expected.raw` | same test | same run of the same script |
| `mlow_dtx_off_frames.json` | `decoder.rs::dtx_off_frames_decode_to_audio` | same script, DTX-off pass |
| `ref_dtx_off_expected.raw` | same test | same run of the same script |

The DTX-off pair is what pins that a coded-inactive frame carries a decodable body rather than
silence: the same script emits it in the same run, so regenerating one regenerates all four.

```sh
MLOW_REFERENCE=/path/to/opus_mlow scripts/regenerate-mlow-vectors.sh
```

The harness it builds lives in `scripts/mlow-vectors/mlow_frames.c`: it encodes `synth_mic.raw` at
the requested duration through the `smpl` C reference and decodes each packet back, emitting both
halves of the vector in one pass.

The committed bytes were produced against one specific oracle —
`github.com/edgardmessias/opus_mlow` at `84b076e0809412df22e8a0d26f944610c4a3e40f`. Reproduction is
byte for byte **against that revision**, which is what makes a changed fixture a real change rather
than tool drift; against a different checkout the reference itself may have moved, so the script
prints the revision it built with and warns when it does not match.

The committed vector is an intentional 8-packet prefix, not the whole input: `synth_mic.raw` chunked
into 120 ms frames would yield ~55 packets, which is far more than the decode path needs and 7x the
bytes. A regeneration that produces more is the script defaulting to the whole file — pass the
packet count, as the script does.

Both halves must be regenerated together — `decoder.rs::multi_frame_fixture_halves_stay_in_step`
fails if the PCM length stops matching the frame count, and asserts every frame is still TOC `0x58`
so the fixture cannot quietly drift off the multi-frame path.

## What is still not reproducible, and why

The fixtures above the multi-frame section predate the harness and were produced by tools that no
longer exist here. The harness reproduces their SHAPE — run at 60 ms with DTX off it emits the same
TOC mix as `inbound_capture_frames.json` (13x `0x10`, 2x `0x12`, 95x `0x50`) — but not their exact
bytes: packet sizes bracket the committed ones without landing on them, so the original run used an
encoder configuration (or reference build) that was not recorded. Regenerating them would therefore
REPLACE those vectors rather than reproduce them, which is a deliberate decision to make with the
correlation thresholds in hand, not a mechanical refresh.

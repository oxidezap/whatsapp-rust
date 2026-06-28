# MLow codec runtime tables

The `*.bin` files here are the MLow (smpl_audio_codec) **runtime constant tables**, not test
fixtures. The codec loads each one once at startup (via `OnceLock`). Each `.bin` is a
**zlib-compressed protobuf blob** (`tables.proto`).

The LSF and pitch tables are stored as a small trained **seed ROM**; the expanded runtime tables are
derived at load by rerunning the init expansion, so the committed blobs stay small.

| `.bin` | loaded by | expands to |
| --- | --- | --- |
| `lsf_seed.bin` | `smpl_lsf_seed::lsf_built` | synth / lsf-decode / lsf-cb tables |
| `pitch_seed.bin` | `smpl_pitch_enc::load_pitch_tables` | pitch tables |
| `cc_seed.bin` | `smpl_cc_tables::load_cc_tables` | nrgres/gains + LTP gain (HR + LR) + pulse CDFs |

These match the smpl C reference.

## Source of truth

The matching `*.json` files are the human-readable dumps from the `smpl` C reference. They are the
source the `.bin` are generated from and are **gitignored** (the committed `.bin` are what the build
embeds via `include_bytes!`). Keep the JSON around locally to regenerate the blobs.

The pitch lag/contour heap window is built from `pitch_seed` at load (the contour is a deterministic
expansion of the blocksegs seed), replacing the old full ~105 KB heap snapshot. The p6!=0 LR
gain/filter/weight tables are built in `CcTables` from the same DCMF/codebook consts as the HR ones.
The full heap dump `smpl_cc_blob.json` (also gitignored) is the byte-identical oracle for the built
window and the logical `cc_seed` tables, so keep it locally; the gates skip when it is absent.

## Regenerate the `.bin` from the JSON

Run the env-gated generator test from the `wacore` crate root (the JSON paths are crate-relative):

```sh
VOIP_GEN_TABLES=1 cargo test -p wacore --features voip gen_runtime_tables -- --nocapture
```

It is deterministic (fixed zlib level 9 + protobuf's canonical encoding): re-running yields
byte-identical `.bin`.

# Audio/video differential oracle

The media oracle belongs to host tooling. WhatsApp-specific callback layouts
and capture pins live in `tools/oracle-core`; reusable static wasm analysis
comes from the pinned `unwasm-core`; capture transport and verification come
from `whatspec::wa-store`. No part of this graph reaches a published runtime
crate.

The foundation records bytes at the boundary where a captured wasm module
calls its host. `MediaWatch` names the callback arguments containing the
payload pointer and length, plus optional sequence and timestamp arguments.
`Runtime::watch_media` installs the complete watch set before execution, and
`Runtime::take_media_observations` returns the payloads in call order.

Each record is bounded to 16 MiB; a trace is bounded to 4096 records and
256 MiB. A missing argument, negative pointer/length, address overflow,
out-of-bounds read or exceeded budget becomes a trace error. Unknown callbacks
are never inferred from a similar signature.

Persist traces with `write_media_trace`. It writes `media-trace.json` and
`record-NNNN.bin`, recording the exact size and SHA-256 of every payload.
`read_media_trace` verifies all of them before returning bytes. Compare a wasm
trace with a Rust trace through:

```sh
cargo xt oracle compare-media .oracle/wasm-audio .oracle/rust-audio
cargo xt oracle compare-media .oracle/wasm-video .oracle/rust-video
```

The comparison is exact and ordered. It compares stream, callback symbol,
sequence, timestamp and payload bytes. Adapters on both sides must map their
callbacks to one canonical boundary name before comparison: the comparator
performs no normalization, reordering, timestamp tolerance or lossy decoding
on its own.

## Audio scenarios

Use one synthetic PCM input and fixed codec configuration for both sides. Keep
the stages separate so a failure identifies its layer:

1. codec: PCM to MLOW/Opus packet and packet back to PCM;
2. RTP: payload type, sequence, timestamp step and marker bit;
3. E2E protection: ciphertext and WARP authentication tag under fixed keys,
   participant ids, SSRC and rollover state;
4. receive: authenticated packet to decoded PCM and media statistics.

MLOW already has a complete codec oracle for J/S. The next audio trace should
therefore begin at `MediaPipeline::protect_audio` and
`MediaPipeline::unprotect_audio`, using the same derived MLOW packet as input.
That isolates RTP/SRTP defects from codec defects.

## Video scenarios

The Rust path accepts H.264 Annex-B access units. Start with a small synthetic
SPS/PPS/IDR sequence and fixed 90 kHz timestamps:

1. Annex-B access unit to single-NAL or FU-A RTP packets;
2. video RTP packets through E2E-SRTP/WARP;
3. authenticated receive, SSRC transition and FU-A reassembly;
4. reconstructed Annex-B access unit and orientation/frame metadata.

The wasm watch must be derived from the pinned module's callback ABI before a
fixture is accepted. Use `oracle inspect`, `oracle callers`, `oracle abi` and a
marker run to prove pointer/length/sequence/timestamp positions. Put that proof
beside the future media spec and pin the capture hash through `wasm.lock.json`.

## Outbound video orientation proof

`tools/oracle-core/tests/video_orientation.rs` executes the captured
`JgwtTQVeWPm.wasm`, rather than a Rust translation of it. The test checks its
SHA-256 before instrumentation:

```text
97259423aea19cc30c1771478e035105cb0d0e64ab4b0297741b62d01deac8db
```

The artifact is pinned in `tools/oracle-core/wasm.lock.json`. Fetch captures
with `cargo xt oracle fetch`, or point `WA_WASM_DIR` at an existing verified
capture directory. From the repository root:

```sh
WA_WASM_DIR=/path/to/captured-wasm cargo test --release -p oracle-core --test video_orientation -- --nocapture
cargo test -p wacore --features voip-mlow --lib voip::
```

An explicit missing or corrupt capture fails. An absent implicit cache may
print `skipping:`, which is not proof. A successful execution prints
`executed JgwtTQVeWPm.wasm: 10 orientation/keyframe cases` and reports upright
delta `0x00` and IDR `0x08`. The pre-fix comparison failed with WASM `[0, 8]`
against Rust `[1, 9]`.

The entry is table slot 9602, function 13128, anchored by
`onEncodedVideoDataFromJsForStream: Manager not initialized!`. Its callee
13065 constructs an encoded frame with internal presence bit `0x800`,
keyframe bit `0x08`, and rotation in the low two bits. The test provides a
synthetic manager and encoder port. It uses existing function 4942 as the
port callback, with a recording-only entry marker to obtain the frame
pointer. That callback changes only the frame type, not its metadata.

The test reads the constructed frame and separately feeds its low metadata
byte to the real extension builder, function 4943 at table slot 3681,
anchored by `media_frame_info_build_header_ext`. It does not execute the
intervening sender pipeline. All ten cases preserve the supplied 1280x720
dimensions, payload pointer and payload length. The only stub invoked during
each case is the recording marker.

The JS enum comes from `WAWebVoipMediaEnums` in bundle
`561b4bd5677d24a29707db4c05b63edc019b73744b66699fb6bbfd6b361d6c1a`.
Its `Unknown`, `Normal`, `Rotate90`, `Rotate180`, and `Rotate270` values are
0 through 4. The WASM maps them to rotation bits 0, 0, 3, 2, and 1. This is
WhatsApp-specific evidence, not an assumed CVO mapping.

The Rust session regression checks upright metadata on every protected IDR
and delta fragment. Android receive fixtures retain their captured `0x09`
and `0x01` values. The oracle does not decode camera pixels, parse a live
laptop SPS, run networking, or replace a browser-to-Android visual retest.
No proprietary WASM, captured JS, or personal data belongs in the test commit.

## CI shape

Future fixtures should have one producer command under `cargo xt oracle` that
runs the pinned wasm and writes a trace, and one Rust producer that drives the
pure pipeline with the same inputs. CI restores captures first, regenerates
both into a workspace cache, then runs `compare-media`. Published audio/video
fixtures should remain small; long calls belong in artifacts with a manifest,
not in Git.

The initial implementation is covered by `tools/oracle-core/tests/media_probe.rs`:
it proves callback payload capture, sequence/timestamp capture, resource-limit
failure, exact comparison, content-addressed persistence and stale-file cleanup.
The cross-layer gate that combines these traces with signaling, IQ and the
pure-Rust protocol tests is [`voip_conformance.md`](voip_conformance.md).

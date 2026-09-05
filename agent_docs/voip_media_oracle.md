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

The comparison is exact and ordered. It compares stream, sequence, timestamp
and payload bytes; callback symbols may differ because one side is wasm and the
other is Rust. Any normalization must happen while producing the two traces,
where it is explicit and reviewable. The comparator does not reorder packets,
apply timestamp tolerances or decode lossy content on its own.

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

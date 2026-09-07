# MLOW fixture derivation from shipped wasm

The primary fixtures come from WhatsApp Web's pinned VoIP wasm, using synthetic
PCM and the original encoder/decoder. J and S produce identical packets and PCM
for the 60/120 ms streams. DSP traces cover front-end, VAD, pitch, LSF, entropy
coding, excitation, noise and postfilters. Independent C auditors are retained
in compact archives; no captured call audio or real PII is used.

## Reproduce and verify

Run from the repository checkout:

```sh
cargo xt mlow regenerate --check
cargo xt mlow specs --check
cargo test --release -p oracle-core
cargo test -p wacore --features voip-mlow --lib voip::mlow
```

`regenerate --check` restores hash-pinned captures, executes all 11 derivations,
checks selectors and output hashes, assembles fixtures and compares canonical
CBOR bytes. Work stays in `.derive-mlow/wasm` by default; `--out` selects another
directory. `verify --from-derived` reuses outputs only after checking their
manifests, hashes and current specs. No recipe depends on old `/tmp` files.

For individual captures, use `cargo xt mlow verify --capture JgwtTQVeWPm` or
`--capture S_ivh1PriOA`. `--update-lock` requires both captures and is reserved
for intentional, reviewed changes. `--refresh-spec-hashes` permits serialization
changes only when modules, resolutions and outputs remain identical. Neither
mode accepts cached outputs; CI uses neither.

Bases and typed recipes in `tools/oracle-task/src/mlow-recipes.json` are tracked.
`cargo xt mlow specs` generates eight trace/S expansions in `.derive-mlow/specs`;
verification generates them automatically. Their spec-byte SHA-256s are checked against
`mlow.lock.json`, and CI uploads the expansions alongside run manifests.

## Ownership and invariants

- `tools/xtask` is the lightweight dispatcher. Capture-backed commands launch
  `tools/oracle-task` in release mode; hashes, descriptors and CI tasks do not
  depend on Wasmtime.
- `oracle-core` owns the WhatsApp host, capture identities and executable specs.
  Pinned `unwasm-core` provides generic static analysis; WhatSpec `wa-store`
  restores verified captures. `xtask-support` provides local I/O and CBOR/zstd.
- These unpublished tools stay outside `default-members` and the application
  runtime graph. Fixture readers are test dependencies.
- Derivation is single-threaded. Ambiguous selectors, invalid pointers, malformed
  hex, wrong status values and hash mismatches are errors, not fallback results.
- Every selector needs a string anchor, fingerprint or exact body SHA-256.
  Short trampolines use `expect_body_sha256`; get the hash for a reviewed index
  with `oracle abi CAPTURE --index N --body-sha256`. Migration still refuses
  short bodies it cannot independently map to the new capture.
- `manifest.json` has no timestamps. Identical specs and modules must produce
  identical resolutions and outputs. Migration checks the old capture hash and
  removes unresolved selectors rather than retaining stale indices.

## Pinned captures

| Capture | Bytes | SHA-256 |
| --- | ---: | --- |
| `JgwtTQVeWPm` | 10,650,934 | `97259423aea19cc30c1771478e035105cb0d0e64ab4b0297741b62d01deac8db` |
| `S_ivh1PriOA` | 10,856,103 | `e55e43babf85e2c0fc76ec65dcb8d47beba0f58b03b135b37a5aaaff7fe70e2f` |

A mismatched capture is rejected before instantiation. Capture updates require
re-deriving layouts and selectors, not merely replacing the lock.

## J codec entry points and layout

Function indices and table slots are different namespaces: `call_indirect`
uses slots; `oracle abi --index N` reads function N. Use `--slot N` to resolve
an indirect target. These identities were checked through bytecode, callers,
static strings and execution.

| Function index | Table slot | Role |
| ---: | ---: | --- |
| 6655 | 5057 | `opus_alloc_codec`; requires initialized factory/pool globals |
| 6658 | 5061 | `opus_codec_open(codec, attr)` |
| 6666 | 5065 | `opus_codec_encode` |
| 6670 | 5068 | `opus_codec_decode` |
| 6668 | 5066 | `opus_codec_encode_with_secondary` |
| 6659 | — | Application mode: `x < 3` asserts; otherwise reads `1045776 + x*4` |
| 6693 | 8598 | Full media endpoint/codec initialization; avoided by the minimal recipe |
| 9981 | 8581 | PJ bootstrap; does not register the Opus factory |
| 9848 | 8583 | PJ logging mutex initialization |
| 11920 | — | Lazy caller of 6693; has no table slot |
| 10566 | 7242 | Void trampoline to SMPL global initialization, function 10747 |

The allocator writes this layout before `opus_codec_open`:

```text
state: 824 bytes
+0=pool  +4=0  +8=5053 +12=5052 +16=5051 +20=5050 +24=5049 +28=5048
+32=5047 +36=5046 +40=5045 +44=5044 +48=5043 +52=5042 +56=5041
+60=5040 +64=5039 +68=5038 +72=5037 +76=5036 +80=5035 +84=5034 +88=5033
+92=5032 +96=5031 +100=5030 +816=1
codec: +8=state +12=1377164 (factory) +20=1312704
```

Slots 5053/5052/5051/5050 resolve to functions 10670/10671/10684/10681;
5039/5031/5030 resolve to 10694/10651/10562. Slot 5039 belongs at `state+64`,
not `state+0`; the latter holds the pool. The write at `state+92` is required.
Function 4313 wraps `pj_pool_calloc_no_trace(pool, 1, bytes)`.

| Field | Meaning or required value |
| --- | --- |
| `attr+0` | Sample rate |
| `attr+4` | `1`, from `opus_default_attr` (6653) |
| `attr+26` (u16) | Frame duration in milliseconds |
| `attr+36` (u8) | Nonzero selects MLOW |
| `attr+1116` | Application-mode index, at least 3; recipe uses 3 |
| `state+108` | Samples per frame: rate × frame duration / 1000 |
| `state+112` | Channel count, 1 |
| `state+120` | Encoder context returned by initialization |
| `state+128` | Decoder context created by `opus_open` |
| `frame+8` | Payload pointer; always allocate a separate buffer |
| `frame+16` | Payload length |

Other observed state fields are +100/+104, +116/+117 (MLOW flags 1/256),
+119/+124 (zero guards), +240=100 and +250=9; open clears 64 bytes at +192.
The remaining attribute fields are not assigned semantics without evidence.

## Initialization and encode/decode findings

The working recipe initializes TLS and the PJ logging mutex, performs PJ and
SMPL bootstrap, prepares codec/state/vtable structures and supplies a valid
single-block pool. The pool uses a 1 MB slab: the encoder needs 322,008 bytes,
plus smaller allocations and scratch space. A zeroed pool caused fuel exhaustion
in `pj_pool_allocate_find` (9884 ← 9883 ← 9887 ← 4313); more fuel did not help.

SMPL initialization sets the global at address 1719408. Without it,
`conv_convert` (10740) returns `SMPL_ENC_NO_GLOBAL_DATA` (-112). PJ bootstrap
alone leaves the codec factory unregistered, so it cannot replace the full
minimal recipe.

Encoding reads PCM through `frame+8`. Its return value `1` means success, not
packet length; the packet length is written at `outframe+16`. Writing PCM at a
low address such as 960 corrupts static data even if no bounds trap occurs.
The corrected first synthetic frame produces a deterministic 137-byte packet
beginning `50 e5 63 8c`.

Decode argument 2 is output PCM capacity, not encoded packet size. Function
6671 requires capacity ≥ samples × channels × 2; a 60 ms frame needs 1920 bytes.
Passing 137 instead produced status 70001. Correct capacity gives decode status
0 and a complete packet/PCM round trip.

The three-frame probe uses frames 0, 50 and 90 in one instance. It produces
137/139/138-byte packets and PCM energy ratios 1.147/1.260/1.158. The first
packet matches the single-frame run, confirming that later history does not
change earlier output. The 110-packet stream spans 14–147 bytes; the eight
120 ms packets span 218–277 bytes and all have TOC `0x58`.

## S migration and decode fix

Fingerprint migration carried 8,741 functions one-to-one from J to S. Open,
encode, decode, PJ and mutex selectors migrated automatically. The two-instruction
SMPL trampoline was below the fingerprint floor and required explicit recovery:
J function 10747 maps to S 11036, reached through S trampoline 10855, slot 7457.

Table slots must be read from S's allocator rather than copied from J. The
final spec rewrites every slot store, including repeated stores at +64/+92.
S uses factory address 1398252 and codec field +20=1325408.

The S decode trap was an invalid attribute allocation, not a synthesis defect.
S open (6950) reads the decoder-init skip flag at `attr+1200`; J open (6658)
reads +1176. The old 1184-byte allocation exposed the next heap block, skipped
initialization and left `state+128=0`. Allocating and clearing 1216 bytes lets
`opus_open` create the decoder context normally.

Earlier experiments forged a decoder context and moved the failure into
synthesis 10932/11007; larger buffers and writes to address 96 did not solve it.
Those hypotheses are retired. The final S recipe passes all 220 hashes for the
60 ms stream and all 16 for the 120 ms stream. The single-frame packet and PCM
hashes begin `e4b19307` and `7b3552b8`, matching J.

## DSP traces and recovered tuning

Capture locations use zero-based operator ordinals in the original function,
before instrumentation. `capture_memory` reads a span through an i32 local
without writing guest memory; it requires exact hit counts and enforces a
64 MiB budget. `capture_value` preserves scalar bits. Tests check observation
neutrality, missing hits, f32 transport and invalid ranges.

| Area | Capture location and evidence | Rust behavior |
| --- | --- | --- |
| Front-end | J10736: window call 1375, LPC call 1394, BWE ends before 1422; kernels 10749/10797 | Compare 330 windows, spectra, autocorrelations and LPC before/after BWE |
| LPC precision | J10797 promotes regularization to f64 before adding 1 | Match that order; also solve against exact oracle autocorrelation |
| BWE | Caller constant `0x3f7f9db2` = 0.9984999895 | Use 0.9985; retain C's 0.9999 only in its auditor |
| VUV/SPACT | J10736 operators 4280/4584; bias bits `0xbe051eb8` | Bias -0.13 instead of C's -0.1038; 330 exact speech-activity results |
| LSF | J10804 entry and implicit return at operator 1099 | 330 quantizations, including 219 conditional cases; chain previous qlsf |
| Pitch | J10736 operators 1691/4266 | Match 330 lag/contour records using the captured 4-survivor, low-complexity configuration |
| Pitch weights | Coarse/fine factors 0.105/0.0046875 divide by block size 64 | Previous weight 0.7, delta weight 0.3; C retains 0.7981/0.1439 |
| Wire parameters | J10758 outputs 330 parameter/context records | Exact symbols, gains, lags, contours, Q14 energy and 11 range-decoder words |
| Noise/excitation | J10726 operators 2631/2632, calling 10777 | 1320 decoder-stream cases, including inactive frames; exact RNG state |
| HP postfilter | J10726 operators 5550/5820 | 330 frames; 1332-byte state; output error below one i16 LSB |
| Harmonic postfilter | J10726 operators 7728/8368 | 110 packets; 9260-byte state; feedback 0.4, strength 0.713 |

The runtime encoder retains its 24-survivor quality budget; only the fixture
test adopts the captured 4-survivor configuration. A provisional pitch delta
weight of 0.15 was rejected after comparing survivor scores, E2 and H blocks.
For LPC, near-silent ill-conditioned cases must meet the coefficient tolerance
or imply less than one i16 LSB of prediction error; they are not skipped.

Sparse pulse positions/magnitudes are stored at +760/+1080 with count at +1400.
The legacy dense field remains zero. Fixture assembly reconstructs the sparse
values and obtains CAV from the TOC, matching the existing decoder.

NoiseGenerator has 11 floats and three i32 fields, with seed at +52. Initial
C-input replays were replaced by live decoder-stream snapshots. Likewise,
front-end traces replaced 40 C-input replays. Expected results now come entirely
from wasm execution rather than host-side DSP calculations.

After the tuning changes, the 23,040-sample encoder golden measured RMS 0.153243,
peak 0.455261 and no clipping; PCM checksum `f535e428a5a1e641`, frame checksum
`8af5211b8d4e38da`. Historical C profiles remain explicit in their own tests.

## Coverage and representation

| Derivation | Evidence |
| --- | --- |
| `mlow_110frames{,_s}` | 110 packets and 110 PCM outputs per capture |
| `mlow_120ms{,_s}` | Eight packets and eight PCM outputs per capture |
| `mlow_dtx_off` | SET/GET DTX=0, with the same 220 packet/PCM hashes |
| `mlow_fe_trace` | 330 windows, spectra, autocorrelations and LPC snapshots |
| `mlow_signal_trace` | 330 VUV/SPACT records and 110 routing decisions |
| `mlow_kernel_trace` | 330 pitch and 330 LSF records |
| `mlow_params_trace` | 330 wire parameter and range-coder states |
| `mlow_gennoise_trace` | 1320 excitation/noise/state/energy records |
| `mlow_postfilter_trace` | 330 HP and 110 harmonic-filter records |

The 60 ms stream contains 11×`0x10`, 3×`0x12` and 96×`0x50`. Inactive packets
are 17–19 and 62–69; `0x12` is active hangover routing. All 110 decoded outputs
must contain exactly 960 samples. The DTX control recipe calls J10684, slot
5051, with SET 4016=0 and GET 4017, requiring status/value 0. This disproved an
earlier claim that the pair used DTX enabled. Rust decodes every packet before
selecting inactive outputs, preserving stream history.

TOC parsing remains covered by the compact C auditor and the dedicated
writer/parser test, including escape durations. J10561 (duration) and J10650
(frame count) confirm the layout; generating another table of bit shifts adds
no independent evidence. SPACT is now covered exactly by the wasm signal trace.

Large fixtures use canonical CBOR+zstd; postfilter raw streams use zstd.
Comparison uses decompressed canonical bytes, so compressor-version changes
cannot hide data drift. `cargo xt mlow pack-legacy --check` reconstructs the
independent C auditor selection from immutable historical blobs. No C output
was changed to agree with Rust. Full corpus details and regeneration commands
are in [PROVENANCE.md](../wacore/src/voip/mlow/testdata/PROVENANCE.md).

## Investigation tools and validation record

`oracle derive --spec SPEC -o OUT` supports allocation, memory I/O, register
arithmetic, direct/export/table calls, assertions, snapshots and markers.
`call_function` exports resolved targets without changing function bodies or
table indices. Calls use explicit f32 arguments and assert status values.

For static investigation, use `oracle abi --index N`, `oracle abi --slot N` and
`unwasm decompile CAPTURE --only INDICES --bare -o decoded.rs`. Annotated strings
can be coincidental; confirm a kernel through dataflow, callers and execution.
Use exact counters to establish whether imports ran; bounded argument traces
cannot prove absence after saturation.

On 2026-09-04, all 11 derivations reproduced in two fresh output directories.
Validation passed 2261 wacore tests, including 131 MLOW tests, with two existing
ignored tests, plus workspace clippy. The initial artifact-upload failure was
fixed by explicitly including hidden evidence directories. Successful runs:

- [Original J/S tool matrix](https://github.com/oxidezap/unwasm/actions/runs/33938645892).
- [Consumer regeneration and MLOW tests](https://github.com/oxidezap/whatsapp-rust/actions/runs/33938854330).
- [Consumer after the ownership split](https://github.com/oxidezap/whatsapp-rust/actions/runs/33983916097).

The Rust task migration removed 14 Python/Bash files while preserving output
trees, packet/PCM bytes and canonical CBOR. Its serialization-only lock refresh
changed spec byte hashes, not modules, resolutions or outputs. The later switch
to generated expansions preserved those Rust-serialized hashes unchanged.

The 2026-09-05 tooling revision passed 150 oracle/CLI tests, seven lightweight
utility tests, four worker tests, all 11 J/S derivations, fixture checks,
all-target clippy and rustdoc with warnings denied. The 26 slow signaling
research scenarios remained ignored. Those broader coverage limits are recorded
in [voip_conformance.md](voip_conformance.md).

Selector hardening added exact body hashes from the existing locked resolutions.
The guarded specs were rerun with `--refresh-spec-hashes`: all 11 runs retained
identical module pins, resolutions and output trees. Only spec-byte hashes and
the fixture manifest's derivation-lock reference changed.

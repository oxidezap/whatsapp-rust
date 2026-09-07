# VoIP conformance gate

No finite test suite proves every behavior of an evolving remote client. This
gate makes the guarantee reviewable instead: each VoIP layer has an explicit
reference, deterministic vectors and a command that fails on drift. A missing
capture, malformed reference or unsupported host behavior is an error, not a
skip or a guessed success.

Run the normal gate with:

```sh
cargo xt oracle conformance
```

The scheduled CI adds the slow, serialized signaling scenarios:

```sh
cargo xt oracle conformance --slow
```

## Coverage contract

| Layer | Reference | Gate | Status |
| --- | --- | --- | --- |
| JS signaling and IQ schemas | pinned WhatSpec IR from the captured WhatsApp Web bundle | `whatspec-codegen --check`, `wacore::iq` tests | enforced |
| wasm signaling behavior | pinned J VoIP engine, with capture hash and callback ABI checks | `oracle-core` differential/signaling tests | enforced |
| MLOW audio codec | pinned J and S engines | 11 rederivations, fixture hashes, 131 Rust MLOW tests | enforced |
| audio/video callback bytes | pinned wasm callback plus declarative `MediaWatch` | content-addressed media traces and `compare-media` | infrastructure ready; callback ABIs still need derivation |
| RTP/RTCP and H.264 packetization | pure Rust protocol tests grounded in captured constants | all `wacore::voip` tests | enforced; wasm differential trace pending |
| E2E-SRTP, WARP, SFrame and HBH-SRTP | independent KATs plus pure Rust round trips | all `wacore::voip` tests | enforced |
| full slow call scenarios | pinned J engine with real worker threads | ignored signaling suite, serialized by both engine locks | weekly |

The wasm is not the source for IQ construction performed in JavaScript, and
the JS is not the source for DSP executed inside wasm. The gate uses the owner
of each behavior and keeps both inputs pinned. `whatspec` owns acquisition and
IR, `unwasm-core` owns static wasm analysis, `oracle-core` owns WhatsApp host
execution, and `wacore` owns the implementation being checked.

## Adding a behavior

1. Identify whether the behavior lives in JS, wasm or both.
2. Pin the exact input through WhatSpec/`wasm.lock.json`.
3. Derive the selector or stanza shape without trusting a raw index.
4. Record a minimal deterministic vector and its provenance.
5. Drive the same input through `wacore` and compare at the narrowest boundary.
6. Add the test under the matching gate above; update this table if a new
   layer or reference is introduced.

Media-specific trace layout and audio/video scenarios are in
[`voip_media_oracle.md`](voip_media_oracle.md).

## Verified coverage and remaining evidence

[CI run 33999004005](https://github.com/oxidezap/whatsapp-rust/actions/runs/33999004005)
verified the normal gate on `041bfdc4d`: all 11 J/S derivations, 680 VoIP tests
and 334 IQ tests passed. The signaling test binary reported 26 ignored tests;
this run does not validate those slow call scenarios. Their assertions still
include capture-specific investigation cases documented in the oracle's
`AGENTS.md`; scheduling them is not proof they pass.

The IQ tests and generated-artifact check establish schema consistency and
Rust regression coverage. They do not execute every IQ against WhatsApp Web.
Full audio/video callback traces and end-to-end signaling/IQ differential
cases remain separate evidence to derive. Until those cases exist and pass,
this command reports the implemented gates, not complete VoIP equivalence.

Expanded derivation specs are generated from committed bases/recipes into
`.derive-mlow/specs/`, verified against `mlow.lock.json`, and uploaded alongside
run manifests. The lightweight `cargo xt` dispatcher launches the release
`whatsapp-oracle-task` worker; neither is linked into the application runtime.

Capture restoration, code generation checks, and oracle tests precede MLOW
derivation. A failure in those stages can leave no derivation evidence to
upload. CI skips an empty upload only after an earlier failure, keeping that
failure visible. It uploads partial evidence when present and still treats
missing evidence after a successful gate as an error.

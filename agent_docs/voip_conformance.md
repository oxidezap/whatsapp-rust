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

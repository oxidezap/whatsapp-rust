# WhatsApp wasm oracle

Unpublished host tooling for captured WhatsApp Web modules. Read `README.md`
for commands and supported APIs, and `../../agent_docs/voip_conformance.md`
for the coverage boundary. Historical experiments live in
`../../agent_docs/voip_oracle_history.md` and `voip_oracle_status.md`.

## Boundaries and verification

- WhatsApp imports, captures and specs belong in `tools/oracle-*`. Generic
  static analysis comes from pinned `unwasm-core`; capture transport comes
  from whatspec `wa-store`. None may become a published runtime dependency.
- Run `cargo fmt --all`, `cargo clippy -p oracle-core --all-targets --release
  -- -D warnings`, and `cargo test --release -p oracle-core`. Examples are
  included in clippy. Check dependencies with `cargo machete`.
- Execute captures in release mode: debug Cranelift compilation is too slow
  for meaningful engine deadlines. Tooling requires Rust 1.95 and stays outside
  `default-members` and the published MSRV contract.
- Fetch with `cargo xt oracle fetch`. `WA_WASM_DIR` overrides `.cache/wa-wasm`.
  Missing implicit captures may skip locally; corrupt captures and explicit
  overrides must fail. A run printing `skipping:` is not conformance evidence.
- Selectors require a string anchor, fingerprint or exact encoded-body hash.
  An index alone is not evidence, including for short initialization trampolines.
- `wasm.lock.json` pins exact hashes. A capture bump requires re-deriving
  indices, addresses, selectors and expectations together. Never commit wasms.

## Host contracts

- Unsupported behavior is an explicit error. A zero-returning stub is a
  hypothesis. Inspect exact `hot_calls()`/`stubs_called()` counters; the bounded
  argument trace cannot prove a late import was never called.
- Resolve exports through `exports.rs`, reporting missing symbols and aliases.
  Memory/table export names come from the module rather than conventional names.
- Derive imports from declared signatures, but verify argument order at guest
  call sites. Equal signatures do not imply equal semantics; random-byte
  callbacks and WASI use different pointer/length ordering.
- Validate memory ranges and resource budgets before allocation or mutation.
  Invalid requests must not silently produce partial records or consume PRNG
  state. Strings require valid encoding and terminators.
- Clocks and filesystem state are process-wide. PRNG state and C++ exceptions
  are per thread. Release the WASI state lock before calling guest code.
- Keep inspection streaming and static (`wasmparser`): do not instantiate
  Wasmtime or decode every instruction to answer metadata queries.
- Identify strings from data segments. Printable opcode bytes are not strings.
- Use fictitious identities in tests; no production PII.

## Concurrency

- Guest threads can execute concurrently. The scheduler is cooperative and has
  a timeout escape; it is not a mutual exclusion or memory-safety guarantee.
- Read/write shared guest memory through the atomic accessors in `state.rs`.
  These prevent host data races but do not provide coherent multi-byte snapshots.
  New unsafe sites require a local safety argument independent of scheduling.
- Each worker must install its guest-allocated stack, initialize pthread/TLS,
  and fail if required exports or stack bounds are missing. Runtime drop stops
  and joins workers.
- Hold scheduler turns through RAII guards on every guest entry path. The main
  emscripten thread must use `can_block = 0` so waiting can yield to host code.
- Main-runtime registration is currently disabled by default because synchronous
  proxy draining can deadlock startup. This remains a full-call coverage gap;
  see the status document. Do not turn a failed drain into evidence of no send.
- Tests starting an engine acquire `common::threaded_guard()` (local and
  cross-process locks). Wait for observable progress/quiescence, not arbitrary
  sleeps. Thread ordering itself is not deterministic.

## Instrumentation and evidence

- Select automatic marker sinks from `RECORDING_ONLY_SINKS`, never by signature
  alone. Other sinks require an explicit choice. Mark call sites when the
  question is about a particular call; body markers answer a different question.
- Reject invalid locals, replacement ranges and unsupported expressions before
  publishing an artifact. Keep capture hashes and output hashes in the evidence.
- Media probes require explicit configuration and bounded payloads. Compare
  meaningful metadata and bytes without silent normalization. Concurrent traces
  may differ in interleaving; an ordinal records one run's observed order.
- Sweep domain boundaries in differential tests. A few successful examples do
  not establish a numerical model or a complete signaling protocol.
- Ignored signaling scenarios are research, not a green full-call guarantee.
  Extend the documented coverage only after the corresponding scenario passes
  against the pinned capture and independently validates the Rust side.

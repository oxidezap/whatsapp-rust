# Observability & Per-Session Metrics

How to measure what one client session costs (memory, I/O, CPU) — including
several clients inside the same process — and the design rules any extension
must follow.

## Design rules

- **Runtime/platform agnostic.** Everything in `wacore::stats` builds on every
  target (Tokio, wasm32, ESP32): counters are `portable_atomic`, CPU metering
  reads the pluggable `wacore::time` monotonic clock, task instrumentation
  wraps the `Runtime` trait. Never add a Tokio/allocator/`tracing` dependency
  to this layer — platform-specific mechanisms plug in from the application
  through the hooks.
- **Zero overhead when unused, no feature gates.** Always-on counters are one
  relaxed `fetch_add` on paths that already do AEAD + a transport write.
  Report code runs only when called; unused public report methods are removed
  by fat LTO (the binary-size CI proves it, see `binary_size_ci.md`). This is
  why there is no `debug-diagnostics`-style feature: dead code elimination
  replaces the cfg-gates.
- **No PII.** Snapshots and reports carry numbers only, never JIDs/phone
  numbers, matching the `wacore::telemetry` label rules.

## The three surfaces

### 1. `Client::stats()` — wire I/O counters (always on)

`wacore::stats::SessionStats`, owned by each `Client`. Recorded at exactly two
chokepoints:

- **Sent**: the noise sender task (`NoiseSocket::with_stats`) after the
  transport write — post-noise wire bytes (frame header + AEAD tag included).
- **Received**: the read loop (`node_io.rs`) per `DataReceived` batch.

It also owns the `last_data_sent_ms`/`last_data_received_ms` activity
timestamps the keepalive dead-socket watchdog reads (they were loose fields on
`Client` before). Message-level counters piggyback on the existing
`telemetry::send`/`recv` chokepoints; reconnect attempts are counted in the
run loop. VoIP relay sockets pass `None` and are not counted — this is the
main WA session socket only.

### 2. `Client::memory_report()` — retained memory (on demand)

Walks every internal collection and returns entry counts plus estimated
retained bytes (`MemoryReport`, per-collection `CollectionStats`). Byte
figures come from the `wacore::stats::HeapSize` trait:

- Signal records use their protobuf encoded size (`SessionRecord::
  estimated_size`, buffa `compute_size` — no encode buffer allocated).
- Collections sum key/payload capacities (`GroupInfo`, `DeviceListRecord`,
  `LidPnEntry`, `ResolvedGroupDevices`, ...).
- Store-backed caches (Redis etc.) report `bytes: 0` — their entries are not
  process memory.

Semantics: honest estimates for attribution and leak detection, not
byte-exact accounting. The e2e `memory_soak.rs` logs the byte totals next to
RSS; its growth-bound assertions are on entry counts.
When a new cache is added to `Client`, add it to `memory_report()` (the
`MemoryReport::collections()` list keeps the total and `Display` in sync) and
— if it can dominate memory — implement `HeapSize` for its value type next to
that type's definition.

### 3. `BotBuilder::with_task_instrument` — CPU / custom attribution (opt-in)

`wacore::stats::TaskInstrument` is an object-safe enter/exit hook called
around every poll of the client's internal tasks and around its blocking
work. Wiring: `build()` wraps the runtime in `InstrumentedRuntime`, so all
spawns through the `Runtime` trait are covered without touching call sites.
The `Option` is resolved once at `build()` — `None` (default) leaves the
runtime untouched, so there is no per-spawn or per-poll cost when unset.

- `CpuMeter` (built-in): busy time (direct CPU proxy) + poll count via
  `wacore::time::Instant`. Works on wasm/embedded once a monotonic provider
  is registered.
- Custom hooks: allocator attribution (see `examples/alloc_tracking.rs` for a
  dependency-free pattern; `tracking-allocator` slots in the same way),
  ESP-IDF `heap_caps` sampling, etc. The library never learns what the hook
  does.

Scope caveats: the hook covers tasks spawned *by the client* through the
`Runtime` trait. Work executed on the caller's own task (e.g. awaiting
`send_message`) belongs to the caller — instrument that side yourself if you
need it. The `voip` feature's media tasks (call driver, relay I/O) currently
spawn directly on Tokio and are not instrumented.

## Relation to the `metrics`/`tracing` features

`wacore::telemetry` (cargo feature `metrics`) emits process-global counters
through the `metrics` facade — no per-client dimension, by design (label
cardinality). The `stats` layer is the per-client dimension: snapshots you
poll and export however you like. `examples/multi_session_metrics.rs` shows
two clients in one process reporting independently.

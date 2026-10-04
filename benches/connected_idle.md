# Connected retention benchmark

## What is connected, and what is synthetic?

`bench_support::connected_idle::Session` performs production Noise XX,
`Client::connect`, encrypted `<success>` login, active IQ and offline completion.
The production read loop, dispatcher, chat-lane workers, durability buffer,
Signal flushes and logged-in keepalive run throughout idle. The local server
answers keepalive pings. No connected flag or lane timeout is changed by the
fixture; it never clears/closes lanes to manufacture reclamation.

The transport is adapted from the existing `test_support::call` synthetic server:
no TCP, credentials, production captures or WhatsApp network access. Certificate
verification is bypassed **only on this client**, using the existing test policy.
Other initialization IQs get explicit 503 replies, not realistic service data.
Signal sessions/device lists/sender keys are established in fixture setup.
Workload sender-key/signature RNG uses seed `0x434f4e4e45435431`; stanza IDs,
chat IDs and repeated `x` payloads are fixed. Client identity/session and Noise
handshake keys are freshly randomized by existing library helpers; the seed
is not a claim that the entire handshake byte trace is identical.
This is a representative *bounded receive/history workload*, not a fully synced
account with all service responses or a production workload distribution.

Each activity session receives **256 encrypted group text messages, 4,096 ASCII
bytes each, across 32 chats** (8 messages/chat). They enter through Noise frames
and the real reader. The initial offline marker is withheld until after activity,
so the registered successful consumer hook exercises the library's real
pending-inbound persistence/commit arena and offline-to-live drain. The external
consumer hook just counts; it does not retain messages or implement another DB.
The event consumer counts individual delivered messages, not batches, and retains
no event payloads. A failure to decrypt/commit/dispatch fails the run.

One inline history task contains **256 historical messages of 4,096 bytes across
the same 32 chats**, with fictitious message secrets. It runs the real history
parse/secret-seeding/event path using zlib-compressed protobuf input. As in the
existing inline history tests it enters at `process_history_sync_task_tracked`, **not**
as an encrypted own-device history notification; no media download is measured.
Input ciphertext and the synthetic peer are dropped before idle. SQLite means a
fresh file-backed `SqliteStore`, with its normal WAL/pool settings, never SQLite
`:memory:`. Its files are private under `target/connected-idle/` and removed
following shutdown.

## Fast CodSpeed/native Divan surfaces

```sh
CARGO_BUILD_JOBS=1 cargo bench -p whatsapp-rust --no-default-features \
  --features bench-harness,sqlite-storage-bundled --bench connected_idle -- --test
CARGO_BUILD_JOBS=1 cargo codspeed build -m simulation -m memory \
  -p whatsapp-rust --features bench-harness
cargo codspeed run -p whatsapp-rust
```

The existing client shard discovers the new target; no workflow/service is added.
Boolean arguments mean `false = InMemoryBackend`, `true = file-backed SQLite`.

* `activity_peak`: setup builds/connects the client, establishes keys and prepares
  ciphertext/history **outside** the measured region. The timed region receives
  the prepared workload, finishes offline drain and flushes library work.
* `cache_maintenance_after_activity`: connection **and activity** are setup;
  setup also joins finite construction/post-login tasks and their finite children.
  Registration happens before spawn under `bench-harness`, with RAII completion
  on return or cancellation. Empty IQ waiters alone cannot establish this barrier:
  initialization may be unpolled or awaiting SQLite before its next query.
  Only the explicit production cache sweep and settling are timed. Maintenance
  rejects pending/new initialization and checks the synthetic wire's initialization
  IQ count; it never waits for startup inside this timed region. Periodic keepalive
  and other session-long workers are deliberately not part of the startup barrier.
* Both use fresh fixtures; graceful shutdown/database cleanup is outside timing.
  Shutdown/database cleanup errors fail the fixture. `AllocProfiler` supplies
  native Divan allocation columns and CodSpeed uses its
  supported simulation/memory instruments. These rows are CPU/allocation-peak
  measurements, **not retained heap, elapsed idle or RSS**. They never sleep for
  a minute or advance virtual time inside a simulated CPU region.

## Deterministic lifecycle tests

```sh
CARGO_BUILD_JOBS=1 cargo test -p whatsapp-rust --no-default-features \
  --features bench-harness,sqlite-storage-bundled --test connected_idle_lifecycle
CARGO_BUILD_JOBS=1 cargo test -p whatsapp-rust --no-default-features \
  --features bench-harness,sqlite-storage-bundled --example connected_idle
```

In-memory tests use paused Tokio time. SQLite initialization/activity uses real
time, then pauses time **only for the idle lifecycle** (blocking SQLite I/O is
not virtual time). Tests assert real login/delivery, worker exit after the
61-second timeout boundary and connected state after maintenance. The control
explicitly probes the real keepalive IQ path; the native run also requires a
pong from the production periodic keepalive during real idle. No memory/RSS
claims come from virtual-time tests. Cache TTL clocks use their own time sources; these tests do not assert TTL eviction from a
Tokio clock advance, nor pin the number of closed lanes main retains.

## Native retained heap and Linux RssAnon

Build first, then invoke the executable (not Cargo/nextest/Divan) from the repo
root. The small parent launches **one fresh child process per backend, mode and
repeat**, sequentially; no build/test harness or sibling session shares its
address space. Default: three repeats, roughly 12 minutes total. One repeat is
an explicitly labelled ~4-minute smoke check, not a noise estimate.

```sh
CARGO_BUILD_JOBS=1 cargo build -p whatsapp-rust --no-default-features \
  --features bench-harness,sqlite-storage-bundled --release --example connected_idle
MALLOC_ARENA_MAX=1 MALLOC_MMAP_THRESHOLD_=131072 \
MALLOC_TRIM_THRESHOLD_=131072 MALLOC_TOP_PAD_=131072 \
  target/release/examples/connected_idle --repeats 3 \
  > target/connected-idle-baseline.jsonl 2> target/connected-idle-baseline-summary.txt
```

A custom `CARGO_TARGET_DIR` changes both the executable and fixture database
paths; preserve that environment variable when invoking the executable.
Match build profile, allocator environment and activity volume across branches. The fixture uses
Rust's System allocator with allocation-free live/peak counters. No forced
`malloc_trim`, GC, cache clear or SQLite checkpoint is applied.

Every child emits JSON checkpoints:

1. `runtime_baseline`: initialized current-thread runtime, before backend/client.
2. `connected_before_activity`: genuine login/active IQ, before offline marker.
3. `after_activity`: history plus message drain/commit/flush, or empty offline
   completion for the **connected no-activity control**.
4. `after_61s`: **at least 61 real elapsed seconds after checkpoint 3**, beyond
   the production 60s worker idle timeout, still connected, workers stopped.
5. `after_maintenance`: one explicitly awaited production cache-maintenance
   sweep immediately after checkpoint 4, plus settlement. This is not a claim
   that every default cache TTL has elapsed; most TTLs exceed 61s. A sweep may
   legitimately release nothing on main.
6. `after_shutdown`: graceful reader/client shutdown and backend guard dropped,
   with the runtime still alive (same reference state as `runtime_baseline`).
7. `after_runtime_drop`: runtime and all its remaining tasks/threads dropped;
   report/result buffers and process-global state may remain. Negative
   baseline-relative deltas are valid.

The parent prints raw child JSONL plus min/median/max *baseline-relative* Rust
live-byte and RssAnon-KiB deltas for checkpoints 3–7. Compare the activity/control
medians for the same backend; also subtract checkpoint 3 from 4/5 within each
child to inspect releases. Keep the range/raw repeats: RSS is page-quantized,
`/proc` accounting and task scheduling introduce noise. There is no hard RSS
threshold masquerading as a deterministic CI guard.

* `rust_live_bytes`: requested Rust allocations not yet freed. Not an estimate
  from `memory_report`, and not all process heap: SQLite's C malloc, allocator
  metadata/size-class slack and thread stacks are excluded. The local transport,
  runtime, fixture counters and backend are included; controls expose fixed cost.
* `rust_peak_bytes`: cumulative process Rust-live high water since the runtime
  baseline. Unlike the CodSpeed activity peak, this **includes connection and
  synthetic preparation**. Small checkpoint-label and `/proc` read transients
  can also contribute to later cumulative peaks.
* `rss_anon_kib`: Linux `/proc/self/status` **RssAnon**, not VmRSS (which includes
  file-backed executable pages). It includes C heap, stacks and anonymous pages
  the allocator still owns. Other platforms report `null` and an unsupported
  warning, never invented zeroes. Linux read/parse errors fail the run.

**Freed Rust heap need not reduce RssAnon immediately.** System/glibc may keep
pages/arenas for reuse. A lower peak alone does not prove lower retention, and a
flat RSS alone does not prove that Rust objects stayed live. Report all three
readings and the control/noise range; never upload RssAnon as a CodSpeed metric.

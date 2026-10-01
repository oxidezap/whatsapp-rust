# Connected post-activity retention baseline

This fixture is independent of the memory optimization branches: it runs on
main without adding arena/cache/lane/Signal/SQLite lifecycle changes. It supplements, rather than
reinterprets, the reported CodSpeed run (408 simulated CPU / 402 memory results).
Those results were operation costs/allocation peaks, not connected idle RSS.

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
* `cache_maintenance_after_activity`: connection **and activity** are setup; only
  the explicit production cache sweep and settling are timed.
* Both use fresh fixtures; graceful shutdown/database cleanup is outside timing.
  `AllocProfiler` supplies native Divan allocation columns and CodSpeed uses its
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

## Observed native baseline (2026-10-01)

Library source: main `19eb64cac58bc2e36cc9e2bc3c92fe9af3a78729`; fixture code:
`e92fb44f1bd08d7324688a7805bd1beb03e6b3a1`. Linux AArch64, glibc 2.39,
Rust `1.98.0-nightly (01dfd7924 2026-06-15)`, **dev/unoptimized + debuginfo**,
minimal features above, frozen malloc environment above. No compiler/build
script was running during sampling. These are debug-profile baseline observations,
**not** release performance, optimization gains, or CodSpeed readings.

Three fresh processes per backend/mode, all 12 successful. Each elapsed idle
interval was 61,001–61,006 ms. Activity children delivered/committed all 256
messages, dispatched history, remained connected with periodic pong(s), and
went from 32 running workers to zero. All SQLite fixture files were removed.

Median requested Rust bytes (live deltas are relative to `runtime_baseline`):

| Backend / mode | Cumulative peak (absolute B) | After activity ΔB | After 61s ΔB | After sweep ΔB |
| --- | ---: | ---: | ---: | ---: |
| Memory / control | 1,197,133 | 201,265 | 168,809 | 170,482 |
| Memory / activity | 4,790,526 | 1,954,893 | 1,580,591 | 1,582,120 |
| SQLite / control | 1,082,647 | 97,568 | 69,427 | 71,100 |
| SQLite / activity | 5,241,544 | 1,730,302 | 1,355,698 | 1,357,227 |

RssAnon deltas from runtime baseline (min / median / max KiB):

| Backend / mode | After activity | After 61s and sweep |
| --- | ---: | ---: |
| Memory / control | 232 / 232 / 232 | 1,136 / 1,136 / 1,136 |
| Memory / activity | 4,852 / 4,856 / 4,864 | 4,852 / 4,856 / 4,864 |
| SQLite / control | 968 / 972 / 1,008 | 1,820 / 1,844 / 1,912 |
| SQLite / activity | 6,404 / 6,412 / 6,456 | 6,404 / 6,412 / 6,456 |

Activity Rust live bytes fell 374,302 B (memory) and 374,604–374,755 B
(SQLite) during idle, with **no RssAnon change in any activity child**.
Fresh startup/periodic work also affects controls: SQLite's after-activity
live delta ranged 95,545–205,419 B, narrowing to 68,179–70,259 B after idle.
Do not attribute every change solely to workers or compare raw cohort peaks
as a storage-backend optimization claim.

After graceful shutdown, activity live deltas were 74,119 B (memory median)
and 75,253 B (SQLite median); after runtime drop, 25,027 B and 26,161 B.
RssAnon activity medians after teardown remained +2,824 / +5,384 KiB.
This is end-of-life accounting, not an assertion that process-global buffers
or allocator pages must disappear when one client is dropped.

To reproduce this **dev** baseline, omit `--release` from the build command and
invoke `target/debug/examples/connected_idle --repeats 3` with the same malloc
and `CARGO_TARGET_DIR` environment. Keep raw JSONL and min/median/max output;
release builds should establish their own baseline rather than reuse this table.

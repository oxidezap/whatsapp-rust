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

- **Sent**: the noise sender task (`NoiseSocket::with_observers`) after the
  transport write — post-noise wire bytes (frame header + AEAD tag included).
- **Received**: the read loop (`node_io.rs`) per `DataReceived` batch.

That sent chokepoint is the *only* place every post-handshake frame crosses (the
XX/IK exchange writes to the transport directly, before this socket exists). Two
functions reach it — `send_raw_bytes` and `send_raw_bytes_burst` — and everything
else funnels through one of those: `send_node` and every IQ through the first,
the ack and delivery-receipt workers through the second. `send_raw_bytes`
deliberately bypasses node logging and sent-node waiters, so anything that has to
see everything the client sends on the session socket belongs at the chokepoint
and nowhere else.

`Event::SentFrame` is the other thing wired into it
(`Client::acquire_sent_frame_forwarding()`, lease-gated like `RawNode`): it hands
over the marshaled plaintext of each frame the transport accepted. Both halves
travel to the socket as `SendObservers`, so the next observer plugs in there
instead of widening `do_handshake` again.

It also owns the activity timestamps the keepalive dead-socket watchdog reads:
`last_data_received_ms` (one clock read per received transport event, plus one
more when that event carries several frames, so a slow drain is not read as
silence) and `first_send_since_recv_ms`, which every frame loads but only the
send that arms or re-arms the anchor spends a clock read on. There
is deliberately no "last send" timestamp: nothing in the core reads one, and it
cost a clock read on every frame written, which is the client's hottest path
and a call out of the module on wasm32/embedded. `frames_sent` answers "is it
still sending?" for free. Message-level counters piggyback on the existing
`telemetry::send`/`recv` chokepoints; reconnect attempts are counted in the
run loop. VoIP relay sockets pass `SendObservers::default()` and are not counted
— this is the main WA session socket only.

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
- In-flight history sync reports queued/running task count, retained compressed
  payload storage, and lifetime peaks. Inline payloads count while queued;
  external payloads contribute their `Vec` capacity once materialized.

Semantics: honest estimates for attribution and leak detection, not
byte-exact accounting. The e2e `memory_soak.rs` logs the byte totals next to
RSS; its growth-bound assertions are on entry counts.
When a new cache is added to `Client`, add it to `memory_report()` (the common
`MemoryReport::collections()` list or its feature-gated report section) and —
if it can dominate memory — implement `HeapSize` for its value type next to that
type's definition.

With the opt-in `plugins` feature, the report also includes installed plugins,
active install/connection tasks, retained connection generations, core-event
subscriptions, custom-event endpoints, and unique queued payload bytes. Fanout
shares one envelope, so queued payload memory is counted once even when several
endpoints retain it.

### Plugin host snapshots (opt-in)

`Client::plugin_stats()` is computed only when called and returns lifecycle,
health, task, subscription, and custom-event counters keyed by public manifest
ID. `PluginEventRouter::stats()` provides endpoint capacity, current unique
queue retention, and cumulative delivery/backpressure totals; publishers can
read their own totals through `PluginEvents::stats()`.

Health is sticky for the lifetime of the host: lifecycle errors/panics,
timeouts, spawned-task panics, task-drain timeouts, isolated core-event panics,
resource teardown panics, publication failures, and queue drops mark only the
responsible plugin as degraded. Concurrent snapshots are intentionally
approximate, and carry no message content, JIDs, or phone numbers.

### 3. `BotBuilder::with_task_instrument` — CPU / custom attribution (opt-in)

`wacore::stats::TaskInstrument` is an object-safe enter/exit hook called
around every poll of the client's internal tasks and around its blocking
work. Wiring: `build()` wraps the runtime in `InstrumentedRuntime`, so all
spawns through the `Runtime` trait are covered without touching call sites.
The `Option` is resolved once at `build()` — `None` (default) leaves the
runtime untouched, so there is no per-spawn or per-poll cost when unset.
Installed, the decorator costs one allocation per spawn: `Runtime::spawn`
takes and returns an erased future, so wrapping it changes the type and needs
a fresh box. Nothing else on the path allocates: `MeteredFuture` is generic
over the future it wraps, and `Bot::run` stack-pins its own.

- `CpuMeter` (built-in): busy time (direct CPU proxy) + poll count via
  `wacore::time::Instant`. Works on wasm/embedded once a monotonic provider
  is registered.
- Custom hooks: allocator attribution (see `examples/alloc_tracking.rs` for a
  dependency-free pattern; `tracking-allocator` slots in the same way),
  ESP-IDF `heap_caps` sampling, etc. The library never learns what the hook
  does.

Scope caveats: the hook covers tasks spawned *by the client* through the
`Runtime` trait, plus the main run loop itself — `Bot::run` meters its own
future (`Bot::spawn` reaches it via `Runtime::spawn`), so the read loop is
covered on either launch path. Work executed on the caller's own task (e.g.
awaiting `send_message`) belongs to the caller — instrument that side
yourself if you need it. The `voip` feature's media tasks (call driver,
relay I/O) currently spawn directly on Tokio and are not instrumented.

## `Client::resource_report()` — out-of-client resource attribution (on demand)

`memory_report()` accounts only for the **client's own** in-process
collections (tens of KiB). The real per-session cost lives mostly **outside**
the `Client`: the storage backend, transport buffers + TLS/noise state, the
HTTP pool, and transient heap. `resource_report()` (`ResourceReport`) composes
all of these into one estimate. Same design rules: runtime/platform-agnostic,
zero cost when unused (LTO drops it), no PII.

How big "per-session" is depends on the backend, and the two profiles differ
enough that quoting one figure misleads (`memory_soak.rs` covers growth over
time, `process_footprint.rs` below covers the marginal cost of one more
client):

| profile | marginal RSS per session |
| --- | ---: |
| `InMemoryBackend` | ~530 KiB |
| `SqliteStore` (defaults) | ~530 KiB + ~512 KiB of storage |

Those are gross RSS, which is the pessimistic number; read the next section for
why the actionable part is `RssAnon` and how much smaller it is.

So the SQLite page cache **is** the single largest chunk, but only on the
SQLite profile, and it is roughly half the total rather than all of it. A
process on a remote or in-memory backend still pays the other ~530 KiB, of
which the largest named pieces are the prekey window the backend retains
(~104 KiB for the default 812 keys, but see the caveat below: that figure is
`InMemoryBackend`-only) and the transport's WebSocket + TLS buffers (64 KiB).
The HTTP idle pool used to belong on that list; the version fetch no longer
leaves one behind (see below), so a session whose only HTTP traffic is that
fetch pays nothing for it.

### Four measured per-session costs, and which of them survive scrutiny

Each of these was measured as marginal `RssAnon` in release against a control
that does everything except the thing being measured. Two of the four figures
that a call-site heap profile suggested did not survive that control, which is
the reason for measuring against one.

| what | measured | now |
| --- | ---: | ---: |
| HTTP: pooled TLS connection from the version fetch | 88 KiB | 0 KiB (#1243) |
| noise: batch buffer after one 60 KiB frame | 60 KiB, vs 8 KiB small-traffic | 8 KiB (#1246) |
| transport: retained `ClientConfig` | 14 KiB | 9 KiB (#1245) |
| topology log preallocation | 4 KiB | 0 KiB (#1244) |

**The prekey window is a backend artifact, not a per-session cost.** Building a
client and generating the default 812 prekeys, against a control that builds the
same client and generates none: `InMemoryBackend` 28 → 132 KiB, file-backed
`SqliteStore` 432 → 448 KiB. So the keys cost ~104 KiB of heap in memory and
~16 KiB on SQLite, but the SQLite client starts 404 KiB higher, so moving
backends relocates the cost rather than removing it, into page cache that
`RssAnon` counts and does not reclaim. The 104 KiB is also not waste: 58.7 KiB
of it is the single `Vec::with_capacity(gen_count * 74)` in `upload_pre_keys_pass`
staying alive because the `Bytes` slices handed to `store_prekeys_batch` *are*
what the backend stores (one allocation instead of 812), and 41 KiB is that
map's `RawTable` at 1024 buckets. Nothing to optimise; do not re-derive it.

That 41 KiB deserves one clarification, because a heap profiler hands it to you
under a name that invites the wrong fix. dhat attributes the final table to
`hashbrown::RawTable::reserve_rehash`, the frame that happened to allocate it,
so a per-session diff reads "41.0 KiB in reserve_rehash" and looks like rehash
churn. It is not: 1024 buckets × (`size_of::<(u32, PreKeyEntry)>()` + 1 control
byte) = 41,984 B is the table that *stays*, and the intermediate tables are all
freed before the process peak. `store_prekeys_batch` does reserve for the batch
length (#1270), which cuts the call from 11 allocations / 84.1 KB to 3 / 42.1 KB
and its in-call transient high-water from 63.1 KB to 42.1 KB — but retained is
bit-identical at 42,072 B either way, because the final table is the same size.
Reserving is worth it for the allocator traffic; it will never move the 41 KiB.

**The rustls session cache is 5 KiB, not 44.** A whole retained
`default_tls_connector()` measures 14.0 KiB; disabling resumption entirely takes
it to 9.0 KiB, and sizing the store for the one host a factory dials takes it to
9.4 KiB. The other 9 KiB is the config plus the webpki root store.

### Should a residency probe be permanent?

No, and the reason generalises. Every finding above that was worth guarding
turned out to have a **deterministic** guard available, and each of those is
strictly better than an `RssAnon` assertion:

- the HTTP pool, by counting accepted TCP connections against a keep-alive
  fixture (`an_ordinary_request_reuses_the_pooled_connection` and its
  `Connection: close` twin);
- the noise batch buffer, by extracting the release decision into
  `should_release_batch_buffer` (#1246) and testing it directly; the wire-level
  test could not see the buffer at all, and passed with the whole feature
  deleted;
- the topology log, by asserting the bound and the floor rather than the bytes.

An `RssAnon` assertion is page-quantised (the topology log's 8 KiB allocation
shows up as a 4 KiB delta, because `with_capacity` reserves without writing),
allocator-dependent, and needs a ceiling wide enough that it only catches
order-of-magnitude regressions. The probe's value was in *finding* the numbers,
not in re-checking them. Write one when you need a number; reach for a
deterministic observable when you need a guard.

To rebuild one: read `/proc/self/status`, construct N of the thing under test in
a loop while holding them all alive, and read the tail.

Keep the **median** there, as `process_footprint.rs` does, whenever the
per-step delta is comfortably above the 4 KiB page: it resists the outlier
steps, and the 88 KiB, 60 KiB and 14 KiB figures above are medians and are not
affected by what follows. It stops working once the per-step delta is within a
small multiple of the page, because then every step rounds to the same one or
two page counts and the median reports that rounding rather than the cost. The
topology log is the example: 8 KiB allocated, deltas alternating 32 and 36 KiB,
median moving by exactly one page. The **mean over the same tail** is the
sharper read there, and the two still agree on the answer: the median put the
saving at 36 -> 32 KiB, the mean at 34.9 -> 31.1 KiB.

Give each variant its own process (`--exact`, or nextest, which forks per
test): RSS never shrinks, so a second variant measured in the same process
reuses the first one's freed heap and reads as ~0. That one is not a rounding
artifact but a wrong answer, and it made a real 14 KiB cost look like 0.4 KiB
until the variants were re-run separately.

The pieces (each an `Option`-only struct in `wacore::stats`, filled only with
what a component can introspect — absent means "not reported", not zero):

- **Storage** — `DeviceStore::resource_report() -> StorageResourceReport`. A
  **defaulted method on the existing `DeviceStore` sub-trait** (next to
  `snapshot_db`), NOT a new `Backend` supertrait: `Backend` is blanket-impl'd,
  so a new supertrait would force every backend (incl. external) to add an impl,
  and an inherent method wouldn't compose through the `Arc<dyn Backend>` the
  client holds. A default on an already-implemented sub-trait gives both —
  composable *and* non-breaking. SQLite reports `min(cache cap, db size)` (an
  upper bound on the page cache; Diesel doesn't expose the raw handle needed for
  `sqlite3_db_status`), plus the DB page count. Remote backends report
  `memory_bytes: Some(0)`. `InMemoryBackend` sums its own maps (table
  allocations plus the heap its keys and values own), which is exact rather
  than a cap, because every byte it holds is this process's heap.
- **Transport** — `Transport::resource_report() -> Option<TransportResourceReport>`,
  a defaulted method (clean here — `Transport` isn't blanket-impl'd). The Tokio
  WebSocket transport fills best-effort static estimates (tokio-websockets and
  rustls don't surface live buffer sizes).
- **HTTP** — `HttpClient::resource_report() -> Option<HttpResourceReport>`,
  defaulted. With the default agent the `ureq` client reports `Some(0)`
  connections and `Some(0)` pool bytes until its first request, then its
  idle-pool buffer estimate. ureq allocates per connection, not per agent
  (`LazyBuffers` and the pool both start empty), so an agent that has never
  connected costs ~2.8 KiB of RSS against the 96 KiB the cap advertises, and
  reporting the cap there put ~28% of a session's `total_estimated_bytes()` on
  memory that was not resident. `Some(0)` rather than `None` because an empty
  pool is a measured fact, not an absence of introspection. Once a request has
  gone out the cap is a floor, not a ceiling: a pooled TLS connection measures
  ~98 KiB, of which the 32 KiB of ureq buffers is all this field claims. A
  custom agent reports `None` throughout — its buffer sizes are opaque, and
  since agents share one pool with all their clones it may already have
  connected before the client wrapped it, so its pool is not knowably empty
  either.
- **Alloc churn** — an `AllocSnapshot` from an `AllocMeter` (below), when one is
  installed.

`ResourceReport::total_estimated_bytes()` sums the **retained** components
(client + storage + transport + HTTP) and is documented as a **lower bound**;
`alloc` is churn, not residency, and is excluded. The future is `Send` (compile
guard in `accessors.rs`, per #964) so multi-session consumers can await it off a
worker.

### The version fetch does not leave a connection behind

`connect()` fetches `sw.js` over TLS through `version::resolve_and_update_version`
unless `with_version` is set or the cached version is under 24h old, so a session
that never touches media still opens one TLS connection. A pooled connection is
retained until something touches the pool again — ureq's `max_idle_age` never
fires, because `Connection::age()` returns zero — and the next fetch for that
device is a day away, so the connection would sit resident for the whole session
buying nothing. Measured at **88 KiB of `RssAnon` per session** (median over 16
agents, release, against a keep-alive TLS server).

`fetch_latest_app_version` therefore sends `Connection: close`, which ureq acts
on itself rather than waiting for the server to agree: `ureq-proto` records a
`ClientConnectionClose` reason at request-build time and drops the connection at
cleanup instead of pooling it. Measured marginal after the change: **0 KiB**.
Media requests are deliberately untouched — there the pool is what makes the next
range request cheap.

`mark_if_dispatchable` treats such a request as non-pooling, so a session whose
only HTTP traffic is the version fetch keeps reporting an empty pool instead of
latching onto the 96 KiB cap. It matches ureq's rule byte for byte rather than
RFC 9110's token list: ureq compares the whole `Connection` value to `close`, so
reading `keep-alive, close` as closing would report an empty pool for a
connection ureq had in fact pooled.

### Sharing one HTTP client across sessions

Still available, and now worth it for a process with pooled HTTP traffic —
media, in practice: build one `UreqHttpClient` and hand a clone to each
`BotBuilder::with_http_client`. Cloning
shares the `ureq::Agent`, and therefore the connection pool, so idle CDN
connections are paid once for the process instead of once per session.

Isolation: media URLs carry their auth per request and the client sends no
cookies (the `cookies` feature is off), so a shared connection carries nothing
between sessions that the shared source IP does not already carry — except TLS
session resumption, which lets a server link two sessions even across a source-IP
change. That is the reason sharing stays opt-in rather than the default.
Concurrency is unaffected: ureq opens a new connection whenever no idle one
matches the authority, so the pool caps idle retention, not throughput.

### `AllocMeter` — per-client allocation attribution (opt-in)

`wacore::stats::AllocMeter` is a first-class `TaskInstrument` (sibling of
`CpuMeter`) that attributes bytes allocated/freed to a client — the churn
counterpart to the point-in-time retained reports. The host installs a
`#[global_allocator]` that calls `AllocMeter::on_alloc`/`on_dealloc`; the meter,
installed via `BotBuilder::with_alloc_meter` (or `with_task_instrument`), marks
per thread which client's poll is running so the charge lands correctly.
`examples/alloc_tracking.rs` is the ~20-line reference.

Attribution boundary (documented honestly on the type): only allocations inside
instrumented polls/tasks are counted (the run loop is covered since #963; work
spawned raw on the runtime — some voip/media paths — is not). Deallocations are
charged to whichever meter is active at free time, so `allocated` (churn) is the
reliable signal and `freed`/`net` drift for buffers that outlive their poll.

### `SqliteStoreConfig::mmap_size` — page-cache tuning knob

`mmap_size` (new optional field, default `None` = current behavior; builder
`with_mmap_size`) emits `PRAGMA mmap_size`, moving reads to reclaimable,
file-backed pages — useful for a process holding many small per-session DBs. WAL
caveat: mmap covers reads of the main DB file; writes still go through the WAL.

## Fixed process cost vs per-session cost

A process that runs one session pays far more than a process that runs ten
divided by ten. `tests/e2e/tests/process_footprint.rs` measures the split:
`client_construction_footprint` (no mock server needed) for what a `Client`
costs before it talks to anything, `session_footprint_fixed_vs_marginal` for a
connected, paired, synced session. Both build N clients in one process
(`FOOTPRINT_CLIENTS`, default 16) and log a per-client table plus first /
second / median-marginal deltas.

Report the marginal and the fixed number, never the average over N: the average
is dominated by the first client and hides the very asymmetry the split exists
to expose.

Read the `anon` column. `/proc/self/status` splits RSS into `RssAnon` and
`RssFile`, and the first client's RSS delta is overwhelmingly `RssFile`: the
executable's own text and rodata faulted in the first time each code path runs.
That is bounded by the binary, shared by every session in the process, backed by
the page cache rather than by the allocator, and roughly 3.5x smaller under the
release profile than under `dev`. Anonymous growth is the part a consumer can
act on, and it is two orders of magnitude smaller.

Two consequences for anything measured this way:

- A one-off RSS jump on a path's first execution is not a leak and not
  attributable to the session that happened to run it first. Compare against a
  control step that does no work at all; in `dev` builds a single `println!`
  moves `RssFile` by ~2 MiB.
- Static tables (the binary protocol token maps, `webpki-roots`) never appear in
  `anon` at all, and the protobuf descriptor is consumed at build time, so none
  of them scale with sessions. The lazily built codec tables under the `voip`
  features do: they are `OnceLock` heap, process-wide, and only materialize once
  a call is encoded.

### Reading a heap profile next to it

`--features dhat-heap` (in `tests/e2e`) attributes retained bytes to call
sites, which the footprint numbers cannot. Three things about that profile are
easy to read backwards:

- **dhat sees only the Rust global allocator.** SQLite's page cache comes from
  the amalgamation's own `malloc`, so it never appears in a heap profile at any
  size, and a profile taken on SQLite looks the same as one taken in memory.
  The storage report and `RssAnon` are the only places it shows up.
- **Its end-of-run `curr_bytes` is a leak metric, not residency.** The profiler
  outlives the clients it profiled, so that figure describes what survived
  teardown. Residency is the peak-time figure, or a `resource_report()` taken
  while the clients are still connected.
- **The report cannot reach RSS, by construction.** Against a counting global
  allocator, glibc's anonymous RSS runs ~1.1x live heap, so
  `total_estimated_bytes()` is a lower bound on `RssAnon` before any component
  under-reports at all. Closing that last tenth is not a goal.

Not everything is expressible. The noise sender task's batch buffer grows to
`MAX_BATCH_WIRE_BYTES` and lives as a local inside a spawned task, so nothing
can read it without a channel built for the purpose; it stays unreported rather
than guessed. The safety net for the parts that *are* reachable is
`tests/report_coverage.rs`, which parses `Client`'s fields and fails when one
whose type names a collection never reaches `memory_report()`.

## Relation to the `metrics`/`tracing` features

`wacore::telemetry` (cargo feature `metrics`) emits process-global counters
through the `metrics` facade — no per-client dimension, by design (label
cardinality). The `stats` layer is the per-client dimension: snapshots you
poll and export however you like. `examples/multi_session_metrics.rs` shows
two clients in one process reporting independently.

# Adding metrics and inspecting cost

Keep `wacore::stats` platform-neutral. Use portable atomics and the pluggable
clock/runtime hooks; do not add Tokio, allocator or tracing dependencies there.
Counters on hot paths must avoid allocations and unbounded labels. Reports run
on demand. Snapshots contain numbers, never JIDs, message IDs or phone numbers.

Session wire I/O is recorded at the Noise socket send observers and read-loop
transport boundary, after the handshake. Count successful transport writes there,
not at every caller. VoIP relay traffic is a separate scope. Activity deadlines
use monotonic time; wall-clock timestamps in reports are derived when sampled.

Use the existing `Client::stats`, `memory_report` and `resource_report` methods
and their types for field definitions. Each retained collection needs reporting
or a reason in `tests/report_coverage.rs`. Inspect nested owners too. Distinguish
logical entries, capacity and estimated heap bytes. Include outer buffers and
queued protobuf payloads; do not count shared allocations twice.

Unkeyable-device counters count attempts, including aborted sends and retries,
not distinct devices or successful deliveries. Keep reasons disjoint and labels
closed. Per-message `RecipientFanout` explains what one send addressed.
Task CPU hooks report polled work, not wall time spent sleeping or blocked.

For memory measurements, separate live Rust heap, SQLite native allocations,
allocator-retained pages, RSS and file-backed residency. DHAT does not observe
SQLite's allocator, and end-of-run retained bytes describe teardown. A report's
estimate cannot prove total process RSS. Do not add an always-on residency probe
just to preserve one investigation; use ignored measurements and executable
benchmarks. Record measurements and machine details in PR evidence.

`plugins/wam` reports application telemetry through the existing lifecycle hooks.
Its catalog comes from `plugins/wam-catalog`; regenerated schema and call-site
files must share the whatspec pin. Report only observed application behavior;
a catalog entry alone does not justify inventing client facts. Exporter labels
must follow `wacore::telemetry`'s privacy/cardinality rules.

Use `.github/workflows/codspeed.yml` for CPU/allocation regressions and
`benches/connected_idle.md` for connected retention. A numerical result belongs
to the measured commit, flags and workload; do not turn it into a permanent
claim about current memory or performance.

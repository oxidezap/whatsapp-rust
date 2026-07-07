# Adversarial Review — Findings (P2 tier)

> `scripts/adversarial-review.workflow.js` (`args: { tier: "P2" }`), run `wf_547a6a13-1e8`: 40 agents, 0 errors, ~1.7M tokens.
> 11 findings confirmed after 3-skeptic adversarial verification (default-refute; majority-real to survive).

## Summary
| Sev | Group | File:line | Finding |
|---|---|---|---|
| **P2** | concurrency | `src/features/groups.rs:543` | Group-metadata cache uses an unlocked read-modify-write (get -> clone -> await persist -> insert) with no per-group lock, so concurrent participant... |
| **P2** | overhead | `wacore/src/usync.rs:59` | Duplicate, dead, behaviorally-divergent device-list parser in wacore/src/usync.rs shadows the live DeviceListSpec parser and mishandles the omitted... |
| **P2** | compliance | `wacore/src/iq/usync.rs:144` | usync per-subprotocol server backoff (`<error backoff=...>`) is parsed into UsyncSubprotocolError.backoff but never honored — there is no waitForBa... |
| **P2** | performance | `src/usync.rs:129` | process_device_list_response loads each user's existing DeviceListRecord serially, re-introducing the exact per-user sequential-DB-read anti-patter... |
| **P2** | compliance | `src/features/newsletter.rs:575` | Newsletter verification is matched against uppercase "VERIFIED" while the wire/WA use lowercase, so verified channels are always reported as Unveri... |
| **P3** | overhead | `wacore/src/store/persistence.rs:70` | Second, dead, divergent PersistenceManager in wacore is publicly re-exported but never used, and its get_device_snapshot violates the cached-Arc sn... |
| **P3** | feature-gap | `src/mediaconn.rs:26` | media_conn `max_buckets` and per-host `download_buckets` are parsed but dropped before download/upload route selection, so the vcache bucket-hash h... |
| **P3** | overhead | `src/features/blocking.rs:101` | is_blocked() issues a full blocklist IQ round-trip to the server on every call (no local cache), and the connect-time blocklist fetch is discarded,... |
| **P3** | feature-gap | `src/features/newsletter.rs:162` | Newsletter admin/management MEX operations are code-generated (doc_ids present in mex_operations.rs) but no Newsletter<'a> method surfaces them, so... |
| **P3** | performance | `src/portable_cache.rs:77` | PortableCache capacity eviction is insertion-order FIFO (get() never refreshes recency), but the code documents and reasons about it as LRU; bounde... |
| **P3** | compliance | `src/features/newsletter.rs:580` | Newsletter state and viewer role are parsed case-sensitively against lowercase literals whereas WA lowercases first, so any non-lowercase casing si... |

**Totals**: P2=5, P3=6 (11 confirmed).

---

## `review-features-surface-overhead` — Feature-Surface Gaps, IQ/usync, Group/MEX + Store Overhead

*P2 · group: feature-gap · raw 11 → confirmed 11*

**One live correctness race (group-metadata lost update drops SKDM recipients) and a case-sensitive newsletter parser (verified channels always read as unverified) are the actionable P2s; the rest are latent dead-code, perf, or additive feature-surface gaps.**

## Feature-Surface Gaps, IQ/usync & Group/MEX + Store Overhead

### P2 — correctness / compliance

**[P2] src/features/groups.rs:543 — unlocked read-modify-write on group-metadata cache loses concurrent participant updates**
Two concurrent mutations on the same group (`add_participants([A])` racing an inbound `w:gp2` notification, or two local calls) both read base `{X,Y}`, both `Arc::unwrap_or_clone`, both yield at `persist_group_metadata().await`, then last-insert-wins clobbers the other arm — participant A is silently dropped from the in-memory `GroupInfo`. Next send's `query_info` (src/send/mod.rs:1406) returns the stale snapshot, so the SKDM omits A and A can't decrypt until the ~1h TTL/invalidate. Same pattern in `remove_participants` (:578) and src/handlers/notification/groups.rs:137-203; notification tasks are `spawn().detach()`ed so they run genuinely concurrent, and `message_enqueue_locks` only covers message nodes.
*Fix:* hold a keyed per-group lock (like `session_locks`) across get..insert, or use an atomic entry-compute so overlapping updates compose.

**[P2] src/features/newsletter.rs:575 (+:580) — newsletter enum parsing is case-sensitive vs WA's lowercase wire values**
`match thread["verification"] { Some("VERIFIED") => Verified, _ => Unverified }` never matches the lowercase `"verified"` actually on the wire, so every verified channel reports Unverified; the sibling `state.type`/`role` matches (:580-606) use lowercase literals, confirming wire casing and making the uppercase arm dead. Same class at :580: `"SUSPENDED"`/`"OWNER"` fall through to Active/None. WA-Web `NewsletterParseUtils` `.toLowerCase()`es before every compare.
*Fix:* lowercase the extracted `&str` once and match normalized values for verification/state/role; treat any non-null verification != `"unverified"` as Verified.

**[P2] wacore/src/iq/usync.rs:144 — parsed per-subprotocol `<error backoff=...>` is never honored**
`UsyncSubprotocolError.backoff` is parsed (:158-161) then discarded — `warn_usync_result_error` (:407-418, called :763) only logs. On `<devices><error code="429" backoff="30"/>` the client warns and immediately fires the next `DeviceListSpec` usync, hammering a server that asked for a 30s pause (escalating rate-limits/bans). WA-Web `setProtocolBackoffMs(protocol, backoff*1000)` + `waitForBackoff` gate the next non-interactive usync.
*Fix:* keep a per-protocol backoff-until map; record `now + backoff*1000ms` in the devices error path and gate build/execute of non-interactive usync specs (DeviceListSpec/UserInfoSpec/LidQuerySpec), exempting interactive and message/voip `devices` contexts.

**[P2] wacore/src/usync.rs:59 — dead, divergent device-list parser shadows the live `DeviceListSpec`**
`parse_get_user_devices_response_with_phash` / `_response` / `build_get_user_devices_query` are pub but have zero workspace callers (live path is `DeviceListSpec::parse_response`). They diverge dangerously: :68-70 uses `.ok_or_else(...)?` so any user missing `<device-list>` aborts the whole batch and drops every other user's devices (live parser `continue`s per-user at iq/usync.rs:804-812); they also skip `<lid>` PN→LID mapping and `<devices><error>` handling. A future/external caller on an omitted-user response → group send encrypts to zero devices.
*Fix:* delete the three functions; keep only `UsyncDevice`/`UserDeviceList`/`UsyncLidMapping`/`parse_lid_mappings_from_response`, which `DeviceListSpec` actually consumes.

**[P2] src/usync.rs:129 — `process_device_list_response` reads each user's DeviceListRecord serially**
The `for user_list in &response.device_lists` loop awaits `load_device_record` per user (resolve_lookup_keys + up to two spawn_blocking SQLite reads on a cold cache), so a cold-cache large-group sync (N≥256) scales linearly on the send critical path. The sibling `get_user_devices` (:24-35) already fans the identical read out with `buffer_unordered(16)` and documents this exact hazard.
*Fix:* split into a parallel read phase (`buffer_unordered` over `load_device_record`) then the existing sequential build + single batched write.

### P3 — latent / feature-surface / efficiency

**[P3] wacore/src/store/persistence.rs:70 — dead, convention-violating duplicate `PersistenceManager`**
Publicly re-exported (`wacore::store::PersistenceManager`) but never instantiated; its `get_device_snapshot` is `async` and does `self.device.read().await.clone()` — a full Device deep clone under a read lock per call, violating the cached-`Arc<Device>` rule the live PM enforces, and it lacks `flush()`/`is_saver_halted()`/backpressure. Zero runtime impact today; the doc-comment invites consolidation onto it.
*Fix:* delete it, or make it the single impl but first port `get_device_snapshot` to the cached-Arc form so adoption doesn't regress per-message cost.

**[P3] src/features/newsletter.rs:162 — newsletter admin/management ops unreachable from public API**
`delete_newsletter`, `change_newsletter_owner`, `demote_newsletter_admin`, `create/accept/revoke_newsletter_admin_invite`, `fetch_newsletter_admin_info`, `fetch_newsletter_followers` are all code-generated with valid doc_ids in wacore/src/iq/mex_operations.rs but no `Newsletter<'a>` method surfaces them, so owners can't delete/transfer/manage a channel. Pure additive gap.
*Fix:* wire `Newsletter::delete/change_owner/demote_admin/…_admin_invite/followers/admin_info` to those modules via `mex().mutate/query`, matching existing create/join patterns.

**[P3] src/features/blocking.rs:101 — `is_blocked()` does a full blocklist IQ per call; connect-time fetch discarded**
No local cache: `is_blocked -> get_blocklist -> execute(GetBlocklistSpec)` round-trips every call, so app-side looping over N contacts = N IQs (throttle/drop risk). The connect-time fetch (node_io.rs:843-858) inspects `r_block` only for `Err` and drops the data; no `blocklist` notification arm exists to invalidate a cache. No internal caller loops it, so the storm is app-driven.
*Fix:* store the connect-time blocklist in an in-memory set, answer `is_blocked` locally, and add a `blocklist` notification arm applying incremental add/remove/full-sync updates.

**[P3] src/mediaconn.rs:26 — vcache bucket-hash host routing dropped; downloads always hit primary first**
`max_buckets` and per-host `download_buckets` are parsed then discarded at `MediaConnSpec::parse_response` (MediaConnHost has no buckets field) and `refresh_media_conn`; route selection is naive in-order iteration. WA-Web routes via `base64Modulo(encFilehash, maxBuckets)+100` against each host's `downloadBuckets`, so the Rust client misses the aggregated vcache entry (higher latency/CDN origin load). Functionally succeeds; only matters when server has `mms_vcache_aggregation_enabled`.
*Fix:* carry `max_buckets`/`download_buckets` through to runtime and order download requests by the enc-hash bucket, falling back to primary when absent.

**[P3] src/portable_cache.rs:77 — capacity eviction is insertion-order FIFO, not LRU as some docs claim**
`get()` refreshes only `last_accessed_at`, never the `seq` that `order.pop_first()` evicts on, so the hottest entry (lowest seq) is evicted first under capacity pressure. For `group_cache` (cap 250, 1h TTL) a client active in >250 groups within the TTL evicts its hottest group, forcing a `QueryGroupJob` re-fetch. Bounded/self-healing; several comments loosely say "LRU".
*Fix:* implement true LRU (re-stamp the order key on `get`), or correct the "LRU" doc claims to "FIFO" and re-evaluate `group_cache`/`session_locks` capacities against FIFO.


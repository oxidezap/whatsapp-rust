# Audit: `whatsapp-rust` top-level crate, `src/` (excluding `src/voip/`, `src/store/`, plugin crates)

Scope: `src/client/`, `src/send/`, `src/retry.rs`, `src/receipt.rs`, `src/handlers/`, `src/features/`, `src/message/`, `src/pair_code.rs`, `src/plugins/mod.rs`, `src/bot.rs`, etc. Line numbers are from the tree as read on 2026-09-02 and will drift; symbols are the durable anchor.

## Headline numbers

| | |
| --- | --- |
| `src/` total | 174,788 lines (incl. voip) |
| `Client` struct (`src/client.rs:1254-1874`) | **145 fields**, 26 `Mutex<>`, 3 `RwLock<>`, 13 `Cache<>`, 10 `OnceLock<>`, 44 atomics, 6 `event_listener::Event`, 7 `#[cfg(test)]` fields |
| `MemoryReport` (`src/client.rs:427`) | 57 fields mirrored by 57 `writeln!` lines and 10 `entry_count()` lines in `accessors.rs` |
| Error enums in scope | 39 `pub enum *Error` (13 of them in `src/features/*` with near-identical `Iq / Mex / InvalidRequest(String) / Internal(anyhow)` shapes) |
| `pub fn` vs `pub(crate) fn` | 757 vs 559 (excluding voip) |
| Log macros | 388 `log::*!`, 7 `tracing::*!`, 197 `feature = "tracing"` gates |

### Test-vs-code split of the large files (measured at `mod tests {`)

| file | total | code | tests | test share |
| --- | ---: | ---: | ---: | ---: |
| `src/send/mod.rs` | 8158 | 2983 | 5175 | 63% |
| `src/client/app_state.rs` | 6850 | 3685 | 3165 | 46% |
| `src/plugins/mod.rs` | 6558 | 3144 | 3414 | 52% |
| `src/retry.rs` | 5357 | 1756 | 3601 | 67% |
| `src/client/device_registry.rs` | 4124 | 1454 | 2670 | 65% |
| `src/receipt.rs` | 3631 | 1134 | 2497 | 69% |
| `src/client/lifecycle.rs` | 3569 | 2333 | 1236 | 35% |
| `src/client/lid_pn.rs` | 3183 | 1352 | 1831 | 58% |
| `src/features/groups.rs` | 3081 | 1973 | 1108 | 36% |
| `src/pair_code.rs` | 2699 | 1096 | 1603 | 59% |
| `src/client/extension_lifecycle.rs` | 2388 | 816 | 1572 | 66% |
| `src/handlers/notification/mod.rs` | 1839 | 116 | 1723 | 94% |
| `src/appstate_sync.rs` | 1782 | **5** | 1777 | 99.7% |
| `src/client.rs` | 2185 | 2185 | 0 (tests in `client/tests.rs`, 7833) | — |

So the "8k-line file" problem is mostly a **test placement** problem: six of the ten biggest files are >55% inline tests. Moving inline tests to sibling `*_tests.rs` files (the pattern `src/client.rs` + `src/client/tests.rs` already uses) is a zero-risk, purely mechanical change that halves the size of the files people actually edit.

Hot-path allocation hygiene is already good: `src/message/receive.rs` has 2 `to_string()` and 1 `format!` in 2,219 code lines; `src/send/mod.rs` 8 and 3 in 2,983. The std-Mutex-held-across-await scan found nothing in production code. The perf findings below are therefore small; the big wins are structural.

---

## Findings, ranked by impact

### 1. Move inline `mod tests` out of the six mega-files (mechanical, ~0 net LOC, huge editability win)

**Evidence.** `src/send/mod.rs:2983` (`mod tests {` runs to 8158 with three more test modules at 7867/7928/7957); `src/retry.rs:1756`; `src/receipt.rs:1134`; `src/client/device_registry.rs:1454`; `src/client/lid_pn.rs:1352`; `src/pair_code.rs:1096`; `src/client/extension_lifecycle.rs:816`. `src/client.rs` already does it the other way (`mod tests;` at line 2185 -> `src/client/tests.rs`).

**Change.** `#[cfg(test)] mod tests;` + `git mv` the body to `src/send/tests.rs`, `src/retry_tests.rs` (or `src/retry/tests.rs`), etc. Test code that reaches `super::private_fn` keeps working because a child module sees its parent's private items.

**LOC delta:** 0 (moves ~23,000 lines out of eight production files). **Risk:** none. **Behaviour-preserving:** yes. This should be done first because it makes every other split below reviewable.

### 2. `Client` is a 145-field god struct; `Arc<>` on 18 atomics and ~25 other fields is never cloned

**Evidence.** `src/client.rs:1254-1874`. Of the 18 `Arc<AtomicX>` fields, only `id_counter` (1 site) and `connection_generation` (3 sites) are ever `.clone()`d/`Arc::clone`d; `is_logged_in`, `is_connecting`, `is_running`, `is_connected`, `expected_disconnect`, `enable_auto_reconnect`, `is_ready`, `authenticated_generation`, `offline_sync_*`, … are all accessed through `&self` only. Same for `Arc<Mutex<()>>` fields `app_state_send_lock`, `prekey_upload_lock`, `signed_pre_key_rotation_lock`, `media_conn`, `ensure_inflight`, `group_metadata_inflight`, `ab_props`, `needs_initial_full_sync`, `app_state_syncing`, `offline_batch`, `offline_sync_metrics`, `history_sync_activity`, `outbound_flush`, `presence_subscriptions`, `pairing_cancellation_tx`, `pairing_qr_refresh_tx`, `pair_code_state` and all six `Arc<event_listener::Event>` notifiers (0 clone sites each). `Client` itself is always held as `Arc<Client>` (`self_weak: OnceLock<Weak<Client>>`), so a task that needs a field clones the client, not the field.

**Also:** `pairing_cancellation_tx` / `pairing_qr_refresh_tx` are `async_lock::Mutex<Option<Sender<()>>>` (`src/pair.rs:48,96,97,337`) guarding a pointer-sized `Option` with no await under the guard — a `std::sync::Mutex` (or `OnceLock`/`ArcSwapOption`) removes an async lock from the pair path.

**Change.** (a) Drop the `Arc<>` wrapper from every never-cloned atomic/mutex/notifier field (~40 fields): one less pointer indirection per read on `is_connected()`, `is_logged_in()`, generation checks, and a shorter `assemble()` (`src/client/lifecycle.rs:339-590`, 251 lines of `Arc::new(AtomicBool::new(false))`). (b) Then group fields into sub-structs by lifecycle domain, which the code already names: `ConnectionState` (is_connected/is_logged_in/is_ready/expected_disconnect/generation/backoff/notifiers), `OfflineSync` (offline_sync_*, offline_receipt_buffer, offline_batch, offline_terminal_*), `AppStateSync` (needs_initial_full_sync, app_state_*, initial_keys_*), `Pairing` (pairing_*_tx, pair_code_state), `RetryState` (pending_retries, message_retry_counts, session_recreate_history, resend_rate_limiter), `DeviceMemos` (device_registry_cache, device_topology, *_memo, device_memo_counters), `Waiters` (response_waiters, node_waiters, sent_node_waiters + counts). This does not need to change any behaviour; each sub-struct's fields keep their visibility.

**LOC delta:** -150 to -250 (mostly `Arc::new(...)` in `assemble` and `Arc::clone` noise). **Risk:** low for (a) — the compiler finds every clone site; medium-low for (b) — pure field renames but touches many files. **Behaviour-preserving:** yes.

### 3. `subsystem_boundary.md` test 2 is violated by ~30 `Client` fields that only one module reads

**Evidence.** Fields read by exactly one production module (per grep, excluding the declaration/`assemble`/`memory_report` mirror sites): `pdo_pending_requests` + `pdo_requested` (only `src/pdo.rs`, 10 sites), `pending_lid_refreshes` (only `src/client/lid_pn.rs`, 11), `pending_retries` + `session_recreate_history` (only `src/retry.rs`), `message_retry_counts` + `undecryptable_dispatched` (only `src/message/retry.rs`), `dispatched_messages` (only `src/message/dispatch.rs`), `skdm_warm_memo`/`group_distribution_locks` (send path), `pairing_*_tx`/`pair_code_state` (`src/pair.rs`/`src/pair_code.rs`), `app_state_key_requests`/`app_state_syncing`/`app_state_send_lock`/`needs_initial_full_sync`/`initial_keys_synced_notifier`/`initial_app_state_keys_received` (`src/client/app_state.rs`), `chatstate_handlers`/`chatstate_handler_count` (`src/features/chatstate.rs`). `agent_docs/subsystem_boundary.md` already lists `pdo` and `pair_code` as coupled precisely on this axis.

**Change.** Same mechanism as finding 2(b): each of these becomes a field of a per-module state struct that the owning module defines (`pdo::PdoState`, `lid_pn::LidPnState`, `retry::RetryState`, `app_state::AppStateSyncState`, …) and `Client` holds one field per state. This is the shape `Subsystems`/`subsystem!` already uses for `passkey`, so `pdo` and `pair_code` could go straight onto that seam and drop their rows from the "coupled" table.

**LOC delta:** -50 to -100 net (declaration + construction + memory-report lines collapse). **Risk:** low. **Behaviour-preserving:** yes.

### 4. `MemoryReport` / `CacheConfig` / `assemble` / `memory_report()` mirror every cache four times by hand

**Evidence.** Adding a `Cache<>` field today requires: the field in `src/client.rs:1254+`, a `max_capacity(cache_config.x_capacity)` builder in `src/client/lifecycle.rs:436-470`, a `x_capacity: u64` in `src/cache_config.rs:211-223`, a `pub x: u64` field in `MemoryReport` (`src/client.rs:427`, 57 fields), an `x: self.x.entry_count()` line in `src/client/accessors.rs:432-452`, and a `writeln!(f, "  x: {}", self.x)` in `Display for MemoryReport` (`src/client.rs:650-820`, 57 `writeln!`s). Six places, no compile-time check that they agree (a field added to the struct but not to `Display` compiles fine).

**Change.** Replace the 57-field `MemoryReport` with a `Vec<(&'static str, u64)>` (or a small `struct Entry { name, count, bytes }` list) built from a single table in `memory_report()`; `Display` becomes a 5-line loop; `SubsystemMemory` already uses that shape (`src/client.rs:598`). Public API note: `MemoryReport` is `pub` with `pub` fields — check for external readers before changing; a `fn get(&self, name) -> Option<u64>` keeps callers working.

**LOC delta:** -180 to -220. **Risk:** low-medium (public type). **Behaviour-preserving:** output text can be kept byte-identical.

### 5. PN⇄LID JID resolution is implemented eight times with slightly different semantics

**Evidence.**
- `src/client/lid_pn.rs:716` `resolve_lid_mappings` (loop, PN→LID by `get_current_lid`)
- `src/client/lid_pn.rs:753` `resolve_encryption_jid` (PN→LID, Hosted→HostedLid, keeps device/agent/integrator)
- `src/client/lid_pn.rs:924` `swap_pn_lid_namespace` (either direction, keeps device)
- `src/client/lid_pn.rs:1235` `resolve_recipient_to_lid` (PN→LID via `get_lid_pn_entry`, bare)
- `src/client/lid_pn.rs:1272` `refresh_lid_target` (LID→PN, bare, Hosted-aware)
- `src/send/tctoken_lifecycle.rs:371-420` `resolve_tc_token_key`, `resolve_to_lid_jid`, `resolve_issuance_jid` — re-implement `get_current_lid`/`get_phone_number` lookups instead of calling the above
- `src/features/blocking.rs:42` `resolve_lid_pn` (both, via `get_lid_pn_entry`)
- `src/features/polls.rs:187` `resolve_voter_jid` and `src/features/events.rs:141` `resolve_responder_jid` are **byte-identical** apart from one `log::warn!`.

Two of these go through the async entry lookup (`get_lid_pn_entry`) and the rest through the in-memory cache, so they can disagree on the same JID within one send.

**Change.** One `LidPnResolver` API on `Client` (or on `LidPnCache`): `to_lid(&Jid, Keep::Device|Bare) -> Option<Jid>`, `to_pn(&Jid, Keep) -> Option<Jid>`, `both(&Jid) -> Option<(lid, pn)>`. `resolve_encryption_jid` stays as the WA-Web-named wrapper. `tctoken_lifecycle` collapses to three one-liners; `polls`/`events` share one `own_jid_in_namespace_of(creator)` helper.

**LOC delta:** -120 to -160. **Risk:** medium — the Hosted/HostedLid and device-preservation differences are deliberate in places; the unified API must expose them explicitly. **Behaviour-preserving:** yes if the `Keep` flag is chosen per call site to match today's behaviour.

### 6. `src/client/app_state.rs` (3,685 code lines) is four modules in one file

**Evidence.** Structure (`impl Client` from line 815): scheduling/scope types (`SyncScope`, `BootstrapGate`, `SyncInFlight`, `BatchedSyncOutcome`, `CriticalSyncPlan`, lines 75-815), sync execution (`sync_collections_batched*`, `process_app_state_sync_task`, 1922-2660; `sync_collections_batched_inner` alone is 484 lines), key requests (`request_keys_and_wait` … `request_app_state_keys`, 2660-2920), patch send/conflict/recovery (`send_app_state_patch` 194 lines, `absorb_conflicting_patches`, `apply_recovered_collection` 232 lines, `escalate_to_snapshot_recovery`, 2920-3550), mutation dispatch (`dispatch_app_state_mutation`, `clean_dirty_bits`, 3550-3685). There is also a sibling `src/features/app_state_resync.rs` (1185) and `src/appstate_sync.rs` (see finding 12).

**Change.** Split into `client/app_state/{scope.rs, sync.rs, keys.rs, patch.rs, mutation.rs, tests.rs}`; break `sync_collections_batched_inner` at its three obvious phases (reserve → fetch/apply per collection → settle/report). No API changes: everything is `pub(crate)` on `Client`.

**LOC delta:** ~0 (maybe -50 from de-duplicating the retry/backoff scaffolding that `schedule_app_state_retry` (148 lines) and `schedule_app_state_task_retry` share). **Risk:** low. **Behaviour-preserving:** yes.

### 7. `process_session_enc_batch` is 749 lines / 13 indent levels; `handle_success` 578 lines; `send_group_branch` 388 lines / 10 levels

**Evidence.** `src/message/receive.rs:744-1493`: the per-payload `for` at +39 holds a 530-line `match decrypt_res` (`Ok` arm 30 lines; `Err` arm 500 lines covering DuplicatedMessage, UntrustedIdentity-then-retry, PN→LID migration retry, retry-receipt emission, nack). `src/client/node_io.rs:1007-1585`: `handle_success` spawns one task whose body is a linear post-login script (LID update, `lc` bump, pushname check, PDO session, prekey upload/rotate ordering, set_passive, offline drain arming, …) each guarded by a `still_valid!` macro. `src/send/mod.rs:2273-2661` (`send_group_branch`) and `:2661-2832` (`send_dm_branch`) are single block expressions nested to 10/9 levels.

**Change.** Extract `Err` arm of the decrypt match into `fn classify_decrypt_failure(...) -> DecryptFailure` + `async fn handle_decrypt_failure(...)` (the comment at `receive.rs:826` already says the nack code "mirrors the `handle_decrypt_failure` shape", i.e. the helper is half-written). Turn the `handle_success` task body into a `post_login_steps: [fn]` sequence or at least named `async fn`s per step so the `still_valid!` check lives in one loop. In `send_group_branch`, the "prepare" match that assigns four `let` bindings (`outbound_msg_secret`, `outbound_group_sender_identity`, `skdm_update`, `group_ack_phash`) is a natural `struct PreparedGroupSend` returned from `prepare_group_send()`.

**LOC delta:** ~-100 (less rightward drift; some shared code between the retry-emit branches). **Risk:** medium — this is the hot path and the Signal lock discipline (`session_guard` dropped around `try_pn_to_lid_migration_decrypt`, `receive.rs:761-765`) must be preserved exactly; keep the checklist in `signal_durability.md` beside the PR. **Behaviour-preserving:** yes if done as pure extraction.

### 8. Thirteen `features/*Error` enums with the same four variants; 68 files use `anyhow`

**Evidence.** `GroupError`, `NewsletterError`, `CommunityError`, `ProfileError`, `BlockingError`, `ContactError`, `PresenceError`, `ChatStateError`, `TcTokenError`, `PollError`, `MessageEditError`, `BusinessError`, `MexError` (`src/features/*.rs`) — all of the shape `{ Iq(#[from] IqError), Mex(#[from] MexError)?, Client(#[from] ClientError)?, InvalidRequest|InvalidJid|InvalidArgument(String), Internal|Other(#[from] anyhow::Error) }`. `ChatStateError` has a single `Client(#[from] ClientError)` variant. `plugins/mod.rs` alone has 23 `anyhow!(` sites and four `Plugin*Error` enums.

**Change.** One `features::FeatureError { Iq, Mex, Client, InvalidRequest(Cow<'static, str>), Internal(anyhow) }` plus per-feature `#[non_exhaustive]` enums only where a feature has a domain variant (`GroupError::DescriptionConflict`, `MexError::ExtensionError`, `PollError::NotLoggedIn`, `BusinessError::InvalidUpdate`) that wrap `FeatureError` via `#[from]`. Keep the names as type aliases for one release if they are public. Consistency rule for the doc: `thiserror` at the public boundary, `anyhow` only inside `pub(crate)` internals.

**LOC delta:** -120 to -180. **Risk:** medium (public error types; exhaustive matches downstream). **Behaviour-preserving:** yes, error text can be kept.

### 9. `BotBuilder` re-declares 17 `ClientBuilder` setters as forwarding wrappers

**Evidence.** `src/bot.rs` public fns vs `src/client/builder.rs`: common names `build, with_alloc_meter, with_cache_config, with_enc_handler, with_http_client(_arc), with_inbound_durability_hook, with_plugin(_arc), with_plugin_host_config, with_resend_rate_limit, with_runtime, with_task_instrument, with_transport_factory, with_untyped_plugin(_arc), with_wanted_pre_key_count`. Each is stored in a parallel `Option<…>` field on `BotBuilder` and re-applied to `Client::builder()` at `src/bot.rs:1482`. `bot.rs` is 1,531 code lines.

**Change.** `BotBuilder` holds a `ClientBuilder` and forwards with `self.client = self.client.with_x(v)`; or expose `fn client(self, f: impl FnOnce(ClientBuilder) -> ClientBuilder) -> Self` and delete the 17 wrappers (keep the type-state markers on the bot-specific setters only).

**LOC delta:** -150 to -250. **Risk:** low (public API shape retained if the wrappers stay as one-liners). **Behaviour-preserving:** yes.

### 10. `update_device_list_guarded` duplicates `update_device_lists_guarded`

**Evidence.** `src/client/device_registry.rs:747-823` (singular, 69 lines) and `:824-900` (plural, 76 lines) implement the same canonical-key resolve → cache insert with all aliases → backend write → canonical-flip invalidate/delete/invalidate sequence; the plural's doc comment even says "Same alias rule as update_device_list". The only behavioural difference is `backend.update_device_list(record)` vs `backend.update_device_lists(prepared)`.

**Change.** `update_device_list_guarded(record, guard)` becomes `self.update_device_lists_guarded(vec![record], guard).await` (if the backend's single-row method is not needed for a semantic reason — if `update_device_lists` opens a transaction that `update_device_list` deliberately avoids, note that in the one comment and keep the split of the *backend* call only).

**LOC delta:** -60. **Risk:** low. **Behaviour-preserving:** yes (modulo a transaction on the one-row path).

### 11. `src/plugins/mod.rs` (3,144 code lines) has 60+ types in one file

**Evidence.** Structure listing: `PluginCapability…PluginHostConfig…PluginManifest` (config, 57-210), `ClientPlugin`/`UntypedClientPlugin` traits, six error enums, `PluginResources`/`TaskTracker`/`TaskLease`/`GatedForwarding`/subscription types (378-1000), `PluginDiagnostics`, `PluginTasks`, `PluginCoreEvents`, `PluginStanzaInterception`, `PluginMessaging`, `PluginIq`, `PluginContext`, `PluginConnectionScope/Tasks`, `GuardedPluginTask`, `ApiRegistry`/`ErasedApi`, plugin adapters, `PluginPlan`/`InstalledPlugin`/`PluginInstallRollback`, `PluginHost` (2260-2906). `src/plugins/events.rs` already exists as a split-out.

**Change.** `plugins/{config.rs, traits.rs, errors.rs, resources.rs, context.rs, tasks.rs, api.rs, plan.rs, host.rs, tests.rs}`. `pub(crate)`/private visibility is already narrow, so this is a `git mv`-style split.

**LOC delta:** 0. **Risk:** none. **Behaviour-preserving:** yes.

### 12. `src/appstate_sync.rs` is a 5-line re-export carrying a 1,777-line test module with a hand-rolled `MockBackend`

**Evidence.** `src/appstate_sync.rs:1-3` re-exports `wacore::appstate_sync::{AppStateProcessor, AppStateSyncError}`; lines 7-1782 are `mod tests` defining `struct MockBackend` with stub `impl`s for `SignalStore`, `AppSyncStore`, `ProtocolStore`, `MsgSecretStore`, `DeviceStore` (~280 lines of `async fn … { Ok(None) }`), then 17 tests of `AppStateProcessor` — a `wacore` type. `src/test_utils.rs:402` already has `create_test_backend()`; `src/message/tests.rs:548-640` has yet another trio of in-memory `Sig*Store` impls.

**Change.** Move the 17 processor tests next to `AppStateProcessor` in `wacore` (they test wacore) or make them use `create_test_backend()`; delete `MockBackend`. Consolidate the `Mem*Store` trio in `message/tests.rs` onto the same helper. Whether the re-export file itself should survive is a semver question; if it stays it is 5 lines.

**LOC delta:** -300 to -400 in this crate. **Risk:** none (tests). **Behaviour-preserving:** yes.

### 13. Test fixtures: 240 helper fns in `message/tests.rs`, three `MockHttpClient`s, two `MockTransport`s, three test `Runtime` impls, 31 hand-built fake `<iq type="result|error">` nodes

**Evidence.** `src/message/tests.rs` (16,246 lines): 70 named non-test helpers including `capturing_client`, `capturing_client_with_cache_config`, `client_with_account`, `create_test_client_for_retry_with_id`, `mock_transport`, `mock_http_client`, `find_receipt`, `find_receipt_details`, `delivery_receipts_for`, `sender_receipts_for`, `message_acks_for`, `find_message_ack`, `find_message_ack_for`, `retry_request_stanza`, `status_stanza`, `group_skmsg_stanza`, … `MockHttpClient` is defined in `src/test_utils.rs:192`, `src/bot.rs:1542`, `src/features/presence.rs:260`. Fake IQ replies (`NodeBuilder::new("iq").attr("type","result"|"error")`) are hand-built 31 times across `app_state.rs`, `pair_code.rs`, `sessions.rs`, `rotate_key.rs`, `newsletter.rs`, `groups.rs`, `app_state_resync.rs`, `passkey/flow.rs` tests, while `test_utils.rs:87` (`server_error_iq`) and `:681` (`answer_iq`) exist. `create_transport` is implemented five times (`transport.rs:41,184`, `builder.rs:730`, `lifecycle.rs:2651`, `bench_support.rs:66`).

**Change.** (a) `test_utils::iq_result(id, children)` / `iq_error(id, code, text)`; (b) one `test_utils::MockHttpClient`; (c) a `sent_frames(client) -> Frames` query object with `.receipts()`, `.acks()`, `.messages_to(jid)` replacing the eight `find_*`/`*_for` scanners; (d) a `TestClientBuilder` (`.capturing()`, `.with_account(pn, lid)`, `.with_cache_config()`) replacing the `create_test_client_*` × 7 + `capturing_client*` + `client_with_account` family.

**LOC delta:** -800 to -1,500 across test code. **Risk:** none. **Behaviour-preserving:** yes.

### 14. `src/client/lifecycle.rs` mixes construction, run loop, connect graph, pause/resume, reachability

**Evidence.** `impl Client` (28-2165) contains `new`/`new_with_cache_config`/`assemble` (299-590, construction), `start_services`/`run`/`drive_connection`/`connect_graph` (590-1214, supervision), `logout`/`disconnect`/`reconnect*` (1214-1460), `pause`/`resume`/`wait_while_paused` (1460-1654), `cleanup_connection_state*` (1654-1924), `wait_for_*`/`reachability` (1924-2165) plus `Reachability` and `Connection<'a>` types. `cleanup_connection_state_inner` is 221 lines and `connect_graph` 214.

**Change.** `client/lifecycle/{construct.rs, supervise.rs, pause.rs, cleanup.rs, reachability.rs}`; `assemble` shrinks automatically once finding 2 lands (sub-struct `Default`s replace ~80 explicit `Arc::new(AtomicX::new(...))` lines).

**LOC delta:** -80 with finding 2, else 0. **Risk:** none. **Behaviour-preserving:** yes.

### 15. Small hot-path items

- `src/send/mod.rs:1286` `let to_str = to.to_string();` and `:2332` `let to_str = to.to_string();` in `send_group_branch`/DM branch: check whether `to_str` is only used for a log target/cache key; `Jid` implements `Display` so `%to` in the log and a `Jid` key avoid the allocation per send.
- `src/client/sender_keys.rs:148,199` `group_jid.to_string()` and `:276,435,457` `chat.to_string()` — sender-key lookups by `String` per group send; if the underlying store API takes `&str`, `write!` into a stack `arrayvec`/`CompactString` (`wacore_binary::CompactString` is already used for `group_ack_phash`) avoids the heap allocation.
- `src/client/node_io.rs:1674-1680,1766-1777,1899-1951` build `Event::Ack`/`Event::StreamError` payloads with `as_str().to_string()` for `id`, `class`, `error`, `code`; the builder fields could be `CompactString`/`Arc<str>` without changing the public event (they are `#[non_exhaustive]` builders). Ack events fire per sent message, so this is one alloc per ack.
- `Client::session_locks: Cache<String, Arc<Mutex<()>>>` keyed by `signal_addr_str` (`src/client/adapters.rs:36`) — `get_with_by_ref(&str, …)` is already zero-copy on the hit path; only the miss path allocates. Fine.
- `pending_retries: Arc<std::sync::Mutex<HashSet<String>>>` (`src/retry.rs:501-520`, `build_retry_processing_key` formats `chat|id|requester`) — one `String` per inbound retry receipt; a `(Jid, CompactString, Jid)` tuple key avoids it and the `Arc` is only used for the scopeguard closure (finding 2).
- Two `JidExt` traits (`wacore::types::jid::JidExt` and `wacore_binary::JidExt`) are imported together in 6 files (`send/mod.rs:13,20`, `retry.rs:1763-1764`, …). Not a perf issue, but a persistent source of `as _` renames; merging them upstream (out of this crate's scope) or a single re-export in `crate::types::jid` would remove the double import.

**LOC delta:** ~-20. **Risk:** low. **Behaviour-preserving:** yes.

---

## Not a problem (checked, so nobody re-checks)

- Locks across await: the scan for a `std::sync::Mutex` guard followed by `.await` within 6 lines finds only a test (`src/message/commit_batch.rs:1516`). `session_locks`, `group_distribution_locks`, `chat_lanes` use `async_lock`. `login_transition` is a std `Mutex<()>` taken at the top of `handle_success` (`node_io.rs:1009`); it is released before the spawned task, as the comment says.
- Feature gates: `voip-runtime` has 284 mentions but 275 are in `src/client/voip.rs`, `src/handlers/call.rs` and `src/voip/` (files it owns); the 9 outside match the documented cap. `plugins` 91 and `client-lifecycle` 72 are structural per the doc.
- `tracing` gates (197) are mostly `#[cfg_attr(feature = "tracing", tracing::instrument(...))]` on one line — cheap and consistent. Only 3 `#[cfg(feature = "tracing")] let …` shadow variables exist.
- `groups.rs` feature code (1,973 lines) is mostly thin `client.execute(XIq::new(jid, …)).await?` wrappers plus metadata-cache patching; the IQ specs live in `wacore/src/iq/groups.rs`. No hand-built IQs in production code anywhere in scope (all 31 `NodeBuilder::new("iq")` sites are tests).
- The device-memo pair `resolve_group_devices_memoized` / `resolve_dm_devices_memoized` (126/106 lines) share the generation-stamp discipline but differ in the invalidation key (member set vs bare jid) and the "complete only" rule; a shared generic would obscure the two comments that explain why. Leave.

---

## Quick wins (each ≤ 1 hour, zero behaviour risk)

1. `#[cfg(test)] mod tests;` + move for `send/mod.rs`, `retry.rs`, `receipt.rs`, `device_registry.rs`, `lid_pn.rs`, `pair_code.rs`, `extension_lifecycle.rs` (finding 1).
2. Delete `Arc<>` from the 16 never-cloned `Arc<AtomicX>` fields and the six `Arc<event_listener::Event>` notifiers on `Client` (finding 2a); compiler-driven.
3. `update_device_list_guarded` → `update_device_lists_guarded(vec![record])` (finding 10).
4. Replace `events.rs:141 resolve_responder_jid` and `polls.rs:187 resolve_voter_jid` with one shared helper (finding 5, smallest slice).
5. `test_utils::iq_result` / `iq_error` and one `MockHttpClient`; delete the `bot.rs` and `presence.rs` copies (finding 13a/b).
6. Delete `MockBackend` in `appstate_sync.rs` tests in favour of `create_test_backend()` (finding 12).
7. `pairing_cancellation_tx` / `pairing_qr_refresh_tx` → `std::sync::Mutex` (finding 2).
8. Split `plugins/mod.rs` by the type groups it already has (finding 11) — pure `git mv`.
9. `ChatStateError` (single `Client(#[from] ClientError)` variant, `src/features/chatstate.rs:13`) → `pub type ChatStateError = ClientError` or fold into the shared feature error (finding 8, smallest slice).
10. Table-drive `Display for MemoryReport` (finding 4) — 57 `writeln!` → one loop, output unchanged.

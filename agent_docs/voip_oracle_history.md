# Oracle investigation archive

Historical notes preserved from the operational guides during PR #1447.
Claims below describe experiments at their recorded stage, including hypotheses
later disproved. They are not current contracts. Use
[`tools/oracle-core/AGENTS.md`](../tools/oracle-core/AGENTS.md), the
[README](../tools/oracle-core/README.md), and
[conformance coverage](voip_conformance.md) for current behavior.

## Archived oracle instructions and investigation

# wa-wasm-oracle

Runs WhatsApp Web's shipped wasm modules and calls into them. Read `README.md`
first — it holds the module map, the recovered VoIP API, and the known limits.

This is unpublished host tooling. WhatsApp-specific imports, captures and
derivation specs belong here; only static fingerprint analysis comes from the
commit-pinned `unwasm-core`. Neither direction may introduce a dependency into
`wacore` or the application runtime.

## Build & verify

```sh
cargo fmt --all
cargo clippy -p oracle-core --all-targets --release -- -D warnings
cargo test --release -p oracle-core
cargo machete                 # no unused dependencies
```

`--examples` is in that line deliberately. They were outside it, and what
accumulated behind the gap was a dead helper carrying `flate2` — a whole
dependency kept alive by a function nothing called.

CI uses the repository's pinned nightly, currently new enough for Wasmtime's
Rust 1.95 floor. Runtime/MSRV jobs exclude this package through
`default-members`; changing that boundary would make a tooling dependency part
of the published portability contract.

Always `--release` for anything that executes a module. In a debug build
Cranelift compiles the 10.2 MiB VoIP module so slowly that runs look hung.

Tests exercise the real captures and skip when the capture directory is missing.
A skipped run is not a passing run: check for `skipping:` in
`cargo test --release -p oracle-core -- --nocapture` before trusting green.

## Where the modules come from

`cargo xt oracle fetch` puts them in `.cache/wa-wasm/`, pulled from the whatspec
`bundle-store` release and verified against the SHA-256s in `tools/oracle-core/wasm.lock.json`.
Do not commit them — a copy in the repository drifts from the capture the
protocol notes refer to. `WA_WASM_DIR` overrides the lookup.

**The lock pins hashes, not the latest capture, and that is the point.** Every
function index and absolute address in `README.md`, `agent_docs/voip_oracle_status.md` and the
tests — `infer_index(&bytes, 13_364)`, `read(1_719_816, 4)` — was read out of
these exact bytes. Repointing the lock at a newer WhatsApp module invalidates
all of them at once, silently: the reads still succeed, they just answer about
different code. Treat a capture bump as a re-derivation, never as an update.

## Ground rules

- **A stub that returns zero is a hypothesis, not an implementation.** The table
  in `README.md` lists what each zeroed stub actually cost: busy-waits in the
  millions, skipped static initialisers, threads that never ran. When a module
  misbehaves, read `hot_calls()` before suspecting the module.
- **Never look up an export without reporting a miss.** Go through
  `exports.rs`. A `let Some(..) = get_export(..) else { return; }` cost a day:
  the module exports `_emscripten_thread_init`, the host asked for
  `__emscripten_thread_init`, and the silent miss left the main thread
  unregistered — so every call a worker queued for it was dropped, and the
  symptom surfaced thousands of instructions away. A miss now names the near
  misses, normalising leading underscores and case.
- **A host import's declared type is not its argument order.** `(i32, i32)`
  says nothing about which one is the buffer. `env::get_random_bytes_js` takes
  `(len, buf)`, the host had it as `(buf, len)` by analogy with the
  `getentropy(buf, len)` sitting directly above it, and a request for 32 bytes
  at `0xf00000` became fifteen megabytes of PRNG written from address 32. That
  one transposition was the ring corruption, the `free`-refusing-a-pointer trap,
  and every "host and guest read different memory" theory this repository
  accumulated. Read the call site: the bytecode pushes the constant, and
  `oracle abi --index` shows it.
- **Determinism is the product.** Anything that would vary between runs — clocks,
  randomness, filesystem — must be replaced by something reproducible. A
  comparison against whatsapp-rust is worthless if the oracle's own output
  drifts.
- **Unsupported is an error, never a guess.** `call.rs` refuses types it cannot
  marshal, and `wasi.rs` returns `ENOSYS` rather than success for calls it does
  not implement. A wrong answer from an oracle is worse than no answer.
- **Derive host functions from the module's declared signature.** Emscripten
  changes these between releases — `_embind_register_bigint` takes five arguments
  in one capture and seven in another — so a fixed `func_wrap` breaks on the next
  module. `embind.rs`, `cxa.rs` and `wasi.rs` all build from `import.ty()`.
- **The main thread must not be able to block in wasm.** Pass
  `can_block = 0` to `__emscripten_thread_init` — the value emscripten itself
  uses on the web (`canBlock: !ENVIRONMENT_IS_WEB`). With 1, a waiting main
  thread takes `memory.atomic.wait32`, which blocks *inside* wasm with no host
  call: it holds its scheduler turn while blocked and the thread that would
  notify it never gets one. That was the "startup race", and it was the
  harness's, not the engine's — `initVoipStack` trapped about one attempt in
  six, and no amount of retrying would have fixed it. With 0 the main thread
  takes emscripten's busy-wait, which calls `_emscripten_yield` each time round:
  a host call, so the turn is yielded and the proxying queue drains.
  `startup_is_reliable_and_never_forces_a_turn` is the guard; `forced_turns()`
  must stay zero.
- **Guest threads are not serialised, whatever `schedule.rs` says.** Measured:
  `Runtime::max_threads_in_wasm()` peaks at **five or six**, in every round,
  healthy and corrupt alike. A thread acquires the turn once around its whole
  routine and `yield_point` hands it on only while somebody is blocked in their
  own first `acquire`, so once every worker has forced past `TURN_TIMEOUT`
  nothing waits and nothing yields. Making the turn cover exactly the
  guest-execution window does serialise and is unusable — a two-minute round
  had not finished in ten. Do not write code whose safety argument is "the
  scheduler holds all but one thread outside guest code"; that is not true
  today. `HostState::read` used to say it and no longer does: every byte of
  shared memory now goes through a relaxed atomic (`load_shared` /
  `store_shared` in `state.rs`), so a host read racing a guest write can still
  tear — which is what `forced_turns()` counts — but it is a defined outcome
  rather than undefined behaviour the compiler may optimise around.
- **A held turn is released by a guard, not by a paired call.** `run_thread`
  can fail with `?` between taking its turn and entering the routine —
  `__emscripten_thread_init` traps, `_emscripten_tls_init` traps — and a dead
  worker left as the recorded holder makes every later acquisition wait out
  `TURN_TIMEOUT` and force its way through. One failed initialisation then
  turns serialisation off for the rest of the run. `Scheduler::turn` returns
  the guard; `schedule.rs`'s own tests are the check.
- **Every test that starts an engine takes both locks.** `threaded_guard()`
  serialises within a test binary; `common::engine_lock()` serialises *across*
  them, because cargo runs the binaries in parallel and `threading`,
  `signaling` and `host_environment` all bring up PJSIP worker pools. Two pools
  competing for cores miss their own deadlines, and that surfaces as an
  unrelated-looking failure — `initVoipStack` trapping inside a test that is not
  about startup. The cross-binary lock is a TCP bind rather than a lock file:
  the OS releases a port when the process dies, so a killed test cannot wedge
  every later run.
- **Sweep, don't spot-check.** The `convertFixed32BitToFloat` model was wrong in
  a way that only showed up at `n >= 25`; a two-point test would have shipped it.
- **Inspection must not compile.** `inspect.rs` is `wasmparser` only, and
  resolves signatures by hand. Reaching for a runtime there trades 8 ms for
  seconds to learn something already present in the bytes.

  It must not *decode* either, and that is the sharper constraint. `unwasm`'s
  `Module::parse` reads the same file into a full instruction-level model, and
  measured on `JgwtTQVeWPm` that costs **~0.3 s against `oracle inspect`'s
  under 10 ms**. Sharing the decoder between the two projects looks like the
  obvious de-duplication and is the one to refuse: the streaming reads here —
  `inspect`, `xref`, `callers`, `konst` — are fast precisely because they never
  build the bodies. The parts worth sharing are the ones that are already
  cheap on both sides.
- **Keep the dependency surface honest.** wasmtime is `default-features = false`
  with an explicit list; adding a feature means something needs it. Run
  `cargo machete` before calling work done.
- **Identify by data segments, not `strings(1)`.** Dense wasm opcodes decode as
  printable ASCII by accident; the `strings` subcommand scans data segments only.
- **No real PII.** Test JIDs use fictitious `1555...` numbers.
- `unsafe` is denied workspace-wide. The shared-memory accessors in `runtime.rs`
  carry `#[allow]` plus a SAFETY note; do not add more without one.

## Where things live

Host environment, in the order a module exercises it:

- `emscripten.rs` — clock, PRNG, `invoke_*` trampolines, thread refusal
- `cxa.rs` — C++ throw/catch, exception messages via `__get_exception_message`
- `wasi.rs` — preview-1 subset over an in-memory filesystem
- `embind.rs` — recovers the registered API from the `_embind_register_*` calls
- `call.rs` — marshals C++ types and calls through the invoker table
- `patch.rs` — rewrites a module so a run can be traced: markers at entry, at a
  call site or at a return, plus instruction replacement. Reuses an import as
  the sink so nothing is renumbered, and splices raw bytes because a wasm body
  holds no absolute offsets. **Mark the call site, not the body** — a body patch
  reports whichever call ran, which is what made the old null-key measurement
  weak. **And the sink is chosen by name, not by signature**: the first
  `(i32, i32) -> ()` import is as likely to be `get_random_bytes_js`, whose
  marker call would write guest memory, as anything harmless. Only
  `RECORDING_ONLY_SINKS` is picked automatically; anything else has to be named
  through `Plan::sink` / `--sink`, and a module with no candidate is refused.

## Two host bugs worth not repeating

- **The guest memory is not always exported as `memory`.** mozjpeg exports it as
  `x`; a host looking only for the conventional name cannot read that module at
  all, and every read fails in a way that reads as "the module is broken". The
  name now comes from the module's export list.
- **A dropped `Runtime` used to leave its workers running.** A guest worker loop
  only ends when its fuel runs out, so finished tests kept burning CPU. `Drop`
  signals a shutdown that every host call checks.

## Signaling on `JgwtTQVeWPm`: eight of seventeen fail, and not for startup

Measured after the capture bump, before and after the per-thread stack fix.
**Identical both ways** on the first eleven — 7 failed, 4 passed — so these are
not the startup fault and the stack fix does not touch them:

```
a_phone_number_peer_is_refused_because_this_build_enforces_lid   FAILED
a_well_formed_offer_is_accepted                                  FAILED
a_well_formed_offer_reaches_the_call_stack                       FAILED
an_offers_settings_blob_becomes_readable_through_get_voip_param  FAILED
an_unmarked_settings_blob_is_rejected                            FAILED
call_id_must_sit_on_the_call_element                             FAILED
engine_accepts_the_framed_encoding                               FAILED
```

They encode the offer format worked out against `D5pLH9sfOOl`, one engine
complaint at a time, and this rollout tightened the offer path. The
expectations have to be re-derived the way they were derived the first time,
from what the engine now says; until then a green run of this file would mean
the assertions had been loosened rather than the format re-established.

### What the engine says now, and what that has ruled out

`examples/offer_probe.rs` is the tool: it raises the log threshold before
delivering, because the default lets through two lines and neither names a
cause. Run it against either capture — `cargo run --release --example
offer_probe -- D5pLH9sfOOl` — since the comparison is the point.

At the default threshold the whole rejection reads:

```
VoipSignaling.cpp:767 handleIncomingSignalingOffer from platform web version 2.3000.0
VoipSignaling.cpp:826 convertToXmppMsg() conversion_result FAILED
```

At level 9 it names itself:

```
call_jid.cc    wa_call_device_jid_from_string: invalid domain for device JID
wa_call_signa  parse_xmpp_offer: invalid call-creator jid call_id=...
wa_call_signa  handle_incoming_xmpp_offer failed to parse offer, status=70004
```

Established, each by measurement rather than by reading:

- **The same offer is accepted by `D5pLH9sfOOl` with no complaint at all.** So
  this is a check the rollout added, not a stanza this repository builds wrong.
- **It is `wa_call_device_jid_from_string` (func 11440) that refuses**, and
  `wa_call_device_jid_create` (11438) is never reached — marking every argument
  of `create` produced no hits. The guard people will find first in `create`,
  `a4 > 11`, is therefore not the one firing.
- **All twelve domains the module itself knows are refused**: segment 3 carries
  `smax_rs_jid::generated::AnyJid` — `s.whatsapp.net g.us lid msgr interop
  interop_msgr call broadcast newsletter bot hosted hosted.lid` — and each was
  delivered as a `call-creator`. So the fix is not a domain.
- **A device suffix changes which function refuses**, from `from_string` to
  `create`, so the parser wants one and something after it still objects.
- **The self JID passed to `initVoipStack` affects the complaint**: with a bare
  LID there, the `call_jid.cc` line disappears and only `invalid call-creator
  jid` remains. Whatever the creator is checked against, it is relative to the
  identity the stack was brought up with.
- **`from` is not what is checked.** The stanza carries both `from` and
  `call-creator`; varying `from` across phone-number, device and LID forms while
  holding the creator changes nothing at all. Only `call-creator` decides, which
  matches the message naming it.

### The rule itself, read out of the bytecode

`wa_call_device_jid_from_string` (11440) and `wa_call_device_jid_create` (11438)
share one test, and it is a bitmask rather than a list:

```wat
local.get 4        ;; the domain, as an enum
i32.const 11 / i32gtu / br_if     ;; anything above 11 is refused
i32.const 1 / local.get 4 / i32shl
i32.const 2600 / i32and / br_if   ;; accepted iff the bit is set
```

`2600 = 2^11 + 2^9 + 2^5 + 2^3`, so **exactly four domains are accepted: 3, 5, 9
and 11.** Func 11430 is the string-to-enum map and gives the names:

| domain | enum | accepted |
| --- | --- | --- |
| `s.whatsapp.net` | 0 | **no** |
| `g.us` | 1 | no |
| `call` | 3 | yes |
| `lid` | 5 | yes |
| `newsletter` | 8 | no |
| (address 112348) | 9 | yes |
| `hosted.lid` | 11 | yes |
| anything else | -1 | no |

**That is the change.** The previous capture took `s.whatsapp.net`, which is
enum 0 and is now the one value that gets its own log line rather than a silent
`70004` — which is why the failure reads as "invalid domain" rather than as a
rejected JID.

So a `call-creator` of `<user>:<device>@lid` gets past the parser, where
`<user>@s.whatsapp.net` cannot. Confirmed: `15550001111:0@lid` and
`…:0@hosted.lid` both reach `create`, and the `invalid call-creator jid` line
disappears with them.

### Why it fails, end to end — and why the wasm cannot finish the answer

The chain was walked with `oracle instrument`, one marked entry at a time,
because none of it is reachable by reading: `create` has a single direct caller
and is reached through the table instead.

```wat
;; fill_common_header_from_incoming_stanza (12635)
call 11447            ;; wa_call_jid_clone — it CLONES a JID, it does not parse one
local8  = frame[28]   ;; the cloned JID
local10 = local8[0]   ;; its domain
br_table (local10 - 5) [1 2 6 6 6 6 3 6 3 default 0]
;;   domain 5 (lid)  -> its own path, reading a field at jid+72 only a LID carries
;;   domain 0        -> create_default_from_user_jid -> create(0) -> refused
```

**The JID it clones is entirely zeroed.** Marking `wa_call_jid_clone`'s source
pointer and reading the struct back out of guest memory gives
`domain=0` and forty-eight nul bytes. It is not a JID parsed with the wrong
domain; it is one that was never filled — and domain 0 is both
`s.whatsapp.net` *and* the value a zero-initialised struct holds.

**And the previous capture has no such check at all.** `invalid domain for
device JID` does not appear anywhere in `D5pLH9sfOOl`; neither does the mask.
So the sequence is:

1. the offer stanza this repository builds omits some attribute,
2. the field it would have filled stays zeroed,
3. the old build read that as `s.whatsapp.net` and carried on, which is why
   every signaling test passed against it,
4. this build refuses domain 0, and the omission becomes fatal.

**Which attribute is not answerable from the module.** The code shows only a
clone of something that arrived empty; the parse that would have filled it never
ran, because the attribute was not there to parse. Nothing in the bytecode names
a string that is absent. Ruled out by measurement, each supplied in an accepted
domain: `from`, `to`, `call-creator`, `caller_pn`, `lid_user_jid`,
`call_creator_jid`, and the identity given to `initVoipStack`.

**The JavaScript bridge was read, and it does not correspond to this capture.**
`~/projects/sigilo/.corpus` holds the shipped bundle, and its VoIP bridge shows
the engine asking the host for a LID and the host answering by calling back in:

```js
requestLidJid = function(e) {
  onRequestLidJid(e).then(function(t) { handleLidJid(e.toString(), t) })
}
```

That is exactly the shape of the fault — an identity the host is supposed to
supply and this one never does, leaving the JID zeroed. But **`handleLidJid` is
not in `JgwtTQVeWPm`'s embind API**: the 212 registered functions contain no
`lid`, `pn` or `jid` entry at all. The bundle in that corpus is a *later*
WhatsApp build than this wasm capture, so its bridge names a callback this
module does not have.

**The version-paired bundle was then fetched, and it does not carry the bridge
at all.** whatspec's lock at the commit that shipped `JgwtTQVeWPm` names
`waVersion 2.3000.1043899084`, and its `bundle-store` release publishes
`bundles-2.3000.1043899084-…tar.xz` — the JS from the same rollout as this wasm.
565 files, and a search for `serializeVoipWapNode`, `handleIncomingSignaling`,
`call-creator`, `VoipSignaling` and `offer_notice` returns **zero** hits for
every one. The VoIP bridge is a code-split chunk fetched on demand when a call
starts, and neither archive collects it. (`wa-codegen-research`'s finding #1 is
the same fact from the other side: 75.8% of its corpus cannot be linked because
a dependency is a numbered chunk the fetch never got.)

So three sources have been read and none answers it:

| source | what it gives | why it stops |
| --- | --- | --- |
| the wasm capture | the JID arrives zeroed; the domain mask; the whole chain | nothing in bytecode names a string that is *absent* |
| `sigilo`'s JS corpus | `handleLidJid(pn, lid)`, exactly the missing shape | a later build — that callback is not in this module's 212 |
| whatspec's paired bundle | the JS from this exact rollout | does not contain the VoIP bridge chunk at all |

Closing it needs the VoIP bridge chunk captured against this rollout, and no
published archive has one. That is a gap in what is collected, not in the
analysis — and it is worth fixing upstream, because the same gap will block the
next capture bump too.

Until then the eight tests stay red, and that is the correct state: they encode
a format that this build no longer accepts, and making them green would mean
loosening assertions rather than re-establishing the format.

## The signaling tests are slow, and were flakier than this file claimed

They bring up PJSIP's worker pool and take about fifteen minutes, so they are
`#[ignore]`d and run on their own:

```sh
cargo test --release --test signaling -- --ignored --test-threads 1
```

This heading used to end at "not flaky", while `a_well_formed_offer_is_accepted`
failed about one run in four. Two more causes are now found and handled, both in
the module docs of `signaling.rs`: startup returning before `call_event_proc`
exists, and the engine's own lock watchdog firing on the offer path — the second
is `schedule.rs`'s doing, and the retry for it is conditioned on that complaint
and nothing else, so a real refusal still fails the test.

What was already known, and still holds:

- **Startup raced about one time in nine.** Measured with
  `examples/init_stress.rs`: `initVoipStack` finishes in ~5 ms, and the trap
  landed with 99.6% of the fuel untouched and the media init already logged as
  complete — two real threads reaching the same state. `schedule.rs` now runs
  one guest thread at a time, which took it to roughly 1 in 40; six retries
  cover the rest.
- **Offer handling is asynchronous.** The call returning says nothing about the
  event thread. Wait for the log to grow and go quiet, never for a fixed time.

Also worth remembering: **substring matches on log lines lie.**
`handleIncomingSignalingOffer from platform ...` contains `Offer from`, so a
test looking for the call-stack banner passed for a stanza that never parsed.

## Meeting a module you have never seen

`oracle abi` is the way in, and it is deliberately general — it reads bytecode,
so it works on captures that do not exist yet. The order that has paid off:

1. `oracle inspect <id>` — what it imports says which host environment it wants
   (`env::_embind_*` → emscripten/embind; `wasi_snapshot_preview1` → a WASI
   command; a shared memory → it expects threads).
2. `oracle strings <id>` — data segments identify the module. Never `strings(1)`
   on the whole file.
3. `oracle embind <id>` for an emscripten module; `oracle abi <id>` for anything
   stripped.
4. If `abi` reports a trampoline, read the vtable slot out of a live object and
   follow it with `--slot`.

## Open work

1. **Reconcile `participants[0]` with the bytecode.** `offer.cc:485` reads it as
   null, and three static facts say it cannot be — see "What `l1` is" in
   `agent_docs/voip_oracle_status.md`. One of the two is measuring something else, and the probe
   is the newer and less certain of them.
2. **Guest threads run concurrently — but no longer on one stack.** Each worker
   now takes the 64 KiB region the guest's own `pthread_create` allocated for
   it, installed the way emscripten's `establishStackSpace` does. That closed
   the startup failure on the `JgwtTQVeWPm` bump: a `std::string` in func 724's
   frame was being overwritten by a neighbouring thread, and `~basic_string`
   reached `free(1)`.

   **The lesson is about the measurement, not the fix.** The same change had
   been tried three times and written down as making things worse, because it
   was scored on `startVoipCall` — which fails for reasons of its own, so it
   could not tell "this made it worse" from "this changed nothing about a
   different bug". `oracle instrument` gives a signal that can: mark the
   deallocator and ask whether it gets a pointer or a small integer. Before
   re-running an experiment this file records as failed, check what it was
   scored on.

   What remains true is the concurrency: `Runtime::max_threads_in_wasm()` peaks
   at five or six when the design says one, and serialising properly is correct
   and unusable. Do not write code whose safety argument is "the scheduler holds
   all but one thread outside guest code".
3. **Drive a full call flow**: `initVoipStack` then
   `handleIncomingSignalingOffer`, and compare the recorded
   `sendSignalingXMPP_js_sync` payloads against what whatsapp-rust emits. The
   marshalling this needs is done. What is in the way is not the payload but the
   main-thread proxy queue — see `state.rs`: the engine queues its outbound
   stanzas there and every drain fails while `register_main_thread` is off.
   `init_stress --register-main-thread` measures what turning it on costs.
4. **Re-check `examples/outgoing_call.rs`.** It used to end with corrupted
   memory whichever stack the workers used, which is exactly what the
   `get_random_bytes_js` transposition did to anything that reached key
   generation. It has not been re-run since that was fixed. What follows is the
   pre-fix note:

   **What corrupts memory in `examples/outgoing_call.rs`.** It ends with traps
   whichever stack the workers use, while `examples/profiler_flag.rs` — same
   engine, same log level, same assert gate — has none. `startJsWorkerThread`
   and `initSctpRingBuffer` are what remain untested between them.
5. **Non-vector embind classes**, if a module ever registers one that matters.

`_start` exiting 71 on the media modules used to head this list. It was already
fixed by the WASI memory-window bug in the table above and nothing noticed,
because the MP4 core was the one module in the lock that no test exercised.
`the_mp4_core_reads_its_arguments` now does.

## Archived README section

## What a zero-returning stub costs

Every piece of the host environment here exists because stubbing it produced a
hang or a wrong answer, not because a spec said to implement it:

| stub | what it did |
| --- | --- |
| `emscripten_get_now` → 0 | startup busy-waited: **34 million calls** before the fuel ran out |
| `fd_write` → "wrote nothing" | libc retried forever: **3.2 million calls** |
| `invoke_*` → no-op | silently skipped **every static initialiser**, so the embind API looked empty |
| `__pthread_create_js` → success | reported a thread that never ran; whatever waited on it hung |
| `__cxa_*` → 0 | every `try`/`catch` broke, turning recoverable errors into opaque traps |
| a memory window read once | WASI wrote arguments into a stale window and reported success, so every media tool behaved as if invoked with no arguments — and exited 71 from inside its own panic handler |

`EM_ASM` is handled by source text when the module embeds it. Three stripped
selectors needed by the pinned VOPRF and VoIP captures are keyed by the hash of
their data section and index; every other missing, unreadable or unknown
snippet traps. The data fingerprint survives instrumentation, which rewrites
only code, while keeping a selector address from one capture from acquiring
meaning in another module that happens to use the same number.

With those implemented, `initVoipStack` returns, `handleIncomingSignalingOffer`
completes without raising, and a genuine C++ error arrives readable:
`std::invalid_argument: stoull: no conversion`.


## Archived README section

## What the offer format turned out to be

Established one engine complaint at a time, and none of it is guessable from the
embind signature:

| | |
| --- | --- |
| `call-id` | on the **`<call>`**, not the `<offer>` — otherwise `empty call-id` |
| `<voip_settings>` | a **sibling** of `<offer>`, not a child — otherwise `missing voip_settings` |
| `<voip_settings uncompressed="1">` | without the attribute the blob is read as compressed and the whole offer is rejected |
| arguments 4 and 5 | the stanza's `e` and `t` timestamps, read with `stoull` |
| timestamps | on the **guest's** clock (`virtual_unix_time`) — the host clock starts in 2021, so a real timestamp is from the future |
| the payload | base64 of the encoded stanza **including** the transport flag byte, though the JS glue drops it |

The `uncompressed="1"` answer came from whatsapp-rust, which sets the same
attribute when it builds an accept — a case of the two implementations checking
each other, which is the point of having both.


## Archived README section

### One flag was the startup race, the hang and the slowness

`initVoipStack` used to trap about one attempt in six, and the scheduler in
`schedule.rs` could only narrow it. It was read as a race inside the engine.
It was not: it was `can_block`.

`__emscripten_thread_init(ptr, is_main, is_runtime, can_block, ...)` decides
which wait a thread takes, and this host passed `1`. Emscripten itself passes
`canBlock: !ENVIRONMENT_IS_WEB` — **zero on the web**, because a browser's main
thread cannot use `Atomics.wait` either:

```c
// system/lib/pthread/emscripten_futex_wait.c
// For the main browser thread and audio worklets we can't use
// __builtin_wasm_memory_atomic_wait32 so we have busy wait instead.
if (!_emscripten_thread_supports_atomics_wait())
  return futex_wait_main_browser_thread(addr, val, max_wait_ms, cancelable);
```

With `1`, a waiting main thread takes `memory.atomic.wait32` — a wait that
happens *inside* wasm, with no host call. It holds its scheduler turn while
blocked, and the thread that would notify it never gets one. Deadlock, until
the turn times out five seconds later; that timeout was the "slowness", and the
forced turn was the "race".

With `0` it takes emscripten's own busy-wait, which calls `_emscripten_yield`
each time round. That is a host call, so the turn is yielded *and* the proxying
queue drains. Both problems close together:

```
                        can_block = 1     can_block = 0
initVoipStack           16/20             60/60
forced turns            non-zero          0
main-thread queue       traps             drains
```

Registering the instantiating thread as the main runtime thread is on by
default as a result, which is what `emscripten_main_thread_process_queued_calls`
requires.

The sentence that used to follow — that draining that queue is how the VoIP
engine's outbound signaling leaves, through table slot 436 — was wrong twice.
The trampoline that calls `sendSignalingXMPP_js_sync` is function **#855** at
slot **464** (`cargo run --example table_slot_of -- <module> 855`), and the
engine reaches it with or without main-thread registration: outbound signaling
is delivered on a bare engine, measured through the call *counters*. See
"Counting host calls" below.


## Archived README section

### What was lost

`wasmi` used to answer a second question — whether a module would load under a
`no_std` interpreter, i.e. whether it could ever be embedded. That signal is now
read straight from the module instead: `oracle inspect` reports a **shared
memory** as a threads requirement, which is what actually decides it. Cheaper,
and it cannot drift with a runtime version.


## Archived README section

## Real threads

`ThreadPolicy::Spawn` starts guest threads for real. A wasm thread is not a host
thread running guest code — it is a **separate instance of the same module, on
its own `Store`, over the same shared memory**, which is what emscripten's Web
Worker glue does. The function table is per-instance but identical in each, so a
function pointer means the same thing everywhere.

It is what brings the VoIP media stack up:

```
                       Refuse            Spawn
initVoipStack          120011            0
                       (EAGAIN)          endpoint.c  worker thread started
                                         wa_media_api. pjmedia_endpt_create = 0
                                         wa_opus.c   pjmedia_codec_opus_init success
                                         wa_call_event call_event_proc started
```

One more piece was needed: emscripten initialises the *main* thread through
`__emscripten_init_main_thread_js`, and while that was stubbed the runtime
believed no thread was the main one. Nothing failed immediately — but the first
time a worker tried to coordinate with the main thread, the main thread spun
forever waiting for one that never identified itself.

**Each guest thread now runs on the stack the guest allocated for it.** The
stack pointer is a per-instance global, so a new instance would otherwise start
from the module's initial `0x24cf60` and push its frames over whatever the main
thread has live there. `threads.rs` does what emscripten's `establishStackSpace`
does: reads the 64 KiB region out of `struct pthread` and installs it with
`emscripten_stack_set_limits` and `stackRestore`.

This had been tried three times and recorded as making things worse. What was
missing was a signal sharp enough to judge it — the earlier attempts were scored
on `startVoipCall`, which fails for unrelated reasons. `oracle instrument` gives
one: mark `~basic_string`'s deallocator and ask whether it is handed a pointer
or a small integer.

| | shared stack | per-thread stack |
| --- | --- | --- |
| `initVoipStack` | 8/12, then 20/20 | **12/12, 20/20** |
| last value freed | `1`, `2` on the trapping rounds | **always a pointer** |
| `threading` suite | 12 tests, 262 s | **13 tests, 169 s** |

The fault it fixes is worth stating precisely, because it was read as a race for
a long time: a `std::string` built in func 724's own frame was overwritten by
another thread's frame, so `__is_long_` read as set while `__data_` held a
neighbour's local, and `~basic_string` reached `free(1)`. See "A deleter is
handed the integer 1" in `agent_docs/voip_oracle_status.md`.


## Archived README section

## Working out an unknown module

A stripped module tells you `(i32, i32, i32) -> i32` and nothing about what
those integers are. `oracle abi` reads the function's own code: an argument
dereferenced as an address is a pointer, and the access width says what it
points at; one that only bounds a loop is a length; one passed straight through
is a handle.

```console
$ oracle abi php8T1oSIZM -f z
z (function #273)
  arg0  handle (passed through)            stored as a value x1, arithmetic x1
  arg1  length or count                    compared x1, forwarded x1
  arg2  handle (passed through)            stored as a value x1
  arg3  handle (passed through)            stored as a value x1
  arg4  length or count                    compared x1
  arg5  length or count                    compared x1
```

It needs no name section, no glue and no debug info, so it works on any module —
including ones not captured yet. `tests/abi_inference.rs` runs it across
toolchains: minified C++ and Rust/WASI alike.

**A trap frame is a way in.** A wasm backtrace names its frames by index and
nothing else, so `--index` takes one and gives back a name and a body. Constants
that address static text are quoted inline, which on a minified module is often
the only readable thing left:

```console
$ oracle abi php8T1oSIZM --index 53
func[53] (function #53)
  body:
    i32.const 211967  ; "called `Option::unwrap()` on a `None` value"
    i32.const 43
    call 146
    unreachable
```

That is how the mozjpeg module was worked out. Every call to `z` aborted in
`wasm function 151`; `--index` walked the frames to a Rust panic, and reading
`z`'s own body from there gave the six arguments: a `i64.store offset=180` of
`0xC_00000004` is `input_components = 4` next to `in_color_space = 12`, which is
libjpeg-turbo's `JCS_EXT_RGBA`. The pixels are RGBA, which is what every earlier
attempt had wrong.

**Trampolines are named as such**, because they change what a caller has to do:

```console
$ oracle abi COs9e0Kj0ic -f blind
blind (function #183)
  arg0  pointer (read, 4-byte access)      loaded 4B, forwarded x1
  arg1..6 handle (passed through)
  body:
    local.get 0 … local.get 6
    local.get 0
    i32.load offset=12       ← function pointer out of the object
    call_indirect type=12
    ^ trampoline: the real arguments belong to its callee.
```

`blind` takes no arguments of its own: it loads a function pointer from offset
12 of its first argument and jumps through it. So the first argument is an
object with a vtable, and `oracle abi <id> --slot N` follows that slot to the
function that really takes the arguments. Reading the slot out of a live object
(`examples/voprf_flow.rs`) shows which objects have it filled in — a Ristretto
curve carries slot 2, a VOPRF context slot 21, and an uninitialised KDF carries
nothing callable, which is exactly the difference between a call that works and
one that traps.

The output is evidence, not proof, and the limits are documented in `abi.rs`.
The scan is linear, so it follows one path: a parameter only touched inside a
branch comes back thinner than it is, and a slot the optimiser recycled as a
temporary is retired at the first store that does not write the parameter back —
evidence is given up rather than invented. That is why the counts are printed
next to the role rather than hidden behind it.

### A missing export must never be silent

The host asked for `__emscripten_thread_init`; the module exports
`_emscripten_thread_init`. The lookup was `let Some(..) = get_export(..) else {
return; }`, so nothing failed — the main thread simply never registered itself,
`emscripten_main_thread_process_queued_calls` then asserted
`emscripten_is_main_runtime_thread()` and trapped, and every call a worker
thread queued for the main thread was dropped. One underscore, and the symptom
thousands of instructions from the cause.

`exports.rs` is the answer: every lookup goes through it, a miss is always
reported, and the report names the near misses — normalising leading
underscores and case, which is exactly the class of difference a reader skips
over.

```
module exports no function named any of ["__emscripten_thread_init"];
  the module does export ["_emscripten_thread_init"] — spelling?
```

An optional export logs its own absence rather than returning `None` quietly,
because "this module has no `setTempRet0`" is a fact worth seeing when
behaviour later looks wrong.


## Archived README section

### Counting host calls

Three accessors answer "did the guest call this", and **two of them return zero
for reasons that have nothing to do with the guest**. An investigation into the
VoIP engine's outbound path ran for several rounds on such a zero and reached
the opposite of the truth, so this is worth reading before trusting one.

| accessor | what it really answers |
|---|---|
| `all_calls_to(sym)` / `shared().calls()` | the **first 8192** host calls of the run, with arguments |
| `shared().hot_calls()` / `total_calls()` | exact counts, unbounded |
| `stubs_called()` | only imports that got a **stub**; one with a real implementation never appears |

Bringing the VoIP engine up makes roughly **39 million** host calls, so the
recorded-call list is full within moments and every later query finds nothing.

```rust
// whether something happened — counters, exact
let sent = runtime.shared().hot_calls().iter()
    .find(|(s, _)| s == "env::sendSignalingXMPP_js_sync").map(|(_, n)| *n).unwrap_or(0);

// with what arguments — empty the list first, then measure a short stretch
runtime.shared().clear_trace();
runtime.call_embind("startVoipCall", &args)?;
for call in runtime.all_calls_to("env::sendSignalingXMPP_js_sync") { /* … */ }
```

`watch_markers` has the same shape of hazard: an instrumented copy records
nothing until the sink is named, so "the marker never fired" and "the marker was
never watched" are indistinguishable. Put a marker on a function you *know* runs
as a control before believing one that stays silent.


## Archived README section

## Differential testing

`tests/differential.rs` is the template for comparing against whatsapp-rust: the
oracle supplies ground truth, Rust supplies the candidate, and the test sweeps a
range of inputs.

It has already paid for itself. `convertFixed32BitToFloat` looked like
`value / 2^n`, and that formula passes for every `n < 25`. The sweep found that
the module returns `0` for `f(-1, 30)` where the formula returns `-9.3e-10`,
because the module adds an integer part and a fraction **in `f32`** and the
fraction rounds to exactly `1.0`, cancelling the integer part. A hand-written
test at a couple of points would have shipped the wrong model.

## Runtime and dependencies

**Inspection compiles nothing.** `oracle inspect` reads the module with
`wasmparser` and resolves signatures by hand from the type, import, function and
export sections. A 9.3 MB module is inspected in **8 ms**. The same information
used to cost a full compile.

**The engine log is read, not scanned.** The ring buffer has a 24-byte header
carrying the number of bytes written; `engine_log()` reads that count instead of
searching the allocation for printable runs. The earlier version was effectively
a memory scan, so unrelated heap bytes came back as extra log lines and two
probes of the same input could disagree — which is exactly what stalled the
offer investigation. `engine_log_overflowed()` reports when the ring has wrapped,
because past that point an index from an earlier read no longer means anything.

**And it refuses when it cannot be trusted.** `memory_view_is_coherent()`
re-reads a slice of the module's own static data, sampled once its constructors
placed it, and `engine_log()` returns nothing when that slice no longer matches.
It was written for a fault that destroyed the whole of linear memory about one
run in four — the host reading `env::get_random_bytes_js` as `(buf, len)` when
the module calls it `(len, buf)`, turning a 32-byte key request into fifteen
megabytes of PRNG written from address 32. That is fixed (see "The host was
writing the key material itself" in `agent_docs/voip_oracle_status.md`); the refusal stays,
because an oracle that answers from the wrong memory is worse than one that
declines to answer.

**Execution caches.** Compiled modules are cached on disk by wasmtime, keyed on
their bytes and the compiler settings. The captured modules never change, so
after the first run Cranelift is skipped entirely — which matters because the
harness builds a fresh instance per test and another per guest thread.

| | cold | warm |
| --- | --- | --- |
| `oracle call` on the 9.3 MB VoIP module | 4.0 s | **0.16 s** |
| full test suite (40 tests) | — | **15 s** |

**Dependencies are trimmed to what is used.** wasmtime is built with
`default-features = false`: the component model, GC, async, the Winch backend,
the WAT parser and debug tooling are all dead weight for a harness that runs
plain core-wasm modules off disk. `wasmi` was removed entirely once inspection
stopped needing a second runtime.

| | before | after |
| --- | --- | --- |
| clean release build | 1m 05s | **10.7 s** |
| `oracle` binary | 26.1 MB | **18.1 MB** |
| dependencies compiled | 380 | **237** |

Cranelift also runs at `OptLevel::None`: the oracle calls each function a
handful of times, so optimising the generated code costs more than it saves.


## Archived README section

## Known limits

- **An offer is accepted but not answered.** The engine now returns
  `wa_call_handle_incoming_xmpp_offer() status 0` and records the call, but ends
  it with `EVENT: Call missed by the user` and emits no outbound signaling. Two
  loose ends: the caller resolves to `0@s.whatsapp.net` (WhatsApp Web reads that
  attribute with `attrDeviceJid`, but passing a device JID changes nothing), and
  `record_incoming_msg: no active call` suggests something else has to create
  the call before an offer can be answered.
- **`ThreadPolicy` is a choice, not a default to ignore.** `Refuse` keeps a run
  fully reproducible and fails PJSIP's init; `Spawn` gets the engine running and
  weakens reproducibility to the milestone level. `PretendSuccess` exists for
  modules that only need a spawn to *appear* to work, and does not help here.
- **`getVoipParam` throws** before `initVoipStack`, and returns empty after.
  Genuine engine behaviour, and a useful signal that init took effect.
- **Only vector classes are marshalled.** `Uint8List`, `StringList` and `IntList`
  round-trip in both directions; a class with a non-default constructor or a
  non-vector shape would need its own path. Unsupported types are refused, never
  guessed.
- **Threads are refused, not emulated.** A module that genuinely requires a
  worker thread to make progress cannot run here.

The VOPRF module registers no embind API at all — it exposes plain C exports
(`sodiumInit`, `curve_init_ristretto`, `voprf_evaluate`). That is the correct
result for it, not a recovery failure, and a test pins it so the distinction
stays visible.

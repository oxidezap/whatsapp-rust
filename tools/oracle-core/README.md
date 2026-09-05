# wa-wasm-oracle

Runs WhatsApp Web's shipped WebAssembly modules and calls into them, so protocol
and media behaviour can be checked against the original artifact instead of
against a reading of its decompilation.

```console
$ oracle call JgwtTQVeWPm getWebP2PVirtualIpv4
Str("192.0.2.1")

$ oracle embind JgwtTQVeWPm
37 types, 206 functions, 3 classes, 30 methods
```

The modules are not vendored — they are WhatsApp's artifacts, and a copy
committed here would drift from the capture every offset in this file was read
out of.

## Getting set up

```sh
cargo xt oracle fetch     # captured modules -> .cache/wa-wasm, checked by hash
cargo build --release -p oracle-cli      # always --release; see below
cargo test --release -p oracle-core -- --nocapture
```

`cargo xt oracle fetch` reads `tools/oracle-core/wasm.lock.json` and refuses any payload whose SHA-256
does not match. Three sources, tried in order:

- **`static.whatsapp.net` — the capture's own origin**, one url per module,
  recorded in the lock. WhatsApp's CDN still serves the pinned 2025-05-27 bytes,
  all six verifying against the hashes here, so a clone with no credentials at
  all gets the full set from where the capture was taken. The path segment after
  `rsrc.php` is part of the address, not decoration: the same file under a
  different one is a 403.
- [oxidezap/whatspec](https://github.com/oxidezap/whatspec) `bundle-store` —
  public, and carries whatever set the current WhatsApp rollout serves. Four of
  the six modules are in its current set.
- `jlucaso1/wa-wasm-oracle` `captured-modules` — private, and carries the VoIP
  engine and MP4 core, which whatspec's rolling set no longer has. Needs a
  token: `GITHUB_TOKEN` or `GH_TOKEN`.

The release archives are the fallback for the day a capture rolls off the CDN;
until then nothing but network access is needed. The token is offered to GitHub
only — a token sent to the CDN would be a credential disclosed to a third party.

The oracle finds `.cache/wa-wasm/` on its own; `WA_WASM_DIR` or `--dir`
override the lookup.

**Always `--release`.** In a debug build Cranelift compiles the 10.2 MiB VoIP
module so slowly that a run looks hung.

Tests skip when a module they need is absent, and **a skipped run is not a
passing run** — check for `skipping:` in the output before trusting green.

### Following a capture forward

WhatsApp renames these files on every rollout, so the ids below are the capture
of 2025-05-27 and nothing more permanent than that. whatspec tracks the current
set in `generated/wasm.lock.json` and publishes it, which is how a newer module
is obtained: read that lock, take the id whose size and imports match the one
you want, and add it here.

What does *not* carry over is everything read out of the old bytes. The module
behind a name stays the same program; it is not the same binary. Measured
against the current set:

| | pinned here | current whatspec set |
| --- | --- | --- |
| VoIP engine | `JgwtTQVeWPm`, 10,650,934 B | `S_ivh1PriOA`, 10,856,103 B |
| MP4 core | `9Nbh3eMuVjD`, 2,985,612 B | `GtsvNqhytbm`, 3,698,978 B |

The VoIP engine moved here from `D5pLH9sfOOl` (9,794,866 B) so that this oracle
and [`unwasm`](https://github.com/oxidezap/unwasm) read the same bytes — an index carried between the two
means nothing otherwise. `JgwtTQVeWPm` has since rolled off whatspec's set too;
the CDN still serves it, which is why the lock records the url.

**What that bump cost, measured.** Four of the six modules are byte-identical
across the two sets, so only the engine moved. Of its functions, **6,561 carry
forward one-to-one** under `unwasm`'s fingerprint — which is the tool to reach
for, since it hashes a body's shape and signature while dropping exactly what a
rebuild changes. What did not carry is the code WhatsApp actually edited:
`make_and_cache_offer` and its callers, which is why the offer-guard offsets in
`signaling.rs` are marked as needing re-derivation rather than carried over.

The newer engine comes up under this host environment and registers its API, so
the *harness* moves forward unchanged. The recorded positions do not:
`abi_inference.rs` asks for `infer_index(&bytes, 13_364)` and
`signaling.rs` reads absolute address `1_719_816` — running the suite against
the newer engine fails the first of those, and the reads that still succeed are
answering about different code. Treat a capture bump as a re-derivation of every
index, slot and address in the tests, `README.md` and `agent_docs/voip_oracle_status.md`, and
bump the lock only once that is done.

## Usage

```sh
oracle list                                  # catalogued modules
oracle inspect <id> [--full]                 # sections, imports, exports, toolchain
oracle strings <id> [--min N]                # printable runs in data segments only
oracle instantiate <id>                      # bring a module up, report what startup did
oracle embind <id> [--full]                  # run ctors, list the registered API
oracle call <id> <function> [args...]        # call a registered function
oracle run <id> -f in.mp4 -o out=fixed.mp4 -- mp4repair in.mp4 out
oracle abi <id> [-f name] [--slot N] [--index N] [--body N]
                                             # infer what a function's arguments are
oracle xref <id> "<text>"                    # find the code that reaches a string
oracle xref-addr <id> <addr> [--window N]    # ... and the table base that holds it
oracle callers <id> <index>                  # walk back up the call graph
oracle instrument <id> --calls-in N [--sink env::name] -o out.wasm
                                             # trace which call sites ran
oracle patch <id> --replace F:AT:N:SPEC -o out.wasm
oracle derive --spec spec.json -o out/   # run a pinned derivation, write outputs + manifest.json
```

## Watching a run, not just reading it

`abi` lists a function's ten call sites and says nothing about which one ran.
`instrument` splices a marker — a call to an import the module already declares,
so nothing is renumbered — into chosen places, and the host reports which were
reached, in order:

```console
$ oracle instrument JgwtTQVeWPm --value 13894:0 --calls-in 13894 -o traced.wasm
JgwtTQVeWPm -> traced.wasm (10650966 bytes, was 10650934), markers call env::on_call_event_js_sync
   200000  value        local 0 at entry of func 13894
   200001  before-call  call 13895 in func 13894
   200003  before-call  call 606 in func 13894
```

`--value FUNC:LOCAL` reports a parameter rather than just presence, which is the
difference between "`free` trapped" and "`free` was handed `1`". That one
distinction is what identified the startup failure on this capture — see
`agent_docs/voip_oracle_status.md`.

**It marks the call site, not the body**, and that is the point: the previous
attempt at the same question patched the *bodies* of two functions with ten and
three call sites each, so the result reported whichever call happened to run
rather than the one being traced.

Bodies are spliced as raw bytes. That is sound because a wasm body holds no
absolute offsets — branches carry relative label depths — and the inserted
sequence is stack-neutral, so it is well-typed anywhere, including between a
callee's arguments and its `call`.

**The sink is chosen by name, not by signature.** `(i32, i32) -> ()` is also the
shape of `env::get_random_bytes_js`, which takes `(len, buf)` and writes: a
marker calling that one asks for two hundred thousand bytes of PRNG output at
address zero, and the instrumented module still validates and still runs. So
only imports this host answers without touching the guest are picked
automatically; a module whose candidates are all something else is refused, and
the refusal names them so one can be nominated with `--sink module::name`.

Two flags apply to anything that executes a module:

- `--threads` runs guest threads for real. Modules whose initialisation waits on
  a worker need it — the VoIP engine's media stack is one, and without it
  `initVoipStack` returns `120011` instead of `0`.
- `--log` attaches a ring buffer and prints the module's own diagnostics. The
  VoIP engine explains its failures there in detail.

```console
$ oracle call JgwtTQVeWPm initVoipStack 15550002222@s.whatsapp.net 0 '{}'
Int(120011)                                  # PJSIP denied a thread

$ oracle call JgwtTQVeWPm initVoipStack 15550002222@s.whatsapp.net 0 '{}' --threads --log
Int(0)
  wa_media_api.  pjmedia_endpt_create = 0
  wa_opus.c      pjmedia_codec_opus_init success
```

`oracle run` executes a WASI module against an in-memory filesystem: `-f` copies
a host file in, `-o guest=host` copies a produced file back out. The guest never
touches the real filesystem.

## What the captured modules are

| id | size | what it is | state |
| --- | --- | --- | --- |
| `JgwtTQVeWPm` | 10.2 MiB | **VoIP engine** — emscripten + embind + pthreads | API callable; `initVoipStack` fails ~40% of runs, see below |
| `COs9e0Kj0ic` | 234 KiB | **VOPRF** — `voprf_evaluate`, `verifiable_unblind`, Ristretto255, Naor-Reingold KDF, libsodium | exports callable, no embind |
| `php8T1oSIZM` | 373 KiB | **mozjpeg** — `imgoperations/wajs-mozjpeg-wasm` | instantiates clean |
| `rogm88TRRiw` | 2.0 MiB | **WebP / media** — `webpcheck.rs`, `libwamediacommon-rs` | **runs as a CLI** |
| `ayqr5HQtlkb` | 2.0 MiB | **MP4 utils** — check, repair, remux | **runs as a CLI** |
| `9Nbh3eMuVjD` | 2.8 MiB | **MP4 core** — `libmp4operations-rs`, stream-type tables | **runs as a CLI** |

`rogm88TRRiw` and `ayqr5HQtlkb` kept their name section and export readable
symbols (`ExamineH264Stream`, `ParseAACStream`, `convertFixed32BitToFloat`).

`9Nbh3eMuVjD` is the odd one: a *Rust* implementation with a `clap` command
line, next to the C++ tool suite that does the same job.

```console
$ oracle run 9Nbh3eMuVjD -f in.mp4=clip.mp4 -- mediautils mp4check in.mp4
MP4 file consistency: OK

$ oracle run 9Nbh3eMuVjD -f in.mp4=junk.bin -- mediautils mp4check in.mp4
Error: WamediaError(239: Unknown MP4 box topology)
exit: 1

$ oracle run 9Nbh3eMuVjD -f x=clip.mp4 -- classify x       # by content, not by name
Mimetype: Some("video/mp4"), Extension: Some("mp4"), Score: 0, Reason: 0
```

## The VoIP engine

Its 41 wasm exports are all emscripten plumbing — `malloc`, `stackSave`,
`__cxa_*`. None of the calling API is there, because embind registers it at
runtime against callbacks the JS glue would normally provide. Implementing those
callbacks recovers it:

```
handleIncomingSignalingOffer    (std::string ×5, bool, bool, std::string, Uint8List) -> void
handleIncomingSignalingMessage  (std::string ×5, bool, std::string, Uint8List) -> void
initVoipStack                   (std::string, std::string, std::string) -> int
startVoipCall                   (std::string, StringList, std::string, bool, ...) -> int
acceptCall  rejectCall  endCall  getVoipParam  setCallMute  raiseHand ...
```

The callbacks it calls *out* through are visible too —
`sendSignalingXMPP_js_sync`, `call_sendto`, `on_call_event_js_sync` — which is
what makes offer-in / stanza-out comparison possible.

## The media tools run

They are WASI command-line programs, and they work:

```console
$ media_run rogm88TRRiw input.webp
WebPFileInfo { num_frames: 0, canvas_width: 1, canvas_height: 1, ... }

$ media_run ayqr5HQtlkb mp4check input.mp4
ERROR : Found unknown/invalid top level MP4 box at file offset Some(32),
        error UnknownMp4BoxTopology
```

These are WhatsApp's own validators, so what they accept and reject *is* the
specification. `tests/media_tools.rs` asserts their verdicts.

## Reading the engine's own diagnostics

The VoIP engine writes structured log lines into a ring buffer the host
supplies. `Runtime::attach_log_ring` provides one, and `engine_log()` reads it
back. This is the difference between "the call returned void" and a diagnosis:

```
VoipInit.cpp:539  initVoipStack called enable_passthrough_video_decoder: 0
os_core_unix.     pjlib 2.13 for POSIX initialized
wa_media_api.     pjmedia_endpt_create = 120011
VoipInit.cpp:609  wa_call_init failed with return code 120011
VoipSignaling.cpp:767 handleIncomingSignalingOffer from platform web version 2.3000.0
WAWapReader.cpp:353   invalid list size in readListSize: token 8
```

Three things fell out of that output, none of which are derivable from the
embind signature:

- **The engine is PJSIP/PJMEDIA.** `120011` is `PJ_ERRNO_START_SYS + EAGAIN`.
- **The argument order.** `handleIncomingSignalingOffer` starts with five
  consecutive strings; the log echoes arguments 2 and 3 as *platform* and
  *version*, which is how they were identified.
- **The stanza encoding.** WhatsApp Web's bridge calls
  `handleIncomingSignalingOffer(serializeVoipWapNode(node), ...)`, and that
  helper is `base64(encodeStanza(node))` with the transport flag byte dropped.
  Dropping it makes the engine's reader fail at the first token
  (`invalid list size`); keeping it parses cleanly. Only the real implementation
  could settle that one-byte question.

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

With those implemented, `initVoipStack` returns, `handleIncomingSignalingOffer`
completes without raising, and a genuine C++ error arrives readable:
`std::invalid_argument: stoull: no conversion`.

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

## Determinism

Non-determinism is replaced, not removed: a virtual clock that advances per
observation, a seeded SplitMix64 PRNG behind `getentropy` / `random_get` /
`get_random_bytes_js`, and an in-memory filesystem. Single-threaded, two
instances given the same input produce identical results *and* identical
host-call traces, which `tests/host_environment.rs` asserts.

**Threads cost some of that.** `schedule.rs` gives back the largest piece: at
most one guest thread executes at a time, handing off at host calls. That is
what took the VoIP engine's startup race from about one failure in nine to one
in forty — two guest threads can no longer be inside guest code at once. It does
not make a run reproducible on its own, because which thread wins the next turn
is still the OS's choice.

The rest of what the harness gives back:

| mitigation | what it recovers |
| --- | --- |
| One clock behind a lock, shared by every thread | Time never runs backwards when execution crosses threads — the first thing a deadline loop would notice |
| A seeded PRNG **per thread**, keyed on the thread id | Each thread's sequence depends only on how many bytes *that* thread took. One shared stream would not survive threading: it is reproducible only if consumed in a reproducible order, and two runs can interleave their `random_get` calls differently |
| Every log line carries a global sequence number | The transcript can be put back in a stable order regardless of how the OS scheduled the writers |
| `emscripten_num_logical_cores` returns a fixed 4 | Worker-pool sizes do not depend on the machine the tests run on |
| `quiesce(timeout)` waits for every thread to finish | The *interleaving* is not reproducible, but the state after all threads have settled generally is. Reading before quiescing is a race with the module's own workers |
| One guest thread runnable at a time (`schedule.rs`) | No data races between guest threads. `forced_turns()` reports when a thread had to run without a turn — the escape hatch that keeps a waiting guest from hanging the host |

What that buys is a weaker but honest property: **milestones are reproducible,
interleaving is not.** `tests/threading.rs` asserts the former and deliberately
does not assert the latter — a test demanding identical thread interleaving
would be flaky by construction.

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

### Reading the outbound signaling

`sendSignalingXMPP_js_sync` is implemented rather than stubbed, because its
bytes only exist during the call: the trampoline that reaches it — function
#855 at table slot 464 — frees all three pointers on return, so a caller
reading the recorded arguments afterwards gets whatever the allocator handed
out next. `Runtime::signaling()` returns what the host copied:

```rust
for call in runtime.signaling() {
    // peer_jid, call_id, and the stanza as bytes
    let node = wacore_binary::marshal::unmarshal_ref(&call.stanza[1..])?;  // +1: stream flag
}
```

An origination on a bare engine produces one, and whatsapp-rust's parser —
sharing no lineage with the engine — decodes it:

```xml
<offer call-id="0011223344556677" call-creator="99887766554433@lid">
  <privacy>a5 … 32 bytes</privacy>   <!-- the tcToken passed to startVoipCall -->
  <audio enc="opus" rate="8000"/>
  <audio enc="opus" rate="16000"/>
  <net medium="3"/>
  <capability ver="1">01 05 f7 09 e0 bb 5b</capability>
  <enc count="0">32 bytes</enc>
  <encopt keygen="2"/>
</offer>
```

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

### Imports that cannot be recognised by name

Emscripten routes any call that might throw through an `invoke_*` trampoline:
first argument a table index, the rest the callee's own. Stubbing one is not
neutral — the call it should have dispatched silently does not happen, and the
guest reads back a result nobody produced. Minification takes the names away,
and mozjpeg's `jpeg_start_compress` was never called for exactly that reason:
the module built a compress struct, called `a.d`, and reported failure.

The generated code gives them away regardless. Emscripten clears a fixed
`__THREW__` word, makes the call, and reads the word back, and both halves name
the same address:

```wat
i32.const 224072 / i32.const 0 / i32.store   ;; __THREW__ = 0
<args> / call 3                              ;; the trampoline
i32.const 224072 / i32.load                  ;; did it throw?
```

`find_invoke_imports` scans for that shape and the runtime dispatches whatever
it finds, alongside anything still called `invoke_*`. It is checked against a
module that kept its names — all eight found, nothing else matched — before
being trusted on one that did not. The function table is looked up the same way,
by export rather than by the conventional `__indirect_function_table`, which a
minified module calls `A`.

## Reading values back

Building a vector and handing it to the module was only half the job: `get`
returns `emscripten::val`, a handle to a value on the JavaScript side, so
anything the module *produced* was unreadable. `emval.rs` keeps the handle table
the JS glue would, and `call_method` calls a registered method — whose invoker
takes `(context, this, args…)`, unlike a free function's.

```rust
let handle = runtime.build_vector(class, &[0, 7, 42, 255], &[])?;
runtime.read_vector(handle)?;   // [Int(0), Int(7), Int(42), Int(255)]
```

Two things fell out of doing it:

- **`bool` is not `int`.** A method registered as returning `bool` now comes back
  as `Value::Bool`; the wire is the same i32, but the caller asked a yes/no
  question.
- **embind's `set` does not bounds-check.** Its signature reads `-> bool`, which
  looks like validation, but it writes through `v[index]` and returns `true`
  regardless. An out-of-range index is undefined behaviour in the guest, not a
  rejected call. The oracle is what established that.

## Media round trip

`mp4repair` writes its output into the in-memory filesystem, so it can be read
back and fed to the checker:

```
mp4check  in.mp4   -> ok    H.264 (prf=100, lvl=10), 16 x 16, 1.00 fps
mp4repair in.mp4 out.mp4 -> ok    "file needs to be streamified"
mp4check  out.mp4  -> ok    (repair's own output is accepted)
```

The fixture is a real 16×16 clip from ffmpeg's synthetic test pattern.
Hand-built MP4s get as far as the H.264 parser and no further — these tools
validate the elementary stream, not just the container, which is exactly what
makes them worth using as a specification.

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

### What was lost

`wasmi` used to answer a second question — whether a module would load under a
`no_std` interpreter, i.e. whether it could ever be embedded. That signal is now
read straight from the module instead: `oracle inspect` reports a **shared
memory** as a threads requirement, which is what actually decides it. Cheaper,
and it cannot drift with a runtime version.

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

## Layout

```
tools/oracle-core
  catalog.rs      finding captured modules on disk
  inspect.rs      static inspection, wasmparser only, compiles nothing
  data.rs         data-segment extraction
  state.rs        per-thread host state and guest memory access
  host.rs         engine config, linker, and the stubs for unclaimed imports
  runtime.rs      one instance: calling it, its log ring, its lifetime
  shared.rs       cross-thread state: trace, clock, thread bookkeeping
  threads.rs      real guest threads over one shared memory
  schedule.rs     one guest thread runnable at a time, handing off at host calls
  emscripten.rs   deterministic clock/PRNG, invoke_* trampolines
  cxa.rs          C++ exception handling
  wasi.rs         deterministic WASI preview-1 subset with an in-memory filesystem
  embind.rs       recovering the registered API
  emval.rs        the emscripten::val handle table
  call.rs         calling registered functions, C++ type marshalling
tools/oracle-cli the `oracle` binary
```

Tests run against the real captures and skip when the capture directory is
absent. A skipped run is not a passing run — check for `skipping:` in
`cargo test --release -p oracle-core -- --nocapture`.

The signaling tests bring up PJSIP's worker pool and are `#[ignore]`d; see
`AGENTS.md` for how to run them and why they are quarantined.

## Driving the VoIP engine

`agent_docs/voip_oracle_status.md` is the place to start: where an outgoing call currently stops,
what the engine expects from the environment that a host has to supply, and a
table of hypotheses already ruled out by measurement — several of them
expensive, none worth repeating.

Two tools exist because inference from disassembly kept being wrong:

```sh
# Export a module's globals into a copy, so the host can read them.
cargo xt oracle export-globals <src.wasm> <out.wasm> <global-count>

# Read the engine's own view of a call — the only window into its state,
# since the call context is reachable from guest code alone.
runtime.call_embind("getCallInfo", &[])
```

## Licence and scope

The code here is licensed under the repository's [MIT license](../../LICENSE).

That covers this harness only. The captured `.wasm` modules it loads are
WhatsApp's, are not redistributed by this repository, and are not covered by
either licence. This is an independent interoperability and protocol-research
tool; it is not affiliated with, authorised by, or endorsed by WhatsApp or Meta.

## Reproducible MLOW oracle corpus

`cargo xt mlow verify` re-derives the codec corpus from the pinned
J/S captures. The lock verifies every output and selector; the capture CI
runs both modules independently. See [mlow_derivation.md](../../agent_docs/mlow_derivation.md) for the
recovered layouts, DSP boundaries, migration refusals and measured results.

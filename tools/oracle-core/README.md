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

Historical measurements and superseded hypotheses are in the
[investigation archive](../../agent_docs/voip_oracle_history.md).

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

## Real threads

`ThreadPolicy::Spawn` creates a separate instance and `Store` for each worker,
over the same shared memory. Each worker installs the stack allocated by the
guest, initializes pthread/TLS state, and reports initialization failures.
The main thread uses `can_block = 0` so waits can yield to host code.
`Runtime::drop` closes worker registration, wakes scheduler waiters and joins
every worker, including those still in host code.

Main-runtime registration remains disabled by default: synchronous proxy
queue draining can block startup. Experiments may opt in explicitly through
`set_main_thread_registration`; this does not establish complete call coverage.

## Determinism

Non-determinism is replaced, not removed: a virtual clock that advances per
observation, a seeded SplitMix64 PRNG behind `getentropy` / `random_get` /
`get_random_bytes_js`, and an in-memory filesystem. Single-threaded, two
instances given the same input produce identical results *and* identical
host-call traces, which `tests/host_environment.rs` asserts.

**Guest threads can execute concurrently.** The cooperative scheduler has a
timeout escape and cannot establish mutual exclusion. Shared-memory host
accesses use atomic bytes; multi-byte snapshots can still tear. Per-thread
stacks prevent workers from overwriting each other's stack frames.

The rest of what the harness gives back:

| mitigation | what it recovers |
| --- | --- |
| One clock behind a lock, shared by every thread | Time never runs backwards when execution crosses threads — the first thing a deadline loop would notice |
| A seeded PRNG **per thread**, keyed on the thread id | Each thread's sequence depends only on how many bytes *that* thread took. One shared stream would not survive threading: it is reproducible only if consumed in a reproducible order, and two runs can interleave their `random_get` calls differently |
| Every log line carries a global sequence number | The transcript preserves the observed order of that run; another run may interleave differently |
| `emscripten_num_logical_cores` returns a fixed 4 | Worker-pool sizes do not depend on the machine the tests run on |
| `quiesce(timeout)` waits for every thread to finish | The *interleaving* is not reproducible, but the state after all threads have settled generally is. Reading before quiescing is a race with the module's own workers |
| Cooperative scheduling (`schedule.rs`) | `forced_turns()` records timeout escapes; neither a held turn nor a zero counter establishes memory safety |

What that buys is a weaker but honest property: **milestones are reproducible,
interleaving is not.** `tests/threading.rs` asserts the former and deliberately
does not assert the latter — a test demanding identical thread interleaving
would be flaky by construction.

## Working out an unknown module

Start with `oracle inspect <id>` for imports, exports and toolchain, then
`oracle strings <id>` for data-segment strings. `oracle embind <id>` lists a
registered API; `oracle abi <id>` infers roles from bytecode without execution.

Use `abi --index N` for a trap frame and `abi --slot N` for an indirect-call
target. A trampoline's arguments belong to its callee: read the live object's
vtable before interpreting them. ABI roles are static evidence, not proof;
branches and recycled locals can limit the inference.

Export lookup uses `exports.rs`: missing exports report aliases and near
matches instead of silently skipping initialization. Capture-specific examples
and investigation results are retained in the history document.

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

| accessor | evidence |
| --- | --- |
| `all_calls_to(sym)` / `shared().calls()` | First 8192 host calls, with arguments |
| `shared().hot_calls()` / `total_calls()` | Exact counters throughout the run |
| `stubs_called()` | Exact counts restricted to unimplemented imports |

Use counters to establish whether an import ran. Clear the argument trace
before a short experiment when arguments matter; VoIP startup can fill it.
Marker recording requires `watch_markers` with the selected sink. A missing
probe or a saturated trace cannot establish that a callback never ran.

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

## Differential testing and runtime

`tests/differential.rs` supplies the pattern: execute the pinned capture,
compare with an independent Rust implementation, and sweep boundary inputs.
Numerical ordering and precision matter even when a simpler formula agrees
on a few samples.

Inspection uses streaming `wasmparser` reads. Execution uses Wasmtime with an
explicit feature set, Cranelift at `OptLevel::None`, and an on-disk compilation
cache keyed by module bytes and compiler configuration. Check the manifest
before changing the runtime feature budget.

Engine logs are read through the ring header, not by scanning memory for text.
`engine_log_overflowed()` returns a result: propagate a failed overflow probe
rather than treating missing log lines as evidence. The coherence probe can
reject logs after guest-memory corruption.

## Known limits

- Full audio/video callback ABIs and end-to-end signaling/IQ equivalence remain
  work for differential adapters. See the [coverage matrix](../../agent_docs/voip_conformance.md).
  The 26 ignored signaling scenarios are not proof of conformance.
- `Refuse` rejects worker creation; `Spawn` permits real concurrent workers.
  `PretendSuccess` is a diagnostic hypothesis and does not run a worker.
  Concurrent interleavings are not reproducible.
- Main-thread proxy draining is not fully modeled; observe actual callbacks
  and counters rather than inferring an absent send from incomplete diagnostics.
- `getVoipParam` requires initialization and settings supplied through the
  captured engine's settings path. An empty result is not a universal default.
- Vector classes (`Uint8List`, `StringList`, `IntList`) are supported. Other
  object layouts and unsupported wire types require explicit implementations.
- The VOPRF module exposes plain C exports rather than embind; an empty embind
  registry is expected for that module.

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
  schedule.rs     cooperative turns with timeout escape; not mutual exclusion
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

## Task layout and generated specs

`cargo xt` compiles only the lightweight repository dispatcher for hashes,
descriptors and CI metadata. Its `mlow` and `oracle` commands launch
`whatsapp-oracle-task` in release mode, where capture acquisition, derivation,
patches, media comparison and conformance are separate modules.

`cargo xt mlow specs` expands the committed bases and typed recipes into
`.derive-mlow/specs/`; `--check` compares generated bytes against the committed
hashes without requiring expanded files in Git. Verification materializes the
same specs automatically. CI publishes them with the run manifests.

WASI files, descriptors, arguments and streams belong to the process and are
shared by workers. `Runtime::wasi()` returns a lock guard: release it before
calling guest code. Unsupported `path_open` descriptor flags (including append)
return `EINVAL` before any file mutation. `oracle run --log` reports unsupported
logging explicitly.

Derivation output paths reserve `manifest.json` and are validated before any
step runs. Enum-table reads are limited to 65,536 entries and reject address
overflow. WASI validates `nwritten` before changing streams, files or offsets.

Selectors require a string anchor, fingerprint or `expect_body_sha256` for the
exact encoded body. `oracle abi CAPTURE --index N --body-sha256` computes the
latter for a reviewed function index; an index alone is rejected. Vector reads
are capped at 65,536 elements and derivation fills at 64 MiB. WASI validates
result pointers and all iovec ranges before changing buffers, files or offsets.

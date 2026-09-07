# Driving the VoIP engine: where it stops, and what has been ruled out

> **SUPERSEDED IN ITS CENTRAL CLAIM — read this first.**
>
> This file is written around "the engine reaches a ringing outbound call and
> then stops without putting an offer on the wire". **That is not true of the
> current captures, and the evidence for it was a measurement artefact.**
>
> On `S_ivh1PriOA` (the payload WhatsApp Web 2.3000.1044659339 names) and on
> `JgwtTQVeWPm`, with a LID peer, a filled device list and a 32-byte tcToken:
>
> * `startVoipCall` returns `0` and reaches `[None -> Calling]`;
> * `make_and_cache_offer` does **not** fail — no `70008` anywhere;
> * the engine builds the offer and hands it to the host:
>   `sendSignalingXMPP_js_sync(peer_jid, call_id, stanza, 179)`, once per
>   origination, on a bare engine with no glue, no A/B properties, no settings
>   blob, no `initSctpRingBuffer` and no `startJsWorkerThread`.
>
> Why it looked otherwise: `all_calls_to` reads a recorded-call list capped at
> `MAX_TRACE` (8192) while engine startup makes **~39 million** host calls, so
> it answers zero for everything that happens afterwards. Two neighbouring
> holes compound it — `watch_markers` must be armed before an instrumented copy
> records anything, and `stubs_called` lists only imports that got a stub. Use
> `shared().hot_calls()` for *whether*, and `clear_trace()` immediately before
> the measured stretch for *with what arguments*.
>
> Also corrected here: the sender's trampoline is function **#855** at table
> slot **464** (`examples/table_slot_of.rs`), not slot 436 as noted elsewhere.
>
> And the stanza is a stanza. `sendSignalingXMPP_js_sync` is implemented rather
> than stubbed so the bytes are copied while they exist — #855 frees them on
> return — and decoding them with **whatsapp-rust's** parser, which shares no
> lineage with the engine, yields:
>
> ```xml
> <offer call-id="0011223344556677" call-creator="99887766554433@lid">
>   <privacy>a5 a5 … 32 bytes</privacy>   <!-- the tcToken passed to startVoipCall -->
>   <audio enc="opus" rate="8000"/>
>   <audio enc="opus" rate="16000"/>
>   <net medium="3"/>
>   <capability ver="1">01 05 f7 09 e0 bb 5b</capability>
>   <enc count="0">32 bytes</enc>
>   <encopt keygen="2"/>
> </offer>
> ```
>
> The leading byte is the stream flag, so the node starts at +1. The `<privacy>`
> content is the tcToken handed in at the embind surface, which makes the path
> end to end. `Runtime::signaling()` returns these. See
> `examples/outbound_setup_matrix.rs` and `examples/call_the_sender.rs`.
>
> Everything below predates that correction. The ruled-out list, the
> instrumentation notes and the architectural reading of the P2P path are still
> useful; any sentence that turns on "nothing is sent" is not.

The engine comes up, negotiates media, and reaches the state a ringing outbound
call is in — and then stops without putting an offer on the wire. This is what
is known about why, so the next attempt starts from evidence rather than from
the reading that occupied most of the last one.

**Everything below the next section was measured on `D5pLH9sfOOl`.** The capture
has since moved to `JgwtTQVeWPm` so that this oracle and `unwasm` read the same
bytes. Indices carry across only where they are marked as carried: 6,561 of the
engine's functions survive `unwasm`'s fingerprint one-to-one, and the ones that
do not are the code WhatsApp edited — `make_and_cache_offer` and its callers
among them, which is why the offer-guard offsets are marked as needing
re-derivation rather than quietly updated.

## A deleter is handed the integer 1, and that is the startup failure

`initVoipStack` traps on roughly two runs in five on this capture. The chain,
measured end to end:

```
free(1)  ->  wasm trap: out of bounds memory access     (func 606 = free)
  -> invoke_viii reports the trap to the guest as a C++ throw, via setThrew
    -> func 13891 is a noexcept wrapper: "it threw" -> std::terminate
      -> the terminate handler calls abort(), the host answers Err,
         invoke_v reports *that* as a throw too -> terminate again
        -> 10,857 identical frames, no diagnostic
```

Only the first line is the engine's fault. The rest is this host converting a
hard trap into a catchable exception, which `emscripten.rs` documents and which
turns a one-line fault into an unreadable stack overflow.

**The value handed over is what identifies it.** `patch.rs` marks the entry of
func 13894 — the deleter the failing backtrace passes through, which reaches
`free` in exactly one place — with its own first argument. Twelve rounds:

| round | outcome | last pointer the deleter got |
| --- | --- | --- |
| 1, 4 | ok | `0x6`, `0x10` |
| 2, 3, 5, 6, 12 | ok | `0x2a9b08` |
| 9 | ok | `0x0` |
| **7, 8, 10, 11** | **trap** | **`0x1`, `0x2`, `0x1`, `0x1`** |

Every trapping round ends on 1 or 2; no passing round does. `free`'s own body
explains the cut exactly — it opens `local.get 0 / i32eqz / br_if 0`, so a null
returns immediately, and then reads at `ptr - 4`:

| argument | `ptr - 4` | what happens |
| --- | --- | --- |
| `0` | — | short-circuits, safe |
| `1`, `2` | `0xFFFFFFFD`, `0xFFFFFFFE` | **out of bounds, traps** |
| `6`, `0x10`, `0x86` | 2, 12, 130 | reads low memory, silent |

So the flakiness is not the bug. **The deleter is handed a small integer instead
of a pointer on every run**; whether that traps depends only on which small
integer it happens to be. Chasing the 40% would have been chasing the wrong
number — a fix that made it always answer `0x6` would look like a fix and change
nothing.

### What the integer is, read out of the bytecode

The chain above it is all static, and it names the object:

```
func 724      builds a std::string in its own frame:  local2 = (SP - 48) + 12
              ... then destroys it:                   call 14397(local2)
func 14397    ~basic_string:  if (is_long(s)) deallocate(*(u32*)(s+0), cap(s), 1)
func 13857    is_long(s)  =  *(u8*)(s + 11) >> 7
func 13859    cap(s)      =  *(u32*)(s + 8) & 0x7FFFFFFF
func 13891    deallocate(ptr, size, align) — noexcept, so a throw is terminate
func 13894    if (align > 8) aligned_free(ptr) else free(ptr)
```

That is libc++'s 32-bit `basic_string`, laid out `{ char* __data_; size_t
__size_; size_t __cap_ : 31, __is_long_ : 1; }` — and every field the
destructor reads is confirmed by the code that reads it, not assumed.

So the failing object is **a `std::string` whose `__is_long_` bit is set while
its `__data_` word holds a small integer**, and it lives in func 724's own stack
frame. That is not a value any string constructor produces; it is what a live
object looks like after something else has written over it.

**Which makes this the same fault as the open item about stacks.** Every guest
thread starts from the module's initial `__stack_pointer`, and
`Runtime::max_threads_in_wasm()` peaks at five or six — so 724's frame and
another thread's frame occupy the same addresses. The values seen in `__data_`
(0, 1, 2, 6, 16, 0x72, 0x86, 0x10e …) are small and clustered rather than
random, which is what a neighbouring frame's locals look like and not what
random garbage looks like.

`cargo run --release --example free_trap -- 12` reproduces the table above;
`--func`/`--local` point it elsewhere.

## Where it stops

`startVoipCall` reaches `[None -> Calling]` with SSRCs generated for audio,
video and screen-share on both participants, and then:

```
wa_call_tr  create_p2p_transport start
core/call_  wa_call_start_internal, make_and_cache_offer failed: 70008
events/ev   EVENT: Call offer send failed
```

**The offer is never built.** Nothing reaches `sendSignalingXMPP_js_sync`
because nothing was ever constructed to send. Any account of this that starts
from "the outbound channel is blocked" is wrong — that reading survived a long
time and cost accordingly.

### Finding the site that actually fires

`i32.const 70008` occurs **481 times** in the module — it is a data constant as
well as an error code, so any account built on a hand-counted subset is
guessing. (An earlier draft of this file said "nine sites at lines 296, 430,
463…". That was wrong, and chasing line 430 in particular cost several rounds.)

Every site pushes the same four bytes, and neighbouring values encode to the
same 3-byte sleb — so a copy of the module can give **all 481** a distinct code
without moving anything:

```sh
# site i -> i32.const (200000 + i), then read back which one the engine reports in
#   "wa_call_start_internal, make_and_cache_offer failed: %d"
```

Exactly one comes back: **200391**, which is function **11198**, and the
bytecode there carries the line number as a literal:

```
call 10297 ; local.tee 15
call 10530 ; local.tee 21
i32.eqz ; if
  i32.const 0 ; i32.const <file> ; i32.const <func> ; i32.const 485   <- offer.cc:485
  call 8502                                                          <- log
  i32.const 70008
```

### The failure, measured end to end

The lookup is not the problem. **The key handed to it is null**, so nothing is
ever compared. And the array that key comes from is not built inside the engine
at all — it arrives as an argument:

```
wa_call_start_call
  t5 = *(params + 0)          <- the participant array
  t6 = *(params + 4)          <- the count
  |
  +- 10425  wa_call_start_internal   [25 params; l3 = p3 = t5, l4 = p4 = t6, never reassigned]
       |
       +- 11198(ctx, l3, l4, ...)   [10 params]
            if it returns non-zero:
              "wa_call_start_internal, make_and_cache_offer failed: %d"
            |
            +- 10297(*(l3)) -> NULL          <- measured
                 10297(x) = (x == NULL ? log(line 158), NULL : x->[0])
                 |
                 +- 10530(ctx, NULL) -> 10535 returns before its loop
                                     -> 0 -> offer.cc:485 -> 70008
```

**So the null is `params->[0]->[0]`: the first entry of the array the host
supplies.** The engine does not assemble it, which puts the defect close to the
host boundary — quite possibly on our side of it, in how `startVoipCall` is
called.

**The "null key" step is the weak link, and it is weak for a specific reason.**
It was measured by patching the *body* of 10297 and of 10535 — and a body patch
reports whichever call happened to run, not the one on the path being traced:

| function | call sites |
| --- | --- |
| `10284` | 58 |
| `10530` | 32 |
| `10297` | 10 |
| `10535` | 3 |
| `11198` | 2 |

Measured against a single-caller body instead — `wa_call_start_call`'s own —
`*(params+0)` is `0x24bed0`, and dumping it gives `+0 = 0x24bf3a`,
`+4 = 0x24bf2e`: **the first slot is not null.** The engine's own log agrees the
list arrived: `num_peers 1` and `ACTION start_precall with 1 peers`.

So the chain above is right about *where* it fails and wrong, or at least
unproven, about *why*. **Never instrument the body of a shared function here.**
Patch the specific call site, which the decompiled source makes findable, or a
function with one caller — and always pair it with a control variant.

### The one instrumentation site that is above suspicion

The body of the `offer.cc:485` assert inside `make_and_cache_offer` — sixteen
bytes, `41 00 41 c9da2e 41 e1df12 41 e503 10 b642`, **unique in the module**, and
reached only when the offer fails. `11198` has two call sites and only `10425`'s
runs, so a patch here reports the failing path and nothing else. Six more bytes
follow (`41 f8a204 21 0b`, the `70008` and its store) and can be absorbed for a
longer expression, at the cost of the return value.

The mould: `41 24` (address 36), `20 00` (arg0), the loads, `36 02 00`, padded
with `01`. Everything below was read that way, two or three runs each, stable:

| read on the failing path | value |
| --- | --- |
| the key passed to the lookup | `0x6b1018`, `+8` → `"11223344556677@lid"`, len 18 |
| `ctx->[659164]`, the group | `0x8d0018` — **not null** |
| `group->[552]`, participant count | **2** |
| `participant[0]`'s jid `+8` | `"99887766554433@lid"` — self |
| `participant[1]`'s jid `+8` | `"11223344556677@lid"` — **identical to the key** |
| `participant[0]->[8]`, its state | 7 |
| `participant[1]->[8]`, its state | 2 |
| `participant[i]->[0]`, the loop's guard | non-null for both |

And the semantics, read rather than inferred:

* `f10284(a, b)` returns **1 on a match**: `if a == b return 1; r = (a==0||b==0) ?
  1 : pj_strcmp(a+8, b+8); return r == 0`.
* `f10530(ctx, key)` is not a plain lookup — it finds the participant and then
  **filters by state**: `s = *(p+8); if s <= 12 return ((5233 >> s) & 1) ? 0 : p`.
  The mask is bits {0, 4, 5, 6, 10, 12}; neither 7 nor 2 is in it.
* `f10619(group, i)` is `*(f3719(group+44, i, 127, 4))`, which confirms
  participants live at `group + 44 + i*4` — previously an assumption.
* `10535`'s loop walks indices 0 and 1 and compares both.

### What the group has nothing to do with

Those group readings all say the lookup *should* succeed, and that is the point:
it never gets far enough to use them. Reading the locals at the same site, with
a control:

| read at the failing site | value |
| --- | --- |
| control, stores `-1` | `0xffffffff` — the site runs, the address works |
| `l1`, the array `11198` was handed | `0x24bed0` |
| `l11`, i.e. `array[0]` | **0** |

And `0x24bed0` is exactly what `wa_call_start_call` was handed at entry, where
`array[0]` was `0x6b1108` and the JID beyond it was the peer's. **The array
pointer never changes; the contents of slot 0 are gone by the time the offer is
built.**

That closes the chain, and it also redeems a reading discarded earlier:
`get_participant` logs line **1382**, its null-argument branch, which is exactly
what a null key produces.

```
array[0] cleared  ->  10297 returns null  ->  10530(ctx, null)
                  ->  get_participant takes its null-argument branch (1382)
                  ->  0  ->  offer.cc:485  ->  70008
```

**The array lives on the stack.** At the moment of failure the stack pointer is
`0x24b8c0` and the array is at `0x24bed0` — 1552 bytes above it, inside the live
region, so this is not a use-after-free of popped stack. Something writes over
that slot in between. (The `l27 = SP - 16` in `10425`'s LID-consistency loop is a
temporary copy of the `{ptr, count}` pair, not the array; ruled out.)

Reading the same slot at three points narrows the window to one function:

| where | `array[0]` |
| --- | --- |
| `wa_call_start_call` entry | `0x6b1108` |
| `wa_call_start_internal` entry | `0x6b1108` |
| `make_and_cache_offer`, at the failure | **0** |

**So it is cleared inside `wa_call_start_internal`.** Instrumenting that entry
uses the same shape: the `call_lifecycle.cc:695` assert has a dead 16-byte body
at file offset 4505540, preceded by `45 04 40`; turning that into `1a 02 40`
(`drop; block`) makes the body run unconditionally, and the body becomes the
store. Check the sleb encoding against the bytecode first — `775533` is
`ed aa 2f`, and assuming `ad aa 2f` finds nothing.

The frame arithmetic says where to look. `11198` allocates 384 bytes and `10425`
allocates 1168; the stack pointer at the failure is `0x24b8c0`, and
`0x24b8c0 + 1552` is exactly the array's address. The array sits immediately
above `10425`'s frame, at `frame + 1168` — so anything writing at or past that
offset lands on it. Direct stores do not: the largest offset `10425` writes is
1164. That leaves writes through computed pointers, or a callee writing past its
own frame.

A fourth reading closes the window further. `make_and_cache_offer`'s own entry
already sees the slot at zero — measured with a control at the same site that
stores `-1` and does fire, and with `l1` reading `0x24bed0` as expected. So the
clearing happens inside `wa_call_start_internal`, **before** it calls the offer.

That entry is instrumentable the same way: the `offer.cc:409` assert (file offset
5095354, unique) is preceded by a `0d 01` at `at-3`; turning it into `1a 01`
(`drop; nop`) makes the body run every time. The function then returns 70004,
which does not matter — the store has already happened.

Two candidates are already ruled out. `memory.fill(l5+208, 0, 400)` in `10425`
has `l5 = SP - 640`, a fresh allocation below the stack pointer, so it writes
entirely below the array. And no direct `store(frame, N)` goes past 1164 against
a 1168-byte frame.

### Probing by fixed address, which is what made bisection practical

The array sits at `0x24bed0` on every run, so it can be read from *any* site,
including inside callees where no local holds it — thirteen bytes:

```
41 24  41 d0 fd 92 01  28 02 00  36 02 00
i32.const 36 ; i32.const 0x24bed0 ; i32.load ; i32.store
```

Put it in a **logging call whose message shows up in a real run**, not in a dead
assert body. The sequence `41 <file> 41 00 41 <msg> <args> 10 <logger>` runs
14–19 bytes, which is room to spare, and unlike an assert it is known to
execute. Always run the control variant (`41 24 41 7f 36 02 00`) beside it.

| where | `array[0]` |
| --- | --- |
| `wa_call_start_call` entry | `0x6b1108` |
| `wa_call_start_internal` entry | `0x6b1108` |
| `start_precall begin` log (offset 4506149) | `0x6b1108` |
| `create_p2p_transport start` log (offset 5315086) | **0** |
| `make_and_cache_offer` entry | 0 |
| the `offer.cc:485` failure | 0 |

**The slot is cleared between `start_precall begin` and
`create_p2p_transport start`** — the stretch where the log shows participants
being created and SSRCs generated.

One confounder, recorded because it produced a silent null result: these are
measured under `self_participant_probe`, and **not every site is on its path**.
The `"updating peer jid to"` log inside `10532` (offset 4581479) did not execute
there — control and probe both came back with the pre-existing `0xeeade615`. A
message appearing in an `outgoing_call` log does not mean the probe reaches it.

**Store `value + 1`, not `value`.** Address 36 is not reliably zero — an
unpatched run already has `0xeeade615` sitting there — so a probe that reads a
garbage-looking word cannot be told apart from a probe that never ran. Adding
`41 01 6a` before the store costs three bytes and makes 0 mean "did not run".

It paid for itself immediately. Probing the SSRC log site
(`call_generate_ssrc_for_participant`, offset 4667237, 18 bytes):

| run | `*(36)` | reading |
| --- | --- | --- |
| 1 | `0x6b1109` | `array[0]` is `0x6b1108` — **still populated** |
| 2 | `0xeeade615` | did not store; the site is not reached every run |

Without the `+1`, run 2 reads as "already cleared" and the window closes on the
wrong side. **The window is now between SSRC generation and
`create_p2p_transport start`.**

### It is not one instruction — the clearing is non-deterministic

Probing `"updating peer jid to"` (offset 4581479, inside
`wa_call_group_create_participant`) three times, same binary, same point:

| run | `*(36)` | reading |
| --- | --- | --- |
| 1 | `0x6b1109` | `array[0]` populated |
| 2 | `0x6b1109` | populated |
| 3 | **`0x1`** | **already cleared** |

At a fixed point in the code the slot is sometimes alive and sometimes not. That
rules out "find the store that writes zero" and reframes the whole thing: the
array is a **stack temporary** — measured at `SP + 1552` when the offer fails —
and the engine runs guest threads. **Its lifetime does not cover its use.**

It also accounts for the run-to-run spread recorded above (24 versus ~200 log
lines from the same unpatched module), and for why the group's participants are
correct while the raw array is not: those were copied.

### A hypothesis that led for a long time and is wrong: the shared stack

**Refuted.** Read this section as a record of where the investigation went, not
as an open lead. Everything it observes is true; the conclusion is not. Giving
each worker its own 4 MiB stack, confirmed from inside the guest with
`stackSave`, changes nothing — 27 log lines and eleven traps either way. What
actually killed the workers was `f12302`, the thread-status profiler, and what
it was reading was corrupted static data. See "The thread-status profiler was the
blocker".

Not how the list is passed — that was checked. `Runtime::build_vector` builds the
`StringList` with the engine's own constructor and `push_back`, exactly as the JS
glue does, so it is a guest-heap object. The array at `0x24bed0` is a stack copy
`startVoipCall` makes for itself.

The suspect is `tools/oracle-core/src/threads.rs`. Guest threads here are
**separate module instances over one SharedMemory**, and `__stack_pointer` is a
**per-instance** global. The code deliberately skips `establishStackSpace`, on
the grounds that `_emscripten_thread_init` already gives the thread its stack in
this build. **If that is not true**, every thread starts from the module's
initial stack pointer and they all write over the same region.

That would account for every symptom at once: a stack temporary zeroed
non-deterministically at a fixed point, the 24-versus-200-line spread between
runs of the same module, the traps inside `startVoipCall`, and workers dying on
wild addresses.

### What the run has been saying all along

Two things sit in `outgoing_call`'s own output and went unread for a long time.

**Call events fail to serialise.** Four times a run, on a thread that is not the
main one:

```
VoipEvent.cpp:88 Error converting call event data to JSON:
  [json.exception.type_error.305] cannot use operator[] with a string argument
  with number, for event: 16
```

Event 16 is **"Call state changed"** — `oracle enum D5pLH9sfOOl 0x126348` reads
the name table, and it is the same event the log shows as `[None -> Calling]`
just before the failure. The most basic event of a call's lifetime, and the
engine builds it, the conversion throws, and it never reaches the boundary at
all — `on_call_event_js_sync` being a stub is downstream of a
problem that happens before it. Traced: the JSON is built in function 11937, whose keys are `event_type`,
`call_id`, `result` and `previous_state`, and `operator[]` is function 100. The
throw is on the **first** access —

```rust
l10 = f11892(frame + 48832);          // the event's json
f100(8266, l10, "event_type");        // l10["event_type"]  <- throws here
```

— so the value is already a number before any field is read, and the conversion
dies before touching `previous_state` or the rest. **This is fixed, and it was a symptom rather than a bug of its own.** With the
thread-status profiler neutralised the conversion no longer throws: a run
delivers four events cleanly — `Call state changed`, `Call offer send failed`,
`Field stats ready`, `Call is ending` — and `VoipEvent.cpp:88` does not appear.
The dying workers were the cause; the JSON was fine.

Two things above did not survive either. The function that builds the JSON is
not 11937 — `unwasm` names 11937 `rk_optimizer_check_best_transport`, which is
transport selection, so that index came from somewhere else. And the empty
`getVoipParam("options.*")` and *"Application settings not loaded"* were never
the upstream: both are still there in a run that emits every event.

**Determinism is load-dependent, and the earlier claim needs that caveat.**
Registering the main thread gave 167 engine-log lines on every run of an idle
machine, which read as determinism. Under load — a compiler and another engine
running alongside — three runs in four die at 26-27 lines again. So registration
removes a large source of variance without removing the variance, and any
measurement here still wants several runs and a note on what else was running.

**And the stubs are not the problem.** `Runtime::stubs_called()` reports what the
guest actually called, and over a full run that is one thing:
`env::emscripten_check_blocking_allowed`, once. Invented answers barely touch
this run.

Both were printed by the example every time. Read the whole output before
forming a hypothesis — several of the hypotheses above would never have been
written.

### The 347 KB fill is not it — measured

`wa_call_group_create_participant` does zero 347 KB, and again 287 KB past that:

```rust
memory.fill(l6, 0, 347664);
memory.fill(l6 + 59400, 0, 287864);
```

Against a 64 KiB main-thread stack that looked decisive. It is not. Patched to
store its own destination instead of filling, `l6` reads **`0x960018`** on two
runs of three — a heap address, and one that has turned up before as a
participant object. The fill covers `0x960018..0x9b4f28`; the array at
`0x24bed0` is nowhere near it. This is an object being initialised in the heap,
which is what it looks like.

The third run read back the scratch word's pre-existing garbage while a control
storing `-1` at the same site fired, so the site is reached and the store works —
the site simply is not reached on every run.

`unwasm`'s watchpoint reports the same function writing at `0x24bed0` via a Fill
from a worker thread, and the two do not agree. The file offset it quotes,
4667241, holds `53 22 00 04 40 20 01 10` rather than a fill. Both wanted checking
before anything was built on them, and this is that check.

### Confirmed, and it does not matter: every guest thread starts on the same stack

```
thread 1 stack pointer 0x24cf60
thread 2 stack pointer 0x24cf60
thread 3 stack pointer 0x24cf60
thread 4 stack pointer 0x24cf60
thread 5 stack pointer 0x24cf60
```

`0x24cf60` is the module's initial value, which is also the main thread's. Read
again after the export hazard above came to light — patched capture, and the
absent case reported as "unavailable" rather than falling back to zero — it is
the same five values.
`_emscripten_thread_init` does not relocate it in this build, whatever its
documentation says — and the comment in `threads.rs` asserting that it does is
load-bearing, since it is the reason nothing sets one.

The participant array sits at `0x24bed0`, 4240 bytes below that shared top.

**Reading a global needs a patched module, and mistaking that for a zero is
easy.** The capture in `docs/captured-js/wasm` exports **no globals at all** —
`wasm-tools` finds zero. `cargo xt oracle export-globals` writes a copy that does, and
the probe was built for that copy; run against the original, `get_export
("__global_0")` returns nothing, and code that falls back to `0` reports a zero
where it should report "not available". Anything below that reads a global was
measured on the patched copy, and the stack pointer is global 0.

`pthread_self` is `global.get 3`, so the pthread pointer is global 3 — worth
knowing, because reading `__global_3` on the original returns nothing and looks
like a null pthread.

### The obvious fix does not work — and neither does the working one

Allocating 512 KiB per thread with the guest's own `malloc` and writing global 0
does give each thread a distinct region, far from the main thread's:

```
thread 1 stack 0x849088..0x8c9080     thread 4 stack 0xfa46b0..0x10246b0
thread 2 stack 0x940030..0x9c0030     thread 5 stack 0x10246b8..0x10a46b0
thread 3 stack 0x9c0038..0xa40030
```

And it breaks the run: four attempts, all 24 log lines and 11-12 traps, the
dead-run signature. Tried both before and after `__emscripten_thread_init`, with
no difference, so it is not an ordering problem. Reverted.

Going through emscripten's own entry points does not help either. This module
exports `emscripten_stack_set_limits`, `stackRestore`, `stackSave`,
`emscripten_stack_get_base/end/current/free` and `emscripten_stack_init`, so the
bounds can be set without touching the pthread struct at all — and calling
`set_limits(top, base)` followed by `stackRestore(top)` fails exactly the same
way. Four variants were tried: writing global 0 or going through those exports,
each before and after `__emscripten_thread_init`. All four give 24 lines and
11-12 traps.

**What isolates the mistake:** running the allocation *without* moving the stack
gives 202 lines, 2 × 70008 and zero traps across three runs. So `malloc` on a
thread's instance is harmless; **the relocation is what breaks it**.

Which says the approach was wrong rather than the mechanics. In emscripten's
model the guest's own `pthread_create` has already allocated this thread's
stack and recorded it in the pthread struct — that is why `establishStackSpace`
*reads* it rather than allocating. A freshly malloc'd region is a second stack
the guest knows nothing about, while its TLS and canaries still refer to the
first.

### The `+52/+56` offsets do hold here, and it still does not help

Dumping each worker's `struct pthread`:

```
thread 1 pthread 0x820030: +52=0x832350 +56=0x10000
thread 2 pthread 0x880030: +52=0x892350 +56=0x10000
thread 3 pthread 0x892370: +52=0x8a4690 +56=0x10000
thread 4 pthread 0xe00030: +52=0xe12350 +56=0x10000
thread 5 pthread 0xe12370: +52=0xe24690 +56=0x10000
```

A distinct top per thread and a size of 64 KiB — so the guest's own
`pthread_create` did allocate a stack, and the note claiming those offsets do not
hold for this module was wrong. `_emscripten_thread_init` does not install it:
it calls a four-line function that sets the TLS globals and nothing else.
Establishing the stack is the host's job, and this host does not do it.

**But those words are not there when a worker starts.** Read at the top of the
thread, before `__emscripten_thread_init`, `+52` and `+56` are zero; the values
above are read after it. Nothing in that function writes them — it sets four TLS
globals and returns — so the guest's own `pthread_create` fills them, on the
creating thread, while the new one is already running.

Two things follow. The variants that installed a stack *before* thread init were
not testing ordering: they read zeros and skipped. And a worker here can evidently
begin against a half-initialised pthread, which the baseline never has cause to
notice.

**And installing it correctly still fails.** Five variants, all 24 lines and
9-12 traps:

1. write global 0 with a malloc'd stack, before thread init
2. the same, after
3. `emscripten_stack_set_limits` + `stackRestore` with a malloc'd stack, before
4. the same, after
5. `set_limits` + `stackRestore` with **the guest's own stack from +52/+56**
6. the same, plus passing the real size (`0x10000`, from `+56`) as thread init's
   fifth argument instead of zero
7. the mirror image — moving the **main thread** to a private 4 MiB region right
   after `run_ctors`, while its stack is still shallow. The move itself succeeds
   (`Ok((0x64ecf0, 0xa4ecf0))`) and the run dies the same way

So it is not *which* stack. Relocating either side breaks this harness, which
means something in how the module is driven is incompatible with moving the
stack pointer after instantiation. Do not spend more attempts on that direction
without a new hypothesis.

**The sharpest statement, and it is a strange one.** `stackRestore` is three
instructions — `local.get 0; global.set 0`, nothing else. Calling it with the
value already in the global is **healthy**: 202 lines, no traps, three runs.
Calling it with any other value is fatal. So it is not the call, not the region,
not the bounds, not the ordering, and not TLS at the top of the stack — a
worker's stack pointer simply cannot leave the module's initial value without
the thread dying. Ten variants say so.

Aliasing is ruled out too, by the cleanest region available: growing the shared
memory and using pages nothing has ever touched, which have no other claimant at
all. Same failure. So it is not what lives at the address — **the value itself
cannot change**.

That is the question to answer before anything else here: **why must a worker's
stack pointer keep its initial value?** Eleven variants say it must, something
the workers depend on is evidently tied to it, and nothing measured so far says
what. A reasonable next suspicion is that these workers are not really executing
against their own instance's globals the way this harness assumes.

An independent implementation disagrees with this one on a structural point.
`unwasm`'s threading model — instances over one shared memory, each with its own
globals — states that **the memory and the table are shared**. `threads.rs` here
states the opposite: each instance builds its own table from the element
segments, on the grounds that they all initialise identically. That holds for
static function pointers and stops holding the moment anything is registered at
runtime.

**In this module nothing ever is.** `wasm-tools print` finds zero `table.grow`,
`table.set`, `table.fill` and `table.copy`, and the table is declared
`(table 9291 9291 funcref)` — minimum equal to maximum, so it cannot grow, and
defined rather than imported. Independently, `unwasm` models none of those
opcodes and refuses by name any it cannot model, yet decompiles all 13347
functions here without complaint. Measurement agrees: 9291 entries at every
thread's start and at the main thread's end, 9290 slots filled after the
constructors and 9290 at the end.

**How the opposite claim got its evidence is the part worth keeping.** The counts
that suggested it — "16 `table.grow`, 4185 `table.set`" — came from searching the
binary for those *byte values*. A `0x26` inside an immediate, a data segment or a
function index matches just as well. Counting bytes is not counting instructions:
use `wasm-tools print`, or a decoder that walks the code section, never a search
over the whole file.

Testing it means giving the workers the main thread's table, which wasmtime does
not make easy: a `Func` belongs to its store, so entries cannot simply be copied
across.

**The earlier discriminator, still worth keeping.**
Running `emscripten_stack_init` on a worker's instance and then
`emscripten_stack_set_limits(top, end)` with the pthread's own `+52/+56`, but
*not* `stackRestore`, gives **202 lines and 0-2 traps** — healthy. Adding the
`stackRestore` gives 24 lines and 11 traps. So the bounds are accepted and the
region is right; **it is assigning the stack pointer that breaks**, and ordering
does not save it: installing the whole thing before `__emscripten_thread_init`
and `_emscripten_tls_init` fails identically.

Two more facts from the same round. A worker instance never runs
`emscripten_stack_init`, so its bounds globals read **0/0** — nothing is checked
there by default. And running it aims them at `0x14cf60..0x24cf60`, the module's
static 64 KiB stack, which is the main thread's; that 64 KiB also matches the
`0x10000` at `+56`, which confirms those offsets really are {top, size}.

One tempting hypothesis is already eliminated: that the stack pointer is an
*imported* global and therefore shared between instances, which would explain
both the identical readings and why writing it pulls the stack out from under a
running thread. It is not. The module's 228 imports are 227 functions and one
memory; all fifteen globals are defined in the module, so they are genuinely
per-instance.

Its sibling is eliminated too: that `emscripten_stack_set_limits` keeps the
bounds in linear memory, which *is* shared, so setting them from any thread
would clobber every other thread's. It does not — the whole function is
`g8 = base; g7 = end`, two more per-instance globals. So a worker setting its own
limits cannot be reaching the main thread that way.

With the allocation but no relocation: 202 lines, zero traps, three runs. So the
relocation is what breaks it, whichever stack it installs.

**What the traps actually say, which none of this had looked at:**
`wasm trap: unaligned atomic` — every worker, every time, at its entry point,
with `emscripten_stack_set_limits` + `stackRestore` pointed at the pthread's own
`{high, size}`. In the healthy baseline no worker ends at all. Raising the fuel
bound a hundredfold changes nothing, so it is not exhaustion either.

The trap is in function 12302, the thread profiler's sampler, which
`_emscripten_thread_init` calls unconditionally at the end through `f12303(1)`.
It reads `*(pthread + 112)` atomically, and that field is allocated by
`_emscripten_thread_profiler_init` — which runs only when thread init's sixth
argument is set. Passing zero there turns the consumer on and leaves the producer
off, and the field is **measured as 0 on every worker**.

Passing 1 instead does not fix it: the field becomes allocated and 4-byte
aligned — measured, `0x64fa78` and friends — and the trap is unchanged.

Zeroing the profiler's enable byte at 1358168 before the worker runs does not fix
it either, and that one is contradictory: with the flag clear, function 12302
returns at its first guard and cannot reach an atomic, yet it is still where all
five workers trap. Either the write does not take or something sets the flag
again between there and the entry point.

**The instruction is isolated, and it sharpens the contradiction rather than
settling it.** Wasmtime's attribution is right — 12302's body is
`0x7f3ccb..0x7f3d45` and the backtrace's `0x7f3cea` falls inside it — and the
body decodes to:

```
i32.const 1358168 ; i32.load8_u ; i32.eqz ; br_if 0    <- leaves if the flag is clear
global.get 3 ; local.tee 2 ; i32.eqz ; br_if 0         <- leaves if the pthread is null
local.get 2
i32.atomic.load align=2 offset=112                     <- *(pthread + 112)
i32.atomic.load align=2 offset=0                       <- 0x7f3cea, the trap
```

`align=2` means four-byte alignment. And in all three configurations measured the
value it loads through should be fine: `0` with the sixth argument clear (address
zero is aligned and in bounds), `0x64fa78` with it set, and with the flag zeroed
the code should not reach the atomic at all. It traps in every one.

Which says the field holds something else by the time the worker runs than it
held at initialisation. **Patching that instruction from `i32.atomic.load` to a
plain `i32.load` — same four bytes — says what**: the trap becomes *out of bounds
memory access*, five times, at the same address. So the pointer is not merely
unaligned, it is outside linear memory. Garbage.

Where the garbage comes from is still open, and the shared stack is not it.

Every worker really does start from the module's initial stack pointer, and that
really is the same address the main thread uses — that part was measured and
holds. What does not hold is the conclusion drawn from it. Give each worker
**its own 4 MiB stack** from the heap, four megabytes apart so no two can reach
each other, and read the pointer back **from inside the guest** with `stackSave`:

    thread 1 RELOC base=0x86a3d0  sp_agora=0xc6a3d0
    thread 2 RELOC base=0xcc0030  sp_agora=0x10c0030
    thread 3 RELOC base=0x10c0038 sp_agora=0x14c0030
    thread 4 RELOC base=0x15d2388 sp_agora=0x19d2380
    thread 6 RELOC base=0x1f346b0 sp_agora=0x23346b0
    thread 5 RELOC base=0x23346b8 sp_agora=0x27346b0

The relocation takes — each pointer is its own base plus four megabytes. And the
run is unchanged: 27 lines, 11 traps, the same two reasons at the same address.
So threads sharing one stack is a true statement about this host that does not
explain the failure, and the fourteen "failed stack fixes" below were failing for
a reason other than the one they were testing.

The appealing arithmetic was wrong too. Worker 1's pthread sits at `0x820030`
and the stack its struct names runs `0x822350..0x832350`, leaving the struct 8992
bytes below the stack's low end — which reads like a 64 KiB overrun writing over
its own pthread. Moving the stack four megabytes away changes nothing, so it is
not that either.

Two measurement traps cost several retracted conclusions here, both worth
knowing:

- **`outgoing_call` prints only log lines containing the word "thread"**
  (`examples/outgoing_call.rs`, the filter on `runtime.logs()`). A probe labelled
  `RELOC …` or `STEP …` runs and is recorded and never appears. Two experiments
  were read as "the code did not run" when the code ran fine.
- **The module exports no globals.** Only the separately patched capture from
  `cargo xt oracle export-globals` does. So `get_export("__global_0")` returns `None`
  against the normal capture, and a diagnostic written against it prints nothing
  — silence that reads as "nothing to report". `stackSave` answers the same
  question through an export that actually exists, and `threads.rs` now uses it.

### The thread-status profiler was the blocker

`f12302` is emscripten's `emscripten_conditional_set_current_thread_status`,
reached from the futex wait path every engine worker sits in. Its first act is
to read a byte at `0x14B958` and return if it is zero — the profiler flag. Its
only writer is `emscripten_thread_profiler_enable` (`f13134`), which `unwasm`
reports with **zero call sites and zero table slots**: nothing in this module
ever calls it. The flag reads zero before a worker enters its routine, and zero
again after the one worker that returns cleanly has finished.

Forcing that test to fail — the guard's `i32.load8_u` becomes `drop; i32.const
0`, same three bytes, `cargo xt oracle neutralize-thread-profiler` — changes the run
from **27 engine log lines and eleven traps to 167 lines and none**, three runs.

That is a proof by construction, and it settles two things at once. The patch
only has an effect if the flag was non-zero, so **static data at `0x14B958` is
corrupted at runtime**; that is no longer a hypothesis. And the same corruption
is what leaves an out-of-linear-memory pointer in `pthread + 112`, which is
where the traps land. One bug, two symptoms, and neither is the stack.

The patch is an instrument, not a fix. It masks a symptom so the engine can be
studied; anything measured against a patched capture has to say so.

### What the engine does once its workers survive

It gets all the way to the offer. It generates SSRCs per participant for
**audio, video and screen-share** streams, emits `EVENT: Call state changed`,
and then:

    core/call_ 001  wa_call_start_internal, make_and_cache_offer failed: 70008
    events/ev 0011  EVENT: Call offer send failed

So the blocker is no longer "the workers die". It is a nameable error out of
`f11198_make_and_cache_offer`, with the whole participant and stream setup
visible in the log ahead of it.

The engine's own log narrows where. The last two lines before the failure are
`create_p2p_transport start` and `wa_one_side_bwe_create: Creating one_side_bwe`,
and exactly one host import was stubbed for the whole run
(`emscripten_check_blocking_allowed`) — so this is not a missing import, it is
the engine refusing internally.

`make_and_cache_offer` sets `70008` at nine sites, and each one names its own
source line, because each is guarded by a `voip_assert` carrying `offer.cc` and
a line constant:

| site | asserts at | guard |
| --- | --- | --- |
| 1 | `offer.cc:430` | a byte flag on the call object |
| 2 | `offer.cc:463` | `f10539(call)` returned null |
| 3 | `offer.cc:485` | `f10530(call, …)` returned null |
| 4 | `wa_call_signaling_handler.cc:296` | key index `>= 6` |
| 5 | — | local certificate fingerprint missing |
| 6 | `offer.cc:767` | a count that must be `> 0` |
| 7 | `offer.cc:776` | a pointer that must be non-null |
| 8 | `offer.cc:784` | the same pointer, again |
| 9 | `offer.cc:789` | falls through |

Site 5 is already excluded: it logs *"Local certificate fingerprint needs to be
generated by this point"* through `f8410` rather than asserting, and that line
never appears.

**Site 2 is excluded too, and it was the one this repository named as the
blocker.** `offer.cc:463` fires when `wa_call_group_get_self_participant`
returns null. It does not return null. Once the workers survive, `getCallInfo`
answers — it could not before — and reports:

    participant_count: 2
    participants[0]  raw_jid 11223344556677@lid   is_self false   state 2
    participants[1]  raw_jid 99887766554433@lid   is_self true    state 7

So the self participant exists and the engine knows which one we are. Two
further checks agree: `wa_call_group_get_self_participant` is nothing but
`*(group + 592)`, and both functions that fill that field —
`call_create_participants_for_1_to_1_call` and
`call_create_self_participant_for_phash_based_call` — log *"self participant not
created"* when they fail, which no run does.

The earlier conclusion came from a heuristic memory scan that picked a candidate
group pointer and read `group[592]` as zero. It picked the wrong pointer. A scan
that finds sixteen matches for the call id and then reasons about one of them is
a hypothesis, not a measurement, and it was recorded as the latter. The rest are silent because `voip_assert` needs two gates — the
byte at 1351084 *and* a callback slot at 1351212 that nothing ever fills — so
naming the site means making `f8502_voip_assert` observable, or re-running the
unique-site instrumentation on `offer.cc:485` now that the workers survive.

### The site is `offer.cc:485`

Naming it did not need the assert at all. Each site sets the same `70008`, and
`i32.const 70008` is four bytes (`41 f8 a2 04`) — so is `i32.const 70001`
(`41 f1 a2 04`), because the sleb128 encoding of anything in this range differs
only in its low seven bits. Give each site its own code and the engine prints
the answer itself. `cargo xt oracle tag-offer-error-sites` does that; no control flow
changes, nothing is written to memory, and no offset moves.

    wa_call_start_internal, make_and_cache_offer failed: 70003

70003 is the third site, and it really is `offer.cc:485` — the sixteen bytes
before its `i32.const 70008` are `i32.const 0`, `i32.const 765257`,
`i32.const 307169`, `i32.const 485`, `call 8502`, so the positional mapping from
byte order to source line holds:

    l11 = load32(l1, 0)                              // a participant entry
    l15 = wa_call_participant_jid_get_user_jid(l11)  // its *user* jid
    l21 = f10530(call, l15)
    if l21 == 0 { voip_assert(offer.cc, 485); return 70008 }

`f10530` is a lookup plus a filter: `get_participant(call, jid)`, then reject the
result if its state (`*(participant + 8)`) is one of the bits in the mask 5233 —
0, 4, 5, 6, 10, 12.

**It is the lookup, not the filter.** `i32.const 5233` (`41 f1 28`) and
`i32.const -8192` (`41 80 40`) are both three bytes, and -8192 has every bit
below 13 clear, so substituting it makes the filter accept every state the
`state <= 12` guard admits. The run still fails with 70003. (Consistent with
`getCallInfo`, which reports state 2 for the peer and 7 for us — neither in the
mask.)

So `get_participant` finds nothing. It walks the group's participants comparing
with `f10284`, which is a **textual** comparison — `pj_strcmp` on the `pj_str` at
offset +8 of each jid.

**None of it is reached: the participant pointer is null.**

`offer.cc:485` reads `l11 = *(l1 + 0)` — `l1` is `make_and_cache_offer`'s second
argument — and hands it to `get_user_jid`. Storing `l11 + 1` into scratch at that
site (the assert's sixteen bytes become `i32.const SCRATCH; local.get 11;
i32.const 1; i32.add; i32.store`, encoding the value so that 0 means "did not
run") reads back **1**. The store ran and `l11` is zero.

So the chain is: `get_user_jid(0)` asserts at `wa_call_participant_jid.cc:158`
and returns 0; `get_participant(call, 0)` hits its own null check, asserts at
`call_membership.cc:1382`, and returns 0; `f10530` returns 0; 485 fires. Nothing
is ever compared and no jid form is involved.

This also retracts an inference recorded here earlier. Patching `f10284` to
return 1 unconditionally makes 70003 disappear, and that was read as proof that
the search loop reaches the comparison. `f10284` has 58 call sites — it is a
blunt instrument, the run breaks elsewhere afterwards, and what it changed was
something upstream of this site, not the comparison at it.

### What `l1` is

Straight through, by static reading rather than by guessing, and the top of the
chain is the embind entry point itself:

    f1085_start_call_md(p0..p6)          <- startVoipCall's seven arguments
      builds an args struct, calls table slot 682
    call_manager_start_dual_call(args)   <- slot 682; no direct callers
      t5 = *(args + 0)
      wa_call_start_internal(…, p3 = t5, …)
        l3 = p3, never reassigned
        make_and_cache_offer(call, l3, …)
          l11 = *(l3 + 0)      <- measured null

`start_call_md` converts the peer (`p0`) and the `alt_jid` (`p4`) with the same
routine and tag — `f100(271, …)` — and the participant list (`p1`) with
`f100(680, …)`, so the two jid-shaped arguments go through one path and the list
through another.

**And `l3` is a stack address.** Probing it the same way — store `l3 + 1`, so
that 0 still means "did not run" — reads back `0x24bed0`. The initial stack
pointer is `0x24cf60` and `start_call_md`'s frame is 4176 bytes, so `l3` points
inside that frame.

That is also the address this file has been calling "the participant array
`make_and_cache_offer` reads", from much earlier and by a different route. The
two agree — and **the array is what it is**, not a participant jid built on the
stack. Reading `start_call_md`'s tail settles it, `oracle abi --index 1085
--body 700`:

    frame+268 .. +4124   memset 0, 3856 bytes    <- the call params blob
    frame+268            strncpy(call_id, <= 63)
    frame+264 = 1                                <- participant count
    frame+0 .. +256      memset 0, 256 bytes     <- 64 participant slots
    frame+0   = l8                               <- participants[0]
    frame+260 = frame                            <- params->participants
    f99(682, frame + 260)                        <- args = frame+260, not frame

So `args + 0` is `params->participants`, `args + 4` is the count, and `l3` is
the array — which is why it is a stack address and why `*(l3 + 0)` is
`participants[0]`. `wa_call_start_internal`'s own entry guard agrees: it demands
`1 <= arg4 <= 63`, so `arg3`/`arg4` are an array and a count, and `local 3` is
never reassigned in its 2,842 instructions.

**Which makes the null harder to explain, not easier.** Three static facts, each
read out of the bytes rather than inferred:

* `start_call_md` never builds the args struct when `create_participant_jid`
  returns null. The two instructions after the call are `local.get 8; br_if 6`
  and `br 8`, and `br 8` lands on the epilogue — it restores the stack and
  returns an uninitialised `l11`, so a failed participant jid produces no
  `make_and_cache_offer` and no `70008` at all.
* `wa_call_participant_jid_create_with_params` (`f10293`, table slot 679)
  **refuses** a null `params->user_jid`: the entry guard is
  `params && pool && out && *(params + 0)`, and failing it asserts
  `wa_call_participant_jid.cc:30` and returns `70004`. Its first act on success
  is `*(obj + 0) = *(params + 0)` — which is precisely what
  `get_user_jid` reads back.
* `create_participant_jid` checks that return and asserts
  `WaCallWebCallingBridge.cpp:101` if it is non-zero.

A participant jid that exists therefore *has* a user jid, and a participant jid
that does not exist never reaches the offer. Both cannot be true alongside
`participants[0] == 0` at `offer.cc:485`, so one of the two is measuring
something else — and the probe is the newer, less certain of the two.

The obvious suspect is that `participants[0]` is written correctly and then
overwritten: it lives at `0x24bed0`, on the main thread's stack, which every
guest thread also runs on. That is exactly the shape of "stored non-null, read
back zero" — but it does not survive the next section. Giving the workers their
own stacks does not make the offer path behave differently; it makes
`startVoipCall` trap earlier, on a heap pointer, deterministically. Whatever
zeroes this word has to be something that still happens when the workers are
nowhere near that region.

The probe only reports on runs that reach the site; the ones that stop earlier
read 0 and say "did not run", which is exactly what the `+ 1` encoding is for.
Repeated five times, three runs reached the site and all three read zero — so
the null is the steady state there, not a one-off, and everything above rests on
a measurement that repeats.

And the object in `args + 0` is built one call earlier, by the bridge itself:

    l8 = f111(681, pool, peer, list, out)   <- table slot 681
    memory.fill(frame, 0, 256)              <- the args struct, zeroed
    memory.store32(frame, 0, l8)            <- args + 0
    f99(682, frame + 260)                   <- call_manager_start_dual_call

Slot 681 is `f1083_create_participant_jid`, which `unwasm table` names and whose
own asserts give the source: `xplat/wa-voip/platforms/wasm/
WaCallWebCallingBridge.cpp`. It has exactly two guards, and both are named by
line:

- **line 32** — the memory pool argument is null.
- **line 33** — the peer string is empty **or** the device vector is empty
  (`*(list + 0) == *(list + 4)`, begin equal to end). Note the *or*: a non-empty
  peer with an empty list fails here too.

Past both, the construction goes:

    f96(675, out=frame+188, peer_string, frame+352)   -> f1084, the jid parser
    l1 = f99(458, frame+188)                          -> f839, moves it to the heap
    if l1 != 0 { f100(444, frame+340, l1 + 36) }
    else       { *(frame+348) = 0; *(frame+340) = 0 } <- the zeroing branch

and neither step is the failure:

- **`f1084` (slot 675) is the jid string parser.** It writes a 48-byte struct,
  zero-fills it when the string is empty, and compares against `"call"`,
  `"@call"` and **`"@lid"`** — so the LID form we pass is one it knows.
- **`f839` (slot 458) cannot return null.** It is a move: `malloc(48)`, copy the
  eight fields across, blank the source. Only an allocation failure returns
  nothing, so the zeroing branch below it is not the one being taken.

So the participant jid is built, from a parse that understands `@lid`, out of a
heap allocation that succeeded — and the field still reaches the offer as zero.

### A hypothesis that was labelled as one, and then refuted

The arithmetic fits, which is exactly why it is written here as a hypothesis.
`start_call_md` takes `frame = g0 - 4176` and `create_participant_jid` takes
`frame = g0 - 368`, so with the initial stack pointer at `0x24cf60`:

    0x24cf60   stack pointer at entry
    0x24bf10   start_call_md's frame
    0x24bda0   create_participant_jid's frame
    0x24bed0   l3, measured — inside that frame, at +304

If that holds, the pointer `args + 0` carries is a temporary in the frame of a
function that has already returned by the time `make_and_cache_offer` reads it,
and the zero is whatever later reused that stack.

**It is refuted, and by the cheapest possible test.** The assert at
`offer.cc:485` fires, so its sixteen bytes execute — enough to store the stack
pointer itself (`i32.const SCRATCH; global.get 0; i32.const 1; i32.add;
i32.store`). At the moment of the failure the stack pointer is **`0x24b8c0`**,
three runs, deterministic. The stack grows down, so everything above the pointer
is live frames — and `0x24bed0` is above it by 1552 bytes. The object is inside a
frame that is still on the stack, not in space that has been given back.

So it is a live object whose user-jid field is genuinely zero, and the
arithmetic above fitted a story that was not happening. That is twice in this
file that a frame-offset calculation has looked conclusive and been wrong; the
lesson is not to stop calculating but to keep labelling it until a control
agrees.

Note the probe technique has a limit worth knowing: a `voip_assert` call site is
only free real estate when the assert actually fires. `offer.cc:485` works
because the assert *is* the failure path. Replacing an assert that never fires —
the first one in `wa_call_start_internal`, for instance — probes nothing, and
reports "did not run" every time.

Worth noting for whoever picks this up: `wa_call_participant_jid_create_with_
params` is called by `10603` and by `10642_call_manager_start_dual_call_from_
context` — and **not** by `10428`, the entry point this path uses. The bridge's
own `create_participant_jid` is what builds it here.

And `*(participant_jid + 0)` is exactly what
`f10297_wa_call_participant_jid_get_user_jid` returns. So the participant-jid
object reaches the offer with **a null user jid**. Its device list is populated —
the engine generates SSRCs per device from it — so the object is not empty, it is
missing that one field. `wa_call_group_create_participant updating peer jid to:
6677:0@lid` in the log is plausibly the engine filling in from the devices what
the user jid did not supply.

Which embind argument should have filled it is still open, and four candidates
are already excluded — each tested, each still 70003:

| what was varied | variants |
| --- | --- |
| participant list | one device LID, two device LIDs, a bare LID |
| the fifth argument (`alt_jid`) | LID form, legacy `@c.us` form |
| the lookup's jid | user form, device form |
| `f10530`'s state filter | mask 5233, mask -8192 (accepts everything) |

The legacy form was worth testing because WhatsApp Web passes
`(g ?? h).toString({legacy: true})` there; it makes no difference here.

**And the first argument cannot be the legacy form in this build, even though
WhatsApp Web sends it that way.** `StackInterfaceWeb.js` is unambiguous:

    s.startVoipCall(e.toString({ legacy: !0 }), u, n, r, a, i, c);

so the peer goes in legacy form and only the participant list `u` is LID.
Passing `11223344556677@c.us` there gets the call rejected outright, in 30 log
lines: *"start_precall peer_participant_jids must be LID, enforce LID for all
calls"*. This capture enforces LID on the peer argument too, so the captured JS
and this wasm are not from the same configuration — worth knowing before reading
that file as ground truth for argument *forms*, as opposed to argument *order*,
which it did get right.

One real gap between what WhatsApp Web does at init and what we did is now
closed. Before `initVoipStack` — which takes the same three arguments we pass —
it runs `setABPropsOnWasm`, walking `WAWeb/Voip/ABPropConfig.js` and pushing 27
properties. We pushed none, and the engine said so: *"Application settings not
loaded"*.

`outgoing_call` now pushes the twelve boolean ones. The engine accepts all of
them, that complaint stops appearing, and the run goes from 167 log lines to
193 — so the properties do reach it and do change what it does.

**It is not the cause of 70003**, which is unchanged. And the integers are
deliberately left out: zero is a sane default for a feature flag and is not one
for `heartbeat_interval_s` or `default_endpoint_thread_poll_timeout`. Pushing
all 27 as zero registers fine and takes the run *down* to 93 lines. A real client
gets those values from the server.

What remains is which two strings differ. The engine's log says
`wa_call_group_create_participant updating peer jid to: 6677:0@lid` — the
*device* form — while the offer looks up by the *user* form.

**The input shape is not what causes it.** Three variants all produce the same
`updating peer jid to: 6677:0@lid` and the same 70003:

| participant list | result |
| --- | --- |
| one device LID (`…:0@lid`) | 70003 |
| two device LIDs (`…:0@lid`, `…:1@lid`) | 70003 |
| a bare LID (`…@lid`, no device) | 70003 |

So the engine derives the device form itself rather than taking it from what we
pass, and it does so whatever we pass.

**And the mismatch is not user-versus-device either.** The lookup's argument can
be changed without touching anything else: `l15 = get_user_jid(l11)` is a
three-byte `call` at `0x4dc35f`, and replacing it with three `nop`s leaves the
participant jid itself on the stack, so `l15 = l11` and the lookup runs against
the *device* form instead. Three runs, still 70003.

So `get_participant` matches neither form. Together with the earlier result —
the loop does reach the comparison — the group holds at least one live entry
whose jid string equals neither the user nor the device form of what the offer
is holding. Which two strings those actually are is still the open question, and
answering it needs somewhere to put them: see the note above on why an address
with no `i32.const` is not free.

Making `f8502_voip_assert` observable was tried and does not work yet. The idea
fits: the gate at the top of the function is eleven bytes
(`i32.const 1351084; i32.load8_u; i32.eqz; br_if 0`), which is exactly enough for
`i32.const SCRATCH; local.get 3; i32.store; br 0` — record the line argument and
leave the same way the disabled gate did. The signature is confirmed:
`f8502_voip_assert(p0, file, func, line)`, so `local.get 3` is the line.

It produced a contradiction that is not resolved:

- The store never lands. The scratch word still holds its initialised data
  (`0x37340034`) after a run, so `voip_assert` was seemingly never called before
  the host reads.
- Yet the run regresses from 167 engine log lines to 63, identically for two
  different scratch addresses — so the *control-flow* half of the patch does
  matter, which requires `voip_assert` to have been called.

Both cannot be true as stated, so one of the two measurements is wrong. Worth
keeping either way: finding a free scratch word took two wrong answers, and
neither is obvious.

- **A low address like 40 is not scratch, it is emscripten's.** On an unpatched
  capture that word already holds `0xFAE5C6B7` by the end of a call.
- **The assert's own callback slot at 1351212 reads as zero, which looks free**,
  but `f10347_update_voip_params_in_use2` also addresses it.
- `unwasm constants <module> <address>` does **not** answer "does anything touch
  this word". It finds `i32.const <address>`, and almost every struct field is
  reached as `base + offset`, which never appears as a constant. 1351220 has no
  `i32.const` anywhere and is still not free: storing to it takes a run from 167
  engine log lines to 126 and makes the offer failure stop being logged, and the
  stored value is gone by the time the host reads it. Both failed store
  experiments in this file were writing into memory the engine uses.

Finding a genuinely free word in this module is therefore still open. What
*would* answer it is the watchpoint from `unwasm --instrument-stores`, which
reports writes by whoever makes them rather than by how they compute the
address.

So moving the stack does not corrupt anything and does not run out of anything:
it puts some atomic on an address that is not aligned for it. The stack tops
involved are 16-byte aligned (`0x832350`, `0x8a4690`), so the misalignment is
downstream of the stack rather than in it — TLS or a lock whose address derives
from it are the obvious candidates.

**Do not re-run those five.** Diffing a 24-line log against a healthy one says
where it goes: the run dies **inside `initVoipStack`**, not on the call path. Its
last lines are media init —

```
wa_media_api.  init_audio_codecs = 0
wa_media_api.  init_media_endpt_and_codecs Exit
```

— so the switch kills the workers started during initialisation, long before an
offer is built. That is where to look.

One concrete lead. `_emscripten_thread_init` stores its fifth argument at
1268876 when both that and its third are non-zero, which makes it the default
stack size. This host calls it with `(thread_ptr, 0, 0, 1, 0, 0)`: both are zero,
so the global is never written. Emscripten's signature is `(pthread_ptr,
isMainBrowserThread, isMainRuntimeThread, canBlock, defaultStackSize,
startProfiling)`, so passing the real size — `0x10000`, from `+56` — and possibly
a non-zero third argument is worth measuring. `emscripten_stack_init` is exported
too, and may need calling on the thread's instance.

Exports worth knowing: `emscripten_stack_set_limits`, `..._get_base`,
`..._get_end`, `..._get_current`, `..._get_free`, `emscripten_stack_init`,
`stackSave`, `stackRestore`, `stackAlloc`, `pthread_self`.

### Giving each thread its own stack works, and is still not the fix

Those exports answer the question they were listed for. `emscripten_stack_get_
base` and `..._get_end` report **`0x24cf60` and `0x14cf60`** — the main thread's
stack is a 1 MiB region, and it is the region every guest thread starts from.

Running emscripten's `establishStackSpace` on each worker — reading `+52`/`+56`
out of the thread's own `struct pthread`, exactly as the web build does — works.
The workers land where the guest's `pthread_create` put them:

```
thread 1 stack 0x822350..0x832350 (65536 bytes)
thread 2 stack 0x882350..0x892350 (65536 bytes)
thread 3 stack 0x894690..0x8a4690 (65536 bytes)
```

**The offsets hold.** This file recorded "those offsets do not hold for this
module"; they do. What the earlier attempt hit is the race the spin-wait in
`threads.rs` now closes — the creating thread fills `+52`/`+56` *after* the new
thread is already running, so reading them at the top of the thread gives zeros,
and zero bounds are what put the workers on wild addresses.

**And it is still not an improvement, measured both ways.** It is not in
`threads.rs`, and the reason is this pair of results rather than a preference:

| | shared stack | own stack |
| --- | --- | --- |
| `profiler_flag.rs` — `startVoipCall` | traps in `f763` | returns `70004` |
| `profiler_flag.rs` — workers stopped | 1 | 0 |
| `profiler_flag.rs` — main SP afterwards | `0x241830` | `0x24cf60` |
| `signaling --ignored` | **23 passed** | **21 passed, 2 failed** |

The minimal probe says the change fixes something; the full suite says it breaks
two things. The tie-break is *what* breaks — and it is **the same trap in both
columns, only in a different run**:

    f1139   startVoipCall's embind wrapper
    f763    a container destructor
    f13513
    f13089  free, refusing the pointer it was handed

Shared stack, that is `profiler_flag.rs`. Own stacks, that is
`the_engine_starts_an_outgoing_call`, four attempts out of four. The second
failure is offer-then-call filling the log ring with 880 unstructured lines.

**So the heap corruption behind that trap is not the shared stack.** Moving the
stacks moves which run trips it, and nothing more. Anything built on "the
workers were writing over each other" has to survive that.

What the change does buy, and what a next attempt should keep: the workers get
64 KiB each where they had been borrowing 1 MiB. That is a 16× cut in headroom,
wasm has no guard page, and the engine's own frames are not small —
`start_call_md` alone takes 4,176 bytes plus a 3,856-byte `memory.fill`. It is
the first thing to account for before re-trying this.

### The host was writing the key material itself

**`env::get_random_bytes_js` takes `(len, buf)`. This host had it as
`(buf, len)`.** That one transposition is the whole of the ring corruption, the
`free`-refusing-a-pointer trap, and every "the host and the guest read different
memory" theory in the history of this file.

The module's only caller of it is the crypto callback in function-table slot 298
that `generate_raw_e2e_keys` (`wa_call_participant_crypto.cc`) dispatches
through, and its bytecode leaves nothing to interpret:

```
f649:   i32.const 32      ; the length — this callback rejects any other
        local.get 0       ; the destination
        call 8            ; env::get_random_bytes_js
```

Read with the arguments swapped, a request for 32 bytes at `0xf00000` becomes
**fifteen megabytes of the host's own PRNG written from address 32**. Every
symptom follows from that and nothing else is needed to explain any of them:

| symptom | what it was |
| --- | --- |
| high-entropy bytes that "look like key material" | they *are* key material: the host's PRNG, on the key-generation path |
| the whole image changed, one span, 83% zeroes down to 3% | one write covering almost all of memory |
| the ring destroyed with `getLogRingBufferOverflowCount` zero | the ring was simply inside the range |
| the guest's own `malloc` trapping afterwards | its heap was inside the range too |
| the 64 KiB `0xA5` guard block absent rather than moved | overwritten, like everything else |
| `free` refusing a pointer inside `startVoipCall` | same write, caught where it traps instead of where it reads |

**And it explains the exact discriminator**, which was the sharpest clue on the
table and was pointing the right way all along. `HostState::write` refuses an
out-of-bounds range, so the bogus write only lands when `32 + destination` still
fits inside linear memory. A round that grew to `0x10e0000` had room and was
destroyed; a round that stopped at `0xf10000` did not and survived untouched.
Two values, not a distribution, because it was not a correlation — the heap size
*decided* whether the write was refused.

Measured: **8 corrupt rounds of 8 became 8 clean rounds of 8**, and a round
reaching `0x10e0000` — previously an exact predictor of corruption — now
completes healthy with 61 structured log lines.

`settings_from_an_incoming_offer_do_not_unblock_an_outgoing_call` asserts
coherence now rather than tolerating its absence.

#### How it hid for so long

Worth recording, because every one of these was a reasonable-looking step in the
wrong direction:

* **The declared type does not disambiguate.** `(i32, i32)` is what the module
  says, and `getentropy(buf, len)` sits directly above it in `emscripten.rs`.
  This is not a standard emscripten import — it is WhatsApp's own — so the
  convention that made the guess feel safe never applied.
* **The host's randomness had been "excluded by measurement".** Twice. The first
  pass instrumented writes of 64 KiB or more and found only the probe's own
  guard fills — but that instrumentation was watching `HostState::write` call
  sites it knew about. The second pass counted `fill_random` calls through
  `calls_to`, which reads the argument-carrying trace, and that trace stops at
  8192 entries while a round makes fifty million host calls. It answered "never
  called" about a function that was called. The counts in `hot_calls` are
  unbounded and are what such a question needs.
* **`emscripten_stack_get_base` was treated as evidence.** Its body is
  `global.get 8; end` — a wasm global, which lives in the store, not in memory.
  It answers identically on a wiped module and a healthy one. Everything built
  on "the guest is fine, so the host must be looking elsewhere" was built on
  that.

Two real defects were found on the way and are documented below rather than
fixed, because neither turned out to cause this and both remain true: guest
threads run concurrently when the design says they must not, and they all start
from the same stack pointer.

### Guest threads run concurrently, on one stack

Found while chasing the corruption above, unrelated to it, and true regardless.

**Up to six guest threads execute at the same time.** `max_threads_in_wasm()`
counts it, bracketing every crossing of the host boundary in the store's
`call_hook`. It must be 1. It is 5 or 6 in every round, healthy and corrupt
alike.

`schedule.rs` cannot prevent that as written. A thread acquires the turn once,
around its whole routine, and `yield_point` hands it on only while `waiting > 0`
— that is, only while some other thread is blocked in its own first `acquire`.
Once every worker has forced its way past `TURN_TIMEOUT`, nothing is waiting
again, so nothing ever yields. The `func_wrap` gap makes it worse: a PJSIP
worker sitting in `pj_thread_sleep` reaches `emscripten_get_now` and nothing
else, and that import passes through neither `host_func` nor `yield_point`.

**And every guest thread starts from the same stack pointer.** It is a
per-instance wasm global, every instance is initialised from the same module,
and `__emscripten_thread_init` sets the TLS globals and nothing else. So each
thread begins at `0x24cf60`, the main thread's own region.

Neither is fixed, and the measurements say why:

* Serialising properly — holding the turn across exactly the guest-execution
  window, acquired and released in the `call_hook` — is correct and unusable. A
  two-minute round had not finished in ten, and cutting the turn timeout from
  five seconds to 25 ms did not help: under strict turns every crossing takes
  the scheduler lock, and a worker polling the clock crosses tens of millions of
  times per round. `Runtime::demand_strict_turns` exposes it for an
  investigator who wants attribution and can wait.
* Giving each worker its own stack makes things strictly worse, three times over
  — 64 KiB from the guest's own `pthread_create` traps `startVoipCall` four
  attempts of four, 4 MiB "changes nothing", and 1 MiB installed through the
  module's own exports gave 8 corrupt rounds of 8 against a baseline of one in
  four. That contradiction is still unexplained; it is now a curiosity rather
  than a lead, since the corruption it was competing to explain has a cause.

Do not write code whose safety argument is "the scheduler holds all but one
thread outside guest code". `HostState::read`'s SAFETY note is the one place
that still says it.

### The profiler flag is 5,640 bytes below the main stack

`cargo xt oracle neutralize-thread-profiler` says the corruption of `0x14B958` is
"still unexplained". Here is the geometry it was missing: **`emscripten_stack_
get_end` is `0x14cf60`**, and `0x14B958` is `0x1608` bytes below it. Static data
begins where the stack region ends, and the profiler flag is the first
interesting byte under it — so a stack running past its own low bound writes
exactly there, and seven threads sharing one 1 MiB region is not a rare way to
do that.

That is geometry, not proof, and `examples/profiler_flag.rs` is what would
carry it further: it reads the byte at instantiation, after the constructors,
after `initVoipStack` and after `startVoipCall`, with the engine log ring
attached at level 9 and the soft-assert gate open. **On the current host it
reads `0x00` at every one of those points, with no worker trapping.** So the
run that needs the patched capture is not this one, and the script's own
"27 lines and eleven traps" baseline is not reproducible here.

`examples/outgoing_call.rs` still ends with corrupted memory, and this is where
that lives now. It is a probe script written against a *patched* capture — it
reports "probe: did not run (unpatched capture?)" against the original — and it
enables machinery a real client does not. Both of its recent runs came back at
~54 engine-log lines against the ~200 a healthy run reaches, so by the health
rule further down neither is evidence about anything. What it does establish is
that something it does corrupts memory on its own, because `profiler_flag.rs`
reproduces its log level *and* its assert gate with none of that.
`startJsWorkerThread` and `initSctpRingBuffer` are what is left between them.

Two theories died getting here, both of them mine. The JID-shape mismatch is
gone — the strings are identical, and `pj_strcmp` reads its length as an i64 at
`+8` of the `pj_str_t`, which matches the dumps. And "the key is null" was first
measured by patching shared function bodies, which was unsound; it happens to be
true, and the sound measurement is the one above.

### The caller this file used to name, and why it is the wrong one

`grep "self.f11198_"` over the decompiled module returns **two** call sites: one
in `10534`, one in `10425`. An earlier draft of this section traced the first,
and everything it concluded — that `10534` builds a 64-pointer array at
`local2+80`, that a conditional write leaves element 0 null, that `10532`
returning null is the cause — describes **a path that does not run**.

Two things settle which one does. In a healthy baseline no
`call_create_participants_*` or `wa_call_invite_*` message appears at all; break
the run and `call_create_participants_for_1_to_1_call` shows up, and that string
lives inside `10425` (which carries both the `1_to_1` and `n_way_group`
variants and picks by branch). And the `if != 0` guarding `10425`'s call emits
the exact line the log shows. `10425` is `wa_call_start_internal`.

Two smaller corrections from the same detour, worth keeping because both cost
real time:

* In the `10534` path the array write is **not** conditional. `if (l4 == 0)
  break` skips only the `*(l4+80) = 1` that follows; the write to
  `*(l2+80+l6*4)` happens either way. That was a misread `br_if` depth.
* `5233` is not a validity mask. `arg3` is 2, measured, and the guard reads
  `(both non-null) & ((1 << arg3) & 5233) == 0 || arg3 > 12)` — being *outside*
  the mask is what lets execution continue.

`oracle abi <module> --index <n>` names functions by resolving the constants
they hand their logger. Trust the `__func__` argument of an assert over a
derived name: the decompiler's own naming called `10532`
`wa_vid_quality_manager_get_vid_rate_control`, from a string it references once
in a message about a *callee* failing, while its asserts say
`wa_call_group_create_participant` repeatedly.

| index | name | file |
| --- | --- | --- |
| 11198 | `make_and_cache_offer` | `messages/senders/offer.cc` |
| 10425 | `wa_call_start_internal` | `core/call_lifecycle.cc` |
| 10532 | `wa_call_group_create_participant` | `core/call_membership.cc` |

### What this replaces

The device-vs-user JID story that used to fill this section is **dead**. The
comparison it blamed never executes. Two things kept it alive longer than they
should have:

* `10284` is a *generic* JID comparator with **57 call sites**. Instrumenting it
  measures whichever call happened last, not the one on the offer path.
* The claim "patching 10284's offsets makes the 70008 disappear" cannot be
  reconciled with the loop never running. Unhealthy runs also produce *no*
  70008, and that failure mode had already burned us once. Re-verify anything
  resting on it against the health marker before reusing it.

### Instrumentation that works

Reading a guest value at a chosen point, length-preserving:

* Replace the call/compare with `i32.const <ADDR> ; local.get N ; i32.store ;
  i32.const 1 ; nop...`.
* **Address choice is the whole trick.** `0x900000` lands in the live heap and
  is overwritten; `0xF00000` and above make the store *trap* because memory has
  not grown that far, which kills the run early and looks like "never
  executed". **36** works — low static area, writable, survives.
* Encode `value + 1` when zero is a meaningful answer, so "stored 0" and "never
  stored" stay distinguishable. Otherwise pair every measurement with a control
  variant that stores a constant.
* To free bytes for a store, swap `if` (`04 40`) for `block` (`02 40`) — same
  two bytes, and the condition's bytes become yours.

Applied to 10535, with control:

| variant | `*(36)` | reading |
| --- | --- | --- |
| control, stores -1 | `0xffffffff` | the site does execute |
| `arg0` | `0x6d0018` | the context, non-null — matches `*(u32*)1352840` |
| `arg1` | 0 | the key is null |

Two decoding traps worth remembering: the guard in 10535 is `eqz ; eqz ; or ;
eqz ; if` — there is an **extra `45`**, so it reads "both non-null", and reading
the polarity backwards sends you to the wrong branch. And engine log lines
**do not print the line number** they are given, so the absence of a particular
line in the log proves nothing.

Signatures on the path: `11198` 10 params / 1 result · `10534` 1/1 · `10530` 2/1
· `10535` 2/1 · `10297` 1/1.

### Reading the call context

`*(u32*)1352840`. The engine reaches it the same way: `getCallInfo` registers
table slot 746 (function 1108), which calls function 10386 — seven instructions
that open `i32.const 1352840 / i32.load`.

Three things worth knowing before trusting a run:

* **A run is not repeatable, so never conclude from one.** Four runs of the same
  unpatched module through `outgoing_call`:

  | run | engine-log lines | `70008` | traps |
  | --- | --- | --- | --- |
  | 1 | 24 | **0** | 11 |
  | 2 | 202 | 2 | 0 |
  | 3 | 202 | 2 | 2 |
  | 4 | 201 | 2 | 3 |

  A healthy baseline reaches ~200 lines and reports the failure **twice**. The
  short run reports *no* `70008` — not because an offer was built, but because
  nothing got that far. **The health signal is how far the log got**; "the error
  disappeared" on its own reads a dead run as a fix, which is how a wrong
  conclusion survived several rounds here. Compare only runs of comparable
  length, and run each variant three or four times.
* A healthy run has `0x6d0018` there and structured data around `1352680`. A run
  showing `0xe0c70adc` and high-entropy data died before reaching the offer, and
  the 70008 never appears in it. Note the probe (`self_participant_probe`) traps
  inside `startVoipCall` on *every* run, baseline included, so it is a memory
  reader, not a progress meter.
* Whether the engine's log is real. After an incoming offer followed by an
  outgoing call it sometimes fills with random printable bytes instead of
  messages, which reads as "the call went quiet" when it means the opposite.

## What the watchpoint route would cost

`unwasm --instrument-stores` plus `memory.watch(addr, len)` answers "who wrote
this address" with a backtrace instead of a day of bisection, and it covers
`fill`/`copy`, which matters because `memset` is the usual answer to "who zeroed
it". That is the right tool for the cleared slot. The cost was measured rather
than guessed:

* `unwasm host` on this module emits **102 methods, 51 still to implement**:
  filesystem syscalls, the thread glue (`_emscripten_thread_mailbox_await`,
  `_emscripten_notify_mailbox_postmessage`, `emscripten_receive_on_main_thread_js`,
  `_emscripten_thread_set_strongref`), `emscripten_asm_const_int/double` — embedded
  JavaScript, deliberately left as `todo!()` — `_embind_register_class_constructor`,
  and this engine's own callbacks. This repository already implements all of
  them, so it is translation rather than invention.
* **The blocker is threads.** The decompiled Rust holds memory as a `Vec<u8>` in
  one instance and has no threading model, so `__pthread_create_js` has nowhere
  to put a second instance over shared memory — and this engine needs workers to
  initialise.

That first question is cheap to answer here, and the answer closes the route:
running this harness with `ThreadPolicy::PretendSuccess` — `pthread_create`
reporting success and starting nothing — gives **zero engine-log lines and a
trap, three times**. The engine does not initialise without real workers, so a
single-instance decompilation cannot reach `startVoipCall` either.

**The watchpoint needs a threading model to be usable on this module**: shared
memory with a second instance over it, which is what this repository's
`threads.rs` does and what the generated code has no shape for. That is the ask
if this route is worth opening.

## Read the module as source before disassembling anything

`unwasm` (`~/projects/unwasm`) decompiles this module into Rust:

```sh
unwasm decompile D5pLH9sfOOl.wasm -o generated.rs   # 2.4M lines, 13347 functions, ~1 min
grep -n "fn f10532" generated.rs                    # then awk to the next `pub(crate) fn f`
```

It annotates every `i32.const` with the string it addresses, which is what makes
the output readable — a bare `call 8502` says nothing, but
`f8502_voip_assert(0, "…/call_membership.cc", "wa_call_group_create_participant",
1665)` says everything. Both corrections in the section above came from reading
it; neither was visible in hours of hand-decoding, and one of them (a misread
`br_if` depth) had sent the whole investigation down a path that does not run.

Reach for it first. Disassembly is for confirming a specific byte you are about
to patch.

## The one tool to reach for first

```rust
runtime.call_embind("getCallInfo", &[])
```

It takes no arguments, reads the call context itself, and returns the engine's
entire view of the call as JSON — state, result, both participants, flags. It
is empty before a call and populated after, so it can also distinguish state
built by a call from state that was already there.

This matters because the module **exports no globals**, so the host cannot read
the call context directly; the context arrives in guest code through
`global.get 10`. `getCallInfo` is the whole of the available observability, and
reaching for it earlier would have replaced days of disassembly.

## Ruled out by measurement — do not retry these

Each was implemented or configured, measured, and reverted.

| Hypothesis | What happened |
| --- | --- |
| Per-instance function table diverging between threads | Table is `9291..9291`, non-growable, identical in every instance |
| `establishStackSpace` on the host (pthread `+52`/`+56`) | Much worse: workers went from 1 returning to all five stopping on wild addresses |
| Registering the main thread (`thread_id == 0`) | Drains stop failing, but `initVoipStack` starts trapping |
| Draining the proxy queue from inside `host_func` | Worse still — startup fails a round earlier |
| `can_block = 0` on workers | 7 of 12 threading tests fail; only the main thread cannot block |
| Turning the guest scheduler off | Reaching `Calling` went from 5/6 to 0/6 |
| Adding our own device LID to the participant list | Identical failure |
| Legacy-form JID as the fifth `startVoipCall` argument | Identical failure |
| Registering a video renderer | No such embind API exists; the video imports are never called |
| AB props for logging | No change in verbosity |
| Guest threads sharing the main stack | 4 MiB per worker, `stackSave`-confirmed: identical run |
| Memory accumulating across tests | No accumulation; one test reached 12 GB alone, through an unbounded log |
| A missing self participant (`offer.cc:463`) | `getCallInfo` reports `is_self: true` and `participant_count: 2` |
| A user-versus-device jid mismatch at the lookup | Lookup patched to use the device form: identical 70003 |
| `f10530`'s state filter rejecting the participant | Mask 5233 replaced by -8192, which accepts every state: identical 70003 |
| One device vs two devices vs a bare LID in the list | All three identical |
| Legacy form as the *first* `startVoipCall` argument | Rejected outright: "peer_participant_jids must be LID" |
| The 12 boolean AB props WhatsApp Web pushes | Accepted, "settings not loaded" stops, 167 -> 193 lines, same 70003 |
| The jid string parser (`f1084`) not knowing `@lid` | It compares against `"call"`, `"@call"` and `"@lid"` |
| `f839` returning null for the user jid | It is a move — `malloc(48)`, copy, blank — and cannot |

## The architectural gap

WhatsApp Web does not let the wasm engine gather ICE candidates. The browser's
WebRTC stack does that, and JavaScript hands the result back. From
`StackInterfaceWeb.js` and `HandleNativeCallEvent.js`:

1. the server sends the relay list, the **engine** raises it as an event, and JS
   caches it — with no cached list, `initP2PConnectionIfEnabled` gives up;
2. JS builds `stun:` URLs from it and calls
   `initP2PConnection(isCaller, iceServers, callback)`;
3. JS reads `getWebP2PVirtualIpv4` / `…Ipv6` / `…Port` from the engine and sets
   up the virtual addresses on its own side;
4. inbound P2P data enters the engine through
   `handleOnMessageFromHeap(ptr, len, callId, …)`;
5. **only once a real DataChannel opens** does JS call
   `notifyWebP2PChannelReady(true, false)`;
6. the resulting transport goes back via `sendWebP2PTransport(callId, …)`.

Calling `notifyWebP2PChannelReady` without a bridge behind it is a claim the
engine is right to reject — it answers `70004`. A bidirectional call here needs
that bridge emulated: a loopback data channel, `handleOnMessageFromHeap` fed,
and a relay list supplied.

## A sibling project that does place calls, and why its recipe does not transfer

`~/projects/meowmeow-node-wasm` drives a WhatsApp Web VoIP module from Node and
successfully places calls. Its module is not this one:

|  | that build | this build |
| --- | --- | --- |
| embind surface | 82 functions | 206 functions |
| `initVoipStack` | `(str, str, str, bool, u32, u32, u32, u32)` | `(str, str, str)` |
| `startVoipCall` | 8 arguments | 7 arguments |
| peer JID in its example | `…@s.whatsapp.net` | rejected — `peer_participant_jids must be LID` |

The extra `initVoipStack` arguments are configuration: `voipParamsVersion`,
`maxParticipants`, `maxGroupSize`. This build takes none of them, and rejects
phone-number peers outright. It is the newer of the two, and configuration has
moved out of initialisation — which lines up with what it says at runtime,
where `getVoipParam("options.*")` answers empty and the incoming path reports
`Application settings not loaded`.

So the shape of the fix is not "pass more arguments". Something has to supply
settings the way a server would. `the_call_entry_points_take_the_arguments_this_build_declares`
in `tests/voip_oracle.rs` pins the signatures so a capture update cannot quietly
invalidate this comparison.

Three things borrowed from that project and measured here, none of which
changes the outcome: its JID shapes (bare LID for self, PN for the user JID, no
device suffix), the peer's PN as the fifth `startVoipCall` argument, and its
readiness wait before placing a call. Its SCTP relay bridge remains the best
reference for emulating the data path.

## Reading the engine's state directly

Most of what is above was inferred from disassembly, and inference has a poor
record here. Two pieces of instrumentation now make parts of it measurable.

**Exported globals.** `cargo xt oracle export-globals` adds an export for every
global to a *copy* of a module, so `Runtime::global_i32` can read them:

```sh
cargo xt oracle export-globals caps/D5pLH9sfOOl.wasm .cache/patched/D5pLH9sfOOl.wasm 15
WA_WASM_DIR=.cache/patched cargo run --release --example self_participant_probe
```

Only the export section is rewritten; everything else is copied byte for byte.

What that measured, and it is worth knowing before reading `global.get N` as
anything: `global 0` is the stack pointer (the only one that moves), `7` and `8`
are the stack bounds, `10`–`14` are small constants — `global 10` is `0x18`, not
a pointer, despite `startVoipCall` opening with `global.get 10`. **No global
holds the call context.**

**Scanning for the context.** `examples/self_participant_probe.rs` searches
memory for the call id and walks back to a base whose `+659164` looks like a
heap pointer. Within a single run this is self-consistent and reports
`group[592] = 0` — the null the offer path rejects.

It is *not* validated, and the failure mode is instructive: a base that
reproduced across two runs still turned out to hold garbage on a third, because
two runs agreeing on an address only shows the allocator is deterministic enough
to put the call id in the same place. Scan per run; never hardcode a base. To
make it trustworthy, one of: derive the call id's offset from the object layout,
require several fields to agree with `getCallInfo` at the same moment, or find
where `wa_call_start_internal` gets its first argument.

## Working notes

- The engine's `file.cc:line` soft-asserts all pass through function 8502, which
  is gated on a byte at `1351084`. Setting it to 1 is *not* sufficient to make
  them appear, so their absence proves nothing about whether a path ran.
- `wa_call_group_create_participant`'s fourth argument is the participant state:
  **7 is self, 2 is peer** — matching the `state` field in `getCallInfo`.
- Static reading of this module has a poor record: eight hypotheses drawn from
  disassembly were refuted by measurement in a single session, twice because an
  unverified "and therefore this code runs" slipped into the chain. `callers`
  answers *who calls*, never *what executed*.
- When checking whether a fix worked, do not grep only for the old failure
  message. A change that merely renamed the failure once read as a success.

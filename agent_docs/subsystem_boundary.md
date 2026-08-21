# Subsystem Boundary

The core compiles conditionally for three optional subsystems and nobody had
measured what any of them cost. This is the rule that decides whether a
subsystem may stop being part of the core, the classification of every optional
subsystem against it, and the numbers behind both.

Read it before adding a feature gate, before adding a `Client` field only one
subsystem reads, and before proposing that a subsystem move out of the tree.

Anchors here are files and symbols, never line numbers: a `file:line` citation
in a document nobody recompiles is wrong within a week.

## Where the counts come from

```sh
grep -rc 'cfg(feature = "<name>")' --include='*.rs' src
```

Counted at `ff4ac10`, production sites only, since a gate inside a `mod tests`
block is scaffolding rather than coupling: `voip-runtime` 171, `plugins` 87,
`client-lifecycle` 56.

The same VoIP subsystem is 47 gates for 46k lines in `wacore`, where `voip` is
one gated `mod` and everything under it is unconditional. The difference is not
the subsystem, it is whether the subsystem owns its own files.

## The cut rule

A subsystem is **cuttable** when all four tests pass.

1. **Reach.** The core enters it on a dispatch key the core already routes on (a
   stanza tag, a notification type, an IQ namespace), or not at all. A core
   function that runs subsystem statements inline fails this.
2. **State.** Its per-client state is read only by itself.
3. **Return.** Everything it needs from the core is already `pub` or
   `pub(crate)` for some other caller.
4. **Contract.** Nothing it owns changes the *construction surface* of a public
   type with the feature: no `cfg` on a field, a builder setter, or a variant.
   `Event` is exempt from removal but not from mutation: `EventKind`
   discriminants are `EventInterest` bit indices consumers persist, so a cut
   subsystem keeps its variants and payload types compiled unconditionally.

   The test is deliberately narrower than "no gated API at all", because that
   is not achievable: `IncomingCall::media` is reached through an accessor that
   is itself gated, so the type does still have two shapes. What the accessor
   buys is that the two shapes differ only in a method. Code that builds,
   matches or destructures the payload compiles the same with the feature on or
   off, and only code that asks for the optional half stops compiling. A gated
   field takes that choice away from every consumer, which is why it fails and
   a gated method does not.

Verdicts:

- **Cuttable.** All four pass. The core may name it in exactly two places: its
  `mod` declaration and its entry in the `subsystems!` list
  (`src/client/subsystem.rs`). `tests/subsystem_boundary.rs` fails on a third.
- **Coupled.** Fails 1 or 2. It can be *disciplined* (interleaved statements
  hoisted into files it owns, one call per seam) but not cut, because the seam
  it needs does not exist yet.
- **Structural.** It is a core seam or a platform adapter slot, not a passenger.
  Its gate count is inherent.
- **Cross-cutting.** Instrumentation, gated at the point being instrumented by
  definition.

The rule deliberately does not say "a subsystem with its own directory can
leave": `src/voip/` has one and is not cuttable. Nor "a big subsystem should
leave": `src/message` is the largest thing in the crate and is the hot path, not
a subsystem.

## Inventory

### Cuttable

| subsystem | why | status |
| --- | --- | --- |
| `passkey` (`src/passkey/`) | claims two notification types and nothing else; state is its own; needs only `persistence_manager`, the event bus and `query`; owns `Event::PairPasskey*` with no gated field | cut, behind the `passkey` feature |

### Coupled

| subsystem | the edge that fails | test |
| --- | --- | --- |
| `voip-runtime` | `would_emit_pkmsg` (`src/client/sessions.rs`), `register_ack_waiter` (`src/client/messaging.rs`) and `should_issue_tc_token` (`src/send/tctoken_lifecycle.rs`) exist only for it | 3 |
| `pdo` (`src/pdo.rs`) | driven from the retry pipeline, and `pdo_requested` is the memo that keeps retry idempotent | 1, 2 |
| `pair_code` (`src/pair_code.rs`) | `pair-success` takes this subsystem's lock on the shared pairing path, QR included, so that a pair-code flow being retired cannot re-mint the ADV secret between verification and completion (`src/pair.rs`). Cutting it would either drop that interlock or leave the core reaching into an optional subsystem | 2 |
| `features/groups`, `features/newsletter`, `features/business`, `features/mex` | outbound IQ in `src/features/`, inbound handling in `src/handlers/notification/`, so neither half owns the subsystem; `group_cache` is also read from `src/voip/facade.rs` | 1, 2 |

### `voip-runtime` is a runtime, not the subsystem

Asked often enough to write down: the runtime-free VoIP core already exists, and
it is not what `voip-runtime` gates.

`wacore::voip` is sans-IO. Its feature is `["dep:aes-gcm", "dep:zerocopy"]`,
`tokio` appears only under wacore's `[dev-dependencies]`, and the four mentions
of tokio or webrtc under `wacore/src/voip/` are doc comments describing what the
native side injects. The executor is the `wacore::runtime::Runtime` trait, with
a `Send` native shape and a non-`Send` wasm one; the socket is the
`RelayTransport` seam. CI builds it for `wasm32-unknown-unknown` with
`--no-default-features --features "voip,js"` on every PR, which is what keeps
that true.

What `voip-runtime` gates in this crate is the native media plane: the webrtc-rs
DTLS/SCTP DataChannel, the libopus FFI and the task orchestration around them.
That is runtime-bound by construction, and `src/voip/mod.rs` has a
`compile_error!` pointing wasm32 and espidf builds at `wacore/voip` instead.
Making it runtime-agnostic would not be a refactor of this code, it would be
replacing webrtc-rs, and there is no consumer waiting for it: the one a split
was supposed to free is already served by `wacore::voip`.

`voip-runtime` is one test away from cuttable, and the three sites that keep it
coupled are all the same shape: a `pub(crate)` helper whose only caller is VoIP.
Moving them under `src/voip/` would pass test 3 by separating each from the
Signal-session, response-waiter and tc-token code it belongs with. Worse code
for a better number, so they stay, and this row is the record of that choice.

### Not a subsystem: WAM

Asked and answered rather than left for the next reader, because telemetry looks
like a passenger and is not one.

| test | WAM |
| --- | --- |
| 1. Reach | **Fails.** A subsystem is entered on a dispatch key the core already routes on. WAM claims no stanza tag, no notification type and no IQ namespace on the way in; what it wants is to watch work the core does for its own reasons, which is the shape this document calls coupled. |
| 2. State | Passes. Its buffers, queue and counters are read only by itself. |
| 3. Return | Passes. It needs the event bus and one IQ request, both already public. |
| 4. Contract | Passes. It owns no `Event` variant and no field of a public type. |

Failing test 1 leaves two options: a fifth `Subsystem` hook, or somewhere else.
The hook needs two askers and a measured floor, and WAM is one asker, so it goes
where a watcher belongs, on the plugin host's `events.core.observe`. The core
gains nothing: production code under `src/` and `wacore/` is untouched by that
batch apart from the files whatspec regenerates, and one pinned literal in a
`mod tests` that moved with the spec bump.

What the capability surface does not have, recorded because the next watcher will
want the same things:

- **An outbound observation point with the send's semantics.** `Event::SentFrame`
  hands over the marshaled bytes of a stanza after the write, which is enough to
  replay a stanza and not enough to say what kind of message it was, how many
  devices it was encrypted for, or how long each stage took. Eighteen WAM events
  are blocked on that alone.
- **A storage handle.** WAM wants a sequence number and undelivered buffers to
  survive a restart. There is no storage capability, and this is not an argument
  for adding one: a capability is a promise about every plugin. The plugin
  defines its own trait and ships an in-memory default.
- **A way to see what the core counts.** Two WAM events describe things
  `wacore::telemetry` already measures, whose doc comments name those very WAM
  ids, and a plugin cannot reach a counter, only the event bus.
- **A per-stanza inbound seam that is not the whole firehose.** A metric about
  receipt stanzas has to be derived from `Event::RawNode`, because the typed
  `Event::Receipt` fans out per peer on the aggregated shape and is skipped
  entirely on the retry path. `RawNode` gives the right unit and forwards every
  decoded stanza to get there, so a watcher that wants one tag pays for all of
  them.

`agent_docs/wam_telemetry.md` has the rest.

### Structural

`client-lifecycle` is the generation-scoped seam; `plugins` is the generic host,
and its gate count is the price of the seam existing. `sqlite-storage`,
`tokio-transport`, `tokio-runtime`, `ureq-client`, `signal` and `tokio-native`
are platform adapter selection. `voip-mlow`, `voip-libopus` and `voip-encoded` are codec profiles inside `voip`. `bench-harness`,
`debug-snapshots`, `legacy-session-interop` and `danger-skip-*` are build-time
switches.

### Cross-cutting

`tracing` and `metrics`. Their gates are not coupling.

### Not subsystems

`src/message`, `src/send` and the shared plumbing under `src/features` are the
hot path and the core's own work. They fail tests 1 and 2 by construction.

## The seam

A subsystem implements one trait and the core names it once:

```rust
pub(crate) trait Subsystem: 'static {
    type State: Default + MaybeSendSync;
    const NAME: &'static str;
    const NOTIFICATIONS: &'static [NotificationType] = &[];
    // handle_notification, on_connection_cleanup, on_response, memory
}

subsystems! {
    #[cfg(feature = "passkey")]
    passkey: crate::passkey::Passkey,
}
```

`Client` gains one field, `subsystems`, not one per subsystem. The list
generates a struct holding each attached subsystem's `State` under its real
type, so with none attached the struct has no fields and every generated loop
folds away.

The implementing type is the whole handle: its state, its claims and its hooks
hang off one `impl`, so they cannot drift apart the way a record of function
pointers lets them. Three things that were runtime questions are answered by the
compiler instead:

- **Reaching the state back.** `client.subsystem::<Voip>()` returns `&VoipState`
  directly. There is no `Any`, no downcast and no `Option`: the generated
  `Attached` trait is implemented only for a subsystem this build carries, so
  naming a detached one does not compile. That replaced an `expect` on one side
  and an unreachable `Err` arm on the other.
- **Two subsystems claiming one notification type.** Dispatch takes the first
  match, so a collision would make routing depend on list order. `CLAIMS` is a
  `const`, so a `const` assertion rejects the build.
- **A hook a subsystem does not fill.** It is a defaulted trait method, not a
  `None` in a table, so it costs no branch rather than a checked one.

The list is a macro rather than runtime registration because static
registration through a linker-section crate would trade the core's last gate for
a new dependency. The guard test is what keeps "one gate" enforceable instead.

### Adding a fifth hook

Four hooks is not a budget, it is what two subsystems happened to need. The
failure mode from here is obvious and slow: `pdo` wants a point that does not
exist, a defaulted method is the shortest path, and three batches later the
trait is a god object with eight defaults that no single subsystem fills. That
is the coupling this document is about, moved into one file rather than removed.

So a fifth hook has to clear two bars:

1. **Two askers.** Two subsystems want the same point, or the one that wants it
   gets its own seam instead. One subsystem's need is not a core extension
   point; it is that subsystem's problem.
2. **A measured floor.** The hook costs nothing in a build that does not fill
   it. That is a claim about codegen, so it is measured the way the rest of this
   document is, not asserted: build with and without and put the number here.

A hook that cannot clear both is a sign the subsystem is coupled (rule test 1),
and the honest answer is the inventory row, not the trait.

Besides state, a subsystem can fill three hooks: connection cleanup, a response
about to reach its waiter, and the collections it retains for
`Client::memory_report`. `memory` takes `&Self::State` rather than the client,
so reporting cannot quietly become a second way for a subsystem to read the
core.

The report does not undo that with strings. A subsystem exports its collections
as `SubsystemCollection` constants (`voip::collections`) and its hook names them
from those same constants, so a caller writes
`report.subsystem(voip::collections::ACTIVE_CALLS)` and a typo is a compile
error rather than a silent `None`.

The core's own match arms win: the seam is consulted only for a notification
type the core does not model itself, so a claim on a type the core later starts
handling would silently stop arriving.
`a_claimed_notification_type_is_not_shadowed_by_a_core_arm` fails when that happens.

## What a subsystem costs

Stripped `demo`, release profile, the build `binary_size_ci.md` gates on. Sizes
are deterministic for a pinned toolchain; the baseline reproduced byte for byte
across two runs.

Read the file size as a shipping proxy and nothing more. It is quantized by
section alignment, so two builds can land on the same byte count while their
codegen differs, and an unchanged number is therefore not evidence that a change
was free. `binary_size_ci.md` names the sensitive measures for that question,
`.text` and llvm-lines, and they are what a "this cost nothing" claim has to
rest on.

Two questions, so two tables. What this batch changed, each row measured on
`main` and on the branch from the same working tree so no other commit can drift
into the delta:

| build | main | branch | delta |
| --- | ---: | ---: | ---: |
| default | 10,806,752 | 10,756,792 | -48.8 KiB |
| default + `voip` | 11,373,952 | 11,319,160 | -53.5 KiB |

And what a subsystem costs to turn on, each row against the default build beside
it:

| build | bin size | vs default |
| --- | ---: | ---: |
| default | 10,756,792 | |
| default + `passkey` | 10,807,352 | +49.4 KiB |
| default + `plugins`, host on and no plugin installed | 10,960,960 | +150.6 KiB |

`passkey` has no before column: it used to be compiled in unconditionally, so
there was no build without it to compare against. The `plugins` figure is
measured on the pre-batch tree, which this batch does not touch.

Turning the smallest cuttable subsystem off is worth ~48 KiB. The `voip` row is
the one to read twice: moving five `Client` fields and their construction and
teardown branches onto the seam made the VoIP build 53 KiB *smaller*, so
attaching through it did not cost that subsystem anything.

Of that `voip` figure, 11.2 KiB came from making the seam static rather than
erased, measured on its own by building both designs back to back:

| build | erased seam | typed seam | delta |
| --- | ---: | ---: | ---: |
| default | 10,757,208 | 10,756,792 | -0.4 KiB |
| default + `voip` | 11,330,648 | 11,319,160 | -11.2 KiB |

An `Arc<dyn Any>` per subsystem, the vtables behind it, the boxed futures the
hook signatures forced and the scan that found the state again were all real
bytes. Storing each state under its own type spends none of them, which is why
the type-safe version is also the smaller one. The `plugins` row
is the enabled-with-no-plugin number `plugin_architecture.md`'s checklist asks
for and that nothing in the repo had produced.

The CPU half of that checklist item, `warm_group_send` from
`benches/client_group_send.rs`, fastest of 20 samples, microseconds per send:

```sh
cargo bench --bench client_group_send --features bench-harness -- warm_group_send
cargo bench --bench client_group_send --features bench-harness,plugins -- warm_group_send
```

| group size | host off | host on, no plugin installed |
| ---: | ---: | ---: |
| 8 | 62.90 | 62.36 |
| 32 | 63.08 | 61.49 |
| 128 | 61.80 | 61.70 |
| 512 | 63.66 | 63.82 |

Inside the noise floor of an ordinary machine, which is what a null check on
`Option<Arc<PluginHost>>` should read as: the host-on column is faster in three
rows of four, which is not a speedup, it is the spread.

Both columns have to come from one session on one machine, and only the two
columns may be compared. The absolute figures move with the box, so a number
here is not comparable to one from another run, and this table is worth nothing
as a record of whether the client got faster over time. Re-run it rather than
trust it. CodSpeed is the instrument for the across-time question, and the
reason this is a command here instead of a CI gate is that CodSpeed keys a
series by benchmark name and cannot hold two configurations of the same one.

## When to stop

Not at a gate count. `plugins` and `client-lifecycle` are supposed to have
theirs, and three of VoIP's are a deliberate choice recorded above. The chain of
batches is done when every subsystem in the inventory is either cut or carries a
written test-1/test-2/test-3 edge a maintainer decided to keep. What is left
after this batch:

- `pdo` and the `features/*` halves have never been examined beyond the row
  above; each needs its own reading before anyone moves it.
- `pair_code` needs a decision about the ADV-rotation interlock before it can be
  anything but coupled, and that is a protocol-correctness question, not a
  refactor.
- The plugin host's runtime cost with no plugin installed is measured above by
  hand rather than gated in CI, because a CodSpeed series is keyed by benchmark
  name and cannot hold two configurations of the same benchmark.

## What the guard proves, and what it does not

`tests/subsystem_boundary.rs` holds two guards, one per verdict.

**Cuttable.** The core may not name the subsystem outside the files it owns and
its two allowed mentions. It scans text, so it sees a mention in a comment too,
which is deliberate: a comment in the core explaining what a subsystem needs is
the same coupling one commit early. It also checks the *shape* of each allowed
line rather than only counting them, because a budget of three lines is
otherwise satisfied by spending one on a `pub use crate::<name>::Thing`, which
keeps the count and puts the subsystem back in the core's surface. An allowed
line has to be a feature gate, the `mod` declaration or the `subsystems!` entry,
and the gate arm requires the line to *end* as an attribute, or a one-line
`#[cfg(feature = "x")] pub use crate::x::Thing;` would open like a gate and pass.

**Coupled but disciplined.** A subsystem that cannot leave still has gates in the
core, so the guard caps how many. VoIP's is 9 outside the files it owns, and
raising it is meant to be a decision with a line in this document behind it. The
cap counts production and test gates together, because telling them apart needs
a parser the guard does not have and a new gate is worth a look either way.

It counts the `feature = "..."` term rather than a whole `cfg(feature = "...")`,
so a composite gate counts as well. That matters because the budget sits at
exactly the current count, so the next gate fails the test, and the cheapest way
out of that failure is to write the gate as `all(...)`. rustfmt already splits
such a gate across lines and leaves the term on one of its own, which the
narrower spelling would not have seen at all. Matching the term also counts
`not(feature = "...")`, which is still core code conditioned on the subsystem.
Without this, the 29-to-5 result above had nothing holding it: the next change
could spend it back one `Client` field at a time, and human review is exactly
what let the original 314 accumulate.

What neither reaches: test 3, the subsystem calling core internals that exist
only for it. Neither claims the disabled build carries zero bytes of the
subsystem either, since `Event` variants and payload types stay in `wacore` by
test 4. "Zero cost" here means zero code, state and branches of the subsystem's
own.

One narrow hole is left in the cuttable guard, recorded rather than fixed
because closing it costs more code than it saves: only lines that *contain* the
subsystem's name are examined, so an item gated by `#[cfg(feature = "passkey")]`
whose own line never says "passkey" is invisible to it. The gate line itself is
still counted against the budget, so this cannot be done silently at scale, but
one such item fits inside a budget that has room.

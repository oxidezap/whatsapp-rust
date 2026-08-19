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
4. **Contract.** Nothing it owns changes shape with the feature. `Event` is
   exempt from removal but not from mutation: `EventKind` discriminants are
   `EventInterest` bit indices consumers persist, so a cut subsystem keeps its
   variants and payload types compiled unconditionally. What fails this test is
   a payload *field* behind a `cfg`, because then one public type has two
   shapes.

Verdicts:

- **Cuttable.** All four pass. The core may name it in exactly two places: its
  `mod` declaration and its entry in `SUBSYSTEMS` (`src/client/subsystem.rs`).
  `tests/subsystem_boundary.rs` fails on a third.
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

`voip-runtime` is one test away from cuttable, and the three sites that keep it
coupled are all the same shape: a `pub(crate)` helper whose only caller is VoIP.
Moving them under `src/voip/` would pass test 3 by separating each from the
Signal-session, response-waiter and tc-token code it belongs with. Worse code
for a better number, so they stay, and this row is the record of that choice.

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

`src/client/subsystem.rs` holds one `const` table:

```rust
pub(crate) const SUBSYSTEMS: &[Subsystem] = &[
    #[cfg(feature = "passkey")]
    crate::passkey::SUBSYSTEM,
];
```

`Client` gains one field, `subsystems`, not one per subsystem. A subsystem parks
its per-client state there and lists the notification types it models. With none
attached the table is a zero-length slice, so every loop over it folds away.

The table is a `const` rather than runtime registration because static
registration through a linker-section crate would trade the core's last gate for
a new dependency. The guard test is what keeps "one gate" enforceable instead.

Besides state, a subsystem can fill three optional hooks: connection cleanup,
a response about to reach its waiter, and the collections it retains for
`Client::memory_report`. Each is a plain function pointer, so an unattached
build folds the table away rather than paying for an empty vtable. A subsystem
whose state is reached from many of its own call sites takes it with
`expect`, which is infallible by construction: the table entry and the callers
sit behind one gate.

The core's own match arms win: the table is consulted only for a notification
type the core does not model itself, so a claim on a type the core later starts
handling would silently stop arriving.
`a_claimed_notification_type_is_not_shadowed_by_a_core_arm` fails when that happens.

## What a subsystem costs

Stripped `demo`, release profile, the build `binary_size_ci.md` gates on. Sizes
are deterministic for a pinned toolchain; the baseline reproduced byte for byte
across two runs.

| build | before | after | delta |
| --- | ---: | ---: | ---: |
| default | 10,806,752 | 10,757,216 | -48.4 KiB |
| default + `passkey` | n/a, always compiled in | 10,810,016 | +51.8 KiB over default |
| default + `voip` | 11,373,952 | 11,330,656 | -42.3 KiB |
| default + `plugins`, host on and no plugin installed | 10,960,960 | | +150.6 KiB over default |

Turning the smallest cuttable subsystem off is worth ~48 KiB. The `voip` row is
the one to read twice: moving five `Client` fields and their construction and
teardown branches onto the table made the VoIP build 42 KiB *smaller*, so the
seam did not cost that subsystem anything to attach through. The `plugins` row
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
| 8 | 52.99 | 53.20 |
| 32 | 53.71 | 53.30 |
| 128 | 53.80 | 53.39 |
| 512 | 54.62 | 54.24 |

Inside the noise floor of an ordinary machine, which is what a null check on
`Option<Arc<PluginHost>>` should read as. Run it rather than trust it: CodSpeed
keys a series by benchmark name, so it cannot hold both configurations of the
same benchmark, which is why this is a command here and not a CI gate.

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
- The plugin host's runtime cost with no plugin installed is measured below by
  hand rather than gated in CI, because a CodSpeed series is keyed by benchmark
  name and cannot hold two configurations of the same benchmark.

## What the guard proves, and what it does not

`tests/subsystem_boundary.rs` fails when a cuttable subsystem is named outside
the files it owns and its two allowed core mentions. It scans text, so it sees a
mention in a comment too, which is deliberate: a comment in the core explaining
what a subsystem needs is the same coupling one commit early.

It does not reach test 3 (the subsystem calling core internals that exist only
for it), and it does not claim the disabled build carries zero bytes of the
subsystem: `Event` variants and payload types stay in `wacore` by test 4. "Zero
cost" here means zero code, state and branches of the subsystem's own.

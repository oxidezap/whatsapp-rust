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
| `voip-runtime` | `bind_pending_call_link_join_ack` runs inline in the ack path (`src/client/node_io.rs`) | 1 |
| | `call_registry` is read by `CallHandler` and by `memory_report` | 2 |
| | `would_emit_pkmsg` (`src/client/sessions.rs`) and `should_issue_tc_token` (`src/send/tctoken_lifecycle.rs`) exist only for it | 3 |
| | `IncomingCall::media` is a `cfg` field inside a public payload (`wacore/src/types/call.rs`) | 4 |
| `pdo` (`src/pdo.rs`) | driven from the retry pipeline, and `pdo_requested` is the memo that keeps retry idempotent | 1, 2 |
| `pair_code` (`src/pair_code.rs`) | `pair_code_state` is written by the companion-reg notification handler, by `src/pair.rs` and by connection cleanup | 2 |
| `features/groups`, `features/newsletter`, `features/business`, `features/mex` | outbound IQ in `src/features/`, inbound handling in `src/handlers/notification/`, so neither half owns the subsystem; `group_cache` is also read from `src/voip/facade.rs` | 1, 2 |

### Structural

`client-lifecycle` is the generation-scoped seam; `plugins` is the generic host,
and its gate count is the price of the seam existing. `sqlite-storage`,
`tokio-transport`, `tokio-runtime`, `ureq-client`, `signal` and `tokio-native`
are platform adapter selection. `voip-mlow`, `voip-libopus`, `voip-encoded` and
`mlow-fast-fft` are codec profiles inside `voip`. `bench-harness`,
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

The core's own match arms win: the table is consulted only for a notification
type the core does not model itself, so a claim on a type the core later starts
handling would silently stop arriving.
`a_claimed_notification_type_is_not_shadowed_by_a_core_arm` fails when that happens.

## What a subsystem costs

Stripped `demo`, release profile, the build `binary_size_ci.md` gates on. Sizes
are deterministic for a pinned toolchain; the baseline reproduced byte for byte
across two runs.

| build | bin size | vs default |
| --- | ---: | ---: |
| default, `passkey` compiled in unconditionally (the old shape) | 10,806,752 | |
| default, `passkey` off | 10,756,992 | -48.6 KiB |
| default + `passkey` | 10,809,824 | +51.6 KiB |
| default + `plugins`, host on and no plugin installed | 10,960,960 | +150.6 KiB |
| default + `voip` | 11,373,952 | +553.9 KiB |

Turning the smallest cuttable subsystem off is worth ~49 KiB, and the seam that
makes it cuttable costs 2.5 KiB of `.text` when the subsystem is on. The
`plugins` row is the enabled-with-no-plugin number `plugin_architecture.md`'s
checklist asks for and that nothing in the repo had produced.

## What the guard proves, and what it does not

`tests/subsystem_boundary.rs` fails when a cuttable subsystem is named outside
the files it owns and its two allowed core mentions. It scans text, so it sees a
mention in a comment too, which is deliberate: a comment in the core explaining
what a subsystem needs is the same coupling one commit early.

It does not reach test 3 (the subsystem calling core internals that exist only
for it), and it does not claim the disabled build carries zero bytes of the
subsystem: `Event` variants and payload types stay in `wacore` by test 4. "Zero
cost" here means zero code, state and branches of the subsystem's own.

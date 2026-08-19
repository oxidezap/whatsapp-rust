# Subsystem Boundary

The core carries 314 production `#[cfg(feature = ...)]` sites for three optional
subsystems, and until this document nobody had measured what any of them cost.
This is the rule that decides whether a subsystem may stop being part of the
core, the classification of every optional subsystem against it, and the numbers
the classification rests on.

Read it before adding a feature gate, before adding a `Client` field for
something only one subsystem reads, and before proposing that a subsystem move
out of the tree.

## The counts this starts from

Production sites only (a `cfg` inside a `mod tests` block is test scaffolding,
not core coupling), measured at `ff4ac10`:

| feature | prod `cfg` in `src/` | test `cfg` | `Client` fields |
| --- | ---: | ---: | ---: |
| `voip-runtime` | 171 | 121 | 5 |
| `plugins` | 87 | 3 | 1 |
| `client-lifecycle` | 56 | 16 | 2 |

The same subsystem is 47 `cfg` for 46k lines in `wacore`, where `voip` is one
gated `mod` declaration and every line below it is unconditional. The difference
is not the subsystem. It is whether the subsystem owns its own files.

## The cut rule

A subsystem is **cuttable** when all four tests pass. Any failure names the
specific edge that has to go first, so the verdict is checkable rather than a
matter of taste.

1. **Reach.** The core enters it on a dispatch key the core already routes on
   (a stanza tag, a notification type, an IQ namespace) or does not enter it at
   all. A core function that has to run subsystem statements in the middle of
   its own work fails this test.
2. **State.** Its per-client state is read only by itself. A core path that
   reads or mutates that state fails this test.
3. **Return.** Everything it needs from the core is already `pub` or
   `pub(crate)` for some other caller. A `pub(crate) fn` that exists only
   because this subsystem calls it fails this test.
4. **Contract.** Nothing it owns changes shape with the feature. `Event` and its
   payloads are exempt from removal but not from mutation: `EventKind`
   discriminants are `EventInterest` bit indices consumers persist
   (`wacore/src/types/events.rs:244`), so a cut subsystem keeps its variants and
   payload types compiled unconditionally. What fails this test is a payload
   *field* behind a `cfg`, because then one public type has two shapes depending
   on who compiled it.

Verdicts:

- **Cuttable.** All four pass. The core is allowed to name it in exactly two
  places: its `mod` declaration and its entry in `SUBSYSTEMS`
  (`src/client/subsystem.rs`). `tests/subsystem_boundary.rs` fails on a third.
- **Coupled.** Fails 1 or 2. It can be *disciplined* (its interleaved statements
  hoisted into files it owns, one call per seam) but not cut, because the seam
  it would need does not exist yet.
- **Structural.** It is not a subsystem sitting on the core, it *is* a core seam
  or a platform adapter slot. There is nothing to move; its `cfg` count is
  inherent.
- **Cross-cutting.** Instrumentation. Its `cfg` sites are at the points being
  instrumented by definition, so counting them as coupling is a category error.

### Why not the two obvious alternatives

The rule deliberately does not say "a subsystem with its own directory can
leave". `src/voip/` is its own directory and `voip-runtime` is not cuttable,
because it enters the core through the ack path (test 1) and the core reads
`call_registry` (test 2).

It also does not say "a big subsystem should leave". `src/message` is the
largest thing in the crate and is not a subsystem at all: it is the hot path.

## Inventory

Every optional subsystem of the `whatsapp-rust` crate, classified. `file:line`
is the specific edge the verdict rests on.

### Cuttable

| subsystem | evidence | note |
| --- | --- | --- |
| `passkey` (`src/passkey/`, 1,467 lines) | reach: two notification types only (`src/handlers/notification/mod.rs:77,80`). state: `passkey_state`, `passkey_opening` (`src/client.rs:1549,1554`), read nowhere else. return: uses `persistence_manager`, `event_bus`, `query` only. contract: owns `Event::PairPasskey{Request,Confirmation,Error}` with no gated field | the vertical slice of this batch |

### Coupled

| subsystem | the edge that fails | test |
| --- | --- | --- |
| `voip-runtime` | `bind_pending_call_link_join_ack` runs inside the ack fast path (`src/client/node_io.rs:519,1597,1621`) | 1 |
| | `call_registry` is read by the stanza handler and by `memory_report` (`src/handlers/call.rs:117`, `src/client/accessors.rs:473`) | 2 |
| | `would_emit_pkmsg` (`src/client/sessions.rs:962`) and `should_issue_tc_token` (`src/send/tctoken_lifecycle.rs:169`) exist only for it | 3 |
| | `IncomingCall.media` is a `cfg` field inside a public payload (`wacore/src/types/call.rs:329`) | 4 |
| `pdo` (`src/pdo.rs`) | driven from the retry pipeline; `pdo_requested` is the memo that keeps retry idempotent (`src/message/retry.rs:357`) | 1, 2 |
| `pair_code` (`src/pair_code.rs`) | `pair_code_state` is written by the notification handler (`src/handlers/notification/companion_reg.rs:62`), the pairing flow (`src/pair.rs:314`) and connection cleanup (`src/client/lifecycle.rs:1662`) | 2 |
| `features/groups`, `features/newsletter`, `features/business`, `features/mex` | outbound IQ lives in `src/features/`, inbound handling in `src/handlers/notification/groups.rs` and siblings, so neither half owns the subsystem; `group_cache` (`src/client.rs:1326`) is also read from `src/voip/facade.rs:4085` | 1, 2 |

### Structural

| subsystem | why it never cuts |
| --- | --- |
| `client-lifecycle` (56 `cfg`) | it is the generation-scoped seam other things attach to |
| `plugins` (87 `cfg`) | it is the generic host; the `cfg` count is the price of the seam existing at all |
| `sqlite-storage`, `tokio-transport`, `tokio-runtime`, `ureq-client`, `signal`, `tokio-native` | platform adapter selection: the core names a trait, Cargo picks the impl |
| `voip-mlow`, `voip-libopus`, `voip-encoded`, `mlow-fast-fft` | codec profile selection inside `voip`, not separate subsystems |
| `bench-harness`, `debug-snapshots`, `legacy-session-interop`, `danger-skip-*` | build-time switches |

### Cross-cutting

`tracing` (14 sites) and `metrics` (0 sites in this crate) instrument code at the
point being instrumented. Their gates are not coupling.

### Not subsystems

`src/message` (21k lines), `src/send` (8.4k) and `src/features`' shared
plumbing are the hot path and the core's own work. They fail tests 1 and 2 by
construction and are listed here so a later reader does not re-derive it.

## The seam

`src/client/subsystem.rs` is the whole design. It holds one `const` table:

```rust
pub(crate) const SUBSYSTEMS: &[Subsystem] = &[
    #[cfg(feature = "passkey")]
    crate::passkey::SUBSYSTEM,
];
```

`Client` gains one field, `subsystems`, not one per subsystem. A subsystem parks
its per-client state there (type-keyed, built once during assembly) and lists
the notification types it models. With no subsystem compiled in, the table is a
zero-length slice, so every loop over it folds away and `Subsystems` holds an
empty boxed slice, which allocates nothing.

The table is a `const` rather than runtime registration on purpose. Static
registration through a linker-section crate would remove the last `cfg` the core
carries, at the cost of a new dependency; one `cfg` in one file was not worth
that, and the guard test makes the "one" enforceable.

## What a subsystem costs

Stripped `demo`, release profile (fat LTO, `codegen-units = 1`), the same build
`binary_size_ci.md` gates on. Sizes are deterministic for a pinned toolchain:
the baseline reproduced byte for byte across two runs.

| build | bin size | vs default |
| --- | ---: | ---: |
| default, `passkey` compiled in unconditionally (the old shape) | 10,806,752 | |
| default, `passkey` off | 10,756,992 | -48.6 KiB |
| default + `passkey` | 10,809,824 | +51.6 KiB |
| default + `plugins`, host on and no plugin installed | 10,960,960 | +150.6 KiB |
| default + `voip` | 11,373,952 | +553.9 KiB |

Two things to read out of that. Turning the smallest cuttable subsystem off is
worth ~49 KiB, and the seam that makes it cuttable costs 2.5 KiB of `.text` when
the subsystem is on, which is what the whole design buys and costs. And the
`plugins` row is the enabled-with-no-plugin number `plugin_architecture.md`'s
checklist asks for and nobody had produced: 150.6 KiB for a host with no
consumer.

## What the guard proves, and what it does not

`tests/subsystem_boundary.rs` scans `src/` and fails when a cuttable subsystem
is named outside the files it owns and its two allowed core mentions.

It does not prove the compiled-out subsystem left no trace. Its `Event` variants
and payload types stay in `wacore` by test 4, so "zero cost" means zero code,
zero `Client` state and zero branches from *this* crate, not zero bytes in the
binary. The binary numbers in the PR are what quantifies the difference.

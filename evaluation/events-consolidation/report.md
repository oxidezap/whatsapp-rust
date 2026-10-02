# Events validation receipt

PR: https://github.com/oxidezap/whatsapp-rust/pull/1607
Base: `d9f78b806f1f4ca80c8008caa5846e5d542c2c55`.
Implementation checkpoint: `fb2d6e20f2c69764b12f1faa666f6162a66f9179`.
Final tested code/CI source: `a5b7c4e502d4cc34d62ff05dd7577b00f0cb1938`.
Subsequent changes are documentation-only (this receipt, cancellation limits and
an exported-symbol SAFETY comment in the standalone executable).
The forge's current PR head and the final operational receipt identify the latest
published source; results below are explicitly checkpoint-scoped, not blanket CI
approval. Full local command output, exit files, timings, input hashes and CI/review
receipts remain in `.task-events/` in the isolated task worktree.

## Executed at the implementation checkpoint

Standard Cargo via configured mbx, `CARGO_TARGET_DIR` unset, jobs=1; sequential
children completed before subsequent source/configuration edits.

| Control | Result |
| --- | --- |
| Original base `event_delivery_public` | 12 passed; original five-second causal controls |
| Head `event_delivery_public` | 13 passed, including new ordered/drop-newest control |
| `nextest run -p whatsapp-rust -p wacore --lib` | 4,153 passed; 4 existing skipped |
| Touched crate doctests | 37 passed; 27 existing ignored |
| Standalone `tests/event-consumer` | 13 public tests, 1 synchronous host test, 2 negative old-name doctests passed |
| `clippy -p whatsapp-rust -p wacore --all-targets -- -D warnings` | exit 0 |

The early setup status accidentally called the original public baseline 13 tests;
the retained actual log shows **12**, and subsequent status corrected it. Head has
**13** because one ordered/overflow test was added. No assertion, five-second
deadline, cleanup ordering or required check was weakened.

The controls observe cancellation before terminal cleanup, actual driver future
drop, capture/Client release, independent subscription survival, bounded running
callbacks, exact overflow, ordered accepted events and one shared ChatPresence
fact per registration. Typed chatstate subscriptions and general bus subscriptions
observe the same parsed payload. Native/core tests retain stanza/central-reader
and IQ controls. The standalone host exercises synchronous delivery, unsubscribe,
bounded defaults and an explicit unbounded channel through coherent crate exports.

## Final-source validation

The final sequential queue at `a5b7c4e5` exited 0 for all commands: the same
public 13, lib 4,153 (4 existing skipped), doctests 37 (27 existing ignored), root
and standalone formatting checks, matched native/WASM builds, and the standalone
host tests at **Rust 1.94.1 MSRV** (13 public + 1 synchronous host + 2 negative
old-name doctests). No children remained live when documentation was corrected.
Runtime/CI code and executable fixture behavior are unchanged by the subsequent
documentation-only corrections; the export's single definition was checked with
repository search and `nm` on the linked native executable.

Default touched Clippy passed at the implementation checkpoint; final-head CI
still owns all-feature build/lint/test, feature-matrix, rustdoc and full demo-size
results. Pending CI or rate-limited reviews are not described as approval.

## Size and portability scope

Matched standalone native/WASM consumer builds use the same fixture, locked
resolution, release profile, compiler and optimizer flags before/after cleanup.
| Metric (bytes) | Baseline | Head | Delta |
| --- | ---: | ---: | ---: |
| Native stripped executable | 2,812,504 | 2,812,456 | -48 |
| Native `size` text | 2,794,685 | 2,794,641 | -44 |
| Native data | 14,440 | 14,440 | 0 |
| Native bss | 2,690 | 2,738 | +48 |
| Native allocated (text + data + bss) | 2,811,815 | 2,811,819 | +4 |
| Linked WASM consumer | 220,071 | 220,063 | -8 |

Artifacts are not byte-identical. These tiny deltas are reported, not attributed
to name removal as a saving. This is a narrow synchronous-host profile, not a
whole-client or callback workload/RAM benchmark. No size/RAM savings are claimed.
The existing full demo CI size budget and WASM/no-default-feature gates are
unchanged. Compiler: repository-pinned nightly-2026-06-16; native config flags are
unchanged, WASM uses the existing `--cfg getrandom_backend="wasm_js"`. Standalone
release defaults and locked dependency resolution are identical in both builds.

Resource measurements were repeated before each heavy queue; compilation was
jobs=1, with no cache/target administration. The implementation validation's
largest observed peak was 3,066,632 KiB. Historical free RAM/disk is not a
reservation for other lanes.

## Forge evidence, failures and ownership

Initial source checkpoint reviews: Greptile substantive review (5/5, no findings),
Cubic substantive review (2 findings: CI artifact reuse and deliberate source
breaks), Codex configured code/security reviews completed. CodeRabbit's initial
passing check was **rate-limited, not a substantive review**; its stated reset was
33 minutes. Its eventual substantive full review at `fe8fafba` posted one finding:
add a SAFETY note to the fixture's retained WASM export. That note is now present,
with single-symbol ownership verified; the export remains necessary to retain the
host probe in the linked WASM artifact. Latest-head reviews and required CI are
still being monitored.

Cubic's subsequent contract wording finding is addressed by explicitly stating
cooperative future-poll-boundary cancellation: no preemption of blocking code or
cancellation of work spawned elsewhere. This matches existing adapter rustdoc and
changes no Runtime behavior.

Cubic's CI reuse finding is addressed by using the existing MSRV job's shared
`github.workspace/target` for the standalone consumer, without removing any test,
assertion or job. The intentional source-break finding is documented in rustdoc,
PR migration and `contract.md`, without undoing the explicitly requested removals.

Required Cargo Deny is **red**, not waived: the unchanged `tools/oracle-task`
Wasmtime 48.0.3 pin triggers RUSTSEC-2026-0325/0326/0327 (fix >=48.0.4). Initial
failure: https://github.com/oxidezap/whatsapp-rust/actions/runs/37072260851/job/111054188763
Full job log is retained as `.task-events/cargo-deny-full.log`. Firstmate assigned
the separate `wr-host-wasmtime-new-advisories` backlog ownership; this disposition
does **not** resolve the audit failure or authorize a security patch/ignore here.

Informational semver is also **red** against published 0.7.0: 23 major categories
in wacore, 3 in wacore-binary and 7 in waproto. Its inventory covers preexisting
struct/enum/trait/module/feature/protobuf changes; our core change is trait docs
only. That job does not select the runtime crate, so it does not itself enumerate
these two intentional runtime removals; the external negative doctests do.
Full inventory remains in `.task-events/semver-full.log`. No aliases, generated
files, audit ignores, budgets, profiles, workflow permissions/triggers, release or
protocol claims were changed to hide failures. No WhatsApp capture was executed;
this task changes Rust delivery API, not Web protocol interpretation.

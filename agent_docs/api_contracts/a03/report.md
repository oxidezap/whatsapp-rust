# A03 execution report

Base: `d9f78b806f1f4ca80c8008caa5846e5d542c2c55`. Source implementation complete; initial source checkpoint publication follows the directed gates below. Current-head CI/reviews and the remaining gates are not yet accepted.

- Isolation verified by physical cwd and git toplevel; branch `fm/wr-api-a03-encapsulation`.
- Round contract/assessment, original A03 request and landed A06 contract/handoff read completely. No dependency on A13 names; no send/edit facade hidden.
- Initial bounded default check exited **101**: the new reconnect notifier accidentally used the protocol `Event` enum instead of `event_listener::Event`. Corrected only its type and constructor after the foreground process ended. Peak 2,027,592 KiB, 7m23s.
- Second bounded default check exited **101**: the new test used BotBuilder's backend input on ClientBuilder. Production library checked; new fixture corrected to the existing persistence-manager input, plus five unnecessary qualifications. Peak 2,117,776 KiB, 1m49s. These are separate compiler errors, not successful tests.
- Corrected default test compilation exited **0**, peak 1,843,216 KiB. Independent single-dependency no-default consumer check exited **0**, peak 2,084,040 KiB; that lock resolves its own compatible dependencies, not root dev-feature unification.
- Public encapsulation/lifecycle/admission/event/report tests: **45/45 passed**, no skips, exit **0**, peak 2,441,708 KiB.
- Directed library mutation/notification/save cancellation/adapter/PN-LID/ownership/raw lease/handler/cache-flush controls: **44/44 passed**, 2380 filtered, exit **0**, peak 3,618,976 KiB. Filtering is not a full-suite pass.
- SDK doctests: **34 passed, 16 existing ignored**, exit **0**, including five independent compile-fail negative controls (one per field and manager lock). Compile-only examples are not runtime execution.
- Full SDK/all-feature quality, actual MSRV, WASM consumer, matched size and current-head forge checks/reviews remain **pending/unrun** at this checkpoint. No historic parent acceptance is reused.
- Actual public bridge audited statically at `1fa1484ff98eb90d3f083c8a510769635537e535`, pinned SDK revision `c4dc16d223917d1aab759e9828f6ea82904f0c99`: reconnection wrapper and two HTTP calls need the new narrow methods. Manual sync/shutdown/backend/cache/admission/runtime traits remain available. Migration handed to Firstmate; no bridge source/pin/producer was changed or full bridge compilation claimed.
- fmt completed; changes confined to implementation visibility/accessors, invariant-preserving scoped mutations, required test migrations, external proof and this bounded handoff. AGENTS changed only to correct the now-removed lock method's factual instruction.
- No protocol/Signal/cache algorithm, persisted schema, version, optimizer, size budget or pipeline assertion changed. Removing names is not a measured RAM/binary saving.

Operational logs/exits/search outputs: `.refs/a03/` in this disposable task worktree. Standard Cargo via configured mbx, `CARGO_TARGET_DIR` unset, jobs=1. Resources measured before checks: 21GiB available RAM/185GiB disk with seven visible other Cargo lanes; own bounded check planned at 2GiB, heavy-phase coordination requested rather than starting ten full matrices.

Advanced visibility/consumer migration contract for A08: `contract.md` alongside this report. Public bridge source consumers and exact replacement bodies are recorded in that contract and the independent consumer. Unpublished producers and whole-bridge adoption are not claimed validated.

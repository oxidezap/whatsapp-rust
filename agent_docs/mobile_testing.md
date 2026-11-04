# Mobile validation boundaries

Mobile protocol decisions belong in `wacore::mobile`. Pass time, randomness,
service results and persistence completion into the decision logic explicitly.
Do not read an ambient clock, start a task, open a socket or access a database
from a mobile state transition. Concrete runtimes, transports, HTTP clients,
push/attestation providers and stores remain replaceable host adapters.

The architectural guard in `tests/mobile_boundary.rs` inspects the production
mobile sources for runtime, I/O, ambient clock and entropy dependencies. Its
negative cases prove that aliased imports and nested macro arguments cannot
silently introduce those dependencies. This is an architectural tripwire, not
proof that every called dependency is pure or that a target executes correctly.
Review newly allowed dependencies and keep the target feature checks below.

## Future bartender/barback contract

bartender/barback mobile APIs are a future integration target. The existing Web
E2E suite uses `TestClient` in `tests/e2e/src/lib.rs` and the image pinned by
`.github/bartender-image.txt`, selected in `.github/workflows/e2e.yml`. That Web
fixture establishes neither available mobile routes nor mobile protocol parity.
Do not assume a mobile endpoint or admin API from its Web URL.

When the server implements mobile support, qualify a versioned integration
contract before adding its scenarios to the mandatory E2E gate. Record:

- The server image digest or source revision, configuration and fixture/protocol
  revision. Select the same pinned artifact for local reproduction and CI.
- The exact supported capabilities, such as registration, OTP challenges, login,
  primary/companion linking, device revocation and history/app-state. A missing
  required capability must fail preflight explicitly. A skipped scenario is not
  passing coverage.
- Independent registration HTTP, mobile connection and admin/control endpoints.
  Inject them per client, with no fallback to a production destination. A test
  control endpoint is not a protocol endpoint and must not become a public wire
  format assumption.
- Test-only registration encryption keys, Noise trust roots and server identity.
  Keep validation enabled against those explicit roots. Never use a global
  certificate bypass as the mechanism for testing another server identity.
- Isolated credential/storage namespaces bound to the selected environment.
  Reconnect must retain that binding and must not reuse mock credentials against
  production, or production credentials against the mock. Web and mobile clients
  must be able to run concurrently without mutable global endpoint/provider state.
- Deterministic OTP, push and attestation fixtures at the service boundary, using
  fictitious identities and test-only keys. These simulate provider results;
  they do not qualify real SMS delivery or hardware-backed attestation.

The real protocol builders, envelope encoding, parsers, state transitions and
persistence commands must run unchanged against the mock. Do not introduce a
mock-only registration path or a branch that skips validation for bartender.
Use a network allowlist for the E2E process/container so that any accidental
production request fails, including redirects, reconnects and fallback routes.

## E2E scenario requirements

Once the corresponding mock capability exists, cover the following through the
normal SDK/transport/HTTP paths and wait for observable events or state changes.
Do not use arbitrary sleeps to establish ordering.

| Capability | Scenarios and assertions |
| --- | --- |
| Registration | Request/submission success, incorrect/expired code, per-method rate limits, declined challenge/PIN, unknown or malformed responses, cancellation and bounded retry exhaustion. No real SMS or number registration. |
| Uncertain outcomes | Lose a response after server execution, restart from persisted state and reconcile explicitly. Cancellation or timeout must not imply that a request never reached the server. |
| Login and transport | Independent mock destination, fragmented/coalesced reads, EOF, backpressure, partial/cancelled writes, trust failures, reconnect and session restore. Validate cancellation releases resources. |
| Persistence | Inject write failure and cancellation, verify snapshot publication follows durability, restart without losing an outstanding operation or its environment binding. |
| Primary role | Approve companion QR and pairing code, signed identities, key-index/device-list updates, concurrent link attempts and revocation. Exercise app-state/history only for declared server capabilities. |
| Session isolation | Concurrent Web/mobile clients, separate environments and stores, no endpoint/provider leakage, no production fallback and no change to the existing Web behavior. |
| Provider behavior | Missing/invalid attestation, push delivery failure and unavailable provider produce distinct outcomes. A noop provider cannot report evidence of successful hardware attestation. |

Preflight and scenario output must retain the server digest, selected capability
set, synthetic fixture identifiers and actual results. Redact credentials, PINs,
OTP values and push/attestation tokens. Keep failed, unsupported and unexecuted
scenarios visible rather than turning them into successful empty tests.

## Portability qualification

Mobile core must remain available without native transport, HTTP, Tokio or an
operating system. The optional native TCP adapter is one implementation of the
existing transport contract, not part of the mobile state machine. Host code
owns deadlines, task cancellation, service calls and storage I/O.

Qualification is per target, feature profile, runtime and store, as described in
`support.md`. Add isolated consumer checks so workspace feature unification
cannot conceal an accidental native dependency. At minimum, exercise the mobile
core by itself, the default Web client, and Web+mobile in the same binary. Run
mobile feature checks with defaults disabled for the WASM bridge profile before
claiming browser compatibility. A browser requires a host-provided transport;
compilation cannot make arbitrary TCP sockets available to browser JavaScript.

The isolated profiles are registered in `tools/xtask/consumers.json` and the
compatibility profiles in `tools/xtask/compatibility.json`. The API consumer uses
the SDK re-export and checks environment-bound state and uncertain-outcome restore
without an executor. Reproduce the focused profiles with:

```console
cargo test --manifest-path tests/api-consumer/Cargo.toml --locked --no-default-features --features mobile-core --lib
cargo test --manifest-path tests/api-consumer/Cargo.toml --locked --no-default-features --features mobile-transport-native --lib
cargo test --manifest-path tests/api-consumer/Cargo.toml --locked --no-default-features --features native-default,mobile-core --lib
RUSTFLAGS='--cfg getrandom_backend="wasm_js"' cargo build --manifest-path tests/api-consumer/Cargo.toml --locked --no-default-features --features mobile-core,js --target wasm32-unknown-unknown --release --lib
```

The registered WASM lane also sets its required getrandom backend configuration;
use `cargo xt ci consumers run --lane wasm` for the complete maintained command.
A passing WASM build is a link check, not browser mobile execution. The optional
native transport profile is intentionally not a WASM profile.

ESP32 remains experimental until an explicit supported target, allocator/runtime,
transport and persistence adapter can build and execute representative scenarios.
Do not claim ESP32 support from a Linux or WASM build. Check target memory budgets
and concurrency assumptions separately. The absence of mandatory Tokio/I/O in
core is necessary for portability, but is not target execution evidence.

## Evidence must remain separate

Pure state tests prove decisions under supplied inputs. Mock E2E tests prove
integration under bartender/barback's modeled behavior. APK differential tests
compare a bounded behavior with a pinned official client artifact. Separately
authorized live tests establish only the server/account/version conditions they
actually exercise. None of these categories substitutes for the others.

Do not mark the future mobile mock gate validated until its required capabilities
exist and the scenarios execute. Do not require that future server work to land
before testing the pure core and optional native transport offline.

Registration checkpoints bind both the selected environment and the account's
identity/Noise public-key digest. Restore rejects a different binding. These
hashes prevent accidental mixing; they do not authenticate untrusted storage or
detect rollback to an older database. Semantic response inputs must come from a
qualified parser. Local `confirm_persisted` records the host's durability
confirmation; it does not perform or independently prove a database write.

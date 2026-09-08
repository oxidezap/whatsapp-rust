# Transport reuse audit

Baseline `47e1b5b41b63c23b59372828901ea945c8149565`, audited on 2026-09-08.
This audit covers runtime access, ED construction and TLS state ownership.

## Runtime

Keep `whatsapp_rust::TokioRuntime`, `runtime_impl::TokioRuntime` and the prelude
export behind `tokio-runtime`. A standalone consumer compiled with defaults
disabled, using `Runtime` and the Tokio transport without importing `Client`.
SQLite, Diesel, ureq and OS signal handling were absent. The main crate still
compiled, as did Signal protocol through wacore's unconditional dependency.
The `signal` feature controls OS signals, not Signal protocol.

Documentation is the chosen change. Moving the adapter into transport would
make runtime-only users acquire WebSocket/TLS dependencies unless another
configuration were added. A separate crate could preserve the old exports but
adds publication and CI responsibilities without removing wacore's Signal
dependency. No measured extraction benefit justifies either change here.

External probes passed on current-thread and multi-thread Tokio. They covered
spawn, cancellation on drop, detach, executor shutdown, lazy blocking submission,
queued and running blocking work, panic handling and timer requirements. Raw
blocking joins discard panic errors under unwind; the result-returning core
helper raises a new panic when its oneshot loses the result. Abort-profile
panics terminate the process. The adapter's yield hook remains a no-op on both
executor flavors; 1,000 calls did not schedule a queued peer. The docs now state
that limit rather than claiming multiple workers eliminate cooperative yielding.

The consumer's release build compiled 145 packages including itself. Compared
with its core/transport-only graph, enabling the existing adapter added
`whatsapp-rust` and `scopeguard`. Symbol inspection found adapter functions but
no named Client or Signal functions. That is not proof that all unrelated
inlined code or anonymous data disappeared. No adapter relocation or linked-size
improvement is claimed.

## ED

The pinned whatspec IR at `1a441f0329c941fcdb238490a6c604550d8a9939`, WA
`2.3000.1045368834`, contains routing references but not this socket construction
flow. The official `.wa-cache` instead identifies WA `2.3000.1044770897`.
All 504 cached bundle sizes and hashes matched its manifest. Do not attribute
that older bundle to the newer IR version.

Primary bundle evidence, using content SHA-256 and original bundle lines:

| Content hash | Modules and lines |
| --- | --- |
| `4de5ec27b53a202be96d64493441fe6aed113d6e35e5745e61d1f32a3341a30a` | `WABinary` 8, `WABase64` 27 |
| `e855464a86b395cabf4aa380540a5585565e6e32db1765227c4a3aac5e94a3c7` | `WAFrameSocket` 1379, `WAWebOpenSocket` 1455, `WAWebOpenChatSocket` 1459 |

`WAWebOpenChatSocket` reads stored opaque routing bytes, calls
`encodeB64UrlSafe` without its padding argument, and gives the resulting string
to `WAWebOpenSocket`. That module appends `?ED=` to both fixed endpoints for
non-null strings. Independently, the same raw bytes enter the binary pre-intro:

```text
45 44 00 01 | three-byte big-endian byte length | routing bytes | WA header
```

Executed official module factories confirmed the synthetic vectors in
`edge_routing_param_retains_padding_unlike_captured_wa_web`, including `01` to
`AQ` and `FB FF` to `-_8`. Prefix-only tests exercised the three-byte length
boundary without allocating maximum-sized routing payloads.

Keep the existing Rust helper's output contract. It emits padded base64url,
omits absent, empty or oversized bytes, and appends before fragments without
validating URLs or replacing existing ED parameters. In contrast, official
empty ArrayBuffers produce `?ED=` and a zero-length binary pre-intro. Rust's
binary builder also accepts empty bytes, but oversized routing falls back to
WA-only, whereas the official binary writer rejects the unrepresentable length.
These are retained differences, not a claim of complete WA output parity or
proof that the server rejects padded output.

No general ED decoder or HTTP parsing API is added. No production caller of the
query helper or request-side ED decoder was found in this repository. Integrators
own missing versus empty input, malformed base64/padding rejection, one-time
percent decoding, duplicate rejection and bounded HTTP request parsing. Existing
base64 engines supply strict padded and unpadded decoding. Bound the request and
encoded component before decoding, then bound decoded bytes. A 4 KiB request
policy is not a protocol limit; `0xFFFFFF` comes from binary framing and is not
a sensible default HTTP budget. A query built externally is not automatically
synchronized with the device snapshot used later by the Noise handshake.

## TLS

Keep `with_connector` and per-factory defaults. `Connector` is not cloneable;
its rustls payload clones an `Arc<ClientConfig>`. That config owns the session
store. Cloning the config itself also shares its Arc-backed store, so it does
not establish tenant isolation. A full client already retains its factory.
Transport-only callers recreating factories should construct the connector once
per intended trust/client-identity/tenant policy and inject payload clones.

Pinned rustls 0.23.43 exposed a separate defect. An eight-ticket budget gives
its server-name queue capacity one, and insertion evicts when length reaches
capacity. Public cache probes retained a key-exchange hint in 0/100 fresh caches
at budget eight and 100/100 at sixteen. Sixteen retains one server name while
leaving the per-name TLS 1.3 ticket maximum at eight. This minimum sizing fix
benefits both full clients and standalone transports without global sharing.

Local trusted TLS 1.3 tests observe `Full`, then `Resumed` on repeated dials and
shared factories with changed synthetic ED queries and ports. Ticket insertion
events precede reconnects. An independent config/store performs `Full`, and
returning to the shared config resumes. Config pointer checks establish retention
within one factory and separation across default factories. These are identity
observations, not allocator or constructor-call measurements.

Factory tests use IPv4 literals and assert no SNI for IP addresses. A separate
preconnected loopback test supplies a DNS ServerName to tokio-rustls, observes
localhost SNI and rejects a different DNS name. Factory tests also reject an
untrusted root and a trusted certificate lacking the requested IP SAN. Origin
remains unchanged. Every network phase has a deadline, with no DNS dependency,
arbitrary sleeps, external connections or verification bypass.

New dials still open new sockets. Session keys use server names, not ED or ports;
concurrent racing attempts can consume tickets, and server policy determines
whether they resume. No WhatsApp resumption rate, latency or memory saving is
claimed. The full URL was removed from the dial debug log to avoid disclosing ED.

## Cost and validation

No runtime move, parsing dependency or new crate. `rcgen` was already locked and
is now a transport dev dependency; WebSocket server support is enabled only for
tests. The lockfile also reconciles the baseline's rtc-shared version with the
existing exact beta.2 manifest pin, required for locked validation. Production
dependencies and public paths are unchanged. The cache now retains state that
the broken configuration discarded. Empty and populated heap sizes have not
been remeasured, and the observability guide marks its old numbers as historical.

Toolchain was rustc `1.98.0-nightly (01dfd7924 2026-06-15)`, with two build jobs.

| Command or check | Result |
| --- | --- |
| `cargo fmt --all` | Passed |
| `cargo clippy --workspace --all-targets -- -D warnings` | Passed |
| `cargo nextest run --profile ci -p wacore -p whatsapp-rust --lib` | 3,643 passed, three existing skips |
| `cargo nextest run -p whatsapp-rust-tokio-transport --lib --locked` | 12 passed |
| Transport stress, 20 iterations without retries | 240 passed |
| Transport doctests | One passed |
| Runtime-only main doctest | One passed |
| Core no-default doctests | Three passed, 11 existing examples ignored |
| External runtime probes without/with multi-thread support | Four/six passed |

Workflow wasm release builds passed for main without defaults, core with
`voip,js`, and main with `voip-mlow`, using
`RUSTFLAGS='--cfg getrandom_backend="wasm_js"'`. Main emitted two existing
unused-qualification warnings in request/upload code. These are build checks,
not wasm execution tests. Core normal/build dependencies contain no Tokio.

An initial full-suite failure in
`online_device_sync_releases_its_dedup_entry` reproduced unchanged in a separate
baseline worktree on iteration 729/1,000. Its fixed yield loop sometimes ends
before cleanup. The final rebuilt patch suite passed; the test was not relaxed.
No E2E infrastructure or live WhatsApp acceptance test was used. Ignored tests
and unmeasured performance are not passing evidence.

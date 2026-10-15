# whatsapp-rust

[![crates.io](https://img.shields.io/crates/v/whatsapp-rust.svg)](https://crates.io/crates/whatsapp-rust)
[![crates.io downloads](https://img.shields.io/crates/d/whatsapp-rust.svg)](https://crates.io/crates/whatsapp-rust)
[![CodSpeed](https://img.shields.io/endpoint?url=https://codspeed.io/badge.json)](https://app.codspeed.io/oxidezap/whatsapp-rust?utm_source=badge)

An async Rust library for WhatsApp Web. Pair a device, send and receive end-to-end encrypted messages, and build on a modular protocol stack.

[Documentation](https://whatsapp-rust.jlucaso.com) · [llms.txt](https://whatsapp-rust.jlucaso.com/llms.txt) · [llms-full.txt](https://whatsapp-rust.jlucaso.com/llms-full.txt)

## Capabilities

- QR and pairing-code authentication with persistent sessions.
- End-to-end encrypted direct and group messaging, media, reactions, receipts, and history sync.
- Groups, communities, newsletters, status, contacts, and privacy controls.
- Experimental audio and video calls, both 1:1 and group, plus call links and screen sharing. The optional `voip` feature provides native relay transport and MLOW/Opus audio. Video takes application-supplied H.264 frames ([media API and codec options](https://github.com/oxidezap/whatsapp-rust-docs)).
- Replaceable storage, transport, HTTP client, and runtime. The default features include SQLite, Tokio WebSocket, ureq, and Tokio. Applications still choose a storage backend. Native plugins are [opt-in](agent_docs/architecture.md).

## Quick start

```toml
[dependencies]
whatsapp-rust = "0.7"
tokio = { version = "1", features = ["macros", "rt-multi-thread", "signal"] }
```

```rust,no_run
use whatsapp_rust::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bot = Bot::builder()
        .with_backend(SqliteStore::open("whatsapp.db").await?)
        .on_qr_code(|code, _timeout| async move {
            println!("Scan to pair:\n{code}");
        })
        .on_message(|ctx| async move {
            if ctx.message.text_content() == Some("ping") {
                let _ = ctx.reply("pong").await;
            }
        })
        .build()
        .await?;

    bot.run().await;
    Ok(())
}
```

The main crate re-exports the bundled stack. You do not need to add its sibling crates separately. To use a specific Git revision instead of crates.io:

```toml
[dependencies]
whatsapp-rust = { git = "https://github.com/oxidezap/whatsapp-rust", rev = "<commit>" }
```

See the [documentation](https://whatsapp-rust.jlucaso.com) for configuration, alternative backends, and API examples.

## Support contracts

This branch documents the current pre-1.0 API. It does not establish a release
candidate or a frozen compatibility baseline. Use documentation for the version
you depend on. When following unreleased APIs, pin a full Git commit rather than
assuming the published `0.7` package contains every API on `main`.

### Crates and extension points

| Package | Intended use and compatibility responsibility |
| --- | --- |
| `whatsapp-rust` | Consumer API, feature facades, lifecycle reports and host adapters. Public low-level modules and re-exports remain public API; a feature flag does not waive source compatibility. |
| `wacore` | Advanced protocol, IQ, runtime, network, storage and client-context implementers. Platform-neutral means independent of Tokio, not independent of `std`. |
| `wacore-binary`, `wacore-appstate`, `wacore-libsignal`, `wacore-noise` | Advanced node/JID, app-state and cryptographic protocol APIs. These are not a separate promise of complete WhatsApp interoperability. |
| `waproto` | Generated public models, views and enum representations. Follow its non-exhaustive construction policy. Unknown protobuf fields are discarded on decode/re-encode; retain original input bytes for lossless forwarding. |
| `wacore-derive` | Public derive macros, including their generated paths and construction contracts. |
| `whatsapp-rust-sqlite-storage`, `whatsapp-rust-tokio-transport`, `whatsapp-rust-ureq-http-client` | Optional host implementations, with platform and dependency requirements of their own. |
| Plugins, examples, tests and `tools/*` | Unpublished workspace components. Test/benchmark features are fixtures, not shipping profiles. Host tools are not runtime dependencies. |

Runtime, transport/factory, HTTP, storage domain traits, durability hooks and the
neutral VoIP backend/session traits are intended implementation points. Keep them
open to downstream implementations. `Backend` is composed by a blanket
implementation from `SignalStore`, `AppSyncStore`, `ProtocolStore`, `DeviceStore`
and `MsgSecretStore`; implement the domain traits rather than a competing
`Backend` implementation. Custom core client contexts also own the synchronization
obligations documented on their methods, including pairwise session serialization.

Public dependency representations are part of these contracts. The SDK
re-exports `anyhow`, `async_channel`, `async_trait`, `bytes`, `futures`, `serde`,
`serde_json`, `chrono` and `buffa` to allow matching versions. Node/JID and
`CompactString` types also appear publicly. Re-exporting them does not hide their
compatibility cost. Use the target-correct `BoxFuture` and async-trait conventions
when implementing adapters; consult the owning trait rather than adding blanket
`Send` bounds. Storage domain traits retain `Send + Sync` on WASM too.
Published packages declare their MSRV in their manifests; host tools can have a
different floor.

| Implementer | Required contract |
| --- | --- |
| `Runtime` | `Send + Sync` object on native and WASM; native task futures are `Send`, local WASM futures need not be. Abort closures remain `Send`; abort is not join completion. |
| `Transport` / `TransportFactory` | Target-conditional `MaybeSendSync` and async-trait futures; `Bytes`, `anyhow::Error` and an `async_channel` receiver are exposed. Dropped dial futures must not leak transports. |
| `HttpClient` | Target-conditional async execution with status-preserving responses. Streaming is an optional synchronous `Read + Send` capability, not an async chunk protocol. |
| Five storage domain traits | `Send + Sync` even on WASM; required atomic patch commits and device scope. Optional pending-inbound support fails closed; batch overrides must preserve bounded/CAS ownership rules. |
| Durability hooks | Target-conditional bounds; host-owned idempotent commits and cancellation reconciliation. Message, group and history capture are separate registrations. |
| Neutral media backend/session and relay ports | Target-conditional bounds; the host supplies media and relay capability. Audio/video source and sink ports separately retain `Send + Sync`. |

### Features and targets

| Profile | Configuration and host obligations | Evidence boundary |
| --- | --- | --- |
| Native defaults | SQLite with bundled SQLite, Tokio runtime/transport, ureq, `tokio-native` and OS `signal`. Choose a backend explicitly. | Native CI and isolated API consumers. This does not qualify every OS or deployment. |
| Native `std`, without Tokio | `default-features = false`; provide runtime, transport, HTTP and storage implementations. `wacore` has no Tokio runtime dependency. | Separate core/SDK consumer profiles prevent workspace dev-feature unification from standing in for a minimal consumer. This is not `no_std`. |
| Local WASM | `wasm32-unknown-unknown`, defaults disabled, JS randomness configured by the host. Futures can be non-`Send`; network adapters use `MaybeSendSync`. | WASM CI builds and executes selected JS-hosted probes. It does not establish browser-wide, WASI or threaded/atomics WASM support. `Runtime` still requires `Send + Sync`, and its abort closure still requires `Send`. |
| ESP-IDF / ESP32 | Portable core with `std`, application-supplied executor, entropy, network and storage. Native defaults and native VoIP relay are not an ESP-IDF profile. | The existing ESP32 consumer pins an older SDK and needs API migration for current main. Its declared S3/C3/C5 targets are not candidate-qualified. There is no ESP-IDF CI lane here. |
| VoIP control | `voip-control` exposes signaling and neutral media backend ports. Without an injected backend, media setup returns `NoBackend`. | Control-only consumers are separate from the engine graph. |
| Portable VoIP engine | `voip-runtime` / `voip-engine-wacore`; `voip-encoded` accepts application-supplied encoded audio. `voip-mlow` adds PCM/MLOW, `voip-libopus` links the Opus adapter. | A portable build is not proof of a host relay, codec budget or live peer interoperability. |
| Native VoIP | `voip-relay-native` adds UDP/DTLS/SCTP and Tokio; `voip` also includes both codec adapters. | Experimental interoperability. Direct/group voice/video, call links, screen sharing and upgrade/downgrade need qualification for the exact peers and negotiated modes used. |

The [ESP32 consumer at `540603be`](https://github.com/oxidezap/whatsapp-rust-esp32/tree/540603be53012b905c9288add6663a2734a87206)
pins SDK `e14300d1`. A host API probe against SDK `20c48f54` identified missing
atomic app-state and typed message-secret methods, secret conversion changes and
non-exhaustive HTTP construction. An adapted signature-only control compiled;
this is not a firmware build or a working storage migration. ESP-IDF compilation,
board-specific resource limits, reboot recovery and physical flash/power-loss
behavior still require validation. Do not promote an older integration into a
support claim for current main.

`signal` means OS signal handling, not the Signal encryption protocol.
`tracing-pii`, debug snapshots and TLS verification bypasses are diagnostic
choices, not production defaults. Feature combinations remain subject to their
adapter's platform requirements.

HTTP streaming currently uses synchronous `Read + Send` bodies and a known upload
length. It requires a blocking context. Browser buffered HTTP is a separate
capability; asynchronous Fetch chunk streaming and bounded whole-body buffering
are not guaranteed by this interface. Check the adapter's streaming capability
methods. A preallocation cap is not a download byte limit.

Hosted-business coexistence and hosted/E2EE account-type transitions are outside
the supported capability boundary. Device identity changes are handled, but
account-type transition cleanup is deferred in notification and usync paths.
Do not infer support from the presence of a parsed account-type value.

| VoIP mode | Interoperability scope |
| --- | --- |
| Direct voice, incoming and outgoing | Experimental. Qualify bidirectional audio, termination and negotiated MLOW/Opus or encoded I/O with the actual peer. |
| Direct video, incoming and outgoing | Experimental. Picture reception does not establish peer audio; qualify both media directions and application-supplied H.264. |
| Group voice/video, incoming and outgoing | Experimental. Direct-call evidence does not qualify group membership, relay or key transitions. |
| Call links, screen sharing, upgrade/downgrade | Experimental. Qualify each transition and negotiated mode separately. |

Engine tests and preserved official-client code do not prove audible peer media
on a particular phone/Web build. Record peer versions, feature profile, codec,
direction, topology and tested commit when qualifying a deployment. This scope
does not change the source compatibility obligations of public call APIs.

### Delivery by origin

Observers are not a durable inbox. Choose the gate for the actual origin and
persist idempotently before returning success. No gate promises exactly-once
application effects or indefinite server retention.

| Origin | Gate and ACK/receipt boundary | Persistence, retry and restart |
| --- | --- | --- |
| Encrypted direct/group live and offline messages | `InboundDurabilityHook` commits before pending-row cleanup and positive ACK. Without it, observers do not gate ACK. | Host commits the whole batch idempotently by chat, sender and message ID. SDK pending rows support replay after durable storage; resident plaintext surviving reconnect cannot survive process exit. Admission/storage failures have narrower guarantees, detailed on the hook. |
| Newsletter/channel messages | Separate event-only path; the inbound message hook does not gate ACK. | No durable capture or replay guarantee from registering the message hook. |
| PDO placeholder recovery | Separate recovery path; excluded from the inbound message hook. | Do not treat recovered events as a durable inbox. |
| Group `w:gp2` notifications | `GroupNotificationDurabilityHook` commits the full raw envelope before effects and generic ACK. | Host atomically stores raw backing bytes and an outbox. Scoped IDs can conflict; absent IDs leave duplicate ambiguity. Withheld ACK permits possible redelivery, not guaranteed redelivery or local effects replay. |
| History sync | Separately register `HistorySyncCaptureHook`. For admitted chunks, capture completes before attempting the history receipt. Skipped/rejected chunks can be acknowledged without capture. | Host deduplicates by message ID. There is no SDK replay buffer or hook retry; withholding a receipt does not guarantee phone retry. Capture precedes decompression/validation and detached history tasks are not joined by shutdown. |
| Callbacks and event subscriptions | Bounded observers; asynchronous callbacks can drop work on overflow or cancellation. Ordered mode orders accepted work only. | Default callback capacity is 256 with concurrency 16. Inspect delivery stats; callback completion means `()` returned, not a durable commit. Drive durable downstream work from an outbox. |

Do not synchronously reply from a durability hook while the receive pipeline
holds the sender Signal lock. Commit and return, then schedule replies
independently. Cancellation can leave an indeterminate host commit; reconcile
persisted state before retrying. See the individual hook contracts for admission,
ordering and shutdown details.

### Shutdown and backend ownership

Stop application producers first. For a background bot, obtain `store_release`
from its client before consuming `BotHandle::shutdown`. Inspect both the cleanup
report and run outcome. Drop every client, persistence manager and device snapshot
owned by the host before awaiting release. Callback-initiated shutdown must not
wait for its own callback to join.

`ShutdownReport` describes one cleanup invocation, not a global durability fence.
A completed outbound drain includes cancelled work and does not confirm delivery.
Inspect inbound, outbound and Signal-settle outcomes, the device flush result,
and cumulative message-secret failures. Device and secret flushes have no total
shutdown deadline. A host timeout drops the awaited future; it is not proof that
cleanup or backend I/O stopped. Abort handles request cancellation without a join
acknowledgement, and detached work may retain backend ownership indefinitely.

`StoreRelease` observes release of crate-owned backend references. It does not
close the database, certify durability, or cover host-retained raw handles and
backend-internal I/O. Only after coordinating all owners should the host invoke
its backend-specific close/reset/remove operation. SQLite reset/remove does not
revoke stale writable handles; stop those writers before account administration.
SQLite NORMAL mode is not a power-loss durability guarantee.

The `StoreRelease` Rustdoc contains a compile-checked low-level stop sequence.
Use the Rustdoc generated from your pinned dependency for unreleased signatures. Operational recipes and
migrations remain in the [consumer documentation](https://github.com/oxidezap/whatsapp-rust-docs).

## Related projects

- [oxidezap/client](https://github.com/oxidezap/client): desktop and browser client built on this library as a complete application.
- [whatsapp-rust-esp32](https://github.com/oxidezap/whatsapp-rust-esp32): end-to-end-encrypted client on ESP32 microcontrollers, demonstrating the core on constrained hardware.
- [whatsapp-rust-bridge](https://github.com/oxidezap/whatsapp-rust-bridge): Rust/WebAssembly utilities for JavaScript, including protocol, Signal, and media operations.
- [baileyrs](https://github.com/oxidezap/baileyrs): JavaScript library backed by Rust/WASM with a Baileys-compatible API.
- [whatspec](https://github.com/oxidezap/whatspec): extracts a language-neutral protocol specification from WhatsApp Web. Its IR supplies protocol definitions generated here.

## Used by

Browse GitHub's indexed public dependents for the [crates.io package](https://github.com/oxidezap/whatsapp-rust/network/dependents?package_id=UGFja2FnZS02MTE0MTExMzY3) and the [repository entry](https://github.com/oxidezap/whatsapp-rust/network/dependents?package_id=UGFja2FnZS03MDY4Njk1OTE3). GitHub's lists are incomplete and exclude private repositories. A zero on the repository entry does not mean nobody uses the library via Git. The sidebar counter, when shown, covers only one package.

## Disclaimer

This is an unofficial client, not affiliated with WhatsApp or Meta. Using it may violate Meta's Terms of Service and could result in account suspension.

## Acknowledgements

Inspired by [whatsmeow](https://github.com/tulir/whatsmeow) and [Baileys](https://github.com/WhiskeySockets/Baileys).

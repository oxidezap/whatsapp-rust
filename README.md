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

## Support

Use docs matching your dependency. Pin a full commit for unreleased APIs;
the published package version does not identify the current `main` API.

| Profile | Requirements and limits |
| --- | --- |
| Native without Tokio | Disable default features and supply runtime, transport, HTTP and storage adapters. The portable core requires `std`; it is not `no_std`. |
| Local WASM | `wasm32-unknown-unknown`, defaults disabled and host-configured JS randomness. CI executes selected JS-hosted probes; browser-wide, threaded WASM and WASI support are not qualified. |
| ESP-IDF | `std` with application adapters. The existing ESP32 consumer needs API migration; current S3/C3/C5 firmware, resource limits and recovery are not qualified. |

For VoIP, `voip-control` accepts a host media backend;
`voip-runtime` adds the portable engine. `voip-relay-native` requires native
networking, while `voip` also includes MLOW/Opus adapters. A portable build does
not establish peer interoperability. Hosted-business coexistence and hosted/E2EE
account-type transitions are outside the supported capability boundary.

For durable ingestion, use the origin-specific hooks documented in
`types::durability_hook`; callbacks are observers. For shutdown, follow the
`StoreRelease` Rustdoc sequence and inspect cleanup results before closing your
backend. Generate Rustdoc from your pinned dependency for unreleased signatures.

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

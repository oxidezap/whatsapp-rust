# whatsapp-rust

[![CodSpeed](https://img.shields.io/endpoint?url=https://codspeed.io/badge.json)](https://app.codspeed.io/oxidezap/whatsapp-rust?utm_source=badge)

A high-performance, async Rust library for the WhatsApp Web API. Inspired by [whatsmeow](https://github.com/tulir/whatsmeow) (Go) and [Baileys](https://github.com/WhiskeySockets/Baileys) (TypeScript).

**[Documentation](https://whatsapp-rust.jlucaso.com)** | [llms.txt](https://whatsapp-rust.jlucaso.com/llms.txt) | [llms-full.txt](https://whatsapp-rust.jlucaso.com/llms-full.txt)

## Features

- **Authentication** — QR code pairing, pair code linking, persistent sessions
- **Messaging** — E2E encrypted (Signal Protocol), 1-on-1 and group chats, editing, reactions, quoting, receipts
- **Media** — Upload/download images, videos, documents, GIFs, audio with automatic encryption
- **Groups & Communities** — Create, manage, invite, membership approval, subgroup linking
- **Newsletters** — Create, join, send messages, reactions
- **Status** — Text, image, and video status posts with privacy controls
- **Contacts** — Phone number lookup, profile pictures, user info, business profiles
- **Presence & Chat State** — Online/offline, typing indicators, blocking
- **Chat Actions** — Archive, pin, mute, star messages
- **Profile** — Set push name, status text, profile picture
- **Privacy** — Fetch/set privacy settings, disappearing messages
- **Modular** — Pluggable storage, transport, HTTP client, and async runtime; SQLite, Tokio WebSocket, and ureq ship as the defaults, swap any of them with `default-features = false`
- **Runtime agnostic** — Bring your own async runtime via the `Runtime` trait (Tokio included by default)

For the full API reference and guides, see the **[documentation](https://whatsapp-rust.jlucaso.com)**.

## Quick Start

```toml
[dependencies]
whatsapp-rust = "0.6"
tokio = { version = "1", features = ["macros", "rt-multi-thread", "signal"] }
```

```rust,no_run
use whatsapp_rust::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bot = Bot::builder()
        .with_backend(SqliteStore::new("whatsapp.db").await?)
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

    // Runs until logout or shutdown; a single await.
    bot.run().await;
    Ok(())
}
```

The default cargo features wire up the Tokio WebSocket transport, the ureq HTTP client, the SQLite store, and the Tokio runtime; only the storage backend has to be chosen explicitly. Every piece is replaceable through the builder (`with_transport_factory`, `with_http_client`, `with_runtime`) for custom environments such as wasm or embedded targets.

### One dependency is enough

`whatsapp-rust` re-exports the whole stack, so you never need to declare the sibling crates (`wacore`, `wacore-binary`, `waproto`, `whatsapp-rust-tokio-transport`, `whatsapp-rust-ureq-http-client`, `whatsapp-rust-sqlite-storage`) yourself, including when pinning a git revision:

```toml
[dependencies]
whatsapp-rust = { git = "https://github.com/oxidezap/whatsapp-rust", rev = "<commit>" }
```

- Protobuf types: `whatsapp_rust::waproto::whatsapp` (aliased as `wa` in the prelude)
- Core protocol/types: `whatsapp_rust::wacore`, `whatsapp_rust::wacore_binary` (`Jid` is also at the crate root)
- Bundled implementations: `whatsapp_rust::transport::TokioWebSocketTransportFactory`, `whatsapp_rust::http::UreqHttpClient`, `whatsapp_rust::store::SqliteStore`, each behind its default-on cargo feature (`tokio-transport`, `ureq-client`, `sqlite-storage`)

With `default-features = false`, pick only what you need (e.g. `features = ["tokio-runtime", "tokio-transport", "ureq-client"]` for a custom store while keeping the bundled networking).

To run the bot in the background instead of blocking, use `spawn()` and keep the handle:

```rust,no_run
use whatsapp_rust::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let bot = Bot::builder()
        .with_backend(SqliteStore::new("whatsapp.db").await?)
        .build()
        .await?;

    let handle = bot.spawn(); // full Client API stays available via handle.client()

    tokio::signal::ctrl_c().await?;
    handle.shutdown().await; // graceful: flushes pending state, then stops
    Ok(())
}
```

Run the included demo bot:

```bash
cargo run                              # QR code only
cargo run -- -p 15551234567            # Pair code + QR code
cargo run -- -p 15551234567 -c MYCODE  # Custom pair code
```

## Project Structure

```text
whatsapp-rust/
├── src/                    # Main client library
├── wacore/                 # Platform-agnostic core (no runtime deps)
│   ├── binary/             # WhatsApp binary protocol
│   ├── libsignal/          # Signal Protocol implementation
│   └── appstate/           # App state management
├── waproto/                # Protocol Buffers definitions
├── storages/sqlite-storage # SQLite backend
├── transports/tokio-transport
└── http_clients/ureq-client
```

## Disclaimer

This is an unofficial, open-source reimplementation. Using custom WhatsApp clients may violate Meta's Terms of Service and could result in account suspension. Use at your own risk.

## Acknowledgements

- [whatsmeow](https://github.com/tulir/whatsmeow) (Go)
- [Baileys](https://github.com/WhiskeySockets/Baileys) (TypeScript)

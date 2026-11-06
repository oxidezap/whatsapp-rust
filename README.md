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
- Audio and video calls, both 1:1 and group, plus call links and screen sharing. The optional `voip` feature provides native relay transport and MLOW/Opus audio. Video takes application-supplied H.264 frames ([media API and codec options](https://github.com/oxidezap/whatsapp-rust-docs)).
- Replaceable storage, transport, HTTP client, and runtime. The default features include SQLite, Tokio WebSocket, ureq, and Tokio. Applications still choose a storage backend. Native plugins are [opt-in](agent_docs/architecture.md).

The [support qualification matrix](agent_docs/support.md) records maintained target, feature, runtime, and storage checks. Windows/macOS are unqualified and ESP32 remains experimental; a Linux build does not establish support on those platforms.

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


## 🚀 Enhanced Bot Engine & Baileys / WhatsMeow Parity

This fork by [@KazeDevID](https://github.com/KazeDevID/whatsapp-rust) introduces high-level bot abstractions inspired by [WhiskeySockets/Baileys](https://github.com/WhiskeySockets/Baileys) and [tulir/whatsmeow](https://github.com/tulir/whatsmeow):

- **Fluent `BotRouter`:** Register prefix commands (`!`, `/`, `.`), multi-command aliases, and default fallbacks without boilerplate.
- **Convenient `MessageContext`:**
  - `ctx.text()`: Extracted text from conversation, extended text, or media captions.
  - `ctx.reply("...")` / `ctx.reply_quoting("...")`: Send replies with automatic context quoting.
  - `ctx.react("👍")`: Instant emoji reactions.
  - `ctx.send_poll("Question", &options, 1)`: Multi-device interactive WhatsApp polls.
  - `ctx.send_typing()` / `ctx.send_recording()` / `ctx.send_paused()`: Native chatstate presence indicators.
  - `ctx.mark_as_read()`: Automated read receipt delivery (blue checkmarks).
  - `ctx.is_group()` / `ctx.sender()` / `ctx.chat()`: Clean addressing helpers.

### Baileys / WhatsMeow to whatsapp-rust Comparison

| Feature | Baileys (Node.js) | WhatsMeow (Go) | whatsapp-rust (Rust) |
| :--- | :--- | :--- | :--- |
| **Send Text** | `sock.sendMessage(jid, { text: "hi" })` | `cli.SendMessage(ctx, jid, &waProto.Message{...})` | `ctx.reply("hi").await?` |
| **Reaction** | `sock.sendMessage(jid, { react: { text: "❤️", key } })` | `cli.SendMessage(ctx, jid, cli.BuildReaction(...))` | `ctx.react("❤️").await?` |
| **Polls** | `sock.sendMessage(jid, { poll: { name, values } })` | `cli.SendMessage(ctx, jid, cli.BuildPollCreation(...))` | `ctx.send_poll(name, &options, 1).await?` |
| **Typing** | `sock.sendPresenceUpdate("composing", jid)` | `cli.SendChatPresence(jid, PresenceComposing)` | `ctx.send_typing().await?` |
| **Read Receipt**| `sock.readMessages([msg.key])` | `cli.MarkRead([]types.MessageID{id}, ...)` | `ctx.mark_as_read().await?` |
| **Performance** | V8 JavaScript Event Loop (~150MB RAM) | Goroutines (~45MB RAM) | Zero-copy `buffa` + Tokio (~8MB RAM) |

### Fluent Bot Example

```rust,no_run
use whatsapp_rust::prelude::*;
use whatsapp_rust::storages::SqliteStore;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let backend = SqliteStore::open("whatsapp.db").await?;
    let mut router = BotRouter::new();

    router.prefixes(&["!", "/", "."]);
    router.command("ping", |cmd| async move {
        cmd.mark_as_read().await?;
        cmd.react("🏓").await?;
        cmd.reply("Pong! 🦀 Powered by Rust & KazeDevID").await?;
        Ok(())
    });

    router.command("poll", |cmd| async move {
        let opts = vec!["Rust 🦀".into(), "Go 🐹".into(), "Node 🟩".into()];
        cmd.send_poll("Best language for WhatsApp bots?", &opts, 1).await?;
        Ok(())
    });

    let bot = Bot::builder()
        .with_backend(backend)
        .with_router(router)
        .build()
        .await?;

    bot.run().await;
    Ok(())
}
```

Run the complete bot example:
```bash
cargo run --example ultimate_bot
```

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

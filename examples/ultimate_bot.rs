//! # Ultimate WhatsApp Bot Example
//!
//! A production-grade WhatsApp bot in Rust powered by `whatsapp-rust`.
//! Features:
//! - Multi-device authentication (QR code or 8-digit Pairing Code)
//! - Baileys & WhatsMeow parity fluent API
//! - High-level [`BotRouter`] with prefix handling, argument parsing, and aliases
//! - Admin-only guards (`router.admin_command`) and group-only guards
//! - Group management (`group_metadata`, `group_kick`, `group_invite_code`)
//! - Tagall / Hidetag group announcements (`mention_all`)
//! - Automated blue tick read receipts (`mark_as_read`)
//! - Instant emoji reactions (`react`)
//! - Interactive WhatsApp polls (`send_poll`)
//! - Location and Contact cards (`reply_location`, `reply_contact`)
//! - Chat presence indicators (`send_typing`)
//! - Persistent session storage in SQLite
//!
//! ## Run with QR code:
//! ```bash
//! cargo run --example ultimate_bot
//! ```
//!
//! ## Run with 8-digit Phone Pairing Code:
//! ```bash
//! cargo run --example ultimate_bot -- --phone 628123456789
//! ```

use log::{error, info};
use std::time::Instant;
use whatsapp_rust::pair_code::PairCodeOptions;
use whatsapp_rust::prelude::*;
use whatsapp_rust::storages::SqliteStore;

fn parse_arg(args: &[String], long: &str, short: &str) -> Option<String> {
    for i in 0..args.len() {
        if args[i] == long || args[i] == short {
            return args.get(i + 1).cloned();
        }
        if let Some(val) = args[i].strip_prefix(&format!("{long}=")) {
            return Some(val.to_string());
        }
    }
    None
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format(|buf, record| {
            use std::io::Write;
            writeln!(
                buf,
                "{} [{:<5}] [{}] {}",
                wacore::time::now_utc().format("%H:%M:%S"),
                record.level(),
                record.target(),
                record.args()
            )
        })
        .init();

    let args: Vec<String> = std::env::args().collect();
    let phone_number = parse_arg(&args, "--phone", "-p");
    let start_time = Instant::now();

    info!("Starting WhatsApp Rust Bot (Baileys & WhatsMeow Parity Engine)...");

    // Initialize persistent SQLite session database
    let backend = SqliteStore::open("whatsapp_bot.db").await?;

    // Create high-level command router
    let mut router = BotRouter::new();
    router.prefixes(&["!", "/", "."]);
    router.ignore_self(true);

    // Command: !ping
    router.command_with_aliases("ping", &["p"], |cmd| async move {
        let _ = cmd.mark_as_read().await;
        cmd.react("🏓").await?;
        cmd.reply("Pong! 🦀 High-performance WhatsApp Bot in Rust is alive and blazing fast!").await?;
        Ok(())
    });

    // Command: !echo <message>
    router.command("echo", |cmd| async move {
        let _ = cmd.mark_as_read().await;
        if cmd.args.is_empty() {
            cmd.reply("Usage: !echo <your message here>").await?;
        } else {
            cmd.reply(format!("📢 Echo: {}", cmd.raw_args)).await?;
        }
        Ok(())
    });

    // Command: !poll <question>
    router.command("poll", |cmd| async move {
        let _ = cmd.mark_as_read().await;
        let question = if cmd.raw_args.is_empty() {
            "Which WhatsApp client library do you prefer?".to_string()
        } else {
            cmd.raw_args.clone()
        };

        let options = vec![
            "whatsapp-rust (Blazingly Fast & Safe) 🦀".to_string(),
            "tulir/whatsmeow (Go) 🐹".to_string(),
            "whiskeysockets/Baileys (Node.js) 🟩".to_string(),
        ];

        cmd.react("📊").await?;
        cmd.send_poll(&question, &options, 1).await?;
        Ok(())
    });

    // Command: !typing
    router.command("typing", |cmd| async move {
        let _ = cmd.mark_as_read().await;
        cmd.send_typing().await?;
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        cmd.send_paused().await?;
        cmd.reply("Done typing simulation! ✨").await?;
        Ok(())
    });

    // Command: !tagall / !hidetag (Admin only in groups)
    router.admin_command("tagall", |cmd| async move {
        let _ = cmd.mark_as_read().await;
        cmd.react("📢").await?;
        let header = if cmd.raw_args.is_empty() {
            "📢 *Pengumuman Untuk Semua Anggota:*"
        } else {
            &cmd.raw_args
        };
        cmd.mention_all(Some(header)).await?;
        Ok(())
    });

    // Command: !link (Get group invite link)
    router.admin_command("link", |cmd| async move {
        let _ = cmd.mark_as_read().await;
        cmd.react("🔗").await?;
        match cmd.group_invite_code().await {
            Ok(code) => {
                cmd.reply(format!("🔗 *Tautan Undangan Grup:*\nhttps://chat.whatsapp.com/{code}")).await?;
            }
            Err(e) => {
                cmd.reply(format!("Gagal mengambil tautan grup: {e}")).await?;
            }
        }
        Ok(())
    });

    // Command: !admins (List group admins)
    router.group_command("admins", |cmd| async move {
        let _ = cmd.mark_as_read().await;
        cmd.react("🛡️").await?;
        let meta = cmd.group_metadata().await?;
        let admins: Vec<_> = meta.participants.iter().filter(|p| p.is_admin()).collect();
        let mut msg = format!("🛡️ *Daftar Admin Grup ({})*:\n\n", admins.len());
        for (i, adm) in admins.iter().enumerate() {
            msg.push_str(&format!("{}. @{}\n", i + 1, adm.jid.user));
        }
        let admin_jids: Vec<_> = admins.iter().map(|p| p.jid.clone()).collect();
        cmd.reply_with_mentions(msg, &admin_jids).await?;
        Ok(())
    });

    // Command: !location (Send demo pin)
    router.command("location", |cmd| async move {
        let _ = cmd.mark_as_read().await;
        cmd.react("📍").await?;
        cmd.reply_location(
            -6.1753924,
            106.8271528,
            Some("Monas (Monumen Nasional)".into()),
            Some("Jakarta Pusat, DKI Jakarta, Indonesia".into()),
        ).await?;
        Ok(())
    });

    // Command: !contact (Send bot developer vCard)
    router.command("contact", |cmd| async move {
        let _ = cmd.mark_as_read().await;
        cmd.react("👤").await?;
        let vcard = "BEGIN:VCARD\nVERSION:3.0\nFN:KazeDevID\nORG:whatsapp-rust Developer\nTEL;type=CELL;type=VOICE;waid=628123456789:+62 812-3456-789\nEND:VCARD";
        cmd.reply_contact("KazeDevID (Bot Creator)", vcard).await?;
        Ok(())
    });

    // Command: !info
    router.command_with_aliases("info", &["about", "bot"], move |cmd| {
        let uptime_secs = start_time.elapsed().as_secs();
        async move {
            let _ = cmd.mark_as_read().await;
            cmd.react("ℹ️").await?;
            let is_group = if cmd.is_group() { "Yes (Group)" } else { "No (Direct Message)" };
            let sender = cmd.sender().to_string();
            let chat = cmd.chat().to_string();

            let message = format!(
                "🤖 *WhatsApp-Rust Bot Information*\n\n\
                • *Runtime:* Pure Rust (tokio + buffa zero-copy proto)\n\
                • *Chat ID:* `{chat}`\n\
                • *Sender ID:* `{sender}`\n\
                • *Group Context:* {is_group}\n\
                • *Uptime:* {uptime_secs}s\n\
                • *Multi-Device:* Enabled (Signal E2EE + Curve25519)\n\
                • *Engine:* Forked & Enhanced by @KazeDevID"
            );
            cmd.reply(message).await?;
            Ok(())
        }
    });

    // Command: !help
    router.command_with_aliases("help", &["menu", "commands"], |cmd| async move {
        let _ = cmd.mark_as_read().await;
        cmd.react("📜").await?;
        let help_text = format!(
            "🤖 *Available Bot Commands* (Prefix: `{}`)\n\n\
            *General Commands:*\n\
            • `{0}ping` — Check bot health and latency\n\
            • `{0}echo <text>` — Echo back message\n\
            • `{0}poll [title]` — Create an interactive WhatsApp poll\n\
            • `{0}typing` — Demonstrate composing presence state\n\
            • `{0}location` — Send sample location pin message\n\
            • `{0}contact` — Send contact card (vCard)\n\
            • `{0}info` — Display session and JID diagnostic info\n\
            • `{0}help` — Show this commands menu\n\n\
            *Group & Admin Commands:*\n\
            • `{0}tagall [pesan]` — Tag all members (Admin only)\n\
            • `{0}link` — Fetch group invite link (Admin only)\n\
            • `{0}admins` — List all administrators in this group\n\n\
            _Powered by whatsapp-rust & KazeDevID_",
            cmd.prefix
        );
        cmd.reply(help_text).await?;
        Ok(())
    });

    // Fallback when a message begins with a prefix but command is unknown
    router.default(|cmd| async move {
        cmd.react("❓").await?;
        cmd.reply(format!(
            "Unknown command `{}{}`. Type `{}help` for available commands.",
            cmd.prefix, cmd.command, cmd.prefix
        )).await?;
        Ok(())
    });

    // Configure Bot Builder
    let mut builder = Bot::builder()
        .with_backend(backend)
        .with_router(router);

    // Pair code support if phone number provided
    if let Some(phone) = phone_number {
        info!("Pair code requested for phone: {phone}");
        builder = builder
            .with_pair_code(PairCodeOptions::new(phone))
            .on_pair_code(|code, timeout| async move {
                println!("\n==========================================");
                println!("  WhatsApp 8-Digit Pairing Code:  {code}");
                println!("  Valid for: {} seconds", timeout.as_secs());
                println!("==========================================\n");
            });
    } else {
        // Fallback to QR code terminal printing
        builder = builder.on_qr_code(|qr_string, timeout| async move {
            println!("\n--- Scan WhatsApp QR Code --- (Valid for {}s)", timeout.as_secs());
            println!("QR String: {qr_string}");
            println!("-----------------------------\n");
        });
    }

    builder = builder.on_connected(|client| async move {
        info!("Successfully connected and authenticated with WhatsApp Multi-Device servers!");
        if let Some(jid) = client.pn() {
            info!("Connected as PN: {jid}");
        }
    });

    let bot = builder.build().await?;
    info!("Bot is starting run loop. Press Ctrl+C to stop.");
    bot.run().await;

    Ok(())
}

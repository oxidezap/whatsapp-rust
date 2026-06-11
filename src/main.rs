// See the matching note in lib.rs: instrumented large async fns need a deeper
// recursion limit when the `tracing` + `tracing-pii` paths combine.
#![recursion_limit = "512"]

use log::{error, info};
use whatsapp_rust::pair_code::PairCodeOptions;
use whatsapp_rust::prelude::*;

const PING_TRIGGER: &str = "🦀ping";
const PONG_TEXT: &str = "🏓 Pong!";
const REACTION_EMOJI: &str = "🏓";

// Usage:
//   cargo run                                      # QR code pairing only
//   cargo run -- --phone 15551234567               # Pair code + QR code (concurrent)
//   cargo run -- -p 15551234567                    # Short form
//   cargo run -- -p 15551234567 --code MYCODE12    # Custom 8-char pair code
//   cargo run -- -p 15551234567 -c MYCODE12        # Short form
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let phone_number = parse_arg(&args, "--phone", "-p");
    let custom_code = parse_arg(&args, "--code", "-c");

    if let Some(ref phone) = phone_number {
        eprintln!("Phone number provided: {}", phone);
        if let Some(ref code) = custom_code {
            eprintln!("Custom pair code: {}", code);
        }
        eprintln!("Will use pair code authentication (concurrent with QR)");
    }
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .format(|buf, record| {
            use std::io::Write;
            writeln!(
                buf,
                "{} [{:<5}] [{}] - {}",
                wacore::time::now_utc().format("%H:%M:%S"),
                record.level(),
                record.target(),
                record.args()
            )
        })
        .init();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("Failed to build tokio runtime");

    rt.block_on(async {
        let store = match SqliteStore::new("whatsapp.db").await {
            Ok(store) => store,
            Err(e) => {
                error!("Failed to create SQLite backend: {}", e);
                return;
            }
        };
        info!("SQLite backend initialized successfully.");

        // Transport, HTTP client and runtime fall back to the defaults shipped
        // with the default cargo features (Tokio WebSocket, ureq, Tokio).
        let mut builder = Bot::builder()
            .with_backend(store)
            .on_qr_code(|code, timeout| async move {
                info!("----------------------------------------");
                info!(
                    "QR code received (valid for {} seconds):",
                    timeout.as_secs()
                );
                info!("\n{}\n", code);
                info!("----------------------------------------");
            })
            .on_pair_code(|code, timeout| async move {
                info!("========================================");
                info!("PAIR CODE (valid for {} seconds):", timeout.as_secs());
                info!("Enter this code on your phone:");
                info!("WhatsApp > Linked Devices > Link a Device");
                info!("> Link with phone number instead");
                info!("");
                info!("    >>> {} <<<", code);
                info!("");
                info!("========================================");
            })
            .on_connected(|_client| async {
                info!("✅ Bot connected successfully!");
            })
            .on_logged_out(|_info| async {
                error!("❌ Bot was logged out!");
            })
            .on_message(|ctx| async move {
                if let Some(reply) = build_media_pong(&ctx.message) {
                    info!("Received media ping from {}", ctx.info.source.sender);
                    if let Err(e) = ctx.send_message(reply).await {
                        error!("Failed to send media pong: {}", e);
                    }
                } else if ctx.message.text_content() == Some(PING_TRIGGER) {
                    handle_text_ping(&ctx).await;
                }
            });

        if let Some(phone) = phone_number {
            builder = builder.with_pair_code(PairCodeOptions {
                phone_number: phone,
                custom_code,
                ..Default::default()
            });
        }

        let bot = match builder.build().await {
            Ok(bot) => bot,
            Err(e) => {
                error!("Failed to build bot: {}", e);
                return;
            }
        };

        #[cfg(feature = "signal")]
        {
            let mut handle = bot.spawn();
            tokio::select! {
                _ = &mut handle => {}
                _ = tokio::signal::ctrl_c() => {
                    info!("Received Ctrl+C, shutting down...");
                    handle.shutdown().await;
                }
            }
        }

        #[cfg(not(feature = "signal"))]
        bot.run().await;
    });
}

async fn handle_text_ping(ctx: &MessageContext) {
    info!("Received text ping, sending pong...");

    if let Err(e) = ctx.react(REACTION_EMOJI).await {
        error!("Failed to send reaction: {}", e);
    }

    let start = wacore::time::Instant::now();
    let sent = match ctx.reply_quoting(PONG_TEXT).await {
        Ok(r) => r,
        Err(e) => {
            error!("Failed to send pong: {}", e);
            return;
        }
    };

    let duration = format!("{:.2?}", start.elapsed());
    info!(
        "Send took {}. Editing message {}...",
        duration, &sent.message_id
    );

    let edit = wa::Message {
        extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
            text: Some(format!("{PONG_TEXT}\n`{duration}`")),
            ..Default::default()
        })),
        ..Default::default()
    };
    if let Err(e) = ctx.edit_message(sent.message_id.clone(), edit).await {
        error!("Failed to edit message {}: {}", sent.message_id, e);
    }
}

/// Reuses the original CDN blob, only swaps the caption. Instant regardless of file size.
fn build_media_pong(message: &wa::Message) -> Option<wa::Message> {
    let base = message.get_base_message();

    if let Some(img) = &base.image_message
        && img.caption.as_deref() == Some(PING_TRIGGER)
    {
        return Some(wa::Message {
            image_message: Some(Box::new(wa::message::ImageMessage {
                caption: Some(PONG_TEXT.to_string()),
                ..*img.clone()
            })),
            ..Default::default()
        });
    }
    if let Some(vid) = &base.video_message
        && vid.caption.as_deref() == Some(PING_TRIGGER)
    {
        return Some(wa::Message {
            video_message: Some(Box::new(wa::message::VideoMessage {
                caption: Some(PONG_TEXT.to_string()),
                ..*vid.clone()
            })),
            ..Default::default()
        });
    }
    None
}

fn parse_arg(args: &[String], long: &str, short: &str) -> Option<String> {
    let long_prefix = format!("{}=", long);
    let mut iter = args.iter().skip(1);
    while let Some(arg) = iter.next() {
        if arg == long || arg == short {
            return iter.next().cloned();
        }
        if let Some(value) = arg.strip_prefix(&long_prefix) {
            return Some(value.to_string());
        }
    }
    None
}

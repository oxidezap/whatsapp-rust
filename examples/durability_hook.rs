//! Inbound durability hook: at-least-once delivery.
//!
//! By default the client acknowledges a message to the server as soon as it is
//! decrypted (at-most-once): if the process crashes or your storage write fails
//! before you persist the message, it is lost — the server already considers it
//! delivered and never resends it.
//!
//! Registering an [`InboundDurabilityHook`] defers that acknowledgement until
//! your hook durably commits the message. On success the message is acked; on
//! failure (or a crash) it stays in the server's offline queue and is
//! redelivered on the next connect, where the hook runs again. This is
//! at-least-once: the hook MUST be idempotent — deduplicate by
//! `(info.source.chat, info.source.sender, info.id)`, since stanza ids are only
//! unique within a chat/sender.
//!
//!   cargo run --example durability_hook

use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::Context;
use log::{error, info};
use whatsapp_rust::InboundDurabilityHook;
use whatsapp_rust::prelude::*;

/// Idempotency key: stanza ids are only unique within a `(chat, sender)`.
type CommitKey = (String, String, String);

/// A hook that durably appends each message to a file (with fsync) before
/// returning `Ok`, deduplicating by `(chat, sender, id)`. A real implementation
/// would INSERT into a database or enqueue to a broker; the important property is
/// that the commit is durable BEFORE the hook returns `Ok` (which lets the SDK
/// ack), and that the hook returns `Err` when the commit genuinely failed.
struct InboxArchiver {
    // `Arc` so the file can be moved into `spawn_blocking` (the write/fsync must
    // not run on the async receive thread).
    file: Arc<Mutex<File>>,
    /// Keys already committed, for idempotency. Seeded from the archive on
    /// startup so dedupe survives a restart; a real store would dedupe with a
    /// unique constraint on `(chat, sender, id)` instead.
    seen: Mutex<HashSet<CommitKey>>,
}

impl InboxArchiver {
    async fn open(path: &str) -> anyhow::Result<Self> {
        let path = path.to_string();
        // Seed the dedupe set and open the file off the async runtime thread so a
        // large archive does not stall boot.
        let (file, seen) = tokio::task::spawn_blocking(move || -> anyhow::Result<_> {
            let mut seen = HashSet::new();
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    for line in content.split_inclusive('\n') {
                        // Only seed from complete records: a record is durable
                        // once its terminating newline is on disk. A torn trailing
                        // line (crash mid-append) lacks it, so skip it — otherwise
                        // a never-committed message would be dropped as a dup.
                        if !line.ends_with('\n') {
                            continue;
                        }
                        let mut parts = line.trim_end_matches('\n').splitn(4, '\t');
                        if let (Some(c), Some(s), Some(i)) =
                            (parts.next(), parts.next(), parts.next())
                        {
                            seen.insert((c.to_string(), s.to_string(), i.to_string()));
                        }
                    }
                }
                // A missing archive is a fresh start; any other error is real and
                // must not silently empty the dedupe set.
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => {
                    return Err(anyhow::Error::from(e).context(format!("reading archive {path}")));
                }
            }
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .with_context(|| format!("opening archive file {path}"))?;
            Ok((file, seen))
        })
        .await
        .map_err(|e| anyhow::anyhow!("archive open task failed: {e}"))??;

        Ok(Self {
            file: Arc::new(Mutex::new(file)),
            seen: Mutex::new(seen),
        })
    }
}

#[async_trait::async_trait]
impl InboundDurabilityHook for InboxArchiver {
    async fn on_messages(
        &self,
        _client: Arc<Client>,
        batch: &[InboundMessage],
    ) -> anyhow::Result<()> {
        // Live traffic arrives one message at a time; an offline drain hands
        // over a whole batch. Either way the commit below is a single append +
        // fsync, so the durability cost amortizes over the batch.
        let mut lines = String::new();
        let mut keys: Vec<CommitKey> = Vec::with_capacity(batch.len());
        {
            // Idempotency: a redelivery (or a replay after a crash between
            // commit and ack) can hand us the same keys more than once. Check,
            // but only record them as committed AFTER the durable write below
            // succeeds.
            let seen = self
                .seen
                .lock()
                .map_err(|_| anyhow::anyhow!("seen lock poisoned"))?;
            for m in batch {
                let key: CommitKey = (
                    m.info.source.chat.to_string(),
                    m.info.source.sender.to_string(),
                    m.info.id.clone(),
                );
                // Dedup against the archive AND earlier entries of this same
                // batch, so one fsync can never append a key twice.
                if seen.contains(&key) || keys.contains(&key) {
                    info!("[{}] already committed, skipping (dedup)", m.info.id);
                    continue;
                }
                // Sanitize so the tab-delimited archive stays parseable on restart.
                let preview = m
                    .message
                    .conversation
                    .as_deref()
                    .unwrap_or("<non-text>")
                    .replace(['\t', '\n'], " ");
                lines.push_str(&format!("{}\t{}\t{}\t{preview}\n", key.0, key.1, key.2));
                keys.push(key);
            }
        }

        if !keys.is_empty() {
            // Durable commit on a blocking thread: append then fsync — all-or-
            // nothing for the batch. Returning Ok only after sync_all means
            // "safe to ack every message"; any error returns Err, so the acks
            // are suppressed and the server redelivers the batch later. The
            // hook is awaited on the receive path, so disk I/O goes to
            // spawn_blocking.
            let file = Arc::clone(&self.file);
            tokio::task::spawn_blocking(move || -> std::io::Result<()> {
                let mut file = file.lock().expect("file lock poisoned");
                file.write_all(lines.as_bytes())?;
                file.sync_all()
            })
            .await
            .map_err(|e| anyhow::anyhow!("archive write task failed: {e}"))??;

            let mut seen = self
                .seen
                .lock()
                .map_err(|_| anyhow::anyhow!("seen lock poisoned"))?;
            let count = keys.len();
            for key in keys {
                seen.insert(key);
            }
            info!("committed {count} message(s) durably in one fsync");
        }
        Ok(())
    }
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");

    rt.block_on(async {
        let store = match SqliteStore::new("whatsapp.db").await {
            Ok(store) => store,
            Err(e) => {
                error!("failed to create SQLite backend: {e}");
                return;
            }
        };

        let archiver = match InboxArchiver::open("inbox.jsonl").await {
            Ok(archiver) => archiver,
            Err(e) => {
                error!("failed to open archive file: {e}");
                return;
            }
        };

        let bot = Bot::builder()
            .with_backend(store)
            // Opt in to at-least-once delivery. Without this call the client
            // keeps its default at-most-once behavior.
            .with_inbound_durability_hook(archiver)
            .on_qr_code(|code, _timeout| async move {
                info!("scan to pair:\n{code}");
            })
            .on_connected(|_client| async {
                info!("connected; inbound messages are now committed before ack");
            })
            .build()
            .await
            .expect("failed to build bot");

        bot.run().await;
    });
}

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
//! at-least-once: the hook MUST be idempotent — deduplicate by `info.id`.
//!
//!   cargo run --example durability_hook

use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::Context;
use log::{error, info};
use whatsapp_rust::InboundDurabilityHook;
use whatsapp_rust::prelude::*;

/// A hook that durably appends each message to a file (with fsync) before
/// returning `Ok`, deduplicating by message id. A real implementation would
/// INSERT into a database or enqueue to a broker; the important property is that
/// the commit is durable BEFORE the hook returns `Ok` (which lets the SDK ack),
/// and that the hook returns `Err` when the commit genuinely failed.
struct InboxArchiver {
    // `Arc` so the file can be moved into `spawn_blocking` (the write/fsync must
    // not run on the async receive thread).
    file: Arc<Mutex<File>>,
    /// Message ids already committed, for idempotency. Seeded from the archive on
    /// startup so dedupe survives a restart; a real store would dedupe with a
    /// unique constraint on the id column instead.
    seen: Mutex<HashSet<String>>,
}

impl InboxArchiver {
    fn open(path: &str) -> anyhow::Result<Self> {
        // Rebuild the dedupe set from already-committed ids so a replay after a
        // restart does not append the same message twice.
        let mut seen = HashSet::new();
        if let Ok(existing) = File::open(path) {
            for line in BufReader::new(existing).lines() {
                let line = line?;
                if let Some((id, _)) = line.split_once('\t') {
                    seen.insert(id.to_string());
                }
            }
        }
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .with_context(|| format!("opening archive file {path}"))?;
        Ok(Self {
            file: Arc::new(Mutex::new(file)),
            seen: Mutex::new(seen),
        })
    }
}

#[async_trait::async_trait]
impl InboundDurabilityHook for InboxArchiver {
    async fn on_message(
        &self,
        _client: Arc<Client>,
        info: &MessageInfo,
        message: &wa::Message,
    ) -> anyhow::Result<()> {
        // Idempotency: a redelivery (or a replay after a crash between commit and
        // ack) can hand us the same message id more than once. Check, but only
        // record it as committed AFTER the durable write below succeeds.
        if self
            .seen
            .lock()
            .map_err(|_| anyhow::anyhow!("seen lock poisoned"))?
            .contains(&info.id)
        {
            info!("[{}] already committed, skipping (dedup)", info.id);
            return Ok(());
        }

        let preview = message.conversation.as_deref().unwrap_or("<non-text>");
        let line = format!("{}\t{preview}\n", info.id);

        // Durable commit on a blocking thread: append then fsync. Returning Ok
        // only after sync_all means "safe to ack"; any error returns Err, so the
        // ack is suppressed and the server redelivers the message later. The hook
        // is awaited on the receive path, so the disk I/O goes to spawn_blocking.
        let file = Arc::clone(&self.file);
        tokio::task::spawn_blocking(move || -> std::io::Result<()> {
            let mut file = file.lock().expect("file lock poisoned");
            file.write_all(line.as_bytes())?;
            file.sync_all()
        })
        .await
        .map_err(|e| anyhow::anyhow!("archive write task failed: {e}"))??;

        self.seen
            .lock()
            .map_err(|_| anyhow::anyhow!("seen lock poisoned"))?
            .insert(info.id.clone());
        info!("[{}] committed durably: {preview}", info.id);
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

        let archiver = match InboxArchiver::open("inbox.jsonl") {
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

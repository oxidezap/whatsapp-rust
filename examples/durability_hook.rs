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
use std::sync::Arc;
use std::sync::Mutex;

use log::{error, info};
use whatsapp_rust::InboundDurabilityHook;
use whatsapp_rust::prelude::*;

/// A toy hook that "commits" each message into an in-memory set, deduplicating
/// by message id. A real implementation would INSERT into a database or enqueue
/// to a broker, and return `Err` only when that commit genuinely failed.
#[derive(Default)]
struct InboxArchiver {
    seen: Mutex<HashSet<String>>,
}

#[async_trait::async_trait]
impl InboundDurabilityHook for InboxArchiver {
    async fn on_message(
        &self,
        _client: Arc<Client>,
        info: &MessageInfo,
        message: &wa::Message,
    ) -> anyhow::Result<()> {
        // Idempotency: a redelivery (or a replay after a crash between commit
        // and ack) can hand us the same message id more than once.
        let first_time = self.seen.lock().unwrap().insert(info.id.clone());
        if !first_time {
            info!("[{}] already committed, skipping (dedup)", info.id);
            return Ok(());
        }

        // Durably persist here. Returning Ok means "safe to ack"; returning Err
        // suppresses the ack so the server redelivers the message later.
        let preview = message.conversation.as_deref().unwrap_or("<non-text>");
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

        let bot = Bot::builder()
            .with_backend(store)
            // Opt in to at-least-once delivery. Without this call the client
            // keeps its default at-most-once behavior.
            .with_inbound_durability_hook(InboxArchiver::default())
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

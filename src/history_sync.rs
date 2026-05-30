use crate::types::events::{Event, LazyHistorySync};
use prost::Message as ProtoMessage;
use std::sync::Arc;
use wacore::history_sync::{TcTokenCandidate, process_history_sync};
use wacore::store::traits::TcTokenEntry;
use wacore_binary::{Jid, JidExt as _};
use waproto::whatsapp::{self as wa, message::HistorySyncNotification};

use crate::client::Client;

impl Client {
    pub(crate) async fn handle_history_sync(
        self: &Arc<Self>,
        message_id: String,
        notification: HistorySyncNotification,
    ) {
        if self.is_shutting_down() {
            log::debug!(
                "Dropping history sync {} during shutdown (Type: {:?})",
                message_id,
                notification.sync_type()
            );
            return;
        }

        if self.skip_history_sync_enabled() {
            log::debug!(
                "Skipping history sync for message {} (Type: {:?})",
                message_id,
                notification.sync_type()
            );
            // Send receipt so the phone considers this chunk delivered and stops
            // retrying. This intentionally diverges from WhatsApp Web's AB prop
            // drop path (which sends no receipt) because bots will never process
            // history, and without the receipt the phone would keep re-uploading
            // blobs that will never be consumed.
            self.send_protocol_receipt(
                message_id,
                crate::types::presence::ReceiptType::HistorySync,
            )
            .await;
            return;
        }

        // Enqueue a MajorSyncTask for the dedicated sync worker to consume.
        self.begin_history_sync_task();
        let task = crate::sync_task::MajorSyncTask::HistorySync {
            message_id,
            notification: Box::new(notification),
        };
        if let Err(e) = self.major_sync_task_sender.send(task).await {
            self.finish_history_sync_task();
            if self.is_shutting_down() {
                log::debug!("Dropping history sync task during shutdown: {e}");
            } else {
                log::error!("Failed to enqueue history sync task: {e}");
            }
        }
    }

    /// Process history sync: decompress, extract internal data (tctokens,
    /// pushname, nct_salt), then dispatch a single `Event::HistorySync`
    /// with the full decompressed blob for on-demand consumer decoding.
    pub(crate) async fn process_history_sync_task(
        self: &Arc<Self>,
        message_id: String,
        mut notification: HistorySyncNotification,
    ) {
        if self.is_shutting_down() {
            log::debug!("Aborting history sync {} before processing", message_id);
            return;
        }

        log::info!(
            "Processing history sync for message {} (Size: {}, Type: {:?})",
            message_id,
            notification.file_length(),
            notification.sync_type()
        );

        self.send_protocol_receipt(
            message_id.clone(),
            crate::types::presence::ReceiptType::HistorySync,
        )
        .await;

        if self.is_shutting_down() {
            log::debug!(
                "Aborting history sync {} after receipt during shutdown",
                message_id
            );
            return;
        }

        // file_length is the decrypted (but still zlib-compressed) blob size, not
        // the final decompressed size. We still pass it as a hint — the decompressor
        // uses it with a 4x multiplier, which is a better estimate than guessing
        // from the encrypted size (which includes MAC/padding overhead).
        let compressed_size_hint = notification.file_length.filter(|&s| s > 0);

        // Use take() to avoid cloning large payloads - moves ownership instead
        let compressed_data = if let Some(inline_payload) =
            notification.initial_hist_bootstrap_inline_payload.take()
        {
            log::info!(
                "Found inline history sync payload ({} bytes). Using directly.",
                inline_payload.len()
            );
            inline_payload
        } else {
            log::info!("Downloading external history sync blob...");
            if self.is_shutting_down() || !self.is_connected() {
                log::debug!(
                    "Aborting history sync {} before blob download: client disconnected",
                    message_id
                );
                return;
            }
            // Stream-decrypt: reads encrypted chunks (8KB) from the network and
            // decrypts on the fly into a Vec, avoiding holding the full encrypted
            // blob in memory alongside the decrypted one.
            match self
                .download_to_writer(&notification, std::io::Cursor::new(Vec::new()))
                .await
            {
                Ok(cursor) => {
                    log::info!("Successfully downloaded history sync blob.");
                    cursor.into_inner()
                }
                Err(e) => {
                    if self.is_shutting_down() {
                        log::debug!(
                            "History sync blob download aborted during shutdown: {:?}",
                            e
                        );
                    } else {
                        log::error!("Failed to download history sync blob: {:?}", e);
                    }
                    return;
                }
            }
        };

        let own_user = {
            let device_snapshot = self.persistence_manager.get_device_snapshot().await;
            device_snapshot.pn.as_ref().map(|j| j.to_non_ad().user)
        };

        let has_listeners = self.core.event_bus.has_handlers();
        let retain_history_blob = true;

        // Small blobs (PushName, Recent): decode inline to avoid spawn_blocking overhead.
        // Large blobs: use blocking thread to avoid stalling the async runtime.
        const INLINE_THRESHOLD: usize = 256 * 1024;
        let parse_result = if compressed_data.len() < INLINE_THRESHOLD {
            Some(process_history_sync(
                compressed_data,
                own_user.as_deref(),
                retain_history_blob,
                compressed_size_hint,
            ))
        } else {
            let (result_tx, result_rx) = futures::channel::oneshot::channel();
            let blocking_fut = self.runtime.spawn_blocking(Box::new(move || {
                let result = process_history_sync(
                    compressed_data,
                    own_user.as_deref(),
                    retain_history_blob,
                    compressed_size_hint,
                );
                let _ = result_tx.send(result);
            }));
            self.runtime
                .spawn(Box::pin(async move {
                    blocking_fut.await;
                }))
                .detach();
            result_rx.await.ok()
        };

        if self.is_shutting_down() {
            log::debug!(
                "Aborting history sync {} after parse during shutdown",
                message_id
            );
            return;
        }

        match parse_result {
            Some(Ok(sync_result)) => {
                log::info!(
                    "Successfully processed HistorySync (message {message_id}); {} conversations",
                    sync_result.conversations_processed
                );

                // Update own push name if found
                if let Some(new_name) = sync_result.own_pushname {
                    log::info!("Updating own push name from history sync to '{new_name}'");
                    self.update_push_name_and_notify(new_name).await;
                }

                // Store NCT salt if found.
                // WA Web: storeNctSaltFromHistorySync in MsgHandlerAction.js
                if let Some(salt) = sync_result.nct_salt {
                    log::info!(
                        "History sync provided NCT salt ({} bytes); applying as backfill only",
                        salt.len()
                    );
                    self.persistence_manager
                        .process_command(
                            wacore::store::commands::DeviceCommand::SetNctSaltFromHistorySync(salt),
                        )
                        .await;
                }

                // Store tctokens extracted during streaming (move to avoid cloning)
                for candidate in sync_result.tc_token_candidates {
                    self.store_tc_token_candidate(candidate).await;
                }

                if let Some(decompressed) = sync_result.decompressed_bytes {
                    self.store_history_sync_msg_secrets(decompressed.as_ref())
                        .await;

                    if !has_listeners {
                        return;
                    }

                    let lazy_hs = LazyHistorySync::new(
                        decompressed,
                        notification.sync_type().into(),
                        notification.chunk_order,
                        notification.progress,
                    )
                    .with_peer_data_request_session_id(
                        notification.peer_data_request_session_id.take(),
                    );
                    self.core
                        .event_bus
                        .dispatch(Event::HistorySync(Box::new(lazy_hs)));
                }
            }
            Some(Err(e)) => {
                log::error!("Failed to process HistorySync data: {:?}", e);
            }
            None => {
                log::error!("History sync blocking task was cancelled");
            }
        }
    }

    async fn store_history_sync_msg_secrets(&self, raw_history_sync: &[u8]) -> usize {
        let history = match wa::HistorySync::decode(raw_history_sync) {
            Ok(history) => history,
            Err(e) => {
                log::warn!("Failed to decode HistorySync for messageSecret capture: {e:?}");
                return 0;
            }
        };

        let device_snapshot = self.persistence_manager.get_device_snapshot().await;
        let own_pn = device_snapshot.pn.as_ref().map(|j| j.to_non_ad());
        let own_lid = device_snapshot.lid.as_ref().map(|j| j.to_non_ad());
        let mut stored = 0usize;

        for conversation in &history.conversations {
            let Ok(chat) = conversation.id.parse::<Jid>() else {
                continue;
            };

            for history_msg in &conversation.messages {
                let Some(web_msg) = history_msg.message.as_ref() else {
                    continue;
                };
                let Some(secret) = web_msg.message_secret.as_deref().or_else(|| {
                    web_msg
                        .message
                        .as_ref()
                        .and_then(|m| m.message_context_info.as_ref())
                        .and_then(|mci| mci.message_secret.as_deref())
                }) else {
                    continue;
                };
                let Some(msg_id) = web_msg.key.id.as_deref() else {
                    continue;
                };

                let senders =
                    history_msg_secret_senders(&chat, web_msg, own_pn.as_ref(), own_lid.as_ref());
                for sender in senders {
                    if self
                        .persist_msg_secret_bytes(&chat, &sender, msg_id, secret)
                        .await
                    {
                        stored += 1;
                    }
                }
            }
        }

        stored
    }

    /// Ask the phone to re-upload a history-sync blob whose download failed,
    /// by sending a `<receipt type="server-error" category="peer">` with the
    /// blob's `media_key`.
    ///
    /// WA Web (`WAWebHandleHistorySyncNotification`) sends this on a non-network
    /// download failure; the encrypted payload is the same `ServerErrorReceipt`
    /// used for media retries. Exposed for consumers that detect an undownloadable
    /// or unwanted history-sync chunk and want the phone to re-send it.
    pub async fn send_history_sync_server_error_receipt(
        &self,
        message_id: &str,
        media_key: &[u8],
    ) -> Result<(), anyhow::Error> {
        let own_jid = self
            .get_pn()
            .await
            .ok_or(crate::client::ClientError::NotLoggedIn)?
            .to_non_ad();
        let (ciphertext, iv) =
            wacore::media_retry::encrypt_media_retry_receipt(media_key, message_id)?;
        let node = wacore::media_retry::build_history_sync_server_error_receipt(
            &own_jid,
            message_id,
            &ciphertext,
            &iv,
        );
        self.send_node(node).await?;
        Ok(())
    }

    /// Store a tctoken candidate extracted during history sync streaming.
    async fn store_tc_token_candidate(&self, candidate: TcTokenCandidate) {
        let jid: wacore_binary::Jid = match candidate.id.parse() {
            Ok(j) => j,
            Err(_) => return,
        };

        let resolved_lid = if jid.is_lid() {
            None
        } else {
            self.lid_pn_cache.get_current_lid(&jid.user).await
        };
        let token_key: &str = resolved_lid.as_deref().unwrap_or(&jid.user);

        let backend = self.persistence_manager.backend();

        // Avoid clobbering a newer local sender_timestamp from post-send issuance
        let incoming_sender_ts = candidate.tc_token_sender_timestamp.map(|ts| ts as i64);
        let merged_sender_ts = if let Ok(Some(existing)) = backend.get_tc_token(token_key).await {
            if (existing.token_timestamp as u64) > candidate.tc_token_timestamp {
                return;
            }
            match (existing.sender_timestamp, incoming_sender_ts) {
                (Some(e), Some(i)) => Some(e.max(i)),
                (Some(e), None) => Some(e),
                (None, i) => i,
            }
        } else {
            incoming_sender_ts
        };

        let entry = TcTokenEntry {
            token: candidate.tc_token,
            token_timestamp: candidate.tc_token_timestamp as i64,
            sender_timestamp: merged_sender_ts,
        };

        if let Err(e) = backend.put_tc_token(token_key, &entry).await {
            log::warn!(
                target: "Client/TcToken",
                "Failed to store history sync tctoken for {}: {e}",
                token_key
            );
        } else {
            log::debug!(
                target: "Client/TcToken",
                "Stored tctoken from history sync for {} (t={})",
                token_key,
                candidate.tc_token_timestamp
            );
        }
    }
}

fn history_msg_secret_senders(
    chat: &Jid,
    web_msg: &wa::WebMessageInfo,
    own_pn: Option<&Jid>,
    own_lid: Option<&Jid>,
) -> Vec<Jid> {
    let mut senders = Vec::with_capacity(2);

    if web_msg.key.from_me == Some(true) {
        if let Some(lid) = own_lid {
            push_unique_sender(&mut senders, lid.to_non_ad());
        }
        if let Some(pn) = own_pn {
            push_unique_sender(&mut senders, pn.to_non_ad());
        }
        return senders;
    }

    if chat.is_pn() || chat.is_lid() || chat.is_bot() {
        senders.push(chat.to_non_ad());
        return senders;
    }

    if let Some(raw_sender) = web_msg
        .key
        .participant
        .as_deref()
        .or(web_msg.participant.as_deref())
        && let Ok(sender) = raw_sender.parse::<Jid>()
    {
        senders.push(sender.to_non_ad());
    }

    senders
}

fn push_unique_sender(senders: &mut Vec<Jid>, sender: Jid) {
    if !senders.contains(&sender) {
        senders.push(sender);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::{Compression, write::ZlibEncoder};
    use std::io::Write;
    use std::sync::atomic::Ordering;

    fn compress_history_sync(history_sync: &wa::HistorySync) -> Vec<u8> {
        let raw = history_sync.encode_to_vec();
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&raw).expect("zlib write");
        encoder.finish().expect("zlib finish")
    }

    #[tokio::test]
    async fn process_history_sync_task_stores_message_secrets_without_handlers() {
        let client = crate::test_utils::create_test_client_with_name("history_msg_secret").await;
        client
            .persistence_manager
            .process_command(wacore::store::commands::DeviceCommand::SetId(Some(
                "5511000000001:0@s.whatsapp.net".parse().unwrap(),
            )))
            .await;
        client.is_running.store(true, Ordering::Relaxed);

        let chat = "5511777776666@s.whatsapp.net";
        let parent_id = "HIST_PARENT";
        let secret = vec![0x44u8; 32];
        let history_sync = wa::HistorySync {
            sync_type: wa::history_sync::HistorySyncType::InitialBootstrap as i32,
            conversations: vec![wa::Conversation {
                id: chat.to_string(),
                messages: vec![wa::HistorySyncMsg {
                    message: Some(wa::WebMessageInfo {
                        key: wa::MessageKey {
                            remote_jid: Some(chat.to_string()),
                            from_me: Some(false),
                            id: Some(parent_id.to_string()),
                            participant: None,
                        },
                        message: Some(wa::Message {
                            conversation: Some("historical".to_string()),
                            ..Default::default()
                        }),
                        message_secret: Some(secret.clone()),
                        ..Default::default()
                    }),
                    msg_order_id: Some(1),
                }],
                ..Default::default()
            }],
            ..Default::default()
        };
        let compressed = compress_history_sync(&history_sync);
        let notification = HistorySyncNotification {
            file_length: Some(compressed.len() as u64),
            sync_type: Some(wa::message::HistorySyncType::InitialBootstrap as i32),
            initial_hist_bootstrap_inline_payload: Some(compressed),
            ..Default::default()
        };

        client
            .process_history_sync_task("HIST_SYNC_SECRET".to_string(), notification)
            .await;

        let got = client
            .persistence_manager
            .backend()
            .get_msg_secret(chat, chat, parent_id)
            .await
            .unwrap();
        assert_eq!(got, Some(secret));
    }
}

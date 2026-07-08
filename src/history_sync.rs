use crate::types::events::{Event, LazyHistorySync};
use std::sync::Arc;
use wacore::history_sync::{HistoryMsgSecretRecord, TcTokenCandidate, process_history_sync};
use wacore::store::traits::MsgSecretEntry;
use wacore_binary::{Jid, JidExt as _};
use waproto::whatsapp::message::HistorySyncNotification;

use crate::client::Client;

impl Client {
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.media.history_sync", level = "debug", skip_all, fields(msg_id = %message_id)))]
    pub(crate) async fn handle_history_sync(
        self: &Arc<Self>,
        message_id: String,
        notification: HistorySyncNotification,
    ) {
        if self.is_shutting_down() {
            log::debug!(
                "Dropping history sync {} during shutdown (Type: {:?})",
                message_id,
                notification.sync_type
            );
            return;
        }

        if self.skip_history_sync_enabled() {
            log::debug!(
                "Skipping history sync for message {} (Type: {:?})",
                message_id,
                notification.sync_type
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
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.media.history_sync_task", level = "debug", skip_all, fields(msg_id = %message_id)))]
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
            notification.file_length.unwrap_or(0),
            notification.sync_type
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
            let device_snapshot = self.persistence_manager.get_device_snapshot();
            device_snapshot.pn.as_ref().map(|j| j.to_non_ad().user)
        };

        // Always carry the compressed input through (a move, no copy or extra
        // inflate); handler interest is evaluated at dispatch time below, so a
        // handler that registers while a large blob is being parsed still gets
        // the event instead of racing a pre-parse snapshot.
        // Small blobs (PushName, Recent): decode inline to avoid spawn_blocking overhead.
        // Large blobs: use blocking thread to avoid stalling the async runtime.
        const INLINE_THRESHOLD: usize = 256 * 1024;
        let parse_result = if compressed_data.len() < INLINE_THRESHOLD {
            Some(process_history_sync(
                compressed_data,
                own_user.as_deref(),
                true,
            ))
        } else {
            let (result_tx, result_rx) = futures::channel::oneshot::channel();
            let blocking_fut = self.runtime.spawn_blocking(Box::new(move || {
                let result = process_history_sync(compressed_data, own_user.as_deref(), true);
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

                self.store_history_sync_msg_secrets(sync_result.msg_secret_records)
                    .await;

                // Bulk PN-LID identity seed from field 15; persist and migrations
                // run detached, like the other batch learn paths.
                if !sync_result.lid_mappings.is_empty() {
                    let pairs: Vec<(String, String)> = sync_result
                        .lid_mappings
                        .into_iter()
                        .map(|m| (m.lid, m.phone_number))
                        .collect();
                    log::info!(
                        "History sync provided {} PN-LID mappings; learning",
                        pairs.len()
                    );
                    self.learn_lid_pn_mappings_batch(
                        pairs,
                        crate::lid_pn_cache::LearningSource::MigrationSyncLatest,
                        false,
                    )
                    .await;
                }

                // No interest pre-check: dispatch() evaluates handler interest
                // against a single bus snapshot (and skips materializing the
                // Arc when nobody listens), so deferring to it removes the
                // check-to-dispatch race window entirely. Building the event
                // is just a Bytes refcount move plus metadata.
                if let Some(compressed) = sync_result.compressed_bytes {
                    let lazy_hs = LazyHistorySync::new(
                        compressed,
                        sync_result.decompressed_size,
                        notification.sync_type.map(|t| t as i32).unwrap_or(0),
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

    async fn store_history_sync_msg_secrets(&self, records: Vec<HistoryMsgSecretRecord>) -> usize {
        use wacore::msg_secret::{self, RetentionClass};
        const SECRET_LEN: usize = wacore::reporting_token::MESSAGE_SECRET_SIZE;

        if !self.cache_config.seed_msg_secrets_from_history {
            // Opt-out of the pairing-time seed; live capture still runs.
            log::debug!(
                target: "Client/MsgSecret",
                "Skipping history-sync msg_secret seed (seed_msg_secrets_from_history = false)"
            );
            return 0;
        }
        let policy = self.cache_config.msg_secret_policy;
        if !policy.persists() {
            // Disabled: rely on the resolver / app store, seed nothing.
            log::debug!(
                target: "Client/MsgSecret",
                "Skipping history-sync msg_secret seed (policy = {policy:?})"
            );
            return 0;
        }
        let retention = &self.cache_config.msg_secret_retention;
        let now = wacore::time::now_secs();

        let device_snapshot = self.persistence_manager.get_device_snapshot();
        let own_pn = device_snapshot.pn.as_ref().map(|j| j.to_non_ad());
        let own_lid = device_snapshot.lid.as_ref().map(|j| j.to_non_ad());

        let mut entries = Vec::new();
        for record in records {
            if record.secret.len() != SECRET_LEN {
                continue;
            }
            let Ok(chat) = record.chat_id.parse::<Jid>() else {
                continue;
            };
            // Same three-way rule as the live path. A group bot prompt is a bot
            // context via its botMetadata (record.is_bot_invocation), so BotOnly
            // keeps it and a later bot reply can still decrypt.
            let class = msg_secret::classify_from_flags(
                chat.is_bot() || record.is_bot_invocation,
                record.is_poll_or_event,
            );
            // BotOnly seeds only bot-context secrets.
            if policy.bot_only() && class != RetentionClass::Bot {
                continue;
            }
            // Drop secrets whose parent is already past its retention horizon:
            // no add-on can still reference them, so seeding is pure waste. Full
            // skips the filter (prunes() is false) and seeds everything.
            if policy.prunes()
                && !msg_secret::within_seed_horizon(retention, class, record.timestamp, now)
            {
                continue;
            }
            let expires_at =
                msg_secret::expires_at(policy, retention, class, record.timestamp, now);
            let message_ts = record
                .timestamp
                .and_then(|t| i64::try_from(t).ok())
                .unwrap_or(0);

            let mut senders =
                history_msg_secret_senders(&chat, &record, own_pn.as_ref(), own_lid.as_ref());
            if chat.is_bot()
                && let Some(lid) = own_lid.as_ref()
            {
                push_unique_sender(&mut senders, lid.to_non_ad());
            }
            if senders.is_empty() {
                continue;
            }

            let sender_count = senders.len();
            let mut chat_id = chat.to_non_ad_string();
            let mut msg_id = record.msg_id.into_string();
            let mut secret = record.secret.into_vec();
            for (idx, sender) in senders.into_iter().enumerate() {
                let last_sender = idx + 1 == sender_count;
                entries.push(MsgSecretEntry {
                    chat: if last_sender {
                        std::mem::take(&mut chat_id)
                    } else {
                        chat_id.clone()
                    },
                    sender: sender.to_non_ad_string(),
                    msg_id: if last_sender {
                        std::mem::take(&mut msg_id)
                    } else {
                        msg_id.clone()
                    },
                    secret: if last_sender {
                        std::mem::take(&mut secret)
                    } else {
                        secret.clone()
                    },
                    expires_at,
                    message_ts,
                });
            }
        }

        if entries.is_empty() {
            return 0;
        }

        match self
            .persistence_manager
            .backend()
            .put_msg_secrets(entries)
            .await
        {
            Ok(stored) => stored,
            Err(e) => {
                log::warn!("failed to persist history-sync messageSecrets: {e:?}");
                0
            }
        }
    }

    /// Ask the phone to re-upload a history-sync blob whose download failed,
    /// by sending a `<receipt type="server-error" category="peer">` with the
    /// blob's `media_key`.
    ///
    /// WA Web (`WAWebHandleHistorySyncNotification`) sends this on a non-network
    /// download failure; the encrypted payload is the same `ServerErrorReceipt`
    /// used for media retries. Exposed for consumers that detect an undownloadable
    /// or unwanted history-sync chunk and want the phone to re-send it.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.media.history_sync_error_receipt", level = "debug", skip_all, fields(msg_id = %message_id), err(Debug)))]
    pub async fn send_history_sync_server_error_receipt(
        &self,
        message_id: &str,
        media_key: &[u8],
    ) -> Result<(), anyhow::Error> {
        let own_jid = self
            .get_pn()
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

        // Newer-wins lives in the store now (atomic, lock-free), so concurrent
        // history-sync chunks and the privacy path converge without a
        // get-then-store or a lock.
        if let Err(e) = backend
            .store_received_tc_token(
                token_key,
                &candidate.tc_token,
                candidate.tc_token_timestamp as i64,
            )
            .await
        {
            log::warn!(target: "Client/TcToken", "Failed to store history sync tctoken for {}: {e}", jid.observe());
            return;
        }
        if let Some(sender_ts) = candidate.tc_token_sender_timestamp
            && let Err(e) = backend
                .touch_tc_token_sender_timestamp(token_key, sender_ts as i64)
                .await
        {
            log::warn!(target: "Client/TcToken", "Failed to record history sync sender_timestamp for {}: {e}", jid.observe());
        }
        log::debug!(target: "Client/TcToken", "Stored tctoken from history sync for {} (t={})", jid.observe(), candidate.tc_token_timestamp);
    }
}

fn history_msg_secret_senders(
    chat: &Jid,
    record: &HistoryMsgSecretRecord,
    own_pn: Option<&Jid>,
    own_lid: Option<&Jid>,
) -> Vec<Jid> {
    let mut senders = Vec::with_capacity(2);

    if record.from_me {
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

    if let Some(raw_sender) = record
        .key_participant
        .as_deref()
        .or(record.web_msg_participant.as_deref())
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
    use buffa::Message as ProtoMessage;
    use flate2::{Compression, write::ZlibEncoder};
    use std::io::Write;
    use std::sync::atomic::Ordering;
    use waproto::whatsapp as wa;

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
            sync_type: wa::history_sync::HistorySyncType::INITIAL_BOOTSTRAP,
            conversations: vec![wa::Conversation {
                id: chat.to_string(),
                messages: vec![wa::HistorySyncMsg {
                    message: buffa::MessageField::some(wa::WebMessageInfo {
                        key: buffa::MessageField::some(wa::MessageKey {
                            remote_jid: Some(chat.to_string()),
                            from_me: Some(false),
                            id: Some(parent_id.to_string()),
                            participant: None,
                        }),
                        message: buffa::MessageField::some(wa::Message {
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
            sync_type: Some(wa::message::HistorySyncType::INITIAL_BOOTSTRAP),
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

    #[tokio::test]
    async fn process_history_sync_task_learns_lid_mappings() {
        let client = crate::test_utils::create_test_client_with_name("history_lid_mappings").await;
        client.is_running.store(true, Ordering::Relaxed);

        let history_sync = wa::HistorySync {
            sync_type: wa::history_sync::HistorySyncType::INITIAL_BOOTSTRAP,
            phone_number_to_lid_mappings: vec![wa::PhoneNumberToLIDMapping {
                pn_jid: Some("5511777776666@s.whatsapp.net".to_string()),
                lid_jid: Some("111222333444555@lid".to_string()),
            }],
            ..Default::default()
        };
        let compressed = compress_history_sync(&history_sync);
        let notification = HistorySyncNotification {
            file_length: Some(compressed.len() as u64),
            sync_type: Some(wa::message::HistorySyncType::INITIAL_BOOTSTRAP),
            initial_hist_bootstrap_inline_payload: Some(compressed),
            ..Default::default()
        };

        client
            .process_history_sync_task("HIST_SYNC_LID".to_string(), notification)
            .await;

        assert_eq!(
            client
                .lid_pn_cache
                .get_current_lid("5511777776666")
                .await
                .as_deref(),
            Some("111222333444555"),
            "history-sync mapping must be learned into the LID-PN cache"
        );
    }

    #[tokio::test]
    async fn process_history_sync_task_dispatches_compressed_lazy_event() {
        let client = crate::test_utils::create_test_client_with_name("history_lazy_event").await;
        client.is_running.store(true, Ordering::Relaxed);

        let chat = "5511777776666@s.whatsapp.net";
        let history_sync = wa::HistorySync {
            sync_type: wa::history_sync::HistorySyncType::INITIAL_BOOTSTRAP,
            conversations: vec![wa::Conversation {
                id: chat.to_string(),
                ..Default::default()
            }],
            ..Default::default()
        };
        let raw_len = history_sync.encode_to_vec().len();
        let compressed = compress_history_sync(&history_sync);
        let compressed_copy = compressed.clone();
        let notification = HistorySyncNotification {
            file_length: Some(compressed.len() as u64),
            sync_type: Some(wa::message::HistorySyncType::INITIAL_BOOTSTRAP),
            initial_hist_bootstrap_inline_payload: Some(compressed),
            ..Default::default()
        };

        // Register a handler BEFORE the task so retain_blob is true.
        let (handler, event_rx) = wacore::types::events::ChannelEventHandler::new();
        client.core.event_bus.add_handler(handler);

        client
            .process_history_sync_task("HIST_LAZY_EVENT".to_string(), notification)
            .await;

        let event = event_rx.try_recv().expect("HistorySync event dispatched");
        let crate::types::events::Event::HistorySync(lazy) = &*event else {
            panic!("expected HistorySync event, got {event:?}");
        };

        // The event carries the original compressed payload plus the exact
        // inflated size, and every consumption path works.
        assert_eq!(lazy.compressed_bytes().as_ref(), &compressed_copy[..]);
        assert_eq!(lazy.decompressed_size(), raw_len);
        let decoded = lazy.get().expect("decodes");
        assert_eq!(decoded.conversations[0].id, chat);
        let mut stream = lazy.stream();
        assert_eq!(
            stream.next_conversation().unwrap().unwrap().id,
            chat,
            "stream still works after get()"
        );
    }

    #[tokio::test]
    async fn process_history_sync_task_stores_bot_dm_secret_alias() {
        let client =
            crate::test_utils::create_test_client_with_name("history_bot_msg_secret").await;
        client
            .persistence_manager
            .process_command(wacore::store::commands::DeviceCommand::SetLid(Some(
                "999888777666555:0@lid".parse().unwrap(),
            )))
            .await;
        client.is_running.store(true, Ordering::Relaxed);

        let chat = "867051314767696@bot";
        let parent_id = "HIST_BOT_PARENT";
        let secret = vec![0x61u8; 32];
        let history_sync = wa::HistorySync {
            sync_type: wa::history_sync::HistorySyncType::INITIAL_BOOTSTRAP,
            conversations: vec![wa::Conversation {
                id: chat.to_string(),
                messages: vec![wa::HistorySyncMsg {
                    message: buffa::MessageField::some(wa::WebMessageInfo {
                        key: buffa::MessageField::some(wa::MessageKey {
                            remote_jid: Some(chat.to_string()),
                            from_me: Some(false),
                            id: Some(parent_id.to_string()),
                            participant: None,
                        }),
                        message: buffa::MessageField::some(wa::Message {
                            conversation: Some("bot historical".to_string()),
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
            sync_type: Some(wa::message::HistorySyncType::INITIAL_BOOTSTRAP),
            initial_hist_bootstrap_inline_payload: Some(compressed),
            ..Default::default()
        };

        client
            .process_history_sync_task("HIST_SYNC_BOT_SECRET".to_string(), notification)
            .await;

        let backend = client.persistence_manager.backend();
        let primary = backend.get_msg_secret(chat, chat, parent_id).await.unwrap();
        let alias = backend
            .get_msg_secret(chat, "999888777666555@lid", parent_id)
            .await
            .unwrap();

        assert_eq!(primary, Some(secret.clone()));
        assert_eq!(alias, Some(secret));
    }

    /// One inbound history message in `chat`, stamped at `ts_secs`, optionally a
    /// poll-creation message, carrying `secret`.
    fn history_msg(
        chat: &str,
        msg_id: &str,
        secret: &[u8],
        ts_secs: u64,
        is_poll: bool,
    ) -> wa::HistorySyncMsg {
        let message = if is_poll {
            wa::Message {
                poll_creation_message: buffa::MessageField::some(
                    wa::message::PollCreationMessage::default(),
                ),
                ..Default::default()
            }
        } else {
            wa::Message {
                conversation: Some("historical".to_string()),
                ..Default::default()
            }
        };
        wa::HistorySyncMsg {
            message: buffa::MessageField::some(wa::WebMessageInfo {
                key: buffa::MessageField::some(wa::MessageKey {
                    remote_jid: Some(chat.to_string()),
                    from_me: Some(false),
                    id: Some(msg_id.to_string()),
                    participant: None,
                }),
                message: buffa::MessageField::some(message),
                message_secret: Some(secret.to_vec()),
                message_timestamp: Some(ts_secs),
                ..Default::default()
            }),
            msg_order_id: Some(1),
        }
    }

    fn history_notification(
        chat: &str,
        messages: Vec<wa::HistorySyncMsg>,
    ) -> HistorySyncNotification {
        let history_sync = wa::HistorySync {
            sync_type: wa::history_sync::HistorySyncType::INITIAL_BOOTSTRAP,
            conversations: vec![wa::Conversation {
                id: chat.to_string(),
                messages,
                ..Default::default()
            }],
            ..Default::default()
        };
        let compressed = compress_history_sync(&history_sync);
        HistorySyncNotification {
            file_length: Some(compressed.len() as u64),
            sync_type: Some(wa::message::HistorySyncType::INITIAL_BOOTSTRAP),
            initial_hist_bootstrap_inline_payload: Some(compressed),
            ..Default::default()
        }
    }

    async fn seeded_client(
        name: &str,
        policy: crate::cache_config::MsgSecretPolicy,
    ) -> Arc<Client> {
        let cfg = crate::cache_config::CacheConfig {
            msg_secret_policy: policy,
            ..Default::default()
        };
        let client = crate::test_utils::create_test_client_with_config(
            name,
            std::sync::Arc::new(crate::test_utils::MockHttpClient),
            cfg,
        )
        .await;
        client
            .persistence_manager
            .process_command(wacore::store::commands::DeviceCommand::SetId(Some(
                "5511000000001:0@s.whatsapp.net".parse().unwrap(),
            )))
            .await;
        client.is_running.store(true, Ordering::Relaxed);
        client
    }

    #[tokio::test]
    async fn history_seed_managed_drops_old_text_keeps_recent() {
        use crate::cache_config::MsgSecretPolicy;
        let client = seeded_client("seed_managed_text", MsgSecretPolicy::Managed).await;
        let chat = "5511777776666@s.whatsapp.net";
        let now = wacore::time::now_secs() as u64;
        let old_ts = now - 60 * 86_400; // past the 30d text horizon
        let recent_ts = now - 86_400; // within it

        let notification = history_notification(
            chat,
            vec![
                history_msg(chat, "OLD_TEXT", &[0x11u8; 32], old_ts, false),
                history_msg(chat, "RECENT_TEXT", &[0x22u8; 32], recent_ts, false),
            ],
        );
        client
            .process_history_sync_task("S1".to_string(), notification)
            .await;

        let backend = client.persistence_manager.backend();
        assert_eq!(
            backend
                .get_msg_secret(chat, chat, "OLD_TEXT")
                .await
                .unwrap(),
            None,
            "a text secret past its 30d horizon must not be seeded"
        );
        assert_eq!(
            backend
                .get_msg_secret(chat, chat, "RECENT_TEXT")
                .await
                .unwrap(),
            Some(vec![0x22u8; 32]),
            "a recent text secret must be seeded"
        );
    }

    #[tokio::test]
    async fn history_seed_managed_keeps_old_poll_within_90d() {
        use crate::cache_config::MsgSecretPolicy;
        let client = seeded_client("seed_managed_poll", MsgSecretPolicy::Managed).await;
        let chat = "5511777776666@s.whatsapp.net";
        let now = wacore::time::now_secs() as u64;
        let ts = now - 60 * 86_400; // past 30d text but within 90d poll/event

        let notification = history_notification(
            chat,
            vec![history_msg(chat, "OLD_POLL", &[0x33u8; 32], ts, true)],
        );
        client
            .process_history_sync_task("S2".to_string(), notification)
            .await;

        assert_eq!(
            client
                .persistence_manager
                .backend()
                .get_msg_secret(chat, chat, "OLD_POLL")
                .await
                .unwrap(),
            Some(vec![0x33u8; 32]),
            "a poll parent within the 90d horizon must be seeded even past 30d"
        );
    }

    #[tokio::test]
    async fn history_seed_full_keeps_old_text() {
        use crate::cache_config::MsgSecretPolicy;
        let client = seeded_client("seed_full", MsgSecretPolicy::Full).await;
        let chat = "5511777776666@s.whatsapp.net";
        let now = wacore::time::now_secs() as u64;
        let old_ts = now - 365 * 86_400; // a year old

        let notification = history_notification(
            chat,
            vec![history_msg(chat, "ANCIENT", &[0x44u8; 32], old_ts, false)],
        );
        client
            .process_history_sync_task("S3".to_string(), notification)
            .await;

        assert_eq!(
            client
                .persistence_manager
                .backend()
                .get_msg_secret(chat, chat, "ANCIENT")
                .await
                .unwrap(),
            Some(vec![0x44u8; 32]),
            "Full seeds everything regardless of age"
        );
    }

    #[tokio::test]
    async fn history_seed_disabled_stores_nothing() {
        use crate::cache_config::MsgSecretPolicy;
        let client = seeded_client("seed_disabled", MsgSecretPolicy::Disabled).await;
        let chat = "5511777776666@s.whatsapp.net";
        let now = wacore::time::now_secs() as u64;

        let notification = history_notification(
            chat,
            vec![history_msg(chat, "ANY", &[0x55u8; 32], now - 60, false)],
        );
        client
            .process_history_sync_task("S4".to_string(), notification)
            .await;

        assert_eq!(
            client
                .persistence_manager
                .backend()
                .get_msg_secret(chat, chat, "ANY")
                .await
                .unwrap(),
            None,
            "Disabled persists nothing"
        );
    }

    #[tokio::test]
    async fn history_seed_managed_stamps_expires_at_from_message_time() {
        use crate::cache_config::MsgSecretPolicy;
        let client = seeded_client("seed_expires", MsgSecretPolicy::Managed).await;
        let chat = "5511777776666@s.whatsapp.net";
        let now = wacore::time::now_secs();
        let msg_ts = (now - 86_400) as u64; // 1 day old text → expires at msg_ts + 30d

        let notification = history_notification(
            chat,
            vec![history_msg(chat, "RECENT", &[0x66u8; 32], msg_ts, false)],
        );
        client
            .process_history_sync_task("S5".to_string(), notification)
            .await;

        let backend = client.persistence_manager.backend();
        // Deadline is msg_ts + 30d ≈ now + 29d: a prune at "now" keeps it.
        backend.delete_expired_msg_secrets(now).await.unwrap();
        assert!(
            backend
                .get_msg_secret(chat, chat, "RECENT")
                .await
                .unwrap()
                .is_some(),
            "row must survive a prune before its deadline"
        );
        // A prune past msg_ts + 30d removes it, proving the deadline tracks
        // message time, not seed time.
        let removed = backend
            .delete_expired_msg_secrets(now + 31 * 86_400)
            .await
            .unwrap();
        assert_eq!(removed, 1);
        assert!(
            backend
                .get_msg_secret(chat, chat, "RECENT")
                .await
                .unwrap()
                .is_none(),
            "row must be pruned once its message-time deadline passes"
        );
    }

    #[tokio::test]
    async fn history_seed_skipped_when_flag_disabled() {
        use crate::cache_config::{CacheConfig, MsgSecretPolicy};
        let cfg = CacheConfig {
            msg_secret_policy: MsgSecretPolicy::Managed,
            seed_msg_secrets_from_history: false,
            ..Default::default()
        };
        let client = crate::test_utils::create_test_client_with_config(
            "seed_flag_off",
            std::sync::Arc::new(crate::test_utils::MockHttpClient),
            cfg,
        )
        .await;
        client
            .persistence_manager
            .process_command(wacore::store::commands::DeviceCommand::SetId(Some(
                "5511000000001:0@s.whatsapp.net".parse().unwrap(),
            )))
            .await;
        client.is_running.store(true, Ordering::Relaxed);

        let chat = "5511777776666@s.whatsapp.net";
        let now = wacore::time::now_secs() as u64;
        let notification = history_notification(
            chat,
            vec![history_msg(chat, "RECENT", &[0x77u8; 32], now - 60, false)],
        );
        client
            .process_history_sync_task("S6".to_string(), notification)
            .await;

        assert_eq!(
            client
                .persistence_manager
                .backend()
                .get_msg_secret(chat, chat, "RECENT")
                .await
                .unwrap(),
            None,
            "seed flag off must skip history seeding even under Managed"
        );
    }

    /// A group message in `chat` from `participant`, optionally carrying
    /// botMetadata (a bot invocation), with `secret`.
    fn group_history_msg(
        chat: &str,
        participant: &str,
        msg_id: &str,
        secret: &[u8],
        ts_secs: u64,
        bot_prompt: bool,
    ) -> wa::HistorySyncMsg {
        let message_context_info = bot_prompt.then(|| wa::MessageContextInfo {
            bot_metadata: buffa::MessageField::some(wa::BotMetadata {
                persona_id: Some("867051314767696".into()),
                ..Default::default()
            }),
            ..Default::default()
        });
        wa::HistorySyncMsg {
            message: buffa::MessageField::some(wa::WebMessageInfo {
                key: buffa::MessageField::some(wa::MessageKey {
                    remote_jid: Some(chat.to_string()),
                    from_me: Some(false),
                    id: Some(msg_id.to_string()),
                    participant: Some(participant.to_string()),
                }),
                message: buffa::MessageField::some(wa::Message {
                    extended_text_message: buffa::MessageField::some(
                        wa::message::ExtendedTextMessage {
                            text: Some("hi".into()),
                            ..Default::default()
                        },
                    ),
                    message_context_info: message_context_info.into(),
                    ..Default::default()
                }),
                message_secret: Some(secret.to_vec()),
                message_timestamp: Some(ts_secs),
                ..Default::default()
            }),
            msg_order_id: Some(1),
        }
    }

    #[tokio::test]
    async fn history_seed_botonly_keeps_group_bot_prompt_skips_plain() {
        use crate::cache_config::MsgSecretPolicy;
        let client = seeded_client("seed_botonly_bot", MsgSecretPolicy::BotOnly).await;
        let group = "120363021033254949@g.us";
        let participant = "5511888887777@s.whatsapp.net";
        let now = wacore::time::now_secs() as u64;

        let notification = history_notification(
            group,
            vec![
                group_history_msg(
                    group,
                    participant,
                    "BOT_PROMPT",
                    &[0x88u8; 32],
                    now - 60,
                    true,
                ),
                group_history_msg(
                    group,
                    participant,
                    "PLAIN_GRP",
                    &[0x99u8; 32],
                    now - 60,
                    false,
                ),
            ],
        );
        client
            .process_history_sync_task("SB".to_string(), notification)
            .await;

        let backend = client.persistence_manager.backend();
        assert_eq!(
            backend
                .get_msg_secret(group, participant, "BOT_PROMPT")
                .await
                .unwrap(),
            Some(vec![0x88u8; 32]),
            "BotOnly must seed a group bot prompt (botMetadata = bot context)"
        );
        assert_eq!(
            backend
                .get_msg_secret(group, participant, "PLAIN_GRP")
                .await
                .unwrap(),
            None,
            "BotOnly must skip a plain group message"
        );
    }

    #[tokio::test]
    async fn history_sync_tctoken_replaces_byteless_placeholder() {
        let client = crate::test_utils::create_test_client_with_name("history_tctoken_ph").await;
        let backend = client.persistence_manager.backend();

        // Post-send issuance wrote a placeholder: no bytes, token_timestamp is a
        // recent sender epoch (newer than the real token minted earlier).
        backend
            .touch_tc_token_sender_timestamp("555000999", 2000)
            .await
            .unwrap();

        client
            .store_tc_token_candidate(TcTokenCandidate {
                id: "555000999@lid".to_string(),
                tc_token: vec![0xAB, 0xCD],
                tc_token_timestamp: 1000,
                tc_token_sender_timestamp: None,
            })
            .await;

        let stored = backend.get_tc_token("555000999").await.unwrap().unwrap();
        assert_eq!(
            stored.token,
            vec![0xAB, 0xCD],
            "history-sync token must replace the placeholder despite its older timestamp"
        );
        assert_eq!(stored.token_timestamp, 1000);
        assert_eq!(
            stored.sender_timestamp,
            Some(2000),
            "the placeholder's sender bucket must be preserved"
        );
    }

    #[tokio::test]
    async fn history_sync_tctoken_seeds_sender_bucket() {
        let client = crate::test_utils::create_test_client_with_name("history_tctoken_seed").await;
        let backend = client.persistence_manager.backend();

        // No prior local state — the candidate's own sender timestamp seeds it.
        client
            .store_tc_token_candidate(TcTokenCandidate {
                id: "555000998@lid".to_string(),
                tc_token: vec![0x01],
                tc_token_timestamp: 1000,
                tc_token_sender_timestamp: Some(1500),
            })
            .await;

        let stored = backend.get_tc_token("555000998").await.unwrap().unwrap();
        assert_eq!(stored.token, vec![0x01]);
        assert_eq!(stored.sender_timestamp, Some(1500));
    }
}

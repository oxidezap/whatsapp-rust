use crate::types::events::{Event, LazyHistorySync};
use bytes::Bytes;
use std::sync::Arc;
use wacore::history_sync::{
    HistoryMsgSecretRecordRef, HistoryMsgSecretRecordVisitor, TcTokenCandidate,
    process_history_sync_bytes_with_record_sink,
};
use wacore::messages::DetachedHistorySyncNotification;
use wacore::msg_secret::{MsgSecretPolicy, MsgSecretRetention, RetentionClass};
use wacore::store::traits::MsgSecretEntry;
use wacore_binary::{Jid, JidExt as _};

use crate::client::Client;

const HISTORY_MSG_SECRET_SIZE: usize = wacore::reporting_token::MESSAGE_SECRET_SIZE;

/// Immutable seed policy snapshot shared by the streaming filter and the
/// persistence pass. Keeping one snapshot prevents a long parse from applying
/// different time boundaries at its two stages.
#[derive(Clone, Copy)]
struct HistorySecretSeedConfig {
    enabled: bool,
    policy: MsgSecretPolicy,
    retention: MsgSecretRetention,
    now: i64,
}

impl HistorySecretSeedConfig {
    fn snapshot(config: &crate::cache_config::CacheConfig) -> Self {
        Self {
            enabled: config.seed_msg_secrets_from_history,
            policy: config.msg_secret_policy,
            retention: config.msg_secret_retention,
            now: wacore::time::now_secs(),
        }
    }

    #[inline]
    fn accepts_secret(self, secret_len: usize) -> bool {
        self.enabled && self.policy.persists() && secret_len == HISTORY_MSG_SECRET_SIZE
    }

    fn retained_class(
        self,
        secret_len: usize,
        chat_is_bot: bool,
        is_bot_invocation: bool,
        is_poll_or_event: bool,
        timestamp: Option<u64>,
    ) -> Option<RetentionClass> {
        if !self.accepts_secret(secret_len) {
            return None;
        }
        let class = wacore::msg_secret::classify_from_flags(
            chat_is_bot || is_bot_invocation,
            is_poll_or_event,
        );
        if self.policy.bot_only() && class != RetentionClass::Bot {
            return None;
        }
        if self.policy.prunes()
            && !wacore::msg_secret::within_seed_horizon(&self.retention, class, timestamp, self.now)
        {
            return None;
        }
        Some(class)
    }
}

/// Builds the store's final representation directly from each borrowed core
/// record. The parsed chat is cached per conversation; valid history syncs no
/// longer allocate an intermediate record or parse the same JID per message.
struct HistorySecretSeedCollector {
    config: HistorySecretSeedConfig,
    own_pn: Option<Jid>,
    own_lid: Option<Jid>,
    last_chat_id: String,
    last_chat_is_bot: Option<bool>,
    last_chat: Option<Jid>,
    last_chat_non_ad_id: Option<Arc<str>>,
    entries: Vec<MsgSecretEntry>,
}

impl HistorySecretSeedCollector {
    fn new(config: HistorySecretSeedConfig, own_pn: Option<Jid>, own_lid: Option<Jid>) -> Self {
        Self {
            config,
            own_pn,
            own_lid,
            last_chat_id: String::new(),
            last_chat_is_bot: None,
            last_chat: None,
            last_chat_non_ad_id: None,
            entries: Vec::new(),
        }
    }

    fn collect(&mut self, record: HistoryMsgSecretRecordRef<'_>) {
        if !self.config.accepts_secret(record.secret.len()) {
            return;
        }

        if self.last_chat_id != record.chat_id {
            self.last_chat_id.clear();
            self.last_chat_id.push_str(record.chat_id);
            self.last_chat = None;
            self.last_chat_non_ad_id = None;
            self.last_chat_is_bot =
                wacore_binary::jid::parse_jid_ref(record.chat_id).map(|chat| chat.is_bot());
            if self.last_chat_is_bot.is_none()
                && let Ok(chat) = record.chat_id.parse::<Jid>()
            {
                self.last_chat_is_bot = Some(chat.is_bot());
                self.last_chat_non_ad_id = Some(Arc::from(chat.to_non_ad_string()));
                self.last_chat = Some(chat);
            }
        }

        let Some(class) = self.config.retained_class(
            record.secret.len(),
            self.last_chat_is_bot.unwrap_or(false),
            record.is_bot_invocation,
            record.is_poll_or_event,
            record.timestamp,
        ) else {
            return;
        };
        let expires_at = wacore::msg_secret::expires_at(
            self.config.policy,
            &self.config.retention,
            class,
            record.timestamp,
            self.config.now,
        );
        let message_ts = record
            .timestamp
            .and_then(|timestamp| i64::try_from(timestamp).ok())
            .unwrap_or(0);

        if self.last_chat.is_none() {
            let Ok(chat) = record.chat_id.parse::<Jid>() else {
                return;
            };
            self.last_chat_is_bot = Some(chat.is_bot());
            self.last_chat_non_ad_id = Some(Arc::from(chat.to_non_ad_string()));
            self.last_chat = Some(chat);
        }
        let (Some(chat), Some(chat_non_ad_id)) =
            (self.last_chat.as_ref(), self.last_chat_non_ad_id.as_ref())
        else {
            return;
        };

        let senders =
            history_msg_secret_senders(chat, record, self.own_pn.as_ref(), self.own_lid.as_ref());
        if senders.iter().all(Option::is_none) {
            return;
        }

        let chat_id = Arc::clone(chat_non_ad_id);
        let msg_id: Arc<str> = Arc::from(record.msg_id);
        let secret = match <&[u8; HISTORY_MSG_SECRET_SIZE]>::try_from(record.secret) {
            Ok(secret) => *secret,
            Err(_) => return,
        };
        for sender in senders.into_iter().flatten() {
            let sender_id = if sender.is_same_chat_as(chat) {
                Arc::clone(&chat_id)
            } else {
                Arc::from(sender.to_non_ad_string())
            };
            self.entries.push(MsgSecretEntry {
                chat: Arc::clone(&chat_id),
                sender: sender_id,
                msg_id: Arc::clone(&msg_id),
                secret,
                expires_at,
                message_ts,
            });
        }
    }

    fn into_entries(self) -> Vec<MsgSecretEntry> {
        self.entries
    }
}

impl HistoryMsgSecretRecordVisitor for &mut HistorySecretSeedCollector {
    fn visit(&mut self, record: HistoryMsgSecretRecordRef<'_>) -> usize {
        let previous_len = self.entries.len();
        self.collect(record);
        self.entries.len() - previous_len
    }

    fn reserve(&mut self, additional: usize) {
        self.entries.reserve(additional);
    }

    fn retained_item_size(&self) -> Option<std::num::NonZeroUsize> {
        std::num::NonZeroUsize::new(std::mem::size_of::<MsgSecretEntry>())
    }
}

impl Client {
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.media.history_sync", level = "debug", skip_all, fields(msg_id = %message_id)))]
    pub(crate) async fn handle_history_sync(
        self: &Arc<Self>,
        message_id: String,
        notification: DetachedHistorySyncNotification,
    ) {
        if self.is_shutting_down() {
            log::debug!(
                "Dropping history sync {} during shutdown (Type: {:?})",
                message_id,
                notification.notification.sync_type
            );
            return;
        }

        if self.skip_history_sync_enabled() {
            log::debug!(
                "Skipping history sync for message {} (Type: {:?})",
                message_id,
                notification.notification.sync_type
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
        let payload_bytes = notification.inline_payload.as_ref().map_or(0, Bytes::len);
        let tracker = self.begin_history_sync_task(payload_bytes);
        let task = crate::sync_task::MajorSyncTask::HistorySync {
            message_id,
            notification: Box::new(notification),
            tracker,
        };
        if let Err(e) = self.major_sync_task_sender.send(task).await {
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
    #[cfg(test)]
    pub(crate) async fn process_history_sync_task(
        self: &Arc<Self>,
        message_id: String,
        notification: DetachedHistorySyncNotification,
    ) {
        let payload_bytes = notification.inline_payload.as_ref().map_or(0, Bytes::len);
        let mut tracker = self.begin_history_sync_task(payload_bytes);
        self.process_history_sync_task_tracked(message_id, notification, &mut tracker)
            .await;
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.media.history_sync_task", level = "debug", skip_all, fields(msg_id = %message_id)))]
    pub(crate) async fn process_history_sync_task_tracked(
        self: &Arc<Self>,
        message_id: String,
        notification: DetachedHistorySyncNotification,
        tracker: &mut crate::sync_task::HistorySyncTaskTracker,
    ) {
        if self.is_shutting_down() {
            log::debug!("Aborting history sync {} before processing", message_id);
            return;
        }

        let DetachedHistorySyncNotification {
            mut notification,
            inline_payload,
        } = notification;

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
        let (compressed_data, payload_bytes) = if let Some(inline_payload) = inline_payload {
            log::info!(
                "Found inline history sync payload ({} bytes). Using directly.",
                inline_payload.len()
            );
            let payload_bytes = inline_payload.len();
            (inline_payload, payload_bytes)
        } else {
            log::info!("Downloading external history sync blob...");
            if self.is_shutting_down() || !self.is_connected() {
                log::debug!(
                    "Aborting history sync {} before blob download: client disconnected",
                    message_id
                );
                return;
            }
            // The native in-memory downloader streams decryption and safely
            // pre-sizes its fresh retry buffer from the declared file length.
            // Reuse it here so a ~1 MiB history blob does not climb the Vec
            // doubling ladder (1 MiB -> 2 MiB) on every successful download.
            match self.download(&notification).await {
                Ok(bytes) => {
                    log::info!("Successfully downloaded history sync blob.");
                    // Payload accounting uses logical compressed bytes for
                    // both owned Vecs and shared Bytes slices. Bytes does not
                    // expose the capacity of its backing allocation.
                    let payload_bytes = bytes.len();
                    (Bytes::from(bytes), payload_bytes)
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
        tracker.set_payload_bytes(payload_bytes);

        let device_snapshot = self.persistence_manager.get_device_snapshot();
        let own_pn = device_snapshot.pn.as_ref().map(|jid| jid.to_non_ad());
        let own_lid = device_snapshot.lid.as_ref().map(|jid| jid.to_non_ad());
        let own_user = own_pn.as_ref().map(|jid| jid.user.clone());
        let secret_seed_config = HistorySecretSeedConfig::snapshot(&self.cache_config);
        let mut secret_collector =
            HistorySecretSeedCollector::new(secret_seed_config, own_pn, own_lid);

        // Always carry the compressed input through (a move, no copy or extra
        // inflate); handler interest is evaluated at dispatch time below, so a
        // handler that registers while a large blob is being parsed still gets
        // the event instead of racing a pre-parse snapshot.
        // Small blobs (PushName, Recent): decode inline to avoid spawn_blocking overhead.
        // Large blobs: use blocking thread to avoid stalling the async runtime.
        const INLINE_THRESHOLD: usize = 256 * 1024;
        let parse_result = if compressed_data.len() < INLINE_THRESHOLD {
            let result = process_history_sync_bytes_with_record_sink(
                compressed_data,
                own_user.as_deref(),
                true,
                &mut secret_collector,
            );
            Some((result, secret_collector.into_entries()))
        } else {
            let (result_tx, result_rx) = futures::channel::oneshot::channel();
            let blocking_fut = self.runtime.spawn_blocking(Box::new(move || {
                let result = process_history_sync_bytes_with_record_sink(
                    compressed_data,
                    own_user.as_deref(),
                    true,
                    &mut secret_collector,
                );
                let _ = result_tx.send((result, secret_collector.into_entries()));
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
            Some((Ok(sync_result), secret_entries)) => {
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

                self.store_history_sync_msg_secret_entries(secret_entries, secret_seed_config)
                    .await;

                // Bulk PN-LID identity seed from field 15; persist and migrations
                // run detached, like the other batch learn paths. `Other`
                // matches WA Web's `learningSource: "other"` for this harvest
                // (WAWebHistorySyncChunk): a conservative seed that only adds
                // new LIDs and never clobbers a live-learned mapping.
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
                        crate::lid_pn_cache::LearningSource::Other,
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
            Some((Err(e), _)) => {
                log::error!("Failed to process HistorySync data: {:?}", e);
            }
            None => {
                log::error!("History sync blocking task was cancelled");
            }
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "wa.history.secrets.store",
            level = "debug",
            skip_all,
            fields(entries = entries.len() as u64)
        )
    )]
    async fn store_history_sync_msg_secret_entries(
        &self,
        entries: Vec<MsgSecretEntry>,
        seed_config: HistorySecretSeedConfig,
    ) -> usize {
        if !seed_config.enabled {
            // Opt-out of the pairing-time seed; live capture still runs.
            log::debug!(
                target: "Client/MsgSecret",
                "Skipping history-sync msg_secret seed (seed_msg_secrets_from_history = false)"
            );
            return 0;
        }
        if !seed_config.policy.persists() {
            // Disabled: rely on the resolver / app store, seed nothing.
            log::debug!(
                target: "Client/MsgSecret",
                "Skipping history-sync msg_secret seed (policy = {:?})",
                seed_config.policy
            );
            return 0;
        }

        if entries.is_empty() {
            return 0;
        }

        // This is already the final owned batch. Routing it through the live
        // write-behind would retain the Vec alongside its keyed Arc map and
        // flush snapshot; the backend consumes it directly and applies its
        // own bounded statement batching.
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

/// At most two aliases are persisted for one secret: the account's PN/LID
/// pair for an outgoing message, or an incoming bot chat plus the account LID.
const MAX_HISTORY_SECRET_SENDERS: usize = 2;
type HistorySecretSenders = [Option<Jid>; MAX_HISTORY_SECRET_SENDERS];

fn history_msg_secret_senders(
    chat: &Jid,
    record: HistoryMsgSecretRecordRef<'_>,
    own_pn: Option<&Jid>,
    own_lid: Option<&Jid>,
) -> HistorySecretSenders {
    let mut senders = std::array::from_fn(|_| None);

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
        push_unique_sender(&mut senders, chat.to_non_ad());
        if chat.is_bot()
            && let Some(lid) = own_lid
        {
            push_unique_sender(&mut senders, lid.to_non_ad());
        }
        return senders;
    }

    if let Some(raw_sender) = record.key_participant.or(record.web_msg_participant)
        && let Ok(sender) = raw_sender.parse::<Jid>()
    {
        push_unique_sender(&mut senders, sender.to_non_ad());
    }

    senders
}

fn push_unique_sender(senders: &mut HistorySecretSenders, sender: Jid) {
    if senders.iter().flatten().any(|existing| existing == &sender) {
        return;
    }
    let empty = senders.iter_mut().find(|slot| slot.is_none());
    debug_assert!(empty.is_some(), "history secret sender capacity exhausted");
    if let Some(slot) = empty {
        *slot = Some(sender);
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;
    use buffa::Message as ProtoMessage;
    use flate2::{Compression, write::ZlibEncoder};
    use std::io::Write;
    use std::sync::atomic::Ordering;
    use waproto::whatsapp as wa;
    use waproto::whatsapp::message::HistorySyncNotification;

    fn compress_history_sync(history_sync: &wa::HistorySync) -> Vec<u8> {
        let raw = history_sync.encode_to_vec();
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        encoder.write_all(&raw).expect("zlib write");
        encoder.finish().expect("zlib finish")
    }

    fn sender_record(from_me: bool) -> HistoryMsgSecretRecordRef<'static> {
        HistoryMsgSecretRecordRef {
            conversation_index: 0,
            chat_id: "",
            from_me,
            key_participant: None,
            web_msg_participant: None,
            msg_id: "M",
            secret: &[],
            timestamp: None,
            is_poll_or_event: false,
            is_bot_invocation: false,
        }
    }

    #[test]
    fn fixed_sender_capacity_covers_outgoing_and_incoming_bot_aliases() {
        let pn: Jid = "5511000000001@s.whatsapp.net".parse().unwrap();
        let lid: Jid = "111222333444555@lid".parse().unwrap();
        let bot: Jid = "867051314767696@bot".parse().unwrap();

        let outgoing = history_msg_secret_senders(&bot, sender_record(true), Some(&pn), Some(&lid));
        assert_eq!(
            outgoing.into_iter().flatten().collect::<Vec<_>>(),
            vec![lid.to_non_ad(), pn.to_non_ad()],
            "outgoing messages return before the chat-alias branch"
        );

        let incoming =
            history_msg_secret_senders(&bot, sender_record(false), Some(&pn), Some(&lid));
        assert_eq!(
            incoming.into_iter().flatten().collect::<Vec<_>>(),
            vec![bot.to_non_ad(), lid.to_non_ad()],
            "incoming bot messages retain the chat and account-LID aliases"
        );
    }

    #[test]
    fn collector_refreshes_chat_cache_when_id_changes_within_conversation() {
        const CONVERSATION_INDEX: usize = 0;
        const FIRST_CHAT: &str = "15550000001@s.whatsapp.net";
        const FIRST_MSG_ID: &str = "FIRST";
        const SECOND_CHAT: &str = "15550000002@s.whatsapp.net";
        const SECOND_MSG_ID: &str = "SECOND";
        const SECRET: [u8; HISTORY_MSG_SECRET_SIZE] = [0x5a; HISTORY_MSG_SECRET_SIZE];
        let config = HistorySecretSeedConfig {
            enabled: true,
            policy: MsgSecretPolicy::Full,
            retention: MsgSecretRetention::default(),
            now: 0,
        };
        let mut collector = HistorySecretSeedCollector::new(config, None, None);

        for (chat_id, msg_id) in [(FIRST_CHAT, FIRST_MSG_ID), (SECOND_CHAT, SECOND_MSG_ID)] {
            collector.collect(HistoryMsgSecretRecordRef {
                conversation_index: CONVERSATION_INDEX,
                chat_id,
                from_me: false,
                key_participant: None,
                web_msg_participant: None,
                msg_id,
                secret: &SECRET,
                timestamp: None,
                is_poll_or_event: false,
                is_bot_invocation: false,
            });
        }

        let entries = collector.into_entries();
        assert_eq!(entries.len(), 2);
        assert_eq!(entries[0].chat.as_ref(), FIRST_CHAT);
        assert_eq!(entries[0].sender, entries[0].chat);
        assert_eq!(entries[1].chat.as_ref(), SECOND_CHAT);
        assert_eq!(entries[1].sender, entries[1].chat);
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
            .process_history_sync_task("HIST_SYNC_SECRET".to_string(), notification.into())
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
            .process_history_sync_task("HIST_SYNC_LID".to_string(), notification.into())
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
        client.core.event_bus.subscribe_handler(handler).detach();

        client
            .process_history_sync_task("HIST_LAZY_EVENT".to_string(), notification.into())
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
            .process_history_sync_task("HIST_SYNC_BOT_SECRET".to_string(), notification.into())
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
            .process_history_sync_task("S1".to_string(), notification.into())
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
            .process_history_sync_task("S2".to_string(), notification.into())
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
            .process_history_sync_task("S3".to_string(), notification.into())
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
            .process_history_sync_task("S4".to_string(), notification.into())
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
            .process_history_sync_task("S5".to_string(), notification.into())
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
            .process_history_sync_task("S6".to_string(), notification.into())
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
            .process_history_sync_task("SB".to_string(), notification.into())
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

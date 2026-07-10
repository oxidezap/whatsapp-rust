//! WhatsApp client wrapper for UI integration

use std::collections::HashMap;
use std::sync::Arc;

use log::{debug, error, info, warn};
use tokio::sync::{Mutex, mpsc};
use wacore::proto_helpers::MessageExt;
use wacore::types::call::{CallAction, IncomingCall as WaIncomingCall};
use wacore::types::events::Event;
use wacore::types::presence::ReceiptType;
use wacore_binary::jid::{Jid, JidExt};
use waproto::whatsapp as wa;
use whatsapp_rust::bot::Bot;
use whatsapp_rust::client::Client;
use whatsapp_rust::store::SqliteStore;
use whatsapp_rust::voip::CallHandle;
use whatsapp_rust_chat_store::ChatStore;

use crate::audio::{spawn_mic, spawn_speaker};
use crate::state::{
    ChatMessage, DownloadableMedia, IncomingCall, MediaContent, MediaType, UiEvent,
};
use wacore::download::MediaType as DownloadMediaType;

/// Resolve a stable per-user path for the SQLite database. A CWD-relative
/// path would silently split state between launch methods (desktop launcher
/// vs terminal), so prefer the platform data dir and only fall back to the
/// working directory when no home is known.
fn resolve_database_path() -> String {
    const DB_FILE: &str = "whatsapp.db";

    let not_empty = |v: std::ffi::OsString| (!v.is_empty()).then_some(std::path::PathBuf::from(v));
    let data_root = if cfg!(target_os = "macos") {
        std::env::var_os("HOME")
            .and_then(not_empty)
            .map(|home| home.join("Library/Application Support"))
    } else if cfg!(target_os = "windows") {
        std::env::var_os("LOCALAPPDATA")
            .and_then(not_empty)
            .or_else(|| {
                std::env::var_os("USERPROFILE")
                    .and_then(not_empty)
                    .map(|profile| profile.join("AppData").join("Local"))
            })
    } else {
        std::env::var_os("XDG_DATA_HOME")
            .and_then(not_empty)
            .or_else(|| {
                std::env::var_os("HOME")
                    .and_then(not_empty)
                    .map(|home| home.join(".local/share"))
            })
    };

    let Some(dir) = data_root.map(|root| root.join("whatsapp-rust-desktop")) else {
        warn!("No home directory found; using CWD-relative {DB_FILE}");
        return DB_FILE.to_string();
    };
    // SQLite won't create missing parent directories itself.
    if let Err(e) = std::fs::create_dir_all(&dir) {
        warn!(
            "Failed to create data dir {}: {e}; using CWD-relative {DB_FILE}",
            dir.display()
        );
        return DB_FILE.to_string();
    }

    dir.join(DB_FILE).to_string_lossy().into_owned()
}

/// Helper struct for building DownloadableMedia from common message fields
struct DownloadableBuilder<'a> {
    direct_path: Option<&'a str>,
    media_key: Option<&'a [u8]>,
    file_enc_sha256: Option<&'a [u8]>,
    file_length: Option<u64>,
    mime_type: &'a str,
    duration_secs: Option<u32>,
    download_type: DownloadMediaType,
}

impl<'a> DownloadableBuilder<'a> {
    /// Try to build a DownloadableMedia from the provided fields.
    /// Returns None if any required field (direct_path, media_key, file_enc_sha256) is missing.
    fn build(self) -> Option<DownloadableMedia> {
        let direct_path = self.direct_path?;
        let media_key = self.media_key?;
        let file_enc_sha256 = self.file_enc_sha256?;

        Some(DownloadableMedia {
            direct_path: direct_path.to_string(),
            media_key: media_key.to_vec(),
            file_enc_sha256: file_enc_sha256.to_vec(),
            file_length: self.file_length.unwrap_or(0),
            mime_type: self.mime_type.to_string(),
            duration_secs: self.duration_secs,
            download_type: self.download_type,
        })
    }
}

/// Shared client handle for accessing the WhatsApp client from UI
pub type ClientHandle = Arc<Mutex<Option<Arc<Client>>>>;

/// Shared UI event sender for sending events from async operations
pub type UiEventSender = Arc<Mutex<Option<mpsc::UnboundedSender<UiEvent>>>>;

/// Shared chat-store handle (durable message history in the same SQLite file)
pub type ChatStoreHandle = Arc<Mutex<Option<Arc<ChatStore>>>>;

/// Live call state shared between the event pump and the UI action methods.
#[derive(Clone, Default)]
pub struct CallRegistry {
    /// Ringing offers by call id, consumed by accept/decline.
    pending: Arc<Mutex<HashMap<String, Arc<WaIncomingCall>>>>,
    /// Media-live calls by call id.
    active: Arc<Mutex<HashMap<String, Arc<CallHandle>>>>,
    /// Ids cancelled before any handle existed (the UI's placeholder id while
    /// start_call is still connecting); start_call hangs these up on arrival.
    cancelled: Arc<Mutex<std::collections::HashSet<String>>>,
}

/// WhatsApp client wrapper that manages the connection and provides
/// a clean interface for UI operations.
pub struct WhatsAppClient {
    /// Tokio runtime for async operations
    runtime: Arc<tokio::runtime::Runtime>,
    /// Shared client reference
    client_handle: ClientHandle,
    /// Shared UI event sender for sending events from operations like start_call
    ui_sender: UiEventSender,
    /// Live/ringing calls
    calls: CallRegistry,
    /// Durable chat history (same SQLite file as the device store)
    chat_store: ChatStoreHandle,
    /// Tears down `run_client` on retry: without it the replaced client's
    /// thread would keep its runtime and SQLite pool alive forever (bot.run()
    /// reconnects internally and never returns on its own).
    shutdown: Arc<tokio::sync::Notify>,
    /// Whether the client has been started
    started: bool,
}

impl WhatsAppClient {
    /// Create a new WhatsApp client wrapper
    pub fn new() -> Self {
        let runtime = Arc::new(
            tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .expect("Failed to create tokio runtime"),
        );

        Self {
            runtime,
            client_handle: Arc::new(Mutex::new(None)),
            ui_sender: Arc::new(Mutex::new(None)),
            calls: CallRegistry::default(),
            chat_store: Arc::new(Mutex::new(None)),
            shutdown: Arc::new(tokio::sync::Notify::new()),
            started: false,
        }
    }

    /// Stop the background run loop so its thread exits and the runtime and
    /// SQLite handles drop. Idempotent; a signal fired before the loop is up
    /// still lands (notify_one stores a permit).
    pub fn shutdown(&self) {
        self.shutdown.notify_one();
    }

    /// Get the runtime handle for UI async operations
    #[allow(dead_code)]
    pub fn runtime(&self) -> Arc<tokio::runtime::Runtime> {
        self.runtime.clone()
    }

    /// Get the client handle for sending messages
    #[allow(dead_code)]
    pub fn client_handle(&self) -> ClientHandle {
        self.client_handle.clone()
    }

    /// Durable chat history store, once the client is up (None before init).
    #[allow(dead_code)]
    pub fn chat_store(&self) -> ChatStoreHandle {
        self.chat_store.clone()
    }

    /// Start the WhatsApp client in a background thread
    ///
    /// Returns a receiver for UI events, or an error if already started
    pub fn start(&mut self) -> Result<mpsc::UnboundedReceiver<UiEvent>, &'static str> {
        if self.started {
            return Err("WhatsApp client already started");
        }
        self.started = true;

        let (ui_tx, ui_rx) = mpsc::unbounded_channel::<UiEvent>();
        let client_handle = self.client_handle.clone();
        let ui_sender = self.ui_sender.clone();
        let calls = self.calls.clone();
        let chat_store = self.chat_store.clone();
        let runtime = self.runtime.clone();
        let shutdown = self.shutdown.clone();

        std::thread::spawn(move || {
            runtime.block_on(async move {
                // Also store sender in the async context
                {
                    let mut guard = ui_sender.lock().await;
                    *guard = Some(ui_tx.clone());
                }
                Self::run_client(
                    ui_tx,
                    client_handle,
                    calls,
                    chat_store,
                    ui_sender.clone(),
                    shutdown,
                )
                .await;
            });
        });

        Ok(ui_rx)
    }

    /// Internal async function to run the client
    async fn run_client(
        ui_tx: mpsc::UnboundedSender<UiEvent>,
        client_handle: ClientHandle,
        calls: CallRegistry,
        chat_store_handle: ChatStoreHandle,
        ui_sender: UiEventSender,
        shutdown: Arc<tokio::sync::Notify>,
    ) {
        // Device store + durable chat history share one SQLite file (one pool,
        // one WAL writer).
        let db_path = resolve_database_path();
        info!("Using database at {}", db_path);
        let backend = match SqliteStore::new(&db_path).await {
            Ok(store) => store,
            Err(e) => {
                error!("Failed to create SQLite backend: {}", e);
                let _ = ui_tx.send(UiEvent::Error(format!("Database error: {}", e)));
                return;
            }
        };
        let chat_store = match ChatStore::new(&backend).await {
            Ok(store) => store,
            Err(e) => {
                error!("Failed to open chat store: {}", e);
                let _ = ui_tx.send(UiEvent::Error(format!("Database error: {}", e)));
                return;
            }
        };
        {
            let mut guard = chat_store_handle.lock().await;
            *guard = Some(chat_store.clone());
        }
        info!("SQLite backend + chat store initialized.");

        let ui_tx_clone = ui_tx.clone();
        let calls_clone = calls.clone();
        let ui_sender_clone = ui_sender.clone();

        // Transport, HTTP client and runtime come from the default cargo
        // features (Tokio WebSocket, ureq, Tokio).
        let bot = match Bot::builder()
            .with_backend(backend)
            .on_event(move |event, client| {
                let ui_tx = ui_tx_clone.clone();
                let calls = calls_clone.clone();
                let ui_sender = ui_sender_clone.clone();
                async move {
                    Self::handle_event(event, client, ui_tx, calls, ui_sender).await;
                }
            })
            .build()
            .await
        {
            Ok(bot) => bot,
            Err(e) => {
                error!("Failed to build bot: {}", e);
                let _ = ui_tx.send(UiEvent::Error(format!("Connection failed: {}", e)));
                return;
            }
        };

        // Hydrate the UI from durable history before the network is even up
        // (bot.run() is what connects). The client is needed here so hydrated
        // JIDs normalize through the same PN->LID mapping live events use.
        match Self::load_history(&chat_store, &bot.client()).await {
            Ok(chats) if !chats.is_empty() => {
                let _ = ui_tx.send(UiEvent::HistoryLoaded { chats });
            }
            Ok(_) => {}
            Err(e) => warn!("Failed to load chat history: {e}"),
        }

        // The chat store materializes history straight off the event bus.
        bot.client().register_handler(chat_store.handler());

        // Re-hydrate the UI off the store's invalidation stream instead of raw
        // client events: changes are emitted only after commit, so a reload can
        // never observe pre-commit state (no flush barrier, no dispatch-order
        // dependency), and the debounce coalesces history-sync bursts into one
        // reload. HistoryLoaded merges, so re-sending is safe.
        Self::spawn_history_reloader(chat_store.subscribe(), chat_store.clone(), &bot, &ui_tx);

        // Store client reference for UI to use
        {
            let mut guard = client_handle.lock().await;
            *guard = Some(bot.client());
        }

        // Notify UI that init is complete
        let _ = ui_tx.send(UiEvent::InitComplete);

        // bot.run() reconnects internally, so on its own it only returns after
        // a logout; the shutdown signal is how a replaced client's thread gets
        // to exit (letting block_on return drops the runtime + SQLite pool).
        let client = bot.client();
        tokio::select! {
            _ = bot.run() => {}
            _ = shutdown.notified() => {
                // Graceful stop: flushes state and closes the transport. The
                // dropped run future is not awaited out instead, because a
                // disconnect() landing before run()'s first poll would be
                // clobbered by run's own is_running swap.
                client.disconnect().await;
            }
        }
    }

    /// Handle events from the WhatsApp client
    async fn handle_event(
        event: Arc<Event>,
        client: Arc<Client>,
        ui_tx: mpsc::UnboundedSender<UiEvent>,
        calls: CallRegistry,
        ui_sender: UiEventSender,
    ) {
        match &*event {
            Event::PairingQrCode(qr) => {
                info!("QR code received");
                let _ = ui_tx.send(UiEvent::QrCode {
                    code: qr.code.clone(),
                    timeout_secs: qr.timeout.as_secs(),
                });
            }
            Event::PairingCode(pair) => {
                info!("Pair code received: {}", pair.code);
                let _ = ui_tx.send(UiEvent::PairCode {
                    code: pair.code.clone(),
                    timeout_secs: pair.timeout.as_secs(),
                });
            }
            Event::PairSuccess(_) => {
                info!("Pairing successful, syncing...");
                let _ = ui_tx.send(UiEvent::PairSuccess);
            }
            Event::Connected(_) => {
                info!("Connected to WhatsApp!");
                let _ = ui_tx.send(UiEvent::Connected);
            }
            Event::LoggedOut(_) => {
                info!("Logged out from WhatsApp");
                let _ = ui_tx.send(UiEvent::Disconnected("Logged out".to_string()));
            }
            Event::IncomingCall(call) => match &call.action {
                CallAction::Offer {
                    call_id, is_video, ..
                } => {
                    if call.offline {
                        info!("Ignoring offline call {} (stale)", call_id);
                        return;
                    }
                    info!("Incoming call from {}", call.from);
                    let offer = Arc::new(call.clone());
                    calls
                        .pending
                        .lock()
                        .await
                        .insert(call_id.clone(), offer.clone());
                    let ui_call = IncomingCall::new(
                        call_id.clone(),
                        call.from.to_string(),
                        call.from.to_string(),
                        *is_video,
                        offer,
                    );
                    let _ = ui_tx.send(UiEvent::IncomingCall(ui_call));
                }
                CallAction::Accept { call_id, .. } => {
                    info!("Call {} accepted by peer", call_id);
                    let _ = ui_tx.send(UiEvent::CallAccepted(call_id.clone()));
                }
                CallAction::Reject { call_id, .. } => {
                    info!("Call {} rejected by peer", call_id);
                    calls.pending.lock().await.remove(call_id);
                    let _ = ui_tx.send(UiEvent::CallEnded(call_id.clone()));
                }
                CallAction::Terminate { call_id, .. } => {
                    info!("Call {} terminated by peer", call_id);
                    calls.pending.lock().await.remove(call_id);
                    if let Some(handle) = calls.active.lock().await.remove(call_id) {
                        tokio::spawn(async move { handle.hangup().await });
                    }
                    let _ = ui_tx.send(UiEvent::CallEnded(call_id.clone()));
                }
                _ => {}
            },
            Event::MissedCall(missed) => {
                info!("Missed call {} from {}", missed.call_id, missed.from);
                calls.pending.lock().await.remove(&missed.call_id);
                let _ = ui_tx.send(UiEvent::CallEnded(missed.call_id.clone()));
            }
            Event::CallEndedElsewhere(ended) => {
                info!("Call {} handled on another device", ended.call_id);
                calls.pending.lock().await.remove(&ended.call_id);
                let _ = ui_tx.send(UiEvent::CallEnded(ended.call_id.clone()));
            }
            Event::Messages(batch) => {
                for inbound in batch.iter() {
                    Self::handle_inbound_message(&inbound.message, &inbound.info, &client, &ui_tx)
                        .await;
                }
            }
            Event::Receipt(receipt) => {
                let Some(dominated_type) = (match &receipt.r#type {
                    ReceiptType::Read | ReceiptType::ReadSelf => Some(ReceiptType::Read),
                    ReceiptType::Played | ReceiptType::PlayedSelf => Some(ReceiptType::Played),
                    _ => None,
                }) else {
                    return;
                };

                debug!(
                    "Receipt {:?} for {} message(s) in {}",
                    dominated_type,
                    receipt.message_ids.len(),
                    receipt.source.chat
                );

                // Normalize the chat JID
                let normalized_chat_jid =
                    normalize_chat_jid(&client, &receipt.source.chat.to_string()).await;

                let _ = ui_tx.send(UiEvent::ReceiptReceived {
                    chat_jid: normalized_chat_jid,
                    message_ids: receipt.message_ids.clone(),
                    receipt_type: dominated_type,
                });
            }
            _ => {
                let _ = ui_sender; // silences unused when no branch needs it
            }
        }
    }

    /// One decrypted inbound message -> UiEvent (reaction or chat message).
    async fn handle_inbound_message(
        msg: &wa::Message,
        info: &wacore::types::message::MessageInfo,
        client: &Arc<Client>,
        ui_tx: &mpsc::UnboundedSender<UiEvent>,
    ) {
        // Use MessageExt to unwrap ephemeral/device_sent/view_once wrappers
        let base_msg = msg.get_base_message();

        // Check if this is a reaction message
        if let Some(reaction) = base_msg.reaction_message.as_option() {
            if let Some(key) = reaction.key.as_option()
                && let Some(target_id) = &key.id
            {
                let emoji = reaction.text.clone().unwrap_or_default();
                debug!(
                    "Reaction '{}' from {} on message {}",
                    emoji, info.source.sender, target_id
                );

                // Use remote_jid from key if available, otherwise use chat from info
                let chat_jid = key
                    .remote_jid
                    .clone()
                    .unwrap_or_else(|| info.source.chat.to_string());

                let normalized_chat_jid = normalize_chat_jid(client, &chat_jid).await;

                let _ = ui_tx.send(UiEvent::ReactionReceived {
                    chat_jid: normalized_chat_jid,
                    message_id: target_id.clone(),
                    sender: info.source.sender.to_string(),
                    emoji,
                });
            }
            return;
        }

        // Revokes/edits and other protocol stubs carry no displayable body;
        // the chat store materializes them durably (a reload shows the right
        // state), so don't fabricate a "[Media]" bubble under their own id.
        if base_msg.protocol_message.is_set() {
            return;
        }

        // Try to extract media content
        let media_result = Self::try_extract_media(base_msg, client).await;

        // Extract text content
        let content = msg
            .text_content()
            .map(|s| s.to_string())
            .or_else(|| msg.get_caption().map(|s| s.to_string()))
            .unwrap_or_else(|| {
                if media_result.is_some() {
                    String::new() // Empty for media-only messages
                } else {
                    "[Media]".to_string()
                }
            });

        let mut chat_message = ChatMessage {
            id: info.id.clone(),
            sender: info.source.sender.to_string(),
            sender_name: None, // Will be set in handle_message_received for groups
            content,
            timestamp: info.timestamp,
            is_from_me: info.source.is_from_me,
            is_read: false,
            media: None,
            reactions: std::collections::HashMap::new(),
            failed: false,
        };

        if let Some(media) = media_result {
            chat_message.media = Some(media);
        }

        // Normalize chat JID to LID if mapping exists, so the same user doesn't
        // appear as two chats when messages come from PN vs LID.
        let normalized_chat_jid = normalize_chat_jid(client, &info.source.chat.to_string()).await;

        let sender_name = (!info.push_name.is_empty()).then(|| info.push_name.clone());

        let _ = ui_tx.send(UiEvent::MessageReceived {
            chat_jid: normalized_chat_jid,
            message: Box::new(chat_message),
            sender_name,
        });
    }

    /// Helper to download media with logging
    async fn download_media<T: wacore::download::Downloadable>(
        client: &Arc<Client>,
        media: &T,
        media_name: &str,
    ) -> Option<Vec<u8>> {
        info!("Downloading {}...", media_name);
        match client.download(media).await {
            Ok(data) => {
                info!(
                    "{} downloaded successfully: {} bytes",
                    media_name,
                    data.len()
                );
                Some(data)
            }
            Err(e) => {
                warn!("Failed to download {}: {}", media_name, e);
                None
            }
        }
    }

    /// Try to extract and download media from a message
    async fn try_extract_media(msg: &wa::Message, _client: &Arc<Client>) -> Option<MediaContent> {
        // Check for sticker message
        if let Some(sticker) = effective_sticker(msg) {
            let mime = sticker
                .mimetype
                .clone()
                .unwrap_or_else(|| "image/webp".to_string());
            let downloadable = DownloadableBuilder {
                direct_path: sticker.direct_path.as_deref(),
                media_key: sticker.media_key.as_deref(),
                file_enc_sha256: sticker.file_enc_sha256.as_deref(),
                file_length: sticker.file_length,
                mime_type: &mime,
                duration_secs: None,
                download_type: DownloadMediaType::Sticker,
            }
            .build();
            // Same rule as the image path below: a failed eager download
            // degrades to the thumbnail (and stays retryable through the
            // download metadata) instead of the message losing its media.
            let (data, mime_type, is_animated, data_is_preview) =
                match Self::download_media(_client, sticker, "sticker").await {
                    Some(data) => (data, mime, sticker.is_animated.unwrap_or(false), false),
                    None => (
                        sticker
                            .png_thumbnail
                            .as_ref()
                            .filter(|t| !t.is_empty())
                            .cloned()
                            .unwrap_or_default(),
                        "image/png".to_string(),
                        false,
                        true,
                    ),
                };
            if data.is_empty() && downloadable.is_none() {
                return None;
            }
            info!(
                "Sticker: mime={}, is_animated={}, is_lottie={}, size={} bytes",
                mime_type,
                is_animated,
                sticker.is_lottie.unwrap_or(false),
                data.len()
            );
            return Some(MediaContent {
                media_type: MediaType::Sticker,
                data: Arc::new(data),
                mime_type,
                width: sticker.width,
                height: sticker.height,
                caption: None,
                file_name: None,
                downloadable,
                is_animated,
                duration_secs: None,
                data_is_preview,
            });
        }

        // Check for image message
        if let Some(image) = msg.image_message.as_option() {
            let downloadable = DownloadableBuilder {
                direct_path: image.direct_path.as_deref(),
                media_key: image.media_key.as_deref(),
                file_enc_sha256: image.file_enc_sha256.as_deref(),
                file_length: image.file_length,
                mime_type: image.mimetype.as_deref().unwrap_or("image/jpeg"),
                duration_secs: None,
                download_type: DownloadMediaType::Image,
            }
            .build();
            // A failed eager download keeps the metadata: the thumbnail shows
            // now and the full image stays retryable, instead of the message
            // degrading to a plain text row for the whole session.
            let (data, mime_type, data_is_preview) =
                match Self::download_media(_client, image, "image").await {
                    Some(data) => (
                        data,
                        image
                            .mimetype
                            .clone()
                            .unwrap_or_else(|| "image/jpeg".to_string()),
                        false,
                    ),
                    None => (
                        image
                            .jpeg_thumbnail
                            .as_ref()
                            .filter(|t| !t.is_empty())
                            .cloned()
                            .unwrap_or_default(),
                        "image/jpeg".to_string(),
                        true,
                    ),
                };
            if data.is_empty() && downloadable.is_none() {
                return None;
            }
            return Some(MediaContent {
                media_type: MediaType::Image,
                data: Arc::new(data),
                mime_type,
                width: image.width,
                height: image.height,
                caption: image.caption.clone(),
                file_name: None,
                downloadable,
                is_animated: false,
                duration_secs: None,
                data_is_preview,
            });
        }

        // Check for video message - store thumbnail for preview, metadata for
        // download. PTVs (round video notes) are the same proto type in a
        // different field and play like any other video.
        if let Some(video) = msg
            .ptv_message
            .as_option()
            .or(msg.video_message.as_option())
        {
            // Use thumbnail for display, or empty vec if none
            let thumbnail_data = video
                .jpeg_thumbnail
                .as_ref()
                .filter(|t| !t.is_empty())
                .cloned()
                .unwrap_or_default();

            // Build downloadable info using helper
            let downloadable = DownloadableBuilder {
                direct_path: video.direct_path.as_deref(),
                media_key: video.media_key.as_deref(),
                file_enc_sha256: video.file_enc_sha256.as_deref(),
                file_length: video.file_length,
                mime_type: video.mimetype.as_deref().unwrap_or("video/mp4"),
                duration_secs: video.seconds,
                download_type: DownloadMediaType::Video,
            }
            .build();

            // Only return if we have either thumbnail or downloadable info
            if !thumbnail_data.is_empty() || downloadable.is_some() {
                return Some(MediaContent {
                    media_type: MediaType::Video,
                    data: Arc::new(thumbnail_data),
                    mime_type: "image/jpeg".to_string(), // Thumbnail is JPEG
                    width: video.width,
                    height: video.height,
                    caption: video.caption.clone(),
                    file_name: None,
                    downloadable,
                    is_animated: false,
                    duration_secs: video.seconds,
                    data_is_preview: false,
                });
            }
        }

        // Check for audio message - lazy load, only download when user clicks play
        if let Some(audio) = msg.audio_message.as_option() {
            let default_mime = "audio/ogg; codecs=opus";
            let mime_type = audio.mimetype.as_deref().unwrap_or(default_mime);

            // Build downloadable info using helper
            let downloadable = DownloadableBuilder {
                direct_path: audio.direct_path.as_deref(),
                media_key: audio.media_key.as_deref(),
                file_enc_sha256: audio.file_enc_sha256.as_deref(),
                file_length: audio.file_length,
                mime_type,
                duration_secs: audio.seconds,
                download_type: DownloadMediaType::Audio,
            }
            .build();

            // Only return if we have downloadable info
            if downloadable.is_some() {
                return Some(MediaContent {
                    media_type: MediaType::Audio,
                    data: Arc::new(vec![]), // Empty until downloaded
                    mime_type: mime_type.to_string(),
                    width: None,
                    height: None,
                    caption: None,
                    file_name: None,
                    downloadable,
                    is_animated: false,
                    duration_secs: audio.seconds,
                    data_is_preview: false,
                });
            }
        }

        // Check for document message (no eager download, just metadata)
        if let Some(doc) = msg.document_message.as_option() {
            let mime = doc.mimetype.clone().unwrap_or_default();
            let downloadable = DownloadableBuilder {
                direct_path: doc.direct_path.as_deref(),
                media_key: doc.media_key.as_deref(),
                file_enc_sha256: doc.file_enc_sha256.as_deref(),
                file_length: doc.file_length,
                mime_type: &mime,
                duration_secs: None,
                download_type: DownloadMediaType::Document,
            }
            .build();
            return Some(MediaContent {
                media_type: MediaType::Document,
                data: Arc::new(vec![]),
                mime_type: mime,
                width: None,
                height: None,
                caption: doc.caption.clone(),
                file_name: doc.file_name.clone(),
                downloadable,
                is_animated: false,
                duration_secs: None,
                data_is_preview: false,
            });
        }

        None
    }

    /// Send a text message to a chat
    pub fn send_message(&self, jid_str: &str, content: &str, local_id: String) {
        let client_handle = self.client_handle.clone();
        let chat_store = self.chat_store.clone();
        let ui_sender = self.ui_sender.clone();
        let jid_str = jid_str.to_string();
        let content = content.to_string();
        let runtime = self.runtime.clone();

        std::thread::spawn(move || {
            runtime.block_on(async move {
                // Parse JID string
                let jid: Jid = match jid_str.parse() {
                    Ok(j) => j,
                    Err(e) => {
                        error!("Invalid JID '{}': {}", jid_str, e);
                        return;
                    }
                };

                // Clone the Arc and release the mutex: a slow network call
                // here must not queue every other client action behind it.
                let client = client_handle.lock().await.clone();
                if let Some(client) = client {
                    let message = wa::Message {
                        conversation: Some(content.clone()),
                        ..Default::default()
                    };

                    // Record BEFORE sending: the server ack event fires during
                    // send_message, so a row recorded after it would stay
                    // Pending forever (the ack precedes it in writer order).
                    let msg_id = client.generate_message_id();
                    // Receipts/reactions arrive keyed by this id; rename the
                    // optimistic bubble before they can race it.
                    notify_message_id(&ui_sender, &jid_str, local_id, &msg_id).await;
                    record_outgoing(&chat_store, &jid, &msg_id, &message).await;
                    let options = whatsapp_rust::SendOptions {
                        message_id: Some(msg_id.clone()),
                        ..Default::default()
                    };
                    match client
                        .send_message_with_options(jid.clone(), message, options)
                        .await
                    {
                        Ok(result) => {
                            info!("Message sent successfully: {}", result.message_id);
                        }
                        Err(e) => {
                            error!("Failed to send message {}: {}", msg_id, e);
                            mark_send_failed(&chat_store, &jid, &msg_id).await;
                            notify_send_failed(&ui_sender, &jid_str, &msg_id, e.to_string()).await;
                        }
                    }
                } else {
                    error!("Client not available for sending message");
                    // The bubble still carries its local id (no rename ran)
                    notify_send_failed(
                        &ui_sender,
                        &jid_str,
                        &local_id,
                        "client not available".to_string(),
                    )
                    .await;
                }
            });
        });
    }

    /// Download media using DownloadableMedia info
    /// Returns a oneshot receiver that will contain the result
    pub fn download_downloadable_media(
        &self,
        downloadable: DownloadableMedia,
    ) -> tokio::sync::oneshot::Receiver<Result<Vec<u8>, String>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let client_handle = self.client_handle.clone();
        let runtime = self.runtime.clone();

        std::thread::spawn(move || {
            runtime.block_on(async move {
                // Clone the Arc and release the mutex: a slow network call
                // here must not queue every other client action behind it.
                let client = client_handle.lock().await.clone();
                if let Some(client) = client {
                    info!(
                        "Downloading media: {} bytes expected",
                        downloadable.file_length
                    );
                    match client.download(&downloadable).await {
                        Ok(data) => {
                            info!("Media downloaded successfully: {} bytes", data.len());
                            let _ = tx.send(Ok(data));
                        }
                        Err(e) => {
                            error!("Failed to download media: {}", e);
                            let _ = tx.send(Err(e.to_string()));
                        }
                    }
                } else {
                    let _ = tx.send(Err("Client not available".to_string()));
                }
            });
        });

        rx
    }

    /// Send a PTT audio message to a chat
    pub fn send_audio_message(
        &self,
        jid_str: &str,
        audio_data: Vec<u8>,
        duration_secs: u32,
        waveform: Vec<u8>,
        local_id: String,
    ) {
        let chat_store = self.chat_store.clone();
        let ui_sender = self.ui_sender.clone();
        let client_handle = self.client_handle.clone();
        let jid_str = jid_str.to_string();
        let runtime = self.runtime.clone();

        std::thread::spawn(move || {
            runtime.block_on(async move {
                // Parse JID string
                let jid: Jid = match jid_str.parse() {
                    Ok(j) => j,
                    Err(e) => {
                        error!("Invalid JID '{}': {}", jid_str, e);
                        return;
                    }
                };

                // Clone the Arc and release the mutex: a slow network call
                // here must not queue every other client action behind it.
                let client = client_handle.lock().await.clone();
                if let Some(client) = client {
                    // Upload the audio file
                    let upload_result = match client
                        .upload(audio_data, DownloadMediaType::Audio, Default::default())
                        .await
                    {
                        Ok(resp) => resp,
                        Err(e) => {
                            error!("Failed to upload audio: {}", e);
                            // Bubble still carries the local id at this point.
                            notify_send_failed(&ui_sender, &jid_str, &local_id, e.to_string())
                                .await;
                            return;
                        }
                    };

                    info!("Audio uploaded successfully: {}", upload_result.url);

                    // Build the AudioMessage
                    let audio_message = wa::message::AudioMessage {
                        url: Some(upload_result.url),
                        direct_path: Some(upload_result.direct_path),
                        media_key: Some(upload_result.media_key.to_vec()),
                        file_sha256: Some(upload_result.file_sha256.to_vec()),
                        file_enc_sha256: Some(upload_result.file_enc_sha256.to_vec()),
                        file_length: Some(upload_result.file_length),
                        mimetype: Some("audio/ogg; codecs=opus".to_string()),
                        seconds: Some(duration_secs),
                        ptt: Some(true), // This marks it as a voice message
                        waveform: Some(waveform),
                        ..Default::default()
                    };

                    let message = wa::Message {
                        audio_message: buffa::MessageField::some(audio_message),
                        ..Default::default()
                    };

                    // Same ordering as the text path: record before sending so
                    // the ack can't precede the row in the writer queue.
                    let msg_id = client.generate_message_id();
                    notify_message_id(&ui_sender, &jid_str, local_id, &msg_id).await;
                    record_outgoing(&chat_store, &jid, &msg_id, &message).await;
                    let options = whatsapp_rust::SendOptions {
                        message_id: Some(msg_id.clone()),
                        ..Default::default()
                    };
                    match client
                        .send_message_with_options(jid.clone(), message, options)
                        .await
                    {
                        Ok(result) => {
                            info!("Audio message sent successfully: {}", result.message_id);
                        }
                        Err(e) => {
                            error!("Failed to send audio message {}: {}", msg_id, e);
                            mark_send_failed(&chat_store, &jid, &msg_id).await;
                            notify_send_failed(&ui_sender, &jid_str, &msg_id, e.to_string()).await;
                        }
                    }
                } else {
                    error!("Client not available for sending audio message");
                    // The bubble still carries its local id (no rename ran)
                    notify_send_failed(
                        &ui_sender,
                        &jid_str,
                        &local_id,
                        "client not available".to_string(),
                    )
                    .await;
                }
            });
        });
    }

    /// Send "composing" chat state (typing indicator)
    pub fn send_composing(&self, jid_str: &str) {
        let client_handle = self.client_handle.clone();
        let jid_str = jid_str.to_string();
        let runtime = self.runtime.clone();

        std::thread::spawn(move || {
            runtime.block_on(async move {
                let jid: Jid = match jid_str.parse() {
                    Ok(j) => j,
                    Err(e) => {
                        error!("Invalid JID '{}': {}", jid_str, e);
                        return;
                    }
                };

                let client = client_handle.lock().await.clone();
                if let Some(client) = client
                    && let Err(e) = client.chatstate().send_composing(&jid).await
                {
                    warn!("Failed to send composing state: {}", e);
                }
            });
        });
    }

    /// Send "paused" chat state (stopped typing)
    pub fn send_paused(&self, jid_str: &str) {
        let client_handle = self.client_handle.clone();
        let jid_str = jid_str.to_string();
        let runtime = self.runtime.clone();

        std::thread::spawn(move || {
            runtime.block_on(async move {
                let jid: Jid = match jid_str.parse() {
                    Ok(j) => j,
                    Err(e) => {
                        error!("Invalid JID '{}': {}", jid_str, e);
                        return;
                    }
                };

                let client = client_handle.lock().await.clone();
                if let Some(client) = client
                    && let Err(e) = client.chatstate().send_paused(&jid).await
                {
                    warn!("Failed to send paused state: {}", e);
                }
            });
        });
    }

    /// Send read receipts to mark messages as read
    ///
    /// # Arguments
    /// * `chat_jid_str` - The JID of the chat (e.g., "123456@s.whatsapp.net")
    /// * `messages` - List of (message_id, sender_jid_string) tuples
    pub fn send_read_receipts(&self, chat_jid_str: &str, messages: Vec<(String, String)>) {
        if messages.is_empty() {
            return;
        }

        let client_handle = self.client_handle.clone();
        let chat_jid_str = chat_jid_str.to_string();
        let runtime = self.runtime.clone();

        std::thread::spawn(move || {
            runtime.block_on(async move {
                // Parse chat JID
                let chat_jid: Jid = match chat_jid_str.parse() {
                    Ok(j) => j,
                    Err(e) => {
                        error!("Invalid chat JID '{}': {}", chat_jid_str, e);
                        return;
                    }
                };

                // Parse message sender JIDs, skipping invalid ones
                let parsed_messages: Vec<(String, Jid)> = messages
                    .into_iter()
                    .filter_map(|(msg_id, sender_str)| {
                        sender_str
                            .parse::<Jid>()
                            .inspect_err(|e| warn!("Invalid sender JID '{}': {}", sender_str, e))
                            .ok()
                            .map(|jid| (msg_id, jid))
                    })
                    .collect();

                if parsed_messages.is_empty() {
                    return;
                }

                // Only group/broadcast receipts carry a participant (matches
                // whatsmeow/WA Web); a plain DM receipt must not.
                let needs_participant = chat_jid.is_group()
                    || chat_jid.is_broadcast_list()
                    || chat_jid.is_status_broadcast();

                // Clone the Arc and release the mutex: a slow network call
                // here must not queue every other client action behind it.
                let client = client_handle.lock().await.clone();
                if let Some(client) = client {
                    // Group messages by sender, then send read receipts per sender
                    let mut by_sender: HashMap<Jid, Vec<String>> = HashMap::new();
                    for (msg_id, sender) in parsed_messages {
                        by_sender.entry(sender).or_default().push(msg_id);
                    }
                    for (sender, msg_ids) in by_sender {
                        let id_refs: Vec<&str> = msg_ids.iter().map(String::as_str).collect();
                        if let Err(e) = client
                            .mark_as_read(&chat_jid, needs_participant.then_some(&sender), &id_refs)
                            .await
                        {
                            warn!("Failed to mark messages as read: {}", e);
                        }
                    }
                } else {
                    error!("Client not available for sending read receipts");
                }
            });
        });
    }

    /// Durably clear a manual-unread mark. The store's `-1` sentinel is only
    /// cleared by a MarkChatAsRead app-state action (echoed back through the
    /// event stream), never by message receipts — without this the badge
    /// would come back on the next history reload.
    pub fn mark_chat_read(&self, chat_jid_str: &str) {
        let client_handle = self.client_handle.clone();
        let chat_jid_str = chat_jid_str.to_string();
        let runtime = self.runtime.clone();

        std::thread::spawn(move || {
            runtime.block_on(async move {
                let chat_jid: Jid = match chat_jid_str.parse() {
                    Ok(j) => j,
                    Err(e) => {
                        error!("Invalid chat JID '{}': {}", chat_jid_str, e);
                        return;
                    }
                };
                let Some(client) = client_handle.lock().await.clone() else {
                    error!("Client not available for marking chat read");
                    return;
                };
                if let Err(e) = client
                    .chat_actions()
                    .mark_chat_as_read(&chat_jid, true, None)
                    .await
                {
                    warn!("Failed to mark chat {} as read: {}", chat_jid, e);
                }
            });
        });
    }

    /// Accept an incoming call: signaling, callKey decrypt, relay connect and
    /// the audio engine are all inside `client.voip().accept(..)`; this side
    /// only supplies the cpal mic/speaker bridge.
    pub fn accept_call(&self, call_id: &str) {
        let client_handle = self.client_handle.clone();
        let calls = self.calls.clone();
        let ui_sender = self.ui_sender.clone();
        let call_id = call_id.to_string();
        let runtime = self.runtime.clone();

        std::thread::spawn(move || {
            runtime.block_on(async move {
                let Some(client) = client_handle.lock().await.clone() else {
                    error!("Client not available for accepting call");
                    return;
                };
                let Some(offer) = calls.pending.lock().await.remove(&call_id) else {
                    warn!("No pending offer for call {}", call_id);
                    return;
                };
                let (mic, speaker) = match (spawn_mic(), spawn_speaker()) {
                    (Ok(mic), Ok(speaker)) => (mic, speaker),
                    (mic, speaker) => {
                        let err = mic.err().or(speaker.err()).map(|e| e.to_string());
                        error!("Audio device setup failed: {:?}", err);
                        // The offer is consumed and no accept went out: reject
                        // so the caller stops ringing instead of waiting out
                        // the timeout.
                        if let Err(e) = client.voip().reject(&offer).await {
                            error!(
                                "Failed to reject call {} after audio failure: {}",
                                call_id, e
                            );
                        }
                        Self::notify_call_ended(&ui_sender, &call_id).await;
                        return;
                    }
                };
                match client
                    .voip()
                    .accept(&offer)
                    .audio(mic, speaker)
                    .start()
                    .await
                {
                    Ok(handle) => {
                        info!("Call {} media live", handle.call_id());
                        let handle = Arc::new(handle);
                        calls
                            .active
                            .lock()
                            .await
                            .insert(call_id.clone(), handle.clone());
                        Self::watch_call_end(handle, calls.clone(), ui_sender.clone());
                    }
                    Err(e) => {
                        error!("Failed to start call media for {}: {}", call_id, e);
                        Self::notify_call_ended(&ui_sender, &call_id).await;
                    }
                }
            });
        });
    }

    /// Decline an incoming call (sends the reject signaling).
    pub fn decline_call(&self, call_id: &str) {
        let client_handle = self.client_handle.clone();
        let calls = self.calls.clone();
        let call_id = call_id.to_string();
        let runtime = self.runtime.clone();

        std::thread::spawn(move || {
            runtime.block_on(async move {
                let Some(client) = client_handle.lock().await.clone() else {
                    error!("Client not available for declining call");
                    return;
                };
                let Some(offer) = calls.pending.lock().await.remove(&call_id) else {
                    warn!("No pending offer for call {}", call_id);
                    return;
                };
                match client.voip().reject(&offer).await {
                    Ok(()) => info!("Call {} declined", call_id),
                    Err(e) => error!("Failed to decline call {}: {}", call_id, e),
                }
            });
        });
    }

    /// Place an outgoing 1:1 voice call. Device discovery, callKey encrypt,
    /// offer send and the relay/engine lifecycle are inside
    /// `client.voip().call(..)`. Video calls are not supported by the library
    /// yet; `is_video` only shapes the UI.
    pub fn start_call(&self, recipient_jid_str: &str, is_video: bool, placeholder_id: String) {
        let client_handle = self.client_handle.clone();
        let calls = self.calls.clone();
        let ui_sender = self.ui_sender.clone();
        let recipient_jid = recipient_jid_str.to_string();
        let runtime = self.runtime.clone();

        if is_video {
            warn!("Video calls are not supported yet; placing a voice call");
        }

        std::thread::spawn(move || {
            runtime.block_on(async move {
                let notify_failure = |error: String| {
                    let ui_sender = ui_sender.clone();
                    let recipient_jid = recipient_jid.clone();
                    // A cancel may have landed for a call that will never
                    // start; consume the marker so the set doesn't grow.
                    let calls = calls.clone();
                    let placeholder_id = placeholder_id.clone();
                    async move {
                        calls.cancelled.lock().await.remove(&placeholder_id);
                        error!("Failed to start call to {}: {}", recipient_jid, error);
                        if let Some(tx) = ui_sender.lock().await.as_ref() {
                            let _ = tx.send(UiEvent::OutgoingCallFailed {
                                recipient_jid,
                                error,
                            });
                        }
                    }
                };

                let jid: Jid = match recipient_jid.parse() {
                    Ok(j) => j,
                    Err(e) => {
                        notify_failure(format!("invalid JID: {e}")).await;
                        return;
                    }
                };
                let Some(client) = client_handle.lock().await.clone() else {
                    notify_failure("client not available".to_string()).await;
                    return;
                };
                let (mic, speaker) = match (spawn_mic(), spawn_speaker()) {
                    (Ok(mic), Ok(speaker)) => (mic, speaker),
                    (mic, speaker) => {
                        let err = mic
                            .err()
                            .or(speaker.err())
                            .map(|e| e.to_string())
                            .unwrap_or_default();
                        notify_failure(format!("audio device setup failed: {err}")).await;
                        return;
                    }
                };

                match client.voip().call(&jid).audio(mic, speaker).start().await {
                    Ok(handle) => {
                        let call_id = handle.call_id().to_string();
                        // Cancelled while still connecting: the UI only knew
                        // the placeholder id, so honor it here.
                        if calls.cancelled.lock().await.remove(&placeholder_id) {
                            info!("Outgoing call {} cancelled before start", call_id);
                            handle.hangup().await;
                            return;
                        }
                        info!("Outgoing call {} to {} offered", call_id, recipient_jid);
                        let handle = Arc::new(handle);
                        calls
                            .active
                            .lock()
                            .await
                            .insert(call_id.clone(), handle.clone());
                        Self::watch_call_end(handle, calls.clone(), ui_sender.clone());
                        if let Some(tx) = ui_sender.lock().await.as_ref() {
                            let _ = tx.send(UiEvent::OutgoingCallStarted {
                                call_id,
                                recipient_jid,
                            });
                        }
                    }
                    Err(e) => notify_failure(e.to_string()).await,
                }
            });
        });
    }

    /// Hang up / cancel a call we started or answered.
    pub fn cancel_call(&self, call_id: &str) {
        let calls = self.calls.clone();
        let call_id = call_id.to_string();
        let runtime = self.runtime.clone();

        std::thread::spawn(move || {
            runtime.block_on(async move {
                // Still ringing and never answered: nothing live to hang up.
                calls.pending.lock().await.remove(&call_id);
                if let Some(handle) = calls.active.lock().await.remove(&call_id) {
                    handle.hangup().await;
                    info!("Call {} hung up", call_id);
                } else {
                    // No handle yet (start_call still connecting under a UI
                    // placeholder id): remember the cancel so it lands.
                    debug!("cancel_call: no live handle for {}, deferring", call_id);
                    calls.cancelled.lock().await.insert(call_id);
                }
            });
        });
    }

    /// Mute or unmute the microphone of a live call.
    #[allow(dead_code)]
    pub fn set_call_muted(&self, call_id: &str, muted: bool) {
        let calls = self.calls.clone();
        let call_id = call_id.to_string();
        let runtime = self.runtime.clone();
        runtime.spawn(async move {
            if let Some(handle) = calls.active.lock().await.get(&call_id) {
                handle.set_muted(muted);
            }
        });
    }

    /// Watch a live call until it ends (peer hangup, network loss, local
    /// hangup) and clear it from the registry + UI.
    fn watch_call_end(handle: Arc<CallHandle>, calls: CallRegistry, ui_sender: UiEventSender) {
        tokio::spawn(async move {
            handle.wait_ended().await;
            let call_id = handle.call_id().to_string();
            calls.active.lock().await.remove(&call_id);
            Self::notify_call_ended(&ui_sender, &call_id).await;
        });
    }

    async fn notify_call_ended(ui_sender: &UiEventSender, call_id: &str) {
        if let Some(tx) = ui_sender.lock().await.as_ref() {
            let _ = tx.send(UiEvent::CallEnded(call_id.to_string()));
        }
    }
}

impl WhatsAppClient {
    const HISTORY_CHAT_LIMIT: i64 = 100;
    const HISTORY_MESSAGES_PER_CHAT: i64 = 50;
    /// Quiet window before reloading: one history-sync chunk commits as many
    /// write batches, each emitting a change; reload once per burst.
    const RELOAD_DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(200);

    /// One task for the whole session: chat-store invalidations -> debounced
    /// load_history -> HistoryLoaded. Exits when the store or the UI goes away.
    fn spawn_history_reloader(
        mut changes: tokio::sync::broadcast::Receiver<whatsapp_rust_chat_store::StoreChange>,
        chat_store: Arc<ChatStore>,
        bot: &Bot,
        ui_tx: &mpsc::UnboundedSender<UiEvent>,
    ) {
        use tokio::sync::broadcast::error::RecvError;
        use whatsapp_rust_chat_store::StoreChange;

        let client = bot.client();
        let ui_tx = ui_tx.clone();
        tokio::spawn(async move {
            let mut open = true;
            while open {
                match changes.recv().await {
                    // Contacts too: a push-name landing after the chat row
                    // must refresh chats stuck on the JID placeholder.
                    Ok(
                        StoreChange::Chats | StoreChange::Messages { .. } | StoreChange::Contacts,
                    ) => {}
                    // Missed changes are covered by the full reload below.
                    Err(RecvError::Lagged(_)) => {}
                    Err(RecvError::Closed) => break,
                }
                // Drain the burst; a quiet window flushes the reload.
                loop {
                    match tokio::time::timeout(Self::RELOAD_DEBOUNCE, changes.recv()).await {
                        Ok(Ok(_)) | Ok(Err(RecvError::Lagged(_))) => continue,
                        Ok(Err(RecvError::Closed)) => {
                            // Reload once more: these changes were committed.
                            open = false;
                            break;
                        }
                        Err(_) => break,
                    }
                }
                match Self::load_history(&chat_store, &client).await {
                    Ok(chats) if !chats.is_empty() => {
                        if ui_tx.send(UiEvent::HistoryLoaded { chats }).is_err() {
                            break;
                        }
                    }
                    Ok(_) => {}
                    Err(e) => warn!("failed to reload history after store change: {e}"),
                }
            }
        });
    }

    /// Build the UI chat list from the durable store: chats in display order,
    /// each with its most recent page of messages. Media bodies are not
    /// hydrated here (the proto is in the store; download stays on demand).
    async fn load_history(
        chat_store: &Arc<ChatStore>,
        client: &Arc<Client>,
    ) -> Result<Vec<crate::state::Chat>, whatsapp_rust_chat_store::ChatStoreError> {
        let entries = chat_store.chats(false, Self::HISTORY_CHAT_LIMIT).await?;
        let mut chats: Vec<crate::state::Chat> = Vec::with_capacity(entries.len());
        // Sender-name lookups memoized across the whole load: group pages
        // repeat the same handful of senders many times over.
        let mut sender_names: HashMap<String, Option<String>> = HashMap::new();
        for entry in entries {
            // Same PN->LID mapping live events go through, or the restored
            // chat and the next live message split into two conversations.
            // A PN/LID pair of stored rows collapses into one chat: the most
            // recently active row (entries arrive in display order) keeps the
            // metadata, the older row's messages merge in.
            let jid_str = normalize_chat_jid(client, &entry.jid.to_string()).await;
            if let Some(existing) = chats.iter_mut().find(|c| c.jid == jid_str) {
                let mut page = chat_store
                    .messages(&entry.jid, None, Self::HISTORY_MESSAGES_PER_CHAT)
                    .await?;
                page.reverse();
                let mut msgs: Vec<ChatMessage> =
                    page.into_iter().map(stored_to_chat_message).collect();
                Self::hydrate_reactions(chat_store, &entry.jid, &mut msgs).await;
                if existing.is_group {
                    Self::hydrate_sender_names(chat_store, &mut msgs, &mut sender_names).await;
                }
                // The rows are distinct messages, so this side's unread state
                // adds to the merged chat: fold the counter in and un-read its
                // tail so opening the chat still sends those receipts.
                let mut remaining = entry.unread_count.max(0) as u32;
                for msg in msgs.iter_mut().rev() {
                    if remaining == 0 {
                        break;
                    }
                    if !msg.is_from_me {
                        msg.is_read = false;
                        remaining -= 1;
                    }
                }
                for msg in msgs {
                    existing.insert_history_message(msg);
                }
                existing.unread_count += entry.unread_count.max(0) as u32;
                existing.manually_unread |= entry.unread_count < 0;
                // The kept row can still wear the JID placeholder while this
                // older row (or its contact) knows the display name.
                let placeholder = existing.jid.split('@').next().unwrap_or(&existing.jid);
                if existing.name == placeholder {
                    let mut name = entry.name.clone();
                    if name.is_none()
                        && let Ok(Some(contact)) = chat_store.contact(&entry.jid).await
                    {
                        name = contact.display_name().map(str::to_owned);
                    }
                    if let Some(name) = name.filter(|n| n != placeholder) {
                        existing.name = name;
                    }
                }
                continue;
            }
            let mut name = entry.name.clone();
            if name.is_none()
                && let Ok(Some(contact)) = chat_store.contact(&entry.jid).await
            {
                name = contact.display_name().map(str::to_owned);
            }
            let mut chat = match name {
                Some(name) => crate::state::Chat::with_name(jid_str, name),
                None => crate::state::Chat::new(jid_str),
            };
            chat.unread_count = entry.unread_count.max(0) as u32;
            // -1 = manually marked unread (WA Web convention); .max(0) above
            // must not silently eat the flag.
            chat.manually_unread = entry.unread_count < 0;
            chat.last_message = entry.last_message_preview.clone();
            chat.last_message_time = entry.last_message_at;

            let mut page = chat_store
                .messages(&entry.jid, None, Self::HISTORY_MESSAGES_PER_CHAT)
                .await?;
            page.reverse(); // store returns newest-first; the UI renders oldest-first
            chat.messages = page.into_iter().map(stored_to_chat_message).collect();
            Self::hydrate_reactions(chat_store, &entry.jid, &mut chat.messages).await;
            if chat.is_group {
                Self::hydrate_sender_names(chat_store, &mut chat.messages, &mut sender_names).await;
            }
            // The newest `unread_count` incoming messages are the unread ones;
            // select_chat only sends read receipts for !is_read, so hydrated
            // unread must not come up pre-read.
            let mut remaining = chat.unread_count;
            for msg in chat.messages.iter_mut().rev() {
                if remaining == 0 {
                    break;
                }
                if !msg.is_from_me {
                    msg.is_read = false;
                    remaining -= 1;
                }
            }
            chats.push(chat);
        }
        Ok(chats)
    }

    /// Reactions live in their own table, so hydrated messages come out with
    /// an empty map; fold the stored rows back in. Per-message point lookups:
    /// the store exposes no per-chat batch query. Best-effort: one bad row
    /// must not abort the whole history load and blank the chat list.
    async fn hydrate_reactions(
        chat_store: &Arc<ChatStore>,
        chat_jid: &Jid,
        msgs: &mut [ChatMessage],
    ) {
        for msg in msgs.iter_mut() {
            // The store keeps one row per sender (latest wins), matching the
            // live add_reaction semantics, so a plain rebuild is enough.
            let entries = match chat_store.reactions(chat_jid, &msg.id).await {
                Ok(entries) => entries,
                Err(e) => {
                    warn!("failed to hydrate reactions for {}: {e}", msg.id);
                    continue;
                }
            };
            for entry in entries {
                msg.reactions
                    .entry(entry.emoji)
                    .or_default()
                    .push(entry.sender_jid.to_string());
            }
        }
    }

    /// Group bubbles label their sender, but hydrated rows don't carry the
    /// push name the live path attaches; resolve it from the contacts table.
    /// `cache` memoizes per sender JID (misses included) so a page never pays
    /// more than one query per unique sender. Best-effort like reactions: a
    /// failed lookup logs and the bubble falls back to the JID label.
    async fn hydrate_sender_names(
        chat_store: &Arc<ChatStore>,
        msgs: &mut [ChatMessage],
        cache: &mut HashMap<String, Option<String>>,
    ) {
        for msg in msgs.iter_mut() {
            if msg.is_from_me || msg.sender_name.is_some() {
                continue;
            }
            if !cache.contains_key(&msg.sender) {
                let name = match msg.sender.parse::<Jid>() {
                    Ok(jid) => match chat_store.contact(&jid).await {
                        Ok(contact) => contact.and_then(|c| c.display_name().map(str::to_owned)),
                        Err(e) => {
                            warn!("failed to hydrate sender name for {}: {e}", msg.sender);
                            None
                        }
                    },
                    Err(_) => None,
                };
                cache.insert(msg.sender.clone(), name);
            }
            msg.sender_name = cache.get(&msg.sender).cloned().flatten();
        }
    }
}

/// Convert a durable store row into the UI message model. Media stays
/// download-on-demand (the encoded proto lives in the store if needed later).
/// Media metadata (thumbnail + download info) from a message proto, without
/// downloading anything. Shared by hydration; the live path additionally
/// downloads images/stickers eagerly.
fn media_metadata(msg: &wa::Message) -> Option<MediaContent> {
    if let Some(sticker) = effective_sticker(msg) {
        let mime = sticker
            .mimetype
            .clone()
            .unwrap_or_else(|| "image/webp".to_string());
        let downloadable = DownloadableBuilder {
            direct_path: sticker.direct_path.as_deref(),
            media_key: sticker.media_key.as_deref(),
            file_enc_sha256: sticker.file_enc_sha256.as_deref(),
            file_length: sticker.file_length,
            mime_type: &mime,
            duration_secs: None,
            download_type: DownloadMediaType::Sticker,
        }
        .build()?;
        return Some(MediaContent {
            media_type: MediaType::Sticker,
            data: Arc::new(vec![]),
            mime_type: mime.clone(),
            width: sticker.width,
            height: sticker.height,
            caption: None,
            file_name: None,
            downloadable: Some(downloadable),
            is_animated: sticker.is_animated.unwrap_or(false),
            duration_secs: None,
            data_is_preview: false,
        });
    }
    if let Some(image) = msg.image_message.as_option() {
        let downloadable = DownloadableBuilder {
            direct_path: image.direct_path.as_deref(),
            media_key: image.media_key.as_deref(),
            file_enc_sha256: image.file_enc_sha256.as_deref(),
            file_length: image.file_length,
            mime_type: image.mimetype.as_deref().unwrap_or("image/jpeg"),
            duration_secs: None,
            download_type: DownloadMediaType::Image,
        }
        .build();
        let thumbnail = image
            .jpeg_thumbnail
            .as_ref()
            .filter(|t| !t.is_empty())
            .cloned()
            .unwrap_or_default();
        if thumbnail.is_empty() && downloadable.is_none() {
            return None;
        }
        // Hydrated rows carry only the thumbnail; flag it so the renderer
        // keeps offering the full download instead of treating it as final
        let data_is_preview = !thumbnail.is_empty() && downloadable.is_some();
        return Some(MediaContent {
            media_type: MediaType::Image,
            data: Arc::new(thumbnail),
            mime_type: "image/jpeg".to_string(),
            width: image.width,
            height: image.height,
            caption: image.caption.clone(),
            file_name: None,
            downloadable,
            is_animated: false,
            duration_secs: None,
            data_is_preview,
        });
    }
    // PTVs (round video notes) are VideoMessage in a different field.
    if let Some(video) = msg
        .ptv_message
        .as_option()
        .or(msg.video_message.as_option())
    {
        let downloadable = DownloadableBuilder {
            direct_path: video.direct_path.as_deref(),
            media_key: video.media_key.as_deref(),
            file_enc_sha256: video.file_enc_sha256.as_deref(),
            file_length: video.file_length,
            mime_type: video.mimetype.as_deref().unwrap_or("video/mp4"),
            duration_secs: video.seconds,
            download_type: DownloadMediaType::Video,
        }
        .build();
        let thumbnail = video
            .jpeg_thumbnail
            .as_ref()
            .filter(|t| !t.is_empty())
            .cloned()
            .unwrap_or_default();
        if thumbnail.is_empty() && downloadable.is_none() {
            return None;
        }
        return Some(MediaContent {
            media_type: MediaType::Video,
            data: Arc::new(thumbnail),
            mime_type: "image/jpeg".to_string(),
            width: video.width,
            height: video.height,
            caption: video.caption.clone(),
            file_name: None,
            downloadable,
            is_animated: false,
            duration_secs: video.seconds,
            data_is_preview: false,
        });
    }
    if let Some(audio) = msg.audio_message.as_option() {
        let mime = audio
            .mimetype
            .clone()
            .unwrap_or_else(|| "audio/ogg; codecs=opus".to_string());
        let downloadable = DownloadableBuilder {
            direct_path: audio.direct_path.as_deref(),
            media_key: audio.media_key.as_deref(),
            file_enc_sha256: audio.file_enc_sha256.as_deref(),
            file_length: audio.file_length,
            mime_type: &mime,
            duration_secs: audio.seconds,
            download_type: DownloadMediaType::Audio,
        }
        .build()?;
        return Some(MediaContent {
            media_type: MediaType::Audio,
            data: Arc::new(vec![]),
            mime_type: mime.clone(),
            width: None,
            height: None,
            caption: None,
            file_name: None,
            downloadable: Some(downloadable),
            is_animated: false,
            duration_secs: audio.seconds,
            data_is_preview: false,
        });
    }
    if let Some(doc) = msg.document_message.as_option() {
        let mime = doc.mimetype.clone().unwrap_or_default();
        let downloadable = DownloadableBuilder {
            direct_path: doc.direct_path.as_deref(),
            media_key: doc.media_key.as_deref(),
            file_enc_sha256: doc.file_enc_sha256.as_deref(),
            file_length: doc.file_length,
            mime_type: &mime,
            duration_secs: None,
            download_type: DownloadMediaType::Document,
        }
        .build();
        return Some(MediaContent {
            media_type: MediaType::Document,
            data: Arc::new(vec![]),
            mime_type: mime,
            width: None,
            height: None,
            caption: doc.caption.clone(),
            file_name: doc.file_name.clone(),
            downloadable,
            is_animated: false,
            duration_secs: None,
            data_is_preview: false,
        });
    }
    None
}

/// Some animated stickers arrive wrapped in the `lottie_sticker_message`
/// future-proof envelope instead of the top-level `sticker_message`.
fn effective_sticker(msg: &wa::Message) -> Option<&wa::message::StickerMessage> {
    msg.sticker_message.as_option().or_else(|| {
        msg.lottie_sticker_message
            .as_option()
            .and_then(|w| w.message.as_option())
            .and_then(|m| m.sticker_message.as_option())
    })
}

fn stored_to_chat_message(stored: whatsapp_rust_chat_store::StoredMessage) -> ChatMessage {
    // The stored proto still carries the media envelope: hydrate thumbnails +
    // download info so historical media renders and stays fetchable, instead
    // of degrading to a [kind] text row until a live redelivery.
    let media = (!stored.revoked)
        .then_some(stored.message.as_deref())
        .flatten()
        .and_then(|m| media_metadata(m.get_base_message()));
    let content = match (&stored.text, stored.revoked) {
        (_, true) => "[Message deleted]".to_string(),
        (Some(text), _) => text.clone(),
        (None, _) if media.is_some() => String::new(),
        (None, _) => format!("[{}]", stored.kind.as_str()),
    };
    // Outgoing ticks come from the stored delivery status; incoming default
    // to read and load_history un-reads the chat's unread tail (per-incoming
    // read state lives on the chat cursor, not the row).
    let is_read = if stored.from_me {
        matches!(
            stored.status,
            whatsapp_rust_chat_store::MessageStatus::Read
                | whatsapp_rust_chat_store::MessageStatus::Played
        )
    } else {
        true
    };
    ChatMessage {
        id: stored.id,
        sender: stored.sender_jid.to_string(),
        sender_name: None,
        content,
        timestamp: stored.timestamp,
        is_from_me: stored.from_me,
        is_read,
        media,
        reactions: std::collections::HashMap::new(),
        // Error is terminal for from_me rows (nack or local send failure), so
        // hydration restores the failure indicator instead of grey ticks.
        failed: stored.from_me && stored.status == whatsapp_rust_chat_store::MessageStatus::Error,
    }
}

/// Tell the UI which real id a just-sent optimistic bubble got.
async fn notify_message_id(
    ui_sender: &UiEventSender,
    chat_jid: &str,
    local_id: String,
    message_id: &str,
) {
    if let Some(tx) = ui_sender.lock().await.as_ref() {
        let _ = tx.send(UiEvent::MessageIdAssigned {
            chat_jid: chat_jid.to_string(),
            local_id,
            message_id: message_id.to_string(),
        });
    }
}

/// Tell the UI a send failed so the bubble doesn't sit pending forever.
async fn notify_send_failed(
    ui_sender: &UiEventSender,
    chat_jid: &str,
    message_id: &str,
    reason: String,
) {
    if let Some(tx) = ui_sender.lock().await.as_ref() {
        let _ = tx.send(UiEvent::SendFailed {
            chat_jid: chat_jid.to_string(),
            message_id: message_id.to_string(),
            reason,
        });
    }
}

/// Best-effort durable record of a message this client just sent; the UI's
/// optimistic bubble is independent of this.
async fn record_outgoing(
    chat_store: &ChatStoreHandle,
    jid: &Jid,
    message_id: &str,
    message: &wa::Message,
) {
    if let Some(store) = chat_store.lock().await.as_ref()
        && let Err(e) = store.record_outgoing(jid, message_id, message, wacore::time::now_utc())
    {
        warn!("Failed to record outgoing message {}: {e}", message_id);
    }
}

/// Best-effort failure mark on the durable row a client-side send error
/// orphans at Pending (no server nack will come to fail it), so a restart
/// hydrates the bubble with its failure indicator instead of grey ticks.
async fn mark_send_failed(chat_store: &ChatStoreHandle, jid: &Jid, message_id: &str) {
    if let Some(store) = chat_store.lock().await.as_ref()
        && let Err(e) = store.mark_send_failed(jid, message_id)
    {
        warn!("Failed to mark send {} as failed: {e}", message_id);
    }
}

/// Map a PN chat JID to its LID form when a mapping is known, so the same user
/// doesn't split into two chats (PN vs LID addressing).
async fn normalize_chat_jid(client: &Client, jid_str: &str) -> String {
    let Ok(jid) = jid_str.parse::<Jid>() else {
        return jid_str.to_string();
    };
    if !jid.is_pn() {
        return jid_str.to_string();
    }
    match client.get_lid_pn_entry(&jid).await {
        Ok(Some(entry)) => format!("{}@lid", entry.lid),
        _ => jid_str.to_string(),
    }
}

impl Default for WhatsAppClient {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for WhatsAppClient {
    fn drop(&mut self) {
        // A dropped wrapper can never be shut down explicitly anymore; free
        // its background thread instead of leaking the runtime + DB pool.
        self.shutdown.notify_one();
    }
}

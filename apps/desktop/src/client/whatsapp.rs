//! WhatsApp client wrapper for UI integration

use std::collections::HashMap;
use std::sync::Arc;

use log::{debug, error, info, warn};
use tokio::sync::{Mutex, mpsc};
use wacore::proto_helpers::MessageExt;
use wacore::types::call::{CallAction, IncomingCall as WaIncomingCall};
use wacore::types::events::Event;
use wacore::types::presence::ReceiptType;
use wacore_binary::jid::Jid;
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
            started: false,
        }
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

        std::thread::spawn(move || {
            runtime.block_on(async move {
                // Also store sender in the async context
                {
                    let mut guard = ui_sender.lock().await;
                    *guard = Some(ui_tx.clone());
                }
                Self::run_client(ui_tx, client_handle, calls, chat_store, ui_sender.clone()).await;
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
    ) {
        // Device store + durable chat history share one SQLite file (one pool,
        // one WAL writer).
        let backend = match SqliteStore::new("whatsapp.db").await {
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
        let chat_store_events = chat_store.clone();

        // Transport, HTTP client and runtime come from the default cargo
        // features (Tokio WebSocket, ureq, Tokio).
        let bot = match Bot::builder()
            .with_backend(backend)
            .on_event(move |event, client| {
                let ui_tx = ui_tx_clone.clone();
                let calls = calls_clone.clone();
                let ui_sender = ui_sender_clone.clone();
                let chat_store = chat_store_events.clone();
                async move {
                    Self::handle_event(event, client, ui_tx, calls, ui_sender, chat_store).await;
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

        // Store client reference for UI to use
        {
            let mut guard = client_handle.lock().await;
            *guard = Some(bot.client());
        }

        // Notify UI that init is complete
        let _ = ui_tx.send(UiEvent::InitComplete);

        bot.run().await;
    }

    /// Handle events from the WhatsApp client
    async fn handle_event(
        event: Arc<Event>,
        client: Arc<Client>,
        ui_tx: mpsc::UnboundedSender<UiEvent>,
        calls: CallRegistry,
        ui_sender: UiEventSender,
        chat_store: Arc<ChatStore>,
    ) {
        match &*event {
            Event::HistorySync(_) => {
                // The chat store enqueued this same event synchronously during
                // dispatch; flush() is the barrier that its materialization
                // committed. HistoryLoaded only adds chats the UI doesn't have,
                // so re-sending after each chunk is safe.
                if let Err(e) = chat_store.flush().await {
                    warn!("history sync flush failed: {e}");
                    return;
                }
                match Self::load_history(&chat_store, &client).await {
                    Ok(chats) if !chats.is_empty() => {
                        let _ = ui_tx.send(UiEvent::HistoryLoaded { chats });
                    }
                    Ok(_) => {}
                    Err(e) => warn!("failed to reload history after sync: {e}"),
                }
            }
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
        if let Some(sticker) = msg.sticker_message.as_option()
            && let Some(data) = Self::download_media(_client, sticker, "sticker").await
        {
            let is_animated = sticker.is_animated.unwrap_or(false);
            let is_lottie = sticker.is_lottie.unwrap_or(false);
            let mime = sticker
                .mimetype
                .clone()
                .unwrap_or_else(|| "image/webp".to_string());
            info!(
                "Sticker: mime={}, is_animated={}, is_lottie={}, size={} bytes",
                mime,
                is_animated,
                is_lottie,
                data.len()
            );
            return Some(MediaContent {
                media_type: MediaType::Sticker,
                data: Arc::new(data),
                mime_type: mime,
                width: sticker.width,
                height: sticker.height,
                caption: None,
                downloadable: None,
                is_animated,
                duration_secs: None,
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
            let (data, mime_type) = match Self::download_media(_client, image, "image").await {
                Some(data) => (
                    data,
                    image
                        .mimetype
                        .clone()
                        .unwrap_or_else(|| "image/jpeg".to_string()),
                ),
                None => (
                    image
                        .jpeg_thumbnail
                        .as_ref()
                        .filter(|t| !t.is_empty())
                        .cloned()
                        .unwrap_or_default(),
                    "image/jpeg".to_string(),
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
                downloadable,
                is_animated: false,
                duration_secs: None,
            });
        }

        // Check for video message - store thumbnail for preview, metadata for download
        if let Some(video) = msg.video_message.as_option() {
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
                    downloadable,
                    is_animated: false,
                    duration_secs: video.seconds,
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
                    downloadable,
                    is_animated: false,
                    duration_secs: audio.seconds,
                });
            }
        }

        // Check for document message (no download, just metadata)
        if let Some(doc) = msg.document_message.as_option() {
            return Some(MediaContent {
                media_type: MediaType::Document,
                data: Arc::new(vec![]),
                mime_type: doc.mimetype.clone().unwrap_or_default(),
                width: None,
                height: None,
                caption: doc.caption.clone(),
                downloadable: None,
                is_animated: false,
                duration_secs: None,
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
                        }
                    }
                } else {
                    error!("Client not available for sending message");
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
                        }
                    }
                } else {
                    error!("Client not available for sending audio message");
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
                            .mark_as_read(&chat_jid, Some(&sender), &id_refs)
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

    /// Build the UI chat list from the durable store: chats in display order,
    /// each with its most recent page of messages. Media bodies are not
    /// hydrated here (the proto is in the store; download stays on demand).
    async fn load_history(
        chat_store: &Arc<ChatStore>,
        client: &Arc<Client>,
    ) -> Result<Vec<crate::state::Chat>, whatsapp_rust_chat_store::ChatStoreError> {
        let entries = chat_store.chats(false, Self::HISTORY_CHAT_LIMIT).await?;
        let mut chats: Vec<crate::state::Chat> = Vec::with_capacity(entries.len());
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
}

/// Convert a durable store row into the UI message model. Media stays
/// download-on-demand (the encoded proto lives in the store if needed later).
/// Media metadata (thumbnail + download info) from a message proto, without
/// downloading anything. Shared by hydration; the live path additionally
/// downloads images/stickers eagerly.
fn media_metadata(msg: &wa::Message) -> Option<MediaContent> {
    if let Some(sticker) = msg.sticker_message.as_option() {
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
            downloadable: Some(downloadable),
            is_animated: sticker.is_animated.unwrap_or(false),
            duration_secs: None,
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
        return Some(MediaContent {
            media_type: MediaType::Image,
            data: Arc::new(thumbnail),
            mime_type: "image/jpeg".to_string(),
            width: image.width,
            height: image.height,
            caption: image.caption.clone(),
            downloadable,
            is_animated: false,
            duration_secs: None,
        });
    }
    if let Some(video) = msg.video_message.as_option() {
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
            downloadable,
            is_animated: false,
            duration_secs: video.seconds,
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
            downloadable: Some(downloadable),
            is_animated: false,
            duration_secs: audio.seconds,
        });
    }
    if let Some(doc) = msg.document_message.as_option() {
        return Some(MediaContent {
            media_type: MediaType::Document,
            data: Arc::new(vec![]),
            mime_type: doc.mimetype.clone().unwrap_or_default(),
            width: None,
            height: None,
            caption: doc.caption.clone(),
            downloadable: None,
            is_animated: false,
            duration_secs: None,
        });
    }
    None
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

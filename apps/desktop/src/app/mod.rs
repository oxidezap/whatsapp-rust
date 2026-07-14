//! Main WhatsApp application state and logic
//!
//! This module is being refactored into submodules for better organization:
//! - `chats`: Chat list management, selection, search, keyboard navigation
//! - `media`: Media handling (PTT recording state)
//! - `messages`: Message list caching and height calculation
//! - `calls`: Call state management (incoming/outgoing)

mod calls;
mod chats;
mod media;
mod messages;

pub use chats::ChatListCache;
pub use media::RecordingState;
pub use messages::MessageListCache;

use calls::CallState;

use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

use indexmap::IndexMap;

use gpui::{
    App, Context, Entity, FocusHandle, Focusable, Image, KeyBinding, ScrollStrategy, Task,
    WeakEntity, Window, actions, prelude::*,
};
use gpui_component::VirtualListScrollHandle;
use gpui_component::input::InputState;

// Define our own actions since gpui-component's actions module is private
actions!(chat_list, [SelectUp, SelectDown]);

use crate::components::{InputAreaEvent, InputAreaView};
use log::{error, info, warn};
use wacore_binary::jid::{Jid, JidExt, observe_str};

use crate::audio::{AudioPlayer, AudioRecorder, encode_to_opus_ogg, generate_waveform};
use crate::client::{ReadBoundary, WhatsAppClient};
use crate::responsive::{MobilePanel, ResponsiveLayout};
use crate::state::{
    AppState, CachedQrCode, Chat, ChatMessage, DownloadableMedia, IncomingCall, MediaContent,
    MediaType, OutgoingCall, ReceiptType, UiEvent,
};
use crate::utils::mime_to_image_format;
use crate::video::{StreamingVideoDecoder, VideoPlayer, VideoPlayerState};
use crate::views::pairing::generate_qr_png;
use crate::views::{
    render_connected_view, render_connecting_view, render_error_view, render_loading_view,
    render_pairing_view, render_syncing_view,
};

// ChatListCache is now in chats.rs and re-exported above
// RecordingState is now in media/mod.rs and re-exported above
// MessageListCache is now in messages.rs and re-exported above

/// Key context for chat list keyboard navigation
const CHAT_LIST_CONTEXT: &str = "ChatList";

/// Search debounce delay in milliseconds
const SEARCH_DEBOUNCE_MS: u64 = 150;

/// Maximum number of video players to keep cached (each holds decoded frames)
const MAX_VIDEO_PLAYERS: usize = 10;

/// Maximum number of sticker images to keep cached
const MAX_STICKER_IMAGES: usize = 50;

/// Download timeout in seconds (for audio/video downloads)
const DOWNLOAD_TIMEOUT_SECS: u64 = 60;

/// Download media with timeout - returns Ok(data) or Err(error message)
async fn download_with_timeout(
    download_rx: tokio::sync::oneshot::Receiver<Result<Vec<u8>, String>>,
) -> Result<Vec<u8>, String> {
    let timeout = smol::Timer::after(std::time::Duration::from_secs(DOWNLOAD_TIMEOUT_SECS));
    let download = async {
        download_rx
            .await
            .unwrap_or(Err("Download cancelled".to_string()))
    };

    // Race between download and timeout
    smol::future::or(async { Some(download.await) }, async {
        timeout.await;
        None
    })
    .await
    .ok_or_else(|| "Download timed out".to_string())?
}

/// Write a downloaded document into the user's Downloads directory
/// ($XDG_DOWNLOAD_DIR, then $HOME or %USERPROFILE% + /Downloads, then the CWD
/// like the database fallback when no home is known) and return the path
/// written.
fn save_to_downloads(file_name: &str, data: &[u8]) -> std::io::Result<std::path::PathBuf> {
    use std::io::Write;
    use std::path::PathBuf;

    let not_empty = |v: std::ffi::OsString| (!v.is_empty()).then_some(PathBuf::from(v));
    let dir = std::env::var_os("XDG_DOWNLOAD_DIR")
        .and_then(not_empty)
        .or_else(|| {
            std::env::var_os("HOME")
                .and_then(not_empty)
                .or_else(|| std::env::var_os("USERPROFILE").and_then(not_empty))
                .map(|home| home.join("Downloads"))
        })
        .unwrap_or_else(|| PathBuf::from("."));
    std::fs::create_dir_all(&dir)?;

    // The name comes off the wire: strip path separators (and `:`, which on
    // Windows makes a drive-relative path) so a hostile sender can't traverse
    // out of the directory.
    let sanitized: String = file_name
        .chars()
        .map(|c| {
            if std::path::is_separator(c) || c == '\\' || c == ':' || c.is_control() {
                '_'
            } else {
                c
            }
        })
        .collect();
    let name = match sanitized.trim() {
        "" | "." | ".." => "document",
        trimmed => trimmed,
    };

    // Windows treats device basenames (CON, NUL, COM1…) as reserved for any
    // extension; prefix them so the save can't resolve to a device.
    let stem = name
        .split_once('.')
        .map_or(name, |(stem, _)| stem)
        .trim_end_matches([' ', '.'])
        .to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit());
    let name = if reserved {
        format!("_{name}")
    } else {
        name.to_string()
    };

    // create_new + " (n)" suffixing so a download never clobbers an existing
    // file of the same name.
    for attempt in 0..1000u32 {
        let candidate = if attempt == 0 {
            name.to_string()
        } else {
            match name.rsplit_once('.') {
                Some((stem, ext)) if !stem.is_empty() => format!("{stem} ({attempt}).{ext}"),
                _ => format!("{name} ({attempt})"),
            }
        };
        let path = dir.join(candidate);
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                file.write_all(data)?;
                return Ok(path);
            }
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::new(
        std::io::ErrorKind::AlreadyExists,
        "too many downloads with the same name",
    ))
}

/// Currently active media playback (mutual exclusion: only one media at a time)
/// Animated stickers are excluded from this - they can play alongside audio/video.
#[derive(Clone, Debug, Default)]
enum ActiveMedia {
    /// No media currently playing
    #[default]
    None,
    /// Audio message playing (voice message, PTT)
    Audio { message_id: String },
    /// Video message playing (includes video's audio track)
    Video { message_id: String },
}

impl ActiveMedia {
    /// Check if this is an audio message
    fn is_audio(&self) -> bool {
        matches!(self, Self::Audio { .. })
    }

    /// Check if this is a video message
    fn is_video(&self) -> bool {
        matches!(self, Self::Video { .. })
    }

    /// Get the message ID if any media is playing
    fn message_id(&self) -> Option<&str> {
        match self {
            Self::None => None,
            Self::Audio { message_id } | Self::Video { message_id } => Some(message_id),
        }
    }

    /// Check if the given message ID is currently playing
    fn is_playing(&self, id: &str) -> bool {
        self.message_id() == Some(id)
    }
}

/// Initialize chat list key bindings
pub fn init_chat_list_bindings(cx: &mut gpui::App) {
    cx.bind_keys([
        KeyBinding::new("up", SelectUp, Some(CHAT_LIST_CONTEXT)),
        KeyBinding::new("down", SelectDown, Some(CHAT_LIST_CONTEXT)),
    ]);
}

// Action to navigate back to chat list on mobile
actions!(mobile_nav, [NavigateBack]);

/// Main application struct
pub struct WhatsAppApp {
    /// Current application state
    app_state: AppState,
    /// List of chats
    chats: Vec<Chat>,
    /// Currently selected chat JID
    selected_chat: Option<String>,
    /// WhatsApp client wrapper
    client: Option<WhatsAppClient>,
    /// Scroll handle for chat list
    chat_list_scroll: VirtualListScrollHandle,
    /// Focus handle for chat list keyboard navigation
    chat_list_focus: FocusHandle,
    /// Search input state for chat list (created lazily when window is available)
    chat_search_input: Option<Entity<InputState>>,
    /// Current search query (lowercase, trimmed)
    chat_search_query: String,
    /// Debounced search task
    #[allow(dead_code)]
    chat_search_task: Option<Task<()>>,
    /// Scroll handle for message list
    message_list_scroll: VirtualListScrollHandle,
    /// Isolated input area view (has its own render cycle for performance)
    input_area: Option<Entity<InputAreaView>>,
    /// Chat a composing indicator was last sent to: paused must go back to
    /// this chat even if the user switched chats before the typing timeout
    composing_chat: Option<String>,
    /// Unsent input text stashed per chat on switch; the shared input view
    /// would otherwise carry chat A's draft into chat B and send it there
    drafts: HashMap<String, String>,
    /// Background task for event polling (must be retained)
    #[allow(dead_code)]
    event_task: Option<Task<()>>,
    /// Audio recorder for PTT messages
    audio_recorder: AudioRecorder,
    /// Current recording state
    recording_state: RecordingState,
    /// Chat the current PTT recording started in; the note is sent there even
    /// if the user switches chats before stopping
    recording_chat: Option<String>,
    /// Audio player for voice message and video audio playback
    audio_player: AudioPlayer,
    /// Message ID of the audio currently loaded in audio_player (for ownership tracking)
    /// This ensures we don't resume audio from a different video when switching
    audio_owner: Option<String>,
    /// Currently active media (mutual exclusion: only one audio or video at a time)
    active_media: ActiveMedia,
    /// Message id of the most recent user-requested playback; download/decode
    /// completions autoplay only if they still match it, so a stale download
    /// can't steal playback from media the user started meanwhile.
    pending_media_request: Option<String>,
    /// Call state (incoming and outgoing calls)
    call_state: CallState,
    /// Cache of JID -> display name mappings (from notify/pushname attribute)
    name_cache: HashMap<String, String>,
    /// Video players for each message (message_id -> VideoPlayer)
    video_players: HashMap<String, VideoPlayer>,
    /// Task for video frame updates
    #[allow(dead_code)]
    video_update_task: Option<Task<()>>,
    /// Cache of sticker images (message_id -> Arc<Image>) for animation state preservation.
    /// Uses RefCell for interior mutability since we need to cache during immutable render.
    /// Uses IndexMap to maintain insertion order for deterministic FIFO eviction.
    sticker_images: RefCell<IndexMap<String, Arc<Image>>>,
    /// Cache of message list data per chat to avoid expensive recomputation on every render.
    /// Key is the chat JID, value is the cached data.
    message_list_cache: RefCell<HashMap<String, MessageListCache>>,
    /// Cache of chat list data to avoid recomputation on every render.
    chat_list_cache: RefCell<Option<ChatListCache>>,
    /// Mobile navigation state - which panel to show on mobile devices
    mobile_panel: MobilePanel,
}

impl WhatsAppApp {
    /// Spawn the event handling task that processes UI events from the WhatsApp client
    fn spawn_event_task(
        mut ui_rx: tokio::sync::mpsc::UnboundedReceiver<UiEvent>,
        cx: &mut Context<Self>,
    ) -> Task<()> {
        cx.spawn(async move |entity: WeakEntity<Self>, cx| {
            while let Some(event) = ui_rx.recv().await {
                let result = entity.update(cx, |app, cx| {
                    app.handle_event(event, cx);
                });
                if result.is_err() {
                    // Entity was dropped, stop the loop
                    break;
                }
            }
        })
    }

    /// Create a new WhatsApp application
    pub fn new(cx: &mut Context<Self>) -> Self {
        let bootstrap = WhatsAppClient::new().and_then(|mut client| {
            let ui_rx = client.start().map_err(std::io::Error::other)?;
            Ok((client, ui_rx))
        });
        let (app_state, client, event_task) = match bootstrap {
            Ok((client, ui_rx)) => (
                AppState::Loading,
                Some(client),
                Some(Self::spawn_event_task(ui_rx, cx)),
            ),
            Err(e) => (
                AppState::Error(format!("Failed to start client: {e}")),
                None,
                None,
            ),
        };

        Self {
            app_state,
            chats: Vec::new(),
            selected_chat: None,
            client,
            chat_list_scroll: VirtualListScrollHandle::new(),
            chat_list_focus: cx.focus_handle(),
            chat_search_input: None, // Created lazily when window is available
            chat_search_query: String::new(),
            chat_search_task: None,
            message_list_scroll: VirtualListScrollHandle::new(),
            input_area: None,
            composing_chat: None,
            drafts: HashMap::new(),
            event_task,
            audio_recorder: AudioRecorder::new(),
            recording_state: RecordingState::default(),
            recording_chat: None,
            audio_player: AudioPlayer::new(),
            audio_owner: None,
            active_media: ActiveMedia::None,
            pending_media_request: None,
            call_state: CallState::new(),
            name_cache: HashMap::new(),
            video_players: HashMap::new(),
            video_update_task: None,
            sticker_images: RefCell::new(IndexMap::new()),
            message_list_cache: RefCell::new(HashMap::new()),
            chat_list_cache: RefCell::new(None),
            mobile_panel: MobilePanel::default(),
        }
    }

    // ========== Responsive Layout ==========

    /// Create a ResponsiveLayout from the current window viewport.
    /// This should be called at the start of render to get all layout dimensions.
    pub fn responsive_layout(&self, window: &Window) -> ResponsiveLayout {
        ResponsiveLayout::new(window.viewport_size(), self.mobile_panel)
    }

    /// Get the current mobile panel state
    pub fn mobile_panel(&self) -> MobilePanel {
        self.mobile_panel
    }

    /// Navigate back to chat list (for mobile)
    pub fn navigate_back(&mut self, cx: &mut Context<Self>) {
        self.mobile_panel = MobilePanel::ChatList;
        cx.notify();
    }

    /// Navigate to chat view (for mobile) - called when selecting a chat
    fn navigate_to_chat(&mut self) {
        self.mobile_panel = MobilePanel::Chat;
    }

    // ========== Render Caches ==========

    /// Get or compute the chat list cache.
    /// This avoids expensive recomputation of chat list data on every render.
    /// Filters by search query if active. Item sizes are computed at render time
    /// based on ResponsiveLayout.
    pub fn get_chat_list_cache(&self) -> ChatListCache {
        let mut cache = self.chat_list_cache.borrow_mut();

        // Filter chats by search query
        let filtered_chats: Vec<&Chat> = if self.chat_search_query.is_empty() {
            self.chats.iter().collect()
        } else {
            self.chats
                .iter()
                .filter(|chat| {
                    chat.name.to_lowercase().contains(&self.chat_search_query)
                        || chat.jid.to_lowercase().contains(&self.chat_search_query)
                })
                .collect()
        };

        // Check if we have a valid cache entry
        if let Some(ref cached) = *cache
            && cached.chat_count == filtered_chats.len()
        {
            return cached.clone();
        }

        // Compute new cache entry (item sizes computed at render time)
        let chats_arc: Arc<[Chat]> = filtered_chats.into_iter().cloned().collect();

        let new_cache = ChatListCache {
            chat_count: chats_arc.len(),
            chats: chats_arc,
        };

        *cache = Some(new_cache.clone());
        new_cache
    }

    /// Invalidate chat list cache (call when chats change or search changes)
    fn invalidate_chat_cache(&self) {
        *self.chat_list_cache.borrow_mut() = None;
    }

    // ========== Message List Cache ==========

    /// Get or compute the message list cache for a chat.
    /// This avoids expensive recomputation of message heights on every render.
    /// Uses interior mutability so it can be called during immutable render.
    /// `max_media_size` should come from ResponsiveLayout for correct sizing.
    pub fn get_message_list_cache(
        &self,
        chat_jid: &str,
        messages: &[ChatMessage],
        is_group: bool,
        max_media_size: f32,
    ) -> MessageListCache {
        let mut cache = self.message_list_cache.borrow_mut();

        // Check if we have a valid cache entry; heights depend on the layout
        // inputs too, so a resize or group-flag change must recompute.
        if let Some(cached) = cache.get(chat_jid)
            && cached.message_count == messages.len()
            && cached.is_group == is_group
            && cached.max_media_size == max_media_size
        {
            return cached.clone();
        }

        // Compute new cache entry using the messages module
        let new_cache = MessageListCache::new(messages, is_group, max_media_size);
        cache.insert(chat_jid.to_string(), new_cache.clone());
        new_cache
    }

    /// Invalidate message list cache for a chat (call when messages change)
    fn invalidate_message_cache(&self, chat_jid: &str) {
        self.message_list_cache.borrow_mut().remove(chat_jid);
    }

    // ========== Accessors ==========

    /// Check if the client is connected
    fn is_connected(&self) -> bool {
        matches!(self.app_state, AppState::Connected)
    }

    /// Get the selected chat JID
    pub fn selected_chat_jid(&self) -> Option<String> {
        self.selected_chat.clone()
    }

    /// Get the currently selected chat data
    pub fn selected_chat_data(&self) -> Option<&Chat> {
        self.selected_chat
            .as_ref()
            .and_then(|jid| self.find_chat(jid))
    }

    /// Find a chat by JID (immutable)
    fn find_chat(&self, jid: &str) -> Option<&Chat> {
        self.chats.iter().find(|c| c.jid == jid)
    }

    /// Find a chat by JID (mutable)
    fn find_chat_mut(&mut self, jid: &str) -> Option<&mut Chat> {
        self.chats.iter_mut().find(|c| c.jid == jid)
    }

    fn read_boundary(chat: &Chat) -> Option<ReadBoundary> {
        let last = chat.messages.last()?;
        let ts_secs = last.timestamp.timestamp();
        let ids = chat
            .messages
            .iter()
            .rev()
            .take_while(|message| message.timestamp.timestamp() == ts_secs)
            .map(|message| {
                (
                    message.id.clone(),
                    message.is_from_me,
                    (!message.is_from_me).then(|| message.sender.clone()),
                )
            })
            .collect();
        Some((ts_secs, ids))
    }

    /// Update a message's media data (used to cache downloaded media)
    fn update_message_media_data(&mut self, message_id: &str, data: Vec<u8>) {
        // Find the message in any chat and update its media data
        for chat in &mut self.chats {
            if let Some(msg) = chat.messages.iter_mut().find(|m| m.id == message_id) {
                if let Some(ref mut media) = msg.media {
                    media.data = Arc::new(data);
                    // Full bytes landed; the data no longer needs a re-download
                    media.data_is_preview = false;
                    // Decode with the real media's MIME, not the preview
                    // thumbnail's (a WebP sticker would fail as image/jpeg)
                    if let Some(ref dl) = media.downloadable {
                        media.mime_type = dl.mime_type.clone();
                    }
                    // Drop any render-cached image built from the old bytes
                    self.sticker_images.borrow_mut().shift_remove(message_id);
                    info!("Cached media data for message {}", message_id);
                    // Invalidate message cache since we modified the message
                    self.message_list_cache.borrow_mut().remove(&chat.jid);
                }
                return;
            }
        }
    }

    /// Add a message to a chat, bumping it to the top of the list only when
    /// the message actually advances the chat (duplicates and older backfills
    /// leave the ordering alone).
    /// Returns true if the chat was found and updated, false otherwise.
    fn add_message_to_chat(&mut self, jid: &str, message: ChatMessage) -> bool {
        if let Some(index) = self.chats.iter().position(|c| c.jid == jid) {
            if self.chats[index].add_message(message) {
                self.move_chat_to_top(index);
            }
            // Always invalidate chat cache since the chat's content changed
            // (even if it didn't move, the last message preview needs updating)
            self.invalidate_chat_cache();
            // Also invalidate message cache for this chat
            self.invalidate_message_cache(jid);
            true
        } else {
            false
        }
    }

    /// Move a chat at the given index to the top of the list (index 0).
    /// Does nothing if already at top.
    fn move_chat_to_top(&mut self, index: usize) {
        if index > 0 && index < self.chats.len() {
            let chat = self.chats.remove(index);
            self.chats.insert(0, chat);
            // Note: chat cache invalidation is handled by the caller
        }
    }

    /// Get the chat list scroll handle
    pub fn chat_list_scroll(&self) -> &VirtualListScrollHandle {
        &self.chat_list_scroll
    }

    /// Get the message list scroll handle
    pub fn message_list_scroll(&self) -> &VirtualListScrollHandle {
        &self.message_list_scroll
    }

    /// Scroll to the last message in the currently selected chat.
    /// Uses scroll_to_item with the actual message count (not scroll_to_bottom,
    /// which relies on internal state that may be stale before paint).
    fn scroll_to_last_message(&self) {
        if let Some(ref jid) = self.selected_chat
            && let Some(chat) = self.find_chat(jid)
            && !chat.messages.is_empty()
        {
            self.message_list_scroll
                .scroll_to_item(chat.messages.len() - 1, ScrollStrategy::Top);
        }
    }

    /// Get the isolated input area view entity
    pub fn input_area(&self) -> Option<Entity<InputAreaView>> {
        self.input_area.clone()
    }

    /// Get the chat list focus handle
    pub fn chat_list_focus(&self) -> &FocusHandle {
        &self.chat_list_focus
    }

    /// Get the chat search input entity
    pub fn chat_search_input(&self) -> Option<&Entity<InputState>> {
        self.chat_search_input.as_ref()
    }

    /// Ensure the chat search input is initialized
    pub fn ensure_chat_search_input(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        use gpui_component::input::InputEvent;

        if self.chat_search_input.is_some() {
            return;
        }

        let search_input = cx.new(|cx| InputState::new(window, cx).placeholder("Search chats..."));

        // Subscribe to search input changes
        cx.subscribe(&search_input, |this, input, event: &InputEvent, cx| {
            if let InputEvent::Change = event {
                let query = input.read(cx).value().to_string();
                this.set_chat_search(query, cx);
            }
        })
        .detach();

        self.chat_search_input = Some(search_input);
    }

    // ========== Chat List Navigation ==========

    /// Select the next chat in the list (keyboard navigation)
    pub fn select_next_chat(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let cache = self.get_chat_list_cache();
        if cache.chats.is_empty() {
            return;
        }

        let current_index = self
            .selected_chat
            .as_ref()
            .and_then(|jid| cache.chats.iter().position(|c| &c.jid == jid));

        let next_index = match current_index {
            Some(idx) if idx + 1 < cache.chats.len() => idx + 1,
            None => 0,
            _ => return, // Already at the end
        };

        let next_jid = cache.chats[next_index].jid.clone();
        self.select_chat(next_jid, window, cx);
        self.chat_list_scroll
            .scroll_to_item(next_index, ScrollStrategy::Top);
    }

    /// Select the previous chat in the list (keyboard navigation)
    pub fn select_previous_chat(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let cache = self.get_chat_list_cache();
        if cache.chats.is_empty() {
            return;
        }

        let current_index = self
            .selected_chat
            .as_ref()
            .and_then(|jid| cache.chats.iter().position(|c| &c.jid == jid));

        let prev_index = match current_index {
            Some(idx) if idx > 0 => idx - 1,
            None => cache.chats.len() - 1,
            _ => return, // Already at the beginning
        };

        let prev_jid = cache.chats[prev_index].jid.clone();
        self.select_chat(prev_jid, window, cx);
        self.chat_list_scroll
            .scroll_to_item(prev_index, ScrollStrategy::Top);
    }

    // ========== Chat Search ==========

    /// Update chat search query with debouncing
    pub fn set_chat_search(&mut self, query: String, cx: &mut Context<Self>) {
        // Cancel previous debounce task
        self.chat_search_task = None;

        if query.is_empty() {
            // Immediate clear
            self.chat_search_query.clear();
            self.invalidate_chat_cache();
            cx.notify();
            return;
        }

        // Debounce the actual filtering
        let trimmed = query.trim().to_lowercase();
        self.chat_search_task = Some(cx.spawn(async move |entity: WeakEntity<Self>, cx| {
            cx.background_executor()
                .timer(std::time::Duration::from_millis(SEARCH_DEBOUNCE_MS))
                .await;

            let _ = entity.update(cx, |this, cx| {
                this.chat_search_query = trimmed;
                this.invalidate_chat_cache();
                cx.notify();
            });
        }));
    }

    // ========== Actions ==========

    pub fn select_chat(&mut self, jid: String, window: &mut Window, cx: &mut Context<Self>) {
        self.stop_current_media();
        // Leaving a chat mid-composition: release its typing indicator now,
        // or it would stay "typing..." and the eventual paused would land on
        // the newly selected chat instead.
        if self.composing_chat.as_deref() != Some(jid.as_str())
            && let Some(prev) = self.composing_chat.take()
        {
            if let Some(client) = &self.client {
                client.send_paused(&prev);
            }
            if let Some(ref input_area) = self.input_area {
                input_area.update(cx, |view, _| view.reset_typing());
            }
        }
        // Stash the outgoing chat's unsent text and restore the target's, or
        // the shared input would send A's draft to B. Skipped on reselect so
        // in-progress text survives a same-chat click.
        if self.selected_chat.as_deref() != Some(jid.as_str())
            && let Some(ref input_area) = self.input_area
        {
            let restored = self.drafts.remove(&jid).unwrap_or_default();
            let old = input_area.update(cx, |view, cx| view.swap_text(&restored, window, cx));
            if let Some(prev) = self.selected_chat.clone()
                && !old.trim().is_empty()
            {
                self.drafts.insert(prev, old);
            }
        }
        self.selected_chat = Some(jid.clone());
        self.navigate_to_chat();

        // Collect unread messages from others to send read receipts
        let unread_messages: Vec<(String, String)> = self
            .find_chat(&jid)
            .map(|chat| {
                chat.messages
                    .iter()
                    .filter(|msg| !msg.is_from_me && !msg.is_read)
                    .map(|msg| (msg.id.clone(), msg.sender.clone()))
                    .collect()
            })
            .unwrap_or_default();

        // Send read receipts to WhatsApp
        if !unread_messages.is_empty() {
            info!(
                "Sending read receipts for {} messages in {}",
                unread_messages.len(),
                observe_str(&jid)
            );
            if let Some(client) = &self.client {
                client.send_read_receipts(&jid, unread_messages);
            }
        }

        // The bounded action also covers unread rows not loaded by the UI.
        if let Some(chat) = self
            .find_chat(&jid)
            .filter(|c| c.unread_count > 0 || c.manually_unread)
        {
            // Include same-second siblings so none can re-badge on hydration.
            let last_displayed = Self::read_boundary(chat);
            if let Some(client) = &self.client {
                client.mark_chat_read(&jid, last_displayed);
            }
        }

        // Mark as read locally
        if let Some(chat) = self.find_chat_mut(&jid) {
            chat.mark_as_read();
            // Both caches: the badge, and the is_read snapshot the message
            // list renders ticks from (its count guard can't see this).
            self.invalidate_chat_cache();
            self.invalidate_message_cache(&jid);
        }

        // Scroll to the last message
        self.scroll_to_last_message();
        cx.notify();
    }

    /// Retry connection after an error
    pub fn retry_connection(&mut self, cx: &mut Context<Self>) {
        self.app_state = AppState::Loading;

        // Tear down the old client's background thread first; otherwise every
        // retry stacks another live runtime + DB pool on the same file.
        if let Some(client) = self.client.take() {
            client.shutdown();
        }
        self.event_task.take();

        // A failed rebuild routes back to the error screen (retry stays
        // available) instead of panicking the UI thread.
        match WhatsAppClient::new() {
            Ok(mut client) => match client.start() {
                Ok(ui_rx) => {
                    self.event_task = Some(Self::spawn_event_task(ui_rx, cx));
                    self.client = Some(client);
                }
                Err(e) => {
                    self.app_state = AppState::Error(format!("Failed to restart client: {e}"));
                }
            },
            Err(e) => {
                self.app_state = AppState::Error(format!("Failed to restart client: {e}"));
            }
        }
        cx.notify();
    }

    /// Initialize the isolated input area view (needs window context)
    /// The InputAreaView has its own render cycle, so typing doesn't trigger app re-renders.
    /// IMPORTANT: This method should NOT update the InputAreaView on every call,
    /// as that would defeat the purpose of isolation.
    pub fn ensure_input_area(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        if self.input_area.is_none() {
            // Create the isolated input area view
            let input_area = cx.new(|cx| InputAreaView::new(window, cx));

            // Subscribe to events from the input area
            cx.subscribe(&input_area, Self::handle_input_area_event)
                .detach();

            self.input_area = Some(input_area);
        }
        // NOTE: Do NOT call input_area.update() here - it would trigger re-renders
        // on every parent render, defeating the purpose of component isolation.
        // Recording state is updated via update_input_recording() when it changes.
    }

    /// Update the recording state in the input area (call only when recording state changes)
    fn update_input_recording(&self, cx: &mut Context<Self>) {
        if let Some(ref input_area) = self.input_area {
            let is_recording = self.is_recording();
            input_area.update(cx, |view, cx| {
                view.set_recording(is_recording, cx);
            });
        }
    }

    /// Handle events from the isolated input area view
    fn handle_input_area_event(
        &mut self,
        _input_area: Entity<InputAreaView>,
        event: &InputAreaEvent,
        cx: &mut Context<Self>,
    ) {
        match event {
            InputAreaEvent::SendMessage(text) => {
                self.send_message(text, cx);
            }
            InputAreaEvent::StartRecording => {
                self.start_recording(cx);
            }
            InputAreaEvent::StopRecording => {
                self.stop_recording_and_send(cx);
            }
            InputAreaEvent::StartedTyping => {
                // Send "composing" presence
                if let Some(jid) = &self.selected_chat {
                    self.composing_chat = Some(jid.clone());
                    if let Some(client) = &self.client {
                        client.send_composing(jid);
                    }
                }
            }
            InputAreaEvent::StoppedTyping => {
                // Send "paused" presence to the chat the composing went to,
                // not whatever chat is selected when the timeout fires
                let target = self
                    .composing_chat
                    .take()
                    .or_else(|| self.selected_chat.clone());
                if let Some(jid) = target
                    && let Some(client) = &self.client
                {
                    client.send_paused(&jid);
                }
            }
        }
    }

    /// Unique optimistic-bubble id: a millisecond timestamp alone collides on
    /// fast double-sends (add_message would dedup one bubble away and
    /// MessageIdAssigned could rename the wrong one).
    fn next_local_id(prefix: &str) -> String {
        use portable_atomic::AtomicU64;
        use std::sync::atomic::Ordering;
        static SEQ: AtomicU64 = AtomicU64::new(0);
        format!(
            "{prefix}_{}_{}",
            wacore::time::now_millis(),
            SEQ.fetch_add(1, Ordering::Relaxed)
        )
    }

    /// Send a message to the currently selected chat
    fn send_message(&mut self, text: &str, cx: &mut Context<Self>) {
        // Check if connected before attempting to send
        if !self.is_connected() {
            warn!("Cannot send message: not connected");
            return;
        }

        let Some(jid) = self.selected_chat.clone() else {
            return;
        };

        let local_id = Self::next_local_id("local");
        let Some(client) = &self.client else {
            warn!("Cannot send message: client is unavailable");
            return;
        };
        client.send_message(&jid, text, local_id.clone());

        // Add to local chat immediately for responsiveness; the client renames
        // it to the real id via MessageIdAssigned.
        let msg = ChatMessage::new_outgoing(local_id, text.to_string());
        if self.add_message_to_chat(&jid, msg) {
            self.scroll_to_last_message();
        }

        cx.notify();
    }

    // ========== PTT Recording ==========

    /// Check if currently recording
    pub fn is_recording(&self) -> bool {
        self.recording_state.is_recording()
    }

    /// Start audio recording for PTT
    pub fn start_recording(&mut self, cx: &mut Context<Self>) {
        if self.selected_chat.is_none() {
            warn!("Cannot record: no chat selected");
            return;
        }

        if self.recording_state != RecordingState::Idle {
            warn!("Audio recording is already active");
            return;
        }

        // Initialize and start recording
        if let Err(e) = self.audio_recorder.init() {
            error!("Failed to initialize audio recorder: {}", e);
            return;
        }

        if let Err(e) = self.audio_recorder.start() {
            error!("Failed to start recording: {}", e);
            return;
        }

        self.recording_state = RecordingState::Recording;
        // Bind the note to the chat it started in: resolving the destination
        // at stop time would misdeliver if the user switches chats meanwhile.
        self.recording_chat = self.selected_chat.clone();
        self.update_input_recording(cx);
        info!("PTT recording started");
        cx.notify();
    }

    /// Stop recording and send the audio message
    pub fn stop_recording_and_send(&mut self, cx: &mut Context<Self>) {
        // Check if connected before attempting to send
        if !self.is_connected() {
            warn!("Cannot send audio: not connected");
            self.cancel_recording(cx);
            return;
        }

        if !self.is_recording() {
            warn!("Not recording");
            return;
        }

        let jid = match self.recording_chat.take() {
            Some(jid) => jid,
            None => {
                warn!("No recording chat, cancelling recording");
                self.cancel_recording(cx);
                return;
            }
        };

        self.recording_state = RecordingState::Processing;
        self.update_input_recording(cx);
        cx.notify();

        // Stop recording and get audio data
        let recorded = match self.audio_recorder.stop() {
            Ok(audio) => audio,
            Err(e) => {
                error!("Failed to stop recording: {}", e);
                self.recording_state = RecordingState::Idle;
                // Every abort path must reset the input area too, or it keeps
                // rendering the recording UI forever.
                self.update_input_recording(cx);
                cx.notify();
                return;
            }
        };

        // Check minimum duration (1 second)
        if recorded.duration_secs < 1 {
            warn!("Recording too short, discarding");
            self.recording_state = RecordingState::Idle;
            self.update_input_recording(cx);
            cx.notify();
            return;
        }

        info!(
            "Recording stopped: {} samples, {}s",
            recorded.samples.len(),
            recorded.duration_secs
        );

        let Some(runtime) = self.client.as_ref().map(WhatsAppClient::runtime) else {
            warn!("Cannot send audio: client is unavailable");
            self.recording_state = RecordingState::Idle;
            self.update_input_recording(cx);
            cx.notify();
            return;
        };
        cx.spawn(async move |entity: WeakEntity<Self>, cx| {
            let encoded = runtime
                .spawn_blocking(move || {
                    let waveform = generate_waveform(&recorded.samples);
                    encode_to_opus_ogg(&recorded)
                        .map(|ogg| (ogg, waveform, recorded.duration_secs))
                        .map_err(|error| error.to_string())
                })
                .await
                .unwrap_or_else(|error| Err(format!("encoder task failed: {error}")));
            let _ = entity.update(cx, |app, cx| {
                app.finish_recording_send(jid, encoded, cx);
            });
        })
        .detach();
    }

    fn finish_recording_send(
        &mut self,
        jid: String,
        encoded: Result<(Vec<u8>, Vec<u8>, u32), String>,
        cx: &mut Context<Self>,
    ) {
        let (ogg_data, waveform, duration_secs) = match encoded {
            Ok(encoded) => encoded,
            Err(error) => {
                error!("Failed to encode audio: {error}");
                self.recording_state = RecordingState::Idle;
                self.update_input_recording(cx);
                cx.notify();
                return;
            }
        };

        let Some(client) = &self.client else {
            warn!("Cannot send audio: client is unavailable");
            self.recording_state = RecordingState::Idle;
            self.update_input_recording(cx);
            cx.notify();
            return;
        };
        let local_id = Self::next_local_id("local_audio");
        client.send_audio_message(
            &jid,
            ogg_data.clone(),
            duration_secs,
            waveform,
            local_id.clone(),
        );

        let msg = ChatMessage::new_outgoing_with_media(
            local_id,
            String::new(),
            MediaContent {
                media_type: MediaType::Audio,
                data: Arc::new(ogg_data),
                mime_type: "audio/ogg; codecs=opus".to_string(),
                width: None,
                height: None,
                caption: None,
                file_name: None,
                downloadable: None,
                is_animated: false,
                duration_secs: Some(duration_secs),
                data_is_preview: false,
            },
        );

        if self.add_message_to_chat(&jid, msg) {
            self.scroll_to_last_message();
        }
        self.recording_state = RecordingState::Idle;
        self.update_input_recording(cx);
        info!("PTT audio sent successfully");
        cx.notify();
    }

    /// Cancel recording without sending
    pub fn cancel_recording(&mut self, cx: &mut Context<Self>) {
        self.audio_recorder.cancel();
        self.recording_state = RecordingState::Idle;
        self.recording_chat = None;
        self.update_input_recording(cx);
        info!("PTT recording cancelled");
        cx.notify();
    }

    // ========== Call State ==========

    /// Get the current incoming call (if any)
    pub fn incoming_call(&self) -> Option<&IncomingCall> {
        self.call_state.incoming()
    }

    /// Get the current outgoing call (if any)
    pub fn outgoing_call(&self) -> Option<&OutgoingCall> {
        self.call_state.outgoing()
    }

    /// Accept the incoming call
    pub fn accept_call(&mut self, cx: &mut Context<Self>) {
        let Some(client) = &self.client else {
            warn!("Cannot accept call: client is unavailable");
            return;
        };
        if let Some(call) = self.call_state.take_incoming() {
            info!(
                "Accepting call {} from {}",
                call.call_id,
                observe_str(&call.caller_jid)
            );
            client.accept_call(call.call_id.as_str());
            cx.notify();
        }
    }

    /// Decline the incoming call
    pub fn decline_call(&mut self, cx: &mut Context<Self>) {
        let Some(client) = &self.client else {
            warn!("Cannot decline call: client is unavailable");
            return;
        };
        if let Some(call) = self.call_state.take_incoming() {
            info!(
                "Declining call {} from {}",
                call.call_id,
                observe_str(&call.caller_jid)
            );
            client.decline_call(call.call_id.as_str());
            cx.notify();
        }
    }

    /// Start a call to the specified JID
    pub fn start_call(&mut self, recipient_jid: String, is_video: bool, cx: &mut Context<Self>) {
        let Some(client) = &self.client else {
            warn!("Cannot start call: client is unavailable");
            return;
        };

        // Don't start a call if we already have an outgoing call
        if self.call_state.has_outgoing() {
            warn!("Already have an outgoing call in progress");
            return;
        }

        // Don't start a call if there's an incoming call
        if self.call_state.has_incoming() {
            warn!("Cannot start a call while there's an incoming call");
            return;
        }

        // Get the recipient name from the chat
        let recipient_name = self
            .find_chat(&recipient_jid)
            .map(|chat| chat.name.clone())
            .unwrap_or_else(|| "Unknown contact".to_string());

        info!(
            "Starting {} call to {}",
            if is_video { "video" } else { "audio" },
            observe_str(&recipient_jid)
        );

        // Create the outgoing call state
        let placeholder_call_id = format!("ui-call-{}", wacore::time::now_millis());
        let call = OutgoingCall::new(
            placeholder_call_id.clone(),
            recipient_jid.clone(),
            recipient_name,
            is_video,
        );
        self.call_state.set_outgoing(call);

        // Initiate the call through the client
        client.start_call(&recipient_jid, is_video, placeholder_call_id);

        cx.notify();
    }

    /// Cancel the current outgoing call
    pub fn cancel_outgoing_call(&mut self, cx: &mut Context<Self>) {
        if let Some(call) = self.call_state.take_outgoing() {
            info!("Cancelling outgoing call {}", call.call_id);
            if let Some(client) = &self.client {
                client.cancel_call(call.call_id.as_str());
            }
            cx.notify();
        }
    }

    // ========== Media Playback Control ==========

    /// Stop any currently playing media. Does NOT call cx.notify().
    fn stop_current_media(&mut self) {
        self.audio_player.stop();
        self.audio_owner = None;
        // An in-flight lazy download for the stopped media must not autoplay
        // when it completes; user-initiated requests re-set this after the stop.
        self.pending_media_request = None;

        if let ActiveMedia::Video { message_id } = &self.active_media {
            if let Some(player) = self.video_players.get_mut(message_id) {
                player.stop();
            }
            self.video_update_task = None;
        }

        self.active_media = ActiveMedia::None;
    }

    /// Get the currently playing audio message ID (if audio is playing)
    pub fn playing_message_id(&self) -> Option<&str> {
        match &self.active_media {
            // Gated on the stream so a paused voice note renders as paused;
            // resume still works because toggle_audio matches on active_media.
            ActiveMedia::Audio { message_id } if self.audio_player.is_playing() => Some(message_id),
            _ => None,
        }
    }

    /// Get the currently playing video message ID (if video is playing)
    pub fn playing_video_id(&self) -> Option<&str> {
        match &self.active_media {
            ActiveMedia::Video { message_id } => Some(message_id),
            _ => None,
        }
    }

    // ========== Audio Playback ==========

    pub fn play_audio(&mut self, message_id: String, audio_data: Vec<u8>, cx: &mut Context<Self>) {
        self.stop_current_media();
        self.pending_media_request = Some(message_id.clone());

        let completion_rx = self.audio_player.on_complete();

        match self.audio_player.play(audio_data) {
            Ok(()) => {
                self.audio_owner = Some(message_id.clone());
                self.active_media = ActiveMedia::Audio {
                    message_id: message_id.clone(),
                };
                info!("Started audio playback for message {}", message_id);

                // Wait for completion event (no polling needed)
                let completed_id = message_id;
                cx.spawn(async move |entity: WeakEntity<Self>, cx| {
                    let _ = completion_rx.await;

                    let _ = entity.update(cx, |app, cx| {
                        // Id check, not just is_audio: switching A -> B drops
                        // A's completion sender after B is active, and A's
                        // stale wakeup must not clear B's state.
                        if app.active_media.is_playing(&completed_id) {
                            app.active_media = ActiveMedia::None;
                            info!("Audio playback completed");
                            cx.notify();
                        }
                    });
                })
                .detach();
            }
            Err(e) => {
                error!("Failed to play audio: {}", e);
            }
        }
        cx.notify();
    }

    /// Stop audio playback (only if audio is currently playing)
    pub fn stop_audio(&mut self, cx: &mut Context<Self>) {
        if self.active_media.is_audio() {
            self.audio_player.stop();
            self.active_media = ActiveMedia::None;
            cx.notify();
        }
    }

    /// Toggle play/pause for the current audio
    pub fn toggle_audio(
        &mut self,
        message_id: String,
        audio_data: Vec<u8>,
        cx: &mut Context<Self>,
    ) {
        if self.active_media.is_playing(&message_id) && self.active_media.is_audio() {
            // Same message - toggle play/pause
            if self.audio_player.is_playing() {
                self.audio_player.pause();
            } else {
                self.audio_player.resume();
            }
        } else {
            // Different message or not playing - play it
            self.play_audio(message_id, audio_data, cx);
        }
        cx.notify();
    }

    /// Toggle audio playback with lazy loading (download first if needed)
    pub fn toggle_audio_lazy(
        &mut self,
        message_id: String,
        downloadable: DownloadableMedia,
        cx: &mut Context<Self>,
    ) {
        // If already playing this audio message, just toggle
        if self.active_media.is_playing(&message_id) && self.active_media.is_audio() {
            if self.audio_player.is_playing() {
                self.audio_player.pause();
            } else {
                self.audio_player.resume();
            }
            cx.notify();
            return;
        }

        let Some(client) = &self.client else {
            warn!("Cannot download audio: client is unavailable");
            return;
        };
        let download_rx = client.download_downloadable_media(downloadable);

        // Stop any current playback, video included: a playing video must not
        // keep running underneath the download.
        self.stop_current_media();
        self.pending_media_request = Some(message_id.clone());

        let msg_id = message_id.clone();

        info!("Starting audio download for message {}", msg_id);

        // Spawn a GPUI task to await the download result with timeout
        cx.spawn(async move |entity: WeakEntity<Self>, cx| {
            match download_with_timeout(download_rx).await {
                Ok(data) => {
                    info!("Audio downloaded: {} bytes", data.len());

                    // Cache the downloaded audio and play it
                    let _ = entity.update(cx, |app, cx| {
                        // Cache the audio data in the message so we don't need to download again
                        app.update_message_media_data(&msg_id, data.clone());
                        // Autoplay only if the user hasn't started other media
                        // since this download began.
                        if app.pending_media_request.as_deref() == Some(msg_id.as_str()) {
                            app.play_audio(msg_id, data, cx);
                        } else {
                            cx.notify();
                        }
                    });
                }
                Err(e) => {
                    error!("Failed to download audio: {}", e);
                }
            }
        })
        .detach();

        cx.notify();
    }

    /// Fetch the full image for a bubble whose eager download failed and left
    /// no thumbnail; mirrors the audio lazy-download path (no autoplay, the
    /// cached bytes just render).
    pub fn download_image(
        &mut self,
        message_id: String,
        downloadable: DownloadableMedia,
        cx: &mut Context<Self>,
    ) {
        let Some(client) = &self.client else {
            warn!("Cannot download image: client is unavailable");
            return;
        };
        let download_rx = client.download_downloadable_media(downloadable);

        cx.spawn(async move |entity: WeakEntity<Self>, cx| {
            match download_with_timeout(download_rx).await {
                Ok(data) => {
                    info!("Image downloaded: {} bytes", data.len());
                    let _ = entity.update(cx, |app, cx| {
                        app.update_message_media_data(&message_id, data);
                        cx.notify();
                    });
                }
                Err(e) => {
                    error!("Failed to download image: {}", e);
                }
            }
        })
        .detach();
    }

    /// Download a document and save it to the user's Downloads directory.
    /// Documents open in external apps, so bytes on disk beat cached bytes.
    pub fn download_document(
        &mut self,
        message_id: String,
        file_name: String,
        downloadable: DownloadableMedia,
        cx: &mut Context<Self>,
    ) {
        let Some(client) = &self.client else {
            warn!("Cannot download document: client is unavailable");
            return;
        };
        let download_rx = client.download_downloadable_media(downloadable);
        let runtime = client.runtime();

        cx.spawn(async move |_entity: WeakEntity<Self>, _cx| {
            match download_with_timeout(download_rx).await {
                Ok(data) => {
                    let saved = runtime
                        .spawn_blocking(move || save_to_downloads(&file_name, &data))
                        .await
                        .map_err(|error| std::io::Error::other(error.to_string()))
                        .and_then(|result| result);
                    match saved {
                        Ok(_) => info!("Document {} saved", message_id),
                        Err(e) => warn!("Failed to save document {}: {}", message_id, e),
                    }
                }
                Err(e) => error!("Failed to download document {}: {}", message_id, e),
            }
        })
        .detach();
    }

    // ========== Video Playback ==========

    /// Get the video player state for a message (if any)
    pub fn video_player_state(&self, message_id: &str) -> Option<VideoPlayerState> {
        self.video_players.get(message_id).map(|p| p.state())
    }

    /// Get current video frame for a message (if playing).
    /// Returns an `Arc<RenderImage>` — YUV→RGBA was already converted when
    /// the frame was decoded (same pattern Zed uses on Linux).
    pub fn video_current_frame(&self, message_id: &str) -> Option<Arc<gpui::RenderImage>> {
        self.video_players
            .get(message_id)
            .and_then(|p| p.current_frame())
    }

    /// Get or create a cached sticker image for animation state preservation.
    /// The same Arc<Image> must be returned across renders for GPUI to track animation state.
    /// Uses interior mutability (RefCell) so it can be called during immutable render.
    pub fn get_sticker_image(&self, message_id: &str, data: &[u8], mime_type: &str) -> Arc<Image> {
        // Check if already cached
        if let Some(cached) = self.sticker_images.borrow().get(message_id).cloned() {
            return cached;
        }

        // Create and cache the image
        let format = mime_to_image_format(mime_type);
        let image = Arc::new(Image::from_bytes(format, data.to_vec()));

        let mut cache = self.sticker_images.borrow_mut();

        // Evict oldest entries if cache is full (FIFO eviction using IndexMap insertion order)
        while cache.len() >= MAX_STICKER_IMAGES {
            // shift_remove removes from the front (oldest entry)
            cache.shift_remove_index(0);
        }

        cache.insert(message_id.to_string(), image.clone());
        image
    }

    /// Toggle video playback for a message
    pub fn toggle_video(
        &mut self,
        message_id: String,
        downloadable: DownloadableMedia,
        cx: &mut Context<Self>,
    ) {
        // Get player state first to determine action
        let player_state = self.video_players.get(&message_id).map(|p| p.state());

        match player_state {
            Some(VideoPlayerState::Playing) => {
                // Pause video and its audio
                if let Some(player) = self.video_players.get_mut(&message_id) {
                    player.pause();
                }
                self.audio_player.pause();
                self.active_media = ActiveMedia::None;
                self.video_update_task = None;
            }
            Some(VideoPlayerState::Paused) => {
                // Pausing cleared active_media, so is_playing alone can't
                // tell "nothing else started" from "another media is live";
                // audio ownership does. Stopping here on resume would drop
                // this video's own paused audio and it would come back mute.
                let owns_audio = self.audio_owner.as_deref() == Some(message_id.as_str());
                if !self.active_media.is_playing(&message_id) && !owns_audio {
                    self.stop_current_media();
                }
                self.pending_media_request = Some(message_id.clone());

                let (needs_audio, audio_data) =
                    if let Some(player) = self.video_players.get_mut(&message_id) {
                        let resume_pos = player.current_time();
                        let needs = player.play();
                        let data = if needs {
                            player
                                .get_audio()
                                .map(|a| (a.samples.clone(), a.sample_rate))
                        } else if !owns_audio {
                            // Another media's start stole the paused sink, so a
                            // plain resume() would leave this video silent;
                            // re-feed its audio from the pause position
                            // (samples are mono, so offset is seconds * rate).
                            player.get_audio().map(|a| {
                                let offset = ((resume_pos.as_secs_f64() * a.sample_rate as f64)
                                    as usize)
                                    .min(a.samples.len());
                                (a.samples[offset..].to_vec(), a.sample_rate)
                            })
                        } else {
                            None
                        };
                        (needs, data)
                    } else {
                        return;
                    };

                self.active_media = ActiveMedia::Video {
                    message_id: message_id.clone(),
                };
                self.start_video_update_task(cx);

                if let Some((samples, sample_rate)) = audio_data {
                    info!(
                        "Playing video audio: {} samples at {} Hz",
                        samples.len(),
                        sample_rate
                    );
                    if let Err(e) = self.audio_player.play_samples(samples, sample_rate) {
                        warn!("Failed to play video audio: {}", e);
                    } else {
                        // Ownership only on success: recording it for a dead
                        // sink would turn every later resume into a silent
                        // no-op resume().
                        self.audio_owner = Some(message_id.clone());
                    }
                } else if !needs_audio && self.audio_owner.as_ref() == Some(&message_id) {
                    // Only resume if audio belongs to this video
                    self.audio_player.resume();
                }
            }
            Some(VideoPlayerState::Idle) | Some(VideoPlayerState::Error) => {
                // Start downloading (or retry on error)
                self.start_video_download(message_id, downloadable, cx);
            }
            Some(VideoPlayerState::Downloading) | Some(VideoPlayerState::Decoding) => {
                // Already in progress, do nothing
            }
            None => {
                // No player yet, start downloading
                self.start_video_download(message_id, downloadable, cx);
            }
        }
        cx.notify();
    }

    /// Start downloading a video for playback
    fn start_video_download(
        &mut self,
        message_id: String,
        downloadable: DownloadableMedia,
        cx: &mut Context<Self>,
    ) {
        let Some(client) = &self.client else {
            warn!("Cannot download video: client is unavailable");
            return;
        };
        let download_rx = client.download_downloadable_media(downloadable);
        let runtime = client.runtime();

        // Stop any currently playing media (mutual exclusion)
        self.stop_current_media();
        self.pending_media_request = Some(message_id.clone());

        // Evict old video players if cache is full (excluding currently playing)
        if self.video_players.len() >= MAX_VIDEO_PLAYERS {
            // Remove players that aren't currently playing, up to half the limit
            let current_playing = self.playing_video_id().map(|s| s.to_string());
            let to_remove: Vec<_> = self
                .video_players
                .keys()
                .filter(|k| Some(*k) != current_playing.as_ref() && **k != message_id)
                .take(MAX_VIDEO_PLAYERS / 2)
                .cloned()
                .collect();
            for key in to_remove {
                self.video_players.remove(&key);
            }
        }

        // Create or get player and set to downloading
        let player = self.video_players.entry(message_id.clone()).or_default();
        player.set_downloading();

        let msg_id = message_id.clone();

        // Spawn a GPUI task to await the download result with timeout
        cx.spawn(async move |entity: WeakEntity<Self>, cx| {
            match download_with_timeout(download_rx).await {
                Ok(data) => {
                    info!("Video downloaded: {} bytes", data.len());

                    // Set to decoding state (quick UI update)
                    let _ = entity.update(cx, |app, cx| {
                        if let Some(player) = app.video_players.get_mut(&msg_id) {
                            player.set_decoding();
                        }
                        cx.notify();
                    });

                    let decode_result = match runtime
                        .spawn_blocking(move || StreamingVideoDecoder::new(&data))
                        .await
                    {
                        Ok(result) => result,
                        Err(error) => Err(anyhow::anyhow!("video decoder task failed: {error}")),
                    };

                    // Update UI with decode results
                    let _ = entity.update(cx, |app, cx| {
                        match decode_result {
                            Ok(mut decoder) => {
                                info!(
                                    "Video decoded: {} frames, {:.1}s",
                                    decoder.frame_count(),
                                    decoder.duration().as_secs_f64()
                                );

                                // Extract audio before loading decoder into player
                                let audio = decoder.take_audio();

                                if let Some(player) = app.video_players.get_mut(&msg_id) {
                                    player.load(decoder);

                                    // Store audio in player for replay capability
                                    if let Some(ref audio_data) = audio {
                                        player.set_audio(audio_data.clone());
                                    }
                                    // Don't call play() here - let the first frame render first
                                    // so GPUI can decode the WebP image before playback starts
                                }

                                // Invalidate message cache to force virtual list re-render
                                if let Some(ref jid) = app.selected_chat {
                                    app.invalidate_message_cache(jid);
                                }

                                // Schedule play() for the next frame to allow GPUI to decode the image
                                let msg_id_for_play = msg_id.clone();
                                let audio_for_play = audio;
                                cx.spawn(async move |entity: WeakEntity<Self>, cx| {
                                    // Wait one frame (~16ms at 60fps) for GPUI to decode the first frame
                                    smol::Timer::after(std::time::Duration::from_millis(16)).await;

                                    let _ = entity.update(cx, |app, cx| {
                                        // Skip autoplay when the user started
                                        // other media during download/decode.
                                        if app.pending_media_request.as_deref()
                                            == Some(msg_id_for_play.as_str())
                                            && let Some(player) =
                                                app.video_players.get_mut(&msg_id_for_play)
                                            && player.state() == VideoPlayerState::Paused
                                        {
                                            let needs_audio = player.play();
                                            app.active_media = ActiveMedia::Video {
                                                message_id: msg_id_for_play.clone(),
                                            };
                                            app.start_video_update_task(cx);

                                            if needs_audio && let Some(audio) = audio_for_play {
                                                info!(
                                                    "Playing video audio: {} samples at {} Hz",
                                                    audio.samples.len(),
                                                    audio.sample_rate
                                                );
                                                if let Err(e) = app
                                                    .audio_player
                                                    .play_samples(audio.samples, audio.sample_rate)
                                                {
                                                    warn!("Failed to play video audio: {}", e);
                                                } else {
                                                    app.audio_owner = Some(msg_id_for_play.clone());
                                                }
                                            }
                                        }
                                        cx.notify();
                                    });
                                })
                                .detach();
                            }
                            Err(e) => {
                                error!("Failed to decode video: {}", e);
                                if let Some(player) = app.video_players.get_mut(&msg_id) {
                                    player.set_error(e.to_string());
                                }
                            }
                        }
                        cx.notify();
                    });
                }
                Err(e) => {
                    error!("Failed to download video: {}", e);
                    let _ = entity.update(cx, |app, cx| {
                        if let Some(player) = app.video_players.get_mut(&msg_id) {
                            player.set_error(e);
                        }
                        cx.notify();
                    });
                }
            }
        })
        .detach();

        cx.notify();
    }

    /// Start the video frame update task
    fn start_video_update_task(&mut self, cx: &mut Context<Self>) {
        // Cancel any existing task
        self.video_update_task = None;

        // Get completion receiver from current video player
        // Clone the message_id first to avoid borrow conflicts
        let msg_id = self.playing_video_id().map(|s| s.to_string());
        let completion_rx = msg_id
            .as_ref()
            .and_then(|id| self.video_players.get_mut(id))
            .map(|player| player.on_complete());

        // Spawn update loop (~30 fps) with completion handling
        self.video_update_task = Some(cx.spawn(async move |entity: WeakEntity<Self>, cx| {
            // Create a fused future for completion (handles None case)
            let mut completion_rx = completion_rx;

            loop {
                // Check for completion event (non-blocking)
                if let Some(ref mut rx) = completion_rx {
                    // Try to receive without blocking
                    match rx.try_recv() {
                        Ok(()) => {
                            // Video completed naturally
                            let _ = entity.update(cx, |app, cx| {
                                app.active_media = ActiveMedia::None;
                                app.video_update_task = None;
                                app.audio_player.stop();
                                cx.notify();
                            });
                            break;
                        }
                        Err(tokio::sync::oneshot::error::TryRecvError::Closed) => {
                            // Channel closed (player dropped or stopped manually)
                            break;
                        }
                        Err(tokio::sync::oneshot::error::TryRecvError::Empty) => {
                            // Not completed yet, continue updating frames
                        }
                    }
                }

                // Wait for next frame (~30 fps)
                smol::Timer::after(std::time::Duration::from_millis(33)).await;

                // Update frame
                let should_stop = entity
                    .update(cx, |app, cx| {
                        // Clone message_id first to avoid borrow conflicts
                        let msg_id = app.playing_video_id().map(|s| s.to_string());
                        if let Some(ref id) = msg_id
                            && let Some(player) = app.video_players.get_mut(id)
                        {
                            if player.update() {
                                cx.notify();
                            }
                            // Continue as long as we're in Playing state
                            return player.state() != VideoPlayerState::Playing;
                        }
                        true // Stop if no playing video
                    })
                    .unwrap_or(true);

                if should_stop {
                    let _ = entity.update(cx, |app, cx| {
                        app.active_media = ActiveMedia::None;
                        app.video_update_task = None;
                        app.audio_player.stop();
                        cx.notify();
                    });
                    break;
                }
            }
        }));
    }

    // ========== Event Handling ==========

    /// Handle a single UI event
    fn handle_event(&mut self, event: UiEvent, cx: &mut Context<Self>) {
        match event {
            UiEvent::InitComplete => {
                self.app_state = AppState::Connecting;
                cx.notify();
            }
            UiEvent::HistoryLoaded { chats, complete } => {
                info!("Loaded {} chats from durable history", chats.len());
                // Prune only against a COMPLETE load: there absence means the
                // chat was archived/deleted (possibly on another device), so
                // it must leave the UI too. A truncated load can't distinguish
                // that from a chat that merely fell past the window, so it
                // never prunes. The selected chat is spared either way so the
                // open conversation isn't yanked mid-view.
                if complete {
                    let loaded: std::collections::HashSet<&str> =
                        chats.iter().map(|c| c.jid.as_str()).collect();
                    self.chats.retain(|c| {
                        loaded.contains(c.jid.as_str())
                            || self.selected_chat.as_deref() == Some(c.jid.as_str())
                    });
                }
                for chat in chats {
                    // Later loads (post-HistorySync re-hydration) fold into
                    // chats the UI already shows instead of being dropped.
                    match self.chats.iter_mut().find(|c| c.jid == chat.jid) {
                        Some(existing) => {
                            let jid = chat.jid.clone();
                            existing.merge_history(chat);
                            // The open chat was read locally the moment the
                            // message arrived; the store row commits with the
                            // unread bump before our receipt lands, so the
                            // hydrated counter must not resurrect the badge.
                            if self.selected_chat.as_deref() == Some(jid.as_str()) {
                                existing.mark_as_read();
                            }
                            self.invalidate_message_cache(&jid);
                        }
                        None => self.chats.push(chat),
                    }
                }
                self.chats
                    .sort_by_key(|c| std::cmp::Reverse(c.last_message_time));
                // Count-based cache guards can't see reordering/merges.
                self.invalidate_chat_cache();
                cx.notify();
            }
            UiEvent::QrCode { code, timeout_secs } => {
                let pair_code = match &self.app_state {
                    AppState::WaitingForPairing { pair_code, .. } => pair_code.clone(),
                    _ => None,
                };
                let cached_qr = generate_qr_png(&code).map(|png_bytes| CachedQrCode {
                    data: code,
                    png_bytes: Arc::new(png_bytes),
                });
                self.app_state = AppState::WaitingForPairing {
                    qr_code: cached_qr,
                    pair_code,
                    timeout_secs,
                };
                cx.notify();
            }
            UiEvent::PairCode { code, timeout_secs } => {
                let qr_code = match &self.app_state {
                    AppState::WaitingForPairing { qr_code, .. } => qr_code.clone(),
                    _ => None,
                };
                self.app_state = AppState::WaitingForPairing {
                    qr_code,
                    pair_code: Some(code),
                    timeout_secs,
                };
                cx.notify();
            }
            UiEvent::PairSuccess => {
                self.app_state = AppState::Syncing;
                cx.notify();
            }
            UiEvent::Connected => {
                self.app_state = AppState::Connected;
                cx.notify();
            }
            UiEvent::Disconnected(reason) => {
                self.app_state = AppState::Error(reason);
                cx.notify();
            }
            UiEvent::Error(msg) => {
                self.app_state = AppState::Error(msg);
                cx.notify();
            }
            UiEvent::MessageReceived {
                chat_jid,
                message,
                sender_name,
            } => {
                self.handle_message_received(chat_jid, *message, sender_name);
                cx.notify();
            }
            UiEvent::MessageIdAssigned {
                chat_jid,
                local_id,
                message_id,
            } => {
                if let Some(chat) = self.find_chat_mut(&chat_jid) {
                    // Re-insert, not mutate in place: messages sort by
                    // (timestamp, id) and the rename can reorder same-second
                    // siblings.
                    chat.rename_message(&local_id, &message_id);
                }
                self.invalidate_message_cache(&chat_jid);
                cx.notify();
            }
            UiEvent::SendFailed {
                chat_jid,
                message_id,
                reason,
            } => {
                warn!(
                    "Send failed for {} in {}: {}",
                    message_id,
                    observe_str(&chat_jid),
                    reason
                );
                if let Some(chat) = self.find_chat_mut(&chat_jid)
                    && let Some(msg) = chat.messages.iter_mut().find(|m| m.id == message_id)
                {
                    msg.failed = true;
                    self.invalidate_message_cache(&chat_jid);
                    cx.notify();
                }
            }
            UiEvent::ReceiptReceived {
                chat_jid,
                message_ids,
                receipt_type,
            } => {
                self.handle_receipt_received(chat_jid, message_ids, receipt_type);
                cx.notify();
            }
            UiEvent::ReactionReceived {
                chat_jid,
                message_id,
                sender,
                emoji,
            } => {
                self.handle_reaction_received(chat_jid, message_id, sender, emoji);
                cx.notify();
            }
            UiEvent::IncomingCall(mut call) => {
                if let Some(name) = self
                    .find_chat(&call.caller_jid)
                    .map(|chat| chat.name.clone())
                {
                    call.caller_name = name;
                }
                info!(
                    "Incoming {} call from {}",
                    if call.is_video { "video" } else { "audio" },
                    observe_str(&call.caller_jid)
                );
                self.call_state.set_incoming(call);
                cx.notify();
            }
            UiEvent::CallAccepted(call_id) => {
                info!("Call {} accepted by peer", call_id);
                // Dismiss the incoming call popup if it matches
                let incoming_dismissed = self.call_state.dismiss_incoming(&call_id);
                // For outgoing calls, transition to Connected state
                let outgoing_connected = self.call_state.set_outgoing_connected(&call_id);
                if outgoing_connected {
                    info!("Outgoing call {} is now connected", call_id);
                }
                if incoming_dismissed || outgoing_connected {
                    cx.notify();
                }
            }
            UiEvent::CallEnded(call_id) => {
                info!("Call {} ended", call_id);
                // Dismiss the incoming call popup if it matches
                let incoming_dismissed = self.call_state.dismiss_incoming(&call_id);
                // Also dismiss outgoing call if it matches
                let outgoing_dismissed = self.call_state.dismiss_outgoing(&call_id);
                if incoming_dismissed || outgoing_dismissed {
                    cx.notify();
                }
            }
            UiEvent::OutgoingCallStarted {
                call_id,
                recipient_jid,
            } => {
                info!(
                    "Outgoing call started: {} to {}",
                    call_id,
                    observe_str(&recipient_jid)
                );
                // Update the outgoing call with the actual call ID from CallManager
                if self
                    .call_state
                    .update_outgoing_call_id(&recipient_jid, call_id.clone())
                {
                    cx.notify();
                } else {
                    // Popup already dismissed: the user cancelled while the
                    // call was connecting; hang up the now-real call.
                    if let Some(client) = &self.client {
                        client.cancel_call(&call_id);
                    }
                }
            }
            UiEvent::OutgoingCallFailed {
                recipient_jid,
                error,
            } => {
                warn!(
                    "Outgoing call to {} failed: {}",
                    observe_str(&recipient_jid),
                    error
                );
                // Dismiss the outgoing call popup
                if self
                    .call_state
                    .dismiss_outgoing_for_recipient(&recipient_jid)
                {
                    cx.notify();
                }
            }
        }
    }

    /// Handle a received message
    fn handle_message_received(
        &mut self,
        chat_jid: String,
        mut message: ChatMessage,
        sender_name: Option<String>,
    ) {
        // Parse JID to determine chat type
        let jid = chat_jid.parse::<Jid>().ok();
        let is_group = jid.as_ref().is_some_and(|j| j.is_group());
        let is_status = jid.as_ref().is_some_and(|j| j.is_status_broadcast());

        // A message landing in the currently open chat is read immediately:
        // receipt out now, no badge (select_chat won't re-run to send it).
        let read_now = (!message.is_from_me
            && self.selected_chat.as_deref() == Some(chat_jid.as_str()))
        .then(|| (message.id.clone(), message.sender.clone()));

        // Cache the sender's name if provided
        if let Some(ref name) = sender_name {
            self.name_cache.insert(message.sender.clone(), name.clone());
        }

        // For group chats, set sender_name on the message for display
        if is_group && !message.is_from_me {
            message.sender_name = sender_name
                .clone()
                .or_else(|| self.name_cache.get(&message.sender).cloned());
        }

        // Find the chat index so we can move it to the top after adding message
        let chat_index = self.chats.iter().position(|c| c.jid == chat_jid);

        if let Some(index) = chat_index {
            // Update the existing chat
            let chat = &mut self.chats[index];

            // For groups: update participant name, NOT the chat name
            if is_group {
                if let Some(ref name) = sender_name {
                    chat.update_participant(message.sender.clone(), name.clone());
                }
            } else if !is_status {
                // For DMs only: update chat name if we have a better one
                if let Some(ref name) = sender_name
                    && !message.is_from_me
                {
                    chat.set_name_if_not_worse(name.clone(), 2);
                }
            }
            // Status broadcasts: don't update any names
            let advanced = chat.add_message(message);

            // Move chat to top of list (most recent first); duplicates and
            // older backfills don't reorder
            if advanced {
                self.move_chat_to_top(index);
            }

            // Always invalidate caches since chat content changed
            self.invalidate_chat_cache();
            self.invalidate_message_cache(&chat_jid);
        } else {
            // Create new chat
            let display_name = if is_group || is_status {
                // For groups/status, don't use sender name as chat name
                None
            } else if message.is_from_me {
                // For outgoing DMs, use cached name
                self.name_cache.get(&chat_jid).cloned()
            } else {
                // For incoming DMs, use sender name
                sender_name.clone()
            };

            let mut new_chat = if let Some(name) = display_name {
                Chat::with_name(chat_jid.clone(), name)
            } else {
                Chat::new(chat_jid.clone())
            };

            // For groups: track participant
            if is_group && let Some(ref name) = sender_name {
                new_chat.update_participant(message.sender.clone(), name.clone());
            }

            new_chat.add_message(message);
            self.chats.insert(0, new_chat);
            self.invalidate_chat_cache();
        }

        if let Some(receipt) = read_now {
            let boundary = self.find_chat(&chat_jid).and_then(Self::read_boundary);
            if let Some(client) = &self.client {
                client.send_read_receipts(&chat_jid, vec![receipt]);
                // Persist this immediate read so hydration cannot re-badge it.
                client.mark_chat_read(&chat_jid, boundary);
            }
            if let Some(chat) = self.find_chat_mut(&chat_jid) {
                chat.mark_as_read();
            }
            self.invalidate_chat_cache();
            self.invalidate_message_cache(&chat_jid);
        }
    }

    /// Handle a receipt event (read/played status update)
    fn handle_receipt_received(
        &mut self,
        chat_jid: String,
        message_ids: Vec<String>,
        receipt_type: ReceiptType,
    ) {
        if let Some(chat) = self.find_chat_mut(&chat_jid) {
            let count = chat.mark_messages_as_read(&message_ids);
            if count > 0 {
                info!(
                    "Marked {} message(s) as {:?} in {}",
                    count,
                    receipt_type,
                    observe_str(&chat_jid)
                );
                // Ticks and the unread badge changed; count-based cache
                // guards can't see either.
                self.invalidate_message_cache(&chat_jid);
                self.invalidate_chat_cache();
            }
        }
    }

    /// Handle a reaction event
    fn handle_reaction_received(
        &mut self,
        chat_jid: String,
        message_id: String,
        sender: String,
        emoji: String,
    ) {
        if let Some(chat) = self.find_chat_mut(&chat_jid) {
            if chat.add_reaction(&message_id, emoji.clone(), sender.clone()) {
                // Invalidate cache since reactions affect message height
                self.invalidate_message_cache(&chat_jid);
                info!(
                    "Added reaction '{}' from {} to message {} in {}",
                    emoji,
                    observe_str(&sender),
                    message_id,
                    observe_str(&chat_jid)
                );
            } else {
                info!(
                    "Message {} not found for reaction in chat {}",
                    message_id,
                    observe_str(&chat_jid)
                );
            }
        }
    }
}

impl Focusable for WhatsAppApp {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.chat_list_focus.clone()
    }
}

impl Render for WhatsAppApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let entity = cx.entity().clone();

        match &self.app_state {
            AppState::Loading => render_loading_view().into_any_element(),
            AppState::Connecting => render_connecting_view().into_any_element(),
            AppState::WaitingForPairing {
                qr_code,
                pair_code,
                timeout_secs,
            } => render_pairing_view(qr_code.as_ref(), pair_code.clone(), *timeout_secs)
                .into_any_element(),
            AppState::Syncing => render_syncing_view().into_any_element(),
            AppState::Connected => render_connected_view(self, window, cx).into_any_element(),
            AppState::Error(msg) => render_error_view(msg, entity).into_any_element(),
        }
    }
}

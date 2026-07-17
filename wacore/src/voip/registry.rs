//! Per-call registry: tracks active [`CallSession`]s and their media-task abort handles so a
//! connection teardown can stop every in-flight call. [`CallRegistry::abort_all`] is the teardown
//! primitive, but it is NOT yet wired into the client's connection cleanup; the integrator owns a
//! `CallRegistry` and must call `abort_all` from their own disconnect/reconnect path.
//!
//! The abort handle is [`crate::runtime::AbortHandle`] (runtime-agnostic), so the same registry
//! drives the Tokio driver task, a wasm `spawn_local` task, or any other runtime without coupling
//! the portable core to a specific executor.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use async_lock::Mutex as AsyncMutex;
use portable_atomic::{AtomicBool, AtomicU64};

use crate::runtime::AbortHandle;
use crate::types::call::VideoState;
use crate::voip::driver::{VideoControl, VideoControlSender};
use crate::voip::engine::CallEvent;
use crate::voip::session::{CallPhase, CallSession};
use wacore_binary::Jid;

/// Identifies one peer video-upgrade request within one call generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct VideoUpgradeToken {
    generation: u64,
    epoch: u64,
}

impl VideoUpgradeToken {
    pub fn generation(self) -> u64 {
        self.generation
    }
}

/// Result of applying a committed peer video-state transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PeerVideoTransition {
    Ignored,
    UpgradeRequested(VideoUpgradeToken),
    Applied {
        enable_plane: bool,
        teardown_local: bool,
        answer_upgrade: bool,
    },
}

#[derive(Debug, Clone, Copy)]
struct VideoNegotiation {
    self_state: VideoState,
    peer_state: VideoState,
    peer_request_epoch: u64,
    pending_peer_request: Option<u64>,
    self_request_epoch: u64,
    pending_self_request: Option<u64>,
}

impl VideoNegotiation {
    fn new(is_video: bool) -> Self {
        let state = if is_video {
            VideoState::Enabled
        } else {
            VideoState::Disabled
        };
        Self {
            self_state: state,
            peer_state: state,
            peer_request_epoch: 0,
            pending_peer_request: None,
            self_request_epoch: 0,
            pending_self_request: None,
        }
    }

    fn is_video(self) -> bool {
        !self.self_state.is_inactive_for_call_mode() || !self.peer_state.is_inactive_for_call_mode()
    }

    fn next_peer_request(&mut self, generation: u64) -> VideoUpgradeToken {
        self.peer_request_epoch = self.peer_request_epoch.wrapping_add(1).max(1);
        self.pending_peer_request = Some(self.peer_request_epoch);
        VideoUpgradeToken {
            generation,
            epoch: self.peer_request_epoch,
        }
    }

    fn next_self_request(&mut self) -> u64 {
        self.self_request_epoch = self.self_request_epoch.wrapping_add(1).max(1);
        self.pending_self_request = Some(self.self_request_epoch);
        self.self_request_epoch
    }
}

/// Runs its closure when dropped. Stored on a [`CallEntry`] to wake the call's `wait_ended()` waiter
/// whenever the entry is removed (terminal stanza, disconnect, supersession) -- including in the
/// window after registration but before a media task exists to carry the notify on its own teardown.
/// Every entry-drop is a terminal event for that generation, and the wake (a sticky flag) is
/// idempotent with the media task's own drop-guard, so firing it on every removal is safe.
struct EndedNotify(Option<Box<dyn FnOnce() + Send>>);

impl Drop for EndedNotify {
    fn drop(&mut self) {
        if let Some(f) = self.0.take() {
            f();
        }
    }
}

struct CallEntry {
    session: CallSession,
    media_task: Option<AbortHandle>,
    /// Monotonic token distinguishing this registration from a later same-call-id replacement, so a
    /// finishing task only reaps its OWN entry (the ABA hazard).
    generation: u64,
    /// Caller-only, one-shot: delivers the answering device LID to the drive loop so it can rekey
    /// recv. Taken on first use (a duplicate `<accept>` finds `None`); dropped with the entry.
    rekey_tx: Option<async_channel::Sender<String>>,
    /// The call's consumer-facing event queue (same channel `CallHandle::events()` reads), so the
    /// SIGNALING handler can surface `<video state>` changes next to the engine's events.
    event_tx: Option<async_channel::Sender<CallEvent>>,
    /// Mid-call video-plane control into the drive loop (enable/disable/orientation).
    video_ctl_tx: Option<VideoControlSender>,
    /// Fully release the local video endpoints. Stored here so refusal and terminal paths share the
    /// same teardown without keeping codec resources alive through lingering handles.
    video_teardown: Option<Box<dyn Fn() + Send + Sync>>,
    /// Prevents concurrent video transitions from overtaking each other while an ack is in flight.
    event_publication_reserved: Arc<AtomicBool>,
    /// Mirrors WA's stream mutex for user actions, peer signaling, and timeout callbacks.
    video_transition_lock: Arc<AsyncMutex<()>>,
    video: VideoNegotiation,
    /// Wakes this call's `wait_ended()` waiter on removal, even before a media task exists. Fires from
    /// `EndedNotify`'s Drop whenever the entry leaves the map.
    on_terminal: Option<EndedNotify>,
}

/// Exclusive publication right for an actionable signaling event awaiting its typed ack.
pub struct CallEventPermit {
    tx: async_channel::Sender<CallEvent>,
    reserved: Arc<AtomicBool>,
    generation: u64,
}

impl CallEventPermit {
    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn send(&self, event: CallEvent) -> bool {
        // The latest committed state must remain observable even when its consumer is behind.
        self.tx.force_send(event).is_ok()
    }
}

impl Drop for CallEventPermit {
    fn drop(&mut self) {
        self.reserved.store(false, Ordering::Release);
    }
}

impl Drop for CallEntry {
    fn drop(&mut self) {
        if let Some(teardown) = self.video_teardown.take() {
            teardown();
        }
    }
}

/// Thread-safe map of active calls keyed by call-id.
#[derive(Default)]
pub struct CallRegistry {
    inner: Mutex<HashMap<String, CallEntry>>,
    next_gen: AtomicU64,
    /// Incoming offers we've rung but not yet answered, keyed by call-id. Mirrors WA Web's
    /// `_ringingCalls`: it is the ONLY signal that distinguishes a genuine missed call (a `<terminate>`
    /// for an offer still ringing) from a `<terminate>` for an answered or outgoing call. Active-call
    /// absence cannot tell them apart, since an answered call leaves the map on teardown too.
    ringing: Mutex<HashSet<String>>,
}

impl CallRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Deliver a signaling event through the call's consumer queue.
    pub fn send_call_event(&self, call_id: &str, event: CallEvent) -> bool {
        let tx = self
            .inner
            .lock()
            .expect("registry lock poisoned")
            .get(call_id)
            .and_then(|entry| entry.event_tx.clone());
        tx.is_some_and(|tx| tx.force_send(event).is_ok())
    }

    /// Register a call, returning its generation token. A same-call-id re-offer (retry/glare)
    /// REPLACES the prior registration, aborting its media task; the returned generation
    /// distinguishes the new call from the old. Pass it to [`set_media_task`](Self::set_media_task)
    /// and [`remove_if_current`](Self::remove_if_current) so a finishing task only ever reaps its
    /// OWN entry, never a newer replacement.
    pub fn insert(&self, session: CallSession) -> u64 {
        let generation = self.next_gen.fetch_add(1, Ordering::Relaxed);
        let video = VideoNegotiation::new(session.is_video);
        // Registering a call as active answers (accept) or places (outgoing) it: it is no longer
        // merely ringing. A no-op for an outgoing call (never ringing); for an accepted incoming
        // offer this clears the ringing flag so a later `<terminate>` reads as ended, not missed.
        self.take_ringing(&session.call_id);
        let prev = {
            let mut map = self.inner.lock().expect("registry lock poisoned");
            map.insert(
                session.call_id.clone(),
                CallEntry {
                    session,
                    media_task: None,
                    generation,
                    rekey_tx: None,
                    event_tx: None,
                    video_ctl_tx: None,
                    video_teardown: None,
                    event_publication_reserved: Arc::new(AtomicBool::new(false)),
                    video_transition_lock: Arc::new(AsyncMutex::new(())),
                    video,
                    on_terminal: None,
                },
            )
        };
        // The superseded entry drops here, OUTSIDE the lock: its media-task AbortHandle aborts and its
        // on_terminal hook fires (the old generation ended). Running those closures off-lock keeps them
        // from re-entering or poisoning the registry mutex.
        drop(prev);
        generation
    }

    /// Attach (or replace) the media task for the call registered under `generation`. If the call
    /// was removed or superseded by a newer generation, the handle is aborted immediately so its
    /// task can't outlive the call.
    pub fn set_media_task(&self, call_id: &str, generation: u64, handle: AbortHandle) {
        match self
            .inner
            .lock()
            .expect("registry lock poisoned")
            .get_mut(call_id)
        {
            Some(entry) if entry.generation == generation => {
                if let Some(old) = entry.media_task.replace(handle) {
                    old.abort();
                }
            }
            _ => handle.abort(),
        }
    }

    /// Attach the wake-on-removal hook for the call under `generation`: when the entry is removed
    /// (terminal stanza / disconnect / supersession), `notify` runs to wake a parked `wait_ended()`,
    /// even if no media task was attached yet. Generation-guarded and ignored if removed/superseded.
    pub fn set_ended_notify(
        &self,
        call_id: &str,
        generation: u64,
        notify: impl FnOnce() + Send + 'static,
    ) {
        if let Some(entry) = self
            .inner
            .lock()
            .expect("registry lock poisoned")
            .get_mut(call_id)
            && entry.generation == generation
            // Set-once: a second call for the same generation would otherwise drop (and fire) the
            // existing hook in place, a false terminal notification. The first hook wins.
            && entry.on_terminal.is_none()
        {
            entry.on_terminal = Some(EndedNotify(Some(Box::new(notify))));
        }
    }

    /// Store the per-call recv-rekey sender (the drive loop holds the matching receiver). Generation-
    /// guarded and ignored if the call was removed or superseded, so a stale sender can't outlive its
    /// call. Caller side only.
    pub fn set_rekey_sender(
        &self,
        call_id: &str,
        generation: u64,
        tx: async_channel::Sender<String>,
    ) {
        if let Some(entry) = self
            .inner
            .lock()
            .expect("registry lock poisoned")
            .get_mut(call_id)
            && entry.generation == generation
        {
            entry.rekey_tx = Some(tx);
        }
    }

    /// Store the call's consumer-facing event sender, video-control sender, and the local-video
    /// teardown hook, so the signaling handler can surface `<video state>` changes, steer the video
    /// plane mid-call, and fully release the endpoints on a refused upgrade. Generation-guarded like
    /// the rekey sender.
    pub fn set_video_channels(
        &self,
        call_id: &str,
        generation: u64,
        event_tx: async_channel::Sender<CallEvent>,
        video_ctl_tx: VideoControlSender,
        video_teardown: Box<dyn Fn() + Send + Sync>,
    ) {
        if let Some(entry) = self
            .inner
            .lock()
            .expect("registry lock poisoned")
            .get_mut(call_id)
            && entry.generation == generation
        {
            entry.event_tx = Some(event_tx);
            entry.video_ctl_tx = Some(video_ctl_tx);
            entry.video_teardown = Some(video_teardown);
        }
    }

    /// Replace the one-shot endpoint teardown for the current call generation.
    pub fn set_video_teardown(
        &self,
        call_id: &str,
        generation: u64,
        video_teardown: Box<dyn Fn() + Send + Sync>,
    ) -> bool {
        let mut map = self.inner.lock().expect("registry lock poisoned");
        let Some(entry) = map
            .get_mut(call_id)
            .filter(|entry| entry.generation == generation)
        else {
            return false;
        };
        entry.video_teardown = Some(video_teardown);
        true
    }

    /// Release the local video endpoints once for the current call generation.
    pub fn run_video_teardown(&self, call_id: &str, generation: u64) -> bool {
        let hook = self
            .inner
            .lock()
            .expect("registry lock poisoned")
            .get_mut(call_id)
            .filter(|entry| entry.generation == generation)
            .and_then(|entry| entry.video_teardown.take());
        if let Some(hook) = hook {
            hook();
            true
        } else {
            false
        }
    }

    /// Serialize an actionable signaling event across its typed ack. The permit force-inserts the
    /// committed transition, so queue pressure cannot hide peer-visible state from the consumer.
    pub fn reserve_call_event(&self, call_id: &str) -> Option<CallEventPermit> {
        let (tx, reserved, generation) = {
            let map = self.inner.lock().expect("registry lock poisoned");
            let entry = map.get(call_id)?;
            (
                entry.event_tx.clone()?,
                entry.event_publication_reserved.clone(),
                entry.generation,
            )
        };
        if tx.is_closed()
            || reserved
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
        {
            return None;
        }
        Some(CallEventPermit {
            tx,
            reserved,
            generation,
        })
    }

    /// The current call generation and its shared video-transition lock.
    pub fn current_video_transition(&self, call_id: &str) -> Option<(u64, Arc<AsyncMutex<()>>)> {
        self.inner
            .lock()
            .expect("registry lock poisoned")
            .get(call_id)
            .map(|entry| (entry.generation, entry.video_transition_lock.clone()))
    }

    /// The video-transition lock for one known call generation.
    pub fn video_transition_lock(
        &self,
        call_id: &str,
        generation: u64,
    ) -> Option<Arc<AsyncMutex<()>>> {
        self.inner
            .lock()
            .expect("registry lock poisoned")
            .get(call_id)
            .filter(|entry| entry.generation == generation)
            .map(|entry| entry.video_transition_lock.clone())
    }

    /// Apply a peer state while the caller holds this generation's video-transition lock.
    pub fn apply_peer_video_state(
        &self,
        call_id: &str,
        generation: u64,
        state: VideoState,
    ) -> PeerVideoTransition {
        let mut map = self.inner.lock().expect("registry lock poisoned");
        let Some(entry) = map
            .get_mut(call_id)
            .filter(|entry| entry.generation == generation)
        else {
            return PeerVideoTransition::Ignored;
        };

        let outcome = match state {
            VideoState::UpgradeRequest | VideoState::UpgradeRequestV2 => {
                if entry.video.peer_state.is_upgrade_request()
                    && let Some(epoch) = entry.video.pending_peer_request
                {
                    PeerVideoTransition::UpgradeRequested(VideoUpgradeToken { generation, epoch })
                } else if entry.video.self_state.is_upgrade_request()
                    && entry.video.pending_self_request.is_some()
                {
                    entry.video.self_state = VideoState::Enabled;
                    entry.video.peer_state = VideoState::Enabled;
                    entry.video.pending_self_request = None;
                    entry.video.pending_peer_request = None;
                    PeerVideoTransition::Applied {
                        enable_plane: true,
                        teardown_local: false,
                        answer_upgrade: true,
                    }
                } else if !entry.video.self_state.is_inactive_for_call_mode() {
                    PeerVideoTransition::Ignored
                } else {
                    entry.video.peer_state = state;
                    let token = entry.video.next_peer_request(generation);
                    PeerVideoTransition::UpgradeRequested(token)
                }
            }
            VideoState::UpgradeAccept => {
                if entry.video.self_state.is_upgrade_request()
                    && entry.video.pending_self_request.is_some()
                {
                    entry.video.self_state = VideoState::Enabled;
                    entry.video.peer_state = VideoState::Enabled;
                    entry.video.pending_self_request = None;
                    PeerVideoTransition::Applied {
                        enable_plane: true,
                        teardown_local: false,
                        answer_upgrade: false,
                    }
                } else {
                    PeerVideoTransition::Ignored
                }
            }
            VideoState::Enabled => {
                if entry.video.self_state.is_upgrade_request() {
                    entry.video.self_state = VideoState::Enabled;
                    entry.video.pending_self_request = None;
                }
                entry.video.peer_state = VideoState::Enabled;
                entry.video.pending_peer_request = None;
                PeerVideoTransition::Applied {
                    enable_plane: true,
                    teardown_local: false,
                    answer_upgrade: false,
                }
            }
            VideoState::UpgradeReject | VideoState::UpgradeRejectByTimeout => {
                if entry.video.self_state.is_upgrade_request()
                    && entry.video.pending_self_request.is_some()
                {
                    entry.video.self_state = VideoState::Disabled;
                    entry.video.peer_state = state;
                    entry.video.pending_self_request = None;
                    PeerVideoTransition::Applied {
                        enable_plane: false,
                        teardown_local: true,
                        answer_upgrade: false,
                    }
                } else {
                    PeerVideoTransition::Ignored
                }
            }
            VideoState::Disabled
            | VideoState::UpgradeCancel
            | VideoState::UpgradeCancelByTimeout
            | VideoState::Error => {
                entry.video.self_state = VideoState::Disabled;
                entry.video.peer_state = state;
                entry.video.pending_self_request = None;
                entry.video.pending_peer_request = None;
                PeerVideoTransition::Applied {
                    enable_plane: false,
                    teardown_local: true,
                    answer_upgrade: false,
                }
            }
            VideoState::Stopped => {
                entry.video.peer_state = VideoState::Stopped;
                entry.video.pending_peer_request = None;
                PeerVideoTransition::Applied {
                    enable_plane: false,
                    teardown_local: false,
                    answer_upgrade: false,
                }
            }
            VideoState::Paused | VideoState::UnknownPeer => {
                entry.video.peer_state = state;
                PeerVideoTransition::Applied {
                    enable_plane: false,
                    teardown_local: false,
                    answer_upgrade: false,
                }
            }
            VideoState::Unknown(_) => PeerVideoTransition::Ignored,
        };
        entry.session.is_video = entry.video.is_video();
        outcome
    }

    /// Begin a local upgrade and return its timeout epoch.
    pub fn begin_local_video_request(&self, call_id: &str, generation: u64) -> Option<u64> {
        let mut map = self.inner.lock().expect("registry lock poisoned");
        let entry = map
            .get_mut(call_id)
            .filter(|entry| entry.generation == generation)?;
        if !entry.video.self_state.is_inactive_for_call_mode()
            || entry.video.pending_peer_request.is_some()
        {
            return None;
        }
        entry.video.self_state = VideoState::UpgradeRequestV2;
        let epoch = entry.video.next_self_request();
        entry.session.is_video = entry.video.is_video();
        Some(epoch)
    }

    /// Complete a peer request only when the same request is still pending.
    pub fn complete_peer_video_request(&self, call_id: &str, token: VideoUpgradeToken) -> bool {
        let mut map = self.inner.lock().expect("registry lock poisoned");
        let Some(entry) = map
            .get_mut(call_id)
            .filter(|entry| entry.generation == token.generation)
        else {
            return false;
        };
        if entry.video.pending_peer_request != Some(token.epoch)
            || !entry.video.peer_state.is_upgrade_request()
        {
            return false;
        }
        entry.video.self_state = VideoState::Enabled;
        entry.video.peer_state = VideoState::Enabled;
        entry.video.pending_peer_request = None;
        entry.session.is_video = entry.video.is_video();
        true
    }

    pub fn peer_video_request_is_current(&self, call_id: &str, token: VideoUpgradeToken) -> bool {
        self.inner
            .lock()
            .expect("registry lock poisoned")
            .get(call_id)
            .filter(|entry| entry.generation == token.generation)
            .is_some_and(|entry| {
                entry.video.pending_peer_request == Some(token.epoch)
                    && entry.video.peer_state.is_upgrade_request()
            })
    }

    /// Roll back or expire one local request without touching a newer request.
    pub fn end_local_video_request(&self, call_id: &str, generation: u64, epoch: u64) -> bool {
        let mut map = self.inner.lock().expect("registry lock poisoned");
        let Some(entry) = map
            .get_mut(call_id)
            .filter(|entry| entry.generation == generation)
        else {
            return false;
        };
        if entry.video.pending_self_request != Some(epoch)
            || !entry.video.self_state.is_upgrade_request()
        {
            return false;
        }
        entry.video.self_state = VideoState::Disabled;
        entry.video.peer_state = VideoState::Disabled;
        entry.video.pending_self_request = None;
        entry.video.pending_peer_request = None;
        entry.session.is_video = entry.video.is_video();
        true
    }

    /// Clear both directions after a failed handshake or full downgrade.
    pub fn reset_video(&self, call_id: &str, generation: u64) -> bool {
        let mut map = self.inner.lock().expect("registry lock poisoned");
        let Some(entry) = map
            .get_mut(call_id)
            .filter(|entry| entry.generation == generation)
        else {
            return false;
        };
        entry.video.self_state = VideoState::Disabled;
        entry.video.peer_state = VideoState::Disabled;
        entry.video.pending_self_request = None;
        entry.video.pending_peer_request = None;
        entry.session.is_video = false;
        true
    }

    /// Mark only our direction stopped; the peer may keep sending video.
    pub fn stop_local_video(&self, call_id: &str, generation: u64) -> bool {
        let mut map = self.inner.lock().expect("registry lock poisoned");
        let Some(entry) = map
            .get_mut(call_id)
            .filter(|entry| entry.generation == generation)
        else {
            return false;
        };
        entry.video.self_state = VideoState::Stopped;
        entry.video.pending_self_request = None;
        entry.session.is_video = entry.video.is_video();
        true
    }

    pub fn video_states(&self, call_id: &str, generation: u64) -> Option<(VideoState, VideoState)> {
        self.inner
            .lock()
            .expect("registry lock poisoned")
            .get(call_id)
            .filter(|entry| entry.generation == generation)
            .map(|entry| (entry.video.self_state, entry.video.peer_state))
    }

    /// Send a mid-call video-plane command to the current drive loop.
    pub fn send_video_ctl(&self, call_id: &str, generation: u64, ctl: VideoControl) {
        let tx = self
            .inner
            .lock()
            .expect("registry lock poisoned")
            .get(call_id)
            .filter(|entry| entry.generation == generation)
            .and_then(|e| e.video_ctl_tx.clone());
        if let Some(tx) = tx {
            let _ = tx.send(ctl);
        }
    }

    /// Track whether the call currently has video negotiated (offer `<video>`, upgrade accepted, or
    /// downgraded back). No-op for an unknown call.
    pub fn set_is_video(&self, call_id: &str, generation: u64, is_video: bool) -> bool {
        if let Some(entry) = self
            .inner
            .lock()
            .expect("registry lock poisoned")
            .get_mut(call_id)
            && entry.generation == generation
        {
            entry.session.is_video = is_video;
            entry.video = VideoNegotiation::new(is_video);
            true
        } else {
            false
        }
    }

    pub fn is_current(&self, call_id: &str, generation: u64) -> bool {
        self.inner
            .lock()
            .expect("registry lock poisoned")
            .get(call_id)
            .is_some_and(|entry| entry.generation == generation)
    }

    /// Caller side: rekey recv to the device that answered. One-shot — the sender is TAKEN, so a
    /// duplicate/late `<accept>` from another device is a no-op (first answerer wins, matching WA Web).
    /// Silently ignored when absent (no engine yet, an incoming call, or the call is torn down).
    pub fn send_rekey(&self, call_id: &str, answering_lid: String) {
        let tx = self
            .inner
            .lock()
            .expect("registry lock poisoned")
            .get_mut(call_id)
            .and_then(|e| e.rekey_tx.take());
        if let Some(tx) = tx {
            let _ = tx.try_send(answering_lid);
        }
    }

    /// Caller side: record the callee device that answered (the inbound `<accept>`'s `call.from`), so a
    /// later `<terminate>` can target it instead of the bare peer the offer rang. Set-once -- the first
    /// answerer wins, matching the rekey and WA Web's accepted-elsewhere handling. No-op if the call is
    /// unknown or a device was already recorded.
    pub fn set_answering_device(&self, call_id: &str, device: Jid) {
        if let Some(entry) = self
            .inner
            .lock()
            .expect("registry lock poisoned")
            .get_mut(call_id)
            && entry.session.answering_device.is_none()
        {
            entry.session.answering_device = Some(device);
        }
    }

    /// The callee device that answered the call registered under `generation`, if one has, for
    /// addressing a `<terminate>`. Generation-guarded so a stale handle (superseded by a same-call-id
    /// replacement) can't read the newer call's device; it falls back to its own bare peer instead.
    pub fn answering_device_if_current(&self, call_id: &str, generation: u64) -> Option<Jid> {
        self.inner
            .lock()
            .expect("registry lock poisoned")
            .get(call_id)
            .filter(|e| e.generation == generation)
            .and_then(|e| e.session.answering_device.clone())
    }

    /// The current generation token registered under `call_id`, or `None` if unknown. Lets a caller
    /// confirm its registration still owns the call (not superseded/removed) before attaching to it.
    pub fn generation_of(&self, call_id: &str) -> Option<u64> {
        self.inner
            .lock()
            .expect("registry lock poisoned")
            .get(call_id)
            .map(|e| e.generation)
    }

    pub fn phase(&self, call_id: &str) -> Option<CallPhase> {
        self.inner
            .lock()
            .expect("registry lock poisoned")
            .get(call_id)
            .map(|e| e.session.phase())
    }

    /// Advance a call's phase; returns false if the call is unknown or the transition is illegal.
    pub fn transition(&self, call_id: &str, next: CallPhase) -> bool {
        self.inner
            .lock()
            .expect("registry lock poisoned")
            .get_mut(call_id)
            .is_some_and(|e| e.session.transition_to(next))
    }

    /// Read a clone of a call's session snapshot.
    pub fn snapshot(&self, call_id: &str) -> Option<CallSession> {
        self.inner
            .lock()
            .expect("registry lock poisoned")
            .get(call_id)
            .map(|e| e.session.clone())
    }

    /// Take an outgoing call's sibling-dismiss targets: `(call_creator, rung_device_jids)`, leaving
    /// the session's `ring_devices` empty so a duplicate accept/reject can't re-dismiss. Returns
    /// `None` when the call is unknown or has no devices to dismiss (already taken, single-device, or
    /// incoming). The device list is consumed here, but the entry stays -- an accepted call is live.
    /// Each device JID already names its user, so the bare peer is not returned (the dismiss
    /// terminate is addressed per device JID, not to the bare peer).
    pub fn take_dismiss_targets(&self, call_id: &str) -> Option<(Jid, Vec<Jid>)> {
        let mut map = self.inner.lock().expect("registry lock poisoned");
        let entry = map.get_mut(call_id)?;
        if entry.session.ring_devices.is_empty() {
            return None;
        }
        let devices = std::mem::take(&mut entry.session.ring_devices);
        Some((entry.session.call_creator.clone(), devices))
    }

    pub fn active_count(&self) -> usize {
        self.inner.lock().expect("registry lock poisoned").len()
    }

    /// Record an incoming offer as ringing (not yet answered) so a later `<terminate>` for it can be
    /// surfaced as a missed call. Idempotent; the flag is consumed by [`take_ringing`](Self::take_ringing)
    /// on answer or terminate. Do not call for an offline-queued offer: that one is already surfaced
    /// as missed-offline and must not double-fire.
    pub fn mark_incoming_ringing(&self, call_id: &str) {
        self.ringing
            .lock()
            .expect("registry lock poisoned")
            .insert(call_id.to_string());
    }

    /// Consume the ringing flag for `call_id`, returning whether it was still ringing. True means a
    /// genuine missed call (an unanswered incoming offer the peer gave up on); false means the call
    /// was answered, was outgoing, or was already resolved (so a duplicate `<terminate>` is ended,
    /// never a second missed). One-shot.
    pub fn take_ringing(&self, call_id: &str) -> bool {
        self.ringing
            .lock()
            .expect("registry lock poisoned")
            .remove(call_id)
    }

    /// Remove a call, aborting its media task. Returns true if it existed.
    pub fn remove(&self, call_id: &str) -> bool {
        let removed = self
            .inner
            .lock()
            .expect("registry lock poisoned")
            .remove(call_id);
        // `removed` drops here, after the lock guard: the media-task abort and on_terminal hook run
        // off-lock.
        removed.is_some()
    }

    /// Remove a call only if it is still on `generation` -- the safe self-cleanup for a finishing
    /// media task. A newer same-call-id replacement (different generation) is left untouched, so a
    /// task that ended after being superseded can't reap the live replacement. Returns true if this
    /// generation was the current entry and was removed.
    pub fn remove_if_current(&self, call_id: &str, generation: u64) -> bool {
        let removed = {
            let mut map = self.inner.lock().expect("registry lock poisoned");
            if map.get(call_id).is_some_and(|e| e.generation == generation) {
                map.remove(call_id)
            } else {
                None
            }
        };
        // `removed` drops here, off-lock: the media-task abort and on_terminal hook run without the
        // registry mutex held.
        removed.is_some()
    }

    /// Abort every call's media task and clear the registry. Returns the number cleared.
    /// Call this from your own disconnect/reconnect teardown; it is not wired into the client.
    pub fn abort_all(&self) -> usize {
        self.ringing.lock().expect("registry lock poisoned").clear();
        let drained: Vec<CallEntry> = {
            let mut map = self.inner.lock().expect("registry lock poisoned");
            map.drain().map(|(_, entry)| entry).collect()
        };
        let n = drained.len();
        // `drained` drops here, off-lock: every entry aborts its media task and fires on_terminal.
        n
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voip::driver::video_control_channel;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use wacore_binary::{Jid, Server};

    fn session(id: &str) -> CallSession {
        CallSession::new_outgoing(
            id,
            Jid::new("222222222222222", Server::Lid),
            Jid::new("111111111111111", Server::Lid),
        )
    }

    #[test]
    fn answering_device_is_set_once_and_generation_guarded() {
        let reg = CallRegistry::new();
        assert_eq!(reg.answering_device_if_current("CID", 0), None, "unknown");
        let g = reg.insert(session("CID"));
        assert_eq!(
            reg.answering_device_if_current("CID", g),
            None,
            "none until an accept"
        );
        let dev = Jid::new("222222222222222", Server::Lid).with_device(2);
        reg.set_answering_device("CID", dev.clone());
        assert_eq!(reg.answering_device_if_current("CID", g), Some(dev.clone()));
        // First answerer wins: a later accept from another device is ignored.
        let other = Jid::new("222222222222222", Server::Lid).with_device(5);
        reg.set_answering_device("CID", other);
        assert_eq!(reg.answering_device_if_current("CID", g), Some(dev.clone()));
        // A replacement under a new generation isolates the device: the stale generation reads None.
        let g2 = reg.insert(session("CID"));
        assert_ne!(g, g2);
        assert_eq!(
            reg.answering_device_if_current("CID", g),
            None,
            "a stale generation must not read the replacement's device"
        );
        assert_eq!(reg.answering_device_if_current("CID", g2), None);
    }

    #[test]
    fn signaling_event_commit_survives_queue_backpressure() {
        let reg = CallRegistry::new();
        let generation = reg.insert(session("CID"));
        let (event_tx, event_rx) = async_channel::bounded(1);
        let (ctl_tx, _ctl_rx) = video_control_channel();
        reg.set_video_channels("CID", generation, event_tx.clone(), ctl_tx, Box::new(|| {}));

        let event = || CallEvent::VideoStateChanged {
            state: crate::types::call::VideoState::Enabled,
            orientation: None,
            upgrade_token: None,
        };
        let permit = reg.reserve_call_event("CID").expect("first reservation");
        assert!(
            reg.reserve_call_event("CID").is_none(),
            "only one typed ack may own the signaling slot"
        );
        assert!(
            event_rx.is_empty(),
            "reservation must not publish the event"
        );
        event_tx
            .try_send(CallEvent::RelayAllocated)
            .expect("fill the queue while the typed ack is in flight");
        assert!(permit.send(event()));
        assert!(matches!(
            event_rx.try_recv(),
            Ok(CallEvent::VideoStateChanged { .. })
        ));
        assert!(
            reg.reserve_call_event("CID").is_none(),
            "publishing must not release the transition before its effects finish"
        );
        drop(permit);
        assert!(reg.reserve_call_event("CID").is_some());
        assert!(reg.reserve_call_event("UNKNOWN").is_none());
    }

    #[test]
    fn peer_upgrade_tokens_reject_cancel_request_aba() {
        let reg = CallRegistry::new();
        let generation = reg.insert(session("CID"));
        let first =
            match reg.apply_peer_video_state("CID", generation, VideoState::UpgradeRequestV2) {
                PeerVideoTransition::UpgradeRequested(token) => token,
                transition => panic!("unexpected transition: {transition:?}"),
            };
        assert!(reg.peer_video_request_is_current("CID", first));
        assert!(matches!(
            reg.apply_peer_video_state("CID", generation, VideoState::UpgradeCancel),
            PeerVideoTransition::Applied {
                teardown_local: true,
                ..
            }
        ));

        let second =
            match reg.apply_peer_video_state("CID", generation, VideoState::UpgradeRequestV2) {
                PeerVideoTransition::UpgradeRequested(token) => token,
                transition => panic!("unexpected transition: {transition:?}"),
            };
        assert_ne!(first, second);
        assert!(!reg.peer_video_request_is_current("CID", first));
        assert!(reg.peer_video_request_is_current("CID", second));
        assert!(!reg.complete_peer_video_request("CID", first));
        assert!(reg.peer_video_request_is_current("CID", second));
        assert!(reg.complete_peer_video_request("CID", second));
        assert_eq!(
            reg.video_states("CID", generation),
            Some((VideoState::Enabled, VideoState::Enabled))
        );
    }

    #[test]
    fn duplicate_peer_request_keeps_the_same_token() {
        let reg = CallRegistry::new();
        let generation = reg.insert(session("CID"));
        let request = |state| match reg.apply_peer_video_state("CID", generation, state) {
            PeerVideoTransition::UpgradeRequested(token) => token,
            transition => panic!("unexpected transition: {transition:?}"),
        };
        assert_eq!(
            request(VideoState::UpgradeRequest),
            request(VideoState::UpgradeRequestV2)
        );
    }

    #[test]
    fn stopped_is_directional_but_disabled_is_a_full_downgrade() {
        let reg = CallRegistry::new();
        let generation = reg.insert(session("CID"));
        let token =
            match reg.apply_peer_video_state("CID", generation, VideoState::UpgradeRequestV2) {
                PeerVideoTransition::UpgradeRequested(token) => token,
                transition => panic!("unexpected transition: {transition:?}"),
            };
        assert!(reg.complete_peer_video_request("CID", token));

        assert!(matches!(
            reg.apply_peer_video_state("CID", generation, VideoState::Stopped),
            PeerVideoTransition::Applied {
                teardown_local: false,
                ..
            }
        ));
        assert_eq!(
            reg.video_states("CID", generation),
            Some((VideoState::Enabled, VideoState::Stopped))
        );
        assert!(reg.snapshot("CID").expect("session").is_video);

        assert!(matches!(
            reg.apply_peer_video_state("CID", generation, VideoState::Disabled),
            PeerVideoTransition::Applied {
                teardown_local: true,
                ..
            }
        ));
        assert_eq!(
            reg.video_states("CID", generation),
            Some((VideoState::Disabled, VideoState::Disabled))
        );
        assert!(!reg.snapshot("CID").expect("session").is_video);
    }

    #[test]
    fn local_request_timeout_epoch_cannot_end_a_newer_request() {
        let reg = CallRegistry::new();
        let generation = reg.insert(session("CID"));
        let first = reg
            .begin_local_video_request("CID", generation)
            .expect("first request");
        assert!(reg.end_local_video_request("CID", generation, first));
        let second = reg
            .begin_local_video_request("CID", generation)
            .expect("second request");
        assert_ne!(first, second);
        assert!(!reg.end_local_video_request("CID", generation, first));
        assert_eq!(
            reg.video_states("CID", generation),
            Some((VideoState::UpgradeRequestV2, VideoState::Disabled))
        );
        assert!(reg.end_local_video_request("CID", generation, second));
    }

    #[test]
    fn cancelling_a_local_request_is_a_full_native_downgrade() {
        let reg = CallRegistry::new();
        let mut video_session = session("CID");
        video_session.is_video = true;
        let generation = reg.insert(video_session);
        assert!(reg.stop_local_video("CID", generation));
        let epoch = reg
            .begin_local_video_request("CID", generation)
            .expect("local request");

        assert!(reg.end_local_video_request("CID", generation, epoch));
        assert_eq!(
            reg.video_states("CID", generation),
            Some((VideoState::Disabled, VideoState::Disabled))
        );
        assert!(!reg.snapshot("CID").expect("session").is_video);
    }

    #[test]
    fn video_teardown_runs_after_registry_unlock() {
        let reg = Arc::new(CallRegistry::new());
        let generation = reg.insert(session("CID"));
        let (event_tx, _event_rx) = async_channel::bounded(1);
        let (ctl_tx, _ctl_rx) = video_control_channel();
        let lock_was_free = Arc::new(AtomicBool::new(false));
        reg.set_video_channels("CID", generation, event_tx, ctl_tx, {
            let reg = reg.clone();
            let lock_was_free = lock_was_free.clone();
            Box::new(move || {
                lock_was_free.store(reg.inner.try_lock().is_ok(), Ordering::SeqCst);
            })
        });

        assert!(reg.run_video_teardown("CID", generation));
        assert!(lock_was_free.load(Ordering::SeqCst));
        assert!(!reg.run_video_teardown("CID", generation));
    }

    #[test]
    fn video_teardown_is_one_shot_until_rearmed() {
        let reg = CallRegistry::new();
        let generation = reg.insert(session("CID"));
        let (event_tx, _event_rx) = async_channel::bounded(1);
        let (ctl_tx, _ctl_rx) = video_control_channel();
        let calls = Arc::new(AtomicU64::new(0));
        let hook = |calls: &Arc<AtomicU64>| {
            let calls = calls.clone();
            Box::new(move || {
                calls.fetch_add(1, Ordering::SeqCst);
            }) as Box<dyn Fn() + Send + Sync>
        };
        reg.set_video_channels("CID", generation, event_tx, ctl_tx, hook(&calls));

        assert!(reg.run_video_teardown("CID", generation));
        assert!(!reg.run_video_teardown("CID", generation));
        assert_eq!(calls.load(Ordering::SeqCst), 1);

        assert!(reg.set_video_teardown("CID", generation, hook(&calls)));
        assert!(reg.run_video_teardown("CID", generation));
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        assert!(reg.remove_if_current("CID", generation));
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn video_mutations_are_generation_guarded_and_orientation_is_advisory() {
        let reg = CallRegistry::new();
        let stale = reg.insert(session("CID"));
        let current = reg.insert(session("CID"));
        let (event_tx, _event_rx) = async_channel::bounded(1);
        let (ctl_tx, ctl_rx) = video_control_channel();
        let teardown_calls = Arc::new(AtomicU64::new(0));
        reg.set_video_channels("CID", current, event_tx, ctl_tx, {
            let teardown_calls = teardown_calls.clone();
            Box::new(move || {
                teardown_calls.fetch_add(1, Ordering::SeqCst);
            })
        });

        assert!(reg.set_is_video("CID", current, true));
        assert!(!reg.set_is_video("CID", stale, false));
        assert!(reg.snapshot("CID").expect("session").is_video);
        assert!(!reg.run_video_teardown("CID", stale));
        assert_eq!(teardown_calls.load(Ordering::SeqCst), 0);

        reg.send_video_ctl("CID", current, VideoControl::Disable);
        for orientation in 0..100u8 {
            reg.send_video_ctl(
                "CID",
                current,
                VideoControl::SetOrientation(orientation % 4),
            );
        }
        reg.send_video_ctl("CID", current, VideoControl::Enable);
        assert_eq!(ctl_rx.try_recv(), Ok(VideoControl::Disable));
        assert_eq!(ctl_rx.try_recv(), Ok(VideoControl::Enable));
        assert_eq!(ctl_rx.try_recv(), Ok(VideoControl::SetOrientation(3)));

        reg.send_video_ctl("CID", stale, VideoControl::Disable);
        assert!(ctl_rx.try_recv().is_err());
    }

    #[test]
    fn removal_releases_video_endpoints_after_registry_unlock() {
        let reg = Arc::new(CallRegistry::new());
        let generation = reg.insert(session("CID"));
        let (event_tx, _event_rx) = async_channel::bounded(1);
        let (ctl_tx, _ctl_rx) = video_control_channel();
        let calls = Arc::new(AtomicU64::new(0));
        reg.set_video_channels("CID", generation, event_tx, ctl_tx, {
            let reg = reg.clone();
            let calls = calls.clone();
            Box::new(move || {
                assert!(reg.inner.try_lock().is_ok());
                calls.fetch_add(1, Ordering::SeqCst);
            })
        });

        assert!(reg.remove_if_current("CID", generation));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn ended_notify_fires_on_removal_even_without_a_media_task() {
        let reg = CallRegistry::new();
        let fired = Arc::new(AtomicBool::new(false));
        let g = reg.insert(session("CID"));
        reg.set_ended_notify("CID", g, {
            let fired = fired.clone();
            move || fired.store(true, Ordering::SeqCst)
        });
        // No media task attached (the connect-window case).
        assert!(reg.remove_if_current("CID", g));
        assert!(
            fired.load(Ordering::SeqCst),
            "removing a task-less entry must wake its wait_ended() via the on_terminal hook"
        );
    }

    #[test]
    fn ended_notify_is_generation_guarded_and_fires_via_abort_all() {
        let reg = CallRegistry::new();
        // A stale generation must not attach the hook.
        let stale = Arc::new(AtomicBool::new(false));
        let g = reg.insert(session("CID"));
        reg.set_ended_notify("CID", g + 99, {
            let stale = stale.clone();
            move || stale.store(true, Ordering::SeqCst)
        });
        // The live generation attaches it; abort_all (disconnect) fires it.
        let fired = Arc::new(AtomicBool::new(false));
        reg.set_ended_notify("CID", g, {
            let fired = fired.clone();
            move || fired.store(true, Ordering::SeqCst)
        });
        reg.abort_all();
        assert!(
            fired.load(Ordering::SeqCst),
            "abort_all must fire on_terminal"
        );
        assert!(
            !stale.load(Ordering::SeqCst),
            "a stale-generation hook must never have been attached"
        );
    }

    /// An abort handle that flips a shared flag, so a test can assert the registry actually aborts
    /// the stored handle (the runtime-agnostic analog of asserting a tokio task was cancelled).
    fn flag_handle(flag: &Arc<AtomicBool>) -> AbortHandle {
        let flag = flag.clone();
        AbortHandle::new(move || flag.store(true, Ordering::SeqCst))
    }

    #[test]
    fn send_rekey_is_one_shot_and_generation_guarded() {
        let reg = CallRegistry::new();
        let g = reg.insert(session("CID"));
        let (tx, rx) = async_channel::bounded::<String>(1);
        // A stale generation is ignored (no sender stored).
        reg.set_rekey_sender("CID", g + 99, tx.clone());
        reg.send_rekey("CID", "x".into());
        assert!(
            rx.try_recv().is_err(),
            "stale-generation sender must not fire"
        );
        // The live generation stores it; the first send fires, the second is a no-op (taken).
        reg.set_rekey_sender("CID", g, tx);
        reg.send_rekey("CID", "222222222222222:2@lid".into());
        assert_eq!(rx.try_recv().ok().as_deref(), Some("222222222222222:2@lid"));
        reg.send_rekey("CID", "again".into());
        assert!(rx.try_recv().is_err(), "rekey sender is one-shot");
    }

    #[test]
    fn ringing_is_one_shot_and_distinguishes_missed_from_ended() {
        let reg = CallRegistry::new();
        // An unanswered incoming offer: marked ringing, then a <terminate> consumes it as missed.
        reg.mark_incoming_ringing("RING");
        assert!(reg.take_ringing("RING"), "an unanswered offer is missed");
        assert!(
            !reg.take_ringing("RING"),
            "one-shot: a duplicate <terminate> is ended, not a second missed"
        );
        // A call we never rang (outgoing, or a terminate with no preceding offer) is never missed.
        assert!(!reg.take_ringing("NEVER"));
    }

    #[test]
    fn answering_an_incoming_offer_clears_its_ringing_flag() {
        let reg = CallRegistry::new();
        reg.mark_incoming_ringing("CID");
        // Accepting the call registers it as active (insert), which clears the ringing flag so a
        // later <terminate> reads as ended, not missed.
        let _g = reg.insert(session("CID"));
        assert!(
            !reg.take_ringing("CID"),
            "an answered call must not surface a missed call on terminate"
        );
    }

    #[test]
    fn abort_all_clears_ringing() {
        let reg = CallRegistry::new();
        reg.mark_incoming_ringing("CID");
        reg.abort_all();
        assert!(
            !reg.take_ringing("CID"),
            "a disconnect must drop stale ringing state so it can't surface after reconnect"
        );
    }

    #[test]
    fn insert_transition_remove() {
        let reg = CallRegistry::new();
        let _g = reg.insert(session("CID"));
        assert_eq!(reg.phase("CID"), Some(CallPhase::Idle));
        assert!(reg.transition("CID", CallPhase::Calling));
        assert_eq!(reg.phase("CID"), Some(CallPhase::Calling));
        assert!(!reg.transition("UNKNOWN", CallPhase::Calling));
        assert!(reg.remove("CID"));
        assert!(!reg.remove("CID"));
        assert_eq!(reg.active_count(), 0);
    }

    #[test]
    fn take_dismiss_targets_one_shot_and_dropped_on_remove() {
        let reg = CallRegistry::new();
        let peer = Jid::new("222222222222222", Server::Lid);
        let devs = vec![peer.with_device(1), peer.with_device(2)];

        let mut s = session("CID");
        s.ring_devices = devs.clone();
        let _g = reg.insert(s);

        // First take returns the targets; a second is None (one-shot, so a duplicate accept/reject
        // can't re-dismiss).
        let (got_creator, taken) = reg.take_dismiss_targets("CID").expect("first take");
        assert_eq!(got_creator, Jid::new("111111111111111", Server::Lid));
        assert_eq!(taken, devs);
        assert!(reg.take_dismiss_targets("CID").is_none(), "one-shot");

        // The device list dies with the entry: insert, remove, then take finds nothing.
        let mut s2 = session("CID2");
        s2.ring_devices = devs.clone();
        let _g2 = reg.insert(s2);
        assert!(reg.remove("CID2"));
        assert!(
            reg.take_dismiss_targets("CID2").is_none(),
            "removed entry leaves no tracking to leak"
        );

        assert!(reg.take_dismiss_targets("UNKNOWN").is_none());
    }

    #[test]
    fn remove_aborts_media_task() {
        let reg = CallRegistry::new();
        let g = reg.insert(session("A"));
        let flag = Arc::new(AtomicBool::new(false));
        reg.set_media_task("A", g, flag_handle(&flag));
        assert!(reg.remove("A"));
        assert!(
            flag.load(Ordering::SeqCst),
            "removing a call must abort its media task"
        );
    }

    #[test]
    fn abort_all_aborts_media_tasks() {
        let reg = CallRegistry::new();
        let flags: Vec<Arc<AtomicBool>> = ["A", "B"]
            .iter()
            .map(|id| {
                let g = reg.insert(session(id));
                let flag = Arc::new(AtomicBool::new(false));
                reg.set_media_task(id, g, flag_handle(&flag));
                flag
            })
            .collect();
        assert_eq!(reg.active_count(), 2);
        assert_eq!(reg.abort_all(), 2);
        assert_eq!(reg.active_count(), 0);
        assert!(
            flags.iter().all(|f| f.load(Ordering::SeqCst)),
            "abort_all must abort every media task"
        );
    }

    #[test]
    fn replace_aborts_the_old_media_task() {
        let reg = CallRegistry::new();
        let g = reg.insert(session("A"));
        let old = Arc::new(AtomicBool::new(false));
        let new = Arc::new(AtomicBool::new(false));
        reg.set_media_task("A", g, flag_handle(&old));
        // Replacing the handle for a live call (same generation) aborts the old one, not the new.
        reg.set_media_task("A", g, flag_handle(&new));
        assert!(old.load(Ordering::SeqCst), "replaced task must be aborted");
        assert!(!new.load(Ordering::SeqCst), "the replacement stays live");
        // Cleanup: removing the call aborts the replacement too.
        reg.remove("A");
        assert!(new.load(Ordering::SeqCst), "replacement aborted on remove");
    }

    /// Attaching a media task to an already-removed call must abort the handle immediately so the
    /// task can't outlive the call.
    #[test]
    fn set_media_task_on_unknown_call_aborts_immediately() {
        let reg = CallRegistry::new();
        let flag = Arc::new(AtomicBool::new(false));
        reg.set_media_task("GONE", 0, flag_handle(&flag));
        assert!(
            flag.load(Ordering::SeqCst),
            "an orphan media task must be aborted immediately"
        );
    }

    /// A same-call-id re-offer (retry/glare) replaces the prior call: the old media task is aborted,
    /// and the old generation can no longer reap the replacement. Guards the ABA hazard the example
    /// hit -- a finishing task removing a newer call's handle.
    #[test]
    fn replacement_supersedes_and_old_generation_cannot_reap_it() {
        let reg = CallRegistry::new();
        let g1 = reg.insert(session("CID"));
        let a = Arc::new(AtomicBool::new(false));
        reg.set_media_task("CID", g1, flag_handle(&a));

        // Re-offer with the same id supersedes: aborts task A, fresh generation.
        let g2 = reg.insert(session("CID"));
        assert_ne!(g1, g2);
        assert!(
            a.load(Ordering::SeqCst),
            "the superseded call's task must be aborted on replacement"
        );
        let b = Arc::new(AtomicBool::new(false));
        reg.set_media_task("CID", g2, flag_handle(&b));

        // Task A's stale self-cleanup (old generation) must NOT reap the live replacement.
        assert!(
            !reg.remove_if_current("CID", g1),
            "the old generation must not reap the replacement"
        );
        assert!(!b.load(Ordering::SeqCst), "the replacement task stays live");
        assert_eq!(reg.active_count(), 1);

        // Attaching under the stale generation aborts it immediately (it is for a dead call).
        let stale = Arc::new(AtomicBool::new(false));
        reg.set_media_task("CID", g1, flag_handle(&stale));
        assert!(
            stale.load(Ordering::SeqCst),
            "a stale-generation media task must be aborted"
        );
        assert!(
            !b.load(Ordering::SeqCst),
            "the live replacement is untouched"
        );

        // The current generation reaps correctly.
        assert!(reg.remove_if_current("CID", g2));
        assert!(
            b.load(Ordering::SeqCst),
            "the current generation reap aborts the live task"
        );
        assert_eq!(reg.active_count(), 0);
    }
}

//! A fake media backend for tests, implementing only the neutral contract.
//!
//! This is the architectural gate: if the fake needs `crate::voip::*`, `ResidentMediaSession`,
//! `CallEngine`, `run_call`, `RelayTransport*` or `MediaStatsCell`, the seam is not neutral yet.
//! It compiles under `voip-control` alone, so the compiler is the proof.
//!
//! It records every command it is handed, lets a test publish a [`MediaEvent`] on the session's own
//! subscription, and reports counters a test sets. Its whole point is substitutability: the call
//! registry reserves it, drives it through `submit`/`deliver_group_*`/`stats`, and never learns it
//! is not the resident engine.

use std::sync::{Arc, Mutex};

use crate::types::group_call::GroupCallUpdate;
use crate::voip_control::{
    MediaCloseReason, MediaCommand, MediaDirection, MediaEvent, MediaGroupEpoch, MediaSessionKey,
    MediaSessionSpec, MediaSetupError, MediaStats, VoipMediaBackend, VoipMediaSession,
};

/// One media session the fake handed out, with the commands it received.
#[derive(Debug, Default)]
pub struct FakeSessionRecord {
    /// Every command in submission order, with the answer the session gave.
    pub commands: Vec<(MediaCommand, bool)>,
    /// The reason passed to [`VoipMediaSession::close`], once it is called.
    pub closed: Option<MediaCloseReason>,
    /// Counters the test sets through [`FakeMediaSession::set_stats`].
    pub stats: MediaStats,
}

/// The fake session: records commands, publishes events a test queues, reports set counters.
pub struct FakeMediaSession {
    key: MediaSessionKey,
    record: Mutex<FakeSessionRecord>,
    /// The single event sender, so a test holding the receiver sees what the test publishes.
    events: async_channel::Sender<MediaEvent>,
    /// A cloneable handle to the same stream, handed back by [`VoipMediaSession::subscribe`].
    events_rx: async_channel::Receiver<MediaEvent>,
}

impl FakeMediaSession {
    /// A session and the receiver its events arrive on.
    #[must_use]
    pub fn new_with_key(key: MediaSessionKey) -> (Arc<Self>, async_channel::Receiver<MediaEvent>) {
        let (tx, rx) = async_channel::bounded(16);
        let session = Arc::new(Self {
            key,
            record: Mutex::new(FakeSessionRecord::default()),
            events: tx,
            events_rx: rx.clone(),
        });
        (session, rx)
    }

    /// The generational identity this session was reserved under.
    #[must_use]
    pub fn key(&self) -> &MediaSessionKey {
        &self.key
    }

    /// Snapshot of what this session has seen.
    #[must_use]
    pub fn record(&self) -> FakeSessionRecord {
        let record = self.record.lock().unwrap_or_else(|e| e.into_inner());
        FakeSessionRecord {
            commands: record.commands.clone(),
            closed: record.closed.clone(),
            stats: record.stats,
        }
    }

    /// Publish one media event to this session's subscriber.
    pub fn publish(&self, event: MediaEvent) -> bool {
        self.events.try_send(event).is_ok()
    }

    /// Set the counters [`VoipMediaSession::stats`] reports.
    pub fn set_stats(&self, stats: MediaStats) {
        self.record.lock().unwrap_or_else(|e| e.into_inner()).stats = stats;
    }
}

impl VoipMediaSession for FakeMediaSession {
    fn submit(&self, command: MediaCommand) -> bool {
        self.record
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .commands
            .push((command, true));
        true
    }

    fn group_update_fits(&self, _update: &GroupCallUpdate, _is_call_link: bool) -> bool {
        true
    }

    fn stats(&self) -> MediaStats {
        self.record.lock().unwrap_or_else(|e| e.into_inner()).stats
    }

    fn subscribe(&self) -> async_channel::Receiver<MediaEvent> {
        // A clone over the same bounded stream, so every subscriber sees the published events.
        self.events_rx.clone()
    }

    fn close(&self, reason: MediaCloseReason) {
        self.record.lock().unwrap_or_else(|e| e.into_inner()).closed = Some(reason);
    }
}

/// The fake backend: hands out [`FakeMediaSession`]s and remembers them by key.
#[derive(Default)]
pub struct FakeMediaBackend {
    sessions: Mutex<Vec<Arc<FakeMediaSession>>>,
    /// Set when a test wants `open` to refuse, proving the control plane propagates the error.
    refuse_open: bool,
}

impl FakeMediaBackend {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A backend whose `open` fails, so a test can assert the typed refusal propagates.
    #[must_use]
    pub fn refusing() -> Self {
        Self {
            refuse_open: true,
            ..Self::default()
        }
    }

    /// Every session this backend has reserved, in order.
    #[must_use]
    pub fn sessions(&self) -> Vec<Arc<FakeMediaSession>> {
        self.sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }

    /// The session reserved under `key`, if any.
    #[must_use]
    pub fn session(&self, key: &MediaSessionKey) -> Option<Arc<FakeMediaSession>> {
        self.sessions()
            .into_iter()
            .find(|session| session.key() == key)
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl VoipMediaBackend for FakeMediaBackend {
    fn reserve(
        &self,
        key: &MediaSessionKey,
        _direction: MediaDirection,
    ) -> Arc<dyn VoipMediaSession> {
        let (session, _rx) = FakeMediaSession::new_with_key(key.clone());
        self.sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(session.clone());
        session
    }

    async fn open(
        &self,
        _session: &Arc<dyn VoipMediaSession>,
        _spec: MediaSessionSpec,
    ) -> Result<(), MediaSetupError> {
        if self.refuse_open {
            return Err(MediaSetupError::Backend("fake backend refused".into()));
        }
        Ok(())
    }
}

/// A group epoch a test can build without naming engine key types.
#[must_use]
pub fn fake_epoch(transaction_id: u32, bytes: Vec<u8>) -> (u32, MediaGroupEpoch) {
    (transaction_id, MediaGroupEpoch::new(bytes))
}

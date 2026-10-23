//! Host-owned, opt-in presence maintenance, separate from transport keepalive.
//! A completed pulse is evidence of two successful writes, not a server activity
//! acknowledgement or a guarantee against linked-device expiry.
use std::future::Future;
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::sync::watch;
use tokio::time::Instant;

#[derive(Clone, Copy)]
pub struct Settings {
    pub enabled: bool,
    pub interval: Duration,
    pub available_for: Duration,
    pub retry_after: Duration,
    pub poll_every: Duration,
    pub write_timeout: Duration,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            enabled: true,
            interval: Duration::from_secs(24 * 60 * 60),
            available_for: Duration::from_secs(45),
            retry_after: Duration::from_secs(5 * 60),
            poll_every: Duration::from_secs(5),
            write_timeout: Duration::from_secs(3),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notice {
    Available,
    Completed,
    Interrupted,
    AvailableFailed,
    RestoreFailed,
    Restored,
    CheckpointFailed,
    RecoveryRequested,
}

/// One owner per session. Generation must advance synchronously for every
/// connection replacement, even if a disconnect/reconnect occurs between ticks.
/// Implementations must not log account identifiers or raw transport errors.
pub trait Host: Send + Sync {
    fn generation(&self) -> Option<u64>;
    fn can_pulse(&self) -> impl Future<Output = bool> + Send;
    fn set_available(&self) -> impl Future<Output = Result<()>> + Send;
    fn set_unavailable(&self) -> impl Future<Output = Result<()>> + Send;
    fn load_success(&self) -> impl Future<Output = Result<Option<u64>>> + Send;
    fn save_success(&self, timestamp_ms: u64) -> impl Future<Output = Result<()>> + Send;
    fn notice(&self, notice: Notice);
    fn recover_transport(&self);
}

fn now_ms() -> Result<u64> {
    wacore::time::now_millis()
        .try_into()
        .context("clock before unix epoch")
}

async fn write(result: impl Future<Output = Result<()>>, timeout: Duration) -> bool {
    matches!(tokio::time::timeout(timeout, result).await, Ok(Ok(())))
}

pub struct Maintenance {
    settings: Settings,
    restored: Option<u64>,
    pulse: Option<(u64, Instant)>,
    last_success: Option<u64>,
    loaded: bool,
    pending_save: Option<u64>,
    retry_at: Instant,
    possibly_available: bool,
    restore_failures: u8,
}

impl Maintenance {
    pub fn new(settings: Settings) -> Self {
        Self {
            settings,
            restored: None,
            pulse: None,
            last_success: None,
            loaded: false,
            pending_save: None,
            retry_at: Instant::now(),
            possibly_available: false,
            restore_failures: 0,
        }
    }

    /// Does at most one transition. Writes are bounded and never cancelled by
    /// the stop signal: their outcome may be uncertain and needs compensation.
    pub async fn tick(&mut self, host: &impl Host, now: u64) {
        // A completed cycle is local durable work, independent of connectivity.
        // Failed storage must not prevent a new connection restoring offline.
        let checkpoint_saved = self.flush_checkpoint(host).await;
        let generation = host.generation();
        if self.restored != generation {
            self.restored = None;
        }
        if self
            .pulse
            .is_some_and(|(epoch, _)| Some(epoch) != generation)
        {
            self.pulse = None;
            host.notice(Notice::Interrupted);
        }
        let Some(generation) = generation else {
            self.possibly_available = false;
            return;
        };
        if self.restored.is_none() {
            if !write(host.set_unavailable(), self.settings.write_timeout).await {
                self.failed_restore(host);
                return;
            }
            if host.generation() != Some(generation) {
                return;
            }
            self.possibly_available = false;
            self.restore_failures = 0;
            self.restored = Some(generation);
            host.notice(Notice::Restored);
            return;
        }
        if let Some((epoch, until)) = self.pulse {
            if Instant::now() < until {
                return;
            }
            // Remove before writing: a failed restoration must never count as a
            // completed pulse, even if the following retry restores availability.
            self.pulse = None;
            self.restored = None;
            self.retry_at = Instant::now() + self.settings.retry_after;
            if !write(host.set_unavailable(), self.settings.write_timeout).await {
                self.failed_restore(host);
                return;
            }
            if host.generation() != Some(epoch) {
                host.notice(Notice::Interrupted);
                return;
            }
            self.possibly_available = false;
            self.restore_failures = 0;
            self.restored = Some(epoch);
            self.last_success = Some(now);
            self.pending_save = Some(now);
            if !self.flush_checkpoint(host).await {
                return;
            }
        }
        if !checkpoint_saved {
            return;
        }
        if !self.loaded {
            match tokio::time::timeout(self.settings.write_timeout, host.load_success()).await {
                Ok(Ok(last)) => {
                    self.last_success = last;
                    self.loaded = true;
                }
                _ => {
                    host.notice(Notice::CheckpointFailed);
                    return;
                }
            }
        }
        if !self.settings.enabled
            || Instant::now() < self.retry_at
            || self.last_success.is_some_and(|last| {
                now.saturating_sub(last) < self.settings.interval.as_millis() as u64
            })
        {
            return;
        }
        if !matches!(
            tokio::time::timeout(self.settings.write_timeout, host.can_pulse()).await,
            Ok(true)
        ) || host.generation() != Some(generation)
        {
            return;
        }
        self.retry_at = Instant::now() + self.settings.retry_after;
        // An error/timeout can still mean the stanza reached the server. Force
        // unavailable on the next tick and during shutdown even on this path.
        self.restored = None;
        self.possibly_available = true;
        if !write(host.set_available(), self.settings.write_timeout).await {
            host.notice(Notice::AvailableFailed);
            self.restore(host).await;
            return;
        }
        if host.generation() != Some(generation) {
            host.notice(Notice::Interrupted);
            self.restore(host).await;
            return;
        }
        self.restored = Some(generation);
        self.pulse = Some((generation, Instant::now() + self.settings.available_for));
        host.notice(Notice::Available);
    }

    async fn flush_checkpoint(&mut self, host: &impl Host) -> bool {
        let Some(timestamp) = self.pending_save else {
            return true;
        };
        if !write(host.save_success(timestamp), self.settings.write_timeout).await {
            host.notice(Notice::CheckpointFailed);
            return false;
        }
        self.pending_save = None;
        host.notice(Notice::Completed);
        true
    }

    fn failed_restore(&mut self, host: &impl Host) {
        host.notice(Notice::RestoreFailed);
        self.restore_failures = self.restore_failures.saturating_add(1);
        if self.possibly_available && self.restore_failures >= 3 {
            host.notice(Notice::RecoveryRequested);
            host.recover_transport();
        }
    }

    pub async fn restore(&mut self, host: &impl Host) -> bool {
        self.pulse = None;
        self.restored = None;
        if host.generation().is_none() {
            return true;
        }
        if write(host.set_unavailable(), self.settings.write_timeout).await {
            self.possibly_available = false;
            self.restore_failures = 0;
            host.notice(Notice::Restored);
            true
        } else {
            self.failed_restore(host);
            false
        }
    }
}

/// Signal stop and join this future BEFORE closing the socket. Do not abort it.
/// A hard process kill cannot run cleanup; every new connection first restores
/// unavailable. Dropped stop senders also request cleanup.
pub async fn run(host: impl Host, settings: Settings, mut stop: watch::Receiver<bool>) {
    let mut state = Maintenance::new(settings);
    loop {
        if *stop.borrow() || stop.has_changed().is_err() {
            break;
        }
        match now_ms() {
            Ok(now) => state.tick(&host, now).await,
            Err(_) => {
                host.notice(Notice::CheckpointFailed);
                state.restore(&host).await;
            }
        }
        let delay = state.pulse.map_or(settings.poll_every, |(_, until)| {
            settings
                .poll_every
                .min(until.saturating_duration_since(Instant::now()))
        });
        tokio::select! {
            _ = stop.changed() => {},
            _ = tokio::time::sleep(delay) => {},
        }
    }
    // A bounded second attempt helps on a transient write failure. If both fail,
    // the owner closes the transport, and reconnect restores unavailable first.
    if !state.restore(&host).await {
        state.restore(&host).await;
    }
    // One bounded attempt even offline. An unfinished pulse has no checkpoint.
    state.flush_checkpoint(&host).await;
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;

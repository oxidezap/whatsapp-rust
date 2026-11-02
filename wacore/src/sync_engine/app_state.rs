use crate::appstate::patch_decode::WAPatchName;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// A deferred run keeps its trigger owed, even if earlier pages were committed.
pub enum SyncOutcome {
    Completed,
    Deferred,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Admission is checked before effects. Committed mutations must still be published after retirement.
pub struct SyncScope {
    pub generation: u64,
    pub deadline: Option<crate::time::Instant>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeLost {
    Retired,
    Expired,
}

impl SyncScope {
    #[inline]
    pub fn admits(
        self,
        generation: u64,
        now: impl FnOnce() -> crate::time::Instant,
    ) -> Result<(), ScopeLost> {
        if self.generation != generation {
            return Err(ScopeLost::Retired);
        }
        if let Some(deadline) = self.deadline
            && now() >= deadline
        {
            return Err(ScopeLost::Expired);
        }
        Ok(())
    }

    #[inline]
    pub fn generation(self) -> u64 {
        self.generation
    }

    #[inline]
    pub fn remaining_with(self, now: impl FnOnce() -> crate::time::Instant) -> Option<Duration> {
        self.deadline.map(|d| d.saturating_duration_since(now()))
    }

    #[inline]
    pub fn is_bootstrap(self) -> bool {
        self.deadline.is_some()
    }

    #[inline]
    pub fn rebind(&mut self, to: u64) -> bool {
        let moved = self.generation != to;
        self.generation = to;
        moved
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncHolder {
    Sync,
    PatchSend,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReservationSkip {
    EquivalentSyncInFlight,
    WaitTimedOut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SyncSettles {
    JustTheCollections,
    InitialSync,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
/// Only a matching fetch can discharge another fetch; a patch writer never does.
pub enum ReservationWait {
    SkipBehindSync,
    Always,
    TryOnce,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct BatchedSyncRequest {
    pub rebuild: bool,
    pub wait_for_holder: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StandDown {
    Done,
    Declined(ScopeLost),
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
/// Partial results distinguish permanent refusals, future retries and work covered by other holders.
pub struct BatchedSyncOutcome {
    pub synced: Vec<WAPatchName>,
    pub fatal: Vec<WAPatchName>,
    pub retryable: Vec<WAPatchName>,
    pub skipped: Vec<WAPatchName>,
    pub reached_server: bool,
}

impl BatchedSyncOutcome {
    #[inline]
    pub fn reached_server(&self) -> bool {
        self.reached_server
    }

    #[inline]
    pub fn note_reached_server(&mut self) {
        self.reached_server = true;
    }

    #[inline]
    pub fn unsynced(&self) -> impl Iterator<Item = WAPatchName> + '_ {
        self.fatal
            .iter()
            .chain(&self.retryable)
            .chain(&self.skipped)
            .copied()
    }

    #[inline]
    pub fn all_synced(&self) -> bool {
        self.unsynced().next().is_none()
    }

    #[inline]
    pub fn all_retryable(requested: &[WAPatchName]) -> Self {
        Self {
            retryable: requested.to_vec(),
            ..Default::default()
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct CriticalSyncPlan {
    pub retry: Vec<WAPatchName>,
    pub stranded: bool,
}

impl CriticalSyncPlan {
    #[inline]
    pub fn from_outcome(outcome: &BatchedSyncOutcome) -> Self {
        let BatchedSyncOutcome {
            synced: _,
            fatal,
            retryable,
            skipped,
            reached_server: _,
        } = outcome;
        let retry: Vec<WAPatchName> = retryable.iter().chain(skipped).copied().collect();
        Self {
            retry,
            stranded: !fatal.is_empty(),
        }
    }

    #[inline]
    pub fn outstanding(&self) -> bool {
        self.stranded || !self.retry.is_empty()
    }
}

pub const RETRY_BACKOFF_MIN: Duration = Duration::from_secs(1);
pub const RETRY_BACKOFF_MAX: Duration = Duration::from_secs(60 * 60);
pub const RETRY_MAX_ATTEMPTS: u32 = 8;
const RETRY_MAX_ROUNDS: u32 = RETRY_MAX_ATTEMPTS * 4;

#[inline]
pub fn retry_backoff(attempts: u32) -> Duration {
    RETRY_BACKOFF_MIN
        .saturating_mul(2u32.saturating_pow(attempts))
        .min(RETRY_BACKOFF_MAX)
}

/// Waiting for a writer consumes a round but does not consume a server attempt.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RetryBudget {
    attempts: u32,
    rounds: u32,
}

impl RetryBudget {
    #[inline]
    pub fn next_delay(&mut self) -> Option<Duration> {
        if self.attempts >= RETRY_MAX_ATTEMPTS || self.rounds >= RETRY_MAX_ROUNDS {
            return None;
        }
        self.rounds += 1;
        Some(retry_backoff(self.attempts))
    }

    #[inline]
    pub fn record_attempt(&mut self) {
        self.attempts = self.attempts.saturating_add(1);
    }

    #[inline]
    pub fn attempts(&self) -> u32 {
        self.attempts
    }
}

impl ReservationWait {
    #[inline]
    pub fn blocked_by(self, holder: SyncHolder) -> Option<ReservationSkip> {
        match (self, holder) {
            (Self::SkipBehindSync, SyncHolder::Sync) => {
                Some(ReservationSkip::EquivalentSyncInFlight)
            }
            (Self::TryOnce, _) => Some(ReservationSkip::WaitTimedOut),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unaccounted {
    Omitted,
    NotApplied,
}

#[inline]
pub fn unaccounted_for(responded: bool, answered: bool) -> Option<Unaccounted> {
    match (responded, answered) {
        (_, true) => None,
        (true, false) => Some(Unaccounted::NotApplied),
        (false, false) => Some(Unaccounted::Omitted),
    }
}

#[inline]
pub fn initial_key_retry(timeout: Duration) -> Duration {
    (timeout / 2)
        .max(Duration::from_millis(1))
        .min(Duration::from_secs(10))
}

#[inline]
pub fn next_key_retry(previous: Duration) -> Duration {
    previous.saturating_mul(2).min(Duration::from_secs(60))
}

/// The SDK must apply these proposals with compare-and-swap and retry on contention.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BootstrapState(u64);

impl BootstrapState {
    pub const fn from_word(word: u64) -> Self {
        Self(word)
    }

    pub const fn new(outstanding: bool) -> Self {
        Self(outstanding as u64)
    }

    pub const fn word(self) -> u64 {
        self.0
    }

    pub const fn is_armed(self) -> bool {
        self.0 & 1 == 1
    }

    #[inline]
    pub fn arm_for_pairing(self, current_generation: u64) -> Self {
        let generation = (self.0 >> 1).max(current_generation).saturating_add(1);
        Self((generation << 1) | 1)
    }

    #[inline]
    pub fn settle(self, generation: u64, outstanding: bool) -> Option<Self> {
        if (self.0 >> 1) > generation {
            None
        } else {
            Some(Self((generation << 1) | outstanding as u64))
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RetryProgress {
    Pending,
    Complete { bootstrap_outstanding: Option<bool> },
}

/// Fatal and skipped collections remain owed after the last retryable collection succeeds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CollectionRetryState {
    settles: SyncSettles,
    stranded: bool,
}

impl CollectionRetryState {
    #[inline]
    pub fn new(settles: SyncSettles, stranded: bool) -> Self {
        Self { settles, stranded }
    }

    /// Work survives a reconnect; authority to settle its original bootstrap does not.
    #[inline]
    pub fn rebind(&mut self, scope: &mut SyncScope, generation: u64) -> bool {
        if scope.rebind(generation) {
            self.settles = SyncSettles::JustTheCollections;
            true
        } else {
            false
        }
    }

    #[inline]
    pub fn observe(&mut self, outcome: &BatchedSyncOutcome) -> RetryProgress {
        self.stranded |= !outcome.fatal.is_empty() || !outcome.skipped.is_empty();
        if outcome.retryable.is_empty() {
            RetryProgress::Complete {
                bootstrap_outstanding: (self.settles == SyncSettles::InitialSync)
                    .then_some(self.stranded),
            }
        } else {
            RetryProgress::Pending
        }
    }
}

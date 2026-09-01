//! Pluggable time providers.
//!
//! Two clocks live here, kept distinct on purpose:
//!
//! - **Wall clock** ([`TimeProvider`], [`now_millis`], [`now_utc`]) — answers
//!   "what time is it?". Can jump backwards (NTP sync, manual clock changes,
//!   leap-second smearing). Resolution: milliseconds is sufficient for
//!   timestamps in stanzas, app-state mutations, and log lines.
//! - **Monotonic clock** ([`MonotonicProvider`], [`Instant`]) — answers
//!   "how much time passed?". Never moves backwards; immune to NTP
//!   adjustments. Resolution: nanoseconds where the platform supports it
//!   (`std::time::Instant` on native; sub-millisecond from `performance.now`
//!   in browsers, true ns in Node/WASI when those targets supply a custom
//!   provider).
//!
//! Conflating the two — using a wall clock to measure elapsed time — silently
//! corrupts timeouts and latency metrics whenever the system clock is adjusted
//! mid-measurement. That is why `std::time` separates `SystemTime` from
//! `Instant`, and we mirror the split here.

use std::sync::OnceLock;

/// Test-only clock-read accounting, so a budget over the hot path can be
/// asserted instead of estimated.
///
/// Counting sits at the abstraction boundary, not inside a provider:
/// [`set_time_provider`] is a `OnceLock` that the process's first `now_millis()`
/// fills with the default, so a suite sharing one process cannot install an
/// instrumented provider per test. Counting here also covers the defaults.
#[cfg(feature = "test-util")]
pub mod clock_reads {
    use core::cell::Cell;

    std::thread_local! {
        static WALL: Cell<u64> = const { Cell::new(0) };
        static MONOTONIC: Cell<u64> = const { Cell::new(0) };
    }

    /// Reads counted on one thread.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct Reads {
        /// Reads of the wall clock ([`super::now_millis`] and everything built
        /// on it).
        pub wall: u64,
        /// Reads of the monotonic clock ([`super::Instant::now`],
        /// [`super::Instant::elapsed`]).
        pub monotonic: u64,
    }

    impl Reads {
        pub fn total(&self) -> u64 {
            self.wall + self.monotonic
        }
    }

    #[inline]
    pub(super) fn bump_wall() {
        WALL.with(|c| c.set(c.get().saturating_add(1)));
    }

    #[inline]
    pub(super) fn bump_monotonic() {
        MONOTONIC.with(|c| c.set(c.get().saturating_add(1)));
    }

    /// Counters for the calling thread.
    ///
    /// Per thread, not process-wide, so a measurement stays exact while the
    /// rest of the suite runs in parallel. Work handed to a blocking pool (the
    /// storage backend) is counted on that thread and stays invisible here.
    pub fn snapshot() -> Reads {
        Reads {
            wall: WALL.with(Cell::get),
            monotonic: MONOTONIC.with(Cell::get),
        }
    }

    /// Reads made on this thread since `base`.
    pub fn since(base: Reads) -> Reads {
        let now = snapshot();
        Reads {
            wall: now.wall.saturating_sub(base.wall),
            monotonic: now.monotonic.saturating_sub(base.monotonic),
        }
    }
}

/// Wall-clock provider. Returns the current Unix time. May move backwards
/// across calls when the system clock is adjusted.
pub trait TimeProvider: Send + Sync + 'static {
    /// Current time as milliseconds since Unix epoch.
    fn now_millis(&self) -> i64;
}

/// Default wall-clock provider using `chrono` (native targets only).
///
/// cfg-gated off `wasm32` for the same reason the monotonic clock is: there is
/// no backend for `chrono::Utc::now()` on `wasm32-unknown-unknown` (we don't pull
/// `wasmbind`), so it falls through to `SystemTime::now()`, which panics. The
/// wasm default is [`UnsetWasmTimeProvider`].
#[cfg(not(target_arch = "wasm32"))]
struct ChronoTimeProvider;

#[cfg(not(target_arch = "wasm32"))]
impl TimeProvider for ChronoTimeProvider {
    // The single legitimate call to `chrono::Utc::now()`: this IS the default
    // provider backing `wacore::time::now_utc()`. Everywhere else must go
    // through the abstraction — see clippy.toml.
    #[allow(clippy::disallowed_methods)]
    fn now_millis(&self) -> i64 {
        chrono::Utc::now().timestamp_millis()
    }
}

/// WASM default when no wall-clock provider is registered. Unlike the monotonic
/// clock, the wall clock has no internal source on `wasm32` (and we won't panic
/// like `chrono::Utc::now()` would), so this returns epoch (0) and warns once.
/// Embedders MUST call [`set_time_provider`] with a real provider (e.g. backed
/// by `Date.now()`) before the first timestamp.
#[cfg(target_arch = "wasm32")]
struct UnsetWasmTimeProvider;

#[cfg(target_arch = "wasm32")]
impl TimeProvider for UnsetWasmTimeProvider {
    fn now_millis(&self) -> i64 {
        use std::sync::atomic::{AtomicBool, Ordering};
        static WARNED: AtomicBool = AtomicBool::new(false);
        if !WARNED.swap(true, Ordering::Relaxed) {
            log::warn!(
                "wacore::time: no wall-clock provider set on wasm32; returning epoch. \
                 Call set_time_provider() before the first timestamp."
            );
        }
        0
    }
}

static TIME_PROVIDER: OnceLock<Box<dyn TimeProvider>> = OnceLock::new();

/// Set a custom wall-clock provider. Must be called before any time function
/// is used. Returns `Err` if a provider has already been set.
pub fn set_time_provider(provider: impl TimeProvider) -> Result<(), &'static str> {
    TIME_PROVIDER
        .set(Box::new(provider))
        .map_err(|_| "time provider already set")
}

/// Current time in milliseconds since Unix epoch.
#[cfg(not(target_arch = "wasm32"))]
#[inline]
pub fn now_millis() -> i64 {
    #[cfg(feature = "test-util")]
    clock_reads::bump_wall();
    TIME_PROVIDER
        .get_or_init(default_time_provider)
        .now_millis()
}

/// On wasm32 the epoch fallback is used transiently and never stored in the
/// `OnceLock`, so a later `set_time_provider()` always wins even if an early
/// timestamp already ran during initialization.
#[cfg(target_arch = "wasm32")]
#[inline]
pub fn now_millis() -> i64 {
    #[cfg(feature = "test-util")]
    clock_reads::bump_wall();
    match TIME_PROVIDER.get() {
        Some(provider) => provider.now_millis(),
        None => UnsetWasmTimeProvider.now_millis(),
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn default_time_provider() -> Box<dyn TimeProvider> {
    Box::new(ChronoTimeProvider)
}

/// Current time in seconds since Unix epoch.
#[inline]
pub fn now_secs() -> i64 {
    now_millis() / 1000
}

/// Current time in seconds since Unix epoch, saturated at 0.
///
/// Most stanza encodings carry timestamps as unsigned u64. A naive
/// `now_secs() as u64` silently wraps when the clock is pre-1970 (e.g. an
/// uninitialized system clock during early boot) and corrupts the stanza.
/// This helper clamps to 0 instead.
#[inline]
pub fn now_secs_u64() -> u64 {
    now_secs().max(0) as u64
}

/// Current time as `chrono::DateTime<Utc>`.
#[inline]
pub fn now_utc() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp_millis(now_millis())
        .expect("time provider returned out-of-range millisecond timestamp")
}

/// Convert a Unix timestamp (seconds) to `DateTime<Utc>`.
/// Returns `None` for out-of-range values.
#[inline]
pub fn from_secs(ts: i64) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::from_timestamp(ts, 0)
}

/// Convert a Unix timestamp (seconds) to `DateTime<Utc>`,
/// falling back to `now_utc()` for out-of-range values.
#[inline]
pub fn from_secs_or_now(ts: i64) -> chrono::DateTime<chrono::Utc> {
    from_secs(ts).unwrap_or_else(now_utc)
}

/// Convert a Unix timestamp (milliseconds) to `DateTime<Utc>`.
/// Returns `None` for out-of-range values.
#[inline]
pub fn from_millis(ts: i64) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::from_timestamp_millis(ts)
}

/// Convert a Unix timestamp (milliseconds) to `DateTime<Utc>`,
/// falling back to `now_utc()` for out-of-range values.
#[inline]
pub fn from_millis_or_now(ts: i64) -> chrono::DateTime<chrono::Utc> {
    from_millis(ts).unwrap_or_else(now_utc)
}

// ---------------------------------------------------------------------------
// Monotonic clock
// ---------------------------------------------------------------------------

/// Monotonic-clock provider. Returns nanoseconds since an arbitrary fixed
/// reference; the only guarantee is that successive calls never return a
/// smaller value than a previous one.
pub trait MonotonicProvider: Send + Sync + 'static {
    /// Nanoseconds since the provider's reference point. The reference is
    /// implementation-defined (may be process start, system boot, etc.) —
    /// only differences are meaningful.
    fn now_nanos(&self) -> u64;
}

/// Native default: backed by `std::time::Instant`, which is monotonic and
/// has nanosecond resolution on every supported native platform.
#[cfg(not(target_arch = "wasm32"))]
struct StdMonotonicProvider {
    epoch: std::time::Instant,
}

#[cfg(not(target_arch = "wasm32"))]
impl StdMonotonicProvider {
    // The single legitimate call to `std::time::Instant::now()`: this IS the
    // default provider backing `wacore::time::Instant`. Everywhere else must
    // go through this abstraction — see clippy.toml.
    #[allow(clippy::disallowed_methods)]
    fn new() -> Self {
        Self {
            epoch: std::time::Instant::now(),
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl MonotonicProvider for StdMonotonicProvider {
    fn now_nanos(&self) -> u64 {
        // Saturate at u64::MAX (584 years) — well beyond any realistic
        // process lifetime.
        self.epoch.elapsed().as_nanos().min(u64::MAX as u128) as u64
    }
}

/// WASM fallback when no platform provider is registered. Derives nanos
/// from the wall clock and clamps to non-decreasing so the trait contract
/// holds across NTP backjumps (the value freezes until the wall clock
/// catches up). Resolution is ms × 1_000_000; embedders should register
/// a real provider via [`set_monotonic_provider`] for sub-ms precision.
#[cfg(target_arch = "wasm32")]
struct WallDerivedMonotonicProvider {
    last: portable_atomic::AtomicU64,
}

#[cfg(target_arch = "wasm32")]
impl WallDerivedMonotonicProvider {
    const fn new() -> Self {
        Self {
            last: portable_atomic::AtomicU64::new(0),
        }
    }
}

#[cfg(target_arch = "wasm32")]
impl MonotonicProvider for WallDerivedMonotonicProvider {
    fn now_nanos(&self) -> u64 {
        use std::sync::atomic::Ordering;
        // Warn once: a caller on this fallback is measuring timeouts against a
        // clock that still follows the wall clock forward, so a system-clock
        // correction can fire the dead-socket watchdog on a healthy socket.
        // The clamp below only rules out the backwards half of that.
        {
            use std::sync::atomic::AtomicBool;
            static WARNED: AtomicBool = AtomicBool::new(false);
            if !WARNED.swap(true, Ordering::Relaxed) {
                log::warn!(
                    "wacore::time: no monotonic provider set on wasm32; deriving from the \
                     wall clock, so a forward clock adjustment reads as elapsed time. \
                     Call set_monotonic_provider() with performance.now() before the first timeout."
                );
            }
        }
        let raw = (now_millis().max(0) as u64).saturating_mul(1_000_000);
        let mut last = self.last.load(Ordering::Relaxed);
        loop {
            let next = raw.max(last);
            match self
                .last
                .compare_exchange_weak(last, next, Ordering::Relaxed, Ordering::Relaxed)
            {
                Ok(_) => return next,
                Err(observed) => last = observed,
            }
        }
    }
}

static MONOTONIC_PROVIDER: OnceLock<Box<dyn MonotonicProvider>> = OnceLock::new();

/// Set a custom monotonic-clock provider. Must be called before any
/// [`Instant`] is captured. Returns `Err` if a provider has already been set.
pub fn set_monotonic_provider(provider: impl MonotonicProvider) -> Result<(), &'static str> {
    MONOTONIC_PROVIDER
        .set(Box::new(provider))
        .map_err(|_| "monotonic provider already set")
}

#[inline]
fn now_nanos() -> u64 {
    #[cfg(feature = "test-util")]
    clock_reads::bump_monotonic();
    MONOTONIC_PROVIDER
        .get_or_init(default_monotonic_provider)
        .now_nanos()
}

#[cfg(not(target_arch = "wasm32"))]
fn default_monotonic_provider() -> Box<dyn MonotonicProvider> {
    Box::new(StdMonotonicProvider::new())
}

#[cfg(target_arch = "wasm32")]
fn default_monotonic_provider() -> Box<dyn MonotonicProvider> {
    Box::new(WallDerivedMonotonicProvider::new())
}

/// Portable monotonic instant. On native targets this wraps `std::time::Instant`
/// (via the default [`MonotonicProvider`]) and exposes nanosecond resolution.
/// On `wasm32` targets the embedder should register a sub-millisecond provider
/// via [`set_monotonic_provider`]; otherwise the fallback derives from the
/// wall clock and quantizes to milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Instant(u64);

impl Instant {
    /// The origin of the configured monotonic clock.
    ///
    /// Useful as a storage sentinel when a component has time-based behavior
    /// disabled and therefore must not read the platform clock.
    pub const ZERO: Self = Self(0);

    /// Capture the current monotonic instant.
    #[inline]
    pub fn now() -> Self {
        Self(now_nanos())
    }

    /// Duration elapsed since this instant was captured. Returns
    /// [`Duration::ZERO`](std::time::Duration::ZERO) if the clock somehow reported a smaller value
    /// than at capture time — which a well-behaved monotonic provider
    /// must never do, but we saturate defensively.
    #[inline]
    pub fn elapsed(&self) -> std::time::Duration {
        let now = now_nanos();
        std::time::Duration::from_nanos(now.saturating_sub(self.0))
    }

    /// Duration from `earlier` to `self`. Returns [`Duration::ZERO`](std::time::Duration::ZERO) if
    /// `earlier` is after `self`.
    #[inline]
    pub fn saturating_duration_since(&self, earlier: Instant) -> std::time::Duration {
        std::time::Duration::from_nanos(self.0.saturating_sub(earlier.0))
    }
}

impl std::ops::Add<std::time::Duration> for Instant {
    type Output = Instant;
    fn add(self, rhs: std::time::Duration) -> Self {
        let rhs_nanos: u64 = rhs.as_nanos().min(u64::MAX as u128) as u64;
        Self(self.0.saturating_add(rhs_nanos))
    }
}

impl std::ops::Sub<std::time::Duration> for Instant {
    type Output = Instant;
    fn sub(self, rhs: std::time::Duration) -> Self {
        let rhs_nanos: u64 = rhs.as_nanos().min(u64::MAX as u128) as u64;
        Self(self.0.saturating_sub(rhs_nanos))
    }
}

impl std::ops::Sub<Instant> for Instant {
    type Output = std::time::Duration;
    fn sub(self, rhs: Instant) -> std::time::Duration {
        self.saturating_duration_since(rhs)
    }
}

/// A shared [`Instant`] slot that can also be "unset", for anchors several
/// tasks stamp and clear.
///
/// Exists because the anchors it holds are read on the wire path: storing an
/// `Instant` behind a mutex would put a lock on every frame written, and the
/// alternative of a plain `AtomicU64` of milliseconds is what the dead-socket
/// watchdog used to be, back when a clock adjustment could fire it.
///
/// The encoding keeps "unset" distinct from "captured at the clock's origin",
/// which a bare zero cannot: on `wasm32` with no wall-clock provider the
/// fallback monotonic source answers 0 forever, so a zero sentinel would read
/// every fresh stamp as unset.
#[derive(Debug, Default)]
pub struct AtomicInstant(portable_atomic::AtomicU64);

impl AtomicInstant {
    /// Encoded as nanos + 1 so that 0 means unset, which reserves the top
    /// nanosecond: `u64::MAX` and `u64::MAX - 1` both encode to `u64::MAX`.
    /// That ceiling is inherited, not introduced -- [`StdMonotonicProvider`]
    /// already saturates there, 584 years past any process start.
    fn encode(instant: Instant) -> u64 {
        instant.0.saturating_add(1)
    }

    fn decode(raw: u64) -> Option<Instant> {
        raw.checked_sub(1).map(Instant)
    }

    /// A slot holding no instant.
    pub const fn unset() -> Self {
        Self(portable_atomic::AtomicU64::new(0))
    }

    /// The stored instant, or `None` while unset.
    #[inline]
    pub fn load(&self) -> Option<Instant> {
        Self::decode(self.0.load(portable_atomic::Ordering::Relaxed))
    }

    #[inline]
    pub fn store(&self, instant: Instant) {
        self.0
            .store(Self::encode(instant), portable_atomic::Ordering::Relaxed);
    }

    /// Back to unset.
    #[inline]
    pub fn clear(&self) {
        self.0.store(0, portable_atomic::Ordering::Relaxed);
    }

    /// Read and unset in one step.
    #[inline]
    pub fn take(&self) -> Option<Instant> {
        Self::decode(self.0.swap(0, portable_atomic::Ordering::Relaxed))
    }

    /// Store `instant` only while `should_replace` still accepts the value
    /// actually in the slot, retrying on a racing write. Returns whether the
    /// store landed.
    #[inline]
    pub fn store_if(
        &self,
        instant: Instant,
        mut should_replace: impl FnMut(Option<Instant>) -> bool,
    ) -> bool {
        self.0
            .fetch_update(
                portable_atomic::Ordering::Relaxed,
                portable_atomic::Ordering::Relaxed,
                |current| should_replace(Self::decode(current)).then(|| Self::encode(instant)),
            )
            .is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── AtomicInstant ────────────────────────────────────────────────────────

    #[test]
    fn an_atomic_instant_round_trips_and_clears() {
        let slot = AtomicInstant::unset();
        assert_eq!(slot.load(), None, "a fresh slot holds nothing");

        let at = Instant::now();
        slot.store(at);
        assert_eq!(slot.load(), Some(at));

        assert_eq!(slot.take(), Some(at), "take yields what was there");
        assert_eq!(slot.load(), None, "and leaves the slot unset");

        slot.store(at);
        slot.clear();
        assert_eq!(slot.load(), None);
    }

    /// The clock's own origin is a real instant, not the absence of one: on
    /// `wasm32` with no wall-clock provider the fallback monotonic source
    /// answers zero forever, and a zero sentinel would read every stamp there
    /// as "never armed" and silently disable the dead-socket watchdog.
    #[test]
    fn the_clock_origin_is_a_value_not_an_absence() {
        let slot = AtomicInstant::unset();
        slot.store(Instant::ZERO);
        assert_eq!(slot.load(), Some(Instant::ZERO));
    }

    #[test]
    fn store_if_respects_the_predicate() {
        let slot = AtomicInstant::unset();
        let first = Instant::now();
        assert!(slot.store_if(first, |current| current.is_none()));
        assert_eq!(slot.load(), Some(first));

        // Bad path: the slot is taken, so the second store must not land.
        let second = first + std::time::Duration::from_secs(1);
        assert!(!slot.store_if(second, |current| current.is_none()));
        assert_eq!(
            slot.load(),
            Some(first),
            "an occupied slot is not clobbered"
        );
    }

    // The native default must yield a real wall-clock time, not the wasm fallback's
    // epoch. Guards the cfg-split refactor that keeps wasm32 from panicking.
    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_default_time_provider_returns_real_time() {
        let ms = default_time_provider().now_millis();
        assert!(
            ms > 1_600_000_000_000,
            "expected a post-2020 timestamp, got {ms}"
        );
    }
}

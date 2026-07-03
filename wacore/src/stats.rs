//! Per-session resource accounting: wire I/O counters, retained-memory
//! estimation and a runtime-agnostic task instrumentation hook.
//!
//! Everything here is dependency-free and portable (wasm32/ESP32): counters
//! use `portable_atomic`, CPU metering reads the pluggable
//! [`crate::time::Instant`] clock, and nothing knows which executor or
//! allocator the host application uses.
//!
//! Cost model:
//! - [`SessionStats`] is always on: one relaxed `fetch_add` per wire frame,
//!   on a path that already does AEAD crypto plus a transport write.
//! - [`HeapSize`] / memory reports only run when called; unused report code
//!   is dropped by fat LTO.
//! - [`TaskInstrument`] is resolved once at client build: unset leaves the
//!   runtime untouched. Only an installed instrument pays the per-poll hook.

use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use std::sync::Arc;
use std::time::Duration;

use portable_atomic::{AtomicU64, Ordering};

use crate::sync_marker::MaybeSendSync;

// ── Wire/session counters ────────────────────────────────────────────────────

/// Cumulative per-session counters, updated at the client's wire chokepoints.
///
/// All counters are monotonic over the lifetime of the owning client (they
/// survive reconnects); only the activity timestamps are reset on connection
/// teardown. Reads are relaxed: values are statistics, not synchronization.
#[derive(Debug, Default)]
pub struct SessionStats {
    bytes_sent: AtomicU64,
    bytes_received: AtomicU64,
    frames_sent: AtomicU64,
    frames_received: AtomicU64,
    messages_sent: AtomicU64,
    messages_received: AtomicU64,
    reconnects: AtomicU64,
    /// Timestamp (ms since UNIX epoch) of the last sent WebSocket frame.
    /// WA Web: `callStanza` → `deadSocketTimer.onOrBefore(deadSocketTime)`.
    last_data_sent_ms: AtomicU64,
    /// Timestamp (ms since UNIX epoch) of the last received WebSocket data.
    /// WA Web: `parseAndHandleStanza` → `deadSocketTimer.cancel()`.
    last_data_received_ms: AtomicU64,
}

/// Point-in-time copy of [`SessionStats`], plus client-level counters the
/// client fills in ([`Self::reconnect_errors`], [`Self::resends_throttled`]).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default)]
pub struct StatsSnapshot {
    /// Post-noise wire bytes written to the transport (includes frame headers
    /// and AEAD tags; excludes the handshake and TLS/WebSocket overhead).
    pub bytes_sent: u64,
    /// Wire bytes received from the transport (same framing semantics).
    pub bytes_received: u64,
    pub frames_sent: u64,
    pub frames_received: u64,
    /// Outgoing message send attempts (DM/group/status).
    pub messages_sent: u64,
    /// Incoming messages successfully decrypted and dispatched.
    pub messages_received: u64,
    /// Reconnect attempts started by the auto-reconnect loop.
    pub reconnects: u64,
    /// Consecutive reconnect failures (resets on success).
    pub reconnect_errors: u32,
    /// Outbound resends dropped by the per-chat rate limiter. Surfaces storm
    /// chats.
    pub resends_throttled: u64,
    pub last_data_sent_ms: u64,
    pub last_data_received_ms: u64,
}

impl SessionStats {
    pub fn new() -> Self {
        Self::default()
    }

    fn now_ms() -> u64 {
        crate::time::now_millis().max(0) as u64
    }

    /// One encrypted frame written to the transport.
    #[inline]
    pub fn record_frame_sent(&self, wire_bytes: usize) {
        self.bytes_sent
            .fetch_add(wire_bytes as u64, Ordering::Relaxed);
        self.frames_sent.fetch_add(1, Ordering::Relaxed);
        self.last_data_sent_ms
            .store(Self::now_ms(), Ordering::Relaxed);
    }

    /// One transport data event carrying `frames` decodable frames.
    ///
    /// Refreshes the receive timestamp only for multi-frame batches: the
    /// arrival stamp ([`Self::mark_recv_activity`]) is still fresh in the
    /// single-frame steady state, and the completion re-stamp exists to keep
    /// the dead-socket watchdog quiet while a long batch (offline sync)
    /// drains — not to pay a second clock read per frame.
    #[inline]
    pub fn record_recv_batch(&self, wire_bytes: usize, frames: u32) {
        self.bytes_received
            .fetch_add(wire_bytes as u64, Ordering::Relaxed);
        self.frames_received
            .fetch_add(frames as u64, Ordering::Relaxed);
        if frames > 1 {
            self.last_data_received_ms
                .store(Self::now_ms(), Ordering::Relaxed);
        }
    }

    /// Stamp receive activity at data arrival, without counting traffic
    /// (WA Web: deadSocketTimer reset). Batch completion is re-stamped by
    /// [`Self::record_recv_batch`].
    #[inline]
    pub fn mark_recv_activity(&self) {
        self.last_data_received_ms
            .store(Self::now_ms(), Ordering::Relaxed);
    }

    #[inline]
    pub fn record_message_sent(&self) {
        self.messages_sent.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn record_message_received(&self) {
        self.messages_received.fetch_add(1, Ordering::Relaxed);
    }

    #[inline]
    pub fn record_reconnect(&self) {
        self.reconnects.fetch_add(1, Ordering::Relaxed);
    }

    /// Zero the activity timestamps on connection teardown so the dead-socket
    /// watchdog never reads a previous connection's values. Traffic counters
    /// are cumulative and survive.
    pub fn reset_connection_activity(&self) {
        self.last_data_sent_ms.store(0, Ordering::Relaxed);
        self.last_data_received_ms.store(0, Ordering::Relaxed);
    }

    #[inline]
    pub fn last_data_sent_ms(&self) -> u64 {
        self.last_data_sent_ms.load(Ordering::Relaxed)
    }

    #[inline]
    pub fn last_data_received_ms(&self) -> u64 {
        self.last_data_received_ms.load(Ordering::Relaxed)
    }

    /// Copy the session-level counters. Client-level fields
    /// (`reconnect_errors`, `resends_throttled`) are left zero for the owner
    /// to fill.
    pub fn snapshot(&self) -> StatsSnapshot {
        StatsSnapshot {
            bytes_sent: self.bytes_sent.load(Ordering::Relaxed),
            bytes_received: self.bytes_received.load(Ordering::Relaxed),
            frames_sent: self.frames_sent.load(Ordering::Relaxed),
            frames_received: self.frames_received.load(Ordering::Relaxed),
            messages_sent: self.messages_sent.load(Ordering::Relaxed),
            messages_received: self.messages_received.load(Ordering::Relaxed),
            reconnects: self.reconnects.load(Ordering::Relaxed),
            reconnect_errors: 0,
            resends_throttled: 0,
            last_data_sent_ms: self.last_data_sent_ms.load(Ordering::Relaxed),
            last_data_received_ms: self.last_data_received_ms.load(Ordering::Relaxed),
        }
    }
}

// ── Retained-memory estimation ───────────────────────────────────────────────

/// Estimated heap bytes owned by a value, excluding `size_of::<Self>()` and
/// allocator overhead.
///
/// Implementations are honest approximations (protobuf-encoded size for
/// Signal records, string/collection payload sums elsewhere): good for
/// per-session attribution and growth tracking, not for byte-exact accounting.
pub trait HeapSize {
    fn heap_bytes(&self) -> usize;
}

impl<T: HeapSize> HeapSize for Arc<T> {
    /// Counted where the owning collection holds it; sharing is intra-client
    /// in practice, so attributing the full size to each holder's client is
    /// the useful semantics.
    fn heap_bytes(&self) -> usize {
        core::mem::size_of::<T>() + T::heap_bytes(self)
    }
}

impl HeapSize for Vec<u8> {
    fn heap_bytes(&self) -> usize {
        self.capacity()
    }
}

impl HeapSize for String {
    fn heap_bytes(&self) -> usize {
        self.capacity()
    }
}

impl HeapSize for str {
    fn heap_bytes(&self) -> usize {
        self.len()
    }
}

impl HeapSize for wacore_binary::CompactString {
    fn heap_bytes(&self) -> usize {
        if self.is_heap_allocated() {
            self.len()
        } else {
            0
        }
    }
}

impl HeapSize for wacore_binary::Jid {
    fn heap_bytes(&self) -> usize {
        self.user.heap_bytes()
    }
}

/// Entry count plus estimated retained bytes for one internal collection.
#[derive(Debug, Clone, Copy, Default)]
pub struct CollectionStats {
    pub entries: u64,
    /// Estimated retained heap bytes. `0` for store-backed caches whose
    /// entries live outside this process.
    pub bytes: u64,
}

impl CollectionStats {
    pub fn new(entries: u64, bytes: u64) -> Self {
        Self { entries, bytes }
    }
}

// ── Task instrumentation ─────────────────────────────────────────────────────

/// Runtime-agnostic hook called around every poll of the client's internal
/// tasks (and around its blocking work).
///
/// The library never installs one by itself; the application opts in at build
/// time. Implementations plug in whatever the platform offers: the built-in
/// [`CpuMeter`], an allocator-attribution guard on native, `heap_caps`
/// sampling on ESP32, etc. Calls are balanced: every `on_poll_start` is
/// followed by `on_poll_end` on the same thread.
pub trait TaskInstrument: MaybeSendSync {
    fn on_poll_start(&self);
    fn on_poll_end(&self);
}

/// Future wrapper invoking a [`TaskInstrument`] around each poll.
pub struct MeteredFuture<F> {
    inner: F,
    instrument: Arc<dyn TaskInstrument>,
}

impl<F> MeteredFuture<F> {
    pub fn new(inner: F, instrument: Arc<dyn TaskInstrument>) -> Self {
        Self { inner, instrument }
    }
}

/// Calls `on_poll_end` on drop, so a panicking poll (or blocking closure)
/// still closes the instrument scope — implementors that scope allocator
/// attribution would otherwise leak it across the unwind.
struct PollGuard<'a>(&'a dyn TaskInstrument);
impl Drop for PollGuard<'_> {
    fn drop(&mut self) {
        self.0.on_poll_end();
    }
}

impl<F: Future + Unpin> Future for MeteredFuture<F> {
    type Output = F::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.get_mut();
        this.instrument.on_poll_start();
        let _guard = PollGuard(&*this.instrument);
        Pin::new(&mut this.inner).poll(cx)
    }
}

/// Built-in [`TaskInstrument`]: accumulates poll count and busy time (a
/// direct CPU proxy) via the pluggable monotonic clock.
///
/// On wasm32/embedded this works as soon as the application registers a
/// monotonic provider (see [`crate::time`]).
#[derive(Debug, Default)]
pub struct CpuMeter {
    busy_nanos: AtomicU64,
    polls: AtomicU64,
}

/// Point-in-time copy of a [`CpuMeter`].
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default)]
pub struct CpuSnapshot {
    /// Total time spent inside `poll` (and blocking closures) of the
    /// instrumented tasks.
    pub busy: Duration,
    pub polls: u64,
}

impl CpuMeter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn snapshot(&self) -> CpuSnapshot {
        CpuSnapshot {
            busy: Duration::from_nanos(self.busy_nanos.load(Ordering::Relaxed)),
            polls: self.polls.load(Ordering::Relaxed),
        }
    }
}

std::thread_local! {
    /// Start times of the metered polls active on this thread, innermost
    /// last. A stack, not a single slot: metered scopes can nest (an executor
    /// may poll a freshly spawned task inline from within an already-metered
    /// poll, and several meters can share one thread), and each scope must
    /// keep its own start. Poll scopes strictly nest, so LIFO holds; a nested
    /// scope's time is also part of its enclosing scope's elapsed.
    static POLL_START: core::cell::RefCell<Vec<crate::time::Instant>> =
        const { core::cell::RefCell::new(Vec::new()) };
}

impl TaskInstrument for CpuMeter {
    fn on_poll_start(&self) {
        POLL_START.with(|s| s.borrow_mut().push(crate::time::Instant::now()));
    }

    fn on_poll_end(&self) {
        if let Some(start) = POLL_START.with(|s| s.borrow_mut().pop()) {
            self.busy_nanos
                .fetch_add(start.elapsed().as_nanos() as u64, Ordering::Relaxed);
            self.polls.fetch_add(1, Ordering::Relaxed);
        }
    }
}

// ── Runtime decorator ────────────────────────────────────────────────────────

use crate::runtime::{AbortHandle, Runtime};

/// [`Runtime`] decorator that instruments every spawned future (and blocking
/// closure) with a [`TaskInstrument`]. Wraps any runtime — Tokio, wasm,
/// embedded — since it only intercepts the trait surface.
pub struct InstrumentedRuntime {
    inner: Arc<dyn Runtime>,
    instrument: Arc<dyn TaskInstrument>,
}

impl InstrumentedRuntime {
    pub fn new(inner: Arc<dyn Runtime>, instrument: Arc<dyn TaskInstrument>) -> Self {
        Self { inner, instrument }
    }
}

// The Runtime trait requires Send + Sync even on wasm32 (where concrete
// runtimes use the same escape hatch); single-threaded, so this is sound.
#[cfg(target_arch = "wasm32")]
unsafe impl Send for InstrumentedRuntime {}
#[cfg(target_arch = "wasm32")]
unsafe impl Sync for InstrumentedRuntime {}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait::async_trait]
impl Runtime for InstrumentedRuntime {
    fn spawn(&self, future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>) -> AbortHandle {
        self.inner.spawn(Box::pin(MeteredFuture::new(
            future,
            self.instrument.clone(),
        )))
    }

    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        self.inner.sleep(duration)
    }

    fn spawn_blocking(
        &self,
        f: Box<dyn FnOnce() + Send + 'static>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        let instrument = self.instrument.clone();
        self.inner.spawn_blocking(Box::new(move || {
            instrument.on_poll_start();
            let _guard = PollGuard(&*instrument);
            f();
        }))
    }

    fn yield_now(&self) -> Option<Pin<Box<dyn Future<Output = ()> + Send>>> {
        self.inner.yield_now()
    }

    fn yield_frequency(&self) -> u32 {
        self.inner.yield_frequency()
    }
}

#[cfg(target_arch = "wasm32")]
#[async_trait::async_trait(?Send)]
impl Runtime for InstrumentedRuntime {
    fn spawn(&self, future: Pin<Box<dyn Future<Output = ()> + 'static>>) -> AbortHandle {
        self.inner.spawn(Box::pin(MeteredFuture::new(
            future,
            self.instrument.clone(),
        )))
    }

    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()>>> {
        self.inner.sleep(duration)
    }

    fn spawn_blocking(&self, f: Box<dyn FnOnce() + 'static>) -> Pin<Box<dyn Future<Output = ()>>> {
        let instrument = self.instrument.clone();
        self.inner.spawn_blocking(Box::new(move || {
            instrument.on_poll_start();
            let _guard = PollGuard(&*instrument);
            f();
        }))
    }

    fn yield_now(&self) -> Option<Pin<Box<dyn Future<Output = ()>>>> {
        self.inner.yield_now()
    }

    fn yield_frequency(&self) -> u32 {
        self.inner.yield_frequency()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_reflects_recorded_traffic() {
        let stats = SessionStats::new();
        stats.record_frame_sent(100);
        stats.record_frame_sent(50);
        stats.record_recv_batch(300, 2);
        stats.record_message_sent();
        stats.record_message_received();
        stats.record_reconnect();

        let snap = stats.snapshot();
        assert_eq!(snap.bytes_sent, 150);
        assert_eq!(snap.frames_sent, 2);
        assert_eq!(snap.bytes_received, 300);
        assert_eq!(snap.frames_received, 2);
        assert_eq!(snap.messages_sent, 1);
        assert_eq!(snap.messages_received, 1);
        assert_eq!(snap.reconnects, 1);
        assert!(snap.last_data_sent_ms > 0);
        assert!(snap.last_data_received_ms > 0);
    }

    #[test]
    fn reset_connection_activity_keeps_traffic() {
        let stats = SessionStats::new();
        stats.record_frame_sent(10);
        stats.record_recv_batch(20, 1);
        stats.reset_connection_activity();

        let snap = stats.snapshot();
        assert_eq!(snap.last_data_sent_ms, 0);
        assert_eq!(snap.last_data_received_ms, 0);
        assert_eq!(snap.bytes_sent, 10);
        assert_eq!(snap.bytes_received, 20);
    }

    #[test]
    fn cpu_meter_counts_polls_and_busy_time() {
        let meter = Arc::new(CpuMeter::new());
        let instrument: Arc<dyn TaskInstrument> = meter.clone();

        let mut fut = MeteredFuture::new(Box::pin(async {}), instrument);
        let waker = std::task::Waker::noop();
        let mut cx = Context::from_waker(waker);
        assert!(Pin::new(&mut fut).poll(&mut cx).is_ready());

        let snap = meter.snapshot();
        assert_eq!(snap.polls, 1);
    }
}

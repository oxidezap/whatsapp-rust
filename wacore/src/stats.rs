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

// ── Out-of-client resource reports ───────────────────────────────────────────
//
// `MemoryReport` (in the client crate) accounts only for the client's own
// in-process collections. The dominant per-session RAM lives *outside* the
// client — the storage backend's page cache, the transport buffers, the HTTP
// pool. These small structs let each of those components report what it can
// introspect, so a consumer can compose a realistic per-session estimate.
//
// Every field is `Option`: a component fills only what it knows. All-`None`
// means "not reported" — distinct from a positive `Some(0)` ("holds none",
// e.g. a remote/store-backed backend whose data isn't process memory, matching
// `CollectionStats { bytes: 0 }`). The structs are plain (not `#[non_exhaustive]`)
// because they are built in the backend/transport/HTTP crates, which need
// struct-literal construction; add future fields with a `..Default::default()`
// tail to stay non-breaking.

/// Process-local resource footprint a storage backend attributes to one
/// session. Returned by `store::traits::DeviceStore::resource_report`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct StorageResourceReport {
    /// Estimated process-local bytes the backend holds for this session (e.g. a
    /// SQLite page cache). `Some(0)` for backends whose data lives outside this
    /// process (Redis, other network stores).
    pub memory_bytes: Option<u64>,
    /// Pages/entries currently backing the store, when known (SQLite: database
    /// page count). A size indicator, not part of the memory total.
    pub pages: Option<u64>,
    /// Bytes read from the backing store this session, if the backend counts it.
    pub io_read_bytes: Option<u64>,
    /// Bytes written to the backing store this session, if the backend counts it.
    pub io_write_bytes: Option<u64>,
}

impl StorageResourceReport {
    /// Retained process memory this backend reports (0 when unknown). Excludes
    /// the cumulative I/O counters, which are throughput, not residency.
    pub fn total_bytes(&self) -> u64 {
        self.memory_bytes.unwrap_or(0)
    }
}

/// Per-session footprint of a [`crate::net::Transport`]: read/write framing
/// buffers plus a best-effort TLS/noise session-state estimate.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TransportResourceReport {
    pub read_buffer_bytes: Option<u64>,
    pub write_buffer_bytes: Option<u64>,
    /// Best-effort estimate of TLS/noise session state (record buffers, key
    /// schedule). Transports that can't introspect their TLS stack leave it
    /// `None`.
    pub tls_state_bytes: Option<u64>,
}

impl TransportResourceReport {
    /// Sum of the present byte fields (saturating — `total_bytes` is public, so
    /// a caller-built report with large values must not wrap).
    pub fn total_bytes(&self) -> u64 {
        self.read_buffer_bytes
            .unwrap_or(0)
            .saturating_add(self.write_buffer_bytes.unwrap_or(0))
            .saturating_add(self.tls_state_bytes.unwrap_or(0))
    }
}

/// Per-session footprint of a [`crate::net::HttpClient`]: idle connection-pool
/// buffers plus any in-flight download/media buffering the impl can see.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct HttpResourceReport {
    /// Idle connections the pool may retain (a cap/estimate, not a live count
    /// when the client can't introspect the pool).
    pub pool_connections: Option<u64>,
    /// Bytes held by the pool's per-connection read/write buffers.
    pub pool_buffer_bytes: Option<u64>,
    /// Bytes buffered for in-flight requests/responses right now, when known.
    pub inflight_bytes: Option<u64>,
}

impl HttpResourceReport {
    /// Sum of the present byte fields (excludes the connection count). Saturating
    /// — `total_bytes` is public, so a caller-built report must not wrap.
    pub fn total_bytes(&self) -> u64 {
        self.pool_buffer_bytes
            .unwrap_or(0)
            .saturating_add(self.inflight_bytes.unwrap_or(0))
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

// ── Allocator attribution ────────────────────────────────────────────────────

std::thread_local! {
    /// Meters active on this thread, innermost last. `on_poll_start` pushes,
    /// `on_poll_end` pops; the host's global allocator charges the innermost.
    /// A stack (not a slot) for the same reason as `POLL_START`: metered poll
    /// scopes nest and several meters can share a thread.
    ///
    /// Holds an owned `Arc<AllocMeterInner>`, not a raw pointer: `on_poll_start`
    /// is a safe public method, so a caller could move or drop a stack-local
    /// meter before `on_poll_end` — the strong ref here keeps the counters alive
    /// for the whole scope, so `on_alloc` (driven by the allocator on every
    /// allocation) can never dereference freed memory.
    static ACTIVE_ALLOC_METER: core::cell::RefCell<Vec<Arc<AllocMeterInner>>> =
        const { core::cell::RefCell::new(Vec::new()) };
}

/// Shared counters behind an [`AllocMeter`]. Held by `Arc` so a scope on the
/// active-meter stack owns the lifetime independently of the `AllocMeter` handle.
#[derive(Debug, Default)]
struct AllocMeterInner {
    allocated: AtomicU64,
    freed: AtomicU64,
    allocations: AtomicU64,
}

/// Built-in [`TaskInstrument`] that attributes heap bytes **allocated and
/// freed** to one client, the churn/transient counterpart to the point-in-time
/// retained figures in `Client::memory_report`. It captures task futures,
/// decode arenas and media buffers — anything the client's instrumented tasks
/// allocate — that no named collection holds.
///
/// The library never sees the allocator. The host installs a `#[global_allocator]`
/// that calls [`AllocMeter::on_alloc`] / [`AllocMeter::on_dealloc`] on every
/// (de)allocation; this meter — installed via `with_task_instrument` (or the
/// `with_alloc_meter` convenience) — marks, per thread, *which* client's task is
/// being polled, so those calls charge the right meter. `examples/alloc_tracking.rs`
/// shows the ~20 lines of glue.
///
/// # Attribution boundary (honest limits)
/// Only allocations made *inside an instrumented poll or blocking closure* are
/// counted: every task spawned through the `Runtime` trait, plus the main run
/// loop (metered since the client meters its own future). Work spawned raw on
/// the executor — some voip/media paths — and the caller's own
/// `send_message`-side code are **not** counted. Deallocations are charged to
/// whichever meter is active when the free happens, not the one that allocated
/// the block, so `freed` (and `net`) drift when a buffer outlives the poll that
/// made it; the cumulative `allocated` total is the reliable signal.
///
/// Agnostic: the hook is the same wasm/ESP32-safe [`TaskInstrument`] surface as
/// [`CpuMeter`]. Expect a measurable overhead while a counting allocator is
/// installed (~10-20% for this design); it is a diagnostics tool, not an
/// always-on meter.
#[derive(Debug, Default, Clone)]
pub struct AllocMeter {
    inner: Arc<AllocMeterInner>,
}

/// Point-in-time copy of an [`AllocMeter`]. Counters are cumulative over the
/// meter's lifetime.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default)]
pub struct AllocSnapshot {
    /// Total bytes allocated while this meter was the active one.
    pub allocated_bytes: u64,
    /// Total bytes freed while this meter was active (see the drift caveat on
    /// [`AllocMeter`]).
    pub freed_bytes: u64,
    /// Number of allocations charged.
    pub allocations: u64,
}

impl AllocSnapshot {
    /// Net bytes still attributed (`allocated - freed`), saturating at 0. A
    /// lower bound on live churn: blocks freed under a different active meter
    /// aren't subtracted here.
    pub fn net_bytes(&self) -> u64 {
        self.allocated_bytes.saturating_sub(self.freed_bytes)
    }
}

impl AllocMeter {
    pub fn new() -> Self {
        Self::default()
    }

    /// Charge `bytes` of allocation to the meter currently active on this
    /// thread, if any. Call from a global allocator's `alloc`. Allocation-free
    /// (only a thread-local read + relaxed atomics), so it is safe to call from
    /// inside the allocator without recursing.
    #[inline]
    pub fn on_alloc(bytes: usize) {
        Self::with_active(|inner| {
            inner.allocated.fetch_add(bytes as u64, Ordering::Relaxed);
            inner.allocations.fetch_add(1, Ordering::Relaxed);
        });
    }

    /// Charge `bytes` of deallocation to the meter currently active on this
    /// thread, if any. Call from a global allocator's `dealloc`.
    #[inline]
    pub fn on_dealloc(bytes: usize) {
        Self::with_active(|inner| {
            inner.freed.fetch_add(bytes as u64, Ordering::Relaxed);
        });
    }

    #[inline]
    fn with_active(f: impl FnOnce(&AllocMeterInner)) {
        // `try_with` guards TLS-destroyed-on-exit; `try_borrow` guards the
        // reentrancy where `on_poll_start`'s own `push` reallocates the stack
        // and lands back here — in that window the borrow fails and we skip
        // (charging that tiny bookkeeping allocation to no one).
        let _ = ACTIVE_ALLOC_METER.try_with(|cell| {
            if let Ok(stack) = cell.try_borrow()
                && let Some(inner) = stack.last()
            {
                f(inner);
            }
        });
    }

    pub fn snapshot(&self) -> AllocSnapshot {
        AllocSnapshot {
            allocated_bytes: self.inner.allocated.load(Ordering::Relaxed),
            freed_bytes: self.inner.freed.load(Ordering::Relaxed),
            allocations: self.inner.allocations.load(Ordering::Relaxed),
        }
    }
}

impl TaskInstrument for AllocMeter {
    fn on_poll_start(&self) {
        // `borrow_mut` while pushing: a reentrant `on_alloc` from the push's own
        // reallocation sees the active borrow and skips (see `with_active`).
        let _ = ACTIVE_ALLOC_METER.try_with(|cell| cell.borrow_mut().push(self.inner.clone()));
    }

    fn on_poll_end(&self) {
        // Pop under the borrow, then drop the popped Arc AFTER the borrow is
        // released: if it was the last strong ref, its deallocation reenters the
        // allocator (→ `on_dealloc`), which must not find the stack still borrowed.
        let popped = ACTIVE_ALLOC_METER
            .try_with(|cell| cell.borrow_mut().pop())
            .ok()
            .flatten();
        drop(popped);
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

    #[test]
    fn alloc_meter_charges_only_the_active_scope() {
        let meter = AllocMeter::new();

        // Outside any poll scope: charged to no one.
        AllocMeter::on_alloc(9999);

        meter.on_poll_start();
        AllocMeter::on_alloc(1000);
        AllocMeter::on_alloc(500);
        AllocMeter::on_dealloc(200);
        meter.on_poll_end();

        // After the scope closes: charged to no one again.
        AllocMeter::on_alloc(7777);
        AllocMeter::on_dealloc(7777);

        let snap = meter.snapshot();
        assert_eq!(snap.allocated_bytes, 1500);
        assert_eq!(snap.freed_bytes, 200);
        assert_eq!(snap.allocations, 2);
        assert_eq!(snap.net_bytes(), 1300);
    }

    #[test]
    fn alloc_meter_attributes_nested_scopes_to_the_innermost() {
        let outer = AllocMeter::new();
        let inner = AllocMeter::new();

        outer.on_poll_start();
        AllocMeter::on_alloc(100); // -> outer
        inner.on_poll_start();
        AllocMeter::on_alloc(30); // -> inner (innermost)
        inner.on_poll_end();
        AllocMeter::on_alloc(70); // -> outer again
        outer.on_poll_end();

        assert_eq!(outer.snapshot().allocated_bytes, 170);
        assert_eq!(inner.snapshot().allocated_bytes, 30);
    }

    #[test]
    fn alloc_meter_survives_realloc_reentrancy_during_poll_start() {
        // Force the thread-local stack to grow inside `on_poll_start` while its
        // own `borrow_mut` is held: a reentrant `on_alloc` must skip, not panic.
        let meters: Vec<AllocMeter> = (0..64).map(|_| AllocMeter::new()).collect();
        for m in &meters {
            m.on_poll_start();
            AllocMeter::on_alloc(1);
        }
        for m in meters.iter().rev() {
            m.on_poll_end();
        }
        // Innermost meter got its own charge; no panic reaching here is the test.
        assert_eq!(meters.last().unwrap().snapshot().allocations, 1);
    }

    #[test]
    fn alloc_meter_scope_outlives_a_dropped_handle() {
        // Regression for the raw-pointer soundness hole: the active-meter scope
        // owns an `Arc` to the counters, so a charge after the caller's handle is
        // dropped touches valid memory (with the old `*const AllocMeter` this was
        // a dangling-pointer deref).
        let keep = AllocMeter::new();
        let temp = keep.clone(); // shares the same counters
        temp.on_poll_start();
        drop(temp); // handle gone; the scope's Arc keeps the counters alive
        AllocMeter::on_alloc(128);
        keep.on_poll_end(); // LIFO pop; any handle pops the top scope
        assert_eq!(keep.snapshot().allocated_bytes, 128);
    }

    #[test]
    fn resource_report_total_bytes_saturate() {
        let t = TransportResourceReport {
            read_buffer_bytes: Some(u64::MAX),
            write_buffer_bytes: Some(10),
            tls_state_bytes: Some(10),
        };
        assert_eq!(t.total_bytes(), u64::MAX, "transport total must not wrap");

        let h = HttpResourceReport {
            pool_connections: Some(3),
            pool_buffer_bytes: Some(u64::MAX),
            inflight_bytes: Some(1),
        };
        assert_eq!(h.total_bytes(), u64::MAX, "http total must not wrap");
    }
}

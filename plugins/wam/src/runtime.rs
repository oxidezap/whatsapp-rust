//! Buffering, sampling, rotation, flush and upload.
//!
//! The shape follows the official client's, because its thresholds only make
//! sense together: events accumulate in memory for a few seconds, are written
//! into a buffer, and the buffer is uploaded when it is big enough, old enough,
//! or when nothing has been uploaded yet on this connection.
//!
//! What it must never do is affect the client. Telemetry that cannot be
//! serialized, stored or uploaded is telemetry that is dropped and counted;
//! no function here returns an error to the message path, because none of them
//! is called from it.
//!
//! # Ownership
//!
//! [`WamRuntime`] holds only the queue and the counters, both behind short
//! locks taken from the core-event handler. The buffers live in [`WamWriter`],
//! which one task owns, so no buffer is ever touched from two places and no lock
//! is ever held across an await.

use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use log::{debug, warn};
use portable_atomic::{AtomicU64, Ordering};
use whatsapp_rust::async_trait;
use whatsapp_rust_wam_catalog::{Channel, WamBuffer, WamEvent, constants, events, sampling};

use crate::identity::WamIdentity;
use crate::store::{PendingBuffer, WamStore};

/// How long events accumulate in memory before being written into a buffer.
pub const BUFFERING_INTERVAL: Duration =
    Duration::from_secs(constants::WAM_IN_MEMORY_BUFFERING_DURATION_IN_SECS as u64);

/// How long a buffer may live before it is uploaded even though it is small.
pub const ROTATE_INTERVAL_SECS: i64 = constants::WAM_BUFFER_ROTATE_INTERVAL_IN_SECS;

/// The size past which a buffer is finished rather than added to.
pub const MAX_BUFFER_SIZE: usize = constants::WAM_MAX_BUFFER_SIZE as usize;

/// The size past which a buffer is dropped instead of uploaded.
///
/// Larger than [`MAX_BUFFER_SIZE`] and not a duplicate of it. The size check
/// runs between events, so one event can carry a buffer past the first
/// threshold; this is the ceiling on what the upload stanza may be. A buffer
/// between the two is uploaded, a buffer past this one never would have been
/// accepted, and the same pair also bounds what is retained across a failure.
pub const MAX_UPLOAD_SIZE: usize = constants::WAM_MAX_BUFFER_SIZE_FOR_UPLOAD as usize;

/// Backoff bounds between upload attempts, in seconds.
///
/// The official client retries a failed upload inline, waiting between the two
/// attempts. This one does not: the wait would sit on the task that also drains
/// the queue, so a slow server would stop events being written as well as sent.
/// Instead a failure starts a cooldown and the retry rides the next tick, which
/// gives the same exponential spacing without occupying anything.
const BACKOFF_BASE_SECS: i64 = 1;
const BACKOFF_CAP_SECS: i64 = 120;

/// The stream id every buffer this client writes is stamped with.
///
/// One, as the official web runtime hardcodes it. The field distinguishes
/// concurrent producers inside one client (a worker from a tab); this client has
/// one producer, so it has one stream.
const STREAM_ID: u8 = 1;

/// An event waiting to be written into a buffer.
///
/// A closed enum rather than a boxed trait object, deliberately. An event
/// belongs here only once every field the plugin writes has been checked against
/// WA Web's own call sites, and `Box<dyn WamEvent>` would make adding one a
/// local decision instead of a reviewable one.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum PendingEvent {
    E2eMessageRecv(events::E2eMessageRecv),
    MessageReceive(events::MessageReceive),
    ReceiptStanzaReceive(events::ReceiptStanzaReceive),
    WamClientErrors(events::WamClientErrors),
    WamDroppedEvent(events::WamDroppedEvent),
}

impl PendingEvent {
    /// The event's wire code.
    pub fn code(&self) -> u32 {
        match self {
            Self::E2eMessageRecv(_) => events::E2eMessageRecv::CODE,
            Self::MessageReceive(_) => events::MessageReceive::CODE,
            Self::ReceiptStanzaReceive(_) => events::ReceiptStanzaReceive::CODE,
            Self::WamClientErrors(_) => events::WamClientErrors::CODE,
            Self::WamDroppedEvent(_) => events::WamDroppedEvent::CODE,
        }
    }

    /// The declared weight a production client samples this event at.
    pub fn weight(&self) -> u32 {
        match self {
            Self::E2eMessageRecv(_) => events::E2eMessageRecv::WEIGHTS[sampling::RELEASE],
            Self::MessageReceive(_) => events::MessageReceive::WEIGHTS[sampling::RELEASE],
            Self::ReceiptStanzaReceive(_) => {
                events::ReceiptStanzaReceive::WEIGHTS[sampling::RELEASE]
            }
            Self::WamClientErrors(_) => events::WamClientErrors::WEIGHTS[sampling::RELEASE],
            Self::WamDroppedEvent(_) => events::WamDroppedEvent::WEIGHTS[sampling::RELEASE],
        }
    }

    fn write_into(&self, buffer: &mut WamBuffer, now_secs: i64, weight: u32) -> bool {
        let written = match self {
            Self::E2eMessageRecv(e) => buffer.write_event(e, now_secs, weight),
            Self::MessageReceive(e) => buffer.write_event(e, now_secs, weight),
            Self::ReceiptStanzaReceive(e) => buffer.write_event(e, now_secs, weight),
            Self::WamClientErrors(e) => buffer.write_event(e, now_secs, weight),
            Self::WamDroppedEvent(e) => buffer.write_event(e, now_secs, weight),
        };
        match written {
            Ok(()) => true,
            Err(err) => {
                // Unreachable while every variant is a regular-channel event,
                // which the catalog's own constants decide. Logged rather than
                // asserted: a telemetry plugin must not be able to panic a
                // client, not even on its own bug.
                warn!("wam: refusing to write an event into a buffer: {err}");
                false
            }
        }
    }
}

/// Why an upload failed, reduced to the one distinction the retry policy needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UploadFailure {
    /// The server or the transport may succeed on a retry: a timeout, a lost
    /// connection, a 5xx.
    Retryable(String),
    /// The server refused this buffer and will refuse it again.
    Permanent(String),
}

impl std::fmt::Display for UploadFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Retryable(m) | Self::Permanent(m) => f.write_str(m),
        }
    }
}

/// Where a finished buffer goes.
///
/// A trait rather than a direct call on the IQ capability so the runtime is
/// testable without a client, and so the retry policy is expressed against one
/// small surface instead of against every error the request path can produce.
#[async_trait]
pub trait WamUploader: Send + Sync {
    /// Send one buffer, stamped `t` (unix seconds).
    async fn upload(&self, t: i64, buffer: &[u8]) -> Result<(), UploadFailure>;
}

/// Counters a consumer can read back.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
#[non_exhaustive]
pub struct WamStats {
    /// Events handed to the runtime.
    pub observed: u64,
    /// Events written into a buffer.
    pub written: u64,
    /// Events discarded by sampling. Not a failure: most events are meant to be
    /// sampled out, and a rate of zero here on a busy client is the bug.
    pub sampled_out: u64,
    /// Events dropped because the queue was full.
    pub dropped: u64,
    /// Buffers the server accepted.
    pub uploaded: u64,
    /// Buffers abandoned: too large to upload, or over the retention cap.
    pub discarded: u64,
    /// Upload attempts that failed, retried ones included.
    pub upload_failures: u64,
    /// Whether the store outlives the process.
    pub store_is_durable: bool,
}

/// The queue and the counters: everything a core-event handler touches.
#[derive(Debug, Default)]
struct Shared {
    queue: VecDeque<PendingEvent>,
    stats: WamStats,
}

/// The plugin's telemetry runtime.
pub struct WamRuntime {
    identity: WamIdentity,
    store: Arc<dyn WamStore>,
    max_queued_events: usize,
    shared: Mutex<Shared>,
    /// Distinguishes retained buffers. Monotonic within a run and never a wire
    /// value, so a restart repeating it is harmless.
    next_key: AtomicU64,
}

impl std::fmt::Debug for WamRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WamRuntime")
            .field("identity", &self.identity)
            .field("max_queued_events", &self.max_queued_events)
            .finish_non_exhaustive()
    }
}

/// The buffers, owned by whichever task drives [`WamRuntime::tick`].
#[derive(Debug, Default)]
pub struct WamWriter {
    /// The buffer being filled.
    current: Option<WamBuffer>,
    /// Unix seconds of the last accepted upload, or `None` while nothing has
    /// been uploaded on this run.
    ///
    /// `None` is itself a flush condition, as it is in the official client: the
    /// first buffer goes out as soon as there is one, so a short-lived process
    /// reports something rather than nothing.
    last_upload_secs: Option<i64>,
    /// Failed uploads since the last accepted one, which sets how long the
    /// cooldown is.
    consecutive_failures: u32,
    /// Unix second before which no upload is attempted.
    retry_after_secs: Option<i64>,
}

impl WamRuntime {
    pub fn new(identity: WamIdentity, store: Arc<dyn WamStore>, max_queued_events: usize) -> Self {
        let durable = store.is_durable();
        Self {
            identity,
            store,
            max_queued_events: max_queued_events.max(1),
            shared: Mutex::new(Shared {
                queue: VecDeque::new(),
                stats: WamStats {
                    store_is_durable: durable,
                    ..WamStats::default()
                },
            }),
            next_key: AtomicU64::new(1),
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Shared> {
        self.shared
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    pub fn stats(&self) -> WamStats {
        self.lock().stats.clone()
    }

    /// The events still waiting to be written. Tests only: a consumer has the
    /// counters, and the queue is an implementation detail that drains itself.
    #[cfg(test)]
    fn queued(&self) -> Vec<PendingEvent> {
        self.lock().queue.iter().cloned().collect()
    }

    /// Hand one event to the runtime.
    ///
    /// Called inline from a core-event handler, so it does no work beyond a push
    /// under a short lock: no encoding, no sampling, no I/O. Everything else
    /// happens on the flush task.
    ///
    /// A full queue drops the *newest* event, not the oldest: the oldest is the
    /// one closest to being written, and dropping it would waste the work already
    /// done on it. The drop is reported as a `WamDroppedEvent`, the event the
    /// official client emits for the same reason, and the report takes the
    /// newest slot rather than a new one, because a saturated queue must not be able to
    /// grow by reporting that it is saturated. Consecutive drops of the same
    /// event coalesce into that report's count, which is what the field is for.
    pub fn observe(&self, event: PendingEvent) {
        let mut shared = self.lock();
        shared.stats.observed += 1;
        if shared.queue.len() >= self.max_queued_events {
            shared.stats.dropped += 1;
            let code = i64::from(event.code());
            match shared.queue.back_mut() {
                Some(PendingEvent::WamDroppedEvent(report))
                    if report.dropped_event_code == Some(code) =>
                {
                    report.dropped_event_count =
                        Some(report.dropped_event_count.unwrap_or(0).saturating_add(1));
                }
                _ => {
                    shared.queue.pop_back();
                    shared.queue.push_back(PendingEvent::WamDroppedEvent(
                        events::WamDroppedEvent {
                            dropped_event_code: Some(code),
                            dropped_event_count: Some(1),
                            ..Default::default()
                        },
                    ));
                }
            }
            return;
        }
        shared.queue.push_back(event);
    }

    /// Report a buffer this runtime had to abandon, the way the official client
    /// does.
    fn report_buffer_drop(&self) {
        self.observe(PendingEvent::WamClientErrors(events::WamClientErrors {
            wam_client_buffer_drop_error_count: Some(1),
            ..Default::default()
        }));
    }

    /// Write the queued events into a buffer and upload it when it is due.
    ///
    /// `now_secs` and `roll` are parameters rather than reads of a global clock
    /// and RNG so a test can drive a flush deadline without sleeping and a
    /// sampling decision without repeating.
    pub async fn tick(
        &self,
        writer: &mut WamWriter,
        uploader: &dyn WamUploader,
        now_secs: i64,
        roll: &mut (dyn FnMut() -> f64 + Send),
        force: bool,
    ) {
        // Before anything else, so a buffer retained by this tick waits for the
        // next one and older buffers keep their place in line.
        self.drain_pending(writer, uploader, now_secs).await;

        let queued: Vec<PendingEvent> = {
            let mut shared = self.lock();
            shared.queue.drain(..).collect()
        };

        for event in queued {
            let weight = event.weight();
            if !sampling::keeps(weight, roll()) {
                self.lock().stats.sampled_out += 1;
                continue;
            }
            // The size check runs before the write, not after: the threshold
            // bounds what is uploaded, and appending first would knowingly push
            // a buffer past it.
            if writer
                .current
                .as_ref()
                .is_some_and(|b| b.len() > MAX_BUFFER_SIZE)
            {
                self.upload_current(writer, uploader, now_secs).await;
            }
            if writer.current.is_none() {
                match self.start_buffer().await {
                    Some(buffer) => writer.current = Some(buffer),
                    None => {
                        self.lock().stats.dropped += 1;
                        continue;
                    }
                }
            }
            let Some(buffer) = writer.current.as_mut() else {
                continue;
            };
            if event.write_into(buffer, now_secs, weight) {
                self.lock().stats.written += 1;
            }
        }

        let due = writer.current.as_ref().is_some_and(|buffer| {
            buffer.has_events()
                && (force
                    || buffer.len() > MAX_BUFFER_SIZE
                    || writer
                        .last_upload_secs
                        .is_none_or(|last| now_secs >= last.saturating_add(ROTATE_INTERVAL_SECS)))
        });
        if due {
            self.upload_current(writer, uploader, now_secs).await;
        }
    }

    /// A fresh buffer, carrying the identity's globals.
    async fn start_buffer(&self) -> Option<WamBuffer> {
        let sequence = match self.store.next_sequence(Channel::Regular).await {
            Ok(sequence) => sequence,
            Err(err) => {
                warn!("wam: no sequence number, dropping the event: {err}");
                return None;
            }
        };
        let mut buffer = WamBuffer::new(Channel::Regular, STREAM_ID, sequence);
        for global in self.identity.resolved_for(Channel::Regular) {
            if let Err(err) = buffer.set_global(global.def, global.value.as_ref()) {
                // The identity already filters by channel, so the only way here
                // is a catalog change that moved a global off the regular
                // channel. Refusing is right: the alternative is uploading a
                // buffer no official client would send.
                warn!("wam: cannot start a buffer: {err}");
                return None;
            }
        }
        Some(buffer)
    }

    /// Take the current buffer and try to deliver it.
    async fn upload_current(
        &self,
        writer: &mut WamWriter,
        uploader: &dyn WamUploader,
        now_secs: i64,
    ) {
        let Some(buffer) = writer.current.take() else {
            return;
        };
        if !buffer.has_events() {
            return;
        }
        let bytes = buffer.into_bytes();
        // Inside a cooldown the buffer is retained without an attempt: the
        // cooldown exists so a failing server is asked at the backoff's pace,
        // and a buffer finished mid-drain must not get around it.
        if writer.retry_after_secs.is_some_and(|at| now_secs < at) {
            self.retain(bytes).await;
            return;
        }
        match self.deliver(writer, uploader, now_secs, &bytes).await {
            Delivery::Accepted => writer.last_upload_secs = Some(now_secs),
            Delivery::Failed => self.retain(bytes).await,
            Delivery::TooLarge => {}
        }
    }

    /// Send one buffer once.
    ///
    /// A buffer past the upload ceiling is not sent at all: the stanza would be
    /// refused, and the official client counts exactly this as a client error.
    /// A failure starts or extends the cooldown, so the next attempt is spaced
    /// by the backoff curve rather than by the tick interval.
    async fn deliver(
        &self,
        writer: &mut WamWriter,
        uploader: &dyn WamUploader,
        now_secs: i64,
        bytes: &[u8],
    ) -> Delivery {
        if bytes.len() > MAX_UPLOAD_SIZE {
            warn!(
                "wam: dropping a {} byte buffer, past the {MAX_UPLOAD_SIZE} byte upload ceiling",
                bytes.len()
            );
            self.lock().stats.discarded += 1;
            self.report_buffer_drop();
            return Delivery::TooLarge;
        }
        match uploader.upload(now_secs, bytes).await {
            Ok(()) => {
                self.lock().stats.uploaded += 1;
                writer.consecutive_failures = 0;
                writer.retry_after_secs = None;
                Delivery::Accepted
            }
            Err(failure) => {
                self.lock().stats.upload_failures += 1;
                let wait = backoff(writer.consecutive_failures);
                writer.consecutive_failures = writer.consecutive_failures.saturating_add(1);
                writer.retry_after_secs = Some(now_secs.saturating_add(wait));
                debug!("wam: upload failed, waiting {wait}s: {failure}");
                Delivery::Failed
            }
        }
    }

    /// Keep a buffer for a later attempt, if there is room for it.
    ///
    /// The cap is [`MAX_BUFFER_SIZE`] across everything retained, the same bound
    /// the official client puts on what it writes back after a failed send. Past
    /// it the retained set is abandoned whole rather than trimmed: a partial set
    /// is a set whose sequence numbers have holes, which is worse for the server
    /// than none at all.
    async fn retain(&self, bytes: Vec<u8>) {
        let pending = self.store.pending().await.unwrap_or_default();
        let retained: usize = pending.iter().map(|b| b.bytes.len()).sum();
        if retained.saturating_add(bytes.len()) > MAX_BUFFER_SIZE {
            for buffer in pending {
                let _ = self.store.remove_pending(buffer.key).await;
            }
            self.lock().stats.discarded += 1;
            self.report_buffer_drop();
            return;
        }
        let key = self.next_key.fetch_add(1, Ordering::Relaxed);
        if let Err(err) = self
            .store
            .put_pending(PendingBuffer {
                key,
                channel: Channel::Regular,
                bytes,
            })
            .await
        {
            warn!("wam: could not retain a buffer: {err}");
            self.lock().stats.discarded += 1;
        }
    }

    /// Try the buffers a previous run or a previous failure left behind.
    async fn drain_pending(
        &self,
        writer: &mut WamWriter,
        uploader: &dyn WamUploader,
        now_secs: i64,
    ) {
        if writer.retry_after_secs.is_some_and(|at| now_secs < at) {
            return;
        }
        let pending = match self.store.pending().await {
            Ok(pending) => pending,
            Err(err) => {
                warn!("wam: could not read the retained buffers: {err}");
                return;
            }
        };
        for buffer in pending {
            // Removed first either way. A buffer that failed again is offered
            // back to `retain`, which is where the retention cap lives; leaving
            // it in place instead would let the cap be exceeded by a buffer that
            // is already past it.
            let _ = self.store.remove_pending(buffer.key).await;
            match self
                .deliver(writer, uploader, now_secs, &buffer.bytes)
                .await
            {
                Delivery::Accepted => {
                    writer.last_upload_secs = Some(now_secs);
                }
                Delivery::Failed => {
                    self.retain(buffer.bytes).await;
                    // One failing buffer means the server or the link is
                    // unhappy; marching through the rest would just fail as
                    // many times.
                    return;
                }
                Delivery::TooLarge => {}
            }
        }
    }
}

/// What became of one upload attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Delivery {
    Accepted,
    /// Not accepted, and worth keeping.
    Failed,
    /// Not sent at all, and not worth keeping.
    TooLarge,
}

/// How long to wait before the next upload attempt, in seconds, after
/// `failures` consecutive failures.
pub fn backoff(failures: u32) -> i64 {
    BACKOFF_BASE_SECS
        .saturating_mul(1i64 << failures.min(20))
        .min(BACKOFF_CAP_SECS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::InMemoryWamStore;
    use std::sync::Mutex as StdMutex;

    /// An uploader whose answers a test scripts, recording what it was given.
    #[derive(Default)]
    struct ScriptedUploader {
        answers: StdMutex<VecDeque<Result<(), UploadFailure>>>,
        seen: StdMutex<Vec<Vec<u8>>>,
    }

    impl ScriptedUploader {
        fn with(answers: impl IntoIterator<Item = Result<(), UploadFailure>>) -> Self {
            Self {
                answers: StdMutex::new(answers.into_iter().collect()),
                seen: StdMutex::new(Vec::new()),
            }
        }

        fn seen(&self) -> Vec<Vec<u8>> {
            self.seen.lock().expect("uploader lock").clone()
        }
    }

    #[async_trait]
    impl WamUploader for ScriptedUploader {
        async fn upload(&self, _t: i64, buffer: &[u8]) -> Result<(), UploadFailure> {
            self.seen
                .lock()
                .expect("uploader lock")
                .push(buffer.to_vec());
            self.answers
                .lock()
                .expect("uploader lock")
                .pop_front()
                .unwrap_or(Ok(()))
        }
    }

    fn runtime(max_queued: usize) -> WamRuntime {
        WamRuntime::new(
            WamIdentity::web(),
            Arc::new(InMemoryWamStore::new()),
            max_queued,
        )
    }

    fn receipt() -> PendingEvent {
        PendingEvent::ReceiptStanzaReceive(events::ReceiptStanzaReceive {
            receipt_stanza_total_count: Some(1),
            ..Default::default()
        })
    }

    /// A roll that keeps everything.
    fn keep_all() -> impl FnMut() -> f64 {
        || 0.0
    }

    #[tokio::test]
    async fn a_buffer_reaches_the_uploader_with_the_events_it_carries() {
        let runtime = runtime(64);
        let mut writer = WamWriter::default();
        let uploader = ScriptedUploader::with([Ok(())]);
        runtime.observe(receipt());
        runtime
            .tick(
                &mut writer,
                &uploader,
                1_755_000_000,
                &mut keep_all(),
                false,
            )
            .await;
        let seen = uploader.seen();
        assert_eq!(seen.len(), 1, "the first buffer is uploaded immediately");
        assert!(seen[0].starts_with(b"WAM"));
        let stats = runtime.stats();
        assert_eq!(stats.written, 1);
        assert_eq!(stats.uploaded, 1);
        assert_eq!(stats.discarded, 0);
    }

    #[tokio::test]
    async fn a_second_buffer_waits_for_the_rotate_interval() {
        let runtime = runtime(64);
        let mut writer = WamWriter::default();
        let uploader = ScriptedUploader::with([Ok(()), Ok(())]);
        let start = 1_755_000_000;
        runtime.observe(receipt());
        runtime
            .tick(&mut writer, &uploader, start, &mut keep_all(), false)
            .await;
        runtime.observe(receipt());
        runtime
            .tick(&mut writer, &uploader, start + 5, &mut keep_all(), false)
            .await;
        assert_eq!(uploader.seen().len(), 1, "too soon to upload again");
        runtime.observe(receipt());
        runtime
            .tick(
                &mut writer,
                &uploader,
                start + ROTATE_INTERVAL_SECS,
                &mut keep_all(),
                false,
            )
            .await;
        assert_eq!(uploader.seen().len(), 2, "the interval elapsed");
    }

    #[tokio::test]
    async fn a_failed_upload_retains_the_buffer_and_retries_it_later() {
        let store = Arc::new(InMemoryWamStore::new());
        let runtime = WamRuntime::new(WamIdentity::web(), store.clone(), 64);
        let mut writer = WamWriter::default();
        // Two attempts fail, so the buffer is retained; the next tick delivers it.
        let uploader =
            ScriptedUploader::with([Err(UploadFailure::Retryable("503".into())), Ok(())]);
        runtime.observe(receipt());
        runtime
            .tick(
                &mut writer,
                &uploader,
                1_755_000_000,
                &mut keep_all(),
                false,
            )
            .await;
        assert_eq!(runtime.stats().upload_failures, 1);
        assert_eq!(store.pending().await.expect("pending").len(), 1);

        // Still inside the cooldown the first failure started, so nothing is
        // attempted: a failing server is not asked again every five seconds.
        runtime
            .tick(
                &mut writer,
                &uploader,
                1_755_000_000,
                &mut keep_all(),
                false,
            )
            .await;
        assert_eq!(uploader.seen().len(), 1);
        assert_eq!(store.pending().await.expect("pending").len(), 1);

        runtime
            .tick(
                &mut writer,
                &uploader,
                1_755_000_000 + backoff(0),
                &mut keep_all(),
                false,
            )
            .await;
        assert_eq!(runtime.stats().uploaded, 1);
        assert!(store.pending().await.expect("pending").is_empty());
    }

    #[tokio::test]
    async fn a_failing_upload_never_reaches_the_client() {
        // The one thing telemetry may not do. Every failure path is exercised
        // and none of them returns anything a caller could propagate.
        let store = Arc::new(InMemoryWamStore::new());
        let runtime = WamRuntime::new(WamIdentity::web(), store.clone(), 64);
        let mut writer = WamWriter::default();
        let uploader = ScriptedUploader::with([
            Err(UploadFailure::Permanent("400".into())),
            Err(UploadFailure::Retryable("timeout".into())),
        ]);
        runtime.observe(receipt());
        runtime
            .tick(
                &mut writer,
                &uploader,
                1_755_000_000,
                &mut keep_all(),
                false,
            )
            .await;
        assert_eq!(runtime.stats().uploaded, 0);
        // Retained rather than lost: the retention cap is what bounds memory,
        // not the error class, which is the same choice the official client
        // makes.
        assert_eq!(store.pending().await.expect("pending").len(), 1);

        // A second observation still gets written and queued behind it.
        runtime.observe(receipt());
        runtime
            .tick(
                &mut writer,
                &uploader,
                1_755_000_000 + backoff(0),
                &mut keep_all(),
                false,
            )
            .await;
        assert_eq!(runtime.stats().written, 2);
    }

    #[tokio::test]
    async fn a_full_queue_drops_the_newest_event_and_says_so() {
        let runtime = runtime(2);
        runtime.observe(receipt());
        runtime.observe(receipt());
        runtime.observe(receipt());
        let stats = runtime.stats();
        assert_eq!(stats.observed, 3);
        assert_eq!(stats.dropped, 1);

        // A fourth drop of the same event coalesces into the report already
        // queued rather than pushing another one out.
        runtime.observe(receipt());
        assert_eq!(runtime.stats().dropped, 2);
        let queued = runtime.queued();
        assert_eq!(queued.len(), 2, "the queue stayed at its bound");
        let PendingEvent::WamDroppedEvent(report) = &queued[1] else {
            panic!("the newest slot holds the drop report");
        };
        assert_eq!(report.dropped_event_count, Some(2));
        assert_eq!(
            report.dropped_event_code,
            Some(i64::from(events::ReceiptStanzaReceive::CODE))
        );

        let mut writer = WamWriter::default();
        let uploader = ScriptedUploader::with([Ok(())]);
        runtime
            .tick(
                &mut writer,
                &uploader,
                1_755_000_000,
                &mut keep_all(),
                false,
            )
            .await;
        // Two events written: the one that survived, and one report carrying
        // both drops.
        assert_eq!(runtime.stats().written, 2);
    }

    #[tokio::test]
    async fn an_event_sampled_out_is_not_written() {
        let runtime = runtime(64);
        let mut writer = WamWriter::default();
        let uploader = ScriptedUploader::with([Ok(())]);
        runtime.observe(receipt());
        // ReceiptStanzaReceive declares a release weight of 2000, so a roll of
        // 0.5 is far past 1/2000 and the event is discarded.
        let mut roll = || 0.5;
        runtime
            .tick(&mut writer, &uploader, 1_755_000_000, &mut roll, false)
            .await;
        let stats = runtime.stats();
        assert_eq!(stats.sampled_out, 1);
        assert_eq!(stats.written, 0);
        assert!(
            uploader.seen().is_empty(),
            "an empty buffer is not uploaded"
        );
    }

    #[tokio::test]
    async fn a_buffer_past_the_size_threshold_is_uploaded_mid_drain() {
        let runtime = runtime(4096);
        let mut writer = WamWriter::default();
        let uploader = ScriptedUploader::with(std::iter::repeat_n(Ok(()), 8));
        // Each receipt is a handful of bytes, so this is several buffers' worth.
        for _ in 0..8000 {
            runtime.observe(PendingEvent::ReceiptStanzaReceive(
                events::ReceiptStanzaReceive {
                    receipt_stanza_type: Some("read".repeat(20)),
                    ..Default::default()
                },
            ));
        }
        runtime
            .tick(
                &mut writer,
                &uploader,
                1_755_000_000,
                &mut keep_all(),
                false,
            )
            .await;
        let seen = uploader.seen();
        assert!(seen.len() > 1, "one drain produced {} buffers", seen.len());
        for buffer in &seen {
            assert!(
                buffer.len() <= MAX_UPLOAD_SIZE,
                "a buffer of {} bytes was uploaded",
                buffer.len()
            );
        }
    }

    #[tokio::test]
    async fn a_buffer_past_the_upload_ceiling_is_dropped_and_reported() {
        // One event can carry a buffer past the ceiling on its own, which is
        // the case the two thresholds exist to tell apart: the flush check runs
        // between events, so only this one bounds the stanza.
        let runtime = runtime(64);
        let mut writer = WamWriter::default();
        let uploader = ScriptedUploader::with([Ok(())]);
        runtime.observe(PendingEvent::ReceiptStanzaReceive(
            events::ReceiptStanzaReceive {
                receipt_stanza_type: Some("x".repeat(MAX_UPLOAD_SIZE + 1)),
                ..Default::default()
            },
        ));
        runtime
            .tick(&mut writer, &uploader, 1_755_000_000, &mut keep_all(), true)
            .await;
        assert!(
            uploader.seen().is_empty(),
            "a buffer the server would refuse is not sent"
        );
        let stats = runtime.stats();
        assert_eq!(stats.discarded, 1);
        assert_eq!(stats.uploaded, 0);

        // And the drop is reported the way the official client reports it.
        let mut roll = || 0.0;
        runtime
            .tick(&mut writer, &uploader, 1_755_000_001, &mut roll, true)
            .await;
        let seen = uploader.seen();
        assert_eq!(seen.len(), 1, "the report itself is a small buffer");
        assert!(seen[0].len() < MAX_UPLOAD_SIZE);
    }

    #[tokio::test]
    async fn nothing_is_uploaded_when_there_is_nothing_to_say() {
        let runtime = runtime(64);
        let mut writer = WamWriter::default();
        let uploader = ScriptedUploader::with([Ok(())]);
        runtime
            .tick(&mut writer, &uploader, 1_755_000_000, &mut keep_all(), true)
            .await;
        assert!(uploader.seen().is_empty());
    }

    #[test]
    fn backoff_grows_and_is_capped() {
        assert_eq!(backoff(0), BACKOFF_BASE_SECS);
        assert_eq!(backoff(3), 8);
        assert_eq!(backoff(30), BACKOFF_CAP_SECS);
    }
}

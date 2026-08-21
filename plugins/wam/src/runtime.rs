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
    /// The client opened its chat socket. Fieldless here because it is fieldless
    /// at WA Web's own call site: the four fields the catalog declares for it
    /// name a browser handshake this client does not perform, and the official
    /// constructor writes none of them either.
    WebcSocketConnect(events::WebcSocketConnect),
    /// The client is flushing its buffers ahead of the schedule, which for this
    /// plugin means shutdown. The event declares no fields at all.
    WebWamForceFlush(events::WebWamForceFlush),
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
            Self::WebcSocketConnect(_) => events::WebcSocketConnect::CODE,
            Self::WebWamForceFlush(_) => events::WebWamForceFlush::CODE,
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
            Self::WebcSocketConnect(_) => events::WebcSocketConnect::WEIGHTS[sampling::RELEASE],
            Self::WebWamForceFlush(_) => events::WebWamForceFlush::WEIGHTS[sampling::RELEASE],
        }
    }

    fn write_into(&self, buffer: &mut WamBuffer, now_secs: i64, weight: u32) -> bool {
        let written = match self {
            Self::E2eMessageRecv(e) => buffer.write_event(e, now_secs, weight),
            Self::MessageReceive(e) => buffer.write_event(e, now_secs, weight),
            Self::ReceiptStanzaReceive(e) => buffer.write_event(e, now_secs, weight),
            Self::WamClientErrors(e) => buffer.write_event(e, now_secs, weight),
            Self::WamDroppedEvent(e) => buffer.write_event(e, now_secs, weight),
            Self::WebcSocketConnect(e) => buffer.write_event(e, now_secs, weight),
            Self::WebWamForceFlush(e) => buffer.write_event(e, now_secs, weight),
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
    Retryable {
        message: String,
        /// Seconds the server itself asked us to wait, from the `backoff`
        /// attribute of its error stanza, when it named one.
        ///
        /// Carried rather than folded into the message because it is the one
        /// part of a rejection that is an instruction. The local backoff is a
        /// guess about a server that stopped answering; this is that server
        /// telling us when to come back, and coming back earlier is how a
        /// throttle turns into a longer throttle.
        retry_after_secs: Option<u32>,
    },
    /// The server refused this buffer and will refuse it again.
    Permanent(String),
}

impl UploadFailure {
    /// A retryable failure carrying no server-directed delay.
    pub fn retryable(message: impl Into<String>) -> Self {
        Self::Retryable {
            message: message.into(),
            retry_after_secs: None,
        }
    }
}

impl std::fmt::Display for UploadFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Retryable { message: m, .. } | Self::Permanent(m) => f.write_str(m),
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
    /// Events lost because no buffer could be started to hold them: the store
    /// would not issue a sequence number, or a global no longer belongs on the
    /// channel. Counted apart from [`Self::dropped`] because the two say
    /// different things about a client, and the fix for one is not the fix for
    /// the other.
    pub unbuffered: u64,
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
    /// Losses the store caused that no buffer has carried a report for yet.
    store_losses: u64,
}

/// The plugin's telemetry runtime.
pub struct WamRuntime {
    identity: WamIdentity,
    store: Arc<dyn WamStore>,
    max_queued_events: usize,
    shared: Mutex<Shared>,
}

impl std::fmt::Debug for WamRuntime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WamRuntime")
            .field("identity", &self.identity)
            .field("max_queued_events", &self.max_queued_events)
            .finish_non_exhaustive()
    }
}

/// Why a tick is running, which decides whether it may talk to the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TickKind {
    /// The recurring flush. Uploads a buffer that is due and skips one inside a
    /// cooldown, so a failing server is asked at the backoff's pace.
    Scheduled,
    /// The last tick before the client goes away. Finishes whatever it holds
    /// and persists it, and asks the server nothing, because by the time it runs
    /// the server can no longer be asked: the host closes a plugin's resources
    /// before invoking its `shutdown` callback, so the IQ capability answers
    /// `ShuttingDown` rather than sending. What this tick is for is the store: a
    /// durable one carries the buffer into the next run. The chain is spelled
    /// out where the decision is taken, in `upload_current`.
    Final,
}

/// Why no buffer could be started.
///
/// The two are different faults with different owners: a store that will not
/// issue a sequence number, and a catalog whose global no longer belongs on the
/// channel this buffer is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NoBuffer {
    Store,
    Catalog,
}

/// How draining the retained buffers ended.
///
/// [`TickKind::Final`] ignores a cooldown an earlier tick set, because there is
/// no later tick for it to defer to. It must not ignore one *this* tick set: a
/// refusal seconds old is the server's current answer, not stale policy, and
/// asking again in the same instant is the hammering the backoff exists to stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Drained {
    WithoutFailing,
    AfterAFailure,
    /// The retained set could not be read, so whether an older buffer is
    /// waiting is unknown. Not a failure: the server was never asked.
    WithoutReadingTheSet,
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
                store_losses: 0,
            }),
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
                    // The report needs a slot and the queue has none, so the
                    // newest queued event pays for it. That event is dropped as
                    // surely as the one that just arrived, so it is counted the
                    // same way, and named in the report too when it was the same
                    // kind. A report evicted here needs no second count: its
                    // drops were counted as they happened.
                    let mut count = 1;
                    if let Some(evicted) = shared.queue.pop_back()
                        && !matches!(evicted, PendingEvent::WamDroppedEvent(_))
                    {
                        shared.stats.dropped += 1;
                        // The report names one code. An evicted event of another
                        // kind is real and counted, but there is nowhere on the
                        // wire to say which kind it was without inventing a
                        // second report the queue has no room for.
                        if i64::from(evicted.code()) == code {
                            count += 1;
                        }
                    }
                    shared.queue.push_back(PendingEvent::WamDroppedEvent(
                        events::WamDroppedEvent {
                            dropped_event_code: Some(code),
                            dropped_event_count: Some(count),
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

    /// Note a loss the store caused, to be reported once a buffer can carry it.
    ///
    /// Its own field rather than the drop counter, because the two name
    /// different faults: a drop is this runtime abandoning a buffer it could
    /// have kept, and this is the store refusing to hold one or to hand out the
    /// sequence number it needs. Queued rather than only counted locally, so the
    /// telemetry that eventually uploads says why the gap in it is there; the
    /// report rides a later buffer, which is the only kind that can carry it.
    fn note_store_loss(&self) {
        let mut shared = self.lock();
        shared.store_losses = shared.store_losses.saturating_add(1);
    }

    /// The losses noted since the last report, as the event that carries them.
    ///
    /// `None` when there are none. Counted rather than queued so a store that
    /// keeps failing reports what it lost once it can, instead of one report per
    /// tick describing the attempt to report the last one.
    fn take_store_losses(&self) -> Option<PendingEvent> {
        let count = std::mem::take(&mut self.lock().store_losses);
        i64::try_from(count).ok().filter(|n| *n > 0).map(|count| {
            PendingEvent::WamClientErrors(events::WamClientErrors {
                wam_client_buffer_store_error_count: Some(count),
                ..Default::default()
            })
        })
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
        kind: TickKind,
    ) {
        // Before anything else, so a buffer retained by this tick waits for the
        // next one and older buffers keep their place in line.
        let drained = self.drain_pending(writer, uploader, now_secs, kind).await;
        // Whether the buffer built below may still be *attempted*, which is a
        // different question from whether it is finished: a final tick always
        // finishes its buffer, because there is no later tick to finish it. A
        // refusal in the drain is the server's answer from a moment ago, so that
        // buffer waits for the cooldown it set even here. What a final tick may
        // ignore is a cooldown it did not set.
        let upload_kind = match drained {
            Drained::WithoutFailing | Drained::WithoutReadingTheSet => kind,
            Drained::AfterAFailure => TickKind::Scheduled,
        };

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
                self.upload_current(writer, uploader, now_secs, upload_kind)
                    .await;
            }
            if writer.current.is_none() {
                match self.start_buffer().await {
                    Ok(buffer) => writer.current = Some(buffer),
                    Err(reason) => {
                        self.lock().stats.unbuffered += 1;
                        // Only one of the two is the store's doing. A global the
                        // catalog no longer allows on this channel is this build
                        // disagreeing with itself, and reporting it as a storage
                        // failure would point whoever reads the counter at the
                        // wrong thing.
                        if reason == NoBuffer::Store {
                            self.note_store_loss();
                        }
                        continue;
                    }
                }
            }
            let Some(buffer) = writer.current.as_mut() else {
                continue;
            };
            // A buffer exists, so the losses noted while none could be started
            // have somewhere to go. Written here rather than queued when they
            // happen: a queued report is itself an event that needs a buffer,
            // so a store that keeps failing would drop each report and note a
            // fresh loss for it, reporting its own retries as losses forever.
            if let Some(report) = self.take_store_losses() {
                let weight = report.weight();
                if report.write_into(buffer, now_secs, weight) {
                    self.lock().stats.written += 1;
                }
            }
            if event.write_into(buffer, now_secs, weight) {
                self.lock().stats.written += 1;
            }
        }

        let due = writer.current.as_ref().is_some_and(|buffer| {
            buffer.has_events()
                && (kind == TickKind::Final
                    || buffer.len() > MAX_BUFFER_SIZE
                    || writer
                        .last_upload_secs
                        .is_none_or(|last| now_secs >= last.saturating_add(ROTATE_INTERVAL_SECS)))
        });
        // An unreadable retained set may be hiding an older buffer, and sending
        // this one would put it on the wire ahead of that one. Holding it in the
        // writer for the next tick keeps the order and costs nothing; retaining
        // it instead would discard it, because `retain` needs the same read that
        // just failed. Two cases send anyway: a final tick has no next tick to
        // hold for, and a buffer already past the size threshold would only grow
        // while it waited.
        let holds_its_place = drained == Drained::WithoutReadingTheSet
            && kind != TickKind::Final
            && writer
                .current
                .as_ref()
                .is_some_and(|buffer| buffer.len() <= MAX_BUFFER_SIZE);
        if due && !holds_its_place {
            self.upload_current(writer, uploader, now_secs, upload_kind)
                .await;
        }
    }

    /// A fresh buffer, carrying the identity's globals.
    async fn start_buffer(&self) -> Result<WamBuffer, NoBuffer> {
        let sequence = match self.store.next_sequence(Channel::Regular).await {
            Ok(sequence) => sequence,
            Err(err) => {
                warn!("wam: no sequence number, dropping the event: {err}");
                return Err(NoBuffer::Store);
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
                return Err(NoBuffer::Catalog);
            }
        }
        Ok(buffer)
    }

    /// Take the current buffer and try to deliver it.
    async fn upload_current(
        &self,
        writer: &mut WamWriter,
        uploader: &dyn WamUploader,
        now_secs: i64,
        kind: TickKind,
    ) {
        let Some(buffer) = writer.current.take() else {
            return;
        };
        if !buffer.has_events() {
            return;
        }
        let bytes = buffer.into_bytes();
        // A final tick finishes the buffer and persists it without asking the
        // server, because by then it cannot be asked. `PluginHost::shutdown`
        // runs `signal_shutdown` first, which calls `close_installed_resources`
        // and sets each plugin's resources closed; only then does it invoke the
        // `shutdown` callback this tick runs in. `PluginIq::execute` opens with
        // `ensure_active`, so every upload from here answers
        // `PluginResourceError::ShuttingDown` before a byte leaves. The attempt
        // is cheap, since it fails before any I/O, and it is skipped anyway: it
        // ends at the same retained buffer while recording an upload failure
        // that was never an upload, and that counter is one an operator reads.
        // A durable store delivers the buffer on the next run; the in-memory
        // default loses it either way, which is what it says about itself
        // through `store_is_durable`.
        if kind == TickKind::Final {
            self.retain(bytes).await;
            return;
        }
        // Inside a cooldown the buffer is retained without an attempt: the
        // cooldown exists so a failing server is asked at the backoff's pace,
        // and a buffer finished mid-drain must not get around it.
        if writer.retry_after_secs.is_some_and(|at| now_secs < at) {
            self.retain(bytes).await;
            return;
        }
        match self.deliver(writer, uploader, now_secs, &bytes).await {
            Delivery::Accepted => writer.last_upload_secs = Some(now_secs),
            Delivery::Retryable => self.retain(bytes).await,
            Delivery::Unsendable => {}
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
            return Delivery::Unsendable;
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
                // The longer of the two, never the shorter. The local curve
                // protects a server that stopped answering; a `backoff` the
                // server named is that server saying when to come back, and
                // asking before it said is how a throttle gets extended. Where
                // our own curve has already grown past it, it stays: the server
                // asked for a floor, not a ceiling.
                let directed = match &failure {
                    UploadFailure::Retryable {
                        retry_after_secs, ..
                    } => retry_after_secs.map(i64::from).unwrap_or(0),
                    UploadFailure::Permanent(_) => 0,
                };
                let wait = backoff(writer.consecutive_failures).max(directed);
                writer.consecutive_failures = writer.consecutive_failures.saturating_add(1);
                writer.retry_after_secs = Some(now_secs.saturating_add(wait));
                debug!("wam: upload failed, waiting {wait}s: {failure}");
                // A permanent refusal is a refusal of this buffer, so keeping it
                // buys a retry that fails the same way forever, at the backoff's
                // pace, until unrelated traffic evicts it. The official client
                // re-stores it anyway; this drops it and counts it, which is the
                // same accounting a buffer past the upload ceiling gets.
                if matches!(failure, UploadFailure::Permanent(_)) {
                    self.lock().stats.discarded += 1;
                    self.report_buffer_drop();
                    return Delivery::Unsendable;
                }
                Delivery::Retryable
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
        // A read that failed is not an empty store. Treating it as one would put
        // this buffer under a key a retained buffer may already hold and let the
        // store's insert semantics overwrite an undelivered one, and it would
        // compare the retention cap against a total known to be wrong. Dropping
        // the newest buffer costs one buffer; guessing costs whichever buffer it
        // lands on.
        let pending = match self.store.pending().await {
            Ok(pending) => pending,
            Err(err) => {
                warn!("wam: cannot retain a buffer without reading the retained set: {err}");
                self.lock().stats.discarded += 1;
                self.note_store_loss();
                return;
            }
        };
        let retained: usize = pending.iter().map(|b| b.bytes.len()).sum();
        if retained.saturating_add(bytes.len()) > MAX_BUFFER_SIZE {
            // The newest buffer is the one that goes, and the set it did not fit
            // into stays whole. Clearing the set instead would drop this buffer
            // anyway and take the older ones with it, and it cannot be done
            // safely from here: `remove_pending` is allowed to fail per key, so
            // a delete that stopped halfway would leave an arbitrary subset for
            // the next drain to upload with holes where the rest had been. The
            // set is not stale either, since a buffer the server refuses for
            // good is dropped at the point of refusal and never reaches it.
            self.lock().stats.discarded += 1;
            self.report_buffer_drop();
            return;
        }
        let Some(key) = next_key(&pending) else {
            warn!("wam: no free key is left in the retained set, dropping a buffer");
            self.lock().stats.discarded += 1;
            self.note_store_loss();
            return;
        };
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
            self.note_store_loss();
        }
    }

    /// Try the buffers a previous run or a previous failure left behind.
    ///
    /// Returns whether the server refused one of them, which the caller needs
    /// because a refusal here is this instant's answer rather than an old one.
    async fn drain_pending(
        &self,
        writer: &mut WamWriter,
        uploader: &dyn WamUploader,
        now_secs: i64,
        kind: TickKind,
    ) -> Drained {
        // Nothing to do on a final tick: it cannot reach the server (see
        // `upload_current`), and what is already retained is already persisted.
        if kind == TickKind::Final {
            return Drained::WithoutFailing;
        }
        if writer.retry_after_secs.is_some_and(|at| now_secs < at) {
            return Drained::WithoutFailing;
        }
        let pending = match self.store.pending().await {
            Ok(pending) => pending,
            Err(err) => {
                warn!("wam: could not read the retained buffers: {err}");
                return Drained::WithoutReadingTheSet;
            }
        };
        // This uploader speaks the regular channel's `w:stats` path and nothing
        // else. A store shared with a future private-channel writer, or carried
        // across a version that had one, hands back buffers this cannot send:
        // a private buffer needs its own token and its own endpoint. The field
        // exists to be read, so read it and leave the rest to their owner.
        for buffer in pending
            .into_iter()
            .filter(|buffer| buffer.channel == Channel::Regular)
        {
            // Removed once the outcome is known, never before it. Removing first
            // would lose the buffer to a crash or a cancelled flush between the
            // delete and the answer, which is the one window a durable store
            // exists to close. A buffer left in place is already counted against
            // the retention cap, so keeping it there instead of re-storing it
            // also keeps the cap honest without a second write.
            match self
                .deliver(writer, uploader, now_secs, &buffer.bytes)
                .await
            {
                Delivery::Accepted => {
                    writer.last_upload_secs = Some(now_secs);
                    if !self.forget(buffer.key).await {
                        return Drained::WithoutFailing;
                    }
                }
                Delivery::Retryable => {
                    // One failing buffer means the server or the link is
                    // unhappy; marching through the rest would just fail as
                    // many times.
                    return Drained::AfterAFailure;
                }
                Delivery::Unsendable => {
                    if !self.forget(buffer.key).await {
                        return Drained::WithoutFailing;
                    }
                }
            }
        }
        Drained::WithoutFailing
    }

    /// Drop a retained buffer that has been dealt with, reporting whether the
    /// store agreed.
    ///
    /// A removal that fails leaves the buffer retained, so the next drain offers
    /// it again. For one the server accepted that is a duplicate upload, which
    /// is the direction to fail in: the server sees a buffer twice instead of
    /// this client losing one. The caller stops draining on `false`, because a
    /// store that cannot delete will not delete the next one either, and
    /// marching on would upload the whole retained set again on every tick for
    /// as long as the failure lasts.
    async fn forget(&self, key: u64) -> bool {
        match self.store.remove_pending(key).await {
            Ok(()) => true,
            Err(err) => {
                warn!("wam: a delivered buffer stayed retained and may be sent again: {err}");
                false
            }
        }
    }
}

/// What became of one upload attempt.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Delivery {
    Accepted,
    /// Refused for a reason that may not repeat, so the buffer is kept.
    Retryable,
    /// Not worth keeping: the server refused this buffer and will refuse it
    /// again, or it was never sendable in the first place.
    Unsendable,
}

/// A key no retained buffer is using.
///
/// Read off the retained buffers rather than a counter that starts at 1. A
/// durable store hands back the previous run's buffers, and a counter that does
/// not know about them would hand out a key one of them already holds, which the
/// store's insert semantics turn into an undelivered buffer being overwritten.
/// The caller already has the list, since the retention cap is computed from it.
///
/// One past the highest, except when the highest is the largest a key can be.
/// Saturating there would hand back that same occupied key and let the store's
/// insert semantics overwrite the buffer holding it, which is the loss this
/// function exists to prevent. A key at the top says nothing about the space
/// below it, though, and a store that picks its keys by hash puts one there
/// while holding almost nothing, so the fallback takes the lowest key nobody
/// holds. `None` only when every key is taken, which nothing this retention cap
/// allows can reach.
fn next_key(pending: &[PendingBuffer]) -> Option<u64> {
    let mut used: Vec<u64> = pending.iter().map(|b| b.key).collect();
    used.sort_unstable();
    used.dedup();
    if let Some(next) = used.last().copied().unwrap_or(0).checked_add(1) {
        return Some(next);
    }
    // Keys start at 1, so the walk does too.
    let mut candidate = 1u64;
    for key in used {
        match key.cmp(&candidate) {
            std::cmp::Ordering::Less => {}
            std::cmp::Ordering::Greater => return Some(candidate),
            std::cmp::Ordering::Equal => candidate = candidate.checked_add(1)?,
        }
    }
    Some(candidate)
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
    use crate::store::{InMemoryWamStore, WamStoreError};
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
                TickKind::Scheduled,
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
            .tick(
                &mut writer,
                &uploader,
                start,
                &mut keep_all(),
                TickKind::Scheduled,
            )
            .await;
        runtime.observe(receipt());
        runtime
            .tick(
                &mut writer,
                &uploader,
                start + 5,
                &mut keep_all(),
                TickKind::Scheduled,
            )
            .await;
        assert_eq!(uploader.seen().len(), 1, "too soon to upload again");
        runtime.observe(receipt());
        runtime
            .tick(
                &mut writer,
                &uploader,
                start + ROTATE_INTERVAL_SECS,
                &mut keep_all(),
                TickKind::Scheduled,
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
        let uploader = ScriptedUploader::with([Err(UploadFailure::retryable("503")), Ok(())]);
        runtime.observe(receipt());
        runtime
            .tick(
                &mut writer,
                &uploader,
                1_755_000_000,
                &mut keep_all(),
                TickKind::Scheduled,
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
                TickKind::Scheduled,
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
                TickKind::Scheduled,
            )
            .await;
        assert_eq!(runtime.stats().uploaded, 1);
        assert!(store.pending().await.expect("pending").is_empty());
    }

    /// The local curve is a guess about a server that stopped answering. A
    /// `backoff` the server named is that server saying when to come back, and
    /// its first local step is one second, so dropping it asks again 29 seconds
    /// early and extends the throttle that produced it.
    #[tokio::test]
    async fn a_server_directed_backoff_outlasts_the_local_one() {
        let store = Arc::new(InMemoryWamStore::new());
        let runtime = WamRuntime::new(WamIdentity::web(), store.clone(), 64);
        let mut writer = WamWriter::default();
        let uploader = ScriptedUploader::with([
            Err(UploadFailure::Retryable {
                message: "rate-overlimit".into(),
                retry_after_secs: Some(30),
            }),
            Ok(()),
        ]);

        runtime.observe(receipt());
        runtime
            .tick(
                &mut writer,
                &uploader,
                1_755_000_000,
                &mut keep_all(),
                TickKind::Scheduled,
            )
            .await;
        assert_eq!(uploader.seen().len(), 1);

        // A tick past the local step and short of what the server asked for
        // does not ask again.
        runtime
            .tick(
                &mut writer,
                &uploader,
                1_755_000_000 + backoff(0) + 1,
                &mut keep_all(),
                TickKind::Scheduled,
            )
            .await;
        assert_eq!(uploader.seen().len(), 1, "the server named 30s, not 1s");

        runtime
            .tick(
                &mut writer,
                &uploader,
                1_755_000_000 + 30,
                &mut keep_all(),
                TickKind::Scheduled,
            )
            .await;
        assert_eq!(uploader.seen().len(), 2);
        assert!(store.pending().await.expect("pending").is_empty());
    }

    /// The set the newest buffer did not fit into is not stale and is not this
    /// function's to clear: `remove_pending` may fail per key, so a delete that
    /// stopped halfway would leave a subset for the next drain to send with
    /// holes where the rest had been.
    #[tokio::test]
    async fn a_buffer_past_the_retention_cap_does_not_take_the_set_with_it() {
        let store = Arc::new(InMemoryWamStore::new());
        store
            .put_pending(PendingBuffer {
                key: 1,
                channel: Channel::Regular,
                bytes: vec![b'x'; MAX_BUFFER_SIZE],
            })
            .await
            .expect("seed a set already at the cap");
        let runtime = WamRuntime::new(WamIdentity::web(), store.clone(), 64);
        let mut writer = WamWriter::default();
        let uploader = ScriptedUploader::with([Err(UploadFailure::retryable("503"))]);

        runtime.observe(receipt());
        runtime
            .tick(
                &mut writer,
                &uploader,
                1_755_000_000,
                &mut keep_all(),
                TickKind::Final,
            )
            .await;

        let pending = store.pending().await.expect("pending");
        assert_eq!(pending.len(), 1, "the older buffer is still there");
        assert_eq!(pending[0].key, 1);
        assert_eq!(runtime.stats().discarded, 1, "only the newest was dropped");
    }

    /// The store hands back every retained buffer and this uploader speaks one
    /// channel. A private buffer needs its own token and endpoint, so sending
    /// its bytes down `w:stats` would put it on the wrong path entirely.
    #[tokio::test]
    async fn a_retained_buffer_from_another_channel_is_left_for_its_owner() {
        let store = Arc::new(InMemoryWamStore::new());
        for (key, channel) in [(1, Channel::Private), (2, Channel::Regular)] {
            store
                .put_pending(PendingBuffer {
                    key,
                    channel,
                    bytes: vec![b'W', b'A', b'M', 5, 1, 0, 0, key as u8],
                })
                .await
                .expect("seed");
        }
        let runtime = WamRuntime::new(WamIdentity::web(), store.clone(), 64);
        let mut writer = WamWriter::default();
        let uploader = ScriptedUploader::with([]);

        runtime
            .tick(
                &mut writer,
                &uploader,
                1_755_000_000,
                &mut keep_all(),
                TickKind::Scheduled,
            )
            .await;

        let seen = uploader.seen();
        assert_eq!(seen.len(), 1, "only the regular buffer was sent");
        assert_eq!(seen[0][7], 2);
        let pending = store.pending().await.expect("pending");
        assert_eq!(pending.len(), 1, "the private one is still retained");
        assert_eq!(pending[0].channel, Channel::Private);
    }

    /// The host closes a plugin's resources before it calls that plugin's
    /// `shutdown`, so the IQ capability answers `ShuttingDown` rather than
    /// sending. A final tick therefore does the half it still can: finish the
    /// buffer and put it in the store, where a durable one carries it into the
    /// next run.
    #[tokio::test]
    async fn a_final_tick_persists_its_buffer_without_asking_the_server() {
        let store = Arc::new(InMemoryWamStore::new());
        let runtime = WamRuntime::new(WamIdentity::web(), store.clone(), 64);
        let mut writer = WamWriter::default();
        let uploader = ScriptedUploader::with([]);

        runtime.observe(receipt());
        runtime
            .tick(
                &mut writer,
                &uploader,
                1_755_000_000,
                &mut keep_all(),
                TickKind::Final,
            )
            .await;

        assert!(
            uploader.seen().is_empty(),
            "the server cannot be reached from a shutdown callback"
        );
        let stats = runtime.stats();
        assert_eq!(stats.written, 1, "the event still reached a buffer");
        assert_eq!(
            stats.upload_failures, 0,
            "and no upload was recorded as having failed"
        );
        assert_eq!(
            store.pending().await.expect("pending").len(),
            1,
            "the buffer is in the store for the next run"
        );
    }

    #[tokio::test]
    async fn a_permanently_refused_buffer_is_dropped_rather_than_retried_forever() {
        // Keeping it would buy a retry that fails the same way every two
        // minutes until unrelated traffic evicts it.
        let store = Arc::new(InMemoryWamStore::new());
        let runtime = WamRuntime::new(WamIdentity::web(), store.clone(), 64);
        let mut writer = WamWriter::default();
        let uploader = ScriptedUploader::with([Err(UploadFailure::Permanent("400".into()))]);
        runtime.observe(receipt());
        runtime
            .tick(
                &mut writer,
                &uploader,
                1_755_000_000,
                &mut keep_all(),
                TickKind::Scheduled,
            )
            .await;
        assert_eq!(uploader.seen().len(), 1);
        assert!(
            store.pending().await.expect("pending").is_empty(),
            "a buffer the server will refuse again is not kept"
        );
        assert_eq!(runtime.stats().discarded, 1);
    }

    #[tokio::test]
    async fn a_failing_upload_never_reaches_the_client() {
        // The one thing telemetry may not do. Both failure classes are
        // exercised and neither returns anything a caller could propagate, nor
        // stops the next event being written.
        let store = Arc::new(InMemoryWamStore::new());
        let runtime = WamRuntime::new(WamIdentity::web(), store.clone(), 64);
        let mut writer = WamWriter::default();
        let uploader = ScriptedUploader::with([
            Err(UploadFailure::retryable("timeout")),
            Err(UploadFailure::Permanent("400".into())),
        ]);
        runtime.observe(receipt());
        runtime
            .tick(
                &mut writer,
                &uploader,
                1_755_000_000,
                &mut keep_all(),
                TickKind::Scheduled,
            )
            .await;
        assert_eq!(runtime.stats().uploaded, 0);
        // Retained, because a timeout is not a refusal. The retention cap is
        // what bounds the memory.
        assert_eq!(store.pending().await.expect("pending").len(), 1);

        // The retry is refused outright, so that buffer is dropped, and a new
        // observation is still written.
        runtime.observe(receipt());
        runtime
            .tick(
                &mut writer,
                &uploader,
                1_755_000_000 + backoff(0),
                &mut keep_all(),
                TickKind::Scheduled,
            )
            .await;
        // The refused buffer is gone rather than queued for another refusal.
        // What is retained now is the buffer this tick built, held back by the
        // cooldown the refusal started.
        let stats = runtime.stats();
        assert_eq!(stats.uploaded, 0);
        assert_eq!(stats.discarded, 1);
        assert!(stats.written >= 2);
    }

    #[tokio::test]
    async fn a_full_queue_drops_the_newest_event_and_says_so() {
        let runtime = runtime(2);
        runtime.observe(receipt());
        runtime.observe(receipt());
        runtime.observe(receipt());
        let stats = runtime.stats();
        assert_eq!(stats.observed, 3);
        // Two: the event that arrived to a full queue, and the queued event
        // evicted to make room for the report about it.
        assert_eq!(stats.dropped, 2);

        // A fourth drop of the same event coalesces into the report already
        // queued rather than pushing another one out.
        runtime.observe(receipt());
        assert_eq!(runtime.stats().dropped, 3);
        let queued = runtime.queued();
        assert_eq!(queued.len(), 2, "the queue stayed at its bound");
        let PendingEvent::WamDroppedEvent(report) = &queued[1] else {
            panic!("the newest slot holds the drop report");
        };
        // What the server is told matches what the local counter says, because
        // every one of those drops was a receipt.
        assert_eq!(report.dropped_event_count, Some(3));
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
                TickKind::Scheduled,
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
            .tick(
                &mut writer,
                &uploader,
                1_755_000_000,
                &mut roll,
                TickKind::Scheduled,
            )
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
                TickKind::Scheduled,
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
            .tick(
                &mut writer,
                &uploader,
                1_755_000_000,
                &mut keep_all(),
                TickKind::Scheduled,
            )
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
            .tick(
                &mut writer,
                &uploader,
                1_755_000_001,
                &mut roll,
                TickKind::Scheduled,
            )
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
            .tick(
                &mut writer,
                &uploader,
                1_755_000_000,
                &mut keep_all(),
                TickKind::Final,
            )
            .await;
        assert!(uploader.seen().is_empty());
    }

    #[tokio::test]
    async fn a_retained_buffer_never_overwrites_one_a_previous_run_left() {
        // A durable store hands back the last run's buffers, so a key counter
        // starting at 1 would reuse one of theirs and lose it.
        let store = Arc::new(InMemoryWamStore::new());
        store
            .put_pending(PendingBuffer {
                key: 1,
                channel: Channel::Regular,
                bytes: b"WAM-from-a-previous-run".to_vec(),
            })
            .await
            .expect("seed");
        let runtime = WamRuntime::new(WamIdentity::web(), store.clone(), 64);
        let mut writer = WamWriter::default();
        let uploader = ScriptedUploader::with([
            Err(UploadFailure::retryable("503")),
            Err(UploadFailure::retryable("503")),
        ]);
        runtime.observe(receipt());
        runtime
            .tick(
                &mut writer,
                &uploader,
                1_755_000_000,
                &mut keep_all(),
                TickKind::Scheduled,
            )
            .await;

        let pending = store.pending().await.expect("pending");
        assert_eq!(pending.len(), 2, "both buffers are retained");
        assert!(
            pending
                .iter()
                .any(|b| b.bytes == b"WAM-from-a-previous-run"),
            "the previous run's buffer survived"
        );
    }

    /// A store that answers `pending()` with an error while still accepting
    /// writes, which is the failure that matters: the runtime cannot see what is
    /// retained but can still overwrite it.
    #[derive(Debug)]
    struct BlindStore {
        inner: InMemoryWamStore,
        blind: StdMutex<bool>,
    }

    impl BlindStore {
        fn new() -> Self {
            Self {
                inner: InMemoryWamStore::new(),
                blind: StdMutex::new(true),
            }
        }

        fn see_again(&self) {
            *self.blind.lock().expect("blind lock") = false;
        }
    }

    #[async_trait]
    impl WamStore for BlindStore {
        async fn next_sequence(&self, channel: Channel) -> Result<u16, WamStoreError> {
            self.inner.next_sequence(channel).await
        }

        async fn put_pending(&self, buffer: PendingBuffer) -> Result<(), WamStoreError> {
            self.inner.put_pending(buffer).await
        }

        async fn pending(&self) -> Result<Vec<PendingBuffer>, WamStoreError> {
            if *self.blind.lock().expect("blind lock") {
                return Err(WamStoreError::new("the store cannot be read"));
            }
            self.inner.pending().await
        }

        async fn remove_pending(&self, key: u64) -> Result<(), WamStoreError> {
            self.inner.remove_pending(key).await
        }
    }

    /// A read that fails may be hiding an older retained buffer. Sending the
    /// fresh one anyway puts it on the wire ahead of that one; retaining it
    /// discards it, since `retain` needs the same read. Holding it costs
    /// nothing and keeps both the order and the data.
    #[tokio::test]
    async fn a_buffer_waits_rather_than_jumping_a_retained_set_that_cannot_be_read() {
        let store = Arc::new(BlindStore::new());
        store
            .inner
            .put_pending(PendingBuffer {
                key: 1,
                channel: Channel::Regular,
                bytes: vec![b'W', b'A', b'M', 5, 1, 0, 0, 0],
            })
            .await
            .expect("seed the retained set");
        let runtime = WamRuntime::new(WamIdentity::web(), store.clone(), 64);
        let mut writer = WamWriter::default();
        let uploader = ScriptedUploader::with([]);

        runtime.observe(receipt());
        runtime
            .tick(
                &mut writer,
                &uploader,
                1_755_000_000,
                &mut keep_all(),
                TickKind::Scheduled,
            )
            .await;
        assert!(
            uploader.seen().is_empty(),
            "nothing goes out ahead of a set that could not be read"
        );
        assert_eq!(runtime.stats().discarded, 0, "and nothing is discarded");

        // Once the store answers, the retained buffer goes first and the held
        // one follows it, still carrying the event observed before the failure.
        store.see_again();
        runtime
            .tick(
                &mut writer,
                &uploader,
                1_755_000_000,
                &mut keep_all(),
                TickKind::Scheduled,
            )
            .await;
        assert_eq!(uploader.seen().len(), 1, "the retained one goes first");

        // The drain stamped `last_upload_secs`, so the held buffer is not due
        // in the same instant; it goes out on the rotate that follows.
        runtime
            .tick(
                &mut writer,
                &uploader,
                1_755_000_000 + ROTATE_INTERVAL_SECS,
                &mut keep_all(),
                TickKind::Scheduled,
            )
            .await;
        let seen = uploader.seen();
        assert_eq!(seen.len(), 2, "both go out, and neither was lost");
        assert_eq!(
            seen[0],
            vec![b'W', b'A', b'M', 5, 1, 0, 0, 0],
            "oldest first"
        );
    }

    /// A loss the store caused is not a loss this runtime chose. The catalog
    /// has a field for each, and only the local counter used to move, so the
    /// telemetry that eventually uploaded never said why it had a gap.
    ///
    /// Counted while it cannot be reported and written once it can: queueing
    /// the report instead would make it an event that itself needs a buffer,
    /// and a store that keeps failing would drop each one and note a fresh loss
    /// for it, reporting its own retries forever.
    #[tokio::test]
    async fn a_store_that_loses_a_buffer_reports_it_once_a_buffer_can_carry_it() {
        let store = Arc::new(BlindStore::new());
        let runtime = WamRuntime::new(WamIdentity::web(), store.clone(), 64);
        let mut writer = WamWriter::default();
        let uploader = ScriptedUploader::with([Err(UploadFailure::retryable("503"))]);

        runtime.observe(receipt());
        runtime
            .tick(
                &mut writer,
                &uploader,
                1_755_000_000,
                &mut keep_all(),
                TickKind::Final,
            )
            .await;
        assert_eq!(runtime.stats().discarded, 1);
        assert!(
            runtime.queued().is_empty(),
            "the report is not itself an event waiting for a buffer"
        );

        // Once the store answers, the next buffer carries the report.
        store.see_again();
        runtime.observe(receipt());
        runtime
            .tick(
                &mut writer,
                &uploader,
                1_755_000_000 + ROTATE_INTERVAL_SECS,
                &mut keep_all(),
                TickKind::Scheduled,
            )
            .await;
        assert_eq!(
            runtime.stats().written,
            3,
            "the receipt from each tick, plus one report for the loss"
        );
    }

    /// A store that will not issue a sequence number costs every event it is
    /// handed. What it must not cost is a report per tick describing the last
    /// failed report: nothing is queued, so an idle tick notes nothing.
    #[tokio::test]
    async fn a_sequence_outage_does_not_report_its_own_retries() {
        let runtime = WamRuntime::new(WamIdentity::web(), Arc::new(SequencelessStore), 64);
        let mut writer = WamWriter::default();
        let uploader = ScriptedUploader::with([]);

        runtime.observe(receipt());
        for tick in 0..4 {
            runtime
                .tick(
                    &mut writer,
                    &uploader,
                    1_755_000_000 + tick,
                    &mut keep_all(),
                    TickKind::Scheduled,
                )
                .await;
        }

        assert_eq!(
            runtime.stats().unbuffered,
            1,
            "one event was lost, and the three idle ticks after it lost nothing"
        );
        assert!(runtime.queued().is_empty());
    }

    #[tokio::test]
    async fn a_store_that_cannot_be_read_does_not_get_a_guessed_key() {
        // A failed read is not an empty store. Retaining under a guessed key
        // would overwrite whichever undelivered buffer already holds it, so the
        // new buffer is dropped and counted instead.
        //
        // Driven by a final tick, because a scheduled one no longer reaches
        // `retain` here: it holds the buffer for the next tick rather than
        // sending it ahead of a set it could not read. A final tick has no next
        // tick, so it still sends, still fails, and still lands on the guard
        // this test is about.
        let store = Arc::new(BlindStore::new());
        store
            .inner
            .put_pending(PendingBuffer {
                key: 1,
                channel: Channel::Regular,
                bytes: b"WAM-undelivered".to_vec(),
            })
            .await
            .expect("seed");
        let runtime = WamRuntime::new(WamIdentity::web(), store.clone(), 64);
        let mut writer = WamWriter::default();
        let uploader = ScriptedUploader::with([Err(UploadFailure::retryable("503"))]);
        runtime.observe(receipt());
        runtime
            .tick(
                &mut writer,
                &uploader,
                1_755_000_000,
                &mut keep_all(),
                TickKind::Final,
            )
            .await;

        store.see_again();
        let pending = store.pending().await.expect("pending");
        assert_eq!(pending.len(), 1, "nothing was written over the seeded key");
        assert_eq!(pending[0].bytes, b"WAM-undelivered");
        assert_eq!(runtime.stats().discarded, 1);
    }

    /// A store that will not issue a sequence number, so no buffer can start.
    #[derive(Debug, Default)]
    struct SequencelessStore;

    #[async_trait]
    impl WamStore for SequencelessStore {
        async fn next_sequence(&self, _channel: Channel) -> Result<u16, WamStoreError> {
            Err(WamStoreError::new("no sequence"))
        }

        async fn put_pending(&self, _buffer: PendingBuffer) -> Result<(), WamStoreError> {
            Ok(())
        }

        async fn pending(&self) -> Result<Vec<PendingBuffer>, WamStoreError> {
            Ok(Vec::new())
        }

        async fn remove_pending(&self, _key: u64) -> Result<(), WamStoreError> {
            Ok(())
        }
    }

    #[tokio::test]
    async fn an_event_with_no_buffer_to_hold_it_is_counted_apart_from_queue_pressure() {
        // Both lose an event, but one says the client is producing faster than
        // it uploads and the other says the store is broken. An operator acts on
        // them differently, so they are counted differently.
        let runtime = WamRuntime::new(WamIdentity::web(), Arc::new(SequencelessStore), 64);
        let mut writer = WamWriter::default();
        let uploader = ScriptedUploader::with([Ok(())]);
        runtime.observe(receipt());
        runtime
            .tick(
                &mut writer,
                &uploader,
                1_755_000_000,
                &mut keep_all(),
                TickKind::Scheduled,
            )
            .await;
        let stats = runtime.stats();
        assert_eq!(stats.unbuffered, 1);
        assert_eq!(stats.dropped, 0, "the queue was never full");
        assert_eq!(stats.written, 0);
    }

    /// A store that counts the removals asked of it, so a test can see *when*
    /// a retained buffer was forgotten and not only that it eventually was.
    #[derive(Debug, Default)]
    struct CountingStore {
        inner: InMemoryWamStore,
        removals: StdMutex<Vec<u64>>,
    }

    impl CountingStore {
        fn removals(&self) -> Vec<u64> {
            self.removals.lock().expect("removals lock").clone()
        }
    }

    #[async_trait]
    impl WamStore for CountingStore {
        async fn next_sequence(&self, channel: Channel) -> Result<u16, WamStoreError> {
            self.inner.next_sequence(channel).await
        }

        async fn put_pending(&self, buffer: PendingBuffer) -> Result<(), WamStoreError> {
            self.inner.put_pending(buffer).await
        }

        async fn pending(&self) -> Result<Vec<PendingBuffer>, WamStoreError> {
            self.inner.pending().await
        }

        async fn remove_pending(&self, key: u64) -> Result<(), WamStoreError> {
            self.removals.lock().expect("removals lock").push(key);
            self.inner.remove_pending(key).await
        }
    }

    #[tokio::test]
    async fn a_retained_buffer_is_forgotten_only_once_the_server_has_taken_it() {
        // Removing before the answer loses the buffer to a crash in between,
        // which is the one window a durable store exists to close. So the test
        // is about when the removal happens, not about whether the bytes are
        // still somewhere afterwards.
        let store = Arc::new(CountingStore::default());
        store
            .put_pending(PendingBuffer {
                key: 1,
                channel: Channel::Regular,
                bytes: b"WAM-retained".to_vec(),
            })
            .await
            .expect("seed");
        store.removals.lock().expect("removals lock").clear();
        let runtime = WamRuntime::new(WamIdentity::web(), store.clone(), 64);
        let mut writer = WamWriter::default();

        // First tick: the server refuses. Nothing is removed, and the buffer
        // stays under the key it already had.
        let refusing = ScriptedUploader::with([Err(UploadFailure::retryable("503"))]);
        runtime
            .tick(
                &mut writer,
                &refusing,
                1_755_000_000,
                &mut keep_all(),
                TickKind::Scheduled,
            )
            .await;
        assert_eq!(
            store.removals(),
            Vec::<u64>::new(),
            "a refused buffer is never unstored"
        );
        let pending = store.pending().await.expect("pending");
        assert_eq!(pending.len(), 1, "a refused buffer is still retained");
        assert_eq!(pending[0].key, 1, "and under the key it already had");
        assert_eq!(pending[0].bytes, b"WAM-retained");

        // Second tick, past the cooldown: accepted, and only now forgotten.
        let accepting = ScriptedUploader::with([Ok(())]);
        runtime
            .tick(
                &mut writer,
                &accepting,
                1_755_000_600,
                &mut keep_all(),
                TickKind::Scheduled,
            )
            .await;
        assert_eq!(store.removals(), vec![1], "an accepted buffer is forgotten");
        assert!(store.pending().await.expect("pending").is_empty());
        assert_eq!(accepting.seen().len(), 1);
    }

    /// A store that accepts everything but refuses to delete.
    #[derive(Debug, Default)]
    struct UndeletableStore {
        inner: InMemoryWamStore,
    }

    #[async_trait]
    impl WamStore for UndeletableStore {
        async fn next_sequence(&self, channel: Channel) -> Result<u16, WamStoreError> {
            self.inner.next_sequence(channel).await
        }

        async fn put_pending(&self, buffer: PendingBuffer) -> Result<(), WamStoreError> {
            self.inner.put_pending(buffer).await
        }

        async fn pending(&self) -> Result<Vec<PendingBuffer>, WamStoreError> {
            self.inner.pending().await
        }

        async fn remove_pending(&self, _key: u64) -> Result<(), WamStoreError> {
            Err(WamStoreError::new("this store cannot delete"))
        }
    }

    #[tokio::test]
    async fn a_store_that_cannot_delete_stops_the_drain_instead_of_resending_it_all() {
        // A buffer that stays retained after being accepted comes back on the
        // next tick. Marching through the rest would upload the whole set again
        // every tick for as long as the store stays broken, so the drain stops
        // after the first one that would not go away.
        let store = Arc::new(UndeletableStore::default());
        for key in 1..=3 {
            store
                .put_pending(PendingBuffer {
                    key,
                    channel: Channel::Regular,
                    bytes: format!("WAM-{key}").into_bytes(),
                })
                .await
                .expect("seed");
        }
        let runtime = WamRuntime::new(WamIdentity::web(), store.clone(), 64);
        let mut writer = WamWriter::default();
        let uploader = ScriptedUploader::with([Ok(()), Ok(()), Ok(())]);
        runtime
            .tick(
                &mut writer,
                &uploader,
                1_755_000_000,
                &mut keep_all(),
                TickKind::Scheduled,
            )
            .await;
        assert_eq!(
            uploader.seen().len(),
            1,
            "the drain stopped at the buffer that would not go away"
        );
        assert_eq!(store.pending().await.expect("pending").len(), 3);
    }

    #[test]
    fn next_key_clears_every_key_already_in_use() {
        assert_eq!(next_key(&[]), Some(1));
        let held = |key| PendingBuffer {
            key,
            channel: Channel::Regular,
            bytes: Vec::new(),
        };
        assert_eq!(next_key(&[held(1), held(7), held(3)]), Some(8));
        // The top of the range is not a key to hand out again: saturating there
        // would return the one the buffer at `u64::MAX` already holds. The space
        // below it is untouched, so that is where the next key comes from rather
        // than nowhere.
        assert_eq!(next_key(&[held(u64::MAX)]), Some(1));
        assert_eq!(next_key(&[held(1), held(2), held(u64::MAX)]), Some(3));
    }

    #[test]
    fn backoff_grows_and_is_capped() {
        assert_eq!(backoff(0), BACKOFF_BASE_SECS);
        assert_eq!(backoff(3), 8);
        assert_eq!(backoff(30), BACKOFF_CAP_SECS);
    }
}

//! Non-blocking delivery of core events to asynchronous observers.

#[cfg(test)]
mod tests;

use super::{Client, Event, EventHandler, EventInterest};
use futures::FutureExt;
use portable_atomic::AtomicU64;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::Ordering;
use std::sync::{Arc, Weak};
use wacore::runtime::{AbortHandle, ShutdownNotifier, ShutdownSignal, wait_for_shutdown};

pub(super) type EventHandlerCallback =
    Arc<dyn Fn(Arc<Event>, Arc<Client>) -> Pin<Box<dyn Future<Output = ()> + Send>> + Send + Sync>;

pub(super) struct RegisteredHandler {
    pub(super) callback: EventHandlerCallback,
    pub(super) interest: EventInterest,
}

pub(super) fn combined_interest(handlers: &[RegisteredHandler]) -> EventInterest {
    handlers.iter().fold(EventInterest::none(), |all, handler| {
        all.union(handler.interest)
    })
}

/// How asynchronous core-event observers receive events. Dispatch never waits
/// for mailbox space or user code. These are lossy observations, not a durable
/// queue: neither overflow nor cancellation implies server redelivery.
///
/// The default buffers 256 events and runs at most 16 callbacks simultaneously.
/// Limits are per handler adapter (one adapter for all bot-builder callbacks).
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub enum EventDelivery {
    /// Explicit opt-in to the historical unbounded spawn-per-callback policy.
    /// No ordering or memory/task bound. Tasks still observe terminal shutdown
    /// and adapter cancellation; synchronous blocking user code is not preemptible.
    ConcurrentUnbounded,
    /// A fixed worker pool; each worker handles one event's interested callbacks
    /// in registration order. Different events may overlap and finish out of order.
    BoundedConcurrent {
        /// Events waiting for a worker, separate from events being processed.
        /// Zero is clamped to one. Overflow drops the newest event for this adapter.
        capacity: usize,
        /// Maximum simultaneous callbacks AND worker tasks. Zero is clamped to one.
        max_concurrency: usize,
    },
    /// One worker processes accepted events in mailbox enqueue order, and their
    /// callbacks in registration order. Concurrent producers have no wall-clock
    /// order guarantee. Overflow/cancellation can leave gaps; no global bus order.
    Ordered {
        /// Waiting events (not including the running event). Zero is clamped to one.
        /// A full mailbox drops the newest event without blocking protocol dispatch.
        capacity: usize,
    },
}

impl Default for EventDelivery {
    fn default() -> Self {
        Self::BoundedConcurrent {
            capacity: 256,
            max_concurrency: 16,
        }
    }
}

/// Approximate, independently read counters for one callback adapter. Completion
/// means the callback returned `()`, not that any external operation succeeded.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct EventDeliveryStats {
    /// Events accepted (for ConcurrentUnbounded, accepted for fanout).
    pub accepted: u64,
    /// Events rejected by a full mailbox; also included in Client::stats().events_dropped.
    pub dropped_full: u64,
    /// Events rejected after cancellation/terminal shutdown/worker loss.
    pub closed: u64,
    /// Accepted queued events discarded without starting their callback sequence.
    pub discarded: u64,
    pub callbacks_started: u64,
    pub callbacks_completed: u64,
    pub callbacks_panicked: u64,
    /// Started callback futures dropped without completion, including executor cancellation.
    pub callbacks_cancelled: u64,
    pub callbacks_active: u64,
}

#[derive(Default)]
struct Counters {
    accepted: AtomicU64,
    dropped_full: AtomicU64,
    closed: AtomicU64,
    discarded: AtomicU64,
    started: AtomicU64,
    completed: AtomicU64,
    panicked: AtomicU64,
    cancelled: AtomicU64,
    active: AtomicU64,
}

struct QueuedEvent {
    event: Option<Arc<Event>>,
    counters: Arc<Counters>,
}

impl Drop for QueuedEvent {
    fn drop(&mut self) {
        if self.event.is_some() {
            self.counters.discarded.fetch_add(1, Ordering::Relaxed);
        }
    }
}

// Closing is not draining in async_channel. The last worker must actually drop
// pending payloads even when the adapter remains registered after shutdown.
struct QueueCleanup(async_channel::Receiver<QueuedEvent>);

impl Drop for QueueCleanup {
    fn drop(&mut self) {
        self.0.close();
        while self.0.try_recv().is_ok() {}
    }
}

struct CallbackGuard {
    counters: Arc<Counters>,
    finished: bool,
}

impl CallbackGuard {
    fn new(counters: Arc<Counters>) -> Self {
        counters.started.fetch_add(1, Ordering::Relaxed);
        counters.active.fetch_add(1, Ordering::Relaxed);
        Self {
            counters,
            finished: false,
        }
    }
}

impl Drop for CallbackGuard {
    fn drop(&mut self) {
        self.counters.active.fetch_sub(1, Ordering::Relaxed);
        if !self.finished {
            self.counters.cancelled.fetch_add(1, Ordering::Relaxed);
        }
    }
}

enum Delivery {
    ConcurrentUnbounded(Arc<[RegisteredHandler]>),
    Queued(async_channel::Sender<QueuedEvent>),
}

/// An asynchronous callback adapter for [`Client::subscribe_handler`]. Keep the
/// returned Subscription: dropping it stops future bus dispatches, but an old
/// bus snapshot or already accepted work may still run. Dropping the last adapter
/// owner cancels its workers and pending work; retaining this Arc keeps it alive
/// after unsubscription. [`Self::cancel`] cancels explicitly even with other owners.
///
/// Terminal Client shutdown also cancels pending/in-flight callbacks, without
/// joining or draining them. Cancellation is cooperative at future poll boundaries;
/// blocking synchronous callbacks and external tasks/Client clones are host-owned.
/// Workers hold only `Weak<Client>` while idle, upgrading for each callback.
///
/// Raw EventHandler implementations still run synchronously on the dispatch path
/// and must not block. Plugin envelopes remain a separate API.
pub struct CallbackEventHandler {
    client: Weak<Client>,
    delivery: Delivery,
    interest: EventInterest,
    stop: ShutdownNotifier,
    shutdown: ShutdownSignal,
    counters: Arc<Counters>,
    workers: Vec<AbortHandle>,
}

impl CallbackEventHandler {
    /// Build an adapter without registering it. Registration remains RAII:
    /// ```no_run
    /// # use whatsapp_rust::{Client, CallbackEventHandler, EventDelivery};
    /// # use whatsapp_rust::types::events::{EventInterest, EventKind};
    /// # fn observe(client: &std::sync::Arc<Client>) {
    /// let handler = CallbackEventHandler::from_callback(
    ///     client, EventInterest::of(&[EventKind::Messages]),
    ///     EventDelivery::BoundedConcurrent { capacity: 64, max_concurrency: 4 },
    ///     |event, client| async move { /* process event without blocking */ },
    /// );
    /// let subscription = client.subscribe_handler(handler.clone());
    /// let stats = handler.stats(); // enqueue/drop/callback outcomes
    /// drop(subscription); // no future bus deliveries; accepted work may finish
    /// handler.cancel(); // explicitly abort accepted work instead of draining
    /// # }
    /// ```
    pub fn from_callback<F, Fut>(
        client: &Arc<Client>,
        interest: EventInterest,
        delivery: EventDelivery,
        callback: F,
    ) -> Arc<Self>
    where
        F: Fn(Arc<Event>, Arc<Client>) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = ()> + Send + 'static,
    {
        Arc::new(Self::new(
            client.clone(),
            vec![RegisteredHandler {
                callback: Arc::new(move |event, client| Box::pin(callback(event, client))),
                interest,
            }],
            delivery,
        ))
    }

    pub(super) fn new(
        client: Arc<Client>,
        handlers: Vec<RegisteredHandler>,
        policy: EventDelivery,
    ) -> Self {
        let interest = combined_interest(&handlers);
        let handlers: Arc<[RegisteredHandler]> = handlers.into();
        let stop = ShutdownNotifier::new();
        let shutdown = client.shutdown_signal();
        let counters = Arc::new(Counters::default());
        let mut workers = Vec::new();
        let delivery = match policy {
            EventDelivery::ConcurrentUnbounded => Delivery::ConcurrentUnbounded(handlers),
            EventDelivery::Ordered { capacity }
            | EventDelivery::BoundedConcurrent { capacity, .. } => {
                let concurrency = match policy {
                    EventDelivery::BoundedConcurrent {
                        max_concurrency, ..
                    } => max_concurrency.max(1),
                    _ => 1,
                };
                let (tx, rx) = async_channel::bounded::<QueuedEvent>(capacity.max(1));
                if interest != EventInterest::none() && !shutdown.is_fired() {
                    for _ in 0..concurrency {
                        let rx = rx.clone();
                        let handlers = handlers.clone();
                        let weak = Arc::downgrade(&client);
                        let cancelled = stop.subscribe();
                        let shutdown = shutdown.clone();
                        let counters = counters.clone();
                        // Construct before spawn so cancellation before first poll
                        // closes/drains the mailbox too.
                        let cleanup = QueueCleanup(rx.clone());
                        workers.push(client.runtime.spawn(Box::pin(async move {
                            let _cleanup = cleanup;
                            loop {
                                let receive = rx.recv().fuse();
                                let cancel = wait_for_shutdown(&cancelled).fuse();
                                let terminal = wait_for_shutdown(&shutdown).fuse();
                                futures::pin_mut!(receive, cancel, terminal);
                                let queued = futures::select_biased! {
                                    _ = cancel => break,
                                    _ = terminal => break,
                                    result = receive => match result { Ok(event) => event, Err(_) => break },
                                };
                                if cancelled.is_fired() || shutdown.is_fired() { break; }
                                if !run_queued_event(&weak, &handlers, queued, &counters, &cancelled, &shutdown).await {
                                    return;
                                }
                            }
                        })));
                    }
                }
                Delivery::Queued(tx)
            }
        };
        Self {
            client: Arc::downgrade(&client),
            delivery,
            interest,
            stop,
            shutdown,
            counters,
            workers,
        }
    }

    /// Abort, not drain. Does not wait for a callback, so reentrant cancellation
    /// or shutdown cannot deadlock on the callback requesting it.
    pub fn cancel(&self) {
        self.stop.notify();
        if let Delivery::Queued(tx) = &self.delivery {
            tx.close();
        }
        for worker in &self.workers {
            worker.abort();
        }
    }

    pub fn stats(&self) -> EventDeliveryStats {
        let c = &self.counters;
        EventDeliveryStats {
            accepted: c.accepted.load(Ordering::Relaxed),
            dropped_full: c.dropped_full.load(Ordering::Relaxed),
            closed: c.closed.load(Ordering::Relaxed),
            discarded: c.discarded.load(Ordering::Relaxed),
            callbacks_started: c.started.load(Ordering::Relaxed),
            callbacks_completed: c.completed.load(Ordering::Relaxed),
            callbacks_panicked: c.panicked.load(Ordering::Relaxed),
            callbacks_cancelled: c.cancelled.load(Ordering::Relaxed),
            callbacks_active: c.active.load(Ordering::Relaxed),
        }
    }
}

impl Drop for CallbackEventHandler {
    fn drop(&mut self) {
        self.cancel();
    }
}

impl EventHandler for CallbackEventHandler {
    fn handle_event(&self, event: Arc<Event>) {
        if !self.interest.wants(event.kind()) {
            return;
        }
        if self.stop.subscribe().is_fired() || self.shutdown.is_fired() {
            self.counters.closed.fetch_add(1, Ordering::Relaxed);
            return;
        }
        match &self.delivery {
            Delivery::ConcurrentUnbounded(handlers) => {
                let Some(client) = self.client.upgrade() else {
                    return;
                };
                self.counters.accepted.fetch_add(1, Ordering::Relaxed);
                for handler in handlers.iter().filter(|h| h.interest.wants(event.kind())) {
                    let handler = RegisteredHandler {
                        callback: handler.callback.clone(),
                        interest: handler.interest,
                    };
                    let event = event.clone();
                    let weak = self.client.clone();
                    let counters = self.counters.clone();
                    let cancelled = self.stop.subscribe();
                    let shutdown = self.shutdown.clone();
                    client.runtime.spawn_detached(Box::pin(async move {
                        run_callback(
                            &weak, &handler, event, &counters, &cancelled, &shutdown, None,
                        )
                        .await;
                    }));
                }
            }
            Delivery::Queued(tx) => {
                let queued = QueuedEvent {
                    event: Some(event),
                    counters: self.counters.clone(),
                };
                match tx.try_send(queued) {
                    Ok(()) => {
                        self.counters.accepted.fetch_add(1, Ordering::Relaxed);
                    }
                    Err(error) => {
                        let full = matches!(&error, async_channel::TrySendError::Full(_));
                        // Rejection is not an accepted-event discard.
                        let mut queued = error.into_inner();
                        queued.event.take();
                        if full {
                            self.counters.dropped_full.fetch_add(1, Ordering::Relaxed);
                            if let Some(client) = self.client.upgrade() {
                                client.stats.record_event_dropped();
                            }
                        } else {
                            self.counters.closed.fetch_add(1, Ordering::Relaxed);
                        }
                    }
                }
            }
        }
    }

    fn interest(&self) -> EventInterest {
        self.interest
    }
}

async fn run_queued_event(
    weak: &Weak<Client>,
    handlers: &[RegisteredHandler],
    mut queued: QueuedEvent,
    counters: &Arc<Counters>,
    cancelled: &ShutdownSignal,
    shutdown: &ShutdownSignal,
) -> bool {
    let Some(event) = queued.event.as_ref().cloned() else {
        return true;
    };
    for handler in handlers.iter().filter(|h| h.interest.wants(event.kind())) {
        if !run_callback(
            weak,
            handler,
            event.clone(),
            counters,
            cancelled,
            shutdown,
            Some(&mut queued),
        )
        .await
        {
            return false;
        }
    }
    true
}

async fn run_callback(
    weak: &Weak<Client>,
    handler: &RegisteredHandler,
    event: Arc<Event>,
    counters: &Arc<Counters>,
    cancelled: &ShutdownSignal,
    shutdown: &ShutdownSignal,
    queued: Option<&mut QueuedEvent>,
) -> bool {
    if cancelled.is_fired() || shutdown.is_fired() {
        return false;
    }
    let Some(client) = weak.upgrade() else {
        return false;
    };
    let callback = async {
        if cancelled.is_fired() || shutdown.is_fired() {
            return false;
        }
        // Dequeue is not callback start: retain the discard guard until this
        // branch is polled, including cancellation before the first callback.
        if let Some(queued) = queued {
            queued.event.take();
        }
        let mut guard = CallbackGuard::new(counters.clone());
        // Future creation is user code too; catch both creation and polling panics.
        let result =
            std::panic::AssertUnwindSafe(async { (handler.callback)(event, client).await })
                .catch_unwind()
                .await;
        guard.finished = true;
        if result.is_ok() {
            counters.completed.fetch_add(1, Ordering::Relaxed);
        } else {
            counters.panicked.fetch_add(1, Ordering::Relaxed);
            log::warn!("event delivery callback panicked; continuing");
        }
        true
    }
    .fuse();
    let cancel = wait_for_shutdown(cancelled).fuse();
    let terminal = wait_for_shutdown(shutdown).fuse();
    futures::pin_mut!(callback, cancel, terminal);
    futures::select_biased! {
        _ = cancel => false,
        _ = terminal => false,
        result = callback => result,
    }
}

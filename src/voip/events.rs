use std::pin::Pin;
use std::task::{Context, Poll};

use super::CallEvent;

/// The owned, single-reader event queue acquired by [`super::CallHandle::take_events`].
///
/// Move it into one consumer task, or consume it as a [`futures::Stream`]. Receiving requires
/// exclusive access; this type is not `Clone` and does not expose its underlying receiver.
/// Cancelling a pending receive consumes no event. Dropping the queue does not hang up the call
/// or restore acquisition. Queued events can be drained after the producer closes.
///
/// Backend capacity and overflow policies still apply. This is neither broadcast nor a durable
/// subscription, and it guarantees neither exactly-once processing nor redelivery. Use
/// [`super::CallHandle::wait_ended`] for sticky completion independent of this queue.
///
/// ```no_run
/// # async fn consume(call: &whatsapp_rust::voip::CallHandle) {
/// let Some(mut events) = call.take_events() else { return };
/// while let Ok(event) = events.recv().await {
///     // Handle the event in this task.
///     let _ = event;
/// }
/// # }
/// ```
///
/// ```compile_fail
/// # fn split(events: whatsapp_rust::voip::CallEvents) {
/// let competing_reader = events.clone();
/// # }
/// ```
///
/// ```compile_fail
/// # async fn shared(events: &whatsapp_rust::voip::CallEvents) {
/// let event = events.recv().await; // requires &mut CallEvents
/// # }
/// ```
///
/// ```compile_fail
/// # fn raw(events: whatsapp_rust::voip::CallEvents) {
/// let receiver: whatsapp_rust::async_channel::Receiver<whatsapp_rust::voip::CallEvent> = events.into();
/// # }
/// ```
pub struct CallEvents {
    // Receiver's Stream implementation is !Unpin. Pin the queue once so the owned wrapper can
    // move between consumer tasks and support StreamExt::next without unsafe projection.
    receiver: Pin<Box<async_channel::Receiver<CallEvent>>>,
}

impl CallEvents {
    pub(super) fn new(receiver: async_channel::Receiver<CallEvent>) -> Self {
        Self {
            receiver: Box::pin(receiver),
        }
    }

    /// Receive the next event, waiting if the queue is empty.
    ///
    /// Returns an error only when the producer has closed and all queued events were consumed.
    /// Cancelling a pending receive leaves the queue available to the next receive.
    pub async fn recv(&mut self) -> Result<CallEvent, async_channel::RecvError> {
        self.receiver.as_ref().get_ref().recv().await
    }

    /// Receive a queued event without waiting, distinguishing an empty queue from a closed one.
    pub fn try_recv(&mut self) -> Result<CallEvent, async_channel::TryRecvError> {
        self.receiver.as_ref().get_ref().try_recv()
    }
}

impl futures::Stream for CallEvents {
    type Item = CallEvent;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.get_mut().receiver.as_mut().poll_next(cx)
    }
}

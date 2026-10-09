//! Admission and cancellation for group-notification effects.

use super::Client;
use std::future::{Future, poll_fn};
use std::pin::pin;
use std::sync::atomic::Ordering;
use std::task::Poll;
use wacore::runtime::{ShutdownSignal, wait_for_shutdown};

#[derive(Clone, Copy)]
pub(crate) struct NotificationScope<'a> {
    generation: u64,
    shutdown: &'a ShutdownSignal,
}

impl<'a> NotificationScope<'a> {
    pub(crate) fn new(generation: u64, shutdown: &'a ShutdownSignal) -> Self {
        Self {
            generation,
            shutdown,
        }
    }

    pub(crate) fn is_current(self, client: &Client) -> bool {
        !self.shutdown.is_fired()
            && client.connection_generation.load(Ordering::Acquire) == self.generation
    }

    /// A task parked on a group lock or storage must not resume into a new
    /// connection. Subscribe to shutdown so even an idle wait is cancelled.
    pub(crate) async fn run<F: Future>(self, client: &Client, future: F) -> Option<F::Output> {
        let mut future = pin!(future);
        let mut shutdown = pin!(wait_for_shutdown(self.shutdown));
        poll_fn(|cx| {
            if !self.is_current(client) || shutdown.as_mut().poll(cx).is_ready() {
                Poll::Ready(None)
            } else {
                future.as_mut().poll(cx).map(Some)
            }
        })
        .await
    }
}

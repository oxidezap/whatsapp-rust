//! Observe one fixture spawn without changing the production dedup guard.

use super::*;
use std::pin::Pin;
use std::task::{Context, Poll};

struct Completed(Option<oneshot::Sender<()>>);

impl Drop for Completed {
    fn drop(&mut self) {
        if let Some(completed) = self.0.take() {
            let _ = completed.send(());
        }
    }
}

struct ObservedRefresh {
    // Declaration order fences cancellation too: drop the real future (and
    // its dedup-release guard) BEFORE announcing terminal completion.
    future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
    _completed: Completed,
    gate: Option<oneshot::Receiver<()>>,
}

impl Future for ObservedRefresh {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        let this = self.get_mut();
        if let Some(gate) = &mut this.gate {
            match Pin::new(gate).poll(cx) {
                Poll::Pending => return Poll::Pending,
                // Cancelling the test's gate drops the unpolled refresh.
                Poll::Ready(Err(_)) => return Poll::Ready(()),
                Poll::Ready(Ok(())) => this.gate = None,
            }
        }
        this.future.as_mut().poll(cx)
    }
}

struct NextRefresh {
    completed: oneshot::Sender<()>,
    gate: Option<oneshot::Receiver<()>>,
}

#[derive(Default)]
pub(super) struct RefreshRuntime {
    next: std::sync::Mutex<Option<NextRefresh>>,
}

impl RefreshRuntime {
    pub(super) fn observe_next(
        &self,
        gate: Option<oneshot::Receiver<()>>,
    ) -> oneshot::Receiver<()> {
        let (completed, completion) = oneshot::channel();
        let mut next = self.next.lock().unwrap();
        assert!(next.is_none(), "only one refresh is observed");
        *next = Some(NextRefresh { completed, gate });
        completion
    }
}

#[async_trait::async_trait]
impl Runtime for RefreshRuntime {
    fn spawn(
        &self,
        future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
    ) -> wacore::runtime::AbortHandle {
        let future = match self.next.lock().unwrap().take() {
            Some(NextRefresh { completed, gate }) => Box::pin(ObservedRefresh {
                future,
                _completed: Completed(Some(completed)),
                gate,
            })
                as Pin<Box<dyn Future<Output = ()> + Send>>,
            None => future,
        };
        crate::runtime_impl::TokioRuntime.spawn(future)
    }

    fn sleep(&self, duration: Duration) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        crate::runtime_impl::TokioRuntime.sleep(duration)
    }

    fn spawn_blocking(
        &self,
        operation: Box<dyn FnOnce() + Send + 'static>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        crate::runtime_impl::TokioRuntime.spawn_blocking(operation)
    }

    fn yield_now(&self) -> Option<Pin<Box<dyn Future<Output = ()> + Send>>> {
        crate::runtime_impl::TokioRuntime.yield_now()
    }
}

pub(super) async fn fixture(name: &str) -> (Arc<Client>, Arc<RefreshRuntime>) {
    let runtime = Arc::new(RefreshRuntime::default());
    let client = crate::test_utils::create_test_client_with_runtime(name, runtime.clone()).await;
    assert!(
        !client.is_socket_connected(),
        "this fixture must exercise refresh failure"
    );
    (client, runtime)
}

pub(super) async fn wait(completion: oneshot::Receiver<()>) {
    // Same five-second real deadline as this SQLite fixture's poll_until;
    // completion is an event, not a yield count or a sleep-based assumption.
    tokio::time::timeout(Duration::from_secs(5), completion)
        .await
        .expect("refresh did not terminate within fixture deadline")
        .expect("refresh completion observer disappeared");
}

#[tokio::test]
async fn yield_limit_does_not_prove_release() {
    let (client, runtime) = fixture("online_device_sync_late").await;
    let jid: Jid = "15550000001@s.whatsapp.net".parse().unwrap();
    let (release, gate) = oneshot::channel();
    let mut completion = runtime.observe_next(Some(gate));
    client
        .schedule_unknown_device_sync(jid.clone(), false)
        .await;
    // Counterfactual: exactly the OLD oracle's yield count, not a larger wait.
    // The registered refresh is legitimately late, not a leaked reservation.
    for _ in 0..1_000 {
        tokio::task::yield_now().await;
    }
    assert!(futures::poll!(&mut completion).is_pending());
    assert_eq!(client.pending_device_sync.len(), 1);
    release.send(()).unwrap();
    wait(completion).await;
    // Disconfirmation: a leaked guard would still fail this exact assertion
    // after the REAL refresh future and its captured guard have been dropped.
    assert_eq!(client.pending_device_sync.len(), 0);
    assert!(client.pending_device_sync.add(&jid));
}

#[tokio::test]
async fn cancelled_unpolled_refresh_notifies_only_after_guard_release() {
    let (client, runtime) = fixture("online_device_sync_cancelled").await;
    let jid: Jid = "15550000002@s.whatsapp.net".parse().unwrap();
    let (cancel, gate) = oneshot::channel();
    let mut completion = runtime.observe_next(Some(gate));
    client.schedule_unknown_device_sync(jid, false).await;
    assert!(futures::poll!(&mut completion).is_pending());
    assert_eq!(client.pending_device_sync.len(), 1);
    drop(cancel);
    wait(completion).await;
    assert_eq!(client.pending_device_sync.len(), 0);
}

#[tokio::test]
async fn offline_reservation_does_not_fake_online_completion() {
    let (client, runtime) = fixture("offline_device_sync_negative").await;
    let jid: Jid = "15550000003@s.whatsapp.net".parse().unwrap();
    let mut completion = runtime.observe_next(None);
    client.schedule_unknown_device_sync(jid, true).await;
    assert!(futures::poll!(&mut completion).is_pending());
    assert_eq!(client.pending_device_sync.len(), 1);
    // No online task was spawned. Cancel only the unused observer, NEVER the
    // production reservation: it correctly stays queued for the offline drain.
    let unused = runtime.next.lock().unwrap().take();
    assert!(unused.is_some());
    drop(unused);
    assert!(
        completion.await.is_err(),
        "unused observer must not announce completion"
    );
    assert_eq!(client.pending_device_sync.len(), 1);
}

//! Finite startup accounting for the connected benchmark, not connection readiness.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Default)]
pub(crate) struct StartupTasks {
    pending: AtomicUsize,
    started: AtomicUsize,
    idle: event_listener::Event,
}

impl StartupTasks {
    /// Register before handing a future to the runtime: an unpolled task or a
    /// task awaiting storage has no IQ waiter but is still startup work.
    pub(crate) fn begin(self: &Arc<Self>) -> StartupTask {
        self.pending.fetch_add(1, Ordering::AcqRel);
        self.started.fetch_add(1, Ordering::Release);
        StartupTask(Arc::clone(self))
    }

    pub(crate) fn pending(&self) -> usize {
        self.pending.load(Ordering::Acquire)
    }

    pub(crate) fn started(&self) -> usize {
        self.started.load(Ordering::Acquire)
    }

    pub(crate) async fn wait(&self) {
        loop {
            let idle = self.idle.listen();
            if self.pending() == 0 {
                return;
            }
            idle.await;
        }
    }
}

pub(crate) struct StartupTask(Arc<StartupTasks>);

impl StartupTask {
    /// The parent stays registered while it creates finite children, so late
    /// child registration cannot expose an intermediate idle state.
    pub(crate) fn child(&self) -> Self {
        self.0.begin()
    }
}

impl Drop for StartupTask {
    fn drop(&mut self) {
        let previous = self.0.pending.fetch_sub(1, Ordering::AcqRel);
        debug_assert!(previous > 0, "startup task underflow");
        if previous == 1 {
            self.0.idle.notify(usize::MAX);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn unpolled_future_is_registered_and_cancellation_releases_it() {
        let tasks = Arc::new(StartupTasks::default());
        let task = tasks.begin();
        let future = async move {
            let _task = task;
            std::future::pending::<()>().await;
        };
        let mut waiter = Box::pin(tasks.wait());
        assert!(futures::poll!(waiter.as_mut()).is_pending());
        assert_eq!(tasks.pending(), 1);
        drop(future);
        waiter.await;
        assert_eq!(tasks.pending(), 0);
    }

    #[tokio::test]
    async fn late_child_outlives_parent_without_premature_quiescence() {
        let tasks = Arc::new(StartupTasks::default());
        let parent = tasks.begin();
        let (release, gated) = async_channel::bounded(1);
        let (child_tx, child_rx) = async_channel::bounded(1);
        let worker = tokio::spawn(async move {
            gated.recv().await.unwrap();
            child_tx.send(parent.child()).await.unwrap();
        });
        let mut waiter = Box::pin(tasks.wait());
        assert!(futures::poll!(waiter.as_mut()).is_pending());
        release.send(()).await.unwrap();
        let child = child_rx.recv().await.unwrap();
        worker.await.unwrap();
        assert_eq!(tasks.pending(), 1);
        assert_eq!(tasks.started(), 2);
        assert!(futures::poll!(waiter.as_mut()).is_pending());
        drop(child);
        waiter.await;
    }

    #[tokio::test]
    async fn aborting_a_polled_task_releases_startup() {
        let tasks = Arc::new(StartupTasks::default());
        let task = tasks.begin();
        let (entered, entry) = async_channel::bounded(1);
        let worker = tokio::spawn(async move {
            let _task = task;
            entered.send(()).await.unwrap();
            std::future::pending::<()>().await;
        });
        entry.recv().await.unwrap();
        let mut waiter = Box::pin(tasks.wait());
        assert!(futures::poll!(waiter.as_mut()).is_pending());
        worker.abort();
        assert!(worker.await.unwrap_err().is_cancelled());
        waiter.await;
        assert_eq!(tasks.pending(), 0);
    }

    #[tokio::test]
    async fn completed_late_startup_remains_observable() {
        let tasks = Arc::new(StartupTasks::default());
        tasks.wait().await;
        let before = tasks.started();
        let task = tasks.begin();
        drop(task);
        assert_eq!(tasks.pending(), 0);
        assert_ne!(tasks.started(), before);
    }

    #[tokio::test]
    async fn all_waiters_and_late_waiters_observe_completion() {
        let tasks = Arc::new(StartupTasks::default());
        let task = tasks.begin();
        let mut first = Box::pin(tasks.wait());
        let mut second = Box::pin(tasks.wait());
        assert!(futures::poll!(first.as_mut()).is_pending());
        assert!(futures::poll!(second.as_mut()).is_pending());
        drop(task);
        first.await;
        second.await;
        tasks.wait().await;
    }

    #[tokio::test]
    async fn cancelling_a_waiter_does_not_release_work() {
        let tasks = Arc::new(StartupTasks::default());
        let task = tasks.begin();
        let mut waiter = Box::pin(tasks.wait());
        assert!(futures::poll!(waiter.as_mut()).is_pending());
        drop(waiter);
        assert_eq!(tasks.pending(), 1);
        drop(task);
        tasks.wait().await;
    }
}

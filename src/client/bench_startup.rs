//! Finite startup accounting for the connected benchmark, not connection readiness.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(Default)]
pub(crate) struct StartupTasks {
    pending: AtomicUsize,
    before_offline: AtomicUsize,
    started: AtomicUsize,
    idle: event_listener::Event,
}

impl StartupTasks {
    /// Register before handing a future to the runtime: an unpolled task or a
    /// task awaiting storage has no IQ waiter but is still startup work.
    pub(crate) fn begin(self: &Arc<Self>) -> StartupTask {
        self.begin_phase(true)
    }

    fn begin_phase(self: &Arc<Self>, before_offline: bool) -> StartupTask {
        self.pending.fetch_add(1, Ordering::AcqRel);
        if before_offline {
            self.before_offline.fetch_add(1, Ordering::AcqRel);
        }
        self.started.fetch_add(1, Ordering::Release);
        StartupTask {
            tasks: Arc::clone(self),
            before_offline,
        }
    }

    pub(crate) fn pending(&self) -> usize {
        self.pending.load(Ordering::Acquire)
    }

    pub(crate) fn started(&self) -> usize {
        self.started.load(Ordering::Acquire)
    }

    pub(crate) fn pending_before_offline(&self) -> usize {
        self.before_offline.load(Ordering::Acquire)
    }

    /// The post-login parent releases this phase before waiting for the offline
    /// marker. Its already-registered passive children still belong to setup.
    pub(crate) async fn wait_before_offline(&self) {
        loop {
            let idle = self.idle.listen();
            if self.pending_before_offline() == 0 {
                return;
            }
            idle.await;
        }
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

pub(crate) struct StartupTask {
    tasks: Arc<StartupTasks>,
    before_offline: bool,
}

impl StartupTask {
    /// The parent stays registered while it creates finite children, so late
    /// child registration cannot expose an intermediate idle state.
    pub(crate) fn child(&self) -> Self {
        self.tasks.begin_phase(self.before_offline)
    }

    pub(crate) fn finish_before_offline(&mut self) {
        if std::mem::take(&mut self.before_offline) {
            let previous = self.tasks.before_offline.fetch_sub(1, Ordering::AcqRel);
            debug_assert!(previous > 0, "startup setup task underflow");
            if previous == 1 {
                self.tasks.idle.notify(usize::MAX);
            }
        }
    }
}

impl Drop for StartupTask {
    fn drop(&mut self) {
        self.finish_before_offline();
        let previous = self.tasks.pending.fetch_sub(1, Ordering::AcqRel);
        debug_assert!(previous > 0, "startup task underflow");
        if previous == 1 {
            self.tasks.idle.notify(usize::MAX);
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

    #[tokio::test]
    async fn setup_joins_passive_child_without_waiting_for_offline_parent() {
        let tasks = Arc::new(StartupTasks::default());
        let mut parent = tasks.begin();
        let passive = parent.child();
        parent.finish_before_offline();
        parent.finish_before_offline();
        let mut setup = Box::pin(tasks.wait_before_offline());
        assert!(futures::poll!(setup.as_mut()).is_pending());
        assert_eq!(tasks.pending_before_offline(), 1);
        drop(passive);
        setup.await;
        assert_eq!(tasks.pending(), 1);
        let mut all = Box::pin(tasks.wait());
        assert!(futures::poll!(all.as_mut()).is_pending());
        let after_offline = parent.child();
        tasks.wait_before_offline().await;
        drop(parent);
        assert!(futures::poll!(all.as_mut()).is_pending());
        drop(after_offline);
        all.await;
    }

    #[tokio::test]
    async fn cancelling_passive_child_releases_setup_and_full_barriers() {
        let tasks = Arc::new(StartupTasks::default());
        let mut parent = tasks.begin();
        let passive = parent.child();
        parent.finish_before_offline();
        let (entered, entry) = async_channel::bounded(1);
        let worker = tokio::spawn(async move {
            let _passive = passive;
            entered.send(()).await.unwrap();
            std::future::pending::<()>().await;
        });
        entry.recv().await.unwrap();
        let mut setup = Box::pin(tasks.wait_before_offline());
        assert!(futures::poll!(setup.as_mut()).is_pending());
        worker.abort();
        assert!(worker.await.unwrap_err().is_cancelled());
        setup.await;
        assert_eq!(tasks.pending(), 1);
        drop(parent);
        tasks.wait().await;
    }
}

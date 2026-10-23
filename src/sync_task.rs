use std::sync::{Arc, Mutex, MutexGuard};

use wacore::appstate::patch_decode::WAPatchName;
use wacore::messages::DetachedHistorySyncNotification;

/// Shared accounting for queued/running history-sync work. Payload bytes are
/// logical compressed lengths: this keeps owned downloads comparable with
/// sliced [`bytes::Bytes`], whose larger shared allocation is not observable.
pub(crate) struct HistorySyncActivity {
    state: Mutex<HistorySyncActivityState>,
    idle_notifier: event_listener::Event,
    /// Shared by the worker and explicit host replay, never reset on reconnect.
    pub(crate) processing_permits: Arc<async_lock::Semaphore>,
    /// Replays take this first, leaving global capacity for the sync worker.
    pub(crate) replay_permits: Arc<async_lock::Semaphore>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct HistorySyncGeneration(usize);

#[derive(Debug, Default)]
struct HistorySyncActivityState {
    generation: HistorySyncGeneration,
    tasks: usize,
    tasks_peak: usize,
    payload_bytes: usize,
    payload_bytes_peak: usize,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct HistorySyncActivitySnapshot {
    pub(crate) tasks: usize,
    pub(crate) tasks_peak: usize,
    pub(crate) payload_bytes: usize,
    pub(crate) payload_bytes_peak: usize,
}

impl HistorySyncActivity {
    pub(crate) fn new() -> Self {
        Self {
            state: Mutex::new(HistorySyncActivityState::default()),
            idle_notifier: event_listener::Event::new(),
            processing_permits: Arc::new(async_lock::Semaphore::new(2)),
            replay_permits: Arc::new(async_lock::Semaphore::new(1)),
        }
    }

    pub(crate) fn begin(self: &Arc<Self>, payload_bytes: usize) -> HistorySyncTaskTracker {
        let mut state = self.state_guard();
        let generation = state.generation;
        state.tasks = state.tasks.saturating_add(1);
        state.tasks_peak = state.tasks_peak.max(state.tasks);
        state.payload_bytes = state.payload_bytes.saturating_add(payload_bytes);
        state.payload_bytes_peak = state.payload_bytes_peak.max(state.payload_bytes);
        drop(state);
        HistorySyncTaskTracker {
            inner: Arc::new(HistorySyncTaskTrackerInner {
                activity: Arc::clone(self),
                generation,
                state: Mutex::new(HistorySyncTaskState {
                    payload_bytes,
                    durable_key: None,
                    processing_permits: None,
                }),
            }),
        }
    }

    pub(crate) fn reset(&self) {
        let mut state = self.state_guard();
        state.generation.0 = state.generation.0.wrapping_add(1);
        state.tasks = 0;
        state.payload_bytes = 0;
        drop(state);
        self.idle_notifier.notify(usize::MAX);
    }

    pub(crate) fn listen(&self) -> event_listener::EventListener {
        self.idle_notifier.listen()
    }

    #[cfg(test)]
    pub(crate) fn idle_notifier(&self) -> &event_listener::Event {
        &self.idle_notifier
    }

    pub(crate) fn tasks(&self) -> usize {
        self.state_guard().tasks
    }

    pub(crate) fn snapshot(&self) -> HistorySyncActivitySnapshot {
        let state = self.state_guard();
        HistorySyncActivitySnapshot {
            tasks: state.tasks,
            tasks_peak: state.tasks_peak,
            payload_bytes: state.payload_bytes,
            payload_bytes_peak: state.payload_bytes_peak,
        }
    }

    fn release(&self, generation: HistorySyncGeneration, payload_bytes: usize) {
        let mut state = self.state_guard();
        if state.generation != generation {
            return;
        }
        state.payload_bytes = state.payload_bytes.saturating_sub(payload_bytes);
        state.tasks = state.tasks.saturating_sub(1);
        let is_idle = state.tasks == 0;
        drop(state);
        if is_idle {
            self.idle_notifier.notify(usize::MAX);
        }
    }

    fn state_guard(&self) -> MutexGuard<'_, HistorySyncActivityState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

#[doc(hidden)]
pub struct HistorySyncTaskTracker {
    // The bounded channel allocates every slot when the Client is built.
    // Keep recovery state out of those empty slots, without changing the
    // public MajorSyncTask variant or its tracker field type.
    inner: Arc<HistorySyncTaskTrackerInner>,
}

struct HistorySyncTaskTrackerInner {
    activity: Arc<HistorySyncActivity>,
    generation: HistorySyncGeneration,
    state: Mutex<HistorySyncTaskState>,
}

struct HistorySyncTaskState {
    payload_bytes: usize,
    // Recovery context follows this one task through the existing public queue
    // shape. It must not be keyed globally by the non-unique stanza ID.
    durable_key: Option<Box<crate::HistorySyncKey>>,
    // A blocking parser/encoder keeps a lease after its async caller is
    // cancelled. Keep both limits in one lease so a detached replay cannot
    // release its replay slot while still occupying global processing capacity.
    processing_permits: Option<Arc<HistorySyncProcessingPermits>>,
}

pub(crate) struct HistorySyncProcessingPermits {
    _processing: async_lock::SemaphoreGuardArc,
    _replay: Option<async_lock::SemaphoreGuardArc>,
}

impl HistorySyncTaskTracker {
    /// Retain this task's activity and capacity until physical CPU work ends.
    pub(crate) fn blocking_lease(&self) -> Self {
        Self {
            inner: Arc::clone(&self.inner),
        }
    }

    fn state(&self) -> MutexGuard<'_, HistorySyncTaskState> {
        self.inner
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub(crate) fn set_processing_permits(
        &mut self,
        processing: async_lock::SemaphoreGuardArc,
        replay: Option<async_lock::SemaphoreGuardArc>,
    ) {
        self.state().processing_permits = Some(Arc::new(HistorySyncProcessingPermits {
            _processing: processing,
            _replay: replay,
        }));
    }

    pub(crate) fn processing_permits(&self) -> Option<Arc<HistorySyncProcessingPermits>> {
        self.state().processing_permits.clone()
    }

    pub(crate) fn clear_processing_permits(&mut self) {
        self.state().processing_permits = None;
    }

    pub(crate) fn set_durable_key(&mut self, key: crate::HistorySyncKey) {
        self.state().durable_key = Some(Box::new(key));
    }

    pub(crate) fn take_durable_key(&mut self) -> Option<Box<crate::HistorySyncKey>> {
        self.state().durable_key.take()
    }

    pub(crate) fn set_payload_bytes(&mut self, payload_bytes: usize) {
        let mut task = self.state();
        let mut state = self.inner.activity.state_guard();
        if state.generation == self.inner.generation {
            state.payload_bytes = state
                .payload_bytes
                .saturating_sub(task.payload_bytes)
                .saturating_add(payload_bytes);
            state.payload_bytes_peak = state.payload_bytes_peak.max(state.payload_bytes);
        }
        task.payload_bytes = payload_bytes;
    }
}

impl std::fmt::Debug for HistorySyncTaskTracker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HistorySyncTaskTracker")
            .field("generation", &self.inner.generation)
            .field("payload_bytes", &self.state().payload_bytes)
            .finish_non_exhaustive()
    }
}

impl Drop for HistorySyncTaskTrackerInner {
    fn drop(&mut self) {
        let state = self
            .state
            .get_mut()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        drop(state.processing_permits.take());
        self.activity.release(self.generation, state.payload_bytes);
    }
}

#[derive(Debug)]
pub enum MajorSyncTask {
    HistorySync {
        message_id: String,
        notification: Box<DetachedHistorySyncNotification>,
        tracker: HistorySyncTaskTracker,
    },
    AppStateSync {
        name: WAPatchName,
        full_sync: bool,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_sync_tracker_updates_payload_peaks_and_releases_exactly_once() {
        let activity = Arc::new(HistorySyncActivity::new());
        let mut first = activity.begin(100);
        let second = activity.begin(200);

        first.set_payload_bytes(150);
        assert_eq!(
            activity.snapshot(),
            HistorySyncActivitySnapshot {
                tasks: 2,
                tasks_peak: 2,
                payload_bytes: 350,
                payload_bytes_peak: 350,
            }
        );

        drop(first);
        assert_eq!(activity.tasks(), 1);
        assert_eq!(activity.snapshot().payload_bytes, 200);

        drop(second);
        assert_eq!(
            activity.snapshot(),
            HistorySyncActivitySnapshot {
                tasks: 0,
                tasks_peak: 2,
                payload_bytes: 0,
                payload_bytes_peak: 350,
            }
        );
    }

    #[test]
    fn queued_history_sync_tracker_cannot_release_current_generation() {
        let activity = Arc::new(HistorySyncActivity::new());
        let mut stale_tracker = activity.begin(1024);
        activity.reset();
        let current_tracker = activity.begin(2048);

        stale_tracker.set_payload_bytes(4096);
        drop(stale_tracker);

        assert_eq!(activity.tasks(), 1);
        assert_eq!(activity.snapshot().payload_bytes, 2048);

        drop(current_tracker);
        assert_eq!(activity.tasks(), 0);
        assert_eq!(activity.snapshot().payload_bytes, 0);
    }
}

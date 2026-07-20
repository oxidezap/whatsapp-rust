use std::sync::{Arc, Mutex, MutexGuard};

use wacore::appstate::patch_decode::WAPatchName;
use wacore::messages::DetachedHistorySyncNotification;

/// Shared accounting for queued/running history-sync work. Payload bytes are
/// logical compressed lengths: this keeps owned downloads comparable with
/// sliced [`bytes::Bytes`], whose larger shared allocation is not observable.
pub(crate) struct HistorySyncActivity {
    state: Mutex<HistorySyncActivityState>,
    idle_notifier: event_listener::Event,
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
            activity: Arc::clone(self),
            generation,
            payload_bytes,
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
    activity: Arc<HistorySyncActivity>,
    generation: HistorySyncGeneration,
    payload_bytes: usize,
}

impl HistorySyncTaskTracker {
    pub(crate) fn set_payload_bytes(&mut self, payload_bytes: usize) {
        let mut state = self.activity.state_guard();
        if state.generation != self.generation {
            self.payload_bytes = payload_bytes;
            return;
        }
        match payload_bytes.cmp(&self.payload_bytes) {
            std::cmp::Ordering::Greater => {
                let additional = payload_bytes - self.payload_bytes;
                state.payload_bytes = state.payload_bytes.saturating_add(additional);
                state.payload_bytes_peak = state.payload_bytes_peak.max(state.payload_bytes);
            }
            std::cmp::Ordering::Less => {
                let released = self.payload_bytes - payload_bytes;
                state.payload_bytes = state.payload_bytes.saturating_sub(released);
            }
            std::cmp::Ordering::Equal => {}
        }
        self.payload_bytes = payload_bytes;
    }
}

impl std::fmt::Debug for HistorySyncTaskTracker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HistorySyncTaskTracker")
            .field("generation", &self.generation)
            .field("payload_bytes", &self.payload_bytes)
            .finish_non_exhaustive()
    }
}

impl Drop for HistorySyncTaskTracker {
    fn drop(&mut self) {
        self.activity.release(self.generation, self.payload_bytes);
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

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use wacore::appstate::patch_decode::WAPatchName;
use wacore::messages::DetachedHistorySyncNotification;

/// Shared accounting for queued/running history-sync work. Payload bytes are a
/// retained-allocation estimate; downloaded buffers contribute their capacity.
pub(crate) struct HistorySyncActivity {
    tasks: AtomicUsize,
    tasks_peak: AtomicUsize,
    payload_bytes: AtomicUsize,
    payload_bytes_peak: AtomicUsize,
    idle_notifier: event_listener::Event,
}

impl HistorySyncActivity {
    pub(crate) fn new() -> Self {
        Self {
            tasks: AtomicUsize::new(0),
            tasks_peak: AtomicUsize::new(0),
            payload_bytes: AtomicUsize::new(0),
            payload_bytes_peak: AtomicUsize::new(0),
            idle_notifier: event_listener::Event::new(),
        }
    }

    pub(crate) fn begin(&self, retained_payload_bytes: usize) {
        let tasks = add_saturating(&self.tasks, 1);
        self.tasks_peak.fetch_max(tasks, Ordering::Relaxed);
        let payload_bytes = add_saturating(&self.payload_bytes, retained_payload_bytes);
        self.payload_bytes_peak
            .fetch_max(payload_bytes, Ordering::Relaxed);
    }

    pub(crate) fn tracker(
        self: &Arc<Self>,
        retained_payload_bytes: usize,
    ) -> HistorySyncTaskTracker {
        HistorySyncTaskTracker {
            activity: Arc::clone(self),
            retained_payload_bytes,
        }
    }

    pub(crate) fn reset(&self) {
        self.tasks.store(0, Ordering::Relaxed);
        self.payload_bytes.store(0, Ordering::Relaxed);
        self.idle_notifier.notify(usize::MAX);
    }

    pub(crate) fn listen(&self) -> event_listener::EventListener {
        self.idle_notifier.listen()
    }

    pub(crate) fn tasks(&self) -> usize {
        self.tasks.load(Ordering::Relaxed)
    }

    pub(crate) fn tasks_peak(&self) -> usize {
        self.tasks_peak.load(Ordering::Relaxed)
    }

    pub(crate) fn payload_bytes(&self) -> usize {
        self.payload_bytes.load(Ordering::Relaxed)
    }

    pub(crate) fn payload_bytes_peak(&self) -> usize {
        self.payload_bytes_peak.load(Ordering::Relaxed)
    }

    fn add_payload_bytes(&self, additional: usize) {
        let total = add_saturating(&self.payload_bytes, additional);
        self.payload_bytes_peak.fetch_max(total, Ordering::Relaxed);
    }

    fn release(&self, retained_payload_bytes: usize) {
        subtract_saturating(&self.payload_bytes, retained_payload_bytes);
        let previous_tasks = self
            .tasks
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |tasks| {
                Some(tasks.saturating_sub(1))
            })
            .unwrap_or_default();
        if previous_tasks <= 1 {
            self.idle_notifier.notify(usize::MAX);
        }
    }
}

pub(crate) struct HistorySyncTaskTracker {
    activity: Arc<HistorySyncActivity>,
    retained_payload_bytes: usize,
}

impl HistorySyncTaskTracker {
    pub(crate) fn set_retained_payload_bytes(&mut self, retained_payload_bytes: usize) {
        match retained_payload_bytes.cmp(&self.retained_payload_bytes) {
            std::cmp::Ordering::Greater => {
                let additional = retained_payload_bytes - self.retained_payload_bytes;
                self.activity.add_payload_bytes(additional);
            }
            std::cmp::Ordering::Less => {
                let released = self.retained_payload_bytes - retained_payload_bytes;
                subtract_saturating(&self.activity.payload_bytes, released);
            }
            std::cmp::Ordering::Equal => {}
        }
        self.retained_payload_bytes = retained_payload_bytes;
    }
}

impl std::fmt::Debug for HistorySyncTaskTracker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HistorySyncTaskTracker")
            .field("retained_payload_bytes", &self.retained_payload_bytes)
            .finish_non_exhaustive()
    }
}

impl Drop for HistorySyncTaskTracker {
    fn drop(&mut self) {
        self.activity.release(self.retained_payload_bytes);
    }
}

fn add_saturating(counter: &AtomicUsize, amount: usize) -> usize {
    let previous = counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            Some(value.saturating_add(amount))
        })
        .unwrap_or_default();
    previous.saturating_add(amount)
}

fn subtract_saturating(counter: &AtomicUsize, amount: usize) {
    let _ = counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
        Some(value.saturating_sub(amount))
    });
}

#[derive(Debug)]
pub enum MajorSyncTask {
    HistorySync {
        message_id: String,
        notification: Box<DetachedHistorySyncNotification>,
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
        activity.begin(100);
        let mut first = activity.tracker(100);
        activity.begin(200);
        let second = activity.tracker(200);

        first.set_retained_payload_bytes(150);
        assert_eq!(activity.payload_bytes(), 350);
        assert_eq!(activity.payload_bytes_peak(), 350);
        assert_eq!(activity.tasks_peak(), 2);

        drop(first);
        assert_eq!(activity.tasks(), 1);
        assert_eq!(activity.payload_bytes(), 200);

        drop(second);
        assert_eq!(activity.tasks(), 0);
        assert_eq!(activity.payload_bytes(), 0);
        assert_eq!(activity.payload_bytes_peak(), 350);
    }

    #[test]
    fn history_sync_tracker_drop_is_saturating_after_connection_cleanup() {
        let activity = Arc::new(HistorySyncActivity::new());
        activity.begin(1024);
        let tracker = activity.tracker(1024);
        activity.reset();

        drop(tracker);

        assert_eq!(activity.tasks(), 0);
        assert_eq!(activity.payload_bytes(), 0);
    }
}

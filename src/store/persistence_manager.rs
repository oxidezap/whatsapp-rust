use super::error::StoreError;
use crate::store::Device;
use crate::store::traits::Backend;
use async_lock::RwLock;
use event_listener::Event;
use futures::FutureExt;
use log::{debug, error};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use wacore::runtime::{AbortHandle, Runtime, ShutdownSignal, wait_for_shutdown};

pub struct PersistenceManager {
    device: Arc<RwLock<Device>>,
    /// Read-mostly snapshot, rebuilt under the device write guard in
    /// `modify_device` so it can never lag a committed mutation. Turns every
    /// `get_device_snapshot` into an Arc refcount bump instead of a full
    /// Device clone (the snapshot is read on every inbound message).
    device_snapshot: std::sync::RwLock<Arc<Device>>,
    backend: Arc<dyn Backend>,
    dirty: Arc<AtomicBool>,
    save_notify: Arc<Event>,
    /// Set to true when the background saver halts due to repeated flush failures.
    saver_halted: Arc<AtomicBool>,
    #[cfg(test)]
    fail_sender_key_device_clears: AtomicBool,
    #[cfg(test)]
    fail_sender_key_device_status_writes: AtomicBool,
}

impl PersistenceManager {
    /// Create a PersistenceManager with a backend implementation.
    ///
    /// Note: The backend should already be configured with the correct device_id
    /// (via SqliteStore::new_for_device for multi-account scenarios).
    pub async fn new(backend: Arc<dyn Backend>) -> Result<Self, StoreError> {
        debug!("PersistenceManager: Ensuring device row exists.");
        // Ensure a device row exists for this backend's device_id; create it if not.
        let exists = backend.exists().await?;
        if !exists {
            debug!("PersistenceManager: No device row found. Creating new device row.");
            let id = backend.create().await?;
            debug!("PersistenceManager: Created device row with id={id}.");
        }

        debug!("PersistenceManager: Attempting to load device data via Backend.");
        let device_data_opt = backend.load().await?;

        let device = if let Some(serializable_device) = device_data_opt {
            debug!(
                "PersistenceManager: Loaded existing device data (PushName: '{}'). Initializing Device.",
                serializable_device.push_name
            );
            let mut dev = Device::new(backend.clone());
            dev.load_from_serializable(serializable_device);
            dev
        } else {
            debug!("PersistenceManager: No data yet; initializing default Device in memory.");
            Device::new(backend.clone())
        };

        let snapshot = Arc::new(device.clone());
        Ok(Self {
            device: Arc::new(RwLock::new(device)),
            device_snapshot: std::sync::RwLock::new(snapshot),
            backend,
            dirty: Arc::new(AtomicBool::new(false)),
            save_notify: Arc::new(Event::new()),
            saver_halted: Arc::new(AtomicBool::new(false)),
            #[cfg(test)]
            fail_sender_key_device_clears: AtomicBool::new(false),
            #[cfg(test)]
            fail_sender_key_device_status_writes: AtomicBool::new(false),
        })
    }

    /// Handle for store adapters that need `&mut Device` trait access.
    /// For plain reads, prefer [`get_device_snapshot`](Self::get_device_snapshot).
    pub async fn get_device_arc(&self) -> Arc<RwLock<Device>> {
        self.device.clone()
    }

    /// Cheap point-in-time view of the device state: an Arc refcount bump,
    /// no locking against writers and no Device clone. Always reflects the
    /// last committed `modify_device`/`process_command` mutation.
    pub fn get_device_snapshot(&self) -> Arc<Device> {
        self.device_snapshot
            .read()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }

    pub fn backend(&self) -> Arc<dyn Backend> {
        self.backend.clone()
    }

    /// Returns true if the background saver halted due to repeated flush failures.
    pub fn is_saver_halted(&self) -> bool {
        self.saver_halted.load(Ordering::Acquire)
    }

    pub async fn modify_device<F, R>(&self, modifier: F) -> R
    where
        F: FnOnce(&mut Device) -> R,
    {
        let mut device_guard = self.device.write().await;
        let result = modifier(&mut device_guard);

        // Dirty BEFORE the snapshot rebuild: a shutdown flush racing this
        // window must see the store dirty, or it would exit clean and drop
        // the committed mutation (the clone below is not free).
        self.dirty.store(true, Ordering::Relaxed);

        // Rebuild while still holding the write guard so no reader can
        // observe post-mutation effects with a pre-mutation snapshot.
        *self
            .device_snapshot
            .write()
            .unwrap_or_else(|p| p.into_inner()) = Arc::new(device_guard.clone());
        drop(device_guard);

        self.save_notify.notify(1);

        result
    }

    /// Flush any dirty device state to the backend immediately.
    pub async fn flush(&self) -> Result<(), StoreError> {
        self.save_to_disk().await
    }

    async fn save_to_disk(&self) -> Result<(), StoreError> {
        if self.dirty.swap(false, Ordering::AcqRel) {
            debug!("Device state is dirty, saving to disk.");
            let device_guard = self.device.read().await;
            let serializable_device = device_guard.to_serializable();
            drop(device_guard);

            if let Err(e) = self.backend.save(&serializable_device).await {
                // Restore dirty flag so the next tick retries the save
                self.dirty.store(true, Ordering::Release);
                return Err(e);
            }
            debug!("Device state saved successfully.");
        }
        Ok(())
    }

    /// Triggers a snapshot of the underlying storage backend.
    /// Useful for debugging critical errors like crypto state corruption.
    pub async fn create_snapshot(
        &self,
        name: &str,
        extra_content: Option<&[u8]>,
    ) -> Result<(), StoreError> {
        #[cfg(feature = "debug-snapshots")]
        {
            // Ensure pending changes are saved first
            self.save_to_disk().await?;
            self.backend.snapshot_db(name, extra_content).await
        }
        #[cfg(not(feature = "debug-snapshots"))]
        {
            let _ = name;
            let _ = extra_content;
            log::warn!("Snapshot requested but 'debug-snapshots' feature is disabled");
            Ok(())
        }
    }

    /// Spawn the background saver. The task wakes on `save_notify`, the
    /// interval tick, or the `shutdown` signal; runs `save_to_disk` after
    /// each wake (no-op when the dirty flag is clear); and performs a final
    /// flush before exiting on shutdown.
    ///
    /// Caller must keep the returned [`AbortHandle`] — dropping it aborts
    /// the task. [`ShutdownSignal`] is sticky (see [`ShutdownNotifier`]):
    /// a notify that races the task's first [`listen()`](event_listener::Event::listen)
    /// is observed via the flag on the first iteration, so no data is stranded.
    pub fn run_background_saver(
        self: Arc<Self>,
        runtime: Arc<dyn Runtime>,
        interval: Duration,
        shutdown: ShutdownSignal,
    ) -> AbortHandle {
        const MAX_CONSECUTIVE_FAILURES: u32 = 10;

        let rt = runtime.clone();
        let weak = Arc::downgrade(&self);
        drop(self);
        debug!("Background saver started (interval {interval:?})");
        runtime.spawn(Box::pin(async move {
            let mut consecutive_failures: u32 = 0;

            // Flush any state dirtied during construction. save_notify is
            // edge-triggered and fires from SetDeviceProps etc. before Bot::build
            // spawns this task, so the dirty flag is our sticky catch for
            // pre-spawn writes.
            if let Some(this) = weak.upgrade()
                && let Err(e) = this.save_to_disk().await
            {
                error!("Background saver: initial flush failed: {e}");
                consecutive_failures = 1;
            }

            loop {
                let Some(this) = weak.upgrade() else {
                    debug!("PersistenceManager dropped, exiting background saver.");
                    return;
                };
                let save_listener = this.save_notify.listen();
                drop(this);

                let should_exit = futures::select! {
                    _ = save_listener.fuse() => false,
                    _ = rt.sleep(interval).fuse() => false,
                    _ = wait_for_shutdown(&shutdown).fuse() => true,
                };

                let Some(this) = weak.upgrade() else {
                    debug!("PersistenceManager dropped, exiting background saver.");
                    return;
                };
                let flush_result = this.save_to_disk().await;

                // On the shutdown path the task is terminating either way; a failed
                // final flush should not permanently flag the store as halted.
                if should_exit {
                    match &flush_result {
                        Err(e) => {
                            error!("Background saver: final flush on shutdown failed: {e}");
                        }
                        Ok(()) => {
                            debug!("Background saver received shutdown; final flush complete.");
                        }
                    }
                    return;
                }

                if let Err(e) = flush_result {
                    consecutive_failures += 1;
                    if consecutive_failures >= MAX_CONSECUTIVE_FAILURES {
                        this.saver_halted.store(true, Ordering::Release);
                        error!(
                            "Background saver: {consecutive_failures} consecutive flush failures, \
                             halting to prevent silent data loss. Last error: {e}"
                        );
                        return;
                    }
                    error!(
                        "Background saver flush failed ({consecutive_failures}/{MAX_CONSECUTIVE_FAILURES}): {e}"
                    );
                } else {
                    consecutive_failures = 0;
                }
            }
        }))
    }
}

use super::commands::{DeviceCommand, apply_command_to_device};

impl PersistenceManager {
    pub async fn process_command(&self, command: DeviceCommand) {
        self.modify_device(|device| {
            apply_command_to_device(device, command);
        })
        .await;
    }
}

impl PersistenceManager {
    pub async fn get_sender_key_devices(
        &self,
        group_jid: &str,
    ) -> Result<Vec<(String, bool)>, StoreError> {
        self.backend.get_sender_key_devices(group_jid).await
    }

    pub async fn set_sender_key_status(
        &self,
        group_jid: &str,
        entries: &[(&str, bool)],
    ) -> Result<(), StoreError> {
        #[cfg(test)]
        if self
            .fail_sender_key_device_status_writes
            .load(Ordering::Acquire)
        {
            return Err(StoreError::Io(std::io::Error::other(
                "injected sender-key tracker status-write failure",
            )));
        }
        self.backend.set_sender_key_status(group_jid, entries).await
    }

    pub async fn clear_sender_key_devices(&self, group_jid: &str) -> Result<(), StoreError> {
        #[cfg(test)]
        if self.fail_sender_key_device_clears.load(Ordering::Acquire) {
            return Err(StoreError::Io(std::io::Error::other(
                "injected sender-key tracker clear failure",
            )));
        }
        self.backend.clear_sender_key_devices(group_jid).await
    }

    #[cfg(test)]
    pub(crate) fn fail_sender_key_device_clears_for_tests(&self, fail: bool) {
        self.fail_sender_key_device_clears
            .store(fail, Ordering::Release);
    }

    #[cfg(test)]
    pub(crate) fn fail_sender_key_device_status_writes_for_tests(&self, fail: bool) {
        self.fail_sender_key_device_status_writes
            .store(fail, Ordering::Release);
    }

    pub async fn delete_sender_key_device_rows(
        &self,
        device_jids: &[&str],
    ) -> Result<(), StoreError> {
        self.backend
            .delete_sender_key_device_rows(device_jids)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime_impl::TokioRuntime;
    use wacore::time::Instant;

    // Saver must observe shutdown.notify, run a final flush, and exit so the
    // AbortHandle-backed task doesn't outlive the Bot.
    #[tokio::test]
    async fn saver_flushes_and_exits_on_shutdown() {
        let backend = crate::test_utils::create_test_backend().await;
        let pm = Arc::new(
            PersistenceManager::new(backend.clone())
                .await
                .expect("pm init"),
        );

        let notifier = wacore::runtime::ShutdownNotifier::new();
        let shutdown_signal = notifier.subscribe();

        let runtime: Arc<dyn Runtime> = Arc::new(TokioRuntime);
        // Interval far in the future so only shutdown can wake the saver.
        let handle =
            pm.clone()
                .run_background_saver(runtime, Duration::from_secs(3600), shutdown_signal);

        // Let the task enter its select before mutating.
        tokio::time::sleep(Duration::from_millis(50)).await;

        pm.modify_device(|d| {
            d.push_name = "shutdown-flush".to_string();
        })
        .await;

        notifier.notify();

        let deadline = Instant::now() + Duration::from_secs(2);
        loop {
            if let Ok(Some(d)) = backend.load().await
                && d.push_name == "shutdown-flush"
            {
                break;
            }
            if Instant::now() > deadline {
                panic!("final flush did not reach backend after shutdown");
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }

        // Dropping the handle must be a no-op when the task already exited.
        drop(handle);
    }

    // Drop of the AbortHandle must actually terminate the task — not merely
    // "not panic." Use the runtime Arc's strong count as the observable:
    // the spawned task captures one reference via `rt = runtime.clone()`,
    // which is released when the task's state machine is dropped.
    #[tokio::test]
    async fn saver_exits_when_abort_handle_dropped_without_signal() {
        let backend = crate::test_utils::create_test_backend().await;
        let pm = Arc::new(PersistenceManager::new(backend).await.expect("pm init"));

        let runtime: Arc<dyn Runtime> = Arc::new(TokioRuntime);
        let baseline = Arc::strong_count(&runtime);

        let handle = pm.clone().run_background_saver(
            Arc::clone(&runtime),
            Duration::from_secs(3600),
            ShutdownSignal::never(),
        );

        tokio::time::sleep(Duration::from_millis(50)).await;
        assert!(
            Arc::strong_count(&runtime) > baseline,
            "running saver should hold a captured runtime Arc"
        );

        drop(handle);

        let deadline = Instant::now() + Duration::from_secs(1);
        while Arc::strong_count(&runtime) > baseline {
            if Instant::now() > deadline {
                panic!(
                    "saver task did not release the runtime Arc within 1s of AbortHandle drop \
                     (strong_count={}, baseline={})",
                    Arc::strong_count(&runtime),
                    baseline
                );
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    // Regression guard for the Client-lifetime-tie fix: storing the saver's
    // AbortHandle inside a struct held by Arc means the handle survives Arc
    // clones and only runs abort when the LAST strong ref drops. If the
    // handle were held by Bot alone, extracting Arc<Client> and dropping
    // Bot would leave the Client without periodic persistence.
    //
    // Tested at the primitive level (Arc<T> + OnceLock<AbortHandle>) because
    // Client's internal detached tasks hold their own strong refs and would
    // keep Client alive regardless. Rust's Drop semantics guarantee the
    // chain Arc::drop -> T::drop -> OnceLock::drop -> AbortHandle::drop.
    #[tokio::test]
    async fn abort_handle_in_arc_drops_only_when_last_ref_released() {
        use std::sync::atomic::{AtomicBool, Ordering};

        struct Owner(std::sync::OnceLock<AbortHandle>);

        let owner = Arc::new(Owner(std::sync::OnceLock::new()));

        let aborted = Arc::new(AtomicBool::new(false));
        let aborted_clone = Arc::clone(&aborted);
        owner
            .0
            .set(AbortHandle::new(move || {
                aborted_clone.store(true, Ordering::SeqCst);
            }))
            .ok()
            .expect("first set");

        let owner_clone = Arc::clone(&owner);
        drop(owner);
        assert!(
            !aborted.load(Ordering::SeqCst),
            "handle must survive while another Arc ref is held"
        );

        drop(owner_clone);
        assert!(
            aborted.load(Ordering::SeqCst),
            "last Arc drop must release the handle and fire abort"
        );
    }
}

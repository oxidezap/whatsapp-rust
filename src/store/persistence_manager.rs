use super::error::StoreError;
use super::release::{BackendLease, StoreRelease};
use crate::store::Device;
use crate::store::traits::Backend;
use async_lock::{Mutex, RwLock, RwLockWriteGuard};
use event_listener::Event;
use futures::FutureExt;
use log::{debug, error};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use wacore::appstate_sync::AppStateMutationPersistence;
use wacore::runtime::{AbortHandle, Runtime, ShutdownSignal, wait_for_shutdown};

#[cfg(not(target_arch = "wasm32"))]
type PendingDeviceSave = wacore::runtime::BoxFuture<'static, Result<(), StoreError>>;
#[cfg(target_arch = "wasm32")]
type PendingDeviceSave =
    send_wrapper::SendWrapper<wacore::runtime::BoxFuture<'static, Result<(), StoreError>>>;

fn pending_device_save(
    future: wacore::runtime::BoxFuture<'static, Result<(), StoreError>>,
) -> PendingDeviceSave {
    #[cfg(target_arch = "wasm32")]
    {
        // Browser stores return local futures; access stays on their origin
        // thread even though the store adapters require Send + Sync handles.
        send_wrapper::SendWrapper::new(future)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        future
    }
}

/// Managed device mutations publish a cached snapshot and schedule persistence.
/// Reading a snapshot does not give access to the manager's writable lock:
///
/// ```compile_fail,E0599
/// async fn raw_device(pm: &whatsapp_rust::store::persistence_manager::PersistenceManager) {
///     let _ = pm.get_device_arc().await;
/// }
/// ```
pub struct PersistenceManager {
    device: Arc<RwLock<Device>>,
    /// Read-mostly snapshot, rebuilt under the device write guard in
    /// `modify_device` so it can never lag a committed mutation. Turns every
    /// `get_device_snapshot` into an Arc refcount bump instead of a full
    /// Device clone (the snapshot is read on every inbound message).
    device_snapshot: std::sync::RwLock<Arc<Device>>,
    dirty: Arc<AtomicBool>,
    /// The manager, not a flush caller, owns an initiated save. Cancellation
    /// releases the driver lock but retains the future, including a backend's
    /// spawned I/O, so the next caller resumes it before writing a newer snapshot.
    pending_save: Mutex<Option<PendingDeviceSave>>,
    save_notify: Arc<Event>,
    /// Set to true when the background saver halts due to repeated flush failures.
    saver_halted: Arc<AtomicBool>,
    #[cfg(test)]
    fail_sender_key_device_clears: AtomicBool,
    #[cfg(test)]
    fail_sender_key_device_status_writes: AtomicBool,
    // Last: mutable/cached Devices and cancellation-retained saves must release
    // their raw backend references before the manager releases its lease.
    backend: BackendLease,
}

/// Publish even when an advanced modifier unwinds or its future is cancelled.
/// The guard never escapes the manager, so no live mutation can bypass this path.
struct DeviceMutation<'a> {
    manager: &'a PersistenceManager,
    device: RwLockWriteGuard<'a, Device>,
}

impl Drop for DeviceMutation<'_> {
    fn drop(&mut self) {
        // Dirty before rebuilding, under the device write guard: a racing flush
        // waits for publication before consuming the dirty flag.
        self.manager.dirty.store(true, Ordering::Relaxed);
        *self
            .manager
            .device_snapshot
            .write()
            .unwrap_or_else(|p| p.into_inner()) = Arc::new(self.device.clone());
        self.manager.save_notify.notify(1);
    }
}

struct DeviceSave {
    // Fields drop in order, even if its driving future was never polled or is
    // cancelled: the raw backend in the snapshot goes before the ownership lease.
    snapshot: Arc<Device>,
    backend: BackendLease,
}

impl DeviceSave {
    async fn run(&self) -> Result<(), StoreError> {
        self.backend.save(&self.snapshot.core).await
    }
}

impl PersistenceManager {
    /// Create a PersistenceManager with a backend implementation.
    ///
    /// Note: The backend should already be configured with the correct device_id
    /// (via SqliteDatabase::store for multi-account scenarios).
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
            debug!("PersistenceManager: Loaded existing device data. Initializing Device.");
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
            backend: BackendLease::new(backend),
            dirty: Arc::new(AtomicBool::new(false)),
            pending_save: Mutex::new(None),
            save_notify: Arc::new(Event::new()),
            saver_halted: Arc::new(AtomicBool::new(false)),
            #[cfg(test)]
            fail_sender_key_device_clears: AtomicBool::new(false),
            #[cfg(test)]
            fail_sender_key_device_status_writes: AtomicBool::new(false),
        })
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

    /// A host-owned handle to the original backend allocation. Such raw handles
    /// are outside [`StoreRelease`]'s crate-ownership boundary.
    pub fn backend(&self) -> Arc<dyn Backend> {
        self.backend.raw()
    }

    /// Observe release without owning this manager or its backend. Sharing this
    /// manager between clients shares the release boundary; drop all manager and
    /// client handles before waiting. Raw backend/Device handles may outlive it.
    pub fn store_release(&self) -> StoreRelease {
        self.backend.observer()
    }

    pub(crate) fn backend_lease(&self) -> BackendLease {
        self.backend.clone()
    }

    /// Returns true if the background saver halted due to repeated flush failures.
    pub fn is_saver_halted(&self) -> bool {
        self.saver_halted.load(Ordering::Acquire)
    }

    /// Mutate managed device state and publish its snapshot, dirty flag and save
    /// notification before returning. Prefer [`Self::process_command`] for named
    /// operations. Unwinding publishes any partial mutation; this is not rollback.
    pub async fn modify_device<F, R>(&self, modifier: F) -> R
    where
        F: FnOnce(&mut Device) -> R,
    {
        let mut mutation = DeviceMutation {
            manager: self,
            device: self.device.write().await,
        };
        modifier(&mut mutation.device)
    }

    /// Advanced adapter access to `&mut Device` across an async trait call,
    /// without exposing the writable lock. The boxed callback keeps the device
    /// borrow inside this operation and preserves native/wasm future conventions.
    ///
    /// On completion, error, panic or cancellation after lock acquisition, any
    /// partial mutation is published and scheduled for saving, not rolled back.
    /// This does not make detached backend I/O transactional or flush it.
    ///
    /// The device write lock is held during the callback. Do not re-enter this
    /// manager (including `flush`) from it. Ordinary Signal operations should use
    /// [`super::signal_adapter::SignalProtocolStoreAdapter`] instead, which reads
    /// snapshots per call without holding the device lock over backend I/O.
    pub async fn modify_device_async<F, R>(&self, modifier: F) -> R
    where
        F: for<'a> FnOnce(&'a mut Device) -> wacore::runtime::BoxFuture<'a, R>,
    {
        let mut mutation = DeviceMutation {
            manager: self,
            device: self.device.write().await,
        };
        modifier(&mut mutation.device).await
    }

    /// Flush any dirty device state to the backend immediately.
    ///
    /// Cancelling the caller retains an initiated save in the manager. The next
    /// flush or background-saver tick resumes it before persisting newer state.
    pub async fn flush(&self) -> Result<(), StoreError> {
        self.save_to_disk().await
    }

    async fn save_to_disk(&self) -> Result<(), StoreError> {
        let mut pending = self.pending_save.lock().await;
        self.finish_device_save(&mut pending).await?;
        // An async modifier may still hold the write guard with dirty=false.
        // Even a clean/final flush must wait for its publication before deciding
        // there is nothing to save. The host must finish/cancel the modifier:
        // device persistence, unlike the client's task drains, is not timed out.
        let device_guard = self.device.read().await;
        if !self.dirty.load(Ordering::Acquire) {
            return Ok(());
        }

        // Consume dirty only after publication under the device write guard.
        if !self.dirty.swap(false, Ordering::AcqRel) {
            return Ok(());
        }
        let snapshot = self.get_device_snapshot();
        drop(device_guard);
        let save = DeviceSave {
            snapshot,
            backend: self.backend_lease(),
        };
        *pending = Some(pending_device_save(Box::pin(
            async move { save.run().await },
        )));
        self.finish_device_save(&mut pending).await
    }

    async fn finish_device_save(
        &self,
        pending: &mut Option<PendingDeviceSave>,
    ) -> Result<(), StoreError> {
        let Some(save) = pending.as_mut() else {
            return Ok(());
        };
        let result = save.await;
        *pending = None;
        if result.is_err() {
            self.dirty.store(true, Ordering::Release);
        }
        result
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
    /// the task. [`ShutdownSignal`] is sticky (see [`ShutdownNotifier`](wacore::runtime::ShutdownNotifier)):
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

    pub(crate) async fn persist_status_privacy(
        &self,
        action: &waproto::whatsapp::sync_action_value::StatusPrivacyAction,
    ) -> Result<(), StoreError> {
        self.process_command(DeviceCommand::SetStatusPrivacy(action.clone()))
            .await;
        self.flush().await
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl AppStateMutationPersistence for PersistenceManager {
    async fn persist_status_privacy(
        &self,
        action: &waproto::whatsapp::sync_action_value::StatusPrivacyAction,
    ) -> Result<(), StoreError> {
        self.persist_status_privacy(action).await
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
    use wacore::store::traits::DeviceStore;
    use wacore::time::Instant;

    #[tokio::test]
    async fn scoped_mutations_notify_saving_even_when_cancelled() {
        let backend = Arc::new(wacore::store::in_memory::InMemoryBackend::new());
        let pm = PersistenceManager::new(backend.clone()).await.unwrap();
        let mut notified = Box::pin(pm.save_notify.listen());
        assert!(futures::poll!(notified.as_mut()).is_pending());
        pm.modify_device(|device| device.push_name = "sync".into())
            .await;
        assert!(futures::poll!(notified.as_mut()).is_ready());
        assert!(pm.dirty.load(Ordering::Acquire));
        pm.flush().await.unwrap();

        let mut notified = Box::pin(pm.save_notify.listen());
        let mut mutation = Box::pin(pm.modify_device_async(|device| {
            Box::pin(async move {
                device.push_name = "cancelled".into();
                std::future::pending::<()>().await;
            })
        }));
        assert!(futures::poll!(mutation.as_mut()).is_pending());
        assert!(futures::poll!(notified.as_mut()).is_pending());
        drop(mutation);
        assert!(futures::poll!(notified.as_mut()).is_ready());
        assert!(pm.dirty.load(Ordering::Acquire));
        pm.flush().await.unwrap();
        assert_eq!(
            backend.load().await.unwrap().unwrap().push_name,
            "cancelled"
        );
    }

    #[tokio::test]
    async fn audit_cancelled_flush_must_preserve_dirty_device() {
        let backend = Arc::new(wacore::store::in_memory::InMemoryBackend::new());
        let pm = PersistenceManager::new(backend.clone()).await.unwrap();
        pm.process_command(DeviceCommand::SetPushName("before".into()))
            .await;
        pm.flush().await.unwrap();
        pm.process_command(DeviceCommand::SetPushName("after".into()))
            .await;

        // Block snapshot acquisition without mutating Device; cancellation
        // must not consume the dirty state while waiting for this guard.
        let guard = pm.device.write().await;
        let mut flush = Box::pin(pm.flush());
        assert!(futures::poll!(flush.as_mut()).is_pending());
        drop(flush);
        drop(guard);

        pm.flush().await.unwrap();
        assert_eq!(backend.load().await.unwrap().unwrap().push_name, "after");
    }

    #[tokio::test]
    async fn audit_overlapping_flush_must_wait_for_durable_device() {
        let backend = Arc::new(wacore::store::in_memory::InMemoryBackend::new());
        let pm = PersistenceManager::new(backend.clone()).await.unwrap();
        pm.process_command(DeviceCommand::SetPushName("before".into()))
            .await;
        pm.flush().await.unwrap();
        pm.process_command(DeviceCommand::SetPushName("after".into()))
            .await;

        let guard = pm.device.write().await;
        let mut first = Box::pin(pm.flush());
        assert!(futures::poll!(first.as_mut()).is_pending());
        assert_eq!(backend.load().await.unwrap().unwrap().push_name, "before");
        let mut second = Box::pin(pm.flush());
        assert!(
            futures::poll!(second.as_mut()).is_pending(),
            "flush returned success while the newer device snapshot was not durable"
        );
        drop(guard);
        first.await.unwrap();
        second.await.unwrap();
    }

    #[tokio::test]
    async fn cancelled_driver_resumes_pending_save_before_newer_snapshot() {
        let backend = Arc::new(wacore::store::in_memory::InMemoryBackend::new());
        let pm = PersistenceManager::new(backend.clone()).await.unwrap();
        pm.process_command(DeviceCommand::SetPushName("before".into()))
            .await;
        pm.flush().await.unwrap();

        let snapshot = pm.get_device_snapshot();
        let saved = backend.clone();
        let (entered_tx, entered_rx) = async_channel::bounded(1);
        let (release_tx, release_rx) = async_channel::bounded(1);
        *pm.pending_save.lock().await = Some(pending_device_save(Box::pin(async move {
            entered_tx.send(()).await.unwrap();
            release_rx.recv().await.unwrap();
            saved.save(&snapshot.core).await
        })));
        pm.process_command(DeviceCommand::SetPushName("after".into()))
            .await;

        let mut first = Box::pin(pm.flush());
        assert!(futures::poll!(first.as_mut()).is_pending());
        entered_rx.try_recv().unwrap();
        drop(first);
        assert!(
            !release_tx.is_closed(),
            "cancellation dropped the save future"
        );

        let mut next = Box::pin(pm.flush());
        assert!(futures::poll!(next.as_mut()).is_pending());
        release_tx.try_send(()).unwrap();
        next.await.unwrap();
        assert_eq!(backend.load().await.unwrap().unwrap().push_name, "after");
        assert!(pm.pending_save.lock().await.is_none());
        assert!(!pm.dirty.load(Ordering::Acquire));
    }

    #[tokio::test]
    async fn failed_pending_save_retries_latest_snapshot() {
        let backend = Arc::new(wacore::store::in_memory::InMemoryBackend::new());
        let pm = PersistenceManager::new(backend.clone()).await.unwrap();
        pm.process_command(DeviceCommand::SetPushName("after".into()))
            .await;
        *pm.pending_save.lock().await = Some(pending_device_save(Box::pin(async {
            Err(StoreError::Io(std::io::Error::other(
                "injected save failure",
            )))
        })));
        assert!(pm.flush().await.is_err());
        assert!(pm.dirty.load(Ordering::Acquire));
        assert!(pm.pending_save.lock().await.is_none());
        pm.flush().await.unwrap();
        assert_eq!(backend.load().await.unwrap().unwrap().push_name, "after");
    }

    #[tokio::test]
    async fn status_privacy_persistence_survives_manager_restart() {
        let backend = Arc::new(wacore::store::in_memory::InMemoryBackend::new());
        let pm = PersistenceManager::new(backend.clone()).await.unwrap();
        let action = waproto::whatsapp::sync_action_value::StatusPrivacyAction {
            mode: Some(buffa::EnumValue::Unknown(99)),
            user_jid: vec!["120363000000000042@lid".into()],
            ..Default::default()
        };
        pm.persist_status_privacy(&action).await.unwrap();

        let restarted = PersistenceManager::new(backend).await.unwrap();
        assert_eq!(
            restarted.get_device_snapshot().status_privacy.as_deref(),
            Some(&action)
        );
    }

    #[tokio::test]
    async fn last_client_drop_does_not_release_a_parked_saver_flush() {
        assert_saver_retains_backend(true).await;
    }

    #[tokio::test]
    async fn cancelled_saver_releases_only_when_its_future_is_destroyed() {
        assert_saver_retains_backend(false).await;
    }

    async fn assert_saver_retains_backend(complete: bool) {
        use crate::store::release::tests::{ProbeBackend, SaverRuntime};
        let (mut backend, dropped) = ProbeBackend::new();
        let (unpark, parked) = async_channel::bounded(1);
        let (entered, entry) = async_channel::bounded(1);
        backend.gate = Some(parked);
        backend.entered = Some(entered);
        let client = crate::test_utils::create_test_client_with_backend(Arc::new(backend)).await;
        let weak = Arc::downgrade(&client);
        let pm = client.persistence_manager();
        let release = client.store_release();
        let mut waiter = Box::pin(release.wait());
        let runtime = Arc::new(SaverRuntime::new());
        let notifier = wacore::runtime::ShutdownNotifier::new();
        let handle = pm.clone().run_background_saver(
            runtime.clone(),
            Duration::from_secs(3600),
            notifier.subscribe(),
        );
        assert!(client.saver_handle.set(handle).is_ok());
        let mut saver = runtime.future.lock().unwrap().take().unwrap();
        assert!(futures::poll!(saver.as_mut()).is_pending());

        // A save retained after driver cancellation is real manager state.
        // The clean saver is parked in select; only shutdown wakes it, so this
        // resumes inside the final flush (not the initial or periodic flush).
        let save = DeviceSave {
            snapshot: pm.get_device_snapshot(),
            backend: pm.backend_lease(),
        };
        *pm.pending_save.lock().await =
            Some(pending_device_save(Box::pin(
                async move { save.run().await },
            )));
        notifier.notify();
        assert!(futures::poll!(saver.as_mut()).is_pending());
        entry.try_recv().unwrap();
        drop(pm);
        drop(client);
        crate::test_utils::poll_until("last client reference", || weak.upgrade().is_none()).await;
        assert!(runtime.aborted.load(Ordering::SeqCst));
        assert!(
            !dropped.load(Ordering::SeqCst),
            "saver retains backend after Client Drop"
        );
        assert!(futures::poll!(waiter.as_mut()).is_pending());
        if complete {
            unpark.send(()).await.unwrap();
            saver.await;
        } else {
            drop(saver); // executor acknowledgement, not merely the abort request
        }
        tokio::time::timeout(Duration::from_secs(5), waiter)
            .await
            .unwrap();
        assert!(dropped.load(Ordering::SeqCst));
        release.wait().await;
    }

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

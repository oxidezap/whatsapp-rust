//! Signal/sender-key store adapters, per-session locks and noise socket access.

use super::*;

impl Client {
    /// Build a [`SignalProtocolStoreAdapter`] from the current device state and signal cache.
    pub(crate) async fn signal_adapter(
        &self,
    ) -> crate::store::signal_adapter::SignalProtocolStoreAdapter {
        let device_store = self.persistence_manager.get_device_arc().await;
        self.signal_adapter_from(device_store)
    }

    /// Build a standalone [`SenderKeyAdapter`] from the current device state and
    /// signal cache, avoiding the full five-store adapter on the SKDM path.
    pub(crate) async fn sender_key_adapter(
        &self,
    ) -> crate::store::signal_adapter::SenderKeyAdapter {
        crate::store::signal_adapter::SenderKeyAdapter::new(
            self.persistence_manager.get_device_arc().await,
            self.signal_cache.clone(),
        )
    }

    /// Build a [`SignalProtocolStoreAdapter`] from a pre-fetched device arc.
    pub(crate) fn signal_adapter_from(
        &self,
        device_store: Arc<async_lock::RwLock<crate::store::Device>>,
    ) -> crate::store::signal_adapter::SignalProtocolStoreAdapter {
        crate::store::signal_adapter::SignalProtocolStoreAdapter::new(
            device_store,
            self.signal_cache.clone(),
        )
    }

    /// Get the per-address session mutex from the lock cache.
    pub(crate) async fn session_lock_for(
        &self,
        signal_addr_str: &str,
    ) -> Arc<async_lock::Mutex<()>> {
        self.session_locks
            .get_with_by_ref(signal_addr_str, async {
                Arc::new(async_lock::Mutex::new(()))
            })
            .await
    }

    /// Acquire the per-group cold-distribution single-flight guard.
    pub(crate) async fn group_distribution_lock(
        &self,
        group: &Jid,
    ) -> async_lock::MutexGuardArc<()> {
        self.group_distribution_locks
            .get_with_by_ref(group, async { Arc::new(async_lock::Mutex::new(())) })
            .await
            .lock_arc()
            .await
    }

    /// Get the active noise socket, or error if not connected.
    pub(crate) async fn get_noise_socket(
        &self,
    ) -> Result<Arc<crate::socket::noise_socket::NoiseSocket>, ClientError> {
        self.noise_socket
            .lock()
            .await
            .clone()
            .ok_or(ClientError::NotConnected)
    }

    /// Flush the in-memory signal cache to the database backend.
    /// Called after each message is decrypted or after encryption operations.
    pub(crate) async fn flush_signal_cache(&self) -> Result<(), anyhow::Error> {
        // Hold no device guard across the flush: this per-message batched SQLite
        // write would otherwise block every concurrent Device write for its duration.
        let backend = self
            .persistence_manager
            .get_device_snapshot()
            .backend
            .clone();
        self.signal_cache
            .flush(&*backend)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to flush signal cache: {e}"))?;
        // Single choke point every flush goes through: state retained by a
        // failed teardown flush is persisted now, so the connect-time clear
        // may resume on future connections.
        self.signal_cache_retained_dirty
            .store(false, std::sync::atomic::Ordering::Release);
        Ok(())
    }

    /// [`flush_signal_cache`](Self::flush_signal_cache) with error logging instead of propagation.
    ///
    /// Both of these are safe only when the caller holds the message
    /// processing permit or the batcher is known inactive: they persist the
    /// WHOLE cache, including ratchet advances of drain entries that may not
    /// have a durable buffered row yet. Everything else must go through the
    /// `_batch_safe` variants below.
    pub(crate) async fn flush_signal_cache_logged(&self, context: &str, id: Option<&str>) {
        if let Err(e) = self.flush_signal_cache().await {
            log_signal_flush_error(context, id, &e);
        }
    }

    /// Signal-cache flush that is safe while the offline drain is active.
    ///
    /// During the drain, decrypted messages accumulate in the commit batcher
    /// with no durable buffered copy; flushing the cache from an unrelated
    /// path (a retry receipt, a send, an identity change) would persist their
    /// ratchet advances, and a crash/teardown that then drops the entries
    /// turns each redelivery into an ackable duplicate — silent loss for hook
    /// consumers. So in drain mode this routes through the batcher: commit
    /// the pending entries (rows first) and flush under the processing
    /// permit. Outside the drain it is exactly [`Self::flush_signal_cache`].
    ///
    /// Must NOT be called while holding the processing permit (it acquires
    /// it); permit-holding paths commit via the batcher directly.
    pub(crate) async fn flush_signal_cache_batch_safe(&self) -> Result<(), anyhow::Error> {
        if self.inbound_commit_batch.is_active() {
            if let Some(client) = self.self_weak.get().and_then(|w| w.upgrade()) {
                return if client
                    .flush_inbound_commits_under_permit(false, None, None)
                    .await
                {
                    Ok(())
                } else {
                    Err(anyhow::anyhow!(
                        "inbound drain batch commit failed; Signal cache left unflushed so the server redelivers"
                    ))
                };
            }
            if self.inbound_commit_batch.has_entries() {
                return Err(anyhow::anyhow!(
                    "client dropping with uncommitted drain entries; skipping Signal flush"
                ));
            }
        }
        self.flush_signal_cache().await
    }

    /// [`flush_signal_cache_batch_safe`](Self::flush_signal_cache_batch_safe)
    /// with error logging instead of propagation.
    pub(crate) async fn flush_signal_cache_batch_safe_logged(
        &self,
        context: &str,
        id: Option<&str>,
    ) {
        if let Err(e) = self.flush_signal_cache_batch_safe().await {
            log_signal_flush_error(context, id, &e);
        }
    }
}

fn log_signal_flush_error(context: &str, id: Option<&str>, e: &anyhow::Error) {
    if let Some(id) = id {
        log::error!("Failed to flush signal cache ({context} {id}): {e:?}");
    } else {
        log::error!("Failed to flush signal cache ({context}): {e:?}");
    }
}

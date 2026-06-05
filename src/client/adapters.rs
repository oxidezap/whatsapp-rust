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
        let device = self.persistence_manager.get_device_arc().await;
        let device_guard = device.read().await;
        self.signal_cache
            .flush(&*device_guard.backend)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to flush signal cache: {e}"))
    }

    /// [`flush_signal_cache`](Self::flush_signal_cache) with error logging instead of propagation.
    pub(crate) async fn flush_signal_cache_logged(&self, context: &str, id: Option<&str>) {
        if let Err(e) = self.flush_signal_cache().await {
            if let Some(id) = id {
                log::error!("Failed to flush signal cache ({context} {id}): {e:?}");
            } else {
                log::error!("Failed to flush signal cache ({context}): {e:?}");
            }
        }
    }
}

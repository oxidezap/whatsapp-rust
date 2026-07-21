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

    /// Force any pending write-behind Signal cache state to the backend,
    /// returning once the flush completes (or fails).
    ///
    /// The live receive path schedules a coalesced flush (see `signal_flush.rs`)
    /// instead of writing through, and lease-covered sends do the same. Only a
    /// send that raises a session or sender-key counter lease flushes
    /// synchronously. On success, the
    /// backend normally trails the cache by about the coalescing window, but
    /// that is not a hard wall-clock bound — the timer can slip under runtime
    /// starvation and the flush can wait on locks or slow/failing storage (a
    /// backend outage extends it until the retry loop succeeds). Use this to
    /// settle durability deterministically before reading persisted state or
    /// ahead of a non-graceful shutdown — and check the returned `Result`, as a
    /// failure leaves state pending.
    ///
    /// Call from a control task, never from inside an event handler or an
    /// [`InboundDurabilityHook`]: during an offline-sync drain those run while
    /// the processing permit is held, and settling routes through that same
    /// permit — re-entering it would deadlock.
    ///
    /// [`InboundDurabilityHook`]: crate::types::durability_hook::InboundDurabilityHook
    pub async fn flush_pending_signal_state(&self) -> Result<(), anyhow::Error> {
        self.flush_signal_cache_batch_safe().await
    }

    /// Flush the in-memory signal cache to the database backend. Invoked by the
    /// send path (synchronously, pre-wire), the receive-path coalescer, and the
    /// drain/retry/teardown recovery paths — not unconditionally per message.
    pub(crate) async fn flush_signal_cache(&self) -> Result<(), anyhow::Error> {
        // Clone the backend before awaiting so a slow write cannot retain the
        // device guard and stall every concurrent Device write.
        let backend = self
            .persistence_manager
            .get_device_snapshot()
            .backend
            .clone();
        self.signal_cache
            .flush(&*backend)
            .await
            .map_err(|error| error.context("Failed to flush signal cache"))
    }

    /// Signal-cache flush that is safe while the offline drain is active.
    ///
    /// [`flush_signal_cache`](Self::flush_signal_cache) is safe only when the
    /// caller holds the message processing permit or the batcher is known
    /// inactive: it persists the WHOLE cache, including ratchet advances of
    /// drain entries that may not have a durable buffered row yet. Everything
    /// else must go through this `_batch_safe` variant.
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
        // Under the permit the commit ALWAYS flushes the Signal cache (an empty
        // batch still flushes — see flush_inbound_commits_under_permit), so a
        // successful call is proof the out-of-band advance is persisted; no
        // stale is_active() re-check needed even if the drain finisher
        // deactivated while we waited for the permit.
        let drain_active = self.inbound_commit_batch.is_active();
        if drain_active && let Some(client) = self.self_weak.get().and_then(|w| w.upgrade()) {
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
        } else if drain_active {
            // Drain active but no live client to route the permit-held flush
            // through (practically unreachable — the run loop holds a strong
            // Arc). Fail closed regardless of has_entries(): an empty drain can
            // still carry dirty SKDM-only advances with no rows, and a raw
            // flush here would persist them rowless — the exact loss this
            // batch-safe path exists to prevent. Leaving the cache unflushed
            // makes the server redeliver.
            return Err(anyhow::anyhow!(
                "client dropping while inbound drain is active; skipping Signal flush"
            ));
        }
        self.flush_signal_cache().await
    }

    /// Pre-wire durability gate for the send path. Flushes synchronously only
    /// when an outbound crypto advance actually demands it — a raised session
    /// counter lease or a raised sender-key lease not yet persisted (see
    /// `SignalStoreCache::needs_pre_wire_flush`). Otherwise the dirty state is
    /// covered by an existing durable lease, so it only needs to land
    /// eventually: it rides the same coalesced write-behind as the receive
    /// path instead of paying a serialize + storage transaction per message.
    /// A failure must abort the send — transmitting a ciphertext whose lease
    /// could not be persisted reintroduces the counter-reuse window.
    ///
    /// The gate is deliberately a GLOBAL predicate, not one scoped to the
    /// addresses this stanza encrypted for: the send path does not carry them
    /// back up, and erring toward an extra flush is the safe direction (it can
    /// over-flush, never under-flush). The cost is a sharp edge under a failing
    /// backend: another session's pending lease makes this send flush, and that
    /// flush's failure aborts this send too — the same collateral every send
    /// took before leases existed, now limited to the window where some lease
    /// is actually unpersisted. So "a lease-covered send needs no flush" is a
    /// statement about what this send REQUIRES, not a promise it never flushes.
    pub(crate) async fn persist_signal_state_pre_wire(&self) -> Result<(), anyhow::Error> {
        if self.signal_cache.needs_pre_wire_flush().await {
            return self.flush_signal_cache_batch_safe().await;
        }
        self.schedule_signal_flush(
            self.connection_generation
                .load(std::sync::atomic::Ordering::Acquire),
        );
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;
    use wacore::store::in_memory::InMemoryBackend;
    use wacore_binary::{Jid, Server};

    #[tokio::test]
    async fn signal_flush_context_preserves_the_backend_error_chain() {
        let backend = Arc::new(InMemoryBackend::new());
        let client = crate::test_utils::create_test_client_with_backend(backend.clone()).await;
        let peer = Jid::new("15550001111", Server::Pn).with_device(1);
        crate::test_utils::seed_peer_session(&client, &peer).await;
        backend.set_fail_session_writes(true);

        let error = client
            .flush_signal_cache()
            .await
            .expect_err("injected backend failure must propagate");
        let chain: Vec<String> = error.chain().map(ToString::to_string).collect();

        assert_eq!(
            chain.first().map(String::as_str),
            Some("Failed to flush signal cache")
        );
        assert!(
            chain
                .iter()
                .any(|cause| cause.contains("put_sessions_batch failing (test hook)")),
            "typed backend cause missing from {chain:?}"
        );
    }
}

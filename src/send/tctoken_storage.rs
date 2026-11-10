use crate::client::Client;
use wacore_binary::Jid;

pub(crate) struct TcTokenStorageOutcome {
    pub(crate) key: Jid,
    pub(crate) changed: bool,
}

impl Client {
    /// Resolve and merge under migration's existing fence. The owned worker
    /// retains that fence through every ACK even if its requester is dropped.
    /// Callers must pass the original JID or explicit sender LID, never a PN
    /// storage key selected before acquiring the fence. No network work belongs
    /// in this scope; migration itself uses its already-guarded backend calls.
    pub(crate) async fn persist_tc_token_owned(
        &self,
        jid: Jid,
        received: Option<(Vec<u8>, i64)>,
        sender_timestamp: Option<i64>,
        notification: bool,
    ) -> anyhow::Result<TcTokenStorageOutcome> {
        let client = self
            .self_weak
            .get()
            .and_then(|weak| weak.upgrade())
            .ok_or_else(|| anyhow::anyhow!("client ownership unavailable during token storage"))?;
        let (send, receive) = futures::channel::oneshot::channel();
        self.runtime.spawn_detached(Box::pin(async move {
            let observed = jid.clone();
            let result = client
                .persist_tc_token_inner(jid, received, sender_timestamp, notification)
                .await;
            if let Err(Err(error)) = send.send(result) {
                log::warn!(target: "Client/TcToken", "Owned token storage for {} failed after caller cancellation: {error:#}", observed.observe());
            }
        }));
        receive
            .await
            .map_err(|_| anyhow::anyhow!("token storage worker stopped"))?
    }

    async fn persist_tc_token_inner(
        &self,
        mut jid: Jid,
        received: Option<(Vec<u8>, i64)>,
        sender_timestamp: Option<i64>,
        notification: bool,
    ) -> anyhow::Result<TcTokenStorageOutcome> {
        let _mapping = self.lid_pn_cache.lock_mutation().await;
        if !jid.is_lid()
            && let Some(lid) = self.lid_pn_cache.get_current_lid(&jid.user).await
        {
            jid = Jid::lid(lid);
        }
        let backend = self.persistence_manager.backend();
        let mut changed = false;
        if let Some((token, timestamp)) = received {
            let previous = if notification {
                backend.get_tc_token(&jid.user).await?
            } else {
                None
            };
            if let Some(previous) = previous.as_ref().filter(|old| old.token == token) {
                if timestamp > previous.token_timestamp {
                    backend
                        .store_received_tc_token(&jid.user, &token, timestamp)
                        .await?;
                }
            } else if previous
                .as_ref()
                .is_none_or(|old| old.token.is_empty() || timestamp >= old.token_timestamp)
            {
                backend
                    .store_received_tc_token(&jid.user, &token, timestamp)
                    .await?;
                changed = true;
            }
        }
        if let Some(timestamp) = sender_timestamp {
            backend
                .touch_tc_token_sender_timestamp(&jid.user, timestamp)
                .await?;
        }
        Ok(TcTokenStorageOutcome { key: jid, changed })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lid_pn_cache::LearningSource;
    use crate::store::release::tests::ProbeBackend;
    use crate::test_utils::create_iq_test_client_with_backend;
    use std::sync::Arc;
    use std::time::Duration;

    #[tokio::test]
    async fn token_writer_resolves_the_original_jid_after_mapping_fence_admission() {
        for received in [false, true] {
            let (client, _) = crate::test_utils::create_iq_test_client().await;
            tokio::time::timeout(Duration::from_secs(5), async {
                while !client.lid_pn_cache.startup_finished_for_tests() {
                    tokio::task::yield_now().await;
                }
            })
            .await
            .unwrap();
            let phone = Jid::pn("12025550191");
            let lid = Jid::lid("100000000000191");
            let now = client.runtime.wall_time_secs();
            let backend = client.persistence_manager.backend();
            backend
                .store_received_tc_token(&lid.user, b"synthetic-existing-token", now - 1)
                .await
                .unwrap();
            let mapping = client.lid_pn_cache.lock_mutation().await;
            // Drive the same inner future retained by the owned worker. The
            // pending poll proves it has reached fence admission; changing the
            // fixture now catches resolving the original PN before that fence.
            let mut writer = std::pin::pin!(client.persist_tc_token_inner(
                phone.clone(),
                received.then(|| (b"synthetic-new-token".to_vec(), now)),
                Some(now),
                received,
            ));
            assert!(futures::poll!(&mut writer).is_pending());
            let entry = crate::lid_pn_cache::LidPnEntry::with_timestamp(
                lid.user.as_str(),
                phone.user.as_str(),
                now,
                LearningSource::Usync,
            );
            backend
                .put_lid_mapping(&wacore::store::traits::LidPnMappingEntry {
                    lid: lid.user.to_string(),
                    phone_number: phone.user.to_string(),
                    created_at: now,
                    updated_at: now,
                    learning_source: "usync".into(),
                })
                .await
                .unwrap();
            client.lid_pn_cache.add_guarded(&entry, &mapping).await;
            client
                .lid_pn_cache
                .mark_persisted(&entry.phone_number, &entry.lid)
                .await;
            drop(mapping);
            let outcome = writer.await.unwrap();
            assert_eq!(outcome.key, lid);
            assert!(backend.get_tc_token(&phone.user).await.unwrap().is_none());
            let saved = backend.get_tc_token(&lid.user).await.unwrap().unwrap();
            assert_eq!(saved.sender_timestamp, Some(now));
            assert_eq!(
                saved.token,
                if received {
                    b"synthetic-new-token".as_slice()
                } else {
                    b"synthetic-existing-token".as_slice()
                }
            );
            client.signal_shutdown_sync();
        }
    }

    #[tokio::test]
    async fn adv_review_round3_token_writer_keeps_ownership_after_cancellation() {
        for (received, existing_lid) in [(false, false), (false, true), (true, false)] {
            let (mut probe, _) = ProbeBackend::new();
            let raw = probe.backend.clone();
            let (release, gate) = async_channel::unbounded();
            let (entered, arrived) = async_channel::unbounded();
            probe.gate_method = if received {
                "after:store_received_tc_token"
            } else {
                "after:touch_tc_token_sender_timestamp"
            };
            probe.gate_once = true;
            probe.gate = Some(gate);
            probe.entered = Some(entered);
            let (client, _) = create_iq_test_client_with_backend(Arc::new(probe)).await;
            let phone = Jid::pn("12025550181");
            let lid = Jid::lid("100000000000281");
            let now = client.runtime.wall_time_secs();
            if existing_lid {
                raw.store_received_tc_token(&lid.user, b"synthetic-existing-token", now - 1)
                    .await
                    .unwrap();
            }
            let writer = {
                let client = client.clone();
                let phone = phone.clone();
                tokio::spawn(async move {
                    client
                        .persist_tc_token_owned(
                            phone,
                            received.then(|| (b"synthetic-owned-token".to_vec(), now)),
                            Some(now),
                            false,
                        )
                        .await
                })
            };
            tokio::time::timeout(Duration::from_secs(5), arrived.recv())
                .await
                .unwrap()
                .unwrap();
            writer.abort();
            assert!(matches!(writer.await, Err(error) if error.is_cancelled()));
            let mut guard = Box::pin(client.lid_pn_cache.lock_mutation());
            assert!(futures::poll!(guard.as_mut()).is_pending());
            drop(guard);
            let mapping = {
                let client = client.clone();
                let phone = phone.clone();
                let lid = lid.clone();
                tokio::spawn(async move {
                    client
                        .add_lid_pn_mapping(&lid.user, &phone.user, LearningSource::Usync)
                        .await
                })
            };
            release.send(()).await.unwrap();
            tokio::time::timeout(Duration::from_secs(5), mapping)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            let saved = raw.get_tc_token(&lid.user).await.unwrap().unwrap();
            assert_eq!(saved.sender_timestamp, Some(now));
            assert_eq!(
                saved.token,
                if existing_lid {
                    b"synthetic-existing-token".as_slice()
                } else if received {
                    b"synthetic-owned-token".as_slice()
                } else {
                    b"".as_slice()
                }
            );
            assert!(raw.get_tc_token(&phone.user).await.unwrap().is_none());
            client.signal_shutdown_sync();
        }
    }
}

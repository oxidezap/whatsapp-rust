//! Special message types: newsletter, app-state key share, sender-key distribution.

use super::*;
use buffa::Message as _;

const APP_STATE_KEY_SHARE_SEND_ATTEMPTS: u8 = 3;
const APP_STATE_KEY_SHARE_SEND_RETRY: std::time::Duration = std::time::Duration::from_secs(1);

fn app_state_key_share_requires_reconnect(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<crate::client::ClientError>()
            .is_some_and(crate::client::ClientError::is_transport_unavailable)
            || cause
                .downcast_ref::<crate::request::IqError>()
                .is_some_and(crate::request::IqError::is_transport_unavailable)
            || cause
                .downcast_ref::<crate::socket::error::EncryptSendError>()
                .is_some_and(crate::socket::error::EncryptSendError::is_transport_unavailable)
    })
}

impl Client {
    /// Handles a newsletter plaintext message.
    /// Newsletters are not E2E encrypted and use the <plaintext> tag directly.
    /// They never carry a `secret_encrypted_message`, so no messageSecret is
    /// stored or retained for newsletter chats (no newsletter retention class).
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.recv.newsletter", level = "debug", skip_all, fields(chat = %info.source.chat.observe(), msg_id = %info.id)))]
    pub(crate) async fn handle_newsletter_message(
        self: &Arc<Self>,
        node: &NodeRef<'_>,
        info: &Arc<MessageInfo>,
    ) {
        let Some(plaintext_node) = node.get_optional_child_by_tag(&["plaintext"]) else {
            log::warn!(
                "[msg:{}] Received newsletter message without <plaintext> child: {}",
                info.id,
                node.tag
            );
            return;
        };

        if let Some(bytes) = plaintext_node.content_bytes() {
            match waproto::codec::message_decode(bytes) {
                Ok(msg) => {
                    log::info!(
                        "[msg:{}] Received newsletter plaintext message from {}",
                        info.id,
                        info.source.chat.observe()
                    );
                    self.dispatch_parsed_message(msg, info).await;
                }
                Err(e) => {
                    log::warn!(
                        "[msg:{}] Failed to decode newsletter plaintext: {e}",
                        info.id
                    );
                }
            }
        } else {
            log::debug!(
                "[msg:{}] Newsletter <plaintext> node from {} had no content bytes; skipping decode",
                info.id,
                info.source.chat.observe()
            );
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.recv.appstate_key_share", level = "debug", skip_all)
    )]
    pub(crate) async fn handle_app_state_sync_key_share(
        &self,
        keys: &wa::message::AppStateSyncKeyShare,
    ) {
        struct KeyComponents<'a> {
            key_id: &'a [u8],
            data: &'a [u8],
            fingerprint_bytes: Vec<u8>,
            timestamp: i64,
        }

        /// Extract components from an AppStateSyncKey for storage.
        fn extract_key_components(key: &wa::message::AppStateSyncKey) -> Option<KeyComponents<'_>> {
            let key_id = key.key_id.as_option()?.key_id.as_ref()?;
            let key_data = key.key_data.as_option()?;
            let fingerprint = key_data.fingerprint.as_option()?;
            let data = key_data.key_data.as_ref()?;
            Some(KeyComponents {
                key_id,
                data,
                fingerprint_bytes: fingerprint.encode_to_vec(),
                timestamp: key_data.timestamp.unwrap_or_default(),
            })
        }

        let device_snapshot = self.persistence_manager.get_device_snapshot();
        let key_store = device_snapshot.backend.clone();
        drop(device_snapshot);

        let mut stored_count = 0;
        let mut failed_count = 0;
        let mut fulfilled_request_ids = Vec::new();

        for key in &keys.keys {
            if let Some(components) = extract_key_components(key) {
                let new_key = crate::store::traits::AppStateSyncKey {
                    key_data: components.data.to_vec(),
                    fingerprint: components.fingerprint_bytes,
                    timestamp: components.timestamp,
                };

                if let Err(e) = key_store.set_sync_key(components.key_id, new_key).await {
                    log::error!(
                        "Failed to store app state sync key {:02x?}: {:?}",
                        components.key_id,
                        e
                    );
                    failed_count += 1;
                } else {
                    stored_count += 1;
                    fulfilled_request_ids.push(components.key_id);
                }
            }
        }

        if !fulfilled_request_ids.is_empty() {
            let mut requests = self.app_state_key_requests.lock().await;
            for key_id in fulfilled_request_ids {
                requests.remove(key_id);
            }
        }

        if stored_count > 0 || failed_count > 0 {
            log::info!(
                target: "Client/AppState",
                "Processed app state key share: {} stored, {} failed.",
                stored_count,
                failed_count
            );
        }

        // Mark that keys have arrived (idempotent) and wake every waiter. Notifying
        // on each share, not just the first, lets the app-state retry loop repair
        // multiple missing keys one share at a time.
        if stored_count > 0 {
            self.initial_app_state_keys_received
                .store(true, std::sync::atomic::Ordering::Relaxed);
            self.initial_keys_synced_notifier.notify(usize::MAX);
        }
    }

    pub(super) async fn build_app_state_sync_key_share(
        &self,
        request: &wa::message::AppStateSyncKeyRequest,
    ) -> Result<wa::message::AppStateSyncKeyShare, anyhow::Error> {
        let device_snapshot = self.persistence_manager.get_device_snapshot();
        let key_store = device_snapshot.backend.clone();
        drop(device_snapshot);

        let keys = futures::future::try_join_all(request.key_ids.iter().filter_map(|requested| {
            let key_id = requested.key_id.as_deref()?;
            let key_store = &key_store;
            Some(async move {
                let key_data = if let Some(stored) = key_store.get_sync_key(key_id).await? {
                    let fingerprint = wa::message::AppStateSyncKeyFingerprint::decode_from_slice(
                        &stored.fingerprint,
                    )?;
                    buffa::MessageField::some(wa::message::AppStateSyncKeyData {
                        key_data: Some(stored.key_data),
                        fingerprint: buffa::MessageField::some(fingerprint),
                        timestamp: Some(stored.timestamp),
                    })
                } else {
                    buffa::MessageField::default()
                };

                Ok::<_, anyhow::Error>(wa::message::AppStateSyncKey {
                    key_id: buffa::MessageField::some(requested.clone()),
                    key_data,
                })
            })
        }))
        .await?;

        Ok(wa::message::AppStateSyncKeyShare { keys })
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.recv.appstate_key_request.prepare", level = "debug", skip_all, fields(count = request.key_ids.len()), err(Debug))
    )]
    pub(crate) async fn prepare_app_state_sync_key_share(
        &self,
        request: &wa::message::AppStateSyncKeyRequest,
    ) -> Result<(wa::Message, String), anyhow::Error> {
        let share = self.build_app_state_sync_key_share(request).await?;
        let message = wa::Message {
            protocol_message: buffa::MessageField::some(wa::message::ProtocolMessage {
                r#type: Some(wa::message::protocol_message::Type::AppStateSyncKeyShare),
                app_state_sync_key_share: buffa::MessageField::some(share),
                ..Default::default()
            }),
            ..Default::default()
        };
        Ok((message, self.generate_message_id()))
    }

    pub(crate) fn schedule_app_state_sync_key_share(
        self: &Arc<Self>,
        requester: Jid,
        message: wa::Message,
        message_id: String,
    ) {
        let weak = Arc::downgrade(self);
        let runtime = self.runtime.clone();
        self.runtime
            .spawn(Box::pin(async move {
                let mut attempt = 0;
                let mut retry_delay = APP_STATE_KEY_SHARE_SEND_RETRY;
                let mut reconnect_after = None;

                loop {
                    let wait_deadline =
                        wacore::time::Instant::now() + Client::DEFAULT_OFFLINE_SYNC_TIMEOUT;
                    loop {
                        let Some(client) = weak.upgrade() else {
                            return;
                        };
                        let listener = client.offline_sync_notifier.listen();
                        let ready = client
                            .offline_sync_completed
                            .load(std::sync::atomic::Ordering::Acquire)
                            && !client.inbound_commit_batch.is_active()
                            && reconnect_after.is_none_or(|generation| {
                                client
                                    .connection_generation
                                    .load(std::sync::atomic::Ordering::Acquire)
                                    != generation
                            });
                        drop(client);

                        if ready {
                            reconnect_after = None;
                            break;
                        }
                        let remaining = wait_deadline
                            .saturating_duration_since(wacore::time::Instant::now());
                        if remaining.is_zero()
                            || wacore::runtime::timeout(&*runtime, remaining, listener)
                                .await
                                .is_err()
                        {
                            warn!(
                                "Dropping deferred app-state key share to {} after offline sync timeout",
                                requester.observe()
                            );
                            return;
                        }
                    }

                    let Some(client) = weak.upgrade() else {
                        return;
                    };
                    let send_generation = client
                        .connection_generation
                        .load(std::sync::atomic::Ordering::Acquire);
                    let Some(flush_guard) = client.outbound_flush.try_track() else {
                        reconnect_after = Some(send_generation);
                        drop(client);
                        continue;
                    };
                    let result = client
                        .send_app_state_sync_key_share_once(
                            &requester,
                            &message,
                            &message_id,
                        )
                        .await;
                    drop(flush_guard);
                    drop(client);

                    let Err(error) = result else {
                        return;
                    };
                    attempt += 1;
                    warn!(
                        "App-state key share to {} failed (attempt {attempt}/{APP_STATE_KEY_SHARE_SEND_ATTEMPTS}): {error}",
                        requester.observe()
                    );
                    if attempt >= APP_STATE_KEY_SHARE_SEND_ATTEMPTS {
                        return;
                    }

                    if app_state_key_share_requires_reconnect(&error) {
                        reconnect_after = Some(send_generation);
                    } else {
                        runtime.sleep(retry_delay).await;
                    }
                    retry_delay = retry_delay.saturating_mul(2);
                }
            }))
            .detach();
    }

    async fn send_app_state_sync_key_share_once(
        &self,
        requester: &Jid,
        message: &wa::Message,
        message_id: &str,
    ) -> Result<(), anyhow::Error> {
        self.ensure_e2e_sessions(std::slice::from_ref(requester))
            .await?;
        self.send_message_impl(
            requester.clone(),
            message,
            Some(message_id.to_owned()),
            true,
            false,
            None,
            Vec::new(),
            None,
        )
        .await
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.recv.skdm", level = "debug", skip_all, fields(group = %group_jid.observe(), sender = %sender_jid.observe())))]
    pub(crate) async fn handle_sender_key_distribution_message(
        self: &Arc<Self>,
        group_jid: &Jid,
        sender_jid: &Jid,
        axolotl_bytes: &[u8],
    ) {
        let skdm = match SenderKeyDistributionMessage::try_from(axolotl_bytes) {
            Ok(msg) => msg,
            Err(e1) => match wa::SenderKeyDistributionMessage::decode_from_slice(axolotl_bytes) {
                Ok(go_msg) => {
                    let (Some(signing_key), Some(id), Some(iteration), Some(chain_key)) = (
                        go_msg.signing_key.as_ref(),
                        go_msg.id,
                        go_msg.iteration,
                        go_msg.chain_key.as_ref(),
                    ) else {
                        log::warn!(
                            "Go SKDM from {} missing required fields (signing_key={}, id={}, iteration={}, chain_key={})",
                            sender_jid.observe(),
                            go_msg.signing_key.is_some(),
                            go_msg.id.is_some(),
                            go_msg.iteration.is_some(),
                            go_msg.chain_key.is_some()
                        );
                        return;
                    };
                    let chain_key_arr: [u8; 32] = match chain_key.as_slice().try_into() {
                        Ok(arr) => arr,
                        Err(_) => {
                            log::error!(
                                "Invalid chain_key length {} from Go SKDM from {}",
                                chain_key.len(),
                                sender_jid.observe()
                            );
                            return;
                        }
                    };
                    match SignalPublicKey::from_djb_public_key_bytes(signing_key) {
                        Ok(pub_key) => {
                            match SenderKeyDistributionMessage::new(
                                SENDERKEY_MESSAGE_CURRENT_VERSION,
                                id,
                                iteration,
                                chain_key_arr,
                                pub_key,
                            ) {
                                Ok(skdm) => skdm,
                                Err(e) => {
                                    log::error!(
                                        "Failed to construct SKDM from Go format from {}: {:?} (original parse error: {:?})",
                                        sender_jid.observe(),
                                        e,
                                        e1
                                    );
                                    return;
                                }
                            }
                        }
                        Err(e) => {
                            log::error!(
                                "Failed to parse public key from Go SKDM for {}: {:?} (original parse error: {:?})",
                                sender_jid.observe(),
                                e,
                                e1
                            );
                            return;
                        }
                    }
                }
                Err(e2) => {
                    log::error!(
                        "Failed to parse SenderKeyDistributionMessage (standard and Go fallback) from {}: primary: {:?}, fallback: {:?}",
                        sender_jid.observe(),
                        e1,
                        e2
                    );
                    return;
                }
            },
        };

        // Normalize to bare sender for consistent sender key addressing.
        let sender_bare = sender_jid.to_non_ad();
        let sender_address = sender_bare.to_protocol_address();

        let sender_key_name = make_sender_key_name(group_jid, &sender_address);

        // Route through the signal cache adapter so the sender key is immediately visible
        // in the cache for subsequent group_decrypt calls within the same message batch.
        // Only the sender-key store is needed here, so build it standalone instead of
        // the full five-store adapter.
        let mut sender_key_store = self.sender_key_adapter().await;
        let chain_lock = sender_key_store.sender_key_lock(&sender_key_name).await;
        let _chain_guard = chain_lock.lock().await;

        if let Err(e) =
            process_sender_key_distribution_message(&sender_key_name, &skdm, &mut sender_key_store)
                .await
        {
            log::error!(
                "Failed to process SenderKeyDistributionMessage from {}: {:?}",
                sender_jid.observe(),
                e
            );
        } else {
            log::debug!(
                "Successfully processed sender key distribution for group {} from {}",
                group_jid.observe(),
                sender_jid.observe()
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::app_state_key_share_requires_reconnect;

    #[test]
    fn key_share_reconnects_for_direct_and_iq_connection_loss() {
        let direct = anyhow::Error::new(crate::client::ClientError::NotConnected);
        assert!(app_state_key_share_requires_reconnect(&direct));

        let iq = anyhow::Error::new(crate::request::IqError::NotConnected);
        assert!(app_state_key_share_requires_reconnect(&iq));

        let non_transport = anyhow::anyhow!("pre-wire failure");
        assert!(!app_state_key_share_requires_reconnect(&non_transport));
    }
}

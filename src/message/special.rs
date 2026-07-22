//! Special message types: newsletter, app-state key share, sender-key distribution.

use super::*;

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
                    self.dispatch_parsed_message(msg, info, false).await;
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
                fingerprint_bytes: waproto::codec::app_state_sync_key_fingerprint_to_vec(
                    fingerprint,
                ),
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

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.recv.appstate_key_request.prepare", level = "debug", skip_all, fields(count = request.key_ids.len()), err(Debug))
    )]
    pub(super) async fn build_app_state_sync_key_share(
        &self,
        request: &wa::message::AppStateSyncKeyRequest,
    ) -> Result<wa::message::AppStateSyncKeyShare, anyhow::Error> {
        #[cfg(test)]
        if self
            .app_state_key_share_prepare_test_failures
            .fetch_update(
                std::sync::atomic::Ordering::AcqRel,
                std::sync::atomic::Ordering::Acquire,
                |remaining| remaining.checked_sub(1),
            )
            .is_ok()
        {
            anyhow::bail!("injected app-state key-share preparation failure");
        }

        let device_snapshot = self.persistence_manager.get_device_snapshot();
        let key_store = device_snapshot.backend.clone();
        drop(device_snapshot);

        let mut keys = Vec::with_capacity(request.key_ids.len());
        for requested in &request.key_ids {
            let Some(key_id) = requested.key_id.as_deref() else {
                continue;
            };
            let key_data = if let Some(stored) = key_store.get_sync_key(key_id).await? {
                match waproto::codec::app_state_sync_key_fingerprint_decode(&stored.fingerprint) {
                    Ok(fingerprint) => {
                        buffa::MessageField::some(wa::message::AppStateSyncKeyData {
                            key_data: Some(stored.key_data),
                            fingerprint: buffa::MessageField::some(fingerprint),
                            timestamp: Some(stored.timestamp),
                        })
                    }
                    Err(error) => {
                        warn!(target: "Client/AppState", "Stored app-state key fingerprint is invalid; returning orphan: {error}");
                        buffa::MessageField::default()
                    }
                }
            } else {
                buffa::MessageField::default()
            };
            keys.push(wa::message::AppStateSyncKey {
                key_id: buffa::MessageField::some(requested.clone()),
                key_data,
            });
        }

        Ok(wa::message::AppStateSyncKeyShare { keys })
    }

    pub(crate) fn schedule_app_state_sync_key_share(
        self: &Arc<Self>,
        requester: Jid,
        request: wa::message::AppStateSyncKeyRequest,
        mut commit_ticket: Option<InboundCommitTicket>,
    ) {
        let weak = Arc::downgrade(self);
        let runtime = self.runtime.clone();
        let message_id = self.generate_message_id();
        self.runtime
            .spawn(Box::pin(async move {
                let mut attempt = 0;
                let mut retry_delay = APP_STATE_KEY_SHARE_SEND_RETRY;
                let mut reconnect_after = None;
                let mut message = None;

                loop {
                    let wait_deadline =
                        wacore::time::Instant::now() + Client::DEFAULT_OFFLINE_SYNC_TIMEOUT;
                    loop {
                        let Some(client) = weak.upgrade() else {
                            return;
                        };
                        let listener = client.offline_sync_notifier.listen();
                        let commit_ready = match commit_ticket
                            .as_ref()
                            .map(InboundCommitTicket::state)
                        {
                            Some(InboundCommitTicketState::Pending) => false,
                            Some(InboundCommitTicketState::Dropped) => return,
                            Some(InboundCommitTicketState::Durable) => {
                                commit_ticket = None;
                                true
                            }
                            None => true,
                        };
                        let ready = commit_ready
                            && client
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
                    let result = match message.as_ref() {
                        Some(message) => {
                            client
                                .send_app_state_sync_key_share_once(
                                    &requester,
                                    message,
                                    &message_id,
                                )
                                .await
                        }
                        None => match client.build_app_state_sync_key_share(&request).await {
                            Ok(share) => {
                                let prepared = wa::Message {
                                    protocol_message: buffa::MessageField::some(
                                        wa::message::ProtocolMessage {
                                            r#type: Some(
                                                wa::message::protocol_message::Type::AppStateSyncKeyShare,
                                            ),
                                            app_state_sync_key_share: buffa::MessageField::some(
                                                share,
                                            ),
                                            ..Default::default()
                                        },
                                    ),
                                    ..Default::default()
                                };
                                let result = client
                                    .send_app_state_sync_key_share_once(
                                        &requester,
                                        &prepared,
                                        &message_id,
                                    )
                                    .await;
                                message = Some(prepared);
                                result
                            }
                            Err(error) => Err(error),
                        },
                    };
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
            crate::send::SendPipelineOptions {
                request_id: Some(message_id.to_owned()),
                peer: true,
                ..Default::default()
            },
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
        if let Err(e) = self
            .signal()
            .process_sender_key_distribution_cached(group_jid, sender_jid, axolotl_bytes)
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
        use wacore_binary::builder::NodeBuilder;

        let direct = anyhow::Error::new(crate::client::ClientError::NotConnected);
        assert!(app_state_key_share_requires_reconnect(&direct));

        let iq = anyhow::Error::new(crate::request::IqError::NotConnected);
        assert!(app_state_key_share_requires_reconnect(&iq));

        let channel_closed = anyhow::Error::new(crate::request::IqError::InternalChannelClosed);
        assert!(app_state_key_share_requires_reconnect(&channel_closed));

        let disconnected = anyhow::Error::new(crate::request::IqError::Disconnected(Box::new(
            NodeBuilder::new("disconnect").build(),
        )));
        assert!(app_state_key_share_requires_reconnect(&disconnected));

        let non_transport = anyhow::anyhow!("pre-wire failure");
        assert!(!app_state_key_share_requires_reconnect(&non_transport));
    }
}

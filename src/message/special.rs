//! Special message types: newsletter, app-state key share, sender-key distribution.

use super::*;

impl Client {
    /// Handles a newsletter plaintext message.
    /// Newsletters are not E2E encrypted and use the <plaintext> tag directly.
    /// They never carry a `secret_encrypted_message`, so no messageSecret is
    /// stored or retained for newsletter chats (no newsletter retention class).
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
            match wa::Message::decode(bytes) {
                Ok(msg) => {
                    log::info!(
                        "[msg:{}] Received newsletter plaintext message from {}",
                        info.id,
                        info.source.chat
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
                info.source.chat
            );
        }
    }

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
            let key_id = key.key_id.as_ref()?.key_id.as_ref()?;
            let key_data = key.key_data.as_ref()?;
            let fingerprint = key_data.fingerprint.as_ref()?;
            let data = key_data.key_data.as_ref()?;
            Some(KeyComponents {
                key_id,
                data,
                fingerprint_bytes: fingerprint.encode_to_vec(),
                timestamp: key_data.timestamp(),
            })
        }

        let device_snapshot = self.persistence_manager.get_device_snapshot().await;
        let key_store = device_snapshot.backend.clone();

        let mut stored_count = 0;
        let mut failed_count = 0;

        for key in &keys.keys {
            if let Some(components) = extract_key_components(key) {
                let new_key = crate::store::traits::AppStateSyncKey {
                    key_data: components.data.to_vec(),
                    fingerprint: components.fingerprint_bytes,
                    timestamp: components.timestamp,
                };

                if let Err(e) = key_store.set_sync_key(components.key_id, new_key).await {
                    log::error!(
                        "Failed to store app state sync key {:?}: {:?}",
                        hex::encode(components.key_id),
                        e
                    );
                    failed_count += 1;
                } else {
                    stored_count += 1;
                }
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

        // Notify any waiters (initial full sync) that at least one key share was processed.
        if stored_count > 0
            && !self
                .initial_app_state_keys_received
                .swap(true, std::sync::atomic::Ordering::Relaxed)
        {
            // First time setting; notify any waiters
            self.initial_keys_synced_notifier.notify(usize::MAX);
        }
    }

    pub(crate) async fn handle_sender_key_distribution_message(
        self: &Arc<Self>,
        group_jid: &Jid,
        sender_jid: &Jid,
        axolotl_bytes: &[u8],
    ) {
        let skdm = match SenderKeyDistributionMessage::try_from(axolotl_bytes) {
            Ok(msg) => msg,
            Err(e1) => match wa::SenderKeyDistributionMessage::decode(axolotl_bytes) {
                Ok(go_msg) => {
                    let (Some(signing_key), Some(id), Some(iteration), Some(chain_key)) = (
                        go_msg.signing_key.as_ref(),
                        go_msg.id,
                        go_msg.iteration,
                        go_msg.chain_key.as_ref(),
                    ) else {
                        log::warn!(
                            "Go SKDM from {} missing required fields (signing_key={}, id={}, iteration={}, chain_key={})",
                            sender_jid,
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
                                sender_jid
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
                                        sender_jid,
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
                                sender_jid,
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
                        sender_jid,
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

        if let Err(e) =
            process_sender_key_distribution_message(&sender_key_name, &skdm, &mut sender_key_store)
                .await
        {
            log::error!(
                "Failed to process SenderKeyDistributionMessage from {}: {:?}",
                sender_jid,
                e
            );
        } else {
            log::debug!(
                "Successfully processed sender key distribution for group {} from {}",
                group_jid,
                sender_jid
            );
        }
    }
}

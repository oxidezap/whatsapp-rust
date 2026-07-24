//! Low-level Signal protocol and raw transport APIs.
//!
//! Encryption, decryption, session management, and participant node creation.

use thiserror::Error;
use wacore::libsignal::protocol::{
    CiphertextMessage, DecryptionResult, IdentityChange, PreKeyBundle, PreKeySignalMessage,
    PublicKey, SENDERKEY_MESSAGE_CURRENT_VERSION, SenderKeyDistributionMessage, SenderKeyStore,
    SignalMessage, SignalProtocolError, UsePQRatchet, message_decrypt, message_encrypt,
    process_sender_key_distribution_message,
};
use wacore::message_processing::EncType;
use wacore::messages::MessageUtils;
use wacore::types::jid::{JidExt, make_sender_key_name};
use wacore_binary::Jid;
use wacore_binary::Node;

use crate::client::Client;

/// Error returned by the low-level Signal protocol operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SignalError {
    /// A Signal protocol primitive (encrypt/decrypt/session) failed.
    #[error("{0}")]
    Protocol(#[from] SignalProtocolError),
    /// The requested operation is not valid for this input (e.g. a sender-key
    /// or message-secret envelope passed to the pairwise decrypt path).
    #[error("unsupported signal operation: {0}")]
    Unsupported(String),
    /// The operation is supported but one of its inputs is malformed.
    #[error("invalid signal input: {0}")]
    InvalidInput(String),
    /// Catch-all for internal failures (device resolution, cache flush).
    #[error("{0}")]
    Internal(#[from] anyhow::Error),
}

impl From<crate::client::SignalMaintenanceError> for SignalError {
    fn from(err: crate::client::SignalMaintenanceError) -> Self {
        match err {
            crate::client::SignalMaintenanceError::Signal(e) => SignalError::Protocol(e),
            other => SignalError::Internal(other.into()),
        }
    }
}

/// Read-only information from a currently open pairwise session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalSessionInfo {
    /// Local base key identifying the active session state.
    pub base_key: Vec<u8>,
    /// Remote registration identifier recorded by the session.
    pub registration_id: u32,
}

/// Result of moving pairwise session state between address namespaces.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct SignalSessionMigration {
    /// Pairwise sessions moved to the destination namespace.
    pub migrated: usize,
    /// Pairwise session lookups skipped after a storage error.
    pub skipped: usize,
    /// Pairwise sessions found or unsuccessfully queried.
    pub total: usize,
    /// Identity records moved when the destination had no identity.
    pub migrated_identities: usize,
    /// Source identity records removed in favor of an existing destination.
    pub discarded_identities: usize,
    /// Identity lookups skipped after a storage error.
    pub skipped_identities: usize,
}

impl SignalSessionMigration {
    /// Whether any source state was moved or removed.
    pub fn has_state_changes(self) -> bool {
        self.migrated != 0 || self.migrated_identities != 0 || self.discarded_identities != 0
    }
}

fn decode_sender_key_distribution(
    bytes: &[u8],
) -> Result<SenderKeyDistributionMessage, SignalError> {
    match SenderKeyDistributionMessage::try_from(bytes) {
        Ok(message) => Ok(message),
        Err(primary_error) => {
            let fallback = waproto::codec::sender_key_distribution_message_decode(bytes)
                .map_err(|fallback_error| {
                    SignalError::InvalidInput(format!(
                        "sender-key distribution decode failed: primary={primary_error}; fallback={fallback_error}"
                    ))
                })?;
            let signing_key = fallback.signing_key.ok_or_else(|| {
                SignalError::InvalidInput("sender-key distribution is missing signing_key".into())
            })?;
            let id = fallback.id.ok_or_else(|| {
                SignalError::InvalidInput("sender-key distribution is missing id".into())
            })?;
            let iteration = fallback.iteration.ok_or_else(|| {
                SignalError::InvalidInput("sender-key distribution is missing iteration".into())
            })?;
            let chain_key: [u8; 32] = fallback
                .chain_key
                .ok_or_else(|| {
                    SignalError::InvalidInput("sender-key distribution is missing chain_key".into())
                })?
                .try_into()
                .map_err(|value: Vec<u8>| {
                    SignalError::InvalidInput(format!(
                        "sender-key distribution chain_key must be 32 bytes, got {}",
                        value.len()
                    ))
                })?;
            let signing_key =
                PublicKey::from_djb_public_key_bytes(&signing_key).map_err(|error| {
                    SignalError::InvalidInput(format!(
                        "sender-key distribution signing_key is invalid: {error}"
                    ))
                })?;
            Ok(SenderKeyDistributionMessage::new(
                SENDERKEY_MESSAGE_CURRENT_VERSION,
                id,
                iteration,
                chain_key,
                signing_key,
            )?)
        }
    }
}

/// Feature handle for Signal protocol operations.
pub struct Signal<'a> {
    client: &'a Client,
}

impl<'a> Signal<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    async fn session_info_at(&self, jid: &Jid) -> Result<Option<SignalSessionInfo>, SignalError> {
        let address = jid.to_protocol_address();
        let session_mutex = self.client.session_lock_for(address.as_str()).await;
        let _session_guard = session_mutex.lock().await;
        let device = self.client.persistence_manager.get_device_snapshot();
        let Some(session) = self
            .client
            .signal_cache
            .peek_session(&address, &*device.backend)
            .await?
        else {
            return Ok(None);
        };
        let base_key = session.alice_base_key()?;
        let registration_id = session.remote_registration_id()?;
        Ok(Some(SignalSessionInfo {
            base_key: base_key.to_vec(),
            registration_id,
        }))
    }

    /// Move a legacy source-namespace session only after a resolved lookup
    /// misses. Successful steady-state operations therefore pay no migration
    /// preflight, while startup state stored under PN remains recoverable.
    async fn migrate_legacy_pairwise_state(
        &self,
        source: &Jid,
        resolved: &Jid,
    ) -> Result<bool, SignalError> {
        if source.server == resolved.server {
            return Ok(false);
        }
        Ok(self.migrate_sessions(source, resolved).await?.migrated != 0)
    }

    async fn encrypt_pairwise_at(
        &self,
        jid: &Jid,
        plaintext: &[u8],
    ) -> Result<CiphertextMessage, SignalError> {
        let address = jid.to_protocol_address();
        let lock = self.client.session_lock_for(address.as_str()).await;
        let _guard = lock.lock().await;
        let mut adapter = self.client.signal_adapter().await;
        Ok(message_encrypt(
            plaintext,
            &address,
            &mut adapter.session_store,
            &mut adapter.identity_store,
        )
        .await?)
    }

    async fn decrypt_pairwise_at(
        &self,
        jid: &Jid,
        parsed: &CiphertextMessage,
    ) -> Result<DecryptionResult, SignalError> {
        let address = jid.to_protocol_address();
        let lock = self.client.session_lock_for(address.as_str()).await;
        let _guard = lock.lock().await;
        let mut adapter = self.client.signal_adapter().await;
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let decrypted = message_decrypt(
            parsed,
            &address,
            &mut adapter.session_store,
            &mut adapter.identity_store,
            &mut adapter.pre_key_store,
            &adapter.signed_pre_key_store,
            &mut rng,
            UsePQRatchet::No,
        )
        .await?;

        // A pkmsg consumed prekey is reported, not deleted by the decrypt;
        // buffer it so the caller's flush removes it atomically with the
        // promoted session.
        if let Some(prekey_id) = decrypted.consumed_prekey_id {
            adapter
                .pre_key_store
                .buffer_consumed_prekey(prekey_id, &address)
                .await;
        }
        Ok(decrypted)
    }

    async fn delete_pairwise_state_at(&self, jid: &Jid) {
        let address = jid.to_protocol_address();
        let lock = self.client.session_lock_for(address.as_str()).await;
        let _guard = lock.lock().await;
        self.client.signal_cache.delete_session(&address).await;
        self.client.signal_cache.delete_identity(&address).await;
    }

    /// Install a supplied pairwise pre-key bundle and durably expose the new
    /// session before returning.
    pub async fn install_prekey_bundle(
        &self,
        jid: &Jid,
        bundle: &PreKeyBundle,
    ) -> Result<IdentityChange, SignalError> {
        let resolved = self.client.resolve_encryption_jid(jid).await;
        let mut adapter = self.client.signal_adapter().await;
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let identity_change = self
            .client
            .install_prekey_bundle_cached(&resolved, bundle, &mut adapter, &mut rng)
            .await?;
        self.client.flush_signal_cache_batch_safe().await?;
        Ok(identity_change)
    }

    /// Process a sender-key distribution and durably expose it before return.
    pub async fn process_sender_key_distribution(
        &self,
        group_jid: &Jid,
        sender_jid: &Jid,
        distribution: &[u8],
    ) -> Result<(), SignalError> {
        self.process_sender_key_distribution_cached(group_jid, sender_jid, distribution)
            .await?;
        self.client.flush_signal_cache_batch_safe().await?;
        Ok(())
    }

    /// Cache-only variant for the inbound message pipeline, whose enclosing
    /// commit owns the batched durability flush.
    pub(crate) async fn process_sender_key_distribution_cached(
        &self,
        group_jid: &Jid,
        sender_jid: &Jid,
        distribution: &[u8],
    ) -> Result<(), SignalError> {
        let distribution = decode_sender_key_distribution(distribution)?;
        let sender_address = sender_jid.to_non_ad().to_protocol_address();
        let sender_key_name = make_sender_key_name(group_jid, &sender_address);
        let mut store = self.client.sender_key_adapter().await;
        let chain_lock = store.sender_key_lock(&sender_key_name).await;
        let chain_guard = chain_lock.lock().await;

        process_sender_key_distribution_message(&sender_key_name, &distribution, &mut store)
            .await?;
        drop(chain_guard);
        Ok(())
    }

    /// Create the current sender-key distribution for a group.
    pub async fn sender_key_distribution(
        &self,
        group_jid: &Jid,
        sender_jid: &Jid,
    ) -> Result<Vec<u8>, SignalError> {
        let sender_address = sender_jid.to_non_ad().to_protocol_address();
        let sender_key_name = make_sender_key_name(group_jid, &sender_address);
        let mut store = self.client.sender_key_adapter().await;
        let chain_lock = store.sender_key_lock(&sender_key_name).await;
        let chain_guard = chain_lock.lock().await;
        let distribution = wacore::send::create_sender_key_distribution_message_for_group(
            &mut store,
            &sender_key_name,
        )
        .await?;
        drop(chain_guard);
        self.client.persist_signal_state_pre_wire().await?;
        Ok(distribution)
    }

    /// Check whether sender-key state exists for a group and sender.
    pub async fn has_sender_key(
        &self,
        group_jid: &Jid,
        sender_jid: &Jid,
    ) -> Result<bool, SignalError> {
        let sender_address = sender_jid.to_non_ad().to_protocol_address();
        let sender_key_name = make_sender_key_name(group_jid, &sender_address);
        let device = self.client.persistence_manager.get_device_snapshot();
        Ok(self
            .client
            .signal_cache
            .get_sender_key(&sender_key_name, &*device.backend)
            .await?
            .is_some())
    }

    /// Delete one sender-key chain and make the removal durable before returning.
    pub async fn delete_sender_key(
        &self,
        group_jid: &Jid,
        sender_jid: &Jid,
    ) -> Result<(), SignalError> {
        let sender_address = sender_jid.to_non_ad().to_protocol_address();
        let sender_key_name = make_sender_key_name(group_jid, &sender_address);
        let backend = self.client.persistence_manager.backend();
        self.client
            .signal_cache
            .delete_sender_key_durable(&sender_key_name, backend.as_ref())
            .await?;
        Ok(())
    }

    /// Inspect the currently open pairwise session for a JID.
    pub async fn session_info(&self, jid: &Jid) -> Result<Option<SignalSessionInfo>, SignalError> {
        let resolved = self.client.resolve_encryption_jid(jid).await;
        let info = self.session_info_at(&resolved).await?;
        if info.is_some() || !self.migrate_legacy_pairwise_state(jid, &resolved).await? {
            return Ok(info);
        }
        self.session_info_at(&resolved).await
    }

    /// Move pairwise session state from a phone-number namespace to its linked
    /// identifier namespace across known device slots.
    pub async fn migrate_sessions(
        &self,
        from: &Jid,
        to: &Jid,
    ) -> Result<SignalSessionMigration, SignalError> {
        if !matches!(
            (from.server, to.server),
            (wacore_binary::Server::Pn, wacore_binary::Server::Lid)
                | (
                    wacore_binary::Server::Hosted,
                    wacore_binary::Server::HostedLid
                )
        ) {
            return Err(SignalError::InvalidInput(
                "source and destination must be matching phone and linked-identifier namespaces"
                    .into(),
            ));
        }
        let outcome = self.client.migrate_signal_sessions(from, to).await;
        if outcome.has_state_changes()
            || self
                .client
                .signal_cache
                .has_pending_pairwise_writes_for_user(&from.user)
                .await
        {
            self.client.flush_signal_cache_batch_safe().await?;
        }
        Ok(outcome)
    }

    /// Encrypt plaintext for a single recipient using the Signal protocol.
    ///
    /// Returns `(EncType, ciphertext_bytes)`. The caller is responsible
    /// for padding if needed; this method encrypts raw bytes.
    ///
    /// PN JIDs are resolved to LID, with a legacy PN session migrated lazily
    /// if the resolved lookup misses.
    pub async fn encrypt_message(
        &self,
        jid: &Jid,
        plaintext: &[u8],
    ) -> Result<(EncType, Vec<u8>), SignalError> {
        let encryption_jid = self.client.resolve_encryption_jid(jid).await;
        let encrypted = match self.encrypt_pairwise_at(&encryption_jid, plaintext).await {
            Ok(encrypted) => encrypted,
            Err(error @ SignalError::Protocol(SignalProtocolError::SessionNotFound(_))) => {
                if !self
                    .migrate_legacy_pairwise_state(jid, &encryption_jid)
                    .await?
                {
                    return Err(error);
                }
                self.encrypt_pairwise_at(&encryption_jid, plaintext).await?
            }
            Err(error) => return Err(error),
        };

        // Same pre-wire gate as the send path: the caller transmits these
        // bytes, so a raised lease must be durable before they leave here.
        self.client.persist_signal_state_pre_wire().await?;

        let (_, is_prekey, bytes) = wacore::send::extract_ciphertext(encrypted)
            .ok_or_else(|| SignalError::Unsupported("unexpected ciphertext variant".into()))?;
        let enc_type = if is_prekey {
            EncType::PreKeyMessage
        } else {
            EncType::Message
        };
        Ok((enc_type, bytes.into_vec()))
    }

    /// Decrypt a Signal protocol message from a sender.
    ///
    /// Returns raw padded plaintext. Use [`MessageUtils::unpad_message_ref`]
    /// with the stanza's `v` attribute if WhatsApp message unpadding is needed.
    ///
    /// PN JIDs are resolved to LID, with a legacy PN session migrated lazily
    /// if the resolved lookup misses.
    pub async fn decrypt_message(
        &self,
        jid: &Jid,
        enc_type: EncType,
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, SignalError> {
        let parsed = match enc_type {
            EncType::PreKeyMessage => {
                CiphertextMessage::PreKeySignalMessage(PreKeySignalMessage::try_from(ciphertext)?)
            }
            EncType::Message => {
                CiphertextMessage::SignalMessage(SignalMessage::try_from(ciphertext)?)
            }
            EncType::SenderKey => {
                return Err(SignalError::Unsupported(
                    "use decrypt_group_message for sender-key messages".into(),
                ));
            }
            EncType::MessageSecret => {
                return Err(SignalError::Unsupported(
                    "msmsg envelopes are not Signal messages; use the bot_message path".into(),
                ));
            }
        };

        let encryption_jid = self.client.resolve_encryption_jid(jid).await;
        let decrypted = match self.decrypt_pairwise_at(&encryption_jid, &parsed).await {
            Ok(decrypted) => decrypted,
            Err(error @ SignalError::Protocol(SignalProtocolError::SessionNotFound(_))) => {
                if !self
                    .migrate_legacy_pairwise_state(jid, &encryption_jid)
                    .await?
                {
                    return Err(error);
                }
                self.decrypt_pairwise_at(&encryption_jid, &parsed).await?
            }
            Err(error) => return Err(error),
        };

        self.client.flush_signal_cache_batch_safe().await?;

        Ok(decrypted.plaintext)
    }

    /// Encrypt plaintext for a group using sender keys.
    ///
    /// Returns `(Option<skdm_bytes>, ciphertext_bytes)`. The SKDM is `Some`
    /// while a new sender key still requires distribution (first encrypt for
    /// this group, after key rotation, or a retry whose earlier durability
    /// gate failed). Callers must distribute the SKDM to all group participants
    /// when present.
    ///
    /// Concurrent calls for the same `(group, sender)` are serialized on the
    /// sender-key chain, so the SKDM and the skmsg can't be split across keys.
    pub async fn encrypt_group_message(
        &self,
        group_jid: &Jid,
        plaintext: &[u8],
    ) -> Result<(Option<Vec<u8>>, Vec<u8>), SignalError> {
        let own_jid = self.client.get_own_jid_for_group(group_jid).await?;
        let sender_addr = own_jid.to_protocol_address();
        let sender_key_name = make_sender_key_name(group_jid, &sender_addr);

        // Serialize the key-existence check + SKDM creation + encrypt for this chain.
        let chain_lock = self
            .client
            .signal_cache
            .sender_key_lock(&sender_key_name)
            .await;
        let _chain_guard = chain_lock.lock().await;

        // Only create SKDM when no sender key exists (matches WA Web behavior)
        let device_snapshot = self.client.persistence_manager.get_device_snapshot();
        let key_exists = self
            .client
            .signal_cache
            .get_sender_key(&sender_key_name, &*device_snapshot.backend)
            .await?
            .is_some();

        let mut store = self.client.sender_key_adapter().await;
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();

        let pending_distribution = self
            .client
            .signal_cache
            .pending_sender_key_distribution(&sender_key_name)
            .await;
        let skdm_bytes = if let Some(distribution) = pending_distribution {
            Some(distribution.as_ref().to_vec())
        } else if !key_exists {
            let distribution = wacore::send::create_sender_key_distribution_message_for_group(
                &mut store,
                &sender_key_name,
            )
            .await?;
            self.client
                .signal_cache
                .cache_pending_sender_key_distribution(
                    &sender_key_name,
                    std::sync::Arc::from(distribution.clone()),
                )
                .await;
            Some(distribution)
        } else {
            None
        };

        let ciphertext =
            wacore::send::encrypt_group_message(&mut store, &sender_key_name, plaintext, &mut rng)
                .await?;

        // The durability gate can need the processing permit, whose holder may
        // need this chain lock.
        drop(_chain_guard);
        self.client.persist_signal_state_pre_wire().await?;

        if let Some(distribution) = &skdm_bytes {
            self.client
                .signal_cache
                .clear_pending_sender_key_distribution(&sender_key_name, distribution)
                .await;
        }

        Ok((skdm_bytes, ciphertext.into_serialized().into_vec()))
    }

    /// Decrypt a group (sender-key) message.
    ///
    /// Returns raw padded plaintext. Use [`MessageUtils::unpad_message_ref`]
    /// with the stanza's `v` attribute if WhatsApp message unpadding is needed.
    ///
    /// Concurrent mutations of the same sender-key chain are serialized.
    pub async fn decrypt_group_message(
        &self,
        group_jid: &Jid,
        sender_jid: &Jid,
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, SignalError> {
        let sender_key_name =
            make_sender_key_name(group_jid, &sender_jid.to_non_ad().to_protocol_address());

        let mut store = self.client.sender_key_adapter().await;
        let chain_lock = store.sender_key_lock(&sender_key_name).await;
        let _chain_guard = chain_lock.lock().await;

        let plaintext =
            wacore::libsignal::protocol::group_decrypt(ciphertext, &mut store, &sender_key_name)
                .await?;

        drop(_chain_guard);
        self.client.flush_signal_cache_batch_safe().await?;

        Ok(plaintext.to_vec())
    }

    /// Check whether a Signal session exists for `jid`.
    ///
    /// PN JIDs are resolved to LID when a LID mapping exists, matching
    /// the encrypt/decrypt paths.
    pub async fn validate_session(&self, jid: &Jid) -> Result<bool, SignalError> {
        let resolved = self.client.resolve_encryption_jid(jid).await;
        let signal_addr = resolved.to_protocol_address();
        let device_snapshot = self.client.persistence_manager.get_device_snapshot();
        let exists = self
            .client
            .signal_cache
            .has_session(&signal_addr, &*device_snapshot.backend)
            .await
            .map_err(|e| SignalError::Internal(e.context("session check failed")))?;
        if exists || !self.migrate_legacy_pairwise_state(jid, &resolved).await? {
            return Ok(exists);
        }
        self.client
            .signal_cache
            .has_session(&signal_addr, &*device_snapshot.backend)
            .await
            .map_err(|e| SignalError::Internal(e.context("session check failed")))
    }

    /// Delete Signal sessions and identity keys for the given JIDs.
    ///
    /// Matches WA Web's `deleteRemoteSession` which removes both session
    /// and identity as a paired operation. Changes are flushed to the
    /// persistent backend before returning.
    ///
    /// When a supplied PN JID resolves to LID, both namespace representations
    /// are removed so legacy PN state cannot be migrated back after deletion.
    pub async fn delete_sessions(&self, jids: &[Jid]) -> Result<(), SignalError> {
        for jid in jids {
            let resolved = self.client.resolve_encryption_jid(jid).await;
            self.delete_pairwise_state_at(jid).await;
            if resolved != *jid {
                self.delete_pairwise_state_at(&resolved).await;
            }
        }

        self.client.flush_signal_cache_batch_safe().await?;
        Ok(())
    }

    /// Create encrypted participant `<to>` nodes for the given recipient JIDs.
    ///
    /// Resolves devices, ensures Signal sessions, encrypts the message for
    /// each device, and returns the resulting XML nodes.
    ///
    /// Returns `(nodes, should_include_device_identity)`.
    pub async fn create_participant_nodes(
        &self,
        recipient_jids: &[Jid],
        message: &waproto::whatsapp::Message,
    ) -> Result<(Vec<Node>, bool), SignalError> {
        let device_jids = self.client.get_user_devices(recipient_jids).await?;
        self.client.ensure_e2e_sessions(&device_jids).await?;

        // Acquire per-device session locks before encrypting (matches DM send path)
        let lock_jids = self.client.build_session_lock_keys(&device_jids).await;
        let session_mutexes = self.client.session_mutexes_for(&lock_jids).await;
        let mut _session_guards = Vec::with_capacity(session_mutexes.len());
        for mutex in &session_mutexes {
            _session_guards.push(mutex.lock().await);
        }

        let plaintext = MessageUtils::encode_and_pad(message);
        let mut adapter = self.client.signal_adapter().await;
        let mediatype = wacore::send::media_type_from_message(message);
        let hide_decrypt_fail = wacore::send::should_hide_decrypt_fail(message);

        let mut stores = adapter.as_signal_stores();
        let result = wacore::send::encrypt_for_devices(
            &*self.client.runtime,
            &mut stores,
            self.client,
            &device_jids,
            &plaintext,
            hide_decrypt_fail,
            mediatype,
        )
        .await?;

        drop(_session_guards);
        self.client.persist_signal_state_pre_wire().await?;

        Ok((result.participant_nodes, result.includes_prekey_message))
    }

    /// Ensure E2E sessions exist for the given JIDs.
    pub async fn assert_sessions(&self, jids: &[Jid]) -> Result<(), SignalError> {
        self.client.ensure_e2e_sessions(jids).await?;
        Ok(())
    }

    /// Get all known device JIDs for the given user JIDs via usync.
    pub async fn get_user_devices(&self, jids: &[Jid]) -> Result<Vec<Jid>, SignalError> {
        Ok(self.client.get_user_devices(jids).await?)
    }
}

impl Client {
    /// Access low-level Signal protocol operations.
    pub fn signal(&self) -> Signal<'_> {
        Signal::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::Ordering;

    use wacore::store::in_memory::InMemoryBackend;
    use wacore::store::traits::{DeviceInfo, DeviceListRecord, SignalStore};
    use wacore_binary::Server;

    use crate::lid_pn_cache::LearningSource;
    use crate::test_utils::seed_peer_session;

    async fn memory_client() -> (Arc<Client>, Arc<InMemoryBackend>) {
        let backend = Arc::new(InMemoryBackend::new());
        let client = crate::test_utils::create_test_client_with_backend(backend.clone()).await;
        client
            .persistence_manager
            .process_command(crate::store::commands::DeviceCommand::SetId(Some(
                Jid::new("15550001000", Server::Pn),
            )))
            .await;
        (client, backend)
    }

    fn peer_prekey_bundle(registration_id: u32, device_id: u32) -> PreKeyBundle {
        use wacore::libsignal::protocol::{IdentityKeyPair, KeyPair};

        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let identity = IdentityKeyPair::generate(&mut rng);
        let signed_prekey = KeyPair::generate(&mut rng);
        let prekey = KeyPair::generate(&mut rng);
        let signature = identity
            .private_key()
            .calculate_signature(&signed_prekey.public_key.serialize(), &mut rng)
            .expect("signed prekey signature");
        PreKeyBundle::new(
            registration_id,
            device_id.into(),
            Some((7u32.into(), prekey.public_key)),
            9u32.into(),
            signed_prekey.public_key,
            signature.to_vec(),
            *identity.identity_key(),
        )
        .expect("prekey bundle")
    }

    async fn seed_legacy_pn_session_with_mapping(
        client: &Arc<Client>,
        pn: &Jid,
        lid: &Jid,
        registration_id: u32,
    ) {
        client
            .signal()
            .install_prekey_bundle(
                pn,
                &peer_prekey_bundle(registration_id, u32::from(pn.device)),
            )
            .await
            .expect("install legacy PN session");
        client
            .lid_pn_cache
            .warm_up([crate::lid_pn_cache::LidPnEntry::new(
                lid.user.to_string(),
                pn.user.to_string(),
                LearningSource::Other,
            )])
            .await;
    }

    #[tokio::test]
    async fn supplied_prekey_bundle_exposes_session_info() {
        let (client, _) = memory_client().await;
        let peer = Jid::pn_device("15550002000", 2);
        let bundle = peer_prekey_bundle(4242, u32::from(peer.device));

        client
            .signal()
            .install_prekey_bundle(&peer, &bundle)
            .await
            .expect("install bundle");

        assert!(client.signal().validate_session(&peer).await.unwrap());
        let info = client
            .signal()
            .session_info(&peer)
            .await
            .unwrap()
            .expect("open session info");
        assert_eq!(info.registration_id, 4242);
        assert!(!info.base_key.is_empty());
    }

    #[tokio::test]
    async fn supplied_prekey_bundle_uses_known_lid_namespace() {
        let (client, backend) = memory_client().await;
        let pn = Jid::pn_device("15550002002", 2);
        let lid = Jid::lid_device("100000000000002", 2);
        client
            .add_lid_pn_mapping(&lid.user, &pn.user, LearningSource::PeerPnMessage)
            .await
            .unwrap();

        client
            .signal()
            .install_prekey_bundle(&pn, &peer_prekey_bundle(4244, u32::from(pn.device)))
            .await
            .expect("install mapped bundle");

        assert!(client.signal().validate_session(&pn).await.unwrap());
        assert!(
            backend
                .get_session(pn.to_protocol_address().as_str())
                .await
                .unwrap()
                .is_none(),
            "the obsolete phone-number slot must stay empty"
        );
        assert!(
            backend
                .get_session(lid.to_protocol_address().as_str())
                .await
                .unwrap()
                .is_some(),
            "the installed session must be durable in the resolved namespace"
        );
    }

    #[tokio::test]
    async fn facade_lookup_migrates_a_legacy_pn_session_on_lid_miss() {
        let (client, backend) = memory_client().await;
        let pn = Jid::pn_device("15550002003", 3);
        let lid = Jid::lid_device("100000000000003", 3);
        seed_legacy_pn_session_with_mapping(&client, &pn, &lid, 4245).await;

        assert!(
            backend
                .get_session(lid.to_protocol_address().as_str())
                .await
                .unwrap()
                .is_none(),
            "the fixture must begin with state only in the PN namespace"
        );
        assert!(client.signal().validate_session(&pn).await.unwrap());
        assert_eq!(
            client
                .signal()
                .session_info(&pn)
                .await
                .unwrap()
                .expect("migrated session info")
                .registration_id,
            4245
        );
        assert!(
            backend
                .get_session(pn.to_protocol_address().as_str())
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            backend
                .get_session(lid.to_protocol_address().as_str())
                .await
                .unwrap()
                .is_some()
        );
    }

    #[tokio::test]
    async fn facade_encrypt_retries_after_migrating_a_legacy_pn_session() {
        let (client, backend) = memory_client().await;
        let pn = Jid::pn_device("15550002004", 4);
        let lid = Jid::lid_device("100000000000004", 4);
        seed_legacy_pn_session_with_mapping(&client, &pn, &lid, 4246).await;

        let (_, ciphertext) = client
            .signal()
            .encrypt_message(&pn, b"legacy namespace")
            .await
            .expect("encrypt after lazy migration");
        assert!(!ciphertext.is_empty());
        assert!(
            backend
                .get_session(pn.to_protocol_address().as_str())
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            backend
                .get_session(lid.to_protocol_address().as_str())
                .await
                .unwrap()
                .is_some()
        );
    }

    #[tokio::test]
    async fn delete_sessions_removes_legacy_and_resolved_namespaces() {
        let (client, backend) = memory_client().await;
        let pn = Jid::pn_device("15550002005", 5);
        let lid = Jid::lid_device("100000000000005", 5);
        seed_legacy_pn_session_with_mapping(&client, &pn, &lid, 4247).await;
        let pn_address = pn.to_protocol_address();
        let lid_address = lid.to_protocol_address();

        assert!(
            backend
                .get_session(pn_address.as_str())
                .await
                .unwrap()
                .is_some()
        );
        assert!(
            backend
                .load_identity(pn_address.as_str())
                .await
                .unwrap()
                .is_some()
        );

        client
            .signal()
            .delete_sessions(std::slice::from_ref(&pn))
            .await
            .expect("delete both known namespaces");

        for address in [&pn_address, &lid_address] {
            assert!(
                backend
                    .get_session(address.as_str())
                    .await
                    .unwrap()
                    .is_none()
            );
            assert!(
                backend
                    .load_identity(address.as_str())
                    .await
                    .unwrap()
                    .is_none()
            );
        }
        assert!(!client.signal().validate_session(&pn).await.unwrap());
    }

    #[tokio::test]
    async fn session_info_waits_for_pairwise_mutations() {
        let (client, _) = memory_client().await;
        let peer = Jid::pn_device("15550002001", 2);
        let bundle = peer_prekey_bundle(4243, u32::from(peer.device));

        client
            .signal()
            .install_prekey_bundle(&peer, &bundle)
            .await
            .expect("install bundle");

        let address = peer.to_protocol_address();
        let session_mutex = client.session_lock_for(address.as_str()).await;
        let session_guard = session_mutex.lock().await;
        assert!(
            tokio::time::timeout(
                std::time::Duration::from_millis(100),
                client.signal().session_info(&peer),
            )
            .await
            .is_err(),
            "inspection must not observe a session while a pairwise mutation owns it"
        );

        drop(session_guard);
        assert!(client.signal().session_info(&peer).await.unwrap().is_some());
    }

    #[tokio::test]
    async fn session_migration_reports_moves_for_both_user_namespaces() {
        for (from_server, to_server) in [
            (Server::Pn, Server::Lid),
            (Server::Hosted, Server::HostedLid),
        ] {
            let (client, _) = memory_client().await;
            let from = Jid::new("15550003000", from_server).with_device(3);
            let to = Jid::new("100000000000003", to_server).with_device(3);
            let bundle = peer_prekey_bundle(4343, 3);
            client
                .signal()
                .install_prekey_bundle(&from, &bundle)
                .await
                .expect("install source session");

            let outcome = client
                .signal()
                .migrate_sessions(&from, &to)
                .await
                .expect("migrate session");
            assert_eq!(outcome.migrated, 1);
            assert_eq!(outcome.skipped, 0);
            assert_eq!(outcome.total, 1);
            assert_eq!(outcome.migrated_identities, 1);
            assert_eq!(outcome.discarded_identities, 0);
            assert_eq!(outcome.skipped_identities, 0);
            assert!(outcome.has_state_changes());
            assert!(client.signal().session_info(&from).await.unwrap().is_none());
            assert!(client.signal().session_info(&to).await.unwrap().is_some());
        }
    }

    #[tokio::test]
    async fn session_migration_rejects_mismatched_namespaces() {
        let (client, _) = memory_client().await;
        for (from, to) in [
            (
                Jid::new("15550003000", Server::Pn),
                Jid::new("100000000000003", Server::HostedLid),
            ),
            (
                Jid::new("15550003000", Server::Hosted),
                Jid::new("100000000000003", Server::Lid),
            ),
        ] {
            assert!(
                matches!(
                    client.signal().migrate_sessions(&from, &to).await,
                    Err(SignalError::InvalidInput(_))
                ),
                "mismatched namespace pair {from} -> {to} must be rejected"
            );
        }
    }

    #[tokio::test]
    async fn session_migration_retries_pending_durability_after_flush_failure() {
        let (client, backend) = memory_client().await;
        let from = Jid::new("15550003001", Server::Pn);
        let to = Jid::new("100000000000004", Server::Lid);
        let from_device = from.with_device(4);
        let to_device = to.with_device(4);
        client
            .signal()
            .install_prekey_bundle(&from_device, &peer_prekey_bundle(4344, 4))
            .await
            .expect("install source session");

        backend.set_fail_session_writes(true);
        assert!(
            client.signal().migrate_sessions(&from, &to).await.is_err(),
            "the injected durability failure must reach the caller"
        );
        assert!(
            client
                .signal_cache
                .has_pending_pairwise_writes_for_user(&from.user)
                .await
        );

        backend.set_fail_session_writes(false);
        let attempts_before_retry = backend.session_batch_write_count();
        let retry = client
            .signal()
            .migrate_sessions(&from, &to)
            .await
            .expect("retry pending migration flush");

        assert!(
            !retry.has_state_changes(),
            "the cache already reflects the move"
        );
        assert!(backend.session_batch_write_count() > attempts_before_retry);
        assert!(
            backend
                .get_session(from_device.to_protocol_address().as_str())
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            backend
                .get_session(to_device.to_protocol_address().as_str())
                .await
                .unwrap()
                .is_some()
        );
        assert!(
            !client
                .signal_cache
                .has_pending_pairwise_writes_for_user(&from.user)
                .await
        );
    }

    #[tokio::test]
    async fn sender_key_distribution_roundtrip_uses_shared_store() {
        let (sender, sender_backend) = memory_client().await;
        let (receiver, _) = memory_client().await;
        let group = Jid::new("120363000000000001", Server::Group);
        let author = Jid::new("15550001000", Server::Pn);

        assert!(
            !sender
                .signal()
                .has_sender_key(&group, &author)
                .await
                .unwrap()
        );

        let distribution = sender
            .signal()
            .sender_key_distribution(&group, &author)
            .await
            .expect("create distribution");
        assert!(!distribution.is_empty());
        assert!(
            sender
                .signal()
                .has_sender_key(&group, &author)
                .await
                .unwrap()
        );

        receiver
            .signal()
            .process_sender_key_distribution(&group, &author, &distribution)
            .await
            .expect("process distribution");

        let (_, ciphertext) = sender
            .signal()
            .encrypt_group_message(&group, b"sender-key payload")
            .await
            .expect("group encrypt");
        let plaintext = receiver
            .signal()
            .decrypt_group_message(&group, &author, &ciphertext)
            .await
            .expect("group decrypt");
        assert_eq!(plaintext, b"sender-key payload");

        let sender_key_name = make_sender_key_name(&group, &author.to_protocol_address());
        let chain_lock = sender.signal_cache.sender_key_lock(&sender_key_name).await;
        let chain_guard = chain_lock.lock().await;
        let signal = sender.signal();
        let mut deletion = Box::pin(signal.delete_sender_key(&group, &author));
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(100), &mut deletion)
                .await
                .is_err(),
            "deletion must wait for an in-flight chain mutation"
        );
        drop(chain_guard);
        tokio::time::timeout(std::time::Duration::from_secs(5), deletion)
            .await
            .expect("delete must finish after the chain unlocks")
            .expect("delete sender key");
        assert!(
            !sender
                .signal()
                .has_sender_key(&group, &author)
                .await
                .unwrap()
        );
        assert!(
            sender_backend
                .get_sender_key(sender_key_name.cache_key())
                .await
                .unwrap()
                .is_none(),
            "delete must be durable before returning"
        );
    }

    #[tokio::test]
    async fn group_encrypt_flushes_only_at_sender_key_lease_boundaries() {
        use wacore::libsignal::protocol::consts::SENDER_CHAIN_RESERVATION_BATCH;

        let (client, backend) = memory_client().await;
        let group = Jid::new("120363000000000000", Server::Group);

        let (skdm, _) = client
            .signal()
            .encrypt_group_message(&group, b"first")
            .await
            .expect("first group encrypt");
        assert!(skdm.is_some(), "the first encrypt must create an SKDM");
        assert_eq!(backend.sender_key_batch_write_count(), 1);

        client
            .signal_flush_test_block
            .store(true, Ordering::Release);
        for _ in 1..SENDER_CHAIN_RESERVATION_BATCH {
            let (skdm, _) = client
                .signal()
                .encrypt_group_message(&group, b"warm")
                .await
                .expect("lease-covered group encrypt");
            assert!(skdm.is_none(), "a warm chain must reuse its SKDM");
        }
        assert_eq!(
            backend.sender_key_batch_write_count(),
            1,
            "lease-covered iterations must not flush synchronously"
        );

        client
            .signal()
            .encrypt_group_message(&group, b"boundary")
            .await
            .expect("boundary group encrypt");
        assert_eq!(
            backend.sender_key_batch_write_count(),
            2,
            "raising the next lease must flush before returning ciphertext"
        );
        client
            .signal_flush_test_block
            .store(false, Ordering::Release);
    }

    #[tokio::test]
    async fn group_encrypt_retry_preserves_distribution_after_flush_failure() {
        let (sender, backend) = memory_client().await;
        let (receiver, _) = memory_client().await;
        let group = Jid::new("120363000000000002", Server::Group);
        let author = Jid::new("15550001000", Server::Pn);

        backend.set_fail_sender_key_writes(true);
        assert!(
            sender
                .signal()
                .encrypt_group_message(&group, b"failed attempt")
                .await
                .is_err(),
            "the injected durability failure must reach the caller"
        );

        backend.set_fail_sender_key_writes(false);
        let (distribution, ciphertext) = sender
            .signal()
            .encrypt_group_message(&group, b"retry payload")
            .await
            .expect("retry group encryption");
        let distribution = distribution.expect("retry must retain the pending distribution");
        receiver
            .signal()
            .process_sender_key_distribution(&group, &author, &distribution)
            .await
            .expect("process retained distribution");
        assert_eq!(
            receiver
                .signal()
                .decrypt_group_message(&group, &author, &ciphertext)
                .await
                .expect("decrypt retry ciphertext"),
            b"retry payload"
        );

        let (distribution, _) = sender
            .signal()
            .encrypt_group_message(&group, b"warm payload")
            .await
            .expect("warm group encryption");
        assert!(
            distribution.is_none(),
            "the retained distribution must clear after a successful retry"
        );
    }

    #[tokio::test]
    async fn participant_fanout_reuses_durable_session_leases() {
        let (client, backend) = memory_client().await;
        let recipient = Jid::new("15550002000", Server::Pn);
        client
            .update_device_list(DeviceListRecord {
                user: recipient.user.to_string(),
                devices: vec![DeviceInfo::new(0, None), DeviceInfo::new(1, None)],
                timestamp: wacore::time::now_secs(),
                phash: None,
                raw_id: None,
            })
            .await
            .expect("device registry");

        let devices = [recipient.with_device(0), recipient.with_device(1)];
        for device in &devices {
            seed_peer_session(&client, device).await;
            client
                .signal()
                .encrypt_message(device, b"warm lease")
                .await
                .expect("lease warmup");
        }
        let writes_before = backend.session_batch_write_count();

        client
            .signal_flush_test_block
            .store(true, Ordering::Release);
        let message = waproto::whatsapp::Message {
            conversation: Some("fanout".into()),
            ..Default::default()
        };
        let (nodes, _) = client
            .signal()
            .create_participant_nodes(std::slice::from_ref(&recipient), &message)
            .await
            .expect("participant fanout");
        assert_eq!(nodes.len(), devices.len());
        assert_eq!(
            backend.session_batch_write_count(),
            writes_before,
            "a warm fanout must not flush durable leases synchronously"
        );
        client
            .signal_flush_test_block
            .store(false, Ordering::Release);
    }
}

// Re-export everything from wacore::appstate_sync for backwards compatibility
pub use wacore::appstate::Mutation;
pub use wacore::appstate_sync::{AppStateProcessor, AppStateSyncDriver, AppStateSyncError};

#[cfg(test)]
mod tests {
    use super::*;
    use async_lock::Mutex;
    use async_trait::async_trait;
    use prost::Message;
    use std::collections::HashMap;
    use std::sync::Arc;
    use wacore::appstate::WAPATCH_INTEGRITY;
    use wacore::appstate::hash::HashState;
    use wacore::appstate::hash::generate_content_mac;
    use wacore::appstate::keys::expand_app_state_keys;
    use wacore::appstate::patch_decode::{CollectionSyncError, PatchList, WAPatchName};
    use wacore::appstate::processor::AppStateMutationMAC;
    use wacore::libsignal::crypto::aes_256_cbc_encrypt_into;
    use wacore::store::error::Result as StoreResult;
    use wacore::store::traits::{
        AppStateSyncKey, AppSyncStore, DeviceListRecord, DeviceStore, LidPnMappingEntry,
        MsgSecretStore, ProtocolStore, SignalStore,
    };
    use waproto::whatsapp as wa;

    type MockMacMap = Arc<Mutex<HashMap<(String, Vec<u8>), Vec<u8>>>>;

    #[derive(Default, Clone)]
    struct MockBackend {
        versions: Arc<Mutex<HashMap<String, HashState>>>,
        macs: MockMacMap,
        keys: Arc<Mutex<HashMap<Vec<u8>, AppStateSyncKey>>>,
        latest_key_id: Arc<Mutex<Option<Vec<u8>>>>,
        // Fault injection: when set, clear_mutation_macs fails (transient store error).
        fail_clear_macs: Arc<Mutex<bool>>,
        // Call counters distinguishing the batched MAC prefetch from per-item
        // lookups, so tests can pin which path the processor takes.
        singular_mac_calls: Arc<portable_atomic::AtomicU64>,
        batch_mac_calls: Arc<portable_atomic::AtomicU64>,
    }

    // Implement SignalStore - Signal protocol cryptographic operations
    #[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
    impl SignalStore for MockBackend {
        async fn put_identity(&self, _: &str, _: [u8; 32]) -> StoreResult<()> {
            Ok(())
        }
        async fn load_identity(&self, _: &str) -> StoreResult<Option<[u8; 32]>> {
            Ok(None)
        }
        async fn delete_identity(&self, _: &str) -> StoreResult<()> {
            Ok(())
        }
        async fn get_session(&self, _: &str) -> StoreResult<Option<bytes::Bytes>> {
            Ok(None)
        }
        async fn put_session(&self, _: &str, _: &[u8]) -> StoreResult<()> {
            Ok(())
        }
        async fn delete_session(&self, _: &str) -> StoreResult<()> {
            Ok(())
        }
        async fn store_prekey(&self, _: u32, _: &[u8], _: bool) -> StoreResult<()> {
            Ok(())
        }
        async fn load_prekey(&self, _: u32) -> StoreResult<Option<bytes::Bytes>> {
            Ok(None)
        }
        async fn remove_prekey(&self, _: u32) -> StoreResult<()> {
            Ok(())
        }
        async fn mark_prekeys_uploaded(&self, _: &[u32]) -> StoreResult<()> {
            Ok(())
        }
        async fn get_max_prekey_id(&self) -> StoreResult<u32> {
            Ok(0)
        }
        async fn store_signed_prekey(&self, _: u32, _: &[u8]) -> StoreResult<()> {
            Ok(())
        }
        async fn load_signed_prekey(&self, _: u32) -> StoreResult<Option<Vec<u8>>> {
            Ok(None)
        }
        async fn load_all_signed_prekeys(&self) -> StoreResult<Vec<(u32, Vec<u8>)>> {
            Ok(vec![])
        }
        async fn remove_signed_prekey(&self, _: u32) -> StoreResult<()> {
            Ok(())
        }
        async fn put_sender_key(&self, _: &str, _: &[u8]) -> StoreResult<()> {
            Ok(())
        }
        async fn get_sender_key(&self, _: &str) -> StoreResult<Option<Vec<u8>>> {
            Ok(None)
        }
        async fn delete_sender_key(&self, _: &str) -> StoreResult<()> {
            Ok(())
        }
    }

    // Implement AppSyncStore - WhatsApp app state synchronization
    #[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
    impl AppSyncStore for MockBackend {
        async fn get_sync_key(&self, key_id: &[u8]) -> StoreResult<Option<AppStateSyncKey>> {
            Ok(self.keys.lock().await.get(key_id).cloned())
        }
        async fn set_sync_key(&self, key_id: &[u8], key: AppStateSyncKey) -> StoreResult<()> {
            self.keys.lock().await.insert(key_id.to_vec(), key);
            *self.latest_key_id.lock().await = Some(key_id.to_vec());
            Ok(())
        }
        async fn get_version(&self, name: &str) -> StoreResult<HashState> {
            Ok(self
                .versions
                .lock()
                .await
                .get(name)
                .cloned()
                .unwrap_or_default())
        }
        async fn set_version(&self, name: &str, state: HashState) -> StoreResult<()> {
            self.versions.lock().await.insert(name.to_string(), state);
            Ok(())
        }
        async fn put_mutation_macs(
            &self,
            name: &str,
            _version: u64,
            mutations: &[AppStateMutationMAC],
        ) -> StoreResult<()> {
            let mut macs = self.macs.lock().await;
            for m in mutations {
                macs.insert((name.to_string(), m.index_mac.clone()), m.value_mac.clone());
            }
            Ok(())
        }
        async fn get_mutation_mac(
            &self,
            name: &str,
            index_mac: &[u8],
        ) -> StoreResult<Option<Vec<u8>>> {
            self.singular_mac_calls
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(self
                .macs
                .lock()
                .await
                .get(&(name.to_string(), index_mac.to_vec()))
                .cloned())
        }
        // Real batch override (not the default singular-loop fallback) so the
        // counters can prove which path the processor used.
        async fn get_mutation_macs(
            &self,
            name: &str,
            index_macs: &[Vec<u8>],
        ) -> StoreResult<HashMap<Vec<u8>, Vec<u8>>> {
            self.batch_mac_calls
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let macs = self.macs.lock().await;
            Ok(index_macs
                .iter()
                .filter_map(|index_mac| {
                    macs.get(&(name.to_string(), index_mac.clone()))
                        .map(|mac| (index_mac.clone(), mac.clone()))
                })
                .collect())
        }
        async fn delete_mutation_macs(&self, _: &str, _: &[Vec<u8>]) -> StoreResult<()> {
            Ok(())
        }
        async fn clear_mutation_macs(&self, name: &str) -> StoreResult<()> {
            if *self.fail_clear_macs.lock().await {
                return Err(wacore::store::error::StoreError::Io(std::io::Error::other(
                    "injected clear_mutation_macs failure",
                )));
            }
            self.macs.lock().await.retain(|(n, _), _| n != name);
            Ok(())
        }
        async fn get_latest_sync_key_id(&self) -> StoreResult<Option<Vec<u8>>> {
            Ok(self.latest_key_id.lock().await.clone())
        }
    }

    // Implement ProtocolStore - WhatsApp Web protocol alignment
    #[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
    impl ProtocolStore for MockBackend {
        async fn get_sender_key_devices(&self, _: &str) -> StoreResult<Vec<(String, bool)>> {
            Ok(vec![])
        }
        async fn set_sender_key_status(&self, _: &str, _: &[(&str, bool)]) -> StoreResult<()> {
            Ok(())
        }
        async fn clear_sender_key_devices(&self, _: &str) -> StoreResult<()> {
            Ok(())
        }
        async fn clear_all_sender_key_devices(&self) -> StoreResult<()> {
            Ok(())
        }
        async fn delete_sender_key_device_rows(&self, _: &[&str]) -> StoreResult<()> {
            Ok(())
        }
        async fn get_lid_mapping(&self, _: &str) -> StoreResult<Option<LidPnMappingEntry>> {
            Ok(None)
        }
        async fn get_pn_mapping(&self, _: &str) -> StoreResult<Option<LidPnMappingEntry>> {
            Ok(None)
        }
        async fn put_lid_mapping(&self, _: &LidPnMappingEntry) -> StoreResult<()> {
            Ok(())
        }
        async fn get_all_lid_mappings(&self) -> StoreResult<Vec<LidPnMappingEntry>> {
            Ok(vec![])
        }
        async fn save_base_key(&self, _: &str, _: &str, _: &[u8]) -> StoreResult<()> {
            Ok(())
        }
        async fn has_same_base_key(&self, _: &str, _: &str, _: &[u8]) -> StoreResult<bool> {
            Ok(false)
        }
        async fn delete_base_key(&self, _: &str, _: &str) -> StoreResult<()> {
            Ok(())
        }
        async fn update_device_list(&self, _: DeviceListRecord) -> StoreResult<()> {
            Ok(())
        }
        async fn get_devices(&self, _: &str) -> StoreResult<Option<DeviceListRecord>> {
            Ok(None)
        }
        async fn delete_devices(&self, _: &str) -> StoreResult<()> {
            Ok(())
        }
        async fn get_tc_token(
            &self,
            _: &str,
        ) -> StoreResult<Option<wacore::store::traits::TcTokenEntry>> {
            Ok(None)
        }
        async fn put_tc_token(
            &self,
            _: &str,
            _: &wacore::store::traits::TcTokenEntry,
        ) -> StoreResult<()> {
            Ok(())
        }
        async fn delete_tc_token(&self, _: &str) -> StoreResult<()> {
            Ok(())
        }
        async fn get_all_tc_token_jids(&self) -> StoreResult<Vec<String>> {
            Ok(vec![])
        }
        async fn delete_expired_tc_tokens(&self, _: i64) -> StoreResult<u32> {
            Ok(0)
        }
        async fn store_sent_message(&self, _: &str, _: &str, _: &[u8]) -> StoreResult<()> {
            Ok(())
        }
        async fn take_sent_message(&self, _: &str, _: &str) -> StoreResult<Option<Vec<u8>>> {
            Ok(None)
        }
        async fn delete_expired_sent_messages(&self, _: i64) -> StoreResult<u32> {
            Ok(0)
        }
    }

    #[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
    impl MsgSecretStore for MockBackend {
        async fn put_msg_secrets(
            &self,
            entries: Vec<wacore::store::traits::MsgSecretEntry>,
        ) -> StoreResult<usize> {
            Ok(entries.len())
        }

        async fn get_msg_secret(
            &self,
            _chat: &str,
            _sender: &str,
            _msg_id: &str,
        ) -> StoreResult<Option<Vec<u8>>> {
            Ok(None)
        }

        async fn delete_expired_msg_secrets(&self, _cutoff: i64) -> StoreResult<u32> {
            Ok(0)
        }
    }

    // Implement DeviceStore - Device persistence
    #[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
    impl DeviceStore for MockBackend {
        async fn save(&self, _: &wacore::store::Device) -> StoreResult<()> {
            Ok(())
        }
        async fn load(&self) -> StoreResult<Option<wacore::store::Device>> {
            Ok(Some(wacore::store::Device::new()))
        }
        async fn exists(&self) -> StoreResult<bool> {
            Ok(true)
        }
        async fn create(&self) -> StoreResult<i32> {
            Ok(1)
        }
    }

    fn create_encrypted_mutation(
        op: wa::syncd_mutation::SyncdOperation,
        index_mac: &[u8],
        plaintext: &[u8],
        keys: &wacore::appstate::keys::ExpandedAppStateKeys,
        key_id_bytes: &[u8],
    ) -> wa::SyncdMutation {
        let iv = vec![0u8; 16];

        let mut ciphertext = Vec::new();
        aes_256_cbc_encrypt_into(plaintext, &keys.value_encryption, &iv, &mut ciphertext)
            .expect("AES-CBC encryption should succeed with valid inputs");
        let mut value_with_iv = iv;
        value_with_iv.extend_from_slice(&ciphertext);
        let value_mac = generate_content_mac(op, &value_with_iv, key_id_bytes, &keys.value_mac);
        let mut value_blob = value_with_iv;
        value_blob.extend_from_slice(&value_mac);

        wa::SyncdMutation {
            operation: Some(op as i32),
            record: Some(wa::SyncdRecord {
                index: Some(wa::SyncdIndex {
                    blob: Some(index_mac.to_vec()),
                }),
                value: Some(wa::SyncdValue {
                    blob: Some(value_blob),
                }),
                key_id: Some(wa::KeyId {
                    id: Some(key_id_bytes.to_vec()),
                }),
            }),
        }
    }

    #[tokio::test]
    async fn test_process_patch_list_handles_set_overwrite_correctly() {
        let backend = Arc::new(MockBackend::default());
        let processor =
            AppStateProcessor::new(backend.clone(), Arc::new(crate::runtime_impl::TokioRuntime));
        let collection_name = WAPatchName::Regular;
        let index_mac = vec![1; 32];
        let key_id_bytes = b"test_key_id".to_vec();
        let master_key = [7u8; 32];
        let keys = expand_app_state_keys(&master_key);

        let sync_key = AppStateSyncKey {
            key_data: master_key.to_vec(),
            ..Default::default()
        };
        backend
            .set_sync_key(&key_id_bytes, sync_key)
            .await
            .expect("test backend should accept sync key");

        let original_plaintext = wa::SyncActionData {
            value: Some(wa::SyncActionValue {
                timestamp: Some(1000),
                ..Default::default()
            }),
            ..Default::default()
        }
        .encode_to_vec();
        let original_mutation = create_encrypted_mutation(
            wa::syncd_mutation::SyncdOperation::Set,
            &index_mac,
            &original_plaintext,
            &keys,
            &key_id_bytes,
        );

        let mut initial_state = HashState {
            version: 1,
            ..Default::default()
        };
        let (hash_result, res) =
            initial_state.update_hash(std::slice::from_ref(&original_mutation), |_, _| Ok(None));
        assert!(res.is_ok() && !hash_result.has_missing_remove);
        backend
            .set_version(collection_name.as_str(), initial_state.clone())
            .await
            .expect("test backend should accept app state version");

        let original_value_blob = original_mutation
            .record
            .expect("mutation should have record")
            .value
            .expect("record should have value")
            .blob
            .expect("value should have blob");
        let original_value_mac = original_value_blob[original_value_blob.len() - 32..].to_vec();
        backend
            .put_mutation_macs(
                collection_name.as_str(),
                1,
                &[AppStateMutationMAC {
                    index_mac: index_mac.clone(),
                    value_mac: original_value_mac.clone(),
                }],
            )
            .await
            .expect("test backend should accept mutation MACs");

        let new_plaintext = wa::SyncActionData {
            value: Some(wa::SyncActionValue {
                timestamp: Some(2000),
                ..Default::default()
            }),
            ..Default::default()
        }
        .encode_to_vec();
        let overwrite_mutation = create_encrypted_mutation(
            wa::syncd_mutation::SyncdOperation::Set,
            &index_mac,
            &new_plaintext,
            &keys,
            &key_id_bytes,
        );

        let patch_list = PatchList {
            name: collection_name,
            has_more_patches: false,
            patches: vec![wa::SyncdPatch {
                mutations: vec![overwrite_mutation.clone()],
                version: Some(wa::SyncdVersion { version: Some(2) }),
                key_id: Some(wa::KeyId {
                    id: Some(key_id_bytes),
                }),
                ..Default::default()
            }],
            snapshot: None,
            snapshot_ref: None,
            error: None,
        };

        let result = processor.process_patch_list(patch_list, false).await;

        assert!(
            result.is_ok(),
            "Processing the patch should succeed, but it failed: {:?}",
            result.err()
        );
        let (_, final_state, _) = result.expect("process_patch_list should succeed");

        let mut expected_state = initial_state.clone();
        let new_value_blob = overwrite_mutation
            .record
            .expect("mutation should have record")
            .value
            .expect("record should have value")
            .blob
            .expect("value should have blob");
        let new_value_mac = new_value_blob[new_value_blob.len() - 32..].to_vec();

        WAPATCH_INTEGRITY.subtract_then_add_in_place(
            &mut expected_state.hash,
            &[original_value_mac],
            &[new_value_mac],
        );

        assert_eq!(
            final_state.hash, expected_state.hash,
            "The final LTHash is incorrect, meaning the overwrite was not handled properly."
        );
        assert_eq!(
            final_state.version, 2,
            "The version should be updated to that of the patch."
        );
    }

    /// Builds a snapshot resync (incoming v2 over persisted v1) that carries one
    /// record, after seeding an unrelated stale MAC at v1. Returns the backend,
    /// processor, the patch list, and the stale index MAC that the resync must drop.
    async fn snapshot_resync_scenario() -> (Arc<MockBackend>, AppStateProcessor, PatchList, Vec<u8>)
    {
        let backend = Arc::new(MockBackend::default());
        let processor =
            AppStateProcessor::new(backend.clone(), Arc::new(crate::runtime_impl::TokioRuntime));
        let collection_name = WAPatchName::Regular;
        let key_id_bytes = b"snap_key_id".to_vec();
        let master_key = [9u8; 32];
        let keys = expand_app_state_keys(&master_key);

        backend
            .set_sync_key(
                &key_id_bytes,
                AppStateSyncKey {
                    key_data: master_key.to_vec(),
                    ..Default::default()
                },
            )
            .await
            .expect("test backend should accept sync key");

        backend
            .set_version(
                collection_name.as_str(),
                HashState {
                    version: 1,
                    ..Default::default()
                },
            )
            .await
            .expect("test backend should accept version");

        let stale_index_mac = vec![0xAB; 32];
        backend
            .put_mutation_macs(
                collection_name.as_str(),
                1,
                &[AppStateMutationMAC {
                    index_mac: stale_index_mac.clone(),
                    value_mac: vec![0xCD; 32],
                }],
            )
            .await
            .expect("test backend should accept mutation MACs");

        let plaintext = wa::SyncActionData {
            value: Some(wa::SyncActionValue {
                timestamp: Some(2000),
                ..Default::default()
            }),
            ..Default::default()
        }
        .encode_to_vec();
        let record = create_encrypted_mutation(
            wa::syncd_mutation::SyncdOperation::Set,
            &[0x11; 32],
            &plaintext,
            &keys,
            &key_id_bytes,
        )
        .record
        .expect("mutation should carry a record");

        let patch_list = PatchList {
            name: collection_name,
            has_more_patches: false,
            patches: vec![],
            snapshot: Some(wa::SyncdSnapshot {
                version: Some(wa::SyncdVersion { version: Some(2) }),
                records: vec![record],
                key_id: Some(wa::KeyId {
                    id: Some(key_id_bytes),
                }),
                ..Default::default()
            }),
            snapshot_ref: None,
            error: None,
        };

        (backend, processor, patch_list, stale_index_mac)
    }

    /// Locks the move-and-restore handoff: the snapshot and each patch move into
    /// blocking closures (instead of being deep-cloned for the 'static bound) and
    /// must come back on the returned PatchList, because the caller reads
    /// pl.snapshot/pl.patches afterwards (get_missing_key_ids, has_more bookkeeping).
    #[tokio::test]
    async fn process_patch_list_returns_snapshot_and_patches_to_caller() {
        let (_backend, processor, mut patch_list, _) = snapshot_resync_scenario().await;

        let key_id_bytes = b"snap_key_id".to_vec();
        let master_key = [9u8; 32];
        let keys = expand_app_state_keys(&master_key);
        let plaintext = wa::SyncActionData {
            value: Some(wa::SyncActionValue {
                timestamp: Some(3000),
                ..Default::default()
            }),
            ..Default::default()
        }
        .encode_to_vec();
        for (version, index_mac) in [(3u64, [0x22u8; 32]), (4, [0x33; 32])] {
            let mutation = create_encrypted_mutation(
                wa::syncd_mutation::SyncdOperation::Set,
                &index_mac,
                &plaintext,
                &keys,
                &key_id_bytes,
            );
            patch_list.patches.push(wa::SyncdPatch {
                mutations: vec![mutation],
                version: Some(wa::SyncdVersion {
                    version: Some(version),
                }),
                key_id: Some(wa::KeyId {
                    id: Some(key_id_bytes.clone()),
                }),
                ..Default::default()
            });
        }

        let (mutations, state, pl) = processor
            .process_patch_list(patch_list, false)
            .await
            .expect("snapshot + patches should process");

        assert_eq!(state.version, 4);
        assert_eq!(
            mutations.len(),
            3,
            "snapshot record + one mutation per patch"
        );

        let snapshot = pl.snapshot.as_ref().expect("snapshot handed back");
        assert_eq!(
            snapshot.version.as_ref().and_then(|v| v.version),
            Some(2),
            "the same snapshot must come back, not a substitute"
        );
        assert_eq!(snapshot.records.len(), 1, "snapshot records preserved");

        let patch_versions: Vec<_> = pl
            .patches
            .iter()
            .map(|p| p.version.as_ref().and_then(|v| v.version))
            .collect();
        assert_eq!(
            patch_versions,
            vec![Some(3), Some(4)],
            "patches handed back in processing order"
        );
        let patch_index_macs: Vec<_> = pl
            .patches
            .iter()
            .map(|p| {
                p.mutations[0]
                    .record
                    .as_ref()
                    .and_then(|r| r.index.as_ref())
                    .and_then(|i| i.blob.as_deref())
                    .map(|b| b[0])
            })
            .collect();
        assert_eq!(
            patch_index_macs,
            vec![Some(0x22), Some(0x33)],
            "each patch keeps its own mutations through the handoff"
        );
    }

    /// Locks that build_patch consults the previous value MACs (now via the
    /// batched get_mutation_macs): a SET overwriting an existing index must
    /// produce the subtract-then-add ltHash. If the prefetch wiring broke and
    /// returned nothing, the old MAC would never be subtracted and the
    /// emitted snapshot_mac would diverge.
    #[tokio::test]
    async fn build_patch_subtracts_previous_macs_fetched_in_batch() {
        let backend = Arc::new(MockBackend::default());
        let processor =
            AppStateProcessor::new(backend.clone(), Arc::new(crate::runtime_impl::TokioRuntime));
        let collection_name = WAPatchName::Regular;
        let index_mac = vec![4; 32];
        let key_id_bytes = b"patch_key_id".to_vec();
        let master_key = [11u8; 32];
        let keys = expand_app_state_keys(&master_key);

        backend
            .set_sync_key(
                &key_id_bytes,
                AppStateSyncKey {
                    key_data: master_key.to_vec(),
                    ..Default::default()
                },
            )
            .await
            .expect("test backend should accept sync key");

        let second_index_mac = vec![5; 32];
        let old_value_macs: HashMap<Vec<u8>, Vec<u8>> = HashMap::from([
            (index_mac.clone(), vec![0xAA; 32]),
            (second_index_mac.clone(), vec![0xBB; 32]),
        ]);
        backend
            .set_version(
                collection_name.as_str(),
                HashState {
                    version: 1,
                    ..Default::default()
                },
            )
            .await
            .expect("test backend should accept version");
        backend
            .put_mutation_macs(
                collection_name.as_str(),
                1,
                &old_value_macs
                    .iter()
                    .map(|(index_mac, value_mac)| AppStateMutationMAC {
                        index_mac: index_mac.clone(),
                        value_mac: value_mac.clone(),
                    })
                    .collect::<Vec<_>>(),
            )
            .await
            .expect("test backend should accept mutation MACs");

        let plaintext = wa::SyncActionData {
            value: Some(wa::SyncActionValue {
                timestamp: Some(5000),
                ..Default::default()
            }),
            ..Default::default()
        }
        .encode_to_vec();
        let mutations: Vec<wa::SyncdMutation> = [&index_mac, &second_index_mac]
            .into_iter()
            .map(|mac| {
                create_encrypted_mutation(
                    wa::syncd_mutation::SyncdOperation::Set,
                    mac,
                    &plaintext,
                    &keys,
                    &key_id_bytes,
                )
            })
            .collect();

        let singular_before = backend
            .singular_mac_calls
            .load(std::sync::atomic::Ordering::Relaxed);
        let batch_before = backend
            .batch_mac_calls
            .load(std::sync::atomic::Ordering::Relaxed);

        let (patch_bytes, base_version) = processor
            .build_patch(collection_name.as_str(), mutations.clone())
            .await
            .expect("build_patch should succeed");
        assert_eq!(base_version, 1);

        // The prefetch must be ONE batched round-trip, never per-mutation
        // singular lookups (the N+1 this change removes).
        assert_eq!(
            backend
                .batch_mac_calls
                .load(std::sync::atomic::Ordering::Relaxed)
                - batch_before,
            1,
            "previous MACs must be fetched via a single get_mutation_macs call"
        );
        assert_eq!(
            backend
                .singular_mac_calls
                .load(std::sync::atomic::Ordering::Relaxed)
                - singular_before,
            0,
            "build_patch must not fall back to per-mutation get_mutation_mac"
        );

        // Recompute the expected post-patch state with the seeded prev MACs.
        let mut expected_state = HashState {
            version: 1,
            ..Default::default()
        };
        let (hash_result, res) =
            expected_state.update_hash(&mutations, |mac, _| Ok(old_value_macs.get(mac).cloned()));
        assert!(res.is_ok() && !hash_result.has_missing_remove);
        expected_state.version = 2;
        let expected_snapshot_mac =
            expected_state.generate_snapshot_mac(collection_name.as_str(), &keys.snapshot_mac);

        let patch = wa::SyncdPatch::decode(patch_bytes.as_slice()).expect("patch should decode");
        assert_eq!(
            patch.snapshot_mac.as_deref(),
            Some(expected_snapshot_mac.as_slice()),
            "snapshot_mac must reflect subtract(old)+add(new); a broken prev-MAC prefetch diverges here"
        );
    }

    #[tokio::test]
    async fn snapshot_resync_drops_stale_mutation_macs() {
        let (backend, processor, patch_list, stale_index_mac) = snapshot_resync_scenario().await;

        processor
            .process_patch_list(patch_list, false)
            .await
            .expect("snapshot resync should succeed");

        assert_eq!(
            backend
                .get_mutation_mac(WAPatchName::Regular.as_str(), &stale_index_mac)
                .await
                .unwrap(),
            None,
            "stale MAC from the old baseline must be cleared by the snapshot resync"
        );
        assert_eq!(
            backend
                .get_version(WAPatchName::Regular.as_str())
                .await
                .unwrap()
                .version,
            2
        );
    }

    /// Guards the write-ahead ordering: if clearing MACs fails, the version must
    /// stay at the old baseline so the retry reapplies the snapshot instead of
    /// skipping it as stale.
    #[tokio::test]
    async fn snapshot_resync_keeps_old_version_when_clear_fails() {
        let (backend, processor, patch_list, _) = snapshot_resync_scenario().await;
        *backend.fail_clear_macs.lock().await = true;

        let err = processor.process_patch_list(patch_list, false).await;
        assert!(err.is_err(), "clear failure must abort the resync");

        assert_eq!(
            backend
                .get_version(WAPatchName::Regular.as_str())
                .await
                .unwrap()
                .version,
            1,
            "version must not advance when the MAC reset fails"
        );
    }

    #[tokio::test]
    async fn non_genesis_patch_on_empty_collection_is_retried() {
        let backend = Arc::new(MockBackend::default());
        let processor =
            AppStateProcessor::new(backend.clone(), Arc::new(crate::runtime_impl::TokioRuntime));

        // Empty collection (version 0), patches without a snapshot, first patch v5.
        let patch_list = PatchList {
            name: WAPatchName::Regular,
            has_more_patches: false,
            patches: vec![wa::SyncdPatch {
                version: Some(wa::SyncdVersion { version: Some(5) }),
                ..Default::default()
            }],
            snapshot: None,
            snapshot_ref: None,
            error: None,
        };

        let (mutations, state, pl) = processor
            .process_patch_list(patch_list, true)
            .await
            .expect("guard returns Ok with a retryable error, not a hard failure");

        assert!(
            mutations.is_empty(),
            "the unanchored patch must not be applied"
        );
        assert_eq!(
            state.version, 0,
            "version stays 0 so the refetch re-requests a snapshot"
        );
        assert!(matches!(pl.error, Some(CollectionSyncError::Retry { .. })));
    }
}

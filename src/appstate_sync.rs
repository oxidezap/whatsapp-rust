// Re-export everything from wacore::appstate_sync for backwards compatibility
pub use wacore::appstate::Mutation;
pub use wacore::appstate_sync::{AppStateProcessor, AppStateSyncError};

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;
    use async_lock::Mutex;
    use async_trait::async_trait;
    use buffa::Message;
    use std::collections::HashMap;
    use std::sync::Arc;
    use wacore::appstate::WAPATCH_INTEGRITY;
    use wacore::appstate::hash::HashState;
    use wacore::appstate::hash::{generate_content_mac, generate_patch_mac};
    use wacore::appstate::keys::{ExpandedAppStateKeys, expand_app_state_keys};
    use wacore::appstate::patch_decode::{CollectionSyncError, PatchList, WAPatchName};
    use wacore::appstate::processor::AppStateMutationMAC;
    use wacore::appstate_sync::RecoveryOutcome;
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
        // Counts key reads, so a test can tell "before the recovery started"
        // from "part-way through it".
        sync_key_calls: Arc<portable_atomic::AtomicU64>,
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
            self.sync_key_calls
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(self.keys.lock().await.get(key_id).cloned())
        }
        async fn set_sync_key(&self, key_id: &[u8], key: AppStateSyncKey) -> StoreResult<()> {
            self.keys.lock().await.insert(key_id.to_vec(), key);
            *self.latest_key_id.lock().await = Some(key_id.to_vec());
            Ok(())
        }
        async fn get_version(&self, name: &str) -> StoreResult<Option<HashState>> {
            Ok(self.versions.lock().await.get(name).cloned())
        }
        async fn delete_version(&self, name: &str) -> StoreResult<()> {
            self.versions.lock().await.remove(name);
            Ok(())
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
            index_macs: &[[u8; 32]],
        ) -> StoreResult<HashMap<[u8; 32], Vec<u8>>> {
            self.batch_mac_calls
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let macs = self.macs.lock().await;
            Ok(index_macs
                .iter()
                .filter_map(|index_mac| {
                    macs.get(&(name.to_string(), index_mac.to_vec()))
                        .map(|mac| (*index_mac, mac.clone()))
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
        async fn delete_expired_tc_tokens(&self, _: i64, _: i64) -> StoreResult<u32> {
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
        keys: &ExpandedAppStateKeys,
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
            operation: Some(op.into()),
            record: buffa::MessageField::some(wa::SyncdRecord {
                index: buffa::MessageField::some(wa::SyncdIndex {
                    blob: Some(index_mac.to_vec()),
                }),
                value: buffa::MessageField::some(wa::SyncdValue {
                    blob: Some(value_blob),
                }),
                key_id: buffa::MessageField::some(wa::KeyId {
                    id: Some(key_id_bytes.to_vec()),
                }),
            }),
        }
    }

    /// Builds the reply a primary device sends back, with one record per index.
    fn recovery_of(
        collection: &str,
        version: u64,
        lthash: [u8; 128],
        key_id: &[u8],
        records: &[(&str, [u8; 32])],
    ) -> wa::SyncdSnapshotRecovery {
        wa::SyncdSnapshotRecovery {
            version: buffa::MessageField::some(wa::SyncdVersion {
                version: Some(version),
            }),
            collection_name: Some(collection.to_string()),
            collection_lthash: Some(lthash.to_vec()),
            mutation_records: records
                .iter()
                .map(|(index_json, value_mac)| wa::SyncdPlainTextRecord {
                    value: buffa::MessageField::some(wa::SyncActionData {
                        index: Some(index_json.as_bytes().to_vec()),
                        version: Some(version as i32),
                        // Carried, because it is the whole point: every consumer
                        // event is built from `action_value`, so a regression
                        // that dropped it would pass a suite asserting only the
                        // index and the operation.
                        value: buffa::MessageField::some(wa::SyncActionValue {
                            mute_action: buffa::MessageField::some(
                                wa::sync_action_value::MuteAction {
                                    muted: Some(true),
                                    ..Default::default()
                                },
                            ),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    key_id: Some(key_id.to_vec()),
                    mac: Some(value_mac.to_vec()),
                })
                .collect(),
        }
    }

    /// The version and the ltHash are the primary's, written as given.
    ///
    /// Folding the records to check them would be this side recomputing the very
    /// thing it just failed to agree about, which is the whole reason the phone
    /// was asked. What is derived locally is only each index MAC, from the index
    /// the record carries in the clear -- the same split WA Web makes.
    #[tokio::test]
    async fn snapshot_recovery_writes_what_the_primary_sent() {
        let backend = Arc::new(MockBackend::default());
        let processor =
            AppStateProcessor::new(backend.clone(), Arc::new(crate::runtime_impl::TokioRuntime));
        let key_id = b"test_key_id".to_vec();
        let master_key = [7u8; 32];
        let keys = expand_app_state_keys(&master_key);
        backend
            .set_sync_key(
                &key_id,
                AppStateSyncKey {
                    key_data: master_key.to_vec(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        // A collection this side had already given up on: stuck, and marked so.
        backend
            .set_version(
                "regular_low",
                HashState {
                    version: 0,
                    mac_mismatch_fatal: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let lthash = [0x5Au8; 128];
        let recovery = recovery_of(
            "regular_low",
            253,
            lthash,
            &key_id,
            &[(r#"["mute","1@s.whatsapp.net"]"#, [0x11; 32])],
        );

        let outcome = processor
            .apply_snapshot_recovery(recovery, "regular_low", &|| true)
            .await
            .expect("a recovery for the collection that was asked for applies");
        let RecoveryOutcome::Applied(mutations) = outcome else {
            panic!("a collection stuck at v0 is not ahead of anything: {outcome:?}");
        };

        assert_eq!(mutations.len(), 1);
        assert_eq!(mutations[0].index, vec!["mute", "1@s.whatsapp.net"]);
        let action = mutations[0]
            .action_value
            .as_ref()
            .expect("the record's payload survives into the mutation");
        assert_eq!(
            action.mute_action.as_option().and_then(|m| m.muted),
            Some(true),
            "every consumer event is built from this; losing it would ship silently"
        );
        assert_eq!(
            mutations[0].operation,
            wa::syncd_mutation::SyncdOperation::SET,
            "a recovered collection is what exists; there is nothing left to remove"
        );

        let stored = backend
            .get_version("regular_low")
            .await
            .unwrap()
            .expect("the collection is written");
        assert_eq!(
            stored.version, 253,
            "the primary's version, not a computed one"
        );
        assert_eq!(stored.hash, lthash, "and the primary's ltHash, verbatim");
        assert!(stored.bootstrapped, "a recovered collection has a baseline");
        assert!(
            !stored.mac_mismatch_fatal,
            "the snapshot that could not be validated is not the collection we now hold"
        );

        // The MAC store has to be repopulated, or the next patch that overwrites
        // an index has nothing to subtract.
        let expected_index_mac = wacore::appstate::hash::generate_index_mac(
            br#"["mute","1@s.whatsapp.net"]"#,
            &keys.index,
        );
        let got = backend
            .get_mutation_mac("regular_low", &expected_index_mac)
            .await
            .unwrap();
        assert_eq!(
            got.as_deref(),
            Some(&[0x11u8; 32][..]),
            "the record's own MAC is the value MAC; nothing is re-encrypted to get one"
        );
    }

    /// Nothing waits for the reply, so the collection can move on while the
    /// phone is answering. A recovery that lands behind where this side already
    /// is must be discarded, not applied: rolling the version back would undo
    /// real state and leave the next patch conflicting with a baseline nobody
    /// is at.
    #[tokio::test]
    async fn a_recovery_that_arrives_behind_the_collection_is_discarded() {
        let backend = Arc::new(MockBackend::default());
        let processor =
            AppStateProcessor::new(backend.clone(), Arc::new(crate::runtime_impl::TokioRuntime));

        // A later sync got through while the phone was answering.
        let current = HashState {
            version: 260,
            hash: [0x77; 128],
            ..Default::default()
        };
        backend
            .set_version("regular_low", current.clone())
            .await
            .unwrap();

        let recovery = recovery_of("regular_low", 253, [0x5A; 128], b"k", &[]);
        let outcome = processor
            .apply_snapshot_recovery(recovery, "regular_low", &|| true)
            .await
            .expect("a stale recovery is discarded, not an error");

        assert!(
            matches!(
                outcome,
                RecoveryOutcome::Stale {
                    held: 260,
                    offered: 253
                }
            ),
            "{outcome:?}"
        );

        let stored = backend.get_version("regular_low").await.unwrap().unwrap();
        assert_eq!(stored.version, 260, "the newer version stands");
        assert_eq!(stored.hash, current.hash, "and so does its ltHash");
    }

    /// The name the payload claims, against the name the ask was made about.
    /// The caller resolves the latter from the request id, so this comparison is
    /// what stops a reply carrying one collection's id and another's name from
    /// overwriting a collection nobody asked about. WA Web compares the two the
    /// same way.
    #[tokio::test]
    async fn snapshot_recovery_for_another_collection_is_refused() {
        let backend = Arc::new(MockBackend::default());
        let processor =
            AppStateProcessor::new(backend.clone(), Arc::new(crate::runtime_impl::TokioRuntime));

        let recovery = recovery_of("regular", 5, [0u8; 128], b"k", &[]);
        let err = processor
            .apply_snapshot_recovery(recovery, "regular_low", &|| true)
            .await
            .expect_err("a recovery naming another collection must be refused");
        assert!(err.to_string().contains("regular_low"), "{err}");

        assert!(
            backend.get_version("regular").await.unwrap().is_none(),
            "and nothing is written for the collection it did name"
        );
    }

    /// The MAC store keeps one value per index, so a collection that names an
    /// index twice persists one of them. Dispatching the loser beside the winner
    /// would hand a consumer state the persisted collection does not describe --
    /// and delivery is concurrent, so it could be the one that sticks. The
    /// snapshot path picks the last record per index; so does this.
    #[tokio::test]
    async fn a_repeated_index_in_a_recovery_dispatches_only_its_last_record() {
        let backend = Arc::new(MockBackend::default());
        let processor =
            AppStateProcessor::new(backend.clone(), Arc::new(crate::runtime_impl::TokioRuntime));
        let key_id = b"test_key_id".to_vec();
        let master_key = [7u8; 32];
        let keys = expand_app_state_keys(&master_key);
        backend
            .set_sync_key(
                &key_id,
                AppStateSyncKey {
                    key_data: master_key.to_vec(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        // A second key, so the test can also pin what "the same index" means.
        let other_key_id = b"another_key_id".to_vec();
        let other_master_key = [9u8; 32];
        let other_keys = expand_app_state_keys(&other_master_key);
        backend
            .set_sync_key(
                &other_key_id,
                AppStateSyncKey {
                    key_data: other_master_key.to_vec(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let index = r#"["mute","1@s.whatsapp.net"]"#;
        let mut recovery = recovery_of(
            "regular_low",
            253,
            [0x5A; 128],
            &key_id,
            // The same index twice, a second index that must survive, and the
            // first index again under another key.
            &[
                (index, [0x11; 32]),
                (index, [0x22; 32]),
                (r#"["mute","2@s.whatsapp.net"]"#, [0x33; 32]),
                (index, [0x44; 32]),
            ],
        );
        // The loser carries a payload the winner does not, so a regression that
        // dispatched the wrong one of the pair is visible rather than merely
        // mis-ordered: without this the two differ only in a MAC the mutation
        // does not carry.
        recovery.mutation_records[0]
            .value
            .as_option_mut()
            .and_then(|data| data.value.as_option_mut())
            .and_then(|value| value.mute_action.as_option_mut())
            .expect("the helper builds a mute action")
            .muted = Some(false);
        // Same index, different key -- so a different index MAC and a different
        // row in the store. It is not the pair's duplicate and must survive.
        recovery.mutation_records[3].key_id = Some(other_key_id.clone());

        let outcome = processor
            .apply_snapshot_recovery(recovery, "regular_low", &|| true)
            .await
            .expect("a recovery with a repeated index still applies");
        let RecoveryOutcome::Applied(mutations) = outcome else {
            panic!("{outcome:?}");
        };

        assert_eq!(
            mutations.len(),
            3,
            "the losing record is dropped and the other-key record is kept: {mutations:?}"
        );
        assert_eq!(mutations[0].index, vec!["mute", "1@s.whatsapp.net"]);
        assert_eq!(mutations[1].index, vec!["mute", "2@s.whatsapp.net"]);
        assert_eq!(mutations[2].index, vec!["mute", "1@s.whatsapp.net"]);
        assert_eq!(
            mutations[0]
                .action_value
                .as_ref()
                .and_then(|v| v.mute_action.as_option())
                .and_then(|m| m.muted),
            Some(true),
            "the winner's payload is what reaches the consumer, not the loser's"
        );

        // And what the store kept is the winner's MAC, which is what the next
        // patch overwriting this index has to subtract.
        let index_mac = wacore::appstate::hash::generate_index_mac(index.as_bytes(), &keys.index);
        let got = backend
            .get_mutation_mac("regular_low", &index_mac)
            .await
            .unwrap();
        assert_eq!(
            got.as_deref(),
            Some(&[0x22u8; 32][..]),
            "the last record for the index is the one that stands"
        );

        // And the other key's record has its own row, which is why it is not a
        // duplicate: collapsing the two would leave this one out of the ltHash
        // the primary just handed over.
        let other_index_mac =
            wacore::appstate::hash::generate_index_mac(index.as_bytes(), &other_keys.index);
        assert_ne!(
            other_index_mac, index_mac,
            "a different key, a different MAC"
        );
        assert_eq!(
            backend
                .get_mutation_mac("regular_low", &other_index_mac)
                .await
                .unwrap()
                .as_deref(),
            Some(&[0x44u8; 32][..]),
            "the same index under another key keeps its own row"
        );
    }

    /// The reservation this write runs under is a connection's, and a disconnect
    /// drops the whole registry -- so a recovery that is still doing its key
    /// lookups and its HMACs when one happens no longer excludes the new
    /// connection's sync from the rows it is about to clear and rewrite.
    #[tokio::test]
    async fn a_recovery_whose_connection_goes_writes_nothing() {
        let backend = Arc::new(MockBackend::default());
        let processor =
            AppStateProcessor::new(backend.clone(), Arc::new(crate::runtime_impl::TokioRuntime));
        let key_id = b"test_key_id".to_vec();
        let master_key = [7u8; 32];
        backend
            .set_sync_key(
                &key_id,
                AppStateSyncKey {
                    key_data: master_key.to_vec(),
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        // Where the collection stood when the recovery started: stuck, and the
        // state the new connection's sync would be reading and moving.
        let held = HashState {
            version: 0,
            mac_mismatch_fatal: true,
            ..Default::default()
        };
        backend.set_version("regular_low", held).await.unwrap();

        let recovery = recovery_of(
            "regular_low",
            253,
            [0x5A; 128],
            &key_id,
            &[(r#"["mute","1@s.whatsapp.net"]"#, [0x11; 32])],
        );

        // True when the recovery starts and false by the time it is asked, which
        // is the window this guard exists for -- a predicate that was already
        // false would have been caught by the caller's own check and would say
        // nothing about where inside the apply the second one happens. The key
        // read is the first thing the apply does that reaches the store, so
        // "a key has been fetched" is "we are past the reads and the record
        // work".
        let reads = Arc::clone(&backend.sync_key_calls);
        let still_current = move || reads.load(std::sync::atomic::Ordering::Relaxed) == 0;
        assert!(
            still_current(),
            "the connection is live when the recovery begins"
        );

        let outcome = processor
            .apply_snapshot_recovery(recovery, "regular_low", &still_current)
            .await
            .expect("a retired recovery is dropped, not an error");
        assert!(matches!(outcome, RecoveryOutcome::Retired), "{outcome:?}");

        let stored = backend.get_version("regular_low").await.unwrap().unwrap();
        assert_eq!(stored.version, 0, "the version is left where it was");
        assert!(
            stored.mac_mismatch_fatal,
            "and so is the flag that will have it asked for again"
        );
        let keys = expand_app_state_keys(&master_key);
        let index_mac = wacore::appstate::hash::generate_index_mac(
            br#"["mute","1@s.whatsapp.net"]"#,
            &keys.index,
        );
        assert!(
            backend
                .get_mutation_mac("regular_low", &index_mac)
                .await
                .unwrap()
                .is_none(),
            "nothing reached the MAC store either"
        );
    }

    /// One reply per request, and none at all for a request nobody made.
    #[tokio::test]
    async fn a_recovery_request_is_taken_once() {
        let backend = Arc::new(MockBackend::default());
        let processor =
            AppStateProcessor::new(backend.clone(), Arc::new(crate::runtime_impl::TokioRuntime));

        assert!(
            !processor.take_recovery_request("regular_low").await,
            "an unsolicited reply is not one this side is waiting for"
        );

        assert!(
            processor.mark_recovery_requested("regular_low").await,
            "the first ask is a new one"
        );
        assert!(
            !processor.mark_recovery_requested("regular_low").await,
            "a second ask while one is outstanding is suppressed: the reply already coming answers it"
        );
        assert!(processor.take_recovery_request("regular_low").await);
        assert!(
            !processor.take_recovery_request("regular_low").await,
            "a second reply to one request would apply a collection twice"
        );

        // And an answer is claimed, not merely recognised: a response repeating
        // its result, or a second copy of it, would otherwise each inflate and
        // decode a whole collection against the one ask.
        assert!(processor.mark_recovery_requested("regular_low").await);
        processor
            .note_recovery_request_id("regular_low", "req-1")
            .await;
        assert_eq!(
            processor.claim_recovery_request_by_id("req-1").await,
            Some("regular_low".to_string()),
            "the first copy of the answer takes the ask up"
        );
        assert_eq!(
            processor.claim_recovery_request_by_id("req-1").await,
            None,
            "a repeat of it does not get a second decode"
        );
        assert_eq!(
            processor.take_recovery_request_by_id("req-1").await,
            Some("regular_low".to_string()),
            "and the claim does not stop the answer from spending its request"
        );
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
            value: buffa::MessageField::some(wa::SyncActionValue {
                timestamp: Some(1000),
                ..Default::default()
            }),
            ..Default::default()
        }
        .encode_to_vec();
        let original_mutation = create_encrypted_mutation(
            wa::syncd_mutation::SyncdOperation::SET,
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
            .into_option()
            .expect("mutation should have record")
            .value
            .into_option()
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
            value: buffa::MessageField::some(wa::SyncActionValue {
                timestamp: Some(2000),
                ..Default::default()
            }),
            ..Default::default()
        }
        .encode_to_vec();
        let overwrite_mutation = create_encrypted_mutation(
            wa::syncd_mutation::SyncdOperation::SET,
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
                version: buffa::MessageField::some(wa::SyncdVersion { version: Some(2) }),
                key_id: buffa::MessageField::some(wa::KeyId {
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
            .into_option()
            .expect("mutation should have record")
            .value
            .into_option()
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

    /// The processor's key cache is bounded, so a patch list that references
    /// more keys than it holds must still apply in full: the keys a list needs
    /// come from the backend, and the cache only decides what the *next* list
    /// gets to reuse.
    #[tokio::test]
    async fn a_patch_list_referencing_more_keys_than_the_cache_holds_still_applies() {
        const KEYS: usize = 40;

        let backend = Arc::new(MockBackend::default());
        let processor =
            AppStateProcessor::new(backend.clone(), Arc::new(crate::runtime_impl::TokioRuntime));

        let mut patches = Vec::with_capacity(KEYS);
        for i in 0..KEYS {
            let key_id = format!("key-{i:02}").into_bytes();
            let master_key = [i as u8; 32];
            backend
                .set_sync_key(
                    &key_id,
                    AppStateSyncKey {
                        key_data: master_key.to_vec(),
                        ..Default::default()
                    },
                )
                .await
                .expect("test backend should accept sync key");
            let keys = expand_app_state_keys(&master_key);
            let plaintext = wa::SyncActionData {
                value: buffa::MessageField::some(wa::SyncActionValue {
                    timestamp: Some(1000 + i as i64),
                    ..Default::default()
                }),
                ..Default::default()
            }
            .encode_to_vec();
            let mutation = create_encrypted_mutation(
                wa::syncd_mutation::SyncdOperation::SET,
                &[i as u8; 32],
                &plaintext,
                &keys,
                &key_id,
            );
            patches.push(wa::SyncdPatch {
                mutations: vec![mutation],
                version: buffa::MessageField::some(wa::SyncdVersion {
                    version: Some(i as u64 + 1),
                }),
                key_id: buffa::MessageField::some(wa::KeyId { id: Some(key_id) }),
                ..Default::default()
            });
        }
        let patch_list = PatchList {
            name: WAPatchName::Regular,
            has_more_patches: false,
            patches,
            snapshot: None,
            snapshot_ref: None,
            error: None,
        };

        let (mutations, state, _) = processor
            .process_patch_list(patch_list, false)
            .await
            .expect("every key is in the backend, so every patch applies");

        assert_eq!(mutations.len(), KEYS);
        assert_eq!(state.version, KEYS as u64);
        assert!(
            processor.cached_key_count().await < KEYS,
            "the cache holds fewer keys than this list referenced; that eviction \
             is what the list had to survive"
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
            value: buffa::MessageField::some(wa::SyncActionValue {
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
                version: buffa::MessageField::some(wa::SyncdVersion { version: Some(2) }),
                records: vec![record],
                key_id: buffa::MessageField::some(wa::KeyId {
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
        let (backend, processor, mut patch_list, _) = snapshot_resync_scenario().await;

        let key_id_bytes = b"snap_key_id".to_vec();
        let master_key = [9u8; 32];
        let keys = expand_app_state_keys(&master_key);
        let plaintext = wa::SyncActionData {
            value: buffa::MessageField::some(wa::SyncActionValue {
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
                version: buffa::MessageField::some(wa::SyncdVersion {
                    version: Some(version),
                }),
                key_id: buffa::MessageField::some(wa::KeyId {
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
        // Load-bearing beyond this test: the batched sync stopped writing the
        // returned state back, on the strength of the processor having persisted
        // it already. A second write there lands after a reconnect may have let
        // the replacement connection move the collection on, putting the older
        // version back next to newer mutation MACs.
        assert_eq!(
            backend
                .get_version(WAPatchName::Regular.as_str())
                .await
                .unwrap()
                .expect("the collection has a version record")
                .version,
            state.version,
            "the state handed back must be the state already persisted"
        );
        assert_eq!(
            mutations.len(),
            3,
            "snapshot record + one mutation per patch"
        );

        let snapshot = pl.snapshot.as_ref().expect("snapshot handed back");
        assert_eq!(
            snapshot.version.as_option().and_then(|v| v.version),
            Some(2),
            "the same snapshot must come back, not a substitute"
        );
        assert_eq!(snapshot.records.len(), 1, "snapshot records preserved");

        let patch_versions: Vec<_> = pl
            .patches
            .iter()
            .map(|p| p.version.as_option().and_then(|v| v.version))
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
                    .as_option()
                    .and_then(|r| r.index.as_option())
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
            value: buffa::MessageField::some(wa::SyncActionValue {
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

        let patch =
            wa::SyncdPatch::decode_from_slice(patch_bytes.as_slice()).expect("patch should decode");
        assert_eq!(
            patch.snapshot_mac.as_deref(),
            Some(expected_snapshot_mac.as_slice()),
            "snapshot_mac must reflect subtract(old)+add(new); a broken prev-MAC prefetch diverges here"
        );
    }

    /// The stale-snapshot guard stops a snapshot from rewinding a collection,
    /// and nothing lifts it — a rebuild stands the collection down to unsynced
    /// first, so the snapshot it then receives is never measured against a
    /// version at all.
    #[tokio::test]
    async fn a_snapshot_at_the_persisted_version_is_discarded() {
        let (backend, processor, patch_list, stale_index_mac) = snapshot_resync_scenario().await;
        // The snapshot is v2; persisting v2 makes it exactly as new, which the
        // guard treats as stale.
        backend
            .set_version(
                WAPatchName::Regular.as_str(),
                HashState {
                    version: 2,
                    ..Default::default()
                },
            )
            .await
            .expect("test backend should accept version");

        processor
            .process_patch_list(patch_list, false)
            .await
            .expect("a discarded snapshot is not an error");

        assert!(
            backend
                .get_mutation_mac(WAPatchName::Regular.as_str(), &stale_index_mac)
                .await
                .unwrap()
                .is_some(),
            "the snapshot must not have been applied"
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
                .expect("the collection has a version record")
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
                .expect("the collection has a version record")
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
                version: buffa::MessageField::some(wa::SyncdVersion { version: Some(5) }),
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

    // Companion to snapshot_resync_drops_stale_mutation_macs: a collection whose
    // version blob reset to 0 (e.g. an old bincode row that no longer decodes) keeps
    // its pre-reset mutation MACs on disk. When the v0 resync arrives as a genesis
    // patch (v1) WITHOUT a snapshot, those stale MACs must be wiped before the patch
    // runs, or its ltHash anchors to index->value entries that aren't part of the
    // fresh baseline.
    #[tokio::test]
    async fn genesis_patch_on_reset_collection_drops_stale_mutation_macs() {
        let backend = Arc::new(MockBackend::default());
        let processor =
            AppStateProcessor::new(backend.clone(), Arc::new(crate::runtime_impl::TokioRuntime));
        let name = WAPatchName::Regular;

        // Reset collection: version 0 / empty hash, but stale MACs still present.
        backend
            .set_version(name.as_str(), HashState::default())
            .await
            .unwrap();
        let stale_index_mac = vec![0xAB; 32];
        backend
            .put_mutation_macs(
                name.as_str(),
                7,
                &[AppStateMutationMAC {
                    index_mac: stale_index_mac.clone(),
                    value_mac: vec![0xCD; 32],
                }],
            )
            .await
            .unwrap();

        // A genesis patch (v1) served without a snapshot.
        let patch_list = PatchList {
            name,
            has_more_patches: false,
            patches: vec![wa::SyncdPatch {
                version: buffa::MessageField::some(wa::SyncdVersion { version: Some(1) }),
                ..Default::default()
            }],
            snapshot: None,
            snapshot_ref: None,
            error: None,
        };

        processor
            .process_patch_list(patch_list, false)
            .await
            .expect("genesis patch onto a reset collection should process");

        assert_eq!(
            backend
                .get_mutation_mac(name.as_str(), &stale_index_mac)
                .await
                .unwrap(),
            None,
            "a genesis-patch resync onto a reset collection must clear the stale pre-reset MACs"
        );
    }

    // The SNAPSHOT's key_id lives INSIDE its external blob, so get_missing_key_ids on
    // the un-inlined list can't see it. missing_key_ids_after_inline must download and
    // inline the blob first, so an absent snapshot key is requested up front instead of
    // aborting mid-process with KeyNotFound (the regression a paired companion hit when
    // its snapshot key was absent after the bincode->prost reset).
    #[tokio::test]
    async fn missing_key_ids_after_inline_sees_external_snapshot_key() {
        let backend = Arc::new(MockBackend::default());
        let processor =
            AppStateProcessor::new(backend.clone(), Arc::new(crate::runtime_impl::TokioRuntime));

        let snapshot_key_id = b"snapshot-key-xyz".to_vec();
        let snapshot_bytes = wa::SyncdSnapshot {
            key_id: buffa::MessageField::some(wa::KeyId {
                id: Some(snapshot_key_id.clone()),
            }),
            ..Default::default()
        }
        .encode_to_vec();
        let direct_path = "/snapshot/blob".to_string();

        let mut pl = PatchList {
            name: WAPatchName::Regular,
            has_more_patches: false,
            patches: vec![],
            snapshot: None,
            snapshot_ref: Some(wa::ExternalBlobReference {
                direct_path: Some(direct_path),
                ..Default::default()
            }),
            error: None,
        };

        let snapshot_bytes = bytes::Bytes::from(snapshot_bytes);
        let download = |_ext: &wa::ExternalBlobReference| -> anyhow::Result<bytes::Bytes> {
            Ok(snapshot_bytes.clone())
        };

        // Before inlining, the external snapshot's key is invisible.
        assert!(
            processor.get_missing_key_ids(&pl).await.unwrap().is_empty(),
            "the snapshot key is inside the un-downloaded blob, so it can't be seen yet"
        );

        // After inlining, the absent snapshot key is reported so it gets requested.
        let missing = processor
            .missing_key_ids_after_inline(&mut pl, &download)
            .await
            .unwrap();
        assert_eq!(
            missing,
            vec![snapshot_key_id],
            "the snapshot's key must be requestable after inlining the blob"
        );
    }

    // ─── #1156: a collection whose ltHash diverged must still converge ───────
    //
    // A patch carries two aggregate MACs. `patchMac` covers the patch's own
    // bytes under the app-state key, so it proves the patch came from a device
    // that holds the key — the server cannot forge it. `snapshotMac` covers the
    // ltHash the SENDER held after applying the patch, so it only agrees when
    // the receiver's base is byte-identical to the sender's.
    //
    // Once a receiver's ltHash diverges, every later patch fails the snapshotMac
    // comparison forever, whatever its origin. WA Web
    // (`WAWebSyncdAntiTampering`, fn `z`) treats that case as degradation, not
    // tampering: on a snapshotMac mismatch raised from a PATCH it persists
    // `isCollectionInMacMismatchFatal` for the collection, logs
    // "skip fatal after snapshot mac mismatch", and keeps applying — and every
    // later patch short-circuits at `if (E && k) return null`, skipping the
    // comparison entirely. Only a mismatch raised from a SNAPSHOT is fatal
    // (it escalates to peer snapshot recovery).
    //
    // These two tests pin the whole-pipeline half of that: a divergent patch
    // reaches `process_patch_list`, applies, advances the persisted version,
    // and leaves the latch on disk. The pure-validation half is pinned in
    // `wacore-appstate`'s own tests.

    /// Sign `patch` the way a diverged peer signs one: a correct `patchMac`
    /// (the patch really is authentic), over a `snapshotMac` computed from
    /// `foreign_hash` — an ltHash this client does not share.
    fn sign_patch_over_foreign_base(
        patch: &mut wa::SyncdPatch,
        keys: &ExpandedAppStateKeys,
        collection: &str,
        version: u64,
        foreign_hash: [u8; 128],
    ) {
        let foreign = HashState {
            version,
            hash: foreign_hash,
            ..Default::default()
        };
        patch.snapshot_mac = Some(foreign.generate_snapshot_mac(collection, &keys.snapshot_mac));
        // patchMac is computed over snapshot_mac, so it must be stamped last.
        patch.patch_mac = Some(generate_patch_mac(
            patch,
            collection,
            &keys.patch_mac,
            version,
        ));
    }

    /// A mutation that survives `validate_macs = true`: the record's index blob
    /// is the HMAC of the index identity bytes, and the encrypted payload
    /// carries the same identity, so `decode_record`'s index-MAC check agrees.
    fn validating_mutation(
        index: &[u8],
        timestamp: i64,
        keys: &ExpandedAppStateKeys,
        key_id: &[u8],
    ) -> wa::SyncdMutation {
        let plaintext = wa::SyncActionData {
            index: Some(index.to_vec()),
            value: buffa::MessageField::some(wa::SyncActionValue {
                timestamp: Some(timestamp),
                ..Default::default()
            }),
            ..Default::default()
        }
        .encode_to_vec();
        create_encrypted_mutation(
            wa::syncd_mutation::SyncdOperation::SET,
            &wacore::appstate::hash::generate_index_mac(index, &keys.index),
            &plaintext,
            keys,
            key_id,
        )
    }

    /// The steady-state shape of #1156: the collection is at v5 with an ltHash
    /// nobody else computes, and the server keeps serving the authentic patches
    /// that follow. Every one of them must apply — refusing them only freezes
    /// the collection at the base already proven wrong, which is the reported
    /// "114 identical failures, forever" loop.
    #[tokio::test]
    async fn diverged_collection_keeps_applying_authentic_patches() {
        let backend = Arc::new(MockBackend::default());
        let processor =
            AppStateProcessor::new(backend.clone(), Arc::new(crate::runtime_impl::TokioRuntime));
        let name = WAPatchName::RegularLow;
        let key_id = b"diverged_key_id".to_vec();
        let master_key = [3u8; 32];
        let keys = expand_app_state_keys(&master_key);

        backend
            .set_sync_key(
                &key_id,
                AppStateSyncKey {
                    key_data: master_key.to_vec(),
                    ..Default::default()
                },
            )
            .await
            .expect("test backend should accept sync key");

        // Diverged base: a non-empty ltHash at v5 that no peer would compute.
        backend
            .set_version(
                name.as_str(),
                HashState {
                    version: 5,
                    hash: [0x11; 128],
                    ..Default::default()
                },
            )
            .await
            .expect("test backend should accept version");

        for (version, index_mac, foreign_hash) in [
            (6u64, [0x21u8; 32], [0x99u8; 128]),
            (7, [0x22; 32], [0x9A; 128]),
        ] {
            let mut patch = wa::SyncdPatch {
                version: buffa::MessageField::some(wa::SyncdVersion {
                    version: Some(version),
                }),
                mutations: vec![validating_mutation(
                    &index_mac,
                    version as i64 * 1000,
                    &keys,
                    &key_id,
                )],
                key_id: buffa::MessageField::some(wa::KeyId {
                    id: Some(key_id.clone()),
                }),
                ..Default::default()
            };
            sign_patch_over_foreign_base(&mut patch, &keys, name.as_str(), version, foreign_hash);

            let patch_list = PatchList {
                name,
                has_more_patches: false,
                patches: vec![patch],
                snapshot: None,
                snapshot_ref: None,
                error: None,
            };

            let (mutations, state, _) = processor
                .process_patch_list(patch_list, true)
                .await
                .unwrap_or_else(|e| {
                    panic!(
                        "v{version} carries a valid patchMac, so it is authentic and must apply \
                         even though the local ltHash diverged: {e:#}"
                    )
                });

            assert_eq!(mutations.len(), 1, "v{version} mutation must be dispatched");
            assert_eq!(state.version, version);
            let persisted = backend
                .get_version(name.as_str())
                .await
                .expect("version readable")
                .expect("the collection has a version record");
            assert_eq!(
                persisted.version, version,
                "the collection must advance, or the next sync re-requests the same patch"
            );
            assert!(
                persisted.mac_mismatch_fatal,
                "the latch must be persisted with the version, or a restart re-detects \
                 the divergence on v{version} and every patch after it"
            );
        }
    }

    /// The second half of #1156, from the issue's follow-up comment: resetting
    /// the collection and re-fetching the snapshot recovers the base, but the
    /// server's post-snapshot window can still contain a patch the diverged
    /// client itself pushed. The snapshot validates and the trailing patch does
    /// not, so today the error propagates after the snapshot was persisted:
    /// the trailing patch's mutations are dropped and the collection never
    /// passes the cut, re-fetching the same snapshot forever.
    #[tokio::test]
    async fn poisoned_trailing_patch_does_not_strand_a_valid_snapshot() {
        let backend = Arc::new(MockBackend::default());
        let processor =
            AppStateProcessor::new(backend.clone(), Arc::new(crate::runtime_impl::TokioRuntime));
        let name = WAPatchName::RegularLow;
        let key_id = b"trailing_key_id".to_vec();
        let master_key = [4u8; 32];
        let keys = expand_app_state_keys(&master_key);

        backend
            .set_sync_key(
                &key_id,
                AppStateSyncKey {
                    key_data: master_key.to_vec(),
                    ..Default::default()
                },
            )
            .await
            .expect("test backend should accept sync key");

        // A legitimately signed snapshot at v10 (the reset-and-refetch result).
        let record = validating_mutation(&[0x31; 32], 10_000, &keys, &key_id)
            .record
            .into_option()
            .expect("mutation carries a record");
        let mut snapshot_state = HashState {
            version: 10,
            ..Default::default()
        };
        snapshot_state.update_hash_from_records(std::slice::from_ref(&record));
        let snapshot = wa::SyncdSnapshot {
            version: buffa::MessageField::some(wa::SyncdVersion { version: Some(10) }),
            records: vec![record],
            key_id: buffa::MessageField::some(wa::KeyId {
                id: Some(key_id.clone()),
            }),
            mac: Some(snapshot_state.generate_snapshot_mac(name.as_str(), &keys.snapshot_mac)),
        };

        // The trailing patch the diverged client pushed before it was reset.
        let mut trailing = wa::SyncdPatch {
            version: buffa::MessageField::some(wa::SyncdVersion { version: Some(11) }),
            mutations: vec![validating_mutation(&[0x32; 32], 11_000, &keys, &key_id)],
            key_id: buffa::MessageField::some(wa::KeyId {
                id: Some(key_id.clone()),
            }),
            ..Default::default()
        };
        sign_patch_over_foreign_base(&mut trailing, &keys, name.as_str(), 11, [0x77; 128]);

        let patch_list = PatchList {
            name,
            has_more_patches: false,
            patches: vec![trailing],
            snapshot: Some(snapshot),
            snapshot_ref: None,
            error: None,
        };

        let (mutations, state, _) = processor
            .process_patch_list(patch_list, true)
            .await
            .unwrap_or_else(|e| {
                panic!("a poisoned trailing patch must not strand a valid snapshot: {e:#}")
            });

        assert_eq!(
            mutations.len(),
            2,
            "the snapshot record and the trailing patch's mutation must both dispatch"
        );
        assert_eq!(state.version, 11);
        assert_eq!(
            backend
                .get_version(name.as_str())
                .await
                .expect("version readable")
                .expect("the collection has a version record")
                .version,
            11,
            "the collection must pass the cut, or every later sync repeats the snapshot"
        );
    }
}

use crate::AppStateError;
use crate::hash::{generate_content_mac, validate_index_mac};
use crate::keys::ExpandedAppStateKeys;
use buffa::MessageView;
use wacore_libsignal::crypto::aes_256_cbc_decrypt_into;
use waproto::whatsapp as wa;

/// A decoded mutation from an app state record.
#[derive(Debug, Clone)]
pub struct Mutation {
    /// The decoded action value.
    pub action_value: Option<wa::SyncActionValue>,
    /// The MAC of the index.
    pub index_mac: Vec<u8>,
    /// The MAC of the value.
    pub value_mac: Vec<u8>,
    /// The parsed index components (JSON array of strings).
    pub index: Vec<String>,
    /// The operation type (Set or Remove).
    pub operation: wa::syncd_mutation::SyncdOperation,
}

/// Decode a single encrypted record into a mutation.
///
/// This is a pure, synchronous function that takes the expanded keys directly,
/// avoiding any async key lookup.
///
/// # Arguments
/// * `operation` - The operation type (Set or Remove)
/// * `record` - The encrypted SyncdRecord to decode
/// * `keys` - The pre-expanded app state keys for decryption
/// * `key_id` - The key ID used for MAC validation
/// * `validate_macs` - Whether to validate MACs during decoding
///
/// # Returns
/// A decoded `Mutation` or an error if decoding/validation fails.
pub fn decode_record(
    operation: wa::syncd_mutation::SyncdOperation,
    record: &wa::SyncdRecord,
    keys: &ExpandedAppStateKeys,
    key_id: &[u8],
    validate_macs: bool,
) -> Result<Mutation, AppStateError> {
    let value_blob = record
        .value
        .blob
        .as_ref()
        .ok_or(AppStateError::MissingValueBlob)?;

    if value_blob.len() < 16 + 32 {
        return Err(AppStateError::ValueBlobTooShort);
    }

    let (iv, rest) = value_blob.split_at(16);
    let (ciphertext, value_mac) = rest.split_at(rest.len() - 32);

    if validate_macs {
        let expected = generate_content_mac(
            operation,
            &value_blob[..value_blob.len() - 32],
            key_id,
            &keys.value_mac,
        );
        if expected != value_mac {
            return Err(AppStateError::MismatchingContentMAC);
        }
    }

    let mut plaintext = Vec::new();
    aes_256_cbc_decrypt_into(ciphertext, &keys.value_encryption, iv, &mut plaintext)
        .map_err(|_| AppStateError::DecryptionFailed)?;

    let action = wa::SyncActionDataView::decode_view(plaintext.as_slice())
        .map_err(|_| AppStateError::DecodeFailed)?;

    let mut index_list: Vec<String> = Vec::new();
    if let Some(idx_bytes) = action.index {
        if validate_macs {
            let stored = record
                .index
                .blob
                .as_ref()
                .ok_or(AppStateError::MissingIndexMAC)?;
            validate_index_mac(idx_bytes, stored, &keys.index)?;
        }
        if let Ok(parsed) = serde_json::from_slice::<Vec<String>>(idx_bytes) {
            index_list = parsed;
        }
    }

    Ok(Mutation {
        action_value: action.value.as_option().map(MessageView::to_owned_message),
        index_mac: record.index.blob.clone().unwrap_or_default(),
        value_mac: value_mac.to_vec(),
        index: index_list,
        operation,
    })
}

/// Extract all unique key IDs from a patch list that need to be fetched.
///
/// This is a pure function that collects key IDs from snapshots and patches
/// without checking against storage.
pub fn collect_key_ids_from_patch_list(
    snapshot: Option<&wa::SyncdSnapshot>,
    patches: &[wa::SyncdPatch],
) -> Vec<Vec<u8>> {
    collect_key_id_refs_from_patch_list(snapshot, patches)
        .into_iter()
        .map(<[u8]>::to_vec)
        .collect()
}

/// Borrowing variant for callers that only need to look up keys immediately.
pub fn collect_key_id_refs_from_patch_list<'a>(
    snapshot: Option<&'a wa::SyncdSnapshot>,
    patches: &'a [wa::SyncdPatch],
) -> Vec<&'a [u8]> {
    fn push_unique<'a>(key_ids: &mut Vec<&'a [u8]>, key_id: Option<&'a Vec<u8>>) {
        if let Some(k) = key_id {
            let k = k.as_slice();
            if !key_ids.contains(&k) {
                key_ids.push(k);
            }
        }
    }

    let mut key_ids = Vec::new();

    if let Some(snapshot) = snapshot {
        push_unique(&mut key_ids, snapshot.key_id.id.as_ref());
        for rec in &snapshot.records {
            push_unique(&mut key_ids, rec.key_id.id.as_ref());
        }
    }

    for patch in patches {
        push_unique(&mut key_ids, patch.key_id.id.as_ref());
        for mutation in &patch.mutations {
            if mutation.record.is_set() {
                push_unique(&mut key_ids, mutation.record.key_id.id.as_ref());
            }
        }
    }

    key_ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::generate_content_mac;
    use crate::keys::expand_app_state_keys;
    use buffa::Message;
    use wacore_libsignal::crypto::{CryptographicMac, aes_256_cbc_encrypt_into};

    fn create_test_record(
        op: wa::syncd_mutation::SyncdOperation,
        keys: &ExpandedAppStateKeys,
        key_id: &[u8],
        action_data: &wa::SyncActionData,
    ) -> wa::SyncdRecord {
        let plaintext = action_data.encode_to_vec();
        let iv = vec![0u8; 16];
        let mut ciphertext = Vec::new();
        aes_256_cbc_encrypt_into(&plaintext, &keys.value_encryption, &iv, &mut ciphertext)
            .expect("test encryption should succeed");

        let mut value_with_iv = iv;
        value_with_iv.extend_from_slice(&ciphertext);
        let value_mac = generate_content_mac(op, &value_with_iv, key_id, &keys.value_mac);
        let mut value_blob = value_with_iv;
        value_blob.extend_from_slice(&value_mac);

        let index_blob = action_data
            .index
            .as_ref()
            .map(|index| {
                let mut mac = CryptographicMac::new("HmacSha256", &keys.index)
                    .expect("HmacSha256 is a valid algorithm");
                mac.update(index);
                mac.finalize()
            })
            .unwrap_or_else(|| vec![1; 32]);

        wa::SyncdRecord {
            index: buffa::MessageField::some(wa::SyncdIndex {
                blob: Some(index_blob),
            }),
            value: buffa::MessageField::some(wa::SyncdValue {
                blob: Some(value_blob),
            }),
            key_id: buffa::MessageField::some(wa::KeyId {
                id: Some(key_id.to_vec()),
            }),
        }
    }

    #[test]
    fn test_decode_record_basic() {
        let master_key = [7u8; 32];
        let keys = expand_app_state_keys(&master_key);
        let key_id = b"test_key_id".to_vec();

        let action_data = wa::SyncActionData {
            value: buffa::MessageField::some(wa::SyncActionValue {
                timestamp: Some(1234567890),
                ..Default::default()
            }),
            ..Default::default()
        };

        let record = create_test_record(
            wa::syncd_mutation::SyncdOperation::SET,
            &keys,
            &key_id,
            &action_data,
        );

        let mutation = decode_record(
            wa::syncd_mutation::SyncdOperation::SET,
            &record,
            &keys,
            &key_id,
            false, // skip MAC validation for this test
        )
        .expect("test encryption should succeed");

        assert_eq!(
            mutation.action_value.as_ref().and_then(|v| v.timestamp),
            Some(1234567890)
        );
        assert_eq!(mutation.operation, wa::syncd_mutation::SyncdOperation::SET);
    }

    #[test]
    fn test_decode_record_with_mac_validation() {
        let master_key = [7u8; 32];
        let keys = expand_app_state_keys(&master_key);
        let key_id = b"test_key_id".to_vec();

        let action_data = wa::SyncActionData {
            value: buffa::MessageField::some(wa::SyncActionValue {
                timestamp: Some(1234567890),
                ..Default::default()
            }),
            ..Default::default()
        };

        let record = create_test_record(
            wa::syncd_mutation::SyncdOperation::SET,
            &keys,
            &key_id,
            &action_data,
        );

        // With MAC validation enabled but no index in action_data, should succeed
        let result = decode_record(
            wa::syncd_mutation::SyncdOperation::SET,
            &record,
            &keys,
            &key_id,
            true,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn test_decode_record_view_preserves_index_and_value_with_mac_validation() {
        let master_key = [7u8; 32];
        let keys = expand_app_state_keys(&master_key);
        let key_id = b"test_key_id".to_vec();
        let index = serde_json::to_vec(&vec!["regular", "chat", "15550000001@s.whatsapp.net"])
            .expect("test index should serialize");

        let action_data = wa::SyncActionData {
            index: Some(index),
            value: buffa::MessageField::some(wa::SyncActionValue {
                timestamp: Some(1234567890),
                push_name_setting: buffa::MessageField::some(
                    wa::sync_action_value::PushNameSetting {
                        name: Some("Test User".to_string()),
                    },
                ),
                ..Default::default()
            }),
            padding: Some(vec![0; 4]),
            version: Some(1),
        };

        let record = create_test_record(
            wa::syncd_mutation::SyncdOperation::SET,
            &keys,
            &key_id,
            &action_data,
        );

        let mutation = decode_record(
            wa::syncd_mutation::SyncdOperation::SET,
            &record,
            &keys,
            &key_id,
            true,
        )
        .expect("view-backed decode should preserve action data");

        assert_eq!(
            mutation.index,
            vec!["regular", "chat", "15550000001@s.whatsapp.net"]
        );
        assert_eq!(
            mutation.action_value.as_ref().and_then(|v| v.timestamp),
            Some(1234567890)
        );
        assert_eq!(
            mutation
                .action_value
                .as_ref()
                .and_then(|v| v.push_name_setting.as_option())
                .and_then(|p| p.name.as_deref()),
            Some("Test User")
        );
        assert_eq!(
            mutation.index_mac,
            record.index.blob.clone().unwrap_or_default()
        );
    }

    #[test]
    fn test_collect_key_ids_from_patch_list() {
        let key_id_1 = vec![1, 2, 3];
        let key_id_2 = vec![4, 5, 6];
        let key_id_3 = vec![7, 8, 9];
        let key_id_4 = vec![10, 11, 12];

        let snapshot = wa::SyncdSnapshot {
            key_id: buffa::MessageField::some(wa::KeyId {
                id: Some(key_id_1.clone()),
            }),
            records: vec![wa::SyncdRecord {
                key_id: buffa::MessageField::some(wa::KeyId {
                    id: Some(key_id_2.clone()),
                }),
                ..Default::default()
            }],
            ..Default::default()
        };

        let patches = vec![wa::SyncdPatch {
            key_id: buffa::MessageField::some(wa::KeyId {
                id: Some(key_id_3.clone()),
            }),
            mutations: vec![wa::SyncdMutation {
                record: buffa::MessageField::some(wa::SyncdRecord {
                    key_id: buffa::MessageField::some(wa::KeyId {
                        id: Some(key_id_4.clone()),
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            }],
            ..Default::default()
        }];

        let key_ids = collect_key_ids_from_patch_list(Some(&snapshot), &patches);

        assert_eq!(key_ids.len(), 4);
        assert!(key_ids.contains(&key_id_1));
        assert!(key_ids.contains(&key_id_2));
        assert!(key_ids.contains(&key_id_3));
        assert!(key_ids.contains(&key_id_4));
    }

    #[test]
    fn test_collect_key_ids_deduplicates() {
        let key_id = vec![1, 2, 3];

        let snapshot = wa::SyncdSnapshot {
            key_id: buffa::MessageField::some(wa::KeyId {
                id: Some(key_id.clone()),
            }),
            records: vec![wa::SyncdRecord {
                key_id: buffa::MessageField::some(wa::KeyId {
                    id: Some(key_id.clone()),
                }),
                ..Default::default()
            }],
            ..Default::default()
        };

        let patches = vec![wa::SyncdPatch {
            key_id: buffa::MessageField::some(wa::KeyId {
                id: Some(key_id.clone()),
            }),
            ..Default::default()
        }];

        let key_ids = collect_key_ids_from_patch_list(Some(&snapshot), &patches);

        // Should only have one entry since all key IDs are the same
        assert_eq!(key_ids.len(), 1);
        assert_eq!(key_ids[0], key_id);
    }
}

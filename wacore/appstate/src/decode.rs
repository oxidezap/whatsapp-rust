use crate::AppStateError;
use crate::hash::{generate_content_mac, validate_index_mac};
use crate::keys::ExpandedAppStateKeys;
use prost::Message;
use wacore_libsignal::crypto::aes_256_cbc_decrypt_into;
use waproto::whatsapp as wa;

/// A decoded mutation from an app state record.
#[derive(Debug, Clone)]
pub struct Mutation {
    /// The decoded action value.
    pub action_value: Option<wa::SyncActionValue>,
    /// The parsed index components (JSON array of strings).
    pub index: Vec<String>,
    /// The operation type (Set or Remove).
    pub operation: wa::syncd_mutation::SyncdOperation,
}

/// Index/value MACs extracted from a record, returned alongside the decoded
/// [`Mutation`]. Kept out of `Mutation` so the MACs live only in the persisted
/// MAC list rather than being duplicated on every returned mutation.
#[derive(Debug, Clone)]
pub struct RecordMacs {
    pub index_mac: Vec<u8>,
    pub value_mac: Vec<u8>,
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
/// The decoded `Mutation` together with its index/value MACs, or an error if
/// decoding/validation fails.
pub fn decode_record(
    operation: wa::syncd_mutation::SyncdOperation,
    record: &wa::SyncdRecord,
    keys: &ExpandedAppStateKeys,
    key_id: &[u8],
    validate_macs: bool,
) -> Result<(Mutation, RecordMacs), AppStateError> {
    let value_blob = record
        .value
        .as_ref()
        .and_then(|v| v.blob.as_ref())
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

    let action = wa::SyncActionData::decode(plaintext.as_slice())
        .map_err(|_| AppStateError::DecodeFailed)?;

    // WA Web (syncdDecryptMutation) computes the index MAC unconditionally over the
    // decoded index (empty buffer when the field is absent) and rejects on mismatch,
    // so an absent index must still match the stored MAC rather than bypass the check.
    if validate_macs {
        let stored = record
            .index
            .as_ref()
            .and_then(|i| i.blob.as_ref())
            .ok_or(AppStateError::MissingIndexMAC)?;
        validate_index_mac(action.index.as_deref().unwrap_or(&[]), stored, &keys.index)?;
    }

    let mut index_list: Vec<String> = Vec::new();
    if let Some(idx_bytes) = action.index.as_ref()
        && let Ok(parsed) = serde_json::from_slice::<Vec<String>>(idx_bytes)
    {
        index_list = parsed;
    }

    // A record without an index MAC is malformed; never persist an empty MAC
    // (previously unwrap_or_default() let this through when validate_macs=false).
    let index_mac = record
        .index
        .as_ref()
        .and_then(|i| i.blob.clone())
        .ok_or(AppStateError::MissingIndexMAC)?;
    Ok((
        Mutation {
            action_value: action.value,
            index: index_list,
            operation,
        },
        RecordMacs {
            index_mac,
            value_mac: value_mac.to_vec(),
        },
    ))
}

/// Extract all unique key IDs from a patch list that need to be fetched.
///
/// This is a pure function that collects key IDs from snapshots and patches
/// without checking against storage.
pub fn collect_key_ids_from_patch_list(
    snapshot: Option<&wa::SyncdSnapshot>,
    patches: &[wa::SyncdPatch],
) -> Vec<Vec<u8>> {
    use std::collections::HashSet;

    let mut seen = HashSet::new();
    let mut key_ids = Vec::new();

    let mut check = |key_id: Option<&Vec<u8>>| {
        if let Some(k) = key_id
            && !seen.contains(k.as_slice())
        {
            // Unique key ID: two owned buffers are allocated via k.clone() and
            // owned.clone() — one stored in `seen` for future dedup checks, one
            // pushed to `key_ids` as the result. Duplicate key IDs are skipped
            // by the seen.contains() check above, avoiding any allocation.
            let owned = k.clone();
            seen.insert(owned.clone());
            key_ids.push(owned);
        }
    };

    if let Some(snapshot) = snapshot {
        check(snapshot.key_id.as_ref().and_then(|k| k.id.as_ref()));
        for rec in &snapshot.records {
            check(rec.key_id.as_ref().and_then(|k| k.id.as_ref()));
        }
    }

    for patch in patches {
        check(patch.key_id.as_ref().and_then(|k| k.id.as_ref()));
        for mutation in &patch.mutations {
            if let Some(record) = &mutation.record {
                check(record.key_id.as_ref().and_then(|k| k.id.as_ref()));
            }
        }
    }

    key_ids
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::{generate_content_mac, generate_index_mac};
    use crate::keys::expand_app_state_keys;
    use prost::Message;
    use wacore_libsignal::crypto::aes_256_cbc_encrypt_into;

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

        let index_bytes = action_data.index.as_deref().unwrap_or(&[]);
        wa::SyncdRecord {
            index: Some(wa::SyncdIndex {
                blob: Some(generate_index_mac(index_bytes, &keys.index)),
            }),
            value: Some(wa::SyncdValue {
                blob: Some(value_blob),
            }),
            key_id: Some(wa::KeyId {
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
            value: Some(wa::SyncActionValue {
                timestamp: Some(1234567890),
                ..Default::default()
            }),
            ..Default::default()
        };

        let record = create_test_record(
            wa::syncd_mutation::SyncdOperation::Set,
            &keys,
            &key_id,
            &action_data,
        );

        let (mutation, macs) = decode_record(
            wa::syncd_mutation::SyncdOperation::Set,
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
        assert_eq!(mutation.operation, wa::syncd_mutation::SyncdOperation::Set);
        // MACs are returned separately and must carry the real bytes, not empty
        // or swapped values: index_mac is the HMAC of the (absent here) index.
        assert_eq!(macs.index_mac, generate_index_mac(&[], &keys.index));
        assert!(!macs.value_mac.is_empty());
        assert_ne!(macs.index_mac, macs.value_mac);
    }

    #[test]
    fn test_decode_record_with_mac_validation() {
        let master_key = [7u8; 32];
        let keys = expand_app_state_keys(&master_key);
        let key_id = b"test_key_id".to_vec();

        let action_data = wa::SyncActionData {
            value: Some(wa::SyncActionValue {
                timestamp: Some(1234567890),
                ..Default::default()
            }),
            ..Default::default()
        };

        let record = create_test_record(
            wa::syncd_mutation::SyncdOperation::Set,
            &keys,
            &key_id,
            &action_data,
        );

        // No index field, but the stored index MAC matches the empty-index HMAC: passes.
        let result = decode_record(
            wa::syncd_mutation::SyncdOperation::Set,
            &record,
            &keys,
            &key_id,
            true,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn index_mac_is_validated_even_when_index_field_absent() {
        let master_key = [7u8; 32];
        let keys = expand_app_state_keys(&master_key);
        let key_id = b"test_key_id".to_vec();

        let action_data = wa::SyncActionData {
            value: Some(wa::SyncActionValue {
                timestamp: Some(1234567890),
                ..Default::default()
            }),
            ..Default::default()
        };
        let mut record = create_test_record(
            wa::syncd_mutation::SyncdOperation::Set,
            &keys,
            &key_id,
            &action_data,
        );
        // Tamper the stored index MAC: with no index field the old code skipped the
        // check entirely and accepted this; WA Web (and now we) reject it.
        record.index = Some(wa::SyncdIndex {
            blob: Some(vec![0xFF; 32]),
        });

        let err = decode_record(
            wa::syncd_mutation::SyncdOperation::Set,
            &record,
            &keys,
            &key_id,
            true,
        )
        .unwrap_err();
        assert!(matches!(err, AppStateError::MismatchingIndexMAC));
    }

    #[test]
    fn test_collect_key_ids_from_patch_list() {
        let key_id_1 = vec![1, 2, 3];
        let key_id_2 = vec![4, 5, 6];
        let key_id_3 = vec![7, 8, 9];
        let key_id_4 = vec![10, 11, 12];

        let snapshot = wa::SyncdSnapshot {
            key_id: Some(wa::KeyId {
                id: Some(key_id_1.clone()),
            }),
            records: vec![wa::SyncdRecord {
                key_id: Some(wa::KeyId {
                    id: Some(key_id_2.clone()),
                }),
                ..Default::default()
            }],
            ..Default::default()
        };

        let patches = vec![wa::SyncdPatch {
            key_id: Some(wa::KeyId {
                id: Some(key_id_3.clone()),
            }),
            mutations: vec![wa::SyncdMutation {
                record: Some(wa::SyncdRecord {
                    key_id: Some(wa::KeyId {
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
            key_id: Some(wa::KeyId {
                id: Some(key_id.clone()),
            }),
            records: vec![wa::SyncdRecord {
                key_id: Some(wa::KeyId {
                    id: Some(key_id.clone()),
                }),
                ..Default::default()
            }],
            ..Default::default()
        };

        let patches = vec![wa::SyncdPatch {
            key_id: Some(wa::KeyId {
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

use crate::AppStateError;
use crate::hash::{generate_content_mac, validate_index_mac};
use crate::keys::ExpandedAppStateKeys;
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

    // Owned decode (not a view): the `value` sub-message is needed owned, so a
    // view would parse it once into a view and copy it again into the owned
    // form — two passes over the largest field. Owned decode does it in one.
    // Via waproto::codec so this crate doesn't instantiate the decode tree.
    let action = waproto::codec::sync_action_data_decode(plaintext.as_slice())
        .map_err(|_| AppStateError::DecodeFailed)?;

    // WA Web (syncdDecryptMutation) computes the index MAC unconditionally over the
    // decoded index (empty buffer when the field is absent) and rejects on mismatch,
    // so an absent index must still match the stored MAC rather than bypass the check.
    if validate_macs {
        let stored = record
            .index
            .as_option()
            .and_then(|i| i.blob.as_ref())
            .ok_or(AppStateError::MissingIndexMAC)?;
        validate_index_mac(action.index.as_deref().unwrap_or(&[]), stored, &keys.index)?;
    }

    let mut index_list: Vec<String> = Vec::new();
    if let Some(idx_bytes) = action.index.as_deref()
        && let Ok(parsed) = serde_json::from_slice::<Vec<String>>(idx_bytes)
    {
        index_list = parsed;
    }

    // A record without an index MAC is malformed; never persist an empty MAC
    // (previously unwrap_or_default() let this through when validate_macs=false).
    let index_mac = record
        .index
        .blob
        .clone()
        .ok_or(AppStateError::MissingIndexMAC)?;
    Ok((
        Mutation {
            action_value: action.value.into_option(),
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
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;
    use crate::hash::{generate_content_mac, generate_index_mac};
    use crate::keys::expand_app_state_keys;
    use buffa::Message;
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
            index: buffa::MessageField::some(wa::SyncdIndex {
                blob: Some(generate_index_mac(index_bytes, &keys.index)),
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

        let (mutation, macs) = decode_record(
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

        // No index field, but the stored index MAC matches the empty-index HMAC: passes.
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
    fn index_mac_is_validated_even_when_index_field_absent() {
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
        let mut record = create_test_record(
            wa::syncd_mutation::SyncdOperation::SET,
            &keys,
            &key_id,
            &action_data,
        );
        // Tamper the stored index MAC: with no index field the old code skipped the
        // check entirely and accepted this; WA Web (and now we) reject it.
        record.index = buffa::MessageField::some(wa::SyncdIndex {
            blob: Some(vec![0xFF; 32]),
        });

        let err = decode_record(
            wa::syncd_mutation::SyncdOperation::SET,
            &record,
            &keys,
            &key_id,
            true,
        )
        .unwrap_err();
        assert!(matches!(err, AppStateError::MismatchingIndexMAC));
    }

    // ── Negative cases ──────────────────────────────────────────────────────
    //
    // Every input here is server-controlled, so each rejection branch of
    // `decode_record` needs a test: a silently-accepted malformed record either
    // corrupts local app state or lets a tampered mutation through.

    const MASTER_KEY: [u8; 32] = [7u8; 32];

    fn test_keys() -> (ExpandedAppStateKeys, Vec<u8>) {
        (expand_app_state_keys(&MASTER_KEY), b"test_key_id".to_vec())
    }

    fn valid_action_data() -> wa::SyncActionData {
        wa::SyncActionData {
            index: Some(br#"["mute","1555550100@s.whatsapp.net"]"#.to_vec()),
            value: buffa::MessageField::some(wa::SyncActionValue {
                timestamp: Some(1234567890),
                ..Default::default()
            }),
            ..Default::default()
        }
    }

    /// Build a record whose value blob wraps `plaintext` verbatim, so a test can
    /// hand `decode_record` bytes that decrypt fine but aren't a SyncActionData.
    fn record_from_plaintext(
        op: wa::syncd_mutation::SyncdOperation,
        keys: &ExpandedAppStateKeys,
        key_id: &[u8],
        plaintext: &[u8],
        index_bytes: &[u8],
    ) -> wa::SyncdRecord {
        let iv = vec![0u8; 16];
        let mut ciphertext = Vec::new();
        aes_256_cbc_encrypt_into(plaintext, &keys.value_encryption, &iv, &mut ciphertext)
            .expect("test encryption should succeed");

        let mut value_with_iv = iv;
        value_with_iv.extend_from_slice(&ciphertext);
        let value_mac = generate_content_mac(op, &value_with_iv, key_id, &keys.value_mac);
        let mut value_blob = value_with_iv;
        value_blob.extend_from_slice(&value_mac);

        wa::SyncdRecord {
            index: buffa::MessageField::some(wa::SyncdIndex {
                blob: Some(generate_index_mac(index_bytes, &keys.index)),
            }),
            value: buffa::MessageField::some(wa::SyncdValue {
                blob: Some(value_blob),
            }),
            key_id: buffa::MessageField::some(wa::KeyId {
                id: Some(key_id.to_vec()),
            }),
        }
    }

    /// Rewrite a record's value blob through `f` (MessageField has no DerefMut).
    fn map_value_blob(record: &mut wa::SyncdRecord, f: impl FnOnce(&mut Vec<u8>)) {
        let mut blob = record
            .value
            .as_option()
            .and_then(|v| v.blob.clone())
            .expect("fixture record has a value blob");
        f(&mut blob);
        record.value = buffa::MessageField::some(wa::SyncdValue { blob: Some(blob) });
    }

    fn decode(record: &wa::SyncdRecord, validate_macs: bool) -> Result<Mutation, AppStateError> {
        let (keys, key_id) = test_keys();
        decode_record(
            wa::syncd_mutation::SyncdOperation::SET,
            record,
            &keys,
            &key_id,
            validate_macs,
        )
        .map(|(mutation, _)| mutation)
    }

    #[test]
    fn decode_record_rejects_missing_value_blob() {
        let (keys, key_id) = test_keys();
        let mut record = create_test_record(
            wa::syncd_mutation::SyncdOperation::SET,
            &keys,
            &key_id,
            &valid_action_data(),
        );
        record.value = buffa::MessageField::some(wa::SyncdValue { blob: None });

        assert!(matches!(
            decode(&record, true).unwrap_err(),
            AppStateError::MissingValueBlob
        ));
    }

    #[test]
    fn decode_record_rejects_truncated_value_blob() {
        let (keys, key_id) = test_keys();
        let mut record = create_test_record(
            wa::syncd_mutation::SyncdOperation::SET,
            &keys,
            &key_id,
            &valid_action_data(),
        );

        // One byte short of the IV + MAC floor: the split_at calls below would
        // otherwise panic instead of erroring.
        map_value_blob(&mut record, |blob| blob.truncate(16 + 32 - 1));

        assert!(matches!(
            decode(&record, true).unwrap_err(),
            AppStateError::ValueBlobTooShort
        ));
    }

    #[test]
    fn decode_record_rejects_tampered_content_mac() {
        let (keys, key_id) = test_keys();
        let mut record = create_test_record(
            wa::syncd_mutation::SyncdOperation::SET,
            &keys,
            &key_id,
            &valid_action_data(),
        );

        map_value_blob(&mut record, |blob| {
            let last = blob.len() - 1;
            blob[last] ^= 0xFF;
        });

        assert!(matches!(
            decode(&record, true).unwrap_err(),
            AppStateError::MismatchingContentMAC
        ));
        // Without validation the same record decodes: the rejection above comes
        // from the MAC check, not from a knock-on decrypt failure.
        assert!(decode(&record, false).is_ok());
    }

    #[test]
    fn decode_record_rejects_corrupt_ciphertext() {
        let (keys, key_id) = test_keys();
        let mut record = create_test_record(
            wa::syncd_mutation::SyncdOperation::SET,
            &keys,
            &key_id,
            &valid_action_data(),
        );

        // Drop a single ciphertext byte so the remainder is no longer
        // block-aligned — an unconditional CBC failure, unlike a bit flip whose
        // padding can survive by chance.
        map_value_blob(&mut record, |blob| {
            blob.remove(16);
        });

        assert!(matches!(
            decode(&record, false).unwrap_err(),
            AppStateError::DecryptionFailed
        ));
    }

    #[test]
    fn decode_record_rejects_garbage_protobuf() {
        let (keys, key_id) = test_keys();
        // Field 1, length-delimited, length 255, no payload: decodes past the
        // buffer end whatever the field means.
        let record = record_from_plaintext(
            wa::syncd_mutation::SyncdOperation::SET,
            &keys,
            &key_id,
            &[0x0A, 0xFF],
            &[],
        );

        assert!(matches!(
            decode(&record, false).unwrap_err(),
            AppStateError::DecodeFailed
        ));
    }

    #[test]
    fn decode_record_rejects_missing_index_mac_when_validating() {
        let (keys, key_id) = test_keys();
        let mut record = create_test_record(
            wa::syncd_mutation::SyncdOperation::SET,
            &keys,
            &key_id,
            &valid_action_data(),
        );
        record.index = buffa::MessageField::none();

        assert!(matches!(
            decode(&record, true).unwrap_err(),
            AppStateError::MissingIndexMAC
        ));
    }

    #[test]
    fn decode_record_rejects_missing_index_mac_without_validation() {
        let (keys, key_id) = test_keys();
        let mut record = create_test_record(
            wa::syncd_mutation::SyncdOperation::SET,
            &keys,
            &key_id,
            &valid_action_data(),
        );
        // Skipping MAC validation must not turn a MAC-less record into an
        // empty persisted MAC.
        record.index = buffa::MessageField::some(wa::SyncdIndex { blob: None });

        assert!(matches!(
            decode(&record, false).unwrap_err(),
            AppStateError::MissingIndexMAC
        ));
    }

    #[test]
    fn decode_record_parses_index_json_into_components() {
        let (keys, key_id) = test_keys();
        let record = create_test_record(
            wa::syncd_mutation::SyncdOperation::SET,
            &keys,
            &key_id,
            &valid_action_data(),
        );

        let mutation = decode(&record, true).expect("valid record should decode");
        assert_eq!(mutation.index, vec!["mute", "1555550100@s.whatsapp.net"]);
    }

    #[test]
    fn decode_record_tolerates_non_json_index() {
        let (keys, key_id) = test_keys();
        let action_data = wa::SyncActionData {
            index: Some(b"not-json".to_vec()),
            value: buffa::MessageField::some(wa::SyncActionValue {
                timestamp: Some(1),
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

        // The index MAC still covers the raw bytes, so the record is authentic —
        // only the component split is skipped.
        let mutation = decode(&record, true).expect("authentic record should decode");
        assert!(mutation.index.is_empty());
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

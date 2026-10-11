use crate::hash::generate_content_mac;
use crate::keys::ExpandedAppStateKeys;
use wacore_libsignal::crypto::{CryptographicMac, aes_256_cbc_encrypt_into};
use waproto::whatsapp as wa;

/// Encode and encrypt a mutation into a SyncdRecord.
///
/// This is the reverse of `decode_record` — it takes plaintext data and produces
/// an encrypted record ready for sending.
///
/// # Returns
/// A tuple of (SyncdMutation, value_mac_bytes) where value_mac is needed for
/// hash state updates and persistence.
pub fn encode_record(
    operation: wa::syncd_mutation::SyncdOperation,
    index: &[u8],
    value: &wa::SyncActionValue,
    keys: &ExpandedAppStateKeys,
    key_id: &[u8],
    iv: &[u8; 16],
    // Per-action schema version, mirroring whatsmeow's per-mutation `Version`.
    // WA Web stamps each action with its own (e.g. label_edit/label_jid = 3);
    // callers pass the value for the action they are encoding.
    version: i32,
) -> (wa::SyncdMutation, [u8; 32]) {
    // 1. Encode the SyncActionData wrapper straight from the borrowed value.
    let plaintext = waproto::codec::sync_action_data_to_vec(index, value, version);

    // 2. Build the value blob in place: IV || ciphertext || MAC.
    //
    // The blob is the only buffer this path needs, so it is sized for the whole
    // thing up front and the encryption writes its ciphertext directly after the
    // IV (`aes_256_cbc_encrypt_into` appends). CBC pads to the next full block
    // and always adds at least one byte, which is what fixes the ciphertext
    // length before the encryption runs.
    let cipher_len = plaintext.len() - plaintext.len() % 16 + 16;
    let mut value_blob = Vec::with_capacity(16 + cipher_len + 32);
    value_blob.extend_from_slice(iv);
    aes_256_cbc_encrypt_into(&plaintext, &keys.value_encryption, iv, &mut value_blob)
        .expect("AES encryption should not fail with valid 32-byte key and 16-byte IV");

    // 3. Generate the content MAC over IV || ciphertext, before the MAC is
    //    appended to the same buffer.
    let value_mac = generate_content_mac(operation, &value_blob, key_id, &keys.value_mac);
    value_blob.extend_from_slice(&value_mac);

    // 4. Generate index MAC
    let index_mac = {
        let mut mac = CryptographicMac::new("HmacSha256", &keys.index)
            .expect("HmacSha256 is a valid algorithm");
        mac.update(index);
        mac.finalize()
    };

    // 5. Build the record
    let record = wa::SyncdRecord {
        index: buffa::MessageField::some(wa::SyncdIndex {
            blob: Some(index_mac),
        }),
        value: buffa::MessageField::some(wa::SyncdValue {
            blob: Some(value_blob),
        }),
        key_id: buffa::MessageField::some(wa::KeyId {
            id: Some(key_id.to_vec()),
        }),
    };

    let mutation = wa::SyncdMutation {
        operation: Some(operation.into()),
        record: buffa::MessageField::some(record),
    };

    (mutation, value_mac)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::decode::decode_record;
    use crate::keys::expand_app_state_keys;

    #[test]
    fn test_encode_then_decode_roundtrip() {
        let master_key = [7u8; 32];
        let keys = expand_app_state_keys(&master_key);
        let key_id = b"test_key_id";
        let iv = [0u8; 16];

        let index = b"[\"setting_pushName\"]";
        let value = wa::SyncActionValue {
            push_name_setting: buffa::MessageField::some(wa::sync_action_value::PushNameSetting {
                name: Some("Test User".to_string()),
            }),
            timestamp: Some(1234567890),
            ..Default::default()
        };

        let (mutation, _value_mac) = encode_record(
            wa::syncd_mutation::SyncdOperation::SET,
            index,
            &value,
            &keys,
            key_id,
            &iv,
            1,
        );

        // Decode the encoded record
        let record = &*mutation.record;
        let (decoded, _macs) = decode_record(
            wa::syncd_mutation::SyncdOperation::SET,
            record,
            &keys,
            key_id,
            true, // validate MACs
        )
        .expect("roundtrip decode should succeed");

        assert_eq!(
            decoded.action_value.as_ref().and_then(|v| v.timestamp),
            Some(1234567890)
        );
        assert_eq!(
            decoded
                .action_value
                .as_ref()
                .and_then(|v| v.push_name_setting.as_option())
                .and_then(|p| p.name.as_deref()),
            Some("Test User")
        );
        assert_eq!(decoded.index, vec!["setting_pushName"]);
        assert_eq!(decoded.operation, wa::syncd_mutation::SyncdOperation::SET);
    }

    /// The plaintext is written field by field instead of through an owned
    /// `SyncActionData`, so the bytes are pinned against what the generated
    /// encoder produces for the same four fields.
    // The generated encoder is the reference this test exists to compare
    // against, so it calls it directly instead of going through
    // `waproto::codec` (which no longer has an owned-`SyncActionData` entry
    // point). Test-only, so the extra instantiation ships nowhere.
    #[allow(clippy::disallowed_methods)]
    #[test]
    fn hand_written_plaintext_matches_generated_encoder() {
        use buffa::Message as _;

        let index = b"[\"contact\",\"5511999998888@s.whatsapp.net\"]";
        let cases = [
            (
                1,
                wa::SyncActionValue {
                    timestamp: Some(1_700_000_000),
                    contact_action: wa::sync_action_value::ContactAction {
                        full_name: Some("Contact Full Name".to_string()),
                        ..Default::default()
                    }
                    .into(),
                    ..Default::default()
                },
            ),
            // Empty value and a multi-byte version: the degenerate shapes the
            // exact-size arithmetic is most likely to get wrong.
            (0, wa::SyncActionValue::default()),
            (i32::MAX, wa::SyncActionValue::default()),
            (-1, wa::SyncActionValue::default()),
        ];

        for (version, value) in cases {
            let expected = wa::SyncActionData {
                index: Some(index.to_vec()),
                value: buffa::MessageField::some(value.clone()),
                padding: Some(vec![]),
                version: Some(version),
            }
            .encode_to_vec();
            let got = waproto::codec::sync_action_data_to_vec(index, &value, version);
            assert_eq!(got, expected, "version {version}");
            assert_eq!(got.capacity(), got.len(), "buffer must be exactly sized");
        }
    }
}

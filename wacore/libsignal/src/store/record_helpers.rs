//! Bridges between the persisted `*RecordStructure` protobufs and the record
//! types.
//!
//! Both sides encode `public_key` as the raw 32-byte DJB key (see the
//! `PreKeyRecord` doc comment), so these conversions are lossless in both
//! directions and the structure a record produces is byte-identical to the one
//! it was built from. What they still do is validate: a structure carrying
//! malformed key bytes is rejected here rather than at some later use.
//!
//! Reads go through `PublicKey::from_stored_public_key_bytes`, so a store
//! holding the 33-byte form written by pre-0.7 record constructors still loads,
//! and converting it back out normalizes it.

use crate::protocol::{
    KeyPair, PreKeyRecord, PrivateKey, PublicKey, SignalProtocolError, SignedPreKeyRecord,
};
use chrono::Utc;
use waproto::whatsapp as wa;

/// Append a pre-key record directly from the key pair's fixed-size buffers.
///
/// The generated protobuf view keeps both key fields borrowed, avoiding the two
/// temporary `Vec<u8>` allocations performed by [`new_pre_key_record`]. The
/// schema-generated `ViewEncode` implementation remains the single source of
/// truth for field numbers and wire types.
pub fn encode_pre_key_record_to(id: u32, key_pair: &KeyPair, out: &mut Vec<u8>) {
    use buffa::ViewEncode as _;

    let view = wa::PreKeyRecordStructureView {
        id: Some(id),
        public_key: Some(key_pair.public_key.public_key_bytes()),
        private_key: Some(key_pair.private_key.serialize()),
    };

    // `ViewEncode::encode` computes the size but leaves capacity management to
    // the sink. Reserve from that schema-derived size, then reuse the same
    // cache for the write: an empty output allocates once, while a retained
    // batch buffer remains allocation-free.
    let mut cache = buffa::SizeCache::new();
    let encoded_len = buffa::checked_encode_size(view.compute_size(&mut cache))
        .unwrap_or_else(|_| buffa::encode_size_overflow()) as usize;
    let start_len = out.len();
    out.reserve(encoded_len);
    view.write_to(&mut cache, out);
    buffa::debug_assert_two_pass(out.len() - start_len, encoded_len);
}

pub fn new_pre_key_record(id: u32, key_pair: &KeyPair) -> wa::PreKeyRecordStructure {
    wa::PreKeyRecordStructure {
        id: Some(id),
        public_key: Some(key_pair.public_key.public_key_bytes().to_vec()),
        private_key: Some(key_pair.private_key.serialize().to_vec()),
    }
}

pub fn new_signed_pre_key_record(
    id: u32,
    key_pair: &KeyPair,
    signature: [u8; 64],
    timestamp: chrono::DateTime<Utc>,
) -> wa::SignedPreKeyRecordStructure {
    wa::SignedPreKeyRecordStructure {
        id: Some(id),
        public_key: Some(key_pair.public_key.public_key_bytes().to_vec()),
        private_key: Some(key_pair.private_key.serialize().to_vec()),
        signature: Some(signature.to_vec()),
        timestamp: Some(
            timestamp
                .timestamp()
                .try_into()
                .expect("Timestamp conversion failed"),
        ),
    }
}

pub fn prekey_structure_to_record(
    mut structure: wa::PreKeyRecordStructure,
) -> Result<PreKeyRecord, SignalProtocolError> {
    // The parsed keys are dropped again: they exist to reject a malformed
    // structure, not to rebuild one. Rebuilding is what this used to do, and it
    // re-allocated both key fields from the keys it had just parsed out of the
    // buffers the caller already owned -- two allocations per prekey read, on
    // the path every PreKeySignalMessage decrypt takes.
    PublicKey::from_stored_public_key_bytes(
        structure
            .public_key
            .as_ref()
            .ok_or(SignalProtocolError::InvalidProtobufEncoding)?
            .as_slice(),
    )?;
    PrivateKey::deserialize(
        structure
            .private_key
            .as_ref()
            .ok_or(SignalProtocolError::InvalidProtobufEncoding)?,
    )?;
    structure.id = Some(structure.id.unwrap_or(0));
    Ok(PreKeyRecord::from_storage(structure))
}

pub fn prekey_record_to_structure(
    record: &PreKeyRecord,
) -> Result<wa::PreKeyRecordStructure, SignalProtocolError> {
    // Re-derived from the parsed key pair rather than copied field-by-field, so
    // a structure that reached the record with malformed key bytes cannot be
    // written back out to the store.
    let key_pair = record.key_pair()?;
    Ok(new_pre_key_record(record.id()?.into(), &key_pair))
}

pub fn signed_prekey_structure_to_record(
    mut structure: wa::SignedPreKeyRecordStructure,
) -> Result<SignedPreKeyRecord, SignalProtocolError> {
    // Same shape as `prekey_structure_to_record`: validate, then adopt. The
    // rebuild it replaces copied both keys and the 64-byte signature.
    PublicKey::from_stored_public_key_bytes(
        structure
            .public_key
            .as_ref()
            .ok_or(SignalProtocolError::InvalidProtobufEncoding)?
            .as_slice(),
    )?;
    PrivateKey::deserialize(
        structure
            .private_key
            .as_ref()
            .ok_or(SignalProtocolError::InvalidProtobufEncoding)?,
    )?;
    if structure.signature.is_none() {
        return Err(SignalProtocolError::InvalidProtobufEncoding);
    }
    structure.id = Some(structure.id.unwrap_or(0));
    structure.timestamp = Some(structure.timestamp.unwrap_or(0));
    Ok(
        <SignedPreKeyRecord as crate::protocol::GenericSignedPreKey>::from_stored_structure(
            structure,
        ),
    )
}

// Tests intentionally exercise the raw buffa Message methods: the borrowed
// encoder is verified byte-for-byte against the owned trait encoder.
#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;
    use crate::protocol::{GenericSignedPreKey, KeyPair, PreKeyRecord, Timestamp};
    use rand::Rng;

    #[test]
    fn borrowed_prekey_encoder_matches_owned_encoder() {
        use buffa::Message as _;

        let key_pair = KeyPair::generate(&mut rand::rng());
        let mut actual = Vec::new();
        let mut retained_allocation = None;
        for id in [u32::MAX, (1 << 24) - 1, 1] {
            let expected = new_pre_key_record(id, &key_pair).encode_to_vec();
            actual.clear();
            encode_pre_key_record_to(id, &key_pair, &mut actual);
            assert_eq!(actual, expected);

            if let Some((ptr, capacity)) = retained_allocation {
                assert_eq!(actual.as_ptr(), ptr);
                assert_eq!(actual.capacity(), capacity);
            } else {
                retained_allocation = Some((actual.as_ptr(), actual.capacity()));
            }

            let decoded = wa::PreKeyRecordStructure::decode_from_slice(&actual)
                .expect("borrowed record should decode");
            assert_eq!(decoded.id, Some(id));
            assert_eq!(
                decoded.public_key.as_deref(),
                Some(key_pair.public_key.public_key_bytes())
            );
            assert_eq!(
                decoded.private_key.as_deref(),
                Some(key_pair.private_key.serialize().as_slice())
            );
        }
    }

    #[test]
    fn test_prekey_serialization_length() -> Result<(), Box<dyn std::error::Error>> {
        let key_pair = KeyPair::generate(&mut rand::rng());
        let record = PreKeyRecord::new(1.into(), &key_pair);
        let structure = prekey_record_to_structure(&record)?;

        // DJB format is 32 bytes (no prefix byte)
        let pub_key = structure.public_key.clone().unwrap();
        assert_eq!(pub_key.len(), 32);

        Ok(())
    }

    #[test]
    fn test_prekey_round_trip() -> Result<(), Box<dyn std::error::Error>> {
        let key_pair = KeyPair::generate(&mut rand::rng());
        let original_record = PreKeyRecord::new(42.into(), &key_pair);

        // Serialize to structure
        let structure = prekey_record_to_structure(&original_record)?;

        // Deserialize back to record
        let restored_record = prekey_structure_to_record(structure)?;

        // Verify round-trip integrity
        assert_eq!(original_record.id()?, restored_record.id()?);

        let original_keypair = original_record.key_pair()?;
        let restored_keypair = restored_record.key_pair()?;

        // Compare public keys (DJB format)
        assert_eq!(
            original_keypair.public_key.public_key_bytes(),
            restored_keypair.public_key.public_key_bytes()
        );

        // Compare private keys
        assert_eq!(
            original_keypair.private_key.serialize(),
            restored_keypair.private_key.serialize()
        );

        Ok(())
    }

    /// The property the two encodings used to break: bytes written by the store
    /// path must read back through the record's *own* API, not only through the
    /// `record_helpers` bridge that production happens to use.
    #[test]
    fn store_written_prekey_reads_back_through_record_api() -> Result<(), Box<dyn std::error::Error>>
    {
        let key_pair = KeyPair::generate(&mut rand::rng());
        let mut stored = Vec::new();
        encode_pre_key_record_to(7, &key_pair, &mut stored);

        let record = PreKeyRecord::deserialize(&stored)?;
        assert_eq!(record.id()?, 7u32.into());
        assert_eq!(
            record.public_key()?.public_key_bytes(),
            key_pair.public_key.public_key_bytes()
        );
        assert_eq!(
            record.key_pair()?.private_key.serialize(),
            key_pair.private_key.serialize()
        );

        // And the reverse direction: a record built in memory serializes to the
        // exact bytes the store writer produces for the same key.
        assert_eq!(
            PreKeyRecord::new(7u32.into(), &key_pair).serialize()?,
            stored
        );
        Ok(())
    }

    /// Same property for the signed record, whose getters go through the
    /// `KeySerde` record encoding rather than `PublicKey::deserialize`.
    #[test]
    fn store_written_signed_prekey_reads_back_through_record_api()
    -> Result<(), Box<dyn std::error::Error>> {
        let key_pair = KeyPair::generate(&mut rand::rng());
        let mut signature = [0u8; 64];
        rand::rng().fill_bytes(&mut signature);
        let timestamp = chrono::DateTime::from_timestamp(1_700_000_000, 0)
            .expect("fixed timestamp is in range");

        let structure = new_signed_pre_key_record(9, &key_pair, signature, timestamp);
        let stored = waproto::codec::signed_pre_key_record_to_vec(&structure);

        let record = <SignedPreKeyRecord as GenericSignedPreKey>::deserialize(&stored)?;
        assert_eq!(record.id()?, 9u32.into());
        assert_eq!(
            record.public_key()?.public_key_bytes(),
            key_pair.public_key.public_key_bytes()
        );
        assert_eq!(
            record.key_pair()?.private_key.serialize(),
            key_pair.private_key.serialize()
        );
        assert_eq!(record.signature()?, signature.to_vec());

        // `GenericSignedPreKey::new` must agree byte-for-byte with the store helper.
        let rebuilt = <SignedPreKeyRecord as GenericSignedPreKey>::new(
            9u32.into(),
            Timestamp::from_epoch_millis(structure.timestamp.expect("timestamp set")),
            &key_pair,
            &signature,
        );
        assert_eq!(rebuilt.serialize()?, stored);
        Ok(())
    }

    /// Records written by the pre-0.7 constructors carry the 33-byte tagged key.
    /// They predate this crate settling on one encoding, are reachable by any
    /// downstream store that persisted `record.serialize()`, and must keep
    /// loading — through the record getters and through the bridges alike.
    #[test]
    fn legacy_tagged_records_still_load_and_normalize() -> Result<(), Box<dyn std::error::Error>> {
        use crate::protocol::PublicKey;
        use buffa::Message as _;

        let key_pair = KeyPair::generate(&mut rand::rng());
        let tagged = key_pair.public_key.serialize().to_vec();
        assert_eq!(tagged.len(), PublicKey::SERIALIZED_KEY_LEN);

        let legacy_prekey = wa::PreKeyRecordStructure {
            id: Some(3),
            public_key: Some(tagged.clone()),
            private_key: Some(key_pair.private_key.serialize().to_vec()),
        };
        let record = PreKeyRecord::deserialize(&legacy_prekey.clone().encode_to_vec())?;
        assert_eq!(
            record.public_key()?.public_key_bytes(),
            key_pair.public_key.public_key_bytes()
        );
        assert_eq!(
            record.key_pair()?.private_key.serialize(),
            key_pair.private_key.serialize()
        );
        // Decoding normalized the field, so re-serializing heals the stored row
        // instead of writing the tagged key back. This is the only path a store
        // that round-trips through the record API ever takes.
        let mut expected = Vec::new();
        encode_pre_key_record_to(3, &key_pair, &mut expected);
        assert_eq!(record.serialize()?, expected);

        // The bridge accepts it too, and writing it back out normalizes to raw.
        let normalized = prekey_record_to_structure(&prekey_structure_to_record(legacy_prekey)?)?;
        assert_eq!(
            normalized.public_key.as_deref(),
            Some(key_pair.public_key.public_key_bytes())
        );

        let legacy_signed = wa::SignedPreKeyRecordStructure {
            id: Some(4),
            public_key: Some(tagged),
            private_key: Some(key_pair.private_key.serialize().to_vec()),
            signature: Some(vec![0u8; 64]),
            timestamp: Some(0),
        };
        let signed = <SignedPreKeyRecord as GenericSignedPreKey>::deserialize(
            &waproto::codec::signed_pre_key_record_to_vec(&legacy_signed),
        )?;
        assert_eq!(
            signed.public_key()?.public_key_bytes(),
            key_pair.public_key.public_key_bytes()
        );
        assert_eq!(
            signed.key_pair()?.private_key.serialize(),
            key_pair.private_key.serialize()
        );
        let expected_signed =
            waproto::codec::signed_pre_key_record_to_vec(&new_signed_pre_key_record(
                4,
                &key_pair,
                [0u8; 64],
                chrono::DateTime::from_timestamp(0, 0).expect("epoch is in range"),
            ));
        assert_eq!(signed.serialize()?, expected_signed);
        assert_eq!(
            signed_prekey_structure_to_record(legacy_signed)?.serialize()?,
            expected_signed
        );
        Ok(())
    }

    /// Both record constructors write the 32-byte form WhatsApp uploads in
    /// `<key><value>` / `<skey><value>` and hashes in the key-bundle digest.
    #[test]
    fn record_constructors_write_raw_public_keys() -> Result<(), Box<dyn std::error::Error>> {
        use crate::protocol::PublicKey;

        let key_pair = KeyPair::generate(&mut rand::rng());
        let prekey = PreKeyRecord::new(1.into(), &key_pair).serialize()?;
        assert_eq!(
            waproto::codec::pre_key_record_decode(&prekey)?
                .public_key
                .expect("public key set")
                .len(),
            PublicKey::RAW_KEY_LEN
        );

        let signed = <SignedPreKeyRecord as GenericSignedPreKey>::new(
            1.into(),
            Timestamp::from_epoch_millis(0),
            &key_pair,
            &[0u8; 64],
        )
        .serialize()?;
        assert_eq!(
            waproto::codec::signed_pre_key_record_decode(&signed)?
                .public_key
                .expect("public key set")
                .len(),
            PublicKey::RAW_KEY_LEN
        );
        Ok(())
    }

    #[test]
    fn test_signed_prekey_round_trip() -> Result<(), Box<dyn std::error::Error>> {
        let key_pair = KeyPair::generate(&mut rand::rng());
        let mut signature = [0u8; 64];
        rand::rng().fill_bytes(&mut signature);
        let timestamp = chrono::DateTime::from_timestamp(1_700_000_000, 0)
            .expect("fixed timestamp is in range");
        let id = 123u32;

        // Create structure using new_signed_pre_key_record
        let structure = new_signed_pre_key_record(id, &key_pair, signature, timestamp);

        // Deserialize back to record
        let restored_record = signed_prekey_structure_to_record(structure)?;

        // Verify round-trip integrity
        assert_eq!(restored_record.id()?, id.into());

        let restored_keypair = restored_record.key_pair()?;

        // Compare public keys (DJB format)
        assert_eq!(
            key_pair.public_key.public_key_bytes(),
            restored_keypair.public_key.public_key_bytes()
        );

        // Compare private keys
        assert_eq!(
            key_pair.private_key.serialize(),
            restored_keypair.private_key.serialize()
        );

        // Compare signature
        assert_eq!(signature.to_vec(), restored_record.signature()?);

        Ok(())
    }
}

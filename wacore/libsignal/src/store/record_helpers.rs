use crate::protocol::{
    KeyPair, PreKeyRecord, PrivateKey, PublicKey, SignalProtocolError, SignedPreKeyRecord,
    Timestamp,
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
    structure: wa::PreKeyRecordStructure,
) -> Result<PreKeyRecord, SignalProtocolError> {
    let id = structure.id.unwrap_or(0).into();
    let public_key = PublicKey::from_djb_public_key_bytes(
        structure
            .public_key
            .as_ref()
            .ok_or(SignalProtocolError::InvalidProtobufEncoding)?
            .as_slice(),
    )?;
    let private_key = PrivateKey::deserialize(
        structure
            .private_key
            .as_ref()
            .ok_or(SignalProtocolError::InvalidProtobufEncoding)?,
    )?;
    Ok(PreKeyRecord::new(
        id,
        &KeyPair::new(public_key, private_key),
    ))
}

pub fn prekey_record_to_structure(
    record: &PreKeyRecord,
) -> Result<wa::PreKeyRecordStructure, SignalProtocolError> {
    Ok(wa::PreKeyRecordStructure {
        id: Some(record.id()?.into()),
        public_key: Some(record.key_pair()?.public_key.public_key_bytes().to_vec()),
        private_key: Some(record.key_pair()?.private_key.serialize().to_vec()),
    })
}

pub fn signed_prekey_structure_to_record(
    structure: wa::SignedPreKeyRecordStructure,
) -> Result<SignedPreKeyRecord, SignalProtocolError> {
    let id = structure.id.unwrap_or(0).into();
    let public_key = PublicKey::from_djb_public_key_bytes(
        structure
            .public_key
            .as_ref()
            .ok_or(SignalProtocolError::InvalidProtobufEncoding)?
            .as_slice(),
    )?;
    let private_key = PrivateKey::deserialize(
        structure
            .private_key
            .as_ref()
            .ok_or(SignalProtocolError::InvalidProtobufEncoding)?,
    )?;
    let key_pair = KeyPair::new(public_key, private_key);
    let signature = structure
        .signature
        .as_ref()
        .ok_or(SignalProtocolError::InvalidProtobufEncoding)?;
    let timestamp = Timestamp::from_epoch_millis(structure.timestamp.unwrap_or(0));
    Ok(
        <SignedPreKeyRecord as crate::protocol::GenericSignedPreKey>::new(
            id, timestamp, &key_pair, signature,
        ),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::{GenericSignedPreKey, KeyPair, PreKeyRecord};
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

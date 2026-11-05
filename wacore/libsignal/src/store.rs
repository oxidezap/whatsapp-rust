// SPDX-License-Identifier: MIT
//! Storage boundaries. Generated protobufs occur only at legacy host data
//! boundaries; the protocol engine does not retain them in memory.
use crate::protocol::{ProtocolAddress, SessionRecord};
use async_trait::async_trait;
use waproto::whatsapp::{PreKeyRecordStructure, SignedPreKeyRecordStructure};
pub type StoreError = Box<dyn std::error::Error + Send + Sync>;
#[cfg_attr(target_arch="wasm32",async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait PreKeyStore: Send + Sync {
    async fn load_prekey(&self, id: u32) -> Result<Option<PreKeyRecordStructure>, StoreError>;
    async fn store_prekey(
        &self,
        id: u32,
        record: PreKeyRecordStructure,
        uploaded: bool,
    ) -> Result<(), StoreError>;
    async fn contains_prekey(&self, id: u32) -> Result<bool, StoreError>;
    async fn remove_prekey(&self, id: u32) -> Result<(), StoreError>;
}
#[cfg_attr(target_arch="wasm32",async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait SignedPreKeyStore: Send + Sync {
    async fn load_signed_prekey(
        &self,
        id: u32,
    ) -> Result<Option<SignedPreKeyRecordStructure>, StoreError>;
    async fn load_signed_prekeys(&self) -> Result<Vec<SignedPreKeyRecordStructure>, StoreError>;
    async fn store_signed_prekey(
        &self,
        id: u32,
        record: SignedPreKeyRecordStructure,
    ) -> Result<(), StoreError>;
    async fn contains_signed_prekey(&self, id: u32) -> Result<bool, StoreError>;
    async fn remove_signed_prekey(&self, id: u32) -> Result<(), StoreError>;
}
#[cfg_attr(target_arch="wasm32",async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait SessionStore: Send + Sync {
    async fn load_session(&self, address: &ProtocolAddress) -> Result<SessionRecord, StoreError>;
    async fn get_sub_device_sessions(&self, name: &str) -> Result<Vec<u32>, StoreError>;
    async fn store_session(
        &self,
        address: &ProtocolAddress,
        record: &SessionRecord,
    ) -> Result<(), StoreError>;
    async fn contains_session(&self, address: &ProtocolAddress) -> Result<bool, StoreError>;
    async fn delete_session(&self, address: &ProtocolAddress) -> Result<(), StoreError>;
    async fn delete_all_sessions(&self, name: &str) -> Result<(), StoreError>;
}
pub mod sender_key_name {
    #[derive(Clone, Debug, Hash, PartialEq, Eq)]
    pub struct SenderKeyName {
        key: String,
        group_len: usize,
    }
    impl SenderKeyName {
        pub fn new(group: String, sender: String) -> Self {
            Self::from_parts(&group, &sender)
        }
        pub fn from_parts(group: &str, sender: &str) -> Self {
            let mut key = String::with_capacity(group.len() + sender.len() + 1);
            key.push_str(group);
            key.push(':');
            key.push_str(sender);
            Self {
                key,
                group_len: group.len(),
            }
        }
        pub fn from_buf(key: String, group_len: usize) -> Self {
            Self { key, group_len }
        }
        /// Validate an externally supplied offset before using the slicing accessors.
        pub fn try_from_buf(key: String, group_len: usize) -> Option<Self> {
            (key.is_char_boundary(group_len) && key.as_bytes().get(group_len) == Some(&b':'))
                .then_some(Self { key, group_len })
        }
        pub fn cache_key(&self) -> &str {
            &self.key
        }
        pub fn group_id(&self) -> &str {
            &self.key[..self.group_len]
        }
        pub fn sender_id(&self) -> &str {
            &self.key[self.group_len + 1..]
        }
        pub fn group(&self) -> &str {
            self.group_id()
        }
        pub fn sender(&self) -> &str {
            self.sender_id()
        }
    }
    impl core::fmt::Display for SenderKeyName {
        fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
            self.key.fmt(f)
        }
    }
    #[cfg(test)]
    mod tests {
        use super::SenderKeyName;

        #[test]
        fn checked_offsets_require_a_character_boundary_and_separator() {
            for offset in [0, 1, 3, 4, usize::MAX] {
                assert!(SenderKeyName::try_from_buf("é:x".into(), offset).is_none());
            }
            assert!(SenderKeyName::try_from_buf("group-sender".into(), 5).is_none());
            assert!(SenderKeyName::try_from_buf(String::new(), 0).is_none());
            for (key, offset, group, sender) in
                [("é:x", 2, "é", "x"), (":", 0, "", ""), ("g:", 1, "g", "")]
            {
                let name = SenderKeyName::try_from_buf(key.into(), offset).unwrap();
                assert_eq!(name.group_id(), group);
                assert_eq!(name.sender_id(), sender);
                assert_eq!(name.cache_key(), key);
            }
        }
    }
}
pub mod record_helpers {
    use crate::protocol::{self, GenericSignedPreKey, KeyPair, PreKeyRecord, SignedPreKeyRecord};
    use protocol::error::Result;
    use waproto::whatsapp::{PreKeyRecordStructure, SignedPreKeyRecordStructure};
    pub fn new_pre_key_record(id: u32, key: &KeyPair) -> PreKeyRecordStructure {
        {
            let mut record = PreKeyRecordStructure::default();
            record.id = Some(id);
            record.public_key = Some(key.public_key.public_key_bytes().to_vec());
            record.private_key = Some(key.private_key.serialize().to_vec());
            record
        }
    }
    pub fn encode_pre_key_record_to(id: u32, key: &KeyPair, out: &mut Vec<u8>) {
        out.extend_from_slice(&waproto::codec::pre_key_record_to_vec(&new_pre_key_record(
            id, key,
        )));
    }
    pub fn prekey_structure_to_record(record: PreKeyRecordStructure) -> Result<PreKeyRecord> {
        PreKeyRecord::deserialize(&waproto::codec::pre_key_record_to_vec(&record))
    }
    pub fn prekey_record_to_structure(record: &PreKeyRecord) -> Result<PreKeyRecordStructure> {
        waproto::codec::pre_key_record_decode(&record.serialize()?)
            .map_err(|_| protocol::SignalProtocolError::InvalidProtobufEncoding)
    }
    pub fn new_signed_pre_key_record(
        id: u32,
        key: &KeyPair,
        signature: [u8; 64],
        timestamp: chrono::DateTime<chrono::Utc>,
    ) -> Result<SignedPreKeyRecordStructure> {
        {
            let mut record = SignedPreKeyRecordStructure::default();
            record.id = Some(id);
            record.public_key = Some(key.public_key.public_key_bytes().to_vec());
            record.private_key = Some(key.private_key.serialize().to_vec());
            record.signature = Some(signature.to_vec());
            record.timestamp = Some(timestamp.timestamp().max(0) as u64);
            Ok(record)
        }
    }
    pub fn signed_prekey_structure_to_record(
        record: SignedPreKeyRecordStructure,
    ) -> Result<SignedPreKeyRecord> {
        SignedPreKeyRecord::deserialize(&waproto::codec::signed_pre_key_record_to_vec(&record))
    }
    pub fn signed_prekey_record_to_structure(
        record: &SignedPreKeyRecord,
    ) -> Result<SignedPreKeyRecordStructure> {
        waproto::codec::signed_pre_key_record_decode(&record.serialize()?)
            .map_err(|_| protocol::SignalProtocolError::InvalidProtobufEncoding)
    }
}

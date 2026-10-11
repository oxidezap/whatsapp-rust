//
// Copyright 2020-2022 Signal Messenger, LLC.
// SPDX-License-Identifier: AGPL-3.0-only
//

use std::convert::AsRef;
use std::fmt;

use crate::protocol::{
    KeyPair, PrivateKey, PublicKey, Result, SignalProtocolError, Timestamp,
    stores::SignedPreKeyRecordStructure,
};

/// A unique identifier selecting among this client's known signed pre-keys.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct SignedPreKeyId(u32);

impl From<u32> for SignedPreKeyId {
    #[inline]
    fn from(id: u32) -> Self {
        Self(id)
    }
}

impl From<SignedPreKeyId> for u32 {
    #[inline]
    fn from(id: SignedPreKeyId) -> Self {
        id.0
    }
}

impl fmt::Display for SignedPreKeyId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone)]
pub struct SignedPreKeyRecord {
    signed_pre_key: SignedPreKeyRecordStructure,
}

impl SignedPreKeyRecord {
    pub fn private_key(&self) -> Result<PrivateKey> {
        Ok(PrivateKey::deserialize(
            self.get_storage()
                .private_key
                .as_ref()
                .ok_or(SignalProtocolError::InvalidProtobufEncoding)?,
        )?)
    }
}

impl GenericSignedPreKey for SignedPreKeyRecord {
    type KeyPair = KeyPair;
    type Id = SignedPreKeyId;

    fn get_storage(&self) -> &SignedPreKeyRecordStructure {
        &self.signed_pre_key
    }

    fn from_storage(storage: SignedPreKeyRecordStructure) -> Self {
        Self {
            signed_pre_key: storage,
        }
    }
}

pub trait GenericSignedPreKey {
    type KeyPair: KeyPairSerde;
    type Id: From<u32> + Into<u32>;

    fn get_storage(&self) -> &SignedPreKeyRecordStructure;
    fn from_storage(storage: SignedPreKeyRecordStructure) -> Self;

    fn new(id: Self::Id, timestamp: Timestamp, key_pair: &Self::KeyPair, signature: &[u8]) -> Self
    where
        Self: Sized,
    {
        let timestamp = timestamp.epoch_millis();
        let public_key = key_pair.get_public().to_record_bytes();
        let private_key = key_pair.get_private().to_record_bytes();
        let signature = signature.to_vec();
        Self::from_storage(SignedPreKeyRecordStructure {
            id: Some(id.into()),
            timestamp: Some(timestamp),
            public_key: Some(public_key),
            private_key: Some(private_key),
            signature: Some(signature),
        })
    }

    fn serialize(&self) -> Result<Vec<u8>> {
        Ok(waproto::codec::signed_pre_key_record_to_vec(
            self.get_storage(),
        ))
    }

    /// Adopt a stored structure, normalizing its public key the way
    /// [`deserialize`](Self::deserialize) does.
    ///
    /// See `PreKeyRecord::from_storage` for why a store read moves the structure
    /// in instead of rebuilding it from parsed keys.
    fn from_stored_structure(mut storage: SignedPreKeyRecordStructure) -> Self
    where
        Self: Sized,
    {
        super::normalize_stored_public_key(&mut storage.public_key);
        Self::from_storage(storage)
    }

    fn deserialize(data: &[u8]) -> Result<Self>
    where
        Self: Sized,
    {
        let storage = waproto::codec::signed_pre_key_record_decode(data)
            .map_err(|_| SignalProtocolError::InvalidProtobufEncoding)?;
        Ok(Self::from_stored_structure(storage))
    }

    fn id(&self) -> Result<Self::Id> {
        Ok(self
            .get_storage()
            .id
            .ok_or(SignalProtocolError::InvalidProtobufEncoding)?
            .into())
    }

    fn timestamp(&self) -> Result<Timestamp> {
        Ok(Timestamp::from_epoch_millis(
            self.get_storage()
                .timestamp
                .ok_or(SignalProtocolError::InvalidProtobufEncoding)?,
        ))
    }

    /// Borrow the stored signature. Prefer this over
    /// [`signature`](Self::signature) wherever the bytes are only read.
    fn signature_bytes(&self) -> Result<&[u8]> {
        self.get_storage()
            .signature
            .as_deref()
            .ok_or(SignalProtocolError::InvalidProtobufEncoding)
    }

    fn signature(&self) -> Result<Vec<u8>> {
        self.signature_bytes().map(<[u8]>::to_vec)
    }

    fn public_key(&self) -> Result<<Self::KeyPair as KeyPairSerde>::PublicKey> {
        <Self::KeyPair as KeyPairSerde>::PublicKey::from_record_bytes(
            self.get_storage()
                .public_key
                .as_ref()
                .ok_or(SignalProtocolError::InvalidProtobufEncoding)?,
        )
    }

    fn key_pair(&self) -> Result<Self::KeyPair> {
        Self::KeyPair::from_record_public_and_private(
            self.get_storage()
                .public_key
                .as_ref()
                .ok_or(SignalProtocolError::InvalidProtobufEncoding)?,
            self.get_storage()
                .private_key
                .as_ref()
                .ok_or(SignalProtocolError::InvalidProtobufEncoding)?,
        )
    }
}

/// How a key is encoded *inside a record structure*, which for public keys is
/// not `PublicKey::serialize`.
///
/// The methods are deliberately not named `serialize`/`deserialize`: the record
/// encoding is WhatsApp's raw 32-byte DJB key, while the inherent
/// `PublicKey::serialize` produces Signal's 33-byte type-tagged form. Two
/// same-named functions differing by one leading byte is the trap this trait
/// exists to keep out of the record types — see the `PreKeyRecord` doc comment
/// for why 32 is the form that must be persisted.
pub trait KeySerde {
    fn to_record_bytes(&self) -> Vec<u8>;
    fn from_record_bytes<T: AsRef<[u8]>>(bytes: T) -> Result<Self>
    where
        Self: Sized;
}

pub trait KeyPairSerde {
    type PublicKey: KeySerde;
    type PrivateKey: KeySerde;
    fn from_record_public_and_private(public_key: &[u8], private_key: &[u8]) -> Result<Self>
    where
        Self: Sized;
    fn get_public(&self) -> &Self::PublicKey;
    fn get_private(&self) -> &Self::PrivateKey;
}

impl KeySerde for PublicKey {
    fn to_record_bytes(&self) -> Vec<u8> {
        self.public_key_bytes().to_vec()
    }

    fn from_record_bytes<T: AsRef<[u8]>>(bytes: T) -> Result<Self> {
        Ok(Self::from_stored_public_key_bytes(bytes.as_ref())?)
    }
}

impl KeySerde for PrivateKey {
    fn to_record_bytes(&self) -> Vec<u8> {
        self.serialize().to_vec()
    }

    fn from_record_bytes<T: AsRef<[u8]>>(bytes: T) -> Result<Self> {
        Ok(Self::deserialize(bytes.as_ref())?)
    }
}

impl KeyPairSerde for KeyPair {
    type PublicKey = PublicKey;
    type PrivateKey = PrivateKey;

    fn from_record_public_and_private(public_key: &[u8], private_key: &[u8]) -> Result<Self> {
        Ok(KeyPair::new(
            PublicKey::from_record_bytes(public_key)?,
            PrivateKey::from_record_bytes(private_key)?,
        ))
    }

    fn get_public(&self) -> &PublicKey {
        &self.public_key
    }

    fn get_private(&self) -> &PrivateKey {
        &self.private_key
    }
}

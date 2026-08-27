//
// Copyright 2020-2022 Signal Messenger, LLC.
// SPDX-License-Identifier: AGPL-3.0-only
//

use std::fmt;

use crate::protocol::{
    KeyPair, PrivateKey, PublicKey, Result, SignalProtocolError, stores::PreKeyRecordStructure,
};

/// A unique identifier selecting among this client's known pre-keys.
#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct PreKeyId(u32);

impl From<u32> for PreKeyId {
    #[inline]
    fn from(id: u32) -> Self {
        Self(id)
    }
}

impl From<PreKeyId> for u32 {
    #[inline]
    fn from(id: PreKeyId) -> Self {
        id.0
    }
}

impl fmt::Display for PreKeyId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A one-time pre-key, in the shape WhatsApp persists it.
///
/// `public_key` holds the **raw 32-byte** DJB key, not Signal's type-tagged
/// 33-byte serialization. That is WhatsApp's encoding rather than upstream
/// Signal's: `uploadPreKeys` puts 32 bytes in each `<key><value>`, and WA Web's
/// `validateLocalKeyBundle` sizes its digest buffer at `keys.length * 32` and
/// copies each stored `keyPair.pubKey` into it — a 33-byte field would overrun
/// it. Every writer in the tree (the store helpers, this constructor) agrees on
/// the raw form, so a record is byte-identical whether it was just built or
/// decoded back out of the store.
///
/// Readers additionally accept the 33-byte form via
/// [`PublicKey::from_stored_public_key_bytes`], because up to 0.6.0 this
/// constructor wrote it and a downstream store that persisted
/// [`serialize`](Self::serialize) still holds records in that shape. Such a
/// record reads correctly, and [`deserialize`](Self::deserialize) rewrites the
/// field to the raw form, so re-serializing heals the stored row rather than
/// carrying the old encoding forward.
#[derive(Debug, Clone)]
pub struct PreKeyRecord {
    pre_key: PreKeyRecordStructure,
}

impl PreKeyRecord {
    pub fn new(id: PreKeyId, key: &KeyPair) -> Self {
        let public_key = key.public_key.public_key_bytes().to_vec();
        let private_key = key.private_key.serialize().to_vec();
        Self {
            pre_key: PreKeyRecordStructure {
                id: Some(id.into()),
                public_key: Some(public_key),
                private_key: Some(private_key),
            },
        }
    }

    /// Adopt a record structure the caller already owns.
    ///
    /// Reusing its buffers is the whole point: a store read that decodes a
    /// structure and then rebuilds the record through [`new`](Self::new)
    /// re-allocates both key fields and drops the originals, for two 32-byte
    /// copies that change nothing. The stored public key is normalized to the
    /// raw 32-byte form exactly as [`deserialize`](Self::deserialize) does, so a
    /// row written before 0.7.0 reads back identical to a freshly built record.
    ///
    /// The key bytes are not parsed here. A caller that must reject a malformed
    /// structure validates it through [`key_pair`](Self::key_pair) first.
    pub fn from_storage(mut pre_key: PreKeyRecordStructure) -> Self {
        super::normalize_stored_public_key(&mut pre_key.public_key);
        Self { pre_key }
    }

    pub fn deserialize(data: &[u8]) -> Result<Self> {
        let pre_key = waproto::codec::pre_key_record_decode(data)
            .map_err(|_| SignalProtocolError::InvalidProtobufEncoding)?;
        Ok(Self::from_storage(pre_key))
    }

    pub fn id(&self) -> Result<PreKeyId> {
        Ok(self
            .pre_key
            .id
            .ok_or(SignalProtocolError::InvalidProtobufEncoding)?
            .into())
    }

    pub fn key_pair(&self) -> Result<KeyPair> {
        Ok(KeyPair::new(self.public_key()?, self.private_key()?))
    }

    pub fn public_key(&self) -> Result<PublicKey> {
        Ok(PublicKey::from_stored_public_key_bytes(
            self.pre_key
                .public_key
                .as_ref()
                .ok_or(SignalProtocolError::InvalidProtobufEncoding)?,
        )?)
    }

    pub fn private_key(&self) -> Result<PrivateKey> {
        Ok(PrivateKey::deserialize(
            self.pre_key
                .private_key
                .as_ref()
                .ok_or(SignalProtocolError::InvalidProtobufEncoding)?,
        )?)
    }

    pub fn serialize(&self) -> Result<Vec<u8>> {
        Ok(waproto::codec::pre_key_record_to_vec(&self.pre_key))
    }
}

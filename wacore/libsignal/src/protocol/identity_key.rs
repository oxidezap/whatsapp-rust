//
// Copyright 2020-2022 Signal Messenger, LLC.
// SPDX-License-Identifier: AGPL-3.0-only
//

//! Wrappers over cryptographic primitives from [`libsignal_core::curve`] to represent a user.

#![warn(missing_docs)]

use buffa::Message;
use rand::{CryptoRng, Rng};
use std::sync::Arc;

use crate::protocol::{
    KeyPair, PrivateKey, PublicKey, Result, SignalProtocolError, stores::IdentityKeyPairStructure,
};

/// A public key that represents the identity of a user.
///
/// Wrapper for [`PublicKey`].
#[derive(
    Debug, PartialOrd, Ord, PartialEq, Eq, Clone, Copy, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct IdentityKey {
    public_key: PublicKey,
}

impl From<PublicKey> for IdentityKey {
    #[inline]
    fn from(public_key: PublicKey) -> Self {
        Self { public_key }
    }
}

impl From<IdentityKey> for PublicKey {
    #[inline]
    fn from(identity: IdentityKey) -> Self {
        identity.public_key
    }
}

impl IdentityKey {
    /// Initialize a public-facing identity from a public key.
    pub fn new(public_key: PublicKey) -> Self {
        Self { public_key }
    }

    /// Return the public key representing this identity.
    #[inline]
    pub fn public_key(&self) -> &PublicKey {
        &self.public_key
    }

    /// Serialize the identity key to a fixed-size array (1 type byte + 32 key bytes).
    #[inline]
    pub fn serialize(&self) -> [u8; 33] {
        self.public_key.serialize()
    }

    /// Deserialize a public identity from a byte slice.
    pub fn decode(value: &[u8]) -> Result<Self> {
        let pk = PublicKey::try_from(value)?;
        Ok(Self { public_key: pk })
    }
}

impl TryFrom<&[u8]> for IdentityKey {
    type Error = SignalProtocolError;

    fn try_from(value: &[u8]) -> Result<Self> {
        IdentityKey::decode(value)
    }
}

/// The private identity of a user.
///
/// Can be converted to and from [`KeyPair`].
#[derive(Clone, serde::Serialize, serde::Deserialize)]
pub struct IdentityKeyPair {
    identity_key: IdentityKey,
    private_key: PrivateKey,
    // Wire-only future state stays out of the existing serde object contract.
    // The descriptor guard separately rejects newly known storage fields.
    #[serde(skip)]
    unknown_fields: Option<Arc<buffa::UnknownFields>>,
}

impl IdentityKeyPair {
    /// Create a key pair from a public `identity_key` and a private `private_key`.
    pub fn new(identity_key: IdentityKey, private_key: PrivateKey) -> Self {
        Self {
            identity_key,
            private_key,
            unknown_fields: None,
        }
    }

    /// Generate a random new identity from randomness in `csprng`.
    pub fn generate<R: CryptoRng + Rng>(csprng: &mut R) -> Self {
        let keypair = KeyPair::generate(csprng);

        Self::from(keypair)
    }

    /// Return the public identity of this user.
    #[inline]
    pub fn identity_key(&self) -> &IdentityKey {
        &self.identity_key
    }

    /// Return the public key that defines this identity.
    #[inline]
    pub fn public_key(&self) -> &PublicKey {
        self.identity_key.public_key()
    }

    /// Return the private key that defines this identity.
    #[inline]
    pub fn private_key(&self) -> &PrivateKey {
        &self.private_key
    }

    /// Return a byte slice which can later be deserialized with [`Self::try_from`].
    // IdentityKeyPairStructure round-trips only here; no codec pin needed.
    #[allow(clippy::disallowed_methods)]
    pub fn serialize(&self) -> Box<[u8]> {
        let structure = {
            let mut proto = IdentityKeyPairStructure::default();
            proto.public_key = Some(self.identity_key.serialize().to_vec());
            proto.private_key = Some(self.private_key.serialize().to_vec());
            if let Some(fields) = &self.unknown_fields {
                proto.__buffa_unknown_fields = fields.as_ref().clone().into();
            }
            proto
        };

        let result = structure.encode_to_vec();
        result.into_boxed_slice()
    }
}

impl TryFrom<&[u8]> for IdentityKeyPair {
    type Error = SignalProtocolError;

    #[allow(clippy::disallowed_methods)]
    fn try_from(value: &[u8]) -> Result<Self> {
        let structure = IdentityKeyPairStructure::decode_from_slice(value)
            .map_err(|_| SignalProtocolError::InvalidProtobufEncoding)?;
        Ok(Self {
            identity_key: IdentityKey::try_from(
                structure
                    .public_key
                    .as_ref()
                    .ok_or(SignalProtocolError::InvalidProtobufEncoding)?
                    .as_slice(),
            )?,
            private_key: PrivateKey::deserialize(
                structure
                    .private_key
                    .as_ref()
                    .ok_or(SignalProtocolError::InvalidProtobufEncoding)?,
            )?,
            unknown_fields: (!structure.__buffa_unknown_fields.is_empty())
                .then(|| Arc::new(structure.__buffa_unknown_fields.into())),
        })
    }
}

impl TryFrom<PrivateKey> for IdentityKeyPair {
    type Error = SignalProtocolError;

    fn try_from(private_key: PrivateKey) -> Result<Self> {
        let identity_key = IdentityKey::new(private_key.public_key()?);
        Ok(Self::new(identity_key, private_key))
    }
}

impl From<KeyPair> for IdentityKeyPair {
    fn from(value: KeyPair) -> Self {
        Self::new(value.public_key.into(), value.private_key)
    }
}

impl From<IdentityKeyPair> for KeyPair {
    fn from(value: IdentityKeyPair) -> Self {
        Self::new(value.identity_key.into(), value.private_key)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_wire_round_trip_retains_unknown_fields_after_clone() {
        let original = IdentityKeyPair::generate(&mut rand::rng());
        let known = original.serialize();
        let mut future = known.to_vec();
        // Repeated unknown varints, bytes, and a nested unknown group.
        future.extend_from_slice(&[
            0xa0, 0x06, 7, 0xa0, 0x06, 9, 0xaa, 0x06, 2, 0x12, 0x34, 0xb3, 0x06, 8, 1, 0xb4, 0x06,
        ]);
        // The compatibility encoder emits retained fields before typed fields;
        // their values and relative order must still survive exactly.
        let mut expected = future[known.len()..].to_vec();
        expected.extend_from_slice(&known);
        let restored =
            IdentityKeyPair::try_from(future.as_slice()).expect("future identity record");
        assert_eq!(restored.serialize().as_ref(), expected);
        assert_eq!(restored.identity_key(), original.identity_key());
        assert_eq!(
            restored.private_key().serialize(),
            original.private_key().serialize()
        );
        let cloned = restored.clone();
        drop(restored);
        assert_eq!(cloned.serialize().as_ref(), expected);
        assert_eq!(
            serde_json::to_value(&cloned).expect("future identity JSON"),
            serde_json::to_value(&original).expect("known identity JSON")
        );
        assert!(
            IdentityKeyPair::try_from(known.as_ref())
                .expect("known identity record")
                .unknown_fields
                .is_none()
        );
    }
}

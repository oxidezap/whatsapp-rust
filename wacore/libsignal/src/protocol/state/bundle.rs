//
// Copyright 2020 Signal Messenger, LLC.
// SPDX-License-Identifier: AGPL-3.0-only
//

use std::clone::Clone;

use crate::protocol::state::{PreKeyId, SignedPreKeyId};
use crate::protocol::{DeviceId, IdentityKey, PublicKey, Result, SignalProtocolError};

/// Length of the XEdDSA signature over a signed pre-key. Fixed by the curve, so
/// the bundle stores it inline: a `Vec` here cost one allocation per bundle and
/// another per clone, for 64 bytes that can never be any other size.
pub const SIGNED_PRE_KEY_SIGNATURE_LEN: usize = 64;

#[derive(Clone)]
struct SignedPreKey {
    id: SignedPreKeyId,
    public_key: PublicKey,
    signature: [u8; SIGNED_PRE_KEY_SIGNATURE_LEN],
}

impl SignedPreKey {
    fn new(
        id: SignedPreKeyId,
        public_key: PublicKey,
        signature: [u8; SIGNED_PRE_KEY_SIGNATURE_LEN],
    ) -> Self {
        Self {
            id,
            public_key,
            signature,
        }
    }
}

// Represents the raw contents of the pre-key bundle without any notion of required/optional
// fields.
// Can be used as a "builder" for PreKeyBundle, in which case all the validation will happen in
// PreKeyBundle::new.
pub struct PreKeyBundleContent {
    pub registration_id: Option<u32>,
    pub device_id: Option<DeviceId>,
    pub pre_key_id: Option<PreKeyId>,
    pub pre_key_public: Option<PublicKey>,
    pub ec_pre_key_id: Option<SignedPreKeyId>,
    pub ec_pre_key_public: Option<PublicKey>,
    pub ec_pre_key_signature: Option<Vec<u8>>,
    pub identity_key: Option<IdentityKey>,
}

impl From<PreKeyBundle> for PreKeyBundleContent {
    fn from(bundle: PreKeyBundle) -> Self {
        Self {
            registration_id: Some(bundle.registration_id),
            device_id: Some(bundle.device_id),
            pre_key_id: bundle.pre_key_id,
            pre_key_public: bundle.pre_key_public,
            ec_pre_key_id: Some(bundle.ec_signed_pre_key.id),
            ec_pre_key_public: Some(bundle.ec_signed_pre_key.public_key),
            ec_pre_key_signature: Some(bundle.ec_signed_pre_key.signature.to_vec()),
            identity_key: Some(bundle.identity_key),
        }
    }
}

impl TryFrom<PreKeyBundleContent> for PreKeyBundle {
    type Error = SignalProtocolError;

    fn try_from(content: PreKeyBundleContent) -> Result<Self> {
        let bundle = PreKeyBundle::new(
            content.registration_id.ok_or_else(|| {
                SignalProtocolError::InvalidArgument("registration_id is required".to_string())
            })?,
            content.device_id.ok_or_else(|| {
                SignalProtocolError::InvalidArgument("device_id is required".to_string())
            })?,
            content.pre_key_id.zip(content.pre_key_public),
            content.ec_pre_key_id.ok_or_else(|| {
                SignalProtocolError::InvalidArgument("signed_pre_key_id is required".to_string())
            })?,
            content.ec_pre_key_public.ok_or_else(|| {
                SignalProtocolError::InvalidArgument(
                    "signed_pre_key_public is required".to_string(),
                )
            })?,
            content.ec_pre_key_signature.ok_or_else(|| {
                SignalProtocolError::InvalidArgument(
                    "signed_pre_key_signature is required".to_string(),
                )
            })?,
            content.identity_key.ok_or_else(|| {
                SignalProtocolError::InvalidArgument("identity_key is required".to_string())
            })?,
        )?;

        Ok(bundle)
    }
}

#[derive(Clone)]
pub struct PreKeyBundle {
    registration_id: u32,
    device_id: DeviceId,
    pre_key_id: Option<PreKeyId>,
    pre_key_public: Option<PublicKey>,
    ec_signed_pre_key: SignedPreKey,
    identity_key: IdentityKey,
}

impl PreKeyBundle {
    /// `signed_pre_key_signature` is anything that converts into the fixed
    /// 64-byte signature: a `[u8; 64]` straight off `calculate_signature` passes
    /// through with no allocation, while a `Vec<u8>` from a store row is checked
    /// and rejected if it is not exactly 64 bytes.
    pub fn new(
        registration_id: u32,
        device_id: DeviceId,
        pre_key: Option<(PreKeyId, PublicKey)>,
        signed_pre_key_id: SignedPreKeyId,
        signed_pre_key_public: PublicKey,
        signed_pre_key_signature: impl TryInto<[u8; SIGNED_PRE_KEY_SIGNATURE_LEN]>,
        identity_key: IdentityKey,
    ) -> Result<Self> {
        let signature = signed_pre_key_signature.try_into().map_err(|_| {
            SignalProtocolError::InvalidArgument(format!(
                "signed_pre_key_signature must be {SIGNED_PRE_KEY_SIGNATURE_LEN} bytes"
            ))
        })?;
        // The conversion is the only generic part; the rest stays out of line so
        // each argument type costs one thin wrapper rather than a second copy of
        // the constructor.
        Ok(Self::assemble(
            registration_id,
            device_id,
            pre_key,
            signed_pre_key_id,
            signed_pre_key_public,
            signature,
            identity_key,
        ))
    }

    fn assemble(
        registration_id: u32,
        device_id: DeviceId,
        pre_key: Option<(PreKeyId, PublicKey)>,
        signed_pre_key_id: SignedPreKeyId,
        signed_pre_key_public: PublicKey,
        signed_pre_key_signature: [u8; SIGNED_PRE_KEY_SIGNATURE_LEN],
        identity_key: IdentityKey,
    ) -> Self {
        let (pre_key_id, pre_key_public) = match pre_key {
            None => (None, None),
            Some((id, key)) => (Some(id), Some(key)),
        };

        let ec_signed_pre_key = SignedPreKey::new(
            signed_pre_key_id,
            signed_pre_key_public,
            signed_pre_key_signature,
        );

        Self {
            registration_id,
            device_id,
            pre_key_id,
            pre_key_public,
            ec_signed_pre_key,
            identity_key,
        }
    }

    pub fn registration_id(&self) -> Result<u32> {
        Ok(self.registration_id)
    }

    pub fn device_id(&self) -> Result<DeviceId> {
        Ok(self.device_id)
    }

    pub fn pre_key_id(&self) -> Result<Option<PreKeyId>> {
        Ok(self.pre_key_id)
    }

    pub fn pre_key_public(&self) -> Result<Option<PublicKey>> {
        Ok(self.pre_key_public)
    }

    pub fn signed_pre_key_id(&self) -> Result<SignedPreKeyId> {
        Ok(self.ec_signed_pre_key.id)
    }

    pub fn signed_pre_key_public(&self) -> Result<PublicKey> {
        Ok(self.ec_signed_pre_key.public_key)
    }

    pub fn signed_pre_key_signature(&self) -> Result<&[u8]> {
        Ok(&self.ec_signed_pre_key.signature)
    }

    pub fn identity_key(&self) -> Result<&IdentityKey> {
        Ok(&self.identity_key)
    }

    pub fn modify<F>(self, modify: F) -> Result<Self>
    where
        F: FnOnce(&mut PreKeyBundleContent),
    {
        let mut content = self.into();
        modify(&mut content);
        content.try_into()
    }
}

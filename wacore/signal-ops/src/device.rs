// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

use crate::error::OpsError;
use rand::{Rng, RngExt};
use wacore::libsignal::protocol::{IdentityKeyPair, KeyPair, PrivateKey, PublicKey};

/// The local device's own keys. Private halves never leave the Signal store.
#[derive(Clone)]
pub struct DeviceKeys {
    pub identity: KeyPair,
    pub registration_id: u32,
    pub adv_secret: [u8; 32],
    pub signed_prekey_id: u32,
    pub signed_prekey: KeyPair,
    pub signed_prekey_signature: [u8; 64],
    /// Next one-time prekey ID to hand out. WhatsApp IDs are 24-bit.
    pub next_prekey_id: u32,
    /// Encoded `ADVSignedDeviceIdentity` from pairing. Sent with prekey
    /// messages so recipients can verify this device belongs to the account.
    pub account: Option<Vec<u8>>,
    /// This device's phone-number JID (`user:device@s.whatsapp.net`).
    pub own_pn: Option<String>,
    /// This device's LID (`user:device@lid`).
    pub own_lid: Option<String>,
}

/// Everything the WhatsApp client needs to register and pair the device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicIdentity {
    pub registration_id: u32,
    /// 32-byte Curve25519 public key, without the type byte.
    pub identity_key: [u8; 32],
    pub signed_prekey: PublicSignedPreKey,
    /// Shared with the primary through the QR code; an HMAC key for pairing,
    /// not a decryption key.
    pub adv_secret: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicSignedPreKey {
    pub id: u32,
    pub public_key: [u8; 32],
    pub signature: [u8; 64],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicPreKey {
    pub id: u32,
    pub public_key: [u8; 32],
}

const FORMAT_V1: u8 = 1;
const FORMAT_V2: u8 = 2;
/// Fixed part shared by both formats; format 2 appends the paired identity.
const FIXED_LEN: usize = 1 + 32 + 4 + 32 + 4 + 32 + 64 + 4;
/// One-time prekey IDs are 3 bytes on the wire.
pub(crate) const MAX_PREKEY_ID: u32 = 0x00FF_FFFF;

fn public32(key: &PublicKey) -> [u8; 32] {
    let mut out = [0u8; 32];
    out.copy_from_slice(key.public_key_bytes());
    out
}

impl DeviceKeys {
    /// Fresh keys, generated the way wacore's own `Device::new` does.
    pub fn generate() -> Result<Self, OpsError> {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let identity_pair = IdentityKeyPair::generate(&mut rng);
        let identity = KeyPair::new(
            *identity_pair.public_key(),
            identity_pair.private_key().clone(),
        );
        let signed_prekey = KeyPair::generate(&mut rng);
        let signed_prekey_signature = sign_prekey(&identity, &signed_prekey)?;
        let mut adv_secret = [0u8; 32];
        rng.fill_bytes(&mut adv_secret);
        Ok(Self {
            identity,
            registration_id: rng.random_range(1..=2_147_483_647),
            adv_secret,
            signed_prekey_id: 1,
            signed_prekey,
            signed_prekey_signature,
            next_prekey_id: 1,
            account: None,
            own_pn: None,
            own_lid: None,
        })
    }

    pub fn identity_pair(&self) -> IdentityKeyPair {
        self.identity.clone().into()
    }

    pub fn public(&self) -> PublicIdentity {
        PublicIdentity {
            registration_id: self.registration_id,
            identity_key: public32(&self.identity.public_key),
            signed_prekey: PublicSignedPreKey {
                id: self.signed_prekey_id,
                public_key: public32(&self.signed_prekey.public_key),
                signature: self.signed_prekey_signature,
            },
            adv_secret: self.adv_secret,
        }
    }

    /// Layout, format 2: version, identity private, registration ID, ADV
    /// secret, signed prekey ID, signed prekey private, signature, next prekey
    /// ID, then three length-prefixed fields (u32 account, u16 own PN, u16 own
    /// LID; length 0 means absent). Integers are big-endian. Public keys are
    /// derived on load. Format 1 is the fixed part alone and still decodes.
    /// Encode for storage. Refuses a field too long for its length prefix
    /// (the account blob over `u32::MAX` bytes, a JID over `u16::MAX`), since
    /// a truncated prefix would make the stored record undecodable.
    pub fn encode(&self) -> Result<Vec<u8>, OpsError> {
        let too_long = |what: &str| OpsError::InvalidInput(format!("device keys: {what} too long"));
        let mut out = Vec::with_capacity(FIXED_LEN + 8);
        out.push(FORMAT_V2);
        out.extend_from_slice(self.identity.private_key.serialize());
        out.extend_from_slice(&self.registration_id.to_be_bytes());
        out.extend_from_slice(&self.adv_secret);
        out.extend_from_slice(&self.signed_prekey_id.to_be_bytes());
        out.extend_from_slice(self.signed_prekey.private_key.serialize());
        out.extend_from_slice(&self.signed_prekey_signature);
        out.extend_from_slice(&self.next_prekey_id.to_be_bytes());
        let account = self.account.as_deref().unwrap_or_default();
        let account_len = u32::try_from(account.len()).map_err(|_| too_long("account"))?;
        out.extend_from_slice(&account_len.to_be_bytes());
        out.extend_from_slice(account);
        for jid in [&self.own_pn, &self.own_lid] {
            let jid = jid.as_deref().unwrap_or_default().as_bytes();
            let jid_len = u16::try_from(jid.len()).map_err(|_| too_long("JID"))?;
            out.extend_from_slice(&jid_len.to_be_bytes());
            out.extend_from_slice(jid);
        }
        Ok(out)
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, OpsError> {
        let invalid = |what: &str| OpsError::InvalidInput(format!("device keys: {what}"));
        let format = *bytes.first().ok_or_else(|| invalid("empty"))?;
        match format {
            FORMAT_V1 if bytes.len() == FIXED_LEN => {}
            FORMAT_V2 if bytes.len() >= FIXED_LEN => {}
            FORMAT_V1 | FORMAT_V2 => return Err(invalid("wrong length")),
            _ => return Err(invalid("unsupported format")),
        }
        let mut at = 1;
        let mut take = |len: usize| -> Result<&[u8], OpsError> {
            let slice = bytes
                .get(at..at + len)
                .ok_or_else(|| invalid("truncated"))?;
            at += len;
            Ok(slice)
        };
        let key_pair = |private: &[u8]| -> Result<KeyPair, OpsError> {
            let private = PrivateKey::deserialize(private).map_err(|_| invalid("private key"))?;
            let public = private.public_key().map_err(|_| invalid("public key"))?;
            Ok(KeyPair::new(public, private))
        };
        let u32_at = |s: &[u8]| u32::from_be_bytes([s[0], s[1], s[2], s[3]]);

        let identity = key_pair(take(32)?)?;
        let registration_id = u32_at(take(4)?);
        let adv_secret: [u8; 32] = take(32)?.try_into().map_err(|_| invalid("adv secret"))?;
        let signed_prekey_id = u32_at(take(4)?);
        let signed_prekey = key_pair(take(32)?)?;
        let signed_prekey_signature: [u8; 64] =
            take(64)?.try_into().map_err(|_| invalid("signature"))?;
        let next_prekey_id = u32_at(take(4)?);
        let (mut account, mut own_pn, mut own_lid) = (None, None, None);
        if format == FORMAT_V2 {
            let len = u32_at(take(4)?) as usize;
            account = Some(take(len)?.to_vec()).filter(|a| !a.is_empty());
            let mut jid = || -> Result<Option<String>, OpsError> {
                let len_bytes = take(2)?;
                let len = u16::from_be_bytes([len_bytes[0], len_bytes[1]]) as usize;
                let raw = take(len)?;
                let text = std::str::from_utf8(raw).map_err(|_| invalid("jid is not UTF-8"))?;
                Ok(Some(text.to_owned()).filter(|j| !j.is_empty()))
            };
            own_pn = jid()?;
            own_lid = jid()?;
        }
        if at != bytes.len() {
            return Err(invalid("trailing bytes"));
        }
        Ok(Self {
            identity,
            registration_id,
            adv_secret,
            signed_prekey_id,
            signed_prekey,
            signed_prekey_signature,
            next_prekey_id,
            account,
            own_pn,
            own_lid,
        })
    }
}

/// WhatsApp signs the serialized (type-prefixed) signed prekey public key.
pub(crate) fn sign_prekey(identity: &KeyPair, prekey: &KeyPair) -> Result<[u8; 64], OpsError> {
    let mut rng = rand::make_rng::<rand::rngs::StdRng>();
    let signature = identity
        .private_key
        .calculate_signature(&prekey.public_key.serialize(), &mut rng)
        .map_err(|e| OpsError::InvalidInput(format!("signing the prekey failed: {e}")))?;
    signature
        .as_ref()
        .try_into()
        .map_err(|_| OpsError::InvalidInput("signature is not 64 bytes".into()))
}

pub(crate) fn public_bytes(key: &PublicKey) -> [u8; 32] {
    public32(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_round_trip_through_the_encoding() {
        let keys = DeviceKeys::generate().unwrap();
        let decoded = DeviceKeys::decode(&keys.encode().unwrap()).unwrap();
        assert_eq!(decoded.public(), keys.public());
        assert_eq!(decoded.next_prekey_id, keys.next_prekey_id);
        assert_eq!(
            decoded.identity.private_key.serialize(),
            keys.identity.private_key.serialize()
        );
    }

    #[test]
    fn the_signed_prekey_signature_verifies_against_the_identity() {
        let keys = DeviceKeys::generate().unwrap();
        assert!(keys.identity.public_key.verify_signature(
            &keys.signed_prekey.public_key.serialize(),
            &keys.signed_prekey_signature
        ));
    }

    #[test]
    fn a_field_too_long_for_its_length_prefix_is_refused() {
        let mut keys = DeviceKeys::generate().unwrap();
        keys.own_pn = Some("1".repeat(usize::from(u16::MAX) + 1));
        assert!(matches!(keys.encode(), Err(OpsError::InvalidInput(_))));
        keys.own_pn = Some("1".repeat(usize::from(u16::MAX)));
        let decoded = DeviceKeys::decode(&keys.encode().unwrap()).unwrap();
        assert_eq!(decoded.own_pn, keys.own_pn);
    }

    #[test]
    fn the_paired_identity_round_trips_and_format_1_still_decodes() {
        let mut keys = DeviceKeys::generate().unwrap();
        keys.account = Some(vec![1, 2, 3]);
        keys.own_pn = Some("15550001111:4@s.whatsapp.net".into());
        keys.own_lid = Some("123456789:4@lid".into());
        let decoded = DeviceKeys::decode(&keys.encode().unwrap()).unwrap();
        assert_eq!(decoded.account, keys.account);
        assert_eq!(decoded.own_pn, keys.own_pn);
        assert_eq!(decoded.own_lid, keys.own_lid);

        let mut v1 = keys.encode().unwrap()[..FIXED_LEN].to_vec();
        v1[0] = FORMAT_V1;
        let old = DeviceKeys::decode(&v1).unwrap();
        assert_eq!(old.public(), keys.public());
        assert_eq!(old.account, None);
    }

    #[test]
    fn corrupt_encodings_are_rejected() {
        let mut bytes = DeviceKeys::generate().unwrap().encode().unwrap();
        assert!(DeviceKeys::decode(&bytes[..10]).is_err());
        bytes[0] = 9;
        assert!(DeviceKeys::decode(&bytes).is_err());
    }
}

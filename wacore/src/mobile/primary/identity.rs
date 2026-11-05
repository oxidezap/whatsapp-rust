use buffa::Message as _;
use rand::{CryptoRng, Rng};
use subtle::ConstantTimeEq;
use wacore_binary::JidExt as _;
use waproto::whatsapp as wa;

use crate::adv::{AccountSignatureMessage, AdvValidation, validate_adv_with_identity_key};
use crate::libsignal::crypto::hmac_sha256;
use crate::libsignal::protocol::{KeyPair, PublicKey};

use super::PrimaryLinkError;

/// The pinned reference clients differ in presence of protobuf zero enums.
/// Callers choose a qualified profile rather than silently normalizing signed bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdvDefaultEncoding {
    Omit,
    Explicit,
}

pub struct IdentityParameters<'a> {
    pub raw_id: u32,
    pub timestamp_seconds: u64,
    pub key_index: u32,
    pub valid_indexes: &'a [u32],
    pub default_encoding: AdvDefaultEncoding,
}

impl IdentityParameters<'_> {
    fn validate(&self) -> Result<(), PrimaryLinkError> {
        if self.valid_indexes.is_empty()
            || self.valid_indexes.len() > 4096
            || !self.valid_indexes.contains(&0)
        {
            return Err(PrimaryLinkError::InvalidInput("key-index set"));
        }
        let unique: std::collections::BTreeSet<_> = self.valid_indexes.iter().collect();
        if unique.len() != self.valid_indexes.len() {
            return Err(PrimaryLinkError::InvalidInput("duplicate key index"));
        }
        Ok(())
    }
    fn encryption_type(&self) -> Option<wa::ADVEncryptionType> {
        (self.default_encoding == AdvDefaultEncoding::Explicit)
            .then_some(wa::ADVEncryptionType::E2EE)
    }
}

/// Signed public material for one companion; this does not prove it was linked.
#[derive(Clone)]
pub struct SignedCompanionIdentity {
    pub device_identity: Vec<u8>,
    pub key_index_list: Vec<u8>,
}

pub fn build_signed_key_index_list<R: Rng + CryptoRng>(
    primary: &KeyPair,
    params: &IdentityParameters<'_>,
    rng: &mut R,
) -> Result<Vec<u8>, PrimaryLinkError> {
    params.validate()?;
    validate_pair(primary)?;
    let mut details = wa::ADVKeyIndexList::default();
    details.raw_id = Some(params.raw_id);
    details.timestamp = Some(params.timestamp_seconds);
    details.current_index = Some(params.key_index);
    details.valid_indexes = params.valid_indexes.to_vec();
    details.account_type = params.encryption_type();
    let encoded = waproto::codec::adv_key_index_list_to_vec(&details);
    let mut message = Vec::with_capacity(2 + encoded.len());
    message.extend_from_slice(&[6, 2]);
    message.extend_from_slice(&encoded);
    let signature = primary.calculate_signature(&message, rng)?;
    let mut signed = wa::ADVSignedKeyIndexList::default();
    signed.details = Some(encoded);
    signed.account_signature = Some(signature.to_vec());
    Ok(waproto::codec::adv_signed_key_index_list_to_vec(&signed))
}

pub fn build_signed_companion_identity<R: Rng + CryptoRng>(
    primary: &KeyPair,
    companion_identity: &[u8; 32],
    adv_secret: &[u8; 32],
    params: &IdentityParameters<'_>,
    rng: &mut R,
) -> Result<SignedCompanionIdentity, PrimaryLinkError> {
    params.validate()?;
    validate_pair(primary)?;
    if params.key_index == 0 || !params.valid_indexes.contains(&params.key_index) {
        return Err(PrimaryLinkError::InvalidInput("companion key index"));
    }
    let companion = PublicKey::from_djb_public_key_bytes(companion_identity)?;
    let agreement = zeroize::Zeroizing::new(primary.calculate_agreement(&companion)?);
    if bool::from(agreement.as_ref().ct_eq(&[0; 32])) {
        return Err(PrimaryLinkError::Authentication);
    }
    let mut details = wa::ADVDeviceIdentity::default();
    details.raw_id = Some(params.raw_id);
    details.timestamp = Some(params.timestamp_seconds);
    details.key_index = Some(params.key_index);
    details.account_type = params.encryption_type();
    details.device_type = params.encryption_type();
    let encoded = details.encode_to_vec();
    let message = AccountSignatureMessage::new(&encoded, &companion, details.device_type);
    let signature = primary.calculate_signature(message.as_bytes(), rng)?;
    let mut signed = wa::ADVSignedDeviceIdentity::default();
    signed.details = Some(encoded);
    signed.account_signature_key = Some(primary.public_key.public_key_bytes().to_vec());
    signed.account_signature = Some(signature.to_vec());
    let encoded = waproto::codec::adv_signed_device_identity_to_vec(&signed);
    let mut wrapped = wa::ADVSignedDeviceIdentityHMAC::default();
    wrapped.hmac = Some(hmac_sha256(adv_secret, &encoded).to_vec());
    wrapped.details = Some(encoded);
    wrapped.account_type = params.encryption_type();
    Ok(SignedCompanionIdentity {
        device_identity: wrapped.encode_to_vec(),
        key_index_list: build_signed_key_index_list(primary, params, rng)?,
    })
}

/// Allocate beyond the journal's high-water mark and every currently observed
/// live index. Revoked indexes remain spent; a server snapshot cannot rewind it.
pub fn next_companion_key_index(
    high_water: u32,
    live_indexes: &[u32],
) -> Result<u32, PrimaryLinkError> {
    if live_indexes.len() > 4096 {
        return Err(PrimaryLinkError::InvalidInput("key-index set"));
    }
    live_indexes
        .iter()
        .copied()
        .fold(high_water, u32::max)
        .checked_add(1)
        .ok_or(PrimaryLinkError::InvalidInput("key-index exhaustion"))
}

/// Validate an authenticated login's self identity against the already registered
/// primary key. A self-consistent certificate cannot introduce another account key.
pub fn validate_primary_account_identity(
    bytes: &[u8],
    primary_identity: &[u8; 32],
    own_account: &wacore_binary::Jid,
) -> Result<wa::ADVSignedDeviceIdentity, PrimaryLinkError> {
    super::validate_accounts(std::slice::from_ref(own_account))?;
    if bytes.is_empty()
        || bytes.len() > 16384
        || validate_adv_with_identity_key(
            bytes,
            primary_identity,
            Some(primary_identity),
            own_account,
        ) != AdvValidation::Valid
    {
        return Err(PrimaryLinkError::Authentication);
    }
    Ok(waproto::codec::adv_signed_device_identity_decode(bytes)?)
}

fn validate_pair(pair: &KeyPair) -> Result<(), PrimaryLinkError> {
    if pair.private_key.public_key()? != pair.public_key {
        return Err(PrimaryLinkError::InvalidInput("identity key-pair mismatch"));
    }
    Ok(())
}

/// Authenticate the exact issued details and both identity signatures before a
/// host records link confirmation. A pair-device IQ acknowledgement is insufficient.
pub fn verify_companion_proof(
    issued: &SignedCompanionIdentity,
    proof: &[u8],
    companion_identity: &[u8; 32],
    own_accounts: &[wacore_binary::Jid],
    companion_jid: &wacore_binary::Jid,
) -> Result<(), PrimaryLinkError> {
    if proof.len() > 16384 || companion_jid.device == 0 || companion_jid.is_hosted() {
        return Err(PrimaryLinkError::InvalidInput("companion proof"));
    }
    super::validate_companion(companion_jid, own_accounts)?;
    let envelope = waproto::codec::adv_signed_device_identity_hmac_decode(&issued.device_identity)?;
    let signed = waproto::codec::adv_signed_device_identity_decode(
        envelope
            .details
            .as_deref()
            .ok_or(PrimaryLinkError::Authentication)?,
    )?;
    let candidate = waproto::codec::adv_signed_device_identity_decode(proof)?;
    let expected_details = signed
        .details
        .as_deref()
        .ok_or(PrimaryLinkError::Authentication)?;
    let supplied_details = candidate
        .details
        .as_deref()
        .ok_or(PrimaryLinkError::Authentication)?;
    if !bool::from(expected_details.ct_eq(supplied_details)) {
        return Err(PrimaryLinkError::Authentication);
    }
    let account: &[u8; 32] = signed
        .account_signature_key
        .as_deref()
        .ok_or(PrimaryLinkError::Authentication)?
        .try_into()
        .map_err(|_| PrimaryLinkError::Authentication)?;
    if validate_adv_with_identity_key(proof, companion_identity, Some(account), companion_jid)
        != AdvValidation::Valid
    {
        return Err(PrimaryLinkError::Authentication);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pair::{DeviceState, PairUtils};
    use rand::SeedableRng;
    use std::str::FromStr;

    #[test]
    fn generated_identity_and_key_indexes_pass_existing_companion_verifiers() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(107);
        let primary = KeyPair::generate(&mut rng);
        let companion = DeviceState {
            identity_key: KeyPair::generate(&mut rng),
            noise_key: KeyPair::generate(&mut rng),
            adv_secret_key: [4; 32],
        };
        let peer: &[u8; 32] = companion
            .identity_key
            .public_key
            .public_key_bytes()
            .try_into()
            .unwrap();
        let account = wacore_binary::Jid::from_str("123@s.whatsapp.net").unwrap();
        let device = wacore_binary::Jid::from_str("123:2@s.whatsapp.net").unwrap();
        for encoding in [AdvDefaultEncoding::Omit, AdvDefaultEncoding::Explicit] {
            let params = IdentityParameters {
                raw_id: 7,
                timestamp_seconds: 12345,
                key_index: 2,
                valid_indexes: &[0, 2],
                default_encoding: encoding,
            };
            let issued = build_signed_companion_identity(
                &primary,
                peer,
                &companion.adv_secret_key,
                &params,
                &mut rng,
            )
            .unwrap();
            let (proof, index) =
                PairUtils::do_pair_crypto(&companion, &issued.device_identity).unwrap();
            assert_eq!(index, 2);
            verify_companion_proof(
                &issued,
                &proof,
                peer,
                std::slice::from_ref(&account),
                &device,
            )
            .unwrap();
            assert!(
                crate::adv::verify_key_index_list(
                    &issued.key_index_list,
                    primary.public_key.public_key_bytes().try_into().unwrap()
                )
                .is_some()
            );
            let envelope =
                waproto::codec::adv_signed_device_identity_hmac_decode(&issued.device_identity)
                    .unwrap();
            assert_eq!(
                envelope.account_type.is_some(),
                encoding == AdvDefaultEncoding::Explicit
            );
            let signed = waproto::codec::adv_signed_device_identity_decode(
                envelope.details.as_deref().unwrap(),
            )
            .unwrap();
            let details =
                waproto::codec::adv_device_identity_decode(signed.details.as_deref().unwrap())
                    .unwrap();
            assert_eq!(
                details.account_type.is_some(),
                encoding == AdvDefaultEncoding::Explicit
            );
            assert_eq!(
                details.device_type.is_some(),
                encoding == AdvDefaultEncoding::Explicit
            );
            for bad in [
                "999:2@s.whatsapp.net",
                "123:2@g.us",
                "123:2@newsletter",
                "123@s.whatsapp.net",
            ] {
                assert!(
                    verify_companion_proof(
                        &issued,
                        &proof,
                        peer,
                        std::slice::from_ref(&account),
                        &wacore_binary::Jid::from_str(bad).unwrap()
                    )
                    .is_err()
                );
            }
            let pending = crate::mobile::primary::PendingCompanion::new(
                b"active-ref",
                std::slice::from_ref(&account),
                peer,
                SignedCompanionIdentity {
                    device_identity: issued.device_identity.clone(),
                    key_index_list: issued.key_index_list.clone(),
                },
            )
            .unwrap();
            let acknowledged = pending.acknowledge(&device).unwrap();
            assert!(acknowledged.verify(b"stale-ref", &device, &proof).is_err());
            let foreign = wacore_binary::Jid::from_str("123:3@s.whatsapp.net").unwrap();
            assert!(
                acknowledged
                    .verify(b"active-ref", &foreign, &proof)
                    .is_err()
            );
            let verified = acknowledged.verify(b"active-ref", &device, &proof).unwrap();
            assert_eq!(verified.device_jid(), &device);
            assert_eq!(verified.peer_identity(), peer);
            assert_eq!(verified.signed_identity(), proof);
            let mut tampered = proof;
            tampered[0] ^= 1;
            assert!(
                verify_companion_proof(
                    &issued,
                    &tampered,
                    peer,
                    std::slice::from_ref(&account),
                    &device
                )
                .is_err()
            );
        }
    }
    #[test]
    fn key_index_allocation_never_reuses_revoked_or_live_indexes() {
        assert_eq!(next_companion_key_index(9, &[0, 2, 7]).unwrap(), 10);
        assert_eq!(next_companion_key_index(9, &[0, 12]).unwrap(), 13);
        assert!(next_companion_key_index(u32::MAX, &[0]).is_err());
    }
    #[test]
    fn published_list_keeps_high_water_after_revoking_last_companion() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(110);
        let primary = KeyPair::generate(&mut rng);
        let params = IdentityParameters {
            raw_id: 7,
            timestamp_seconds: 9,
            key_index: 4,
            valid_indexes: &[0],
            default_encoding: AdvDefaultEncoding::Omit,
        };
        let signed = build_signed_key_index_list(&primary, &params, &mut rng).unwrap();
        assert!(
            crate::adv::verify_key_index_list(
                &signed,
                primary.public_key.public_key_bytes().try_into().unwrap()
            )
            .is_some()
        );
        let wrapped = waproto::codec::adv_signed_key_index_list_decode(&signed).unwrap();
        let details =
            waproto::codec::adv_key_index_list_decode(wrapped.details.as_deref().unwrap()).unwrap();
        assert_eq!(details.current_index, Some(4));
        assert_eq!(details.valid_indexes, vec![0]);
    }

    #[test]
    fn own_account_identity_requires_registered_primary_key() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(111);
        let primary = KeyPair::generate(&mut rng);
        let state = DeviceState {
            identity_key: primary,
            noise_key: KeyPair::generate(&mut rng),
            adv_secret_key: [3; 32],
        };
        let own: &[u8; 32] = state
            .identity_key
            .public_key
            .public_key_bytes()
            .try_into()
            .unwrap();
        let issued = build_signed_companion_identity(
            &state.identity_key,
            own,
            &state.adv_secret_key,
            &IdentityParameters {
                raw_id: 2,
                timestamp_seconds: 9,
                key_index: 1,
                valid_indexes: &[0, 1],
                default_encoding: AdvDefaultEncoding::Omit,
            },
            &mut rng,
        )
        .unwrap();
        let (proof, _) = PairUtils::do_pair_crypto(&state, &issued.device_identity).unwrap();
        let jid = wacore_binary::Jid::from_str("123@s.whatsapp.net").unwrap();
        validate_primary_account_identity(&proof, own, &jid).unwrap();
        let other = KeyPair::generate(&mut rng);
        assert!(
            validate_primary_account_identity(
                &proof,
                other.public_key.public_key_bytes().try_into().unwrap(),
                &jid
            )
            .is_err()
        );
    }
}

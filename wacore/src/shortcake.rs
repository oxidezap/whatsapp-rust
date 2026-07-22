//! SHORTCAKE_PASSKEY companion-linking — platform-independent crypto + protobuf.
//!
//! WhatsApp's 2026 passkey/WebAuthn linking gate adds a third `PairingType`
//! (`QR_CODE`, `ALT_DEVICE_LINKING`, `SHORTCAKE_PASSKEY`). When the account has
//! the server flag `shortcake_companion_prologue__passkeys__enabled`, linking a
//! companion requires a WebAuthn assertion (the SOLE unforgeable step) followed
//! by an ephemeral-identity handshake that ends in an AES-256-GCM-encrypted
//! `PairingRequest` carrying the newly-rotated ADV secret.
//!
//! This module is the deterministic, offline-testable foundation: every function
//! here is pure crypto / protobuf encoding. The single non-reproducible step —
//! the WebAuthn assertion — is abstracted behind the host's `PasskeyAuthenticator`
//! (see the main crate's `passkey` module); it is NOT in this file.
//!
//! All constants/labels are reverse-engineered verbatim from WhatsApp Web
//! (modules `WAWebShortcakeLinking*`). Key facts the
//! RE pinned down (and which are easy to get wrong):
//! - companion ephemeral pubkey is RAW 32 bytes (no 0x05 Signal prefix).
//! - commitment = SHA256(companionEphemeralIdentityBytes ‖ companionNonce).
//! - verification code = SHA256(companionNonce ‖ primaryPublicKey); then
//!   `out[i] = primaryNonce[i] ^ digest[i]` for i in 0..5; Crockford-base32 → 8 chars.
//! - encryption key = HKDF-SHA256(IKM = X25519(companionPriv, primaryPub),
//!   **salt = "Companion Pairing {deviceTypeNumeric} with ref {ref}"**,
//!   **info = "Pairing Information Encryption Key"**, len 32). The human-readable
//!   string is the SALT, not the info; deviceType is the numeric enum (CHROME=1).
//! - handoff key = HKDF-SHA256(IKM = priorAdvSecret, salt = none, info =
//!   "shortcake-passkey-handoff-v1", len 32); proof = HMAC-SHA256(key, prologuePayload).
//!   The handoff only proves ADV-secret continuity for a re-link (suppresses the
//!   verification-code UX); it does NOT replace the WebAuthn assertion.

use crate::libsignal::crypto::aes_256_gcm_encrypt;
use crate::libsignal::protocol::{CurveError, KeyPair, PublicKey};
use crate::pair_code::PairCodeUtils;
use buffa::Enumeration;
#[cfg(test)]
use hkdf::Hkdf;
use hmac::{Hmac, KeyInit as _, Mac};
use rand::RngExt;
use sha2::{Digest, Sha256};
use waproto::whatsapp as wa;

/// HKDF `info` for the pairing-handoff HMAC key (RE: "shortcake-passkey-handoff-v1").
const HANDOFF_INFO: &[u8] = b"shortcake-passkey-handoff-v1";
/// HKDF `info` for the pairing-request encryption key (RE: "Pairing Information Encryption Key").
const ENC_KEY_INFO: &[u8] = b"Pairing Information Encryption Key";
/// First N bytes of the verification-code reveal (RE: const s=5 → 8 Crockford chars).
const VERIFICATION_CODE_BYTES: usize = 5;

#[derive(Debug, thiserror::Error)]
pub enum ShortcakeError {
    #[error("invalid primary public key: {0}")]
    InvalidPrimaryKey(CurveError),
    #[error("X25519 agreement failed: {0}")]
    KeyAgreement(CurveError),
    #[error("HKDF expand failed for {0}")]
    Hkdf(&'static str),
    #[error("AES-256-GCM encryption failed")]
    Aead,
    #[error("failed to decode {0} protobuf")]
    Decode(&'static str),
    #[error("unexpected length: {what} expected {expected} got {got}")]
    Length {
        what: &'static str,
        expected: usize,
        got: usize,
    },
}

/// Parsed `<primary_ephemeral_identity>` from the server's continuation
/// notification: the primary's ephemeral X25519 pubkey + nonce, both fixed 32 B.
pub struct PrimaryEphemeralIdentity {
    pub public_key: [u8; 32],
    pub nonce: [u8; 32],
}

/// Output of [`ShortcakeUtils::encrypt_pairing_request`].
pub struct EncryptedPairing {
    /// AES-256-GCM ciphertext ‖ 16-byte tag (matches WebCrypto's single buffer).
    pub encrypted_payload: Vec<u8>,
    /// 12-byte random GCM IV.
    pub iv: [u8; 12],
}

/// Platform-independent SHORTCAKE_PASSKEY crypto + protobuf builders.
pub struct ShortcakeUtils;

impl ShortcakeUtils {
    /// Encode the companion ephemeral identity protobuf.
    /// `public_key` is the RAW 32-byte X25519 pubkey (no 0x05 prefix).
    /// Typed `device_type` keeps the wire value and the key-derivation salt in
    /// lockstep by construction (the salt embeds the same enum number).
    pub fn build_companion_ephemeral_identity(
        public_key: &[u8; 32],
        device_type: wa::device_props::PlatformType,
        ref_str: &str,
    ) -> Vec<u8> {
        waproto::codec::companion_ephemeral_identity_to_vec(&wa::CompanionEphemeralIdentity {
            public_key: Some(public_key.to_vec()),
            device_type: Some(device_type),
            r#ref: Some(ref_str.to_string()),
        })
    }

    /// commitment = SHA256(companionEphemeralIdentityBytes ‖ companionNonce).
    /// The identity is a variable-length protobuf blob; the nonce is fixed 32 B.
    pub fn commitment_hash(
        companion_ephemeral_identity: &[u8],
        companion_nonce: &[u8; 32],
    ) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(companion_ephemeral_identity);
        h.update(companion_nonce);
        h.finalize().into()
    }

    /// Encode the prologue payload protobuf (companion identity + commitment{hash}).
    pub fn build_prologue_payload(
        companion_ephemeral_identity: &[u8],
        commitment_hash: &[u8; 32],
    ) -> Vec<u8> {
        waproto::codec::prologue_payload_to_vec(&wa::ProloguePayload {
            companion_ephemeral_identity: Some(companion_ephemeral_identity.to_vec()),
            commitment: buffa::MessageField::some(wa::CompanionCommitment {
                hash: Some(commitment_hash.to_vec()),
            }),
        })
    }

    /// Decode + length-validate the `PrimaryEphemeralIdentity` protobuf from the
    /// `<primary_ephemeral_identity>` continuation node. Both fields must be
    /// exactly 32 bytes; a wrong length is rejected here rather than producing a
    /// bogus shared secret / verification code downstream.
    pub fn parse_primary_ephemeral_identity(
        bytes: &[u8],
    ) -> Result<PrimaryEphemeralIdentity, ShortcakeError> {
        let parsed = waproto::codec::primary_ephemeral_identity_decode(bytes)
            .map_err(|_| ShortcakeError::Decode("primary_ephemeral_identity"))?;
        let pk = parsed.public_key.unwrap_or_default();
        let nc = parsed.nonce.unwrap_or_default();
        let public_key: [u8; 32] =
            pk.as_slice()
                .try_into()
                .map_err(|_| ShortcakeError::Length {
                    what: "primary_public_key",
                    expected: 32,
                    got: pk.len(),
                })?;
        let nonce: [u8; 32] = nc
            .as_slice()
            .try_into()
            .map_err(|_| ShortcakeError::Length {
                what: "primary_nonce",
                expected: 32,
                got: nc.len(),
            })?;
        Ok(PrimaryEphemeralIdentity { public_key, nonce })
    }

    /// Derive the 8-char verification code shown to the user (and the phone).
    /// `h = SHA256(companionNonce ‖ primaryPublicKey)`; `out[i] = primaryNonce[i] ^ h[i]`
    /// for the first 5 bytes; Crockford base32 → 8 chars.
    pub fn derive_verification_code(
        companion_nonce: &[u8; 32],
        primary_public_key: &[u8; 32],
        primary_nonce: &[u8; 32],
    ) -> String {
        let mut h = Sha256::new();
        h.update(companion_nonce);
        h.update(primary_public_key);
        let digest = h.finalize();
        let mut out = [0u8; VERIFICATION_CODE_BYTES];
        for (i, slot) in out.iter_mut().enumerate() {
            *slot = primary_nonce[i] ^ digest[i];
        }
        PairCodeUtils::encode_crockford(&out)
    }

    /// Derive the AES-256 pairing-request encryption key from the shared secret
    /// (deterministic core, unit-testable). The salt embeds the numeric value of
    /// `device_type`, matching what the identity protobuf carried on the wire.
    pub fn derive_encryption_key_from_shared_secret(
        shared_secret: &[u8; 32],
        device_type: wa::device_props::PlatformType,
        ref_str: &str,
    ) -> Result<[u8; 32], ShortcakeError> {
        let salt = format!(
            "Companion Pairing {} with ref {ref_str}",
            device_type.to_i32()
        );
        let mut key = [0u8; 32];
        crate::crypto::hkdf_sha256_into(
            shared_secret,
            Some(salt.as_bytes()),
            ENC_KEY_INFO,
            &mut key,
        )
        .map_err(|_| ShortcakeError::Hkdf("encryption_key"))?;
        Ok(key)
    }

    /// Full encryption-key derivation: X25519(companionPriv, primaryPub) → HKDF.
    pub fn derive_encryption_key(
        companion_keypair: &KeyPair,
        primary_public_key: &[u8; 32],
        device_type: wa::device_props::PlatformType,
        ref_str: &str,
    ) -> Result<[u8; 32], ShortcakeError> {
        let primary = PublicKey::from_djb_public_key_bytes(primary_public_key)
            .map_err(ShortcakeError::InvalidPrimaryKey)?;
        let shared = companion_keypair
            .private_key
            .calculate_agreement(&primary)
            .map_err(ShortcakeError::KeyAgreement)?;
        Self::derive_encryption_key_from_shared_secret(&shared, device_type, ref_str)
    }

    /// Encode the inner `PairingRequest` plaintext (companion static + identity
    /// pubkeys + the NEWLY-ROTATED ADV secret) — this is what gets encrypted.
    pub fn build_pairing_request(
        companion_public_key: &[u8; 32],
        companion_identity_key: &[u8; 32],
        adv_secret: &[u8; 32],
    ) -> Vec<u8> {
        waproto::codec::pairing_request_to_vec(&wa::PairingRequest {
            companion_public_key: Some(companion_public_key.to_vec()),
            companion_identity_key: Some(companion_identity_key.to_vec()),
            adv_secret: Some(adv_secret.to_vec()),
        })
    }

    /// AES-256-GCM encrypt the pairing-request plaintext with a fresh 12-byte IV,
    /// no AAD. Output is ciphertext‖tag in one buffer (matches WebCrypto).
    pub fn encrypt_pairing_request(
        plaintext: &[u8],
        key: &[u8; 32],
    ) -> Result<EncryptedPairing, ShortcakeError> {
        let mut iv = [0u8; 12];
        rand::make_rng::<rand::rngs::StdRng>().fill(&mut iv);
        let mut encrypted_payload = Vec::with_capacity(plaintext.len() + 16);
        aes_256_gcm_encrypt(key, &iv, b"", plaintext, &mut encrypted_payload)
            .map_err(|_| ShortcakeError::Aead)?;
        Ok(EncryptedPairing {
            encrypted_payload,
            iv,
        })
    }

    /// Encode the `EncryptedPairingRequest` protobuf sent in the final IQ.
    pub fn build_encrypted_pairing_request(enc: &EncryptedPairing) -> Vec<u8> {
        waproto::codec::encrypted_pairing_request_to_vec(&wa::EncryptedPairingRequest {
            encrypted_payload: Some(enc.encrypted_payload.clone()),
            iv: Some(enc.iv.to_vec()),
        })
    }

    /// Derive the pairing-handoff HMAC key from a PRIOR session's 32-byte ADV
    /// secret: HKDF-SHA256(IKM = priorAdvSecret, salt = none, info = handoff label).
    pub fn derive_pairing_handoff_hmac_key(
        prior_adv_secret: &[u8; 32],
    ) -> Result<[u8; 32], ShortcakeError> {
        let mut key = [0u8; 32];
        crate::crypto::hkdf_sha256_into(prior_adv_secret, None, HANDOFF_INFO, &mut key)
            .map_err(|_| ShortcakeError::Hkdf("handoff_key"))?;
        Ok(key)
    }

    /// Compute the pairing-handoff proof = HMAC-SHA256(handoffKey, prologuePayload).
    /// Proves continuity from a prior linked session (re-link UX skip); OPTIONAL.
    pub fn compute_pairing_handoff_proof(
        handoff_key: &[u8; 32],
        prologue_payload: &[u8],
    ) -> [u8; 32] {
        let mut mac =
            Hmac::<Sha256>::new_from_slice(handoff_key).expect("HMAC accepts any key length");
        mac.update(prologue_payload);
        mac.finalize().into_bytes().into()
    }

    /// Generate the companion ephemeral X25519 keypair for a new SHORTCAKE attempt.
    pub fn generate_companion_ephemeral_keypair() -> KeyPair {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        KeyPair::generate(&mut rng)
    }

    /// Generate a fresh 32-byte companion nonce.
    pub fn generate_companion_nonce() -> [u8; 32] {
        let mut nonce = [0u8; 32];
        rand::make_rng::<rand::rngs::StdRng>().fill(&mut nonce);
        nonce
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;
    use buffa::Message;

    // Deterministic vectors guard the bug-prone concat/XOR/label details the RE flagged.

    #[test]
    fn commitment_is_sha256_of_identity_then_nonce() {
        let identity = b"identity-bytes";
        let nonce = [7u8; 32];
        let got = ShortcakeUtils::commitment_hash(identity, &nonce);
        // independent re-derivation, asserting the exact concat ORDER
        let mut h = Sha256::new();
        h.update(identity);
        h.update(nonce);
        let want: [u8; 32] = h.finalize().into();
        assert_eq!(got, want);
        // order matters: nonce-then-identity must differ
        let mut h2 = Sha256::new();
        h2.update(nonce);
        h2.update(identity);
        let wrong: [u8; 32] = h2.finalize().into();
        assert_ne!(got, wrong);
    }

    #[test]
    fn verification_code_format_and_xor_order() {
        let companion_nonce = [1u8; 32];
        let primary_pub = [2u8; 32];
        let primary_nonce = [3u8; 32];
        let code = ShortcakeUtils::derive_verification_code(
            &companion_nonce,
            &primary_pub,
            &primary_nonce,
        );
        // exactly 8 Crockford chars
        assert_eq!(code.len(), 8);
        const CROCKFORD: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTVWXYZ";
        assert!(code.bytes().all(|b| CROCKFORD.contains(&b)));
        // re-derive independently: SHA256(companionNonce ‖ primaryPub), XOR first 5 of primaryNonce
        let mut h = Sha256::new();
        h.update(companion_nonce);
        h.update(primary_pub);
        let d = h.finalize();
        let mut out = [0u8; 5];
        for i in 0..5 {
            out[i] = primary_nonce[i] ^ d[i];
        }
        assert_eq!(code, PairCodeUtils::encode_crockford(&out));
        // deterministic
        assert_eq!(
            code,
            ShortcakeUtils::derive_verification_code(
                &companion_nonce,
                &primary_pub,
                &primary_nonce
            )
        );
    }

    // The fixed-size crypto inputs are `&[u8; 32]` parameters, so a wrong length
    // is a compile error rather than a runtime check. Length validation only
    // survives at the wire boundary: `parse_primary_ephemeral_identity` below.

    #[test]
    fn parse_primary_ephemeral_identity_roundtrip_and_length_validation() {
        let proto = wa::PrimaryEphemeralIdentity {
            public_key: Some(vec![0xAB; 32]),
            nonce: Some(vec![0xCD; 32]),
        }
        .encode_to_vec();
        let parsed = ShortcakeUtils::parse_primary_ephemeral_identity(&proto).unwrap();
        assert_eq!(parsed.public_key, [0xAB; 32]);
        assert_eq!(parsed.nonce, [0xCD; 32]);

        // wrong-length pubkey is rejected with a Length error
        let bad_pk = wa::PrimaryEphemeralIdentity {
            public_key: Some(vec![0xAB; 31]),
            nonce: Some(vec![0xCD; 32]),
        }
        .encode_to_vec();
        assert!(matches!(
            ShortcakeUtils::parse_primary_ephemeral_identity(&bad_pk),
            Err(ShortcakeError::Length {
                what: "primary_public_key",
                ..
            })
        ));

        // wrong-length nonce is rejected
        let bad_nonce = wa::PrimaryEphemeralIdentity {
            public_key: Some(vec![0xAB; 32]),
            nonce: Some(vec![0xCD; 1]),
        }
        .encode_to_vec();
        assert!(matches!(
            ShortcakeUtils::parse_primary_ephemeral_identity(&bad_nonce),
            Err(ShortcakeError::Length {
                what: "primary_nonce",
                ..
            })
        ));

        // garbage that isn't a valid protobuf for this message: an absent field
        // decodes to empty (length 0), which is also a Length error, not a panic.
        assert!(matches!(
            ShortcakeUtils::parse_primary_ephemeral_identity(&[]),
            Err(ShortcakeError::Length { got: 0, .. })
        ));
    }

    #[test]
    fn encryption_key_uses_string_as_salt_not_info() {
        let ikm = [9u8; 32];
        let key = ShortcakeUtils::derive_encryption_key_from_shared_secret(
            &ikm,
            wa::device_props::PlatformType::CHROME,
            "REF123",
        )
        .unwrap();
        // independent re-derivation with the documented salt/info placement
        let salt = "Companion Pairing 1 with ref REF123";
        let hk = Hkdf::<Sha256>::new(Some(salt.as_bytes()), &ikm);
        let mut want = [0u8; 32];
        hk.expand(b"Pairing Information Encryption Key", &mut want)
            .unwrap();
        assert_eq!(key, want);
        // swapping salt<->info (a plausible bug) must produce a different key
        let hk2 = Hkdf::<Sha256>::new(Some(b"Pairing Information Encryption Key"), &ikm);
        let mut wrong = [0u8; 32];
        hk2.expand(salt.as_bytes(), &mut wrong).unwrap();
        assert_ne!(key, wrong);
        // device_type and ref are bound into the key
        assert_ne!(
            key,
            ShortcakeUtils::derive_encryption_key_from_shared_secret(
                &ikm,
                wa::device_props::PlatformType::FIREFOX,
                "REF123"
            )
            .unwrap()
        );
        assert_ne!(
            key,
            ShortcakeUtils::derive_encryption_key_from_shared_secret(
                &ikm,
                wa::device_props::PlatformType::CHROME,
                "OTHER"
            )
            .unwrap()
        );
    }

    #[test]
    fn handoff_key_and_proof() {
        let prior = [5u8; 32];
        let k = ShortcakeUtils::derive_pairing_handoff_hmac_key(&prior).unwrap();
        // independent HKDF (salt none, info label)
        let hk = Hkdf::<Sha256>::new(None, &prior);
        let mut want = [0u8; 32];
        hk.expand(b"shortcake-passkey-handoff-v1", &mut want)
            .unwrap();
        assert_eq!(k, want);
        let proof = ShortcakeUtils::compute_pairing_handoff_proof(&k, b"prologue");
        let mut mac = Hmac::<Sha256>::new_from_slice(&k).unwrap();
        mac.update(b"prologue");
        let want_proof: [u8; 32] = mac.finalize().into_bytes().into();
        assert_eq!(proof, want_proof);
    }

    #[test]
    fn protobufs_roundtrip_with_expected_fields() {
        let id = ShortcakeUtils::build_companion_ephemeral_identity(
            &[0xAA; 32],
            wa::device_props::PlatformType::CHROME,
            "theref",
        );
        let decoded = wa::CompanionEphemeralIdentity::decode_from_slice(id.as_slice()).unwrap();
        assert_eq!(decoded.public_key.as_deref(), Some(&[0xAA; 32][..]));
        assert_eq!(
            decoded.device_type,
            Some(wa::device_props::PlatformType::CHROME)
        );
        assert_eq!(decoded.r#ref.as_deref(), Some("theref"));

        let prologue = ShortcakeUtils::build_prologue_payload(&id, &[0xBB; 32]);
        let dp = wa::ProloguePayload::decode_from_slice(prologue.as_slice()).unwrap();
        assert_eq!(
            dp.companion_ephemeral_identity.as_deref(),
            Some(id.as_slice())
        );
        assert_eq!(
            dp.commitment.into_option().and_then(|c| c.hash).as_deref(),
            Some(&[0xBB; 32][..])
        );

        let pr = ShortcakeUtils::build_pairing_request(&[1; 32], &[2; 32], &[3; 32]);
        let dpr = wa::PairingRequest::decode_from_slice(pr.as_slice()).unwrap();
        assert_eq!(dpr.companion_public_key.as_deref(), Some(&[1u8; 32][..]));
        assert_eq!(dpr.companion_identity_key.as_deref(), Some(&[2u8; 32][..]));
        assert_eq!(dpr.adv_secret.as_deref(), Some(&[3u8; 32][..]));
    }

    #[test]
    fn encrypt_pairing_request_shape() {
        let key = [4u8; 32];
        let enc = ShortcakeUtils::encrypt_pairing_request(b"hello pairing", &key).unwrap();
        assert_eq!(enc.iv.len(), 12);
        // ciphertext + 16-byte GCM tag
        assert_eq!(enc.encrypted_payload.len(), b"hello pairing".len() + 16);
        let wire = ShortcakeUtils::build_encrypted_pairing_request(&enc);
        let d = wa::EncryptedPairingRequest::decode_from_slice(wire.as_slice()).unwrap();
        assert_eq!(d.iv.as_deref(), Some(&enc.iv[..]));
        assert_eq!(d.encrypted_payload, Some(enc.encrypted_payload));
    }

    // Runs both the companion and a simulated primary through the primitives: the
    // verification codes match, the X25519+HKDF keys agree, the primary decrypts the
    // companion's PairingRequest, and the handoff proof verifies.
    #[test]
    fn full_handshake_interops_with_a_simulated_primary() {
        use crate::libsignal::crypto::aes_256_gcm_decrypt;

        let device_type = wa::device_props::PlatformType::CHROME;
        let pairing_ref = "REF-XYZ";
        let prior_adv_secret = [0x11u8; 32]; // a prior linked session's secret
        let new_adv_secret = [0x22u8; 32]; // rotated for this link

        // companion: ephemeral identity + commitment + prologue + handoff proof
        let companion_kp = ShortcakeUtils::generate_companion_ephemeral_keypair();
        let companion_nonce = ShortcakeUtils::generate_companion_nonce();
        let companion_pub: [u8; 32] = companion_kp
            .public_key
            .public_key_bytes()
            .try_into()
            .unwrap();
        let identity = ShortcakeUtils::build_companion_ephemeral_identity(
            &companion_pub,
            device_type,
            pairing_ref,
        );
        let commitment = ShortcakeUtils::commitment_hash(&identity, &companion_nonce);
        let prologue = ShortcakeUtils::build_prologue_payload(&identity, &commitment);
        let companion_handoff =
            ShortcakeUtils::derive_pairing_handoff_hmac_key(&prior_adv_secret).unwrap();
        let proof = ShortcakeUtils::compute_pairing_handoff_proof(&companion_handoff, &prologue);

        // primary: its own ephemeral identity, sent back as <primary_ephemeral_identity>
        let primary_kp = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
        let primary_pub: [u8; 32] = primary_kp.public_key.public_key_bytes().try_into().unwrap();
        let primary_nonce = [0x33u8; 32];
        let primary_wire = wa::PrimaryEphemeralIdentity {
            public_key: Some(primary_pub.to_vec()),
            nonce: Some(primary_nonce.to_vec()),
        }
        .encode_to_vec();
        let parsed = ShortcakeUtils::parse_primary_ephemeral_identity(&primary_wire).unwrap();

        // the primary verifies the handoff proof against the shared prior secret
        let primary_handoff =
            ShortcakeUtils::derive_pairing_handoff_hmac_key(&prior_adv_secret).unwrap();
        assert_eq!(
            proof,
            ShortcakeUtils::compute_pairing_handoff_proof(&primary_handoff, &prologue),
            "handoff proof must verify on the primary"
        );

        // both sides derive the SAME verification code from the same inputs
        let companion_code = ShortcakeUtils::derive_verification_code(
            &companion_nonce,
            &parsed.public_key,
            &parsed.nonce,
        );
        let primary_code = ShortcakeUtils::derive_verification_code(
            &companion_nonce,
            &primary_pub,
            &primary_nonce,
        );
        assert_eq!(companion_code, primary_code);

        // X25519 is symmetric, so both sides derive the same AES key
        let companion_key = ShortcakeUtils::derive_encryption_key(
            &companion_kp,
            &parsed.public_key,
            device_type,
            pairing_ref,
        )
        .unwrap();
        let primary_shared = primary_kp
            .private_key
            .calculate_agreement(&PublicKey::from_djb_public_key_bytes(&companion_pub).unwrap())
            .unwrap();
        let primary_key = ShortcakeUtils::derive_encryption_key_from_shared_secret(
            &primary_shared,
            device_type,
            pairing_ref,
        )
        .unwrap();
        assert_eq!(companion_key, primary_key, "X25519+HKDF keys must agree");

        // companion encrypts the PairingRequest; the primary decrypts it and reads
        // the newly-rotated ADV secret out
        let request =
            ShortcakeUtils::build_pairing_request(&[0xAA; 32], &[0xBB; 32], &new_adv_secret);
        let enc = ShortcakeUtils::encrypt_pairing_request(&request, &companion_key).unwrap();
        let wire = ShortcakeUtils::build_encrypted_pairing_request(&enc);
        let decoded = wa::EncryptedPairingRequest::decode_from_slice(wire.as_slice()).unwrap();
        let iv: [u8; 12] = decoded.iv.unwrap().as_slice().try_into().unwrap();

        let mut plaintext = Vec::new();
        aes_256_gcm_decrypt(
            &primary_key,
            &iv,
            b"",
            &decoded.encrypted_payload.unwrap(),
            &mut plaintext,
        )
        .unwrap();
        let recovered = wa::PairingRequest::decode_from_slice(plaintext.as_slice()).unwrap();
        assert_eq!(recovered.adv_secret.as_deref(), Some(&new_adv_secret[..]));
        assert_eq!(
            recovered.companion_public_key.as_deref(),
            Some(&[0xAA; 32][..])
        );
        assert_eq!(
            recovered.companion_identity_key.as_deref(),
            Some(&[0xBB; 32][..])
        );
    }
}

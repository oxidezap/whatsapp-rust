//! Poll vote encryption/decryption.
//!
//! Thin wrapper over [`secret_enc_addon`] specialised for the
//! `PollVoteMessage` proto and the `"Poll Vote"` use-case.

use anyhow::{Result, anyhow};
use sha2::{Digest, Sha256};

use crate::secret_enc_addon::{
    AddonContext, ModificationType, build_aad, decrypt_addon, encrypt_addon,
};

const GCM_IV_SIZE: usize = 12;
const GCM_TAG_SIZE: usize = 16;

fn poll_vote_addon_ctx<'a>(
    stanza_id: &'a str,
    poll_creator_jid: &'a str,
    voter_jid: &'a str,
) -> AddonContext<'a> {
    AddonContext {
        stanza_id,
        parent_msg_original_sender: poll_creator_jid,
        modification_sender: voter_jid,
        modification_type: ModificationType::PollVote,
    }
}

/// Votes reference options by SHA-256 hash, not by name.
pub fn compute_option_hash(option_name: &str) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(option_name.as_bytes());
    hasher.finalize().into()
}

/// HKDF-SHA256: info = stanzaId || pollCreator || voter || "Poll Vote", no salt.
///
/// Kept as a public API for backwards compatibility; new code should call
/// [`secret_enc_addon::derive_use_case_secret`] with `ModificationType::PollVote`.
pub fn derive_vote_encryption_key(
    message_secret: &[u8],
    stanza_id: &str,
    poll_creator_jid: &str,
    voter_jid: &str,
) -> Result<[u8; 32]> {
    crate::secret_enc_addon::derive_use_case_secret(
        message_secret,
        &poll_vote_addon_ctx(stanza_id, poll_creator_jid, voter_jid),
    )
}

/// Encrypt a poll vote with a pre-derived 32-byte key, symmetric to
/// [`decrypt_poll_vote`]. Returns `(payload_with_tag, iv)`.
///
/// Kept for callers that built their own key via [`derive_vote_encryption_key`].
/// New code should prefer [`encrypt_poll_vote_with_secret`], which derives
/// the key in a single step from the parent poll's `messageSecret`.
pub fn encrypt_poll_vote(
    selected_option_hashes: &[Vec<u8>],
    encryption_key: &[u8; 32],
    stanza_id: &str,
    voter_jid: &str,
) -> Result<(Vec<u8>, [u8; GCM_IV_SIZE])> {
    use crate::libsignal::crypto::aes_256_gcm_encrypt;
    use prost::Message;
    use rand::Rng;

    let vote_msg = waproto::whatsapp::message::PollVoteMessage {
        selected_options: selected_option_hashes.to_vec(),
    };
    let mut plaintext = Vec::new();
    vote_msg.encode(&mut plaintext)?;

    let mut iv = [0u8; GCM_IV_SIZE];
    rand::make_rng::<rand::rngs::StdRng>().fill_bytes(&mut iv);

    // poll_creator_jid is not part of the AAD; supply an empty placeholder.
    let aad = build_aad(&poll_vote_addon_ctx(stanza_id, "", voter_jid));

    let mut payload = Vec::with_capacity(plaintext.len() + GCM_TAG_SIZE);
    aes_256_gcm_encrypt(encryption_key, &iv, &aad, &plaintext, &mut payload)
        .map_err(|e| anyhow!("AES-GCM encrypt failed: {e}"))?;

    Ok((payload, iv))
}

/// Encrypt a poll vote given the parent poll's `messageSecret`. Returns
/// `(payload_with_tag, iv)`.
pub fn encrypt_poll_vote_with_secret(
    selected_option_hashes: &[Vec<u8>],
    message_secret: &[u8],
    stanza_id: &str,
    poll_creator_jid: &str,
    voter_jid: &str,
) -> Result<(Vec<u8>, [u8; GCM_IV_SIZE])> {
    use prost::Message;

    let vote_msg = waproto::whatsapp::message::PollVoteMessage {
        selected_options: selected_option_hashes.to_vec(),
    };
    let mut plaintext = Vec::new();
    vote_msg.encode(&mut plaintext)?;

    encrypt_addon(
        &plaintext,
        message_secret,
        &poll_vote_addon_ctx(stanza_id, poll_creator_jid, voter_jid),
    )
}

/// Returns the selected option hashes (each 32 bytes).
///
/// Kept for backwards compatibility with callers that pre-derived the key.
/// New code should call [`decrypt_poll_vote_with_secret`].
pub fn decrypt_poll_vote(
    enc_payload: &[u8],
    iv: &[u8],
    encryption_key: &[u8; 32],
    stanza_id: &str,
    voter_jid: &str,
) -> Result<Vec<Vec<u8>>> {
    use crate::libsignal::crypto::aes_256_gcm_decrypt;
    use prost::Message as _;

    let nonce: &[u8; GCM_IV_SIZE] = iv
        .try_into()
        .map_err(|_| anyhow!("Invalid IV size: expected {GCM_IV_SIZE}, got {}", iv.len()))?;
    if enc_payload.len() < GCM_TAG_SIZE {
        return Err(anyhow!(
            "Encrypted payload too short: need at least {GCM_TAG_SIZE} bytes for tag, got {}",
            enc_payload.len()
        ));
    }

    // poll_creator_jid is not part of the AAD; supply an empty placeholder.
    let aad = build_aad(&poll_vote_addon_ctx(stanza_id, "", voter_jid));

    let mut plaintext = Vec::with_capacity(enc_payload.len().saturating_sub(GCM_TAG_SIZE));
    aes_256_gcm_decrypt(encryption_key, nonce, &aad, enc_payload, &mut plaintext)
        .map_err(|_| anyhow!("Poll vote GCM tag verification failed"))?;

    let vote_msg = waproto::whatsapp::message::PollVoteMessage::decode(&plaintext[..])?;
    Ok(vote_msg.selected_options)
}

/// Decrypt a poll vote given the poll's `messageSecret` directly. Preferred
/// over the legacy two-step path that splits derive+decrypt.
pub fn decrypt_poll_vote_with_secret(
    enc_payload: &[u8],
    iv: &[u8],
    message_secret: &[u8],
    stanza_id: &str,
    poll_creator_jid: &str,
    voter_jid: &str,
) -> Result<Vec<Vec<u8>>> {
    use prost::Message as _;

    let plaintext = decrypt_addon(
        enc_payload,
        iv,
        message_secret,
        &poll_vote_addon_ctx(stanza_id, poll_creator_jid, voter_jid),
    )?;
    let vote_msg = waproto::whatsapp::message::PollVoteMessage::decode(&plaintext[..])?;
    Ok(vote_msg.selected_options)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn option_hash_deterministic() {
        let h1 = compute_option_hash("Option A");
        let h2 = compute_option_hash("Option A");
        let h3 = compute_option_hash("Option B");
        assert_eq!(h1, h2);
        assert_ne!(h1, h3);
        assert_eq!(h1.len(), 32);
    }

    #[test]
    fn vote_encrypt_decrypt_roundtrip() {
        let secret = [0xCDu8; 32];
        let stanza_id = "3EB0ABCD1234";
        let creator = "creator@s.whatsapp.net";
        let voter = "voter@s.whatsapp.net";

        let hashes = vec![
            compute_option_hash("Yes").to_vec(),
            compute_option_hash("No").to_vec(),
        ];

        let (enc, iv) =
            encrypt_poll_vote_with_secret(&hashes, &secret, stanza_id, creator, voter).unwrap();
        let out =
            decrypt_poll_vote_with_secret(&enc, &iv, &secret, stanza_id, creator, voter).unwrap();
        assert_eq!(out, hashes);
    }

    #[test]
    fn legacy_decrypt_path_still_works() {
        let secret = [0xCDu8; 32];
        let stanza_id = "3EB0ABCD1234";
        let creator = "creator@s.whatsapp.net";
        let voter = "voter@s.whatsapp.net";

        let hashes = vec![compute_option_hash("Yes").to_vec()];
        let (enc, iv) =
            encrypt_poll_vote_with_secret(&hashes, &secret, stanza_id, creator, voter).unwrap();

        let key = derive_vote_encryption_key(&secret, stanza_id, creator, voter).unwrap();
        let out = decrypt_poll_vote(&enc, &iv, &key, stanza_id, voter).unwrap();
        assert_eq!(out, hashes);
    }

    #[test]
    fn wrong_voter_fails() {
        let secret = [0xEFu8; 32];
        let (enc, iv) = encrypt_poll_vote_with_secret(
            &[compute_option_hash("Yes").to_vec()],
            &secret,
            "id",
            "c@s.whatsapp.net",
            "v@s.whatsapp.net",
        )
        .unwrap();

        assert!(
            decrypt_poll_vote_with_secret(
                &enc,
                &iv,
                &secret,
                "id",
                "c@s.whatsapp.net",
                "wrong@s.whatsapp.net"
            )
            .is_err()
        );
    }

    #[test]
    fn empty_vote_roundtrip() {
        let secret = [0xEFu8; 32];
        let (enc, iv) = encrypt_poll_vote_with_secret(
            &[],
            &secret,
            "id",
            "c@s.whatsapp.net",
            "v@s.whatsapp.net",
        )
        .unwrap();
        let out = decrypt_poll_vote_with_secret(
            &enc,
            &iv,
            &secret,
            "id",
            "c@s.whatsapp.net",
            "v@s.whatsapp.net",
        )
        .unwrap();
        assert!(out.is_empty());
    }
}

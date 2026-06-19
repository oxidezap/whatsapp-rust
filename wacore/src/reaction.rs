//! Encrypted reaction (CAG) encryption.
//!
//! Thin wrapper over [`crate::secret_enc_addon`] specialised for the
//! `enc_reaction_message` envelope used in Community Announcement Groups.
//! Mirrors `WAWebReactionEncryptMsgData` / `WAWebReactionsEncryption`: the
//! inner plaintext is a `ReactionMessage` proto carrying ONLY `text` and
//! `sender_timestamp_ms` (the target key travels in the outer envelope), and
//! the HKDF use-case is `"Enc Reaction"` with empty AAD.

use anyhow::{Result, ensure};
use prost::Message;
use waproto::whatsapp::message::ReactionMessage;

use crate::secret_enc_addon::{AddonContext, ModificationType, decrypt_addon, encrypt_addon};

const GCM_IV_SIZE: usize = 12;
const MESSAGE_SECRET_SIZE: usize = 32;

fn reaction_addon_ctx<'a>(
    parent_msg_id: &'a str,
    parent_sender_jid: &'a str,
    reactor_jid: &'a str,
) -> AddonContext<'a> {
    AddonContext {
        stanza_id: parent_msg_id,
        parent_msg_original_sender: parent_sender_jid,
        modification_sender: reactor_jid,
        modification_type: ModificationType::EncReaction,
    }
}

/// Encrypt a reaction given the parent message's `messageSecret`.
/// Returns `(payload_with_tag, iv)`.
///
/// An empty `text` is the reaction-removal form, same as the plaintext path.
pub fn encrypt_reaction_with_secret(
    text: &str,
    sender_timestamp_ms: i64,
    message_secret: &[u8],
    parent_msg_id: &str,
    parent_sender_jid: &str,
    reactor_jid: &str,
) -> Result<(Vec<u8>, [u8; GCM_IV_SIZE])> {
    ensure!(
        message_secret.len() == MESSAGE_SECRET_SIZE,
        "message_secret must be {MESSAGE_SECRET_SIZE} bytes, got {}",
        message_secret.len()
    );
    // WA Web encodes only { text, senderTimestampMs }; the key is envelope-side.
    let inner = ReactionMessage {
        text: Some(text.to_string()),
        sender_timestamp_ms: Some(sender_timestamp_ms),
        ..Default::default()
    };
    let plaintext = inner.encode_to_vec();
    encrypt_addon(
        &plaintext,
        message_secret,
        &reaction_addon_ctx(parent_msg_id, parent_sender_jid, reactor_jid),
    )
}

/// Decrypt an `enc_reaction_message` payload given the parent's `messageSecret`.
///
/// The returned `ReactionMessage` carries no `key`; the caller fills it from
/// the envelope's `target_message_key`.
pub fn decrypt_reaction_with_secret(
    enc_payload: &[u8],
    iv: &[u8],
    message_secret: &[u8],
    parent_msg_id: &str,
    parent_sender_jid: &str,
    reactor_jid: &str,
) -> Result<ReactionMessage> {
    ensure!(
        message_secret.len() == MESSAGE_SECRET_SIZE,
        "message_secret must be {MESSAGE_SECRET_SIZE} bytes, got {}",
        message_secret.len()
    );
    let plaintext = decrypt_addon(
        enc_payload,
        iv,
        message_secret,
        &reaction_addon_ctx(parent_msg_id, parent_sender_jid, reactor_jid),
    )?;
    Ok(ReactionMessage::decode(&plaintext[..])?)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: [u8; 32] = [0x42; 32];
    const PARENT_ID: &str = "3EB0PARENT";
    const AUTHOR: &str = "111111111111111@lid";
    const REACTOR: &str = "222222222222222@lid";

    #[test]
    fn roundtrip_keeps_text_and_timestamp_only() {
        let (enc, iv) = encrypt_reaction_with_secret(
            "\u{1F525}",
            1_700_000_000_123,
            &SECRET,
            PARENT_ID,
            AUTHOR,
            REACTOR,
        )
        .unwrap();
        let out =
            decrypt_reaction_with_secret(&enc, &iv, &SECRET, PARENT_ID, AUTHOR, REACTOR).unwrap();
        assert_eq!(out.text.as_deref(), Some("\u{1F525}"));
        assert_eq!(out.sender_timestamp_ms, Some(1_700_000_000_123));
        assert!(out.key.is_none(), "key must not travel in the plaintext");
        assert!(out.grouping_key.is_none());
    }

    #[test]
    fn empty_text_removal_roundtrips() {
        let (enc, iv) =
            encrypt_reaction_with_secret("", 1, &SECRET, PARENT_ID, AUTHOR, REACTOR).unwrap();
        let out =
            decrypt_reaction_with_secret(&enc, &iv, &SECRET, PARENT_ID, AUTHOR, REACTOR).unwrap();
        assert_eq!(out.text.as_deref(), Some(""));
    }

    #[test]
    fn wrong_reactor_fails_decrypt() {
        // The reactor JID keys the HKDF info, so a mismatched identity must
        // not produce a valid key.
        let (enc, iv) =
            encrypt_reaction_with_secret("x", 1, &SECRET, PARENT_ID, AUTHOR, REACTOR).unwrap();
        assert!(
            decrypt_reaction_with_secret(&enc, &iv, &SECRET, PARENT_ID, AUTHOR, "333333@lid")
                .is_err()
        );
    }

    #[test]
    fn invalid_secret_size_rejected() {
        assert!(
            encrypt_reaction_with_secret("x", 1, &[0u8; 16], PARENT_ID, AUTHOR, REACTOR).is_err()
        );
        assert!(
            decrypt_reaction_with_secret(
                &[0u8; 32], &[0u8; 12], &[0u8; 16], PARENT_ID, AUTHOR, REACTOR
            )
            .is_err()
        );
    }
}

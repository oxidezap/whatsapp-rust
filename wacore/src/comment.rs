//! Encrypted channel comment (CAG) encryption.
//!
//! Thin wrapper over [`crate::secret_enc_addon`] specialised for the
//! `enc_comment_message` envelope (threaded comments under a Community
//! Announcement Group post). Mirrors `WAWebAddonEncryption` (`MessageSpec` +
//! `ENC_COMMENT` use-case, empty AAD): the inner plaintext is a full `Message`
//! proto carrying the comment body; the parent post's key travels in the
//! outer envelope.

use anyhow::{Result, ensure};
use waproto::whatsapp as wa;

use crate::secret_enc_addon::{AddonContext, ModificationType, decrypt_addon, encrypt_addon};

const GCM_IV_SIZE: usize = 12;
const MESSAGE_SECRET_SIZE: usize = 32;

fn comment_addon_ctx<'a>(
    parent_msg_id: &'a str,
    parent_sender_jid: &'a str,
    commenter_jid: &'a str,
) -> AddonContext<'a> {
    AddonContext {
        stanza_id: parent_msg_id,
        parent_msg_original_sender: parent_sender_jid,
        modification_sender: commenter_jid,
        modification_type: ModificationType::EncComment,
    }
}

/// Encrypt a comment body given the parent post's `messageSecret`.
/// Returns `(payload_with_tag, iv)`.
pub fn encrypt_comment_with_secret(
    inner: &wa::Message,
    message_secret: &[u8],
    parent_msg_id: &str,
    parent_sender_jid: &str,
    commenter_jid: &str,
) -> Result<(Vec<u8>, [u8; GCM_IV_SIZE])> {
    ensure!(
        message_secret.len() == MESSAGE_SECRET_SIZE,
        "message_secret must be {MESSAGE_SECRET_SIZE} bytes, got {}",
        message_secret.len()
    );
    let plaintext = waproto::codec::message_to_vec(inner);
    encrypt_addon(
        &plaintext,
        message_secret,
        &comment_addon_ctx(parent_msg_id, parent_sender_jid, commenter_jid),
    )
}

/// Decrypt an `enc_comment_message` payload given the parent's `messageSecret`.
/// Returns the inner comment body `Message`.
pub fn decrypt_comment_with_secret(
    enc_payload: &[u8],
    iv: &[u8],
    message_secret: &[u8],
    parent_msg_id: &str,
    parent_sender_jid: &str,
    commenter_jid: &str,
) -> Result<wa::Message> {
    ensure!(
        message_secret.len() == MESSAGE_SECRET_SIZE,
        "message_secret must be {MESSAGE_SECRET_SIZE} bytes, got {}",
        message_secret.len()
    );
    let plaintext = decrypt_addon(
        enc_payload,
        iv,
        message_secret,
        &comment_addon_ctx(parent_msg_id, parent_sender_jid, commenter_jid),
    )?;
    Ok(waproto::codec::message_decode(&plaintext[..])?)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SECRET: [u8; 32] = [0x24; 32];
    const PARENT_ID: &str = "3EB0POST";
    const AUTHOR: &str = "111111111111111@lid";
    const COMMENTER: &str = "222222222222222@lid";

    fn body(text: &str) -> wa::Message {
        wa::Message {
            extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
                text: Some(text.to_string()),
                ..Default::default()
            })),
            ..Default::default()
        }
    }

    #[test]
    fn roundtrip_extended_text_body() {
        let inner = body("nice post");
        let (enc, iv) =
            encrypt_comment_with_secret(&inner, &SECRET, PARENT_ID, AUTHOR, COMMENTER).unwrap();
        let out =
            decrypt_comment_with_secret(&enc, &iv, &SECRET, PARENT_ID, AUTHOR, COMMENTER).unwrap();
        assert_eq!(
            out.extended_text_message
                .as_ref()
                .and_then(|m| m.text.as_deref()),
            Some("nice post")
        );
    }

    #[test]
    fn use_case_differs_from_reaction() {
        // Same inputs under the reaction use-case must not decrypt a comment:
        // the HKDF info embeds the use-case literal.
        let inner = body("hi");
        let (enc, iv) =
            encrypt_comment_with_secret(&inner, &SECRET, PARENT_ID, AUTHOR, COMMENTER).unwrap();
        assert!(
            crate::reaction::decrypt_reaction_with_secret(
                &enc, &iv, &SECRET, PARENT_ID, AUTHOR, COMMENTER
            )
            .is_err()
        );
    }

    #[test]
    fn invalid_secret_size_rejected() {
        let inner = body("x");
        assert!(
            encrypt_comment_with_secret(&inner, &[0u8; 31], PARENT_ID, AUTHOR, COMMENTER).is_err()
        );
    }
}

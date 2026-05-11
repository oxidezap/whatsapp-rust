//! E2E-encrypted message edit envelope (`secret_encrypted_message` with
//! `secret_enc_type = MESSAGE_EDIT`).
//!
//! Introduced by WhatsApp in 2026: newer clients wrap message edits in an
//! E2EE envelope keyed by the original message's
//! `messageContextInfo.messageSecret`, replacing the older in-clear
//! `protocolMessage.editedMessage` path.
//!
//! Verified against `docs/captured-js/`:
//!
//! - `WA/Use/CaseSecret.js` — use-case literal `"Message Edit"` and the
//!   HKDF info ordering (`stanzaId || parentOrigSender || editor || usecase`).
//! - `WAWeb/Parse/MessageEditEncryptedMessageProto.js` — envelope detection
//!   and IV-length validation (`u.length !== 12`).
//! - `WAWeb/Process/EncryptedMessageEditMsgs.js` — invocation contract.
//! - `WAWeb/Addon/Encryption.js` function `g` — AAD is empty for the
//!   MessageEdit branch (only `PollVote` and `EventResponse` bind stanza/sender
//!   into AAD).
//!
//! The plaintext is a `Message` proto whose `protocolMessage.editedMessage`
//! carries the new content — same shape as the legacy edit path, so callers
//! that already handle `protocolMessage.editedMessage` can reuse their code.

use anyhow::{Result, anyhow};
use prost::Message;

use crate::secret_enc_addon::{AddonContext, ModificationType, decrypt_addon, encrypt_addon};

const IV_SIZE: usize = 12;

/// Inputs for decrypting / encrypting a `MESSAGE_EDIT` envelope.
///
/// JIDs must be passed in the addressing they were received under. The
/// caller is responsible for any LID↔PN normalisation; see
/// [`decrypt_message_edit_with_fallback`].
#[derive(Debug, Clone, Copy)]
pub struct MessageEditContext<'a> {
    /// ID of the *original* (target) message being edited — this is the
    /// stanza id WA Web feeds into the HKDF info buffer.
    pub original_msg_id: &'a str,
    /// JID of the sender of the original message (`participant` for groups,
    /// `remote_jid` for 1:1).
    pub original_sender_jid: &'a str,
    /// JID of the user performing the edit (sender of the envelope).
    pub editor_jid: &'a str,
}

impl<'a> MessageEditContext<'a> {
    fn as_addon_ctx(&self) -> AddonContext<'a> {
        AddonContext {
            stanza_id: self.original_msg_id,
            parent_msg_original_sender: self.original_sender_jid,
            modification_sender: self.editor_jid,
            modification_type: ModificationType::MessageEdit,
        }
    }
}

/// Encrypt an edit payload for testing or for client-side outgoing edits.
///
/// `inner_message` is the full `Message` proto whose `protocolMessage.editedMessage`
/// carries the new content; it gets serialised and encrypted in one shot.
pub fn encrypt_message_edit(
    inner_message: &waproto::whatsapp::Message,
    message_secret: &[u8],
    ctx: &MessageEditContext<'_>,
) -> Result<(Vec<u8>, [u8; IV_SIZE])> {
    let mut plaintext = Vec::new();
    inner_message.encode(&mut plaintext)?;
    encrypt_addon(&plaintext, message_secret, &ctx.as_addon_ctx())
}

/// Decrypt a `secret_encrypted_message` MESSAGE_EDIT envelope.
///
/// Returns the inner `Message` whose `protocolMessage.editedMessage` carries
/// the new content. The caller can re-wrap it in `protocolMessage.editedMessage`
/// for consumer parity with the legacy edit path.
///
/// `iv` must be exactly 12 bytes (matches WA Web's `u.length !== 12` check).
pub fn decrypt_message_edit(
    enc_payload: &[u8],
    iv: &[u8],
    message_secret: &[u8],
    ctx: &MessageEditContext<'_>,
) -> Result<waproto::whatsapp::Message> {
    if iv.len() != IV_SIZE {
        return Err(anyhow!(
            "Invalid edit IV length: expected {IV_SIZE}, got {}",
            iv.len()
        ));
    }
    let plaintext = decrypt_addon(enc_payload, iv, message_secret, &ctx.as_addon_ctx())?;
    let msg = waproto::whatsapp::Message::decode(&plaintext[..])
        .map_err(|e| anyhow!("Failed to decode inner edit Message: {e}"))?;
    Ok(msg)
}

/// Try decryption with up to two JID combinations, mirroring WA Web's
/// `decryptAddOn` resilience for cross-addressing edits.
///
/// The pair `(originals, alternates)` is the (as-received, normalised)
/// combination — caller decides which is which. Returns the first success.
///
/// WA Web tries 3 attempts (LID→LID, PN→PN, originals) but two are enough
/// in practice when the caller already knows the canonical pair.
pub fn decrypt_message_edit_with_fallback(
    enc_payload: &[u8],
    iv: &[u8],
    message_secret: &[u8],
    primary: &MessageEditContext<'_>,
    fallback: Option<&MessageEditContext<'_>>,
) -> Result<waproto::whatsapp::Message> {
    match decrypt_message_edit(enc_payload, iv, message_secret, primary) {
        Ok(m) => Ok(m),
        Err(primary_err) => match fallback {
            Some(fb) => {
                decrypt_message_edit(enc_payload, iv, message_secret, fb).map_err(|fb_err| {
                    anyhow!("edit decrypt failed: primary={primary_err}; fallback={fb_err}")
                })
            }
            None => Err(primary_err),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use waproto::whatsapp as wa;

    fn make_inner_edit(new_text: &str) -> wa::Message {
        wa::Message {
            protocol_message: Some(Box::new(wa::message::ProtocolMessage {
                key: Some(wa::MessageKey {
                    remote_jid: Some("g@g.us".to_string()),
                    from_me: Some(true),
                    id: Some("AC1234567890ABCDEF".to_string()),
                    participant: None,
                }),
                r#type: Some(wa::message::protocol_message::Type::MessageEdit as i32),
                edited_message: Some(Box::new(wa::Message {
                    conversation: Some(new_text.to_string()),
                    ..Default::default()
                })),
                timestamp_ms: Some(1_700_000_000_000),
                ..Default::default()
            })),
            ..Default::default()
        }
    }

    #[test]
    fn encrypt_decrypt_roundtrip_text() {
        let secret = [0x07u8; 32];
        let ctx = MessageEditContext {
            original_msg_id: "AC1234567890ABCDEF",
            original_sender_jid: "5511999999999@s.whatsapp.net",
            editor_jid: "5511999999999@s.whatsapp.net",
        };
        let inner = make_inner_edit("edited text");
        let (enc, iv) = encrypt_message_edit(&inner, &secret, &ctx).unwrap();

        let decoded = decrypt_message_edit(&enc, &iv, &secret, &ctx).unwrap();
        let edited = decoded
            .protocol_message
            .as_ref()
            .and_then(|pm| pm.edited_message.as_ref())
            .expect("inner edited message present");
        assert_eq!(edited.conversation.as_deref(), Some("edited text"));
    }

    #[test]
    fn encrypt_decrypt_roundtrip_with_lid_jids() {
        let secret = [0x42u8; 32];
        let ctx = MessageEditContext {
            original_msg_id: "AC1191FE0A25A0E319BEA72064819280",
            original_sender_jid: "260661598801930@lid",
            editor_jid: "260661598801930@lid",
        };
        let inner = make_inner_edit("B");
        let (enc, iv) = encrypt_message_edit(&inner, &secret, &ctx).unwrap();
        let decoded = decrypt_message_edit(&enc, &iv, &secret, &ctx).unwrap();
        assert_eq!(
            decoded
                .protocol_message
                .as_ref()
                .and_then(|pm| pm.edited_message.as_ref())
                .and_then(|m| m.conversation.as_deref()),
            Some("B")
        );
    }

    #[test]
    fn wrong_editor_jid_fails() {
        // GCM tag should fail when the editor JID feeding HKDF differs.
        let secret = [0x07u8; 32];
        let ctx = MessageEditContext {
            original_msg_id: "AC1",
            original_sender_jid: "a@s.whatsapp.net",
            editor_jid: "a@s.whatsapp.net",
        };
        let (enc, iv) = encrypt_message_edit(&make_inner_edit("x"), &secret, &ctx).unwrap();

        let bad = MessageEditContext {
            editor_jid: "b@s.whatsapp.net",
            ..ctx
        };
        assert!(decrypt_message_edit(&enc, &iv, &secret, &bad).is_err());
    }

    #[test]
    fn wrong_message_secret_fails() {
        let ctx = MessageEditContext {
            original_msg_id: "AC1",
            original_sender_jid: "a@s.whatsapp.net",
            editor_jid: "a@s.whatsapp.net",
        };
        let (enc, iv) = encrypt_message_edit(&make_inner_edit("x"), &[0x07u8; 32], &ctx).unwrap();
        assert!(decrypt_message_edit(&enc, &iv, &[0x08u8; 32], &ctx).is_err());
    }

    #[test]
    fn invalid_iv_length_rejected() {
        let ctx = MessageEditContext {
            original_msg_id: "AC1",
            original_sender_jid: "a@s.whatsapp.net",
            editor_jid: "a@s.whatsapp.net",
        };
        let (enc, _iv) = encrypt_message_edit(&make_inner_edit("x"), &[0x07u8; 32], &ctx).unwrap();
        // WA Web enforces 12-byte IV; we surface a typed error here.
        assert!(decrypt_message_edit(&enc, &[0u8; 11], &[0x07u8; 32], &ctx).is_err());
        assert!(decrypt_message_edit(&enc, &[0u8; 16], &[0x07u8; 32], &ctx).is_err());
    }

    #[test]
    fn fallback_recovers_on_alternate_jid_form() {
        let secret = [0x09u8; 32];
        // Encrypted under PN form...
        let pn_ctx = MessageEditContext {
            original_msg_id: "ID",
            original_sender_jid: "5511999@s.whatsapp.net",
            editor_jid: "5511999@s.whatsapp.net",
        };
        // ...but consumer first guesses LID.
        let lid_ctx = MessageEditContext {
            original_msg_id: "ID",
            original_sender_jid: "12345@lid",
            editor_jid: "12345@lid",
        };

        let (enc, iv) = encrypt_message_edit(&make_inner_edit("hello"), &secret, &pn_ctx).unwrap();

        // Primary (LID) fails, fallback (PN) succeeds.
        let m = decrypt_message_edit_with_fallback(&enc, &iv, &secret, &lid_ctx, Some(&pn_ctx))
            .expect("fallback should rescue");
        assert_eq!(
            m.protocol_message
                .as_ref()
                .and_then(|pm| pm.edited_message.as_ref())
                .and_then(|m| m.conversation.as_deref()),
            Some("hello")
        );
    }

    #[test]
    fn fallback_returns_combined_error_when_both_fail() {
        let secret = [0x09u8; 32];
        let pn_ctx = MessageEditContext {
            original_msg_id: "ID",
            original_sender_jid: "5511999@s.whatsapp.net",
            editor_jid: "5511999@s.whatsapp.net",
        };
        let (enc, iv) = encrypt_message_edit(&make_inner_edit("x"), &secret, &pn_ctx).unwrap();

        let wrong1 = MessageEditContext {
            editor_jid: "evil1@s.whatsapp.net",
            ..pn_ctx
        };
        let wrong2 = MessageEditContext {
            editor_jid: "evil2@s.whatsapp.net",
            ..pn_ctx
        };
        let err = decrypt_message_edit_with_fallback(&enc, &iv, &secret, &wrong1, Some(&wrong2))
            .expect_err("both fail");
        let s = err.to_string();
        assert!(s.contains("primary="));
        assert!(s.contains("fallback="));
    }
}

//! Decryption of E2E message-edit envelopes (`secret_encrypted_message`
//! with `secret_enc_type = MESSAGE_EDIT`).
//!
//! See [`wacore::message_edit`] for the cryptographic primitives. This module
//! adds high-level helpers that take typed [`Jid`]s, normalise them the same
//! way WA Web does (strip the device suffix, optional LID↔PN fallback) and
//! return the decrypted inner [`wa::Message`].
//!
//! ### Integration
//!
//! The library does not auto-decrypt edits on the dispatch path because doing
//! so requires a callback into the consumer's message store to fetch the
//! parent's `messageContextInfo.messageSecret`. Consumers should:
//!
//! 1. Observe `Event::Message` for messages whose
//!    `message.secret_encrypted_message.secret_enc_type == MessageEdit`.
//! 2. Look up the targeted message via `secret_encrypted_message.target_message_key`.
//! 3. Call [`MessageEdits::decrypt`] with the parent's `messageSecret`.
//! 4. Surface the decoded inner message (e.g. emit their own edit event).
//!
//! This mirrors the existing flow for poll vote decryption ([`crate::features::Polls`]).

use anyhow::{Result, anyhow};
use wacore::message_edit::{self, MessageEditContext};
use wacore_binary::Jid;
use waproto::whatsapp as wa;

use crate::client::Client;

/// Surface for decrypting `MESSAGE_EDIT` envelopes.
pub struct MessageEdits<'a> {
    _client: &'a Client,
}

impl<'a> MessageEdits<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { _client: client }
    }

    /// Decrypt a `secret_encrypted_message` MESSAGE_EDIT envelope.
    ///
    /// JIDs may carry their device suffix — they are normalised before being
    /// fed into the HKDF info buffer (matching WA Web's `widToUserJid`).
    ///
    /// Returns the inner [`wa::Message`]; the new content is at
    /// `result.protocol_message.edited_message`.
    ///
    /// Implementation notes:
    /// - HKDF: `salt = zeros[32]`, `ikm = message_secret`, `info = original_msg_id ||
    ///   original_sender_jid || editor_jid || "Message Edit"`, `L = 32`.
    /// - AAD: empty (confirmed in `docs/captured-js/WAWeb/Addon/Encryption.js`
    ///   function `g` — only PollVote/EventResponse bind stanza+sender into AAD).
    /// - IV must be exactly 12 bytes (matches WA Web's `Parse/MessageEditEncryptedMessageProto.js`).
    pub fn decrypt(
        enc_payload: &[u8],
        enc_iv: &[u8],
        message_secret: &[u8],
        original_msg_id: &str,
        original_sender_jid: &Jid,
        editor_jid: &Jid,
    ) -> Result<wa::Message> {
        let primary_orig = original_sender_jid.to_non_ad().to_string();
        let primary_editor = editor_jid.to_non_ad().to_string();
        let primary = MessageEditContext {
            original_msg_id,
            original_sender_jid: &primary_orig,
            editor_jid: &primary_editor,
        };
        message_edit::decrypt_message_edit(enc_payload, enc_iv, message_secret, &primary)
    }

    /// Same as [`Self::decrypt`] but tries a fallback addressing combination
    /// if the first attempt fails its GCM tag check.
    ///
    /// `fallback_original_sender` / `fallback_editor` are typically the
    /// LID-form when the primary attempt used PN-form (or vice versa). This
    /// mirrors `WAWebAddonEncryption.decryptAddOn` which falls back across
    /// LID/PN to handle cross-addressing edits between newer and legacy
    /// clients.
    #[allow(clippy::too_many_arguments)]
    pub fn decrypt_with_fallback(
        enc_payload: &[u8],
        enc_iv: &[u8],
        message_secret: &[u8],
        original_msg_id: &str,
        original_sender_jid: &Jid,
        editor_jid: &Jid,
        fallback_original_sender: Option<&Jid>,
        fallback_editor: Option<&Jid>,
    ) -> Result<wa::Message> {
        let primary_orig = original_sender_jid.to_non_ad().to_string();
        let primary_editor = editor_jid.to_non_ad().to_string();
        let primary = MessageEditContext {
            original_msg_id,
            original_sender_jid: &primary_orig,
            editor_jid: &primary_editor,
        };

        // Build the fallback context if any alternative JID was provided.
        let fb_orig = fallback_original_sender.map(|j| j.to_non_ad().to_string());
        let fb_editor = fallback_editor.map(|j| j.to_non_ad().to_string());
        let fallback_ctx = match (fb_orig.as_deref(), fb_editor.as_deref()) {
            (None, None) => None,
            (orig, editor) => Some(MessageEditContext {
                original_msg_id,
                original_sender_jid: orig.unwrap_or(primary.original_sender_jid),
                editor_jid: editor.unwrap_or(primary.editor_jid),
            }),
        };

        message_edit::decrypt_message_edit_with_fallback(
            enc_payload,
            enc_iv,
            message_secret,
            &primary,
            fallback_ctx.as_ref(),
        )
    }

    /// Pull `enc_payload` / `enc_iv` / `target_message_key` out of a received
    /// [`wa::Message`] if it carries a MESSAGE_EDIT envelope. Returns
    /// `None` if the message is not an encrypted edit.
    ///
    /// Use this in your `Event::Message` handler to detect the envelope
    /// before fetching the parent and calling [`Self::decrypt`].
    pub fn extract_envelope(msg: &wa::Message) -> Option<EncryptedEdit<'_>> {
        let sec = msg.secret_encrypted_message.as_ref()?;
        let enc_type = sec.secret_enc_type();
        if enc_type != wa::message::secret_encrypted_message::SecretEncType::MessageEdit {
            return None;
        }
        let enc_payload = sec.enc_payload.as_deref()?;
        let enc_iv = sec.enc_iv.as_deref()?;
        let target_key = sec.target_message_key.as_ref()?;
        // WA Web validates IV length here too (see Parse/MessageEditEncryptedMessageProto.js)
        if enc_iv.len() != 12 {
            return None;
        }
        Some(EncryptedEdit {
            enc_payload,
            enc_iv,
            target_message_key: target_key,
        })
    }

    /// Rewrap a decrypted edit `inner` into the same shape produced by the
    /// legacy `protocol_message.edited_message` path so downstream consumers
    /// can use one code path:
    ///
    /// ```text
    /// Message { protocol_message: { edited_message: <inner_edited_message> } }
    /// ```
    ///
    /// `inner` is the value returned by [`Self::decrypt`]. Returns `None`
    /// if the decrypted message did not contain
    /// `protocol_message.edited_message` (caller should log + skip).
    pub fn rewrap_as_legacy_edit(inner: wa::Message) -> Option<wa::Message> {
        let pm = inner.protocol_message?;
        let edited = pm.edited_message?;
        Some(wa::Message {
            protocol_message: Some(Box::new(wa::message::ProtocolMessage {
                key: pm.key,
                r#type: Some(wa::message::protocol_message::Type::MessageEdit as i32),
                edited_message: Some(edited),
                timestamp_ms: pm.timestamp_ms,
                ..Default::default()
            })),
            ..Default::default()
        })
    }
}

/// Extracted edit-envelope fields ready to feed into [`MessageEdits::decrypt`].
#[derive(Debug, Clone, Copy)]
pub struct EncryptedEdit<'a> {
    pub enc_payload: &'a [u8],
    pub enc_iv: &'a [u8],
    pub target_message_key: &'a wa::MessageKey,
}

impl<'a> EncryptedEdit<'a> {
    /// Convenience: returns the targeted message id.
    pub fn target_id(&self) -> Option<&str> {
        self.target_message_key.id.as_deref()
    }

    /// Resolve the original sender JID from the target message key.
    /// For groups WA puts the sender in `participant`; for 1:1 it's the
    /// `remote_jid` field. Returns `None` when both are missing.
    pub fn original_sender_jid(&self) -> Result<Jid> {
        let raw = self
            .target_message_key
            .participant
            .as_deref()
            .or(self.target_message_key.remote_jid.as_deref())
            .ok_or_else(|| anyhow!("target message key missing participant and remote_jid"))?;
        raw.parse::<Jid>()
            .map_err(|e| anyhow!("invalid sender jid in target key: {e}"))
    }
}

impl Client {
    pub fn message_edits(&self) -> MessageEdits<'_> {
        MessageEdits::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wacore::message_edit::encrypt_message_edit;

    fn inner(text: &str) -> wa::Message {
        wa::Message {
            protocol_message: Some(Box::new(wa::message::ProtocolMessage {
                key: Some(wa::MessageKey {
                    remote_jid: Some("123@s.whatsapp.net".to_string()),
                    from_me: Some(false),
                    id: Some("AC1".to_string()),
                    participant: None,
                }),
                r#type: Some(wa::message::protocol_message::Type::MessageEdit as i32),
                edited_message: Some(Box::new(wa::Message {
                    conversation: Some(text.to_string()),
                    ..Default::default()
                })),
                timestamp_ms: Some(1_700_000_000_000),
                ..Default::default()
            })),
            ..Default::default()
        }
    }

    #[test]
    fn decrypt_normalises_device_suffix() {
        let secret = [0x55u8; 32];
        // Encrypt with the non-AD form, the only form WA actually feeds to HKDF.
        let ctx = MessageEditContext {
            original_msg_id: "AC1",
            original_sender_jid: "5511999@s.whatsapp.net",
            editor_jid: "5511999@s.whatsapp.net",
        };
        let (enc, iv) = encrypt_message_edit(&inner("hi"), &secret, &ctx).unwrap();

        // Caller passes JIDs with device numbers — they should be stripped.
        let with_device = "5511999:13@s.whatsapp.net".parse::<Jid>().unwrap();
        let m =
            MessageEdits::decrypt(&enc, &iv, &secret, "AC1", &with_device, &with_device).unwrap();
        assert_eq!(
            m.protocol_message
                .as_ref()
                .and_then(|pm| pm.edited_message.as_ref())
                .and_then(|e| e.conversation.as_deref()),
            Some("hi")
        );
    }

    #[test]
    fn extract_envelope_recognises_message_edit() {
        let msg = wa::Message {
            secret_encrypted_message: Some(wa::message::SecretEncryptedMessage {
                target_message_key: Some(wa::MessageKey {
                    remote_jid: Some("g@g.us".to_string()),
                    from_me: Some(false),
                    id: Some("AC1".to_string()),
                    participant: Some("5511999@s.whatsapp.net".to_string()),
                }),
                enc_payload: Some(vec![0u8; 32]),
                enc_iv: Some(vec![0u8; 12]),
                secret_enc_type: Some(
                    wa::message::secret_encrypted_message::SecretEncType::MessageEdit as i32,
                ),
                remote_key_id: None,
            }),
            ..Default::default()
        };
        let env = MessageEdits::extract_envelope(&msg).expect("recognised");
        assert_eq!(env.target_id(), Some("AC1"));
        // Group: participant takes priority over remote_jid.
        assert_eq!(
            env.original_sender_jid().unwrap().to_non_ad().to_string(),
            "5511999@s.whatsapp.net"
        );
    }

    #[test]
    fn extract_envelope_rejects_non_edit_secret_enc_type() {
        // EVENT_EDIT envelope — must not be picked up by the MESSAGE_EDIT extractor.
        let msg = wa::Message {
            secret_encrypted_message: Some(wa::message::SecretEncryptedMessage {
                target_message_key: Some(wa::MessageKey::default()),
                enc_payload: Some(vec![0u8; 32]),
                enc_iv: Some(vec![0u8; 12]),
                secret_enc_type: Some(
                    wa::message::secret_encrypted_message::SecretEncType::EventEdit as i32,
                ),
                remote_key_id: None,
            }),
            ..Default::default()
        };
        assert!(MessageEdits::extract_envelope(&msg).is_none());
    }

    #[test]
    fn extract_envelope_rejects_invalid_iv_size() {
        let msg = wa::Message {
            secret_encrypted_message: Some(wa::message::SecretEncryptedMessage {
                target_message_key: Some(wa::MessageKey::default()),
                enc_payload: Some(vec![0u8; 32]),
                enc_iv: Some(vec![0u8; 11]),
                secret_enc_type: Some(
                    wa::message::secret_encrypted_message::SecretEncType::MessageEdit as i32,
                ),
                remote_key_id: None,
            }),
            ..Default::default()
        };
        assert!(MessageEdits::extract_envelope(&msg).is_none());
    }

    #[test]
    fn rewrap_yields_legacy_shape() {
        let dec = inner("edited");
        let rewrap = MessageEdits::rewrap_as_legacy_edit(dec).expect("present");
        let edited = rewrap
            .protocol_message
            .as_ref()
            .and_then(|pm| pm.edited_message.as_ref())
            .and_then(|m| m.conversation.as_deref());
        assert_eq!(edited, Some("edited"));
        // The rewrapped message has a protocol message of type MessageEdit.
        assert_eq!(
            rewrap.protocol_message.as_ref().and_then(|pm| pm.r#type),
            Some(wa::message::protocol_message::Type::MessageEdit as i32)
        );
    }

    #[test]
    fn rewrap_returns_none_when_inner_missing_edit() {
        let m = wa::Message {
            protocol_message: Some(Box::new(wa::message::ProtocolMessage::default())),
            ..Default::default()
        };
        assert!(MessageEdits::rewrap_as_legacy_edit(m).is_none());
    }
}

//! Decryption of E2E message-edit envelopes (`secret_encrypted_message`
//! with `secret_enc_type = MESSAGE_EDIT`).
//!
//! See [`wacore::message_edit`] for the cryptographic primitives. This
//! module is the high-level surface: it takes typed [`Jid`]s, normalises
//! them the same way WA Web does (strip device suffix, optional LID↔PN
//! fallback) and returns the decrypted inner [`wa::Message`].
//!
//! ### Integration
//!
//! The library does not auto-decrypt edits on the dispatch path because
//! doing so requires a callback into the consumer's message store to
//! fetch the parent's `messageContextInfo.messageSecret`. Consumers:
//!
//! 1. Observe `Event::Message` for messages whose
//!    `message.secret_encrypted_message.secret_enc_type == MessageEdit`.
//! 2. Detect the envelope with [`extract_envelope`].
//! 3. Look up the targeted message via `target_message_key`.
//! 4. Call [`decrypt`] with the parent's `messageSecret`.
//! 5. Optionally call [`rewrap_as_legacy_edit`] so downstream code that
//!    already handles `protocol_message.edited_message` sees one shape.
//!
//! Mirrors the existing flow for poll vote decryption (`Polls::decrypt_vote`).

use anyhow::{Result, anyhow};
use log::warn;
use wacore::message_edit::{self, MessageEditContext};
use wacore_binary::Jid;
use waproto::whatsapp as wa;

/// Decrypt a `secret_encrypted_message` MESSAGE_EDIT envelope.
///
/// JIDs may carry their device suffix — they are normalised before being
/// fed into the HKDF info buffer (matching WA Web's `widToUserJid`).
///
/// Returns the inner [`wa::Message`]; the new content is at
/// `result.protocol_message.edited_message`.
///
/// Implementation notes:
/// - HKDF: `salt = zeros[32]`, `ikm = message_secret`,
///   `info = original_msg_id || original_sender_jid || editor_jid || "Message Edit"`,
///   `L = 32`.
/// - AAD: empty. WA Web's `WAWebAddonEncryption` (function `g`) only binds
///   `stanzaId\0sender` into AAD for PollVote/EventResponse; everything
///   else, including MessageEdit, uses an empty AAD.
/// - IV must be exactly 12 bytes (matches WA Web's
///   `WAWebParseMessageEditEncryptedMessageProto`).
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

/// Same as [`decrypt`] but tries a fallback addressing combination if
/// the first attempt fails its GCM tag check.
///
/// `fallback_original_sender` / `fallback_editor` are typically the LID
/// form when the primary attempt used PN form (or vice versa). Mirrors
/// `WAWebAddonEncryption.decryptAddOn`, which falls back across LID/PN
/// to handle cross-addressing edits between newer and legacy clients.
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

    let fb_orig = fallback_original_sender.map(|j| j.to_non_ad().to_string());
    let fb_editor = fallback_editor.map(|j| j.to_non_ad().to_string());
    let fb_orig_resolved = fb_orig.as_deref().unwrap_or(primary.original_sender_jid);
    let fb_editor_resolved = fb_editor.as_deref().unwrap_or(primary.editor_jid);
    // Skip the retry when the fallback would key the HKDF identically to
    // primary — covers both "no fallback supplied" and "fallback normalises
    // to the same JIDs". Avoids a guaranteed-failing duplicate decrypt.
    let fallback_ctx = if fb_orig_resolved == primary.original_sender_jid
        && fb_editor_resolved == primary.editor_jid
    {
        None
    } else {
        Some(MessageEditContext {
            original_msg_id,
            original_sender_jid: fb_orig_resolved,
            editor_jid: fb_editor_resolved,
        })
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
/// [`wa::Message`] if it carries a MESSAGE_EDIT envelope. Returns `None`
/// if the message is not an encrypted edit, or if the envelope is
/// malformed (missing fields, IV not 12 bytes).
///
/// Malformed-but-tagged envelopes emit a `log::warn!` so the gap is
/// visible without exposing the encrypted payload.
pub fn extract_envelope(msg: &wa::Message) -> Option<EncryptedEdit<'_>> {
    let sec = msg.secret_encrypted_message.as_ref()?;
    let enc_type = sec.secret_enc_type();
    if enc_type != wa::message::secret_encrypted_message::SecretEncType::MessageEdit {
        return None;
    }
    let target_key = sec.target_message_key.as_ref();
    let enc_payload = sec.enc_payload.as_deref();
    let enc_iv = sec.enc_iv.as_deref();

    match (target_key, enc_payload, enc_iv) {
        (Some(tk), Some(payload), Some(iv)) if iv.len() == 12 => Some(EncryptedEdit {
            enc_payload: payload,
            enc_iv: iv,
            target_message_key: tk,
        }),
        (tk, payload, iv) => {
            warn!(
                "secret_encrypted_message MESSAGE_EDIT malformed: target_id={:?} has_payload={} iv_len={:?} (expected 12)",
                tk.and_then(|t| t.id.as_deref()),
                payload.is_some(),
                iv.map(|b| b.len()),
            );
            None
        }
    }
}

/// Rewrap a decrypted edit `inner` into the same shape produced by the
/// legacy `protocol_message.edited_message` path so downstream consumers
/// can use one code path:
///
/// ```text
/// Message { protocol_message: { edited_message: <inner_edited_message> } }
/// ```
///
/// `inner` is the value returned by [`decrypt`]. Returns `None` if the
/// decrypted message did not contain `protocol_message.edited_message`
/// (caller should log + skip).
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

/// Extracted edit-envelope fields ready to feed into [`decrypt`].
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
    ///
    /// `my_jid` is the receiver's own JID in the addressing mode of the
    /// chat (PN or LID). It is needed because for self-sent edits — e.g.
    /// edits to our own messages that arrive via device sync —
    /// `target_message_key` has `from_me = true` and its `remote_jid`
    /// points to the *other* party, not us. WA Web's
    /// `MsgGetters.getOriginalSender` reads `originalSelfAuthor || sender`
    /// from its materialised msg-row store; we have no row here, so we
    /// reconstruct the same fact from `from_me` + own jid.
    ///
    /// Resolution order:
    /// 1. `participant` if present (always set in groups).
    /// 2. `my_jid` if `from_me == Some(true)` (self-sent edit sync).
    /// 3. `remote_jid` (1:1 incoming edit; the chat is the other party).
    pub fn original_sender_jid(&self, my_jid: &Jid) -> Result<Jid> {
        if let Some(p) = self.target_message_key.participant.as_deref() {
            return p
                .parse::<Jid>()
                .map_err(|e| anyhow!("invalid participant jid in target key: {e}"));
        }
        if self.target_message_key.from_me == Some(true) {
            return Ok(my_jid.to_non_ad());
        }
        let raw = self
            .target_message_key
            .remote_jid
            .as_deref()
            .ok_or_else(|| anyhow!("target message key missing participant and remote_jid"))?;
        raw.parse::<Jid>()
            .map_err(|e| anyhow!("invalid remote_jid in target key: {e}"))
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
        let m = decrypt(&enc, &iv, &secret, "AC1", &with_device, &with_device).unwrap();
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
        let env = extract_envelope(&msg).expect("recognised");
        assert_eq!(env.target_id(), Some("AC1"));
        // Group: participant takes priority over my_jid and remote_jid.
        let my_jid = "999@s.whatsapp.net".parse::<Jid>().unwrap();
        assert_eq!(
            env.original_sender_jid(&my_jid).unwrap().to_string(),
            "5511999@s.whatsapp.net"
        );
    }

    #[test]
    fn original_sender_jid_uses_my_jid_for_self_sent_edits() {
        let msg = wa::Message {
            secret_encrypted_message: Some(wa::message::SecretEncryptedMessage {
                target_message_key: Some(wa::MessageKey {
                    remote_jid: Some("5510000@s.whatsapp.net".to_string()),
                    from_me: Some(true),
                    id: Some("AC1".to_string()),
                    participant: None,
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
        let env = extract_envelope(&msg).expect("recognised");
        let my_jid = "5511999:13@s.whatsapp.net".parse::<Jid>().unwrap();
        // Must return my_jid (stripped of device), NOT remote_jid (the other party).
        assert_eq!(
            env.original_sender_jid(&my_jid).unwrap().to_string(),
            "5511999@s.whatsapp.net"
        );
    }

    #[test]
    fn original_sender_jid_falls_back_to_remote_jid_for_incoming_one_to_one_edit() {
        let msg = wa::Message {
            secret_encrypted_message: Some(wa::message::SecretEncryptedMessage {
                target_message_key: Some(wa::MessageKey {
                    remote_jid: Some("5510000@s.whatsapp.net".to_string()),
                    from_me: Some(false),
                    id: Some("AC1".to_string()),
                    participant: None,
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
        let env = extract_envelope(&msg).expect("recognised");
        let my_jid = "5511999@s.whatsapp.net".parse::<Jid>().unwrap();
        assert_eq!(
            env.original_sender_jid(&my_jid).unwrap().to_string(),
            "5510000@s.whatsapp.net"
        );
    }

    #[test]
    fn extract_envelope_rejects_non_edit_secret_enc_type() {
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
        assert!(extract_envelope(&msg).is_none());
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
        assert!(extract_envelope(&msg).is_none());
    }

    #[test]
    fn fallback_normalising_to_primary_jids_is_skipped() {
        // wacore::message_edit::decrypt_message_edit_with_fallback returns the
        // bare primary error when no fallback is run, or a combined
        // "edit decrypt failed: primary=...; fallback=..." when both attempts
        // run. We use that to assert the dedup path.
        let secret = [0xAAu8; 32];
        let real_ctx = MessageEditContext {
            original_msg_id: "ID",
            original_sender_jid: "5511777@s.whatsapp.net",
            editor_jid: "5511777@s.whatsapp.net",
        };
        let (enc, iv) = encrypt_message_edit(&inner("hi"), &secret, &real_ctx).unwrap();

        // Wrong primary JID so decrypt fails; fallback is a device-suffixed
        // form of the *same* wrong jid → normalises identical → must be skipped.
        let wrong = "5511000@s.whatsapp.net".parse::<Jid>().unwrap();
        let wrong_with_device = "5511000:5@s.whatsapp.net".parse::<Jid>().unwrap();

        let err = decrypt_with_fallback(
            &enc,
            &iv,
            &secret,
            "ID",
            &wrong,
            &wrong,
            Some(&wrong_with_device),
            Some(&wrong_with_device),
        )
        .expect_err("decryption should fail");
        assert!(
            !err.to_string().contains("fallback="),
            "no-op fallback must be skipped, got: {err}"
        );
    }

    #[test]
    fn rewrap_yields_legacy_shape() {
        let dec = inner("edited");
        let rewrap = rewrap_as_legacy_edit(dec).expect("present");
        let edited = rewrap
            .protocol_message
            .as_ref()
            .and_then(|pm| pm.edited_message.as_ref())
            .and_then(|m| m.conversation.as_deref());
        assert_eq!(edited, Some("edited"));
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
        assert!(rewrap_as_legacy_edit(m).is_none());
    }
}

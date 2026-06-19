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
use wacore::secret_enc_addon::ModificationType;
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
    let primary_orig = original_sender_jid.to_non_ad_string();
    let primary_editor = editor_jid.to_non_ad_string();
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
    decrypt_secret_encrypted_with_fallback(
        enc_payload,
        enc_iv,
        message_secret,
        SecretEncKind::MessageEdit,
        original_msg_id,
        original_sender_jid,
        editor_jid,
        fallback_original_sender,
        fallback_editor,
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
    let env = extract_secret_encrypted(msg)?;
    (env.kind == SecretEncKind::MessageEdit).then_some(EncryptedEdit {
        enc_payload: env.enc_payload,
        enc_iv: env.enc_iv,
        target_message_key: env.target_message_key,
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

    /// Resolve the original sender JID from the target message key alone.
    ///
    /// CAUTION: `target_message_key` is written in the *editor's* frame, so its
    /// `from_me` is `true` for any edit the editor authored, including an
    /// incoming peer edit, where it then resolves to `my_jid` (the receiver)
    /// rather than the peer. On the receive path use
    /// [`Self::original_sender_for_dispatch`], which takes the author from the
    /// envelope frame. This target-key-only resolver is kept for callers that
    /// have no envelope frame.
    ///
    /// `my_jid` is the receiver's own JID in the addressing mode of the chat
    /// (PN or LID). Resolution order:
    /// 1. `participant` if present (always set in groups).
    /// 2. `my_jid` if `from_me == Some(true)`.
    /// 3. `remote_jid` otherwise.
    pub fn original_sender_jid(&self, my_jid: &Jid) -> Result<Jid> {
        resolve_target_sender(self.target_message_key, my_jid)
    }

    /// Resolve the edit's parent author from the dispatch-time envelope frame.
    ///
    /// A message can only be edited by its author, so the parent author of a
    /// MESSAGE_EDIT is always the editor: ourselves for a self-synced edit
    /// (`is_from_me`), else the envelope sender. Mirrors WA Web's
    /// `MsgGetters.getOriginalSender = originalSelfAuthor || sender` and is the
    /// correct resolver for the receive path, where the target key's `from_me`
    /// reflects the editor, not the receiver.
    pub fn original_sender_for_dispatch(
        &self,
        is_from_me: bool,
        envelope_sender: &Jid,
        my_jid: &Jid,
    ) -> Jid {
        edit_author_from_envelope(is_from_me, envelope_sender, my_jid)
    }
}

/// Resolve a MESSAGE_EDIT's parent author from the dispatch-time envelope
/// frame: us for a self-synced edit (`is_from_me`), else the envelope sender.
/// The editor is always the author, so this is the parent author too.
fn edit_author_from_envelope(is_from_me: bool, envelope_sender: &Jid, my_jid: &Jid) -> Jid {
    if is_from_me {
        my_jid.to_non_ad()
    } else {
        envelope_sender.to_non_ad()
    }
}

/// Resolve the original sender JID from a `secret_encrypted_message`'s target
/// key (see [`EncryptedEdit::original_sender_jid`] for the rationale).
fn resolve_target_sender(target: &wa::MessageKey, my_jid: &Jid) -> Result<Jid> {
    if let Some(p) = target.participant.as_deref() {
        return p
            .parse::<Jid>()
            .map_err(|e| anyhow!("invalid participant jid in target key: {e}"));
    }
    if target.from_me == Some(true) {
        return Ok(my_jid.to_non_ad());
    }
    let raw = target
        .remote_jid
        .as_deref()
        .ok_or_else(|| anyhow!("target message key missing participant and remote_jid"))?;
    raw.parse::<Jid>()
        .map_err(|e| anyhow!("invalid remote_jid in target key: {e}"))
}

/// Which `secret_encrypted_message` use case an envelope carries.
///
/// These are the `SecretEncType` variants that decrypt to a `Message` with the
/// shared empty-AAD scheme. `MESSAGE_SCHEDULE` and `UNKNOWN` are intentionally
/// excluded — neither WA Web (`WAWebAddonEncryption`) nor whatsmeow assigns them
/// a use-case secret, so they are not decryptable through this path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretEncKind {
    EventEdit,
    MessageEdit,
    PollEdit,
    PollAddOption,
    /// `enc_reaction_message` (CAG reaction): a distinct top-level field, not a
    /// `SecretEncType`; the inner plaintext is a `ReactionMessage`, not a `Message`.
    EncReaction,
    /// `enc_comment_message` (CAG channel comment): distinct top-level field;
    /// the inner plaintext is the comment body `Message`.
    EncComment,
}

impl SecretEncKind {
    fn from_proto(t: wa::message::secret_encrypted_message::SecretEncType) -> Option<Self> {
        use wa::message::secret_encrypted_message::SecretEncType as T;
        match t {
            T::EventEdit => Some(Self::EventEdit),
            T::MessageEdit => Some(Self::MessageEdit),
            T::PollEdit => Some(Self::PollEdit),
            T::PollAddOption => Some(Self::PollAddOption),
            T::MessageSchedule | T::Unknown => None,
        }
    }

    fn modification_type(self) -> ModificationType {
        match self {
            Self::EventEdit => ModificationType::EventEdit,
            Self::MessageEdit => ModificationType::MessageEdit,
            Self::PollEdit => ModificationType::PollEdit,
            Self::PollAddOption => ModificationType::PollAddOption,
            Self::EncReaction => ModificationType::EncReaction,
            Self::EncComment => ModificationType::EncComment,
        }
    }
}

/// A decryptable `secret_encrypted_message` envelope of any supported kind.
///
/// The general counterpart of [`EncryptedEdit`]: use [`extract_secret_encrypted`]
/// to obtain it, [`Self::original_sender_jid`] to resolve the targeted message's
/// author, then [`decrypt_secret_encrypted`] with the parent's `messageSecret`.
#[derive(Debug, Clone, Copy)]
pub struct SecretEncrypted<'a> {
    pub kind: SecretEncKind,
    pub enc_payload: &'a [u8],
    pub enc_iv: &'a [u8],
    pub target_message_key: &'a wa::MessageKey,
}

impl<'a> SecretEncrypted<'a> {
    pub fn target_id(&self) -> Option<&str> {
        self.target_message_key.id.as_deref()
    }

    /// Resolve the targeted message's author from the target key alone.
    ///
    /// See the caution on [`EncryptedEdit::original_sender_jid`]: for
    /// MESSAGE_EDIT prefer [`Self::original_sender_for_dispatch`] on the receive
    /// path. Authoritative for poll/event kinds (whose target key carries the
    /// real author).
    pub fn original_sender_jid(&self, my_jid: &Jid) -> Result<Jid> {
        resolve_target_sender(self.target_message_key, my_jid)
    }

    /// Resolve the parent message's author for the secret lookup + HKDF info,
    /// using the dispatch-time envelope frame.
    ///
    /// For `MESSAGE_EDIT` the editor is always the author (you can only edit
    /// your own message) and `target_message_key` is written in the editor's
    /// frame — its `from_me` is `true` even for an incoming peer edit, so it is
    /// not a reliable receiver-side signal. Take the author from the envelope
    /// instead: ourselves for a self-synced edit, else the envelope sender.
    /// Other kinds (poll/event) can be modified by a non-author, so for those
    /// `target_message_key` stays authoritative.
    pub fn original_sender_for_dispatch(
        &self,
        is_from_me: bool,
        envelope_sender: &Jid,
        my_jid: &Jid,
    ) -> Result<Jid> {
        match self.kind {
            SecretEncKind::MessageEdit => Ok(edit_author_from_envelope(
                is_from_me,
                envelope_sender,
                my_jid,
            )),
            _ => resolve_target_sender(self.target_message_key, my_jid),
        }
    }
}

/// Extract any supported `secret_encrypted_message` envelope (EVENT_EDIT,
/// MESSAGE_EDIT, POLL_EDIT, POLL_ADD_OPTION) from a received message.
///
/// Returns `None` when the message is not secret-encrypted, carries an
/// unsupported type, or is malformed (missing fields, IV not 12 bytes).
pub fn extract_secret_encrypted(msg: &wa::Message) -> Option<SecretEncrypted<'_>> {
    if let Some(sec) = msg.secret_encrypted_message.as_ref() {
        let kind = SecretEncKind::from_proto(sec.secret_enc_type())?;
        return secret_envelope(
            kind,
            sec.target_message_key.as_ref(),
            sec.enc_payload.as_deref(),
            sec.enc_iv.as_deref(),
        );
    }
    if let Some(enc) = msg.enc_reaction_message.as_ref() {
        return secret_envelope(
            SecretEncKind::EncReaction,
            enc.target_message_key.as_ref(),
            enc.enc_payload.as_deref(),
            enc.enc_iv.as_deref(),
        );
    }
    if let Some(enc) = msg.enc_comment_message.as_ref() {
        return secret_envelope(
            SecretEncKind::EncComment,
            enc.target_message_key.as_ref(),
            enc.enc_payload.as_deref(),
            enc.enc_iv.as_deref(),
        );
    }
    None
}

/// Validate the shared `{target_message_key, enc_payload, enc_iv}` envelope
/// shape (all three present, 12-byte IV) for any addon kind.
fn secret_envelope<'a>(
    kind: SecretEncKind,
    target_message_key: Option<&'a wa::MessageKey>,
    enc_payload: Option<&'a [u8]>,
    enc_iv: Option<&'a [u8]>,
) -> Option<SecretEncrypted<'a>> {
    match (target_message_key, enc_payload, enc_iv) {
        (Some(tk), Some(payload), Some(iv)) if iv.len() == 12 => Some(SecretEncrypted {
            kind,
            enc_payload: payload,
            enc_iv: iv,
            target_message_key: tk,
        }),
        (tk, payload, iv) => {
            warn!(
                "secret_encrypted_message {kind:?} malformed: target_id={:?} has_payload={} iv_len={:?} (expected 12)",
                tk.and_then(|t| t.id.as_deref()),
                payload.is_some(),
                iv.map(|b| b.len()),
            );
            None
        }
    }
}

/// Decrypt a `secret_encrypted_message` of the given `kind` to its inner
/// [`wa::Message`]. JIDs are normalised the same way as [`decrypt`].
pub fn decrypt_secret_encrypted(
    enc_payload: &[u8],
    enc_iv: &[u8],
    message_secret: &[u8],
    kind: SecretEncKind,
    original_msg_id: &str,
    original_sender_jid: &Jid,
    modification_sender_jid: &Jid,
) -> Result<wa::Message> {
    let orig = original_sender_jid.to_non_ad_string();
    let sender = modification_sender_jid.to_non_ad_string();
    match kind {
        // The reaction plaintext is a ReactionMessage, not a Message; surface
        // it in the plaintext-reaction shape (key filled by the caller from
        // the envelope's target_message_key).
        SecretEncKind::EncReaction => {
            let reaction = wacore::reaction::decrypt_reaction_with_secret(
                enc_payload,
                enc_iv,
                message_secret,
                original_msg_id,
                &orig,
                &sender,
            )?;
            Ok(wa::Message {
                reaction_message: Some(Box::new(reaction)),
                ..Default::default()
            })
        }
        SecretEncKind::EncComment => wacore::comment::decrypt_comment_with_secret(
            enc_payload,
            enc_iv,
            message_secret,
            original_msg_id,
            &orig,
            &sender,
        ),
        _ => {
            let ctx = MessageEditContext {
                original_msg_id,
                original_sender_jid: &orig,
                editor_jid: &sender,
            };
            message_edit::decrypt_secret_encrypted(
                enc_payload,
                enc_iv,
                message_secret,
                kind.modification_type(),
                &ctx,
            )
        }
    }
}

/// [`decrypt_secret_encrypted`] with a LID↔PN fallback addressing, mirroring
/// [`decrypt_with_fallback`].
#[allow(clippy::too_many_arguments)]
pub fn decrypt_secret_encrypted_with_fallback(
    enc_payload: &[u8],
    enc_iv: &[u8],
    message_secret: &[u8],
    kind: SecretEncKind,
    original_msg_id: &str,
    original_sender_jid: &Jid,
    modification_sender_jid: &Jid,
    fallback_original_sender: Option<&Jid>,
    fallback_modification_sender: Option<&Jid>,
) -> Result<wa::Message> {
    // The reaction/comment kinds decode a different inner proto, so they go
    // through the per-kind dispatch instead of the wacore Message-only helper.
    // Like the receive path, every distinct LID/PN combination is attempted:
    // a migration case can need the alternate on only ONE side of the HKDF.
    if matches!(kind, SecretEncKind::EncReaction | SecretEncKind::EncComment) {
        let mut last_err = match decrypt_secret_encrypted(
            enc_payload,
            enc_iv,
            message_secret,
            kind,
            original_msg_id,
            original_sender_jid,
            modification_sender_jid,
        ) {
            Ok(inner) => return Ok(inner),
            Err(e) => e,
        };

        let combos = [
            (fallback_original_sender, Some(modification_sender_jid)),
            (Some(original_sender_jid), fallback_modification_sender),
            (fallback_original_sender, fallback_modification_sender),
        ];
        let mut tried: Vec<(Jid, Jid)> = vec![(
            original_sender_jid.to_non_ad(),
            modification_sender_jid.to_non_ad(),
        )];
        for (orig, sender) in combos {
            let (Some(orig), Some(sender)) = (orig, sender) else {
                continue;
            };
            let pair = (orig.to_non_ad(), sender.to_non_ad());
            if tried.contains(&pair) {
                continue;
            }
            match decrypt_secret_encrypted(
                enc_payload,
                enc_iv,
                message_secret,
                kind,
                original_msg_id,
                orig,
                sender,
            ) {
                Ok(inner) => return Ok(inner),
                Err(e) => last_err = anyhow!("{last_err}; fallback: {e}"),
            }
            tried.push(pair);
        }
        return Err(last_err);
    }

    let orig = original_sender_jid.to_non_ad_string();
    let sender = modification_sender_jid.to_non_ad_string();
    let primary = MessageEditContext {
        original_msg_id,
        original_sender_jid: &orig,
        editor_jid: &sender,
    };

    let fb_orig = fallback_original_sender.map(|j| j.to_non_ad_string());
    let fb_sender = fallback_modification_sender.map(|j| j.to_non_ad_string());
    let fb_orig_resolved = fb_orig.as_deref().unwrap_or(primary.original_sender_jid);
    let fb_sender_resolved = fb_sender.as_deref().unwrap_or(primary.editor_jid);
    let fallback_ctx = if fb_orig_resolved == primary.original_sender_jid
        && fb_sender_resolved == primary.editor_jid
    {
        None
    } else {
        Some(MessageEditContext {
            original_msg_id,
            original_sender_jid: fb_orig_resolved,
            editor_jid: fb_sender_resolved,
        })
    };

    message_edit::decrypt_secret_encrypted_with_fallback(
        enc_payload,
        enc_iv,
        message_secret,
        kind.modification_type(),
        &primary,
        fallback_ctx.as_ref(),
    )
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
            secret_encrypted_message: Some(Box::new(wa::message::SecretEncryptedMessage {
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
            })),
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
            secret_encrypted_message: Some(Box::new(wa::message::SecretEncryptedMessage {
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
            })),
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
    fn original_sender_jid_uses_remote_jid_when_target_not_from_me() {
        // Unit-tests the `resolve_target_sender` remote_jid branch, not a real
        // edit frame: an actual incoming peer edit writes the target key in the
        // editor's frame (from_me=true), so the receive path uses the envelope
        // sender via `original_sender_for_dispatch`, not this resolver.
        let msg = wa::Message {
            secret_encrypted_message: Some(Box::new(wa::message::SecretEncryptedMessage {
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
            })),
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
    fn encrypted_edit_dispatch_resolver_uses_envelope_frame() {
        // The MESSAGE_EDIT-specific consumer API resolves from the envelope
        // frame, ignoring the editor-framed target key (here from_me=true).
        let msg = wa::Message {
            secret_encrypted_message: Some(Box::new(wa::message::SecretEncryptedMessage {
                target_message_key: Some(wa::MessageKey {
                    remote_jid: Some("100000000000001@lid".to_string()),
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
            })),
            ..Default::default()
        };
        let env = extract_envelope(&msg).expect("recognised");
        let my_jid = "100000000000001:3@lid".parse::<Jid>().unwrap();
        let editor = "200000000000002@lid".parse::<Jid>().unwrap();
        // Incoming peer edit → envelope sender (editor); device suffix stripped.
        assert_eq!(
            env.original_sender_for_dispatch(false, &editor, &my_jid)
                .to_string(),
            "200000000000002@lid"
        );
        // Self-synced edit → us, device suffix stripped.
        assert_eq!(
            env.original_sender_for_dispatch(true, &editor, &my_jid)
                .to_string(),
            "100000000000001@lid"
        );
    }

    #[test]
    fn message_edit_original_sender_uses_envelope_sender_for_incoming_peer_edit() {
        // Captured wire data: when a peer edits THEIR OWN message, the target
        // key is written in the EDITOR's frame — from_me=true, no participant,
        // even in groups. Trusting target_key.from_me resolves the original
        // sender to *us*, so the parent messageSecret lookup misses. The editor
        // is always the author (you can only edit your own message), so the
        // sender must come from the envelope frame.
        let msg = wa::Message {
            secret_encrypted_message: Some(Box::new(wa::message::SecretEncryptedMessage {
                target_message_key: Some(wa::MessageKey {
                    remote_jid: Some("100000000000001@lid".to_string()), // our LID (editor's frame)
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
            })),
            ..Default::default()
        };
        let env = extract_secret_encrypted(&msg).expect("recognised");
        let my_jid = "100000000000001@lid".parse::<Jid>().unwrap();
        let editor = "200000000000002@lid".parse::<Jid>().unwrap();
        // Incoming peer edit: envelope is NOT from me → sender is the editor.
        assert_eq!(
            env.original_sender_for_dispatch(false, &editor, &my_jid)
                .unwrap()
                .to_string(),
            "200000000000002@lid"
        );
    }

    #[test]
    fn message_edit_original_sender_uses_my_jid_for_self_synced_edit() {
        // Our own edit, synced from another linked device: the envelope IS from
        // me, so the original sender is us — device suffix stripped.
        let msg = wa::Message {
            secret_encrypted_message: Some(Box::new(wa::message::SecretEncryptedMessage {
                target_message_key: Some(wa::MessageKey {
                    remote_jid: Some("200000000000002@lid".to_string()),
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
            })),
            ..Default::default()
        };
        let env = extract_secret_encrypted(&msg).expect("recognised");
        let my_jid = "100000000000001:3@lid".parse::<Jid>().unwrap();
        let editor = "100000000000001@lid".parse::<Jid>().unwrap();
        assert_eq!(
            env.original_sender_for_dispatch(true, &editor, &my_jid)
                .unwrap()
                .to_string(),
            "100000000000001@lid"
        );
    }

    #[test]
    fn poll_edit_original_sender_still_uses_target_key() {
        // Regression guard: poll/event modifications can be authored by someone
        // other than the target's author (e.g. a peer votes on our poll), so the
        // target key stays authoritative for non-edit kinds.
        let msg = wa::Message {
            secret_encrypted_message: Some(Box::new(wa::message::SecretEncryptedMessage {
                target_message_key: Some(wa::MessageKey {
                    remote_jid: Some("g@g.us".to_string()),
                    from_me: Some(false),
                    id: Some("AC1".to_string()),
                    participant: Some("creator@s.whatsapp.net".to_string()),
                }),
                enc_payload: Some(vec![0u8; 32]),
                enc_iv: Some(vec![0u8; 12]),
                secret_enc_type: Some(
                    wa::message::secret_encrypted_message::SecretEncType::PollEdit as i32,
                ),
                remote_key_id: None,
            })),
            ..Default::default()
        };
        let env = extract_secret_encrypted(&msg).expect("recognised");
        let my_jid = "999@s.whatsapp.net".parse::<Jid>().unwrap();
        let voter = "voter@s.whatsapp.net".parse::<Jid>().unwrap();
        // Envelope sender (voter) differs, but the target's participant (poll
        // creator) wins for poll kinds.
        assert_eq!(
            env.original_sender_for_dispatch(false, &voter, &my_jid)
                .unwrap()
                .to_string(),
            "creator@s.whatsapp.net"
        );
    }

    #[test]
    fn extract_envelope_rejects_non_edit_secret_enc_type() {
        let msg = wa::Message {
            secret_encrypted_message: Some(Box::new(wa::message::SecretEncryptedMessage {
                target_message_key: Some(wa::MessageKey::default()),
                enc_payload: Some(vec![0u8; 32]),
                enc_iv: Some(vec![0u8; 12]),
                secret_enc_type: Some(
                    wa::message::secret_encrypted_message::SecretEncType::EventEdit as i32,
                ),
                remote_key_id: None,
            })),
            ..Default::default()
        };
        assert!(extract_envelope(&msg).is_none());
    }

    #[test]
    fn extract_envelope_rejects_invalid_iv_size() {
        let msg = wa::Message {
            secret_encrypted_message: Some(Box::new(wa::message::SecretEncryptedMessage {
                target_message_key: Some(wa::MessageKey::default()),
                enc_payload: Some(vec![0u8; 32]),
                enc_iv: Some(vec![0u8; 11]),
                secret_enc_type: Some(
                    wa::message::secret_encrypted_message::SecretEncType::MessageEdit as i32,
                ),
                remote_key_id: None,
            })),
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

    use wa::message::secret_encrypted_message::SecretEncType;

    fn secret_msg(enc_type: SecretEncType, payload: Vec<u8>, iv: Vec<u8>) -> wa::Message {
        wa::Message {
            secret_encrypted_message: Some(Box::new(wa::message::SecretEncryptedMessage {
                target_message_key: Some(wa::MessageKey {
                    remote_jid: Some("5510000@s.whatsapp.net".to_string()),
                    from_me: Some(false),
                    id: Some("PARENT1".to_string()),
                    participant: None,
                }),
                enc_payload: Some(payload),
                enc_iv: Some(iv),
                secret_enc_type: Some(enc_type as i32),
                remote_key_id: None,
            })),
            ..Default::default()
        }
    }

    #[test]
    fn extract_secret_encrypted_recognises_all_supported_kinds() {
        for (t, k) in [
            (SecretEncType::EventEdit, SecretEncKind::EventEdit),
            (SecretEncType::MessageEdit, SecretEncKind::MessageEdit),
            (SecretEncType::PollEdit, SecretEncKind::PollEdit),
            (SecretEncType::PollAddOption, SecretEncKind::PollAddOption),
        ] {
            let msg = secret_msg(t, vec![0u8; 32], vec![0u8; 12]);
            let env = extract_secret_encrypted(&msg).expect("recognised");
            assert_eq!(env.kind, k);
            assert_eq!(env.target_id(), Some("PARENT1"));
        }
    }

    #[test]
    fn extract_secret_encrypted_rejects_unsupported_kinds() {
        for t in [SecretEncType::MessageSchedule, SecretEncType::Unknown] {
            let msg = secret_msg(t, vec![0u8; 32], vec![0u8; 12]);
            assert!(extract_secret_encrypted(&msg).is_none());
        }
    }

    #[test]
    fn extract_envelope_still_only_matches_message_edit() {
        // The MESSAGE_EDIT-specific helper must ignore other kinds even though
        // the general extractor accepts them.
        let poll = secret_msg(SecretEncType::PollEdit, vec![0u8; 32], vec![0u8; 12]);
        assert!(extract_envelope(&poll).is_none());
        assert!(extract_secret_encrypted(&poll).is_some());

        let edit = secret_msg(SecretEncType::MessageEdit, vec![0u8; 32], vec![0u8; 12]);
        assert!(extract_envelope(&edit).is_some());
    }

    #[test]
    fn decrypt_secret_encrypted_roundtrip_poll_edit() {
        use prost::Message as _;
        use wacore::secret_enc_addon::{AddonContext, encrypt_addon};

        let secret = [0x63u8; 32];
        let parent_id = "PARENT1";
        let creator: Jid = "5510000@s.whatsapp.net".parse().unwrap();
        let actor: Jid = "5511111@s.whatsapp.net".parse().unwrap();

        let payload = wa::Message {
            conversation: Some("poll edited".to_string()),
            ..Default::default()
        }
        .encode_to_vec();
        let (enc, iv) = encrypt_addon(
            &payload,
            &secret,
            &AddonContext {
                stanza_id: parent_id,
                parent_msg_original_sender: &creator.to_string(),
                modification_sender: &actor.to_string(),
                modification_type: ModificationType::PollEdit,
            },
        )
        .unwrap();

        let msg = {
            let mut m = secret_msg(SecretEncType::PollEdit, enc, iv.to_vec());
            // creator is the parent's remote_jid (1:1 incoming).
            if let Some(sec) = m.secret_encrypted_message.as_mut() {
                sec.target_message_key.as_mut().unwrap().remote_jid = Some(creator.to_string());
            }
            m
        };
        let env = extract_secret_encrypted(&msg).unwrap();
        assert_eq!(env.kind, SecretEncKind::PollEdit);

        let my_jid: Jid = "5599999@s.whatsapp.net".parse().unwrap();
        let original_sender = env.original_sender_jid(&my_jid).unwrap();
        assert_eq!(original_sender, creator);

        let out = decrypt_secret_encrypted(
            env.enc_payload,
            env.enc_iv,
            &secret,
            env.kind,
            env.target_id().unwrap(),
            &original_sender,
            &actor,
        )
        .unwrap();
        assert_eq!(out.conversation.as_deref(), Some("poll edited"));
    }
}

#[cfg(test)]
mod enc_addon_tests {
    use super::*;

    fn key(id: &str) -> wa::MessageKey {
        wa::MessageKey {
            id: Some(id.to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn extract_recognises_enc_reaction_and_comment_envelopes() {
        let reaction = wa::Message {
            enc_reaction_message: Some(Box::new(wa::message::EncReactionMessage {
                target_message_key: Some(key("PARENT1")),
                enc_payload: Some(vec![0; 32]),
                enc_iv: Some(vec![0; 12]),
            })),
            ..Default::default()
        };
        let env = extract_secret_encrypted(&reaction).expect("reaction recognised");
        assert_eq!(env.kind, SecretEncKind::EncReaction);
        assert_eq!(env.target_id(), Some("PARENT1"));

        let comment = wa::Message {
            enc_comment_message: Some(Box::new(wa::message::EncCommentMessage {
                target_message_key: Some(key("PARENT2")),
                enc_payload: Some(vec![0; 32]),
                enc_iv: Some(vec![0; 12]),
            })),
            ..Default::default()
        };
        let env = extract_secret_encrypted(&comment).expect("comment recognised");
        assert_eq!(env.kind, SecretEncKind::EncComment);
        assert_eq!(env.target_id(), Some("PARENT2"));
    }

    #[test]
    fn extract_rejects_malformed_enc_reaction_envelope() {
        let bad_iv = wa::Message {
            enc_reaction_message: Some(Box::new(wa::message::EncReactionMessage {
                target_message_key: Some(key("PARENT1")),
                enc_payload: Some(vec![0; 32]),
                enc_iv: Some(vec![0; 8]),
            })),
            ..Default::default()
        };
        assert!(extract_secret_encrypted(&bad_iv).is_none());

        let no_key = wa::Message {
            enc_reaction_message: Some(Box::new(wa::message::EncReactionMessage {
                target_message_key: None,
                enc_payload: Some(vec![0; 32]),
                enc_iv: Some(vec![0; 12]),
            })),
            ..Default::default()
        };
        assert!(extract_secret_encrypted(&no_key).is_none());
    }

    #[test]
    fn enc_reaction_decrypts_via_kind_dispatch_with_fallback() {
        let secret = [0x21u8; 32];
        let author: Jid = "5511000000001@s.whatsapp.net".parse().unwrap();
        let author_lid: Jid = "111111111111111@lid".parse().unwrap();
        let reactor: Jid = "5511000000002@s.whatsapp.net".parse().unwrap();

        // Encrypted under the author's LID identity; the primary (PN) attempt
        // must fail and the LID fallback succeed.
        let (enc, iv) = wacore::reaction::encrypt_reaction_with_secret(
            "\u{2764}",
            42,
            &secret,
            "PARENT1",
            &author_lid.to_non_ad_string(),
            &reactor.to_non_ad_string(),
        )
        .unwrap();

        let out = decrypt_secret_encrypted_with_fallback(
            &enc,
            &iv,
            &secret,
            SecretEncKind::EncReaction,
            "PARENT1",
            &author,
            &reactor,
            Some(&author_lid),
            None,
        )
        .expect("fallback identity must decrypt");
        let rm = out.reaction_message.expect("reaction shape");
        assert_eq!(rm.text.as_deref(), Some("\u{2764}"));

        // Without a distinct fallback the primary error surfaces.
        assert!(
            decrypt_secret_encrypted_with_fallback(
                &enc,
                &iv,
                &secret,
                SecretEncKind::EncReaction,
                "PARENT1",
                &author,
                &reactor,
                None,
                None,
            )
            .is_err()
        );

        // Mixed combo on the OTHER side: encrypted under the modifier's LID
        // while the parent author is already in the right namespace.
        let reactor_lid: Jid = "222222222222222@lid".parse().unwrap();
        let (enc2, iv2) = wacore::reaction::encrypt_reaction_with_secret(
            "\u{1F44D}",
            43,
            &secret,
            "PARENT1",
            &author.to_non_ad_string(),
            &reactor_lid.to_non_ad_string(),
        )
        .unwrap();
        let out = decrypt_secret_encrypted_with_fallback(
            &enc2,
            &iv2,
            &secret,
            SecretEncKind::EncReaction,
            "PARENT1",
            &author,
            &reactor,
            Some(&author_lid),
            Some(&reactor_lid),
        )
        .expect("primary-author + fallback-modifier combination must decrypt");
        assert_eq!(
            out.reaction_message
                .as_ref()
                .and_then(|r| r.text.as_deref()),
            Some("\u{1F44D}")
        );
    }

    #[test]
    fn enc_comment_decrypts_to_inner_body() {
        let secret = [0x22u8; 32];
        let author: Jid = "5511000000001@s.whatsapp.net".parse().unwrap();
        let commenter: Jid = "5511000000002@s.whatsapp.net".parse().unwrap();
        let body = wa::Message {
            extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
                text: Some("hi".to_string()),
                ..Default::default()
            })),
            ..Default::default()
        };
        let (enc, iv) = wacore::comment::encrypt_comment_with_secret(
            &body,
            &secret,
            "PARENT1",
            &author.to_non_ad_string(),
            &commenter.to_non_ad_string(),
        )
        .unwrap();

        let out = decrypt_secret_encrypted(
            &enc,
            &iv,
            &secret,
            SecretEncKind::EncComment,
            "PARENT1",
            &author,
            &commenter,
        )
        .expect("comment decrypts");
        assert_eq!(
            out.extended_text_message
                .as_ref()
                .and_then(|m| m.text.as_deref()),
            Some("hi")
        );
    }
}

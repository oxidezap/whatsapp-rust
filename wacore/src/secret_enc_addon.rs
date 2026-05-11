//! Secret-encrypted addon envelope (HKDF-SHA256 + AES-256-GCM).
//!
//! Mirrors `WAWebAddonEncryption.decryptAddOn` / `WAUseCaseSecret.createUseCaseSecret`
//! from the captured WA Web bundle. Used for the addon family that piggybacks on a
//! parent message's `messageContextInfo.messageSecret`:
//!
//! - Poll Vote, Poll Edit, Poll Add Option
//! - Event Response, Event Edit
//! - Enc Reaction, Enc Comment
//! - **Message Edit** (added by WA in 2026)
//!
//! Key derivation (per `WAUseCaseSecret.createUseCaseSecret`):
//!
//! ```text
//! info = stanzaId || parentMsgOriginalSender || modificationSender || <usecase>
//! key  = HKDF-SHA256(salt = zeros[32], ikm = messageSecret, info, L = 32)
//! ```
//!
//! AAD (per `WAWebAddonEncryption.js` function `g`):
//!
//! - `PollVote` / `EventResponse` → `stanzaId || 0x00 || modificationSenderJid`
//! - everything else (edits, reactions, comments, poll add option) → empty

use anyhow::{Result, anyhow};
use hkdf::Hkdf;
use sha2::Sha256;

use crate::libsignal::crypto::{aes_256_gcm_decrypt, aes_256_gcm_encrypt};

const GCM_IV_SIZE: usize = 12;
const GCM_TAG_SIZE: usize = 16;
const KEY_SIZE: usize = 32;

/// Use-case literal that goes into the HKDF `info` buffer.
///
/// Source of truth: `docs/captured-js/WA/Use/CaseSecret.js` `UseCaseSecretModificationType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModificationType {
    PollVote,
    EncReaction,
    EncComment,
    ReportToken,
    EventResponse,
    EventEdit,
    PollEdit,
    PollAddOption,
    MessageEdit,
}

impl ModificationType {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PollVote => "Poll Vote",
            Self::EncReaction => "Enc Reaction",
            Self::EncComment => "Enc Comment",
            Self::ReportToken => "Report Token",
            Self::EventResponse => "Event Response",
            Self::EventEdit => "Event Edit",
            Self::PollEdit => "Poll Edit",
            Self::PollAddOption => "Poll Add Option",
            Self::MessageEdit => "Message Edit",
        }
    }

    /// Only PollVote and EventResponse bind the stanza+sender into AAD.
    /// Everything else (edits, reactions, comments, add-option) uses empty AAD.
    pub const fn aad_mode(self) -> AadMode {
        match self {
            Self::PollVote | Self::EventResponse => AadMode::StanzaAndSender,
            _ => AadMode::Empty,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AadMode {
    /// AAD = `stanzaId || 0x00 || modificationSenderJid`
    StanzaAndSender,
    /// AAD = empty buffer
    Empty,
}

/// Inputs threaded through every addon (en|de)crypt.
///
/// `parent_msg_original_sender` is the sender of the *targeted* message (the poll
/// creator, the event creator, the edited message's original author, etc.).
/// `modification_sender` is the user performing the addon action (voter, editor,
/// reactor, ...).
#[derive(Debug, Clone, Copy)]
pub struct AddonContext<'a> {
    pub stanza_id: &'a str,
    pub parent_msg_original_sender: &'a str,
    pub modification_sender: &'a str,
    pub modification_type: ModificationType,
}

/// HKDF derivation, matching `WAUseCaseSecret.createUseCaseSecret`.
pub fn derive_use_case_secret(
    message_secret: &[u8],
    ctx: &AddonContext<'_>,
) -> Result<[u8; KEY_SIZE]> {
    if message_secret.len() != KEY_SIZE {
        return Err(anyhow!(
            "Invalid messageSecret size: expected {KEY_SIZE}, got {}",
            message_secret.len()
        ));
    }

    let mut info = Vec::with_capacity(
        ctx.stanza_id.len()
            + ctx.parent_msg_original_sender.len()
            + ctx.modification_sender.len()
            + ctx.modification_type.as_str().len(),
    );
    info.extend_from_slice(ctx.stanza_id.as_bytes());
    info.extend_from_slice(ctx.parent_msg_original_sender.as_bytes());
    info.extend_from_slice(ctx.modification_sender.as_bytes());
    info.extend_from_slice(ctx.modification_type.as_str().as_bytes());

    let hk = Hkdf::<Sha256>::new(None, message_secret);
    let mut key = [0u8; KEY_SIZE];
    hk.expand(&info, &mut key)
        .map_err(|e| anyhow!("HKDF expand failed: {e}"))?;
    Ok(key)
}

pub(crate) fn build_aad(ctx: &AddonContext<'_>) -> Vec<u8> {
    match ctx.modification_type.aad_mode() {
        AadMode::Empty => Vec::new(),
        AadMode::StanzaAndSender => {
            let mut aad =
                Vec::with_capacity(ctx.stanza_id.len() + 1 + ctx.modification_sender.len());
            aad.extend_from_slice(ctx.stanza_id.as_bytes());
            aad.push(0);
            aad.extend_from_slice(ctx.modification_sender.as_bytes());
            aad
        }
    }
}

/// AES-256-GCM encrypt of `plaintext` with addon use-case key and AAD.
/// Returns `(payload_with_tag, iv)`.
pub fn encrypt_addon(
    plaintext: &[u8],
    message_secret: &[u8],
    ctx: &AddonContext<'_>,
) -> Result<(Vec<u8>, [u8; GCM_IV_SIZE])> {
    use rand::Rng;

    let key = derive_use_case_secret(message_secret, ctx)?;
    let aad = build_aad(ctx);

    let mut iv = [0u8; GCM_IV_SIZE];
    rand::make_rng::<rand::rngs::StdRng>().fill_bytes(&mut iv);

    let mut payload = Vec::with_capacity(plaintext.len() + GCM_TAG_SIZE);
    aes_256_gcm_encrypt(&key, &iv, &aad, plaintext, &mut payload)
        .map_err(|e| anyhow!("AES-GCM encrypt failed: {e}"))?;

    Ok((payload, iv))
}

/// AES-256-GCM decrypt with key derived from `message_secret` + `ctx`.
///
/// Returns the raw plaintext; the caller is responsible for decoding the
/// underlying protobuf (the inner shape depends on `modification_type`).
pub fn decrypt_addon(
    enc_payload: &[u8],
    iv: &[u8],
    message_secret: &[u8],
    ctx: &AddonContext<'_>,
) -> Result<Vec<u8>> {
    let nonce: &[u8; GCM_IV_SIZE] = iv
        .try_into()
        .map_err(|_| anyhow!("Invalid IV size: expected {GCM_IV_SIZE}, got {}", iv.len()))?;
    if enc_payload.len() < GCM_TAG_SIZE {
        return Err(anyhow!(
            "Encrypted payload too short: need at least {GCM_TAG_SIZE} bytes for tag, got {}",
            enc_payload.len()
        ));
    }

    let key = derive_use_case_secret(message_secret, ctx)?;
    let aad = build_aad(ctx);

    let mut plaintext = Vec::with_capacity(enc_payload.len().saturating_sub(GCM_TAG_SIZE));
    aes_256_gcm_decrypt(&key, nonce, &aad, enc_payload, &mut plaintext)
        .map_err(|_| anyhow!("Addon GCM tag verification failed"))?;
    Ok(plaintext)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ctx<'a>(
        modification_type: ModificationType,
        stanza_id: &'a str,
        parent: &'a str,
        sender: &'a str,
    ) -> AddonContext<'a> {
        AddonContext {
            stanza_id,
            parent_msg_original_sender: parent,
            modification_sender: sender,
            modification_type,
        }
    }

    #[test]
    fn use_case_literals_match_wa_web() {
        // Mirror WAUseCaseSecret enum exactly.
        assert_eq!(ModificationType::PollVote.as_str(), "Poll Vote");
        assert_eq!(ModificationType::EncReaction.as_str(), "Enc Reaction");
        assert_eq!(ModificationType::EncComment.as_str(), "Enc Comment");
        assert_eq!(ModificationType::ReportToken.as_str(), "Report Token");
        assert_eq!(ModificationType::EventResponse.as_str(), "Event Response");
        assert_eq!(ModificationType::EventEdit.as_str(), "Event Edit");
        assert_eq!(ModificationType::PollEdit.as_str(), "Poll Edit");
        assert_eq!(ModificationType::PollAddOption.as_str(), "Poll Add Option");
        assert_eq!(ModificationType::MessageEdit.as_str(), "Message Edit");
    }

    #[test]
    fn aad_mode_matches_wa_web_function_g() {
        // Per WAWebAddonEncryption.js function `g`: AAD only for PollVote and EventResponse.
        assert_eq!(
            ModificationType::PollVote.aad_mode(),
            AadMode::StanzaAndSender
        );
        assert_eq!(
            ModificationType::EventResponse.aad_mode(),
            AadMode::StanzaAndSender
        );
        for mt in [
            ModificationType::EncReaction,
            ModificationType::EncComment,
            ModificationType::ReportToken,
            ModificationType::EventEdit,
            ModificationType::PollEdit,
            ModificationType::PollAddOption,
            ModificationType::MessageEdit,
        ] {
            assert_eq!(mt.aad_mode(), AadMode::Empty, "{mt:?} must use empty AAD");
        }
    }

    #[test]
    fn derive_key_invalid_secret_size() {
        let bad = [0u8; 16];
        let c = ctx(ModificationType::MessageEdit, "id", "a", "b");
        assert!(derive_use_case_secret(&bad, &c).is_err());
    }

    #[test]
    fn derive_key_changes_with_each_input() {
        let secret = [0xAAu8; 32];
        let base = ctx(ModificationType::MessageEdit, "id1", "alice@s", "bob@s");
        let k0 = derive_use_case_secret(&secret, &base).unwrap();

        // Different stanza id
        let k1 = derive_use_case_secret(
            &secret,
            &ctx(ModificationType::MessageEdit, "id2", "alice@s", "bob@s"),
        )
        .unwrap();
        // Different parent
        let k2 = derive_use_case_secret(
            &secret,
            &ctx(ModificationType::MessageEdit, "id1", "carol@s", "bob@s"),
        )
        .unwrap();
        // Different sender
        let k3 = derive_use_case_secret(
            &secret,
            &ctx(ModificationType::MessageEdit, "id1", "alice@s", "carol@s"),
        )
        .unwrap();
        // Different use-case literal
        let k4 = derive_use_case_secret(
            &secret,
            &ctx(ModificationType::PollEdit, "id1", "alice@s", "bob@s"),
        )
        .unwrap();

        for other in [k1, k2, k3, k4] {
            assert_ne!(k0, other);
        }
    }

    #[test]
    fn encrypt_decrypt_message_edit_roundtrip() {
        let secret = [0x11u8; 32];
        let c = ctx(
            ModificationType::MessageEdit,
            "AC1234567890ABCDEF",
            "5511999999999@s.whatsapp.net",
            "5511999999999@s.whatsapp.net",
        );
        let pt = b"hello world plaintext";
        let (ct, iv) = encrypt_addon(pt, &secret, &c).unwrap();
        let out = decrypt_addon(&ct, &iv, &secret, &c).unwrap();
        assert_eq!(out, pt);
    }

    #[test]
    fn encrypt_decrypt_poll_vote_roundtrip_uses_aad() {
        let secret = [0x22u8; 32];
        let c = ctx(
            ModificationType::PollVote,
            "stanza",
            "creator@s.whatsapp.net",
            "voter@s.whatsapp.net",
        );
        let pt = b"vote payload";
        let (ct, iv) = encrypt_addon(pt, &secret, &c).unwrap();

        // Tamper modification_sender — AAD differs → GCM tag fails.
        let bad = ctx(
            ModificationType::PollVote,
            "stanza",
            "creator@s.whatsapp.net",
            "attacker@s.whatsapp.net",
        );
        assert!(decrypt_addon(&ct, &iv, &secret, &bad).is_err());

        let out = decrypt_addon(&ct, &iv, &secret, &c).unwrap();
        assert_eq!(out, pt);
    }

    #[test]
    fn aad_mismatch_under_same_key_fails_decrypt() {
        // Prove the AAD branch is actually load-bearing in the GCM check, not
        // just an unused field. Bypass derive (which would also change with
        // modification_type) and hand-roll an encrypt with one AAD shape, then
        // try to decrypt with the other under the same key.
        use crate::libsignal::crypto::{aes_256_gcm_decrypt, aes_256_gcm_encrypt};

        let secret = [0x33u8; 32];
        let pv_ctx = ctx(ModificationType::PollVote, "stanza", "p@s", "s@s");
        let me_ctx = ctx(ModificationType::MessageEdit, "stanza", "p@s", "s@s");
        let aad_pv = build_aad(&pv_ctx);
        let aad_me = build_aad(&me_ctx);
        assert!(!aad_pv.is_empty(), "PollVote AAD must bind stanza+sender");
        assert!(aad_me.is_empty(), "MessageEdit AAD must be empty");

        // Pick one key (PollVote's) and use it for both encrypt and decrypt
        // attempts so the only thing that varies is the AAD.
        let key = derive_use_case_secret(&secret, &pv_ctx).unwrap();
        let iv = [0u8; GCM_IV_SIZE];
        let plaintext = b"vote payload";

        let mut ct = Vec::with_capacity(plaintext.len() + GCM_TAG_SIZE);
        aes_256_gcm_encrypt(&key, &iv, &aad_pv, plaintext, &mut ct).unwrap();

        // Same key, PollVote AAD → ok.
        let mut out = Vec::new();
        aes_256_gcm_decrypt(&key, &iv, &aad_pv, &ct, &mut out).unwrap();
        assert_eq!(out, plaintext);

        // Same key, MessageEdit AAD (empty) → must fail despite identical key.
        let mut out2 = Vec::new();
        assert!(aes_256_gcm_decrypt(&key, &iv, &aad_me, &ct, &mut out2).is_err());
    }

    #[test]
    fn decrypt_invalid_iv_size() {
        let secret = [0x44u8; 32];
        let c = ctx(ModificationType::MessageEdit, "id", "p@s", "s@s");
        let (ct, _iv) = encrypt_addon(b"x", &secret, &c).unwrap();
        let bad_iv = [0u8; 8];
        assert!(decrypt_addon(&ct, &bad_iv, &secret, &c).is_err());
    }

    #[test]
    fn decrypt_payload_too_short() {
        let secret = [0x55u8; 32];
        let c = ctx(ModificationType::MessageEdit, "id", "p@s", "s@s");
        let iv = [0u8; GCM_IV_SIZE];
        let too_short = vec![0u8; GCM_TAG_SIZE - 1];
        assert!(decrypt_addon(&too_short, &iv, &secret, &c).is_err());
    }
}

//! Event response encryption.
//!
//! Thin wrapper over [`secret_enc_addon`] specialised for the
//! `EventResponseMessage` proto and the `"Event Response"` use-case.

use anyhow::{Result, ensure};
use waproto::whatsapp::message::EventResponseMessage;

use crate::secret_enc_addon::{AddonContext, ModificationType, decrypt_addon, encrypt_addon};

const GCM_IV_SIZE: usize = 12;
const MESSAGE_SECRET_SIZE: usize = 32;

fn event_response_addon_ctx<'a>(
    stanza_id: &'a str,
    event_creator_jid: &'a str,
    responder_jid: &'a str,
) -> AddonContext<'a> {
    AddonContext {
        stanza_id,
        parent_msg_original_sender: event_creator_jid,
        modification_sender: responder_jid,
        modification_type: ModificationType::EventResponse,
    }
}

/// Encrypt an event response given the parent event's `messageSecret`.
/// Returns `(payload_with_tag, iv)`.
pub fn encrypt_event_response_with_secret(
    response: &EventResponseMessage,
    message_secret: &[u8],
    stanza_id: &str,
    event_creator_jid: &str,
    responder_jid: &str,
) -> Result<(Vec<u8>, [u8; GCM_IV_SIZE])> {
    ensure!(
        message_secret.len() == MESSAGE_SECRET_SIZE,
        "message_secret must be {MESSAGE_SECRET_SIZE} bytes, got {}",
        message_secret.len()
    );
    let plaintext = waproto::codec::event_response_message_to_vec(response);
    encrypt_addon(
        &plaintext,
        message_secret,
        &event_response_addon_ctx(stanza_id, event_creator_jid, responder_jid),
    )
}

/// Decrypt an event response given the parent event's `messageSecret`.
///
/// The event creator + responder JIDs key the derivation and AAD, so they must
/// match what the responder used (matches WA Web `WAWebAddonEncryption`).
pub fn decrypt_event_response_with_secret(
    enc_payload: &[u8],
    iv: &[u8],
    message_secret: &[u8],
    stanza_id: &str,
    event_creator_jid: &str,
    responder_jid: &str,
) -> Result<EventResponseMessage> {
    // The IV length is validated downstream by decrypt_addon (try_into [u8; 12]).
    ensure!(
        message_secret.len() == MESSAGE_SECRET_SIZE,
        "message_secret must be {MESSAGE_SECRET_SIZE} bytes, got {}",
        message_secret.len()
    );
    let plaintext = decrypt_addon(
        enc_payload,
        iv,
        message_secret,
        &event_response_addon_ctx(stanza_id, event_creator_jid, responder_jid),
    )?;
    Ok(waproto::codec::event_response_message_decode(&plaintext)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use waproto::whatsapp::message::event_response_message::EventResponseType;

    #[test]
    fn event_response_roundtrip() {
        let secret = [0x55u8; 32];
        let resp = EventResponseMessage {
            response: Some(EventResponseType::Going),
            timestamp_ms: Some(1_700_000_000_000),
            extra_guest_count: Some(2),
        };
        let (enc, iv) = encrypt_event_response_with_secret(
            &resp,
            &secret,
            "EVTID",
            "5511777777777@s.whatsapp.net",
            "5511888888888@s.whatsapp.net",
        )
        .unwrap();
        let out = decrypt_event_response_with_secret(
            &enc,
            &iv,
            &secret,
            "EVTID",
            "5511777777777@s.whatsapp.net",
            "5511888888888@s.whatsapp.net",
        )
        .unwrap();
        assert_eq!(out.response, Some(EventResponseType::Going));
        assert_eq!(out.extra_guest_count, Some(2));
    }

    #[test]
    fn event_response_wrong_responder_fails() {
        // A different responder JID derives a different key + AAD, so decryption
        // must fail rather than silently mis-decrypt.
        let secret = [0x55u8; 32];
        let resp = EventResponseMessage {
            response: Some(EventResponseType::Maybe),
            timestamp_ms: None,
            extra_guest_count: None,
        };
        let (enc, iv) = encrypt_event_response_with_secret(
            &resp,
            &secret,
            "EVTID",
            "creator@s.whatsapp.net",
            "responder@s.whatsapp.net",
        )
        .unwrap();
        assert!(
            decrypt_event_response_with_secret(
                &enc,
                &iv,
                &secret,
                "EVTID",
                "creator@s.whatsapp.net",
                "other@s.whatsapp.net",
            )
            .is_err()
        );
    }
}

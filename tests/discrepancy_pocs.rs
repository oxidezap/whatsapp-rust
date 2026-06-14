//! Regression tests pinning whatsapp-rust to WA Web behavior (see
//! `docs/captured-js/`). Assertions encode the **correct** state; a failure
//! here means the implementation has regressed away from WA Web.
//!
//! Run with: `cargo test --test discrepancy_pocs`

use wacore::store::device::Device;
use wacore::types::message::EditAttribute;
use waproto::whatsapp as wa;

// A1. EditAttribute::infer_from_message parity with editAttribute(msg, subtype).

#[test]
fn regression_a1_revoked_reaction_returns_sender_revoke() {
    let msg = wa::Message {
        reaction_message: Some(Box::new(wa::message::ReactionMessage {
            text: Some(String::new()),
            ..Default::default()
        })),
        ..Default::default()
    };
    assert_eq!(
        EditAttribute::infer_from_message(&msg),
        Some(EditAttribute::SenderRevoke),
    );
}

#[test]
fn regression_a1_keep_in_chat_undo_returns_sender_revoke() {
    let msg = wa::Message {
        keep_in_chat_message: Some(Box::new(wa::message::KeepInChatMessage {
            key: Some(wa::MessageKey {
                from_me: Some(true),
                ..Default::default()
            }),
            keep_type: Some(wa::KeepType::UndoKeepForAll as i32),
            ..Default::default()
        })),
        ..Default::default()
    };
    assert_eq!(
        EditAttribute::infer_from_message(&msg),
        Some(EditAttribute::SenderRevoke),
    );
}

#[test]
fn regression_a1_secret_encrypted_message_edit_returns_message_edit() {
    let msg = wa::Message {
        secret_encrypted_message: Some(Box::new(wa::message::SecretEncryptedMessage {
            secret_enc_type: Some(
                wa::message::secret_encrypted_message::SecretEncType::MessageEdit as i32,
            ),
            ..Default::default()
        })),
        ..Default::default()
    };
    assert_eq!(
        EditAttribute::infer_from_message(&msg),
        Some(EditAttribute::MessageEdit),
    );
}

#[test]
fn regression_a1_secret_encrypted_event_edit_returns_message_edit() {
    let msg = wa::Message {
        secret_encrypted_message: Some(Box::new(wa::message::SecretEncryptedMessage {
            secret_enc_type: Some(
                wa::message::secret_encrypted_message::SecretEncType::EventEdit as i32,
            ),
            ..Default::default()
        })),
        ..Default::default()
    };
    assert_eq!(
        EditAttribute::infer_from_message(&msg),
        Some(EditAttribute::MessageEdit),
    );
}

// A4. `passive` defaults to false (WA Web's default) and is configurable.

#[test]
fn regression_a4_login_payload_passive_defaults_to_false() {
    let mut device = Device::new();
    device.pn = Some("5511999999999@s.whatsapp.net".parse().unwrap());
    assert_eq!(device.get_client_payload().passive, Some(false));
}

#[test]
fn regression_a4_login_payload_passive_is_configurable() {
    let mut profile = wacore::client_profile::ClientProfile::web();
    profile.passive_login = true;
    let mut device = Device::new();
    device.set_client_profile(profile);
    device.pn = Some("5511999999999@s.whatsapp.net".parse().unwrap());
    assert_eq!(device.get_client_payload().passive, Some(true));
}

// A5. UserAgent: phone_id is omitted by default (WA Web parity, see
// Client/Payload.js), locale country is ISO-3166-1 alpha-2.

#[test]
fn regression_a5_useragent_phone_id_is_omitted_by_default() {
    let mut device = Device::new();
    device.pn = Some("5511999999999@s.whatsapp.net".parse().unwrap());

    let user_agent = device.get_client_payload().user_agent.unwrap();
    assert!(
        user_agent.phone_id.is_none(),
        "phone_id must stay unset on the wire (WA Web never assigns UserAgent.phoneId)"
    );
}

#[test]
fn regression_a5_useragent_phone_id_can_be_overridden() {
    let mut profile = wacore::client_profile::ClientProfile::web();
    profile.phone_id = Some("deadbeef-0000-0000-0000-000000000000".into());

    let mut device = Device::new();
    device.set_client_profile(profile);
    device.pn = Some("5511999999999@s.whatsapp.net".parse().unwrap());

    assert_eq!(
        device.get_client_payload().user_agent.unwrap().phone_id,
        Some("deadbeef-0000-0000-0000-000000000000".to_string()),
    );
}

#[test]
fn regression_a5_useragent_locale_is_configurable_and_default_is_country_code() {
    let mut device = Device::new();
    device.pn = Some("5511999999999@s.whatsapp.net".parse().unwrap());
    let ua = device.get_client_payload().user_agent.unwrap();
    assert_eq!(ua.locale_language_iso6391.as_deref(), Some("en"));
    assert_eq!(ua.locale_country_iso31661_alpha2.as_deref(), Some("US"));

    let mut profile = wacore::client_profile::ClientProfile::web();
    profile.locale_language = "pt".into();
    profile.locale_country = "BR".into();
    let mut device = Device::new();
    device.set_client_profile(profile);
    device.pn = Some("5511999999999@s.whatsapp.net".parse().unwrap());
    let ua = device.get_client_payload().user_agent.unwrap();
    assert_eq!(ua.locale_language_iso6391.as_deref(), Some("pt"));
    assert_eq!(ua.locale_country_iso31661_alpha2.as_deref(), Some("BR"));
}

// A6. Login payload carries `lc` (login counter) and `lid_db_migrated`.

#[test]
fn regression_a6_login_payload_carries_lc_and_lid_db_migrated() {
    let mut device = Device::new();
    device.pn = Some("5511999999999@s.whatsapp.net".parse().unwrap());

    let payload = device.get_client_payload();
    assert_eq!(payload.lc, Some(0));
    assert_eq!(payload.lid_db_migrated, Some(false));
}

#[test]
fn regression_a6_login_counter_increments_via_device_command() {
    use wacore::store::commands::{DeviceCommand, apply_command_to_device};

    let mut device = Device::new();
    device.pn = Some("5511999999999@s.whatsapp.net".parse().unwrap());
    assert_eq!(device.get_client_payload().lc, Some(0));

    apply_command_to_device(&mut device, DeviceCommand::IncrementLoginCounter);
    apply_command_to_device(&mut device, DeviceCommand::IncrementLoginCounter);
    assert_eq!(device.get_client_payload().lc, Some(2));
}

// A11. default_history_sync_config advertises WA Web's support_* flags.

#[test]
fn regression_a11_history_sync_config_advertises_support_flags() {
    let cfg = wacore::store::device::default_history_sync_config();

    // Static booleans WA Web's WAWebClientPayload always sends.
    assert_eq!(cfg.inline_initial_payload_in_e2_ee_msg, Some(true));
    assert_eq!(cfg.support_bot_user_agent_chat_history, Some(true));
    assert_eq!(cfg.support_cag_reactions_and_polls, Some(true));
    assert_eq!(
        cfg.support_recent_sync_chunk_message_count_tuning,
        Some(true)
    );
    assert_eq!(cfg.support_hosted_group_msg, Some(true));
    assert_eq!(cfg.support_biz_hosted_msg, Some(true));
    assert_eq!(cfg.support_fbid_bot_chat_history, Some(true));
    assert_eq!(cfg.support_message_association, Some(true));

    // Newer support flags previously missing from this lib.
    assert_eq!(cfg.support_group_history, Some(true));
    assert_eq!(cfg.support_manus_history, Some(true));
    assert_eq!(cfg.support_hatch_history, Some(true));

    // Platform-gated in WA Web: only Windows clients advertise it.
    assert_eq!(cfg.support_call_log_history, Some(false));
}

// A7. value-MAC matches WA Web bytewise (u8 packed at offset 7 of 8-byte buf).

/// WA Web oracle for `generate_content_mac`.
fn wa_web_value_mac(
    operation: wa::syncd_mutation::SyncdOperation,
    data: &[u8],
    key_id: &[u8],
    key: &[u8],
) -> [u8; 32] {
    use hmac::Mac;
    type HmacSha512 = hmac::Hmac<sha2::Sha512>;
    let mut mac = <HmacSha512 as hmac::KeyInit>::new_from_slice(key).unwrap();
    mac.update(&[operation as u8 + 1]);
    mac.update(key_id);
    mac.update(data);
    let mut octet = [0u8; 8];
    octet[7] = ((key_id.len() + 1) & 0xff) as u8;
    mac.update(&octet);
    let out = mac.finalize().into_bytes();
    let mut r = [0u8; 32];
    r.copy_from_slice(&out[..32]);
    r
}

#[test]
fn regression_a7_content_mac_matches_wa_web_at_short_key_id() {
    use wacore::appstate::hash::generate_content_mac;

    let op = wa::syncd_mutation::SyncdOperation::Set;
    let key = [7u8; 32];
    let key_id = vec![0u8, 0, 0, 0, 42, 1]; // 6 bytes, ad.length = 7
    let data = b"some-value";

    let ours = generate_content_mac(op, data, &key_id, &key);
    let theirs = wa_web_value_mac(op, data, &key_id, &key);
    assert_eq!(ours, theirs, "MAC must match WA Web for typical key_id");
}

#[test]
fn regression_a7_content_mac_matches_wa_web_at_wrap_boundary() {
    use wacore::appstate::hash::generate_content_mac;

    // ad.length = 256: WA Web encodes octet[7] = 0; the pre-fix Rust code
    // encoded [0,0,0,0,0,0,1,0] (256 BE), which differed.
    let op = wa::syncd_mutation::SyncdOperation::Set;
    let key = [9u8; 32];
    let key_id = vec![0xAA; 255];
    let data = b"x";

    let ours = generate_content_mac(op, data, &key_id, &key);
    let theirs = wa_web_value_mac(op, data, &key_id, &key);
    assert_eq!(
        ours, theirs,
        "MAC must match WA Web even at the 256-byte wrap"
    );
}

// A3. LTHash lanes are little-endian (WA Web spec).

#[test]
fn regression_a3_lthash_lanes_are_little_endian() {
    use wacore::appstate::lthash::WAPATCH_INTEGRITY;

    let mac_a = vec![1u8; 32];
    let mac_b = vec![2u8; 32];

    let mut got = vec![0u8; 128];
    WAPATCH_INTEGRITY.subtract_then_add_in_place(
        &mut got,
        &[mac_b.as_slice()],
        &[mac_a.as_slice()],
    );

    // Reference LE add/sub using the same HKDF expansion.
    use hkdf::Hkdf;
    use sha2::Sha256;
    let derive = |seed: &[u8]| -> Vec<u8> {
        let hk = Hkdf::<Sha256>::new(None, seed);
        let mut out = vec![0u8; 128];
        hk.expand(b"WhatsApp Patch Integrity", &mut out).unwrap();
        out
    };
    let added = derive(&mac_a);
    let removed = derive(&mac_b);
    let mut expected = vec![0u8; 128];
    for i in (0..128).step_by(2) {
        let acc = u16::from_le_bytes([expected[i], expected[i + 1]]);
        let a = u16::from_le_bytes([added[i], added[i + 1]]);
        let r = u16::from_le_bytes([removed[i], removed[i + 1]]);
        let v = acc.wrapping_add(a).wrapping_sub(r);
        let b = v.to_le_bytes();
        expected[i] = b[0];
        expected[i + 1] = b[1];
    }

    assert_eq!(got, expected, "LTHash output must match LE-lane reference");
}

// A10. Media decrypt rejects tampered MACs.

/// Build a valid AES-256-CBC + HMAC-SHA256 payload in the shape the download
/// path expects: ciphertext || mac[..10] over (iv || ciphertext).
fn build_media_payload(media_key: &[u8; 32], plaintext: &[u8]) -> Vec<u8> {
    use aes::cipher::BlockModeEncrypt;
    use cbc::cipher::{KeyIvInit, block_padding::Pkcs7};
    use hmac::Mac;

    type Aes256CbcEnc = cbc::Encryptor<aes::Aes256>;
    type HmacSha256 = hmac::Hmac<sha2::Sha256>;

    let (iv, cipher_key, mac_key) = wacore::download::DownloadUtils::get_media_keys(
        media_key,
        wacore::download::MediaType::Image,
    )
    .unwrap();

    let mut ct = Aes256CbcEnc::new_from_slices(&cipher_key, &iv)
        .unwrap()
        .encrypt_padded_vec::<Pkcs7>(plaintext);

    let mut mac = <HmacSha256 as hmac::KeyInit>::new_from_slice(&mac_key).unwrap();
    mac.update(&iv);
    mac.update(&ct);
    let tag = mac.finalize().into_bytes();
    ct.extend_from_slice(&tag[..10]);
    ct
}

#[test]
fn regression_a10_media_decrypt_accepts_valid_mac_roundtrip() {
    let media_key = [42u8; 32];
    let plaintext = b"the quick brown fox jumps over the lazy dog".to_vec();
    let payload = build_media_payload(&media_key, &plaintext);
    let decoded = wacore::download::DownloadUtils::verify_and_decrypt(
        &payload,
        &media_key,
        wacore::download::MediaType::Image,
    )
    .expect("valid MAC must decrypt");
    assert_eq!(decoded, plaintext);
}

#[test]
fn regression_a10_media_decrypt_rejects_mac_flipped_at_byte_zero() {
    let media_key = [7u8; 32];
    let mut payload = build_media_payload(&media_key, b"payload");
    let mac_start = payload.len() - 10;
    payload[mac_start] ^= 0x01;
    let err = wacore::download::DownloadUtils::verify_and_decrypt(
        &payload,
        &media_key,
        wacore::download::MediaType::Image,
    )
    .expect_err("tampered MAC must be rejected");
    assert!(matches!(
        err,
        wacore::download::MediaDecryptionError::InvalidMac
    ));
}

#[test]
fn regression_a10_media_decrypt_rejects_mac_flipped_at_last_byte() {
    let media_key = [9u8; 32];
    let mut payload = build_media_payload(&media_key, b"payload");
    let last = payload.len() - 1;
    payload[last] ^= 0x80;
    let err = wacore::download::DownloadUtils::verify_and_decrypt(
        &payload,
        &media_key,
        wacore::download::MediaType::Image,
    )
    .expect_err("tampered MAC must be rejected");
    assert!(matches!(
        err,
        wacore::download::MediaDecryptionError::InvalidMac
    ));
}

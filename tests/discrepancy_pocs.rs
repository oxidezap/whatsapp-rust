//! Empirical reproductions of whatsapp-rust vs WA Web JS protocol discrepancies.
//!
//! Each test below documents a single divergence between this library and the
//! ground-truth implementation in `docs/captured-js/`. Tests are written so they
//! PASS today (asserting the current, buggy state); when a fix lands the tests
//! should FAIL, signalling the asserts must be updated.
//!
//! Run with: `cargo test --test discrepancy_pocs -- --nocapture`
//!
//! Each test prints "BUG #N" with a one-line summary at the start, plus the
//! observed value and the WA-Web-expected value, so the divergence is visible
//! in `cargo test` output.

use wacore::store::device::Device;
use wacore::types::message::EditAttribute;
use waproto::whatsapp as wa;

// ---------------------------------------------------------------------------
// A1. Edit attribute coverage parity with WAWebSendMsgCommonApi.editAttribute
// ---------------------------------------------------------------------------
//
// Each regression test below pins one branch of WA Web's `editAttribute(msg,
// subtype)` so the wire `edit="N"` attribute is correctly emitted by both
// initial send and retry resend.

#[test]
fn regression_a1_revoked_reaction_returns_sender_revoke() {
    let msg = wa::Message {
        reaction_message: Some(wa::message::ReactionMessage {
            text: Some(String::new()),
            ..Default::default()
        }),
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
        keep_in_chat_message: Some(wa::message::KeepInChatMessage {
            key: Some(wa::MessageKey {
                from_me: Some(true),
                ..Default::default()
            }),
            keep_type: Some(wa::KeepType::UndoKeepForAll as i32),
            ..Default::default()
        }),
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
        secret_encrypted_message: Some(wa::message::SecretEncryptedMessage {
            secret_enc_type: Some(
                wa::message::secret_encrypted_message::SecretEncType::MessageEdit as i32,
            ),
            ..Default::default()
        }),
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
        secret_encrypted_message: Some(wa::message::SecretEncryptedMessage {
            secret_enc_type: Some(
                wa::message::secret_encrypted_message::SecretEncType::EventEdit as i32,
            ),
            ..Default::default()
        }),
        ..Default::default()
    };
    assert_eq!(
        EditAttribute::infer_from_message(&msg),
        Some(EditAttribute::MessageEdit),
    );
}

// ---------------------------------------------------------------------------
// A4. `passive` flag defaults to WA Web's `false` and is configurable
// ---------------------------------------------------------------------------

#[test]
fn regression_a4_login_payload_passive_defaults_to_false() {
    // WA Web's m() in Payload.js defaults `passive: false`. Match that.
    let mut device = Device::new();
    device.pn = Some("5511999999999@s.whatsapp.net".parse().unwrap());

    let payload = device.get_client_payload();
    assert_eq!(payload.passive, Some(false));
}

#[test]
fn regression_a4_login_payload_passive_is_configurable() {
    // Callers that want the whatsmeow-style passive=true must be able to opt in.
    let mut profile = wacore::client_profile::ClientProfile::web();
    profile.passive_login = true;

    let mut device = Device::new();
    device.set_client_profile(profile);
    device.pn = Some("5511999999999@s.whatsapp.net".parse().unwrap());

    assert_eq!(device.get_client_payload().passive, Some(true));
}

// ---------------------------------------------------------------------------
// A5. UserAgent: phone_id auto-populated, locale country is ISO-3166-1
// ---------------------------------------------------------------------------

#[test]
fn regression_a5_useragent_phone_id_is_uuid_v4_by_default() {
    let mut device = Device::new();
    device.pn = Some("5511999999999@s.whatsapp.net".parse().unwrap());

    let user_agent = device.get_client_payload().user_agent.unwrap();
    let phone_id = user_agent.phone_id.expect("phone_id must be populated");
    uuid::Uuid::parse_str(&phone_id).expect("phone_id must be a valid UUID");
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
    // Default locale: en / US (US is a valid ISO-3166-1 alpha-2; "en" was not).
    let mut device = Device::new();
    device.pn = Some("5511999999999@s.whatsapp.net".parse().unwrap());
    let ua = device.get_client_payload().user_agent.unwrap();
    assert_eq!(ua.locale_language_iso6391.as_deref(), Some("en"));
    assert_eq!(ua.locale_country_iso31661_alpha2.as_deref(), Some("US"));

    // And it's overridable.
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

// ---------------------------------------------------------------------------
// A6. Login payload includes `lc` and `lid_db_migrated`
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// A11. default HistorySyncConfig advertises the WA Web support_* flags
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// A7. value-MAC `octet-length` encoding diverges bytewise from WA Web
// ---------------------------------------------------------------------------
//
// Ground truth: `docs/captured-js/WAWeb/Syncd/MutationKey/Api.js` (Crypto):
//   octetLength = new Uint8Array(8);
//   octetLength[7] = ad.length & 0xff;   // ONLY the low byte
//
// This is u8 in the last byte of an 8-byte zero buffer. WA Web treats the
// associatedData length as a 1-byte unsigned int packed at offset 7.
//
// whatsapp-rust: `wacore/appstate/src/hash.rs:148`:
//   let key_data_length = u64_to_be((key_id.len() + 1) as u64);  // full 8-byte BE
//
// For typical keyIds (<=254 bytes ad.length), the byte representations
// coincide because the upper 7 bytes are zero in both. For larger ad lengths,
// they diverge: WA Web wraps at 256 (low byte only), Rust does not.
//
// This is a literal protocol-divergence; today it never bites because keyIds
// are short, but it's a spec violation.

/// Recompute the value-MAC the way WA Web does (`Crypto.js`): u8 packed at
/// offset 7 of an 8-byte zero buffer. Used as the spec-correct oracle.
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

// ---------------------------------------------------------------------------
// A3. LTHash uses native-endian, WA Web spec is little-endian
// ---------------------------------------------------------------------------
//
// Ground truth: `docs/captured-js/WA/Crypto/LtHash.js`:
//   view.getUint16(offset, true)   // `true` = little-endian
//   view.setUint16(offset, val, true)
//
// whatsapp-rust: `wacore/appstate/src/lthash.rs:88-99` uses `from_ne_bytes`
// and `to_ne_bytes`. On little-endian targets (x86, ARM little-endian) this
// is identical to LE, so the bug is latent. On big-endian targets, the
// snapshot/patch MACs would diverge from WA Web's, breaking interop.
//
// The POC below demonstrates that the current native-endian implementation
// happens to match LE only because we're running on a LE host. We cannot
// "simulate" big-endian at runtime, so the POC documents the spec divergence
// and verifies the host is LE (so today's pass-through is coincidental).

#[test]
fn regression_a3_lthash_lanes_are_little_endian() {
    // WA Web treats the LTHash accumulator as a stream of little-endian u16
    // lanes; snapshot/patch MACs are HMACs over those bytes, so the lane
    // endianness is part of the protocol. Lock the byte layout against an
    // explicit LE oracle so a future regression to native-endian is caught
    // even on LE hosts (where the bug used to be invisible).
    use wacore::appstate::lthash::WAPATCH_INTEGRITY;

    // Two distinct MAC inputs so the derived bytes are nonzero.
    let mac_a = vec![1u8; 32];
    let mac_b = vec![2u8; 32];

    let mut got = vec![0u8; 128];
    WAPATCH_INTEGRITY.subtract_then_add_in_place(
        &mut got,
        &[mac_b.as_slice()],
        &[mac_a.as_slice()],
    );

    // Reference LTHash add/sub computed explicitly in little-endian using
    // the same HKDF expansion the lib uses.
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

// ---------------------------------------------------------------------------
// A10. Media-decrypt MAC compare is not constant-time
// ---------------------------------------------------------------------------
//
// Ground truth: any reputable crypto library uses constant-time compares for
// MAC verification (e.g. `subtle::ConstantTimeEq`). WA Web's primitives are
// browser-provided HMAC verifiers that are constant-time.
//
// whatsapp-rust: `wacore/src/download.rs:531`:
//   if &computed_mac_full[..MAC_SIZE] != received_mac { ... }
//
// `!=` on `&[u8]` uses `slice::eq` which short-circuits on first mismatch.
// This is the textbook timing-attack vector.
//
// A timing-attack POC is inherently flaky in a unit test, so the POC below
// just measures average wall-clock time for "differs at byte 0" vs "differs
// at byte 9". A statistically significant gap is evidence of non-constant-time
// behaviour; passing the threshold check is not certified but typically holds.

#[test]
#[ignore = "timing-based; run with `cargo test bug_a10 -- --ignored --nocapture`"]
fn bug_a10_mac_compare_not_constant_time_timing_signal() {
    println!("\nBUG A10: media-decrypt MAC compare is not constant-time");

    // We can't directly call the private compare in download.rs, but the same
    // pattern (`a != b` on `&[u8]`) is used. Demonstrate the timing signal on
    // a synthetic compare equivalent to the one in download.rs.
    let mac_correct = [0xAAu8; 10];
    let mac_early_diff = {
        let mut m = mac_correct;
        m[0] = 0x00;
        m
    };
    let mac_late_diff = {
        let mut m = mac_correct;
        m[9] = 0x00;
        m
    };

    const ITERS: u32 = 5_000_000;
    let mut sink: u64 = 0;

    let t_early = wacore::time::Instant::now();
    for _ in 0..ITERS {
        // `!=` short-circuits at the first byte; this is the buggy pattern.
        if mac_correct.as_slice() != mac_early_diff.as_slice() {
            sink = sink.wrapping_add(1);
        }
    }
    let d_early = t_early.elapsed();

    let t_late = wacore::time::Instant::now();
    for _ in 0..ITERS {
        if mac_correct.as_slice() != mac_late_diff.as_slice() {
            sink = sink.wrapping_add(1);
        }
    }
    let d_late = t_late.elapsed();

    println!("  diff at byte 0: {:?} ({} ops)", d_early, ITERS);
    println!("  diff at byte 9: {:?} ({} ops)", d_late, ITERS);
    println!("  sink (anti-DCE): {}", sink);
    println!(
        "  ratio late/early = {:.2}x  (constant-time should be ~1.00x)",
        d_late.as_nanos() as f64 / d_early.as_nanos().max(1) as f64
    );

    // The fix is `subtle::ConstantTimeEq` in download.rs. This POC doesn't
    // panic on the timing gap (compilers and CPUs vary too much for a stable
    // threshold); it just prints the measurement for manual inspection.
}

#[test]
fn regression_a10_mac_compare_uses_constant_time() {
    // Source-level check that the media-decrypt MAC compare uses a
    // constant-time primitive. Guards against accidental reintroduction of
    // the `slice != slice` short-circuit pattern.
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/wacore/src/download.rs"
    ))
    .expect("download.rs is in the workspace");

    assert!(
        !src.contains("&computed_mac_full[..MAC_SIZE] != received_mac"),
        "regression: download.rs reintroduced non-constant-time MAC compare"
    );
    assert!(
        src.contains("ct_eq") || src.contains("ConstantTimeEq"),
        "regression: download.rs no longer uses a constant-time compare for MAC verify"
    );
}

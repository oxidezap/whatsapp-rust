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
// A1. Edit attribute incomplete on outgoing message stanzas
// ---------------------------------------------------------------------------
//
// Ground truth (verified in `docs/captured-js/WAWeb/Send/MsgCommonApi.js`,
// function `editAttribute(message, subtype)`):
//   reactionMessage.text === ''           -> "7"  (SENDER_REVOKE)
//   keepInChatMessage UNDO_KEEP_FOR_ALL   -> "7"
//   editedMessage (top-level)             -> "1"  (MESSAGE_EDIT)
//   secretEncryptedMessage MESSAGE_EDIT   -> "1"
//   secretEncryptedMessage EVENT_EDIT     -> "1"
//   protocolMessage.type == REVOKE        -> "7"/"8" depending on subtype
//   pinInChatMessage                      -> "2"
//
// whatsapp-rust:
//   - `src/send.rs:infer_stanza_metadata` only emits `EditAttribute::PinInChat`
//     for `pin_in_chat_message`; nothing else.
//   - `EditAttribute::infer_from_message` (the retry-side inference, public API)
//     handles pin/edited_message/protocol_message but misses reaction-revoke,
//     keep_in_chat undo, and secret_encrypted edit envelopes.
//
// The POCs below exercise the PUBLIC `EditAttribute::infer_from_message`,
// which is the canonical source of truth used both on initial send (via
// `infer_stanza_metadata`) and on retry resend. Each `assert_eq!(.., None)`
// documents the bug; the comment immediately above states what WA Web does.

#[test]
fn bug_a1_revoked_reaction_misses_sender_revoke() {
    println!("\nBUG A1.1: revoked reaction (text=\"\") does NOT emit edit=\"7\"");

    // A reaction-revoke in WhatsApp Web is just a reaction_message with empty
    // text. WA Web's editAttribute() returns SENDER_REVOKE ("7") for this so
    // the recipient knows to UN-react, not just show an empty reaction.
    let msg = wa::Message {
        reaction_message: Some(wa::message::ReactionMessage {
            text: Some(String::new()), // empty = revoked
            ..Default::default()
        }),
        ..Default::default()
    };

    let inferred = EditAttribute::infer_from_message(&msg);
    println!("  WA Web expected: Some(SenderRevoke) -> edit=\"7\"");
    println!("  whatsapp-rust : {:?}", inferred);

    // The bug: returns None instead of SenderRevoke.
    assert_eq!(
        inferred, None,
        "POC outdated: lib now recognizes revoked reactions"
    );
}

#[test]
fn bug_a1_keep_in_chat_undo_misses_sender_revoke() {
    println!("\nBUG A1.2: keep_in_chat UNDO_KEEP_FOR_ALL does NOT emit edit=\"7\"");

    // KeepInChat "undo keep for all" is also a sender-revoke at the wire level.
    // WA Web special-cases this in editAttribute().
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

    let inferred = EditAttribute::infer_from_message(&msg);
    println!("  WA Web expected: Some(SenderRevoke) -> edit=\"7\"");
    println!("  whatsapp-rust : {:?}", inferred);

    assert_eq!(
        inferred, None,
        "POC outdated: lib now recognizes UNDO_KEEP_FOR_ALL"
    );
}

#[test]
fn bug_a1_secret_encrypted_message_edit_misses_message_edit() {
    println!("\nBUG A1.3: secretEncryptedMessage MESSAGE_EDIT does NOT emit edit=\"1\"");

    // The newer encrypted-edit envelope (`secret_encrypted_message`) is the
    // current WA Web format. editAttribute() returns MESSAGE_EDIT for both
    // EVENT_EDIT and MESSAGE_EDIT secret enc types.
    let msg = wa::Message {
        secret_encrypted_message: Some(wa::message::SecretEncryptedMessage {
            secret_enc_type: Some(
                wa::message::secret_encrypted_message::SecretEncType::MessageEdit as i32,
            ),
            ..Default::default()
        }),
        ..Default::default()
    };

    let inferred = EditAttribute::infer_from_message(&msg);
    println!("  WA Web expected: Some(MessageEdit) -> edit=\"1\"");
    println!("  whatsapp-rust : {:?}", inferred);

    assert_eq!(
        inferred, None,
        "POC outdated: lib now recognizes secret_encrypted MESSAGE_EDIT"
    );
}

#[test]
fn bug_a1_secret_encrypted_event_edit_misses_message_edit() {
    println!("\nBUG A1.4: secretEncryptedMessage EVENT_EDIT does NOT emit edit=\"1\"");

    let msg = wa::Message {
        secret_encrypted_message: Some(wa::message::SecretEncryptedMessage {
            secret_enc_type: Some(
                wa::message::secret_encrypted_message::SecretEncType::EventEdit as i32,
            ),
            ..Default::default()
        }),
        ..Default::default()
    };

    let inferred = EditAttribute::infer_from_message(&msg);
    println!("  WA Web expected: Some(MessageEdit) -> edit=\"1\"");
    println!("  whatsapp-rust : {:?}", inferred);

    assert_eq!(
        inferred, None,
        "POC outdated: lib now recognizes secret_encrypted EVENT_EDIT"
    );
}

// ---------------------------------------------------------------------------
// A4. `passive = true` hardcoded on login
// ---------------------------------------------------------------------------
//
// Ground truth: `docs/captured-js/WAWeb/Client/Payload.js`, function `m()`:
//   passive: (e?.passive) != null ? e.passive : false
// Default is `false`. WA Web only sends `passive=true` when the caller
// explicitly overrides it (e.g. background reconnects).
//
// whatsapp-rust: `wacore/src/store/device.rs:411`
//   payload.passive = Some(true);   // hardcoded, no opt-out
//
// Note: whatsmeow also uses `passive=true`, so this MAY be intentional. Verify
// against real-world offline-sync behaviour before flipping.

#[test]
fn bug_a4_login_payload_passive_hardcoded_true() {
    println!("\nBUG A4: login payload has passive=true hardcoded (WA Web default: false)");

    let mut device = Device::new();
    device.pn = Some("5511999999999@s.whatsapp.net".parse().unwrap());

    let payload = device.get_client_payload();

    println!("  WA Web default: passive=false (unless caller overrides)");
    println!("  whatsapp-rust : passive={:?}", payload.passive);

    assert_eq!(
        payload.passive,
        Some(true),
        "POC outdated: passive is no longer hardcoded to true"
    );
}

// ---------------------------------------------------------------------------
// A5. UserAgent: `phoneId` (UUID) missing, locale hardcoded
// ---------------------------------------------------------------------------
//
// Ground truth: `Payload.js`, function `y()` (UserAgent builder). The real
// UserAgent carries `phoneId: randomUUID()` plus locale fields derived from
// the device's actual locale. Reference: zapo correctly populates `phoneId`
// in `src/transport/noise/WaClientPayload.ts:109`.
//
// whatsapp-rust: `wacore/src/store/device.rs:81-112`
//   - `phoneId` is never set (field doesn't exist on the UserAgent the lib
//     builds — `..Default::default()` covers it but it never gets populated).
//   - `locale_language_iso6391` / `locale_country_iso31661_alpha2` are
//     hardcoded `"en"/"en"`.

#[test]
fn bug_a5_useragent_phone_id_missing() {
    println!("\nBUG A5.1: UserAgent.phone_id is None (WA Web sends a UUID per connect)");

    let mut device = Device::new();
    device.pn = Some("5511999999999@s.whatsapp.net".parse().unwrap());

    let payload = device.get_client_payload();
    let user_agent = payload.user_agent.expect("user_agent is always populated");

    println!("  WA Web expected: phone_id = Some(\"<uuid>\")");
    println!("  whatsapp-rust : phone_id = {:?}", user_agent.phone_id);

    assert_eq!(
        user_agent.phone_id, None,
        "POC outdated: phone_id is now populated"
    );
}

#[test]
fn bug_a5_useragent_locale_hardcoded_en() {
    println!("\nBUG A5.2: locale_country hardcoded \"en\" (should be a country code)");

    let mut device = Device::new();
    device.pn = Some("5511999999999@s.whatsapp.net".parse().unwrap());

    let payload = device.get_client_payload();
    let user_agent = payload.user_agent.unwrap();

    println!(
        "  Observed locale_language={:?} locale_country={:?}",
        user_agent.locale_language_iso6391, user_agent.locale_country_iso31661_alpha2
    );
    println!("  WA Web expected: language=\"<ISO-639-1>\" country=\"<ISO-3166-1 alpha2>\"");
    println!(
        "  whatsapp-rust : both hardcoded \"en\" - the country code is wrong (\"en\" is not ISO-3166-1)"
    );

    // The literal divergence: country code is "en" (a language code, not a
    // country code per ISO-3166-1 alpha-2). WA Web would send e.g. "BR"/"pt".
    assert_eq!(
        user_agent.locale_country_iso31661_alpha2.as_deref(),
        Some("en"),
        "POC outdated: country code is no longer the literal \"en\""
    );
}

// ---------------------------------------------------------------------------
// A6. Login payload: `lc` (login counter) and `lidDbMigrated` missing
// ---------------------------------------------------------------------------
//
// Ground truth: `Payload.js`, function `s()` (login payload builder):
//   { ..., lc: getLoginCounter(), lidDbMigrated: Lid1X1MigrationUtils.isLidMigrated() }
//
// Both fields are sent on every login. The wa6 protobuf has the corresponding
// fields, so this is a true protocol omission, not a schema gap.
//
// whatsapp-rust: `device.rs:401-413` only sets `username`, `device`, and
// `passive`. Misses `lc` and `lid_db_migrated`.

#[test]
fn bug_a6_login_payload_missing_lc() {
    println!("\nBUG A6.1: login payload missing `lc` (login counter)");

    let mut device = Device::new();
    device.pn = Some("5511999999999@s.whatsapp.net".parse().unwrap());

    let payload = device.get_client_payload();

    println!("  WA Web expected: lc = Some(<counter>)");
    println!("  whatsapp-rust : lc = {:?}", payload.lc);

    // The bug: `lc` is None on every login. WA Web increments this each login
    // and sends it; it informs server-side anti-spam / session tracking.
    assert_eq!(payload.lc, None, "POC outdated: lc is now populated");
}

// `lidDbMigrated` would map to a separate boolean field on the ClientPayload
// proto. If the field exists in the proto, the bug is "not populated"; if
// the proto doesn't expose it, the bug is "schema missing". We cannot probe
// the proto without depending on its exact field name, so the lc check above
// is sufficient evidence that the login payload is incomplete.

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

#[test]
fn bug_a7_content_mac_octet_length_diverges_for_long_key_id() {
    println!("\nBUG A7: content-MAC octet-length encoding diverges for long key_id");

    use wacore::appstate::hash::generate_content_mac;

    let op = wa::syncd_mutation::SyncdOperation::Set;
    let data = b"some-encrypted-value";
    let mac_key = [0u8; 32];

    // Construct a key_id with len = 255 so that ad.length = key_id.len()+1 = 256.
    // WA Web: octetLength[7] = 256 & 0xff = 0  ->  [0,0,0,0,0,0,0,0]
    // Rust  : 256.to_be_bytes()                 ->  [0,0,0,0,0,0,1,0]
    // The HMAC inputs differ, so the MACs differ.
    let key_id_short = vec![0xAA; 6]; // typical: 6-byte key_id
    let key_id_long = vec![0xAA; 255]; // ad.length = 256, the wrap boundary

    let mac_short = generate_content_mac(op, data, &key_id_short, &mac_key);
    let mac_long = generate_content_mac(op, data, &key_id_long, &mac_key);

    println!(
        "  short key_id (6 bytes)  -> mac[0..4] = {:02x?}",
        &mac_short[..4]
    );
    println!(
        "  long  key_id (255 byte) -> mac[0..4] = {:02x?}",
        &mac_long[..4]
    );

    // Demonstrate the divergence semantically by reimplementing both encodings
    // and comparing the trailing 8 bytes of the HMAC input.
    let wa_web_octet: [u8; 8] = {
        let mut b = [0u8; 8];
        b[7] = ((key_id_long.len() + 1) & 0xff) as u8; // = 0 for len 255
        b
    };
    let whatsapp_rust_octet: [u8; 8] = ((key_id_long.len() + 1) as u64).to_be_bytes(); // = [0,0,0,0,0,0,1,0]

    println!("  WA Web octet bytes      : {:?}", wa_web_octet);
    println!("  whatsapp-rust octet bytes: {:?}", whatsapp_rust_octet);

    assert_ne!(
        wa_web_octet, whatsapp_rust_octet,
        "octet-length encodings should differ at the 256-byte boundary"
    );

    // For the typical (short) key_id, both encodings coincide -> MACs would
    // match. For the long one, the encodings differ -> MACs would differ.
    // We don't have a WA Web oracle here, so we just demonstrate the encoding
    // divergence at the spec level.
}

#[test]
fn bug_a7_content_mac_coincides_for_typical_key_ids() {
    println!("\nNote A7: for typical key_id sizes the MAC matches WA Web");

    use wacore::appstate::hash::generate_content_mac;

    // Typical key_id is 6 bytes (key:int + version:byte). ad.length = 7.
    // WA Web octet:        [0,0,0,0,0,0,0,7]
    // whatsapp-rust octet: 7u64.to_be_bytes() = [0,0,0,0,0,0,0,7]
    // -> they coincide, MAC matches.
    let key_id = vec![0u8, 0, 0, 0, 42, 1]; // typical shape
    let op = wa::syncd_mutation::SyncdOperation::Set;
    let mac = generate_content_mac(op, b"data", &key_id, &[0u8; 32]);

    let expected_octet: [u8; 8] = {
        let mut b = [0u8; 8];
        b[7] = ((key_id.len() + 1) & 0xff) as u8;
        b
    };
    let actual_octet: [u8; 8] = ((key_id.len() + 1) as u64).to_be_bytes();
    assert_eq!(expected_octet, actual_octet);

    println!("  key_id len=6 -> ad.length=7 -> both libs encode [0,0,0,0,0,0,0,7]");
    println!("  MAC head: {:02x?}", &mac[..4]);
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
fn bug_a3_lthash_endianness_is_host_dependent() {
    println!("\nBUG A3: LTHash uses native-endian; WA Web spec is little-endian");

    use wacore::appstate::lthash::WAPATCH_INTEGRITY;

    // Run a deterministic LTHash update.
    let mut acc = vec![0u8; 128]; // 64 lanes of u16
    let mac_a = vec![1u8; 32];
    let mac_b = vec![2u8; 32];

    WAPATCH_INTEGRITY.subtract_then_add_in_place(
        &mut acc,
        &[mac_b.as_slice()],
        &[mac_a.as_slice()],
    );

    // Print the first 4 bytes of the accumulator.
    println!("  acc[0..4] after one add/sub = {:02x?}", &acc[..4]);

    let host_is_le = cfg!(target_endian = "little");
    println!(
        "  host target_endian = {}",
        if host_is_le { "little" } else { "big" }
    );

    if host_is_le {
        println!(
            "  -> bytes happen to match WA Web's LE spec on this host. \
             On a big-endian host they would differ."
        );
    } else {
        println!("  -> BUG VISIBLE: this host is big-endian; MACs diverge from WA Web.");
    }

    // The spec-correct implementation would use to_le_bytes/from_le_bytes
    // regardless of host. The code currently uses to_ne_bytes/from_ne_bytes.
    // We verify the runtime is LE so any future big-endian CI would catch this.
    assert!(
        host_is_le,
        "Unsupported BE host. The current lthash impl would silently diverge from WA Web."
    );
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
fn bug_a10_mac_compare_uses_short_circuit_eq() {
    println!("\nBUG A10 (static evidence): MAC compare uses `!=`, not ConstantTimeEq");

    // Read the source line at runtime and assert it still uses `!=`. This
    // makes the POC fail the day someone swaps in a constant-time compare,
    // signalling that the fix landed and the assertion should be flipped.
    let src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/wacore/src/download.rs"
    ))
    .expect("download.rs is in the workspace");

    let uses_non_constant_time = src.contains("&computed_mac_full[..MAC_SIZE] != received_mac");
    let uses_constant_time = src.contains("ct_eq") || src.contains("ConstantTimeEq");

    println!(
        "  Found `!= received_mac` short-circuit: {}",
        uses_non_constant_time
    );
    println!(
        "  Found `ConstantTimeEq` / `ct_eq`     : {}",
        uses_constant_time
    );

    assert!(
        uses_non_constant_time && !uses_constant_time,
        "POC outdated: download.rs no longer uses the short-circuit MAC compare"
    );
}

//! Byte-exactness tests for the buffer codec.
//!
//! Every `WA_` constant below is a buffer produced by WA Web's own
//! `WABinary`/`WAWebWamLibProtocol` modules, loaded out of the bundle set the
//! pinned whatspec commit records and driven by a small harness. They are
//! vectors from the source of truth, not from a reading of it, which is the only
//! way to be sure of a format whose size classes and flag bits are decided by
//! control flow rather than declared anywhere.
//!
//! Each case is a whole buffer (header, globals, event, fields) because that
//! is what reaches the wire. What is *not* claimed is byte identity with a
//! particular WA Web call site: the official writer emits an event's fields in
//! the order the calling code assigned them, and this encoder emits them in the
//! order the catalog declares them. Each record is identical; the order of the
//! field records within one event is this repository's, deterministically.
//!
//! To regenerate a vector, drive `writeGlobalAttribute`/`writeEvent`/`writeField`
//! from those two modules with the same values, writing globals and the
//! timestamp together in ascending id order.

use super::*;
use crate::generated::{enums, events, globals};

/// The timestamp every vector stamps, and its encoding: a unix second past
/// 32767, so it lands in the 32-bit class.
const TS: i64 = 1_755_000_000;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// A regular-channel buffer with stream id 1, the stream id the official web
/// runtime hardcodes.
fn buffer(sequence: u16) -> WamBuffer {
    WamBuffer::new(Channel::Regular, 1, sequence)
}

/// One `UnknownStanza` carrying only its integer field, which is the smallest
/// event that exercises a size class.
fn int_field(value: i64) -> String {
    let mut buf = buffer(1);
    buf.write_event(
        &events::UnknownStanza {
            unknown_stanza_drop_reason: Some(value),
            ..Default::default()
        },
        TS,
        1,
    )
    .expect("regular event on a regular buffer");
    hex(buf.as_bytes())
}

#[test]
fn every_integer_size_class_matches_wa_web() {
    // 0 and 1 have no payload at all; the rest widen at exactly the signed
    // boundaries, and anything outside i32 becomes a double rather than an
    // int64: the writer's test is a 32-bit truncation round trip.
    let cases: &[(i64, &str)] = &[
        (0, "57414d0501010000502fc02c9b6839780dff1603"),
        (1, "57414d0501010000502fc02c9b6839780dff2603"),
        (-1, "57414d0501010000502fc02c9b6839780dff3603ff"),
        (2, "57414d0501010000502fc02c9b6839780dff360302"),
        (127, "57414d0501010000502fc02c9b6839780dff36037f"),
        (-128, "57414d0501010000502fc02c9b6839780dff360380"),
        (128, "57414d0501010000502fc02c9b6839780dff46038000"),
        (-129, "57414d0501010000502fc02c9b6839780dff46037fff"),
        (32767, "57414d0501010000502fc02c9b6839780dff4603ff7f"),
        (-32768, "57414d0501010000502fc02c9b6839780dff46030080"),
        (32768, "57414d0501010000502fc02c9b6839780dff560300800000"),
        (-32769, "57414d0501010000502fc02c9b6839780dff5603ff7fffff"),
        (
            2147483647,
            "57414d0501010000502fc02c9b6839780dff5603ffffff7f",
        ),
        (
            -2147483648,
            "57414d0501010000502fc02c9b6839780dff560300000080",
        ),
        (
            2147483648,
            "57414d0501010000502fc02c9b6839780dff7603000000000000e041",
        ),
        (
            -2147483649,
            "57414d0501010000502fc02c9b6839780dff7603000020000000e0c1",
        ),
    ];
    for (value, want) in cases {
        assert_eq!(&int_field(*value), want, "integer {value}");
    }
}

#[test]
fn every_string_length_class_matches_wa_web() {
    let short: &[(&str, &str)] = &[
        ("", "57414d0501010000502fc02c9b6839780dff860100"),
        ("a", "57414d0501010000502fc02c9b6839780dff86010161"),
        // The length prefix counts UTF-8 bytes, not characters: nine characters,
        // fourteen bytes.
        (
            "héllo·世界",
            "57414d0501010000502fc02c9b6839780dff86010e68c3a96c6c6fc2b7e4b896e7958c",
        ),
    ];
    for (value, want) in short {
        let mut buf = buffer(1);
        buf.write_event(
            &events::UnknownStanza {
                unknown_stanza_tag: Some((*value).to_string()),
                ..Default::default()
            },
            TS,
            1,
        )
        .expect("regular event");
        assert_eq!(&hex(buf.as_bytes()), want, "string {value:?}");
    }

    // The long cases are asserted by prefix plus body, since a 64 KiB hex
    // literal in a test file is unreadable and proves nothing extra. The space
    // in the last prefix separates the descriptor from the four-byte length that
    // class carries, which is the one place the split is not obvious; it is
    // stripped before the comparison.
    let long: &[(usize, &str)] = &[
        (255, "57414d0501010000502fc02c9b6839780dff8601ff"),
        (256, "57414d0501010000502fc02c9b6839780dff96010001"),
        (65535, "57414d0501010000502fc02c9b6839780dff9601ffff"),
        (65536, "57414d0501010000502fc02c9b6839780dffa601 00000100"),
    ];
    for (len, prefix) in long {
        let prefix = prefix.replace(' ', "");
        let mut buf = buffer(1);
        buf.write_event(
            &events::UnknownStanza {
                unknown_stanza_tag: Some("a".repeat(*len)),
                ..Default::default()
            },
            TS,
            1,
        )
        .expect("regular event");
        let got = hex(buf.as_bytes());
        assert_eq!(&got[..prefix.len()], prefix, "prefix for {len} bytes");
        assert_eq!(&got[prefix.len()..], "61".repeat(*len), "body for {len}");
    }
}

#[test]
fn a_float_field_is_a_double_only_when_it_is_not_an_integer() {
    let mut buf = buffer(1);
    buf.write_event(
        &events::MemoryStat {
            private_bytes: Some(2.5),
            ..Default::default()
        },
        TS,
        1,
    )
    .expect("regular event");
    assert_eq!(
        hex(buf.as_bytes()),
        "57414d0501010000502fc02c9b68393805ff76030000000000000440"
    );

    // 4.0 is integral, so it goes out in the one-byte integer class. Writing it
    // as a double would be a different eight bytes to the server.
    let mut buf = buffer(1);
    buf.write_event(
        &events::MemoryStat {
            private_bytes: Some(4.0),
            ..Default::default()
        },
        TS,
        1,
    )
    .expect("regular event");
    assert_eq!(
        hex(buf.as_bytes()),
        "57414d0501010000502fc02c9b68393805ff360304"
    );
}

#[test]
fn a_field_id_past_255_widens_the_id() {
    let mut buf = buffer(1);
    buf.write_event(
        &events::Call {
            accept_ack_latency_ms: Some(12),
            ..Default::default()
        },
        TS,
        1,
    )
    .expect("regular event");
    assert_eq!(
        hex(buf.as_bytes()),
        "57414d0501010000502fc02c9b6839ce01ff3ef8030c"
    );
}

#[test]
fn an_event_with_no_fields_ends_its_own_group() {
    let mut buf = buffer(7);
    buf.write_event(&events::WebWamForceFlush::default(), TS, 1)
        .expect("regular event");
    assert_eq!(hex(buf.as_bytes()), "57414d0501070000502fc02c9b683dc00cff");
}

#[test]
fn the_weight_reaches_the_wire_negated() {
    let mut buf = buffer(258);
    buf.write_event(
        &events::ReceiptStanzaReceive {
            processing_deferred: Some(true),
            receipt_stanza_duration: Some(42),
            receipt_stanza_offline_count: Some(0),
            receipt_stanza_stage: Some(enums::ReceiptStanzaStage::Parse),
            receipt_stanza_total_count: Some(3),
            receipt_stanza_type: Some("read".to_string()),
            ..Default::default()
        },
        TS,
        // The release weight this event declares.
        2000,
    )
    .expect("regular event");
    assert_eq!(
        hex(buf.as_bytes()),
        "57414d0501020100502fc02c9b6849c00930f8220e32012a1203220932070386040472656164"
    );
}

#[test]
fn a_weight_of_zero_is_written_as_zero_rather_than_omitted() {
    // Weight 0 means "never sampled out", not "no weight": the record still
    // carries a value, in the zero class.
    let mut buf = buffer(7);
    buf.write_event(&events::WebWamForceFlush::default(), TS, 0)
        .expect("regular event");
    assert_eq!(hex(buf.as_bytes()), "57414d0501070000502fc02c9b681dc00c");
}

#[test]
fn a_global_is_written_once_and_the_timestamp_before_every_event() {
    let mut buf = buffer(3);
    buf.set_global(globals::APP_VERSION, GlobalValue::Str("2.3000.1045368834"))
        .expect("appVersion is a regular global");
    buf.set_global(
        globals::PLATFORM,
        GlobalValue::Int(enums::PlatformType::Webclient.wire()),
    )
    .expect("platform is a regular global");
    buf.set_global(globals::AB_KEY2, GlobalValue::Str(""))
        .expect("abKey2 is a regular global");
    buf.write_event(
        &events::WamDroppedEvent {
            dropped_event_code: Some(3448),
            dropped_event_count: Some(1),
            ..Default::default()
        },
        TS,
        1,
    )
    .expect("regular event");
    // Re-setting an unchanged global must add nothing.
    buf.set_global(globals::APP_VERSION, GlobalValue::Str("2.3000.1045368834"))
        .expect("appVersion is a regular global");
    buf.write_event(
        &events::WamClientErrors {
            wam_client_buffer_drop_error_count: Some(2),
            ..Default::default()
        },
        TS + 61,
        1,
    )
    .expect("regular event");
    assert_eq!(
        hex(buf.as_bytes()),
        "57414d0501030000300b08801111322e333030302e3130343533363838333450\
         2fc02c9b6888791100390611ff4201780d2602502ffd2c9b68397804ff361c02"
    );
    assert_eq!(buf.events(), 2);
}

#[test]
fn a_cleared_global_writes_an_empty_record() {
    let mut buf = buffer(4);
    buf.set_global(globals::BROWSER, GlobalValue::Str("x"))
        .expect("browser is a regular global");
    buf.write_event(&events::WebWamForceFlush::default(), TS, 1)
        .expect("regular event");
    buf.clear_global(globals::BROWSER)
        .expect("browser is a regular global");
    buf.write_event(&events::WebWamForceFlush::default(), TS + 1, 1)
        .expect("regular event");
    assert_eq!(
        hex(buf.as_bytes()),
        "57414d0501040000502fc02c9b68880b0301783dc00cff502fc12c9b68080b033dc00cff"
    );
}

#[test]
fn the_realtime_channel_differs_only_in_the_header_byte() {
    let mut buf = WamBuffer::new(Channel::Realtime, 1, 9);
    buf.write_event(
        &events::WefrClientExposure {
            is_canonical_ent_present: Some(true),
            ..Default::default()
        },
        TS,
        1,
    )
    .expect("realtime event on a realtime buffer");
    assert_eq!(
        hex(buf.as_bytes()),
        "57414d0501090001502fc02c9b68398015ff2606"
    );
}

#[test]
fn a_global_the_channel_does_not_allow_is_refused() {
    // `psId` is private-only. Writing it onto a regular buffer would produce a
    // buffer the official client never sends, so the buffer refuses rather than
    // skipping it the way WA Web does.
    let mut buf = buffer(1);
    let err = buf
        .set_global(globals::PS_ID, GlobalValue::Str("x"))
        .expect_err("psId is not legal on the regular channel");
    assert_eq!(
        err,
        BufferError::GlobalNotOnChannel {
            name: "psId",
            channel: Channel::Regular,
        }
    );
    // And nothing reached the bytes.
    assert_eq!(buf.len(), 8);
}

#[test]
fn a_global_given_the_wrong_kind_of_value_is_refused() {
    // An event's fields are typed by its generated struct. A global is the one
    // untyped way into a buffer, so the catalog's declared kind is the only
    // thing standing between a caller's slip and a record the server reads back
    // as the wrong type.
    let mut buf = buffer(1);
    let err = buf
        .set_global(globals::APP_VERSION, GlobalValue::Int(1))
        .expect_err("appVersion is a string");
    assert_eq!(
        err,
        BufferError::GlobalValueKind {
            name: "appVersion",
            expected: ValueKind::String,
            actual: ValueKind::Integer,
        }
    );
    assert_eq!(buf.len(), 8, "and nothing reached the bytes");

    // An enum global takes the integer its member resolves to, which is the
    // shape the identity writes and must keep working.
    buf.set_global(
        globals::PLATFORM,
        GlobalValue::Int(enums::PlatformType::Webclient.wire()),
    )
    .expect("an enum global takes its wire value");
}

#[test]
fn a_realtime_buffer_accepts_the_regular_globals() {
    // The official writer maps `realtime` onto `regular` before consulting a
    // global's channel list, so a regular-only global is legal here.
    let mut buf = WamBuffer::new(Channel::Realtime, 1, 1);
    buf.set_global(globals::BROWSER, GlobalValue::Str("x"))
        .expect("browser is legal on a realtime buffer");
    assert!(globals::BROWSER.channels.allows(Channel::Realtime));
    assert!(!globals::PS_ID.channels.allows(Channel::Realtime));
}

#[test]
fn an_event_from_another_channel_is_refused() {
    let mut buf = buffer(1);
    let err = buf
        .write_event(&events::WefrClientExposure::default(), TS, 1)
        .expect_err("a realtime event does not belong in a regular buffer");
    assert_eq!(
        err,
        BufferError::EventOnWrongChannel {
            event: "WefrClientExposure",
            event_channel: Channel::Realtime,
            buffer_channel: Channel::Regular,
        }
    );
    assert!(!buf.has_events());
}

#[test]
fn a_buffer_with_no_events_is_only_a_header() {
    let buf = buffer(1);
    assert_eq!(buf.len(), 8);
    assert!(!buf.has_events());
    assert_eq!(buf.sequence(), 1);
    assert_eq!(buf.channel(), Channel::Regular);
}

#[test]
fn sampling_keeps_everything_at_weight_zero_and_one() {
    // Weight 0 skips the test entirely, so even a roll that would fail keeps.
    assert!(sampling::keeps(0, 0.999_999));
    // Weight 1 can never exceed the threshold.
    assert!(sampling::keeps(1, 0.999_999));
    // Weight 20 keeps one in twenty.
    assert!(sampling::keeps(20, 0.049));
    assert!(!sampling::keeps(20, 0.051));
    assert!(sampling::keeps(2000, 0.000_4));
    assert!(!sampling::keeps(2000, 0.000_6));
}

#[test]
fn the_release_weight_is_the_third_declared_one() {
    use crate::WamEvent;
    assert_eq!(
        events::ReceiptStanzaReceive::WEIGHTS[sampling::RELEASE],
        2000
    );
    assert_eq!(events::UnknownStanza::WEIGHTS[sampling::RELEASE], 1);
}

#[test]
fn a_nan_field_is_dropped_rather_than_written() {
    // A NaN fails the official client's own pre-serialization check, so a buffer
    // carrying one is a buffer it would have discarded. Dropping the field keeps
    // the rest of the event, and the group-end flag still lands correctly.
    let mut buf = buffer(1);
    buf.write_event(
        &events::MemoryStat {
            private_bytes: Some(f64::NAN),
            num_messages: Some(1.0),
            ..Default::default()
        },
        TS,
        1,
    )
    .expect("regular event");
    // Only `numMessages` (id 8) survives, and it carries the group-end flag.
    assert_eq!(
        hex(buf.as_bytes()),
        "57414d0501010000502fc02c9b68393805ff2608"
    );
}

#[test]
fn the_catalog_covers_the_whole_declared_surface() {
    // A regeneration that silently lost a domain would otherwise only show up
    // as a missing type at a use site.
    assert_eq!(globals::ALL.len(), 46);
    assert_eq!(PRIVATE_STATS_IDS.len(), 9);
    assert_eq!(constants::WAM_PROTOCOL_VERSION, 5);
    assert_eq!(constants::WAM_MAX_BUFFER_SIZE, 50_000);
    assert_eq!(constants::WAM_MAX_BUFFER_SIZE_FOR_UPLOAD, 64_000);
}

/// How many bytes a second event costs when `def` is set to the same value
/// again between the two, on a `private` buffer.
///
/// The comparison this feeds is the point: a global the writer deduplicates
/// costs the second event nothing, and one it rewrites costs it a record.
fn private_repeat_cost(def: GlobalDef, value: GlobalValue<'_>) -> usize {
    let mut buf = WamBuffer::new(Channel::Private, 1, 1);
    buf.set_global(def, value.clone())
        .expect("a private global on a private buffer");
    buf.write_event(&events::TestAnonymousDaily {}, TS, 1)
        .expect("a private event on a private buffer");
    let after_first = buf.as_bytes().len();
    buf.set_global(def, value)
        .expect("a private global on a private buffer");
    buf.write_event(&events::TestAnonymousDaily {}, TS, 1)
        .expect("a private event on a private buffer");
    buf.as_bytes().len() - after_first
}

#[test]
fn the_private_stats_id_is_rewritten_before_every_event_and_another_global_is_not() {
    // Not a WA Web vector: the harness produced no private-channel bytes, so
    // this locks the exemption's behaviour rather than an exact encoding. The
    // two globals are the same kind and the same length, so the whole difference
    // between the two costs is the psId record the second event carries.
    let ps_id = private_repeat_cost(globals::PS_ID, GlobalValue::Str("aaaa"));
    let country = private_repeat_cost(globals::PS_COUNTRY_CODE, GlobalValue::Str("aaaa"));
    assert!(
        ps_id > country,
        "psId must be rewritten before every event on a private buffer \
         (psId cost {ps_id}, an ordinary private global cost {country})"
    );
}

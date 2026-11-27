use waproto::protobuf::{
    DecodeError, MessageView as _,
    encoding::{put_bytes_field, put_varint},
};
use waproto::whatsapp::{MediaRetryNotificationView, media_retry_notification::ResultType};

fn canonical(id: &[u8], path: &[u8]) -> Vec<u8> {
    let mut wire = Vec::new();
    put_bytes_field(1, id, &mut wire);
    put_bytes_field(2, path, &mut wire);
    put_varint(3 << 3, &mut wire);
    put_varint(1, &mut wire);
    wire
}

fn unknown_after_id(id: &[u8], path: &[u8]) -> Vec<u8> {
    let mut wire = Vec::new();
    put_bytes_field(1, id, &mut wire);
    put_bytes_field(50_000, &[], &mut wire);
    put_bytes_field(2, path, &mut wire);
    put_varint(3 << 3, &mut wire);
    put_varint(1, &mut wire);
    wire
}

fn assert_projection(wire: &[u8], expected: Option<(&str, &str)>) {
    let projected = waproto::codec::media_retry_success(wire);
    assert_eq!(projected, expected, "wire length {}", wire.len());
    if let Some((id, path)) = projected {
        let view = MediaRetryNotificationView::decode_view(wire).unwrap();
        assert_eq!(view.stanza_id, Some(id));
        assert_eq!(view.direct_path, Some(path));
        assert_eq!(view.result, Some(ResultType::SUCCESS));
    }
}

#[test]
fn projection_preserves_short_and_message_length_boundaries() {
    for (id_len, path_len, accepted) in [
        (0, 0, true),
        (1, 127, true),
        (1, 128, false),
        (127, 0, true),
        (128, 0, false),
        (127, 122, true),
        (127, 123, true),
        (127, 127, true),
    ] {
        let id = "i".repeat(id_len);
        let path = "p".repeat(path_len);
        let wire = canonical(id.as_bytes(), path.as_bytes());
        if id_len == 127 && path_len == 122 {
            assert_eq!(wire.len(), 255);
        }
        if id_len == 127 && path_len == 123 {
            assert_eq!(wire.len(), 256);
        }
        if id_len == 127 && path_len == 127 {
            assert_eq!(wire.len(), 260);
        }
        let expected = accepted.then_some((id.as_str(), path.as_str()));
        assert_projection(&wire, expected);
        assert!(MediaRetryNotificationView::decode_view(&wire).is_ok());
    }
}

#[test]
fn projection_bounds_unknowns_and_preserves_decoder_fallback() {
    let id = "i".repeat(123);
    let path = "p".repeat(122);
    let short = unknown_after_id(id.as_bytes(), path.as_bytes());
    assert_eq!(short.len(), 255);
    assert_projection(&short, Some((&id, &path)));

    let longer_id = "i".repeat(124);
    let long = unknown_after_id(longer_id.as_bytes(), path.as_bytes());
    assert_eq!(long.len(), 256);
    assert_projection(&long, None);
    assert!(MediaRetryNotificationView::decode_view(&long).is_ok());

    let mut unknown_short_tag = Vec::new();
    put_bytes_field(1, b"old", &mut unknown_short_tag);
    put_bytes_field(5, b"unknown", &mut unknown_short_tag);
    put_bytes_field(2, b"/path", &mut unknown_short_tag);
    unknown_short_tag.extend_from_slice(&[0x18, 1]);
    assert_projection(&unknown_short_tag, None);
    assert!(MediaRetryNotificationView::decode_view(&unknown_short_tag).is_ok());

    let mut unknown_binary = Vec::new();
    put_bytes_field(1, b"id", &mut unknown_binary);
    put_bytes_field(50_000, &[0xff], &mut unknown_binary);
    put_bytes_field(2, b"/path", &mut unknown_binary);
    unknown_binary.extend_from_slice(&[0x18, 1]);
    assert_projection(&unknown_binary, Some(("id", "/path")));
}

#[test]
fn projection_falls_back_for_duplicates_invalid_utf8_and_wrong_wire() {
    let mut duplicate = Vec::new();
    put_bytes_field(1, b"old", &mut duplicate);
    put_bytes_field(1, b"new", &mut duplicate);
    put_bytes_field(2, b"/path", &mut duplicate);
    duplicate.extend_from_slice(&[0x18, 1]);
    assert_projection(&duplicate, None);
    assert_eq!(
        MediaRetryNotificationView::decode_view(&duplicate)
            .unwrap()
            .stanza_id,
        Some("new")
    );

    let invalid_utf8 = canonical(&[0xff], b"/path");
    assert_projection(&invalid_utf8, None);
    assert_eq!(
        MediaRetryNotificationView::decode_view(&invalid_utf8).unwrap_err(),
        DecodeError::InvalidUtf8
    );

    let mut wrong_wire = vec![0x08, 1];
    put_bytes_field(2, b"/path", &mut wrong_wire);
    wrong_wire.extend_from_slice(&[0x18, 1]);
    assert_projection(&wrong_wire, None);
    assert!(matches!(
        MediaRetryNotificationView::decode_view(&wrong_wire),
        Err(DecodeError::WireTypeMismatch { .. })
    ));
}

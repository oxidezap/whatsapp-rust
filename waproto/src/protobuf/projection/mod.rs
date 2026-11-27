//! Small projections used by generated view decoding and media retry.

use crate::tags::media_retry_notification as tag;
use crate::whatsapp::media_retry_notification::ResultType;
use core::mem::size_of;

mod media_short;

const ID: u8 = ((tag::STANZA_ID << 3) | 2) as u8;
const PATH: u8 = ((tag::DIRECT_PATH << 3) | 2) as u8;
const RESULT: u8 = (tag::RESULT << 3) as u8;
const SUCCESS: u8 = ResultType::SUCCESS as u8;

const _: () = {
    assert!(tag::STANZA_ID < 16);
    assert!(tag::DIRECT_PATH < 16);
    assert!(tag::RESULT < 16);
    assert!(ResultType::SUCCESS as i32 >= 0 && ResultType::SUCCESS as i32 <= 127);
    assert!(2 * 127 + 6 <= super::MAX_MESSAGE_BYTES as usize);
    assert!(255 / 2 <= super::DEFAULT_UNKNOWN_FIELD_LIMIT);
    assert!(255 / 2 <= super::DEFAULT_ELEMENT_MEMORY_LIMIT / size_of::<u32>());
    assert!(255 / 2 * size_of::<(usize, usize)>() + 64 * size_of::<usize>() <= 16 * 1024 * 1024);
};

pub(crate) fn media_retry_success(input: &[u8]) -> Option<(&str, &str)> {
    if let Some(success) = parse_media_retry_success(input) {
        return Some(success);
    }
    media_short::project_short_media_success(input)
}

fn parse_media_retry_success(input: &[u8]) -> Option<(&str, &str)> {
    let body = input.strip_suffix(&[RESULT, SUCCESS])?;
    let [ID, id_len @ 0..=127, rest @ ..] = body else {
        return None;
    };
    let (id, rest) = rest.split_at_checked(usize::from(*id_len))?;
    let [PATH, path_len @ 0..=127, path @ ..] = rest else {
        return None;
    };
    if usize::from(*path_len) != path.len() {
        return None;
    }
    // The one-byte tags and lengths are ASCII, so no UTF-8 sequence can span a field header.
    let text = core::str::from_utf8(body).ok()?;
    let id_end = 2 + id.len();
    Some((text.get(2..id_end)?, text.get(id_end + 2..)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protobuf::MessageView as _;

    #[test]
    fn media_success_projection_agrees_with_generated_decoder() {
        let cases: &[&[u8]] = &[
            b"\x0a\x02id\x12\x05/path\x18\x01",
            b"\x12\x05/path\x0a\x02id\x18\x01",
            b"\x0a\x02id\xa2\x06\x01x\x12\x05/path\x18\x01",
            b"\x0a\x01\xff\x12\x05/path\x18\x01",
            b"\x0a\x02id\x0a\x03new\x12\x05/path\x18\x01",
            b"\x0a\x02id\x12\x05/path\x18\x02",
        ];
        let mut accepted = 0;
        for input in cases {
            if let Some((id, path)) = media_retry_success(input) {
                accepted += 1;
                let reference =
                    crate::whatsapp::MediaRetryNotificationView::decode_view(input).unwrap();
                assert_eq!(reference.stanza_id, Some(id));
                assert_eq!(reference.direct_path, Some(path));
                assert_eq!(reference.result, Some(ResultType::SUCCESS));
            }
        }
        assert!(accepted >= 2);
    }

    #[test]
    fn media_projection_matches_on_text_and_length_boundaries() {
        use crate::protobuf::encoding::encode_varint;
        let strings = [
            String::new(),
            "a".into(),
            "á🙂".into(),
            "x".repeat(127),
            "x".repeat(128),
        ];
        for id in &strings {
            for path in &strings {
                let mut input = Vec::new();
                for (tag, value) in [(0x0a, id), (0x12, path)] {
                    input.push(tag);
                    encode_varint(value.len() as u64, &mut input);
                    input.extend_from_slice(value.as_bytes());
                }
                input.extend_from_slice(&[0x18, 1]);
                if let Some((actual_id, actual_path)) = media_retry_success(&input) {
                    let reference =
                        crate::whatsapp::MediaRetryNotificationView::decode_view(&input).unwrap();
                    assert_eq!(reference.stanza_id, Some(actual_id));
                    assert_eq!(reference.direct_path, Some(actual_path));
                    assert_eq!(reference.result, Some(ResultType::SUCCESS));
                }
            }
        }
    }

    #[test]
    fn media_projection_accepts_only_inputs_the_generated_view_accepts() {
        let original = b"\x0a\x02id\x12\x05/path\x18\x01";
        let mut state = 0xdad1_4e73_02c9_4c51u64;
        let mut accepted = 0;
        for iteration in 0..20_000 {
            let mut input = original.to_vec();
            for _ in 0..iteration % 4 {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                let position = state as usize % input.len();
                input[position] = (state >> 32) as u8;
            }
            if let Some((id, path)) = media_retry_success(&input) {
                accepted += 1;
                let reference =
                    crate::whatsapp::MediaRetryNotificationView::decode_view(&input).unwrap();
                assert_eq!(reference.stanza_id, Some(id));
                assert_eq!(reference.direct_path, Some(path));
                assert_eq!(reference.result, Some(ResultType::SUCCESS));
            }
        }
        assert!(accepted > 5_000);
    }
}

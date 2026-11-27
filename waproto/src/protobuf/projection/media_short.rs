use super::{ID, PATH, RESULT, SUCCESS};
use crate::protobuf::indexed_wire;
use crate::tags::media_retry_notification as tag;

fn short_media_header_can_project(input: &[u8]) -> bool {
    let next = match input {
        [ID, length @ 0..=127, rest @ ..] => rest.get(usize::from(*length)),
        _ => input.first(),
    };
    matches!(next.copied(), Some(PATH | RESULT | 128..=255))
}

#[inline(never)]
fn checked_text(bytes: &[u8]) -> Option<&str> {
    core::str::from_utf8(bytes).ok()
}

#[cold]
#[inline(never)]
fn parse_short_media_retry_fields(mut input: &[u8]) -> Option<(&str, &str)> {
    let (mut id, mut path, mut success) = (None, None, false);
    while !input.is_empty() {
        match input {
            [RESULT, SUCCESS, tail @ ..] if !success => {
                success = true;
                input = tail;
            }
            [field @ (ID | PATH), len @ 0..=127, bytes @ ..] => {
                let value = if *field == ID { &mut id } else { &mut path };
                if value.is_some() {
                    return None;
                }
                let (bytes, tail) = bytes.split_at_checked(usize::from(*len))?;
                *value = Some(bytes);
                input = tail;
            }
            _ => {
                let width = indexed_wire::short_unknown_delimited_len(input, tag::MESSAGE_SECRET)?;
                input = input.get(width..)?;
            }
        }
    }
    if !success {
        return None;
    }
    Some((checked_text(id?)?, checked_text(path?)?))
}

#[cold]
#[inline(never)]
pub(super) fn project_short_media_success(input: &[u8]) -> Option<(&str, &str)> {
    if input.len() > usize::from(u8::MAX) || !short_media_header_can_project(input) {
        return None;
    }
    parse_short_media_retry_fields(input)
}

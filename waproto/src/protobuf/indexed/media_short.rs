use super::{
    InlineMediaSuccess, Limits, MediaRetryNotification, Parts, Schema, View, checked_text,
    media_retry_notification, protobuf,
};

pub(super) fn short_media_header_can_project(input: &[u8]) -> bool {
    const ID: u8 = (MediaRetryNotification::STANZA_ID.spec.number() * 8 + 2) as u8;
    const PATH: u8 = (MediaRetryNotification::DIRECT_PATH.spec.number() * 8 + 2) as u8;
    const RESULT: u8 = (MediaRetryNotification::RESULT.spec.number() * 8) as u8;
    let next = match input {
        [ID, length @ 0..=127, rest @ ..] => rest.get(usize::from(*length)),
        _ => input.first(),
    };
    matches!(next.copied(), Some(PATH | RESULT | 128..=255))
}

#[cold]
#[inline(never)]
pub(super) fn parse_short_media_retry_fields<'a>(
    mut input: &'a [u8],
    schema: &Schema,
) -> Option<(InlineMediaSuccess<'a>, usize)> {
    const ID: u8 = (MediaRetryNotification::STANZA_ID.spec.number() * 8 + 2) as u8;
    const PATH: u8 = (MediaRetryNotification::DIRECT_PATH.spec.number() * 8 + 2) as u8;
    const RESULT: u8 = (MediaRetryNotification::RESULT.spec.number() * 8) as u8;
    const SUCCESS: u8 = media_retry_notification::ResultType::SUCCESS.number() as u8;
    let cutoff = schema.fields.last()?.number();
    let (mut id, mut path, mut success, mut count) = (None, None, false, 0);
    while !input.is_empty() {
        count += 1;
        match input {
            [RESULT, SUCCESS, tail @ ..] if !success => {
                success = true;
                input = tail;
            }
            [tag @ (ID | PATH), len @ 0..=127, bytes @ ..] => {
                let field = if *tag == ID { &mut id } else { &mut path };
                if field.is_some() {
                    return None;
                }
                let (bytes, tail) = bytes.split_at_checked(usize::from(*len))?;
                *field = Some(bytes);
                input = tail;
            }
            _ => {
                let width = protobuf::short_unknown_delimited_len(input, cutoff)?;
                input = input.get(width..)?;
            }
        }
    }
    if !success {
        return None;
    }
    let id = checked_text(id?).ok()?;
    let path = checked_text(path?).ok()?;
    Some((InlineMediaSuccess { id, path }, count))
}

#[cold]
#[inline(never)]
pub(super) fn project_short_media_success(input: &[u8], limits: Limits) -> Option<(&str, &str)> {
    if input.len() > usize::from(u8::MAX)
        || input.len() > limits.max_bytes
        || limits.max_values < InlineMediaSuccess::COUNT
        || limits.max_fragments == 0
        || !short_media_header_can_project(input)
    {
        return None;
    }
    let schema = &MediaRetryNotification::SCHEMA;
    let (success, count) = parse_short_media_retry_fields(input, schema)?;
    if count > limits.max_fields {
        return None;
    }
    View::index_charge(&Parts::one(input), schema, limits, count).ok()?;
    Some((success.id, success.path))
}

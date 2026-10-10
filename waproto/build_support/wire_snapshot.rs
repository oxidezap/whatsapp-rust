//! Bounded snapshot of the descriptor-guarded Header media projection.
use ::buffa::alloc::vec::Vec;
use ::buffa::{DecodeContext, DecodeError};

pub(crate) enum Kind { Bytes, Message(u16) }
pub(crate) struct Field { pub(crate) number: u32, pub(crate) kind: Kind }
pub(crate) type Schema = &'static [&'static [Field]];
pub(crate) enum ValueRef<'a> { Bytes(&'a [u8]), Child(&'a dyn Visitor) }
pub(crate) trait Visitor {
    fn supported(&self) -> bool;
    fn field(&self, number: u32, index: usize) -> Option<ValueRef<'_>>;
}

fn sizes(schema: usize, current: &dyn Visitor, plan: &mut [usize; 4]) -> Option<usize> {
    if !current.supported() { return None; }
    let mut total = 0usize;
    for field in SCHEMA[schema] {
        let Some(value) = current.field(field.number, 0) else { continue; };
        let length = match (&field.kind, value) {
            (Kind::Bytes, ValueRef::Bytes(bytes)) => bytes.len(),
            (Kind::Message(child), ValueRef::Child(value)) => sizes(usize::from(*child), value, plan)?,
            _ => return None,
        };
        // Leave shapes beyond the original protobuf size limit
        // to that codec before allocating or debiting a caller allowance.
        if length > ::buffa::MAX_MESSAGE_BYTES as usize { return None; }
        total = total.checked_add(::buffa::encoding::varint_len(u64::from(field.number) << 3 | 2))?
            .checked_add(::buffa::encoding::varint_len(length as u64))?.checked_add(length)?;
    }
    if total > ::buffa::MAX_MESSAGE_BYTES as usize { return None; }
    plan[schema] = total;
    Some(total)
}

fn write(schema: usize, current: &dyn Visitor, plan: &[usize; 4], output: &mut Vec<u8>) {
    for field in SCHEMA[schema] {
        let Some(value) = current.field(field.number, 0) else { continue; };
        match (&field.kind, value) {
            (Kind::Bytes, ValueRef::Bytes(bytes)) => ::buffa::types::put_bytes_field(field.number, bytes, output),
            (Kind::Message(child), ValueRef::Child(value)) => {
                ::buffa::encoding::encode_varint(u64::from(field.number) << 3 | 2, output);
                ::buffa::encoding::encode_varint(plan[usize::from(*child)] as u64, output);
                write(usize::from(*child), value, plan, output);
            }
            _ => unreachable!("validated snapshot shape"),
        }
    }
}

#[cold]
#[inline(never)]
pub(crate) fn snapshot(current: &dyn Visitor, ctx: Option<DecodeContext<'_>>) -> Result<Option<Vec<u8>>, DecodeError> {
    let mut plan = [0; 4];
    let Some(size) = sizes(0, current, &mut plan) else { return Ok(None); };
    // Snapshot accepted fields without spending incoming recursion depth.
    // Charge the same final wire allocation once, before allocating.
    if let Some(ctx) = ctx { ctx.register_element_memory(size)?; }
    let mut bytes = Vec::with_capacity(size);
    write(0, current, &plan, &mut bytes);
    Ok(Some(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ::buffa::Message as _;
    use crate::whatsapp as wa;
    use wa::message::interactive_message::{Header, header::Media};
    use ::buffa::alloc::{vec, string::String};

    fn nested(length: usize, present: bool) -> Header {
        let quote = wa::Message {
            conversation: present.then(|| String::from("q").repeat(length)),
            ..Default::default()
        };
        let context = wa::ContextInfo { quoted_message: quote.into(), ..Default::default() };
        let image = wa::message::ImageMessage {
            caption: present.then(|| String::from("c").repeat(length)),
            jpeg_thumbnail: present.then(|| vec![42; length]),
            context_info: context.into(),
            ..Default::default()
        };
        Header { media: Some(Media::ImageMessage(Box::new(image))), ..Default::default() }
    }

    #[test]
    fn snapshot_matches_original_codec_presence_and_length_boundaries() {
        for length in [0, 1, 127, 128, 255, 16_383, 16_384] {
            for present in [false, true] {
                let header = nested(length, present);
                assert_eq!(snapshot(&header, None).unwrap().unwrap(), header.encode_to_vec());
            }
            let header = Header { media: Some(Media::JpegThumbnail(vec![42; length])), ..Default::default() };
            assert_eq!(snapshot(&header, None).unwrap().unwrap(), header.encode_to_vec());
        }
        assert_eq!(snapshot(&Header::default(), None).unwrap(), Some(Vec::new()));
    }

    #[test]
    fn snapshot_preserves_exact_wire_allocation_debit_without_incoming_depth() {
        let header = nested(128, true);
        let expected = header.encode_to_vec();
        let unknown = ::core::cell::Cell::new(0);
        for extra in [0, 1] {
            let allowance = ::core::cell::Cell::new(expected.len() + extra);
            let ctx = DecodeContext::new(0, &unknown).with_element_memory(&allowance);
            assert_eq!(snapshot(&header, Some(ctx)).unwrap().unwrap(), expected);
            assert_eq!(allowance.get(), extra);
        }
        let allowance = ::core::cell::Cell::new(expected.len() - 1);
        let ctx = DecodeContext::new(0, &unknown).with_element_memory(&allowance);
        assert!(matches!(snapshot(&header, Some(ctx)), Err(DecodeError::ElementMemoryLimitExceeded)));
        assert_eq!(allowance.get(), expected.len() - 1);
        assert_eq!(unknown.get(), 0);
    }

    #[test]
    fn snapshot_unsupported_child_falls_back_without_budget_debit() {
        let mut header = nested(1, true);
        let Some(Media::ImageMessage(image)) = &mut header.media else { panic!("image"); };
        image.url = Some(String::new());
        let allowance = ::core::cell::Cell::new(0);
        let unknown = ::core::cell::Cell::new(0);
        let ctx = DecodeContext::new(0, &unknown).with_element_memory(&allowance);
        assert!(snapshot(&header, Some(ctx)).unwrap().is_none());
        assert_eq!(allowance.get(), 0);
        assert_eq!(unknown.get(), 0);
    }

    #[test]
    fn snapshot_excludes_root_unknowns_but_rejects_child_unknowns_and_journals() {
        let mut wire = vec![0xb8, 0x0c, 1];
        wire.extend(nested(1, true).encode_to_vec());
        let mut header = Header::decode_from_slice(&wire).unwrap();
        header.title = Some("outside selected projection".into());
        let mut reference = header.clone();
        reference.__buffa_unknown_fields.clear();
        reference.title = None;
        assert_eq!(snapshot(&header, None).unwrap().unwrap(), reference.encode_to_vec());
        for child in [&[0xb8, 0x0c, 1][..], &[0xf8, 1, 99][..]] {
            let image = wa::message::ImageMessage::decode_from_slice(child).unwrap();
            let header = Header { media: Some(Media::ImageMessage(Box::new(image))), ..Default::default() };
            assert!(snapshot(&header, None).unwrap().is_none());
        }
    }
}

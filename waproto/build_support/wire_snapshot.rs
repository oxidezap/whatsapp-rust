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

// Only completed Known occurrences reach this predicate: the decoder has
// already validated strings. Prove the guarded singular-field projection is
// exactly the original writer's encoding, without visiting the receiver.
pub(crate) fn canonical(raw: &[u8]) -> bool {
    !raw.is_empty() && canonical_fields(0, raw)
}

fn canonical_fields(schema: usize, mut input: &[u8]) -> bool {
    if input.len() > ::buffa::MAX_MESSAGE_BYTES as usize { return false; }
    let mut previous = 0;
    let mut count = 0;
    while !input.is_empty() {
        let start = input.len();
        let Ok(tag) = ::buffa::encoding::decode_varint(&mut input) else { return false; };
        if tag & 7 != 2 || start - input.len() != ::buffa::encoding::varint_len(tag) { return false; }
        let Ok(number) = u32::try_from(tag >> 3) else { return false; };
        if number <= previous { return false; }
        let Some(field) = SCHEMA[schema].iter().find(|field| field.number == number) else { return false; };
        let start = input.len();
        let Ok(length) = ::buffa::encoding::decode_varint(&mut input) else { return false; };
        if start - input.len() != ::buffa::encoding::varint_len(length) { return false; }
        let Ok(length) = usize::try_from(length) else { return false; };
        let Some(bytes) = input.get(..length) else { return false; };
        if let Kind::Message(child) = field.kind
            && !canonical_fields(usize::from(child), bytes) { return false; }
        input = &input[length..];
        previous = number;
        count += 1;
    }
    // Header's supported fields are alternatives of one oneof. Descendants
    // contain ordinary singular fields and can represent an empty message.
    schema != 0 || count == 1
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
    use ::buffa::{Message as _, MessageView as _};
    use crate::whatsapp as wa;
    use wa::message::interactive_message::{Header, HeaderView, header::Media};
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
                let expected = header.encode_to_vec();
                let view = HeaderView::decode_view(&expected).unwrap();
                assert_eq!(snapshot(&header, None).unwrap().unwrap(), expected);
                assert!(canonical(&expected));
                assert_eq!(snapshot(&view, None).unwrap().unwrap(), expected);
            }
            let header = Header { media: Some(Media::JpegThumbnail(vec![42; length])), ..Default::default() };
            let expected = header.encode_to_vec();
            let view = HeaderView::decode_view(&expected).unwrap();
            assert_eq!(snapshot(&header, None).unwrap().unwrap(), expected);
            assert!(canonical(&expected));
            assert_eq!(snapshot(&view, None).unwrap().unwrap(), expected);
        }
        assert_eq!(snapshot(&Header::default(), None).unwrap(), Some(Vec::new()));
        assert_eq!(snapshot(&HeaderView::decode_view(&[]).unwrap(), None).unwrap(), Some(Vec::new()));
    }

    #[test]
    fn canonical_projection_rejects_noncanonical_and_unsupported_occurrences() {
        for raw in [
            &[0xa2, 0x00, 0][..], // Overlong Header tag.
            &[0x22, 0x80, 0][..], // Overlong empty image length.
            &[0x22, 0, 0x32, 0][..], // Two media alternatives.
            &[0x22, 4, 0x1a, 0, 0x1a, 0][..], // Duplicate caption.
            &[0x22, 5, 0x82, 1, 0, 0x1a, 0][..], // Reordered image fields.
            &[0x22, 2, 0x0a, 0][..], // Unsupported image URL.
            &[0x22, 3, 0xb8, 0x0c, 1][..], // Future nested field.
            &[0x22, 2, 0x1a][..], // Truncated image.
            &[][..],
        ] {
            assert!(!canonical(raw), "unexpected canonical projection: {raw:?}");
        }
        assert!(canonical(&[0x22, 0])); // Present empty image.
        assert!(canonical(&[0x22, 3, 0x8a, 1, 0])); // Present empty context.
    }

    #[test]
    fn canonical_completion_keeps_public_edits_and_failed_batch_retry() {
        let future = [0xc2, 0x3e, 4, 11, 22, 33, 44];
        let known = nested(1, true).encode_to_vec();
        let mut wire = future.to_vec();
        wire.extend_from_slice(&known);
        let mut header = Header::decode_from_slice(&wire).unwrap();
        assert_eq!(header.encode_to_vec(), wire);
        let mut edited = header.clone();
        let Some(Media::ImageMessage(image)) = &mut edited.media else { panic!("image"); };
        image.caption = Some("edited".into());
        let mut reference = edited.clone();
        reference.__buffa_unknown_fields.clear();
        let mut expected = future.to_vec();
        expected.extend(reference.encode_to_vec());
        assert_eq!(edited.encode_to_vec(), expected);
        assert_eq!(header.encode_to_vec(), wire);

        header.media = None;
        header.merge_from_slice(&known).unwrap();
        assert_eq!(header.encode_to_vec(), wire);
        let mut pending = Header::decode_from_slice(&future).unwrap();
        let mut failed = known.clone();
        failed.push(0x80);
        assert!(pending.merge_from_slice(&failed).is_err());
        assert_eq!(pending.encode_to_vec(), wire);
        pending.merge_from_slice(&[0x22, 0]).unwrap();
        wire.extend_from_slice(&[0x22, 0]);
        assert_eq!(pending.encode_to_vec(), wire);
    }

    #[test]
    fn snapshot_preserves_exact_wire_allocation_debit_without_incoming_depth() {
        let header = nested(128, true);
        let expected = header.encode_to_vec();
        let view = HeaderView::decode_view(&expected).unwrap();
        let unknown = ::core::cell::Cell::new(0);
        for current in [&header as &dyn Visitor, &view as &dyn Visitor] {
            for extra in [0, 1] {
                let allowance = ::core::cell::Cell::new(expected.len() + extra);
                let ctx = DecodeContext::new(0, &unknown).with_element_memory(&allowance);
                assert_eq!(snapshot(current, Some(ctx)).unwrap().unwrap(), expected);
                assert_eq!(allowance.get(), extra);
            }
            let allowance = ::core::cell::Cell::new(expected.len() - 1);
            let ctx = DecodeContext::new(0, &unknown).with_element_memory(&allowance);
            assert!(matches!(snapshot(current, Some(ctx)), Err(DecodeError::ElementMemoryLimitExceeded)));
            assert_eq!(allowance.get(), expected.len() - 1);
        }
        assert_eq!(unknown.get(), 0);
    }

    #[test]
    fn empty_projection_keeps_presence_and_exact_debits_for_owned_and_view() {
        use crate::whatsapp::__wire_order::{Adapter, OwnedCodec, ViewCodec};
        // Ordinary fields and root unknowns are outside the retained media
        // projection. Empty present alternatives still pay for their tags.
        for media in [&[][..], &[0x22, 0], &[0x32, 0], &[0x3a, 0]] {
            let mut wire = vec![0x0a, 1, b't', 0xb8, 0x0c, 1];
            wire.extend_from_slice(media);
            let mut header = Header::decode_from_slice(&wire).unwrap();
            let mut view = HeaderView::decode_view(&wire).unwrap();
            let unknown = ::core::cell::Cell::new(0);
            let allowance = ::core::cell::Cell::new(media.len());
            let ctx = DecodeContext::new(0, &unknown).with_element_memory(&allowance);
            assert_eq!(OwnedCodec::known(&Adapter(&mut header), ctx).unwrap(), media);
            assert_eq!(allowance.get(), 0);
            allowance.set(media.len());
            assert_eq!(ViewCodec::known(&Adapter(&mut view), ctx).unwrap(), media);
            assert_eq!(allowance.get(), 0);
            if !media.is_empty() {
                allowance.set(media.len() - 1);
                assert!(OwnedCodec::known(&Adapter(&mut header), ctx).is_err());
                assert_eq!(allowance.get(), media.len() - 1);
                assert!(ViewCodec::known(&Adapter(&mut view), ctx).is_err());
                assert_eq!(allowance.get(), media.len() - 1);
            }
            assert_eq!(unknown.get(), 0);
        }
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
        let encoded = header.encode_to_vec();
        let view = HeaderView::decode_view(&encoded).unwrap();
        assert!(snapshot(&view, Some(ctx)).unwrap().is_none());
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
        let mut view = HeaderView::decode_view(&wire).unwrap();
        view.title = Some("outside selected projection");
        assert_eq!(snapshot(&header, None).unwrap().unwrap(), reference.encode_to_vec());
        assert_eq!(snapshot(&view, None).unwrap().unwrap(), reference.encode_to_vec());
        for child in [&[0xb8, 0x0c, 1][..], &[0xf8, 1, 99][..]] {
            let image = wa::message::ImageMessage::decode_from_slice(child).unwrap();
            let header = Header { media: Some(Media::ImageMessage(Box::new(image))), ..Default::default() };
            assert!(snapshot(&header, None).unwrap().is_none());
            let encoded = header.encode_to_vec();
            let view = HeaderView::decode_view(&encoded).unwrap();
            assert!(snapshot(&view, None).unwrap().is_none());
        }
    }
}

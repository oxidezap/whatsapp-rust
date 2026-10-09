//! Conservative encoded-size growth for accepted noncanonical protobuf input.

use std::collections::BTreeMap;

use buffa_descriptor::generated::descriptor::{
    DescriptorProto, FieldDescriptorProto, FileDescriptorSet,
    field_descriptor_proto::{Label, Type},
};
use heck::ToSnakeCase as _;

pub type Bounds = BTreeMap<String, BTreeMap<u32, usize>>;

struct Message<'a> {
    rust: String,
    fields: &'a [FieldDescriptorProto],
}

pub fn bounds(fds: &FileDescriptorSet, package: &str) -> Bounds {
    fn collect<'a>(
        messages: &'a [DescriptorProto],
        proto: &str,
        rust: &str,
        output: &mut BTreeMap<String, Message<'a>>,
    ) {
        for message in messages {
            let name = message.name.as_deref().expect("descriptor message name");
            let proto_name = format!("{proto}.{name}");
            output.insert(
                proto_name.clone(),
                Message {
                    rust: format!("{rust}{name}"),
                    fields: &message.field,
                },
            );
            collect(
                &message.nested_type,
                &proto_name,
                &format!("{rust}{}::", name.to_snake_case()),
                output,
            );
        }
    }
    let mut messages = BTreeMap::new();
    for file in &fds.file {
        let name = file.package.as_deref().unwrap_or_default();
        collect(&file.message_type, &format!(".{name}"), "", &mut messages);
    }
    // A five-byte negative int32 can encode in ten bytes. Nested messages
    // inherit their largest field ratio: length prefixes cannot exceed that
    // ratio, so recursive schemas converge through a finite maximum.
    let mut ratios: BTreeMap<_, _> = messages.keys().map(|name| (name.clone(), 2usize)).collect();
    fn factor(field: &FieldDescriptorProto, ratios: &BTreeMap<String, usize>) -> usize {
        match field.r#type {
            Some(Type::TYPE_MESSAGE | Type::TYPE_GROUP) => *ratios
                .get(field.type_name.as_deref().expect("message field type"))
                .expect("referenced message descriptor"),
            Some(Type::TYPE_STRING | Type::TYPE_BYTES) => 1,
            _ if field.label == Some(Label::LABEL_REPEATED) => {
                // Even unpacked declarations accept packed input. Each one-byte
                // element can acquire a full tag; signed int32 also needs its
                // five-to-ten-byte expansion. This bounds packed declarations
                // too, since a future enum can force unpacked journal output.
                let number = field.number.expect("field number") as u32;
                let tag_len = buffa::encoding::varint_len(u64::from(number) << 3);
                (tag_len + 1).max(3)
            }
            _ => 2,
        }
    }
    loop {
        let next: BTreeMap<_, _> = messages
            .iter()
            .map(|(name, message)| {
                (
                    name.clone(),
                    message
                        .fields
                        .iter()
                        .map(|field| factor(field, &ratios))
                        .max()
                        .unwrap_or(2)
                        .max(2),
                )
            })
            .collect();
        if next == ratios {
            break;
        }
        ratios = next;
    }
    let prefix = format!(".{package}.");
    messages
        .into_iter()
        .filter(|(name, _)| name.starts_with(&prefix))
        .map(|(_, message)| {
            (
                message.rust,
                message
                    .fields
                    .iter()
                    .map(|field| {
                        (
                            field.number.expect("field number") as u32,
                            factor(field, &ratios),
                        )
                    })
                    .collect(),
            )
        })
        .collect()
}

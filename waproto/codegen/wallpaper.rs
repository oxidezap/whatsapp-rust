//! Retain the two historical string fields alongside the new fixed32 fields.
//! These are distinguished by (field number, wire type), never by conversion.
use crate::descriptor::{
    FieldDescriptorProto, FileDescriptorSet,
    field_descriptor_proto::{Label, Type},
};
use std::io;

pub fn retain_fields(fds: &mut FileDescriptorSet) -> io::Result<()> {
    let message = fds
        .file
        .iter_mut()
        .filter(|f| f.package.as_deref() == Some("whatsapp"))
        .flat_map(|f| f.message_type.iter_mut())
        .find(|m| m.name.as_deref() == Some("Message"))
        .and_then(|m| {
            m.nested_type
                .iter_mut()
                .find(|m| m.name.as_deref() == Some("ChatSolidColorWallpaper"))
        })
        .ok_or_else(|| io::Error::other("wallpaper compatibility target missing"))?;
    for (number, current, legacy) in [
        (1, "backgroundLightArgb", "colorLight"),
        (2, "backgroundDarkArgb", "colorDark"),
    ] {
        let fields: Vec<_> = message
            .field
            .iter()
            .filter(|f| f.number == Some(number))
            .collect();
        if fields.len() != 1
            || fields[0].name.as_deref() != Some(current)
            || fields[0].r#type != Some(Type::TYPE_FIXED32)
            || fields[0].label != Some(Label::LABEL_OPTIONAL)
            || message
                .field
                .iter()
                .any(|f| f.name.as_deref() == Some(legacy))
        {
            return Err(io::Error::other(
                "wallpaper wire-type compatibility requires source review",
            ));
        }
        message.field.push(FieldDescriptorProto {
            name: Some(legacy.into()),
            json_name: Some(legacy.into()),
            number: Some(number),
            r#type: Some(Type::TYPE_STRING),
            label: Some(Label::LABEL_OPTIONAL),
            ..Default::default()
        });
    }
    Ok(())
}

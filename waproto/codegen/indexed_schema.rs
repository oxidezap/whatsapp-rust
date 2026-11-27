#![forbid(unsafe_code)]

use super::schema::{
    Field, FieldShape, Message, MessageEncoding, Presence, Scalar, Schema, ValueType,
};
use std::fmt::Write as _;

fn kind(value: ValueType) -> &'static str {
    match value {
        ValueType::Message(_, MessageEncoding::Group) => "Group",
        ValueType::Message(_, MessageEncoding::LengthPrefixed) => "Message",
        ValueType::Enumeration(_) => "Enum",
        ValueType::Scalar(value) => match value {
            Scalar::Bool => "Bool",
            Scalar::Int32 => "Int32",
            Scalar::Int64 => "Int64",
            Scalar::Uint32 => "Uint32",
            Scalar::Uint64 => "Uint64",
            Scalar::Sint32 => "Sint32",
            Scalar::Sint64 => "Sint64",
            Scalar::Fixed32 => "Fixed32",
            Scalar::Fixed64 => "Fixed64",
            Scalar::Sfixed32 => "Sfixed32",
            Scalar::Sfixed64 => "Sfixed64",
            Scalar::Float => "Float",
            Scalar::Double => "Double",
            Scalar::String => "String",
            Scalar::Bytes => "Bytes",
        },
    }
}

fn field_spec(schema: &Schema, message: &Message, field: &Field) -> Result<String, String> {
    let (value, repeated, packed, required, group) = match field.shape {
        FieldShape::Singular { value, presence } => {
            (value, false, false, presence == Presence::Required, None)
        }
        FieldShape::Repeated { value, packed } => (value, true, packed, false, None),
        FieldShape::Map { entry, .. } => (
            ValueType::Message(entry, MessageEncoding::LengthPrefixed),
            true,
            false,
            false,
            None,
        ),
        FieldShape::Oneof { value, owner } => {
            let index = message
                .oneofs
                .iter()
                .position(|id| *id == owner)
                .ok_or_else(|| format!("missing oneof for {}.{}", message.full_name, field.name))?;
            let index = u16::try_from(index)
                .map_err(|_| format!("too many oneofs in {}", message.full_name))?;
            (value, false, false, false, Some(index))
        }
    };
    let (open, values) = if let ValueType::Enumeration(id) = value {
        let enumeration = schema.enumeration(id);
        let values = enumeration
            .values
            .iter()
            .map(|value| value.number.to_string())
            .collect::<Vec<_>>()
            .join(",");
        (enumeration.open, format!("&[{values}]"))
    } else {
        (false, "&[]".to_owned())
    };
    Ok(format!(
        "Spec::new({},Kind::{},FieldOptions{{repeated:{repeated},packed:{packed},required:{required},open:{open}}},{group:?},{values})",
        field.number.get(),
        kind(value)
    ))
}

fn map_default(schema: &Schema, field: &Field) -> Result<String, String> {
    let FieldShape::Singular { value, .. } = field.shape else {
        return Err(format!("invalid map entry field {}", field.name));
    };
    Ok(match value {
        ValueType::Message(..) | ValueType::Scalar(Scalar::String | Scalar::Bytes) => {
            "Value::Bytes(&[])".to_owned()
        }
        ValueType::Scalar(Scalar::Double | Scalar::Fixed64 | Scalar::Sfixed64) => {
            "Value::Fixed64(0)".to_owned()
        }
        ValueType::Scalar(Scalar::Float | Scalar::Fixed32 | Scalar::Sfixed32) => {
            "Value::Fixed32(0)".to_owned()
        }
        ValueType::Enumeration(id) => {
            let number = schema
                .enumeration(id)
                .values
                .first()
                .ok_or_else(|| format!("empty enum for {}", field.name))?
                .number;
            format!("Value::Varint({})", number as i64 as u64)
        }
        _ => "Value::Varint(0)".to_owned(),
    })
}

pub fn generate(schema: &Schema) -> Result<String, String> {
    let mut output = String::from("#[cfg(test)] pub(crate) static SCHEMAS: &[Schema] = &[\n");
    let mut media = None;
    for (index, message) in schema.messages.iter().enumerate() {
        let identity = u32::try_from(index).map_err(|_| "too many messages".to_owned())?;
        let mut fields: Vec<_> = message.fields.iter().map(|id| schema.field(*id)).collect();
        fields.sort_by_key(|field| field.number.get());
        let required_prefix = fields
            .iter()
            .rposition(|field| {
                matches!(
                    field.shape,
                    FieldShape::Singular {
                        presence: Presence::Required,
                        ..
                    }
                )
            })
            .map_or(0, |index| index + 1);
        if message.oneofs.len() > usize::from(u16::MAX) + 1 {
            return Err(format!("too many oneofs in {}", message.full_name));
        }
        let defaults = if message.map_entry {
            fields
                .iter()
                .enumerate()
                .map(|(index, field)| Ok(format!("({index},{})", map_default(schema, field)?)))
                .collect::<Result<Vec<_>, String>>()?
                .join(",")
        } else {
            String::new()
        };
        let mut declaration = String::new();
        write!(declaration, "Schema{{identity:SchemaIdentity({identity}),oneofs:{},required_prefix:{required_prefix},map_defaults:&[{defaults}],fields:&[", message.oneofs.len()).map_err(|error| error.to_string())?;
        for field in &fields {
            declaration.push_str(&field_spec(schema, message, field)?);
            declaration.push(',');
        }
        declaration.push_str("]}");
        if message.full_name == ".whatsapp.MediaRetryNotification" {
            let expected = [(1, "String"), (2, "String"), (3, "Enum"), (4, "Bytes")];
            if fields.len() != expected.len()
                || !message.oneofs.is_empty()
                || fields.iter().zip(expected).any(|(field, (number, expected_kind))| {
                    field.number.get() != number
                        || !matches!(field.shape, FieldShape::Singular { value, presence: Presence::Explicit } if kind(value) == expected_kind)
                })
            {
                return Err("media retry schema no longer matches the indexed reader".into());
            }
            let spec = |number| {
                let field = fields
                    .iter()
                    .find(|field| field.number.get() == number)
                    .ok_or_else(|| format!("missing media retry field {number}"))?;
                field_spec(schema, message, field)
            };
            media = Some((declaration.clone(), spec(1)?, spec(2)?, spec(3)?));
            output.push_str("MEDIA_RETRY_SCHEMA,\n");
        } else {
            output.push_str(&declaration);
            output.push_str(",\n");
        }
    }
    output.push_str("];\n");
    if let Some((declaration, stanza, path, result)) = media {
        write!(output, "const MEDIA_RETRY_SCHEMA: Schema = {declaration};\nconst MEDIA_STANZA: Spec = {stanza};\nconst MEDIA_PATH: Spec = {path};\nconst MEDIA_RESULT: Spec = {result};\n").map_err(|error| error.to_string())?;
    }
    Ok(output)
}

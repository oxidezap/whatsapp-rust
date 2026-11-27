#![forbid(unsafe_code)]

use super::schema::{FieldShape, Presence, Scalar, Schema, ValueType};

pub fn validate(schema: &Schema) -> Result<(), String> {
    let message = schema
        .messages
        .iter()
        .find(|message| message.full_name == ".whatsapp.MediaRetryNotification")
        .ok_or_else(|| "media retry schema is missing".to_owned())?;
    let mut fields: Vec<_> = message.fields.iter().map(|id| schema.field(*id)).collect();
    fields.sort_by_key(|field| field.number.get());
    let valid = fields.len() == 4
        && message.oneofs.is_empty()
        && fields.iter().enumerate().all(|(index, field)| {
            field.number.get() == index as u32 + 1
                && matches!(
                    (field.number.get(), field.shape),
                    (
                        1 | 2,
                        FieldShape::Singular {
                            value: ValueType::Scalar(Scalar::String),
                            presence: Presence::Explicit,
                        }
                    ) | (
                        3,
                        FieldShape::Singular {
                            value: ValueType::Enumeration(_),
                            presence: Presence::Explicit,
                        }
                    ) | (
                        4,
                        FieldShape::Singular {
                            value: ValueType::Scalar(Scalar::Bytes),
                            presence: Presence::Explicit,
                        }
                    )
                )
        });
    if !valid {
        return Err("media retry schema no longer matches the projection".to_owned());
    }
    Ok(())
}

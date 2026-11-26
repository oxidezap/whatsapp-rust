#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt;

use buffa_descriptor::features::{
    self, EnumType, FieldPresence, RepeatedFieldEncoding, ResolvedFeatures,
};
use buffa_descriptor::generated::descriptor::{
    DescriptorProto, EnumDescriptorProto, FieldDescriptorProto, FileDescriptorSet,
    field_descriptor_proto::{Label, Type},
};

use super::schema::*;

#[derive(Debug)]
pub enum ProjectionError {
    Missing(&'static str),
    DuplicateType(String),
    MissingType(String),
    WrongType(String),
    InvalidNumber(String, i32),
    InvalidOneof(String, i32),
    InvalidMap(String),
    UnsupportedPresence(String),
}

impl fmt::Display for ProjectionError {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, output)
    }
}

impl std::error::Error for ProjectionError {}

#[derive(Clone, Copy)]
enum Symbol {
    Message(MessageId),
    Enumeration(EnumId),
}

struct Pending<'a> {
    id: MessageId,
    descriptor: &'a DescriptorProto,
    features: ResolvedFeatures,
}

#[derive(Default)]
struct Builder<'a> {
    schema: Schema,
    symbols: BTreeMap<String, Symbol>,
    pending: Vec<Pending<'a>>,
}

fn qualified(scope: &str, name: &str) -> String {
    if scope.is_empty() {
        format!(".{name}")
    } else {
        format!("{scope}.{name}")
    }
}

impl<'a> Builder<'a> {
    fn insert(&mut self, name: String, symbol: Symbol) -> Result<(), ProjectionError> {
        if self.symbols.insert(name.clone(), symbol).is_some() {
            return Err(ProjectionError::DuplicateType(name));
        }
        Ok(())
    }

    fn enumerations(
        &mut self,
        declarations: &[EnumDescriptorProto],
        scope: &str,
        parent: Option<MessageId>,
        inherited: &ResolvedFeatures,
    ) -> Result<(), ProjectionError> {
        for declaration in declarations {
            let name = declaration
                .name
                .as_deref()
                .ok_or(ProjectionError::Missing("enum name"))?;
            let full_name = qualified(scope, name);
            let id = EnumId(self.schema.enumerations.len());
            self.insert(full_name.clone(), Symbol::Enumeration(id))?;
            let resolved = features::resolve_child(inherited, features::enum_features(declaration));
            self.schema.enumerations.push(Enumeration {
                open: resolved.enum_type == EnumType::Open
                    || crate::policy::OPEN_ENUMS.contains(&full_name.as_str()),
                full_name,
                name: name.into(),
                parent,
                values: declaration
                    .value
                    .iter()
                    .map(|value| {
                        Ok(EnumValue {
                            name: value
                                .name
                                .clone()
                                .ok_or(ProjectionError::Missing("enum value name"))?,
                            number: value
                                .number
                                .ok_or(ProjectionError::Missing("enum value number"))?,
                        })
                    })
                    .collect::<Result<_, ProjectionError>>()?,
            });
        }
        Ok(())
    }

    fn messages(
        &mut self,
        declarations: &'a [DescriptorProto],
        scope: &str,
        parent: Option<MessageId>,
        inherited: &ResolvedFeatures,
    ) -> Result<(), ProjectionError> {
        for declaration in declarations {
            let name = declaration
                .name
                .as_deref()
                .ok_or(ProjectionError::Missing("message name"))?;
            let full_name = qualified(scope, name);
            let id = MessageId(self.schema.messages.len());
            self.insert(full_name.clone(), Symbol::Message(id))?;
            let resolved =
                features::resolve_child(inherited, features::message_features(declaration));
            self.schema.messages.push(Message {
                full_name: full_name.clone(),
                name: name.into(),
                parent,
                fields: Vec::new(),
                oneofs: Vec::new(),
                map_entry: declaration
                    .options
                    .as_option()
                    .and_then(|options| options.map_entry)
                    == Some(true),
            });
            self.pending.push(Pending {
                id,
                descriptor: declaration,
                features: resolved,
            });
            self.enumerations(&declaration.enum_type, &full_name, Some(id), &resolved)?;
            self.messages(&declaration.nested_type, &full_name, Some(id), &resolved)?;
        }
        Ok(())
    }

    fn value_type(
        &self,
        field: &FieldDescriptorProto,
        inherited: &ResolvedFeatures,
    ) -> Result<ValueType, ProjectionError> {
        let scalar = match field.r#type.ok_or(ProjectionError::Missing("field type"))? {
            Type::TYPE_BOOL => Scalar::Bool,
            Type::TYPE_INT32 => Scalar::Int32,
            Type::TYPE_INT64 => Scalar::Int64,
            Type::TYPE_UINT32 => Scalar::Uint32,
            Type::TYPE_UINT64 => Scalar::Uint64,
            Type::TYPE_SINT32 => Scalar::Sint32,
            Type::TYPE_SINT64 => Scalar::Sint64,
            Type::TYPE_FIXED32 => Scalar::Fixed32,
            Type::TYPE_FIXED64 => Scalar::Fixed64,
            Type::TYPE_SFIXED32 => Scalar::Sfixed32,
            Type::TYPE_SFIXED64 => Scalar::Sfixed64,
            Type::TYPE_FLOAT => Scalar::Float,
            Type::TYPE_DOUBLE => Scalar::Double,
            Type::TYPE_STRING => Scalar::String,
            Type::TYPE_BYTES => Scalar::Bytes,
            kind @ (Type::TYPE_MESSAGE | Type::TYPE_GROUP | Type::TYPE_ENUM) => {
                let name = field
                    .type_name
                    .as_deref()
                    .ok_or(ProjectionError::Missing("field target"))?;
                let symbol = self
                    .symbols
                    .get(name)
                    .ok_or_else(|| ProjectionError::MissingType(name.into()))?;
                return match (kind, symbol) {
                    (Type::TYPE_ENUM, Symbol::Enumeration(id)) => Ok(ValueType::Enumeration(*id)),
                    (Type::TYPE_MESSAGE | Type::TYPE_GROUP, Symbol::Message(id)) => {
                        let resolved =
                            features::resolve_child(inherited, features::field_features(field));
                        let encoding = if kind == Type::TYPE_GROUP
                            || (!self.schema.message(*id).map_entry
                                && resolved.message_encoding
                                    == features::MessageEncoding::Delimited)
                        {
                            MessageEncoding::Group
                        } else {
                            MessageEncoding::LengthPrefixed
                        };
                        Ok(ValueType::Message(*id, encoding))
                    }
                    _ => Err(ProjectionError::WrongType(name.into())),
                };
            }
        };
        Ok(ValueType::Scalar(scalar))
    }

    fn populate(&mut self, pending: Pending<'_>) -> Result<(), ProjectionError> {
        let full_name = self.schema.message(pending.id).full_name.clone();
        let descriptor = pending.descriptor;
        let mut oneof_members = vec![(0usize, 0usize); descriptor.oneof_decl.len()];
        for field in &descriptor.field {
            if let Some(index) = field.oneof_index {
                let slot = usize::try_from(index)
                    .ok()
                    .and_then(|index| oneof_members.get_mut(index))
                    .ok_or_else(|| ProjectionError::InvalidOneof(full_name.clone(), index))?;
                if field.label == Some(Label::LABEL_REPEATED) {
                    return Err(ProjectionError::InvalidOneof(full_name.clone(), index));
                }
                slot.0 += 1;
                slot.1 += usize::from(field.proto3_optional == Some(true));
                if slot.1 != 0 && slot.0 != 1 {
                    return Err(ProjectionError::InvalidOneof(full_name.clone(), index));
                }
            } else if field.proto3_optional == Some(true) {
                return Err(ProjectionError::Missing("proto3_optional oneof index"));
            }
        }
        let mut oneofs = BTreeMap::new();
        for (index, oneof) in descriptor.oneof_decl.iter().enumerate() {
            if oneof_members[index].0 == 0 {
                return Err(ProjectionError::InvalidOneof(
                    full_name.clone(),
                    index as i32,
                ));
            }
            if oneof_members[index].1 != 0 {
                continue;
            }
            let id = OneofId(self.schema.oneofs.len());
            self.schema.oneofs.push(Oneof {
                owner: pending.id,
                name: oneof
                    .name
                    .clone()
                    .ok_or(ProjectionError::Missing("oneof name"))?,
                fields: Vec::new(),
            });
            self.schema.messages[pending.id.0].oneofs.push(id);
            oneofs.insert(index as i32, id);
        }
        let mut inherited = pending.features;
        if self.schema.message(pending.id).map_entry {
            inherited.message_encoding = features::MessageEncoding::LengthPrefixed;
        }
        for field in &descriptor.field {
            let name = field
                .name
                .clone()
                .ok_or(ProjectionError::Missing("field name"))?;
            let number = field
                .number
                .ok_or(ProjectionError::Missing("field number"))?;
            let number = FieldNumber::new(number)
                .ok_or_else(|| ProjectionError::InvalidNumber(full_name.clone(), number))?;
            let value = self.value_type(field, &inherited)?;
            let resolved = features::resolve_child(&inherited, features::field_features(field));
            let shape = if field.label == Some(Label::LABEL_REPEATED) {
                if let ValueType::Message(entry, _) = value
                    && self.schema.message(entry).map_entry
                {
                    let entry_descriptor = self.pending[entry.0].descriptor;
                    let key = entry_descriptor
                        .field
                        .iter()
                        .find(|field| field.number == Some(1))
                        .ok_or_else(|| ProjectionError::InvalidMap(full_name.clone()))?;
                    let data = entry_descriptor
                        .field
                        .iter()
                        .find(|field| field.number == Some(2))
                        .ok_or_else(|| ProjectionError::InvalidMap(full_name.clone()))?;
                    let ValueType::Scalar(key) = self.value_type(key, &inherited)? else {
                        return Err(ProjectionError::InvalidMap(full_name.clone()));
                    };
                    if matches!(key, Scalar::Bytes | Scalar::Float | Scalar::Double) {
                        return Err(ProjectionError::InvalidMap(full_name.clone()));
                    }
                    let mut entry_features = features::resolve_child(
                        &inherited,
                        features::message_features(entry_descriptor),
                    );
                    entry_features.message_encoding = features::MessageEncoding::LengthPrefixed;
                    FieldShape::Map {
                        key,
                        value: self.value_type(data, &entry_features)?,
                        entry,
                    }
                } else {
                    FieldShape::Repeated {
                        value,
                        packed: value.is_packable()
                            && field
                                .options
                                .as_option()
                                .and_then(|options| options.packed)
                                .unwrap_or(
                                    resolved.repeated_field_encoding
                                        == RepeatedFieldEncoding::Packed,
                                ),
                    }
                }
            } else if let Some(index) = field.oneof_index
                && field.proto3_optional != Some(true)
            {
                FieldShape::Oneof {
                    value,
                    owner: *oneofs
                        .get(&index)
                        .ok_or_else(|| ProjectionError::InvalidOneof(full_name.clone(), index))?,
                }
            } else {
                let presence = if field.label == Some(Label::LABEL_REQUIRED) {
                    Presence::Required
                } else if field.proto3_optional == Some(true) {
                    Presence::Explicit
                } else {
                    match resolved.field_presence {
                        FieldPresence::LegacyRequired => Presence::Required,
                        FieldPresence::Explicit => Presence::Explicit,
                        FieldPresence::Implicit if matches!(value, ValueType::Message(..)) => {
                            Presence::Explicit
                        }
                        FieldPresence::Implicit => Presence::Implicit,
                        _ => return Err(ProjectionError::UnsupportedPresence(full_name.clone())),
                    }
                };
                FieldShape::Singular { value, presence }
            };
            let id = FieldId(self.schema.fields.len());
            if let FieldShape::Oneof { owner, .. } = shape {
                self.schema.oneofs[owner.0].fields.push(id);
            }
            self.schema.fields.push(Field {
                owner: pending.id,
                name,
                number,
                shape,
                default_literal: field.default_value.clone(),
                bytes_storage: if value == ValueType::Scalar(Scalar::Bytes)
                    && crate::policy::SHARED_BYTES_MESSAGES.contains(&full_name.as_str())
                {
                    BytesStorage::Shared
                } else {
                    BytesStorage::Vector
                },
            });
            self.schema.messages[pending.id.0].fields.push(id);
        }
        Ok(())
    }
}

pub fn project(descriptors: &FileDescriptorSet) -> Result<Schema, ProjectionError> {
    let mut builder = Builder::default();
    for file in &descriptors.file {
        let scope = match file.package.as_deref() {
            None | Some("") => String::new(),
            Some(name) => format!(".{name}"),
        };
        let features = features::for_file(file);
        builder.enumerations(&file.enum_type, &scope, None, &features)?;
        builder.messages(&file.message_type, &scope, None, &features)?;
    }
    let pending = builder
        .pending
        .iter()
        .map(|item| Pending {
            id: item.id,
            descriptor: item.descriptor,
            features: item.features,
        })
        .collect::<Vec<_>>();
    for message in pending {
        builder.populate(message)?;
    }
    Ok(builder.schema)
}

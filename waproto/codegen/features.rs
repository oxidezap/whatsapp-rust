#![forbid(unsafe_code)]

use super::descriptor::{
    self, DescriptorProto, EnumDescriptorProto, FeatureSet, FieldDescriptorProto,
    FileDescriptorProto,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FieldPresence {
    Explicit,
    Implicit,
    LegacyRequired,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnumType {
    Open,
    Closed,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepeatedFieldEncoding {
    Packed,
    Expanded,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageEncoding {
    LengthPrefixed,
    Delimited,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResolvedFeatures {
    pub field_presence: FieldPresence,
    pub enum_type: EnumType,
    pub repeated_field_encoding: RepeatedFieldEncoding,
    pub message_encoding: MessageEncoding,
}

pub fn for_file(file: &FileDescriptorProto) -> ResolvedFeatures {
    let proto3 = file.syntax.as_deref() == Some("proto3")
        || (file.syntax.as_deref() == Some("editions")
            && file.edition == Some(descriptor::Edition::EDITION_PROTO3));
    let editions = file.syntax.as_deref() == Some("editions")
        && !matches!(
            file.edition,
            Some(descriptor::Edition::EDITION_LEGACY | descriptor::Edition::EDITION_PROTO2)
        );
    let inherited = ResolvedFeatures {
        field_presence: if proto3 {
            FieldPresence::Implicit
        } else {
            FieldPresence::Explicit
        },
        enum_type: if proto3 || editions {
            EnumType::Open
        } else {
            EnumType::Closed
        },
        repeated_field_encoding: if proto3 || editions {
            RepeatedFieldEncoding::Packed
        } else {
            RepeatedFieldEncoding::Expanded
        },
        message_encoding: MessageEncoding::LengthPrefixed,
    };
    resolve_child(
        &inherited,
        file.options
            .as_ref()
            .and_then(|options| options.features.as_ref()),
    )
}

pub fn resolve_child(parent: &ResolvedFeatures, child: Option<&FeatureSet>) -> ResolvedFeatures {
    use descriptor::feature_set as fs;
    let Some(child) = child else {
        return *parent;
    };
    ResolvedFeatures {
        field_presence: match child.field_presence {
            Some(fs::FieldPresence::EXPLICIT) => FieldPresence::Explicit,
            Some(fs::FieldPresence::IMPLICIT) => FieldPresence::Implicit,
            Some(fs::FieldPresence::LEGACY_REQUIRED) => FieldPresence::LegacyRequired,
            _ => parent.field_presence,
        },
        enum_type: match child.enum_type {
            Some(fs::EnumType::OPEN) => EnumType::Open,
            Some(fs::EnumType::CLOSED) => EnumType::Closed,
            _ => parent.enum_type,
        },
        repeated_field_encoding: match child.repeated_field_encoding {
            Some(fs::RepeatedFieldEncoding::PACKED) => RepeatedFieldEncoding::Packed,
            Some(fs::RepeatedFieldEncoding::EXPANDED) => RepeatedFieldEncoding::Expanded,
            _ => parent.repeated_field_encoding,
        },
        message_encoding: match child.message_encoding {
            Some(fs::MessageEncoding::LENGTH_PREFIXED) => MessageEncoding::LengthPrefixed,
            Some(fs::MessageEncoding::DELIMITED) => MessageEncoding::Delimited,
            _ => parent.message_encoding,
        },
    }
}
pub fn message_features(value: &DescriptorProto) -> Option<&FeatureSet> {
    value
        .options
        .as_ref()
        .and_then(|options| options.features.as_ref())
}
pub fn field_features(value: &FieldDescriptorProto) -> Option<&FeatureSet> {
    value
        .options
        .as_ref()
        .and_then(|options| options.features.as_ref())
}
pub fn enum_features(value: &EnumDescriptorProto) -> Option<&FeatureSet> {
    value
        .options
        .as_ref()
        .and_then(|options| options.features.as_ref())
}

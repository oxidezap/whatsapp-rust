#![forbid(unsafe_code)]

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct MessageId(pub(super) usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct EnumId(pub(super) usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct FieldId(pub(super) usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub struct OneofId(pub(super) usize);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FieldNumber(u32);

impl FieldNumber {
    pub fn new(value: i32) -> Option<Self> {
        let value = u32::try_from(value).ok()?;
        ((1..=(1 << 29) - 1).contains(&value) && !(19_000..=19_999).contains(&value))
            .then_some(Self(value))
    }

    pub fn get(self) -> u32 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Scalar {
    Bool,
    Int32,
    Int64,
    Uint32,
    Uint64,
    Sint32,
    Sint64,
    Fixed32,
    Fixed64,
    Sfixed32,
    Sfixed64,
    Float,
    Double,
    String,
    Bytes,
}

impl Scalar {
    pub fn is_packable(self) -> bool {
        !matches!(self, Self::String | Self::Bytes)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessageEncoding {
    LengthPrefixed,
    Group,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueType {
    Scalar(Scalar),
    Enumeration(EnumId),
    Message(MessageId, MessageEncoding),
}

impl ValueType {
    pub fn is_packable(self) -> bool {
        match self {
            Self::Scalar(kind) => kind.is_packable(),
            Self::Enumeration(_) => true,
            Self::Message(..) => false,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Presence {
    Explicit,
    Implicit,
    Required,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FieldShape {
    Singular {
        value: ValueType,
        presence: Presence,
    },
    Repeated {
        value: ValueType,
        packed: bool,
    },
    Map {
        key: Scalar,
        value: ValueType,
        entry: MessageId,
    },
    Oneof {
        value: ValueType,
        owner: OneofId,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BytesStorage {
    Vector,
    Shared,
}

#[derive(Clone, Debug)]
pub struct Field {
    pub owner: MessageId,
    pub name: String,
    pub number: FieldNumber,
    pub shape: FieldShape,
    pub default_literal: Option<String>,
    pub bytes_storage: BytesStorage,
}

#[derive(Clone, Debug)]
pub struct Message {
    pub full_name: String,
    pub name: String,
    pub parent: Option<MessageId>,
    pub fields: Vec<FieldId>,
    pub oneofs: Vec<OneofId>,
    pub map_entry: bool,
}

#[derive(Clone, Debug)]
pub struct EnumValue {
    pub name: String,
    pub number: i32,
}

#[derive(Clone, Debug)]
pub struct Enumeration {
    pub full_name: String,
    pub name: String,
    pub parent: Option<MessageId>,
    pub values: Vec<EnumValue>,
    pub open: bool,
}

#[derive(Clone, Debug)]
pub struct Oneof {
    pub owner: MessageId,
    pub name: String,
    pub fields: Vec<FieldId>,
}

#[derive(Clone, Debug, Default)]
pub struct Schema {
    pub messages: Vec<Message>,
    pub enumerations: Vec<Enumeration>,
    pub fields: Vec<Field>,
    pub oneofs: Vec<Oneof>,
}

impl Schema {
    pub fn message(&self, id: MessageId) -> &Message {
        &self.messages[id.0]
    }

    pub fn enumeration(&self, id: EnumId) -> &Enumeration {
        &self.enumerations[id.0]
    }

    pub fn field(&self, id: FieldId) -> &Field {
        &self.fields[id.0]
    }

    pub fn oneof(&self, id: OneofId) -> &Oneof {
        &self.oneofs[id.0]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn declaration_numbers_reject_the_reserved_band_and_wire_overflow() {
        for value in [0, -1, 19_000, 19_999, 1 << 29] {
            assert!(FieldNumber::new(value).is_none());
        }
        for value in [1, 18_999, 20_000, (1 << 29) - 1] {
            assert_eq!(FieldNumber::new(value).unwrap().get(), value as u32);
        }
    }
}

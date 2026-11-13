#![forbid(unsafe_code)]

//! The descriptor fields consumed by this repository's generator. Wire tags
//! follow google/protobuf/descriptor.proto from protobuf v36.1.

use std::io::{self, ErrorKind};

macro_rules! descriptor_enum {
    ($name:ident {$first:ident=$number:literal $(,$variant:ident=$value:literal)* $(,)?}) => {
        #[allow(non_camel_case_types)]
        #[derive(Clone,Copy,Debug,Default,PartialEq,Eq)]
        #[repr(i32)]
        pub enum $name { #[default] $first=$number, $($variant=$value,)* }
        impl $name {
            pub(super) fn from_i32(value:i32)->Option<Self> {
                match value { $number=>Some(Self::$first), $($value=>Some(Self::$variant),)* _=>None }
            }
        }
    };
}

pub mod field_descriptor_proto {
    descriptor_enum!(Label {LABEL_OPTIONAL=1,LABEL_REQUIRED=2,LABEL_REPEATED=3});
    descriptor_enum!(Type {
        TYPE_DOUBLE=1,TYPE_FLOAT=2,TYPE_INT64=3,TYPE_UINT64=4,TYPE_INT32=5,
        TYPE_FIXED64=6,TYPE_FIXED32=7,TYPE_BOOL=8,TYPE_STRING=9,TYPE_GROUP=10,
        TYPE_MESSAGE=11,TYPE_BYTES=12,TYPE_UINT32=13,TYPE_ENUM=14,
        TYPE_SFIXED32=15,TYPE_SFIXED64=16,TYPE_SINT32=17,TYPE_SINT64=18
    });
}
pub mod feature_set {
    descriptor_enum!(FieldPresence {FIELD_PRESENCE_UNKNOWN=0,EXPLICIT=1,IMPLICIT=2,LEGACY_REQUIRED=3});
    descriptor_enum!(EnumType {ENUM_TYPE_UNKNOWN=0,OPEN=1,CLOSED=2});
    descriptor_enum!(RepeatedFieldEncoding {REPEATED_FIELD_ENCODING_UNKNOWN=0,PACKED=1,EXPANDED=2});
    descriptor_enum!(MessageEncoding {MESSAGE_ENCODING_UNKNOWN=0,LENGTH_PREFIXED=1,DELIMITED=2});
}
descriptor_enum!(Edition {
    EDITION_UNKNOWN=0,EDITION_LEGACY=900,EDITION_PROTO2=998,EDITION_PROTO3=999,
    EDITION_2023=1000,EDITION_2024=1001,EDITION_UNSTABLE=9999
});

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileDescriptorSet {
    pub file: Vec<FileDescriptorProto>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileDescriptorProto {
    pub name: Option<String>,
    pub package: Option<String>,
    pub syntax: Option<String>,
    pub edition: Option<Edition>,
    pub message_type: Vec<DescriptorProto>,
    pub enum_type: Vec<EnumDescriptorProto>,
    pub options: Option<FileOptions>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DescriptorProto {
    pub name: Option<String>,
    pub field: Vec<FieldDescriptorProto>,
    pub nested_type: Vec<DescriptorProto>,
    pub enum_type: Vec<EnumDescriptorProto>,
    pub oneof_decl: Vec<OneofDescriptorProto>,
    pub options: Option<MessageOptions>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FieldDescriptorProto {
    pub name: Option<String>,
    pub number: Option<i32>,
    pub label: Option<field_descriptor_proto::Label>,
    pub r#type: Option<field_descriptor_proto::Type>,
    pub type_name: Option<String>,
    pub extendee: Option<String>,
    pub default_value: Option<String>,
    pub oneof_index: Option<i32>,
    pub json_name: Option<String>,
    pub options: Option<FieldOptions>,
    pub proto3_optional: Option<bool>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EnumDescriptorProto {
    pub name: Option<String>,
    pub value: Vec<EnumValueDescriptorProto>,
    pub options: Option<EnumOptions>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EnumValueDescriptorProto {
    pub name: Option<String>,
    pub number: Option<i32>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OneofDescriptorProto {
    pub name: Option<String>,
    pub options: Option<OneofOptions>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FileOptions {
    pub features: Option<FeatureSet>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MessageOptions {
    pub map_entry: Option<bool>,
    pub features: Option<FeatureSet>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FieldOptions {
    pub packed: Option<bool>,
    pub features: Option<FeatureSet>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct EnumOptions {
    pub features: Option<FeatureSet>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct OneofOptions {
    pub features: Option<FeatureSet>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct FeatureSet {
    pub field_presence: Option<feature_set::FieldPresence>,
    pub enum_type: Option<feature_set::EnumType>,
    pub repeated_field_encoding: Option<feature_set::RepeatedFieldEncoding>,
    pub message_encoding: Option<feature_set::MessageEncoding>,
}

fn invalid(message: &str) -> io::Error {
    io::Error::new(ErrorKind::InvalidData, message)
}
struct Cursor<'a, 'b> {
    input: &'a [u8],
    depth: u32,
    memory: &'b mut usize,
}
impl Cursor<'_, '_> {
    fn varint(&mut self) -> io::Result<u64> {
        let mut value = 0;
        for shift in (0..70).step_by(7) {
            let Some((&byte, tail)) = self.input.split_first() else {
                return Err(invalid("truncated descriptor varint"));
            };
            self.input = tail;
            if shift == 63 && byte > 1 {
                return Err(invalid("descriptor varint overflow"));
            }
            value |= u64::from(byte & 127) << shift;
            if byte < 128 {
                return Ok(value);
            }
        }
        Err(invalid("descriptor varint overflow"))
    }
    fn take(&mut self, length: usize) -> io::Result<&[u8]> {
        if length > self.input.len() {
            return Err(invalid("truncated descriptor payload"));
        }
        let (head, tail) = self.input.split_at(length);
        self.input = tail;
        Ok(head)
    }
    fn charge(&mut self, length: usize) -> io::Result<()> {
        *self.memory = self
            .memory
            .checked_sub(length)
            .ok_or_else(|| invalid("descriptor allocation budget exceeded"))?;
        Ok(())
    }
    fn string(&mut self) -> io::Result<String> {
        let length =
            usize::try_from(self.varint()?).map_err(|_| invalid("descriptor length overflow"))?;
        if length > self.input.len() {
            return Err(invalid("truncated descriptor string"));
        }
        let input = self.input;
        let (head, tail) = input.split_at(length);
        let text = std::str::from_utf8(head).map_err(|_| invalid("invalid descriptor UTF-8"))?;
        self.charge(length)?;
        self.input = tail;
        Ok(text.to_owned())
    }
    fn tag(&mut self) -> io::Result<(u32, u8)> {
        let tag = self.varint()?;
        let number =
            u32::try_from(tag >> 3).map_err(|_| invalid("invalid descriptor field number"))?;
        let wire = (tag & 7) as u8;
        if number == 0 || number > 0x1fff_ffff || wire > 5 {
            return Err(invalid("invalid descriptor tag"));
        }
        Ok((number, wire))
    }
    fn child<T>(
        &mut self,
        value: &mut T,
        merge: fn(&mut T, &mut Cursor<'_, '_>) -> io::Result<()>,
    ) -> io::Result<()> {
        let depth = self
            .depth
            .checked_sub(1)
            .ok_or_else(|| invalid("descriptor recursion limit exceeded"))?;
        let length =
            usize::try_from(self.varint()?).map_err(|_| invalid("descriptor length overflow"))?;
        if length > self.input.len() {
            return Err(invalid("truncated descriptor message"));
        }
        let (input, tail) = self.input.split_at(length);
        self.input = tail;
        merge(
            value,
            &mut Cursor {
                input,
                depth,
                memory: self.memory,
            },
        )
    }
    fn repeated<T: Default>(
        &mut self,
        list: &mut Vec<T>,
        merge: fn(&mut T, &mut Cursor<'_, '_>) -> io::Result<()>,
    ) -> io::Result<()> {
        self.charge(size_of::<T>())?;
        let mut value = T::default();
        self.child(&mut value, merge)?;
        list.push(value);
        Ok(())
    }
    fn optional<T: Default>(
        &mut self,
        value: &mut Option<T>,
        merge: fn(&mut T, &mut Cursor<'_, '_>) -> io::Result<()>,
    ) -> io::Result<()> {
        self.child(value.get_or_insert_default(), merge)
    }
    fn skip(&mut self, number: u32, wire: u8) -> io::Result<()> {
        match wire {
            0 => {
                self.varint()?;
            }
            1 => {
                self.take(8)?;
            }
            2 => {
                let length = usize::try_from(self.varint()?)
                    .map_err(|_| invalid("descriptor length overflow"))?;
                self.take(length)?;
            }
            3 => {
                let depth = self
                    .depth
                    .checked_sub(1)
                    .ok_or_else(|| invalid("descriptor recursion limit exceeded"))?;
                let mut child = Cursor {
                    input: self.input,
                    depth,
                    memory: self.memory,
                };
                loop {
                    let (n, w) = child.tag()?;
                    if w == 4 {
                        if n != number {
                            return Err(invalid("mismatched descriptor group"));
                        }
                        break;
                    }
                    child.skip(n, w)?;
                }
                self.input = child.input;
            }
            _ => return Err(invalid("unexpected descriptor end group")),
        }
        Ok(())
    }
}

macro_rules! merge_fields {
    ($name:ident,$kind:ty,$value:ident,$cursor:ident,{$($number:literal/$wire:literal=>$body:expr),* $(,)?})=>{
        fn $name($value:&mut $kind,$cursor:&mut Cursor<'_, '_>)->io::Result<()> {
            while !$cursor.input.is_empty() {
                let(number,wire)=$cursor.tag()?;
                match(number,wire) {
                    $(($number,$wire)=>{$body},)*
                    $(($number,_)=>return Err(invalid("wrong descriptor wire type")),)*
                    _=>$cursor.skip(number,wire)?,
                }
            }Ok(())
        }
    };
}
merge_fields!(merge_set,FileDescriptorSet,v,c,{1/2=>c.repeated(&mut v.file,merge_file)?});
merge_fields!(merge_file,FileDescriptorProto,v,c,{
    1/2=>v.name=Some(c.string()?),2/2=>v.package=Some(c.string()?),
    4/2=>c.repeated(&mut v.message_type,merge_message)?,5/2=>c.repeated(&mut v.enum_type,merge_enum)?,
    8/2=>c.optional(&mut v.options,merge_file_options)?,12/2=>v.syntax=Some(c.string()?),
    14/0=>if let Some(value)=Edition::from_i32(c.varint()?as i32){v.edition=Some(value);},
});
merge_fields!(merge_message,DescriptorProto,v,c,{
    1/2=>v.name=Some(c.string()?),2/2=>c.repeated(&mut v.field,merge_field)?,
    3/2=>c.repeated(&mut v.nested_type,merge_message)?,4/2=>c.repeated(&mut v.enum_type,merge_enum)?,
    7/2=>c.optional(&mut v.options,merge_message_options)?,8/2=>c.repeated(&mut v.oneof_decl,merge_oneof)?,
});
merge_fields!(merge_field,FieldDescriptorProto,v,c,{
    1/2=>v.name=Some(c.string()?),2/2=>v.extendee=Some(c.string()?),3/0=>v.number=Some(c.varint()?as i32),
    4/0=>if let Some(value)=field_descriptor_proto::Label::from_i32(c.varint()?as i32){v.label=Some(value);},
    5/0=>if let Some(value)=field_descriptor_proto::Type::from_i32(c.varint()?as i32){v.r#type=Some(value);},
    6/2=>v.type_name=Some(c.string()?),7/2=>v.default_value=Some(c.string()?),
    8/2=>c.optional(&mut v.options,merge_field_options)?,9/0=>v.oneof_index=Some(c.varint()?as i32),
    10/2=>v.json_name=Some(c.string()?),17/0=>v.proto3_optional=Some(c.varint()?!=0),
});
merge_fields!(merge_enum,EnumDescriptorProto,v,c,{
    1/2=>v.name=Some(c.string()?),2/2=>c.repeated(&mut v.value,merge_enum_value)?,
    3/2=>c.optional(&mut v.options,merge_enum_options)?,
});
merge_fields!(merge_enum_value,EnumValueDescriptorProto,v,c,{1/2=>v.name=Some(c.string()?),2/0=>v.number=Some(c.varint()?as i32)});
merge_fields!(merge_oneof,OneofDescriptorProto,v,c,{1/2=>v.name=Some(c.string()?),2/2=>c.optional(&mut v.options,merge_oneof_options)?});
merge_fields!(merge_file_options,FileOptions,v,c,{50/2=>c.optional(&mut v.features,merge_features)?});
merge_fields!(merge_message_options,MessageOptions,v,c,{7/0=>v.map_entry=Some(c.varint()?!=0),12/2=>c.optional(&mut v.features,merge_features)?});
merge_fields!(merge_field_options,FieldOptions,v,c,{2/0=>v.packed=Some(c.varint()?!=0),21/2=>c.optional(&mut v.features,merge_features)?});
merge_fields!(merge_enum_options,EnumOptions,v,c,{7/2=>c.optional(&mut v.features,merge_features)?});
merge_fields!(merge_oneof_options,OneofOptions,v,c,{1/2=>c.optional(&mut v.features,merge_features)?});
merge_fields!(merge_features,FeatureSet,v,c,{
    1/0=>if let Some(value)=feature_set::FieldPresence::from_i32(c.varint()?as i32){v.field_presence=Some(value);},
    2/0=>if let Some(value)=feature_set::EnumType::from_i32(c.varint()?as i32){v.enum_type=Some(value);},
    3/0=>if let Some(value)=feature_set::RepeatedFieldEncoding::from_i32(c.varint()?as i32){v.repeated_field_encoding=Some(value);},
    5/0=>if let Some(value)=feature_set::MessageEncoding::from_i32(c.varint()?as i32){v.message_encoding=Some(value);},
});

impl FileDescriptorSet {
    pub fn decode_from_slice(input: &[u8]) -> io::Result<Self> {
        if input.len() > 0x7fff_ffff {
            return Err(invalid("descriptor exceeds protobuf size limit"));
        }
        let mut memory = 32 * 1024 * 1024;
        let mut value = Self::default();
        merge_set(
            &mut value,
            &mut Cursor {
                input,
                depth: 100,
                memory: &mut memory,
            },
        )?;
        Ok(value)
    }
}

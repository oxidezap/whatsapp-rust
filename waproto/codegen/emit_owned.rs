#![forbid(unsafe_code)]

use super::schema::*;
use heck::{ToSnakeCase, ToUpperCamelCase};
use std::collections::{BTreeMap, BTreeSet};

const RT: &str = "crate::protobuf";

#[derive(Default)]
struct Module {
    items: String,
    children: BTreeMap<String, Module>,
}
impl Module {
    fn at(&mut self, path: &[String]) -> &mut Self {
        let mut current = self;
        for component in path {
            current = current.children.entry(component.clone()).or_default();
        }
        current
    }
    fn render(&self, out: &mut String) {
        out.push_str("#[allow(unused_imports)] use crate::protobuf::Message as _;\n");
        out.push_str(
            "#[allow(unused_imports)] use crate::protobuf::{MessageView as _,ViewEncode as _};\n",
        );
        out.push_str(&self.items);
        for (name, child) in &self.children {
            out.push_str(&format!("pub mod {name} {{\n"));
            child.render(out);
            out.push_str("}\n");
        }
    }
}

fn ident(name: &str) -> String {
    match name {
        "_" | "Self" | "self" | "super" | "crate" | "extern" => format!("{name}_"),
        "as" | "break" | "const" | "continue" | "else" | "enum" | "false" | "fn" | "for" | "if"
        | "impl" | "in" | "let" | "loop" | "match" | "mod" | "move" | "mut" | "pub" | "ref"
        | "return" | "static" | "struct" | "trait" | "true" | "type" | "unsafe" | "use"
        | "where" | "while" | "dyn" | "abstract" | "become" | "box" | "do" | "final" | "macro"
        | "override" | "priv" | "typeof" | "unsized" | "virtual" | "yield" | "async" | "await"
        | "try" | "gen" => format!("r#{name}"),
        _ => name.into(),
    }
}
fn member(name: &str) -> String {
    ident(&name.to_snake_case())
}

fn module_name(name: &str) -> String {
    let mut output = String::with_capacity(name.len());
    let mut chars = name.chars().peekable();
    let mut previous: Option<char> = None;
    while let Some(current) = chars.next() {
        if current.is_uppercase()
            && previous.is_some_and(|before| {
                before.is_lowercase()
                    || (before.is_uppercase()
                        && chars.peek().is_some_and(|after| after.is_lowercase()))
            })
        {
            output.push('_');
        }
        output.extend(current.to_lowercase());
        previous = Some(current);
    }
    ident(&output)
}

struct Emitter<'a> {
    schema: &'a Schema,
    base: &'a str,
}
impl Emitter<'_> {
    fn scope(&self, parent: Option<MessageId>) -> Vec<String> {
        let mut out = Vec::new();
        let mut parent = parent;
        while let Some(id) = parent {
            let message = self.schema.message(id);
            out.push(module_name(&message.name));
            parent = message.parent;
        }
        out.reverse();
        out
    }
    fn path(&self, parent: Option<MessageId>, name: &str) -> String {
        let mut parts = vec![self.base.to_owned()];
        parts.extend(self.scope(parent));
        parts.push(ident(name));
        parts.join("::")
    }
    fn message_path(&self, id: MessageId) -> String {
        let message = self.schema.message(id);
        self.path(message.parent, &message.name)
    }
    fn enum_path(&self, id: EnumId) -> String {
        let enumeration = self.schema.enumeration(id);
        self.path(enumeration.parent, &enumeration.name)
    }
    fn oneof_path(&self, id: OneofId) -> String {
        let oneof = self.schema.oneof(id);
        self.path(Some(oneof.owner), &oneof.name.to_upper_camel_case())
    }
    fn scalar_type(kind: Scalar, storage: BytesStorage) -> &'static str {
        match kind {
            Scalar::Bool => "bool",
            Scalar::Int32 | Scalar::Sint32 | Scalar::Sfixed32 => "i32",
            Scalar::Int64 | Scalar::Sint64 | Scalar::Sfixed64 => "i64",
            Scalar::Uint32 | Scalar::Fixed32 => "u32",
            Scalar::Uint64 | Scalar::Fixed64 => "u64",
            Scalar::Float => "f32",
            Scalar::Double => "f64",
            Scalar::String => "::std::string::String",
            Scalar::Bytes if storage == BytesStorage::Shared => "::bytes::Bytes",
            Scalar::Bytes => "::std::vec::Vec<u8>",
        }
    }
    fn value_type(&self, value: ValueType, storage: BytesStorage) -> String {
        match value {
            ValueType::Scalar(kind) => Self::scalar_type(kind, storage).into(),
            ValueType::Message(id, _) => self.message_path(id),
            ValueType::Enumeration(id) if self.schema.enumeration(id).open => {
                format!("{RT}::EnumValue<{}>", self.enum_path(id))
            }
            ValueType::Enumeration(id) => self.enum_path(id),
        }
    }
    fn field_type(&self, field: &Field) -> String {
        match field.shape {
            FieldShape::Singular {
                value: ValueType::Message(id, _),
                ..
            } => format!("{RT}::MessageField<{}>", self.message_path(id)),
            FieldShape::Singular {
                value,
                presence: Presence::Explicit,
            } => format!(
                "::core::option::Option<{}>",
                self.value_type(value, field.bytes_storage)
            ),
            FieldShape::Singular { value, .. } => self.value_type(value, field.bytes_storage),
            FieldShape::Repeated { value, .. } => {
                format!(
                    "::std::vec::Vec<{}>",
                    self.value_type(value, field.bytes_storage)
                )
            }
            FieldShape::Map { key, value, .. } => format!(
                "::std::collections::HashMap<{}, {}>",
                Self::scalar_type(key, BytesStorage::Vector),
                self.value_type(value, field.bytes_storage)
            ),
            FieldShape::Oneof { .. } => unreachable!("oneof member has no struct slot"),
        }
    }
    fn enum_default(&self, id: EnumId, literal: Option<&str>) -> String {
        let enumeration = self.schema.enumeration(id);
        let selected = literal
            .and_then(|name| enumeration.values.iter().find(|value| value.name == name))
            .unwrap_or_else(|| enumeration.values.first().expect("nonempty enum"));
        let canonical = enumeration
            .values
            .iter()
            .find(|value| value.number == selected.number)
            .expect("canonical variant");
        let expression = format!("{}::{}", self.enum_path(id), ident(&canonical.name));
        if enumeration.open {
            format!("{RT}::EnumValue::Known({expression})")
        } else {
            expression
        }
    }
    fn default_field(&self, field: &Field) -> String {
        match field.shape {
            FieldShape::Singular {
                value: ValueType::Message(..),
                ..
            } => format!("{RT}::MessageField::none()"),
            FieldShape::Singular {
                presence: Presence::Explicit,
                ..
            } => "None".into(),
            FieldShape::Repeated { .. } => "::std::vec::Vec::new()".into(),
            FieldShape::Map { .. } => "::std::collections::HashMap::new()".into(),
            FieldShape::Oneof { .. } => "None".into(),
            FieldShape::Singular {
                value: ValueType::Enumeration(id),
                ..
            } => self.enum_default(id, field.default_literal.as_deref()),
            FieldShape::Singular {
                value: ValueType::Scalar(kind),
                ..
            } => {
                if let Some(literal) = &field.default_literal {
                    assert!(
                        !matches!(
                            kind,
                            Scalar::Bytes | Scalar::String | Scalar::Float | Scalar::Double
                        ),
                        "unqualified literal {literal}"
                    );
                    return literal.clone();
                }
                match kind {
                    Scalar::Bool => "false".into(),
                    Scalar::String => "::std::string::String::new()".into(),
                    Scalar::Bytes if field.bytes_storage == BytesStorage::Shared => {
                        "::bytes::Bytes::new()".into()
                    }
                    Scalar::Bytes => "::std::vec::Vec::new()".into(),
                    Scalar::Float | Scalar::Double => "0.0".into(),
                    _ => "0".into(),
                }
            }
        }
    }
    fn enum_source(&self, enumeration: &Enumeration) -> String {
        let name = ident(&enumeration.name);
        let mut out = format!(
            "#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)] #[repr(i32)] #[non_exhaustive]\n#[cfg_attr(not(feature=\"serde-enum-repr\"),derive(::serde::Serialize))]\n#[cfg_attr(feature=\"serde-enum-repr\",derive(::serde_repr::Serialize_repr))]\n#[cfg_attr(all(feature=\"serde-deserialize\",not(feature=\"serde-enum-repr\")),derive(::serde::Deserialize))]\n#[cfg_attr(all(feature=\"serde-deserialize\",feature=\"serde-enum-repr\"),derive(::serde_repr::Deserialize_repr))]\n#[cfg_attr(all(feature=\"serde-snake-case\",not(feature=\"serde-enum-repr\")),serde(rename_all(deserialize=\"lowercase\")))]\npub enum {name} {{\n"
        );
        let mut canonical = BTreeMap::<i32, String>::new();
        for value in &enumeration.values {
            if let std::collections::btree_map::Entry::Vacant(slot) = canonical.entry(value.number)
            {
                slot.insert(ident(&value.name));
                out.push_str(&format!("{} = {},\n", ident(&value.name), value.number));
            }
        }
        out.push_str("}\n");
        out.push_str(&format!(
            "impl ::core::default::Default for {name} {{ fn default()->Self {{ Self::{} }} }}\n",
            canonical[&enumeration.values.first().expect("enum default").number]
        ));
        out.push_str(&format!("impl {RT}::Enumeration for {name} {{ fn from_i32(value:i32)->::core::option::Option<Self>{{match value{{"));
        for (number, variant) in &canonical {
            out.push_str(&format!("{number}=>Some(Self::{variant}),"));
        }
        out.push_str(
            "_=>None}}\nfn to_i32(&self)->i32{*self as i32}\nfn proto_name(&self)->&'static str{",
        );
        if canonical.len() == 1 {
            out.push_str(&format!(
                "{:?}",
                enumeration.values.first().expect("enum name").name
            ));
        } else {
            out.push_str("match self{");
            for (number, variant) in &canonical {
                let proto = &enumeration
                    .values
                    .iter()
                    .find(|value| value.number == *number)
                    .expect("enum name")
                    .name;
                out.push_str(&format!("Self::{variant}=>{proto:?},"));
            }
            out.push('}');
        }
        out.push_str(
            "}\nfn from_proto_name(value:&str)->::core::option::Option<Self>{match value{",
        );
        for value in &enumeration.values {
            out.push_str(&format!(
                "{:?}=>Some(Self::{}),",
                value.name, canonical[&value.number]
            ));
        }
        out.push_str("_=>None}}\nfn values()->&'static[Self]{&[");
        let mut seen = BTreeSet::new();
        for value in &enumeration.values {
            if seen.insert(value.number) {
                out.push_str(&format!("Self::{},", canonical[&value.number]));
            }
        }
        out.push_str("]}}\n");
        out.push_str(&format!("impl {name} {{ pub fn as_str_name(&self)->&'static str{{ <Self as {RT}::Enumeration>::proto_name(self) }} pub fn from_str_name(name:&str)->::core::option::Option<Self>{{<Self as {RT}::Enumeration>::from_proto_name(name)}}\n"));
        let mut names: BTreeSet<_> = canonical.values().cloned().collect();
        for value in &enumeration.values {
            for alias in [value.name.clone(), value.name.to_upper_camel_case()] {
                let alias = ident(&alias);
                if names.insert(alias.clone()) {
                    out.push_str(&format!(
                        "#[allow(non_upper_case_globals)] pub const {alias}:Self=Self::{};\n",
                        canonical[&value.number]
                    ));
                }
            }
        }
        out.push_str("}\n");
        out
    }
    fn oneof_source(&self, oneof: &Oneof) -> String {
        let name = ident(&oneof.name.to_upper_camel_case());
        let mut out = format!(
            "#[derive(Clone,Debug,PartialEq,::serde::Serialize)] #[cfg_attr(feature=\"serde-deserialize\",derive(::serde::Deserialize))] #[non_exhaustive] pub enum {name} {{\n"
        );
        for id in &oneof.fields {
            let field = self.schema.field(*id);
            let FieldShape::Oneof { value, .. } = field.shape else {
                unreachable!()
            };
            let mut ty = self.value_type(value, field.bytes_storage);
            if matches!(value, ValueType::Message(..)) {
                ty = format!("::std::boxed::Box<{ty}>");
            }
            out.push_str(&format!(
                "{}({ty}),\n",
                ident(&field.name.to_upper_camel_case())
            ));
        }
        out.push_str("}\n");
        out
    }
    fn slot(&self, field: &Field) -> String {
        match field.shape {
            FieldShape::Oneof { owner, .. } => member(&self.schema.oneof(owner).name),
            _ => member(&field.name),
        }
    }
    fn for_value(&self, field: &Field, body: &str) -> String {
        let slot = self.slot(field);
        match field.shape {
            FieldShape::Singular {
                value: ValueType::Message(..),
                ..
            } => format!("if let Some(v)=self.{slot}.as_option(){{{body}}}\n"),
            FieldShape::Singular {
                presence: Presence::Explicit,
                ..
            } => format!("if let Some(v)=&self.{slot}{{{body}}}\n"),
            FieldShape::Singular {
                presence: Presence::Implicit,
                value,
            } => {
                let check = match value {
                    ValueType::Scalar(Scalar::String | Scalar::Bytes) => {
                        format!("!self.{slot}.is_empty()")
                    }
                    ValueType::Scalar(Scalar::Bool) => format!("self.{slot}"),
                    ValueType::Enumeration(_) => {
                        format!("{RT}::Enumeration::to_i32(&self.{slot})!=0")
                    }
                    ValueType::Scalar(Scalar::Float | Scalar::Double) => {
                        format!("self.{slot}.to_bits()!=0")
                    }
                    _ => format!("self.{slot}!=0"),
                };
                format!("if {check}{{let v=&self.{slot};{body}}}\n")
            }
            FieldShape::Singular { .. } => format!("{{let v=&self.{slot};{body}}}\n"),
            FieldShape::Repeated { .. } => format!("for v in &self.{slot}{{{body}}}\n"),
            FieldShape::Oneof { owner, .. } => format!(
                "if let Some({}::{}(v))=&self.{slot}{{{body}}}\n",
                self.oneof_path(owner),
                ident(&field.name.to_upper_camel_case())
            ),
            FieldShape::Map { .. } => unreachable!("map loop"),
        }
    }
    fn numeric(&self, kind: ValueType, value: &str) -> String {
        match kind {
            ValueType::Enumeration(id) if self.schema.enumeration(id).open => {
                format!("{value}.to_i32() as u64")
            }
            ValueType::Enumeration(_) => format!("{RT}::Enumeration::to_i32({value}) as u64"),
            ValueType::Scalar(Scalar::Bool) => format!("u64::from(*{value})"),
            ValueType::Scalar(Scalar::Uint64) => format!("*{value}"),
            ValueType::Scalar(Scalar::Sint32) => {
                format!("u64::from(((*{value} as u32)<<1)^((*{value}>>31)as u32))")
            }
            ValueType::Scalar(Scalar::Sint64) => {
                format!("((*{value} as u64)<<1)^((*{value}>>63)as u64)")
            }
            _ => format!("*{value} as u64"),
        }
    }
    fn wire(kind: ValueType) -> u8 {
        match kind {
            ValueType::Message(_, MessageEncoding::Group) => 3,
            ValueType::Message(..) | ValueType::Scalar(Scalar::String | Scalar::Bytes) => 2,
            ValueType::Scalar(Scalar::Fixed64 | Scalar::Sfixed64 | Scalar::Double) => 1,
            ValueType::Scalar(Scalar::Fixed32 | Scalar::Sfixed32 | Scalar::Float) => 5,
            _ => 0,
        }
    }
    fn payload_size(&self, kind: ValueType, value: &str) -> String {
        match Self::wire(kind) {
            1 => format!("{{let _={value};8u64}}"),
            5 => format!("{{let _={value};4u64}}"),
            2 => {
                format!("{RT}::encoding::varint_len({value}.len()as u64)as u64+{value}.len()as u64")
            }
            0 => format!(
                "{RT}::encoding::varint_len({})as u64",
                self.numeric(kind, value)
            ),
            _ => unreachable!(),
        }
    }
    fn size_value(&self, kind: ValueType, field: u32, value: &str) -> String {
        let tag = (u64::from(field) << 3) | u64::from(Self::wire(kind));
        if let ValueType::Message(_, encoding) = kind {
            if encoding == MessageEncoding::Group {
                format!(
                    "size=size.saturating_add({RT}::Message::compute_size({value},cache)as u64+2*{RT}::encoding::varint_len({tag})as u64);"
                )
            } else {
                format!(
                    "let slot=cache.reserve();let n={RT}::Message::compute_size({value},cache);cache.set(slot,n);size=size.saturating_add({RT}::encoding::varint_len({tag})as u64+{RT}::encoding::varint_len(u64::from(n))as u64+u64::from(n));"
                )
            }
        } else {
            format!(
                "size=size.saturating_add({RT}::encoding::varint_len({tag})as u64+{});",
                self.payload_size(kind, value)
            )
        }
    }
    fn write_value(&self, kind: ValueType, field: u32, value: &str) -> String {
        let wire = Self::wire(kind);
        let tag = (u64::from(field) << 3) | u64::from(wire);
        let mut out = format!("{RT}::encoding::put_varint({tag},output);");
        match kind {
            ValueType::Message(_, encoding) => { if encoding==MessageEncoding::LengthPrefixed { out.push_str(&format!("{RT}::encoding::put_varint(u64::from(cache.consume_next()),output);")); } out.push_str(&format!("{RT}::Message::write_to({value},cache,output);")); if encoding==MessageEncoding::Group { out.push_str(&format!("{RT}::encoding::put_varint({},output);",(u64::from(field)<<3)|4)); } }
            ValueType::Scalar(Scalar::String) => out.push_str(&format!("{RT}::encoding::put_varint({value}.len()as u64,output);output.put_slice({value}.as_bytes());")),
            ValueType::Scalar(Scalar::Bytes) => out.push_str(&format!("{RT}::encoding::put_varint({value}.len()as u64,output);output.put_slice({value});")),
            ValueType::Scalar(Scalar::Float) => out.push_str(&format!("output.put_u32_le({value}.to_bits());")),
            ValueType::Scalar(Scalar::Double) => out.push_str(&format!("output.put_u64_le({value}.to_bits());")),
            ValueType::Scalar(Scalar::Fixed32) => out.push_str(&format!("output.put_u32_le(*{value});")),
            ValueType::Scalar(Scalar::Fixed64) => out.push_str(&format!("output.put_u64_le(*{value});")),
            _ if wire==5 => out.push_str(&format!("output.put_u32_le(*{value} as u32);")),
            _ if wire==1 => out.push_str(&format!("output.put_u64_le(*{value} as u64);")),
            _=>out.push_str(&format!("{RT}::encoding::put_varint({},output);",self.numeric(kind,value))),
        }
        out
    }
    fn read_value(&self, kind: ValueType, storage: BytesStorage, input: &str) -> String {
        match kind {
            ValueType::Scalar(Scalar::String) => format!("{RT}::types::decode_string({input})?"),
            ValueType::Scalar(Scalar::Bytes) if storage == BytesStorage::Shared => {
                format!("{RT}::types::decode_shared_bytes({input})?")
            }
            ValueType::Scalar(Scalar::Bytes) => format!("{RT}::types::decode_bytes({input})?"),
            ValueType::Scalar(Scalar::Bool) => {
                format!("{RT}::encoding::decode_varint({input})?!=0")
            }
            ValueType::Scalar(Scalar::Sint32) => format!(
                "{{let n={RT}::encoding::decode_varint({input})?as u32;(n>>1)as i32^-((n&1)as i32)}}"
            ),
            ValueType::Scalar(Scalar::Sint64) => format!(
                "{{let n={RT}::encoding::decode_varint({input})?;(n>>1)as i64^-((n&1)as i64)}}"
            ),
            ValueType::Scalar(Scalar::Float) => {
                format!("f32::from_bits({RT}::types::decode_fixed32({input})?)")
            }
            ValueType::Scalar(Scalar::Double) => {
                format!("f64::from_bits({RT}::types::decode_fixed64({input})?)")
            }
            ValueType::Scalar(Scalar::Fixed32) => format!("{RT}::types::decode_fixed32({input})?"),
            ValueType::Scalar(Scalar::Fixed64) => format!("{RT}::types::decode_fixed64({input})?"),
            ValueType::Scalar(Scalar::Uint64) => format!("{RT}::encoding::decode_varint({input})?"),
            ValueType::Scalar(kind) if Self::wire(ValueType::Scalar(kind)) == 5 => format!(
                "{RT}::types::decode_fixed32({input})?as {}",
                Self::scalar_type(kind, storage)
            ),
            ValueType::Scalar(kind) if Self::wire(ValueType::Scalar(kind)) == 1 => format!(
                "{RT}::types::decode_fixed64({input})?as {}",
                Self::scalar_type(kind, storage)
            ),
            ValueType::Scalar(kind) => format!(
                "{RT}::encoding::decode_varint({input})?as {}",
                Self::scalar_type(kind, storage)
            ),
            ValueType::Enumeration(id) if self.schema.enumeration(id).open => format!(
                "{RT}::EnumValue::<{}>::from({RT}::encoding::decode_varint({input})?as i32)",
                self.enum_path(id)
            ),
            ValueType::Enumeration(id) => format!(
                "<{} as {RT}::Enumeration>::from_i32({RT}::encoding::decode_varint({input})?as i32)",
                self.enum_path(id)
            ),
            ValueType::Message(..) => unreachable!("message merge"),
        }
    }
    fn message_source(&self, message: &Message) -> String {
        let name = ident(&message.name);
        let pinned_clone = message.full_name == ".whatsapp.Message";
        let clone = if pinned_clone { "" } else { "Clone," };
        let mut out = format!(
            "#[derive({clone}Debug,PartialEq,::serde::Serialize)] #[cfg_attr(feature=\"serde-deserialize\",derive(::serde::Deserialize),serde(default))] #[non_exhaustive] pub struct {name}{{\n"
        );
        for id in &message.fields {
            let field = self.schema.field(*id);
            if matches!(field.shape, FieldShape::Oneof { .. }) {
                continue;
            }
            if field.bytes_storage == BytesStorage::Shared {
                out.push_str("#[serde(skip)]\n");
            }
            let field_path = format!("{}.{}", message.full_name, field.name);
            let enum_adapter =
                if crate::policy::FEATURE_ENUM_SERDE_FIELDS.contains(&field_path.as_str()) {
                    match field.shape {
                        FieldShape::Singular {
                            value: ValueType::Enumeration(id),
                            presence: Presence::Explicit,
                        } if self.schema.enumeration(id).open => Some(("serialize", "deserialize")),
                        FieldShape::Repeated {
                            value: ValueType::Enumeration(id),
                            ..
                        } if self.schema.enumeration(id).open => {
                            Some(("serialize_repeated", "deserialize_repeated"))
                        }
                        _ => None,
                    }
                } else {
                    None
                };
            if let Some((serialize, deserialize)) = enum_adapter {
                out.push_str(&format!("#[serde(serialize_with=\"crate::protobuf::open_enum_serde::{serialize}\")] #[cfg_attr(feature=\"serde-deserialize\",serde(deserialize_with=\"crate::protobuf::open_enum_serde::{deserialize}\"))]\n"));
            }
            out.push_str(&format!(
                "pub {}: {},\n",
                member(&field.name),
                self.field_type(field)
            ));
        }
        for id in &message.oneofs {
            let oneof = self.schema.oneof(*id);
            out.push_str(&format!(
                "pub {}: ::core::option::Option<{}>,\n",
                member(&oneof.name),
                self.oneof_path(*id)
            ));
        }
        out.push_str("}\n");
        if pinned_clone {
            out.push_str(&format!("impl ::core::clone::Clone for {name}{{#[inline(never)]fn clone(&self)->Self{{Self{{"));
            for id in &message.fields {
                let field = self.schema.field(*id);
                if !matches!(field.shape, FieldShape::Oneof { .. }) {
                    let slot = member(&field.name);
                    out.push_str(&format!(
                        "{slot}: ::core::clone::Clone::clone(&self.{slot}),"
                    ));
                }
            }
            for id in &message.oneofs {
                let slot = member(&self.schema.oneof(*id).name);
                out.push_str(&format!(
                    "{slot}: ::core::clone::Clone::clone(&self.{slot}),"
                ));
            }
            out.push_str("}}}\n");
        }
        let mut initial = String::new();
        for id in &message.fields {
            let field = self.schema.field(*id);
            if !matches!(field.shape, FieldShape::Oneof { .. }) {
                initial.push_str(&format!(
                    "{}: {},",
                    member(&field.name),
                    self.default_field(field)
                ));
            }
        }
        for id in &message.oneofs {
            initial.push_str(&format!("{}:None,", member(&self.schema.oneof(*id).name)));
        }
        out.push_str(&format!(
            "impl ::core::default::Default for {name}{{fn default()->Self{{Self{{{initial}}}}}}}\n"
        ));
        if message
            .fields
            .iter()
            .any(|id| matches!(self.schema.field(*id).shape, FieldShape::Map { .. }))
        {
            out.push_str(&format!("impl {RT}::DefaultInstance for {name}{{fn default_instance()->&'static Self{{static INSTANCE: ::std::sync::LazyLock<{name}>=::std::sync::LazyLock::new(::core::default::Default::default);&INSTANCE}}}}\n"));
        } else {
            out.push_str(&format!("impl {RT}::DefaultInstance for {name}{{fn default_instance()->&'static Self{{static INSTANCE:{name}={name}{{{initial}}};&INSTANCE}}}}\n"));
        }
        let full = message.full_name.trim_start_matches('.');
        let mut top = message;
        while let Some(parent) = top.parent {
            top = self.schema.message(parent);
        }
        let package = top
            .full_name
            .rsplit_once('.')
            .map_or("", |(package, _)| package.trim_start_matches('.'));
        let relative = if package.is_empty() {
            full
        } else {
            full.strip_prefix(package)
                .expect("message package")
                .trim_start_matches('.')
        };
        let url = format!("type.googleapis.com/{full}");
        out.push_str(&format!("impl {name}{{pub const TYPE_URL:&'static str={url:?};}}impl {RT}::MessageName for {name}{{const NAME:&'static str={relative:?};const PACKAGE:&'static str={package:?};const FULL_NAME:&'static str={full:?};const TYPE_URL:&'static str={url:?};}}\n"));
        out.push_str(&format!("impl {name}{{"));
        for id in &message.fields {
            let field = self.schema.field(*id);
            let FieldShape::Singular {
                value,
                presence: Presence::Explicit,
            } = field.shape
            else {
                continue;
            };
            if matches!(value, ValueType::Message(..)) {
                continue;
            }
            let ty = self.value_type(value, field.bytes_storage);
            let convert = matches!(
                value,
                ValueType::Scalar(Scalar::String | Scalar::Bytes) | ValueType::Enumeration(_)
            );
            let arg = if convert {
                format!("impl ::core::convert::Into<{ty}>")
            } else {
                ty
            };
            let expression = if convert { "value.into()" } else { "value" };
            out.push_str(&format!("#[must_use] #[inline] pub fn with_{}(mut self,value:{arg})->Self{{self.{}=Some({expression});self}}",field.name.to_snake_case(),member(&field.name)));
        }
        out.push_str("}\n");
        let mut fields = message
            .fields
            .iter()
            .map(|id| self.schema.field(*id))
            .collect::<Vec<_>>();
        let group_positions: BTreeMap<_, _> = message
            .oneofs
            .iter()
            .map(|id| {
                let first = self
                    .schema
                    .oneof(*id)
                    .fields
                    .iter()
                    .map(|field| self.schema.field(*field).number.get())
                    .min()
                    .expect("nonempty oneof");
                (*id, first)
            })
            .collect();
        fields.sort_by_key(|field| match field.shape {
            FieldShape::Oneof { owner, .. } => (group_positions[&owner], field.number.get()),
            _ => (field.number.get(), field.number.get()),
        });
        let mutable = if fields.is_empty() { "" } else { "mut " };
        out.push_str(&format!("impl {RT}::Message for {name}{{fn compute_size(&self,cache:&mut {RT}::SizeCache)->u32{{let {mutable}size=0u64;let _=&cache;"));
        for field in &fields {
            out.push_str(&self.encode_field(field, false));
        }
        out.push_str(&format!("{RT}::saturate_size(size)}}\nfn write_to(&self,cache:&mut {RT}::SizeCache,output:&mut impl {RT}::EncodeSink){{let _=&cache;let _=&output;"));
        for field in &fields {
            out.push_str(&self.encode_field(field, true));
        }
        out.push_str(&format!("}}\nfn merge_field(&mut self,tag:{RT}::encoding::Tag,input:&mut impl ::bytes::Buf,context:{RT}::DecodeContext<'_>)->Result<(),{RT}::DecodeError>{{let _=&context;use ::bytes::Buf as _;"));
        if !fields.is_empty() {
            out.push_str("match(tag.field_number(),tag.wire_type()){");
        }
        for field in &fields {
            out.push_str(&self.decode_field(field));
        }
        let mut known = BTreeSet::new();
        for field in &fields {
            if known.insert(field.number.get()) {
                let wire = Self::wire(match field.shape {
                    FieldShape::Singular { value, .. }
                    | FieldShape::Repeated { value, .. }
                    | FieldShape::Oneof { value, .. } => value,
                    FieldShape::Map { .. } => ValueType::Scalar(Scalar::Bytes),
                });
                out.push_str(&format!("({},_)=>Err({RT}::encoding::wire_type_mismatch(tag,{RT}::encoding::WireType::from_u32({wire})?)),",field.number.get()));
            }
        }
        if fields.is_empty() {
            out.push_str(&format!("{RT}::encoding::skip_field_depth(tag,input,context.depth())}}\nfn clear(&mut self){{"));
        } else {
            out.push_str(&format!("_=>{RT}::encoding::skip_field_depth(tag,input,context.depth()),}}}}\nfn clear(&mut self){{"));
        }
        for id in &message.fields {
            let field = self.schema.field(*id);
            if matches!(field.shape, FieldShape::Oneof { .. }) {
                continue;
            }
            let slot = member(&field.name);
            if matches!(
                field.shape,
                FieldShape::Repeated { .. } | FieldShape::Map { .. }
            ) {
                out.push_str(&format!("self.{slot}.clear();"));
            } else {
                out.push_str(&format!("self.{slot}={};", self.default_field(field)));
            }
        }
        for id in &message.oneofs {
            out.push_str(&format!(
                "self.{}=None;",
                member(&self.schema.oneof(*id).name)
            ));
        }
        out.push_str("}}\n");
        out
    }
    fn encode_field(&self, field: &Field, write: bool) -> String {
        let number = field.number.get();
        if let FieldShape::Map { key, value, .. } = field.shape {
            let slot = member(&field.name);
            let key_ty = ValueType::Scalar(key);
            let key_size = self.payload_size(key_ty, "k");
            let value_size = if matches!(value, ValueType::Message(..)) {
                "u64::from(n)".into()
            } else {
                self.payload_size(value, "v")
            };
            let mut out = format!("for(k,v)in &self.{slot}{{");
            if let ValueType::Message(..) = value {
                if write {
                    out.push_str(&format!("let entry_n=cache.consume_next();{RT}::encoding::put_varint({},output);{RT}::encoding::put_varint(u64::from(entry_n),output);",(u64::from(number)<<3)|2));
                    out.push_str(&self.write_value(key_ty, 1, "k"));
                    out.push_str(&self.write_value(value, 2, "v"));
                } else {
                    out.push_str(&format!("let entry_slot=cache.reserve();let value_slot=cache.reserve();let n={RT}::Message::compute_size(v,cache);cache.set(value_slot,n);let entry=1u64+{key_size}+1u64+{RT}::encoding::varint_len(u64::from(n))as u64+u64::from(n);cache.set(entry_slot,{RT}::saturate_size(entry));size=size.saturating_add({RT}::encoding::varint_len({})as u64+{RT}::encoding::varint_len(entry)as u64+entry);",(u64::from(number)<<3)|2));
                }
            } else {
                out.push_str(&format!("let entry=1u64+{key_size}+1u64+{value_size};"));
                if write {
                    out.push_str(&format!("{RT}::encoding::put_varint({},output);{RT}::encoding::put_varint(entry,output);",(u64::from(number)<<3)|2));
                    out.push_str(&self.write_value(key_ty, 1, "k"));
                    out.push_str(&self.write_value(value, 2, "v"));
                } else {
                    out.push_str(&format!("size=size.saturating_add({RT}::encoding::varint_len({})as u64+{RT}::encoding::varint_len(entry)as u64+entry);",(u64::from(number)<<3)|2));
                }
            }
            out.push('}');
            return out;
        }
        let value = match field.shape {
            FieldShape::Singular { value, .. }
            | FieldShape::Repeated { value, .. }
            | FieldShape::Oneof { value, .. } => value,
            _ => unreachable!(),
        };
        if let FieldShape::Repeated { packed: true, .. } = field.shape {
            let slot = member(&field.name);
            let payload = self.payload_size(value, "v");
            let mut out = format!(
                "if !self.{slot}.is_empty(){{let n=self.{slot}.iter().fold(0u64,|n,v|n.saturating_add({payload}));"
            );
            if write {
                out.push_str(&format!("{RT}::encoding::put_varint({},output);{RT}::encoding::put_varint(n,output);for v in &self.{slot}{{",(u64::from(number)<<3)|2));
                let encoded = self.write_value(value, number, "v");
                let start = encoded.find(';').expect("tag terminator") + 1;
                out.push_str(&encoded[start..]);
                out.push('}');
            } else {
                out.push_str(&format!("size=size.saturating_add({RT}::encoding::varint_len({})as u64+{RT}::encoding::varint_len(n)as u64+n);",(u64::from(number)<<3)|2));
            }
            out.push('}');
            return out;
        }
        let v = if matches!(
            field.shape,
            FieldShape::Oneof {
                value: ValueType::Message(..),
                ..
            }
        ) {
            "v.as_ref()"
        } else {
            "v"
        };
        let body = if write {
            self.write_value(value, number, v)
        } else {
            self.size_value(value, number, v)
        };
        self.for_value(field, &body)
    }
    fn decode_field(&self, field: &Field) -> String {
        let slot = self.slot(field);
        let number = field.number.get();
        if let FieldShape::Map { key, value, .. } = field.shape {
            let key_ty = Self::scalar_type(key, BytesStorage::Vector);
            let val_ty = self.value_type(value, field.bytes_storage);
            let mut out = format!(
                "({number},{RT}::encoding::WireType::LengthDelimited)=>{{context.register_element_memory(::std::mem::size_of::<({key_ty},{val_ty})>())?;let context=context.descend()?;let n={RT}::encoding::decode_varint(input)?;if n>u64::from({RT}::MAX_MESSAGE_BYTES){{return Err({RT}::DecodeError::MessageTooLarge);}}if n as usize>input.remaining(){{return Err({RT}::DecodeError::UnexpectedEof);}}let limit=input.remaining()-n as usize;let mut key: {key_ty}=::core::default::Default::default();let mut value: {val_ty}=::core::default::Default::default();while input.remaining()>limit{{let item={RT}::encoding::Tag::decode(input)?;match item.field_number(){{1=>{{{RT}::encoding::check_wire_type(item,{RT}::encoding::WireType::from_u32({})?)?;key={};}},2=>{{",
                Self::wire(ValueType::Scalar(key)),
                self.read_value(ValueType::Scalar(key), BytesStorage::Vector, "input")
            );
            out.push_str(&format!(
                "{RT}::encoding::check_wire_type(item,{RT}::encoding::WireType::from_u32({})?)?;",
                Self::wire(value)
            ));
            if let ValueType::Message(_, encoding) = value {
                if encoding == MessageEncoding::Group {
                    out.push_str(&format!(
                        "{RT}::Message::merge_group(&mut value,input,context,2)?;"
                    ));
                } else {
                    out.push_str(&format!(
                        "{RT}::Message::merge_length_delimited(&mut value,input,context)?;"
                    ));
                }
            } else if let ValueType::Enumeration(id) = value
                && !self.schema.enumeration(id).open
            {
                out.push_str(&format!(
                    "if let Some(v)={}{{value=v;}}",
                    self.read_value(value, field.bytes_storage, "input")
                ));
            } else {
                out.push_str(&format!(
                    "value={};",
                    self.read_value(value, field.bytes_storage, "input")
                ));
            }
            out.push_str(&format!("}},_=>{RT}::encoding::skip_field_depth(item,input,context.depth())?,}}}}if input.remaining()!=limit{{return Err({RT}::DecodeError::UnexpectedEof);}}self.{slot}.insert(key,value);Ok(())}},"));
            return out;
        }
        let value = match field.shape {
            FieldShape::Singular { value, .. }
            | FieldShape::Repeated { value, .. }
            | FieldShape::Oneof { value, .. } => value,
            _ => unreachable!(),
        };
        let wire = Self::wire(value);
        let wire_name = match wire {
            0 => "Varint",
            1 => "Fixed64",
            2 => "LengthDelimited",
            3 => "StartGroup",
            5 => "Fixed32",
            _ => unreachable!(),
        };
        let mut out = String::new();
        if matches!(field.shape, FieldShape::Repeated { .. }) && value.is_packable() {
            let expression = self.read_value(value, field.bytes_storage, "&mut packed");
            let push = if let ValueType::Enumeration(id) = value
                && !self.schema.enumeration(id).open
            {
                format!("if let Some(v)={expression}{{self.{slot}.push(v);}} ")
            } else {
                format!("self.{slot}.push({expression});")
            };
            out.push_str(&format!("({number},{RT}::encoding::WireType::LengthDelimited)=>{{let n={RT}::encoding::decode_varint(input)?;let n=usize::try_from(n).map_err(|_|{RT}::DecodeError::MessageTooLarge)?;if n>input.remaining(){{return Err({RT}::DecodeError::UnexpectedEof);}}let mut packed=input.take(n);while packed.has_remaining(){{{push}}}Ok(())}},"));
        }
        out.push_str(&format!(
            "({number},{RT}::encoding::WireType::{wire_name})=>{{"
        ));
        if let ValueType::Message(id, encoding) = value {
            let merge = if encoding == MessageEncoding::Group {
                format!("merge_group(input,context,{number})?")
            } else {
                "merge_length_delimited(input,context)?".into()
            };
            match field.shape{FieldShape::Singular{..}=>out.push_str(&format!("self.{slot}.get_or_insert_default().{merge};")),FieldShape::Repeated{..}=>out.push_str(&format!("context.register_element_memory(::std::mem::size_of::<{}>())?;let mut value={}::default();value.{merge};self.{slot}.push(value);",self.message_path(id),self.message_path(id))),FieldShape::Oneof{owner,..}=>{let path=self.oneof_path(owner);let variant=ident(&field.name.to_upper_camel_case());out.push_str(&format!("if !matches!(&self.{slot},Some({path}::{variant}(_))){{self.{slot}=Some({path}::{variant}(::std::boxed::Box::default()));}}if let Some({path}::{variant}(value))=&mut self.{slot}{{value.{merge};}}"));},_=>unreachable!()}
        } else {
            if matches!(field.shape, FieldShape::Repeated { .. })
                && matches!(value, ValueType::Scalar(Scalar::String | Scalar::Bytes))
            {
                out.push_str(&format!(
                    "context.register_element_memory(::std::mem::size_of::<{}>())?;",
                    self.value_type(value, field.bytes_storage)
                ));
            }
            let expr = self.read_value(value, field.bytes_storage, "input");
            let closed =
                matches!(value,ValueType::Enumeration(id)if !self.schema.enumeration(id).open);
            if closed {
                out.push_str(&format!("if let Some(value)={expr}{{"));
            } else {
                out.push_str(&format!("let value={expr};"));
            }
            match field.shape {
                FieldShape::Singular {
                    presence: Presence::Explicit,
                    ..
                } => out.push_str(&format!("self.{slot}=Some(value);")),
                FieldShape::Singular { .. } => out.push_str(&format!("self.{slot}=value;")),
                FieldShape::Repeated { .. } => out.push_str(&format!("self.{slot}.push(value);")),
                FieldShape::Oneof { owner, .. } => out.push_str(&format!(
                    "self.{slot}=Some({}::{}(value));",
                    self.oneof_path(owner),
                    ident(&field.name.to_upper_camel_case())
                )),
                _ => unreachable!(),
            }
            if closed {
                out.push('}');
            }
        }
        out.push_str("Ok(())},");
        out
    }
}

pub fn generate(schema: &Schema, base: &str) -> Result<String, syn::Error> {
    let source = generate_raw(schema, base);
    let parsed = syn::parse_file(&source)?;
    Ok(prettyplease::unparse(&parsed))
}

pub fn generate_raw(schema: &Schema, base: &str) -> String {
    let emitter = Emitter { schema, base };
    let mut root = Module::default();
    for enumeration in &schema.enumerations {
        root.at(&emitter.scope(enumeration.parent))
            .items
            .push_str(&emitter.enum_source(enumeration));
    }
    for (index, message) in schema.messages.iter().enumerate() {
        if message.map_entry {
            continue;
        }
        root.at(&emitter.scope(message.parent))
            .items
            .push_str(&emitter.message_source(message));
        root.at(&emitter.scope(message.parent))
            .items
            .push_str(&emitter.view_source(message, MessageId(index)));
        for id in &message.oneofs {
            root.at(&emitter.scope(Some(MessageId(index))))
                .items
                .push_str(&emitter.oneof_source(schema.oneof(*id)));
            root.at(&emitter.scope(Some(MessageId(index))))
                .items
                .push_str(&emitter.view_oneof_source(schema.oneof(*id), *id));
        }
    }
    let mut source = String::new();
    root.render(&mut source);
    source
}

impl Emitter<'_> {
    fn view_path(&self, id: MessageId) -> String {
        let message = self.schema.message(id);
        self.path(message.parent, &format!("{}View", message.name))
    }
    fn view_oneof_path(&self, id: OneofId) -> String {
        let group = self.schema.oneof(id);
        self.path(
            Some(group.owner),
            &format!("{}View", group.name.to_upper_camel_case()),
        )
    }
    fn view_oneof_lifetime(&self, id: OneofId) -> bool {
        self.schema.oneof(id).fields.iter().any(|field| {
            matches!(
                self.schema.field(*field).shape,
                FieldShape::Oneof {
                    value: ValueType::Message(..)
                        | ValueType::Scalar(Scalar::String | Scalar::Bytes),
                    ..
                }
            )
        })
    }
    fn view_value_type(&self, value: ValueType) -> String {
        match value {
            ValueType::Scalar(Scalar::String) => "&'a str".into(),
            ValueType::Scalar(Scalar::Bytes) => "&'a [u8]".into(),
            ValueType::Message(id, _) => format!("{}<'a>", self.view_path(id)),
            _ => self.value_type(value, BytesStorage::Vector),
        }
    }
    fn view_field_type(&self, field: &Field) -> String {
        match field.shape {
            FieldShape::Singular {
                value: ValueType::Message(id, _),
                ..
            } => format!("{RT}::MessageFieldView<{}<'a>>", self.view_path(id)),
            FieldShape::Singular {
                value,
                presence: Presence::Explicit,
            } => format!("::core::option::Option<{}>", self.view_value_type(value)),
            FieldShape::Singular { value, .. } => self.view_value_type(value),
            FieldShape::Repeated { value, .. } => {
                format!("{RT}::RepeatedView<'a,{}>", self.view_value_type(value))
            }
            FieldShape::Map { key, value, .. } => format!(
                "{RT}::MapView<'a,{},{}>",
                self.view_value_type(ValueType::Scalar(key)),
                self.view_value_type(value)
            ),
            FieldShape::Oneof { .. } => unreachable!(),
        }
    }
    fn view_default(&self, field: &Field) -> String {
        match field.shape {
            FieldShape::Singular {
                value: ValueType::Message(..),
                ..
            } => format!("{RT}::MessageFieldView::unset()"),
            FieldShape::Repeated { .. } => {
                format!("{RT}::RepeatedView::new(::std::vec::Vec::new())")
            }
            FieldShape::Map { .. } => format!("{RT}::MapView::new(::std::vec::Vec::new())"),
            FieldShape::Singular {
                presence: Presence::Explicit,
                ..
            }
            | FieldShape::Oneof { .. } => "None".into(),
            FieldShape::Singular {
                value: ValueType::Scalar(Scalar::String),
                ..
            } => "\"\"".into(),
            FieldShape::Singular {
                value: ValueType::Scalar(Scalar::Bytes),
                ..
            } => "&[]".into(),
            _ => self.default_field(field),
        }
    }
    fn view_read(&self, value: ValueType, input: &str) -> String {
        match value {
            ValueType::Scalar(Scalar::String) => format!("{RT}::types::borrow_str({input})?"),
            ValueType::Scalar(Scalar::Bytes) => format!("{RT}::types::borrow_bytes({input})?"),
            _ => self.read_value(value, BytesStorage::Vector, input),
        }
    }
    fn promoted(&self, value: ValueType, storage: BytesStorage, input: &str) -> String {
        let dereferenced = input
            .strip_prefix('&')
            .map_or_else(|| format!("*{input}"), str::to_owned);
        match value {
            ValueType::Scalar(Scalar::String) => format!("({dereferenced}).to_owned()"),
            ValueType::Scalar(Scalar::Bytes) if storage == BytesStorage::Shared => {
                format!("{RT}::view::bytes_from_source(source,{input})")
            }
            ValueType::Scalar(Scalar::Bytes) => format!("({dereferenced}).to_vec()"),
            ValueType::Message(..) => {
                format!("{RT}::MessageView::to_owned_from_source({input},source)?")
            }
            _ => dereferenced,
        }
    }
    fn view_oneof_source(&self, group: &Oneof, id: OneofId) -> String {
        let name = format!("{}View", group.name.to_upper_camel_case());
        let lifetime = if self.view_oneof_lifetime(id) {
            "<'a>"
        } else {
            ""
        };
        let mut out =
            format!("#[derive(Clone,Debug,PartialEq)]#[non_exhaustive]pub enum {name}{lifetime}{{");
        for field in &group.fields {
            let field = self.schema.field(*field);
            let FieldShape::Oneof { value, .. } = field.shape else {
                unreachable!()
            };
            let mut ty = self.view_value_type(value);
            if matches!(value, ValueType::Message(..)) {
                ty = format!("::std::boxed::Box<{ty}>");
            }
            out.push_str(&format!(
                "{}({ty}),",
                ident(&field.name.to_upper_camel_case())
            ));
        }
        out.push_str("}\n");
        out
    }
    fn view_encode_field(&self, field: &Field, write: bool) -> String {
        let mut out = self
            .encode_field(field, write)
            .replace(&format!("{RT}::Message::"), &format!("{RT}::ViewEncode::"));
        if let FieldShape::Oneof { owner, .. } = field.shape {
            out = out.replace(&self.oneof_path(owner), &self.view_oneof_path(owner));
        }
        out
    }
    fn view_decode_field(&self, field: &Field) -> String {
        let number = field.number.get();
        let slot = self.slot(field);
        if let FieldShape::Map { key, value, .. } = field.shape {
            let kt = self.view_value_type(ValueType::Scalar(key));
            let vt = self.view_value_type(value);
            let key_default = if key == Scalar::String {
                "\"\""
            } else {
                "::core::default::Default::default()"
            };
            let value_default = match value {
                ValueType::Scalar(Scalar::String) => "\"\"",
                ValueType::Scalar(Scalar::Bytes) => "&[]",
                _ => "::core::default::Default::default()",
            };
            let mut out = format!(
                "({number},{RT}::encoding::WireType::LengthDelimited)=>{{context.register_element_memory(::std::mem::size_of::<({kt},{vt})>())?;let context=context.descend()?;let mut entry={RT}::types::borrow_bytes(input)?;let mut key: {kt}={key_default};let mut value: {vt}={value_default};while !entry.is_empty(){{let item={RT}::encoding::Tag::decode(&mut entry)?;match item.field_number(){{1=>{{{RT}::encoding::check_wire_type(item,{RT}::encoding::WireType::from_u32({})?)?;key={};}},2=>{{{RT}::encoding::check_wire_type(item,{RT}::encoding::WireType::from_u32({})?)?;",
                Self::wire(ValueType::Scalar(key)),
                self.view_read(ValueType::Scalar(key), "&mut entry"),
                Self::wire(value)
            );
            if let ValueType::Message(_, encoding) = value {
                let payload = if encoding == MessageEncoding::Group {
                    format!("{RT}::types::borrow_group(&mut entry,2,child_context.depth())?")
                } else {
                    format!("{RT}::types::borrow_bytes(&mut entry)?")
                };
                out.push_str(&format!("let child_context=context.descend()?;let payload={payload};{RT}::MessageView::merge_into_view(&mut value,payload,child_context)?;"));
            } else if matches!(value,ValueType::Enumeration(id)if !self.schema.enumeration(id).open)
            {
                out.push_str(&format!(
                    "if let Some(v)={}{{value=v;}}",
                    self.view_read(value, "&mut entry")
                ));
            } else {
                out.push_str(&format!("value={};", self.view_read(value, "&mut entry")));
            }
            out.push_str(&format!("}},_=>{RT}::encoding::skip_field_depth(item,&mut entry,context.depth())?,}}}}self.{slot}.push(key,value);Ok(())}},"));
            return out;
        }
        let value = match field.shape {
            FieldShape::Singular { value, .. }
            | FieldShape::Repeated { value, .. }
            | FieldShape::Oneof { value, .. } => value,
            _ => unreachable!(),
        };
        let wire = Self::wire(value);
        let wn = match wire {
            0 => "Varint",
            1 => "Fixed64",
            2 => "LengthDelimited",
            3 => "StartGroup",
            5 => "Fixed32",
            _ => unreachable!(),
        };
        let mut out = String::new();
        if matches!(field.shape, FieldShape::Repeated { .. }) && value.is_packable() {
            let expr = self.view_read(value, "&mut packed");
            let push = if matches!(value,ValueType::Enumeration(id)if !self.schema.enumeration(id).open)
            {
                format!("if let Some(v)={expr}{{self.{slot}.push(v);}} ")
            } else {
                format!("self.{slot}.push({expr});")
            };
            out.push_str(&format!("({number},{RT}::encoding::WireType::LengthDelimited)=>{{let mut packed={RT}::types::borrow_bytes(input)?;while !packed.is_empty(){{{push}}}Ok(())}},"));
        }
        out.push_str(&format!("({number},{RT}::encoding::WireType::{wn})=>{{"));
        if let ValueType::Message(id, encoding) = value {
            if matches!(field.shape, FieldShape::Repeated { .. }) {
                out.push_str(&format!(
                    "context.register_element_memory(::std::mem::size_of::<{}<'a>>())?;",
                    self.view_path(id)
                ));
            }
            let payload = if encoding == MessageEncoding::Group {
                format!("{RT}::types::borrow_group(input,{number},child_context.depth())?")
            } else {
                format!("{RT}::types::borrow_bytes(input)?")
            };
            out.push_str(&format!(
                "let child_context=context.descend()?;let payload={payload};"
            ));
            match field.shape{
                FieldShape::Singular{..}=>out.push_str(&format!("if let Some(value)=self.{slot}.as_mut(){{{RT}::MessageView::merge_into_view(value,payload,child_context)?;}}else{{self.{slot}={RT}::MessageFieldView::set(<{}<'a>as {RT}::MessageView<'a>>::decode_view_ctx(payload,child_context)?);}}",self.view_path(id))),
                FieldShape::Repeated{..}=>out.push_str(&format!("self.{slot}.push(<{}<'a>as {RT}::MessageView<'a>>::decode_view_ctx(payload,child_context)?);",self.view_path(id))),
                FieldShape::Oneof{owner,..}=>{let path=self.view_oneof_path(owner);let variant=ident(&field.name.to_upper_camel_case());out.push_str(&format!("if let Some({path}::{variant}(value))=&mut self.{slot}{{{RT}::MessageView::merge_into_view(value.as_mut(),payload,child_context)?;}}else{{self.{slot}=Some({path}::{variant}(::std::boxed::Box::new(<{}<'a>as {RT}::MessageView<'a>>::decode_view_ctx(payload,child_context)?)));}}",self.view_path(id)));},_=>unreachable!(),
            }
        } else {
            if matches!(field.shape, FieldShape::Repeated { .. })
                && matches!(value, ValueType::Scalar(Scalar::String | Scalar::Bytes))
            {
                out.push_str(&format!(
                    "context.register_element_memory(::std::mem::size_of::<{}>())?;",
                    self.view_value_type(value)
                ));
            }
            let expr = self.view_read(value, "input");
            let closed =
                matches!(value,ValueType::Enumeration(id)if !self.schema.enumeration(id).open);
            if closed {
                out.push_str(&format!("if let Some(value)={expr}{{"));
            } else {
                out.push_str(&format!("let value={expr};"));
            }
            match field.shape {
                FieldShape::Singular {
                    presence: Presence::Explicit,
                    ..
                } => out.push_str(&format!("self.{slot}=Some(value);")),
                FieldShape::Singular { .. } => out.push_str(&format!("self.{slot}=value;")),
                FieldShape::Repeated { .. } => out.push_str(&format!("self.{slot}.push(value);")),
                FieldShape::Oneof { owner, .. } => out.push_str(&format!(
                    "self.{slot}=Some({}::{}(value));",
                    self.view_oneof_path(owner),
                    ident(&field.name.to_upper_camel_case())
                )),
                _ => unreachable!(),
            }
            if closed {
                out.push('}');
            }
        }
        out.push_str("Ok(())},");
        out
    }
    fn view_source(&self, message: &Message, id: MessageId) -> String {
        let name = ident(&format!("{}View", message.name));
        let owned = self.message_path(id);
        let mut out = format!("#[derive(Clone,PartialEq)]#[non_exhaustive]pub struct {name}<'a>{{");
        for field in &message.fields {
            let field = self.schema.field(*field);
            if !matches!(field.shape, FieldShape::Oneof { .. }) {
                out.push_str(&format!(
                    "pub {}: {},",
                    member(&field.name),
                    self.view_field_type(field)
                ));
            }
        }
        for group in &message.oneofs {
            let oneof = self.schema.oneof(*group);
            out.push_str(&format!(
                "pub {}: ::core::option::Option<{}{}>,",
                member(&oneof.name),
                self.view_oneof_path(*group),
                if self.view_oneof_lifetime(*group) {
                    "<'a>"
                } else {
                    ""
                }
            ));
        }
        out.push_str("_lifetime: ::core::marker::PhantomData<&'a()>,}\n");
        let mut initial = String::new();
        for field in &message.fields {
            let field = self.schema.field(*field);
            if !matches!(field.shape, FieldShape::Oneof { .. }) {
                initial.push_str(&format!(
                    "{}: {},",
                    member(&field.name),
                    self.view_default(field)
                ));
            }
        }
        for group in &message.oneofs {
            initial.push_str(&format!(
                "{}:None,",
                member(&self.schema.oneof(*group).name)
            ));
        }
        initial.push_str("_lifetime: ::core::marker::PhantomData,");
        out.push_str(&format!("impl<'a>::core::default::Default for {name}<'a>{{fn default()->Self{{Self{{{initial}}}}}}}impl<'v>{RT}::DefaultViewInstance for {name}<'v>{{fn default_view_instance<'b>()->&'b Self where Self:'b{{static INSTANCE:{name}<'static>={name}{{{initial}}};&INSTANCE}}}}\n"));
        out.push_str(&format!("impl<'a>::core::fmt::Debug for {name}<'a>{{fn fmt(&self,f:&mut ::core::fmt::Formatter<'_>)->::core::fmt::Result{{let mut d=f.debug_struct({name:?});"));
        for field in &message.fields {
            let field = self.schema.field(*field);
            if !matches!(field.shape, FieldShape::Oneof { .. }) {
                let n = member(&field.name);
                out.push_str(&format!("d.field({n:?},&self.{n});"));
            }
        }
        for group in &message.oneofs {
            let n = member(&self.schema.oneof(*group).name);
            out.push_str(&format!("d.field({n:?},&self.{n});"));
        }
        out.push_str("d.finish()}}\n");
        out.push_str(&format!("impl {RT}::ViewReborrow for {name}<'static>{{type Reborrowed<'b>={name}<'b>;fn reborrow<'b>(this:&'b Self)->&'b Self::Reborrowed<'b>{{this}}}}impl<'a>{RT}::MessageName for {name}<'a>{{const NAME:&'static str=<{owned} as {RT}::MessageName>::NAME;const PACKAGE:&'static str=<{owned} as {RT}::MessageName>::PACKAGE;const FULL_NAME:&'static str=<{owned} as {RT}::MessageName>::FULL_NAME;const TYPE_URL:&'static str=<{owned} as {RT}::MessageName>::TYPE_URL;}}\n"));
        let dispatch = if message.fields.is_empty() {
            ""
        } else {
            "match(tag.field_number(),tag.wire_type()){ "
        };
        out.push_str(&format!("impl<'a>{RT}::MessageView<'a> for {name}<'a>{{type Owned={owned};fn decode_view(input:&'a[u8])->Result<Self,{RT}::DecodeError>{{{RT}::DecodeOptions::new().with_context(|context|Self::decode_view_ctx(input,context))}}fn decode_view_with_ctx(input:&'a[u8],context:{RT}::DecodeContext<'_>)->Result<Self,{RT}::DecodeError>{{Self::decode_view_ctx(input,context)}}fn merge_view_field(&mut self,tag:{RT}::encoding::Tag,cur:&'a[u8],_before_tag:&'a[u8],context:{RT}::DecodeContext<'_>)->Result<&'a[u8],{RT}::DecodeError>{{let _=&context;let mut cur=cur;let input=&mut cur;let result={dispatch}"));
        for field in &message.fields {
            out.push_str(&self.view_decode_field(self.schema.field(*field)));
        }
        let mut known = BTreeSet::new();
        for field in &message.fields {
            let field = self.schema.field(*field);
            if known.insert(field.number.get()) {
                let value = match field.shape {
                    FieldShape::Singular { value, .. }
                    | FieldShape::Repeated { value, .. }
                    | FieldShape::Oneof { value, .. } => value,
                    _ => ValueType::Scalar(Scalar::Bytes),
                };
                out.push_str(&format!("({},_)=>Err({RT}::encoding::wire_type_mismatch(tag,{RT}::encoding::WireType::from_u32({})?)),",field.number.get(),Self::wire(value)));
            }
        }
        let (fallback, end) = if message.fields.is_empty() {
            ("", "")
        } else {
            ("_=>", ",}")
        };
        out.push_str(&format!("{fallback}{RT}::encoding::skip_field_depth(tag,input,context.depth()){end};result?;Ok(cur)}}fn to_owned_message(&self)->Result<Self::Owned,{RT}::DecodeError>{{self.to_owned_from_source(None)}}fn to_owned_from_source(&self,source: ::core::option::Option<&::bytes::Bytes>)->Result<Self::Owned,{RT}::DecodeError>{{let _=&source;Ok({owned}{{"));
        for field in &message.fields {
            let field = self.schema.field(*field);
            let slot = member(&field.name);
            let expr = match field.shape {
                FieldShape::Oneof { .. } => continue,
                FieldShape::Singular {
                    value: ValueType::Message(..),
                    ..
                } => format!(
                    "match self.{slot}.as_option(){{Some(v)=>{RT}::MessageField::some({RT}::MessageView::to_owned_from_source(v,source)?),None=>{RT}::MessageField::none()}}"
                ),
                FieldShape::Singular {
                    value,
                    presence: Presence::Explicit,
                } => format!(
                    "self.{slot}.as_ref().map(|v|{})",
                    self.promoted(value, field.bytes_storage, "v")
                ),
                FieldShape::Singular { value, .. } => {
                    self.promoted(value, field.bytes_storage, &format!("&self.{slot}"))
                }
                FieldShape::Repeated {
                    value: ValueType::Message(..),
                    ..
                } => format!(
                    "self.{slot}.iter().map(|v|{RT}::MessageView::to_owned_from_source(v,source)).collect::<Result<_,{RT}::DecodeError>>()?"
                ),
                FieldShape::Repeated { value, .. } => format!(
                    "self.{slot}.iter().map(|v|Ok({})).collect::<Result<_,{RT}::DecodeError>>()?",
                    self.promoted(value, field.bytes_storage, "v")
                ),
                FieldShape::Map { key, value, .. } => format!(
                    "self.{slot}.iter().map(|(k,v)|Ok(({},{}))).collect::<Result<_,{RT}::DecodeError>>()?",
                    self.promoted(ValueType::Scalar(key), BytesStorage::Vector, "k"),
                    self.promoted(value, field.bytes_storage, "v")
                ),
            };
            out.push_str(&format!("{slot}: {expr},"));
        }
        for group in &message.oneofs {
            let oneof = self.schema.oneof(*group);
            let slot = member(&oneof.name);
            if oneof.fields.len() == 1 {
                let field = self.schema.field(oneof.fields[0]);
                let FieldShape::Oneof { value, .. } = field.shape else {
                    unreachable!()
                };
                if !matches!(value, ValueType::Message(..)) {
                    let variant = ident(&field.name.to_upper_camel_case());
                    let promoted = self.promoted(value, field.bytes_storage, "v");
                    out.push_str(&format!("{slot}:self.{slot}.as_ref().map(|{}::{variant}(v)|{}::{variant}({promoted})),",self.view_oneof_path(*group),self.oneof_path(*group)));
                    continue;
                }
            }
            out.push_str(&format!("{slot}:match &self.{slot}{{None=>None,"));
            for fid in &oneof.fields {
                let field = self.schema.field(*fid);
                let FieldShape::Oneof { value, .. } = field.shape else {
                    unreachable!()
                };
                let variant = ident(&field.name.to_upper_camel_case());
                let v = if matches!(value, ValueType::Message(..)) {
                    "v.as_ref()"
                } else {
                    "v"
                };
                let mut promoted = self.promoted(value, field.bytes_storage, v);
                if matches!(value, ValueType::Message(..)) {
                    promoted = format!("::std::boxed::Box::new({promoted})");
                }
                out.push_str(&format!(
                    "Some({}::{variant}(v))=>Some({}::{variant}({promoted})),",
                    self.view_oneof_path(*group),
                    self.oneof_path(*group)
                ));
            }
            out.push_str("},");
        }
        out.push_str("})}}\n");
        let mut fields = message
            .fields
            .iter()
            .map(|field| self.schema.field(*field))
            .collect::<Vec<_>>();
        let positions: BTreeMap<_, _> = message
            .oneofs
            .iter()
            .map(|id| {
                (
                    *id,
                    self.schema
                        .oneof(*id)
                        .fields
                        .iter()
                        .map(|field| self.schema.field(*field).number.get())
                        .min()
                        .expect("oneof member"),
                )
            })
            .collect();
        fields.sort_by_key(|field| match field.shape {
            FieldShape::Oneof { owner, .. } => (positions[&owner], field.number.get()),
            _ => (field.number.get(), field.number.get()),
        });
        let mutable = if fields.is_empty() { "" } else { "mut " };
        out.push_str(&format!("impl<'a>{RT}::ViewEncode<'a> for {name}<'a>{{fn compute_size(&self,cache:&mut {RT}::SizeCache)->u32{{let _=&cache;let {mutable}size=0u64;"));
        for field in &fields {
            out.push_str(&self.view_encode_field(field, false));
        }
        out.push_str(&format!("{RT}::saturate_size(size)}}fn write_to(&self,cache:&mut {RT}::SizeCache,output:&mut impl {RT}::EncodeSink){{let _=&cache;let _=&output;"));
        for field in &fields {
            out.push_str(&self.view_encode_field(field, true));
        }
        out.push_str("}}\n");
        out.push_str(&self.owned_view_source(message, id));
        out
    }
    fn owned_view_source(&self, message: &Message, id: MessageId) -> String {
        let name = ident(&format!("{}OwnedView", message.name));
        let view = ident(&format!("{}View", message.name));
        let owned = self.message_path(id);
        let inner = format!("{RT}::OwnedView<{view}<'static>>");
        let mut out = format!(
            "#[derive(Clone,Debug)]pub struct {name}({inner});impl {name}{{\
             pub fn decode(bytes: ::bytes::Bytes)->Result<Self,{RT}::DecodeError>{{Ok(Self({RT}::OwnedView::decode(bytes)?))}}\
             pub fn decode_with_options(bytes: ::bytes::Bytes,options:&{RT}::DecodeOptions)->Result<Self,{RT}::DecodeError>{{Ok(Self({RT}::OwnedView::decode_with_options(bytes,options)?))}}\
             pub fn from_owned(value:&{owned})->Result<Self,{RT}::DecodeError>{{Ok(Self({RT}::OwnedView::from_owned(value)?))}}\
             pub fn view(&self)->&{view}<'_>{{self.0.reborrow()}}\
             pub fn to_owned_message(&self)->{owned}{{self.0.to_owned_message()}}\
             pub fn bytes(&self)->&::bytes::Bytes{{self.0.bytes()}}\
             pub fn into_bytes(self)->::bytes::Bytes{{self.0.into_bytes()}}"
        );
        let reserved = [
            "decode",
            "decode_with_options",
            "from_owned",
            "view",
            "to_owned_message",
            "bytes",
            "into_bytes",
            "clone",
            "into",
        ];
        for fid in &message.fields {
            let field = self.schema.field(*fid);
            if matches!(field.shape, FieldShape::Oneof { .. }) {
                continue;
            }
            let slot = member(&field.name);
            if reserved.contains(&slot.as_str()) {
                continue;
            }
            let ty = self.view_field_type(field).replace("'a", "'_");
            let reference = matches!(
                field.shape,
                FieldShape::Map { .. }
                    | FieldShape::Repeated { .. }
                    | FieldShape::Singular {
                        value: ValueType::Message(..),
                        ..
                    }
            );
            let borrow = if reference { "&" } else { "" };
            out.push_str(&format!(
                "pub fn {slot}(&self)->{borrow}{ty}{{{borrow}self.view().{slot}}}"
            ));
        }
        for gid in &message.oneofs {
            let group = self.schema.oneof(*gid);
            let slot = member(&group.name);
            if reserved.contains(&slot.as_str()) {
                continue;
            }
            let ty = self.view_oneof_path(*gid);
            let lifetime = if self.view_oneof_lifetime(*gid) {
                "<'_>"
            } else {
                ""
            };
            out.push_str(&format!("pub fn {slot}(&self)->::core::option::Option<&{ty}{lifetime}>{{self.view().{slot}.as_ref()}}"));
        }
        out.push_str(&format!("}}impl From<{inner}> for {name}{{fn from(value:{inner})->Self{{Self(value)}}}}impl From<{name}> for {inner}{{fn from(value:{name})->Self{{value.0}}}}impl AsRef<{inner}> for {name}{{fn as_ref(&self)->&{inner}{{&self.0}}}}impl {RT}::HasMessageView for {owned}{{type View<'a>={view}<'a>;type ViewHandle={name};}}\n"));
        out
    }
    fn oracle_value(&self, value: ValueType, a: &str, b: &str) -> String {
        match value {
            ValueType::Scalar(Scalar::Float | Scalar::Double) => {
                format!("assert_eq!({a}.to_bits(),{b}.to_bits());")
            }
            ValueType::Scalar(Scalar::Bytes) => {
                format!("assert_eq!(AsRef::<[u8]>::as_ref({a}),AsRef::<[u8]>::as_ref({b}));")
            }
            ValueType::Scalar(_) => format!("assert_eq!({a},{b});"),
            ValueType::Enumeration(id) if self.schema.enumeration(id).open => format!(
                "assert_eq!({a}.to_i32(),{b}.to_i32());assert_eq!({a}.is_known(),{b}.is_known());"
            ),
            ValueType::Enumeration(_) => {
                format!("assert_eq!(rt::Enumeration::to_i32({a}),buffa::Enumeration::to_i32({b}));")
            }
            ValueType::Message(id, _) => format!("assert_m{}({a},{b});", id.0),
        }
    }
}

fn oracle_seed(schema: &Schema, id: MessageId, depth: usize) -> Vec<u8> {
    fn varint(mut value: u64, out: &mut Vec<u8>) {
        while value >= 128 {
            out.push((value as u8 & 127) | 128);
            value >>= 7;
        }
        out.push(value as u8);
    }
    let mut out = Vec::new();
    for fid in &schema.message(id).fields {
        let field = schema.field(*fid);
        // Keep one map key across combined fixtures so randomized map
        // iteration cannot obscure the byte-exact codec comparison.
        if schema.message(id).map_entry && field.number.get() == 1 {
            continue;
        }
        let value = match field.shape {
            FieldShape::Singular { value, .. }
            | FieldShape::Repeated { value, .. }
            | FieldShape::Oneof { value, .. } => value,
            FieldShape::Map { entry, .. } => {
                ValueType::Message(entry, MessageEncoding::LengthPrefixed)
            }
        };
        let payload = match value {
            ValueType::Message(child, _) if depth > 0 => oracle_seed(schema, child, depth - 1),
            ValueType::Message(..) => continue,
            ValueType::Scalar(Scalar::String) => b"fixture".to_vec(),
            ValueType::Scalar(Scalar::Bytes) => vec![0x42, 0x81],
            ValueType::Enumeration(id) => {
                let mut payload = Vec::new();
                varint(
                    schema.enumeration(id).values[0].number as i64 as u64,
                    &mut payload,
                );
                payload
            }
            _ => match Emitter::wire(value) {
                0 => vec![1],
                1 => 1f64.to_le_bytes().to_vec(),
                5 => 1f32.to_le_bytes().to_vec(),
                _ => unreachable!(),
            },
        };
        let wire = Emitter::wire(value);
        varint(
            (u64::from(field.number.get()) << 3) | u64::from(wire),
            &mut out,
        );
        if wire == 2 {
            varint(payload.len() as u64, &mut out);
        }
        out.extend_from_slice(&payload);
        if wire == 3 {
            varint((u64::from(field.number.get()) << 3) | 4, &mut out);
        }
    }
    out
}

pub fn generate_oracle(schema: &Schema) -> Result<String, syn::Error> {
    let new = Emitter {
        schema,
        base: "rt::generated::whatsapp",
    };
    let old = Emitter {
        schema,
        base: "waproto::reference",
    };
    let mut source = String::new();
    for (index, message) in schema.messages.iter().enumerate() {
        if message.map_entry {
            continue;
        }
        let id = MessageId(index);
        source.push_str(&format!(
            "fn assert_m{index}(a:&{},b:&{}){{let _=(a,b);",
            new.message_path(id),
            old.message_path(id)
        ));
        for fid in &message.fields {
            let field = schema.field(*fid);
            let slot = member(&field.name);
            match field.shape {
                FieldShape::Singular { value: ValueType::Message(child,_), .. } => source.push_str(&format!("assert_eq!(a.{slot}.is_set(),b.{slot}.is_set());if let(Some(a),Some(b))=(a.{slot}.as_option(),b.{slot}.as_option()){{assert_m{}(a,b);}}",child.0)),
                FieldShape::Singular { value, presence: Presence::Explicit } => source.push_str(&format!("match(&a.{slot},&b.{slot}){{(Some(a),Some(b))=>{{{}}},(None,None)=>{{}},_=>panic!(\"presence mismatch {}.{slot}\"),}}",new.oracle_value(value,"a","b"),message.full_name)),
                FieldShape::Singular { value, .. } => source.push_str(&new.oracle_value(value,&format!("&a.{slot}"),&format!("&b.{slot}"))),
                FieldShape::Repeated { value, .. } => source.push_str(&format!("assert_eq!(a.{slot}.len(),b.{slot}.len());for(a,b)in a.{slot}.iter().zip(&b.{slot}){{{}}}",new.oracle_value(value,"a","b"))),
                FieldShape::Map { value, .. } => source.push_str(&format!("assert_eq!(a.{slot}.len(),b.{slot}.len());for(key,a)in &a.{slot}{{let b=b.{slot}.get(key).expect(\"same map key\");{}}}",new.oracle_value(value,"a","b"))),
                FieldShape::Oneof { .. } => {},
            }
        }
        for oid in &message.oneofs {
            let oneof = schema.oneof(*oid);
            let slot = member(&oneof.name);
            source.push_str(&format!("match(&a.{slot},&b.{slot}){{(None,None)=>{{}},"));
            for fid in &oneof.fields {
                let field = schema.field(*fid);
                let FieldShape::Oneof { value, .. } = field.shape else {
                    unreachable!()
                };
                let variant = ident(&field.name.to_upper_camel_case());
                source.push_str(&format!(
                    "(Some({}::{variant}(a)),Some({}::{variant}(b)))=>{{{}}},",
                    new.oneof_path(*oid),
                    old.oneof_path(*oid),
                    new.oracle_value(value, "a", "b")
                ));
            }
            source.push_str("_=>panic!(\"oneof discriminator mismatch\"),}");
        }
        source.push_str("}\n");
        source.push_str(&format!(
            "#[test]fn m{index}(){{compare::<{},{}>(assert_m{index},check_v{index},&[",
            new.message_path(id),
            old.message_path(id)
        ));
        for fid in &message.fields {
            let field = schema.field(*fid);
            let value = match field.shape {
                FieldShape::Singular { value, .. }
                | FieldShape::Repeated { value, .. }
                | FieldShape::Oneof { value, .. } => value,
                FieldShape::Map { entry, .. } => {
                    ValueType::Message(entry, MessageEncoding::LengthPrefixed)
                }
            };
            let kind = match value {
                ValueType::Message(..) => "Message",
                ValueType::Scalar(Scalar::String) => "String",
                ValueType::Scalar(Scalar::Bytes) => "Bytes",
                _ => match Emitter::wire(value) {
                    0 => "Varint",
                    1 => "Fixed64",
                    5 => "Fixed32",
                    _ => unreachable!(),
                },
            };
            if let ValueType::Message(child, _) = value {
                let seed = oracle_seed(schema, child, 1);
                source.push_str(&format!(
                    "({},Kind::Message(&{seed:?})),",
                    field.number.get()
                ));
            } else {
                source.push_str(&format!("({},Kind::{kind}),", field.number.get()));
            }
        }
        source.push_str("]);}\n");
    }
    source.push_str(&generate_view_oracle(schema));
    let parsed = syn::parse_file(&source)?;
    Ok(prettyplease::unparse(&parsed))
}

fn generate_view_oracle(schema: &Schema) -> String {
    let new = Emitter {
        schema,
        base: "rt::generated::whatsapp",
    };
    let old = Emitter {
        schema,
        base: "waproto::reference",
    };
    let mut out = String::new();
    let value_assert = |value, a: &str, b: &str| {
        new.oracle_value(value, a, b)
            .replace("assert_m", "assert_v")
    };
    for (index, message) in schema.messages.iter().enumerate() {
        if message.map_entry {
            continue;
        }
        let id = MessageId(index);
        out.push_str(&format!(
            "fn assert_v{index}(a:&{}<'_>,b:&{}<'_>){{let _=(a,b);",
            new.view_path(id),
            old.view_path(id)
        ));
        for fid in &message.fields {
            let field = schema.field(*fid);
            let slot = member(&field.name);
            match field.shape{
            FieldShape::Singular{value:ValueType::Message(child,_),..}=>out.push_str(&format!("assert_eq!(a.{slot}.is_set(),b.{slot}.is_set());if let(Some(a),Some(b))=(a.{slot}.as_option(),b.{slot}.as_option()){{assert_v{}(a,b);}}",child.0)),
            FieldShape::Singular{value,presence:Presence::Explicit}=>out.push_str(&format!("match(&a.{slot},&b.{slot}){{(Some(a),Some(b))=>{{{}}},(None,None)=>{{}},_=>panic!(\"borrowed presence mismatch\"),}}",value_assert(value,"a","b"))),
            FieldShape::Singular{value,..}=>out.push_str(&value_assert(value,&format!("&a.{slot}"),&format!("&b.{slot}"))),
            FieldShape::Repeated{value,..}=>out.push_str(&format!("assert_eq!(a.{slot}.len(),b.{slot}.len());for(a,b)in a.{slot}.iter().zip(b.{slot}.iter()){{{}}}",value_assert(value,"a","b"))),
            FieldShape::Map{key,value,..}=>out.push_str(&format!("assert_eq!(a.{slot}.len(),b.{slot}.len());for((ak,av),(bk,bv))in a.{slot}.iter().zip(b.{slot}.iter()){{{}{}}}",value_assert(ValueType::Scalar(key),"ak","bk"),value_assert(value,"av","bv"))),
            FieldShape::Oneof{..}=>{},
        }
        }
        for oid in &message.oneofs {
            let oneof = schema.oneof(*oid);
            let slot = member(&oneof.name);
            out.push_str(&format!("match(&a.{slot},&b.{slot}){{(None,None)=>{{}},"));
            for fid in &oneof.fields {
                let field = schema.field(*fid);
                let FieldShape::Oneof { value, .. } = field.shape else {
                    unreachable!()
                };
                let variant = ident(&field.name.to_upper_camel_case());
                out.push_str(&format!(
                    "(Some({}::{variant}(a)),Some({}::{variant}(b)))=>{{{}}},",
                    new.view_oneof_path(*oid),
                    old.view_oneof_path(*oid),
                    value_assert(value, "a", "b")
                ));
            }
            out.push_str("_=>panic!(\"borrowed oneof discriminator mismatch\"),}");
        }
        out.push_str("}\n");
        out.push_str(&format!("fn check_v{index}(input:&[u8]){{let a=<{}<'_>as rt::MessageView>::decode_view(input);let b=<{}<'_>as buffa::MessageView>::decode_view(input);match(a,b){{(Ok(a),Ok(b))=>{{assert_v{index}(&a,&b);let ao=rt::MessageView::to_owned_message(&a).expect(\"valid view\");let bo=buffa::MessageView::to_owned_message(&b).expect(\"valid reference view\");assert_m{index}(&ao,&bo);assert_eq!(rt::ViewEncode::encode_to_vec(&a),buffa::ViewEncode::encode_to_vec(&b));let handle=<{} as rt::HasMessageView>::decode_view_handle(bytes::Bytes::copy_from_slice(input)).expect(\"valid handle\");assert_v{index}(handle.view(),&b);assert_m{index}(&handle.to_owned_message(),&bo);let cloned=handle.clone();drop(handle);assert_v{index}(cloned.view(),&b);assert_eq!(cloned.into_bytes().as_ref(),input);}},(Err(a),Err(b))=>assert_eq!(format!(\"{{a:?}}\"),format!(\"{{b:?}}\")),(a,b)=>panic!(\"borrowed rejection mismatch {{a:?}} {{b:?}}\"),}}}}\n",new.view_path(id),old.view_path(id),new.message_path(id)));
    }
    out
}

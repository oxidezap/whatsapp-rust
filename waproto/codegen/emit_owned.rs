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
        let mut out = format!(
            "#[derive(Clone,Debug,PartialEq,::serde::Serialize)] #[cfg_attr(feature=\"serde-deserialize\",derive(::serde::Deserialize),serde(default))] #[non_exhaustive] pub struct {name}{{\n"
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
        for id in &message.oneofs {
            root.at(&emitter.scope(Some(MessageId(index))))
                .items
                .push_str(&emitter.oneof_source(schema.oneof(*id)));
        }
    }
    let mut source = String::new();
    root.render(&mut source);
    source
}

impl Emitter<'_> {
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

pub fn generate_oracle(schema: &Schema) -> Result<String, syn::Error> {
    let new = Emitter {
        schema,
        base: "rt::generated::whatsapp",
    };
    let old = Emitter {
        schema,
        base: "waproto::whatsapp",
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
            "#[test]fn m{index}(){{compare::<{},{}>(assert_m{index},&[",
            new.message_path(id),
            old.message_path(id)
        ));
        for fid in &message.fields {
            let field = schema.field(*fid);
            let value = match field.shape {
                FieldShape::Singular { value, .. }
                | FieldShape::Repeated { value, .. }
                | FieldShape::Oneof { value, .. } => value,
                FieldShape::Map { .. } => ValueType::Message(id, MessageEncoding::LengthPrefixed),
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
            source.push_str(&format!("({},Kind::{kind}),", field.number.get()));
        }
        source.push_str("]);}\n");
    }
    let parsed = syn::parse_file(&source)?;
    Ok(prettyplease::unparse(&parsed))
}

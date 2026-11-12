use buffa_descriptor::generated::descriptor::{
    DescriptorProto, FieldDescriptorProto, FileDescriptorSet,
    field_descriptor_proto::{Label, Type},
};
use heck::ToSnakeCase as _;
use std::path::Path;

const ROOTS: &[&str] = &[
    "MessageKey",
    "ExternalBlobReference",
    "KeyId",
    "SyncdIndex",
    "SyncdValue",
    "SessionStructure.Chain.ChainKey",
    "SessionStructure.Chain.MessageKey",
    "MessageSecretMessage",
    "Money",
    "Point",
    "Location",
    "EphemeralSetting",
    "Message.ChatAnimatedWallpaper",
    "Message.ChatSolidColorWallpaper",
];

struct Scalar {
    owned: &'static str,
    borrowed: &'static str,
    wire: u8,
    read: &'static str,
    length: &'static str,
    write: &'static str,
    promote: &'static str,
}

fn scalar(kind: Type, bytes: bool) -> Scalar {
    let (owned, borrowed, wire, read, length, write, promote) = match kind {
        Type::TYPE_STRING => (
            "String",
            "&'a str",
            2,
            "r.string()?",
            "wire::varint_len(value.len() as u64) + value.len()",
            "wire::varint(value.len() as u64, output); output.extend_from_slice(value.as_bytes());",
            "value.to_owned()",
        ),
        Type::TYPE_BYTES => (
            if bytes { "bytes::Bytes" } else { "Vec<u8>" },
            "&'a [u8]",
            2,
            "r.bytes()?",
            "wire::varint_len(value.len() as u64) + value.len()",
            "wire::varint(value.len() as u64, output); output.extend_from_slice(value);",
            if bytes {
                "bytes::Bytes::copy_from_slice(value)"
            } else {
                "value.to_vec()"
            },
        ),
        Type::TYPE_BOOL => (
            "bool",
            "bool",
            0,
            "r.varint()? != 0",
            "1",
            "wire::varint(u64::from(*value), output);",
            "value",
        ),
        Type::TYPE_UINT32 => (
            "u32",
            "u32",
            0,
            "r.varint()? as u32",
            "wire::varint_len(u64::from(*value))",
            "wire::varint(u64::from(*value), output);",
            "value",
        ),
        Type::TYPE_UINT64 => (
            "u64",
            "u64",
            0,
            "r.varint()?",
            "wire::varint_len(*value)",
            "wire::varint(*value, output);",
            "value",
        ),
        Type::TYPE_INT32 => (
            "i32",
            "i32",
            0,
            "r.varint()? as i32",
            "wire::varint_len(*value as u64)",
            "wire::varint(*value as u64, output);",
            "value",
        ),
        Type::TYPE_INT64 => (
            "i64",
            "i64",
            0,
            "r.varint()? as i64",
            "wire::varint_len(*value as u64)",
            "wire::varint(*value as u64, output);",
            "value",
        ),
        Type::TYPE_FIXED32 => (
            "u32",
            "u32",
            5,
            "r.fixed32()?",
            "4",
            "output.extend_from_slice(&value.to_le_bytes());",
            "value",
        ),
        Type::TYPE_SFIXED32 => (
            "i32",
            "i32",
            5,
            "r.fixed32()? as i32",
            "4",
            "output.extend_from_slice(&value.to_le_bytes());",
            "value",
        ),
        Type::TYPE_SFIXED64 => (
            "i64",
            "i64",
            1,
            "r.fixed64()? as i64",
            "8",
            "output.extend_from_slice(&value.to_le_bytes());",
            "value",
        ),
        Type::TYPE_DOUBLE => (
            "f64",
            "f64",
            1,
            "f64::from_bits(r.fixed64()?)",
            "8",
            "output.extend_from_slice(&value.to_bits().to_le_bytes());",
            "value",
        ),
        Type::TYPE_FLOAT => (
            "f32",
            "f32",
            5,
            "f32::from_bits(r.fixed32()?)",
            "4",
            "output.extend_from_slice(&value.to_bits().to_le_bytes());",
            "value",
        ),
        other => panic!("unqualified scalar in experiment: {other:?}"),
    };
    Scalar {
        owned,
        borrowed,
        wire,
        read,
        length,
        write,
        promote,
    }
}

fn locate<'a>(fds: &'a FileDescriptorSet, path: &str) -> &'a DescriptorProto {
    let mut messages = fds.file.iter().flat_map(|file| &file.message_type);
    let mut parts = path.split('.');
    let first = parts.next().expect("root name");
    let mut message = messages
        .find(|m| m.name.as_deref() == Some(first))
        .expect("schema root");
    for part in parts {
        message = message
            .nested_type
            .iter()
            .find(|m| m.name.as_deref() == Some(part))
            .expect("nested schema root");
    }
    message
}

fn field_scalar(path: &str, field: &FieldDescriptorProto) -> Scalar {
    assert_eq!(field.label, Some(Label::LABEL_OPTIONAL));
    assert!(field.oneof_index.is_none());
    scalar(
        field.r#type.expect("field kind"),
        path.starts_with("SessionStructure.Chain."),
    )
}

pub fn generate(fds: &FileDescriptorSet, output: &Path) -> std::io::Result<()> {
    let mut source = String::from("// @generated by experiment/generate.rs. Do not edit.\n");
    let mut checks = String::new();
    for path in ROOTS {
        let message = locate(fds, path);
        let name = path.replace('.', "");
        let view = format!("{name}View");
        let mut fields: Vec<_> = message.field.iter().collect();
        fields.sort_by_key(|field| field.number);
        source.push_str(&format!(
            "#[derive(Clone, Debug, Default, PartialEq)] #[non_exhaustive] pub struct {name} {{\n"
        ));
        for field in &fields {
            let scalar = field_scalar(path, field);
            source.push_str(&format!(
                "pub {}: Option<{}>,\n",
                field.name.as_deref().expect("field name").to_snake_case(),
                scalar.owned
            ));
        }
        source.push_str("}\n");
        source.push_str(&format!(
            "#[derive(Clone, Debug, Default, PartialEq)] #[non_exhaustive] pub struct {view}<'a> {{\n"
        ));
        for field in &fields {
            source.push_str(&format!(
                "pub {}: Option<{}>,\n",
                field.name.as_deref().expect("field name").to_snake_case(),
                field_scalar(path, field).borrowed
            ));
        }
        source.push_str("lifetime: std::marker::PhantomData<&'a ()>,\n}\n");
        source.push_str(&format!("impl<'a> {view}<'a> {{ pub fn decode(input: &'a [u8]) -> Result<Self, wire::Error> {{ let mut r = wire::Reader::new(input)?; let mut result = Self::default(); while !r.is_empty() {{ let (field, kind) = r.tag()?; match field {{\n"));
        let wallpaper = *path == "Message.ChatSolidColorWallpaper";
        for field in &fields {
            let scalar = field_scalar(path, field);
            let number = field.number.expect("field number");
            let field_name = field.name.as_deref().expect("field name").to_snake_case();
            let guard = if wallpaper && (number == 1 || number == 2) {
                format!(" if kind == {}", scalar.wire)
            } else {
                String::new()
            };
            source.push_str(&format!("{number}{guard} => {{ wire::check(field, kind, {})?; result.{field_name} = Some({}); }},\n", scalar.wire, scalar.read));
        }
        if wallpaper {
            source.push_str("1 | 2 => { wire::check(field, kind, 5)?; unreachable!() },\n");
        }
        source.push_str("_ => r.skip(field, kind, wire::MAX_DEPTH)?,\n} } Ok(result) }\n");
        source.push_str(&format!(
            "pub fn to_owned_message(&self) -> {name} {{ {name} {{\n"
        ));
        for field in &fields {
            let scalar = field_scalar(path, field);
            let name = field.name.as_deref().expect("name").to_snake_case();
            if scalar.promote == "value" {
                source.push_str(&format!("{name}: self.{name},\n"));
                continue;
            }
            if scalar.promote == "bytes::Bytes::copy_from_slice(value)" {
                source.push_str(&format!(
                    "{name}: self.{name}.map(bytes::Bytes::copy_from_slice),\n"
                ));
                continue;
            }
            source.push_str(&format!(
                "{}: self.{}.map(|value| {}),\n",
                field.name.as_deref().expect("name").to_snake_case(),
                field.name.as_deref().expect("name").to_snake_case(),
                scalar.promote
            ));
        }
        source.push_str("} } }\n");
        source.push_str(&format!("impl {name} {{ pub fn decode(input: &[u8]) -> Result<Self, wire::Error> {{ Ok({view}::decode(input)?.to_owned_message()) }}\n pub fn encoded_len(&self) -> usize {{ let mut len = 0usize;\n"));
        for field in &fields {
            let scalar = field_scalar(path, field);
            let tag = ((field.number.expect("number") as u64) << 3) | u64::from(scalar.wire);
            source.push_str(&format!("if let Some(value) = &self.{} {{ let _ = value; len = len.saturating_add(wire::varint_len({tag}) + {}); }}\n", field.name.as_deref().expect("name").to_snake_case(), scalar.length));
        }
        source.push_str("assert!(len <= wire::MAX_SIZE, \"protobuf message size limit\"); len }\n pub fn encode_into(&self, output: &mut Vec<u8>) { let _ = self.encoded_len();\n");
        for field in &fields {
            let scalar = field_scalar(path, field);
            let tag = ((field.number.expect("number") as u64) << 3) | u64::from(scalar.wire);
            source.push_str(&format!(
                "if let Some(value) = &self.{} {{ wire::varint({tag}, output); {} }}\n",
                field.name.as_deref().expect("name").to_snake_case(),
                scalar.write
            ));
        }
        source.push_str("}\n}\n");
        let parts: Vec<_> = path.split('.').collect();
        let mut oracle = String::from("waproto::whatsapp::");
        for part in &parts[..parts.len() - 1] {
            oracle.push_str(&part.to_snake_case());
            oracle.push_str("::");
        }
        oracle.push_str(parts.last().expect("type"));
        let test_start = source.len();
        source.push_str(&format!("#[cfg(test)] #[test] fn differential_{}() {{ crate::differential::<{oracle}, {name}>({name}::decode, &[", name.to_snake_case()));
        for field in &fields {
            let scalar = field_scalar(path, field);
            source.push_str(&format!(
                "({}, {}),",
                field.number.expect("number"),
                scalar.wire
            ));
        }
        source.push_str("]); }\n");
        source.push_str(&format!("#[cfg(test)] impl crate::Candidate for {name} {{ type Reference = {oracle}; fn encode_bytes(&self) -> Vec<u8> {{ let mut bytes = Vec::with_capacity(self.encoded_len()); self.encode_into(&mut bytes); assert_eq!(bytes.len(), self.encoded_len()); bytes }} fn borrowed_to_owned(input: &[u8]) -> Result<Self, wire::Error> {{ Ok({view}::decode(input)?.to_owned_message()) }} fn assert_reference(&self, expected: &Self::Reference) {{\n"));
        for field in &fields {
            let field_name = field.name.as_deref().expect("field name").to_snake_case();
            let bits = match field.r#type {
                Some(Type::TYPE_DOUBLE) => ".map(f64::to_bits)",
                Some(Type::TYPE_FLOAT) => ".map(f32::to_bits)",
                _ => "",
            };
            source.push_str(&format!("assert_eq!(self.{field_name}{bits}, expected.{field_name}{bits}, \"{name}.{field_name}\");\n"));
        }
        source.push_str(&format!("}} fn assert_borrowed(input: &[u8], expected: &Self::Reference) {{ let view = {view}::decode(input).expect(\"valid candidate view\");\n"));
        for field in &fields {
            let field_name = field.name.as_deref().expect("field name").to_snake_case();
            let (actual, expected) = match field.r#type {
                Some(Type::TYPE_STRING | Type::TYPE_BYTES) => ("", ".as_deref()"),
                Some(Type::TYPE_DOUBLE) => (".map(f64::to_bits)", ".map(f64::to_bits)"),
                Some(Type::TYPE_FLOAT) => (".map(f32::to_bits)", ".map(f32::to_bits)"),
                _ => ("", ""),
            };
            source.push_str(&format!("assert_eq!(view.{field_name}{actual}, expected.{field_name}{expected}, \"{view}.{field_name}\");\n"));
        }
        source.push_str("} }\n");
        checks.push_str(&source.split_off(test_start));
    }
    let file = syn::parse_file(&source).map_err(std::io::Error::other)?;
    std::fs::write(output.join("experiment.rs"), prettyplease::unparse(&file))?;
    let checks = syn::parse_file(&checks).map_err(std::io::Error::other)?;
    std::fs::write(
        output.join("experiment_tests.rs"),
        prettyplease::unparse(&checks),
    )
}

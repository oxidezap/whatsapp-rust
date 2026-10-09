//! Test-only schema fixtures, emitted with the production generator and policy.
use super::{emission, names};
use buffa::Message;
use buffa_descriptor::generated::descriptor::*;
use field_descriptor_proto::{Label, Type};

fn schema(new: bool) -> FileDescriptorSet {
    let message = if new { "Entry" } else { "Record" };
    let mode = if new { "State" } else { "Mode" };
    let mut record = DescriptorProto {
        name: Some(message.into()),
        ..Default::default()
    };
    record.enum_type.push(EnumDescriptorProto {
        name: Some(mode.into()),
        value: vec![EnumValueDescriptorProto {
            name: Some(if new { "READY_NOW" } else { "READY" }.into()),
            number: Some(0),
            ..Default::default()
        }],
        ..Default::default()
    });
    record.oneof_decl.push(OneofDescriptorProto {
        name: Some(if new { "selection" } else { "choice" }.into()),
        ..Default::default()
    });
    let field = |name: &str, number, ty| FieldDescriptorProto {
        name: Some(name.into()),
        json_name: Some(name.into()),
        number: Some(number),
        r#type: Some(ty),
        label: Some(Label::LABEL_OPTIONAL),
        ..Default::default()
    };
    record.field.push(field(
        if new { "displayName" } else { "name" },
        1,
        Type::TYPE_STRING,
    ));
    let mut mode_field = field("mode", 2, Type::TYPE_ENUM);
    record.enum_type[0].value.push(EnumValueDescriptorProto {
        name: Some("OTHER".into()),
        number: Some(2),
        ..Default::default()
    });
    record.enum_type[0].value.push(EnumValueDescriptorProto {
        name: Some("NEGATIVE".into()),
        number: Some(-1),
        ..Default::default()
    });
    mode_field.type_name = Some(format!(".contract.{message}.{mode}"));
    mode_field.default_value = Some(if new { "READY_NOW" } else { "READY" }.into());
    record.field.push(mode_field);
    let mut text = field("text", 3, Type::TYPE_STRING);
    text.oneof_index = Some(0);
    record.field.push(text);
    record.nested_type.push(DescriptorProto {
        name: Some("Child".into()),
        field: vec![
            field("left", 1, Type::TYPE_UINT32),
            field("right", 2, Type::TYPE_UINT32),
        ],
        ..Default::default()
    });
    let mut values = field("values", 3, Type::TYPE_UINT32);
    values.label = Some(Label::LABEL_REPEATED);
    record.nested_type[0].field.push(values);
    let mut back = field("next", 8, Type::TYPE_MESSAGE);
    back.type_name = Some(format!(".contract.{message}"));
    record.field.push(back);
    let mut child = field("child", 6, Type::TYPE_MESSAGE);
    child.type_name = Some(format!(".contract.{message}.Child"));
    record.field.push(child);
    let mut detail = field("detail", 7, Type::TYPE_MESSAGE);
    detail.type_name = Some(format!(".contract.{message}.Child"));
    detail.oneof_index = Some(0);
    record.field.push(detail);
    if new {
        record.field.push(field("extra", 4, Type::TYPE_UINT32));
        let mut bytes = field("bytes", 5, Type::TYPE_BYTES);
        bytes.oneof_index = Some(0);
        record.field.push(bytes);
        record.enum_type[0].value.push(EnumValueDescriptorProto {
            name: Some("NEW".into()),
            number: Some(1),
            ..Default::default()
        });
    }
    let mut enum_mode = field("mode", 2, Type::TYPE_ENUM);
    enum_mode.type_name = Some(format!(".contract.{message}.{mode}"));
    let mut enum_other = enum_mode.clone();
    enum_other.name = Some("other".into());
    enum_other.json_name = Some("other".into());
    enum_other.number = Some(3);
    let mut enum_type = enum_mode.clone();
    enum_type.name = Some("type".into());
    enum_type.json_name = Some("type".into());
    enum_type.number = Some(4);
    let enum_record = DescriptorProto {
        name: Some("EnumRecord".into()),
        field: vec![
            field("label", 1, Type::TYPE_UINT32),
            enum_mode,
            enum_other,
            enum_type,
        ],
        ..Default::default()
    };
    let mut modes = field("modes", 1, Type::TYPE_ENUM);
    modes.type_name = Some(format!(".contract.{message}.{mode}"));
    modes.label = Some(Label::LABEL_REPEATED);
    let mut packed_modes = modes.clone();
    packed_modes.name = Some("packed_modes".into());
    packed_modes.json_name = Some("packedModes".into());
    packed_modes.number = Some(2);
    packed_modes.options = ::buffa::MessageField::some(FieldOptions {
        packed: Some(true),
        ..Default::default()
    });
    let repeated_record = DescriptorProto {
        name: Some("RepeatedRecord".into()),
        field: vec![modes, packed_modes],
        ..Default::default()
    };
    FileDescriptorSet {
        file: vec![FileDescriptorProto {
            name: Some("contract.proto".into()),
            package: Some("contract".into()),
            syntax: Some("proto2".into()),
            message_type: vec![record, enum_record, repeated_record],
            ..Default::default()
        }],
        ..Default::default()
    }
}
pub fn generate(root: &std::path::Path) -> std::io::Result<()> {
    let mut previous: Option<String> = None;
    for new in [false, true] {
        let mut fds = schema(new);
        if new {
            names::apply(
                &mut fds,
                &[
                    names::TypeName {
                        upstream: ".contract.Entry",
                        frozen: ".contract.Record",
                    },
                    names::TypeName {
                        upstream: ".contract.Record.State",
                        frozen: ".contract.Record.Mode",
                    },
                ],
                &[names::FieldName {
                    message: ".contract.Record",
                    number: 1,
                    upstream: "displayName",
                    frozen: "name",
                }],
                &[names::EnumName {
                    enumeration: ".contract.Record.Mode",
                    number: 0,
                    upstream: "READY_NOW",
                    frozen: "READY",
                }],
                &[names::OneofName {
                    message: ".contract.Record",
                    upstream: "selection",
                    frozen: "choice",
                    numbers: &[3],
                }],
            )?;
            assert_eq!(
                fds.file[0].message_type[0].field[0].json_name.as_deref(),
                Some("displayName")
            );
        }
        let out = root.join(if new { "v2" } else { "v1" });
        let api = emit(&fds, &out, buffa_build::PointerRepr::Box)?;
        if let Some(old) = previous {
            emission::check_api(&old, &api)?;
        }
        previous = Some(api.into_iter().collect::<Vec<_>>().join("\n"));
    }
    // Exercise the guard with real generator output, including a generator
    // ownership setting that changes Rust types without changing wire bytes.
    let baseline = emit(
        &schema(false),
        &root.join("baseline"),
        buffa_build::PointerRepr::Box,
    )?
    .into_iter()
    .collect::<Vec<_>>()
    .join("\n");
    let mut changed_type = schema(false);
    changed_type.file[0].message_type[0].field[0].r#type = Some(Type::TYPE_BYTES);
    for (name, fds, pointer) in [
        (
            "unreviewed-rename",
            schema(true),
            buffa_build::PointerRepr::Box,
        ),
        ("changed-type", changed_type, buffa_build::PointerRepr::Box),
        (
            "changed-ownership",
            schema(false),
            buffa_build::PointerRepr::Inline,
        ),
    ] {
        let incompatible = emit(&fds, &root.join(name), pointer)?;
        assert!(
            emission::check_api(&baseline, &incompatible).is_err(),
            "{name} must be rejected"
        );
    }
    Ok(())
}

#[allow(clippy::disallowed_methods)] // descriptor fixture, not an application message
fn emit(
    fds: &FileDescriptorSet,
    out: &std::path::Path,
    pointer: buffa_build::PointerRepr,
) -> std::io::Result<std::collections::BTreeSet<String>> {
    std::fs::create_dir_all(out)?;
    let desc = out.join("contract.desc");
    let bytes = fds.encode_to_vec();
    if !std::fs::read(&desc).is_ok_and(|current| current == bytes) {
        std::fs::write(&desc, bytes)?;
    }
    buffa_build::Config::new()
        .descriptor_set(desc)
        .files(&["contract.proto"])
        .idiomatic_field_names(true)
        .box_type(pointer)
        .generate_views(true)
        .preserve_unknown_fields(true)
        .message_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
        .message_attribute(".", "#[serde(default)]")
        .enum_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
        .oneof_attribute(".", "#[derive(serde::Serialize, serde::Deserialize)]")
        .out_dir(out)
        .compile()
        .map_err(|e| std::io::Error::other(e.to_string()))?;
    let mut api = emission::finish(out, "contract")?;
    api.extend(names::wire_api(fds));
    Ok(api)
}

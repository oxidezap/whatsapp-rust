#![forbid(unsafe_code)]
#![allow(clippy::disallowed_methods)]

#[path = "../build_support/api_guard.rs"]
mod api_guard;
#[path = "../codegen/descriptor.rs"]
pub mod descriptor;
#[path = "../codegen/features.rs"]
pub mod features;
#[path = "../codegen/names.rs"]
pub mod names;
#[path = "../codegen/retained.rs"]
pub mod retained;
#[path = "../codegen/wallpaper.rs"]
pub mod wallpaper;

#[path = "../codegen/emit_owned.rs"]
pub mod emit_owned;
#[path = "../codegen/indexed_schema.rs"]
pub mod indexed_schema;
#[path = "../codegen/policy.rs"]
pub mod policy;
#[path = "../codegen/project.rs"]
pub mod project;
#[path = "../codegen/schema.rs"]
pub mod schema;

use descriptor::FileDescriptorSet;

#[test]
fn indexed_schema_generation_is_deterministic_and_valid_rust() {
    let descriptor =
        std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/src/whatsapp.desc")).unwrap();
    let mut descriptor = FileDescriptorSet::decode_from_slice(&descriptor).unwrap();
    names::apply(
        &mut descriptor,
        names::TYPES,
        names::FIELDS,
        names::ENUM_VALUES,
        names::ONEOFS,
    )
    .unwrap();
    retained::apply_local_fields(&mut descriptor).unwrap();
    wallpaper::retain_fields(&mut descriptor).unwrap();
    let schema = project::project(&descriptor).unwrap();
    let first = indexed_schema::generate(&schema).unwrap();
    assert_eq!(first, indexed_schema::generate(&schema).unwrap());
    syn::parse_file(&first).unwrap();
    assert_eq!(
        first.matches("Schema{identity:").count(),
        schema.messages.len()
    );
}

#[test]
fn indexed_schema_preserves_field_shapes() {
    use schema::*;
    let enumeration = Enumeration {
        full_name: ".test.Code".into(),
        name: "Code".into(),
        parent: None,
        open: true,
        values: vec![
            EnumValue {
                name: "NEGATIVE".into(),
                number: -1,
            },
            EnumValue {
                name: "TWO".into(),
                number: 2,
            },
        ],
    };
    let message = Message {
        full_name: ".test.Record".into(),
        name: "Record".into(),
        parent: None,
        fields: vec![FieldId(0), FieldId(1), FieldId(2)],
        oneofs: vec![OneofId(0)],
        map_entry: false,
    };
    let entry = Message {
        full_name: ".test.Entry".into(),
        name: "Entry".into(),
        parent: None,
        fields: vec![FieldId(3), FieldId(4)],
        oneofs: vec![],
        map_entry: true,
    };
    let make_field = |owner, name: &str, number, shape| Field {
        owner,
        name: name.into(),
        number: FieldNumber::new(number).unwrap(),
        shape,
        default_literal: None,
        bytes_storage: BytesStorage::Vector,
    };
    let schema = Schema {
        messages: vec![message, entry],
        enumerations: vec![enumeration],
        fields: vec![
            make_field(
                MessageId(0),
                "required_text",
                1,
                FieldShape::Singular {
                    value: ValueType::Scalar(Scalar::String),
                    presence: Presence::Required,
                },
            ),
            make_field(
                MessageId(0),
                "codes",
                2,
                FieldShape::Repeated {
                    value: ValueType::Enumeration(EnumId(0)),
                    packed: true,
                },
            ),
            make_field(
                MessageId(0),
                "selected",
                3,
                FieldShape::Oneof {
                    value: ValueType::Scalar(Scalar::Bytes),
                    owner: OneofId(0),
                },
            ),
            make_field(
                MessageId(1),
                "key",
                1,
                FieldShape::Singular {
                    value: ValueType::Scalar(Scalar::String),
                    presence: Presence::Implicit,
                },
            ),
            make_field(
                MessageId(1),
                "value",
                2,
                FieldShape::Singular {
                    value: ValueType::Enumeration(EnumId(0)),
                    presence: Presence::Implicit,
                },
            ),
        ],
        oneofs: vec![Oneof {
            owner: MessageId(0),
            name: "choice".into(),
            fields: vec![FieldId(2)],
        }],
    };
    let generated = indexed_schema::generate(&schema).unwrap();
    assert!(generated.contains("required_prefix:1,fields:&["));
    assert!(generated.contains("Spec::new(2,Kind::Enum,FieldOptions{repeated:true,packed:true,required:false,open:true},None,&[-1,2])"));
    assert!(generated.contains("Spec::new(3,Kind::Bytes,FieldOptions{repeated:false,packed:false,required:false,open:false},Some(0),&[])"));
    assert!(generated.contains("Spec::new(1,Kind::String,FieldOptions{repeated:false,packed:false,required:false,open:false},None,&[])"));
    assert!(generated.contains("Spec::new(2,Kind::Enum,FieldOptions{repeated:false,packed:false,required:false,open:true},None,&[-1,2])"));
}

#[test]
fn private_codecs_are_current_and_deterministic() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    for (descriptor, base, output) in [
        (
            "wacore/src/voip/mlow/tables.desc",
            "crate::voip::mlow::smpl_tables_blob::tables",
            "wacore/src/voip/mlow/tables.generated.rs",
        ),
        (
            "storages/sqlite-storage/proto/wire.desc",
            "crate::wire::proto",
            "storages/sqlite-storage/src/wire.generated.rs",
        ),
    ] {
        let descriptor = match std::fs::read(root.join(descriptor)) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                eprintln!(
                    "workspace-only private codec freshness gate NOT_EXECUTED: schema source is outside the waproto package"
                );
                return;
            }
            Err(error) => panic!("read descriptor: {error}"),
        };
        let descriptor = FileDescriptorSet::decode_from_slice(&descriptor).unwrap();
        let schema = project::project(&descriptor).unwrap();
        let first =
            emit_owned::generate_private_owned(&schema, base, "::waproto::protobuf").unwrap();
        let second =
            emit_owned::generate_private_owned(&schema, base, "::waproto::protobuf").unwrap();
        assert_eq!(first, second);
        let expected = format!("// @generated by protobuf_codegen. Do not edit.\n{first}");
        assert_eq!(
            std::fs::read_to_string(root.join(output)).unwrap(),
            expected
        );
        assert!(!expected.contains("serde::"));
        assert!(!expected.contains("pub struct HeapWindowView"));
        assert!(!expected.contains("pub struct HashStateWireView"));
    }
}

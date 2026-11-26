#![forbid(unsafe_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write as _;

use buffa::{Enumeration as _, Message as _};
use buffa_descriptor::features::{
    self as reference_features, FieldPresence, RepeatedFieldEncoding, ResolvedFeatures,
};
use buffa_descriptor::generated::descriptor::{
    DescriptorProto, EnumDescriptorProto, FileDescriptorSet,
    field_descriptor_proto::{Label, Type},
};
use serde::Serialize;

#[path = "../codegen/descriptor.rs"]
pub mod descriptor;
#[path = "../codegen/features.rs"]
pub mod features;

#[path = "../codegen/policy.rs"]
pub mod policy;
#[path = "../codegen/project.rs"]
pub mod project;
#[path = "../codegen/schema.rs"]
pub mod schema;

#[derive(Default, Serialize)]
struct Inventory {
    files: usize,
    public_messages: usize,
    synthetic_map_entries: usize,
    enumerations: usize,
    enum_values: usize,
    fields: usize,
    real_oneofs: usize,
    synthetic_oneofs: usize,
    fields_by_type: BTreeMap<String, usize>,
    fields_by_presence: BTreeMap<String, usize>,
    packed_fields: Vec<String>,
    expanded_packable_fields: Vec<String>,
    maps: Vec<String>,
    explicit_defaults: BTreeMap<String, String>,
    message_edges: Vec<MessageEdge>,
    repeated_field_numbers: BTreeMap<String, Vec<String>>,
}

#[derive(Serialize)]
struct MessageEdge {
    source: String,
    field: String,
    target: String,
}

fn enums(items: &[EnumDescriptorProto], inventory: &mut Inventory) {
    inventory.enumerations += items.len();
    inventory.enum_values += items.iter().map(|e| e.value.len()).sum::<usize>();
}

fn map_entries(items: &[DescriptorProto], parent: &str, output: &mut BTreeSet<String>) {
    for message in items {
        let path = format!(
            "{parent}.{}",
            message.name.as_deref().expect("message name")
        );
        if message
            .options
            .as_option()
            .and_then(|options| options.map_entry)
            == Some(true)
        {
            output.insert(path.clone());
        }
        map_entries(&message.nested_type, &path, output);
    }
}

fn messages(
    items: &[DescriptorProto],
    parent: &str,
    inherited: &ResolvedFeatures,
    map_names: &BTreeSet<String>,
    inventory: &mut Inventory,
) {
    for message in items {
        let path = format!(
            "{parent}.{}",
            message.name.as_deref().expect("message name")
        );
        if map_names.contains(&path) {
            inventory.synthetic_map_entries += 1;
        } else {
            inventory.public_messages += 1;
        }
        enums(&message.enum_type, inventory);
        let resolved = reference_features::resolve_child(
            inherited,
            reference_features::message_features(message),
        );
        let mut numbers: BTreeMap<i32, Vec<String>> = BTreeMap::new();
        let synthetic_oneofs: BTreeSet<_> = message
            .field
            .iter()
            .filter(|field| field.proto3_optional == Some(true))
            .filter_map(|field| field.oneof_index)
            .collect();
        inventory.synthetic_oneofs += synthetic_oneofs.len();
        inventory.real_oneofs += message.oneof_decl.len() - synthetic_oneofs.len();
        for field in &message.field {
            inventory.fields += 1;
            let name = field.name.as_deref().expect("field name");
            let field_path = format!("{path}.{name}");
            let kind = field.r#type.expect("field type");
            *inventory
                .fields_by_type
                .entry(kind.proto_name().into())
                .or_default() += 1;
            numbers
                .entry(field.number.expect("field number"))
                .or_default()
                .push(format!("{name}:{}", kind.proto_name()));
            let resolved = reference_features::resolve_child(
                &resolved,
                reference_features::field_features(field),
            );
            let repeated = field.label == Some(Label::LABEL_REPEATED);
            let presence = if repeated {
                "repeated"
            } else if field.label == Some(Label::LABEL_REQUIRED)
                || resolved.field_presence == FieldPresence::LegacyRequired
            {
                "required"
            } else if field.proto3_optional == Some(true)
                || matches!(kind, Type::TYPE_MESSAGE | Type::TYPE_GROUP)
            {
                "explicit"
            } else {
                match resolved.field_presence {
                    FieldPresence::Explicit => "explicit",
                    FieldPresence::Implicit => "implicit",
                    FieldPresence::LegacyRequired => "required",
                    other => panic!("unqualified field presence: {other:?}"),
                }
            };
            *inventory
                .fields_by_presence
                .entry(presence.into())
                .or_default() += 1;
            if let Some(value) = &field.default_value {
                inventory
                    .explicit_defaults
                    .insert(field_path.clone(), value.clone());
            }
            if matches!(kind, Type::TYPE_MESSAGE | Type::TYPE_GROUP) {
                let target = field.type_name.as_deref().expect("message target");
                inventory.message_edges.push(MessageEdge {
                    source: path.clone(),
                    field: name.into(),
                    target: target.into(),
                });
                if map_names.contains(target) {
                    inventory.maps.push(field_path);
                }
            } else if repeated && !matches!(kind, Type::TYPE_STRING | Type::TYPE_BYTES) {
                let packed =
                    field.options.as_option().and_then(|o| o.packed).unwrap_or(
                        resolved.repeated_field_encoding == RepeatedFieldEncoding::Packed,
                    );
                if packed {
                    inventory.packed_fields.push(field_path);
                } else {
                    inventory.expanded_packable_fields.push(field_path);
                }
            }
        }
        for (number, alternatives) in numbers {
            if alternatives.len() > 1 {
                inventory
                    .repeated_field_numbers
                    .insert(format!("{path}#{number}"), alternatives);
            }
        }
        messages(
            &message.nested_type,
            &path,
            &reference_features::resolve_child(
                inherited,
                reference_features::message_features(message),
            ),
            map_names,
            inventory,
        );
    }
}

#[allow(clippy::disallowed_methods)]
fn inspect(bytes: &[u8]) -> Inventory {
    let descriptors = FileDescriptorSet::decode_from_slice(bytes).expect("locked descriptor");
    let input = descriptor::FileDescriptorSet::decode_from_slice(bytes).expect("locked descriptor");
    let projected = project::project(&input).expect("complete schema projection");
    let mut inventory = Inventory {
        files: descriptors.file.len(),
        ..Inventory::default()
    };
    let mut map_names = BTreeSet::new();
    for file in &descriptors.file {
        map_entries(
            &file.message_type,
            &format!(".{}", file.package.as_deref().unwrap_or_default()),
            &mut map_names,
        );
    }
    for file in &descriptors.file {
        enums(&file.enum_type, &mut inventory);
        messages(
            &file.message_type,
            &format!(".{}", file.package.as_deref().unwrap_or_default()),
            &reference_features::for_file(file),
            &map_names,
            &mut inventory,
        );
    }
    assert_eq!(
        projected.messages.len(),
        inventory.public_messages + inventory.synthetic_map_entries
    );
    assert_eq!(projected.fields.len(), inventory.fields);
    assert_eq!(projected.enumerations.len(), inventory.enumerations);
    assert_eq!(projected.oneofs.len(), inventory.real_oneofs);
    inventory
}

fn main() -> std::io::Result<()> {
    let snapshot = include_str!("../api.snapshot");
    let mut api_categories = BTreeMap::<_, usize>::new();
    let mut owned_view_reference_methods = Vec::new();
    for line in snapshot.lines() {
        let Some((category, rest)) = line.split_once(' ') else {
            continue;
        };
        *api_categories.entry(category).or_default() += 1;
        if category == "method"
            && let Some((owner, signature)) = rest.split_once(" fn ")
            && owner.ends_with("OwnedView")
            && let Some((_, output)) = signature.split_once(" -> & ")
            && output.contains("View")
        {
            owned_view_reference_methods.push(line);
        }
    }
    let result = serde_json::json!({
        "whatsapp": inspect(include_bytes!(concat!(env!("OUT_DIR"), "/whatsapp.local.desc"))),
        "mlow": inspect(include_bytes!("../../wacore/src/voip/mlow/tables.desc")),
        "sqlite": inspect(include_bytes!("../../storages/sqlite-storage/proto/wire.desc")),
        "frozen_api_categories": api_categories,
        "owned_view_reference_methods": owned_view_reference_methods,
    });
    writeln!(
        std::io::stdout().lock(),
        "{}",
        serde_json::to_string_pretty(&result)?
    )
}

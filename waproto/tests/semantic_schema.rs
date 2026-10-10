#![allow(clippy::disallowed_methods)]
#[path = "../build_support/wire_semantic_plan.rs"]
mod semantic_plan;
use buffa::Message as _;
use buffa_descriptor::generated::descriptor::{
    DescriptorProto, FieldDescriptorProto, FileDescriptorSet, OneofDescriptorProto,
    field_descriptor_proto::{Label, Type},
};
use semantic_plan::FieldMode;

fn descriptors() -> FileDescriptorSet {
    FileDescriptorSet::decode_from_slice(include_bytes!("../src/whatsapp.desc")).unwrap()
}
fn owner_mut<'a>(messages: &'a mut [DescriptorProto], path: &[&str]) -> &'a mut DescriptorProto {
    let message = messages
        .iter_mut()
        .find(|message| message.name.as_deref() == Some(path[0]))
        .unwrap();
    if path.len() == 1 {
        message
    } else {
        owner_mut(&mut message.nested_type, &path[1..])
    }
}
#[test]
fn actual_header_closure_distinguishes_root_projection_from_independent_children() {
    let plan = semantic_plan::plan(&descriptors());
    assert_eq!(plan[0].name, ".whatsapp.Message.InteractiveMessage.Header");
    assert!(!plan[0].reject_unknowns && !plan[0].reject_journal);
    assert!(
        plan[1..]
            .iter()
            .all(|owner| owner.reject_unknowns && owner.reject_journal)
    );
    let mode = |owner: usize, number| {
        plan[owner]
            .fields
            .iter()
            .find(|field| field.number == number)
            .unwrap()
            .mode
    };
    assert_eq!(mode(0, 1), FieldMode::Ignored);
    assert_eq!(mode(0, 4), FieldMode::Message(1));
    assert_eq!(mode(0, 6), FieldMode::Bytes);
    assert_eq!(mode(0, 3), FieldMode::Unsupported);
    assert_eq!(mode(1, 17), FieldMode::Message(2));
    assert_eq!(mode(1, 31), FieldMode::Unsupported);
    assert_eq!(mode(2, 3), FieldMode::Message(3));
    assert_eq!(mode(2, 57), FieldMode::Unsupported);
    assert_eq!(mode(3, 1), FieldMode::Bytes);
    assert_eq!(mode(3, 3), FieldMode::Unsupported);
    assert_eq!(
        plan[0]
            .fields
            .iter()
            .find(|field| field.number == 4)
            .unwrap()
            .name,
        "imageMessage"
    );
    assert!(
        plan[0]
            .fields
            .iter()
            .find(|field| field.number == 4)
            .unwrap()
            .oneof
            .is_some()
    );
    assert!(plan[3].fields.iter().all(|field| field.oneof.is_none()));
}
#[test]
fn new_descendant_field_automatically_requires_a_presence_guard() {
    let mut fds = descriptors();
    let image = owner_mut(&mut fds.file[0].message_type, &["Message", "ImageMessage"]);
    image.field.push(FieldDescriptorProto {
        name: Some("futureCaption".into()),
        number: Some(199),
        label: Some(Label::LABEL_OPTIONAL),
        r#type: Some(Type::TYPE_STRING),
        ..Default::default()
    });
    let plan = semantic_plan::plan(&fds);
    assert_eq!(plan[1].fields.last().unwrap().mode, FieldMode::Unsupported);
    assert_eq!(plan[1].fields.last().unwrap().name, "futureCaption");
}
#[test]
fn new_root_variant_is_unsupported_but_new_ordinary_field_is_outside_projection() {
    let mut fds = descriptors();
    let header = owner_mut(
        &mut fds.file[0].message_type,
        &["Message", "InteractiveMessage", "Header"],
    );
    let media = header
        .field
        .iter()
        .find(|field| field.number == Some(4))
        .unwrap()
        .oneof_index;
    for (number, oneof_index) in [(199, media), (200, None)] {
        header.field.push(FieldDescriptorProto {
            name: Some(format!("future{number}")),
            number: Some(number),
            oneof_index,
            label: Some(Label::LABEL_OPTIONAL),
            r#type: Some(Type::TYPE_BYTES),
            ..Default::default()
        });
    }
    let plan = semantic_plan::plan(&fds);
    assert_eq!(
        plan[0]
            .fields
            .iter()
            .find(|field| field.number == 199)
            .unwrap()
            .mode,
        FieldMode::Unsupported
    );
    assert_eq!(
        plan[0]
            .fields
            .iter()
            .find(|field| field.number == 200)
            .unwrap()
            .mode,
        FieldMode::Ignored
    );
}
#[test]
fn changed_kind_default_or_child_reference_disables_the_optimized_field() {
    for mutation in 0..4 {
        let mut fds = descriptors();
        let image = owner_mut(&mut fds.file[0].message_type, &["Message", "ImageMessage"]);
        let field = image
            .field
            .iter_mut()
            .find(|field| field.number == Some(17))
            .unwrap();
        match mutation {
            0 => field.r#type = Some(Type::TYPE_GROUP),
            1 => field.default_value = Some("future".into()),
            2 => field.type_name = Some(".whatsapp.Message".into()),
            _ => field.label = Some(Label::LABEL_REPEATED),
        }
        assert_eq!(
            semantic_plan::plan(&fds)[1]
                .fields
                .iter()
                .find(|field| field.number == 17)
                .unwrap()
                .mode,
            FieldMode::Unsupported
        );
    }
}

#[test]
fn implicit_proto3_presence_is_not_treated_as_proto2_optional_presence() {
    let mut fds = descriptors();
    for file in &mut fds.file {
        if file.package.as_deref() == Some("whatsapp") {
            file.syntax = Some("proto3".into());
        }
    }
    let plan = semantic_plan::plan(&fds);
    assert!(
        plan.iter()
            .flat_map(|owner| &owner.fields)
            .all(|field| matches!(field.mode, FieldMode::Ignored | FieldMode::Unsupported))
    );
}

#[test]
fn new_enum_and_other_oneof_group_are_guarded_at_the_root() {
    let mut fds = descriptors();
    let header = owner_mut(
        &mut fds.file[0].message_type,
        &["Message", "InteractiveMessage", "Header"],
    );
    header.oneof_decl.push(OneofDescriptorProto {
        name: Some("otherMedia".into()),
        ..Default::default()
    });
    header.field.push(FieldDescriptorProto {
        name: Some("newMode".into()),
        number: Some(199),
        label: Some(Label::LABEL_OPTIONAL),
        r#type: Some(Type::TYPE_ENUM),
        type_name: Some(".whatsapp.ContextInfo.StatusAttributionType".into()),
        ..Default::default()
    });
    header.field.push(FieldDescriptorProto {
        name: Some("newChoice".into()),
        number: Some(200),
        label: Some(Label::LABEL_OPTIONAL),
        r#type: Some(Type::TYPE_BYTES),
        oneof_index: Some(1),
        ..Default::default()
    });
    let plan = semantic_plan::plan(&fds);
    for number in [199, 200] {
        assert_eq!(
            plan[0]
                .fields
                .iter()
                .find(|field| field.number == number)
                .unwrap()
                .mode,
            FieldMode::Unsupported
        );
    }
}
#[test]
fn removing_the_media_association_disables_header_optimization() {
    let mut fds = descriptors();
    let header = owner_mut(
        &mut fds.file[0].message_type,
        &["Message", "InteractiveMessage", "Header"],
    );
    header
        .field
        .iter_mut()
        .find(|field| field.number == Some(4))
        .unwrap()
        .oneof_index = None;
    assert!(
        semantic_plan::plan(&fds)[0]
            .fields
            .iter()
            .all(|field| field.mode == FieldMode::Unsupported)
    );
}

#[test]
#[should_panic(expected = "descriptor matches generated member")]
fn a_descriptor_member_missing_from_the_generated_type_fails_closed() {
    let mut fds = descriptors();
    owner_mut(&mut fds.file[0].message_type, &["Message", "ImageMessage"])
        .field
        .push(FieldDescriptorProto {
            name: Some("futureCaption".into()),
            number: Some(199),
            label: Some(Label::LABEL_OPTIONAL),
            r#type: Some(Type::TYPE_STRING),
            ..Default::default()
        });
    let source = std::fs::read_to_string(concat!(env!("OUT_DIR"), "/whatsapp.rs")).unwrap();
    let syntax = syn::parse_file(&source).unwrap();
    semantic_plan::emit(&semantic_plan::plan(&fds), &syntax);
}

#[test]
fn a_candidate_moved_to_an_independent_oneof_is_not_merged_into_media() {
    let mut fds = descriptors();
    let header = owner_mut(
        &mut fds.file[0].message_type,
        &["Message", "InteractiveMessage", "Header"],
    );
    header.oneof_decl.push(OneofDescriptorProto {
        name: Some("otherMedia".into()),
        ..Default::default()
    });
    header
        .field
        .iter_mut()
        .find(|field| field.number == Some(6))
        .unwrap()
        .oneof_index = Some(1);
    let plan = semantic_plan::plan(&fds);
    assert_eq!(
        plan[0]
            .fields
            .iter()
            .find(|field| field.number == 6)
            .unwrap()
            .mode,
        FieldMode::Unsupported
    );
}

#![forbid(unsafe_code)]
#![allow(clippy::disallowed_methods)]

#[path = "../codegen/descriptor.rs"]
pub mod descriptor;
#[path = "../codegen/features.rs"]
pub mod features;
#[path = "../codegen/policy.rs"]
pub mod policy;
#[path = "../codegen/project.rs"]
pub mod project;
#[path = "support/project_reference.rs"]
pub mod reference;
#[path = "../codegen/schema.rs"]
pub mod schema;

use buffa::Message as _;
use buffa_descriptor::generated::descriptor::FileDescriptorSet as Reference;

fn submessage(number: u8, payload: &[u8]) -> Vec<u8> {
    let mut output = vec![(number << 3) | 2];
    let mut length = payload.len();
    while length >= 128 {
        output.push((length as u8 & 127) | 128);
        length >>= 7;
    }
    output.push(length as u8);
    output.extend_from_slice(payload);
    output
}

fn compare(input: &[u8]) {
    let expected = Reference::decode_from_slice(input).unwrap();
    let actual = descriptor::FileDescriptorSet::decode_from_slice(input).unwrap();
    let expected = reference::project(&expected).unwrap();
    let actual = project::project(&actual).unwrap();
    assert_eq!(format!("{actual:#?}"), format!("{expected:#?}"));
}

#[test]
fn all_committed_schema_projections_match_the_locked_descriptor_oracle() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap();
    compare(include_bytes!("../src/whatsapp.desc"));
    for path in [
        "wacore/src/voip/mlow/tables.desc",
        "storages/sqlite-storage/proto/wire.desc",
    ] {
        match std::fs::read(root.join(path)) {
            Ok(bytes) => compare(&bytes),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                eprintln!(
                    "workspace-only auxiliary descriptor oracle NOT_EXECUTED: schema source is outside the waproto package"
                );
                return;
            }
            Err(error) => panic!("read descriptor: {error}"),
        }
    }
}

#[test]
fn malformed_descriptor_boundaries_are_rejected_independently() {
    for input in [
        vec![0],
        vec![14],
        vec![10, 128],
        vec![10, 255, 255, 255, 255, 255, 255, 255, 255, 255, 2],
        vec![10, 2, 10, 255],
        vec![10, 3, 10, 1, 255],
        vec![27, 36],
        vec![12],
    ] {
        assert!(
            Reference::decode_from_slice(&input).is_err(),
            "reference accepted {input:02x?}"
        );
        assert!(
            descriptor::FileDescriptorSet::decode_from_slice(&input).is_err(),
            "candidate accepted {input:02x?}"
        );
    }
}

#[test]
fn descriptor_depth_boundary_matches_the_locked_reader() {
    for depth in [0, 1, 97, 98, 99, 100, 101] {
        let mut message = Vec::new();
        for _ in 0..depth {
            message = submessage(3, &message);
        }
        let input = submessage(1, &submessage(4, &message));
        let expected = Reference::decode_from_slice(&input).is_ok();
        assert_eq!(expected, depth <= 98);
        assert_eq!(
            descriptor::FileDescriptorSet::decode_from_slice(&input).is_ok(),
            expected,
            "depth {depth}"
        );
    }
}

#[test]
fn options_merge_and_unknown_fields_keep_their_descriptor_meaning() {
    // Two occurrences of FileOptions, each carrying a different FeatureSet
    // member. Singular submessages must merge across both nesting levels.
    let input = [
        10, 24, 98, 8, b'e', b'd', b'i', b't', b'i', b'o', b'n', b's', 66, 5, 146, 3, 2, 8, 2, 66,
        5, 146, 3, 2, 16, 2, 24, 1, 24, 2,
    ];
    let expected = Reference::decode_from_slice(&input).unwrap();
    let actual = descriptor::FileDescriptorSet::decode_from_slice(&input).unwrap();
    assert_eq!(
        actual.file[0].syntax.as_deref(),
        expected.file[0].syntax.as_deref()
    );
    let resolved = features::for_file(&actual.file[0]);
    let reference = buffa_descriptor::features::for_file(&expected.file[0]);
    assert_eq!(resolved.field_presence, features::FieldPresence::Implicit);
    assert_eq!(resolved.enum_type, features::EnumType::Closed);
    assert_eq!(
        reference.field_presence,
        buffa_descriptor::features::FieldPresence::Implicit
    );
    assert_eq!(
        reference.enum_type,
        buffa_descriptor::features::EnumType::Closed
    );
}

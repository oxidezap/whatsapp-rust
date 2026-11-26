#![cfg(feature = "protobuf-experiment")]
#![allow(clippy::disallowed_methods)]
#![forbid(unsafe_code)]

use waproto::protobuf as rt;

#[derive(Clone, Copy)]
enum Kind {
    Varint,
    Fixed32,
    Fixed64,
    String,
    Bytes,
    Message(&'static [u8]),
}

fn field(number: u32, kind: Kind, payload: &[u8]) -> Vec<u8> {
    let wire = match kind {
        Kind::Varint => 0,
        Kind::Fixed64 => 1,
        Kind::Fixed32 => 5,
        _ => 2,
    };
    let mut out = Vec::new();
    rt::encoding::encode_varint((u64::from(number) << 3) | wire, &mut out);
    if wire == 2 {
        rt::encoding::encode_varint(payload.len() as u64, &mut out);
    }
    out.extend_from_slice(payload);
    out
}

fn compare<C, R>(assert_fields: fn(&C, &R), assert_views: fn(&[u8]), fields: &[(u32, Kind)])
where
    C: rt::Message + rt::MessageName + std::fmt::Debug + serde::Serialize,
    R: buffa::Message + buffa::MessageName + std::fmt::Debug + serde::Serialize,
{
    assert_eq!(C::NAME, R::NAME);
    assert_eq!(C::PACKAGE, R::PACKAGE);
    assert_eq!(C::FULL_NAME, R::FULL_NAME);
    assert_eq!(C::TYPE_URL, R::TYPE_URL);
    let check = |input: &[u8]| {
        assert_views(input);
        let candidate = C::decode_from_slice(input);
        let reference = R::decode_from_slice(input);
        match (candidate, reference) {
            (Ok(candidate), Ok(reference)) => {
                assert_fields(&candidate, &reference);
                assert_eq!(
                    serde_json::to_value(&candidate).unwrap(),
                    serde_json::to_value(&reference).unwrap(),
                    "serde {} input={input:02x?}",
                    std::any::type_name::<C>()
                );
                assert_eq!(
                    candidate.encode_to_vec(),
                    reference.encode_to_vec(),
                    "canonical {} input={input:02x?}",
                    std::any::type_name::<C>()
                );
            }
            (Err(candidate), Err(reference)) => assert_eq!(
                format!("{candidate:?}"),
                format!("{reference:?}"),
                "rejection {} input={input:02x?}",
                std::any::type_name::<C>()
            ),
            (candidate, reference) => panic!(
                "decode mismatch {} input={input:02x?}\ncandidate={candidate:?}\nreference={reference:?}",
                std::any::type_name::<C>()
            ),
        }
    };
    let candidate = C::default();
    let reference = R::default();
    assert_fields(&candidate, &reference);
    assert_eq!(
        serde_json::to_value(&candidate).unwrap(),
        serde_json::to_value(&reference).unwrap()
    );
    assert_eq!(
        candidate.encode_to_vec(),
        reference.encode_to_vec(),
        "default {}",
        std::any::type_name::<C>()
    );
    check(&[]);
    let mut combined = Vec::new();
    for &(number, kind) in fields {
        let payloads: Vec<Vec<u8>> = match kind {
            Kind::Varint => [0, 1, 127, 128, 150, u32::MAX as u64, u64::MAX]
                .into_iter()
                .map(|value| {
                    let mut out = Vec::new();
                    rt::encoding::encode_varint(value, &mut out);
                    out
                })
                .collect(),
            Kind::Fixed32 => [0u32, 1, u32::MAX, 0x8000_0000, 0x7f80_0000, 0x7fc0_0042]
                .into_iter()
                .map(|value| value.to_le_bytes().to_vec())
                .collect(),
            Kind::Fixed64 => [
                0u64,
                1,
                u64::MAX,
                0x8000_0000_0000_0000,
                0x7ff0_0000_0000_0000,
                0x7ff8_0000_0000_0042,
            ]
            .into_iter()
            .map(|value| value.to_le_bytes().to_vec())
            .collect(),
            Kind::String => [
                b"".as_slice(),
                b"x",
                "olá".as_bytes(),
                b"12345@s.whatsapp.net",
            ]
            .into_iter()
            .map(<[u8]>::to_vec)
            .collect(),
            Kind::Bytes => vec![vec![], vec![0], vec![255], vec![1, 2, 3]],
            Kind::Message(seed) => vec![vec![], seed.to_vec()],
        };
        for payload in payloads {
            let bytes = field(number, kind, &payload);
            check(&bytes);
            for len in 1..bytes.len() {
                check(&bytes[..len]);
            }
            combined.extend_from_slice(&bytes);
        }
        if matches!(kind, Kind::Message(_)) {
            check(&field(number, kind, &[0]));
        }
        if matches!(kind, Kind::String) {
            check(&field(number, kind, &[255]));
        }
    }
    check(&combined);
    let mut unknown = combined.clone();
    unknown.extend_from_slice(&[0x80, 0xb5, 0x18, 0x81, 0]);
    check(&unknown);
    for invalid in [vec![0], vec![6], vec![7], vec![255; 10], vec![128; 11]] {
        check(&invalid);
    }
}

include!(concat!(env!("OUT_DIR"), "/owned_full_oracle.rs"));

#[test]
fn committed_codec_matches_the_independently_spliced_descriptor_path() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let source = std::fs::read_to_string(root.join("src/generated/whatsapp.rs")).unwrap();
    let source = source
        .strip_prefix("// @generated by protobuf_codegen. Do not edit.\n")
        .unwrap();
    assert_eq!(
        source,
        std::fs::read_to_string(std::path::Path::new(env!("OUT_DIR")).join("owned_full.rs"))
            .unwrap()
    );
    let old_tags =
        std::fs::read_to_string(std::path::Path::new(env!("OUT_DIR")).join("tags.rs")).unwrap();
    let body = old_tags
        .strip_prefix("// @generated from whatsapp.desc by waproto's build.rs. Do not edit.\n")
        .unwrap();
    let expected =
        format!("// @generated from whatsapp.desc by protobuf-codegen. Do not edit.\n{body}");
    assert_eq!(
        std::fs::read_to_string(root.join("src/generated/tags.rs")).unwrap(),
        expected
    );
}

#[test]
fn multiple_map_keys_preserve_semantics_and_duplicate_entries_replace_values() {
    use buffa::Message as _;
    use rt::{Message as _, MessageView as _};
    let input = [
        10, 6, 8, 1, 18, 2, 8, 2, 10, 6, 8, 2, 18, 2, 8, 3, 10, 6, 8, 1, 18, 2, 16, 4,
    ];
    let candidate = rt::generated::whatsapp::Config::decode_from_slice(&input).unwrap();
    let reference = waproto::reference::Config::decode_from_slice(&input).unwrap();
    let expected = serde_json::to_value(&reference).unwrap();
    assert_eq!(serde_json::to_value(&candidate).unwrap(), expected);
    assert_eq!(candidate.field.len(), 2);
    assert_eq!(candidate.field[&1].min_version, None);
    assert_eq!(candidate.field[&1].max_version, Some(4));
    assert_eq!(candidate.field[&2].min_version, Some(3));
    let view = rt::generated::whatsapp::ConfigView::decode_view(&input).unwrap();
    let old_view =
        <waproto::reference::ConfigView<'_> as buffa::MessageView>::decode_view(&input).unwrap();
    assert_eq!(view.field.len(), old_view.field.len());
    assert_eq!(view.field.len(), 3);
    assert_eq!(view.field.keys().copied().collect::<Vec<_>>(), [1, 2, 1]);
    assert_eq!(
        view.field
            .iter_unique()
            .map(|(key, _)| *key)
            .collect::<Vec<_>>(),
        [2, 1]
    );
    assert_eq!(view.field.len_unique(), 2);
    assert_eq!(
        serde_json::to_value(view.to_owned_message().unwrap()).unwrap(),
        expected
    );
    assert_eq!(
        rt::ViewEncode::encode_to_vec(&view),
        buffa::ViewEncode::encode_to_vec(&old_view)
    );
    let reference_from_candidate =
        waproto::reference::Config::decode_from_slice(&candidate.encode_to_vec()).unwrap();
    let candidate_from_reference =
        rt::generated::whatsapp::Config::decode_from_slice(&reference.encode_to_vec()).unwrap();
    assert_eq!(
        serde_json::to_value(reference_from_candidate).unwrap(),
        expected
    );
    assert_eq!(
        serde_json::to_value(candidate_from_reference).unwrap(),
        expected
    );
}

//! Enum serde representation: numeric repr (prost parity / JS bridge) under the
//! `serde-enum-repr` feature, variant name otherwise.

use waproto::whatsapp::ADVEncryptionType;

#[test]
fn enum_serde_representation() {
    let json = serde_json::to_value(ADVEncryptionType::HOSTED).unwrap();

    #[cfg(feature = "serde-enum-repr")]
    assert_eq!(
        json,
        serde_json::json!(1),
        "enum should serialize as its numeric repr under serde-enum-repr, got {json}"
    );

    #[cfg(not(feature = "serde-enum-repr"))]
    assert_eq!(
        json,
        serde_json::json!("HOSTED"),
        "enum should serialize as its variant name by default, got {json}"
    );
}

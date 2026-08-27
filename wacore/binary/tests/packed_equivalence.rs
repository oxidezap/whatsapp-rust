//! The packed-value tables must decode exactly what the scalar nibble/hex
//! walk did, including where an invalid nibble reports the failure.
use wacore_binary::builder::NodeBuilder;
use wacore_binary::marshal::{marshal, unmarshal_packed_ref, unmarshal_ref};

fn roundtrip_attr(value: &str) -> String {
    let node = NodeBuilder::new("t").attr("v", value).build();
    let bytes = marshal(&node).unwrap();
    let decoded = unmarshal_packed_ref(&bytes).unwrap();
    match decoded.get_attr("v").unwrap() {
        wacore_binary::node::ValueRef::String(s) => s.to_string(),
        other => panic!("unexpected value {other:?}"),
    }
}

#[test]
fn packed_values_round_trip_across_the_simd_boundary() {
    // Hex and nibble, at lengths below, at, and above the 16-byte chunk the
    // SIMD path handles, plus odd lengths that exercise the half-byte trim.
    for len in [1, 2, 3, 15, 16, 17, 31, 32, 33, 63, 100, 127] {
        let hex: String = (0..len)
            .map(|i| b"0123456789ABCDEF"[i % 16] as char)
            .collect();
        assert_eq!(roundtrip_attr(&hex), hex, "hex len {len}");

        let nibble: String = (0..len).map(|i| b"0123456789-."[i % 12] as char).collect();
        assert_eq!(roundtrip_attr(&nibble), nibble, "nibble len {len}");
    }
}

/// Nibble 12, 13 and 14 encode nothing. Frames are built by hand, since the
/// encoder would never emit one. Both lengths matter: one byte stays in the
/// scalar remainder, while 20 bytes puts the bad nibble inside the first
/// 16-byte chunk, which is where the SIMD build takes its validation
/// fallback.
#[test]
fn an_invalid_nibble_is_still_rejected() {
    for bad in [0x0Cu8, 0x0D, 0x0E] {
        // NIBBLE_8, length 1, one byte whose low half is invalid.
        let short = [248u8, 2, 1, 255, 1, 0xF0 | bad];
        let err = unmarshal_ref(&short).unwrap_err();
        assert!(
            format!("{err}").contains(&bad.to_string()),
            "short frame must name the bad nibble {bad}: {err}"
        );

        // NIBBLE_8, length 20, with the bad nibble at byte 5 so it lands in
        // the chunk rather than the remainder.
        let mut long = vec![248u8, 2, 1, 255, 20];
        for i in 0..20u8 {
            long.push(if i == 5 { 0xF0 | bad } else { 0x12 });
        }
        let err = unmarshal_ref(&long).unwrap_err();
        assert!(
            format!("{err}").contains(&bad.to_string()),
            "chunked frame must name the bad nibble {bad}: {err}"
        );
    }
}

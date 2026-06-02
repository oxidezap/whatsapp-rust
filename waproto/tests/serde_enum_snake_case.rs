//! Under `serde-snake-case`, enums deserialize from the lowercased proto name
//! (CHROME -> "chrome", ANDROID_TABLET -> "android_tablet"). buffa emits
//! SCREAMING_SNAKE variants, so the rename rule must be `lowercase`, not
//! `snake_case` (which would expect "c_h_r_o_m_e" / "a_n_d_r_o_i_d__t_a_b_l_e_t").

#![cfg(all(feature = "serde-snake-case", not(feature = "serde-enum-repr")))]

use waproto::whatsapp::device_props::PlatformType;

#[test]
fn enum_deserializes_from_lowercased_proto_name() {
    let chrome: PlatformType = serde_json::from_value(serde_json::json!("chrome")).unwrap();
    assert_eq!(chrome, PlatformType::CHROME);

    // Multi-word: only `lowercase` yields the single-underscore form.
    let tablet: PlatformType = serde_json::from_value(serde_json::json!("android_tablet")).unwrap();
    assert_eq!(tablet, PlatformType::ANDROID_TABLET);
}

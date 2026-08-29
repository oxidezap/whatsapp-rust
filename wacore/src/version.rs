mod generated;

pub use generated::{WA_WEB_VERSION, WA_WEB_VERSION_STR};

const REVISION_KEY: &str = "client_revision";
const ASSETS_KEY: &str = "assets-manifest-";

/// The object that carries the build revision in the Facebook JS SDK bundle.
/// `revision` alone is far too common a word to look for in a JS blob, so the
/// search starts at the object that owns the field we mean.
const SDK_CONFIG_ANCHOR: &str = "JSSDKRuntimeConfig";
const SDK_REVISION_KEY: &str = "\"revision\"";

/// Reads the integer that `key` names, tolerating the JSON punctuation that may
/// sit between a key and its value in a bundle (`:`, quotes, and the
/// backslashes of a string-embedded JSON blob). Anything else between the two
/// means the value is not a number, and that is a miss rather than a guess.
fn revision_after(s: &str, key: &str) -> Option<u32> {
    let after = &s[s.find(key)? + key.len()..];
    let value =
        after.trim_start_matches(|c: char| matches!(c, ':' | '"' | '\\') || c.is_whitespace());
    let end = value
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(value.len());
    value[..end].parse().ok()
}

/// Parses the Meta build revision from a Facebook JS SDK bundle.
/// Returns the same `(2, 3000, revision)` shape as [`parse_sw_js`]: the two
/// sources carry the same number, since it is the revision of Meta's shared
/// `www` build rather than anything specific to one property.
pub fn parse_meta_sdk_js(s: &str) -> Option<(u32, u32, u32)> {
    let anchored = &s[s.find(SDK_CONFIG_ANCHOR)?..];
    Some((2, 3000, revision_after(anchored, SDK_REVISION_KEY)?))
}

/// Parses the WhatsApp Web version from sw.js content.
/// Returns `(2, 3000, revision)` tuple.
pub fn parse_sw_js(s: &str) -> Option<(u32, u32, u32)> {
    if let Some(start_index) = s.find(REVISION_KEY) {
        let suffix = &s[start_index + REVISION_KEY.len()..];

        if let Some(first_digit_index) = suffix.find(|c: char| c.is_ascii_digit()) {
            let number_slice = &suffix[first_digit_index..];

            let end_of_number_index = number_slice
                .find(|c: char| !c.is_ascii_digit())
                .unwrap_or(number_slice.len());

            let version_str = &number_slice[..end_of_number_index];

            if let Ok(revision) = version_str.parse::<u32>() {
                return Some((2, 3000, revision));
            }
        }
    }

    if let Some(start_index) = s.find(ASSETS_KEY) {
        let suffix = &s[start_index + ASSETS_KEY.len()..];
        if let Some(end_index) = suffix.find(|c: char| !c.is_ascii_digit()) {
            let version_str = &suffix[..end_index];
            if !s.contains(&format!("wa{}.canary", version_str)) {
                return Some((2, 3000, 0));
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sw_js_client_revision_quoted() {
        let s = r#"var x = {"client_revision": "123456"};"#;
        assert_eq!(parse_sw_js(s), Some((2, 3000, 123456)));
    }

    #[test]
    fn test_parse_sw_js_client_revision_unquoted() {
        let s = r#"client_revision:12345;"#;
        assert_eq!(parse_sw_js(s), Some((2, 3000, 12345)));
    }

    #[test]
    fn test_parse_sw_js_assets_fallback() {
        let s = "... assets-manifest-98765 ...";
        assert_eq!(parse_sw_js(s), Some((2, 3000, 0)));
    }

    #[test]
    fn test_parse_sw_js_realistic_sw_js() {
        let s = r#"{"client_revision":1026131876}"#;
        assert_eq!(parse_sw_js(s), Some((2, 3000, 1026131876)));
    }

    #[test]
    fn test_parse_meta_sdk_js_happy() {
        let s =
            r#"a={"JSSDKRuntimeConfig":{"locale":"en_US","revision":"1046341789","rtl":false}};"#;
        assert_eq!(parse_meta_sdk_js(s), Some((2, 3000, 1046341789)));
    }

    #[test]
    fn test_parse_meta_sdk_js_missing_field() {
        let s = r#"a={"JSSDKRuntimeConfig":{"locale":"en_US","rtl":false}};"#;
        assert_eq!(parse_meta_sdk_js(s), None);
    }

    #[test]
    fn test_parse_meta_sdk_js_missing_anchor() {
        let s = r#"a={"revision":"1046341789"};"#;
        assert_eq!(parse_meta_sdk_js(s), None);
    }

    #[test]
    fn test_parse_meta_sdk_js_non_numeric_value() {
        let s = r#"a={"JSSDKRuntimeConfig":{"revision":null,"rtl":false}};"#;
        assert_eq!(parse_meta_sdk_js(s), None);
    }

    /// The anchor exists because the bare word appears elsewhere in the bundle:
    /// the first occurrence is not the one we mean.
    #[test]
    fn test_parse_meta_sdk_js_ignores_earlier_unrelated_revisions() {
        let s = r#"var a={"revision":"1"};var b={"x":{"revision":"2"}};
                   c={"JSSDKRuntimeConfig":{"locale":"en_US","revision":"1046341789"}};
                   d={"revision":"3"};"#;
        assert_eq!(parse_meta_sdk_js(s), Some((2, 3000, 1046341789)));
    }

    #[test]
    fn test_parse_sw_js_not_found() {
        let s = "no version info here";
        assert_eq!(parse_sw_js(s), None);
    }
}

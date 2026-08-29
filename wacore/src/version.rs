mod generated;

pub use generated::{WA_WEB_VERSION, WA_WEB_VERSION_STR};

const REVISION_KEY: &str = "client_revision";
const ASSETS_KEY: &str = "assets-manifest-";

/// The object that carries the build revision in the Facebook JS SDK bundle.
/// `revision` alone is far too common a word to look for in a JS blob, so the
/// search starts at the object that owns the field we mean. The key is quoted
/// because the bundle also names the object from minified code, where it is not.
const SDK_CONFIG_ANCHOR: &str = "\"JSSDKRuntimeConfig\"";
const SDK_REVISION_KEY: &str = "revision";

/// The text following `key` where it appears as a direct member of `object`,
/// skipping any nested object or array. A namesake nested inside the config is
/// not the config's own field, and taking the first one found would let it
/// stand in for the value we mean.
fn direct_member_value<'a>(object: &'a str, key: &str) -> Option<&'a str> {
    let bytes = object.as_bytes();
    let mut depth = 0usize;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'{' | b'[' => {
                depth += 1;
                i += 1;
            }
            b'}' | b']' => {
                depth = depth.saturating_sub(1);
                i += 1;
            }
            b'"' => {
                let start = i + 1;
                let mut end = start;
                while end < bytes.len() && bytes[end] != b'"' {
                    end += if bytes[end] == b'\\' { 2 } else { 1 };
                }
                // A member separator has to follow, or this is a value that
                // merely reads like the key and the real member is still ahead.
                if depth == 0
                    && object.get(start..end) == Some(key)
                    && let Some(after) = object.get(end + 1..)
                    && after.trim_start().starts_with(':')
                {
                    return Some(after);
                }
                i = end + 1;
            }
            _ => i += 1,
        }
    }
    None
}

/// Reads the integer a member's value holds, tolerating the JSON punctuation
/// that may sit between a key and its value in a bundle (`:`, quotes, and the
/// backslashes of a string-embedded JSON blob). Anything else between the two
/// means the value is not a number, and that is a miss rather than a guess.
fn revision_value(after_key: &str) -> Option<u32> {
    let value =
        after_key.trim_start_matches(|c: char| matches!(c, ':' | '"' | '\\') || c.is_whitespace());
    let end = value
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(value.len());
    value[..end].parse().ok()
}

/// The body of the JSON object `anchor` names. Bounding the lookup this way is
/// the point of the anchor: were the field renamed inside the object, an
/// unbounded search would run on and adopt an unrelated `revision` from further
/// down the bundle instead of reporting that the source changed shape. Anything
/// but a balanced object yields nothing, which fails the same way.
///
/// Braces are counted only outside string values, because a brace inside one
/// would otherwise move the boundary: a `{` extends the window past the object
/// and readmits the unrelated field this exists to exclude.
fn object_after<'a>(s: &'a str, anchor: &str) -> Option<&'a str> {
    let after = &s[s.find(anchor)? + anchor.len()..];
    let body = after
        .trim_start_matches(|c: char| c == ':' || c.is_whitespace())
        .strip_prefix('{')?;
    let mut depth = 1usize;
    let mut in_string = false;
    let mut escaped = false;
    for (i, c) in body.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match c {
            '\\' => escaped = true,
            '"' => in_string = !in_string,
            '{' if !in_string => depth += 1,
            '}' if !in_string => {
                depth -= 1;
                if depth == 0 {
                    return Some(&body[..i]);
                }
            }
            _ => {}
        }
    }
    None
}

/// Parses the Meta build revision from a Facebook JS SDK bundle.
/// Returns the same `(2, 3000, revision)` shape as [`parse_sw_js`]: the two
/// sources carry the same number, since it is the revision of Meta's shared
/// `www` build rather than anything specific to one property.
pub fn parse_meta_sdk_js(s: &str) -> Option<(u32, u32, u32)> {
    // Every match, not just the first: the anchor also appears in minified code
    // that names the object, and could appear in a string the bundle carries.
    // Telling those from the real config would take a JS lexer, which is far
    // more than a heuristic over a minified bundle is worth, so candidates that
    // disagree are treated as not knowing rather than as a winner to pick. That
    // turns the worst outcome, a confidently wrong version, into a reported
    // fallback.
    let mut found: Option<u32> = None;
    for (index, _) in s.match_indices(SDK_CONFIG_ANCHOR) {
        if let Some(config) = object_after(&s[index..], SDK_CONFIG_ANCHOR)
            && let Some(after_key) = direct_member_value(config, SDK_REVISION_KEY)
            && let Some(revision) = revision_value(after_key)
        {
            match found {
                Some(seen) if seen != revision => return None,
                _ => found = Some(revision),
            }
        }
    }
    found.map(|revision| (2, 3000, revision))
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

    /// The live bundle names this object twice: once from minified code, and
    /// once as the JSON literal that actually carries the revision. Stopping at
    /// the first match finds an object that has no revision in it.
    #[test]
    fn test_parse_meta_sdk_js_skips_a_config_object_without_the_field() {
        let s = r#"catch(e){var C=a.JSSDKRuntimeConfig,b=C.revision;L({error:"LOAD",extra:{revision:b}})}
                   x={"JSSDKRuntimeConfig":{"locale":"en_US","revision":"1046341789","rtl":false}};"#;
        assert_eq!(parse_meta_sdk_js(s), Some((2, 3000, 1046341789)));
    }

    #[test]
    fn test_parse_meta_sdk_js_nested_object_before_the_field() {
        let s = r#"a={"JSSDKRuntimeConfig":{"sdkab":{"x":1},"revision":"1046341789"}};"#;
        assert_eq!(parse_meta_sdk_js(s), Some((2, 3000, 1046341789)));
    }

    /// A nested namesake is not the config's own revision, and it appears
    /// first, so taking the first match would return the wrong build.
    #[test]
    fn test_parse_meta_sdk_js_prefers_the_direct_member_over_a_nested_namesake() {
        let s = r#"a={"JSSDKRuntimeConfig":{"nested":{"revision":"7"},"revision":"1046341789"}};"#;
        assert_eq!(parse_meta_sdk_js(s), Some((2, 3000, 1046341789)));
    }

    /// A value that reads like the key is not the key, and the real member is
    /// still ahead of it.
    #[test]
    fn test_parse_meta_sdk_js_skips_a_value_that_looks_like_the_key() {
        let s = r#"a={"JSSDKRuntimeConfig":{"label":"revision","revision":"1046341789"}};"#;
        assert_eq!(parse_meta_sdk_js(s), Some((2, 3000, 1046341789)));
    }

    /// Two anchors that disagree mean the bundle no longer says one thing, and
    /// guessing between them would be a confidently wrong version.
    #[test]
    fn test_parse_meta_sdk_js_disagreeing_candidates_are_a_miss() {
        let s = r#"m='"JSSDKRuntimeConfig":{"revision":"7"}';
                   x={"JSSDKRuntimeConfig":{"revision":"1046341789"}};"#;
        assert_eq!(parse_meta_sdk_js(s), None);
    }

    /// Repeating the same answer is not a disagreement.
    #[test]
    fn test_parse_meta_sdk_js_agreeing_candidates_resolve() {
        let s = r#"m={"JSSDKRuntimeConfig":{"revision":"1046341789"}};
                   x={"JSSDKRuntimeConfig":{"revision":"1046341789"}};"#;
        assert_eq!(parse_meta_sdk_js(s), Some((2, 3000, 1046341789)));
    }

    /// Only nested namesakes means the field we mean is gone, which is a miss.
    #[test]
    fn test_parse_meta_sdk_js_ignores_a_nested_only_namesake() {
        let s = r#"a={"JSSDKRuntimeConfig":{"nested":{"revision":"7"},"rev":"8"}};"#;
        assert_eq!(parse_meta_sdk_js(s), None);
    }

    /// Same rule for an array element.
    #[test]
    fn test_parse_meta_sdk_js_ignores_a_namesake_inside_an_array() {
        let s = r#"a={"JSSDKRuntimeConfig":{"xs":[{"revision":"7"}],"revision":"1046341789"}};"#;
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

    /// A rename inside the config object has to read as "source changed shape",
    /// not as a licence to adopt whatever number appears next in the bundle.
    #[test]
    fn test_parse_meta_sdk_js_does_not_escape_the_config_object() {
        let s = r#"a={"JSSDKRuntimeConfig":{"locale":"en_US","rev":"1046341789"}};
                   b={"revision":"7"};"#;
        assert_eq!(parse_meta_sdk_js(s), None);
    }

    /// A brace inside a string value is text, not structure. An opening one is
    /// the dangerous case: counted as structure it holds the object open and
    /// lets the lookup reach a later, unrelated field.
    #[test]
    fn test_parse_meta_sdk_js_ignores_braces_inside_string_values() {
        let s = r#"a={"JSSDKRuntimeConfig":{"sdkns":"a{b","revision":"1046341789"}};"#;
        assert_eq!(parse_meta_sdk_js(s), Some((2, 3000, 1046341789)));

        let closing = r#"a={"JSSDKRuntimeConfig":{"sdkns":"a}b","revision":"1046341789"}};"#;
        assert_eq!(parse_meta_sdk_js(closing), Some((2, 3000, 1046341789)));
    }

    /// The open brace must not hold past the object when the field is gone.
    #[test]
    fn test_parse_meta_sdk_js_string_brace_does_not_extend_the_window() {
        let s = r#"a={"JSSDKRuntimeConfig":{"sdkns":"a{b","rev":"1"}};
                   b={"revision":"7"};"#;
        assert_eq!(parse_meta_sdk_js(s), None);
    }

    #[test]
    fn test_parse_meta_sdk_js_escaped_quote_in_a_string_value() {
        let s = r#"a={"JSSDKRuntimeConfig":{"sdkns":"a\"}b","revision":"1046341789"}};"#;
        assert_eq!(parse_meta_sdk_js(s), Some((2, 3000, 1046341789)));
    }

    /// A non-object value is not an object to search.
    #[test]
    fn test_parse_meta_sdk_js_non_object_value() {
        let s = r#"a={"JSSDKRuntimeConfig":null};b={"revision":"7"};"#;
        assert_eq!(parse_meta_sdk_js(s), None);
    }

    #[test]
    fn test_parse_meta_sdk_js_unterminated_config_object() {
        let s = r#"a={"JSSDKRuntimeConfig":{"locale":"en_US","#;
        assert_eq!(parse_meta_sdk_js(s), None);
    }

    /// The anchor exists because the bare word appears elsewhere in the bundle:
    /// neither the occurrences before it nor the ones after it are the one we mean.
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

//! Identifier casing and keyword escaping for the emitted Rust.
//!
//! These rules decide the public names of the generated registries, so they are
//! reproduced exactly as whatspec's reference emitter defines them: a bundle key
//! must keep mapping to the same `pub const` / `pub mod` across regenerations,
//! or a refresh silently renames API.

use std::collections::HashSet;

const RUST_KEYWORDS: &[&str] = &[
    "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn", "for",
    "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return",
    "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use", "where",
    "while", "async", "await", "dyn", "gen", "abstract", "become", "box", "do", "final", "macro",
    "override", "priv", "try", "typeof", "unsized", "virtual", "yield",
];

/// Keywords a raw identifier cannot spell (`r#self` is a compile error), so they
/// are disambiguated with a suffix instead.
const RAW_INELIGIBLE: &[&str] = &["self", "Self", "crate", "super"];

/// Coerce an arbitrary string into a usable Rust identifier: non-empty, not
/// digit-led, keyword-safe. A no-op on the ordinary alphanumeric bundle names.
pub fn ensure_ident(s: &str) -> String {
    let base = if s.is_empty() {
        "_".to_string()
    } else if s.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{s}")
    } else {
        s.to_string()
    };
    if RAW_INELIGIBLE.contains(&base.as_str()) {
        format!("{base}_")
    } else if RUST_KEYWORDS.contains(&base.as_str()) {
        format!("r#{base}")
    } else {
        base
    }
}

/// Separate `fooBar` into `foo<sep>Bar`.
fn split_lower_upper(s: &str, sep: char) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len() + 8);
    for (i, &c) in chars.iter().enumerate() {
        if i > 0 && c.is_ascii_uppercase() {
            let prev = chars[i - 1];
            if prev.is_ascii_lowercase() || prev.is_ascii_digit() {
                out.push(sep);
            }
        }
        out.push(c);
    }
    out
}

/// Separate a run of capitals from the word that follows it: `HTTPServer` into
/// `HTTP<sep>Server`. The break goes before the last capital of the run, which
/// is the one that starts the next word.
fn split_acronym(s: &str, sep: char) -> String {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len() + 8);
    for (i, &c) in chars.iter().enumerate() {
        if i > 0
            && i + 1 < chars.len()
            && c.is_ascii_uppercase()
            && chars[i - 1].is_ascii_uppercase()
            && chars[i + 1].is_ascii_lowercase()
        {
            out.push(sep);
        }
        out.push(c);
    }
    out
}

fn non_alnum_to(s: &str, sep: char) -> String {
    s.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { sep })
        .collect()
}

/// `FooBar` / `foo-bar` into `foo_bar`.
pub fn snake_case(s: &str) -> String {
    let s = split_lower_upper(s, '_');
    let s = split_acronym(&s, '_');
    let s = non_alnum_to(&s, '_');
    let mut collapsed = String::with_capacity(s.len());
    let mut last_underscore = false;
    for c in s.chars() {
        if c == '_' {
            if !last_underscore {
                collapsed.push(c);
            }
            last_underscore = true;
        } else {
            collapsed.push(c);
            last_underscore = false;
        }
    }
    collapsed.trim_matches('_').to_lowercase()
}

/// `foo_bar` / `fooBar` into `FooBar`, coerced into a valid Rust type or
/// variant identifier.
pub fn pascal_case(s: &str) -> String {
    let s = split_lower_upper(s, ' ');
    let s = split_acronym(&s, ' ');
    let s = non_alnum_to(&s, ' ');
    let pascal: String = s
        .split_whitespace()
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                Some(first) => first.to_uppercase().collect::<String>() + c.as_str(),
                None => String::new(),
            }
        })
        .collect();
    ensure_ident(&pascal)
}

/// `snake_case` coerced into a valid, keyword-safe Rust identifier.
pub fn rust_ident(s: &str) -> String {
    ensure_ident(&snake_case(s))
}

/// Make `base` a unique, ident-safe name within `used`, falling back to
/// `fallback_prefix` when the bundle name is empty and appending a numeric
/// suffix on collision.
pub fn unique_ident(base: &str, used: &mut HashSet<String>, fallback_prefix: &str) -> String {
    let mut name = if base.is_empty() {
        ensure_ident(fallback_prefix)
    } else {
        ensure_ident(base)
    };
    if used.contains(&name) {
        let mut n = 2;
        while used.contains(&format!("{name}_{n}")) {
            n += 1;
        }
        name = format!("{name}_{n}");
    }
    used.insert(name.clone());
    name
}

/// [`unique_ident`] for a name that has to stay UpperCamelCase.
///
/// The `_2` suffix would make an enum variant trip `non_camel_case_types`, and
/// the generated file is compiled with warnings denied.
pub fn unique_type_ident(base: &str, used: &mut HashSet<String>, fallback_prefix: &str) -> String {
    let mut name = ensure_ident(if base.is_empty() {
        fallback_prefix
    } else {
        base
    });
    if used.contains(&name) {
        let mut n = 2;
        while used.contains(&format!("{name}{n}")) {
            n += 1;
        }
        name = format!("{name}{n}");
    }
    used.insert(name.clone());
    name
}

/// A fully escaped Rust string literal. Bundle strings are ordinarily plain, but
/// this stays correct for one carrying a quote, a backslash or a control byte.
pub fn rust_lit(s: &str) -> String {
    format!("{s:?}")
}

/// A valid Rust `f64` literal. `{:?}` renders finite values as literals but
/// spells the others `NaN`/`inf`, which are not.
pub fn float_lit(f: f64) -> String {
    if f.is_nan() {
        "f64::NAN".to_string()
    } else if f.is_infinite() {
        if f > 0.0 {
            "f64::INFINITY".to_string()
        } else {
            "f64::NEG_INFINITY".to_string()
        }
    } else {
        format!("{f:?}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn casing_matches_the_reference_rules() {
        assert_eq!(snake_case("FooBarBaz"), "foo_bar_baz");
        assert_eq!(snake_case("HTTPServer"), "http_server");
        assert_eq!(snake_case("w:g2"), "w_g2");
        assert_eq!(
            snake_case("ACSServerProviderConfig"),
            "acs_server_provider_config"
        );
        assert_eq!(
            snake_case("__leading__and__trailing__"),
            "leading_and_trailing"
        );
        assert_eq!(pascal_case("foo_bar"), "FooBar");
        assert_eq!(pascal_case("getGroupInfo"), "GetGroupInfo");
        assert_eq!(
            pascal_case("xwa2_newsletter_admin_invite_accept"),
            "Xwa2NewsletterAdminInviteAccept"
        );
        // A digit run does not start a new word, so the capital after it does.
        assert_eq!(snake_case("mlow2Codec"), "mlow2_codec");
    }

    #[test]
    fn keyword_and_shape_escaping() {
        assert_eq!(rust_ident("type"), "r#type");
        assert_eq!(rust_ident("name"), "name");
        // self/Self/crate/super cannot be raw identifiers.
        assert_eq!(rust_ident("self"), "self_");
        assert_eq!(pascal_case("self"), "Self_");
        // `gen` became a keyword in edition 2024; a bundle field named `gen`
        // would otherwise emit a struct that does not parse.
        assert_eq!(rust_ident("gen"), "r#gen");
        assert_eq!(rust_ident("2fa"), "_2fa");
        assert_eq!(pascal_case("123"), "_123");
        assert_eq!(ensure_ident(""), "_");
    }

    #[test]
    fn unique_ident_sanitizes_and_disambiguates() {
        let mut used = HashSet::new();
        assert_eq!(unique_ident("match", &mut used, "m"), "r#match");
        assert_eq!(unique_ident("self", &mut used, "m"), "self_");
        assert_eq!(unique_ident("foo", &mut used, "m"), "foo");
        assert_eq!(unique_ident("foo", &mut used, "m"), "foo_2");
        assert_eq!(unique_ident("foo", &mut used, "m"), "foo_3");
        assert_eq!(unique_ident("", &mut used, "fallback"), "fallback");
    }

    #[test]
    fn literals_stay_valid_for_awkward_bundle_strings() {
        assert_eq!(rust_lit("plain"), "\"plain\"");
        assert_eq!(rust_lit("a\"b\\c"), "\"a\\\"b\\\\c\"");
        assert_eq!(float_lit(0.0), "0.0");
        assert_eq!(float_lit(f64::NAN), "f64::NAN");
        assert_eq!(float_lit(f64::INFINITY), "f64::INFINITY");
        assert_eq!(float_lit(f64::NEG_INFINITY), "f64::NEG_INFINITY");
    }
}

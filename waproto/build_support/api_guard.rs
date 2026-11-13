use std::collections::BTreeSet;
use std::io;
fn serialization_attribute(text: &str) -> bool {
    fn has_serde(meta: &syn::Meta) -> bool {
        if meta.path().is_ident("serde") {
            return true;
        }
        if let syn::Meta::List(list) = meta
            && list.path.is_ident("cfg_attr")
            && let Ok(nested) = list.parse_args_with(
                syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated,
            )
        {
            return nested.iter().any(has_serde);
        }
        false
    }
    syn::parse::Parser::parse_str(syn::Attribute::parse_outer, text)
        .is_ok_and(|attrs| attrs.iter().any(|attr| has_serde(&attr.meta)))
}

pub fn check_api(expected: &str, actual: &BTreeSet<String>) -> io::Result<()> {
    let missing: Vec<_> = expected
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !actual.contains(*l))
        .collect();
    // Attribute entries are separate so existing snapshots stay readable.
    // A new attribute on a retained field can change serialization, even
    // though adding an entirely new field and its attributes is compatible.
    let retained_fields: BTreeSet<_> = expected
        .lines()
        .filter_map(|line| {
            line.strip_prefix("field ")?
                .split_once(' ')
                .map(|(owner, _)| owner)
        })
        .collect();
    let retained_types: BTreeSet<_> = expected
        .lines()
        .filter_map(|line| {
            line.strip_prefix("struct ")
                .or_else(|| line.strip_prefix("enum "))?
                .split_once(' ')
                .map(|(owner, _)| owner)
        })
        .collect();
    let expected_entries: BTreeSet<_> = expected.lines().collect();
    let added_attributes: Vec<_> = actual
        .iter()
        .filter(|line| {
            line.strip_prefix("attribute ")
                .and_then(|entry| entry.split_once(' '))
                .is_some_and(|(owner, attribute)| {
                    retained_fields.contains(owner)
                        || (retained_types.contains(owner) && serialization_attribute(attribute))
                })
                && !expected_entries.contains(line.as_str())
        })
        .collect();
    if !added_attributes.is_empty() {
        return Err(io::Error::other(format!(
            "generated protobuf API changed: contract attributes added to retained declarations:\n{}",
            added_attributes
                .into_iter()
                .take(20)
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join("\n")
        )));
    }
    if missing.is_empty() {
        return Ok(());
    }
    Err(io::Error::other(format!(
        "generated protobuf API changed: {} frozen entries missing or changed. Preserve names in build_support/names.rs and retained declarations in the proto emitter; a generator upgrade is a public API change. First entries:\n{}",
        missing.len(),
        missing.into_iter().take(20).collect::<Vec<_>>().join("\n")
    )))
}

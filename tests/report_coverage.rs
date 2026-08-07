//! `Client`'s growable state must stay reachable from `memory_report()`.
//!
//! The gap this guards is the one that made per-session RAM unattributable: a
//! cache lands on `Client`, nobody adds it to the report, and the only symptom
//! is an RSS number no report explains. Scalars, locks and notifiers are left
//! alone, since they cost `size_of` and cannot drift. Collections can, so every
//! field whose type names one is either walked by the report or listed below
//! with the reason it is not.

use std::path::{Path, PathBuf};

/// Type constructors that can retain an unbounded number of entries. A field
/// naming one of these anywhere in its type is a candidate for the report.
const GROWABLE: &[&str] = &[
    "BTreeMap",
    "Cache",
    "HashMap",
    "HashSet",
    "TypedCache",
    "Vec",
    "VecDeque",
];

/// Growable fields the report deliberately does not walk. Each entry states why
/// it is not a per-session memory question; a new cache does NOT belong here.
const EXEMPT: &[(&str, &str)] = &[
    (
        "offline_receipt_buffer",
        "drained at the end of every offline batch; bounded by one batch, and the \
         MessageInfo values are already counted where the batch owns them",
    ),
    (
        "message_retry_counts",
        "counter-only map bounded by the retry ceiling; entries are two integers",
    ),
];

fn manifest_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

fn read(relative: &str) -> String {
    let path = manifest_path(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"))
}

/// Every ident appearing anywhere in a type expression, including inside
/// generic arguments (`Arc<Mutex<HashMap<..>>>` yields all four).
fn type_idents(ty: &syn::Type, out: &mut Vec<String>) {
    match ty {
        syn::Type::Path(path) => {
            for segment in &path.path.segments {
                out.push(segment.ident.to_string());
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    for arg in &args.args {
                        if let syn::GenericArgument::Type(inner) = arg {
                            type_idents(inner, out);
                        }
                    }
                }
            }
        }
        syn::Type::Reference(r) => type_idents(&r.elem, out),
        syn::Type::Paren(p) => type_idents(&p.elem, out),
        syn::Type::Group(g) => type_idents(&g.elem, out),
        syn::Type::Tuple(t) => t.elems.iter().for_each(|e| type_idents(e, out)),
        _ => {}
    }
}

fn client_struct(text: &str) -> syn::ItemStruct {
    let file = syn::parse_file(text).expect("parse src/client.rs");
    file.items
        .into_iter()
        .find_map(|item| match item {
            syn::Item::Struct(item) if item.ident == "Client" => Some(item),
            _ => None,
        })
        .expect("`struct Client` in src/client.rs")
}

#[test]
fn every_growable_client_field_reaches_the_memory_report() {
    let client = client_struct(&read("src/client.rs"));
    // The report walks the fields here; a name mentioned anywhere in the file is
    // reached by it, which is the property under test, not how it is rendered.
    let accessors = read("src/client/accessors.rs");

    let mut missing = Vec::new();
    for field in &client.fields {
        let Some(name) = field.ident.as_ref().map(ToString::to_string) else {
            continue;
        };
        let mut idents = Vec::new();
        type_idents(&field.ty, &mut idents);
        if !idents.iter().any(|i| GROWABLE.contains(&i.as_str())) {
            continue;
        }
        if EXEMPT.iter().any(|(exempt, _)| *exempt == name) {
            continue;
        }
        if !accessors.contains(&name) {
            missing.push(format!("  {name}: {}", idents.join("<")));
        }
    }

    assert!(
        missing.is_empty(),
        "these `Client` fields can grow but never reach `memory_report()`:\n{}\n\n\
         Add them to the report (see agent_docs/observability.md), or to EXEMPT in \
         this file with the reason they are not a per-session memory question.",
        missing.join("\n"),
    );
}

/// An exemption must name a field that still exists, so the list cannot rot
/// into permission for a field nobody checks any more.
#[test]
fn every_exemption_still_names_a_client_field() {
    let client = client_struct(&read("src/client.rs"));
    let fields: Vec<String> = client
        .fields
        .iter()
        .filter_map(|f| f.ident.as_ref().map(ToString::to_string))
        .collect();

    for (name, reason) in EXEMPT {
        assert!(
            fields.iter().any(|f| f == name),
            "EXEMPT names `{name}` ({reason}), which is no longer a `Client` field"
        );
    }
}

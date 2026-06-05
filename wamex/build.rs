//! Generate typed Rust for every mex (GraphQL persisted-query) operation from
//! the committed `mex_index.json` IR (extracted by whatspec from the WhatsApp Web
//! bundle).
//!
//! Only the JSON is version-controlled; the generated `operations.rs` is written
//! to `OUT_DIR` and `include!`d by `src/lib.rs`. Refresh by replacing
//! `mex_index.json` with a newer whatspec `generated/mex/index.json`.
//!
//! The codegen mirrors whatspec's own `wa-codegen` mex emitter so the output is
//! the same shape as its reference `generated/mex/operations.rs`.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use serde::Deserialize;

// ─── IR (subset of the whatspec mex schema we consume) ──────────────────────

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MexIr {
    wa_version: String,
    schema_version: String,
    operations: BTreeMap<String, MexOperation>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct MexOperation {
    original_name: String,
    doc_id: String,
    operation_kind: String,
    #[serde(default)]
    variables_shape: BTreeMap<String, TypeNode>,
    #[serde(default)]
    response: BTreeMap<String, TypeNode>,
}

/// A node in a `variablesShape` / `response` type tree: an object renders as a
/// nested struct, a single-element array as a `Vec`, a string leaf as a scalar.
#[derive(Deserialize)]
#[serde(untagged)]
enum TypeNode {
    Object(BTreeMap<String, TypeNode>),
    Array(Vec<TypeNode>),
    Leaf(String),
}

// ─── Identifier casing + Rust keyword escaping ──────────────────────────────

const RUST_KEYWORDS: &[&str] = &[
    "as", "break", "const", "continue", "crate", "else", "enum", "extern", "false", "fn", "for",
    "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return",
    "self", "Self", "static", "struct", "super", "trait", "true", "type", "unsafe", "use", "where",
    "while", "async", "await", "dyn", "abstract", "become", "box", "do", "final", "macro",
    "override", "priv", "typeof", "unsized", "virtual", "yield",
];

/// Keywords that cannot be raw identifiers (`r#self` is a compile error) and so
/// must be disambiguated with a trailing `_` instead.
const RAW_INELIGIBLE: &[&str] = &["self", "Self", "crate", "super"];

fn ensure_ident(s: &str) -> String {
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

/// Insert `sep` at every camelCase boundary: `[a-z0-9]` followed by `[A-Z]`.
fn split_camel(s: &str, sep: char, out: &mut String) {
    let c: Vec<char> = s.chars().collect();
    for i in 0..c.len() {
        out.push(c[i]);
        if i + 1 < c.len()
            && (c[i].is_ascii_lowercase() || c[i].is_ascii_digit())
            && c[i + 1].is_ascii_uppercase()
        {
            out.push(sep);
        }
    }
}

/// Insert `sep` at every acronym boundary: `[A-Z]` followed by `[A-Z][a-z]`
/// (so `HTTPServer` splits into `HTTP`/`Server`).
fn split_acronym(s: &str, sep: char) -> String {
    let c: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len() + 4);
    for i in 0..c.len() {
        out.push(c[i]);
        if i + 2 < c.len()
            && c[i].is_ascii_uppercase()
            && c[i + 1].is_ascii_uppercase()
            && c[i + 2].is_ascii_lowercase()
        {
            out.push(sep);
        }
    }
    out
}

/// `FooBar`/`foo-bar`/`HTTPServer` → `foo_bar`/`http_server`.
fn snake_case(s: &str) -> String {
    let mut camel = String::with_capacity(s.len() + 4);
    split_camel(s, '_', &mut camel);
    let acro = split_acronym(&camel, '_');
    let mut collapsed = String::with_capacity(acro.len());
    let mut prev_us = false;
    for ch in acro.chars() {
        let c = if ch.is_ascii_alphanumeric() { ch } else { '_' };
        if c == '_' {
            if !prev_us {
                collapsed.push('_');
            }
            prev_us = true;
        } else {
            collapsed.push(c);
            prev_us = false;
        }
    }
    collapsed.trim_matches('_').to_lowercase()
}

/// `snake_case` coerced into a valid, keyword-safe Rust identifier.
fn rust_ident(s: &str) -> String {
    ensure_ident(&snake_case(s))
}

/// `foo_bar`/`fooBar` → `FooBar`, coerced into a valid Rust type identifier.
fn pascal_case(s: &str) -> String {
    let mut camel = String::with_capacity(s.len() + 4);
    split_camel(s, ' ', &mut camel);
    let acro = split_acronym(&camel, ' ');
    let spaced: String = acro
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { ' ' })
        .collect();
    let pascal: String = spaced
        .split_whitespace()
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect();
    ensure_ident(&pascal)
}

/// Make `base` a unique, ident-safe module name, suffixing on collision.
fn unique_ident(base: &str, used: &mut HashSet<String>, fallback_prefix: &str) -> String {
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

/// A fully-escaped Rust string literal.
fn lit(s: &str) -> String {
    format!("{s:?}")
}

// ─── Struct emission (nested objects → named, deduped sub-structs) ───────────

struct StructDef {
    name: String,
    fields: Vec<(String, String, String)>,
}

impl StructDef {
    /// `(json_key, field, type)` signature for dedup (ignores the struct name).
    fn signature(&self) -> String {
        self.fields
            .iter()
            .map(|(r, json, t)| format!("{json}={r}:{t}"))
            .collect::<Vec<_>>()
            .join(",")
    }

    fn render(&self, indent: &str) -> String {
        let mut s = String::new();
        s.push_str(&format!(
            "{indent}#[derive(Debug, Clone, Default, Serialize, Deserialize)]\n"
        ));
        s.push_str(&format!("{indent}pub struct {} {{\n", self.name));
        for (rust_field, json_key, ty) in &self.fields {
            if rust_field.trim_start_matches("r#") != json_key {
                s.push_str(&format!(
                    "{indent}    #[serde(rename = {})]\n",
                    lit(json_key)
                ));
            }
            s.push_str(&format!(
                "{indent}    #[serde(default, skip_serializing_if = \"Option::is_none\")]\n"
            ));
            s.push_str(&format!("{indent}    pub {rust_field}: {ty},\n"));
        }
        s.push_str(&format!("{indent}}}\n"));
        s
    }
}

#[derive(Default)]
struct Builder {
    structs: Vec<StructDef>,
    by_name: BTreeMap<String, String>,
}

impl Builder {
    fn register(&mut self, name: &str, fields: &BTreeMap<String, TypeNode>) {
        let mut def = StructDef {
            name: name.to_string(),
            fields: Vec::new(),
        };
        for (key, node) in fields {
            let rust_field = rust_ident(key);
            let hint = pascal_case(key);
            let ty = self.field_type(node, &hint);
            def.fields
                .push((rust_field, key.clone(), format!("Option<{ty}>")));
        }
        self.intern(def);
    }

    fn field_type(&mut self, node: &TypeNode, name_hint: &str) -> String {
        match node {
            TypeNode::Leaf(tag) => scalar_rust(tag).to_string(),
            TypeNode::Array(items) => {
                let inner = items
                    .first()
                    .map(|n| self.field_type(n, name_hint))
                    .unwrap_or_else(|| "String".to_string());
                format!("Vec<{inner}>")
            }
            TypeNode::Object(map) => {
                let mut def = StructDef {
                    name: name_hint.to_string(),
                    fields: Vec::new(),
                };
                for (key, child) in map {
                    let rust_field = rust_ident(key);
                    let child_hint = pascal_case(key);
                    let ty = self.field_type(child, &child_hint);
                    def.fields
                        .push((rust_field, key.clone(), format!("Option<{ty}>")));
                }
                self.intern(def)
            }
        }
    }

    /// Add `def`, reusing an identically-shaped struct of the same name or
    /// disambiguating with a numeric suffix on a name+shape collision.
    fn intern(&mut self, mut def: StructDef) -> String {
        if def.name.is_empty() {
            def.name = "Item".to_string();
        }
        let sig = def.signature();
        let base = def.name.clone();
        let mut name = base.clone();
        let mut n = 2;
        loop {
            match self.by_name.get(&name) {
                Some(existing) if *existing == sig => return name,
                Some(_) => {
                    name = format!("{base}{n}");
                    n += 1;
                }
                None => break,
            }
        }
        def.name = name.clone();
        self.by_name.insert(name.clone(), sig);
        self.structs.push(def);
        name
    }
}

fn scalar_rust(tag: &str) -> &'static str {
    match tag {
        "number" => "i64",
        "boolean" => "bool",
        _ => "String",
    }
}

// ─── Top-level render ───────────────────────────────────────────────────────

fn generate(ir: &MexIr) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "// Auto-generated typed mex operations (WhatsApp {}). DO NOT EDIT.\n\
         // Generated at build time from `mex_index.json` by `wamex/build.rs`.\n\n\
         use serde::{{Deserialize, Serialize}};\n\n",
        ir.wa_version
    ));
    out.push_str(&format!(
        "pub const WA_VERSION: &str = {};\n",
        lit(&ir.wa_version)
    ));
    out.push_str(&format!(
        "pub const SCHEMA_VERSION: &str = {};\n\n",
        lit(&ir.schema_version)
    ));

    // Flat registry: every operation's relay name, persisted doc id, and kind,
    // usable without referencing per-op module names (drift checks, iteration).
    out.push_str(
        "/// Persisted-query descriptor for one mex operation.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq)]\n\
         pub struct OperationMeta {\n    \
         pub name: &'static str,\n    \
         pub doc_id: &'static str,\n    \
         pub kind: &'static str,\n}\n\n",
    );
    out.push_str("/// Every extracted operation, sorted by module name.\n");
    out.push_str("pub const ALL: &[OperationMeta] = &[\n");
    for op in ir.operations.values() {
        out.push_str(&format!(
            "    OperationMeta {{ name: {}, doc_id: {}, kind: {} }},\n",
            lit(&op.original_name),
            lit(&op.doc_id),
            lit(&op.operation_kind),
        ));
    }
    out.push_str("];\n");

    let mut used_mods = HashSet::new();
    for (short, op) in &ir.operations {
        let module = unique_ident(&snake_case(short), &mut used_mods, "op");
        let mut b = Builder::default();
        b.register("Variables", &op.variables_shape);
        b.register("Response", &op.response);

        out.push_str(&format!(
            "\n/// `{}` ({}).\n",
            op.original_name, op.operation_kind
        ));
        out.push_str(&format!("pub mod {module} {{\n"));
        out.push_str("    use super::{Deserialize, Serialize};\n\n");
        out.push_str(&format!(
            "    pub const NAME: &str = {};\n",
            lit(&op.original_name)
        ));
        out.push_str(&format!(
            "    pub const DOC_ID: &str = {};\n",
            lit(&op.doc_id)
        ));
        out.push_str(&format!(
            "    pub const OPERATION_KIND: &str = {};\n\n",
            lit(&op.operation_kind)
        ));
        for s in &b.structs {
            out.push_str(&s.render("    "));
            out.push('\n');
        }
        out.push_str("}\n");
    }
    out
}

fn main() {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR");
    let json_path = Path::new(&manifest).join("mex_index.json");
    println!("cargo:rerun-if-changed=mex_index.json");
    println!("cargo:rerun-if-changed=build.rs");

    let raw = std::fs::read_to_string(&json_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", json_path.display()));
    let ir: MexIr = serde_json::from_str(&raw).expect("parse mex_index.json");

    let code = generate(&ir);
    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR");
    let out_path = Path::new(&out_dir).join("operations.rs");
    std::fs::write(&out_path, code).expect("write operations.rs");
}

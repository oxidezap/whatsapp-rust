//! `wacore/src/iq/mex_operations.rs`: typed inputs and outputs for every
//! persisted Relay operation.
//!
//! One `pub mod` per operation, so a caller that names one operation pulls in
//! that operation's types and nothing else.

use std::collections::{BTreeMap, HashSet};

use crate::ir::{MexIr, TypeNode};
use crate::naming::{pascal_case, rust_ident, rust_lit, snake_case, unique_ident};

const HEADER: &str = "\
//! One module per persisted GraphQL operation: typed `Variables` + `Response`
//! plus `DOC_ID`/`OPERATION_KIND`/`NAME`. Depends only on `serde`.

#![allow(clippy::all)]

use serde::{Deserialize, Serialize};
";

pub fn generate(ir: &MexIr) -> String {
    let mut out = super::header("typed mex operations", &ir.wa_version);
    out.push_str(HEADER);

    let mut used_mods = HashSet::new();
    for (short, op) in &ir.operations {
        // An operation named e.g. `match` would otherwise emit `pub mod match`.
        let module = unique_ident(&snake_case(short), &mut used_mods, "op");
        let mut b = Builder::default();
        // Both before either walk: a nested object inside `variables_shape` can
        // just as easily want the name `Response`.
        b.reserve("Variables");
        b.reserve("Response");
        b.register("Variables", &op.variables_shape);
        b.register("Response", &op.response);

        let kind = op.operation_kind.as_str();
        out.push_str(&format!("\n/// `{}` ({kind}).\n", op.original_name));
        out.push_str(&format!("pub mod {module} {{\n"));
        out.push_str("    use super::{Deserialize, Serialize};\n\n");
        out.push_str(&format!(
            "    pub const NAME: &str = {};\n",
            rust_lit(&op.original_name)
        ));
        out.push_str(&format!(
            "    pub const DOC_ID: &str = {};\n",
            rust_lit(&op.doc_id)
        ));
        out.push_str(&format!(
            "    pub const OPERATION_KIND: &str = {};\n\n",
            rust_lit(kind)
        ));
        for s in &b.structs {
            out.push_str(&s.render("    "));
            out.push('\n');
        }
        out.push_str("}\n");
    }
    out
}

/// A generated struct: name plus ordered `(rust_field, json_key, type)`.
struct StructDef {
    name: String,
    fields: Vec<(String, String, String)>,
}

impl StructDef {
    /// Shape signature used to merge identical structs. The JSON key is part of
    /// it: two structs with the same Rust fields but different `serde(rename)`
    /// keys are different types on the wire.
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
                    rust_lit(json_key)
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
    /// Struct name to its signature, to merge identical shapes and disambiguate
    /// distinct shapes that want the same name.
    by_name: BTreeMap<String, String>,
    /// Names the operation module has to expose verbatim.
    reserved: HashSet<String>,
}

impl Builder {
    /// Claim a name for a top-level struct before any nested object can take it.
    ///
    /// Children are interned before their parent, so without this a response
    /// field literally named `response` renames the operation's own `Response`
    /// to `Response2` — a generated-API break that depends on field naming luck.
    fn reserve(&mut self, name: &str) {
        self.reserved.insert(name.to_string());
    }

    fn register(&mut self, name: &str, fields: &BTreeMap<String, TypeNode>) -> String {
        let def = self.struct_def(name, fields);
        self.intern(def, true)
    }

    /// Build one struct, interning any object-typed field as its own struct.
    fn struct_def(&mut self, name: &str, fields: &BTreeMap<String, TypeNode>) -> StructDef {
        let mut def = StructDef {
            name: name.to_string(),
            fields: Vec::new(),
        };
        // `rust_ident` is not injective: `fooBar` and `foo_bar` are one Rust
        // field. Two of them in the same object would emit the field twice.
        let mut used_fields = HashSet::new();
        for (key, node) in fields {
            let ty = self.field_type(node, &pascal_case(key));
            let field = unique_ident(&rust_ident(key), &mut used_fields, "f");
            def.fields
                .push((field, key.clone(), format!("Option<{ty}>")));
        }
        def
    }

    fn field_type(&mut self, node: &TypeNode, name_hint: &str) -> String {
        match node {
            TypeNode::Leaf(tag) => scalar_rust(tag).to_string(),
            TypeNode::Array(items) => {
                // The IR carries a single element shape; an empty array would
                // still have to name a type.
                let inner = items
                    .first()
                    .map(|n| self.field_type(n, name_hint))
                    .unwrap_or_else(|| "String".to_string());
                format!("Vec<{inner}>")
            }
            TypeNode::Object(map) => {
                let def = self.struct_def(name_hint, map);
                self.intern(def, false)
            }
        }
    }

    fn intern(&mut self, mut def: StructDef, top_level: bool) -> String {
        if def.name.is_empty() {
            def.name = "Item".to_string();
        }
        let sig = def.signature();
        let base = def.name.clone();
        let mut name = base.clone();
        let mut n = 2;
        loop {
            if !top_level && self.reserved.contains(&name) {
                name = format!("{base}{n}");
                n += 1;
                continue;
            }
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

/// The IR's scalar tags. Anything else, including `enum:A|B` and `unknown`,
/// deserializes as a string: the extractor cannot prove a closed variant set,
/// and a wrong enum would fail the whole response.
fn scalar_rust(tag: &str) -> &'static str {
    match tag {
        "number" => "i64",
        "boolean" => "bool",
        _ => "String",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{MexOperation, MexOperationKind};

    fn leaf(t: &str) -> TypeNode {
        TypeNode::Leaf(t.to_string())
    }

    fn obj(fields: &[(&str, TypeNode)]) -> TypeNode {
        TypeNode::Object(
            fields
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
        )
    }

    fn ir(key: &str, op: MexOperation) -> MexIr {
        MexIr {
            wa_version: "2.3000.1".into(),
            operations: BTreeMap::from([(key.to_string(), op)]),
        }
    }

    fn op(variables: &[(&str, TypeNode)], response: &[(&str, TypeNode)]) -> MexOperation {
        MexOperation {
            original_name: "WAWebFetchThingQuery".into(),
            doc_id: "123".into(),
            operation_kind: MexOperationKind::Query,
            variables_shape: variables
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
            response: response
                .iter()
                .map(|(k, v)| (k.to_string(), v.clone()))
                .collect(),
        }
    }

    #[test]
    fn emits_a_module_with_the_persisted_ids_and_typed_shapes() {
        let code = generate(&ir(
            "FetchThing",
            op(
                &[("thing_id", leaf("string"))],
                &[(
                    "xwa2_thing",
                    obj(&[("count", leaf("number")), ("ok", leaf("boolean"))]),
                )],
            ),
        ));
        assert!(code.contains("pub mod fetch_thing {"));
        assert!(code.contains("pub const NAME: &str = \"WAWebFetchThingQuery\";"));
        assert!(code.contains("pub const DOC_ID: &str = \"123\";"));
        assert!(code.contains("pub const OPERATION_KIND: &str = \"query\";"));
        assert!(code.contains("pub thing_id: Option<String>,"));
        assert!(code.contains("pub count: Option<i64>,"));
        assert!(code.contains("pub ok: Option<bool>,"));
        assert!(code.contains("pub struct Xwa2Thing {"));
        assert!(code.contains("pub xwa2_thing: Option<Xwa2Thing>,"));
        // Nested structs are declared before the struct that names them.
        let nested = code.find("pub struct Xwa2Thing").expect("nested struct");
        let response = code.find("pub struct Response").expect("response struct");
        assert!(nested < response);
    }

    #[test]
    fn a_plural_field_becomes_a_vec_of_its_element_struct() {
        let code = generate(&ir(
            "FetchThing",
            op(
                &[],
                &[(
                    "items",
                    TypeNode::Array(vec![obj(&[("id", leaf("string"))])]),
                )],
            ),
        ));
        assert!(code.contains("pub struct Items {"));
        assert!(code.contains("pub items: Option<Vec<Items>>,"));
    }

    #[test]
    fn json_keys_that_are_not_rust_idents_get_a_rename() {
        let code = generate(&ir(
            "FetchThing",
            op(
                &[],
                &[("__typename", leaf("string")), ("type", leaf("string"))],
            ),
        ));
        assert!(code.contains("#[serde(rename = \"__typename\")]"));
        assert!(code.contains("pub typename: Option<String>,"));
        // A raw identifier already spells its JSON key, so it needs no rename.
        assert!(code.contains("pub r#type: Option<String>,"));
        assert!(!code.contains("#[serde(rename = \"type\")]"));
    }

    #[test]
    fn same_named_structs_with_different_shapes_are_kept_apart() {
        let code = generate(&ir(
            "FetchThing",
            op(
                &[("node", obj(&[("a", leaf("string"))]))],
                &[("node", obj(&[("b", leaf("number"))]))],
            ),
        ));
        assert!(code.contains("pub struct Node {"));
        assert!(code.contains("pub struct Node2 {"));
    }

    #[test]
    fn an_identical_shape_reused_under_the_same_name_is_emitted_once() {
        let code = generate(&ir(
            "FetchThing",
            op(
                &[("node", obj(&[("a", leaf("string"))]))],
                &[("node", obj(&[("a", leaf("string"))]))],
            ),
        ));
        assert_eq!(code.matches("pub struct Node {").count(), 1);
        assert!(!code.contains("pub struct Node2"));
    }

    #[test]
    fn a_nested_object_cannot_take_the_top_level_struct_names() {
        // `op::Variables` and `op::Response` are the generated API. A field
        // named after either one must not push the real struct to `Response2`.
        let code = generate(&ir(
            "FetchThing",
            op(
                &[("response", obj(&[("a", leaf("string"))]))],
                &[("variables", obj(&[("b", leaf("string"))]))],
            ),
        ));
        assert!(code.contains("pub struct Variables {"), "{code}");
        assert!(code.contains("pub struct Response {"), "{code}");
        assert!(code.contains("pub struct Response2 {"), "{code}");
        assert!(code.contains("pub struct Variables2 {"), "{code}");
    }

    #[test]
    fn two_keys_that_snake_case_alike_stay_two_fields() {
        // `rust_ident` is not injective, and a struct cannot declare the same
        // field twice.
        let code = generate(&ir(
            "FetchThing",
            op(
                &[],
                &[("fooBar", leaf("string")), ("foo_bar", leaf("string"))],
            ),
        ));
        assert_eq!(code.matches("pub foo_bar:").count(), 1, "{code}");
        assert_eq!(code.matches("pub foo_bar_2:").count(), 1, "{code}");
        // Both keys still deserialize from their own wire name.
        assert!(code.contains("#[serde(rename = \"fooBar\")]"), "{code}");
        assert!(code.contains("#[serde(rename = \"foo_bar\")]"), "{code}");
    }

    #[test]
    fn an_unmodelled_leaf_tag_falls_back_to_string() {
        let code = generate(&ir(
            "FetchThing",
            op(
                &[],
                &[("state", leaf("enum:ON|OFF")), ("x", leaf("unknown"))],
            ),
        ));
        assert!(code.contains("pub state: Option<String>,"));
        assert!(code.contains("pub x: Option<String>,"));
    }
}

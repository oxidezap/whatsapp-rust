//! `wacore/appstate/src/schemas.rs`: the syncd action registry.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use anyhow::{Context, Result};

use crate::ir::{AppstateIr, IndexPart};
use crate::naming::{pascal_case, rust_lit, snake_case, unique_ident, unique_type_ident};

const HEADER: &str = "\
//! Typed registry of syncd actions: collection, version, scope, value proto type,
//! enum fields, and the mutation-index parts. `const`/`&'static`, no deps.

#![allow(clippy::all)]

";

/// The fixed part of the model: the index-part union and the schema record.
const INDEX_PART_AND_SCHEMA: &str = "\
/// One component of a mutation index key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexPart {
    /// Fixed wire name at position 0.
    Literal { value: &'static str },
    /// A WhatsApp JID (legacy-encoded).
    Jid { name: &'static str },
    /// '0' or '1' bool encoding.
    BoolString { name: &'static str },
    /// Participant slot: a JID, or '0' when fromMe/null.
    JidOrZero { name: &'static str },
    /// Stringified protobuf-enum integer; `proto_enum` is the dotted enum path.
    Enum {
        name: &'static str,
        proto_enum: &'static str,
    },
    /// Opaque identifier (msg/label/agent id, etc.).
    StringPart { name: &'static str },
    /// Unrecognized slot.
    Unknown { name: &'static str },
}

/// A syncd action schema.
#[derive(Debug, Clone, Copy)]
pub struct Schema {
    /// Registry key (e.g. \"Agent\").
    pub key: &'static str,
    /// On-wire action name (e.g. \"deviceAgent\").
    pub name: &'static str,
    /// Source WA Web module.
    pub module: &'static str,
    pub collection: Collection,
    pub version: u32,
    pub scope: Scope,
    /// Field on `SyncActionValue` carrying the payload.
    pub value_field: Option<&'static str>,
    /// Dotted protobuf type of the value (in `waproto`).
    pub value_proto_type: Option<&'static str>,
    /// `(field, dotted enum path)` for enum-typed value fields.
    pub value_enum_fields: &'static [(&'static str, &'static str)],
    /// Index position holding the chat JID, if any.
    pub chat_jid_index: Option<i64>,
    pub index_parts: &'static [IndexPart],
}

";

pub fn generate(ir: &AppstateIr) -> Result<String> {
    let mut out = super::header("AppState (syncd) action schemas", &ir.wa_version);
    out.push_str(HEADER);

    let (collection_enum, collection_variants) = render_enum(
        "/// A syncd collection (mutation bucket / priority).",
        "Collection",
        ir.collections.iter().map(String::as_str),
    );
    out.push_str(&collection_enum);
    let scopes: BTreeSet<&str> = ir.actions.values().map(|a| a.scope.as_str()).collect();
    let (scope_enum, scope_variants) = render_enum(
        "/// The index scope an action applies to.",
        "Scope",
        scopes.iter().copied(),
    );
    out.push_str(&scope_enum);
    out.push_str(INDEX_PART_AND_SCHEMA);

    // Every `Collection::` and `Scope::` below has to name the variant the enum
    // just declared. Re-deriving it with `pascal_case` is how two wire strings
    // that normalize alike end up sharing one variant: the file still compiles
    // and one action silently carries the other's value.
    let variant = |wire: &str| -> Result<String> {
        collection_variants
            .get(wire)
            .map(|v| format!("Collection::{v}"))
            .with_context(|| format!("collection {wire:?} is not in the IR's collection list"))
    };
    let scope_variant = |wire: &str| -> Result<String> {
        scope_variants
            .get(wire)
            .map(|v| format!("Scope::{v}"))
            .with_context(|| format!("scope {wire:?} is not in the declared scope set"))
    };
    let collections: Vec<String> = ir
        .collections
        .iter()
        .map(|c| variant(c))
        .collect::<Result<_>>()?;
    out.push_str(&format!(
        "/// All syncd collections, in dependency order.\npub const COLLECTIONS: &[Collection] = &[{}];\n\n",
        collections.join(", ")
    ));
    // The bucket an action falls into when the bundle names none.
    let default_collection = variant("regular").context(
        "the IR declares no `regular` collection, so the no-bucket fallback would not compile",
    )?;

    let mut all_entries: Vec<String> = Vec::new();
    let mut used_consts = HashSet::new();
    for (key, action) in &ir.actions {
        let const_name = unique_ident(&snake_case(key).to_uppercase(), &mut used_consts, "ACTION");
        let collection = match action.collection.as_deref() {
            Some(c) => variant(c).with_context(|| format!("action {key}"))?,
            None => default_collection.clone(),
        };
        let enum_fields = match &action.value_enum_fields {
            Some(m) if !m.is_empty() => format!(
                "&[{}]",
                m.iter()
                    .map(|(k, v)| format!("({}, {})", rust_lit(k), rust_lit(v)))
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            _ => "&[]".to_string(),
        };
        let index_parts = action
            .index_parts
            .iter()
            .map(render_index_part)
            .collect::<Vec<_>>()
            .join(", ");

        out.push_str(&format!(
            "/// `{key}` — module `{module}`.\npub const {const_name}: Schema = Schema {{\n\
             \x20   key: {key_lit},\n    name: {name_lit},\n    module: {module_lit},\n    collection: {collection},\n\
             \x20   version: {version},\n    scope: {scope},\n    value_field: {value_field},\n\
             \x20   value_proto_type: {value_proto},\n    value_enum_fields: {enum_fields},\n\
             \x20   chat_jid_index: {chat_jid_index},\n    index_parts: &[{index_parts}],\n}};\n\n",
            module = action.module,
            key_lit = rust_lit(key),
            name_lit = rust_lit(&action.name),
            module_lit = rust_lit(&action.module),
            version = action.version.unwrap_or(0),
            scope = scope_variant(&action.scope).with_context(|| format!("action {key}"))?,
            value_field = opt_lit(action.value_field.as_deref()),
            value_proto = opt_lit(action.value_proto_type.as_deref()),
            chat_jid_index = match action.chat_jid_index {
                Some(i) => format!("Some({i})"),
                None => "None".to_string(),
            },
        ));
        all_entries.push(const_name);
    }

    out.push_str("/// Every action schema, keyed-sorted.\npub const ALL: &[Schema] = &[\n");
    for name in &all_entries {
        out.push_str(&format!("    {name},\n"));
    }
    out.push_str("];\n\n");
    out.push_str(
        "/// Look up a schema by its action key (the registry key, e.g. `\"Agent\"`).\n\
         pub fn by_name(key: &str) -> Option<&'static Schema> {\n\
         \x20   ALL.iter().find(|s| s.key == key)\n}\n",
    );
    Ok(out)
}

fn opt_lit(v: Option<&str>) -> String {
    match v {
        Some(s) => format!("Some({})", rust_lit(s)),
        None => "None".to_string(),
    }
}

/// A C-like enum over wire strings, plus the `as_str` that maps back.
///
/// Returns the rendered enum and the wire-string-to-variant map, because
/// `pascal_case` is not injective (`critical_block` and `criticalBlock` both
/// give `CriticalBlock`) and every site that names a variant has to agree with
/// the declaration on which of the two got suffixed.
fn render_enum<'a>(
    doc: &str,
    name: &str,
    values: impl Iterator<Item = &'a str>,
) -> (String, BTreeMap<String, String>) {
    let mut used = HashSet::new();
    let variants: Vec<(&str, String)> = values
        .map(|v| (v, unique_type_ident(&pascal_case(v), &mut used, "V")))
        .collect();

    let mut s =
        format!("{doc}\n#[derive(Debug, Clone, Copy, PartialEq, Eq)]\npub enum {name} {{\n");
    for (_, variant) in &variants {
        s.push_str(&format!("    {variant},\n"));
    }
    s.push_str(&format!(
        "}}\n\nimpl {name} {{\n    pub const fn as_str(self) -> &'static str {{\n        match self {{\n"
    ));
    for (wire, variant) in &variants {
        s.push_str(&format!(
            "            {name}::{variant} => {},\n",
            rust_lit(wire)
        ));
    }
    s.push_str("        }\n    }\n}\n\n");

    let map = variants
        .into_iter()
        .map(|(wire, variant)| (wire.to_string(), variant))
        .collect();
    (s, map)
}

fn render_index_part(part: &IndexPart) -> String {
    match part {
        IndexPart::Literal { value } => {
            format!("IndexPart::Literal {{ value: {} }}", rust_lit(value))
        }
        IndexPart::Jid { name } => format!("IndexPart::Jid {{ name: {} }}", rust_lit(name)),
        IndexPart::BoolString { name } => {
            format!("IndexPart::BoolString {{ name: {} }}", rust_lit(name))
        }
        IndexPart::JidOrZero { name } => {
            format!("IndexPart::JidOrZero {{ name: {} }}", rust_lit(name))
        }
        IndexPart::Enum { name, proto_enum } => format!(
            "IndexPart::Enum {{ name: {}, proto_enum: {} }}",
            rust_lit(name),
            rust_lit(proto_enum)
        ),
        IndexPart::StringPart { name } => {
            format!("IndexPart::StringPart {{ name: {} }}", rust_lit(name))
        }
        IndexPart::Unknown { name } => format!("IndexPart::Unknown {{ name: {} }}", rust_lit(name)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The emitters are fallible now; a test IR that trips a guard should
    /// fail the test, not be silently skipped.
    fn emitted(ir: &AppstateIr) -> String {
        generate(ir).expect("emitter rejected the test IR")
    }
    use crate::ir::AppstateAction;
    use std::collections::BTreeMap;

    fn action(name: &str, scope: &str, collection: Option<&str>) -> AppstateAction {
        AppstateAction {
            module: "WAWebAgentSync".into(),
            name: name.into(),
            collection: collection.map(Into::into),
            version: Some(7),
            scope: scope.into(),
            value_field: Some("agentAction".into()),
            value_proto_type: Some("SyncActionValue.AgentAction".into()),
            value_enum_fields: None,
            chat_jid_index: None,
            index_parts: vec![
                IndexPart::Literal { value: name.into() },
                IndexPart::StringPart {
                    name: "agentId".into(),
                },
            ],
        }
    }

    fn ir(actions: Vec<(&str, AppstateAction)>) -> AppstateIr {
        AppstateIr {
            wa_version: "2.3000.1".into(),
            collections: vec!["regular".into(), "critical_block".into()],
            actions: actions
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
        }
    }

    #[test]
    fn an_action_naming_an_undeclared_collection_is_rejected() {
        // The variant would not exist on the enum, so the emitted file would
        // not compile and the failure would land on the next build instead.
        let err = generate(&ir(vec![(
            "Agent",
            action("deviceAgent", "account", Some("no_such_bucket")),
        )]))
        .expect_err("an undeclared collection must not be emitted");
        assert!(format!("{err:#}").contains("no_such_bucket"), "{err:#}");
    }

    #[test]
    fn an_ir_without_the_regular_collection_is_rejected() {
        // Actions with no declared bucket fall back to it, so its absence is
        // the same dangling reference one step removed.
        let mut bad = ir(vec![("Nux", action("nux", "account", None))]);
        bad.collections = vec!["critical_block".into()];
        let err = generate(&bad).expect_err("the fallback bucket must exist");
        assert!(err.to_string().contains("regular"), "{err}");
    }

    #[test]
    fn collection_names_that_pascal_case_alike_stay_distinct_variants() {
        let mut ir = ir(vec![("Agent", action("deviceAgent", "account", None))]);
        ir.collections = vec![
            "regular".into(),
            "criticalBlock".into(),
            "critical_block".into(),
        ];
        let code = emitted(&ir);
        assert!(code.contains("    CriticalBlock,\n"), "{code}");
        assert!(code.contains("    CriticalBlock2,\n"), "{code}");
        // Each variant keeps its own wire string.
        assert!(code.contains("Collection::CriticalBlock => \"criticalBlock\","));
        assert!(code.contains("Collection::CriticalBlock2 => \"critical_block\","));
    }

    #[test]
    fn scope_names_that_pascal_case_alike_stay_distinct_variants() {
        // Worse than the collection case: re-deriving the name would compile
        // and silently give one action the other's scope.
        let code = emitted(&ir(vec![
            ("A", action("a", "critical_block", None)),
            ("B", action("b", "criticalBlock", None)),
        ]));
        assert!(
            code.contains("Scope::CriticalBlock => \"criticalBlock\","),
            "{code}"
        );
        assert!(
            code.contains("Scope::CriticalBlock2 => \"critical_block\","),
            "{code}"
        );
        assert!(code.contains("scope: Scope::CriticalBlock2,"), "{code}");
        assert!(code.contains("scope: Scope::CriticalBlock,"), "{code}");
    }

    #[test]
    fn a_scope_whose_pascal_case_is_escaped_still_names_the_declared_variant() {
        // `ensure_ident` escapes `Self` to `Self_`; formatting the variant
        // independently would emit `Scope::Self`, which does not exist.
        let code = emitted(&ir(vec![("A", action("a", "self", None))]));
        assert!(code.contains("Scope::Self_ => \"self\","), "{code}");
        assert!(code.contains("scope: Scope::Self_,"), "{code}");
    }

    #[test]
    fn emits_a_const_per_action_with_its_index() {
        let code = emitted(&ir(vec![(
            "Agent",
            action("deviceAgent", "account", Some("regular")),
        )]));
        assert!(code.contains("pub enum Collection {\n    Regular,\n    CriticalBlock,\n}"));
        assert!(code.contains("Collection::CriticalBlock => \"critical_block\","));
        assert!(code.contains("pub enum Scope {\n    Account,\n}"));
        assert!(code.contains("pub const AGENT: Schema = Schema {"));
        assert!(code.contains("key: \"Agent\","));
        assert!(code.contains("collection: Collection::Regular,"));
        assert!(code.contains("scope: Scope::Account,"));
        assert!(code.contains(
            "index_parts: &[IndexPart::Literal { value: \"deviceAgent\" }, IndexPart::StringPart { name: \"agentId\" }],"
        ));
        assert!(code.contains("pub const ALL: &[Schema] = &[\n    AGENT,\n];"));
    }

    #[test]
    fn an_action_without_a_collection_falls_back_to_regular() {
        // WA Web builds a few actions with no declared bucket; the wire default
        // is the regular collection, and emitting `Collection::` alone would not
        // compile.
        let code = emitted(&ir(vec![("Nux", action("nux", "account", None))]));
        assert!(code.contains("collection: Collection::Regular,"));
    }

    #[test]
    fn enum_index_parts_and_value_enum_fields_survive() {
        let mut a = action("settingsSync", "account", Some("regular"));
        a.index_parts = vec![IndexPart::Enum {
            name: "k".into(),
            proto_enum: "SettingsSyncAction.SettingKey".into(),
        }];
        a.value_enum_fields = Some(BTreeMap::from([(
            "settingKey".to_string(),
            "SettingsSyncAction.SettingKey".to_string(),
        )]));
        let code = emitted(&ir(vec![("SettingsSync", a)]));
        assert!(code.contains(
            "IndexPart::Enum { name: \"k\", proto_enum: \"SettingsSyncAction.SettingKey\" }"
        ));
        assert!(code.contains(
            "value_enum_fields: &[(\"settingKey\", \"SettingsSyncAction.SettingKey\")],"
        ));
    }

    #[test]
    fn two_keys_that_upper_case_alike_do_not_collide() {
        let code = emitted(&ir(vec![
            ("Agent", action("deviceAgent", "account", Some("regular"))),
            ("AGENT", action("deviceAgent2", "account", Some("regular"))),
        ]));
        assert!(code.contains("pub const AGENT: Schema"));
        assert!(code.contains("pub const AGENT_2: Schema"));
    }
}

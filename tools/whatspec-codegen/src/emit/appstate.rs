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
///
/// Consumers must allow new index-part variants.
///
/// ```compile_fail,E0004
/// use wacore_appstate::schemas::IndexPart;
/// fn exhaustive(part: IndexPart) {
///     match part {
///         IndexPart::Literal { .. } | IndexPart::Jid { .. }
///         | IndexPart::BoolString { .. } | IndexPart::JidOrZero { .. }
///         | IndexPart::Enum { .. } | IndexPart::StringPart { .. }
///         | IndexPart::Unknown { .. } => (),
///     }
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
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
///
/// Use [`Schema::new`] or copy a registry entry, leaving room for new metadata.
///
/// ```compile_fail,E0639
/// use wacore_appstate::schemas::{Schema, ALL};
/// let schema = Schema { ..ALL[0] };
/// ```
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
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

impl Schema {
    /// Construct a schema with no optional metadata.
    ///
    /// Public fields remain readable and writable for host-defined actions.
    ///
    /// ```
    /// use wacore_appstate::schemas::{Collection, IndexPart, Schema, Scope};
    /// const CUSTOM: Schema = Schema::new(
    ///     \"Custom\", \"custom\", \"Host\", Collection::Regular, 1, Scope::Account,
    ///     &[IndexPart::Literal { value: \"custom\" }],
    /// );
    /// let mut action = CUSTOM;
    /// action.value_field = Some(\"customAction\");
    /// let Schema { name, value_field, .. } = action;
    /// assert_eq!(name, \"custom\");
    /// assert_eq!(value_field, Some(\"customAction\"));
    /// ```
    pub const fn new(
        key: &'static str,
        name: &'static str,
        module: &'static str,
        collection: Collection,
        version: u32,
        scope: Scope,
        index_parts: &'static [IndexPart],
    ) -> Self {
        Self {
            key,
            name,
            module,
            collection,
            version,
            scope,
            value_field: None,
            value_proto_type: None,
            value_enum_fields: &[],
            chat_jid_index: None,
            index_parts,
        }
    }
}

";

/// What `generate` returns: the full registry plus the compact log-gating
/// copy. Both come from one IR pass so the two files cannot drift: the
/// root crate's logger embeds only match arms over string literals, never
/// `Schema` records.
#[derive(Debug)]
pub struct Generated {
    pub schemas: String,
    pub known_verbs: String,
}

pub fn generate(ir: &AppstateIr) -> Result<Generated> {
    let mut out = super::header("AppState (syncd) action schemas", &ir.wa_version);
    out.push_str(HEADER);

    let (collection_enum, collection_variants) = render_enum(
        "/// A syncd collection (mutation bucket / priority).",
        "Collection",
        ir.collections.iter().map(String::as_str),
    );
    out.push_str(&collection_enum);
    out.push_str(&render_patch_names(&collection_variants)?);
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
    out.push_str(&log_gate_matcher(ir));
    out.push_str(
        "/// Look up a schema by its action key (the registry key, e.g. `\"Agent\"`).\n\
         pub fn by_name(key: &str) -> Option<&'static Schema> {\n\
         \x20   ALL.iter().find(|s| s.key == key)\n}\n",
    );
    Ok(Generated {
        schemas: out,
        known_verbs: known_verbs_module_inner(ir),
    })
}

/// The root crate's own log-gating copy: same arms as [`log_gate_matcher`],
/// wrapped as a standalone generated module so `whatsapp-rust` gates on the
/// same set without referencing `Schema` records. `main.rs` builds it from
/// `Generated.known_verbs` (same IR pass as schemas); this fn exists so
/// tests can build it from a fixture without the full registry.
#[cfg(test)]
pub fn known_verbs_module(ir: &AppstateIr, wa_version: &str) -> Result<String> {
    Ok(format!(
        "{}\n{}\n",
        super::header("AppState known verbs (log gating)", wa_version),
        known_verbs_module_inner(ir)
    ))
}

fn known_verbs_module_inner(ir: &AppstateIr) -> String {
    format!(
        "//! Declared syncd action names for log gating. Shares its arms with\n\
         //! `wacore-appstate`'s registry (see `emit::appstate::log_gate_matcher`);\n\
         //! a `match` over string literals, never `Schema` records.\n\
         #![allow(clippy::all)]\n\
         \n\
         pub(crate) fn is_known_wire_name(name: &str) -> bool {{\n    match name {{\n{arms}\n        _ => false,\n    }}\n}}\n",
        arms = log_gate_match_arms(ir)
    )
}

/// Compact log-gating matcher shared by both crates. Every on-wire action
/// name becomes a `match` arm over string literals, so referencing a name
/// keeps just the compared bytes in the binary — unlike `schemas::ALL`, it
/// can never pull the full `Schema` records reachable. `log_gate_match_arms`
/// below reuses the same arms for the root crate's own generated copy.
fn log_gate_matcher(ir: &AppstateIr) -> String {
    format!(
        "/// Whether `name` is an on-wire action name. Log gating only: a\n\
         /// `match` over string literals, so referencing a name keeps just\n\
         /// the compared bytes, never the full `Schema` record.\n\
         pub(crate) fn is_known_wire_name(name: &str) -> bool {{\n    match name {{\n{arms}\n        _ => false,\n    }}\n}}\n",
        arms = log_gate_match_arms(ir)
    )
}

/// The match arms of [`log_gate_matcher`], without the surrounding fn.
/// The root crate embeds these in its own generated module so its logger
/// gates on the same set without referencing `Schema` records.
fn log_gate_match_arms(ir: &AppstateIr) -> String {
    let mut wire_names: Vec<&str> = ir.actions.values().map(|a| a.name.as_str()).collect();
    wire_names.sort_unstable();
    wire_names
        .iter()
        .map(|name| format!("        {name_lit} => true,", name_lit = rust_lit(name)))
        .collect::<Vec<_>>()
        .join("\n")
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

    let mut s = format!(
        "{doc}\n///\n/// Consumers must allow new catalog variants.\n///\n\
         /// ```compile_fail,E0004\n/// use wacore_appstate::schemas::{name};\n\
         /// fn exhaustive(value: {name}) {{\n///     match value {{\n"
    );
    for (_, variant) in &variants {
        s.push_str(&format!("///         {name}::{variant} => (),\n"));
    }
    s.push_str(&format!(
        "///     }}\n/// }}\n/// ```\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq)]\n#[non_exhaustive]\npub enum {name} {{\n"
    ));
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

/// The sync engine must know every collection the action catalog can name.
/// Keep its legacy Unknown sentinel for names absent from the pinned catalog.
fn render_patch_names(collections: &BTreeMap<String, String>) -> Result<String> {
    anyhow::ensure!(
        !collections.contains_key("unknown"),
        "collection `unknown` is reserved for the runtime sentinel"
    );
    let mut used = HashSet::from(["Unknown".to_owned()]);
    let mut names = BTreeMap::new();
    for (wire, variant) in collections {
        names.insert(
            wire.as_str(),
            unique_type_ident(variant, &mut used, "Collection"),
        );
    }
    names.insert("unknown", "Unknown".to_owned());
    anyhow::ensure!(names.len() <= 256, "collection reservation ranks exceed u8");
    let mut out = String::from(
        "/// Runtime sync collection, derived from the same catalog as [`Collection`].\n\
         /// Unlisted wire names retain the legacy [`Self::Unknown`] fallback.\n\
         #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n\
         #[non_exhaustive]\npub enum WAPatchName {\n",
    );
    for variant in names.values() {
        out.push_str(&format!("    {variant},\n"));
    }
    out.push_str("}\n\nimpl WAPatchName {\n    pub fn as_str(&self) -> &'static str {\n        match self {\n");
    for (wire, variant) in &names {
        out.push_str(&format!(
            "            Self::{variant} => {},\n",
            rust_lit(wire)
        ));
    }
    out.push_str("        }\n    }\n\n    /// Total reservation order, sorted by the exact wire name.\n    pub const fn reservation_rank(self) -> u8 {\n        match self {\n");
    for (rank, variant) in names.values().enumerate() {
        out.push_str(&format!("            Self::{variant} => {rank},\n"));
    }
    out.push_str("        }\n    }\n}\n\nimpl std::str::FromStr for WAPatchName {\n    type Err = ();\n    fn from_str(value: &str) -> Result<Self, Self::Err> {\n        Ok(match value {\n");
    for (wire, variant) in &names {
        if wire != &"unknown" {
            out.push_str(&format!(
                "            {} => Self::{variant},\n",
                rust_lit(wire)
            ));
        }
    }
    out.push_str("            _ => Self::Unknown,\n        })\n    }\n}\n\n");
    Ok(out)
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
        generate(ir).expect("emitter rejected the test IR").schemas
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
    fn new_catalog_collection_survives_runtime_sync_conversion() {
        let mut fixture = ir(vec![(
            "Future",
            action("futureAction", "account", Some("future_sync_bucket")),
        )]);
        fixture.collections.push("future_sync_bucket".into());
        fixture.collections.push("z_future_bucket".into());
        let mut code = emitted(&fixture);
        code.push_str(
            r#"
fn main() {
    let schema = by_name("Future").unwrap();
    let runtime = schema.collection.as_str().parse::<WAPatchName>().unwrap();
    assert_ne!(runtime, WAPatchName::Unknown);
    assert_eq!(runtime.as_str(), "future_sync_bucket");
    assert_eq!("unlisted_runtime_bucket".parse::<WAPatchName>().unwrap(), WAPatchName::Unknown);
    let mut names: Vec<_> = COLLECTIONS.iter()
        .map(|c| c.as_str().parse::<WAPatchName>().unwrap()).collect();
    names.push(WAPatchName::Unknown);
    names.sort_by_key(|n| n.reservation_rank());
    let sorted: Vec<_> = names.iter().map(WAPatchName::as_str).collect();
    let mut expected = sorted.clone();
    expected.sort_unstable();
    expected.dedup();
    assert_eq!(sorted, expected);
}
"#,
        );
        let dir =
            std::env::temp_dir().join(format!("whatspec-appstate-runtime-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let source = dir.join("probe.rs");
        let executable = dir.join(format!("probe{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&source, code).unwrap();
        let output = std::process::Command::new("rustc")
            .args(["--edition=2024", "--crate-name", "collection_probe"])
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = std::process::Command::new(&executable).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn runtime_unknown_sentinel_cannot_alias_a_catalog_collection() {
        let mut fixture = ir(vec![]);
        fixture.collections.push("unknown".into());
        let Err(error) = generate(&fixture) else {
            panic!("the runtime sentinel is not a collection");
        };
        assert!(error.to_string().contains("reserved"));
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
        // Log gating: match arms over literals from the IR, never `X.name`
        // references that could keep full `Schema` records reachable.
        assert!(code.contains("pub(crate) fn is_known_wire_name(name: &str) -> bool"));
        assert!(code.contains("\"deviceAgent\" => true,"), "{code}");
        assert!(!code.contains(".name"), "{code}");
    }

    #[test]
    fn known_verbs_module_shares_the_same_arms() {
        let fixture = ir(vec![(
            "Agent",
            action("deviceAgent", "account", Some("regular")),
        )]);
        let module = known_verbs_module(&fixture, "2.3000.1").expect("known verbs");
        assert!(module.contains("\"deviceAgent\" => true,"), "{module}");
        assert!(!module.contains("struct Schema"), "{module}");
        // "Schema" appears in the doc comment pointing at the registry;
        // what must not appear is a reference to a record.
        assert!(!module.contains(".name"), "{module}");
        assert!(!module.contains("ALL"), "{module}");
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

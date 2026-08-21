//! `plugins/wam-catalog/src/generated.rs` and `.../call_sites.rs`: the WAM
//! telemetry surface, as a typed Rust catalog.
//!
//! Unlike the other emitters this one binds the domain whole rather than a
//! chosen subset. The enum emitter picks entries because a catalog name there
//! cannot carry a stable Rust identity; WAM has no such problem, since every event,
//! enum and global is keyed by a unique module or name, so there is nothing to
//! curate and the value of the artifact is precisely that it is complete. What
//! a client emits is then a choice made in the plugin, against a catalog that
//! already knows every field's id and type.
//!
//! Two artifacts rather than one, because they have different lifetimes in a
//! build. `generated.rs` is the catalog a shipping client links; `call_sites.rs`
//! is evidence about WA Web used only to check an emitter against it, so it sits
//! behind the catalog's `parity` feature and never reaches a release binary.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, bail};

use crate::ir::{WamCallSite, WamEventDef, WamFieldWrite, WamIr, WamValueKind};
use crate::naming::{ensure_ident, rust_ident, rust_lit, snake_case, unique_type_ident};

/// A type or variant identifier for a bundle name.
///
/// Not [`crate::naming::pascal_case`], which preserves the case of a word it
/// does not split: WAM spells its enum members `REGULAR_MESSAGE`, and preserving
/// that gives `REGULARMESSAGE`, which is both unreadable and a
/// `non_camel_case_types` warning in a tree that denies them. Going through
/// snake case first lowercases every word, so the rule is one way for a member,
/// an event and an enum alike, and it round-trips the names that already are
/// UpperCamelCase.
fn type_ident(s: &str) -> String {
    let pascal: String = snake_case(s)
        .split('_')
        .filter(|w| !w.is_empty())
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

/// Every module defining a WAM enum carries this prefix, so the Rust type name
/// drops it: `WAWebWamEnumMessageType` is `MessageType`. Generation fails if a
/// module ever stops carrying it rather than emitting a name whose shape
/// silently changed.
const ENUM_MODULE_PREFIX: &str = "WAWebWamEnum";

/// The channels `defineEvents` and `defineGlobal` can name. An unknown one is a
/// protocol change the codec has no encoding for, so it fails generation.
const CHANNELS: &[(&str, &str)] = &[
    ("regular", "Regular"),
    ("realtime", "Realtime"),
    ("private", "Private"),
];

fn channel_variant(channel: &str) -> Result<&'static str> {
    CHANNELS
        .iter()
        .find(|(wire, _)| *wire == channel)
        .map(|(_, rust)| *rust)
        .ok_or_else(|| anyhow::anyhow!("unknown WAM channel {channel:?}"))
}

/// The generated catalog and the generated parity table, in that order.
#[derive(Debug)]
pub struct Artifacts {
    pub catalog: String,
    pub call_sites: String,
}

pub fn generate(ir: &WamIr, wa_version: &str) -> Result<Artifacts> {
    let enums = enum_names(ir)?;
    Ok(Artifacts {
        catalog: catalog(ir, wa_version, &enums)?,
        call_sites: call_sites(ir, wa_version),
    })
}

/// Module to Rust type name for every enum in the catalog.
fn enum_names(ir: &WamIr) -> Result<BTreeMap<String, String>> {
    let mut names = BTreeMap::new();
    let mut used = BTreeSet::new();
    for e in &ir.enums {
        let Some(stem) = e.module.strip_prefix(ENUM_MODULE_PREFIX) else {
            bail!(
                "WAM enum module {:?} does not start with {ENUM_MODULE_PREFIX}; the Rust name rule no longer holds",
                e.module
            );
        };
        let rust = type_ident(stem);
        if !used.insert(rust.clone()) {
            bail!("two WAM enum modules both map to the Rust type {rust:?}");
        }
        names.insert(e.module.clone(), rust);
    }
    Ok(names)
}

fn catalog(ir: &WamIr, wa_version: &str, enums: &BTreeMap<String, String>) -> Result<String> {
    let mut out = String::with_capacity(1 << 20);
    out.push_str(&format!(
        "//! Auto-generated WAM catalog (WhatsApp {wa_version}). DO NOT EDIT.\n\
         //!\n\
         //! Regenerate with `cargo run -p whatspec-codegen`. Every event, enum,\n\
         //! global and constant WA Web declares is here; which of them this client\n\
         //! may honestly emit is decided in the plugin, not in this file.\n\
         \n\
         #![allow(clippy::all)]\n\
         \n\
         use crate::PrivateStatsId;\n\
         \n"
    ));

    out.push_str(&constants(ir)?);
    out.push_str(&private_stats_ids(ir));
    out.push_str(&enum_module(ir, enums));
    out.push_str(&globals_module(ir, enums)?);
    out.push_str(&events_module(ir, enums)?);
    Ok(out)
}

fn constants(ir: &WamIr) -> Result<String> {
    let mut out = String::new();
    out.push_str(
        "/// The literals `WAWebWamConstants` exports: the buffer's protocol version\n\
         /// and its size and flush policy. They govern no event's schema.\n\
         pub mod constants {\n",
    );
    for c in &ir.constants {
        // The name is written out as an item name and inside a doc comment, so
        // anything that is not a plain constant identifier would emit Rust this
        // emitter did not intend. Failing is the only safe answer: the fix is a
        // naming rule here, not a file that happens to compile.
        if !is_screaming_snake(&c.name) {
            bail!(
                "WAM constant {:?} in {:?} is not a SCREAMING_SNAKE_CASE identifier, \
                 so it cannot be emitted as one",
                c.name,
                c.module,
            );
        }
        out.push_str(&format!(
            "    /// `{}::{}`.\n    pub const {}: i64 = {};\n",
            c.module, c.name, c.name, c.value
        ));
    }
    out.push_str("}\n\n");
    Ok(out)
}

/// Whether a bundle name is already a constant identifier: ASCII letters,
/// digits and underscores, not starting with a digit, with no lowercase.
fn is_screaming_snake(name: &str) -> bool {
    !name.is_empty()
        && !name.starts_with(|c: char| c.is_ascii_digit())
        && name
            .chars()
            .all(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || c == '_')
}

/// The Rust type for an enum module the catalog declares.
///
/// A miss is a bundle that names an enum module in a field or a global without
/// declaring it. Indexing would panic with the module name and no explanation;
/// this says which one and where it was expected.
fn enum_type<'a>(enums: &'a BTreeMap<String, String>, module: &str) -> Result<&'a str> {
    match enums.get(module) {
        Some(rust) => Ok(rust.as_str()),
        None => bail!("WAM uses the enum module {module:?}, which the catalog does not declare"),
    }
}

fn private_stats_ids(ir: &WamIr) -> String {
    let mut out = String::new();
    out.push_str(
        "/// The private-stats rotation groups, sorted by id.\n\
         ///\n\
         /// A `private` buffer writes the anonymous id of the group its event\n\
         /// names. Listed here because the catalog is complete; nothing in this\n\
         /// repository emits on that channel yet.\n\
         pub const PRIVATE_STATS_IDS: &[PrivateStatsId] = &[\n",
    );
    for p in &ir.private_stats_ids {
        out.push_str(&format!(
            "    PrivateStatsId {{ key: {}, id: {}, rotation_period_days: {} }},\n",
            rust_lit(&p.key),
            p.id,
            p.rotation_period_days
        ));
    }
    out.push_str("];\n\n");
    out
}

fn enum_module(ir: &WamIr, names: &BTreeMap<String, String>) -> String {
    let mut out = String::new();
    out.push_str(
        "/// Every enum a WAM field or global is typed by.\n\
         ///\n\
         /// Each is closed: WA Web validates a field against the frozen member set\n\
         /// and throws otherwise, so a value outside it is not representable here\n\
         /// either. `wire()` is the integer that reaches the buffer.\n\
         pub mod enums {\n",
    );
    for e in &ir.enums {
        let rust = &names[&e.module];
        out.push_str(&format!(
            "    /// `{}` (`{}`).\n    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]\n    pub enum {rust} {{\n",
            e.name, e.module
        ));
        let mut used = std::collections::HashSet::new();
        let mut variants = Vec::with_capacity(e.variants.len());
        for v in &e.variants {
            let ident = unique_type_ident(&type_ident(&v.key), &mut used, "Variant");
            out.push_str(&format!(
                "        /// `{}` = {}.\n        {ident},\n",
                v.key, v.value
            ));
            variants.push((ident, v.value));
        }
        out.push_str("    }\n\n");
        out.push_str(&format!(
            "    impl {rust} {{\n        /// The integer WA Web writes for this member.\n        pub const fn wire(self) -> i64 {{\n            match self {{\n"
        ));
        for (ident, value) in &variants {
            out.push_str(&format!("                Self::{ident} => {value},\n"));
        }
        out.push_str("            }\n        }\n    }\n\n");
    }
    out.push_str("}\n\n");
    out
}

fn kind_expr(kind: &WamValueKind) -> &'static str {
    match kind {
        WamValueKind::Boolean => "ValueKind::Boolean",
        WamValueKind::Integer => "ValueKind::Integer",
        WamValueKind::Number => "ValueKind::Number",
        WamValueKind::String => "ValueKind::String",
        WamValueKind::Timer => "ValueKind::Timer",
        WamValueKind::Enum { .. } => "ValueKind::Enum",
    }
}

fn channel_set(channels: &[String]) -> Result<String> {
    let mut variants = Vec::with_capacity(channels.len());
    for c in channels {
        variants.push(format!("Channel::{}", channel_variant(c)?));
    }
    Ok(format!("ChannelSet::of(&[{}])", variants.join(", ")))
}

fn globals_module(ir: &WamIr, enums: &BTreeMap<String, String>) -> Result<String> {
    let mut out = String::new();
    out.push_str(
        "/// The buffer-level globals, each with the channels on which writing it\n\
         /// is legal.\n\
         ///\n\
         /// The channel list is the half an event field does not have: WA Web's\n\
         /// writer skips a global whose list does not contain the buffer's channel,\n\
         /// so a buffer carrying one anyway is a buffer no client sends.\n\
         pub mod globals {\n\
         \x20   use crate::{Channel, ChannelSet, GlobalDef, ValueKind};\n\n",
    );
    for g in &ir.globals {
        let doc_type = match &g.kind {
            WamValueKind::Enum { module } => format!("enum `{}`", enum_type(enums, module)?),
            other => kind_expr(other)
                .trim_start_matches("ValueKind::")
                .to_lowercase(),
        };
        out.push_str(&format!(
            "    /// `{}` (id {}, {}).\n    pub const {}: GlobalDef = GlobalDef {{\n        name: {},\n        id: {},\n        kind: {},\n        channels: {},\n    }};\n\n",
            g.name,
            g.id,
            doc_type,
            const_name(&g.name),
            rust_lit(&g.name),
            g.id,
            kind_expr(&g.kind),
            channel_set(&g.channels)?,
        ));
    }
    out.push_str("    /// Every global, sorted by name.\n    pub const ALL: &[GlobalDef] = &[\n");
    for g in &ir.globals {
        out.push_str(&format!("        {},\n", const_name(&g.name)));
    }
    out.push_str("    ];\n}\n\n");
    Ok(out)
}

/// A `SCREAMING_SNAKE_CASE` constant name for a camelCase bundle key.
fn const_name(name: &str) -> String {
    snake_case(name).to_uppercase()
}

fn events_module(ir: &WamIr, enums: &BTreeMap<String, String>) -> Result<String> {
    let mut out = String::new();
    out.push_str(
        "/// One struct per WAM event, with one `Option` field per declared field.\n\
         ///\n\
         /// Absence is the only way to say \"this client does not know\": there is no\n\
         /// zero or empty-string sentinel, because the buffer distinguishes a field\n\
         /// that was written from one that was not and so does the server.\n\
         pub mod events {\n\
         \x20   use super::enums;\n\
         \x20   use crate::{Channel, EventFields, WamEvent};\n\n",
    );
    for e in &ir.events {
        out.push_str(&event_struct(e, enums)?);
    }
    out.push_str("}\n");
    Ok(out)
}

fn event_struct(e: &WamEventDef, enums: &BTreeMap<String, String>) -> Result<String> {
    let rust = type_ident(&e.name);
    let channel = channel_variant(&e.channel)?;
    let mut out = String::new();
    out.push_str(&format!(
        "    /// `{}`: WAM event {} on the `{}` channel.\n    ///\n    /// Declared in `{}`. Sampling weights `{:?}` as the catalog lists them;\n    /// the weight a buffer carries can be overridden at runtime.\n    #[derive(Debug, Clone, Default, PartialEq)]\n    pub struct {rust} {{\n",
        e.name, e.code, e.channel, e.module, e.weights
    ));
    let mut fields = Vec::with_capacity(e.fields.len());
    for f in &e.fields {
        // The buffer format spells a wide field id in two bytes, so an id past
        // that is not encodable at all. Emitting it would produce a catalog
        // whose events silently go out under a truncated id.
        if f.id > u32::from(u16::MAX) {
            bail!(
                "WAM field {:?} on event {:?} has id {}, which does not fit the \
                 buffer format's two-byte field id",
                f.name,
                e.name,
                f.id,
            );
        }
        let ident = rust_ident(&f.name);
        let ty = match &f.kind {
            WamValueKind::Boolean => "bool".to_string(),
            WamValueKind::Integer | WamValueKind::Timer => "i64".to_string(),
            WamValueKind::Number => "f64".to_string(),
            WamValueKind::String => "String".to_string(),
            WamValueKind::Enum { module } => format!("enums::{}", enum_type(enums, module)?),
        };
        let note = match &f.kind {
            WamValueKind::Timer => " A duration in milliseconds.",
            _ => "",
        };
        out.push_str(&format!(
            "        /// `{}` (id {}).{note}\n        pub {ident}: Option<{ty}>,\n",
            f.name, f.id
        ));
        fields.push((ident, f.id, f.kind.clone()));
    }
    out.push_str("    }\n\n");

    // Three, one per sampling cohort, and the catalog's own `WEIGHTS: [u32; 3]`
    // depends on it. A bundle that starts declaring a different number has
    // changed the sampling model, which is a thing to read before regenerating.
    let [alpha, beta, release] = e.weights[..] else {
        bail!(
            "WAM event {:?} declares {} sampling weights, not the three the cohorts need",
            e.name,
            e.weights.len(),
        );
    };
    out.push_str(&format!(
        "    impl WamEvent for {rust} {{\n        const NAME: &'static str = {};\n        const CODE: u32 = {};\n        const CHANNEL: Channel = Channel::{channel};\n        const WEIGHTS: [u32; 3] = [{}, {}, {}];\n        const PRIVATE_STATS_ID: Option<i64> = {};\n\n        fn encode(&self, fields: &mut EventFields<'_>) {{\n",
        rust_lit(&e.name),
        e.code,
        alpha,
        beta,
        release,
        match e.private_stats_id {
            Some(id) => format!("Some({id})"),
            None => "None".to_string(),
        },
    ));
    if fields.is_empty() {
        out.push_str("            let _ = fields;\n");
    }
    for (ident, id, kind) in &fields {
        let call = match kind {
            WamValueKind::Boolean => format!("fields.boolean({id}, self.{ident});"),
            WamValueKind::Integer | WamValueKind::Timer => {
                format!("fields.integer({id}, self.{ident});")
            }
            WamValueKind::Number => format!("fields.number({id}, self.{ident});"),
            WamValueKind::String => format!("fields.string({id}, self.{ident}.as_deref());"),
            WamValueKind::Enum { .. } => {
                format!("fields.integer({id}, self.{ident}.map(|v| v.wire()));")
            }
        };
        out.push_str(&format!("            {call}\n"));
    }
    out.push_str("        }\n    }\n\n");
    Ok(out)
}

fn call_sites(ir: &WamIr, wa_version: &str) -> String {
    let mut out = String::with_capacity(1 << 18);
    out.push_str(&format!(
        "//! Auto-generated WAM call-site evidence (WhatsApp {wa_version}). DO NOT EDIT.\n\
         //!\n\
         //! Where WA Web constructs each event and which fields it is seen writing\n\
         //! there. This is *where* and *with which fields*, never *when*: the guard a\n\
         //! site sits under is control flow the extractor does not read, so a site is\n\
         //! a place the official client can write a field, not a promise that it does.\n\
         //!\n\
         //! Behind the `parity` feature because it is evidence about WA Web, not part\n\
         //! of the catalog a client links. Its consumer is the conformance test that\n\
         //! checks every field this repository writes against it.\n\
         \n\
         use crate::{{CallSite, CallSiteField, CatalogField, EventCallSites, FieldWrite}};\n\n"
    ));
    out.push_str(
        "/// Every event's declared fields and recovered call sites, sorted by\n\
         /// event name so a lookup is a binary search.\n\
         pub const CALL_SITES: &[EventCallSites] = &[\n",
    );
    let mut by_name: Vec<&WamEventDef> = ir.events.iter().collect();
    by_name.sort_by(|a, b| a.name.cmp(&b.name));
    for e in by_name {
        out.push_str(&format!(
            "    EventCallSites {{\n        event: {},\n        code: {},\n        fields: &[",
            rust_lit(&e.name),
            e.code
        ));
        for f in &e.fields {
            out.push_str(&format!(
                "CatalogField {{ name: {}, id: {} }}, ",
                rust_lit(&f.name),
                f.id
            ));
        }
        out.push_str("],\n        sites: &[\n");
        for s in &e.call_sites {
            out.push_str(&call_site(s));
        }
        out.push_str("        ],\n    },\n");
    }
    out.push_str("];\n");
    out
}

fn call_site(s: &WamCallSite) -> String {
    let mut out = String::new();
    out.push_str(&format!(
        "            CallSite {{ module: {}, partial: {}, fields: &[",
        rust_lit(&s.module),
        s.partial
    ));
    for f in &s.fields {
        let write = match f.write {
            WamFieldWrite::Constructor => "FieldWrite::Constructor",
            WamFieldWrite::Assigned => "FieldWrite::Assigned",
        };
        out.push_str(&format!(
            "CallSiteField {{ name: {}, write: {write} }}, ",
            rust_lit(&f.name)
        ));
    }
    out.push_str("] },\n");
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ir() -> WamIr {
        serde_json::from_str(
            r#"{
              "constants": [{"name":"WAM_PROTOCOL_VERSION","value":5,"module":"WAWebWamConstants"}],
              "privateStatsIds": [{"key":"none","id":0,"rotationPeriodDays":-1}],
              "enums": [{"name":"MESSAGE_TYPE","module":"WAWebWamEnumMessageType",
                         "variants":[{"key":"TEXT","value":1},{"key":"IMAGE","value":2}]}],
              "globals": [{"name":"appVersion","id":17,"kind":"string","channels":["regular","private"]}],
              "events": [{"name":"UnknownStanza","code":3448,"module":"WAWebUnknownStanzaWamEvent",
                          "channel":"regular","weights":[1,1,20],
                          "fields":[{"name":"unknownStanzaTag","id":1,"kind":"string"},
                                    {"name":"messageType","id":2,"kind":"enum","module":"WAWebWamEnumMessageType"},
                                    {"name":"someT","id":3,"kind":"timer"}],
                          "callSites":[{"module":"WAWebPostUnknownStanzaMetric",
                                        "fields":[{"name":"unknownStanzaTag","write":"constructor"}]}]}]
            }"#,
        )
        .expect("fixture IR")
    }

    #[test]
    fn the_catalog_carries_ids_types_and_channels() {
        let out = generate(&ir(), "2.3000.1").expect("generate");
        assert!(
            out.catalog
                .contains("pub const WAM_PROTOCOL_VERSION: i64 = 5;")
        );
        assert!(out.catalog.contains("pub enum MessageType"));
        assert!(out.catalog.contains("Self::Text => 1,"));
        assert!(
            out.catalog
                .contains("channels: ChannelSet::of(&[Channel::Regular, Channel::Private])")
        );
        assert!(out.catalog.contains("pub struct UnknownStanza"));
        assert!(
            out.catalog
                .contains("pub unknown_stanza_tag: Option<String>,")
        );
        assert!(
            out.catalog
                .contains("pub message_type: Option<enums::MessageType>,")
        );
        // A timer is a millisecond duration written as an integer, so it shares
        // the integer encoder rather than getting one of its own.
        assert!(out.catalog.contains("fields.integer(3, self.some_t);"));
        assert!(out.catalog.contains("const CODE: u32 = 3448;"));
        assert!(
            out.catalog
                .contains("const WEIGHTS: [u32; 3] = [1, 1, 20];")
        );
    }

    #[test]
    fn the_parity_table_keeps_each_site_module_and_partiality() {
        let out = generate(&ir(), "2.3000.1").expect("generate");
        assert!(out.call_sites.contains("event: \"UnknownStanza\""));
        assert!(
            out.call_sites
                .contains("module: \"WAWebPostUnknownStanzaMetric\", partial: false")
        );
        assert!(
            out.call_sites
                .contains("name: \"unknownStanzaTag\", write: FieldWrite::Constructor")
        );
        // The declared fields travel with the sites, so a parity check can name
        // the ids it decodes out of a buffer.
        assert!(
            out.call_sites
                .contains("CatalogField { name: \"unknownStanzaTag\", id: 1 }")
        );
    }

    #[test]
    fn an_enum_module_without_the_catalog_prefix_fails_generation() {
        let mut ir = ir();
        ir.enums[0].module = "SomeOtherModule".to_string();
        let err = generate(&ir, "2.3000.1").expect_err("the naming rule must hold");
        assert!(err.to_string().contains("WAWebWamEnum"), "{err}");
    }

    #[test]
    fn an_unknown_channel_fails_generation() {
        let mut ir = ir();
        ir.events[0].channel = "quantum".to_string();
        let err = generate(&ir, "2.3000.1").expect_err("an unencodable channel must fail");
        assert!(err.to_string().contains("unknown WAM channel"), "{err}");
    }

    #[test]
    fn an_enum_module_no_enum_declares_fails_generation() {
        let mut ir = ir();
        ir.events[0].fields[1].kind = WamValueKind::Enum {
            module: "WAWebWamEnumNobodyDeclares".to_string(),
        };
        let err = generate(&ir, "2.3000.1").expect_err("an undeclared enum module must fail");
        assert!(
            err.to_string().contains("WAWebWamEnumNobodyDeclares"),
            "{err}"
        );
    }

    #[test]
    fn a_weight_list_that_is_not_three_long_fails_generation() {
        let mut ir = ir();
        ir.events[0].weights = vec![1, 20];
        let err = generate(&ir, "2.3000.1").expect_err("the three cohorts must hold");
        assert!(err.to_string().contains("sampling weights"), "{err}");
    }

    #[test]
    fn a_field_id_past_the_wire_format_fails_generation() {
        let mut ir = ir();
        ir.events[0].fields[0].id = 70_000;
        let err = generate(&ir, "2.3000.1").expect_err("an unencodable field id must fail");
        assert!(err.to_string().contains("two-byte field id"), "{err}");
    }

    #[test]
    fn a_constant_name_that_is_not_an_identifier_fails_generation() {
        let mut ir = ir();
        ir.constants[0].name = "WAM VERSION: i64 = 0; //".to_string();
        let err =
            generate(&ir, "2.3000.1").expect_err("a name that is not an identifier must fail");
        assert!(err.to_string().contains("SCREAMING_SNAKE_CASE"), "{err}");
    }
}

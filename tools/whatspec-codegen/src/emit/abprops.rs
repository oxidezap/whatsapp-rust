//! `wacore/src/iq/abprops.rs`: the A/B-props (feature-flag) registry.

use std::collections::HashSet;

use anyhow::{Result, ensure};

use crate::ir::{AbPropConfig, AbPropType, AbPropsIr, Scalar};
use crate::naming::{float_lit, rust_lit, snake_case, unique_ident};

const HEADER: &str = "\
//! One `pub mod` per WA Web registry, one `pub const` per flag (screaming-snake of
//! its key) with the numeric `code` sent in the `<props>` IQ, value type, and default;
//! reference only what you use. Each module's `ALL` lists its flags; the top-level
//! `ALL` lists every module's slice.

#![allow(clippy::all)]

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AbPropType {
    Bool,
    Int,
    Float,
    Str,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AbDefault {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(&'static str),
}

#[derive(Debug, Clone, Copy)]
pub struct AbProp {
    pub name: &'static str,
    pub code: u32,
    pub value_type: AbPropType,
    pub default: AbDefault,
}

";

pub fn generate(ir: &AbPropsIr) -> Result<String> {
    let mut out = super::header("A/B-props registry", &ir.wa_version);
    out.push_str(HEADER);

    // `configs` arrives sorted by (module, name); one Rust module per run.
    let mut module_idents = HashSet::new();
    let mut emitted: Vec<String> = Vec::new();
    // Interleaved modules would visit the same registry twice and split it
    // across `web` and `web_2` — a silent API rename that `--check` would then
    // bless. Fail on the IR instead.
    let mut seen_modules: HashSet<&str> = HashSet::new();
    let mut i = 0;
    while i < ir.configs.len() {
        let wa_module = ir.configs[i].module.as_str();
        ensure!(
            seen_modules.insert(wa_module),
            "abprops IR is not grouped by module: {wa_module} appears in two runs"
        );
        let mut j = i;
        while j < ir.configs.len() && ir.configs[j].module == wa_module {
            j += 1;
        }
        let ident = unique_ident(&module_ident(wa_module), &mut module_idents, "m");
        out.push_str(&render_module(&ident, wa_module, &ir.configs[i..j]));
        emitted.push(ident);
        i = j;
    }

    out.push_str(
        "/// Every registry's `ALL`, for whole-catalog iteration:\n\
         /// `ALL.iter().flat_map(|r| r.iter())`.\npub const ALL: &[&[AbProp]] = &[\n",
    );
    for m in &emitted {
        out.push_str(&format!("    {m}::ALL,\n"));
    }
    out.push_str("];\n");
    Ok(out)
}

fn render_module(ident: &str, wa_module: &str, group: &[AbPropConfig]) -> String {
    let mut out = format!(
        "/// `{wa_module}` — {} flags.\npub mod {ident} {{\n    use super::{{AbDefault, AbProp, AbPropType}};\n\n",
        group.len()
    );
    // Ident scope is per module, so a flag carried by two registries keeps its
    // name in each.
    let mut used = HashSet::new();
    let names: Vec<String> = group
        .iter()
        .map(|c| unique_ident(&snake_case(&c.name).to_uppercase(), &mut used, "F"))
        .collect();
    for (c, const_name) in group.iter().zip(&names) {
        out.push_str(&format!(
            "    pub const {const_name}: AbProp = AbProp {{ name: {}, code: {}, value_type: AbPropType::{}, default: {} }};\n",
            rust_lit(&c.name),
            c.code,
            type_variant(c.value_type),
            default_lit(&c.default),
        ));
    }
    out.push_str(&format!(
        "\n    /// All {} flags in this registry, sorted by name.\n    pub const ALL: &[AbProp] = &[\n",
        group.len()
    ));
    for const_name in &names {
        out.push_str(&format!("        {const_name},\n"));
    }
    out.push_str("    ];\n}\n\n");
    out
}

/// `WAWebHybridABPropsConfigs` into `hybrid`; the bare registry into `web`.
fn module_ident(wa_module: &str) -> String {
    let s = wa_module.strip_prefix("WAWeb").unwrap_or(wa_module);
    let s = s.strip_suffix("ABPropsConfigs").unwrap_or(s);
    if s.is_empty() {
        "web".to_string()
    } else {
        snake_case(s)
    }
}

fn type_variant(t: AbPropType) -> &'static str {
    match t {
        AbPropType::Bool => "Bool",
        AbPropType::Int => "Int",
        AbPropType::String => "Str",
        AbPropType::Float => "Float",
    }
}

fn default_lit(s: &Scalar) -> String {
    match s {
        Scalar::Bool(b) => format!("AbDefault::Bool({b})"),
        Scalar::Int(i) => format!("AbDefault::Int({i})"),
        Scalar::Float(f) => format!("AbDefault::Float({})", float_lit(*f)),
        Scalar::Str(v) => format!("AbDefault::Str({})", rust_lit(v)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The emitters are fallible now; a test IR that trips a guard should
    /// fail the test, not be silently skipped.
    fn emitted(ir: &AbPropsIr) -> String {
        generate(ir).expect("emitter rejected the test IR")
    }

    fn cfg(module: &str, name: &str, code: u32, ty: AbPropType, default: Scalar) -> AbPropConfig {
        AbPropConfig {
            module: module.into(),
            name: name.into(),
            code,
            value_type: ty,
            default,
        }
    }

    fn ir(configs: Vec<AbPropConfig>) -> AbPropsIr {
        AbPropsIr {
            wa_version: "2.3000.1".into(),
            configs,
        }
    }

    #[test]
    fn emits_one_tree_shakeable_const_per_flag() {
        let code = emitted(&ir(vec![
            cfg(
                "WAWebABPropsConfigs",
                "flag_b",
                10,
                AbPropType::Bool,
                Scalar::Bool(false),
            ),
            cfg(
                "WAWebABPropsConfigs",
                "flag_f",
                11,
                AbPropType::Float,
                Scalar::Float(0.5),
            ),
        ]));
        assert!(code.contains("(WhatsApp 2.3000.1). DO NOT EDIT."));
        assert!(code.contains("pub mod web {"));
        assert!(code.contains(
            "pub const FLAG_B: AbProp = AbProp { name: \"flag_b\", code: 10, value_type: AbPropType::Bool, default: AbDefault::Bool(false) };"
        ));
        assert!(code.contains("default: AbDefault::Float(0.5) }"));
        assert!(code.contains("pub const ALL: &[&[AbProp]] = &[\n    web::ALL,\n];"));
    }

    #[test]
    fn keeps_registries_apart_and_disambiguates_names() {
        let code = emitted(&ir(vec![
            cfg(
                "WAWebABPropsConfigs",
                "shared",
                1,
                AbPropType::Bool,
                Scalar::Bool(false),
            ),
            cfg(
                "WAWebABPropsConfigs",
                "SHARED",
                2,
                AbPropType::Bool,
                Scalar::Bool(true),
            ),
            cfg(
                "WAWebHybridABPropsConfigs",
                "shared",
                1,
                AbPropType::Bool,
                Scalar::Bool(false),
            ),
        ]));
        assert!(code.contains("pub mod web {"));
        assert!(code.contains("pub mod hybrid {"));
        // Same key in two registries keeps the same const name in each; a
        // within-registry collision gets a suffix instead of a duplicate item.
        assert_eq!(code.matches("pub const SHARED: AbProp").count(), 2);
        assert_eq!(code.matches("pub const SHARED_2: AbProp").count(), 1);
    }

    #[test]
    fn an_ir_not_grouped_by_module_is_rejected() {
        // The run-grouping loop would visit the same registry twice and split
        // it across `web` and `web_2`, renaming half the flags in silence.
        let err = generate(&ir(vec![
            cfg(
                "WAWebABPropsConfigs",
                "a",
                1,
                AbPropType::Bool,
                Scalar::Bool(false),
            ),
            cfg(
                "WAWebHybridABPropsConfigs",
                "b",
                2,
                AbPropType::Bool,
                Scalar::Bool(false),
            ),
            cfg(
                "WAWebABPropsConfigs",
                "c",
                3,
                AbPropType::Bool,
                Scalar::Bool(false),
            ),
        ]))
        .expect_err("interleaved modules must not be emitted");
        assert!(err.to_string().contains("two runs"), "{err}");
    }

    #[test]
    fn a_non_finite_float_default_still_renders_a_literal() {
        let code = emitted(&ir(vec![cfg(
            "WAWebABPropsConfigs",
            "nanflag",
            1,
            AbPropType::Float,
            Scalar::Float(f64::NAN),
        )]));
        assert!(code.contains("AbDefault::Float(f64::NAN)"), "{code}");
        assert!(!code.contains("Float(NaN)"));
    }

    #[test]
    fn a_default_holding_quotes_stays_a_valid_literal() {
        let code = emitted(&ir(vec![cfg(
            "WAWebABPropsConfigs",
            "quoted",
            1,
            AbPropType::String,
            Scalar::Str("a\"b\\c".into()),
        )]));
        assert!(code.contains(r#"AbDefault::Str("a\"b\\c")"#), "{code}");
    }
}

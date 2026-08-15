//! `wacore/src/types/wire_enums.rs`: the protocol enums this repository binds
//! to Rust types, with their variants taken from the whatspec enum catalog.
//!
//! The catalog carries 403 entries and this emits a handful, which is
//! deliberate. 88 of them have a `syntheticName`, a placeholder whatspec builds
//! by concatenating the variant values, so adding one variant upstream renames
//! the entry; names also repeat across modules (`ACK`, `ENUM_LID_PN`,
//! `EventType`), and 17 are proto-nested names `waproto` already generates from
//! the `.proto`. None of those can carry a stable Rust type identity. So the
//! split is: [`WANTED`] binds a name, a shape and any variant spellings, and
//! the IR owns everything that can drift -- which variants exist, what they
//! carry, and whether integers are bit positions. A variant added upstream
//! lands here on the next sync, and `--check` fails if the tree disagrees.

use std::collections::BTreeSet;

use anyhow::{Context, Result, bail, ensure};

use crate::ir::{EnumDef, EnumValueKind, EnumsIr, Scalar};
use crate::naming::pascal_case;

const HEADER: &str = "\
//! Protocol enums generated from the whatspec enum catalog.
//!
//! Regenerate with `cargo run -p whatspec-codegen`; never edit by hand. To bind
//! another catalog entry, add it to `WANTED` in the codegen's enum emitter --
//! the variants come from the bundle, not from this file.

#![allow(clippy::all)]

";

/// How a catalog entry is bound to Rust.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// A `WireEnum` over the variant values, closed: a wire value outside the
    /// set is not representable. Mirrors an `attrEnumOrNullIfUnknown` field,
    /// where the official parser nulls what it does not recognize.
    Closed,
    /// The same, plus a `#[wire_fallback] Unknown(String)` arm keeping the wire
    /// bytes of a value this build does not model.
    Open,
    /// Integer variants emitted as `pub const` masks named `<rust>_<VARIANT>`.
    /// `bitPosition` entries are shifted here so a caller never repeats the
    /// shift.
    Masks,
}

/// One catalog entry this repository binds, keyed the way the catalog is:
/// module first, because the name alone is not unique.
pub struct Wanted {
    pub module: &'static str,
    pub name: &'static str,
    /// The Rust type name, or for [`Shape::Masks`] the prefix its constants
    /// share.
    pub rust: &'static str,
    pub shape: Shape,
    /// Variant spellings the mechanical `pascal_case` gets wrong, as
    /// `(wire value, Rust identifier)`. `medianotify` is one word on the wire
    /// and two in English, and nothing in the bundle says so.
    pub renames: &'static [(&'static str, &'static str)],
    pub doc: &'static str,
}

pub const WANTED: &[Wanted] = &[
    Wanted {
        module: "WAWebHandleMsgCommon",
        name: "STANZA_MSG_TYPES",
        rust: "StanzaMessageType",
        shape: Shape::Open,
        renames: &[("medianotify", "MediaNotify")],
        doc: "The `type` attribute of an incoming `<message>` envelope.\n\
              ///\n\
              /// The official parser rejects a stanza whose `type` is absent or\n\
              /// outside this set (`unknownValue: \"reject\"`). This client keeps\n\
              /// the stanza instead, so the fallback arm holds the exact wire\n\
              /// bytes of a value it does not model.",
    },
    Wanted {
        module: "WAWebHandleMsgCommon",
        name: "POLL_TYPES",
        rust: "PollType",
        shape: Shape::Closed,
        renames: &[],
        doc: "The `polltype` attribute of an incoming `<message><meta>` node.\n\
              ///\n\
              /// Closed on purpose: the attribute is `attrEnumOrNullIfUnknown`\n\
              /// upstream (`unknownValue: \"null\"`), so a value outside this set\n\
              /// is dropped rather than preserved.",
    },
    Wanted {
        module: "WAWebBackendJobs.flow",
        name: "EncMediaType",
        rust: "EncMediaType",
        shape: Shape::Open,
        renames: &[("livelocation", "LiveLocation")],
        doc: "The `mediatype` attribute of an `<enc>` node.\n\
              ///\n\
              /// A hint about the payload the ciphertext carries, available\n\
              /// before the decryption that would reveal it. It is the sender's\n\
              /// claim and nothing checks it against the decrypted message.",
    },
    Wanted {
        module: "WAWebSendReceiptJobCommon",
        name: "ReceiptModeBitPosition",
        rust: "RECEIPT_MODE",
        shape: Shape::Masks,
        renames: &[],
        doc: "Bits of a receipt's `<meta mode>` bitmask.",
    },
];

pub fn generate(ir: &EnumsIr) -> Result<String> {
    let mut out = super::header("protocol enums", &ir.wa_version);
    out.push_str(HEADER);

    for wanted in WANTED {
        let def = lookup(ir, wanted)?;
        match wanted.shape {
            Shape::Closed | Shape::Open => out.push_str(&wire_enum(wanted, def)?),
            Shape::Masks => out.push_str(&masks(wanted, def)?),
        }
    }
    Ok(out)
}

/// The one entry matching `(module, name)`. Ambiguity is fatal rather than
/// first-wins: the catalog does repeat names, and silently binding the wrong
/// module's enum is the failure this key exists to prevent.
fn lookup<'a>(ir: &'a EnumsIr, wanted: &Wanted) -> Result<&'a EnumDef> {
    let matches: Vec<&EnumDef> = ir
        .enums
        .iter()
        .filter(|e| e.name == wanted.name && e.module == wanted.module)
        .collect();
    match matches.as_slice() {
        [one] => {
            ensure!(
                one.synthetic_name != Some(true),
                "{}::{} has a synthetic name, which cannot be a stable Rust type identity",
                wanted.module,
                wanted.name
            );
            Ok(one)
        }
        [] => bail!(
            "the enum catalog has no {}::{}; it was renamed or dropped upstream",
            wanted.module,
            wanted.name
        ),
        many => bail!(
            "the enum catalog has {} entries for {}::{}",
            many.len(),
            wanted.module,
            wanted.name
        ),
    }
}

fn variant_ident(wanted: &Wanted, wire: &str) -> String {
    for (value, rust) in wanted.renames {
        if *value == wire {
            return (*rust).to_string();
        }
    }
    pascal_case(wire)
}

fn wire_enum(wanted: &Wanted, def: &EnumDef) -> Result<String> {
    ensure!(
        def.value_kind == EnumValueKind::String,
        "{}::{} carries integers, so it cannot be a unit-string WireEnum",
        wanted.module,
        wanted.name
    );

    let copy = if wanted.shape == Shape::Closed {
        ", Copy"
    } else {
        ""
    };
    let mut out = format!(
        "/// {}\n///\n/// Generated from `{}` in `{}`.\n#[derive(Debug, Clone{copy}, PartialEq, Eq, crate::WireEnum)]\npub enum {} {{\n",
        wanted.doc, wanted.name, def.module, wanted.rust
    );

    let mut used = BTreeSet::new();
    for variant in &def.variants {
        let Scalar::Str(wire) = &variant.value else {
            bail!(
                "{}::{} variant {} does not carry a string",
                wanted.module,
                wanted.name,
                variant.name
            );
        };
        let ident = variant_ident(wanted, wire);
        ensure!(
            used.insert(ident.clone()),
            "{}::{} maps two wire values onto the Rust variant {ident}",
            wanted.module,
            wanted.name
        );
        out.push_str(&format!("    #[wire = {}]\n    {ident},\n", rust_str(wire)));
    }

    if wanted.shape == Shape::Open {
        out.push_str(
            "    /// A value this build does not model, kept verbatim.\n    #[wire_fallback]\n    Unknown(String),\n",
        );
    }
    out.push_str("}\n\n");
    Ok(out)
}

fn masks(wanted: &Wanted, def: &EnumDef) -> Result<String> {
    ensure!(
        def.value_kind == EnumValueKind::Int,
        "{}::{} carries strings, so it cannot be emitted as masks",
        wanted.module,
        wanted.name
    );
    let shifted = def.bit_position == Some(true);

    let mut out = format!(
        "// {} `{}` in `{}`. {}\n",
        wanted.doc,
        wanted.name,
        def.module,
        if shifted {
            "The catalog stores bit positions; these are already shifted."
        } else {
            "The catalog stores the values themselves."
        }
    );
    for variant in &def.variants {
        let Scalar::Int(value) = &variant.value else {
            bail!(
                "{}::{} variant {} does not carry an integer",
                wanted.module,
                wanted.name,
                variant.name
            );
        };
        // Checked here rather than left to the generated file: `1 << 32` trips
        // rustc's overflow lint inside `wire_enums.rs`, which sends whoever hits
        // it to the artifact instead of to the catalog entry that caused it.
        let expr = if shifted {
            ensure!(
                (0..u32::BITS as i64).contains(value),
                "{}::{} variant {} is bit position {value}, which does not fit a u32 mask",
                wanted.module,
                wanted.name,
                variant.name
            );
            format!("1 << {value}")
        } else {
            let value = u32::try_from(*value).with_context(|| {
                format!(
                    "{}::{} variant {} carries {value}, which is not a u32 mask",
                    wanted.module, wanted.name, variant.name
                )
            })?;
            value.to_string()
        };
        out.push_str(&format!(
            "/// `{}` of `{}`.\npub const {}_{}: u32 = {expr};\n",
            variant.name, wanted.name, wanted.rust, variant.name
        ));
    }
    out.push('\n');
    Ok(out)
}

/// A Rust string literal for a wire value. Wire values are ASCII identifiers in
/// practice, so this only has to survive a quote or a backslash appearing.
fn rust_str(s: &str) -> String {
    format!("{s:?}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::EnumVariant;

    /// Entries are looked up by the identity they already carry. Indexing
    /// `WANTED` by position would silently re-aim a test at another entry the
    /// moment the table is reordered or grown at the top.
    fn wanted(name: &str) -> &'static Wanted {
        WANTED
            .iter()
            .find(|w| w.name == name)
            .expect("WANTED holds the entry this test names")
    }

    fn def(name: &str, module: &str, values: &[&str]) -> EnumDef {
        EnumDef {
            name: name.to_string(),
            module: module.to_string(),
            value_kind: EnumValueKind::String,
            variants: values
                .iter()
                .map(|v| EnumVariant {
                    name: (*v).to_string(),
                    value: Scalar::Str((*v).to_string()),
                })
                .collect(),
            synthetic_name: None,
            bit_position: None,
        }
    }

    #[test]
    fn an_open_enum_gets_a_fallback_and_a_closed_one_does_not() {
        let open = wanted("STANZA_MSG_TYPES");
        let out = wire_enum(
            open,
            &def("STANZA_MSG_TYPES", "m", &["text", "medianotify"]),
        )
        .expect("emit");
        assert!(out.contains("#[wire = \"medianotify\"]\n    MediaNotify,"));
        assert!(out.contains("#[wire_fallback]"));
        assert!(!out.contains(", Copy,"));

        let closed =
            wire_enum(wanted("POLL_TYPES"), &def("POLL_TYPES", "m", &["vote"])).expect("emit");
        assert!(!closed.contains("#[wire_fallback]"));
        assert!(closed.contains(", Copy,"));
    }

    /// The catalog repeats names across modules, so binding by name alone would
    /// pick whichever entry came first.
    #[test]
    fn lookup_is_keyed_by_module_and_rejects_a_missing_entry() {
        let ir = EnumsIr {
            wa_version: "2.0.0".to_string(),
            enums: vec![def("POLL_TYPES", "SomeOtherModule", &["vote"])],
        };
        let err = lookup(&ir, wanted("POLL_TYPES")).expect_err("the module does not match");
        assert!(
            err.to_string()
                .contains("has no WAWebHandleMsgCommon::POLL_TYPES")
        );
    }

    #[test]
    fn a_synthetic_name_is_refused() {
        let mut entry = def("POLL_TYPES", "WAWebHandleMsgCommon", &["vote"]);
        entry.synthetic_name = Some(true);
        let ir = EnumsIr {
            wa_version: "2.0.0".to_string(),
            enums: vec![entry],
        };
        let err =
            lookup(&ir, wanted("POLL_TYPES")).expect_err("synthetic names are not identities");
        assert!(err.to_string().contains("synthetic name"));
    }

    /// The whole point of reading `bitPosition`: position 2 has to reach Rust
    /// as 4, so no caller repeats the shift.
    #[test]
    fn bit_positions_are_shifted_and_plain_values_are_not() {
        let mut entry = EnumDef {
            name: "ReceiptModeBitPosition".to_string(),
            module: "m".to_string(),
            value_kind: EnumValueKind::Int,
            variants: vec![EnumVariant {
                name: "HID_FAILED_DECRYPT".to_string(),
                value: Scalar::Int(2),
            }],
            synthetic_name: None,
            bit_position: Some(true),
        };
        let out = masks(wanted("ReceiptModeBitPosition"), &entry).expect("emit");
        assert!(out.contains("pub const RECEIPT_MODE_HID_FAILED_DECRYPT: u32 = 1 << 2;"));

        entry.bit_position = None;
        let plain = masks(wanted("ReceiptModeBitPosition"), &entry).expect("emit");
        assert!(plain.contains("pub const RECEIPT_MODE_HID_FAILED_DECRYPT: u32 = 2;"));
    }

    /// A value the generated file could not compile has to stop the generator,
    /// where the offending catalog entry is still in hand. `1 << 32` would
    /// otherwise surface as an overflow lint inside `wire_enums.rs`.
    #[test]
    fn an_out_of_range_mask_stops_the_generator() {
        let mut entry = EnumDef {
            name: "ReceiptModeBitPosition".to_string(),
            module: "m".to_string(),
            value_kind: EnumValueKind::Int,
            variants: vec![EnumVariant {
                name: "TOO_WIDE".to_string(),
                value: Scalar::Int(32),
            }],
            synthetic_name: None,
            bit_position: Some(true),
        };
        let err = masks(wanted("ReceiptModeBitPosition"), &entry).expect_err("32 is not a u32 bit");
        assert!(err.to_string().contains("does not fit a u32 mask"), "{err}");

        entry.variants[0].value = Scalar::Int(-1);
        let err = masks(wanted("ReceiptModeBitPosition"), &entry).expect_err("negative position");
        assert!(err.to_string().contains("does not fit a u32 mask"), "{err}");

        entry.bit_position = None;
        entry.variants[0].value = Scalar::Int(i64::from(u32::MAX) + 1);
        let err = masks(wanted("ReceiptModeBitPosition"), &entry).expect_err("beyond u32");
        assert!(err.to_string().contains("is not a u32 mask"), "{err}");
    }
}

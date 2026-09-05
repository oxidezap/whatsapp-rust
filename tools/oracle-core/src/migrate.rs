//! Carrying a derivation spec from one capture to the next.
//!
//! WhatsApp renames and rebuilds its modules on every rollout: function
//! indices move, bodies change where WhatsApp edited code, and data moves.
//! A spec pinned to one capture's indices silently answers about different
//! code on the next — so capture bumps re-derive rather than update.
//!
//! This automates the mechanical part of that re-derivation and refuses the
//! rest. For each selector it carries the hint forward by body fingerprint
//! ([`crate::carry`]), re-settles it against its `must_hold_string` anchor
//! on the new bytes, and records the new fingerprint — so a later bump can
//! tell a renumbering (same print, new index) from a rewrite (new print).
//! Anything that is not a unique answer on both sides is reported for a
//! human instead of guessed.
//!
//! Migration never claims the outputs still match: run the migrated spec
//! (its steps are unchanged) and compare output hashes against the old
//! manifest. Same bytes mean the codec survived the bump; different bytes
//! mean WhatsApp changed it, which is a finding, not a failure.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result};
use serde::Serialize;

use crate::carry::{Captures, Carried};
use crate::derive::{FuncSelector, ModulePin, Spec};

/// What became of one selector.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "outcome")]
pub enum Migrated {
    /// Carried one-to-one by fingerprint and re-settled by its anchor.
    Carried {
        /// Index in the old capture.
        old_index: u32,
        /// Index in the new capture.
        new_index: u32,
        /// Body fingerprint on the new bytes.
        fingerprint: Option<u64>,
    },
    /// The shape survived on both sides but is not unique; the string
    /// anchor settled it to one candidate on the new side.
    Disambiguated {
        /// Index in the old capture.
        old_index: u32,
        /// Index in the new capture.
        new_index: u32,
        /// How many shared the shape on the new side.
        candidates: usize,
    },
    /// Needs a human: the body changed, is too short to fingerprint, or the
    /// anchor no longer settles it. The reason says which.
    NeedsHuman {
        /// Index in the old capture.
        old_index: u32,
        /// Why no mechanical route exists.
        reason: String,
    },
}

/// A migrated spec plus per-selector answers.
#[derive(Debug, Clone, Serialize)]
pub struct Migration {
    /// The new module pin, as verified before migrating.
    pub module: MigratedModule,
    /// Per-selector answers, in spec order.
    pub selectors: BTreeMap<String, Migrated>,
    /// Coverage of the bump itself: renumbering or rewrite.
    pub coverage: MigrationCoverage,
}

/// New module identity, as verified.
#[derive(Debug, Clone, Serialize)]
pub struct MigratedModule {
    /// Module id.
    pub id: String,
    /// Verified SHA-256, hex.
    pub sha256: String,
    /// Verified size in bytes.
    pub size: u64,
}

/// How much of the module carried one-to-one.
#[derive(Debug, Clone, Serialize)]
pub struct MigrationCoverage {
    /// Defined functions in the previous capture.
    pub old_functions: usize,
    /// Defined functions in the new one.
    pub new_functions: usize,
    /// How many carry one-to-one.
    pub carried: usize,
}

/// Migrate `spec` (written against `old_bytes`) onto `new_bytes`.
///
/// `new_pin` names the capture the result runs against and is verified
/// before anything else: the new pin must come from a trusted lock, never
/// computed from the file at hand — a hash that vouches for itself vouches
/// for nothing.
///
/// Returns the migrated spec (same steps, updated hints and fingerprints;
/// unresolved selectors removed so their calls cannot execute)
/// and the per-selector report. Fails outright only when a selector's hint
/// does not even exist on the old side; anything unmigratable on the new
/// side is reported per selector instead, so one changed function does not
/// hide the answers for the rest.
pub fn migrate_spec(
    spec: &Spec,
    old_bytes: &[u8],
    new_bytes: &[u8],
    new_pin: &ModulePin,
    new_path: &Path,
) -> Result<(Spec, Migration)> {
    crate::derive::verify_pin(&spec.module, Path::new(&spec.module.id), old_bytes)?;
    crate::derive::resolve_all(old_bytes, spec)?;
    crate::derive::verify_pin(new_pin, new_path, new_bytes)?;

    let old_count = crate::abi::function_count(old_bytes)?;
    let captures =
        Captures::new(old_bytes, new_bytes).context("indexing both captures by fingerprint")?;
    let coverage = captures.coverage();

    let mut migrated = spec.clone();
    migrated.module = new_pin.clone();
    let mut selectors = BTreeMap::new();

    for (name, selector) in &spec.functions {
        let answer =
            migrate_one(&captures, new_bytes, name, selector, old_count).unwrap_or_else(|error| {
                Migrated::NeedsHuman {
                    old_index: selector.index_hint,
                    reason: format!("{error:#}"),
                }
            });
        if let Migrated::Carried { new_index, .. } | Migrated::Disambiguated { new_index, .. } =
            &answer
        {
            let entry = migrated
                .functions
                .get_mut(name)
                .context("selector vanished mid-migration")?;
            entry.index_hint = *new_index;
            entry.expect_fingerprint = crate::derive::body_fingerprint(new_bytes, *new_index)?;
        } else {
            // An old hint under the new module pin could call unrelated code.
            // Missing selectors are refused by derive before instantiation.
            migrated.functions.remove(name);
        }
        selectors.insert(name.clone(), answer);
    }

    Ok((
        migrated,
        Migration {
            module: MigratedModule {
                id: new_pin.id.clone(),
                sha256: new_pin.sha256.clone(),
                size: new_pin.size,
            },
            selectors,
            coverage: MigrationCoverage {
                old_functions: coverage.old_functions,
                new_functions: coverage.new_functions,
                carried: coverage.carried,
            },
        },
    ))
}

/// Migrate one selector.
fn migrate_one(
    captures: &Captures,
    new_bytes: &[u8],
    name: &str,
    selector: &FuncSelector,
    old_count: usize,
) -> Result<Migrated> {
    let hint = selector.index_hint;
    if hint as usize >= old_count {
        anyhow::bail!(
            "selector `{name}`: index_hint {hint} is outside the old capture ({old_count} functions)"
        );
    }

    match captures.carry(hint) {
        Carried::One(found) => {
            check_anchor(new_bytes, name, selector, found)?;
            Ok(Migrated::Carried {
                old_index: hint,
                new_index: found,
                fingerprint: crate::derive::body_fingerprint(new_bytes, found)?,
            })
        }
        Carried::Ambiguous { new, .. } => {
            let Some(needle) = selector.must_hold_string.as_deref() else {
                return Ok(Migrated::NeedsHuman {
                    old_index: hint,
                    reason: format!(
                        "shape shared by {} functions on the new side and no string anchor to settle it",
                        new.len()
                    ),
                });
            };
            let holders = string_holders(new_bytes, needle)?;
            let settled: Vec<u32> = new.into_iter().filter(|c| holders.contains(c)).collect();
            match settled.as_slice() {
                [only] => Ok(Migrated::Disambiguated {
                    old_index: hint,
                    new_index: *only,
                    candidates: holders.len(),
                }),
                _ => Ok(Migrated::NeedsHuman {
                    old_index: hint,
                    reason: format!(
                        "{needle:?} settles to {} of the shape-sharers on the new side, need exactly one",
                        settled.len()
                    ),
                }),
            }
        }
        Carried::Changed => Ok(Migrated::NeedsHuman {
            old_index: hint,
            reason: "body changed; no mechanical route, re-derive it the way it was found the first time"
                .to_owned(),
        }),
        Carried::NotFingerprintable => Ok(Migrated::NeedsHuman {
            old_index: hint,
            reason: "too short to fingerprint, or an import; carry cannot see it".to_owned(),
        }),
    }
}

/// Confirm the anchor still names the carried function on the new bytes.
fn check_anchor(new_bytes: &[u8], name: &str, selector: &FuncSelector, found: u32) -> Result<()> {
    let Some(needle) = selector.must_hold_string.as_deref() else {
        return Ok(());
    };
    let holders = string_holders(new_bytes, needle)?;
    if holders.is_empty() {
        anyhow::bail!("selector `{name}`: no function references {needle:?} on the new capture");
    }
    if !holders.contains(&found) {
        anyhow::bail!(
            "selector `{name}`: carried to {found}, which does not reference {needle:?} \
             (holders: {holders:?}) — the anchor moved, re-derive it"
        );
    }
    Ok(())
}

/// Every function referencing `needle` on these bytes.
fn string_holders(bytes: &[u8], needle: &str) -> Result<Vec<u32>> {
    let refs = crate::abi::find_string_refs(bytes, needle)?;
    let mut holders: Vec<u32> = refs
        .iter()
        .flat_map(|entry| entry.referenced_by.iter().copied())
        .collect();
    holders.sort_unstable();
    holders.dedup();
    Ok(holders)
}

/// Compare a fresh manifest's outputs against a reference manifest's, by file.
///
/// Returns `(matched, mismatched, missing)` file lists. A mismatch is not a
/// verdict on its own: same bytes mean the codec survived the bump, different
/// bytes mean WhatsApp changed it — which is what this comparison exists to
/// surface, with names attached.
pub fn compare_manifests(
    reference: &crate::derive::Manifest,
    fresh: &crate::derive::Manifest,
) -> (Vec<String>, Vec<String>, Vec<String>) {
    let mut matched = Vec::new();
    let mut mismatched = Vec::new();
    let mut missing = Vec::new();
    for expected in &reference.outputs {
        match fresh.outputs.iter().find(|got| got.file == expected.file) {
            Some(got) if got.sha256 == expected.sha256 => matched.push(expected.file.clone()),
            Some(_) => mismatched.push(expected.file.clone()),
            None => missing.push(expected.file.clone()),
        }
    }
    (matched, mismatched, missing)
}

/// Read a manifest written by a previous `derive` run.
pub fn read_manifest(path: &Path) -> Result<crate::derive::Manifest> {
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_slice(&bytes).with_context(|| format!("parsing {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::derive::sha256_hex;

    /// Carrying inside one module is the identity: every fingerprintable
    /// function carries to itself, which is what makes this a test of the
    /// machinery rather than of any capture.
    #[test]
    fn migration_within_one_module_is_the_identity() {
        let bytes = crate::derive::probe_module_bytes();
        let mut functions = BTreeMap::new();
        functions.insert(
            "probe".to_owned(),
            FuncSelector {
                index_hint: 0,
                must_hold_string: None,
                expect_fingerprint: None,
            },
        );
        let spec = Spec {
            module: ModulePin {
                id: "probe".to_owned(),
                sha256: sha256_hex(&bytes),
                size: bytes.len() as u64,
            },
            functions,
            steps: Vec::new(),
        };
        let pin = spec.module.clone();
        let (migrated, report) = migrate_spec(&spec, &bytes, &bytes, &pin, Path::new("probe.wasm"))
            .expect("self-migration");
        assert_eq!(migrated.module.sha256, pin.sha256);
        match &report.selectors["probe"] {
            Migrated::Carried {
                old_index,
                new_index,
                ..
            } => {
                assert_eq!((*old_index, *new_index), (0, 0));
            }
            other => panic!("expected Carried, got {other:?}"),
        }
    }

    /// A hint outside the old module fails the whole migration rather than
    /// resolving to whatever happens to sit at that index elsewhere.
    #[test]
    fn a_hint_outside_the_old_module_fails() {
        let bytes = crate::derive::probe_module_bytes();
        let mut functions = BTreeMap::new();
        functions.insert(
            "probe".to_owned(),
            FuncSelector {
                index_hint: 99,
                must_hold_string: None,
                expect_fingerprint: None,
            },
        );
        let spec = Spec {
            module: ModulePin {
                id: "probe".to_owned(),
                sha256: sha256_hex(&bytes),
                size: bytes.len() as u64,
            },
            functions,
            steps: Vec::new(),
        };
        let pin = spec.module.clone();
        migrate_spec(&spec, &bytes, &bytes, &pin, Path::new("probe.wasm")).expect_err("must fail");
    }
    #[test]
    fn an_old_capture_pin_is_checked_before_carrying() {
        let bytes = crate::derive::probe_module_bytes();
        let pin = ModulePin {
            id: "probe".into(),
            sha256: sha256_hex(&bytes),
            size: bytes.len() as u64,
        };
        let mut spec = Spec {
            module: pin.clone(),
            functions: BTreeMap::new(),
            steps: Vec::new(),
        };
        spec.module.sha256 = "00".repeat(32);
        assert!(migrate_spec(&spec, &bytes, &bytes, &pin, Path::new("probe")).is_err());
    }

    #[test]
    fn an_unresolved_selector_cannot_silently_reuse_its_old_index() {
        use wasm_encoder::{
            CodeSection, Function, FunctionSection, Instruction, Module, TypeSection,
        };
        let mut module = Module::new();
        let mut types = TypeSection::new();
        types.ty().function([], []);
        module.section(&types);
        let mut funcs = FunctionSection::new();
        funcs.function(0);
        module.section(&funcs);
        let mut body = Function::new([]);
        body.instruction(&Instruction::End);
        let mut code = CodeSection::new();
        code.function(&body);
        module.section(&code);
        let bytes = module.finish();
        let pin = ModulePin {
            id: "short".into(),
            sha256: sha256_hex(&bytes),
            size: bytes.len() as u64,
        };
        let spec = Spec {
            module: pin.clone(),
            functions: BTreeMap::from([(
                "short".into(),
                FuncSelector {
                    index_hint: 0,
                    must_hold_string: None,
                    expect_fingerprint: None,
                },
            )]),
            steps: vec![crate::derive::Step::CallFunction {
                func: "short".into(),
                args: vec![],
                results: vec![],
            }],
        };
        let (migrated, report) =
            migrate_spec(&spec, &bytes, &bytes, &pin, Path::new("short")).expect("report");
        assert!(matches!(
            report.selectors["short"],
            Migrated::NeedsHuman { .. }
        ));
        assert!(
            crate::derive::resolve_all(&bytes, &migrated).is_err(),
            "old index must not be callable"
        );
    }
}

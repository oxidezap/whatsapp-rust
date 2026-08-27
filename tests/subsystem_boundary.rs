//! A cuttable subsystem stays cut.
//!
//! `agent_docs/subsystem_boundary.md` classifies a subsystem as cuttable when
//! the core neither holds its state nor runs its code inline, and gives the core
//! a budget of two mentions for it: the `mod` declaration that brings the files
//! in, and the entry in the subsystem list that routes to them. The failure
//! this guards is the cheap one: a third mention, added because a `Client` field
//! or an inline branch was the shortest path, and nothing objected. That is how
//! the subsystems the same document classifies as coupled got to 314 `cfg`
//! sites.
//!
//! It scans text, so it sees a mention in a comment too. That is deliberate: a
//! comment in the core explaining what the subsystem needs is the same coupling
//! one commit early.
//!
//! What it does not reach: the subsystem calling into core internals that exist
//! only for it (test 3 of the rule), and anything outside this crate's `src/`.
//! `Event` variants and payload types stay in `wacore` unconditionally by test 4
//! of the rule, so a green run here does not claim the disabled build carries
//! zero bytes of the subsystem, only zero code, state and branches of its own.

use std::path::{Path, PathBuf};

struct Cuttable {
    /// The identifier the core is not allowed to say, matched case-insensitively.
    name: &'static str,
    /// The directory that owns the subsystem. Never scanned.
    owns: &'static str,
    /// Core files allowed to name it, with the number of lines that may.
    budget: &'static [(&'static str, usize)],
}

/// Whether a line that names a cuttable subsystem is one of the shapes the
/// budget is for: a feature gate, the `mod` declaration that brings the files
/// in, or the entry in the `subsystems!` list.
///
/// The count alone would not catch the cheapest way through: spending one of
/// the allowed lines on a `pub use crate::<name>::Thing` re-export keeps the
/// total the same and puts the subsystem back in the core's surface.
fn is_declaration(line: &str, name: &str) -> bool {
    let line = line.trim();
    // An attribute and nothing else. The `)]` matters: without it
    // `#[cfg(feature = "x")] pub use crate::x::Thing;` is one line that opens
    // like a gate and ends as a re-export.
    let is_attribute =
        (line.starts_with("#[cfg(") || line.starts_with("#[cfg_attr(")) && line.ends_with(")]");
    let is_mod = line == format!("pub mod {name};") || line == format!("mod {name};");
    let is_entry = line.starts_with(&format!("{name}: crate::{name}::")) && line.ends_with(',');
    is_attribute || is_mod || is_entry
}

const CUTTABLE: &[Cuttable] = &[Cuttable {
    name: "passkey",
    owns: "src/passkey",
    budget: &[
        // `pub mod passkey;` with its feature gate and its docs.rs attribute.
        ("src/lib.rs", 3),
        // The entry in the subsystem list, with its feature gate.
        ("src/client/subsystem.rs", 2),
    ],
}];

/// A subsystem the cut rule calls *coupled but disciplined*: it cannot leave the
/// core, so it keeps gates there, and the only thing worth guarding is that the
/// count does not creep back up. Weaker than [`Cuttable`] on purpose, and the
/// batch that wrote `agent_docs/subsystem_boundary.md` needed it: it took VoIP
/// from 29 gates outside its own files to 5, and nothing but this stops the
/// next change from spending that back one field at a time.
struct Disciplined {
    /// The feature whose gates are counted.
    feature: &'static str,
    /// The paths that own the subsystem. Gates inside them are its own business.
    owns: &'static [&'static str],
    /// How many gates may appear anywhere else, production and test alike.
    /// Counting both is deliberate: telling them apart needs a parser this
    /// guard does not have, and a gate that appears is worth a look either way.
    budget: usize,
}

/// Whether `line` conditions something on `feature`.
///
/// Matches the `feature = "..."` term rather than a whole `cfg(feature = "...")`,
/// so a composite gate counts too. That is not hypothetical: rustfmt splits
/// `#[cfg(all(feature = "voip-runtime", any(...)))]` across lines and leaves the
/// term on one of its own, and the budget below sits at exactly the current
/// count, so the cheapest way out of a failure is to write the new gate as
/// `all(...)`. Matching the term also counts `not(feature = "...")`, which is
/// still core code conditioned on the subsystem.
fn names_feature(line: &str, feature: &str) -> bool {
    line.contains(&format!("feature = \"{feature}\""))
}

const DISCIPLINED: &[Disciplined] = &[Disciplined {
    feature: "voip-runtime",
    owns: &["src/voip", "src/client/voip.rs", "src/handlers/call.rs"],
    // 5 in production: the `mod` declaration, the `subsystems!` entry, and the
    // three `pub(crate)` helpers whose only caller is VoIP, which the document
    // records as a deliberate keep. The other 4 are test scaffolding.
    budget: 9,
}];

fn crate_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display()));
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn a_disciplined_subsystem_does_not_creep_back_into_the_core() {
    let root = crate_root();
    let mut sources = Vec::new();
    rust_sources(&root.join("src"), &mut sources);
    sources.sort();

    for subsystem in DISCIPLINED {
        let mut gates = Vec::new();

        for path in &sources {
            let relative = path
                .strip_prefix(&root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            if subsystem
                .owns
                .iter()
                .any(|owned| relative == *owned || relative.starts_with(&format!("{owned}/")))
            {
                continue;
            }

            let source =
                std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {relative}: {e}"));
            for (index, line) in source.lines().enumerate() {
                if names_feature(line, subsystem.feature) {
                    gates.push(format!("  {relative}:{}", index + 1));
                }
            }
        }

        assert!(
            gates.len() <= subsystem.budget,
            "`{}` now has {} gates outside the files it owns, budget is {}:\n{}\n\n\
             Raising the budget is a decision, not a formality: the point of the \
             number is that a subsystem that cannot be cut still does not spread. \
             If the new gate belongs, say why in agent_docs/subsystem_boundary.md \
             and move the budget with it.",
            subsystem.feature,
            gates.len(),
            subsystem.budget,
            gates.join("\n"),
        );
    }
}

#[test]
fn the_core_does_not_name_a_cuttable_subsystem() {
    let root = crate_root();
    let mut sources = Vec::new();
    rust_sources(&root.join("src"), &mut sources);
    sources.sort();

    let mut violations = Vec::new();

    for subsystem in CUTTABLE {
        let owns = root.join(subsystem.owns);
        for path in &sources {
            if path.starts_with(&owns) {
                continue;
            }
            let relative = path
                .strip_prefix(&root)
                .unwrap_or(path)
                .to_string_lossy()
                .replace('\\', "/");
            let budget = subsystem
                .budget
                .iter()
                .find(|(file, _)| *file == relative)
                .map(|(_, lines)| *lines);

            let source =
                std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {relative}: {e}"));
            let named: Vec<(usize, &str)> = source
                .lines()
                .enumerate()
                .filter(|(_, line)| line.to_lowercase().contains(subsystem.name))
                .map(|(index, line)| (index + 1, line))
                .collect();
            let hits: Vec<usize> = named.iter().map(|(number, _)| *number).collect();

            if budget.is_some() {
                let shapes: Vec<usize> = named
                    .iter()
                    .filter(|(_, line)| !is_declaration(line, subsystem.name))
                    .map(|(number, _)| *number)
                    .collect();
                if !shapes.is_empty() {
                    violations.push(format!(
                        "{relative} names `{}` at {shapes:?} in something other than a feature \
                         gate, its `mod` declaration or its `subsystems!` entry",
                        subsystem.name
                    ));
                }
            }

            match budget {
                None if !hits.is_empty() => violations.push(format!(
                    "{relative} names `{}` at {hits:?}; the core may name a cuttable subsystem \
                     only in its `mod` declaration and its subsystem-list entry",
                    subsystem.name
                )),
                Some(allowed) if hits.len() > allowed => violations.push(format!(
                    "{relative} names `{}` on {} lines {hits:?}, budget is {allowed}",
                    subsystem.name,
                    hits.len()
                )),
                _ => {}
            }
        }
    }

    assert!(
        violations.is_empty(),
        "subsystem boundary violated:\n  {}",
        violations.join("\n  ")
    );
}

#[cfg(test)]
mod gate {
    use super::names_feature;

    /// Every form a gate takes in this tree, including the one rustfmt produces.
    #[test]
    fn a_gate_is_counted_however_it_is_written() {
        for line in [
            "#[cfg(feature = \"voip-runtime\")]",
            "#[cfg(all(feature = \"voip-runtime\", not(target_arch = \"wasm32\")))]",
            "    feature = \"voip-runtime\",",
            "#[cfg(not(feature = \"voip-runtime\"))]",
            "#[cfg_attr(feature = \"voip-runtime\", allow(dead_code))]",
        ] {
            assert!(
                names_feature(line, "voip-runtime"),
                "{line:?} conditions on the feature and has to count"
            );
        }
    }

    #[test]
    fn another_feature_is_not() {
        for line in [
            "#[cfg(feature = \"voip\")]",
            "#[cfg(feature = \"voip-encoded\")]",
            "// voip-runtime is the native media plane",
        ] {
            assert!(
                !names_feature(line, "voip-runtime"),
                "{line:?} does not condition on the feature"
            );
        }
    }
}

#[cfg(test)]
mod shape {
    use super::is_declaration;

    /// The three shapes the budget exists for.
    #[test]
    fn a_declaration_is_recognized() {
        for line in [
            "#[cfg(feature = \"passkey\")]",
            "    #[cfg_attr(docsrs, doc(cfg(feature = \"passkey\")))]",
            "pub mod passkey;",
            "    passkey: crate::passkey::Passkey,",
        ] {
            assert!(
                is_declaration(line, "passkey"),
                "{line:?} is one of the shapes the core is allowed"
            );
        }
    }

    /// What counting alone would let through: the same number of lines, spent
    /// on putting the subsystem back into the core's surface.
    #[test]
    fn anything_else_is_not() {
        for line in [
            "pub use crate::passkey::PasskeyError;",
            "#[cfg(feature = \"passkey\")] pub use crate::passkey::PasskeyError;",
            "    let state = self.subsystem::<crate::passkey::Passkey>();",
            "// passkey needs the event bus here",
        ] {
            assert!(
                !is_declaration(line, "passkey"),
                "{line:?} is coupling, not a declaration"
            );
        }
    }
}

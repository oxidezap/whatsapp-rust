//! A cuttable subsystem stays cut.
//!
//! `agent_docs/subsystem_boundary.md` classifies a subsystem as cuttable when
//! the core neither holds its state nor runs its code inline, and gives the core
//! a budget of two mentions for it: the `mod` declaration that brings the files
//! in, and the entry in the attachment table that routes to them. The failure
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

const CUTTABLE: &[Cuttable] = &[Cuttable {
    name: "passkey",
    owns: "src/passkey",
    budget: &[
        // `pub mod passkey;` with its feature gate and its docs.rs attribute.
        ("src/lib.rs", 3),
        // The attachment-table entry with its feature gate.
        ("src/client/subsystem.rs", 2),
    ],
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
            let hits: Vec<usize> = source
                .lines()
                .enumerate()
                .filter(|(_, line)| line.to_lowercase().contains(subsystem.name))
                .map(|(index, _)| index + 1)
                .collect();

            match budget {
                None if !hits.is_empty() => violations.push(format!(
                    "{relative} names `{}` at {hits:?}; the core may name a cuttable subsystem \
                     only in its `mod` declaration and its attachment-table entry",
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

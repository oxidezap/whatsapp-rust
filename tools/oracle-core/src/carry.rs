//! Carrying a function index from one capture to the next.
//!
//! WhatsApp renumbers every function between rollouts, so an index recorded
//! against one binary answers about different code in the next — silently, in
//! the worst way: the read still succeeds. `AGENTS.md` calls a capture bump a
//! re-derivation for exactly this reason, and before this existed the
//! re-derivation was done by hand, one function at a time.
//!
//! This is the one place the oracle borrows from the decompiler, and it is the
//! reason they share a repository. [`unwasm_core::analysis::fingerprint`] hashes
//! a body's *shape* and its signature while dropping precisely what a rebuild
//! changes anyway — constant values, call targets, global indices. A function
//! that survived a rebuild unchanged keeps its fingerprint and can be found
//! again by it.
//!
//! Measured on the `D5pLH9sfOOl` -> `JgwtTQVeWPm` bump: 13,347 functions became
//! 14,733, and **6,561 carry one-to-one** in about half a second. What does not
//! carry is the code WhatsApp edited, which is the honest answer rather than a
//! failure — `make_and_cache_offer` really is a different function now.
//!
//! ## Why it costs a full decode, and why that is affordable here
//!
//! Everything else in this crate streams the module without building its
//! bodies, because `inspect`, `xref` and `callers` are asked constantly and a
//! full decode costs ~300 ms against under 10 ms. This is the exception: it is
//! asked once per capture bump, not once per question, so paying the decode for
//! a whole-module answer is the right trade. Do not reach for it from the hot
//! paths.

use std::collections::HashMap;

use anyhow::{Context, Result};
use unwasm_core::analysis::{FINGERPRINT_FLOOR, fingerprint};
use unwasm_core::module::Module;

/// What became of one index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Carried {
    /// Exactly one function on each side carries this shape.
    ///
    /// The only answer worth acting on: unique on *both* sides, so neither a
    /// coincidence in the new capture nor a family of identical helpers in the
    /// old one can be mistaken for a match.
    One(u32),
    /// The shape survived but is not unique. The candidates are given so a
    /// second fact — a table slot, a string it references, a call count — can
    /// settle it.
    Ambiguous {
        /// How many old functions share the shape.
        old: usize,
        /// The candidates in the new capture.
        new: Vec<u32>,
    },
    /// The body changed. There is no mechanical route from the old index, and
    /// the function has to be found the way it was found the first time.
    Changed,
    /// Too short to fingerprint, or an import. Below
    /// [`FINGERPRINT_FLOOR`] distinct functions collide, so a match would be
    /// worse than no answer.
    NotFingerprintable,
}

/// A pair of captures, ready to answer questions about what moved.
pub struct Captures {
    old_by_print: HashMap<u64, Vec<u32>>,
    new_by_print: HashMap<u64, Vec<u32>>,
    old_print: HashMap<u32, u64>,
    old_total: usize,
    new_total: usize,
    new_first_defined: u32,
}

impl Captures {
    /// Decodes both modules and indexes them by fingerprint.
    ///
    /// # Errors
    ///
    /// Returns an error if either module cannot be decoded.
    pub fn new(old: &[u8], new: &[u8]) -> Result<Self> {
        let old = Module::parse(old)
            .map_err(|error| anyhow::anyhow!("decoding the previous capture: {error}"))?;
        let new = Module::parse(new)
            .map_err(|error| anyhow::anyhow!("decoding the new capture: {error}"))?;

        let old_prints = prints(&old);
        let new_prints = prints(&new);

        let mut old_by_print: HashMap<u64, Vec<u32>> = HashMap::new();
        for (index, print) in &old_prints {
            old_by_print.entry(*print).or_default().push(*index);
        }
        let mut new_by_print: HashMap<u64, Vec<u32>> = HashMap::new();
        for (index, print) in &new_prints {
            new_by_print.entry(*print).or_default().push(*index);
        }

        Ok(Self {
            old_by_print,
            new_by_print,
            old_print: old_prints.into_iter().collect(),
            old_total: old.funcs.len(),
            new_total: new.funcs.len(),
            new_first_defined: u32::try_from(new.func_imports.len()).unwrap_or(u32::MAX),
        })
    }

    /// Where an index from the previous capture went.
    #[must_use]
    pub fn carry(&self, index: u32) -> Carried {
        let Some(print) = self.old_print.get(&index) else {
            return Carried::NotFingerprintable;
        };
        let old = self.old_by_print.get(print).map_or(0, Vec::len);
        match self.new_by_print.get(print) {
            None => Carried::Changed,
            Some(new) if new.len() == 1 && old == 1 => Carried::One(new[0]),
            Some(new) => Carried::Ambiguous {
                old,
                new: new.clone(),
            },
        }
    }

    /// How much of the module carries one-to-one, which says up front whether a
    /// bump is a renumbering or a rewrite.
    #[must_use]
    pub fn coverage(&self) -> Coverage {
        let carried = self
            .old_by_print
            .iter()
            .filter(|(print, olds)| {
                olds.len() == 1
                    && self
                        .new_by_print
                        .get(*print)
                        .is_some_and(|news| news.len() == 1)
            })
            .count();
        Coverage {
            old_functions: self.old_total,
            new_functions: self.new_total,
            new_first_defined: self.new_first_defined,
            carried,
        }
    }
}

/// The summary [`Captures::coverage`] returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Coverage {
    /// Defined functions in the previous capture.
    pub old_functions: usize,
    /// Defined functions in the new one.
    pub new_functions: usize,
    /// The index the new capture's first *defined* function takes.
    ///
    /// Imports occupy the low indices, so a walk over every function starts
    /// here rather than at zero — which is the kind of off-by-imports that
    /// makes a sweep quietly miss the end of the module.
    pub new_first_defined: u32,
    /// How many carry one-to-one.
    pub carried: usize,
}

/// Every defined function's index and fingerprint, skipping bodies too short
/// for the hash to distinguish.
fn prints(module: &Module) -> Vec<(u32, u64)> {
    let base = module.func_imports.len() as u32;
    module
        .funcs
        .iter()
        .enumerate()
        .filter(|(_, func)| func.body.len() >= FINGERPRINT_FLOOR)
        .map(|(ordinal, func)| {
            (
                base + u32::try_from(ordinal).unwrap_or(u32::MAX),
                fingerprint(module, func),
            )
        })
        .collect()
}

/// Reads two modules off disk and carries the given indices forward.
///
/// # Errors
///
/// Returns an error if either file cannot be read or decoded.
pub fn carry_indices(
    old_path: &std::path::Path,
    new_path: &std::path::Path,
    indices: &[u32],
) -> Result<(Coverage, Vec<(u32, Carried)>)> {
    let old = std::fs::read(old_path).with_context(|| format!("reading {}", old_path.display()))?;
    let new = std::fs::read(new_path).with_context(|| format!("reading {}", new_path.display()))?;

    let captures = Captures::new(&old, &new)?;
    let answers = indices
        .iter()
        .map(|index| (*index, captures.carry(*index)))
        .collect();
    Ok((captures.coverage(), answers))
}

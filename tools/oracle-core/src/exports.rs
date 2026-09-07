//! Looking up a module's exports, and failing loudly when one is missing.
//!
//! This module exists because of one bug. Emscripten ships its thread
//! initialiser as `_emscripten_thread_init` on some builds and
//! `__emscripten_thread_init` on others. The host asked for the two-underscore
//! spelling, the module had the one-underscore spelling, and the lookup was
//! written as:
//!
//! ```ignore
//! let Some(Extern::Func(init)) = caller.get_export("__emscripten_thread_init") else {
//!     return;
//! };
//! ```
//!
//! Nothing failed. The main thread simply never registered itself, so
//! `emscripten_main_thread_process_queued_calls` asserted
//! `emscripten_is_main_runtime_thread()` and trapped — and everything a worker
//! thread queued for the main thread was silently dropped. The symptom appeared
//! thousands of instructions from the cause, and the cause was one underscore.
//!
//! So: a missing export is reported, always, and the report names the near
//! misses. `_emscripten_thread_init` differs from `__emscripten_thread_init` by
//! one character, and saying so turns a day into a minute.

use std::collections::BTreeSet;

use anyhow::{Result, anyhow};
use wasmtime::{Caller, Extern, Func, Table};

use crate::state::HostState;

/// Exports whose names differ from `wanted` only in ways a human would miss.
///
/// Leading underscores are the usual culprit — emscripten's own naming is not
/// consistent about them — and case is the other. Anything matching after
/// stripping both is worth putting in front of the reader.
fn near_misses(available: &BTreeSet<String>, wanted: &[&str]) -> Vec<String> {
    let normalise = |name: &str| name.trim_start_matches('_').to_ascii_lowercase();
    let targets: Vec<String> = wanted.iter().map(|name| normalise(name)).collect();

    available
        .iter()
        .filter(|name| targets.contains(&normalise(name)))
        .cloned()
        .collect()
}

/// Builds the message for an export that is not there.
fn missing(caller: &Caller<'_, HostState>, kind: &str, wanted: &[&str]) -> anyhow::Error {
    missing_from(caller.data().shared.exports.get(), kind, wanted)
}

/// The same message, for a caller that holds the export set rather than a
/// `Caller` — `Runtime` looks up its function table outside any host call.
pub(crate) fn missing_from(
    available: Option<&BTreeSet<String>>,
    kind: &str,
    wanted: &[&str],
) -> anyhow::Error {
    let suggestion = match available {
        Some(available) => {
            let close = near_misses(available, wanted);
            if close.is_empty() {
                format!(
                    "; the module exports {} names, none like it",
                    available.len()
                )
            } else {
                format!("; the module does export {close:?} — spelling?")
            }
        }
        // Populated at instantiation; absent only on a store built by hand.
        None => String::new(),
    };

    anyhow!("module exports no {kind} named any of {wanted:?}{suggestion}")
}

/// Finds the first of `names` exported as a function.
///
/// Several names because emscripten's spellings vary across versions, and a
/// host that accepts only one silently does nothing on the other.
pub fn func(caller: &mut Caller<'_, HostState>, names: &[&str]) -> Result<Func> {
    for name in names {
        if let Some(Extern::Func(func)) = caller.get_export(name) {
            return Ok(func);
        }
    }
    Err(missing(caller, "function", names))
}

/// The same, for an export the caller can do without.
///
/// The absence is logged rather than returned, because "this module has no
/// `setTempRet0`" is a fact about the module and not a failure — but a fact
/// worth seeing when behaviour later looks wrong.
pub fn optional_func(caller: &mut Caller<'_, HostState>, names: &[&str]) -> Option<Func> {
    match func(caller, names) {
        Ok(func) => Some(func),
        Err(error) => {
            caller
                .data()
                .log(format!("optional export absent: {error}"));
            None
        }
    }
}

/// Finds the first of `names` exported as a table.
pub fn table(caller: &mut Caller<'_, HostState>, names: &[&str]) -> Result<Table> {
    for name in names {
        if let Some(Extern::Table(table)) = caller.get_export(name) {
            return Ok(table);
        }
    }
    Err(missing(caller, "table", names))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_extra_underscore_is_reported_as_a_near_miss() {
        let available: BTreeSet<String> = ["_emscripten_thread_init", "malloc", "free"]
            .into_iter()
            .map(str::to_owned)
            .collect();

        let close = near_misses(&available, &["__emscripten_thread_init"]);
        assert_eq!(close, vec!["_emscripten_thread_init".to_owned()]);
    }

    #[test]
    fn an_unrelated_name_is_not_suggested() {
        let available: BTreeSet<String> =
            ["malloc", "free"].into_iter().map(str::to_owned).collect();

        assert!(near_misses(&available, &["__emscripten_thread_init"]).is_empty());
    }

    /// Case alone should not hide a match either.
    #[test]
    fn case_is_normalised_too() {
        let available: BTreeSet<String> = ["setTempRet0"].into_iter().map(str::to_owned).collect();
        assert_eq!(
            near_misses(&available, &["settempret0"]),
            vec!["setTempRet0".to_owned()]
        );
    }
}

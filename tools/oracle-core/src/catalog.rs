//! Discovery of captured modules on disk.
//!
//! The captured artifacts are not vendored here. `cargo xt mlow fetch` places
//! them in `.cache/wa-wasm` at the workspace root, verified against the hashes in
//! `tools/oracle-core/wasm.lock.json`; a copy committed here would drift from the
//! capture that the protocol notes refer to.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};
use serde::Deserialize;
use sha2::{Digest, Sha256};

/// Environment variable pointing at the directory holding captured `.wasm`
/// files. Overrides the lookup below.
pub const DIR_ENV: &str = "WA_WASM_DIR";

const FETCH_DIR: &str = ".cache/wa-wasm";

#[derive(Deserialize)]
struct CaptureLock {
    modules: Vec<CapturePin>,
}

#[derive(Deserialize)]
struct CapturePin {
    #[serde(rename = "fileName")]
    file_name: String,
    sha256: String,
    size: u64,
}

fn capture_pins() -> Result<BTreeMap<String, CapturePin>> {
    let lock: CaptureLock = serde_json::from_str(include_str!("../wasm.lock.json"))?;
    let mut pins = BTreeMap::new();
    for pin in lock.modules {
        let id = pin
            .file_name
            .strip_suffix(".wasm")
            .context("capture lock entry is not a .wasm file")?
            .to_owned();
        ensure!(
            pins.insert(id.clone(), pin).is_none(),
            "duplicate capture {id}"
        );
    }
    Ok(pins)
}

fn captured_module(path: PathBuf, fallback_id: &str) -> Result<CapturedModule> {
    let id = path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or(fallback_id)
        .to_owned();
    let size = path.metadata()?.len();
    if let Some(pin) = capture_pins()?.get(&id) {
        ensure!(
            size == pin.size,
            "capture {id} has {size} bytes; lock requires {}",
            pin.size
        );
        let sha256 = hex::encode(Sha256::digest(std::fs::read(&path)?));
        ensure!(
            sha256 == pin.sha256,
            "capture {id} hashes to {sha256}; lock requires {}",
            pin.sha256
        );
    }
    Ok(CapturedModule { id, path, size })
}

/// Resolve the repository-local cache from the crate's stable manifest path.
fn find_capture_dir() -> Option<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let fetched = root.join(FETCH_DIR);
    holds_a_capture(&fetched).then_some(fetched)
}

/// Whether a candidate directory is one worth stopping at.
///
/// Existing is not enough. A `cargo xt oracle fetch` run that fails or is interrupted
/// leaves `.cache/wa-wasm/` behind with nothing in it, and that empty directory used to
/// win over a populated sibling checkout — so the catalogue came back valid and
/// empty, and every capture-dependent test skipped while the captures were
/// sitting one directory away. A directory answers only if it actually holds a
/// `.wasm`.
fn holds_a_capture(dir: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return false;
    };
    entries.flatten().any(|entry| {
        entry
            .path()
            .extension()
            .is_some_and(|extension| extension == "wasm")
    })
}

/// A captured module found on disk.
#[derive(Debug, Clone)]
pub struct CapturedModule {
    /// The file stem, which for WhatsApp Web assets is its content hash id.
    pub id: String,
    /// Where it sits on disk.
    pub path: PathBuf,
    /// Its size in bytes, which is how the catalogue orders itself.
    pub size: u64,
}

/// The set of captured modules available to this run.
#[derive(Debug, Clone)]
pub struct Catalog {
    dir: PathBuf,
    modules: Vec<CapturedModule>,
}

impl Catalog {
    /// Resolves the capture directory from `WA_WASM_DIR`, then by walking up
    /// from this crate and the working directory looking for a sibling
    /// repository cache, and lists every `.wasm` in it.
    pub fn discover() -> Result<Self> {
        let dir = match std::env::var_os(DIR_ENV) {
            Some(dir) => PathBuf::from(dir),
            None => find_capture_dir().with_context(|| {
                format!(
                    "no directory holding captured .wasm files was found (an empty `{FETCH_DIR}/` \
                     does not count — see `holds_a_capture`); run cargo xt mlow fetch, or set \
                     {DIR_ENV}"
                )
            })?,
        };
        Self::from_dir(dir)
    }

    /// Lists every `.wasm` in one directory, smallest first.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be read.
    pub fn from_dir(dir: impl Into<PathBuf>) -> Result<Self> {
        let dir = dir.into();
        let entries = std::fs::read_dir(&dir)
            .with_context(|| format!("reading capture directory {}", dir.display()))?;

        let mut modules = Vec::new();
        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_none_or(|ext| ext != "wasm") {
                continue;
            }
            modules.push(captured_module(path, "")?);
        }
        modules.sort_by_key(|module| module.size);

        Ok(Self { dir, modules })
    }

    /// The directory these modules were found in.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// Every module found, smallest first.
    pub fn modules(&self) -> &[CapturedModule] {
        &self.modules
    }

    /// Resolves a user-supplied target: either a path to a `.wasm` file or the
    /// id of a catalogued module.
    pub fn resolve(&self, target: &str) -> Result<CapturedModule> {
        let as_path = Path::new(target);
        if as_path.is_file() {
            return captured_module(as_path.to_path_buf(), target);
        }

        self.modules
            .iter()
            .find(|module| module.id == target)
            .cloned()
            .with_context(|| {
                format!(
                    "no module `{target}` in {} and no such file",
                    self.dir.display()
                )
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scratch directory that removes itself. No `tempfile` dependency: this
    /// is the only test that needs one.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(name: &str) -> Self {
            let path = std::env::temp_dir().join(format!("oracle-catalog-{name}"));
            let _ = std::fs::remove_dir_all(&path);
            std::fs::create_dir_all(&path).expect("scratch directory");
            Self(path)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// The failure this guards: `cargo xt oracle fetch` creates `.cache/wa-wasm/` before it
    /// downloads anything, so an interrupted run leaves an empty directory that
    /// used to win the search — and the catalogue came back valid and empty
    /// while a populated sibling checkout sat one directory further up.
    #[test]
    fn an_empty_directory_is_not_a_capture_directory() {
        let scratch = Scratch::new("empty");
        assert!(
            !holds_a_capture(&scratch.0),
            "an empty directory holds no captures"
        );

        // Nor does one holding something else.
        std::fs::write(scratch.0.join("notes.md"), b"not a module").expect("write");
        assert!(!holds_a_capture(&scratch.0));

        std::fs::write(scratch.0.join("Abc123.wasm"), b"\0asm").expect("write");
        assert!(holds_a_capture(&scratch.0), "one .wasm is enough");
    }

    #[test]
    fn a_directory_that_does_not_exist_is_not_one_either() {
        assert!(!holds_a_capture(Path::new(
            "/nonexistent/oracle/capture/dir"
        )));
    }

    #[test]
    fn a_known_capture_name_must_match_its_locked_identity() {
        let scratch = Scratch::new("wrong-known-capture");
        std::fs::write(scratch.0.join("JgwtTQVeWPm.wasm"), b"\0asm").expect("write");
        let error = Catalog::from_dir(&scratch.0).expect_err("wrong capture must fail");
        assert!(error.to_string().contains("lock requires"), "{error:#}");
    }
}

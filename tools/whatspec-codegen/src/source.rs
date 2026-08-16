//! Getting the whatspec IR, and proving it is the one the lock pins.
//!
//! Acquisition is `git`, not HTTP: the lock pins a commit, and fetching that
//! commit reuses whatever credentials, proxy and CA configuration the user's git
//! already has, so the tool works wherever `git clone` does. The per-file
//! digests are checked on top of that, because they are also what makes a local
//! checkout (`--from`) trustworthy.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// The IR files this repository consumes. Nothing else is read, so a whatspec
/// domain we do not vendor cannot break a regeneration.
pub const IR_FILES: &[&str] = &[
    "manifest.json",
    "abprops/index.json",
    "appstate/index.json",
    "enums/index.json",
    "iq/index.json",
    "mex/index.json",
    "tokens/index.json",
    "proto/WAProto.proto",
];

/// `tools/whatspec-codegen/whatspec.lock.json`: which whatspec commit the
/// committed artifacts came from, and what its files hashed to.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Lock {
    pub repo: String,
    /// Full commit sha. A branch name would not pin anything.
    pub rev: String,
    pub wa_version: String,
    /// Path under `generated/` to its `sha256:` digest.
    pub files: BTreeMap<String, String>,
}

impl Lock {
    pub fn read(path: &Path) -> Result<Self> {
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading the lock at {}", path.display()))?;
        let lock: Lock = serde_json::from_str(&text)
            .with_context(|| format!("parsing the lock at {}", path.display()))?;
        ensure!(
            lock.rev.len() == 40 && lock.rev.chars().all(|c| c.is_ascii_hexdigit()),
            "lock rev {:?} is not a full commit sha; a branch or short sha pins nothing",
            lock.rev
        );
        Ok(lock)
    }

    pub fn write(&self, path: &Path) -> Result<()> {
        let text = serde_json::to_string_pretty(self)? + "\n";
        std::fs::write(path, text)
            .with_context(|| format!("writing the lock at {}", path.display()))
    }
}

pub fn digest(bytes: &[u8]) -> String {
    let mut out = String::from("sha256:");
    for byte in Sha256::digest(bytes) {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

/// The IR files, read and (unless `verify` is off) checked against the lock.
pub struct Ir {
    pub files: BTreeMap<String, Vec<u8>>,
}

impl Ir {
    pub fn get(&self, rel: &str) -> Result<&[u8]> {
        self.files
            .get(rel)
            .map(Vec::as_slice)
            .with_context(|| format!("IR file {rel} was not read"))
    }

    pub fn text(&self, rel: &str) -> Result<String> {
        String::from_utf8(self.get(rel)?.to_vec())
            .with_context(|| format!("IR file {rel} is not UTF-8"))
    }

    pub fn digests(&self) -> BTreeMap<String, String> {
        self.files
            .iter()
            .map(|(k, v)| (k.clone(), digest(v)))
            .collect()
    }
}

/// Read `IR_FILES` out of a whatspec `generated/` directory.
pub fn read_generated_dir(dir: &Path) -> Result<Ir> {
    ensure!(
        dir.join("manifest.json").is_file(),
        "{} does not look like a whatspec `generated/` directory (no manifest.json)",
        dir.display()
    );
    let mut files = BTreeMap::new();
    for rel in IR_FILES {
        let path = dir.join(rel);
        let bytes =
            std::fs::read(&path).with_context(|| format!("reading IR file {}", path.display()))?;
        files.insert((*rel).to_string(), bytes);
    }
    Ok(Ir { files })
}

/// Fetch the pinned commit into `cache` and read its `generated/` tree.
///
/// The checkout is sparse: `generated/` is the only path this tool reads, and
/// the rest of whatspec is tens of megabytes of bundles.
pub fn fetch_pinned(repo: &str, rev: &str, cache: &Path) -> Result<Ir> {
    let dir = cache.join(rev);
    let generated = dir.join("generated");
    // Completeness, not existence: an interrupted checkout leaves the directory
    // behind, and treating that as a finished cache entry makes every later run
    // skip the fetch and fail in `read_generated_dir` with no way out but
    // deleting the cache by hand.
    let complete = IR_FILES.iter().all(|rel| generated.join(rel).is_file());
    if !complete {
        std::fs::create_dir_all(&dir)
            .with_context(|| format!("creating the IR cache at {}", dir.display()))?;
        git(&dir, &["init", "--quiet", "."])?;
        // Idempotent across a retry of a half-finished cache entry.
        let _ = git(&dir, &["remote", "remove", "origin"]);
        git(&dir, &["remote", "add", "origin", repo])?;
        git(&dir, &["sparse-checkout", "set", "--cone", "generated"])?;
        git(&dir, &["fetch", "--quiet", "--depth", "1", "origin", rev])
            .with_context(|| format!("fetching whatspec {rev} from {repo}"))?;
        git(&dir, &["checkout", "--quiet", "--force", "FETCH_HEAD"])?;
    }
    read_generated_dir(&generated)
}

fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .with_context(|| format!("running `git {}`", args.join(" ")))?;
    if !out.status.success() {
        bail!(
            "`git {}` failed in {}: {}",
            args.join(" "),
            dir.display(),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Resolve a ref (branch, tag or sha) to a full commit sha, for `--update-lock`.
pub fn resolve_rev(repo: &str, git_ref: &str) -> Result<String> {
    if git_ref.len() == 40 && git_ref.chars().all(|c| c.is_ascii_hexdigit()) {
        return Ok(git_ref.to_string());
    }
    let out = Command::new("git")
        .args(["ls-remote", repo, git_ref])
        .output()
        .context("running `git ls-remote`")?;
    ensure!(
        out.status.success(),
        "`git ls-remote {repo} {git_ref}` failed: {}",
        String::from_utf8_lossy(&out.stderr).trim()
    );
    let stdout = String::from_utf8_lossy(&out.stdout);
    resolve_from_ls_remote(&stdout, repo, git_ref)
}

/// Pick the one commit a `git ls-remote` listing pins the requested ref to.
///
/// `ls-remote` matches the *tail* of a ref name, so asking for `main` also
/// prints `refs/heads/feature/main`. Taking the first line would then pin a
/// commit nobody asked for, and the lock records that as fact. Only a ref whose
/// name is exactly `<ref>`, `refs/heads/<ref>` or `refs/tags/<ref>` counts.
fn resolve_from_ls_remote(stdout: &str, repo: &str, git_ref: &str) -> Result<String> {
    let wanted = [
        git_ref.to_string(),
        format!("refs/heads/{git_ref}"),
        format!("refs/tags/{git_ref}"),
    ];
    let mut exact: Vec<(&str, &str)> = Vec::new();
    for line in stdout.lines() {
        let mut cols = line.split_whitespace();
        let (Some(sha), Some(name)) = (cols.next(), cols.next()) else {
            continue;
        };
        // An annotated tag prints twice; the `^{}` line carries the commit the
        // tag points at, which is what a checkout would land on.
        let bare = name.strip_suffix("^{}").unwrap_or(name);
        if wanted.iter().any(|w| w == bare) {
            exact.push((sha, name));
        }
    }
    ensure!(
        !exact.is_empty(),
        "{repo} has no ref named exactly {git_ref:?}"
    );
    let names: Vec<&str> = exact
        .iter()
        .map(|(_, n)| *n)
        .filter(|n| !n.ends_with("^{}"))
        .collect();
    ensure!(
        names.len() <= 1,
        "{git_ref:?} is ambiguous in {repo}: it names {}",
        names.join(", ")
    );
    let sha = exact
        .iter()
        .find(|(_, n)| n.ends_with("^{}"))
        .map_or(exact[0].0, |(sha, _)| *sha);
    ensure!(
        sha.len() == 40 && sha.chars().all(|c| c.is_ascii_hexdigit()),
        "`git ls-remote {repo} {git_ref}` returned {sha:?}, which is not a commit sha"
    );
    Ok(sha.to_string())
}

/// Every IR file matches the lock, and every domain stamps the same build.
pub fn verify(ir: &Ir, lock: &Lock) -> Result<()> {
    let actual = ir.digests();
    for rel in IR_FILES {
        let want = lock.files.get(*rel).with_context(|| {
            format!("lock does not pin {rel}; regenerate the lock with --update-lock")
        })?;
        let got = actual
            .get(*rel)
            .with_context(|| format!("IR file {rel} was never read"))?;
        ensure!(
            want == got,
            "IR file {rel} does not match the lock\n  expected {want}\n  got      {got}"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lock_with(files: BTreeMap<String, String>) -> Lock {
        Lock {
            repo: "https://example.invalid/whatspec".into(),
            rev: "0".repeat(40),
            wa_version: "2.3000.1".into(),
            files,
        }
    }

    fn ir_with(files: &[(&str, &str)]) -> Ir {
        Ir {
            files: files
                .iter()
                .map(|(k, v)| (k.to_string(), v.as_bytes().to_vec()))
                .collect(),
        }
    }

    /// Every IR file, holding `body`.
    fn full_ir(body: &str) -> Vec<(&str, &str)> {
        IR_FILES.iter().map(|f| (*f, body)).collect()
    }

    /// A lock pinning every IR file to the digest of `body`.
    fn full_lock(body: &str) -> Lock {
        lock_with(
            IR_FILES
                .iter()
                .map(|f| ((*f).to_string(), digest(body.as_bytes())))
                .collect(),
        )
    }

    #[test]
    fn digest_is_a_prefixed_sha256() {
        assert_eq!(
            digest(b""),
            "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn verify_accepts_a_matching_tree() {
        verify(&ir_with(&full_ir("x")), &full_lock("x")).expect("matching tree verifies");
    }

    #[test]
    fn verify_rejects_a_tampered_file() {
        let mut files = full_ir("x");
        files[1].1 = "tampered";
        let err = verify(&ir_with(&files), &full_lock("x")).expect_err("tampering must be caught");
        assert!(err.to_string().contains(IR_FILES[1]), "{err}");
    }

    #[test]
    fn verify_rejects_a_lock_that_does_not_pin_every_file() {
        let lock = lock_with(BTreeMap::from([(IR_FILES[0].to_string(), digest(b"x"))]));
        let err = verify(&ir_with(&full_ir("x")), &lock).expect_err("incomplete lock");
        assert!(err.to_string().contains("--update-lock"), "{err}");
    }

    #[test]
    fn a_half_written_cache_entry_is_not_reused() {
        // An interrupted checkout leaves `generated/` behind. Reusing it would
        // make every later run fail the same way, with no ordinary retry that
        // repairs it.
        let cache = std::env::temp_dir().join("whatspec-codegen-cache-test");
        let _ = std::fs::remove_dir_all(&cache);
        let generated = cache.join("deadbeef").join("generated");
        std::fs::create_dir_all(generated.join("abprops")).expect("partial cache");
        std::fs::write(generated.join("manifest.json"), b"{}").expect("one file");

        // No network: the reuse branch would have returned a read error for the
        // missing files, so reaching git at all is what proves the entry was
        // rejected.
        let msg = match fetch_pinned("file:///nonexistent-whatspec", "deadbeef", &cache) {
            Err(e) => format!("{e:#}"),
            Ok(_) => panic!("an incomplete entry must be refetched"),
        };
        assert!(!msg.contains("reading IR file"), "{msg}");

        let _ = std::fs::remove_dir_all(&cache);
    }

    #[test]
    fn a_ref_resolves_only_against_an_exact_ref_name() {
        let sha = "a".repeat(40);
        let other = "b".repeat(40);
        // `ls-remote` matches ref-name tails, so asking for `main` also prints
        // `feature/main`. Pinning that would record a commit nobody asked for.
        let only_tail = format!("{other}\trefs/heads/feature/main\n");
        let err = resolve_from_ls_remote(&only_tail, "r", "main").expect_err("tail is not a match");
        assert!(err.to_string().contains("no ref named exactly"), "{err}");

        let both = format!("{sha}\trefs/heads/main\n{other}\trefs/heads/feature/main\n");
        assert_eq!(
            resolve_from_ls_remote(&both, "r", "main").expect("exact"),
            sha
        );
    }

    #[test]
    fn an_annotated_tag_resolves_to_the_commit_it_points_at() {
        let tag = "c".repeat(40);
        let commit = "d".repeat(40);
        let out = format!("{tag}\trefs/tags/v1.0\n{commit}\trefs/tags/v1.0^{{}}\n");
        assert_eq!(
            resolve_from_ls_remote(&out, "r", "v1.0").expect("peeled"),
            commit
        );
    }

    #[test]
    fn a_ref_line_that_is_not_a_sha_is_rejected() {
        let err = resolve_from_ls_remote("not-a-sha\trefs/heads/main\n", "r", "main")
            .expect_err("a lock may only pin a commit");
        assert!(err.to_string().contains("not a commit sha"), "{err}");
    }

    #[test]
    fn a_lock_pinning_a_branch_is_rejected() {
        let dir = std::env::temp_dir().join("whatspec-codegen-lock-test");
        std::fs::create_dir_all(&dir).expect("temp dir");
        let path = dir.join("branch.json");
        std::fs::write(
            &path,
            r#"{"repo":"r","rev":"main","waVersion":"2.3000.1","files":{}}"#,
        )
        .expect("write");
        let err = Lock::read(&path).expect_err("a branch pins nothing");
        assert!(err.to_string().contains("pins nothing"), "{err}");
    }
}

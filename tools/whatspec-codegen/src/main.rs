//! Regenerates the whatspec-derived artifacts committed in this repository.
//!
//! whatspec publishes a language-neutral IR (`generated/*/index.json`) and no
//! longer commits Rust, so refreshing a vendored file is no longer a copy. This
//! tool is the missing half: it reads that IR at a pinned commit and writes the
//! files `build` lists, in the shape `wacore` already exposes.
//!
//! ```text
//! cargo run -p whatspec-codegen                  # regenerate from the pinned commit
//! cargo run -p whatspec-codegen -- --check       # fail if the tree has drifted
//! cargo run -p whatspec-codegen -- --from ../whatspec/generated
//! cargo run -p whatspec-codegen -- --update-lock --rev main
//! ```
//!
//! Regeneration is all-or-nothing on purpose. Every artifact carries the same
//! `waVersion` stamp, and the version a device announces is generated from it,
//! so refreshing one domain alone is the drift this tool exists to remove.

// A CLI's output is its interface; the workspace print lints are aimed at the
// library, where diagnostics belong in `log`.
#![allow(clippy::print_stdout, clippy::print_stderr)]

mod emit;
mod ir;
mod naming;
mod source;

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail, ensure};

use source::{Ir, Lock};

/// Where the IR comes from.
enum Origin {
    /// The commit the lock pins, fetched into the target directory.
    Pinned,
    /// A whatspec `generated/` directory the caller already has.
    Local(PathBuf),
}

struct Options {
    origin: Origin,
    check: bool,
    update_lock: bool,
    /// Ref to resolve when updating the lock.
    git_ref: String,
    /// Skip the `protoc` step, for a machine that has no protoc.
    skip_proto_desc: bool,
}

const USAGE: &str = "\
Usage: cargo run -p whatspec-codegen -- [options]

  --check                Do not write; exit non-zero if any artifact differs.
  --from <dir>           Read a whatspec `generated/` directory instead of fetching.
  --update-lock          Re-pin the lock to --rev and rewrite it.
  --rev <git-ref>        Ref to pin when updating the lock (default: main).
  --skip-proto-desc      Do not run protoc after writing the .proto.
  -h, --help             This message.
";

/// One committed file: its path in the repo and its generated content.
struct Artifact {
    /// Repo-relative, so the diagnostics name the file a reader can open.
    path: &'static str,
    content: String,
    /// Whether the content is Rust and has to go through rustfmt.
    rust: bool,
}

fn main() -> Result<()> {
    let opts = parse_args(std::env::args().skip(1))?;
    let repo_root = repo_root();
    let lock_path = repo_root.join("tools/whatspec-codegen/whatspec.lock.json");
    let mut lock = Lock::read(&lock_path)?;

    if opts.update_lock {
        lock.rev = source::resolve_rev(&lock.repo, &opts.git_ref)?;
        println!("pinning {} at {}", lock.repo, lock.rev);
    }

    let ir = match &opts.origin {
        Origin::Local(dir) => source::read_generated_dir(dir)?,
        Origin::Pinned => {
            source::fetch_pinned(&lock.repo, &lock.rev, &repo_root.join("target/whatspec-ir"))?
        }
    };

    let wa_version = stamped_version(&ir)?;
    if opts.update_lock {
        lock.wa_version = wa_version.clone();
        lock.files = ir.digests();
        lock.write(&lock_path)?;
        println!("wrote {}", lock_path.display());
    } else {
        source::verify(&ir, &lock)?;
        ensure!(
            lock.wa_version == wa_version,
            "lock says WhatsApp {} but the IR stamps {wa_version}; run --update-lock",
            lock.wa_version
        );
    }

    let artifacts = build(&ir, &wa_version)?;
    if opts.check {
        return check(&repo_root, &artifacts);
    }
    write(&repo_root, &artifacts)?;

    if !opts.skip_proto_desc {
        regenerate_proto_descriptor(&repo_root)?;
    }
    println!(
        "regenerated {} artifacts at WhatsApp {wa_version}",
        artifacts.len()
    );
    Ok(())
}

fn parse_args(args: impl Iterator<Item = String>) -> Result<Options> {
    let mut opts = Options {
        origin: Origin::Pinned,
        check: false,
        update_lock: false,
        git_ref: "main".to_string(),
        skip_proto_desc: false,
    };
    let mut args = args.peekable();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--check" => opts.check = true,
            "--update-lock" => opts.update_lock = true,
            "--skip-proto-desc" => opts.skip_proto_desc = true,
            "--from" => {
                let dir = args.next().context("--from needs a directory")?;
                opts.origin = Origin::Local(PathBuf::from(dir));
            }
            "--rev" => opts.git_ref = args.next().context("--rev needs a git ref")?,
            "-h" | "--help" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            other => bail!("unknown argument {other:?}\n\n{USAGE}"),
        }
    }
    ensure!(
        !(opts.check && opts.update_lock),
        "--check and --update-lock contradict each other"
    );
    Ok(opts)
}

/// The workspace root, from this crate's manifest directory.
fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the crate lives at <root>/tools/whatspec-codegen")
        .to_path_buf()
}

/// The one WhatsApp build every IR document must agree on.
///
/// whatspec emits the domains from a single bundle set, so a disagreement means
/// the tree was assembled from two runs and the artifacts would be internally
/// inconsistent, which is the failure this tool replaces.
fn stamped_version(ir: &Ir) -> Result<String> {
    let manifest: ir::Manifest =
        serde_json::from_str(&ir.text("manifest.json")?).context("parsing manifest.json")?;
    let major = manifest
        .schema_version
        .split('.')
        .next()
        .unwrap_or_default();
    ensure!(
        major == ir::SUPPORTED_SCHEMA_MAJOR,
        "whatspec IR schemaVersion {} is not supported (this tool reads {}.x)",
        manifest.schema_version,
        ir::SUPPORTED_SCHEMA_MAJOR
    );

    for rel in source::IR_FILES {
        if !rel.ends_with("index.json") {
            continue;
        }
        let envelope: ir::Envelope = serde_json::from_str(&ir.text(rel)?)
            .with_context(|| format!("parsing the envelope of {rel}"))?;
        ensure!(
            envelope.wa_version == manifest.wa_version,
            "{rel} stamps WhatsApp {} but the manifest stamps {}",
            envelope.wa_version,
            manifest.wa_version
        );
        ensure!(
            envelope.schema_version == manifest.schema_version,
            "{rel} stamps IR schema {} but the manifest stamps {}",
            envelope.schema_version,
            manifest.schema_version
        );
    }
    Ok(manifest.wa_version)
}

fn build(ir: &Ir, wa_version: &str) -> Result<Vec<Artifact>> {
    let abprops: ir::AbPropsIr =
        serde_json::from_str(&ir.text("abprops/index.json")?).context("parsing the abprops IR")?;
    let appstate: ir::AppstateIr = serde_json::from_str(&ir.text("appstate/index.json")?)
        .context("parsing the appstate IR")?;
    let mex: ir::MexIr =
        serde_json::from_str(&ir.text("mex/index.json")?).context("parsing the mex IR")?;
    let tokens: ir::TokensIr =
        serde_json::from_str(&ir.text("tokens/index.json")?).context("parsing the tokens IR")?;

    Ok(vec![
        Artifact {
            path: "wacore/src/version/generated.rs",
            content: emit::version::generate(wa_version)?,
            rust: true,
        },
        Artifact {
            path: "wacore/src/iq/abprops.rs",
            content: emit::abprops::generate(&abprops),
            rust: true,
        },
        Artifact {
            path: "wacore/src/iq/mex_operations.rs",
            content: emit::mex::generate(&mex),
            rust: true,
        },
        Artifact {
            path: "wacore/appstate/src/schemas.rs",
            content: emit::appstate::generate(&appstate),
            rust: true,
        },
        Artifact {
            path: "wacore/binary/src/tokens.json",
            content: emit::tokens::generate(&tokens)?,
            rust: false,
        },
        Artifact {
            path: "waproto/src/whatsapp.proto",
            content: emit::proto::generate(&ir.text("proto/WAProto.proto")?)?,
            rust: false,
        },
    ])
}

fn write(root: &Path, artifacts: &[Artifact]) -> Result<()> {
    for a in artifacts {
        let path = root.join(a.path);
        let content = finalize(root, a)?;
        // Leave the mtime alone when nothing changed: these files feed build
        // scripts whose rerun-if-changed would otherwise rebuild the workspace.
        if std::fs::read_to_string(&path).is_ok_and(|old| old == content) {
            println!("unchanged {}", a.path);
            continue;
        }
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, &content).with_context(|| format!("writing {}", path.display()))?;
        println!("wrote     {}", a.path);
    }
    Ok(())
}

fn check(root: &Path, artifacts: &[Artifact]) -> Result<()> {
    let mut stale = Vec::new();
    for a in artifacts {
        let path = root.join(a.path);
        let content = finalize(root, a)?;
        match std::fs::read_to_string(&path) {
            Ok(on_disk) if on_disk == content => {}
            Ok(_) => stale.push(a.path),
            Err(e) => {
                eprintln!("{}: {e}", a.path);
                stale.push(a.path);
            }
        }
    }
    ensure!(
        stale.is_empty(),
        "these artifacts do not match the pinned IR: {}\nrun `cargo run -p whatspec-codegen`",
        stale.join(", ")
    );
    println!("all artifacts match the pinned IR");
    Ok(())
}

/// The exact bytes an artifact should have on disk, rustfmt included.
fn finalize(root: &Path, a: &Artifact) -> Result<String> {
    if a.rust {
        rustfmt(root, &a.content)
    } else {
        Ok(a.content.clone())
    }
}

/// Format through the repository's pinned rustfmt.
///
/// Via a scratch file rather than stdin so `--check` can format without touching
/// the tree, and run from the workspace root so `rust-toolchain.toml` selects the
/// same rustfmt CI runs.
fn rustfmt(root: &Path, source: &str) -> Result<String> {
    let dir = root.join("target/whatspec-codegen");
    std::fs::create_dir_all(&dir)?;
    let path = dir.join("scratch.rs");
    std::fs::write(&path, source)?;
    let out = Command::new("rustfmt")
        .current_dir(root)
        .arg("--edition")
        .arg("2024")
        .arg(&path)
        .output()
        .context("running rustfmt (install it with `rustup component add rustfmt`)")?;
    ensure!(
        out.status.success(),
        "rustfmt rejected generated source: {}",
        String::from_utf8_lossy(&out.stderr).trim()
    );
    std::fs::read_to_string(&path).context("reading back the formatted source")
}

/// Rebuild `whatsapp.desc` from the `.proto` we just wrote.
///
/// The descriptor is what `waproto`'s build script compiles, and it carries a
/// hash of the `.proto` beside it, so leaving it behind fails the next build
/// rather than shipping a stale schema.
fn regenerate_proto_descriptor(root: &Path) -> Result<()> {
    let script = root.join("scripts/regenerate-proto-desc.sh");
    let status = Command::new("bash")
        .arg(&script)
        .current_dir(root)
        .status()
        .with_context(|| format!("running {}", script.display()))?;
    ensure!(
        status.success(),
        "{} failed; install protoc, or pass --skip-proto-desc and run it yourself",
        script.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Result<Options> {
        parse_args(args.iter().map(|s| s.to_string()))
    }

    #[test]
    fn defaults_to_regenerating_from_the_pinned_commit() {
        let o = parse(&[]).expect("defaults");
        assert!(matches!(o.origin, Origin::Pinned));
        assert!(!o.check && !o.update_lock && !o.skip_proto_desc);
        assert_eq!(o.git_ref, "main");
    }

    #[test]
    fn reads_the_flags_it_documents() {
        let o =
            parse(&["--check", "--from", "/tmp/generated", "--skip-proto-desc"]).expect("flags");
        assert!(o.check && o.skip_proto_desc);
        assert!(matches!(&o.origin, Origin::Local(p) if p == Path::new("/tmp/generated")));
        let o = parse(&["--update-lock", "--rev", "abc"]).expect("flags");
        assert!(o.update_lock);
        assert_eq!(o.git_ref, "abc");
    }

    #[test]
    fn rejects_unknown_and_contradictory_arguments() {
        assert!(parse(&["--wat"]).is_err());
        assert!(parse(&["--from"]).is_err(), "--from needs a value");
        assert!(parse(&["--rev"]).is_err(), "--rev needs a value");
        // --check proves the tree matches the lock; --update-lock rewrites the
        // lock to match the tree. Together they prove nothing.
        assert!(parse(&["--check", "--update-lock"]).is_err());
    }

    #[test]
    fn repo_root_holds_the_workspace_manifest() {
        assert!(repo_root().join("Cargo.toml").is_file());
        assert!(repo_root().join("wacore/src/iq/abprops.rs").is_file());
    }

    fn ir_from(files: &[(&str, String)]) -> Ir {
        Ir {
            files: files
                .iter()
                .map(|(k, v)| (k.to_string(), v.as_bytes().to_vec()))
                .collect(),
        }
    }

    fn envelope(schema: &str, wa: &str) -> String {
        format!(r#"{{"schemaVersion":"{schema}","waVersion":"{wa}"}}"#)
    }

    #[test]
    fn stamped_version_agrees_across_domains() {
        let files: Vec<(&str, String)> = source::IR_FILES
            .iter()
            .map(|f| (*f, envelope("2.0.0", "2.3000.7")))
            .collect();
        assert_eq!(
            stamped_version(&ir_from(&files)).expect("stamp"),
            "2.3000.7"
        );
    }

    #[test]
    fn a_domain_from_another_build_stops_the_run() {
        // The exact drift this tool replaces: abprops and mex vendored from two
        // different WhatsApp releases.
        let mut files: Vec<(&str, String)> = source::IR_FILES
            .iter()
            .map(|f| (*f, envelope("2.0.0", "2.3000.7")))
            .collect();
        let mex = files
            .iter_mut()
            .find(|(f, _)| *f == "mex/index.json")
            .expect("mex");
        mex.1 = envelope("2.0.0", "2.3000.8");
        let err = stamped_version(&ir_from(&files)).expect_err("mixed builds");
        assert!(err.to_string().contains("mex/index.json"), "{err}");
    }

    #[test]
    fn an_unsupported_ir_schema_major_stops_the_run() {
        let files: Vec<(&str, String)> = source::IR_FILES
            .iter()
            .map(|f| (*f, envelope("3.0.0", "2.3000.7")))
            .collect();
        let err = stamped_version(&ir_from(&files)).expect_err("schema 3");
        assert!(err.to_string().contains("not supported"), "{err}");
    }
}

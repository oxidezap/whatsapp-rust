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

/// The one IR document that is not JSON, so the envelope loop cannot reach it.
const PROTO_IR: &str = "proto/WAProto.proto";

/// Its committed counterpart, whose descriptor `--check` also validates.
const PROTO_ARTIFACT: &str = "waproto/src/whatsapp.proto";

/// Where the IR comes from.
#[derive(Debug)]
enum Origin {
    /// The commit the lock pins, fetched into the target directory.
    Pinned,
    /// A whatspec `generated/` directory the caller already has.
    Local(PathBuf),
}

#[derive(Debug)]
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
    if !opts.update_lock {
        source::verify(&ir, &lock)?;
        ensure!(
            lock.wa_version == wa_version,
            "lock says WhatsApp {} but the IR stamps {wa_version}; run --update-lock",
            lock.wa_version
        );
    }

    // Before the lock is touched: the emitters reject a lost proto anchor, an
    // invalid token table and an unrepresentable version, and a lock pinning a
    // build that no committed artifact describes is worse than no new lock.
    let artifacts = build(&ir, &wa_version)?;
    if opts.check {
        return check(&repo_root, &artifacts);
    }

    // Before anything is written, for the same reason. `write` replaces the
    // `.proto` but not the descriptor beside it, so discovering a missing protoc
    // afterwards leaves a tree whose two halves describe different schemas and
    // which does not build.
    if !opts.skip_proto_desc {
        require_protoc()?;
    }
    write(&repo_root, &artifacts)?;
    if !opts.skip_proto_desc {
        regenerate_proto_descriptor(&repo_root)?;
    }
    if opts.update_lock {
        lock.wa_version = wa_version.clone();
        lock.files = ir.digests();
        lock.write(&lock_path)?;
        println!("wrote {}", lock_path.display());
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
    // A lock says "commit X hashes to these digests". Resolving the rev against
    // the remote while hashing an unrelated local tree writes a claim nobody
    // ever checked, and it only surfaces on the next pinned run.
    ensure!(
        !(opts.update_lock && matches!(opts.origin, Origin::Local(_))),
        "--from and --update-lock contradict each other: the lock would pin a remote rev and record local digests"
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

    // The proto is the one domain with no JSON envelope, so skipping it here is
    // how a schema from a different build reaches `whatsapp.proto` while every
    // JSON-derived registry agrees. It carries the same stamp as a comment.
    let proto = ir.text(PROTO_IR)?;
    let stamped = proto
        .lines()
        .find_map(|l| l.trim().strip_prefix("/// WhatsApp Version:"))
        .map(str::trim)
        .with_context(|| format!("{PROTO_IR} carries no `/// WhatsApp Version:` header"))?;
    ensure!(
        stamped == manifest.wa_version,
        "{PROTO_IR} stamps WhatsApp {stamped} but the manifest stamps {}",
        manifest.wa_version
    );

    Ok(manifest.wa_version)
}

fn build(ir: &Ir, wa_version: &str) -> Result<Vec<Artifact>> {
    let abprops: ir::AbPropsIr =
        serde_json::from_str(&ir.text("abprops/index.json")?).context("parsing the abprops IR")?;
    let appstate: ir::AppstateIr = serde_json::from_str(&ir.text("appstate/index.json")?)
        .context("parsing the appstate IR")?;
    let enums: ir::EnumsIr =
        serde_json::from_str(&ir.text("enums/index.json")?).context("parsing the enums IR")?;
    let iq: ir::IqIr =
        serde_json::from_str(&ir.text("iq/index.json")?).context("parsing the IQ IR")?;
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
            content: emit::abprops::generate(&abprops)?,
            rust: true,
        },
        Artifact {
            path: "wacore/src/types/wire_enums.rs",
            content: emit::enums::generate(&enums)?,
            rust: true,
        },
        Artifact {
            path: "wacore/src/iq/targets.rs",
            content: emit::iq_targets::generate(&iq)?,
            rust: true,
        },
        Artifact {
            path: "wacore/src/iq/mex_operations.rs",
            content: emit::mex::generate(&mex),
            rust: true,
        },
        Artifact {
            path: "wacore/appstate/src/schemas.rs",
            content: emit::appstate::generate(&appstate)?,
            rust: true,
        },
        Artifact {
            path: "wacore/binary/src/tokens.json",
            content: emit::tokens::generate(&tokens)?,
            rust: false,
        },
        Artifact {
            path: PROTO_ARTIFACT,
            content: emit::proto::generate(&ir.text(PROTO_IR)?)?,
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
    let proto = artifacts
        .iter()
        .find(|a| a.path == PROTO_ARTIFACT)
        .with_context(|| format!("{PROTO_ARTIFACT} is not in the artifact set"))?;
    check_proto_descriptor(root, &proto.content)?;
    println!("all artifacts match the pinned IR");
    Ok(())
}

/// The descriptor beside the `.proto` describes the same schema.
///
/// `--check` would otherwise pass on a tree regenerated with `--skip-proto-desc`
/// or with a hand-edited `.desc`, and the contradiction would surface much later
/// as a `waproto` build failure in a different job. Comparing the recorded
/// hashes catches it here and needs no protoc.
fn check_proto_descriptor(root: &Path, proto: &str) -> Result<()> {
    let hashes = root.join("waproto/src/whatsapp.desc.sha256");
    let text = std::fs::read_to_string(&hashes)
        .with_context(|| format!("reading {}", hashes.display()))?;
    let recorded = |key: &str| -> Result<String> {
        text.lines()
            .find_map(|l| l.strip_prefix(key).map(|h| h.trim().to_string()))
            .with_context(|| format!("{} has no `{key}` line", hashes.display()))
    };
    let desc = std::fs::read(root.join("waproto/src/whatsapp.desc"))
        .context("reading waproto/src/whatsapp.desc")?;
    for (what, want, got) in [
        ("proto", recorded("proto ")?, sha256_hex(proto.as_bytes())),
        ("desc", recorded("desc ")?, sha256_hex(&desc)),
    ] {
        ensure!(
            want == got,
            "waproto/src/whatsapp.desc.sha256 records {what} {want} but the {} hashes to {got}\n\
             run `cargo run -p whatspec-codegen` (or scripts/regenerate-proto-desc.sh)",
            if what == "proto" {
                "generated schema"
            } else {
                "committed descriptor"
            }
        );
    }
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(bytes)
        .iter()
        .fold(String::new(), |mut s, b| {
            s.push_str(&format!("{b:02x}"));
            s
        })
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
    // Per-process, because CI's `--check` and a developer regenerating in the
    // same checkout would otherwise overwrite each other's scratch between the
    // write and the read-back, and the loser commits the winner's bytes.
    let path = dir.join(format!("scratch-{}.rs", std::process::id()));
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
    let formatted = std::fs::read_to_string(&path).context("reading back the formatted source")?;
    let _ = std::fs::remove_file(&path);
    Ok(formatted)
}

/// Rebuild `whatsapp.desc` from the `.proto` we just wrote.
///
/// The descriptor is what `waproto`'s build script compiles, and it carries a
/// hash of the `.proto` beside it, so leaving it behind fails the next build
/// rather than shipping a stale schema.
/// Fail before writing when the descriptor step could not run afterwards.
fn require_protoc() -> Result<()> {
    let found = Command::new("protoc")
        .arg("--version")
        .output()
        .is_ok_and(|o| o.status.success());
    ensure!(
        found,
        "protoc is not on PATH, and the descriptor is regenerated from the schema this run \
         writes.\nInstall it, or pass --skip-proto-desc and run scripts/regenerate-proto-desc.sh \
         yourself."
    );
    Ok(())
}

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
        let err = parse(&["--from", "/tmp/ir", "--update-lock"])
            .expect_err("--from cannot source an updated lock");
        assert!(
            err.to_string().contains("--from and --update-lock"),
            "{err}"
        );
    }

    #[test]
    fn the_committed_descriptor_hashes_match_the_generated_schema() {
        // The real tree, so this fails if someone regenerates the `.proto` with
        // --skip-proto-desc and commits without rerunning protoc.
        let root = repo_root();
        let proto = std::fs::read_to_string(root.join(PROTO_ARTIFACT)).expect("the proto");
        check_proto_descriptor(&root, &proto).expect("descriptor agrees with the schema");

        let err = check_proto_descriptor(&root, "syntax = \"proto2\";\n")
            .expect_err("a different schema must not pass");
        assert!(format!("{err:#}").contains("records proto"), "{err:#}");
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

    /// A schema version this tool accepts, derived from the gate rather than
    /// spelled out, so raising the supported major does not silently turn every
    /// fixture below into a test of the rejection path.
    fn supported_schema() -> String {
        format!("{}.0.0", ir::SUPPORTED_SCHEMA_MAJOR)
    }

    /// The next major up, which the gate must refuse.
    fn unsupported_schema() -> String {
        let major: u32 = ir::SUPPORTED_SCHEMA_MAJOR
            .parse()
            .expect("the supported major is numeric");
        format!("{}.0.0", major + 1)
    }

    /// The proto carries its stamp as a comment rather than a JSON envelope.
    fn proto_stub(wa: &str) -> String {
        format!("syntax = \"proto2\";\n\n/// WhatsApp Version: {wa}\n")
    }

    fn ir_files(schema: &str, wa: &str) -> Vec<(&'static str, String)> {
        source::IR_FILES
            .iter()
            .map(|f| {
                let body = if *f == PROTO_IR {
                    proto_stub(wa)
                } else {
                    envelope(schema, wa)
                };
                (*f, body)
            })
            .collect()
    }

    #[test]
    fn stamped_version_agrees_across_domains() {
        assert_eq!(
            stamped_version(&ir_from(&ir_files(&supported_schema(), "2.3000.7"))).expect("stamp"),
            "2.3000.7"
        );
    }

    #[test]
    fn a_domain_from_another_build_stops_the_run() {
        // The exact drift this tool replaces: abprops and mex vendored from two
        // different WhatsApp releases.
        let mut files = ir_files(&supported_schema(), "2.3000.7");
        let mex = files
            .iter_mut()
            .find(|(f, _)| *f == "mex/index.json")
            .expect("mex");
        mex.1 = envelope(&supported_schema(), "2.3000.8");
        let err = stamped_version(&ir_from(&files)).expect_err("mixed builds");
        assert!(err.to_string().contains("mex/index.json"), "{err}");
    }

    #[test]
    fn a_proto_from_another_build_stops_the_run() {
        // The proto has no JSON envelope, so the loop above cannot see it. It is
        // the one domain whose drift would otherwise reach whatsapp.proto with
        // every other registry agreeing.
        let mut files = ir_files(&supported_schema(), "2.3000.7");
        let proto = files
            .iter_mut()
            .find(|(f, _)| *f == PROTO_IR)
            .expect("proto");
        proto.1 = proto_stub("2.3000.8");
        let err = stamped_version(&ir_from(&files)).expect_err("mixed builds");
        assert!(err.to_string().contains(PROTO_IR), "{err}");
    }

    #[test]
    fn a_proto_with_no_version_header_stops_the_run() {
        let mut files = ir_files(&supported_schema(), "2.3000.7");
        let proto = files
            .iter_mut()
            .find(|(f, _)| *f == PROTO_IR)
            .expect("proto");
        proto.1 = "syntax = \"proto2\";\n".to_string();
        let err = stamped_version(&ir_from(&files)).expect_err("no stamp to compare");
        assert!(err.to_string().contains("carries no"), "{err}");
    }

    #[test]
    fn an_unsupported_ir_schema_major_stops_the_run() {
        let err = stamped_version(&ir_from(&ir_files(&unsupported_schema(), "2.3000.7")))
            .expect_err("a newer major is not readable");
        assert!(err.to_string().contains("not supported"), "{err}");
    }
}

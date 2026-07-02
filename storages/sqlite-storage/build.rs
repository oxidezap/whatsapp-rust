//! Generates Rust types for the on-disk wire format (`src/wire.rs` consumes
//! them) from the committed descriptor `proto/wire.desc`, compiled once from
//! `proto/wire.proto`. Reading the descriptor means consumers never need
//! `protoc`; editing the proto requires regenerating the descriptor via
//! `scripts/regenerate-wire-desc.sh`.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=proto/wire.desc");
    println!("cargo:rerun-if-changed=proto/wire.proto");
    println!("cargo:rerun-if-changed=proto/wire.desc.sha256");
    println!("cargo:rerun-if-changed=build.rs");

    ensure_descriptor_fresh(
        "proto/wire.proto",
        "proto/wire.desc",
        "proto/wire.desc.sha256",
        "sqlite-storage (wire)",
        "scripts/regenerate-wire-desc.sh",
    )?;

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR must be set by cargo");

    buffa_build::Config::new()
        .descriptor_set("proto/wire.desc")
        .files(&["wire.proto"])
        .preserve_unknown_fields(false)
        .out_dir(&out_dir)
        .compile()
}

/// Fail the build if the committed `.desc` no longer matches its `.proto` (proto
/// edited without rerunning the regenerate script), so codegen never silently
/// runs against a stale descriptor — for `wire.proto` that would ship an
/// incompatible on-disk blob layout.
fn ensure_descriptor_fresh(
    proto: &str,
    desc: &str,
    sha: &str,
    label: &str,
    regen: &str,
) -> std::io::Result<()> {
    use sha2::{Digest as _, Sha256};

    let hex = |bytes: &[u8]| -> String {
        Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    };

    let (mut want_proto, mut want_desc) = (None, None);
    for line in std::fs::read_to_string(sha)?.lines() {
        let mut parts = line.split_whitespace();
        match (parts.next(), parts.next()) {
            (Some("proto"), Some(h)) => want_proto = Some(h.to_owned()),
            (Some("desc"), Some(h)) => want_desc = Some(h.to_owned()),
            _ => {}
        }
    }

    let got_proto = hex(&std::fs::read(proto)?);
    let got_desc = hex(&std::fs::read(desc)?);

    if want_proto.as_deref() != Some(&got_proto) || want_desc.as_deref() != Some(&got_desc) {
        return Err(std::io::Error::other(format!(
            "{label}: {proto}/{desc} do not match {sha}. Run `{regen}` and commit all three. \
             expected proto {want_proto:?}, desc {want_desc:?}; got proto {got_proto}, desc {got_desc}"
        )));
    }

    Ok(())
}

//! Checks the committed MLow schema, descriptor and static codec hashes.
//! Only `voip` consumes these tables. Consumers need no generator or protoc;
//! schema changes regenerate the descriptor and codec with `cargo xt tables-desc`.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=src/voip/mlow/tables.desc");
    println!("cargo:rerun-if-changed=src/voip/mlow/tables.proto");
    println!("cargo:rerun-if-changed=src/voip/mlow/tables.desc.sha256");
    println!("cargo:rerun-if-changed=build.rs");

    if std::env::var_os("CARGO_FEATURE_VOIP").is_none() {
        return Ok(());
    }

    // Gated inside the voip block: the .sha256 only matters when the descriptor
    // is actually consumed, so non-voip builds neither read nor validate it.
    ensure_descriptor_fresh(
        "src/voip/mlow/tables.proto",
        "src/voip/mlow/tables.desc",
        "src/voip/mlow/tables.desc.sha256",
        "wacore (voip tables)",
        "cargo xt tables-desc",
    )?;

    ensure_generated_fresh(
        "src/voip/mlow/tables.desc",
        "src/voip/mlow/tables.generated.rs",
        "src/voip/mlow/tables.generated.rs.sha256",
    )?;
    Ok(())
}

fn ensure_generated_fresh(descriptor: &str, source: &str, manifest: &str) -> std::io::Result<()> {
    use sha2::{Digest as _, Sha256};
    println!("cargo:rerun-if-changed={source}");
    println!("cargo:rerun-if-changed={manifest}");
    let manifest = std::fs::read_to_string(manifest)?;
    for (kind, path) in [("descriptor", descriptor), ("rust", source)] {
        let expected = manifest
            .lines()
            .find_map(|line| line.strip_prefix(&format!("{kind} ")));
        let digest: String = Sha256::digest(std::fs::read(path)?)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        if expected != Some(digest.as_str()) {
            return Err(std::io::Error::other(format!(
                "{path} has stale generated protobuf code. Run `cargo xt proto-codecs`."
            )));
        }
    }
    Ok(())
}

/// Fail the build if the committed `.desc` no longer matches its `.proto` (proto
/// edited without rerunning the regenerate script), so codegen never silently
/// runs against a stale descriptor.
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

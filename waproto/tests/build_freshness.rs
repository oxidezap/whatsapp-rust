//! The build script must leave the crate fresh after its first run.

use std::path::Path;
use std::time::SystemTime;

fn modified(path: &Path) -> SystemTime {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .unwrap_or_else(|err| panic!("{}: {err}", path.display()))
}

/// `buffa_build` registers the spliced descriptor with `rerun-if-changed`.
/// Dating it like its source avoids making a freshly generated output trigger
/// another build-script run. Check that contract directly: Cargo's internal
/// invocation timestamp moved with the Rust 1.100 build-directory layout.
#[test]
fn spliced_descriptor_inherits_its_source_timestamp() {
    let out_dir = Path::new(env!("OUT_DIR"));
    let descriptor = out_dir.join("whatsapp.local.desc");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/whatsapp.desc");
    let descriptor_time = modified(&descriptor);
    let source_time = modified(&source);
    assert_eq!(
        descriptor_time,
        source_time,
        "{} must inherit the timestamp of {} so it cannot cause a redundant build-script run",
        descriptor.display(),
        source.display()
    );
}

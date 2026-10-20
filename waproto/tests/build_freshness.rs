//! The build script must leave the crate fresh after its first run.

use std::path::Path;
use std::time::SystemTime;

fn modified(path: &Path) -> SystemTime {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .unwrap_or_else(|err| panic!("{}: {err}", path.display()))
}

/// Cargo reruns a build script when a path it registered with
/// `rerun-if-changed` is newer than `invoked.timestamp`, which cargo writes
/// just before running the script. `buffa_build` registers the spliced
/// descriptor, so writing it with the current time made every first rebuild
/// rerun the script and recompile waproto and everything that depends on it.
#[test]
fn spliced_descriptor_is_not_newer_than_the_build_script_run() {
    let out_dir = Path::new(env!("OUT_DIR"));
    let descriptor = out_dir.join("whatsapp.local.desc");
    let invoked = out_dir.parent().unwrap().join("invoked.timestamp");
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/whatsapp.desc");
    let invoked_time = modified(&invoked);
    let descriptor_time = modified(&descriptor);
    let source_time = modified(&source);
    assert!(
        descriptor_time <= invoked_time || source_time > invoked_time,
        "{} ({descriptor_time:?}) is newer than {} ({invoked_time:?}) while {} ({source_time:?}) is not; the descriptor can cause the next build to rerun build.rs",
        descriptor.display(),
        invoked.display(),
        source.display()
    );
}

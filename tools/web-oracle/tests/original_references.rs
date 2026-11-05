//! Explicit original-reference executions, never part of ordinary offline tests.
use std::{path::PathBuf, process::Command};
fn repo() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}
fn run(args: &[&std::ffi::OsStr]) {
    let output = Command::new(env!("CARGO_BIN_EXE_web-oracle"))
        .args(args)
        .current_dir(repo())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
#[test]
#[ignore = "requires pinned upstream whatspec checkout and Node with local inspector"]
fn all_group_payloads_reads_warnings_and_nulls_match() {
    let root = std::env::var_os("WEB_GROUP_WHATSPEC").expect("WEB_GROUP_WHATSPEC");
    run(&[
        "group".as_ref(),
        &root,
        "--check".as_ref(),
        "tools/whatspec-codegen/fixtures/group_normalization_oracle.json".as_ref(),
    ]);
}
#[test]
#[ignore = "requires restored complete locked WAM reference set"]
fn every_wam_vector_matches_the_independent_node_baseline() {
    let root = std::env::var_os("WEB_WAM_BUNDLES").expect("WEB_WAM_BUNDLES");
    let lock = std::env::var_os("WEB_WAM_LOCK").expect("WEB_WAM_LOCK");
    run(&[
        "wam".as_ref(),
        &root,
        "--lock".as_ref(),
        &lock,
        "--check".as_ref(),
        "tools/web-oracle/fixtures/wam-vectors.json".as_ref(),
    ]);
}
#[test]
#[ignore = "requires restored complete locked retry metadata reference set"]
fn original_metadata_matches_all_existing_fixtures() {
    let root = std::env::var_os("WEB_RETRY_BUNDLES").expect("WEB_RETRY_BUNDLES");
    let lock = std::env::var_os("WEB_RETRY_LOCK").expect("WEB_RETRY_LOCK");
    run(&[
        "retry".as_ref(),
        &root,
        "--lock".as_ref(),
        &lock,
        "--check".as_ref(),
        "wacore/tests/fixtures/retry_metadata.json".as_ref(),
    ]);
}

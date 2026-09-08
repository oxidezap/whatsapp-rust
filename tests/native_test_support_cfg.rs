//! Compile the actual fixture-path cfg guards without the root's native-only dev dependency graph.
#![cfg(not(target_arch = "wasm32"))]

use std::io::Write;
use std::path::Path;
use std::process::{Command, Output, Stdio};

fn imported_name(tree: &syn::UseTree, name: &str) -> bool {
    match tree {
        syn::UseTree::Path(path) => imported_name(&path.tree, name),
        syn::UseTree::Name(leaf) => leaf.ident == name,
        _ => false,
    }
}

fn guards(path: &str, names: &[&str]) -> String {
    let source = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(path)).unwrap();
    let parsed = syn::parse_file(&source).unwrap();
    let mut attributes = parsed.attrs;
    let mut items = parsed.items;
    for name in names {
        let item = items
            .into_iter()
            .find(|item| match item {
                syn::Item::Mod(item) => item.ident == name,
                syn::Item::Fn(item) => item.sig.ident == name,
                syn::Item::Use(item) => imported_name(&item.tree, name),
                _ => false,
            })
            .unwrap_or_else(|| panic!("missing {path}::{name}"));
        items = match item {
            syn::Item::Mod(item) => {
                attributes.extend(item.attrs);
                item.content.map_or_else(Vec::new, |(_, items)| items)
            }
            syn::Item::Fn(item) => {
                attributes.extend(item.attrs);
                Vec::new()
            }
            syn::Item::Use(item) => {
                attributes.extend(item.attrs);
                Vec::new()
            }
            _ => unreachable!(),
        };
    }
    let clauses: Vec<_> = attributes
        .iter()
        .filter(|attr| attr.path().is_ident("cfg"))
        .map(|attr| attr.meta.require_list().unwrap().tokens.to_string())
        .collect();
    assert!(!clauses.is_empty(), "{path}::{names:?} has no cfg guard");
    format!("all({})", clauses.join(","))
}

fn compile(source: &str, wasm: bool, test: bool, feature: bool) -> Output {
    let mut command = Command::new("rustc");
    command.args([
        "--edition=2024",
        "--crate-name=native_fixture_cfg_probe",
        "--crate-type=lib",
        "--emit=metadata",
        "-o",
        "-",
        "-",
    ]);
    if wasm {
        command.args(["--target", "wasm32-unknown-unknown"]);
    }
    if test {
        command.args(["--cfg", "test"]);
    }
    if feature {
        command.args(["--cfg", "feature=\"test-support\""]);
    }
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("rustc");
    child
        .stdin
        .take()
        .unwrap()
        .write_all(source.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

#[test]
#[ignore = "requires an installed wasm32-unknown-unknown Rust target; checks cfg reachability, not the full WASM test graph"]
fn native_fixture_paths_are_excluded_even_with_wasm_test_and_feature_cfgs() {
    let paths: &[(&str, &[&str])] = &[
        ("src/lib.rs", &["test_support"]),
        ("src/test_utils.rs", &["seed_peer_session"]),
        ("src/client/adapters.rs", &["tests"]),
        ("src/voip/facade.rs", &["tests"]),
        ("src/features/signal.rs", &["tests", "seed_peer_session"]),
        (
            "src/features/signal.rs",
            &["tests", "participant_fanout_reuses_durable_session_leases"],
        ),
        (
            "src/retry.rs",
            &[
                "tests",
                "public_retransmission_recaches_the_supplied_message",
            ],
        ),
        (
            "src/send/group_repair.rs",
            &[
                "tests",
                "primary_identity_change_is_shared_by_pn_and_lid_aliases",
            ],
        ),
        (
            "src/send/group_repair.rs",
            &[
                "tests",
                "historical_repair_emits_only_for_a_continuous_account",
            ],
        ),
        ("src/send/mod.rs", &["tests"]),
        ("src/send/mod.rs", &["clock_budget_tests"]),
        ("tests/voip_call_fixture.rs", &[]),
    ];
    let mut source = String::from("#![no_std]\n");
    for (index, (path, names)) in paths.iter().enumerate() {
        source.push_str(&format!(
            "#[cfg({})] compile_error!(\"NATIVE_FIXTURE_{index:02}\");\n",
            guards(path, names)
        ));
    }
    let native = compile(&source, false, true, true);
    assert!(!native.status.success());
    let errors = String::from_utf8(native.stderr).unwrap();
    for index in 0..paths.len() {
        assert!(
            errors.contains(&format!("NATIVE_FIXTURE_{index:02}")),
            "missing native positive control: {errors}"
        );
    }
    for test in [false, true] {
        for feature in [false, true] {
            let result = compile(&source, true, test, feature);
            assert!(
                result.status.success(),
                "wasm test={test} feature={feature}: {}",
                String::from_utf8_lossy(&result.stderr)
            );
        }
    }
}

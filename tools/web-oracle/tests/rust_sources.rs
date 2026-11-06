//! Guard the user's repository-owned Rust source policy (schemas/config are data).
use std::{path::Path, process::Command};
#[test]
fn tracked_program_sources_are_rust() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let output = Command::new("git")
        .args(["ls-files", "-z"])
        .current_dir(&root)
        .output()
        .unwrap();
    assert!(output.status.success());
    let forbidden = [
        "java", "kt", "kts", "c", "h", "cc", "cpp", "hpp", "js", "jsx", "mjs", "cjs", "ts", "tsx",
        "py", "sh", "bash", "ps1", "rb", "go", "swift", "cs", "php", "lua", "pl", "ex", "exs",
        "scala", "dart", "html", "css",
    ];
    let paths = std::str::from_utf8(&output.stdout).unwrap();
    let offenders: Vec<_> = paths
        .split('\0')
        .filter(|p| !p.is_empty())
        .filter(|p| {
            root.join(p).is_file()
                && Path::new(p)
                    .extension()
                    .and_then(|e| e.to_str())
                    .is_some_and(|e| forbidden.contains(&e))
        })
        .collect();
    assert!(
        offenders.is_empty(),
        "repository-owned program sources must be Rust: {offenders:?}"
    );
}

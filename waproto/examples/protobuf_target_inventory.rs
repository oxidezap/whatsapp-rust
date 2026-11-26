#![forbid(unsafe_code)]

use std::collections::{BTreeMap, HashSet};
use std::io::Write as _;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt as _;
use std::path::Path;

use serde::Serialize;

#[derive(Default, Serialize)]
struct Totals {
    files: u64,
    apparent_bytes: u64,
    reported_block_bytes: u64,
}

fn visit(
    root: &Path,
    path: &Path,
    totals: &mut BTreeMap<String, Totals>,
    seen: &mut HashSet<(u64, u64)>,
) -> std::io::Result<()> {
    let mut entries = std::fs::read_dir(path)?.collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let kind = entry.file_type()?;
        if kind.is_dir() {
            visit(root, &entry.path(), totals, seen)?;
        } else if kind.is_file() {
            let path = entry.path();
            let relative = path.strip_prefix(root).expect("entry below inventory root");
            let category = relative
                .components()
                .take(2)
                .collect::<std::path::PathBuf>();
            let category = category.to_string_lossy().into_owned();
            let metadata = entry.metadata()?;
            #[cfg(unix)]
            if !seen.insert((metadata.dev(), metadata.ino())) {
                continue;
            }
            #[cfg(unix)]
            let blocks = metadata.blocks() * 512;
            #[cfg(not(unix))]
            let blocks = 0;
            for key in ["total", &category] {
                let total = totals.entry(key.into()).or_default();
                total.files += 1;
                total.apparent_bytes += metadata.len();
                total.reported_block_bytes += blocks;
            }
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with("libwaproto-") || name.starts_with("libwaproto.") {
                let total = totals
                    .entry("waproto-library-artifacts".into())
                    .or_default();
                total.files += 1;
                total.apparent_bytes += metadata.len();
                total.reported_block_bytes += blocks;
            }
        }
    }
    Ok(())
}

fn main() -> std::io::Result<()> {
    let root = std::env::args()
        .nth(1)
        .expect("expected the Cargo-reported target directory");
    let root = std::fs::canonicalize(root)?;
    let mut totals = BTreeMap::new();
    visit(&root, &root, &mut totals, &mut HashSet::new())?;
    writeln!(
        std::io::stdout().lock(),
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "target": root,
            "block_accounting": "sum of st_blocks, not exclusive disk usage; reflinks may share extents",
            "symlinks": "not followed below the inventory root",
        "hardlinks_deduplicated": cfg!(unix),
            "totals": totals,
        }))?
    )?;
    Ok(())
}

//! Verify the complete locked bundle multiset before executing any module.
use anyhow::{Context, Result, ensure};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    io::Read,
    path::{Path, PathBuf},
};
pub fn sha(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}
pub struct Bundle {
    pub path: PathBuf,
    pub bytes: Vec<u8>,
}
pub fn verify(root: &Path, lock_path: &Path) -> Result<Vec<Bundle>> {
    let lock: Value = serde_json::from_slice(&std::fs::read(lock_path)?)?;
    let bundles = lock["bundles"].as_array().context("bundle lock entries")?;
    ensure!(
        !bundles.is_empty() && bundles.len() <= 10000,
        "lock count budget"
    );
    ensure!(
        lock["bundleCount"].as_u64() == Some(bundles.len() as u64),
        "lock count"
    );
    let mut expected = BTreeMap::new();
    let mut lines = vec![];
    for entry in bundles {
        let hash = entry["sha256"].as_str().context("bundle hash")?;
        let size = entry["size"].as_u64().context("bundle size")?;
        ensure!(
            hash.len() == 64
                && hash
                    .bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
            "invalid digest"
        );
        *expected.entry((hash.to_owned(), size)).or_insert(0_usize) += 1;
        lines.push(format!("{hash}:{size}"));
    }
    lines.sort();
    ensure!(
        lock["setHash"] == sha(lines.join("\n").as_bytes()),
        "lock set fingerprint"
    );
    let mut paths = vec![];
    let mut actual = BTreeMap::new();
    let mut total = 0_u64;
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_none_or(|s| s != "js") {
            continue;
        }
        ensure!(
            entry.file_type()?.is_file(),
            "ordinary bundle files required"
        );
        let size = entry.metadata()?.len();
        total += size;
        ensure!(
            size <= 64 * 1024 * 1024 && total <= 1024 * 1024 * 1024,
            "bundle size budget"
        );
        let mut bytes = vec![];
        std::fs::File::open(&path)?
            .take(64 * 1024 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        ensure!(
            bytes.len() as u64 == size && paths.len() < bundles.len(),
            "bundle changed/count budget"
        );
        *actual
            .entry((sha(&bytes), bytes.len() as u64))
            .or_insert(0_usize) += 1;
        paths.push(Bundle { path, bytes });
    }
    ensure!(actual == expected, "bundle multiset differs from lock");
    paths.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(paths)
}

/// AST-delimited original module calls, with duplicate bodies checked exactly.
pub fn modules(paths: &[Bundle], names: &[&str]) -> Result<BTreeMap<String, Vec<u8>>> {
    use oxc_ast::ast::CallExpression;
    use oxc_ast_visit::{Visit, walk};
    use oxc_span::SourceType;
    struct Calls<'n> {
        names: &'n [&'n str],
        spans: Vec<(String, u32, u32)>,
    }
    impl<'a> Visit<'a> for Calls<'_> {
        fn visit_call_expression(&mut self, call: &CallExpression<'a>) {
            if call.callee.is_specific_id("__d")
                && let Some(oxc_ast::ast::Argument::StringLiteral(name)) = call.arguments.first()
                && self.names.contains(&name.value.as_str())
            {
                self.spans
                    .push((name.value.to_string(), call.span.start, call.span.end));
            }
            walk::walk_call_expression(self, call);
        }
    }
    let mut output = BTreeMap::new();
    for bundle in paths {
        let bytes = &bundle.bytes;
        if !names.iter().any(|name| {
            bytes
                .windows(name.len())
                .any(|window| window == name.as_bytes())
        }) {
            continue;
        }
        let text = std::str::from_utf8(bytes)?;
        let allocator = oxc_allocator::Allocator::default();
        let parsed = oxc_parser::Parser::new(&allocator, text, SourceType::default()).parse();
        ensure!(
            parsed.diagnostics.is_empty(),
            "original bundle AST parse failed: {}",
            bundle.path.display()
        );
        let mut calls = Calls {
            names,
            spans: vec![],
        };
        calls.visit_program(&parsed.program);
        for (name, start, end) in calls.spans {
            let source = bytes[start as usize..end as usize].to_vec();
            if let Some(old) = output.get(&name) {
                ensure!(*old == source, "conflicting original definitions: {name}");
            } else {
                output.insert(name, source);
            }
        }
    }
    ensure!(
        output.len() == names.len(),
        "missing requested original modules"
    );
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn modules_rejects_parser_diagnostics_before_extracting_calls() {
        let valid = br#"__d("Fixture", [], function() {});"#;
        let bundle = |bytes: &[u8]| Bundle {
            path: PathBuf::from("fixture.js"),
            bytes: bytes.to_vec(),
        };
        let extracted = modules(&[bundle(valid)], &["Fixture"]).unwrap();
        assert_eq!(extracted["Fixture"], &valid[..valid.len() - 1]);

        let invalid = br#"__d("Fixture", [], function() {}); let = ;"#;
        let error = modules(&[bundle(invalid)], &["Fixture"]).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("original bundle AST parse failed")
        );
    }

    #[test]
    fn changed_missing_extra_and_repeated_inputs_fail_before_execution() {
        let temp = tempfile::tempdir().unwrap();
        let bundles = temp.path().join("inputs");
        std::fs::create_dir(&bundles).unwrap();
        let file = bundles.join("reference.js");
        // Opaque bytes are sufficient here: no program is executed by verify.
        let bytes = b"reference fixture";
        let hash = sha(bytes);
        let lock = temp.path().join("lock.json");
        std::fs::write(
            &lock,
            serde_json::to_vec(&json!({
                "bundleCount":1,"setHash":sha(format!("{hash}:{}",bytes.len()).as_bytes()),
                "bundles":[{"sha256":hash,"size":bytes.len()}]
            }))
            .unwrap(),
        )
        .unwrap();
        std::fs::write(&file, bytes).unwrap();
        assert_eq!(verify(&bundles, &lock).unwrap().len(), 1);
        std::fs::write(&file, b"changed reference").unwrap();
        assert!(verify(&bundles, &lock).is_err());
        std::fs::remove_file(&file).unwrap();
        assert!(verify(&bundles, &lock).is_err());
        std::fs::write(&file, bytes).unwrap();
        let extra = bundles.join("duplicate.js");
        std::fs::write(&extra, bytes).unwrap();
        assert!(verify(&bundles, &lock).is_err());
        std::fs::remove_file(extra).unwrap();
        let mut corrupt: Value = serde_json::from_slice(&std::fs::read(&lock).unwrap()).unwrap();
        corrupt["setHash"] = json!("0".repeat(64));
        std::fs::write(&lock, serde_json::to_vec(&corrupt).unwrap()).unwrap();
        assert!(verify(&bundles, &lock).is_err());
    }
}

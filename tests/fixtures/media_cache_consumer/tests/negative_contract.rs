//! Controls reject only the intended downstream API use, not unrelated errors.
#![cfg(not(target_arch = "wasm32"))]
use std::process::Command;

#[test]
fn old_literals_and_exhaustive_matches_fail_precisely() {
    let cases = [
        ("old-image-literal", "E0639", "ImageOptions"),
        ("old-video-literal", "E0639", "VideoOptions"),
        ("old-document-literal", "E0639", "DocumentOptions"),
        ("old-audio-literal", "E0639", "AudioOptions"),
        ("old-catalog-literal", "E0639", "CatalogOptions"),
        ("old-collection-literal", "E0639", "CollectionOptions"),
        ("old-cache-config-literal", "E0639", "CacheConfig"),
        ("old-cache-entry-literal", "E0639", "CacheEntryConfig"),
        ("old-cache-stores-literal", "E0639", "CacheStores"),
        (
            "exhaustive-product-availability",
            "E0004",
            "ProductAvailability",
        ),
    ];
    // Keep nested Cargo independent of the running test's artifact directory.
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("target"))
        .join("negative-contract");
    for (feature, code, name) in cases {
        let output = Command::new(env!("CARGO"))
            .current_dir(env!("CARGO_MANIFEST_DIR"))
            .env("CARGO_TARGET_DIR", &target)
            .args([
                "check",
                "--offline",
                "--locked",
                "--lib",
                "--no-default-features",
                "--features",
                feature,
                "--message-format=json",
                "-j1",
            ])
            .output()
            .expect("launch negative cargo check");
        assert!(!output.status.success(), "{feature} unexpectedly compiled");
        let errors: Vec<serde_json::Value> = String::from_utf8_lossy(&output.stdout)
            .lines()
            .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
            .filter(|record| {
                record["reason"] == "compiler-message" && record["message"]["level"] == "error"
            })
            .collect();
        assert_eq!(
            errors.len(),
            1,
            "{feature}: {errors:#?}\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let message = &errors[0]["message"];
        assert_eq!(message["code"]["code"], code, "{feature}: {message:#?}");
        assert!(
            message["message"]
                .as_str()
                .unwrap()
                .contains("non-exhaustive"),
            "{message:#?}"
        );
        assert!(
            message["spans"].as_array().unwrap().iter().any(|span| {
                span["is_primary"] == true
                    && span["file_name"]
                        .as_str()
                        .is_some_and(|file| file.ends_with("src/negative.rs"))
            }),
            "{feature}: primary error must point to negative.rs: {message:#?}"
        );
        assert!(
            message["rendered"].as_str().unwrap().contains(name),
            "{feature}: diagnostic must identify {name}: {message:#?}"
        );
    }
}

//! Explicit, version-bound APK material recipes. No inferred logo, ABI or signer.
use crate::{ApkLock, ArtifactKind};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::Read;
use std::path::Path;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MaterialRecipe {
    pub schema_version: u32,
    pub lock_sha256: String,
    pub manifest: Member,
    pub dex: Member,
    pub logo: Member,
    #[serde(default)]
    pub signatures: Vec<Member>,
    #[serde(default)]
    pub signing_block: Option<SigningSelection>,
    pub salt_hex: String,
    pub iterations: u32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SigningSelection {
    pub scheme: crate::SigningScheme,
    pub platform_sdk: u32,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Member {
    pub apk_path: String,
    pub member: String,
}
#[derive(Serialize)]
pub struct MaterialReport {
    pub schema_version: u32,
    pub lock_sha256: String,
    pub recipe_sha256: String,
    pub qualification: &'static str,
    pub authenticity: &'static str,
    pub protocol_execution: &'static str,
    pub package: String,
    pub version_name: String,
    pub version_code: u64,
    pub dex_md5: String,
    pub components: Vec<Component>,
    pub certificate_sha256: Vec<String>,
    pub secret_exported: bool,
}
/// Material and its provenance are constructed together and cannot be relabelled
/// by pairing independently obtained reports and credential material at export.
pub struct ExtractedMaterial {
    report: MaterialReport,
    material: wacore::mobile::AndroidTokenMaterial,
}
impl ExtractedMaterial {
    pub fn report(&self) -> &MaterialReport {
        &self.report
    }

    pub fn material(&self) -> &wacore::mobile::AndroidTokenMaterial {
        &self.material
    }

    #[cfg(all(test, unix))]
    pub(crate) fn synthetic(
        report: MaterialReport,
        material: wacore::mobile::AndroidTokenMaterial,
    ) -> Self {
        Self { report, material }
    }
}

#[derive(Serialize)]
pub struct Component {
    pub source: Member,
    pub size_bytes: usize,
    pub sha256: String,
}
fn read(
    lock: &ApkLock,
    root: &Path,
    member: &Member,
    limit: u64,
    components: &mut Vec<Component>,
) -> Result<Vec<u8>> {
    let artifact = lock
        .artifacts
        .iter()
        .find(|a| a.path == member.apk_path)
        .context("recipe APK missing from lock")?;
    ensure!(
        matches!(
            artifact.kind,
            ArtifactKind::BaseApk | ArtifactKind::SplitApk
        ),
        "recipe source is not APK"
    );
    let bytes = crate::zip_lineage::read_material_member(
        crate::open_artifact(root, artifact)?,
        &member.member,
        limit,
    )?;
    components.push(Component {
        source: Member {
            apk_path: member.apk_path.clone(),
            member: member.member.clone(),
        },
        size_bytes: bytes.len(),
        sha256: hex::encode(Sha256::digest(&bytes)),
    });
    Ok(bytes)
}
/// Parse explicit APK members and derive registration material. This verifies byte
/// ancestry, not signer authenticity or acceptance by a mobile registration server.
/// Secrets stay in memory; the report contains only metadata and source hashes.
pub fn extract_material(
    lock_path: &Path,
    root: &Path,
    recipe_path: &Path,
) -> Result<ExtractedMaterial> {
    let verified = crate::verify(lock_path, root)?;
    extract_verified_material(lock_path, root, recipe_path, &verified.lock_sha256)
}

fn extract_verified_material(
    lock_path: &Path,
    root: &Path,
    recipe_path: &Path,
    verified_lock_sha256: &str,
) -> Result<ExtractedMaterial> {
    let (lock, lock_sha256) = crate::read_lock(lock_path)?;
    ensure!(
        lock_sha256 == verified_lock_sha256,
        "lock changed after verification"
    );
    let root = root.canonicalize()?;
    let mut recipe_bytes = Vec::new();
    std::fs::File::open(recipe_path)?
        .take(64 * 1024 + 1)
        .read_to_end(&mut recipe_bytes)?;
    ensure!(recipe_bytes.len() <= 64 * 1024, "recipe budget exceeded");
    let recipe: MaterialRecipe = serde_json::from_slice(&recipe_bytes)?;
    ensure!(
        recipe.schema_version == 1 && recipe.lock_sha256 == verified_lock_sha256,
        "recipe/lock binding mismatch"
    );
    ensure!(
        recipe.manifest.member == "AndroidManifest.xml" && recipe.dex.member == "classes.dex",
        "unexpected base material members"
    );
    let base = lock
        .artifacts
        .iter()
        .find(|a| a.path == recipe.manifest.apk_path && a.kind == ArtifactKind::BaseApk)
        .context("manifest must use base APK")?;
    ensure!(
        recipe.dex.apk_path == base.path,
        "DEX must use same base APK"
    );
    ensure!(
        recipe.signatures.len() <= 32
            && (recipe.signatures.is_empty() != recipe.signing_block.is_none()),
        "select exactly one of ordered v1 signature blocks or explicit v2/v3/v3.1 scheme"
    );
    let mut components = Vec::new();
    let manifest = crate::apk_manifest::parse(&read(
        &lock,
        &root,
        &recipe.manifest,
        8 * 1024 * 1024,
        &mut components,
    )?)?;
    ensure!(
        manifest.package == lock.package_name
            && manifest.version_name == lock.version_name
            && manifest.version_code == lock.version_code,
        "manifest differs from version lock"
    );
    ensure!(
        manifest.min_sdk.is_none_or(|v| v == lock.min_sdk)
            && manifest.target_sdk.is_none_or(|v| v == lock.target_sdk),
        "manifest SDK differs from lock"
    );
    let mut md5 = wacore::crypto::Md5::new();
    let mut dex_sha = Sha256::new();
    let mut dex_size = 0;
    crate::zip_lineage::stream_material_member(
        crate::open_artifact(&root, base)?,
        &recipe.dex.member,
        256 * 1024 * 1024,
        |chunk| {
            md5.consume(chunk);
            dex_sha.update(chunk);
            dex_size += chunk.len();
        },
    )?;
    let dex_md5 = md5.finalize();
    components.push(Component {
        source: Member {
            apk_path: recipe.dex.apk_path.clone(),
            member: recipe.dex.member.clone(),
        },
        size_bytes: dex_size,
        sha256: hex::encode(dex_sha.finalize()),
    });
    let logo = read(&lock, &root, &recipe.logo, 8 * 1024 * 1024, &mut components)?;
    let mut certificates = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for signature in &recipe.signatures {
        let upper = signature.member.to_ascii_uppercase();
        ensure!(
            signature.apk_path == base.path
                && upper.starts_with("META-INF/")
                && !upper[9..].contains('/')
                && [".RSA", ".DSA", ".EC"]
                    .iter()
                    .any(|suffix| upper.ends_with(suffix))
                && seen.insert(&signature.member),
            "invalid or duplicate v1 block selection"
        );
        certificates.extend(crate::apk_der::certificates(&read(
            &lock,
            &root,
            signature,
            4 * 1024 * 1024,
            &mut components,
        )?)?);
        ensure!(
            certificates.len() <= 32
                && certificates.iter().map(Vec::len).sum::<usize>() <= 1024 * 1024,
            "aggregate certificate budget exceeded"
        );
    }
    if let Some(selection) = &recipe.signing_block {
        ensure!(
            (1..=10_000).contains(&selection.platform_sdk),
            "invalid explicit platform SDK"
        );
        let block = crate::zip_lineage::read_signing_block(crate::open_artifact(&root, base)?)?;
        certificates =
            crate::apk_signing::certificates(&block, selection.scheme, selection.platform_sdk)?;
        components.push(Component {
            source: Member {
                apk_path: base.path.clone(),
                member: format!(
                    "<APK signing block {:?} SDK {}>",
                    selection.scheme, selection.platform_sdk
                ),
            },
            size_bytes: block.len(),
            sha256: hex::encode(Sha256::digest(&block)),
        });
    }
    let certificate_sha256 = certificates
        .iter()
        .map(|c| hex::encode(Sha256::digest(c)))
        .collect();
    let salt = hex::decode(&recipe.salt_hex).context("invalid recipe salt")?;
    let secret = wacore::mobile::AndroidTokenMaterial::derive_secret(
        &manifest.package,
        &logo,
        &salt,
        recipe.iterations,
    )?;
    let material =
        wacore::mobile::AndroidTokenMaterial::from_derived(*secret, dex_md5, certificates)?;
    Ok(ExtractedMaterial {
        report: MaterialReport {
            schema_version: 1,
            lock_sha256,
            recipe_sha256: hex::encode(Sha256::digest(&recipe_bytes)),
            qualification: "pinned-reference-derived; not APK-oracle qualified",
            authenticity: "requires independent apksigner review; certificate extraction is not signature verification",
            protocol_execution: "not_run",
            package: manifest.package,
            version_name: manifest.version_name,
            version_code: manifest.version_code,
            dex_md5: hex::encode(dex_md5),
            components,
            certificate_sha256,
            secret_exported: false,
        },
        material,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::{
        fs,
        io::{Cursor, Write},
    };
    fn fixture() -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        for (name, data) in [
            ("AndroidManifest.xml", crate::apk_manifest::tests::fixture()),
            ("classes.dex", b"synthetic-not-dex".to_vec()),
            ("logo.png", vec![0, 255, 7]),
            (
                "META-INF/SYNTHETIC.RSA",
                crate::apk_der::tests::fixture(&[0x30, 1, 1]),
            ),
        ] {
            zip.start_file(
                name,
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
            zip.write_all(&data).unwrap();
        }
        let apk = zip.finish().unwrap().into_inner();
        fs::write(dir.path().join("base.apk"), &apk).unwrap();
        let signature = b"synthetic, no trusted signer";
        fs::write(dir.path().join("signature.txt"), signature).unwrap();
        let lock = json!({"schema_version":1,"package_name":"com.whatsapp","version_name":"2.0","version_code":42,"abi":"arm64-v8a","min_sdk":23,"target_sdk":35,"distribution_channel":"synthetic","acquisition":{"source_url":"https://example.invalid/synthetic","acquired_at_utc":"2026-11-04T22:00:00Z","access_conditions":"synthetic only"},"signature":{"signer_certificate_sha256":"00".repeat(32),"trusted_signer_reference":"synthetic not trusted","verification_report_path":"signature.txt","verifier_version":"synthetic"},"declared_installation_complete":true,"artifacts":[{"path":"base.apk","kind":"base_apk","size_bytes":apk.len(),"sha256":hex::encode(Sha256::digest(&apk))},{"path":"signature.txt","kind":"signature_report","size_bytes":signature.len(),"sha256":hex::encode(Sha256::digest(signature))}]});
        let bytes = serde_json::to_vec(&lock).unwrap();
        let path = dir.path().join("lock.json");
        fs::write(&path, &bytes).unwrap();
        let recipe = json!({"schema_version":1,"lock_sha256":hex::encode(Sha256::digest(&bytes)),"manifest":{"apk_path":"base.apk","member":"AndroidManifest.xml"},"dex":{"apk_path":"base.apk","member":"classes.dex"},"logo":{"apk_path":"base.apk","member":"logo.png"},"signatures":[{"apk_path":"base.apk","member":"META-INF/SYNTHETIC.RSA"}],"salt_hex":"090002","iterations":128});
        let recipe_path = dir.path().join("recipe.json");
        fs::write(&recipe_path, serde_json::to_vec(&recipe).unwrap()).unwrap();
        (dir, path, recipe_path)
    }

    #[test]
    fn synthetic_archive_material_is_version_bound_and_redacted() {
        let (dir, path, recipe_path) = fixture();
        let extracted = extract_material(&path, dir.path(), &recipe_path).unwrap();
        let report = extracted.report();
        let material = extracted.material();
        assert_eq!(report.version_code, 42);
        assert_eq!(report.components.len(), 4);
        assert!(material.token_base64("01234").is_ok());
        let json = serde_json::to_string(report).unwrap();
        assert!(!json.contains("090002"));
        assert!(!json.contains("secret_key"));
        assert!(json.contains("not_run"));
        let mut recipe: serde_json::Value =
            serde_json::from_slice(&fs::read(&recipe_path).unwrap()).unwrap();
        recipe["lock_sha256"] = json!("00".repeat(32));
        fs::write(&recipe_path, serde_json::to_vec(&recipe).unwrap()).unwrap();
        assert!(extract_material(&path, dir.path(), &recipe_path).is_err());
    }

    #[test]
    fn changed_lock_is_rejected_before_material_extraction() {
        let (dir, path, recipe_path) = fixture();
        let verified = crate::verify(&path, dir.path()).unwrap();
        let mut lock: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        lock["distribution_channel"] = json!("changed-after-verification");
        fs::write(&path, serde_json::to_vec(&lock).unwrap()).unwrap();
        let Err(error) =
            extract_verified_material(&path, dir.path(), &recipe_path, &verified.lock_sha256)
        else {
            panic!("changed lock must fail");
        };
        assert!(
            error
                .to_string()
                .contains("lock changed after verification")
        );
    }

    #[cfg(unix)]
    #[test]
    fn each_export_retains_its_extraction_provenance() {
        let (dir, path, recipe_path) = fixture();
        assert!(
            std::process::Command::new("git")
                .args(["init", "--quiet"])
                .arg(dir.path())
                .status()
                .unwrap()
                .success()
        );
        fs::write(dir.path().join(".gitignore"), "/.cache/\n").unwrap();
        let first = extract_material(&path, dir.path(), &recipe_path).unwrap();
        let mut recipe: serde_json::Value =
            serde_json::from_slice(&fs::read(&recipe_path).unwrap()).unwrap();
        recipe["salt_hex"] = json!("090003");
        fs::write(&recipe_path, serde_json::to_vec(&recipe).unwrap()).unwrap();
        let second = extract_material(&path, dir.path(), &recipe_path).unwrap();
        assert_ne!(
            first.material().token_base64("01234").unwrap(),
            second.material().token_base64("01234").unwrap()
        );
        for extracted in [&first, &second] {
            let receipt = crate::export_private_material(dir.path(), extracted).unwrap();
            let restored = crate::import_private_material(
                &receipt.path,
                &receipt.sha256,
                &extracted.report().lock_sha256,
                &extracted.report().recipe_sha256,
            )
            .unwrap();
            assert_eq!(
                restored.token_base64("01234").unwrap(),
                extracted.material().token_base64("01234").unwrap()
            );
            let other = if std::ptr::eq(extracted, &first) {
                &second
            } else {
                &first
            };
            assert!(
                crate::import_private_material(
                    &receipt.path,
                    &receipt.sha256,
                    &other.report().lock_sha256,
                    &other.report().recipe_sha256,
                )
                .is_err()
            );
        }
    }
}

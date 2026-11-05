//! Opt-in private cache export. No default secret persistence or stdout output.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use zeroize::{Zeroize, Zeroizing};
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PrivateMaterial {
    schema_version: u32,
    lock_sha256: String,
    recipe_sha256: String,
    secret_hex: String,
    dex_md5_hex: String,
    certificates_hex: Vec<String>,
}
impl Drop for PrivateMaterial {
    fn drop(&mut self) {
        self.secret_hex.zeroize();
    }
}
#[derive(Serialize)]
pub struct PrivateExport {
    pub path: PathBuf,
    pub sha256: String,
}
fn hash(value: &str) -> Result<()> {
    ensure!(
        value.len() == 64
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "invalid provenance digest"
    );
    Ok(())
}
#[cfg(unix)]
fn private_file(path: &Path) -> Result<fs::File> {
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let metadata = fs::symlink_metadata(path)?;
    ensure!(
        metadata.is_file()
            && !metadata.file_type().is_symlink()
            && metadata.permissions().mode() & 0o077 == 0,
        "material file must be private regular file"
    );
    Ok(OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW)
        .open(path)?)
}
#[cfg(not(unix))]
fn private_file(_path: &Path) -> Result<fs::File> {
    anyhow::bail!("private material files require Unix permission enforcement")
}
/// Explicit export under the repository's ignored `.cache/mobile-oracle/private`.
/// Existing files are never overwritten; returned metadata contains no secret.
#[cfg(unix)]
pub fn export_private_material(
    repository: &Path,
    report: &crate::MaterialReport,
    material: &wacore::mobile::AndroidTokenMaterial,
) -> Result<PrivateExport> {
    use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt};
    hash(&report.lock_sha256)?;
    hash(&report.recipe_sha256)?;
    let root = repository.canonicalize()?;
    ensure!(
        fs::read_to_string(root.join(".gitignore"))?
            .lines()
            .any(|line| line.trim() == "/.cache/"),
        "repository must ignore /.cache/"
    );
    let mut directory = root;
    for name in [".cache", "mobile-oracle", "private"] {
        directory.push(name);
        match fs::DirBuilder::new().mode(0o700).create(&directory) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.into()),
        }
        let meta = fs::symlink_metadata(&directory)?;
        ensure!(
            meta.is_dir() && !meta.file_type().is_symlink(),
            "private cache directory cannot be symlink"
        );
        if name == "private" {
            ensure!(
                meta.permissions().mode() & 0o077 == 0,
                "private cache directory permissions too broad"
            );
        }
    }
    let path = directory.join(format!(
        "{}-{}.json",
        report.lock_sha256, report.recipe_sha256
    ));
    let (secret, dex, certificates) = material.export_private_parts();
    let envelope = PrivateMaterial {
        schema_version: 1,
        lock_sha256: report.lock_sha256.clone(),
        recipe_sha256: report.recipe_sha256.clone(),
        secret_hex: hex::encode(*secret),
        dex_md5_hex: hex::encode(dex),
        certificates_hex: certificates.iter().map(hex::encode).collect(),
    };
    let bytes = Zeroizing::new(serde_json::to_vec(&envelope)?);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(&path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    Ok(PrivateExport {
        path,
        sha256: hex::encode(Sha256::digest(&*bytes)),
    })
}
#[cfg(not(unix))]
pub fn export_private_material(
    _repository: &Path,
    _report: &crate::MaterialReport,
    _material: &wacore::mobile::AndroidTokenMaterial,
) -> Result<PrivateExport> {
    anyhow::bail!("private export requires Unix permission enforcement")
}
/// Import a bounded private envelope only when all expected provenance hashes match.
/// The expected file hash must come from a separately retained export receipt.
pub fn import_private_material(
    path: &Path,
    expected_sha256: &str,
    lock_sha256: &str,
    recipe_sha256: &str,
) -> Result<wacore::mobile::AndroidTokenMaterial> {
    hash(expected_sha256)?;
    hash(lock_sha256)?;
    hash(recipe_sha256)?;
    let mut bytes = Zeroizing::new(Vec::new());
    private_file(path)?
        .take(4 * 1024 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    ensure!(
        bytes.len() <= 4 * 1024 * 1024 && hex::encode(Sha256::digest(&*bytes)) == expected_sha256,
        "private material byte hash mismatch"
    );
    let envelope: PrivateMaterial = serde_json::from_slice(&bytes)?;
    ensure!(
        envelope.schema_version == 1
            && envelope.lock_sha256 == lock_sha256
            && envelope.recipe_sha256 == recipe_sha256,
        "private material provenance mismatch"
    );
    let mut secret = Zeroizing::new([0; 64]);
    hex::decode_to_slice(&envelope.secret_hex, &mut secret[..])
        .context("invalid private key encoding")?;
    let mut dex = [0; 16];
    hex::decode_to_slice(&envelope.dex_md5_hex, &mut dex)?;
    ensure!(
        envelope.certificates_hex.len() <= 32,
        "certificate budget exceeded"
    );
    let certificates = envelope
        .certificates_hex
        .iter()
        .map(hex::decode)
        .collect::<std::result::Result<Vec<_>, _>>()?;
    Ok(wacore::mobile::AndroidTokenMaterial::from_derived(
        *secret,
        dex,
        certificates,
    )?)
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    #[test]
    fn synthetic_private_roundtrip_binding_permissions_and_no_clobber() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(".gitignore"), "/.cache/\n").unwrap();
        let material = wacore::mobile::AndroidTokenMaterial::from_derived(
            [7; 64],
            [3; 16],
            vec![vec![0x30, 1, 1]],
        )
        .unwrap();
        let report = crate::MaterialReport {
            schema_version: 1,
            lock_sha256: "11".repeat(32),
            recipe_sha256: "22".repeat(32),
            qualification: "synthetic",
            authenticity: "not_verified",
            protocol_execution: "not_run",
            package: "com.whatsapp".into(),
            version_name: "synthetic".into(),
            version_code: 1,
            dex_md5: "03".repeat(16),
            components: vec![],
            certificate_sha256: vec![],
            secret_exported: false,
        };
        let receipt = export_private_material(dir.path(), &report, &material).unwrap();
        assert_eq!(
            fs::metadata(&receipt.path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let restored = import_private_material(
            &receipt.path,
            &receipt.sha256,
            &report.lock_sha256,
            &report.recipe_sha256,
        )
        .unwrap();
        assert_eq!(
            restored.token_base64("01234").unwrap(),
            material.token_base64("01234").unwrap()
        );
        assert!(export_private_material(dir.path(), &report, &material).is_err());
        assert!(
            import_private_material(
                &receipt.path,
                &"00".repeat(32),
                &report.lock_sha256,
                &report.recipe_sha256
            )
            .is_err()
        );
        assert!(
            import_private_material(
                &receipt.path,
                &receipt.sha256,
                &"00".repeat(32),
                &report.recipe_sha256
            )
            .is_err()
        );
        let alias = dir.path().join("alias.json");
        symlink(&receipt.path, &alias).unwrap();
        assert!(
            import_private_material(
                &alias,
                &receipt.sha256,
                &report.lock_sha256,
                &report.recipe_sha256
            )
            .is_err()
        );
        fs::set_permissions(&receipt.path, fs::Permissions::from_mode(0o644)).unwrap();
        assert!(
            import_private_material(
                &receipt.path,
                &receipt.sha256,
                &report.lock_sha256,
                &report.recipe_sha256
            )
            .is_err()
        );
    }
}

//! Opt-in private cache export. No default secret persistence or stdout output.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
};
use zeroize::Zeroizing;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PrivateMaterial {
    schema_version: u32,
    lock_sha256: String,
    recipe_sha256: String,
    #[serde(
        serialize_with = "serialize_secret_hex",
        deserialize_with = "deserialize_secret_hex"
    )]
    secret_hex: Zeroizing<String>,
    dex_md5_hex: String,
    certificates_hex: Vec<String>,
}
fn serialize_secret_hex<S: serde::Serializer>(
    secret: &Zeroizing<String>,
    serializer: S,
) -> std::result::Result<S::Ok, S::Error> {
    serializer.serialize_str(secret.as_str())
}

fn deserialize_secret_hex<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Zeroizing<String>, D::Error> {
    // Wrap the decoded field before Serde reads later fields, so an error while
    // constructing the envelope still clears this allocation.
    String::deserialize(deserializer).map(Zeroizing::new)
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
#[cfg(unix)]
fn require_ignored_untracked(repository: &Path, relative: &Path) -> Result<()> {
    let tracked = std::process::Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(["ls-files", "--error-unmatch", "--"])
        .arg(relative)
        .output()
        .context("checking private export index status")?;
    ensure!(
        tracked.status.code() == Some(1),
        "private export path must be untracked in a Git repository"
    );
    // The directory must also be ignored so an in-progress temporary export
    // cannot be staged by an ordinary Git add.
    for ignored_path in [Path::new(".cache/mobile-oracle/private/"), relative] {
        let ignored = std::process::Command::new("git")
            .arg("-C")
            .arg(repository)
            .args(["check-ignore", "--quiet", "--"])
            .arg(ignored_path)
            .output()
            .context("checking private export ignore status")?;
        ensure!(
            ignored.status.success(),
            "Git must ignore the private export directory and path"
        );
    }
    Ok(())
}

/// Explicit export under the repository's ignored `.cache/mobile-oracle/private`.
/// Existing files are never overwritten; returned metadata contains no secret.
#[cfg(unix)]
pub fn export_private_material(
    repository: &Path,
    extracted: &crate::ExtractedMaterial,
) -> Result<PrivateExport> {
    export_private_material_with(repository, extracted, |file, bytes| {
        file.write_all(bytes)?;
        file.sync_all()
    })
}
#[cfg(unix)]
fn export_private_material_with(
    repository: &Path,
    extracted: &crate::ExtractedMaterial,
    write_and_sync: impl FnOnce(&mut fs::File, &[u8]) -> std::io::Result<()>,
) -> Result<PrivateExport> {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    let report = extracted.report();
    hash(&report.lock_sha256)?;
    hash(&report.recipe_sha256)?;
    let root = repository.canonicalize()?;
    let relative = Path::new(".cache/mobile-oracle/private").join(format!(
        "{}-{}.json",
        report.lock_sha256, report.recipe_sha256
    ));
    require_ignored_untracked(&root, &relative)?;
    let mut directory = root.clone();
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
    let path = root.join(relative);
    let (secret, dex, certificates) = extracted.material().export_private_parts();
    let envelope = PrivateMaterial {
        schema_version: 1,
        lock_sha256: report.lock_sha256.clone(),
        recipe_sha256: report.recipe_sha256.clone(),
        secret_hex: Zeroizing::new(hex::encode(*secret)),
        dex_md5_hex: hex::encode(dex),
        certificates_hex: certificates.iter().map(hex::encode).collect(),
    };
    let bytes = Zeroizing::new(serde_json::to_vec(&envelope)?);
    // Keep failed writes private and unpublished. The same-directory no-clobber
    // publication exposes only a complete synced file, and drop removes a failed
    // temporary export so a retry can use the same provenance-bound final path.
    let mut file = tempfile::NamedTempFile::new_in(&directory)?;
    file.as_file()
        .set_permissions(fs::Permissions::from_mode(0o600))?;
    write_and_sync(file.as_file_mut(), &bytes)?;
    file.persist_noclobber(&path).map_err(|error| error.error)?;
    Ok(PrivateExport {
        path,
        sha256: hex::encode(Sha256::digest(&*bytes)),
    })
}
#[cfg(not(unix))]
pub fn export_private_material(
    _repository: &Path,
    _extracted: &crate::ExtractedMaterial,
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
    hex::decode_to_slice(envelope.secret_hex.as_bytes(), &mut secret[..])
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

    fn synthetic_inputs() -> (tempfile::TempDir, crate::ExtractedMaterial) {
        let dir = tempfile::tempdir().unwrap();
        assert!(
            std::process::Command::new("git")
                .args(["init", "--quiet"])
                .arg(dir.path())
                .status()
                .unwrap()
                .success()
        );
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
        (dir, crate::ExtractedMaterial::synthetic(report, material))
    }

    #[test]
    fn failed_partial_export_is_unpublished_and_retry_succeeds() {
        use std::os::unix::fs::PermissionsExt;
        let (dir, extracted) = synthetic_inputs();
        let report = extracted.report();
        let material = extracted.material();
        let private = dir.path().join(".cache/mobile-oracle/private");
        let final_path = private.join(format!(
            "{}-{}.json",
            report.lock_sha256, report.recipe_sha256
        ));
        let Err(error) =
            export_private_material_with(dir.path(), &extracted, |file, bytes| {
                assert_eq!(file.metadata()?.permissions().mode() & 0o777, 0o600);
                file.write_all(&bytes[..37])?;
                assert!(!final_path.exists());
                Err(std::io::Error::from(std::io::ErrorKind::StorageFull))
            })
        else {
            panic!("partial write must fail export");
        };
        assert!(error.downcast_ref::<std::io::Error>().is_some());
        assert!(!final_path.exists());
        assert_eq!(fs::read_dir(&private).unwrap().count(), 0);

        let receipt = export_private_material(dir.path(), &extracted).unwrap();
        assert_eq!(receipt.path, final_path);
        assert_eq!(fs::read_dir(&private).unwrap().count(), 1);
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
    }

    #[test]
    fn synthetic_private_roundtrip_binding_permissions_and_no_clobber() {
        use std::os::unix::fs::{PermissionsExt, symlink};
        let (dir, extracted) = synthetic_inputs();
        let report = extracted.report();
        let material = extracted.material();
        let receipt = export_private_material(dir.path(), &extracted).unwrap();
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
        let original = fs::read(&receipt.path).unwrap();
        assert!(export_private_material(dir.path(), &extracted).is_err());
        assert_eq!(fs::read(&receipt.path).unwrap(), original);
        assert_eq!(
            fs::read_dir(receipt.path.parent().unwrap())
                .unwrap()
                .count(),
            1,
            "failed publication must remove its temporary file"
        );
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

    #[test]
    fn export_rejects_effectively_unignored_path_before_writing() {
        let (dir, extracted) = synthetic_inputs();
        fs::write(dir.path().join(".gitignore"), "/.cache/\n!/.cache/\n").unwrap();
        let Err(error) = export_private_material(dir.path(), &extracted) else {
            panic!("unignored export must fail");
        };
        assert!(error.to_string().contains("Git must ignore"));
        assert!(!dir.path().join(".cache").exists());
    }

    #[test]
    fn export_rejects_ignore_rule_covering_only_the_final_file() {
        let (dir, extracted) = synthetic_inputs();
        let rule = format!(
            "/.cache/mobile-oracle/private/{}-{}.json\n",
            extracted.report().lock_sha256,
            extracted.report().recipe_sha256
        );
        fs::write(dir.path().join(".gitignore"), rule).unwrap();
        assert!(export_private_material(dir.path(), &extracted).is_err());
        assert!(!dir.path().join(".cache").exists());
    }

    #[test]
    fn export_rejects_tracked_destination_even_when_deleted() {
        let (dir, extracted) = synthetic_inputs();
        let relative = Path::new(".cache/mobile-oracle/private").join(format!(
            "{}-{}.json",
            extracted.report().lock_sha256,
            extracted.report().recipe_sha256
        ));
        let path = dir.path().join(&relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(&path, b"synthetic tracked placeholder").unwrap();
        assert!(
            std::process::Command::new("git")
                .arg("-C")
                .arg(dir.path())
                .args(["add", "--force", "--"])
                .arg(&relative)
                .status()
                .unwrap()
                .success()
        );
        fs::remove_file(&path).unwrap();
        let Err(error) = export_private_material(dir.path(), &extracted) else {
            panic!("tracked export must fail");
        };
        assert!(error.to_string().contains("must be untracked"));
        assert!(!path.exists());
        assert_eq!(fs::read_dir(path.parent().unwrap()).unwrap().count(), 0);
    }
}

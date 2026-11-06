//! APK acquisition only. Hash agreement is independent of APK authenticity or execution.
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result, ensure};
use clap::Subcommand;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const MAX_APK_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_LOCK_BYTES: u64 = 16 * 1024;
const SOURCE: &str = "https://www.whatsapp.com/download/android";

#[derive(Subcommand)]
pub enum Task {
    /// Record an explicitly supplied APK's hash and declared version. Does not authenticate it.
    Pin {
        #[arg(long)]
        input: PathBuf,
        /// Operator-declared version; verify separately against APK metadata.
        #[arg(long)]
        version: String,
        #[arg(long)]
        lock: PathBuf,
    },
    /// Restore pinned bytes from an HTTPS URL supplied through an environment variable.
    Fetch {
        #[arg(long)]
        lock: PathBuf,
        /// Name of the variable holding the current direct URL; its value is never logged.
        #[arg(long, default_value = "WHATSAPP_APK_URL")]
        url_env: String,
        #[arg(long)]
        cache: Option<PathBuf>,
    },
    /// Import an existing APK into the content-addressed cache, checking the lock first.
    Import {
        #[arg(long)]
        lock: PathBuf,
        #[arg(long)]
        input: PathBuf,
        #[arg(long)]
        cache: Option<PathBuf>,
    },
    /// Verify cached bytes without network access or APK execution.
    Verify {
        #[arg(long)]
        lock: PathBuf,
        #[arg(long)]
        cache: Option<PathBuf>,
    },
}

/// Acquisition identity, deliberately separate from the authenticated oracle evidence lock.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct DownloadLock {
    schema_version: u32,
    declared_version: String,
    source_page: String,
    size_bytes: u64,
    sha256: String,
}

impl DownloadLock {
    fn validate(&self) -> Result<()> {
        ensure!(
            self.schema_version == 1,
            "unsupported APK download lock schema"
        );
        ensure!(
            !self.declared_version.is_empty()
                && self.declared_version.len() <= 128
                && self
                    .declared_version
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._+-".contains(&b)),
            "invalid declared APK version"
        );
        ensure!(
            self.source_page == SOURCE,
            "unexpected official source page"
        );
        ensure!(
            (4..=MAX_APK_BYTES).contains(&self.size_bytes),
            "invalid APK size"
        );
        ensure!(
            self.sha256.len() == 64
                && self
                    .sha256
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "invalid APK SHA-256"
        );
        Ok(())
    }

    fn read(path: &Path) -> Result<Self> {
        let bytes = read_bounded(
            File::open(path).context("opening APK download lock")?,
            MAX_LOCK_BYTES,
        )?;
        let lock: Self = serde_json::from_slice(&bytes).context("parsing APK download lock")?;
        lock.validate()?;
        Ok(lock)
    }

    fn cached_path(&self, cache: &Path) -> PathBuf {
        cache.join(&self.sha256).join("WhatsApp.apk")
    }
}

pub fn run(root: &Path, task: Task) -> Result<()> {
    match task {
        Task::Pin {
            input,
            version,
            lock,
        } => {
            let (size_bytes, sha256) = copy_digest(
                File::open(input).context("opening supplied APK")?,
                &mut std::io::sink(),
                MAX_APK_BYTES,
            )?;
            let identity = DownloadLock {
                schema_version: 1,
                declared_version: version,
                source_page: SOURCE.into(),
                size_bytes,
                sha256,
            };
            identity.validate()?;
            // A changed input must be an explicit new lock/review, never an implicit refresh.
            ensure!(
                !lock.exists(),
                "lock already exists; select a new lock path for a new version"
            );
            let parent = lock
                .parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new("."));
            fs::create_dir_all(parent)?;
            let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
            serde_json::to_writer_pretty(&mut temporary, &identity)?;
            temporary.write_all(b"\n")?;
            temporary
                .persist_noclobber(&lock)
                .context("publishing APK download lock")?;
            println!(
                "Pinned {} bytes as {}. Version is declared; authenticity requires independent review.",
                identity.size_bytes, identity.sha256
            );
        }
        Task::Fetch {
            lock,
            url_env,
            cache,
        } => {
            let lock = DownloadLock::read(&lock)?;
            let cache = cache.unwrap_or_else(|| root.join("target/mobile-apk"));
            let destination = lock.cached_path(&cache);
            if destination.exists() {
                verify_file(&lock, &destination)?;
            } else {
                let url = std::env::var(&url_env)
                    .context("APK URL environment variable is missing or not Unicode")?;
                validate_fetch_url(&url)?;
                // Never format a transport error: it may embed the signed URL/query.
                let agent: ureq::Agent = ureq::config::Config::builder()
                    .https_only(true)
                    .max_redirects(0)
                    .http_status_as_error(false)
                    .tls_config(tls_config()?)
                    .timeout_global(Some(Duration::from_secs(600)))
                    .build()
                    .into();
                fetch(&agent, &url, &lock, &destination)?;
            }
            println!("Verified APK cache: {}", destination.display());
        }
        Task::Import { lock, input, cache } => {
            let lock = DownloadLock::read(&lock)?;
            let destination =
                lock.cached_path(&cache.unwrap_or_else(|| root.join("target/mobile-apk")));
            if destination.exists() {
                verify_file(&lock, &destination)?;
            } else {
                install(
                    &lock,
                    File::open(input).context("opening supplied APK")?,
                    &destination,
                )?;
            }
            println!("Verified APK cache: {}", destination.display());
        }
        Task::Verify { lock, cache } => {
            let lock = DownloadLock::read(&lock)?;
            let destination =
                lock.cached_path(&cache.unwrap_or_else(|| root.join("target/mobile-apk")));
            verify_file(&lock, &destination)?;
            println!("Verified APK cache: {}", destination.display());
        }
    }
    Ok(())
}

fn fetch(agent: &ureq::Agent, url: &str, lock: &DownloadLock, destination: &Path) -> Result<()> {
    let mut response = agent
        .get(url)
        .header("Accept-Encoding", "identity")
        .call()
        .map_err(|_| {
            anyhow::anyhow!(
                "APK HTTPS request failed; check URL expiry, network policy and TLS trust"
            )
        })?;
    ensure!(
        response.status().as_u16() == 200,
        "APK HTTPS request returned status {}",
        response.status().as_u16()
    );
    ensure!(
        response
            .headers()
            .get("content-encoding")
            .is_none_or(|v| v == "identity"),
        "encoded APK response is not accepted"
    );
    // Ignore Content-Length as an authority. The bounded stream and digest decide.
    install(lock, response.body_mut().as_reader(), destination)
        .map_err(|_| anyhow::anyhow!("APK download was incomplete or did not match the pinned size/SHA-256; cache not published"))
}

fn tls_config() -> Result<ureq::tls::TlsConfig> {
    let mut builder = ureq::tls::TlsConfig::builder();
    if let Some(path) = std::env::var_os("SSL_CERT_FILE") {
        let bytes = read_bounded(
            File::open(path).context("opening configured SSL_CERT_FILE")?,
            4 * 1024 * 1024,
        )?;
        builder = builder.root_certs(parse_roots(&bytes)?);
    }
    Ok(builder.build())
}

fn parse_roots(bytes: &[u8]) -> Result<ureq::tls::RootCerts> {
    let mut certificates = Vec::new();
    for item in ureq::tls::parse_pem(bytes) {
        match item.context("parsing configured TLS trust bundle")? {
            ureq::tls::PemItem::Certificate(certificate) => certificates.push(certificate),
            _ => anyhow::bail!("TLS trust bundle must contain certificates only"),
        }
    }
    ensure!(
        !certificates.is_empty(),
        "TLS trust bundle contains no certificates"
    );
    Ok(certificates.into())
}

fn validate_fetch_url(value: &str) -> Result<()> {
    ensure!(
        !value.chars().any(char::is_whitespace) && !value.chars().any(char::is_control),
        "invalid APK download URL"
    );
    let url = url::Url::parse(value).map_err(|_| anyhow::anyhow!("invalid APK download URL"))?;
    ensure!(
        url.scheme() == "https"
            && url.host_str().is_some()
            && url.username().is_empty()
            && url.password().is_none()
            && url.fragment().is_none(),
        "APK download URL must be credential-free HTTPS without a fragment"
    );
    Ok(())
}

fn read_bounded(mut reader: impl Read, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.by_ref().take(limit + 1).read_to_end(&mut bytes)?;
    ensure!(bytes.len() as u64 <= limit, "APK lock exceeds size limit");
    Ok(bytes)
}

fn copy_digest(
    mut reader: impl Read,
    writer: &mut impl Write,
    limit: u64,
) -> Result<(u64, String)> {
    let mut digest = Sha256::new();
    let mut size = 0;
    let mut prefix = [0; 4];
    reader.read_exact(&mut prefix).context("APK is truncated")?;
    ensure!(
        &prefix == b"PK\x03\x04",
        "APK must start with a ZIP local-file header"
    );
    let mut buffer = [0; 64 * 1024];
    writer.write_all(&prefix)?;
    digest.update(prefix);
    size += 4;
    loop {
        // Read one byte beyond the expected size to reject oversized/concatenated inputs.
        let remaining = (limit - size + 1).min(buffer.len() as u64) as usize;
        let count = reader.read(&mut buffer[..remaining])?;
        if count == 0 {
            break;
        }
        size += count as u64;
        ensure!(size <= limit, "APK exceeds pinned size or maximum limit");
        digest.update(&buffer[..count]);
        writer.write_all(&buffer[..count])?;
    }
    Ok((size, hex::encode(digest.finalize())))
}

fn verify_file(lock: &DownloadLock, path: &Path) -> Result<()> {
    ensure!(
        fs::symlink_metadata(path)?.file_type().is_file(),
        "cached APK must be a regular file"
    );
    let (size, digest) = copy_digest(File::open(path)?, &mut std::io::sink(), lock.size_bytes)?;
    ensure!(
        size == lock.size_bytes && digest == lock.sha256,
        "cached APK size/SHA-256 mismatch"
    );
    Ok(())
}

fn install(lock: &DownloadLock, reader: impl Read, destination: &Path) -> Result<()> {
    let parent = destination.parent().context("APK cache parent")?;
    fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    let (size, digest) = copy_digest(reader, temporary.as_file_mut(), lock.size_bytes)?;
    ensure!(
        size == lock.size_bytes && digest == lock.sha256,
        "APK size/SHA-256 mismatch"
    );
    temporary.as_file_mut().sync_all()?;
    match temporary.persist_noclobber(destination) {
        Ok(_) => Ok(()),
        Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
            verify_file(lock, destination)
        }
        Err(error) => Err(error).context("publishing APK cache"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const APK: &[u8] = b"PK\x03\x04synthetic apk container, not official evidence";

    fn identity() -> DownloadLock {
        DownloadLock {
            schema_version: 1,
            declared_version: "synthetic-1".into(),
            source_page: SOURCE.into(),
            size_bytes: APK.len() as u64,
            sha256: xtask_support::sha256(APK),
        }
    }

    /// Public commands still reject HTTP. This tests transfer handling using synthetic
    /// bytes on loopback without a network dependency or a production APK.
    #[test]
    fn fetch_checks_response_stream_and_redacts_signed_query_on_errors() {
        use std::net::TcpListener;
        for (status, body) in [
            (200, APK),
            (200, &b"PK\x03\x04wrong"[..]),
            (403, &b"expired"[..]),
            (302, &b"redirect"[..]),
        ] {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let address = listener.local_addr().unwrap();
            let server = std::thread::spawn(move || {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = [0; 4096];
                let count = stream.read(&mut request).unwrap();
                assert!(
                    String::from_utf8_lossy(&request[..count]).contains("?signed=private-token")
                );
                write!(stream, "HTTP/1.1 {status} Synthetic\r\nContent-Length: {}\r\nLocation: /private-token\r\nConnection: close\r\n\r\n", body.len()).unwrap();
                stream.write_all(body).unwrap();
            });
            let agent: ureq::Agent = ureq::config::Config::builder()
                .max_redirects(0)
                .http_status_as_error(false)
                .timeout_global(Some(Duration::from_secs(5)))
                .build()
                .into();
            let root = tempfile::tempdir().unwrap();
            let target = root.path().join("WhatsApp.apk");
            let result = fetch(
                &agent,
                &format!("http://{address}/apk?signed=private-token"),
                &identity(),
                &target,
            );
            if status == 200 && body == APK {
                result.unwrap();
                verify_file(&identity(), &target).unwrap();
            } else {
                assert!(!format!("{:#}", result.unwrap_err()).contains("private-token"));
                assert!(!target.exists());
            }
            server.join().unwrap();
        }
    }

    #[test]
    fn import_verify_and_pin_are_offline_and_content_addressed() {
        let root = tempfile::tempdir().unwrap();
        let input = root.path().join("input.apk");
        let lock = root.path().join("version.lock.json");
        fs::write(&input, APK).unwrap();
        run(
            root.path(),
            Task::Pin {
                input: input.clone(),
                version: "synthetic-1".into(),
                lock: lock.clone(),
            },
        )
        .unwrap();
        run(
            root.path(),
            Task::Import {
                input,
                lock: lock.clone(),
                cache: None,
            },
        )
        .unwrap();
        run(
            root.path(),
            Task::Verify {
                lock: lock.clone(),
                cache: None,
            },
        )
        .unwrap();
        let pinned = DownloadLock::read(&lock).unwrap();
        assert!(
            pinned
                .cached_path(&root.path().join("target/mobile-apk"))
                .is_file()
        );
        fs::write(
            pinned.cached_path(&root.path().join("target/mobile-apk")),
            b"tampered",
        )
        .unwrap();
        assert!(run(root.path(), Task::Verify { lock, cache: None }).is_err());
    }

    #[test]
    fn mismatch_and_truncation_never_publish_and_cleanup_temporary_files() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("cache/WhatsApp.apk");
        for bytes in [
            &APK[..APK.len() - 1],
            b"PK\x03\x04wrong",
            b"PK\x03\x04synthetic apk container, not official evidenceEXTRA",
        ] {
            assert!(install(&identity(), bytes, &target).is_err());
            assert!(!target.exists());
            assert_eq!(fs::read_dir(target.parent().unwrap()).unwrap().count(), 0);
        }
    }

    #[test]
    fn installation_never_replaces_an_existing_cache() {
        let root = tempfile::tempdir().unwrap();
        let target = root.path().join("WhatsApp.apk");
        install(&identity(), APK, &target).unwrap();
        install(&identity(), APK, &target).unwrap();
        verify_file(&identity(), &target).unwrap();
        assert!(install(&identity(), &b"PK\x03\x04bad"[..], &target).is_err());
        assert_eq!(fs::read(&target).unwrap(), APK);
    }

    #[test]
    fn commands_reuse_valid_cache_without_input_or_url_and_reject_corruption() {
        let root = tempfile::tempdir().unwrap();
        let lock = root.path().join("version.json");
        xtask_support::write_json(&lock, &identity()).unwrap();
        let target = identity().cached_path(&root.path().join("target/mobile-apk"));
        install(&identity(), APK, &target).unwrap();
        let import = || Task::Import {
            lock: lock.clone(),
            input: root.path().join("missing.apk"),
            cache: None,
        };
        let fetch = || Task::Fetch {
            lock: lock.clone(),
            url_env: "WHATSAPP_XTASK_TEST_NONEXISTENT_APK_URL_82C81".into(),
            cache: None,
        };
        run(root.path(), import()).unwrap();
        run(root.path(), fetch()).unwrap();
        fs::write(&target, b"PK\x03\x04corrupt").unwrap();
        for command in [import(), fetch()] {
            let error = run(root.path(), command).unwrap_err();
            assert!(format!("{error:#}").contains("mismatch"));
        }
        assert_eq!(fs::read(target).unwrap(), b"PK\x03\x04corrupt");
    }

    #[cfg(unix)]
    #[test]
    fn symlinked_cached_artifact_is_rejected() {
        let root = tempfile::tempdir().unwrap();
        let actual = root.path().join("actual.apk");
        let linked = root.path().join("linked.apk");
        fs::write(&actual, APK).unwrap();
        std::os::unix::fs::symlink(actual, &linked).unwrap();
        assert!(verify_file(&identity(), &linked).is_err());
    }

    #[test]
    fn configured_tls_roots_reject_empty_malformed_or_key_bundles() {
        for bytes in [
            &b""[..],
            &b"not certificates"[..],
            &b"-----BEGIN CERTIFICATE-----\n%%%\n-----END CERTIFICATE-----"[..],
            &b"-----BEGIN PRIVATE KEY-----\nMAA=\n-----END PRIVATE KEY-----"[..],
        ] {
            assert!(parse_roots(bytes).is_err());
        }
    }

    #[test]
    fn locks_and_urls_reject_unsafe_or_unsupported_inputs() {
        for url in [
            "http://example.test/a.apk",
            "https://user:secret@example.test/a",
            " https://example.test/a",
            "https://example.test/a#fragment",
            "https://example.test/a\n",
        ] {
            assert!(validate_fetch_url(url).is_err());
        }
        validate_fetch_url("https://scontent.whatsapp.net/a.apk?expiring=secret").unwrap();
        let mut lock = identity();
        lock.sha256 = "../escape".into();
        assert!(lock.validate().is_err());
        lock = identity();
        lock.schema_version = 2;
        assert!(lock.validate().is_err());
        assert!(read_bounded(&b"12345"[..], 4).is_err());
    }

    #[test]
    fn pin_never_replaces_an_existing_lock() {
        let root = tempfile::tempdir().unwrap();
        let input = root.path().join("input.apk");
        let lock = root.path().join("version.json");
        fs::write(&input, APK).unwrap();
        fs::write(&lock, b"existing review").unwrap();
        assert!(
            run(
                root.path(),
                Task::Pin {
                    input,
                    version: "synthetic".into(),
                    lock: lock.clone()
                }
            )
            .is_err()
        );
        assert_eq!(fs::read(lock).unwrap(), b"existing review");
    }
}

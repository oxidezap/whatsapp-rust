//! Fixture-only bounded polling and owned SQLite-file cleanup.

use anyhow::{Context, Result};
use std::time::Duration;

pub(crate) async fn wait_until(timeout: Duration, ready: impl Fn() -> bool) -> Result<()> {
    tokio::time::timeout(timeout, async {
        while !ready() {
            // A runnable yield-loop prevents paused Tokio time from advancing.
            tokio::time::sleep(Duration::from_millis(1)).await;
        }
    })
    .await
    .context("connected fixture condition timed out")
}

#[cfg(feature = "sqlite-storage")]
pub(crate) fn cleanup_database(path: &std::path::Path) -> Result<()> {
    for suffix in ["", "-wal", "-shm"] {
        let file = format!("{}{suffix}", path.display());
        match std::fs::remove_file(&file) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| format!("removing fixture file {file}"));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(start_paused = true)]
    async fn false_condition_reaches_the_virtual_deadline() {
        let start = tokio::time::Instant::now();
        let timeout = Duration::from_millis(10);
        let error = wait_until(timeout, || false)
            .await
            .expect_err("must time out");
        assert!(start.elapsed() >= timeout);
        assert!(error.to_string().contains("condition timed out"));
    }

    #[cfg(feature = "sqlite-storage")]
    #[test]
    fn cleanup_reports_io_failures_and_accepts_missing_sidecars() -> Result<()> {
        let root = std::env::var_os("CARGO_TARGET_DIR")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| std::path::PathBuf::from("target"))
            .join("connected-idle");
        std::fs::create_dir_all(&root)?;
        let path = root.join(format!(
            "cleanup-{}-{}.db",
            std::process::id(),
            rand::random::<u64>()
        ));
        std::fs::create_dir(&path)?;
        struct OwnedPath(std::path::PathBuf);
        impl Drop for OwnedPath {
            fn drop(&mut self) {
                // Best effort on test failure only; the successful path below
                // explicitly checks cleanup. Never remove someone else's path.
                let _ = std::fs::remove_file(&self.0);
                let _ = std::fs::remove_dir(&self.0);
            }
        }
        let _owned = OwnedPath(path.clone());
        let failure = cleanup_database(&path);
        std::fs::remove_dir(&path)?;
        assert!(
            failure.is_err(),
            "a directory cannot be silently removed as a file"
        );
        std::fs::write(&path, b"fictitious fixture")?;
        cleanup_database(&path)?;
        assert!(!path.exists());
        cleanup_database(&path)?;
        Ok(())
    }
}

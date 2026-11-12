// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

//! Build a fixed SQLite for native builds while Diesel 2.3 caps
//! `libsqlite3-sys` below 0.39 (RUSTSEC-2026-0459).
//!
//! `libsqlite3-sys` 0.38 bundles an affected SQLite. Its documented escape
//! hatch is to link a library you built yourself: with
//! `LIBSQLITE3_SYS_USE_PKG_CONFIG=1` it ignores `bundled`, and
//! `SQLITE3_LIB_DIR` plus `SQLITE3_STATIC=1` point it at a static library.
//! This task downloads the official release amalgamation, checks it against
//! the SHA3-256 that sqlite.org publishes, and compiles it with the options
//! `libsqlite3-sys` uses for its bundled copy, so behavior matches.
use anyhow::{Context, Result, ensure};
use sha3::{Digest, Sha3_256};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

/// SQLite 3.54.1, from <https://sqlite.org/download.html>.
const VERSION: &str = "3.54.1";
const URL: &str = "https://sqlite.org/2026/sqlite-autoconf-3540100.tar.gz";
const SIZE: u64 = 3_332_984;
const SHA3_256: &str = "b6ced585c750916b1052667239a12c25a7ff34e7f06e1e6e01ecb0427657d9c4";
const ROOT: &str = "sqlite-autoconf-3540100";

/// The compile options `libsqlite3-sys` 0.38 sets for its bundled SQLite on
/// Unix targets.
const DEFINES: &[&str] = &[
    "SQLITE_CORE",
    "SQLITE_DEFAULT_FOREIGN_KEYS=1",
    "SQLITE_ENABLE_API_ARMOR",
    "SQLITE_ENABLE_COLUMN_METADATA",
    "SQLITE_ENABLE_DBSTAT_VTAB",
    "SQLITE_ENABLE_FTS3",
    "SQLITE_ENABLE_FTS3_PARENTHESIS",
    "SQLITE_ENABLE_FTS5",
    "SQLITE_ENABLE_JSON1",
    "SQLITE_ENABLE_LOAD_EXTENSION=1",
    "SQLITE_ENABLE_MEMORY_MANAGEMENT",
    "SQLITE_ENABLE_RTREE",
    "SQLITE_ENABLE_STAT4",
    "SQLITE_SOUNDEX",
    "SQLITE_THREADSAFE=1",
    "SQLITE_USE_URI",
    "HAVE_USLEEP=1",
    "HAVE_ISNAN",
    "HAVE_LOCALTIME_R",
    "_POSIX_THREAD_SAFE_FUNCTIONS",
];

pub fn run(out: &Path, github_env: bool) -> Result<()> {
    std::fs::create_dir_all(out).with_context(|| format!("creating {}", out.display()))?;
    let archive = download()?;
    let source = out.join("src");
    std::fs::create_dir_all(&source)?;
    extract(&archive, &source)?;
    compile(&source, out)?;
    let lib_dir = std::fs::canonicalize(out)?;
    write_pkg_config(&lib_dir)?;
    let lines = [
        "LIBSQLITE3_SYS_USE_PKG_CONFIG=1".to_owned(),
        format!("SQLITE3_LIB_DIR={}", lib_dir.display()),
        format!("SQLITE3_INCLUDE_DIR={}", lib_dir.display()),
        "SQLITE3_STATIC=1".to_owned(),
    ];
    for line in &lines {
        println!("{line}");
    }
    if github_env {
        let path = std::env::var_os("GITHUB_ENV").context("GITHUB_ENV is not set")?;
        let mut file = std::fs::OpenOptions::new().append(true).open(path)?;
        for line in &lines {
            writeln!(file, "{line}")?;
        }
    }
    println!("built SQLite {VERSION} in {}", lib_dir.display());
    Ok(())
}

fn download() -> Result<Vec<u8>> {
    let agent: ureq::Agent = ureq::config::Config::builder()
        .https_only(true)
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(300)))
        .build()
        .into();
    let mut response = agent
        .get(URL)
        .header("Accept-Encoding", "identity")
        .call()
        .context("downloading the SQLite release archive")?;
    ensure!(
        response.status().as_u16() == 200,
        "SQLite download returned status {}",
        response.status().as_u16()
    );
    let mut bytes = Vec::new();
    response
        .body_mut()
        .as_reader()
        .take(SIZE + 1)
        .read_to_end(&mut bytes)?;
    verify(&bytes)?;
    Ok(bytes)
}

/// The archive must have the published size and SHA3-256.
fn verify(bytes: &[u8]) -> Result<()> {
    ensure!(
        bytes.len() as u64 == SIZE,
        "SQLite archive is {} bytes, expected {SIZE}",
        bytes.len()
    );
    let digest = hex::encode(Sha3_256::digest(bytes));
    ensure!(
        digest == SHA3_256,
        "SQLite archive SHA3-256 {digest} does not match the published {SHA3_256}"
    );
    Ok(())
}

fn extract(archive: &[u8], into: &Path) -> Result<()> {
    let mut tar = tar::Archive::new(flate2::read::GzDecoder::new(archive));
    let mut found = 0;
    for entry in tar.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        let Ok(relative) = path.strip_prefix(ROOT) else {
            continue;
        };
        let name = relative.to_str().unwrap_or_default();
        if matches!(name, "sqlite3.c" | "sqlite3.h" | "sqlite3ext.h") {
            ensure!(
                entry.header().entry_type().is_file(),
                "{name} is not a file"
            );
            let mut contents = Vec::new();
            entry.read_to_end(&mut contents)?;
            std::fs::write(into.join(name), contents)?;
            found += 1;
        }
    }
    ensure!(found == 3, "SQLite archive is missing its amalgamation");
    Ok(())
}

fn compile(source: &Path, out: &Path) -> Result<()> {
    let host = host_triple()?;
    let mut build = cc::Build::new();
    build
        .file(source.join("sqlite3.c"))
        .include(source)
        .target(&host)
        .host(&host)
        .opt_level(2)
        .out_dir(out)
        .cargo_metadata(false)
        .warnings(false);
    for define in DEFINES {
        match define.split_once('=') {
            Some((name, value)) => build.define(name, value),
            None => build.define(define, None),
        };
    }
    build.try_compile("sqlite3").context("compiling SQLite")?;
    for header in ["sqlite3.h", "sqlite3ext.h"] {
        std::fs::copy(source.join(header), out.join(header))?;
    }
    ensure!(
        out.join("libsqlite3.a").is_file(),
        "compiling SQLite produced no libsqlite3.a"
    );
    Ok(())
}

/// `libsqlite3-sys` asks pkg-config first, searching `SQLITE3_LIB_DIR/pkgconfig`
/// ahead of the default paths. Without this file a system `sqlite3.pc` wins
/// and the older system library is linked instead.
fn write_pkg_config(lib_dir: &Path) -> Result<()> {
    let dir = lib_dir.join("pkgconfig");
    std::fs::create_dir_all(&dir)?;
    let lib = lib_dir.display();
    std::fs::write(
        dir.join("sqlite3.pc"),
        format!(
            "Name: SQLite\nDescription: SQL database engine (RUSTSEC-2026-0459 fixed)\n\
             Version: {VERSION}\nLibs: -L{lib} -lsqlite3\nCflags: -I{lib}\n"
        ),
    )?;
    Ok(())
}

fn host_triple() -> Result<String> {
    let output = std::process::Command::new(std::env::var_os("RUSTC").unwrap_or("rustc".into()))
        .arg("-vV")
        .output()
        .context("running rustc -vV")?;
    ensure!(output.status.success(), "rustc -vV failed");
    String::from_utf8(output.stdout)?
        .lines()
        .find_map(|line| line.strip_prefix("host: ").map(str::to_owned))
        .context("rustc -vV printed no host")
}

/// The default output directory.
pub fn default_out(root: &Path) -> PathBuf {
    root.join("target/fixed-sqlite")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_wrong_archive_is_refused() {
        assert!(verify(b"not sqlite").is_err());
        let mut sized = vec![0u8; SIZE as usize];
        assert!(verify(&sized).is_err());
        sized.pop();
        assert!(verify(&sized).is_err());
    }
}

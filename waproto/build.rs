//! Verify committed schemas and generated Rust without compiling a generator.

use sha2::{Digest as _, Sha256};
use std::io::{self, Read as _};

#[cfg(feature = "protobuf-experiment")]
#[path = "build/reference.rs"]
#[allow(dead_code)]
mod reference;
#[cfg(feature = "protobuf-experiment")]
pub use reference::policy;

fn hash(path: &str) -> io::Result<String> {
    let mut input = std::fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 16384];
    loop {
        let length = input.read(&mut buffer)?;
        if length == 0 {
            break;
        }
        digest.update(&buffer[..length]);
    }
    Ok(digest
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn verify(manifest: &str, entries: &[(&str, &str)]) -> io::Result<()> {
    println!("cargo:rerun-if-changed={manifest}");
    let expected = std::fs::read_to_string(manifest)?;
    for &(kind, path) in entries {
        println!("cargo:rerun-if-changed={path}");
        let matches = expected
            .lines()
            .filter_map(|line| line.split_once(' '))
            .filter(|(name, _)| *name == kind)
            .collect::<Vec<_>>();
        if matches.len() != 1 || matches[0].1.trim() != hash(path)? {
            return Err(io::Error::other(format!(
                "{path} does not match {manifest}. Run `cargo xt proto-desc` or `cargo xt proto-codecs` and commit the generated files."
            )));
        }
    }
    Ok(())
}

fn main() -> io::Result<()> {
    verify(
        "src/whatsapp.desc.sha256",
        &[
            ("proto", "src/whatsapp.proto"),
            ("desc", "src/whatsapp.desc"),
        ],
    )?;
    verify(
        "src/generated/whatsapp.rs.sha256",
        &[
            ("descriptor", "src/whatsapp.desc"),
            ("rust", "src/generated/whatsapp.rs"),
            ("tags", "src/generated/tags.rs"),
        ],
    )?;
    #[cfg(feature = "protobuf-experiment")]
    reference::generate_reference()?;
    Ok(())
}

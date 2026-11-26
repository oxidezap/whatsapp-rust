//! Generate the protobuf model from the committed descriptor.

use sha2::{Digest as _, Sha256};
use std::io::{self, Read as _};

#[path = "codegen/descriptor.rs"]
mod descriptor;
#[path = "codegen/emit_owned.rs"]
#[allow(dead_code)]
mod emit_owned;
#[path = "codegen/features.rs"]
mod features;
#[path = "codegen/names.rs"]
#[allow(dead_code)]
mod names;
#[path = "codegen/policy.rs"]
#[allow(dead_code)]
mod policy;
#[path = "codegen/project.rs"]
#[allow(dead_code)]
mod project;
#[cfg(feature = "protobuf-reference")]
#[path = "build/reference.rs"]
#[allow(dead_code)]
mod reference;
#[path = "codegen/retained.rs"]
mod retained;
#[path = "codegen/schema.rs"]
#[allow(dead_code)]
mod schema;
#[path = "build_support/signal_storage.rs"]
mod signal_storage;
#[path = "codegen/wallpaper.rs"]
mod wallpaper;
#[path = "codegen/whatsapp.rs"]
mod whatsapp;

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
                "{path} does not match {manifest}. Run `cargo xt proto-desc` to regenerate the descriptor."
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
    println!("cargo:rerun-if-changed=codegen");
    println!("cargo:rerun-if-changed=build_support/signal_storage.rs");
    println!("cargo:rerun-if-changed=signal-storage.snapshot");
    let descriptor = std::fs::read("src/whatsapp.desc")?;
    let (_, source, tags) = whatsapp::generate(&descriptor)?;
    let out = std::path::PathBuf::from(
        std::env::var_os("OUT_DIR").ok_or_else(|| io::Error::other("OUT_DIR is not set"))?,
    );
    std::fs::write(out.join("generated-whatsapp.rs"), source)?;
    std::fs::write(out.join("generated-tags.rs"), tags)?;
    #[cfg(feature = "protobuf-reference")]
    reference::generate_reference()?;
    Ok(())
}

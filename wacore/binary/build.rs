use std::env;
use std::fs;
use std::io;
use std::path::PathBuf;

fn main() -> io::Result<()> {
    println!("cargo:rerun-if-changed=src/token_maps.generated.rs");
    let out_dir = env::var_os("OUT_DIR").ok_or_else(|| io::Error::other("missing OUT_DIR"))?;
    fs::copy(
        "src/token_maps.generated.rs",
        PathBuf::from(out_dir).join("token_maps.rs"),
    )?;
    Ok(())
}

//! Generates buffa types for the MLow runtime constant tables (the `voip`
//! feature's `src/voip/mlow/` codec) from the committed descriptor
//! `src/voip/mlow/tables.desc`, compiled once from `tables.proto`. Only the
//! `voip` feature consumes these tables, so codegen is skipped otherwise.
//! Reading the descriptor means consumers never need `protoc`; editing the
//! proto requires regenerating the descriptor via
//! `scripts/regenerate-tables-desc.sh`.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=src/voip/mlow/tables.desc");
    println!("cargo:rerun-if-changed=src/voip/mlow/tables.proto");
    println!("cargo:rerun-if-changed=build.rs");

    if std::env::var_os("CARGO_FEATURE_VOIP").is_none() {
        return Ok(());
    }

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR must be set by cargo");

    buffa_build::Config::new()
        .descriptor_set("src/voip/mlow/tables.desc")
        .files(&["tables.proto"])
        .preserve_unknown_fields(false)
        .out_dir(&out_dir)
        .compile()
}

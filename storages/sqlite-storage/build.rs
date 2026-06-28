//! Generates Rust types for the on-disk wire format (`src/wire.rs` consumes
//! them) from the committed descriptor `proto/wire.desc`, compiled once from
//! `proto/wire.proto`. Reading the descriptor means consumers never need
//! `protoc`; editing the proto requires regenerating the descriptor via
//! `scripts/regenerate-wire-desc.sh`.

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("cargo:rerun-if-changed=proto/wire.desc");
    println!("cargo:rerun-if-changed=proto/wire.proto");
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR must be set by cargo");

    buffa_build::Config::new()
        .descriptor_set("proto/wire.desc")
        .files(&["wire.proto"])
        .preserve_unknown_fields(false)
        .out_dir(&out_dir)
        .compile()
}

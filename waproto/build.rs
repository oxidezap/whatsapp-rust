//! # Updating the proto
//!
//! 1. Edit `src/whatsapp.proto`.
//! 2. Optional: format with `buf format src/whatsapp.proto -w`.
//! 3. Regenerate the descriptor: `scripts/regenerate-proto-desc.sh`
//!    (wraps `protoc --descriptor_set_out=src/whatsapp.desc …`).
//! 4. `cargo build` — this script consumes `whatsapp.desc` and writes
//!    `whatsapp.rs` to `OUT_DIR`. Consumers never need `protoc` installed;
//!    only editors of the proto do.

fn main() -> std::io::Result<()> {
    // Rerun on desc change (new codegen) and proto change (so the staleness
    // guard below runs). `build.rs` itself too.
    println!("cargo:rerun-if-changed=src/whatsapp.desc");
    println!("cargo:rerun-if-changed=src/whatsapp.desc.sha256");
    println!("cargo:rerun-if-changed=src/whatsapp.proto");
    println!("cargo:rerun-if-changed=build.rs");

    ensure_proto_descriptor_hash()?;

    let out_dir = std::env::var("OUT_DIR").expect("OUT_DIR must be set by cargo");
    let out_path = std::path::PathBuf::from(&out_dir);

    buffa_build::Config::new()
        .descriptor_set("src/whatsapp.desc")
        .files(&["whatsapp.proto"])
        // Messages + oneofs: serde over the struct/oneof shape. Serialize always;
        // Deserialize only for the WASM bridge (halves serde codegen).
        .message_attribute(".", "#[derive(serde::Serialize)]")
        .message_attribute(
            ".",
            "#[cfg_attr(feature = \"serde-deserialize\", derive(serde::Deserialize))]",
        )
        .message_attribute(
            ".",
            "#[cfg_attr(feature = \"serde-deserialize\", serde(default))]",
        )
        .oneof_attribute(".", "#[derive(serde::Serialize)]")
        .oneof_attribute(
            ".",
            "#[cfg_attr(feature = \"serde-deserialize\", derive(serde::Deserialize))]",
        )
        // Enums: variant name by default; numeric repr (prost parity, JS bridge)
        // under `serde-enum-repr`. Targeting enums separately from oneofs needs
        // buffa's enum_attribute/oneof_attribute split.
        .enum_attribute(
            ".",
            "#[cfg_attr(not(feature = \"serde-enum-repr\"), derive(serde::Serialize))]",
        )
        .enum_attribute(
            ".",
            "#[cfg_attr(feature = \"serde-enum-repr\", derive(serde_repr::Serialize_repr))]",
        )
        .enum_attribute(
            ".",
            "#[cfg_attr(all(feature = \"serde-deserialize\", not(feature = \"serde-enum-repr\")), derive(serde::Deserialize))]",
        )
        .enum_attribute(
            ".",
            "#[cfg_attr(all(feature = \"serde-deserialize\", feature = \"serde-enum-repr\"), derive(serde_repr::Deserialize_repr))]",
        )
        .enum_attribute(
            ".",
            "#[cfg_attr(all(feature = \"serde-snake-case\", not(feature = \"serde-enum-repr\")), serde(rename_all(deserialize = \"snake_case\")))]",
        )
        // O(1)-clone Bytes for hot-path crypto structures instead of Vec<u8>.
        .use_bytes_type_in(&[
            ".whatsapp.SessionStructure.Chain.ChainKey",
            ".whatsapp.SessionStructure.Chain.MessageKey",
            ".whatsapp.SenderKeyStateStructure.SenderChainKey",
            ".whatsapp.SenderKeyStateStructure.SenderMessageKey",
            ".whatsapp.SenderKeyStateStructure.SenderSigningKey",
        ])
        // Bytes fields lack serde support; skip them (internal crypto state).
        .field_attribute(
            ".whatsapp.SessionStructure.Chain.ChainKey.key",
            "#[serde(skip)]",
        )
        .field_attribute(
            ".whatsapp.SessionStructure.Chain.MessageKey.cipher_key",
            "#[serde(skip)]",
        )
        .field_attribute(
            ".whatsapp.SessionStructure.Chain.MessageKey.mac_key",
            "#[serde(skip)]",
        )
        .field_attribute(
            ".whatsapp.SessionStructure.Chain.MessageKey.iv",
            "#[serde(skip)]",
        )
        .field_attribute(
            ".whatsapp.SenderKeyStateStructure.SenderChainKey.seed",
            "#[serde(skip)]",
        )
        .field_attribute(
            ".whatsapp.SenderKeyStateStructure.SenderMessageKey.seed",
            "#[serde(skip)]",
        )
        .field_attribute(
            ".whatsapp.SenderKeyStateStructure.SenderSigningKey.public",
            "#[serde(skip)]",
        )
        .field_attribute(
            ".whatsapp.SenderKeyStateStructure.SenderSigningKey.private",
            "#[serde(skip)]",
        )
        // We control both encoder and decoder — no need to preserve
        // unknown fields. Disabling removes __buffa_unknown_fields from
        // every struct, eliminating allocation/drop overhead in nested
        // types like SessionStructure (chains × message keys).
        .preserve_unknown_fields(false)
        // Generate view types for zero-copy decoding.
        .generate_views(true)
        .out_dir(&out_path)
        .compile()
        .map_err(|e| std::io::Error::other(e.to_string()))?;

    // Buffa <0.7 generated an owned `__buffa_cached_size` field that needed
    // to be skipped for serde. Buffa 0.7 removed it, so keep the patch only
    // for older generated output.
    let generated = out_path.join("whatsapp.rs");
    let original = std::fs::read_to_string(&generated)?;
    let needle = "pub __buffa_cached_size:";
    let count = original.matches(needle).count();
    if count > 0 {
        let patched = original.replace(needle, &format!("#[serde(skip)]\n    {needle}"));
        std::fs::write(&generated, patched)?;
    }

    Ok(())
}

fn ensure_proto_descriptor_hash() -> std::io::Result<()> {
    let proto = std::fs::read("src/whatsapp.proto")?;
    let desc = std::fs::read("src/whatsapp.desc")?;
    let expected = read_expected_hashes("src/whatsapp.desc.sha256")?;
    let actual_proto = sha256_hex(&proto);
    let actual_desc = sha256_hex(&desc);

    if actual_proto != expected.proto || actual_desc != expected.desc {
        return Err(std::io::Error::other(format!(
            "waproto: src/whatsapp.proto/src/whatsapp.desc do not match src/whatsapp.desc.sha256. \
             Run `scripts/regenerate-proto-desc.sh` to refresh the descriptor \
             and commit src/whatsapp.proto, src/whatsapp.desc, and \
             src/whatsapp.desc.sha256. expected proto {}, desc {}; got proto {}, desc {}",
            expected.proto, expected.desc, actual_proto, actual_desc
        )));
    }

    Ok(())
}

struct ExpectedHashes {
    proto: String,
    desc: String,
}

fn read_expected_hashes(path: &str) -> std::io::Result<ExpectedHashes> {
    let contents = std::fs::read_to_string(path)?;
    let mut proto = None;
    let mut desc = None;

    for line in contents.lines() {
        let mut parts = line.split_whitespace();
        let Some(name) = parts.next() else {
            continue;
        };
        let Some(hash) = parts.next() else {
            continue;
        };
        match name {
            "proto" => proto = Some(hash.to_owned()),
            "desc" => desc = Some(hash.to_owned()),
            _ => {}
        }
    }

    let Some(proto) = proto else {
        return Err(std::io::Error::other(format!(
            "waproto: {path} missing `proto <sha256>` entry"
        )));
    };
    let Some(desc) = desc else {
        return Err(std::io::Error::other(format!(
            "waproto: {path} missing `desc <sha256>` entry"
        )));
    };

    Ok(ExpectedHashes { proto, desc })
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest as _, Sha256};

    const HEX: &[u8; 16] = b"0123456789abcdef";

    let digest = Sha256::digest(bytes);
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

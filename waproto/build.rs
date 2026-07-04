//! # Updating the proto
//!
//! 1. Edit `src/whatsapp.proto` (kept in the upstream / whatspec camelCase
//!    form — do NOT hand-rename fields to snake_case).
//! 2. Optional: format with `buf format src/whatsapp.proto -w`.
//! 3. Regenerate the descriptor: `scripts/regenerate-proto-desc.sh`
//!    (wraps `protoc --descriptor_set_out=src/whatsapp.desc …`).
//! 4. `cargo build` — this script consumes `whatsapp.desc`, snake_cases the
//!    field names for the Rust API, and writes `whatsapp.rs` to `OUT_DIR`.
//!    Consumers never need `protoc`; only editors of the proto do.
//!
//! ## Why the descriptor is snake_cased at build time
//!
//! buffa generates Rust field idents verbatim from the proto field names, so a
//! camelCase proto would yield camelCase Rust fields. To keep the proto in the
//! upstream camelCase form (so it can be regenerated from whatspec untouched)
//! while exposing the prost-style snake_case Rust API the codebase uses, this
//! script decodes the committed descriptor, snake_cases every message/oneof
//! field name, and feeds the rewritten descriptor to buffa. Wire format is
//! unaffected (the wire keys on field numbers, not names); message/enum type
//! names and enum value names are left as-is. Drop this once buffa grows a
//! native idiomatic-field-names option (anthropics/buffa#256).

use buffa::Message as _;
use buffa_descriptor::generated::descriptor::{DescriptorProto, FileDescriptorSet};
use heck::ToSnakeCase as _;

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

    // Rewrite the committed camelCase descriptor into a snake_case-field one for
    // buffa codegen (see module docs). buffa reads the rewritten copy from OUT_DIR.
    let snake_desc = out_path.join("whatsapp.snake.desc");
    snake_case_descriptor_fields("src/whatsapp.desc", &snake_desc)?;

    // Emit the wire-tag consts (field numbers) for hand-written partial decoders.
    // Generated from the original descriptor; the shouty/snake transforms are
    // unaffected by the camelCase->snake_case field rename.
    let fds = FileDescriptorSet::decode_from_slice(&std::fs::read("src/whatsapp.desc")?)
        .map_err(std::io::Error::other)?;
    generate_tags(&fds, &out_path.join("tags.rs"))?;

    buffa_build::Config::new()
        .descriptor_set(&snake_desc)
        .files(&["whatsapp.proto"])
        // Box every singular message field. buffa defaults them to inline; for
        // WhatsApp's deep, many-optional-field messages (every message variant
        // is its own inline slot) that makes size_of explode recursively,
        // turning decode and Vec growth into large struct memcpys. Box keeps
        // the structs pointer-sized.
        .box_type(buffa_build::PointerRepr::Box)
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
        // buffa emits SCREAMING_SNAKE variant names (CHROME, MESSAGE_EDIT), so
        // `lowercase` yields the intended chrome / message_edit. `snake_case`
        // would insert a separator before every char (c_h_r_o_m_e).
        .enum_attribute(
            ".",
            "#[cfg_attr(all(feature = \"serde-snake-case\", not(feature = \"serde-enum-repr\")), serde(rename_all(deserialize = \"lowercase\")))]",
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
        // We control both encoder and decoder — no need to preserve unknown
        // fields. Disabling removes __buffa_unknown_fields from every struct,
        // eliminating allocation/drop overhead in nested types like
        // SessionStructure (chains × message keys).
        .preserve_unknown_fields(false)
        // Generate view types for zero-copy decoding.
        .generate_views(true)
        // No `with_*` setters: the client builds messages with struct-literal
        // syntax (`..Default::default()`), never the generated setters. Skipping
        // them drops ~2k dead `#[inline]` methods worth of MIR/llvm-lines/compile
        // time (they're already DCE'd from `.text`, so this is a build-time win).
        .generate_with_setters(false)
        .out_dir(&out_path)
        .compile()
        .map_err(|e| std::io::Error::other(e.to_string()))?;

    Ok(())
}

/// Decode the committed (camelCase-field) descriptor, snake_case every message
/// and oneof field name, and write the rewritten descriptor to `output`. Field
/// numbers — the only thing the wire format depends on — are untouched, so this
/// is wire-compatible; it only changes the Rust field idents buffa generates.
fn snake_case_descriptor_fields(input: &str, output: &std::path::Path) -> std::io::Result<()> {
    let mut fds = FileDescriptorSet::decode_from_slice(&std::fs::read(input)?)
        .map_err(std::io::Error::other)?;
    for file in &mut fds.file {
        for message in &mut file.message_type {
            snake_case_message_fields(message);
        }
    }
    write_if_changed(output, &fds.encode_to_vec())
}

/// Skip the write when bytes are unchanged, to keep the file's mtime stable.
///
/// buffa_build emits a `cargo:rerun-if-changed` for this OUT_DIR descriptor, so
/// rewriting it every run would re-invalidate the build script and force a full
/// waproto rebuild on every `cargo run`.
fn write_if_changed(path: &std::path::Path, contents: &[u8]) -> std::io::Result<()> {
    if let Ok(existing) = std::fs::read(path)
        && existing == contents
    {
        return Ok(());
    }
    std::fs::write(path, contents)
}

fn snake_case_message_fields(message: &mut DescriptorProto) {
    for field in &mut message.field {
        if let Some(name) = &field.name {
            let snake = name.to_snake_case();
            // Keep json_name in sync so buffa's serde/proto_name matches the ident.
            field.json_name = Some(snake.clone());
            field.name = Some(snake);
        }
    }
    for oneof in &mut message.oneof_decl {
        if let Some(name) = &oneof.name {
            oneof.name = Some(name.to_snake_case());
        }
    }
    for nested in &mut message.nested_type {
        snake_case_message_fields(nested);
    }
}

/// Emit `tags.rs`: a nested module tree mirroring the proto's message
/// hierarchy, with one `pub const <FIELD>: u32 = <number>;` per field. Reads
/// the original (camelCase) descriptor; const/module names go through
/// shouty/snake transforms that yield the same output regardless of the
/// camelCase->snake_case rename, so the consts match the generated Rust API.
fn generate_tags(fds: &FileDescriptorSet, out_path: &std::path::Path) -> std::io::Result<()> {
    use heck::{ToShoutySnakeCase as _, ToSnakeCase as _};

    /// Mirror of prost-build's identifier sanitization so module names always
    /// match the message names buffa/prost would generate.
    fn module_ident(name: &str) -> String {
        let snake = name.to_snake_case();
        match snake.as_str() {
            "as" | "break" | "const" | "continue" | "else" | "enum" | "false" | "fn" | "for"
            | "if" | "impl" | "in" | "let" | "loop" | "match" | "mod" | "move" | "mut" | "pub"
            | "ref" | "return" | "static" | "struct" | "trait" | "true" | "type" | "unsafe"
            | "use" | "where" | "while" | "dyn" | "abstract" | "become" | "box" | "do"
            | "final" | "macro" | "override" | "priv" | "typeof" | "unsized" | "virtual"
            | "yield" | "async" | "await" | "try" | "gen" => format!("r#{snake}"),
            "_" | "super" | "self" | "crate" | "extern" => format!("{snake}_"),
            other if other.starts_with(|c: char| c.is_numeric()) => format!("_{snake}"),
            _ => snake,
        }
    }

    fn emit_message(out: &mut String, msg: &DescriptorProto, indent: usize) {
        // Synthetic map-entry messages have no hand-decodable surface.
        if msg
            .options
            .as_option()
            .and_then(|o| o.map_entry)
            .unwrap_or(false)
        {
            return;
        }
        let msg_name = msg.name.as_deref().unwrap_or_default();
        let pad = "    ".repeat(indent);
        out.push_str(&format!("{pad}pub mod {} {{\n", module_ident(msg_name)));
        let mut seen = std::collections::HashSet::new();
        for field in &msg.field {
            let const_name = field
                .name
                .as_deref()
                .unwrap_or_default()
                .to_shouty_snake_case();
            // Two field names collapsing to one const (e.g. fooBar/foo_bar)
            // would emit duplicate consts; fail loudly at generation time.
            assert!(
                seen.insert(const_name.clone()),
                "tags.rs: const name collision `{const_name}` in message `{msg_name}`"
            );
            out.push_str(&format!(
                "{pad}    pub const {const_name}: u32 = {};\n",
                field.number.unwrap_or_default()
            ));
        }
        for nested in &msg.nested_type {
            emit_message(out, nested, indent + 1);
        }
        out.push_str(&format!("{pad}}}\n"));
    }

    let mut out = String::with_capacity(1 << 20);
    out.push_str(
        "// @generated from whatsapp.desc by waproto's build.rs. Do not edit.\n\
         //\n\
         // Wire tag of every message field in whatsapp.proto, for hand-written\n\
         // partial decoders. Referencing these (or compile-time asserting against\n\
         // them) ties custom wire-walking code to the schema: renumbered fields\n\
         // propagate automatically, removed or renamed ones fail compilation.\n",
    );
    for file in &fds.file {
        for msg in &file.message_type {
            emit_message(&mut out, msg, 0);
        }
    }
    std::fs::write(out_path, out)
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

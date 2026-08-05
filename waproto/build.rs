//! # Updating the proto
//!
//! 1. Edit `src/whatsapp.proto` (kept in the upstream / whatspec camelCase
//!    form — do NOT hand-rename fields to snake_case).
//! 2. Optional: format with `buf format src/whatsapp.proto -w`.
//! 3. Regenerate the descriptor: `scripts/regenerate-proto-desc.sh`
//!    (wraps `protoc --descriptor_set_out=src/whatsapp.desc …`).
//! 4. `cargo build` — this script consumes `whatsapp.desc` and writes
//!    `whatsapp.rs` to `OUT_DIR`. Consumers never need `protoc`; only editors
//!    of the proto do.
//!
//! The proto stays in the upstream camelCase form; buffa's
//! `idiomatic_field_names` converts field/oneof idents to snake_case at
//! codegen time (word boundaries match heck/prost), so the Rust API keeps the
//! prost-style names. Attribute/override paths below therefore use the
//! *proto* (camelCase) field names.
//!
//! Fields this crate persists but upstream does not declare go in
//! [`LOCAL_FIELDS`], never in the `.proto`.

use buffa::Message as _;
use buffa_descriptor::generated::descriptor::{
    DescriptorProto, FieldDescriptorProto, FileDescriptorSet, field_descriptor_proto,
};

/// A field this crate persists that the upstream proto does not declare.
struct LocalField {
    /// Message path inside the `whatsapp` package, e.g. `Outer.Inner`.
    message: &'static str,
    name: &'static str,
    number: i32,
    kind: field_descriptor_proto::Type,
}

/// Local additions to the upstream schema, spliced into the descriptor at
/// build time. `src/whatsapp.proto` and `src/whatsapp.desc` are regenerated
/// wholesale from whatspec, so a field declared there would be lost on the
/// next sync; declared here it survives, and a sync that lands on one of these
/// numbers fails the build instead of silently reinterpreting records already
/// written.
///
/// Numbers stay far above what upstream uses — it appends low ones without
/// notice, the way `kyberPreKeyId = 4` and `kyberCiphertext = 5` arrived on
/// `SessionStructure.PendingPreKey`.
const LOCAL_FIELDS: &[LocalField] = &[LocalField {
    // Deriving the stored cipher/mac/iv from this seed is one-way, so a
    // skipped message key that kept only the derived material could never be
    // projected back into a seed-based external format.
    message: "SessionStructure.Chain.MessageKey",
    name: "seed",
    number: 100,
    kind: field_descriptor_proto::Type::TYPE_BYTES,
}];

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

    // Build-time descriptor decode — nothing to pin in waproto::codec.
    #[allow(clippy::disallowed_methods)]
    let mut fds = FileDescriptorSet::decode_from_slice(&std::fs::read("src/whatsapp.desc")?)
        .map_err(std::io::Error::other)?;
    apply_local_fields(&mut fds)?;

    // Emit the wire-tag consts (field numbers) for hand-written partial decoders.
    generate_tags(&fds, &out_path.join("tags.rs"))?;

    // Codegen reads the spliced descriptor, so local fields reach the Rust API
    // with the same guarantees as upstream ones. `buffa_build` registers this
    // path as rerun-if-changed, so rewriting it unconditionally would bump its
    // mtime on every build and leave the crate permanently dirty.
    let descriptor = out_path.join("whatsapp.local.desc");
    #[allow(clippy::disallowed_methods)]
    let encoded = fds.encode_to_vec();
    if !std::fs::read(&descriptor).is_ok_and(|current| current == encoded) {
        std::fs::write(&descriptor, &encoded)?;
    }

    buffa_build::Config::new()
        .descriptor_set(&descriptor)
        .files(&["whatsapp.proto"])
        // snake_case Rust idents from the upstream camelCase proto (see module
        // docs); wire format and descriptor names are untouched.
        .idiomatic_field_names(true)
        // Open enum (buffa 0.9): unknown wire values surface as
        // EnumValue::Unknown(n) instead of silently decoding as None (which
        // the appstate LTHash path used to default to SET, corrupting the
        // hash). Scoped to SyncdOperation only: opening an enum changes its
        // serde shape (EnumValue serializes known values as proto names,
        // unknown as numbers), so the message-level enums under the
        // serde-enum-repr JS-bridge contract stay closed.
        .open_enums_in(&[".whatsapp.SyncdMutation.SyncdOperation"])
        // Keep the opened field on the closed-enum serde contracts
        // (serde-enum-repr numbers, serde-snake-case lowercase); see
        // crate::open_enum_serde.
        .field_attribute(
            ".whatsapp.SyncdMutation.operation",
            "#[serde(serialize_with = \"crate::open_enum_serde::serialize\")]",
        )
        .field_attribute(
            ".whatsapp.SyncdMutation.operation",
            "#[cfg_attr(feature = \"serde-deserialize\", serde(deserialize_with = \"crate::open_enum_serde::deserialize\"))]",
        )
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
            ".whatsapp.SessionStructure.Chain.MessageKey.cipherKey",
            "#[serde(skip)]",
        )
        .field_attribute(
            ".whatsapp.SessionStructure.Chain.MessageKey.macKey",
            "#[serde(skip)]",
        )
        .field_attribute(
            ".whatsapp.SessionStructure.Chain.MessageKey.iv",
            "#[serde(skip)]",
        )
        .field_attribute(
            ".whatsapp.SessionStructure.Chain.MessageKey.seed",
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
        .out_dir(&out_path)
        .compile()
        .map_err(|e| std::io::Error::other(e.to_string()))?;

    Ok(())
}

/// Splice [`LOCAL_FIELDS`] into the descriptor decoded from `whatsapp.desc`.
///
/// Fails rather than resolves when upstream already occupies the name or the
/// number: silently winning that race would change what an existing field
/// decodes as, and every record already on disk with it.
fn apply_local_fields(fds: &mut FileDescriptorSet) -> std::io::Result<()> {
    for local in LOCAL_FIELDS {
        let mut messages = fds
            .file
            .iter_mut()
            .filter(|file| file.package.as_deref() == Some("whatsapp"))
            .flat_map(|file| file.message_type.iter_mut());
        let mut message = None;
        for segment in local.message.split('.') {
            message = match message {
                None => messages.find(|msg| msg.name.as_deref() == Some(segment)),
                Some(parent) => {
                    let parent: &mut DescriptorProto = parent;
                    parent
                        .nested_type
                        .iter_mut()
                        .find(|msg| msg.name.as_deref() == Some(segment))
                }
            };
            if message.is_none() {
                break;
            }
        }
        let message = message.ok_or_else(|| {
            std::io::Error::other(format!("local field target {} not found", local.message))
        })?;

        if let Some(existing) = message
            .field
            .iter()
            .find(|f| f.number == Some(local.number) || f.name.as_deref() == Some(local.name))
        {
            return Err(std::io::Error::other(format!(
                "local field {}.{} = {} collides with upstream {:?} = {:?}",
                local.message, local.name, local.number, existing.name, existing.number
            )));
        }

        message.field.push(FieldDescriptorProto {
            name: Some(local.name.to_owned()),
            number: Some(local.number),
            label: Some(field_descriptor_proto::Label::LABEL_OPTIONAL),
            r#type: Some(local.kind),
            json_name: Some(local.name.to_owned()),
            ..Default::default()
        });
    }
    Ok(())
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

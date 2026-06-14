//! # Updating the proto
//!
//! 1. Edit `src/whatsapp.proto`.
//! 2. Optional: format with `buf format src/whatsapp.proto -w`.
//! 3. Regenerate the descriptor: `scripts/regenerate-proto-desc.sh`
//!    (wraps `protoc --descriptor_set_out=src/whatsapp.desc ...`).
//! 4. `cargo build` — this script consumes `whatsapp.desc` and writes
//!    `whatsapp.rs` + `tags.rs` to `OUT_DIR`. Consumers never need `protoc`
//!    installed; only editors of the proto do.

use prost::Message as _;

fn main() -> std::io::Result<()> {
    // Rerun on desc change (new codegen) and proto change (so the staleness
    // guard below runs). `build.rs` itself too.
    println!("cargo:rerun-if-changed=src/whatsapp.desc");
    println!("cargo:rerun-if-changed=src/whatsapp.desc.sha256");
    println!("cargo:rerun-if-changed=src/whatsapp.proto");
    println!("cargo:rerun-if-changed=build.rs");

    ensure_proto_descriptor_hash()?;

    let out_dir = std::path::PathBuf::from(std::env::var_os("OUT_DIR").ok_or_else(|| {
        std::io::Error::other("OUT_DIR not set (cargo always sets it for build scripts)")
    })?);

    let fds =
        prost_types::FileDescriptorSet::decode(std::fs::read("src/whatsapp.desc")?.as_slice())
            .map_err(std::io::Error::other)?;

    let mut config = prost_build::Config::new();

    // Serialize always; Deserialize only for WASM bridge (halves serde codegen).
    config.type_attribute(".", "#[derive(serde::Serialize)]");
    config.type_attribute(
        ".",
        "#[cfg_attr(feature = \"serde-deserialize\", derive(serde::Deserialize))]",
    );
    // Default missing fields to match protobuf semantics (structs only).
    config.message_attribute(
        ".",
        "#[cfg_attr(feature = \"serde-deserialize\", serde(default))]",
    );

    // Accept snake_case on deserialization for WASM bridge enum variants.
    config.type_attribute(
        ".",
        "#[cfg_attr(feature = \"serde-snake-case\", serde(rename_all(deserialize = \"snake_case\")))]",
    );

    // O(1)-clone Bytes for hot-path crypto structures instead of Vec<u8>.
    config.bytes([
        ".whatsapp.SessionStructure.Chain.ChainKey",
        ".whatsapp.SessionStructure.Chain.MessageKey",
        ".whatsapp.SenderKeyStateStructure.SenderChainKey",
        ".whatsapp.SenderKeyStateStructure.SenderMessageKey",
        ".whatsapp.SenderKeyStateStructure.SenderSigningKey",
    ]);

    // Boxed: large (and mostly absent-on-the-wire) submessages whose inline
    // form makes prost's repeated-field decode memcpy-bound — every element
    // pays push(default) plus Vec-growth copies of the full struct size.
    config.boxed(".whatsapp.HistorySyncMsg.message");
    config.boxed(".whatsapp.WebMessageInfo.message");
    config.boxed(".whatsapp.WebMessageInfo.statusMentionMessageInfo");
    config.boxed(".whatsapp.Message.messageContextInfo");

    // Box the remaining inline message-typed fields so `wa::Message` — a union
    // of ~110 content variants of which exactly one is ever set — stops paying
    // for all of them inline. prost already boxes the variants in recursion
    // cycles; these are the rest. Shrinking the struct makes every clone,
    // decode, and `Arc<Message>` event cheaper to move and hold.
    for field in [
        "bcallMessage",
        "callLogMesssage",
        "cancelPaymentRequestMessage",
        "chat",
        "conditionalRevealMessage",
        "declinePaymentRequestMessage",
        "encCommentMessage",
        "encEventResponseMessage",
        "encReactionMessage",
        "groupRootKeyShare",
        "invoiceMessage",
        "keepInChatMessage",
        "paymentInviteMessage",
        "paymentReminderMessage",
        "pinInChatMessage",
        "placeholderMessage",
        "pollAddOptionMessage",
        "pollUpdateMessage",
        "questionResponseMessage",
        "reactionMessage",
        "rootSecretDistributeMessage",
        "scheduledCallCreationMessage",
        "scheduledCallEditMessage",
        "secretEncryptedMessage",
        "statusNotificationMessage",
        "statusQuestionAnswerMessage",
        "statusQuotedMessage",
        "statusStickerInteractionMessage",
        "stickerSyncRmrMessage",
    ] {
        config.boxed(format!(".whatsapp.Message.{field}").as_str());
    }

    // Bytes fields lack serde support; skip them (internal crypto state).
    config.field_attribute(
        ".whatsapp.SessionStructure.Chain.ChainKey.key",
        "#[serde(skip)]",
    );
    config.field_attribute(
        ".whatsapp.SessionStructure.Chain.MessageKey.cipherKey",
        "#[serde(skip)]",
    );
    config.field_attribute(
        ".whatsapp.SessionStructure.Chain.MessageKey.macKey",
        "#[serde(skip)]",
    );
    config.field_attribute(
        ".whatsapp.SessionStructure.Chain.MessageKey.iv",
        "#[serde(skip)]",
    );
    config.field_attribute(
        ".whatsapp.SenderKeyStateStructure.SenderChainKey.seed",
        "#[serde(skip)]",
    );
    config.field_attribute(
        ".whatsapp.SenderKeyStateStructure.SenderMessageKey.seed",
        "#[serde(skip)]",
    );
    config.field_attribute(
        ".whatsapp.SenderKeyStateStructure.SenderSigningKey.public",
        "#[serde(skip)]",
    );
    config.field_attribute(
        ".whatsapp.SenderKeyStateStructure.SenderSigningKey.private",
        "#[serde(skip)]",
    );

    config.out_dir(&out_dir);
    config.compile_fds(fds.clone())?;

    generate_tags(&fds, &out_dir.join("tags.rs"))
}

/// Generate `tags.rs`: one module per message carrying a `u32` const per
/// field with its wire tag, straight from the descriptor. Hand-written
/// partial decoders reference these consts (or compile-time assert against
/// them), so a schema change that renumbers, renames or removes a field breaks
/// the build instead of silently desyncing.
fn generate_tags(
    fds: &prost_types::FileDescriptorSet,
    out_path: &std::path::Path,
) -> std::io::Result<()> {
    use heck::{ToShoutySnakeCase, ToSnakeCase};
    use prost_types::DescriptorProto;

    /// prost-parity identifier sanitization (mirror of prost-build's
    /// `ident::sanitize_identifier` + `to_snake`), so module names always
    /// match what prost would generate for the same message.
    fn module_ident(name: &str) -> String {
        let snake = name.to_snake_case();
        match snake.as_str() {
            // Strict and reserved keywords across editions: raw identifier.
            "as" | "break" | "const" | "continue" | "else" | "enum" | "false" | "fn" | "for"
            | "if" | "impl" | "in" | "let" | "loop" | "match" | "mod" | "move" | "mut" | "pub"
            | "ref" | "return" | "static" | "struct" | "trait" | "true" | "type" | "unsafe"
            | "use" | "where" | "while" | "dyn" | "abstract" | "become" | "box" | "do"
            | "final" | "macro" | "override" | "priv" | "typeof" | "unsized" | "virtual"
            | "yield" | "async" | "await" | "try" | "gen" => format!("r#{snake}"),
            // Not usable as raw identifiers: underscore suffix.
            "_" | "super" | "self" | "crate" | "extern" => format!("{snake}_"),
            // Digit-leading names get an underscore prefix.
            other if other.starts_with(|c: char| c.is_numeric()) => format!("_{snake}"),
            _ => snake,
        }
    }

    fn emit_message(out: &mut String, msg: &DescriptorProto, indent: usize) {
        // Synthetic map-entry messages have no hand-decodable surface.
        if msg
            .options
            .as_ref()
            .and_then(|o| o.map_entry)
            .unwrap_or(false)
        {
            return;
        }
        let pad = "    ".repeat(indent);
        out.push_str(&format!("{pad}pub mod {} {{\n", module_ident(msg.name())));
        let mut seen = std::collections::HashSet::new();
        for field in &msg.field {
            let const_name = field.name().to_shouty_snake_case();
            // Two field names collapsing to one const (e.g. fooBar/foo_bar)
            // would emit duplicate consts; fail loudly at generation time.
            assert!(
                seen.insert(const_name.clone()),
                "tags.rs: const name collision `{const_name}` in message `{}`",
                msg.name()
            );
            out.push_str(&format!(
                "{pad}    pub const {const_name}: u32 = {};\n",
                field.number()
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

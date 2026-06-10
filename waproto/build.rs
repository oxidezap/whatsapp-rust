// # Updating the Proto File
//
// When modifying `src/whatsapp.proto`, follow these steps:
//
// 1. Format the proto file (requires `buf` CLI: https://buf.build/docs/installation):
//    ```
//    buf format waproto/src/whatsapp.proto -w
//    ```
//
// 2. Regenerate the Rust code:
//    ```
//    cargo build -p waproto --features generate
//    ```
//
// 3. Fix any breaking changes in the codebase (e.g., `optional` -> `required` field changes)

fn main() -> std::io::Result<()> {
    #[cfg(not(feature = "generate"))]
    {
        println!("cargo:rerun-if-changed=build.rs");
        Ok(())
    }

    #[cfg(feature = "generate")]
    {
        println!("cargo:rerun-if-changed=src/whatsapp.proto");
        println!("cargo:warning=Regenerating proto definitions...");

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

        // Output to src/ so generated code is version-controlled.
        config.out_dir("src/");

        let fds_path =
            std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("whatsapp_fds.bin");
        config.file_descriptor_set_path(&fds_path);

        config.compile_protos(&["src/whatsapp.proto"], &["src/"])?;
        generate_tags(&fds_path)?;
        Ok(())
    }
}

/// Generate `src/tags.rs`: one module per message carrying a `u32` const per
/// field with its wire tag, straight from the compiled descriptor. Hand-written
/// partial decoders reference these consts (or compile-time assert against
/// them), so a schema change that renumbers, renames or removes a field breaks
/// the build instead of silently desyncing.
#[cfg(feature = "generate")]
fn generate_tags(fds_path: &std::path::Path) -> std::io::Result<()> {
    use heck::{ToShoutySnakeCase, ToSnakeCase};
    use prost::Message;
    use prost_types::{DescriptorProto, FileDescriptorSet};

    fn module_ident(name: &str) -> String {
        let snake = name.to_snake_case();
        // Keep parity with prost's module naming for raw-identifier cases.
        if matches!(
            snake.as_str(),
            "as" | "box"
                | "break"
                | "const"
                | "continue"
                | "else"
                | "enum"
                | "fn"
                | "for"
                | "if"
                | "impl"
                | "in"
                | "let"
                | "loop"
                | "match"
                | "mod"
                | "move"
                | "mut"
                | "pub"
                | "ref"
                | "return"
                | "static"
                | "struct"
                | "trait"
                | "type"
                | "use"
                | "where"
                | "while"
        ) {
            format!("r#{snake}")
        } else {
            snake
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
        for field in &msg.field {
            out.push_str(&format!(
                "{pad}    pub const {}: u32 = {};\n",
                field.name().to_shouty_snake_case(),
                field.number()
            ));
        }
        for nested in &msg.nested_type {
            emit_message(out, nested, indent + 1);
        }
        out.push_str(&format!("{pad}}}\n"));
    }

    let fds = FileDescriptorSet::decode(std::fs::read(fds_path)?.as_slice())
        .map_err(std::io::Error::other)?;

    let mut out = String::with_capacity(1 << 20);
    out.push_str(
        "// @generated by waproto's build.rs (feature `generate`). Do not edit.\n\
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
    std::fs::write("src/tags.rs", out)
}

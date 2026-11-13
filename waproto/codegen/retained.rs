use crate::descriptor::{
    DescriptorProto, FieldDescriptorProto, FileDescriptorSet, field_descriptor_proto,
};

/// A field this crate persists that the upstream proto does not declare.
struct LocalField {
    /// Message path inside the `whatsapp` package, e.g. `Outer.Inner`.
    message: &'static str,
    name: &'static str,
    number: i32,
    kind: field_descriptor_proto::Type,
    type_name: Option<&'static str>,
}

/// Local additions to the upstream schema, spliced into the descriptor at
/// build time. `src/whatsapp.proto` and `src/whatsapp.desc` are regenerated
/// wholesale from whatspec, so a field declared there would be lost on the
/// next sync; declared here it survives, and a sync that lands on one of these
/// numbers fails the build instead of silently reinterpreting records already
/// written.
///
/// New local fields use high numbers. Retained upstream fields keep their
/// original wire numbers; either kind must stop on a future collision.
const LOCAL_FIELDS: &[LocalField] = &[
    LocalField {
        // Local persisted retention policy for a bounded imported backlog.
        // Never alters the network forward-jump limit.
        message: "SessionStructure.Chain",
        name: "messageKeyLimit",
        number: 100,
        kind: field_descriptor_proto::Type::TYPE_UINT32,
        type_name: None,
    },
    LocalField {
        // Deriving the stored cipher/mac/iv from this seed is one-way, so a
        // skipped message key that kept only the derived material could never be
        // projected back into a seed-based external format.
        message: "SessionStructure.Chain.MessageKey",
        name: "seed",
        number: 100,
        kind: field_descriptor_proto::Type::TYPE_BYTES,
        type_name: None,
    },
    LocalField {
        // Public field from whatspec 1a441f0. Its absence in the next Web
        // capture does not make already stored or received payloads obsolete.
        message: "Message",
        name: "newsletterAdminProfileMessageV2",
        number: 117,
        kind: field_descriptor_proto::Type::TYPE_MESSAGE,
        type_name: Some(".whatsapp.Message.FutureProofMessage"),
    },
];

/// Splice [`LOCAL_FIELDS`] into the descriptor decoded from `whatsapp.desc`.
///
/// Fails rather than resolves when upstream already occupies the name or the
/// number: silently winning that race would change what an existing field
/// decodes as, and every record already on disk with it.
pub fn apply_local_fields(fds: &mut FileDescriptorSet) -> std::io::Result<()> {
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
            type_name: local.type_name.map(str::to_owned),
            json_name: Some(local.name.to_owned()),
            ..Default::default()
        });
    }
    Ok(())
}

pub fn tags(fds: &FileDescriptorSet) -> String {
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
            .as_ref()
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
        "// @generated from whatsapp.desc by protobuf-codegen. Do not edit.\n\
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
    out
}

//! Deliberate protobuf-name compatibility overrides. Wire numbers are identities
//! for fields and enum values; an upstream spelling is not the Rust API identity.
//! Keep rules here after a rename. Never infer a rename from a similar name.

use buffa_descriptor::generated::descriptor::{
    DescriptorProto, EnumDescriptorProto, FileDescriptorSet,
};
use buffa_descriptor::{features, features::ResolvedFeatures};
use std::io;

pub struct TypeName<'a> {
    pub upstream: &'a str,
    pub frozen: &'a str,
}

pub struct FieldName<'a> {
    /// Fully qualified upstream message name, after type-name overrides.
    pub message: &'a str,
    pub number: i32,
    pub upstream: &'a str,
    pub frozen: &'a str,
}

pub struct OneofName<'a> {
    pub message: &'a str,
    pub upstream: &'a str,
    pub frozen: &'a str,
    /// Previously published alternatives, identified by their wire numbers.
    pub numbers: &'a [i32],
}

pub struct EnumName<'a> {
    pub enumeration: &'a str,
    pub number: i32,
    pub upstream: &'a str,
    pub frozen: &'a str,
}

// Empty at the initial freeze. Populate only after confirming the upstream
// declaration's wire identity and semantics; the API guard blocks silent drift.
pub const TYPES: &[TypeName<'_>] = &[];
pub const FIELDS: &[FieldName<'_>] = &[];
pub const ONEOFS: &[OneofName<'_>] = &[];
pub const ENUM_VALUES: &[EnumName<'_>] = &[];

pub fn apply(
    fds: &mut FileDescriptorSet,
    types: &[TypeName<'_>],
    fields: &[FieldName<'_>],
    values: &[EnumName<'_>],
    oneofs: &[OneofName<'_>],
) -> io::Result<()> {
    for rule in types {
        let (old_parent, old_leaf) = rule
            .upstream
            .rsplit_once('.')
            .ok_or_else(|| invalid("qualified upstream type required"))?;
        let (new_parent, new_leaf) = rule
            .frozen
            .rsplit_once('.')
            .ok_or_else(|| invalid("qualified frozen type required"))?;
        if old_parent != new_parent {
            return Err(invalid(
                "type-name overrides cannot move a declaration to another scope",
            ));
        }
        let mut seen = 0;
        for file in &mut fds.file {
            let scope = format!(".{}", file.package.as_deref().unwrap_or_default());
            rename_type(
                &mut file.message_type,
                &mut file.enum_type,
                &scope,
                old_parent,
                old_leaf,
                new_leaf,
                &mut seen,
            )?;
        }
        if seen != 1 {
            return Err(invalid(&format!(
                "type override {} matched {seen} declarations",
                rule.upstream
            )));
        }
        for file in &mut fds.file {
            rewrite_refs(&mut file.message_type, rule.upstream, rule.frozen);
        }
    }
    for rule in fields {
        let message = find_message(fds, rule.message)
            .ok_or_else(|| invalid(&format!("field-name target {} missing", rule.message)))?;
        if message
            .field
            .iter()
            .any(|f| f.name.as_deref() == Some(rule.frozen) && f.number != Some(rule.number))
        {
            return Err(invalid(
                "frozen field name collides with another wire number",
            ));
        }
        let field = message
            .field
            .iter_mut()
            .find(|f| f.number == Some(rule.number))
            .ok_or_else(|| invalid("frozen field wire number missing"))?;
        if field.name.as_deref() != Some(rule.upstream) {
            return Err(invalid(
                "upstream field name no longer matches the reviewed override",
            ));
        }
        // json_name remains the descriptor's explicit upstream JSON spelling.
        // Derived-serde bridge names remain the frozen Rust field spelling.
        field.name = Some(rule.frozen.to_owned());
    }
    for rule in oneofs {
        let message =
            find_message(fds, rule.message).ok_or_else(|| invalid("oneof-name target missing"))?;
        if message
            .oneof_decl
            .iter()
            .any(|oneof| oneof.name.as_deref() == Some(rule.frozen))
        {
            return Err(invalid("frozen oneof name is already occupied"));
        }
        let index = message
            .oneof_decl
            .iter()
            .position(|oneof| oneof.name.as_deref() == Some(rule.upstream))
            .ok_or_else(|| invalid("reviewed upstream oneof name missing"))?;
        for number in rule.numbers {
            if !message.field.iter().any(|field| {
                field.number == Some(*number) && field.oneof_index == Some(index as i32)
            }) {
                return Err(invalid(
                    "oneof-name override lost a frozen wire alternative",
                ));
            }
        }
        message.oneof_decl[index].name = Some(rule.frozen.to_owned());
    }
    for rule in values {
        let mut seen = 0;
        for file in &mut fds.file {
            let scope = format!(".{}", file.package.as_deref().unwrap_or_default());
            walk_enums(
                &mut file.message_type,
                &mut file.enum_type,
                &scope,
                &mut |path, enumeration| {
                    if path != rule.enumeration {
                        return Ok(());
                    }
                    if enumeration.value.iter().any(|v| {
                        v.name.as_deref() == Some(rule.frozen) && v.number != Some(rule.number)
                    }) {
                        return Err(invalid(
                            "frozen enum value name collides with another wire number",
                        ));
                    }
                    let value = enumeration
                        .value
                        .iter_mut()
                        .find(|v| {
                            v.name.as_deref() == Some(rule.upstream)
                                && v.number == Some(rule.number)
                        })
                        .ok_or_else(|| {
                            invalid("enum-name override no longer matches its reviewed name/number")
                        })?;
                    value.name = Some(rule.frozen.to_owned());
                    seen += 1;
                    Ok(())
                },
            )?;
        }
        if seen != 1 {
            return Err(invalid("enum-name override target missing or ambiguous"));
        }
        fn defaults(messages: &mut [DescriptorProto], rule: &EnumName<'_>) {
            for message in messages {
                for field in &mut message.field {
                    if field.type_name.as_deref() == Some(rule.enumeration)
                        && field.default_value.as_deref() == Some(rule.upstream)
                    {
                        field.default_value = Some(rule.frozen.to_owned());
                    }
                }
                defaults(&mut message.nested_type, rule);
            }
        }
        for file in &mut fds.file {
            defaults(&mut file.message_type, rule);
        }
    }
    Ok(())
}

fn invalid(message: &str) -> io::Error {
    io::Error::other(message)
}

fn rename_type(
    messages: &mut [DescriptorProto],
    enums: &mut [EnumDescriptorProto],
    scope: &str,
    parent: &str,
    old: &str,
    new: &str,
    seen: &mut usize,
) -> io::Result<()> {
    if scope == parent {
        if messages.iter().any(|m| m.name.as_deref() == Some(new))
            || enums.iter().any(|e| e.name.as_deref() == Some(new))
        {
            return Err(invalid("frozen type name is already occupied"));
        }
        for m in messages.iter_mut() {
            if m.name.as_deref() == Some(old) {
                m.name = Some(new.to_owned());
                *seen += 1;
            }
        }
        for e in enums {
            if e.name.as_deref() == Some(old) {
                e.name = Some(new.to_owned());
                *seen += 1;
            }
        }
    }
    for message in messages {
        let child = format!("{scope}.{}", message.name.as_deref().unwrap_or_default());
        rename_type(
            &mut message.nested_type,
            &mut message.enum_type,
            &child,
            parent,
            old,
            new,
            seen,
        )?;
    }
    Ok(())
}

fn rewrite_refs(messages: &mut [DescriptorProto], from: &str, to: &str) {
    for message in messages {
        for field in &mut message.field {
            for reference in [&mut field.type_name, &mut field.extendee]
                .into_iter()
                .flatten()
            {
                if reference == from
                    || reference
                        .strip_prefix(from)
                        .is_some_and(|s| s.starts_with('.'))
                {
                    *reference = format!("{to}{}", &reference[from.len()..]);
                }
            }
        }
        rewrite_refs(&mut message.nested_type, from, to);
    }
}

fn find_message<'a>(fds: &'a mut FileDescriptorSet, path: &str) -> Option<&'a mut DescriptorProto> {
    fn descend<'a>(
        messages: &'a mut [DescriptorProto],
        scope: &str,
        path: &str,
    ) -> Option<&'a mut DescriptorProto> {
        for m in messages {
            let current = format!("{scope}.{}", m.name.as_deref()?);
            if current == path {
                return Some(m);
            }
            if path
                .strip_prefix(&current)
                .is_some_and(|s| s.starts_with('.'))
            {
                return descend(&mut m.nested_type, &current, path);
            }
        }
        None
    }
    for file in &mut fds.file {
        let scope = format!(".{}", file.package.as_deref().unwrap_or_default());
        if let Some(found) = descend(&mut file.message_type, &scope, path) {
            return Some(found);
        }
    }
    None
}

fn walk_enums(
    messages: &mut [DescriptorProto],
    enums: &mut [EnumDescriptorProto],
    scope: &str,
    visitor: &mut impl FnMut(&str, &mut EnumDescriptorProto) -> io::Result<()>,
) -> io::Result<()> {
    for enumeration in enums {
        visitor(
            &format!(
                "{scope}.{}",
                enumeration.name.as_deref().unwrap_or_default()
            ),
            enumeration,
        )?;
    }
    for message in messages {
        let child = format!("{scope}.{}", message.name.as_deref().unwrap_or_default());
        walk_enums(
            &mut message.nested_type,
            &mut message.enum_type,
            &child,
            visitor,
        )?;
    }
    Ok(())
}

/// Rust signatures alone cannot distinguish e.g. int32 from sint32, or a
/// required message field from an optional one using the same MessageField.
/// Freeze those encoding/presence choices alongside the emitted API.
pub fn wire_api(fds: &FileDescriptorSet) -> std::collections::BTreeSet<String> {
    fn messages(
        items: &[DescriptorProto],
        scope: &str,
        parent_features: &ResolvedFeatures,
        out: &mut std::collections::BTreeSet<String>,
    ) {
        use buffa_descriptor::generated::descriptor::field_descriptor_proto::{Label, Type};
        for message in items {
            let path = format!("{scope}.{}", message.name.as_deref().unwrap_or_default());
            let message_features =
                features::resolve_child(parent_features, features::message_features(message));
            for field in &message.field {
                let oneof = field
                    .oneof_index
                    .and_then(|index| message.oneof_decl.get(index as usize))
                    .and_then(|oneof| oneof.name.as_deref());
                out.insert(format!(
                    "wire {path}.{} number={:?} type={:?} label={:?} target={:?} default={:?} oneof={oneof:?}",
                    field.name.as_deref().unwrap_or_default(), field.number, field.r#type, field.label, field.type_name, field.default_value,
                ));
                if field.label == Some(Label::LABEL_REPEATED)
                    && !matches!(
                        field.r#type,
                        None | Some(
                            Type::TYPE_STRING
                                | Type::TYPE_BYTES
                                | Type::TYPE_MESSAGE
                                | Type::TYPE_GROUP
                        )
                    )
                {
                    // Use the generator's feature resolver for syntax defaults
                    // and inherited editions settings. Legacy packed wins.
                    let resolved =
                        features::resolve_child(&message_features, features::field_features(field));
                    let packed = field
                        .options
                        .as_option()
                        .and_then(|options| options.packed)
                        .unwrap_or(
                            resolved.repeated_field_encoding
                                == features::RepeatedFieldEncoding::Packed,
                        );
                    out.insert(format!(
                        "packed {path}.{}={packed}",
                        field.name.as_deref().unwrap_or_default(),
                    ));
                }
            }
            messages(&message.nested_type, &path, &message_features, out);
        }
    }
    let mut out = std::collections::BTreeSet::new();
    for file in &fds.file {
        messages(
            &file.message_type,
            &format!(".{}", file.package.as_deref().unwrap_or_default()),
            &features::for_file(file),
            &mut out,
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use buffa_descriptor::generated::descriptor::{
        FeatureSet, FieldDescriptorProto, FieldOptions, FileDescriptorProto, MessageOptions,
        feature_set::RepeatedFieldEncoding,
        field_descriptor_proto::{Label, Type},
    };

    fn fixture(syntax: &str, packed: Option<bool>) -> FileDescriptorSet {
        FileDescriptorSet {
            file: vec![FileDescriptorProto {
                package: Some("contract".into()),
                syntax: Some(syntax.into()),
                message_type: vec![DescriptorProto {
                    name: Some("Record".into()),
                    field: vec![FieldDescriptorProto {
                        name: Some("values".into()),
                        number: Some(1),
                        label: Some(Label::LABEL_REPEATED),
                        r#type: Some(Type::TYPE_UINT32),
                        options: buffa::MessageField::some(FieldOptions {
                            packed,
                            ..Default::default()
                        }),
                        ..Default::default()
                    }],
                    ..Default::default()
                }],
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    #[test]
    fn packed_inventory_resolves_defaults_and_overrides() {
        for (syntax, explicit, packed) in [
            ("proto2", None, false),
            ("proto3", None, true),
            ("proto2", Some(true), true),
            ("proto3", Some(false), false),
        ] {
            let api = wire_api(&fixture(syntax, explicit));
            assert!(api.contains(&format!("packed .contract.Record.values={packed}")));
        }
        let mut nested = fixture("proto3", None);
        let mut child = nested.file[0].message_type.pop().unwrap();
        child.name = Some("Child".into());
        nested.file[0].message_type.push(DescriptorProto {
            name: Some("Parent".into()),
            options: buffa::MessageField::some(MessageOptions {
                features: buffa::MessageField::some(FeatureSet {
                    repeated_field_encoding: Some(RepeatedFieldEncoding::EXPANDED),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            nested_type: vec![child],
            ..Default::default()
        });
        assert!(wire_api(&nested).contains("packed .contract.Parent.Child.values=false"));
        nested.file[0].message_type[0].nested_type[0].field[0]
            .options
            .as_option_mut()
            .unwrap()
            .packed = Some(true);
        assert!(wire_api(&nested).contains("packed .contract.Parent.Child.values=true"));
    }

    #[test]
    fn changing_packed_encoding_fails_api_compatibility() {
        let baseline = wire_api(&fixture("proto2", Some(true)));
        let expected = baseline.iter().cloned().collect::<Vec<_>>().join("\n");
        super::super::emission::check_api(&expected, &wire_api(&fixture("proto2", None)))
            .expect_err("removing packed changes encoded wire bytes");
        super::super::emission::check_api(&expected, &wire_api(&fixture("proto3", None)))
            .expect("equivalent effective packing remains compatible");
    }
}

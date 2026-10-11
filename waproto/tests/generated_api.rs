//! Generated source compatibility and exhaustive Signal schema guards.

#[allow(dead_code)]
#[path = "../build_support/emission.rs"]
mod emission;
#[path = "../build_support/evolution.rs"]
mod evolution;
#[allow(dead_code)]
#[path = "../build_support/names.rs"]
mod names;
#[path = "../build_support/signal_storage.rs"]
mod signal_storage;

use std::path::PathBuf;

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn signal_projection_guard_rejects_additive_known_fields() {
    use buffa::Message as _;
    use buffa_descriptor::generated::descriptor::{
        FieldDescriptorProto, FileDescriptorSet, field_descriptor_proto,
    };

    let baseline: std::collections::BTreeSet<String> = include_str!("../api.snapshot")
        .lines()
        .map(str::to_owned)
        .collect();
    signal_storage::check(&baseline).expect("current Signal projections match");
    for (root, nested) in [
        ("IdentityKeyPairStructure", &[][..]),
        ("SessionStructure", &[][..]),
        ("SessionStructure", &["Chain", "MessageKey"][..]),
        ("SenderKeyStateStructure", &[][..]),
        ("SenderKeyStateStructure", &["SenderChainKey"][..]),
        ("SenderKeyStateStructure", &["SenderMessageKey"][..]),
        ("PreKeyRecordStructure", &[][..]),
    ] {
        // Descriptor fixtures have no application-message wrapper in waproto::codec.
        #[allow(clippy::disallowed_methods)]
        let mut descriptor =
            FileDescriptorSet::decode_from_slice(include_bytes!("../src/whatsapp.desc"))
                .expect("committed descriptor");
        let mut message = descriptor
            .file
            .iter_mut()
            .flat_map(|file| &mut file.message_type)
            .find(|message| message.name.as_deref() == Some(root))
            .expect("persisted record");
        for nested in nested {
            message = message
                .nested_type
                .iter_mut()
                .find(|message| message.name.as_deref() == Some(nested))
                .expect("nested persisted record");
        }
        // A present-empty message must not disappear through semantic equality
        // or a projection unaware of this new generated field.
        message.field.push(FieldDescriptorProto {
            name: Some("futureState".into()),
            number: Some(1000),
            label: Some(field_descriptor_proto::Label::LABEL_OPTIONAL),
            r#type: Some(field_descriptor_proto::Type::TYPE_MESSAGE),
            type_name: Some(".whatsapp.SessionStructure".into()),
            ..Default::default()
        });
        let mut candidate = baseline.clone();
        candidate.extend(names::wire_api(&descriptor));
        emission::check_api(include_str!("../api.snapshot"), &candidate)
            .expect("general public API deliberately permits additions");
        let error = signal_storage::check(&candidate)
            .expect_err("Signal additions require coordinated projection changes");
        assert!(error.to_string().contains("futureState"));
    }
}

#[test]
fn signal_projection_guard_allows_unrelated_schema_evolution() {
    let mut candidate = include_str!("../api.snapshot")
        .lines()
        .map(str::to_owned)
        .collect::<std::collections::BTreeSet<_>>();
    candidate.insert("wire .whatsapp.Message.futureState number=Some(1000)".into());
    signal_storage::check(&candidate).expect("ordinary message evolution remains additive");
    candidate.retain(|line| !line.starts_with("wire .whatsapp.SessionStructure.rootKey "));
    signal_storage::check(&candidate).expect_err("removing represented state must also fail");
}

#[test]
fn generated_schema_evolution_preserves_source_contract() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let scratch = Scratch(
        root.join("tests")
            .join(format!(".api-fixture-{}", std::process::id())),
    );
    evolution::generate(&scratch.0).expect("generate additive and incompatible schema fixtures");
}

#[test]
fn history_whitelist_covers_current_generated_fields() {
    use std::collections::{BTreeMap, BTreeSet};
    use syn::parse::{Parse, ParseStream};
    use syn::punctuated::Punctuated;
    use syn::{Ident, Token, bracketed};

    struct Rule(Ident);
    impl Parse for Rule {
        fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
            let field = input.parse()?;
            input.parse::<Token![=>]>()?;
            input.parse::<Ident>()?;
            Ok(Self(field))
        }
    }
    struct Whitelist {
        ty: Ident,
        fields: BTreeSet<String>,
    }
    impl Parse for Whitelist {
        fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
            input.parse::<Ident>()?;
            input.parse::<Token![,]>()?;
            let ty = input.parse()?;
            input.parse::<Token![,]>()?;
            let allowed;
            bracketed!(allowed in input);
            let allowed = Punctuated::<Ident, Token![,]>::parse_terminated(&allowed)?;
            input.parse::<Token![,]>()?;
            let forbidden;
            bracketed!(forbidden in input);
            let forbidden = Punctuated::<Rule, Token![,]>::parse_terminated(&forbidden)?;
            let fields = allowed
                .iter()
                .map(ToString::to_string)
                .chain(forbidden.iter().map(|rule| rule.0.to_string()))
                .collect();
            Ok(Self { ty, fields })
        }
    }
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../src/features/group_history.rs");
    if !path.is_file() {
        eprintln!(
            "workspace-only history whitelist gate not executed: SDK source is outside the waproto package"
        );
        return;
    }
    let source = syn::parse_file(&std::fs::read_to_string(path).expect("history source"))
        .expect("history source syntax");
    let mut whitelists = BTreeMap::new();
    for item in source.items {
        let syn::Item::Macro(item) = item else {
            continue;
        };
        if !item.mac.path.is_ident("history_field_whitelist") {
            continue;
        }
        let whitelist: Whitelist = syn::parse2(item.mac.tokens).expect("whitelist grammar");
        assert!(
            whitelists
                .insert(whitelist.ty.to_string(), whitelist.fields)
                .is_none()
        );
    }
    assert_eq!(whitelists.len(), 2);
    for ty in ["Message", "MessageContextInfo"] {
        let prefix = format!("field whatsapp::{ty}::");
        let generated: BTreeSet<_> = include_str!(concat!(env!("OUT_DIR"), "/api.snapshot"))
            .lines()
            .filter_map(|line| line.strip_prefix(&prefix))
            .map(|line| {
                line.split_whitespace()
                    .next()
                    .expect("field name")
                    .to_owned()
            })
            .collect();
        assert_eq!(
            generated, whitelists[ty],
            "review history whitelist after schema changes: {ty}"
        );
    }
}

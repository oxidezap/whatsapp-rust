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

// Keep AST dependency in this build-tool test crate; pairing runtime needs none.
fn explicit_platform_variants(source: &str) -> std::collections::BTreeSet<String> {
    fn collect(pattern: &syn::Pat, names: &mut std::collections::BTreeSet<String>) {
        match pattern {
            syn::Pat::Path(path) => {
                assert_eq!(path.path.segments.len(), 2);
                assert_eq!(path.path.segments[0].ident, "P");
                names.insert(path.path.segments[1].ident.to_string());
            }
            syn::Pat::Or(pattern) => {
                for case in &pattern.cases {
                    collect(case, names);
                }
            }
            syn::Pat::Guard(_) => panic!("conditional arms do not cover a platform"),
            syn::Pat::Wild(_) => {}
            _ => panic!("unsupported platform mapping pattern"),
        }
    }
    let file = syn::parse_file(source).expect("pairing source parses");
    let function = file
        .items
        .iter()
        .find_map(|item| match item {
            syn::Item::Fn(function)
                if function.sig.ident == "companion_web_client_type_for_platform" =>
            {
                Some(function)
            }
            _ => None,
        })
        .expect("platform mapping exists");
    let mapping = function
        .block
        .stmts
        .iter()
        .find_map(|statement| match statement {
            syn::Stmt::Expr(syn::Expr::Match(mapping), _) => Some(mapping),
            _ => None,
        })
        .expect("platform mapping retains explicit match");
    assert!(
        mapping
            .arms
            .iter()
            .any(|arm| matches!(arm.pat, syn::Pat::Wild(_))),
        "non-exhaustive enum requires the wildcard arm"
    );
    let mut names = std::collections::BTreeSet::new();
    for arm in &mapping.arms {
        collect(&arm.pat, &mut names);
    }
    names
}

#[test]
fn platform_mapping_covers_every_generated_variant() {
    use buffa::Enumeration;
    use waproto::whatsapp::device_props::PlatformType;
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../wacore/src/companion_reg.rs");
    let source = match std::fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            eprintln!(
                "workspace-only platform mapping gate NOT_EXECUTED: wacore source is outside the waproto package"
            );
            return;
        }
        Err(error) => panic!("read platform mapping source: {error}"),
    };
    let known: std::collections::BTreeSet<String> = PlatformType::values()
        .iter()
        .map(|platform| platform.proto_name().to_owned())
        .collect();
    assert!(!known.is_empty());
    let explicit = explicit_platform_variants(&source);
    assert_eq!(
        explicit, known,
        "every current platform needs a deliberate mapping"
    );

    // A wildcard still handles these values, so result-only tests cannot detect
    // a forgotten arm. Inspect the arms against generated metadata instead.
    let removed = source.replace("P::CHROME => C::Chrome,", "");
    assert_ne!(removed, source);
    let missing = explicit_platform_variants(&removed);
    assert!(!missing.contains("CHROME"));
    assert_ne!(missing, known);
    let conditional = source.replace("P::CHROME => C::Chrome,", "P::CHROME if true => C::Chrome,");
    assert_ne!(conditional, source);
    assert!(std::panic::catch_unwind(|| explicit_platform_variants(&conditional)).is_err());
    let mut future = known;
    future.insert("FUTURE_BROWSER".to_owned());
    assert_ne!(
        explicit, future,
        "an additive enum variant must require a mapping decision"
    );
}

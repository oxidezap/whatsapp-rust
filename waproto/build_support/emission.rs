//! Compatibility policy applied to buffa's output, including views, for which
//! buffa 0.9 does not provide attribute hooks. This is part of the emitter, not
//! a source-file patch: every build applies it to freshly generated syntax.

use std::collections::BTreeSet;
use std::io;
use std::path::Path;

use quote::ToTokens;
use syn::visit_mut::{self, VisitMut};

struct Extensible {
    serde: bool,
}

impl VisitMut for Extensible {
    fn visit_item_struct_mut(&mut self, item: &mut syn::ItemStruct) {
        if matches!(item.vis, syn::Visibility::Public(_)) {
            protect(&mut item.attrs);
            // Unknown protobuf data is retained on the wire, but was never a
            // member of the bridge's derived-serde object contract.
            if self.serde {
                for field in &mut item.fields {
                    if field
                        .ident
                        .as_ref()
                        .is_some_and(|id| id == "__buffa_unknown_fields")
                    {
                        field.attrs.push(syn::parse_quote!(#[serde(skip)]));
                    }
                }
            }
        }
        visit_mut::visit_item_struct_mut(self, item);
    }

    fn visit_item_enum_mut(&mut self, item: &mut syn::ItemEnum) {
        if matches!(item.vis, syn::Visibility::Public(_)) {
            protect(&mut item.attrs);
        }
        visit_mut::visit_item_enum_mut(self, item);
    }
}

// Protobuf singular fields and oneofs use the last value on the wire. Retained
// future values must precede typed fields so a caller's explicit edit wins when
// a newer reader recognizes both. Apply this to owned messages and views.
struct UnknownFieldsFirst;

impl VisitMut for UnknownFieldsFirst {
    fn visit_impl_item_fn_mut(&mut self, method: &mut syn::ImplItemFn) {
        if method.sig.ident == "write_to"
            && let Some(index) = method.block.stmts.iter().position(|statement| {
                matches!(statement,
                    syn::Stmt::Expr(syn::Expr::MethodCall(call), _)
                    if call.method == "write_to"
                        && matches!(&*call.receiver, syn::Expr::Field(field)
                            if matches!(&field.member, syn::Member::Named(name)
                                if name == "__buffa_unknown_fields")))
            })
        {
            let unknown = method.block.stmts.remove(index);
            method.block.stmts.insert(0, unknown);
        }
        visit_mut::visit_impl_item_fn_mut(self, method);
    }
}

struct ColdStorage {
    depth: usize,
}
impl VisitMut for ColdStorage {
    fn visit_item_mod_mut(&mut self, item: &mut syn::ItemMod) {
        self.depth += 1;
        visit_mut::visit_item_mod_mut(self, item);
        self.depth -= 1;
    }
    fn visit_field_mut(&mut self, field: &mut syn::Field) {
        if field
            .ident
            .as_ref()
            .is_some_and(|name| name == "__buffa_unknown_fields")
        {
            let prefix = "super::".repeat(self.depth);
            field.ty = syn::parse_str(&format!("{prefix}__unknown_storage::Storage"))
                .expect("internal storage path");
        }
        visit_mut::visit_field_mut(self, field);
    }
    fn visit_expr_mut(&mut self, expr: &mut syn::Expr) {
        visit_mut::visit_expr_mut(self, expr);
        let syn::Expr::MethodCall(call) = expr else {
            return;
        };
        if call.method != "push" || call.args.len() != 1 {
            return;
        }
        let syn::Expr::Field(field) = &*call.receiver else {
            return;
        };
        if !matches!(&field.member, syn::Member::Named(name) if name == "__buffa_unknown_fields") {
            return;
        }
        let syn::Expr::Try(attempt) = &call.args[0] else {
            return;
        };
        let syn::Expr::Call(decode) = &*attempt.expr else {
            return;
        };
        let syn::Expr::Path(path) = &*decode.func else {
            return;
        };
        if !path
            .path
            .segments
            .last()
            .is_some_and(|p| p.ident == "decode_unknown_field")
        {
            return;
        }
        let receiver = &call.receiver;
        let args = &decode.args;
        *expr = syn::parse_quote!(#receiver.merge_unknown(#args)?);
    }
}

fn protect(attrs: &mut Vec<syn::Attribute>) {
    if !attrs.iter().any(|a| a.path().is_ident("non_exhaustive")) {
        attrs.push(syn::parse_quote!(#[non_exhaustive]));
    }
}

// Share nonempty codecs across parents, including wrappers with few fields.
// Field count alone misses the code duplicated by nested message fields.
fn share_codecs(items: &mut [syn::Item]) {
    let nonempty: BTreeSet<_> = items
        .iter()
        .filter_map(|item| {
            let syn::Item::Struct(item) = item else {
                return None;
            };
            item.fields
                .iter()
                .any(|field| {
                    field
                        .ident
                        .as_ref()
                        .is_none_or(|id| id != "__buffa_unknown_fields")
                })
                .then(|| item.ident.clone())
        })
        .collect();
    for item in items {
        match item {
            syn::Item::Mod(module) => {
                if let Some((_, items)) = &mut module.content {
                    share_codecs(items);
                }
            }
            syn::Item::Impl(item) => {
                let Some((_, trait_path, _)) = &item.trait_ else {
                    continue;
                };
                if !trait_path
                    .segments
                    .last()
                    .is_some_and(|s| s.ident == "Message")
                {
                    continue;
                }
                let syn::Type::Path(ty) = &*item.self_ty else {
                    continue;
                };
                if !ty.path.get_ident().is_some_and(|id| nonempty.contains(id)) {
                    continue;
                }
                for member in &mut item.items {
                    if let syn::ImplItem::Fn(method) = member
                        && (method.sig.ident == "compute_size"
                            || method.sig.ident == "write_to"
                            || method.sig.ident == "merge_field")
                    {
                        method.attrs.retain(|attr| !attr.path().is_ident("inline"));
                        method.attrs.push(syn::parse_quote!(#[inline(never)]));
                    }
                }
            }
            _ => {}
        }
    }
}

fn share_message_impls(items: &mut Vec<syn::Item>, scope: &str) {
    let mut implementations = Vec::new();
    for item in items.iter_mut() {
        if let syn::Item::Mod(module) = item
            && let Some((_, children)) = &mut module.content
        {
            share_message_impls(children, &format!("{scope}::{}", module.ident));
        }
        let syn::Item::Struct(message) = item else {
            continue;
        };
        let name = message.ident.to_string();
        let selected = match scope {
            "" => [
                "Message",
                "ContextInfo",
                "BotMetadata",
                "MessageContextInfo",
                "AIRichResponseSubMessage",
                "SyncActionValue",
                "WebMessageInfo",
            ]
            .contains(&name.as_str()),
            "::message" => [
                "ImageMessage",
                "VideoMessage",
                "InteractiveMessage",
                "HighlyStructuredMessage",
                "ProtocolMessage",
                "ExtendedTextMessage",
            ]
            .contains(&name.as_str()),
            _ => false,
        };
        if !selected {
            continue;
        }
        let mut derived_default = false;
        for attribute in &mut message.attrs {
            if attribute.path().is_ident("derive") {
                let traits = attribute
                    .parse_args_with(
                        syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
                    )
                    .expect("derive list");
                let traits: Vec<_> = traits
                    .into_iter()
                    .filter(|p| {
                        if p.is_ident("Default") {
                            derived_default = true;
                            false
                        } else {
                            !p.is_ident("Clone")
                        }
                    })
                    .collect();
                *attribute = syn::parse_quote!(#[derive(#(#traits),*)]);
            }
        }
        let name = &message.ident;
        let fields: Vec<_> = message
            .fields
            .iter()
            .map(|field| field.ident.as_ref().expect("named protobuf field"))
            .collect();
        implementations.push(syn::parse_quote! {
            impl ::core::clone::Clone for #name {
                #[inline(never)]
                fn clone(&self) -> Self {
                    Self { #(#fields: ::core::clone::Clone::clone(&self.#fields)),* }
                }
            }
        });
        // Only replace a derived Default: generator-provided protobuf defaults
        // can differ from each field's Rust default and must remain intact.
        if derived_default {
            implementations.push(syn::parse_quote! {
                impl ::core::default::Default for #name {
                    #[inline(never)]
                    fn default() -> Self {
                        Self { #(#fields: ::core::default::Default::default()),* }
                    }
                }
            });
        }
    }
    items.extend(implementations);
}

pub fn finish(out: &Path, package: &str) -> io::Result<BTreeSet<String>> {
    let mut api = BTreeSet::new();
    for (suffix, serde) in [
        ("", true),
        (".__oneof", false),
        (".__view", false),
        (".__view_oneof", false),
        (".mod", false),
    ] {
        let path = out.join(format!("{package}{suffix}.rs"));
        let source = std::fs::read_to_string(&path)?;
        let mut file = syn::parse_file(&source).map_err(io::Error::other)?;
        Extensible { serde }.visit_file_mut(&mut file);
        UnknownFieldsFirst.visit_file_mut(&mut file);
        if serde {
            share_codecs(&mut file.items);
            ColdStorage { depth: 0 }.visit_file_mut(&mut file);
            let body = syn::parse_file(include_str!("unknown_storage.rs")).expect("storage syntax");
            let body = body.items;
            file.items
                .push(syn::parse_quote!(#[doc(hidden)] pub mod __unknown_storage { #(#body)* }));
        }

        if serde {
            share_message_impls(&mut file.items, "");
        }
        let implementation = match suffix {
            ".__oneof" => "::__buffa::oneof",
            ".__view" => "::__buffa::view",
            ".__view_oneof" => "::__buffa::view::oneof",
            _ => "",
        };
        inventory(&file.items, &format!("{package}{implementation}"), &mut api);
        let emitted = format!(
            "// @generated by buffa and waproto's compatibility emitter. Do not edit.\n{}",
            prettyplease::unparse(&file)
        );
        std::fs::write(path, emitted)?;
    }
    let tags = out.join("tags.rs");
    if tags.exists() {
        let file = syn::parse_file(&std::fs::read_to_string(tags)?).map_err(io::Error::other)?;
        inventory(&file.items, "tags", &mut api);
    }
    Ok(api)
}

/// Each entry describes one independently additive API item. An extra field,
/// variant or method cannot invalidate an existing entry. A renamed or removed
/// item, changed ownership/type, or changed enum number does.
fn inventory(items: &[syn::Item], scope: &str, api: &mut BTreeSet<String>) {
    let tokens = |value: &dyn ToTokens| normalize(&value.to_token_stream().to_string(), scope);
    let public_scope = canonical(scope.split("::").map(str::to_owned).collect(), false);
    for item in items {
        match item {
            syn::Item::Mod(m) if !m.ident.to_string().starts_with("__") => {
                // Keep owned/view source scopes distinct: their canonical
                // public paths overlap, but either module can lose visibility.
                api.insert(format!("module {scope}::{} {}", m.ident, tokens(&m.vis)));
                if let Some((_, items)) = &m.content {
                    inventory(items, &format!("{scope}::{}", m.ident), api);
                }
            }
            syn::Item::Struct(s) if matches!(s.vis, syn::Visibility::Public(_)) => {
                api.insert(format!(
                    "struct {public_scope}::{} {}",
                    s.ident,
                    tokens(&s.generics)
                ));
                contract_attributes(&s.attrs, &format!("{public_scope}::{}", s.ident), api);
                for f in &s.fields {
                    if matches!(f.vis, syn::Visibility::Public(_))
                        && !f
                            .ident
                            .as_ref()
                            .is_some_and(|id| id.to_string().starts_with("__"))
                    {
                        contract_attributes(
                            &f.attrs,
                            &format!("{public_scope}::{}::{}", s.ident, tokens(&f.ident)),
                            api,
                        );
                        api.insert(format!(
                            "field {public_scope}::{}::{} {}",
                            s.ident,
                            tokens(&f.ident),
                            tokens(&f.ty)
                        ));
                    }
                }
            }
            syn::Item::Enum(e) if matches!(e.vis, syn::Visibility::Public(_)) => {
                let name = if scope.contains("::__buffa::view::oneof") {
                    format!("{}View", e.ident)
                } else {
                    e.ident.to_string()
                };
                api.insert(format!(
                    "enum {public_scope}::{name} {}",
                    tokens(&e.generics)
                ));
                contract_attributes(&e.attrs, &format!("{public_scope}::{name}"), api);
                for v in &e.variants {
                    let mut v = v.clone();
                    v.attrs.clear();
                    api.insert(format!("variant {public_scope}::{name}::{}", tokens(&v)));
                }
            }
            syn::Item::Impl(i) if i.trait_.is_some() => {
                let (_, path, _) = i.trait_.as_ref().expect("trait implementation");
                let owner = format!(
                    "impl {public_scope}::{} {} for {} {}",
                    tokens(&i.generics),
                    tokens(path),
                    tokens(&i.self_ty),
                    tokens(&i.generics.where_clause)
                );
                api.insert(owner.clone());
                for item in &i.items {
                    if let syn::ImplItem::Type(associated) = item {
                        let mut associated = associated.clone();
                        associated.attrs.clear();
                        api.insert(format!("associated {owner} {}", tokens(&associated)));
                    }
                }
            }
            syn::Item::Impl(i) if i.trait_.is_none() => {
                for item in &i.items {
                    match item {
                        syn::ImplItem::Fn(f) if matches!(f.vis, syn::Visibility::Public(_)) => {
                            api.insert(format!(
                                "method {public_scope}::{} {}",
                                tokens(&i.self_ty),
                                tokens(&f.sig)
                            ));
                        }
                        syn::ImplItem::Const(c) if matches!(c.vis, syn::Visibility::Public(_)) => {
                            api.insert(format!(
                                "const {public_scope}::{}::{} {} = {}",
                                tokens(&i.self_ty),
                                c.ident,
                                tokens(&c.ty),
                                tokens(&c.expr)
                            ));
                        }
                        _ => {}
                    }
                }
            }
            syn::Item::Use(u) if matches!(u.vis, syn::Visibility::Public(_)) => {
                api.insert(format!("use {public_scope} {}", tokens(&u.tree)));
            }
            syn::Item::Const(c) if matches!(c.vis, syn::Visibility::Public(_)) => {
                api.insert(format!(
                    "const {public_scope}::{} {} = {}",
                    c.ident,
                    tokens(&c.ty),
                    tokens(&c.expr)
                ));
            }
            syn::Item::Type(t) if matches!(t.vis, syn::Visibility::Public(_)) => {
                api.insert(format!(
                    "alias {public_scope}::{} {} = {}",
                    t.ident,
                    tokens(&t.generics),
                    tokens(&t.ty)
                ));
            }
            _ => {}
        }
    }
}

fn contract_attributes(attrs: &[syn::Attribute], owner: &str, api: &mut BTreeSet<String>) {
    for attr in attrs {
        let text = attr.to_token_stream().to_string();
        if attr.path().is_ident("derive")
            || attr.path().is_ident("repr")
            || attr.path().is_ident("serde")
            || (attr.path().is_ident("cfg_attr")
                && (text.contains("derive") || text.contains("serde")))
        {
            api.insert(format!("attribute {owner} {text}"));
        }
    }
}

/// Map buffa's implementation modules onto their published re-export paths.
/// Hidden storage fields and the __buffa module itself are not API commitments.
fn canonical(mut segments: Vec<String>, type_terminal: bool) -> String {
    if let Some(at) = segments.iter().position(|s| s == "__buffa") {
        let view = segments.get(at + 1).is_some_and(|s| s == "view");
        let oneof_at = at + if view { 2 } else { 1 };
        let oneof = segments.get(oneof_at).is_some_and(|s| s == "oneof");
        let end = at + 1 + usize::from(view) + usize::from(oneof);
        segments.drain(at..end);
        if view
            && oneof
            && type_terminal
            && let Some(last) = segments.last_mut()
        {
            last.push_str("View");
        }
    }
    segments.join("::")
}

fn normalize(input: &str, scope: &str) -> String {
    // ToTokens gives a whitespace-delimited spelling, independent of source
    // formatting. Rewrite only explicit relative paths and buffa's namespace;
    // leave Self, generic parameters and dependency-qualified types unchanged.
    let words: Vec<_> = input.split_whitespace().collect();
    let mut output = Vec::new();
    let mut i = 0;
    while i < words.len() {
        if matches!(words[i], "super" | "self" | "__buffa") && words.get(i + 1) == Some(&"::") {
            let mut resolved: Vec<String> = scope.split("::").map(str::to_owned).collect();
            loop {
                let word = words[i];
                match word {
                    "super" => {
                        resolved.pop();
                    }
                    "self" => {}
                    other => resolved.push(other.to_owned()),
                }
                i += 1;
                if words.get(i) != Some(&"::")
                    || words
                        .get(i + 1)
                        .is_none_or(|w| !w.chars().all(|c| c.is_alphanumeric() || c == '_'))
                {
                    break;
                }
                i += 1;
            }
            output.push(canonical(resolved, true));
        } else {
            output.push(words[i].to_owned());
            i += 1;
        }
    }
    output.join(" ")
}

pub fn check_api(expected: &str, actual: &BTreeSet<String>) -> io::Result<()> {
    let missing: Vec<_> = expected
        .lines()
        .filter(|l| !l.is_empty() && !l.starts_with('#') && !actual.contains(*l))
        .collect();
    if missing.is_empty() {
        return Ok(());
    }
    Err(io::Error::other(format!(
        "generated protobuf API changed: {} frozen entries missing or changed. Preserve names in build_support/names.rs and retained declarations in the proto emitter; a generator upgrade is a public API change. First entries:\n{}",
        missing.len(),
        missing.into_iter().take(20).collect::<Vec<_>>().join("\n")
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn associated_type_mappings_are_part_of_the_frozen_api() {
        let collect = |source: &str| {
            let file = syn::parse_file(source).unwrap();
            let mut api = BTreeSet::new();
            inventory(&file.items, "whatsapp", &mut api);
            api
        };
        let baseline =
            collect("impl HasMessageView for Message { type View<'a> = MessageView<'a>; }");
        let expected = baseline.iter().cloned().collect::<Vec<_>>().join("\n");
        for changed in [
            "impl HasMessageView for Message { type View<'a> = OtherView<'a>; }",
            "impl HasMessageView for Message { type View<'a> = MessageView<'static>; }",
            "impl HasMessageView for Message {}",
        ] {
            check_api(&expected, &collect(changed)).expect_err("associated mappings are API");
        }
        check_api(&expected, &collect(
            "impl HasMessageView for Message { type View<'a> = MessageView<'a>; type Added = (); }",
        )).expect("new associated declarations remain additive");
    }

    #[test]
    fn module_visibility_is_part_of_the_frozen_api() {
        let collect = |source: &str| {
            let file = syn::parse_file(source).unwrap();
            let mut api = BTreeSet::new();
            inventory(&file.items, "whatsapp", &mut api);
            api
        };
        let baseline = collect("pub mod outer { pub mod inner { pub struct Message; } }");
        let expected = baseline.iter().cloned().collect::<Vec<_>>().join("\n");
        check_api(&expected, &baseline).unwrap();
        for visibility in ["", "pub(crate)", "pub(super)"] {
            for source in [
                format!("{visibility} mod outer {{ pub mod inner {{ pub struct Message; }} }}"),
                format!("pub mod outer {{ {visibility} mod inner {{ pub struct Message; }} }}"),
            ] {
                check_api(&expected, &collect(&source))
                    .expect_err("public children do not compensate for an inaccessible module");
            }
        }
        check_api(
            &expected,
            &collect("pub mod outer { pub mod inner { pub struct Message; } pub mod added {} }"),
        )
        .expect("new public modules remain additive");

        let views =
            syn::parse_file("pub mod outer { pub mod inner { pub struct Message; } }").unwrap();
        let mut combined = baseline.clone();
        inventory(&views.items, "whatsapp::__buffa::view", &mut combined);
        let expected = combined.iter().cloned().collect::<Vec<_>>().join("\n");
        let mut candidate = collect("mod outer { pub mod inner { pub struct Message; } }");
        inventory(&views.items, "whatsapp::__buffa::view", &mut candidate);
        check_api(&expected, &candidate)
            .expect_err("a visible view module cannot mask a private owned module");
    }
}

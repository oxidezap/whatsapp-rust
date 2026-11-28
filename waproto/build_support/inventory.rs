use std::collections::BTreeSet;

use quote::ToTokens;

#[allow(dead_code)]
pub fn inventory(items: &[syn::Item], scope: &str, api: &mut BTreeSet<String>) {
    inventory_inner(items, scope, api, false);
}

#[allow(dead_code)]
pub fn inventory_compact(items: &[syn::Item], scope: &str, api: &mut BTreeSet<String>) {
    inventory_inner(items, scope, api, true);
}

#[allow(dead_code)]
pub fn compact_snapshot(api: &BTreeSet<String>) -> BTreeSet<String> {
    api.iter()
        .filter(|entry| {
            if !entry.starts_with("impl ") {
                return true;
            }
            // Every associated item carries the complete impl identity.
            let prefix = format!("associated {entry} ");
            !api.range(prefix.clone()..)
                .next()
                .is_some_and(|associated| associated.starts_with(&prefix))
        })
        .cloned()
        .collect()
}

fn inventory_inner(items: &[syn::Item], scope: &str, api: &mut BTreeSet<String>, compact: bool) {
    let tokens = |value: &dyn ToTokens| normalize(&value.to_token_stream().to_string(), scope);
    let public_scope = canonical(scope.split("::").map(str::to_owned).collect(), false);
    let availability = |attrs: &[syn::Attribute]| {
        let attrs: Vec<_> = attrs
            .iter()
            .filter(|attr| attr.path().is_ident("cfg") || attr.path().is_ident("cfg_attr"))
            .collect();
        if attrs.is_empty() {
            String::new()
        } else {
            let attrs = quote::quote!(#(#attrs)*);
            format!(" availability={}", tokens(&attrs))
        }
    };
    for item in items {
        match item {
            syn::Item::Mod(m) if !m.ident.to_string().starts_with("__") => {
                // Keep owned/view source scopes distinct: their canonical
                // public paths overlap, but either module can lose visibility.
                api.insert(format!(
                    "module {scope}::{} {}{}",
                    m.ident,
                    tokens(&m.vis),
                    availability(&m.attrs)
                ));
                if let Some((_, items)) = &m.content {
                    inventory_inner(items, &format!("{scope}::{}", m.ident), api, compact);
                }
            }
            syn::Item::Struct(s) if matches!(s.vis, syn::Visibility::Public(_)) => {
                api.insert(format!(
                    "struct {public_scope}::{} {}{}",
                    s.ident,
                    tokens(&s.generics),
                    availability(&s.attrs)
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
                            "field {public_scope}::{}::{} {}{}",
                            s.ident,
                            tokens(&f.ident),
                            tokens(&f.ty),
                            availability(&f.attrs)
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
                    "enum {public_scope}::{name} {}{}",
                    tokens(&e.generics),
                    availability(&e.attrs)
                ));
                contract_attributes(&e.attrs, &format!("{public_scope}::{name}"), api);
                for v in &e.variants {
                    let mut v = v.clone();
                    // Keep attributes in the existing variant entry so adding
                    // a rename cannot pass as an additive API declaration.
                    v.attrs.retain(is_contract_attribute);
                    api.insert(format!("variant {public_scope}::{name}::{}", tokens(&v)));
                }
            }
            syn::Item::Impl(i) if i.trait_.is_some() => {
                let (path, _) = i.trait_.as_ref().expect("trait implementation");
                let owner = format!(
                    "impl {public_scope}::{} {} for {} {}{}",
                    tokens(&i.generics),
                    tokens(path),
                    tokens(&i.self_ty),
                    tokens(&i.generics.where_clause),
                    availability(&i.attrs)
                );
                api.insert(owner.clone());
                for item in &i.items {
                    match item {
                        syn::ImplItem::Type(associated) => {
                            let conditional = availability(&associated.attrs);
                            let mut associated = associated.clone();
                            associated.attrs.clear();
                            api.insert(format!(
                                "associated {owner} {}{conditional}",
                                tokens(&associated)
                            ));
                        }
                        syn::ImplItem::Const(associated) => {
                            let conditional = availability(&associated.attrs);
                            let mut associated = associated.clone();
                            associated.attrs.clear();
                            api.insert(format!(
                                "associated {owner} {}{conditional}",
                                tokens(&associated)
                            ));
                        }
                        _ => {}
                    }
                }
            }
            syn::Item::Impl(i) if i.trait_.is_none() => {
                let generics = tokens(&i.generics);
                let where_clause = tokens(&i.generics.where_clause);
                let bounds = if compact && (!generics.is_empty() || !where_clause.is_empty()) {
                    format!(" impl-generics={generics} impl-where={where_clause}")
                } else {
                    String::new()
                };
                let impl_availability = availability(&i.attrs);
                for item in &i.items {
                    match item {
                        syn::ImplItem::Fn(f) if matches!(f.vis, syn::Visibility::Public(_)) => {
                            if !compact {
                                api.insert(format!(
                                    "inherent-method-bound {public_scope}::{}::{} {} {}{}{}",
                                    tokens(&i.self_ty),
                                    f.sig.ident,
                                    generics,
                                    where_clause,
                                    impl_availability,
                                    availability(&f.attrs)
                                ));
                            }
                            api.insert(format!(
                                "method {public_scope}::{} {}{}{}{}",
                                tokens(&i.self_ty),
                                tokens(&f.sig),
                                bounds,
                                impl_availability,
                                availability(&f.attrs)
                            ));
                        }
                        syn::ImplItem::Const(c) if matches!(c.vis, syn::Visibility::Public(_)) => {
                            if !compact {
                                api.insert(format!(
                                    "inherent-const-bound {public_scope}::{}::{} {} {}{}{}",
                                    tokens(&i.self_ty),
                                    c.ident,
                                    generics,
                                    where_clause,
                                    impl_availability,
                                    availability(&c.attrs)
                                ));
                            }
                            let kind = if compact { "inherent-const" } else { "const" };
                            api.insert(format!(
                                "{kind} {public_scope}::{}::{} {} = {}{}{}{}",
                                tokens(&i.self_ty),
                                c.ident,
                                tokens(&c.ty),
                                tokens(&c.expr),
                                bounds,
                                impl_availability,
                                availability(&c.attrs)
                            ));
                        }
                        _ => {}
                    }
                }
            }
            syn::Item::Use(u) if matches!(u.vis, syn::Visibility::Public(_)) => {
                api.insert(format!(
                    "use {public_scope} {}{}",
                    tokens(&u.tree),
                    availability(&u.attrs)
                ));
            }
            syn::Item::Const(c) if matches!(c.vis, syn::Visibility::Public(_)) => {
                api.insert(format!(
                    "const {public_scope}::{} {} = {}{}",
                    c.ident,
                    tokens(&c.ty),
                    tokens(&c.expr),
                    availability(&c.attrs)
                ));
            }
            syn::Item::Type(t) if matches!(t.vis, syn::Visibility::Public(_)) => {
                api.insert(format!(
                    "alias {public_scope}::{} {} = {}{}",
                    t.ident,
                    tokens(&t.generics),
                    tokens(&t.ty),
                    availability(&t.attrs)
                ));
            }
            _ => {}
        }
    }
}

fn is_contract_attribute(attr: &syn::Attribute) -> bool {
    attr.path().is_ident("derive")
        || attr.path().is_ident("non_exhaustive")
        || attr.path().is_ident("repr")
        || attr.path().is_ident("serde")
        || attr.path().is_ident("cfg")
        || attr.path().is_ident("cfg_attr")
}

fn contract_attributes(attrs: &[syn::Attribute], owner: &str, api: &mut BTreeSet<String>) {
    for attr in attrs {
        if is_contract_attribute(attr) {
            let text = attr.to_token_stream().to_string();
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
    let mut output = String::with_capacity(input.len());
    let mut i = 0;
    while i < words.len() {
        if i != 0 {
            output.push(' ');
        }
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
            output.push_str(&canonical(resolved, true));
        } else {
            output.push_str(words[i]);
            i += 1;
        }
    }
    output
}

//! Add a cold wire-occurrence journal only to generated enum/oneof owners.

use quote::{format_ident, quote};
use std::collections::{BTreeMap, BTreeSet};
use syn::visit_mut::{self, VisitMut};

#[derive(Default)]
struct Probe {
    fields: BTreeSet<String>,
    enumeration: bool,
    choice: bool,
    enum_number: Option<u32>,
}
impl VisitMut for Probe {
    fn visit_expr_field_mut(&mut self, expr: &mut syn::ExprField) {
        if let syn::Expr::Path(base) = &*expr.base
            && base.path.is_ident("self")
            && let syn::Member::Named(name) = &expr.member
        {
            self.fields.insert(name.to_string());
        }
        visit_mut::visit_expr_field_mut(self, expr);
    }
    fn visit_expr_method_call_mut(&mut self, expr: &mut syn::ExprMethodCall) {
        self.enumeration |= expr.method == "to_i32";
        visit_mut::visit_expr_method_call_mut(self, expr);
    }
    fn visit_expr_match_mut(&mut self, expr: &mut syn::ExprMatch) {
        self.choice = true;
        visit_mut::visit_expr_match_mut(self, expr);
    }
    fn visit_expr_call_mut(&mut self, expr: &mut syn::ExprCall) {
        if let syn::Expr::Path(path) = &*expr.func
            && path
                .path
                .segments
                .last()
                .is_some_and(|part| part.ident == "put_int32_field")
            && let Some(syn::Expr::Lit(literal)) = expr.args.first()
            && let syn::Lit::Int(number) = &literal.lit
        {
            self.enum_number = Some(number.base10_parse().expect("generated field number"));
        }
        visit_mut::visit_expr_call_mut(self, expr);
    }
}

fn fields(statement: &syn::Stmt) -> Probe {
    let mut result = Probe::default();
    result.visit_stmt_mut(&mut statement.clone());
    result
}

fn type_name(item: &syn::ItemImpl) -> Option<String> {
    let syn::Type::Path(path) = &*item.self_ty else {
        return None;
    };
    path.path.segments.last().map(|p| p.ident.to_string())
}
fn trait_name(item: &syn::ItemImpl) -> Option<String> {
    item.trait_
        .as_ref()?
        .1
        .segments
        .last()
        .map(|p| p.ident.to_string())
}
fn method(item: &syn::ItemImpl, name: &str) -> Option<syn::ImplItemFn> {
    item.items.iter().find_map(|m| match m {
        syn::ImplItem::Fn(f) if f.sig.ident == name => Some(f.clone()),
        _ => None,
    })
}
fn argument(method: &syn::ImplItemFn, index: usize) -> syn::Ident {
    let syn::FnArg::Typed(arg) = &method.sig.inputs[index] else {
        panic!("generated typed argument")
    };
    let syn::Pat::Ident(name) = &*arg.pat else {
        panic!("generated identifier argument")
    };
    name.ident.clone()
}

struct PushDecoded;
impl VisitMut for PushDecoded {
    fn visit_expr_mut(&mut self, expr: &mut syn::Expr) {
        visit_mut::visit_expr_mut(self, expr);
        if let syn::Expr::MethodCall(call) = expr
            && call.method == "push"
            && let syn::Expr::Field(field) = &*call.receiver
            && matches!(&field.member, syn::Member::Named(name) if name == "__buffa_unknown_fields")
        {
            let receiver = &call.receiver;
            let args = &call.args;
            *expr = syn::parse_quote!(#receiver.push_decoded(#args, ctx)?);
        }
    }
}

struct PushViewDecoded;
impl VisitMut for PushViewDecoded {
    fn visit_expr_method_call_mut(&mut self, call: &mut syn::ExprMethodCall) {
        visit_mut::visit_expr_method_call_mut(self, call);
        if let syn::Expr::Field(field) = &*call.receiver
            && matches!(&field.member, syn::Member::Named(name) if name == "__buffa_unknown_fields")
        {
            if call.method == "push_record" {
                call.method = format_ident!("push_decoded_record");
            } else if call.method == "push_varint" {
                call.method = format_ident!("push_decoded_varint");
            }
        }
    }
}

pub fn apply(file: &mut syn::File, view: bool) {
    transform(&mut file.items, view, 0);
}

fn transform(items: &mut Vec<syn::Item>, view: bool, depth: usize) {
    for item in items.iter_mut() {
        if let syn::Item::Mod(module) = item
            && let Some((_, children)) = &mut module.content
        {
            transform(children, view, depth + 1);
        }
    }
    let mut selected: BTreeMap<String, BTreeMap<String, u32>> = BTreeMap::new();
    let mut enum_fields = BTreeMap::new();
    for item in items.iter() {
        let syn::Item::Impl(item) = item else {
            continue;
        };
        if trait_name(item).as_deref() != Some(if view { "ViewEncode" } else { "Message" }) {
            continue;
        }
        let Some(write) = method(item, "write_to") else {
            continue;
        };
        let mut groups = BTreeMap::new();
        let mut numbers = BTreeMap::new();
        for statement in &write.block.stmts {
            // Repeated fields have a different mutation contract; singular
            // values and oneofs are emitted as individual conditional blocks.
            if matches!(statement, syn::Stmt::Expr(syn::Expr::ForLoop(_), _)) {
                continue;
            }
            let probe = fields(statement);
            if (probe.enumeration || probe.choice) && probe.fields.len() == 1 {
                let name = probe
                    .fields
                    .iter()
                    .next()
                    .expect("one projected field")
                    .clone();
                if !probe.choice
                    && let Some(number) = probe.enum_number
                {
                    numbers.insert(name.clone(), number);
                }
                groups.insert(name, groups.len() as u32 + 1);
            }
        }
        if !groups.is_empty() {
            let name = type_name(item).expect("generated type");
            if numbers.len() == groups.len() {
                enum_fields.insert(name.clone(), numbers);
            }
            selected.insert(name, groups);
        }
    }
    if selected.is_empty() {
        return;
    }
    let prefix = "super::".repeat(depth + if view { 2 } else { 0 });
    let runtime: syn::Path =
        syn::parse_str(&format!("{prefix}__wire_order")).expect("runtime path");
    let mut helpers = Vec::new();
    let mut enum_values = BTreeMap::new();
    for item in items.iter() {
        let syn::Item::Struct(owner) = item else {
            continue;
        };
        let Some(groups) = selected.get(&owner.ident.to_string()) else {
            continue;
        };
        let name = &owner.ident;
        if let Some(numbers) = enum_fields.get(&name.to_string()) {
            let values: Vec<_> = owner.fields.iter().filter_map(|field| {
                let field_name = field.ident.as_ref()?;
                let number = numbers.get(&field_name.to_string())?;
                let optional = matches!(&field.ty, syn::Type::Path(path) if path.path.segments.last().is_some_and(|part| part.ident == "Option"));
                let value = if optional {
                    quote!(self.#field_name.as_ref().map(|value| value.to_i32()))
                } else {
                    quote!(Some(self.#field_name.to_i32()))
                };
                Some(quote!((#number, #value)))
            }).collect();
            enum_values.insert(name.to_string(), values);
        }
        let (impl_generics, ty_generics, where_clause) = owner.generics.split_for_impl();
        for field in &owner.fields {
            let Some(field_name) = &field.ident else {
                continue;
            };
            let Some(group) = groups.get(&field_name.to_string()) else {
                continue;
            };
            let setter = format_ident!("with_{}", field_name);
            let exists = items.iter().any(|item| matches!(item, syn::Item::Impl(item)
                if item.trait_.is_none() && type_name(item).as_deref() == Some(name.to_string().as_str())
                    && method(item, &setter.to_string()).is_some()));
            if exists {
                continue;
            }
            let mut value_ty = &field.ty;
            let mut optional = false;
            if let syn::Type::Path(path) = &field.ty
                && let Some(segment) = path.path.segments.last()
                && segment.ident == "Option"
                && let syn::PathArguments::AngleBracketed(args) = &segment.arguments
                && let Some(syn::GenericArgument::Type(ty)) = args.args.first()
            {
                value_ty = ty;
                optional = true;
            }
            let assign = if optional {
                quote!(Some(value.into()))
            } else {
                quote!(value.into())
            };
            helpers.push(syn::parse_quote! {
                impl #impl_generics #name #ty_generics #where_clause {
                    /// Explicitly replace this known value, including when it
                    /// equals the received projection of a future-bearing field.
                    #[must_use = "with_* setters return self; assign or chain the result"]
                    pub fn #setter(mut self, value: impl Into<#value_ty>) -> Self {
                        self.#field_name = #assign;
                        self.__buffa_unknown_fields.force(#group);
                        self
                    }
                }
            });
        }
    }
    for item in items.iter_mut() {
        if let syn::Item::Struct(item) = item
            && selected.contains_key(&item.ident.to_string())
        {
            for field in &mut item.fields {
                if field
                    .ident
                    .as_ref()
                    .is_some_and(|id| id == "__buffa_unknown_fields")
                {
                    field.ty = if view {
                        syn::parse_quote!(#runtime::ViewStorage<'a>)
                    } else {
                        syn::parse_quote!(#runtime::Storage)
                    };
                }
            }
        }
        let syn::Item::Impl(item) = item else {
            continue;
        };
        let Some(name) = type_name(item) else {
            continue;
        };
        let Some(groups) = selected.get(&name) else {
            continue;
        };
        let ty = item.self_ty.clone();
        let (impl_generics, _, where_clause) = item.generics.split_for_impl();
        let trait_id = trait_name(item);
        if trait_id.as_deref() == Some(if view { "ViewEncode" } else { "Message" }) {
            let mut compute = method(item, "compute_size").expect("generated size method");
            let mut write = method(item, "write_to").expect("generated write method");
            compute.sig.ident = format_ident!("__wire_compute");
            write.sig.ident = format_ident!("__wire_write");
            for helper in [&mut compute, &mut write] {
                // Only selected fields participate in mutation detection. The
                // remaining fields keep their original generated codec and
                // traversal-cache entries, avoiding a second whole-message
                // encoder and copies of unrelated payloads during decoding.
                helper.block.stmts.retain(|statement| {
                    let probe = fields(statement);
                    probe.fields.is_empty()
                        || probe.fields.iter().any(|field| groups.contains_key(field))
                });
                helper.block.stmts.insert(
                    0,
                    syn::parse_quote!(
                        #[allow(unused_imports)]
                        use ::buffa::{Message as _, MessageView as _, ViewEncode as _};
                    ),
                );
            }
            let compute_cache = argument(&compute, 1);
            let write_cache = argument(&write, 1);
            let write_buf = argument(&write, 2);
            for member in &mut item.items {
                if let syn::ImplItem::Fn(f) = member {
                    let is_compute = f.sig.ident == "compute_size";
                    if !is_compute && f.sig.ident != "write_to" {
                        continue;
                    }
                    f.attrs.retain(|attr| !attr.path().is_ident("inline"));
                    f.attrs.push(syn::parse_quote!(#[inline(never)]));
                    let mut unknown = false;
                    for statement in &mut f.block.stmts {
                        let probe = fields(statement);
                        if probe.fields.iter().any(|field| groups.contains_key(field)) {
                            if let syn::Stmt::Expr(syn::Expr::If(conditional), _) = statement
                                && conditional.else_branch.is_none()
                            {
                                let condition = &conditional.cond;
                                *conditional.cond = syn::parse_quote!(!__wire_active && #condition);
                            } else {
                                let original = statement.clone();
                                *statement = syn::parse_quote!(if !__wire_active { #original });
                            }
                        } else if probe.fields.contains("__buffa_unknown_fields") {
                            assert!(!unknown, "one generated unknown-field statement");
                            unknown = true;
                            let original = statement.clone();
                            *statement = if is_compute {
                                syn::parse_quote!(if __wire_active {
                                    let bytes = self.__buffa_unknown_fields.compose(&self.__wire_known(), Self::__wire_group);
                                    size += #runtime::cache_output(&bytes, #compute_cache) as u64;
                                } else { #original })
                            } else {
                                syn::parse_quote!(if __wire_active {
                                    #runtime::write_cached(#write_cache, #write_buf);
                                } else { #original })
                            };
                        }
                    }
                    assert!(unknown, "generated codec retains unknown fields");
                    f.block.stmts.insert(0, syn::parse_quote!(let __wire_active = self.__buffa_unknown_fields.active();));
                }
            }
            let (codecs, projection) = if let Some(values) = enum_values.get(&name) {
                (
                    quote!(),
                    quote!({
                        #[allow(unused_imports)]
                        use ::buffa::Enumeration as _;
                        #runtime::enum_snapshot(&[#(#values),*], ctx)
                    }),
                )
            } else {
                (
                    quote!(#compute #write),
                    quote!({
                        let mut cache = ::buffa::SizeCache::new();
                        let size = self.__wire_compute(&mut cache);
                        if let Some(ctx) = ctx {
                            ctx.register_element_memory(size as usize)?;
                        }
                        let mut bytes = ::buffa::alloc::vec::Vec::with_capacity(size as usize);
                        self.__wire_write(&mut cache, &mut bytes);
                        Ok(bytes)
                    }),
                )
            };
            helpers.push(syn::parse_quote! {
                impl #impl_generics #ty #where_clause {
                    #codecs
                    #[cold]
                    pub(crate) fn __wire_known(&self) -> ::buffa::alloc::vec::Vec<u8> {
                        self.__wire_snapshot(None).expect("unbudgeted wire projection")
                    }
                    #[cold]
                    fn __wire_known_for_decode(&self, ctx: ::buffa::DecodeContext<'_>) -> ::core::result::Result<::buffa::alloc::vec::Vec<u8>, ::buffa::DecodeError> {
                        self.__wire_snapshot(Some(ctx))
                    }
                    #[cold]
                    #[inline(never)]
                    fn __wire_snapshot(&self, ctx: ::core::option::Option<::buffa::DecodeContext<'_>>) -> ::core::result::Result<::buffa::alloc::vec::Vec<u8>, ::buffa::DecodeError> #projection
                }
            });
        }
        if trait_id.as_deref() == Some(if view { "MessageView" } else { "Message" }) {
            let merge_name = if view {
                "merge_view_field"
            } else {
                "merge_field"
            };
            let mut merge = method(item, merge_name).expect("generated merge method");
            let mut cases = Vec::new();
            for statement in &merge.block.stmts {
                if let syn::Stmt::Expr(syn::Expr::Match(m), _) = statement {
                    for arm in &m.arms {
                        let mut probe = Probe::default();
                        // View code aliases self to `view`; examine the emitted
                        // field accesses structurally with that alias normalized.
                        let mut body = (*arm.body).clone();
                        struct Alias;
                        impl VisitMut for Alias {
                            fn visit_expr_path_mut(&mut self, path: &mut syn::ExprPath) {
                                if path.path.is_ident("view") {
                                    path.path = syn::parse_quote!(self);
                                }
                            }
                        }
                        Alias.visit_expr_mut(&mut body);
                        probe.visit_expr_mut(&mut body);
                        if let Some(group) = probe.fields.iter().find_map(|field| groups.get(field))
                        {
                            let pattern = &arm.pat;
                            cases.push(quote!(#pattern => #group,));
                        }
                    }
                }
            }
            if !view {
                PushDecoded.visit_impl_item_fn_mut(&mut merge);
            } else {
                PushViewDecoded.visit_impl_item_fn_mut(&mut merge);
            }
            merge.sig.ident = format_ident!("__wire_merge");
            // Both the ordinary decoder and the retained-occurrence adapter
            // call this codec. Inlining small codecs into both routes repeats
            // their nested decode trees, despite the outer dispatch being cold.
            merge.attrs.retain(|attr| !attr.path().is_ident("inline"));
            merge.attrs.push(syn::parse_quote!(#[inline(never)]));
            merge.block.stmts.insert(
                0,
                syn::parse_quote!(
                    #[allow(unused_imports)]
                    use ::buffa::{Message as _, MessageView as _};
                ),
            );
            for member in &mut item.items {
                let syn::ImplItem::Fn(f) = member else {
                    continue;
                };
                if f.sig.ident == merge_name {
                    f.attrs.retain(|attr| !attr.path().is_ident("inline"));
                    f.attrs.push(syn::parse_quote!(#[inline(never)]));
                    f.block = if view {
                        syn::parse_quote!({
                            if !self.__buffa_unknown_fields.active() {
                                let rest = self.__wire_merge(tag, cur, before_tag, ctx)?;
                                if !self.__buffa_unknown_fields.is_empty() {
                                    #runtime::begin_view(self, ctx)?;
                                }
                                return Ok(rest);
                            }
                            #runtime::merge_view(self, tag, cur, before_tag, ctx)
                        })
                    } else {
                        syn::parse_quote!({
                            if !self.__buffa_unknown_fields.active() {
                                self.__wire_merge(tag, buf, ctx)?;
                                if !self.__buffa_unknown_fields.is_empty() {
                                    #runtime::begin_owned(self, ctx)?;
                                }
                                return Ok(());
                            }
                            if Self::__wire_group(tag.field_number()) != 0 {
                                #runtime::merge_owned_known(self, tag, buf, ctx)
                            } else {
                                let previous = self.__buffa_unknown_fields.len();
                                self.__wire_merge(tag, buf, ctx)?;
                                #runtime::finish_owned_unknown(self, previous, ctx)
                            }
                        })
                    };
                } else if view && f.sig.ident == "to_owned_from_source" {
                    let body = f.block.clone();
                    f.block = syn::parse_quote!({
                        let mut owned = (|| -> ::core::result::Result<Self::Owned, ::buffa::DecodeError> #body)()?;
                        if self.__buffa_unknown_fields.active() {
                            let view_known = self.__wire_known();
                            let owned_known = owned.__wire_known();
                            owned.__buffa_unknown_fields.rebase(&view_known, &owned_known, Self::__wire_group);
                        }
                        Ok(owned)
                    });
                }
            }
            helpers.push(syn::parse_quote! {
                impl #impl_generics #ty #where_clause {
                    #merge
                    #[inline]
                    fn __wire_group(tag: u32) -> u32 {
                        match tag { #(#cases)* _ => 0 }
                    }
                }
            });
            helpers.push(if view {
                syn::parse_quote! {
                    impl #impl_generics #runtime::ViewCodec<'a> for #ty #where_clause {
                        fn storage(&mut self) -> &mut #runtime::ViewStorage<'a> { &mut self.__buffa_unknown_fields }
                        fn groups(&self) -> fn(u32) -> u32 { Self::__wire_group }
                        fn known(&self, ctx: ::buffa::DecodeContext<'_>) -> ::core::result::Result<::buffa::alloc::vec::Vec<u8>, ::buffa::DecodeError> { self.__wire_known_for_decode(ctx) }
                        fn merge(&mut self, tag: ::buffa::encoding::Tag, cur: &'a [u8], before: &'a [u8], ctx: ::buffa::DecodeContext<'_>) -> ::core::result::Result<&'a [u8], ::buffa::DecodeError> { self.__wire_merge(tag, cur, before, ctx) }
                    }
                }
            } else {
                syn::parse_quote! {
                    impl #impl_generics #runtime::OwnedCodec for #ty #where_clause {
                        fn storage(&mut self) -> &mut #runtime::Storage { &mut self.__buffa_unknown_fields }
                        fn groups(&self) -> fn(u32) -> u32 { Self::__wire_group }
                        fn known(&self, ctx: ::buffa::DecodeContext<'_>) -> ::core::result::Result<::buffa::alloc::vec::Vec<u8>, ::buffa::DecodeError> { self.__wire_known_for_decode(ctx) }
                        fn merge_slice(&mut self, tag: ::buffa::encoding::Tag, buf: &mut &[u8], ctx: ::buffa::DecodeContext<'_>) -> ::core::result::Result<(), ::buffa::DecodeError> { self.__wire_merge(tag, buf, ctx) }
                    }
                }
            });
        }
        if !view && item.trait_.is_none() {
            for member in &mut item.items {
                let syn::ImplItem::Fn(method) = member else {
                    continue;
                };
                if let Some(field) = method.sig.ident.to_string().strip_prefix("with_")
                    && let Some(group) = groups.get(field)
                {
                    let last = method.block.stmts.pop().expect("setter returns self");
                    method
                        .block
                        .stmts
                        .push(syn::parse_quote!(self.__buffa_unknown_fields.force(#group);));
                    method.block.stmts.push(last);
                }
            }
        }
    }
    items.extend(helpers);
}

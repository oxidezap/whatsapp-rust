//! Add a cold wire-occurrence journal only to generated enum/oneof owners.

use quote::{format_ident, quote};
use std::collections::{BTreeMap, BTreeSet};
use syn::visit_mut::{self, VisitMut};

#[derive(Default)]
struct Probe {
    fields: BTreeSet<String>,
    enumeration: bool,
    choice: bool,
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
                groups.insert(name, groups.len() as u32 + 1);
            }
        }
        if !groups.is_empty() {
            selected.insert(type_name(item).expect("generated type"), groups);
        }
    }
    if selected.is_empty() {
        return;
    }
    let prefix = "super::".repeat(depth + if view { 2 } else { 0 });
    let runtime: syn::Path =
        syn::parse_str(&format!("{prefix}__wire_order")).expect("runtime path");
    let mut helpers = Vec::new();
    for item in items.iter() {
        let syn::Item::Struct(owner) = item else {
            continue;
        };
        let Some(groups) = selected.get(&owner.ident.to_string()) else {
            continue;
        };
        let name = &owner.ident;
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
                helper
                    .sig
                    .inputs
                    .push(syn::parse_quote!(__include_unknown: bool));
                for statement in &mut helper.block.stmts {
                    let probe = fields(statement);
                    if probe.fields.len() == 1 && probe.fields.contains("__buffa_unknown_fields") {
                        let original = statement.clone();
                        *statement = syn::parse_quote!(if __include_unknown { #original });
                    }
                }
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
                    if f.sig.ident == "compute_size" || f.sig.ident == "write_to" {
                        // Journal dispatch is shared even for small messages;
                        // inlining it into every parent repeats cold machinery.
                        f.attrs.retain(|attr| !attr.path().is_ident("inline"));
                        f.attrs.push(syn::parse_quote!(#[inline(never)]));
                    }
                    if f.sig.ident == "compute_size" {
                        f.block = syn::parse_quote!({
                            if self.__buffa_unknown_fields.active() {
                                let bytes = self.__buffa_unknown_fields.compose(&self.__wire_known(), Self::__wire_group);
                                return #runtime::cache_output(&bytes, #compute_cache);
                            }
                            self.__wire_compute(#compute_cache, true)
                        });
                    } else if f.sig.ident == "write_to" {
                        f.block = syn::parse_quote!({
                            if self.__buffa_unknown_fields.active() {
                                #runtime::write_cached(#write_cache, #write_buf);
                            } else {
                                self.__wire_write(#write_cache, #write_buf, true);
                            }
                        });
                    }
                }
            }
            helpers.push(syn::parse_quote! {
                impl #impl_generics #ty #where_clause {
                    #compute
                    #write
                    #[cold]
                    pub(crate) fn __wire_known(&self) -> ::buffa::alloc::vec::Vec<u8> {
                        let mut cache = ::buffa::SizeCache::new();
                        let size = self.__wire_compute(&mut cache, false);
                        let mut bytes = ::buffa::alloc::vec::Vec::with_capacity(size as usize);
                        self.__wire_write(&mut cache, &mut bytes, false);
                        bytes
                    }
                    #[cold]
                    fn __wire_known_for_decode(&self, ctx: ::buffa::DecodeContext<'_>) -> ::core::result::Result<::buffa::alloc::vec::Vec<u8>, ::buffa::DecodeError> {
                        let mut cache = ::buffa::SizeCache::new();
                        let size = self.__wire_compute(&mut cache, false);
                        ctx.register_element_memory(size as usize)?;
                        let mut bytes = ::buffa::alloc::vec::Vec::with_capacity(size as usize);
                        self.__wire_write(&mut cache, &mut bytes, false);
                        Ok(bytes)
                    }
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
                                    let known = self.__wire_known_for_decode(ctx)?;
                                    self.__buffa_unknown_fields.begin(
                                        &known,
                                        Self::__wire_group,
                                        ctx,
                                    )?;
                                }
                                return Ok(rest);
                            }
                            let group = Self::__wire_group(tag.field_number());
                            let previous = self.__buffa_unknown_fields.len();
                            if group != 0 {
                                let known = self.__wire_known_for_decode(ctx)?;
                                self.__buffa_unknown_fields.reconcile(
                                    &known,
                                    Self::__wire_group,
                                    ctx,
                                )?;
                            }
                            let rest = self.__wire_merge(tag, cur, before_tag, ctx)?;
                            let count = self.__buffa_unknown_fields.len();
                            if group != 0 || count != previous {
                                let known = self.__wire_known_for_decode(ctx)?;
                                if group == 0 {
                                    self.__buffa_unknown_fields.reconcile(
                                        &known,
                                        Self::__wire_group,
                                        ctx,
                                    )?;
                                }
                                self.__buffa_unknown_fields.finish(
                                    group,
                                    &before_tag[..before_tag.len() - rest.len()],
                                    &known,
                                    Self::__wire_group,
                                    previous,
                                    ctx,
                                )?;
                            }
                            Ok(rest)
                        })
                    } else {
                        syn::parse_quote!({
                            if !self.__buffa_unknown_fields.active() {
                                self.__wire_merge(tag, buf, ctx)?;
                                if !self.__buffa_unknown_fields.is_empty() {
                                    let known = self.__wire_known_for_decode(ctx)?;
                                    self.__buffa_unknown_fields.begin(&known, Self::__wire_group, ctx)?;
                                }
                                return Ok(());
                            }
                            let group = Self::__wire_group(tag.field_number());
                            let previous = self.__buffa_unknown_fields.len();
                            let raw = if group != 0 {
                                let known = self.__wire_known_for_decode(ctx)?;
                                self.__buffa_unknown_fields.reconcile(&known, Self::__wire_group, ctx)?;
                                let mut captured = #runtime::Capture::new(buf, tag, ctx)?;
                                ::buffa::encoding::skip_field_depth(tag, &mut captured, ctx.depth())?;
                                let raw = captured.finish()?;
                                let mut payload = raw.as_slice();
                                ::buffa::encoding::Tag::decode(&mut payload)?;
                                self.__wire_merge(tag, &mut payload, ctx)?;
                                Some(raw)
                            } else {
                                self.__wire_merge(tag, buf, ctx)?;
                                None
                            };
                            let count = self.__buffa_unknown_fields.len();
                            if group != 0 || count != previous {
                                let known = self.__wire_known_for_decode(ctx)?;
                                if group == 0 { self.__buffa_unknown_fields.reconcile(&known, Self::__wire_group, ctx)?; }
                                self.__buffa_unknown_fields.finish(group, raw, &known, Self::__wire_group, previous, ctx)?;
                            }
                            Ok(())
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

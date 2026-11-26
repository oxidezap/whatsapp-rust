//! Keep large recursive clones in one crate. Rust's derived Clone is
//! inline, so downstream callers otherwise carry separate copies of this large
//! tree even with LTO. Derive the replacement from the generated Rust fields,
//! not a second hand-maintained schema, and keep every other derive unchanged.

pub fn apply(file: &mut syn::File, targets: &[&str]) -> Result<(), String> {
    fn visit(
        items: &mut Vec<syn::Item>,
        scope: &str,
        targets: &[&str],
        found: &mut [bool],
    ) -> Result<(), String> {
        let mut implementations = Vec::new();
        for item in items.iter_mut() {
            if let syn::Item::Mod(module) = item {
                if let Some((_, children)) = &mut module.content {
                    let nested = format!("{scope}{}::", module.ident);
                    visit(children, &nested, targets, found)?;
                }
                continue;
            }
            let syn::Item::Struct(message) = item else {
                continue;
            };
            let qualified = format!("{scope}{}", message.ident);
            let Some(index) = targets.iter().position(|&name| name == qualified) else {
                continue;
            };
            let name = &message.ident;
            if found[index] || !message.generics.params.is_empty() {
                return Err(format!(
                    "expected one non-generic generated {qualified} struct"
                ));
            }
            found[index] = true;
            let syn::Fields::Named(fields) = &message.fields else {
                return Err(format!("expected named fields in generated {qualified}"));
            };
            let names = fields
                .named
                .iter()
                .map(|field| &field.ident)
                .collect::<Vec<_>>();
            let mut removed = 0;
            for attribute in &mut message.attrs {
                if !attribute.path().is_ident("derive") {
                    continue;
                }
                let paths = attribute
                    .parse_args_with(
                        syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
                    )
                    .map_err(|error| error.to_string())?;
                let paths = paths
                    .into_iter()
                    .filter(|path| {
                        if path.is_ident("Clone") {
                            removed += 1;
                            false
                        } else {
                            true
                        }
                    })
                    .collect::<Vec<_>>();
                attribute.meta = syn::parse_quote!(derive(#(#paths),*));
            }
            if removed != 1 {
                return Err(format!(
                    "expected exactly one derived Clone on generated {qualified}"
                ));
            }
            implementations.push(syn::parse_quote! {
                #[automatically_derived]
                impl ::core::clone::Clone for #name {
                    #[inline(never)]
                    #[allow(clippy::clone_on_copy)]
                    fn clone(&self) -> Self {
                        Self { #(#names: ::core::clone::Clone::clone(&self.#names)),* }
                    }
                }
            });
        }
        items.extend(implementations);
        Ok(())
    }
    let mut found = vec![false; targets.len()];
    visit(&mut file.items, "", targets, &mut found)?;
    for (target, found) in targets.iter().zip(found) {
        if !found {
            return Err(format!("generated {target} struct is missing"));
        }
    }
    Ok(())
}

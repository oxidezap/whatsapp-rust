//! Keep large recursive clones in one crate. Rust's derived Clone is
//! inline, so downstream callers otherwise carry separate copies of this large
//! tree even with LTO. Derive the replacement from the generated Rust fields,
//! not a second hand-maintained schema, and keep every other derive unchanged.

pub fn generate(source: &str, targets: &[&str]) -> Result<String, String> {
    let mut file = syn::parse_file(source).map_err(|error| error.to_string())?;
    let mut implementations: Vec<Option<syn::Item>> = targets.iter().map(|_| None).collect();
    for item in &mut file.items {
        let syn::Item::Struct(message) = item else {
            continue;
        };
        let Some(index) = targets.iter().position(|&name| message.ident == name) else {
            continue;
        };
        let name = &message.ident;
        let implementation = &mut implementations[index];
        if implementation.is_some() || !message.generics.params.is_empty() {
            return Err(format!("expected one non-generic generated {name} struct"));
        }
        let syn::Fields::Named(fields) = &message.fields else {
            return Err(format!("expected named fields in generated {name}"));
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
                "expected exactly one derived Clone on generated {name}"
            ));
        }
        *implementation = Some(syn::parse_quote! {
            #[automatically_derived]
            impl ::core::clone::Clone for #name {
                #[inline(never)]
                fn clone(&self) -> Self {
                    Self { #(#names: ::core::clone::Clone::clone(&self.#names)),* }
                }
            }
        });
    }
    for (target, implementation) in targets.iter().zip(implementations) {
        file.items
            .push(implementation.ok_or_else(|| format!("generated {target} struct is missing"))?);
    }
    Ok(prettyplease::unparse(&file))
}

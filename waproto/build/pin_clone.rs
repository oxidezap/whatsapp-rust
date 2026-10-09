//! Keep the recursive Message clone in one crate. Rust's derived Clone is
//! inline, so downstream callers otherwise carry separate copies of this large
//! tree even with LTO. Derive the replacement from the generated Rust fields,
//! not a second hand-maintained schema, and keep every other derive unchanged.

pub fn generate(source: &str) -> Result<String, String> {
    let mut file = syn::parse_file(source).map_err(|error| error.to_string())?;
    let mut implementation = None;
    for item in &mut file.items {
        let syn::Item::Struct(message) = item else {
            continue;
        };
        if message.ident != "Message" {
            continue;
        }
        if implementation.is_some() || !message.generics.params.is_empty() {
            return Err("expected one non-generic generated Message struct".into());
        }
        let syn::Fields::Named(fields) = &message.fields else {
            return Err("expected named fields in generated Message".into());
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
            return Err("expected exactly one derived Clone on generated Message".into());
        }
        implementation = Some(syn::parse_quote! {
            #[automatically_derived]
            impl ::core::clone::Clone for Message {
                #[inline(never)]
                fn clone(&self) -> Self {
                    Self { #(#names: ::core::clone::Clone::clone(&self.#names)),* }
                }
            }
        });
    }
    file.items
        .push(implementation.ok_or("generated Message struct is missing")?);
    Ok(prettyplease::unparse(&file))
}

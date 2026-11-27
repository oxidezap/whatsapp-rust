#![forbid(unsafe_code)]

use crate::emit_owned::RT;
use crate::raw;
use crate::schema::Schema;

pub fn generate_private_owned(
    schema: &Schema,
    base: &str,
    runtime: &str,
) -> Result<String, syn::Error> {
    use syn::visit_mut::VisitMut as _;
    let _: syn::Path = syn::parse_str(runtime)?;
    let source = generate_private_owned_raw(schema, base, runtime);
    let mut parsed = syn::parse_file(&source)?;
    struct BinaryOnly;
    impl syn::visit_mut::VisitMut for BinaryOnly {
        fn visit_attributes_mut(&mut self, attributes: &mut Vec<syn::Attribute>) {
            attributes.retain(|attribute| {
                !attribute.path().is_ident("serde") && !attribute.path().is_ident("cfg_attr")
            });
            for attribute in attributes {
                if attribute.path().is_ident("derive") {
                    let mut paths = attribute.parse_args_with(syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated).expect("emitted derives parse");
                    paths = paths
                        .into_iter()
                        .filter(|path| {
                            !path.segments.first().is_some_and(|segment| {
                                segment.ident == "serde" || segment.ident == "serde_repr"
                            })
                        })
                        .collect();
                    attribute.meta = syn::parse_quote!(derive(#paths));
                }
            }
        }
    }
    BinaryOnly.visit_file_mut(&mut parsed);
    Ok(prettyplease::unparse(&parsed))
}

pub fn generate_private_owned_raw(schema: &Schema, base: &str, runtime: &str) -> String {
    raw::generate_raw_models(schema, base, false).replace(RT, &format!(" {runtime}"))
}

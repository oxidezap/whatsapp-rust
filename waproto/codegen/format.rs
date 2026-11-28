#![forbid(unsafe_code)]

use crate::emit_owned::{Module, generate_modules};
use crate::inventory::inventory_compact;
use crate::schema::Schema;
use std::collections::BTreeSet;

impl Module {
    fn collect_inventory(
        self,
        scope: &str,
        depth: usize,
        api: &mut BTreeSet<String>,
    ) -> Result<(), syn::Error> {
        for item in self.items {
            inventory_fragment(&item, scope, depth, api)?;
        }
        for (name, child) in self.children {
            if name.starts_with("__") {
                continue;
            }
            let module: syn::Item = syn::parse_str(&format!("pub mod {name} {{}}"))?;
            inventory_compact(&[module], scope, api);
            child.collect_inventory(&format!("{scope}::{name}"), depth + 1, api)?;
        }
        Ok(())
    }
}

fn inventory_fragment(
    source: &str,
    scope: &str,
    depth: usize,
    api: &mut BTreeSet<String>,
) -> Result<(), syn::Error> {
    // Prettyplease can insert trailing commas based on the indentation depth.
    let mut wrapped = String::new();
    for level in 0..depth {
        wrapped.push_str(&format!("mod __format_{level} {{\n"));
    }
    wrapped.push_str(source);
    for _ in 0..depth {
        wrapped.push_str("}\n");
    }
    let mut parsed = syn::parse_file(&wrapped)?;
    strip_impl_method_bodies(&mut parsed.items);
    let formatted = syn::parse_file(&prettyplease::unparse(&parsed))?;
    let mut items = formatted.items.as_slice();
    for _ in 0..depth {
        let [syn::Item::Mod(module)] = items else {
            unreachable!("formatter module wrapper");
        };
        items = &module.content.as_ref().expect("inline formatter module").1;
    }
    inventory_compact(items, scope, api);
    Ok(())
}

fn strip_impl_method_bodies(items: &mut [syn::Item]) {
    for item in items {
        match item {
            syn::Item::Impl(implementation) => {
                for item in &mut implementation.items {
                    if let syn::ImplItem::Fn(method) = item {
                        method.block.stmts.clear();
                    }
                }
            }
            syn::Item::Mod(module) => {
                if let Some((_, items)) = &mut module.content {
                    strip_impl_method_bodies(items);
                }
            }
            _ => {}
        }
    }
}

pub fn inventory(schema: &Schema, base: &str) -> Result<BTreeSet<String>, syn::Error> {
    let mut api = BTreeSet::new();
    generate_modules(schema, base, true).collect_inventory("whatsapp", 0, &mut api)?;
    Ok(api)
}

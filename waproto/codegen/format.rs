#![forbid(unsafe_code)]

use crate::emit_owned::{Module, generate_modules};
use crate::schema::Schema;

impl Module {
    fn render_formatted(&self, out: &mut String, depth: usize) -> Result<(), syn::Error> {
        let imports = "#[allow(unused_imports)] use crate::protobuf::Message as _;\n#[allow(unused_imports)] use crate::protobuf::{MessageView as _,ViewEncode as _};\n";
        format_fragment(out, imports, depth)?;
        for item in &self.items {
            format_fragment(out, item, depth)?;
        }
        for (name, child) in &self.children {
            out.push_str(&"    ".repeat(depth));
            out.push_str(&format!("pub mod {name} {{\n"));
            child.render_formatted(out, depth + 1)?;
            out.push_str(&"    ".repeat(depth));
            out.push_str("}\n");
        }
        Ok(())
    }
}

fn format_fragment(out: &mut String, source: &str, depth: usize) -> Result<(), syn::Error> {
    let mut wrapped = String::new();
    for level in 0..depth {
        wrapped.push_str(&format!("mod __format_{level} {{\n"));
    }
    wrapped.push_str(source);
    for _ in 0..depth {
        wrapped.push_str("}\n");
    }
    let formatted = prettyplease::unparse(&syn::parse_file(&wrapped)?);
    let mut body = formatted.as_str();
    for level in 0..depth {
        let opening = format!("{}mod __format_{level} {{\n", "    ".repeat(level));
        body = body
            .strip_prefix(&opening)
            .expect("formatter module opening");
    }
    for level in 0..depth {
        let closing = format!("{}}}\n", "    ".repeat(level));
        body = body
            .strip_suffix(&closing)
            .expect("formatter module closing");
    }
    out.push_str(body);
    Ok(())
}

pub fn generate(schema: &Schema, base: &str) -> Result<String, syn::Error> {
    let root = generate_modules(schema, base, true);
    let mut source = String::new();
    root.render_formatted(&mut source, 0)?;
    Ok(source)
}

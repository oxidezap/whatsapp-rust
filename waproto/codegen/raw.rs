#![forbid(unsafe_code)]

use crate::emit_owned::{Module, generate_modules};
use crate::schema::Schema;

impl Module {
    fn render(&self, out: &mut String) {
        out.push_str("#[allow(unused_imports)] use crate::protobuf::Message as _;\n");
        out.push_str(
            "#[allow(unused_imports)] use crate::protobuf::{MessageView as _,ViewEncode as _};\n",
        );
        for item in &self.items {
            out.push_str(item);
        }
        for (name, child) in &self.children {
            out.push_str(&format!("pub mod {name} {{\n"));
            child.render(out);
            out.push_str("}\n");
        }
    }
}

pub fn generate_raw(schema: &Schema, base: &str) -> String {
    generate_raw_models(schema, base, true)
}

pub(crate) fn generate_raw_models(schema: &Schema, base: &str, views: bool) -> String {
    let root = generate_modules(schema, base, views);
    let mut source = String::new();
    root.render(&mut source);
    source
}

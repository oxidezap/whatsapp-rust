#![forbid(unsafe_code)]

use crate::emit_owned::{Module, generate_modules};
use crate::schema::Schema;
use std::io::{self, BufWriter, Write};
use std::path::Path;

impl Module {
    fn render(self, out: &mut impl Write) -> io::Result<()> {
        out.write_all(
            b"#[allow(unused_imports)] use crate::protobuf::Message as _;\n\
              #[allow(unused_imports)] use crate::protobuf::{MessageView as _,ViewEncode as _};\n",
        )?;
        for item in self.items {
            out.write_all(item.as_bytes())?;
        }
        for (name, child) in self.children {
            writeln!(out, "pub mod {name} {{")?;
            child.render(out)?;
            out.write_all(b"}\n")?;
        }
        Ok(())
    }
}

pub fn write_whatsapp(schema: &Schema, base: &str, path: &Path) -> io::Result<()> {
    let mut out = BufWriter::new(std::fs::File::create(path)?);
    generate_modules(schema, base, true).render(&mut out)?;
    out.flush()
}

#[cfg(test)]
pub fn generate_raw(schema: &Schema, base: &str) -> String {
    generate_raw_models(schema, base, true)
}

pub(crate) fn generate_raw_models(schema: &Schema, base: &str, views: bool) -> String {
    let mut source = Vec::new();
    generate_modules(schema, base, views)
        .render(&mut source)
        .expect("writing rendered source to memory");
    String::from_utf8(source).expect("rendered source is UTF-8")
}

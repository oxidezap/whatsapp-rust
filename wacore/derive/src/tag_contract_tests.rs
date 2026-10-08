use super::*;
use std::path::PathBuf;
use std::process::{Command, Output};

// Compile the emitted declaration across a real crate boundary. Strip only
// the recursive wire derive and its helper attributes: serde/wire behavior
// is exercised by wacore/tests/wire_enum_serde_test.rs. Keeping this probe
// dependency-free lets it run with the proc-macro crate's existing test host.
fn emitted_tag(source: &str) -> String {
    let input: DeriveInput = syn::parse_str(source).unwrap();
    let Data::Enum(data) = &input.data else {
        panic!("expected enum");
    };
    let expanded = expand_wire_enum_tagged(&input, &data.variants, "type", None);
    let file: syn::File = syn::parse2(expanded).unwrap();
    let mut tag = file
        .items
        .into_iter()
        .find_map(|item| match item {
            syn::Item::Enum(item) => Some(item),
            _ => None,
        })
        .expect("generated sibling enum");
    tag.attrs.retain(|attr| !attr.path().is_ident("derive"));
    for variant in &mut tag.variants {
        variant.attrs.retain(|attr| {
            !["wire", "wire_alias", "wire_fallback"]
                .iter()
                .any(|name| attr.path().is_ident(name))
        });
    }
    quote!(#tag).to_string()
}

struct Compiler(PathBuf);

impl Compiler {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("wire-enum-tags-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        Self(dir)
    }

    fn compile(&self, name: &str, source: &str, consumer: bool) -> Output {
        let source_path = self.0.join(format!("{name}.rs"));
        std::fs::write(&source_path, source).unwrap();
        let mut rustc = Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()));
        rustc.args(["--edition=2024", "--crate-type=lib", "--crate-name", name]);
        rustc.arg(&source_path).arg("--out-dir").arg(&self.0);
        if consumer {
            rustc.arg("--extern").arg(format!(
                "schema={}",
                self.0.join("libschema.rlib").display()
            ));
        }
        rustc.output().expect("run rustc with the active toolchain")
    }

    fn pass(&self, name: &str, source: &str, consumer: bool) {
        let output = self.compile(name, source, consumer);
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn fail(&self, source: &str, code: &str) {
        let output = self.compile("consumer", source, true);
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(!output.status.success(), "negative control compiled");
        assert!(stderr.contains(code), "expected {code}: {stderr}");
    }
}

impl Drop for Compiler {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn emitted_tags_preserve_the_external_evolution_and_visibility_contract() {
    let compiler = Compiler::new();
    let old_consumer = "pub fn dispatch(tag: schema::ActionTag) -> bool {
        match tag { schema::ActionTag::Known => true, _ => false }
    }";
    for variants in ["", "#[wire = \"new\"] Added,"] {
        let tag = emitted_tag(&format!(
            "#[non_exhaustive] pub enum Action {{ #[wire = \"known\"] Known, {variants} }}"
        ));
        compiler.pass("schema", &tag, false);
        // The exact same old consumer works before and after schema growth.
        compiler.pass("consumer", old_consumer, true);
        if variants.is_empty() {
            compiler.fail(
                "pub fn dispatch(tag: schema::ActionTag) {
                    match tag { schema::ActionTag::Known => {} }
                }",
                "E0004",
            );
            // The defining crate may still match all its own variants.
            compiler.pass(
                "schema",
                &format!(
                    "{tag}
                fn dispatch(tag: ActionTag) {{ match tag {{ ActionTag::Known => {{}} }} }}"
                ),
                false,
            );
        }
    }

    let closed = emitted_tag("pub enum Closed { #[wire = \"known\"] Known }");
    compiler.pass("schema", &closed, false);
    compiler.pass(
        "consumer",
        "pub fn dispatch(tag: schema::ClosedTag) {
        match tag { schema::ClosedTag::Known => {} }
    }",
        true,
    );

    for visibility in ["", "pub(crate)", "pub(super)", "pub(in crate::outer)"] {
        let tag = emitted_tag(&format!(
            "{visibility} enum Hidden {{ #[wire = \"known\"] Known }}"
        ));
        let within = "fn local() { let _ = HiddenTag::Known; }";
        compiler.pass(
            "schema",
            &format!("pub mod outer {{ pub mod inner {{ {tag} {within} }} }}"),
            false,
        );
        compiler.fail("use schema::outer::inner::HiddenTag;", "E0603");
        let sibling = format!(
            "pub mod outer {{ pub mod inner {{ {tag} }}
            fn sibling() {{ let _ = inner::HiddenTag::Known; }} }}"
        );
        let output = compiler.compile("schema", &sibling, false);
        if visibility.is_empty() {
            assert!(!output.status.success());
            assert!(String::from_utf8_lossy(&output.stderr).contains("E0603"));
        } else {
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}

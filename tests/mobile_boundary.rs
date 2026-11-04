//! Runtime and I/O independence of mobile protocol decisions.
//!
//! Scan the source tree rather than a hand-maintained file list, so a new mobile
//! module cannot escape the boundary by omission. Host adapters belong elsewhere.

use std::path::{Path, PathBuf};

use syn::{Expr, parse::Parser, punctuated::Punctuated, visit::Visit};

#[derive(Default)]
struct Boundary {
    violations: Vec<String>,
}

impl Boundary {
    fn check_path(&mut self, path: impl IntoIterator<Item = String>) {
        let parts: Vec<_> = path.into_iter().collect();
        let joined = parts.join("::");
        let forbidden = parts.iter().any(|part| {
            matches!(
                part.as_str(),
                "tokio"
                    | "async_std"
                    | "smol"
                    | "reqwest"
                    | "ureq"
                    | "hyper"
                    | "fs"
                    | "net"
                    | "io"
                    | "thread"
                    | "process"
                    | "env"
                    | "SystemTime"
                    | "OsRng"
                    | "rand"
                    | "rand_core"
                    | "getrandom"
                    | "chrono"
                    | "now_secs"
                    | "now_millis"
                    | "now_utc"
                    | "elapsed"
                    | "spawn"
                    | "sleep"
            )
        });
        let global_clock = joined.starts_with("crate::time")
            || joined.starts_with("wacore::time")
            || joined.starts_with("std::time::Instant");
        let reads_now = parts.len() > 1 && parts.last().is_some_and(|part| part == "now");
        if forbidden || global_clock || reads_now {
            self.violations.push(joined);
        }
    }

    fn check_use(&mut self, prefix: &mut Vec<String>, tree: &syn::UseTree) {
        match tree {
            syn::UseTree::Path(path) => {
                prefix.push(path.ident.to_string());
                self.check_use(prefix, &path.tree);
                prefix.pop();
            }
            syn::UseTree::Name(name) => {
                self.check_path(prefix.iter().cloned().chain([name.ident.to_string()]));
            }
            syn::UseTree::Rename(rename) => {
                self.check_path(prefix.iter().cloned().chain([rename.ident.to_string()]));
            }
            syn::UseTree::Group(group) => {
                for item in &group.items {
                    self.check_use(prefix, item);
                }
            }
            syn::UseTree::Glob(_) => self.violations.push("wildcard import".into()),
        }
    }
}

struct MatchesArguments {
    value: Expr,
    pattern: syn::Pat,
    guard: Option<Expr>,
}

impl syn::parse::Parse for MatchesArguments {
    fn parse(input: syn::parse::ParseStream<'_>) -> syn::Result<Self> {
        let value = input.parse()?;
        input.parse::<syn::Token![,]>()?;
        let pattern = syn::Pat::parse_multi_with_leading_vert(input)?;
        let guard = if input.peek(syn::Token![if]) {
            input.parse::<syn::Token![if]>()?;
            Some(input.parse()?)
        } else {
            None
        };
        if input.peek(syn::Token![,]) {
            input.parse::<syn::Token![,]>()?;
        }
        Ok(Self {
            value,
            pattern,
            guard,
        })
    }
}

impl<'ast> Visit<'ast> for Boundary {
    fn visit_item_mod(&mut self, module: &'ast syn::ItemMod) {
        if module.attrs.iter().any(|attr| {
            attr.path().is_ident("cfg")
                && attr
                    .parse_args::<syn::Path>()
                    .is_ok_and(|path| path.is_ident("test"))
        }) {
            return;
        }
        syn::visit::visit_item_mod(self, module);
    }

    fn visit_item_use(&mut self, item: &'ast syn::ItemUse) {
        self.check_use(&mut Vec::new(), &item.tree);
    }

    fn visit_path(&mut self, path: &'ast syn::Path) {
        self.check_path(
            path.segments
                .iter()
                .map(|segment| segment.ident.to_string()),
        );
        syn::visit::visit_path(self, path);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        if call.method == "now" {
            self.violations.push("ambient now() method".into());
        }
        self.check_path([call.method.to_string()]);
        syn::visit::visit_expr_method_call(self, call);
    }

    fn visit_expr_await(&mut self, _: &'ast syn::ExprAwait) {
        self.violations
            .push("await in protocol decision logic".into());
    }

    fn visit_signature(&mut self, signature: &'ast syn::Signature) {
        if signature.asyncness.is_some() {
            self.violations.push("async protocol function".into());
        }
        syn::visit::visit_signature(self, signature);
    }

    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        self.check_path(
            mac.path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string()),
        );
        if mac.path.is_ident("matches") {
            match syn::parse2::<MatchesArguments>(mac.tokens.clone()) {
                Ok(arguments) => {
                    self.visit_expr(&arguments.value);
                    self.visit_pat(&arguments.pattern);
                    if let Some(guard) = arguments.guard {
                        self.visit_expr(&guard);
                    }
                }
                Err(_) => self.violations.push("uninspectable match arguments".into()),
            }
            return;
        }
        if ![
            "vec",
            "format",
            "format_args",
            "assert",
            "assert_eq",
            "assert_ne",
            "write",
        ]
        .iter()
        .any(|name| mac.path.is_ident(name))
        {
            self.violations
                .push("unreviewed macro in protocol logic".into());
            return;
        }
        let parser = Punctuated::<Expr, syn::Token![,]>::parse_terminated;
        match parser.parse2(mac.tokens.clone()) {
            Ok(arguments) => {
                for argument in &arguments {
                    self.visit_expr(argument);
                }
            }
            Err(_) => self.violations.push("uninspectable macro arguments".into()),
        }
    }
}

fn violations(source: &str) -> Vec<String> {
    let mut boundary = Boundary::default();
    boundary.visit_file(&syn::parse_file(source).expect("valid Rust source"));
    boundary.violations
}

fn rust_sources(directory: &Path, sources: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory).expect("mobile source directory") {
        let path = entry.expect("source entry").path();
        if path.is_dir() {
            rust_sources(&path, sources);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            sources.push(path);
        }
    }
}

#[test]
fn mobile_decisions_are_runtime_and_io_independent() {
    let mut sources = Vec::new();
    rust_sources(
        &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("wacore/src/mobile"),
        &mut sources,
    );
    assert!(!sources.is_empty(), "mobile guard must not pass vacuously");
    for path in sources {
        let source = std::fs::read_to_string(&path).expect("read mobile source");
        assert_eq!(
            violations(&source),
            Vec::<String>::new(),
            "{}",
            path.display()
        );
    }
}

#[test]
fn guard_rejects_aliased_services_and_nested_clock_reads() {
    for source in [
        "use tokio::task as jobs; fn f() { jobs::spawn(async {}); }",
        "use std::fs as files; fn f() { files::read(\"secret\"); }",
        "use std::time::Instant as Clock; fn f() { Clock::now(); }",
        "use crate::time as clock; fn f() { clock::now_millis(); }",
        "use getrandom::fill as entropy; fn f() { entropy(&mut []); }",
        "fn f() { let _ = format!(\"{}\", std::time::SystemTime::now()); }",
        "fn f() { opaque_macro!(unknown => code); }",
        "fn f() { let _ = matches!(crate::time::now_secs(), 0); }",
        "fn f(clock: Clock) { clock.now(); }",
        "fn f() { std::env::var(\"URL\"); }",
        "async fn f() {}",
        "#[cfg(test)] mod tests {} fn f() { std::net::TcpStream::connect(\"host\"); }",
    ] {
        assert!(!violations(source).is_empty(), "{source}");
    }
}

#[test]
fn guard_accepts_explicit_time_and_test_only_adapters() {
    assert!(
        violations("fn expired(now: u64, deadline: u64) -> bool { now >= deadline }").is_empty()
    );
    assert!(
        violations(
            "#[cfg(test)] mod tests { fn f() { std::time::Instant::now(); } } fn pure() {} "
        )
        .is_empty()
    );
}

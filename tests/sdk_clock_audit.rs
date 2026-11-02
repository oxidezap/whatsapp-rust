use std::path::Path;
use syn::{ExprPath, visit::Visit};

fn requires_fixture(meta: &syn::Meta) -> bool {
    match meta {
        syn::Meta::Path(path) => path.is_ident("test"),
        syn::Meta::NameValue(value) if value.path.is_ident("feature") => {
            matches!(&value.value, syn::Expr::Lit(syn::ExprLit {lit:syn::Lit::Str(value),..}) if ["bench-harness","test-support"].contains(&value.value().as_str()))
        }
        syn::Meta::List(list) => {
            use syn::parse::Parser;
            let parser = syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated;
            let Ok(parts) = parser.parse2(list.tokens.clone()) else {
                return false;
            };
            if list.path.is_ident("all") {
                parts.iter().any(requires_fixture)
            } else if list.path.is_ident("any") {
                !parts.is_empty() && parts.iter().all(requires_fixture)
            } else {
                false
            }
        }
        _ => false,
    }
}

fn fixture(attrs: &[syn::Attribute]) -> bool {
    attrs.iter().any(|attr| {
        attr.path()
            .segments
            .last()
            .is_some_and(|part| part.ident == "test")
            || attr.path().is_ident("cfg")
                && attr
                    .parse_args::<syn::Meta>()
                    .is_ok_and(|meta| requires_fixture(&meta))
    })
}

#[derive(Default)]
struct Audit {
    function: String,
    calls: Vec<(String, String)>,
}

impl<'ast> Visit<'ast> for Audit {
    fn visit_item_mod(&mut self, node: &'ast syn::ItemMod) {
        if !fixture(&node.attrs) {
            syn::visit::visit_item_mod(self, node);
        }
    }
    fn visit_item_fn(&mut self, node: &'ast syn::ItemFn) {
        if fixture(&node.attrs) {
            return;
        }
        let previous = std::mem::replace(&mut self.function, node.sig.ident.to_string());
        syn::visit::visit_item_fn(self, node);
        self.function = previous;
    }
    fn visit_impl_item_fn(&mut self, node: &'ast syn::ImplItemFn) {
        if fixture(&node.attrs) {
            return;
        }
        let previous = std::mem::replace(&mut self.function, node.sig.ident.to_string());
        syn::visit::visit_impl_item_fn(self, node);
        self.function = previous;
    }
    fn visit_expr_path(&mut self, node: &'ast ExprPath) {
        let name = node.path.segments.last().unwrap().ident.to_string();
        if node.path.segments.len() > 1
            && ([
                "now_secs",
                "now_secs_u64",
                "now_millis",
                "now_utc",
                "from_secs_or_now",
                "from_millis_or_now",
            ]
            .contains(&name.as_str())
                || name == "now")
        {
            self.calls.push((
                self.function.clone(),
                node.path
                    .segments
                    .iter()
                    .map(|s| s.ident.to_string())
                    .collect::<Vec<_>>()
                    .join("::"),
            ));
        }
        syn::visit::visit_expr_path(self, node);
    }
    fn visit_expr_method_call(&mut self, node: &'ast syn::ExprMethodCall) {
        if node.method == "elapsed" {
            self.calls.push((self.function.clone(), "elapsed".into()));
        }
        syn::visit::visit_expr_method_call(self, node);
    }
}

fn scan(root: &Path, repository: &Path, failures: &mut Vec<String>) {
    let mut entries = std::fs::read_dir(root)
        .unwrap()
        .map(|e| e.unwrap().path())
        .collect::<Vec<_>>();
    entries.sort();
    for path in entries {
        let name = path.file_name().unwrap().to_str().unwrap();
        if ["tests", "test_utils.rs", "test_support", "bench_support"].contains(&name)
            || name.contains("bench")
            || name.ends_with("tests.rs")
            || name.ends_with("_test.rs")
        {
            continue;
        }
        if path.is_dir() {
            scan(&path, repository, failures);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            let source = std::fs::read_to_string(&path).unwrap();
            let mut visitor = Audit::default();
            visitor.visit_file(&syn::parse_file(&source).unwrap());
            let relative = path.strip_prefix(repository).unwrap().to_str().unwrap();
            for (function, call) in visitor.calls {
                let legacy = matches!(
                    (relative, function.as_str()),
                    ("src/lid_pn_cache/implementation.rs", "warm_up")
                        | ("src/mediaconn.rs", "is_expired")
                        | ("src/portable_cache/implementation.rs", "entry_time")
                        | ("src/store/signal.rs", "load_signed_prekey")
                        | ("src/version.rs", "resolve_version_with_runtime")
                        | ("src/voip/transport/native.rs", "now")
                        | ("src/voip/transport/native.rs", "prefer_web_client_port")
                );
                if !legacy {
                    failures.push(format!("{relative}::{function}: {call}"));
                }
            }
        }
    }
}

#[test]
fn direct_sdk_clock_reads_stay_at_legacy_and_native_adapter_boundaries() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut failures = Vec::new();
    scan(&repository.join("src"), repository, &mut failures);
    assert!(failures.is_empty(), "ambient SDK time reads: {failures:#?}");
}

#[test]
fn clock_audit_skips_fixtures_and_accepts_explicit_ports() {
    let mut audit = Audit::default();
    audit.visit_file(&syn::parse_file(r#"#[cfg(all(test, feature = "fixture"))] mod tests { fn f() { wacore::time::now_secs(); } } fn f(rt: &dyn Runtime) {rt.wall_time_secs();}"#).unwrap());
    assert!(audit.calls.is_empty());
    audit.visit_file(&syn::parse_file("fn f() { let _ = wacore::time::now_secs(); }").unwrap());
    assert_eq!(audit.calls.len(), 1);
}

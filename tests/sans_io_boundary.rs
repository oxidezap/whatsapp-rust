use syn::visit::Visit;

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
                "runtime"
                    | "Runtime"
                    | "Backend"
                    | "Transport"
                    | "tokio"
                    | "rand"
                    | "chrono"
                    | "async_channel"
                    | "fs"
                    | "thread"
                    | "net"
                    | "http"
                    | "SystemTime"
                    | "now_secs"
                    | "now_millis"
                    | "now_utc"
                    | "elapsed"
                    | "spawn"
                    | "sleep"
            )
        });
        let time_import = ["crate::time", "wacore::time"].iter().any(|prefix| {
            joined.starts_with(prefix)
                && joined != format!("{prefix}::Instant")
                && !joined.starts_with(&format!("{prefix}::Instant::"))
        });
        let std_time = joined.starts_with("std::time")
            && joined != "std::time::Duration"
            && !joined.starts_with("std::time::Duration::");
        let reads_now = parts.len() > 1 && parts.last().is_some_and(|part| part == "now");
        if forbidden || time_import || std_time || reads_now {
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
        self.check_path(path.segments.iter().map(|s| s.ident.to_string()));
        syn::visit::visit_path(self, path);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        self.check_path([call.method.to_string()]);
        syn::visit::visit_expr_method_call(self, call);
    }

    fn visit_expr_await(&mut self, _: &'ast syn::ExprAwait) {
        self.violations.push("await".into());
    }

    fn visit_signature(&mut self, signature: &'ast syn::Signature) {
        if signature.asyncness.is_some() {
            self.violations.push("async function".into());
        }
        syn::visit::visit_signature(self, signature);
    }
}

fn violations(source: &str) -> Vec<String> {
    let mut boundary = Boundary::default();
    boundary.visit_file(&syn::parse_file(source).expect("valid source"));
    boundary.violations
}

#[test]
fn protocol_state_does_not_execute_io_or_read_global_time() {
    for (name, source) in [
        ("health", include_str!("../wacore/src/protocol/health.rs")),
        (
            "reconnect",
            include_str!("../wacore/src/protocol/reconnect.rs"),
        ),
        ("retry", include_str!("../wacore/src/protocol/retry.rs")),
        ("requests", include_str!("../wacore/src/request/pending.rs")),
        (
            "frames",
            include_str!("../wacore/src/socket/frame_encoder.rs"),
        ),
        (
            "appstate",
            include_str!("../wacore/src/appstate_sync/preparation.rs"),
        ),
        ("history", include_str!("../src/history_sync/parse_job.rs")),
    ] {
        assert_eq!(violations(source), Vec::<String>::new(), "{name}");
    }
}

#[test]
fn guard_detects_aliased_clocks_and_runtime_dependencies() {
    for source in [
        "use crate::time::now_millis as clock; fn read() { clock(); }",
        "use crate::time as clock; fn read() { clock::now_utc(); }",
        "use std::time::Instant as Clock; fn read() { Clock::now(); }",
        "fn read(at: crate::time::Instant) { at.elapsed(); }",
        "fn read() { crate::time::now_secs(); }",
        "use tokio::task as jobs; fn start() { jobs::spawn(async {}); }",
        "async fn read() {}",
    ] {
        assert!(!violations(source).is_empty(), "{source}");
    }
    assert!(violations("use crate::time::Instant; fn age(now: Instant, at: Instant) { now.saturating_duration_since(at); }").is_empty());
}

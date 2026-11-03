//! Keep the protocol engine usable by deterministic, executor-free hosts.
use syn::visit::Visit;

#[derive(Default)]
struct Boundary(Vec<String>);
impl<'ast> Visit<'ast> for Boundary {
    fn visit_item_mod(&mut self, item: &'ast syn::ItemMod) {
        if item.attrs.iter().any(|attr| {
            attr.path().is_ident("cfg")
                && matches!(&attr.meta,syn::Meta::List(list)if list.tokens.to_string()=="test")
        }) {
            return;
        }
        syn::visit::visit_item_mod(self, item);
    }
    fn visit_item_static(&mut self, item: &'ast syn::ItemStatic) {
        self.0.push(format!("global state {}", item.ident));
    }
    fn visit_signature(&mut self, item: &'ast syn::Signature) {
        if item.asyncness.is_some() {
            self.0.push(format!("async function {}", item.ident));
        }
        syn::visit::visit_signature(self, item);
    }
    fn visit_path(&mut self, path: &'ast syn::Path) {
        let parts: Vec<_> = path
            .segments
            .iter()
            .map(|part| part.ident.to_string())
            .collect();
        if parts.iter().any(|part| {
            matches!(
                part.as_str(),
                "tokio"
                    | "rand"
                    | "rand_core"
                    | "getrandom"
                    | "chrono"
                    | "SystemTime"
                    | "Instant"
                    | "OsRng"
            )
        }) || parts.first().is_some_and(|part| part == "std")
            && parts.get(1).is_some_and(|part| {
                matches!(
                    part.as_str(),
                    "fs" | "io" | "net" | "process" | "thread" | "env" | "time"
                )
            })
        {
            self.0.push(parts.join("::"));
        }
        syn::visit::visit_path(self, path);
    }
}

#[test]
fn signal_transitions_have_no_io_executor_clock_or_global_randomness() {
    for (name, source) in [
        ("lib", include_str!("../wacore/signal-engine/src/lib.rs")),
        (
            "codec",
            include_str!("../wacore/signal-engine/src/codec.rs"),
        ),
        (
            "crypto",
            include_str!("../wacore/signal-engine/src/crypto.rs"),
        ),
        (
            "rustcrypto",
            include_str!("../wacore/signal-engine/src/rustcrypto.rs"),
        ),
        (
            "pairwise",
            include_str!("../wacore/signal-engine/src/pairwise.rs"),
        ),
        (
            "sender",
            include_str!("../wacore/signal-engine/src/sender.rs"),
        ),
        (
            "state",
            include_str!("../wacore/signal-engine/src/state.rs"),
        ),
        ("wire", include_str!("../wacore/signal-engine/src/wire.rs")),
    ] {
        let syntax = syn::parse_file(source).unwrap();
        let mut boundary = Boundary::default();
        boundary.visit_file(&syntax);
        assert!(boundary.0.is_empty(), "{name}: {:?}", boundary.0);
    }
}

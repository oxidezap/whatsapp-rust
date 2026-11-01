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
                "runtime"
                    | "Runtime"
                    | "Backend"
                    | "Transport"
                    | "tokio"
                    | "rand"
                    | "rand_core"
                    | "getrandom"
                    | "OsRng"
                    | "io"
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
        let unreviewed_root = parts.len() > 1
            && parts.first().is_some_and(|root| {
                root.starts_with(char::is_lowercase)
                    && ![
                        "crate",
                        "super",
                        "self",
                        "std",
                        "core",
                        "alloc",
                        "wacore",
                        "wacore_binary",
                        "waproto",
                        "wa",
                        "bytes",
                        "u8",
                        "u16",
                        "u32",
                        "u64",
                        "u128",
                        "usize",
                        "i8",
                        "i16",
                        "i32",
                        "i64",
                        "i128",
                        "isize",
                        "f32",
                        "f64",
                        "str",
                        "bool",
                        "char",
                    ]
                    .contains(&root.as_str())
            });
        if forbidden || time_import || std_time || reads_now || unreviewed_root {
            self.violations.push(joined);
        }
    }

    fn check_import(&mut self, path: impl IntoIterator<Item = String>) {
        let parts: Vec<_> = path.into_iter().collect();
        let joined = parts.join("::");
        const ALLOWED: &[&str] = &[
            "std::collections::HashMap",
            "std::sync::Arc",
            "std::num::NonZeroU64",
            "std::time::Duration",
            "bytes::Bytes",
            "bytes::BytesMut",
            "waproto::whatsapp",
            "wacore_binary::CompactString",
            "wacore_binary::Node",
            "wacore_binary::NodeContent",
            "wacore_binary::NodeRef",
            "wacore_binary::builder::NodeBuilder",
            "super::IndexMac",
            "super::HistorySecretSeedCollector",
            "super::error::EncryptSendError",
            "super::keepalive::KEEP_ALIVE_INTERVAL_MIN",
            "super::keepalive::elapsed_since_at",
            "super::keepalive::is_dead_socket_at",
            "crate::time::Instant",
            "crate::appstate::hash::HashState",
            "crate::appstate::keys::ExpandedAppStateKeys",
            "crate::appstate::processor::PatchProcessingResult",
            "crate::appstate::processor::ProcessedSnapshot",
            "crate::appstate::AppStateError",
            "crate::appstate::Mutation",
            "crate::appstate::process_patch",
            "crate::appstate::process_snapshot",
            "crate::handshake::NoiseCipher",
            "crate::handshake::NoiseError",
            "crate::libsignal::crypto::GcmInPlaceBuffer",
            "crate::iq::prekeys::OneTimePreKeyNode",
            "crate::iq::prekeys::SignedPreKeyNode",
            "crate::libsignal::protocol::PublicKey",
            "crate::protocol::ProtocolNode",
            "crate::types::wire_enums::RECEIPT_MODE_HID_FAILED_DECRYPT",
            "wacore::history_sync::HistorySyncError",
            "wacore::history_sync::HistorySyncResult",
            "wacore::history_sync::process_history_sync_bytes_with_record_sink",
            "wacore::store::traits::MsgSecretEntry",
        ];
        if !ALLOWED.contains(&joined.as_str()) {
            self.violations.push(format!("unreviewed import {joined}"));
        }
        self.check_path(parts);
    }

    fn check_use(&mut self, prefix: &mut Vec<String>, tree: &syn::UseTree) {
        match tree {
            syn::UseTree::Path(path) => {
                prefix.push(path.ident.to_string());
                self.check_use(prefix, &path.tree);
                prefix.pop();
            }
            syn::UseTree::Name(name) => {
                self.check_import(prefix.iter().cloned().chain([name.ident.to_string()]));
            }
            syn::UseTree::Rename(rename) => {
                self.check_import(prefix.iter().cloned().chain([rename.ident.to_string()]));
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
    fn visit_macro(&mut self, mac: &'ast syn::Macro) {
        self.check_path(
            mac.path
                .segments
                .iter()
                .map(|segment| segment.ident.to_string()),
        );
        let parser = Punctuated::<Expr, syn::Token![,]>::parse_terminated;
        match parser.parse2(mac.tokens.clone()) {
            Ok(arguments) => {
                let mut nested = Boundary::default();
                for argument in &arguments {
                    nested.visit_expr(argument);
                }
                self.violations.extend(nested.violations);
            }
            Err(_) => self.violations.push("uninspectable macro arguments".into()),
        }
    }

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
        "fn read() { let _ = format_args!(\"{:?}\", std::time::SystemTime::now()); }",
        "fn read() { opaque_macro!(not => expressions); }",
        "use std::io::Read;",
        "fn read() { std::io::stdin(); }",
        "use getrandom::fill as entropy;",
        "fn read() { getrandom::fill(&mut []); }",
        "use rand_core::OsRng;",
        "fn read() { rand_core::OsRng; }",
        "use unknown_io::read as dependency;",
        "fn read() { unknown_io::read(); }",
        "#[cfg(test)] mod tests {} fn read() { crate::time::now_millis(); }",
    ] {
        assert!(!violations(source).is_empty(), "{source}");
    }
    assert!(violations("use crate::time::Instant; fn age(now: Instant, at: Instant) { now.saturating_duration_since(at); }").is_empty());
}

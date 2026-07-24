//! The contract for the public error surface.
//!
//! Two kinds of test live here. The `surface_*` ones read the crate sources and
//! fail on any error type that departs from the pattern, including one added in
//! a future PR that never touches this file. The rest pin the behaviour that
//! makes a failure recoverable by type.

use std::error::Error as StdError;
use std::path::{Path, PathBuf};
use std::time::Duration;

use whatsapp_rust::client::{ConnectError, ConnectStage};
use whatsapp_rust::handshake::HandshakeError;

use wacore::request::{IqError as CoreIqError, ServerErrorCode};
use wacore::store::error::StoreError;
use whatsapp_rust::features::{
    BlockingError, ChatStateError, CommunityError, ContactError, GroupError, MediaReuploadError,
    MexError, NewsletterError, PollError, PresenceError, ProfileError, StanzaResponseError,
    TcTokenError,
};
use whatsapp_rust::{ClientError, ErrorChainExt, IqError, SendError, ServerRejection};

// ── Source scan ─────────────────────────────────────────────────────────────

fn rust_sources() -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read {dir:?}: {e}"));
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                out.push(path);
            }
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut out = Vec::new();
    walk(&root.join("src"), &mut out);
    walk(&root.join("wacore/src"), &mut out);
    out.sort();
    out
}

/// Every `enum` in `text`, including those nested in inline modules.
fn enums_of(text: &str) -> Vec<syn::ItemEnum> {
    fn collect(items: &[syn::Item], out: &mut Vec<syn::ItemEnum>) {
        for item in items {
            match item {
                syn::Item::Enum(item) => out.push(item.clone()),
                syn::Item::Mod(module) => {
                    if let Some((_, items)) = &module.content {
                        collect(items, out);
                    }
                }
                _ => {}
            }
        }
    }
    // A parse failure must be loud: silently skipping a file would let an
    // unchecked error enum through, which is the whole thing this guards.
    let file = syn::parse_file(text).expect("parse Rust source");
    let mut out = Vec::new();
    collect(&file.items, &mut out);
    out
}

fn has_attribute(attrs: &[syn::Attribute], name: &str) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident(name))
}

/// A `thiserror` enum, identified by its variants carrying `#[error(..)]`.
fn is_error_enum(item: &syn::ItemEnum) -> bool {
    item.variants
        .iter()
        .any(|variant| has_attribute(&variant.attrs, "error"))
}

/// Names of error enums in `text` that are `pub` and lack `#[non_exhaustive]`.
fn error_enums_missing_non_exhaustive(text: &str) -> Vec<String> {
    enums_of(text)
        .iter()
        .filter(|item| matches!(item.vis, syn::Visibility::Public(_)))
        .filter(|item| is_error_enum(item))
        .filter(|item| !has_attribute(&item.attrs, "non_exhaustive"))
        .map(|item| item.ident.to_string())
        .collect()
}

/// Names of variants in `text` annotated `#[error(transparent)]`.
fn transparent_variants(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for item in enums_of(text) {
        for variant in &item.variants {
            for attr in &variant.attrs {
                if attr.path().is_ident("error")
                    && attr
                        .parse_args::<syn::Ident>()
                        .is_ok_and(|arg| arg == "transparent")
                {
                    out.push(format!("{}::{}", item.ident, variant.ident));
                }
            }
        }
    }
    out
}

/// `#[error(transparent)]` delegates `source()` to the *wrapped error\'s own*
/// source, so a wrapped leaf disappears from the chain entirely and can never
/// be downcast. `#[error("{0}")]` renders identically and keeps it reachable.
///
/// This is a blanket policy, deliberately wider than the reported symptom: it
/// covers private enums too, because today\'s private error is tomorrow\'s public
/// one and the attribute is invisible at the point where it hurts. Waiving it
/// for a case where erasure is genuinely wanted is a decision to argue for in
/// review, not to make silently.
#[test]
fn surface_has_no_transparent_error_attribute() {
    let mut offenders = Vec::new();
    for file in rust_sources() {
        let text = std::fs::read_to_string(&file).expect("read source");
        for variant in transparent_variants(&text) {
            offenders.push(format!("{} {}", file.display(), variant));
        }
    }
    assert!(
        offenders.is_empty(),
        "`#[error(transparent)]` erases the wrapped error from the source chain. \
         Use `#[error(\"{{0}}\")]`, which renders the same text and keeps it \
         downcastable. Found at:\n  {}",
        offenders.join("\n  ")
    );
}

/// A new variant is a breaking change for anyone matching exhaustively unless
/// the enum is sealed against it.
#[test]
fn surface_error_enums_are_non_exhaustive() {
    let mut offenders = Vec::new();
    for file in rust_sources() {
        let text = std::fs::read_to_string(&file).expect("read source");
        for name in error_enums_missing_non_exhaustive(&text) {
            offenders.push(format!("{} {}", file.display(), name));
        }
    }
    assert!(
        offenders.is_empty(),
        "public error enums must be `#[non_exhaustive]` so a new variant is not \
         a breaking change. Found without it:\n  {}",
        offenders.join("\n  ")
    );
}

// ── The scanner guards itself ───────────────────────────────────────────────
//
// Text scanning got both of these wrong: a blank line between the attribute and
// the declaration hid `#[non_exhaustive]`, and an unbalanced brace in a doc
// comment truncated the enum so it stopped looking like an error enum at all.
// The second was fail-open, which is why this is parsed rather than scanned.

#[test]
fn scanner_sees_attributes_separated_by_a_blank_line() {
    let source = "#[derive(Debug, thiserror::Error)]\n#[non_exhaustive]\n\npub enum E {\n    #[error(\"x\")]\n    V,\n}";
    assert!(error_enums_missing_non_exhaustive(source).is_empty());
}

#[test]
fn scanner_sees_through_unbalanced_braces_in_doc_comments() {
    let source = "#[derive(Debug, thiserror::Error)]\npub enum E {\n    /// shape: { \"k\": v }}\n    #[error(\"x\")]\n    V,\n}";
    assert_eq!(error_enums_missing_non_exhaustive(source), vec!["E"]);
}

#[test]
fn scanner_sees_transparent_followed_by_other_text() {
    let source =
        "pub enum E {\n    #[error(transparent)] // legacy\n    V(#[from] std::fmt::Error),\n}";
    assert_eq!(transparent_variants(source), vec!["E::V"]);
}

#[test]
fn scanner_reads_a_declaration_whose_header_wraps() {
    let source =
        "#[derive(Debug, thiserror::Error)]\npub enum\n    E\n{\n    #[error(\"x\")]\n    V,\n}";
    assert_eq!(error_enums_missing_non_exhaustive(source), vec!["E"]);
}

#[test]
fn scanner_ignores_enums_that_are_not_errors() {
    // Prose mentioning Error, and a variant named Error, must not be enough.
    let source = "/// Errors are boxed here.\n#[derive(Debug)]\npub enum Outcome {\n    Value(u8),\n    Error(Box<u8>),\n}";
    assert!(error_enums_missing_non_exhaustive(source).is_empty());
}

#[test]
fn scanner_ignores_private_and_empty_enums_for_non_exhaustive() {
    let private = "#[derive(Debug, thiserror::Error)]\nenum E {\n    #[error(\"x\")]\n    V,\n}";
    assert!(error_enums_missing_non_exhaustive(private).is_empty());
    let empty = "#[derive(Debug)]\npub enum E {}";
    assert!(error_enums_missing_non_exhaustive(empty).is_empty());
}

#[test]
fn scanner_finds_enums_nested_in_modules() {
    let source = "mod inner {\n    #[derive(Debug, thiserror::Error)]\n    pub enum E {\n        #[error(\"x\")]\n        V,\n    }\n}";
    assert_eq!(error_enums_missing_non_exhaustive(source), vec!["E"]);
}

// ── Helpers ─────────────────────────────────────────────────────────────────

fn rejected(code: u16) -> IqError {
    IqError::ServerError {
        code,
        text: "forbidden".to_string(),
        error_type: Some("cancel".to_string()),
        backoff: None,
    }
}

#[track_caller]
fn assert_source_is<T: StdError + 'static>(err: &(dyn StdError + 'static), what: &str) {
    let source = err
        .source()
        .unwrap_or_else(|| panic!("{what}: source() is None, the wrapped error was erased"));
    assert!(
        source.downcast_ref::<T>().is_some(),
        "{what}: source() is not a {}",
        std::any::type_name::<T>()
    );
}

// ── Every wrapping variant keeps its typed source ───────────────────────────

/// One sample per public variant that wraps a typed error. Complements
/// [`surface_has_no_transparent_error_attribute`]: the scan proves the
/// attribute is right, this proves the resulting chain actually is.
#[test]
fn wrapping_variants_preserve_their_typed_source() {
    assert_source_is::<IqError>(&BlockingError::Iq(rejected(403)), "BlockingError::Iq");
    assert_source_is::<IqError>(&ContactError::Iq(rejected(403)), "ContactError::Iq");
    assert_source_is::<IqError>(&GroupError::Iq(rejected(403)), "GroupError::Iq");
    assert_source_is::<IqError>(&NewsletterError::Iq(rejected(403)), "NewsletterError::Iq");
    assert_source_is::<IqError>(&ProfileError::Iq(rejected(403)), "ProfileError::Iq");
    assert_source_is::<IqError>(&CommunityError::Iq(rejected(403)), "CommunityError::Iq");
    assert_source_is::<IqError>(&TcTokenError::Iq(rejected(403)), "TcTokenError::Iq");

    let mex = || MexError::ExtensionError {
        code: 1,
        message: "denied".to_string(),
    };
    assert_source_is::<MexError>(&GroupError::Mex(mex()), "GroupError::Mex");
    assert_source_is::<MexError>(&CommunityError::Mex(mex()), "CommunityError::Mex");
    assert_source_is::<MexError>(&NewsletterError::Mex(mex()), "NewsletterError::Mex");

    let client = || ClientError::NotConnected;
    assert_source_is::<ClientError>(&PresenceError::Client(client()), "PresenceError::Client");
    assert_source_is::<ClientError>(&ChatStateError::Client(client()), "ChatStateError::Client");
    assert_source_is::<ClientError>(&ProfileError::Client(client()), "ProfileError::Client");
    assert_source_is::<ClientError>(
        &NewsletterError::Client(client()),
        "NewsletterError::Client",
    );
    assert_source_is::<ClientError>(
        &MediaReuploadError::Client(client()),
        "MediaReuploadError::Client",
    );
    assert_source_is::<ClientError>(
        &StanzaResponseError::Client(client()),
        "StanzaResponseError::Client",
    );
    assert_source_is::<ClientError>(&SendError::Client(client()), "SendError::Client");

    assert_source_is::<StoreError>(
        &TcTokenError::Store(StoreError::DeviceNotFound(1)),
        "TcTokenError::Store",
    );
    assert_source_is::<SendError>(&PollError::Send(SendError::NotLoggedIn), "PollError::Send");
    assert_source_is::<GroupError>(
        &CommunityError::Group(GroupError::DescriptionConflict),
        "CommunityError::Group",
    );
}

// ── The reported symptom ────────────────────────────────────────────────────

/// Regression for the original report: a `403` from a group operation was
/// unrecoverable because `GroupError::Iq` erased the `IqError`.
#[test]
fn group_server_rejection_exposes_code_and_text() {
    let err = GroupError::Iq(rejected(403));

    let rejection = err
        .server_rejection()
        .expect("a server rejection is recoverable from a group error");
    assert_eq!(rejection.code, 403);
    assert_eq!(rejection.text, "forbidden");
    assert_eq!(rejection.error_type, Some("cancel"));

    // The same fact is reachable by hand, for a consumer that walks the chain
    // itself rather than using the trait.
    let source = StdError::source(&err).expect("source preserved");
    assert!(matches!(
        source.downcast_ref::<IqError>(),
        Some(IqError::ServerError { code: 403, .. })
    ));
}

/// The other half of the report: the `409` a group description update gets when
/// the `prev` token is stale.
#[test]
fn group_description_conflict_409_exposes_its_code() {
    let err = GroupError::Iq(IqError::ServerError {
        code: 409,
        text: "conflict".to_string(),
        error_type: None,
        backoff: None,
    });
    let rejection = err.server_rejection().expect("409 is recoverable");
    assert_eq!(rejection.code, 409);
    assert_eq!(rejection.error_type, None);
}

/// `Display` is unchanged by the switch away from `transparent`: it still
/// renders the wrapped error verbatim.
#[test]
fn wrapping_variants_render_the_wrapped_error_verbatim() {
    let inner = rejected(403);
    let rendered = inner.to_string();
    assert_eq!(GroupError::Iq(rejected(403)).to_string(), rendered);
    assert_eq!(ProfileError::Iq(rejected(403)).to_string(), rendered);
    assert_eq!(
        PresenceError::Client(ClientError::NotConnected).to_string(),
        ClientError::NotConnected.to_string()
    );
}

// ── Domain-agnostic recovery ────────────────────────────────────────────────

/// Goal: code written against one domain works for every other. The helper
/// below names no concrete error type.
fn code_of(err: &(dyn StdError + 'static)) -> Option<u16> {
    err.server_rejection().map(|r: ServerRejection<'_>| r.code)
}

#[test]
fn server_rejection_is_recovered_the_same_way_in_every_domain() {
    let domains: Vec<Box<dyn StdError>> = vec![
        Box::new(GroupError::Iq(rejected(403))),
        Box::new(NewsletterError::Iq(rejected(403))),
        Box::new(ProfileError::Iq(rejected(403))),
        Box::new(BlockingError::Iq(rejected(403))),
        Box::new(CommunityError::Iq(rejected(403))),
        Box::new(ContactError::Iq(rejected(403))),
        Box::new(TcTokenError::Iq(rejected(403))),
        // Nested two domains deep.
        Box::new(CommunityError::Group(GroupError::Iq(rejected(403)))),
        // Reached through the shared cross-crate carrier instead of an IqError.
        Box::new(GroupError::Internal(anyhow::Error::new(ServerErrorCode {
            code: 403,
            text: "forbidden".to_string(),
            error_type: None,
            backoff: None,
        }))),
    ];
    for err in &domains {
        assert_eq!(code_of(err.as_ref()), Some(403), "failed for {err:?}");
    }
}

#[test]
fn timeout_is_recovered_across_domains() {
    assert!(GroupError::Iq(IqError::Timeout).is_timeout());
    assert!(ProfileError::Iq(IqError::Timeout).is_timeout());
    assert!(CommunityError::Group(GroupError::Iq(IqError::Timeout)).is_timeout());
    assert!(!GroupError::Iq(rejected(403)).is_timeout());
}

/// Connect and handshake run out of time without any `IqError` involved, and
/// the crate already tells those apart from every other connect failure.
#[test]
fn connect_and_handshake_timeouts_are_recovered_too() {
    let connect = ConnectError::Timeout {
        stage: ConnectStage::Socket,
        timeout: Duration::from_secs(10),
    };
    assert!(connect.is_timeout());
    assert!(HandshakeError::Timeout.is_timeout());
    // Reached through the wrapper as well.
    assert!(ConnectError::Handshake(HandshakeError::Timeout).is_timeout());

    // Neighbouring failures of the same flow are not timeouts.
    assert!(!ConnectError::AlreadyConnected.is_timeout());
    assert!(!HandshakeError::StreamClosed.is_timeout());
    assert!(!ConnectError::Handshake(HandshakeError::Disconnected).is_timeout());
}

#[test]
fn transport_loss_is_recovered_across_domains() {
    assert!(PresenceError::Client(ClientError::NotConnected).is_transport_unavailable());
    assert!(NewsletterError::Client(ClientError::NotConnected).is_transport_unavailable());
    assert!(GroupError::Iq(IqError::NotConnected).is_transport_unavailable());
    assert!(SendError::Client(ClientError::NotConnected).is_transport_unavailable());
    // A refusal is not a disconnection.
    assert!(!GroupError::Iq(rejected(403)).is_transport_unavailable());
}

#[test]
fn store_failure_is_recovered_across_domains() {
    let err = TcTokenError::Store(StoreError::DeviceNotFound(7));
    assert!(matches!(
        err.store_failure(),
        Some(StoreError::DeviceNotFound(7))
    ));
    assert!(GroupError::Iq(rejected(403)).store_failure().is_none());
}

/// A `wacore` error reaches the same answers, so the two `IqError` types are
/// not a distinction the consumer has to know about.
#[test]
fn the_wacore_iq_error_answers_identically() {
    let core = CoreIqError::ServerError {
        code: 401,
        text: "unauthorized".to_string(),
        error_type: None,
        backoff: Some(30),
    };
    let rejection = core.server_rejection().expect("recoverable");
    assert_eq!(rejection.code, 401);
    assert_eq!(rejection.backoff, Some(30));
    assert!(CoreIqError::Timeout.is_timeout());
    assert!(CoreIqError::NotConnected.is_transport_unavailable());
}

/// `Internal(anyhow)` is not a dead end: the head of the `anyhow` chain is
/// exposed as `source()` and stays downcastable.
#[test]
fn internal_anyhow_still_exposes_its_head() {
    let err = GroupError::Internal(anyhow::Error::new(rejected(403)));
    let source = StdError::source(&err).expect("anyhow head is exposed");
    assert!(source.downcast_ref::<IqError>().is_some());
    assert_eq!(code_of(&err), Some(403));
}

// ── Categories are recovered, never invented ────────────────────────────────

/// A MEX extension `code` is a GraphQL extension code, a different space from
/// the IQ `code` attribute. Reporting it as a server rejection would make the
/// number mean two things.
#[test]
fn mex_extension_error_is_not_reported_as_a_server_rejection() {
    let err = GroupError::Mex(MexError::ExtensionError {
        code: 403,
        message: "denied".to_string(),
    });
    assert_eq!(err.server_rejection(), None);
    // It is still recoverable by type, just not under a category it does not
    // belong to.
    let source = StdError::source(&err).expect("source preserved");
    assert!(matches!(
        source.downcast_ref::<MexError>(),
        Some(MexError::ExtensionError { code: 403, .. })
    ));
}

/// Errors that carry none of the modelled facts answer `None`/`false` rather
/// than being forced into some bucket.
#[test]
fn errors_without_a_modelled_category_report_nothing() {
    let err = GroupError::InvalidRequest("empty invite code".to_string());
    assert_eq!(err.server_rejection(), None);
    assert!(!err.is_timeout());
    assert!(!err.is_transport_unavailable());
    assert!(err.store_failure().is_none());

    let conflict = GroupError::DescriptionConflict;
    assert_eq!(conflict.server_rejection(), None);
    assert!(!conflict.is_transport_unavailable());
}

// ── Chain access for facts the trait does not model ─────────────────────────

#[test]
fn sources_walks_the_whole_chain_nearest_first() {
    let err = CommunityError::Group(GroupError::Iq(rejected(403)));
    let chain: Vec<&(dyn StdError + 'static)> = err.sources().collect();
    assert_eq!(chain.len(), 3, "community -> group -> iq");
    assert!(chain[0].downcast_ref::<CommunityError>().is_some());
    assert!(chain[1].downcast_ref::<GroupError>().is_some());
    assert!(chain[2].downcast_ref::<IqError>().is_some());
}

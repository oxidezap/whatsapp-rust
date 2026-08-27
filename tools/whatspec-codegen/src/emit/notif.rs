//! `wacore/src/stanza/wire_tags.rs`: the two vocabularies this client routes
//! on -- the tag a stanza arrives under, and a `<notification type>`.
//!
//! Both were string literals spread across handler files, which is the thing
//! `AGENTS.md` asks parsers not to do: a value renamed upstream leaves a `match`
//! arm that still compiles, still reads correctly, and never matches again. The
//! handler is not deleted, it is orphaned, and nothing says so.
//!
//! ## Where the tag set comes from
//!
//! No single document lists every top-level tag. `notif` carries the tags WA
//! Web's stanza dispatcher registers, which is most of them but not `iq` (that
//! goes through its request/reply layer) and not `ack`. So the set is the union
//! of every document that names a tag a stanza can arrive under: `notif`'s
//! dispatcher table, `srvreq`'s incoming requests, and the type of an outgoing
//! `stanza`. Taking `notif` alone would omit two tags this repository has
//! handlers for, and a "complete" list missing them would be worse than no list
//! -- it would read as an assertion that they do not exist.
//!
//! Both enums are closed. An unknown value is not something either call site
//! wants to hold: the router simply matches no handler, and the notification
//! dispatcher already forwards anything it does not recognize as a raw event.
//! Closed also keeps `as_str()` returning `&'static str`, which is what the
//! handler trait is declared to return.

use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, ensure};

use crate::ir::{NotifIr, SrvReqIr, StanzaIr};
use crate::naming::pascal_case;

const HEADER: &str = "\
//! The tag a stanza arrives under, and the type of a `<notification>`.
//!
//! Regenerate with `cargo run -p whatspec-codegen`; never edit by hand.

";

/// Variant spellings the mechanical `pascal_case` gets wrong. Nothing in the
/// bundle says how to split these, and `Ib` reads as a typo for a reader who
/// does not know it is an initialism.
const TAG_RENAMES: &[(&str, &str)] = &[
    ("ib", "InfoBanner"),
    ("xmlstreamend", "XmlStreamEnd"),
    ("chatstate", "ChatState"),
    // `Error` alone collides with the associated `Error` type the `TryFrom`
    // half of `WireEnum` declares, and the resulting ambiguity is reported
    // against the derive rather than the variant.
    ("error", "ErrorStanza"),
];

/// The same, for notification types.
const KIND_RENAMES: &[(&str, &str)] = &[("mediaretry", "MediaRetry")];

/// Outgoing stanza types that never arrive under that tag.
///
/// Filtered out of the outgoing document's contribution alone, not out of the
/// merged set: the claim being made is only that *sending* a `privacy` stanza
/// is not evidence of receiving one. Should a later bundle list it in the
/// dispatcher table or among the server's requests, that is direct evidence it
/// arrives, and it must survive into the enum.
const OUTGOING_ONLY: &[&str] = &["privacy"];

pub fn generate(notif: &NotifIr, srvreq: &SrvReqIr, stanza: &StanzaIr) -> Result<String> {
    let mut out = super::header("stanza and notification vocabulary", &notif.wa_version);
    out.push_str(HEADER);

    let mut tags: BTreeSet<&str> = BTreeSet::new();
    tags.extend(notif.stanza_tags.iter().map(|t| t.tag.as_str()));
    tags.extend(srvreq.requests.iter().map(|r| r.tag.as_str()));
    tags.extend(
        stanza
            .stanzas
            .iter()
            .filter_map(|s| s.stanza_type.as_deref())
            .filter(|t| !OUTGOING_ONLY.contains(t)),
    );

    out.push_str(&wire_enum(
        "StanzaTag",
        "The tag a stanza arrives under.\n\
         ///\n\
         /// The union of WA Web's stanza dispatcher, the requests the server\n\
         /// sends us, and the types this client sends: no one document lists\n\
         /// them all.",
        &tags,
        TAG_RENAMES,
    )?);

    let kinds: BTreeSet<&str> = notif
        .notifications
        .iter()
        .map(|n| n.kind.as_str())
        .collect();
    out.push_str(&wire_enum(
        "NotificationType",
        "The `type` attribute of an incoming `<notification>`.\n\
         ///\n\
         /// Every value WA Web routes. This client handles a subset and\n\
         /// forwards the rest as a raw event, so a variant here is a value the\n\
         /// protocol carries, not a promise that anything acts on it.",
        &kinds,
        KIND_RENAMES,
    )?);

    Ok(out)
}

fn wire_enum(
    rust: &str,
    doc: &str,
    values: &BTreeSet<&str>,
    renames: &[(&str, &str)],
) -> Result<String> {
    ensure!(!values.is_empty(), "{rust} would have no variants");

    let mut out = format!(
        "/// {doc}\n#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, crate::WireEnum)]\npub enum {rust} {{\n"
    );
    let mut used: BTreeMap<String, &str> = BTreeMap::new();
    for value in values {
        let ident = renames
            .iter()
            .find(|(wire, _)| wire == value)
            .map(|(_, r)| (*r).to_string())
            .unwrap_or_else(|| pascal_case(value));
        if let Some(prev) = used.insert(ident.clone(), value) {
            anyhow::bail!("{rust} maps {prev:?} and {value:?} onto the same variant {ident}");
        }
        out.push_str(&format!(
            "    #[wire = {}]\n    {ident},\n",
            super::rust_str(value)
        ));
    }
    out.push_str("}\n\n");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{NotifKind, NotifStanzaTag, OutgoingStanza, SrvReq};

    fn ir(
        tags: &[&str],
        reqs: &[&str],
        outgoing: &[&str],
        kinds: &[&str],
    ) -> (NotifIr, SrvReqIr, StanzaIr) {
        (
            NotifIr {
                wa_version: "2.0.0".to_string(),
                stanza_tags: tags
                    .iter()
                    .map(|t| NotifStanzaTag {
                        tag: (*t).to_string(),
                    })
                    .collect(),
                notifications: kinds
                    .iter()
                    .map(|k| NotifKind {
                        kind: (*k).to_string(),
                    })
                    .collect(),
            },
            SrvReqIr {
                requests: reqs
                    .iter()
                    .map(|t| SrvReq {
                        tag: (*t).to_string(),
                    })
                    .collect(),
            },
            StanzaIr {
                stanzas: outgoing
                    .iter()
                    .map(|t| OutgoingStanza {
                        stanza_type: Some((*t).to_string()),
                    })
                    .collect(),
            },
        )
    }

    /// The reason three documents are read instead of one: the dispatcher table
    /// alone omits tags this repository has handlers for.
    #[test]
    fn the_tag_set_is_the_union_of_every_document() {
        let (n, s, o) = ir(&["message"], &["iq"], &["ack"], &["encrypt"]);
        let out = generate(&n, &s, &o).expect("emit");
        for wire in ["message", "iq", "ack"] {
            assert!(
                out.contains(&format!("#[wire = \"{wire}\"]")),
                "{wire}: {out}"
            );
        }
    }

    /// An outgoing-only type must not become a tag a handler could register
    /// for, since nothing ever arrives under it.
    #[test]
    fn an_outgoing_only_type_is_not_a_received_tag() {
        let (n, s, o) = ir(&["message"], &[], &["privacy"], &["encrypt"]);
        let out = generate(&n, &s, &o).expect("emit");
        assert!(!out.contains("#[wire = \"privacy\"]"), "{out}");
    }

    /// The exclusion is about the outgoing document, not about the tag. A
    /// bundle that starts dispatching `privacy` is saying it arrives, and
    /// dropping it then would omit a tag upstream just told us to expect.
    #[test]
    fn a_dispatched_tag_survives_even_when_it_is_also_sent() {
        for (dispatcher, requests) in [(vec!["privacy"], vec![]), (vec![], vec!["privacy"])] {
            let (n, s, o) = ir(&dispatcher, &requests, &["privacy"], &["encrypt"]);
            let out = generate(&n, &s, &o).expect("emit");
            assert!(out.contains("#[wire = \"privacy\"]"), "{out}");
        }
    }

    #[test]
    fn colons_and_initialisms_become_readable_variants() {
        let (n, s, o) = ir(&["stream:error", "ib"], &[], &[], &["w:gp2", "fb:update"]);
        let out = generate(&n, &s, &o).expect("emit");
        for ident in ["StreamError", "InfoBanner", "WGp2", "FbUpdate"] {
            assert!(out.contains(&format!("    {ident},")), "{ident}: {out}");
        }
    }

    /// Two wire values colliding on one identifier would silently drop a value,
    /// which is the failure a generated vocabulary exists to prevent.
    #[test]
    fn a_variant_collision_stops_the_generator() {
        let (n, s, o) = ir(&["stream:error", "stream_error"], &[], &[], &["encrypt"]);
        let err = generate(&n, &s, &o).expect_err("collision");
        assert!(err.to_string().contains("same variant"), "{err}");
    }
}

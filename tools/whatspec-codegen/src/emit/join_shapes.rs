//! `wacore/src/iq/join_shapes.rs`: the success shapes of RPCs whose answers
//! carry presence gates, taken from the whatspec IQ index rather than from a
//! reading of the bundle.
//!
//! A join is the call whose success the server may answer with nothing but the
//! result envelope: WA Web's own `AcceptGroupAddResponseSuccess` variant
//! requires no child at all, while `...GroupJoinRequestSuccess` requires a
//! `<membership_approval_request>` child. A parser that demands a child for
//! every answer turns an accepted join into a parse error, with no error to
//! see -- the join already happened. Passive-mode setters are the mirror
//! image: a single success variant that requires its child.
//!
//! What this emits is the *expectation*, not the parse itself. The specs keep
//! their hand-written response parsing; the generated constants give the tests
//! something to check the shapes against that upstream owns. When WhatsApp
//! changes which success shapes an RPC has, the next sync flips the constants
//! and the tests fail, instead of the change going unnoticed.

use anyhow::{Result, bail};

use crate::emit::{header, rust_str};
use crate::ir::IqIr;

/// One RPC whose success shapes are pinned, keyed the way the index is: the
/// response parser name, since neither the request module nor the exported
/// function is unique across shapes.
pub struct Wanted {
    /// Response parser name, as the index keys it.
    pub parser: &'static str,
    /// The generated constant's name. Named after our result type rather than
    /// after the upstream RPC, since the constant exists to be read next to the
    /// parser it checks.
    pub rust: &'static str,
}

/// The RPCs whose success shapes are pinned. To pin another one, add a row:
/// the next sync either emits its shapes or fails loudly on a missing parser.
pub const WANTED: &[Wanted] = &[
    Wanted {
        parser: "WASmaxGroupsAcceptGroupAddRPC",
        rust: "ACCEPT_GROUP_ADD_SUCCESS",
    },
    Wanted {
        parser: "WASmaxGroupsJoinLinkedGroupRPC",
        rust: "JOIN_LINKED_GROUP_SUCCESS",
    },
    Wanted {
        parser: "WASmaxPassiveModeActiveIQRPC",
        rust: "PASSIVE_ACTIVE_SUCCESS",
    },
    Wanted {
        parser: "WASmaxPassiveModePassiveIQRPC",
        rust: "PASSIVE_PASSIVE_SUCCESS",
    },
];

/// (variant tag, required child tags) for each success variant of one RPC, in
/// RPC cascade order. A variant with no required children accepts a bare
/// `<iq type="result">`.
fn shapes(iq: &IqIr, parser: &str) -> Result<Vec<(String, Vec<String>)>> {
    let stanza = iq
        .stanzas
        .iter()
        .find(|s| s.response.parser_name == parser)
        .ok_or_else(|| anyhow::anyhow!("join RPC ({parser}) missing from the IQ index"))?;
    let mut out = Vec::new();
    for v in &stanza.response.variants {
        if !v.is_success() {
            continue;
        }
        let mut children: Vec<String> = v
            .assertions
            .iter()
            .filter_map(|a| a.child_gate().map(str::to_string))
            .collect();
        children.sort();
        children.dedup();
        out.push((v.tag.clone(), children));
    }
    if out.is_empty() {
        bail!("join RPC ({parser}) has no success variant in the IQ index");
    }
    Ok(out)
}

pub fn generate(iq: &IqIr, wa_version: &str) -> Result<String> {
    let mut out = header("response presence-gate shapes", wa_version);
    out.push_str(
        "//! The success shapes of RPCs whose answers carry presence gates, from the\n//! whatspec IQ index.\n//!\n//! Regenerate with `cargo run -p whatspec-codegen`; never edit by hand. To pin\n//! another RPC, add it to `WANTED` in the codegen's join-shape emitter.\n\n",
    );
    for w in WANTED {
        let shapes = shapes(iq, w.parser)?;
        out.push_str(&format!(
            "/// (variant tag, required child tags) for each success variant of\n/// `{}`, in RPC cascade order. A variant with no required children\n/// accepts a bare `<iq type=\"result\">`.\n",
            w.parser
        ));
        out.push_str(&format!("pub const {}: &[(&str, &[&str])] = &[\n", w.rust));
        for (tag, children) in &shapes {
            let kids = children
                .iter()
                .map(|c| rust_str(c))
                .collect::<Vec<_>>()
                .join(", ");
            out.push_str(&format!("    ({}, &[{}]),\n", rust_str(tag), kids));
        }
        out.push_str("];\n\n");
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{ParsedResponse, ResponseAssertion, ResponseVariant};

    fn stanza(parser: &str, variants: Vec<(&str, &str, Vec<&str>)>) -> IqIr {
        IqIr {
            wa_version: "2.3000.1".into(),
            stanzas: vec![crate::ir::IqStanza {
                module_name: "m".into(),
                namespace: "w:g2".into(),
                iq_type: "set".into(),
                target: crate::ir::IqTarget::GroupJid,
                exported_function: "f".into(),
                response: ParsedResponse {
                    parser_name: parser.into(),
                    variants: variants
                        .into_iter()
                        .map(|(tag, kind, children)| ResponseVariant {
                            tag: tag.into(),
                            kind: Some(kind.into()),
                            assertions: children
                                .into_iter()
                                .map(|c| ResponseAssertion {
                                    kind: Some("child".into()),
                                    name: Some(c.into()),
                                })
                                .collect(),
                        })
                        .collect(),
                },
            }],
        }
    }

    #[test]
    fn success_shapes_keep_cascade_order_and_gates() {
        let mut iq = stanza(
            "WASmaxGroupsAcceptGroupAddRPC",
            vec![
                (
                    "AcceptGroupAddResponseGroupJoinRequestSuccess",
                    "success",
                    vec!["membership_approval_request"],
                ),
                ("AcceptGroupAddResponseSuccess", "success", vec![]),
                ("AcceptGroupAddResponseClientError", "error", vec![]),
            ],
        );
        // generate() pins every WANTED parser; the others carry a bare success.
        for w in WANTED {
            if w.parser == "WASmaxGroupsAcceptGroupAddRPC" {
                continue;
            }
            iq.stanzas.push(crate::ir::IqStanza {
                module_name: "m".into(),
                namespace: "w:g2".into(),
                iq_type: "set".into(),
                target: crate::ir::IqTarget::GroupJid,
                exported_function: "f".into(),
                response: ParsedResponse {
                    parser_name: w.parser.into(),
                    variants: vec![ResponseVariant {
                        tag: "SomeSuccess".into(),
                        kind: Some("success".into()),
                        assertions: vec![],
                    }],
                },
            });
        }
        let out = generate(&iq, "2.3000.1").expect("generated");
        assert!(
            out.contains(
                "(\"AcceptGroupAddResponseGroupJoinRequestSuccess\", &[\"membership_approval_request\"])"
            ),
            "{out}"
        );
        assert!(
            out.contains("(\"AcceptGroupAddResponseSuccess\", &[])"),
            "{out}"
        );
        assert!(!out.contains("ClientError"), "{out}");
    }

    #[test]
    fn missing_rpc_fails_the_sync() {
        let mut iq = stanza("WASmaxGroupsAcceptGroupAddRPC", vec![]);
        iq.stanzas.clear();
        assert!(generate(&iq, "2.3000.1").is_err());
    }

    #[test]
    fn every_wanted_rpc_emits_its_own_constant() {
        let iq = IqIr {
            wa_version: "2.3000.1".into(),
            stanzas: WANTED
                .iter()
                .map(|w| crate::ir::IqStanza {
                    module_name: "m".into(),
                    namespace: "w:g2".into(),
                    iq_type: "set".into(),
                    target: crate::ir::IqTarget::GroupJid,
                    exported_function: "f".into(),
                    response: ParsedResponse {
                        parser_name: w.parser.into(),
                        variants: vec![ResponseVariant {
                            tag: "SomeSuccess".into(),
                            kind: Some("success".into()),
                            assertions: vec![],
                        }],
                    },
                })
                .collect(),
        };
        let out = generate(&iq, "2.3000.1").expect("generated");
        for w in WANTED {
            assert!(
                out.contains(&format!("pub const {}: ", w.rust)),
                "missing constant for {}: {out}",
                w.parser
            );
        }
    }
}

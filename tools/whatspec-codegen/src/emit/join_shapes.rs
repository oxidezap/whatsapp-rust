//! `wacore/src/iq/join_shapes.rs`: the success shapes of the group-join RPCs,
//! taken from the whatspec IQ index rather than from a reading of the bundle.
//!
//! A join is the one call whose success the server may answer with nothing but
//! the result envelope: WA Web's own `AcceptGroupAddResponseSuccess` variant
//! requires no child at all, while `...GroupJoinRequestSuccess` requires a
//! `<membership_approval_request>` child. A parser that demands a child for
//! every answer turns an accepted join into a parse error, with no error to
//! see -- the join already happened.
//!
//! What this emits is the *expectation*, not the parse itself. The specs keep
//! their hand-written response parsing; the generated constants give the tests
//! something to check the shapes against that upstream owns. When WhatsApp
//! changes which success shapes a join RPC has, the next sync flips the
//! constants and the tests fail, instead of the change going unnoticed until a
//! join that succeeded reports an error.

use anyhow::{Result, bail};

use crate::emit::{header, rust_str};
use crate::ir::IqIr;

/// The join RPC whose success shapes are pinned (the response parser name, as
/// the index keys it).
const RPC_PARSER: &str = "WASmaxGroupsAcceptGroupAddRPC";

/// The generated constant's name. Named after our result type rather than
/// after the upstream RPC, since the constant exists to be read next to the
/// parser it checks.
const RUST: &str = "ACCEPT_GROUP_ADD_SUCCESS";

/// (variant tag, required child tags) for each success variant of the join
/// RPC, in RPC cascade order. A variant with no required children accepts a
/// bare `<iq type="result">`.
fn shapes(iq: &IqIr) -> Result<Vec<(String, Vec<String>)>> {
    let stanza = iq
        .stanzas
        .iter()
        .find(|s| s.response.parser_name == RPC_PARSER)
        .ok_or_else(|| anyhow::anyhow!("join RPC ({RPC_PARSER}) missing from the IQ index"))?;
    let mut out = Vec::new();
    for v in &stanza.response.variants {
        if v.kind.as_deref() != Some("success") {
            continue;
        }
        let mut children: Vec<String> = v
            .assertions
            .iter()
            .filter(|a| a.kind.as_deref() == Some("child"))
            .filter_map(|a| a.name.clone())
            .collect();
        children.sort();
        children.dedup();
        out.push((v.tag.clone(), children));
    }
    if out.is_empty() {
        bail!("join RPC ({RPC_PARSER}) has no success variant in the IQ index");
    }
    Ok(out)
}

pub fn generate(iq: &IqIr, wa_version: &str) -> Result<String> {
    let shapes = shapes(iq)?;
    let mut out = header("group-join success shapes", wa_version);
    out.push_str(
        "//! The success shapes of the group-join RPC, from the whatspec IQ index.\n//!\n//! Regenerate with `cargo run -p whatspec-codegen`; never edit by hand. To pin\n//! another join RPC, extend the codegen's join-shape emitter.\n\n",
    );
    out.push_str(&format!(
        "/// (variant tag, required child tags) for each success variant of\n/// `{RPC_PARSER}`, in RPC cascade order. A variant with no required children\n/// accepts a bare `<iq type=\"result\">`.\n"
    ));
    out.push_str(&format!("pub const {RUST}: &[(&str, &[&str])] = &[\n"));
    for (tag, children) in &shapes {
        let kids = children
            .iter()
            .map(|c| rust_str(c))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("    ({}, &[{}]),\n", rust_str(tag), kids));
    }
    out.push_str("];\n");
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{ParsedResponse, ResponseAssertion, ResponseVariant};

    fn stanza(variants: Vec<(&str, &str, Vec<&str>)>) -> IqIr {
        IqIr {
            wa_version: "2.3000.1".into(),
            stanzas: vec![crate::ir::IqStanza {
                module_name: "m".into(),
                namespace: "w:g2".into(),
                iq_type: "set".into(),
                target: crate::ir::IqTarget::GroupJid,
                exported_function: "f".into(),
                response: ParsedResponse {
                    parser_name: RPC_PARSER.into(),
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
        let iq = stanza(vec![
            (
                "AcceptGroupAddResponseGroupJoinRequestSuccess",
                "success",
                vec!["membership_approval_request"],
            ),
            ("AcceptGroupAddResponseSuccess", "success", vec![]),
            ("AcceptGroupAddResponseClientError", "error", vec![]),
        ]);
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
        let mut iq = stanza(vec![]);
        iq.stanzas.clear();
        assert!(generate(&iq, "2.3000.1").is_err());
    }
}

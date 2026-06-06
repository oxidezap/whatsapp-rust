//! Build-equivalence drift guard between the hand-written IQ specs and the
//! whatspec-generated wire-shape specs (`wacore::iq::generated`).
//!
//! The two are maintained from **independent sources**: the hand-written domain
//! specs are written by hand (and carry domain types/validation), while the
//! generated specs are extracted from the live WhatsApp Web bundle. When the two
//! agree on the request wire, that wire is corroborated; when WhatsApp Web changes
//! a request shape, a regenerated `generated.rs` will diverge from its hand-written
//! twin and the matching assertion below fails — flagging exactly which domain spec
//! needs updating.
//!
//! Coverage is the **intersection**: only ops that exist in *both* the hand-written
//! tree and the smax-extracted generated tree are paired. Many hand-written ops
//! (legacy/non-smax request builders — e.g. `urn:xmpp:whatsapp:dirty`, `status`,
//! `md` `remove-companion-device`) have no generated counterpart and cannot be
//! guarded here. Add a pair whenever a generated spec gains a hand-written twin.

use wacore::iq::spec::IqSpec;

/// Assert two specs build byte-equivalent request wire: same namespace, type,
/// routing (`to`/`target`) and content node tree. `id`/`timeout` are runtime
/// concerns (the `id` is assigned at send time) and are intentionally ignored.
fn assert_build_eq<A: IqSpec, B: IqSpec>(label: &str, hand: &A, generated: &B) {
    let a = hand.build_iq();
    let b = generated.build_iq();
    assert_eq!(a.namespace, b.namespace, "{label}: namespace");
    assert_eq!(a.query_type, b.query_type, "{label}: query_type");
    assert_eq!(a.to, b.to, "{label}: to");
    assert_eq!(a.target, b.target, "{label}: target");
    assert_eq!(a.content, b.content, "{label}: content");
}

#[test]
fn passive_active_matches_generated() {
    use wacore::iq::generated::passive::MakeActiveIQRequestSpec;
    use wacore::iq::passive::PassiveModeSpec;
    assert_build_eq(
        "passive:active",
        &PassiveModeSpec::active(),
        &MakeActiveIQRequestSpec,
    );
}

#[test]
fn passive_passive_matches_generated() {
    use wacore::iq::generated::passive::MakePassiveIQRequestSpec;
    use wacore::iq::passive::PassiveModeSpec;
    assert_build_eq(
        "passive:passive",
        &PassiveModeSpec::passive(),
        &MakePassiveIQRequestSpec,
    );
}

#[test]
fn keepalive_ping_matches_generated() {
    use wacore::iq::generated::w_p::MakeClientRequestSpec;
    use wacore::iq::keepalive::KeepaliveSpec;
    // Keepalive's only divergence from the generated ping is an optional runtime
    // timeout, which `assert_build_eq` ignores.
    assert_build_eq("w:p:ping", &KeepaliveSpec::new(), &MakeClientRequestSpec);
}

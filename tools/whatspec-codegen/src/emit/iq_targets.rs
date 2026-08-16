//! `wacore/src/iq/targets.rs`: who each request this repository sends is
//! addressed to, taken from the whatspec IQ index rather than from a reading of
//! the bundle.
//!
//! Addressing is worth deriving because getting it wrong is invisible. A `w:g2`
//! request sent to `g.us` when the server expects the group JID is a
//! well-formed stanza that is simply never answered, so the only symptom is a
//! caller that waits out its timeout -- there is no error to see and no nack to
//! log. Nothing in the shape of the stanza says which of the two it should be.
//!
//! What this emits is the *expectation*, not the address itself. The specs keep
//! building their own `to`; the generated constants give the tests something to
//! check them against that upstream owns. When WhatsApp moves a request from one
//! target to the other, the next sync flips the constant and the test fails,
//! instead of the change going unnoticed until someone reports a hang.

use std::collections::BTreeSet;

use anyhow::{Result, bail, ensure};

use crate::ir::{IqIr, IqStanza, IqTarget};

const HEADER: &str = "\
//! How the official client addresses the requests this repository sends.
//!
//! Regenerate with `cargo run -p whatspec-codegen`; never edit by hand. To pin
//! another request, add it to `WANTED` in the codegen's IQ target emitter.

/// Who a request is addressed to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IqTarget {
    /// The main server, `s.whatsapp.net`.
    MainServer,
    /// The group server, `g.us`: it answers about groups in general rather than
    /// about one group.
    GroupServer,
    /// The one group the request acts on.
    GroupJid,
}

";

/// One IQ builder this repository pins, keyed the way the index is: module plus
/// the exported function, because neither alone is unique.
pub struct Wanted {
    pub module: &'static str,
    pub function: &'static str,
    /// The generated constant's name. Named after our spec rather than after
    /// the upstream function, since the constant exists to be read next to the
    /// spec it checks.
    pub rust: &'static str,
    /// What our spec is called, for the generated doc line. A reader landing on
    /// the constant should not have to guess which spec it governs.
    pub spec: &'static str,
}

/// The requests whose addressing is pinned.
///
/// `w:g2` only, and deliberately: it is the only namespace where the
/// distinction exists. Outside it every entry in the index resolves to
/// `s.whatsapp.net`, except four `newsletter` builders whatspec cannot resolve
/// at all -- so a constant elsewhere would either restate what the namespace
/// already guarantees or pin a target nobody read.
pub const WANTED: &[Wanted] = &[
    Wanted {
        module: "WAWebGroupExitJob",
        function: "leaveGroup",
        rust: "LEAVE_GROUP",
        spec: "LeaveGroupIq",
    },
    Wanted {
        module: "WASmaxOutGroupsGetParticipatingGroupsRequest",
        function: "makeGetParticipatingGroupsRequestParticipatingParticipants",
        rust: "GROUP_PARTICIPATING",
        spec: "GroupParticipatingIq",
    },
    Wanted {
        module: "WASmaxOutGroupsBatchGetGroupInfoRequest",
        function: "makeBatchGetGroupInfoRequestQueryGroup",
        rust: "BATCH_GET_GROUP_INFO",
        spec: "BatchGetGroupInfoIq",
    },
    Wanted {
        module: "WASmaxOutGroupsGetInviteGroupInfoRequest",
        function: "makeGetInviteGroupInfoRequest",
        rust: "GET_GROUP_INVITE_INFO",
        spec: "GetGroupInviteInfoIq",
    },
    Wanted {
        module: "WASmaxOutGroupsGetGroupInfoRequest",
        function: "makeGetGroupInfoRequestQueryAddRequest",
        rust: "GROUP_QUERY",
        spec: "GroupQueryIq",
    },
    Wanted {
        module: "WASmaxOutGroupsSetSubjectRequest",
        function: "makeSetSubjectRequest",
        rust: "SET_GROUP_SUBJECT",
        spec: "SetGroupSubjectIq",
    },
    Wanted {
        module: "WASmaxOutGroupsSetDescriptionRequest",
        function: "makeSetDescriptionRequestDescriptionBody",
        rust: "SET_GROUP_DESCRIPTION",
        spec: "SetGroupDescriptionIq",
    },
    Wanted {
        module: "WASmaxOutGroupsSetPropertyRequest",
        function: "makeSetPropertyRequestLocked",
        rust: "SET_GROUP_LOCKED",
        spec: "SetGroupLockedIq",
    },
    Wanted {
        module: "WASmaxOutGroupsReportMessagesRequest",
        function: "makeReportMessagesRequest",
        rust: "REPORT_GROUP_MESSAGES",
        spec: "ReportGroupMessagesIq",
    },
    Wanted {
        module: "WASmaxOutGroupsGetReportedMessagesRequest",
        function: "makeGetReportedMessagesRequest",
        rust: "GET_REPORTED_GROUP_MESSAGES",
        spec: "GetReportedGroupMessagesIq",
    },
    Wanted {
        module: "WASmaxOutGroupsGetMembershipApprovalRequestsRequest",
        function: "makeGetMembershipApprovalRequestsRequest",
        rust: "GET_MEMBERSHIP_REQUESTS",
        spec: "GetMembershipRequestsIq",
    },
    Wanted {
        module: "WASmaxOutGroupsMembershipRequestsActionRequest",
        function: "makeMembershipRequestsActionRequestMembershipRequestsActionApproveParticipant",
        rust: "MEMBERSHIP_REQUEST_ACTION",
        spec: "MembershipRequestActionIq",
    },
];

pub fn generate(ir: &IqIr) -> Result<String> {
    let mut out = super::header("IQ addressing", &ir.wa_version);
    out.push_str(HEADER);

    let mut used = BTreeSet::new();
    for wanted in WANTED {
        ensure!(
            used.insert(wanted.rust),
            "two WANTED entries both emit {}",
            wanted.rust
        );
        let stanza = lookup(ir, wanted)?;
        let target = match stanza.target {
            IqTarget::MainServer => "MainServer",
            IqTarget::GroupServer => "GroupServer",
            IqTarget::GroupJid => "GroupJid",
            // Pinning an unresolved target would assert that we address the
            // request the way whatspec failed to read, which is not a claim
            // anything here can make.
            IqTarget::Unknown => bail!(
                "{}::{} resolves to no target, so its addressing cannot be pinned",
                wanted.module,
                wanted.function
            ),
        };
        out.push_str(&format!(
            "/// Addressing of `{}`, from `{}` in `{}` (`{}` in `{}`).\npub const {}: IqTarget = IqTarget::{target};\n\n",
            wanted.spec,
            wanted.function,
            stanza.module_name,
            stanza.iq_type,
            stanza.namespace,
            wanted.rust,
        ));
    }
    Ok(out)
}

/// The one entry matching `(module, function)`.
///
/// Ambiguity is fatal rather than first-wins, and here that is not a
/// hypothetical: `resetGroupInviteCode` appears twice in `WAWebGroupInviteJob`,
/// once resolving to `group_jid` and once to `g.us`. They are different calls
/// that share a name, so picking either by position would pin a coin flip.
fn lookup<'a>(ir: &'a IqIr, wanted: &Wanted) -> Result<&'a IqStanza> {
    let matches: Vec<&IqStanza> = ir
        .stanzas
        .iter()
        .filter(|s| s.module_name == wanted.module && s.exported_function == wanted.function)
        .collect();
    match matches.as_slice() {
        [one] => Ok(one),
        [] => bail!(
            "the IQ index has no {}::{}; it was renamed or dropped upstream",
            wanted.module,
            wanted.function
        ),
        many => bail!(
            "the IQ index has {} entries for {}::{}, which resolve to {:?}; \
             a name shared by several calls cannot pin one of them",
            many.len(),
            wanted.module,
            wanted.function,
            many.iter().map(|s| s.target).collect::<Vec<_>>(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stanza(module: &str, function: &str, target: IqTarget) -> IqStanza {
        IqStanza {
            module_name: module.to_string(),
            namespace: "w:g2".to_string(),
            iq_type: "get".to_string(),
            target,
            exported_function: function.to_string(),
        }
    }

    /// An index covering every `WANTED` entry, since `generate` resolves the
    /// whole table. Each case below then perturbs the one entry it is about, so
    /// the failure it asserts is the one it caused.
    fn fixture() -> IqIr {
        IqIr {
            wa_version: "2.0.0".to_string(),
            stanzas: WANTED
                .iter()
                .map(|w| stanza(w.module, w.function, IqTarget::GroupJid))
                .collect(),
        }
    }

    /// `leaveGroup` leads `WANTED`, so a bail on it stops the run before any
    /// later entry is reached.
    const FIRST: &str = "leaveGroup";

    fn first_entry() -> &'static Wanted {
        WANTED
            .iter()
            .find(|w| w.function == FIRST)
            .expect("WANTED leads with the entry these tests perturb")
    }

    #[test]
    fn the_target_comes_from_the_index_not_the_table() {
        let entry = first_entry();
        for (target, rendered) in [
            (IqTarget::GroupServer, "IqTarget::GroupServer"),
            (IqTarget::GroupJid, "IqTarget::GroupJid"),
        ] {
            let mut ir = fixture();
            ir.stanzas[0] = stanza(entry.module, entry.function, target);
            let out = generate(&ir).expect("emit");
            assert!(
                out.contains(&format!("pub const LEAVE_GROUP: IqTarget = {rendered};")),
                "{out}"
            );
        }
    }

    /// The failure this exists for: two calls sharing a name, resolving
    /// differently. Binding either would pin whichever the index happened to
    /// list first.
    #[test]
    fn a_name_shared_by_two_targets_stops_the_generator() {
        let entry = first_entry();
        let mut ir = fixture();
        ir.stanzas
            .push(stanza(entry.module, entry.function, IqTarget::GroupServer));
        let err = generate(&ir).expect_err("ambiguous");
        assert!(err.to_string().contains("cannot pin one of them"), "{err}");
    }

    #[test]
    fn an_unresolved_target_is_refused() {
        let entry = first_entry();
        let mut ir = fixture();
        ir.stanzas[0] = stanza(entry.module, entry.function, IqTarget::Unknown);
        let err = generate(&ir).expect_err("unknown target");
        assert!(err.to_string().contains("resolves to no target"), "{err}");
    }

    #[test]
    fn a_dropped_entry_is_refused() {
        let mut ir = fixture();
        ir.stanzas.remove(0);
        let err = generate(&ir).expect_err("the entry is gone");
        assert!(
            err.to_string()
                .contains("has no WAWebGroupExitJob::leaveGroup"),
            "{err}"
        );
    }
}

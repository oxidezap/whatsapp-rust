//! Auto-generated private IQ pilot builders and result payloads (WhatsApp 2.3000.1047483476). DO NOT EDIT.
//!
//! Envelope correlation and error conversion stay in the request layer.

use crate::request::InfoQuery;
use wacore_binary::{Jid, NodeContent, NodeRef, builder::NodeBuilder};

pub(super) fn build_set_subject(iq_to: &Jid, subject_element_value: &str) -> InfoQuery<'static> {
    InfoQuery::set_ref(
        "w:g2",
        iq_to,
        Some(NodeContent::Nodes(vec![
            NodeBuilder::new("subject")
                .string_content(subject_element_value)
                .build(),
        ])),
    )
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum SetSubjectSuccess {
    Success,
}

pub(super) fn parse_set_subject_payload(_response: &NodeRef<'_>) -> SetSubjectSuccess {
    SetSubjectSuccess::Success
}

pub(super) fn build_accept_group_add(
    iq_to: &Jid,
    accept_code: &str,
    accept_expiration: i64,
    accept_admin: &Jid,
) -> InfoQuery<'static> {
    InfoQuery::set_ref(
        "w:g2",
        iq_to,
        Some(NodeContent::Nodes(vec![
            NodeBuilder::new("accept")
                .attr("code", accept_code)
                .attr("expiration", accept_expiration)
                .attr("admin", accept_admin)
                .build(),
        ])),
    )
}

#[derive(Debug, PartialEq, Eq)]
pub(super) enum AcceptGroupAddSuccess {
    GroupJoinRequestSuccess,
    Success,
}

pub(super) fn parse_accept_group_add_payload(response: &NodeRef<'_>) -> AcceptGroupAddSuccess {
    if response
        .get_children_by_tag("membership_approval_request")
        .take(2)
        .count()
        == 1
    {
        return AcceptGroupAddSuccess::GroupJoinRequestSuccess;
    }
    AcceptGroupAddSuccess::Success
}

//! Auto-generated private IQ pilot builders and result payloads (WhatsApp 2.3000.1047483476). DO NOT EDIT.
//!
//! Envelope correlation and error conversion stay in the request layer.

use crate::request::InfoQuery;
use wacore_binary::{Jid, NodeContent, NodeRef, builder::NodeBuilder};

pub(super) struct SetSubjectRequest<'a> {
    pub(super) iq_to: &'a Jid,
    pub(super) subject_element_value: &'a str,
}

pub(super) fn build_set_subject(request: SetSubjectRequest<'_>) -> InfoQuery<'static> {
    let SetSubjectRequest {
        iq_to,
        subject_element_value,
    } = request;
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

pub(super) struct AcceptGroupAddRequest<'a> {
    pub(super) iq_to: &'a Jid,
    pub(super) accept_code: &'a str,
    pub(super) accept_expiration: i64,
    pub(super) accept_admin: &'a Jid,
}

pub(super) fn build_accept_group_add(request: AcceptGroupAddRequest<'_>) -> InfoQuery<'static> {
    let AcceptGroupAddRequest {
        iq_to,
        accept_code,
        accept_expiration,
        accept_admin,
    } = request;
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

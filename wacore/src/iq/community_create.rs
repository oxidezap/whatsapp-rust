//! Auto-generated community Create request projection (WhatsApp 2.3000.1047483476). DO NOT EDIT.
//!
use crate::request::InfoQuery;
use wacore_binary::{Jid, NodeContent, builder::NodeBuilder, jid::Server};

pub(super) struct CommunityCreateRequest<'a> {
    pub(super) any_subject: &'a str,
    pub(super) has_parent_group_default_membership_approval_mode: Option<()>,
    pub(super) has_allow_non_admin_sub_group_creation: bool,
    pub(super) has_create_general_chat: bool,
}

pub(super) fn build_community_create(request: CommunityCreateRequest<'_>) -> InfoQuery<'static> {
    let mut children = Vec::new();
    let mut parent = NodeBuilder::new("parent");
    if request
        .has_parent_group_default_membership_approval_mode
        .is_some()
    {
        parent = parent.attr("default_membership_approval_mode", "request_required");
    }
    children.push(parent.build());
    if request.has_allow_non_admin_sub_group_creation {
        children.push(NodeBuilder::new("allow_non_admin_sub_group_creation").build());
    }
    if request.has_create_general_chat {
        children.push(NodeBuilder::new("create_general_chat").build());
    }
    InfoQuery::set(
        "w:g2",
        Jid::new("", Server::Group),
        Some(NodeContent::Nodes(vec![
            NodeBuilder::new("create")
                .attr("subject", request.any_subject)
                .children(children)
                .build(),
        ])),
    )
}

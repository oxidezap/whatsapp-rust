//! Linked native/WASM probe using the same canonical API on base and head.

use std::hint::black_box;
use whatsapp_rust::wacore::types::call::CallAction;
use whatsapp_rust::{
    GroupHierarchy, GroupLookupResult, GroupMetadata, GroupMetadataResult, GroupOverview,
    GroupOverviewResult, Jid, SubgroupKind, anyhow, serde_json,
};

fn main() -> anyhow::Result<()> {
    let parent: Jid = black_box("120363000000000011@g.us").parse()?;
    let metadata = GroupMetadata {
        parent_group_jid: Some(parent.clone()),
        is_general_chat: true,
        ..Default::default()
    };
    match black_box(&metadata).hierarchy() {
        GroupHierarchy::Subgroup {
            parent,
            kind: SubgroupKind::General,
        } => {
            black_box(parent);
        }
        other => {
            black_box(other);
        }
    }
    let full: GroupMetadataResult = GroupLookupResult::Found(Box::new(metadata));
    let overview: GroupOverviewResult = full.map(|metadata| GroupOverview::from(metadata.as_ref()));
    black_box(overview);
    let action = CallAction::OfferNotice {
        call_id: black_box("CALL-ID").to_owned(),
        call_creator: parent,
        is_video: false,
        is_group: true,
    };
    black_box(action.wire_tag());
    black_box(serde_json::to_vec(&action)?);
    Ok(())
}

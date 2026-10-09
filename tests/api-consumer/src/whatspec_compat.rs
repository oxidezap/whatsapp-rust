//! Public contracts used before the 2.3000.1047483476 capture.
use wacore::iq::{abprops, mex_operations as mex};

pub fn historical_operations() -> [&'static str; 9] {
    [
        mex::create_labyrinth_backup::DOC_ID,
        mex::debug_labyrinth_inbox_snapshot::DOC_ID,
        mex::debug_labyrinth_range::DOC_ID,
        mex::eb_message_metadata_query::DOC_ID,
        mex::rotate_labyrinth_epoch::DOC_ID,
        mex::team_link_create_invitation::DOC_ID,
        mex::team_link_list_invitations::DOC_ID,
        mex::team_link_remove_invitation::DOC_ID,
        mex::upload_labyrinth_messages::DOC_ID,
    ]
}

pub fn historical_props() -> [abprops::AbProp; 6] {
    [
        abprops::web::AI_3P_AGENT_LINK_ENABLED,
        abprops::web::LISTS_SMB_WEB_ENABLED,
        abprops::web::SCHEDULED_COMPANION_CONTACT_REFRESH_DAYS,
        abprops::web::SCHEDULED_COMPANION_CONTACT_REFRESH_HOURS,
        abprops::web::SMOOTHIE_PERFORMANCE_MSG_SEND,
        abprops::web::UPDATED_HARMFUL_DOCUMENT_DIALOG,
    ]
}

#[test]
fn customer_profile_candidates_still_serialize_as_an_array() {
    let variables = mex::contact_manager_customer_profiles::Variables {
        input: Some(mex::contact_manager_customer_profiles::Input {
            candidate_lids: Some(vec!["123".into(), "456".into()]),
            ..Default::default()
        }),
    };
    assert_eq!(
        serde_json::to_value(variables).unwrap(),
        serde_json::json!({"input": {"candidate_lids": ["123", "456"]}})
    );
}

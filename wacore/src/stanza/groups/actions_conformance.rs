//! Auto-generated complete official nonraw group action conformance (WhatsApp 2.3000.1047483476). DO NOT EDIT.
//!
//! Original callback source: https://github.com/oxidezap/whatspec/blob/ae4ea6e30816f307319522d4bac8fc448185f485/tests/conformance/groups/actions.cjs
//! Module SHA-256 a65d861ba0dacdcec52d2df48712a80677ecf10021d7df63c84183f195db13a1.
//! Account facts/Wid conversion/logging are explicit test mocks; runtime handlers are not run.

mod projection {
    //! Test-only complete JSON projection for the nonraw official-action oracle.
    //! No Serialize contract or JSON interpreter is added to the runtime API.
    #![cfg(test)]
    use crate::stanza::groups::actions_normalized::NormalizedGroupAction as A;
    use crate::stanza::groups::create_normalized::CreateParticipant;
    use serde_json::{Map, Value, json};

    fn maybe(map: &mut Map<String, Value>, name: &str, value: Option<Value>) {
        if let Some(value) = value {
            map.insert(name.into(), value);
        }
    }
    fn string<T: ToString>(value: &T) -> Value {
        Value::String(value.to_string())
    }
    fn participant(p: &CreateParticipant, history: bool) -> Value {
        let mut value = json!({
            "displayName":p.display_name,"id":p.id.to_string(),"isSuperAdmin":p.is_super_admin,"isAdmin":p.is_admin,
            "lid":p.lid.as_ref().map(ToString::to_string),"phoneNumber":p.phone_number.as_ref().map(ToString::to_string),"username":p.username,
        });
        if history {
            value["joinTime"] = json!(p.join_time);
            value["groupHistorySentState"] = json!(p.history_sent_state_code());
        }
        value
    }
    fn participants(values: &[CreateParticipant], history: bool) -> Value {
        Value::Array(values.iter().map(|p| participant(p, history)).collect())
    }

    /// Every key in these source objects participates in exact Value equality.
    /// Complex action families have their own separate full-payload oracles and are
    /// refused here; a new variant cannot silently become an empty object.
    pub(super) fn project(action: &A, history: bool) -> Result<Value, String> {
        let kind = action
            .action_type()
            .ok_or("unexpected unqualified action in nonraw oracle")?;
        let mut value = Map::from_iter([("actionType".into(), json!(kind.as_str()))]);
        match action {
            A::Add {
                participants: ps,
                reason,
                is_lid_addressing_mode,
            } => {
                value.insert("participants".into(), participants(ps, history));
                value.insert("reason".into(), json!(reason.map(|r| r.as_str())));
                value.insert("isLidAddressingMode".into(), json!(is_lid_addressing_mode));
            }
            A::Remove {
                participants: ps,
                reason,
                is_lid_addressing_mode,
            } => {
                value.insert("participants".into(), participants(ps, history));
                value.insert("reason".into(), json!(reason.map(|r| r.as_str())));
                value.insert("isLidAddressingMode".into(), json!(is_lid_addressing_mode));
            }
            A::Delete {
                reason,
                group_datas,
            } => {
                value.insert("reason".into(), json!(reason.map(|r| r.as_str())));
                value.insert(
                    "groupDatas".into(),
                    Value::Array(
                        group_datas
                            .iter()
                            .map(|g| json!({"id":g.id.to_string(),"subject":g.subject}))
                            .collect(),
                    ),
                );
            }
            A::Promote { participants: ps }
            | A::Demote { participants: ps }
            | A::Modify { participants: ps } => {
                value.insert("participants".into(), participants(ps, history));
            }
            A::LinkedGroupPromote {
                participants: ps,
                jid,
            }
            | A::LinkedGroupDemote {
                participants: ps,
                jid,
            } => {
                value.insert("participants".into(), participants(ps, history));
                maybe(&mut value, "jid", jid.as_ref().map(string));
            }
            A::Subject {
                subject,
                owner,
                owner_pn,
                owner_username,
                subject_time,
            } => {
                value.insert("subject".into(), json!(subject));
                value.insert("s_o".into(), json!(owner.as_ref().map(ToString::to_string)));
                value.insert(
                    "subjectOwnerPn".into(),
                    json!(owner_pn.as_ref().map(ToString::to_string)),
                );
                value.insert("subjectOwnerUsername".into(), json!(owner_username));
                value.insert("s_t".into(), json!(subject_time));
            }
            A::DescriptionAdded {
                id,
                description,
                timestamp,
            } => {
                value.insert("descId".into(), json!(id));
                value.insert("desc".into(), json!(description));
                value.insert("descTime".into(), json!(timestamp));
            }
            A::DescriptionRemoved { id } => {
                value.insert("descId".into(), json!(id));
            }
            A::Suspend { value: enabled }
            | A::Announce { value: enabled }
            | A::NoFrequentlyForwarded { value: enabled }
            | A::AllowNonAdminSubgroupCreation { value: enabled }
            | A::GroupSafetyCheck { value: enabled } => {
                value.insert("value".into(), json!(enabled));
            }
            A::Restrict {
                value: enabled,
                threshold,
            } => {
                value.insert("value".into(), json!(enabled));
                maybe(&mut value, "threshold", threshold.as_ref().map(string));
            }
            A::Invite { code } => {
                value.insert("code".into(), json!(code));
            }
            A::Ephemeral {
                duration,
                trigger,
                initiated_by_me,
            } => {
                value.insert("duration".into(), json!(duration));
                maybe(&mut value, "trigger", trigger.map(|v| json!(v)));
                maybe(
                    &mut value,
                    "initiatedByMe",
                    initiated_by_me.map(|v| json!(v)),
                );
            }
            A::RevokeInvite { participants: ps } => {
                value.insert(
                    "participants".into(),
                    Value::Array(
                        ps.iter()
                            .map(|p| json!({"id":p.id.to_string(),"expiration":p.expiration}))
                            .collect(),
                    ),
                );
            }
            A::GrowthLocked {
                expiration,
                lock_type,
            } => {
                value.insert("expiration".into(), json!(expiration));
                value.insert("type".into(), json!(lock_type));
            }
            A::GrowthUnlocked | A::GeneralChatAutoAddDisabled => {}
            A::MembershipApprovalMode {
                value: enabled,
                triggered,
            } => {
                value.insert("value".into(), json!(enabled));
                maybe(&mut value, "triggered", triggered.as_ref().map(string));
            }
            A::MembershipApprovalRequest {
                request_method,
                parent_group_id,
            } => {
                value.insert("requestMethod".into(), json!(request_method.as_str()));
                maybe(
                    &mut value,
                    "parentGroupId",
                    parent_group_id.as_ref().map(string),
                );
            }
            A::AllowAdminReports {
                value: enabled,
                should_skip_generation,
                author,
                triggered,
            } => {
                value.insert("value".into(), json!(enabled));
                value.insert("shouldSkipGenMsg".into(), json!(should_skip_generation));
                value.insert(
                    "author".into(),
                    json!(author.as_ref().map(ToString::to_string)),
                );
                maybe(&mut value, "triggered", triggered.as_ref().map(string));
            }
            A::AdminReportReceived {
                should_skip_generation,
                timestamp,
            } => {
                value.insert("shouldSkipGenMsg".into(), json!(should_skip_generation));
                value.insert("value".into(), json!(timestamp));
            }
            A::CreatedMembershipRequests {
                request_method,
                parent_group_id,
                requests,
                skip_generation,
            } => {
                value.insert("requestMethod".into(), json!(request_method.as_str()));
                maybe(
                    &mut value,
                    "parentGroupId",
                    parent_group_id.as_ref().map(string),
                );
                value.insert("skipGenMsg".into(), json!(skip_generation));
                value.insert(
                    "requests".into(),
                    Value::Array(
                        requests
                            .iter()
                            .map(|p| {
                                let mut pvalue =
                                    Map::from_iter([("wid".into(), json!(p.wid.to_string()))]);
                                maybe(&mut pvalue, "username", p.username.as_ref().map(string));
                                maybe(
                                    &mut pvalue,
                                    "phoneNumber",
                                    p.phone_number.as_ref().map(string),
                                );
                                Value::Object(pvalue)
                            })
                            .collect(),
                    ),
                );
            }
            A::RevokedMembershipRequests { requests } => {
                value.insert(
                    "requests".into(),
                    json!(requests.iter().map(ToString::to_string).collect::<Vec<_>>()),
                );
            }
            A::MemberAddMode { mode } => {
                maybe(&mut value, "memberAddMode", mode.map(|v| json!(v.as_str())));
            }
            A::Create { .. }
            | A::Link { .. }
            | A::Unlink { .. }
            | A::CreatedSubgroupSuggestion { .. }
            | A::RevokedSubgroupSuggestions { .. }
            | A::SubgroupSuggestionsOwnerChanged { .. } => {
                return Err(format!(
                    "complex action {} belongs to a separate payload oracle",
                    kind.as_str()
                ));
            }
        }
        Ok(Value::Object(value))
    }
}
use crate::stanza::groups::actions_normalized::GroupActionParseError;
use crate::stanza::groups::create_normalized::CreateParseError;
use crate::stanza::groups::normalized::{
    GroupParseContext, GroupParseError, parse_normalized_group_notification,
};
use crate::stanza::groups::suggestions_normalized::SuggestionParseError;
use serde_json::Value;
use wacore_binary::{Jid, builder::NodeBuilder};
fn is_me(jid: &Jid) -> bool {
    jid.to_string() == "200002@s.whatsapp.net"
}
fn country(value: &str) -> Option<String> {
    matches!(value, "US" | "BR").then(|| value.to_string())
}
fn failed_field(error: &GroupParseError) -> Option<&'static str> {
    match error {
        GroupParseError::Field(field) => Some(field),
        GroupParseError::Action(error) => match error.as_ref() {
            GroupActionParseError::Parse(GroupParseError::Field(field))
            | GroupActionParseError::Create(CreateParseError::Field(field))
            | GroupActionParseError::Suggestion(SuggestionParseError::Field(field)) => Some(field),
            _ => None,
        },
        _ => None,
    }
}

#[test]
fn complete_nonraw_actions_match_the_original_callback() {
    let me_lid = Jid::lid_device("100001", 2);
    let me_pn = Jid::pn_device("200002", 2);
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("addressing_mode", "lid")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("add")
                    .attr("reason", "default_sub_group_admin_add")
                    .children(vec![
                        NodeBuilder::new("participant")
                            .attr("jid", "100001@lid")
                            .attr("type", "superadmin")
                            .attr("username", "ungated")
                            .build(),
                    ])
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual = actual
            .unwrap_or_else(|error| panic!("{}: {error:?}", "add-reason-normalizes-admin-key"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected:Value=serde_json::from_str("[{\"actionType\":\"add\",\"isLidAddressingMode\":true,\"participants\":[{\"displayName\":null,\"id\":\"100001@lid\",\"isAdmin\":true,\"isSuperAdmin\":true,\"lid\":null,\"phoneNumber\":null,\"username\":\"ungated\"}],\"reason\":\"default_subgroup_admin_add\"}]").unwrap();
        assert_eq!(actual, expected, "{}", "add-reason-normalizes-admin-key");
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("add")
                    .attr("reason", "future")
                    .children(vec![])
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual = actual
            .unwrap_or_else(|error| panic!("{}: {error:?}", "add-ordinary-unknown-reason-is-null"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected:Value=serde_json::from_str("[{\"actionType\":\"add\",\"isLidAddressingMode\":false,\"participants\":[],\"reason\":null}]").unwrap();
        assert_eq!(
            actual, expected,
            "{}",
            "add-ordinary-unknown-reason-is-null"
        );
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("remove")
                    .attr("reason", "default_sub_group_demote")
                    .children(vec![])
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual =
            actual.unwrap_or_else(|error| panic!("{}: {error:?}", "remove-reason-is-closed"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected:Value=serde_json::from_str("[{\"actionType\":\"remove\",\"isLidAddressingMode\":false,\"participants\":[],\"reason\":\"default_sub_group_demote\"}]").unwrap();
        assert_eq!(actual, expected, "{}", "remove-reason-is-closed");
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("remove")
                    .attr("reason", "future")
                    .children(vec![])
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual =
            actual.unwrap_or_else(|error| panic!("{}: {error:?}", "remove-unknown-reason-is-null"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected:Value=serde_json::from_str("[{\"actionType\":\"remove\",\"isLidAddressingMode\":false,\"participants\":[],\"reason\":null}]").unwrap();
        assert_eq!(actual, expected, "{}", "remove-unknown-reason-is-null");
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("delete")
                    .attr("reason", "integrity_delete_parent")
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual =
            actual.unwrap_or_else(|error| panic!("{}: {error:?}", "delete-root-group-data"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected:Value=serde_json::from_str("[{\"actionType\":\"delete\",\"groupDatas\":[{\"id\":\"123456@g.us\",\"subject\":\"\"}],\"reason\":\"integrity_delete_parent\"}]").unwrap();
        assert_eq!(actual, expected, "{}", "delete-root-group-data");
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("delete").attr("reason", "future").build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual =
            actual.unwrap_or_else(|error| panic!("{}: {error:?}", "delete-unknown-reason-is-null"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected:Value=serde_json::from_str("[{\"actionType\":\"delete\",\"groupDatas\":[{\"id\":\"123456@g.us\",\"subject\":\"\"}],\"reason\":null}]").unwrap();
        assert_eq!(actual, expected, "{}", "delete-unknown-reason-is-null");
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("promote")
                    .children(vec![
                        NodeBuilder::new("participant")
                            .attr("jid", "200003@s.whatsapp.net")
                            .build(),
                    ])
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual = actual.unwrap_or_else(|error| panic!("{}: {error:?}", "promote"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected:Value=serde_json::from_str("[{\"actionType\":\"promote\",\"participants\":[{\"displayName\":null,\"id\":\"200003@s.whatsapp.net\",\"isAdmin\":false,\"isSuperAdmin\":false,\"lid\":null,\"phoneNumber\":null,\"username\":null}]}]").unwrap();
        assert_eq!(actual, expected, "{}", "promote");
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![NodeBuilder::new("demote").children(vec![]).build()])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual = actual.unwrap_or_else(|error| panic!("{}: {error:?}", "demote"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected: Value =
            serde_json::from_str("[{\"actionType\":\"demote\",\"participants\":[]}]").unwrap();
        assert_eq!(actual, expected, "{}", "demote");
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("linked_group_promote")
                    .attr("jid", "987654@g.us")
                    .children(vec![])
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual =
            actual.unwrap_or_else(|error| panic!("{}: {error:?}", "linked-promote-group-id"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected:Value=serde_json::from_str("[{\"actionType\":\"linked_group_promote\",\"jid\":\"987654@g.us\",\"participants\":[]}]").unwrap();
        assert_eq!(actual, expected, "{}", "linked-promote-group-id");
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("linked_group_demote")
                    .children(vec![])
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual = actual
            .unwrap_or_else(|error| panic!("{}: {error:?}", "linked-demote-without-group-id"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected: Value =
            serde_json::from_str("[{\"actionType\":\"linked_group_demote\",\"participants\":[]}]")
                .unwrap();
        assert_eq!(actual, expected, "{}", "linked-demote-without-group-id");
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: true,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("modify")
                    .children(vec![
                        NodeBuilder::new("participant")
                            .attr("group_history_sent_state", "HISTORY_SENT")
                            .attr("jid", "100001@lid")
                            .attr("join_time", "4294967297")
                            .build(),
                    ])
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual =
            actual.unwrap_or_else(|error| panic!("{}: {error:?}", "modify-history-enabled"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected:Value=serde_json::from_str("[{\"actionType\":\"modify\",\"participants\":[{\"displayName\":null,\"groupHistorySentState\":2,\"id\":\"100001@lid\",\"isAdmin\":false,\"isSuperAdmin\":false,\"joinTime\":1,\"lid\":null,\"phoneNumber\":null,\"username\":null}]}]").unwrap();
        assert_eq!(actual, expected, "{}", "modify-history-enabled");
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: true,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("modify")
                    .children(vec![
                        NodeBuilder::new("participant")
                            .attr("group_history_sent_state", "future")
                            .attr("jid", "100001@lid")
                            .build(),
                    ])
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual = actual
            .unwrap_or_else(|error| panic!("{}: {error:?}", "modify-history-null-unknown-state"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected:Value=serde_json::from_str("[{\"actionType\":\"modify\",\"participants\":[{\"displayName\":null,\"groupHistorySentState\":null,\"id\":\"100001@lid\",\"isAdmin\":false,\"isSuperAdmin\":false,\"joinTime\":null,\"lid\":null,\"phoneNumber\":null,\"username\":null}]}]").unwrap();
        assert_eq!(actual, expected, "{}", "modify-history-null-unknown-state");
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("add")
                    .children(vec![
                        NodeBuilder::new("participant")
                            .attr("jid", "bad")
                            .attr("type", "future_role")
                            .build(),
                    ])
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let error = actual.unwrap_err();
        assert_eq!(
            failed_field(&error),
            Some("type"),
            "{}: {error:?}",
            "participant-unknown-role-precedes-jid"
        );
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("subject")
                    .attr("s_o", "100001@lid")
                    .attr("s_o_pn", "100002@lid")
                    .attr("s_o_username", "ungated")
                    .attr("s_t", "4294967297")
                    .attr("subject", "synthetic title")
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual = actual.unwrap_or_else(|error| {
            panic!(
                "{}: {error:?}",
                "subject-generic-owner-alias-and-ungated-username"
            )
        });
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected:Value=serde_json::from_str("[{\"actionType\":\"subject\",\"s_o\":\"100001@lid\",\"s_t\":1,\"subject\":\"synthetic title\",\"subjectOwnerPn\":\"100002@lid\",\"subjectOwnerUsername\":\"ungated\"}]").unwrap();
        assert_eq!(
            actual, expected,
            "{}",
            "subject-generic-owner-alias-and-ungated-username"
        );
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![NodeBuilder::new("subject").build()])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let error = actual.unwrap_err();
        assert_eq!(
            failed_field(&error),
            Some("subject"),
            "{}: {error:?}",
            "subject-missing-required-name"
        );
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("description")
                    .attr("id", "description-id")
                    .children(vec![
                        NodeBuilder::new("body")
                            .bytes(vec![
                                115, 121, 110, 116, 104, 101, 116, 105, 99, 32, 100, 101, 115, 99,
                                114, 105, 112, 116, 105, 111, 110,
                            ])
                            .build(),
                    ])
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual = actual.unwrap_or_else(|error| {
            panic!("{}: {error:?}", "description-add-retains-envelope-time")
        });
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected:Value=serde_json::from_str("[{\"actionType\":\"desc_add\",\"desc\":\"synthetic description\",\"descId\":\"description-id\",\"descTime\":123}]").unwrap();
        assert_eq!(
            actual, expected,
            "{}",
            "description-add-retains-envelope-time"
        );
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("description")
                    .attr("id", "description-id")
                    .children(vec![
                        NodeBuilder::new("delete").build(),
                        NodeBuilder::new("body").build(),
                    ])
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual = actual
            .unwrap_or_else(|error| panic!("{}: {error:?}", "description-delete-ignores-body"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected: Value = serde_json::from_str(
            "[{\"actionType\":\"desc_remove\",\"descId\":\"description-id\"}]",
        )
        .unwrap();
        assert_eq!(actual, expected, "{}", "description-delete-ignores-body");
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("description")
                    .attr("id", "description-id")
                    .children(vec![NodeBuilder::new("body").build()])
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let error = actual.unwrap_err();
        assert_eq!(
            failed_field(&error),
            Some("body"),
            "{}: {error:?}",
            "description-body-without-content-errors"
        );
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("suspended")
                    .attr("threshold", "synthetic-threshold")
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual = actual.unwrap_or_else(|error| panic!("{}: {error:?}", "paired-suspended"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected: Value =
            serde_json::from_str("[{\"actionType\":\"suspend\",\"value\":true}]").unwrap();
        assert_eq!(actual, expected, "{}", "paired-suspended");
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("unsuspended")
                    .attr("threshold", "synthetic-threshold")
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual = actual.unwrap_or_else(|error| panic!("{}: {error:?}", "paired-unsuspended"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected: Value =
            serde_json::from_str("[{\"actionType\":\"suspend\",\"value\":false}]").unwrap();
        assert_eq!(actual, expected, "{}", "paired-unsuspended");
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("locked")
                    .attr("threshold", "synthetic-threshold")
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual = actual.unwrap_or_else(|error| panic!("{}: {error:?}", "paired-locked"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected: Value = serde_json::from_str(
            "[{\"actionType\":\"restrict\",\"threshold\":\"synthetic-threshold\",\"value\":true}]",
        )
        .unwrap();
        assert_eq!(actual, expected, "{}", "paired-locked");
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("unlocked")
                    .attr("threshold", "synthetic-threshold")
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual = actual.unwrap_or_else(|error| panic!("{}: {error:?}", "paired-unlocked"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected: Value =
            serde_json::from_str("[{\"actionType\":\"restrict\",\"value\":false}]").unwrap();
        assert_eq!(actual, expected, "{}", "paired-unlocked");
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("announcement")
                    .attr("threshold", "synthetic-threshold")
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual = actual.unwrap_or_else(|error| panic!("{}: {error:?}", "paired-announcement"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected: Value =
            serde_json::from_str("[{\"actionType\":\"announce\",\"value\":true}]").unwrap();
        assert_eq!(actual, expected, "{}", "paired-announcement");
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("not_announcement")
                    .attr("threshold", "synthetic-threshold")
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual =
            actual.unwrap_or_else(|error| panic!("{}: {error:?}", "paired-not_announcement"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected: Value =
            serde_json::from_str("[{\"actionType\":\"announce\",\"value\":false}]").unwrap();
        assert_eq!(actual, expected, "{}", "paired-not_announcement");
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("no_frequently_forwarded")
                    .attr("threshold", "synthetic-threshold")
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual = actual
            .unwrap_or_else(|error| panic!("{}: {error:?}", "paired-no_frequently_forwarded"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected: Value =
            serde_json::from_str("[{\"actionType\":\"no_frequently_forwarded\",\"value\":true}]")
                .unwrap();
        assert_eq!(actual, expected, "{}", "paired-no_frequently_forwarded");
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("frequently_forwarded_ok")
                    .attr("threshold", "synthetic-threshold")
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual = actual
            .unwrap_or_else(|error| panic!("{}: {error:?}", "paired-frequently_forwarded_ok"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected: Value =
            serde_json::from_str("[{\"actionType\":\"no_frequently_forwarded\",\"value\":false}]")
                .unwrap();
        assert_eq!(actual, expected, "{}", "paired-frequently_forwarded_ok");
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("invite")
                    .attr("code", "synthetic-code")
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual = actual.unwrap_or_else(|error| panic!("{}: {error:?}", "invite"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected: Value =
            serde_json::from_str("[{\"actionType\":\"invite\",\"code\":\"synthetic-code\"}]")
                .unwrap();
        assert_eq!(actual, expected, "{}", "invite");
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("ephemeral")
                    .attr("expiration", " -2seconds")
                    .attr("trigger", " +3trailing")
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual = actual
            .unwrap_or_else(|error| panic!("{}: {error:?}", "ephemeral-prefix-integers-and-actor"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected: Value = serde_json::from_str(
            "[{\"actionType\":\"ephemeral\",\"duration\":-2,\"initiatedByMe\":true,\"trigger\":3}]",
        )
        .unwrap();
        assert_eq!(
            actual, expected,
            "{}",
            "ephemeral-prefix-integers-and-actor"
        );
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("not_ephemeral")
                    .attr("expiration", "999")
                    .attr("trigger", "invalid")
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual = actual
            .unwrap_or_else(|error| panic!("{}: {error:?}", "not-ephemeral-ignores-attributes"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected: Value =
            serde_json::from_str("[{\"actionType\":\"ephemeral\",\"duration\":0}]").unwrap();
        assert_eq!(actual, expected, "{}", "not-ephemeral-ignores-attributes");
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![NodeBuilder::new("ephemeral").build()])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let error = actual.unwrap_err();
        assert_eq!(
            failed_field(&error),
            Some("expiration"),
            "{}: {error:?}",
            "ephemeral-missing-expiration-errors"
        );
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("revoke")
                    .children(vec![
                        NodeBuilder::new("participant")
                            .attr("expiration", " +5days")
                            .attr("jid", "100001@lid")
                            .build(),
                    ])
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual =
            actual.unwrap_or_else(|error| panic!("{}: {error:?}", "revoke-participant-expiration"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected:Value=serde_json::from_str("[{\"actionType\":\"revoke_invite\",\"participants\":[{\"expiration\":5,\"id\":\"100001@lid\"}]}]").unwrap();
        assert_eq!(actual, expected, "{}", "revoke-participant-expiration");
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![NodeBuilder::new("revoke").string_content("").build()])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let error = actual.unwrap_err();
        assert_eq!(
            failed_field(&error),
            Some("children"),
            "{}: {error:?}",
            "revoke-scalar-content-errors"
        );
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("growth_locked")
                    .attr("expiration", "-2.5")
                    .attr("type", "synthetic-lock")
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual = actual
            .unwrap_or_else(|error| panic!("{}: {error:?}", "growth-locked-integer-and-type"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected: Value = serde_json::from_str(
            "[{\"actionType\":\"growth_locked\",\"expiration\":-2,\"type\":\"synthetic-lock\"}]",
        )
        .unwrap();
        assert_eq!(actual, expected, "{}", "growth-locked-integer-and-type");
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![NodeBuilder::new("growth_unlocked").build()])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual = actual.unwrap_or_else(|error| panic!("{}: {error:?}", "growth-unlocked"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected: Value =
            serde_json::from_str("[{\"actionType\":\"growth_unlocked\"}]").unwrap();
        assert_eq!(actual, expected, "{}", "growth-unlocked");
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("membership_approval_mode")
                    .attr("triggered", "synthetic")
                    .children(vec![
                        NodeBuilder::new("group_join").attr("state", "on").build(),
                    ])
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual =
            actual.unwrap_or_else(|error| panic!("{}: {error:?}", "membership-mode-triggered"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected:Value=serde_json::from_str("[{\"actionType\":\"membership_approval_mode\",\"triggered\":\"synthetic\",\"value\":true}]").unwrap();
        assert_eq!(actual, expected, "{}", "membership-mode-triggered");
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![NodeBuilder::new("membership_approval_mode").build()])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let error = actual.unwrap_err();
        assert_eq!(
            failed_field(&error),
            Some("group_join"),
            "{}: {error:?}",
            "membership-mode-child-required"
        );
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("membership_approval_request")
                    .attr("parent_group_jid", "987654@g.us")
                    .attr("request_method", "future")
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual = actual.unwrap_or_else(|error| {
            panic!(
                "{}: {error:?}",
                "membership-request-ordinary-unknown-default"
            )
        });
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected:Value=serde_json::from_str("[{\"actionType\":\"membership_approval_request\",\"parentGroupId\":\"987654@g.us\",\"requestMethod\":\"InviteLink\"}]").unwrap();
        assert_eq!(
            actual, expected,
            "{}",
            "membership-request-ordinary-unknown-default"
        );
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("membership_approval_request")
                    .attr("request_method", "linked_group_join")
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual = actual.unwrap_or_else(|error| {
            panic!("{}: {error:?}", "membership-request-linked-group-method")
        });
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected:Value=serde_json::from_str("[{\"actionType\":\"membership_approval_request\",\"requestMethod\":\"LinkedGroupJoin\"}]").unwrap();
        assert_eq!(
            actual, expected,
            "{}",
            "membership-request-linked-group-method"
        );
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("membership_approval_request").build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let error = actual.unwrap_err();
        assert_eq!(
            failed_field(&error),
            Some("request_method"),
            "{}: {error:?}",
            "membership-request-missing-method-errors"
        );
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("membership_approval_request")
                    .attr("request_method", "toString")
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        assert!(
            matches!(
                actual,
                Err(GroupParseError::UnsupportedActionType("request_method"))
            ),
            "{}: {actual:?}",
            "membership-request-inherited-method-is-opaque"
        );
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("membership_approval_request")
                    .attr("parent_group_jid", "200002@lid")
                    .attr("request_method", "toString")
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let error = actual.unwrap_err();
        assert_eq!(
            failed_field(&error),
            Some("parent_group_jid"),
            "{}: {error:?}",
            "membership-request-later-group-error-before-opaque"
        );
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("allow_admin_reports")
                    .attr("triggered", "synthetic")
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual = actual
            .unwrap_or_else(|error| panic!("{}: {error:?}", "allow-admin-reports-author-trigger"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected:Value=serde_json::from_str("[{\"actionType\":\"allow_admin_reports\",\"author\":\"200002@s.whatsapp.net\",\"shouldSkipGenMsg\":false,\"triggered\":\"synthetic\",\"value\":true}]").unwrap();
        assert_eq!(actual, expected, "{}", "allow-admin-reports-author-trigger");
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("not_allow_admin_reports")
                    .attr("triggered", "synthetic")
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual = actual.unwrap_or_else(|error| {
            panic!("{}: {error:?}", "not-allow-admin-reports-author-trigger")
        });
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected:Value=serde_json::from_str("[{\"actionType\":\"allow_admin_reports\",\"author\":\"200002@s.whatsapp.net\",\"shouldSkipGenMsg\":false,\"triggered\":\"synthetic\",\"value\":false}]").unwrap();
        assert_eq!(
            actual, expected,
            "{}",
            "not-allow-admin-reports-author-trigger"
        );
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![NodeBuilder::new("reports").build()])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual =
            actual.unwrap_or_else(|error| panic!("{}: {error:?}", "reports-use-envelope-time"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected: Value = serde_json::from_str(
            "[{\"actionType\":\"admin_report_received\",\"shouldSkipGenMsg\":true,\"value\":123}]",
        )
        .unwrap();
        assert_eq!(actual, expected, "{}", "reports-use-envelope-time");
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("created_membership_requests")
                    .attr("request_method", "invite_link")
                    .attr("suppress_sys_msg", "true")
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual =
            actual.unwrap_or_else(|error| panic!("{}: {error:?}", "requests-no-content-fallback"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected:Value=serde_json::from_str("[{\"actionType\":\"created_membership_requests\",\"requestMethod\":\"InviteLink\",\"requests\":[{\"wid\":\"200002@s.whatsapp.net\"}],\"skipGenMsg\":true}]").unwrap();
        assert_eq!(actual, expected, "{}", "requests-no-content-fallback");
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("created_membership_requests")
                    .attr("request_method", "invite_link")
                    .children(vec![])
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual = actual.unwrap_or_else(|error| {
            panic!("{}: {error:?}", "requests-empty-array-is-not-fallback")
        });
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected:Value=serde_json::from_str("[{\"actionType\":\"created_membership_requests\",\"requestMethod\":\"InviteLink\",\"requests\":[],\"skipGenMsg\":false}]").unwrap();
        assert_eq!(
            actual, expected,
            "{}",
            "requests-empty-array-is-not-fallback"
        );
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("created_membership_requests")
                    .attr("request_method", "invite_link")
                    .bytes(vec![115, 99, 97, 108, 97, 114])
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual =
            actual.unwrap_or_else(|error| panic!("{}: {error:?}", "requests-scalar-is-fallback"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected:Value=serde_json::from_str("[{\"actionType\":\"created_membership_requests\",\"requestMethod\":\"InviteLink\",\"requests\":[{\"wid\":\"200002@s.whatsapp.net\"}],\"skipGenMsg\":false}]").unwrap();
        assert_eq!(actual, expected, "{}", "requests-scalar-is-fallback");
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("created_membership_requests")
                    .attr("request_method", "invite_link")
                    .children(vec![NodeBuilder::new("future_child").build()])
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual = actual.unwrap_or_else(|error| {
            panic!(
                "{}: {error:?}",
                "requests-unrelated-children-do-not-fallback"
            )
        });
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected:Value=serde_json::from_str("[{\"actionType\":\"created_membership_requests\",\"requestMethod\":\"InviteLink\",\"requests\":[],\"skipGenMsg\":false}]").unwrap();
        assert_eq!(
            actual, expected,
            "{}",
            "requests-unrelated-children-do-not-fallback"
        );
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("created_membership_requests")
                    .attr("request_method", "invite_link")
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        assert!(
            matches!(actual,Err(GroupParseError::Action(ref error)) if matches!(error.as_ref(),GroupActionParseError::MissingRequestFallbackAuthor)),
            "{}: {actual:?}",
            "requests-no-author-fallback-errors"
        );
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("created_membership_requests")
                    .attr("request_method", "invite_link")
                    .attr("suppress_sys_msg", "TRUE")
                    .children(vec![
                        NodeBuilder::new("requested_user")
                            .attr("jid", "100001@lid")
                            .attr("phone_number", "100002@lid")
                            .attr("username", "gated")
                            .build(),
                    ])
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual = actual.unwrap_or_else(|error| {
            panic!(
                "{}: {error:?}",
                "requests-username-disabled-phone-is-generic-user"
            )
        });
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected:Value=serde_json::from_str("[{\"actionType\":\"created_membership_requests\",\"requestMethod\":\"InviteLink\",\"requests\":[{\"phoneNumber\":\"100002@lid\",\"wid\":\"100001@lid\"}],\"skipGenMsg\":false}]").unwrap();
        assert_eq!(
            actual, expected,
            "{}",
            "requests-username-disabled-phone-is-generic-user"
        );
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: true,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("created_membership_requests")
                    .attr("request_method", "non_admin_add")
                    .children(vec![
                        NodeBuilder::new("requested_user")
                            .attr("jid", "100001@lid")
                            .attr("username", "visible")
                            .build(),
                    ])
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual =
            actual.unwrap_or_else(|error| panic!("{}: {error:?}", "requests-username-enabled"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected:Value=serde_json::from_str("[{\"actionType\":\"created_membership_requests\",\"requestMethod\":\"NonAdminAdd\",\"requests\":[{\"username\":\"visible\",\"wid\":\"100001@lid\"}],\"skipGenMsg\":false}]").unwrap();
        assert_eq!(actual, expected, "{}", "requests-username-enabled");
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("created_membership_requests")
                    .attr("request_method", "__proto__")
                    .children(vec![])
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        assert!(
            matches!(
                actual,
                Err(GroupParseError::UnsupportedActionType("request_method"))
            ),
            "{}: {actual:?}",
            "requests-opaque-method"
        );
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("revoked_membership_requests")
                    .children(vec![
                        NodeBuilder::new("participant")
                            .attr("jid", "100001@lid")
                            .build(),
                        NodeBuilder::new("participant")
                            .attr("jid", "200003@s.whatsapp.net")
                            .build(),
                    ])
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual =
            actual.unwrap_or_else(|error| panic!("{}: {error:?}", "revoked-membership-requests"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected:Value=serde_json::from_str("[{\"actionType\":\"revoked_membership_requests\",\"requests\":[\"100001@lid\",\"200003@s.whatsapp.net\"]}]").unwrap();
        assert_eq!(actual, expected, "{}", "revoked-membership-requests");
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("revoked_membership_requests")
                    .children(vec![
                        NodeBuilder::new("participant")
                            .attr("jid", "200003@g.us")
                            .build(),
                    ])
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let error = actual.unwrap_err();
        assert_eq!(
            failed_field(&error),
            Some("jid"),
            "{}: {error:?}",
            "revoked-membership-invalid-child-errors"
        );
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("allow_non_admin_sub_group_creation").build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual =
            actual.unwrap_or_else(|error| panic!("{}: {error:?}", "allow-nonadmin-subgroup"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected: Value = serde_json::from_str(
            "[{\"actionType\":\"allow_non_admin_sub_group_creation\",\"value\":true}]",
        )
        .unwrap();
        assert_eq!(actual, expected, "{}", "allow-nonadmin-subgroup");
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("not_allow_non_admin_sub_group_creation").build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual =
            actual.unwrap_or_else(|error| panic!("{}: {error:?}", "not-allow-nonadmin-subgroup"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected: Value = serde_json::from_str(
            "[{\"actionType\":\"allow_non_admin_sub_group_creation\",\"value\":false}]",
        )
        .unwrap();
        assert_eq!(actual, expected, "{}", "not-allow-nonadmin-subgroup");
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("member_add_mode")
                    .bytes(vec![
                        97, 108, 108, 95, 109, 101, 109, 98, 101, 114, 95, 97, 100, 100,
                    ])
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual = actual.unwrap_or_else(|error| panic!("{}: {error:?}", "member-add-mode"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected: Value = serde_json::from_str(
            "[{\"actionType\":\"member_add_mode\",\"memberAddMode\":\"all_member_add\"}]",
        )
        .unwrap();
        assert_eq!(actual, expected, "{}", "member-add-mode");
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![
                NodeBuilder::new("member_add_mode")
                    .bytes(vec![102, 117, 116, 117, 114, 101, 95, 109, 111, 100, 101])
                    .build(),
            ])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual = actual.unwrap_or_else(|error| {
            panic!("{}: {error:?}", "member-add-mode-unknown-is-undefined")
        });
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected: Value =
            serde_json::from_str("[{\"actionType\":\"member_add_mode\"}]").unwrap();
        assert_eq!(
            actual, expected,
            "{}",
            "member-add-mode-unknown-is-undefined"
        );
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![NodeBuilder::new("member_add_mode").build()])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let error = actual.unwrap_err();
        assert_eq!(
            failed_field(&error),
            Some("member_add_mode"),
            "{}: {error:?}",
            "member-add-mode-missing-content-errors"
        );
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![NodeBuilder::new("auto_add_disabled").build()])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual = actual.unwrap_or_else(|error| panic!("{}: {error:?}", "auto-add-disabled"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected: Value =
            serde_json::from_str("[{\"actionType\":\"general_chat_auto_add_disabled\"}]").unwrap();
        assert_eq!(actual, expected, "{}", "auto-add-disabled");
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![NodeBuilder::new("group_safety_check").build()])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        let actual = actual.unwrap_or_else(|error| panic!("{}: {error:?}", "group-safety-check"));
        let actual = Value::Array(
            actual
                .actions
                .iter()
                .map(|action| projection::project(action, context.history_after_join).unwrap())
                .collect(),
        );
        let expected: Value =
            serde_json::from_str("[{\"actionType\":\"group_safety_check\",\"value\":true}]")
                .unwrap();
        assert_eq!(actual, expected, "{}", "group-safety-check");
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![NodeBuilder::new("is_capi_hosted_group").build()])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        assert!(
            matches!(actual,Err(GroupParseError::UnknownAction(ref tag)) if tag=="is_capi_hosted_group"),
            "{}: {actual:?}",
            "unknown-is_capi_hosted_group"
        );
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![NodeBuilder::new("limit_sharing_enabled").build()])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        assert!(
            matches!(actual,Err(GroupParseError::UnknownAction(ref tag)) if tag=="limit_sharing_enabled"),
            "{}: {actual:?}",
            "unknown-limit_sharing_enabled"
        );
    }
    {
        let context = GroupParseContext {
            me_device_lid: Some(&me_lid),
            me_device_pn: Some(&me_pn),
            username_displayed: false,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &is_me,
            normalize_country_code: &country,
        };
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("participant", "200002@s.whatsapp.net")
            .attr("t", "123")
            .children(vec![NodeBuilder::new("future_action").build()])
            .build();
        let actual = parse_normalized_group_notification(&node.as_node_ref(), &context);
        assert!(
            matches!(actual,Err(GroupParseError::UnknownAction(ref tag)) if tag=="future_action"),
            "{}: {actual:?}",
            "unknown-future_action"
        );
    }
}

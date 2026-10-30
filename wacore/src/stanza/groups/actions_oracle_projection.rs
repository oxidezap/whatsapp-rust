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

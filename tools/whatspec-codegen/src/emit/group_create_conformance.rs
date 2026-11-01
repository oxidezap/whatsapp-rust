//! Rust assertions against independently executed original CREATE helpers and
//! decoders. The fixture is emitted by whatspec's groups/create.cjs --json.
use super::{header, rust_str};
use anyhow::{Context, Result, ensure};
use serde_json::Value;
const ORACLE: &str = include_str!("../../fixtures/group_create_oracle.json");

fn exact_keys(value: &Value, allowed: &[&str]) -> Result<()> {
    let object = value.as_object().context("expected oracle object")?;
    for key in object.keys() {
        ensure!(
            allowed.contains(&key.as_str()),
            "unqualified CREATE oracle field {key}"
        );
    }
    Ok(())
}
fn node(value: &Value) -> Result<String> {
    exact_keys(value, &["tag", "attrs", "children", "bytes"])?;
    let tag = value["tag"].as_str().context("missing oracle tag")?;
    let mut out = format!("NodeBuilder::new({})", rust_str(tag));
    for (key, value) in value["attrs"]
        .as_object()
        .context("missing oracle attributes")?
    {
        out.push_str(&format!(
            ".attr({},{})",
            rust_str(key),
            rust_str(value.as_str().context("non-string oracle attribute")?)
        ));
    }
    ensure!(
        value.get("children").is_none() || value.get("bytes").is_none(),
        "ambiguous oracle content"
    );
    if let Some(children) = value.get("children") {
        let children = children
            .as_array()
            .context("non-array oracle children")?
            .iter()
            .map(node)
            .collect::<Result<Vec<_>>>()?;
        out.push_str(&format!(".children(vec![{}])", children.join(",")));
    }
    if let Some(bytes) = value.get("bytes") {
        let bytes = bytes
            .as_array()
            .context("non-array oracle bytes")?
            .iter()
            .map(|b| {
                let value = b.as_u64().context("non-integer oracle byte")?;
                ensure!(value <= 255, "out-of-range oracle byte");
                Ok(value.to_string())
            })
            .collect::<Result<Vec<_>>>()?;
        out.push_str(&format!(".bytes(vec![{}])", bytes.join(",")));
    }
    out.push_str(".build()");
    Ok(out)
}
fn optional_string(value: &Value) -> Result<String> {
    if value.is_null() {
        return Ok("None".into());
    }
    Ok(format!(
        "Some({})",
        rust_str(value.as_str().context("non-string optional oracle value")?)
    ))
}
fn optional_int(value: &Value) -> Result<String> {
    if value.is_null() {
        return Ok("None".into());
    }
    Ok(format!(
        "Some({}_i64)",
        value
            .as_i64()
            .context("non-integer optional oracle value")?
    ))
}
fn optional_bool(value: &Value) -> Result<String> {
    if value.is_null() {
        return Ok("None".into());
    }
    Ok(format!(
        "Some({})",
        value
            .as_bool()
            .context("non-boolean optional oracle value")?
    ))
}
fn compare(out: &mut String, actual: &str, expected: &str, label: &str) {
    match expected {
        "true" => out.push_str(&format!("assert!({actual},{});\n", rust_str(label))),
        "false" => out.push_str(&format!("assert!(!({actual}),{});\n", rust_str(label))),
        _ => out.push_str(&format!(
            "assert_eq!({actual},{expected},{});\n",
            rust_str(label)
        )),
    }
}
fn opt_text(out: &mut String, actual: &str, value: &Value, label: &str) -> Result<()> {
    compare(out, actual, &optional_string(value)?, label);
    Ok(())
}
fn opt_jid(out: &mut String, actual: &str, value: &Value, label: &str) -> Result<()> {
    opt_text(
        out,
        &format!("{actual}.as_ref().map(ToString::to_string).as_deref()"),
        value,
        label,
    )
}

pub fn generate(wa_version: &str) -> Result<String> {
    let oracle: Value = serde_json::from_str(ORACLE)?;
    ensure!(
        wa_version == "2.3000.1052559453" && oracle["waVersion"].as_str() == Some(wa_version),
        "CREATE oracle requires snapshot review"
    );
    ensure!(
        oracle["setHash"] == "d6ab7ae601eda281cfac570a68de1d787adde8f4d778f0bb994416f661a21abf",
        "CREATE oracle bundle mismatch"
    );
    exact_keys(&oracle, &["waVersion", "setHash", "cases"])?;
    let cases = oracle["cases"]
        .as_array()
        .context("missing CREATE oracle cases")?;
    ensure!(cases.len() == 13, "CREATE oracle coverage requires review");
    let mut out = header("isolated official CREATE conformance cases", wa_version);
    out.push_str("//! Original handler module SHA-256 d1641fad966fd3b32d6b9859e074f804526e67181584811364774047208c273d.\n//! The complete original groupNotificationParser callback and official XML/Wap/JID/time/enum dependencies run in the versioned fixture harness.\n//! Effective account flags/country registry, Wid conversion, binary adapter, native-Error bridge and telemetry are mocked.\n\nuse super::{parse_group_create,GroupCreateContext,CreateParseError};\nuse crate::stanza::groups::{GroupParticipantType,GroupHistorySentState};\nuse wacore_binary::{Jid,builder::NodeBuilder};\n\n#[test]\nfn isolated_official_create_helpers_match_rust_projection() {\n");
    for case in cases {
        exact_keys(case, &["name", "flags", "input", "success", "error"])?;
        let name = case["name"].as_str().context("missing CREATE case name")?;
        exact_keys(&case["flags"], &["history", "username", "opus", "me"])?;
        let flag = |key: &str| {
            case["flags"][key]
                .as_bool()
                .with_context(|| format!("missing CREATE flag {key}"))
        };
        let history = flag("history")?;
        let username = flag("username")?;
        let opus = flag("opus")?;
        let is_me = case["flags"]
            .get("me")
            .filter(|v| !v.is_null())
            .map(|v| v.as_str().context("invalid me fact"))
            .transpose()?;
        out.push_str("{\n");
        out.push_str(&format!("let node={};\n", node(&case["input"])?));
        out.push_str(&format!(
            "let is_me=|jid:&Jid| {};\n",
            is_me.map_or("{let _=jid;false}".into(), |me| format!(
                "jid.to_string()=={}",
                rust_str(me)
            ))
        ));
        out.push_str(&format!("let country_enabled={username};let country=|value:&str| {{assert!(country_enabled,\"disabled country lookup\");match value {{\"US\"|\"BR\"=>Some(value.to_owned()),_=>None}}}};\nlet context=GroupCreateContext{{username_displayed:{username},history_after_join:{history},opus_enabled:{opus},envelope_author:None,is_me_account:&is_me,normalize_country_code:&country}};\n"));
        ensure!(
            case.get("success").is_some() != case.get("error").is_some(),
            "ambiguous CREATE oracle result"
        );
        if let Some(error) = case.get("error") {
            let error = error.as_str().context("non-string CREATE oracle error")?;
            let field = if error.contains("to have \"type\"={") {
                "type"
            } else if error.contains("to have attribute \"id\"") {
                "id"
            } else if error
                == "XmppParsingFailure: synthetic: expected <participant> to have \"join_time\"={integer} but has value \"invalid\""
            {
                "join_time"
            } else {
                anyhow::bail!("unqualified CREATE oracle error {error}");
            };
            compare(
                &mut out,
                "parse_group_create(&node.as_node_ref(),&context).unwrap_err()",
                &format!("CreateParseError::Field({})", rust_str(field)),
                name,
            );
            out.push_str("}\n");
            continue;
        }
        let expected = &case["success"];
        exact_keys(
            expected,
            &[
                "actionType",
                "reason",
                "contextGroupId",
                "groupInfo",
                "isNewGroup",
            ],
        )?;
        ensure!(
            expected["actionType"] == "create",
            "non-CREATE oracle action"
        );
        out.push_str("let parsed=parse_group_create(&node.as_node_ref(),&context).unwrap();\n");
        compare(
            &mut out,
            "parsed.is_new_group",
            &expected["isNewGroup"]
                .as_bool()
                .context("missing isNewGroup")?
                .to_string(),
            name,
        );
        opt_text(
            &mut out,
            "parsed.reason.as_ref().map(|v|v.normalized_value())",
            &expected["reason"],
            &format!("{name}.reason"),
        )?;
        opt_jid(
            &mut out,
            "parsed.context_group_id",
            &expected["contextGroupId"],
            &format!("{name}.contextGroupId"),
        )?;
        let group = &expected["groupInfo"];
        exact_keys(
            group,
            &[
                "subject",
                "restrict",
                "announce",
                "noFrequentlyForwarded",
                "ephemeralDuration",
                "disappearingModeTrigger",
                "disappearingModeInitiatedByMe",
                "owner",
                "creatorPn",
                "creatorCountryCode",
                "creatorUsername",
                "creation",
                "participants",
                "desc",
                "descId",
                "subjectOwner",
                "subjectOwnerPn",
                "subjectOwnerUsername",
                "subjectTime",
                "support",
                "isParentGroup",
                "isParentGroupClosed",
                "parentGroup",
                "defaultSubgroup",
                "generalSubgroup",
                "size",
                "membershipApprovalMode",
                "allowNonAdminSubGroupCreation",
                "generalChatAutoAddDisabled",
                "hiddenSubgroup",
                "groupSafetyCheck",
                "groupAdder",
                "hasCapi",
                "limitSharingEnabled",
                "memberLinkMode",
                "memberAddMode",
                "memberShareGroupHistoryMode",
            ],
        )?;
        out.push_str("let group=&parsed.group_info;\n");
        compare(
            &mut out,
            "group.subject.as_str()",
            &rust_str(group["subject"].as_str().context("missing group subject")?),
            name,
        );
        compare(
            &mut out,
            "group.creation",
            &format!(
                "{}_i64",
                group["creation"].as_i64().context("missing creation")?
            ),
            name,
        );
        for (wire, rust) in [
            ("restrict", "restrict"),
            ("announce", "announce"),
            ("noFrequentlyForwarded", "no_frequently_forwarded"),
            ("support", "support"),
            ("isParentGroup", "is_parent_group"),
            ("isParentGroupClosed", "is_parent_group_closed"),
            ("defaultSubgroup", "default_subgroup"),
            ("generalSubgroup", "general_subgroup"),
            ("membershipApprovalMode", "membership_approval_mode"),
            (
                "allowNonAdminSubGroupCreation",
                "allow_non_admin_subgroup_creation",
            ),
            (
                "generalChatAutoAddDisabled",
                "general_chat_auto_add_disabled",
            ),
            ("hiddenSubgroup", "hidden_subgroup"),
            ("groupSafetyCheck", "group_safety_check"),
            ("hasCapi", "has_capi"),
            ("limitSharingEnabled", "limit_sharing_enabled"),
        ] {
            compare(
                &mut out,
                &format!("group.{rust}"),
                &group[wire]
                    .as_bool()
                    .with_context(|| format!("missing group bool {wire}"))?
                    .to_string(),
                &format!("{name}.{wire}"),
            );
        }
        for (wire, rust) in [
            ("owner", "owner"),
            ("creatorPn", "creator_pn"),
            ("subjectOwner", "subject_owner"),
            ("subjectOwnerPn", "subject_owner_pn"),
            ("parentGroup", "parent_group"),
            ("groupAdder", "group_adder"),
        ] {
            opt_jid(
                &mut out,
                &format!("group.{rust}"),
                &group[wire],
                &format!("{name}.{wire}"),
            )?;
        }
        for (wire, rust) in [
            ("creatorCountryCode", "creator_country_code"),
            ("creatorUsername", "creator_username"),
            ("subjectOwnerUsername", "subject_owner_username"),
        ] {
            opt_text(
                &mut out,
                &format!("group.{rust}.as_deref()"),
                &group[wire],
                &format!("{name}.{wire}"),
            )?;
        }
        for (wire, rust) in [("subjectTime", "subject_time"), ("size", "size")] {
            compare(
                &mut out,
                &format!("group.{rust}"),
                &optional_int(&group[wire])?,
                &format!("{name}.{wire}"),
            );
        }
        for (wire, rust) in [
            ("memberLinkMode", "member_link_mode"),
            ("memberAddMode", "member_add_mode"),
            ("memberShareGroupHistoryMode", "member_share_history_mode"),
        ] {
            opt_text(
                &mut out,
                &format!("group.{rust}.as_ref().map(|v|v.as_str())"),
                &group[wire],
                &format!("{name}.{wire}"),
            )?;
        }
        compare(
            &mut out,
            "group.ephemerality.is_some()",
            &(!group["ephemeralDuration"].is_null()).to_string(),
            name,
        );
        compare(
            &mut out,
            "group.ephemerality.as_ref().map(|v|v.duration)",
            &optional_int(&group["ephemeralDuration"])?,
            name,
        );
        opt_text(
            &mut out,
            "group.ephemerality.as_ref().and_then(|v|v.trigger).as_ref().map(|v|v.as_str())",
            &group["disappearingModeTrigger"],
            name,
        )?;
        compare(
            &mut out,
            "group.ephemerality.as_ref().map(|v|v.initiated_by_me)",
            &optional_bool(&group["disappearingModeInitiatedByMe"])?,
            name,
        );
        opt_text(
            &mut out,
            "group.description.as_ref().map(|v|v.content.as_str())",
            &group["desc"],
            name,
        )?;
        opt_text(
            &mut out,
            "group.description.as_ref().map(|v|v.id.as_str())",
            &group["descId"],
            name,
        )?;
        let participants = group["participants"]
            .as_array()
            .context("missing CREATE participant output")?;
        compare(
            &mut out,
            "group.participants.len()",
            &participants.len().to_string(),
            name,
        );
        for (index, p) in participants.iter().enumerate() {
            exact_keys(
                p,
                &[
                    "displayName",
                    "id",
                    "isSuperAdmin",
                    "isAdmin",
                    "lid",
                    "phoneNumber",
                    "username",
                    "joinTime",
                    "groupHistorySentState",
                ],
            )?;
            out.push_str(&format!("let participant=&group.participants[{index}];\n"));
            compare(
                &mut out,
                "participant.id.to_string()",
                &rust_str(p["id"].as_str().context("missing participant id")?),
                name,
            );
            for (wire, rust) in [("displayName", "display_name"), ("username", "username")] {
                opt_text(
                    &mut out,
                    &format!("participant.{rust}.as_deref()"),
                    &p[wire],
                    name,
                )?;
            }
            for (wire, rust) in [("lid", "lid"), ("phoneNumber", "phone_number")] {
                opt_jid(&mut out, &format!("participant.{rust}"), &p[wire], name)?;
            }
            let admin = p["isAdmin"]
                .as_bool()
                .context("missing participant admin")?;
            let super_admin = p["isSuperAdmin"]
                .as_bool()
                .context("missing participant super-admin")?;
            compare(&mut out, "participant.is_admin", &admin.to_string(), name);
            compare(
                &mut out,
                "participant.is_super_admin",
                &super_admin.to_string(),
                name,
            );
            let role = if super_admin {
                "SuperAdmin"
            } else if admin {
                "Admin"
            } else {
                "Participant"
            };
            compare(
                &mut out,
                "participant.role",
                &format!("GroupParticipantType::{role}"),
                name,
            );
            compare(
                &mut out,
                "participant.join_time",
                &optional_int(&p["joinTime"])?,
                name,
            );
            compare(
                &mut out,
                "participant.group_history_sent_state.map(|v|match v {GroupHistorySentState::HistoryNotSent=>1_i64,GroupHistorySentState::HistorySent=>2_i64,GroupHistorySentState::NoticeSent=>3_i64})",
                &optional_int(&p["groupHistorySentState"])?,
                name,
            );
        }
        out.push_str("}\n");
    }
    out.push_str("}\n");
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn source_snapshot_is_required() {
        assert!(generate("future").is_err());
    }
    #[test]
    fn emits_independent_output_and_error_checks() {
        let rust = generate("2.3000.1052559453").unwrap();
        assert!(rust.contains("group.ephemerality"));
        assert!(rust.contains("CreateParseError::Field(\"type\")"));
        assert!(rust.contains("group.participants"));
    }
}

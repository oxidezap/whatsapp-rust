//! Compile complete independent official action outputs into test-only Rust.
//! No expected key or nonraw variant is silently selected away.
use super::{header, rust_str};
use anyhow::{Context, Result, bail, ensure};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

const ORACLE: &str = include_str!("../../fixtures/group_actions_oracle.json");
const REV: &str = "57494ccd865fa6471743264fea036f00d26d89cb";
const ORACLE_SHA: &str = "1e132eaa76698909e07ae33e90e624400b55cc6c6f061940b6553d394095031a";
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Oracle {
    wa_version: String,
    set_hash: String,
    cases: Vec<Case>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Case {
    name: String,
    flags: Flags,
    input: Node,
    #[serde(default)]
    success: Option<Value>,
    #[serde(default)]
    error: Option<SourceError>,
    #[serde(default)]
    unsupported_fields: Vec<Unsupported>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Flags {
    history: bool,
    username: bool,
    opus: bool,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Node {
    tag: String,
    attrs: BTreeMap<String, String>,
    children: Option<Vec<Node>>,
    bytes: Option<Vec<u8>>,
    string: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceError {
    name: String,
    message: String,
    reason: Option<String>,
    parser: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Unsupported {
    action: usize,
    field: String,
    value_type: String,
}

enum ExpectedError {
    Field(String),
    Unknown(String),
    MissingAuthor,
    UnsupportedRequestMethod,
}

pub fn generate(wa_version: &str) -> Result<String> {
    ensure!(
        wa_version == "2.3000.1047483476",
        "action oracle snapshot requires qualification"
    );
    ensure!(
        REV.len() == 40 && REV.bytes().all(|b| b.is_ascii_hexdigit()),
        "action oracle revision is not pinned"
    );
    ensure!(
        crate::source::digest(ORACLE.as_bytes()) == format!("sha256:{ORACLE_SHA}"),
        "action oracle differs from the independent pinned capture"
    );
    generate_reviewed(ORACLE, wa_version)
}
fn generate_reviewed(json: &str, wa_version: &str) -> Result<String> {
    let oracle: Oracle = serde_json::from_str(json)?;
    ensure!(
        oracle.wa_version == wa_version
            && oracle.set_hash
                == "05609307e68b0f6ccfd2a121a573049999e38b752cfe7155ebbe5c661805751b",
        "action oracle provenance differs"
    );
    ensure!(
        oracle.cases.len() == 64,
        "incomplete official nonraw action matrix"
    );
    let mut names = BTreeSet::new();
    let mut out = header(
        "complete official nonraw group action conformance",
        wa_version,
    );
    out.push_str(&format!("//! Original callback source: https://github.com/oxidezap/whatspec/blob/{REV}/tests/conformance/groups/actions.cjs\n//! Module SHA-256 a65d861ba0dacdcec52d2df48712a80677ecf10021d7df63c84183f195db13a1.\n//! Account facts/Wid conversion/logging are explicit test mocks; runtime handlers are not run.\n\n"));
    out.push_str("mod projection {\n");
    out.push_str(include_str!(
        "../../../../wacore/src/stanza/groups/actions_oracle_projection.rs"
    ));
    out.push_str("\n}\n");
    out.push_str("use crate::stanza::groups::normalized::{parse_normalized_group_notification,GroupParseContext,GroupParseError};\nuse crate::stanza::groups::actions_normalized::GroupActionParseError;\nuse crate::stanza::groups::create_normalized::CreateParseError;\nuse crate::stanza::groups::suggestions_normalized::SuggestionParseError;\nuse wacore_binary::{Jid,builder::NodeBuilder};\nuse serde_json::Value;\nfn is_me(jid:&Jid)->bool {jid.to_string()==\"200002@s.whatsapp.net\"}\nfn country(value:&str)->Option<String> {matches!(value,\"US\"|\"BR\").then(||value.to_string())}\nfn failed_field(error:&GroupParseError)->Option<&'static str> {match error {GroupParseError::Field(field)=>Some(field),GroupParseError::Action(error)=>match error.as_ref(){GroupActionParseError::Parse(GroupParseError::Field(field))|GroupActionParseError::Create(CreateParseError::Field(field))|GroupActionParseError::Suggestion(SuggestionParseError::Field(field))=>Some(field),_=>None},_=>None}}\n\n#[test]\nfn complete_nonraw_actions_match_the_original_callback() {\nlet me_lid=Jid::lid_device(\"100001\",2);let me_pn=Jid::pn_device(\"200002\",2);\n");
    for case in oracle.cases {
        ensure!(
            names.insert(case.name.clone()),
            "duplicate official action case name"
        );
        ensure!(
            case.success.is_some() != case.error.is_some(),
            "case must contain exactly one complete success or error"
        );
        let name = rust_str(&case.name);
        let input = node_expr(&case.input)?;
        let error = if let Some(error) = &case.error {
            ensure!(
                case.unsupported_fields.is_empty(),
                "error also contains an opaque successful result"
            );
            Some(classify(error)?)
        } else if !case.unsupported_fields.is_empty() {
            let [field] = case.unsupported_fields.as_slice() else {
                bail!("unqualified opaque field family");
            };
            ensure!(
                field.action == 0
                    && field.field == "requestMethod"
                    && matches!(field.value_type.as_str(), "function" | "object"),
                "unqualified opaque result field/type"
            );
            Some(ExpectedError::UnsupportedRequestMethod)
        } else {
            None
        };
        let expected = if let Some(success) = &case.success {
            keys(
                success,
                &[
                    "externalId",
                    "chatId",
                    "isLidAddressingMode",
                    "author",
                    "authorPhoneNumber",
                    "authorUsername",
                    "authorUsernameCountryCode",
                    "ts",
                    "pushname",
                    "offline",
                    "actions",
                    "hasIncompleteParticipantInformation",
                ],
            )?;
            let actions = success["actions"]
                .as_array()
                .context("missing complete action array")?;
            ensure!(actions.len() == 1, "nonraw oracle shape changed");
            validate_action(
                &actions[0],
                &case.flags,
                !case.unsupported_fields.is_empty(),
            )?;
            Some(Value::Array(actions.clone()))
        } else {
            None
        };
        out.push_str(&format!("{{\nlet context=GroupParseContext{{me_device_lid:Some(&me_lid),me_device_pn:Some(&me_pn),username_displayed:{},history_after_join:{},opus_enabled:{},is_me_account:&is_me,normalize_country_code:&country}};\nlet node={input};\nlet actual=parse_normalized_group_notification(&node.as_node_ref(),&context);\n",case.flags.username,case.flags.history,case.flags.opus));
        match error {
            Some(ExpectedError::Field(field))=>out.push_str(&format!("let error=actual.unwrap_err();assert_eq!(failed_field(&error),Some({}),\"{{}}: {{error:?}}\",{name});\n",rust_str(&field))),
            Some(ExpectedError::Unknown(tag))=>out.push_str(&format!("assert!(matches!(actual,Err(GroupParseError::UnknownAction(ref tag)) if tag=={}),\"{{}}: {{actual:?}}\",{name});\n",rust_str(&tag))),
            Some(ExpectedError::MissingAuthor)=>out.push_str(&format!("assert!(matches!(actual,Err(GroupParseError::Action(ref error)) if matches!(error.as_ref(),GroupActionParseError::MissingRequestFallbackAuthor)),\"{{}}: {{actual:?}}\",{name});\n")),
            Some(ExpectedError::UnsupportedRequestMethod)=>out.push_str(&format!("assert!(matches!(actual,Err(GroupParseError::UnsupportedActionType(\"request_method\"))),\"{{}}: {{actual:?}}\",{name});\n")),
            None=>{
                let expected=expected.context("missing success output")?;
                out.push_str(&format!("let actual=actual.unwrap_or_else(|error|panic!(\"{{}}: {{error:?}}\",{name}));\nlet actual=Value::Array(actual.actions.iter().map(|action|projection::project(action,context.history_after_join).unwrap()).collect());\nlet expected:Value=serde_json::from_str({}).unwrap();\nassert_eq!(actual,expected,\"{{}}\",{name});\n",rust_str(&expected.to_string())));
            }
        }
        out.push_str("}\n");
    }
    out.push_str("}\n");
    Ok(out)
}
fn node_expr(node: &Node) -> Result<String> {
    ensure!(
        [
            node.children.is_some(),
            node.bytes.is_some(),
            node.string.is_some()
        ]
        .into_iter()
        .filter(|v| *v)
        .count()
            <= 1,
        "ambiguous fixture content"
    );
    let mut out = format!("NodeBuilder::new({})", rust_str(&node.tag));
    for (key, value) in &node.attrs {
        out.push_str(&format!(".attr({},{})", rust_str(key), rust_str(value)));
    }
    if let Some(children) = &node.children {
        out.push_str(&format!(
            ".children(vec![{}])",
            children
                .iter()
                .map(node_expr)
                .collect::<Result<Vec<_>>>()?
                .join(",")
        ));
    }
    if let Some(bytes) = &node.bytes {
        out.push_str(&format!(".bytes(vec!{:?})", bytes));
    }
    if let Some(string) = &node.string {
        out.push_str(&format!(".string_content({})", rust_str(string)));
    }
    out.push_str(".build()");
    Ok(out)
}
fn classify(error: &SourceError) -> Result<ExpectedError> {
    if error.name == "Error" {
        ensure!(
            error.message == "Error: Got unexpected null or undefined"
                && error.reason.is_none()
                && error.parser.is_none(),
            "unqualified ordinary source error"
        );
        return Ok(ExpectedError::MissingAuthor);
    }
    ensure!(
        error.name == "XmppParsingFailure" && error.parser.as_deref() == Some("synthetic"),
        "unqualified source error class"
    );
    let reason = error
        .reason
        .as_deref()
        .context("missing parsing error reason")?;
    let (tag, rest) = reason
        .strip_prefix("expected <")
        .and_then(|s| s.split_once("> "))
        .context("unqualified parsing reason envelope")?;
    if let Some(action) = rest.strip_prefix("Unrecognized group action ") {
        ensure!(
            tag == "notification" && !action.is_empty(),
            "unqualified unknown-action source"
        );
        return Ok(ExpectedError::Unknown(action.into()));
    }
    if let Some(field) = rest
        .strip_prefix("to have attribute \"")
        .and_then(|s| s.strip_suffix('"'))
    {
        return Ok(ExpectedError::Field(field.into()));
    }
    if let Some((field, tail)) = rest
        .strip_prefix("to have \"")
        .and_then(|s| s.split_once("\"="))
    {
        ensure!(
            tail.contains("but") && !field.is_empty(),
            "unqualified typed accessor error"
        );
        return Ok(ExpectedError::Field(field.into()));
    }
    if let Some(child) = rest
        .strip_prefix("to have child <")
        .and_then(|s| s.strip_suffix('>'))
    {
        return Ok(ExpectedError::Field(child.into()));
    }
    if rest == "to have children" {
        return Ok(ExpectedError::Field("children".into()));
    }
    if rest == "to have content" {
        return Ok(ExpectedError::Field(tag.into()));
    }
    bail!("unqualified parsing failure: {reason}")
}
fn keys(value: &Value, allowed: &[&str]) -> Result<()> {
    let object = value
        .as_object()
        .context("expected complete source object")?;
    for key in object.keys() {
        ensure!(
            allowed.contains(&key.as_str()),
            "unqualified expected output key {key}"
        );
    }
    Ok(())
}
fn records<'a>(value: &'a Value, name: &str) -> Result<&'a Vec<Value>> {
    value[name]
        .as_array()
        .with_context(|| format!("missing complete {name} records"))
}
fn participant(value: &Value, flags: &Flags) -> Result<()> {
    keys(
        value,
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
    let object = value.as_object().context("participant object")?;
    for field in [
        "displayName",
        "id",
        "isSuperAdmin",
        "isAdmin",
        "lid",
        "phoneNumber",
        "username",
    ] {
        ensure!(
            object.contains_key(field),
            "incomplete participant output {field}"
        );
    }
    ensure!(
        object.contains_key("joinTime") == flags.history
            && object.contains_key("groupHistorySentState") == flags.history,
        "participant history presence differs from source gate"
    );
    if flags.history {
        ensure!(
            value["groupHistorySentState"].is_null()
                || matches!(value["groupHistorySentState"].as_i64(), Some(1..=3)),
            "unqualified normalized history state"
        );
    }
    Ok(())
}
fn validate_action(value: &Value, flags: &Flags, opaque: bool) -> Result<()> {
    let kind = value["actionType"]
        .as_str()
        .context("missing normalized source discriminant")?;
    let fields: &[&str] = match kind {
        "add" | "remove" => &[
            "actionType",
            "participants",
            "reason",
            "isLidAddressingMode",
        ],
        "delete" => &["actionType", "reason", "groupDatas"],
        "promote" | "demote" | "modify" => &["actionType", "participants"],
        "linked_group_promote" | "linked_group_demote" => &["actionType", "participants", "jid"],
        "subject" => &[
            "actionType",
            "subject",
            "s_o",
            "subjectOwnerPn",
            "subjectOwnerUsername",
            "s_t",
        ],
        "desc_add" => &["actionType", "descId", "desc", "descTime"],
        "desc_remove" => &["actionType", "descId"],
        "suspend"
        | "announce"
        | "no_frequently_forwarded"
        | "allow_non_admin_sub_group_creation"
        | "group_safety_check" => &["actionType", "value"],
        "restrict" => &["actionType", "value", "threshold"],
        "invite" => &["actionType", "code"],
        "ephemeral" => &["actionType", "duration", "trigger", "initiatedByMe"],
        "revoke_invite" => &["actionType", "participants"],
        "growth_locked" => &["actionType", "expiration", "type"],
        "growth_unlocked" | "general_chat_auto_add_disabled" => &["actionType"],
        "membership_approval_mode" => &["actionType", "value", "triggered"],
        "membership_approval_request" => &["actionType", "requestMethod", "parentGroupId"],
        "allow_admin_reports" => &[
            "actionType",
            "value",
            "shouldSkipGenMsg",
            "author",
            "triggered",
        ],
        "admin_report_received" => &["actionType", "shouldSkipGenMsg", "value"],
        "created_membership_requests" => &[
            "actionType",
            "requestMethod",
            "parentGroupId",
            "requests",
            "skipGenMsg",
        ],
        "revoked_membership_requests" => &["actionType", "requests"],
        "member_add_mode" => &["actionType", "memberAddMode"],
        other => bail!("unqualified nonraw source action {other}"),
    };
    keys(value, fields)?;
    match kind {
        "add"
        | "remove"
        | "promote"
        | "demote"
        | "modify"
        | "linked_group_promote"
        | "linked_group_demote" => {
            for record in records(value, "participants")? {
                participant(record, flags)?;
            }
        }
        "delete" => {
            for record in records(value, "groupDatas")? {
                keys(record, &["id", "subject"])?;
            }
        }
        "revoke_invite" => {
            for record in records(value, "participants")? {
                keys(record, &["id", "expiration"])?;
            }
        }
        "created_membership_requests" => {
            for record in records(value, "requests")? {
                keys(record, &["wid", "username", "phoneNumber"])?;
                ensure!(
                    flags.username || record.get("username").is_none(),
                    "requested-user username gate differs"
                );
            }
        }
        _ => {}
    }
    if matches!(
        kind,
        "membership_approval_request" | "created_membership_requests"
    ) && !opaque
    {
        ensure!(
            matches!(
                value["requestMethod"].as_str(),
                Some("InviteLink" | "NonAdminAdd" | "LinkedGroupJoin")
            ),
            "requestMethod was not normalized to source enum value"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_matrix_is_complete_and_generates_static_tests() {
        let output = generate_reviewed(ORACLE, "2.3000.1047483476").unwrap();
        assert!(output.contains("MissingRequestFallbackAuthor"));
        assert!(output.contains("projection::project"));
    }
    #[test]
    fn unknown_expected_output_key_rejects_generation() {
        let mut oracle: Value = serde_json::from_str(ORACLE).unwrap();
        let success = oracle["cases"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|c| c.get("success").is_some())
            .unwrap();
        success["success"]["actions"][0]["unexpected_output"] = "unqualified".into();
        assert!(generate_reviewed(&oracle.to_string(), "2.3000.1047483476").is_err());
    }
    #[test]
    fn unknown_error_class_rejects_generation() {
        let mut oracle: Value = serde_json::from_str(ORACLE).unwrap();
        let error = oracle["cases"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|c| c.get("error").is_some())
            .unwrap();
        error["error"]["name"] = "future_error_class".into();
        assert!(generate_reviewed(&oracle.to_string(), "2.3000.1047483476").is_err());
    }
}

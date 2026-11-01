//! Typed consumer assertions against the original groupNotificationParser
//! callback and official XML/Wap/JID/time decoders, independent of envelope IR.
use super::{header, rust_str};
use anyhow::{Context, Result, ensure};
use serde_json::Value;

const ORACLE: &str = include_str!("../../fixtures/group_envelope_oracle.json");
const CALLBACK_SHA: &str = "0c9b28f4c4ac5e27d2032dae378710cb9cc7dd17f9dcbf17769f8962f2ec4fb0";
fn keys(value: &Value, allowed: &[&str]) -> Result<()> {
    for key in value
        .as_object()
        .context("expected envelope oracle object")?
        .keys()
    {
        ensure!(
            allowed.contains(&key.as_str()),
            "unqualified envelope oracle field {key}"
        );
    }
    Ok(())
}
fn node(value: &Value) -> Result<String> {
    keys(value, &["tag", "attrs", "children", "bytes", "string"])?;
    let mut out = format!(
        "NodeBuilder::new({})",
        rust_str(value["tag"].as_str().context("missing oracle tag")?)
    );
    for (key, value) in value["attrs"].as_object().context("missing oracle attrs")? {
        out.push_str(&format!(
            ".attr({},{})",
            rust_str(key),
            rust_str(value.as_str().context("non-string oracle attribute")?)
        ));
    }
    ensure!(
        ["children", "bytes", "string"]
            .iter()
            .filter(|key| value.get(**key).is_some())
            .count()
            <= 1,
        "ambiguous envelope oracle content"
    );
    if let Some(children) = value.get("children") {
        let children = children
            .as_array()
            .context("non-array children")?
            .iter()
            .map(node)
            .collect::<Result<Vec<_>>>()?;
        out.push_str(&format!(".children(vec![{}])", children.join(",")));
    }
    if let Some(bytes) = value.get("bytes") {
        let bytes = bytes
            .as_array()
            .context("non-array bytes")?
            .iter()
            .map(|v| {
                let byte = v.as_u64().context("non-integer byte")?;
                ensure!(byte <= 255, "out-of-range byte");
                Ok(byte.to_string())
            })
            .collect::<Result<Vec<_>>>()?;
        out.push_str(&format!(".bytes(vec![{}])", bytes.join(",")));
    }
    if let Some(text) = value.get("string") {
        out.push_str(&format!(
            ".string_content({})",
            rust_str(text.as_str().context("non-string content")?)
        ));
    }
    out.push_str(".build()");
    Ok(out)
}
fn optional_text(value: &Value) -> Result<String> {
    if value.is_null() {
        return Ok("None".into());
    }
    Ok(format!(
        "Some({})",
        rust_str(value.as_str().context("non-string optional oracle field")?)
    ))
}
fn optional_jid_fact(value: &Value) -> Result<String> {
    if value.is_null() {
        return Ok("None".into());
    }
    Ok(format!(
        "Some({}.parse::<Jid>().unwrap())",
        rust_str(value.as_str().context("non-string local identity fact")?)
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
fn expected_error(case: &Value) -> Result<String> {
    if case["errorName"] == "MissingContextFact" {
        ensure!(
            case["error"] == "MissingContextFact: missing local device LID",
            "unqualified context-fact error"
        );
        return Ok("GroupParseError::MissingContext(\"me_device_lid\")".into());
    }
    ensure!(
        case["errorName"] == "XmppParsingFailure",
        "unqualified envelope exception class"
    );
    let reason = case["errorReason"]
        .as_str()
        .context("missing official parse reason")?;
    if reason.contains("to be <notification>") {
        return Ok("GroupParseError::Tag".into());
    }
    if reason.contains("matching meLid") {
        return Ok("GroupParseError::Destination".into());
    }
    if let Some((_, name)) = reason.split_once("Unrecognized group action ") {
        return Ok(format!(
            "GroupParseError::UnknownAction({}.into())",
            rust_str(name)
        ));
    }
    if reason.contains("children, but has non-children content")
        || reason.contains("to have children")
    {
        return Ok("GroupParseError::Field(\"children\")".into());
    }
    for field in ["from", "participant", "t", "id"] {
        if reason.contains(&format!("attribute \"{field}\""))
            || reason.contains(&format!("\"{field}\"={{"))
        {
            return Ok(format!("GroupParseError::Field({})", rust_str(field)));
        }
    }
    anyhow::bail!("unqualified official envelope error {reason}")
}

pub fn generate(wa_version: &str) -> Result<String> {
    ensure!(
        crate::source::digest(ORACLE.as_bytes())
            == "sha256:2492335fbd9e10446e0c832e15a45365d8e6649e4dcaa2a81e50ad0c69fd792f",
        "envelope oracle differs from qualified capture"
    );
    let oracle: Value = serde_json::from_str(ORACLE)?;
    keys(&oracle, &["waVersion", "setHash", "callback", "cases"])?;
    ensure!(
        wa_version == "2.3000.1052559453" && oracle["waVersion"].as_str() == Some(wa_version),
        "envelope oracle requires snapshot review"
    );
    ensure!(
        oracle["setHash"] == "d6ab7ae601eda281cfac570a68de1d787adde8f4d778f0bb994416f661a21abf",
        "envelope oracle bundle set changed"
    );
    ensure!(
        oracle["callback"]["sha256"] == CALLBACK_SHA
            && oracle["callback"]["startByte"] == 10858
            && oracle["callback"]["endByte"] == 23344,
        "envelope callback requires review"
    );
    let cases = oracle["cases"]
        .as_array()
        .context("missing envelope cases")?;
    ensure!(cases.len() >= 29, "envelope oracle coverage decreased");
    let mut out = header(
        "isolated official group envelope conformance cases",
        wa_version,
    );
    out.push_str(&format!(
        "//! Original callback SHA-256 {CALLBACK_SHA}, module-relative bytes 10858..23344.\n"
    ));
    out.push_str("//! Source accessor traces are checked by the upstream capture verifier; Rust assertions below cover outputs and errors.\n//! Original XML/Wap/JID/time methods run in the fixture harness. Account facts, country/Wid/binary bridges and telemetry are mocked.\n//! Only empty/marker/unknown/suspended actions are exercised here; complex actions have separate conformance cases.\n\nuse super::{parse_normalized_group_notification,GroupParseContext,GroupParseError};\nuse crate::stanza::groups::actions_normalized::NormalizedGroupAction;\nuse wacore_binary::{Jid,builder::NodeBuilder};\n\n#[test]\nfn original_group_envelope_callback_matches_rust_projection() {\n");
    for case in cases {
        keys(
            case,
            &[
                "name",
                "flags",
                "input",
                "success",
                "error",
                "errorName",
                "errorParser",
                "errorReason",
                "telemetryErrors",
                "localLidReads",
                "reads",
            ],
        )?;
        ensure!(
            case["reads"]
                .as_array()
                .is_some_and(|reads| reads.iter().all(|read| read
                    .as_array()
                    .is_some_and(|args| !args.is_empty() && args.iter().all(Value::is_string)))),
            "invalid source accessor trace"
        );
        let name = case["name"]
            .as_str()
            .context("missing envelope case name")?;
        let flags = &case["flags"];
        keys(
            flags,
            &["username", "history", "opus", "meLid", "mePn", "me"],
        )?;
        let username = flags["username"]
            .as_bool()
            .context("missing username flag")?;
        let history = flags["history"].as_bool().context("missing history flag")?;
        let opus = flags["opus"].as_bool().context("missing opus flag")?;
        let me = flags
            .get("me")
            .filter(|value| !value.is_null())
            .map(|v| v.as_str().context("invalid me account fact"))
            .transpose()?;
        out.push_str("{\n");
        out.push_str(&format!(
            "let node={};\nlet me_lid:Option<Jid>={};\nlet me_pn:Option<Jid>={};\n",
            node(&case["input"])?,
            optional_jid_fact(&flags["meLid"])?,
            optional_jid_fact(&flags["mePn"])?
        ));
        out.push_str(&format!(
            "let is_me=|jid:&Jid| {};\n",
            me.map_or("{let _=jid;false}".into(), |value| format!(
                "jid.to_string()=={}",
                rust_str(value)
            ))
        ));
        out.push_str(&format!("let country_enabled={username};let country=|value:&str| {{assert!(country_enabled,\"disabled country lookup\");match value {{\"US\"|\"BR\"=>Some(value.to_owned()),_=>None}}}};\nlet context=GroupParseContext{{me_device_lid:me_lid.as_ref(),me_device_pn:me_pn.as_ref(),username_displayed:{username},history_after_join:{history},opus_enabled:{opus},is_me_account:&is_me,normalize_country_code:&country}};\n"));
        ensure!(
            case.get("success").is_some() != case.get("error").is_some(),
            "ambiguous envelope oracle outcome"
        );
        if case.get("error").is_some() {
            compare(
                &mut out,
                "parse_normalized_group_notification(&node.as_node_ref(),&context).unwrap_err()",
                &expected_error(case)?,
                name,
            );
            out.push_str("}\n");
            continue;
        }
        let expected = &case["success"];
        keys(
            expected,
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
        out.push_str("let parsed=parse_normalized_group_notification(&node.as_node_ref(),&context).unwrap();\n");
        compare(
            &mut out,
            "parsed.external_id.as_str()",
            &rust_str(
                expected["externalId"]
                    .as_str()
                    .context("missing envelope string output")?,
            ),
            &format!("{name}.externalId"),
        );
        compare(
            &mut out,
            "parsed.chat_id.to_string()",
            &rust_str(
                expected["chatId"]
                    .as_str()
                    .context("missing envelope chat id")?,
            ),
            name,
        );
        for (wire, rust) in [
            ("author", "author"),
            ("authorPhoneNumber", "author_phone_number"),
        ] {
            compare(
                &mut out,
                &format!("parsed.{rust}.as_ref().map(ToString::to_string).as_deref()"),
                &optional_text(&expected[wire])?,
                &format!("{name}.{wire}"),
            );
        }
        for (wire, rust) in [
            ("authorUsername", "author_username"),
            ("authorUsernameCountryCode", "author_username_country_code"),
            ("pushname", "pushname"),
            ("offline", "offline"),
        ] {
            compare(
                &mut out,
                &format!("parsed.{rust}.as_deref()"),
                &optional_text(&expected[wire])?,
                &format!("{name}.{wire}"),
            );
        }
        for (wire, rust) in [
            ("isLidAddressingMode", "is_lid_addressing_mode"),
            (
                "hasIncompleteParticipantInformation",
                "has_incomplete_participant_information",
            ),
        ] {
            compare(
                &mut out,
                &format!("parsed.{rust}"),
                &expected[wire]
                    .as_bool()
                    .context("missing envelope boolean output")?
                    .to_string(),
                &format!("{name}.{wire}"),
            );
        }
        compare(
            &mut out,
            "parsed.timestamp",
            &format!(
                "{}_i64",
                expected["ts"]
                    .as_i64()
                    .context("missing envelope time output")?
            ),
            name,
        );
        compare(
            &mut out,
            "parsed.diagnostics.len()",
            &case["telemetryErrors"]
                .as_u64()
                .context("missing oracle telemetry count")?
                .to_string(),
            &format!("{name}.caught PN diagnostics"),
        );
        let actions = expected["actions"]
            .as_array()
            .context("missing envelope actions")?;
        compare(
            &mut out,
            "parsed.actions.len()",
            &actions.len().to_string(),
            &format!("{name}.actions"),
        );
        for (index, action) in actions.iter().enumerate() {
            keys(action, &["actionType", "value"])?;
            ensure!(
                action["actionType"] == "suspend",
                "unqualified envelope-scope action"
            );
            let value = action["value"].as_bool().context("missing suspend value")?;
            let guard = if value { "*value" } else { "!*value" };
            out.push_str(&format!("assert!(matches!(&parsed.actions[{index}],NormalizedGroupAction::Suspend{{value}} if {guard}),{});\n", rust_str(name)));
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
    fn changed_snapshot_requires_requalification() {
        assert!(generate("future").is_err());
    }
    #[test]
    fn emits_source_order_and_recovery_cases() {
        let rust = generate("2.3000.1052559453").unwrap();
        assert!(rust.contains("GroupParseError::Destination"));
        assert!(rust.contains("GroupParseError::MissingContext(\"me_device_lid\")"));
        assert!(rust.contains("parsed.diagnostics.len()"));
        assert!(rust.contains("-2147483647_i64"));
    }
}

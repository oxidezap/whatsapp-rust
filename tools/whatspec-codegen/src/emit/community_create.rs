//! The named-subject, parent-only projection of the upstream Create builder.
//! Group defaults and response correlation stay outside this projection.
use anyhow::{Context, Result, ensure};
use serde_json::Value;

use super::{header, rust_str};
use crate::naming::{ensure_ident, snake_case};

fn path(value: &Value, field: &str) -> Result<Vec<String>> {
    value[field]
        .as_array()
        .context("missing argument path")?
        .iter()
        .map(|s| {
            ensure!(
                s["list"].as_bool() != Some(true),
                "unexpected repeated argument"
            );
            Ok(s["key"]
                .as_str()
                .context("missing argument key")?
                .to_owned())
        })
        .collect()
}
fn binding(value: &Value, field: &str) -> Result<String> {
    Ok(ensure_ident(&snake_case(
        path(value, field)?.last().context("empty argument path")?,
    )))
}
fn tag(value: &Value) -> Result<String> {
    Ok(rust_str(value["tag"].as_str().context("missing wire tag")?))
}
fn keys(value: &Value, allowed: &[&str]) -> Result<()> {
    ensure!(
        value
            .as_object()
            .context("expected contract object")?
            .keys()
            .all(|key| allowed.contains(&key.as_str())),
        "unsupported create contract field"
    );
    Ok(())
}
fn unique(values: &[Value], predicate: impl Fn(&Value) -> bool) -> Result<&Value> {
    let matches: Vec<_> = values.iter().filter(|v| predicate(v)).collect();
    ensure!(
        matches.len() == 1,
        "expected one qualified create contract, got {}",
        matches.len()
    );
    Ok(matches[0])
}

pub fn generate(json: &str, version: &str) -> Result<String> {
    let ir: Value = serde_json::from_str(json)?;
    let stanza = unique(
        ir["stanzas"].as_array().context("missing IQ stanzas")?,
        |s| s["moduleName"] == "WASmaxOutGroupsCreateRequest",
    )?;
    let request = &stanza["request"];
    keys(request, &["namespace", "iqType", "target", "children"])?;
    ensure!(
        request["target"] == "g.us"
            && request["iqType"] == "set"
            && stanza["target"] == request["target"]
            && stanza["namespace"] == request["namespace"]
            && stanza["iqType"] == request["iqType"],
        "create envelope changed"
    );
    let roots = request["children"]
        .as_array()
        .context("missing create root")?;
    ensure!(roots.len() == 1, "create needs one root");
    let create = &roots[0];
    keys(
        create,
        &["tag", "attrs", "children", "repeats", "variantGroups"],
    )?;
    ensure!(
        create["repeats"] == false && create["content"].is_null(),
        "unsupported create root"
    );
    ensure!(
        create["attrs"]
            .as_array()
            .context("missing create attributes")?
            .iter()
            .all(|a| a["required"] == false),
        "new required create attribute needs review"
    );
    let groups = create["variantGroups"]
        .as_array()
        .context("missing subject disjunction")?;
    ensure!(
        groups.len() == 1 && groups[0]["optional"].as_bool() != Some(true),
        "subject disjunction changed"
    );
    keys(&groups[0], &["variants", "optional"])?;
    let variants = groups[0]["variants"]
        .as_array()
        .context("missing subject variants")?;
    ensure!(variants.len() == 2, "subject variants changed");
    let named = unique(variants, |v| v["attrs"][0]["required"] == true)?;
    keys(named, &["attrs", "children"])?;
    ensure!(
        named["attrs"].as_array().is_some_and(|a| a.len() == 1)
            && named["children"].as_array().is_none_or(|a| a.is_empty()),
        "named subject shape changed"
    );
    let subject = &named["attrs"][0];
    keys(subject, &["name", "kind", "required", "argPath"])?;
    ensure!(
        subject["kind"] == "string"
            && path(subject, "argPath")?
                == [
                    "namedSubjectOrUnnamedSubjectFallbackMixinGroupArgs",
                    "namedSubject",
                    "anySubject"
                ],
        "named subject binding changed"
    );
    let children = create["children"]
        .as_array()
        .context("missing create children")?;
    let parent = unique(children, |n| {
        n["argPath"] == serde_json::json!([{"key":"parentArgs"}])
    })?;
    ensure!(
        parent["presence"] == "optional"
            && parent["repeats"] == false
            && parent["children"] == serde_json::json!([])
            && parent["attrs"].as_array().is_some_and(|a| a.len() == 1),
        "parent shape changed"
    );
    let approval = &parent["attrs"][0];
    keys(
        parent,
        &["tag", "attrs", "children", "repeats", "presence", "argPath"],
    )?;
    keys(
        approval,
        &["name", "kind", "value", "required", "presenceArgPath"],
    )?;
    ensure!(
        approval["kind"] == "const"
            && approval["required"] == false
            && approval["value"].is_string()
            && path(approval, "presenceArgPath")?
                == ["parentArgs", "hasParentGroupDefaultMembershipApprovalMode"],
        "parent mode binding changed"
    );
    let subject_arg = binding(subject, "argPath")?;
    let approval_arg = binding(approval, "presenceArgPath")?;
    let mut fields = vec![
        format!("pub(super) {subject_arg}: &'a str"),
        format!("pub(super) {approval_arg}: Option<()>"),
    ];
    let mut body = String::from("let mut children = Vec::new();\n");
    for child in children {
        if std::ptr::eq(child, parent) {
            body.push_str(&format!("let mut parent = NodeBuilder::new({});\nif request.{approval_arg}.is_some() {{ parent = parent.attr({}, {}); }}\nchildren.push(parent.build());\n", tag(parent)?, rust_str(approval["name"].as_str().context("missing mode attribute")?), rust_str(approval["value"].as_str().context("missing mode literal")?)));
        } else if [
            serde_json::json!([{"key":"hasAllowNonAdminSubGroupCreation"}]),
            serde_json::json!([{"key":"hasCreateGeneralChat"}]),
        ]
        .contains(&child["argPath"])
        {
            keys(
                child,
                &["tag", "attrs", "children", "repeats", "presence", "argPath"],
            )?;
            ensure!(
                child["presence"] == "presence_flag"
                    && child["attrs"] == serde_json::json!([])
                    && child["children"] == serde_json::json!([])
                    && child["repeats"] == false,
                "unsupported community flag"
            );
            let arg = binding(child, "argPath")?;
            fields.push(format!("pub(super) {arg}: bool"));
            body.push_str(&format!(
                "if request.{arg} {{ children.push(NodeBuilder::new({}).build()); }}\n",
                tag(child)?
            ));
        } else {
            // This projection supplies no participants, description or group settings.
            ensure!(
                child["presence"] == "optional"
                    || child["presence"] == "presence_flag"
                    || (child["repeats"] == true && child["repeatMin"] == 0),
                "new required create child needs review"
            );
        }
    }
    ensure!(fields.len() == 4, "missing community flags");
    ensure!(fields[2] != fields[3], "duplicate community flag");
    let namespace = rust_str(request["namespace"].as_str().context("missing namespace")?);
    Ok(format!(
        "{}use crate::request::InfoQuery;\nuse wacore_binary::{{Jid, NodeContent, builder::NodeBuilder, jid::Server}};\n\npub(super) struct CommunityCreateRequest<'a> {{\n{}\n}}\n\npub(super) fn build_community_create(request: CommunityCreateRequest<'_>) -> InfoQuery<'static> {{\n{body}InfoQuery::set({namespace}, Jid::new(\"\", Server::Group), Some(NodeContent::Nodes(vec![NodeBuilder::new({}).attr({}, request.{subject_arg}).children(children).build()])))\n}}\n",
        header("community Create request projection", version),
        fields.join(",\n"),
        tag(create)?,
        rust_str(
            subject["name"]
                .as_str()
                .context("missing subject attribute")?
        )
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    const FIXTURE: &str = include_str!("../../tests/fixtures/community_create.json");
    #[test]
    fn request_wire_names_and_presence_come_from_ir() {
        let mut ir: Value = serde_json::from_str(FIXTURE).unwrap();
        let output = generate(FIXTURE, "test").unwrap();
        assert!(output.contains("request_required"));
        assert!(output.contains("has_parent_group_default_membership_approval_mode: Option<()>"));
        assert!(!output.contains("member_add_mode"));
        ir["stanzas"][0]["request"]["children"][0]["tag"] = "renamed_create".into();
        let children = ir["stanzas"][0]["request"]["children"][0]["children"]
            .as_array_mut()
            .unwrap();
        let parent = children.iter_mut().find(|c| c["tag"] == "parent").unwrap();
        parent["tag"] = "renamed_parent".into();
        parent["attrs"][0]["value"] = "renamed_mode".into();
        let output = generate(&ir.to_string(), "test").unwrap();
        for value in ["renamed_create", "renamed_parent", "renamed_mode"] {
            assert!(output.contains(value));
        }
    }
    #[test]
    fn missing_or_changed_contracts_fail_closed() {
        for mutate in [0, 1, 2, 3] {
            let mut ir: Value = serde_json::from_str(FIXTURE).unwrap();
            let request = &mut ir["stanzas"][0]["request"];
            match mutate {
                0 => request["target"] = "group_jid".into(),
                1 => request["children"][0]["variantGroups"] = serde_json::json!([]),
                2 => {
                    request["children"][0]["children"]
                        .as_array_mut()
                        .unwrap()
                        .push(serde_json::json!({"tag":"new_required","repeats":false}));
                }
                _ => {
                    let parent = request["children"][0]["children"]
                        .as_array_mut()
                        .unwrap()
                        .iter_mut()
                        .find(|c| c["tag"] == "parent")
                        .unwrap();
                    parent["attrs"][0]["presenceArgPath"] = Value::Null;
                }
            }
            assert!(generate(&ir.to_string(), "test").is_err());
        }
    }
}

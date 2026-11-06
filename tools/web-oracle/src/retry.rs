//! Native Rust corpus and explicit mocks for the two original metadata modules.
//! No media upload, retry eligibility, account or authenticated client is run.
use crate::{engine::*, inputs};
use anyhow::{Result, ensure};
use boa_engine::{
    JsArgs, JsNativeError, JsResult, JsSymbol, JsValue, js_string,
    object::{FunctionObjectBuilder, builtins::JsProxy},
};
use serde_json::{Value, json};
use std::path::Path;
fn enum_cast(_: &JsValue, args: &[JsValue], cx: &mut Cx) -> JsResult<JsValue> {
    let value = args.get_or_undefined(0).clone();
    let cast = function(cx, "cast", identity);
    set(&value, "cast", cast, cx)?;
    Ok(value)
}
fn invalid(this: &JsValue, args: &[JsValue], cx: &mut Cx) -> JsResult<JsValue> {
    let message = if args.get_or_undefined(0).is_undefined() {
        String::new()
    } else {
        args[0].to_string(cx)?.to_std_string_escaped()
    };
    let error: JsValue = JsNativeError::error()
        .with_message(message)
        .into_opaque(cx)
        .into();
    set(&error, "__oracle_invalid_metadata", true.into(), cx)?;
    if this.is_object() {
        let p = get(this, "prototype", cx)?;
        if p.is_object() {
            obj(&error)?.set_prototype(Some(obj(&p)?));
        }
    }
    Ok(error)
}
fn type_get(_: &JsValue, args: &[JsValue], cx: &mut Cx) -> JsResult<JsValue> {
    let v =
        obj(args.get_or_undefined(0))?.get(args.get_or_undefined(1).to_property_key(cx)?, cx)?;
    Ok(if v.is_null_or_undefined() {
        args.get_or_undefined(1).clone()
    } else {
        v
    })
}
fn wap(_: &JsValue, args: &[JsValue], cx: &mut Cx) -> JsResult<JsValue> {
    let attrs = args.get_or_undefined(1);
    let drop = cx.global_object().get(js_string!("__oracle_drop"), cx)?;
    let output: JsValue = object(cx).into();
    for key in obj(attrs)?.own_property_keys(cx)? {
        let value = obj(attrs)?.get(key.clone(), cx)?;
        if !JsValue::same_value(&value, &drop) {
            obj(&output)?.set(key, value, true, cx)?;
        }
    }
    let node: JsValue = object(cx).into();
    set(&node, "tag", args.get_or_undefined(0).clone(), cx)?;
    set(&node, "attrs", output, cx)?;
    Ok(node)
}
fn stubs(cx: &mut Cx) -> JsResult<()> {
    let internal = function(cx, "InternalEnum", enum_cast);
    stub("$InternalEnum", internal, cx)?;
    let invalid =
        FunctionObjectBuilder::new(cx.realm(), boa_engine::NativeFunction::from_fn_ptr(invalid))
            .name(js_string!("InvalidMessageTypes"))
            .constructor(true)
            .build();
    let prototype: JsValue = object(cx).into();
    let error = cx.global_object().get(js_string!("Error"), cx)?;
    obj(&prototype)?.set_prototype(Some(obj(&get(&error, "prototype", cx)?)?));
    let inv: JsValue = invalid.into();
    set(&inv, "prototype", prototype, cx)?;
    let errors: JsValue = object(cx).into();
    set(&errors, "MessageProtobufInvalidMessageTypes", inv, cx)?;
    stub("WAWebHandleMsgError", errors, cx)?;
    for (module, key) in [
        ("justknobx", "_"),
        ("WAWebCurrentUser", "isEmployee"),
        ("WAWebBotUtils", "isMetaAiBot"),
    ] {
        let value: JsValue = object(cx).into();
        let f = function(cx, key, false_fn);
        set(&value, key, f, cx)?;
        stub(module, value, cx)?;
    }
    let disabled = function(cx, "disabledPollSnapshot", false_fn);
    stub(
        "WAWebPollResultSnapshotPollTypeEnvelopeEnabled",
        disabled,
        cx,
    )?;
    for (name, value) in [
        (
            "WAWebCommonMsgSubtypeTypes",
            json!({"MsgSubtype":{"EphemeralSyncResponse":"ephemeral_sync_response"}}),
        ),
        (
            "WAWebUsernameTypes",
            json!({"LidOriginType":{"PNH_CTWA":"pnh_ctwa"}}),
        ),
        (
            "WAWebHandleMsgCommon",
            json!({"POLL_TYPES":{"creation":"creation","vote":"vote","result_snapshot":"result_snapshot","edit":"edit"},"EVENT_TYPES":{"creation":"creation","response":"response","edit":"edit"}}),
        ),
        (
            "WAWebProtobufsE2E.pb",
            json!({"Message$SecretEncryptedMessage$SecretEncType":{"EVENT_EDIT":1,"POLL_EDIT":3}}),
        ),
    ] {
        let value = JsValue::from_json(&value, cx)?;
        stub(name, value, cx)?;
    }
    let target = JsValue::from_json(
        &json!({"PROTOCOL":"protocol","RICH_RESPONSE":"rich_response","MESSAGE_HISTORY_NOTICE":"history"}),
        cx,
    )?;
    let proxy: JsValue = JsProxy::builder(obj(&target)?)
        .get(type_get)
        .build(cx)?
        .into();
    let types: JsValue = object(cx).into();
    set(&types, "MSG_TYPE", proxy, cx)?;
    stub("WAWebMsgType", types, cx)?;
    let drop: JsValue = JsSymbol::new(Some(js_string!("drop")))
        .ok_or_else(|| JsNativeError::error().with_message("symbol allocation"))?
        .into();
    cx.register_global_property(
        js_string!("__oracle_drop"),
        drop.clone(),
        boa_engine::property::Attribute::all(),
    )?;
    let w: JsValue = object(cx).into();
    set(&w, "DROP_ATTR", drop, cx)?;
    let custom = function(cx, "CUSTOM_STRING", identity);
    set(&w, "CUSTOM_STRING", custom, cx)?;
    let builder = function(cx, "wap", wap);
    set(&w, "wap", builder, cx)?;
    stub("WAWap", w, cx)?;
    Ok(())
}
fn is_lid(cx: &mut Cx) -> JsValue {
    function(cx, "isLid", false_fn)
}
fn append(
    kind: &str,
    flag: Value,
    wrappers: &[&str],
    module: &JsValue,
    cx: &mut Cx,
    output: &mut Vec<Value>,
) -> JsResult<()> {
    let media = matches!(kind, "image" | "video" | "audio" | "ptv");
    let mut proto = match kind {
        "image" | "video" | "audio" | "ptv" => {
            let value = if flag.is_null() {
                json!({})
            } else {
                json!({"viewOnce":flag})
            };
            let mut m = serde_json::Map::new();
            m.insert(format!("{kind}Message"), value);
            Value::Object(m)
        }
        "text" => json!({"conversation":"synthetic"}),
        "extended_text" => json!({"extendedTextMessage":{"viewOnce":flag}}),
        "buttons" => json!({"buttonsMessage":{}}),
        "interactive" => {
            json!({"interactiveMessage":{"header":{"imageMessage":{"viewOnce":true}}}})
        }
        "quoted" => {
            json!({"imageMessage":{"contextInfo":{"quotedMessage":{"viewOnceMessage":{"message":{"imageMessage":{}}}}}}})
        }
        "poll_v1" => json!({"pollCreationMessage":{}}),
        "poll_v2" => json!({"pollCreationMessageV2":{}}),
        "poll_v3" => json!({"pollCreationMessageV3":{}}),
        "poll_vote" => json!({"pollUpdateMessage":{"vote":{}}}),
        "poll_empty_vote" => json!({"pollUpdateMessage":{}}),
        "poll_snapshot" => json!({"pollResultSnapshotMessage":{}}),
        "event" => json!({"eventMessage":{}}),
        "event_response" => json!({"encEventResponseMessage":{}}),
        "event_edit" => json!({"secretEncryptedMessage":{"secretEncType":1}}),
        "member_label" => json!({"protocolMessage":{}}),
        "empty" => json!({}),
        _ => {
            return Err(JsNativeError::error()
                .with_message("unknown corpus kind")
                .into());
        }
    };
    for wrapper in wrappers.iter().rev() {
        let key = match *wrapper {
            "dsm" => "deviceSentMessage",
            "ephemeral" => "ephemeralMessage",
            "v1" => "viewOnceMessage",
            "v2" => "viewOnceMessageV2",
            "v2ext" => "viewOnceMessageV2Extension",
            _ => {
                return Err(JsNativeError::error()
                    .with_message("unknown wrapper")
                    .into());
            }
        };
        let mut map = serde_json::Map::new();
        map.insert(
            key.into(),
            if kind == "empty" {
                json!({})
            } else {
                json!({"message":proto})
            },
        );
        proto = Value::Object(map);
    }
    let mut data = json!({"type":if kind=="member_label" {"protocol"} else {"ordinary"}});
    if media {
        data["mediaData"] =
            json!({"isViewOnce":flag==true || wrappers.iter().any(|w|w.starts_with('v'))});
    }
    if kind == "quoted" {
        data["mediaData"] = json!({"isViewOnce":false});
    }
    if kind == "member_label" {
        data["subtype"] = json!("member_label");
        data["memberLabelData"] = json!({"label":if flag==true {"synthetic"} else {""}});
    }
    let input = JsValue::from_json(
        &json!({"msgProtobuf":proto,"msgRecord":{"type":"message","data":data}}),
        cx,
    )?;
    let chat: JsValue = object(cx).into();
    let lid = is_lid(cx);
    set(&chat, "isLid", lid, cx)?;
    set(&input, "chatId", chat, cx)?;
    let (invalid, expected) = match method(module, "genMetaNode", &[input], cx) {
        Ok(v) => {
            let attrs = if v.is_null_or_undefined() {
                None
            } else {
                get(&v, "attrs", cx)?.to_json(cx)?
            };
            (false, attrs.unwrap_or_else(|| json!({})))
        }
        Err(error) => {
            let error = error.into_opaque(cx)?;
            if !get(&error, "__oracle_invalid_metadata", cx)?.to_boolean() {
                return Err(boa_engine::JsError::from_opaque(error));
            }
            (true, json!({}))
        }
    };
    output.push(
        json!({"kind":kind,"flag":flag,"wrappers":wrappers,"invalid":invalid,"expected":expected}),
    );
    Ok(())
}
pub fn run(root: &Path, lock: &Path, check: Option<&Path>) -> Result<()> {
    let paths = inputs::verify(root, lock)?;
    let pins = [
        (
            "WAWebVerifyProtobufMsgObjectKeys",
            "430cb8979511aada95178605faa35a8483446f677ce6d1fe0fd66333c7879eee",
            434797,
            442805,
        ),
        (
            "WAWebSendMsgMetaNode",
            "f588ee2f033b87e018a3be34a37ec1347d8b6b8700de539e6be31481b29c4690",
            985968,
            990146,
        ),
    ];
    let mut cx = context().map_err(|e| anyhow::anyhow!("{e}"))?;
    stubs(&mut cx).map_err(|e| anyhow::anyhow!("{e}"))?;
    for (name, hash, start, end) in pins {
        let mut source = None;
        for bundle in &paths {
            if inputs::sha(&bundle.bytes) == hash {
                source = Some(&bundle.bytes);
                break;
            }
        }
        let bytes = source.ok_or_else(|| anyhow::anyhow!("missing pinned retry bundle {name}"))?;
        let span = bytes
            .get(start..end)
            .ok_or_else(|| anyhow::anyhow!("invalid module range"))?;
        let modules = inputs::modules(&paths, &[name])?;
        ensure!(
            modules[name] == span,
            "pinned module range differs from AST definition"
        );
        evaluate(span, &mut cx).map_err(|e| anyhow::anyhow!("{e}"))?;
    }
    let module = load("WAWebSendMsgMetaNode", &mut cx).map_err(|e| anyhow::anyhow!("{e}"))?;
    let mut output = vec![];
    (|| -> JsResult<()> {
        for kind in ["image", "video", "audio", "ptv"] {
            for flag in [json!(true), json!(false), Value::Null] {
                append(kind, flag, &[], &module, &mut cx, &mut output)?;
            }
            for w in ["v1", "v2", "v2ext"] {
                append(
                    kind,
                    Value::Null,
                    &["dsm", "ephemeral", w],
                    &module,
                    &mut cx,
                    &mut output,
                )?;
            }
        }
        for kind in ["text", "extended_text", "buttons", "interactive", "empty"] {
            for wrappers in [vec![], vec!["v1"]] {
                append(kind, json!(true), &wrappers, &module, &mut cx, &mut output)?;
            }
        }
        for kind in [
            "quoted",
            "poll_v1",
            "poll_v2",
            "poll_v3",
            "poll_vote",
            "poll_empty_vote",
            "poll_snapshot",
            "event",
            "event_response",
            "event_edit",
        ] {
            append(kind, Value::Null, &[], &module, &mut cx, &mut output)?;
        }
        append(
            "event",
            Value::Null,
            &["dsm", "ephemeral"],
            &module,
            &mut cx,
            &mut output,
        )?;
        append(
            "member_label",
            json!(true),
            &[],
            &module,
            &mut cx,
            &mut output,
        )?;
        append(
            "member_label",
            json!(false),
            &[],
            &module,
            &mut cx,
            &mut output,
        )?;
        Ok(())
    })()
    .map_err(|e| anyhow::anyhow!("{e}"))?;
    let rendered = format!(
        "[\n{}\n]\n",
        output
            .iter()
            .map(|v| serde_json::to_string(v).map(|s| format!("  {s}")))
            .collect::<std::result::Result<Vec<_>, _>>()?
            .join(",\n")
    );
    if let Some(path) = check {
        ensure!(
            rendered == std::fs::read_to_string(path)?,
            "retry metadata fixture differs"
        );
        println!("{} original metadata cases match", output.len());
    } else {
        print!("{rendered}");
    }
    Ok(())
}

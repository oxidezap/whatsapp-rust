//! Auto-generated ordered group envelope guards (WhatsApp 2.3000.1052559453). DO NOT EDIT.
//!
//! Source contract <https://github.com/oxidezap/whatspec/blob/ae3880925bacd23863cc30ec480c9f82bfd869f1/generated/notif/group-envelope.json>
//! WAWebHandleGroupNotification callback bytes 10858..23344; child action boundary 12132..23052.
//! Actions are supplied by the normalized action parser between prefix and projection.
use super::{
    Binary, Conversion, Diagnostic, Fact, Read, State, Value, binary, context, convert, is_empty,
    read, reject,
};
use super::{GroupParseContext, GroupParseError, NodeRef};
pub(in crate::stanza::groups) const MISSING_IDENTIFICATION_TAG: &str =
    "missing_participant_identification";
pub(in crate::stanza::groups) const UNKNOWN_ACTION_ERROR_PREFIX: &str =
    "Unrecognized group action ";
pub(in crate::stanza::groups) fn action_facts(state: &State) -> Result<Value, GroupParseError> {
    Ok(Value::Object(vec![
        ("chatId", state.local(6)?),
        ("author", state.local(7)?),
        ("isLidAddressingMode", state.local(12)?),
        ("timestamp", state.local(11)?),
    ]))
}
pub(in crate::stanza::groups) fn parse_prefix(
    node: &NodeRef<'_>,
    context_value: &GroupParseContext<'_>,
) -> Result<State, GroupParseError> {
    let mut state = State::new(17, 15, 16);
    if ({
        let _ = read(node, Read::AssertTag, "notification")?;
        read(node, Read::HasAttr, "to")?
    })
    .truthy()
    {
        let envelope_tmp_0 = read(node, Read::AttrString, "to")?;
        let _ = state.assign(1, envelope_tmp_0)?;
        let envelope_tmp_1 = convert(
            context_value,
            Conversion::ToJid,
            context(context_value, Fact::MeDeviceLid)?,
        )?;
        let _ = state.assign(2, envelope_tmp_1)?;
        let envelope_tmp_3 = if (binary(
            Binary::NullishEqual,
            {
                let envelope_tmp_2 = context(context_value, Fact::MaybeMeDevicePn)?;
                state.assign(0, envelope_tmp_2)?
            },
            Value::Null,
        )?)
        .truthy()
        {
            Value::Undefined
        } else {
            convert(context_value, Conversion::ToJid, state.local(0)?)?
        };
        let _ = state.assign(3, envelope_tmp_3)?;
        let envelope_tmp_4 = binary(Binary::StrictEqual, state.local(1)?, state.local(2)?)?;
        let _ = state.assign(4, envelope_tmp_4)?;
        let envelope_tmp_6 = {
            let envelope_tmp_5 = Value::Bool(!(is_empty(state.local(3)?)).truthy());
            if envelope_tmp_5.truthy() {
                binary(Binary::StrictEqual, state.local(1)?, state.local(3)?)?
            } else {
                envelope_tmp_5
            }
        };
        let _ = state.assign(5, envelope_tmp_6)?;
        let _ = {
            let envelope_tmp_8 = {
                let envelope_tmp_7 = Value::Bool(!(state.local(4)?).truthy());
                if envelope_tmp_7.truthy() {
                    Value::Bool(!(state.local(5)?).truthy())
                } else {
                    envelope_tmp_7
                }
            };
            if envelope_tmp_8.truthy() {
                reject(binary(
                    Binary::Add,
                    binary(
                        Binary::Add,
                        binary(
                            Binary::Add,
                            binary(
                                Binary::Add,
                                binary(
                                    Binary::Add,
                                    binary(
                                        Binary::Add,
                                        Value::String("to have \"to\" matching meLid (\"".into()),
                                        state.local(2)?,
                                    )?,
                                    Value::String("\") or mePn (\"".into()),
                                )?,
                                if (binary(Binary::NullishNotEqual, state.local(3)?, Value::Null)?)
                                    .truthy()
                                {
                                    state.local(3)?
                                } else {
                                    Value::String("".into())
                                },
                            )?,
                            Value::String("\"), but instead has \"".into()),
                        )?,
                        state.local(1)?,
                    )?,
                    Value::String("\"".into()),
                )?)?
            } else {
                envelope_tmp_8
            }
        };
    }
    let envelope_tmp_9 = convert(
        context_value,
        Conversion::GroupJidToWid,
        read(node, Read::AttrGroupJid, "from")?,
    )?;
    let _ = state.assign(6, envelope_tmp_9)?;
    let envelope_tmp_10 = if (read(node, Read::HasAttr, "participant")?).truthy() {
        convert(
            context_value,
            Conversion::UserJidToWid,
            read(node, Read::AttrUserJid, "participant")?,
        )?
    } else {
        Value::Null
    };
    let _ = state.assign(7, envelope_tmp_10)?;
    let envelope_tmp_11 = Value::Null;
    let _ = state.assign(8, envelope_tmp_11)?;
    let envelope_tmp_14 = (|| -> Result<(), GroupParseError> {
        let envelope_tmp_12 = read(node, Read::MaybeAttrPhoneUserJid, "participant_pn")?;
        let _ = state.assign(9, envelope_tmp_12)?;
        let _ = {
            let envelope_tmp_13 =
                if (binary(Binary::NullishNotEqual, state.local(9)?, Value::Null)?).truthy() {
                    convert(context_value, Conversion::UserJidToWid, state.local(9)?)?
                } else {
                    Value::Null
                };
            state.assign(8, envelope_tmp_13)?
        };
        Ok(())
    })();
    if let Err(error) = envelope_tmp_14 {
        state.diagnostics.push(Diagnostic {
            message_parts: &["Known error T150827746: ", ""],
            error,
        });
    }
    let envelope_tmp_15 = context(context_value, Fact::UsernameDisplayed)?;
    let _ = state.assign(10, envelope_tmp_15)?;
    let envelope_tmp_16 = read(node, Read::AttrTime, "t")?;
    let _ = state.assign(11, envelope_tmp_16)?;
    let envelope_tmp_17 = if (read(node, Read::HasAttr, "addressing_mode")?).truthy() {
        binary(
            Binary::StrictEqual,
            read(node, Read::AttrString, "addressing_mode")?,
            Value::String("lid".into()),
        )?
    } else {
        Value::Bool(!(Value::Integer(1)).truthy())
    };
    let _ = state.assign(12, envelope_tmp_17)?;
    let envelope_tmp_19 = if ({
        let envelope_tmp_18 = read(node, Read::HasAttr, "participant_username")?;
        if envelope_tmp_18.truthy() {
            state.local(10)?
        } else {
            envelope_tmp_18
        }
    })
    .truthy()
    {
        read(node, Read::AttrString, "participant_username")?
    } else {
        Value::Null
    };
    let _ = state.assign(13, envelope_tmp_19)?;
    let envelope_tmp_21 = if ({
        let envelope_tmp_20 = read(node, Read::HasAttr, "participant_country_code")?;
        if envelope_tmp_20.truthy() {
            state.local(10)?
        } else {
            envelope_tmp_20
        }
    })
    .truthy()
    {
        convert(
            context_value,
            Conversion::AsIsoCountryCode,
            read(node, Read::AttrString, "participant_country_code")?,
        )?
    } else {
        Value::Null
    };
    let _ = state.assign(14, envelope_tmp_21)?;
    Ok(state)
}
pub(in crate::stanza::groups) fn finish_projection(
    node: &NodeRef<'_>,
    context_value: &GroupParseContext<'_>,
    state: &mut State,
) -> Result<Value, GroupParseError> {
    let _ = context_value;
    state.require_actions()?;
    Ok(Value::Object(vec![
        ("externalId", read(node, Read::AttrString, "id")?),
        ("chatId", state.local(6)?),
        ("isLidAddressingMode", state.local(12)?),
        ("author", state.local(7)?),
        ("authorPhoneNumber", state.local(8)?),
        ("authorUsername", state.local(13)?),
        ("authorUsernameCountryCode", state.local(14)?),
        ("ts", read(node, Read::AttrTime, "t")?),
        ("pushname", read(node, Read::MaybeAttrString, "notify")?),
        ("offline", read(node, Read::MaybeAttrString, "offline")?),
        ("actions", state.local(16)?),
        ("hasIncompleteParticipantInformation", {
            let envelope_tmp_22 = state.local(10)?;
            if envelope_tmp_22.truthy() {
                binary(
                    Binary::StrictEqual,
                    state.local(15)?,
                    Value::Bool(!(Value::Integer(0)).truthy()),
                )?
            } else {
                envelope_tmp_22
            }
        }),
    ]))
}

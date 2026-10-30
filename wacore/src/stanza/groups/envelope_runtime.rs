//! Small typed operations used by the statically generated envelope program.
//! This evaluates no JSON or JavaScript at runtime. JID and integer decoding
//! share the source-qualified accessors used by normalized group actions.
use super::identity;
use super::normalized::{GroupParseContext, GroupParseError};
use wacore_binary::{Jid, NodeRef};
#[path = "envelope_generated.rs"]
mod generated;
pub(super) use generated::{
    MISSING_IDENTIFICATION_TAG, UNKNOWN_ACTION_ERROR_PREFIX, action_facts, finish_projection,
    parse_prefix,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum Value {
    Undefined,
    Null,
    Bool(bool),
    String(String),
    Integer(i64),
    Jid(Jid),
    Object(Vec<(&'static str, Value)>),
    Actions,
}
impl Value {
    pub(super) fn truthy(&self) -> bool {
        match self {
            Self::Undefined | Self::Null => false,
            Self::Bool(v) => *v,
            Self::String(v) => !v.is_empty(),
            Self::Integer(v) => *v != 0,
            _ => true,
        }
    }
    pub(super) fn is_nullish(&self) -> bool {
        matches!(self, Self::Undefined | Self::Null)
    }
    pub(super) fn field(&self, name: &str) -> Option<&Self> {
        let Self::Object(fields) = self else {
            return None;
        };
        fields
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value)
    }
    pub(super) fn as_jid(&self) -> Option<&Jid> {
        if let Self::Jid(v) = self {
            Some(v)
        } else {
            None
        }
    }
    pub(super) fn as_string(&self) -> Option<&str> {
        if let Self::String(v) = self {
            Some(v)
        } else {
            None
        }
    }
    pub(super) fn as_integer(&self) -> Option<i64> {
        if let Self::Integer(v) = self {
            Some(*v)
        } else {
            None
        }
    }
    pub(super) fn as_bool(&self) -> Option<bool> {
        if let Self::Bool(v) = self {
            Some(*v)
        } else {
            None
        }
    }
}
#[derive(Debug, Clone)]
pub(super) struct Diagnostic {
    pub message_parts: &'static [&'static str],
    pub error: GroupParseError,
}
#[derive(Debug, Clone)]
pub(super) struct State {
    locals: Vec<Value>,
    marker_local: usize,
    actions_local: usize,
    pub diagnostics: Vec<Diagnostic>,
}
impl State {
    pub(super) fn new(locals: usize, marker_local: usize, actions_local: usize) -> Self {
        Self {
            locals: vec![Value::Undefined; locals],
            marker_local,
            actions_local,
            diagnostics: vec![],
        }
    }
    pub(super) fn local(&self, local: usize) -> Result<Value, GroupParseError> {
        self.locals
            .get(local)
            .cloned()
            .ok_or_else(|| GroupParseError::Rejected("invalid generated envelope local".into()))
    }
    pub(super) fn assign(&mut self, local: usize, value: Value) -> Result<Value, GroupParseError> {
        let target = self
            .locals
            .get_mut(local)
            .ok_or_else(|| GroupParseError::Rejected("invalid generated envelope local".into()))?;
        *target = value.clone();
        Ok(value)
    }
    pub(super) fn mark_missing(&mut self) -> Result<(), GroupParseError> {
        self.assign(self.marker_local, Value::Bool(true))
            .map(|_| ())
    }
    pub(super) fn mark_actions(&mut self) -> Result<(), GroupParseError> {
        self.assign(self.actions_local, Value::Actions).map(|_| ())
    }
    fn require_actions(&self) -> Result<(), GroupParseError> {
        if matches!(self.local(self.actions_local)?, Value::Actions) {
            Ok(())
        } else {
            Err(GroupParseError::MissingContext("normalized_actions"))
        }
    }
}

pub(super) fn read(
    node: &NodeRef<'_>,
    method: Read,
    name: &'static str,
) -> Result<Value, GroupParseError> {
    if method == Read::AssertTag {
        return if node.tag.as_ref() == name {
            Ok(Value::Undefined)
        } else {
            Err(GroupParseError::Tag)
        };
    }
    if method == Read::HasAttr {
        return Ok(Value::Bool(node.get_attr(name).is_some()));
    }
    let Some(raw) = node.get_attr(name) else {
        return if matches!(method, Read::MaybeAttrPhoneUserJid | Read::MaybeAttrString) {
            Ok(Value::Null)
        } else {
            Err(GroupParseError::Field(name))
        };
    };
    let text = raw.as_str();
    match method {
        Read::AttrString | Read::MaybeAttrString => Ok(Value::String(text.into_owned())),
        Read::AttrGroupJid => identity::parse_group(&text)
            .map(Value::Jid)
            .ok_or(GroupParseError::Field(name)),
        Read::AttrUserJid => identity::parse_user(&text)
            .map(Value::Jid)
            .ok_or(GroupParseError::Field(name)),
        Read::MaybeAttrPhoneUserJid => identity::parse_phone(&text)
            .map(Value::Jid)
            .ok_or(GroupParseError::Field(name)),
        Read::AttrTime => identity::parse_js_time(&text)
            .map(Value::Integer)
            .map_err(|error| match error {
                identity::IntegerReadError::NotInteger => GroupParseError::Field(name),
                identity::IntegerReadError::OutsideExactRange => {
                    GroupParseError::NumericRange(name)
                }
            }),
        Read::AssertTag | Read::HasAttr => Err(GroupParseError::Rejected(
            "invalid generated envelope read".into(),
        )),
    }
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Read {
    AssertTag,
    HasAttr,
    AttrString,
    AttrGroupJid,
    AttrUserJid,
    MaybeAttrPhoneUserJid,
    AttrTime,
    MaybeAttrString,
}
#[derive(Clone, Copy)]
pub(super) enum Fact {
    MeDeviceLid,
    MaybeMeDevicePn,
    UsernameDisplayed,
}
pub(super) fn context(
    context: &GroupParseContext<'_>,
    fact: Fact,
) -> Result<Value, GroupParseError> {
    match fact {
        Fact::MeDeviceLid => context
            .me_device_lid
            .cloned()
            .map(Value::Jid)
            .ok_or(GroupParseError::MissingContext("me_device_lid")),
        Fact::MaybeMeDevicePn => Ok(context
            .me_device_pn
            .cloned()
            .map_or(Value::Undefined, Value::Jid)),
        Fact::UsernameDisplayed => Ok(Value::Bool(context.username_displayed)),
    }
}
#[derive(Clone, Copy)]
pub(super) enum Conversion {
    GroupJidToWid,
    UserJidToWid,
    AsIsoCountryCode,
    ToJid,
}
pub(super) fn convert(
    context: &GroupParseContext<'_>,
    conversion: Conversion,
    value: Value,
) -> Result<Value, GroupParseError> {
    match (conversion, value) {
        (Conversion::GroupJidToWid | Conversion::UserJidToWid, value @ Value::Jid(_)) => Ok(value),
        (Conversion::ToJid, Value::Jid(jid)) => Ok(Value::String(jid.to_string())),
        (Conversion::AsIsoCountryCode, Value::String(code)) => {
            Ok((context.normalize_country_code)(&code).map_or(Value::Undefined, Value::String))
        }
        _ => Err(GroupParseError::UnsupportedActionType(
            "generated envelope conversion",
        )),
    }
}
pub(super) fn is_empty(value: Value) -> Value {
    Value::Bool(value.is_nullish() || matches!(value,Value::String(ref s) if s.is_empty()))
}
#[derive(Clone, Copy)]
pub(super) enum Binary {
    StrictEqual,
    NullishEqual,
    NullishNotEqual,
    Add,
}
fn primitive_string(value: &Value) -> Result<String, GroupParseError> {
    match value {
        Value::Undefined => Ok("undefined".into()),
        Value::Null => Ok("null".into()),
        Value::Bool(v) => Ok(v.to_string()),
        Value::String(v) => Ok(v.clone()),
        Value::Integer(v) => Ok(v.to_string()),
        _ => Err(GroupParseError::UnsupportedActionType(
            "object coercion in envelope",
        )),
    }
}
pub(super) fn binary(
    operator: Binary,
    left: Value,
    right: Value,
) -> Result<Value, GroupParseError> {
    match operator {
        Binary::NullishEqual => Ok(Value::Bool(left.is_nullish() && right.is_nullish())),
        Binary::NullishNotEqual => Ok(Value::Bool(!(left.is_nullish() && right.is_nullish()))),
        Binary::StrictEqual => {
            if matches!(left, Value::Jid(_) | Value::Object(_) | Value::Actions)
                || matches!(right, Value::Jid(_) | Value::Object(_) | Value::Actions)
            {
                return Err(GroupParseError::UnsupportedActionType(
                    "object equality in envelope",
                ));
            }
            Ok(Value::Bool(left == right))
        }
        Binary::Add => {
            if matches!(left, Value::String(_)) || matches!(right, Value::String(_)) {
                Ok(Value::String(
                    primitive_string(&left)? + &primitive_string(&right)?,
                ))
            } else if let (Value::Integer(a), Value::Integer(b)) = (left, right) {
                a.checked_add(b)
                    .filter(|value| value.unsigned_abs() <= 9_007_199_254_740_991)
                    .map(Value::Integer)
                    .ok_or(GroupParseError::NumericRange("generated envelope addition"))
            } else {
                Err(GroupParseError::UnsupportedActionType(
                    "nonprimitive addition in envelope",
                ))
            }
        }
    }
}
pub(super) fn reject(message: Value) -> Result<Value, GroupParseError> {
    Err(GroupParseError::Rejected(primitive_string(&message)?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use wacore_binary::builder::NodeBuilder;
    fn never_me(_: &Jid) -> bool {
        false
    }
    fn country(code: &str) -> Option<String> {
        (code == "US").then(|| "US".into())
    }
    fn ctx(lid: Option<&Jid>, username: bool) -> GroupParseContext<'_> {
        GroupParseContext {
            me_device_lid: lid,
            me_device_pn: None,
            username_displayed: username,
            history_after_join: false,
            opus_enabled: false,
            is_me_account: &never_me,
            normalize_country_code: &country,
        }
    }
    fn base() -> NodeBuilder {
        NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("t", "123")
    }
    #[test]
    fn local_identity_is_requested_only_when_destination_is_present() {
        let node = base().build();
        assert!(parse_prefix(&node.as_node_ref(), &ctx(None, false)).is_ok());
        let node = base().attr("to", "100001:2@lid").build();
        assert!(matches!(
            parse_prefix(&node.as_node_ref(), &ctx(None, false)),
            Err(GroupParseError::MissingContext("me_device_lid"))
        ));
        let lid = Jid::lid_device("100001", 2);
        assert!(parse_prefix(&node.as_node_ref(), &ctx(Some(&lid), false)).is_ok());
    }
    #[test]
    fn phone_failure_is_local_and_marker_is_combined_with_display_gate() {
        for enabled in [false, true] {
            let node = base().attr("participant_pn", "100004@lid").build();
            let context = ctx(None, enabled);
            let mut state = parse_prefix(&node.as_node_ref(), &context).unwrap();
            assert_eq!(state.diagnostics.len(), 1);
            state.mark_missing().unwrap();
            state.mark_actions().unwrap();
            let projection = finish_projection(&node.as_node_ref(), &context, &mut state).unwrap();
            assert_eq!(projection.field("authorPhoneNumber"), Some(&Value::Null));
            assert_eq!(
                projection.field("hasIncompleteParticipantInformation"),
                Some(&Value::Bool(enabled))
            );
        }
    }
    #[test]
    fn final_id_read_follows_the_external_action_boundary() {
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("t", "123")
            .build();
        let context = ctx(None, false);
        let mut state = parse_prefix(&node.as_node_ref(), &context).unwrap();
        assert!(matches!(
            finish_projection(&node.as_node_ref(), &context, &mut state),
            Err(GroupParseError::MissingContext("normalized_actions"))
        ));
        state.mark_actions().unwrap();
        assert!(matches!(
            finish_projection(&node.as_node_ref(), &context, &mut state),
            Err(GroupParseError::Field("id"))
        ));
    }
    #[test]
    fn country_code_uses_undefined_for_unknown_and_null_for_disabled() {
        let node = base().attr("participant_country_code", "unknown").build();
        for enabled in [false, true] {
            let context = ctx(None, enabled);
            let mut state = parse_prefix(&node.as_node_ref(), &context).unwrap();
            state.mark_actions().unwrap();
            let projection = finish_projection(&node.as_node_ref(), &context, &mut state).unwrap();
            assert_eq!(
                projection.field("authorUsernameCountryCode"),
                Some(if enabled {
                    &Value::Undefined
                } else {
                    &Value::Null
                })
            );
        }
    }
    #[test]
    fn timestamps_follow_the_official_signed_time_cast() {
        let node = NodeBuilder::new("notification")
            .attr("from", "123456@g.us")
            .attr("id", "synthetic")
            .attr("t", "2147483648")
            .build();
        let context = ctx(None, false);
        let mut state = parse_prefix(&node.as_node_ref(), &context).unwrap();
        state.mark_actions().unwrap();
        let projection = finish_projection(&node.as_node_ref(), &context, &mut state).unwrap();
        assert_eq!(
            projection.field("ts"),
            Some(&Value::Integer(-2_147_483_647))
        );
    }
}

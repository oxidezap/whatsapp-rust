//! Borrowed, bounded protocol diagnostics. Payloads remain on the original nodes.
use crate::jid::{ObservedJid, observe_token};
use crate::node::ValueRef;
use crate::{Node, NodeContent, NodeContentRef, NodeRef, NodeValue};
use std::fmt;

/// Correlates an opaque protocol identifier without disclosing its value.
pub struct ObservedId<'a>(&'a str);
impl<'a> ObservedId<'a> {
    pub const fn new(value: &'a str) -> Self {
        Self(value)
    }
}
impl fmt::Display for ObservedId<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "id#{:016x}", observe_token(self.0))
    }
}
impl fmt::Debug for ObservedId<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

/// Displays a known protocol atom; arbitrary server text is never a log payload.
pub struct ProtocolText<'a>(&'a str);
impl<'a> ProtocolText<'a> {
    pub const fn new(value: &'a str) -> Self {
        Self(value)
    }
}
impl fmt::Display for ProtocolText<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.len() <= 5
            && let Ok(code) = self.0.parse::<u16>()
        {
            return write!(f, "{code}");
        }
        match protocol_atom(self.0) {
            Some(atom) => f.write_str(atom),
            None if self.0.is_empty() => f.write_str("empty"),
            None => write!(f, "other(len={})", self.0.len()),
        }
    }
}
impl fmt::Debug for ProtocolText<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

fn protocol_atom(value: &str) -> Option<&'static str> {
    if value.len() > 128 {
        return None;
    }
    use crate::token::{self, TokenKind};
    if let Some(kind) = token::index_of_token(value) {
        return match kind {
            TokenKind::Single(i) => token::get_single_token(i),
            TokenKind::Double(d, i) => token::get_double_token(d, i),
        };
    }
    match value {
        "chatstate" => Some("chatstate"),
        "xmlstreamend" => Some("xmlstreamend"),
        "stream:error" => Some("stream:error"),
        "replaced" => Some("replaced"),
        "device_removed" => Some("device_removed"),
        "wait" => Some("wait"),
        "cancel" => Some("cancel"),
        "continue" => Some("continue"),
        "modify" => Some("modify"),
        "auth" => Some("auth"),
        "not-authorized" => Some("not-authorized"),
        "forbidden" => Some("forbidden"),
        "gone" => Some("gone"),
        "feature-not-implemented" => Some("feature-not-implemented"),
        "recipient-unavailable" => Some("recipient-unavailable"),
        "redirect" => Some("redirect"),
        "registration-required" => Some("registration-required"),
        "remote-server-not-found" => Some("remote-server-not-found"),
        "remote-server-timeout" => Some("remote-server-timeout"),
        "subscription-required" => Some("subscription-required"),
        "unexpected-request" => Some("unexpected-request"),
        "not-well-formed" => Some("not-well-formed"),
        "invalid-xml" => Some("invalid-xml"),
        "xml-not-well-formed" => Some("xml-not-well-formed"),
        "resource-constraint" => Some("resource-constraint"),
        "policy-violation" => Some("policy-violation"),
        "service-unavailable" => Some("service-unavailable"),
        "internal-server-error" => Some("internal-server-error"),
        "item-not-found" => Some("item-not-found"),
        "bad-request" => Some("bad-request"),
        "not-allowed" => Some("not-allowed"),
        "not-acceptable" => Some("not-acceptable"),
        _ => None,
    }
}

enum Value<'a> {
    Text(&'a str),
    Jid(ObservedJid<'a>),
}
enum Content {
    Empty,
    Bytes(usize),
    Text(usize),
    Children(usize),
}
trait Shape {
    fn tag(&self) -> &str;
    fn attrs_len(&self) -> usize;
    fn attr(&self, index: usize) -> Option<(&str, Value<'_>)>;
    fn content(&self) -> Content;
    fn child(&self, index: usize) -> Option<&dyn Shape>;
}
impl Shape for Node {
    fn tag(&self) -> &str {
        &self.tag
    }
    fn attrs_len(&self) -> usize {
        self.attrs.len()
    }
    fn attr(&self, index: usize) -> Option<(&str, Value<'_>)> {
        self.attrs.iter().nth(index).map(|(k, v)| {
            (
                k.as_ref(),
                match v {
                    NodeValue::String(s) => Value::Text(s.as_str()),
                    NodeValue::Jid(j) => Value::Jid(j.observe()),
                },
            )
        })
    }
    fn content(&self) -> Content {
        match self.content.as_ref() {
            None => Content::Empty,
            Some(NodeContent::Bytes(b)) => Content::Bytes(b.len()),
            Some(NodeContent::String(s)) => Content::Text(s.len()),
            Some(NodeContent::Nodes(n)) => Content::Children(n.len()),
        }
    }
    fn child(&self, index: usize) -> Option<&dyn Shape> {
        self.children()?.get(index).map(|n| n as &dyn Shape)
    }
}
impl Shape for NodeRef<'_> {
    fn tag(&self) -> &str {
        self.tag.as_ref()
    }
    fn attrs_len(&self) -> usize {
        self.attrs.len()
    }
    fn attr(&self, index: usize) -> Option<(&str, Value<'_>)> {
        self.attrs.iter().nth(index).map(|(k, v)| {
            (
                k.as_ref(),
                match v {
                    ValueRef::String(s) => Value::Text(s.as_ref()),
                    ValueRef::Jid(j) => Value::Jid(j.observe()),
                },
            )
        })
    }
    fn content(&self) -> Content {
        match self.content.as_ref() {
            None => Content::Empty,
            Some(NodeContentRef::Bytes(b)) => Content::Bytes(b.len()),
            Some(NodeContentRef::String(s)) => Content::Text(s.len()),
            Some(NodeContentRef::Nodes(n)) => Content::Children(n.len()),
        }
    }
    fn child(&self, index: usize) -> Option<&dyn Shape> {
        self.children()?.get(index).map(|n| n as &dyn Shape)
    }
}

/// Lazy safe node view: known tags, reasons/codes, shape and identifier correlation.
/// No node conversion/cloning, payload reads, hashing or allocation at construction.
/// Formatting visits at most 32 nodes, 16 attributes per node and three levels.
pub struct NodeSummary<'a>(&'a dyn Shape);
impl<'a> From<&'a Node> for NodeSummary<'a> {
    fn from(node: &'a Node) -> Self {
        Self(node)
    }
}
impl<'a, 'data> From<&'a NodeRef<'data>> for NodeSummary<'a> {
    fn from(node: &'a NodeRef<'data>) -> Self {
        Self(node)
    }
}
impl fmt::Display for NodeSummary<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        format_node(self.0, f, 0, &mut 32)
    }
}
impl fmt::Debug for NodeSummary<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
fn format_node(
    node: &dyn Shape,
    f: &mut fmt::Formatter<'_>,
    depth: usize,
    budget: &mut usize,
) -> fmt::Result {
    if *budget == 0 {
        return f.write_str("...");
    }
    *budget -= 1;
    write!(f, "<{}", protocol_atom(node.tag()).unwrap_or("unknown"))?;
    for index in 0..node.attrs_len().min(16) {
        let Some((key, value)) = node.attr(index) else {
            break;
        };
        match (key, value) {
            ("id" | "stanza_id" | "message_id", Value::Text(s)) => {
                write!(f, " {key}={}", ObservedId::new(s))?
            }
            ("from" | "to" | "participant" | "recipient", Value::Text(s)) => {
                write!(f, " {key}={}", ObservedJid::from(s))?
            }
            ("from" | "to" | "participant" | "recipient", Value::Jid(j)) => {
                write!(f, " {key}={j}")?
            }
            ("code" | "error_code" | "error", Value::Text(s)) if s.len() <= 5 => {
                if let Ok(code) = s.parse::<u16>() {
                    write!(f, " {key}={code}")?;
                }
            }
            (
                "type" | "reason" | "status" | "stage" | "class" | "xmlns" | "text",
                Value::Text(s),
            ) => {
                if s.len() <= 5
                    && let Ok(code) = s.parse::<u16>()
                {
                    write!(f, " {key}={code}")?;
                } else {
                    write!(f, " {key}={}", ProtocolText::new(s))?;
                }
            }
            ("backoff" | "retry" | "v", Value::Text(s)) if s.len() <= 10 => {
                if let Ok(number) = s.parse::<u32>() {
                    write!(f, " {key}={number}")?;
                }
            }
            _ => {}
        }
    }
    write!(f, " attrs={}", node.attrs_len())?;
    match node.content() {
        Content::Empty => f.write_str(" body=none")?,
        Content::Bytes(n) => write!(f, " body=bytes({n})")?,
        Content::Text(n) => write!(f, " body=text({n})")?,
        Content::Children(n) => {
            write!(f, " children={n}")?;
            if depth < 2 && *budget > 0 {
                f.write_str("[")?;
                let limit = n.min(8);
                for index in 0..limit {
                    if index > 0 {
                        f.write_str(", ")?;
                    }
                    if let Some(child) = node.child(index) {
                        format_node(child, f, depth + 1, budget)?;
                    }
                    if *budget == 0 {
                        break;
                    }
                }
                if n > limit || *budget == 0 {
                    f.write_str(", ...")?;
                }
                f.write_str("]")?;
            }
        }
    }
    f.write_str(">")
}

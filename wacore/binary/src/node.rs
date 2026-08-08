use crate::attrs::{AttrParser, AttrParserRef};
use crate::jid::{Jid, JidRef};
use crate::token;
use bytes::Bytes;
use compact_str::CompactString;
use stable_deref_trait::StableDeref;
use std::borrow::Cow;

/// Borrowed-or-inline string for decoded nodes. Short owned values (≤24 bytes)
/// are stored inline via `CompactString`, avoiding heap allocation.
#[derive(Clone, yoke::Yokeable)]
pub enum NodeStr<'a> {
    Borrowed(&'a str),
    Owned(CompactString),
}

impl NodeStr<'_> {
    /// Clone-preserving conversion. Avoids re-parsing the inner CompactString
    /// when converting owned NodeStr values in `to_owned()` paths.
    #[inline]
    pub fn to_compact_string(&self) -> CompactString {
        match self {
            NodeStr::Borrowed(s) => CompactString::from(*s),
            NodeStr::Owned(cs) => cs.clone(),
        }
    }
}

impl Default for NodeStr<'_> {
    #[inline]
    fn default() -> Self {
        NodeStr::Borrowed("")
    }
}

impl std::ops::Deref for NodeStr<'_> {
    type Target = str;
    #[inline(always)]
    fn deref(&self) -> &str {
        match self {
            NodeStr::Borrowed(s) => s,
            NodeStr::Owned(cs) => cs.as_str(),
        }
    }
}

impl AsRef<str> for NodeStr<'_> {
    #[inline(always)]
    fn as_ref(&self) -> &str {
        self
    }
}

impl fmt::Debug for NodeStr<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&**self, f)
    }
}

impl fmt::Display for NodeStr<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self)
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for NodeStr<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self)
    }
}

impl PartialEq for NodeStr<'_> {
    #[inline]
    fn eq(&self, other: &Self) -> bool {
        **self == **other
    }
}

impl Eq for NodeStr<'_> {}

impl std::hash::Hash for NodeStr<'_> {
    #[inline]
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        (**self).hash(state)
    }
}

impl PartialEq<str> for NodeStr<'_> {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        &**self == other
    }
}

impl PartialEq<&str> for NodeStr<'_> {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        &**self == *other
    }
}

impl<'a> From<&'a str> for NodeStr<'a> {
    #[inline]
    fn from(s: &'a str) -> Self {
        NodeStr::Borrowed(s)
    }
}

impl From<CompactString> for NodeStr<'_> {
    #[inline]
    fn from(s: CompactString) -> Self {
        NodeStr::Owned(s)
    }
}

/// Intern a string as a `Cow::Borrowed(&'static str)` if it matches a known token,
/// otherwise allocate a `Cow::Owned(String)`. This avoids heap allocations for the
/// vast majority of tag names and attribute keys which are protocol tokens.
#[inline]
fn intern_cow(s: &str) -> Cow<'static, str> {
    if let Some(kind) = token::index_of_token(s) {
        let interned = match kind {
            token::TokenKind::Single(idx) => token::get_single_token(idx),
            token::TokenKind::Double(dict, idx) => token::get_double_token(dict, idx),
        };
        if let Some(token) = interned {
            return Cow::Borrowed(token);
        }
    }
    Cow::Owned(s.to_string())
}

/// An owned attribute value that can be either a string or a structured JID.
/// This avoids string allocation for JID attributes by storing the JID directly,
/// eliminating format/parse overhead when routing logic needs the JID.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub enum NodeValue {
    String(CompactString),
    Jid(Jid),
}

impl Default for NodeValue {
    fn default() -> Self {
        NodeValue::String(CompactString::default())
    }
}

impl NodeValue {
    /// String view of the value. Works for both variants.
    /// - String variant: Cow::Borrowed(&str) — zero copy
    /// - Jid variant: Cow::Owned(formatted) — allocates only when needed
    #[inline]
    pub fn as_str(&self) -> Cow<'_, str> {
        match self {
            NodeValue::String(s) => Cow::Borrowed(s.as_str()),
            NodeValue::Jid(j) => Cow::Owned(j.to_string()),
        }
    }

    /// Convert to an owned Jid, parsing from string if necessary.
    #[inline]
    pub fn to_jid(&self) -> Option<Jid> {
        match self {
            NodeValue::Jid(j) => Some(j.clone()),
            NodeValue::String(s) => s.parse().ok(),
        }
    }
}

use std::fmt;

impl fmt::Display for NodeValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NodeValue::String(s) => write!(f, "{}", s),
            NodeValue::Jid(j) => write!(f, "{}", j),
        }
    }
}

impl PartialEq<str> for NodeValue {
    fn eq(&self, other: &str) -> bool {
        match self {
            NodeValue::String(s) => s == other,
            NodeValue::Jid(j) => j.display_eq(other),
        }
    }
}

impl PartialEq<&str> for NodeValue {
    fn eq(&self, other: &&str) -> bool {
        self == *other
    }
}

impl PartialEq<String> for NodeValue {
    fn eq(&self, other: &String) -> bool {
        self == other.as_str()
    }
}

impl From<String> for NodeValue {
    #[inline]
    fn from(s: String) -> Self {
        NodeValue::String(CompactString::from(s))
    }
}

impl From<&str> for NodeValue {
    #[inline]
    fn from(s: &str) -> Self {
        NodeValue::String(CompactString::from(s))
    }
}

impl From<&String> for NodeValue {
    #[inline]
    fn from(s: &String) -> Self {
        NodeValue::String(CompactString::from(s.as_str()))
    }
}

impl From<CompactString> for NodeValue {
    #[inline]
    fn from(s: CompactString) -> Self {
        NodeValue::String(s)
    }
}

impl From<Jid> for NodeValue {
    #[inline]
    fn from(jid: Jid) -> Self {
        NodeValue::Jid(jid)
    }
}

impl From<&Jid> for NodeValue {
    #[inline]
    fn from(jid: &Jid) -> Self {
        NodeValue::Jid(jid.clone())
    }
}

macro_rules! impl_from_integer_for_nodevalue {
    ($($t:ty),* $(,)?) => {
        $(
            impl From<$t> for NodeValue {
                #[inline]
                fn from(n: $t) -> Self {
                    let mut buf = itoa::Buffer::new();
                    NodeValue::String(CompactString::from(buf.format(n)))
                }
            }
        )*
    };
}

impl_from_integer_for_nodevalue!(
    u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize
);

impl From<bool> for NodeValue {
    #[inline]
    fn from(b: bool) -> Self {
        NodeValue::String(CompactString::from(if b { "true" } else { "false" }))
    }
}

/// Inline backing store for [`Attrs`]. A plain `Vec` paid one heap allocation
/// per node on the encode hot path just for the backing buffer. Capacity 3 is
/// the measured optimum: it is the width of the `<enc>` fanout node (`v`,
/// `type`, `mediatype`), the highest-multiplicity node in a send, so a 64-device
/// fanout keeps it inline and drops a fifth of its allocations and a sixth of
/// its live bytes. Capacity 4 spares almost no further spills but widens `Node`
/// enough that moving it through the builder costs 8% more instructions.
pub type AttrsVec = smallvec::SmallVec<[(Cow<'static, str>, NodeValue); 3]>;

/// A collection of node attributes stored as key-value pairs.
/// Stored inline for small attribute counts for cache locality and to avoid a
/// per-node heap allocation; see [`AttrsVec`].
/// Values can be either strings or JIDs, avoiding stringification overhead for JID attributes.
/// Keys use `Cow<'static, str>` to avoid heap allocation for compile-time-known strings
/// (e.g., "type", "id", "to") which are the vast majority of attribute keys.
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Attrs(pub AttrsVec);

impl Attrs {
    #[inline]
    pub fn new() -> Self {
        Self(AttrsVec::new())
    }

    #[inline]
    pub fn with_capacity(capacity: usize) -> Self {
        Self(AttrsVec::with_capacity(capacity))
    }

    /// Get a reference to the NodeValue for a key, or None if not found.
    /// Uses linear search which is efficient for small attribute counts.
    #[inline]
    pub fn get(&self, key: &str) -> Option<&NodeValue> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// Check if a key exists.
    #[inline]
    pub fn contains_key(&self, key: &str) -> bool {
        self.0.iter().any(|(k, _)| k == key)
    }

    /// Insert a key-value pair. If the key already exists, update the value.
    #[inline]
    pub fn insert(&mut self, key: impl Into<Cow<'static, str>>, value: impl Into<NodeValue>) {
        let key = key.into();
        let value = value.into();
        if let Some(pos) = self.0.iter().position(|(k, _)| k == &key) {
            self.0[pos].1 = value;
        } else {
            self.0.push((key, value));
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Iterate over key-value pairs.
    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = (&Cow<'static, str>, &NodeValue)> {
        self.0.iter().map(|(k, v)| (k, v))
    }

    /// Push a key-value pair without checking for duplicates.
    /// Use this when building from a known-unique source (e.g., decoding).
    #[inline]
    pub fn push(&mut self, key: impl Into<Cow<'static, str>>, value: impl Into<NodeValue>) {
        self.0.push((key.into(), value.into()));
    }

    /// Push a NodeValue directly without conversion.
    /// Slightly more efficient when you already have a NodeValue.
    #[inline]
    pub fn push_value(&mut self, key: impl Into<Cow<'static, str>>, value: NodeValue) {
        self.0.push((key.into(), value));
    }

    /// Iterate over keys only.
    #[inline]
    pub fn keys(&self) -> impl Iterator<Item = &Cow<'static, str>> {
        self.0.iter().map(|(k, _)| k)
    }
}

/// Owned iterator implementation (consuming).
impl IntoIterator for Attrs {
    type Item = (Cow<'static, str>, NodeValue);
    type IntoIter = smallvec::IntoIter<[(Cow<'static, str>, NodeValue); 3]>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

/// Borrowed iterator implementation.
impl<'a> IntoIterator for &'a Attrs {
    type Item = (&'a Cow<'static, str>, &'a NodeValue);
    type IntoIter = std::iter::Map<
        std::slice::Iter<'a, (Cow<'static, str>, NodeValue)>,
        fn(&'a (Cow<'static, str>, NodeValue)) -> (&'a Cow<'static, str>, &'a NodeValue),
    >;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter().map(|(k, v)| (k, v))
    }
}

impl FromIterator<(Cow<'static, str>, NodeValue)> for Attrs {
    fn from_iter<I: IntoIterator<Item = (Cow<'static, str>, NodeValue)>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}
/// Covariant attribute container for decoded nodes.
///
/// Uses `Box<[T]>` (16 bytes: ptr + len) instead of `Vec<T>` (24 bytes: ptr + len + cap)
/// or inline storage (which inflated NodeRef size). Zero-attr nodes skip allocation
/// entirely. The boxed slice is allocated once with exact size from the decoder.
///
/// Covariant in `'a` (both Box and slices are covariant), compatible with yoke::Yokeable.
#[derive(Debug, Clone)]
pub enum AttrsRef<'a> {
    Empty,
    Slice(Box<[(NodeStr<'a>, ValueRef<'a>)]>),
}

impl PartialEq for AttrsRef<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl<'a> AttrsRef<'a> {
    /// Build from a pre-filled Vec. Preferred path from the decoder which
    /// knows the exact attr count upfront.
    pub fn from_vec(v: Vec<(NodeStr<'a>, ValueRef<'a>)>) -> Self {
        if v.is_empty() {
            Self::Empty
        } else {
            Self::Slice(v.into_boxed_slice())
        }
    }

    #[inline]
    pub fn len(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::Slice(s) => s.len(),
        }
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.as_slice().is_empty()
    }

    #[inline]
    pub fn as_slice(&self) -> &[(NodeStr<'a>, ValueRef<'a>)] {
        match self {
            Self::Empty => &[],
            Self::Slice(s) => s,
        }
    }

    #[inline]
    pub fn iter(&self) -> impl Iterator<Item = &(NodeStr<'a>, ValueRef<'a>)> {
        self.as_slice().iter()
    }
}

impl<'a> FromIterator<(NodeStr<'a>, ValueRef<'a>)> for AttrsRef<'a> {
    fn from_iter<I: IntoIterator<Item = (NodeStr<'a>, ValueRef<'a>)>>(iter: I) -> Self {
        Self::from_vec(iter.into_iter().collect())
    }
}

// Compile-time covariance check: if AttrsRef ever becomes invariant
// (e.g. by adding a Cell or &mut), this function will fail to compile.
fn _assert_attrs_ref_covariant<'short, 'long: 'short>(x: AttrsRef<'long>) -> AttrsRef<'short> {
    x
}

// Safety: AttrsRef<'a> is covariant in 'a because:
// - Empty carries no lifetime
// - Slice(Box<[(NodeStr<'a>, ValueRef<'a>)]>): Box<[T]> is covariant in T,
//   and (NodeStr<'a>, ValueRef<'a>) is covariant in 'a
// The _assert_attrs_ref_covariant function above enforces this at compile time.
unsafe impl<'a> yoke::Yokeable<'a> for AttrsRef<'static> {
    type Output = AttrsRef<'a>;

    fn transform(&'a self) -> &'a Self::Output {
        self
    }

    fn transform_owned(self) -> Self::Output {
        self
    }

    unsafe fn make(from: Self::Output) -> Self {
        unsafe { std::mem::transmute(from) }
    }

    fn transform_mut<F>(&'a mut self, f: F)
    where
        F: 'static + for<'b> FnOnce(&'b mut Self::Output),
    {
        unsafe { f(std::mem::transmute::<&mut Self, &mut Self::Output>(self)) }
    }
}

/// A decoded attribute value that can be either a string or a structured JID.
/// This avoids string allocation when decoding JID tokens - the JidRef is returned
/// directly and only converted to a string when actually needed.
#[derive(Debug, Clone, PartialEq, yoke::Yokeable)]
pub enum ValueRef<'a> {
    String(NodeStr<'a>),
    Jid(JidRef<'a>),
}

#[cfg(feature = "serde")]
impl serde::Serialize for ValueRef<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            ValueRef::String(s) => {
                serializer.serialize_newtype_variant("NodeValue", 0, "String", &**s)
            }
            ValueRef::Jid(j) => serializer.serialize_newtype_variant("NodeValue", 1, "Jid", j),
        }
    }
}

impl<'a> ValueRef<'a> {
    /// Encode this value directly to the binary encoder.
    pub fn encode_value<W: crate::encoder::ByteWriter>(
        &self,
        encoder: &mut crate::encoder::Encoder<'_, W>,
    ) -> crate::error::Result<()> {
        match self {
            ValueRef::String(s) => encoder.write_string(s),
            ValueRef::Jid(jid) => encoder.write_jid_ref(jid),
        }
    }

    /// String view of the value. Borrows from `self`.
    /// - String variant: borrows the inner str — zero copy
    /// - Jid variant: Cow::Owned — allocates only when needed
    pub fn as_str(&self) -> Cow<'_, str> {
        match self {
            ValueRef::String(s) => Cow::Borrowed(s),
            ValueRef::Jid(j) => Cow::Owned(j.to_string()),
        }
    }

    /// Get the value as a JidRef, if it's a JID variant.
    pub fn as_jid(&self) -> Option<&JidRef<'a>> {
        match self {
            ValueRef::Jid(j) => Some(j),
            ValueRef::String(_) => None,
        }
    }

    /// Convert to an owned Jid, parsing from string if necessary.
    pub fn to_jid(&self) -> Option<Jid> {
        match self {
            ValueRef::Jid(j) => Some(j.to_owned()),
            ValueRef::String(s) => Jid::from_str(s.as_ref()).ok(),
        }
    }

    /// Convert to an owned NodeValue, preserving the variant (JID stays JID).
    pub fn to_node_value(&self) -> NodeValue {
        match self {
            ValueRef::String(s) => NodeValue::String(s.to_compact_string()),
            ValueRef::Jid(j) => NodeValue::Jid(j.to_owned()),
        }
    }
}

impl PartialEq<str> for ValueRef<'_> {
    #[inline]
    fn eq(&self, other: &str) -> bool {
        match self {
            ValueRef::String(value) => value == other,
            ValueRef::Jid(value) => value.display_eq(other),
        }
    }
}

impl PartialEq<&str> for ValueRef<'_> {
    #[inline]
    fn eq(&self, other: &&str) -> bool {
        self == *other
    }
}

impl PartialEq<String> for ValueRef<'_> {
    #[inline]
    fn eq(&self, other: &String) -> bool {
        self == other.as_str()
    }
}

use std::str::FromStr;

impl<'a> fmt::Display for ValueRef<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValueRef::String(s) => write!(f, "{}", s),
            ValueRef::Jid(j) => write!(f, "{}", j),
        }
    }
}

pub type NodeVec<'a> = Vec<NodeRef<'a>>;

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq)]
pub enum NodeContent {
    Bytes(Vec<u8>),
    String(CompactString),
    Nodes(Vec<Node>),
}

#[derive(Debug, Clone, PartialEq, yoke::Yokeable)]
pub enum NodeContentRef<'a> {
    Bytes(Cow<'a, [u8]>),
    String(NodeStr<'a>),
    Nodes(Box<[NodeRef<'a>]>),
}

#[cfg(feature = "serde")]
impl serde::Serialize for NodeContentRef<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            NodeContentRef::Bytes(b) => {
                serializer.serialize_newtype_variant("NodeContent", 0, "Bytes", b.as_ref())
            }
            NodeContentRef::String(s) => {
                serializer.serialize_newtype_variant("NodeContent", 1, "String", &**s)
            }
            NodeContentRef::Nodes(nodes) => {
                serializer.serialize_newtype_variant("NodeContent", 2, "Nodes", &**nodes)
            }
        }
    }
}

impl NodeContent {
    /// Convert an owned NodeContent to a borrowed NodeContentRef.
    pub fn as_content_ref(&self) -> NodeContentRef<'_> {
        match self {
            NodeContent::Bytes(b) => NodeContentRef::Bytes(Cow::Borrowed(b)),
            NodeContent::String(s) => NodeContentRef::String(NodeStr::Borrowed(s.as_str())),
            NodeContent::Nodes(nodes) => {
                let v: Vec<_> = nodes.iter().map(|n| n.as_node_ref()).collect();
                NodeContentRef::Nodes(v.into_boxed_slice())
            }
        }
    }
}

#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Node {
    pub tag: Cow<'static, str>,
    pub attrs: Attrs,
    pub content: Option<NodeContent>,
}

#[derive(Debug, Clone, PartialEq, yoke::Yokeable)]
pub struct NodeRef<'a> {
    pub tag: NodeStr<'a>,
    pub attrs: AttrsRef<'a>,
    pub content: Option<NodeContentRef<'a>>,
}

impl Node {
    pub fn new(
        tag: impl Into<Cow<'static, str>>,
        attrs: Attrs,
        content: Option<NodeContent>,
    ) -> Self {
        Self {
            tag: tag.into(),
            attrs,
            content,
        }
    }

    /// Convert an owned Node to a borrowed NodeRef.
    /// The returned NodeRef borrows from self.
    pub fn as_node_ref(&self) -> NodeRef<'_> {
        NodeRef {
            tag: NodeStr::Borrowed(self.tag.as_ref()),
            attrs: self
                .attrs
                .iter()
                .map(|(k, v)| {
                    let value_ref = match v {
                        NodeValue::String(s) => ValueRef::String(NodeStr::Borrowed(s.as_str())),
                        NodeValue::Jid(j) => ValueRef::Jid(JidRef {
                            user: NodeStr::Borrowed(&j.user),
                            server: j.server,
                            agent: j.agent,
                            device: j.device,
                            integrator: j.integrator,
                        }),
                    };
                    (NodeStr::Borrowed(k.as_ref()), value_ref)
                })
                .collect(),
            content: self.content.as_ref().map(|c| c.as_content_ref()),
        }
    }

    pub fn children(&self) -> Option<&[Node]> {
        match &self.content {
            Some(NodeContent::Nodes(nodes)) => Some(nodes),
            _ => None,
        }
    }

    pub fn attrs(&self) -> AttrParser<'_> {
        AttrParser::new(self)
    }

    pub fn get_optional_child_by_tag<'a>(&'a self, tags: &[&str]) -> Option<&'a Node> {
        let mut current_node = self;
        for &tag in tags {
            let children = current_node.children()?;
            current_node = children.iter().find(|c| c.tag == tag)?;
        }
        Some(current_node)
    }

    pub fn get_children_by_tag<'a>(&'a self, tag: &'a str) -> impl Iterator<Item = &'a Node> {
        // `unwrap_or_default` and not `into_iter().flatten()`: the absent case is
        // already an empty slice, so flattening only buys a nested iterator whose
        // state machine LLVM does not fold away on this hot traversal.
        self.children()
            .unwrap_or_default()
            .iter()
            .filter(move |c| c.tag == tag)
    }

    pub fn get_optional_child(&self, tag: &str) -> Option<&Node> {
        self.children()
            .and_then(|nodes| nodes.iter().find(|node| node.tag == tag))
    }

    /// Extract text content, handling both String and Bytes (lossy UTF-8).
    pub fn content_as_string(&self) -> Option<CompactString> {
        match &self.content {
            Some(NodeContent::String(s)) => Some(s.clone()),
            Some(NodeContent::Bytes(b)) => {
                Some(CompactString::from(String::from_utf8_lossy(b).as_ref()))
            }
            _ => None,
        }
    }
}

/// Wrapper that serializes `AttrsRef` with the same newtype-struct framing
/// that serde's derive produces for `Attrs(Vec<...>)`. Without this, binary
/// formats (bincode, postcard, etc.) would see a bare sequence instead of a
/// newtype struct wrapper.
#[cfg(feature = "serde")]
struct AttrsRefWrapper<'a, 'b>(&'b AttrsRef<'a>);

#[cfg(feature = "serde")]
impl serde::Serialize for AttrsRefWrapper<'_, '_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_newtype_struct("Attrs", self.0.as_slice())
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for NodeRef<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("Node", 3)?;
        s.serialize_field("tag", &*self.tag)?;
        s.serialize_field("attrs", &AttrsRefWrapper(&self.attrs))?;
        s.serialize_field("content", &self.content)?;
        s.end()
    }
}

impl<'a> NodeRef<'a> {
    pub fn new(tag: NodeStr<'a>, attrs: AttrsRef<'a>, content: Option<NodeContentRef<'a>>) -> Self {
        Self {
            tag,
            attrs,
            content,
        }
    }

    pub fn attrs(&self) -> AttrParserRef<'_> {
        AttrParserRef::new(self)
    }

    pub fn children(&self) -> Option<&[NodeRef<'a>]> {
        match self.content.as_ref() {
            Some(NodeContentRef::Nodes(nodes)) => Some(nodes),
            _ => None,
        }
    }

    pub fn get_attr(&self, key: &str) -> Option<&ValueRef<'a>> {
        self.attrs.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    pub fn attrs_iter(&self) -> impl Iterator<Item = (&NodeStr<'a>, &ValueRef<'a>)> {
        self.attrs.iter().map(|(k, v)| (k, v))
    }

    pub fn get_optional_child_by_tag(&self, tags: &[&str]) -> Option<&NodeRef<'a>> {
        let mut current_node = self;
        for &tag in tags {
            let children = current_node.children()?;
            current_node = children.iter().find(|c| c.tag == tag)?;
        }
        Some(current_node)
    }

    pub fn get_children_by_tag<'b>(&'b self, tag: &'b str) -> impl Iterator<Item = &'b NodeRef<'a>>
    where
        'a: 'b,
    {
        self.children()
            .unwrap_or_default()
            .iter()
            .filter(move |c| c.tag == tag)
    }

    pub fn get_optional_child(&self, tag: &str) -> Option<&NodeRef<'a>> {
        self.children()
            .and_then(|nodes| nodes.iter().find(|node| node.tag == tag))
    }

    /// Extract text content, handling both String and Bytes (lossy UTF-8).
    pub fn content_as_string(&self) -> Option<CompactString> {
        match self.content.as_ref() {
            Some(NodeContentRef::String(s)) => Some(s.to_compact_string()),
            Some(NodeContentRef::Bytes(b)) => Some(CompactString::from(
                String::from_utf8_lossy(b.as_ref()).as_ref(),
            )),
            _ => None,
        }
    }

    /// Zero-copy byte content, if this node has Bytes content.
    pub fn content_bytes(&self) -> Option<&[u8]> {
        match self.content.as_ref() {
            Some(NodeContentRef::Bytes(b)) => Some(b.as_ref()),
            _ => None,
        }
    }

    /// Zero-copy string content, if this node has String content.
    pub fn content_str(&self) -> Option<&str> {
        match self.content.as_ref() {
            Some(NodeContentRef::String(s)) => Some(s.as_ref()),
            _ => None,
        }
    }

    /// Child nodes from content, if this node has Nodes content.
    /// Alias for `children()`.
    #[inline]
    pub fn content_nodes(&self) -> Option<&[NodeRef<'a>]> {
        self.children()
    }

    pub fn to_owned(&self) -> Node {
        Node {
            tag: intern_cow(&self.tag),
            attrs: self
                .attrs
                .iter()
                .map(|(k, v)| {
                    let value = match v {
                        ValueRef::String(s) => NodeValue::String(s.to_compact_string()),
                        ValueRef::Jid(j) => NodeValue::Jid(j.to_owned()),
                    };
                    (intern_cow(k), value)
                })
                .collect::<Attrs>(),
            content: self.content.as_ref().map(|c| match c {
                NodeContentRef::Bytes(b) => NodeContent::Bytes(b.to_vec()),
                NodeContentRef::String(s) => NodeContent::String(s.to_compact_string()),
                NodeContentRef::Nodes(nodes) => {
                    NodeContent::Nodes(nodes.iter().map(|n| n.to_owned()).collect())
                }
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// OwnedNodeRef — self-referential zero-copy node via yoke
// ---------------------------------------------------------------------------

use yoke::Yoke;

#[derive(Clone)]
struct BytesCart(Bytes);

impl std::ops::Deref for BytesCart {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.0.as_ref()
    }
}

// Safety: `Bytes` points to immutable backing storage whose deref target
// remains stable for the lifetime of the value, even when the wrapper moves.
unsafe impl StableDeref for BytesCart {}

/// A decoded node that owns its decompressed buffer. The inner `NodeRef`
/// borrows string/byte payloads directly from the buffer, avoiding copies.
/// Container allocations (attribute Vec, child Vec) still occur during decode.
///
/// Wrap in `Arc<OwnedNodeRef>` for cheap sharing across handlers.
pub struct OwnedNodeRef {
    inner: Yoke<NodeRef<'static>, BytesCart>,
}

impl OwnedNodeRef {
    /// Decode a node from an owned buffer of node bytes: after decompression and
    /// without the format byte, which [`unpack`](crate::util::unpack) strips.
    /// A buffer that still carries it goes through `unpack` first.
    pub fn new(buffer: impl Into<Bytes>) -> crate::error::Result<Self> {
        let inner = Yoke::try_attach_to_cart(BytesCart(buffer.into()), |buf| {
            crate::marshal::unmarshal_ref(buf)
        })?;
        Ok(Self { inner })
    }

    /// Access the borrowed node.
    #[inline]
    pub fn get(&self) -> &NodeRef<'_> {
        self.inner.get()
    }

    /// Convert to an owned `Node`, cloning all data out of the buffer.
    /// Use sparingly — this is the allocation path that yoke is designed to avoid.
    pub fn to_owned_node(&self) -> Node {
        self.inner.get().to_owned()
    }

    /// The whole backing buffer, verbatim: exactly what [`Self::new`] consumed.
    ///
    /// A refcount bump, not a copy — the yoke already retains this buffer, and
    /// [`Self::slice_bytes`] hands out views into the same allocation.
    ///
    /// These are node bytes, so they are **not** a sendable frame: send paths
    /// take a packed payload, one format byte in front of the node bytes, which
    /// [`unpack`](crate::util::unpack) stripped on the way in. Put it back with
    /// [`pack`](crate::util::pack) before forwarding these bytes anywhere that
    /// expects marshal output.
    ///
    /// Re-encoding through [`marshal_ref`] is the alternative and a worse one
    /// for anything that forwards a stanza onward — to another process, a
    /// recording, a replay harness. Re-encoding costs a second pass and is only
    /// faithful while the token dictionaries match the ones that decoded it;
    /// these bytes stay true whatever the dictionaries do.
    ///
    /// [`marshal_ref`]: crate::marshal::marshal_ref
    pub fn backing_bytes(&self) -> Bytes {
        self.inner.backing_cart().0.clone()
    }

    /// Return a zero-copy `Bytes` sub-view for a slice that borrows from this
    /// node's backing buffer. Panics if `slice` does not point within the buffer.
    pub fn slice_bytes(&self, slice: &[u8]) -> Bytes {
        let cart = &self.inner.backing_cart().0;
        let base = cart.as_ptr() as usize;
        let end = base + cart.len();
        let ptr = slice.as_ptr() as usize;
        assert!(
            ptr >= base && ptr + slice.len() <= end,
            "slice is not within the backing buffer"
        );
        let offset = ptr - base;
        cart.slice(offset..offset + slice.len())
    }

    /// The tag name of this node.
    #[inline]
    pub fn tag(&self) -> &str {
        &self.get().tag
    }

    /// Get an attribute parser for this node.
    #[inline]
    pub fn attrs(&self) -> AttrParserRef<'_> {
        self.get().attrs()
    }

    /// Look up a single attribute by key.
    #[inline]
    pub fn get_attr(&self, key: &str) -> Option<&ValueRef<'_>> {
        self.get().get_attr(key)
    }

    /// Get child nodes, if content is a node list.
    #[inline]
    pub fn children(&self) -> Option<&[NodeRef<'_>]> {
        self.get().children()
    }

    /// Find a child node by tag.
    #[inline]
    pub fn get_optional_child(&self, tag: &str) -> Option<&NodeRef<'_>> {
        self.get().get_optional_child(tag)
    }

    /// Find a child by traversing a path of tags.
    #[inline]
    pub fn get_optional_child_by_tag(&self, tags: &[&str]) -> Option<&NodeRef<'_>> {
        self.get().get_optional_child_by_tag(tags)
    }

    /// Get children matching a tag.
    #[inline]
    pub fn get_children_by_tag<'b>(
        &'b self,
        tag: &'b str,
    ) -> impl Iterator<Item = &'b NodeRef<'b>> {
        self.get().get_children_by_tag(tag)
    }

    /// Zero-copy byte content, if this node has Bytes content.
    #[inline]
    pub fn content_bytes(&self) -> Option<&[u8]> {
        self.get().content_bytes()
    }

    /// Zero-copy string content, if this node has String content.
    #[inline]
    pub fn content_str(&self) -> Option<&str> {
        self.get().content_str()
    }

    /// Child nodes from content, if this node has Nodes content.
    #[inline]
    pub fn content_nodes(&self) -> Option<&[NodeRef<'_>]> {
        self.get().content_nodes()
    }

    /// Extract text content, handling both String and Bytes (lossy UTF-8).
    #[inline]
    pub fn content_as_string(&self) -> Option<CompactString> {
        self.get().content_as_string()
    }
}

#[cfg(test)]
mod attrs_capacity_tests {
    use super::*;

    fn node_with(n: usize) -> Node {
        const KEYS: [&str; 8] = ["to", "id", "type", "t", "edit", "phash", "count", "offline"];
        let mut attrs = Attrs::new();
        for (i, key) in KEYS.iter().take(n).enumerate() {
            attrs.push(
                Cow::Borrowed(*key),
                NodeValue::String(format!("v{i}").into()),
            );
        }
        Node::new("message", attrs, Some(NodeContent::String("body".into())))
    }

    /// Capacity is invisible to the wire: every attribute count decodes back to
    /// what it was encoded from, on both sides of the inline/heap transition.
    #[test]
    fn every_attribute_count_survives_a_round_trip() {
        for n in 0..=8 {
            let node = node_with(n);
            let bytes = crate::marshal::marshal(&node).unwrap();
            let decoded = crate::marshal::unmarshal_ref(&bytes[1..])
                .unwrap()
                .to_owned();
            assert_eq!(decoded, node, "{n} attributes did not round-trip");
        }
    }

    /// The inline/heap boundary, where an off-by-one in the capacity hides.
    #[test]
    fn the_inline_boundary_is_where_the_declaration_says_it_is() {
        assert!(!node_with(3).attrs.0.spilled(), "3 attrs must stay inline");
        assert!(node_with(4).attrs.0.spilled(), "4 attrs must spill");
    }
}

#[cfg(test)]
mod value_ref_compare_tests {
    use super::*;
    use crate::jid::Server;
    use std::str::FromStr;

    /// Callers compare a `ValueRef` against a literal to decide whether a
    /// stanza came from the server, so `v == needle` has to answer exactly what
    /// `v.as_str() == needle` answered — including for the server-only shape
    /// (`s.whatsapp.net`, no user), which is the one those checks actually use.
    #[test]
    fn comparing_a_jid_value_matches_comparing_its_rendered_form() {
        let jids = [
            "s.whatsapp.net",
            "5511999998888@s.whatsapp.net",
            "5511999998888:7@s.whatsapp.net",
            "5511999998888.2@s.whatsapp.net",
            "120363012345678901@g.us",
            "123456789012345@lid",
            "123456789012345:9@lid",
            "123456789.4:17@interop",
            "status@broadcast",
            "12345.6@hosted.lid",
        ];
        let needles = [
            "s.whatsapp.net",
            "5511999998888@s.whatsapp.net",
            "5511999998888:7@s.whatsapp.net",
            "123456789012345@lid",
            "",
            "not-a-jid",
        ];

        for raw in jids {
            let owned = Jid::from_str(raw).unwrap_or_else(|e| panic!("{raw}: {e}"));
            let borrowed = ValueRef::Jid(JidRef {
                user: NodeStr::Borrowed(&owned.user),
                server: owned.server,
                agent: owned.agent,
                device: owned.device,
                integrator: owned.integrator,
            });
            let as_string = ValueRef::String(NodeStr::Borrowed(raw));

            for needle in needles {
                assert_eq!(
                    borrowed.as_str() == needle,
                    borrowed == needle,
                    "jid value {raw:?} vs {needle:?}"
                );
                assert_eq!(
                    as_string.as_str() == needle,
                    as_string == needle,
                    "string value {raw:?} vs {needle:?}"
                );
            }
        }

        // The server-only shape is the one the server checks compare against.
        let server_only = ValueRef::Jid(JidRef {
            user: NodeStr::Borrowed(""),
            server: Server::Pn,
            agent: 0,
            device: 0,
            integrator: 0,
        });
        assert!(server_only == "s.whatsapp.net");
        assert!(server_only != "5511999998888@s.whatsapp.net");
    }
}

#[cfg(test)]
mod owned_node_ref_tests {
    use super::*;

    /// Node bytes, as `OwnedNodeRef::new` wants them: marshal output with the
    /// format byte taken off, the way the receive path gets them.
    fn encoded(node: &Node) -> Bytes {
        let packed = crate::marshal::marshal(node).unwrap();
        Bytes::from(crate::util::unpack(&packed).unwrap().into_owned())
    }

    fn sample() -> Node {
        Node::new(
            "iq",
            Attrs(vec![(Cow::Borrowed("id"), NodeValue::String("abc".into()))].into()),
            Some(NodeContent::Bytes(b"payload".to_vec())),
        )
    }

    #[test]
    fn borrowed_payloads_survive_moving_the_cart() {
        let node = sample();
        let owned = OwnedNodeRef::new(encoded(&node)).unwrap();

        // Move the value twice — through a Box and into a Vec — before reading
        // anything back. That is the whole `StableDeref` claim: the yoked
        // `NodeRef` keeps pointing at live bytes even though the wrapper it
        // borrows from has moved. Nothing but an interpreter notices when it
        // stops being true, which is why this test exists separately from the
        // serde one it used to be a side effect of.
        let mut moved = vec![*Box::new(owned)];
        let owned = moved.pop().unwrap();

        assert_eq!(owned.tag(), "iq");
        assert!(owned.get_attr("id").unwrap() == "abc");
        assert_eq!(owned.content_bytes(), Some(&b"payload"[..]));
        assert_eq!(owned.to_owned_node(), node);
    }

    #[test]
    fn yoked_attrs_ref_survives_make_transform_and_mutation() {
        // `NodeRef`'s derived `Yokeable` transmutes the whole struct in one go,
        // so yoking a node never calls `AttrsRef`'s hand-written impl — that one
        // is there to satisfy the derive's bound on the field. Reaching its
        // three methods takes a yoke of `AttrsRef` itself, and it is the only
        // way to put our own transmutes, rather than yoke's generated ones, in
        // front of the interpreter.
        let cart = BytesCart(Bytes::from_static(b"idabc"));
        let mut yoke: Yoke<AttrsRef<'static>, BytesCart> = Yoke::attach_to_cart(cart, |buf| {
            // Borrowed from the cart, which is what makes the transmutes load-bearing.
            let (key, value) = buf.split_at(2);
            AttrsRef::from_vec(vec![(
                NodeStr::Borrowed(std::str::from_utf8(key).expect("ascii")),
                ValueRef::String(NodeStr::Borrowed(
                    std::str::from_utf8(value).expect("ascii"),
                )),
            )])
        });

        // `make` ran on attach; `transform` runs here.
        let (key, value) = &yoke.get().as_slice()[0];
        assert!(*key == "id");
        assert!(*value == "abc");

        // `transform_mut`, which nothing in the workspace calls.
        yoke.with_mut(|attrs| {
            *attrs = AttrsRef::from_vec(vec![(
                NodeStr::Owned("k".into()),
                ValueRef::String(NodeStr::Owned("v".into())),
            )]);
        });
        assert!(yoke.get().as_slice()[0].1 == "v");
    }

    #[test]
    fn slice_bytes_views_the_backing_buffer_without_copying() {
        let owned = OwnedNodeRef::new(encoded(&sample())).unwrap();
        let content = owned.content_bytes().unwrap();

        let view = owned.slice_bytes(content);

        assert_eq!(view.as_ref(), b"payload");
        assert_eq!(view.as_ptr(), content.as_ptr(), "slice_bytes copied");
    }

    #[test]
    fn backing_bytes_returns_what_was_decoded_verbatim() {
        let encoded = encoded(&sample());
        let owned = OwnedNodeRef::new(encoded.clone()).unwrap();

        let backing = owned.backing_bytes();

        assert_eq!(backing.as_ref(), encoded.as_ref());
        // Decoding the returned bytes reproduces the same node, which is what
        // lets an observer forward a stanza instead of re-encoding it.
        let reparsed = OwnedNodeRef::new(backing).unwrap();
        assert_eq!(reparsed.to_owned_node(), owned.to_owned_node());
    }

    #[test]
    fn backing_bytes_shares_the_allocation_rather_than_copying() {
        let owned = OwnedNodeRef::new(encoded(&sample())).unwrap();
        let content = owned.content_bytes().unwrap();

        let backing = owned.backing_bytes();

        // `slice_bytes` views into this same buffer, so a payload borrowed from
        // the node is a subrange of what `backing_bytes` returns.
        assert_eq!(owned.slice_bytes(content).as_ptr(), content.as_ptr());
        let offset = content.as_ptr() as usize - backing.as_ptr() as usize;
        assert_eq!(&backing[offset..offset + content.len()], content);
    }
}

#[cfg(feature = "serde")]
impl serde::Serialize for OwnedNodeRef {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.get().serialize(serializer)
    }
}

impl fmt::Debug for OwnedNodeRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.get().fmt(f)
    }
}

#[cfg(test)]
mod value_ref_tests {
    use super::*;
    use crate::jid::Server;

    #[test]
    fn value_ref_compares_string_and_jid_display_without_conversion() {
        let string = ValueRef::String(NodeStr::Borrowed("encrypt"));
        assert!(string == "encrypt");
        assert!(string != "other");

        let jid = ValueRef::Jid(JidRef {
            user: NodeStr::Borrowed("12025550111"),
            server: Server::Pn,
            agent: 0,
            device: 7,
            integrator: 0,
        });
        assert!(jid == "12025550111:7@s.whatsapp.net");
        assert!(jid != "12025550111@s.whatsapp.net");
    }
}

#[cfg(test)]
#[cfg(feature = "serde")]
mod serde_tests {
    use super::*;
    use crate::jid::{Jid, Server};

    #[test]
    fn node_ref_serializes_same_as_node() {
        let node = Node::new(
            Cow::Borrowed("message"),
            Attrs(
                vec![
                    (Cow::Borrowed("type"), NodeValue::String("text".into())),
                    (Cow::Borrowed("from"), NodeValue::Jid(Jid::pn("5550199999"))),
                ]
                .into(),
            ),
            Some(NodeContent::String("hello".into())),
        );
        let node_ref = node.as_node_ref();

        let owned_json = serde_json::to_value(&node).unwrap();
        let ref_json = serde_json::to_value(&node_ref).unwrap();
        assert_eq!(owned_json, ref_json);
    }

    #[test]
    fn nested_nodes_serialize_same() {
        let child = Node::new(Cow::Borrowed("item"), Attrs::new(), None);
        let parent = Node::new(
            Cow::Borrowed("list"),
            Attrs::new(),
            Some(NodeContent::Nodes(vec![child])),
        );
        let parent_ref = parent.as_node_ref();

        assert_eq!(
            serde_json::to_value(&parent).unwrap(),
            serde_json::to_value(&parent_ref).unwrap(),
        );
    }

    #[test]
    fn bytes_content_serializes_same() {
        let node = Node::new(
            Cow::Borrowed("iq"),
            Attrs(vec![(Cow::Borrowed("id"), NodeValue::String("1".into()))].into()),
            Some(NodeContent::Bytes(vec![0xDE, 0xAD])),
        );
        let node_ref = node.as_node_ref();

        let owned_json = serde_json::to_value(&node).unwrap();
        let ref_json = serde_json::to_value(&node_ref).unwrap();
        assert_eq!(owned_json, ref_json);
    }

    #[test]
    fn value_ref_matches_node_value() {
        let string_val = NodeValue::String("hello".into());
        let string_ref = ValueRef::String(NodeStr::Borrowed("hello"));
        assert_eq!(
            serde_json::to_value(&string_val).unwrap(),
            serde_json::to_value(&string_ref).unwrap(),
        );

        let jid = Jid {
            user: "5550199999".into(),
            server: Server::Group,
            agent: 1,
            device: 2,
            integrator: 3,
        };
        let jid_val = NodeValue::Jid(jid.clone());
        let jid_ref_val = ValueRef::Jid(JidRef {
            user: NodeStr::Borrowed("5550199999"),
            server: Server::Group,
            agent: 1,
            device: 2,
            integrator: 3,
        });
        assert_eq!(
            serde_json::to_value(&jid_val).unwrap(),
            serde_json::to_value(&jid_ref_val).unwrap(),
        );
    }

    #[test]
    fn owned_node_ref_serializes_same_as_owned() {
        let node = Node::new(
            Cow::Borrowed("iq"),
            Attrs(vec![(Cow::Borrowed("id"), NodeValue::String("abc".into()))].into()),
            Some(NodeContent::String("payload".into())),
        );

        let packed = crate::marshal::marshal(&node).unwrap();
        let node_bytes = crate::util::unpack(&packed).unwrap().into_owned();
        let owned_ref = OwnedNodeRef::new(Bytes::from(node_bytes)).unwrap();

        let from_ref = serde_json::to_value(&owned_ref).unwrap();
        let from_owned = serde_json::to_value(owned_ref.to_owned_node()).unwrap();
        assert_eq!(from_ref, from_owned);
    }
}

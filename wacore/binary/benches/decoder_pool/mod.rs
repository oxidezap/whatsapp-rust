//! Benchmark-only storage alternatives using the production wire parser.
use crate as wacore_binary;
use wacore_binary::node::{NodeStr, ValueRef};
use wacore_binary::{Node, NodeContent, Result};

type Entry<'a> = (NodeStr<'a>, ValueRef<'a>);

#[derive(Clone, Copy)]
struct AttrRange {
    chunk: usize,
    start: usize,
    len: usize,
}

#[derive(yoke::Yokeable)]
struct Pool<'a> {
    chunks: Vec<Vec<Entry<'a>>>,
    chunk_capacity: usize,
}

impl<'a> Pool<'a> {
    fn new(chunk_capacity: usize) -> Self {
        Self {
            chunks: Vec::new(),
            chunk_capacity,
        }
    }

    fn reserve_node(&mut self, count: usize) -> AttrRange {
        if count == 0 {
            return AttrRange {
                chunk: 0,
                start: 0,
                len: 0,
            };
        }
        if self
            .chunks
            .last()
            .is_none_or(|v| v.capacity() - v.len() < count)
        {
            self.chunks
                .push(Vec::with_capacity(self.chunk_capacity.max(count)));
        }
        let chunk = self.chunks.len() - 1;
        AttrRange {
            chunk,
            start: self.chunks[chunk].len(),
            len: count,
        }
    }

    fn attrs(&self, range: AttrRange) -> &[Entry<'a>] {
        if range.len == 0 {
            &[]
        } else {
            &self.chunks[range.chunk][range.start..range.start + range.len]
        }
    }
}

#[derive(yoke::Yokeable)]
struct PooledNode<'a> {
    tag: NodeStr<'a>,
    attrs: AttrRange,
    content: Content<'a>,
}

#[derive(yoke::Yokeable)]
enum Content<'a> {
    None,
    Bytes(std::borrow::Cow<'a, [u8]>),
    String(NodeStr<'a>),
    Children(Box<[PooledNode<'a>]>),
}

#[derive(yoke::Yokeable)]
pub struct Frame<'a> {
    root: PooledNode<'a>,
    pool: Pool<'a>,
}

pub struct OwnedFrame {
    inner: yoke::Yoke<Frame<'static>, crate::node::BytesCart>,
}

impl OwnedFrame {
    pub fn new(buffer: bytes::Bytes, chunk_capacity: usize, two_pass: bool) -> Result<Self> {
        let inner = yoke::Yoke::try_attach_to_cart(crate::node::BytesCart(buffer), |data| {
            decode(data, chunk_capacity, two_pass)
        })?;
        Ok(Self { inner })
    }

    pub fn get(&self) -> &Frame<'_> {
        self.inner.get()
    }
}

impl Frame<'_> {
    pub fn to_owned(&self) -> Node {
        self.root.to_owned(&self.pool)
    }

    pub fn consume(&self) -> Result<usize> {
        self.root.consume(&self.pool)
    }
}

impl PooledNode<'_> {
    fn to_owned(&self, pool: &Pool<'_>) -> Node {
        Node::new(
            self.tag.to_compact_string(),
            pool.attrs(self.attrs)
                .iter()
                .map(|(k, v)| (std::borrow::Cow::Owned(k.to_string()), v.to_node_value()))
                .collect(),
            match &self.content {
                Content::None => None,
                Content::Bytes(bytes) => Some(NodeContent::Bytes(bytes.to_vec())),
                Content::String(s) => Some(NodeContent::String(s.to_compact_string())),
                Content::Children(nodes) => Some(NodeContent::Nodes(
                    nodes.iter().map(|n| n.to_owned(pool)).collect(),
                )),
            },
        )
    }

    fn consume(&self, pool: &Pool<'_>) -> Result<usize> {
        let mut sum = consume_attrs(pool.attrs(self.attrs))? + self.tag.len();
        if let Content::Children(children) = &self.content {
            for child in children {
                sum += child.consume(pool)?;
            }
        }
        Ok(sum)
    }
}

// Reuse the production parser and its validation in every storage arm.
mod wire {
    use crate::decoder::{Decoder, MAX_NODE_DEPTH, OpenContent};
    use crate::{BinaryError, NodeContentRef, NodeRef, Result};

    impl<'a> Decoder<'a> {
        fn read_pooled(
            &mut self,
            pool: &mut super::Pool<'a>,
            depth: usize,
        ) -> Result<super::PooledNode<'a>> {
            if depth >= MAX_NODE_DEPTH {
                return Err(BinaryError::MaxDepthExceeded);
            }
            let head = self.read_node_open_with(|decoder, count| {
                let range = pool.reserve_node(count);
                for _ in 0..count {
                    pool.chunks[range.chunk].push(decoder.read_attribute()?);
                }
                Ok(range)
            })?;
            let content = match head.content {
                OpenContent::None => super::Content::None,
                OpenContent::Scalar(NodeContentRef::Bytes(value)) => super::Content::Bytes(value),
                OpenContent::Scalar(NodeContentRef::String(value)) => super::Content::String(value),
                OpenContent::Scalar(NodeContentRef::Nodes(_)) => {
                    unreachable!("head parser returns scalar content")
                }
                OpenContent::Children(count) => {
                    let mut children = Vec::with_capacity(count);
                    for _ in 0..count {
                        children.push(self.read_pooled(pool, depth + 1)?);
                    }
                    super::Content::Children(children.into_boxed_slice())
                }
            };
            Ok(super::PooledNode {
                tag: head.tag,
                attrs: head.attrs,
                content,
            })
        }

        // Decode and validate every head, including packed values and scalar
        // content, without retaining an attribute list. This pass is timed.
        fn count_attributes(&mut self, depth: usize) -> Result<usize> {
            if depth >= MAX_NODE_DEPTH {
                return Err(BinaryError::MaxDepthExceeded);
            }
            let head = self.read_node_open_with(|decoder, count| {
                for _ in 0..count {
                    drop(decoder.read_attribute()?);
                }
                Ok(count)
            })?;
            let mut total = head.attrs;
            if let OpenContent::Children(count) = head.content {
                for _ in 0..count {
                    total += self.count_attributes(depth + 1)?;
                }
            }
            Ok(total)
        }
    }

    pub fn baseline(data: &[u8]) -> Result<NodeRef<'_>> {
        let mut decoder = Decoder::new(data);
        let node = decoder.read_node_ref()?;
        finished(&decoder)?;
        Ok(node)
    }

    fn finished(decoder: &Decoder<'_>) -> Result<()> {
        if decoder.is_finished() {
            Ok(())
        } else {
            Err(BinaryError::LeftoverData(decoder.bytes_left()))
        }
    }

    pub fn decode(data: &[u8], chunk_capacity: usize, two_pass: bool) -> Result<super::Frame<'_>> {
        let capacity = if two_pass {
            let mut counter = Decoder::new(data);
            let total = counter.count_attributes(0)?;
            finished(&counter)?;
            total
        } else {
            chunk_capacity
        };
        let mut pool = super::Pool::new(capacity);
        let mut decoder = Decoder::new(data);
        let root = decoder.read_pooled(&mut pool, 0)?;
        finished(&decoder)?;
        Ok(super::Frame { root, pool })
    }
}

pub use wire::{baseline, decode};

// Use the production attribute consumer on both representations, including
// structured-JID conversion and parse errors, without copying attribute lists.
use crate::attrs as consumer_attrs;

fn consume_attrs(attrs: &[Entry<'_>]) -> Result<usize> {
    let mut parser = consumer_attrs::AttrParserRef {
        attrs,
        errors: Vec::new(),
    };
    let from = parser.optional_jid_result("from")?;
    let participant = parser.optional_jid_result("participant")?;
    let id = parser.optional_string("id");
    parser.finish()?;
    Ok(from.map_or(0, |v| v.user.len())
        + participant.map_or(0, |v| v.user.len())
        + id.map_or(0, |s| s.len()))
}

pub fn consume_tree(node: &wacore_binary::NodeRef<'_>) -> Result<usize> {
    let mut sum = consume_attrs(node.attrs.as_slice())? + node.tag.len();
    if let Some(children) = node.children() {
        for child in children {
            sum += consume_tree(child)?;
        }
    }
    Ok(sum)
}

pub fn fixture(width: usize) -> Vec<u8> {
    use wacore_binary::{builder::NodeBuilder, jid::Jid, marshal::marshal_shallow};
    let children: Vec<_> = (0..width)
        .map(|i| {
            NodeBuilder::new("message")
                .attr(
                    "from",
                    Jid::pn_device(format!("55110000{:05}", i / 4), (i % 4) as u16),
                )
                .attr("participant", Jid::pn("5511000000000"))
                .attr(
                    "id",
                    if i % 3 == 0 {
                        "A12345678901234567890123456789012345678901234567890123456789012345"
                    } else {
                        "3EB0123456789"
                    },
                )
                .children(vec![
                    NodeBuilder::new("enc")
                        .attr("v", "2")
                        .attr("type", "msg")
                        .bytes(vec![0xAB; 128])
                        .build(),
                ])
                .build()
        })
        .collect();
    let root = NodeBuilder::new("notification")
        .attr("type", "devices")
        .children(children)
        .build();
    marshal_shallow(&root).expect("synthetic fixture")[1..].to_vec()
}

//! Incremental decoding of one node: a cursor that walks a node's children one
//! at a time instead of building the whole tree.
//!
//! The tree decoder ([`unmarshal_ref`](crate::marshal::unmarshal_ref), [`OwnedNodeRef`]) needs every
//! decompressed byte of a node in memory at once and one heap object per node
//! and attribute list. For a stanza of a few thousand small children that is
//! more memory than the payload by an order of magnitude, and on a
//! microcontroller it is more memory than there is. A [`NodeStream`] decodes the
//! same wire format from a source that produces bytes on demand, so a caller
//! that wants a handful of children out of thousands keeps one child, and one
//! inflate window, alive at a time.
//!
//! The cursor moves through a node the way the bytes are laid out: [`open`]
//! reads the head of the next node (tag, attributes, what its content is) and
//! descends into it; [`next_child`] decodes the next child whole; [`close`]
//! leaves the open node, skipping any children not yet read. What comes back
//! borrows the stream's buffer and is valid until the next call.
//!
//! [`open`]: NodeStream::open
//! [`next_child`]: NodeStream::next_child
//! [`close`]: NodeStream::close
//! [`OwnedNodeRef`]: crate::OwnedNodeRef

use std::io;

use crate::decoder::{Decoder, MAX_NODE_DEPTH, OpenContent};
use crate::error::{BinaryError, Result};
use crate::node::{AttrsRef, NodeRef, NodeStr, ValueRef};
use crate::util::{FORMAT_COMPRESSED, MAX_DECOMPRESSED_SIZE};
use crate::zlib_pool::InflateReader;

/// Inflate window for a compressed frame decoded through a stream.
///
/// The window is the stream's whole footprint beyond the pooled zlib state and
/// the child being decoded, so it is sized for the smallest target rather than
/// for throughput: a 4 KB window holds any run of the sub-100-byte children a
/// props response is made of, and a child larger than it grows the window to
/// fit. On a host the cost is one inflate call per 4 KB of output instead of
/// one per payload, a few microseconds on a 100 KB frame.
const FRAME_INFLATE_CHUNK: usize = 4096;

/// Where a stream's bytes come from.
enum Source<'a> {
    /// Node bytes already in memory.
    Plain { data: &'a [u8], pos: usize },
    /// Node bytes inflated from a compressed payload as they are needed.
    Inflate(InflateReader<'a>),
}

impl Source<'_> {
    #[inline]
    fn available(&self) -> &[u8] {
        match self {
            Self::Plain { data, pos } => &data[*pos..],
            Self::Inflate(reader) => reader.available(),
        }
    }

    #[inline]
    fn consume(&mut self, n: usize) {
        match self {
            Self::Plain { data, pos } => *pos = (*pos + n).min(data.len()),
            Self::Inflate(reader) => reader.consume(n),
        }
    }

    /// Make at least `need` bytes available, or every byte there is.
    fn ensure(&mut self, need: usize) -> Result<bool> {
        match self {
            Self::Plain { data, pos } => Ok(data.len() - *pos >= need),
            Self::Inflate(reader) => reader.ensure(need).map_err(zlib_error),
        }
    }
}

fn zlib_error(e: io::Error) -> BinaryError {
    BinaryError::Zlib(e.to_string())
}

/// The head of a node the stream has descended into.
///
/// Borrows the stream's buffer, so it lives until the stream's next call;
/// anything kept longer is copied out.
#[derive(Debug)]
pub struct OpenNode<'b> {
    pub tag: NodeStr<'b>,
    pub attrs: AttrsRef<'b>,
    pub content: OpenContent<'b>,
}

impl<'b> OpenNode<'b> {
    /// Look up a single attribute by key.
    pub fn get_attr(&self, key: &str) -> Option<&ValueRef<'b>> {
        self.attrs.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }

    /// An attribute as text, whatever its wire form.
    pub fn attr_str(&self, key: &str) -> Option<std::borrow::Cow<'_, str>> {
        self.get_attr(key).map(|v| v.as_str())
    }

    /// How many children the node has still to offer, when its content is a
    /// list of nodes.
    pub fn child_count(&self) -> usize {
        match self.content {
            OpenContent::Children(n) => n,
            _ => 0,
        }
    }
}

/// One node the cursor has descended into: how many of its children are still
/// unread.
struct Level {
    remaining: usize,
}

/// A cursor over one node's wire bytes; see the [module docs](self).
pub struct NodeStream<'a> {
    source: Source<'a>,
    /// Bytes decoded by the last `open`/`next_child`, still backing what that
    /// call returned. Consumed at the start of the next call.
    pending: usize,
    /// Every byte consumed so far. A materialization needs all of them, so it
    /// is only offered while this is still zero.
    consumed: usize,
    levels: Vec<Level>,
    /// The root has been opened (or skipped): there is no node left at depth 0.
    root_done: bool,
}

impl<'a> NodeStream<'a> {
    /// A stream over a packed payload: the format byte and, behind it, node
    /// bytes that are inflated on demand when the byte says they are compressed.
    ///
    /// This is what the receive path holds after decryption. A compressed
    /// payload checks out the thread's pooled inflate state for the life of the
    /// stream.
    pub fn from_packed(packed: &'a [u8]) -> Result<Self> {
        let Some((&format, node_bytes)) = packed.split_first() else {
            return Err(BinaryError::EmptyData);
        };
        if format & FORMAT_COMPRESSED != 0 {
            Ok(Self::with_source(Source::Inflate(
                InflateReader::with_chunk(node_bytes, MAX_DECOMPRESSED_SIZE, FRAME_INFLATE_CHUNK),
            )))
        } else {
            Ok(Self::from_node_bytes(node_bytes))
        }
    }

    /// A stream over node bytes already in memory (no format byte): the
    /// buffer behind an [`OwnedNodeRef`](crate::OwnedNodeRef), for a consumer
    /// that walks a node it was handed whole.
    pub fn from_node_bytes(node_bytes: &'a [u8]) -> Self {
        Self::with_source(Source::Plain {
            data: node_bytes,
            pos: 0,
        })
    }

    fn with_source(source: Source<'a>) -> Self {
        Self {
            source,
            pending: 0,
            consumed: 0,
            levels: Vec::new(),
            root_done: false,
        }
    }

    /// Whether the bytes are being inflated as they are read.
    pub fn is_compressed(&self) -> bool {
        matches!(self.source, Source::Inflate(_))
    }

    /// How deep the cursor is: 0 before the root is opened, 1 inside it.
    pub fn depth(&self) -> usize {
        self.levels.len()
    }

    /// Decode the head of the next node and descend into it.
    ///
    /// At depth 0 that is the root; inside an open node it is that node's next
    /// child, which then becomes the open node. `None` when the open node has
    /// no children left (or, at depth 0, once the root has been read).
    pub fn open(&mut self) -> Result<Option<OpenNode<'_>>> {
        if !self.take_slot() {
            return Ok(None);
        }
        if self.levels.len() >= MAX_NODE_DEPTH {
            return Err(BinaryError::MaxDepthExceeded);
        }
        let Self {
            source,
            pending,
            levels,
            ..
        } = self;
        let (head, used) = Self::decode_next(source, |decoder| decoder.read_node_open())?;
        *pending = used;
        levels.push(Level {
            remaining: match head.content {
                OpenContent::Children(n) => n,
                _ => 0,
            },
        });
        Ok(Some(OpenNode {
            tag: head.tag,
            attrs: head.attrs,
            content: head.content,
        }))
    }

    /// Decode the open node's next child whole, subtree and all.
    ///
    /// `None` once the open node has no children left, or at depth 0 once the
    /// root has been read.
    pub fn next_child(&mut self) -> Result<Option<NodeRef<'_>>> {
        if !self.take_slot() {
            return Ok(None);
        }
        let depth = self.levels.len();
        let Self {
            source, pending, ..
        } = self;
        let (node, used) = Self::decode_next(source, |decoder| decoder.read_node_ref_at(depth))?;
        *pending = used;
        Ok(Some(node))
    }

    /// Leave the open node, decoding and discarding any children not yet read,
    /// so the cursor stands at the parent's next child.
    pub fn close(&mut self) -> Result<()> {
        self.settle();
        let Some(level) = self.levels.last() else {
            return Ok(());
        };
        let mut remaining = level.remaining;
        while remaining > 0 {
            // Skipping still walks every byte: the wire carries no lengths, so
            // a child's end is only known once its tokens have been read.
            let depth = self.levels.len();
            let used = Self::skip_next(&mut self.source, |decoder| decoder.skip_node_at(depth))?;
            self.source.consume(used);
            self.consumed += used;
            remaining -= 1;
        }
        self.levels.pop();
        Ok(())
    }

    /// Consume everything after the cursor and check that the bytes end where
    /// the node does: nothing left over behind the root, and for a compressed
    /// payload a properly terminated stream (the checksum in the trailer is
    /// what says every inflated byte was the one the peer sent).
    ///
    /// The strictness [`unmarshal_ref`](crate::marshal::unmarshal_ref) applies to a tree
    /// (it rejects leftover data), applied to a stream that was only partly
    /// read. Only worth calling once what was wanted has been read: a stream
    /// abandoned on a parse error is simply dropped.
    pub fn finish(&mut self) -> Result<()> {
        while !self.levels.is_empty() {
            self.close()?;
        }
        if !self.root_done {
            // Nothing was ever read; the root still has to be walked.
            self.root_done = true;
            let used = Self::skip_next(&mut self.source, |decoder| decoder.skip_node_at(0))?;
            self.source.consume(used);
            self.consumed += used;
        }
        self.settle();
        match &mut self.source {
            Source::Plain { data, pos } => {
                if *pos < data.len() {
                    return Err(BinaryError::LeftoverData(data.len() - *pos));
                }
            }
            Source::Inflate(reader) => {
                // Pull the trailer through: a stream that ends exactly on the
                // root's last byte has its adler32 still unread.
                if reader.ensure(1).map_err(zlib_error)? {
                    return Err(BinaryError::LeftoverData(reader.available().len()));
                }
                if !reader.stream_ended() {
                    return Err(BinaryError::Zlib("zlib stream truncated".into()));
                }
            }
        }
        Ok(())
    }

    /// The whole node as one buffer, the way [`unpack`](crate::util::unpack)
    /// would have produced it, for a caller that looked at the root's head and
    /// decided the node is one to hold as a tree after all.
    ///
    /// Only for a compressed payload, and only before anything was consumed
    /// (an [`open`](Self::open) of the root consumes nothing until the next
    /// call): the bytes of a plain payload are the caller's own, and a
    /// consumed prefix is gone. Both return `None`.
    pub fn into_inflated(self) -> Option<Result<Vec<u8>>> {
        if self.consumed != 0 {
            return None;
        }
        match self.source {
            Source::Plain { .. } => None,
            Source::Inflate(reader) => Some(reader.read_to_end().map_err(zlib_error)),
        }
    }

    /// Drop the bytes behind what the last call returned.
    fn settle(&mut self) {
        if self.pending != 0 {
            self.source.consume(self.pending);
            self.consumed += self.pending;
            self.pending = 0;
        }
    }

    /// Claim the next node at the cursor: a child of the open node, or the
    /// root. False when there is none.
    fn take_slot(&mut self) -> bool {
        self.settle();
        match self.levels.last_mut() {
            Some(level) => {
                if level.remaining == 0 {
                    return false;
                }
                level.remaining -= 1;
                true
            }
            None => {
                if self.root_done {
                    return false;
                }
                self.root_done = true;
                true
            }
        }
    }

    /// Decode the next node out of the window, growing the window and decoding
    /// again when it ends mid-node. Returns what `decode` produced and how
    /// many bytes it took.
    ///
    /// The wire carries no lengths, so whether the window holds the whole node
    /// is only known once the node has been read. Decoding straight from the
    /// window and retrying on a short read costs one wasted attempt per window
    /// boundary the node straddles, a few per frame; walking the node first to
    /// size it (what [`Self::skip_next`] does) would cost a second pass over
    /// every node, a quarter of the decode. Each retry asks for double the
    /// window, so retries stay logarithmic in the node's size; a source that
    /// cannot grow turns the shortfall into a real `UnexpectedEof`.
    fn decode_next<'s, T>(
        source: &'s mut Source<'_>,
        decode: impl Fn(&mut Decoder<'s>) -> Result<T>,
    ) -> Result<(T, usize)> {
        loop {
            let window: *const [u8] = source.available();
            // SAFETY: `window` is a live slice of the source's buffer, and the
            // buffer is not touched again on the path that keeps `bytes`: a
            // successful decode returns at once, with `bytes` (and anything
            // `T` borrows from it) bound to `'s`, the exclusive borrow of
            // `source`, so no growth can follow while the value lives. On the
            // short-read path neither `bytes` nor the decoder over it is used
            // again once `grow` writes to the buffer. The raw pointer is what
            // lets the borrow checker accept a borrow that is returned on one
            // path and mutated past on the other (NLL problem case 3).
            let bytes: &'s [u8] = unsafe { &*window };
            let have = bytes.len();
            let mut decoder = Decoder::new(bytes);
            match decode(&mut decoder) {
                Ok(value) => return Ok((value, decoder.position())),
                Err(BinaryError::UnexpectedEof) => {}
                Err(e) => return Err(e),
            }
            if !Self::grow(source, have)? {
                return Err(BinaryError::UnexpectedEof);
            }
        }
    }

    /// Size the next node without decoding it (no allocation), growing the
    /// window as [`Self::decode_next`] does. For nodes that are only skipped.
    fn skip_next(
        source: &mut Source<'_>,
        walk: impl Fn(&mut Decoder<'_>) -> Result<()>,
    ) -> Result<usize> {
        loop {
            let have = source.available().len();
            let mut decoder = Decoder::new(source.available());
            match walk(&mut decoder) {
                Ok(()) => return Ok(decoder.position()),
                Err(BinaryError::UnexpectedEof) => {}
                Err(e) => return Err(e),
            }
            if !Self::grow(source, have)? {
                return Err(BinaryError::UnexpectedEof);
            }
        }
    }

    /// Grow the window past `have` bytes, asking for double. False when the
    /// source has nothing more.
    fn grow(source: &mut Source<'_>, have: usize) -> Result<bool> {
        let want = have.max(64).saturating_mul(2);
        Ok(source.ensure(want)? || source.available().len() > have)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::builder::NodeBuilder;
    use crate::marshal::marshal;
    use crate::node::{Node, NodeContent, NodeContentRef};
    use crate::zlib_pool::test_support::stored_zlib;

    /// The compressed form of a packed payload. Stored blocks, not deflate:
    /// the inflate path is what is under test, and a real compressor is not
    /// Miri-clean (see `stored_zlib`).
    fn compress(node_bytes: &[u8]) -> Vec<u8> {
        let mut out = vec![FORMAT_COMPRESSED];
        out.extend_from_slice(&stored_zlib(node_bytes));
        out
    }

    /// The same node as a plain packed payload and as a compressed one.
    fn both_forms(node: &Node) -> Vec<(&'static str, Vec<u8>)> {
        let packed = marshal(node).unwrap();
        let compressed = compress(&packed[1..]);
        vec![("plain", packed), ("compressed", compressed)]
    }

    fn prop(code: u32, value: &str) -> Node {
        NodeBuilder::new("prop")
            .attr("config_code", code)
            .attr("config_value", value)
            .build()
    }

    /// A props-shaped response: many small children under one child of the
    /// root, with one child much larger than the inflate window.
    fn props_iq(count: usize) -> Node {
        let mut children: Vec<Node> = (0..count as u32)
            .map(|i| prop(1000 + i, if i % 3 == 0 { "true" } else { "17" }))
            .collect();
        let big_value = "x".repeat(3 * FRAME_INFLATE_CHUNK);
        children.insert(count / 2, prop(7, &big_value));
        let props = NodeBuilder::new("props")
            .attr("protocol", "1")
            .attr("hash", "abc")
            .children(children)
            .build();
        NodeBuilder::new("iq")
            .attr("type", "result")
            .attr("id", "42")
            .attr("from", "s.whatsapp.net")
            .children(vec![props])
            .build()
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn walks_children_one_at_a_time_in_both_forms() {
        let node = props_iq(2000);
        let expected: Vec<Node> = match &node.content {
            Some(NodeContent::Nodes(c)) => match &c[0].content {
                Some(NodeContent::Nodes(props)) => props.clone(),
                _ => unreachable!(),
            },
            _ => unreachable!(),
        };
        for (form, packed) in both_forms(&node) {
            let mut stream = NodeStream::from_packed(&packed).unwrap();
            assert_eq!(stream.is_compressed(), form == "compressed");
            let root = stream.open().unwrap().expect("root");
            assert_eq!(root.tag, "iq");
            assert_eq!(root.attr_str("id").as_deref(), Some("42"));
            assert_eq!(root.child_count(), 1);
            let props = stream.open().unwrap().expect("props");
            assert_eq!(props.tag, "props");
            assert_eq!(props.attr_str("hash").as_deref(), Some("abc"));
            assert_eq!(props.child_count(), expected.len());
            assert_eq!(stream.depth(), 2);

            let mut seen = 0;
            while let Some(child) = stream.next_child().unwrap() {
                assert_eq!(child.to_owned(), expected[seen], "{form}: child {seen}");
                seen += 1;
            }
            assert_eq!(seen, expected.len(), "{form}");
            stream.close().unwrap();
            assert!(
                stream.next_child().unwrap().is_none(),
                "{form}: iq has one child"
            );
            stream.close().unwrap();
            assert!(
                stream.open().unwrap().is_none(),
                "{form}: nothing after the root"
            );
            stream.finish().unwrap();
        }
    }

    #[test]
    fn close_skips_unread_children_and_continues_with_siblings() {
        let node = NodeBuilder::new("iq")
            .children(vec![
                NodeBuilder::new("first")
                    .children((0..50).map(|i| prop(i, "v")))
                    .build(),
                NodeBuilder::new("second")
                    .children(vec![prop(1, "a"), prop(2, "b")])
                    .build(),
                NodeBuilder::new("third").attr("k", "v").build(),
            ])
            .build();
        for (form, packed) in both_forms(&node) {
            let mut stream = NodeStream::from_packed(&packed).unwrap();
            stream.open().unwrap().expect("root");
            let first = stream.open().unwrap().expect("first");
            assert_eq!(first.tag, "first");
            // Read three, leave the rest.
            for _ in 0..3 {
                stream.next_child().unwrap().expect("child");
            }
            stream.close().unwrap();

            let second = stream.next_child().unwrap().expect("second, whole");
            assert_eq!(second.tag, "second");
            assert_eq!(second.children().map(|c| c.len()), Some(2));

            let third = stream.open().unwrap().expect("third");
            assert_eq!(third.tag, "third");
            assert_eq!(third.attr_str("k").as_deref(), Some("v"));
            assert_eq!(third.content, OpenContent::None);
            assert!(stream.next_child().unwrap().is_none());
            stream.close().unwrap();
            assert!(stream.next_child().unwrap().is_none(), "{form}");
            stream.finish().unwrap();
        }
    }

    #[test]
    fn scalar_content_and_empty_nodes_open_without_children() {
        let node = NodeBuilder::new("message")
            .children(vec![
                NodeBuilder::new("body").string_content("hello").build(),
                NodeBuilder::new("enc").bytes(vec![1, 2, 3]).build(),
                NodeBuilder::new("empty").build(),
            ])
            .build();
        for (_, packed) in both_forms(&node) {
            let mut stream = NodeStream::from_packed(&packed).unwrap();
            stream.open().unwrap().expect("root");
            let body = stream.open().unwrap().expect("body");
            // The tree decoder holds string content as its wire bytes too.
            assert!(matches!(
                &body.content,
                OpenContent::Scalar(NodeContentRef::Bytes(b)) if b.as_ref() == b"hello"
            ));
            assert!(stream.next_child().unwrap().is_none());
            stream.close().unwrap();
            let enc = stream.open().unwrap().expect("enc");
            assert!(matches!(
                &enc.content,
                OpenContent::Scalar(NodeContentRef::Bytes(b)) if b.as_ref() == [1, 2, 3]
            ));
            stream.close().unwrap();
            let empty = stream.open().unwrap().expect("empty");
            assert_eq!(empty.content, OpenContent::None);
            stream.close().unwrap();
            stream.finish().unwrap();
        }
    }

    /// `finish` applies the tree decoder's strictness: trailing bytes behind the
    /// root, and a compressed stream without its trailer, are errors.
    #[test]
    #[cfg_attr(miri, ignore)]
    fn finish_rejects_leftover_bytes_and_a_truncated_stream() {
        let node = props_iq(10);
        let mut packed = marshal(&node).unwrap();
        packed.push(0);
        let mut stream = NodeStream::from_packed(&packed).unwrap();
        stream.open().unwrap();
        assert!(matches!(stream.finish(), Err(BinaryError::LeftoverData(1))));

        // Larger, so a cut inside the stream still leaves the head decodable.
        let node = props_iq(300);
        let compressed = compress(&marshal(&node).unwrap()[1..]);
        let truncated = &compressed[..compressed.len() - 2];
        let mut stream = NodeStream::from_packed(truncated).unwrap();
        stream.open().unwrap();
        stream.open().unwrap();
        while stream.next_child().unwrap().is_some() {}
        assert!(matches!(stream.finish(), Err(BinaryError::Zlib(_))));

        // A stream cut inside a child fails on that child, as `UnexpectedEof`.
        let truncated = &compressed[..compressed.len() * 9 / 10];
        let mut stream = NodeStream::from_packed(truncated).unwrap();
        stream.open().unwrap();
        stream.open().unwrap();
        let mut result = Ok(());
        loop {
            match stream.next_child() {
                Ok(Some(_)) => {}
                Ok(None) => break,
                Err(e) => {
                    result = Err(e);
                    break;
                }
            }
        }
        assert!(
            matches!(result, Err(BinaryError::UnexpectedEof)),
            "{result:?}"
        );

        // An untouched stream is walked whole by `finish` and still validated.
        let mut stream = NodeStream::from_packed(&compressed).unwrap();
        stream.finish().unwrap();
    }

    /// A stream that only looked at the root's head can still hand over the
    /// whole payload, identical to what the one-shot unpack produces.
    #[test]
    #[cfg_attr(miri, ignore)]
    fn into_inflated_after_peeking_the_root_matches_unpack() {
        let node = props_iq(500);
        let packed = marshal(&node).unwrap();
        let compressed = compress(&packed[1..]);

        let mut stream = NodeStream::from_packed(&compressed).unwrap();
        let root = stream.open().unwrap().expect("root");
        assert_eq!(root.tag, "iq");
        let whole = stream.into_inflated().expect("compressed").unwrap();
        assert_eq!(&whole[..], &packed[1..]);

        // Plain payloads and consumed streams have nothing to hand over.
        let mut stream = NodeStream::from_packed(&packed).unwrap();
        stream.open().unwrap();
        assert!(stream.into_inflated().is_none());
        let mut stream = NodeStream::from_packed(&compressed).unwrap();
        stream.open().unwrap();
        stream.open().unwrap();
        assert!(stream.into_inflated().is_none());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn depth_is_capped_like_the_tree_decoder() {
        let mut node = NodeBuilder::new("leaf").build();
        for _ in 0..(MAX_NODE_DEPTH + 2) {
            node = NodeBuilder::new("n").children(vec![node]).build();
        }
        let packed = marshal(&node).unwrap();
        let mut stream = NodeStream::from_packed(&packed).unwrap();
        let mut result = Ok(());
        loop {
            match stream.open() {
                Ok(Some(_)) => {}
                Ok(None) => break,
                Err(e) => {
                    result = Err(e);
                    break;
                }
            }
        }
        assert!(matches!(result, Err(BinaryError::MaxDepthExceeded)));
    }

    #[test]
    fn empty_payload_is_refused() {
        assert!(matches!(
            NodeStream::from_packed(&[]),
            Err(BinaryError::EmptyData)
        ));
    }

    /// A child the stream skips unread is still held to the tree decoder's
    /// validation: a string that is not UTF-8, or a packed digit outside the
    /// nibble alphabet, fails `close` and `finish` as it fails `unmarshal_ref`.
    #[test]
    fn skipping_a_malformed_child_fails_like_the_tree_decoder() {
        let node = NodeBuilder::new("iq")
            .children(vec![
                NodeBuilder::new("first").build(),
                prop(1, "hello"),
                prop(2, "12345"),
            ])
            .build();
        let packed = marshal(&node).unwrap();

        // "hello" is a BINARY_8 string; a 0xFF inside it is not UTF-8.
        let mut bad_utf8 = packed.clone();
        let at = bad_utf8
            .windows(5)
            .position(|w| w == b"hello")
            .expect("the value is stored verbatim");
        bad_utf8[at] = 0xFF;
        // "12345" is packed as nibbles 0x12 0x34 0x5F; nibble 0xC is not a digit.
        let mut bad_nibble = packed.clone();
        let at = bad_nibble
            .windows(3)
            .position(|w| w == [0x12, 0x34, 0x5F])
            .expect("the digits are nibble-packed");
        bad_nibble[at] = 0xC2;

        for (name, bytes) in [("utf-8", bad_utf8), ("nibble", bad_nibble)] {
            assert!(
                crate::marshal::unmarshal_ref(&bytes[1..]).is_err(),
                "{name}: the tree decoder must reject the fixture"
            );
            let mut stream = NodeStream::from_packed(&bytes).unwrap();
            stream.open().unwrap().expect("root");
            stream.open().unwrap().expect("first");
            stream.close().unwrap();
            assert!(
                stream.close().is_err(),
                "{name}: skipping the rest must fail"
            );

            let mut stream = NodeStream::from_packed(&bytes).unwrap();
            assert!(stream.finish().is_err(), "{name}: finish must fail");
        }
    }

    /// Two more shapes the tree decoder refuses (`InvalidNode` for a node
    /// without a tag, `NonStringKey` for an attribute key that is not a
    /// string), hand-built since no encoder writes them, and skipped by the
    /// stream the same way.
    #[test]
    fn skipping_a_child_with_no_tag_or_a_non_string_key_fails_like_the_tree_decoder() {
        use crate::token::{BINARY_8, LIST_8, LIST_EMPTY};
        let root_head = [LIST_8, 2, BINARY_8, 2, b'i', b'q', LIST_8, 2];
        let first = [LIST_8, 1, BINARY_8, 5, b'f', b'i', b'r', b's', b't'];
        let no_tag = vec![LIST_8, 1, LIST_EMPTY];
        let bad_key = vec![LIST_8, 3, BINARY_8, 1, b'p', LIST_EMPTY, BINARY_8, 1, b'v'];

        for (name, child, expected) in [
            ("no tag", no_tag, BinaryError::InvalidNode),
            ("non-string key", bad_key, BinaryError::NonStringKey),
        ] {
            let mut packed = vec![crate::util::FORMAT_PLAIN];
            packed.extend_from_slice(&root_head);
            packed.extend_from_slice(&first);
            packed.extend_from_slice(&child);
            assert!(
                matches!(&crate::marshal::unmarshal_ref(&packed[1..]), Err(e) if std::mem::discriminant(e) == std::mem::discriminant(&expected)),
                "{name}: the tree decoder must reject the fixture with {expected:?}"
            );

            let mut stream = NodeStream::from_packed(&packed).unwrap();
            stream.open().unwrap().expect("root");
            stream.open().unwrap().expect("first");
            stream.close().unwrap();
            let err = stream.close().expect_err("skipping the rest must fail");
            assert_eq!(
                std::mem::discriminant(&err),
                std::mem::discriminant(&expected),
                "{name}: {err:?}"
            );
            let mut stream = NodeStream::from_packed(&packed).unwrap();
            assert!(stream.finish().is_err(), "{name}: finish must fail");
        }
    }
}

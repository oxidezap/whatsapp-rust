use crate::error::{BinaryError, Result};
use crate::jid::{JidRef, push_jid_to_compact};
use crate::node::{AttrsRef, NodeContentRef, NodeRef, NodeStr, ValueRef};
use crate::token;
use compact_str::CompactString;
use std::borrow::Cow;

/// Format a JidRef directly into CompactString using direct push operations,
/// bypassing `fmt::Display` and `dyn Write` dispatch entirely.
fn jid_ref_to_compact(j: &JidRef<'_>) -> CompactString {
    let mut s = CompactString::with_capacity(j.user.len() + 20);
    push_jid_to_compact(&j.user, j.server, j.agent, j.device, &mut s);
    s
}

/// Each byte's two output characters, so unpacking is one load and one 2-byte
/// store per input byte instead of two shifts, two lookups and two bounds
/// checks. Packed values on the wire (a 13-digit phone number, a 20-character
/// id) are shorter than the SIMD chunk above, so this is the path that runs.
static HEX_PAIRS: [[u8; 2]; 256] = {
    const HEX: [u8; 16] = *b"0123456789ABCDEF";
    let mut table = [[0u8; 2]; 256];
    let mut i = 0;
    while i < 256 {
        table[i] = [HEX[i >> 4], HEX[i & 0x0F]];
        i += 1;
    }
    table
};

/// Nibble values 12, 13 and 14 encode nothing, so they are marked and the byte
/// carrying one falls back to the scalar path, which reports which half was
/// bad. `NIBBLE_INVALID` cannot collide with an output character.
const NIBBLE_INVALID: u8 = 0xFF;
static NIBBLE_PAIRS: [[u8; 2]; 256] = {
    const fn glyph(nibble: usize) -> u8 {
        match nibble {
            0..=9 => b'0' + nibble as u8,
            10 => b'-',
            11 => b'.',
            15 => 0,
            _ => NIBBLE_INVALID,
        }
    }
    let mut table = [[0u8; 2]; 256];
    let mut i = 0;
    while i < 256 {
        table[i] = [glyph(i >> 4), glyph(i & 0x0F)];
        i += 1;
    }
    table
};

/// Node-nesting cap rejecting deep-`LIST` frames that would overflow the stack via
/// unbounded `read_node_ref` recursion (real WA trees are well under 20 levels).
const MAX_NODE_DEPTH: usize = 128;

pub(crate) struct Decoder<'a> {
    data: &'a [u8],
    position: usize,
}

impl<'a> Decoder<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }

    pub(crate) fn is_finished(&self) -> bool {
        self.position >= self.data.len()
    }

    pub(crate) fn bytes_left(&self) -> usize {
        self.data.len() - self.position
    }

    #[inline(always)]
    fn check_eos(&self, len: usize) -> Result<()> {
        if self.bytes_left() >= len {
            Ok(())
        } else {
            Err(BinaryError::UnexpectedEof)
        }
    }

    #[inline(always)]
    fn read_u8(&mut self) -> Result<u8> {
        self.check_eos(1)?;
        let position = self.position;
        self.position += 1;
        Ok(self.data[position])
    }

    #[inline(always)]
    fn read_u16_be(&mut self) -> Result<u16> {
        self.check_eos(2)?;
        let position = self.position;
        self.position += 2;
        Ok(u16::from_be_bytes([
            self.data[position],
            self.data[position + 1],
        ]))
    }

    #[inline(always)]
    fn read_u20_be(&mut self) -> Result<u32> {
        self.check_eos(3)?;
        let position = self.position;
        self.position += 3;
        let bytes = [
            self.data[position],
            self.data[position + 1],
            self.data[position + 2],
        ];
        Ok(((bytes[0] as u32 & 0x0F) << 16) | ((bytes[1] as u32) << 8) | (bytes[2] as u32))
    }

    #[inline(always)]
    fn read_u32_be(&mut self) -> Result<u32> {
        self.check_eos(4)?;
        let position = self.position;
        self.position += 4;
        Ok(u32::from_be_bytes([
            self.data[position],
            self.data[position + 1],
            self.data[position + 2],
            self.data[position + 3],
        ]))
    }

    #[inline(always)]
    fn read_bytes(&mut self, len: usize) -> Result<&'a [u8]> {
        self.check_eos(len)?;
        let start = self.position;
        let end = start + len;
        self.position = end;
        Ok(&self.data[start..end])
    }

    #[inline(always)]
    fn read_string(&mut self, len: usize) -> Result<NodeStr<'a>> {
        let bytes = self.read_bytes(len)?;
        // smoothutf8 has a faster fast-path for the short strings dominating the
        // wire (tags, attribute values, JID parts). It only answers valid/invalid,
        // so the cold rejection path reuses std to recover the precise Utf8Error.
        if let Some(s) = smoothutf8::from_utf8(bytes) {
            Ok(NodeStr::Borrowed(s))
        } else {
            match std::str::from_utf8(bytes) {
                Ok(s) => Ok(NodeStr::Borrowed(s)),
                Err(e) => Err(BinaryError::InvalidUtf8(e)),
            }
        }
    }

    #[inline(always)]
    fn read_list_size(&mut self, tag: u8) -> Result<usize> {
        match tag {
            token::LIST_EMPTY => Ok(0),
            token::LIST_8 => self.read_u8().map(|v| v as usize),
            token::LIST_16 => self.read_u16_be().map(|v| v as usize),
            _ => Err(BinaryError::InvalidToken(tag)),
        }
    }

    fn read_jid_pair(&mut self) -> Result<JidRef<'a>> {
        let user = self.read_value_as_string()?.unwrap_or_default();
        let server_str = self.read_value_as_string()?.unwrap_or_default();
        let server = crate::jid::Server::try_from(server_str.as_ref()).map_err(|_| {
            BinaryError::AttrParse(format!("JID_PAIR unknown server: {}", server_str))
        })?;
        Ok(JidRef {
            user,
            server,
            agent: 0,
            device: 0,
            integrator: 0,
        })
    }

    fn read_ad_jid(&mut self) -> Result<JidRef<'a>> {
        let agent = self.read_u8()?;
        let device = self.read_u8()? as u16;
        let Some(user) = self.read_value_as_string()? else {
            return Err(BinaryError::InvalidNode);
        };

        // Domain type mapping must mirror WA Web decodeJidU.
        // WA Web: 0=WHATSAPP, 1=LID, even+bit7=HOSTED, 129=HOSTED_LID, else throw.
        let server = match agent {
            0 => crate::jid::Server::Pn,
            1 => crate::jid::Server::Lid,
            128 => crate::jid::Server::Hosted,
            129 => crate::jid::Server::HostedLid,
            n if (n & 128) != 0 && (n & 1) == 0 => crate::jid::Server::Hosted,
            _ => {
                return Err(BinaryError::AttrParse(format!(
                    "read_ad_jid - Invalid domain type encoding {agent}"
                )));
            }
        };

        Ok(JidRef {
            user,
            server,
            // The domain byte is not an agent — it is `server` in wire form, and
            // `write_jid_*` re-derives it from `server` on the way out. Keeping a
            // second copy here put a field in the derived `PartialEq`/`Hash` that
            // only wire-decoded JIDs ever carry, so the same JID compared unequal
            // depending on whether it came off the wire or out of the store (which
            // holds JIDs as text, where `Display` suppresses the agent anyway).
            agent: 0,
            device,
            integrator: 0,
        })
    }

    fn read_interop_jid(&mut self) -> Result<JidRef<'a>> {
        let Some(user) = self.read_value_as_string()? else {
            return Err(BinaryError::InvalidNode);
        };
        let device = self.read_u16_be()?;
        let integrator = self.read_u16_be()?;
        let server_str = self.read_value_as_string()?.unwrap_or_default();
        if server_str.as_ref() != crate::jid::INTEROP_SERVER {
            return Err(BinaryError::InvalidNode);
        }
        Ok(JidRef {
            user,
            server: crate::jid::Server::Interop,
            device,
            integrator,
            agent: 0,
        })
    }

    fn read_fb_jid(&mut self) -> Result<JidRef<'a>> {
        let Some(user) = self.read_value_as_string()? else {
            return Err(BinaryError::InvalidNode);
        };
        let device = self.read_u16_be()?;
        let server_str = self.read_value_as_string()?.unwrap_or_default();
        if server_str.as_ref() != crate::jid::MESSENGER_SERVER {
            return Err(BinaryError::InvalidNode);
        }
        Ok(JidRef {
            user,
            server: crate::jid::Server::Messenger,
            device,
            agent: 0,
            integrator: 0,
        })
    }

    fn read_value_as_string(&mut self) -> Result<Option<NodeStr<'a>>> {
        let tag = self.read_u8()?;
        self.read_value_as_string_from_tag(tag)
    }

    #[inline(always)]
    fn read_value_as_string_from_tag(&mut self, tag: u8) -> Result<Option<NodeStr<'a>>> {
        match tag {
            token::LIST_EMPTY => Ok(None),
            token::BINARY_8 => {
                let size = self.read_u8()? as usize;
                self.read_string(size).map(Some)
            }
            token::BINARY_20 => {
                let size = self.read_u20_be()? as usize;
                self.read_string(size).map(Some)
            }
            token::BINARY_32 => {
                let size = self.read_u32_be()? as usize;
                self.read_string(size).map(Some)
            }
            token::JID_PAIR => self
                .read_jid_pair()
                .map(|j| Some(NodeStr::Owned(jid_ref_to_compact(&j)))),
            token::AD_JID => self
                .read_ad_jid()
                .map(|j| Some(NodeStr::Owned(jid_ref_to_compact(&j)))),
            token::INTEROP_JID => self
                .read_interop_jid()
                .map(|j| Some(NodeStr::Owned(jid_ref_to_compact(&j)))),
            token::FB_JID => self
                .read_fb_jid()
                .map(|j| Some(NodeStr::Owned(jid_ref_to_compact(&j)))),
            token::NIBBLE_8 | token::HEX_8 => {
                self.read_packed(tag).map(|s| Some(NodeStr::Owned(s)))
            }
            tag @ token::DICTIONARY_0..=token::DICTIONARY_3 => {
                let index = self.read_u8()?;
                match token::get_double_token(tag - token::DICTIONARY_0, index) {
                    Some(s) => Ok(Some(NodeStr::Borrowed(s))),
                    None => Err(BinaryError::InvalidToken(tag)),
                }
            }
            _ => match token::get_single_token(tag) {
                Some(s) => Ok(Some(NodeStr::Borrowed(s))),
                None => Err(BinaryError::InvalidToken(tag)),
            },
        }
    }

    fn read_value(&mut self) -> Result<Option<ValueRef<'a>>> {
        let tag = self.read_u8()?;
        match tag {
            token::LIST_EMPTY => Ok(None),
            token::BINARY_8 => {
                let size = self.read_u8()? as usize;
                self.read_string(size).map(|s| Some(ValueRef::String(s)))
            }
            token::BINARY_20 => {
                let size = self.read_u20_be()? as usize;
                self.read_string(size).map(|s| Some(ValueRef::String(s)))
            }
            token::BINARY_32 => {
                let size = self.read_u32_be()? as usize;
                self.read_string(size).map(|s| Some(ValueRef::String(s)))
            }
            token::JID_PAIR => self.read_jid_pair().map(|j| Some(ValueRef::Jid(j))),
            token::AD_JID => self.read_ad_jid().map(|j| Some(ValueRef::Jid(j))),
            token::INTEROP_JID => self.read_interop_jid().map(|j| Some(ValueRef::Jid(j))),
            token::FB_JID => self.read_fb_jid().map(|j| Some(ValueRef::Jid(j))),
            token::NIBBLE_8 | token::HEX_8 => self
                .read_packed(tag)
                .map(|s| Some(ValueRef::String(NodeStr::Owned(s)))),
            tag @ token::DICTIONARY_0..=token::DICTIONARY_3 => {
                let index = self.read_u8()?;
                match token::get_double_token(tag - token::DICTIONARY_0, index) {
                    Some(s) => Ok(Some(ValueRef::String(NodeStr::Borrowed(s)))),
                    None => Err(BinaryError::InvalidToken(tag)),
                }
            }
            _ => match token::get_single_token(tag) {
                Some(s) => Ok(Some(ValueRef::String(NodeStr::Borrowed(s)))),
                None => Err(BinaryError::InvalidToken(tag)),
            },
        }
    }

    /// Decode packed nibble/hex into a stack buffer, then create CompactString.
    /// Max unpacked length is 254 bytes (127 packed × 2), so the stack buffer
    /// is always sufficient. Short values (≤24 bytes) are stored inline.
    fn read_packed(&mut self, tag: u8) -> Result<CompactString> {
        let packed_len_byte = self.read_u8()?;
        let is_half_byte = (packed_len_byte & 0x80) != 0;
        let len = (packed_len_byte & 0x7F) as usize;

        if len == 0 {
            return Ok(CompactString::default());
        }

        let packed_data = self.read_bytes(len)?;
        let mut buf = [0u8; 254];
        let mut pos = 0;

        match tag {
            token::HEX_8 => Self::decode_packed_hex(packed_data, &mut buf, &mut pos),
            token::NIBBLE_8 => Self::decode_packed_nibble(packed_data, &mut buf, &mut pos)?,
            _ => return Err(BinaryError::InvalidToken(tag)),
        }

        if is_half_byte && pos > 0 {
            pos -= 1;
        }

        // Unlike `read_string`, which validates bytes that came off the wire,
        // this validates bytes the tables above just wrote, so it can never
        // fail. Keeping a check at all is cheap insurance against a future
        // table edit; smoothutf8 is the same validator the wire path uses.
        let s = smoothutf8::from_utf8(&buf[..pos]).expect("packed decode produced non-ASCII");
        Ok(CompactString::from(s))
    }

    // Deliberately scalar. A vectorised version of this loop lived here until
    // it was measured against the table: `HEX_PAIRS[byte]` is one 2-byte load
    // per input byte, and a shuffle/interleave/store sequence does not beat
    // that. Under callgrind the SIMD path cost 4.5% more instructions on a
    // 20-character id and 9.7% more on a 32-character one, and enabling real
    // `pshufb` (`-Ctarget-cpu=x86-64-v2`) only narrowed the loss to 7.0%.
    // Packed payloads are ids and phone numbers, so the loop also needed a
    // 31-character string before it engaged at all.
    #[inline]
    fn decode_packed_hex(packed_data: &[u8], out: &mut [u8], pos: &mut usize) {
        let written = packed_data.len() * 2;
        for (slot, &byte) in out[*pos..*pos + written]
            .chunks_exact_mut(2)
            .zip(packed_data)
        {
            slot.copy_from_slice(&HEX_PAIRS[byte as usize]);
        }
        *pos += written;
    }

    // Scalar for the same reason as `decode_packed_hex`, and more so: the
    // vector version had to validate every lane against the two legal
    // out-of-range nibbles before it could shuffle, then fall back to this
    // loop anyway whenever a lane failed.
    #[inline]
    fn decode_packed_nibble(packed_data: &[u8], out: &mut [u8], pos: &mut usize) -> Result<()> {
        let written = packed_data.len() * 2;
        for (slot, &byte) in out[*pos..*pos + written]
            .chunks_exact_mut(2)
            .zip(packed_data)
        {
            let pair = NIBBLE_PAIRS[byte as usize];
            if pair[0] == NIBBLE_INVALID || pair[1] == NIBBLE_INVALID {
                // Report the bad half in the order the byte carries it.
                Self::unpack_nibble((byte & 0xF0) >> 4)?;
                Self::unpack_nibble(byte & 0x0F)?;
            }
            slot.copy_from_slice(&pair);
        }
        *pos += written;

        Ok(())
    }

    #[inline(always)]
    fn unpack_nibble(value: u8) -> Result<u8> {
        match value {
            0..=9 => Ok(b'0' + value),
            10 => Ok(b'-'),
            11 => Ok(b'.'),
            15 => Ok(0),
            _ => Err(BinaryError::InvalidToken(value)),
        }
    }

    fn read_attributes(&mut self, size: usize) -> Result<AttrsRef<'a>> {
        if size == 0 {
            return Ok(AttrsRef::Empty);
        }
        let mut v = Vec::with_capacity(size);
        for _ in 0..size {
            let Some(key) = self.read_value_as_string()? else {
                return Err(BinaryError::NonStringKey);
            };
            let value = match self.read_value()? {
                Some(v) => v,
                None => ValueRef::String(NodeStr::Borrowed("")),
            };
            v.push((key, value));
        }
        Ok(AttrsRef::from_vec(v))
    }

    fn read_content(&mut self, depth: usize) -> Result<Option<NodeContentRef<'a>>> {
        let tag = self.read_u8()?;
        self.read_content_from_tag(tag, depth)
    }

    #[inline(always)]
    fn read_content_from_tag(
        &mut self,
        tag: u8,
        depth: usize,
    ) -> Result<Option<NodeContentRef<'a>>> {
        match tag {
            token::LIST_EMPTY => Ok(None),

            token::LIST_8 | token::LIST_16 => {
                let size = self.read_list_size(tag)?;
                let mut nodes = Vec::with_capacity(size);
                for _ in 0..size {
                    nodes.push(self.read_node_ref_at(depth + 1)?);
                }
                Ok(Some(NodeContentRef::Nodes(nodes.into_boxed_slice())))
            }

            token::BINARY_8 => {
                let len = self.read_u8()? as usize;
                let bytes = self.read_bytes(len)?;
                Ok(Some(NodeContentRef::Bytes(Cow::Borrowed(bytes))))
            }
            token::BINARY_20 => {
                let len = self.read_u20_be()? as usize;
                let bytes = self.read_bytes(len)?;
                Ok(Some(NodeContentRef::Bytes(Cow::Borrowed(bytes))))
            }
            token::BINARY_32 => {
                let len = self.read_u32_be()? as usize;
                let bytes = self.read_bytes(len)?;
                Ok(Some(NodeContentRef::Bytes(Cow::Borrowed(bytes))))
            }

            _ => {
                let string_content = self.read_value_as_string_from_tag(tag)?;

                match string_content {
                    Some(s) => Ok(Some(NodeContentRef::String(s))),
                    None => Ok(None),
                }
            }
        }
    }

    pub(crate) fn read_node_ref(&mut self) -> Result<NodeRef<'a>> {
        self.read_node_ref_at(0)
    }

    fn read_node_ref_at(&mut self, depth: usize) -> Result<NodeRef<'a>> {
        // Reject before recursing, so a hostile deep-LIST frame errors instead of
        // aborting the process on a stack overflow.
        if depth >= MAX_NODE_DEPTH {
            return Err(BinaryError::MaxDepthExceeded);
        }
        let tag = self.read_u8()?;
        let list_size = self.read_list_size(tag)?;
        if list_size == 0 {
            return Err(BinaryError::InvalidNode);
        }

        let Some(tag) = self.read_value_as_string()? else {
            return Err(BinaryError::InvalidNode);
        };

        let attr_count = (list_size - 1) / 2;
        let has_content = list_size.is_multiple_of(2);

        let attrs = self.read_attributes(attr_count)?;
        let content = if has_content {
            self.read_content(depth)?
        } else {
            None
        };

        Ok(NodeRef {
            tag,
            attrs,
            content,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::{Attrs, Node};

    type TestResult = Result<()>;

    #[test]
    fn test_decode_node() -> TestResult {
        let node = Node::new(
            "message",
            Attrs::new(),
            Some(crate::node::NodeContent::String("receipt".into())),
        );

        let mut buffer = Vec::new();
        {
            let mut encoder = crate::encoder::Encoder::new(std::io::Cursor::new(&mut buffer))?;
            encoder.write_node(&node)?;
        }

        let mut decoder = Decoder::new(&buffer[1..]);
        let decoded = decoder.read_node_ref().unwrap();

        assert_eq!(decoded.tag, "message");
        assert!(decoded.attrs.is_empty());
        match &decoded.content {
            Some(content) => match content {
                NodeContentRef::String(s) => assert_eq!(s, "receipt"),
                _ => panic!("Expected string content"),
            },
            None => panic!("Expected content"),
        }
        Ok(())
    }

    #[test]
    fn test_decode_nibble_packing() -> TestResult {
        let test_str = "-.0123456789";
        let node = Node::new(
            "test",
            Attrs::new(),
            Some(crate::node::NodeContent::String(test_str.into())),
        );

        let mut buffer = Vec::new();
        {
            let mut encoder = crate::encoder::Encoder::new(std::io::Cursor::new(&mut buffer))?;
            encoder.write_node(&node)?;
        }

        let mut decoder = Decoder::new(&buffer[1..]);
        let decoded = decoder.read_node_ref().unwrap();

        assert_eq!(decoded.tag, "test");
        assert!(decoded.attrs.is_empty());
        match &decoded.content {
            Some(content) => match content {
                NodeContentRef::String(s) => assert_eq!(s, test_str),
                _ => panic!("Expected string content"),
            },
            None => panic!("Expected content"),
        }
        Ok(())
    }

    #[test]
    fn test_invalid_nibble_rejection() {
        let invalid_data = vec![1, 0xC0];

        let mut decoder = Decoder::new(&invalid_data);
        let result = decoder.read_packed(token::NIBBLE_8);
        assert!(
            result.is_err(),
            "Expected error for invalid nibble 12, got: {:?}",
            result
        );

        if let Err(BinaryError::InvalidToken(invalid_nibble)) = result {
            assert_eq!(invalid_nibble, 12, "Expected invalid nibble 12");
        } else {
            panic!("Expected InvalidToken error, got: {:?}", result);
        }
    }

    /// Test empty input returns appropriate error
    #[test]
    fn test_empty_input() {
        let mut decoder = Decoder::new(&[]);
        let result = decoder.read_node_ref();
        assert!(result.is_err());
    }

    /// Test truncated u16 read
    #[test]
    fn test_truncated_u16() {
        // Only one byte when u16 expected
        let data = vec![0x42];
        let mut decoder = Decoder::new(&data);
        let result = decoder.read_u16_be();
        assert!(result.is_err());
        if let Err(BinaryError::UnexpectedEof) = result {
            // Expected
        } else {
            panic!("Expected UnexpectedEof, got: {:?}", result);
        }
    }

    /// Test truncated u20 read
    #[test]
    fn test_truncated_u20() {
        // Only two bytes when u20 (3 bytes) expected
        let data = vec![0x42, 0x43];
        let mut decoder = Decoder::new(&data);
        let result = decoder.read_u20_be();
        assert!(result.is_err());
    }

    /// Test truncated u32 read
    #[test]
    fn test_truncated_u32() {
        // Only three bytes when u32 expected
        let data = vec![0x42, 0x43, 0x44];
        let mut decoder = Decoder::new(&data);
        let result = decoder.read_u32_be();
        assert!(result.is_err());
    }

    /// Test BINARY_8 with length larger than remaining buffer
    #[test]
    fn test_binary8_length_exceeds_buffer() {
        // BINARY_8 token, length 100, but only 5 bytes of data
        let data = vec![token::BINARY_8, 100, 1, 2, 3, 4, 5];
        let mut decoder = Decoder::new(&data);
        let result = decoder.read_value_as_string();
        assert!(result.is_err());
    }

    /// Test BINARY_20 with length larger than remaining buffer
    #[test]
    fn test_binary20_length_exceeds_buffer() {
        // BINARY_20 token, length encoded as 256, but only a few bytes of data
        let data = vec![token::BINARY_20, 0x00, 0x01, 0x00, 1, 2, 3]; // length = 256
        let mut decoder = Decoder::new(&data);
        let result = decoder.read_value_as_string();
        assert!(result.is_err());
    }

    /// Test LIST_8 with size larger than remaining data
    #[test]
    fn test_list8_size_exceeds_data() {
        // LIST_8 token, size 10, but not enough data for 10 nodes
        let data = vec![token::LIST_8, 10, 1]; // Only 1 byte of data for nodes
        let mut decoder = Decoder::new(&data);
        let result = decoder.read_node_ref();
        assert!(result.is_err());
    }

    /// Attribute storage capacity must not change how a broken attribute list
    /// is rejected: a truncated pair, a non-string key and a declared count the
    /// frame cannot back all fail the same way they did before.
    #[test]
    fn malformed_attribute_lists_keep_their_errors() {
        // <message> claiming one attribute, cut off after the key.
        let truncated = [
            token::LIST_8,
            3,
            token::DICTIONARY_0,
            0,
            token::BINARY_8,
            2,
            b'i',
            b'd',
        ];
        assert!(matches!(
            Decoder::new(&truncated).read_node_ref(),
            Err(BinaryError::UnexpectedEof)
        ));

        // An empty list where a string key is required.
        let non_string_key = [token::LIST_8, 3, token::DICTIONARY_0, 0, token::LIST_EMPTY];
        assert!(matches!(
            Decoder::new(&non_string_key).read_node_ref(),
            Err(BinaryError::NonStringKey)
        ));

        // Declares 4 attributes, carries none.
        let overlong_count = [token::LIST_8, 9, token::DICTIONARY_0, 0];
        assert!(matches!(
            Decoder::new(&overlong_count).read_node_ref(),
            Err(BinaryError::UnexpectedEof)
        ));

        // A list size of zero has no room even for the tag.
        assert!(matches!(
            Decoder::new(&[token::LIST_EMPTY]).read_node_ref(),
            Err(BinaryError::InvalidNode)
        ));
    }

    /// Test invalid token value
    #[test]
    fn test_invalid_token() {
        // Use a token value that's reserved and not valid as a string token
        // e.g., AD_JID (247) followed by insufficient data
        let data = vec![token::AD_JID]; // No data following
        let mut decoder = Decoder::new(&data);
        let result = decoder.read_value_as_string();
        assert!(result.is_err());
    }

    #[test]
    fn test_ad_jid_rejects_invalid_domain_type() {
        let data = vec![token::AD_JID, 2, 1, token::BINARY_8, 3, b'1', b'2', b'3'];
        let mut decoder = Decoder::new(&data);
        let result = decoder.read_value_as_string();

        match result {
            Err(BinaryError::AttrParse(msg)) => {
                assert!(
                    msg.contains("Invalid domain type encoding 2"),
                    "unexpected error message: {msg}"
                );
            }
            other => panic!("expected AttrParse for invalid AD_JID domain type, got {other:?}"),
        }
    }

    /// Test read_bytes with exact length
    #[test]
    fn test_read_bytes_exact_length() {
        let data = vec![1, 2, 3, 4, 5];
        let mut decoder = Decoder::new(&data);
        let bytes = decoder.read_bytes(5).unwrap();
        assert_eq!(bytes, &[1, 2, 3, 4, 5]);
        assert!(decoder.is_finished());
    }

    /// Test read_bytes exceeding length
    #[test]
    fn test_read_bytes_exceeding_length() {
        let data = vec![1, 2, 3];
        let mut decoder = Decoder::new(&data);
        let result = decoder.read_bytes(5);
        assert!(result.is_err());
    }

    /// Test u20 encoding/decoding values
    #[test]
    fn test_u20_encoding() {
        // Test value 0
        let data = vec![0x00, 0x00, 0x00];
        let mut decoder = Decoder::new(&data);
        assert_eq!(decoder.read_u20_be().unwrap(), 0);

        // Test value 256 (0x100)
        let data = vec![0x00, 0x01, 0x00];
        let mut decoder = Decoder::new(&data);
        assert_eq!(decoder.read_u20_be().unwrap(), 256);

        // Test value 65536 (0x10000)
        let data = vec![0x01, 0x00, 0x00];
        let mut decoder = Decoder::new(&data);
        assert_eq!(decoder.read_u20_be().unwrap(), 65536);

        // Test max u20 value (0xFFFFF = 1048575)
        let data = vec![0x0F, 0xFF, 0xFF];
        let mut decoder = Decoder::new(&data);
        assert_eq!(decoder.read_u20_be().unwrap(), 1048575);
    }

    /// Test bytes_left tracking
    #[test]
    fn test_bytes_left() {
        let data = vec![1, 2, 3, 4, 5];
        let mut decoder = Decoder::new(&data);

        assert_eq!(decoder.bytes_left(), 5);
        decoder.read_u8().unwrap();
        assert_eq!(decoder.bytes_left(), 4);
        decoder.read_u8().unwrap();
        assert_eq!(decoder.bytes_left(), 3);
        decoder.read_bytes(3).unwrap();
        assert_eq!(decoder.bytes_left(), 0);
        assert!(decoder.is_finished());
    }

    /// Test hex packed string decoding
    #[test]
    fn test_hex_packed_decoding() {
        // Encode "ABCDEF" as hex packed
        // Each byte packs two hex digits
        // A=10, B=11, C=12, D=13, E=14, F=15
        let packed_data = vec![
            3,    // length = 3 bytes = 6 characters
            0xAB, // AB
            0xCD, // CD
            0xEF, // EF
        ];

        let mut decoder = Decoder::new(&packed_data);
        let result = decoder.read_packed(token::HEX_8).unwrap();
        assert_eq!(result, "ABCDEF");
    }

    /// Test nibble packed string with odd length
    #[test]
    fn test_nibble_packed_odd_length() {
        // Encode "123" as nibble packed (odd length = 3)
        // 1=1, 2=2, 3=3, pad=15
        let packed_data = vec![
            0x82, // length = 2 bytes, high bit set for odd
            0x12, // 12
            0x3F, // 3 + pad (15)
        ];

        let mut decoder = Decoder::new(&packed_data);
        let result = decoder.read_packed(token::NIBBLE_8).unwrap();
        assert_eq!(result, "123");
    }

    /// Test empty packed string
    #[test]
    fn test_empty_packed_string() {
        let packed_data = vec![0]; // length = 0

        let mut decoder = Decoder::new(&packed_data);
        let result = decoder.read_packed(token::NIBBLE_8).unwrap();
        assert_eq!(result, "");
    }

    /// Test invalid nibble value 12 (only 0-11, 15 are valid)
    #[test]
    fn test_invalid_nibble_value_12() {
        // 12 (0xC) is not a valid nibble
        let packed_data = vec![1, 0xC0]; // first nibble is 12

        let mut decoder = Decoder::new(&packed_data);
        let result = decoder.read_packed(token::NIBBLE_8);
        assert!(result.is_err());
    }

    /// Test invalid nibble value 13
    #[test]
    fn test_invalid_nibble_value_13() {
        let packed_data = vec![1, 0xD0]; // first nibble is 13

        let mut decoder = Decoder::new(&packed_data);
        let result = decoder.read_packed(token::NIBBLE_8);
        assert!(result.is_err());
    }

    /// Test invalid nibble value 14
    #[test]
    fn test_invalid_nibble_value_14() {
        let packed_data = vec![1, 0xE0]; // first nibble is 14

        let mut decoder = Decoder::new(&packed_data);
        let result = decoder.read_packed(token::NIBBLE_8);
        assert!(result.is_err());
    }

    /// Test deeply nested nodes (recursion safety)
    #[test]
    fn test_nested_nodes() -> TestResult {
        // Create a 50-level deep node structure
        let mut current = Node::new("leaf", Attrs::new(), None);

        for i in 0..50 {
            let tag = format!("level{}", i);
            current = Node::new(
                tag,
                Attrs::new(),
                Some(crate::node::NodeContent::Nodes(vec![current])),
            );
        }

        let mut buffer = Vec::new();
        {
            let mut encoder = crate::encoder::Encoder::new(std::io::Cursor::new(&mut buffer))?;
            encoder.write_node(&current)?;
        }

        let mut decoder = Decoder::new(&buffer[1..]);
        let decoded = decoder.read_node_ref()?;

        // Verify top level tag
        assert_eq!(decoded.tag, "level49");
        Ok(())
    }

    fn encode_nested(levels: usize) -> Vec<u8> {
        let mut current = Node::new("leaf", Attrs::new(), None);
        for i in 0..levels {
            current = Node::new(
                format!("l{i}"),
                Attrs::new(),
                Some(crate::node::NodeContent::Nodes(vec![current])),
            );
        }
        let mut buffer = Vec::new();
        {
            let mut encoder =
                crate::encoder::Encoder::new(std::io::Cursor::new(&mut buffer)).unwrap();
            encoder.write_node(&current).unwrap();
        }
        buffer
    }

    fn is_max_depth_err(buffer: &[u8]) -> bool {
        matches!(
            Decoder::new(&buffer[1..]).read_node_ref(),
            Err(BinaryError::MaxDepthExceeded)
        )
    }

    /// The deepest accepted nesting (a leaf at depth `MAX_NODE_DEPTH - 1`) decodes.
    /// Pins the exact accept side of the cap so a `>`/`>=` flip is caught.
    #[test]
    fn deeply_nested_at_cap_decodes() -> TestResult {
        let buffer = encode_nested(MAX_NODE_DEPTH - 1);
        let decoded = Decoder::new(&buffer[1..]).read_node_ref()?;
        assert_eq!(decoded.tag, format!("l{}", MAX_NODE_DEPTH - 2).as_str());
        Ok(())
    }

    /// Exactly one level past the accepted range is rejected (pins the reject side).
    #[test]
    fn deeply_nested_one_past_cap_is_rejected() {
        assert!(is_max_depth_err(&encode_nested(MAX_NODE_DEPTH)));
    }

    /// A frame nested far past the cap must return `MaxDepthExceeded`, not overflow
    /// the native stack — this test completing at all is the assertion against abort.
    #[test]
    fn deeply_nested_far_past_cap_is_rejected() {
        assert!(is_max_depth_err(&encode_nested(MAX_NODE_DEPTH * 3)));
    }
}

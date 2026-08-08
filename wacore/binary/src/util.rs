use crate::error::{BinaryError, Result};
use crate::zlib_pool::decompress_zlib_pooled;
use bytes::{Buf, Bytes, BytesMut};
use std::borrow::Cow;

/// Protocol frames larger than 16 MiB after decompression are rejected.
const MAX_DECOMPRESSED_SIZE: u64 = 16 * 1024 * 1024;

/// A plaintext frame is a format byte followed by the node bytes the decoder
/// reads. The byte is a flag set with a single defined bit, [`FORMAT_COMPRESSED`];
/// `FORMAT_PLAIN` is the uncompressed form and the only one we emit.
///
/// `Encoder` writes this byte, [`unpack`] strips it, and the decoder never sees
/// it. That is the whole reason a decoded buffer is one byte shorter than the
/// marshal output it came from: [`pack`] is the conversion back.
pub const FORMAT_PLAIN: u8 = 0;

/// Set in the format byte when the node bytes after it are zlib-compressed.
/// Only inbound payloads ever set it.
pub const FORMAT_COMPRESSED: u8 = 2;

/// Check that `data` is a packed payload in the form we produce: a
/// [`FORMAT_PLAIN`] byte followed by at least one node byte.
///
/// The compressed form is a legitimate inbound frame but nothing any `marshal*`
/// writes, so anything that only ever handles our own output holds a buffer it
/// did not build if it sees one. A shape check, not a decode: it exists because
/// passing node bytes where a packed payload is expected fails nowhere locally,
/// and the peer answers by dropping the connection.
pub fn check_plain_payload(data: &[u8]) -> Result<()> {
    match data.split_first() {
        Some((&format, _)) if format != FORMAT_PLAIN => {
            Err(BinaryError::UnexpectedFormatByte(format))
        }
        Some((_, node_bytes)) if !node_bytes.is_empty() => Ok(()),
        // No format byte at all, or one with nothing behind it.
        _ => Err(BinaryError::EmptyData),
    }
}

/// Prefix node bytes with the format byte: the inverse of [`unpack`], turning a
/// buffer the decoder consumed back into one a send path accepts.
pub fn pack(node_bytes: &[u8]) -> Vec<u8> {
    let mut packed = Vec::with_capacity(node_bytes.len() + 1);
    packed.push(FORMAT_PLAIN);
    packed.extend_from_slice(node_bytes);
    packed
}

fn decompress_zlib(compressed: &[u8]) -> Result<Vec<u8>> {
    decompress_zlib_pooled(compressed, MAX_DECOMPRESSED_SIZE)
        .map_err(|e| BinaryError::Zlib(e.to_string()))
}

/// Strip the format byte from a packed payload, decompressing the node bytes
/// when it says they are.
pub fn unpack(data: &[u8]) -> Result<Cow<'_, [u8]>> {
    if data.is_empty() {
        return Err(BinaryError::EmptyData);
    }
    let data_type = data[0];
    let data = &data[1..];

    if (data_type & FORMAT_COMPRESSED) > 0 {
        Ok(Cow::Owned(decompress_zlib(data)?))
    } else {
        Ok(Cow::Borrowed(data))
    }
}

/// Unpack a network payload into an owned `Bytes` buffer.
///
/// Uncompressed payloads reuse the existing `BytesMut` allocation
/// and freeze it without copying. Compressed payloads allocate a
/// decompression buffer which is then wrapped as `Bytes`.
pub fn unpack_bytes(mut data: BytesMut) -> Result<Bytes> {
    if data.is_empty() {
        return Err(BinaryError::EmptyData);
    }
    let data_type = data[0];

    if (data_type & FORMAT_COMPRESSED) > 0 {
        Ok(Bytes::from(decompress_zlib(&data[1..])?))
    } else {
        data.advance(1);
        Ok(data.freeze())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::marshal::{marshal, unmarshal_packed_ref, unmarshal_ref};
    use crate::node::{Attrs, Node, NodeContent, NodeValue, OwnedNodeRef};

    /// One shape per thing the encoder does differently: a child list, a binary
    /// body, and attributes that take the packed hex/nibble tokens.
    fn shapes() -> Vec<(&'static str, Node)> {
        let mut attrs = Attrs::with_capacity(3);
        attrs.push("id".to_string(), "ABC123");
        attrs.push("hex".to_string(), NodeValue::from("DEADBEEF"));
        attrs.push("nibble".to_string(), NodeValue::from("12-345.6"));

        vec![
            (
                "children",
                Node::new(
                    "iq",
                    attrs.clone(),
                    Some(NodeContent::Nodes(vec![
                        Node::new("ping", Attrs::new(), None),
                        Node::new(
                            "text",
                            Attrs::new(),
                            Some(NodeContent::String("hello".into())),
                        ),
                    ])),
                ),
            ),
            (
                "binary body",
                Node::new(
                    "message",
                    Attrs::new(),
                    Some(NodeContent::Bytes(vec![0xAB; 512])),
                ),
            ),
            ("packed attrs", Node::new("receipt", attrs, None)),
        ]
    }

    /// The round trip a forwarding consumer performs: marshal, take it apart the
    /// way the receive path does, then hand what the decoder holds back to a send
    /// path. The bytes that reach the send point must be the ones marshal wrote.
    #[test]
    fn a_decoded_stanza_packs_back_to_the_bytes_it_arrived_as() {
        for (shape, node) in shapes() {
            let packed = marshal(&node).expect("marshal");
            let node_bytes = unpack(&packed).expect("unpack").into_owned();
            let owned = OwnedNodeRef::new(node_bytes).expect("decode");

            let forwarded = pack(&owned.backing_bytes());
            assert_eq!(
                forwarded, packed,
                "{shape}: pack(backing_bytes) must reproduce the marshal output"
            );
            assert_eq!(
                unmarshal_packed_ref(&forwarded)
                    .expect("the forwarded payload decodes")
                    .to_owned(),
                owned.to_owned_node(),
                "{shape}: the forwarded stanza is still the same node"
            );
        }
    }

    /// `unpack_bytes` is what the read loop actually calls, so it has to agree
    /// with `unpack` byte for byte or only one of the two round-trips.
    #[test]
    fn unpack_bytes_strips_the_same_byte_as_unpack() {
        for (shape, node) in shapes() {
            let packed = marshal(&node).expect("marshal");
            let owned = unpack_bytes(BytesMut::from(&packed[..])).expect("unpack_bytes");

            assert_eq!(
                owned.as_ref(),
                unpack(&packed).expect("unpack").as_ref(),
                "{shape}"
            );
            assert_eq!(pack(&owned), packed, "{shape}");
        }
    }

    /// Nails down the byte itself: the encoder writes an uncompressed format
    /// byte, and the decoder neither consumes nor requires it. Change either and
    /// every packed payload we send or forward is off by one.
    #[test]
    fn the_format_byte_is_uncompressed_and_the_decoder_never_reads_it() {
        let node = Node::new("iq", Attrs::new(), None);
        let packed = marshal(&node).expect("marshal");

        assert_eq!(packed[0], FORMAT_PLAIN);
        assert_eq!(FORMAT_PLAIN, 0);
        assert_eq!(FORMAT_COMPRESSED, 2);

        // Not required: node bytes alone decode.
        assert!(unmarshal_ref(&packed[1..]).is_ok());
        // Not consumed: the same bytes with it still attached do not.
        assert!(unmarshal_ref(&packed).is_err());
    }

    /// The failure the asymmetry causes, made local: node bytes offered where a
    /// packed payload belongs are refused, and the error names the byte.
    #[test]
    fn node_bytes_are_refused_where_a_packed_payload_belongs() {
        let node = Node::new(
            "iq",
            Attrs::new(),
            Some(NodeContent::Nodes(vec![Node::new(
                "ping",
                Attrs::new(),
                None,
            )])),
        );
        let packed = marshal(&node).expect("marshal");
        let node_bytes = unpack(&packed).expect("unpack").into_owned();

        assert!(check_plain_payload(&packed).is_ok());
        assert!(matches!(
            check_plain_payload(&node_bytes),
            Err(BinaryError::UnexpectedFormatByte(byte)) if byte == node_bytes[0]
        ));
        // No format byte, and a format byte with no stanza behind it: both are
        // shaped like a packed payload only if the length is not checked.
        assert!(matches!(
            check_plain_payload(&[]),
            Err(BinaryError::EmptyData)
        ));
        assert!(matches!(
            check_plain_payload(&[FORMAT_PLAIN]),
            Err(BinaryError::EmptyData)
        ));
        // Compressed is a legitimate inbound frame, so it is refused by byte
        // rather than by shape: nothing we produce is compressed.
        let mut compressed = packed.clone();
        compressed[0] = FORMAT_COMPRESSED;
        assert!(matches!(
            check_plain_payload(&compressed),
            Err(BinaryError::UnexpectedFormatByte(FORMAT_COMPRESSED))
        ));

        assert!(unmarshal_packed_ref(&packed).is_ok());
        assert!(matches!(
            unmarshal_packed_ref(&node_bytes),
            Err(BinaryError::UnexpectedFormatByte(byte)) if byte == node_bytes[0]
        ));
        assert!(matches!(
            unmarshal_packed_ref(&[]),
            Err(BinaryError::EmptyData)
        ));
    }
}

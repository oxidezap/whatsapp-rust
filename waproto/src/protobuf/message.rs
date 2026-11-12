#![forbid(unsafe_code)]

use super::{
    DecodeContext, DecodeError, DecodeOptions, DefaultInstance, EncodeError, EncodeSink,
    MAX_MESSAGE_BYTES, SizeCache,
    encoding::{self, Tag, WireType},
};
use bytes::Buf;

#[inline]
pub const fn saturate_size(size: u64) -> u32 {
    if size > u32::MAX as u64 {
        u32::MAX
    } else {
        size as u32
    }
}

#[inline]
pub fn checked_encode_size(size: u32) -> Result<u32, EncodeError> {
    if size > MAX_MESSAGE_BYTES {
        Err(EncodeError::MessageTooLarge)
    } else {
        Ok(size)
    }
}

#[cold]
pub fn encode_size_overflow() -> ! {
    panic!("message encoded size exceeds the 2 GiB protobuf limit")
}

pub trait MessageName {
    const PACKAGE: &'static str;
    const NAME: &'static str;
    const FULL_NAME: &'static str;
    const TYPE_URL: &'static str;
}

pub trait Message: DefaultInstance + Clone + PartialEq + Send + Sync {
    fn compute_size(&self, cache: &mut SizeCache) -> u32;
    fn write_to(&self, cache: &mut SizeCache, output: &mut impl EncodeSink);
    fn merge_field(
        &mut self,
        tag: Tag,
        input: &mut impl Buf,
        context: DecodeContext<'_>,
    ) -> Result<(), DecodeError>;
    fn clear(&mut self);

    fn try_encode_with_cache(
        &self,
        cache: &mut SizeCache,
        output: &mut impl EncodeSink,
    ) -> Result<(), EncodeError> {
        cache.clear();
        checked_encode_size(self.compute_size(cache))?;
        self.write_to(cache, output);
        Ok(())
    }
    fn try_encode(&self, output: &mut impl EncodeSink) -> Result<(), EncodeError> {
        self.try_encode_with_cache(&mut SizeCache::new(), output)
    }
    fn encode(&self, output: &mut impl EncodeSink) {
        self.try_encode(output)
            .unwrap_or_else(|_| encode_size_overflow());
    }
    fn encode_with_cache(&self, cache: &mut SizeCache, output: &mut impl EncodeSink) {
        self.try_encode_with_cache(cache, output)
            .unwrap_or_else(|_| encode_size_overflow());
    }
    fn try_encode_bounded_with_cache(
        &self,
        max_bytes: u32,
        cache: &mut SizeCache,
        output: &mut impl EncodeSink,
    ) -> Result<u32, EncodeError> {
        cache.clear();
        let len = checked_encode_size(self.compute_size(cache))?;
        if len > max_bytes {
            return Err(EncodeError::ExceedsBudget { len, max_bytes });
        }
        self.write_to(cache, output);
        Ok(len)
    }
    fn try_encode_bounded(
        &self,
        max_bytes: u32,
        output: &mut impl EncodeSink,
    ) -> Result<u32, EncodeError> {
        self.try_encode_bounded_with_cache(max_bytes, &mut SizeCache::new(), output)
    }
    fn try_encoded_len(&self) -> Result<u32, EncodeError> {
        checked_encode_size(self.compute_size(&mut SizeCache::new()))
    }
    fn encoded_len(&self) -> u32 {
        self.try_encoded_len()
            .unwrap_or_else(|_| encode_size_overflow())
    }
    fn try_encode_to_vec(&self) -> Result<Vec<u8>, EncodeError> {
        let mut cache = SizeCache::new();
        let size = checked_encode_size(self.compute_size(&mut cache))? as usize;
        let mut output = Vec::with_capacity(size);
        self.write_to(&mut cache, &mut output);
        debug_assert_eq!(
            output.len(),
            size,
            "computed and written protobuf lengths differ"
        );
        Ok(output)
    }
    fn encode_to_vec(&self) -> Vec<u8> {
        self.try_encode_to_vec()
            .unwrap_or_else(|_| encode_size_overflow())
    }
    fn try_encode_to_bytes(&self) -> Result<bytes::Bytes, EncodeError> {
        let mut cache = SizeCache::new();
        let size = checked_encode_size(self.compute_size(&mut cache))? as usize;
        let mut output = bytes::BytesMut::with_capacity(size);
        self.write_to(&mut cache, &mut output);
        debug_assert_eq!(
            output.len(),
            size,
            "computed and written protobuf lengths differ"
        );
        Ok(output.freeze())
    }
    fn encode_to_bytes(&self) -> bytes::Bytes {
        self.try_encode_to_bytes()
            .unwrap_or_else(|_| encode_size_overflow())
    }
    fn try_encode_length_delimited(&self, output: &mut impl EncodeSink) -> Result<(), EncodeError> {
        let mut cache = SizeCache::new();
        let size = checked_encode_size(self.compute_size(&mut cache))?;
        encoding::put_varint(u64::from(size), output);
        self.write_to(&mut cache, output);
        Ok(())
    }
    fn encode_length_delimited(&self, output: &mut impl EncodeSink) {
        self.try_encode_length_delimited(output)
            .unwrap_or_else(|_| encode_size_overflow());
    }
    fn decode(input: &mut impl Buf) -> Result<Self, DecodeError> {
        DecodeOptions::new().decode(input)
    }
    fn decode_from_slice(input: &[u8]) -> Result<Self, DecodeError> {
        DecodeOptions::new().decode_from_slice(input)
    }
    fn decode_length_delimited(input: &mut impl Buf) -> Result<Self, DecodeError> {
        DecodeOptions::new().decode_length_delimited(input)
    }
    fn merge(
        &mut self,
        input: &mut impl Buf,
        context: DecodeContext<'_>,
    ) -> Result<(), DecodeError> {
        self.merge_to_limit(input, context, 0)
    }
    fn merge_from_slice(&mut self, mut input: &[u8]) -> Result<(), DecodeError> {
        let unknown = std::cell::Cell::new(super::DEFAULT_UNKNOWN_FIELD_LIMIT);
        let elements = std::cell::Cell::new(super::DEFAULT_ELEMENT_MEMORY_LIMIT);
        self.merge(
            &mut input,
            DecodeContext::new(super::RECURSION_LIMIT, &unknown).with_element_memory(&elements),
        )
    }
    fn merge_to_limit(
        &mut self,
        input: &mut impl Buf,
        context: DecodeContext<'_>,
        limit: usize,
    ) -> Result<(), DecodeError> {
        while input.remaining() > limit {
            let tag = Tag::decode(input)?;
            self.merge_field(tag, input, context)?;
        }
        Ok(())
    }
    fn merge_length_delimited(
        &mut self,
        input: &mut impl Buf,
        context: DecodeContext<'_>,
    ) -> Result<(), DecodeError> {
        let context = context.descend()?;
        let length = encoding::decode_varint(input)?;
        if length > u64::from(MAX_MESSAGE_BYTES) {
            return Err(DecodeError::MessageTooLarge);
        }
        let length = length as usize;
        if length > input.remaining() {
            return Err(DecodeError::UnexpectedEof);
        }
        let limit = input.remaining() - length;
        self.merge_to_limit(input, context, limit)?;
        if input.remaining() < limit {
            return Err(DecodeError::UnexpectedEof);
        }
        input.advance(input.remaining() - limit);
        Ok(())
    }
    fn merge_group(
        &mut self,
        input: &mut impl Buf,
        context: DecodeContext<'_>,
        number: u32,
    ) -> Result<(), DecodeError> {
        let context = context.descend()?;
        loop {
            let tag = Tag::decode(input)?;
            if tag.wire_type() == WireType::EndGroup {
                return if tag.field_number() == number {
                    Ok(())
                } else {
                    Err(DecodeError::InvalidEndGroup(tag.field_number()))
                };
            }
            self.merge_field(tag, input, context)?;
        }
    }
}

impl DecodeOptions {
    pub fn decode<M: Message>(&self, input: &mut impl Buf) -> Result<M, DecodeError> {
        self.run(input.remaining(), |context| {
            let mut message = M::default();
            message.merge(input, context)?;
            Ok(message)
        })
    }
    pub fn decode_from_slice<M: Message>(&self, input: &[u8]) -> Result<M, DecodeError> {
        self.decode(&mut &*input)
    }
    pub fn decode_length_delimited<M: Message>(
        &self,
        input: &mut impl Buf,
    ) -> Result<M, DecodeError> {
        let length = encoding::decode_varint(input)?;
        self.run(
            usize::try_from(length).map_err(|_| DecodeError::MessageTooLarge)?,
            |context| {
                let length = length as usize;
                if input.remaining() < length {
                    return Err(DecodeError::UnexpectedEof);
                }
                let limit = input.remaining() - length;
                let mut message = M::default();
                message.merge_to_limit(input, context, limit)?;
                if input.remaining() != limit {
                    return Err(DecodeError::UnexpectedEof);
                }
                Ok(message)
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, Default, PartialEq)]
    struct Record {
        value: Option<u64>,
        child: super::super::MessageField<Record>,
    }

    impl DefaultInstance for Record {
        fn default_instance() -> &'static Self {
            static DEFAULT: Record = Record {
                value: None,
                child: super::super::MessageField::none(),
            };
            &DEFAULT
        }
    }

    impl Message for Record {
        fn compute_size(&self, cache: &mut SizeCache) -> u32 {
            let mut size = self
                .value
                .map_or(0, |value| 1 + encoding::varint_len(value) as u64);
            if let Some(child) = self.child.as_option() {
                let slot = cache.reserve();
                let child_size = child.compute_size(cache);
                cache.set(slot, child_size);
                size +=
                    1 + encoding::varint_len(u64::from(child_size)) as u64 + u64::from(child_size);
            }
            saturate_size(size)
        }
        fn write_to(&self, cache: &mut SizeCache, output: &mut impl EncodeSink) {
            if let Some(value) = self.value {
                output.put_u8(8);
                encoding::put_varint(value, output);
            }
            if let Some(child) = self.child.as_option() {
                output.put_u8(18);
                encoding::put_varint(u64::from(cache.consume_next()), output);
                child.write_to(cache, output);
            }
        }
        fn merge_field(
            &mut self,
            tag: Tag,
            input: &mut impl Buf,
            context: DecodeContext<'_>,
        ) -> Result<(), DecodeError> {
            match tag.field_number() {
                1 => {
                    encoding::check_wire_type(tag, WireType::Varint)?;
                    self.value = Some(encoding::decode_varint(input)?);
                }
                2 => {
                    encoding::check_wire_type(tag, WireType::LengthDelimited)?;
                    self.child
                        .get_or_insert_default()
                        .merge_length_delimited(input, context)?;
                }
                _ => encoding::skip_field_depth(tag, input, context.depth())?,
            }
            Ok(())
        }
        fn clear(&mut self) {
            *self = Self::default();
        }
    }

    #[test]
    fn independent_nested_wire_fixture_and_fragmented_merge_preserve_presence() {
        let bytes = [8, 150, 1, 18, 2, 8, 7, 18, 0];
        let record = Record::decode_from_slice(&bytes).unwrap();
        assert_eq!(record.value, Some(150));
        assert_eq!(record.child.value, Some(7));
        assert!(record.child.is_set());
        assert_eq!(record.encode_to_vec(), [8, 150, 1, 18, 2, 8, 7]);
        let empty = Record::decode_from_slice(&[18, 0]).unwrap();
        assert!(empty.child.is_set());
        assert_eq!(empty.encode_to_vec(), [18, 0]);
    }

    #[test]
    fn size_and_depth_limits_are_independent_and_cache_reuse_is_exact() {
        let bytes = [18, 2, 8, 7];
        assert_eq!(
            DecodeOptions::new()
                .with_recursion_limit(0)
                .decode_from_slice::<Record>(&bytes),
            Err(DecodeError::RecursionLimitExceeded)
        );
        assert_eq!(
            DecodeOptions::new()
                .with_max_message_size(3)
                .decode_from_slice::<Record>(&bytes),
            Err(DecodeError::MessageTooLarge)
        );
        let record = Record::decode_from_slice(&bytes).unwrap();
        let mut cache = SizeCache::new();
        let mut output = vec![99];
        assert_eq!(
            record.try_encode_bounded_with_cache(3, &mut cache, &mut output),
            Err(EncodeError::ExceedsBudget {
                len: 4,
                max_bytes: 3
            })
        );
        assert_eq!(output, [99]);
        assert_eq!(
            record.try_encode_bounded_with_cache(4, &mut cache, &mut output),
            Ok(4)
        );
        assert_eq!(output, [99, 18, 2, 8, 7]);
    }

    #[test]
    fn length_delimited_decode_stops_at_the_declared_boundary() {
        let mut input = [2, 8, 7, 8, 9].as_slice();
        let record = Record::decode_length_delimited(&mut input).unwrap();
        assert_eq!(record.value, Some(7));
        assert_eq!(input, &[8, 9]);
        assert_eq!(
            Record::decode_from_slice(&[18, 1, 8, 1]),
            Err(DecodeError::UnexpectedEof)
        );
    }
}

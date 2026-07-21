use core::fmt;

use serde::Serialize;
use serde::de::{SeqAccess, Visitor};

const MAX_PREALLOCATED_BYTE_BUFFER: usize = 1024 * 1024;

struct Bytes<'a>(&'a [u8]);

impl Serialize for Bytes<'_> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_bytes(self.0)
    }
}

pub(crate) fn serialize_optional_bytes<T, S>(
    value: &Option<T>,
    serializer: S,
) -> Result<S::Ok, S::Error>
where
    T: AsRef<[u8]>,
    S: serde::Serializer,
{
    match value {
        Some(value) => serializer.serialize_some(&Bytes(value.as_ref())),
        None => serializer.serialize_none(),
    }
}

struct ByteBufferVisitor;

impl<'de> Visitor<'de> for ByteBufferVisitor {
    type Value = Vec<u8>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a byte buffer")
    }

    fn visit_borrowed_bytes<E>(self, value: &'de [u8]) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(value.to_vec())
    }

    fn visit_bytes<E>(self, value: &[u8]) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(value.to_vec())
    }

    fn visit_byte_buf<E>(self, value: Vec<u8>) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(value)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: SeqAccess<'de>,
    {
        let capacity = sequence
            .size_hint()
            .unwrap_or(0)
            .min(MAX_PREALLOCATED_BYTE_BUFFER);
        let mut bytes = Vec::with_capacity(capacity);
        while let Some(byte) = sequence.next_element()? {
            bytes.push(byte);
        }
        Ok(bytes)
    }
}

struct OptionalBytesVisitor;

impl<'de> Visitor<'de> for OptionalBytesVisitor {
    type Value = Option<Vec<u8>>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an optional byte buffer")
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(None)
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: serde::de::Error,
    {
        Ok(None)
    }

    fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        deserializer
            .deserialize_byte_buf(ByteBufferVisitor)
            .map(Some)
    }
}

pub(crate) fn deserialize_optional_bytes<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<u8>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserializer.deserialize_option(OptionalBytesVisitor)
}

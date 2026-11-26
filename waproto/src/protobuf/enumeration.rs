#![forbid(unsafe_code)]

use std::fmt;
use std::hash::Hash;
use std::marker::PhantomData;

pub trait Enumeration: Clone + Copy + PartialEq + Eq + Hash + fmt::Debug {
    fn from_i32(value: i32) -> Option<Self>;
    fn to_i32(&self) -> i32;
    fn proto_name(&self) -> &'static str;
    fn from_proto_name(_name: &str) -> Option<Self> {
        None
    }
    fn values() -> &'static [Self] {
        &[]
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum EnumValue<E: Enumeration> {
    Known(E),
    Unknown(i32),
}

impl<E: Enumeration> EnumValue<E> {
    pub fn to_i32(&self) -> i32 {
        match self {
            Self::Known(value) => value.to_i32(),
            Self::Unknown(value) => *value,
        }
    }
    pub fn is_known(&self) -> bool {
        matches!(self, Self::Known(_))
    }
    pub fn is_unknown(&self) -> bool {
        !self.is_known()
    }
    pub fn as_known(&self) -> Option<E> {
        match self {
            Self::Known(value) => Some(*value),
            Self::Unknown(_) => None,
        }
    }
}

impl<E: Enumeration> From<i32> for EnumValue<E> {
    fn from(value: i32) -> Self {
        E::from_i32(value).map_or(Self::Unknown(value), Self::Known)
    }
}

impl<E: Enumeration> From<E> for EnumValue<E> {
    fn from(value: E) -> Self {
        Self::Known(value)
    }
}

impl<E: Enumeration> Default for EnumValue<E> {
    fn default() -> Self {
        Self::from(0)
    }
}

impl<E: Enumeration> PartialEq<E> for EnumValue<E> {
    fn eq(&self, other: &E) -> bool {
        self.as_known().as_ref() == Some(other)
    }
}

impl<E: Enumeration> fmt::Debug for EnumValue<E> {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Known(value) => fmt::Debug::fmt(value, output),
            Self::Unknown(value) => write!(output, "Unknown({value})"),
        }
    }
}

impl<E: Enumeration> fmt::Display for EnumValue<E> {
    fn fmt(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Known(value) => output.write_str(value.proto_name()),
            Self::Unknown(value) => fmt::Display::fmt(value, output),
        }
    }
}

impl<E: Enumeration> serde::Serialize for EnumValue<E> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Known(value) => serializer.serialize_str(value.proto_name()),
            Self::Unknown(value) => serializer.serialize_i32(*value),
        }
    }
}

struct EnumVisitor<E>(PhantomData<E>);

impl<'de, E: Enumeration> serde::de::Visitor<'de> for EnumVisitor<E> {
    type Value = EnumValue<E>;
    fn expecting(&self, output: &mut fmt::Formatter<'_>) -> fmt::Result {
        output.write_str("a protobuf enum name string or integer value")
    }
    fn visit_unit<Error: serde::de::Error>(self) -> Result<Self::Value, Error> {
        Ok(EnumValue::from(0))
    }
    fn visit_str<Error: serde::de::Error>(self, value: &str) -> Result<Self::Value, Error> {
        E::from_proto_name(value)
            .map(EnumValue::Known)
            .ok_or_else(|| Error::unknown_variant(value, &[]))
    }
    fn visit_i64<Error: serde::de::Error>(self, value: i64) -> Result<Self::Value, Error> {
        let number: i32 = value.try_into().map_err(|_| {
            Error::invalid_value(serde::de::Unexpected::Signed(value), &"an i32 enum value")
        })?;
        Ok(EnumValue::from(number))
    }
    fn visit_u64<Error: serde::de::Error>(self, value: u64) -> Result<Self::Value, Error> {
        let number: i32 = value.try_into().map_err(|_| {
            Error::invalid_value(serde::de::Unexpected::Unsigned(value), &"an i32 enum value")
        })?;
        Ok(EnumValue::from(number))
    }
}

impl<'de, E: Enumeration> serde::Deserialize<'de> for EnumValue<E> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_any(EnumVisitor(PhantomData))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    enum Code {
        First = 1,
        Second = 2,
    }

    impl Enumeration for Code {
        fn from_i32(value: i32) -> Option<Self> {
            match value {
                1 => Some(Self::First),
                2 => Some(Self::Second),
                _ => None,
            }
        }
        fn to_i32(&self) -> i32 {
            *self as i32
        }
        fn proto_name(&self) -> &'static str {
            match self {
                Self::First => "FIRST",
                Self::Second => "SECOND",
            }
        }
        fn from_proto_name(value: &str) -> Option<Self> {
            match value {
                "FIRST" => Some(Self::First),
                "SECOND" => Some(Self::Second),
                _ => None,
            }
        }
    }

    impl buffa::Enumeration for Code {
        fn from_i32(value: i32) -> Option<Self> {
            <Self as Enumeration>::from_i32(value)
        }
        fn to_i32(&self) -> i32 {
            <Self as Enumeration>::to_i32(self)
        }
        fn proto_name(&self) -> &'static str {
            <Self as Enumeration>::proto_name(self)
        }
        fn from_proto_name(value: &str) -> Option<Self> {
            <Self as Enumeration>::from_proto_name(value)
        }
    }

    #[test]
    fn open_values_preserve_unknown_numbers_and_variant_identity() {
        assert_eq!(EnumValue::<Code>::default(), EnumValue::Unknown(0));
        assert_ne!(EnumValue::Known(Code::First), EnumValue::Unknown(1));
        assert!(EnumValue::Unknown(1) != Code::First);
        for number in [i32::MIN, -1, 0, 1, 2, i32::MAX] {
            let candidate = EnumValue::<Code>::from(number);
            let reference = buffa::EnumValue::<Code>::from(number);
            assert_eq!(candidate.to_i32(), reference.to_i32());
            assert_eq!(candidate.is_known(), reference.is_known());
            assert_eq!(candidate.as_known(), reference.as_known());
            assert_eq!(format!("{candidate:?}"), format!("{reference:?}"));
            assert_eq!(candidate.to_string(), reference.to_string());
        }
    }

    #[test]
    fn serde_names_unknown_numbers_nulls_and_range_failures_match() {
        for input in [
            "null",
            "0",
            "1",
            "2",
            "-1",
            "2147483647",
            "-2147483648",
            "2147483648",
            "-2147483649",
            "18446744073709551615",
            "1.0",
            "true",
            "[]",
            "\"FIRST\"",
            "\"SECOND\"",
            "\"first\"",
            "\"unknown\"",
        ] {
            let candidate = serde_json::from_str::<EnumValue<Code>>(input);
            let reference = serde_json::from_str::<buffa::EnumValue<Code>>(input);
            match (candidate, reference) {
                (Ok(candidate), Ok(reference)) => {
                    assert_eq!(candidate.to_i32(), reference.to_i32());
                    assert_eq!(candidate.is_known(), reference.is_known());
                    assert_eq!(
                        serde_json::to_value(candidate).unwrap(),
                        serde_json::to_value(reference).unwrap()
                    );
                }
                (Err(candidate), Err(reference)) => {
                    assert_eq!(candidate.to_string(), reference.to_string())
                }
                (candidate, reference) => {
                    panic!("mismatched serde result for {input}: {candidate:?} vs {reference:?}")
                }
            }
        }
    }
}

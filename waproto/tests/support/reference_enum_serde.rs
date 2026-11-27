use ::buffa::{EnumValue, Enumeration};
use serde::ser::SerializeSeq as _;

pub fn serialize<E, S>(value: &Option<EnumValue<E>>, s: S) -> Result<S::Ok, S::Error>
where
    E: Enumeration + serde::Serialize,
    S: serde::Serializer,
{
    match value {
        None => s.serialize_none(),
        Some(EnumValue::Known(e)) => s.serialize_some(e),
        Some(EnumValue::Unknown(n)) => s.serialize_some(n),
    }
}

pub fn serialize_repeated<E, S>(values: &[EnumValue<E>], s: S) -> Result<S::Ok, S::Error>
where
    E: Enumeration + serde::Serialize,
    S: serde::Serializer,
{
    let mut sequence = s.serialize_seq(Some(values.len()))?;
    for value in values {
        match value {
            EnumValue::Known(e) => sequence.serialize_element(e)?,
            EnumValue::Unknown(n) => sequence.serialize_element(n)?,
        }
    }
    sequence.end()
}

#[cfg(feature = "serde-deserialize")]
pub fn deserialize<'de, E, D>(d: D) -> Result<Option<EnumValue<E>>, D::Error>
where
    E: Enumeration + serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum Wire<E> {
        // The enum's derived impl first, so every feature-selected input
        // shape (lowercase names, numeric repr) keeps working; the raw
        // integer fallback only catches values the enum doesn't know.
        Known(E),
        Raw(i32),
    }
    Ok(
        match <Option<Wire<E>> as serde::Deserialize>::deserialize(d)? {
            None => None,
            Some(Wire::Known(e)) => Some(EnumValue::Known(e)),
            Some(Wire::Raw(n)) => Some(EnumValue::from(n)),
        },
    )
}

#[cfg(feature = "serde-deserialize")]
pub fn deserialize_repeated<'de, E, D>(d: D) -> Result<Vec<EnumValue<E>>, D::Error>
where
    E: Enumeration + serde::Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    #[derive(serde::Deserialize)]
    #[serde(untagged)]
    enum Wire<E> {
        Known(E),
        Raw(i32),
    }

    Ok(<Vec<Wire<E>> as serde::Deserialize>::deserialize(d)?
        .into_iter()
        .map(|value| match value {
            Wire::Known(e) => EnumValue::Known(e),
            Wire::Raw(n) => EnumValue::from(n),
        })
        .collect())
}

// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

//! Key material the page must not receive inside a decoded `Message`.
//!
//! A `Message` can hold another `Message` in many places: the wrapper fields
//! (`ephemeralMessage`, `viewOnceMessage`, …), `ContextInfo.quotedMessage`
//! inside most message types, edits, comments and payments. `scrub_message`
//! clears the positions the library acts on; `holds_key_material` then walks
//! every field through the generated `Serialize` impls, so a key field left
//! set at any depth is found without a hand-kept list of nesting paths.

use serde::Serialize;
use serde::ser;
use waproto::whatsapp::Message;

/// Field names (normalized: lower case, no underscores) whose content is
/// key material.
const KEY_FIELDS: [&str; 4] = [
    "appstatesynckeyshare",
    "appstatesynckeyrequest",
    "senderkeydistributionmessage",
    "fastratchetkeysenderkeydistributionmessage",
];

fn is_key_field(name: &str) -> bool {
    let normalized: String = name
        .chars()
        .filter(|c| *c != '_')
        .map(|c| c.to_ascii_lowercase())
        .collect();
    KEY_FIELDS.contains(&normalized.as_str())
}

/// Clear key material at the positions the library processes: the root and
/// every `DeviceSentMessage` layer. Own-device copies can nest wrappers, and
/// the library unwraps only one.
pub fn scrub_message(message: &mut Message) {
    if let Some(protocol) = message.protocol_message.as_option_mut() {
        protocol.app_state_sync_key_share = Default::default();
        protocol.app_state_sync_key_request = Default::default();
    }
    message.sender_key_distribution_message = Default::default();
    message.fast_ratchet_key_sender_key_distribution_message = Default::default();
    if let Some(sent) = message.device_sent_message.as_option_mut()
        && let Some(inner) = sent.message.as_option_mut()
    {
        scrub_message(inner);
    }
}

/// Whether any key field is set anywhere in `message`. Fails closed: a
/// serialization error counts as key material.
pub fn holds_key_material(message: &Message) -> bool {
    message.serialize(Probe { key_field: false }).is_err()
}

/// Walks a value and fails on the first key field that holds content.
/// `key_field` marks a value that is itself a key field: there anything but
/// "not set" is a hit.
#[derive(Clone, Copy)]
struct Probe {
    key_field: bool,
}

#[derive(Debug)]
struct Found;

impl std::fmt::Display for Found {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("key material")
    }
}

impl std::error::Error for Found {}

impl ser::Error for Found {
    fn custom<T: std::fmt::Display>(_: T) -> Self {
        Found
    }
}

impl Probe {
    fn value(self) -> Result<(), Found> {
        if self.key_field { Err(Found) } else { Ok(()) }
    }

    fn inner(self) -> Self {
        Probe { key_field: false }
    }
}

macro_rules! leaf {
    ($($name:ident: $ty:ty),* $(,)?) => {
        $(fn $name(self, _: $ty) -> Result<(), Found> { self.value() })*
    };
}

impl ser::Serializer for Probe {
    type Ok = ();
    type Error = Found;
    type SerializeSeq = Self;
    type SerializeTuple = Self;
    type SerializeTupleStruct = Self;
    type SerializeTupleVariant = Self;
    type SerializeMap = Self;
    type SerializeStruct = Self;
    type SerializeStructVariant = Self;

    leaf!(
        serialize_bool: bool,
        serialize_i8: i8,
        serialize_i16: i16,
        serialize_i32: i32,
        serialize_i64: i64,
        serialize_i128: i128,
        serialize_u8: u8,
        serialize_u16: u16,
        serialize_u32: u32,
        serialize_u64: u64,
        serialize_u128: u128,
        serialize_f32: f32,
        serialize_f64: f64,
        serialize_char: char,
        serialize_str: &str,
        serialize_bytes: &[u8],
        serialize_unit_struct: &'static str,
    );

    fn serialize_none(self) -> Result<(), Found> {
        Ok(())
    }

    fn serialize_unit(self) -> Result<(), Found> {
        Ok(())
    }

    fn serialize_some<T: ?Sized + Serialize>(self, value: &T) -> Result<(), Found> {
        self.value()?;
        value.serialize(self.inner())
    }

    fn serialize_unit_variant(self, _: &'static str, _: u32, _: &'static str) -> Result<(), Found> {
        self.value()
    }

    fn serialize_newtype_struct<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        value: &T,
    ) -> Result<(), Found> {
        value.serialize(self)
    }

    fn serialize_newtype_variant<T: ?Sized + Serialize>(
        self,
        _: &'static str,
        _: u32,
        variant: &'static str,
        value: &T,
    ) -> Result<(), Found> {
        self.value()?;
        value.serialize(Probe {
            key_field: is_key_field(variant),
        })
    }

    fn serialize_seq(self, len: Option<usize>) -> Result<Self, Found> {
        if self.key_field && len != Some(0) {
            return Err(Found);
        }
        Ok(self.inner())
    }

    fn serialize_tuple(self, len: usize) -> Result<Self, Found> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_struct(self, _: &'static str, len: usize) -> Result<Self, Found> {
        self.serialize_seq(Some(len))
    }

    fn serialize_tuple_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self, Found> {
        self.value()?;
        Ok(self.inner())
    }

    fn serialize_map(self, len: Option<usize>) -> Result<Self, Found> {
        self.serialize_seq(len)
    }

    fn serialize_struct(self, _: &'static str, _: usize) -> Result<Self, Found> {
        self.value()?;
        Ok(self.inner())
    }

    fn serialize_struct_variant(
        self,
        _: &'static str,
        _: u32,
        _: &'static str,
        _: usize,
    ) -> Result<Self, Found> {
        self.value()?;
        Ok(self.inner())
    }
}

impl ser::SerializeSeq for Probe {
    type Ok = ();
    type Error = Found;
    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Found> {
        value.serialize(self.inner())
    }
    fn end(self) -> Result<(), Found> {
        Ok(())
    }
}

impl ser::SerializeTuple for Probe {
    type Ok = ();
    type Error = Found;
    fn serialize_element<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Found> {
        value.serialize(self.inner())
    }
    fn end(self) -> Result<(), Found> {
        Ok(())
    }
}

impl ser::SerializeTupleStruct for Probe {
    type Ok = ();
    type Error = Found;
    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Found> {
        value.serialize(self.inner())
    }
    fn end(self) -> Result<(), Found> {
        Ok(())
    }
}

impl ser::SerializeTupleVariant for Probe {
    type Ok = ();
    type Error = Found;
    fn serialize_field<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Found> {
        value.serialize(self.inner())
    }
    fn end(self) -> Result<(), Found> {
        Ok(())
    }
}

impl ser::SerializeMap for Probe {
    type Ok = ();
    type Error = Found;
    fn serialize_key<T: ?Sized + Serialize>(&mut self, key: &T) -> Result<(), Found> {
        key.serialize(self.inner())
    }
    fn serialize_value<T: ?Sized + Serialize>(&mut self, value: &T) -> Result<(), Found> {
        value.serialize(self.inner())
    }
    fn end(self) -> Result<(), Found> {
        Ok(())
    }
}

impl ser::SerializeStruct for Probe {
    type Ok = ();
    type Error = Found;
    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Found> {
        value.serialize(Probe {
            key_field: is_key_field(key),
        })
    }
    fn end(self) -> Result<(), Found> {
        Ok(())
    }
}

impl ser::SerializeStructVariant for Probe {
    type Ok = ();
    type Error = Found;
    fn serialize_field<T: ?Sized + Serialize>(
        &mut self,
        key: &'static str,
        value: &T,
    ) -> Result<(), Found> {
        value.serialize(Probe {
            key_field: is_key_field(key),
        })
    }
    fn end(self) -> Result<(), Found> {
        Ok(())
    }
}

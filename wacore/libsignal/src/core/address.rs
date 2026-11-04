// SPDX-License-Identifier: MIT
use serde::{Deserialize, Serialize};
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct DeviceId(u32);
impl DeviceId {
    pub const fn new(value: u32) -> Self {
        Self(value)
    }
}
impl From<u32> for DeviceId {
    fn from(v: u32) -> Self {
        Self(v)
    }
}
impl From<DeviceId> for u32 {
    fn from(v: DeviceId) -> Self {
        v.0
    }
}
impl std::fmt::Display for DeviceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}
const INLINE_ADDRESS_BYTES: usize = 47;

#[derive(Clone)]
enum AddressStorage {
    Inline(arrayvec::ArrayString<INLINE_ADDRESS_BYTES>),
    Heap(String),
}

pub struct AddressBuf(AddressStorage);
impl Clone for AddressBuf {
    // Keep the inline/heap clone branches out of every async caller.
    #[inline(never)]
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}
impl Default for AddressBuf {
    fn default() -> Self {
        Self::empty()
    }
}
impl AddressBuf {
    pub fn empty() -> Self {
        Self(AddressStorage::Inline(arrayvec::ArrayString::new()))
    }
    pub fn clear(&mut self) {
        match &mut self.0 {
            AddressStorage::Inline(text) => text.clear(),
            AddressStorage::Heap(text) => text.clear(),
        }
    }
    pub fn push_str(&mut self, s: &str) {
        match &mut self.0 {
            AddressStorage::Inline(inline) => {
                if inline.try_push_str(s).is_err() {
                    let mut text = String::with_capacity((inline.len() + s.len()).max(64));
                    text.push_str(inline.as_str());
                    text.push_str(s);
                    self.0 = AddressStorage::Heap(text);
                }
            }
            AddressStorage::Heap(text) => text.push_str(s),
        }
    }
    pub fn push(&mut self, c: char) {
        self.push_str(c.encode_utf8(&mut [0; 4]));
    }
    pub fn as_str(&self) -> &str {
        match &self.0 {
            AddressStorage::Inline(text) => text.as_str(),
            AddressStorage::Heap(text) => text,
        }
    }
}
impl std::fmt::Debug for AddressBuf {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("AddressBuf").field(&self.as_str()).finish()
    }
}
impl PartialEq for AddressBuf {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}
impl Eq for AddressBuf {}
impl PartialOrd for AddressBuf {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for AddressBuf {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}
impl std::hash::Hash for AddressBuf {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::hash::Hash::hash(self.as_str(), state);
    }
}
impl Serialize for AddressBuf {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_newtype_struct("AddressBuf", self.as_str())
    }
}
impl<'de> Deserialize<'de> for AddressBuf {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(rename = "AddressBuf")]
        struct Text(String);
        let Text(text) = Text::deserialize(deserializer)?;
        if text.len() > INLINE_ADDRESS_BYTES {
            Ok(Self(AddressStorage::Heap(text)))
        } else {
            let mut out = Self::empty();
            out.push_str(&text);
            Ok(out)
        }
    }
}
#[derive(Clone, Debug)]
pub struct ProtocolAddress {
    text: AddressBuf,
    device: DeviceId,
    // The decimal device suffix fits in 11 bytes; serde retains the name_len field.
    suffix_len: u8,
}
struct AddressWriter<'a>(&'a mut ProtocolAddress);
impl Drop for AddressWriter<'_> {
    fn drop(&mut self) {
        self.0.append_device_suffix();
    }
}
impl PartialEq for ProtocolAddress {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
    }
}
impl Eq for ProtocolAddress {}
impl PartialOrd for ProtocolAddress {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for ProtocolAddress {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.text.cmp(&other.text)
    }
}
impl Serialize for ProtocolAddress {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;
        let mut out = serializer.serialize_struct("ProtocolAddress", 3)?;
        out.serialize_field("text", &self.text)?;
        out.serialize_field(
            "name_len",
            &(self.as_str().len() - usize::from(self.suffix_len)),
        )?;
        out.serialize_field("device", &self.device)?;
        out.end()
    }
}
impl<'de> Deserialize<'de> for ProtocolAddress {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(rename = "ProtocolAddress")]
        struct Fields {
            text: AddressBuf,
            name_len: usize,
            device: DeviceId,
        }
        let fields = Fields::deserialize(deserializer)?;
        let mut digits = itoa::Buffer::new();
        let device = digits.format(fields.device.0);
        if fields
            .text
            .as_str()
            .get(fields.name_len..)
            .and_then(|suffix| suffix.strip_prefix('.'))
            != Some(device)
        {
            return Err(serde::de::Error::custom(
                "inconsistent protocol address fields",
            ));
        }
        Ok(Self {
            text: fields.text,
            device: fields.device,
            suffix_len: device.len() as u8 + 1,
        })
    }
}
impl std::hash::Hash for ProtocolAddress {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // The rendered address already includes the device suffix and name boundary.
        std::hash::Hash::hash(self.as_str(), state);
    }
}
impl ProtocolAddress {
    pub fn new(name: &str, device: DeviceId) -> Self {
        let mut out = Self {
            text: AddressBuf::empty(),
            device,
            suffix_len: 0,
        };
        out.reset_with(|buf| buf.push_str(name));
        out
    }
    pub fn empty(device: DeviceId) -> Self {
        let mut out = Self {
            text: AddressBuf::empty(),
            device,
            suffix_len: 0,
        };
        out.reset_with(|_| {});
        out
    }
    // Share each writer's formatting and unwind cleanup across callers.
    #[inline(never)]
    pub fn reset_with(&mut self, write: impl FnOnce(&mut AddressBuf)) {
        self.text.clear();
        // Complete the canonical key even when the caller's writer unwinds.
        let writer = AddressWriter(self);
        write(&mut writer.0.text);
    }
    fn append_device_suffix(&mut self) {
        if self.device.0 == 0 {
            self.text.push_str(".0");
            self.suffix_len = 2;
        } else {
            self.append_nonzero_device_suffix();
        }
    }
    // Share decimal formatting across address constructors and async callers.
    #[inline(never)]
    fn append_nonzero_device_suffix(&mut self) {
        let mut digits = itoa::Buffer::new();
        let device = digits.format(self.device.0);
        self.text.push('.');
        self.text.push_str(device);
        self.suffix_len = device.len() as u8 + 1;
    }
    pub fn name(&self) -> &str {
        &self.as_str()[..self.as_str().len() - usize::from(self.suffix_len)]
    }
    pub fn device_id(&self) -> DeviceId {
        self.device
    }
    pub fn as_str(&self) -> &str {
        self.text.as_str()
    }
}
impl std::fmt::Display for ProtocolAddress {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}
/// Allocation-free, redacted diagnostic view of an address.
pub struct ObservedAddress<'a>(&'a ProtocolAddress);
impl ProtocolAddress {
    pub fn observe(&self) -> ObservedAddress<'_> {
        ObservedAddress(self)
    }
}
impl std::fmt::Display for ObservedAddress<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}.{}",
            wacore_binary::jid::ObservedJid::from(self.0.name()),
            self.0.device_id()
        )
    }
}
impl std::fmt::Debug for ObservedAddress<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_panicking_writer_leaves_a_serializable_canonical_address() {
        let mut address = ProtocolAddress::new("previous", u32::MAX.into());
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                address.reset_with(|out| {
                    out.push_str("partial-λ");
                    panic!("synthetic writer failure");
                });
            }))
            .is_err()
        );
        assert_eq!(address.name(), "partial-λ");
        assert_eq!(address.as_str(), "partial-λ.4294967295");
        assert_eq!(
            serde_json::from_slice::<ProtocolAddress>(&serde_json::to_vec(&address).unwrap())
                .unwrap(),
            address
        );
    }

    #[test]
    fn inline_addresses_fit_the_layout_budget_and_reuse_overflow_storage() {
        assert!(size_of::<ProtocolAddress>() <= 64);
        let mut buffer = AddressBuf::empty();
        buffer.push_str(&"λ".repeat(23));
        buffer.push('x');
        assert_eq!(buffer.as_str().len(), 47);
        assert!(matches!(buffer.0, AddressStorage::Inline(_)));
        buffer.push('🦀');
        assert_eq!(buffer.as_str(), format!("{}x🦀", "λ".repeat(23)));
        assert!(matches!(buffer.0, AddressStorage::Heap(_)));
        let pointer = buffer.as_str().as_ptr();
        buffer.clear();
        buffer.push_str("short");
        assert_eq!(buffer.as_str(), "short");
        assert_eq!(buffer.as_str().as_ptr(), pointer);
        for (length, device) in [(45, 0), (36, u32::MAX)] {
            let address = ProtocolAddress::new(&"a".repeat(length), device.into());
            assert!(matches!(address.text.0, AddressStorage::Inline(_)));
        }
    }

    #[test]
    fn deserialization_rejects_inconsistent_cached_name_boundaries() {
        for (text, name_len, device) in [
            ("λ.0", 1, 0),
            ("a.0", usize::MAX, 0),
            ("a.1", 1, 0),
            ("a", 1, 0),
        ] {
            assert!(
                serde_json::from_value::<ProtocolAddress>(serde_json::json!({
                    "text": text, "name_len": name_len, "device": device,
                }))
                .is_err()
            );
        }
    }

    #[test]
    fn address_text_and_serde_contract_across_utf8_and_storage_boundaries() {
        use sha2::{Digest, Sha256};
        let mut digest = Sha256::new();
        for name in [
            String::new(),
            "synthetic@lid".to_owned(),
            "a".repeat(23),
            "a".repeat(45),
            "a".repeat(47),
            "a".repeat(48),
            "a".repeat(127),
            "a".repeat(4096),
            "λ".repeat(23),
            "🦀".repeat(12),
            "name.with.dots.4294967295".to_owned(),
        ] {
            for device in [0, 127, u32::MAX] {
                let address = ProtocolAddress::new(&name, device.into());
                let expected = format!("{name}.{device}");
                assert_eq!(address.name(), name);
                assert_eq!(address.as_str(), expected);
                let json = serde_json::to_vec(&address).unwrap();
                assert_eq!(
                    serde_json::from_slice::<serde_json::Value>(&json).unwrap(),
                    serde_json::json!({"text": expected, "name_len": name.len(), "device": device})
                );
                assert_eq!(
                    serde_json::from_slice::<ProtocolAddress>(&json).unwrap(),
                    address
                );
                let mut cbor = Vec::new();
                ciborium::into_writer(&address, &mut cbor).unwrap();
                assert_eq!(
                    ciborium::from_reader::<ProtocolAddress, _>(cbor.as_slice()).unwrap(),
                    address
                );
                for encoded in [json, cbor] {
                    digest.update((encoded.len() as u64).to_le_bytes());
                    digest.update(encoded);
                }
            }
        }
        assert_eq!(
            hex::encode(digest.finalize()),
            "7300254ee7efccfff957393831150d98639bcb8b0bffdcc6ef0f1a7175cb4480"
        );
    }

    #[test]
    fn address_reuse_and_clones_have_identical_map_keys_and_order() {
        use std::collections::{BTreeMap, HashMap};
        let mut reused = ProtocolAddress::new(&"λ".repeat(4096), u32::MAX.into());
        for name in ["", "🦀", "synthetic@lid", &"λ".repeat(23), &"a".repeat(48)] {
            reused.reset_with(|out| {
                for character in name.chars() {
                    out.push(character);
                }
            });
            let fresh = ProtocolAddress::new(name, u32::MAX.into());
            let snapshot = reused.clone();
            let map = HashMap::from([(reused.clone(), 17)]);
            let ordered = BTreeMap::from([(reused.clone(), 17)]);
            assert_eq!(map.get(&fresh), Some(&17));
            assert_eq!(ordered.get(&fresh), Some(&17));
            reused.reset_with(|out| out.push_str("changed"));
            assert_eq!(snapshot, fresh);
            assert_eq!(snapshot.name(), name);
        }
        let names = ["b", "", "λ", "a.0", "a"];
        let mut addresses: Vec<_> = names
            .iter()
            .map(|name| ProtocolAddress::new(name, 0.into()))
            .collect();
        addresses.sort();
        let mut expected: Vec<_> = names.iter().map(|name| format!("{name}.0")).collect();
        expected.sort();
        assert_eq!(
            addresses
                .iter()
                .map(ProtocolAddress::as_str)
                .collect::<Vec<_>>(),
            expected
        );
    }

    #[test]
    fn address_cache_key_and_reuse() {
        let mut address = ProtocolAddress::new("synthetic@lid", DeviceId::new(0));
        assert_eq!(address.name(), "synthetic@lid");
        assert_eq!(address.as_str(), "synthetic@lid.0");
        address.reset_with(|name| name.push_str("short@lid"));
        assert_eq!(address.name(), "short@lid");
        assert_eq!(address.as_str(), "short@lid.0");
    }

    #[test]
    fn decimal_device_suffix_preserves_names_and_empty_addresses() {
        for device in [0, 1, 9, 10, 255, 65535, u32::MAX] {
            let mut address = ProtocolAddress::empty(device.into());
            assert_eq!(address.as_str(), format!(".{device}"));
            for name in ["", "synthetic@lid", "utf8-λ", "name.with.dots"] {
                address.reset_with(|out| out.push_str(name));
                assert_eq!(address.name(), name);
                assert_eq!(address.as_str(), format!("{name}.{device}"));
                assert_eq!(ProtocolAddress::new(name, device.into()), address);
            }
        }
    }
}

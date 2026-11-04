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
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct AddressBuf(String);
impl AddressBuf {
    pub fn empty() -> Self {
        Self(String::with_capacity(64))
    }
    pub fn clear(&mut self) {
        self.0.clear()
    }
    pub fn push_str(&mut self, s: &str) {
        self.0.push_str(s)
    }
    pub fn push(&mut self, c: char) {
        self.0.push(c)
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ProtocolAddress {
    text: AddressBuf,
    name_len: usize,
    device: DeviceId,
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
            name_len: 0,
            device,
        };
        out.reset_with(|buf| buf.push_str(name));
        out
    }
    pub fn empty(device: DeviceId) -> Self {
        let mut out = Self {
            text: AddressBuf::empty(),
            name_len: 0,
            device,
        };
        out.reset_with(|_| {});
        out
    }
    pub fn reset_with(&mut self, write: impl FnOnce(&mut AddressBuf)) {
        self.text.clear();
        write(&mut self.text);
        self.name_len = self.text.0.len();
        self.text.push('.');
        self.text
            .push_str(itoa::Buffer::new().format(self.device.0));
    }
    pub fn name(&self) -> &str {
        &self.text.0[..self.name_len]
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

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
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ProtocolAddress {
    text: AddressBuf,
    name_len: usize,
    device: DeviceId,
}
impl ProtocolAddress {
    pub fn new(name: impl AsRef<str>, device: DeviceId) -> Self {
        let mut out = Self::empty(device);
        out.reset_with(|buf| buf.push_str(name.as_ref()));
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
        use std::fmt::Write;
        let _ = write!(self.text.0, ".{}", self.device);
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
    fn address_cache_key_and_reuse() {
        let mut address = ProtocolAddress::new("synthetic@lid", DeviceId::new(0));
        assert_eq!(address.name(), "synthetic@lid");
        assert_eq!(address.as_str(), "synthetic@lid.0");
        address.reset_with(|name| name.push_str("short@lid"));
        assert_eq!(address.name(), "short@lid");
        assert_eq!(address.as_str(), "short@lid.0");
    }
}

use std::fmt;
use std::hash::{Hash, Hasher};

use uuid::Uuid;

#[derive(Clone, Copy, Hash, PartialEq, Eq, derive_more::TryFrom)]
#[try_from(repr)]
#[repr(u8)]
pub enum ServiceIdKind {
    Aci,
    Pni,
}

impl From<ServiceIdKind> for u8 {
    fn from(value: ServiceIdKind) -> Self {
        value as u8
    }
}

impl fmt::Display for ServiceIdKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ServiceIdKind::Aci => f.write_str("ACI"),
            ServiceIdKind::Pni => f.write_str("PNI"),
        }
    }
}

impl fmt::Debug for ServiceIdKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self}")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WrongKindOfServiceIdError {
    pub expected: ServiceIdKind,
    pub actual: ServiceIdKind,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SpecificServiceId<const RAW_KIND: u8>(Uuid);

impl<const KIND: u8> SpecificServiceId<KIND> {
    #[inline]
    pub const fn from_uuid_bytes(bytes: [u8; 16]) -> Self {
        Self::from_uuid(uuid::Uuid::from_bytes(bytes))
    }

    #[inline]
    const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl<const KIND: u8> std::hash::Hash for SpecificServiceId<KIND> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        state.write(self.0.as_bytes());
    }
}

impl<const KIND: u8> From<Uuid> for SpecificServiceId<KIND> {
    #[inline]
    fn from(value: Uuid) -> Self {
        Self::from_uuid(value)
    }
}

impl<const KIND: u8> From<SpecificServiceId<KIND>> for Uuid {
    #[inline]
    fn from(value: SpecificServiceId<KIND>) -> Self {
        value.0
    }
}

impl<const KIND: u8> fmt::Debug for SpecificServiceId<KIND>
where
    ServiceId: From<Self>,
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        ServiceId::from(*self).fmt(f)
    }
}

pub type Aci = SpecificServiceId<{ ServiceIdKind::Aci as u8 }>;

pub type Pni = SpecificServiceId<{ ServiceIdKind::Pni as u8 }>;

pub type ServiceIdFixedWidthBinaryBytes = [u8; 17];

#[derive(Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord, derive_more::From)]
pub enum ServiceId {
    Aci(Aci),
    Pni(Pni),
}

impl ServiceId {
    #[inline]
    pub fn kind(&self) -> ServiceIdKind {
        match self {
            ServiceId::Aci(_) => ServiceIdKind::Aci,
            ServiceId::Pni(_) => ServiceIdKind::Pni,
        }
    }

    #[inline]
    pub fn raw_uuid(self) -> Uuid {
        match self {
            ServiceId::Aci(aci) => aci.into(),
            ServiceId::Pni(pni) => pni.into(),
        }
    }
}

impl fmt::Debug for ServiceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<{}:{}>", self.kind(), self.raw_uuid())
    }
}

impl<const KIND: u8> TryFrom<ServiceId> for SpecificServiceId<KIND> {
    type Error = WrongKindOfServiceIdError;

    #[inline]
    fn try_from(value: ServiceId) -> Result<Self, Self::Error> {
        if u8::from(value.kind()) == KIND {
            Ok(value.raw_uuid().into())
        } else {
            Err(WrongKindOfServiceIdError {
                expected: KIND
                    .try_into()
                    .expect("invalid kind, not covered in ServiceIdKind"),
                actual: value.kind(),
            })
        }
    }
}

impl<const KIND: u8> PartialEq<ServiceId> for SpecificServiceId<KIND>
where
    ServiceId: From<SpecificServiceId<KIND>>,
{
    fn eq(&self, other: &ServiceId) -> bool {
        ServiceId::from(*self) == *other
    }
}

impl<const KIND: u8> PartialEq<SpecificServiceId<KIND>> for ServiceId
where
    ServiceId: From<SpecificServiceId<KIND>>,
{
    fn eq(&self, other: &SpecificServiceId<KIND>) -> bool {
        *self == ServiceId::from(*other)
    }
}

#[derive(
    Copy, Clone, Debug, Hash, Eq, PartialEq, PartialOrd, Ord, derive_more::From, derive_more::Into,
)]
pub struct DeviceId(u32);

impl DeviceId {
    #[inline]
    pub const fn new(id: u32) -> Self {
        Self(id)
    }
}

impl fmt::Display for DeviceId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

const fn digit_count(n: u32) -> usize {
    if n == 0 {
        return 1;
    }
    n.ilog10() as usize + 1
}

#[inline]
fn append_device_suffix(buf: &mut String, device_id: DeviceId) {
    let id = u32::from(device_id);
    if id == 0 {
        buf.push_str(".0");
    } else {
        use std::fmt::Write;
        write!(buf, ".{id}").unwrap();
    }
}

/// Single-buffer protocol address. The buffer stores `"{name}.{device_id}"` and
/// `name_len` marks where the name ends, so `name()` and `as_str()` are both
/// zero-cost slices. One String instead of two — halves allocation count for
/// one-shot construction and eliminates the copy in `reset_with()`.
#[derive(Clone, Debug)]
pub struct ProtocolAddress {
    buf: String,
    name_len: usize,
    device_id: DeviceId,
}

impl ProtocolAddress {
    pub fn new(name: String, device_id: DeviceId) -> Self {
        let name_len = name.len();
        let mut buf = name;
        append_device_suffix(&mut buf, device_id);
        Self {
            buf,
            name_len,
            device_id,
        }
    }

    /// Pre-allocated empty address. Call `reset_with()` to fill.
    pub fn with_capacity(capacity: usize, device_id: DeviceId) -> Self {
        let suffix_len = 1 + digit_count(u32::from(device_id));
        Self {
            buf: String::with_capacity(capacity + suffix_len),
            name_len: 0,
            device_id,
        }
    }

    /// Write the name via closure, then append the device_id suffix.
    /// Single write pass — no intermediate copy.
    pub fn reset_with(&mut self, write_name: impl FnOnce(&mut String)) {
        self.buf.clear();
        write_name(&mut self.buf);
        self.name_len = self.buf.len();
        append_device_suffix(&mut self.buf, self.device_id);
    }

    #[inline]
    pub fn name(&self) -> &str {
        &self.buf[..self.name_len]
    }

    #[inline]
    pub fn device_id(&self) -> DeviceId {
        self.device_id
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        &self.buf
    }
}

impl PartialEq for ProtocolAddress {
    fn eq(&self, other: &Self) -> bool {
        self.buf == other.buf
    }
}

impl Eq for ProtocolAddress {}

impl Hash for ProtocolAddress {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.buf.hash(state);
    }
}

impl PartialOrd for ProtocolAddress {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ProtocolAddress {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.buf.cmp(&other.buf)
    }
}

impl fmt::Display for ProtocolAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.buf)
    }
}

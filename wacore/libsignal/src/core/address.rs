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
        Self::from_uuid(Uuid::from_bytes(bytes))
    }

    #[inline]
    const fn from_uuid(uuid: Uuid) -> Self {
        Self(uuid)
    }
}

impl<const KIND: u8> Hash for SpecificServiceId<KIND> {
    fn hash<H: Hasher>(&self, state: &mut H) {
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

#[inline]
fn append_device_suffix(buf: &mut AddressBuf, device_id: DeviceId) {
    let id = u32::from(device_id);
    if id == 0 {
        buf.push_str(".0");
    } else {
        use std::fmt::Write;
        write!(buf, ".{id}").unwrap();
    }
}

/// Longest address held without touching the heap.
///
/// A real address is short: `"5511987650001:5@c.us.0"` is 22 bytes and the
/// longest server the JID enum can render adds ten more, so this covers every
/// address the protocol produces with room for drift. Anything longer still
/// works -- it spills to a `String`, which is where every address used to live
/// unconditionally.
const INLINE_CAPACITY: usize = 47;

/// A protocol address's characters: inline while they fit, heap when they do
/// not.
///
/// Which arm holds them is not part of the value. `ProtocolAddress` is a
/// `HashMap` key in the session cache, so equality, ordering and hashing all go
/// through [`AddressBuf::as_str`] and never look at the representation: the
/// same characters answer identically whether they were built inline or spilled.
#[derive(Clone)]
pub struct AddressBuf(AddressRepr);

/// Prints what the buffer currently holds, never the array behind it.
///
/// `clear` only rewinds the length, so the tail of an inline buffer still
/// holds the previous address's bytes. A derived `Debug` would print all of
/// the inline capacity and leak an unrelated peer's JID into any log line that
/// formats an address, or an error that embeds one.
impl fmt::Debug for AddressBuf {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self.as_str(), f)
    }
}

#[derive(Clone)]
enum AddressRepr {
    Inline {
        bytes: [u8; INLINE_CAPACITY],
        len: u8,
    },
    Heap(String),
}

impl Default for AddressBuf {
    #[inline]
    fn default() -> Self {
        Self::empty()
    }
}

impl AddressBuf {
    /// An empty buffer, holding nothing on the heap.
    #[inline]
    pub fn empty() -> Self {
        Self(AddressRepr::Inline {
            bytes: [0; INLINE_CAPACITY],
            len: 0,
        })
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        match &self.0 {
            // Only whole `&str` fragments are ever appended, never split, so
            // the inline bytes are valid UTF-8 by construction.
            AddressRepr::Inline { bytes, len } => std::str::from_utf8(&bytes[..usize::from(*len)])
                .expect("address is built from whole str fragments"),
            AddressRepr::Heap(buf) => buf,
        }
    }

    #[inline]
    fn len(&self) -> usize {
        match &self.0 {
            AddressRepr::Inline { len, .. } => usize::from(*len),
            AddressRepr::Heap(buf) => buf.len(),
        }
    }

    /// Empty the buffer, keeping whatever room it already has: an address that
    /// once needed the heap is usually rewritten to something just as long.
    #[inline]
    pub fn clear(&mut self) {
        match &mut self.0 {
            AddressRepr::Inline { len, .. } => *len = 0,
            AddressRepr::Heap(buf) => buf.clear(),
        }
    }

    pub fn push_str(&mut self, s: &str) {
        match &mut self.0 {
            AddressRepr::Inline { bytes, len } => {
                let start = usize::from(*len);
                let end = start + s.len();
                if end <= INLINE_CAPACITY {
                    bytes[start..end].copy_from_slice(s.as_bytes());
                    *len = end as u8;
                    return;
                }
                let mut heap = String::with_capacity(end);
                // Valid UTF-8 for the same reason as in `as_str`.
                heap.push_str(
                    std::str::from_utf8(&bytes[..start])
                        .expect("address is built from whole str fragments"),
                );
                heap.push_str(s);
                self.0 = AddressRepr::Heap(heap);
            }
            AddressRepr::Heap(buf) => buf.push_str(s),
        }
    }

    #[inline]
    pub fn push(&mut self, c: char) {
        self.push_str(c.encode_utf8(&mut [0u8; 4]));
    }
}

impl fmt::Write for AddressBuf {
    #[inline]
    fn write_str(&mut self, s: &str) -> fmt::Result {
        self.push_str(s);
        Ok(())
    }
}

/// Single-buffer protocol address. The buffer stores `"{name}.{device_id}"` and
/// `name_len` marks where the name ends, so `name()` and `as_str()` are both
/// zero-cost slices. One buffer instead of two — halves allocation count for
/// one-shot construction and eliminates the copy in `reset_with()`. The buffer
/// itself is inline up to 47 bytes, so the common address is a value
/// with nothing behind it.
#[derive(Clone, Debug)]
pub struct ProtocolAddress {
    buf: AddressBuf,
    name_len: usize,
    device_id: DeviceId,
}

impl ProtocolAddress {
    pub fn new(name: &str, device_id: DeviceId) -> Self {
        let mut address = Self::empty(device_id);
        address.reset_with(|buf| buf.push_str(name));
        address
    }

    /// An address with no name yet, ready for [`Self::reset_with`]. No capacity
    /// argument: the buffer starts inline and grows only if an address ever
    /// exceeds it.
    ///
    /// The device suffix is written immediately even though the name is empty.
    /// `Hash`, `Eq` and `Ord` all read the rendered string, so leaving the
    /// buffer truly empty would make every unnamed address compare equal
    /// regardless of its device, and two of them would collide as map keys.
    pub fn empty(device_id: DeviceId) -> Self {
        let mut buf = AddressBuf::empty();
        append_device_suffix(&mut buf, device_id);
        Self {
            buf,
            name_len: 0,
            device_id,
        }
    }

    /// Write the name via closure, then append the device_id suffix.
    /// Single write pass — no intermediate copy.
    pub fn reset_with(&mut self, write_name: impl FnOnce(&mut AddressBuf)) {
        self.buf.clear();
        write_name(&mut self.buf);
        self.name_len = self.buf.len();
        append_device_suffix(&mut self.buf, self.device_id);
    }

    #[inline]
    pub fn name(&self) -> &str {
        &self.buf.as_str()[..self.name_len]
    }

    #[inline]
    pub fn device_id(&self) -> DeviceId {
        self.device_id
    }

    #[inline]
    pub fn as_str(&self) -> &str {
        self.buf.as_str()
    }
}

impl PartialEq for ProtocolAddress {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl Eq for ProtocolAddress {}

impl Hash for ProtocolAddress {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.as_str().hash(state);
    }
}

impl PartialOrd for ProtocolAddress {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ProtocolAddress {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.as_str().cmp(other.as_str())
    }
}

impl fmt::Display for ProtocolAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod address_buffer_tests {

    /// Every comparison reads the rendered string, so an address with no name
    /// yet still has to carry its device. Two unnamed addresses that differ
    /// only by device would otherwise be the same map key, and the session
    /// cache would serve one device's entry to another.
    #[test]
    fn unnamed_addresses_still_differ_by_device() {
        let first = ProtocolAddress::empty(DeviceId::new(1));
        let second = ProtocolAddress::empty(DeviceId::new(2));

        assert_ne!(first, second, "the device must keep them apart");
        assert_ne!(
            hash_of(&first),
            hash_of(&second),
            "equal hashes would collide them in the session cache"
        );
        assert_eq!(first.name(), "", "neither has a name yet");
        assert_eq!(first.device_id(), DeviceId::new(1));
    }

    /// Reusing an address for a shorter name leaves the previous one's bytes in
    /// the inline tail, because the reset only rewinds the length. Anything
    /// that prints the backing array rather than the live slice therefore leaks
    /// the peer we were talking to before, into any log line that formats an
    /// address or an error carrying one.
    #[test]
    fn debug_never_shows_the_address_that_was_there_before() {
        let mut address = ProtocolAddress::new("5511999998888@s.whatsapp.net", 1.into());
        assert!(
            address.buf.is_inline(),
            "precondition: this name fits inline"
        );
        let previous_tail = "8888@s.whatsapp.net";
        assert!(
            format!("{:?}", address.buf).contains(previous_tail),
            "precondition: the long name is what the buffer holds"
        );

        address.reset_with(|buf| buf.push_str("55119@lid"));

        let shown = format!("{:?}", address.buf);
        assert!(
            !shown.contains(previous_tail),
            "the previous address must not survive into the output: {shown}"
        );
        assert!(
            !shown.contains('['),
            "printing the backing array is what leaks the tail: {shown}"
        );
    }

    /// The same guarantee one level up, since this is the type that actually
    /// reaches logs and error messages.
    #[test]
    fn a_protocol_address_prints_only_its_own_name() {
        let mut addr = ProtocolAddress::new("5511999998888@s.whatsapp.net", 1.into());
        addr.reset_with(|buf| buf.push_str("55119@lid"));

        let shown = format!("{addr:?}");
        assert!(
            !shown.contains("8888"),
            "a reused address must not print the previous peer: {shown}"
        );
        assert!(
            shown.contains("55119@lid"),
            "it must still print what it holds: {shown}"
        );
    }
    use super::*;
    use std::collections::hash_map::DefaultHasher;

    impl AddressBuf {
        /// Test-only view of the representation. Production code must never
        /// branch on this: the whole contract is that the two arms are
        /// indistinguishable as values.
        fn is_inline(&self) -> bool {
            matches!(self.0, AddressRepr::Inline { .. })
        }
    }

    fn hash_of(address: &ProtocolAddress) -> u64 {
        let mut hasher = DefaultHasher::new();
        address.hash(&mut hasher);
        hasher.finish()
    }

    /// A name that fits, plus its device suffix, never reaches the heap, and
    /// still splits into name and full string at the right place.
    #[test]
    fn a_name_that_fits_stays_inline_and_reads_back_whole() {
        let address = ProtocolAddress::new("5511987650001:5@c.us", DeviceId::new(0));
        assert!(address.buf.is_inline());
        assert_eq!(address.name(), "5511987650001:5@c.us");
        assert_eq!(address.as_str(), "5511987650001:5@c.us.0");
        assert_eq!(address.device_id(), DeviceId::new(0));
    }

    /// The exact boundary in both directions: the last name that fits with its
    /// suffix, and the first one that does not.
    #[test]
    fn the_inline_boundary_holds_on_both_sides() {
        let suffix_len = ".0".len();
        let longest_fitting = "a".repeat(INLINE_CAPACITY - suffix_len);
        let fits = ProtocolAddress::new(&longest_fitting, DeviceId::new(0));
        assert!(
            fits.buf.is_inline(),
            "the longest fitting name must stay inline"
        );
        assert_eq!(fits.as_str().len(), INLINE_CAPACITY);
        assert_eq!(fits.name(), longest_fitting);

        let one_too_long = "a".repeat(INLINE_CAPACITY - suffix_len + 1);
        let spills = ProtocolAddress::new(&one_too_long, DeviceId::new(0));
        assert!(!spills.buf.is_inline(), "one byte over must spill");
        assert_eq!(spills.name(), one_too_long);
        assert_eq!(spills.as_str(), format!("{one_too_long}.0"));
    }

    /// A multi-digit device id takes a different path than `".0"`: the suffix
    /// is written through `write!` rather than one `push_str`, so the spill can
    /// land *between* the dot and the digits. Getting that wrong would leave a
    /// half-written suffix or a `name_len` measured after it, which is why the
    /// case is worth pinning separately from the fast path above.
    #[test]
    fn a_multi_digit_suffix_spills_without_splitting_itself() {
        let suffix = ".123";
        // Sized so the name alone fits and the suffix is what pushes it over.
        let name = "b".repeat(INLINE_CAPACITY - suffix.len() + 1);
        let address = ProtocolAddress::new(&name, DeviceId::new(123));

        assert!(
            !address.buf.is_inline(),
            "the suffix is what must have pushed this past the inline capacity"
        );
        assert_eq!(
            address.as_str(),
            format!("{name}{suffix}"),
            "a fragment written across the spill would corrupt the address"
        );
        assert_eq!(
            address.name(),
            name,
            "name_len must still point at the end of the name, not into the suffix"
        );
        assert_eq!(address.device_id(), DeviceId::new(123));
    }

    /// The representation is not part of the value. `ProtocolAddress` is a
    /// `HashMap` key, so an inline value and a heap value holding the same
    /// characters must compare, order and hash the same.
    #[test]
    fn the_same_characters_compare_and_hash_alike_from_either_representation() {
        let name = "5511987650001:5@c.us";
        let inline = ProtocolAddress::new(name, DeviceId::new(0));

        // Force the heap arm, then rewrite it to the short name: a cleared
        // heap buffer keeps its allocation, so this really is the same
        // characters in the other representation.
        let mut spilled = ProtocolAddress::new(&"z".repeat(INLINE_CAPACITY * 2), DeviceId::new(0));
        assert!(!spilled.buf.is_inline());
        spilled.reset_with(|buf| buf.push_str(name));
        assert!(
            !spilled.buf.is_inline(),
            "the fixture must still be on the heap, or it proves nothing"
        );
        assert!(inline.buf.is_inline());

        assert_eq!(inline, spilled);
        assert_eq!(inline.cmp(&spilled), std::cmp::Ordering::Equal);
        assert_eq!(hash_of(&inline), hash_of(&spilled));
        assert_eq!(inline.name(), spilled.name());
        assert_eq!(inline.as_str(), spilled.as_str());
        assert_eq!(inline.to_string(), spilled.to_string());

        let mut map = std::collections::HashMap::new();
        map.insert(inline, "value");
        assert_eq!(
            map.get(&spilled).copied(),
            Some("value"),
            "a heap-built key must find the inline-built entry"
        );
    }

    /// Different characters must still differ, whichever arm holds them --
    /// otherwise the equality above would be trivially true.
    #[test]
    fn different_characters_stay_different_across_representations() {
        let inline = ProtocolAddress::new("alice@c.us", DeviceId::new(0));
        let mut spilled = ProtocolAddress::new(&"z".repeat(INLINE_CAPACITY * 2), DeviceId::new(0));
        spilled.reset_with(|buf| buf.push_str("bob@c.us"));
        assert!(!spilled.buf.is_inline());

        assert_ne!(inline, spilled);
        assert_eq!(inline.cmp(&spilled), std::cmp::Ordering::Less);
    }

    /// Multi-byte characters survive both the inline copy and the spill. The
    /// spill copies whole appended fragments, so it can never land inside a
    /// character.
    #[test]
    fn multibyte_names_survive_inline_and_spilled() {
        let short = "héllo✅@c.us";
        let inline = ProtocolAddress::new(short, DeviceId::new(0));
        assert!(inline.buf.is_inline());
        assert_eq!(inline.name(), short);

        // Fill to one byte short of the limit, then append a two-byte char:
        // the append cannot fit, so the whole fragment moves to the heap.
        let filler = "a".repeat(INLINE_CAPACITY - 1);
        let mut spilled = ProtocolAddress::empty(DeviceId::new(0));
        spilled.reset_with(|buf| {
            buf.push_str(&filler);
            buf.push('é');
        });
        assert!(!spilled.buf.is_inline());
        assert_eq!(spilled.name(), format!("{filler}é"));
        assert_eq!(spilled.as_str(), format!("{filler}é.0"));
    }

    /// An empty name is a real state (a freshly reset address), and the
    /// suffix still lands.
    #[test]
    fn an_empty_name_still_carries_its_device_suffix() {
        let address = ProtocolAddress::new("", DeviceId::new(0));
        assert!(address.buf.is_inline());
        assert_eq!(address.name(), "");
        assert_eq!(address.as_str(), ".0");
    }

    /// A non-zero device id is rendered, and the name boundary still excludes
    /// the suffix however many digits it takes.
    #[test]
    fn a_multi_digit_device_id_is_appended_outside_the_name() {
        let address = ProtocolAddress::new("alice@c.us", DeviceId::new(123));
        assert_eq!(address.name(), "alice@c.us");
        assert_eq!(address.as_str(), "alice@c.us.123");
        assert_eq!(address.device_id(), DeviceId::new(123));
    }

    /// Rewriting an address must replace its content, not append to it.
    #[test]
    fn resetting_replaces_the_previous_name() {
        let mut address = ProtocolAddress::new("alice@c.us", DeviceId::new(0));
        address.reset_with(|buf| buf.push_str("bob@c.us"));
        assert_eq!(address.name(), "bob@c.us");
        assert_eq!(address.as_str(), "bob@c.us.0");

        let mut spilled = ProtocolAddress::new(&"z".repeat(INLINE_CAPACITY * 2), DeviceId::new(0));
        spilled.reset_with(|buf| buf.push_str("bob@c.us"));
        assert_eq!(spilled.as_str(), "bob@c.us.0");
    }
}

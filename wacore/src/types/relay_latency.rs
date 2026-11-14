//! Peer relay measurements. These are signaling metadata, never dial instructions.
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RelayLatencyKind {
    Normal,
    Xpop,
}

/// Packed native latency, retaining round/flags and the signed 24-bit RTT.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct PackedRelayLatency(u32);

impl PackedRelayLatency {
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }
    pub const fn raw(self) -> u32 {
        self.0
    }
    pub const fn round(self) -> u8 {
        ((self.0 >> 25) & 63) as u8
    }
    /// `None` is the native unknown/unreachable sentinel, not a measured RTT.
    pub const fn rtt_ms(self) -> Option<i32> {
        let value = ((self.0 << 8) as i32) >> 8;
        if value == 0x7f_ffff {
            None
        } else {
            Some(value)
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct RelayLatencyEntry {
    pub kind: RelayLatencyKind,
    pub latency: PackedRelayLatency,
    pub relay_name: String,
    /// Original IPv4/port (6 bytes) or IPv6/port (18 bytes); untrusted peer metadata.
    pub address: Vec<u8>,
}

impl RelayLatencyEntry {
    pub fn new(
        kind: RelayLatencyKind,
        latency: PackedRelayLatency,
        relay_name: String,
        address: Vec<u8>,
    ) -> Option<Self> {
        matches!(address.len(), 6 | 18).then_some(Self {
            kind,
            latency,
            relay_name,
            address,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct RelayLatencyReport {
    pub entries: Vec<RelayLatencyEntry>,
}
impl RelayLatencyReport {
    pub fn new(entries: Vec<RelayLatencyEntry>) -> Self {
        Self { entries }
    }
}

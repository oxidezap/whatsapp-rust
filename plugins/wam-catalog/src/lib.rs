//! The WhatsApp Metrics (WAM) event catalog and the buffer codec that writes it.
//!
//! WAM is the telemetry the official client uploads: a binary buffer of events,
//! each a list of numbered fields, preceded by buffer-level globals. This crate
//! is the schema half (every event, field id, enum member, global and constant
//! WA Web declares, generated from the whatspec IR) plus a byte-exact encoder
//! for the buffer format those definitions are written in.
//!
//! It knows nothing about a client. It cannot decide whether an event may be
//! emitted, cannot read a connection and holds no state beyond the buffer it is
//! building; that belongs to the plugin on top. Two crates rather than one so
//! the catalog's compile cost is paid and measured once, independently of the
//! runtime that uses a dozen of its 436 events.
//!
//! # What this does not guarantee
//!
//! - **That an event is correct to send.** The catalog says a field exists and
//!   what its id is. Whether this client can honestly fill it is a separate
//!   question, answered in the plugin and checked against [`call_sites`].
//! - **That the catalog matches today's WhatsApp.** It matches the pinned
//!   whatspec commit, which is a static reading of one bundle. Regenerating is
//!   how it moves; nothing here detects drift at runtime.
//! - **Any channel but the one you encode on.** [`WamBuffer`] enforces that a
//!   global is legal on its channel, but the `private` channel additionally
//!   needs a blind-signed token this crate does not produce.
//!
//! # Encoding
//!
//! ```text
//! "WAM" | version:u8 | streamId:u8 | sequence:u16le | channel:u8
//! then, per record: flags:u8 | id:u8|u16le | payload
//! ```
//!
//! The low two bits of `flags` say what the record is (global, event, field),
//! bit 2 marks the last record of an event's group, bit 3 widens the id to
//! 16 bits, and the high nibble selects the payload's size class.

#![forbid(unsafe_code)]

use std::collections::BTreeMap;

pub mod generated;

#[cfg(feature = "parity")]
pub mod call_sites;

pub use generated::{PRIVATE_STATS_IDS, constants, enums, events, globals};

/// Which sampling channel a buffer or event belongs to.
///
/// The wire byte in the buffer header, not a name: `regular` is 0, `realtime`
/// is 1 and anything else, in practice only `private`, is 2.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Channel {
    Regular,
    Realtime,
    Private,
}

impl Channel {
    /// The byte the buffer header carries for this channel.
    pub const fn wire(self) -> u8 {
        match self {
            Self::Regular => 0,
            Self::Realtime => 1,
            Self::Private => 2,
        }
    }

    /// The channel a buffer of this channel writes globals for.
    ///
    /// `realtime` shares `regular`'s globals: the official writer maps it onto
    /// `regular` before consulting a global's channel list, so a `realtime`
    /// buffer legally carries every `regular` global and no `realtime`-only one
    /// exists to carry.
    pub const fn global_scope(self) -> Self {
        match self {
            Self::Realtime => Self::Regular,
            other => other,
        }
    }
}

/// The set of channels on which writing a global is legal.
///
/// A set rather than a list so the check is a mask test, and a distinct type
/// from [`Channel`] so a global's legality cannot be mistaken for the channel it
/// is being written to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChannelSet(u8);

impl ChannelSet {
    /// The set holding exactly `channels`.
    pub const fn of(channels: &[Channel]) -> Self {
        let mut bits = 0u8;
        let mut i = 0;
        while i < channels.len() {
            bits |= 1 << channels[i].wire();
            i += 1;
        }
        Self(bits)
    }

    /// Whether a global in this set may be written on `channel`.
    ///
    /// Takes the channel's [`global_scope`](Channel::global_scope), so a
    /// `realtime` buffer is answered for `regular` exactly as the official
    /// writer answers it.
    pub const fn allows(self, channel: Channel) -> bool {
        self.0 & (1 << channel.global_scope().wire()) != 0
    }
}

/// The declared type of a field or global.
///
/// `Timer` is a millisecond duration and `Enum` is an integer member: both reach
/// the wire as integers, and the distinction exists so a reader of the catalog
/// can tell what a number means.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
    Boolean,
    Integer,
    Number,
    String,
    Timer,
    Enum,
}

/// One buffer-level global: its bundle name, its wire id, its type and the
/// channels it may legally be written on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlobalDef {
    pub name: &'static str,
    pub id: u32,
    pub kind: ValueKind,
    pub channels: ChannelSet,
}

/// One private-stats rotation group: the anonymous id a `private` buffer writes
/// and how often it rotates. `rotation_period_days` is `-1` for a group that
/// does not rotate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrivateStatsId {
    pub key: &'static str,
    pub id: i64,
    pub rotation_period_days: i64,
}

/// A value a global can hold. Event fields go through [`EventFields`] instead,
/// which is typed by the generated struct.
#[derive(Debug, Clone, PartialEq)]
pub enum GlobalValue<'a> {
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(&'a str),
}

impl GlobalValue<'_> {
    /// The kind this value would be read back as, for reporting a mismatch.
    fn kind(&self) -> ValueKind {
        match self {
            Self::Bool(_) => ValueKind::Boolean,
            Self::Int(_) => ValueKind::Integer,
            Self::Float(_) => ValueKind::Number,
            Self::Str(_) => ValueKind::String,
        }
    }

    /// Whether this value is one the catalog's declared kind accepts.
    ///
    /// Three kinds share the integer shape and are not interchangeable in the
    /// catalog, only on the wire: a timer and an enum are both written as an
    /// integer, and an enum member reaches here already resolved to its wire
    /// value. `Number` takes either, because the official numeric encoder sends
    /// an integral double as an integer anyway.
    fn fits(&self, kind: ValueKind) -> bool {
        matches!(
            (kind, self),
            (ValueKind::Boolean, Self::Bool(_))
                | (
                    ValueKind::Integer | ValueKind::Timer | ValueKind::Enum,
                    Self::Int(_)
                )
                | (ValueKind::Number, Self::Int(_) | Self::Float(_))
                | (ValueKind::String, Self::Str(_))
        )
    }

    fn to_owned_value(&self) -> OwnedGlobalValue {
        match self {
            // A boolean reaches the wire as 0 or 1, and the official writer
            // coerces it before comparing against the previous value. Coercing
            // here too is what makes `true` after `1` a no-op rather than a
            // rewrite.
            Self::Bool(b) => OwnedGlobalValue::Int(i64::from(*b)),
            Self::Int(i) => OwnedGlobalValue::Int(*i),
            Self::Float(f) => OwnedGlobalValue::Float(*f),
            Self::Str(s) => OwnedGlobalValue::Str((*s).to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
enum OwnedGlobalValue {
    Int(i64),
    Float(f64),
    Str(String),
    /// A global explicitly cleared. Written as an empty record, which is how a
    /// buffer says "this global no longer has a value" rather than repeating
    /// the old one.
    Null,
}

/// One generated WAM event.
///
/// Implemented only by the generated structs. `encode` writes the event's fields
/// in the order the catalog declares them; the buffer, not the event, decides
/// which record ends the group, so an implementation never has to know whether
/// a field is its last.
pub trait WamEvent {
    /// The event's name in the bundle, used by diagnostics and the parity test.
    const NAME: &'static str;
    /// The wire id.
    const CODE: u32;
    /// The channel the event belongs to. A buffer refuses an event from another.
    const CHANNEL: Channel;
    /// The catalog's declared weights, `[alpha, beta, release]`. Not the weight
    /// a buffer carries: [`sampling`] picks one and a runtime override can
    /// replace it.
    const WEIGHTS: [u32; 3];
    /// The rotation group a `private` buffer would name for this event.
    const PRIVATE_STATS_ID: Option<i64>;

    fn encode(&self, fields: &mut EventFields<'_>);
}

/// Which declared weight a client uses, and what a weight means.
pub mod sampling {
    /// The weight a production web client uses: index 2 of the declared
    /// `[alpha, beta, release]`.
    ///
    /// The bundle picks by gate, not by position: with no gate set it uses a
    /// literal `1`, one gate selects index 1 and another selects index 2. The
    /// same two gates decide the `webcEnv` global, where the second one means
    /// `PROD`, so the weight a shipping client uses is index 2, and index 0 is
    /// unreachable from the definition at all.
    pub const RELEASE: usize = 2;

    /// Whether an event with weight `weight` survives sampling, given a uniform
    /// `roll` in `[0, 1)`.
    ///
    /// Weight 0 is *not* "never": the official client skips the sampling test
    /// entirely for it, so the event is always kept. Otherwise the event is
    /// dropped when `roll * weight > 1`, i.e. kept with probability `1/weight`,
    /// and weight 1 keeps everything.
    pub fn keeps(weight: u32, roll: f64) -> bool {
        weight == 0 || roll * f64::from(weight) <= 1.0
    }
}

/// Record kinds, in the low two bits of a record's flag byte.
const KIND_GLOBAL: u8 = 0;
const KIND_EVENT: u8 = 1;
const KIND_FIELD: u8 = 2;
/// Bit 2: this record ends the event's group.
const FLAG_LAST: u8 = 4;
/// Bit 3: the id that follows is 16 bits rather than 8.
const FLAG_WIDE_ID: u8 = 8;

/// Payload size classes, in the high nibble.
const CLASS_ZERO: u8 = 16;
const CLASS_ONE: u8 = 32;
const CLASS_I8: u8 = 48;
const CLASS_I16: u8 = 64;
const CLASS_I32: u8 = 80;
const CLASS_F64: u8 = 112;
const CLASS_STR8: u8 = 128;
const CLASS_STR16: u8 = 144;
const CLASS_STR32: u8 = 160;

/// The wire id of the per-event timestamp, in unix seconds.
///
/// Written as a global before every event. The whatspec IR does not publish it:
/// no `defineGlobal` declares it, because the runtime writes it directly, so
/// there is no upstream record for the extractor to read and this constant is
/// this repository's own reading of the writer.
/// The protocol version as the header byte carries it.
///
/// Narrowed once, in a const block, so a catalog that starts declaring a version
/// past a byte fails the build instead of stamping a truncated one on every
/// buffer this client sends.
const PROTOCOL_VERSION_BYTE: u8 = {
    assert!(
        constants::WAM_PROTOCOL_VERSION >= 0 && constants::WAM_PROTOCOL_VERSION <= u8::MAX as i64,
        "the WAM protocol version no longer fits the one byte the header gives it"
    );
    constants::WAM_PROTOCOL_VERSION as u8
};

pub const TIMESTAMP_FIELD: u32 = 47;

/// The wire id of the beaconing event sequence number.
///
/// Written as a global before an event only while the client is in the beaconing
/// cohort. Like [`TIMESTAMP_FIELD`] the IR does not publish it, for the same
/// reason: the runtime writes it and no `defineGlobal` declares it.
pub const SEQUENCE_FIELD: u32 = 3433;

/// The wire id of the `psId` global, which a `private` buffer rewrites before
/// every event. Declared by `defineGlobal`, so unlike the two above it is in the
/// catalog; named here because the buffer treats it specially.
const PS_ID_FIELD: u32 = 6005;

/// Errors a caller can make against a buffer. Every one is a programming
/// mistake, not a runtime condition: none can be caused by the network or by
/// what the peer sent.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BufferError {
    /// A global whose channel list does not contain the buffer's channel. WA
    /// Web's writer skips it, so a buffer carrying it is one no client sends.
    #[error("global `{name}` is not legal on the {channel:?} channel")]
    GlobalNotOnChannel {
        name: &'static str,
        channel: Channel,
    },
    /// An event whose declared channel is not the buffer's. Buffers are
    /// per-channel and the server reads the channel from the header.
    #[error("event `{event}` belongs to the {event_channel:?} channel, not {buffer_channel:?}")]
    EventOnWrongChannel {
        event: &'static str,
        event_channel: Channel,
        buffer_channel: Channel,
    },
    /// A global handed a value of a type the catalog does not declare for it.
    ///
    /// An event's fields are typed by its generated struct, so this cannot
    /// happen there. A global is the one untyped way into a buffer, and the
    /// declared kind is what the server reads the bytes back as: a string id
    /// carrying an integer is a record no client sends.
    #[error("global `{name}` is {expected:?}, not {actual:?}")]
    GlobalValueKind {
        name: &'static str,
        expected: ValueKind,
        actual: ValueKind,
    },
}

/// One WAM buffer, built record by record.
///
/// Holds the encoded bytes and the globals already written, so a global whose
/// value has not changed is not repeated, which is the only reason a buffer
/// carrying hundreds of events stays small.
#[derive(Debug)]
pub struct WamBuffer {
    bytes: Vec<u8>,
    channel: Channel,
    sequence: u16,
    /// Globals as last written, keyed by id. Ordered because the official
    /// writer iterates integer-like object keys in ascending numeric order, and
    /// the byte stream has to match.
    written: BTreeMap<u32, OwnedGlobalValue>,
    pending: BTreeMap<u32, OwnedGlobalValue>,
    events: usize,
}

impl WamBuffer {
    /// A new buffer for `channel`, stamped with `stream_id` and `sequence`.
    ///
    /// Writes only the 8-byte header: globals reach the bytes with the first
    /// event, so a buffer that never carries one is never worth uploading and
    /// [`has_events`](Self::has_events) says so.
    pub fn new(channel: Channel, stream_id: u8, sequence: u16) -> Self {
        let mut bytes = Vec::with_capacity(256);
        bytes.extend_from_slice(b"WAM");
        bytes.push(PROTOCOL_VERSION_BYTE);
        bytes.push(stream_id);
        bytes.extend_from_slice(&sequence.to_le_bytes());
        bytes.push(channel.wire());
        Self {
            bytes,
            channel,
            sequence,
            written: BTreeMap::new(),
            pending: BTreeMap::new(),
            events: 0,
        }
    }

    pub fn channel(&self) -> Channel {
        self.channel
    }

    pub fn sequence(&self) -> u16 {
        self.sequence
    }

    /// Bytes written so far, the measure every size threshold is expressed in.
    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Whether any event has been written. A buffer without one carries no
    /// information and is never uploaded.
    pub fn has_events(&self) -> bool {
        self.events > 0
    }

    pub fn events(&self) -> usize {
        self.events
    }

    /// The encoded buffer.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }

    /// Stage a global. It reaches the bytes before the next event, and only if
    /// its value differs from the one already written.
    ///
    /// Rejects a global the channel does not allow rather than skipping it the
    /// way the official writer does: skipping silently is how a caller ends up
    /// believing a value was sent.
    pub fn set_global(
        &mut self,
        def: GlobalDef,
        value: GlobalValue<'_>,
    ) -> Result<(), BufferError> {
        if !def.channels.allows(self.channel) {
            return Err(BufferError::GlobalNotOnChannel {
                name: def.name,
                channel: self.channel,
            });
        }
        if !value.fits(def.kind) {
            return Err(BufferError::GlobalValueKind {
                name: def.name,
                expected: def.kind,
                actual: value.kind(),
            });
        }
        self.pending.insert(def.id, value.to_owned_value());
        Ok(())
    }

    /// Stage a global as explicitly absent.
    pub fn clear_global(&mut self, def: GlobalDef) -> Result<(), BufferError> {
        if !def.channels.allows(self.channel) {
            return Err(BufferError::GlobalNotOnChannel {
                name: def.name,
                channel: self.channel,
            });
        }
        self.pending.insert(def.id, OwnedGlobalValue::Null);
        Ok(())
    }

    /// Write one event, stamped `timestamp_secs`, at `weight`.
    ///
    /// `weight` is written negated, as the official writer does. Staged globals
    /// go out first, then the event record, then its fields; the last record
    /// written carries the group-end flag whether that is the event record
    /// itself or its final field.
    pub fn write_event<E: WamEvent>(
        &mut self,
        event: &E,
        timestamp_secs: i64,
        weight: u32,
    ) -> Result<(), BufferError> {
        if E::CHANNEL != self.channel {
            return Err(BufferError::EventOnWrongChannel {
                event: E::NAME,
                event_channel: E::CHANNEL,
                buffer_channel: self.channel,
            });
        }
        self.pending
            .insert(TIMESTAMP_FIELD, OwnedGlobalValue::Int(timestamp_secs));
        self.flush_globals();

        // Negated, matching the writer: a positive weight in this slot is a
        // different number to the server, not a different spelling of the same
        // one.
        let at = self.write_number(E::CODE, KIND_EVENT, -f64::from(weight));
        let mut fields = EventFields {
            buffer: self,
            last_record_at: at,
        };
        event.encode(&mut fields);
        fields.finish();
        self.events += 1;
        Ok(())
    }

    /// Write every staged global whose value changed since it was last written.
    ///
    /// The timestamp and the private-stats id are exempt from the comparison:
    /// both are rewritten before every event even when unchanged, because they
    /// describe the event that follows rather than the buffer.
    fn flush_globals(&mut self) {
        let pending = std::mem::take(&mut self.pending);
        for (id, value) in pending {
            let always = id == TIMESTAMP_FIELD || id == PS_ID_FIELD;
            if !always && self.written.get(&id) == Some(&value) {
                continue;
            }
            match &value {
                OwnedGlobalValue::Int(i) => {
                    self.write_number(id, KIND_GLOBAL, *i as f64);
                }
                OwnedGlobalValue::Float(f) => {
                    if f.is_nan() {
                        // The official writer skips a NaN global rather than
                        // writing one, and a NaN would also make the
                        // already-written comparison never match.
                        continue;
                    }
                    self.write_number(id, KIND_GLOBAL, *f);
                }
                OwnedGlobalValue::Str(s) => {
                    self.write_string(id, KIND_GLOBAL, s);
                }
                OwnedGlobalValue::Null => {
                    self.write_descriptor(id, KIND_GLOBAL);
                }
            }
            self.written.insert(id, value);
        }
    }

    /// `flags | id`, widening the id when it does not fit a byte. Returns where
    /// the flag byte landed, so the group-end bit can be set on whichever record
    /// turns out to be the last one written.
    fn write_descriptor(&mut self, id: u32, flags: u8) -> usize {
        // The wide form is two bytes, so the format itself cannot carry a larger
        // id and a catalog declaring one would be unencodable rather than merely
        // unusual. The generator refuses to emit such an id, which is where this
        // is really enforced; the assertion is here so a hand-written id in a
        // test cannot pass silently.
        debug_assert!(
            id <= u32::from(u16::MAX),
            "field id {id} does not fit the buffer format's two-byte id"
        );
        let at = self.bytes.len();
        if id < 256 {
            self.bytes.push(flags);
            self.bytes.push(id as u8);
        } else {
            self.bytes.push(flags | FLAG_WIDE_ID);
            self.bytes.extend_from_slice(&(id as u16).to_le_bytes());
        }
        at
    }

    /// One numeric value, in the narrowest class that holds it.
    ///
    /// The class test is the writer's own: a number is written as an integer
    /// only when it survives a round trip through a 32-bit truncation, so a
    /// value outside `i32`, a large integer included, goes out as a float.
    /// Reproducing that is what keeps a large timestamp or byte count
    /// byte-identical to the official client's.
    fn write_number(&mut self, id: u32, flags: u8, n: f64) -> usize {
        let as_i32 = n as i32;
        if n.is_finite() && f64::from(as_i32) == n {
            return match as_i32 {
                0 => self.write_descriptor(id, flags | CLASS_ZERO),
                1 => self.write_descriptor(id, flags | CLASS_ONE),
                v if (-128..128).contains(&v) => {
                    let at = self.write_descriptor(id, flags | CLASS_I8);
                    self.bytes.push(v as i8 as u8);
                    at
                }
                v if (-32768..32768).contains(&v) => {
                    let at = self.write_descriptor(id, flags | CLASS_I16);
                    self.bytes.extend_from_slice(&(v as i16).to_le_bytes());
                    at
                }
                v => {
                    let at = self.write_descriptor(id, flags | CLASS_I32);
                    self.bytes.extend_from_slice(&v.to_le_bytes());
                    at
                }
            };
        }
        let at = self.write_descriptor(id, flags | CLASS_F64);
        self.bytes.extend_from_slice(&n.to_le_bytes());
        at
    }

    fn write_string(&mut self, id: u32, flags: u8, s: &str) -> usize {
        let len = s.len();
        let at = if len < 256 {
            let at = self.write_descriptor(id, flags | CLASS_STR8);
            self.bytes.push(len as u8);
            at
        } else if len < 65536 {
            let at = self.write_descriptor(id, flags | CLASS_STR16);
            self.bytes.extend_from_slice(&(len as u16).to_le_bytes());
            at
        } else {
            let at = self.write_descriptor(id, flags | CLASS_STR32);
            self.bytes.extend_from_slice(&(len as u32).to_le_bytes());
            at
        };
        self.bytes.extend_from_slice(s.as_bytes());
        at
    }
}

/// The sink a generated event writes its fields into.
///
/// Records go out as they arrive; what is deferred is only the group-end flag,
/// which is set on the last record written once the event is done. Remembering
/// an offset rather than holding a record back is what lets an event's `encode`
/// push its fields in catalog order, skip the absent ones, and never know which
/// one is final, with no borrow of the event outliving the call.
pub struct EventFields<'buf> {
    buffer: &'buf mut WamBuffer,
    /// Where the last record's flag byte is. Always set: the event record is
    /// written before any field, so there is one from the start.
    last_record_at: usize,
}

impl EventFields<'_> {
    fn finish(&mut self) {
        self.buffer.bytes[self.last_record_at] |= FLAG_LAST;
    }

    /// An integer, timer or enum field. Absent values are not written.
    pub fn integer(&mut self, id: u32, value: Option<i64>) {
        if let Some(v) = value {
            self.last_record_at = self.buffer.write_number(id, KIND_FIELD, v as f64);
        }
    }

    /// A boolean field. Reaches the wire as the integer 0 or 1.
    pub fn boolean(&mut self, id: u32, value: Option<bool>) {
        if let Some(v) = value {
            self.last_record_at = self
                .buffer
                .write_number(id, KIND_FIELD, f64::from(u8::from(v)));
        }
    }

    /// A floating-point field. A NaN is dropped rather than written: the
    /// official client's own pre-serialization check refuses a buffer holding
    /// one, so writing it would produce a buffer it would have discarded.
    pub fn number(&mut self, id: u32, value: Option<f64>) {
        if let Some(v) = value
            && !v.is_nan()
        {
            self.last_record_at = self.buffer.write_number(id, KIND_FIELD, v);
        }
    }

    /// A string field.
    pub fn string(&mut self, id: u32, value: Option<&str>) {
        if let Some(v) = value {
            self.last_record_at = self.buffer.write_string(id, KIND_FIELD, v);
        }
    }
}

/// One field a call site writes.
#[cfg(feature = "parity")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallSiteField {
    pub name: &'static str,
    pub write: FieldWrite,
}

/// How a call site writes a field: as a key of the constructor's argument
/// object, which runs whenever the site runs, or as a later assignment, which
/// WA Web often makes under a condition this evidence does not carry.
#[cfg(feature = "parity")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldWrite {
    Constructor,
    Assigned,
}

/// One place WA Web constructs an event.
#[cfg(feature = "parity")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CallSite {
    pub module: &'static str,
    /// `true` when the site also writes fields the extractor could not read, so
    /// `fields` is a lower bound on what it writes.
    pub partial: bool,
    pub fields: &'static [CallSiteField],
}

/// One field the catalog declares for an event: its bundle name and its wire id.
#[cfg(feature = "parity")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CatalogField {
    pub name: &'static str,
    pub id: u32,
}

/// One event's declared fields and the call sites recovered for it.
#[cfg(feature = "parity")]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventCallSites {
    pub event: &'static str,
    pub code: u32,
    /// Every field the catalog declares, so a checker can name the ids it reads
    /// back out of an encoded buffer.
    pub fields: &'static [CatalogField],
    pub sites: &'static [CallSite],
}

#[cfg(feature = "parity")]
impl EventCallSites {
    /// The union of every site's fields: the set a client may write without
    /// claiming a field the official client never writes.
    pub fn union(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.sites
            .iter()
            .flat_map(|s| s.fields.iter().map(|f| f.name))
    }

    /// Whether any site writes fields the extractor could not read. With none,
    /// the union is the whole truth about this event; with one, it is a lower
    /// bound.
    pub fn any_partial(&self) -> bool {
        self.sites.iter().any(|s| s.partial)
    }

    /// The declared name of the field with this wire id.
    pub fn field_name(&self, id: u32) -> Option<&'static str> {
        self.fields.iter().find(|f| f.id == id).map(|f| f.name)
    }
}

/// Look one event's call sites up by name.
#[cfg(feature = "parity")]
pub fn call_sites_for(event: &str) -> Option<&'static EventCallSites> {
    call_sites::CALL_SITES
        .binary_search_by(|e| e.event.cmp(event))
        .ok()
        .map(|i| &call_sites::CALL_SITES[i])
}

#[cfg(test)]
mod tests;

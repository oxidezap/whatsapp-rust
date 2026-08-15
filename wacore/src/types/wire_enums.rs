//! Auto-generated protocol enums (WhatsApp 2.3000.1044659339). DO NOT EDIT.
//!
//! Protocol enums generated from the whatspec enum catalog.
//!
//! Regenerate with `cargo run -p whatspec-codegen`; never edit by hand. To bind
//! another catalog entry, add it to `WANTED` in the codegen's enum emitter --
//! the variants come from the bundle, not from this file.

#![allow(clippy::all)]

/// The `type` attribute of an incoming `<message>` envelope.
///
/// The official parser rejects a stanza whose `type` is absent or
/// outside this set (`unknownValue: "reject"`). This client keeps
/// the stanza instead, so the fallback arm holds the exact wire
/// bytes of a value it does not model.
///
/// Generated from `STANZA_MSG_TYPES` in `WAWebHandleMsgCommon`.
#[derive(Debug, Clone, PartialEq, Eq, crate::WireEnum)]
pub enum StanzaMessageType {
    #[wire = "text"]
    Text,
    #[wire = "media"]
    Media,
    #[wire = "medianotify"]
    MediaNotify,
    #[wire = "pay"]
    Pay,
    #[wire = "poll"]
    Poll,
    #[wire = "reaction"]
    Reaction,
    #[wire = "event"]
    Event,
    /// A value this build does not model, kept verbatim.
    #[wire_fallback]
    Unknown(String),
}

/// The `polltype` attribute of an incoming `<message><meta>` node.
///
/// Closed on purpose: the attribute is `attrEnumOrNullIfUnknown`
/// upstream (`unknownValue: "null"`), so a value outside this set
/// is dropped rather than preserved.
///
/// Generated from `POLL_TYPES` in `WAWebHandleMsgCommon`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, crate::WireEnum)]
pub enum PollType {
    #[wire = "creation"]
    Creation,
    #[wire = "quiz_creation"]
    QuizCreation,
    #[wire = "vote"]
    Vote,
    #[wire = "result_snapshot"]
    ResultSnapshot,
    #[wire = "edit"]
    Edit,
}

/// The `mediatype` attribute of an `<enc>` node.
///
/// A hint about the payload the ciphertext carries, available
/// before the decryption that would reveal it. It is the sender's
/// claim and nothing checks it against the decrypted message.
///
/// Generated from `EncMediaType` in `WAWebBackendJobs.flow`.
#[derive(Debug, Clone, PartialEq, Eq, crate::WireEnum)]
pub enum EncMediaType {
    #[wire = "image"]
    Image,
    #[wire = "video"]
    Video,
    #[wire = "ptv"]
    Ptv,
    #[wire = "audio"]
    Audio,
    #[wire = "ptt"]
    Ptt,
    #[wire = "location"]
    Location,
    #[wire = "vcard"]
    Vcard,
    #[wire = "document"]
    Document,
    #[wire = "url"]
    Url,
    #[wire = "call"]
    Call,
    #[wire = "gif"]
    Gif,
    #[wire = "future"]
    Future,
    #[wire = "contact_array"]
    ContactArray,
    #[wire = "livelocation"]
    LiveLocation,
    #[wire = "profile_pic"]
    ProfilePic,
    #[wire = "sticker"]
    Sticker,
    #[wire = "sticker_pack"]
    StickerPack,
    #[wire = "hsm"]
    Hsm,
    #[wire = "product_image"]
    ProductImage,
    #[wire = "template"]
    Template,
    #[wire = "md_app_state"]
    MdAppState,
    #[wire = "md_history_sync"]
    MdHistorySync,
    #[wire = "list"]
    List,
    #[wire = "list_response"]
    ListResponse,
    #[wire = "button"]
    Button,
    #[wire = "button_response"]
    ButtonResponse,
    #[wire = "order"]
    Order,
    #[wire = "product"]
    Product,
    #[wire = "native_flow_response"]
    NativeFlowResponse,
    #[wire = "group_history"]
    GroupHistory,
    /// A value this build does not model, kept verbatim.
    #[wire_fallback]
    Unknown(String),
}

// Bits of a receipt's `<meta mode>` bitmask. `ReceiptModeBitPosition` in `WAWebSendReceiptJobCommon`. The catalog stores bit positions; these are already shifted.
/// `ORPHAN` of `ReceiptModeBitPosition`.
pub const RECEIPT_MODE_ORPHAN: u32 = 1 << 0;
/// `NO_CHECKMARK_UX` of `ReceiptModeBitPosition`.
pub const RECEIPT_MODE_NO_CHECKMARK_UX: u32 = 1 << 1;
/// `HID_FAILED_DECRYPT` of `ReceiptModeBitPosition`.
pub const RECEIPT_MODE_HID_FAILED_DECRYPT: u32 = 1 << 2;

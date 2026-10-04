//! Each removed spelling has its own negative control. The same imports and typed
//! arguments compile in `Host::actions`. Nightly rustdoc verifies E0599; stable
//! runs these as compile-fail tests but ignores error-code annotations.
//!
//! ```compile_fail,E0599
//! use wa::{Client, MessageRef};
//! async fn old(c: &Client, t: &MessageRef<'_>) { let _ = c.send_reaction_ref(t, "👍").await; }
//! ```
//!
//! ```compile_fail,E0599
//! use wa::{Client, MessageRef};
//! async fn old(c: &Client, t: &MessageRef<'_>) { let _ = c.keep_message_ref(t, true).await; }
//! ```
//!
//! ```compile_fail,E0599
//! use wa::{Client, MessageRef, PinDuration};
//! async fn old(c: &Client, t: &MessageRef<'_>) { let _ = c.pin_message_ref(t, PinDuration::Days7).await; }
//! ```
//!
//! ```compile_fail,E0599
//! use wa::{Client, MessageRef};
//! async fn old(c: &Client, t: &MessageRef<'_>) { let _ = c.unpin_message_ref(t).await; }
//! ```
//!
//! ```compile_fail,E0599
//! use wa::{Client, MessageRef};
//! async fn old(c: &Client, t: &MessageRef<'_>) { let _ = c.revoke_message_ref(t).await; }
//! ```
//!
//! ```compile_fail,E0599
//! use wa::{Client, NewsletterMessageRef};
//! async fn old(c: &Client, t: &NewsletterMessageRef<'_>) { let _ = c.newsletter().send_reaction_ref(t, "👍").await; }
//! ```
//!
//! ```compile_fail,E0599
//! use wa::{Client, NewsletterMessageRef};
//! async fn old(c: &Client, t: &NewsletterMessageRef<'_>) { let _ = c.newsletter().send_poll_vote_ref(t, &[]).await; }
//! ```
//!
//! Old main-name raw arities are not aliases either; use the explicitly named raw
//! methods compiled in `raw` instead.
//!
//! ```compile_fail,E0061
//! use wa::{Client, Jid, waproto::whatsapp::MessageKey};
//! async fn old(c: &Client, chat: &Jid) { let _ = c.send_reaction(chat, MessageKey::default(), "👍").await; }
//! ```
//!
//! ```compile_fail,E0061
//! use wa::{Client, Jid, waproto::whatsapp::MessageKey};
//! async fn old(c: &Client, chat: &Jid) { let _ = c.keep_message(chat, MessageKey::default(), true).await; }
//! ```
//!
//! ```compile_fail,E0061
//! use wa::{Client, Jid, PinDuration, waproto::whatsapp::MessageKey};
//! async fn old(c: &Client, chat: &Jid) { let _ = c.pin_message(chat, MessageKey::default(), PinDuration::Days7).await; }
//! ```
//!
//! ```compile_fail,E0061
//! use wa::{Client, Jid, waproto::whatsapp::MessageKey};
//! async fn old(c: &Client, chat: &Jid) { let _ = c.unpin_message(chat, MessageKey::default()).await; }
//! ```
//!
//! ```compile_fail,E0061
//! use wa::{Client, Jid, RevokeType};
//! async fn old(c: &Client, chat: &Jid) { let _ = c.revoke_message(chat, "CONTENT", RevokeType::Sender).await; }
//! ```
//!
//! ```compile_fail,E0061
//! use wa::bot::MessageContext;
//! use wa::RevokeType;
//! async fn old(c: &MessageContext) { let _ = c.revoke_message("CONTENT", RevokeType::Sender).await; }
//! ```
//!
//! ```compile_fail,E0061
//! use wa::{Client, Jid};
//! async fn old(c: &Client, chat: &Jid) { let _ = c.newsletter().send_reaction(chat, 1, "👍").await; }
//! ```
//!
//! ```compile_fail,E0061
//! use wa::{Client, Jid};
//! async fn old(c: &Client, chat: &Jid) { let _ = c.newsletter().send_poll_vote(chat, 1, &[]).await; }
//! ```
//!
//! ```compile_fail,E0061
//! use wa::{Client, Jid, waproto::whatsapp::MessageKey};
//! async fn old(c: &Client, chat: &Jid) { let _ = c.comments().send_text(chat, MessageKey::default(), "comment").await; }
//! ```
//!
//! ```compile_fail,E0061
//! use wa::{Client, Jid, waproto::whatsapp::{Message, MessageKey}};
//! async fn old(c: &Client, chat: &Jid) { let _ = c.comments().send_message(chat, MessageKey::default(), Message::default()).await; }
//! ```

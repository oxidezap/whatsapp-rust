//! Each removed spelling has its own negative control. The same imports and typed
//! arguments compile in `Host::actions`. Nightly rustdoc verifies E0599; stable
//! runs these as compile-fail tests but ignores error-code annotations.
//!
//! Ranges accept typed references; the free raw-key factory is removed from
//! both public export paths. Context and send-result methods remain available.
//!
//! ```compile_fail,E0432
//! use whatsapp_rust::message_key;
//! ```
//!
//! ```compile_fail,E0432
//! use whatsapp_rust::features::message_key;
//! ```
//!
//! ```compile_fail,E0271
//! use whatsapp_rust::{message_range, waproto::whatsapp::MessageKey};
//! let _ = message_range(1, None, vec![(MessageKey::default(), 1)]);
//! ```
//!
//! ```compile_fail,E0599
//! use whatsapp_rust::{Client, MessageRef};
//! async fn old(c: &Client, t: &MessageRef<'_>) { let _ = c.send_reaction_ref(t, "👍").await; }
//! ```
//!
//! ```compile_fail,E0599
//! use whatsapp_rust::{Client, MessageRef};
//! async fn old(c: &Client, t: &MessageRef<'_>) { let _ = c.keep_message_ref(t, true).await; }
//! ```
//!
//! ```compile_fail,E0599
//! use whatsapp_rust::{Client, MessageRef, PinDuration};
//! async fn old(c: &Client, t: &MessageRef<'_>) { let _ = c.pin_message_ref(t, PinDuration::Days7).await; }
//! ```
//!
//! ```compile_fail,E0599
//! use whatsapp_rust::{Client, MessageRef};
//! async fn old(c: &Client, t: &MessageRef<'_>) { let _ = c.unpin_message_ref(t).await; }
//! ```
//!
//! ```compile_fail,E0599
//! use whatsapp_rust::{Client, MessageRef};
//! async fn old(c: &Client, t: &MessageRef<'_>) { let _ = c.revoke_message_ref(t).await; }
//! ```
//!
//! ```compile_fail,E0599
//! use whatsapp_rust::{Client, NewsletterMessageRef};
//! async fn old(c: &Client, t: &NewsletterMessageRef<'_>) { let _ = c.newsletter().send_reaction_ref(t, "👍").await; }
//! ```
//!
//! ```compile_fail,E0599
//! use whatsapp_rust::{Client, NewsletterMessageRef};
//! async fn old(c: &Client, t: &NewsletterMessageRef<'_>) { let _ = c.newsletter().send_poll_vote_ref(t, &[]).await; }
//! ```
//!
//! Old main-name raw arities are not aliases either; use the explicitly named raw
//! methods compiled in `raw` instead.
//!
//! ```compile_fail,E0061
//! use whatsapp_rust::{Client, Jid, waproto::whatsapp::MessageKey};
//! async fn old(c: &Client, chat: &Jid) { let _ = c.send_reaction(chat, MessageKey::default(), "👍").await; }
//! ```
//!
//! ```compile_fail,E0061
//! use whatsapp_rust::{Client, Jid, waproto::whatsapp::MessageKey};
//! async fn old(c: &Client, chat: &Jid) { let _ = c.keep_message(chat, MessageKey::default(), true).await; }
//! ```
//!
//! ```compile_fail,E0061
//! use whatsapp_rust::{Client, Jid, PinDuration, waproto::whatsapp::MessageKey};
//! async fn old(c: &Client, chat: &Jid) { let _ = c.pin_message(chat, MessageKey::default(), PinDuration::Days7).await; }
//! ```
//!
//! ```compile_fail,E0061
//! use whatsapp_rust::{Client, Jid, waproto::whatsapp::MessageKey};
//! async fn old(c: &Client, chat: &Jid) { let _ = c.unpin_message(chat, MessageKey::default()).await; }
//! ```
//!
//! ```compile_fail,E0061
//! use whatsapp_rust::{Client, Jid, RevokeType};
//! async fn old(c: &Client, chat: &Jid) { let _ = c.revoke_message(chat, "CONTENT", RevokeType::Sender).await; }
//! ```
//!
//! ```compile_fail,E0061
//! use whatsapp_rust::bot::MessageContext;
//! use whatsapp_rust::RevokeType;
//! async fn old(c: &MessageContext) { let _ = c.revoke_message("CONTENT", RevokeType::Sender).await; }
//! ```
//!
//! ```compile_fail,E0061
//! use whatsapp_rust::{Client, Jid};
//! async fn old(c: &Client, chat: &Jid) { let _ = c.newsletter().send_reaction(chat, 1, "👍").await; }
//! ```
//!
//! ```compile_fail,E0061
//! use whatsapp_rust::{Client, Jid};
//! async fn old(c: &Client, chat: &Jid) { let _ = c.newsletter().send_poll_vote(chat, 1, &[]).await; }
//! ```
//!
//! ```compile_fail,E0061
//! use whatsapp_rust::{Client, Jid, waproto::whatsapp::MessageKey};
//! async fn old(c: &Client, chat: &Jid) { let _ = c.comments().send_text(chat, MessageKey::default(), "comment").await; }
//! ```
//!
//! ```compile_fail,E0061
//! use whatsapp_rust::{Client, Jid, waproto::whatsapp::{Message, MessageKey}};
//! async fn old(c: &Client, chat: &Jid) { let _ = c.comments().send_message(chat, MessageKey::default(), Message::default()).await; }
//! ```

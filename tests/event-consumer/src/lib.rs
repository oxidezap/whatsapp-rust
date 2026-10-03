//! External host contract and intentional removed-API controls.
//!
//! The old unlimited policy name is no longer available:
//! ```compile_fail,E0599
//! use whatsapp_rust::EventDelivery;
//! let _ = EventDelivery::Concurrent;
//! ```
//!
//! Permanent chatstate registration must migrate to an owned Subscription:
//! ```compile_fail,E0599
//! use std::sync::Arc;
//! use whatsapp_rust::Client;
//! fn old_registration(client: &Arc<Client>) {
//!     client.register_chatstate_handler(Arc::new(|_| {}));
//! }
//! ```

#[cfg(all(test, feature = "native-tests"))]
#[path = "../../event_delivery_public.rs"]
mod contracts;

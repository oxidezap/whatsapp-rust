//! External removal control for the superseded Contacts lookup.
//!
//! The imports and request construction are also exercised by this fixture's
//! positive binary. Only the old method name is expected to fail (E0599):
//!
//! ```compile_fail,E0599
//! use whatsapp_rust::{Client, ContactError, ProfilePictureLookup,
//!     ProfilePictureRequest, ProfilePictureTarget, ProfilePictureType};
//! use whatsapp_rust::wacore_binary::Jid;
//! async fn old_lookup(client: &Client, jid: &Jid) -> Result<ProfilePictureLookup, ContactError> {
//!     client.contacts().lookup_picture(ProfilePictureRequest::new(
//!         ProfilePictureTarget::Contact(jid), ProfilePictureType::Full,
//!     )).await
//! }
//! ```

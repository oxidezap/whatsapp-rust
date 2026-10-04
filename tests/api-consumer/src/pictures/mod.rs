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

use whatsapp_rust::{AppStateError, Client, ProfileError, PushNameOutcome};

/// Consumers distinguish cross-device sync from the already-sent presence.
/// The original error remains typed and can be inspected without parsing logs.
pub async fn update_push_name(
    client: &Client,
    name: &str,
) -> Result<PushNameOutcome, ProfileError> {
    client.profile().set_push_name(name).await
}

pub async fn retry_push_name_sync(client: &Client, name: &str) -> Result<(), AppStateError> {
    client.profile().sync_push_name(name).await
}

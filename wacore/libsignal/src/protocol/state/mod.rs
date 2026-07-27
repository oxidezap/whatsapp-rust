// Copyright 2020 Signal Messenger, LLC.
// SPDX-License-Identifier: AGPL-3.0-only

mod bundle;
mod prekey;
mod session;
mod signed_prekey;

pub use bundle::{PreKeyBundle, PreKeyBundleContent};
pub use prekey::{PreKeyId, PreKeyRecord};
pub(crate) use session::{DecryptSnapshot, ReceiverChainState};
pub use session::{InvalidSessionError, SessionRecord, SessionState};
pub use signed_prekey::{GenericSignedPreKey, SignedPreKeyId, SignedPreKeyRecord};

use crate::core::curve::PublicKey;

/// Rewrite a decoded record's `public_key` field to the raw 32-byte form.
///
/// Reading tolerates the 33-byte tagged key that pre-0.7 record constructors
/// wrote, but tolerance alone would leave a downstream store mixed forever: its
/// rows only pass back through `deserialize` and `serialize`, never through the
/// `record_helpers` bridges that rebuild a record from parsed keys. Normalizing
/// at the decode boundary makes "the structure holds the raw key" an invariant
/// of the in-memory record, so re-serializing heals the row.
///
/// A field that is absent, or the wrong length, or not a valid tagged key, is
/// left untouched — turning a `deserialize` that used to succeed into one that
/// fails is not this function's call to make. The getters still reject it.
pub(crate) fn normalize_stored_public_key(field: &mut Option<Vec<u8>>) {
    if let Some(bytes) = field.as_mut()
        && bytes.len() == PublicKey::SERIALIZED_KEY_LEN
        && let Ok(key) = PublicKey::deserialize(bytes)
    {
        *bytes = key.public_key_bytes().to_vec();
    }
}

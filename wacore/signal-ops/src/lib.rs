// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

//! Signal operations for a WhatsApp linked device whose private keys live
//! outside the WhatsApp client.
//!
//! The client (connection, receipts, retries, prekey upload, device lists)
//! runs elsewhere and calls these operations whenever it needs a private key:
//! decrypting and encrypting messages, sender keys, new prekeys, and the one
//! pairing signature. Everything here is pure Signal state over a
//! [`wacore::store::traits::SignalStore`]; nothing opens a socket.
//!
//! # Trust boundary
//!
//! Private keys, plaintext and Signal state stay with the caller of these
//! operations. The client never holds them, and a client that only relays
//! ciphertext learns nothing more than it would as a transport.
//!
//! The client is still trusted for who a message goes to. Device lists,
//! prekey bundles, PN/LID mappings and group membership all come from the
//! [`wacore::client::context::SendContextResolver`] or from the retry
//! receipt, and identity keys are accepted on first use or change, as WA
//! Web does. These operations do not verify a bundle against the peer
//! account's signed device identity. A client that lies about devices or
//! keys can therefore have new messages, and retries of messages still kept
//! by [`SignalOps::sent_message`], encrypted to keys it controls. Prune sent
//! messages ([`SignalOps::prune_sent_messages`]) to bound what a retry can
//! reach.
//!
//! Message padding is the caller's concern: [`SignalOps::decrypt`] returns
//! libsignal's plaintext exactly as received and [`SignalOps::encrypt`] takes
//! already padded plaintext.
//!
//! Use [`SignalOps::prepare_prekey_upload`] for ordinary public-key uploads:
//! its sealed pending batch survives reopening, and retries do not allocate
//! more private keys. Acknowledge a successful server upload with
//! [`SignalOps::acknowledge_prekey_upload`]; the last exact acknowledgment is
//! replayable after a lost response. [`SignalOps::generate_prekeys`] remains
//! fresh generation for retry receipts; [`SignalOps::mark_prekeys_uploaded`]
//! marks only those fresh keys and rejects pending upload members atomically.
//! These operations use the scope's fenced store;
//! upload batch IDs remain reserved across 24-bit wrap even after consumption.
//!
//! Dropping a state-changing operation before completion retires that
//! `SignalOps` instance before its locks are released. Submitted backend writes
//! may still finish, so callers must release its lease and open a new instance
//! with a higher fence, not reopen at the same generation. [`SignalOps::is_usable`]
//! exposes this state; later mutations report [`OpsError::is_fence_lost`].

mod device;
mod error;
mod guard;
mod inbound;
mod ops;
mod outbound;
mod stores;
mod trust;

pub use device::{DeviceKeys, PublicIdentity, PublicPreKey, PublicSignedPreKey};
pub use error::OpsError;
pub use inbound::{ReceiveKind, ReceiveRequest, Received, ReceivedMessage};
pub use ops::{
    AuthorityKey, Decrypted, EncKind, Encrypted, PairingSignature, PreKeyUploadBatch, RemoteBundle,
    ServiceStore, SignalOps,
};
pub use outbound::{PreparedGroupSend, PreparedSend, RetryRequest, RetryRoute};
pub use trust::{BundleRejection, FetchedBundle, SignalResolver, VerifiablePrekeys};

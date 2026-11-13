// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

use crate::client::context::SendContextResolver;
use crate::libsignal::protocol::PreKeyBundle;
use crate::prekeys::RejectedDevice;
use async_trait::async_trait;
use std::collections::HashMap;
use wacore_binary::Jid;

/// A fetched bundle and the `<device-identity>` the server sent with it.
#[derive(Debug, Clone)]
pub struct FetchedBundle {
    pub bundle: PreKeyBundle,
    /// The `ADVSignedDeviceIdentity` bytes. Required for companion devices;
    /// primary devices carry none.
    pub device_identity: Option<Vec<u8>>,
}

/// What one verifiable prekey fetch produced.
#[derive(Debug, Clone, Default)]
pub struct VerifiablePrekeys {
    pub bundles: HashMap<Jid, FetchedBundle>,
    /// Devices the server rejected by name, as in [`crate::prekeys::PreKeyFetchOutcome`].
    pub rejected: Vec<RejectedDevice>,
}

/// The client side of a send: wacore's [`SendContextResolver`], plus prekey
/// fetches that keep each companion's `<device-identity>`. The service uses
/// only these fetches to set up sessions and verifies every bundle itself;
/// the resolver's own `fetch_prekeys` is not called.
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait SignalResolver: SendContextResolver {
    async fn fetch_verifiable_prekeys(&self, jids: &[Jid]) -> anyhow::Result<VerifiablePrekeys>;
}

//! Per-device Signal encryption fanout and the bounded spawn helper.

use super::*;

/// Caller must hold `SenderKeyStore::sender_key_lock` for `sender_key_name`
/// across the surrounding SKDM creation + this encrypt, so a concurrent send
/// can't split the key between the SKDM and the skmsg.
#[cfg_attr(
    feature = "tracing",
    tracing::instrument(name = "wa.send.encrypt_group", level = "debug", skip_all, err(Debug))
)]
pub async fn encrypt_group_message<S, R>(
    sender_key_store: &mut S,
    sender_key_name: &SenderKeyName,
    plaintext: &[u8],
    csprng: &mut R,
) -> Result<SenderKeyMessage>
where
    S: SenderKeyStore + ?Sized,
    R: Rng + CryptoRng,
{
    log::debug!(
        "Attempting to load sender key for group {} sender {}",
        sender_key_name.group_id(),
        sender_key_name.sender_id()
    );

    let mut record = sender_key_store
        .load_sender_key(sender_key_name)
        .await?
        .ok_or_else(|| {
            SignalProtocolError::NoSenderKeyState(format!(
                "no sender key record for group {} sender {}",
                sender_key_name.group_id(),
                sender_key_name.sender_id()
            ))
        })?;

    let sender_key_state = record
        .sender_key_state_mut()
        .map_err(|e| anyhow!("Invalid SenderKey session: {:?}", e))?;

    let sender_chain_key = sender_key_state
        .sender_chain_key()
        .ok_or_else(|| anyhow!("Invalid SenderKey session: missing chain key"))?;

    let message_keys = sender_chain_key.sender_message_key();

    let mut ciphertext = Vec::new();
    aes_256_cbc_encrypt_into(
        plaintext,
        message_keys.cipher_key(),
        message_keys.iv(),
        &mut ciphertext,
    )
    .map_err(|_| anyhow!("AES encryption failed"))?;

    let signing_key = sender_key_state
        .signing_key_private()
        .map_err(|e| anyhow!("Invalid SenderKey session: missing signing key: {:?}", e))?;

    let skm = SenderKeyMessage::new(
        SENDERKEY_MESSAGE_CURRENT_VERSION,
        sender_key_state.chain_id(),
        message_keys.iteration(),
        ciphertext.into_boxed_slice(),
        csprng,
        &signing_key,
    )?;

    sender_key_state.set_sender_chain_key(sender_chain_key.next()?);

    sender_key_store
        .store_sender_key(sender_key_name, record)
        .await?;

    Ok(skm)
}

/// Object-safe `SessionStore` that can clone itself into an owned box. The
/// encrypt fan-out hands each spawned task its own owned store handle; erasing
/// the concrete type here (instead of a generic `S: Clone`) keeps the whole
/// encrypt tree as a single instantiation instead of one ~60 KiB copy per
/// concrete adapter set. Blanket-impl'd for every `Clone` store, so concrete
/// adapters need no changes.
pub trait CloneableSessionStore: crate::libsignal::protocol::SessionStore {
    fn clone_box(&self) -> Box<dyn CloneableSessionStore + Send + Sync>;
}

impl<T> CloneableSessionStore for T
where
    T: crate::libsignal::protocol::SessionStore + Clone + Send + Sync + 'static,
{
    fn clone_box(&self) -> Box<dyn CloneableSessionStore + Send + Sync> {
        Box::new(self.clone())
    }
}

/// See [`CloneableSessionStore`].
pub trait CloneableIdentityStore: crate::libsignal::protocol::IdentityKeyStore {
    fn clone_box(&self) -> Box<dyn CloneableIdentityStore + Send + Sync>;
}

impl<T> CloneableIdentityStore for T
where
    T: crate::libsignal::protocol::IdentityKeyStore + Clone + Send + Sync + 'static,
{
    fn clone_box(&self) -> Box<dyn CloneableIdentityStore + Send + Sync> {
        Box::new(self.clone())
    }
}

/// Borrowed handles to the Signal stores for one send. The stores are
/// type-erased (`dyn`) so the encrypt/session functions compile to a single
/// instantiation rather than one per concrete adapter set.
pub struct SignalStores<'a> {
    pub sender_key_store: &'a mut (dyn crate::libsignal::protocol::SenderKeyStore + Send + Sync),
    pub session_store: &'a mut (dyn CloneableSessionStore + Send + Sync),
    pub identity_store: &'a mut (dyn CloneableIdentityStore + Send + Sync),
    pub prekey_store: &'a mut (dyn crate::libsignal::protocol::PreKeyStore + Send + Sync),
    pub signed_prekey_store: &'a (dyn crate::libsignal::protocol::SignedPreKeyStore + Send + Sync),
}

/// Check if an anyhow error is a 406 "not-acceptable" server error (device unregistered).
/// Uses typed downcast to `ServerErrorCode` — the shared error type that the
/// `SendContextResolver` impl wraps server errors in.
pub(crate) fn is_device_unregistered_error(err: &anyhow::Error) -> bool {
    crate::request::ServerErrorCode::from_anyhow(err).is_some_and(|e| e.code == 406)
}

pub struct EncryptResult {
    pub participant_nodes: Vec<Node>,
    pub includes_prekey_message: bool,
    pub encrypted_devices: Vec<Jid>,
    /// True if any device returned 406 (unregistered) during prekey fetch.
    pub had_unregistered_device: bool,
}

/// One device's encrypted ciphertext, node-agnostic. The DM/peer paths map this
/// into a `<to><enc>` node; the voip offer maps it into an `<enc>` per device.
pub struct EncryptedDevice {
    pub device_jid: Jid,
    /// `pkmsg` or `msg`.
    pub enc_type: &'static str,
    pub is_prekey: bool,
    pub ciphertext: Vec<u8>,
}

/// Node-agnostic encrypt fan-out result: the per-device ciphertexts plus the
/// two batch flags the message path threads into its stanza.
pub struct EncryptForDevicesRaw {
    pub devices: Vec<EncryptedDevice>,
    pub includes_prekey_message: bool,
    /// True if any device returned 406 (unregistered) during prekey fetch.
    pub had_unregistered_device: bool,
}

/// Resolve the `<device-identity>` blob a stanza must carry. A pkmsg recipient
/// validates our identity from it; without it a pkmsg advances the sender chain
/// while the peer can't consume the pre-key message (the linked-device deadlock).
///
/// - `includes_prekey && account.is_some()` -> `Ok(Some(encoded))`
/// - `includes_prekey && account.is_none()` -> `Err` (refuse before send)
/// - `!includes_prekey` -> `Ok(None)`
pub fn needs_device_identity(
    includes_prekey: bool,
    account: Option<&wa::AdvSignedDeviceIdentity>,
) -> Result<Option<Vec<u8>>> {
    if !includes_prekey {
        return Ok(None);
    }
    let acc = account
        .ok_or_else(|| anyhow!("pkmsg requires <device-identity> but no ADV account is present"))?;
    Ok(Some(acc.encode_to_vec()))
}

/// Maximum number of concurrent per-device crypto tasks during group send
/// fan-out. Picked from the `perf-audit` benchmark: speedup plateaus around
/// 16 on Oracle ARM64; 32 gives only ~10% more for double the task overhead.
const ENCRYPT_FANOUT_CONCURRENCY: usize = 16;

/// Per-task encrypt result, shipped from a spawned task back to the orchestrator.
struct EncryptOneResult {
    enc_type: &'static str,
    is_prekey: bool,
    ciphertext: Vec<u8>,
}

/// Surfaces a spawned task that didn't deliver its result — either the task
/// itself panicked or the runtime tore it down (e.g., during shutdown).
/// Surfacing this as an Err lets the encrypt fan-out fall through to its
/// existing log+skip path instead of propagating a panic.
#[derive(Debug, thiserror::Error)]
#[error("spawned task did not produce a result (panic or runtime shutdown)")]
struct SpawnCanceled;

/// Future returned by [`spawn_oneshot`]. Holds the spawned task's
/// [`AbortHandle`] until the result is received, so dropping the future mid-
/// flight (e.g., the outer send was cancelled by a timeout) cancels the
/// in-flight crypto work instead of orphaning it.
struct Spawned<T> {
    rx: futures::channel::oneshot::Receiver<T>,
    abort: Option<AbortHandle>,
}

impl<T> Future for Spawned<T> {
    type Output = std::result::Result<T, SpawnCanceled>;

    fn poll(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        match std::pin::Pin::new(&mut self.rx).poll(cx) {
            std::task::Poll::Ready(Ok(value)) => {
                // Result delivered: disarm so Drop doesn't try to abort an
                // already-completed task.
                if let Some(handle) = self.abort.take() {
                    handle.detach();
                }
                std::task::Poll::Ready(Ok(value))
            }
            std::task::Poll::Ready(Err(_)) => {
                if let Some(handle) = self.abort.take() {
                    handle.detach();
                }
                std::task::Poll::Ready(Err(SpawnCanceled))
            }
            std::task::Poll::Pending => std::task::Poll::Pending,
        }
    }
}

impl<T> Drop for Spawned<T> {
    fn drop(&mut self) {
        // If the future was dropped before completion, abort the spawned
        // task to stop the wasted CPU work. AbortHandle::abort is a no-op
        // after the task has already finished, so this is always safe.
        if let Some(handle) = self.abort.take() {
            handle.abort();
        }
    }
}

/// Spawn `fut` on the runtime and return a future that resolves to its
/// output. Cancellation propagates: dropping the returned future aborts
/// the spawned task. A spawned-task panic surfaces as `Err(SpawnCanceled)`
/// rather than a panic on `rx.await`.
#[cfg(not(target_arch = "wasm32"))]
fn spawn_oneshot<F, T>(
    rt: &dyn Runtime,
    fut: F,
) -> impl Future<Output = std::result::Result<T, SpawnCanceled>> + Send + 'static
where
    F: Future<Output = T> + Send + 'static,
    T: Send + 'static,
{
    let (tx, rx) = futures::channel::oneshot::channel();
    let abort = rt.spawn(Box::pin(async move {
        let _ = tx.send(fut.await);
    }));
    Spawned {
        rx,
        abort: Some(abort),
    }
}

#[cfg(target_arch = "wasm32")]
fn spawn_oneshot<F, T>(
    rt: &dyn Runtime,
    fut: F,
) -> impl Future<Output = std::result::Result<T, SpawnCanceled>> + 'static
where
    F: Future<Output = T> + 'static,
    T: 'static,
{
    let (tx, rx) = futures::channel::oneshot::channel();
    let abort = rt.spawn(Box::pin(async move {
        let _ = tx.send(fut.await);
    }));
    Spawned {
        rx,
        abort: Some(abort),
    }
}

/// Encrypt padded plaintext for each device JID, producing participant `<to>` nodes.
///
/// Encrypt the plaintext for one device's Signal session. Shared by the
/// single-device fast path and the parallel fan-out so both behave identically.
async fn encrypt_one_device(
    plaintext: &[u8],
    addr: &ProtocolAddress,
    session_store: &mut dyn crate::libsignal::protocol::SessionStore,
    identity_store: &mut dyn crate::libsignal::protocol::IdentityKeyStore,
    device_jid: Jid,
) -> (Jid, Result<Option<EncryptOneResult>, String>) {
    match message_encrypt(plaintext, addr, session_store, identity_store).await {
        Ok(encrypted_payload) => {
            let Some((enc_type, is_prekey, serialized_bytes)) =
                extract_ciphertext(encrypted_payload)
            else {
                return (device_jid, Ok(None));
            };
            (
                device_jid,
                Ok(Some(EncryptOneResult {
                    enc_type,
                    is_prekey,
                    // Box<[u8]> -> Vec<u8> reuses the allocation (no copy).
                    ciphertext: serialized_bytes.into(),
                })),
            )
        }
        Err(e) => (device_jid, Err(format!("{addr}: {e}"))),
    }
}

/// Append one encrypt result to the raw fan-out output: an [`EncryptedDevice`]
/// on success, a logged skip on failure. Node-agnostic so both the message
/// `<to>` map and the voip offer share it.
fn push_raw_result(
    (device_jid, res): (Jid, Result<Option<EncryptOneResult>, String>),
    devices: &mut Vec<EncryptedDevice>,
    includes_prekey_message: &mut bool,
) {
    match res {
        Ok(Some(one)) => {
            *includes_prekey_message |= one.is_prekey;
            devices.push(EncryptedDevice {
                device_jid,
                enc_type: one.enc_type,
                is_prekey: one.is_prekey,
                ciphertext: one.ciphertext,
            });
        }
        Ok(None) => {}
        Err(msg) => log::warn!("Failed to encrypt for device: {msg}. Skipping."),
    }
}

/// Map one [`EncryptedDevice`] to the message path's `<to><enc>` participant
/// node, applying the batch-level `mediatype` and `decrypt-fail` attrs. This is
/// exactly the wire shape the DM/peer fan-out produced before the raw split.
fn encrypted_device_to_participant_node(
    one: EncryptedDevice,
    mediatype: Option<&str>,
    hide_decrypt_fail: bool,
) -> Node {
    let mut enc_builder = NodeBuilder::new("enc")
        .attr("v", stanza::ENC_VERSION)
        .attr("type", one.enc_type);
    // `mediatype` is batch-level (same for every device) and originates as
    // a `&'static str`, so it's threaded here instead of cloned per result.
    if let Some(mt) = mediatype {
        enc_builder = enc_builder.attr("mediatype", mt);
    }
    if hide_decrypt_fail {
        enc_builder = enc_builder.attr("decrypt-fail", "hide");
    }
    let enc_node = enc_builder.bytes(one.ciphertext).build();
    NodeBuilder::new("to")
        .attr("jid", one.device_jid)
        .children([enc_node])
        .build()
}

/// Per-device Signal sessions are independent (different ratchet state per
/// recipient), so this fans the encrypt loop out across tokio tasks bounded
/// by [`ENCRYPT_FANOUT_CONCURRENCY`]. Each task clones the store handles
/// (Arc bumps under the hood); the shared cache provides interior mutability.
///
/// Composition of [`ensure_sessions_for_devices`] (network: prekey fetch +
/// X3DH for missing sessions) and [`encrypt_for_devices_with_sessions`]
/// (CPU: the pairwise encrypt fan-out). Callers that must not hold a lock
/// across network I/O (the group sender-key chain lock) call the two phases
/// directly with the lock taken only around the second.
///
/// Callers must hold per-device session locks before calling this function —
/// concurrent ratchet mutations will corrupt Signal session state.
pub async fn encrypt_for_devices(
    runtime: &dyn Runtime,
    stores: &mut SignalStores<'_>,
    resolver: &dyn SendContextResolver,
    devices: &[Jid],
    plaintext_to_encrypt: &[u8],
    hide_decrypt_fail: bool,
    mediatype: Option<&str>,
) -> Result<EncryptResult> {
    let plan = ensure_sessions_for_devices(runtime, stores, resolver, devices).await?;
    encrypt_for_devices_with_sessions(
        runtime,
        stores,
        devices,
        plaintext_to_encrypt,
        hide_decrypt_fail,
        mediatype,
        plan,
    )
    .await
}

/// Session material prepared for one encrypt fan-out: per-index LID
/// encryption overrides (mirroring the `devices` slice it was built from)
/// plus whether any device 406'd during prekey fetch. Produced only by
/// [`ensure_sessions_for_devices`]; consumed by
/// [`encrypt_for_devices_with_sessions`] over the same `devices` slice.
pub struct SessionPlan {
    encryption_overrides: Vec<Option<Jid>>,
    pub had_unregistered_device: bool,
}

impl SessionPlan {
    /// A plan that performs no LID override and no prekey fetch: each device is
    /// encrypted against its own address as-is. For callers that already ensured
    /// sessions out-of-band (the voip offer asserts sessions before encrypting)
    /// and must not touch the network during the encrypt fan-out.
    pub fn assume_ready(device_count: usize) -> Self {
        Self {
            encryption_overrides: vec![None; device_count],
            had_unregistered_device: false,
        }
    }
}

/// Resolve LID overrides and establish missing Signal sessions (prekey
/// fetch + X3DH) for `devices`. This is the network half of the encrypt
/// fan-out and touches only session/identity state — never a sender-key
/// chain — so group sends run it before taking the chain lock.
#[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.send.ensure_sessions", level = "debug", skip_all, fields(count = devices.len()), err(Debug)))]
pub async fn ensure_sessions_for_devices(
    runtime: &dyn Runtime,
    stores: &mut SignalStores<'_>,
    resolver: &dyn SendContextResolver,
    devices: &[Jid],
) -> Result<SessionPlan> {
    // Per-device LID upgrade map: encryption_overrides[i] mirrors devices[i].
    // None = use devices[i] as-is; Some(jid) = use this LID-upgraded version.
    // The Vec replaces a HashMap<&Jid, Jid> that paid hash + alloc per insert
    // and per get (~666 of each on a large group). Plain Vec<Option<Jid>> is
    // direct indexing and contiguous memory.
    let mut encryption_overrides: Vec<Option<Jid>> = vec![None; devices.len()];
    // Indices into `devices` for those needing prekey fetch.
    let mut indices_needing_prekeys: Vec<usize> = Vec::with_capacity(devices.len());
    let mut had_406 = false;

    let mut reusable_addr = crate::types::jid::make_reusable_protocol_address();

    for (idx, device_jid) in devices.iter().enumerate() {
        // WhatsApp Web's SignalAddress.toString() normalizes PN → LID before
        // creating signal addresses. We do the same: check LID session FIRST.
        // This prevents using stale PN sessions when a newer LID session exists.
        if device_jid.is_pn()
            && let Some(lid_user) = resolver.get_lid_for_phone(&device_jid.user).await
        {
            // Construct the LID JID with the same device ID
            let lid_jid = Jid::lid_device(lid_user, device_jid.device);
            lid_jid.reset_protocol_address(&mut reusable_addr);

            if stores.session_store.has_session(&reusable_addr).await? {
                log::debug!(
                    "Using LID session {} for PN {} (LID-first lookup)",
                    lid_jid.observe(),
                    device_jid.observe()
                );
                encryption_overrides[idx] = Some(lid_jid);
                continue;
            }
        }

        device_jid.reset_protocol_address(&mut reusable_addr);
        if stores.session_store.has_session(&reusable_addr).await? {
            continue;
        }

        // No session found - need to fetch prekeys and create session.
        // Keep device_jid for prekey fetch (server returns bundles keyed by this),
        // but normalize to LID for the actual session creation.
        if device_jid.is_pn()
            && let Some(lid_user) = resolver.get_lid_for_phone(&device_jid.user).await
        {
            let lid_jid = Jid::lid_device(lid_user, device_jid.device);
            log::debug!(
                "Will create LID session {} for PN {} (no existing session)",
                lid_jid.observe(),
                device_jid.observe()
            );
            encryption_overrides[idx] = Some(lid_jid);
        }
        indices_needing_prekeys.push(idx);
    }

    if !indices_needing_prekeys.is_empty() {
        log::debug!(
            "Fetching prekeys for {} devices without sessions",
            indices_needing_prekeys.len()
        );
        // Materialize the Jid slice for the resolver call. fetch_prekeys
        // wants &[Jid]; same per-device clone count as the previous Vec
        // model, just sourced from the indices.
        let jids_for_fetch: Vec<Jid> = indices_needing_prekeys
            .iter()
            .map(|&i| devices[i].clone())
            .collect();
        // 406 on this batch is all-or-nothing — per-device retries just wasted
        // N·RTT with the same failure. Mark `had_406` so the caller invalidates
        // the users and the next send re-fetches. Matches WA Web's
        // `GroupSkmsgJob`: log, continue without those devices.
        let prekey_bundles = match resolver
            .fetch_prekeys_for_identity_check(&jids_for_fetch)
            .await
        {
            Ok(bundles) => bundles,
            Err(e) if is_device_unregistered_error(&e) => {
                // No server prekeys for these devices this round; the next send
                // re-fetches. Debug, not warn — a batch 406 would otherwise flood the log.
                log::debug!(
                    "Prekey fetch returned 406 for {} device(s); skipping them this round",
                    jids_for_fetch.len()
                );
                had_406 = true;
                std::collections::HashMap::new()
            }
            Err(e) => return Err(e),
        };

        // Parallel session establishment via process_prekey_bundle. Each
        // recipient device has an independent Signal session and an
        // independent prekey bundle, so the X3DH derivation runs on a
        // separate task per device, bounded at ENCRYPT_FANOUT_CONCURRENCY.
        // Spawning goes through `Runtime::spawn` (the platform-agnostic
        // abstraction) plus a oneshot channel for result delivery —
        // `FuturesUnordered` handles the in-flight window.
        let prekey_bundles = std::sync::Arc::new(prekey_bundles);
        let total = indices_needing_prekeys.len();
        let mut next_spawn = 0usize;

        let make_session_task = |spawn_idx: usize| {
            let idx = indices_needing_prekeys[spawn_idx];
            let device_jid = devices[idx].clone();
            let mut encryption_jid = encryption_overrides[idx]
                .clone()
                .unwrap_or_else(|| device_jid.clone());

            // Normalize agent to 0 for LID JIDs to match how pre-key bundles are stored.
            // prekeys.rs forces agent=0 for LID; we must match that here.
            if encryption_jid.is_lid() {
                encryption_jid.agent = 0;
            }

            let lookup_jid = device_jid.normalize_for_prekey_bundle();
            let bundles = prekey_bundles.clone();
            let mut session_store = stores.session_store.clone_box();
            let mut identity_store = stores.identity_store.clone_box();

            spawn_oneshot(runtime, async move {
                let mut addr = crate::types::jid::make_reusable_protocol_address();
                encryption_jid.reset_protocol_address(&mut addr);

                let Some(bundle) = bundles.get(&lookup_jid) else {
                    // No key material this round (usually the 406 cascade); the next
                    // send re-fetches. Debug avoids one warn per skipped device.
                    log::debug!(
                        "No pre-key bundle returned for device {}. This device will be skipped for encryption.",
                        addr
                    );
                    return Ok::<Option<Jid>, anyhow::Error>(None);
                };

                let mut rng = rand::make_rng::<rand::rngs::StdRng>();
                // No UntrustedIdentity recovery: WA Web's isTrustedIdentity is
                // unconditional Ok(true) (TOFU), and save_identity inside
                // process_prekey_bundle persists rotations transparently.
                match process_prekey_bundle(
                    &addr,
                    &mut *session_store,
                    &mut *identity_store,
                    bundle,
                    &mut rng,
                    UsePQRatchet::No,
                )
                .await
                {
                    // Surface a replaced identity so the caller can react
                    // (resolver has no 'static handle into this spawned task).
                    Ok(IdentityChange::ReplacedExisting) => Ok(Some(encryption_jid)),
                    Ok(IdentityChange::NewOrUnchanged) => Ok(None),
                    Err(e) => Err(anyhow::anyhow!(
                        "Failed to process pre-key bundle for {}: {:?}",
                        addr,
                        e
                    )),
                }
            })
        };

        let mut in_flight: FuturesUnordered<_> = FuturesUnordered::new();
        while next_spawn < total && in_flight.len() < ENCRYPT_FANOUT_CONCURRENCY {
            in_flight.push(make_session_task(next_spawn));
            next_spawn += 1;
        }
        while let Some(spawn_result) = in_flight.next().await {
            match spawn_result {
                // Some(jid) => establishing this session replaced a stored
                // identity; notify the client so it can react off-path.
                Ok(Ok(Some(changed_jid))) => resolver.on_local_identity_change(&changed_jid),
                Ok(Ok(None)) => {}
                Ok(Err(e)) => return Err(e),
                Err(SpawnCanceled) => {
                    log::warn!(
                        "Session-establishment task did not deliver a result; skipping device."
                    );
                }
            }
            if next_spawn < total {
                in_flight.push(make_session_task(next_spawn));
                next_spawn += 1;
            }
        }
    }

    Ok(SessionPlan {
        encryption_overrides,
        had_unregistered_device: had_406,
    })
}

/// CPU half of the encrypt fan-out: pairwise-encrypt `plaintext_to_encrypt`
/// for each device using sessions prepared by [`ensure_sessions_for_devices`]
/// over the same `devices` slice. No resolver, no network — safe to run
/// under locks that must not span I/O. A device whose session is still
/// missing (e.g. its bundle was absent) fails its encrypt and is skipped,
/// matching the combined path's behavior.
#[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.send.encrypt_fanout", level = "debug", skip_all, fields(count = devices.len()), err(Debug)))]
pub async fn encrypt_for_devices_with_sessions(
    runtime: &dyn Runtime,
    stores: &mut SignalStores<'_>,
    devices: &[Jid],
    plaintext_to_encrypt: &[u8],
    hide_decrypt_fail: bool,
    mediatype: Option<&str>,
    plan: SessionPlan,
) -> Result<EncryptResult> {
    let raw =
        encrypt_for_devices_with_sessions_raw(runtime, stores, devices, plaintext_to_encrypt, plan)
            .await?;

    // Map each ciphertext to the message path's `<to><enc>` node, preserving the
    // raw fan-out order so the wire output is identical to the pre-split path.
    // `encrypted_devices` mirrors `participant_nodes` index-for-index, as before.
    let mut participant_nodes = Vec::with_capacity(raw.devices.len());
    let mut encrypted_devices = Vec::with_capacity(raw.devices.len());
    for one in raw.devices {
        encrypted_devices.push(one.device_jid.clone());
        participant_nodes.push(encrypted_device_to_participant_node(
            one,
            mediatype,
            hide_decrypt_fail,
        ));
    }

    Ok(EncryptResult {
        participant_nodes,
        includes_prekey_message: raw.includes_prekey_message,
        encrypted_devices,
        had_unregistered_device: raw.had_unregistered_device,
    })
}

/// Node-agnostic core of the encrypt fan-out: pairwise-encrypt
/// `plaintext_to_encrypt` for each device using sessions prepared by
/// [`ensure_sessions_for_devices`] over the same `devices` slice, returning the
/// per-device ciphertexts. No resolver, no network, no node-building — safe to
/// run under locks that must not span I/O. A device whose session is still
/// missing (e.g. its bundle was absent) fails its encrypt and is skipped.
/// Same parallel fan-out + skip-on-fail contract as the message path.
#[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.send.encrypt_fanout", level = "debug", skip_all, fields(count = devices.len()), err(Debug)))]
pub async fn encrypt_for_devices_with_sessions_raw(
    runtime: &dyn Runtime,
    stores: &mut SignalStores<'_>,
    devices: &[Jid],
    plaintext_to_encrypt: &[u8],
    plan: SessionPlan,
) -> Result<EncryptForDevicesRaw> {
    debug_assert_eq!(
        plan.encryption_overrides.len(),
        devices.len(),
        "SessionPlan built for a different device list"
    );
    let SessionPlan {
        encryption_overrides,
        had_unregistered_device,
    } = plan;

    let mut encrypted = Vec::with_capacity(devices.len());
    let mut includes_prekey_message = false;

    // The wire-order of `<to>` participants does not need to match the input
    // device order: WA Web's `phash` (computed both client and server side)
    // sorts before hashing, as does our `participant_list_hash`.
    if devices.len() == 1 {
        // Single recipient device: the parallel fan-out is pure overhead here
        // (an Arc<[u8]> copy of the plaintext, a spawned task + oneshot channel,
        // a FuturesUnordered, and two store clones), with no parallelism to gain.
        // Encrypt inline.
        let device_jid = devices[0].clone();
        let addr = encryption_overrides
            .first()
            .and_then(|o| o.as_ref())
            .unwrap_or(&devices[0])
            .to_protocol_address();
        let res = encrypt_one_device(
            plaintext_to_encrypt,
            &addr,
            &mut *stores.session_store,
            &mut *stores.identity_store,
            device_jid,
        )
        .await;
        push_raw_result(res, &mut encrypted, &mut includes_prekey_message);
    } else {
        // Parallel encrypt fan-out across tokio tasks bounded by
        // ENCRYPT_FANOUT_CONCURRENCY; collected in completion order so the
        // fastest encrypts ship first.
        let plaintext_arc: std::sync::Arc<[u8]> = std::sync::Arc::from(plaintext_to_encrypt);

        let total = devices.len();
        let mut next_spawn = 0usize;

        let make_encrypt_task = |idx: usize| {
            let device_jid = devices[idx].clone();
            // The encryption JID is only needed to build the Signal address, so
            // derive it here from a borrow rather than cloning the whole Jid into
            // the task (device_jid is still cloned because it's returned).
            let addr = encryption_overrides
                .get(idx)
                .and_then(|o| o.as_ref())
                .unwrap_or(&devices[idx])
                .to_protocol_address();
            let plaintext = plaintext_arc.clone();
            let mut session_store = stores.session_store.clone_box();
            let mut identity_store = stores.identity_store.clone_box();

            spawn_oneshot(runtime, async move {
                encrypt_one_device(
                    &plaintext,
                    &addr,
                    &mut *session_store,
                    &mut *identity_store,
                    device_jid,
                )
                .await
            })
        };

        let mut in_flight: FuturesUnordered<_> = FuturesUnordered::new();
        while next_spawn < total && in_flight.len() < ENCRYPT_FANOUT_CONCURRENCY {
            in_flight.push(make_encrypt_task(next_spawn));
            next_spawn += 1;
        }
        while let Some(spawn_result) = in_flight.next().await {
            match spawn_result {
                Ok(res) => push_raw_result(res, &mut encrypted, &mut includes_prekey_message),
                Err(SpawnCanceled) => {
                    log::warn!("Encrypt task did not deliver a result; skipping device.");
                }
            }

            if next_spawn < total {
                in_flight.push(make_encrypt_task(next_spawn));
                next_spawn += 1;
            }
        }
    }

    Ok(EncryptForDevicesRaw {
        devices: encrypted,
        includes_prekey_message,
        had_unregistered_device,
    })
}

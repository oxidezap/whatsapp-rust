//! Per-device Signal encryption fanout and the bounded spawn helper.

use super::*;
use anyhow::Context;

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
    // Delegate to the libsignal primitive so the sender-key advance, the wire
    // gate, and the iteration lease live in exactly one place. `.context` keeps
    // the concrete SignalProtocolError as the source, so callers can still
    // downcast NoSenderKeyState to clear stale tracking and retry with SKDM
    // redistribution.
    crate::libsignal::protocol::group_encrypt(sender_key_store, sender_key_name, plaintext, csprng)
        .await
        .context("group encrypt failed")
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

pub(crate) struct EncryptAttempt {
    pub result: EncryptResult,
    pub first_error: Option<anyhow::Error>,
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

struct RawEncryptAttempt {
    result: EncryptForDevicesRaw,
    first_error: Option<anyhow::Error>,
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
    account: Option<&wa::ADVSignedDeviceIdentity>,
) -> Result<Option<Vec<u8>>> {
    if !includes_prekey {
        return Ok(None);
    }
    let acc = account
        .ok_or_else(|| anyhow!("pkmsg requires <device-identity> but no ADV account is present"))?;
    Ok(Some(waproto::codec::adv_signed_device_identity_to_vec(acc)))
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
) -> (Jid, Result<Option<EncryptOneResult>>) {
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
        Err(error) => (
            device_jid,
            Err(anyhow::Error::new(error).context(format!("failed to encrypt for {addr}"))),
        ),
    }
}

/// Append one encrypt result to the raw fan-out output: an [`EncryptedDevice`]
/// on success, a logged skip on failure. Node-agnostic so both the message
/// `<to>` map and the voip offer share it.
fn push_raw_result(
    (device_jid, res): (Jid, Result<Option<EncryptOneResult>>),
    devices: &mut Vec<EncryptedDevice>,
    includes_prekey_message: &mut bool,
    first_error: &mut Option<anyhow::Error>,
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
        Err(error) => {
            log::warn!("Failed to encrypt for device: {error:#}. Skipping.");
            if first_error.is_none() {
                *first_error = Some(error);
            }
        }
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
    first_error: Option<anyhow::Error>,
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
            first_error: None,
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
    let mut first_error = None;

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
                    Err(error) => Err(anyhow::Error::new(error)
                        .context(format!("failed to process pre-key bundle for {addr}"))),
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
                // Isolate the failure to this device so one participant can't abort
                // the cohort's SKDM (matching WA Web GroupKeyDistributionMsg's
                // per-device try/catch). The sessionless device is dropped by the
                // fan-out below.
                Ok(Err(e)) => {
                    log::warn!("Group session setup failed for a device, skipping it: {e}");
                    if first_error.is_none() {
                        first_error = Some(e);
                    }
                }
                Err(error) => {
                    log::warn!(
                        "Session-establishment task did not deliver a result; skipping device."
                    );
                    if first_error.is_none() {
                        first_error = Some(anyhow::Error::new(error));
                    }
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
        first_error,
    })
}

/// CPU half of the encrypt fan-out: pairwise-encrypt `plaintext_to_encrypt`
/// for each device using sessions prepared by [`ensure_sessions_for_devices`]
/// over the same `devices` slice. No resolver, no network — safe to run
/// under locks that must not span I/O. A device whose session is still
/// missing (e.g. its bundle was absent) fails its encrypt and is skipped,
/// matching the combined path's behavior.
pub async fn encrypt_for_devices_with_sessions(
    runtime: &dyn Runtime,
    stores: &mut SignalStores<'_>,
    devices: &[Jid],
    plaintext_to_encrypt: &[u8],
    hide_decrypt_fail: bool,
    mediatype: Option<&str>,
    plan: SessionPlan,
) -> Result<EncryptResult> {
    Ok(encrypt_for_devices_with_sessions_detailed(
        runtime,
        stores,
        devices,
        plaintext_to_encrypt,
        hide_decrypt_fail,
        mediatype,
        plan,
    )
    .await?
    .result)
}

#[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.send.encrypt_fanout", level = "debug", skip_all, fields(count = devices.len()), err(Debug)))]
pub(crate) async fn encrypt_for_devices_with_sessions_detailed(
    runtime: &dyn Runtime,
    stores: &mut SignalStores<'_>,
    devices: &[Jid],
    plaintext_to_encrypt: &[u8],
    hide_decrypt_fail: bool,
    mediatype: Option<&str>,
    plan: SessionPlan,
) -> Result<EncryptAttempt> {
    let RawEncryptAttempt {
        result: raw,
        first_error,
    } = encrypt_for_devices_with_sessions_raw_detailed(
        runtime,
        stores,
        devices,
        plaintext_to_encrypt,
        plan,
    )
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

    Ok(EncryptAttempt {
        result: EncryptResult {
            participant_nodes,
            includes_prekey_message: raw.includes_prekey_message,
            encrypted_devices,
            had_unregistered_device: raw.had_unregistered_device,
        },
        first_error,
    })
}

/// Node-agnostic core of the encrypt fan-out: pairwise-encrypt
/// `plaintext_to_encrypt` for each device using sessions prepared by
/// [`ensure_sessions_for_devices`] over the same `devices` slice, returning the
/// per-device ciphertexts. No resolver, no network, no node-building — safe to
/// run under locks that must not span I/O. A device whose session is still
/// missing (e.g. its bundle was absent) fails its encrypt and is skipped.
/// Same parallel fan-out + skip-on-fail contract as the message path.
pub async fn encrypt_for_devices_with_sessions_raw(
    runtime: &dyn Runtime,
    stores: &mut SignalStores<'_>,
    devices: &[Jid],
    plaintext_to_encrypt: &[u8],
    plan: SessionPlan,
) -> Result<EncryptForDevicesRaw> {
    Ok(encrypt_for_devices_with_sessions_raw_detailed(
        runtime,
        stores,
        devices,
        plaintext_to_encrypt,
        plan,
    )
    .await?
    .result)
}

#[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.send.encrypt_fanout_raw", level = "debug", skip_all, fields(count = devices.len()), err(Debug)))]
async fn encrypt_for_devices_with_sessions_raw_detailed(
    runtime: &dyn Runtime,
    stores: &mut SignalStores<'_>,
    devices: &[Jid],
    plaintext_to_encrypt: &[u8],
    plan: SessionPlan,
) -> Result<RawEncryptAttempt> {
    debug_assert_eq!(
        plan.encryption_overrides.len(),
        devices.len(),
        "SessionPlan built for a different device list"
    );
    let SessionPlan {
        encryption_overrides,
        had_unregistered_device,
        mut first_error,
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
        push_raw_result(
            res,
            &mut encrypted,
            &mut includes_prekey_message,
            &mut first_error,
        );
    } else {
        // One task per chunk, not per device: the per-device fan-out allocated a
        // task + oneshot + two store clones for every recipient. Same parallelism,
        // spawns bounded by ENCRYPT_FANOUT_CONCURRENCY. Wire order is irrelevant
        // (phash sorts before hashing on both ends).
        let plaintext_arc: std::sync::Arc<[u8]> = std::sync::Arc::from(plaintext_to_encrypt);

        let total = devices.len();
        let num_chunks = ENCRYPT_FANOUT_CONCURRENCY.min(total);

        let mut in_flight: FuturesUnordered<_> = FuturesUnordered::new();
        // Index partitioning gives exactly num_chunks slices (keeps the configured
        // parallelism) and no-ops on an empty device set instead of dividing by zero.
        for chunk_idx in 0..num_chunks {
            let chunk_start = chunk_idx * total / num_chunks;
            let chunk_end = (chunk_idx + 1) * total / num_chunks;
            // The 'static task can't borrow devices/encryption_overrides.
            let jobs: Vec<(ProtocolAddress, Jid)> = (chunk_start..chunk_end)
                .map(|idx| {
                    let addr = encryption_overrides
                        .get(idx)
                        .and_then(|o| o.as_ref())
                        .unwrap_or(&devices[idx])
                        .to_protocol_address();
                    (addr, devices[idx].clone())
                })
                .collect();
            let plaintext = plaintext_arc.clone();
            // clone_box shares the Arc-backed backend, so the sequential ratchet
            // advances persist despite one clone serving the whole chunk.
            let mut session_store = stores.session_store.clone_box();
            let mut identity_store = stores.identity_store.clone_box();

            in_flight.push(spawn_oneshot(runtime, async move {
                let mut out = Vec::with_capacity(jobs.len());
                for (addr, device_jid) in jobs {
                    out.push(
                        encrypt_one_device(
                            &plaintext,
                            &addr,
                            &mut *session_store,
                            &mut *identity_store,
                            device_jid,
                        )
                        .await,
                    );
                }
                out
            }));
        }
        while let Some(spawn_result) = in_flight.next().await {
            match spawn_result {
                Ok(results) => {
                    for res in results {
                        push_raw_result(
                            res,
                            &mut encrypted,
                            &mut includes_prekey_message,
                            &mut first_error,
                        );
                    }
                }
                Err(error) => {
                    // A whole chunk drops (not one device); its members stay
                    // un-warm and are re-targeted next send.
                    log::warn!(
                        "Encrypt chunk did not deliver a result; up to ~{} device(s) skipped this send.",
                        total.div_ceil(num_chunks)
                    );
                    if first_error.is_none() {
                        first_error = Some(anyhow::Error::new(error));
                    }
                }
            }
        }
    }

    Ok(RawEncryptAttempt {
        result: EncryptForDevicesRaw {
            devices: encrypted,
            includes_prekey_message,
            had_unregistered_device,
        },
        first_error,
    })
}

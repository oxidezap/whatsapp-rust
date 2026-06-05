//! Per-device Signal encryption fanout and the bounded spawn helper.

use super::*;

/// Caller must hold `SenderKeyStore::sender_key_lock` for `sender_key_name`
/// across the surrounding SKDM creation + this encrypt, so a concurrent send
/// can't split the key between the SKDM and the skmsg.
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

pub struct SignalStores<'a, S, I, P, SP> {
    pub sender_key_store: &'a mut (dyn crate::libsignal::protocol::SenderKeyStore + Send + Sync),
    pub session_store: &'a mut S,
    pub identity_store: &'a mut I,
    pub prekey_store: &'a mut P,
    pub signed_prekey_store: &'a SP,
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

/// Maximum number of concurrent per-device crypto tasks during group send
/// fan-out. Picked from the `perf-audit` benchmark: speedup plateaus around
/// 16 on Oracle ARM64; 32 gives only ~10% more for double the task overhead.
const ENCRYPT_FANOUT_CONCURRENCY: usize = 16;

/// Per-task encrypt result, shipped from a spawned task back to the orchestrator.
struct EncryptOneResult {
    enc_type: &'static str,
    is_prekey: bool,
    ciphertext: Vec<u8>,
    hide_decrypt_fail: bool,
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
    hide_decrypt_fail: bool,
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
                    hide_decrypt_fail,
                })),
            )
        }
        Err(e) => (device_jid, Err(format!("{addr}: {e}"))),
    }
}

/// Append one encrypt result to the fan-out output: a `<to>` participant node on
/// success, a logged skip on failure.
fn push_encrypt_result(
    (device_jid, res): (Jid, Result<Option<EncryptOneResult>, String>),
    mediatype: Option<&str>,
    participant_nodes: &mut Vec<Node>,
    encrypted_devices: &mut Vec<Jid>,
    includes_prekey_message: &mut bool,
) {
    match res {
        Ok(Some(one)) => {
            *includes_prekey_message |= one.is_prekey;
            let mut enc_builder = NodeBuilder::new("enc")
                .attr("v", stanza::ENC_VERSION)
                .attr("type", one.enc_type);
            // `mediatype` is batch-level (same for every device) and originates as
            // a `&'static str`, so it's threaded here instead of cloned per result.
            if let Some(mt) = mediatype {
                enc_builder = enc_builder.attr("mediatype", mt);
            }
            if one.hide_decrypt_fail {
                enc_builder = enc_builder.attr("decrypt-fail", "hide");
            }
            let enc_node = enc_builder.bytes(one.ciphertext).build();
            participant_nodes.push(
                NodeBuilder::new("to")
                    .attr("jid", device_jid.clone())
                    .children([enc_node])
                    .build(),
            );
            encrypted_devices.push(device_jid);
        }
        Ok(None) => {}
        Err(msg) => log::warn!("Failed to encrypt for device: {msg}. Skipping."),
    }
}

/// Per-device Signal sessions are independent (different ratchet state per
/// recipient), so this fans the encrypt loop out across tokio tasks bounded
/// by [`ENCRYPT_FANOUT_CONCURRENCY`]. Each task clones the store handles
/// (Arc bumps under the hood); the shared cache provides interior mutability.
///
/// Callers must hold per-device session locks before calling this function —
/// concurrent ratchet mutations will corrupt Signal session state.
pub async fn encrypt_for_devices<'a, S, I, P, SP>(
    runtime: &dyn Runtime,
    stores: &mut SignalStores<'a, S, I, P, SP>,
    resolver: &dyn SendContextResolver,
    devices: &[Jid],
    plaintext_to_encrypt: &[u8],
    hide_decrypt_fail: bool,
    mediatype: Option<&str>,
) -> Result<EncryptResult>
where
    S: crate::libsignal::protocol::SessionStore + Clone + Send + Sync + 'static,
    I: crate::libsignal::protocol::IdentityKeyStore + Clone + Send + Sync + 'static,
    P: crate::libsignal::protocol::PreKeyStore + Send + Sync,
    SP: crate::libsignal::protocol::SignedPreKeyStore + Send + Sync,
{
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
                    lid_jid,
                    device_jid
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
                lid_jid,
                device_jid
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
                log::warn!(
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
            let mut session_store = stores.session_store.clone();
            let mut identity_store = stores.identity_store.clone();

            spawn_oneshot(runtime, async move {
                let mut addr = crate::types::jid::make_reusable_protocol_address();
                encryption_jid.reset_protocol_address(&mut addr);

                let Some(bundle) = bundles.get(&lookup_jid) else {
                    log::warn!(
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
                    &mut session_store,
                    &mut identity_store,
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

    let mut participant_nodes = Vec::with_capacity(devices.len());
    let mut includes_prekey_message = false;
    let mut encrypted_devices = Vec::with_capacity(devices.len());

    // The wire-order of `<to>` participants does not need to match the input
    // device order: WA Web's `phash` (computed both client and server side)
    // sorts before hashing, as does our `participant_list_hash`.
    if devices.len() == 1 {
        // Single recipient device: the parallel fan-out is pure overhead here
        // (an Arc<[u8]> copy of the plaintext, a spawned task + oneshot channel,
        // a FuturesUnordered, and two store clones), with no parallelism to gain.
        // Encrypt inline.
        let device_jid = devices[0].clone();
        let addr = encryption_overrides[0]
            .as_ref()
            .unwrap_or(&devices[0])
            .to_protocol_address();
        let res = encrypt_one_device(
            plaintext_to_encrypt,
            &addr,
            &mut *stores.session_store,
            &mut *stores.identity_store,
            device_jid,
            hide_decrypt_fail,
        )
        .await;
        push_encrypt_result(
            res,
            mediatype,
            &mut participant_nodes,
            &mut encrypted_devices,
            &mut includes_prekey_message,
        );
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
            let addr = encryption_overrides[idx]
                .as_ref()
                .unwrap_or(&devices[idx])
                .to_protocol_address();
            let plaintext = plaintext_arc.clone();
            let mut session_store = stores.session_store.clone();
            let mut identity_store = stores.identity_store.clone();

            spawn_oneshot(runtime, async move {
                encrypt_one_device(
                    &plaintext,
                    &addr,
                    &mut session_store,
                    &mut identity_store,
                    device_jid,
                    hide_decrypt_fail,
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
                Ok(res) => push_encrypt_result(
                    res,
                    mediatype,
                    &mut participant_nodes,
                    &mut encrypted_devices,
                    &mut includes_prekey_message,
                ),
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

    Ok(EncryptResult {
        participant_nodes,
        includes_prekey_message,
        encrypted_devices,
        had_unregistered_device: had_406,
    })
}

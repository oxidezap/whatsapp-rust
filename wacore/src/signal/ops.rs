// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

use super::types::{PublicIdentity, PublicPreKey, PublicSignedPreKey};
use crate::libsignal::protocol::{
    CiphertextMessageType, KeyPair, PreKeySignalMessage, ProtocolAddress,
    SenderKeyDistributionMessage, SignalMessage, UsePQRatchet,
    create_sender_key_distribution_message, group_decrypt, group_encrypt, message_decrypt_prekey,
    message_decrypt_signal, message_encrypt, process_prekey_bundle,
    process_sender_key_distribution_message,
};
use crate::libsignal::store::record_helpers;
use crate::libsignal::store::sender_key_name::SenderKeyName;
use crate::pair::PairUtils;
use crate::signal::device::{DeviceKeys, MAX_PREKEY_ID, public_bytes, sign_prekey};
use crate::signal::error::OpsError;
use crate::signal::stores::{KeyedLocks, Stores};
use crate::store::transaction::SignalRecord as Namespace;
use crate::types::jid::JidExt;
use async_lock::{Mutex, RwLock};
use std::sync::Arc;
use wacore_binary::{Jid, Server};

#[path = "prekeys.rs"]
mod prekeys;

pub use crate::store::transaction::{AuthorityKey, SignalOperationStore};

pub use super::types::{Decrypted, EncKind, Encrypted, PairingSignature, RemoteBundle};

use crate::signal::guard::Held;

/// Lock key for one peer device's session and identity.
pub(crate) fn session_key(address: &ProtocolAddress) -> String {
    format!("s:{}", address.as_str())
}

/// The device of `user`'s address `address` (a stored session or identity
/// key) on `server`, or `None` when it belongs to someone else. Two forms
/// are stored: wacore's `{user}[:{device}]@{server}.0`, with WA Web's `c.us`
/// spelling for phone numbers, and the plain libsignal name
/// `{user}@{server}.{device}` that callers can pass to `establish_session`
/// or `decrypt`. A `:device` part wins over the suffix.
fn device_of(address: &str, user: &str, server: Server) -> Option<u16> {
    let rest = address.strip_prefix(user)?;
    let (named, rest) = match rest.strip_prefix(':') {
        Some(tail) => {
            let at = tail.find('@')?;
            (Some(tail[..at].parse().ok()?), &tail[at..])
        }
        None => (None, rest),
    };
    let rest = rest.strip_prefix('@')?;
    let spellings: &[&str] = match server {
        Server::Pn => &["c.us", "s.whatsapp.net"],
        other => &[other.as_str()],
    };
    // The libsignal device suffix. Requiring digits keeps `hosted` from
    // matching `hosted.lid` addresses.
    let suffix = spellings
        .iter()
        .find_map(|spelling| rest.strip_prefix(spelling)?.strip_prefix('.'))?;
    if suffix.is_empty() || !suffix.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    Some(match named {
        Some(device) => device,
        None => suffix.parse().ok()?,
    })
}

/// `server` plus its hosted counterpart, if it has one.
fn with_hosted(server: Server) -> Vec<Server> {
    match server {
        Server::Pn => vec![Server::Pn, Server::Hosted],
        Server::Lid => vec![Server::Lid, Server::HostedLid],
        other => vec![other],
    }
}

/// Lock key for one sender-key chain.
pub(crate) fn chain_key(group: &str, sender: &str) -> String {
    format!("k:{group}\u{0}{sender}")
}

/// Lock key for a group's sender-key distribution marks.
/// Per-account lock: every path that can create a session with a peer takes
/// it, and `forget_user` takes it before scanning, so a session set up before
/// a reset cannot commit after it. Keyed on the user part, covering the
/// account's PN, hosted and LID addresses of that number alike. `p:` sorts
/// after `g:`/`k:` and before `s:`, matching the order every caller already
/// acquires those in.
pub(crate) fn account_lock(user: &str) -> String {
    format!("p:{user}")
}

pub(crate) fn group_key(group: &str) -> String {
    format!("g:{group}")
}

/// Signal operations for one scope (one linked device).
///
/// Every operation that changes Signal state first locks exactly the state it
/// can write: a peer's session, a sender-key chain, or a group's distribution
/// marks. Staged views commit whole batches, so two views writing the same
/// record would lose an update; disjoint views proceed in parallel. Locks are
/// taken in sorted order, so overlapping operations cannot deadlock. The
/// store must coordinate writers in other processes.
pub struct SignalOps<S> {
    store: Arc<S>,
    authority: Arc<Authority>,
    /// Keeps this authority's registry entry alive, so a later open joins it.
    _slot: Arc<AuthoritySlot>,
}

/// Cached keys, locks and the usability flag for one writer authority. A second
/// instance opened under the same authority shares them instead of creating an
/// independent authority whose writes the store would also accept.
struct Authority {
    keys: RwLock<Arc<DeviceKeys>>,
    /// Held while device keys are read-modify-written.
    device_write: Arc<Mutex<()>>,
    usable: Arc<std::sync::atomic::AtomicBool>,
    locks: Arc<KeyedLocks>,
    sender_key_locks: Arc<KeyedLocks>,
}

/// One writer's authority, filled by the first open that reads its keys.
type AuthoritySlot = async_lock::OnceCell<Arc<Authority>>;

type AuthorityRegistry =
    std::sync::Mutex<std::collections::HashMap<AuthorityKey, std::sync::Weak<AuthoritySlot>>>;

fn authorities() -> &'static AuthorityRegistry {
    static AUTHORITIES: std::sync::OnceLock<AuthorityRegistry> = std::sync::OnceLock::new();
    AUTHORITIES.get_or_init(Default::default)
}

/// Return the live slot for `key`, or register an empty one. The slot is
/// registered before any device keys are read, so an open that overlaps a
/// live instance, or another open, shares one authority instead of building
/// a second one from a read taken before the other's writes.
fn authority_slot(key: AuthorityKey) -> Arc<AuthoritySlot> {
    let mut map = authorities()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if let Some(live) = map.get(&key).and_then(std::sync::Weak::upgrade) {
        return live;
    }
    map.retain(|_, weak| weak.strong_count() > 0);
    let slot = Arc::new(AuthoritySlot::new());
    map.insert(key, Arc::downgrade(&slot));
    slot
}

fn new_authority(keys: DeviceKeys) -> Arc<Authority> {
    Arc::new(Authority {
        keys: RwLock::new(Arc::new(keys)),
        device_write: Arc::new(Mutex::new(())),
        usable: Arc::new(std::sync::atomic::AtomicBool::new(true)),
        locks: Arc::new(KeyedLocks::default()),
        sender_key_locks: Arc::new(KeyedLocks::default()),
    })
}

fn rng() -> rand::rngs::StdRng {
    rand::make_rng::<rand::rngs::StdRng>()
}

impl<S: SignalOperationStore> SignalOps<S> {
    /// Open a scope that already has device keys. Opening a scope again under
    /// the same authority while an instance is live shares that instance's keys,
    /// locks and retirement state.
    pub async fn open(store: Arc<S>) -> Result<Self, OpsError> {
        let slot = authority_slot(store.authority_key());
        let loaded = std::sync::atomic::AtomicBool::new(false);
        let authority = slot
            .get_or_try_init(|| async {
                let keys = Self::stored_keys(&store).await?;
                loaded.store(true, std::sync::atomic::Ordering::SeqCst);
                Ok::<_, OpsError>(new_authority(keys))
            })
            .await?
            .clone();
        if !loaded.load(std::sync::atomic::Ordering::SeqCst) {
            // Joining a live authority skips its key read. Read through this
            // store anyway: one with a different sealer must fail here, not
            // later reseal the shared keys under its own key. The read only
            // checks; the authority keeps the keys it already holds.
            let own = Self::stored_keys(&store).await?;
            let shared = authority.keys.read().await.clone();
            if own.identity.public_key.public_key_bytes()
                != shared.identity.public_key.public_key_bytes()
            {
                return Err(OpsError::InvalidInput(
                    "this store's device keys differ from the open instance's".into(),
                ));
            }
        }
        Ok(Self {
            store,
            authority,
            _slot: slot,
        })
    }

    /// Generate and persist device keys for a new scope. Refuses to replace
    /// existing keys, which would silently unlink the device; the write is an
    /// exclusive insert, so concurrent initialisers cannot both succeed.
    pub async fn create(store: Arc<S>) -> Result<Self, OpsError> {
        let exists = || OpsError::InvalidInput("scope already has device keys".into());
        let slot = authority_slot(store.authority_key());
        let created = std::sync::atomic::AtomicBool::new(false);
        let authority = slot
            .get_or_try_init(|| async {
                let keys = DeviceKeys::generate()?;
                match store.create_device_keys(&keys.encode()?).await {
                    Ok(()) => {
                        created.store(true, std::sync::atomic::Ordering::SeqCst);
                        Ok(new_authority(keys))
                    }
                    Err(e) if crate::store::transaction::is_record_exists(&e) => Err(exists()),
                    Err(e) => Err(e.into()),
                }
            })
            .await?
            .clone();
        // A live instance under this authority means the keys already exist.
        if !created.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(exists());
        }
        Ok(Self {
            store,
            authority,
            _slot: slot,
        })
    }

    async fn stores(&self) -> Stores<S> {
        self.stores_over(self.store.clone()).await
    }

    pub(crate) async fn stores_over(&self, store: Arc<S>) -> Stores<S> {
        Stores {
            store,
            keys: self.authority.keys.read().await.clone(),
            sender_key_locks: self.authority.sender_key_locks.clone(),
        }
    }

    pub(crate) fn store(&self) -> &Arc<S> {
        &self.store
    }

    /// Whether this instance can still mutate Signal state. Cancellation of a
    /// locked operation retires it; reconcile persisted state and use a fresh authority
    /// before constructing its replacement.
    pub fn is_usable(&self) -> bool {
        self.authority
            .usable
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    fn require_usable(&self) -> Result<(), OpsError> {
        if self.is_usable() {
            Ok(())
        } else {
            Err(crate::signal::error::interrupted_operation())
        }
    }

    pub(crate) async fn lock_device(&self) -> Result<Held, OpsError> {
        self.require_usable()?;
        let held = self.authority.device_write.lock_arc().await;
        self.require_usable()?;
        Ok(Held::new(vec![held], self.authority.usable.clone()))
    }

    /// Acquire sorted state locks, then check for a cancelled predecessor.
    pub(crate) async fn lock_keys(&self, mut keys: Vec<String>) -> Result<Held, OpsError> {
        self.require_usable()?;
        keys.sort();
        keys.dedup();
        let mut held = Vec::with_capacity(keys.len());
        for key in keys {
            held.push(self.authority.locks.get(&key).lock_arc().await);
        }
        self.require_usable()?;
        Ok(Held::new(held, self.authority.usable.clone()))
    }

    pub(crate) async fn keys(&self) -> Arc<DeviceKeys> {
        self.authority.keys.read().await.clone()
    }

    /// Record this device's own JIDs once the client knows them (after pair
    /// success). Sends need them to build own-device copies.
    pub async fn set_own_jids(&self, pn: &str, lid: Option<&str>) -> Result<(), OpsError> {
        let parse = |jid: &str| {
            // The device-key record stores each JID behind a u16 length.
            if u16::try_from(jid.len()).is_err() {
                return Err(OpsError::InvalidInput(format!("{jid} is too long")));
            }
            jid.parse::<Jid>()
                .map_err(|_| OpsError::InvalidInput(format!("{jid} is not a JID")))
        };
        if !parse(pn)?.is_pn() {
            return Err(OpsError::InvalidInput(format!(
                "{pn} is not a phone-number JID"
            )));
        }
        if let Some(lid) = lid
            && !parse(lid)?.is_lid()
        {
            return Err(OpsError::InvalidInput(format!("{lid} is not a LID")));
        }
        let held = self.lock_device().await?;
        held.run(async {
            let mut keys = self.durable_keys().await?;
            keys.own_pn = Some(pn.to_owned());
            keys.own_lid = lid.map(str::to_owned);
            self.replace_keys(keys).await
        })
        .await
    }

    async fn replace_keys(&self, keys: DeviceKeys) -> Result<(), OpsError> {
        let encoded = keys.encode()?;
        if let Err(error) = self.store.save_device_keys(&encoded).await {
            self.refresh_keys().await;
            return Err(error.into());
        }
        *self.authority.keys.write().await = Arc::new(keys);
        Ok(())
    }

    async fn stored_keys(store: &S) -> Result<DeviceKeys, OpsError> {
        DeviceKeys::decode(
            &store
                .load_device_keys()
                .await?
                .ok_or(OpsError::NotInitialised)?,
        )
    }

    /// The device keys as last committed. Read-modify-write starts here, not
    /// from the cache: a write whose response was lost may still have
    /// committed, leaving the cache behind the store, and writing the cached
    /// blob back would rewind it (for example `next_prekey_id`). Call with
    /// the device lock held.
    pub(crate) async fn durable_keys(&self) -> Result<DeviceKeys, OpsError> {
        Self::stored_keys(&self.store).await
    }

    /// After a device-key write with an uncertain outcome, bring the cache in
    /// line with the store. Best effort: on failure the cache stays as it
    /// was, and writers still start from [`Self::durable_keys`].
    pub(crate) async fn refresh_keys(&self) {
        if let Ok(keys) = self.durable_keys().await {
            *self.authority.keys.write().await = Arc::new(keys);
        }
    }

    pub async fn public_identity(&self) -> PublicIdentity {
        self.authority.keys.read().await.public()
    }

    /// Generate fresh one-time prekeys for retry receipts. Unlike
    /// `prepare_prekey_upload`, each call reserves new IDs. The counter and
    /// private keys commit together; active upload IDs remain reserved.
    pub async fn generate_prekeys(
        &self,
        runtime: &dyn crate::runtime::Runtime,
        count: u32,
    ) -> Result<Vec<PublicPreKey>, OpsError> {
        self.generate_fresh_prekeys(runtime, count).await
    }

    /// Mark fresh retry prekeys uploaded. Any original member of the pending
    /// upload batch rejects the entire call; use `acknowledge_prekey_upload`
    /// for that batch. Consumed keys stay gone.
    pub async fn mark_prekeys_uploaded(&self, ids: &[u32]) -> Result<(), OpsError> {
        self.mark_fresh_prekeys_uploaded(ids).await
    }

    /// Replace the signed prekey. The old one stays loadable by ID so prekey
    /// messages minted against it still decrypt.
    pub async fn rotate_signed_prekey(&self) -> Result<PublicSignedPreKey, OpsError> {
        let held = self.lock_device().await?;
        held.run(async {
            let mut keys = self.durable_keys().await?;
            let retired = record_helpers::new_signed_pre_key_record(
                keys.signed_prekey_id,
                &keys.signed_prekey,
                keys.signed_prekey_signature,
                crate::time::now_utc(),
            )?;
            self.store
                .store_signed_prekey(
                    keys.signed_prekey_id,
                    &waproto::codec::signed_pre_key_record_to_vec(&retired),
                )
                .await?;
            let pair = KeyPair::generate(&mut rng());
            keys.signed_prekey_signature = sign_prekey(&keys.identity, &pair)?;
            keys.signed_prekey = pair;
            keys.signed_prekey_id = if keys.signed_prekey_id >= MAX_PREKEY_ID {
                1
            } else {
                keys.signed_prekey_id + 1
            };
            let public = keys.public().signed_prekey;
            self.replace_keys(keys).await?;
            Ok(public)
        })
        .await
    }

    /// Stored session and identity addresses of every device of `user` on
    /// `server`, with their device IDs.
    async fn device_addresses(
        &self,
        user: &str,
        server: Server,
    ) -> Result<Vec<(String, u16)>, OpsError> {
        let mut found = Vec::new();
        for ns in [Namespace::Session, Namespace::Identity] {
            for address in self.store.scan_peer_addresses(ns, user).await? {
                if let Some(device) = device_of(&address, user, server) {
                    found.push((address, device));
                }
            }
        }
        found.sort();
        found.dedup();
        Ok(found)
    }

    /// Drop the stored identity and sessions of every device of `user`, after
    /// the server reports that the user's identity changed. A phone-number or
    /// LID user also covers its hosted devices, as the client does. The next
    /// message either way starts a fresh session. Sender keys are left alone:
    /// they are replaced when the peer distributes a new one.
    pub async fn forget_user(&self, user: &Jid) -> Result<(), OpsError> {
        let account = self.lock_keys(vec![account_lock(&user.user)]).await?;
        account
            .run(async {
                // Scanned under the account lock, so no session setup for this
                // user is still in flight.
                let mut addresses = Vec::new();
                for server in with_hosted(user.server) {
                    for (address, _) in self.device_addresses(&user.user, server).await? {
                        addresses.push(address);
                    }
                }
                addresses.dedup();
                if addresses.is_empty() {
                    return Ok(());
                }
                let held = self
                    .lock_keys(addresses.iter().map(|a| format!("s:{a}")).collect())
                    .await?;
                held.run(async {
                    let batch: Vec<Arc<str>> =
                        addresses.iter().map(|a| Arc::from(a.as_str())).collect();
                    let staged = self.store.staged();
                    staged.delete_sessions_batch(&batch).await?;
                    staged.delete_identities_batch(&batch).await?;
                    staged.commit().await?;
                    Ok(())
                })
                .await
            })
            .await
    }

    /// Move a user's sessions and identities from phone-number addresses to
    /// LID addresses once the client learns the mapping, for regular and
    /// hosted devices. On a conflict the phone-number session wins (it is the
    /// one the peer has been using) and an existing LID identity wins. Sender
    /// keys are not moved; WA Web keeps them where they are. When a device is
    /// stored under two address forms, the one wacore writes wins and the other
    /// is dropped. Returns how many target addresses changed.
    pub async fn migrate_pn_to_lid(&self, pn: &Jid, lid: &Jid) -> Result<usize, OpsError> {
        if !pn.is_pn() || !lid.is_lid() {
            return Err(OpsError::InvalidInput(
                "migrate_pn_to_lid needs a phone-number JID and a LID".into(),
            ));
        }
        // Both accounts first, then the scan, so no session for either is
        // being set up while this decides what to move.
        let accounts = self
            .lock_keys(vec![account_lock(&pn.user), account_lock(&lid.user)])
            .await?;
        accounts
            .run(async {
                let mut moves = Vec::new();
                for (from, to) in [
                    (Server::Pn, Server::Lid),
                    (Server::Hosted, Server::HostedLid),
                ] {
                    for (address, device) in self.device_addresses(&pn.user, from).await? {
                        let target = Jid::new(lid.user.clone(), to).with_device(device);
                        // The form wacore itself writes for this device; a
                        // plain libsignal name for the same device is the
                        // fallback.
                        let canonical = Jid::new(pn.user.clone(), from)
                            .with_device(device)
                            .to_protocol_address();
                        let fallback = address != canonical.as_str();
                        moves.push((
                            target.to_protocol_address().as_str().to_owned(),
                            fallback,
                            address,
                        ));
                    }
                }
                // Two stored forms can name one device. Group them by target,
                // canonical form first, so one source decides the target.
                moves.sort();
                let moves: Vec<(String, String)> =
                    moves.into_iter().map(|(to, _, from)| (from, to)).collect();
                let held = self
                    .lock_keys(
                        moves
                            .iter()
                            .flat_map(|(from, to)| [format!("s:{from}"), format!("s:{to}")])
                            .collect(),
                    )
                    .await?;
                held.run(async {
                    let staged = self.store.staged();
                    let mut moved = std::collections::HashSet::new();
                    // Targets whose session this migration already wrote. A
                    // second source for the same device is dropped, not
                    // copied over it, so the session and identity that land
                    // at the target come from one source.
                    let mut decided = std::collections::HashSet::new();
                    for (from, to) in &moves {
                        let first = !decided.contains(to);
                        if let Some(session) = staged.get_session(from).await? {
                            if first {
                                staged.put_session(to, &session).await?;
                            }
                            staged.delete_session(from).await?;
                            moved.insert(to.clone());
                        }
                        if let Some(identity) = staged.load_identity(from).await? {
                            if first && staged.load_identity(to).await?.is_none() {
                                staged.put_identity(to, identity).await?;
                            }
                            staged.delete_identity(from).await?;
                            moved.insert(to.clone());
                        }
                        if moved.contains(to) {
                            decided.insert(to.clone());
                        }
                    }
                    let moved = moved.len();
                    // The account's pin moves with it, even with no session to
                    // move. An existing LID pin wins: it was pinned or accepted
                    // for that address directly.
                    let (pn_account, lid_account) =
                        (pn.to_non_ad().to_string(), lid.to_non_ad().to_string());
                    if let Some(pin) = staged
                        .load_aux(Namespace::AccountIdentity, &pn_account)
                        .await?
                        && staged
                            .load_aux(Namespace::AccountIdentity, &lid_account)
                            .await?
                            .is_none()
                    {
                        staged
                            .insert_aux(Namespace::AccountIdentity, &lid_account, &pin)
                            .await?;
                    }
                    staged.commit().await?;
                    Ok(moved)
                })
                .await
            })
            .await
    }

    pub async fn has_session(&self, address: &ProtocolAddress) -> Result<bool, OpsError> {
        Ok(self.store.has_session(address.as_str()).await?)
    }

    /// Run X3DH against a fetched bundle so [`Self::encrypt`] can produce a
    /// `pkmsg` for that device.
    pub async fn establish_session(
        &self,
        address: &ProtocolAddress,
        bundle: &RemoteBundle,
    ) -> Result<(), OpsError> {
        let user = crate::signal::trust::device_of(address)?.user;
        let held = self
            .lock_keys(vec![session_key(address), account_lock(&user)])
            .await?;
        held.run(async {
            let staged = Arc::new(self.store.staged());
            self.establish_into(&staged, address, bundle).await?;
            staged.commit().await?;
            Ok(())
        })
        .await
    }

    /// X3DH into a staged view without committing it. The caller holds the
    /// session lock and commits.
    pub(crate) async fn establish_into(
        &self,
        staged: &Arc<S>,
        address: &ProtocolAddress,
        bundle: &RemoteBundle,
    ) -> Result<(), OpsError> {
        let device = crate::signal::trust::device_of(address)?;
        let device_identity = bundle.device_identity.as_deref();
        let bundle = bundle.to_prekey_bundle()?;
        self.verify_bundle(staged, &device, &bundle, device_identity, None)
            .await?
            .map_err(OpsError::BundleRefused)?;
        let mut sessions = self.stores_over(staged.clone()).await;
        let mut identities = sessions.clone();
        process_prekey_bundle(
            address,
            &mut sessions,
            &mut identities,
            &bundle,
            &mut rng(),
            UsePQRatchet::No,
        )
        .await?;
        Ok(())
    }

    /// Decrypt one pairwise `<enc>` payload. The session update and the
    /// consumed prekey's removal commit together.
    pub async fn decrypt(
        &self,
        sender: &ProtocolAddress,
        kind: EncKind,
        ciphertext: &[u8],
        device_identity: Option<&[u8]>,
    ) -> Result<Decrypted, OpsError> {
        let user = crate::signal::trust::device_of(sender)?.user;
        let held = self
            .lock_keys(vec![session_key(sender), account_lock(&user)])
            .await?;
        held.run(async {
            let staged = Arc::new(self.store.staged());
            let decrypted = self
                .decrypt_into(&staged, sender, kind, ciphertext, device_identity)
                .await?;
            staged.commit().await?;
            Ok(decrypted)
        })
        .await
    }

    /// Decrypt into a staged view without committing it. The caller holds the
    /// address lock and commits.
    /// `device_identity` is the `<device-identity>` sent with a prekey message
    /// from a companion device; a prekey message would start a session, so its
    /// sender's identity is verified first, as a fetched bundle is.
    pub(crate) async fn decrypt_into(
        &self,
        staged: &Arc<S>,
        sender: &ProtocolAddress,
        kind: EncKind,
        ciphertext: &[u8],
        device_identity: Option<&[u8]>,
    ) -> Result<Decrypted, OpsError> {
        if kind == EncKind::PreKey {
            let identity = *PreKeySignalMessage::try_from(ciphertext)?
                .identity_key()
                .public_key()
                .public_key_bytes();
            let device = crate::signal::trust::device_of(sender)?;
            self.verify_identity(staged, &device, identity, device_identity, None)
                .await?
                .map_err(OpsError::BundleRefused)?;
        }
        let mut sessions = self.stores_over(staged.clone()).await;
        let mut identities = sessions.clone();
        // libsignal reports a replaced identity for a prekey message but not
        // for an ordinary `msg` that commits a session with another identity,
        // so compare the stored identity around the decrypt as well.
        let before =
            crate::libsignal::protocol::IdentityKeyStore::get_identity(&identities, sender).await?;
        let result = match kind {
            EncKind::PreKey => {
                let message = PreKeySignalMessage::try_from(ciphertext)?;
                let mut prekeys = sessions.clone();
                let signed_prekeys = sessions.clone();
                message_decrypt_prekey(
                    &message,
                    sender,
                    &mut sessions,
                    &mut identities,
                    &mut prekeys,
                    &signed_prekeys,
                    &mut rng(),
                    UsePQRatchet::No,
                )
                .await?
            }
            EncKind::Message => {
                let message = SignalMessage::try_from(ciphertext)?;
                message_decrypt_signal(&message, sender, &mut sessions, &mut identities, &mut rng())
                    .await?
            }
        };
        // Staged with the promoted session, so the prekey is never gone while
        // the session that consumed it is not durable.
        if let Some(id) = result.consumed_prekey_id {
            staged.remove_prekey(id.into()).await?;
        }
        let after =
            crate::libsignal::protocol::IdentityKeyStore::get_identity(&identities, sender).await?;
        Ok(Decrypted {
            plaintext: result.plaintext,
            identity_changed: matches!(
                result.identity_change,
                crate::libsignal::protocol::IdentityChange::ReplacedExisting
            ) || before
                .is_some_and(|before| after.is_some_and(|after| before != after)),
        })
    }

    /// Encrypt padded plaintext for one device with an existing session.
    pub async fn encrypt(
        &self,
        recipient: &ProtocolAddress,
        plaintext: &[u8],
    ) -> Result<Encrypted, OpsError> {
        let held = self.lock_keys(vec![session_key(recipient)]).await?;
        held.run(async {
            let mut sessions = self.stores().await;
            let mut identities = sessions.clone();
            let message =
                message_encrypt(plaintext, recipient, &mut sessions, &mut identities).await?;
            let kind = match message.message_type() {
                CiphertextMessageType::PreKey => EncKind::PreKey,
                CiphertextMessageType::Whisper => EncKind::Message,
                other => {
                    return Err(OpsError::InvalidInput(format!(
                        "unexpected pairwise ciphertext type {other:?}"
                    )));
                }
            };
            Ok(Encrypted {
                kind,
                ciphertext: message.serialize().to_vec(),
            })
        })
        .await
    }

    /// The sender-key distribution message for our own chain in `group`,
    /// creating the chain if needed. Encrypt it pairwise to each member device.
    pub async fn sender_key_distribution(
        &self,
        group: &str,
        own_address: &ProtocolAddress,
    ) -> Result<Vec<u8>, OpsError> {
        let name = SenderKeyName::from_parts(group, own_address.as_str());
        let held = self
            .lock_keys(vec![chain_key(group, own_address.as_str())])
            .await?;
        held.run(async {
            let mut store = self.stores().await;
            let skdm =
                create_sender_key_distribution_message(&name, &mut store, &mut rng()).await?;
            Ok(skdm.serialized().to_vec())
        })
        .await
    }

    /// Encrypt group plaintext (`skmsg`) on our own chain.
    pub async fn group_encrypt(
        &self,
        group: &str,
        own_address: &ProtocolAddress,
        plaintext: &[u8],
    ) -> Result<Vec<u8>, OpsError> {
        let name = SenderKeyName::from_parts(group, own_address.as_str());
        let held = self
            .lock_keys(vec![chain_key(group, own_address.as_str())])
            .await?;
        held.run(async {
            let mut store = self.stores().await;
            let message = group_encrypt(&mut store, &name, plaintext, &mut rng()).await?;
            Ok(message.serialized().to_vec())
        })
        .await
    }

    /// Store a member's sender key received in a decrypted message.
    pub async fn process_sender_key_distribution(
        &self,
        group: &str,
        sender: &ProtocolAddress,
        skdm: &[u8],
    ) -> Result<(), OpsError> {
        let name = SenderKeyName::from_parts(group, sender.as_str());
        let message = SenderKeyDistributionMessage::try_from(skdm)?;
        let held = self
            .lock_keys(vec![chain_key(group, sender.as_str())])
            .await?;
        held.run(async {
            let mut store = self.stores().await;
            process_sender_key_distribution_message(&name, &message, &mut store).await?;
            Ok(())
        })
        .await
    }

    /// Decrypt a group `skmsg` into a staged view without committing it.
    pub(crate) async fn group_decrypt_into(
        &self,
        staged: &Arc<S>,
        group: &str,
        sender: &ProtocolAddress,
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, OpsError> {
        let name = SenderKeyName::from_parts(group, sender.as_str());
        let mut store = self.stores_over(staged.clone()).await;
        Ok(group_decrypt(ciphertext, &mut store, &name).await?)
    }

    /// Store a received sender key in a staged view without committing it.
    pub(crate) async fn process_sender_key_distribution_into(
        &self,
        staged: &Arc<S>,
        group: &str,
        sender: &ProtocolAddress,
        skdm: &[u8],
    ) -> Result<(), OpsError> {
        let name = SenderKeyName::from_parts(group, sender.as_str());
        let message = SenderKeyDistributionMessage::try_from(skdm)?;
        let mut store = self.stores_over(staged.clone()).await;
        process_sender_key_distribution_message(&name, &message, &mut store).await?;
        Ok(())
    }

    /// Decrypt a group `skmsg` from `sender`.
    pub async fn group_decrypt(
        &self,
        group: &str,
        sender: &ProtocolAddress,
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, OpsError> {
        let name = SenderKeyName::from_parts(group, sender.as_str());
        let held = self
            .lock_keys(vec![chain_key(group, sender.as_str())])
            .await?;
        held.run(async {
            let mut store = self.stores().await;
            Ok(group_decrypt(ciphertext, &mut store, &name).await?)
        })
        .await
    }

    /// Verify the primary's pair-success container and add our device
    /// signature. Signs only the ADV device-identity message built here, never
    /// caller-chosen bytes.
    pub async fn sign_pairing(
        &self,
        device_identity_container: &[u8],
    ) -> Result<PairingSignature, OpsError> {
        let keys = self.authority.keys.read().await.clone();
        let (signed_identity, key_index) = PairUtils::do_pair_crypto_with(
            &keys.identity,
            &keys.adv_secret,
            device_identity_container,
        )
        .map_err(|e| OpsError::Pairing {
            code: e.code,
            text: e.text,
        })?;
        // Kept for prekey messages, which carry the device identity.
        let held = self.lock_device().await?;
        held.run(async {
            let mut updated = self.durable_keys().await?;
            updated.account = Some(signed_identity.clone());
            self.replace_keys(updated).await?;
            Ok(PairingSignature {
                signed_identity,
                key_index,
            })
        })
        .await
    }
}

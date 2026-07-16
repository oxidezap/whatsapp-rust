use super::*;

use std::collections::{HashSet, VecDeque};
use std::time::Duration;

use anyhow::{Context, ensure};
use rand::{RngExt, SeedableRng};

use crate::libsignal::protocol::{
    ChainKey, CiphertextMessageType, IdentityKey, KeyPair, RootKey, SenderKeyMessage,
    SenderKeyStore, SessionState, SignalProtocolError, create_sender_key_distribution_message,
    group_decrypt, group_encrypt, process_sender_key_distribution_message,
};
use crate::store::in_memory::InMemoryBackend;

const SMOKE_SEEDS: usize = 4;
const SMOKE_STEPS: usize = 64;
const NIGHTLY_SEEDS: usize = 128;
const NIGHTLY_STEPS: usize = 256;
const DEFAULT_SEED: u64 = 0x51A6_DA7A_B1E5_0001;

type DmFingerprint = ([u8; 32], [u8; 16]);
type GroupFingerprint = (u32, u32);

struct CachedSenderKeyStore<'a> {
    cache: &'a SignalStoreCache,
    backend: &'a InMemoryBackend,
}

#[async_trait::async_trait]
impl SenderKeyStore for CachedSenderKeyStore<'_> {
    async fn store_sender_key(
        &mut self,
        name: &SenderKeyName,
        record: SenderKeyRecord,
    ) -> crate::libsignal::protocol::error::Result<()> {
        self.cache.put_sender_key(name, record).await;
        Ok(())
    }

    async fn load_sender_key(
        &self,
        name: &SenderKeyName,
    ) -> crate::libsignal::protocol::error::Result<Option<SenderKeyRecord>> {
        Ok(self
            .cache
            .get_sender_key(name, self.backend)
            .await
            .map_err(|error| {
                crate::libsignal::protocol::SignalProtocolError::BackendError(
                    "durability chaos store",
                    error.into(),
                )
            })?
            .map(|record| (*record).clone()))
    }
}

#[derive(Clone, Copy, Debug)]
enum Action {
    DmSend { fail_gate: bool },
    GroupSend { fail_gate: bool },
    DmCancel,
    DeliverGroup { newest: bool },
    Flush,
    FailPendingFlush,
    CleanReload,
    CrashReload,
    LossyClear,
    DeleteDm,
    DeleteGroup,
    CheckoutDuringFlush,
}

#[derive(Clone, Copy)]
enum FlushFailure {
    None,
    Session,
    SenderKey,
}

struct SplitMix64(u64);

impl SplitMix64 {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^ (value >> 31)
    }

    fn action(&mut self) -> Action {
        match self.next() % 24 {
            0 => Action::DmSend { fail_gate: true },
            1..=6 => Action::DmSend { fail_gate: false },
            7 => Action::GroupSend { fail_gate: true },
            8..=13 => Action::GroupSend { fail_gate: false },
            14 => Action::DmCancel,
            15 => Action::DeliverGroup {
                newest: self.next() & 1 == 0,
            },
            16 => Action::Flush,
            17 => Action::FailPendingFlush,
            18 => Action::CleanReload,
            19 => Action::CrashReload,
            20 => Action::LossyClear,
            21 => Action::DeleteDm,
            22 => Action::DeleteGroup,
            23 => Action::CheckoutDuringFlush,
            _ => unreachable!(),
        }
    }
}

struct ChaosHarness {
    backend: InMemoryBackend,
    cache: SignalStoreCache,
    receiver_backend: InMemoryBackend,
    receiver_cache: SignalStoreCache,
    dm_address: ProtocolAddress,
    group_name: SenderKeyName,
    crypto_rng: rand::rngs::StdRng,
    incarnation_generation: u64,
    published_dm: HashSet<DmFingerprint>,
    published_group: HashSet<GroupFingerprint>,
    pending_group: VecDeque<SenderKeyMessage>,
}

impl ChaosHarness {
    async fn new(seed: u64) -> anyhow::Result<Self> {
        let mut harness = Self {
            backend: InMemoryBackend::new(),
            cache: SignalStoreCache::with_max_entries_and_incarnation(32, incarnation(seed, 0)),
            receiver_backend: InMemoryBackend::new(),
            receiver_cache: SignalStoreCache::with_max_entries_and_incarnation(
                32,
                incarnation(seed ^ 0xA5A5_A5A5_A5A5_A5A5, 0),
            ),
            dm_address: ProtocolAddress::new("15550007001".to_string(), 1.into()),
            group_name: SenderKeyName::from_parts(
                "120363000000070001@g.us",
                "15550007002@s.whatsapp.net:0",
            ),
            crypto_rng: rand::rngs::StdRng::seed_from_u64(seed ^ 0xC4A5_5EED_5AFE_0001),
            incarnation_generation: 0,
            published_dm: HashSet::new(),
            published_group: HashSet::new(),
            pending_group: VecDeque::new(),
        };

        ensure!(harness.dm_send(false).await?, "initial DM send was gated");
        harness.sync_group_distribution().await?;
        ensure!(
            harness.group_send(false).await?,
            "initial group send was gated"
        );
        harness.deliver_group(false).await?;
        harness.assert_invariants().await?;
        Ok(harness)
    }

    async fn apply(&mut self, action: Action) -> anyhow::Result<()> {
        match action {
            Action::DmSend { fail_gate } => {
                self.dm_send(fail_gate).await?;
            }
            Action::GroupSend { fail_gate } => {
                self.group_send(fail_gate).await?;
            }
            Action::DmCancel => self.dm_cancel().await?,
            Action::DeliverGroup { newest } => self.deliver_group(newest).await?,
            Action::Flush => self.flush_successfully().await?,
            Action::FailPendingFlush => self.fail_pending_flush().await?,
            Action::CleanReload => self.clean_reload().await?,
            Action::CrashReload => self.crash_reload(),
            Action::LossyClear => self.lossy_clear().await?,
            Action::DeleteDm => self.cache.delete_session(&self.dm_address).await,
            Action::DeleteGroup => {
                self.cache
                    .delete_sender_key(self.group_name.cache_key())
                    .await;
            }
            Action::CheckoutDuringFlush => self.checkout_during_flush().await?,
        }
        self.assert_invariants().await
    }

    async fn dm_send(&mut self, fail_gate: bool) -> anyhow::Result<bool> {
        let (record, checkout) = self
            .cache
            .checkout_session(&self.dm_address, &self.backend)
            .await?;
        let had_session = record.is_some();
        let mut record = record.unwrap_or_else(|| fresh_session(&mut self.crypto_rng));
        let chain = record
            .session_state()
            .context("DM session state missing")?
            .get_sender_chain_key()
            .map_err(|_| anyhow::anyhow!("DM sender chain missing"))?;
        let keys = chain.message_keys().generate_keys();
        let fingerprint = (*keys.cipher_key(), *keys.iv());
        let next = chain.next_chain_key()?;
        record
            .session_state_mut()
            .context("DM session state missing")?
            .set_sender_chain_key(&next)
            .map_err(|_| anyhow::anyhow!("DM sender chain update failed"))?;
        if chain.index() >= record.reserved_sender_chain_index() {
            record.reserve_sender_chain_counters(chain.index());
        }
        self.commit_dm(record, checkout, had_session).await?;

        let published = self
            .release_wire_gate(if fail_gate {
                FlushFailure::Session
            } else {
                FlushFailure::None
            })
            .await?;
        if published {
            ensure!(
                self.published_dm.insert(fingerprint),
                "DM key/IV was published twice at counter {}",
                chain.index()
            );
        }
        Ok(published)
    }

    async fn dm_cancel(&mut self) -> anyhow::Result<()> {
        let (record, checkout) = self
            .cache
            .checkout_session(&self.dm_address, &self.backend)
            .await?;
        let Some(mut record) = record else {
            self.cache
                .cancel_session_checkout(&self.dm_address, checkout);
            return Ok(());
        };
        let chain = record
            .session_state()
            .context("DM session state missing")?
            .get_sender_chain_key()
            .map_err(|_| anyhow::anyhow!("DM sender chain missing"))?;
        let next = chain.next_chain_key()?;
        record
            .session_state_mut()
            .context("DM session state missing")?
            .set_sender_chain_key(&next)
            .map_err(|_| anyhow::anyhow!("DM sender chain update failed"))?;
        if chain.index() >= record.reserved_sender_chain_index() {
            record.reserve_sender_chain_counters(chain.index());
        }
        self.commit_dm(record, checkout, true).await
    }

    async fn commit_dm(
        &self,
        record: SessionRecord,
        checkout: SessionCheckoutKey,
        had_session: bool,
    ) -> anyhow::Result<()> {
        match self.cache.restore_session_from_checkout(
            &self.dm_address,
            record,
            checkout,
            had_session,
        ) {
            SessionCheckoutStoreResult::Stored => Ok(()),
            SessionCheckoutStoreResult::Pending(completion) => {
                self.cache.complete_session_checkout().await;
                ensure!(
                    completion.load(Ordering::Acquire),
                    "queued DM checkout restore was rejected"
                );
                Ok(())
            }
            SessionCheckoutStoreResult::Rejected => {
                anyhow::bail!("DM checkout restore was rejected")
            }
            SessionCheckoutStoreResult::Unhandled(_) => {
                anyhow::bail!("cache did not handle a DM checkout restore")
            }
        }
    }

    async fn group_send(&mut self, fail_gate: bool) -> anyhow::Result<bool> {
        if self
            .cache
            .get_sender_key(&self.group_name, &self.backend)
            .await?
            .is_none()
        {
            self.sync_group_distribution().await?;
        }
        let message = {
            let mut sender = CachedSenderKeyStore {
                cache: &self.cache,
                backend: &self.backend,
            };
            group_encrypt(
                &mut sender,
                &self.group_name,
                b"durability-chaos",
                &mut self.crypto_rng,
            )
            .await?
        };
        let fingerprint = (message.chain_id(), message.iteration());
        let published = self
            .release_wire_gate(if fail_gate {
                FlushFailure::SenderKey
            } else {
                FlushFailure::None
            })
            .await?;
        if published {
            ensure!(
                self.published_group.insert(fingerprint),
                "group chain/iteration was published twice: {fingerprint:?}"
            );
            self.pending_group.push_back(message);
            if self.pending_group.len() > 64 {
                self.pending_group.pop_front();
            }
        }
        Ok(published)
    }

    async fn sync_group_distribution(&mut self) -> anyhow::Result<()> {
        let distribution = {
            let mut sender = CachedSenderKeyStore {
                cache: &self.cache,
                backend: &self.backend,
            };
            create_sender_key_distribution_message(
                &self.group_name,
                &mut sender,
                &mut self.crypto_rng,
            )
            .await?
        };
        let mut receiver = CachedSenderKeyStore {
            cache: &self.receiver_cache,
            backend: &self.receiver_backend,
        };
        process_sender_key_distribution_message(&self.group_name, &distribution, &mut receiver)
            .await?;
        self.pending_group.clear();
        Ok(())
    }

    async fn deliver_group(&mut self, newest: bool) -> anyhow::Result<()> {
        let Some(message) = (if newest {
            self.pending_group.pop_back()
        } else {
            self.pending_group.pop_front()
        }) else {
            return Ok(());
        };
        let mut receiver = CachedSenderKeyStore {
            cache: &self.receiver_cache,
            backend: &self.receiver_backend,
        };
        match group_decrypt(message.serialized(), &mut receiver, &self.group_name).await {
            Ok(_) => return Ok(()),
            Err(SignalProtocolError::InvalidMessage(
                CiphertextMessageType::SenderKey,
                "message from too far into the future",
            ))
            | Err(SignalProtocolError::NoSenderKeyState(_)) => {}
            Err(error) => return Err(error.into()),
        }

        self.sync_group_distribution().await?;
        ensure!(
            self.group_send(false).await?,
            "retry group message remained behind a durability gate"
        );
        let retry = self
            .pending_group
            .pop_back()
            .context("group retry message missing")?;
        let mut receiver = CachedSenderKeyStore {
            cache: &self.receiver_cache,
            backend: &self.receiver_backend,
        };
        let plaintext = group_decrypt(retry.serialized(), &mut receiver, &self.group_name).await?;
        ensure!(
            plaintext == b"durability-chaos",
            "group retry decrypted the wrong plaintext"
        );
        Ok(())
    }

    async fn release_wire_gate(&self, failure: FlushFailure) -> anyhow::Result<bool> {
        if !self.cache.needs_pre_wire_flush().await {
            return Ok(true);
        }
        self.backend
            .set_fail_session_writes(matches!(failure, FlushFailure::Session));
        self.backend
            .set_fail_sender_key_writes(matches!(failure, FlushFailure::SenderKey));
        let result = self.cache.flush(&self.backend).await;
        self.backend.set_fail_session_writes(false);
        self.backend.set_fail_sender_key_writes(false);

        if matches!(failure, FlushFailure::None) {
            result?;
            ensure!(
                !self.cache.needs_pre_wire_flush().await,
                "successful pre-wire flush left a gate pending"
            );
            return Ok(true);
        }
        ensure!(result.is_err(), "injected pre-wire flush did not fail");
        ensure!(
            self.cache.needs_pre_wire_flush().await,
            "failed pre-wire flush released its gate"
        );
        Ok(false)
    }

    async fn fail_pending_flush(&self) -> anyhow::Result<()> {
        let session_pending = !self
            .cache
            .lock_sessions()
            .await
            .reservation_pending
            .is_empty();
        let sender_pending = !self
            .cache
            .sender_keys
            .lock()
            .await
            .wire_gate_pending
            .is_empty();
        let failure = if session_pending {
            FlushFailure::Session
        } else if sender_pending {
            FlushFailure::SenderKey
        } else {
            return Ok(());
        };
        ensure!(
            !self.release_wire_gate(failure).await?,
            "injected flush unexpectedly released the wire"
        );
        Ok(())
    }

    async fn flush_successfully(&self) -> anyhow::Result<()> {
        self.cache.flush(&self.backend).await?;
        ensure!(
            !self.cache.needs_pre_wire_flush().await,
            "successful background flush left a gate pending"
        );
        Ok(())
    }

    async fn clean_reload(&self) -> anyhow::Result<()> {
        let dm_before = self
            .cache
            .peek_session(&self.dm_address, &self.backend)
            .await?
            .map(|record| dm_chain_index(&record))
            .transpose()?;
        let group_before = self
            .cache
            .get_sender_key(&self.group_name, &self.backend)
            .await?
            .map(|record| group_position(&record))
            .transpose()?;

        self.flush_successfully().await?;
        self.cache.clear_after_flush().await;

        let dm_after = self
            .cache
            .peek_session(&self.dm_address, &self.backend)
            .await?
            .map(|record| dm_chain_index(&record))
            .transpose()?;
        let group_after = self
            .cache
            .get_sender_key(&self.group_name, &self.backend)
            .await?
            .map(|record| group_position(&record))
            .transpose()?;
        ensure!(dm_after == dm_before, "clean DM reload burned a lease");
        ensure!(
            group_after == group_before,
            "clean group reload burned a lease"
        );
        Ok(())
    }

    fn crash_reload(&mut self) {
        self.incarnation_generation = self.incarnation_generation.wrapping_add(1);
        self.cache = SignalStoreCache::with_max_entries_and_incarnation(
            32,
            incarnation(DEFAULT_SEED, self.incarnation_generation),
        );
    }

    async fn lossy_clear(&mut self) -> anyhow::Result<()> {
        let (record, checkout) = self
            .cache
            .checkout_session(&self.dm_address, &self.backend)
            .await?;
        self.incarnation_generation = self.incarnation_generation.wrapping_add(1);
        self.cache
            .clear_with_incarnation(incarnation(
                DEFAULT_SEED ^ 0xFFFF_0000_FFFF_0000,
                self.incarnation_generation,
            ))
            .await;
        if let Some(record) = record {
            ensure!(
                matches!(
                    self.cache.restore_session_from_checkout(
                        &self.dm_address,
                        record,
                        checkout,
                        true,
                    ),
                    SessionCheckoutStoreResult::Rejected
                ),
                "lossy clear accepted a stale checkout"
            );
        } else {
            self.cache
                .cancel_session_checkout(&self.dm_address, checkout);
        }
        ensure!(
            !self.cache.needs_pre_wire_flush().await,
            "lossy clear retained a stale wire gate"
        );
        Ok(())
    }

    async fn checkout_during_flush(&self) -> anyhow::Result<()> {
        let (record, checkout) = self
            .cache
            .checkout_session(&self.dm_address, &self.backend)
            .await?;
        let dm_was_gated = self
            .cache
            .lock_sessions()
            .await
            .reservation_pending
            .contains(self.dm_address.as_str());
        self.cache.flush(&self.backend).await?;
        match record {
            Some(record) => self.commit_dm(record, checkout, true).await?,
            None => self
                .cache
                .cancel_session_checkout(&self.dm_address, checkout),
        }
        if dm_was_gated {
            ensure!(
                self.cache.needs_pre_wire_flush().await,
                "flush released a checked-out session gate"
            );
        }
        Ok(())
    }

    async fn assert_invariants(&self) -> anyhow::Result<()> {
        let sessions = self.cache.lock_sessions().await;
        ensure!(
            sessions
                .reservation_pending
                .iter()
                .all(|key| sessions.dirty.contains(key) || sessions.deleted.contains(key)),
            "DM gate escaped dirty/tombstone tracking"
        );
        ensure!(
            sessions
                .dirty
                .iter()
                .all(|key| sessions.cache.contains_key(key.as_ref())),
            "dirty DM session was evicted"
        );
        ensure!(
            sessions
                .cache
                .values()
                .all(|entry| !matches!(entry, SessionEntry::CheckedOut { .. })),
            "DM checkout remained stranded"
        );
        let session_gate = !sessions.reservation_pending.is_empty();
        drop(sessions);

        ensure!(
            self.cache.pending_session_restores().is_empty(),
            "queued DM restore remained undrained"
        );
        let sender_keys = self.cache.sender_keys.lock().await;
        ensure!(
            sender_keys
                .wire_gate_pending
                .iter()
                .all(|key| sender_keys.dirty.contains(key)),
            "group gate escaped dirty tracking"
        );
        ensure!(
            sender_keys
                .dirty
                .iter()
                .all(|key| sender_keys.cache.contains_key(key.as_ref())),
            "dirty sender key was evicted"
        );
        let sender_gate = !sender_keys.wire_gate_pending.is_empty();
        drop(sender_keys);
        ensure!(
            self.cache.needs_pre_wire_flush().await == (session_gate || sender_gate),
            "wire-gate query disagrees with cache state"
        );
        Ok(())
    }
}

fn fresh_session(rng: &mut rand::rngs::StdRng) -> SessionRecord {
    let local = IdentityKey::new(KeyPair::generate(rng).public_key);
    let remote = IdentityKey::new(KeyPair::generate(rng).public_key);
    let base_key = KeyPair::generate(rng).public_key;
    let mut root = [0; 32];
    let mut chain = [0; 32];
    rng.fill(&mut root);
    rng.fill(&mut chain);
    let mut state = SessionState::new(3, &local, &remote, &RootKey::new(root), &base_key);
    state.set_sender_chain(&KeyPair::generate(rng), &ChainKey::new(chain, 0));
    SessionRecord::new(state)
}

fn dm_chain_index(record: &SessionRecord) -> anyhow::Result<u32> {
    Ok(record
        .session_state()
        .context("DM session state missing")?
        .get_sender_chain_key()
        .map_err(|_| anyhow::anyhow!("DM sender chain missing"))?
        .index())
}

fn group_position(record: &SenderKeyRecord) -> anyhow::Result<(u32, u32)> {
    let state = record
        .sender_key_state()
        .map_err(|_| anyhow::anyhow!("group sender-key state missing"))?;
    Ok((
        state.chain_id(),
        state
            .sender_chain_key()
            .context("group sender chain missing")?
            .iteration(),
    ))
}

fn incarnation(seed: u64, generation: u64) -> StoreIncarnation {
    let mut rng = SplitMix64(seed ^ generation.wrapping_mul(0xD134_2543_DE82_EF95));
    let mut value = [0; 16];
    value[..8].copy_from_slice(&rng.next().to_le_bytes());
    value[8..].copy_from_slice(&rng.next().to_le_bytes());
    value
}

fn env_u64(name: &str, default: u64) -> u64 {
    let Some(value) = std::env::var(name).ok() else {
        return default;
    };
    if let Some(hex) = value.strip_prefix("0x") {
        u64::from_str_radix(hex, 16).unwrap_or(default)
    } else {
        value.parse().unwrap_or(default)
    }
}

fn env_usize(name: &str, default: usize) -> usize {
    std::env::var(name)
        .ok()
        .and_then(|value| value.parse().ok())
        .unwrap_or(default)
        .clamp(1, 4096)
}

async fn run_seed(seed: u64, steps: usize) -> anyhow::Result<()> {
    let mut harness = tokio::time::timeout(Duration::from_secs(5), ChaosHarness::new(seed))
        .await
        .context("chaos setup timed out")??;
    let mut actions = SplitMix64(seed);
    for step in 0..steps {
        let action = actions.action();
        let result = tokio::time::timeout(Duration::from_secs(2), harness.apply(action))
            .await
            .with_context(|| {
                format!("seed=0x{seed:016x} step={step} action={action:?} timed out")
            })?;
        result.with_context(|| format!("seed=0x{seed:016x} step={step} action={action:?}"))?;
    }
    Ok(())
}

async fn run_matrix(first_seed: u64, seeds: usize, steps: usize) {
    for index in 0..seeds {
        let seed = first_seed.wrapping_add((index as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        if let Err(error) = run_seed(seed, steps).await {
            panic!(
                "Signal durability chaos failed: {error:#}\n\
                 replay with SIGNAL_CHAOS_SEED=0x{seed:016x} SIGNAL_CHAOS_SEEDS=1 \
                 SIGNAL_CHAOS_STEPS={steps}"
            );
        }
    }
}

#[tokio::test]
async fn signal_durability_chaos_smoke() {
    run_matrix(DEFAULT_SEED, SMOKE_SEEDS, SMOKE_STEPS).await;
}

#[tokio::test]
#[ignore = "run by signal-durability-nightly.yml"]
async fn signal_durability_chaos_nightly() {
    let seed = env_u64("SIGNAL_CHAOS_SEED", DEFAULT_SEED);
    let seeds = env_usize("SIGNAL_CHAOS_SEEDS", NIGHTLY_SEEDS);
    let steps = env_usize("SIGNAL_CHAOS_STEPS", NIGHTLY_STEPS);
    run_matrix(seed, seeds, steps).await;
}

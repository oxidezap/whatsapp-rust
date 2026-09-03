//! Offline fixture for the client-level group-send benchmarks.
//!
//! `wacore` can be benchmarked directly because it is pure; the client crate
//! cannot — a send touches the transport, the store, the device registry and
//! the async runtime. This module builds the smallest thing that still runs
//! the *real* [`Client::send_message`] group path end to end with no server:
//!
//! - transport: a sink that drops the bytes the noise socket hands it. The
//!   stanza is still marshalled, framed and encrypted, so everything up to and
//!   including the wire write is measured; only the socket write is elided.
//! - store: [`wacore::store::InMemoryBackend`], so a measurement is not
//!   dominated by SQLite. The SQLite backend's own cost is therefore **not**
//!   covered here.
//! - registry / group metadata: pre-seeded into the in-process caches, as a
//!   client that has already resolved the group holds them. No usync or
//!   group-info IQ is issued, which is also why no IQ response has to be faked.
//!
//! Everything a real client resolves once and holds across sends — the
//! `Arc<GroupInfo>`, the resolved device set, the sender key, the warm marks —
//! is established in [`GroupSendHarness::new`](crate::bench_support::GroupSendHarness::new),
//! outside anything a benchmark measures. That is deliberate: PR #1279 found that building an N-participant
//! `GroupInfo` *inside* the measured body was the entirety of the apparent
//! per-member growth in the `wacore` benchmark. A fixture that scales with the
//! sweep parameter measures the fixture.
//!
//! Only compiled under the non-default `bench-harness` feature.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use crate::Client;
use crate::http::{HttpClient, HttpRequest, HttpResponse};
use crate::runtime_impl::TokioRuntime;
use crate::store::persistence_manager::PersistenceManager;
use crate::transport::{Transport, TransportEvent, TransportFactory};
use wacore::client::context::GroupInfo;
use wacore::proto_helpers::MessageBuilderExt;
use wacore::store::InMemoryBackend;
use wacore::store::commands::DeviceCommand;
use wacore::store::traits::{DeviceInfo, DeviceListRecord};
use wacore::types::message::AddressingMode;
use wacore_binary::{Jid, Server};
use waproto::whatsapp as wa;

/// Group sizes every client-level sweep runs. The claim under test is about
/// *scale*, so a single width could not distinguish "expensive" from "grows".
pub const GROUP_SIZES: &[usize] = &[8, 32, 128, 512];

/// Discards every frame. The noise socket still encrypts and frames the stanza
/// before handing it over, so the send path is complete up to the syscall.
struct SinkTransport;

#[async_trait::async_trait]
impl Transport for SinkTransport {
    async fn send(&self, _data: bytes::Bytes) -> Result<(), anyhow::Error> {
        Ok(())
    }

    async fn disconnect(&self) {}
}

struct SinkTransportFactory;

#[async_trait::async_trait]
impl TransportFactory for SinkTransportFactory {
    async fn create_transport(
        &self,
    ) -> Result<(Arc<dyn Transport>, async_channel::Receiver<TransportEvent>), anyhow::Error> {
        let (_tx, rx) = async_channel::bounded(1);
        Ok((Arc::new(SinkTransport), rx))
    }
}

struct NoopHttpClient;

#[async_trait::async_trait]
impl HttpClient for NoopHttpClient {
    async fn execute(&self, _request: HttpRequest) -> Result<HttpResponse, anyhow::Error> {
        Ok(HttpResponse {
            status_code: 200,
            body: Vec::new(),
        })
    }
}

/// Reserved fictional NANP numbers: `1` + a real NPA + the `555` exchange + a
/// line number in the unassignable `0100`-`0199` block, the format the rest of
/// this repository's fixtures use.
///
/// One NPA only yields 100 such numbers, so the block is spread over several
/// NPAs — enough for the 512-member sweep with room left. Deterministic in the
/// index, so the fixture at 512 is a superset of the same fixture at 8.
fn member_user(index: usize) -> String {
    /// Real area codes, used only to shape the number; the `555` exchange plus
    /// the `0100`-`0199` line block is what makes it unassignable.
    const NPAS: [u16; 8] = [202, 212, 213, 312, 415, 503, 617, 702];
    assert!(
        index < NPAS.len() * 100,
        "the reserved 555-01xx block is exhausted; add another NPA"
    );
    format!("1{}555{:04}", NPAS[index / 100], 100 + index % 100)
}

/// Our own account, from the same reserved block and outside the range
/// [`member_user`] draws from.
const OWN_USER: &str = "19045550100";
const GROUP_JID: &str = "120363000000000042@g.us";

/// A logged-in client holding a resolved N-member group, warmed to the steady
/// state a repeat send actually starts from.
pub struct GroupSendHarness {
    runtime: tokio::runtime::Runtime,
    client: Arc<Client>,
    group: Jid,
    group_jid_str: String,
    /// The very `Arc` the group cache serves the send path, so the harness can
    /// call the memoized resolver with the identity the memo is keyed on.
    group_info: Arc<GroupInfo>,
    own_sending_jid: Jid,
}

impl GroupSendHarness {
    /// Build a client whose group has `group_size` other members (each with a
    /// single device) plus our own primary and one companion, then drive it to
    /// the warm steady state: sender key created, every external device marked
    /// warm, both device memos populated.
    ///
    /// The companion is not incidental. WA Web guards every `markHasSenderKey`
    /// with `!isMeDevice`, so our own companions are never memoized warm and
    /// re-receive an SKDM on every send — the warm steady state is "needs SKDM
    /// for own devices only", not "needs nothing". A harness with no companion
    /// would measure a state production rarely sits in.
    pub fn new(group_size: usize) -> Self {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("current-thread runtime");
        let fixture = runtime.block_on(build_fixture(group_size));
        Self {
            runtime,
            client: fixture.client,
            group_jid_str: fixture.group.to_string(),
            group: fixture.group,
            group_info: fixture.group_info,
            own_sending_jid: fixture.own_sending_jid,
        }
    }

    /// One warm group send, all the way through marshalling and the noise
    /// socket. The client-level counterpart of `wacore`'s `bench_group_send_*`.
    pub fn warm_send(&self) {
        self.runtime.block_on(send_once(&self.client, &self.group))
    }

    /// The client-level SKDM target resolution alone: the per-group device map
    /// load, the memoized device resolution, and (on a memo miss) the
    /// O(devices) `filter_skdm_targets` scan. Isolated from encryption so a
    /// per-member term here is not buried under the Ed25519 signature that
    /// dominates a whole send.
    pub fn resolve_skdm_targets(&self) -> usize {
        self.runtime.block_on(self.resolve())
    }

    /// The same resolution with the warm memo dropped first, so the filter
    /// runs. The delta against [`Self::resolve_skdm_targets`] is what the memo
    /// saves per send.
    pub fn resolve_skdm_targets_memo_cold(&self) -> usize {
        self.runtime.block_on(async {
            self.client.skdm_warm_memo.invalidate(&self.group).await;
            self.resolve().await
        })
    }

    async fn resolve(&self) -> usize {
        self.client
            .resolve_skdm_targets_memoized(
                &self.group,
                &self.group_jid_str,
                &self.group_info,
                &self.own_sending_jid,
            )
            .await
            .map(|(_, needs)| needs.len())
            .expect("target resolution must succeed offline")
    }

    /// Per-term outcomes of the two device memos so far.
    ///
    /// The benchmarks below force their memo outcome, which is what makes them
    /// measurable — and what makes them unable to say which outcome a client
    /// takes once the group is in its steady state. Reading the counters after
    /// a run of [`Self::warm_send`] is how the sweep answers that at group
    /// sizes the unit tests do not reach.
    pub fn memo_stats(&self) -> crate::client::DeviceMemoStats {
        self.client.device_memo_stats()
    }

    /// A signal-cache flush over the cached session set this group produced.
    ///
    /// Repeated calls flush an already-clean cache, which is the interesting
    /// case: it isolates the *walk* from the writes. A flush that iterated the
    /// whole cached set would grow with the group here; one that only visits
    /// what is dirty would not.
    pub fn flush_signal_cache(&self) {
        self.runtime
            .block_on(self.client.flush_signal_cache())
            .expect("offline flush");
    }
}

struct Fixture {
    client: Arc<Client>,
    group: Jid,
    group_info: Arc<GroupInfo>,
    own_sending_jid: Jid,
}

async fn build_fixture(group_size: usize) -> Fixture {
    let backend = Arc::new(InMemoryBackend::new());
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("persistence manager"),
    );
    let (client, _sync_rx) = Client::new(
        Arc::new(TokioRuntime),
        pm,
        Arc::new(SinkTransportFactory),
        Arc::new(NoopHttpClient),
        None,
    )
    .await;

    let own_pn = Jid::new(OWN_USER, Server::Pn);
    let own_lid = Jid::new("100000000000001", Server::Lid);
    client
        .persistence_manager
        .process_command(DeviceCommand::SetId(Some(own_pn.clone())))
        .await;
    client
        .persistence_manager
        .process_command(DeviceCommand::SetLid(Some(own_lid)))
        .await;

    // Registry records, seeded straight into the in-process cache the way a
    // client that already ran usync holds them. `promote` rather than `insert`:
    // this is a cache fill for an answer the registry already has, so it records
    // no topology change and the device memo's generation starts where a warm
    // client's does.
    let mut participants = Vec::with_capacity(group_size);
    for index in 0..group_size {
        let user = member_user(index);
        seed_registry(&client, &user, &[0]).await;
        participants.push(Jid::new(&user, Server::Pn));
    }
    // Our own list carries the primary plus one companion.
    seed_registry(&client, OWN_USER, &[0, 1]).await;

    // Signal sessions for every device the cold send distributes to. Members
    // are only encrypted to by that one cold send — afterwards they are marked
    // warm and never re-targeted — so a one-way X3DH is enough for them.
    for participant in &participants {
        seed_peer_session(&client, participant).await;
    }
    // Our own companion is the exception: it is re-targeted on EVERY send
    // (WA Web `!isMeDevice`), so it is the one session whose state every
    // measured iteration depends on. A one-way `process_prekey_bundle` leaves
    // the initiator holding an unacknowledged pre-key, and `message_encrypt`
    // then emits a `pkmsg` — wrapping the identity key, base key and
    // registration id — forever, because the sink transport can never deliver
    // the reply that clears it. That would measure first contact on every
    // iteration instead of the warm pairwise send. Established both ways, and
    // asserted below, for the same reason `wacore`'s DM fan-out benchmark
    // asserts it.
    let companion = own_pn.with_device(1);
    establish_acknowledged_session(&client, &companion).await;

    let group: Jid = GROUP_JID.parse().expect("group jid");
    // Self is in the participant list, as the server's group metadata has it —
    // you are a member of the group you send to. Leaving it out is not a
    // harmless simplification: `ensure_self_in_group` runs per send and CLONES
    // the whole `GroupInfo` when self is absent, so a self-less fixture charges
    // every send an N-participant copy no real send performs. Measured both
    // ways at 512 members, that artifact alone is 22,836 instructions per send
    // (525,989 self-absent vs 503,153 self-present) — 45 instructions per
    // member, which would have roughly doubled the per-member slope this sweep
    // reports. Same class of error as the one PR #1279 removed from the
    // `wacore` sweep.
    participants.push(own_pn.to_non_ad());
    let group_info = Arc::new(GroupInfo::new(participants, AddressingMode::Pn));
    client
        .get_group_cache()
        .insert(group.clone(), Arc::clone(&group_info))
        .await;

    install_offline_socket(&client).await;

    // Two sends, both outside every measurement: the first is cold (creates the
    // sender key and distributes to all `group_size` members), the second
    // establishes the warm memos. A benchmark measures send three onwards.
    send_once(&client, &group).await;
    send_once(&client, &group).await;

    settle_signal_flush_worker(&client).await;

    Fixture {
        client,
        group,
        group_info,
        own_sending_jid: own_pn,
    }
}

async fn send_once(client: &Arc<Client>, group: &Jid) {
    client
        .send_message(group.clone(), wa::Message::text("bench"))
        .await
        .expect("offline group send must reach the sink");
}

async fn seed_registry(client: &Arc<Client>, user: &str, devices: &[u16]) {
    let user: Arc<str> = Arc::from(user);
    let record = DeviceListRecord {
        user: Arc::clone(&user),
        devices: devices
            .iter()
            .map(|id| DeviceInfo::new(*id, None))
            .collect(),
        timestamp: wacore::time::now_secs(),
        phash: None,
        raw_id: None,
    };
    client
        .device_registry_cache
        .promote(user, Arc::new(record))
        .await;
}

/// Let the coalesced flush worker the warm-up sends armed fire and stand down.
///
/// `persist_signal_state_pre_wire` arms a 25 ms worker on a lease-covered send
/// instead of writing through. This fixture drives everything from `block_on`
/// on a single-threaded runtime, so that worker can only ever run *inside* a
/// later `block_on` — which, once sampling starts, means inside a measured
/// closure. Left armed it would fold an unrelated ~1.9K-instruction cache flush
/// into whichever sample happened to cross the deadline, which on the ~3.9K
/// resolver benchmark is a 50% perturbation and makes results depend on the
/// order benchmarks run in.
///
/// Sends inside `warm_group_send` re-arm it, and that is deliberate: a real
/// client pays exactly that coalesced flush on the send path, so suppressing it
/// there would measure less than a send costs. What must not happen is a worker
/// armed by *setup* landing in a benchmark that never sends.
async fn settle_signal_flush_worker(client: &Arc<Client>) {
    // The worker sleeps one 25 ms window, flushes, then exits when nothing is
    // dirty. Poll rather than sleep a fixed span so a slow instrumented run
    // cannot start sampling with the worker still armed.
    for _ in 0..40 {
        if !client.signal_flush_worker_armed() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    panic!("signal flush worker stayed armed; a measured sample could absorb its flush");
}

/// Establish a pairwise Signal session with `peer`, as a real client does off a
/// prekey bundle fetch. Every key here is generated locally; none is real.
///
/// One-way: the initiator's session keeps its unacknowledged pre-key, so the
/// first message to `peer` is a `pkmsg`. That is correct for a device this
/// fixture only ever encrypts to once, during the cold warm-up send. For the
/// own companion — encrypted to on every measured send — use
/// [`establish_acknowledged_session`] instead.
async fn seed_peer_session(client: &Arc<Client>, peer: &Jid) {
    use wacore::libsignal::protocol::{
        IdentityKeyPair, KeyPair, PreKeyBundle, UsePQRatchet, process_prekey_bundle,
    };
    use wacore::types::jid::JidExt;

    let mut rng = rand::make_rng::<rand::rngs::StdRng>();
    let receiver = IdentityKeyPair::generate(&mut rng);
    let spk = KeyPair::generate(&mut rng);
    let opk = KeyPair::generate(&mut rng);
    let signature = receiver
        .private_key()
        .calculate_signature(&spk.public_key.serialize(), &mut rng)
        .expect("signature");
    let bundle = PreKeyBundle::new(
        1,
        1u32.into(),
        Some((1u32.into(), opk.public_key)),
        1u32.into(),
        spk.public_key,
        signature,
        *receiver.identity_key(),
    )
    .expect("prekey bundle");

    let mut adapter = client.signal_adapter();
    process_prekey_bundle(
        &peer.to_protocol_address(),
        &mut adapter.session_store,
        &mut adapter.identity_store,
        &bundle,
        &mut rng,
        UsePQRatchet::No,
    )
    .await
    .expect("peer session");
}

/// Establish a session with `peer` that is *acknowledged*, so `message_encrypt`
/// emits a plain `msg` rather than a `pkmsg`.
///
/// Backed by a real second client rather than a throwaway key set, because the
/// acknowledgement only comes from the peer actually decrypting our first
/// message and replying: the reply's ratchet step is what clears the initiator's
/// pending pre-key. The send fixture drops the returned peer — it exists to
/// produce that one reply, not to be measured; the receive fixture keeps it as
/// the party that encrypts every measured stanza.
async fn establish_acknowledged_session(client: &Arc<Client>, peer: &Jid) -> Arc<Client> {
    use wacore::libsignal::protocol::{
        CiphertextMessage, IdentityKey, PreKeyBundle, PreKeySignalMessage, SignalMessage,
        UsePQRatchet, message_decrypt, message_encrypt, process_prekey_bundle,
    };
    use wacore::types::jid::JidExt;

    let peer_backend = Arc::new(InMemoryBackend::new());
    let peer_pm = Arc::new(
        PersistenceManager::new(peer_backend)
            .await
            .expect("companion persistence manager"),
    );
    let (peer_client, _sync_rx) = Client::new(
        Arc::new(TokioRuntime),
        peer_pm,
        Arc::new(SinkTransportFactory),
        Arc::new(NoopHttpClient),
        None,
    )
    .await;

    let own_snapshot = client.persistence_manager.get_device_snapshot();
    let own_address = own_snapshot
        .pn
        .as_ref()
        .expect("own pn must be set before establishing the companion session")
        .to_protocol_address();
    let peer_address = peer.to_protocol_address();

    // The bundle is the companion device's real published material — identity
    // key and signed pre-key from its own store — so its side can decrypt what
    // we encrypt to it. No one-time pre-key: `PreKeyBundle` allows `None`, and
    // omitting it keeps this fixture from depending on pre-key generation.
    let peer_snapshot = peer_client.persistence_manager.get_device_snapshot();
    let bundle = PreKeyBundle::new(
        peer_snapshot.registration_id,
        u32::from(peer.device).into(),
        None,
        peer_snapshot.signed_pre_key_id.into(),
        peer_snapshot.signed_pre_key.public_key,
        peer_snapshot.signed_pre_key_signature,
        IdentityKey::new(peer_snapshot.identity_key.public_key),
    )
    .expect("companion prekey bundle");

    let mut rng = rand::make_rng::<rand::rngs::StdRng>();
    let mut adapter = client.signal_adapter();
    process_prekey_bundle(
        &peer_address,
        &mut adapter.session_store,
        &mut adapter.identity_store,
        &bundle,
        &mut rng,
        UsePQRatchet::No,
    )
    .await
    .expect("companion session");

    // us → companion: a pkmsg, since our pre-key is still unacknowledged.
    let initial = message_encrypt(
        b"init",
        &peer_address,
        &mut adapter.session_store,
        &mut adapter.identity_store,
    )
    .await
    .expect("initial encrypt");
    let mut peer_adapter = peer_client.signal_adapter();
    message_decrypt(
        &CiphertextMessage::PreKeySignalMessage(
            PreKeySignalMessage::try_from(initial.serialize()).expect("pkmsg parses"),
        ),
        &own_address,
        &mut peer_adapter.session_store,
        &mut peer_adapter.identity_store,
        &mut peer_adapter.pre_key_store,
        &peer_adapter.signed_pre_key_store,
        &mut rng,
        UsePQRatchet::No,
    )
    .await
    .expect("companion decrypts our first message");

    // companion → us: decrypting this reply is what clears our pending pre-key.
    let reply = message_encrypt(
        b"ack",
        &own_address,
        &mut peer_adapter.session_store,
        &mut peer_adapter.identity_store,
    )
    .await
    .expect("companion reply encrypt");
    message_decrypt(
        &CiphertextMessage::SignalMessage(
            SignalMessage::try_from(reply.serialize()).expect("msg parses"),
        ),
        &peer_address,
        &mut adapter.session_store,
        &mut adapter.identity_store,
        &mut adapter.pre_key_store,
        &adapter.signed_pre_key_store,
        &mut rng,
        UsePQRatchet::No,
    )
    .await
    .expect("we decrypt the companion's reply");

    // The point of all of the above. A fixture regression that reverted to a
    // one-way X3DH would otherwise quietly turn every measured send into a
    // first-contact send.
    assert!(
        !wacore::send::pkmsg_would_be_emitted(&mut adapter.session_store, &peer_address)
            .await
            .expect("session must be loadable in setup"),
        "the own companion's session must be acknowledged; a pkmsg here measures first contact"
    );
    peer_client
}

/// Put the client in the state a connected, post-offline-sync client is in: a
/// noise socket over the sink transport, connected flag set, live mode. Without
/// live mode every send would run through the drain batcher, which is a
/// different — and not steady-state — code path.
async fn install_offline_socket(client: &Arc<Client>) {
    use wacore::handshake::NoiseCipher;

    let (transport, _rx) = SinkTransportFactory
        .create_transport()
        .await
        .expect("sink transport");
    let noise_socket = crate::socket::NoiseSocket::with_observers(
        Arc::new(TokioRuntime),
        transport,
        NoiseCipher::new(&[0u8; 32]).expect("32-byte key"),
        NoiseCipher::new(&[0u8; 32]).expect("32-byte key"),
        crate::socket::noise_socket::SendObservers::with_stats(client.stats.clone()),
    );
    *client.noise_socket.lock().expect("noise socket mutex") = Some(Arc::new(noise_socket));
    client.set_connected_for_test(true);
    client.is_running.store(true, Ordering::Release);
    client.enter_live_mode_for_tests();
}

/// The recipient of the DM fixture, from the same reserved block and outside
/// every range the other fixtures draw from.
const DM_PEER_USER: &str = "19045550180";
/// Its LID; ours is `100000000000001`.
const DM_PEER_LID: &str = "100000000000002";

/// How the DM fixture addresses its recipient. The two are different code
/// paths per device, not a cosmetic switch: see each variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DmAddressing {
    /// A 1:1-LID-migrated account sending to a LID. The recipient's devices
    /// resolve under the LID and our companions are re-addressed to ours, so
    /// no device needs a PN→LID lookup on the way to its Signal address. The
    /// shape every migrated account sends in.
    Lid,
    /// An unmigrated account sending to a PN whose LID it knows. The wire
    /// stays PN (the server rejects a LID stanza from such an account) while
    /// every device's Signal address upgrades to LID, which is a mapping
    /// lookup per device at each place the send resolves addresses.
    PnWithKnownLid,
}

/// A logged-in client holding a resolved 1:1 recipient with `peer_devices`
/// devices plus our own companion, warmed to the steady state a repeat DM
/// starts from: every session acknowledged, the device memo populated.
///
/// The DM counterpart of [`GroupSendHarness`]; the same offline construction
/// (sink transport, in-memory store, pre-seeded registry) and the same
/// warm-up discipline. What it adds is the LID/PN mapping, which the group
/// fixture never exercises and which is the per-device term a DM pays.
pub struct DmSendHarness {
    runtime: tokio::runtime::Runtime,
    client: Arc<Client>,
    to: Jid,
}

impl DmSendHarness {
    pub fn new(peer_devices: usize, addressing: DmAddressing) -> Self {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("current-thread runtime");
        let (client, to) = runtime.block_on(build_dm_fixture(peer_devices, addressing));
        Self {
            runtime,
            client,
            to,
        }
    }

    /// One warm DM send through the real `Client::send_message`, from the
    /// recipient resolution to the frame the noise socket writes.
    pub fn warm_send(&self) {
        self.runtime.block_on(async {
            self.client
                .send_message(self.to.clone(), wa::Message::text("bench"))
                .await
                .expect("offline DM send must reach the sink");
        });
    }

    /// Per-term outcomes of the DM device memo so far; see
    /// [`GroupSendHarness::memo_stats`].
    pub fn memo_stats(&self) -> crate::client::DeviceMemoStats {
        self.client.device_memo_stats()
    }
}

async fn build_dm_fixture(peer_devices: usize, addressing: DmAddressing) -> (Arc<Client>, Jid) {
    use wacore::types::lid_pn::{LearningSource, LidPnEntry};

    assert!(peer_devices >= 1, "a recipient has at least its primary");

    let backend = Arc::new(InMemoryBackend::new());
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("persistence manager"),
    );
    let (client, _sync_rx) = Client::new(
        Arc::new(TokioRuntime),
        pm,
        Arc::new(SinkTransportFactory),
        Arc::new(NoopHttpClient),
        None,
    )
    .await;

    let own_pn = Jid::new(OWN_USER, Server::Pn);
    let own_lid = Jid::new("100000000000001", Server::Lid);
    client
        .persistence_manager
        .process_command(DeviceCommand::SetId(Some(own_pn.clone())))
        .await;
    client
        .persistence_manager
        .process_command(DeviceCommand::SetLid(Some(own_lid.clone())))
        .await;
    client
        .persistence_manager
        .process_command(DeviceCommand::SetLidMigrated(
            addressing == DmAddressing::Lid,
        ))
        .await;

    // The mapping a client that has exchanged messages with the peer holds.
    // Seeded before the registry so the topology generation the memo stamps
    // is past the write, as a warm client's is.
    client
        .lid_pn_cache
        .add(&LidPnEntry::new(
            DM_PEER_LID,
            DM_PEER_USER,
            LearningSource::Usync,
        ))
        .await;

    let devices: Vec<u16> = (0..peer_devices)
        .map(|d| u16::try_from(d).expect("device id fits"))
        .collect();
    // The registry is keyed by whichever user the lookup tries, so the same
    // record sits under both of the peer's identifiers.
    seed_registry(&client, DM_PEER_USER, &devices).await;
    seed_registry(&client, DM_PEER_LID, &devices).await;
    seed_registry(&client, OWN_USER, &[0, 1]).await;

    // Sessions live under the Signal address the send resolves to, which is
    // the LID for the peer in both shapes (WA Web's SignalAddress upgrades
    // PN→LID whenever the mapping is known). Every one is acknowledged: a
    // repeat DM re-encrypts to every device on every send, so a one-way
    // X3DH would measure first contact forever, as `GroupSendHarness`
    // explains for the companion.
    let peer_lid = Jid::new(DM_PEER_LID, Server::Lid);
    for device in &devices {
        establish_acknowledged_session(&client, &peer_lid.with_device(*device)).await;
    }
    // Our companion is re-addressed to our LID for a LID recipient and stays
    // PN otherwise (our own mapping is not in the cache, as it is not on a
    // client that never sent itself a message).
    let companion = match addressing {
        DmAddressing::Lid => own_lid.with_device(1),
        DmAddressing::PnWithKnownLid => own_pn.with_device(1),
    };
    establish_acknowledged_session(&client, &companion).await;

    install_offline_socket(&client).await;

    let to = match addressing {
        DmAddressing::Lid => peer_lid,
        DmAddressing::PnWithKnownLid => Jid::new(DM_PEER_USER, Server::Pn),
    };

    // Two sends outside every measurement: the first fills the device memo,
    // the second is the first true repeat. A benchmark measures send three
    // onwards.
    for _ in 0..2 {
        client
            .send_message(to.clone(), wa::Message::text("warm-up"))
            .await
            .expect("offline DM send must reach the sink");
    }
    settle_signal_flush_worker(&client).await;

    (client, to)
}

/// The peer of the receive fixture, from the same reserved block as the group
/// members and outside every range they draw from.
const PEER_USER: &str = "19045550190";

/// Counts the `Event::Messages` batches the bus delivers, standing in for the one
/// consumer a real client always has. A receive with no subscriber at all
/// would let the dispatcher skip work production never skips.
struct MessageCounter {
    delivered: portable_atomic::AtomicU64,
}

impl wacore::types::events::EventHandler for MessageCounter {
    fn handle_event(&self, event: Arc<wacore::types::events::Event>) {
        if matches!(&*event, wacore::types::events::Event::Messages(_)) {
            self.delivered.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn interest(&self) -> wacore::types::events::EventInterest {
        wacore::types::events::EventInterest::of(&[wacore::types::events::EventKind::Messages])
    }
}

/// A logged-in client receiving from one peer it already shares a session and
/// a sender key with: the steady state every message after the first arrives
/// in.
///
/// The inbound counterpart of [`GroupSendHarness`]. The peer is a second
/// in-memory client whose Signal stores encrypt each measured stanza, so what
/// this client decrypts is what a real peer would have sent, and what is
/// measured is everything from the decoded stanza to the dispatched event:
/// classification, the session or sender-key decrypt through the signal
/// cache, plaintext handling, dispatch, and the delivery receipt written to
/// the sink socket. The chat lane is not on the bill: a stanza enters at
/// `Client::handle_incoming_message`, which is what the lane worker awaits
/// per message, so the queue hop is excluded by construction and the
/// per-message work is not.
pub struct ReceiveHarness {
    runtime: tokio::runtime::Runtime,
    client: Arc<Client>,
    peer: Arc<Client>,
    peer_jid: Jid,
    own_address: wacore::libsignal::protocol::ProtocolAddress,
    group: Jid,
    group_sender_key: wacore::libsignal::protocol::SenderKeyName,
    counter: Arc<MessageCounter>,
    /// Dropping it would unsubscribe the counter.
    _subscription: wacore::types::events::Subscription,
    next_id: portable_atomic::AtomicU64,
}

impl ReceiveHarness {
    /// Build the receiving client and its peer, drive both to the steady state
    /// (acknowledged pairwise session, peer's sender key installed), and
    /// subscribe one message consumer.
    pub fn new() -> Self {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("current-thread runtime");
        let fixture = runtime.block_on(build_receive_fixture());
        Self {
            runtime,
            client: fixture.client,
            peer: fixture.peer,
            peer_jid: fixture.peer_jid,
            own_address: fixture.own_address,
            group: fixture.group,
            group_sender_key: fixture.group_sender_key,
            counter: fixture.counter,
            _subscription: fixture.subscription,
            next_id: portable_atomic::AtomicU64::new(0),
        }
    }

    /// A fresh 1:1 text message from the peer, encrypted with the acknowledged
    /// session, as the decoded `<message>` the read loop hands over. Each call
    /// advances the peer's sending chain, so stanzas must be received in the
    /// order they were built.
    pub fn dm_stanza(&self) -> Arc<wacore_binary::OwnedNodeRef> {
        use wacore::libsignal::protocol::message_encrypt;

        let plaintext = wacore::messages::MessageUtils::encode_and_pad(&wa::Message::text("bench"));
        let ciphertext = self.runtime.block_on(async {
            let mut adapter = self.peer.signal_adapter();
            message_encrypt(
                &plaintext,
                &self.own_address,
                &mut adapter.session_store,
                &mut adapter.identity_store,
            )
            .await
            .expect("peer encrypts to us")
        });
        let enc = wacore_binary::builder::NodeBuilder::new("enc")
            .attr("type", "msg")
            .attr("v", "2")
            .bytes(ciphertext.serialize().to_vec())
            .build();
        let node = wacore_binary::builder::NodeBuilder::new("message")
            .attr("from", self.peer_jid.to_string())
            .attr("id", self.next_message_id())
            .attr("t", wacore::time::now_secs().to_string())
            .attr("type", "text")
            .attr("notify", "Bench Peer")
            .children([enc])
            .build();
        decoded(&node)
    }

    /// A fresh group text message from the peer under its sender key. Same
    /// ordering requirement as [`Self::dm_stanza`].
    pub fn group_stanza(&self) -> Arc<wacore_binary::OwnedNodeRef> {
        use wacore::libsignal::protocol::group_encrypt;

        let plaintext = wacore::messages::MessageUtils::encode_and_pad(&wa::Message::text("bench"));
        let ciphertext = self.runtime.block_on(async {
            let mut adapter = self.peer.signal_adapter();
            let mut rng = rand::make_rng::<rand::rngs::StdRng>();
            group_encrypt(
                &mut adapter.sender_key_store,
                &self.group_sender_key,
                &plaintext,
                &mut rng,
            )
            .await
            .expect("peer encrypts to the group")
        });
        let enc = wacore_binary::builder::NodeBuilder::new("enc")
            .attr("type", "skmsg")
            .attr("v", "2")
            .bytes(ciphertext.serialized().to_vec())
            .build();
        let node = wacore_binary::builder::NodeBuilder::new("message")
            .attr("from", self.group.to_string())
            .attr("participant", self.peer_jid.to_string())
            .attr("id", self.next_message_id())
            .attr("t", wacore::time::now_secs().to_string())
            .attr("type", "text")
            .attr("notify", "Bench Peer")
            .children([enc])
            .build();
        decoded(&node)
    }

    /// Receive one stanza, all the way to the dispatched event and the
    /// delivery receipt on the sink socket.
    ///
    /// The receipt is not sent inline: `ack_received_message` hands it to a
    /// detached worker that marshals and noise-encrypts it, and on this
    /// current-thread runtime that worker only runs while something else is
    /// awaiting. Flushing the outbound scope is what pulls that work into
    /// the measured region instead of letting it leak into the next
    /// iteration, or past the last one; with nothing tracked it returns at
    /// once. The timeout only bounds a broken fixture and is never reached.
    pub fn receive(&self, node: Arc<wacore_binary::OwnedNodeRef>) {
        self.runtime.block_on(async {
            Arc::clone(&self.client).handle_incoming_message(node).await;
            self.client
                .outbound_flush
                .flush(&*self.client.runtime, std::time::Duration::from_secs(5))
                .await;
        })
    }

    /// How many `Event::Messages` batches reached the subscriber so far (one
    /// per received stanza in live mode). A benchmark
    /// asserts this against its iteration count, so a silent decrypt failure
    /// (which is fast) cannot pass for a receive.
    pub fn messages_delivered(&self) -> u64 {
        self.counter.delivered.load(Ordering::Relaxed)
    }

    /// A fresh stanza id per built message, so dedup never sees a repeat.
    fn next_message_id(&self) -> String {
        let n = self.next_id.fetch_add(1, Ordering::Relaxed);
        format!("3EB0BENCH{n:011}")
    }
}

impl Default for ReceiveHarness {
    fn default() -> Self {
        Self::new()
    }
}

/// The decoded form the read loop produces: marshalled and decoded back, so
/// attributes arrive wire-typed exactly as production sees them.
fn decoded(node: &wacore_binary::node::Node) -> Arc<wacore_binary::OwnedNodeRef> {
    let packed = wacore_binary::marshal::marshal(node).expect("marshal");
    let bytes = wacore_binary::util::unpack(&packed)
        .expect("unpack")
        .into_owned();
    Arc::new(wacore_binary::OwnedNodeRef::new(bytes).expect("decode"))
}

/// What [`build_receive_fixture`] hands back for [`ReceiveHarness::new`] to
/// pair with the runtime it was built on.
struct ReceiveFixture {
    client: Arc<Client>,
    peer: Arc<Client>,
    peer_jid: Jid,
    own_address: wacore::libsignal::protocol::ProtocolAddress,
    group: Jid,
    group_sender_key: wacore::libsignal::protocol::SenderKeyName,
    counter: Arc<MessageCounter>,
    subscription: wacore::types::events::Subscription,
}

/// The receive fixture's setup, on the harness's own runtime: both clients,
/// the acknowledged pairwise session, the installed sender key and the
/// subscribed counter, none of which a benchmark measures.
async fn build_receive_fixture() -> ReceiveFixture {
    use wacore::libsignal::protocol::create_sender_key_distribution_message;
    use wacore::types::jid::{JidExt, make_sender_key_name};

    let backend = Arc::new(InMemoryBackend::new());
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("persistence manager"),
    );
    let (client, _sync_rx) = Client::new(
        Arc::new(TokioRuntime),
        pm,
        Arc::new(SinkTransportFactory),
        Arc::new(NoopHttpClient),
        None,
    )
    .await;

    let own_pn = Jid::new(OWN_USER, Server::Pn);
    let own_lid = Jid::new("100000000000001", Server::Lid);
    client
        .persistence_manager
        .process_command(DeviceCommand::SetId(Some(own_pn.clone())))
        .await;
    client
        .persistence_manager
        .process_command(DeviceCommand::SetLid(Some(own_lid)))
        .await;

    // The peer is a known device, as it is for any chat with history; an
    // unknown one would fire a device sync on every message.
    seed_registry(&client, PEER_USER, &[0]).await;
    seed_registry(&client, OWN_USER, &[0, 1]).await;

    let peer_jid = Jid::new(PEER_USER, Server::Pn);
    let peer = establish_acknowledged_session(&client, &peer_jid).await;

    // The peer's sender key for the group, installed the way an inbound SKDM
    // installs it, so every measured skmsg finds its chain.
    let group: Jid = GROUP_JID.parse().expect("group jid");
    let group_sender_key = make_sender_key_name(&group, &peer_jid.to_protocol_address());
    let skdm = {
        let mut adapter = peer.signal_adapter();
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        create_sender_key_distribution_message(
            &group_sender_key,
            &mut adapter.sender_key_store,
            &mut rng,
        )
        .await
        .expect("peer sender key")
    };
    client
        .handle_sender_key_distribution_message(&group, &peer_jid, "bench-skdm", skdm.serialized())
        .await;

    let counter = Arc::new(MessageCounter {
        delivered: portable_atomic::AtomicU64::new(0),
    });
    let subscription = client
        .subscribe_handler(Arc::clone(&counter) as Arc<dyn wacore::types::events::EventHandler>);

    install_offline_socket(&client).await;
    settle_signal_flush_worker(&client).await;

    ReceiveFixture {
        client,
        peer,
        peer_jid,
        own_address: own_pn.to_protocol_address(),
        group,
        group_sender_key,
        counter,
        subscription,
    }
}

use crate::client::{Client, ClientError};
use log::{debug, warn};
use thiserror::Error;
use wacore::WireEnum;
use wacore::iq::tctoken::build_tc_token_node;
use wacore_binary::Jid;
use wacore_binary::Node;
use wacore_binary::builder::NodeBuilder;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum PresenceError {
    #[error("cannot send presence without a push name set")]
    PushNameEmpty,
    /// Connection/transport failure sending the `<presence>` stanza.
    #[error("{0}")]
    Client(#[from] ClientError),
    /// Catch-all for internal failures with no dedicated variant.
    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

/// Presence status for online/offline state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, WireEnum)]
#[non_exhaustive]
pub enum PresenceStatus {
    #[wire = "available"]
    Available,
    #[wire = "unavailable"]
    Unavailable,
}

impl From<crate::types::presence::Presence> for PresenceStatus {
    fn from(p: crate::types::presence::Presence) -> Self {
        match p {
            crate::types::presence::Presence::Available => PresenceStatus::Available,
            crate::types::presence::Presence::Unavailable => PresenceStatus::Unavailable,
        }
    }
}

/// Who announces the account's own `available` presence.
///
/// The client sends `<presence type="available">` on its own at three points,
/// matching WhatsApp Web: on connect when the push name is already known, when
/// the push name arrives through the initial app-state sync, and when the
/// server updates it. Explicit calls through [`Presence::set`] are unaffected
/// by this setting.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum PresencePolicy {
    /// The client announces `available` at those points (default).
    #[default]
    Automatic,
    /// The host owns every global presence transition. For a companion that
    /// stays connected around the clock, the automatic announcements would
    /// show the account online on every reconnect and, because the server
    /// treats an available device as where the user reads, stop the phone
    /// from being notified of new messages.
    Manual,
}

/// Feature handle for presence operations.
pub struct Presence<'a> {
    client: &'a Client,
}

impl<'a> Presence<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    async fn build_subscription_node(&self, jid: &Jid) -> Node {
        // Include tctoken if available (no t attribute, matching WhatsApp Web)
        let token = self.client.lookup_tc_token_for_jid(jid).await;
        Self::subscription_node_with_token(jid, token)
    }

    fn subscription_node_with_token(jid: &Jid, token: Option<Vec<u8>>) -> Node {
        let builder = NodeBuilder::new("presence")
            .attr("type", "subscribe")
            .attr("to", jid);

        match token {
            Some(token) => builder.children([build_tc_token_node(&token)]).build(),
            None => builder.build(),
        }
    }

    fn build_unsubscription_node(&self, jid: &Jid) -> Node {
        NodeBuilder::new("presence")
            .attr("type", "unsubscribe")
            .attr("to", jid)
            .build()
    }

    /// Set the presence status.
    pub async fn set(&self, status: PresenceStatus) -> Result<(), PresenceError> {
        let device_snapshot = self.client.persistence_manager().get_device_snapshot();

        debug!("send_presence called");

        if device_snapshot.push_name.is_empty() {
            warn!("Cannot send presence: push_name is empty!");
            return Err(PresenceError::PushNameEmpty);
        }

        // Track receipt activity like whatsmeow: available -> active receipts,
        // unavailable -> back to inactive (a forced value is preserved).
        match status {
            PresenceStatus::Available => {
                self.client.send_unified_session().await;
                self.client.mark_receipts_active_on_presence();
            }
            PresenceStatus::Unavailable => self.client.mark_receipts_inactive_on_presence(),
        }

        let presence_type = status.as_str();

        let node = NodeBuilder::new("presence")
            .attr("type", presence_type)
            .attr("name", &device_snapshot.push_name)
            .build();

        // The stanza carries the push name, so log the type alone: reprinting
        // the attribute puts the user's display name back in the log this
        // module just took it out of.
        debug!("Sending presence stanza: type={presence_type}");

        self.client.send_node(node).await?;
        Ok(())
    }

    /// Set presence to available (online).
    pub async fn set_available(&self) -> Result<(), PresenceError> {
        self.set(PresenceStatus::Available).await
    }

    /// Set presence to unavailable (offline).
    pub async fn set_unavailable(&self) -> Result<(), PresenceError> {
        self.set(PresenceStatus::Unavailable).await
    }

    /// Subscribe to a contact's presence updates.
    ///
    /// Sends a `<presence type="subscribe">` stanza to the target JID.
    /// If a valid tctoken exists for the contact, it is included as a child node.
    ///
    /// ## Wire Format
    /// ```xml
    /// <presence type="subscribe" to="user@s.whatsapp.net">
    ///   <tctoken><!-- raw token bytes --></tctoken>
    /// </presence>
    /// ```
    pub async fn subscribe(&self, jid: impl Into<Jid>) -> Result<(), PresenceError> {
        let jid = &jid.into();
        debug!("presence subscribe: subscribing to {}", jid);
        let node = self.build_subscription_node(jid).await;
        self.client.send_node(node).await?;
        self.client.track_presence_subscription(jid.clone());
        Ok(())
    }

    /// Re-subscribe presence if the JID has an active subscription.
    /// Does not modify the tracking set.
    ///
    /// The check is re-read per JID rather than taken from the resubscribe
    /// snapshot: an `unsubscribe` landing mid-resubscribe must not be undone.
    pub(crate) async fn re_subscribe_when_active(&self, jid: &Jid) -> Result<(), PresenceError> {
        if !self.client.is_presence_subscription_tracked(jid) {
            return Ok(());
        }

        let node = self.build_subscription_node(jid).await;
        // Re-read after the token lookup, which awaits. An `unsubscribe` landing
        // in that window has already sent its own stanza, so subscribing now
        // would leave the peer subscribed while we no longer track it. This
        // narrows the window rather than closing it — `send_node` awaits too —
        // but the lookup is the wide half and the re-read costs an uncontended
        // lock.
        if !self.client.is_presence_subscription_tracked(jid) {
            return Ok(());
        }
        self.client.send_node(node).await?;
        Ok(())
    }

    /// Unsubscribe from a contact's presence updates.
    ///
    /// Sends a `<presence type="unsubscribe">` stanza to the target JID.
    ///
    /// ## Wire Format
    /// ```xml
    /// <presence type="unsubscribe" to="user@s.whatsapp.net"/>
    /// ```
    pub async fn unsubscribe(&self, jid: &Jid) -> Result<(), PresenceError> {
        debug!("presence unsubscribe: unsubscribing from {}", jid);
        let node = self.build_unsubscription_node(jid);
        self.client.send_node(node).await?;
        self.client.untrack_presence_subscription(jid);
        Ok(())
    }
}

/// How many presence re-subscriptions a reconnect keeps in flight at once.
///
/// Matched to the noise sender's own job channel (`bounded(8)`): the point of
/// the window is to give that sender several frames to coalesce, and a window
/// wider than its queue only blocks on it.
const RESUBSCRIBE_WINDOW: usize = 8;

impl Client {
    fn lock_presence_subscriptions(
        &self,
    ) -> std::sync::MutexGuard<'_, std::collections::HashSet<Jid>> {
        self.presence_subscriptions
            .lock()
            .unwrap_or_else(|p| p.into_inner())
    }

    pub(crate) fn track_presence_subscription(&self, jid: Jid) {
        self.lock_presence_subscriptions().insert(jid);
    }

    pub(crate) fn untrack_presence_subscription(&self, jid: &Jid) {
        self.lock_presence_subscriptions().remove(jid);
    }

    pub(crate) fn is_presence_subscription_tracked(&self, jid: &Jid) -> bool {
        self.lock_presence_subscriptions().contains(jid)
    }

    pub(crate) fn tracked_presence_subscriptions(&self) -> Vec<Jid> {
        self.lock_presence_subscriptions().iter().cloned().collect()
    }

    /// Re-subscribe to every tracked contact's presence, a window at a time.
    ///
    /// Two things make the window the right unit rather than the JID. The
    /// tcToken lookup is one store read per contact, and the whole window's
    /// worth collapses into a single backend call. And awaiting each
    /// `send_node` in turn guarantees the noise sender never has more than one
    /// job queued, so every stanza became its own transport write — one TLS
    /// record, one WebSocket frame and one syscall per contact; issuing a
    /// window at once is what lets the sender coalesce them, exactly as the
    /// transport-ack worker does.
    ///
    /// The window is what bounds it: a client tracking hundreds of contacts
    /// still has at most [`RESUBSCRIBE_WINDOW`] stanzas in flight, and the
    /// generation / connection checks run per window, so a reconnect landing
    /// mid-walk still stops it within one window rather than after the lot.
    pub(crate) async fn resubscribe_presence_subscriptions(&self, expected_generation: u64) {
        let subscribed_jids = self.tracked_presence_subscriptions();
        if subscribed_jids.is_empty() {
            return;
        }

        debug!(
            "Re-subscribing to {} tracked presence subscriptions",
            subscribed_jids.len()
        );

        for window in subscribed_jids.chunks(RESUBSCRIBE_WINDOW) {
            if self
                .connection_generation
                .load(std::sync::atomic::Ordering::SeqCst)
                != expected_generation
            {
                debug!("Stopping presence re-subscribe: connection generation changed");
                return;
            }

            if !self.is_connected() {
                debug!("Stopping presence re-subscribe: connection closed");
                return;
            }

            // An `unsubscribe` may land at any point in the walk, so the
            // tracking set is re-read here and again after the lookup — never
            // taken from the snapshot above, which would let this undo it.
            let pending: Vec<Jid> = window
                .iter()
                .filter(|jid| self.is_presence_subscription_tracked(jid))
                .cloned()
                .collect();
            if pending.is_empty() {
                continue;
            }

            let tokens = self.lookup_tc_tokens_for_jids(&pending).await;
            let sends = pending.iter().zip(tokens).filter_map(|(jid, token)| {
                // Re-read after the lookup, which awaited. An `unsubscribe`
                // landing in that window has already sent its own stanza, so
                // subscribing now would leave the peer subscribed while we no
                // longer track it.
                self.is_presence_subscription_tracked(jid).then(|| {
                    let node = Presence::subscription_node_with_token(jid, token);
                    async move {
                        if let Err(err) = self.send_node(node).await {
                            warn!("Failed to re-subscribe to presence for {jid}: {err:?}");
                        }
                    }
                })
            });
            futures::future::join_all(sends).await;
        }
    }

    /// Access presence operations.
    #[allow(clippy::wrong_self_convention)]
    pub fn presence(&self) -> Presence<'_> {
        Presence::new(self)
    }

    /// The `available` announcement the connection lifecycle makes on its
    /// own. Returns `Ok(false)` when [`PresencePolicy::Manual`] suppressed it,
    /// so callers can log the two outcomes apart. The policy is checked before
    /// the push name, because a manual host must not be told that a stanza it
    /// never asked for could not be sent.
    pub(crate) async fn send_automatic_available(&self) -> Result<bool, PresenceError> {
        if self.presence_policy() == PresencePolicy::Manual {
            debug!("Automatic available presence suppressed by the manual policy");
            return Ok(false);
        }
        self.presence().set_available().await?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::TokioRuntime;
    use crate::bot::Bot;
    use crate::http::{HttpClient, HttpRequest, HttpResponse};
    use crate::store::SqliteStore;
    use crate::store::commands::DeviceCommand;
    use anyhow::Result;
    use std::str::FromStr;
    use std::sync::Arc;
    use wacore::store::traits::Backend;
    use whatsapp_rust_tokio_transport::TokioWebSocketTransportFactory;

    // Mock HTTP client for testing
    #[derive(Debug, Clone)]
    struct MockHttpClient;

    #[async_trait::async_trait]
    impl HttpClient for MockHttpClient {
        async fn execute(&self, _request: HttpRequest) -> Result<HttpResponse> {
            Ok(HttpResponse {
                status_code: 200,
                body: br#"self.__swData=JSON.parse(/*BTDS*/"{\"dynamic_data\":{\"SiteData\":{\"server_revision\":1026131876,\"client_revision\":1026131876}}}");"#.to_vec(),
            })
        }
    }

    async fn create_test_backend() -> Arc<dyn Backend> {
        let temp_db = format!(
            "file:memdb_presence_{}?mode=memory&cache=shared",
            uuid::Uuid::new_v4()
        );
        Arc::new(
            SqliteStore::new(&temp_db)
                .await
                .expect("Failed to create test SqliteStore"),
        ) as Arc<dyn Backend>
    }

    /// Verifies WhatsApp Web behavior: presence deferred until pushname available.
    #[tokio::test]
    async fn test_presence_rejected_when_pushname_empty() {
        let backend = create_test_backend().await;
        let transport = TokioWebSocketTransportFactory::new();

        let bot = Bot::builder()
            .with_backend_arc(backend)
            .with_transport_factory(transport)
            .with_http_client(MockHttpClient)
            .with_runtime(TokioRuntime)
            .build()
            .await
            .expect("Failed to build bot");

        let client = bot.client();

        let snapshot = client.persistence_manager().get_device_snapshot();
        assert!(
            snapshot.push_name.is_empty(),
            "Pushname should be empty on fresh device"
        );

        let result = client.presence().set(PresenceStatus::Available).await;

        assert!(
            result.is_err(),
            "Presence should fail when pushname is empty"
        );
        assert!(
            matches!(result.unwrap_err(), PresenceError::PushNameEmpty),
            "Error should be PushNameEmpty"
        );
    }

    /// Simulates pushname arriving from app state sync (setting_pushName mutation).
    #[tokio::test]
    async fn test_presence_succeeds_after_pushname_set() {
        let backend = create_test_backend().await;
        let transport = TokioWebSocketTransportFactory::new();

        let bot = Bot::builder()
            .with_backend_arc(backend)
            .with_transport_factory(transport)
            .with_http_client(MockHttpClient)
            .with_runtime(TokioRuntime)
            .build()
            .await
            .expect("Failed to build bot");

        let client = bot.client();

        client
            .persistence_manager()
            .process_command(DeviceCommand::SetPushName("Test User".to_string()))
            .await;

        let snapshot = client.persistence_manager().get_device_snapshot();
        assert_eq!(snapshot.push_name, "Test User");

        // Validation passes; error should be connection-related, not pushname
        let result = client.presence().set(PresenceStatus::Available).await;

        if let Err(e) = result {
            assert!(
                !matches!(e, PresenceError::PushNameEmpty),
                "Should not fail due to pushname, got: {}",
                e
            );
            assert!(
                matches!(e, PresenceError::Client(_)),
                "Expected connection error (Client), got: {}",
                e
            );
        }
    }

    /// Matches WAWebPushNameSync.js: fresh pairing -> app state sync -> presence.
    #[tokio::test]
    async fn test_pushname_presence_flow_matches_whatsapp_web() {
        let backend = create_test_backend().await;
        let transport = TokioWebSocketTransportFactory::new();

        let bot = Bot::builder()
            .with_backend_arc(backend)
            .with_transport_factory(transport)
            .with_http_client(MockHttpClient)
            .with_runtime(TokioRuntime)
            .build()
            .await
            .expect("Failed to build bot");

        let client = bot.client();

        // Fresh device has empty pushname
        let snapshot = client.persistence_manager().get_device_snapshot();
        assert!(snapshot.push_name.is_empty());

        // Presence deferred when pushname empty
        let result = client.presence().set(PresenceStatus::Available).await;
        assert!(matches!(result, Err(PresenceError::PushNameEmpty)));

        // Pushname arrives via app state sync
        client
            .persistence_manager()
            .process_command(DeviceCommand::SetPushName("WhatsApp User".to_string()))
            .await;

        // Now presence validation passes
        let result = client.presence().set(PresenceStatus::Available).await;

        if let Err(e) = result {
            assert!(
                !matches!(e, PresenceError::PushNameEmpty),
                "Error should be connection-related: {}",
                e
            );
        }
    }

    /// The policy only decides the lifecycle's own announcement: manual mode
    /// answers `Ok(false)` before the push-name check, while an explicit
    /// `set_available` keeps validating and sending as before.
    #[tokio::test]
    async fn manual_policy_suppresses_only_the_automatic_announcement() {
        let backend = create_test_backend().await;
        let transport = TokioWebSocketTransportFactory::new();

        let bot = Bot::builder()
            .with_backend_arc(backend)
            .with_transport_factory(transport)
            .with_http_client(MockHttpClient)
            .with_runtime(TokioRuntime)
            .with_presence_policy(PresencePolicy::Manual)
            .build()
            .await
            .expect("Failed to build bot");

        let client = bot.client();
        assert_eq!(client.presence_policy(), PresencePolicy::Manual);

        // Push name is empty on a fresh device, so an announcement that got
        // as far as the explicit path would fail rather than be skipped.
        assert!(matches!(client.send_automatic_available().await, Ok(false)));
        assert!(matches!(
            client.presence().set_available().await,
            Err(PresenceError::PushNameEmpty)
        ));

        client.set_presence_policy(PresencePolicy::Automatic);
        assert_eq!(client.presence_policy(), PresencePolicy::Automatic);
        assert!(matches!(
            client.send_automatic_available().await,
            Err(PresenceError::PushNameEmpty)
        ));
    }

    #[tokio::test]
    async fn presence_policy_defaults_to_automatic() {
        let backend = create_test_backend().await;
        let transport = TokioWebSocketTransportFactory::new();

        let bot = Bot::builder()
            .with_backend_arc(backend)
            .with_transport_factory(transport)
            .with_http_client(MockHttpClient)
            .with_runtime(TokioRuntime)
            .build()
            .await
            .expect("Failed to build bot");

        assert_eq!(bot.client().presence_policy(), PresencePolicy::Automatic);
    }

    #[tokio::test]
    async fn test_presence_subscription_tracking_is_deduplicated() {
        let backend = create_test_backend().await;
        let transport = TokioWebSocketTransportFactory::new();

        let bot = Bot::builder()
            .with_backend_arc(backend)
            .with_transport_factory(transport)
            .with_http_client(MockHttpClient)
            .with_runtime(TokioRuntime)
            .build()
            .await
            .expect("Failed to build bot");

        let client = bot.client();
        let jid = Jid::from_str("1234567890@s.whatsapp.net").expect("valid jid");

        client.track_presence_subscription(jid.clone());
        client.track_presence_subscription(jid.clone());

        let tracked = client.tracked_presence_subscriptions();
        assert_eq!(tracked, vec![jid]);
    }

    #[tokio::test]
    async fn test_presence_unsubscription_removes_tracked_jid() {
        let backend = create_test_backend().await;
        let transport = TokioWebSocketTransportFactory::new();

        let bot = Bot::builder()
            .with_backend_arc(backend)
            .with_transport_factory(transport)
            .with_http_client(MockHttpClient)
            .with_runtime(TokioRuntime)
            .build()
            .await
            .expect("Failed to build bot");

        let client = bot.client();
        let jid = Jid::from_str("1234567890@s.whatsapp.net").expect("valid jid");

        client.track_presence_subscription(jid.clone());
        client.untrack_presence_subscription(&jid);

        assert!(
            client.tracked_presence_subscriptions().is_empty(),
            "unsubscribe tracking should remove the jid"
        );
    }

    #[tokio::test]
    async fn test_unsubscribe_builds_expected_presence_stanza() {
        let jid = Jid::from_str("1234567890@s.whatsapp.net").expect("valid jid");
        let backend = create_test_backend().await;
        let transport = TokioWebSocketTransportFactory::new();

        let bot = Bot::builder()
            .with_backend_arc(backend)
            .with_transport_factory(transport)
            .with_http_client(MockHttpClient)
            .with_runtime(TokioRuntime)
            .build()
            .await
            .expect("Failed to build bot");

        let client = bot.client();
        let node = client.presence().build_unsubscription_node(&jid);

        assert_eq!(node.tag, "presence");
        assert!(node.attrs.get("type").is_some_and(|v| v == "unsubscribe"));
        assert_eq!(
            node.attrs.get("to").map(ToString::to_string),
            Some(jid.to_string())
        );
        assert!(
            node.content.is_none(),
            "unsubscribe stanza should not have children"
        );
    }

    /// Awaiting each subscribe in turn left the noise sender with one job
    /// queued at a time, so every stanza became its own transport write. A
    /// window's worth in flight is what gives the sender something to coalesce.
    #[tokio::test]
    async fn resubscribe_batches_its_stanzas_into_fewer_transport_writes() {
        use std::sync::atomic::Ordering;

        const TRACKED: usize = 24;

        let (client, transport) = crate::test_utils::create_iq_test_client().await;
        for i in 0..TRACKED {
            let jid: Jid = format!("1904555{i:04}@s.whatsapp.net")
                .parse()
                .expect("valid jid");
            client.track_presence_subscription(jid);
        }

        let generation = client.connection_generation.load(Ordering::SeqCst);
        client.resubscribe_presence_subscriptions(generation).await;

        assert_eq!(
            transport.sent_count(),
            TRACKED,
            "every tracked contact is still re-subscribed exactly once"
        );
        assert!(
            transport.write_count() < TRACKED,
            "{TRACKED} frames reached the transport in {} writes; the point of \
             the window is that the sender coalesces them",
            transport.write_count(),
        );
    }

    /// The resubscribe walk snapshots the tracked set, then re-checks each JID
    /// before building and sending its stanza. This gates the first window's
    /// first write so an `unsubscribe` lands after the snapshot was taken but
    /// before the walk reaches the window that JID is in.
    #[tokio::test]
    async fn resubscribe_skips_a_jid_unsubscribed_before_its_window() {
        use bytes::Bytes;
        use std::sync::Arc;
        use std::sync::atomic::{AtomicBool, Ordering};

        struct GatedTransport {
            inner: Arc<crate::transport::mock::CapturingMockTransport>,
            started: async_channel::Sender<()>,
            release: async_channel::Receiver<()>,
            gate_next_send: AtomicBool,
        }

        #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
        #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
        impl crate::transport::Transport for GatedTransport {
            async fn send(&self, data: Bytes) -> Result<(), anyhow::Error> {
                if self.gate_next_send.swap(false, Ordering::AcqRel) {
                    self.started
                        .send(())
                        .await
                        .map_err(|_| anyhow::anyhow!("gate observer closed"))?;
                    self.release
                        .recv()
                        .await
                        .map_err(|_| anyhow::anyhow!("gate closed"))?;
                }
                self.inner.send(data).await
            }

            async fn disconnect(&self) {}
        }

        let (client, _transport) = crate::test_utils::create_iq_test_client().await;

        let (started_tx, started_rx) = async_channel::bounded(1);
        let (release_tx, release_rx) = async_channel::bounded(1);
        let captured = Arc::new(crate::transport::mock::CapturingMockTransport::new());
        let gated = crate::socket::NoiseSocket::new(
            Arc::new(TokioRuntime),
            Arc::new(GatedTransport {
                inner: captured.clone(),
                started: started_tx,
                release: release_rx,
                gate_next_send: AtomicBool::new(true),
            }),
            wacore::handshake::NoiseCipher::new(&[0u8; 32]).expect("valid key"),
            wacore::handshake::NoiseCipher::new(&[0u8; 32]).expect("valid key"),
        );
        *client.noise_socket.lock().unwrap() = Some(Arc::new(gated));

        // One full window plus one, so there is a JID the walk has not reached
        // when the first window's first write blocks.
        for i in 0..=RESUBSCRIBE_WINDOW {
            let jid: Jid = format!("1202555{i:04}@s.whatsapp.net")
                .parse()
                .expect("valid jid");
            client.track_presence_subscription(jid);
        }
        // The set is not modified between this read and the walk's own, and a
        // `HashSet` iterates one state in one order, so this is the order the
        // walk will use — which is what makes "in the second window" true.
        let walk_order = client.tracked_presence_subscriptions();
        let unsubscribed = walk_order[RESUBSCRIBE_WINDOW].clone();

        let generation = client.connection_generation.load(Ordering::SeqCst);
        let resubscribe = {
            let client = client.clone();
            tokio::spawn(async move { client.resubscribe_presence_subscriptions(generation).await })
        };

        started_rx.recv().await.expect("the first write is gated");
        client.untrack_presence_subscription(&unsubscribed);
        release_tx.send(()).await.expect("gate released");
        resubscribe
            .await
            .expect("resubscribe task should not panic");

        let targets: Vec<String> =
            crate::test_utils::decrypt_wire_frames(&captured.sent(), &[0u8; 32])
                .iter()
                .map(|plaintext| {
                    let unpacked =
                        wacore_binary::util::unpack(plaintext).expect("a sent frame unpacks");
                    let node = wacore_binary::OwnedNodeRef::new(unpacked.into_owned())
                        .expect("a sent frame decodes");
                    node.get()
                        .get_attr("to")
                        .map(|to| to.to_string())
                        .unwrap_or_default()
                })
                .collect();

        assert_eq!(
            targets.len(),
            RESUBSCRIBE_WINDOW,
            "the window that was in flight is re-subscribed; the next one is not, \
             because the JID it held was untracked first"
        );
        assert!(
            !targets.contains(&unsubscribed.to_string()),
            "a JID unsubscribed before its window is reached must not be re-subscribed"
        );
        assert!(
            !client
                .tracked_presence_subscriptions()
                .contains(&unsubscribed),
            "and the unsubscribe must stand"
        );
    }
}

//! Consumer-driven re-sync of app-state collections.
//!
//! Every other sync in this crate is raised by the server — a `server_sync`
//! notification, a dirty bit, the initial bootstrap. This is the one an
//! application raises itself, after deciding from the outside that a collection
//! it mirrors has drifted.

use crate::client::{BatchedSyncOutcome, BatchedSyncRequest, Client};
use crate::features::chat_actions::AppStateError;
use crate::request::IqError;
use wacore::appstate::patch_decode::WAPatchName;

/// How much of a collection a re-sync asks the server for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AppStateResyncMode {
    /// Ask for the patches recorded after the persisted version.
    ///
    /// The cheap repair, and the right one when the *client's own* app-state
    /// store is behind the server: it sends only what changed since the last
    /// successful sync.
    ///
    /// It cannot repair a consumer's mirror on its own. The version it asks
    /// from is the client's, not the mirror's, so a mirror that missed
    /// mutations the client did apply — a handler that failed, an event dropped
    /// on a full mailbox, a database restored behind the client's — asks the
    /// server for changes it already has, and the server rightly sends nothing.
    /// The collection comes back [`synced`](AppStateResyncReport::synced) and
    /// the mirror is exactly as stale as before. Use [`Self::Snapshot`] for
    /// that.
    Incremental,
    /// Ask for the collection's current snapshot, whatever the persisted
    /// version says.
    ///
    /// The expensive repair, for when local state is not behind but *wrong* —
    /// a mirror rebuilt from scratch, a database restored from a backup, an
    /// ltHash that stopped agreeing with its peers.
    ///
    /// # A snapshot replays; it does not reconcile
    ///
    /// The response carries what the collection holds *now*, and every record in
    /// it is dispatched as a fresh mutation with `from_full_sync` set. Nothing is
    /// dispatched for a record the server no longer has, because the snapshot
    /// does not mention it — so a mirror carrying a contact, label or mute that
    /// has since been deleted still carries it afterwards, and the collection is
    /// still reported [`synced`](AppStateResyncReport::synced).
    ///
    /// A consumer repairing that kind of drift should clear its copy of the
    /// collection first and rebuild it from the replay.
    ///
    /// What the return does and does not tell you: every mutation has been
    /// *dispatched to the event bus* by then, and the collection's own state is
    /// persisted. It does not mean handlers have run. Delivery from there is the
    /// bus's, and under
    /// [`EventDelivery::Concurrent`](crate::bot::EventDelivery::Concurrent) each
    /// callback runs in a task of its own, while
    /// [`EventDelivery::Ordered`](crate::bot::EventDelivery::Ordered) queues and
    /// drops on a full mailbox. A report is evidence about the sync, not about a
    /// consumer's mirror.
    ///
    /// A collection can also come back `synced` having replayed nothing, because
    /// a response with no snapshot is what an empty collection answers and the
    /// protocol gives no way to tell that apart from a request the server chose
    /// not to serve.
    ///
    /// A snapshot older than the persisted version is normal — the server
    /// snapshots periodically and sends the patches recorded since — and is
    /// rebuilt from as long as those patches can carry the collection back to
    /// where it was. One that cannot is refused and reported
    /// [`retryable`](AppStateResyncReport::retryable) rather than applied. That
    /// judgement is made on versions alone, so a patch that fails its MACs after
    /// the snapshot has replaced the baseline leaves the collection whole but at
    /// the snapshot's version: a cursor to re-earn on the next sync, not damage.
    Snapshot,
}

/// What [`Client::resync_app_state`] achieved, collection by collection.
///
/// A single `Ok`/`Err` cannot carry this: "the server refused this collection",
/// "the run stopped early and a later one can finish it" and "another writer
/// held it" are three different answers, and only the caller knows which of them
/// is worth reacting to. So the call reports what happened per collection and
/// reserves its `Err` for the request never reaching the server at all.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct AppStateResyncReport {
    /// Fetched, applied and persisted.
    pub synced: Vec<WAPatchName>,
    /// The server refused the collection outright (400/404). Asking again gets
    /// the same answer, so a retry never clears this on its own.
    pub fatal: Vec<WAPatchName>,
    /// Not synced, but a later attempt can: a retryable server error, a snapshot
    /// the request asked for and the response did not carry, a decode key that
    /// never landed, another writer that held the collection past the wait, the
    /// connection going away mid-run, or the pagination cap.
    pub retryable: Vec<WAPatchName>,
    /// Another sync was already fetching the collection, so this call stood
    /// down and left the work to it.
    ///
    /// Empty in practice for this call, which waits for a holder rather than
    /// standing down — an explicit request is not discharged by whatever
    /// another caller happened to ask for. A wait that runs out lands in
    /// `retryable` instead, since nobody covered the collection. The bucket is
    /// here because the report mirrors the sync engine's, and a future caller
    /// that does stand down should not have its result quietly folded into
    /// another category.
    pub skipped: Vec<WAPatchName>,
}

impl AppStateResyncReport {
    /// True when every collection asked for came back synced.
    pub fn all_synced(&self) -> bool {
        self.unsynced().next().is_none()
    }

    /// Every collection this call did not leave synced, whatever the reason.
    pub fn unsynced(&self) -> impl Iterator<Item = WAPatchName> + '_ {
        self.fatal
            .iter()
            .chain(&self.retryable)
            .chain(&self.skipped)
            .copied()
    }
}

impl From<BatchedSyncOutcome> for AppStateResyncReport {
    fn from(outcome: BatchedSyncOutcome) -> Self {
        Self {
            synced: outcome.synced,
            fatal: outcome.fatal,
            retryable: outcome.retryable,
            skipped: outcome.skipped,
        }
    }
}

impl Client {
    /// Re-fetch the named app-state collections from the server.
    ///
    /// For applications that keep their own mirror of chat metadata (mutes,
    /// pins, archives, contacts, labels) and detect from the outside that it has
    /// drifted from the server's. Nothing in the protocol asks for this; it is a
    /// repair, so it is spelled out by the caller rather than inferred.
    ///
    /// Reservation, connection fencing, pagination and mutation dispatch are the
    /// same ones every server-raised sync goes through — this routes into that
    /// path rather than reproducing it, so a re-sync cannot interleave its
    /// version, ltHash and mutation-MAC writes with a sync or a patch send for
    /// the same collection. It waits for such a writer instead of skipping
    /// behind it: an explicit request is not discharged by whatever another
    /// caller happened to ask for.
    ///
    /// Duplicated collections are asked for once. An empty request is a no-op
    /// and never reaches the wire.
    ///
    /// # Blocking and retries
    ///
    /// This waits for a usable connection before asking anything, bounded only
    /// by the client's own lifetime — a reconnect backoff runs to 900s, and a
    /// client that is offline on purpose is waited for rather than failed, so
    /// the wait can be long.
    ///
    /// Bound it with a timeout if a caller must, but bound the *wait*, not the
    /// whole call: dropping the future once a response is being applied cancels
    /// it between the writes that replace a collection's baseline — the mutation
    /// MACs and the version are separate commits — and leaves the collection
    /// needing another sync to agree with itself again. Nothing is lost that a
    /// later sync cannot rebuild, and no other writer can interleave while this
    /// one holds the reservation, but a cancelled apply is not a clean stop.
    ///
    /// It also does not retry: what a run left unsynced comes back in the
    /// report, and *when* to ask again is the caller's decision. Re-asking in a
    /// tight loop is how a client earns a 403 — space attempts out, and treat
    /// [`AppStateResyncReport::fatal`] as an answer rather than as a failure to
    /// retry.
    ///
    /// # Errors
    ///
    /// [`AppStateError::InvalidRequest`] when the request names
    /// [`WAPatchName::Unknown`], which is a parse fallback rather than a
    /// collection the server has. [`AppStateError::NotConnected`] when the
    /// transport was what stopped it: the client stopped or was never running,
    /// the connection this request was admitted on retired before a single IQ
    /// went out, or the IQ was refused for want of a socket.
    /// [`AppStateError::Internal`] when the request itself failed — it timed
    /// out, or a response could not be read. A collection the server answered
    /// *about* is reported in the returned [`AppStateResyncReport`], never as an
    /// error.
    ///
    /// # Example
    ///
    /// ```no_run
    /// # use whatsapp_rust::{AppStateResyncMode, Client, WAPatchName};
    /// # async fn f(client: &Client) -> Result<(), Box<dyn std::error::Error>> {
    /// let report = client
    ///     .resync_app_state([WAPatchName::Regular], AppStateResyncMode::Incremental)
    ///     .await?;
    /// if !report.all_synced() {
    ///     println!("still stale: {:?}", report.unsynced().collect::<Vec<_>>());
    /// }
    /// # Ok(())
    /// # }
    /// ```
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.appstate.resync", level = "debug", skip_all, fields(mode = ?mode), err(Debug)))]
    pub async fn resync_app_state(
        &self,
        collections: impl IntoIterator<Item = WAPatchName>,
        mode: AppStateResyncMode,
    ) -> Result<AppStateResyncReport, AppStateError> {
        let collections: Vec<WAPatchName> = collections.into_iter().collect();
        // Checked before the wait, not after: a caller with nothing to ask for
        // must not park until a connection it does not need shows up.
        if collections.is_empty() {
            return Ok(AppStateResyncReport::default());
        }

        // `WAPatchName::Unknown` is what parsing an unrecognised collection name
        // yields, not a collection the server has; asking for it would put
        // `name="unknown"` on the wire for the server to refuse. Rejected rather
        // than filtered out, and rejected whole: a request that silently syncs
        // the rest reports success for a batch it did not carry out.
        if collections.contains(&WAPatchName::Unknown) {
            return Err(AppStateError::InvalidRequest(
                "WAPatchName::Unknown is a parse fallback, not a collection that can be synced"
                    .to_owned(),
            ));
        }

        // The verdict is the point of the wait. `false` means the client is
        // finished or has no reader to answer an IQ, and going on from there
        // would report every collection retryable for a connection that is never
        // coming.
        if !self.await_connection().await {
            return Err(AppStateError::NotConnected);
        }

        let scope = self.sync_scope(None);
        let outcome = match self
            .sync_collections_batched_with(
                collections,
                scope,
                BatchedSyncRequest {
                    snapshot: mode == AppStateResyncMode::Snapshot,
                    wait_for_holder: true,
                },
            )
            .await
        {
            Ok(outcome) => outcome,
            // The connection can go between being admitted and the IQ reaching
            // the socket, and `send_iq` then refuses it outright. That is the
            // same answer the wait gives when no connection is coming, so it
            // gets the same error rather than an opaque `Internal` a caller
            // would have to unwrap to tell apart from a real failure.
            Err(e) => {
                return Err(
                    if e.chain().any(|source| {
                        source
                            .downcast_ref::<IqError>()
                            .is_some_and(IqError::is_transport_unavailable)
                    }) {
                        AppStateError::NotConnected
                    } else {
                        AppStateError::Internal(e)
                    },
                );
            }
        };

        // The wait can be won and the connection lost before the batch reserves
        // anything: the socket it was admitted on retires, every collection comes
        // back retryable, and no IQ was ever built. Reporting that would answer a
        // question about the server with an outcome the server never saw, so it
        // joins the other way this call never reached one. Asked as "the scope is
        // gone" rather than "nothing synced", because a batch that waited out a
        // holder also reaches nobody — and there the connection is fine and the
        // per-collection verdict is the honest answer.
        if !outcome.reached_server() && self.admits(scope).is_err() {
            return Err(AppStateError::NotConnected);
        }

        Ok(outcome.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::{SyncHolder, batch_result};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    use wacore::appstate::hash::HashState;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::node::Node;

    /// A test client with a socket but no `<success>` behind it, which is what
    /// [`crate::test_utils::create_iq_test_client`] leaves — and which
    /// `await_connection` is right to keep waiting on. Both stores
    /// `handle_success` makes, in that order: the session, then the generation
    /// it is authenticated under.
    async fn create_reachable_client() -> (
        Arc<Client>,
        Arc<crate::transport::mock::CapturingMockTransport>,
    ) {
        let (client, transport) = crate::test_utils::create_iq_test_client().await;
        client.is_logged_in.store(true, Ordering::Relaxed);
        client.authenticated_generation.store(
            client.connection_generation.load(Ordering::SeqCst),
            Ordering::SeqCst,
        );
        (client, transport)
    }

    /// Answer every IQ the resync sends, with `collections` describing the
    /// per-collection reply. Returns the resync's own result plus every
    /// `<collection>` node that reached the wire, in order.
    async fn resync_against(
        client: &Arc<Client>,
        transport: &Arc<crate::transport::mock::CapturingMockTransport>,
        request: Vec<WAPatchName>,
        mode: AppStateResyncMode,
        reply: impl Fn(usize, &str) -> Node,
    ) -> (Result<AppStateResyncReport, AppStateError>, Vec<Node>) {
        use futures::FutureExt;

        let mut resync = {
            let client = Arc::clone(client);
            tokio::spawn(async move { client.resync_app_state(request, mode).await })
        };

        let asked: std::sync::Mutex<Vec<Node>> = std::sync::Mutex::new(Vec::new());
        let server = async {
            let mut frame = 0usize;
            loop {
                let node = crate::test_utils::decode_sent_iq(transport, frame).await;
                let node = node.get().to_owned();
                let id = node
                    .attrs()
                    .optional_string("id")
                    .expect("every IQ carries an id")
                    .into_owned();
                if let Some(sync) = node.get_optional_child("sync")
                    && let Some(children) = sync.children()
                {
                    asked
                        .lock()
                        .expect("not poisoned")
                        .extend(children.iter().filter(|c| c.tag == "collection").cloned());
                }
                let response = reply(frame, &id);
                crate::test_utils::answer_iq(client, &id, &response).await;
                frame += 1;
            }
        };
        futures::pin_mut!(server);
        let result = futures::select! {
            result = (&mut resync).fuse() => result.expect("the resync task should not panic"),
            () = server.as_mut().fuse() => unreachable!("the responder never completes"),
        };

        let asked = asked.lock().expect("not poisoned").clone();
        (result, asked)
    }

    /// Persist a non-zero version so "snapshot" and "incremental" ask for
    /// different things — with version 0 every sync asks for a snapshot anyway,
    /// and the mode would prove nothing.
    async fn persist_version(client: &Arc<Client>, name: WAPatchName, version: u64) {
        client
            .persistence_manager
            .backend()
            .set_version(
                name.as_str(),
                HashState {
                    version,
                    ..Default::default()
                },
            )
            .await
            .expect("the test backend should persist a version");
    }

    fn attr(collection: &Node, key: &str) -> Option<String> {
        collection
            .attrs()
            .optional_string(key)
            .map(|value| value.into_owned())
    }

    #[tokio::test]
    async fn an_empty_request_is_a_noop_and_never_reaches_the_wire() {
        let (client, transport) = crate::test_utils::create_iq_test_client().await;

        let report = client
            .resync_app_state(std::iter::empty(), AppStateResyncMode::Incremental)
            .await
            .expect("an empty resync asks for nothing and cannot fail");

        assert!(report.all_synced(), "nothing was asked, nothing is stale");
        assert!(report.synced.is_empty());
        assert!(
            transport.sent().is_empty(),
            "an empty request must not send an IQ"
        );
    }

    /// Winning the wait does not mean the connection survives to the first IQ.
    /// Reporting every collection retryable for a socket that retired underneath
    /// answers a question about the server with an outcome the server never saw,
    /// and reads to the caller as an attempted repair rather than a reconnect
    /// race.
    #[tokio::test]
    async fn a_connection_retired_before_the_first_iq_is_an_error() {
        let (client, transport) = create_reachable_client().await;
        // Held so the resync parks on the reservation, giving the test a moment
        // where the request is admitted but nothing has reached the wire.
        let held = client
            .app_state_syncing
            .try_begin_as(WAPatchName::Regular, SyncHolder::PatchSend)
            .expect("reserve the collection first");

        let resync = {
            let client = Arc::clone(&client);
            tokio::spawn(async move {
                client
                    .resync_app_state([WAPatchName::Regular], AppStateResyncMode::Incremental)
                    .await
            })
        };
        crate::test_utils::poll_until("the resync to park behind the holder", || {
            client.app_state_syncing.released.total_listeners() >= 1
        })
        .await;

        // The socket this request was admitted on is replaced.
        client.connection_generation.fetch_add(1, Ordering::SeqCst);
        drop(held);

        let error = tokio::time::timeout(Duration::from_secs(5), resync)
            .await
            .expect("the resync must not wait on a connection that is gone")
            .expect("the resync task should not panic")
            .expect_err("a request that never reached the server is not a report");

        assert!(
            matches!(error, AppStateError::NotConnected),
            "expected NotConnected, got {error:?}"
        );
        assert!(
            transport.sent().is_empty(),
            "and nothing may have reached the wire"
        );
    }

    /// The scope can still be live when `send_iq` refuses the request for want of
    /// a socket — the wait was won, the generation never changed, and the socket
    /// went between. Surfacing that as `Internal` would leave the caller
    /// unwrapping an error chain to tell a reconnect from a real failure.
    #[tokio::test]
    async fn an_iq_refused_for_want_of_a_socket_is_not_connected() {
        let (client, transport) = create_reachable_client().await;
        // Parks the resync past `await_connection` with its scope still valid,
        // which is the only place the socket can go without the checks noticing.
        let held = client
            .app_state_syncing
            .try_begin_as(WAPatchName::Regular, SyncHolder::PatchSend)
            .expect("reserve the collection first");

        let resync = {
            let client = Arc::clone(&client);
            tokio::spawn(async move {
                client
                    .resync_app_state([WAPatchName::Regular], AppStateResyncMode::Incremental)
                    .await
            })
        };
        crate::test_utils::poll_until("the resync to park behind the holder", || {
            client.app_state_syncing.released.total_listeners() >= 1
        })
        .await;

        // Not the generation: leaving it alone is what makes `send_iq` the first
        // thing to notice, rather than the scope check before it.
        client.is_running.store(false, Ordering::Release);
        drop(held);

        let error = tokio::time::timeout(Duration::from_secs(5), resync)
            .await
            .expect("the resync must not wait on a socket that is gone")
            .expect("the resync task should not panic")
            .expect_err("an IQ that was never sent is not a report");

        assert!(
            matches!(error, AppStateError::NotConnected),
            "expected NotConnected, got {error:?}"
        );
        assert!(
            transport.sent().is_empty(),
            "and nothing may have reached the wire"
        );
    }

    /// `Unknown` is what parsing an unrecognised name yields, so the server has
    /// nothing under it. Rejected whole rather than filtered out: a request that
    /// quietly syncs the rest reports success for a batch it did not carry out.
    #[tokio::test]
    async fn a_request_naming_the_unknown_collection_is_refused() {
        let (client, transport) = create_reachable_client().await;

        let error = client
            .resync_app_state(
                [WAPatchName::Regular, WAPatchName::Unknown],
                AppStateResyncMode::Incremental,
            )
            .await
            .expect_err("`unknown` is not a collection the server can answer for");

        assert!(
            matches!(error, AppStateError::InvalidRequest(_)),
            "expected InvalidRequest, got {error:?}"
        );
        assert!(
            transport.sent().is_empty(),
            "nothing may reach the wire, including the collections named alongside it"
        );
    }

    /// The wait answers "no connection is coming" for a client that is not
    /// running. Reporting every collection retryable would describe that as a
    /// transient miss the caller should re-ask for.
    #[tokio::test]
    async fn a_client_that_cannot_reach_the_server_is_an_error() {
        let client = crate::test_utils::create_test_client_with_name("resync-offline").await;

        let error = client
            .resync_app_state([WAPatchName::Regular], AppStateResyncMode::Incremental)
            .await
            .expect_err("a client with no connection cannot resync");

        assert!(
            matches!(error, AppStateError::NotConnected),
            "expected NotConnected, got {error:?}"
        );
    }

    /// The whole point of [`AppStateResyncMode::Snapshot`]: the persisted
    /// version is not zero, so every other sync path would ask for patches
    /// after it and never re-read what is already stored.
    #[tokio::test]
    async fn snapshot_mode_asks_for_a_snapshot_past_a_persisted_version() {
        let (client, transport) = create_reachable_client().await;
        persist_version(&client, WAPatchName::Regular, 42).await;

        let (result, asked) = resync_against(
            &client,
            &transport,
            vec![WAPatchName::Regular],
            AppStateResyncMode::Snapshot,
            |_, id| batch_result(id, &[("regular", None)]),
        )
        .await;

        let report = result.expect("the server answered");
        assert_eq!(report.synced, vec![WAPatchName::Regular]);
        assert_eq!(asked.len(), 1, "one collection, one round");
        assert_eq!(attr(&asked[0], "return_snapshot").as_deref(), Some("true"));
        assert_eq!(
            attr(&asked[0], "version"),
            None,
            "a snapshot request carries no version"
        );
    }

    /// The processor discards a snapshot that is not strictly newer than the
    /// persisted version — which is exactly the case this mode exists for — so
    /// the request has to travel with the response as
    /// `PatchList::requested_snapshot`. Nothing persisted may change before the
    /// answer arrives: a caller that bounds the wait with a timeout would
    /// otherwise be left, on every cancellation, with a collection stranded on a
    /// version its stored mutation MACs no longer match.
    #[tokio::test]
    async fn snapshot_mode_writes_nothing_before_the_response() {
        let (client, transport) = create_reachable_client().await;
        persist_version(&client, WAPatchName::Regular, 42).await;

        let resync = {
            let client = Arc::clone(&client);
            tokio::spawn(async move {
                client
                    .resync_app_state([WAPatchName::Regular], AppStateResyncMode::Snapshot)
                    .await
            })
        };

        // The IQ is on the wire and unanswered: everything the request could
        // have written, it has written by now.
        let sent = crate::test_utils::decode_sent_iq(&transport, 0).await;
        let persisted = client
            .persistence_manager
            .backend()
            .get_version(WAPatchName::Regular.as_str())
            .await
            .expect("the version should be readable");
        assert_eq!(
            persisted.version, 42,
            "a request in flight must not have moved the persisted version"
        );

        let id = sent
            .get()
            .attrs()
            .optional_string("id")
            .expect("every IQ carries an id")
            .into_owned();
        crate::test_utils::answer_iq(&client, &id, &batch_result(&id, &[("regular", None)])).await;
        resync
            .await
            .expect("the resync task should not panic")
            .expect("the server answered");
    }

    /// The incremental mode must not touch persisted state: it asks for what
    /// comes after the version, so discarding it would turn a cheap repair into
    /// a full re-download.
    #[tokio::test]
    async fn incremental_mode_asks_from_the_persisted_version() {
        let (client, transport) = create_reachable_client().await;
        persist_version(&client, WAPatchName::Regular, 42).await;

        let (result, asked) = resync_against(
            &client,
            &transport,
            vec![WAPatchName::Regular],
            AppStateResyncMode::Incremental,
            |_, id| batch_result(id, &[("regular", None)]),
        )
        .await;

        result.expect("the server answered");
        assert_eq!(asked.len(), 1);
        assert_eq!(attr(&asked[0], "return_snapshot").as_deref(), Some("false"));
        assert_eq!(attr(&asked[0], "version").as_deref(), Some("42"));
        let persisted = client
            .persistence_manager
            .backend()
            .get_version(WAPatchName::Regular.as_str())
            .await
            .expect("the version should be readable");
        assert_eq!(
            persisted.version, 42,
            "an incremental resync must leave the persisted version alone"
        );
    }

    /// A snapshot answers with the whole collection and moves the version to the
    /// server's head, so asking for one again on the refetch round would discard
    /// the page just applied and re-page the collection from scratch for as long
    /// as the server keeps setting `has_more_patches`.
    #[tokio::test]
    async fn the_snapshot_request_is_spent_on_the_first_round() {
        let (client, transport) = create_reachable_client().await;
        persist_version(&client, WAPatchName::Regular, 42).await;

        let (result, asked) = resync_against(
            &client,
            &transport,
            vec![WAPatchName::Regular],
            AppStateResyncMode::Snapshot,
            |frame, id| {
                // Only the first round has more to send, so the second is the
                // refetch and a third would mean the request never cleared.
                let has_more = frame == 0;
                NodeBuilder::new("iq")
                    .attr("type", "result")
                    .attr("id", id)
                    .attr("from", "s.whatsapp.net")
                    .children([NodeBuilder::new("sync")
                        .children([NodeBuilder::new("collection")
                            .attr("name", "regular")
                            .attr("has_more_patches", if has_more { "true" } else { "false" })
                            .build()])
                        .build()])
                    .build()
            },
        )
        .await;

        result.expect("the server answered");
        assert_eq!(asked.len(), 2, "one refetch round after the snapshot");
        assert_eq!(attr(&asked[0], "return_snapshot").as_deref(), Some("true"));
        assert_eq!(
            attr(&asked[1], "return_snapshot").as_deref(),
            Some("false"),
            "the refetch must resume from the version the response left persisted"
        );
        assert_eq!(
            attr(&asked[1], "version").as_deref(),
            Some("42"),
            "and that version is the one nothing in this run had reason to move"
        );
    }

    /// A caller can name a collection twice — deduplicating it is what keeps the
    /// same collection from being applied twice in one response.
    #[tokio::test]
    async fn a_repeated_collection_is_asked_for_once() {
        let (client, transport) = create_reachable_client().await;

        let (result, asked) = resync_against(
            &client,
            &transport,
            vec![WAPatchName::Regular, WAPatchName::Regular],
            AppStateResyncMode::Incremental,
            |_, id| batch_result(id, &[("regular", None)]),
        )
        .await;

        let report = result.expect("the server answered");
        assert_eq!(report.synced, vec![WAPatchName::Regular]);
        assert_eq!(asked.len(), 1, "the duplicate must not reach the wire");
    }

    /// Both batches wait for whoever holds a collection, and each holds every
    /// guard it has taken while waiting for the next one — so overlapping
    /// requests named in opposite orders could each hold the other's next
    /// collection and make no progress until both waits timed out, minutes
    /// later, for work neither was slow at. Reserving in one order over a shared
    /// set is what makes that cycle unconstructible, and the request the batch
    /// puts on the wire is that order.
    #[tokio::test]
    async fn collections_are_reserved_in_a_fixed_order_whatever_the_caller_asked() {
        let (client, transport) = create_reachable_client().await;

        let (result, asked) = resync_against(
            &client,
            &transport,
            // Deliberately not the canonical order.
            vec![WAPatchName::RegularHigh, WAPatchName::Regular],
            AppStateResyncMode::Incremental,
            |_, id| batch_result(id, &[("regular", None), ("regular_high", None)]),
        )
        .await;

        result.expect("the server answered");
        let order: Vec<Option<String>> = asked.iter().map(|c| attr(c, "name")).collect();
        assert_eq!(
            order,
            vec![Some("regular".to_owned()), Some("regular_high".to_owned())],
            "the batch must reserve, and ask, in name order rather than the caller's"
        );
    }

    /// 400/404 is the server's answer, not a transport failure. Raising it as
    /// `Err` would hide which collection it was about, and invite the caller to
    /// re-ask for something that will be refused again.
    #[tokio::test]
    async fn a_refused_collection_is_reported_not_raised() {
        let (client, transport) = create_reachable_client().await;

        let (result, _) = resync_against(
            &client,
            &transport,
            vec![WAPatchName::CriticalBlock, WAPatchName::Regular],
            AppStateResyncMode::Incremental,
            |_, id| batch_result(id, &[("critical_block", Some("404")), ("regular", None)]),
        )
        .await;

        let report = result.expect("a refused collection is an outcome, not an error");
        assert_eq!(report.fatal, vec![WAPatchName::CriticalBlock]);
        assert_eq!(report.synced, vec![WAPatchName::Regular]);
        assert!(!report.all_synced());
        assert_eq!(
            report.unsynced().collect::<Vec<_>>(),
            vec![WAPatchName::CriticalBlock]
        );
    }

    /// The reason this routes through the batched path instead of calling the
    /// single-collection sync directly: unreserved, a resync writes the same
    /// version and mutation-MAC rows as a concurrent sync or patch send, and the
    /// interleaved writes leave the ltHash disagreeing with the MAC store.
    #[tokio::test]
    async fn a_resync_waits_for_a_patch_send_holding_the_collection() {
        let (client, transport) = create_reachable_client().await;
        let held = client
            .app_state_syncing
            .try_begin_as(WAPatchName::Regular, SyncHolder::PatchSend)
            .expect("reserve the collection for a patch send first");

        let resync = {
            let client = Arc::clone(&client);
            tokio::spawn(async move {
                client
                    .resync_app_state([WAPatchName::Regular], AppStateResyncMode::Snapshot)
                    .await
            })
        };

        crate::test_utils::poll_until("the resync to park behind the patch send", || {
            client.app_state_syncing.released.total_listeners() >= 1
        })
        .await;
        assert!(
            !resync.is_finished(),
            "a resync must wait for a patch send, not skip it"
        );
        assert!(
            transport.sent().is_empty(),
            "nothing may reach the wire while another writer holds the collection"
        );

        drop(held);
        let answered = AtomicUsize::new(0);
        let server = async {
            let mut frame = 0usize;
            loop {
                let node = crate::test_utils::decode_sent_iq(&transport, frame).await;
                let id = node
                    .get()
                    .attrs()
                    .optional_string("id")
                    .expect("every IQ carries an id")
                    .into_owned();
                answered.fetch_add(1, Ordering::Relaxed);
                crate::test_utils::answer_iq(
                    &client,
                    &id,
                    &batch_result(&id, &[("regular", None)]),
                )
                .await;
                frame += 1;
            }
        };
        futures::pin_mut!(server);
        use futures::FutureExt;
        let mut resync = resync;
        let report = futures::select! {
            result = (&mut resync).fuse() => result
                .expect("the resync task should not panic")
                .expect("the server answered"),
            () = server.as_mut().fuse() => unreachable!("the responder never completes"),
        };

        assert_eq!(report.synced, vec![WAPatchName::Regular]);
        assert_eq!(
            answered.load(Ordering::Relaxed),
            1,
            "releasing the send must let the resync run, exactly once"
        );
    }
}

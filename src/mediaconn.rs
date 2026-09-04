//! Media connection management.
//!
//! Protocol types are defined in `wacore::iq::mediaconn`.

use crate::client::Client;
use crate::http::{HTTP_STATUS_FORBIDDEN, HTTP_STATUS_UNAUTHORIZED};
use crate::request::{IqError, RejectionStanza};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;
use wacore::iq::mediaconn::MediaConnSpec;
use wacore::time::Instant;

/// Re-export protocol types from wacore.
pub use wacore::iq::mediaconn::{HostType, MediaConnHost};

/// Number of retry attempts after a media auth error (401/403).
/// On auth failure, the media connection is invalidated and refreshed before retrying.
pub(crate) const MEDIA_AUTH_REFRESH_RETRY_ATTEMPTS: usize = 1;

/// Returns `true` if the HTTP status code indicates a media auth error
/// that should trigger a media connection refresh and retry.
pub(crate) fn is_media_auth_error(status_code: u16) -> bool {
    matches!(
        status_code,
        HTTP_STATUS_UNAUTHORIZED | HTTP_STATUS_FORBIDDEN
    )
}

/// Media connection with runtime-specific fields.
#[derive(Debug, Clone)]
pub struct MediaConn {
    /// Authentication token for media operations.
    pub auth: String,
    /// Time-to-live in seconds for route info.
    pub ttl: u64,
    /// Time-to-live in seconds for auth token (may differ from route TTL).
    pub auth_ttl: Option<u64>,
    /// Available media hosts (sorted: primary first, fallback second).
    pub hosts: Vec<MediaConnHost>,
    /// When this connection info was fetched (runtime-specific).
    pub fetched_at: Instant,
}

impl MediaConn {
    /// Check if this connection info has expired.
    /// Uses the earlier of route TTL and auth TTL (auth may expire before routes).
    pub fn is_expired(&self) -> bool {
        let effective_ttl = self.auth_ttl.map_or(self.ttl, |at| self.ttl.min(at));
        self.fetched_at.elapsed() > Duration::from_secs(effective_ttl)
    }
}

impl Client {
    pub(crate) async fn invalidate_media_conn(&self) {
        *self.media_conn.write().await = None;
    }

    /// Claim the in-flight refresh, or become its leader.
    ///
    /// Synchronous and taken under one short lock, so two callers racing on
    /// an empty cache cannot both come away as leader. The lock is never held
    /// across an await.
    fn claim_media_conn_flight(&self) -> MediaConnClaim {
        let mut slot = self
            .media_conn_flight
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        match &*slot {
            Some(flight) => {
                flight.waiters.fetch_add(1, Ordering::AcqRel);
                MediaConnClaim::Joined(flight.clone())
            }
            None => {
                let (release, released) = async_channel::bounded(1);
                let flight = Arc::new(MediaConnFlight {
                    waiters: AtomicUsize::new(0),
                    result: std::sync::Mutex::new(None),
                    released,
                });
                *slot = Some(flight.clone());
                MediaConnClaim::Leader(MediaConnLease {
                    slot: self.media_conn_flight.clone(),
                    flight,
                    _release: release,
                })
            }
        }
    }

    /// The one round trip a flight performs, shared by every caller that
    /// joined it.
    ///
    /// The store is generation-gated, not last-writer-wins: a fetch that
    /// started earlier may complete later (a forced refresh bypasses the
    /// flight and runs alongside it), and publishing its answer then would
    /// clobber credentials a newer fetch already replaced. The fetch's own
    /// callers still get its result; only the shared cache keeps the newest
    /// starter's.
    async fn fetch_media_conn(&self) -> Result<MediaConn, IqError> {
        let seq = self.media_conn_seq.fetch_add(1, Ordering::AcqRel);
        let response = self.execute(MediaConnSpec::new()).await?;

        let new_conn = MediaConn {
            auth: response.auth,
            ttl: response.ttl,
            auth_ttl: response.auth_ttl,
            hosts: response.hosts,
            fetched_at: Instant::now(),
        };

        // Nothing started after this fetch: its answer is still the newest.
        // Otherwise a newer fetch owns the cache (or will, when it lands)
        // and this result is only good for its own callers.
        //
        // The check runs under the publication lock, not before it: a fetch
        // that passed the check and then waited on the lock could otherwise
        // land after a newer fetch that started and published in between.
        let mut write_guard = self.media_conn.write().await;
        #[cfg(test)]
        {
            // Deterministic TOCTOU reproduction: while set, a fetch parks
            // here holding the lock, so a test can start a newer fetch past
            // it and prove the parked answer cannot publish.
            if self.media_conn_test_block_store.load(Ordering::Acquire) {
                self.media_conn_test_in_store.fetch_add(1, Ordering::AcqRel);
                while self.media_conn_test_block_store.load(Ordering::Acquire) {
                    self.runtime.sleep(Duration::from_millis(1)).await;
                }
            }
        }
        if seq + 1 == self.media_conn_seq.load(Ordering::Acquire) {
            *write_guard = Some(new_conn.clone());
        }

        Ok(new_conn)
    }

    /// Read the cache without touching any flight. The happy path stays on
    /// this alone: one read lock, no channel, no allocation beyond the clone
    /// the caller was always owed.
    async fn cached_media_conn(&self) -> Option<MediaConn> {
        let guard = self.media_conn.read().await;
        match &*guard {
            Some(conn) if !conn.is_expired() => Some(conn.clone()),
            _ => None,
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "wa.media.refresh_conn",
            level = "debug",
            skip_all,
            fields(force),
            err(Debug)
        )
    )]
    pub async fn refresh_media_conn(&self, force: bool) -> Result<MediaConn, IqError> {
        // A forced refresh carries out-of-band knowledge that the cache is
        // bad (a 401/403 on the token it holds), so it never joins a flight:
        // that flight's request may have been issued before the rejection,
        // and reusing its answer would risk the rejected generation again.
        // This keeps the old contract exactly: force always sends.
        if force {
            return self.fetch_media_conn().await;
        }

        if let Some(conn) = self.cached_media_conn().await {
            return Ok(conn);
        }

        loop {
            match self.claim_media_conn_flight() {
                MediaConnClaim::Joined(flight) => {
                    match flight.wait().await {
                        // The flight's answer, no cache round trip: a waiter
                        // re-read could miss on an invalidate racing the
                        // release and serialize the whole burst into fresh
                        // fetches, one leader at a time.
                        Some(MediaFlightOutcome::Refreshed(conn)) => return Ok(conn),
                        Some(MediaFlightOutcome::Failed(class)) => {
                            return Err(class.into_error());
                        }
                        // The leader died before deciding: nothing was asked,
                        // so this caller asks for itself.
                        None => {}
                    }
                }
                MediaConnClaim::Leader(lease) => {
                    // A flight that finished between the miss above and this
                    // claim already refreshed the cache; fetching again would
                    // be the duplicate this flight exists to prevent.
                    if let Some(conn) = self.cached_media_conn().await {
                        drop(lease);
                        return Ok(conn);
                    }
                    match self.fetch_media_conn().await {
                        Ok(conn) => {
                            lease.finish(|| MediaFlightOutcome::Refreshed(conn.clone()));
                            return Ok(conn);
                        }
                        Err(error) => {
                            lease.finish(|| MediaFlightOutcome::Failed(FailureClass::of(&error)));
                            return Err(error);
                        }
                    }
                }
            }
        }
    }
}

/// What one shared refresh flight produced, for the callers that joined it.
///
/// Success carries the fetched connection, so a waiter needs no cache round
/// trip to learn the answer. Only built when someone is waiting (see
/// [`MediaConnLease::finish`]); the uncontended flight still allocates
/// nothing to report.
#[derive(Debug, Clone)]
enum MediaFlightOutcome {
    Refreshed(MediaConn),
    Failed(FailureClass),
}

/// The failure classes a waiter rebuilds faithfully. `IqError` is not `Clone`,
/// and what callers act on through `ErrorChainExt` is timeout vs. transport
/// gone vs. server refusal, so those three rebuild canonically and everything
/// else keeps its message under a shared-attempt wrapper.
#[derive(Debug, Clone)]
enum FailureClass {
    TimedOut,
    TransportGone,
    Refused {
        code: u16,
        text: String,
        error_type: Option<String>,
        backoff: Option<u32>,
        response: RejectionStanza,
    },
    Other(String),
}

impl FailureClass {
    fn of(error: &IqError) -> Self {
        if error.is_timeout() {
            Self::TimedOut
        } else if error.is_transport_unavailable() {
            Self::TransportGone
        } else if let IqError::ServerError {
            code,
            text,
            error_type,
            backoff,
            response,
        } = error
        {
            Self::Refused {
                code: *code,
                text: text.clone(),
                error_type: error_type.clone(),
                backoff: *backoff,
                response: response.clone(),
            }
        } else {
            Self::Other(error.to_string())
        }
    }

    fn into_error(self) -> IqError {
        match self {
            Self::TimedOut => IqError::Timeout,
            Self::TransportGone => IqError::NotConnected,
            Self::Refused {
                code,
                text,
                error_type,
                backoff,
                response,
            } => IqError::ServerError {
                code,
                text,
                error_type,
                backoff,
                response,
            },
            // The predicates all answer false for these, leader or waiter
            // alike; the message keeps the cause readable.
            Self::Other(message) => IqError::ParseError(anyhow::anyhow!(
                "media_conn refresh failed on a shared attempt: {message}"
            )),
        }
    }
}

/// The shared side of the one in-flight refresh. One slot, not a registry:
/// every caller wants the same connection, so there is nothing to key by.
pub(crate) struct MediaConnFlight {
    /// Callers admitted while the leader runs, counted under the slot lock.
    /// A waiter cancelled before the leader finishes stays counted, so the
    /// leader can publish an answer nobody reads. That costs one small enum,
    /// where the alternative is another ordering-sensitive path in a
    /// structure whose bugs would all be ordering bugs.
    waiters: AtomicUsize,
    /// Set by the leader before it releases. Absent means the leader produced
    /// nothing a waiter can act on, so a waiter takes its own turn.
    result: std::sync::Mutex<Option<MediaFlightOutcome>>,
    /// Closes when the leader's lease drops: on success, on failure, and on a
    /// cancelled or panicking leader alike, so no waiter can be stranded by an
    /// outcome the leader never got to report.
    released: async_channel::Receiver<std::convert::Infallible>,
}

impl MediaConnFlight {
    /// Wait for the leader to release, and read what it produced.
    async fn wait(&self) -> Option<MediaFlightOutcome> {
        let _ = self.released.recv().await;
        self.result
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone()
    }
}

/// What a caller got from claiming the flight slot.
enum MediaConnClaim {
    /// This caller runs the one round trip.
    Leader(MediaConnLease),
    /// Someone else is running it; wait on their flight.
    Joined(Arc<MediaConnFlight>),
}

/// The leader's side. Dropping it releases every waiter.
pub(crate) struct MediaConnLease {
    slot: Arc<std::sync::Mutex<Option<Arc<MediaConnFlight>>>>,
    flight: Arc<MediaConnFlight>,
    _release: async_channel::Sender<std::convert::Infallible>,
}

impl MediaConnLease {
    /// Close this flight to new joiners and, if anyone is waiting, publish
    /// what the round trip produced.
    ///
    /// Closing and deciding are atomic: a caller admitted after the leader
    /// decided nobody was waiting would find the flight closed and empty, and
    /// fetch again on the strength of a round trip that had just succeeded.
    ///
    /// `outcome` only runs when someone is actually waiting, so the
    /// uncontended call, which is most of them, never formats a failure
    /// message just to drop it.
    fn finish(self, outcome: impl FnOnce() -> MediaFlightOutcome) {
        let publish = {
            let mut slot = self
                .slot
                .lock()
                .unwrap_or_else(|poison| poison.into_inner());
            // Retired first, so nobody else can join; the count read after
            // it, still under the lock, therefore covers everyone who could.
            Self::retire(&mut slot, &self.flight);
            self.flight.waiters.load(Ordering::Acquire) > 0
        };

        // Built after the lock is released: a waiter reads only once the
        // flight is released, which is this lease being dropped after this
        // returns.
        if publish {
            *self
                .flight
                .result
                .lock()
                .unwrap_or_else(|poison| poison.into_inner()) = Some(outcome());
        }
    }

    /// Remove this flight, but only where the registered one is still ours:
    /// the slot may already belong to a later caller's flight.
    fn retire(slot: &mut Option<Arc<MediaConnFlight>>, flight: &Arc<MediaConnFlight>) {
        if let Some(registered) = slot
            && Arc::ptr_eq(registered, flight)
        {
            *slot = None;
        }
    }
}

impl Drop for MediaConnLease {
    fn drop(&mut self) {
        // Idempotent: a leader that reached `finish` is already gone from the
        // slot, and one that died never got there.
        let mut slot = self
            .slot
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        Self::retire(&mut slot, &self.flight);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{answer_iq, create_iq_test_client, decode_sent_iq, poll_until};
    use crate::transport::mock::CapturingMockTransport;
    use std::sync::Arc;
    use std::sync::atomic::Ordering as AtomicOrdering;
    use std::time::Duration;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::node::Node;

    /// Callers in one burst: parallel AppState collections plus simultaneous
    /// media downloads all ask for the connection at once on a cold cache.
    const BURST: usize = 8;

    fn media_conn_result(id: &str, auth: &str, ttl: u64) -> Node {
        NodeBuilder::new("iq")
            .attr("type", "result")
            .attr("id", id)
            .attr("from", "s.whatsapp.net")
            .children([NodeBuilder::new("media_conn")
                .attr("auth", auth)
                .attr("ttl", ttl)
                .children([NodeBuilder::new("host")
                    .attr("hostname", "mmg.whatsapp.net")
                    .build()])
                .build()])
            .build()
    }

    fn media_conn_error(id: &str, code: u16) -> Node {
        NodeBuilder::new("iq")
            .attr("type", "error")
            .attr("id", id)
            .attr("from", "s.whatsapp.net")
            .children([NodeBuilder::new("error")
                .attr("code", code.to_string())
                .attr("text", "internal server error")
                .build()])
            .build()
    }

    fn request_id(sent: &Arc<wacore_binary::OwnedNodeRef>) -> String {
        sent.get()
            .to_owned()
            .attrs()
            .optional_string("id")
            .expect("every IQ carries an id")
            .into_owned()
    }

    /// The burst reaches the wire and then goes quiet: every caller that is
    /// going to fetch has fetched, and the rest are parked on responses.
    /// Returns the frame count at quiescence, which is the measurement this
    /// suite pins down.
    async fn quiesce(transport: &Arc<CapturingMockTransport>) -> usize {
        tokio::time::timeout(Duration::from_secs(5), async {
            let mut last = usize::MAX;
            loop {
                tokio::time::sleep(Duration::from_millis(300)).await;
                let now = transport.sent().len();
                if now == last && now > 0 {
                    return now;
                }
                last = now;
            }
        })
        .await
        .expect("the burst must reach the wire")
    }

    async fn answer_frame(
        client: &Arc<Client>,
        transport: &Arc<CapturingMockTransport>,
        index: usize,
        auth: &str,
        ttl: u64,
    ) {
        let sent = decode_sent_iq(transport, index).await;
        assert!(
            sent.get().get_optional_child("media_conn").is_some(),
            "frame {index} must be the media_conn query"
        );
        let id = request_id(&sent);
        answer_iq(client, &id, &media_conn_result(&id, auth, ttl)).await;
    }

    fn spawn_burst(
        client: &Arc<Client>,
    ) -> tokio::task::JoinHandle<Vec<Result<MediaConn, IqError>>> {
        let client = client.clone();
        tokio::spawn(async move {
            futures::future::join_all((0..BURST).map(|_| client.refresh_media_conn(false))).await
        })
    }

    /// Eight callers on an empty cache must send one `<media_conn/>` IQ
    /// between them and all come home with the same connection.
    #[tokio::test]
    async fn concurrent_refreshes_share_a_single_wire_request() {
        let (client, transport) = create_iq_test_client().await;
        let mut pending = spawn_burst(&client);

        let dispatched = quiesce(&transport).await;
        answer_frame(&client, &transport, 0, "shared-auth", 3600).await;

        match tokio::time::timeout(Duration::from_secs(5), &mut pending).await {
            Ok(done) => {
                let results = done.expect("the burst task stays alive");
                assert!(
                    results
                        .iter()
                        .all(|r| matches!(r, Ok(conn) if conn.auth == "shared-auth")),
                    "every caller shares the one flight's connection"
                );
                assert_eq!(
                    transport.sent().len(),
                    1,
                    "concurrent refreshes on an empty cache must share one flight"
                );
            }
            Err(_) => {
                pending.abort();
                panic!(
                    "no single-flight: {dispatched} media_conn IQs reached the wire \
                     for {BURST} concurrent refreshes (expected 1)"
                );
            }
        }
    }

    /// An expired cache is a miss, but still one miss: the burst after a
    /// `ttl: 0` seed sends exactly one more IQ.
    #[tokio::test]
    async fn an_expired_cache_still_flies_single() {
        let (client, transport) = create_iq_test_client().await;

        let seed = tokio::spawn({
            let client = client.clone();
            async move { client.refresh_media_conn(false).await }
        });
        answer_frame(&client, &transport, 0, "stale-auth", 0).await;
        let seeded = tokio::time::timeout(Duration::from_secs(5), seed)
            .await
            .expect("the seed refresh must complete")
            .expect("the seed task stays alive")
            .expect("the seed refresh succeeds");
        assert!(seeded.is_expired(), "ttl 0 is expired on arrival");

        let mut pending = spawn_burst(&client);
        quiesce(&transport).await;
        answer_frame(&client, &transport, 1, "fresh-auth", 3600).await;

        match tokio::time::timeout(Duration::from_secs(5), &mut pending).await {
            Ok(done) => {
                let results = done.expect("the burst task stays alive");
                assert!(
                    results
                        .iter()
                        .all(|r| matches!(r, Ok(conn) if conn.auth == "fresh-auth")),
                    "every caller shares the one refetch"
                );
                assert_eq!(
                    transport.sent().len(),
                    2,
                    "seed plus one shared refetch, nothing more"
                );
            }
            Err(_) => {
                pending.abort();
                panic!(
                    "no single-flight: {} media_conn IQs reached the wire \
                     for the seed plus {BURST} concurrent refreshes (expected 2)",
                    transport.sent().len(),
                );
            }
        }
    }

    /// One failed flight fails everyone with the same error: the refusal is
    /// answered once, every caller reports it, and a later call retries on a
    /// cleared slot.
    ///
    /// Answered with a server refusal rather than an injected transport
    /// failure: a failed write poisons the noise sender, which would leave no
    /// healthy socket for the retry round to prove the slot cleared.
    #[tokio::test]
    async fn a_failed_flight_fails_everyone_once_and_retries_after() {
        let (client, transport) = create_iq_test_client().await;
        let mut pending = spawn_burst(&client);

        let dispatched = quiesce(&transport).await;
        let sent = decode_sent_iq(&transport, 0).await;
        let id = request_id(&sent);
        answer_iq(&client, &id, &media_conn_error(&id, 500)).await;

        match tokio::time::timeout(Duration::from_secs(5), &mut pending).await {
            Ok(done) => {
                let results = done.expect("the burst task stays alive");
                assert!(
                    results.iter().all(|r| r.is_err()),
                    "a failed flight reports its error to every caller"
                );
                let first = results[0].as_ref().expect_err("checked above").to_string();
                assert!(
                    first.contains("500"),
                    "the shared error carries the refusal, got: {first}"
                );
                assert!(
                    results
                        .iter()
                        .all(|r| r.as_ref().expect_err("checked above").to_string() == first),
                    "waiters share the flight's error, they do not mint their own"
                );
                assert_eq!(
                    transport.sent().len(),
                    1,
                    "one shared flight sends one query even when refused"
                );
            }
            Err(_) => {
                pending.abort();
                panic!(
                    "no single-flight: {dispatched} media_conn IQs reached the wire \
                     for {BURST} concurrent refreshes (expected 1)"
                );
            }
        }

        let retry = tokio::spawn({
            let client = client.clone();
            async move { client.refresh_media_conn(false).await }
        });
        answer_frame(&client, &transport, 1, "recovered-auth", 3600).await;
        let recovered = tokio::time::timeout(Duration::from_secs(5), retry)
            .await
            .expect("the retry must complete")
            .expect("the retry task stays alive")
            .expect("the retry succeeds after the refusal clears");
        assert_eq!(recovered.auth, "recovered-auth");
    }

    /// A forced refresh never joins a running flight: it carries out-of-band
    /// knowledge that the cache is bad (a 401/403 on the token it holds), so
    /// it always sends its own request, exactly as before single-flight.
    #[tokio::test]
    async fn a_forced_refresh_bypasses_a_running_flight() {
        let (client, transport) = create_iq_test_client().await;

        // Park a non-force refresh mid-flight: one IQ on the wire, unanswered.
        let first = tokio::spawn({
            let client = client.clone();
            async move { client.refresh_media_conn(false).await }
        });
        quiesce(&transport).await;

        // The forced refresh must send its own IQ, not join frame 0.
        let forced = tokio::spawn({
            let client = client.clone();
            async move { client.refresh_media_conn(true).await }
        });
        let sent = decode_sent_iq(&transport, 1).await;
        let forced_id = request_id(&sent);
        answer_iq(
            &client,
            &forced_id,
            &media_conn_result(&forced_id, "forced-auth", 3600),
        )
        .await;

        let sent_first = decode_sent_iq(&transport, 0).await;
        let first_id = request_id(&sent_first);
        answer_iq(
            &client,
            &first_id,
            &media_conn_result(&first_id, "flight-auth", 3600),
        )
        .await;

        let (first_conn, forced_conn) = tokio::time::timeout(Duration::from_secs(5), async {
            (
                first.await.expect("the first task stays alive"),
                forced.await.expect("the forced task stays alive"),
            )
        })
        .await
        .expect("both refreshes complete");
        assert!(
            matches!(first_conn, Ok(ref c) if c.auth == "flight-auth"),
            "the parked flight answers with its own fetch"
        );
        assert!(
            matches!(forced_conn, Ok(ref c) if c.auth == "forced-auth"),
            "the forced refresh answers with its own request"
        );
        assert_eq!(
            transport.sent().len(),
            2,
            "force sends its own request instead of joining"
        );
        // The older flight completes last (its frame is answered second) and
        // must not clobber the forced credentials: the cache keeps the newest
        // starter's answer, while the older flight's own caller still gets
        // what it fetched.
        assert_eq!(
            client
                .cached_media_conn()
                .await
                .expect("the forced fetch publishes")
                .auth,
            "forced-auth",
            "an older flight completing last must not overwrite the forced refresh"
        );
    }

    /// An older fetch parked between lock acquisition and publication must
    /// not publish once a newer fetch began: the sequence re-check runs
    /// under the same lock, so the parked answer cannot slip in after the
    /// forced one even when the scheduler deschedules it mid-publication.
    #[tokio::test]
    async fn an_older_fetch_parked_at_publication_cannot_overwrite_a_newer_one() {
        use AtomicOrdering as Ordering;

        let (client, transport) = create_iq_test_client().await;
        client
            .media_conn_test_block_store
            .store(true, Ordering::Release);

        // Older fetch: answered, then parked inside publication holding the
        // lock (its sequence check has not run yet).
        let older = tokio::spawn({
            let client = client.clone();
            async move { client.refresh_media_conn(false).await }
        });
        answer_frame(&client, &transport, 0, "older-auth", 3600).await;
        poll_until("the older fetch parks at publication", || {
            client.media_conn_test_in_store.load(Ordering::Acquire) > 0
        })
        .await;

        // A newer forced fetch starts (bumping the sequence) while the older
        // one is parked, but cannot publish yet: the older fetch holds the lock.
        let newer = tokio::spawn({
            let client = client.clone();
            async move { client.refresh_media_conn(true).await }
        });
        let sent = decode_sent_iq(&transport, 1).await;
        let newer_id = request_id(&sent);

        // Release the older fetch with the newer one started but unpublished:
        // it must observe the newer sequence and skip its publication, while
        // still answering its own caller with what it fetched.
        client
            .media_conn_test_block_store
            .store(false, Ordering::Release);
        let older_conn = tokio::time::timeout(Duration::from_secs(5), older)
            .await
            .expect("the older fetch must finish once released")
            .expect("the older task stays alive")
            .expect("the older fetch answers its own caller");
        assert_eq!(older_conn.auth, "older-auth");
        assert!(
            client.cached_media_conn().await.is_none(),
            "the parked older fetch must publish nothing once a newer fetch began"
        );

        // The newer fetch then publishes normally.
        answer_iq(
            &client,
            &newer_id,
            &media_conn_result(&newer_id, "forced-auth", 3600),
        )
        .await;
        let newer_conn = tokio::time::timeout(Duration::from_secs(5), newer)
            .await
            .expect("the newer fetch must complete")
            .expect("the newer task stays alive")
            .expect("the newer fetch succeeds");
        assert_eq!(newer_conn.auth, "forced-auth");
        assert_eq!(
            client
                .cached_media_conn()
                .await
                .expect("the newer fetch publishes")
                .auth,
            "forced-auth"
        );
    }
}

//! Media reupload feature: request the server to re-upload expired media.
//!
//! When a media download fails because the URL has expired, this feature
//! sends a `<receipt type="server-error">` stanza and waits for a
//! `<notification type="mediaretry">` response with a new `directPath`.
//!
//! Reference: WAWebRequestMediaReuploadManager.

use crate::client::{Client, ClientError, NodeFilter};
use log::debug;
use std::time::Duration;
use thiserror::Error;
pub use wacore::media_retry::MediaRetryResult;
use wacore::media_retry::{
    build_media_retry_receipt, encrypt_media_retry_receipt, parse_media_retry_notification,
};
use wacore_binary::{Jid, JidExt as _};

const MEDIA_RETRY_TIMEOUT: Duration = Duration::from_secs(30);

/// Max media-reupload requests in flight for [`MediaReupload::request_many`].
/// The per-item work is I/O-light (send a small receipt, park on a notification
/// waiter), so a generous window lets the waits overlap — bulk recovery after a
/// long offline period completes in ~one timeout instead of the sum — while
/// still bounding how many receipts hit the socket/server at once. WA Web caps
/// media work at a similar order (its `ConcurrentPriorityPromiseQueue`).
const MEDIA_REUPLOAD_CONCURRENCY: usize = 32;

/// Error returned by the media reupload request flow.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum MediaReuploadError {
    /// Connection/transport failure sending the server-error receipt.
    #[error(transparent)]
    Client(#[from] ClientError),
    /// The client is not logged in.
    #[error("client is not logged in")]
    NotLoggedIn,
    /// The request is not applicable to this message (e.g. a newsletter message
    /// carries no media keys).
    #[error("invalid media reupload request: {0}")]
    InvalidRequest(String),
    /// The server did not return a `mediaretry` notification in time.
    #[error("media retry notification timed out")]
    Timeout,
    /// Catch-all for internal failures (receipt encryption, response parsing).
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

/// Parameters for a media reupload request.
pub struct MediaReuploadRequest<'a> {
    /// The message ID containing the media.
    pub msg_id: &'a str,
    /// The chat JID where the message was received.
    pub chat_jid: &'a Jid,
    /// The raw media key bytes (32 bytes, from the message's `mediaKey` field).
    pub media_key: &'a [u8],
    /// Whether the message was sent by us.
    pub is_from_me: bool,
    /// For group/broadcast messages, the participant JID who sent the message.
    pub participant: Option<&'a Jid>,
}

pub struct MediaReupload<'a> {
    client: &'a Client,
}

impl<'a> MediaReupload<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Request the server to re-upload media for a message with an expired URL.
    ///
    /// Returns the new `directPath` on success, or an error variant indicating
    /// why the reupload failed.
    ///
    /// # Protocol flow
    /// 1. Encrypt `ServerErrorReceipt` protobuf with HKDF-derived key from media key
    /// 2. Send `<receipt type="server-error">` with encrypted payload + `<rmr>` metadata
    /// 3. Wait for `<notification type="mediaretry">` response
    /// 4. Decrypt response and extract new `directPath`
    pub async fn request(
        &self,
        req: &MediaReuploadRequest<'_>,
    ) -> Result<MediaRetryResult, MediaReuploadError> {
        // WA Web: ServerErrorReceiptJob rejects newsletter messages (no media keys).
        if req.chat_jid.is_newsletter() {
            return Err(MediaReuploadError::InvalidRequest(
                "media reupload is not supported for newsletter messages".into(),
            ));
        }

        debug!(
            "[media][rmr] Requesting media reupload for msg {} in chat {}",
            req.msg_id, req.chat_jid
        );

        // Encrypt the ServerErrorReceipt
        let (ciphertext, iv) = encrypt_media_retry_receipt(req.media_key, req.msg_id)?;

        // Get own JID for the receipt's `to` attribute
        let device_snapshot = self.client.persistence_manager.get_device_snapshot();
        let own_jid = device_snapshot
            .pn
            .as_ref()
            .ok_or(MediaReuploadError::NotLoggedIn)?;

        // Register waiter BEFORE sending (to avoid race)
        let waiter = self.client.wait_for_node(
            NodeFilter::tag("notification")
                .attr("type", "mediaretry")
                .attr("id", req.msg_id),
        );

        // Build and send the receipt node
        let receipt_node = build_media_retry_receipt(
            own_jid,
            req.msg_id,
            req.chat_jid,
            req.is_from_me,
            req.participant,
            &ciphertext,
            &iv,
        );

        self.client.send_node(receipt_node).await?;

        debug!(
            "[media][rmr] Sent server-error receipt for {}, waiting for response",
            req.msg_id
        );

        // Wait for the mediaretry notification
        let notification_node =
            wacore::runtime::timeout(&*self.client.runtime, MEDIA_RETRY_TIMEOUT, waiter)
                .await
                .map_err(|_| MediaReuploadError::Timeout)?
                .map_err(|_| {
                    MediaReuploadError::Internal(anyhow::anyhow!("media retry waiter cancelled"))
                })?;

        debug!(
            "[media][rmr] Received mediaretry notification for {}",
            req.msg_id
        );

        // Parse and decrypt the response
        Ok(parse_media_retry_notification(
            notification_node.get(),
            req.media_key,
        )?)
    }

    /// Request reupload for several messages at once, concurrently.
    ///
    /// Each request registers its own notification waiter and awaits it
    /// independently, so a bulk recovery — e.g. many expired-URL media after a
    /// long offline period — runs `MEDIA_REUPLOAD_CONCURRENCY` at a time instead
    /// of paying the serial sum of per-item waits. A batch larger than that runs
    /// in waves, so the worst case is `ceil(len / MEDIA_REUPLOAD_CONCURRENCY)`
    /// `MEDIA_RETRY_TIMEOUT` windows. Results are returned in the
    /// same order as `reqs`; each entry carries that item's success or error
    /// (one item failing never aborts the others).
    ///
    /// Duplicate `msg_id`s in one batch are rejected (past the first occurrence)
    /// with [`MediaReuploadError::InvalidRequest`]. The `mediaretry` waiter
    /// filters on message id alone and `resolve_waiters` wakes *every* match, so
    /// two same-id waiters would cross-resolve. Serializing them isn't enough:
    /// once the first waiter times out it lingers in `node_waiters` (canceled
    /// waiters are purged only lazily), so its late notification could still
    /// resolve a same-id retry with the wrong payload. Requiring unique ids means
    /// no two waiters ever share a filter — a message id is unique per message,
    /// so a duplicate is a caller mistake, not a real recovery target.
    pub async fn request_many(
        &self,
        reqs: &[MediaReuploadRequest<'_>],
    ) -> Vec<Result<MediaRetryResult, MediaReuploadError>> {
        use futures::StreamExt;
        use std::collections::HashSet;
        if reqs.is_empty() {
            return Vec::new();
        }

        let mut results: Vec<Option<Result<MediaRetryResult, MediaReuploadError>>> =
            (0..reqs.len()).map(|_| None).collect();
        let mut seen: HashSet<&str> = HashSet::with_capacity(reqs.len());
        let mut unique: Vec<usize> = Vec::with_capacity(reqs.len());
        for (i, req) in reqs.iter().enumerate() {
            if seen.insert(req.msg_id) {
                unique.push(i);
            } else {
                results[i] = Some(Err(MediaReuploadError::InvalidRequest(format!(
                    "duplicate msg_id {} in batch",
                    req.msg_id
                ))));
            }
        }

        // Stream over owned indices (not a borrow of `reqs` through the
        // combinator) and index inside each task, so the fan-out future stays
        // Send. Every id here is unique, so no two waiters share a filter.
        let done: Vec<(usize, Result<MediaRetryResult, MediaReuploadError>)> =
            futures::stream::iter(unique)
                .map(|i| async move { (i, self.request(&reqs[i]).await) })
                .buffer_unordered(MEDIA_REUPLOAD_CONCURRENCY)
                .collect()
                .await;
        for (i, res) in done {
            results[i] = Some(res);
        }
        results
            .into_iter()
            .map(|res| res.expect("every index is either a duplicate or fetched"))
            .collect()
    }
}

impl Client {
    /// Access media reupload operations.
    pub fn media_reupload(&self) -> MediaReupload<'_> {
        MediaReupload::new(self)
    }
}

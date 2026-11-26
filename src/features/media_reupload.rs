//! Recover expired media with a correlated receipt ACK and mediaretry notification.
//!
//! Protocol reference: WAWebSendServerErrorReceiptJob and
//! WAWebRequestMediaReuploadManager, WA Web 2.3000.1045368834.

use crate::client::{Client, ClientError, NodeFilter};
use crate::request::RejectionStanza;
use futures::FutureExt;
use futures::future::{Shared, WeakShared};
use std::sync::{Arc, Weak};
use std::time::Duration;
pub use wacore::media_retry::MediaRetryResult;
use wacore::media_retry::{
    build_media_retry_receipt, encrypt_media_retry_receipt, parse_media_retry_notification,
};
use wacore::runtime::{BoxFuture, timeout};
use wacore::stanza::wire_tags::{NotificationType, StanzaTag};
use wacore::types::message_ref::{MessageId, MessageRef};
use wacore_binary::Jid;

const MEDIA_RETRY_TIMEOUT: Duration = Duration::from_secs(30);
const MEDIA_REUPLOAD_CONCURRENCY: usize = 32;

/// The bounded phase in which a reupload stopped making progress.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MediaReuploadPhase {
    /// Sending the receipt and waiting for its correlated ACK share one budget.
    SendAndAck,
    /// Waiting for the mediaretry notification after the receipt was accepted.
    Notification,
}

/// Failure of a media reupload. Shared operations preserve the original cause
/// for every subscriber, including transport and parsing failures.
#[derive(Clone)]
#[non_exhaustive]
pub enum MediaReuploadError {
    Client(Arc<ClientError>),
    /// Reupload requires the local account's LID. There is no PN fallback.
    NotLoggedIn,
    /// Another target or media key already owns this wire ID.
    Conflict(MessageId),
    Timeout {
        phase: MediaReuploadPhase,
    },
    /// The waiter channel closed. Dropping the caller's future produces no error.
    Cancelled {
        phase: MediaReuploadPhase,
    },
    /// A rejected receipt. Unknown or missing codes remain in the full response.
    Rejected {
        code: Option<u16>,
        response: RejectionStanza,
    },
    Retry(Arc<wacore::media_retry::MediaRetryError>),
    Store(Arc<wacore::store::error::StoreError>),
    ClientUnavailable,
}

impl std::fmt::Display for MediaReuploadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Client(error) => std::fmt::Display::fmt(error, f),
            Self::NotLoggedIn => f.write_str("local LID is unavailable"),
            Self::Conflict(id) => write!(f, "conflicting media reupload for message {id}"),
            Self::Timeout { phase } => write!(f, "media retry {phase:?} timed out"),
            Self::Cancelled { phase } => write!(f, "media retry {phase:?} waiter cancelled"),
            Self::Rejected { code, .. } => {
                write!(f, "media retry receipt rejected (code: {code:?})")
            }
            Self::Retry(error) => std::fmt::Display::fmt(error, f),
            Self::Store(error) => std::fmt::Display::fmt(error, f),
            Self::ClientUnavailable => f.write_str("client is no longer available"),
        }
    }
}

impl std::fmt::Debug for MediaReuploadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            // Opaque causes may include keys or signed URLs. Keep them available
            // through source(), but never format them implicitly in diagnostics.
            Self::Client(_) => f.write_str("Client([REDACTED])"),
            Self::Retry(_) => f.write_str("Retry([REDACTED])"),
            Self::Store(_) => f.write_str("Store([REDACTED])"),
            Self::ClientUnavailable => f.write_str("ClientUnavailable"),
            Self::NotLoggedIn => f.write_str("NotLoggedIn"),
            Self::Conflict(id) => f.debug_tuple("Conflict").field(id).finish(),
            Self::Timeout { phase } => f.debug_struct("Timeout").field("phase", phase).finish(),
            Self::Cancelled { phase } => f.debug_struct("Cancelled").field("phase", phase).finish(),
            Self::Rejected { code, response } => f
                .debug_struct("Rejected")
                .field("code", code)
                .field("response", response)
                .finish(),
        }
    }
}

impl std::error::Error for MediaReuploadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        // Expose the original typed cause, not the Arc used to share it.
        // A derived source on Arc<ClientError> exposes the Arc as the cause.
        match self {
            Self::Client(error) => Some(error.as_ref()),
            Self::Retry(error) => Some(error.as_ref()),
            Self::Store(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}

impl From<ClientError> for MediaReuploadError {
    fn from(error: ClientError) -> Self {
        Self::Client(Arc::new(error))
    }
}

impl From<wacore::media_retry::MediaRetryError> for MediaReuploadError {
    fn from(error: wacore::media_retry::MediaRetryError) -> Self {
        Self::Retry(Arc::new(error))
    }
}

impl From<wacore::store::error::StoreError> for MediaReuploadError {
    fn from(error: wacore::store::error::StoreError) -> Self {
        Self::Store(Arc::new(error))
    }
}

/// A validated message target and its media key. Media keys are distinct from
/// the message secrets used by add-ons. Debug never prints key bytes.
#[derive(Clone)]
pub struct MediaReuploadRequest<'a> {
    pub target: MessageRef<'a>,
    pub media_key: &'a [u8; 32],
}

impl std::fmt::Debug for MediaReuploadRequest<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MediaReuploadRequest")
            .field("target", &self.target)
            .field("media_key", &"[REDACTED]")
            .finish()
    }
}

#[derive(PartialEq, Eq)]
struct ReuploadTarget {
    id: MessageId,
    chat: Jid,
    participant: Option<Jid>,
    from_me: bool,
    media_key: [u8; 32],
}

type ReuploadFuture = BoxFuture<'static, Result<MediaRetryResult, MediaReuploadError>>;
#[cfg(not(target_arch = "wasm32"))]
type PendingReupload = WeakShared<ReuploadFuture>;
#[cfg(target_arch = "wasm32")]
type PendingReupload = send_wrapper::SendWrapper<WeakShared<ReuploadFuture>>;

pub(crate) struct MediaReuploadInFlight {
    target: Arc<ReuploadTarget>,
    // The registry must not own the future: the final subscriber's Drop must
    // release its waiters and key even when no more traffic arrives.
    future: PendingReupload,
}

struct ReuploadGuard {
    client: Weak<Client>,
    target: Arc<ReuploadTarget>,
}

impl Drop for ReuploadGuard {
    fn drop(&mut self) {
        if let Some(client) = self.client.upgrade() {
            let mut pending = client
                .media_reuploads
                .lock()
                .unwrap_or_else(|p| p.into_inner());
            if pending
                .get(self.target.id.as_str())
                .is_some_and(|entry| Arc::ptr_eq(&entry.target, &self.target))
            {
                pending.remove(self.target.id.as_str());
            }
        }
    }
}

pub struct MediaReupload<'a> {
    client: &'a Client,
}

impl<'a> MediaReupload<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    fn subscribe(
        &self,
        req: &MediaReuploadRequest<'_>,
    ) -> Result<Shared<ReuploadFuture>, MediaReuploadError> {
        let target = ReuploadTarget {
            id: req.target.id().clone(),
            chat: req.target.chat().to_non_ad(),
            participant: req.target.receipt_sender().map(Jid::to_non_ad),
            from_me: req.target.from_me(),
            media_key: *req.media_key,
        };
        let mut pending = self
            .client
            .media_reuploads
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if let Some(entry) = pending.get(target.id.as_str())
            && let Some(future) = entry.future.upgrade()
        {
            let identical = *entry.target == target;
            // Dropping the last Shared subscriber runs ReuploadGuard. Release
            // the map before dropping a conflicting temporary subscriber.
            drop(pending);
            return if identical {
                Ok(future)
            } else {
                Err(MediaReuploadError::Conflict(target.id))
            };
        }
        let client = self
            .client
            .self_weak
            .get()
            .and_then(Weak::upgrade)
            .ok_or(MediaReuploadError::ClientUnavailable)?;
        let target = Arc::new(target);
        let guard = ReuploadGuard {
            client: Arc::downgrade(&client),
            target: target.clone(),
        };
        let request = target.clone();
        let future: ReuploadFuture = Box::pin(async move {
            let _guard = guard;
            client.media_reupload().request_inner(&request).await
        });
        let future = future.shared();
        let weak = future.downgrade().expect("new shared future is pending");
        #[cfg(target_arch = "wasm32")]
        let weak = send_wrapper::SendWrapper::new(weak);
        pending.insert(
            target.id.to_string(),
            MediaReuploadInFlight {
                target,
                future: weak,
            },
        );
        Ok(future)
    }

    /// Recover one message. Concurrent identical requests share the operation.
    /// A conflicting target/key with the same wire ID returns `Conflict`.
    /// Cancelling one caller leaves other subscribers running; cancelling the
    /// last subscriber removes the operation and its node waiters immediately.
    pub async fn request(
        &self,
        req: &MediaReuploadRequest<'_>,
    ) -> Result<MediaRetryResult, MediaReuploadError> {
        self.subscribe(req)?.await
    }

    async fn request_inner(
        &self,
        req: &ReuploadTarget,
    ) -> Result<MediaRetryResult, MediaReuploadError> {
        let own_jid = self
            .client
            .persistence_manager
            .get_device_snapshot()
            .lid
            .as_ref()
            .ok_or(MediaReuploadError::NotLoggedIn)?
            .to_non_ad();
        // WAWebSendServerErrorReceiptJob only substitutes accountLid for a
        // migrated regular-user chat. A missing mapping keeps the original JID.
        let chat = if req.chat.is_pn() && self.client.is_lid_migrated().await {
            match self.client.get_lid_pn_entry(&req.chat).await? {
                Some(entry) => Jid::lid(entry.lid.as_ref()),
                None => req.chat.clone(),
            }
        } else {
            req.chat.clone()
        };
        // MessageRef requires the author for received group/broadcast messages.
        // Preserve the supplied sender; the Web job omits participant when its
        // message model has no sender (possible for an own-send reference).
        let participant = req.participant.as_ref();
        let (ciphertext, iv) = encrypt_media_retry_receipt(&req.media_key, req.id.as_str())?;
        let notification = self.client.wait_for_node(
            NodeFilter::tag(StanzaTag::Notification.as_str())
                .attr("type", NotificationType::MediaRetry.as_str())
                .attr("id", req.id.as_str()),
        );
        let ack = self.client.wait_for_node(
            NodeFilter::tag(StanzaTag::Ack.as_str())
                .attr("id", req.id.as_str())
                .attr("class", StanzaTag::Receipt.as_str())
                .attr("type", "server-error")
                .from_jid(&own_jid)
                .without_attr("participant"),
        );
        let receipt = build_media_retry_receipt(
            &own_jid,
            req.id.as_str(),
            &chat,
            req.from_me,
            participant,
            &ciphertext,
            &iv,
        );
        // Both subscriptions exist before send. A notification received during
        // send/ACK remains buffered, but cannot hide a rejected receipt.
        timeout(&*self.client.runtime, MEDIA_RETRY_TIMEOUT, async {
            self.client.send_node(receipt).await?;
            let ack = ack.await.map_err(|_| MediaReuploadError::Cancelled {
                phase: MediaReuploadPhase::SendAndAck,
            })?;
            if let Some(error) = ack.get().get_attr("error") {
                return Err(MediaReuploadError::Rejected {
                    code: error.as_str().parse().ok(),
                    response: ack.into(),
                });
            }
            if let Some(error) = ack.get().get_optional_child_by_tag(&["error"]) {
                return Err(MediaReuploadError::Rejected {
                    code: error
                        .get_attr("code")
                        .and_then(|code| code.as_str().parse().ok()),
                    response: ack.into(),
                });
            }
            Ok(())
        })
        .await
        .map_err(|_| MediaReuploadError::Timeout {
            phase: MediaReuploadPhase::SendAndAck,
        })??;
        let node = timeout(&*self.client.runtime, MEDIA_RETRY_TIMEOUT, notification)
            .await
            .map_err(|_| MediaReuploadError::Timeout {
                phase: MediaReuploadPhase::Notification,
            })?
            .map_err(|_| MediaReuploadError::Cancelled {
                phase: MediaReuploadPhase::Notification,
            })?;
        Ok(parse_media_retry_notification(node.get(), &req.media_key)?)
    }

    /// Recover a batch, preserving input order and cardinality. Identical inputs
    /// share one receipt even when separated by more than the concurrency window.
    /// The first occurrence reserves each wire ID; conflicting inputs receive
    /// `Conflict` in their own result slot. Up to 32 distinct operations run at
    /// once; duplicates share a slot and completed operations release it
    /// without waiting for earlier inputs.
    pub async fn request_many(
        &self,
        reqs: &[MediaReuploadRequest<'_>],
    ) -> Vec<Result<MediaRetryResult, MediaReuploadError>> {
        use futures::StreamExt;
        let mut results = vec![None; reqs.len()];
        let mut operations: Vec<(Shared<ReuploadFuture>, Vec<usize>)> = Vec::new();
        let mut by_id = std::collections::HashMap::<&str, usize>::new();
        // Reserve every input before polling: duplicates beyond the window
        // must share even if the first operation finishes immediately.
        for (index, req) in reqs.iter().enumerate() {
            match self.subscribe(req) {
                Err(error) => results[index] = Some(Err(error)),
                Ok(subscription) => {
                    if let Some(&operation) = by_id.get(req.target.id().as_str())
                        && operations[operation].0.ptr_eq(&subscription)
                    {
                        operations[operation].1.push(index);
                    } else {
                        // An external subscriber can complete an operation
                        // during reservation. Only group the same shared future.
                        by_id.insert(req.target.id().as_str(), operations.len());
                        operations.push((subscription, vec![index]));
                    }
                }
            }
        }
        async fn complete(
            (operation, indices): (Shared<ReuploadFuture>, Vec<usize>),
        ) -> (Result<MediaRetryResult, MediaReuploadError>, Vec<usize>) {
            (operation.await, indices)
        }
        // Collect before awaiting: retaining the map in the stream prevents
        // MSRV callers from proving this future is Send.
        let operations: Vec<_> = operations.into_iter().map(complete).collect();
        let mut pending =
            futures::stream::iter(operations).buffer_unordered(MEDIA_REUPLOAD_CONCURRENCY);
        while let Some((result, indices)) = pending.next().await {
            for index in indices {
                results[index] = Some(result.clone());
            }
        }
        results
            .into_iter()
            .map(|result| result.expect("each reserved input has an operation or conflict"))
            .collect()
    }
}

impl Client {
    pub fn media_reupload(&self) -> MediaReupload<'_> {
        MediaReupload::new(self)
    }
}

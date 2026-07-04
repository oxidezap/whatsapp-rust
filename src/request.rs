use crate::client::Client;
use crate::client::ClientError;
use crate::socket::error::{EncryptSendError, SocketError};
use futures::FutureExt;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use thiserror::Error;
use wacore::runtime::timeout as rt_timeout;
use wacore_binary::Node;

pub use wacore::request::{InfoQuery, InfoQueryType, RequestUtils};

/// Type-erased send future handed to [`Client::send_and_wait_iq`]. Boxing it
/// keeps that function non-generic so it isn't re-monomorphized per `IqSpec`.
/// `Send` on native (IQ awaits happen inside spawned handler tasks); dropped
/// on wasm where the runtime is single-threaded.
#[cfg(not(target_arch = "wasm32"))]
type IqSendFuture<'a> =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), ClientError>> + Send + 'a>>;
#[cfg(target_arch = "wasm32")]
type IqSendFuture<'a> =
    std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), ClientError>> + 'a>>;

/// Removes a pending `response_waiters` entry when dropped.
///
/// `send_and_wait_iq` can be cancelled mid-await — e.g. the losing side of a
/// `futures::try_join!` is dropped the instant its sibling errors. Without this
/// guard the registered waiter would linger in the map: the explicit cleanups
/// only fired on the send-fail / timeout / shutdown paths, never on
/// cancellation-via-drop, and a lingering waiter suppresses keepalives for the
/// life of the connection. Dropping the guard removes the entry on every exit
/// path; on success `resolve_waiters` already removed it, so it's a no-op.
struct ResponseWaiterGuard {
    waiters: Arc<std::sync::Mutex<crate::client::ResponseWaiterMap>>,
    req_id: String,
}

impl Drop for ResponseWaiterGuard {
    fn drop(&mut self) {
        self.waiters
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(&self.req_id);
    }
}

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum IqError {
    #[error("IQ request timed out")]
    Timeout,
    #[error("client is not connected")]
    NotConnected,
    #[error("socket error")]
    Socket(#[from] SocketError),
    #[error("encrypted send pipeline failed")]
    EncryptSend(#[from] EncryptSendError),
    // Boxed to break the `ClientError::Iq(IqError)` <-> `IqError::ClientState`
    // type cycle (both would otherwise be infinitely sized).
    #[error("client state prevented send")]
    ClientState(#[source] Box<ClientError>),
    #[error("received disconnect node during IQ wait: {0:?}")]
    Disconnected(Box<Node>),
    #[error("received a server error response: code={code}, text='{text}'")]
    ServerError {
        code: u16,
        text: String,
        /// XMPP error class from the `type` attr; `None` if absent.
        error_type: Option<String>,
        /// Server-directed retry delay in seconds from the `backoff` attr; `None` if absent.
        backoff: Option<u32>,
    },
    #[error("received unexpected IQ response type: {got:?}")]
    UnexpectedResponseType { got: Option<String> },
    #[error("internal channel closed unexpectedly")]
    InternalChannelClosed,
    #[error("failed to encode IQ request")]
    EncodeError(#[source] anyhow::Error),
    #[error("failed to parse IQ response")]
    ParseError(#[from] anyhow::Error),
}

impl From<wacore::request::IqError> for IqError {
    fn from(err: wacore::request::IqError) -> Self {
        match err {
            wacore::request::IqError::Timeout => Self::Timeout,
            wacore::request::IqError::NotConnected => Self::NotConnected,
            wacore::request::IqError::Disconnected(node) => Self::Disconnected(node),
            wacore::request::IqError::ServerError {
                code,
                text,
                error_type,
                backoff,
            } => Self::ServerError {
                code,
                text,
                error_type,
                backoff,
            },
            wacore::request::IqError::UnexpectedResponseType { got } => {
                Self::UnexpectedResponseType { got }
            }
            wacore::request::IqError::InternalChannelClosed => Self::InternalChannelClosed,
            // wacore::IqError is #[non_exhaustive]; a new upstream variant should
            // get its own arm above. Until then treat it as an unexpected internal error.
            _ => Self::InternalChannelClosed,
        }
    }
}

impl Client {
    pub(crate) fn generate_request_id(&self) -> String {
        self.get_request_utils().generate_request_id()
    }

    /// Generates a unique message ID that conforms to the WhatsApp protocol format.
    ///
    /// This is an advanced function that allows library users to generate message IDs
    /// that are compatible with the WhatsApp protocol. The generated ID includes
    /// timestamp, user JID, and random components to ensure uniqueness.
    ///
    /// # Advanced Use Case
    ///
    /// This function is intended for advanced users who need to build custom protocol
    /// interactions or manage message IDs manually. Most users should use higher-level
    /// methods like `send_message` which handle ID generation automatically.
    ///
    /// # Returns
    ///
    /// A string containing the generated message ID in the format expected by WhatsApp.
    pub fn generate_message_id(&self) -> String {
        let device_snapshot = self.persistence_manager.get_device_snapshot();
        self.get_request_utils()
            .generate_message_id(device_snapshot.pn.as_ref())
    }

    fn get_request_utils(&self) -> RequestUtils {
        RequestUtils::with_counter(self.unique_id.clone(), self.id_counter.clone())
    }

    /// Sends a custom IQ (Info/Query) stanza to the WhatsApp server.
    ///
    /// This is an advanced function that allows library users to send custom IQ stanzas
    /// for protocol interactions that are not covered by higher-level methods. Common
    /// use cases include live location updates, custom presence management, or other
    /// advanced WhatsApp features.
    ///
    /// # Advanced Use Case
    ///
    /// This function bypasses some of the higher-level abstractions and safety checks
    /// provided by other client methods. Users should be familiar with the WhatsApp
    /// protocol and IQ stanza format before using this function.
    ///
    /// # Arguments
    ///
    /// * `query` - The IQ query to send, containing the stanza type, namespace, content, and optional timeout
    ///
    /// # Returns
    ///
    /// * `Ok(Arc<OwnedNodeRef>)` - The response node from the server (zero-copy, borrowed from decode buffer)
    /// * `Err(IqError)` - Various error conditions including timeout, connection issues, or server errors
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use wacore::request::{InfoQuery, InfoQueryType};
    /// use wacore_binary::builder::NodeBuilder;
    /// use wacore_binary::NodeContent;
    /// use wacore_binary::{Jid, Server};
    ///
    /// // This is a simplified example - real usage requires proper setup
    /// # async fn example(client: &whatsapp_rust::Client) -> Result<(), Box<dyn std::error::Error>> {
    /// let query_node = NodeBuilder::new("presence")
    ///     .attr("type", "available")
    ///     .build();
    ///
    /// let server_jid = Jid::new("", Server::Pn);
    ///
    /// let query = InfoQuery {
    ///     query_type: InfoQueryType::Set,
    ///     namespace: "presence",
    ///     to: server_jid,
    ///     target: None,
    ///     content: Some(NodeContent::Nodes(vec![query_node])),
    ///     id: None,
    ///     timeout: None,
    /// };
    ///
    /// let response = client.send_iq(query).await?;
    /// // Access the node via response.get()
    /// # Ok(())
    /// # }
    /// ```
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "wa.iq",
            level = "debug",
            skip_all,
            fields(
                ns = %query.namespace,
                kind = ?query.query_type,
                lid = tracing::field::Empty,
                pn = tracing::field::Empty
            ),
            err(Debug)
        )
    )]
    pub async fn send_iq(
        &self,
        query: InfoQuery<'_>,
    ) -> Result<Arc<wacore_binary::OwnedNodeRef>, IqError> {
        #[cfg(feature = "tracing")]
        self.record_identity_on_span(&tracing::Span::current());

        let default_timeout = Duration::from_secs(75);
        let iq_timeout = query.timeout.unwrap_or(default_timeout);
        let req_id = query
            .id
            .clone()
            .unwrap_or_else(|| self.generate_request_id());

        let request_utils = self.get_request_utils();
        let node = request_utils.build_iq_node(query, Some(req_id.clone()));

        self.send_and_wait_iq(
            req_id,
            iq_timeout,
            Box::pin(async { self.send_node(node).await }),
        )
        .await
    }

    /// Executes an IQ specification and returns the typed response.
    ///
    /// This is a convenience method that combines building the IQ request,
    /// sending it, and parsing the response into a single operation.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use wacore::iq::groups::GroupQueryIq;
    ///
    /// let group_info = client.execute(GroupQueryIq::new(&group_jid)).await?;
    /// println!("Group subject: {}", group_info.subject);
    /// ```
    pub async fn execute<S>(&self, spec: S) -> Result<S::Response, IqError>
    where
        S: wacore::iq::spec::IqSpec,
    {
        let req_id = self.generate_request_id();

        // Direct-encode fast path: skip Node tree for hot IQ specs (e.g. PreKeyUploadSpec)
        {
            let mut buf = Vec::new();
            match spec.encode_iq_direct(&req_id, &mut buf) {
                Ok(true) => {
                    let response = self
                        .send_and_wait_iq(
                            req_id,
                            Duration::from_secs(75),
                            Box::pin(async { self.send_raw_bytes(buf).await }),
                        )
                        .await?;
                    return spec
                        .parse_response(response.get())
                        .map_err(IqError::ParseError);
                }
                Err(e) => return Err(IqError::EncodeError(e)),
                Ok(false) => {}
            }
        }

        let mut iq = spec.build_iq();
        if iq.id.is_none() {
            iq.id = Some(req_id);
        }
        let response = self.send_iq(iq).await?;
        spec.parse_response(response.get())
            .map_err(IqError::ParseError)
    }

    /// Centralizes waiter registration and shutdown/timeout handling.
    ///
    /// `send_fn` is type-erased (boxed) rather than a generic `F`: it's only
    /// awaited once, inline, and `execute<S>` would otherwise stamp out a
    /// fresh copy of this whole waiter/timeout body per IqSpec (the send
    /// closure's type is distinct per `S`). One box allocation per IQ — all
    /// control-plane, never the message hot path — collapses ~15 monomorphized
    /// copies into one.
    async fn send_and_wait_iq(
        &self,
        req_id: String,
        timeout: Duration,
        send_fn: IqSendFuture<'_>,
    ) -> Result<Arc<wacore_binary::OwnedNodeRef>, IqError> {
        let _t = wacore::telemetry::timer(wacore::telemetry::IQ_DURATION);
        if !self.is_running.load(Ordering::Relaxed) {
            wacore::telemetry::iq("error");
            return Err(IqError::NotConnected);
        }

        let (tx, rx) = futures::channel::oneshot::channel();
        {
            let mut waiters = self.response_waiters_guard();
            // req_ids come from the monotonic generate_request_id(), so a given
            // id is never in flight twice — the invariant that makes the guard's
            // remove-by-id unambiguous (an overwrite could otherwise let an older
            // guard evict a newer waiter). Assert it so a future caller passing a
            // duplicate id is caught in tests instead of silently.
            debug_assert!(
                !waiters.contains_key(&req_id),
                "duplicate in-flight IQ request id: {req_id}"
            );
            waiters.insert(req_id.clone(), tx);
        }
        // RAII cleanup covers every exit below — including this future being
        // dropped mid-await (cancellation), which the explicit paths can't
        // catch. So the send-fail / timeout / shutdown arms no longer remove
        // the waiter by hand; the guard does it on drop.
        let _waiter_guard = ResponseWaiterGuard {
            waiters: self.response_waiters.clone(),
            req_id,
        };

        // Per-connection: pending IQ requests are bound to the current socket;
        // a reconnect aborts them (sender retries on the new connection).
        let shutdown = wacore::runtime::wait_for_shutdown(&self.connection_shutdown_signal());

        if !self.is_running.load(Ordering::Acquire) {
            wacore::telemetry::iq("error");
            return Err(IqError::NotConnected);
        }

        if let Err(e) = send_fn.await {
            wacore::telemetry::iq("error");
            return match e {
                ClientError::Socket(s_err) => Err(IqError::Socket(s_err)),
                ClientError::EncryptSend(es_err) => Err(IqError::EncryptSend(es_err)),
                ClientError::NotConnected => Err(IqError::NotConnected),
                // The send future only ever yields the transport/state errors
                // above; any other (incl. future #[non_exhaustive]) variant is
                // surfaced as a client-state failure.
                other => Err(IqError::ClientState(Box::new(other))),
            };
        }

        let request_utils = self.get_request_utils();
        let result = futures::select! {
            result = rt_timeout(&*self.runtime, timeout, rx).fuse() => {
                match result {
                    Ok(Ok(response_node)) => match request_utils.parse_iq_response(response_node.get()) {
                        Ok(()) => Ok(response_node),
                        Err(e) => Err(e.into()),
                    },
                    Ok(Err(_)) => Err(IqError::InternalChannelClosed),
                    Err(_) => Err(IqError::Timeout),
                }
            }
            _ = shutdown.fuse() => Err(IqError::NotConnected),
        };
        wacore::telemetry::iq(match &result {
            Ok(_) => "ok",
            Err(IqError::Timeout) => "timeout",
            Err(_) => "error",
        });
        result
    }
}

#[cfg(test)]
mod tests {
    use super::{IqError, ResponseWaiterGuard};
    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    #[test]
    fn converts_unexpected_response_type() {
        let err = IqError::from(wacore::request::IqError::UnexpectedResponseType {
            got: Some("get".to_string()),
        });

        match err {
            IqError::UnexpectedResponseType { got } => assert_eq!(got.as_deref(), Some("get")),
            other => panic!("expected UnexpectedResponseType, got {other:?}"),
        }
    }

    // Cancellation cleanup: dropping a `send_and_wait_iq` future mid-await (e.g.
    // the loser of a `try_join!`) must remove its still-pending waiter, or a
    // leaked entry suppresses keepalives for the life of the connection.
    #[test]
    fn waiter_guard_removes_pending_entry_on_drop() {
        let waiters: Arc<Mutex<crate::client::ResponseWaiterMap>> =
            Arc::new(Mutex::new(HashMap::new()));
        let (tx, _rx) = futures::channel::oneshot::channel();
        waiters.lock().unwrap().insert("req-1".to_string(), tx);
        assert!(waiters.lock().unwrap().contains_key("req-1"));

        {
            let _guard = ResponseWaiterGuard {
                waiters: waiters.clone(),
                req_id: "req-1".to_string(),
            };
        }
        assert!(
            !waiters.lock().unwrap().contains_key("req-1"),
            "dropping the guard must remove the pending waiter"
        );
    }

    // On the success path the resolver already removed the entry before the
    // guard drops, so the guard's removal must be a harmless no-op.
    #[test]
    fn waiter_guard_drop_is_noop_when_already_resolved() {
        let waiters: Arc<Mutex<crate::client::ResponseWaiterMap>> =
            Arc::new(Mutex::new(HashMap::new()));
        // Map empty = resolver already delivered + removed this request's waiter.
        {
            let _guard = ResponseWaiterGuard {
                waiters: waiters.clone(),
                req_id: "req-1".to_string(),
            };
        }
        assert!(waiters.lock().unwrap().is_empty());
    }
}

//! The dispatcher grants one reply tied to the request identity and socket.
//!
//! WAComms fences replies by socket; WASmaxOutMdSetRegResponseError uses a
//! code/text error without echoing the query. Those constrain this envelope,
//! while extension payloads and namespace support remain plugin-owned policy.

use std::sync::atomic::AtomicU8;

use event_listener::Event;
use wacore::socket::noise_socket::NoiseSocket;
use wacore_binary::builder::NodeBuilder;
use wacore_binary::node::{Node, ValueRef};

use super::*;

const PENDING: u8 = 0;
const CLAIMED: u8 = 1;
const REJECTED: u8 = 2;
const USED: u8 = 3;

/// An inline handler for extension-defined, authenticated inbound IQ requests.
///
/// The callback must match its namespace and payload narrowly. Retaining a
/// grant does not claim the request: only returning `Handled` authorizes it.
/// Async work should use an explicitly bounded queue and plugin task scope.
pub trait PluginIqInterceptor: MaybeSendSync + 'static {
    fn intercept(&self, node: &OwnedNodeRef, reply: PluginIqReply) -> Interception;
}

impl<F> PluginIqInterceptor for F
where
    F: Fn(&OwnedNodeRef, PluginIqReply) -> Interception + MaybeSendSync + 'static,
{
    fn intercept(&self, node: &OwnedNodeRef, reply: PluginIqReply) -> Interception {
        self(node, reply)
    }
}

struct Claim {
    state: AtomicU8,
    decided: Event,
}

/// One correlated reply to one claimed inbound IQ, on its original connection.
///
/// Clones share a single send attempt. Once send admission reserves the grant,
/// cancellation or send failure consumes it because the peer may already have
/// received bytes. An unpolled future has not reserved it. Success means local
/// transport completion, not peer processing or delivery confirmation. Dropping
/// an unused grant sends nothing. A later retransmitted request is a new grant;
/// this does not promise server-side deduplication.
#[derive(Clone)]
pub struct PluginIqReply {
    client: Weak<Client>,
    resources: Arc<PluginResources>,
    claim: Arc<Claim>,
    generation: u64,
    shutdown: ShutdownSignal,
    socket: Weak<NoiseSocket>,
    id: String,
    to: Jid,
}

impl PluginIqReply {
    /// Request identifier fixed by the inbound stanza.
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Destination fixed by the validated inbound `from` attribute.
    pub fn destination(&self) -> &Jid {
        &self.to
    }

    /// Reply with extension-defined result children. The host fixes the IQ
    /// envelope; callers are responsible for their namespace's payload schema.
    pub async fn result(&self, children: Vec<Node>) -> Result<(), PluginIqReplyError> {
        self.send("result", children).await
    }

    /// Reply with code 400..=599 and nonempty text, using a code/text pair
    /// defined by the plugin's extension namespace. Invalid arguments leave
    /// the grant unused. Pairing requests remain owned by the core.
    pub async fn error(&self, code: u16, text: &str) -> Result<(), PluginIqReplyError> {
        if !(400..=599).contains(&code) {
            return Err(PluginIqReplyError::InvalidErrorCode);
        }
        if text.is_empty() {
            return Err(PluginIqReplyError::InvalidErrorText);
        }
        self.send(
            "error",
            vec![
                NodeBuilder::new("error")
                    .attr("code", code.to_string())
                    .attr("text", text.to_owned())
                    .build(),
            ],
        )
        .await
    }

    async fn send(
        &self,
        kind: &'static str,
        children: Vec<Node>,
    ) -> Result<(), PluginIqReplyError> {
        let plugin_shutdown = self.resources.shutdown.subscribe();
        let cancelled = async {
            futures::select_biased! {
                _ = wait_for_shutdown(&self.shutdown).fuse() => {},
                _ = wait_for_shutdown(&plugin_shutdown).fuse() => {},
            }
        }
        .fuse();
        let send = async {
            loop {
                let listener = self.claim.decided.listen();
                match self.claim.state.load(Ordering::Acquire) {
                    PENDING => listener.await,
                    REJECTED => return Err(PluginIqReplyError::NotClaimed),
                    USED => return Err(PluginIqReplyError::AlreadyUsed),
                    CLAIMED => break,
                    _ => unreachable!("private claim state"),
                }
            }
            self.resources.ensure_active()?;
            let client = self
                .client
                .upgrade()
                .ok_or(PluginResourceError::ClientUnavailable)?;
            if self.shutdown.is_fired()
                || client.connection_generation.load(Ordering::Acquire) != self.generation
                || client.authenticated_generation.load(Ordering::SeqCst) != self.generation
                || !client.can_reach_server()
            {
                return Err(PluginIqReplyError::ConnectionRetired);
            }
            self.claim
                .state
                .compare_exchange(CLAIMED, USED, Ordering::AcqRel, Ordering::Acquire)
                .map_err(|_| PluginIqReplyError::AlreadyUsed)?;
            // The reservation precedes the first send poll. Dropping this future
            // cannot grant another attempt whose wire outcome would be uncertain.
            let node = NodeBuilder::new("iq")
                .attr("type", kind)
                .attr("id", self.id.clone())
                .attr("to", self.to.clone())
                .children(children)
                .build();
            let socket = self
                .socket
                .upgrade()
                .ok_or(PluginIqReplyError::ConnectionRetired)?;
            client
                .send_node_observed_in_scope(node, None, self.generation, socket)
                .await?;
            Ok(())
        }
        .fuse();
        futures::pin_mut!(cancelled, send);
        futures::select_biased! {
            _ = cancelled => Err(PluginIqReplyError::ConnectionRetired),
            result = send => result,
        }
    }
}

pub(super) struct PluginIqInterceptorAdapter {
    pub(super) client: Weak<Client>,
    pub(super) resources: Arc<PluginResources>,
    pub(super) inner: Arc<dyn PluginIqInterceptor>,
}

impl StanzaInterceptor for PluginIqInterceptorAdapter {
    fn intercept(&self, _node: &OwnedNodeRef) -> Interception {
        // Only the dispatcher can provide the connection scope.
        Interception::Pass
    }

    fn intercept_scoped(&self, node: &OwnedNodeRef, scope: &InterceptionScope) -> Interception {
        let Some((id, to)) = request_identity(node) else {
            return Interception::Pass;
        };
        let Some(client) = self.client.upgrade() else {
            return Interception::Pass;
        };
        if self.resources.ensure_active().is_err()
            || scope.shutdown.is_fired()
            || client.connection_generation.load(Ordering::Acquire) != scope.generation
            || client.authenticated_generation.load(Ordering::SeqCst) != scope.generation
            || !client.can_reach_server()
        {
            return Interception::Pass;
        }
        let Ok(socket) = client.get_noise_socket() else {
            return Interception::Pass;
        };
        let claim = Arc::new(Claim {
            state: AtomicU8::new(PENDING),
            decided: Event::new(),
        });
        let decision = scopeguard::guard(Arc::clone(&claim), |claim| {
            // Reject any grant retained by a callback that unwinds.
            if claim
                .state
                .compare_exchange(PENDING, REJECTED, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
            {
                claim.decided.notify(usize::MAX);
            }
        });
        let reply = PluginIqReply {
            client: self.client.clone(),
            resources: Arc::clone(&self.resources),
            claim,
            generation: scope.generation,
            shutdown: scope.shutdown.clone(),
            socket: Arc::downgrade(&socket),
            id,
            to,
        };
        let interception = self.inner.intercept(node, reply);
        decision.state.store(
            if interception.is_handled() {
                CLAIMED
            } else {
                REJECTED
            },
            Ordering::Release,
        );
        decision.decided.notify(usize::MAX);
        interception
    }
}

fn request_identity(node: &OwnedNodeRef) -> Option<(String, Jid)> {
    let node = node.get();
    if node.tag.as_ref() != "iq" {
        return None;
    }
    let mut names = HashSet::new();
    if node
        .attrs
        .as_slice()
        .iter()
        .any(|(name, _)| !names.insert(name.as_ref()))
    {
        return None;
    }
    let ValueRef::String(kind) = node.get_attr("type")? else {
        return None;
    };
    if !matches!(kind.as_ref(), "get" | "set") {
        return None;
    }
    let ValueRef::String(id) = node.get_attr("id")? else {
        return None;
    };
    let ValueRef::String(namespace) = node.get_attr("xmlns")? else {
        return None;
    };
    let children = node.children()?;
    // Deliberately bounded extension contract. Authentication/pairing and ping
    // traffic do not belong to this capability, even after login.
    if id.is_empty()
        || namespace.is_empty()
        || children.len() != 1
        || matches!(namespace.as_ref(), "urn:xmpp:ping" | "md")
        || matches!(
            children[0].tag.as_ref(),
            "ping" | "pair-device" | "pair-success" | "link_code_companion_reg"
        )
    {
        return None;
    }
    let from = match node.get_attr("from")? {
        ValueRef::Jid(jid) => jid.to_owned(),
        ValueRef::String(value) => value.parse::<Jid>().ok()?,
    };
    Some((id.to_string(), from))
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;

    use wacore::handshake::NoiseCipher;

    use super::*;
    use crate::runtime_impl::TokioRuntime;
    use crate::store::persistence_manager::PersistenceManager;
    use crate::test_utils::MockHttpClient;
    use crate::transport::mock::{
        CapturingMockTransport, MockTransportFactory, StallingMockTransport,
    };

    struct ReplyPlugin {
        reply_capability: bool,
        decision: Interception,
        panic: bool,
    }

    struct ReplyApi {
        grants: Arc<Mutex<Vec<PluginIqReply>>>,
        second_calls: Arc<AtomicUsize>,
        _registrations: Vec<PluginInterceptorRegistration>,
    }

    impl ClientPlugin for ReplyPlugin {
        type Api = ReplyApi;

        fn manifest(&self) -> PluginManifest {
            let manifest = PluginManifest::new("test.iq-reply", "0.1.0")
                .with_capability(PluginCapability::StanzaInterception);
            if self.reply_capability {
                manifest.with_capability(PluginCapability::IqReply)
            } else {
                manifest
            }
        }

        fn install(&self, context: PluginContext) -> BoxFuture<'_, anyhow::Result<Arc<Self::Api>>> {
            Box::pin(async move {
                let grants = Arc::new(Mutex::new(Vec::new()));
                let second_calls = Arc::new(AtomicUsize::new(0));
                let recorder = Arc::clone(&grants);
                let decision = self.decision;
                let panic = self.panic;
                let interception = context.stanza_interception().expect("declared capability");
                let result = interception.register_iq(Arc::new(
                    move |_node: &OwnedNodeRef, reply: PluginIqReply| {
                        recorder.lock().unwrap().push(reply);
                        assert!(!panic, "injected IQ interceptor panic");
                        decision
                    },
                ));
                let mut registrations = Vec::new();
                if self.reply_capability {
                    registrations.push(result?);
                    let calls = Arc::clone(&second_calls);
                    registrations.push(interception.register_iq(Arc::new(
                        move |_node: &OwnedNodeRef, _reply: PluginIqReply| {
                            calls.fetch_add(1, Ordering::SeqCst);
                            Interception::Pass
                        },
                    ))?);
                } else {
                    assert!(matches!(
                        result,
                        Err(PluginIqReplyError::CapabilityRequired)
                    ));
                }
                Ok(Arc::new(ReplyApi {
                    grants,
                    second_calls,
                    _registrations: registrations,
                }))
            })
        }
    }

    async fn client_with_plugin(
        decision: Interception,
        panic: bool,
        reply_capability: bool,
    ) -> Arc<Client> {
        let pm = Arc::new(
            PersistenceManager::new(crate::test_utils::create_test_backend().await)
                .await
                .unwrap(),
        );
        Client::builder()
            .with_runtime(TokioRuntime)
            .with_persistence_manager(pm)
            .with_transport_factory(MockTransportFactory::new())
            .with_http_client(MockHttpClient)
            .with_plugin(ReplyPlugin {
                reply_capability,
                decision,
                panic,
            })
            .build()
            .await
            .unwrap()
            .into_client()
    }

    fn connect(client: &Arc<Client>, transport: Arc<dyn crate::transport::Transport>) {
        let socket = NoiseSocket::new(
            client.runtime.clone(),
            transport,
            NoiseCipher::new(&[0; 32]).unwrap(),
            NoiseCipher::new(&[0; 32]).unwrap(),
        );
        *client.noise_socket.lock().unwrap() = Some(Arc::new(socket));
        client.set_connected_for_test(true);
        client.set_logged_in_for_test(true, Ordering::Release);
        client.set_driver_running_for_test(true, Ordering::Release);
        client.authenticated_generation.store(
            client.connection_generation.load(Ordering::Acquire),
            Ordering::SeqCst,
        );
    }

    fn request() -> Node {
        NodeBuilder::new("iq")
            .attr("type", "get")
            .attr("id", "PLUGIN-IQ-1")
            .attr("from", "s.whatsapp.net")
            .attr("xmlns", "test:extension")
            .children([NodeBuilder::new("query").build()])
            .build()
    }

    fn owned(node: Node) -> Arc<OwnedNodeRef> {
        crate::test_utils::node_to_owned_ref(&node)
    }

    fn grant(client: &Arc<Client>) -> PluginIqReply {
        client
            .plugin::<ReplyPlugin>()
            .unwrap()
            .grants
            .lock()
            .unwrap()
            .last()
            .unwrap()
            .clone()
    }

    #[tokio::test]
    async fn context_only_reply_is_correlated_once_and_first_claimant_wins() {
        let client = client_with_plugin(Interception::Handled, false, true).await;
        let transport = Arc::new(CapturingMockTransport::new());
        connect(&client, transport.clone());
        let (sender, receiver) = futures::channel::oneshot::channel();
        client.response_waiters_guard().insert(
            "PLUGIN-IQ-1".into(),
            crate::client::ResponseWaiter::Iq(sender),
        );

        client.process_node(owned(request())).await;
        let reply = grant(&client);
        assert_eq!(reply.id(), "PLUGIN-IQ-1");
        assert_eq!(reply.destination().to_string(), "s.whatsapp.net");
        assert_eq!(
            client
                .plugin::<ReplyPlugin>()
                .unwrap()
                .second_calls
                .load(Ordering::Acquire),
            0
        );
        assert_eq!(
            client.response_waiters_guard().len(),
            1,
            "inbound request must not steal an outgoing waiter with the same ID"
        );
        reply
            .result(vec![
                NodeBuilder::new("answer")
                    .attr("value", "synthetic")
                    .build(),
            ])
            .await
            .unwrap();
        let sent = crate::test_utils::decode_sent_iq(&transport, 0).await;
        let node = sent.get();
        assert_eq!(node.tag.as_ref(), "iq");
        assert_eq!(node.get_attr("id").unwrap().as_str(), "PLUGIN-IQ-1");
        assert_eq!(node.get_attr("to").unwrap().as_str(), "s.whatsapp.net");
        assert_eq!(node.get_attr("type").unwrap().as_str(), "result");
        assert!(node.get_attr("from").is_none());
        assert_eq!(node.children().unwrap()[0].tag.as_ref(), "answer");
        assert!(matches!(
            reply.result(vec![]).await,
            Err(PluginIqReplyError::AlreadyUsed)
        ));
        assert_eq!(
            transport.sent().len(),
            1,
            "no generic IQ ACK or duplicate reply"
        );
        let response = owned(
            NodeBuilder::new("iq")
                .attr("type", "result")
                .attr("id", "PLUGIN-IQ-1")
                .build(),
        );
        client.process_node(response.clone()).await;
        assert!(Arc::ptr_eq(&receiver.await.unwrap(), &response));
        assert_eq!(
            client
                .plugin::<ReplyPlugin>()
                .unwrap()
                .grants
                .lock()
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn reply_capability_is_explicit_and_requires_authenticated_requests() {
        let no_capability = client_with_plugin(Interception::Handled, false, false).await;
        assert!(!no_capability.has_stanza_interceptors());
        let client = client_with_plugin(Interception::Handled, false, true).await;
        client.process_node(owned(request())).await;
        assert!(
            client
                .plugin::<ReplyPlugin>()
                .unwrap()
                .grants
                .lock()
                .unwrap()
                .is_empty()
        );
        connect(&client, Arc::new(CapturingMockTransport::new()));
        client.process_node(owned(request())).await;
        assert_eq!(
            client
                .plugin::<ReplyPlugin>()
                .unwrap()
                .grants
                .lock()
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn set_error_reply_validates_code_and_text_without_consuming_grant() {
        let client = client_with_plugin(Interception::Handled, false, true).await;
        let transport = Arc::new(CapturingMockTransport::new());
        connect(&client, transport.clone());
        let mut node = request();
        node.attrs.insert("type", "set");
        node.attrs.insert("id", "PLUGIN-SET-ERROR");
        client.process_node(owned(node)).await;
        let reply = grant(&client);
        for code in [200, 399, 600] {
            assert!(matches!(
                reply.error(code, "not-authorized").await,
                Err(PluginIqReplyError::InvalidErrorCode)
            ));
        }
        assert!(matches!(
            reply.error(401, "").await,
            Err(PluginIqReplyError::InvalidErrorText)
        ));
        assert!(transport.sent().is_empty());
        // The Web pairing builder supplies this envelope. The synthetic
        // namespace models neither pairing effects nor server acceptance.
        reply.error(401, "not-authorized").await.unwrap();
        let sent = crate::test_utils::decode_sent_iq(&transport, 0).await;
        let node = sent.get();
        assert_eq!(node.get_attr("type").unwrap().as_str(), "error");
        assert_eq!(node.get_attr("id").unwrap().as_str(), "PLUGIN-SET-ERROR");
        assert_eq!(node.get_attr("to").unwrap().as_str(), "s.whatsapp.net");
        let children = node.children().unwrap();
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].tag.as_ref(), "error");
        assert_eq!(children[0].get_attr("code").unwrap().as_str(), "401");
        assert_eq!(
            children[0].get_attr("text").unwrap().as_str(),
            "not-authorized"
        );
        assert!(node.get_attr("xmlns").is_none());
        assert!(matches!(
            reply.error(401, "not-authorized").await,
            Err(PluginIqReplyError::AlreadyUsed)
        ));
        assert_eq!(transport.sent().len(), 1);
    }

    #[test]
    fn malformed_response_ping_pairing_and_ambiguous_shapes_have_no_grant() {
        let mut cases = Vec::new();
        for field in ["id", "from", "xmlns", "type"] {
            let mut node = request();
            node.attrs.0.retain(|(key, _)| key != field);
            cases.push(node);
        }
        for (field, value) in [
            ("id", ""),
            ("from", "not-a-jid"),
            ("xmlns", ""),
            ("xmlns", "urn:xmpp:ping"),
            ("xmlns", "md"),
            ("type", "result"),
            ("type", "error"),
        ] {
            let mut node = request();
            node.attrs.insert(field, value);
            cases.push(node);
        }
        for tag in [
            "ping",
            "pair-device",
            "pair-success",
            "link_code_companion_reg",
        ] {
            let mut node = request();
            node.content = Some(wacore_binary::NodeContent::Nodes(vec![
                NodeBuilder::new(tag).build(),
            ]));
            cases.push(node);
        }
        let mut duplicate = request();
        duplicate.attrs.push("id", "SPOOFED-ID");
        cases.push(duplicate);
        let mut no_children = request();
        no_children.content = None;
        cases.push(no_children);
        let mut many_children = request();
        many_children.content = Some(wacore_binary::NodeContent::Nodes(vec![
            NodeBuilder::new("query").build(),
            NodeBuilder::new("extra").build(),
        ]));
        cases.push(many_children);
        for case in cases {
            assert!(request_identity(&owned(case.clone())).is_none(), "{case:?}");
        }
    }

    #[tokio::test]
    async fn passed_and_panicking_claims_reject_retained_reply_tokens() {
        for (decision, panic) in [(Interception::Pass, false), (Interception::Handled, true)] {
            let client = client_with_plugin(decision, panic, true).await;
            let transport = Arc::new(CapturingMockTransport::new());
            connect(&client, transport.clone());
            client.process_node(owned(request())).await;
            assert!(matches!(
                grant(&client).result(vec![]).await,
                Err(PluginIqReplyError::NotClaimed)
            ));
            assert!(transport.sent().is_empty());
        }
    }

    #[tokio::test]
    async fn generation_retirement_and_shutdown_reject_reply_on_replacement_socket() {
        for shutdown in [false, true] {
            let client = client_with_plugin(Interception::Handled, false, true).await;
            let old_transport = Arc::new(CapturingMockTransport::new());
            connect(&client, old_transport.clone());
            client.process_node(owned(request())).await;
            let reply = grant(&client);
            if shutdown {
                let _ = client.shutdown().await;
            } else {
                client.advance_generation_for_test(1, Ordering::SeqCst);
            }
            let replacement = Arc::new(CapturingMockTransport::new());
            connect(&client, replacement.clone());
            assert!(reply.result(vec![]).await.is_err());
            assert!(old_transport.sent().is_empty());
            assert!(replacement.sent().is_empty());
        }
    }

    #[tokio::test]
    async fn cancelling_an_admitted_send_consumes_the_grant() {
        let client = client_with_plugin(Interception::Handled, false, true).await;
        let transport = Arc::new(StallingMockTransport::new());
        connect(&client, transport.clone());
        client.process_node(owned(request())).await;
        let reply = grant(&client);
        let sending = reply.clone();
        let task = tokio::spawn(async move { sending.result(vec![]).await });
        tokio::time::timeout(Duration::from_secs(2), async {
            while transport.sends_started() == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert!(matches!(
            reply.result(vec![]).await,
            Err(PluginIqReplyError::AlreadyUsed)
        ));
        assert_eq!(transport.sends_started(), 1);
    }

    #[tokio::test]
    async fn retiring_connection_cancels_admitted_send_without_using_replacement() {
        let client = client_with_plugin(Interception::Handled, false, true).await;
        let transport = Arc::new(StallingMockTransport::new());
        connect(&client, transport.clone());
        client.process_node(owned(request())).await;
        let reply = grant(&client);
        let sending = reply.clone();
        let task = tokio::spawn(async move { sending.result(vec![]).await });
        tokio::time::timeout(Duration::from_secs(2), async {
            while transport.sends_started() == 0 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();

        client.notify_connection_shutdown();
        client.advance_generation_for_test(1, Ordering::SeqCst);
        let replacement = Arc::new(CapturingMockTransport::new());
        connect(&client, replacement.clone());
        let result = tokio::time::timeout(Duration::from_secs(2), task)
            .await
            .expect("reply must observe retirement while send is stalled")
            .unwrap();
        assert!(matches!(result, Err(PluginIqReplyError::ConnectionRetired)));
        assert!(matches!(
            reply.result(vec![]).await,
            Err(PluginIqReplyError::ConnectionRetired)
        ));
        assert_eq!(transport.sends_started(), 1);
        assert!(replacement.sent().is_empty());
    }
}

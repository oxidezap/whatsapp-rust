//! Real client connection and call signaling over a synthetic in-process server.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use anyhow::{Context, Result, ensure};
use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use tokio::sync::oneshot;
use wacore::handshake::{NoiseCipher, NoiseHandshake};
use wacore::libsignal::protocol::KeyPair;
use wacore::store::InMemoryBackend;
use wacore::store::traits::{DeviceInfo, DeviceListRecord};
use wacore::types::events::{Event, EventHandler};
use wacore_binary::consts::{NOISE_PATTERN_XX, WA_CONN_HEADER};
use wacore_binary::{Jid, Node, OwnedNodeRef, Server, builder::NodeBuilder, marshal};

use crate::Client;
use crate::client::ClientBuilder;
use crate::handshake::NoiseCertPolicy;
use crate::http::{HttpClient, HttpRequest, HttpResponse};
use crate::runtime_impl::TokioRuntime;
use crate::store::commands::DeviceCommand;
use crate::store::persistence_manager::PersistenceManager;
use crate::transport::{Transport, TransportEvent, TransportFactory};
use crate::waproto::whatsapp as wa;

const LIMIT: usize = 4096;
const DEADLINE: Duration = Duration::from_secs(10);

/// An offer observed at transport entry, before its send future completes.
/// Dropping this value refuses the send. Completing it does not inject an ACK.
pub struct PendingOffer {
    node: Node,
    completion: oneshot::Sender<Result<(), String>>,
}

impl PendingOffer {
    /// The actual production offer, including its video advertisement and device destinations.
    pub fn stanza(&self) -> &Node {
        &self.node
    }

    /// Let the production send finish. The real builder can then return its dormant handle.
    pub fn complete(self) -> Result<()> {
        self.completion
            .send(Ok(()))
            .map_err(|_| anyhow::anyhow!("offer send was cancelled"))
    }

    /// Fail the production transport send with a synthetic error.
    pub fn fail(self) -> Result<()> {
        self.completion
            .send(Err("fixture refused offer send".into()))
            .map_err(|_| anyhow::anyhow!("offer send was cancelled"))
    }
}

#[derive(Default)]
struct Events(Mutex<Vec<Arc<Event>>>);

impl EventHandler for Events {
    fn handle_event(&self, event: Arc<Event>) {
        let mut events = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if events.len() <= LIMIT {
            events.push(event);
        }
    }
}

/// Native, opt-in call fixture with fictitious identities and no network access.
///
/// `new` completes production Noise XX, login and the empty offline-delivery phase.
/// The synthetic server uses the existing per-client certificate-signature bypass.
/// Calls must be made with `client().voip().call(peer())`, not a fixture readiness setter.
/// No offer ACK is synthesized, so the resulting handle stays dormant until the test injects one.
/// Call `shutdown` before dropping the fixture when testing teardown completion.
pub struct CallFixture {
    client: Arc<Client>,
    peer: Jid,
    transport: Arc<Wire>,
    offers: async_channel::Receiver<PendingOffer>,
    events: Arc<Events>,
    reader: Mutex<Option<tokio::task::JoinHandle<Result<()>>>>,
}

impl CallFixture {
    /// Connect a real client to a synthetic server. Requires a running Tokio runtime.
    pub async fn new() -> Result<Self> {
        let pm = Arc::new(PersistenceManager::new(Arc::new(InMemoryBackend::new())).await?);
        let own = Jid::new("111111111111111", Server::Lid).with_device(1);
        for command in [
            DeviceCommand::SetId(Some(Jid::new("15550002222", Server::Pn).with_device(1))),
            DeviceCommand::SetLid(Some(own.clone())),
            DeviceCommand::SetPushName("Synthetic call fixture".into()),
            DeviceCommand::SetAccount(Some(wa::ADVSignedDeviceIdentity {
                details: Some(vec![0; 32]),
                account_signature_key: Some(vec![0; 32]),
                account_signature: Some(vec![0; 64]),
                device_signature: Some(vec![0; 64]),
            })),
        ] {
            pm.process_command(command).await;
        }
        let (server_tx, server_rx) = async_channel::bounded(256);
        let (offer_tx, offers) = async_channel::bounded(16);
        let transport = Arc::new(Wire {
            state: Mutex::new(State::Hello),
            server_tx,
            server_rx,
            offers: offer_tx,
            outgoing: Mutex::new(Vec::new()),
        });
        let client = ClientBuilder::new()
            .with_runtime(TokioRuntime)
            .with_persistence_manager(pm)
            .with_transport_factory(Factory(transport.clone()))
            .with_http_client(NoHttp)
            .with_version_override((2, 3000, 0))
            .with_noise_cert_policy(NoiseCertPolicy::DangerSkipCertChainVerify)
            .build()
            .await?
            .into_client();
        client.set_relay_transport_provider(Arc::new(NoRelay));
        let peer = Jid::new("333333333333333", Server::Lid);
        client
            .update_device_list(DeviceListRecord {
                user: Arc::from(peer.user.as_str()),
                devices: vec![DeviceInfo::new(0, None), DeviceInfo::new(2, None)]
                    .into_boxed_slice(),
                timestamp: wacore::time::now_utc().timestamp(),
                phash: None,
                raw_id: None,
            })
            .await?;
        for device in [0, 2] {
            super::seed_peer_session(&client, &peer.clone().with_device(device)).await?;
        }
        let events = Arc::new(Events::default());
        client.subscribe_handler(events.clone()).detach();
        let (connected_tx, connected_rx) = oneshot::channel();
        let reader = tokio::spawn({
            let client = client.clone();
            async move {
                let connection = client.connect().await?;
                let _ = connected_tx.send(());
                connection.read_until_disconnected().await;
                Ok(())
            }
        });
        let fixture = Self {
            client,
            peer,
            transport,
            offers,
            events,
            reader: Mutex::new(Some(reader)),
        };
        tokio::time::timeout(DEADLINE, connected_rx).await??;
        fixture
            .inject(NodeBuilder::new("success").attr("lid", own).build())
            .await?;
        fixture.client.wait_for_connected(DEADLINE).await?;
        Ok(fixture)
    }

    /// The production client. Attach normal event subscriptions or call the normal builder.
    pub fn client(&self) -> &Arc<Client> {
        &self.client
    }

    /// Fictitious peer with seeded primary device 0 and companion device 2.
    pub fn peer(&self) -> &Jid {
        &self.peer
    }

    /// Read the registered production session, including a winner selected before start returns.
    /// Mutating this detached snapshot cannot change the live call.
    pub fn call_snapshot(&self, call_id: &str) -> Option<wacore::voip::CallSession> {
        self.client.call_registry().snapshot(call_id)
    }

    /// Wait until an actual offer reaches the transport's controlled send boundary.
    pub async fn next_offer(&self) -> Result<PendingOffer> {
        Ok(tokio::time::timeout(DEADLINE, self.offers.recv()).await??)
    }

    /// Marshal and inject a stanza through production `process_node` and its registered handlers.
    /// Returns after handling, including synchronous sibling dismissal. While an offer send is
    /// blocked, inject on a separate task if the handler must itself send a stanza.
    /// This bypasses inbound Noise framing, not parser, routing or winner-selection policy.
    pub async fn inject(&self, node: Node) -> Result<()> {
        let packed = marshal(&node)?;
        let unpacked = wacore_binary::util::unpack(&packed)?;
        let node = Arc::new(OwnedNodeRef::new(unpacked.into_owned())?);
        self.client.process_node(node).await;
        Ok(())
    }

    /// Decoded outbound stanzas in transport-entry order. A pending offer is not yet completed.
    pub fn outgoing_stanzas(&self) -> Result<Vec<Node>> {
        let nodes = self
            .transport
            .outgoing
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        ensure!(
            nodes.len() <= LIMIT,
            "fixture outgoing observation limit exceeded"
        );
        Ok(nodes.clone())
    }

    /// Production event-bus events, including parsed incoming calls. No event injection API exists.
    pub fn events(&self) -> Result<Vec<Arc<Event>>> {
        let events = self.events.0.lock().unwrap_or_else(|e| e.into_inner());
        ensure!(
            events.len() <= LIMIT,
            "fixture event observation limit exceeded"
        );
        Ok(events.clone())
    }

    /// End the actual client connection and all registered calls.
    pub async fn shutdown(&self) -> Result<()> {
        self.client.disconnect().await;
        let reader = self.reader.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(reader) = reader {
            tokio::time::timeout(DEADLINE, reader).await???;
        }
        Ok(())
    }
}

impl Drop for CallFixture {
    fn drop(&mut self) {
        self.client.signal_shutdown_sync();
        self.transport.server_tx.close();
        self.offers.close();
        // Let the production reader run its cleanup; aborting it leaves live call handles behind.
    }
}

struct NoHttp;
#[async_trait]
impl HttpClient for NoHttp {
    async fn execute(&self, _: HttpRequest) -> Result<HttpResponse> {
        anyhow::bail!("call fixture forbids HTTP")
    }
}

struct NoRelay;
#[async_trait]
impl wacore::voip::RelayTransportProvider for NoRelay {
    async fn factory(
        &self,
        _: &wacore::voip::RelayEndpointParams,
    ) -> Result<Arc<dyn wacore::voip::RelayTransportFactory>> {
        anyhow::bail!("call fixture has no media relay; install a synthetic RelayTransportProvider")
    }
}

struct Factory(Arc<Wire>);
#[async_trait]
impl TransportFactory for Factory {
    async fn create_transport(
        &self,
    ) -> Result<(Arc<dyn Transport>, async_channel::Receiver<TransportEvent>)> {
        Ok((self.0.clone(), self.0.server_rx.clone()))
    }
}

enum State {
    Hello,
    Finish {
        noise: Box<NoiseHandshake>,
        ephemeral: KeyPair,
    },
    Established {
        read: NoiseCipher,
        write: NoiseCipher,
        received: u32,
        sent: u32,
    },
    Closed,
}

struct Wire {
    state: Mutex<State>,
    server_tx: async_channel::Sender<TransportEvent>,
    server_rx: async_channel::Receiver<TransportEvent>,
    offers: async_channel::Sender<PendingOffer>,
    outgoing: Mutex<Vec<Node>>,
}

impl Wire {
    fn frame(&self, body: &[u8]) -> Result<(Option<Bytes>, Option<Node>)> {
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        match std::mem::replace(&mut *state, State::Closed) {
            State::Hello => {
                let hello = waproto::codec::handshake_message_decode(body)?;
                let peer: [u8; 32] = hello
                    .client_hello
                    .into_option()
                    .context("client hello")?
                    .ephemeral
                    .context("client ephemeral")?
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("ephemeral length"))?;
                let identity = KeyPair::generate(&mut rand::rng());
                let ephemeral = KeyPair::generate(&mut rand::rng());
                let identity_pub: [u8; 32] = identity.public_key.public_key_bytes().try_into()?;
                let ephemeral_pub = ephemeral.public_key.public_key_bytes();
                let mut noise = NoiseHandshake::new(NOISE_PATTERN_XX, &WA_CONN_HEADER)?;
                noise.authenticate(&peer);
                noise.authenticate(ephemeral_pub);
                noise.mix_shared_secret(ephemeral.private_key.serialize(), &peer)?;
                let encrypted_static = noise.encrypt(&identity_pub)?;
                noise.mix_shared_secret(identity.private_key.serialize(), &peer)?;
                let payload = noise.encrypt(&wacore_noise::test_util::build_cert_chain_bytes(
                    &identity_pub,
                ))?;
                let response = waproto::codec::handshake_message_to_vec(&wa::HandshakeMessage {
                    server_hello: buffa::MessageField::some(wa::handshake_message::ServerHello {
                        ephemeral: Some(ephemeral_pub.to_vec()),
                        r#static: Some(encrypted_static),
                        payload: Some(payload),
                        ..Default::default()
                    }),
                    ..Default::default()
                });
                *state = State::Finish {
                    noise: Box::new(noise),
                    ephemeral,
                };
                Ok((
                    Some(wacore::framing::encode_frame(&response, None)?.into()),
                    None,
                ))
            }
            State::Finish {
                mut noise,
                ephemeral,
            } => {
                let finish = waproto::codec::handshake_message_decode(body)?
                    .client_finish
                    .into_option()
                    .context("client finish")?;
                let peer: [u8; 32] = noise
                    .decrypt(&finish.r#static.context("client static")?)?
                    .try_into()
                    .map_err(|_| anyhow::anyhow!("client static length"))?;
                noise.mix_shared_secret(ephemeral.private_key.serialize(), &peer)?;
                let _payload = noise.decrypt(&finish.payload.context("client payload")?)?;
                let (read, write) = noise.finish()?;
                *state = State::Established {
                    read,
                    write,
                    received: 0,
                    sent: 0,
                };
                Ok((None, None))
            }
            State::Established {
                read,
                write,
                received,
                sent,
            } => {
                let mut plain = BytesMut::from(body);
                read.decrypt_in_place_with_counter(received, &mut plain)?;
                let unpacked = wacore_binary::util::unpack(&plain)?;
                let node = OwnedNodeRef::new(unpacked.into_owned())?.get().to_owned();
                *state = State::Established {
                    read,
                    write,
                    received: received
                        .checked_add(1)
                        .context("receive counter exhausted")?,
                    sent,
                };
                Ok((None, Some(node)))
            }
            State::Closed => anyhow::bail!("fixture connection closed"),
        }
    }

    async fn reply(&self, node: Node) -> Result<()> {
        let frame = {
            let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
            let State::Established { write, sent, .. } = &mut *state else {
                anyhow::bail!("fixture handshake not complete");
            };
            let encrypted = write.encrypt_with_counter(*sent, &marshal(&node)?)?;
            *sent = sent.checked_add(1).context("send counter exhausted")?;
            wacore::framing::encode_frame(&encrypted, None)?
        };
        self.server_tx
            .send(TransportEvent::DataReceived(frame.into()))
            .await?;
        Ok(())
    }
}

#[async_trait]
impl Transport for Wire {
    async fn send(&self, data: Bytes) -> Result<()> {
        let hello = matches!(
            *self.state.lock().unwrap_or_else(|e| e.into_inner()),
            State::Hello
        );
        let mut remaining = if hello {
            data.strip_prefix(&WA_CONN_HEADER)
                .context("missing fixture connection header")?
        } else {
            &data
        };
        while !remaining.is_empty() {
            ensure!(remaining.len() >= 3, "short fixture frame");
            let length = (usize::from(remaining[0]) << 16)
                | (usize::from(remaining[1]) << 8)
                | usize::from(remaining[2]);
            ensure!(remaining.len() >= 3 + length, "truncated fixture frame");
            let (response, node) = self.frame(&remaining[3..3 + length])?;
            remaining = &remaining[3 + length..];
            if let Some(response) = response {
                self.server_tx
                    .send(TransportEvent::DataReceived(response))
                    .await?;
            }
            let Some(node) = node else {
                continue;
            };
            {
                let mut outgoing = self.outgoing.lock().unwrap_or_else(|e| e.into_inner());
                if outgoing.len() <= LIMIT {
                    outgoing.push(node.clone());
                }
                ensure!(
                    outgoing.len() <= LIMIT,
                    "fixture outgoing observation limit exceeded"
                );
            }
            let nr = node.as_node_ref();
            if nr.tag == "call" && nr.get_optional_child("offer").is_some() {
                let (completion, done) = oneshot::channel();
                self.offers.send(PendingOffer { node, completion }).await?;
                done.await
                    .context("fixture offer was dropped")?
                    .map_err(anyhow::Error::msg)?;
            } else if nr.tag == "iq" {
                let id = nr.attrs().optional_string("id").context("IQ id")?;
                let active = nr.get_optional_child("active").is_some();
                let mut reply = NodeBuilder::new("iq")
                    .attr("id", id.as_ref())
                    .attr("from", Jid::new("", Server::Pn))
                    .attr("type", if active { "result" } else { "error" });
                if !active {
                    reply = reply.children([NodeBuilder::new("error")
                        .attr("code", "503")
                        .attr("text", "unsupported fixture IQ")
                        .build()]);
                }
                self.reply(reply.build()).await?;
                if active {
                    self.reply(
                        NodeBuilder::new("ib")
                            .children([NodeBuilder::new("offline").attr("count", "0").build()])
                            .build(),
                    )
                    .await?;
                }
            }
        }
        Ok(())
    }

    async fn disconnect(&self) {
        self.server_tx.close();
    }
}

use super::util::{ContextExt, Result};
use super::{BackendFixture, DEADLINE, Session};
use crate::{Client, MessageId, SendOptions, SendRequest};
use rand::{SeedableRng, rngs::StdRng};
use std::sync::Arc;
use wacore::libsignal::protocol::{
    CiphertextMessage, ProtocolAddress, SignalMessage, UsePQRatchet, message_decrypt,
    message_encrypt,
};
use wacore::messages::decode_plaintext;
use wacore::proto_helpers::{MessageBuilderExt, MessageExt, build_quote_context_with_info};
use wacore::types::events::{Event, EventHandler, EventInterest, EventKind, Subscription};
use wacore::types::jid::JidExt;
use wacore_binary::{Jid, Node, Server, builder::NodeBuilder};
use waproto::whatsapp as wa;

pub const ROUNDS: u64 = 16;
const WARMUP: u64 = 2;
const PING: &str = "ping";
const PONG: &str = "pong";

struct Events(async_channel::Sender<Arc<Event>>);

impl EventHandler for Events {
    fn interest(&self) -> EventInterest {
        EventInterest::of(&[EventKind::Messages, EventKind::ServerAck])
    }

    fn handle_event(&self, event: Arc<Event>) {
        self.0.try_send(event).expect("ping/pong event queue");
    }
}

pub struct PingPong {
    session: Session,
    peer: Arc<Client>,
    peer_jid: Jid,
    own_address: ProtocolAddress,
    outgoing: async_channel::Receiver<Node>,
    events: async_channel::Receiver<Arc<Event>>,
    _subscription: Subscription,
    timestamp: i64,
    runtime: tokio::runtime::Runtime,
}

impl PingPong {
    pub fn new() -> Result<Self> {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .start_paused(true)
            .build()?;
        let (session, peer, peer_jid, own_address, outgoing, events, subscription) = runtime
            .block_on(async {
                let (outgoing_tx, outgoing) = async_channel::bounded(1);
                let session = Session::connect_with_capture(
                    BackendFixture::memory().backend(),
                    Some(super::OutgoingCapture::Messages(outgoing_tx)),
                )
                .await?;
                session.finish_control().await?;
                let client = &session.client;
                let snapshot = client.persistence_manager.get_device_snapshot();
                let own = snapshot.pn.as_ref().context("fixture PN")?;
                let own_lid = snapshot.lid.as_ref().context("fixture LID")?;
                let peer_jid = Jid::new(super::super::DM_PEER_LID, Server::Lid);
                client
                    .persistence_manager
                    .process_command(crate::store::commands::DeviceCommand::SetLidMigrated(true))
                    .await;
                super::super::seed_registry(client, own.user.as_ref(), &[own.device]).await;
                super::super::seed_registry(client, own_lid.user.as_ref(), &[own_lid.device]).await;
                super::super::seed_registry(client, peer_jid.user.as_ref(), &[0]).await;
                let peer = super::super::establish_acknowledged_session(client, &peer_jid).await;
                let (events_tx, events) = async_channel::bounded(2);
                let subscription = client.subscribe_handler(Arc::new(Events(events_tx)));
                Ok::<_, Box<dyn std::error::Error + Send + Sync>>((
                    session,
                    peer,
                    peer_jid,
                    own.to_protocol_address(),
                    outgoing,
                    events,
                    subscription,
                ))
            })?;
        let mut fixture = Self {
            session,
            peer,
            peer_jid,
            own_address,
            outgoing,
            events,
            _subscription: subscription,
            timestamp: wacore::time::now_secs(),
            runtime,
        };
        fixture.run_rounds(0..WARMUP)?;
        fixture.runtime.block_on(async {
            super::super::settle_signal_flush_worker(&fixture.session.client).await;
            fixture.session.settle().await
        })?;
        Ok(fixture)
    }

    pub fn run(&mut self) -> Result<()> {
        self.run_rounds(WARMUP..WARMUP + ROUNDS)
    }

    fn run_rounds(&mut self, rounds: std::ops::Range<u64>) -> Result<()> {
        self.runtime.block_on(async {
            let start = tokio::time::Instant::now();
            // Warmup and measurement must never repeat a seeded ratchet key.
            let mut rng = StdRng::seed_from_u64(0x5049_4e47_504f_4e47 ^ rounds.start);
            tokio::time::timeout(DEADLINE, async {
                for index in rounds {
                    self.exchange(index, &mut rng, |id| id.as_str()).await?;
                }
                Ok::<(), Box<dyn std::error::Error + Send + Sync>>(())
            })
            .await
            .context("ping/pong did not reach its matching ACK")??;
            ensure!(
                start.elapsed().is_zero(),
                "virtual time advanced during ping/pong"
            );
            Ok(())
        })
    }

    async fn exchange(
        &self,
        index: u64,
        rng: &mut StdRng,
        ack_id: impl FnOnce(&MessageId) -> &str,
    ) -> Result<()> {
        let ping_id = format!("3EB0000000000000{index:016X}");
        let pong_id = MessageId::new(format!("3EB1000000000000{index:016X}"))?;
        let mut plaintext = waproto::codec::message_to_vec(&wa::Message::text(PING));
        plaintext.push(1);
        let mut adapter = self.peer.signal_adapter();
        let ciphertext = message_encrypt(
            &plaintext,
            &self.own_address,
            &mut adapter.session_store,
            &mut adapter.identity_store,
        )
        .await?;
        ensure!(matches!(ciphertext, CiphertextMessage::SignalMessage(_)));
        self.session
            .wire
            .reply(
                NodeBuilder::new("message")
                    .attr("from", self.peer_jid.clone())
                    .attr("id", ping_id.as_str())
                    .attr("t", self.timestamp.to_string())
                    .attr("type", "text")
                    .children([NodeBuilder::new("enc")
                        .attr("type", "msg")
                        .attr("v", "2")
                        .bytes(ciphertext.serialize().to_vec())
                        .build()])
                    .build(),
            )
            .await?;
        let event = tokio::time::timeout(std::time::Duration::from_secs(3), self.events.recv())
            .await
            .with_context(|| format!("waiting for ping {index}"))??;
        let Event::Messages(batch) = &*event else {
            bail!("expected the ping message event");
        };
        let [ping] = &*batch.messages else {
            bail!("expected one ping message");
        };
        ensure!(ping.info.id == ping_id && ping.info.source.sender == self.peer_jid);
        ensure!(ping.message.text_content() == Some(PING));
        let quote = build_quote_context_with_info(
            ping_id.as_str(),
            &ping.info.source.sender,
            &ping.info.source.chat,
            &self.peer_jid,
            &ping.message,
        );
        let result = self
            .session
            .client
            .send(
                SendRequest::new(&self.peer_jid, wa::Message::text_with_context(PONG, quote))
                    .with_options(SendOptions::default().with_message_id(pong_id.clone())),
            )
            .await?;
        ensure!(result.message_id == pong_id);
        let node = tokio::time::timeout(std::time::Duration::from_secs(3), self.outgoing.recv())
            .await
            .with_context(|| format!("waiting for pong {index}"))??;
        ensure!(node.attrs().optional_string("id").as_deref() == Some(pong_id.as_str()));
        ensure!(node.attrs().optional_jid("to").as_ref() == Some(&self.peer_jid));
        let participants = node
            .get_optional_child("participants")
            .context("pong participants")?;
        let mut recipients = participants.get_children_by_tag("to");
        let recipient = recipients.next().context("pong recipient")?;
        ensure!(recipients.next().is_none());
        ensure!(recipient.attrs().optional_jid("jid").as_ref() == Some(&self.peer_jid));
        let enc = recipient
            .get_optional_child("enc")
            .context("pong ciphertext")?;
        ensure!(enc.attrs().optional_string("type").as_deref() == Some("msg"));
        let enc_ref = enc.as_node_ref();
        let ciphertext = enc_ref.content_bytes().context("pong ciphertext bytes")?;
        let plaintext = message_decrypt(
            &CiphertextMessage::SignalMessage(SignalMessage::try_from(ciphertext)?),
            &self.own_address,
            &mut adapter.session_store,
            &mut adapter.identity_store,
            &mut adapter.pre_key_store,
            &adapter.signed_pre_key_store,
            rng,
            UsePQRatchet::No,
        )
        .await?;
        let pong = decode_plaintext(&plaintext.plaintext, 2)?;
        ensure!(pong.text_content() == Some(PONG));
        let quote = pong
            .extended_text_message
            .as_option()
            .and_then(|text| text.context_info.as_option())
            .context("pong quote")?;
        ensure!(quote.stanza_id.as_deref() == Some(ping_id.as_str()));
        ensure!(quote.participant.as_deref() == Some(self.peer_jid.to_string().as_str()));
        ensure!(
            quote
                .quoted_message
                .as_option()
                .and_then(MessageExt::text_content)
                == Some(PING)
        );

        let mut ack = NodeBuilder::new("ack")
            .attr("class", "message")
            .attr("from", self.peer_jid.clone())
            .attr("id", ack_id(&pong_id));
        if let Some(phash) = node.attrs().optional_string("phash") {
            ack = ack.attr("phash", phash.as_ref());
        }
        self.session.wire.reply(ack.build()).await?;
        let event = tokio::time::timeout(std::time::Duration::from_secs(3), self.events.recv())
            .await
            .with_context(|| format!("waiting for ACK {index}"))??;
        let Event::ServerAck(ack) = &*event else {
            bail!("expected the pong ACK event");
        };
        ensure!(
            ack.id == pong_id.as_str()
                && ack.class.as_deref() == Some("message")
                && ack.error.is_none(),
            "expected a successful ACK for {pong_id}, got {ack:?}"
        );
        ensure!(self.events.is_empty() && self.outgoing.is_empty());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completed_exchange_requires_its_own_ack() -> Result<()> {
        let mut fixture = PingPong::new()?;
        fixture.run()?;
        let error = fixture
            .runtime
            .block_on(async {
                let mut rng = StdRng::seed_from_u64(0);
                tokio::time::timeout(
                    DEADLINE,
                    fixture.exchange(WARMUP + ROUNDS, &mut rng, |_| "WRONG-PONG-ID"),
                )
                .await
            })?
            .expect_err("an unrelated ACK must fail the exchange");
        assert!(error.to_string().contains("expected a successful ACK"));
        Ok(())
    }
}

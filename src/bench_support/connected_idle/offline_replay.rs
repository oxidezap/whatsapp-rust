use super::*;
use anyhow::ensure;
use wacore::time::Instant;
use wacore::types::events::{BatchOrigin, EventInterest, EventKind};

const PAGE_ITEMS: usize = 200;
pub const REPLAY_MESSAGES: usize = 7 * PAGE_ITEMS + 127;
pub const REPLAY_PAGES: usize = REPLAY_MESSAGES.div_ceil(PAGE_ITEMS);
const REPLAY_TEXT_BYTES: usize = 256;

struct PublicationOrder {
    next: [usize; LANES],
    sizes: Vec<usize>,
    correct: bool,
    first: Option<Instant>,
    last: Option<Instant>,
}

struct Publications {
    groups: Vec<Jid>,
    measure_wall: bool,
    state: Mutex<PublicationOrder>,
}

impl EventHandler for Publications {
    fn interest(&self) -> EventInterest {
        EventInterest::of(&[EventKind::Messages])
    }

    fn handle_event(&self, event: Arc<Event>) {
        let Event::Messages(batch) = event.as_ref() else {
            return;
        };
        let mut state = self.state.lock().unwrap_or_else(|p| p.into_inner());
        state.correct &= batch.origin == BatchOrigin::OfflineDrain && batch.hook_committed;
        state.sizes.push(batch.len());
        if self.measure_wall {
            let now = Instant::now();
            state.first.get_or_insert(now);
            state.last = Some(now);
        }
        for message in batch {
            let Some(index) = fixture_index(message.info.id.as_ref()) else {
                state.correct = false;
                continue;
            };
            let lane = index % LANES;
            state.correct &=
                index == state.next[lane] && message.info.source.chat == self.groups[lane];
            state.next[lane] += LANES;
        }
    }
}

fn fixture_index(id: &str) -> Option<usize> {
    id.strip_prefix("IDLE-")?
        .parse()
        .ok()
        .filter(|&i| i < REPLAY_MESSAGES)
}

struct Receipts {
    seen: [bool; REPLAY_MESSAGES],
    count: usize,
}

impl Receipts {
    fn record(&mut self, node: &Node) -> Result<()> {
        if node
            .attrs()
            .optional_string("type")
            .is_some_and(|kind| kind != "inactive")
        {
            return Ok(());
        }
        self.one(node)?;
        if let Some(items) = node
            .children()
            .and_then(|children| children.iter().find(|child| child.tag == "list"))
            .and_then(Node::children)
        {
            for item in items.iter().filter(|item| item.tag == "item") {
                self.one(item)?;
            }
        }
        Ok(())
    }

    fn one(&mut self, node: &Node) -> Result<()> {
        let index = node
            .attrs()
            .optional_string("id")
            .and_then(|id| fixture_index(&id))
            .context("unexpected offline receipt ID")?;
        ensure!(
            !self.seen[index],
            "duplicate receipt for offline message {index}"
        );
        self.seen[index] = true;
        self.count += 1;
        Ok(())
    }
}

pub struct Replay {
    session: Session,
    stanzas: Vec<Node>,
    outgoing: async_channel::Receiver<Node>,
    publications: Arc<Publications>,
    _publication_subscription: Subscription,
}

pub struct WallTimings {
    pub request_span: Duration,
    pub first_publication: Duration,
    pub last_publication: Duration,
    pub all_receipts: Duration,
}

impl Replay {
    pub async fn prepare(backend: Arc<dyn Backend>, measure_wall: bool) -> Result<Self> {
        let (tx, outgoing) = async_channel::bounded(REPLAY_MESSAGES + REPLAY_PAGES);
        let session =
            Session::connect_with_capture(backend, Some(OutgoingCapture::OfflineReplay(tx)))
                .await?;
        let (mut stanzas, groups, _, _) = session
            .prepare_encrypted_texts(REPLAY_MESSAGES, REPLAY_TEXT_BYTES)
            .await?;
        for node in &mut stanzas {
            node.attrs.insert("offline", "1");
        }
        let publications = Arc::new(Publications {
            groups,
            measure_wall,
            state: Mutex::new(PublicationOrder {
                next: std::array::from_fn(|lane| lane),
                sizes: Vec::with_capacity(4),
                correct: true,
                first: None,
                last: None,
            }),
        });
        let subscription = session.client.subscribe_handler(publications.clone());
        Ok(Self {
            session,
            stanzas,
            outgoing,
            publications,
            _publication_subscription: subscription,
        })
    }

    pub async fn receive(&mut self) -> Result<Option<WallTimings>> {
        let mut stanzas = std::mem::take(&mut self.stanzas).into_iter();
        let mut receipts = Receipts {
            seen: [false; REPLAY_MESSAGES],
            count: 0,
        };
        let start = self.publications.measure_wall.then(Instant::now);
        let mut request_times = [None; REPLAY_PAGES];
        self.session
            .wire
            .reply(
                NodeBuilder::new("ib")
                    .children([NodeBuilder::new("offline_preview")
                        .attr("count", REPLAY_MESSAGES.to_string())
                        .build()])
                    .build(),
            )
            .await?;

        for (page, request_time) in request_times.iter_mut().enumerate() {
            loop {
                let node = self.next_outgoing().await?;
                if node.tag == "receipt" {
                    receipts.record(&node)?;
                    continue;
                }
                let root = node.as_node_ref();
                let batch = root
                    .get_optional_child("offline_batch")
                    .context("expected offline pull")?;
                ensure!(batch.attrs().optional_u64("count") == Some(PAGE_ITEMS as u64));
                *request_time = start.map(|_| Instant::now());
                break;
            }
            let items = (REPLAY_MESSAGES - page * PAGE_ITEMS).min(PAGE_ITEMS);
            for node in stanzas.by_ref().take(items) {
                self.session.wire.reply(node).await?;
            }
        }
        ensure!(stanzas.len() == 0);

        self.session
            .wait_until(|| {
                self.session.client.inbound_commit_batch.pending_stats().0
                    + self.session.counts.committed.load(Ordering::Relaxed)
                    == REPLAY_MESSAGES
            })
            .await
            .context("messages did not reach the last offline batch")?;
        self.session.finish_offline(REPLAY_MESSAGES).await?;
        while receipts.count < REPLAY_MESSAGES {
            let node = self.next_outgoing().await?;
            ensure!(
                node.tag == "receipt",
                "offline pull beyond the declared backlog"
            );
            receipts.record(&node)?;
        }
        let all_receipts = start.map(|t| t.elapsed());
        self.session.settle().await?;
        while let Ok(node) = self.outgoing.try_recv() {
            ensure!(node.tag == "receipt", "offline pull after completion");
            receipts.record(&node)?;
        }
        ensure!(self.session.counts.committed.load(Ordering::Relaxed) == REPLAY_MESSAGES);

        let state = self
            .publications
            .state
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        ensure!(state.correct, "publication order or durability changed");
        ensure!(
            state.sizes == [400, 400, 400, 327],
            "unexpected commit batches: {:?}",
            state.sizes
        );
        Ok(start.map(|start| WallTimings {
            request_span: request_times[REPLAY_PAGES - 1]
                .unwrap()
                .saturating_duration_since(request_times[0].unwrap()),
            first_publication: state.first.unwrap().saturating_duration_since(start),
            last_publication: state.last.unwrap().saturating_duration_since(start),
            all_receipts: all_receipts.unwrap(),
        }))
    }

    async fn next_outgoing(&self) -> Result<Node> {
        Ok(tokio::time::timeout(DEADLINE, self.outgoing.recv()).await??)
    }

    pub async fn shutdown(self) -> Result<()> {
        self.session.shutdown().await
    }
}

# Poll/event creation and message secrets

## Creation results and references

`Polls::create` and `create_quiz` now return `CreatedPoll`, not `(SendResult,
Vec<u8>)`. `Events::create` returns `CreatedEvent`. Poll and quiz validation and
wire versions remain separate and unchanged.

```rust,ignore
let created = client.polls().create(&chat, "Question", &options, 1).await?;
client.polls().vote(&created.poll_ref()?, &selected_options).await?;
let result = created.send_result(); // creation envelope, not the vote's target
```

Both creation results expose `send_result()`, `creator()`, `secret()` and
`into_parts() -> (SendResult, Jid, MessageSecret)`. `poll_ref()` / `event_ref()`
borrow addressing and secret material, returning `Result<_, MessageRefError>`
like A06's `SendResult::message_ref`. Cloning a result shares the original
protobuf `Arc`; referencing it neither clones nor retains that protobuf.
The stored creator is the send branch's selected identity, not a post-send
routing lookup. The captured creator identity is our PN for ordinary DMs,
our LID for bot DMs, and the actual PN/LID sender selected for groups. The
secret itself is random protocol-sized bytes regardless of chat namespace.

For received or manually persisted creations, construct `PollRef::new` or
`EventRef::new` from a `MessageRef`, the original creator JID, and a borrowed
`MessageSecret`. The creator may be a PN/LID alias of the reference sender;
construction checks shape, not identity equivalence or secret association.
External hosts must supply metadata for the same original creation.
Canonical `Polls::vote(&PollRef, options)` and
`Events::respond(&EventRef, response, extra_guests)` use that metadata together.
Their parent key's `from_me` comes from the reference, including our own LID
creations. Crypto namespace selection and alias decryption remain unchanged.
`Polls::decrypt_vote_ref(ciphertext, &PollRef, voter)` is available alongside
explicit byte-based decryption/aggregation interop. Advanced positional sends
are named `vote_raw` / `respond_raw`; they validate secret length and ID first
and recognize our own PN or LID when deriving the parent key's `from_me`.
Invalid raw secrets retain the typed length error in `PollError::InvalidSecret`
or `SendError::InvalidSecret`, respectively.

Creation references address E2E chats, not newsletters/status/broadcast lists.
Those origins are rejected by these creation helpers before network work;
manual protobuf sending and newsletter publication remain available. The
creation envelope ID is not a later vote/RSVP's operation ID, and neither
result is a recipient-delivery guarantee.

## Secret safety

`MessageSecret` owns the existing protocol-sized array. Construct with
`from_bytes([u8; MESSAGE_SECRET_SIZE])` / `From<array>`, or validate through
`TryFrom<&[u8]>` / `TryFrom<Vec<u8>>`. Wrong lengths return
`InvalidMessageSecret { actual }`; bytes are not included in the error.
`as_bytes()` and `into_bytes()` deliberately expose material for crypto,
protobuf and storage interop. This type does not promise zeroization of these
copies.

**`Debug` redacts the entire `CreatedPoll`/`CreatedEvent`, not just its secret
field.** Their nested `SendResult.message` protobuf also contains the secret.
References and `MessageSecret` also have redacted Debug. Explicitly accessing
`send_result()` or consuming `into_parts()` exposes an unredacted `SendResult`:
do not log it or its protobuf/byte buffers.

All new types above, plus `StoredMessageSecret`, are exported from the crate
root and prelude. Core types are available in `wacore::types::message_secret`
and `wacore::store::traits`.

## Backend migration

`MsgSecretStore::get_stored_msg_secret(chat, sender, id)` is now a **required**
method returning `Result<Option<StoredMessageSecret>>`. There is deliberately no
default that discards a backend's known timestamp. An external backend can
implement the trait through the same async-trait contract; it is not sealed.

- `StoredMessageSecret::new(secret, Option<i64>)` supports external construction.
- `from_stored_bytes(&bytes, persisted_message_ts)` validates raw persisted data.
- The readable `secret` field is `MessageSecret`; `message_ts` is `Option<i64>`
  (Unix seconds). The old persisted unknown value `0` projects to `None`.
- Invalid persisted lengths return `StoreError::InvalidMessageSecret` with the
  typed length error as its source. Never truncate, pad, panic, or log key bytes.
- The tuple getter `get_msg_secret_with_ts` is removed. `get_msg_secret` is a
  documented byte-only interop projection through the required named read.
- The old fixed-array `MessageSecret` write alias is now `MessageSecretBytes`;
  `MsgSecretEntry.secret` still holds that array and its serialized form has not
  changed. Writing raw fixed arrays remains supported.

SQLite/in-memory reads, timestamp-aware buffered receive processing and actual
mock/probe adapters are migrated. SQLite retains write-queue ordering and
account scoping. This is an API projection, **not a schema migration**: existing
parent-time merges (unknown never overwrites known), expiry merges (later wins,
zero means never expire), retention pruning and serialized rows are unchanged.

The external construction/trait/boxed-future fixture is
`tests/fixtures/creation_consumer`; real synthetic transport and PN/LID Signal
wire proofs live in `src/send/tests/creation_tests.rs`. No authenticated
WhatsApp session or server acceptance is asserted by these tests.

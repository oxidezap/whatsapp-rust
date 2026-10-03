# A13 message request contract

## Surface

Imports are available from `whatsapp_rust` and `whatsapp_rust::prelude`:
`SendRequest`, `EditRequest`, `SendOptions`, `EditOptions`, `MessageId`,
`StanzaId`, `MessageRef`, `MessageSecret`, `SendResult`, `SendError`.
The requests also live in `whatsapp_rust::send`.

```rust
# use whatsapp_rust::{Client, Jid, SendRequest, SendOptions, MessageId, EditRequest, EditOptions, StanzaId};
# use whatsapp_rust::prelude::MessageBuilderExt;
# use whatsapp_rust::waproto::whatsapp::Message;
# async fn example(client: &Client, chat: &Jid) -> Result<(), whatsapp_rust::SendError> {
let sent = client.send(SendRequest::new(chat, Message::text("hello"))
    .with_options(SendOptions::default()
        .with_message_id(MessageId::new("CONTENT")?)
        .with_ephemeral_expiration(86400))).await?;
let edited = client.edit_message(
    EditRequest::new(sent.message_ref()?, Message::text("updated"))
        .with_options(EditOptions::default().with_stanza_id(StanzaId::new("EDIT")?))
).await?;
assert_eq!(edited.stanza_id().as_str(), "EDIT");
# Ok(())
# }
```

- `Client::send(SendRequest)` is non-generic. `SendRequest::new(&Jid, Message)`
  moves the owned protobuf into its `Arc` synchronously; `with_options` replaces
  defaults. No public fields, `Clone`, generic builder, chat handle/cache, or
  client ownership is added. The existing branch and outer boxing remain.
- `send_message(to, body)` remains the useful **default-content shortcut**,
  alongside `send_text`, `forward_message`, `MessageContext::reply` and
  `reply_quoting`. All go through `send`, not an independent pipeline.
  `send_message_with_options` is removed from the public API.
- `Client::edit_message(EditRequest<'_>)` replaces `edit_message_ref`, the
  positional `edit_message`, and `edit_message_with_options`.
  `EditRequest::new(MessageRef, Message)` borrows target metadata and moves
  content; `with_options(EditOptions)` configures the operation.
  Own-message and chat-origin validation precede identity/group lookup.
  Plaintext community-announcement edits remain rejected (metadata may be
  queried). A borrowed operation id still skips secret/retry-cache writes.
- `EditRequest::with_secret(&original_creator, &MessageSecret)` selects the
  encrypted edit path, with the host explicitly associating the original
  cryptographic namespace and secret, e.g. `CreatedEvent::creator/secret`.
  Neither literal PN/LID equality nor a later alias lookup determines the
  original creator. The current editor still uses existing chat routing.
  The private A07 `creation_sender`/DM capture during encryption is retained.
- Manual paths are `edit_message_raw(to, original_id, body, EditOptions)` and
  `edit_message_encrypted_raw(to, original_id, &[u8], body)`. The latter keeps
  the old current-chat-identity assumption and validates secret length; prefer
  the request to preserve original creation metadata across namespace changes.
- Newsletter edits/revokes are canonical
  `newsletter().edit_message(&NewsletterMessageRef, body)` and
  `newsletter().revoke_message(&NewsletterMessageRef)`. The positional escapes
  are `edit_message_raw`/`revoke_message_raw`. Client versus server ids and
  newsletter plaintext wire behavior are unchanged. This task does not rename
  unrelated reaction/pin/read methods.

## IDs, results, and optional absence

`SendOptions::message_id` is `Option<MessageId>` and `with_message_id` accepts a
`MessageId`. `EditOptions::stanza_id` is `Option<StanzaId>` and `with_stanza_id`
accepts a `StanzaId`. Empty inputs fail at ID construction, not after being
stored in an options DTO. Spelling (including Unicode) is preserved.

`SendResult::message_id` stores a validated `MessageId` rather than a mutable
`String` that could later invalidate its accessor. `stanza_id()` is infallible;
`StanzaId::from_message_id(&id)` is an explicit emitted-envelope projection,
not an implicit content/operation coercion or an original-target assertion.
For raw consumers use `as_str()`, `to_string()`, or `into_string()`.
Comparing raw string wire fields is supported without a `Deref` or automatic
cross-domain conversion.

`message_ref()`/`newsletter_ref()` remain fallible for **origin**, not ID
validation: a newsletter result cannot become an E2E chat key. These references
clone only the compact ID and borrow metadata, never the body. Results still
share the body `Arc`; normal `SendResult` Debug is sensitive as before. A07
creation-result Debug remains fully redacted.

No content/operation result split was introduced: real edit/revoke consumers
need the emitted envelope plus its original target key, and existing tests
explicitly distinguish these. Separate result types would not improve that
observed contract and would complicate A07 adapters.

`NewsletterMessage::message_id` is `Option<MessageId>`. An omitted `id` is
`None`, never an empty string; a present empty id fails validation. The history
parser still requires its real numeric server id, and newly sent posts still
have no manufactured server id. Other protocol strings, numeric domains,
status options, and message timestamps are not changed for uniformity.

## Provenance and evidence boundary

Initial isolated base: `9ceecd6ea1f7469c7c3307ce4cd2afbc6ceaf64c`.
Approved immutable A07 source: `9a5f22184659c901ba241bdc7ccabd08f6721b08`,
https://github.com/oxidezap/whatsapp-rust/pull/1610.
Preserving integration: `b309539d9f46f8d6908ca84306b53950ba2c2bd4`
(parents initial base and exact A07 source). Both MSRV consumer steps were kept
when reconciling the workflow conflict. Parent's later preserving `db794775`
contains the same production source, differing only in the order of those steps.
A06 is already integrated in the initial base.

Protocol evidence is the assessment's pinned whatspec/static AST evidence and
existing A06/A07 controls; no unavailable protocol skill or new bundle execution
is claimed. Tests use synthetic IDs/identities and transport, not authenticated
WhatsApp sends or evidence of recipient delivery. Validation outcomes and final
publication identities are recorded separately in the accompanying report.

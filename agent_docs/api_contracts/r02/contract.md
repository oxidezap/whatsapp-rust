# Typed message actions and extensible DTOs (R02 / R01 subset)

The primary names now accept the existing borrowed references:

| Owner | Method | Target / other arguments | Result |
| --- | --- | --- | --- |
| Client | send_reaction | &MessageRef, emoji | SendResult |
| Client | keep_message | &MessageRef, keep | SendResult |
| Client | pin_message | &MessageRef, duration | SendResult |
| Client | unpin_message, revoke_message | &MessageRef | SendResult |
| Comments | send_text, send_message | &MessageRef parent, text/body | SendResult |
| Newsletter | send_reaction, send_poll_vote | &NewsletterMessageRef, emoji/hashes | StanzaId |
| MessageContext | revoke_message | &MessageRef | SendResult |

The corresponding explicit `*_raw` methods retain chat/key/id/scope inputs.
The replaced `_ref` names are removed, not deprecated aliases. Raw newsletter
reaction/vote also return StanzaId. Newsletter edit/revoke retain their typed
primary methods, explicit raw escapes, and unit return: their builder reuses the
client-content ID, so there is no new operation ID to report.

```rust,ignore
client.send_reaction(&inbound.message_ref()?, "👍").await?;
client.pin_message(&sent.message_ref()?, PinDuration::Days7).await?;
ctx.revoke_message(&ctx.message_ref()?).await?;
client.comments().send_text(&parent.message_ref()?, "comment").await?;
let operation: StanzaId = client.newsletter().send_poll_vote(&post, &hashes).await?;
```

## Identity and scope

References preserve chat, origin, author and from_me. Own group add-ons resolve
missing authors from group routing and the matching own PN/LID; missing identity
is an error, not an invitation to fallback or query a new identity. A third-party
revoke is group-only and preserves the original author's namespace; the server
still decides admin permission. Revoke/pin/keep results describe the new operation
envelope, not the parent content.

Status reactions preserve author-device fanout. CAG reactions remain encrypted
with the captured parent secret, and comments retain their encrypted body, parent
author derivation, and fresh persisted comment secret. Typed comments reject
non-group origins before routing or secret lookup. For comments, callers supply
CAG posts: generic references do not attest subtype, and the comment path adds
no metadata queries/fallbacks to classify it. Reactions retain their existing
subtype lookup, including a metadata fetch when the cached flag is absent.
Explicit raw paths
keep their pre-existing interop behavior, including raw server-reaction status
delegation; they do not make a newsletter reference into a status reference.

Newsletter reactions/votes require server-content IDs. Edits/revokes require
client-content IDs. Zero is a valid server ID; missing IDs are never substituted.

## Construction and patterns

MessageContext, EventCreationParams and the previously open newsletter output
DTOs are now non-exhaustive. Their fields remain readable and mutable, and patterns
must use `..`. Use the existing bon builder convention rather than external
literals:

```rust,ignore
let params = EventCreationParams::builder().name("Launch".into()).build();
let ctx = MessageContext::builder()
    .message(message).info(info).client(client)
    .ephemeral_expiration(86400).build();
let metadata = NewsletterMetadata::builder()
    .jid(channel).name("Channel".into()).subscriber_count(0)
    .verification(NewsletterVerification::Unverified)
    .state(NewsletterState::Active).build();
let NewsletterMetadata { name, description, .. } = metadata;
```

MessageContext's from_parts/from_arc/from_inbound constructors remain supported;
from_inbound preserves inbound-only metadata. Its builder supports complete host
mocks. EventCreationParams has no Default: a name is required by the builder and
a blank name is rejected by create before sending. Optional fields are omitted
unless set (bon also supplies maybe_* setters for Option values).

NewsletterMetadata, NewsletterAdminProfile, NewsletterAdminInfo,
NewsletterFollower, NewsletterReactionCount, NewsletterPollVote and
NewsletterMessage all have builders for mocks. Existing non-exhaustive
NewsletterMyAddOns/MyReaction/MyPollVote IQ outputs have builders too. Required
observations have no fabricated defaults. NewsletterAdminInfo alone has a neutral
Default (all observations absent); history's reaction/vote collections default
to empty. No host traits are sealed.

## Evidence

`src/send/tests/message_reference_tests.rs` retains transport/identity/scope/ID
regressions under the primary names. `action_identity_tests.rs` adds outgoing CAG
reaction/comment decryption, own/received PN/LID parents, persisted comment-secret
lookup, invalid-origin/missing-secret rejection, context scope and blank events.

Independent consumer: `tests/fixtures/actions_consumer/Cargo.toml`. Native tests
and rustdoc cover primary/raw names, boxed Send futures, output mocks, extensible
patterns and separate negative controls for removed spellings, old raw arities,
literals, exhaustive patterns and invalid event defaults. Nightly rustdoc checks
the diagnostic-code annotations; stable checks failure without code matching.
The same crate checks on Rust 1.94 and wasm32-unknown-unknown (WASM futures use
?Send, no native runtime dependencies). Workflows/registry are integrated by the
consumer-CI owner rather than this domain change. Existing creation/requests
consumers are migrated to the event builder. Tests use synthetic identities;
these assertions do not claim real-server delivery or permission verification.

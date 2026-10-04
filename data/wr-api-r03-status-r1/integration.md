# R03 / R01 status integration

Branch: `fm/wr-api-r03-status-r1`, based on default `e32ec562a207c95188693ec1a2738ea69c08fa48` (verified against fetched `origin/main`).
Owned paths: `src/features/status.rs`, status-only parts of `src/send/mod.rs`, and `tests/fixtures/status_consumer/`. No exports, workflows, registry, shared manifests, or shared locks changed.

## API / migration

`StatusSendOptions` is now `#[non_exhaustive]`. Construct with neutral `Default` and fluent `with_*` setters, following the existing `SendOptions` / `EditOptions` conventions. Default remains valid **configuration**: recipients are a separate required argument, not an empty required field hidden inside the options. Public fields stay readable/mutable; patterns need `..`.

```rust,no_run
# async fn example(client: &whatsapp_rust::Client) -> anyhow::Result<()> {
use whatsapp_rust::{Jid, MessageId, StanzaId, StatusPrivacySetting, StatusSendOptions};
use whatsapp_rust::waproto::whatsapp::message::extended_text_message::FontType;
let recipients = [Jid::pn("15550000001")];
// Use a compatible explicit list. A missing observed audience is not permission
// to send to all contacts; privacy never automatically filters this array.
let observed = client.status().audience();
let post = client.status().send_text(
    "Hello", 0xFF000000, FontType::SYSTEM, &recipients,
    StatusSendOptions::default()
        .with_privacy(StatusPrivacySetting::AllowList)
        .with_message_id(MessageId::new("STATUS-CONTENT")?),
).await?;
let revoke = client.status().revoke(
    post.message_id, &recipients,
    StatusSendOptions::default().with_stanza_id(StanzaId::new("REVOKE-OPERATION")?),
).await?;
let operation = revoke.stanza_id(); // NOT the target content ID
# Ok(())
# }
```

- `message_id: Option<MessageId>` overrides the status **post content** ID.
- New `stanza_id: Option<StanzaId>` overrides a revoke/raw reaction **operation** ID.
- `Status::revoke` takes `MessageId`, no implicit string input.
- Migrate old post overrides using `MessageId::new(raw)?`; migrate old revoke overrides using `StanzaId::new(raw)?` into `stanza_id`, not `message_id`.
- Both domains reject empty strings at construction. The existing runtime empty-ID guard remains. Revoke outer/target collisions still return `SendError::InvalidRequest` before identity/network work.
- Wrong-role or simultaneous overrides return `InvalidRequest`, never silently ignore an ID.
- Additional construction: `with_extra_stanza_nodes(Vec<Node>)` and `with_device_freshness(Freshness)`; no required knobs and no new optional builder dependency needed.
- `SendResult` remains the common send result. Its existing `stanza_id()` describes the emitted operation for revoke, not the embedded target. No broad result-type redesign or R02 edits.

## Wire evidence / limits

Queried pinned whatspec IR first:
`https://raw.githubusercontent.com/oxidezap/whatspec/1a441f0329c941fcdb238490a6c604550d8a9939/generated/stanza/index.json`.
`WAWebEncryptAndSendStatusMsg` has required outer `id` sourced from `sendMsgRecord.data.id.id`.
Current Rust wire construction sends `request_id` to `GroupStanzaRequest.message_id`, recent-message cache, phash correlation, and `SendResult`; revoke's target is separately encoded in `ProtocolMessage.key.id`. This supports different domains without changing wire spelling.
Existing `status_carries_privacy_meta` already unwraps message wrappers and distinguishes content posts from revoke/reaction operations; ID role selection uses that same classification.
Raw captured JS is absent from this worktree. No server-authenticated delivery, live privacy reconciliation, or new protocol behavior claimed.

## Independent fixture / registry handoff

Manifest: `tests/fixtures/status_consumer/Cargo.toml` (standalone workspace, no default runtime features; own lock).

Positive modes:
- `cargo test --locked --manifest-path tests/fixtures/status_consumer/Cargo.toml`
- Same command on MSRV `+1.94.0`.
- `cargo check --locked --manifest-path tests/fixtures/status_consumer/Cargo.toml --target wasm32-unknown-unknown` with `RUSTFLAGS='--cfg getrandom_backend="wasm_js"'`.
- `cargo clippy --locked --manifest-path tests/fixtures/status_consumer/Cargo.toml --all-targets -- -D warnings`

Negative controls: run `cargo check --locked --manifest-path tests/fixtures/status_consumer/Cargo.toml --features <feature>` separately. Each must fail for the named API cause (not a dependency/build failure):

| Feature | Expected diagnostic |
| --- | --- |
| `old-message-id-string` | E0308, `String` instead of `MessageId` |
| `old-revoke-string` | E0308, string instead of revoke target `MessageId` |
| `wrong-message-id-domain` | E0308, `StanzaId` instead of `MessageId` |
| `wrong-stanza-id-domain` | E0308, `MessageId` instead of `StanzaId` |
| `old-options-literal` | E0639, external non-exhaustive struct construction |

Do not enable these negative features in the positive fixture. Native future requires `Send`; WASM future explicitly does not.

Directed library tests cover option setters/defaults, empty IDs, collision, wrong-role/simultaneous overrides, posts/revokes requiring explicit recipients, unknown audience, and privacy modes not substituting audience inference for identity checks. The pre-existing status distribution lock/recipient tests remain intact.

## Validation

- Formatting and diff whitespace checks passed.
- Native MSRV focused tests, standalone consumer, rustdoc, clippy, and negative controls queued under shared build flock with jobs=1; results pending.
- First native attempt encountered an existing shared target compiler mismatch (`rustversion` built with April nightly, active June nightly); no shared artifacts cleaned. Validation now uses an isolated local target directory and direct MSRV cargo.

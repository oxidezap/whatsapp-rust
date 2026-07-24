# E2E Testing Best Practices

E2E tests live in `tests/e2e/` and run against a mock WhatsApp server. They test real connection flows, encryption, and event delivery.

## Test Infrastructure

- **`tests/e2e/src/lib.rs`**: `TestClient` helper — connects to mock server, waits for pairing + sync, provides event-based assertions.
- Each `TestClient` owns an isolated `InMemoryBackend`; the mock server is shared.
- Libtest runs tests within a test binary **in parallel** by default. Never rely on test order.
- Use `unique_push_name()` for server-side account isolation. For a multi-device test,
  create one unique name and pass it only to that test's related clients.
- CI pins the mock-server image by digest. Update it deliberately with the matching
  server change so an unchanged client commit always runs against the same protocol peer.
- Local runs must start the mock with `CHATSTATE_TTL_SECS=3`; `chatstate_ttl.rs`
  intentionally uses the same shortened expiry as CI.

## File Organization

Split test files by domain so ownership and failures stay clear. Do not use file boundaries
as a synchronization mechanism; correctness must not depend on how Cargo schedules test targets.

```
tests/e2e/tests/
├── chat_actions.rs      # Pin, mute, archive, star
├── connection.rs        # Connect, reconnect
├── groups.rs            # Group CRUD, admin, settings
├── media.rs             # Upload, download, send media
├── messaging.rs         # Send/receive text messages
├── chatstate_ttl.rs     # Chatstate expiry with the CI's 3-second mock TTL
├── offline_groups.rs    # Offline group notifications
├── offline_messages.rs  # Offline message queuing + delivery
├── offline_receipts.rs  # Offline receipt + presence delivery
├── presence.rs          # Typing indicators, availability
├── profile.rs           # Push name, status text
├── profile_picture.rs   # Profile picture CRUD
└── receipts.rs          # Online receipt routing
```

When adding new tests, place them in the file matching their domain. If a file grows beyond ~10-15 tests, consider splitting further.

## Recovery and Race Regressions

Make recovery tests deterministic with narrow `test-util` fault hooks, then wait for an
observable event, stanza, or bounded state transition. Do not depend on CPU load to hit a race, and
do not raise a readiness timeout to hide a failed bootstrap phase. `app_state.rs`'s missing-key
test is the reference pattern: remove exactly the required state, trigger a real sync, and
assert that recovery reaches the wire.

## Event-Driven Waiting (Preferred)

Use `wait_for_event()` with predicates instead of arbitrary sleeps. This is both faster and more reliable:

```rust
// GOOD: event-driven — returns as soon as the event arrives
let event = client_b
    .wait_for_event(15, |e| e.messages().any(|m| m.message.conversation.as_deref() == Some("hello")))
    .await?;

// BAD: arbitrary sleep — wastes time or causes flaky failures
tokio::time::sleep(Duration::from_secs(2)).await;
```

Reference: `groups.rs` uses zero sleeps and runs at ~2.2s/test. Follow this pattern for new tests.

## Offline Testing Pattern

`reconnect()` tears the socket down in a background task, so the client is still
online when it returns. Poll for the transition with `wait_for_disconnected()`
instead of sleeping — `Event::Disconnected` is suppressed for expected
disconnects, so the connection flag is the observable:

```rust
// Client goes offline (triggers auto-reconnect in background)
client_b.client.reconnect().await;
client_b.wait_for_disconnected(5).await?;

// Now send while client is offline — server queues it
client_a.client.send_message(jid_b.clone(), message).await?;

// Client reconnects automatically and receives from offline queue
let event = client_b.wait_for_event(30, |e| matches!(e, Event::Messages(_))).await?;
```

Full disconnects need nothing: `TestClient::disconnect()` awaits the run task, so
the client is normally already offline when it returns. It caps that wait at 5s
and warns instead of failing, so a test that depends on the client being offline
afterwards should still assert it rather than assume it.

```rust
client_b.disconnect().await;
```

## `reconnect_and_wait()` Helper

Use `TestClient::reconnect_and_wait()` when you need the client back online (not testing offline behavior):

```rust
// Reconnects and waits for Connected event — no arbitrary sleep needed
client_b.reconnect_and_wait().await?;
```

Do NOT use this for offline tests — it waits for the client to be back online, defeating the purpose.

## Timeout Guidelines

- **Event waits in online flows**: 10-15s (events arrive in <1s normally)
- **Event waits after offline reconnect**: 30s (reconnect + offline queue drain)
- **Negative assertions** (event should NOT arrive): 3-5s
- **Going offline** (`wait_for_disconnected`): 5s

Never synchronize on a fixed sleep: long enough to be reliable is slow, short
enough to be fast is flaky. Wait on the condition itself — an event, or a poll
with a bounded deadline that fails with a clear message.

## Writing New E2E Tests

1. Use `TestClient::connect("unique_prefix")` with a unique prefix per client per test.
2. Use `wait_for_event()` for all assertions; for state with no event, poll it with a bounded deadline. Never sleep.
3. Always call `disconnect()` on all clients at the end (cleanup).
4. Return `anyhow::Result<()>` for clean error propagation.
5. Use `env_logger` for debug output: `let _ = env_logger::builder().is_test(true).try_init();`

# E2E tests

`tests/e2e/` runs real connection, encryption, and event-delivery flows against a mock WhatsApp server. `tests/e2e/src/lib.rs` holds `TestClient`, which connects, waits for pairing and sync, and provides the event-based assertions.

## The one rule

**Never synchronize on a fixed sleep.** Long enough to be reliable is slow; short enough to be fast is flaky. Wait on the condition itself — an event, or a bounded poll that fails with a clear message.

```rust
// Returns as soon as the event arrives
let event = client_b
    .wait_for_event(15, |e| e.messages().any(|m| m.message.conversation.as_deref() == Some("hello")))
    .await?;
```

`groups.rs` uses zero sleeps and runs at ~2.2s per test. For state with no corresponding event, poll it with a deadline; raising a readiness timeout to make a failing bootstrap pass is not a fix.

Timeouts that hold up in practice: 10-15s for event waits in online flows (events normally arrive in under a second), 30s after an offline reconnect (reconnect plus queue drain), 3-5s for negative assertions, 5s for `wait_for_disconnected`.

## Isolation

Each `TestClient` owns an isolated `InMemoryBackend`; the mock server is shared. CI runs the suite under `cargo nextest run --profile e2e -p e2e-tests`, which schedules across binaries — tests from different files run at the same time, each in its own process — so nothing may depend on test order, and file boundaries are organization, not synchronization.

`unique_push_name()` gives server-side account isolation — it appends a fresh UUID, so two clients built from the same prefix still land on different accounts. Sharing an account is therefore explicit: build one name and hand it to each device with `connect_as(prefix, &name)`, which pairs them to the same phone number under different device IDs.

CI pins the mock-server image by digest, so an unchanged client commit always runs against the same protocol peer; bump it deliberately, together with the matching server change. Local runs need `CHATSTATE_TTL_SECS=3` on the mock — `chatstate_ttl.rs` depends on the same shortened expiry CI uses.

## Connect, disconnect, reconnect

The distinction that catches people: **`reconnect()` tears the socket down in a background task**, so the client is still online when it returns. Do not wait for `Event::Disconnected` — it is suppressed for expected disconnects and will never arrive. Observe the connection state instead, which is what `wait_for_disconnected()` polls.

```rust
client_b.client.reconnect().await;
client_b.wait_for_disconnected(5).await?;

// Now offline — the server queues this
client_a.client.send_message(jid_b.clone(), message).await?;

// Auto-reconnect drains the offline queue
let event = client_b.wait_for_event(30, |e| matches!(e, Event::Messages(_))).await?;
```

`TestClient::disconnect()` awaits the run task, so the client is normally already offline on return — but it caps that wait at 5s and warns rather than failing, so a test that depends on being offline afterwards should assert it.

`reconnect_and_wait()` waits for the client to come back online. Using it in an offline test defeats the test.

## Recovery and race regressions

Make these deterministic with narrow `test-util` fault hooks, then wait for an observable event, stanza, or bounded state transition. Never depend on CPU load to hit a race. The reference pattern is `app_state.rs`'s missing-key test: remove exactly the required state, trigger a real sync, assert that recovery reaches the wire.

## Writing a new test

Put it in the file matching its domain, or add one — if a file passes ~10-15 tests, split it. Use `TestClient::connect("unique_prefix")` with a prefix unique per client per test, return `anyhow::Result<()>`, `disconnect()` every client at the end, and initialize logging with `let _ = env_logger::builder().is_test(true).try_init();`.

Cover the failure alongside the success: a guard with two conditions needs both negatives.

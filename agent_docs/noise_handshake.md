# Noise handshake

Three Noise patterns coexist, mirroring WA Web's `WAWebOpenChatSocket`.

| Pattern | When | State machine | Cost |
| --- | --- | --- | --- |
| **XX** | First connect / pairing / forced fallback | `XxHandshakeState` | 1.5 RTT |
| **IK** | Reconnect with a valid cached `serverStaticPub` | `IkHandshakeState` | 1 RTT, ships 0-RTT login payload |
| **XXfallback** | Server rejects an in-flight IK (reply has `static != null`) | `XxFallbackHandshakeState` | 1 RTT, reuses the already-sent ephemeral |

## Selection (`src/handshake.rs::select_pattern`)

```text
ik_failures >= 1  ───────────────────────────────────────► XX
no cached server_cert_chain ─────────────────────────────► XX
leaf.not_after < now OR intermediate.not_after < now ────► XX
otherwise ──────────────────────────────────────────────► IK with leaf.key
```

`Client.ik_handshake_failures: AtomicU32` is per-process and deliberately not persisted, matching WA Web's `K = 0` reset on process start.

## Invalidation policy

| Error | `ik_handshake_failures` | `server_cert_chain` |
| --- | --- | --- |
| Transient (timeout, disconnect, transport) | unchanged | unchanged |
| Crypto-fatal during IK (cert MAC, decrypt, proto) | `+= 1` | cleared via `DeviceCommand::ClearServerCertChain` |
| XX or XX-fallback failure | unchanged | unchanged (XX never reads the cache) |
| Any successful handshake | reset to `0` | repopulated (XX, XX-fallback) or kept (IK Continue) |

The split is `HandshakeError::is_transient()` vs `is_crypto_fatal()`. Misclassifying either way is the failure mode to watch for: too eager and the client oscillates back to XX for nothing, too lax and it loops on a stale cache.

## Persisted state

`Device.server_cert_chain` holds `CachedServerCertChain { intermediate, leaf }`, each cert reduced to `{ key: [u8; 32], not_before: i64, not_after: i64 }` — the same fields WA Web writes in `PrefsInfoStore.js:setCertificateChain`.

`verify_server_cert` checks structural shape, the issuer-serial pin, the chain link, and that `leaf.key` matches the decrypted Noise static. Ed25519 signature verification against `WA_CERT_PUB_KEY` is intentionally skipped — it would break the e2e mock server, and whatsmeow takes the same posture. The constant staying unused is deliberate.

## Logs

These lines mirror WA Web's `[socket]` output, which makes a captured session and a local run directly comparable:

```text
[socket] doFullHandshake: openChatSocket send hello
[socket] resumeNoiseHandshake started
[socket] resumeNoiseHandshake send hello
[socket] resumeNoiseHandshake rcv hello
[socket] resumeNoiseHandshake deriving secrets
[socket] resumeNoiseHandshake failed: serverStaticCiphertext not null —
  doFallbackHandshake continuing handshake with given server hello
[socket] continueFullHandshakeCore client finish and deriving secrets
```

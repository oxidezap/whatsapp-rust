# Noise handshake

Three Noise patterns coexist, mirroring WA Web's `WAWebOpenChatSocket`.

| Pattern | When | State machine | Cost |
| --- | --- | --- | --- |
| **XX** | First connect / pairing / forced fallback | `XxHandshakeState` | 1.5 RTT |
| **IK** | Reconnect with a valid cached `serverStaticPub` | `IkHandshakeState` | 1 RTT, ships 0-RTT login payload |
| **XXfallback** | Server rejects an in-flight IK (reply has `static != null`) | `XxFallbackHandshakeState` | 1 RTT, reuses the already-sent ephemeral |

## Selection (`src/handshake.rs::select_pattern`)

```text
device not registered ───────────────────────────────────► XX
ik_failures >= IK_FAILURE_THRESHOLD ─────────────────────► XX
no cached server_cert_chain ─────────────────────────────► XX
now outside [not_before, not_after) of leaf OR intermediate ► XX
otherwise ──────────────────────────────────────────────► IK with leaf.key
```

Both ends of the validity window are checked on both certs: `not_after` is ordinary expiry, `not_before` catches backwards clock skew. The unregistered gate exists for legacy databases written before the registration gate, which can hold a cached chain that IK must refuse.

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

`verify_server_cert` checks structural shape, the issuer-serial pin against `WA_CERT_ISSUER_SERIAL`, the chain link (the leaf's issuer serial must equal the intermediate's serial), that `leaf.key` equals the decrypted Noise static, and **both XEdDSA signatures**: the intermediate's over `WA_CERT_PUB_KEY`, then the leaf's over the intermediate's key.

Signature verification is bypassed only under `cfg(test)` and the `danger-skip-cert-chain-verify` feature, which exist so callers can drive the surrounding code against zero-signed fixtures — `tests/e2e` enables the feature because the mock server does not sign its chain. **Production builds verify.** If you are changing this path, `wacore/noise/tests/cert_chain_verify.rs` is compiled with `#![cfg(not(feature = "danger-skip-cert-chain-verify"))]` precisely so the real path keeps coverage.

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

Every line above is `debug`, including `resumeNoiseHandshake failed`: a server that declines the IK resume is the ordinary trigger for XXfallback, not a failure of ours. The pattern that actually completed is what gets reported at `info`, as "Handshake complete (IK|XX|XXfallback)". None of these pattern diagnostics warns; other parts of connection setup still do on their own terms (an oversized `edge_routing_info` being dropped, for one).

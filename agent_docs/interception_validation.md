# Validating interception acknowledgements

Interception changes which handler owns an input, not the response class.
Keep `PluginStanzaInterception` documentation consistent with the
`client::interceptor` rustdoc and the executed claim-path matrix below.
Do not infer server delivery, retention or redelivery from a local send.

| Claimed input with `id` and `from` | Generic SDK response after the claim | Claimant obligation |
| --- | --- | --- |
| Direct/group `<message>` | None | Own decryption, Signal/session state, persistence and the appropriate receipt/error policy. |
| `<iq>` other than protected ping | None | Supply the appropriate IQ response through the supported reply contract. |
| Newsletter `<message>` | Transport ACK | Own handling; an ACK does not prove application persistence. |
| Status-broadcast `<message>` or `<status>` | Transport ACK in this SDK | Own handling. The SDK's status fallback is not a claim of identical Web success-path behavior. |
| `<receipt>`, `<notification>`, `<call>` | Transport ACK | Own effects; configured group capture must finish successfully first. |
| Unmodelled tag | Transport ACK | Own the claimed input. Missing addressing identity produces no generic ACK. |

Message/status ACK encoding also requires a configured local device PN to
stamp `from`. The matrix fixture supplies that identity through `DeviceCommand`;
an unlinked fixture cannot demonstrate these positive ACK cases.

`success`, `failure`, `stream:error`, `ack`, and server pings never reach an
interceptor. Request correlation and connection housekeeping precede claims.

`client::tests::claimed_stanza_ack_matrix_preserves_each_response_class` processes
all rows through the client and decrypts emitted mock-transport frames. It checks
both absence of generic answers and exact ACK class/ID without a NACK. The group
capture tests in `handlers::notification::durability_tests` independently check
that blocked, failed or retired capture cannot reach the interceptor/ACK, and
successful capture precedes the ACK.

```console
cargo nextest run -p whatsapp-rust --lib -E 'test(claimed_stanza_ack_matrix) | test(a_claimed_stanza_without_identity) | test(interceptor_claim_cannot_bypass_failed_group_capture) | test(interceptor_claim_ack_follows_successful_group_capture) | test(retirement_during_group_capture) | test(interceptor_retiring_connection_after_capture)'
```

## Pinned Web comparison

The comparison uses whatspec commit
`c581c20a9d0605df49abf297f2e7cfc04ccf1ff7`, schema `4.4.0`, WhatsApp Web
`2.3000.1047483476`. All 579 restored bundles were checked for SHA-256 and size
against that commit's `generated/bundles.lock.json`. The preserved release asset
is `bundles-2.3000.1047483476-05609307e68b0f6ccfd2a121a573049999e38b752cfe7155ebbe5c661805751b.tar.xz`
in the `bundle-store` release; the set hash is
`05609307e68b0f6ccfd2a121a573049999e38b752cfe7155ebbe5c661805751b`.

Full `__d` module definitions were extracted with an AST parser, without
executing the client. Offsets are original UTF-8 byte ranges, end exclusive.
The selected names have one definition each in this set.

| Module | Bundle SHA-256 | Byte range | Static observation |
| --- | --- | --- | --- |
| `WAWebHandleMsg` | `09d91fdc4089c50ab1d35dd6c796ee82ef4c3296710316d20383c85738b871b8` | `1605672..1619716` | Decrypt/process outcome selects receipt, retry, backfill or error behavior; no unconditional generic message ACK. |
| `WAWebHandleMsgSendReceipt` | same | `1588522..1595333` | Successful ordinary delivery selects the delivery-receipt job. Medianotify, backfill, Coex fallback and error outcomes have distinct guards. |
| `WAWebSendDeliveryReceiptJob` | same | `1586221..1588520` | Constructs a `receipt` and calls `deprecatedCastStanza`; status context changes the receipt class. |
| `WAWebHandleMsgSendAck` | same | `756591..757863` | ACK and NACK are separate explicit send paths through `deprecatedCastStanza`. |
| `WAWebCommsHandleWorkerCompatibleStanza` | same | `1775893..1778749` | Newsletter messages route through their own handler; they cannot justify a universal direct-message ACK claim. |
| `WAWebCommsHandleStanza` | `f588ee2f033b87e018a3be34a37ec1347d8b6b8700de539e6be31481b29c4690` | `395958..397496` | IQ uses the common IQ handler rather than a generic message ACK. |
| `WAWebHandleStanzaCommon` | same | `394109..394625` | Ping constructs an IQ result; pairing routes separately; other namespaces throw. |

These observations justify distinguishing response classes. They are static
Web evidence, not authenticated execution, server behavior, a universal IQ
extension policy, or Android/iOS equivalence. The SDK's plugin takeover and
unknown-tag policy are SDK contracts; the Web client has no corresponding
third-party plugin contract. Preserve that distinction when changing tests.

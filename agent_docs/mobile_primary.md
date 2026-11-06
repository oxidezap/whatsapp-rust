# Mobile primary companion hosting

The `mobile-core` SDK exposes `Client::mobile_primary()` for a registered,
bound primary session. Its crypto and IQ primitives live in
`wacore::mobile::primary`; the SDK composes them with existing persistence,
Signal encryption and transport paths. Follow [mobile_api.md](mobile_api.md)
for session, clock, trust and media configuration.

## Approval flow

Initialize a fresh hosting journal once with
`initialize(account_raw_id, AdvDefaultEncoding)`. Restore its existing state on
subsequent runs. The account epoch and signed protobuf field-presence profile
are explicit inputs; initialization must not silently replace an existing epoch.

For QR approval, parse `CompanionQr`, reserve the approval, upload it, then
confirm the companion's signed proof:

```rust,ignore
let primary = client.mobile_primary()?;
let invitation = CompanionQr::parse(qr_text)?;
primary.prepare_qr(&invitation, timestamp_seconds, &[], rng).await?;
let device = primary.upload_prepared(server, client_props).await?;
primary.confirm(invitation.reference().as_bytes(), &device, proof).await?;
```

Here the empty slice supplies no additional host-known valid indexes. The SDK
still queries a fresh complete device list and authenticates its signed indexes.
Select `client_props`, server JID and optional `PairDevicePem` from the intended
profile; `upload_prepared_with_pem` adds explicitly supplied PEM fields.
`derive_companion_pem_key` supplies the pure derivation without choosing a TTL,
key identifier, algorithm or version.

For phone-code approval, the host supplies the authenticated companion hello,
code, fresh ephemeral key, salt and IV to `prepare_primary_hello`. Pass the result
with the invitation reference, companion noise key and platform to
`send_code_hello`. It returns an in-memory `PrimaryCodeExchange`. Route the
matching finish to `prepare_code_finish`, then use the same upload and proof
confirmation steps as QR. Keep notification routing bound to the authenticated
session and exchange; the exchange itself cannot be serialized or logged.

An upload result assigns a device JID. `confirm` additionally requires the exact
reference, issued identity details, trusted account signature, companion
signature and a fresh authenticated device/index observation. An IQ result alone
must not publish a proof-confirmed companion.

## Device indexes and approval recovery

The allocation high-water mark is durable and monotonic. A cancelled or abandoned
index stays spent. It can remain the signed list's `current_index` after it leaves
`valid_indexes`; these fields have different purposes. Fresh device snapshots
preserve other devices' valid indexes, and publication rejects timestamp
regression or detected topology drift. There is no server-side compare-and-swap
guarantee. A companion's PN and LID addresses can name the same owned device
slot. Reconcile these aliases using the authenticated account mappings and key
index; revocation excludes that index from every signed-list source and removes
its local records without rewriting another device's proof address.

Use `pending_stage()` to inspect an interrupted approval. `cancel_prepared` only
cancels an unsent prepared operation. After an upload was attempted,
`settle_unsuccessful_approval` releases its active slot while retaining a bounded
public recovery tombstone. Its observation distinguishes an advertised device,
an advertised index without a device, and an index not advertised in the queried
snapshots. None proves that a delayed result can never land.

Inspect `abandoned_approvals()` and use `recover_abandoned_approval` for a late
result with the original reference, peer identity, proof and fresh device/index
binding. Explicit `revoke` can remove an authenticated own device even when it
was not locally proof-confirmed. `revoke_all` includes hosted devices.
`reconcile` refreshes observed topology without rewinding the allocation mark or
inventing a companion proof.

Revocation and index publication retain maintenance intent across interruption.
Use `resume_maintenance` after settling the transport outcome. When account
topology changed, `refresh_maintenance_indexes` re-signs against a new snapshot
before retry. Do not discard recovery records to work around journal capacity.

## Bootstrap and history ownership

`bootstrap(device, history)` accepts an explicit initial-bootstrap payload with
chunk order zero and progress 100. Invalid metadata is rejected before reserving
an intent. Buffered bootstrap payloads have a local size limit; ordinary small
payloads are sent inline.

For external history, first call
`prepare_external_bootstrap_history(history, media_key)`. Retain the returned
`PreparedBootstrapHistory` in protected host storage before calling
`bootstrap_with_uploaded_history`. Its `upload_descriptor()` contains the media
key and upload metadata. After restoration, reconstruct it with
`PreparedBootstrapHistory::from_uploaded(history, upload)`; this checks the
compressed payload's hash and length. Retry with the original descriptor, even
if a later CDN lookup would return another path. Dispatch never silently
re-uploads or rebinds a changed notification under an old message ID.

For larger exports, compose the existing `Client::upload_stream` encrypted-source
API with `send_uploaded_history_chunk`. Supply the actual `PrimaryHistoryChunk`
type, chunk order and progress from the host's export cursor. Retain each upload
descriptor for ambiguous retries. The journal checkpoints the chunk hash and
cursor; it does not manufacture full-account-history completion.

History and app-state peer messages receive durable message IDs and exact payload
bindings before sending. Recovery settles retained persistence futures before
network work and reuses those IDs. Older ambiguous records missing required IDs
fail explicitly instead of minting replacements. Stable IDs do not establish a
stronger remote delivery or deduplication guarantee than the protocol provides.

`HostedCompanion::bootstrap_dispatch_complete()` means both send paths returned
and their local progress was checkpointed. It does not mean the companion applied
them or received all account history. Preserve that distinction in events and UI.

`cancel_unsent_bootstrap` can release an intent only before a peer payload was
bound. For an interrupted or partly dispatched operation, `abandon_dispatch`
archives its IDs, hashes and honest progress while releasing management actions.
`resume_abandoned_dispatch` restores that exact operation before the original
bootstrap/history call is retried. Explicit revocation remains available; an
abandoned dispatch must not automatically restart with fresh IDs.

## Encryption contract

Primary bootstrap/history sends pass the proof-confirmed peer identity into the
shared send pipeline. Immediately before encryption, that pipeline checks both
the current Signal cache and session remote key while holding the per-address
session lock. An earlier preflight check cannot replace this condition: inbound
processing or another send may change the session between checks.

Identity reads must use the Signal cache with the same backend context. Newly
established identities can exist only in write-behind cache; forcing a disk flush
or reading the backend directly is not an equivalent identity check.

Keep profile choices tied to pinned evidence and use
[mobile_testing.md](mobile_testing.md) and [mobile_oracle.md](mobile_oracle.md)
for validation. Primitive compatibility and synthetic fixtures alone do not
establish acceptance by an official mobile service.

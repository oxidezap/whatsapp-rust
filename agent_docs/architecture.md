# Where a change belongs

`wacore` owns protocol, crypto, typed IQs and platform-neutral storage/runtime
traits. It builds for WASM and ESP32; do not introduce Tokio there. `waproto`
contains buffa-generated protobufs. `src/features` exposes the Tokio SDK.
Host tools under `tools/` stay outside published runtime dependencies.

Find the behavior in WhatsApp Web before changing protocol logic. Follow
[wa_web_reference.md](wa_web_reference.md), change the owning layer, and add a
regression at the boundary that can observe the behavior.

## Protocol types

Use `ProtocolNode::try_from_node_ref` as the parser and `IqSpec` to pair a
request with its response. Constructors borrow `&Jid`; SDK callers use
`client.execute(Spec::new(&jid))`. Node helpers in `wacore/src/iq/node.rs`
keep missing-field errors consistent. See `groups.rs` and `blocklist.rs` for
complete implementations. Child-bearing nodes need explicit implementations.
Streaming responses implement `IqStreamSpec`; their streaming and tree parsers
must agree. Extend the existing encode fast path instead of adding another.

`WireEnum` owns serde and the wire spelling. Do not derive serde separately.
Tagged enums generate `<Name>Tag`; dispatch on that type. Keep validation and
protocol limits in the request type. Public event payloads are non-exhaustive,
builder-constructed, and use `Option` for absent data. Read the stability policy
on `Event` in `wacore/src/types/events.rs` before extending it.

## Ownership and persistence

Read device state through `get_device_snapshot`. Publish mutations through
`DeviceCommand` and `PersistenceManager::process_command`, or scoped
`modify_device_async`; direct write-lock mutation bypasses the snapshot.

Signal encryption/decryption uses per-address locks. Incoming chat lanes
serialize processing, including offline commit ordering. Outgoing sends do not
use a chat lock. Advance and return Signal state before publishing ciphertext;
`persist_signal_state_pre_wire` must succeed when a lease gate is pending.
Never replace the batch-safe flush with a raw cache flush during offline drain.

Session/sender-key reservations are exclusive ceilings for one chain. Only
confirmed backend writes release the matching version's dirty state and gate.
Failed writes, tombstones, checked-out state and cancellation remain pending.
A clean reload in the same cache incarnation is exact; a new incarnation burns
the persisted lease. An old database restore is not detectable from its bytes.
Consumed prekeys are deleted only after their promoted session is durable.
See `wacore/src/store/signal_cache.rs`, `wacore/libsignal/src/protocol/state/session.rs`
and `wacore/libsignal/src/protocol/sender_keys.rs` for lock order, cancellation guards and recovery tests.
SQLite's default NORMAL mode establishes transaction/process-crash ordering;
FULL is configurable for stronger storage durability. Do not promise power-loss
survival under NORMAL.

Noise selection and certificate trust live in `src/handshake.rs` and
`wacore/noise`. Transient errors must not invalidate trusted certificates;
crypto-fatal IK errors clear the cache. An explicit fixture bypass cannot mint
trusted cache provenance or authorize IK. Keep transport TLS configs scoped to
one intended trust/tenant policy; cloning an Arc-backed config shares its store.

## Optional subsystems

A subsystem can leave the core when entry is through an existing dispatch key,
its state is read only by itself, its core requirements already have independent
callers, and its removal does not change fields, builder setters or variants of
public types. `EventKind` indices are persisted interest bits; preserve them.
Keep subsystem state together under `src/client/subsystem.rs`, using weak
references where ownership would otherwise form a cycle. The executable guards
are `tests/subsystem_boundary.rs` and `wacore/tests/voip_control_boundary.rs`.

Native plugins use the existing registration and lifecycle hooks. WASM keeps
plugin dependencies disabled. Do not expand core state for a single plugin or
claim protocol parity from an application hook. SDK integration guides belong
in https://github.com/oxidezap/whatsapp-rust-docs.

## Public contract changes

The README support tables are the entry point for crate, target, delivery-origin
and capability scope. Keep them consistent with Rustdoc when changing a public
contract. Consumer recipes and migrations still belong in the docs repository.
A pre-1.0 policy is not an immutable release-candidate baseline; do not change
`tools/xtask/compatibility.json` as a side effect of documenting support.

Treat public modules, re-exports, generated code, macro output and dependency
types in signatures as source compatibility obligations. Do not describe a
feature-gated API as exempt from compatibility. Before changing visibility or
construction, check the isolated consumers and provide a migration. Generated
MEX models currently allow exhaustive construction; documentation does not make
them non-exhaustive. Change their construction policy only in a dedicated API
change with consumers and codegen updated together.

Runtime, transport, HTTP, storage, hooks and neutral media ports remain open
implementation points. Adding required trait methods or tightening thread bounds
breaks downstream implementers. Prefer optional methods with honest defaults
only when absence is safe. Do not silently default an atomic operation to
sequential writes, advertise unsupported streaming, or return success for an
unsupported durability capability. A future capability redesign must preserve
core ownership and be justified independently of any bridge's naming needs.

### Backend obligations and conformance

Backends implement the five domain traits composed by `Backend`. Keep account
and device scope on every read/write, including batch and detached operations.
`AppSyncStore::commit_patch` must publish version and MAC changes atomically,
including on cancellation. An error after a durability barrier can mean the
complete commit is visible; it does not establish rollback or safe blind retry.
`Ok` must reflect the backend's declared persistence boundary, not just enqueue.
An NVS commit is not evidence of a multi-key atomic transaction. Do not retain a
collection version while evicting MACs needed for its incremental baseline;
capacity refusal must preserve the complete old state or use explicit recovery.

Pending-inbound durability is optional and fails closed by default. Its bounded
reads must reject oversized payloads before copying them, preserve exact/group
alias semantics and share the required snapshot. Conditional mutation must
compare revisions atomically and retain the payload/ownership lease through
all detached workers and retries, including cancellation. Preserve conflicts;
do not overwrite a successor's row. Batch overrides inherit these obligations.
The builder probe rejects missing primitives but cannot certify arbitrary
backend atomicity, crash behavior or custom batching.

Reuse `wacore/tests/support/appstate_commit_contract.rs` and
`wacore/tests/support/tc_token_contract.rs` for the contracts they actually cover.
Their in-memory and SQLite runners show fixture setup and device-scope isolation.
They are source test helpers, not a published conformance crate or a complete
backend certification. Add cancellation, post-commit failure, bounded/CAS and
backend-specific crash tests at the relevant boundary before claiming those
properties for a custom adapter. Never substitute a passing compile probe for
these behavioral checks.

Transport factories must release a half-open dial when their future is dropped.
Keep event receivers bounded according to the host's policy and preserve the
closed `TransportEvent` contract; put extensible close detail in
`DisconnectReason`. An abort callback requests cancellation, not confirmed task
completion. Shutdown ownership and database close are separate boundaries.

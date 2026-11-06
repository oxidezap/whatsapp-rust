# Mobile API development

`mobile-core` is opt-in. It enables `wacore/mobile` and the SDK's instance clock
support. Keep protocol decisions in `wacore::mobile`, without Tokio, sockets,
HTTP clients, filesystem access, ambient entropy or ambient time. The separate
`mobile-transport-native` feature supplies a native TCP adapter. Browser and
embedded hosts supply their own adapters. Consult [support.md](support.md) for
the maintained target checks.

## Host configuration

Bind registration and login to one `MobileEnvironment`, with explicit
registration and connection endpoints and trust roots. The SDK requires a
transport factory; it must not select a Web transport or production endpoint
as a mobile fallback. Keep registration HTTP, media HTTP and connection
transport configuration separate.

Use a `ClockedRuntime` and the same clock instance in persistence. Mobile
connections reject a missing, negative or invalid host clock before sending
handshake bytes. Keep key material and registration checkpoints in protected
storage. Read cached device snapshots through `PersistenceManager` and publish
changes through its commands and mobile registration operations.

`RegisteredMobileSession` holds an explicit login profile, application version,
Noise prologue and host key provider. It does not fetch a Web version. Callers
must select `with_primary_identity_policy`; no policy is inferred from the
platform. Self-identity validation requires a device signature by default. An
explicit account-signature-only profile still verifies the trusted account
signature and rejects an invalid present device signature. Certificate expiry
policy also stays strict by default. The explicit legacy `IfPresent` profile
permits an absent expiry without changing the environment root, signature or
static-key checks; a present expiry must remain valid.

A fresh primary also needs host-generated `PrimaryAppStateKeyInput`
or an existing validated app-state key in storage before connecting.

App-state operations additionally require an explicit immutable
`with_app_state_policy(MobileAppStatePolicy::...)` selection; login alone does
not select it. `SnapshotCompatible` preserves the shared snapshot/bootstrap
exchange seen in pinned Cobalt and whalibmob. `AuthoritativeZapo` selects Zapo's
namespace `3`, incremental-only sync and mutation upload from a missing local
collection. Its outgoing patch sets `deviceIndex` to zero after MAC generation.
The shared post-send resync and durability behavior remain unchanged; this profile
does not introduce new server-acknowledgement or local-commit semantics. It ignores server snapshots and rejects destructive snapshot rebuilds
without clearing the primary's local baseline. Neither choice is inferred from
Android/iOS or advertised as APK-qualified. A primary never asks device zero to
recover its own snapshot through a companion-only peer request.

The authoritative profile comes from Zapo
`bceb268c846763a2cd0b494da830a54aacdc3818`,
`src/appstate/sync/WaAppStateSyncClient.ts` (`buildCollectionSyncRequest`,
`buildSyncIqNode`, `processCollectionRound`). The alternate profile reflects
Cobalt `cb9fbf172507626948fbc3611d0a6fb00bb3a0f1`,
`sync/exchange/MutationRequestBuilder.java`, and whalibmob
`8f50db4c586534dc0af4d13b26bdd09ef42660c0`, `lib/Client.js`.
The unchanged companion guard is also supported by official Web
`WAWebSyncdRequestBuilderBuild`, inspected through an AST in the verified
WhatsApp `2.3000.1052559453` bundle `4ab82076eb3225f2b47d11a8321e3b821057b8c2fff2944bb3262436ebb15da4`
(bytes `952672..961443`) from whatspec `ae3880925bacd23863cc30ec480c9f82bfd869f1`.
The archive SHA-256 is `2958d99a3d84141fa8479f1f991e06246e68c063ca8f618a8d5d361287387f62`;
all 676 locked bundles were verified. This Web evidence does not qualify a
mobile client or server. Tests use synthetic IQs and preserve the existing
companion behavior.

Media operations require an explicit `MobileMediaHttpProfile`, selected with
`with_media_http_profile`. Supply its User-Agent, routes and any intended
Origin. Login itself does not require a media profile. Do not inherit Web
headers or manufacture a current mobile version.

## Registration and imported accounts

For fresh registration, use `MobileRegistrationClient` with stable
`RegistrationMaterial`. Preserve its platform fields and installation IDs
across attempts and restoration. Only `request_code` requests an OTP;
`await_external_code` accepts a code requested elsewhere without making an HTTP
request. Transport failures, retry hints and account checks must never trigger
an implicit code request.

After durable registration, obtain `registered_account()` and hand its canonical
phone number, environment and key binding to the session factory:

```rust,ignore
let account = registration.registered_account()?;
let session = RegisteredMobileSession::from_registered_account(
    &account,
    login_profile,
    login_options,
    prologue,
    ephemeral_keys,
    phone_id_mapping,
)?
.with_primary_identity_policy(identity_policy);
```

The host still supplies login-only metadata. The factory validates shared
platform, version, OS and locale values, plus Android manufacturer when present.
Registration HTTP model IDs can differ from login marketing names, so map model
names explicitly. iOS registration does not carry a manufacturer. Select either
FDID-uppercase or an independent host phone ID with
`RegistrationLoginPhoneIdMapping`; do not silently equate unrelated IDs. Retain
independent phone IDs and other login-only metadata across restoration.

For existing credentials, use the typed key import and persistence APIs, then
construct `RegisteredMobileSession::new` with the matching profile and binding.
A mobile-marked device cannot load through a Web client, including builds with
mobile support disabled. Keep Web and mobile storage namespaces distinct.

## Attempts and cancellation

`MobileEphemeralKeyProvider` supplies a fresh cryptographic key for every XX
attempt. `MobileLoginOptionsProvider` may supply per-attempt session IDs and
other profile fields using host-owned entropy. It receives the validated device
snapshot and a counter reserved per Client immediately before XX. Failed
handshakes count; failed pre-dial attempts do not. The provider returns complete
options, with explicit field presence. Clone configured options and use
`with_session_id`, `with_connect_attempt_count` and `with_login_counter` to replace
only chosen attempt fields; passing `None` explicitly omits that field. The static default inserts no session
ID, login counter or reconnect counter. `encode_payload` inspects static
options; connections use `encode_attempt_payload`.

Registration cancellation or timeout can leave delivery or persistence
uncertain. Check `recovery_required()` and use `resume_pending()` to settle the
retained exchange or store operation. Do not issue a replacement request while
that outcome is unknown. A process restart cannot recover a lost HTTP response
and must not resend it automatically. Expose a registered account only after
its identity and workflow state are durable.

## Companion hosting

Use [mobile_primary.md](mobile_primary.md) for QR/code approval, proof confirmation,
index publication, bootstrap/history dispatch and durable recovery contracts.

## Checks

Run focused tests without real accounts or network services:

```sh
cargo test -p wacore --no-default-features --features mobile --lib mobile::
cargo test -p whatsapp-rust --features mobile-core --lib mobile_registration::tests
cargo test -p whatsapp-rust --features mobile-core --lib mobile_session::tests
cargo test -p whatsapp-rust --features mobile-core --lib handshake::mobile_transport_tests
cargo test -p whatsapp-rust --lib mobile_persisted_role_cannot_load_through_web_builder
```

Use [mobile_testing.md](mobile_testing.md) for host, WASM and mock server
contracts, and [mobile_oracle.md](mobile_oracle.md) for pinned APK tooling.
Keep those host tools out of runtime dependencies.

The SDK `mobile-core` feature enables SQLite mobile provisioning when that backend
is selected. Direct SQLite consumers enable its `mobile` feature to initialize or
import mobile namespaces. Loading and saving existing mobile role bindings stays
available without this feature, so feature-off builds cannot treat mobile state
as an unbound Web account.

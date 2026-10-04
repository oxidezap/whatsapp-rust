# Picture API consolidation

Base: `d9f78b806f1f4ca80c8008caa5846e5d542c2c55`.
Scope: deliberate pre-1.0 Rust API removals, not a wire/protocol or storage migration.
The delivered head and current-head CI results are recorded in the PR.

## Consumer contract / migration for the consolidated guide

Use `client.contacts().lookup_picture(ProfilePictureRequest::new(target, size))`.
All types remain exported through `whatsapp_rust` and `whatsapp_rust::features`.

| Removed entry | Replacement |
| --- | --- |
| Contacts `lookup_profile_picture(jid, preview, existing_id)` | Contact target, explicit size, `.existing_id(existing_id)` |
| Contacts `lookup_profile_picture_with_options(options)` / `ProfilePictureLookupOptions` | Request with `.existing_id`, `.common_gid`, `.invite`, `.persona_id`, `.timeout` |
| Contacts `get_profile_picture(jid, preview)` | Request, then **explicit** `.into_found()` only if non-found states may be discarded |
| Contacts `get_profile_picture_with_timeout(jid, preview, timeout)` | Same, with `.timeout(timeout)` |
| Groups `lookup_profile_picture` | Group target on Contacts' canonical facade |
| Groups `lookup_community_profile_picture` | Community target on Contacts' canonical facade |
| `ProfilePictureLookup::RateOverlimit` / `is_rate_overlimit()` | Inspect the error's `server_rejection()` (429, text, type, optional backoff) |
| Core `parse_response_preserving_rate_limit()` | Ordinary `IqSpec::parse_response()` now preserves the same rejection |

`preview=true` maps to `ProfilePictureType::Preview`; `false` maps to `Full`.
Group wrapper errors now come from `ContactError`, not `GroupError`.
Import `whatsapp_rust::ErrorChainExt` to call `server_rejection()` on the error;
it is a trait method, not an inherent method.

```rust,ignore
let outcome = client.contacts().lookup_picture(
    ProfilePictureRequest::new(ProfilePictureTarget::Group(&jid), ProfilePictureType::Full)
        .existing_id(None) // Do not condition on an ID when image bytes are missing.
        .timeout(Some(Duration::from_secs(3))),
).await?;
let picture = outcome.into_found(); // Intentional consumer policy, not the main contract.
```

Four distinct successful outcomes remain: Found, Unchanged, NotFound,
NotAuthorized. 401/403 map to NotAuthorized; 404 maps to NotFound; 429 is a
rejection error, both at IQ-envelope level and embedded in a picture. Embedded
429 on direct core execution now retains text/type/backoff too; runtime execution
attaches the original response Arc. Ordinary parse errors and unrelated raw or
streaming IQ policies are unchanged. No empty rate-limit state remains.

Contact and Group use `w:profile:picture`; Community uses `w:g2`/`pictures`.
Both Preview and Full remain available on each route. No automatic community
fallback, metadata lookup, image download/decode, or invented cached bytes is
introduced. Conditional no-data/304 is Unchanged, not evidence of local bytes.
Privacy-token discovery, token-first/common-group fallback, special/own/PSA JID
handling, timeout, cancellation and response-waiter cleanup retain their policies.

`Groups::get_profile_pictures(Vec<Jid>, PictureType)` remains a one-IQ batch
with its existing 1,000-entry limit and per-entry outcomes/cost. Group/Profile
setters, empty-image rejection and dedicated removal operations are unchanged.
Advanced `ProfilePictureSpec` and batch IQ construction remain public.
No image/decode dependency, root lockfile, optimizer, feature default, size budget,
CI assertion or deadline is changed.

## Evidence before production changes

The assessment's static evidence was used; the specialized protocol skill is
not installed. No captured JS was executed. Queried pinned whatspec IR at
`1a441f0329c941fcdb238490a6c604550d8a9939` and verified exact lock hashes:

- `generated/iq/index.json`: `209d5d5468892bda7671e7fae77b581a9890cbd02f4d47c0de83dc7f0af7c81c`
- `generated/manifest.json`: `fa39dfe0f08d026c8b049e8c75f9515c743ac9190e69bf502d2dcfc84e38419b`

`WASmaxOutProfilePictureGetRequest`/`WASmaxProfilePictureGetRPC` preserve the
Preview/image wire enum, request attributes, success-no-data variant and 429
error union. `WASmaxOutGroupsGetGroupProfilePicturesRequest` preserves the
separate batch route, parent/sub-group attributes and partial-entry outcomes.
The assessment supplies static bundle offsets for the no-data and rate-limit
mixins when the IR is incomplete. This is client-source evidence, not an
authenticated-server acceptance or delivery claim. Avatar/blob handling is not
expanded by this API cleanup.

Repository callers were exclusively the picture tests, metrics controls, public
compile contract and profile-picture/privacy-token E2E fixtures; no production
caller or example required a lossy facade. All were migrated.

## Validation and failure history

Full local operational logs/exits and frozen base/head artifacts are retained
under `.task/pictures/` in the task worktree (not bundled into source).

- Base directed suite: 9 passed, including genuine routed assertions for the
  old getter/lookup differences; exit 0, 2.86 GiB peak RSS.
- New direct-spec nested-429 metadata regression on the **old implementation**:
  genuine exit 101, `unwrap_err()` saw `Ok(RateOverlimit)`. Its exact source patch
  and failure log are retained as `legacy-negative.patch` / `legacy-negative.log`.
  The regression stays in the final suite; no legacy implementation is retained.
- Initial base WASM external-consumer build failed (exit 101): the fixture
  incorrectly required Send futures on WASM. Corrected only that host cfg to
  `async_trait(?Send)` / non-Send boxing on WASM, retaining Send on native.
  This real failed attempt remains in `consumer-base-wasm.log`.
- Corrected, identical external consumer compiled/linked against the old
  implementation on native and WASM release profiles. Native `size`:
  text 1,501,283; data 11,976; bss 4,226 bytes. WASM artifact: 41,824 bytes.
  These are bounded consumer profiles, not full-client binary-size claims.
  Both keep the same fixture lock, default release optimizer and no-default
  library features; WASM uses `--cfg getrandom_backend="wasm_js"`.
- Initial head library compilation exited 101 because a new wire assertion
  compared `Option<Cow<str>>` with `Option<&str>`; corrected the assertion's
  representation, not its expected value. Removed a now-unused picture reexport
  from the private Groups module. The original `head-lib.log` remains intact.
- Pre-existing WASM unused-qualification warnings in `request.rs`/`upload.rs`
  are preserved in baseline logs rather than presented as new picture findings.

- Corrected head full library suites: whatsapp-rust 2,422 passed / 3 ignored;
  wacore 1,733 passed / 1 ignored, both exit 0. Routed regressions and unchanged
  setter/removal controls passed without assertion or timeout weakening.

- Initial head metrics picture suite: 12 passed, including real raw/ordinary
  parser/domain-error/streaming/cancel controls and the changed direct-spec
  nested-429 failure counter. Public external-crate test: 2 passed.
  Doctests: wacore 7 passed / 11 ignored; whatsapp-rust 30 passed / 16 ignored.
- Configured Cubic/Greptile reviews were read. Added the required external
  ErrorChainExt import, a precise core/runtime 429 wire doc, matrix diagnostics,
  and fixture builds in the existing MSRV/WASM CI jobs (no new pipeline/job).

Final external consumer comparisons, Clippy, E2E compilation and current-head
CI are reported in the PR.
The repository's native Binary Size hard gate remains authoritative for the
full default-feature demo. No RAM/binary savings are inferred from API names.

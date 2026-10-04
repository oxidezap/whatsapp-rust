# Community API contract (proposed, not merged)

Ownership: `src/features/community.rs`, targeted reexports, community E2E call sites and `tests/fixtures/community_consumer`. No wire/registry/workflow/shared-lock edits.

## Construction and group-lane coordination

`CreateCommunityOptions` becomes `#[non_exhaustive]`; `new(name: impl Into<String>) -> Result<Self, CommunityError>` validates the existing `GroupSubject` length contract. Public `name: GroupSubject`, `description: Option<GroupDescription>` and booleans remain readable/mutable. `with_description(GroupDescription)` sets the optional validated description. Defaults remain open, admin-only subgroup creation, and general chat enabled. No `Default` for an input with a required name.

Community group inputs use `GroupCreateOptions::new(subject)` followed by public field assignments, never external literals or struct updates. This works with the group lane's planned non_exhaustive inputs and does not require a changed group constructor. `GroupParticipantOptions::new(Jid)` remains the participant constructor. No changes to wire or subgroup default shortcut.

## Partial creation: one error contract

`CommunityError::ConfigurationFailed { created_jid: Jid, step: CommunityConfigurationStep, source: Box<GroupError> }`. `CommunityConfigurationStep::SetDescription` is non_exhaustive. Failure before creation retains existing errors. No parallel partial-success result family. The source stays typed and participates in std::error::Error chaining.

Creation validates even public unchecked newtypes before any remote effect. If description configuration fails, resume on `created_jid` using `Groups::set_description(..., PreviousDescription::Resolve)`; re-querying the current description token avoids assuming a failed/ambiguous IQ never committed. No rollback, atomicity, automatic retry or creation-retry guarantees. Cancellation after creation can still lose its result; this is not a durable workflow.

## R07

Both batch results use `failed_groups: Vec<SubgroupFailure>`, with non_exhaustive `SubgroupFailure { jid: Jid, code: u32 }` and `SubgroupFailure::new(jid, code)` for mocks/hosts. All successes and failures, including duplicate JIDs, zero and unknown numeric codes, are preserved in their original per-list order. No invented cause or batch retry.

Serialization decision: existing facade results and tuples do not derive Serialize/Deserialize, so there is no library-owned JSON format to migrate or preserve. We do not add serde. Consumer-owned tuple serialization may change from arrays to named objects when the host adopts the DTO; hosts that require the old shape explicitly map `(failure.jid, failure.code)` before serializing. Wire parsing is unchanged.

## Migration

Explicit, caller-selected resumption (not automatic library retry):

```rust,ignore
use whatsapp_rust::{Client, CommunityConfigurationStep, CommunityError,
    CreateCommunityOptions, GroupDescription, Jid, PreviousDescription};

async fn create_or_resume(client: &Client) -> Result<Jid, Box<dyn std::error::Error>> {
    let mut options = CreateCommunityOptions::new("Fictitious community")?
        .with_description(GroupDescription::new("Description")?);
    options.closed = true;
    match client.community().create(options).await {
        Ok(created) => Ok(created.metadata.id),
        Err(CommunityError::ConfigurationFailed {
            created_jid, step: CommunityConfigurationStep::SetDescription, source,
        }) => {
            eprintln!("Initial description configuration failed: {source}");
            client.groups().set_description(
                &created_jid,
                Some(GroupDescription::new("Description")?),
                PreviousDescription::Resolve,
            ).await?;
            Ok(created_jid)
        }
        // Includes other creation errors and future configuration steps.
        Err(error) => Err(error.into()),
    }
}
```

In the separate link/unlink batch flow:

```rust,ignore
// Before: for (jid, code) in batch_result.failed_groups
for failure in batch_result.failed_groups {
    eprintln!("{} failed with code {}", failure.jid, failure.code);
}
```

## Fixtures and modes

Standalone manifest: `tests/fixtures/community_consumer/Cargo.toml`, native minimal runtime dependencies (root default-features=false), native test and doctest, compile-fail literal/exhaustive pattern controls, MSRV 1.94 check. WASM construction check with `RUSTFLAGS='--cfg getrandom_backend="wasm_js"'`; no new feature gates. CI/registry ownership belongs to the R05 consumer-gate lane, which should register native test (including doctests), MSRV check and WASM check for this manifest. In-process IQ fault injection tests in `features::community::tests` verify zero IQ on invalid 2049-character ASCII description, complete creation, preserved JID/step/source on second-IQ failure, and configuration resumption without a second create.

## Evidence

Current default verified via fetch: `e32ec562a207c95188693ec1a2738ea69c08fa48`. Description suppression exists both there and in historical `d9f78b806f1f4ca80c8008caa5846e5d542c2c55`; not a regression attributed to the API merges. Initial red-test build exceeded the foreground harness timeout before execution; no claim of baseline runtime reproduction yet. Final validation results will be appended here.

Pinned whatspec IQ IR queried: `WASmaxOutGroupsCreateRequest` includes both optional `descriptionArgs` and `parentArgs`. Shape alone does not establish community creation sequencing. Local raw captures are not available in this worktree, so we do not assert that inline descriptions are server-forbidden for communities. We deliberately preserve the existing two-IQ behavior, as this task prohibits changing the wire, and document that decision where the group input is built.

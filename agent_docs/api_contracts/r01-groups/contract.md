# Evolution-safe group DTOs

The initial breaking change deliberately prevents external struct literals and
exhaustive patterns for `GroupMetadata`, `GroupParticipant`,
`GroupCreateOptions`, and `GroupParticipantOptions`. Their fields remain public
for reading and customization. This protection permits future field additions;
it does not make field removal, type changes or semantic changes compatible.

## Inputs

Existing bon builders and generic `.build::<T>()` conversions are preserved.
Subject and participant identity remain explicit:

```rust
use whatsapp_rust::{GroupCreateOptions, GroupParticipantOptions, Jid};

let participant = GroupParticipantOptions::new(Jid::pn("15550000001"));
let mut options = GroupCreateOptions::new("Fixture group")
    .with_participant(participant);
options.is_parent = true;
options.closed = true;

let options: GroupCreateOptions = GroupCreateOptions::builder()
    .subject("Fixture group")
    .is_parent(true)
    .closed(true)
    .build();
```

Replace `GroupCreateOptions { subject, ..Default::default() }` with
`GroupCreateOptions::new(subject)` and then set any additional public fields,
or use the builder. `Default` is removed: an identity-free empty subject does
not express neutral group creation intent. The constructor and builder preserve
the same pre-existing wire-setting defaults (admin invite links, all-member add,
no membership approval, disappearing messages off). Explicit `None` values
remain possible through public field mutation for advanced wire callers.

Participant options continue to support `new`, `from_phone`, the existing
fluent methods, and the builder. No new validation or creation IQ is introduced.

## Outputs and mocks

Replace full literals or `GroupMetadata::default()` with an identity-explicit
minimal projection, then populate the public fields needed by the test/host:

```rust
use whatsapp_rust::{GroupMetadata, GroupParticipant, Jid};
use whatsapp_rust::wacore::iq::groups::ParticipantType;

let mut metadata = GroupMetadata::new("120363000000000001@g.us".parse().unwrap());
metadata.subject = Some("Fixture group".into());
let mut participant = GroupParticipant::new(Jid::pn("15550000001"));
participant.participant_type = ParticipantType::Admin;
metadata.participants.push(participant);

let GroupMetadata { id, participants, .. } = metadata;
let GroupParticipant { jid, participant_type, .. } = &participants[0];
```

`GroupMetadata::new(id)` leaves optional observations absent, the participants
empty, and raw flags false. It does not assert completeness, fetch from the
server, or introduce a zero participant-count sentinel. `GroupParticipant::new`
starts at `Member`, without auxiliary identity or details. Construction does
not join/add a member or validate an application-supplied JID.

Always include `..` when destructuring these extensible DTOs. Normalized
hierarchy still derives from the retained wire flags; changing public flags
updates that projection. Overview, full metadata and routing queries remain
distinct projections. Already-extensible `CreateGroupResult` and
`GroupLookupResult` are unchanged, as are host-implemented traits.

## Evidence

`tests/fixtures/groups_consumer` independently tests inputs, mock outputs,
extensible patterns and negative old-construction controls. Its README lists
native/full, core-only, MSRV and WASM modes. Existing lookup and hierarchy
consumers were narrowly migrated. This contract concerns construction and
source evolution, not a WhatsApp protocol change or live-server confirmation.

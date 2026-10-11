# Generated API maintenance

`build.rs` applies this policy to freshly generated syntax. Do not edit the
emitted Rust files. `emission.rs` protects messages, views and oneofs, hides
unknown-field storage from derived serde, and inventories published names,
field types, methods, enum values, wire tags and serialization attributes.
The inventory maps buffa's internal namespaces to their public re-exports and
excludes its hidden storage fields.

`api.snapshot` is the reviewed API inventory, not another message model.
Builds reject removal or alteration of a frozen entry. The integration test
also requires new entries to be recorded, so additive APIs receive the same
protection before publishing. After reviewing additions, copy `api.snapshot`
from the build's `OUT_DIR` to `waproto/api.snapshot`. The test prints its exact
path when the snapshot needs updating. Never replace the baseline merely to
silence a removed name, changed ownership, changed type, or changed wire tag.
A buffa or buffa-build update changes a public dependency and must pass this
guard and the external-consumer tests.

For an upstream rename with unchanged wire identity and semantics, add a
reviewed rule to `names.rs`. Rules preserve declaration/field/oneof/enum names
without changing wire numbers. Descriptor JSON names remain independent of
Rust names and the derived-serde bridge contract. Missing targets and name
collisions fail the build. Retained upstream messages belong in the proto
emitter's `LOCAL_BLOCKS`; persisted local fields belong in `LOCAL_FIELDS` in
`build.rs`, never in generated proto or descriptor files.

`evolution.rs` emits small synthetic schemas through the resolved production
generator. `tests/generated_api.rs` compiles the same consumer in a separate
crate against both versions, including additive fields and variants and
reviewed renames. It also verifies that literals and exhaustive matches are
rejected externally. `tests/wire_extensibility.rs` and the serde profile tests
cover wire retention, local persistence extensions and bridge serialization.

# Generated source compatibility

The emitter marks public generated messages, views, enums and oneofs non-exhaustive. Construct owned messages through Default and field assignments or setters. Match enums with a fallback. This protects source compatibility, not ABI or layout.

The API snapshot records names, types, visibility, wire identities and conditional availability. Additions are permitted; removals or incompatible changes fail. Reviewed rename rules live in names.rs and require unchanged wire identity and semantics.

Signal persistence uses an additional exhaustive schema snapshot because record projections must be updated and tested when stored fields change.

When run in the repository workspace, the generated_api tests also compare the group-history text whitelist with the current emitted Message and MessageContextInfo fields. Update and review that borrowed whitelist when those schemas grow; the compatibility baseline alone permits additions.

This policy adds no runtime wire-occurrence journal, edit baseline or universal unknown-field ordering guarantee. Durable original payload bytes belong to the receive/replay contract.

The workspace-only history check reports that it is unavailable when testing the standalone waproto package; its SDK input is not part of that package. Inherent method and constant bounds are recorded per member so moving a member into a more restrictive impl cannot pass as an additive change.

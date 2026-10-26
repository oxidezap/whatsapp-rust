# Reviewed IQ bundle oracle

`accept_group_add.json` records a bounded, payload-only conformance oracle for
AcceptGroupAdd. Its expectations were derived by reading complete preserved Web
modules, separately from the Rust pilot emitter. It contains no captured account
or session input. The generated test uses synthetic nodes.

The source is whatspec commit `c581c20a9d0605df49abf297f2e7cfc04ccf1ff7`,
schema 4.4.0, WhatsApp Web 2.3000.1047483476. The JSON records the exact release
asset, archive SHA-256, set hash and module hashes with byte intervals. The archive
contains 579 bundles. The archive hash and set hash describe different objects.

The original bundle definitions were extracted with Acorn as complete `__d`
CallExpression nodes. No bundle code was evaluated. All 579 bundle sizes and
SHA-256 values were checked against that commit's `generated/bundles.lock.json`.
There was one definition of each of the four selected modules in that set.

The independent reasoning behind the cases is:

- `WASmaxParseUtils.flattenedChildWithTag`, function `v`, calls `optionalChild`,
  function `y`. It rejects absent or duplicate matching children. An unrelated
  sibling does not count toward that cardinality. `maybeChildren`, function `h`,
  rejects binary content for this child parser.
- `WASmaxInGroupsAcceptGroupAddResponseGroupJoinRequestSuccess` first requires
  exactly one `membership_approval_request`, then parses the result envelope.
- `WASmaxInGroupsAcceptGroupAddResponseSuccess` checks the result envelope and
  does not inspect children or content.
- `WASmaxGroupsAcceptGroupAddRPC` tries the approval variant before bare success,
  then error variants. Thus missing, duplicate, unknown or binary content falls
  back to bare success when the result envelope is valid. One matching child
  wins even with an unrelated sibling.

These are static source conclusions. They do not prove server behavior, Android
or iOS behavior, delivery, or authenticated execution. Envelope ID/from/type and
error handling remain in the request layer. The fixture deliberately does not
invent request context from the reply.

Regeneration validates the complete ordered success contracts against the reviewed
oracle, then the pilot emitter's request, envelope and delegated error guards.
A new version, pin, outcome, payload, child guard or precedence requires review;
`--update-lock` does not silently rebase the oracle. Change expectations only
after restoring and reviewing the new exact bundle set.

With bundles restored as `<sha256>.js`, verify source hashes and regenerate every
artifact in one pass:

```text
cargo run -p whatspec-codegen -- --from /path/to/whatspec/generated --verify-iq-bundles /path/to/bundles
cargo run -p whatspec-codegen -- --check --from /path/to/whatspec/generated --verify-iq-bundles /path/to/bundles
cargo test -p whatspec-codegen
cargo test -p wacore --lib accept_group_add_matches_reviewed_bundle_cases
```

`--verify-iq-bundles` verifies the selected source bundles and exact reviewed
module spans. It does not claim to validate an entire archive or extract modules.
Normal `--check` remains offline-capable against supplied pinned IR and detects
fixture drift byte for byte without requiring the original JavaScript download.
The generated fixture has no runtime dependency or public API.

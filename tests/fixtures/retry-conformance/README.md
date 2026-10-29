# Pinned retry oracle

Producer: [whatspec #60](https://github.com/oxidezap/whatspec/pull/60), commit
`3e5db6626f2863338064310ccdb108875c07dbe3`.
The fixture bytes are copied unchanged from
`tests/retry/session-cases.json` with SHA-256
`dc9412eef9d9241262395948b80745c320922ce6b98a61405748dc4db0a6c0b6`.
Both test targets check that digest before consuming it. The wacore copy keeps
integration-test packaging independent of the parent crate's files.

The producer restores and verifies WhatsApp Web 2.3000.1047483476, captures
four modules by checked AST spans and exercises the isolated session generator
plus the receipt key gate with synthetic dependencies. Reproduce that producer
oracle before refreshing the fixture. Review changed cases and source hashes;
do not regenerate expectations from core implementation output.

Core checks five session state outcomes through the actual private orchestrator,
14 receipt key-gate cases, and cancellation while waiting for its session lock.
Additional tests inject failure before and after committed deletion, and cancel
the real repair after the delete batch commits but before its acknowledgement.
The cache must retain the tombstone and allow an idempotent flush to settle it.
These qualify the manual core persistence extension, not Web storage semantics.
They do not claim full call-order or arbitrary backend-failure equivalence.
The six other Web scenarios
are explicitly excluded from this private-method test because they belong to
sender-key, bundle-install, peer-identity or missing-session boundaries.

Retained exceptions are deliberate: message lookup gates destructive repair;
verified bundles may admit an unknown device; hosted addressing may force key
inclusion; after deleting for registration mismatch core has no session to stamp
at retry two; core clears a consumed base-key stamp. Existing handwritten tests
cover authorization, route selection and hosted ADV validation.
The producer also qualifies 12 identity/eligibility decisions. Core does not
model Web identity-row/delivery eligibility providers, so those cases remain
producer evidence rather than an invented equivalent consumer policy.
No runtime policy, crypto, storage, locks, transport or global whatspec codegen
pin is replaced.

The producer PR supplies test provenance. Merge it before this consumer PR, or
retain the exact producer commit reachable while this fixture is in use. There
is no build-time or runtime dependency on the producer branch.

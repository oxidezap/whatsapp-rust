# Pinned retry oracle

Producer: [whatspec #60](https://github.com/oxidezap/whatspec/pull/60), commit
`42c8789fe9d3be32fcad4857ee1c7f673f068485`.
The fixture bytes are copied unchanged from
`tests/retry/session-cases.json` with SHA-256
`aadc079a167af576f3ee7649d86cd91e4ac83e6029951c5f23ca5ab54205c9b9`.
Both test targets check that digest before consuming it. The wacore copy keeps
integration-test packaging independent of the parent crate's files.

The producer restores and verifies WhatsApp Web 2.3000.1047483476, captures
three modules by checked AST spans and exercises the isolated session generator
plus the receipt key gate with synthetic dependencies. Reproduce that producer
oracle before refreshing the fixture. Review changed cases and source hashes;
do not regenerate expectations from core implementation output.

Core checks five session state outcomes through the actual private orchestrator,
14 receipt key-gate cases, and cancellation while waiting for its session lock.
It does not claim call-order equivalence, backend-failure equivalence, or
cancellation safety after a destructive operation. The six other Web scenarios
are explicitly excluded from this private-method test because they belong to
sender-key, bundle-install, peer-identity or missing-session boundaries.

Retained exceptions are deliberate: message lookup gates destructive repair;
verified bundles may admit an unknown device; hosted addressing may force key
inclusion; after deleting for registration mismatch core has no session to stamp
at retry two; core clears a consumed base-key stamp. Existing handwritten tests
cover authorization, route selection and hosted ADV validation. No runtime policy,
crypto, storage, locks, transport or global whatspec codegen pin is replaced.

The producer PR supplies test provenance. Merge it before this consumer PR, or
retain the exact producer commit reachable while this fixture is in use. There
is no build-time or runtime dependency on the producer branch.

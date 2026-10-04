# Checking behavior against WhatsApp Web

Start at https://github.com/oxidezap/whatspec. Query its generated IR, then read
the exact original bundle when the question involves guards, normalization,
ordering or effects. Baileys and whatsmeow are comparisons, not the source of
truth. Web evidence does not establish Android/iOS or server behavior.

1. Select a whatspec commit. Read `generated/manifest.json`, diagnostics and
   `generated/bundles.lock.json` at that commit. Record schema version, WA
   version, set hash and bundle count. The project's codegen pin is in
   `tools/whatspec-codegen`; an IR snapshot is not automatically today's service.
2. Restore the exact `bundle-store` release asset named by that lock. Verify the
   archive and every bundle's hash, size and completeness. A clone of whatspec
   contains the IR, not necessarily the original bytes. Reuse only verified
   captures for the same lock; do not combine versions or select releases/latest.
3. Read the domain schema before interpreting fields. Use `rg -n` in the IR and
   `rg -l -F` in original bundles to locate symbols without printing minified
   lines. Extract complete `__d` modules with an AST parser, preserving original
   bytes, offsets and duplicate definitions. Never use eval or a brace regex.
4. Follow the dispatcher, parser, validation, identity mapping, flags, state,
   error/fallback paths and caller through actual send or persistence. Building
   an ACK is not proof that the caller transmits it. Distinguish scheduled work,
   resolved promises and committed state.
5. Compare the same input/conditions in Rust. Derive fictitious minimal fixtures
   for each branch that changes the outcome. Record commit, asset/hash,
   module/function and offset beside vectors or in the PR. Separate static
   evidence, isolated tests and authenticated execution.

Use `iq` for IQ requests/responses, `notif`/`incoming`/`srvreq` for dispatch,
`stanza`/`tokens` for outgoing nodes, `mex` for persisted GraphQL variables,
`appstate` for sync actions, `proto` for message fields, `abprops`/`enums` for
flags/values, and `wam` for telemetry. Discover the actual manifest's domains;
this list is guidance, not a fixed snapshot contract. WASM paths also require
the corresponding glue and capture lock.

The IR is a derived static model. Missing entries do not prove absent behavior;
`parserRequired` can be guarded; consumers are references rather than execution.
Check wireName/sourcePath, defaults, unknown-value arms and transformations.
Do not combine code/text from different error variants or infer wire shapes
from API arguments. Feature-flag defaults do not establish account configuration.

If original bytes are unavailable, continue with accessible evidence and state
the gap. Never claim bundle validation or runtime confirmation without doing it.
Regenerate vendored files through the workflow in [codegen.md](codegen.md).

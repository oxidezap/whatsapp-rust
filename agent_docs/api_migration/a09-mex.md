# A09: consistent MEX operations and canonical execution

## Before

`MexRequest::new(name, id, keys, variables)` let callers accidentally combine
metadata from unrelated operations. Public docs suggested `mex_request!`, but
that macro was crate-private. `query` and `mutate` executed identically.

## After

```rust
use whatsapp_rust::{mex_operation, wacore::iq::mex_operations::join_newsletter};

let operation = mex_operation!(join_newsletter);
let request = operation.request(join_newsletter::Variables {
    newsletter_id: Some("123456789@newsletter".into()),
});
// let response = client.mex().execute(request).await?;
```

`whatsapp_rust::mex_operation!` binds the same module's `NAME`, `DOC_ID`,
`VARIABLE_KEYS` and `Variables` into `MexOperation<V>`. `request` accepts only
that variables type. Descriptors are copyable without requiring `V: Copy`, have
private immutable metadata, and own no payload. Their public constructor
`from_raw_parts` is explicitly unchecked advanced access, not a verified binding.

`MexOperation`, `MexRequest`, `Mex`, and `MexError` are imported from
`whatsapp_rust` or `whatsapp_rust::features`. `MexDoc` is now re-exported from
both paths and remains available from `whatsapp_rust::wacore::iq::mex`. The implementation
module `features::mex` remains private; the exported macro does not use it.

## Raw variables and custom documents

Generated inputs can be heuristic, particularly nested objects. Deliberately
bypass the generated variables type with:

```rust
use whatsapp_rust::{mex_operation, wacore::iq::mex_operations::join_newsletter};
let request = mex_operation!(join_newsletter).raw_request(serde_json::json!({
    "newsletter_id": "123456789@newsletter"
}));
```

`raw_request` preserves operation metadata but makes **no** promise that the
payload matches the generated mirror. It also accepts borrowed or custom
`Serialize` implementations; a JSON value is not required. For custom documents,
use `MexRequest::new_raw(doc, keys, variables)` or
`MexOperation::<YourVariables>::from_raw_parts(doc, keys)`.
`MexRequest::new` stays as a legacy unchecked constructor. The internal
`mex_request!` macro stays internal for existing domain callers.

## Compatibility and behavior

- `execute` is canonical. `query` and `mutate` remain non-deprecated compatibility
  aliases with no scheduled removal. Neither checks operation kind; `query`
  does **not** forbid side effects.
- **Breaking:** request metadata fields are private. Read `request.doc()` and
  `request.declared_variables()` instead of fields. Replace request literals or
  metadata mutation with the appropriate typed factory or explicit raw
  constructor. `variables` stays public and owns its previous `V` unchanged.
- **Error classification:** execution now returns `MexError::GraphQl { code,
  message, source }` for fatal GraphQL errors instead of source-less
  `ExtensionError`. Match the new variant with `..`; its source is the original
  `IqError::ParseError`, whose source retains `MexFatalError`. The legacy
  `ExtensionError` variant remains constructible. IQ and JSON sources are
  preserved; no global error enum is introduced.
- Optional variables may still be omitted. `missing_variables` is opt-in and
  diagnostic, not a requiredness check. Execution never calls it.
- Variables are serialized once directly to wire bytes in `MexQuerySpec`, without
  an intermediate `serde_json::Value`. The non-generic IQ executor is retained.
  Response parsing still returns `MexResponse`; no automatic typed output is
  inferred from heuristic generated `Response` mirrors.

No dependency, feature, generated output, snapshot, protocol shape, or domain
migration is needed. Metadata comes from the existing pinned generated modules;
this ergonomics change adds no requiredness or server-behavior assumptions.

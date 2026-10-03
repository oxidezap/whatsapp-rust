# MEX consolidation contract

Base: `d9f78b806f1f4ca80c8008caa5846e5d542c2c55`.
Branch: `fm/wr-api-consolidate-mex`.

Deliberate Rust API removals: `Mex::query`, `Mex::mutate`,
`MexRequest::new(name, id, keys, variables)`, `MexError::ExtensionError`.

Canonical path: `mex_operation!(module).request(module::Variables { ... })`
then `client.mex().execute(request).await`. Queries and mutations share it;
no operation-kind policy is introduced. Metadata stays immutable and the macro
binds metadata and Variables from one module. Response remains `MexResponse`;
no generated output deserialization is inferred, and declared keys are never
made mandatory. Existing domain decoding is untouched.

Advanced paths retained: descriptor `from_raw_parts`, `raw_request` and request
`new_raw`. The internal `mex_request!` remains crate-private construction sugar:
its typed form removes repeated module spelling, its raw form makes existing
corrected inputs readable; neither is an executor.

Repository audit before removal found no productive `ExtensionError`
construction: only its declaration and get_username match in src, plus test
constructions and documentation. get_username continues treating real GraphQL
404 and IQ 404 as absence; all other failures propagate with their source.
The canonical error remains GraphQl -> IqError::ParseError -> MexFatalError;
GraphQL extension codes do not become IQ server-rejection codes.

## Migration input for the final guide

Before:
```rust,ignore
let request = MexRequest::new(op::NAME, op::DOC_ID, op::VARIABLE_KEYS, variables);
let response = client.mex().query(request).await?; // or mutate
match error { MexError::ExtensionError { code, .. } => {}, _ => {} }
```
After:
```rust,ignore
let request = mex_operation!(op).request(variables);
let response = client.mex().execute(request).await?;
match error { MexError::GraphQl { code, .. } => {}, _ => {} }
```
Custom metadata uses `MexRequest::new_raw(MexDoc { name, id }, keys, variables)`
or `MexOperation::<CustomVariables>::from_raw_parts(doc, keys).request(variables)`.
Imperfect generated inputs use `mex_operation!(op).raw_request(variables)`.

No generated, wire, schema, persistence, export or workflow rewrites. No
protocol behavior changed. Existing pinned static whatspec evidence and the
assessment were used; no unavailable protocol skill, captured JS execution,
authenticated sends or real-server acceptance are claimed.

Validation evidence is recorded in report.md; raw operational logs/exits remain
in this directory in the task worktree (ignored, not silently removed).

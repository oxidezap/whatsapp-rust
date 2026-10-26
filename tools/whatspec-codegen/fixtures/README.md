# IQ conformance oracle

`accept_group_add.json` records expectations reviewed against complete Web bundle
modules, independently of the Rust emitter. It stores source hashes and byte spans;
the generated test uses synthetic nodes.

`WASmaxParseUtils.flattenedChildWithTag` requires exactly one matching child.
AcceptGroupAdd tries approval success before bare success, whose parser ignores
content. Thus one `membership_approval_request` wins despite unrelated siblings;
absent, duplicate, unknown or binary content falls back to bare success.

These payload-only cases assume a valid result envelope. Correlation and errors
belong to the request layer; this static oracle does not establish server behavior.

Pin, version or success-contract changes stop generation. Review the new exact
bundles before updating the oracle. With sources restored as `<sha256>.js`, check
source integrity and generated drift below; omit `--check` to regenerate all files.

```text
cargo run -p whatspec-codegen -- --check --from /path/to/whatspec/generated --verify-iq-bundles /path/to/bundles
```

Source verification checks the selected bundles and module spans, without executing
JavaScript. Without that flag, `--check` needs only the pinned IR.

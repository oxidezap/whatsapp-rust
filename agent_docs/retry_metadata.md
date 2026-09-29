# Content metadata on pairwise retry

## Reference identity and reproduction

Investigated Rust commit: `008a68a4fa823e5a3f5e96715d7673dedd259e35`
(the actual starting checkout, also the preliminary baseline).
Reference whatspec commit: `1f5167c4cebf263edaf001e06a2497f213c55793`.
WhatsApp Web: **2.3000.1047483476**, **579** JavaScript bundles.

- Lock setHash: `05609307e68b0f6ccfd2a121a573049999e38b752cfe7155ebbe5c661805751b`.
- Release asset: `bundles-2.3000.1047483476-05609307e68b0f6ccfd2a121a573049999e38b752cfe7155ebbe5c661805751b.tar.xz`, ID **564364719**, **9,055,920** bytes, release `bundle-store`, repository `oxidezap/whatspec`.
- Compressed archive SHA-256: `22243b9c2ff18a66cea8fd66551821fe0f9f09786d107e2be8284637db86c349` (not the setHash).

Both the downloaded archive checksum and the pinned `whatspec restore` verifier
were run successfully. The verifier checks the hash **multiset**, including
multiplicity. A second local-archive restore into its cache also checked locked
sizes. No live update or runtime whatspec dependency is involved.

After restoring the pinned lock with the existing CLI, regenerate the small
fixture using Node (no npm dependencies):

```sh
node wacore/tests/fixtures/retry_metadata.mjs /path/to/restored/bundles \
  > /tmp/retry_metadata.json
cmp /tmp/retry_metadata.json wacore/tests/fixtures/retry_metadata.json
```

The runner validates the two entire bundle hashes before evaluating the actual
module slices. Byte ranges below are zero-based, half-open; module boundaries
were obtained from Acorn's `__d` call AST, not brace regex. Formatting was applied
only to copies. Original public URLs are the entries in the pinned lock:

| Bundle | SHA-256 | Original URL suffix |
| --- | --- | --- |
| A: `4oJRvhb3yiE.js` | `f588ee2f033b87e018a3be34a37ec1347d8b6b8700de539e6be31481b29c4690` | `/rsrc.php/v4iOzA4/yI/l/en_US-j/4oJRvhb3yiE.js` |
| B: `Gq1RScKkurbGlqGfAO78c7xtag4u88eLOz_KXXGjbnflonZ_Reb454b-H5Oows5NHkOoIoRr6pxGHxW-26enxPTN.js` | `09d91fdc4089c50ab1d35dd6c796ee82ef4c3296710316d20383c85738b871b8` | See lock entry with this hash |
| C: `f1Q5yyJIapQ.js` | `430cb8979511aada95178605faa35a8483446f677ce6d1fe0fd66333c7879eee` | `/rsrc.php/v4/yR/r/f1Q5yyJIapQ.js` |
| D: `y-wZBCSidI1.js` | `1ee0c4f92eeddf751b83800a4e225043e774a40cda9462a080bb3306acc7a9b9` | See lock entry with this hash |

URL host: `https://static.whatsapp.net`. Some retry modules occur identically in
another locked bundle too; D is the source used for the formatted copies.

## Evidence table

`code` means inspection of the verified archived source, `isolated` means actual
module execution with synthetic records. Neither means live recipient behavior.
Formatted line references use Prettier 3.9.9's Babel parser.

| ID / conclusion | JS module / export, source range; formatted lines | Rust boundary / test | Level |
| --- | --- | --- | --- |
| M1: view-once attr is literal `"true"`, omitted unless `msgRecord.data.mediaData.isViewOnce === true`; no otherwise-empty meta | A `WAWebSendMsgMetaNode.genMetaNode`, [985968,990146); 38–116 | `message_meta_from_message`; `metadata_matches_archived_js_oracle` | code + isolated |
| M2: inline image/video/audio/PTV populate a media record from `viewOnce === true`, not extended text or interactive headers | B `WAWebParseImageMessageProto` [344242,347609), 56/98; `WAWebParseVideoMessageProto` [349300,352432), 58/98; `WAWebPttParseAudioMessageProto` [4550799,4552263), 27/63; `WAWebPttParsePttMessageProto` [4552265,4553721), 27/63; `WAWebPtvParsePtvMessageProto` [4553723,4555638), 37/77 | Inline true/false/absent, quoted/header negatives in fixture; general `MessageExt::is_view_once` unchanged | code (records simulated in oracle) |
| M2a: extended text parses as chat without using its inline viewOnce field to create mediaData | B `WAWebParseChatMessageProto` [4454974,4455707); `WAWebParseExtendedTextMessageProtoUtils.parseExtendedTextMessageProto` [322515,325262), 17–85 | Extended-text true/wrapped negatives, broad general predicate unchanged | code |
| M3: view-once outgoing wrapper: PTT uses V2Extension, other media uses legacy wrapper; inline generators also retain the flag | A `WAWebE2EProtoGenerator` [1338574,1358591), `v`/`x`, 508–511/581; `WAWebPttGenerateAudioOrPttMessageProto` [1317197,1318467), 37 | Supported media leaves and V1/V2/V2Extension fixtures; no new format authorizations | code |
| M4: legitimate user/group retry actually calls genMetaNode and inserts its result in the final message | D `WAWebSendRetryMsgJob` [28476,32377), 162–233 → A `WAWebSendMsgCreateDeviceStanza` [990148,997866), 171/248–287/return children | `prepare_pairwise_retry_stanza`; routing matrix; Client transport/decryption and group tests | code |
| M5: normal fanout and group skmsg also insert genMetaNode | A `WAWebSendMsgCreateFanoutStanza` [1008314,1037003), 1293–1378; `WAWebSendGroupSkmsgJob` [1092482,1105610), 400 | Client `infer_stanza_metadata` → `extra_nodes`; normal group and retry test | code |
| M6: media type describes original media, independent of SKDM; DSM/ephemeral/legacy/V2Extension are traversed by mediaTypeFromProtobuf | B `WAWebBackendJobsCommon.mediaTypeFromProtobuf`/`encodeMaybeMediaType`, [157437,171411), 131–296 | Existing `media_type_from_message` untouched; retry assertions image/PTT, routing/count/identity | code |
| M7: getUnwrappedProtobufMessage validates message keys then returns the **first** nested Message, not the deepest leaf | C `WAWebVerifyProtobufMsgObjectKeys`, [434797,442805), `m`/`p`, 440–465 | Separate first-unwrapped content and media traversal; nested DSM+ephemeral event intentionally has no event_type | code + isolated |
| M8: the derived meta is not the only possible meta; scheduling contributes a separate one | A fanout, 1170/1360; `WAWebScheduledMsgStanzaContributor.genScheduledMsgMetaNode`, [1002397,1002950), 5–30 | `build_extra_stanza_nodes` composition unchanged; normal group test retains two nodes, retry reconstructs only content meta | code |
| M9: status retry has a distinct SKMSG/SKDM constructor and status meta, not genMetaNode | D `WAWebResendStatusMsg`, [20668,24680), 48–89 → A `WAWebEncryptAndSendStatusMsg.buildStatusMetaNode`, [1126491,1142265), 629–646 | `retransmit_status_message` unchanged, no pairwise count/meta retrofitted | code |
| M10: supported poll/event/member-label families are content/record-derived; result snapshots are gated | A `WAWebSendMsgMetaNode`, 118–184 | Shared existing V1–V3 poll creation/vote, event creation/response/edit and member label; gated absence fixture | code + isolated |
| M11: metadata construction does not authorize retry | D `WAWebProcessRetryKeyBundle.getMsgIfAuthorized`, [9758,15790), 105–179; C `WAWebApiMessageInfoStore.isRetryEligible`, [764525,770477) | Existing retry caps/cache/identity/device/history/audience/durability decisions unchanged | code; no policy oracle/live |

The structured IR contains view_once/event_type/polltype/status_setting shapes,
but does not encode these control-flow and media-record conditions. Its manifest
has extraction diagnostics (including 5 unparseable IQ candidates); absence of an
IR field is not used as protocol evidence here.

### Oracle scope and a disproven assumption

The fixture runs **real** `genMetaNode` and **real** protobuf-key validation /
getUnwrappedProtobufMessage. Stubs disable bot/origin metadata, the poll-result
snapshot envelope experiment, and the diagnostic-only three-level nesting gate.
Media records are synthetic values justified by the inspected media parsers and
wrapper generator; the media pipeline, uploading and retry eligibility are not
executed. `invalid: true` denotes inputs rejected by actual JS validation; their
empty expected object is a defensive Rust no-meta assertion, not proof JS sends
those malformed messages. The fixture's audio cases are voice notes.

Execution refuted the assumption that all meta families should inspect the
deepest unwrapped payload: a doubly wrapped event has **no** event_type in this
snapshot. That absence is retained. View-once instead comes from a media record;
its marker must survive borrowed traversal and requires an image/video/audio/PTV
leaf. Empty/text/button/interactive wrappers cannot manufacture that record.
Quoted messages and other references are never traversed. Traversal uses the
protobuf decoder's existing recursion budget, without a new unbounded walk.

## Ownership, compatibility and limits

Normal Client sends infer the pure rule and insert it via existing extra-node
composition; normal core builders still accept extra nodes and do not auto-insert
it. Pairwise retry has no extra-node input, so that builder alone inserts the
shared derived meta. Direct core callers consequently get the correction too,
without mandatory public struct fields or a contradictory configuration boolean.
Caller-supplied edit attributes retain their precedence and revoke distinctions.
`pre_encoded`, encryption, routing, account identity, count and pre-wire durability
are unchanged. General `MessageExt::is_view_once` remains broad for other callers.
No tokens, secrets, media keys, IDs, timestamps or original extra nodes are replayed
or regenerated. Status, peer and newsletter transport branches are not refactored.

No live test was authorized or executed. Missing view_once is demonstrated at the
stanza boundary; recipient rendering, server rejection and delivery consequences
are not established by this investigation.

Independent findings kept outside this patch: the snapshot recognizes newer poll
creation/edit families beyond the already-supported Rust inference subset; its
mediaTypeFromProtobuf omits V2 while Rust's existing broader classifier handles it.
Neither the entire catalog nor those media-type choices are changed here.

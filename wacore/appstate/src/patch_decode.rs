//! Patch list parsing (snapshot + patches) - partial port of Go appstate/decode.go

use anyhow::{Result, anyhow};
use std::str::FromStr;
use wacore_binary::node::{Node, NodeRef};
use waproto::whatsapp as wa;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WAPatchName {
    CriticalBlock,
    CriticalUnblockLow,
    RegularLow,
    RegularHigh,
    Regular,
    Unknown,
}

impl WAPatchName {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::CriticalBlock => "critical_block",
            Self::CriticalUnblockLow => "critical_unblock_low",
            Self::RegularLow => "regular_low",
            Self::RegularHigh => "regular_high",
            Self::Regular => "regular",
            Self::Unknown => "unknown",
        }
    }
}

impl FromStr for WAPatchName {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "critical_block" => Self::CriticalBlock,
            "critical_unblock_low" => Self::CriticalUnblockLow,
            "regular_low" => Self::RegularLow,
            "regular_high" => Self::RegularHigh,
            "regular" => Self::Regular,
            _ => Self::Unknown,
        })
    }
}

#[derive(Debug, Clone)]
pub struct PatchList {
    pub name: WAPatchName,
    pub has_more_patches: bool,
    pub patches: Vec<wa::SyncdPatch>,
    pub snapshot: Option<wa::SyncdSnapshot>, // filled only if already present inline (currently never)
    pub snapshot_ref: Option<wa::ExternalBlobReference>, // external reference to fetch
    /// Per-collection error from server (None = success).
    pub error: Option<CollectionSyncError>,
}

/// Per-collection error returned by the server inside a `<collection type="error">` node.
/// Matches WA Web's CollectionState enum (`GysEGRAXCvh.js:44755`).
#[derive(Debug, Clone)]
pub enum CollectionSyncError {
    /// 409: Version conflict — patches were applied concurrently.
    /// `has_more` indicates if there are more patches after resolving.
    Conflict { has_more: bool },
    /// 400 or 404: Unrecoverable server error.
    Fatal { code: u16, text: String },
    /// Any other error code: transient, can retry.
    Retry { code: u16, text: String },
}

impl std::fmt::Display for CollectionSyncError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Conflict { has_more } => write!(f, "conflict (has_more={has_more})"),
            Self::Fatal { code, text } => write!(f, "fatal error {code}: {text}"),
            Self::Retry { code, text } => write!(f, "retryable error {code}: {text}"),
        }
    }
}

/// Parse an incoming app state collection node into a PatchList.
/// Node path: sync -> collection (attributes: name, has_more_patches)
pub fn parse_patch_list(node: &Node) -> Result<PatchList> {
    let collection = node
        .get_optional_child_by_tag(&["sync", "collection"]) // naive path descent
        .ok_or_else(|| anyhow!("missing sync/collection"))?;
    parse_single_collection(collection)
}

/// Zero-copy entry point for `parse_patch_list`.
pub fn parse_patch_list_ref(node: &NodeRef<'_>) -> Result<PatchList> {
    parse_patch_list(&node.to_owned())
}

/// Parse all `<collection>` children from a `<sync>` response into PatchLists.
/// Used for batched multi-collection IQ responses.
/// Tolerates both `<iq><sync>...</sync></iq>` and bare `<sync>...</sync>` roots.
pub fn parse_patch_lists_ref(node: &NodeRef<'_>) -> Result<Vec<PatchList>> {
    parse_patch_lists(&node.to_owned())
}

pub fn parse_patch_lists(node: &Node) -> Result<Vec<PatchList>> {
    let sync_node = if node.tag == "sync" {
        node
    } else {
        node.get_optional_child("sync")
            .ok_or_else(|| anyhow!("missing sync node in response"))?
    };

    let Some(children) = sync_node.children() else {
        return Ok(Vec::new());
    };

    children
        .iter()
        .filter(|c| c.tag == "collection")
        .map(parse_single_collection)
        .collect()
}

/// Parse a single `<collection>` node into a PatchList.
fn parse_single_collection(collection: &Node) -> Result<PatchList> {
    let mut ag = collection.attrs();
    let name_str = ag
        .optional_string("name")
        .ok_or_else(|| anyhow!("collection missing 'name' attribute"))?
        .to_string();
    let has_more = ag.optional_bool("has_more_patches");

    // Check for per-collection error (WA Web: `3JJWKHeu5-P.js:54222-54254`)
    let col_type = ag.optional_string("type");
    let error = parse_collection_error(collection, col_type.as_deref());

    ag.finish()?;

    // snapshot (optional)
    let mut snapshot_ref = None;
    if let Some(snapshot_node) = collection.get_optional_child("snapshot")
        && let Some(wacore_binary::node::NodeContent::Bytes(raw)) = &snapshot_node.content
        && let Ok(ext_ref) = waproto::codec::external_blob_reference_decode(raw.as_slice())
    {
        snapshot_ref = Some(ext_ref);
    }
    let snapshot = None; // external only currently

    // patches list
    let children_ref = collection
        .get_optional_child("patches")
        .and_then(|n| n.children());
    let mut patches: Vec<wa::SyncdPatch> =
        Vec::with_capacity(children_ref.as_ref().map_or(0, |c| c.len()));
    if let Some(children) = children_ref {
        for child in children {
            if child.tag == "patch"
                && let Some(wacore_binary::node::NodeContent::Bytes(raw)) = &child.content
            {
                match waproto::codec::syncd_patch_decode(raw.as_slice()) {
                    Ok(p) => patches.push(p),
                    Err(e) => return Err(anyhow!("failed to unmarshal patch: {e}")),
                }
            }
        }
    }

    Ok(PatchList {
        name: WAPatchName::from_str(&name_str).unwrap_or(WAPatchName::Unknown),
        has_more_patches: has_more,
        patches,
        snapshot,
        snapshot_ref,
        error,
    })
}

/// Parse per-collection error from `<collection type="error"><error code="..." text="..."/>`.
/// Returns `None` for successful collections.
fn parse_collection_error(
    collection: &Node,
    col_type: Option<&str>,
) -> Option<CollectionSyncError> {
    if col_type? != "error" {
        return None;
    }

    // Parse error details from child node, or fall back to a default retryable
    // error if the <error> child is missing/malformed.
    let (code, text) = if let Some(error_node) = collection.get_optional_child("error") {
        let mut error_attrs = error_node.attrs();
        let code_str = error_attrs.optional_string("code");
        let text = error_attrs
            .optional_string("text")
            .as_deref()
            .unwrap_or("")
            .to_string();
        let code: u16 = code_str.as_deref().unwrap_or("0").parse().unwrap_or(0);
        (code, text)
    } else {
        (0u16, "missing <error> child".to_string())
    };

    Some(match code {
        409 => CollectionSyncError::Conflict {
            has_more: collection.attrs().optional_bool("has_more_patches"),
        },
        400 | 404 => CollectionSyncError::Fatal { code, text },
        _ => CollectionSyncError::Retry { code, text },
    })
}

#[cfg(test)]
// Fixtures encode protos the pinned `waproto::codec` wrappers don't cover; the
// binary-size reason for pinning them doesn't apply to test code.
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;
    use buffa::Message;
    use wacore_binary::builder::NodeBuilder;

    /// Length-delimited field 1 announcing 5 bytes but carrying 1, so every
    /// protobuf decoder in the tests below fails deterministically.
    const TRUNCATED_PROTOBUF: [u8; 3] = [0x0A, 0x05, 0x01];

    fn patch_bytes(version: u64) -> Vec<u8> {
        waproto::codec::syncd_patch_to_vec(&wa::SyncdPatch {
            version: buffa::MessageField::some(wa::SyncdVersion {
                version: Some(version),
            }),
            snapshot_mac: Some(vec![0xAB; 32]),
            ..Default::default()
        })
    }

    fn snapshot_ref_bytes(direct_path: &str) -> Vec<u8> {
        wa::ExternalBlobReference {
            direct_path: Some(direct_path.to_string()),
            file_size_bytes: Some(4096),
            ..Default::default()
        }
        .encode_to_vec()
    }

    /// `<collection>` carrying a snapshot ref and two patches.
    fn full_collection() -> Node {
        NodeBuilder::new("collection")
            .attr("name", "regular")
            .attr("has_more_patches", "true")
            .children([
                NodeBuilder::new("snapshot")
                    .bytes(snapshot_ref_bytes("/appstate/snapshot.enc"))
                    .build(),
                NodeBuilder::new("patches")
                    .children([
                        NodeBuilder::new("patch").bytes(patch_bytes(1)).build(),
                        NodeBuilder::new("patch").bytes(patch_bytes(2)).build(),
                    ])
                    .build(),
            ])
            .build()
    }

    fn sync_node(collections: impl IntoIterator<Item = Node>) -> Node {
        NodeBuilder::new("sync").children(collections).build()
    }

    fn iq_with(collections: impl IntoIterator<Item = Node>) -> Node {
        NodeBuilder::new("iq")
            .attr("type", "result")
            .children([sync_node(collections)])
            .build()
    }

    #[test]
    fn patch_name_roundtrips_every_wire_string() {
        for name in [
            WAPatchName::CriticalBlock,
            WAPatchName::CriticalUnblockLow,
            WAPatchName::RegularLow,
            WAPatchName::RegularHigh,
            WAPatchName::Regular,
        ] {
            assert_eq!(WAPatchName::from_str(name.as_str()), Ok(name));
        }
        assert_eq!(
            WAPatchName::from_str("something_new"),
            Ok(WAPatchName::Unknown)
        );
    }

    #[test]
    fn parse_patch_list_reads_name_patches_and_snapshot_ref() {
        let list = parse_patch_list(&iq_with([full_collection()])).expect("well-formed collection");

        assert_eq!(list.name, WAPatchName::Regular);
        assert!(list.has_more_patches);
        assert!(list.error.is_none());
        // Inline snapshots are never sent; only the external reference is parsed.
        assert!(list.snapshot.is_none());
        assert_eq!(
            list.snapshot_ref
                .as_ref()
                .and_then(|r| r.direct_path.as_deref()),
            Some("/appstate/snapshot.enc")
        );

        let versions: Vec<Option<u64>> = list
            .patches
            .iter()
            .map(|p| p.version.as_option().and_then(|v| v.version))
            .collect();
        assert_eq!(versions, vec![Some(1), Some(2)]);
    }

    #[test]
    fn parse_patch_list_ref_matches_owned_path() {
        let node = iq_with([full_collection()]);
        let list = parse_patch_list_ref(&node.as_node_ref()).expect("well-formed collection");

        assert_eq!(list.name, WAPatchName::Regular);
        assert_eq!(list.patches.len(), 2);
    }

    #[test]
    fn parse_patch_list_defaults_missing_optional_fields() {
        let collection = NodeBuilder::new("collection")
            .attr("name", "critical_block")
            .build();
        let list = parse_patch_list(&iq_with([collection])).expect("name alone is enough");

        assert_eq!(list.name, WAPatchName::CriticalBlock);
        assert!(!list.has_more_patches);
        assert!(list.patches.is_empty());
        assert!(list.snapshot_ref.is_none());
        assert!(list.error.is_none());
    }

    #[test]
    fn parse_patch_list_maps_unknown_collection_name() {
        let collection = NodeBuilder::new("collection")
            .attr("name", "collection_from_the_future")
            .build();
        let list = parse_patch_list(&iq_with([collection])).expect("unknown names are tolerated");

        assert_eq!(list.name, WAPatchName::Unknown);
    }

    #[test]
    fn parse_patch_list_rejects_missing_sync_collection_path() {
        let err = parse_patch_list(&NodeBuilder::new("iq").build())
            .expect_err("no sync/collection to descend into");
        assert!(err.to_string().contains("missing sync/collection"));

        let err = parse_patch_list(&iq_with([]))
            .expect_err("a sync node without collections is still unusable here");
        assert!(err.to_string().contains("missing sync/collection"));
    }

    #[test]
    fn parse_single_collection_rejects_missing_name() {
        let collection = NodeBuilder::new("collection")
            .attr("has_more_patches", "true")
            .build();
        let err = parse_patch_list(&iq_with([collection]))
            .expect_err("a nameless collection cannot be routed anywhere");
        assert!(
            err.to_string()
                .contains("collection missing 'name' attribute")
        );
    }

    #[test]
    fn parse_single_collection_rejects_undecodable_patch() {
        let collection = NodeBuilder::new("collection")
            .attr("name", "regular")
            .children([NodeBuilder::new("patches")
                .children([
                    NodeBuilder::new("patch").bytes(patch_bytes(1)).build(),
                    NodeBuilder::new("patch")
                        .bytes(TRUNCATED_PROTOBUF.to_vec())
                        .build(),
                ])
                .build()])
            .build();

        let err = parse_patch_list(&iq_with([collection]))
            .expect_err("a garbled patch must not be silently dropped");
        assert!(err.to_string().contains("failed to unmarshal patch"));
    }

    #[test]
    fn parse_single_collection_skips_unusable_snapshot_nodes() {
        // Unlike patches, a snapshot that fails to decode is tolerated: the
        // caller falls back to applying patches from the current version.
        let collection = NodeBuilder::new("collection")
            .attr("name", "regular")
            .children([NodeBuilder::new("snapshot")
                .bytes(TRUNCATED_PROTOBUF.to_vec())
                .build()])
            .build();
        let list = parse_patch_list(&iq_with([collection])).expect("snapshot errors are tolerated");
        assert!(list.snapshot_ref.is_none());

        // A snapshot carrying a string instead of bytes is likewise ignored.
        let collection = NodeBuilder::new("collection")
            .attr("name", "regular")
            .children([NodeBuilder::new("snapshot")
                .string_content("not-bytes")
                .build()])
            .build();
        let list = parse_patch_list(&iq_with([collection])).expect("snapshot errors are tolerated");
        assert!(list.snapshot_ref.is_none());
    }

    #[test]
    fn parse_patch_lists_accepts_both_iq_and_bare_sync_roots() {
        let collections = || {
            [
                full_collection(),
                NodeBuilder::new("collection")
                    .attr("name", "regular_high")
                    .build(),
            ]
        };

        for root in [iq_with(collections()), sync_node(collections())] {
            let lists = parse_patch_lists(&root).expect("well-formed sync node");
            let names: Vec<WAPatchName> = lists.iter().map(|l| l.name).collect();
            assert_eq!(names, vec![WAPatchName::Regular, WAPatchName::RegularHigh]);
        }
    }

    #[test]
    fn parse_patch_lists_ref_matches_owned_path() {
        let node = sync_node([full_collection()]);
        let lists = parse_patch_lists_ref(&node.as_node_ref()).expect("well-formed sync node");
        assert_eq!(lists.len(), 1);
    }

    #[test]
    fn parse_patch_lists_ignores_non_collection_children() {
        let root = NodeBuilder::new("sync")
            .children([NodeBuilder::new("unexpected").build(), full_collection()])
            .build();
        let lists = parse_patch_lists(&root).expect("unknown siblings are skipped");
        assert_eq!(lists.len(), 1);
    }

    #[test]
    fn parse_patch_lists_rejects_missing_sync_node() {
        let err = parse_patch_lists(&NodeBuilder::new("iq").build())
            .expect_err("no sync node to read collections from");
        assert!(err.to_string().contains("missing sync node in response"));
    }

    #[test]
    fn parse_patch_lists_returns_empty_for_childless_sync() {
        let lists = parse_patch_lists(&NodeBuilder::new("sync").build())
            .expect("an empty sync node is not an error");
        assert!(lists.is_empty());
    }

    #[test]
    fn parse_patch_lists_propagates_a_single_bad_collection() {
        let root = sync_node([
            full_collection(),
            NodeBuilder::new("collection").build(), // no name
        ]);
        let err = parse_patch_lists(&root).expect_err("one bad collection fails the batch");
        assert!(
            err.to_string()
                .contains("collection missing 'name' attribute")
        );
    }

    fn error_collection(error_child: Option<Node>) -> Node {
        let mut children = Vec::new();
        children.extend(error_child);
        NodeBuilder::new("collection")
            .attr("name", "regular")
            .attr("type", "error")
            .attr("has_more_patches", "true")
            .children(children)
            .build()
    }

    fn parse_error(collection: Node) -> Option<CollectionSyncError> {
        parse_patch_list(&iq_with([collection]))
            .expect("an error collection still parses")
            .error
    }

    #[test]
    fn collection_error_409_becomes_conflict_carrying_has_more() {
        let error = parse_error(error_collection(Some(
            NodeBuilder::new("error").attr("code", "409").build(),
        )));
        assert!(matches!(
            error,
            Some(CollectionSyncError::Conflict { has_more: true })
        ));
    }

    #[test]
    fn collection_errors_400_and_404_are_fatal() {
        for code in ["400", "404"] {
            let error = parse_error(error_collection(Some(
                NodeBuilder::new("error")
                    .attr("code", code)
                    .attr("text", "bad request")
                    .build(),
            )));
            let Some(CollectionSyncError::Fatal { code: got, text }) = error else {
                panic!("expected a fatal error for code {code}, got {error:?}");
            };
            assert_eq!(got.to_string(), code);
            assert_eq!(text, "bad request");
        }
    }

    #[test]
    fn other_collection_errors_are_retryable() {
        let error = parse_error(error_collection(Some(
            NodeBuilder::new("error")
                .attr("code", "500")
                .attr("text", "internal")
                .build(),
        )));
        assert!(matches!(
            error,
            Some(CollectionSyncError::Retry { code: 500, .. })
        ));
    }

    #[test]
    fn unparseable_error_codes_fall_back_to_retry() {
        // Missing child, missing code and a non-numeric code all land on the
        // same conservative "retry with code 0" answer.
        let cases = [
            (None, "missing <error> child"),
            (Some(NodeBuilder::new("error").build()), ""),
            (
                Some(
                    NodeBuilder::new("error")
                        .attr("code", "not-a-number")
                        .build(),
                ),
                "",
            ),
        ];

        for (child, expected_text) in cases {
            let error = parse_error(error_collection(child));
            let Some(CollectionSyncError::Retry { code, text }) = error else {
                panic!("expected a retryable error, got {error:?}");
            };
            assert_eq!(code, 0);
            assert_eq!(text, expected_text);
        }
    }

    #[test]
    fn non_error_collection_type_yields_no_error() {
        let collection = NodeBuilder::new("collection")
            .attr("name", "regular")
            .attr("type", "result")
            .children([NodeBuilder::new("error").attr("code", "500").build()])
            .build();
        // The `<error>` child only counts when the collection is typed as one.
        assert!(parse_error(collection).is_none());
    }

    #[test]
    fn collection_sync_error_display_names_each_variant() {
        assert_eq!(
            CollectionSyncError::Conflict { has_more: false }.to_string(),
            "conflict (has_more=false)"
        );
        assert_eq!(
            CollectionSyncError::Fatal {
                code: 404,
                text: "not found".to_string(),
            }
            .to_string(),
            "fatal error 404: not found"
        );
        assert_eq!(
            CollectionSyncError::Retry {
                code: 503,
                text: "try later".to_string(),
            }
            .to_string(),
            "retryable error 503: try later"
        );
    }
}

//! Downstream API contract: only public imports and macro expansion are used.
use serde::{Serialize, Serializer, ser::SerializeMap};
use whatsapp_rust::features::MexOperation as FeatureMexOperation;
use whatsapp_rust::{
    MexOperation, MexRequest, mex_operation,
    wacore::iq::{
        mex::MexQuerySpec,
        mex_operations::{fetch_all_subgroups, join_newsletter},
        spec::IqSpec,
    },
    wacore_binary::NodeContent,
};

fn wire<V: Serialize>(request: &MexRequest<V>) -> Vec<u8> {
    let iq = MexQuerySpec::new(request.doc(), &request.variables)
        .unwrap()
        .build_iq();
    let Some(NodeContent::Nodes(nodes)) = iq.content else {
        panic!("query child")
    };
    assert_eq!(nodes[0].attrs.get("query_id").unwrap(), request.doc().id);
    let Some(NodeContent::Bytes(bytes)) = &nodes[0].content else {
        panic!("wire bytes")
    };
    bytes.clone()
}

#[test]
fn descriptor_binds_same_module_metadata_and_variables() {
    let op: MexOperation<join_newsletter::Variables> = mex_operation!(join_newsletter);
    let _: FeatureMexOperation<join_newsletter::Variables> = op;
    assert_eq!(op.doc().name, join_newsletter::NAME);
    assert_eq!(op.doc().id, join_newsletter::DOC_ID);
    assert_eq!(op.declared_variables(), join_newsletter::VARIABLE_KEYS);
    let request = op.request(join_newsletter::Variables {
        newsletter_id: Some("123456789@newsletter".into()),
    });
    assert_eq!(request.declared_variables(), op.declared_variables());
    assert_eq!(
        wire(&request),
        br#"{"variables":{"newsletter_id":"123456789@newsletter"}}"#
    );
}

#[test]
fn macro_expansion_is_hygienic_through_a_renamed_crate_import() {
    use whatsapp_rust as wa;
    let op = wa::mex_operation!(wa::wacore::iq::mex_operations::join_newsletter);
    let request = op.request(join_newsletter::Variables {
        newsletter_id: None,
    });
    assert_eq!(request.doc().id, join_newsletter::DOC_ID);
    assert_eq!(wire(&request), br#"{"variables":{}}"#);
}

#[test]
fn optional_omission_remains_a_diagnostic_not_a_construction_error() {
    let request = mex_operation!(fetch_all_subgroups).request(fetch_all_subgroups::Variables {
        group_id: Some("123456789@g.us".into()),
        query_context: Some("INTERACTIVE".into()),
        sub_group_hint_id: None,
    });
    assert_eq!(request.missing_variables().unwrap(), ["sub_group_hint_id"]);
    assert_eq!(
        wire(&request),
        br#"{"variables":{"group_id":"123456789@g.us","query_context":"INTERACTIVE"}}"#
    );
}

// A Value intermediate would collapse duplicate map keys. The raw escape must
// preserve the serializer's bytes, just as the existing spec does.
struct StreamingVariables<'a>(&'a str);
impl Serialize for StreamingVariables<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(2))?;
        map.serialize_entry("newsletter_id", self.0)?;
        map.serialize_entry("newsletter_id", self.0)?;
        map.end()
    }
}

#[test]
fn explicit_raw_path_preserves_direct_wire_serialization() {
    let op = mex_operation!(join_newsletter);
    let request = op.raw_request(StreamingVariables("123456789@newsletter"));
    assert_eq!(request.doc().name, join_newsletter::NAME);
    assert_eq!(wire(&request), br#"{"variables":{"newsletter_id":"123456789@newsletter","newsletter_id":"123456789@newsletter"}}"#);
    let custom = MexRequest::new_raw(op.doc(), op.declared_variables(), &request.variables);
    assert_eq!(wire(&custom), wire(&request));
}

#[test]
fn custom_descriptor_and_raw_constructor_remain_available() {
    let doc = whatsapp_rust::MexDoc {
        name: "ConsumerCustomQuery",
        id: "123456789",
    };
    let operation = MexOperation::<serde_json::Value>::from_raw_parts(doc, &["optional"]);
    let request = operation.request(serde_json::json!({}));
    assert_eq!(request.missing_variables().unwrap(), ["optional"]);
    assert_eq!(wire(&request), br#"{"variables":{}}"#);
    let raw = MexRequest::new_raw(doc, &["optional"], &request.variables);
    assert_eq!(wire(&raw), wire(&request));
}

// Also check the canonical executor's future from a downstream package.
#[allow(dead_code)]
async fn public_execution(client: &whatsapp_rust::Client) -> Result<(), whatsapp_rust::MexError> {
    let op = mex_operation!(join_newsletter);
    client
        .mex()
        .execute(op.request(join_newsletter::Variables {
            newsletter_id: None,
        }))
        .await?;
    Ok(())
}

use wacore::iq::mex_operations::biz_custom_url_get_user_graphql::{
    CustomUrl, DOC_ID, Data, NAME, OPERATION_KIND, Response, VARIABLE_KEYS, Variables,
};

#[test]
fn historical_custom_url_query_keeps_its_public_path_and_payload() {
    let mut custom_url = CustomUrl::default();
    custom_url.path = Some("example".into());
    let mut data = Data::default();
    data.custom_url = Some(custom_url);
    let mut variables = Variables::default();
    variables.data = Some(data);
    assert_eq!(
        serde_json::to_value(variables).unwrap(),
        serde_json::json!({"data":{"custom_url":{"path":"example"}}})
    );
    let response: Response = serde_json::from_value(
        serde_json::json!({"xwa_custom_url_get_user":{"lid":"100000000000001","success":"true"}}),
    )
    .unwrap();
    let user = response.xwa_custom_url_get_user.unwrap();
    assert_eq!(user.lid.as_deref(), Some("100000000000001"));
    assert_eq!(user.success.as_deref(), Some("true"));
    assert_eq!(DOC_ID, "26867176859566677");
    assert_eq!(NAME, "WAWebBizCustomUrlGetUserGraphqlQuery");
    assert_eq!(OPERATION_KIND, "query");
    assert_eq!(VARIABLE_KEYS, &["data"]);
}

#[test]
fn historical_optional_fields_and_list_input_remain_source_compatible() {
    use wacore::iq::mex_operations as mex;
    macro_rules! string_field {
        ($ty:path, $field:ident) => {{
            let value: $ty = Default::default();
            let _: Option<String> = value.$field;
        }};
    }
    string_field!(
        mex::biz_query_order::Order,
        direct_connection_encrypted_info
    );
    string_field!(
        mex::contact_manager_customer_profile::XfbWaCustomerProfile,
        last_order_date
    );
    string_field!(mex::contact_manager_customer_profile_upsert::Profiles, lid);
    string_field!(
        mex::contact_manager_customer_profiles::Profiles,
        last_order_date
    );
    string_field!(mex::contact_manager_customer_profiles::Profiles, lid);
    string_field!(mex::fetch_newsletter_pending_invites::User, pn);
    string_field!(
        mex::query_catalog::ProductCatalog,
        direct_connection_encrypted_info
    );
    string_field!(
        mex::query_catalog_has_categories::Categories,
        direct_connection_encrypted_info
    );
    string_field!(
        mex::query_catalog_product::Product,
        direct_connection_encrypted_info
    );
    string_field!(
        mex::query_product_collections::Collections,
        direct_connection_encrypted_info
    );
    string_field!(
        mex::query_product_single_collection::Collection,
        direct_connection_encrypted_info
    );
    let mut variables = mex::contact_manager_customer_profile_upsert::Variables::default();
    variables.input = Some(vec!["historical-input".into()]);
    assert_eq!(
        serde_json::to_value(variables).unwrap(),
        serde_json::json!({"input":["historical-input"]})
    );
}

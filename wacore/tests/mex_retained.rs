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

#[test]
fn about_status_and_subscription_contracts_survive_the_new_snapshot() {
    use wacore::iq::mex_operations as mex;
    let mut variables = mex::usync::Variables::default();
    variables.include_about_status = Some(true);
    assert_eq!(
        serde_json::to_value(variables).unwrap(),
        serde_json::json!({"include_about_status":true})
    );
    let response: mex::usync::Response = serde_json::from_value(serde_json::json!({
        "xwa2_fetch_wa_users":[{"about_status_info":{"status":"set","text":"synthetic about","timestamp":123}}]
    })).unwrap();
    let about = response
        .xwa2_fetch_wa_users
        .unwrap()
        .remove(0)
        .about_status_info
        .unwrap();
    assert_eq!(about.text.as_deref(), Some("synthetic about"));
    assert_eq!(about.timestamp, Some(123));
    let response: mex::fetch_subscriptions::Response = serde_json::from_value(serde_json::json!({
        "xwa_get_subscriptions":{"feature_flags":[{"expiration_time":123}],"subscriptions":[{"is_platform_changed":false}]}
    })).unwrap();
    let subscriptions = response.xwa_get_subscriptions.unwrap();
    assert_eq!(
        subscriptions.feature_flags.unwrap()[0].expiration_time,
        Some(123)
    );
    assert_eq!(
        subscriptions.subscriptions.unwrap()[0].is_platform_changed,
        Some(false)
    );
    let _ = mex::fetch_about_status::Variables::default();
    let _ = mex::internal_directory_fetch_members::Variables::default();
    assert_eq!(mex::fetch_about_status::DOC_ID, "24535500086059408");
    assert_eq!(
        mex::internal_directory_fetch_members::DOC_ID,
        "28645118498472724"
    );
}

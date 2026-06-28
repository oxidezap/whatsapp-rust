//! Auto-generated typed mex operations (WhatsApp 2.3000.1040878135). DO NOT EDIT.
//!
//! One module per persisted GraphQL operation: typed `Variables` + `Response`
//! plus `DOC_ID`/`OPERATION_KIND`/`NAME`. Depends only on `serde`.

#![allow(clippy::all)]

use serde::{Deserialize, Serialize};

/// `WAWebACSServerProviderConfigQuery` (query).
pub mod acs_server_provider_config {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebACSServerProviderConfigQuery";
    pub const DOC_ID: &str = "25133761326299603";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub project_name: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct XwaWaAcsConfig {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub cipher_suite: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub expire_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub max_evals: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub public_key: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub redemption_limit: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub token_ttl: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa_wa_acs_config: Option<XwaWaAcsConfig>,
    }
}

/// `WAWebACSServerProviderIssuanceMutation` (mutation).
pub mod acs_server_provider_issuance {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebACSServerProviderIssuanceMutation";
    pub const DOC_ID: &str = "26039599689054760";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Input {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub config_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub issue_element: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub project_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub request_proof: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<Input>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Evaluation {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub data: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Proof {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub c: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub s: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Creds {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub evaluation: Option<Vec<Evaluation>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub proof: Option<Vec<Proof>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct XwaWaAcsIssueCredentials {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub creds: Option<Creds>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub error_message: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub success: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa_wa_acs_issue_credentials: Option<XwaWaAcsIssueCredentials>,
    }
}

/// `WAWebMexAcceptNewsletterAdminInviteJobMutation` (mutation).
pub mod accept_newsletter_admin_invite {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexAcceptNewsletterAdminInviteJobMutation";
    pub const DOC_ID: &str = "9580828702035549";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub newsletter_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2NewsletterAdminInviteAccept {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_newsletter_admin_invite_accept: Option<Xwa2NewsletterAdminInviteAccept>,
    }
}

/// `WAWebAiAgentAutoReplyControlMutation` (mutation).
pub mod ai_agent_auto_reply_control {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebAiAgentAutoReplyControlMutation";
    pub const DOC_ID: &str = "24541201322134132";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub consumer_lid: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub phone_number: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub thread_status: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct XfbWhatsappSmbMaibaStatusUpdate {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub success: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xfb_whatsapp_smb_maiba_status_update: Option<XfbWhatsappSmbMaibaStatusUpdate>,
    }
}

/// `WAWebAuthAgentFeaturePolicyQuery` (query).
pub mod auth_agent_feature_policy {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebAuthAgentFeaturePolicyQuery";
    pub const DOC_ID: &str = "26467789126176720";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct WhatsappAuthorizedAgentFeaturePolicy {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub disabled_features: Option<bool>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub whatsapp_authorized_agent_feature_policy: Option<WhatsappAuthorizedAgentFeaturePolicy>,
    }
}

/// `WAWebBPAccessTokenAndSessionCookiesMutation` (mutation).
pub mod bp_access_token_and_session_cookies {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebBPAccessTokenAndSessionCookiesMutation";
    pub const DOC_ID: &str = "26756198580685447";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub application_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub code: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct XwaBpAccessTokenAndSessionCookies {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub access_token: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub access_token_type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub bp_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub email_attr: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub session_cookies: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa_bp_access_token_and_session_cookies: Option<XwaBpAccessTokenAndSessionCookies>,
    }
}

/// `WAWebBizCreateOrderJobMutation` (mutation).
pub mod biz_create_order {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebBizCreateOrderJobMutation";
    pub const DOC_ID: &str = "26486627094287046";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Order {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub jid: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub products: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Input {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub order: Option<Order>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<Input>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Price {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub currency: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub price_status: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub subtotal_amount: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub total_amount: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Order2 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub order_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub price: Option<Price>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub token: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct XwaCheckoutPlaceOrder {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub order: Option<Order2>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa_checkout_place_order: Option<XwaCheckoutPlaceOrder>,
    }
}

/// `WAWebBizCustomUrlGetUserGraphqlQuery` (query).
pub mod biz_custom_url_get_user_graphql {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebBizCustomUrlGetUserGraphqlQuery";
    pub const DOC_ID: &str = "26867176859566677";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct CustomUrl {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub path: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Data {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub custom_url: Option<CustomUrl>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub data: Option<Data>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct XwaCustomUrlGetUser {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub error_code: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub error_text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub lid: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub success: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa_custom_url_get_user: Option<XwaCustomUrlGetUser>,
    }
}

/// `WAWebBizGetCategoriesQuery` (query).
pub mod biz_get_categories {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebBizGetCategoriesQuery";
    pub const DOC_ID: &str = "26266473919627648";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct QueryParams {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub locale: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub operation: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub query: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub version: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub query_params: Option<QueryParams>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Categories {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub display_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct NotABiz {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub display_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct WhatsappCatkitTypeaheadProxy {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub categories: Option<Vec<Categories>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub not_a_biz: Option<NotABiz>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub whatsapp_catkit_typeahead_proxy: Option<WhatsappCatkitTypeaheadProxy>,
    }
}

/// `WAWebBizGetCategoriesV2Query` (query).
pub mod biz_get_categories_v2 {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebBizGetCategoriesV2Query";
    pub const DOC_ID: &str = "26869203922665622";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct QueryParams {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub locale: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub operation: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub query: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub version: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub query_params: Option<QueryParams>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Categories {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub display_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Categories2 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub categories: Option<Vec<Categories>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub display_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Categories3 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub categories: Option<Vec<Categories2>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub display_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct NotABiz {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub display_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct WhatsappCatkitTypeaheadProxy {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub categories: Option<Vec<Categories3>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub not_a_biz: Option<NotABiz>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub whatsapp_catkit_typeahead_proxy: Option<WhatsappCatkitTypeaheadProxy>,
    }
}

/// `WAWebBizGetCustomUrlUserGraphqlQuery` (query).
pub mod biz_get_custom_url_user_graphql {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebBizGetCustomUrlUserGraphqlQuery";
    pub const DOC_ID: &str = "WAWebBizGetCustomUrlUserGraphqlQuery";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct CustomUrl {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub path: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Data {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub custom_url: Option<CustomUrl>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub data: Option<Data>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct User {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub jid: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct XwaCustomUrlGetUser {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub error_code: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub error_text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub success: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub user: Option<User>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa_custom_url_get_user: Option<XwaCustomUrlGetUser>,
    }
}

/// `WAWebBizGetMerchantComplianceQuery` (query).
pub mod biz_get_merchant_compliance {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebBizGetMerchantComplianceQuery";
    pub const DOC_ID: &str = "25960403573553316";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub request: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct CustomerCareDetails {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub email: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub landline_number: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub mobile_number: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct GrievanceOfficerDetails {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub email: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub landline_number: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub mobile_number: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct MerchantInfo {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub customer_care_details: Option<CustomerCareDetails>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub entity_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub entity_type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub entity_type_custom: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub grievance_officer_details: Option<GrievanceOfficerDetails>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_registered: Option<bool>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct XfbWhatsappBizMerchantComplianceInfo {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub merchant_info: Option<MerchantInfo>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xfb_whatsapp_biz_merchant_compliance_info: Option<XfbWhatsappBizMerchantComplianceInfo>,
    }
}

/// `WAWebBizGetPriceTiersQuery` (query).
pub mod biz_get_price_tiers {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebBizGetPriceTiersQuery";
    pub const DOC_ID: &str = "25362864436721857";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Request {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub locale: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub request: Option<Request>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct PriceTiers {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub description: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub symbol: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct XwaWhatsappGetPricingTiers {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub price_tiers: Option<Vec<PriceTiers>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa_whatsapp_get_pricing_tiers: Option<XwaWhatsappGetPricingTiers>,
    }
}

/// `WAWebBizGetProfileShimlinksQuery` (query).
pub mod biz_get_profile_shimlinks {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebBizGetProfileShimlinksQuery";
    pub const DOC_ID: &str = "24491258413796282";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(rename = "bizJid")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub biz_jid: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct XwaWhatsappSmbGetProfileLinkshims {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub shimmed_website_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub website: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa_whatsapp_smb_get_profile_linkshims: Option<Vec<XwaWhatsappSmbGetProfileLinkshims>>,
    }
}

/// `WAWebBizGraphQLRefreshCartJobQuery` (query).
pub mod biz_graph_ql_refresh_cart {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebBizGraphQLRefreshCartJobQuery";
    pub const DOC_ID: &str = "WAWebBizGraphQLRefreshCartJobQuery";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub request: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct PriceDetails {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub currency: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub price_status: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub subtotal_amount: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub total_amount: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ImporterAddress {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub city: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub country_code: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub postal_code: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub region: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub street1: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub street2: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ComplianceInfo {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub country_code_origin: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub importer_address: Option<ImporterAddress>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub importer_name: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct OriginalDimensions {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub height: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub width: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Images {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub original_dimensions: Option<OriginalDimensions>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub request_image_url: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Videos {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub original_video_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub thumbnail_url: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Media {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub images: Option<Vec<Images>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub videos: Option<Vec<Videos>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct SalePrice {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub end_date: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub price: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub start_date: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct StatusInfo {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub can_appeal: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub commerce_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub reject_reason: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Options {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Listing {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_available: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub options: Option<Vec<Options>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub product_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Availability {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub listing: Option<Vec<Listing>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ListingDetails {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub description: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub lowest_price: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub multi_price: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ThumbnailMedia {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub original_dimensions: Option<OriginalDimensions>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub original_image_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub request_image_url: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Options2 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub thumbnail_media: Option<ThumbnailMedia>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Types {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub options: Option<Vec<Options2>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct VariantProperties {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct VariantInfo {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub availability: Option<Availability>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub listing_details: Option<ListingDetails>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub types: Option<Vec<Types>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub variant_properties: Option<Vec<VariantProperties>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Products {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub availability: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub belongs_to: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub compliance_category: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub compliance_info: Option<ComplianceInfo>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub currency: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub description: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub image_fetch_status: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_hidden: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub max_available: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub media: Option<Media>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub price: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub product_availability: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub retailer_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sale_price: Option<SalePrice>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status_info: Option<StatusInfo>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub variant_info: Option<VariantInfo>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Cart {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub price_details: Option<PriceDetails>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub products: Option<Vec<Products>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct XwaCheckoutRefreshCart {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub cart: Option<Cart>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa_checkout_refresh_cart: Option<XwaCheckoutRefreshCart>,
    }
}

/// `WAWebBizProfileAddressAutocompleteQuery` (query).
pub mod biz_profile_address_autocomplete {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebBizProfileAddressAutocompleteQuery";
    pub const DOC_ID: &str = "34963438739971331";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Input {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub center: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub query: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub use_case_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<Input>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Address {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub city: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub country: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub postalcode: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub stateprovince: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub streetaddress: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Location {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub latitude: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub longitude: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Items {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub address: Option<Address>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub location: Option<Location>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub title: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct WhatsappMapsTypeahead {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub items: Option<Vec<Items>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub whatsapp_maps_typeahead: Option<WhatsappMapsTypeahead>,
    }
}

/// `WAWebBizQueryOrderJobQuery` (query).
pub mod biz_query_order {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebBizQueryOrderJobQuery";
    pub const DOC_ID: &str = "26593811266898374";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ImageDimensions {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub height: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub width: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Token {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sensitive_string_value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Order {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub direct_connection_encrypted_info: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub image_dimensions: Option<ImageDimensions>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub jid: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub token: Option<Token>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Request {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub order: Option<Order>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub request: Option<Request>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct PriceDetails {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub currency: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub subtotal_amount: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub total_amount: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Images {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub request_image_url: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Media {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub images: Option<Vec<Images>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct VariantProperties {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct VariantInfo {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub variant_properties: Option<Vec<VariantProperties>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Products {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub currency: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub media: Option<Media>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub price: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub quantity: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub variant_info: Option<VariantInfo>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Order2 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub creation_time_stamp: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub price_details: Option<PriceDetails>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub products: Option<Vec<Products>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct XwaCheckoutGetOrderInfo {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub order: Option<Order2>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa_checkout_get_order_info: Option<XwaCheckoutGetOrderInfo>,
    }
}

/// `WAWebBizSetMerchantComplianceMutation` (mutation).
pub mod biz_set_merchant_compliance {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebBizSetMerchantComplianceMutation";
    pub const DOC_ID: &str = "25188352884120072";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct CustomerCareDetails {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub email: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub landline_number: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub mobile_number: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct GrievanceOfficerDetails {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub email: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub landline_number: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub mobile_number: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct MerchantInfo {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub customer_care_details: Option<CustomerCareDetails>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub entity_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub entity_type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub entity_type_custom: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub grievance_officer_details: Option<GrievanceOfficerDetails>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_registered: Option<bool>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct XfbWhatsappBizMerchantSetComplianceInfo {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub merchant_info: Option<MerchantInfo>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xfb_whatsapp_biz_merchant_set_compliance_info:
            Option<XfbWhatsappBizMerchantSetComplianceInfo>,
    }
}

/// `WAWebMexCachedTokenJobMutation` (mutation).
pub mod cached_token {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexCachedTokenJobMutation";
    pub const DOC_ID: &str = "27013462064904056";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Input {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub client_pub_key: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub request_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<Input>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct EncryptedAccessTokens {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub algorithm: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub data: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub key: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub nonce: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub tag: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2EntTradeCanonicalNonceForAccessTokens {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub encrypted_access_tokens: Option<EncryptedAccessTokens>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_ent_trade_canonical_nonce_for_access_tokens:
            Option<Xwa2EntTradeCanonicalNonceForAccessTokens>,
    }
}

/// `WAWebCanonicalUserValidQuery` (query).
pub mod canonical_user_valid {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebCanonicalUserValidQuery";
    pub const DOC_ID: &str = "25995999653397511";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct XwaCanonicalUserValid {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub success: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa_canonical_user_valid: Option<XwaCanonicalUserValid>,
    }
}

/// `WAWebMexChangeNewsletterOwnerJobMutation` (mutation).
pub mod change_newsletter_owner {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexChangeNewsletterOwnerJobMutation";
    pub const DOC_ID: &str = "9546742745432473";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub newsletter_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub user_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2NewsletterChangeOwner {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_newsletter_change_owner: Option<Xwa2NewsletterChangeOwner>,
    }
}

/// `WAWebConsumerFetchQuickPromotionsQuery` (query).
pub mod consumer_fetch_quick_promotions {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebConsumerFetchQuickPromotionsQuery";
    pub const DOC_ID: &str = "35462584533386409";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct WaSmbTriggerContext {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub app_version: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub country: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_from_wa_smb: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub locale: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct TriggerContext {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub wa_smb_trigger_context: Option<WaSmbTriggerContext>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub nux_ids: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub trigger_context: Option<TriggerContext>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct WaBannerBackgroundColor {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub dark_mode_background_color: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub dark_mode_highlight_color: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub light_mode_background_color: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub light_mode_highlight_color: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ContentAttributes {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub wa_banner_background_color: Option<WaBannerBackgroundColor>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub wa_eligible_duration_after_impression_in_seconds: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub wa_primary_cta_alternative_url: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Filters {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Clauses {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub clause_type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub filters: Option<Vec<Filters>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Clauses2 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub clause_type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub clauses: Option<Vec<Clauses>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub filters: Option<Vec<Filters>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Clauses3 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub clause_type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub clauses: Option<Vec<Clauses2>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub filters: Option<Vec<Filters>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Clauses4 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub clause_type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub clauses: Option<Vec<Clauses3>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub filters: Option<Vec<Filters>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Clauses5 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub clause_type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub clauses: Option<Vec<Clauses4>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub filters: Option<Vec<Filters>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Clauses6 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub clause_type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub clauses: Option<Vec<Clauses5>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub filters: Option<Vec<Filters>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Clauses7 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub clause_type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub clauses: Option<Vec<Clauses6>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub filters: Option<Vec<Filters>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ContextualFiltersForWaDoNotUse {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub clause_type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub clauses: Option<Vec<Clauses7>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub filters: Option<Vec<Filters>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Content {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub text: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct DismissAction {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub limit: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Title {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub text: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct PrimaryAction {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub limit: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub title: Option<Title>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub url: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct WaDarkModeMediaDetails {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub jpeg_thumbnail: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct WaLightModeMediaDetails {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub jpeg_thumbnail: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Creatives {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub accessibility_text_for_image: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub content: Option<Content>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub dismiss_action: Option<DismissAction>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_dismissible: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub primary_action: Option<PrimaryAction>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub title: Option<Title>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub wa_dark_mode_media_details: Option<WaDarkModeMediaDetails>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub wa_light_mode_media_details: Option<WaLightModeMediaDetails>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct WaQpContentAttributesDoNotUse {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Node {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub ab_prop_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub client_side_dry_run: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub content_attributes: Option<ContentAttributes>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub contextual_filters_for_wa_do_not_use: Option<ContextualFiltersForWaDoNotUse>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub creatives: Option<Vec<Creatives>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub encrypted_logging_data: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_server_force_pass: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub max_impressions: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub promotion_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub surface_delay_in_seconds: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub wa_qp_content_attributes_do_not_use: Option<Vec<WaQpContentAttributesDoNotUse>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct TimeRange {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub end: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub start: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Edges {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub client_ttl_seconds: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_holdout: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub log_eligibility_waterfall: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub node: Option<Node>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub priority: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub time_range: Option<TimeRange>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct EligiblePromotions {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub edges: Option<Vec<Edges>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct QuickPromotionMultiverseBatchFetchRoot {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub eligible_promotions: Option<EligiblePromotions>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub surface_nux_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub quick_promotion_multiverse_batch_fetch_root:
            Option<Vec<QuickPromotionMultiverseBatchFetchRoot>>,
    }
}

/// `WAWebConsumerQuickPromotionActionGraphQLMutation` (mutation).
pub mod consumer_quick_promotion_action_graph_ql {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebConsumerQuickPromotionActionGraphQLMutation";
    pub const DOC_ID: &str = "25690382143972563";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct WaConsumerQuickPromotionLogEvent {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub client_mutation_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub wa_consumer_quick_promotion_log_event: Option<WaConsumerQuickPromotionLogEvent>,
    }
}

/// `WAWebMexCreateInviteCodeJobMutation` (mutation).
pub mod create_invite_code {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexCreateInviteCodeJobMutation";
    pub const DOC_ID: &str = "26155584267463745";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Input {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub entry_point: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub receiver: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub server_send_sms: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<Input>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2GrowthCreateInviteCode {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub code: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_growth_create_invite_code: Option<Xwa2GrowthCreateInviteCode>,
    }
}

/// `WAWebCreateMarketingCampaignActionMutation` (mutation).
pub mod create_marketing_campaign_action {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebCreateMarketingCampaignActionMutation";
    pub const DOC_ID: &str = "26304826652483067";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct WhatsappMarketingMessagesCreate {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub ad_campaign_group_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub ad_campaign_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub ad_creative_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub ad_group_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub ad_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub campaign_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub lifetime_budget: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub start_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub whatsapp_marketing_messages_create: Option<WhatsappMarketingMessagesCreate>,
    }
}

/// `WAWebMexCreateNewsletterJobMutation` (mutation).
pub mod create_newsletter {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexCreateNewsletterJobMutation";
    pub const DOC_ID: &str = "25149874324715067";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Input {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub description: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub picture: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<Input>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct State {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub r#type: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Description {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub update_time: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Name {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub update_time: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Picture {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub direct_path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub r#type: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Preview {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub direct_path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub r#type: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ThreadMetadata {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub creation_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub description: Option<Description>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub handle: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub invite: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<Name>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub picture: Option<Picture>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub preview: Option<Preview>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub subscribers_count: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub verification: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Settings {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub r#type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ViewerMetadata {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub role: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub settings: Option<Vec<Settings>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2NewsletterCreate {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub state: Option<State>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub thread_metadata: Option<ThreadMetadata>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub viewer_metadata: Option<ViewerMetadata>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_newsletter_create: Option<Xwa2NewsletterCreate>,
    }
}

/// `WAWebMexCreateNewsletterAdminInviteJobMutation` (mutation).
pub mod create_newsletter_admin_invite {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexCreateNewsletterAdminInviteJobMutation";
    pub const DOC_ID: &str = "9387141988078609";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub newsletter_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub user_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2NewsletterAdminInviteCreate {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub invite_expiration_time: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_newsletter_admin_invite_create: Option<Xwa2NewsletterAdminInviteCreate>,
    }
}

/// `WAWebMexCreateReportAppealJobMutation` (mutation).
pub mod create_report_appeal {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexCreateReportAppealJobMutation";
    pub const DOC_ID: &str = "27283301737925761";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub reason: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub report_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Appeal {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub appeal_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub appeal_reason: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub creation_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub report_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub state: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct QuestionData {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub server_msg_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ReportedContentData {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub notify_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub question_data: Option<QuestionData>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub server_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub server_msg_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub server_response_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2CreateChannelReportAppealV2 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub appeal: Option<Appeal>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub channel_jid: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub channel_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub creation_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub last_update_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub report_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub reported_content_data: Option<ReportedContentData>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_create_channel_report_appeal_v2: Option<Xwa2CreateChannelReportAppealV2>,
    }
}

/// `WAWebCreateWhatsAppAdsIdentityMutation` (mutation).
pub mod create_whats_app_ads_identity {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebCreateWhatsAppAdsIdentityMutation";
    pub const DOC_ID: &str = "24393949203623093";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Code {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sensitive_string_value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct PhoneNumber {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sensitive_string_value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub code: Option<Code>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub phone_number: Option<PhoneNumber>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct CreateOrUpdateWhatsappAdsIdentity {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub create_or_update_whatsapp_ads_identity: Option<CreateOrUpdateWhatsappAdsIdentity>,
    }
}

/// `WAWebCustomLabel3pdEventQuery` (query).
pub mod custom_label3pd_event {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebCustomLabel3pdEventQuery";
    pub const DOC_ID: &str = "24247439618185103";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub custom_labels: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub expt_group: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct XwaGet3pdEvent {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub ctwa_3pd_conversion_metadata: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub ctwa_3pd_conversion_subtype: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub ctwa_3pd_conversion_type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub custom_label: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa_get_3pd_event: Option<Vec<XwaGet3pdEvent>>,
    }
}

/// `WAWebMexDeleteNewsletterJobMutation` (mutation).
pub mod delete_newsletter {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexDeleteNewsletterJobMutation";
    pub const DOC_ID: &str = "30062808666639665";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub newsletter_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct State {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub r#type: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2NewsletterDeleteV2 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub state: Option<State>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_newsletter_delete_v2: Option<Xwa2NewsletterDeleteV2>,
    }
}

/// `WAWebMexDemoteNewsletterAdminJobMutation` (mutation).
pub mod demote_newsletter_admin {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexDemoteNewsletterAdminJobMutation";
    pub const DOC_ID: &str = "9880997548630971";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub newsletter_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub user_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2NewsletterAdminDemote {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_newsletter_admin_demote: Option<Xwa2NewsletterAdminDemote>,
    }
}

/// `WAWebEditBizProfileMutation` (mutation).
pub mod edit_biz_profile {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebEditBizProfileMutation";
    pub const DOC_ID: &str = "26652989367627867";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub lid: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub edit_wa_web_biz_profile: Option<String>,
    }
}

/// `WAWebExternalCtxAuthoriseWAChatMutation` (mutation).
pub mod external_ctx_authorise_wa_chat {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebExternalCtxAuthoriseWAChatMutation";
    pub const DOC_ID: &str = "9790465291023292";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct XwaExternalCtxAuthoriseWaChat {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub partner_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub success: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa_external_ctx_authorise_wa_chat: Option<XwaExternalCtxAuthoriseWaChat>,
    }
}

/// `WAWebMexFetchAboutStatusJobQuery` (query).
pub mod fetch_about_status {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexFetchAboutStatusJobQuery";
    pub const DOC_ID: &str = "24535500086059408";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct User {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub user_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub user: Option<User>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Updates {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub text: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2UsersUpdatesSince {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub updates: Option<Vec<Updates>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_users_updates_since: Option<Vec<Xwa2UsersUpdatesSince>>,
    }
}

/// `WAWebFetchAdEntryPointsConfigurationQuery` (query).
pub mod fetch_ad_entry_points_configuration {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebFetchAdEntryPointsConfigurationQuery";
    pub const DOC_ID: &str = "9656368401073090";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct CtwaClientEntryPointEntitlement {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub entry_point_or_experience: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub should_show: Option<bool>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub ctwa_client_entry_point_entitlement: Option<Vec<CtwaClientEntryPointEntitlement>>,
    }
}

/// `WAWebFetchAdEntryPointsConfigurationM1Query` (query).
pub mod fetch_ad_entry_points_configuration_m1 {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebFetchAdEntryPointsConfigurationM1Query";
    pub const DOC_ID: &str = "9737776042983782";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct CtwaClientEntryPointEntitlement {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub content: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub entry_point_or_experience: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub should_show: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sub_content: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub ctwa_client_entry_point_entitlement: Option<Vec<CtwaClientEntryPointEntitlement>>,
    }
}

/// `WAWebMexFetchAllNewslettersMetadataJobQuery` (query).
pub mod fetch_all_newsletters_metadata {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexFetchAllNewslettersMetadataJobQuery";
    pub const DOC_ID: &str = "25399611239711790";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub fetch_status_metadata: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub fetch_wamo_sub: Option<bool>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct State {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub r#type: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct StatusMetadata {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub last_status_sent_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub last_status_server_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Description {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub update_time: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Name {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub update_time: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Picture {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub direct_path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub r#type: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Preview {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub direct_path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub r#type: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ReactionCodes {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Settings {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub reaction_codes: Option<ReactionCodes>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct WamoSub {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub plan_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ThreadMetadata {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub creation_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub description: Option<Description>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub handle: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub invite: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<Name>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub picture: Option<Picture>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub preview: Option<Preview>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub settings: Option<Settings>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub verification: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub wamo_sub: Option<WamoSub>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Settings2 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub r#type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ViewerMetadata {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub role: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub settings: Option<Vec<Settings2>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub wamo_sub_status: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2NewsletterSubscribed {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub state: Option<State>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status_metadata: Option<StatusMetadata>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub thread_metadata: Option<ThreadMetadata>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub viewer_metadata: Option<ViewerMetadata>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_newsletter_subscribed: Option<Vec<Xwa2NewsletterSubscribed>>,
    }
}

/// `WAWebMexFetchAllSubgroupsJobQuery` (query).
pub mod fetch_all_subgroups {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexFetchAllSubgroupsJobQuery";
    pub const DOC_ID: &str = "9935467776504344";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub group_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub query_context: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sub_group_hint_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Subject {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub creation_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct DefaultSubGroup {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub subject: Option<Subject>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct MembershipApprovalRequests {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub total_count: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Properties {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub general_chat: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub hidden_group: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub membership_approval_mode_enabled: Option<bool>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Node {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub membership_approval_requests: Option<MembershipApprovalRequests>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub properties: Option<Properties>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub subject: Option<Subject>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Edges {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub node: Option<Node>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct SubGroups {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub edges: Option<Vec<Edges>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2GroupQueryById {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub default_sub_group: Option<DefaultSubGroup>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sub_groups: Option<SubGroups>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_group_query_by_id: Option<Xwa2GroupQueryById>,
    }
}

/// `WAWebMexFetchBotCertificateRevocationListQuery` (query).
pub mod fetch_bot_certificate_revocation_list {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexFetchBotCertificateRevocationListQuery";
    pub const DOC_ID: &str = "35807917542188393";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub crl_name: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2FetchFeaturePkiCrl {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub crl: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub next_update: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_fetch_feature_pki_crl: Option<Xwa2FetchFeaturePkiCrl>,
    }
}

/// `WAWebFetchBotProfilesGQLQuery` (query).
pub mod fetch_bot_profiles_gql {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebFetchBotProfilesGQLQuery";
    pub const DOC_ID: &str = "26368585139502858";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub ids: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Creator {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub profile_uri: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct LatestPublishedVersionForViewer {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub description: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub icebreaker_prompt_list: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub posing_as_professional: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct XfbFetchGenaiPersonas {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub creator: Option<Creator>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_meta_created: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub jid: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub latest_published_version_for_viewer: Option<LatestPublishedVersionForViewer>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xfb_fetch_genai_personas: Option<Vec<XfbFetchGenaiPersonas>>,
    }
}

/// `WAWebFetchDynamicAIModesQuery` (query).
pub mod fetch_dynamic_ai_modes {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebFetchDynamicAIModesQuery";
    pub const DOC_ID: &str = "25335662402775799";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct XfbMetaAiModes {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_experimental: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub mode_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub subtitle: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub title: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub r#type: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xfb_meta_ai_modes: Option<Vec<XfbMetaAiModes>>,
    }
}

/// `WAWebMexFetchGroupInfoJobQuery` (query).
pub mod fetch_group_info {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexFetchGroupInfoJobQuery";
    pub const DOC_ID: &str = "26570027442651356";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub include_username: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub participants_phash: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub query_context: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct UsernameInfo {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub username: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Creator {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub lid: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub pn: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub username_info: Option<UsernameInfo>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Description {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub creation_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub creator: Option<Creator>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Node {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub display_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub lid: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub pn: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub username_info: Option<UsernameInfo>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Edges {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub node: Option<Node>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub role: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Participants {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub edges: Option<Vec<Edges>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub participants_phash_match: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Ephemeral {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub expiration_time_in_sec: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct GrowthLocked2 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub locked: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct LidMigrationState {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub addressing_mode: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct LimitSharing {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub limit_sharing_enabled: Option<bool>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Properties {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub allow_non_admin_sub_group_creation: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub announcement: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub appeal_status: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub appeal_update_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub auto_add_disabled: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub capi: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub closed_by_membership_approval_mode: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub ephemeral: Option<Ephemeral>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub general_chat: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub group_safety_check: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub growth_locked2: Option<GrowthLocked2>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub hidden_group: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub lid_migration_state: Option<LidMigrationState>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub limit_sharing: Option<LimitSharing>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub locked: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub member_add_mode: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub member_link_mode: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub member_share_group_history_mode: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub membership_approval_mode_enabled: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub parent_group_jid: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub support: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Subject {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub creation_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub creator: Option<Creator>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2GroupQueryById {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub creation_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub creator: Option<Creator>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub description: Option<Description>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub membership_approval_request: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub missing_participant_identification: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub participants: Option<Participants>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub properties: Option<Properties>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub state: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub subject: Option<Subject>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub total_participants_count: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_group_query_by_id: Option<Xwa2GroupQueryById>,
    }
}

/// `WAWebMexFetchGroupInfoIncludBotsJobQuery` (query).
pub mod fetch_group_info_includ_bots {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexFetchGroupInfoIncludBotsJobQuery";
    pub const DOC_ID: &str = "26412593755077157";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub include_username: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub participants_phash: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub query_context: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct UsernameInfo {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub username: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Creator {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub lid: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub pn: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub username_info: Option<UsernameInfo>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Description {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub creation_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub creator: Option<Creator>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Participant {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub display_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub jid: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub lid: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub pn: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub username_info: Option<UsernameInfo>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Edges {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub participant: Option<Participant>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub role: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Participants {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub edges: Option<Vec<Edges>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub participants_phash_match: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Ephemeral {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub expiration_time_in_sec: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct GrowthLocked2 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub locked: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct LidMigrationState {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub addressing_mode: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct LimitSharing {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub limit_sharing_enabled: Option<bool>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Properties {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub allow_admin_reports: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub allow_non_admin_sub_group_creation: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub announcement: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub appeal_status: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub appeal_update_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub auto_add_disabled: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub capi: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub closed_by_membership_approval_mode: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub ephemeral: Option<Ephemeral>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub general_chat: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub group_safety_check: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub growth_locked2: Option<GrowthLocked2>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub hidden_group: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub lid_migration_state: Option<LidMigrationState>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub limit_sharing: Option<LimitSharing>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub locked: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub member_add_mode: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub member_link_mode: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub member_share_group_history_mode: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub membership_approval_mode_enabled: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub parent_group_jid: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub support: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Subject {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub creation_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub creator: Option<Creator>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2GroupQueryById {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub creation_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub creator: Option<Creator>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub description: Option<Description>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub membership_approval_request: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub missing_participant_identification: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub participants: Option<Participants>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub properties: Option<Properties>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub state: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub subject: Option<Subject>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub total_participants_count: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_group_query_by_id: Option<Xwa2GroupQueryById>,
    }
}

/// `WAWebMexFetchGroupInviteCodeJobQuery` (query).
pub mod fetch_group_invite_code {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexFetchGroupInviteCodeJobQuery";
    pub const DOC_ID: &str = "29247029834912157";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub query_context: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2GroupQueryById {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub invite_code: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_group_query_by_id: Option<Xwa2GroupQueryById>,
    }
}

/// `WAWebMexFetchGroupIsInternalJobQuery` (query).
pub mod fetch_group_is_internal {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexFetchGroupIsInternalJobQuery";
    pub const DOC_ID: &str = "34119218944390847";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Properties {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub internal: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2GroupQueryById {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub properties: Option<Properties>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_group_query_by_id: Option<Xwa2GroupQueryById>,
    }
}

/// `WAWebMexFetchIntegritySignalsQuery` (query).
pub mod fetch_integrity_signals {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexFetchIntegritySignalsQuery";
    pub const DOC_ID: &str = "26438847999065394";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct IntegritySignals {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub use_case: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct QueryInput {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub integrity_signals: Option<IntegritySignals>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub jid: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Telemetry {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub context: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Input {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub query_input: Option<Vec<QueryInput>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub telemetry: Option<Telemetry>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<Input>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct IntegritySignalsInfo {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_new_account: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_suspicious_start_chat: Option<bool>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2FetchWaUsers {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub integrity_signals_info: Option<IntegritySignalsInfo>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_fetch_wa_users: Option<Vec<Xwa2FetchWaUsers>>,
    }
}

/// `WAWebFetchNativeAdsMvpEligibilityQuery` (query).
pub mod fetch_native_ads_mvp_eligibility {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebFetchNativeAdsMvpEligibilityQuery";
    pub const DOC_ID: &str = "34778300218423824";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub phone_number: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct WaSmbNativeAdsWebInfo {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_page_asset_linked: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_pageless_asset_linked: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub lifetime_native_ctwa_advertiser: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub webclient_l90_ad_creator: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub wa_smb_native_ads_web_info: Option<WaSmbNativeAdsWebInfo>,
    }
}

/// `WAWebMexFetchNewChatMessageCappingInfoJobQuery` (query).
pub mod fetch_new_chat_message_capping_info {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexFetchNewChatMessageCappingInfoJobQuery";
    pub const DOC_ID: &str = "24503548349331633";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Input {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub r#type: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<Input>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2MessageCappingInfo {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub capping_status: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub cycle_end_timestamp: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub cycle_start_timestamp: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub mv_status: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub ote_status: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub server_sent_timestamp: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub total_quota: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub used_quota: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_message_capping_info: Option<Xwa2MessageCappingInfo>,
    }
}

/// `WAWebMexFetchNewsletterJobQuery` (query).
pub mod fetch_newsletter {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexFetchNewsletterJobQuery";
    pub const DOC_ID: &str = "35452404184358876";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Input {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub key: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub r#type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub view_role: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub fetch_creation_time: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub fetch_full_image: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub fetch_status_metadata: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub fetch_viewer_metadata: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub fetch_wamo_sub: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<Input>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct State {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub r#type: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct StatusMetadata {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub last_status_sent_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub last_status_server_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Description {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub update_time: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Name {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub update_time: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Picture {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub direct_path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub r#type: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Preview {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub direct_path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub r#type: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ReactionCodes {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Settings {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub reaction_codes: Option<ReactionCodes>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct WamoSub {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub plan_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ThreadMetadata {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub creation_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub description: Option<Description>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub handle: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub invite: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<Name>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub picture: Option<Picture>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub preview: Option<Preview>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub settings: Option<Settings>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub subscribers_count: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub verification: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub wamo_sub: Option<WamoSub>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Settings2 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub r#type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ViewerMetadata {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub role: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub settings: Option<Vec<Settings2>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub wamo_sub_status: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2Newsletter {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub state: Option<State>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status_metadata: Option<StatusMetadata>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub thread_metadata: Option<ThreadMetadata>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub viewer_metadata: Option<ViewerMetadata>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_newsletter: Option<Xwa2Newsletter>,
    }
}

/// `WAWebMexFetchNewsletterAdminCapabilitiesJobQuery` (query).
pub mod fetch_newsletter_admin_capabilities {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexFetchNewsletterAdminCapabilitiesJobQuery";
    pub const DOC_ID: &str = "9801384413216421";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub newsletter_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2NewsletterAdmin {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub capabilities: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_newsletter_admin: Option<Xwa2NewsletterAdmin>,
    }
}

/// `WAWebMexFetchNewsletterAdminInfoJobQuery` (query).
pub mod fetch_newsletter_admin_info {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexFetchNewsletterAdminInfoJobQuery";
    pub const DOC_ID: &str = "26278439461859188";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub newsletter_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Picture {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub direct_path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct AdminProfile {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub picture: Option<Picture>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct AdminSettings {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub admin_profiles_enabled: Option<bool>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2NewsletterAdmin {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub admin_count: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub admin_profile: Option<AdminProfile>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub admin_settings: Option<AdminSettings>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_newsletter_admin: Option<Xwa2NewsletterAdmin>,
    }
}

/// `WAWebMexFetchNewsletterDehydratedJobQuery` (query).
pub mod fetch_newsletter_dehydrated {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexFetchNewsletterDehydratedJobQuery";
    pub const DOC_ID: &str = "30328461880085868";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Input {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub key: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub r#type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub view_role: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub fetch_wamo_sub: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<Input>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ReactionCodes {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Settings {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub reaction_codes: Option<ReactionCodes>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct WamoSub {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub plan_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ThreadMetadata {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub settings: Option<Settings>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub subscribers_count: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub verification: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub wamo_sub: Option<WamoSub>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ViewerMetadata {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub wamo_sub_status: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2Newsletter {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub thread_metadata: Option<ThreadMetadata>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub viewer_metadata: Option<ViewerMetadata>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_newsletter: Option<Xwa2Newsletter>,
    }
}

/// `WAWebMexFetchNewsletterDirectoryCategoriesPreviewJobQuery` (query).
pub mod fetch_newsletter_directory_categories_preview {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexFetchNewsletterDirectoryCategoriesPreviewJobQuery";
    pub const DOC_ID: &str = "35266481849605779";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Input {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub categories: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub country_code: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub per_category_limit: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub fetch_status_metadata: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<Input>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct StatusMetadata {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub last_status_sent_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub last_status_server_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Description {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub update_time: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Name {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub update_time: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Picture {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub direct_path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub r#type: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ThreadMetadata {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub creation_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub description: Option<Description>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub handle: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub invite: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<Name>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub picture: Option<Picture>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub subscribers_count: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub verification: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Newsletters {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status_metadata: Option<StatusMetadata>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub thread_metadata: Option<ThreadMetadata>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Result {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub category: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub category_title: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub newsletters: Option<Vec<Newsletters>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2NewslettersDirectoryCategoryPreview {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub result: Option<Vec<Result>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_newsletters_directory_category_preview:
            Option<Xwa2NewslettersDirectoryCategoryPreview>,
    }
}

/// `WAWebMexFetchNewsletterDirectoryListJobQuery` (query).
pub mod fetch_newsletter_directory_list {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexFetchNewsletterDirectoryListJobQuery";
    pub const DOC_ID: &str = "26125047313831973";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Filters {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub categories: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub country_codes: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Input {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub filters: Option<Filters>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub limit: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub start_cursor: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub view: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub fetch_status_metadata: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<Input>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct PageInfo {
        #[serde(rename = "endCursor")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub end_cursor: Option<String>,
        #[serde(rename = "hasNextPage")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub has_next_page: Option<String>,
        #[serde(rename = "hasPreviousPage")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub has_previous_page: Option<String>,
        #[serde(rename = "startCursor")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub start_cursor: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct StatusMetadata {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub last_status_sent_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub last_status_server_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Description {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub update_time: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Name {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub update_time: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Picture {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub direct_path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub r#type: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ThreadMetadata {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub creation_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub description: Option<Description>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub handle: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub invite: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<Name>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub picture: Option<Picture>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub subscribers_count: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub verification: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Result {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status_metadata: Option<StatusMetadata>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub thread_metadata: Option<ThreadMetadata>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2NewslettersDirectoryList {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub page_info: Option<PageInfo>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub result: Option<Vec<Result>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_newsletters_directory_list: Option<Xwa2NewslettersDirectoryList>,
    }
}

/// `WAWebMexFetchNewsletterDirectorySearchResultsJobQuery` (query).
pub mod fetch_newsletter_directory_search_results {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexFetchNewsletterDirectorySearchResultsJobQuery";
    pub const DOC_ID: &str = "26301059626252132";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Input {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub categories: Option<Vec<String>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub limit: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub search_text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub start_cursor: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub fetch_status_metadata: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<Input>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct PageInfo {
        #[serde(rename = "endCursor")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub end_cursor: Option<String>,
        #[serde(rename = "hasNextPage")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub has_next_page: Option<String>,
        #[serde(rename = "hasPreviousPage")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub has_previous_page: Option<String>,
        #[serde(rename = "startCursor")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub start_cursor: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct StatusMetadata {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub last_status_sent_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub last_status_server_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Description {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub update_time: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Name {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub update_time: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Picture {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub direct_path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub r#type: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ThreadMetadata {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub creation_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub description: Option<Description>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub handle: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub invite: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<Name>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub picture: Option<Picture>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub subscribers_count: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub verification: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Result {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status_metadata: Option<StatusMetadata>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub thread_metadata: Option<ThreadMetadata>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2NewslettersDirectorySearch {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub page_info: Option<PageInfo>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub result: Option<Vec<Result>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_newsletters_directory_search: Option<Xwa2NewslettersDirectorySearch>,
    }
}

/// `WAWebMexFetchNewsletterEnforcementsJobQuery` (query).
pub mod fetch_newsletter_enforcements {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexFetchNewsletterEnforcementsJobQuery";
    pub const DOC_ID: &str = "26807357005541676";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub locale: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub newsletter_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct AppealExtraData {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub appeal_form_url: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct EnforcementTargetData {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub server_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub server_msg_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct EnforcingEntityData {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct IpViolationReportData {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub appeal_form_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub report_fbid: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub reporter_email: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub reporter_name: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct EnforcementExtraData {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub appeal_extra_data: Option<AppealExtraData>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub enforcement_origin_legal_basis: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub enforcement_origin_workflow: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub enforcement_target_data: Option<EnforcementTargetData>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub enforcing_entity_data: Option<EnforcingEntityData>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub ip_violation_report_data: Option<IpViolationReportData>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct EnforcementPolicyInformation {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub admin_disclaimer: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub explanation: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub headline: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub overview: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub subtitle: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct BaseEnforcementData {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub appeal_creation_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub appeal_state: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub enforcement_creation_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub enforcement_extra_data: Option<EnforcementExtraData>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub enforcement_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub enforcement_policy_information: Option<EnforcementPolicyInformation>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub enforcement_source: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub enforcement_violation_category: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Geosuspensions {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub base_enforcement_data: Option<BaseEnforcementData>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub country_codes: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct EnforcementExtraData2 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub ip_violation_report_data: Option<IpViolationReportData>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ProfilePictureDeletions {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub appeal_creation_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub appeal_state: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub enforcement_creation_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub enforcement_extra_data: Option<EnforcementExtraData2>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub enforcement_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub enforcement_policy_information: Option<EnforcementPolicyInformation>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub enforcement_source: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub enforcement_violation_category: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct EnforcementExtraData3 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub appeal_extra_data: Option<AppealExtraData>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub enforcement_target_data: Option<EnforcementTargetData>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub ip_violation_report_data: Option<IpViolationReportData>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Suspensions {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub appeal_creation_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub appeal_state: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub enforcement_creation_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub enforcement_extra_data: Option<EnforcementExtraData3>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub enforcement_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub enforcement_policy_information: Option<EnforcementPolicyInformation>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub enforcement_source: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub enforcement_violation_category: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct BaseEnforcementData2 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub appeal_creation_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub appeal_state: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub enforcement_creation_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub enforcement_extra_data: Option<EnforcementExtraData2>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub enforcement_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub enforcement_policy_information: Option<EnforcementPolicyInformation>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub enforcement_source: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub enforcement_violation_category: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ContentData {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub server_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub server_msg_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ViolatingMessages {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub base_enforcement_data: Option<BaseEnforcementData2>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub content_data: Option<ContentData>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2ChannelEnforcements {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub geosuspensions: Option<Vec<Geosuspensions>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub profile_picture_deletions: Option<Vec<ProfilePictureDeletions>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub suspensions: Option<Vec<Suspensions>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub violating_messages: Option<Vec<ViolatingMessages>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_channel_enforcements: Option<Xwa2ChannelEnforcements>,
    }
}

/// `WAWebMexFetchNewsletterFollowersJobQuery` (query).
pub mod fetch_newsletter_followers {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexFetchNewsletterFollowersJobQuery";
    pub const DOC_ID: &str = "27472091235714801";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Input {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub count: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub newsletter_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<Input>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Picture {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub direct_path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct AdminProfile {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub picture: Option<Picture>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct UsernameInfo {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub username: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Node {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub display_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub pn: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub username_info: Option<UsernameInfo>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Edges {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub admin_profile: Option<AdminProfile>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub follow_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub node: Option<Node>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub role: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Followers {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub edges: Option<Vec<Edges>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2NewsletterFollowers {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub followers: Option<Followers>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_newsletter_followers: Option<Xwa2NewsletterFollowers>,
    }
}

/// `WAWebMexFetchNewsletterInsightsJobQuery` (query).
pub mod fetch_newsletter_insights {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexFetchNewsletterInsightsJobQuery";
    pub const DOC_ID: &str = "9853618868050977";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Input {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub metrics: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub newsletter_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<Input>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Values {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub country: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub role: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub timestamp: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Result {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub values: Option<Vec<Values>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct State {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub r#type: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2NewsletterAdminInsights {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub last_update_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub metrics_status: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub newsletter_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub result: Option<Vec<Result>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub state: Option<State>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_newsletter_admin_insights: Option<Xwa2NewsletterAdminInsights>,
    }
}

/// `WAWebMexFetchNewsletterIsDomainPreviewableJobQuery` (query).
pub mod fetch_newsletter_is_domain_previewable {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexFetchNewsletterIsDomainPreviewableJobQuery";
    pub const DOC_ID: &str = "9849510985088294";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub url_domains: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct UrlPreviews {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_previewable: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub url_domain: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2NewsletterMessageIntegrity {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub url_previews: Option<Vec<UrlPreviews>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_newsletter_message_integrity: Option<Xwa2NewsletterMessageIntegrity>,
    }
}

/// `WAWebMexFetchNewsletterMessageReactionSenderListJobQuery` (query).
pub mod fetch_newsletter_message_reaction_sender_list {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexFetchNewsletterMessageReactionSenderListJobQuery";
    pub const DOC_ID: &str = "29575462448733991";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Input {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub server_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<Input>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Node {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub profile_pic_direct_path: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Edges {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub node: Option<Node>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct SenderList {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub edges: Option<Vec<Edges>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Reactions {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub reaction_code: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sender_list: Option<SenderList>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2NewslettersReactionSenderList {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub reactions: Option<Vec<Reactions>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_newsletters_reaction_sender_list: Option<Xwa2NewslettersReactionSenderList>,
    }
}

/// `WAWebMexFetchNewsletterPendingInvitesJobQuery` (query).
pub mod fetch_newsletter_pending_invites {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexFetchNewsletterPendingInvitesJobQuery";
    pub const DOC_ID: &str = "9783111038412085";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub newsletter_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct User {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub pn: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct PendingAdminInvites {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub user: Option<User>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2NewsletterAdmin {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub pending_admin_invites: Option<Vec<PendingAdminInvites>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_newsletter_admin: Option<Xwa2NewsletterAdmin>,
    }
}

/// `WAWebMexFetchNewsletterPollVotersJobQuery` (query).
pub mod fetch_newsletter_poll_voters {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexFetchNewsletterPollVotersJobQuery";
    pub const DOC_ID: &str = "9407762219322536";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Input {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub limit: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub newsletter_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub server_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub vote_hash: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<Input>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Node {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Edges {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub action_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub node: Option<Node>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct VoterList {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub edges: Option<Vec<Edges>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Votes {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub vote_hash: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub voter_list: Option<VoterList>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct VoterList2 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub votes: Option<Vec<Votes>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub voter_list: Option<VoterList2>,
    }
}

/// `WAWebMexFetchNewsletterReportsJobQuery` (query).
pub mod fetch_newsletter_reports {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexFetchNewsletterReportsJobQuery";
    pub const DOC_ID: &str = "24241374008893508";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Appeal {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub appeal_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub appeal_reason: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub creation_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub report_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub state: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct QuestionData {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub server_msg_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ReportedContentData {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub notify_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub question_data: Option<QuestionData>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub server_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub server_msg_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub server_response_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ChannelsReports {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub appeal: Option<Appeal>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub channel_jid: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub channel_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub creation_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub last_update_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub report_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub reported_content_data: Option<ReportedContentData>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2ChannelsReports {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub channels_reports: Option<Vec<ChannelsReports>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_channels_reports: Option<Xwa2ChannelsReports>,
    }
}

/// `WAWebFetchOHAIKeyConfigJobQuery` (query).
pub mod fetch_ohai_key_config {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebFetchOHAIKeyConfigJobQuery";
    pub const DOC_ID: &str = "29366514836329275";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct OhaiConfigs {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub aead_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub expiration_date: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub kdf_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub kem_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub key_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub last_updated_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub public_key: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2OhaiConfigurations {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub ohai_configs: Option<Vec<OhaiConfigs>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_ohai_configurations: Option<Xwa2OhaiConfigurations>,
    }
}

/// `WAWebFetchOIDCStateQuery` (query).
pub mod fetch_oidc_state {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebFetchOIDCStateQuery";
    pub const DOC_ID: &str = "24622479247368194";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xfb_wa_biz_get_oidc_state: Option<String>,
    }
}

/// `WAWebMexFetchPlaintextLinkPreviewJobQuery` (query).
pub mod fetch_plaintext_link_preview {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexFetchPlaintextLinkPreviewJobQuery";
    pub const DOC_ID: &str = "9101130456653613";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Input {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub url: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<Input>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2NewsletterLinkPreview {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub description: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub direct_path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub hash: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub height: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub preview_type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub thumb_data: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub title: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub width: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_newsletter_link_preview: Option<Xwa2NewsletterLinkPreview>,
    }
}

/// `WAWebFetchQuickPromotionsQuery` (query).
pub mod fetch_quick_promotions {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebFetchQuickPromotionsQuery";
    pub const DOC_ID: &str = "27262639366727460";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct WaSmbTriggerContext {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub app_version: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub country: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_from_wa_smb: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub locale: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct TriggerContext {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub wa_smb_trigger_context: Option<WaSmbTriggerContext>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub nux_ids: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub trigger_context: Option<TriggerContext>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct WaBannerBackgroundColor {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub dark_mode_background_color: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub dark_mode_highlight_color: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub light_mode_background_color: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub light_mode_highlight_color: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ContentAttributes {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub wa_banner_background_color: Option<WaBannerBackgroundColor>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub wa_eligible_duration_after_impression_in_seconds: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub wa_primary_cta_alternative_url: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Parameters {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub key: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Filters {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub filter_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub filter_result: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub parameters: Option<Vec<Parameters>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub passes_if_client_not_supported: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Clauses {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub clause_type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub filters: Option<Vec<Filters>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Clauses2 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub clause_type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub clauses: Option<Vec<Clauses>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub filters: Option<Vec<Filters>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Clauses3 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub clause_type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub clauses: Option<Vec<Clauses2>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub filters: Option<Vec<Filters>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Clauses4 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub clause_type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub clauses: Option<Vec<Clauses3>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub filters: Option<Vec<Filters>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Clauses5 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub clause_type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub clauses: Option<Vec<Clauses4>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub filters: Option<Vec<Filters>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Clauses6 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub clause_type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub clauses: Option<Vec<Clauses5>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub filters: Option<Vec<Filters>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Clauses7 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub clause_type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub clauses: Option<Vec<Clauses6>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub filters: Option<Vec<Filters>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ContextualFiltersForWaDoNotUse {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub clause_type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub clauses: Option<Vec<Clauses7>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub filters: Option<Vec<Filters>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Content {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub text: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Title {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub text: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct PrimaryAction {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub title: Option<Title>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub url: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct WaDarkModeMediaDetails {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub jpeg_thumbnail: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct WaLightModeMediaDetails {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub jpeg_thumbnail: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Creatives {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub accessibility_text_for_image: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub content: Option<Content>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_dismissible: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub primary_action: Option<PrimaryAction>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub title: Option<Title>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub wa_dark_mode_media_details: Option<WaDarkModeMediaDetails>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub wa_light_mode_media_details: Option<WaLightModeMediaDetails>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct WaQpContentAttributesDoNotUse {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Node {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub ab_prop_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub client_side_dry_run: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub content_attributes: Option<ContentAttributes>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub contextual_filters_for_wa_do_not_use: Option<ContextualFiltersForWaDoNotUse>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub creatives: Option<Vec<Creatives>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub encrypted_logging_data: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_server_force_pass: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub promotion_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub surface_delay_in_seconds: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub wa_qp_content_attributes_do_not_use: Option<Vec<WaQpContentAttributesDoNotUse>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct TimeRange {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub end: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub start: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Edges {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub client_ttl_seconds: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_holdout: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub log_eligibility_waterfall: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub node: Option<Node>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub priority: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub time_range: Option<TimeRange>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct EligiblePromotions {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub edges: Option<Vec<Edges>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct QuickPromotionBatchFetchRoot {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub eligible_promotions: Option<EligiblePromotions>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub surface_nux_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub quick_promotion_batch_fetch_root: Option<Vec<QuickPromotionBatchFetchRoot>>,
    }
}

/// `WAWebMexFetchReachoutTimelockJobQuery` (query).
pub mod fetch_reachout_timelock {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexFetchReachoutTimelockJobQuery";
    pub const DOC_ID: &str = "23983697327930364";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2FetchAccountReachoutTimelock {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub enforcement_type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_active: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub time_enforcement_ends: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_fetch_account_reachout_timelock: Option<Xwa2FetchAccountReachoutTimelock>,
    }
}

/// `WAWebMexFetchRecommendedNewslettersJobQuery` (query).
pub mod fetch_recommended_newsletters {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexFetchRecommendedNewslettersJobQuery";
    pub const DOC_ID: &str = "25806748772361516";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Input {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub country_codes: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub limit: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub fetch_status_metadata: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<Input>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct PageInfo {
        #[serde(rename = "endCursor")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub end_cursor: Option<String>,
        #[serde(rename = "hasNextPage")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub has_next_page: Option<String>,
        #[serde(rename = "hasPreviousPage")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub has_previous_page: Option<String>,
        #[serde(rename = "startCursor")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub start_cursor: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct State {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub r#type: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct StatusMetadata {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub last_status_sent_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub last_status_server_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Description {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub update_time: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Name {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub update_time: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Preview {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub direct_path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub r#type: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ThreadMetadata {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub creation_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub description: Option<Description>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub handle: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub invite: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<Name>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub preview: Option<Preview>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub subscribers_count: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub verification: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Result {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub state: Option<State>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status_metadata: Option<StatusMetadata>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub thread_metadata: Option<ThreadMetadata>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2NewslettersRecommended {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub page_info: Option<PageInfo>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub result: Option<Vec<Result>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_newsletters_recommended: Option<Xwa2NewslettersRecommended>,
    }
}

/// `WAWebMexFetchSimilarNewslettersJobQuery` (query).
pub mod fetch_similar_newsletters {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexFetchSimilarNewslettersJobQuery";
    pub const DOC_ID: &str = "26217043484590756";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Input {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub country_codes: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub limit: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub newsletter_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub fetch_status_metadata: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<Input>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct State {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub r#type: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct StatusMetadata {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub last_status_server_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Name {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub update_time: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Picture {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub direct_path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub r#type: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ThreadMetadata {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<Name>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub picture: Option<Picture>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub verification: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Result {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub state: Option<State>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status_metadata: Option<StatusMetadata>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub thread_metadata: Option<ThreadMetadata>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2NewslettersSimilar {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub result: Option<Vec<Result>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_newsletters_similar: Option<Xwa2NewslettersSimilar>,
    }
}

/// `WAWebMexFetchSubgroupSuggestionsJobQuery` (query).
pub mod fetch_subgroup_suggestions {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexFetchSubgroupSuggestionsJobQuery";
    pub const DOC_ID: &str = "23972005349071865";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub group_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub query_context: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sub_group_hint_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Creator {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Description {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Subject {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Node {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub creation_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub creator: Option<Creator>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub description: Option<Description>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub hidden_group: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_existing_group: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub subject: Option<Subject>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub total_participants_count: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Edges {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub node: Option<Node>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct SubGroupSuggestions {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub edges: Option<Vec<Edges>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2GroupQueryById {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sub_group_suggestions: Option<SubGroupSuggestions>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_group_query_by_id: Option<Xwa2GroupQueryById>,
    }
}

/// `WAWebFetchSubscriptionEntryPointsQuery` (query).
pub mod fetch_subscription_entry_points {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebFetchSubscriptionEntryPointsQuery";
    pub const DOC_ID: &str = "9569660009784796";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct SubscriptionEntryPoints {
        #[serde(rename = "subscriptionType")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub subscription_type: Option<String>,
        #[serde(rename = "webEntryPointEligibility")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub web_entry_point_eligibility: Option<String>,
        #[serde(rename = "webEntryPointRedirectionUri")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub web_entry_point_redirection_uri: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct WaSubscriptionEntryPoints {
        #[serde(rename = "subscriptionEntryPoints")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub subscription_entry_points: Option<Vec<SubscriptionEntryPoints>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(rename = "waSubscriptionEntryPoints")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub wa_subscription_entry_points: Option<WaSubscriptionEntryPoints>,
    }
}

/// `WAWebFetchSubscriptionsQuery` (query).
pub mod fetch_subscriptions {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebFetchSubscriptionsQuery";
    pub const DOC_ID: &str = "35324254123840149";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Data {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub platform: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub data: Option<Data>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct FeatureFlags {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub enabled: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub expiration_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub limit: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Subscriptions {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub creation_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub end_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_platform_changed: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub source: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub start_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub tier: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct XwaGetSubscriptions {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub feature_flags: Option<Vec<FeatureFlags>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub subscriptions: Option<Vec<Subscriptions>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa_get_subscriptions: Option<XwaGetSubscriptions>,
    }
}

/// `WAWebMexFetchTextStatusListJobQuery` (query).
pub mod fetch_text_status_list {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexFetchTextStatusListJobQuery";
    pub const DOC_ID: &str = "24072923595647473";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Emoji {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub content: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2TextStatusList {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub emoji: Option<Emoji>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub ephemeral_duration_sec: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub jid: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub last_update_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub text: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_text_status_list: Option<Vec<Xwa2TextStatusList>>,
    }
}

/// `WAWebGetAccessTokenFromOIDCCodeMutation` (mutation).
pub mod get_access_token_from_oidc_code {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebGetAccessTokenFromOIDCCodeMutation";
    pub const DOC_ID: &str = "25278212845117908";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub code: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub state: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct XfbWaBizGetTokenFromOidcCode {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub access_token: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub fb_user_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xfb_wa_biz_get_token_from_oidc_code: Option<XfbWaBizGetTokenFromOidcCode>,
    }
}

/// `WAWebGetAccountNonceMutation` (mutation).
pub mod get_account_nonce {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebGetAccountNonceMutation";
    pub const DOC_ID: &str = "25091178200467555";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Identifier {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub scope: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Input {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub identifier: Option<Identifier>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<Input>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Request {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Detail {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub nonce: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub request: Option<Request>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct XfbWaBizAccountNonce {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub detail: Option<Detail>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xfb_wa_biz_account_nonce: Option<XfbWaBizAccountNonce>,
    }
}

/// `WAWebMexGetDsbInfoJobMutation` (mutation).
pub mod get_dsb_info {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexGetDsbInfoJobMutation";
    pub const DOC_ID: &str = "9982897848413251";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Input {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub entity_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<Input>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2GetDsbInfo {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub reference_number: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_get_dsb_info: Option<Xwa2GetDsbInfo>,
    }
}

/// `WAWebGetFBAccountPagesQuery` (query).
pub mod get_fb_account_pages {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebGetFBAccountPagesQuery";
    pub const DOC_ID: &str = "24564518546541529";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(rename = "userId")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub user_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ProfilePicture {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub uri: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Nodes {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub permitted_tasks: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub profile_picture: Option<ProfilePicture>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct FacebookPages {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub nodes: Option<Vec<Nodes>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct User {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub facebook_pages: Option<FacebookPages>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub user: Option<User>,
    }
}

/// `WAWebGetNumbersForBrandIdsJobQuery` (query).
pub mod get_numbers_for_brand_ids {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebGetNumbersForBrandIdsJobQuery";
    pub const DOC_ID: &str = "33391034967211217";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Input {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub brand_ids: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub lid_based_response: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<Input>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct BrandIdsData {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub brand_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub error: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub lids: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub phone_numbers: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct XwaGetNumbersForBrandIds {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub brand_ids_data: Option<Vec<BrandIdsData>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa_get_numbers_for_brand_ids: Option<XwaGetNumbersForBrandIds>,
    }
}

/// `WAWebMexGetPrivacyListsQuery` (query).
pub mod get_privacy_lists {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexGetPrivacyListsQuery";
    pub const DOC_ID: &str = "26806428515612550";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct PrivacyContactListType {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub category: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub dhash: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub r#type: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct QueryInput {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub jid: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub privacy_contact_list_type: Option<PrivacyContactListType>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Input {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub query_input: Option<Vec<QueryInput>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<Input>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct UsernameInfo {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub username: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Contacts {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub jid: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub pn_jid: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub username_info: Option<UsernameInfo>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct PrivacyContactList {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub contacts: Option<Vec<Contacts>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub dhash: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2FetchWaUsers {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub privacy_contact_list: Option<PrivacyContactList>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_fetch_wa_users: Option<Vec<Xwa2FetchWaUsers>>,
    }
}

/// `WAWebMexGetPrivacySettingsQuery` (query).
pub mod get_privacy_settings {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexGetPrivacySettingsQuery";
    pub const DOC_ID: &str = "25637004609323493";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct QueryInput {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub jid: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub privacy_features: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Input {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub query_input: Option<Vec<QueryInput>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<Input>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Settings {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub feature: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub setting: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct PrivacySettings {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub settings: Option<Vec<Settings>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2FetchWaUsers {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub privacy_settings: Option<PrivacySettings>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_fetch_wa_users: Option<Vec<Xwa2FetchWaUsers>>,
    }
}

/// `WAWebMexGetUsernameJobQuery` (query).
pub mod get_username {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexGetUsernameJobQuery";
    pub const DOC_ID: &str = "25347099718279209";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct UsernameInfo {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub pin: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub state: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub username: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2UsernameGet {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub username_info: Option<UsernameInfo>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_username_get: Option<Xwa2UsernameGet>,
    }
}

/// `WAWebGetWAAEligibilityQuery` (query).
pub mod get_waa_eligibility {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebGetWAAEligibilityQuery";
    pub const DOC_ID: &str = "24346676171620002";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Input {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub flow_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub request_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<Input>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct EvalWaAdAccountEligibilityRules {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub eligibility_result: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub eval_wa_ad_account_eligibility_rules: Option<EvalWaAdAccountEligibilityRules>,
    }
}

/// `WAWebGraphQLProductCatalogGetPublicKeyJobQuery` (query).
pub mod graph_ql_product_catalog_get_public_key {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebGraphQLProductCatalogGetPublicKeyJobQuery";
    pub const DOC_ID: &str = "WAWebGraphQLProductCatalogGetPublicKeyJobQuery";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct PublicKey {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub biz_jid: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Request {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub public_key: Option<PublicKey>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub request: Option<Request>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct PublicKeyWithSignature {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub public_key_pem: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub public_key_signature: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct XwaProductCatalogGetPublicKey {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub public_key_certificate_pem: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub public_key_with_signature: Option<PublicKeyWithSignature>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa_product_catalog_get_public_key: Option<XwaProductCatalogGetPublicKey>,
    }
}

/// `WAWebGraphQLVerifyPostcodeJobQuery` (query).
pub mod graph_ql_verify_postcode {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebGraphQLVerifyPostcodeJobQuery";
    pub const DOC_ID: &str = "WAWebGraphQLVerifyPostcodeJobQuery";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct VerifyPostcode {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub biz_jid: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub direct_connection_encrypted_info: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Request {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub verify_postcode: Option<VerifyPostcode>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub request: Option<Request>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct PostcodeVerificationResult {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub encrypted_location_name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub result_code: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct XwaProductCatalogGetVerifyPostcode {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub postcode_verification_result: Option<PostcodeVerificationResult>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa_product_catalog_get_verify_postcode: Option<XwaProductCatalogGetVerifyPostcode>,
    }
}

/// `WAWebMexGroupStoreInviteSmsJobMutation` (mutation).
pub mod group_store_invite_sms {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexGroupStoreInviteSmsJobMutation";
    pub const DOC_ID: &str = "26810859745268181";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Input {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub group_jid: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub partcipants: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<Input>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ParticipantResponses {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub error_code: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2GroupStoreInvitesSms {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub group_jid: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub participant_responses: Option<Vec<ParticipantResponses>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_group_store_invites_sms: Option<Xwa2GroupStoreInvitesSms>,
    }
}

/// `WAWebGroupSuspensionAppealMutation` (mutation).
pub mod group_suspension_appeal {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebGroupSuspensionAppealMutation";
    pub const DOC_ID: &str = "25946115325088226";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Input {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub appeal_reason: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub debug_info: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub group_jid: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<Input>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct WaCreateGroupSuspensionAppeal {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub appeal_creation_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub error_message: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub response_code: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub wa_create_group_suspension_appeal: Option<WaCreateGroupSuspensionAppeal>,
    }
}

/// `WAWebMexIntegrityChallengeResponseMutation` (mutation).
pub mod integrity_challenge_response {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexIntegrityChallengeResponseMutation";
    pub const DOC_ID: &str = "26230331493320650";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct PasskeyResponse {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub prf_available: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub signed_challenge: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Input {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub challenge_type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub passkey_response: Option<PasskeyResponse>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<Input>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2SubmitIntegrityChallengeResponse {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub error_message: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub success: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_submit_integrity_challenge_response: Option<Xwa2SubmitIntegrityChallengeResponse>,
    }
}

/// `WAWebMexJoinNewsletterJobMutation` (mutation).
pub mod join_newsletter {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexJoinNewsletterJobMutation";
    pub const DOC_ID: &str = "24404358912487870";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub newsletter_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct State {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub r#type: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2NewsletterJoinV2 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub state: Option<State>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_newsletter_join_v2: Option<Xwa2NewsletterJoinV2>,
    }
}

/// `WAWebMexLeaveNewsletterJobMutation` (mutation).
pub mod leave_newsletter {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexLeaveNewsletterJobMutation";
    pub const DOC_ID: &str = "9767147403369991";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub newsletter_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct State {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub r#type: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2NewsletterLeaveV2 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub state: Option<State>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_newsletter_leave_v2: Option<Xwa2NewsletterLeaveV2>,
    }
}

/// `WAWebMexLidChangeNotificationQuery` (query).
pub mod lid_change_notification {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexLidChangeNotificationQuery";
    pub const DOC_ID: &str = "9892367127524985";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2NotifyLidChange {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub new: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub old: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_notify_lid_change: Option<Xwa2NotifyLidChange>,
    }
}

/// `WAWebMexLogNewsletterExposuresJobMutation` (mutation).
pub mod log_newsletter_exposures {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexLogNewsletterExposuresJobMutation";
    pub const DOC_ID: &str = "25260800823586918";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Exposures {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub capability: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub newsletter_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Input {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub exposures: Option<Vec<Exposures>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<Input>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2NewsletterLogExposures {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_newsletter_log_exposures: Option<Xwa2NewsletterLogExposures>,
    }
}

/// `WAWebNativeMLModelQuery` (query).
pub mod native_ml_model {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebNativeMLModelQuery";
    pub const DOC_ID: &str = "32743078615336512";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub client_capability_metadata: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub model_request_metadatas: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Assets {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub asset_handle: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub asset_type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub cache_key: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub compression_type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub creation_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub filesize_bytes: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub md5_hash: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub source_content_hash: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub url: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Properties {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Models {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub assets: Option<Vec<Assets>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub properties: Option<Vec<Properties>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub version: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct AimModelBatchedManifest {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub asset_count: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub entry_point: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub model_count: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub models: Option<Vec<Models>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status_details: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub aim_model_batched_manifest: Option<AimModelBatchedManifest>,
    }
}

/// `WAWebMexNewsletterAddPaidPartnershipLabelJobMutation` (mutation).
pub mod newsletter_add_paid_partnership_label {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexNewsletterAddPaidPartnershipLabelJobMutation";
    pub const DOC_ID: &str = "26102375079404865";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub message_type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub newsletter_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub server_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2NewsletterLabelPaidPartnership {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_newsletter_label_paid_partnership: Option<Xwa2NewsletterLabelPaidPartnership>,
    }
}

/// `WAWebQueryCatalogQuery` (query).
pub mod query_catalog {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebQueryCatalogQuery";
    pub const DOC_ID: &str = "9916553288394782";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ProductCatalog {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub after: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub allow_shop_source: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub catalog_session_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub direct_connection_encrypted_info: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub height: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub jid: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub limit: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub variant_info_fields: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub variant_thumbnail_height: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub variant_thumbnail_width: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub width: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Request {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub product_catalog: Option<ProductCatalog>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub request: Option<Request>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Paging {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub after: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub before: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ImporterAddress {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub city: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub country_code: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub postal_code: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub region: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub street1: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub street2: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ComplianceInfo {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub country_code_origin: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub importer_address: Option<ImporterAddress>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub importer_name: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Images {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub original_image_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub request_image_url: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Videos {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub original_video_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub thumbnail_url: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Media {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub images: Option<Vec<Images>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub videos: Option<Vec<Videos>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct SalePrice {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub end_date: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub price: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub start_date: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct StatusInfo {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub can_appeal: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Options {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Listing {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_available: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub options: Option<Vec<Options>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub product_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Availability {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub listing: Option<Vec<Listing>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ListingDetails {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub description: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub lowest_price: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub multi_price: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct OriginalDimensions {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub height: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub width: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ThumbnailMedia {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub original_dimensions: Option<OriginalDimensions>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub original_image_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub request_image_url: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Options2 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub thumbnail_media: Option<ThumbnailMedia>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Types {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub options: Option<Vec<Options2>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct VariantProperties {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct VariantInfo {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub availability: Option<Availability>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub listing_details: Option<ListingDetails>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub types: Option<Vec<Types>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub variant_properties: Option<Vec<VariantProperties>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Products {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub belongs_to: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub compliance_category: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub compliance_info: Option<ComplianceInfo>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub currency: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub description: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_hidden: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_sanctioned: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub max_available: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub media: Option<Media>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub price: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub product_availability: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub retailer_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sale_price: Option<SalePrice>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub shimmed_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status_info: Option<StatusInfo>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub variant_info: Option<VariantInfo>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ProductCatalog2 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub paging: Option<Paging>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub products: Option<Vec<Products>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct XwaProductCatalogGetProductCatalog {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub product_catalog: Option<ProductCatalog2>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa_product_catalog_get_product_catalog: Option<XwaProductCatalogGetProductCatalog>,
    }
}

/// `WAWebQueryCatalogHasCategoriesQuery` (query).
pub mod query_catalog_has_categories {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebQueryCatalogHasCategoriesQuery";
    pub const DOC_ID: &str = "9759957480718978";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Categories {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub biz_jid: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub catalog_session_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub direct_connection_encrypted_info: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub image_dimensions: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Request {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub categories: Option<Categories>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub request: Option<Request>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Categories2 {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct XwaProductCatalogGetCategories {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub categories: Option<Vec<Categories2>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa_product_catalog_get_categories: Option<XwaProductCatalogGetCategories>,
    }
}

/// `WAWebQueryCatalogProductQuery` (query).
pub mod query_catalog_product {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebQueryCatalogProductQuery";
    pub const DOC_ID: &str = "9647868451963105";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Product {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub direct_connection_encrypted_info: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub fetch_compliance_info: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub height: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub jid: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub product_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub variant_info_fields: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub variant_thumbnail_height: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub variant_thumbnail_width: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub width: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Request {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub product: Option<Product>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub request: Option<Request>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ImporterAddress {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub city: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub country_code: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub postal_code: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub region: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub street1: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub street2: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ComplianceInfo {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub country_code_origin: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub importer_address: Option<ImporterAddress>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub importer_name: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Images {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub original_image_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub request_image_url: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Videos {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub original_video_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub thumbnail_url: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Media {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub images: Option<Vec<Images>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub videos: Option<Vec<Videos>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct SalePrice {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub end_date: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub price: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub start_date: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct StatusInfo {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub can_appeal: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Options {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Listing {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_available: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub options: Option<Vec<Options>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub product_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Availability {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub listing: Option<Vec<Listing>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ListingDetails {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub description: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub lowest_price: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub multi_price: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct OriginalDimensions {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub height: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub width: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ThumbnailMedia {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub original_dimensions: Option<OriginalDimensions>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub original_image_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub request_image_url: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Options2 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub thumbnail_media: Option<ThumbnailMedia>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Types {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub options: Option<Vec<Options2>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct VariantProperties {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct VariantInfo {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub availability: Option<Availability>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub listing_details: Option<ListingDetails>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub types: Option<Vec<Types>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub variant_properties: Option<Vec<VariantProperties>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Product2 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub belongs_to: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub compliance_category: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub compliance_info: Option<ComplianceInfo>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub currency: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub description: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_hidden: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_sanctioned: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub max_available: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub media: Option<Media>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub price: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub product_availability: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub retailer_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sale_price: Option<SalePrice>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub shimmed_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status_info: Option<StatusInfo>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub variant_info: Option<VariantInfo>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ProductCatalog {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub product: Option<Product2>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct XwaProductCatalogGetProduct {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub product_catalog: Option<ProductCatalog>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa_product_catalog_get_product: Option<XwaProductCatalogGetProduct>,
    }
}

/// `WAWebQueryProductCollectionsQuery` (query).
pub mod query_product_collections {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebQueryProductCollectionsQuery";
    pub const DOC_ID: &str = "9430970660362540";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Collections {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub after: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub biz_jid: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub collection_limit: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub direct_connection_encrypted_info: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub height: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub item_limit: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub variant_info_fields: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub variant_thumbnail_height: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub variant_thumbnail_width: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub width: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Request {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub collections: Option<Collections>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub request: Option<Request>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ImporterAddress {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub city: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub country_code: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub postal_code: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub region: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub street1: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub street2: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ComplianceInfo {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub country_code_origin: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub importer_address: Option<ImporterAddress>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub importer_name: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Images {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub original_image_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub request_image_url: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Videos {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub original_video_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub thumbnail_url: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Media {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub images: Option<Vec<Images>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub videos: Option<Vec<Videos>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct SalePrice {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub end_date: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub price: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub start_date: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct StatusInfo {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub can_appeal: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Options {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Listing {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_available: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub options: Option<Vec<Options>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub product_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Availability {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub listing: Option<Vec<Listing>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ListingDetails {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub description: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub lowest_price: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub multi_price: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct OriginalDimensions {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub height: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub width: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ThumbnailMedia {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub original_dimensions: Option<OriginalDimensions>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub original_image_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub request_image_url: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Options2 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub thumbnail_media: Option<ThumbnailMedia>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Types {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub options: Option<Vec<Options2>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct VariantProperties {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct VariantInfo {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub availability: Option<Availability>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub listing_details: Option<ListingDetails>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub types: Option<Vec<Types>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub variant_properties: Option<Vec<VariantProperties>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Products {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub belongs_to: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub compliance_category: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub compliance_info: Option<ComplianceInfo>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub currency: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub description: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_hidden: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_sanctioned: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub max_available: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub media: Option<Media>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub price: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub product_availability: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub retailer_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sale_price: Option<SalePrice>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub shimmed_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status_info: Option<StatusInfo>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub variant_info: Option<VariantInfo>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct StatusInfo2 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub can_appeal: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub commerce_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub reject_reason: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Collections2 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub products: Option<Vec<Products>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status_info: Option<StatusInfo2>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Paging {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub after: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct XwaProductCatalogGetCollections {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub collections: Option<Vec<Collections2>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub paging: Option<Paging>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa_product_catalog_get_collections: Option<XwaProductCatalogGetCollections>,
    }
}

/// `WAWebQueryProductListCatalogJobQuery` (query).
pub mod query_product_list_catalog {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebQueryProductListCatalogJobQuery";
    pub const DOC_ID: &str = "30125049463760630";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Products {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ProductList {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub direct_connection_encrypted_info: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub height: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub jid: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub products: Option<Vec<Products>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub width: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Request {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub product_list: Option<ProductList>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub request: Option<Request>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ImporterAddress {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub city: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub country_code: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub postal_code: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub region: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub street1: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub street2: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ComplianceInfo {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub country_code_origin: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub importer_address: Option<ImporterAddress>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub importer_name: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Images {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub original_image_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub request_image_url: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Videos {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub original_video_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub thumbnail_url: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Media {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub images: Option<Vec<Images>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub videos: Option<Vec<Videos>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct SalePrice {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub end_date: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub price: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub start_date: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct StatusInfo {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub can_appeal: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Options {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Listing {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_available: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub options: Option<Vec<Options>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub product_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Availability {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub listing: Option<Vec<Listing>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ListingDetails {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub description: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub lowest_price: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub multi_price: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct OriginalDimensions {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub height: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub width: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ThumbnailMedia {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub original_dimensions: Option<OriginalDimensions>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub original_image_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub request_image_url: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Options2 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub thumbnail_media: Option<ThumbnailMedia>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Types {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub options: Option<Vec<Options2>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct VariantProperties {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct VariantInfo {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub availability: Option<Availability>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub listing_details: Option<ListingDetails>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub types: Option<Vec<Types>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub variant_properties: Option<Vec<VariantProperties>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Products2 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub belongs_to: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub compliance_category: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub compliance_info: Option<ComplianceInfo>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub currency: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub description: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_hidden: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_sanctioned: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub max_available: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub media: Option<Media>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub price: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub product_availability: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub retailer_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sale_price: Option<SalePrice>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub shimmed_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status_info: Option<StatusInfo>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub variant_info: Option<VariantInfo>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ProductList2 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub products: Option<Vec<Products2>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct XwaProductCatalogGetProductList {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub product_list: Option<ProductList2>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa_product_catalog_get_product_list: Option<XwaProductCatalogGetProductList>,
    }
}

/// `WAWebQueryProductSingleCollectionQuery` (query).
pub mod query_product_single_collection {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebQueryProductSingleCollectionQuery";
    pub const DOC_ID: &str = "9546992575408789";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Collection {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub after: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub biz_jid: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub direct_connection_encrypted_info: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub height: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub limit: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub variant_info_fields: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub variant_thumbnail_height: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub variant_thumbnail_width: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub width: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Request {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub collection: Option<Collection>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub request: Option<Request>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ImporterAddress {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub city: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub country_code: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub postal_code: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub region: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub street1: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub street2: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ComplianceInfo {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub country_code_origin: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub importer_address: Option<ImporterAddress>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub importer_name: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Images {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub original_image_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub request_image_url: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Videos {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub original_video_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub thumbnail_url: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Media {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub images: Option<Vec<Images>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub videos: Option<Vec<Videos>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct SalePrice {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub end_date: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub price: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub start_date: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct StatusInfo {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub can_appeal: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Options {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Listing {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_available: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub options: Option<Vec<Options>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub product_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Availability {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub listing: Option<Vec<Listing>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ListingDetails {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub description: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub lowest_price: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub multi_price: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct OriginalDimensions {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub height: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub width: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ThumbnailMedia {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub original_dimensions: Option<OriginalDimensions>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub original_image_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub request_image_url: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Options2 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub thumbnail_media: Option<ThumbnailMedia>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Types {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub options: Option<Vec<Options2>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct VariantProperties {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct VariantInfo {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub availability: Option<Availability>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub listing_details: Option<ListingDetails>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub types: Option<Vec<Types>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub variant_properties: Option<Vec<VariantProperties>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Products {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub belongs_to: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub compliance_category: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub compliance_info: Option<ComplianceInfo>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub currency: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub description: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_hidden: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_sanctioned: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub max_available: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub media: Option<Media>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub price: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub product_availability: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub retailer_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sale_price: Option<SalePrice>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub shimmed_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status_info: Option<StatusInfo>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub variant_info: Option<VariantInfo>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct StatusInfo2 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub can_appeal: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub commerce_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub reject_reason: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Collection2 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub products: Option<Vec<Products>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status_info: Option<StatusInfo2>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Paging {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub after: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct XwaProductCatalogGetSingleCollection {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub collection: Option<Collection2>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub paging: Option<Paging>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa_product_catalog_get_single_collection: Option<XwaProductCatalogGetSingleCollection>,
    }
}

/// `WAWebMexQuerySubgroupParticipantCountJobQuery` (query).
pub mod query_subgroup_participant_count {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexQuerySubgroupParticipantCountJobQuery";
    pub const DOC_ID: &str = "24079399904996141";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Input {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub group_jid: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub query_context: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sub_group_jid_hint: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<Input>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Node {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub total_participants_count: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Edges {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub node: Option<Node>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct SubGroups {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub edges: Option<Vec<Edges>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2GroupQueryById {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub sub_groups: Option<SubGroups>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_group_query_by_id: Option<Xwa2GroupQueryById>,
    }
}

/// `WAWebQuickPromotionActionMutation` (mutation).
pub mod quick_promotion_action {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebQuickPromotionActionMutation";
    pub const DOC_ID: &str = "9741612265875562";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct WaQuickPromotionLogEvent {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub client_mutation_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub wa_quick_promotion_log_event: Option<WaQuickPromotionLogEvent>,
    }
}

/// `WAWebReportProductJobMutation` (mutation).
pub mod report_product {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebReportProductJobMutation";
    pub const DOC_ID: &str = "WAWebReportProductJobMutation";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Input {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub jid: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub product_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<Input>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct XwaWhatsappCatalogReportProduct {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub success: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa_whatsapp_catalog_report_product: Option<XwaWhatsappCatalogReportProduct>,
    }
}

/// `WAWebMexRequestClientLogsForBugJobMutation` (mutation).
pub mod request_client_logs_for_bug {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexRequestClientLogsForBugJobMutation";
    pub const DOC_ID: &str = "27135500612803533";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Input {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub bug_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub participant_ids: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub reporter_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub up_to_timestamp_secs: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<Input>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_request_client_logs_for_bug: Option<String>,
    }
}

/// `WAWebMexRequestOTEJobMutation` (mutation).
pub mod request_ote {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexRequestOTEJobMutation";
    pub const DOC_ID: &str = "35108428002105570";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Input {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub reason_text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub selected_reason: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub r#type: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<Input>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2NcmRequestOte {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub capping_status: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub cycle_end_timestamp: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub cycle_start_timestamp: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub mv_status: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub ote_status: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub server_sent_timestamp: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub total_quota: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub used_quota: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_ncm_request_ote: Option<Xwa2NcmRequestOte>,
    }
}

/// `WAWebResolveAccountTypeAndAdPageMutation` (mutation).
pub mod resolve_account_type_and_ad_page {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebResolveAccountTypeAndAdPageMutation";
    pub const DOC_ID: &str = "24732033759799062";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(rename = "pageId")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub page_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xfb_wa_biz_clear_oidc_preference: Option<String>,
    }
}

/// `WAWebResolveAccountTypeAndAdPageQuery` (query).
pub mod resolve_account_type_and_ad_page_query {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebResolveAccountTypeAndAdPageQuery";
    pub const DOC_ID: &str = "24856134350695832";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(rename = "pageId")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub page_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Page {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub can_viewer_do_actions: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub page: Option<Page>,
    }
}

/// `WAWebMexRevokeNewsletterAdminInviteJobMutation` (mutation).
pub mod revoke_newsletter_admin_invite {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexRevokeNewsletterAdminInviteJobMutation";
    pub const DOC_ID: &str = "9656078347839416";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub newsletter_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub user_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2NewsletterAdminInviteRevoke {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_newsletter_admin_invite_revoke: Option<Xwa2NewsletterAdminInviteRevoke>,
    }
}

/// `WAWebMexSetUsernameJobMutation` (mutation).
pub mod set_username {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexSetUsernameJobMutation";
    pub const DOC_ID: &str = "25757341163897635";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub reserved: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub session_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub source: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2UsernameSet {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub result: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_username_set: Option<Xwa2UsernameSet>,
    }
}

/// `WAWebMexSetUsernameKeyJobMutation` (mutation).
pub mod set_username_key {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexSetUsernameKeyJobMutation";
    pub const DOC_ID: &str = "9749436995157074";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub pin: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2UsernamePinSet {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub result: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_username_pin_set: Option<Xwa2UsernamePinSet>,
    }
}

/// `WAWebSignupMetadataQuery` (query).
pub mod signup_metadata {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebSignupMetadataQuery";
    pub const DOC_ID: &str = "26378108788468347";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub phone_number: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub signup_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct WaSignupMetadata {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub privacy_policy_url: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub signup_message: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub wa_signup_metadata: Option<WaSignupMetadata>,
    }
}

/// `WAWebSupportBugReportSubmitMutation` (mutation).
pub mod support_bug_report_submit {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebSupportBugReportSubmitMutation";
    pub const DOC_ID: &str = "25952242091096312";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct XwaWaSupportBugReportSubmit {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub bug_report_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub error_code: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub error_message: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub success: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub task_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa_wa_support_bug_report_submit: Option<XwaWaSupportBugReportSubmit>,
    }
}

/// `WAWebSupportContactFormSubmitMutation` (mutation).
pub mod support_contact_form_submit {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebSupportContactFormSubmitMutation";
    pub const DOC_ID: &str = "26494666453460666";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct XwaWaSupportContactFormSubmit {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub error_code: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub error_message: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub success: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub support_phone_number_jid: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub ticket_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa_wa_support_contact_form_submit: Option<XwaWaSupportContactFormSubmit>,
    }
}

/// `WAWebSupportMessageFeedbackSubmitMutation` (mutation).
pub mod support_message_feedback_submit {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebSupportMessageFeedbackSubmitMutation";
    pub const DOC_ID: &str = "25772720305756789";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct XwaWaSupportMessageFeedbackSubmit {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub error_code: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub error_message: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub success: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa_wa_support_message_feedback_submit: Option<XwaWaSupportMessageFeedbackSubmit>,
    }
}

/// `WAWebMexTransferCommunityOwnershipJobMutation` (mutation).
pub mod transfer_community_ownership {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexTransferCommunityOwnershipJobMutation";
    pub const DOC_ID: &str = "29643783178598899";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct LidMigrationState {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub addressing_mode: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2GroupUpdateUsersRole {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub group_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub lid_migration_state: Option<LidMigrationState>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_group_update_users_role: Option<Xwa2GroupUpdateUsersRole>,
    }
}

/// `WAWebMexUpdateGroupPropertyJobMutation` (mutation).
pub mod update_group_property {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexUpdateGroupPropertyJobMutation";
    pub const DOC_ID: &str = "9418211574894172";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub group_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub update: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2GroupUpdateProperty {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub state: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_group_update_property: Option<Xwa2GroupUpdateProperty>,
    }
}

/// `WAWebMexUpdateNewsletterJobMutation` (mutation).
pub mod update_newsletter {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexUpdateNewsletterJobMutation";
    pub const DOC_ID: &str = "24250201037901610";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Updates {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub description: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub picture: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub settings: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub newsletter_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub updates: Option<Updates>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct State {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub r#type: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Description {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub update_time: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Name {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub update_time: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Picture {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub direct_path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub r#type: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Preview {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub direct_path: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub r#type: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ReactionCodes {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub value: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Settings {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub reaction_codes: Option<ReactionCodes>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ThreadMetadata {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub creation_time: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub description: Option<Description>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub handle: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub invite: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub name: Option<Name>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub picture: Option<Picture>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub preview: Option<Preview>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub settings: Option<Settings>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub verification: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2NewsletterUpdate {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub state: Option<State>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub thread_metadata: Option<ThreadMetadata>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_newsletter_update: Option<Xwa2NewsletterUpdate>,
    }
}

/// `WAWebMexUpdateNewsletterUserSettingJobMutation` (mutation).
pub mod update_newsletter_user_setting {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexUpdateNewsletterUserSettingJobMutation";
    pub const DOC_ID: &str = "31938993655691868";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct State {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub r#type: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2NewsletterUpdateUserSetting {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub state: Option<State>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_newsletter_update_user_setting: Option<Xwa2NewsletterUpdateUserSetting>,
    }
}

/// `WAWebMexUpdateTextStatusJobMutation` (mutation).
pub mod update_text_status {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexUpdateTextStatusJobMutation";
    pub const DOC_ID: &str = "9152604461510864";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2UpdateTextStatus {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub result: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_update_text_status: Option<Xwa2UpdateTextStatus>,
    }
}

/// `WAWebMexUsernameAvailabilityQuery` (query).
pub mod username_availability {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexUsernameAvailabilityQuery";
    pub const DOC_ID: &str = "26122779627399568";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub session_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub source: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2UsernameCheck {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub result: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub suggestions: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_username_check: Option<Xwa2UsernameCheck>,
    }
}

/// `WAWebMexUsyncQuery` (query).
pub mod usync {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebMexUsyncQuery";
    pub const DOC_ID: &str = "29829202653362039";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Input {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub query_input: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub telemetry: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub include_about_status: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub include_country_code: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub include_username: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<Input>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct AboutStatusInfo {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub text: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub timestamp: Option<i64>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct UsernameInfo {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub pin: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub state: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub timestamp: Option<i64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub username: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Xwa2FetchWaUsers {
        #[serde(rename = "__typename")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub typename: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub about_status_info: Option<AboutStatusInfo>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub country_code: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub jid: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub username_info: Option<UsernameInfo>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xwa2_fetch_wa_users: Option<Vec<Xwa2FetchWaUsers>>,
    }
}

/// `WAWebWAAOnboardingMutation` (mutation).
pub mod waa_onboarding {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebWAAOnboardingMutation";
    pub const DOC_ID: &str = "25173295938976172";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Input {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub flow_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub request_id: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<Input>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct CreateOrOnboardWaAdAccount {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub ad_account_id: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub status: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub create_or_onboard_wa_ad_account: Option<CreateOrOnboardWaAdAccount>,
    }
}

/// `WAWebWaffleFXServiceDataQueryV2Mutation` (mutation).
pub mod waffle_fx_service_data_query_v2 {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebWaffleFXServiceDataQueryV2Mutation";
    pub const DOC_ID: &str = "9475021792620702";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct FoaToWaLinkEligibility {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_eligible_to_link_to_linked_fb: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_eligible_to_link_to_linked_ig: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_eligible_to_link_to_linked_rl: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_eligible_to_link_to_unlinked_fb: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_eligible_to_link_to_unlinked_ig: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub is_eligible_to_link_to_unlinked_rl: Option<bool>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct WaffleAfs {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub waffle_wes: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct WaffleXss {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub waffle_iaxe: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub waffle_x_surface: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct WaffleSxs {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub waffle_da: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub waffle_di: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub waffle_xss: Option<Vec<WaffleXss>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Services {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub foa_to_wa_link_eligibility: Option<FoaToWaLinkEligibility>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub waffle_afs: Option<WaffleAfs>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub waffle_sxs: Option<Vec<WaffleSxs>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct WaffleFxServiceData {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub services: Option<Services>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub waffle_fx_service_data: Option<WaffleFxServiceData>,
    }
}

/// `WAWebWaffleFXWAMOUpdateUOOMMutation` (mutation).
pub mod waffle_fxwamo_update_uoom {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebWaffleFXWAMOUpdateUOOMMutation";
    pub const DOC_ID: &str = "10031635203620145";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {}

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub xfb_waffle_fx_wamo_update_uoom: Option<String>,
    }
}

/// `WAWebWaffleXEQuery` (mutation).
pub mod waffle_xe {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "WAWebWaffleXEQuery";
    pub const DOC_ID: &str = "32172601809054525";
    pub const OPERATION_KIND: &str = "mutation";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub input: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct PurposePublicKeys {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub purpose_dummy_ciphertext: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub purpose_dummy_nonce: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub purpose_public_ek: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub purpose_public_ik: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub purpose_public_ik_enc_certificate: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub purpose_public_ik_sig: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct WaffleXas {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub waffle_xan: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub waffle_xs: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct WaffleD {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub waffle_di: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub waffle_xas: Option<WaffleXas>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct WaffleXps {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub waffle_hcbc: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub waffle_xas: Option<WaffleXas>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct WaffleXeRoot {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub purpose_public_keys: Option<PurposePublicKeys>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub waffle_d: Option<Vec<WaffleD>>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub waffle_unique_ids: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub waffle_xps: Option<Vec<WaffleXps>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub waffle_xe_root: Option<WaffleXeRoot>,
    }
}

/// `useWAWebEstimatedDailyReachQuery` (query).
pub mod use_wa_web_estimated_daily_reach {
    use super::{Deserialize, Serialize};

    pub const NAME: &str = "useWAWebEstimatedDailyReachQuery";
    pub const DOC_ID: &str = "26555147174103537";
    pub const OPERATION_KIND: &str = "query";

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Variables {
        #[serde(rename = "audienceOptionAudience")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub audience_option_audience: Option<String>,
        #[serde(rename = "configuredPlacementSpec")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub configured_placement_spec: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub currency: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub flow: Option<String>,
        #[serde(rename = "flowID")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub flow_id: Option<String>,
        #[serde(rename = "legacyAdAccountID")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub legacy_ad_account_id: Option<String>,
        #[serde(rename = "optimizationGoalInput")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub optimization_goal_input: Option<String>,
        #[serde(rename = "postID")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub post_id: Option<String>,
        #[serde(rename = "targetingSpecAudience")]
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub targeting_spec_audience: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct DailyOutcomesCurve {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub actions: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub actions_lower_bound: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub actions_upper_bound: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub bid: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub impressions: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub reach: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub reach_lower_bound: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub reach_upper_bound: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub spend: Option<String>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct BudgetEstimateDataV2 {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub daily_outcomes_curve: Option<Vec<DailyOutcomesCurve>>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Lwi {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub budget_estimate_data_v2: Option<BudgetEstimateDataV2>,
    }

    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Response {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        pub lwi: Option<Lwi>,
    }
}

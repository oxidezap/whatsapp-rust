//! Auto-generated IQ stanza specs (WhatsApp 2.3000.1040944432). DO NOT EDIT.
//!
//! One `pub mod` per IQ namespace; each holds the namespace const, shared child
//! types, and one `IqSpec` impl per stanza. Regenerated from the IQ IR by wa-codegen.

#![allow(clippy::all)]

/// IQ namespace `abt`. Source: WASmaxOutAbPropsGetExperimentConfigRequest, WASmaxOutAbPropsGetGroupExperimentConfigRequest.
pub mod abt {
    use crate::iq::spec::IqSpec;
    use crate::request::InfoQuery;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::jid::{Jid, Server};
    use wacore_binary::node::NodeContent;

    /// IQ namespace.
    pub const ABT_NAMESPACE: &str = "abt";

    // ─── IQ Specs ───────────────────────────────────────────────────────────────

    /// makeGetExperimentConfigRequest:get IQ spec.
    ///
    /// Source: `WASmaxOutAbPropsGetExperimentConfigRequest`
    #[derive(Debug, Clone)]
    pub struct MakeGetExperimentConfigRequestSpec {
        pub hash: Option<String>,
        pub refresh_id: Option<String>,
    }

    impl MakeGetExperimentConfigRequestSpec {
        pub fn new(hash: Option<String>, refresh_id: Option<String>) -> Self {
            Self { hash, refresh_id }
        }
    }

    /// Response from makeGetExperimentConfigRequest:get.
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetExperimentConfigRequestResponse {
        pub props_protocol: String,
        pub props_ab_key: Option<String>,
        pub props_hash: Option<String>,
        pub props_refresh: Option<u64>,
        pub props_refresh_id: Option<u64>,
        pub props_delta_update: Option<String>,
        pub r#type: String,
        pub erid: Vec<u8>,
    }

    impl IqSpec for MakeGetExperimentConfigRequestSpec {
        type Response = MakeGetExperimentConfigRequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut props_node = NodeBuilder::new("props");
            props_node = props_node.attr("protocol", "1");
            if let Some(v) = &self.hash {
                props_node = props_node.attr("hash", v.as_str());
            }
            if let Some(v) = &self.refresh_id {
                props_node = props_node.attr("refresh_id", v.as_str());
            }
            let props_node = props_node.build();

            InfoQuery::get(
                ABT_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![props_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let props_wrap = response
                .get_optional_child("props")
                .ok_or_else(|| anyhow::anyhow!("missing <props>"))?;
            let props_protocol = props_wrap
                .get_attr("protocol")
                .ok_or_else(|| anyhow::anyhow!("missing protocol"))?
                .as_str()
                .to_string();
            let props_ab_key = props_wrap
                .get_attr("ab_key")
                .map(|v| v.as_str().to_string());
            let props_hash = props_wrap.get_attr("hash").map(|v| v.as_str().to_string());
            let props_refresh = props_wrap
                .get_attr("refresh")
                .and_then(|v| v.as_str().parse().ok());
            let props_refresh_id = props_wrap
                .get_attr("refresh_id")
                .and_then(|v| v.as_str().parse().ok());
            let props_delta_update = props_wrap
                .get_attr("delta_update")
                .map(|v| v.as_str().to_string());
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            let erid_node = response
                .get_optional_child("erid")
                .ok_or_else(|| anyhow::anyhow!("missing <erid>"))?;
            let erid = erid_node
                .content_bytes()
                .map(|b| b.to_vec())
                .unwrap_or_default();
            Ok(MakeGetExperimentConfigRequestResponse {
                props_protocol,
                props_ab_key,
                props_hash,
                props_refresh,
                props_refresh_id,
                props_delta_update,
                r#type,
                erid,
                ..Default::default()
            })
        }
    }

    /// makeGetGroupExperimentConfigRequest:get IQ spec.
    ///
    /// Source: `WASmaxOutAbPropsGetGroupExperimentConfigRequest`
    #[derive(Debug, Clone)]
    pub struct MakeGetGroupExperimentConfigRequestSpec {
        pub group: Jid,
        pub hash: Option<String>,
    }

    impl MakeGetGroupExperimentConfigRequestSpec {
        pub fn new(group: &Jid, hash: Option<String>) -> Self {
            Self {
                group: group.clone(),
                hash,
            }
        }
    }

    /// Response from makeGetGroupExperimentConfigRequest:get.
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetGroupExperimentConfigRequestResponse {
        pub props_ab_key: Option<String>,
        pub props_hash: Option<String>,
        pub props_refresh: Option<u64>,
        pub props_refresh_id: Option<u64>,
        pub r#type: String,
    }

    impl IqSpec for MakeGetGroupExperimentConfigRequestSpec {
        type Response = MakeGetGroupExperimentConfigRequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut props_node = NodeBuilder::new("props");
            props_node = props_node.attr("group", self.group.clone());
            if let Some(v) = &self.hash {
                props_node = props_node.attr("hash", v.as_str());
            }
            let props_node = props_node.build();

            InfoQuery::get(
                ABT_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![props_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let props_wrap = response
                .get_optional_child("props")
                .ok_or_else(|| anyhow::anyhow!("missing <props>"))?;
            let props_ab_key = props_wrap
                .get_attr("ab_key")
                .map(|v| v.as_str().to_string());
            let props_hash = props_wrap.get_attr("hash").map(|v| v.as_str().to_string());
            let props_refresh = props_wrap
                .get_attr("refresh")
                .and_then(|v| v.as_str().parse().ok());
            let props_refresh_id = props_wrap
                .get_attr("refresh_id")
                .and_then(|v| v.as_str().parse().ok());
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            Ok(MakeGetGroupExperimentConfigRequestResponse {
                props_ab_key,
                props_hash,
                props_refresh,
                props_refresh_id,
                r#type,
                ..Default::default()
            })
        }
    }
}

/// IQ namespace `blocklist`. Source: WASmaxOutBlocklistsGetBlockListRequest, WASmaxOutBlocklistsUpdateBlockListRequest.
pub mod blocklist {
    use crate::iq::spec::IqSpec;
    use crate::request::InfoQuery;
    use wacore_binary::jid::{Jid, Server};

    /// IQ namespace.
    pub const BLOCKLIST_NAMESPACE: &str = "blocklist";

    // ─── Shared child types ──────────────────────────────────────────────────────

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetBlockListRequestItemItemItem {
        pub jid: Jid,
        pub lid: Jid,
        pub display_name: Option<String>,
    }

    // ─── IQ Specs ───────────────────────────────────────────────────────────────

    /// makeGetBlockListRequestItem:get IQ spec.
    ///
    /// Source: `WASmaxOutBlocklistsGetBlockListRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetBlockListRequestItemSpec;

    impl IqSpec for MakeGetBlockListRequestItemSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::get(BLOCKLIST_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// makeUpdateBlockListRequestItemBizOptOut:set IQ spec.
    ///
    /// Source: `WASmaxOutBlocklistsUpdateBlockListRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeUpdateBlockListRequestItemBizOptOutSpec;

    /// Response from makeUpdateBlockListRequestItemBizOptOut:set.
    #[derive(Debug, Clone, Default)]
    pub struct MakeUpdateBlockListRequestItemBizOptOutResponse {
        pub r#type: String,
        pub list_matched: String,
        pub list_dhash: String,
    }

    impl IqSpec for MakeUpdateBlockListRequestItemBizOptOutSpec {
        type Response = MakeUpdateBlockListRequestItemBizOptOutResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::set(BLOCKLIST_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            let list_wrap = response
                .get_optional_child("list")
                .ok_or_else(|| anyhow::anyhow!("missing <list>"))?;
            let list_matched = list_wrap
                .get_attr("matched")
                .ok_or_else(|| anyhow::anyhow!("missing matched"))?
                .as_str()
                .to_string();
            let list_dhash = list_wrap
                .get_attr("dhash")
                .ok_or_else(|| anyhow::anyhow!("missing dhash"))?
                .as_str()
                .to_string();
            Ok(MakeUpdateBlockListRequestItemBizOptOutResponse {
                r#type,
                list_matched,
                list_dhash,
                ..Default::default()
            })
        }
    }
}

/// IQ namespace `bot`. Source: WASmaxOutBotBotListRequest, WASmaxOutBotBotListIQMixin.
pub mod bot {
    use crate::iq::spec::IqSpec;
    use crate::request::InfoQuery;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::jid::{Jid, Server};
    use wacore_binary::node::NodeContent;

    /// IQ namespace.
    pub const BOT_NAMESPACE: &str = "bot";

    // ─── Shared child types ──────────────────────────────────────────────────────

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeBotListRequestBotBotBotItem {
        pub jid: Jid,
        pub persona_id: String,
        pub count: Option<u64>,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeBotListRequestBotBotSectionItem {
        pub name: String,
        pub r#type: String,
        pub bot: Vec<MakeBotListRequestBotBotBotItem>,
    }

    // ─── IQ Specs ───────────────────────────────────────────────────────────────

    /// makeBotListRequestBotBot:get IQ spec.
    ///
    /// Source: `WASmaxOutBotBotListRequest`
    #[derive(Debug, Clone)]
    pub struct MakeBotListRequestBotBotSpec {
        pub v: Option<String>,
        pub bhash: Option<String>,
    }

    impl MakeBotListRequestBotBotSpec {
        pub fn new(v: Option<String>, bhash: Option<String>) -> Self {
            Self { v, bhash }
        }
    }

    impl IqSpec for MakeBotListRequestBotBotSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut bot_node = NodeBuilder::new("bot");
            if let Some(v) = &self.v {
                bot_node = bot_node.attr("v", v.as_str());
            }
            if let Some(v) = &self.bhash {
                bot_node = bot_node.attr("bhash", v.as_str());
            }
            let bot_node = bot_node.build();

            InfoQuery::get(
                BOT_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![bot_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// mergeBotListIQMixin:get IQ spec.
    ///
    /// Source: `WASmaxOutBotBotListIQMixin`
    #[derive(Debug, Clone, Default)]
    pub struct MergeBotListIQMixinSpec;

    impl IqSpec for MergeBotListIQMixinSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::get(BOT_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }
}

/// IQ namespace `disappearing_mode`. Source: WAWebQueryDisappearingModeJob, WAWebSetDisappearingModeJob.
pub mod disappearing_mode {
    use crate::iq::spec::IqSpec;
    use crate::request::InfoQuery;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::jid::{Jid, Server};
    use wacore_binary::node::NodeContent;

    /// IQ namespace.
    pub const DISAPPEARING_MODE_NAMESPACE: &str = "disappearing_mode";

    // ─── IQ Specs ───────────────────────────────────────────────────────────────

    /// queryDisappearingMode:get IQ spec.
    ///
    /// Source: `WAWebQueryDisappearingModeJob`
    #[derive(Debug, Clone, Default)]
    pub struct QueryDisappearingModeSpec;

    /// Response from queryDisappearingMode:get.
    #[derive(Debug, Clone, Default)]
    pub struct QueryDisappearingModeResponse {
        pub duration: u64,
        pub t: u64,
    }

    impl IqSpec for QueryDisappearingModeSpec {
        type Response = QueryDisappearingModeResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::get(DISAPPEARING_MODE_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let disappearing_mode = response
                .get_optional_child("disappearing_mode")
                .ok_or_else(|| anyhow::anyhow!("missing <disappearing_mode>"))?;
            let duration: u64 = disappearing_mode
                .get_attr("duration")
                .ok_or_else(|| anyhow::anyhow!("missing duration"))?
                .as_str()
                .parse()?;
            let t: u64 = disappearing_mode
                .get_attr("t")
                .ok_or_else(|| anyhow::anyhow!("missing t"))?
                .as_str()
                .parse()?;
            Ok(QueryDisappearingModeResponse {
                duration,
                t,
                ..Default::default()
            })
        }
    }

    /// setDisappearingMode:set IQ spec.
    ///
    /// Source: `WAWebSetDisappearingModeJob`
    #[derive(Debug, Clone)]
    pub struct SetDisappearingModeSpec {
        pub duration: String,
    }

    impl SetDisappearingModeSpec {
        pub fn new(duration: impl Into<String>) -> Self {
            Self {
                duration: duration.into(),
            }
        }
    }

    impl IqSpec for SetDisappearingModeSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut disappearing_mode_node = NodeBuilder::new("disappearing_mode");
            disappearing_mode_node = disappearing_mode_node.attr("duration", &*self.duration);
            let disappearing_mode_node = disappearing_mode_node.build();

            InfoQuery::set(
                DISAPPEARING_MODE_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![disappearing_mode_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }
}

/// IQ namespace `encrypt`. Source: WAWebDigestKeyJob, WAWebGetIdentityKeysJob, WASmaxOutPreKeysFetchKeyBundlesRequest, WASmaxOutPreKeysFetchMissingPreKeysRequest, WAWebUploadPrekeysForRegTask, WAWebRotateKeyJob, WAWebUploadPreKeysJob.
pub mod encrypt {
    use crate::iq::spec::IqSpec;
    use crate::request::InfoQuery;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::jid::{Jid, Server};
    use wacore_binary::node::NodeContent;

    /// IQ namespace.
    pub const ENCRYPT_NAMESPACE: &str = "encrypt";

    // ─── Shared child types ──────────────────────────────────────────────────────

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct GetAndStoreIdentityKeysChildrenItem {
        pub jid: Jid,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeFetchKeyBundlesRequestKeyUserUserItem {
        pub jid: Jid,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeFetchMissingPreKeysRequestKeyFetchUserDeviceUserItem {
        pub jid: Jid,
    }

    // ─── IQ Specs ───────────────────────────────────────────────────────────────

    /// digestKey:get IQ spec.
    ///
    /// Source: `WAWebDigestKeyJob`
    #[derive(Debug, Clone, Default)]
    pub struct DigestKeySpec;

    /// Response from digestKey:get.
    #[derive(Debug, Clone, Default)]
    pub struct DigestKeyResponse {
        pub identity: Vec<u8>,
        pub value: Vec<u8>,
        pub signature: Vec<u8>,
        pub hash: Vec<u8>,
    }

    impl IqSpec for DigestKeySpec {
        type Response = DigestKeyResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let digest_node = NodeBuilder::new("digest").build();

            InfoQuery::get(
                ENCRYPT_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![digest_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let identity_node = response
                .get_optional_child("identity")
                .ok_or_else(|| anyhow::anyhow!("missing <identity>"))?;
            let identity = identity_node
                .content_bytes()
                .map(|b| b.to_vec())
                .unwrap_or_default();
            let value_node = response
                .get_optional_child("value")
                .ok_or_else(|| anyhow::anyhow!("missing <value>"))?;
            let value = value_node
                .content_bytes()
                .map(|b| b.to_vec())
                .unwrap_or_default();
            let signature_node = response
                .get_optional_child("signature")
                .ok_or_else(|| anyhow::anyhow!("missing <signature>"))?;
            let signature = signature_node
                .content_bytes()
                .map(|b| b.to_vec())
                .unwrap_or_default();
            let hash_node = response
                .get_optional_child("hash")
                .ok_or_else(|| anyhow::anyhow!("missing <hash>"))?;
            let hash = hash_node
                .content_bytes()
                .map(|b| b.to_vec())
                .unwrap_or_default();
            Ok(DigestKeyResponse {
                identity,
                value,
                signature,
                hash,
                ..Default::default()
            })
        }
    }

    /// getAndStoreIdentityKeys:get IQ spec.
    ///
    /// Source: `WAWebGetIdentityKeysJob`
    #[derive(Debug, Clone)]
    pub struct GetAndStoreIdentityKeysSpec {
        pub jid: Jid,
    }

    impl GetAndStoreIdentityKeysSpec {
        pub fn new(jid: &Jid) -> Self {
            Self { jid: jid.clone() }
        }
    }

    /// Response from getAndStoreIdentityKeys:get.
    #[derive(Debug, Clone, Default)]
    pub struct GetAndStoreIdentityKeysResponse {
        pub children: Vec<GetAndStoreIdentityKeysChildrenItem>,
        pub code: u64,
        pub text: String,
        pub r#type: Vec<u8>,
        pub identity: Vec<u8>,
        pub jid: Jid,
    }

    impl IqSpec for GetAndStoreIdentityKeysSpec {
        type Response = GetAndStoreIdentityKeysResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut user_node = NodeBuilder::new("user");
            user_node = user_node.attr("jid", self.jid.clone());
            let user_node = user_node.build();
            let mut identity_node = NodeBuilder::new("identity");
            identity_node = identity_node.children([user_node]);
            let identity_node = identity_node.build();

            InfoQuery::get(
                ENCRYPT_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![identity_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let jid = response
                .get_attr("jid")
                .and_then(|v| v.to_jid())
                .ok_or_else(|| anyhow::anyhow!("missing jid"))?;
            let list = response
                .get_optional_child("list")
                .ok_or_else(|| anyhow::anyhow!("missing <list>"))?;
            let mut children_items = Vec::new();
            for child in list.get_children_by_tag("children") {
                let jid = child
                    .get_attr("jid")
                    .and_then(|v| v.to_jid())
                    .ok_or_else(|| anyhow::anyhow!("missing jid"))?;
                children_items.push(GetAndStoreIdentityKeysChildrenItem {
                    jid,
                    ..Default::default()
                });
            }
            let error = response
                .get_optional_child("error")
                .ok_or_else(|| anyhow::anyhow!("missing <error>"))?;
            let code: u64 = error
                .get_attr("code")
                .ok_or_else(|| anyhow::anyhow!("missing code"))?
                .as_str()
                .parse()?;
            let text = error
                .get_attr("text")
                .ok_or_else(|| anyhow::anyhow!("missing text"))?
                .as_str()
                .to_string();
            let r#type_node = response
                .get_optional_child("type")
                .ok_or_else(|| anyhow::anyhow!("missing <type>"))?;
            let r#type = r#type_node
                .content_bytes()
                .map(|b| b.to_vec())
                .unwrap_or_default();
            let identity_node = response
                .get_optional_child("identity")
                .ok_or_else(|| anyhow::anyhow!("missing <identity>"))?;
            let identity = identity_node
                .content_bytes()
                .map(|b| b.to_vec())
                .unwrap_or_default();
            Ok(GetAndStoreIdentityKeysResponse {
                jid,
                children: children_items,
                code,
                text,
                r#type,
                identity,
                ..Default::default()
            })
        }
    }

    /// makeFetchKeyBundlesRequestKeyUser:get IQ spec.
    ///
    /// Source: `WASmaxOutPreKeysFetchKeyBundlesRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeFetchKeyBundlesRequestKeyUserSpec;

    impl IqSpec for MakeFetchKeyBundlesRequestKeyUserSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            let key_node = NodeBuilder::new("key").build();

            InfoQuery::get(
                ENCRYPT_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![key_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// makeFetchMissingPreKeysRequestKeyFetchUserDevice:get IQ spec.
    ///
    /// Source: `WASmaxOutPreKeysFetchMissingPreKeysRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeFetchMissingPreKeysRequestKeyFetchUserDeviceSpec;

    impl IqSpec for MakeFetchMissingPreKeysRequestKeyFetchUserDeviceSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            let key_fetch_node = NodeBuilder::new("key_fetch").build();

            InfoQuery::get(
                ENCRYPT_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![key_fetch_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// $4:set IQ spec.
    ///
    /// Source: `WAWebUploadPrekeysForRegTask`
    #[derive(Debug, Clone, Default)]
    pub struct UploadPrekeysForRegTaskSpec;

    impl IqSpec for UploadPrekeysForRegTaskSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            let registration_node = NodeBuilder::new("registration").build();
            let type_node = NodeBuilder::new("type").build();
            let identity_node = NodeBuilder::new("identity").build();
            let list_node = NodeBuilder::new("list").build();

            InfoQuery::set(
                ENCRYPT_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![
                    registration_node,
                    type_node,
                    identity_node,
                    list_node,
                ])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// rotateKey:set IQ spec.
    ///
    /// Source: `WAWebRotateKeyJob`
    #[derive(Debug, Clone, Default)]
    pub struct RotateKeySpec;

    impl IqSpec for RotateKeySpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            let rotate_node = NodeBuilder::new("rotate").build();

            InfoQuery::set(
                ENCRYPT_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![rotate_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// uploadPreKeys:set IQ spec.
    ///
    /// Source: `WAWebUploadPreKeysJob`
    #[derive(Debug, Clone, Default)]
    pub struct UploadPreKeysSpec;

    /// Response from uploadPreKeys:set.
    #[derive(Debug, Clone, Default)]
    pub struct UploadPreKeysResponse {
        pub r#type: String,
        pub code: u64,
        pub text: Option<String>,
    }

    impl IqSpec for UploadPreKeysSpec {
        type Response = UploadPreKeysResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let registration_node = NodeBuilder::new("registration").build();
            let type_node = NodeBuilder::new("type").build();
            let identity_node = NodeBuilder::new("identity").build();
            let list_node = NodeBuilder::new("list").build();

            InfoQuery::set(
                ENCRYPT_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![
                    registration_node,
                    type_node,
                    identity_node,
                    list_node,
                ])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            let error = response
                .get_optional_child("error")
                .ok_or_else(|| anyhow::anyhow!("missing <error>"))?;
            let code: u64 = error
                .get_attr("code")
                .ok_or_else(|| anyhow::anyhow!("missing code"))?
                .as_str()
                .parse()?;
            let text = error.get_attr("text").map(|v| v.as_str().to_string());
            Ok(UploadPreKeysResponse {
                r#type,
                code,
                text,
                ..Default::default()
            })
        }
    }
}

/// IQ namespace `fb:thrift_iq`. Source: WASmaxOutBizCtwaAdAccountGetAccessTokenAndSessionCookiesRequest, WASmaxOutBizLinkingGetAccountNonceRequest, WASmaxOutBizLinkingGetLinkedAccountsRequest, WASmaxOutBizAccessTokenRequestSilentNonceRequest, WASmaxOutBizCtwaAdAccountSendAccountRecoveryNonceRequest, WAWebQueryBusinessCategoriesJob, WAWebQueryCtwaContextJob, WASmaxOutSupportContactFormRequest, WASmaxOutBugReportingReportBugRequest, WASmaxOutSupportMessageFeedbackSendFeedbackRequest, WASmaxOutBizCtwaNativeAdUploadAdMediaRequest, WAWebBusinessProfileJob.
pub mod fb_thrift_iq {
    use crate::iq::spec::IqSpec;
    use crate::request::InfoQuery;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::jid::{Jid, Server};
    use wacore_binary::node::NodeContent;

    /// IQ namespace.
    pub const FB_THRIFT_IQ_NAMESPACE: &str = "fb:thrift_iq";

    // ─── Shared child types ──────────────────────────────────────────────────────

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct QueryBusinessCategoriesCategoryItem {
        pub id: String,
        pub content: String,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeUploadAdMediaRequestMediaMediaListItem {
        pub id: String,
        pub r#type: String,
    }

    // ─── IQ Specs ───────────────────────────────────────────────────────────────

    /// makeGetAccessTokenAndSessionCookiesRequest:get IQ spec.
    ///
    /// Source: `WASmaxOutBizCtwaAdAccountGetAccessTokenAndSessionCookiesRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetAccessTokenAndSessionCookiesRequestSpec;

    /// Response from makeGetAccessTokenAndSessionCookiesRequest:get.
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetAccessTokenAndSessionCookiesRequestResponse {
        pub access_token_element_value: String,
        pub session_cookies_element_value: String,
        pub business_person_id: String,
        pub to: Jid,
        pub r#type: String,
        pub element_value: String,
    }

    impl IqSpec for MakeGetAccessTokenAndSessionCookiesRequestSpec {
        type Response = MakeGetAccessTokenAndSessionCookiesRequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let code_node = NodeBuilder::new("code").build();
            let mut parameters_node = NodeBuilder::new("parameters");
            parameters_node = parameters_node.children([code_node]);
            let parameters_node = parameters_node.build();

            InfoQuery::get(
                FB_THRIFT_IQ_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![parameters_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let access_token_wrap = response
                .get_optional_child("access_token")
                .ok_or_else(|| anyhow::anyhow!("missing <access_token>"))?;
            let access_token_element_value = access_token_wrap
                .content_str()
                .unwrap_or_default()
                .to_string();
            let session_cookies_wrap = response
                .get_optional_child("session_cookies")
                .ok_or_else(|| anyhow::anyhow!("missing <session_cookies>"))?;
            let session_cookies_element_value = session_cookies_wrap
                .content_str()
                .unwrap_or_default()
                .to_string();
            let business_person_wrap = response
                .get_optional_child("business_person")
                .ok_or_else(|| anyhow::anyhow!("missing <business_person>"))?;
            let business_person_id = business_person_wrap
                .get_attr("id")
                .ok_or_else(|| anyhow::anyhow!("missing id"))?
                .as_str()
                .to_string();
            let to = response
                .get_attr("to")
                .and_then(|v| v.to_jid())
                .ok_or_else(|| anyhow::anyhow!("missing to"))?;
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            let token_type = response
                .get_optional_child("token_type")
                .ok_or_else(|| anyhow::anyhow!("missing <token_type>"))?;
            let element_value = token_type
                .get_attr("elementValue")
                .ok_or_else(|| anyhow::anyhow!("missing elementValue"))?
                .as_str()
                .to_string();
            Ok(MakeGetAccessTokenAndSessionCookiesRequestResponse {
                access_token_element_value,
                session_cookies_element_value,
                business_person_id,
                to,
                r#type,
                element_value,
                ..Default::default()
            })
        }
    }

    /// makeGetAccountNonceRequestIdentifier:get IQ spec.
    ///
    /// Source: `WASmaxOutBizLinkingGetAccountNonceRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetAccountNonceRequestIdentifierSpec;

    /// Response from makeGetAccountNonceRequestIdentifier:get.
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetAccountNonceRequestIdentifierResponse {
        pub detail_nonce_element_value: String,
        pub to: Jid,
        pub r#type: String,
        pub request: String,
    }

    impl IqSpec for MakeGetAccountNonceRequestIdentifierSpec {
        type Response = MakeGetAccountNonceRequestIdentifierResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::get(FB_THRIFT_IQ_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let detail_wrap = response
                .get_optional_child("detail")
                .ok_or_else(|| anyhow::anyhow!("missing <detail>"))?;
            let detail_nonce_wrap = detail_wrap
                .get_optional_child("nonce")
                .ok_or_else(|| anyhow::anyhow!("missing <nonce>"))?;
            let detail_nonce_element_value = detail_nonce_wrap
                .content_str()
                .unwrap_or_default()
                .to_string();
            let to = response
                .get_attr("to")
                .and_then(|v| v.to_jid())
                .ok_or_else(|| anyhow::anyhow!("missing to"))?;
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            let request_node = detail_wrap
                .get_optional_child("request")
                .ok_or_else(|| anyhow::anyhow!("missing <request>"))?;
            let request = request_node.content_str().unwrap_or_default().to_string();
            Ok(MakeGetAccountNonceRequestIdentifierResponse {
                detail_nonce_element_value,
                to,
                r#type,
                request,
                ..Default::default()
            })
        }
    }

    /// makeGetLinkedAccountsRequest:get IQ spec.
    ///
    /// Source: `WASmaxOutBizLinkingGetLinkedAccountsRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetLinkedAccountsRequestSpec;

    impl IqSpec for MakeGetLinkedAccountsRequestSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            let linked_accounts_node = NodeBuilder::new("linked_accounts").build();

            InfoQuery::get(
                FB_THRIFT_IQ_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![linked_accounts_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// makeRequestSilentNonceRequest:get IQ spec.
    ///
    /// Source: `WASmaxOutBizAccessTokenRequestSilentNonceRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeRequestSilentNonceRequestSpec;

    /// Response from makeRequestSilentNonceRequest:get.
    #[derive(Debug, Clone, Default)]
    pub struct MakeRequestSilentNonceRequestResponse {
        pub result_status: String,
        pub to: Jid,
        pub r#type: String,
    }

    impl IqSpec for MakeRequestSilentNonceRequestSpec {
        type Response = MakeRequestSilentNonceRequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::get(FB_THRIFT_IQ_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let result_wrap = response
                .get_optional_child("result")
                .ok_or_else(|| anyhow::anyhow!("missing <result>"))?;
            let result_status = result_wrap
                .get_attr("status")
                .ok_or_else(|| anyhow::anyhow!("missing status"))?
                .as_str()
                .to_string();
            let to = response
                .get_attr("to")
                .and_then(|v| v.to_jid())
                .ok_or_else(|| anyhow::anyhow!("missing to"))?;
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            Ok(MakeRequestSilentNonceRequestResponse {
                result_status,
                to,
                r#type,
                ..Default::default()
            })
        }
    }

    /// makeSendAccountRecoveryNonceRequest:get IQ spec.
    ///
    /// Source: `WASmaxOutBizCtwaAdAccountSendAccountRecoveryNonceRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeSendAccountRecoveryNonceRequestSpec;

    /// Response from makeSendAccountRecoveryNonceRequest:get.
    #[derive(Debug, Clone, Default)]
    pub struct MakeSendAccountRecoveryNonceRequestResponse {
        pub status_element_value: String,
        pub to: Jid,
        pub r#type: String,
    }

    impl IqSpec for MakeSendAccountRecoveryNonceRequestSpec {
        type Response = MakeSendAccountRecoveryNonceRequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::get(FB_THRIFT_IQ_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let to = response
                .get_attr("to")
                .and_then(|v| v.to_jid())
                .ok_or_else(|| anyhow::anyhow!("missing to"))?;
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            let result = response
                .get_optional_child("Result")
                .ok_or_else(|| anyhow::anyhow!("missing <Result>"))?;
            let status_element_value = result
                .get_attr("statusElementValue")
                .ok_or_else(|| anyhow::anyhow!("missing statusElementValue"))?
                .as_str()
                .to_string();
            Ok(MakeSendAccountRecoveryNonceRequestResponse {
                to,
                r#type,
                status_element_value,
                ..Default::default()
            })
        }
    }

    /// queryBusinessCategories:get IQ spec.
    ///
    /// Source: `WAWebQueryBusinessCategoriesJob`
    #[derive(Debug, Clone)]
    pub struct QueryBusinessCategoriesSpec {
        pub op: String,
    }

    impl QueryBusinessCategoriesSpec {
        pub fn new(op: impl Into<String>) -> Self {
            Self { op: op.into() }
        }
    }

    impl IqSpec for QueryBusinessCategoriesSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            let query_node = NodeBuilder::new("query").build();
            let mut request_node = NodeBuilder::new("request");
            request_node = request_node.attr("op", &*self.op);
            request_node = request_node.attr("type", "catkit");
            request_node = request_node.attr("v", "1");
            request_node = request_node.children([query_node]);
            let request_node = request_node.build();

            InfoQuery::get(
                FB_THRIFT_IQ_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![request_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// title:get IQ spec.
    ///
    /// Source: `WAWebQueryCtwaContextJob`
    #[derive(Debug, Clone, Default)]
    pub struct TitleSpec;

    /// Response from title:get.
    #[derive(Debug, Clone, Default)]
    pub struct TitleResponse {
        pub headline: String,
        pub body: String,
        pub source_app: String,
        pub greeting_message_body: String,
        pub automated_greeting_message_shown: String,
        pub cta_payload: String,
        pub original_image_url: String,
    }

    impl IqSpec for TitleSpec {
        type Response = TitleResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let account_number_node = NodeBuilder::new("account_number").build();
            let code_node = NodeBuilder::new("code").build();
            let expected_source_url_node = NodeBuilder::new("expected_source_url").build();

            InfoQuery::get(
                FB_THRIFT_IQ_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![
                    account_number_node,
                    code_node,
                    expected_source_url_node,
                ])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let headline_node = response
                .get_optional_child("headline")
                .ok_or_else(|| anyhow::anyhow!("missing <headline>"))?;
            let headline = headline_node.content_str().unwrap_or_default().to_string();
            let body_node = response
                .get_optional_child("body")
                .ok_or_else(|| anyhow::anyhow!("missing <body>"))?;
            let body = body_node.content_str().unwrap_or_default().to_string();
            let source_app_node = response
                .get_optional_child("sourceApp")
                .ok_or_else(|| anyhow::anyhow!("missing <sourceApp>"))?;
            let source_app = source_app_node
                .content_str()
                .unwrap_or_default()
                .to_string();
            let greeting_message_body_node = response
                .get_optional_child("greetingMessageBody")
                .ok_or_else(|| anyhow::anyhow!("missing <greetingMessageBody>"))?;
            let greeting_message_body = greeting_message_body_node
                .content_str()
                .unwrap_or_default()
                .to_string();
            let automated_greeting_message_shown_node = response
                .get_optional_child("automatedGreetingMessageShown")
                .ok_or_else(|| anyhow::anyhow!("missing <automatedGreetingMessageShown>"))?;
            let automated_greeting_message_shown = automated_greeting_message_shown_node
                .content_str()
                .unwrap_or_default()
                .to_string();
            let cta_payload_node = response
                .get_optional_child("ctaPayload")
                .ok_or_else(|| anyhow::anyhow!("missing <ctaPayload>"))?;
            let cta_payload = cta_payload_node
                .content_str()
                .unwrap_or_default()
                .to_string();
            let original_image_url_node = response
                .get_optional_child("originalImageUrl")
                .ok_or_else(|| anyhow::anyhow!("missing <originalImageUrl>"))?;
            let original_image_url = original_image_url_node
                .content_str()
                .unwrap_or_default()
                .to_string();
            Ok(TitleResponse {
                headline,
                body,
                source_app,
                greeting_message_body,
                automated_greeting_message_shown,
                cta_payload,
                original_image_url,
                ..Default::default()
            })
        }
    }

    /// makeContactFormRequestTopic:set IQ spec.
    ///
    /// Source: `WASmaxOutSupportContactFormRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeContactFormRequestTopicSpec;

    /// Response from makeContactFormRequestTopic:set.
    #[derive(Debug, Clone, Default)]
    pub struct MakeContactFormRequestTopicResponse {
        pub response_status: String,
        pub response_message_element_value: String,
        pub response_ticket_id_element_value: String,
        pub response_group_jid_element_value: String,
        pub to: Jid,
        pub r#type: String,
    }

    impl IqSpec for MakeContactFormRequestTopicSpec {
        type Response = MakeContactFormRequestTopicResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let description_node = NodeBuilder::new("description").build();

            InfoQuery::set(
                FB_THRIFT_IQ_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![description_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let response_wrap = response
                .get_optional_child("response")
                .ok_or_else(|| anyhow::anyhow!("missing <response>"))?;
            let response_status = response_wrap
                .get_attr("status")
                .ok_or_else(|| anyhow::anyhow!("missing status"))?
                .as_str()
                .to_string();
            let response_message_wrap = response_wrap
                .get_optional_child("message")
                .ok_or_else(|| anyhow::anyhow!("missing <message>"))?;
            let response_message_element_value = response_message_wrap
                .content_str()
                .unwrap_or_default()
                .to_string();
            let response_ticket_id_wrap = response_wrap
                .get_optional_child("ticket_id")
                .ok_or_else(|| anyhow::anyhow!("missing <ticket_id>"))?;
            let response_ticket_id_element_value = response_ticket_id_wrap
                .content_str()
                .unwrap_or_default()
                .to_string();
            let response_group_jid_wrap = response_wrap
                .get_optional_child("group_jid")
                .ok_or_else(|| anyhow::anyhow!("missing <group_jid>"))?;
            let response_group_jid_element_value = response_group_jid_wrap
                .content_str()
                .unwrap_or_default()
                .to_string();
            let to = response
                .get_attr("to")
                .and_then(|v| v.to_jid())
                .ok_or_else(|| anyhow::anyhow!("missing to"))?;
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            Ok(MakeContactFormRequestTopicResponse {
                response_status,
                response_message_element_value,
                response_ticket_id_element_value,
                response_group_jid_element_value,
                to,
                r#type,
                ..Default::default()
            })
        }
    }

    /// makeReportBugRequestDeviceLogHandle:set IQ spec.
    ///
    /// Source: `WASmaxOutBugReportingReportBugRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeReportBugRequestDeviceLogHandleSpec;

    /// Response from makeReportBugRequestDeviceLogHandle:set.
    #[derive(Debug, Clone, Default)]
    pub struct MakeReportBugRequestDeviceLogHandleResponse {
        pub task_id_element_value: String,
        pub to: Jid,
        pub r#type: String,
    }

    impl IqSpec for MakeReportBugRequestDeviceLogHandleSpec {
        type Response = MakeReportBugRequestDeviceLogHandleResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::set(FB_THRIFT_IQ_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let task_id_wrap = response
                .get_optional_child("task_id")
                .ok_or_else(|| anyhow::anyhow!("missing <task_id>"))?;
            let task_id_element_value = task_id_wrap.content_str().unwrap_or_default().to_string();
            let to = response
                .get_attr("to")
                .and_then(|v| v.to_jid())
                .ok_or_else(|| anyhow::anyhow!("missing to"))?;
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            Ok(MakeReportBugRequestDeviceLogHandleResponse {
                task_id_element_value,
                to,
                r#type,
                ..Default::default()
            })
        }
    }

    /// makeSendFeedbackRequestFeedbackListFeedback:set IQ spec.
    ///
    /// Source: `WASmaxOutSupportMessageFeedbackSendFeedbackRequest`
    #[derive(Debug, Clone)]
    pub struct MakeSendFeedbackRequestFeedbackListFeedbackSpec {
        pub id: String,
    }

    impl MakeSendFeedbackRequestFeedbackListFeedbackSpec {
        pub fn new(id: impl Into<String>) -> Self {
            Self { id: id.into() }
        }
    }

    /// Response from makeSendFeedbackRequestFeedbackListFeedback:set.
    #[derive(Debug, Clone, Default)]
    pub struct MakeSendFeedbackRequestFeedbackListFeedbackResponse {
        pub result_status: String,
        pub to: Jid,
        pub r#type: String,
    }

    impl IqSpec for MakeSendFeedbackRequestFeedbackListFeedbackSpec {
        type Response = MakeSendFeedbackRequestFeedbackListFeedbackResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut message_node = NodeBuilder::new("message");
            message_node = message_node.attr("id", &*self.id);
            let message_node = message_node.build();
            let feedback_list_node = NodeBuilder::new("feedback_list").build();

            InfoQuery::set(
                FB_THRIFT_IQ_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![message_node, feedback_list_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let result_wrap = response
                .get_optional_child("result")
                .ok_or_else(|| anyhow::anyhow!("missing <result>"))?;
            let result_status = result_wrap
                .get_attr("status")
                .ok_or_else(|| anyhow::anyhow!("missing status"))?
                .as_str()
                .to_string();
            let to = response
                .get_attr("to")
                .and_then(|v| v.to_jid())
                .ok_or_else(|| anyhow::anyhow!("missing to"))?;
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            Ok(MakeSendFeedbackRequestFeedbackListFeedbackResponse {
                result_status,
                to,
                r#type,
                ..Default::default()
            })
        }
    }

    /// makeUploadAdMediaRequestMedia:set IQ spec.
    ///
    /// Source: `WASmaxOutBizCtwaNativeAdUploadAdMediaRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeUploadAdMediaRequestMediaSpec;

    impl IqSpec for MakeUploadAdMediaRequestMediaSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::set(FB_THRIFT_IQ_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// updateCartEnabled:set IQ spec.
    ///
    /// Source: `WAWebBusinessProfileJob`
    #[derive(Debug, Clone)]
    pub struct UpdateCartEnabledSpec {
        pub enabled: String,
    }

    impl UpdateCartEnabledSpec {
        pub fn new(enabled: impl Into<String>) -> Self {
            Self {
                enabled: enabled.into(),
            }
        }
    }

    impl IqSpec for UpdateCartEnabledSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut cart_node = NodeBuilder::new("cart");
            cart_node = cart_node.attr("enabled", &*self.enabled);
            let cart_node = cart_node.build();
            let mut commerce_settings_node = NodeBuilder::new("commerce_settings");
            commerce_settings_node = commerce_settings_node.children([cart_node]);
            let commerce_settings_node = commerce_settings_node.build();

            InfoQuery::set(
                FB_THRIFT_IQ_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![commerce_settings_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }
}

/// IQ namespace `md`. Source: WASmaxOutMdGetCountryCodeRequest, WASmaxOutMdGetPasskeyRequestOptionsRequest, WASmaxOutMdGetRefRequest, WASmaxOutMdSetCompanionNonceRequest, WASmaxOutMdSetEncryptedPairingRequestRequest, WAWebUnpairDeviceJob.
pub mod md {
    use crate::iq::spec::IqSpec;
    use crate::request::InfoQuery;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::jid::{Jid, Server};
    use wacore_binary::node::NodeContent;

    /// IQ namespace.
    pub const MD_NAMESPACE: &str = "md";

    // ─── IQ Specs ───────────────────────────────────────────────────────────────

    /// makeGetCountryCodeRequest:get IQ spec.
    ///
    /// Source: `WASmaxOutMdGetCountryCodeRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetCountryCodeRequestSpec;

    /// Response from makeGetCountryCodeRequest:get.
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetCountryCodeRequestResponse {
        pub country_code_iso: String,
        pub r#type: String,
    }

    impl IqSpec for MakeGetCountryCodeRequestSpec {
        type Response = MakeGetCountryCodeRequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut link_code_companion_reg_node = NodeBuilder::new("link_code_companion_reg");
            link_code_companion_reg_node =
                link_code_companion_reg_node.attr("stage", "get_country_code");
            let link_code_companion_reg_node = link_code_companion_reg_node.build();

            InfoQuery::get(
                MD_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![link_code_companion_reg_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let country_code_wrap = response
                .get_optional_child("country_code")
                .ok_or_else(|| anyhow::anyhow!("missing <country_code>"))?;
            let country_code_iso = country_code_wrap
                .get_attr("iso")
                .ok_or_else(|| anyhow::anyhow!("missing iso"))?
                .as_str()
                .to_string();
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            Ok(MakeGetCountryCodeRequestResponse {
                country_code_iso,
                r#type,
                ..Default::default()
            })
        }
    }

    /// makeGetPasskeyRequestOptionsRequest:get IQ spec.
    ///
    /// Source: `WASmaxOutMdGetPasskeyRequestOptionsRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetPasskeyRequestOptionsRequestSpec;

    /// Response from makeGetPasskeyRequestOptionsRequest:get.
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetPasskeyRequestOptionsRequestResponse {
        pub passkey_request_options: Vec<u8>,
        pub r#type: String,
    }

    impl IqSpec for MakeGetPasskeyRequestOptionsRequestSpec {
        type Response = MakeGetPasskeyRequestOptionsRequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let passkey_request_options_node = NodeBuilder::new("passkey_request_options").build();

            InfoQuery::get(
                MD_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![passkey_request_options_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            let passkey_request_options_node = response
                .get_optional_child("passkey_request_options")
                .ok_or_else(|| anyhow::anyhow!("missing <passkey_request_options>"))?;
            let passkey_request_options = passkey_request_options_node
                .content_bytes()
                .map(|b| b.to_vec())
                .unwrap_or_default();
            Ok(MakeGetPasskeyRequestOptionsRequestResponse {
                r#type,
                passkey_request_options,
                ..Default::default()
            })
        }
    }

    /// makeGetRefRequest:get IQ spec.
    ///
    /// Source: `WASmaxOutMdGetRefRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetRefRequestSpec;

    /// Response from makeGetRefRequest:get.
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetRefRequestResponse {
        pub ref_element_value: Vec<u8>,
        pub r#type: String,
    }

    impl IqSpec for MakeGetRefRequestSpec {
        type Response = MakeGetRefRequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let ref_node = NodeBuilder::new("ref").build();

            InfoQuery::get(
                MD_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![ref_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let ref_wrap = response
                .get_optional_child("ref")
                .ok_or_else(|| anyhow::anyhow!("missing <ref>"))?;
            let ref_element_value = ref_wrap
                .content_bytes()
                .map(|b| b.to_vec())
                .unwrap_or_default();
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            Ok(MakeGetRefRequestResponse {
                ref_element_value,
                r#type,
                ..Default::default()
            })
        }
    }

    /// makeSetCompanionNonceRequest:set IQ spec.
    ///
    /// Source: `WASmaxOutMdSetCompanionNonceRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeSetCompanionNonceRequestSpec;

    /// Response from makeSetCompanionNonceRequest:set.
    #[derive(Debug, Clone, Default)]
    pub struct MakeSetCompanionNonceRequestResponse {
        pub r#type: String,
    }

    impl IqSpec for MakeSetCompanionNonceRequestSpec {
        type Response = MakeSetCompanionNonceRequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let companion_nonce_node = NodeBuilder::new("companion_nonce").build();

            InfoQuery::set(
                MD_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![companion_nonce_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            Ok(MakeSetCompanionNonceRequestResponse {
                r#type,
                ..Default::default()
            })
        }
    }

    /// makeSetEncryptedPairingRequestRequest:set IQ spec.
    ///
    /// Source: `WASmaxOutMdSetEncryptedPairingRequestRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeSetEncryptedPairingRequestRequestSpec;

    /// Response from makeSetEncryptedPairingRequestRequest:set.
    #[derive(Debug, Clone, Default)]
    pub struct MakeSetEncryptedPairingRequestRequestResponse {
        pub r#type: String,
    }

    impl IqSpec for MakeSetEncryptedPairingRequestRequestSpec {
        type Response = MakeSetEncryptedPairingRequestRequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let encrypted_pairing_request_node =
                NodeBuilder::new("encrypted_pairing_request").build();

            InfoQuery::set(
                MD_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![encrypted_pairing_request_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            Ok(MakeSetEncryptedPairingRequestRequestResponse {
                r#type,
                ..Default::default()
            })
        }
    }

    /// unpairDevice:set IQ spec.
    ///
    /// Source: `WAWebUnpairDeviceJob`
    #[derive(Debug, Clone)]
    pub struct UnpairDeviceSpec {
        pub jid: Jid,
        pub reason: String,
    }

    impl UnpairDeviceSpec {
        pub fn new(jid: &Jid, reason: impl Into<String>) -> Self {
            Self {
                jid: jid.clone(),
                reason: reason.into(),
            }
        }
    }

    /// Response from unpairDevice:set.
    #[derive(Debug, Clone, Default)]
    pub struct UnpairDeviceResponse {
        pub r#type: String,
        pub code: u64,
    }

    impl IqSpec for UnpairDeviceSpec {
        type Response = UnpairDeviceResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut remove_companion_device_node = NodeBuilder::new("remove-companion-device");
            remove_companion_device_node =
                remove_companion_device_node.attr("jid", self.jid.clone());
            remove_companion_device_node =
                remove_companion_device_node.attr("reason", &*self.reason);
            let remove_companion_device_node = remove_companion_device_node.build();

            InfoQuery::set(
                MD_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![remove_companion_device_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            let error = response
                .get_optional_child("error")
                .ok_or_else(|| anyhow::anyhow!("missing <error>"))?;
            let code: u64 = error
                .get_attr("code")
                .ok_or_else(|| anyhow::anyhow!("missing code"))?
                .as_str()
                .parse()?;
            Ok(UnpairDeviceResponse {
                r#type,
                code,
                ..Default::default()
            })
        }
    }
}

/// IQ namespace `newsletter`. Source: WASmaxOutNewslettersGetNewsletterMessageUpdatesRequest, WASmaxOutNewslettersGetNewsletterMessagesRequest, WASmaxOutNewslettersGetNewsletterResponsesRequest, WASmaxOutNewslettersGetNewsletterStatusUpdatesRequest, WASmaxOutNewslettersGetNewsletterStatusesRequest, WASmaxOutNewslettersMyAddOnsRequest, WASmaxOutNewslettersNewsletterIQGetRequestMixin, WASmaxOutNewslettersNewsletterMessageRequestIQPayloadMixin, WASmaxOutNewslettersNewsletterStatusRequestIQPayloadMixin, WASmaxOutNewslettersSelfIQGetRequestMixin, WASmaxOutNewslettersSubscribeToLiveUpdatesRequest, WASmaxOutNewslettersNewsletterIQSetRequestMixin.
pub mod newsletter {
    use crate::iq::spec::IqSpec;
    use crate::request::InfoQuery;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::jid::{Jid, Server};
    use wacore_binary::node::NodeContent;

    /// IQ namespace.
    pub const NEWSLETTER_NAMESPACE: &str = "newsletter";

    // ─── Shared child types ──────────────────────────────────────────────────────

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetNewsletterMessageUpdatesRequestReactionItem {
        pub code: Option<String>,
        pub count: Option<u64>,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetNewsletterMessageUpdatesRequestVoteItem {
        pub count: Option<u64>,
        pub element_value: Vec<u8>,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetNewsletterMessageUpdatesRequestMessageItem {
        pub id: Option<String>,
        pub server_id: u64,
        pub t: Option<u64>,
        pub is_sender: Option<String>,
        pub meta_original_msg_t: Option<u64>,
        pub meta_msg_edit_t: Option<u64>,
        pub name_element_value: String,
        pub picture_id: Option<String>,
        pub picture_direct_path: Option<String>,
        pub responses_count_count: Option<u64>,
        pub plaintext_mediatype: Option<String>,
        pub rcat_element_value: Vec<u8>,
        pub r#type: Option<String>,
        pub forwards_count_count: Option<u64>,
        pub reaction: Vec<MakeGetNewsletterMessageUpdatesRequestReactionItem>,
        pub vote: Vec<MakeGetNewsletterMessageUpdatesRequestVoteItem>,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetNewsletterMessagesRequestReactionItem {
        pub code: Option<String>,
        pub count: Option<u64>,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetNewsletterMessagesRequestVoteItem {
        pub count: Option<u64>,
        pub element_value: Vec<u8>,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetNewsletterMessagesRequestMessageItem {
        pub id: Option<String>,
        pub server_id: u64,
        pub t: Option<u64>,
        pub is_sender: Option<String>,
        pub meta_original_msg_t: Option<u64>,
        pub meta_msg_edit_t: Option<u64>,
        pub name_element_value: String,
        pub picture_id: Option<String>,
        pub picture_direct_path: Option<String>,
        pub responses_count_count: Option<u64>,
        pub plaintext_mediatype: Option<String>,
        pub rcat_element_value: Vec<u8>,
        pub r#type: Option<String>,
        pub forwards_count_count: Option<u64>,
        pub reaction: Vec<MakeGetNewsletterMessagesRequestReactionItem>,
        pub vote: Vec<MakeGetNewsletterMessagesRequestVoteItem>,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetNewsletterResponsesRequestQuestionResponseItem {
        pub message_id: String,
        pub message_t: u64,
        pub message_is_sender: Option<String>,
        pub sender_lid: Jid,
        pub sender_notify_name: Option<String>,
        pub sender_picture_direct_path: String,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetNewsletterStatusUpdatesRequestReactionItem {
        pub code: Option<String>,
        pub count: Option<u64>,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetNewsletterStatusUpdatesRequestStatusItem {
        pub id: Option<String>,
        pub server_id: u64,
        pub t: Option<u64>,
        pub is_sender: Option<String>,
        pub meta_original_msg_t: Option<u64>,
        pub meta_msg_edit_t: Option<u64>,
        pub views_count_type: Option<String>,
        pub views_count_count: Option<u64>,
        pub reaction: Vec<MakeGetNewsletterStatusUpdatesRequestReactionItem>,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetNewsletterStatusesRequestReactionItem {
        pub code: Option<String>,
        pub count: Option<u64>,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetNewsletterStatusesRequestStatusItem {
        pub id: Option<String>,
        pub server_id: u64,
        pub t: Option<u64>,
        pub is_sender: Option<String>,
        pub meta_original_msg_t: Option<u64>,
        pub meta_msg_edit_t: Option<u64>,
        pub views_count_type: Option<String>,
        pub views_count_count: Option<u64>,
        pub reaction: Vec<MakeGetNewsletterStatusesRequestReactionItem>,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeMyAddOnsRequestMessageItem {
        pub server_id: u64,
        pub reaction_code: Option<String>,
        pub reaction_t: Option<u64>,
        pub votes_t: Option<u64>,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeMyAddOnsRequestMessagesItem {
        pub jid: Jid,
        pub message: Vec<MakeMyAddOnsRequestMessageItem>,
    }

    // ─── IQ Specs ───────────────────────────────────────────────────────────────

    /// makeGetNewsletterMessageUpdatesRequest:get IQ spec.
    ///
    /// Source: `WASmaxOutNewslettersGetNewsletterMessageUpdatesRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetNewsletterMessageUpdatesRequestSpec;

    impl IqSpec for MakeGetNewsletterMessageUpdatesRequestSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::get(NEWSLETTER_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// makeGetNewsletterMessagesRequest:get IQ spec.
    ///
    /// Source: `WASmaxOutNewslettersGetNewsletterMessagesRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetNewsletterMessagesRequestSpec;

    impl IqSpec for MakeGetNewsletterMessagesRequestSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::get(NEWSLETTER_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// makeGetNewsletterResponsesRequest:get IQ spec.
    ///
    /// Source: `WASmaxOutNewslettersGetNewsletterResponsesRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetNewsletterResponsesRequestSpec;

    impl IqSpec for MakeGetNewsletterResponsesRequestSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::get(NEWSLETTER_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// makeGetNewsletterStatusUpdatesRequest:get IQ spec.
    ///
    /// Source: `WASmaxOutNewslettersGetNewsletterStatusUpdatesRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetNewsletterStatusUpdatesRequestSpec;

    impl IqSpec for MakeGetNewsletterStatusUpdatesRequestSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::get(NEWSLETTER_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// makeGetNewsletterStatusesRequest:get IQ spec.
    ///
    /// Source: `WASmaxOutNewslettersGetNewsletterStatusesRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetNewsletterStatusesRequestSpec;

    impl IqSpec for MakeGetNewsletterStatusesRequestSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::get(NEWSLETTER_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// makeMyAddOnsRequest:get IQ spec.
    ///
    /// Source: `WASmaxOutNewslettersMyAddOnsRequest`
    #[derive(Debug, Clone)]
    pub struct MakeMyAddOnsRequestSpec {
        pub limit: u64,
        pub jid: Option<String>,
    }

    impl MakeMyAddOnsRequestSpec {
        pub fn new(limit: u64, jid: Option<String>) -> Self {
            Self { limit, jid }
        }
    }

    impl IqSpec for MakeMyAddOnsRequestSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut my_addons_node = NodeBuilder::new("my_addons");
            my_addons_node = my_addons_node.attr("limit", self.limit.to_string());
            if let Some(v) = &self.jid {
                my_addons_node = my_addons_node.attr("jid", v.as_str());
            }
            let my_addons_node = my_addons_node.build();

            InfoQuery::get(
                NEWSLETTER_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![my_addons_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// mergeNewsletterIQGetRequestMixin:get IQ spec.
    ///
    /// Source: `WASmaxOutNewslettersNewsletterIQGetRequestMixin`
    #[derive(Debug, Clone, Default)]
    pub struct MergeNewsletterIQGetRequestMixinSpec;

    impl IqSpec for MergeNewsletterIQGetRequestMixinSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::get(NEWSLETTER_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// mergeNewsletterMessageRequestIQPayloadMixin:get IQ spec.
    ///
    /// Source: `WASmaxOutNewslettersNewsletterMessageRequestIQPayloadMixin`
    #[derive(Debug, Clone, Default)]
    pub struct MergeNewsletterMessageRequestIQPayloadMixinSpec;

    impl IqSpec for MergeNewsletterMessageRequestIQPayloadMixinSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            let messages_node = NodeBuilder::new("messages").build();

            InfoQuery::get(
                NEWSLETTER_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![messages_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// mergeNewsletterStatusRequestIQPayloadMixin:get IQ spec.
    ///
    /// Source: `WASmaxOutNewslettersNewsletterStatusRequestIQPayloadMixin`
    #[derive(Debug, Clone, Default)]
    pub struct MergeNewsletterStatusRequestIQPayloadMixinSpec;

    impl IqSpec for MergeNewsletterStatusRequestIQPayloadMixinSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            let statuses_node = NodeBuilder::new("statuses").build();

            InfoQuery::get(
                NEWSLETTER_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![statuses_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// mergeSelfIQGetRequestMixin:get IQ spec.
    ///
    /// Source: `WASmaxOutNewslettersSelfIQGetRequestMixin`
    #[derive(Debug, Clone, Default)]
    pub struct MergeSelfIQGetRequestMixinSpec;

    impl IqSpec for MergeSelfIQGetRequestMixinSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::get(NEWSLETTER_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// makeSubscribeToLiveUpdatesRequest:set IQ spec.
    ///
    /// Source: `WASmaxOutNewslettersSubscribeToLiveUpdatesRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeSubscribeToLiveUpdatesRequestSpec;

    /// Response from makeSubscribeToLiveUpdatesRequest:set.
    #[derive(Debug, Clone, Default)]
    pub struct MakeSubscribeToLiveUpdatesRequestResponse {
        pub live_updates_duration: u64,
        pub r#type: String,
    }

    impl IqSpec for MakeSubscribeToLiveUpdatesRequestSpec {
        type Response = MakeSubscribeToLiveUpdatesRequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let live_updates_node = NodeBuilder::new("live_updates").build();

            InfoQuery::set(
                NEWSLETTER_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![live_updates_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let live_updates_wrap = response
                .get_optional_child("live_updates")
                .ok_or_else(|| anyhow::anyhow!("missing <live_updates>"))?;
            let live_updates_duration: u64 = live_updates_wrap
                .get_attr("duration")
                .ok_or_else(|| anyhow::anyhow!("missing duration"))?
                .as_str()
                .parse()?;
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            Ok(MakeSubscribeToLiveUpdatesRequestResponse {
                live_updates_duration,
                r#type,
                ..Default::default()
            })
        }
    }

    /// mergeNewsletterIQSetRequestMixin:set IQ spec.
    ///
    /// Source: `WASmaxOutNewslettersNewsletterIQSetRequestMixin`
    #[derive(Debug, Clone, Default)]
    pub struct MergeNewsletterIQSetRequestMixinSpec;

    impl IqSpec for MergeNewsletterIQSetRequestMixinSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::set(NEWSLETTER_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }
}

/// IQ namespace `optoutlist`. Source: WASmaxOutBlocklistsGetOptOutListRequest, WASmaxOutBlocklistsUpdateOptOutListRequest.
pub mod optoutlist {
    use crate::iq::spec::IqSpec;
    use crate::request::InfoQuery;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::jid::{Jid, Server};
    use wacore_binary::node::NodeContent;

    /// IQ namespace.
    pub const OPTOUTLIST_NAMESPACE: &str = "optoutlist";

    // ─── Shared child types ──────────────────────────────────────────────────────

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetOptOutListRequestItemItemItem {
        pub action: Option<String>,
        pub category: Option<String>,
        pub expiry_at: Option<u64>,
    }

    // ─── IQ Specs ───────────────────────────────────────────────────────────────

    /// makeGetOptOutListRequestItem:get IQ spec.
    ///
    /// Source: `WASmaxOutBlocklistsGetOptOutListRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetOptOutListRequestItemSpec;

    impl IqSpec for MakeGetOptOutListRequestItemSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::get(OPTOUTLIST_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// makeUpdateOptOutListRequest:set IQ spec.
    ///
    /// Source: `WASmaxOutBlocklistsUpdateOptOutListRequest`
    #[derive(Debug, Clone)]
    pub struct MakeUpdateOptOutListRequestSpec {
        pub jid: Jid,
        pub category: String,
        pub action: String,
        pub dhash: String,
        pub reason: Option<String>,
        pub entry_point: Option<String>,
        pub signup_id: Option<String>,
        pub duration: Option<String>,
    }

    impl MakeUpdateOptOutListRequestSpec {
        pub fn new(
            jid: &Jid,
            category: impl Into<String>,
            action: impl Into<String>,
            dhash: impl Into<String>,
            reason: Option<String>,
            entry_point: Option<String>,
            signup_id: Option<String>,
            duration: Option<String>,
        ) -> Self {
            Self {
                jid: jid.clone(),
                category: category.into(),
                action: action.into(),
                dhash: dhash.into(),
                reason,
                entry_point,
                signup_id,
                duration,
            }
        }
    }

    /// Response from makeUpdateOptOutListRequest:set.
    #[derive(Debug, Clone, Default)]
    pub struct MakeUpdateOptOutListRequestResponse {
        pub r#type: String,
        pub list_matched: String,
        pub list_dhash: String,
        pub action: Option<String>,
        pub category: Option<String>,
        pub expiry_at: Option<u64>,
    }

    impl IqSpec for MakeUpdateOptOutListRequestSpec {
        type Response = MakeUpdateOptOutListRequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut item_node = NodeBuilder::new("item");
            item_node = item_node.attr("jid", self.jid.clone());
            item_node = item_node.attr("category", &*self.category);
            item_node = item_node.attr("action", &*self.action);
            item_node = item_node.attr("dhash", &*self.dhash);
            if let Some(v) = &self.reason {
                item_node = item_node.attr("reason", v.as_str());
            }
            if let Some(v) = &self.entry_point {
                item_node = item_node.attr("entry_point", v.as_str());
            }
            if let Some(v) = &self.signup_id {
                item_node = item_node.attr("signup_id", v.as_str());
            }
            if let Some(v) = &self.duration {
                item_node = item_node.attr("duration", v.as_str());
            }
            let item_node = item_node.build();

            InfoQuery::set(
                OPTOUTLIST_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![item_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            let list_wrap = response
                .get_optional_child("list")
                .ok_or_else(|| anyhow::anyhow!("missing <list>"))?;
            let list_matched = list_wrap
                .get_attr("matched")
                .ok_or_else(|| anyhow::anyhow!("missing matched"))?
                .as_str()
                .to_string();
            let list_dhash = list_wrap
                .get_attr("dhash")
                .ok_or_else(|| anyhow::anyhow!("missing dhash"))?
                .as_str()
                .to_string();
            let item = list_wrap
                .get_optional_child("item")
                .ok_or_else(|| anyhow::anyhow!("missing <item>"))?;
            let action = item.get_attr("action").map(|v| v.as_str().to_string());
            let category = item.get_attr("category").map(|v| v.as_str().to_string());
            let expiry_at = item
                .get_attr("expiry_at")
                .and_then(|v| v.as_str().parse().ok());
            Ok(MakeUpdateOptOutListRequestResponse {
                r#type,
                list_matched,
                list_dhash,
                action,
                category,
                expiry_at,
                ..Default::default()
            })
        }
    }
}

/// IQ namespace `passive`. Source: WASmaxOutPassiveModeActiveIQRequest, WASmaxOutPassiveModePassiveIQRequest.
pub mod passive {
    use crate::iq::spec::IqSpec;
    use crate::request::InfoQuery;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::jid::{Jid, Server};
    use wacore_binary::node::NodeContent;

    /// IQ namespace.
    pub const PASSIVE_NAMESPACE: &str = "passive";

    // ─── IQ Specs ───────────────────────────────────────────────────────────────

    /// makeActiveIQRequest:set IQ spec.
    ///
    /// Source: `WASmaxOutPassiveModeActiveIQRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeActiveIQRequestSpec;

    /// Response from makeActiveIQRequest:set.
    #[derive(Debug, Clone, Default)]
    pub struct MakeActiveIQRequestResponse {
        pub r#type: String,
        pub from: Jid,
    }

    impl IqSpec for MakeActiveIQRequestSpec {
        type Response = MakeActiveIQRequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let active_node = NodeBuilder::new("active").build();

            InfoQuery::set(
                PASSIVE_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![active_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            let from = response
                .get_attr("from")
                .and_then(|v| v.to_jid())
                .ok_or_else(|| anyhow::anyhow!("missing from"))?;
            Ok(MakeActiveIQRequestResponse {
                r#type,
                from,
                ..Default::default()
            })
        }
    }

    /// makePassiveIQRequest:set IQ spec.
    ///
    /// Source: `WASmaxOutPassiveModePassiveIQRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakePassiveIQRequestSpec;

    /// Response from makePassiveIQRequest:set.
    #[derive(Debug, Clone, Default)]
    pub struct MakePassiveIQRequestResponse {
        pub r#type: String,
        pub from: Jid,
    }

    impl IqSpec for MakePassiveIQRequestSpec {
        type Response = MakePassiveIQRequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let passive_node = NodeBuilder::new("passive").build();

            InfoQuery::set(
                PASSIVE_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![passive_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            let from = response
                .get_attr("from")
                .and_then(|v| v.to_jid())
                .ok_or_else(|| anyhow::anyhow!("missing from"))?;
            Ok(MakePassiveIQRequestResponse {
                r#type,
                from,
                ..Default::default()
            })
        }
    }
}

/// IQ namespace `privacy`. Source: WASmaxOutPrivacyGetContactBlacklistRequest, WASmaxOutPrivacyGetIQMixin, WAWebQueryPrivacyDisallowedListPnJob, WAWebQueryPrivacySettingsJob, WAWebSetPrivacyJob, WAWebSetPrivacyTokensJob, WAWebSetReadReceiptJob.
pub mod privacy {
    use crate::iq::spec::IqSpec;
    use crate::request::InfoQuery;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::jid::{Jid, Server};
    use wacore_binary::node::NodeContent;

    /// IQ namespace.
    pub const PRIVACY_NAMESPACE: &str = "privacy";

    // ─── Shared child types ──────────────────────────────────────────────────────

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetContactBlacklistRequestUserItem {
        pub jid: Jid,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct QueryPrivacyDisallowedListPnChildrenItem {
        pub jid: Jid,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct ReadReceiptsCategoryItem {
        pub name: String,
        pub value: String,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct PrivacyUserActionCategoryItem {
        pub value: String,
        pub name: String,
        pub dhash: Option<String>,
    }

    // ─── IQ Specs ───────────────────────────────────────────────────────────────

    /// makeGetContactBlacklistRequest:get IQ spec.
    ///
    /// Source: `WASmaxOutPrivacyGetContactBlacklistRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetContactBlacklistRequestSpec;

    /// Response from makeGetContactBlacklistRequest:get.
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetContactBlacklistRequestResponse {
        pub privacy_addressing_mode: String,
        pub from: Jid,
        pub r#type: String,
        pub dhash: String,
        pub user: Vec<MakeGetContactBlacklistRequestUserItem>,
    }

    impl IqSpec for MakeGetContactBlacklistRequestSpec {
        type Response = MakeGetContactBlacklistRequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::get(PRIVACY_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let privacy_wrap = response
                .get_optional_child("privacy")
                .ok_or_else(|| anyhow::anyhow!("missing <privacy>"))?;
            let privacy_addressing_mode = privacy_wrap
                .get_attr("addressing_mode")
                .ok_or_else(|| anyhow::anyhow!("missing addressing_mode"))?
                .as_str()
                .to_string();
            let from = response
                .get_attr("from")
                .and_then(|v| v.to_jid())
                .ok_or_else(|| anyhow::anyhow!("missing from"))?;
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            let list = privacy_wrap
                .get_optional_child("list")
                .ok_or_else(|| anyhow::anyhow!("missing <list>"))?;
            let dhash = list
                .get_attr("dhash")
                .ok_or_else(|| anyhow::anyhow!("missing dhash"))?
                .as_str()
                .to_string();
            let mut user_items = Vec::new();
            for child in list.get_children_by_tag("user") {
                let jid = child
                    .get_attr("jid")
                    .and_then(|v| v.to_jid())
                    .ok_or_else(|| anyhow::anyhow!("missing jid"))?;
                user_items.push(MakeGetContactBlacklistRequestUserItem {
                    jid,
                    ..Default::default()
                });
            }
            Ok(MakeGetContactBlacklistRequestResponse {
                privacy_addressing_mode,
                from,
                r#type,
                dhash,
                user: user_items,
                ..Default::default()
            })
        }
    }

    /// mergeGetIQMixin:get IQ spec.
    ///
    /// Source: `WASmaxOutPrivacyGetIQMixin`
    #[derive(Debug, Clone, Default)]
    pub struct MergeGetIQMixinSpec;

    impl IqSpec for MergeGetIQMixinSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::get(PRIVACY_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// queryPrivacyDisallowedListPn:get IQ spec.
    ///
    /// Source: `WAWebQueryPrivacyDisallowedListPnJob`
    #[derive(Debug, Clone)]
    pub struct QueryPrivacyDisallowedListPnSpec {
        pub name: String,
        pub value: String,
    }

    impl QueryPrivacyDisallowedListPnSpec {
        pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
            Self {
                name: name.into(),
                value: value.into(),
            }
        }
    }

    /// Response from queryPrivacyDisallowedListPn:get.
    #[derive(Debug, Clone, Default)]
    pub struct QueryPrivacyDisallowedListPnResponse {
        pub dhash: String,
        pub children: Vec<QueryPrivacyDisallowedListPnChildrenItem>,
        pub jid: Jid,
    }

    impl IqSpec for QueryPrivacyDisallowedListPnSpec {
        type Response = QueryPrivacyDisallowedListPnResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut list_node = NodeBuilder::new("list");
            list_node = list_node.attr("name", &*self.name);
            list_node = list_node.attr("value", &*self.value);
            let list_node = list_node.build();
            let mut privacy_node = NodeBuilder::new("privacy");
            privacy_node = privacy_node.children([list_node]);
            let privacy_node = privacy_node.build();

            InfoQuery::get(
                PRIVACY_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![privacy_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let jid = response
                .get_attr("jid")
                .and_then(|v| v.to_jid())
                .ok_or_else(|| anyhow::anyhow!("missing jid"))?;
            let list = response
                .get_optional_child("list")
                .ok_or_else(|| anyhow::anyhow!("missing <list>"))?;
            let dhash = list
                .get_attr("dhash")
                .ok_or_else(|| anyhow::anyhow!("missing dhash"))?
                .as_str()
                .to_string();
            let mut children_items = Vec::new();
            for child in list.get_children_by_tag("children") {
                let jid = child
                    .get_attr("jid")
                    .and_then(|v| v.to_jid())
                    .ok_or_else(|| anyhow::anyhow!("missing jid"))?;
                children_items.push(QueryPrivacyDisallowedListPnChildrenItem {
                    jid,
                    ..Default::default()
                });
            }
            Ok(QueryPrivacyDisallowedListPnResponse {
                jid,
                dhash,
                children: children_items,
                ..Default::default()
            })
        }
    }

    /// readReceipts:get IQ spec.
    ///
    /// Source: `WAWebQueryPrivacySettingsJob`
    #[derive(Debug, Clone, Default)]
    pub struct ReadReceiptsSpec;

    /// Response from readReceipts:get.
    #[derive(Debug, Clone, Default)]
    pub struct ReadReceiptsResponse {
        pub category: Vec<ReadReceiptsCategoryItem>,
        pub name: String,
        pub value: String,
    }

    impl IqSpec for ReadReceiptsSpec {
        type Response = ReadReceiptsResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let privacy_node = NodeBuilder::new("privacy").build();

            InfoQuery::get(
                PRIVACY_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![privacy_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let name = response
                .get_attr("name")
                .ok_or_else(|| anyhow::anyhow!("missing name"))?
                .as_str()
                .to_string();
            let value = response
                .get_attr("value")
                .ok_or_else(|| anyhow::anyhow!("missing value"))?
                .as_str()
                .to_string();
            let value = response
                .get_attr("value")
                .ok_or_else(|| anyhow::anyhow!("missing value"))?
                .as_str()
                .to_string();
            let value = response
                .get_attr("value")
                .ok_or_else(|| anyhow::anyhow!("missing value"))?
                .as_str()
                .to_string();
            let value = response
                .get_attr("value")
                .ok_or_else(|| anyhow::anyhow!("missing value"))?
                .as_str()
                .to_string();
            let value = response
                .get_attr("value")
                .ok_or_else(|| anyhow::anyhow!("missing value"))?
                .as_str()
                .to_string();
            let value = response
                .get_attr("value")
                .ok_or_else(|| anyhow::anyhow!("missing value"))?
                .as_str()
                .to_string();
            let value = response
                .get_attr("value")
                .ok_or_else(|| anyhow::anyhow!("missing value"))?
                .as_str()
                .to_string();
            let value = response
                .get_attr("value")
                .ok_or_else(|| anyhow::anyhow!("missing value"))?
                .as_str()
                .to_string();
            let value = response
                .get_attr("value")
                .ok_or_else(|| anyhow::anyhow!("missing value"))?
                .as_str()
                .to_string();
            let privacy = response
                .get_optional_child("privacy")
                .ok_or_else(|| anyhow::anyhow!("missing <privacy>"))?;
            let mut category_items = Vec::new();
            for child in privacy.get_children_by_tag("category") {
                let name = child
                    .get_attr("name")
                    .ok_or_else(|| anyhow::anyhow!("missing name"))?
                    .as_str()
                    .to_string();
                let value = child
                    .get_attr("value")
                    .ok_or_else(|| anyhow::anyhow!("missing value"))?
                    .as_str()
                    .to_string();
                category_items.push(ReadReceiptsCategoryItem {
                    name,
                    value,
                    ..Default::default()
                });
            }
            Ok(ReadReceiptsResponse {
                name,
                value,
                category: category_items,
                ..Default::default()
            })
        }
    }

    /// PrivacyUserAction:set IQ spec.
    ///
    /// Source: `WAWebSetPrivacyJob`
    #[derive(Debug, Clone)]
    pub struct PrivacyUserActionSpec {
        pub name: String,
        pub value: String,
    }

    impl PrivacyUserActionSpec {
        pub fn new(name: impl Into<String>, value: impl Into<String>) -> Self {
            Self {
                name: name.into(),
                value: value.into(),
            }
        }
    }

    /// Response from PrivacyUserAction:set.
    #[derive(Debug, Clone, Default)]
    pub struct PrivacyUserActionResponse {
        pub category: Vec<PrivacyUserActionCategoryItem>,
        pub value: String,
        pub code: u64,
        pub text: String,
        pub name: String,
        pub dhash: Option<String>,
    }

    impl IqSpec for PrivacyUserActionSpec {
        type Response = PrivacyUserActionResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let privacy_node = NodeBuilder::new("privacy").build();
            let mut category_node = NodeBuilder::new("category");
            category_node = category_node.attr("name", &*self.name);
            category_node = category_node.attr("value", &*self.value);
            let category_node = category_node.build();

            InfoQuery::set(
                PRIVACY_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![privacy_node, category_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let value = response
                .get_attr("value")
                .ok_or_else(|| anyhow::anyhow!("missing value"))?
                .as_str()
                .to_string();
            let name = response
                .get_attr("name")
                .ok_or_else(|| anyhow::anyhow!("missing name"))?
                .as_str()
                .to_string();
            let value = response
                .get_attr("value")
                .ok_or_else(|| anyhow::anyhow!("missing value"))?
                .as_str()
                .to_string();
            let dhash = response.get_attr("dhash").map(|v| v.as_str().to_string());
            let privacy = response
                .get_optional_child("privacy")
                .ok_or_else(|| anyhow::anyhow!("missing <privacy>"))?;
            let mut category_items = Vec::new();
            for child in privacy.get_children_by_tag("category") {
                let value = child
                    .get_attr("value")
                    .ok_or_else(|| anyhow::anyhow!("missing value"))?
                    .as_str()
                    .to_string();
                let name = child
                    .get_attr("name")
                    .ok_or_else(|| anyhow::anyhow!("missing name"))?
                    .as_str()
                    .to_string();
                let dhash = child.get_attr("dhash").map(|v| v.as_str().to_string());
                category_items.push(PrivacyUserActionCategoryItem {
                    value,
                    name,
                    dhash,
                    ..Default::default()
                });
            }
            let error = response
                .get_optional_child("error")
                .ok_or_else(|| anyhow::anyhow!("missing <error>"))?;
            let code: u64 = error
                .get_attr("code")
                .ok_or_else(|| anyhow::anyhow!("missing code"))?
                .as_str()
                .parse()?;
            let text = error
                .get_attr("text")
                .ok_or_else(|| anyhow::anyhow!("missing text"))?
                .as_str()
                .to_string();
            Ok(PrivacyUserActionResponse {
                value,
                name,
                dhash,
                category: category_items,
                code,
                text,
                ..Default::default()
            })
        }
    }

    /// TokenType:set IQ spec.
    ///
    /// Source: `WAWebSetPrivacyTokensJob`
    #[derive(Debug, Clone)]
    pub struct TokenTypeSpec {
        pub jid: Jid,
        pub t: String,
        pub r#type: String,
    }

    impl TokenTypeSpec {
        pub fn new(jid: &Jid, t: impl Into<String>, r#type: impl Into<String>) -> Self {
            Self {
                jid: jid.clone(),
                t: t.into(),
                r#type: r#type.into(),
            }
        }
    }

    /// Response from TokenType:set.
    #[derive(Debug, Clone, Default)]
    pub struct TokenTypeResponse {
        pub id: String,
    }

    impl IqSpec for TokenTypeSpec {
        type Response = TokenTypeResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut token_node = NodeBuilder::new("token");
            token_node = token_node.attr("jid", self.jid.clone());
            token_node = token_node.attr("t", &*self.t);
            token_node = token_node.attr("type", &*self.r#type);
            let token_node = token_node.build();
            let mut tokens_node = NodeBuilder::new("tokens");
            tokens_node = tokens_node.children([token_node]);
            let tokens_node = tokens_node.build();

            InfoQuery::set(
                PRIVACY_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![tokens_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let id = response
                .get_attr("id")
                .ok_or_else(|| anyhow::anyhow!("missing id"))?
                .as_str()
                .to_string();
            Ok(TokenTypeResponse {
                id,
                ..Default::default()
            })
        }
    }

    /// default:set IQ spec.
    ///
    /// Source: `WAWebSetReadReceiptJob`
    #[derive(Debug, Clone)]
    pub struct SetReadReceiptJobSpec {
        pub value: String,
    }

    impl SetReadReceiptJobSpec {
        pub fn new(value: impl Into<String>) -> Self {
            Self {
                value: value.into(),
            }
        }
    }

    /// Response from default:set.
    #[derive(Debug, Clone, Default)]
    pub struct SetReadReceiptJobResponse {
        pub name: String,
        pub value: String,
    }

    impl IqSpec for SetReadReceiptJobSpec {
        type Response = SetReadReceiptJobResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut category_node = NodeBuilder::new("category");
            category_node = category_node.attr("name", "readreceipts");
            category_node = category_node.attr("value", &*self.value);
            let category_node = category_node.build();
            let mut privacy_node = NodeBuilder::new("privacy");
            privacy_node = privacy_node.children([category_node]);
            let privacy_node = privacy_node.build();

            InfoQuery::set(
                PRIVACY_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![privacy_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let category = response
                .get_optional_child("category")
                .ok_or_else(|| anyhow::anyhow!("missing <category>"))?;
            let name = category
                .get_attr("name")
                .ok_or_else(|| anyhow::anyhow!("missing name"))?
                .as_str()
                .to_string();
            let value = category
                .get_attr("value")
                .ok_or_else(|| anyhow::anyhow!("missing value"))?
                .as_str()
                .to_string();
            Ok(SetReadReceiptJobResponse {
                name,
                value,
                ..Default::default()
            })
        }
    }
}

/// IQ namespace `privatestats`. Source: privateStatsToken.
pub mod privatestats {
    use crate::iq::spec::IqSpec;
    use crate::request::InfoQuery;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::jid::{Jid, Server};
    use wacore_binary::node::NodeContent;

    /// IQ namespace.
    pub const PRIVATESTATS_NAMESPACE: &str = "privatestats";

    // ─── IQ Specs ───────────────────────────────────────────────────────────────

    /// retryStartTime:get IQ spec.
    ///
    /// Source: `privateStatsToken`
    #[derive(Debug, Clone, Default)]
    pub struct RetryStartTimeSpec;

    /// Response from retryStartTime:get.
    #[derive(Debug, Clone, Default)]
    pub struct RetryStartTimeResponse {
        pub signed_credential: Vec<u8>,
        pub acs_public_key: Vec<u8>,
    }

    impl IqSpec for RetryStartTimeSpec {
        type Response = RetryStartTimeResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let blinded_credential_node = NodeBuilder::new("blinded_credential").build();
            let mut sign_credential_node = NodeBuilder::new("sign_credential");
            sign_credential_node = sign_credential_node.attr("version", "1");
            sign_credential_node = sign_credential_node.children([blinded_credential_node]);
            let sign_credential_node = sign_credential_node.build();

            InfoQuery::get(
                PRIVATESTATS_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![sign_credential_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let signed_credential_node = response
                .get_optional_child("signed_credential")
                .ok_or_else(|| anyhow::anyhow!("missing <signed_credential>"))?;
            let signed_credential = signed_credential_node
                .content_bytes()
                .map(|b| b.to_vec())
                .unwrap_or_default();
            let acs_public_key_node = response
                .get_optional_child("acs_public_key")
                .ok_or_else(|| anyhow::anyhow!("missing <acs_public_key>"))?;
            let acs_public_key = acs_public_key_node
                .content_bytes()
                .map(|b| b.to_vec())
                .unwrap_or_default();
            Ok(RetryStartTimeResponse {
                signed_credential,
                acs_public_key,
                ..Default::default()
            })
        }
    }
}

/// IQ namespace `spam`. Source: WASmaxOutSpamGroupReportRequest, WASmaxOutSpamIndividualReportRequest, WASmaxOutSpamNewsletterReportRequest, WASmaxOutSpamStatusReportRequest, WASmaxOutSpamStatusReportV2Request.
pub mod spam {
    use crate::iq::spec::IqSpec;
    use crate::request::InfoQuery;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::jid::{Jid, Server};
    use wacore_binary::node::NodeContent;

    /// IQ namespace.
    pub const SPAM_NAMESPACE: &str = "spam";

    // ─── IQ Specs ───────────────────────────────────────────────────────────────

    /// makeGroupReportRequestSpamListMessage:set IQ spec.
    ///
    /// Source: `WASmaxOutSpamGroupReportRequest`
    #[derive(Debug, Clone)]
    pub struct MakeGroupReportRequestSpamListMessageSpec {
        pub jid: Jid,
        pub source: Option<String>,
    }

    impl MakeGroupReportRequestSpamListMessageSpec {
        pub fn new(jid: &Jid, source: Option<String>) -> Self {
            Self {
                jid: jid.clone(),
                source,
            }
        }
    }

    /// Response from makeGroupReportRequestSpamListMessage:set.
    #[derive(Debug, Clone, Default)]
    pub struct MakeGroupReportRequestSpamListMessageResponse {
        pub from: Jid,
        pub r#type: String,
        pub report_id: Option<String>,
    }

    impl IqSpec for MakeGroupReportRequestSpamListMessageSpec {
        type Response = MakeGroupReportRequestSpamListMessageResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut spam_list_node = NodeBuilder::new("spam_list");
            spam_list_node = spam_list_node.attr("jid", self.jid.clone());
            if let Some(v) = &self.source {
                spam_list_node = spam_list_node.attr("source", v.as_str());
            }
            let spam_list_node = spam_list_node.build();

            InfoQuery::set(
                SPAM_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![spam_list_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let from = response
                .get_attr("from")
                .and_then(|v| v.to_jid())
                .ok_or_else(|| anyhow::anyhow!("missing from"))?;
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            let report_wrap = response
                .get_optional_child("report")
                .ok_or_else(|| anyhow::anyhow!("missing <report>"))?;
            let report_id = report_wrap.get_attr("id").map(|v| v.as_str().to_string());
            Ok(MakeGroupReportRequestSpamListMessageResponse {
                from,
                r#type,
                report_id,
                ..Default::default()
            })
        }
    }

    /// makeIndividualReportRequestSpamListMessage:set IQ spec.
    ///
    /// Source: `WASmaxOutSpamIndividualReportRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeIndividualReportRequestSpamListMessageSpec;

    /// Response from makeIndividualReportRequestSpamListMessage:set.
    #[derive(Debug, Clone, Default)]
    pub struct MakeIndividualReportRequestSpamListMessageResponse {
        pub from: Jid,
        pub r#type: String,
        pub report_id: Option<String>,
    }

    impl IqSpec for MakeIndividualReportRequestSpamListMessageSpec {
        type Response = MakeIndividualReportRequestSpamListMessageResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::set(SPAM_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let from = response
                .get_attr("from")
                .and_then(|v| v.to_jid())
                .ok_or_else(|| anyhow::anyhow!("missing from"))?;
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            let report_wrap = response
                .get_optional_child("report")
                .ok_or_else(|| anyhow::anyhow!("missing <report>"))?;
            let report_id = report_wrap.get_attr("id").map(|v| v.as_str().to_string());
            Ok(MakeIndividualReportRequestSpamListMessageResponse {
                from,
                r#type,
                report_id,
                ..Default::default()
            })
        }
    }

    /// makeNewsletterReportRequestSpamListMessage:set IQ spec.
    ///
    /// Source: `WASmaxOutSpamNewsletterReportRequest`
    #[derive(Debug, Clone)]
    pub struct MakeNewsletterReportRequestSpamListMessageSpec {
        pub jid: Jid,
    }

    impl MakeNewsletterReportRequestSpamListMessageSpec {
        pub fn new(jid: &Jid) -> Self {
            Self { jid: jid.clone() }
        }
    }

    /// Response from makeNewsletterReportRequestSpamListMessage:set.
    #[derive(Debug, Clone, Default)]
    pub struct MakeNewsletterReportRequestSpamListMessageResponse {
        pub from: Jid,
        pub r#type: String,
        pub report_id: Option<String>,
    }

    impl IqSpec for MakeNewsletterReportRequestSpamListMessageSpec {
        type Response = MakeNewsletterReportRequestSpamListMessageResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut spam_list_node = NodeBuilder::new("spam_list");
            spam_list_node = spam_list_node.attr("jid", self.jid.clone());
            let spam_list_node = spam_list_node.build();

            InfoQuery::set(
                SPAM_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![spam_list_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let from = response
                .get_attr("from")
                .and_then(|v| v.to_jid())
                .ok_or_else(|| anyhow::anyhow!("missing from"))?;
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            let report_wrap = response
                .get_optional_child("report")
                .ok_or_else(|| anyhow::anyhow!("missing <report>"))?;
            let report_id = report_wrap.get_attr("id").map(|v| v.as_str().to_string());
            Ok(MakeNewsletterReportRequestSpamListMessageResponse {
                from,
                r#type,
                report_id,
                ..Default::default()
            })
        }
    }

    /// makeStatusReportRequest:set IQ spec.
    ///
    /// Source: `WASmaxOutSpamStatusReportRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeStatusReportRequestSpec;

    /// Response from makeStatusReportRequest:set.
    #[derive(Debug, Clone, Default)]
    pub struct MakeStatusReportRequestResponse {
        pub from: Jid,
        pub r#type: String,
        pub report_id: Option<String>,
    }

    impl IqSpec for MakeStatusReportRequestSpec {
        type Response = MakeStatusReportRequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::set(SPAM_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let from = response
                .get_attr("from")
                .and_then(|v| v.to_jid())
                .ok_or_else(|| anyhow::anyhow!("missing from"))?;
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            let report_wrap = response
                .get_optional_child("report")
                .ok_or_else(|| anyhow::anyhow!("missing <report>"))?;
            let report_id = report_wrap.get_attr("id").map(|v| v.as_str().to_string());
            Ok(MakeStatusReportRequestResponse {
                from,
                r#type,
                report_id,
                ..Default::default()
            })
        }
    }

    /// makeStatusReportV2Request:set IQ spec.
    ///
    /// Source: `WASmaxOutSpamStatusReportV2Request`
    #[derive(Debug, Clone)]
    pub struct MakeStatusReportV2RequestSpec {
        pub jid: Jid,
    }

    impl MakeStatusReportV2RequestSpec {
        pub fn new(jid: &Jid) -> Self {
            Self { jid: jid.clone() }
        }
    }

    /// Response from makeStatusReportV2Request:set.
    #[derive(Debug, Clone, Default)]
    pub struct MakeStatusReportV2RequestResponse {
        pub from: Jid,
        pub r#type: String,
        pub report_id: Option<String>,
    }

    impl IqSpec for MakeStatusReportV2RequestSpec {
        type Response = MakeStatusReportV2RequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let status_node = NodeBuilder::new("status").build();
            let mut spam_list_node = NodeBuilder::new("spam_list");
            spam_list_node = spam_list_node.attr("jid", self.jid.clone());
            spam_list_node = spam_list_node.children([status_node]);
            let spam_list_node = spam_list_node.build();

            InfoQuery::set(
                SPAM_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![spam_list_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let from = response
                .get_attr("from")
                .and_then(|v| v.to_jid())
                .ok_or_else(|| anyhow::anyhow!("missing from"))?;
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            let report_wrap = response
                .get_optional_child("report")
                .ok_or_else(|| anyhow::anyhow!("missing <report>"))?;
            let report_id = report_wrap.get_attr("id").map(|v| v.as_str().to_string());
            Ok(MakeStatusReportV2RequestResponse {
                from,
                r#type,
                report_id,
                ..Default::default()
            })
        }
    }
}

/// IQ namespace `status`. Source: WAWebSetAboutJob.
pub mod status {
    use crate::iq::spec::IqSpec;
    use crate::request::InfoQuery;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::jid::{Jid, Server};
    use wacore_binary::node::NodeContent;

    /// IQ namespace.
    pub const STATUS_NAMESPACE: &str = "status";

    // ─── IQ Specs ───────────────────────────────────────────────────────────────

    /// setAbout:set IQ spec.
    ///
    /// Source: `WAWebSetAboutJob`
    #[derive(Debug, Clone, Default)]
    pub struct SetAboutSpec;

    /// Response from setAbout:set.
    #[derive(Debug, Clone, Default)]
    pub struct SetAboutResponse {
        pub id: u64,
    }

    impl IqSpec for SetAboutSpec {
        type Response = SetAboutResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let status_node = NodeBuilder::new("status").build();

            InfoQuery::set(
                STATUS_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![status_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let id: u64 = response
                .get_attr("id")
                .ok_or_else(|| anyhow::anyhow!("missing id"))?
                .as_str()
                .parse()?;
            Ok(SetAboutResponse {
                id,
                ..Default::default()
            })
        }
    }
}

/// IQ namespace `tos`. Source: WASmaxOutUserNoticeGetDisclosureStageByIdsRequest, WASmaxOutUserNoticeGetDisclosuresRequest, WAWebTosJob, WASmaxOutUserNoticeSetRequest, WASmaxOutUserNoticeSetResultRequest.
pub mod tos {
    use crate::iq::spec::IqSpec;
    use crate::request::InfoQuery;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::jid::{Jid, Server};
    use wacore_binary::node::NodeContent;

    /// IQ namespace.
    pub const TOS_NAMESPACE: &str = "tos";

    // ─── Shared child types ──────────────────────────────────────────────────────

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetDisclosureStageByIdsRequestGetDisclosureStageByIdNoticeItem {
        pub t: u64,
        pub version: Option<u64>,
        pub r#type: Option<u64>,
        pub id: u64,
        pub stage: u64,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetDisclosuresRequestNoticeItem {
        pub t: u64,
        pub version: u64,
        pub r#type: u64,
        pub id: u64,
        pub stage: u64,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct RefreshNoticeItem {
        pub state: Option<String>,
        pub id: String,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct Refresh2NoticeItem {
        pub state: Option<String>,
        pub id: String,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct Refresh3NoticeItem {
        pub state: Option<String>,
        pub id: String,
    }

    // ─── IQ Specs ───────────────────────────────────────────────────────────────

    /// makeGetDisclosureStageByIdsRequestGetDisclosureStageById:get IQ spec.
    ///
    /// Source: `WASmaxOutUserNoticeGetDisclosureStageByIdsRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetDisclosureStageByIdsRequestGetDisclosureStageByIdSpec;

    impl IqSpec for MakeGetDisclosureStageByIdsRequestGetDisclosureStageByIdSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::get(TOS_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// makeGetDisclosuresRequest:get IQ spec.
    ///
    /// Source: `WASmaxOutUserNoticeGetDisclosuresRequest`
    #[derive(Debug, Clone)]
    pub struct MakeGetDisclosuresRequestSpec {
        pub t: u64,
    }

    impl MakeGetDisclosuresRequestSpec {
        pub fn new(t: u64) -> Self {
            Self { t }
        }
    }

    impl IqSpec for MakeGetDisclosuresRequestSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut get_user_disclosures_node = NodeBuilder::new("get_user_disclosures");
            get_user_disclosures_node = get_user_disclosures_node.attr("t", self.t.to_string());
            let get_user_disclosures_node = get_user_disclosures_node.build();

            InfoQuery::get(
                TOS_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![get_user_disclosures_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// refresh:get IQ spec.
    ///
    /// Source: `WAWebTosJob`
    #[derive(Debug, Clone)]
    pub struct RefreshSpec {
        pub id: String,
    }

    impl RefreshSpec {
        pub fn new(id: impl Into<String>) -> Self {
            Self { id: id.into() }
        }
    }

    /// Response from refresh:get.
    #[derive(Debug, Clone, Default)]
    pub struct RefreshResponse {
        pub refresh: u64,
        pub notice: Vec<RefreshNoticeItem>,
    }

    impl IqSpec for RefreshSpec {
        type Response = RefreshResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut notice_node = NodeBuilder::new("notice");
            notice_node = notice_node.attr("id", &*self.id);
            let notice_node = notice_node.build();
            let mut request_node = NodeBuilder::new("request");
            request_node = request_node.children([notice_node]);
            let request_node = request_node.build();

            InfoQuery::get(
                TOS_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![request_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let tos = response
                .get_optional_child("tos")
                .ok_or_else(|| anyhow::anyhow!("missing <tos>"))?;
            let refresh: u64 = tos
                .get_attr("refresh")
                .ok_or_else(|| anyhow::anyhow!("missing refresh"))?
                .as_str()
                .parse()?;
            let mut notice_items = Vec::new();
            for child in tos.get_children_by_tag("notice") {
                let state = child.get_attr("state").map(|v| v.as_str().to_string());
                let id = child
                    .get_attr("id")
                    .ok_or_else(|| anyhow::anyhow!("missing id"))?
                    .as_str()
                    .to_string();
                notice_items.push(RefreshNoticeItem {
                    state,
                    id,
                    ..Default::default()
                });
            }
            Ok(RefreshResponse {
                refresh,
                notice: notice_items,
                ..Default::default()
            })
        }
    }

    /// makeSetRequest:set IQ spec.
    ///
    /// Source: `WASmaxOutUserNoticeSetRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeSetRequestSpec;

    /// Response from makeSetRequest:set.
    #[derive(Debug, Clone, Default)]
    pub struct MakeSetRequestResponse {
        pub r#type: String,
        pub t: u64,
        pub id: u64,
        pub stage: u64,
    }

    impl IqSpec for MakeSetRequestSpec {
        type Response = MakeSetRequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let notice_node = NodeBuilder::new("notice").build();

            InfoQuery::set(
                TOS_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![notice_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            let notice = response
                .get_optional_child("notice")
                .ok_or_else(|| anyhow::anyhow!("missing <notice>"))?;
            let t: u64 = notice
                .get_attr("t")
                .ok_or_else(|| anyhow::anyhow!("missing t"))?
                .as_str()
                .parse()?;
            let id: u64 = notice
                .get_attr("id")
                .ok_or_else(|| anyhow::anyhow!("missing id"))?
                .as_str()
                .parse()?;
            let stage: u64 = notice
                .get_attr("stage")
                .ok_or_else(|| anyhow::anyhow!("missing stage"))?
                .as_str()
                .parse()?;
            Ok(MakeSetRequestResponse {
                r#type,
                t,
                id,
                stage,
                ..Default::default()
            })
        }
    }

    /// makeSetResultRequest:set IQ spec.
    ///
    /// Source: `WASmaxOutUserNoticeSetResultRequest`
    #[derive(Debug, Clone)]
    pub struct MakeSetResultRequestSpec {
        pub id: u64,
        pub result: u64,
    }

    impl MakeSetResultRequestSpec {
        pub fn new(id: u64, result: u64) -> Self {
            Self { id, result }
        }
    }

    /// Response from makeSetResultRequest:set.
    #[derive(Debug, Clone, Default)]
    pub struct MakeSetResultRequestResponse {
        pub r#type: String,
        pub id: u64,
        pub result: u64,
    }

    impl IqSpec for MakeSetResultRequestSpec {
        type Response = MakeSetResultRequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut trackable_node = NodeBuilder::new("trackable");
            trackable_node = trackable_node.attr("id", self.id.to_string());
            trackable_node = trackable_node.attr("result", self.result.to_string());
            let trackable_node = trackable_node.build();

            InfoQuery::set(
                TOS_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![trackable_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            let trackable = response
                .get_optional_child("trackable")
                .ok_or_else(|| anyhow::anyhow!("missing <trackable>"))?;
            let id: u64 = trackable
                .get_attr("id")
                .ok_or_else(|| anyhow::anyhow!("missing id"))?
                .as_str()
                .parse()?;
            let result: u64 = trackable
                .get_attr("result")
                .ok_or_else(|| anyhow::anyhow!("missing result"))?
                .as_str()
                .parse()?;
            Ok(MakeSetResultRequestResponse {
                r#type,
                id,
                result,
                ..Default::default()
            })
        }
    }

    /// refresh:set IQ spec.
    ///
    /// Source: `WAWebTosJob`
    #[derive(Debug, Clone)]
    pub struct Refresh2Spec {
        pub id: String,
    }

    impl Refresh2Spec {
        pub fn new(id: impl Into<String>) -> Self {
            Self { id: id.into() }
        }
    }

    /// Response from refresh:set.
    #[derive(Debug, Clone, Default)]
    pub struct Refresh2Response {
        pub refresh: u64,
        pub notice: Vec<Refresh2NoticeItem>,
    }

    impl IqSpec for Refresh2Spec {
        type Response = Refresh2Response;

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut delete_node = NodeBuilder::new("delete");
            delete_node = delete_node.attr("id", &*self.id);
            let delete_node = delete_node.build();

            InfoQuery::set(
                TOS_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![delete_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let tos = response
                .get_optional_child("tos")
                .ok_or_else(|| anyhow::anyhow!("missing <tos>"))?;
            let refresh: u64 = tos
                .get_attr("refresh")
                .ok_or_else(|| anyhow::anyhow!("missing refresh"))?
                .as_str()
                .parse()?;
            let mut notice_items = Vec::new();
            for child in tos.get_children_by_tag("notice") {
                let state = child.get_attr("state").map(|v| v.as_str().to_string());
                let id = child
                    .get_attr("id")
                    .ok_or_else(|| anyhow::anyhow!("missing id"))?
                    .as_str()
                    .to_string();
                notice_items.push(Refresh2NoticeItem {
                    state,
                    id,
                    ..Default::default()
                });
            }
            Ok(Refresh2Response {
                refresh,
                notice: notice_items,
                ..Default::default()
            })
        }
    }

    /// refresh:set IQ spec.
    ///
    /// Source: `WAWebTosJob`
    #[derive(Debug, Clone)]
    pub struct Refresh3Spec {
        pub id: String,
    }

    impl Refresh3Spec {
        pub fn new(id: impl Into<String>) -> Self {
            Self { id: id.into() }
        }
    }

    /// Response from refresh:set.
    #[derive(Debug, Clone, Default)]
    pub struct Refresh3Response {
        pub refresh: u64,
        pub notice: Vec<Refresh3NoticeItem>,
    }

    impl IqSpec for Refresh3Spec {
        type Response = Refresh3Response;

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut notice_node = NodeBuilder::new("notice");
            notice_node = notice_node.attr("id", &*self.id);
            let notice_node = notice_node.build();
            let mut request_node = NodeBuilder::new("request");
            request_node = request_node.attr("type", "session_update");
            request_node = request_node.children([notice_node]);
            let request_node = request_node.build();

            InfoQuery::set(
                TOS_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![request_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let tos = response
                .get_optional_child("tos")
                .ok_or_else(|| anyhow::anyhow!("missing <tos>"))?;
            let refresh: u64 = tos
                .get_attr("refresh")
                .ok_or_else(|| anyhow::anyhow!("missing refresh"))?
                .as_str()
                .parse()?;
            let mut notice_items = Vec::new();
            for child in tos.get_children_by_tag("notice") {
                let state = child.get_attr("state").map(|v| v.as_str().to_string());
                let id = child
                    .get_attr("id")
                    .ok_or_else(|| anyhow::anyhow!("missing id"))?
                    .as_str()
                    .to_string();
                notice_items.push(Refresh3NoticeItem {
                    state,
                    id,
                    ..Default::default()
                });
            }
            Ok(Refresh3Response {
                refresh,
                notice: notice_items,
                ..Default::default()
            })
        }
    }
}

/// IQ namespace `urn:xmpp:whatsapp:account`. Source: WAWebGdprHookUtils, WASmaxOutAccountSetPaymentsTOSv3Request, WASmaxOutAccountSetIQMixin.
pub mod urn_xmpp_whatsapp_account {
    use crate::iq::spec::IqSpec;
    use crate::request::InfoQuery;
    use wacore_binary::jid::{Jid, Server};

    /// IQ namespace.
    pub const URN_XMPP_WHATSAPP_ACCOUNT_NAMESPACE: &str = "urn:xmpp:whatsapp:account";

    // ─── IQ Specs ───────────────────────────────────────────────────────────────

    /// getGdprIq:get IQ spec.
    ///
    /// Source: `WAWebGdprHookUtils`
    #[derive(Debug, Clone, Default)]
    pub struct GetGdprIqSpec;

    impl IqSpec for GetGdprIqSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::get(
                URN_XMPP_WHATSAPP_ACCOUNT_NAMESPACE,
                Jid::new("", Server::Pn),
                None,
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// makeSetPaymentsTOSv3Request:set IQ spec.
    ///
    /// Source: `WASmaxOutAccountSetPaymentsTOSv3Request`
    #[derive(Debug, Clone, Default)]
    pub struct MakeSetPaymentsTOSv3RequestSpec;

    /// Response from makeSetPaymentsTOSv3Request:set.
    #[derive(Debug, Clone, Default)]
    pub struct MakeSetPaymentsTOSv3RequestResponse {
        pub accept_pay_outage: Option<String>,
        pub accept_pay_sandbox: Option<String>,
        pub r#type: String,
    }

    impl IqSpec for MakeSetPaymentsTOSv3RequestSpec {
        type Response = MakeSetPaymentsTOSv3RequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::set(
                URN_XMPP_WHATSAPP_ACCOUNT_NAMESPACE,
                Jid::new("", Server::Pn),
                None,
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let accept_pay_wrap = response
                .get_optional_child("accept_pay")
                .ok_or_else(|| anyhow::anyhow!("missing <accept_pay>"))?;
            let accept_pay_outage = accept_pay_wrap
                .get_attr("outage")
                .map(|v| v.as_str().to_string());
            let accept_pay_sandbox = accept_pay_wrap
                .get_attr("sandbox")
                .map(|v| v.as_str().to_string());
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            Ok(MakeSetPaymentsTOSv3RequestResponse {
                accept_pay_outage,
                accept_pay_sandbox,
                r#type,
                ..Default::default()
            })
        }
    }

    /// mergeSetIQMixin:set IQ spec.
    ///
    /// Source: `WASmaxOutAccountSetIQMixin`
    #[derive(Debug, Clone, Default)]
    pub struct MergeSetIQMixinSpec;

    impl IqSpec for MergeSetIQMixinSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::set(
                URN_XMPP_WHATSAPP_ACCOUNT_NAMESPACE,
                Jid::new("", Server::Pn),
                None,
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }
}

/// IQ namespace `urn:xmpp:whatsapp:dirty`. Source: WAWebClearDirtyBitsJob.
pub mod urn_xmpp_whatsapp_dirty {
    use crate::iq::spec::IqSpec;
    use crate::request::InfoQuery;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::jid::{Jid, Server};
    use wacore_binary::node::NodeContent;

    /// IQ namespace.
    pub const URN_XMPP_WHATSAPP_DIRTY_NAMESPACE: &str = "urn:xmpp:whatsapp:dirty";

    // ─── IQ Specs ───────────────────────────────────────────────────────────────

    /// clearDirtyBits:set IQ spec.
    ///
    /// Source: `WAWebClearDirtyBitsJob`
    #[derive(Debug, Clone)]
    pub struct ClearDirtyBitsSpec {
        pub r#type: String,
        pub timestamp: u64,
    }

    impl ClearDirtyBitsSpec {
        pub fn new(r#type: impl Into<String>, timestamp: u64) -> Self {
            Self {
                r#type: r#type.into(),
                timestamp,
            }
        }
    }

    impl IqSpec for ClearDirtyBitsSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut clean_node = NodeBuilder::new("clean");
            clean_node = clean_node.attr("type", &*self.r#type);
            clean_node = clean_node.attr("timestamp", self.timestamp.to_string());
            let clean_node = clean_node.build();

            InfoQuery::set(
                URN_XMPP_WHATSAPP_DIRTY_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![clean_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }
}

/// IQ namespace `urn:xmpp:whatsapp:push`. Source: WAWebGetPushServerSettingsJob, WASmaxOutPushConfigSetRequest, WAWebSetWindowsPushConfig.
pub mod urn_xmpp_whatsapp_push {
    use crate::iq::spec::IqSpec;
    use crate::request::InfoQuery;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::jid::{Jid, Server};
    use wacore_binary::node::NodeContent;

    /// IQ namespace.
    pub const URN_XMPP_WHATSAPP_PUSH_NAMESPACE: &str = "urn:xmpp:whatsapp:push";

    // ─── IQ Specs ───────────────────────────────────────────────────────────────

    /// getPushServerSettings:get IQ spec.
    ///
    /// Source: `WAWebGetPushServerSettingsJob`
    #[derive(Debug, Clone, Default)]
    pub struct GetPushServerSettingsSpec;

    /// Response from getPushServerSettings:get.
    #[derive(Debug, Clone, Default)]
    pub struct GetPushServerSettingsResponse {
        pub webserverkey: String,
        pub code: u64,
        pub text: String,
    }

    impl IqSpec for GetPushServerSettingsSpec {
        type Response = GetPushServerSettingsResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let settings_node = NodeBuilder::new("settings").build();

            InfoQuery::get(
                URN_XMPP_WHATSAPP_PUSH_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![settings_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let settings = response
                .get_optional_child("settings")
                .ok_or_else(|| anyhow::anyhow!("missing <settings>"))?;
            let webserverkey = settings
                .get_attr("webserverkey")
                .ok_or_else(|| anyhow::anyhow!("missing webserverkey"))?
                .as_str()
                .to_string();
            let error = response
                .get_optional_child("error")
                .ok_or_else(|| anyhow::anyhow!("missing <error>"))?;
            let code: u64 = error
                .get_attr("code")
                .ok_or_else(|| anyhow::anyhow!("missing code"))?
                .as_str()
                .parse()?;
            let text = error
                .get_attr("text")
                .ok_or_else(|| anyhow::anyhow!("missing text"))?
                .as_str()
                .to_string();
            Ok(GetPushServerSettingsResponse {
                webserverkey,
                code,
                text,
                ..Default::default()
            })
        }
    }

    /// makeSetRequest:set IQ spec.
    ///
    /// Source: `WASmaxOutPushConfigSetRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeSetRequestSpec;

    /// Response from makeSetRequest:set.
    #[derive(Debug, Clone, Default)]
    pub struct MakeSetRequestResponse {
        pub r#type: String,
    }

    impl IqSpec for MakeSetRequestSpec {
        type Response = MakeSetRequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::set(
                URN_XMPP_WHATSAPP_PUSH_NAMESPACE,
                Jid::new("", Server::Pn),
                None,
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            Ok(MakeSetRequestResponse {
                r#type,
                ..Default::default()
            })
        }
    }

    /// setWindowsPushConfig:set IQ spec.
    ///
    /// Source: `WAWebSetWindowsPushConfig`
    #[derive(Debug, Clone)]
    pub struct SetWindowsPushConfigSpec {
        pub id: String,
        pub version: String,
    }

    impl SetWindowsPushConfigSpec {
        pub fn new(id: impl Into<String>, version: impl Into<String>) -> Self {
            Self {
                id: id.into(),
                version: version.into(),
            }
        }
    }

    /// Response from setWindowsPushConfig:set.
    #[derive(Debug, Clone, Default)]
    pub struct SetWindowsPushConfigResponse {
        pub code: u64,
        pub text: String,
    }

    impl IqSpec for SetWindowsPushConfigSpec {
        type Response = SetWindowsPushConfigResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut config_node = NodeBuilder::new("config");
            config_node = config_node.attr("id", &*self.id);
            config_node = config_node.attr("platform", "wns");
            config_node = config_node.attr("version", &*self.version);
            let config_node = config_node.build();

            InfoQuery::set(
                URN_XMPP_WHATSAPP_PUSH_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![config_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let error = response
                .get_optional_child("error")
                .ok_or_else(|| anyhow::anyhow!("missing <error>"))?;
            let code: u64 = error
                .get_attr("code")
                .ok_or_else(|| anyhow::anyhow!("missing code"))?
                .as_str()
                .parse()?;
            let text = error
                .get_attr("text")
                .ok_or_else(|| anyhow::anyhow!("missing text"))?
                .as_str()
                .to_string();
            Ok(SetWindowsPushConfigResponse {
                code,
                text,
                ..Default::default()
            })
        }
    }
}

/// IQ namespace `usync`. Source: WAWebUsync.
pub mod usync {
    use crate::iq::spec::IqSpec;
    use crate::request::InfoQuery;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::jid::{Jid, Server};
    use wacore_binary::node::NodeContent;

    /// IQ namespace.
    pub const USYNC_NAMESPACE: &str = "usync";

    // ─── IQ Specs ───────────────────────────────────────────────────────────────

    /// $3:get IQ spec.
    ///
    /// Source: `WAWebUsync`
    #[derive(Debug, Clone)]
    pub struct UsyncSpec {
        pub mode: String,
        pub context: String,
        pub jid: Option<String>,
        pub pn_jid: Option<String>,
    }

    impl UsyncSpec {
        pub fn new(
            mode: impl Into<String>,
            context: impl Into<String>,
            jid: Option<String>,
            pn_jid: Option<String>,
        ) -> Self {
            Self {
                mode: mode.into(),
                context: context.into(),
                jid,
                pn_jid,
            }
        }
    }

    /// Response from $3:get.
    #[derive(Debug, Clone, Default)]
    pub struct UsyncResponse {
        pub refresh: u64,
        pub code: u64,
        pub text: String,
        pub backoff: u64,
    }

    impl IqSpec for UsyncSpec {
        type Response = UsyncResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let query_node = NodeBuilder::new("query").build();
            let mut user_node = NodeBuilder::new("user");
            if let Some(v) = &self.jid {
                user_node = user_node.attr("jid", v.as_str());
            }
            if let Some(v) = &self.pn_jid {
                user_node = user_node.attr("pn_jid", v.as_str());
            }
            let user_node = user_node.build();
            let mut list_node = NodeBuilder::new("list");
            list_node = list_node.children([user_node]);
            let list_node = list_node.build();
            let mut usync_node = NodeBuilder::new("usync");
            usync_node = usync_node.attr("index", "0");
            usync_node = usync_node.attr("last", "true");
            usync_node = usync_node.attr("mode", &*self.mode);
            usync_node = usync_node.attr("context", &*self.context);
            usync_node = usync_node.children([query_node, list_node]);
            let usync_node = usync_node.build();

            InfoQuery::get(
                USYNC_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![usync_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let usync = response
                .get_optional_child("usync")
                .ok_or_else(|| anyhow::anyhow!("missing <usync>"))?;
            let refresh: u64 = usync
                .get_attr("refresh")
                .ok_or_else(|| anyhow::anyhow!("missing refresh"))?
                .as_str()
                .parse()?;
            let error = response
                .get_optional_child("error")
                .ok_or_else(|| anyhow::anyhow!("missing <error>"))?;
            let code: u64 = error
                .get_attr("code")
                .ok_or_else(|| anyhow::anyhow!("missing code"))?
                .as_str()
                .parse()?;
            let text = error
                .get_attr("text")
                .ok_or_else(|| anyhow::anyhow!("missing text"))?
                .as_str()
                .to_string();
            let backoff: u64 = error
                .get_attr("backoff")
                .ok_or_else(|| anyhow::anyhow!("missing backoff"))?
                .as_str()
                .parse()?;
            Ok(UsyncResponse {
                refresh,
                code,
                text,
                backoff,
                ..Default::default()
            })
        }
    }
}

/// IQ namespace `w:biz`. Source: WAWebQueryBusinessProfileJob, WAWebHandleBusinessNameChange, WASmaxOutBizMarketingMessageGetBusinessEligibilityRequest, WASmaxOutBizSettingsGetPrivacySettingRequest, WASmaxOutSmbMeteredMessagingAccountGetSMBMeteredMessagingCheckoutRequest, WASmaxOutBizSettingsSetPrivacySettingRequest, WAWebBusinessProfileJob.
pub mod w_biz {
    use crate::iq::spec::IqSpec;
    use crate::request::InfoQuery;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::jid::{Jid, Server};
    use wacore_binary::node::NodeContent;

    /// IQ namespace.
    pub const W_BIZ_NAMESPACE: &str = "w:biz";

    // ─── Shared child types ──────────────────────────────────────────────────────

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct QueryBusinessProfileJobProfileItem {
        pub jid: Jid,
        pub tag: String,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetSMBMeteredMessagingCheckoutRequestParticipantsToDiscountItem {
        pub r#type: String,
        pub percentage: Option<u64>,
        pub amount: u64,
        pub amount_formatted: String,
    }

    // ─── IQ Specs ───────────────────────────────────────────────────────────────

    /// default:get IQ spec.
    ///
    /// Source: `WAWebQueryBusinessProfileJob`
    #[derive(Debug, Clone)]
    pub struct QueryBusinessProfileJobSpec {
        pub v: u64,
        pub jid: Jid,
        pub tag: Option<String>,
    }

    impl QueryBusinessProfileJobSpec {
        pub fn new(v: u64, jid: &Jid, tag: Option<String>) -> Self {
            Self {
                v,
                jid: jid.clone(),
                tag,
            }
        }
    }

    /// Response from default:get.
    #[derive(Debug, Clone, Default)]
    pub struct QueryBusinessProfileJobResponse {
        pub profile: Vec<QueryBusinessProfileJobProfileItem>,
        pub jid: Jid,
        pub tag: String,
    }

    impl IqSpec for QueryBusinessProfileJobSpec {
        type Response = QueryBusinessProfileJobResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut profile_node = NodeBuilder::new("profile");
            profile_node = profile_node.attr("jid", self.jid.clone());
            if let Some(v) = &self.tag {
                profile_node = profile_node.attr("tag", v.as_str());
            }
            let profile_node = profile_node.build();
            let mut business_profile_node = NodeBuilder::new("business_profile");
            business_profile_node = business_profile_node.attr("v", self.v.to_string());
            business_profile_node = business_profile_node.children([profile_node]);
            let business_profile_node = business_profile_node.build();

            InfoQuery::get(
                W_BIZ_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![business_profile_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let jid = response
                .get_attr("jid")
                .and_then(|v| v.to_jid())
                .ok_or_else(|| anyhow::anyhow!("missing jid"))?;
            let tag = response
                .get_attr("tag")
                .ok_or_else(|| anyhow::anyhow!("missing tag"))?
                .as_str()
                .to_string();
            let business_profile = response
                .get_optional_child("business_profile")
                .ok_or_else(|| anyhow::anyhow!("missing <business_profile>"))?;
            let mut profile_items = Vec::new();
            for child in business_profile.get_children_by_tag("profile") {
                let jid = child
                    .get_attr("jid")
                    .and_then(|v| v.to_jid())
                    .ok_or_else(|| anyhow::anyhow!("missing jid"))?;
                let tag = child
                    .get_attr("tag")
                    .ok_or_else(|| anyhow::anyhow!("missing tag"))?
                    .as_str()
                    .to_string();
                profile_items.push(QueryBusinessProfileJobProfileItem {
                    jid,
                    tag,
                    ..Default::default()
                });
            }
            Ok(QueryBusinessProfileJobResponse {
                jid,
                tag,
                profile: profile_items,
                ..Default::default()
            })
        }
    }

    /// handleVerifiedBusinessNameNotificationHash:get IQ spec.
    ///
    /// Source: `WAWebHandleBusinessNameChange`
    #[derive(Debug, Clone)]
    pub struct HandleVerifiedBusinessNameNotificationHashSpec {
        pub jid: Jid,
    }

    impl HandleVerifiedBusinessNameNotificationHashSpec {
        pub fn new(jid: &Jid) -> Self {
            Self { jid: jid.clone() }
        }
    }

    impl IqSpec for HandleVerifiedBusinessNameNotificationHashSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut verified_name_node = NodeBuilder::new("verified_name");
            verified_name_node = verified_name_node.attr("jid", self.jid.clone());
            let verified_name_node = verified_name_node.build();

            InfoQuery::get(
                W_BIZ_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![verified_name_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// makeGetBusinessEligibilityRequest:get IQ spec.
    ///
    /// Source: `WASmaxOutBizMarketingMessageGetBusinessEligibilityRequest`
    #[derive(Debug, Clone)]
    pub struct MakeGetBusinessEligibilityRequestSpec {
        pub meta_verified: Option<String>,
        pub marketing_messages: Option<String>,
        pub genai: Option<String>,
    }

    impl MakeGetBusinessEligibilityRequestSpec {
        pub fn new(
            meta_verified: Option<String>,
            marketing_messages: Option<String>,
            genai: Option<String>,
        ) -> Self {
            Self {
                meta_verified,
                marketing_messages,
                genai,
            }
        }
    }

    /// Response from makeGetBusinessEligibilityRequest:get.
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetBusinessEligibilityRequestResponse {
        pub to: Jid,
        pub r#type: String,
        pub status: String,
        pub should_show_privacy_interstitial_to_new_users: Option<String>,
        pub additional_params: Option<String>,
        pub expiration: Option<u64>,
    }

    impl IqSpec for MakeGetBusinessEligibilityRequestSpec {
        type Response = MakeGetBusinessEligibilityRequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut features_node = NodeBuilder::new("features");
            if let Some(v) = &self.meta_verified {
                features_node = features_node.attr("meta_verified", v.as_str());
            }
            if let Some(v) = &self.marketing_messages {
                features_node = features_node.attr("marketing_messages", v.as_str());
            }
            if let Some(v) = &self.genai {
                features_node = features_node.attr("genai", v.as_str());
            }
            let features_node = features_node.build();

            InfoQuery::get(
                W_BIZ_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![features_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let to = response
                .get_attr("to")
                .and_then(|v| v.to_jid())
                .ok_or_else(|| anyhow::anyhow!("missing to"))?;
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            let meta_verified = response
                .get_optional_child("meta_verified")
                .ok_or_else(|| anyhow::anyhow!("missing <meta_verified>"))?;
            let status = meta_verified
                .get_attr("status")
                .ok_or_else(|| anyhow::anyhow!("missing status"))?
                .as_str()
                .to_string();
            let should_show_privacy_interstitial_to_new_users = meta_verified
                .get_attr("should_show_privacy_interstitial_to_new_users")
                .map(|v| v.as_str().to_string());
            let additional_params = meta_verified
                .get_attr("additional_params")
                .map(|v| v.as_str().to_string());
            let marketing_messages = response
                .get_optional_child("marketing_messages")
                .ok_or_else(|| anyhow::anyhow!("missing <marketing_messages>"))?;
            let status = marketing_messages
                .get_attr("status")
                .ok_or_else(|| anyhow::anyhow!("missing status"))?
                .as_str()
                .to_string();
            let expiration = marketing_messages
                .get_attr("expiration")
                .and_then(|v| v.as_str().parse().ok());
            let genai = response
                .get_optional_child("genai")
                .ok_or_else(|| anyhow::anyhow!("missing <genai>"))?;
            let status = genai
                .get_attr("status")
                .ok_or_else(|| anyhow::anyhow!("missing status"))?
                .as_str()
                .to_string();
            Ok(MakeGetBusinessEligibilityRequestResponse {
                to,
                r#type,
                status,
                should_show_privacy_interstitial_to_new_users,
                additional_params,
                expiration,
                ..Default::default()
            })
        }
    }

    /// makeGetPrivacySettingRequest:get IQ spec.
    ///
    /// Source: `WASmaxOutBizSettingsGetPrivacySettingRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetPrivacySettingRequestSpec;

    /// Response from makeGetPrivacySettingRequest:get.
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetPrivacySettingRequestResponse {
        pub value: String,
        pub r#type: String,
    }

    impl IqSpec for MakeGetPrivacySettingRequestSpec {
        type Response = MakeGetPrivacySettingRequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let privacy_node = NodeBuilder::new("privacy").build();

            InfoQuery::get(
                W_BIZ_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![privacy_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            let privacy = response
                .get_optional_child("privacy")
                .ok_or_else(|| anyhow::anyhow!("missing <privacy>"))?;
            let value = privacy
                .get_attr("value")
                .ok_or_else(|| anyhow::anyhow!("missing value"))?
                .as_str()
                .to_string();
            Ok(MakeGetPrivacySettingRequestResponse {
                r#type,
                value,
                ..Default::default()
            })
        }
    }

    /// makeGetSMBMeteredMessagingCheckoutRequestParticipantsTo:get IQ spec.
    ///
    /// Source: `WASmaxOutSmbMeteredMessagingAccountGetSMBMeteredMessagingCheckoutRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetSMBMeteredMessagingCheckoutRequestParticipantsToSpec;

    impl IqSpec for MakeGetSMBMeteredMessagingCheckoutRequestParticipantsToSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            let participants_node = NodeBuilder::new("participants").build();

            InfoQuery::get(
                W_BIZ_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![participants_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// makeSetPrivacySettingRequest:set IQ spec.
    ///
    /// Source: `WASmaxOutBizSettingsSetPrivacySettingRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeSetPrivacySettingRequestSpec;

    /// Response from makeSetPrivacySettingRequest:set.
    #[derive(Debug, Clone, Default)]
    pub struct MakeSetPrivacySettingRequestResponse {
        pub value: String,
        pub r#type: String,
    }

    impl IqSpec for MakeSetPrivacySettingRequestSpec {
        type Response = MakeSetPrivacySettingRequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::set(W_BIZ_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            let privacy = response
                .get_optional_child("privacy")
                .ok_or_else(|| anyhow::anyhow!("missing <privacy>"))?;
            let value = privacy
                .get_attr("value")
                .ok_or_else(|| anyhow::anyhow!("missing value"))?
                .as_str()
                .to_string();
            Ok(MakeSetPrivacySettingRequestResponse {
                r#type,
                value,
                ..Default::default()
            })
        }
    }

    /// updateCartEnabled:set IQ spec.
    ///
    /// Source: `WAWebBusinessProfileJob`
    #[derive(Debug, Clone, Default)]
    pub struct UpdateCartEnabledSpec;

    impl IqSpec for UpdateCartEnabledSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut business_profile_node = NodeBuilder::new("business_profile");
            business_profile_node = business_profile_node.attr("v", "3");
            business_profile_node = business_profile_node.attr("mutation_type", "delta");
            let business_profile_node = business_profile_node.build();

            InfoQuery::set(
                W_BIZ_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![business_profile_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// updateCartEnabled:set IQ spec.
    ///
    /// Source: `WAWebBusinessProfileJob`
    #[derive(Debug, Clone)]
    pub struct UpdateCartEnabled2Spec {
        pub id: String,
        pub ts: String,
        pub token: String,
    }

    impl UpdateCartEnabled2Spec {
        pub fn new(id: impl Into<String>, ts: impl Into<String>, token: impl Into<String>) -> Self {
            Self {
                id: id.into(),
                ts: ts.into(),
                token: token.into(),
            }
        }
    }

    impl IqSpec for UpdateCartEnabled2Spec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut cover_photo_node = NodeBuilder::new("cover_photo");
            cover_photo_node = cover_photo_node.attr("op", "update");
            cover_photo_node = cover_photo_node.attr("id", &*self.id);
            cover_photo_node = cover_photo_node.attr("ts", &*self.ts);
            cover_photo_node = cover_photo_node.attr("token", &*self.token);
            let cover_photo_node = cover_photo_node.build();
            let mut business_profile_node = NodeBuilder::new("business_profile");
            business_profile_node = business_profile_node.attr("v", "3");
            business_profile_node = business_profile_node.attr("mutation_type", "delta");
            business_profile_node = business_profile_node.children([cover_photo_node]);
            let business_profile_node = business_profile_node.build();

            InfoQuery::set(
                W_BIZ_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![business_profile_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// updateCartEnabled:set IQ spec.
    ///
    /// Source: `WAWebBusinessProfileJob`
    #[derive(Debug, Clone)]
    pub struct UpdateCartEnabled3Spec {
        pub id: String,
    }

    impl UpdateCartEnabled3Spec {
        pub fn new(id: impl Into<String>) -> Self {
            Self { id: id.into() }
        }
    }

    impl IqSpec for UpdateCartEnabled3Spec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut cover_photo_node = NodeBuilder::new("cover_photo");
            cover_photo_node = cover_photo_node.attr("op", "delete");
            cover_photo_node = cover_photo_node.attr("id", &*self.id);
            let cover_photo_node = cover_photo_node.build();
            let mut business_profile_node = NodeBuilder::new("business_profile");
            business_profile_node = business_profile_node.attr("v", "3");
            business_profile_node = business_profile_node.attr("mutation_type", "delta");
            business_profile_node = business_profile_node.children([cover_photo_node]);
            let business_profile_node = business_profile_node.build();

            InfoQuery::set(
                W_BIZ_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![business_profile_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }
}

/// IQ namespace `w:biz:catalog`. Source: WAWebQueryGetSignedUserInfoJob, WAWebQueryProductListCatalogJob, WAWebVerifyPostcodeJob.
pub mod w_biz_catalog {
    use crate::iq::spec::IqSpec;
    use crate::request::InfoQuery;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::jid::{Jid, Server};
    use wacore_binary::node::NodeContent;

    /// IQ namespace.
    pub const W_BIZ_CATALOG_NAMESPACE: &str = "w:biz:catalog";

    // ─── IQ Specs ───────────────────────────────────────────────────────────────

    /// QueryGetSignedUserInfo:get IQ spec.
    ///
    /// Source: `WAWebQueryGetSignedUserInfoJob`
    #[derive(Debug, Clone)]
    pub struct QueryGetSignedUserInfoSpec {
        pub biz_jid: Jid,
    }

    impl QueryGetSignedUserInfoSpec {
        pub fn new(biz_jid: &Jid) -> Self {
            Self {
                biz_jid: biz_jid.clone(),
            }
        }
    }

    impl IqSpec for QueryGetSignedUserInfoSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut signed_user_info_node = NodeBuilder::new("signed_user_info");
            signed_user_info_node = signed_user_info_node.attr("biz_jid", self.biz_jid.clone());
            let signed_user_info_node = signed_user_info_node.build();

            InfoQuery::get(
                W_BIZ_CATALOG_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![signed_user_info_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// QueryProductListCatalog:get IQ spec.
    ///
    /// Source: `WAWebQueryProductListCatalogJob`
    #[derive(Debug, Clone)]
    pub struct QueryProductListCatalogSpec {
        pub jid: Jid,
    }

    impl QueryProductListCatalogSpec {
        pub fn new(jid: &Jid) -> Self {
            Self { jid: jid.clone() }
        }
    }

    /// Response from QueryProductListCatalog:get.
    #[derive(Debug, Clone, Default)]
    pub struct QueryProductListCatalogResponse {
        pub id: Option<String>,
        pub status: Option<String>,
    }

    impl IqSpec for QueryProductListCatalogSpec {
        type Response = QueryProductListCatalogResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut product_list_node = NodeBuilder::new("product_list");
            product_list_node = product_list_node.attr("jid", self.jid.clone());
            let product_list_node = product_list_node.build();

            InfoQuery::get(
                W_BIZ_CATALOG_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![product_list_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let product_list = response
                .get_optional_child("product_list")
                .ok_or_else(|| anyhow::anyhow!("missing <product_list>"))?;
            let id = response
                .get_optional_child("id")
                .and_then(|n| n.content_str().map(|s| s.to_string()));
            let status = response
                .get_optional_child("status")
                .and_then(|n| n.content_str().map(|s| s.to_string()));
            Ok(QueryProductListCatalogResponse {
                id,
                status,
                ..Default::default()
            })
        }
    }

    /// VerifyPostcode:get IQ spec.
    ///
    /// Source: `WAWebVerifyPostcodeJob`
    #[derive(Debug, Clone)]
    pub struct VerifyPostcodeSpec {
        pub biz_jid: Jid,
    }

    impl VerifyPostcodeSpec {
        pub fn new(biz_jid: &Jid) -> Self {
            Self {
                biz_jid: biz_jid.clone(),
            }
        }
    }

    /// Response from VerifyPostcode:get.
    #[derive(Debug, Clone, Default)]
    pub struct VerifyPostcodeResponse {
        pub result_code: String,
        pub encrypted_location_name: Option<String>,
    }

    impl IqSpec for VerifyPostcodeSpec {
        type Response = VerifyPostcodeResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let direct_connection_encrypted_info_node =
                NodeBuilder::new("direct_connection_encrypted_info").build();
            let mut verify_postcode_node = NodeBuilder::new("verify_postcode");
            verify_postcode_node = verify_postcode_node.attr("biz_jid", self.biz_jid.clone());
            verify_postcode_node =
                verify_postcode_node.children([direct_connection_encrypted_info_node]);
            let verify_postcode_node = verify_postcode_node.build();

            InfoQuery::get(
                W_BIZ_CATALOG_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![verify_postcode_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let result_code_node = response
                .get_optional_child("result_code")
                .ok_or_else(|| anyhow::anyhow!("missing <result_code>"))?;
            let result_code = result_code_node
                .content_str()
                .unwrap_or_default()
                .to_string();
            let encrypted_location_name = response
                .get_optional_child("encrypted_location_name")
                .and_then(|n| n.content_str().map(|s| s.to_string()));
            Ok(VerifyPostcodeResponse {
                result_code,
                encrypted_location_name,
                ..Default::default()
            })
        }
    }
}

/// IQ namespace `w:biz:msg_feedback`. Source: WASmaxOutBizMsgUserFeedbackUpdatePreferenceRequest.
pub mod w_biz_msg_feedback {
    use crate::iq::spec::IqSpec;
    use crate::request::InfoQuery;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::jid::{Jid, Server};
    use wacore_binary::node::NodeContent;

    /// IQ namespace.
    pub const W_BIZ_MSG_FEEDBACK_NAMESPACE: &str = "w:biz:msg_feedback";

    // ─── IQ Specs ───────────────────────────────────────────────────────────────

    /// makeUpdatePreferenceRequest:set IQ spec.
    ///
    /// Source: `WASmaxOutBizMsgUserFeedbackUpdatePreferenceRequest`
    #[derive(Debug, Clone)]
    pub struct MakeUpdatePreferenceRequestSpec {
        pub action: String,
        pub jid: Jid,
        pub feedback: Option<String>,
    }

    impl MakeUpdatePreferenceRequestSpec {
        pub fn new(action: impl Into<String>, jid: &Jid, feedback: Option<String>) -> Self {
            Self {
                action: action.into(),
                jid: jid.clone(),
                feedback,
            }
        }
    }

    /// Response from makeUpdatePreferenceRequest:set.
    #[derive(Debug, Clone, Default)]
    pub struct MakeUpdatePreferenceRequestResponse {
        pub r#type: String,
    }

    impl IqSpec for MakeUpdatePreferenceRequestSpec {
        type Response = MakeUpdatePreferenceRequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut user_feedback_node = NodeBuilder::new("user_feedback");
            user_feedback_node = user_feedback_node.attr("action", &*self.action);
            user_feedback_node = user_feedback_node.attr("jid", self.jid.clone());
            if let Some(v) = &self.feedback {
                user_feedback_node = user_feedback_node.attr("feedback", v.as_str());
            }
            let user_feedback_node = user_feedback_node.build();

            InfoQuery::set(
                W_BIZ_MSG_FEEDBACK_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![user_feedback_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            Ok(MakeUpdatePreferenceRequestResponse {
                r#type,
                ..Default::default()
            })
        }
    }
}

/// IQ namespace `w:comms`. Source: WASmaxOutInAppCommsEventRequest.
pub mod w_comms {
    use crate::iq::spec::IqSpec;
    use crate::request::InfoQuery;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::jid::{Jid, Server};
    use wacore_binary::node::NodeContent;

    /// IQ namespace.
    pub const W_COMMS_NAMESPACE: &str = "w:comms";

    // ─── IQ Specs ───────────────────────────────────────────────────────────────

    /// makeEventRequest:set IQ spec.
    ///
    /// Source: `WASmaxOutInAppCommsEventRequest`
    #[derive(Debug, Clone)]
    pub struct MakeEventRequestSpec {
        pub promotion_id: String,
        pub r#type: String,
        pub timestamp_sec: u64,
        pub logdata: String,
    }

    impl MakeEventRequestSpec {
        pub fn new(
            promotion_id: impl Into<String>,
            r#type: impl Into<String>,
            timestamp_sec: u64,
            logdata: impl Into<String>,
        ) -> Self {
            Self {
                promotion_id: promotion_id.into(),
                r#type: r#type.into(),
                timestamp_sec,
                logdata: logdata.into(),
            }
        }
    }

    /// Response from makeEventRequest:set.
    #[derive(Debug, Clone, Default)]
    pub struct MakeEventRequestResponse {
        pub r#type: String,
    }

    impl IqSpec for MakeEventRequestSpec {
        type Response = MakeEventRequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut event_node = NodeBuilder::new("event");
            event_node = event_node.attr("promotion_id", &*self.promotion_id);
            event_node = event_node.attr("type", &*self.r#type);
            event_node = event_node.attr("timestamp_sec", self.timestamp_sec.to_string());
            event_node = event_node.attr("logdata", &*self.logdata);
            let event_node = event_node.build();

            InfoQuery::set(
                W_COMMS_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![event_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            Ok(MakeEventRequestResponse {
                r#type,
                ..Default::default()
            })
        }
    }
}

/// IQ namespace `w:comms:chat`. Source: WASmaxOutPsaChatBlockGetRequest, WASmaxOutPsaChatBlockSetRequest.
pub mod w_comms_chat {
    use crate::iq::spec::IqSpec;
    use crate::request::InfoQuery;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::jid::{Jid, Server};
    use wacore_binary::node::NodeContent;

    /// IQ namespace.
    pub const W_COMMS_CHAT_NAMESPACE: &str = "w:comms:chat";

    // ─── IQ Specs ───────────────────────────────────────────────────────────────

    /// makeChatBlockGetRequest:get IQ spec.
    ///
    /// Source: `WASmaxOutPsaChatBlockGetRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeChatBlockGetRequestSpec;

    /// Response from makeChatBlockGetRequest:get.
    #[derive(Debug, Clone, Default)]
    pub struct MakeChatBlockGetRequestResponse {
        pub blocking_status: String,
        pub r#type: String,
    }

    impl IqSpec for MakeChatBlockGetRequestSpec {
        type Response = MakeChatBlockGetRequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let blocking_status_node = NodeBuilder::new("blocking_status").build();
            let mut query_node = NodeBuilder::new("query");
            query_node = query_node.children([blocking_status_node]);
            let query_node = query_node.build();

            InfoQuery::get(
                W_COMMS_CHAT_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![query_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let blocking_wrap = response
                .get_optional_child("blocking")
                .ok_or_else(|| anyhow::anyhow!("missing <blocking>"))?;
            let blocking_status = blocking_wrap
                .get_attr("status")
                .ok_or_else(|| anyhow::anyhow!("missing status"))?
                .as_str()
                .to_string();
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            Ok(MakeChatBlockGetRequestResponse {
                blocking_status,
                r#type,
                ..Default::default()
            })
        }
    }

    /// makeChatBlockSetRequest:set IQ spec.
    ///
    /// Source: `WASmaxOutPsaChatBlockSetRequest`
    #[derive(Debug, Clone)]
    pub struct MakeChatBlockSetRequestSpec {
        pub action: String,
    }

    impl MakeChatBlockSetRequestSpec {
        pub fn new(action: impl Into<String>) -> Self {
            Self {
                action: action.into(),
            }
        }
    }

    /// Response from makeChatBlockSetRequest:set.
    #[derive(Debug, Clone, Default)]
    pub struct MakeChatBlockSetRequestResponse {
        pub blocking_status: String,
        pub r#type: String,
    }

    impl IqSpec for MakeChatBlockSetRequestSpec {
        type Response = MakeChatBlockSetRequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut blocking_node = NodeBuilder::new("blocking");
            blocking_node = blocking_node.attr("action", &*self.action);
            let blocking_node = blocking_node.build();

            InfoQuery::set(
                W_COMMS_CHAT_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![blocking_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let blocking_wrap = response
                .get_optional_child("blocking")
                .ok_or_else(|| anyhow::anyhow!("missing <blocking>"))?;
            let blocking_status = blocking_wrap
                .get_attr("status")
                .ok_or_else(|| anyhow::anyhow!("missing status"))?
                .as_str()
                .to_string();
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            Ok(MakeChatBlockSetRequestResponse {
                blocking_status,
                r#type,
                ..Default::default()
            })
        }
    }
}

/// IQ namespace `w:g2`. Source: WASmaxOutGroupsBatchGetGroupInfoRequest, WASmaxOutGroupsGetGroupInfoRequest, WASmaxOutGroupsGetInviteGroupInfoRequest, WASmaxOutGroupsGetLinkedGroupRequest, WASmaxOutGroupsGetLinkedGroupsParticipantsRequest, WASmaxOutGroupsGetMembershipApprovalRequestsRequest, WASmaxOutGroupsGetParticipatingGroupsRequest, WASmaxOutGroupsGetReportedMessagesRequest, WASmaxOutGroupsBaseGetGroupMixin, WASmaxOutGroupsBaseGetServerMixin, WAWebQueryGroupInviteProfilePicApi, WAWebGroupExitJob, WASmaxOutGroupsAcceptGroupAddRequest, WASmaxOutGroupsAcknowledgeGroupRequest, WASmaxOutGroupsAddParticipantsRequest, WASmaxOutGroupsCancelGroupMembershipRequestsRequest, WASmaxOutGroupsCreateRequest, WASmaxOutGroupsCreateSubGroupSuggestionRequest, WASmaxOutGroupsDeleteParentGroupRequest, WASmaxOutGroupsJoinLinkedGroupRequest, WASmaxOutGroupsLinkSubGroupsRequest, WASmaxOutGroupsMembershipRequestsActionRequest, WASmaxOutGroupsPromoteDemoteAdminRequest, WASmaxOutGroupsPromoteDemoteRequest, WASmaxOutGroupsRemoveParticipantsRequest, WASmaxOutGroupsReportMessagesRequest, WASmaxOutGroupsRevokeRequestCodeRequest, WASmaxOutGroupsSetDescriptionRequest, WASmaxOutGroupsSetPropertyRequest, WASmaxOutGroupsSetSubjectRequest, WASmaxOutGroupsSubGroupSuggestionsActionRequest, WASmaxOutGroupsUnlinkGroupsRequest, WASmaxOutGroupsBaseSetGroupMixin, WASmaxOutGroupsBaseSetServerMixin, WAWebGroupInviteJob.
pub mod w_g2 {
    use crate::iq::spec::IqSpec;
    use crate::request::InfoQuery;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::jid::{Jid, Server};
    use wacore_binary::node::NodeContent;

    /// IQ namespace.
    pub const W_G2_NAMESPACE: &str = "w:g2";

    // ─── Shared child types ──────────────────────────────────────────────────────

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeBatchGetGroupInfoRequestQueryGroupGroupItem {
        pub key: Option<String>,
        pub create_ctx: Option<String>,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetGroupInfoRequestQueryAddRequestParticipantItem {
        pub addressable: Option<String>,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetLinkedGroupsParticipantsRequestParticipantItem {
        pub jid: Jid,
        pub phone_number: Jid,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetMembershipApprovalRequestsRequestMembershipApprovalRequestItem {
        pub jid: Jid,
        pub requestor: Jid,
        pub requestor_pn: Jid,
        pub requestor_username: Option<String>,
        pub parent_group_jid: Jid,
        pub request_time: u64,
        pub request_method: String,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetParticipatingGroupsRequestParticipatingParticipantsGroupItem {
        pub key: Option<String>,
        pub create_ctx: Option<String>,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetReportedMessagesRequestReporterItem {
        pub jid: Jid,
        pub timestamp: u64,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetReportedMessagesRequestReportItem {
        pub message_id: String,
        pub reporter: Vec<MakeGetReportedMessagesRequestReporterItem>,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct LeaveGroupChildrenItem {
        pub id: Jid,
        pub error: Option<u64>,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeCancelGroupMembershipRequestsRequestCancelMembershipRequestsParticipantParticipantItem
    {
        pub jid: Jid,
        pub phone_number: Jid,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeLinkSubGroupsRequestLinksLinkGroupHiddenGroupParticipantItem {
        pub jid: Jid,
        pub error: String,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeLinkSubGroupsRequestLinksLinkGroupHiddenGroupGroupItem {
        pub jid: Jid,
        pub participant: Vec<MakeLinkSubGroupsRequestLinksLinkGroupHiddenGroupParticipantItem>,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeMembershipRequestsActionRequestMembershipRequestsActionApproveParticipantParticipantItem
    {
        pub jid: Jid,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakePromoteDemoteAdminRequestAdminPromoteParticipantParticipantItem {
        pub jid: Jid,
        pub r#type: Option<String>,
        pub error: Option<String>,
        pub phone_number: Jid,
        pub username: Option<String>,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakePromoteDemoteRequestPromoteParticipantParticipantItem {
        pub jid: Jid,
        pub r#type: Option<String>,
        pub error: Option<String>,
        pub phone_number: Jid,
        pub username: Option<String>,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeRemoveParticipantsRequestRemoveParticipantParticipantItem {
        pub jid: Jid,
        pub phone_number: Jid,
        pub username: Option<String>,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeRevokeRequestCodeRequestRevokeParticipantParticipantItem {
        pub jid: Jid,
        pub error: Option<String>,
        pub phone_number: Jid,
        pub username: Option<String>,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeSubGroupSuggestionsActionRequestSubGroupSuggestionsActionApproveSubGroupSuggestionSubGroupSuggestionItem
    {
        pub creator: Jid,
        pub jid: Jid,
        pub creator_pn: Jid,
        pub error: Option<String>,
    }

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeUnlinkGroupsRequestUnlinkGroupGroupItem {
        pub jid: Jid,
        pub remove_orphaned_members: Option<String>,
    }

    // ─── IQ Specs ───────────────────────────────────────────────────────────────

    /// makeBatchGetGroupInfoRequestQueryGroup:get IQ spec.
    ///
    /// Source: `WASmaxOutGroupsBatchGetGroupInfoRequest`
    #[derive(Debug, Clone)]
    pub struct MakeBatchGetGroupInfoRequestQueryGroupSpec {
        pub context: Option<String>,
    }

    impl MakeBatchGetGroupInfoRequestQueryGroupSpec {
        pub fn new(context: Option<String>) -> Self {
            Self { context }
        }
    }

    impl IqSpec for MakeBatchGetGroupInfoRequestQueryGroupSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut query_node = NodeBuilder::new("query");
            if let Some(v) = &self.context {
                query_node = query_node.attr("context", v.as_str());
            }
            let query_node = query_node.build();

            InfoQuery::get(
                W_G2_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![query_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// makeGetGroupInfoRequestQueryAddRequest:get IQ spec.
    ///
    /// Source: `WASmaxOutGroupsGetGroupInfoRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetGroupInfoRequestQueryAddRequestSpec;

    impl IqSpec for MakeGetGroupInfoRequestQueryAddRequestSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::get(W_G2_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// makeGetInviteGroupInfoRequest:get IQ spec.
    ///
    /// Source: `WASmaxOutGroupsGetInviteGroupInfoRequest`
    #[derive(Debug, Clone)]
    pub struct MakeGetInviteGroupInfoRequestSpec {
        pub code: String,
    }

    impl MakeGetInviteGroupInfoRequestSpec {
        pub fn new(code: impl Into<String>) -> Self {
            Self { code: code.into() }
        }
    }

    impl IqSpec for MakeGetInviteGroupInfoRequestSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut invite_node = NodeBuilder::new("invite");
            invite_node = invite_node.attr("code", &*self.code);
            let invite_node = invite_node.build();

            InfoQuery::get(
                W_G2_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![invite_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// makeGetLinkedGroupRequest:get IQ spec.
    ///
    /// Source: `WASmaxOutGroupsGetLinkedGroupRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetLinkedGroupRequestSpec;

    impl IqSpec for MakeGetLinkedGroupRequestSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::get(W_G2_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// makeGetLinkedGroupsParticipantsRequest:get IQ spec.
    ///
    /// Source: `WASmaxOutGroupsGetLinkedGroupsParticipantsRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetLinkedGroupsParticipantsRequestSpec;

    impl IqSpec for MakeGetLinkedGroupsParticipantsRequestSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            let linked_groups_participants_node =
                NodeBuilder::new("linked_groups_participants").build();

            InfoQuery::get(
                W_G2_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![linked_groups_participants_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// makeGetMembershipApprovalRequestsRequest:get IQ spec.
    ///
    /// Source: `WASmaxOutGroupsGetMembershipApprovalRequestsRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetMembershipApprovalRequestsRequestSpec;

    impl IqSpec for MakeGetMembershipApprovalRequestsRequestSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::get(W_G2_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// makeGetParticipatingGroupsRequestParticipatingParticipants:get IQ spec.
    ///
    /// Source: `WASmaxOutGroupsGetParticipatingGroupsRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetParticipatingGroupsRequestParticipatingParticipantsSpec;

    impl IqSpec for MakeGetParticipatingGroupsRequestParticipatingParticipantsSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            let participants_node = NodeBuilder::new("participants").build();
            let mut participating_node = NodeBuilder::new("participating");
            participating_node = participating_node.children([participants_node]);
            let participating_node = participating_node.build();

            InfoQuery::get(
                W_G2_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![participating_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// makeGetReportedMessagesRequest:get IQ spec.
    ///
    /// Source: `WASmaxOutGroupsGetReportedMessagesRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetReportedMessagesRequestSpec;

    impl IqSpec for MakeGetReportedMessagesRequestSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            let reports_node = NodeBuilder::new("reports").build();

            InfoQuery::get(
                W_G2_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![reports_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// mergeBaseGetGroupMixin:get IQ spec.
    ///
    /// Source: `WASmaxOutGroupsBaseGetGroupMixin`
    #[derive(Debug, Clone, Default)]
    pub struct MergeBaseGetGroupMixinSpec;

    impl IqSpec for MergeBaseGetGroupMixinSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::get(W_G2_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// mergeBaseGetServerMixin:get IQ spec.
    ///
    /// Source: `WASmaxOutGroupsBaseGetServerMixin`
    #[derive(Debug, Clone, Default)]
    pub struct MergeBaseGetServerMixinSpec;

    impl IqSpec for MergeBaseGetServerMixinSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::get(W_G2_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// queryGroupInviteLinkProfilePic:get IQ spec.
    ///
    /// Source: `WAWebQueryGroupInviteProfilePicApi`
    #[derive(Debug, Clone)]
    pub struct QueryGroupInviteLinkProfilePicSpec {
        pub id: Option<String>,
        pub r#type: String,
        pub query: String,
        pub invite: String,
    }

    impl QueryGroupInviteLinkProfilePicSpec {
        pub fn new(
            id: Option<String>,
            r#type: impl Into<String>,
            query: impl Into<String>,
            invite: impl Into<String>,
        ) -> Self {
            Self {
                id,
                r#type: r#type.into(),
                query: query.into(),
                invite: invite.into(),
            }
        }
    }

    /// Response from queryGroupInviteLinkProfilePic:get.
    #[derive(Debug, Clone, Default)]
    pub struct QueryGroupInviteLinkProfilePicResponse {
        pub id: String,
        pub r#type: String,
        pub url: String,
        pub direct_path: String,
    }

    impl IqSpec for QueryGroupInviteLinkProfilePicSpec {
        type Response = QueryGroupInviteLinkProfilePicResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut picture_node = NodeBuilder::new("picture");
            if let Some(v) = &self.id {
                picture_node = picture_node.attr("id", v.as_str());
            }
            picture_node = picture_node.attr("type", &*self.r#type);
            picture_node = picture_node.attr("query", &*self.query);
            picture_node = picture_node.attr("invite", &*self.invite);
            let picture_node = picture_node.build();

            InfoQuery::get(
                W_G2_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![picture_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let picture = response
                .get_optional_child("picture")
                .ok_or_else(|| anyhow::anyhow!("missing <picture>"))?;
            let id = picture
                .get_attr("id")
                .ok_or_else(|| anyhow::anyhow!("missing id"))?
                .as_str()
                .to_string();
            let r#type = picture
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            let url = picture
                .get_attr("url")
                .ok_or_else(|| anyhow::anyhow!("missing url"))?
                .as_str()
                .to_string();
            let direct_path = picture
                .get_attr("direct_path")
                .ok_or_else(|| anyhow::anyhow!("missing direct_path"))?
                .as_str()
                .to_string();
            Ok(QueryGroupInviteLinkProfilePicResponse {
                id,
                r#type,
                url,
                direct_path,
                ..Default::default()
            })
        }
    }

    /// leaveGroup:set IQ spec.
    ///
    /// Source: `WAWebGroupExitJob`
    #[derive(Debug, Clone)]
    pub struct LeaveGroupSpec {
        pub id: Jid,
    }

    impl LeaveGroupSpec {
        pub fn new(id: &Jid) -> Self {
            Self { id: id.clone() }
        }
    }

    /// Response from leaveGroup:set.
    #[derive(Debug, Clone, Default)]
    pub struct LeaveGroupResponse {
        pub children: Vec<LeaveGroupChildrenItem>,
        pub id: Jid,
        pub error: Option<u64>,
    }

    impl IqSpec for LeaveGroupSpec {
        type Response = LeaveGroupResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut group_node = NodeBuilder::new("group");
            group_node = group_node.attr("id", self.id.clone());
            let group_node = group_node.build();
            let mut leave_node = NodeBuilder::new("leave");
            leave_node = leave_node.children([group_node]);
            let leave_node = leave_node.build();

            InfoQuery::set(
                W_G2_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![leave_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let id = response
                .get_attr("id")
                .and_then(|v| v.to_jid())
                .ok_or_else(|| anyhow::anyhow!("missing id"))?;
            let error = response
                .get_attr("error")
                .and_then(|v| v.as_str().parse().ok());
            let leave = response
                .get_optional_child("leave")
                .ok_or_else(|| anyhow::anyhow!("missing <leave>"))?;
            let mut children_items = Vec::new();
            for child in leave.get_children_by_tag("children") {
                let id = child
                    .get_attr("id")
                    .and_then(|v| v.to_jid())
                    .ok_or_else(|| anyhow::anyhow!("missing id"))?;
                let error = child
                    .get_attr("error")
                    .and_then(|v| v.as_str().parse().ok());
                children_items.push(LeaveGroupChildrenItem {
                    id,
                    error,
                    ..Default::default()
                });
            }
            Ok(LeaveGroupResponse {
                id,
                error,
                children: children_items,
                ..Default::default()
            })
        }
    }

    /// makeAcceptGroupAddRequest:set IQ spec.
    ///
    /// Source: `WASmaxOutGroupsAcceptGroupAddRequest`
    #[derive(Debug, Clone)]
    pub struct MakeAcceptGroupAddRequestSpec {
        pub code: String,
        pub expiration: u64,
        pub admin: Jid,
    }

    impl MakeAcceptGroupAddRequestSpec {
        pub fn new(code: impl Into<String>, expiration: u64, admin: &Jid) -> Self {
            Self {
                code: code.into(),
                expiration,
                admin: admin.clone(),
            }
        }
    }

    /// Response from makeAcceptGroupAddRequest:set.
    #[derive(Debug, Clone, Default)]
    pub struct MakeAcceptGroupAddRequestResponse {
        pub r#type: String,
    }

    impl IqSpec for MakeAcceptGroupAddRequestSpec {
        type Response = MakeAcceptGroupAddRequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut accept_node = NodeBuilder::new("accept");
            accept_node = accept_node.attr("code", &*self.code);
            accept_node = accept_node.attr("expiration", self.expiration.to_string());
            accept_node = accept_node.attr("admin", self.admin.clone());
            let accept_node = accept_node.build();

            InfoQuery::set(
                W_G2_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![accept_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            Ok(MakeAcceptGroupAddRequestResponse {
                r#type,
                ..Default::default()
            })
        }
    }

    /// makeAcknowledgeGroupRequest:set IQ spec.
    ///
    /// Source: `WASmaxOutGroupsAcknowledgeGroupRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeAcknowledgeGroupRequestSpec;

    /// Response from makeAcknowledgeGroupRequest:set.
    #[derive(Debug, Clone, Default)]
    pub struct MakeAcknowledgeGroupRequestResponse {
        pub r#type: String,
    }

    impl IqSpec for MakeAcknowledgeGroupRequestSpec {
        type Response = MakeAcknowledgeGroupRequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let ack_node = NodeBuilder::new("ack").build();

            InfoQuery::set(
                W_G2_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![ack_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            Ok(MakeAcknowledgeGroupRequestResponse {
                r#type,
                ..Default::default()
            })
        }
    }

    /// makeAddParticipantsRequestAddParticipant:set IQ spec.
    ///
    /// Source: `WASmaxOutGroupsAddParticipantsRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeAddParticipantsRequestAddParticipantSpec;

    /// Response from makeAddParticipantsRequestAddParticipant:set.
    #[derive(Debug, Clone, Default)]
    pub struct MakeAddParticipantsRequestAddParticipantResponse {
        pub r#type: String,
        pub addressing_mode: Option<String>,
    }

    impl IqSpec for MakeAddParticipantsRequestAddParticipantSpec {
        type Response = MakeAddParticipantsRequestAddParticipantResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let add_node = NodeBuilder::new("add").build();

            InfoQuery::set(
                W_G2_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![add_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            let addressing_mode = response
                .get_attr("addressing_mode")
                .map(|v| v.as_str().to_string());
            Ok(MakeAddParticipantsRequestAddParticipantResponse {
                r#type,
                addressing_mode,
                ..Default::default()
            })
        }
    }

    /// makeCancelGroupMembershipRequestsRequestCancelMembershipRequestsParticipant:set IQ spec.
    ///
    /// Source: `WASmaxOutGroupsCancelGroupMembershipRequestsRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeCancelGroupMembershipRequestsRequestCancelMembershipRequestsParticipantSpec;

    impl IqSpec for MakeCancelGroupMembershipRequestsRequestCancelMembershipRequestsParticipantSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            let cancel_membership_requests_node =
                NodeBuilder::new("cancel_membership_requests").build();

            InfoQuery::set(
                W_G2_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![cancel_membership_requests_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// makeCreateRequestCreateParticipant:set IQ spec.
    ///
    /// Source: `WASmaxOutGroupsCreateRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeCreateRequestCreateParticipantSpec;

    /// Response from makeCreateRequestCreateParticipant:set.
    #[derive(Debug, Clone, Default)]
    pub struct MakeCreateRequestCreateParticipantResponse {
        pub from: Jid,
        pub r#type: String,
        pub group_id: String,
        pub group_creator: Jid,
        pub group_creation: u64,
        pub group_st: Option<u64>,
        pub group_so: Jid,
        pub key: String,
        pub create_ctx: Option<String>,
        pub addressing_mode: String,
        pub s_o_pn: Jid,
        pub s_o_username: Option<String>,
        pub creator_pn: Jid,
        pub creator_username: Option<String>,
        pub creator_country_code: Option<String>,
        pub id: String,
        pub error: Option<String>,
        pub default_membership_approval_mode: Option<String>,
        pub expiration: u64,
        pub trigger: Option<u64>,
        pub state: String,
        pub jid: Jid,
    }

    impl IqSpec for MakeCreateRequestCreateParticipantSpec {
        type Response = MakeCreateRequestCreateParticipantResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::set(W_G2_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let from = response
                .get_attr("from")
                .and_then(|v| v.to_jid())
                .ok_or_else(|| anyhow::anyhow!("missing from"))?;
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            let group_wrap = response
                .get_optional_child("group")
                .ok_or_else(|| anyhow::anyhow!("missing <group>"))?;
            let group_id = group_wrap
                .get_attr("id")
                .ok_or_else(|| anyhow::anyhow!("missing id"))?
                .as_str()
                .to_string();
            let group_creator = group_wrap
                .get_attr("creator")
                .and_then(|v| v.to_jid())
                .ok_or_else(|| anyhow::anyhow!("missing creator"))?;
            let group_creation: u64 = group_wrap
                .get_attr("creation")
                .ok_or_else(|| anyhow::anyhow!("missing creation"))?
                .as_str()
                .parse()?;
            let group_st = group_wrap
                .get_attr("s_t")
                .and_then(|v| v.as_str().parse().ok());
            let group_so = group_wrap
                .get_attr("s_o")
                .and_then(|v| v.to_jid())
                .ok_or_else(|| anyhow::anyhow!("missing s_o"))?;
            let group = response
                .get_optional_child("group")
                .ok_or_else(|| anyhow::anyhow!("missing <group>"))?;
            let key = group
                .get_attr("key")
                .ok_or_else(|| anyhow::anyhow!("missing key"))?
                .as_str()
                .to_string();
            let create_ctx = group.get_attr("create_ctx").map(|v| v.as_str().to_string());
            let group = response
                .get_optional_child("group")
                .ok_or_else(|| anyhow::anyhow!("missing <group>"))?;
            let addressing_mode = group
                .get_attr("addressing_mode")
                .ok_or_else(|| anyhow::anyhow!("missing addressing_mode"))?
                .as_str()
                .to_string();
            let group = response
                .get_optional_child("group")
                .ok_or_else(|| anyhow::anyhow!("missing <group>"))?;
            let s_o_pn = group
                .get_attr("s_o_pn")
                .and_then(|v| v.to_jid())
                .ok_or_else(|| anyhow::anyhow!("missing s_o_pn"))?;
            let s_o_username = group
                .get_attr("s_o_username")
                .map(|v| v.as_str().to_string());
            let group = response
                .get_optional_child("group")
                .ok_or_else(|| anyhow::anyhow!("missing <group>"))?;
            let creator_pn = group
                .get_attr("creator_pn")
                .and_then(|v| v.to_jid())
                .ok_or_else(|| anyhow::anyhow!("missing creator_pn"))?;
            let creator_username = group
                .get_attr("creator_username")
                .map(|v| v.as_str().to_string());
            let creator_country_code = group
                .get_attr("creator_country_code")
                .map(|v| v.as_str().to_string());
            let description = group_wrap
                .get_optional_child("description")
                .ok_or_else(|| anyhow::anyhow!("missing <description>"))?;
            let id = description
                .get_attr("id")
                .ok_or_else(|| anyhow::anyhow!("missing id"))?
                .as_str()
                .to_string();
            let error = description
                .get_attr("error")
                .map(|v| v.as_str().to_string());
            let parent = group_wrap
                .get_optional_child("parent")
                .ok_or_else(|| anyhow::anyhow!("missing <parent>"))?;
            let default_membership_approval_mode = parent
                .get_attr("default_membership_approval_mode")
                .map(|v| v.as_str().to_string());
            let ephemeral = group_wrap
                .get_optional_child("ephemeral")
                .ok_or_else(|| anyhow::anyhow!("missing <ephemeral>"))?;
            let expiration: u64 = ephemeral
                .get_attr("expiration")
                .ok_or_else(|| anyhow::anyhow!("missing expiration"))?
                .as_str()
                .parse()?;
            let trigger = ephemeral
                .get_attr("trigger")
                .and_then(|v| v.as_str().parse().ok());
            let membership_approval_mode = group_wrap
                .get_optional_child("membership_approval_mode")
                .ok_or_else(|| anyhow::anyhow!("missing <membership_approval_mode>"))?;
            let state = membership_approval_mode
                .get_attr("state")
                .ok_or_else(|| anyhow::anyhow!("missing state"))?
                .as_str()
                .to_string();
            let linked_parent = group_wrap
                .get_optional_child("linked_parent")
                .ok_or_else(|| anyhow::anyhow!("missing <linked_parent>"))?;
            let jid = linked_parent
                .get_attr("jid")
                .and_then(|v| v.to_jid())
                .ok_or_else(|| anyhow::anyhow!("missing jid"))?;
            Ok(MakeCreateRequestCreateParticipantResponse {
                from,
                r#type,
                group_id,
                group_creator,
                group_creation,
                group_st,
                group_so,
                key,
                create_ctx,
                addressing_mode,
                s_o_pn,
                s_o_username,
                creator_pn,
                creator_username,
                creator_country_code,
                id,
                error,
                default_membership_approval_mode,
                expiration,
                trigger,
                state,
                jid,
                ..Default::default()
            })
        }
    }

    /// makeCreateSubGroupSuggestionRequest:set IQ spec.
    ///
    /// Source: `WASmaxOutGroupsCreateSubGroupSuggestionRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeCreateSubGroupSuggestionRequestSpec;

    /// Response from makeCreateSubGroupSuggestionRequest:set.
    #[derive(Debug, Clone, Default)]
    pub struct MakeCreateSubGroupSuggestionRequestResponse {
        pub sub_group_suggestion_jid: Jid,
        pub sub_group_suggestion_creator: Jid,
        pub sub_group_suggestion_creation: u64,
        pub creator_pn: Jid,
        pub r#type: String,
        pub addressing_mode: Option<String>,
        pub error: Option<String>,
    }

    impl IqSpec for MakeCreateSubGroupSuggestionRequestSpec {
        type Response = MakeCreateSubGroupSuggestionRequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::set(W_G2_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let sub_group_suggestion_wrap = response
                .get_optional_child("sub_group_suggestion")
                .ok_or_else(|| anyhow::anyhow!("missing <sub_group_suggestion>"))?;
            let sub_group_suggestion_jid = sub_group_suggestion_wrap
                .get_attr("jid")
                .and_then(|v| v.to_jid())
                .ok_or_else(|| anyhow::anyhow!("missing jid"))?;
            let sub_group_suggestion_creator = sub_group_suggestion_wrap
                .get_attr("creator")
                .and_then(|v| v.to_jid())
                .ok_or_else(|| anyhow::anyhow!("missing creator"))?;
            let sub_group_suggestion_creation: u64 = sub_group_suggestion_wrap
                .get_attr("creation")
                .ok_or_else(|| anyhow::anyhow!("missing creation"))?
                .as_str()
                .parse()?;
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            let addressing_mode = response
                .get_attr("addressing_mode")
                .map(|v| v.as_str().to_string());
            let sub_group_suggestion = response
                .get_optional_child("sub_group_suggestion")
                .ok_or_else(|| anyhow::anyhow!("missing <sub_group_suggestion>"))?;
            let creator_pn = sub_group_suggestion
                .get_attr("creator_pn")
                .and_then(|v| v.to_jid())
                .ok_or_else(|| anyhow::anyhow!("missing creator_pn"))?;
            let description = sub_group_suggestion_wrap
                .get_optional_child("description")
                .ok_or_else(|| anyhow::anyhow!("missing <description>"))?;
            let error = description
                .get_attr("error")
                .map(|v| v.as_str().to_string());
            Ok(MakeCreateSubGroupSuggestionRequestResponse {
                sub_group_suggestion_jid,
                sub_group_suggestion_creator,
                sub_group_suggestion_creation,
                r#type,
                addressing_mode,
                creator_pn,
                error,
                ..Default::default()
            })
        }
    }

    /// makeDeleteParentGroupRequest:set IQ spec.
    ///
    /// Source: `WASmaxOutGroupsDeleteParentGroupRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeDeleteParentGroupRequestSpec;

    /// Response from makeDeleteParentGroupRequest:set.
    #[derive(Debug, Clone, Default)]
    pub struct MakeDeleteParentGroupRequestResponse {
        pub r#type: String,
    }

    impl IqSpec for MakeDeleteParentGroupRequestSpec {
        type Response = MakeDeleteParentGroupRequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let delete_parent_node = NodeBuilder::new("delete_parent").build();

            InfoQuery::set(
                W_G2_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![delete_parent_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            Ok(MakeDeleteParentGroupRequestResponse {
                r#type,
                ..Default::default()
            })
        }
    }

    /// makeJoinLinkedGroupRequest:set IQ spec.
    ///
    /// Source: `WASmaxOutGroupsJoinLinkedGroupRequest`
    #[derive(Debug, Clone)]
    pub struct MakeJoinLinkedGroupRequestSpec {
        pub r#type: Option<String>,
        pub jid: Jid,
    }

    impl MakeJoinLinkedGroupRequestSpec {
        pub fn new(r#type: Option<String>, jid: &Jid) -> Self {
            Self {
                r#type,
                jid: jid.clone(),
            }
        }
    }

    /// Response from makeJoinLinkedGroupRequest:set.
    #[derive(Debug, Clone, Default)]
    pub struct MakeJoinLinkedGroupRequestResponse {
        pub r#type: String,
    }

    impl IqSpec for MakeJoinLinkedGroupRequestSpec {
        type Response = MakeJoinLinkedGroupRequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut join_linked_group_node = NodeBuilder::new("join_linked_group");
            if let Some(v) = &self.r#type {
                join_linked_group_node = join_linked_group_node.attr("type", v.as_str());
            }
            join_linked_group_node = join_linked_group_node.attr("jid", self.jid.clone());
            let join_linked_group_node = join_linked_group_node.build();

            InfoQuery::set(
                W_G2_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![join_linked_group_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            Ok(MakeJoinLinkedGroupRequestResponse {
                r#type,
                ..Default::default()
            })
        }
    }

    /// makeLinkSubGroupsRequestLinksLinkGroupHiddenGroup:set IQ spec.
    ///
    /// Source: `WASmaxOutGroupsLinkSubGroupsRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeLinkSubGroupsRequestLinksLinkGroupHiddenGroupSpec;

    impl IqSpec for MakeLinkSubGroupsRequestLinksLinkGroupHiddenGroupSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut link_node = NodeBuilder::new("link");
            link_node = link_node.attr("link_type", "sub_group");
            let link_node = link_node.build();
            let mut links_node = NodeBuilder::new("links");
            links_node = links_node.children([link_node]);
            let links_node = links_node.build();

            InfoQuery::set(
                W_G2_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![links_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// makeMembershipRequestsActionRequestMembershipRequestsActionApproveParticipant:set IQ spec.
    ///
    /// Source: `WASmaxOutGroupsMembershipRequestsActionRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeMembershipRequestsActionRequestMembershipRequestsActionApproveParticipantSpec;

    /// Response from makeMembershipRequestsActionRequestMembershipRequestsActionApproveParticipant:set.
    #[derive(Debug, Clone, Default)]
    pub struct MakeMembershipRequestsActionRequestMembershipRequestsActionApproveParticipantResponse {
        pub r#type: String,
        pub addressing_mode: Option<String>,
        pub participant: Vec<MakeMembershipRequestsActionRequestMembershipRequestsActionApproveParticipantParticipantItem>,
    }

    impl IqSpec for MakeMembershipRequestsActionRequestMembershipRequestsActionApproveParticipantSpec {
        type Response =
            MakeMembershipRequestsActionRequestMembershipRequestsActionApproveParticipantResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let membership_requests_action_node =
                NodeBuilder::new("membership_requests_action").build();

            InfoQuery::set(
                W_G2_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![membership_requests_action_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            let addressing_mode = response
                .get_attr("addressing_mode")
                .map(|v| v.as_str().to_string());
            let membership_requests_action_wrap = response
                .get_optional_child("membership_requests_action")
                .ok_or_else(|| anyhow::anyhow!("missing <membership_requests_action>"))?;
            let approve = membership_requests_action_wrap
                .get_optional_child("approve")
                .ok_or_else(|| anyhow::anyhow!("missing <approve>"))?;
            let mut participant_items = Vec::new();
            for child in approve.get_children_by_tag("participant") {
                let jid = child
                    .get_attr("jid")
                    .and_then(|v| v.to_jid())
                    .ok_or_else(|| anyhow::anyhow!("missing jid"))?;
                participant_items.push(MakeMembershipRequestsActionRequestMembershipRequestsActionApproveParticipantParticipantItem {
                    jid,
                    ..Default::default()
                });
            }
            let reject = membership_requests_action_wrap
                .get_optional_child("reject")
                .ok_or_else(|| anyhow::anyhow!("missing <reject>"))?;
            let mut participant_items = Vec::new();
            for child in reject.get_children_by_tag("participant") {
                let jid = child
                    .get_attr("jid")
                    .and_then(|v| v.to_jid())
                    .ok_or_else(|| anyhow::anyhow!("missing jid"))?;
                participant_items.push(MakeMembershipRequestsActionRequestMembershipRequestsActionApproveParticipantParticipantItem {
                    jid,
                    ..Default::default()
                });
            }
            Ok(MakeMembershipRequestsActionRequestMembershipRequestsActionApproveParticipantResponse {
                r#type,
                addressing_mode,
                participant: participant_items,
                ..Default::default()
            })
        }
    }

    /// makePromoteDemoteAdminRequestAdminPromoteParticipant:set IQ spec.
    ///
    /// Source: `WASmaxOutGroupsPromoteDemoteAdminRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakePromoteDemoteAdminRequestAdminPromoteParticipantSpec;

    impl IqSpec for MakePromoteDemoteAdminRequestAdminPromoteParticipantSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            let admin_node = NodeBuilder::new("admin").build();

            InfoQuery::set(
                W_G2_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![admin_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// makePromoteDemoteRequestPromoteParticipant:set IQ spec.
    ///
    /// Source: `WASmaxOutGroupsPromoteDemoteRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakePromoteDemoteRequestPromoteParticipantSpec;

    impl IqSpec for MakePromoteDemoteRequestPromoteParticipantSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::set(W_G2_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// makeRemoveParticipantsRequestRemoveParticipant:set IQ spec.
    ///
    /// Source: `WASmaxOutGroupsRemoveParticipantsRequest`
    #[derive(Debug, Clone)]
    pub struct MakeRemoveParticipantsRequestRemoveParticipantSpec {
        pub linked_groups: Option<String>,
    }

    impl MakeRemoveParticipantsRequestRemoveParticipantSpec {
        pub fn new(linked_groups: Option<String>) -> Self {
            Self { linked_groups }
        }
    }

    impl IqSpec for MakeRemoveParticipantsRequestRemoveParticipantSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut remove_node = NodeBuilder::new("remove");
            if let Some(v) = &self.linked_groups {
                remove_node = remove_node.attr("linked_groups", v.as_str());
            }
            let remove_node = remove_node.build();

            InfoQuery::set(
                W_G2_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![remove_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// makeReportMessagesRequest:set IQ spec.
    ///
    /// Source: `WASmaxOutGroupsReportMessagesRequest`
    #[derive(Debug, Clone)]
    pub struct MakeReportMessagesRequestSpec {
        pub message_id: String,
    }

    impl MakeReportMessagesRequestSpec {
        pub fn new(message_id: impl Into<String>) -> Self {
            Self {
                message_id: message_id.into(),
            }
        }
    }

    /// Response from makeReportMessagesRequest:set.
    #[derive(Debug, Clone, Default)]
    pub struct MakeReportMessagesRequestResponse {
        pub r#type: String,
    }

    impl IqSpec for MakeReportMessagesRequestSpec {
        type Response = MakeReportMessagesRequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut report_node = NodeBuilder::new("report");
            report_node = report_node.attr("message_id", &*self.message_id);
            let report_node = report_node.build();
            let mut reports_node = NodeBuilder::new("reports");
            reports_node = reports_node.children([report_node]);
            let reports_node = reports_node.build();

            InfoQuery::set(
                W_G2_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![reports_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            Ok(MakeReportMessagesRequestResponse {
                r#type,
                ..Default::default()
            })
        }
    }

    /// makeRevokeRequestCodeRequestRevokeParticipant:set IQ spec.
    ///
    /// Source: `WASmaxOutGroupsRevokeRequestCodeRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeRevokeRequestCodeRequestRevokeParticipantSpec;

    impl IqSpec for MakeRevokeRequestCodeRequestRevokeParticipantSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            let revoke_node = NodeBuilder::new("revoke").build();

            InfoQuery::set(
                W_G2_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![revoke_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// makeSetDescriptionRequestDescriptionBody:set IQ spec.
    ///
    /// Source: `WASmaxOutGroupsSetDescriptionRequest`
    #[derive(Debug, Clone)]
    pub struct MakeSetDescriptionRequestDescriptionBodySpec {
        pub id: Option<String>,
        pub prev: Option<String>,
        pub delete: Option<String>,
    }

    impl MakeSetDescriptionRequestDescriptionBodySpec {
        pub fn new(id: Option<String>, prev: Option<String>, delete: Option<String>) -> Self {
            Self { id, prev, delete }
        }
    }

    /// Response from makeSetDescriptionRequestDescriptionBody:set.
    #[derive(Debug, Clone, Default)]
    pub struct MakeSetDescriptionRequestDescriptionBodyResponse {
        pub t: Option<u64>,
        pub r#type: String,
    }

    impl IqSpec for MakeSetDescriptionRequestDescriptionBodySpec {
        type Response = MakeSetDescriptionRequestDescriptionBodyResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut description_node = NodeBuilder::new("description");
            if let Some(v) = &self.id {
                description_node = description_node.attr("id", v.as_str());
            }
            if let Some(v) = &self.prev {
                description_node = description_node.attr("prev", v.as_str());
            }
            if let Some(v) = &self.delete {
                description_node = description_node.attr("delete", v.as_str());
            }
            let description_node = description_node.build();

            InfoQuery::set(
                W_G2_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![description_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let t = response.get_attr("t").and_then(|v| v.as_str().parse().ok());
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            Ok(MakeSetDescriptionRequestDescriptionBodyResponse {
                t,
                r#type,
                ..Default::default()
            })
        }
    }

    /// makeSetPropertyRequestLocked:set IQ spec.
    ///
    /// Source: `WASmaxOutGroupsSetPropertyRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeSetPropertyRequestLockedSpec;

    /// Response from makeSetPropertyRequestLocked:set.
    #[derive(Debug, Clone, Default)]
    pub struct MakeSetPropertyRequestLockedResponse {
        pub r#type: String,
        pub expiration: u64,
        pub trigger: Option<u64>,
    }

    impl IqSpec for MakeSetPropertyRequestLockedSpec {
        type Response = MakeSetPropertyRequestLockedResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::set(W_G2_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            let ephemeral = response
                .get_optional_child("ephemeral")
                .ok_or_else(|| anyhow::anyhow!("missing <ephemeral>"))?;
            let expiration: u64 = ephemeral
                .get_attr("expiration")
                .ok_or_else(|| anyhow::anyhow!("missing expiration"))?
                .as_str()
                .parse()?;
            let trigger = ephemeral
                .get_attr("trigger")
                .and_then(|v| v.as_str().parse().ok());
            Ok(MakeSetPropertyRequestLockedResponse {
                r#type,
                expiration,
                trigger,
                ..Default::default()
            })
        }
    }

    /// makeSetSubjectRequest:set IQ spec.
    ///
    /// Source: `WASmaxOutGroupsSetSubjectRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeSetSubjectRequestSpec;

    /// Response from makeSetSubjectRequest:set.
    #[derive(Debug, Clone, Default)]
    pub struct MakeSetSubjectRequestResponse {
        pub r#type: String,
    }

    impl IqSpec for MakeSetSubjectRequestSpec {
        type Response = MakeSetSubjectRequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::set(W_G2_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            Ok(MakeSetSubjectRequestResponse {
                r#type,
                ..Default::default()
            })
        }
    }

    /// makeSubGroupSuggestionsActionRequestSubGroupSuggestionsActionApproveSubGroupSuggestion:set IQ spec.
    ///
    /// Source: `WASmaxOutGroupsSubGroupSuggestionsActionRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeSubGroupSuggestionsActionRequestSubGroupSuggestionsActionApproveSubGroupSuggestionSpec;

    impl IqSpec for MakeSubGroupSuggestionsActionRequestSubGroupSuggestionsActionApproveSubGroupSuggestionSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            let sub_group_suggestions_action_node = NodeBuilder::new("sub_group_suggestions_action").build();

            InfoQuery::set(
                W_G2_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![sub_group_suggestions_action_node])),
            )
        }

        fn parse_response(&self, _response: &wacore_binary::NodeRef<'_>) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// makeUnlinkGroupsRequestUnlinkGroup:set IQ spec.
    ///
    /// Source: `WASmaxOutGroupsUnlinkGroupsRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeUnlinkGroupsRequestUnlinkGroupSpec;

    impl IqSpec for MakeUnlinkGroupsRequestUnlinkGroupSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut unlink_node = NodeBuilder::new("unlink");
            unlink_node = unlink_node.attr("unlink_type", "sub_group");
            let unlink_node = unlink_node.build();

            InfoQuery::set(
                W_G2_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![unlink_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// mergeBaseSetGroupMixin:set IQ spec.
    ///
    /// Source: `WASmaxOutGroupsBaseSetGroupMixin`
    #[derive(Debug, Clone, Default)]
    pub struct MergeBaseSetGroupMixinSpec;

    impl IqSpec for MergeBaseSetGroupMixinSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::set(W_G2_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// mergeBaseSetServerMixin:set IQ spec.
    ///
    /// Source: `WASmaxOutGroupsBaseSetServerMixin`
    #[derive(Debug, Clone, Default)]
    pub struct MergeBaseSetServerMixinSpec;

    impl IqSpec for MergeBaseSetServerMixinSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::set(W_G2_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// resetGroupInviteCode:set IQ spec.
    ///
    /// Source: `WAWebGroupInviteJob`
    #[derive(Debug, Clone, Default)]
    pub struct ResetGroupInviteCodeSpec;

    /// Response from resetGroupInviteCode:set.
    #[derive(Debug, Clone, Default)]
    pub struct ResetGroupInviteCodeResponse {
        pub code: String,
    }

    impl IqSpec for ResetGroupInviteCodeSpec {
        type Response = ResetGroupInviteCodeResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let invite_node = NodeBuilder::new("invite").build();

            InfoQuery::set(
                W_G2_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![invite_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let invite = response
                .get_optional_child("invite")
                .ok_or_else(|| anyhow::anyhow!("missing <invite>"))?;
            let code = invite
                .get_attr("code")
                .ok_or_else(|| anyhow::anyhow!("missing code"))?
                .as_str()
                .to_string();
            Ok(ResetGroupInviteCodeResponse {
                code,
                ..Default::default()
            })
        }
    }

    /// resetGroupInviteCode:set IQ spec.
    ///
    /// Source: `WAWebGroupInviteJob`
    #[derive(Debug, Clone)]
    pub struct ResetGroupInviteCode2Spec {
        pub code: String,
    }

    impl ResetGroupInviteCode2Spec {
        pub fn new(code: impl Into<String>) -> Self {
            Self { code: code.into() }
        }
    }

    /// Response from resetGroupInviteCode:set.
    #[derive(Debug, Clone, Default)]
    pub struct ResetGroupInviteCode2Response {
        pub code: String,
    }

    impl IqSpec for ResetGroupInviteCode2Spec {
        type Response = ResetGroupInviteCode2Response;

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut invite_node = NodeBuilder::new("invite");
            invite_node = invite_node.attr("code", &*self.code);
            let invite_node = invite_node.build();

            InfoQuery::set(
                W_G2_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![invite_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let invite = response
                .get_optional_child("invite")
                .ok_or_else(|| anyhow::anyhow!("missing <invite>"))?;
            let code = invite
                .get_attr("code")
                .ok_or_else(|| anyhow::anyhow!("missing code"))?
                .as_str()
                .to_string();
            Ok(ResetGroupInviteCode2Response {
                code,
                ..Default::default()
            })
        }
    }
}

/// IQ namespace `w:m`. Source: WAWebQueryMediaConnsJob.
pub mod w_m {
    use crate::iq::spec::IqSpec;
    use crate::request::InfoQuery;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::jid::{Jid, Server};
    use wacore_binary::node::NodeContent;

    /// IQ namespace.
    pub const W_M_NAMESPACE: &str = "w:m";

    // ─── IQ Specs ───────────────────────────────────────────────────────────────

    /// mapParsedMediaConn:set IQ spec.
    ///
    /// Source: `WAWebQueryMediaConnsJob`
    #[derive(Debug, Clone, Default)]
    pub struct MapParsedMediaConnSpec;

    impl IqSpec for MapParsedMediaConnSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            let media_conn_node = NodeBuilder::new("media_conn").build();

            InfoQuery::set(
                W_M_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![media_conn_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }
}

/// IQ namespace `w:mex`. Source: WAWebMexRelayEnvironment.
pub mod w_mex {
    use crate::iq::spec::IqSpec;
    use crate::request::InfoQuery;
    use wacore_binary::jid::{Jid, Server};

    /// IQ namespace.
    pub const W_MEX_NAMESPACE: &str = "w:mex";

    // ─── IQ Specs ───────────────────────────────────────────────────────────────

    /// name:get IQ spec.
    ///
    /// Source: `WAWebMexRelayEnvironment`
    #[derive(Debug, Clone, Default)]
    pub struct NameSpec;

    /// Response from name:get.
    #[derive(Debug, Clone, Default)]
    pub struct NameResponse {
        pub result: Vec<u8>,
    }

    impl IqSpec for NameSpec {
        type Response = NameResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::get(W_MEX_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let result_node = response
                .get_optional_child("result")
                .ok_or_else(|| anyhow::anyhow!("missing <result>"))?;
            let result = result_node
                .content_bytes()
                .map(|b| b.to_vec())
                .unwrap_or_default();
            Ok(NameResponse {
                result,
                ..Default::default()
            })
        }
    }
}

/// IQ namespace `w:p`. Source: WASmaxOutPingsClientRequest.
pub mod w_p {
    use crate::iq::spec::IqSpec;
    use crate::request::InfoQuery;
    use wacore_binary::jid::{Jid, Server};

    /// IQ namespace.
    pub const W_P_NAMESPACE: &str = "w:p";

    // ─── IQ Specs ───────────────────────────────────────────────────────────────

    /// makeClientRequest:get IQ spec.
    ///
    /// Source: `WASmaxOutPingsClientRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeClientRequestSpec;

    impl IqSpec for MakeClientRequestSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::get(W_P_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }
}

/// IQ namespace `w:pay`. Source: WASmaxOutBrPaymentCreateCustomPaymentMethodRequest, WASmaxOutBrPaymentSetIQMixin.
pub mod w_pay {
    use crate::iq::spec::IqSpec;
    use crate::request::InfoQuery;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::jid::{Jid, Server};
    use wacore_binary::node::NodeContent;

    /// IQ namespace.
    pub const W_PAY_NAMESPACE: &str = "w:pay";

    // ─── Shared child types ──────────────────────────────────────────────────────

    /// Child item struct.
    #[derive(Debug, Clone, Default)]
    pub struct MakeCreateCustomPaymentMethodRequestMetadataItem {
        pub key: Option<String>,
        pub value: Option<String>,
    }

    // ─── IQ Specs ───────────────────────────────────────────────────────────────

    /// makeCreateCustomPaymentMethodRequest:set IQ spec.
    ///
    /// Source: `WASmaxOutBrPaymentCreateCustomPaymentMethodRequest`
    #[derive(Debug, Clone)]
    pub struct MakeCreateCustomPaymentMethodRequestSpec {
        pub device_id: String,
    }

    impl MakeCreateCustomPaymentMethodRequestSpec {
        pub fn new(device_id: impl Into<String>) -> Self {
            Self {
                device_id: device_id.into(),
            }
        }
    }

    /// Response from makeCreateCustomPaymentMethodRequest:set.
    #[derive(Debug, Clone, Default)]
    pub struct MakeCreateCustomPaymentMethodRequestResponse {
        pub r#type: String,
        pub country: Option<String>,
        pub created: Option<String>,
        pub flow: Option<String>,
        pub credential_id: String,
        pub p2p_eligible: Option<String>,
        pub p2m_eligible: Option<String>,
        pub metadata: Vec<MakeCreateCustomPaymentMethodRequestMetadataItem>,
    }

    impl IqSpec for MakeCreateCustomPaymentMethodRequestSpec {
        type Response = MakeCreateCustomPaymentMethodRequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut account_node = NodeBuilder::new("account");
            account_node = account_node.attr("action", "create-custom-payment-method");
            account_node = account_node.attr("device_id", &*self.device_id);
            account_node = account_node.attr("country", "BR");
            let account_node = account_node.build();

            InfoQuery::set(
                W_PAY_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![account_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            let account_wrap = response
                .get_optional_child("account")
                .ok_or_else(|| anyhow::anyhow!("missing <account>"))?;
            let custom_payment_method = account_wrap
                .get_optional_child("custom_payment_method")
                .ok_or_else(|| anyhow::anyhow!("missing <custom_payment_method>"))?;
            let r#type = custom_payment_method
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            let country = custom_payment_method
                .get_attr("country")
                .map(|v| v.as_str().to_string());
            let created = custom_payment_method
                .get_attr("created")
                .map(|v| v.as_str().to_string());
            let flow = custom_payment_method
                .get_attr("flow")
                .map(|v| v.as_str().to_string());
            let credential_id = custom_payment_method
                .get_attr("credential-id")
                .ok_or_else(|| anyhow::anyhow!("missing credential-id"))?
                .as_str()
                .to_string();
            let p2p_eligible = custom_payment_method
                .get_attr("p2p-eligible")
                .map(|v| v.as_str().to_string());
            let p2m_eligible = custom_payment_method
                .get_attr("p2m-eligible")
                .map(|v| v.as_str().to_string());
            let mut metadata_items = Vec::new();
            for child in custom_payment_method.get_children_by_tag("metadata") {
                let key = child.get_attr("key").map(|v| v.as_str().to_string());
                let value = child.get_attr("value").map(|v| v.as_str().to_string());
                metadata_items.push(MakeCreateCustomPaymentMethodRequestMetadataItem {
                    key,
                    value,
                    ..Default::default()
                });
            }
            Ok(MakeCreateCustomPaymentMethodRequestResponse {
                r#type,
                country,
                created,
                flow,
                credential_id,
                p2p_eligible,
                p2m_eligible,
                metadata: metadata_items,
                ..Default::default()
            })
        }
    }

    /// mergeSetIQMixin:set IQ spec.
    ///
    /// Source: `WASmaxOutBrPaymentSetIQMixin`
    #[derive(Debug, Clone, Default)]
    pub struct MergeSetIQMixinSpec;

    impl IqSpec for MergeSetIQMixinSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::set(W_PAY_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }
}

/// IQ namespace `w:profile:picture`. Source: WASmaxOutProfilePictureGetRequest, WASmaxOutProfilePictureBaseGetIQMixin, WAWebQueryGroupInviteProfilePicApi, WAWebSendProfilePictureJob.
pub mod w_profile_picture {
    use crate::iq::spec::IqSpec;
    use crate::request::InfoQuery;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::jid::{Jid, Server};
    use wacore_binary::node::NodeContent;

    /// IQ namespace.
    pub const W_PROFILE_PICTURE_NAMESPACE: &str = "w:profile:picture";

    // ─── IQ Specs ───────────────────────────────────────────────────────────────

    /// makeGetRequest:get IQ spec.
    ///
    /// Source: `WASmaxOutProfilePictureGetRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetRequestSpec;

    /// Response from makeGetRequest:get.
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetRequestResponse {
        pub picture_id: String,
        pub picture_type: String,
        pub picture_url: String,
        pub picture_direct_path: String,
        pub picture_hash: Option<String>,
        pub picture_has_staging: Option<String>,
        pub r#type: String,
    }

    impl IqSpec for MakeGetRequestSpec {
        type Response = MakeGetRequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::get(W_PROFILE_PICTURE_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let picture_wrap = response
                .get_optional_child("picture")
                .ok_or_else(|| anyhow::anyhow!("missing <picture>"))?;
            let picture_id = picture_wrap
                .get_attr("id")
                .ok_or_else(|| anyhow::anyhow!("missing id"))?
                .as_str()
                .to_string();
            let picture_type = picture_wrap
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            let picture_url = picture_wrap
                .get_attr("url")
                .ok_or_else(|| anyhow::anyhow!("missing url"))?
                .as_str()
                .to_string();
            let picture_direct_path = picture_wrap
                .get_attr("direct_path")
                .ok_or_else(|| anyhow::anyhow!("missing direct_path"))?
                .as_str()
                .to_string();
            let picture_hash = picture_wrap
                .get_attr("hash")
                .map(|v| v.as_str().to_string());
            let picture_has_staging = picture_wrap
                .get_attr("has_staging")
                .map(|v| v.as_str().to_string());
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            Ok(MakeGetRequestResponse {
                picture_id,
                picture_type,
                picture_url,
                picture_direct_path,
                picture_hash,
                picture_has_staging,
                r#type,
                ..Default::default()
            })
        }
    }

    /// mergeBaseGetIQMixin:get IQ spec.
    ///
    /// Source: `WASmaxOutProfilePictureBaseGetIQMixin`
    #[derive(Debug, Clone, Default)]
    pub struct MergeBaseGetIQMixinSpec;

    impl IqSpec for MergeBaseGetIQMixinSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::get(W_PROFILE_PICTURE_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// queryGroupInviteLinkProfilePic:get IQ spec.
    ///
    /// Source: `WAWebQueryGroupInviteProfilePicApi`
    #[derive(Debug, Clone)]
    pub struct QueryGroupInviteLinkProfilePicSpec {
        pub id: Option<String>,
        pub r#type: String,
        pub query: String,
        pub code: String,
        pub expiration: String,
        pub admin: Jid,
    }

    impl QueryGroupInviteLinkProfilePicSpec {
        pub fn new(
            id: Option<String>,
            r#type: impl Into<String>,
            query: impl Into<String>,
            code: impl Into<String>,
            expiration: impl Into<String>,
            admin: &Jid,
        ) -> Self {
            Self {
                id,
                r#type: r#type.into(),
                query: query.into(),
                code: code.into(),
                expiration: expiration.into(),
                admin: admin.clone(),
            }
        }
    }

    /// Response from queryGroupInviteLinkProfilePic:get.
    #[derive(Debug, Clone, Default)]
    pub struct QueryGroupInviteLinkProfilePicResponse {
        pub id: String,
        pub r#type: String,
        pub url: String,
        pub direct_path: String,
    }

    impl IqSpec for QueryGroupInviteLinkProfilePicSpec {
        type Response = QueryGroupInviteLinkProfilePicResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut add_request_node = NodeBuilder::new("add_request");
            add_request_node = add_request_node.attr("code", &*self.code);
            add_request_node = add_request_node.attr("expiration", &*self.expiration);
            add_request_node = add_request_node.attr("admin", self.admin.clone());
            let add_request_node = add_request_node.build();
            let mut picture_node = NodeBuilder::new("picture");
            if let Some(v) = &self.id {
                picture_node = picture_node.attr("id", v.as_str());
            }
            picture_node = picture_node.attr("type", &*self.r#type);
            picture_node = picture_node.attr("query", &*self.query);
            picture_node = picture_node.children([add_request_node]);
            let picture_node = picture_node.build();

            InfoQuery::get(
                W_PROFILE_PICTURE_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![picture_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let picture = response
                .get_optional_child("picture")
                .ok_or_else(|| anyhow::anyhow!("missing <picture>"))?;
            let id = picture
                .get_attr("id")
                .ok_or_else(|| anyhow::anyhow!("missing id"))?
                .as_str()
                .to_string();
            let r#type = picture
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            let url = picture
                .get_attr("url")
                .ok_or_else(|| anyhow::anyhow!("missing url"))?
                .as_str()
                .to_string();
            let direct_path = picture
                .get_attr("direct_path")
                .ok_or_else(|| anyhow::anyhow!("missing direct_path"))?
                .as_str()
                .to_string();
            Ok(QueryGroupInviteLinkProfilePicResponse {
                id,
                r#type,
                url,
                direct_path,
                ..Default::default()
            })
        }
    }

    /// default:set IQ spec.
    ///
    /// Source: `WAWebSendProfilePictureJob`
    #[derive(Debug, Clone, Default)]
    pub struct SendProfilePictureJobSpec;

    /// Response from default:set.
    #[derive(Debug, Clone, Default)]
    pub struct SendProfilePictureJobResponse {
        pub id: u64,
    }

    impl IqSpec for SendProfilePictureJobSpec {
        type Response = SendProfilePictureJobResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::set(W_PROFILE_PICTURE_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let picture = response
                .get_optional_child("picture")
                .ok_or_else(|| anyhow::anyhow!("missing <picture>"))?;
            let id: u64 = picture
                .get_attr("id")
                .ok_or_else(|| anyhow::anyhow!("missing id"))?
                .as_str()
                .parse()?;
            Ok(SendProfilePictureJobResponse {
                id,
                ..Default::default()
            })
        }
    }
}

/// IQ namespace `w:stats`. Source: WASmaxOutStatsSendBufferRequest.
pub mod w_stats {
    use crate::iq::spec::IqSpec;
    use crate::request::InfoQuery;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::jid::{Jid, Server};
    use wacore_binary::node::NodeContent;

    /// IQ namespace.
    pub const W_STATS_NAMESPACE: &str = "w:stats";

    // ─── IQ Specs ───────────────────────────────────────────────────────────────

    /// makeSendBufferRequest:set IQ spec.
    ///
    /// Source: `WASmaxOutStatsSendBufferRequest`
    #[derive(Debug, Clone)]
    pub struct MakeSendBufferRequestSpec {
        pub t: u64,
    }

    impl MakeSendBufferRequestSpec {
        pub fn new(t: u64) -> Self {
            Self { t }
        }
    }

    /// Response from makeSendBufferRequest:set.
    #[derive(Debug, Clone, Default)]
    pub struct MakeSendBufferRequestResponse {
        pub r#type: String,
    }

    impl IqSpec for MakeSendBufferRequestSpec {
        type Response = MakeSendBufferRequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let mut add_node = NodeBuilder::new("add");
            add_node = add_node.attr("t", self.t.to_string());
            let add_node = add_node.build();

            InfoQuery::set(
                W_STATS_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![add_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            Ok(MakeSendBufferRequestResponse {
                r#type,
                ..Default::default()
            })
        }
    }
}

/// IQ namespace `w:sync:app:state`. Source: WAWebKmpSyncdRequestBuilder, WAWebSyncdRequestBuilderBuild.
pub mod w_sync_app_state {
    use crate::iq::spec::IqSpec;
    use crate::request::InfoQuery;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::jid::{Jid, Server};
    use wacore_binary::node::NodeContent;

    /// IQ namespace.
    pub const W_SYNC_APP_STATE_NAMESPACE: &str = "w:sync:app:state";

    // ─── IQ Specs ───────────────────────────────────────────────────────────────

    /// buildOutgoingRequestWithKmp:set IQ spec.
    ///
    /// Source: `WAWebKmpSyncdRequestBuilder`
    #[derive(Debug, Clone, Default)]
    pub struct BuildOutgoingRequestWithKmpSpec;

    impl IqSpec for BuildOutgoingRequestWithKmpSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            let sync_node = NodeBuilder::new("sync").build();

            InfoQuery::set(
                W_SYNC_APP_STATE_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![sync_node])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// patchMac:set IQ spec.
    ///
    /// Source: `WAWebSyncdRequestBuilderBuild`
    #[derive(Debug, Clone, Default)]
    pub struct PatchMacSpec;

    impl IqSpec for PatchMacSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            InfoQuery::set(W_SYNC_APP_STATE_NAMESPACE, Jid::new("", Server::Pn), None)
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }
}

/// IQ namespace `waffle`. Source: WASmaxOutWaffleForceDeleteStateRequest, WASmaxOutWaffleForceSuspendStateRequest, WASmaxOutWaffleGetCertificateRequest, WASmaxOutWaffleStateExistsRequest.
pub mod waffle {
    use crate::iq::spec::IqSpec;
    use crate::request::InfoQuery;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::jid::{Jid, Server};
    use wacore_binary::node::NodeContent;

    /// IQ namespace.
    pub const WAFFLE_NAMESPACE: &str = "waffle";

    // ─── IQ Specs ───────────────────────────────────────────────────────────────

    /// makeForceDeleteStateRequestOnlyIfSuspended:get IQ spec.
    ///
    /// Source: `WASmaxOutWaffleForceDeleteStateRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeForceDeleteStateRequestOnlyIfSuspendedSpec;

    /// Response from makeForceDeleteStateRequestOnlyIfSuspended:get.
    #[derive(Debug, Clone, Default)]
    pub struct MakeForceDeleteStateRequestOnlyIfSuspendedResponse {
        pub r#type: String,
    }

    impl IqSpec for MakeForceDeleteStateRequestOnlyIfSuspendedSpec {
        type Response = MakeForceDeleteStateRequestOnlyIfSuspendedResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let timestamp_node = NodeBuilder::new("timestamp").build();

            InfoQuery::get(
                WAFFLE_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![timestamp_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            Ok(MakeForceDeleteStateRequestOnlyIfSuspendedResponse {
                r#type,
                ..Default::default()
            })
        }
    }

    /// makeForceSuspendStateRequest:get IQ spec.
    ///
    /// Source: `WASmaxOutWaffleForceSuspendStateRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeForceSuspendStateRequestSpec;

    /// Response from makeForceSuspendStateRequest:get.
    #[derive(Debug, Clone, Default)]
    pub struct MakeForceSuspendStateRequestResponse {
        pub npr_element_value: String,
        pub r#type: String,
    }

    impl IqSpec for MakeForceSuspendStateRequestSpec {
        type Response = MakeForceSuspendStateRequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let timestamp_node = NodeBuilder::new("timestamp").build();

            InfoQuery::get(
                WAFFLE_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![timestamp_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let npr_wrap = response
                .get_optional_child("npr")
                .ok_or_else(|| anyhow::anyhow!("missing <npr>"))?;
            let npr_element_value = npr_wrap
                .get_attr("nprElementValue")
                .ok_or_else(|| anyhow::anyhow!("missing nprElementValue"))?
                .as_str()
                .to_string();
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            Ok(MakeForceSuspendStateRequestResponse {
                npr_element_value,
                r#type,
                ..Default::default()
            })
        }
    }

    /// makeGetCertificateRequestPayloadEncCertificates:get IQ spec.
    ///
    /// Source: `WASmaxOutWaffleGetCertificateRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeGetCertificateRequestPayloadEncCertificatesSpec;

    impl IqSpec for MakeGetCertificateRequestPayloadEncCertificatesSpec {
        type Response = ();

        fn build_iq(&self) -> InfoQuery<'static> {
            let timestamp_node = NodeBuilder::new("timestamp").build();
            let payload_enc_certificates_node =
                NodeBuilder::new("payload_enc_certificates").build();

            InfoQuery::get(
                WAFFLE_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![
                    timestamp_node,
                    payload_enc_certificates_node,
                ])),
            )
        }

        fn parse_response(
            &self,
            _response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            Ok(())
        }
    }

    /// makeStateExistsRequest:get IQ spec.
    ///
    /// Source: `WASmaxOutWaffleStateExistsRequest`
    #[derive(Debug, Clone, Default)]
    pub struct MakeStateExistsRequestSpec;

    /// Response from makeStateExistsRequest:get.
    #[derive(Debug, Clone, Default)]
    pub struct MakeStateExistsRequestResponse {
        pub wf_state_element_value: String,
        pub r#type: String,
        pub npr: Option<String>,
    }

    impl IqSpec for MakeStateExistsRequestSpec {
        type Response = MakeStateExistsRequestResponse;

        fn build_iq(&self) -> InfoQuery<'static> {
            let timestamp_node = NodeBuilder::new("timestamp").build();

            InfoQuery::get(
                WAFFLE_NAMESPACE,
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![timestamp_node])),
            )
        }

        #[allow(clippy::needless_update, unused_variables)]
        fn parse_response(
            &self,
            response: &wacore_binary::NodeRef<'_>,
        ) -> Result<Self::Response, anyhow::Error> {
            let wf_state_wrap = response
                .get_optional_child("wf_state")
                .ok_or_else(|| anyhow::anyhow!("missing <wf_state>"))?;
            let wf_state_element_value = wf_state_wrap
                .content_str()
                .and_then(|s| s.parse().ok())
                .unwrap_or_default();
            let r#type = response
                .get_attr("type")
                .ok_or_else(|| anyhow::anyhow!("missing type"))?
                .as_str()
                .to_string();
            let suspended_state = response
                .get_optional_child("suspended_state")
                .ok_or_else(|| anyhow::anyhow!("missing <suspended_state>"))?;
            let npr = suspended_state
                .get_attr("npr")
                .map(|v| v.as_str().to_string());
            Ok(MakeStateExistsRequestResponse {
                wf_state_element_value,
                r#type,
                npr,
                ..Default::default()
            })
        }
    }
}

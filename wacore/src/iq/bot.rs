//! Bot directory IQ types and specifications (`xmlns="bot"`).
//!
//! This is the catalogue of first-party AI bots the server offers an account:
//! a set of display sections, each holding `(jid, persona_id)` pairs. It is not
//! related to the bot *framework* in the `whatsapp-rust` crate, which builds a
//! client that answers messages.
//!
//! The persona ids returned here are the input to the `WAWebFetchBotProfilesGQLQuery`
//! MEX operation, which hydrates them into displayable profiles (name, description,
//! creator, icebreakers). This IQ says *which* bots exist; that operation says what
//! they are.

use crate::WireEnum;
use crate::iq::node::{optional_attr, required_child};
use crate::iq::spec::IqSpec;
use crate::protocol::ProtocolNode;
use crate::request::InfoQuery;
use anyhow::{Result, anyhow};
use wacore_binary::builder::NodeBuilder;
use wacore_binary::{Jid, Node, NodeContent, NodeContentRef, NodeRef, Server};

/// IQ namespace for bot directory operations.
pub const BOT_IQ_NAMESPACE: &str = "bot";

/// Protocol version WhatsApp Web asks for. The official client pins `"2"` on
/// every bot-list request even though its parser also accepts a `v="3"`
/// response, so the request side has exactly one value and the response side
/// has to handle both.
pub const BOT_LIST_REQUEST_VERSION: &str = "2";

/// Shape version of a bot-list response, carried in `<bot v="...">`.
///
/// The two versions differ only in which fields are populated, so one parser
/// reads both; this records which shape actually arrived.
#[derive(Debug, Clone, PartialEq, Eq, WireEnum)]
pub enum BotListVersion {
    /// `<default>` is mandatory, bots may carry `<theme>` children, sections
    /// have no `display_type`.
    #[wire = "2"]
    V2,
    /// `<bot>` carries `bhash`, `<default>` is optional, sections carry
    /// `display_type`, bots carry `card_title` and no themes.
    #[wire = "3"]
    V3,
    #[wire_fallback]
    Other(String),
}

/// What a section groups. WhatsApp Web reads bots out of *every* section
/// regardless of type — the type only drives how the section is presented.
#[derive(Debug, Clone, PartialEq, Eq, WireEnum)]
pub enum BotSectionType {
    #[wire = "all"]
    All,
    #[wire = "category"]
    Category,
    #[wire = "featured"]
    Featured,
    #[wire_fallback]
    Other(String),
}

/// How a section should be laid out. Present only in `v="3"` responses.
#[derive(Debug, Clone, PartialEq, Eq, WireEnum)]
pub enum BotSectionDisplayType {
    #[wire = "hidden"]
    Hidden,
    #[wire = "hscroll"]
    Hscroll,
    #[wire = "hscroll_icebreakers"]
    HscrollIcebreakers,
    #[wire = "hscroll_large"]
    HscrollLarge,
    #[wire = "hscroll_small"]
    HscrollSmall,
    #[wire = "listview"]
    Listview,
    #[wire_fallback]
    Other(String),
}

/// Colour scheme a `<theme>` block applies to.
#[derive(Debug, Clone, PartialEq, Eq, WireEnum)]
pub enum BotThemeMode {
    #[wire = "dark"]
    Dark,
    #[wire = "light"]
    Light,
    #[wire_fallback]
    Other(String),
}

/// Per-mode colours for a bot's card.
///
/// Wire format:
/// `<theme mode="light"><background>…</background><primary_text>…</primary_text>
///  <secondary_text>…</secondary_text></theme>`
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BotTheme {
    pub mode: BotThemeMode,
    pub background: Option<String>,
    pub primary_text: Option<String>,
    pub secondary_text: Option<String>,
}

fn node_text(node: &NodeRef<'_>) -> Option<String> {
    match node.content.as_ref() {
        Some(NodeContentRef::String(s)) => Some(s.to_string()),
        Some(NodeContentRef::Bytes(b)) => std::str::from_utf8(b).ok().map(|s| s.to_string()),
        _ => None,
    }
}

fn text_child(node: &NodeRef<'_>, tag: &str) -> Option<String> {
    node.get_optional_child(tag).and_then(node_text)
}

fn text_node(tag: &'static str, value: String) -> Node {
    NodeBuilder::new(tag).string_content(value).build()
}

impl ProtocolNode for BotTheme {
    fn tag(&self) -> &'static str {
        "theme"
    }

    fn into_node(self) -> Node {
        let mut children = Vec::with_capacity(3);
        if let Some(background) = self.background {
            children.push(text_node("background", background));
        }
        if let Some(primary_text) = self.primary_text {
            children.push(text_node("primary_text", primary_text));
        }
        if let Some(secondary_text) = self.secondary_text {
            children.push(text_node("secondary_text", secondary_text));
        }
        NodeBuilder::new("theme")
            .attr("mode", self.mode.as_str())
            .children(children)
            .build()
    }

    fn try_from_node_ref(node: &NodeRef<'_>) -> Result<Self> {
        let mode = optional_attr(node, "mode")
            .ok_or_else(|| anyhow!("<theme> is missing the mode attribute"))?;
        Ok(Self {
            mode: BotThemeMode::from(mode.as_ref()),
            background: text_child(node, "background"),
            primary_text: text_child(node, "primary_text"),
            secondary_text: text_child(node, "secondary_text"),
        })
    }
}

/// One bot in the directory.
///
/// Wire format: `<bot jid="…" persona_id="…" card_title="…" count="…">…themes…</bot>`
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BotListEntry {
    pub jid: Jid,
    /// Identifier to hand to the bot-profile MEX operation.
    pub persona_id: String,
    /// `v="3"` only.
    pub card_title: Option<String>,
    pub count: Option<u64>,
    /// `v="2"` only; empty otherwise.
    pub themes: Vec<BotTheme>,
}

impl BotListEntry {
    pub fn new(jid: Jid, persona_id: String) -> Self {
        Self {
            jid,
            persona_id,
            card_title: None,
            count: None,
            themes: Vec::new(),
        }
    }
}

impl ProtocolNode for BotListEntry {
    fn tag(&self) -> &'static str {
        "bot"
    }

    fn into_node(self) -> Node {
        let mut builder = NodeBuilder::new("bot")
            .attr("jid", self.jid.to_string())
            .attr("persona_id", &self.persona_id);
        if let Some(card_title) = self.card_title {
            builder = builder.attr("card_title", card_title);
        }
        if let Some(count) = self.count {
            builder = builder.attr("count", count.to_string());
        }
        if !self.themes.is_empty() {
            builder = builder.children(self.themes.into_iter().map(ProtocolNode::into_node));
        }
        builder.build()
    }

    fn try_from_node_ref(node: &NodeRef<'_>) -> Result<Self> {
        let mut attrs = node.attrs();
        let jid = attrs.required_jid("jid")?;
        let persona_id = attrs
            .optional_string("persona_id")
            .ok_or_else(|| anyhow!("<bot> is missing the persona_id attribute"))?
            .to_string();
        let card_title = attrs.optional_string("card_title").map(|v| v.to_string());
        let count = attrs.optional_u64("count");
        let themes = node
            .get_children_by_tag("theme")
            .map(BotTheme::try_from_node_ref)
            .collect::<Result<Vec<_>>>()?;
        Ok(Self {
            jid,
            persona_id,
            card_title,
            count,
            themes,
        })
    }
}

/// A display group of bots.
///
/// Wire format: `<section name="…" type="all" display_type="listview">…bots…</section>`
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BotListSection {
    pub name: Option<String>,
    pub section_type: BotSectionType,
    /// `v="3"` only.
    pub display_type: Option<BotSectionDisplayType>,
    pub bots: Vec<BotListEntry>,
}

impl ProtocolNode for BotListSection {
    fn tag(&self) -> &'static str {
        "section"
    }

    fn into_node(self) -> Node {
        let mut builder = NodeBuilder::new("section").attr("type", self.section_type.as_str());
        if let Some(name) = self.name {
            builder = builder.attr("name", name);
        }
        if let Some(display_type) = self.display_type {
            builder = builder.attr("display_type", display_type.as_str());
        }
        builder
            .children(self.bots.into_iter().map(ProtocolNode::into_node))
            .build()
    }

    fn try_from_node_ref(node: &NodeRef<'_>) -> Result<Self> {
        let section_type = optional_attr(node, "type")
            .ok_or_else(|| anyhow!("<section> is missing the type attribute"))?;
        Ok(Self {
            name: optional_attr(node, "name").map(|v| v.to_string()),
            section_type: BotSectionType::from(section_type.as_ref()),
            display_type: optional_attr(node, "display_type")
                .map(|v| BotSectionDisplayType::from(v.as_ref())),
            bots: node
                .get_children_by_tag("bot")
                .map(BotListEntry::try_from_node_ref)
                .collect::<Result<Vec<_>>>()?,
        })
    }
}

/// The bot the client offers by default.
///
/// A distinct type from [`BotListEntry`] because `<default>` carries exactly
/// these two attributes on the wire in both response versions — it has no
/// `card_title`, `count` or themes to lose.
///
/// Wire format: `<default jid="…" persona_id="…"/>`
#[derive(Debug, Clone, PartialEq, Eq, crate::ProtocolNode)]
#[protocol(tag = "default")]
#[non_exhaustive]
pub struct BotDefault {
    #[attr(name = "jid", jid)]
    pub jid: Jid,
    #[attr(name = "persona_id")]
    pub persona_id: String,
}

impl BotDefault {
    pub fn new(jid: Jid, persona_id: String) -> Self {
        Self { jid, persona_id }
    }
}

/// The whole bot directory as the server returned it.
///
/// Sections are kept as sent. WhatsApp Web reads bots out of every section and
/// only uses `type` for presentation, so filtering to `type="all"` here would
/// silently drop the bots that a `category` or `featured` section is the sole
/// carrier of.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BotList {
    pub version: BotListVersion,
    /// Cache handle for the directory; `v="3"` only.
    pub bhash: Option<String>,
    /// The bot the client should offer by default. Mandatory in `v="2"`,
    /// optional in `v="3"`, and not necessarily repeated inside a section.
    pub default_bot: Option<BotDefault>,
    pub sections: Vec<BotListSection>,
}

impl BotList {
    /// Every bot in the directory, in section order, with the default bot first
    /// when the sections do not already contain it. This is the flattening
    /// WhatsApp Web applies before it fetches bot profiles.
    ///
    /// A default bot that no section carries has only the two attributes
    /// `<default>` holds, so it arrives here with no card title, count or themes.
    pub fn flatten(&self) -> Vec<BotListEntry> {
        let mut out: Vec<BotListEntry> = self
            .sections
            .iter()
            .flat_map(|section| section.bots.iter())
            .cloned()
            .collect();
        if let Some(default_bot) = self.default_bot.as_ref()
            && !out.iter().any(|bot| bot.jid == default_bot.jid)
        {
            out.insert(
                0,
                BotListEntry::new(default_bot.jid.clone(), default_bot.persona_id.clone()),
            );
        }
        out
    }

    /// The default bot's JID, if the server named one.
    pub fn default_jid(&self) -> Option<&Jid> {
        self.default_bot.as_ref().map(|bot| &bot.jid)
    }
}

impl ProtocolNode for BotList {
    fn tag(&self) -> &'static str {
        "bot"
    }

    fn into_node(self) -> Node {
        let mut builder = NodeBuilder::new("bot").attr("v", self.version.as_str());
        if let Some(bhash) = self.bhash {
            builder = builder.attr("bhash", bhash);
        }
        let mut children = Vec::with_capacity(self.sections.len() + 1);
        if let Some(default_bot) = self.default_bot {
            children.push(default_bot.into_node());
        }
        children.extend(self.sections.into_iter().map(ProtocolNode::into_node));
        builder.children(children).build()
    }

    fn try_from_node_ref(node: &NodeRef<'_>) -> Result<Self> {
        let version =
            optional_attr(node, "v").ok_or_else(|| anyhow!("<bot> is missing the v attribute"))?;
        let version = BotListVersion::from(version.as_ref());
        let default_bot = node
            .get_optional_child("default")
            .map(BotDefault::try_from_node_ref)
            .transpose()?;
        let bhash = optional_attr(node, "bhash").map(|v| v.to_string());
        // Each version has one attribute the other does not require, and the official
        // parser rejects its whole variant when that one is absent: <default> for v2,
        // bhash for v3. An unknown version is an unknown contract, so it is held to
        // neither rule.
        match version {
            BotListVersion::V2 if default_bot.is_none() => {
                return Err(anyhow!("<bot v=\"2\"> is missing its <default> child"));
            }
            BotListVersion::V3 if bhash.is_none() => {
                return Err(anyhow!("<bot v=\"3\"> is missing the bhash attribute"));
            }
            _ => {}
        }
        Ok(Self {
            version,
            bhash,
            default_bot,
            sections: node
                .get_children_by_tag("section")
                .map(BotListSection::try_from_node_ref)
                .collect::<Result<Vec<_>>>()?,
        })
    }
}

/// Fetches the bot directory.
///
/// Wire format: `<iq type="get" xmlns="bot" to="s.whatsapp.net"><bot v="2"/></iq>`
#[derive(Debug, Default, Clone, Copy)]
pub struct GetBotListSpec;

impl IqSpec for GetBotListSpec {
    type Response = BotList;

    fn build_iq(&self) -> InfoQuery<'static> {
        InfoQuery::get(
            BOT_IQ_NAMESPACE,
            Jid::new("", Server::Pn),
            Some(NodeContent::Nodes(vec![
                NodeBuilder::new("bot")
                    .attr("v", BOT_LIST_REQUEST_VERSION)
                    .build(),
            ])),
        )
    }

    fn parse_response(&self, response: &NodeRef<'_>) -> Result<Self::Response> {
        BotList::try_from_node_ref(required_child(response, "bot")?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bot_node(jid: &str, persona_id: &str) -> NodeBuilder {
        NodeBuilder::new("bot")
            .attr("jid", jid)
            .attr("persona_id", persona_id)
    }

    fn iq_with(bot: Node) -> Node {
        NodeBuilder::new("iq")
            .attr("type", "result")
            .attr("from", "s.whatsapp.net")
            .children([bot])
            .build()
    }

    fn parse(iq: &Node) -> Result<BotList> {
        GetBotListSpec.parse_response(&iq.as_node_ref())
    }

    #[test]
    fn request_asks_for_v2_on_the_bot_namespace() {
        let iq = GetBotListSpec.build_iq();
        assert_eq!(iq.namespace, "bot");
        assert_eq!(iq.query_type, crate::request::InfoQueryType::Get);
        assert_eq!(iq.to.to_string(), "s.whatsapp.net");

        let Some(NodeContent::Nodes(children)) = iq.content else {
            panic!("bot list request must carry a <bot> child");
        };
        assert_eq!(children.len(), 1);
        assert_eq!(children[0].tag, "bot");
        assert!(children[0].attrs.get("v").is_some_and(|v| v == "2"));
    }

    #[test]
    fn parses_v2_response_with_default_sections_and_themes() {
        let iq = iq_with(
            NodeBuilder::new("bot")
                .attr("v", "2")
                .children([
                    NodeBuilder::new("default")
                        .attr("jid", "10000000000000@bot")
                        .attr("persona_id", "persona-default")
                        .build(),
                    NodeBuilder::new("section")
                        .attr("name", "All")
                        .attr("type", "all")
                        .children([
                            bot_node("10000000000000@bot", "persona-default")
                                .attr("count", "7")
                                .children([NodeBuilder::new("theme")
                                    .attr("mode", "light")
                                    .children([
                                        NodeBuilder::new("background")
                                            .string_content("#FFFFFF")
                                            .build(),
                                        NodeBuilder::new("primary_text")
                                            .string_content("#111111")
                                            .build(),
                                        NodeBuilder::new("secondary_text")
                                            .string_content("#666666")
                                            .build(),
                                    ])
                                    .build()])
                                .build(),
                            bot_node("20000000000000@bot", "persona-second").build(),
                        ])
                        .build(),
                ])
                .build(),
        );

        let list = parse(&iq).expect("v2 payload parses");
        assert_eq!(list.version, BotListVersion::V2);
        assert_eq!(list.bhash, None);
        assert_eq!(
            list.default_jid().map(|jid| jid.to_string()),
            Some("10000000000000@bot".to_string())
        );

        assert_eq!(list.sections.len(), 1);
        let section = &list.sections[0];
        assert_eq!(section.name.as_deref(), Some("All"));
        assert_eq!(section.section_type, BotSectionType::All);
        assert_eq!(section.display_type, None);
        assert_eq!(section.bots.len(), 2);

        let first = &section.bots[0];
        assert_eq!(first.persona_id, "persona-default");
        assert_eq!(first.count, Some(7));
        assert_eq!(first.card_title, None);
        assert_eq!(first.themes.len(), 1);
        assert_eq!(first.themes[0].mode, BotThemeMode::Light);
        assert_eq!(first.themes[0].background.as_deref(), Some("#FFFFFF"));
        assert_eq!(first.themes[0].primary_text.as_deref(), Some("#111111"));
        assert_eq!(first.themes[0].secondary_text.as_deref(), Some("#666666"));

        // The default bot is already in a section, so flattening must not duplicate it.
        assert_eq!(list.flatten().len(), 2);
    }

    #[test]
    fn parses_v3_response_with_bhash_display_type_and_card_title() {
        let iq = iq_with(
            NodeBuilder::new("bot")
                .attr("v", "3")
                .attr("bhash", "cafebabe")
                .children([
                    NodeBuilder::new("default")
                        .attr("jid", "10000000000000@bot")
                        .attr("persona_id", "persona-default")
                        .build(),
                    NodeBuilder::new("section")
                        .attr("name", "Featured")
                        .attr("type", "featured")
                        .attr("display_type", "hscroll_large")
                        .children([bot_node("20000000000000@bot", "persona-second")
                            .attr("card_title", "Try me")
                            .build()])
                        .build(),
                ])
                .build(),
        );

        let list = parse(&iq).expect("v3 payload parses");
        assert_eq!(list.version, BotListVersion::V3);
        assert_eq!(list.bhash.as_deref(), Some("cafebabe"));
        assert_eq!(
            list.sections[0].display_type,
            Some(BotSectionDisplayType::HscrollLarge)
        );
        assert_eq!(list.sections[0].section_type, BotSectionType::Featured);
        assert_eq!(
            list.sections[0].bots[0].card_title.as_deref(),
            Some("Try me")
        );
        assert!(list.sections[0].bots[0].themes.is_empty());

        // The default bot is in no section, so flattening prepends it.
        let flattened = list.flatten();
        assert_eq!(flattened.len(), 2);
        assert_eq!(flattened[0].persona_id, "persona-default");
    }

    #[test]
    fn keeps_sections_whose_type_is_not_recognised() {
        let iq = iq_with(
            NodeBuilder::new("bot")
                .attr("v", "2")
                .children([
                    NodeBuilder::new("default")
                        .attr("jid", "10000000000000@bot")
                        .attr("persona_id", "persona-default")
                        .build(),
                    NodeBuilder::new("section")
                        .attr("type", "all")
                        .children([bot_node("10000000000000@bot", "persona-all").build()])
                        .build(),
                    NodeBuilder::new("section")
                        .attr("type", "carousel_v9")
                        .children([bot_node("20000000000000@bot", "persona-new").build()])
                        .build(),
                ])
                .build(),
        );

        let list = parse(&iq).expect("unknown section types must not fail the parse");
        assert_eq!(list.sections.len(), 2);
        assert_eq!(
            list.sections[1].section_type,
            BotSectionType::Other("carousel_v9".to_string())
        );
        // The bot only the unknown section carries is still reachable.
        let personas: Vec<String> = list
            .flatten()
            .into_iter()
            .map(|bot| bot.persona_id)
            .collect();
        assert_eq!(personas, vec!["persona-all", "persona-new"]);
    }

    #[test]
    fn keeps_an_unknown_protocol_version() {
        // An unknown version is an unknown contract, so v2's <default> rule
        // must not be applied to it.
        let iq = iq_with(NodeBuilder::new("bot").attr("v", "4").build());
        let list = parse(&iq).expect("unknown version must not fail the parse");
        assert_eq!(list.version, BotListVersion::Other("4".to_string()));
        assert_eq!(list.default_bot, None);
    }

    #[test]
    fn v2_response_without_a_default_is_an_error() {
        // The official v2 parser requires <default>; a response without one makes
        // it fall through to v3, which then fails on the missing bhash.
        let iq = iq_with(
            NodeBuilder::new("bot")
                .attr("v", "2")
                .children([NodeBuilder::new("section")
                    .attr("type", "all")
                    .children([bot_node("10000000000000@bot", "persona-all").build()])
                    .build()])
                .build(),
        );
        let err = parse(&iq).expect_err("a v2 list with no <default> must fail");
        assert!(err.to_string().contains("<default>"), "got: {err}");
    }

    #[test]
    fn v3_response_without_a_default_is_accepted() {
        let iq = iq_with(
            NodeBuilder::new("bot")
                .attr("v", "3")
                .attr("bhash", "cafebabe")
                .children([NodeBuilder::new("section")
                    .attr("type", "all")
                    .attr("display_type", "listview")
                    .children([bot_node("10000000000000@bot", "persona-all").build()])
                    .build()])
                .build(),
        );
        let list = parse(&iq).expect("v3 makes <default> optional");
        assert_eq!(list.default_bot, None);
        assert_eq!(list.flatten().len(), 1);
    }

    #[test]
    fn v3_response_without_a_bhash_is_an_error() {
        // bhash is required by the official v3 parser the same way <default> is
        // required by v2, so a v3 response without it is one no client accepts.
        let iq = iq_with(
            NodeBuilder::new("bot")
                .attr("v", "3")
                .children([NodeBuilder::new("section")
                    .attr("type", "all")
                    .children([bot_node("10000000000000@bot", "persona-all").build()])
                    .build()])
                .build(),
        );
        let err = parse(&iq).expect_err("a v3 list with no bhash must fail");
        assert!(err.to_string().contains("bhash"), "got: {err}");
    }

    #[test]
    fn v2_response_without_a_bhash_is_accepted() {
        // The v2 variant has no bhash at all; the v3 rule must not leak into it.
        let iq = iq_with(
            NodeBuilder::new("bot")
                .attr("v", "2")
                .children([NodeBuilder::new("default")
                    .attr("jid", "10000000000000@bot")
                    .attr("persona_id", "persona-default")
                    .build()])
                .build(),
        );
        let list = parse(&iq).expect("v2 carries no bhash");
        assert_eq!(list.bhash, None);
    }

    #[test]
    fn default_without_persona_id_is_an_error() {
        let iq = iq_with(
            NodeBuilder::new("bot")
                .attr("v", "2")
                .children([NodeBuilder::new("default")
                    .attr("jid", "10000000000000@bot")
                    .build()])
                .build(),
        );
        let err = parse(&iq).expect_err("a <default> with no persona_id must fail");
        assert!(err.to_string().contains("persona_id"), "got: {err}");
    }

    #[test]
    fn response_without_a_bot_node_is_an_error() {
        let iq = NodeBuilder::new("iq").attr("type", "result").build();
        let err = parse(&iq).expect_err("a response with no <bot> child must fail");
        assert!(err.to_string().contains("<bot>"), "got: {err}");
    }

    #[test]
    fn server_error_response_is_an_error() {
        // The request layer turns `type="error"` into `IqError::ServerError` before
        // the spec sees it; this pins the behaviour if one ever reaches the parser.
        let iq = NodeBuilder::new("iq")
            .attr("type", "error")
            .attr("from", "s.whatsapp.net")
            .children([NodeBuilder::new("error")
                .attr("code", "403")
                .attr("text", "forbidden")
                .build()])
            .build();
        let err = parse(&iq).expect_err("an error IQ must not parse as a bot list");
        assert!(err.to_string().contains("<bot>"), "got: {err}");
    }

    #[test]
    fn response_without_a_version_attribute_is_an_error() {
        let iq = iq_with(NodeBuilder::new("bot").build());
        let err = parse(&iq).expect_err("a <bot> with no v attribute must fail");
        assert!(err.to_string().contains("v attribute"), "got: {err}");
    }

    #[test]
    fn bot_without_persona_id_is_an_error() {
        let iq = iq_with(
            NodeBuilder::new("bot")
                .attr("v", "2")
                .children([
                    NodeBuilder::new("default")
                        .attr("jid", "10000000000000@bot")
                        .attr("persona_id", "persona-default")
                        .build(),
                    NodeBuilder::new("section")
                        .attr("type", "all")
                        .children([NodeBuilder::new("bot")
                            .attr("jid", "10000000000000@bot")
                            .build()])
                        .build(),
                ])
                .build(),
        );
        let err = parse(&iq).expect_err("a <bot> with no persona_id must fail");
        assert!(err.to_string().contains("persona_id"), "got: {err}");
    }

    #[test]
    fn section_without_a_type_is_an_error() {
        let iq = iq_with(
            NodeBuilder::new("bot")
                .attr("v", "2")
                .children([
                    NodeBuilder::new("default")
                        .attr("jid", "10000000000000@bot")
                        .attr("persona_id", "persona-default")
                        .build(),
                    NodeBuilder::new("section").attr("name", "All").build(),
                ])
                .build(),
        );
        let err = parse(&iq).expect_err("a <section> with no type must fail");
        assert!(err.to_string().contains("type attribute"), "got: {err}");
    }

    #[test]
    fn bot_list_round_trips_through_a_node() {
        let list = BotList {
            version: BotListVersion::V3,
            bhash: Some("cafebabe".to_string()),
            default_bot: Some(BotDefault::new(
                "10000000000000@bot".parse().expect("jid"),
                "persona-default".to_string(),
            )),
            sections: vec![BotListSection {
                name: Some("All".to_string()),
                section_type: BotSectionType::All,
                display_type: Some(BotSectionDisplayType::Listview),
                bots: vec![BotListEntry {
                    card_title: Some("Try me".to_string()),
                    count: Some(3),
                    themes: vec![BotTheme {
                        mode: BotThemeMode::Dark,
                        background: Some("#000000".to_string()),
                        primary_text: Some("#FFFFFF".to_string()),
                        secondary_text: None,
                    }],
                    ..BotListEntry::new(
                        "20000000000000@bot".parse().expect("jid"),
                        "persona-second".to_string(),
                    )
                }],
            }],
        };

        let node = list.clone().into_node();
        assert_eq!(BotList::try_from_node(&node).expect("round trip"), list);
    }
}

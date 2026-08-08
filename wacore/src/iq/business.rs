//! Business profile IQ specification (namespace `w:biz`).

use crate::WireEnum;
use crate::iq::node::optional_attr;
use crate::iq::spec::IqSpec;
use crate::request::InfoQuery;
use wacore_binary::builder::NodeBuilder;
use wacore_binary::node::Node;
use wacore_binary::{Jid, Server};
use wacore_binary::{NodeContent, NodeContentRef, NodeRef};

/// `v` attribute on the `business_profile` node of a *write*. The read path
/// sends `244`; WhatsApp Web's mutation path sends `3` and the two are not
/// interchangeable, so they stay separate constants.
const BUSINESS_PROFILE_MUTATION_VERSION: &str = "3";

/// WhatsApp Web emits at most two `<website/>` nodes per profile mutation
/// (`websites[0]` and `websites[1]`); further entries are silently dropped by
/// the client rather than sent. Rejecting them here keeps a caller from
/// believing a third URL was stored.
pub const BUSINESS_PROFILE_MAX_WEBSITES: usize = 2;

#[derive(Debug, Clone, PartialEq, Eq, WireEnum)]
pub enum DayOfWeek {
    #[wire = "sun"]
    Sunday,
    #[wire = "mon"]
    Monday,
    #[wire = "tue"]
    Tuesday,
    #[wire = "wed"]
    Wednesday,
    #[wire = "thu"]
    Thursday,
    #[wire = "fri"]
    Friday,
    #[wire = "sat"]
    Saturday,
    #[wire_fallback]
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq, WireEnum)]
pub enum BusinessHourMode {
    #[wire = "open_24h"]
    Open24H,
    #[wire = "specific_hours"]
    SpecificHours,
    #[wire = "appointment_only"]
    AppointmentOnly,
    #[wire_fallback]
    Other(String),
}

fn node_text(node: &NodeRef<'_>) -> Option<String> {
    match node.content.as_ref() {
        Some(NodeContentRef::String(s)) => Some(s.to_string()),
        Some(NodeContentRef::Bytes(b)) => std::str::from_utf8(b).ok().map(|s| s.to_string()),
        _ => None,
    }
}

#[derive(Debug, Clone, Default, serde::Serialize)]
#[non_exhaustive]
pub struct BusinessProfile {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wid: Option<Jid>,
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
    pub website: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub categories: Vec<BusinessCategory>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    pub business_hours: BusinessHours,
}

#[derive(Debug, Clone, Default, serde::Serialize)]
#[non_exhaustive]
pub struct BusinessHours {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timezone: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub business_config: Option<Vec<BusinessHoursConfig>>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[non_exhaustive]
pub struct BusinessHoursConfig {
    pub day_of_week: DayOfWeek,
    pub mode: BusinessHourMode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub open_time: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub close_time: Option<u32>,
}

impl BusinessHoursConfig {
    /// A day whose `mode` carries no explicit range (`open_24h`,
    /// `appointment_only`, or a closed day).
    pub fn new(day_of_week: DayOfWeek, mode: BusinessHourMode) -> Self {
        Self {
            day_of_week,
            mode,
            open_time: None,
            close_time: None,
        }
    }

    /// A single opening range, in minutes past local midnight. WhatsApp Web
    /// emits one `business_hours_config` per range, so a day with two ranges is
    /// two entries sharing a `day_of_week`.
    pub fn with_hours(
        day_of_week: DayOfWeek,
        mode: BusinessHourMode,
        open_time: u32,
        close_time: u32,
    ) -> Self {
        Self {
            day_of_week,
            mode,
            open_time: Some(open_time),
            close_time: Some(close_time),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize)]
#[non_exhaustive]
pub struct BusinessCategory {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct BusinessProfileSpec {
    pub jid: Jid,
}

impl BusinessProfileSpec {
    pub fn new(jid: &Jid) -> Self {
        Self { jid: jid.clone() }
    }
}

impl IqSpec for BusinessProfileSpec {
    type Response = Option<BusinessProfile>;

    fn build_iq(&self) -> InfoQuery<'static> {
        InfoQuery::get(
            "w:biz",
            Jid::new("", Server::Pn),
            Some(NodeContent::Nodes(vec![
                NodeBuilder::new("business_profile")
                    .attr("v", "244")
                    .children([NodeBuilder::new("profile").attr("jid", &self.jid).build()])
                    .build(),
            ])),
        )
    }

    fn parse_response(&self, response: &NodeRef<'_>) -> Result<Self::Response, anyhow::Error> {
        let biz_node = match response.get_optional_child("business_profile") {
            Some(n) => n,
            None => return Ok(None),
        };

        let profile_node = match biz_node.get_optional_child("profile") {
            Some(n) => n,
            None => return Ok(None),
        };

        let wid = optional_attr(profile_node, "jid").and_then(|s| s.parse::<Jid>().ok());

        let description = profile_node
            .get_optional_child("description")
            .and_then(node_text)
            .unwrap_or_default();

        let address = profile_node
            .get_optional_child("address")
            .and_then(node_text);

        let email = profile_node.get_optional_child("email").and_then(node_text);

        let website: Vec<String> = profile_node
            .get_children_by_tag("website")
            .filter_map(node_text)
            .collect();

        let categories: Vec<BusinessCategory> = profile_node
            .get_optional_child("categories")
            .map(|cats| {
                cats.get_children_by_tag("category")
                    .filter_map(|c| {
                        let id = optional_attr(c, "id")?.into_owned();
                        let name = node_text(c).unwrap_or_default();
                        Some(BusinessCategory { id, name })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let business_hours =
            if let Some(bh_node) = profile_node.get_optional_child("business_hours") {
                let timezone = optional_attr(bh_node, "timezone").map(|s| s.into_owned());
                let configs: Vec<BusinessHoursConfig> = bh_node
                    .get_children_by_tag("business_hours_config")
                    .filter_map(|c| {
                        let day = optional_attr(c, "day_of_week")?;
                        let mode_str = optional_attr(c, "mode")?;
                        Some(BusinessHoursConfig {
                            day_of_week: DayOfWeek::from(day.as_ref()),
                            mode: BusinessHourMode::from(mode_str.as_ref()),
                            open_time: optional_attr(c, "open_time")
                                .and_then(|s| s.parse::<u32>().ok()),
                            close_time: optional_attr(c, "close_time")
                                .and_then(|s| s.parse::<u32>().ok()),
                        })
                    })
                    .collect();

                BusinessHours {
                    timezone,
                    business_config: if configs.is_empty() {
                        None
                    } else {
                        Some(configs)
                    },
                }
            } else {
                BusinessHours::default()
            };

        Ok(Some(BusinessProfile {
            wid,
            description,
            email,
            website,
            categories,
            address,
            business_hours,
        }))
    }
}

/// Opening hours carried by a profile mutation.
///
/// Distinct from the read-side [`BusinessHours`] because the mutation also
/// carries a free-text note, which the profile query does not return.
#[derive(Debug, Clone, Default)]
pub struct BusinessHoursUpdate {
    /// IANA zone name (e.g. `America/Araguaina`). Omitted when `None`.
    pub timezone: Option<String>,
    /// Free-text note shown under the hours. `Some("")` is not special-cased:
    /// WhatsApp Web drops empty notes, and so does the builder.
    pub note: Option<String>,
    /// One entry per opening range; see [`BusinessHoursConfig::with_hours`].
    pub config: Vec<BusinessHoursConfig>,
}

/// A delta mutation of the business profile.
///
/// Every field is `None` by default and a `None` field is left untouched —
/// this is a delta, not a replacement. The distinction that matters is between
/// "leave alone" and "clear": clearing is `Some` of an empty value
/// (`Some(String::new())` for text, `Some(Vec::new())` for `websites`).
#[derive(Debug, Clone, Default)]
pub struct BusinessProfileUpdate {
    pub address: Option<String>,
    pub latitude: Option<f64>,
    pub longitude: Option<f64>,
    pub description: Option<String>,
    pub email: Option<String>,
    /// Up to [`BUSINESS_PROFILE_MAX_WEBSITES`] URLs. `Some(vec![])` clears the
    /// list, matching WhatsApp Web's lone empty `<website/>` node.
    pub websites: Option<Vec<String>>,
    /// Category *ids* (from the business categories directory), not names.
    pub categories: Option<Vec<String>>,
    pub business_hours: Option<BusinessHoursUpdate>,
}

#[derive(Debug, thiserror::Error, PartialEq)]
#[non_exhaustive]
pub enum BusinessProfileUpdateError {
    /// A delta with no fields set would be a round trip that changes nothing.
    #[error("business profile update is empty")]
    Empty,
    #[error(
        "business profile accepts at most {BUSINESS_PROFILE_MAX_WEBSITES} websites, got {count}"
    )]
    TooManyWebsites { count: usize },
    /// `open_time`/`close_time` are minutes past local midnight. A value at or
    /// past the end of the day is serialized verbatim, and the server rejects
    /// the whole delta for it — losing the other fields in the same update.
    #[error("{day} {field} {value} is not a minute of the day (must be < {MINUTES_PER_DAY})")]
    InvalidBusinessHourTime {
        day: String,
        field: &'static str,
        value: u32,
    },
    /// Times and `specific_hours` go together. WhatsApp Web emits `open_time`
    /// and `close_time` exactly when a day has a range, and reads them back
    /// only for `specific_hours` — so a ranged `open_24h`, or a
    /// `specific_hours` day with no range, is a stanza it never produces.
    /// A mode this crate does not know is left alone rather than guessed at.
    #[error("{day} is {mode}, which {expectation}")]
    MismatchedBusinessHourMode {
        day: String,
        mode: String,
        expectation: &'static str,
    },
    /// WhatsApp Web writes `open_time` and `close_time` together, from one
    /// `[open, close]` pair, so half of a range is a shape it never emits —
    /// whatever the mode.
    #[error("{day} has only one of open_time and close_time")]
    IncompleteBusinessHourRange { day: String },
    /// `f64::to_string` would render a non-finite or out-of-range coordinate as
    /// `NaN`/`inf`, or as a latitude no point on earth has. The server rejects
    /// the whole delta in that case, so the other fields in the same update
    /// would be silently lost too.
    #[error("{axis} {value} is not a finite coordinate within ±{limit}")]
    InvalidCoordinate {
        axis: &'static str,
        value: f64,
        limit: f64,
    },
}

const LATITUDE_LIMIT: f64 = 90.0;
const LONGITUDE_LIMIT: f64 = 180.0;

/// Minutes in a day, and the exclusive upper bound on an opening time.
///
/// WhatsApp Web renders these with `startOf("day").add(n, "minutes")` and only
/// ever produces them by parsing a within-day time string, so 1439 (23:59) is
/// the largest value it emits. Ranges that cross midnight are deliberately
/// *not* rejected here: WA Web's own editor coerces `open > close` in the
/// picker, but that is a UI affordance rather than a wire constraint, and other
/// clients may legitimately send such a range.
const MINUTES_PER_DAY: u32 = 1440;

fn check_coordinate(
    axis: &'static str,
    value: f64,
    limit: f64,
) -> Result<(), BusinessProfileUpdateError> {
    // Rejects NaN too: every comparison against NaN is false, so `abs() <=`
    // fails for it without a separate is_nan check.
    if value.abs() <= limit {
        return Ok(());
    }
    Err(BusinessProfileUpdateError::InvalidCoordinate { axis, value, limit })
}

/// A day carries a range exactly when its mode is `specific_hours`.
///
/// An unrecognised mode is accepted either way: `BusinessHourMode::Other`
/// exists so a mode WhatsApp adds later degrades to a passthrough, and
/// guessing its arity here would turn that into a hard failure.
fn check_mode_pairing(config: &BusinessHoursConfig) -> Result<(), BusinessProfileUpdateError> {
    // A half-open range is malformed whatever the mode — including one this
    // crate does not know — because WhatsApp Web only ever emits the two times
    // together, from a single `[open, close]` pair. So it is settled before the
    // mode is consulted at all.
    let has_range = match (config.open_time, config.close_time) {
        (Some(_), Some(_)) => true,
        (None, None) => false,
        _ => {
            return Err(BusinessProfileUpdateError::IncompleteBusinessHourRange {
                day: config.day_of_week.as_str().to_string(),
            });
        }
    };
    let expectation = match (&config.mode, has_range) {
        (BusinessHourMode::SpecificHours, false) => "needs an opening and closing time",
        (BusinessHourMode::Open24H | BusinessHourMode::AppointmentOnly, true) => {
            "carries no opening or closing time"
        }
        _ => return Ok(()),
    };
    Err(BusinessProfileUpdateError::MismatchedBusinessHourMode {
        day: config.day_of_week.as_str().to_string(),
        mode: config.mode.as_str().to_string(),
        expectation,
    })
}

fn check_minutes(
    day: &DayOfWeek,
    field: &'static str,
    value: u32,
) -> Result<(), BusinessProfileUpdateError> {
    if value < MINUTES_PER_DAY {
        return Ok(());
    }
    Err(BusinessProfileUpdateError::InvalidBusinessHourTime {
        day: day.as_str().to_string(),
        field,
        value,
    })
}

impl BusinessProfileUpdate {
    fn is_empty(&self) -> bool {
        self.address.is_none()
            && self.latitude.is_none()
            && self.longitude.is_none()
            && self.description.is_none()
            && self.email.is_none()
            && self.websites.is_none()
            && self.categories.is_none()
            && self.business_hours.is_none()
    }

    /// Children of the `business_profile` node, in WhatsApp Web's emission order.
    fn to_nodes(&self) -> Result<Vec<Node>, BusinessProfileUpdateError> {
        if self.is_empty() {
            return Err(BusinessProfileUpdateError::Empty);
        }
        if let Some(websites) = &self.websites
            && websites.len() > BUSINESS_PROFILE_MAX_WEBSITES
        {
            return Err(BusinessProfileUpdateError::TooManyWebsites {
                count: websites.len(),
            });
        }
        if let Some(latitude) = self.latitude {
            check_coordinate("latitude", latitude, LATITUDE_LIMIT)?;
        }
        if let Some(longitude) = self.longitude {
            check_coordinate("longitude", longitude, LONGITUDE_LIMIT)?;
        }
        if let Some(hours) = &self.business_hours {
            for config in &hours.config {
                if let Some(open_time) = config.open_time {
                    check_minutes(&config.day_of_week, "open_time", open_time)?;
                }
                if let Some(close_time) = config.close_time {
                    check_minutes(&config.day_of_week, "close_time", close_time)?;
                }
                check_mode_pairing(config)?;
            }
        }

        let mut nodes = Vec::new();
        let mut text = |tag: &'static str, value: &str| {
            nodes.push(NodeBuilder::new(tag).string_content(value).build());
        };

        if let Some(address) = &self.address {
            text("address", address);
        }
        if let Some(latitude) = self.latitude {
            text("latitude", &latitude.to_string());
        }
        if let Some(longitude) = self.longitude {
            text("longitude", &longitude.to_string());
        }
        if let Some(description) = &self.description {
            text("description", description);
        }
        if let Some(email) = &self.email {
            text("email", email);
        }

        if let Some(websites) = &self.websites {
            if websites.is_empty() {
                // A single childless <website/> is how the client clears the
                // list; omitting the node entirely would leave it untouched.
                nodes.push(NodeBuilder::new("website").build());
            } else {
                for website in websites {
                    nodes.push(
                        NodeBuilder::new("website")
                            .string_content(website.as_str())
                            .build(),
                    );
                }
            }
        }

        if let Some(categories) = &self.categories {
            nodes.push(
                NodeBuilder::new("categories")
                    .children(
                        categories
                            .iter()
                            .map(|id| NodeBuilder::new("category").attr("id", id.as_str()).build()),
                    )
                    .build(),
            );
        }

        if let Some(hours) = &self.business_hours {
            let mut children = Vec::new();
            if let Some(note) = &hours.note
                && !note.is_empty()
            {
                children.push(
                    NodeBuilder::new("business_hours_note")
                        .string_content(note.as_str())
                        .build(),
                );
            }
            for config in &hours.config {
                let mut builder = NodeBuilder::new("business_hours_config")
                    .attr("day_of_week", config.day_of_week.as_str())
                    .attr("mode", config.mode.as_str());
                // Absent times are dropped attributes, not zeros: `open_time=0`
                // is a valid midnight opening.
                if let Some(open_time) = config.open_time {
                    builder = builder.attr("open_time", open_time.to_string());
                }
                if let Some(close_time) = config.close_time {
                    builder = builder.attr("close_time", close_time.to_string());
                }
                children.push(builder.build());
            }

            let mut builder = NodeBuilder::new("business_hours");
            if let Some(timezone) = &hours.timezone {
                builder = builder.attr("timezone", timezone.as_str());
            }
            nodes.push(builder.children(children).build());
        }

        Ok(nodes)
    }
}

/// Wraps mutation children in the `business_profile` delta envelope and the
/// surrounding `w:biz` set. Shared by every write in this namespace.
fn business_profile_mutation(children: Vec<Node>) -> InfoQuery<'static> {
    InfoQuery::set(
        "w:biz",
        Jid::new("", Server::Pn),
        Some(NodeContent::Nodes(vec![
            NodeBuilder::new("business_profile")
                .attr("v", BUSINESS_PROFILE_MUTATION_VERSION)
                .attr("mutation_type", "delta")
                .children(children)
                .build(),
        ])),
    )
}

/// Apply a delta to the authenticated account's own business profile.
#[derive(Debug, Clone)]
pub struct BusinessProfileUpdateSpec {
    nodes: Vec<Node>,
}

impl BusinessProfileUpdateSpec {
    /// Validates the update up front so a rejected delta never reaches the wire.
    pub fn new(update: &BusinessProfileUpdate) -> Result<Self, BusinessProfileUpdateError> {
        Ok(Self {
            nodes: update.to_nodes()?,
        })
    }
}

impl IqSpec for BusinessProfileUpdateSpec {
    type Response = ();

    fn build_iq(&self) -> InfoQuery<'static> {
        business_profile_mutation(self.nodes.clone())
    }

    fn parse_response(&self, _response: &NodeRef<'_>) -> Result<Self::Response, anyhow::Error> {
        // The server acknowledges with a bare <iq type="result"/>; a non-result
        // type is already turned into an error before parsing.
        Ok(())
    }
}

/// The upload receipt a cover photo mutation has to quote back.
///
/// These three values come from a `biz-cover-photo` media upload, whose
/// response carries `fbid`, `meta_hmac` and `ts` instead of the
/// `url`/`direct_path` pair every other media type returns. This crate does not
/// yet perform that upload — see the module docs on [`SetCoverPhotoSpec`].
#[derive(Debug, Clone)]
pub struct CoverPhotoUpload {
    /// `fbid` from the upload response.
    pub id: String,
    /// `meta_hmac` from the upload response.
    pub token: String,
    /// `ts` from the upload response.
    pub timestamp: i64,
}

/// Point the business profile at an already-uploaded cover photo.
///
/// Takes a [`CoverPhotoUpload`] rather than image bytes because the
/// `biz-cover-photo` upload leg is not implemented here: this crate's upload
/// pipeline parses `url`/`direct_path` out of every upload response, and the
/// cover-photo endpoint returns `fbid`/`meta_hmac`/`ts` instead.
#[derive(Debug, Clone)]
pub struct SetCoverPhotoSpec {
    upload: CoverPhotoUpload,
}

impl SetCoverPhotoSpec {
    pub fn new(upload: CoverPhotoUpload) -> Self {
        Self { upload }
    }
}

impl IqSpec for SetCoverPhotoSpec {
    type Response = ();

    fn build_iq(&self) -> InfoQuery<'static> {
        business_profile_mutation(vec![
            NodeBuilder::new("cover_photo")
                .attr("op", "update")
                .attr("id", self.upload.id.as_str())
                .attr("ts", self.upload.timestamp.to_string())
                .attr("token", self.upload.token.as_str())
                .build(),
        ])
    }

    fn parse_response(&self, _response: &NodeRef<'_>) -> Result<Self::Response, anyhow::Error> {
        Ok(())
    }
}

/// Remove the business profile's cover photo. `id` is the `fbid` of the photo
/// currently set.
#[derive(Debug, Clone)]
pub struct RemoveCoverPhotoSpec {
    id: String,
}

impl RemoveCoverPhotoSpec {
    pub fn new(id: &str) -> Self {
        Self { id: id.to_string() }
    }
}

impl IqSpec for RemoveCoverPhotoSpec {
    type Response = ();

    fn build_iq(&self) -> InfoQuery<'static> {
        business_profile_mutation(vec![
            NodeBuilder::new("cover_photo")
                .attr("op", "delete")
                .attr("id", self.id.as_str())
                .build(),
        ])
    }

    fn parse_response(&self, _response: &NodeRef<'_>) -> Result<Self::Response, anyhow::Error> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_absent_business_hour_times() {
        let jid: Jid = "5511999999999@s.whatsapp.net".parse().unwrap();
        let spec = BusinessProfileSpec::new(&jid);
        let response = NodeBuilder::new("iq")
            .children([NodeBuilder::new("business_profile")
                .children([NodeBuilder::new("profile")
                    .attr("jid", &jid)
                    .children([NodeBuilder::new("business_hours")
                        .attr("timezone", "America/Araguaina")
                        .children([
                            NodeBuilder::new("business_hours_config")
                                .attr("day_of_week", "mon")
                                .attr("mode", "open_24h")
                                .build(),
                            NodeBuilder::new("business_hours_config")
                                .attr("day_of_week", "tue")
                                .attr("mode", "specific_hours")
                                .attr("open_time", "480")
                                .attr("close_time", "1080")
                                .build(),
                        ])
                        .build()])
                    .build()])
                .build()])
            .build();

        let profile = spec
            .parse_response(&response.as_node_ref())
            .unwrap()
            .unwrap();
        let configs = profile.business_hours.business_config.unwrap();

        assert_eq!(configs[0].open_time, None);
        assert_eq!(configs[0].close_time, None);
        assert_eq!(configs[1].open_time, Some(480));
        assert_eq!(configs[1].close_time, Some(1080));
    }

    /// Children of the single `business_profile` node in a mutation.
    fn mutation_children(iq: &InfoQuery<'static>) -> Vec<Node> {
        let Some(NodeContent::Nodes(nodes)) = &iq.content else {
            panic!("mutation content should be nodes");
        };
        let profile = &nodes[0];
        assert_eq!(profile.tag.as_ref(), "business_profile");
        assert_eq!(
            profile.attrs.get("v").map(|v| v.to_string()).as_deref(),
            Some("3")
        );
        assert_eq!(
            profile
                .attrs
                .get("mutation_type")
                .map(|v| v.to_string())
                .as_deref(),
            Some("delta")
        );
        match &profile.content {
            Some(NodeContent::Nodes(children)) => children.clone(),
            _ => Vec::new(),
        }
    }

    fn child_text(node: &Node) -> Option<String> {
        match &node.content {
            Some(NodeContent::String(s)) => Some(s.to_string()),
            Some(NodeContent::Bytes(b)) => String::from_utf8(b.clone()).ok(),
            _ => None,
        }
    }

    #[test]
    fn profile_update_builds_delta_mutation() {
        let update = BusinessProfileUpdate {
            address: Some("221B Baker Street".into()),
            description: Some("We fix things".into()),
            email: Some("shop@example.invalid".into()),
            websites: Some(vec!["https://example.invalid".into()]),
            ..Default::default()
        };
        let spec = BusinessProfileUpdateSpec::new(&update).unwrap();
        let iq = spec.build_iq();

        assert_eq!(iq.namespace, "w:biz");
        assert!(matches!(iq.query_type, crate::request::InfoQueryType::Set));

        let children = mutation_children(&iq);
        let tags: Vec<&str> = children.iter().map(|n| n.tag.as_ref()).collect();
        assert_eq!(tags, ["address", "description", "email", "website"]);
        assert_eq!(
            child_text(&children[0]).as_deref(),
            Some("221B Baker Street")
        );
        assert_eq!(
            child_text(&children[3]).as_deref(),
            Some("https://example.invalid")
        );
    }

    /// The empty `<website/>` node is the only way to clear the list; dropping
    /// it would silently turn "clear" into "leave alone".
    #[test]
    fn empty_website_list_emits_a_clearing_node() {
        let update = BusinessProfileUpdate {
            websites: Some(Vec::new()),
            ..Default::default()
        };
        let children =
            mutation_children(&BusinessProfileUpdateSpec::new(&update).unwrap().build_iq());

        assert_eq!(children.len(), 1);
        assert_eq!(children[0].tag.as_ref(), "website");
        assert_eq!(child_text(&children[0]), None);
    }

    /// A field left `None` is "leave alone" and must not reach the wire at all.
    #[test]
    fn absent_fields_are_not_emitted() {
        let update = BusinessProfileUpdate {
            email: Some("shop@example.invalid".into()),
            ..Default::default()
        };
        let children =
            mutation_children(&BusinessProfileUpdateSpec::new(&update).unwrap().build_iq());

        assert_eq!(children.len(), 1);
        assert_eq!(children[0].tag.as_ref(), "email");
    }

    #[test]
    fn business_hours_carry_note_and_per_range_configs() {
        let update = BusinessProfileUpdate {
            business_hours: Some(BusinessHoursUpdate {
                timezone: Some("America/Araguaina".into()),
                note: Some("Closed on holidays".into()),
                config: vec![
                    BusinessHoursConfig::new(DayOfWeek::Sunday, BusinessHourMode::Open24H),
                    BusinessHoursConfig::with_hours(
                        DayOfWeek::Monday,
                        BusinessHourMode::SpecificHours,
                        0,
                        1080,
                    ),
                ],
            }),
            ..Default::default()
        };
        let children =
            mutation_children(&BusinessProfileUpdateSpec::new(&update).unwrap().build_iq());

        let hours = &children[0];
        assert_eq!(hours.tag.as_ref(), "business_hours");
        assert_eq!(
            hours
                .attrs
                .get("timezone")
                .map(|v| v.to_string())
                .as_deref(),
            Some("America/Araguaina")
        );
        let Some(NodeContent::Nodes(entries)) = &hours.content else {
            panic!("business_hours should have children");
        };
        assert_eq!(entries[0].tag.as_ref(), "business_hours_note");
        assert_eq!(
            child_text(&entries[0]).as_deref(),
            Some("Closed on holidays")
        );

        // open_24h carries no range at all...
        assert_eq!(entries[1].attrs.get("open_time"), None);
        assert_eq!(entries[1].attrs.get("close_time"), None);
        // ...while a midnight opening is a real 0, not an omission.
        assert_eq!(
            entries[2]
                .attrs
                .get("open_time")
                .map(|v| v.to_string())
                .as_deref(),
            Some("0")
        );
        assert_eq!(
            entries[2]
                .attrs
                .get("close_time")
                .map(|v| v.to_string())
                .as_deref(),
            Some("1080")
        );
    }

    #[test]
    fn empty_update_is_rejected() {
        assert_eq!(
            BusinessProfileUpdateSpec::new(&BusinessProfileUpdate::default()).unwrap_err(),
            BusinessProfileUpdateError::Empty
        );
    }

    #[test]
    fn more_than_two_websites_is_rejected() {
        let update = BusinessProfileUpdate {
            websites: Some(vec![
                "https://a.invalid".into(),
                "https://b.invalid".into(),
                "https://c.invalid".into(),
            ]),
            ..Default::default()
        };
        assert_eq!(
            BusinessProfileUpdateSpec::new(&update).unwrap_err(),
            BusinessProfileUpdateError::TooManyWebsites { count: 3 }
        );
    }

    #[test]
    fn coordinates_are_emitted_when_in_range() {
        let update = BusinessProfileUpdate {
            latitude: Some(-23.55052),
            longitude: Some(-46.633308),
            ..Default::default()
        };
        let children =
            mutation_children(&BusinessProfileUpdateSpec::new(&update).unwrap().build_iq());

        assert_eq!(children[0].tag.as_ref(), "latitude");
        assert_eq!(child_text(&children[0]).as_deref(), Some("-23.55052"));
        assert_eq!(children[1].tag.as_ref(), "longitude");
        assert_eq!(child_text(&children[1]).as_deref(), Some("-46.633308"));
    }

    /// `f64::to_string` renders these as `NaN`/`inf`, which the server rejects —
    /// taking the rest of the delta down with it.
    #[test]
    fn non_finite_and_out_of_range_coordinates_are_rejected() {
        for latitude in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 90.5, -91.0] {
            let update = BusinessProfileUpdate {
                latitude: Some(latitude),
                ..Default::default()
            };
            assert!(
                matches!(
                    BusinessProfileUpdateSpec::new(&update),
                    Err(BusinessProfileUpdateError::InvalidCoordinate {
                        axis: "latitude",
                        ..
                    })
                ),
                "latitude {latitude} should be rejected"
            );
        }

        // Longitude has the wider bound: 100 is fine here, illegal as a latitude.
        for longitude in [f64::NAN, 180.5, -181.0] {
            let update = BusinessProfileUpdate {
                longitude: Some(longitude),
                ..Default::default()
            };
            assert!(
                matches!(
                    BusinessProfileUpdateSpec::new(&update),
                    Err(BusinessProfileUpdateError::InvalidCoordinate {
                        axis: "longitude",
                        ..
                    })
                ),
                "longitude {longitude} should be rejected"
            );
        }

        let wide = BusinessProfileUpdate {
            longitude: Some(150.0),
            ..Default::default()
        };
        assert!(BusinessProfileUpdateSpec::new(&wide).is_ok());
    }

    /// Minutes past midnight are serialized verbatim, so a value outside the day
    /// makes the server reject the whole delta — the same failure mode as an
    /// out-of-range coordinate.
    #[test]
    fn business_hour_times_outside_the_day_are_rejected() {
        for (open, close, field) in [
            (1440, 1020, "open_time"),
            (540, 1440, "close_time"),
            (u32::MAX, 1020, "open_time"),
        ] {
            let update = BusinessProfileUpdate {
                business_hours: Some(BusinessHoursUpdate {
                    note: None,
                    timezone: None,
                    config: vec![BusinessHoursConfig::with_hours(
                        DayOfWeek::Monday,
                        BusinessHourMode::SpecificHours,
                        open,
                        close,
                    )],
                }),
                ..Default::default()
            };
            let err = BusinessProfileUpdateSpec::new(&update)
                .err()
                .unwrap_or_else(|| panic!("{open}-{close} should be rejected"));
            assert!(
                matches!(
                    &err,
                    BusinessProfileUpdateError::InvalidBusinessHourTime { field: f, .. } if *f == field
                ),
                "{open}-{close} should name {field}, got {err:?}"
            );
        }
    }

    /// The bounds of a legal day, plus a range crossing midnight: WA Web's editor
    /// coerces `open > close` in its picker, but that is a UI affordance, not a
    /// wire constraint, so the builder must not invent one.
    #[test]
    fn business_hour_times_within_the_day_are_accepted() {
        for (open, close) in [(0, 1439), (540, 1020), (1200, 120)] {
            let update = BusinessProfileUpdate {
                business_hours: Some(BusinessHoursUpdate {
                    note: None,
                    timezone: None,
                    config: vec![BusinessHoursConfig::with_hours(
                        DayOfWeek::Monday,
                        BusinessHourMode::SpecificHours,
                        open,
                        close,
                    )],
                }),
                ..Default::default()
            };
            assert!(
                BusinessProfileUpdateSpec::new(&update).is_ok(),
                "{open}-{close} should be accepted"
            );
        }
    }

    /// WhatsApp Web emits times exactly when a day has a range, and reads them
    /// back only for `specific_hours`, so the two other shapes are stanzas it
    /// never produces.
    #[test]
    fn business_hour_mode_must_match_the_presence_of_a_range() {
        let ranged_but_not_specific = BusinessHoursConfig::with_hours(
            DayOfWeek::Monday,
            BusinessHourMode::Open24H,
            540,
            1020,
        );
        let specific_without_range =
            BusinessHoursConfig::new(DayOfWeek::Tuesday, BusinessHourMode::SpecificHours);

        for config in [ranged_but_not_specific, specific_without_range] {
            let update = BusinessProfileUpdate {
                business_hours: Some(BusinessHoursUpdate {
                    note: None,
                    timezone: None,
                    config: vec![config.clone()],
                }),
                ..Default::default()
            };
            assert!(
                matches!(
                    BusinessProfileUpdateSpec::new(&update),
                    Err(BusinessProfileUpdateError::MismatchedBusinessHourMode { .. })
                ),
                "{config:?} should be rejected"
            );
        }
    }

    /// WA Web writes the two times together from one `[open, close]` pair, so
    /// half a range is malformed under every mode — including an unknown one,
    /// where the shape is broken regardless of what arity the mode implies.
    #[test]
    fn half_open_business_hour_range_is_rejected_under_any_mode() {
        for mode in [
            BusinessHourMode::SpecificHours,
            BusinessHourMode::Open24H,
            BusinessHourMode::Other("seasonal".into()),
        ] {
            for (open, close) in [(Some(540), None), (None, Some(1020))] {
                let mut config = BusinessHoursConfig::new(DayOfWeek::Monday, mode.clone());
                config.open_time = open;
                config.close_time = close;
                let update = BusinessProfileUpdate {
                    business_hours: Some(BusinessHoursUpdate {
                        note: None,
                        timezone: None,
                        config: vec![config],
                    }),
                    ..Default::default()
                };
                assert!(
                    matches!(
                        BusinessProfileUpdateSpec::new(&update),
                        Err(BusinessProfileUpdateError::IncompleteBusinessHourRange { .. })
                    ),
                    "{mode:?} with {open:?}/{close:?} should be rejected"
                );
            }
        }
    }

    /// `Other` exists so a mode WhatsApp adds later still round-trips; guessing
    /// whether it takes a range would turn that passthrough into a failure.
    #[test]
    fn unknown_business_hour_mode_is_left_alone() {
        for config in [
            BusinessHoursConfig::new(
                DayOfWeek::Monday,
                BusinessHourMode::Other("seasonal".into()),
            ),
            BusinessHoursConfig::with_hours(
                DayOfWeek::Monday,
                BusinessHourMode::Other("seasonal".into()),
                540,
                1020,
            ),
        ] {
            let update = BusinessProfileUpdate {
                business_hours: Some(BusinessHoursUpdate {
                    note: None,
                    timezone: None,
                    config: vec![config],
                }),
                ..Default::default()
            };
            assert!(BusinessProfileUpdateSpec::new(&update).is_ok());
        }
    }

    #[test]
    fn set_cover_photo_quotes_the_upload_receipt() {
        let spec = SetCoverPhotoSpec::new(CoverPhotoUpload {
            id: "1234567890".into(),
            token: "dG9rZW4=".into(),
            timestamp: 1_700_000_000,
        });
        let children = mutation_children(&spec.build_iq());

        assert_eq!(children.len(), 1);
        let photo = &children[0];
        assert_eq!(photo.tag.as_ref(), "cover_photo");
        let attr = |k: &str| photo.attrs.get(k).map(|v| v.to_string());
        assert_eq!(attr("op").as_deref(), Some("update"));
        assert_eq!(attr("id").as_deref(), Some("1234567890"));
        assert_eq!(attr("ts").as_deref(), Some("1700000000"));
        assert_eq!(attr("token").as_deref(), Some("dG9rZW4="));
    }

    #[test]
    fn remove_cover_photo_sends_delete_without_a_token() {
        let children = mutation_children(&RemoveCoverPhotoSpec::new("1234567890").build_iq());

        let photo = &children[0];
        assert_eq!(photo.tag.as_ref(), "cover_photo");
        assert_eq!(
            photo.attrs.get("op").map(|v| v.to_string()).as_deref(),
            Some("delete")
        );
        assert_eq!(
            photo.attrs.get("id").map(|v| v.to_string()).as_deref(),
            Some("1234567890")
        );
        assert_eq!(photo.attrs.get("token"), None);
        assert_eq!(photo.attrs.get("ts"), None);
    }
}

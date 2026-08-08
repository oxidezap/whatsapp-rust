//! WhatsApp Business catalog, collections, orders and business-profile writes.
//!
//! Two transports, split the way WhatsApp Web splits them:
//!
//! - **Catalog, collections and order lookup go over MEX** (persisted GraphQL).
//!   WhatsApp Web has no IQ fallback for these — a failed
//!   `xwa_product_catalog_get_product_catalog` raises `CatalogUnknownError`
//!   rather than retrying over `w:biz:catalog`. The only catalog IQ left in the
//!   bundle is `product_list` (a by-id status query) plus `signed_user_info`.
//! - **Business-profile writes stay on IQ** `w:biz`, as a `business_profile`
//!   node with `v="3"` and `mutation_type="delta"`. Those builders live in
//!   [`wacore::iq::business`].
//!
//! Numbers on the MEX side are not JSON numbers. WhatsApp Web stringifies
//! `limit`, `width`, `height`, `collection_limit` and `item_limit`, and sends
//! `allow_shop_source` as `ALLOWSHOPSOURCE_TRUE`/`_FALSE`. The generated
//! `Variables` mirrors in [`wacore::iq::mex_operations`] type these as `i64`
//! and `bool`, so the catalog and collection queries build their variables with
//! `json!` instead; the order query, whose dimensions really are numbers, uses
//! the generated type.
//!
//! **Catalog-side variant data is not requested, and so is never returned.**
//! `variant_info` on a catalog or collection product is opt-in: the server
//! populates it only when the request carries `variant_info_fields`, a
//! comma-separated field list whose full form WhatsApp Web spells
//! `"listing_details,types,availability,variant_properties"`
//! (`WAWebCatalogVariantHelper.FULL_VARIANT_INFO_FIELDS`, alongside a
//! `VARIANT_THUMBNAIL_IMAGE_SIZE` of 100 for the `variant_thumbnail_*`
//! dimensions). WA Web itself only asks for it when `shouldRequestVariantInfo`
//! passes — a linked catalog with variant viewing enabled. Adding it here means
//! sending that field list *and* modelling the option types, availability
//! listings and per-variant pricing it returns; requesting without parsing, or
//! parsing without requesting, would both be dead weight. Orders are the
//! opposite case and are handled: `variant_info.variant_properties` comes back
//! on a plain order query, so [`OrderProduct::variant_properties`] carries it.

use crate::client::Client;
use crate::features::mex::{MexError, mex_request};
use crate::request::IqError;
use serde_json::{Value, json};
use thiserror::Error;
use wacore::WireEnum;
use wacore::iq::business::{BusinessProfileUpdateSpec, RemoveCoverPhotoSpec, SetCoverPhotoSpec};
use wacore::iq::mex_operations::{biz_query_order, query_catalog, query_product_collections};
use wacore_binary::Jid;

// The profile types are protocol types and live in wacore, but they are the
// arguments, results and error of this feature's public methods, so they belong
// on the feature's public surface rather than making callers reach into wacore.
// That includes the types nested inside `BusinessProfile` — naming its
// `business_hours` or `categories` in a signature must not require the
// dependency either.
pub use wacore::iq::business::{
    BUSINESS_PROFILE_MAX_WEBSITES, BusinessCategory, BusinessHourMode, BusinessHours,
    BusinessHoursConfig, BusinessHoursUpdate, BusinessProfile, BusinessProfileUpdate,
    BusinessProfileUpdateError, CoverPhotoUpload, DayOfWeek,
};

// The three defaults below are this client's choice, not values read off
// WhatsApp Web. WA Web computes each of them at the call site from the surface
// it is rendering into, and there is no A/B prop or bundle constant behind
// them, so a headless client has nothing to copy. Override them per request
// via CatalogOptions / CollectionOptions.

/// Thumbnail edge, in pixels, requested for catalog and collection images.
const DEFAULT_IMAGE_DIMENSION: u32 = 100;

/// Products per catalog page.
const DEFAULT_CATALOG_LIMIT: u32 = 10;

/// Collections per page, and products within each. Collections carry their
/// items inline, so the two limits multiply into one response.
const DEFAULT_COLLECTION_LIMIT: u32 = 51;

#[derive(Debug, Error)]
#[non_exhaustive]
pub enum BusinessError {
    #[error("MEX request failed")]
    Mex(#[from] MexError),

    #[error("IQ request failed")]
    Request(#[from] IqError),

    #[error("invalid business profile update")]
    InvalidUpdate(#[from] BusinessProfileUpdateError),

    /// The response was structurally valid JSON but missing a field the
    /// operation is defined by, or carrying it with the wrong shape.
    #[error("malformed {operation} response: {detail}")]
    MalformedResponse {
        operation: &'static str,
        detail: String,
    },
}

impl BusinessError {
    fn malformed(operation: &'static str, detail: impl Into<String>) -> Self {
        Self::MalformedResponse {
            operation,
            detail: detail.into(),
        }
    }
}

/// Whether a product can currently be bought.
///
/// The wire values are the GraphQL enum names; WhatsApp Web maps anything it
/// does not recognise to "unknown" rather than failing the whole catalog.
#[derive(Debug, Clone, PartialEq, Eq, WireEnum)]
#[non_exhaustive]
pub enum ProductAvailability {
    #[wire = "IN_STOCK"]
    InStock,
    #[wire = "OUT_OF_STOCK"]
    OutOfStock,
    #[wire = "AVAILABLE_FOR_ANOTHER_POSTCODE"]
    AvailableForAnotherPostcode,
    #[wire_fallback]
    Other(String),
}

/// A price, in thousandths of the currency's main unit.
///
/// WhatsApp represents money as an integer scaled by 1000 — WhatsApp Web calls
/// it `priceAmount1000` and formats it through `formatAmount1000`, and the
/// protobuf `ProductSnapshot.priceAmount1000` is an `int64`. Keeping the raw
/// integer means no rounding: a price is exact or it is absent. Dividing by
/// 1000 into a float is the caller's decision, at the point of display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Price {
    /// Thousandths of one currency unit: `1_990` is 1.99 in `currency`.
    pub amount_1000: i64,
    /// ISO 4217 code. Absent when the server sends a price without one.
    pub currency: Option<String>,
}

/// A sale price and the window it applies to.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct SalePrice {
    pub price: Price,
    /// Both ends or neither: WhatsApp Web only treats the window as set when
    /// both are present, and the parser normalises a lone endpoint away rather
    /// than surfacing a period the official client does not honour.
    pub start_date: Option<String>,
    pub end_date: Option<String>,
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ProductImage {
    pub id: Option<String>,
    /// URL at the dimensions the request asked for.
    pub request_image_url: Option<String>,
    /// URL at the image's stored dimensions.
    pub original_image_url: Option<String>,
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ProductVideo {
    pub id: Option<String>,
    pub original_video_url: Option<String>,
    pub thumbnail_url: Option<String>,
}

/// A catalog product.
///
/// Only `id` is guaranteed. Every other field is optional because the server
/// omits rather than blanks: an absent `name` is `None`, never `""`, and an
/// absent `is_hidden` is `None`, never `false`.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Product {
    pub id: String,
    /// The merchant's own SKU, distinct from `id`.
    pub retailer_id: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub url: Option<String>,
    /// The link-shimmed form of `url`, sent alongside it rather than instead of
    /// it. Both are surfaced as received: which one to open is a policy call
    /// (the shim adds interstitial handling), and WhatsApp Web picks per
    /// surface, so choosing one here would bake in a preference the wire does
    /// not express.
    pub shimmed_url: Option<String>,
    pub price: Option<Price>,
    pub sale_price: Option<SalePrice>,
    /// Hidden products stay in the catalog but are not shown to customers.
    pub is_hidden: Option<bool>,
    pub is_sanctioned: Option<bool>,
    /// Upper bound on the quantity a single order may contain.
    pub max_available: Option<i64>,
    pub availability: Option<ProductAvailability>,
    /// WhatsApp's review verdict, e.g. `APPROVED`.
    pub review_status: Option<String>,
    pub can_appeal: Option<bool>,
    /// Whether the product belongs to the queried business's own catalog.
    pub belongs_to: Option<bool>,
    pub images: Vec<ProductImage>,
    pub videos: Vec<ProductVideo>,
    pub compliance_category: Option<String>,
    pub country_code_origin: Option<String>,
    /// Importer of record, sent alongside [`Product::importer_address`] for
    /// products carrying an importer disclosure.
    pub importer_name: Option<String>,
    /// Importer's address. Some markets require this to be shown next to the
    /// product, so it is surfaced whole rather than reduced to a country code.
    pub importer_address: Option<ImporterAddress>,
}

/// A postal address, as sent for a product's importer of record.
///
/// Every part is optional: the server omits what it does not hold rather than
/// sending an empty string.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ImporterAddress {
    pub street1: Option<String>,
    pub street2: Option<String>,
    pub city: Option<String>,
    pub region: Option<String>,
    pub postal_code: Option<String>,
    pub country_code: Option<String>,
}

/// One page of a business catalog.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Catalog {
    pub products: Vec<Product>,
    /// Cursor for the next page; `None` when this is the last page. Pass it
    /// back as [`CatalogOptions::after`].
    pub after_cursor: Option<String>,
    /// Cursor for the previous page.
    pub before_cursor: Option<String>,
}

/// A named group of products within a catalog.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Collection {
    pub id: Option<String>,
    pub name: Option<String>,
    /// The first [`CollectionOptions::item_limit`] products, inline.
    ///
    /// **This list can be truncated and says so only by its length.** The
    /// collections query returns a prefix and carries no per-collection cursor,
    /// so `products.len() == item_limit` is the signal that more may exist.
    /// Reading a collection to the end needs a different operation —
    /// `WAWebQueryProductSingleCollectionQuery`, which takes its own `after`
    /// cursor and is vendored but not yet wired up here.
    pub products: Vec<Product>,
    pub review_status: Option<String>,
    /// Whether a rejected collection can still be appealed.
    pub can_appeal: Option<bool>,
    /// Why the collection was rejected. Present only on a rejection, and
    /// carried by collections alone — a product's `status_info` has no
    /// equivalent, which is why [`Product`] exposes no counterpart.
    pub reject_reason: Option<String>,
    /// Where to review or appeal the decision.
    pub commerce_url: Option<String>,
}

/// One page of a business's collections.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Collections {
    pub collections: Vec<Collection>,
    /// Cursor for the next page; `None` on the last page. Pass it back as
    /// [`CollectionOptions::after`].
    ///
    /// Unlike the catalog, this response carries a forward cursor only — there
    /// is no `before` in the collections paging object.
    pub after_cursor: Option<String>,
}

/// A line item on an order, which is a snapshot rather than a live product:
/// the price is what was quoted when the order was placed.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct OrderProduct {
    pub id: Option<String>,
    pub name: Option<String>,
    pub price: Option<Price>,
    pub quantity: Option<i64>,
    pub images: Vec<ProductImage>,
    /// The variant the customer actually chose — size, colour and the like.
    /// Empty for a product with no variants. Without it an order for a
    /// variant product cannot be fulfilled: `id`, `name` and `price` are the
    /// same across the variants of one listing.
    pub variant_properties: Vec<VariantProperty>,
}

/// One dimension of a chosen product variant, e.g. `name: "Size"`,
/// `value: "Large"`.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct VariantProperty {
    pub name: Option<String>,
    pub value: Option<String>,
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct OrderPriceDetails {
    pub currency: Option<String>,
    pub subtotal: Option<Price>,
    pub total: Option<Price>,
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct Order {
    pub products: Vec<OrderProduct>,
    pub price_details: Option<OrderPriceDetails>,
    /// Unix seconds, as sent. Absent when the server omits it.
    pub creation_timestamp: Option<i64>,
}

/// Paging and thumbnail options for [`Business::get_catalog`].
#[derive(Debug, Clone)]
pub struct CatalogOptions {
    /// Products per page.
    pub limit: u32,
    /// Cursor from a previous page's [`Catalog::after_cursor`].
    pub after: Option<String>,
    pub image_width: u32,
    pub image_height: u32,
    /// Lets the server include products surfaced through Shop as well as the
    /// business's own catalog.
    pub allow_shop_source: bool,
}

impl Default for CatalogOptions {
    fn default() -> Self {
        Self {
            limit: DEFAULT_CATALOG_LIMIT,
            after: None,
            image_width: DEFAULT_IMAGE_DIMENSION,
            image_height: DEFAULT_IMAGE_DIMENSION,
            allow_shop_source: true,
        }
    }
}

/// Options for [`Business::get_collections`].
#[derive(Debug, Clone)]
pub struct CollectionOptions {
    /// Collections per page.
    pub collection_limit: u32,
    /// Products returned inline per collection.
    pub item_limit: u32,
    pub after: Option<String>,
    pub image_width: u32,
    pub image_height: u32,
}

impl Default for CollectionOptions {
    fn default() -> Self {
        Self {
            collection_limit: DEFAULT_COLLECTION_LIMIT,
            item_limit: DEFAULT_COLLECTION_LIMIT,
            after: None,
            image_width: DEFAULT_IMAGE_DIMENSION,
            image_height: DEFAULT_IMAGE_DIMENSION,
        }
    }
}

/// Feature handle for business catalog and profile operations.
pub struct Business<'a> {
    client: &'a Client,
}

impl Client {
    #[inline]
    pub fn business(&self) -> Business<'_> {
        Business::new(self)
    }
}

impl<'a> Business<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Fetch one page of a business's product catalog.
    ///
    /// Paginate by feeding [`Catalog::after_cursor`] back through
    /// [`CatalogOptions::after`] until it comes back `None`.
    pub async fn get_catalog(
        &self,
        jid: &Jid,
        options: &CatalogOptions,
    ) -> Result<Catalog, BusinessError> {
        let response = self
            .client
            .mex()
            .query(mex_request!(query_catalog, catalog_variables(jid, options)))
            .await?;

        parse_catalog(&response_data(response.data, "catalog")?)
    }

    /// Fetch one page of a business's product collections, each with its
    /// products inline.
    ///
    /// Paginate by feeding [`Collections::after_cursor`] back through
    /// [`CollectionOptions::after`] until it comes back `None`.
    pub async fn get_collections(
        &self,
        jid: &Jid,
        options: &CollectionOptions,
    ) -> Result<Collections, BusinessError> {
        let response = self
            .client
            .mex()
            .query(mex_request!(
                query_product_collections,
                collection_variables(jid, options)
            ))
            .await?;

        parse_collections(&response_data(response.data, "collections")?)
    }

    /// Look up an order's line items and totals.
    ///
    /// Both `order_id` and `token` come from the order message itself
    /// (`OrderMessage.orderId` and `OrderMessage.token`); the token is a
    /// per-order capability, so an order cannot be read without the message
    /// that announced it. `jid` is the business the order was placed with.
    pub async fn get_order(
        &self,
        jid: &Jid,
        order_id: &str,
        token: &str,
    ) -> Result<Order, BusinessError> {
        let response = self
            .client
            .mex()
            .query(mex_request!(
                biz_query_order,
                order_variables(jid, order_id, token)
            ))
            .await?;

        parse_order(&response_data(response.data, "order")?)
    }

    /// Apply a delta to the authenticated account's own business profile.
    ///
    /// Fields left `None` are untouched; see [`BusinessProfileUpdate`] for how
    /// to clear one instead.
    pub async fn update_profile(
        &self,
        update: &BusinessProfileUpdate,
    ) -> Result<(), BusinessError> {
        let spec = BusinessProfileUpdateSpec::new(update)?;
        self.client.execute(spec).await?;
        Ok(())
    }

    /// Point the business profile at an already-uploaded cover photo.
    ///
    /// The [`CoverPhotoUpload`] receipt comes from a `biz-cover-photo` media
    /// upload, which this crate does not yet perform — see the type's docs.
    pub async fn set_cover_photo(&self, upload: CoverPhotoUpload) -> Result<(), BusinessError> {
        self.client.execute(SetCoverPhotoSpec::new(upload)).await?;
        Ok(())
    }

    /// Remove the business profile's cover photo, by the `fbid` it was set with.
    pub async fn remove_cover_photo(&self, id: &str) -> Result<(), BusinessError> {
        self.client.execute(RemoveCoverPhotoSpec::new(id)).await?;
        Ok(())
    }
}

fn response_data(data: Option<Value>, operation: &'static str) -> Result<Value, BusinessError> {
    data.ok_or_else(|| BusinessError::malformed(operation, "response carried no data"))
}

/// Variables for `WAWebQueryCatalogQuery`.
///
/// `limit`, `width` and `height` are stringified and `allow_shop_source` is an
/// enum spelling, matching WhatsApp Web. The generated `Variables` mirror types
/// them as `i64`/`bool`, which is why this is hand-built JSON.
fn catalog_variables(jid: &Jid, options: &CatalogOptions) -> Value {
    let mut product_catalog = json!({
        "jid": jid.to_string(),
        "limit": options.limit.to_string(),
        "width": options.image_width.to_string(),
        "height": options.image_height.to_string(),
        "allow_shop_source": if options.allow_shop_source {
            "ALLOWSHOPSOURCE_TRUE"
        } else {
            "ALLOWSHOPSOURCE_FALSE"
        },
    });
    // Omitted entirely on the first page: an explicit null is a cursor value.
    if let Some(after) = &options.after {
        product_catalog["after"] = json!(after);
    }
    json!({ "request": { "product_catalog": product_catalog } })
}

/// Variables for `WAWebQueryProductCollectionsQuery`. Same stringified-number
/// convention as the catalog query.
fn collection_variables(jid: &Jid, options: &CollectionOptions) -> Value {
    let mut collections = json!({
        "biz_jid": jid.to_string(),
        "collection_limit": options.collection_limit.to_string(),
        "item_limit": options.item_limit.to_string(),
        "width": options.image_width.to_string(),
        "height": options.image_height.to_string(),
    });
    if let Some(after) = &options.after {
        collections["after"] = json!(after);
    }
    json!({ "request": { "collections": collections } })
}

/// Variables for `WAWebBizQueryOrderJobQuery`. This one uses the generated type:
/// unlike the catalog query, its image dimensions really are JSON numbers.
fn order_variables(jid: &Jid, order_id: &str, token: &str) -> biz_query_order::Variables {
    biz_query_order::Variables {
        request: Some(biz_query_order::Request {
            order: Some(biz_query_order::Order {
                id: Some(order_id.to_string()),
                jid: Some(jid.to_string()),
                token: Some(biz_query_order::Token {
                    sensitive_string_value: Some(token.to_string()),
                }),
                image_dimensions: Some(biz_query_order::ImageDimensions {
                    width: Some(DEFAULT_IMAGE_DIMENSION.into()),
                    height: Some(DEFAULT_IMAGE_DIMENSION.into()),
                }),
                ..Default::default()
            }),
        }),
    }
}

/// Reads WhatsApp's scaled-integer money fields.
///
/// The GraphQL scalar is a decimal string, but WhatsApp Web tolerates a bare
/// JSON number in the same position, so both are accepted. An empty string is
/// "no price", not zero. Parsing into `i64` rather than a float is the point:
/// at 1/1000 granularity a large total loses cents in `f64`.
fn parse_amount_1000(value: &Value) -> Option<i64> {
    match value {
        Value::String(s) if s.is_empty() => None,
        Value::String(s) => s.parse::<i64>().ok(),
        Value::Number(n) => n.as_i64(),
        _ => None,
    }
}

fn parse_price(value: &Value, currency: Option<&str>) -> Option<Price> {
    Some(Price {
        amount_1000: parse_amount_1000(value)?,
        currency: currency.map(str::to_string),
    })
}

fn opt_str(value: &Value) -> Option<String> {
    match value {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        _ => None,
    }
}

/// Booleans arrive as JSON booleans or as strings, and `is_hidden` uses its own
/// enum spelling (`ISHIDDEN_TRUE`) rather than `"true"`. Anything unrecognised
/// stays `None` so an absent flag is never reported as `false`.
fn opt_bool(value: &Value) -> Option<bool> {
    match value {
        Value::Bool(b) => Some(*b),
        Value::String(s) if s.is_empty() => None,
        Value::String(s) => match s.as_str() {
            "true" | "ISHIDDEN_TRUE" => Some(true),
            "false" | "ISHIDDEN_FALSE" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

/// Reads `variant_info.variant_properties`. A product with no variants omits
/// the object entirely, which is an empty list rather than an error — but a
/// field that is present and not an array is malformed, and reporting it as
/// variant-free would hide an order that cannot be fulfilled.
fn parse_variant_properties(
    operation: &'static str,
    variant_info: &Value,
) -> Result<Vec<VariantProperty>, BusinessError> {
    if variant_info.is_null() {
        return Ok(Vec::new());
    }
    // Indexing a non-object yields Null, so without this the corrupt case would
    // be indistinguishable from a product that simply has no variants.
    if !variant_info.is_object() {
        return Err(BusinessError::malformed(
            operation,
            "variant_info is not an object",
        ));
    }
    Ok(
        object_entries(operation, variant_info, "variant_properties")?
            .into_iter()
            .map(|property| VariantProperty {
                name: opt_str(&property["name"]),
                value: opt_str(&property["value"]),
            })
            .collect(),
    )
}

fn opt_i64(value: &Value) -> Option<i64> {
    match value {
        Value::Number(n) => n.as_i64(),
        Value::String(s) if s.is_empty() => None,
        Value::String(s) => s.parse::<i64>().ok(),
        _ => None,
    }
}

fn parse_images(
    operation: &'static str,
    media: &Value,
) -> Result<Vec<ProductImage>, BusinessError> {
    Ok(object_entries(operation, media, "images")?
        .into_iter()
        .map(|image| ProductImage {
            id: opt_str(&image["id"]),
            request_image_url: opt_str(&image["request_image_url"]),
            original_image_url: opt_str(&image["original_image_url"]),
        })
        .collect())
}

fn parse_videos(
    operation: &'static str,
    media: &Value,
) -> Result<Vec<ProductVideo>, BusinessError> {
    Ok(object_entries(operation, media, "videos")?
        .into_iter()
        .map(|video| ProductVideo {
            id: opt_str(&video["id"]),
            original_video_url: opt_str(&video["original_video_url"]),
            thumbnail_url: opt_str(&video["thumbnail_url"]),
        })
        .collect())
}

fn parse_product(value: &Value, operation: &'static str) -> Result<Product, BusinessError> {
    // `id` is the one field WhatsApp Web asserts on; without it the entry
    // cannot be referenced, ordered, or deduplicated, so it is an error rather
    // than a skipped product.
    let id = opt_str(&value["id"])
        .ok_or_else(|| BusinessError::malformed(operation, "product entry has no id"))?;

    let currency = opt_str(&value["currency"]);
    let compliance = object(operation, value, "compliance_info")?;
    let status = object(operation, value, "status_info")?;
    let media = object(operation, value, "media")?;
    let sale = object(operation, value, "sale_price")?;
    // The window is both dates or neither, the way WhatsApp Web reads it:
    // `start_date != null && end_date != null ? {...} : null` on the MEX path,
    // and `hasChild("start_date") && hasChild("end_date")` on the IQ one. A lone
    // endpoint is not a period, and surfacing one would invite a caller to
    // schedule a promotion the official client does not consider scheduled.
    let window = opt_str(&sale["start_date"]).zip(opt_str(&sale["end_date"]));
    let sale_price = parse_price(&sale["price"], currency.as_deref()).map(|price| {
        let (start_date, end_date) = match window {
            Some((start, end)) => (Some(start), Some(end)),
            None => (None, None),
        };
        SalePrice {
            price,
            start_date,
            end_date,
        }
    });

    Ok(Product {
        id,
        retailer_id: opt_str(&value["retailer_id"]),
        name: opt_str(&value["name"]),
        description: opt_str(&value["description"]),
        url: opt_str(&value["url"]),
        shimmed_url: opt_str(&value["shimmed_url"]),
        price: parse_price(&value["price"], currency.as_deref()),
        sale_price,
        is_hidden: opt_bool(&value["is_hidden"]),
        is_sanctioned: opt_bool(&value["is_sanctioned"]),
        max_available: opt_i64(&value["max_available"]),
        availability: opt_str(&value["product_availability"])
            .map(|a| ProductAvailability::from(a.as_str())),
        review_status: opt_str(&status["status"]),
        can_appeal: opt_bool(&status["can_appeal"]),
        belongs_to: opt_bool(&value["belongs_to"]),
        images: parse_images(operation, media)?,
        videos: parse_videos(operation, media)?,
        compliance_category: opt_str(&value["compliance_category"]),
        country_code_origin: opt_str(&compliance["country_code_origin"]),
        importer_name: opt_str(&compliance["importer_name"]),
        importer_address: parse_importer_address(object(
            operation,
            compliance,
            "importer_address",
        )?),
    })
}

/// A nested object, borrowed for reading.
///
/// Absent is fine and reads as an object with nothing in it; a *present*
/// non-object is not. This distinction has to be drawn explicitly everywhere,
/// because indexing a scalar yields `Null` for every child — so without the
/// check, malformed data is indistinguishable from absent data and degrades
/// into plausible-looking empty values instead of an error.
fn object<'a>(
    operation: &'static str,
    parent: &'a Value,
    field: &'static str,
) -> Result<&'a Value, BusinessError> {
    let value = &parent[field];
    if value.is_null() || value.is_object() {
        Ok(value)
    } else {
        Err(BusinessError::malformed(
            operation,
            format!("{field} is not an object"),
        ))
    }
}

/// The entries of an array of objects. Absent is an empty list; a present
/// non-array, or an entry that is not an object, is malformed.
fn object_entries<'a>(
    operation: &'static str,
    parent: &'a Value,
    field: &'static str,
) -> Result<Vec<&'a Value>, BusinessError> {
    let value = &parent[field];
    if value.is_null() {
        return Ok(Vec::new());
    }
    let entries = value
        .as_array()
        .ok_or_else(|| BusinessError::malformed(operation, format!("{field} is not an array")))?;
    if entries.iter().any(|entry| !entry.is_object()) {
        return Err(BusinessError::malformed(
            operation,
            format!("{field} has an entry that is not an object"),
        ));
    }
    Ok(entries.iter().collect())
}

/// Reads `compliance_info.importer_address`. Present with every part missing is
/// `None` — an address with nothing in it is not an address.
fn parse_importer_address(value: &Value) -> Option<ImporterAddress> {
    let address = ImporterAddress {
        street1: opt_str(&value["street1"]),
        street2: opt_str(&value["street2"]),
        city: opt_str(&value["city"]),
        region: opt_str(&value["region"]),
        postal_code: opt_str(&value["postal_code"]),
        country_code: opt_str(&value["country_code"]),
    };
    let empty = address.street1.is_none()
        && address.street2.is_none()
        && address.city.is_none()
        && address.region.is_none()
        && address.postal_code.is_none()
        && address.country_code.is_none();
    (!empty).then_some(address)
}

fn parse_catalog(data: &Value) -> Result<Catalog, BusinessError> {
    const OP: &str = "catalog";

    let catalog = &data["xwa_product_catalog_get_product_catalog"]["product_catalog"];
    if !catalog.is_object() {
        return Err(BusinessError::malformed(
            OP,
            "missing xwa_product_catalog_get_product_catalog.product_catalog",
        ));
    }

    let products = catalog["products"]
        .as_array()
        .ok_or_else(|| BusinessError::malformed(OP, "product_catalog.products is not an array"))?
        .iter()
        .map(|p| parse_product(p, OP))
        .collect::<Result<Vec<_>, _>>()?;

    // A malformed paging object would otherwise read as "no more pages" and
    // silently truncate the catalog at whatever page it appeared on.
    let paging = object(OP, catalog, "paging")?;

    Ok(Catalog {
        products,
        after_cursor: opt_str(&paging["after"]),
        before_cursor: opt_str(&paging["before"]),
    })
}

fn parse_collections(data: &Value) -> Result<Collections, BusinessError> {
    const OP: &str = "collections";

    let root = &data["xwa_product_catalog_get_collections"];
    let collections = root["collections"].as_array().ok_or_else(|| {
        BusinessError::malformed(
            OP,
            "missing xwa_product_catalog_get_collections.collections array",
        )
    })?;

    let collections = collections
        .iter()
        .map(|collection| {
            // Indexing a non-object yields Null for every field, which would
            // turn a malformed entry into a plausible-looking nameless
            // collection instead of surfacing the bad response.
            if !collection.is_object() {
                return Err(BusinessError::malformed(
                    OP,
                    "collection entry is not an object",
                ));
            }

            // A collection with no `products` key is legitimately empty, but a
            // `products` that is present and not an array is malformed — the
            // same distinction the catalog path makes.
            let products = match &collection["products"] {
                Value::Null => Vec::new(),
                Value::Array(products) => products
                    .iter()
                    .map(|p| parse_product(p, OP))
                    .collect::<Result<Vec<_>, _>>()?,
                _ => {
                    return Err(BusinessError::malformed(
                        OP,
                        "collection.products is not an array",
                    ));
                }
            };

            let status = object(OP, collection, "status_info")?;
            Ok(Collection {
                id: opt_str(&collection["id"]),
                name: opt_str(&collection["name"]),
                products,
                review_status: opt_str(&status["status"]),
                can_appeal: opt_bool(&status["can_appeal"]),
                reject_reason: opt_str(&status["reject_reason"]),
                commerce_url: opt_str(&status["commerce_url"]),
            })
        })
        .collect::<Result<Vec<_>, BusinessError>>()?;

    Ok(Collections {
        collections,
        after_cursor: opt_str(&object(OP, root, "paging")?["after"]),
    })
}

fn parse_order(data: &Value) -> Result<Order, BusinessError> {
    const OP: &str = "order";

    let order = &data["xwa_checkout_get_order_info"]["order"];
    if !order.is_object() {
        return Err(BusinessError::malformed(
            OP,
            "missing xwa_checkout_get_order_info.order",
        ));
    }

    let products = order["products"]
        .as_array()
        .ok_or_else(|| BusinessError::malformed(OP, "order.products is not an array"))?
        .iter()
        .map(|product| {
            if !product.is_object() {
                return Err(BusinessError::malformed(
                    OP,
                    "order.products entry is not an object",
                ));
            }
            let currency = opt_str(&product["currency"]);
            Ok(OrderProduct {
                id: opt_str(&product["id"]),
                name: opt_str(&product["name"]),
                price: parse_price(&product["price"], currency.as_deref()),
                quantity: opt_i64(&product["quantity"]),
                images: parse_images(OP, object(OP, product, "media")?)?,
                variant_properties: parse_variant_properties(
                    OP,
                    object(OP, product, "variant_info")?,
                )?,
            })
        })
        .collect::<Result<Vec<_>, BusinessError>>()?;

    let details = object(OP, order, "price_details")?;
    let price_details = details.is_object().then(|| {
        let currency = opt_str(&details["currency"]);
        OrderPriceDetails {
            subtotal: parse_price(&details["subtotal_amount"], currency.as_deref()),
            total: parse_price(&details["total_amount"], currency.as_deref()),
            currency,
        }
    });

    Ok(Order {
        products,
        price_details,
        creation_timestamp: opt_i64(&order["creation_time_stamp"]),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // Fixtures below are hand-written from the response shapes WhatsApp Web
    // reads, with fictitious ids, names and JIDs.

    fn biz_jid() -> Jid {
        "12025550111@s.whatsapp.net".parse().unwrap()
    }

    /// Every numeric catalog variable is a *string* on the wire. Sending JSON
    /// numbers here is the mistake the generated mirror types invite.
    #[test]
    fn catalog_request_stringifies_numbers_and_enumerates_shop_source() {
        let vars = catalog_variables(&biz_jid(), &CatalogOptions::default());
        let request = &vars["request"]["product_catalog"];

        assert_eq!(request["jid"], json!("12025550111@s.whatsapp.net"));
        assert_eq!(request["limit"], json!("10"));
        assert_eq!(request["width"], json!("100"));
        assert_eq!(request["height"], json!("100"));
        assert_eq!(request["allow_shop_source"], json!("ALLOWSHOPSOURCE_TRUE"));
        assert!(request["limit"].is_string());
        assert!(request["width"].is_string());
        // No cursor on the first page — not even a null.
        assert!(request.get("after").is_none());
    }

    #[test]
    fn catalog_request_carries_the_paging_cursor() {
        let options = CatalogOptions {
            limit: 25,
            after: Some("cursor-page-2".into()),
            allow_shop_source: false,
            ..Default::default()
        };
        let vars = catalog_variables(&biz_jid(), &options);
        let request = &vars["request"]["product_catalog"];

        assert_eq!(request["after"], json!("cursor-page-2"));
        assert_eq!(request["limit"], json!("25"));
        assert_eq!(request["allow_shop_source"], json!("ALLOWSHOPSOURCE_FALSE"));
    }

    #[test]
    fn collection_request_uses_biz_jid_and_two_limits() {
        let vars = collection_variables(&biz_jid(), &CollectionOptions::default());
        let request = &vars["request"]["collections"];

        // The collections query keys the business on `biz_jid`, not `jid`.
        assert_eq!(request["biz_jid"], json!("12025550111@s.whatsapp.net"));
        assert!(request.get("jid").is_none());
        assert_eq!(request["collection_limit"], json!("51"));
        assert_eq!(request["item_limit"], json!("51"));
        assert_eq!(request["width"], json!("100"));
    }

    /// The order query is the one that sends real JSON numbers, and it wraps
    /// the token in a `sensitive_string_value` object.
    #[test]
    fn order_request_wraps_the_token_and_keeps_numeric_dimensions() {
        let vars = order_variables(&biz_jid(), "order-1", "b3JkZXItdG9rZW4=");
        let value = serde_json::to_value(&vars).unwrap();
        let request = &value["request"]["order"];

        assert_eq!(request["id"], json!("order-1"));
        assert_eq!(request["jid"], json!("12025550111@s.whatsapp.net"));
        assert_eq!(
            request["token"]["sensitive_string_value"],
            json!("b3JkZXItdG9rZW4=")
        );
        assert_eq!(request["image_dimensions"]["width"], json!(100));
        assert!(request["image_dimensions"]["width"].is_number());
    }

    fn catalog_response() -> Value {
        json!({
            "xwa_product_catalog_get_product_catalog": {
                "product_catalog": {
                    "paging": { "after": "cursor-page-2", "before": "" },
                    "products": [{
                        "id": "1000000000000001",
                        "retailer_id": "SKU-RED-01",
                        "name": "Red Notebook",
                        "description": "A5, dotted",
                        "url": "https://shop.example.invalid/red-notebook",
                        "currency": "BRL",
                        "price": "12990",
                        "sale_price": { "price": "9990", "start_date": "1700000000", "end_date": "1700600000" },
                        "is_hidden": "ISHIDDEN_FALSE",
                        "is_sanctioned": false,
                        "max_available": "25",
                        "product_availability": "IN_STOCK",
                        "belongs_to": "true",
                        "status_info": { "status": "APPROVED", "can_appeal": "false" },
                        "compliance_category": "COUNTRY_ORIGIN_EXEMPT",
                        "compliance_info": { "country_code_origin": "BR" },
                        "media": {
                            "images": [
                                { "id": "img-1", "request_image_url": "https://cdn.example.invalid/1-100.jpg", "original_image_url": "https://cdn.example.invalid/1.jpg" },
                                { "id": "img-2", "request_image_url": "https://cdn.example.invalid/2-100.jpg", "original_image_url": "https://cdn.example.invalid/2.jpg" }
                            ],
                            "videos": [
                                { "id": "vid-1", "original_video_url": "https://cdn.example.invalid/1.mp4", "thumbnail_url": "https://cdn.example.invalid/1.jpg" }
                            ]
                        }
                    }]
                }
            }
        })
    }

    /// Importer disclosures arrive unconditionally — there is no opt-in field
    /// for them the way there is for variant data — and some markets require
    /// them shown, so the whole address is kept rather than the origin alone.
    #[test]
    fn products_keep_the_importer_disclosure() {
        let response = json!({
            "xwa_product_catalog_get_product_catalog": {
                "product_catalog": { "products": [{
                    "id": "1000000000000001",
                    "compliance_info": {
                        "country_code_origin": "CN",
                        "importer_name": "Example Imports Ltda",
                        "importer_address": {
                            "street1": "Rua das Flores 100",
                            "city": "São Paulo",
                            "region": "SP",
                            "postal_code": "01000-000",
                            "country_code": "BR"
                        }
                    }
                }] }
            }
        });

        let product = &parse_catalog(&response).unwrap().products[0];
        assert_eq!(product.country_code_origin.as_deref(), Some("CN"));
        assert_eq!(
            product.importer_name.as_deref(),
            Some("Example Imports Ltda")
        );
        let address = product.importer_address.as_ref().unwrap();
        assert_eq!(address.city.as_deref(), Some("São Paulo"));
        assert_eq!(address.postal_code.as_deref(), Some("01000-000"));
        // Absent parts stay absent rather than becoming empty strings.
        assert_eq!(address.street2, None);
    }

    /// An origin-only disclosure carries no address, and an address with
    /// nothing in it is not an address.
    #[test]
    fn absent_importer_address_is_none() {
        let response = json!({
            "xwa_product_catalog_get_product_catalog": {
                "product_catalog": { "products": [{
                    "id": "1000000000000001",
                    "compliance_info": { "country_code_origin": "BR" }
                }] }
            }
        });

        let product = &parse_catalog(&response).unwrap().products[0];
        assert_eq!(product.country_code_origin.as_deref(), Some("BR"));
        assert_eq!(product.importer_name, None);
        assert_eq!(product.importer_address, None);

        // A non-object address indexes to Null on every part, so without its own
        // check it would be indistinguishable from a product with no importer.
        assert!(
            parse_catalog(&json!({
                "xwa_product_catalog_get_product_catalog": {
                    "product_catalog": { "products": [{
                        "id": "1000000000000001",
                        "compliance_info": { "importer_address": "nope" }
                    }] }
                }
            }))
            .is_err()
        );
    }

    #[test]
    fn parses_a_catalog_page() {
        let catalog = parse_catalog(&catalog_response()).unwrap();

        assert_eq!(catalog.after_cursor.as_deref(), Some("cursor-page-2"));
        // An empty cursor string is "no previous page", not a cursor of "".
        assert_eq!(catalog.before_cursor, None);

        let product = &catalog.products[0];
        assert_eq!(product.id, "1000000000000001");
        assert_eq!(product.name.as_deref(), Some("Red Notebook"));
        assert_eq!(product.retailer_id.as_deref(), Some("SKU-RED-01"));
        assert_eq!(
            product.price,
            Some(Price {
                amount_1000: 12990,
                currency: Some("BRL".into())
            })
        );
        assert_eq!(product.availability, Some(ProductAvailability::InStock));
        assert_eq!(product.max_available, Some(25));
        assert_eq!(product.images.len(), 2);
        assert_eq!(product.videos.len(), 1);
        assert_eq!(product.country_code_origin.as_deref(), Some("BR"));
        assert_eq!(product.review_status.as_deref(), Some("APPROVED"));

        let sale = product.sale_price.as_ref().unwrap();
        assert_eq!(sale.price.amount_1000, 9990);
        assert_eq!(sale.start_date.as_deref(), Some("1700000000"));
    }

    /// `is_hidden` is the string `ISHIDDEN_TRUE`, not a JSON boolean, and
    /// `belongs_to`/`can_appeal` are `"true"`/`"false"` strings.
    #[test]
    fn reads_wapped_boolean_spellings() {
        let catalog = parse_catalog(&catalog_response()).unwrap();
        let product = &catalog.products[0];
        assert_eq!(product.is_hidden, Some(false));
        assert_eq!(product.belongs_to, Some(true));
        assert_eq!(product.can_appeal, Some(false));
        assert_eq!(product.is_sanctioned, Some(false));

        let mut hidden = catalog_response();
        hidden["xwa_product_catalog_get_product_catalog"]["product_catalog"]["products"][0]["is_hidden"] =
            json!("ISHIDDEN_TRUE");
        assert_eq!(
            parse_catalog(&hidden).unwrap().products[0].is_hidden,
            Some(true)
        );
    }

    /// An absent flag must stay unknown; reporting `false` would claim the
    /// product is visible when the server never said so.
    #[test]
    fn absent_fields_stay_none_rather_than_defaulting() {
        let bare = json!({
            "xwa_product_catalog_get_product_catalog": {
                "product_catalog": { "products": [{ "id": "1000000000000002" }] }
            }
        });
        let product = &parse_catalog(&bare).unwrap().products[0];

        assert_eq!(product.name, None);
        assert_eq!(product.price, None);
        assert_eq!(product.is_hidden, None);
        assert_eq!(product.can_appeal, None);
        assert_eq!(product.max_available, None);
        assert_eq!(product.availability, None);
        assert!(product.images.is_empty());
    }

    /// Money must survive a value that `f64` cannot hold exactly. 2^53 + 1
    /// thousandths round-trips through `i64` and would come back as 2^53 if it
    /// ever passed through a float.
    #[test]
    fn large_prices_do_not_lose_precision() {
        const BEYOND_F64: i64 = 9_007_199_254_740_993; // 2^53 + 1

        let mut response = catalog_response();
        response["xwa_product_catalog_get_product_catalog"]["product_catalog"]["products"][0]["price"] =
            json!(BEYOND_F64.to_string());

        let product = &parse_catalog(&response).unwrap().products[0];
        let amount = product.price.as_ref().unwrap().amount_1000;
        assert_eq!(amount, BEYOND_F64);
        assert_ne!(amount, BEYOND_F64 as f64 as i64);
    }

    /// A price whose fractional part is not representable in binary floating
    /// point still reads back exactly.
    #[test]
    fn fractional_prices_are_exact() {
        let mut response = catalog_response();
        response["xwa_product_catalog_get_product_catalog"]["product_catalog"]["products"][0]["price"] =
            json!("10");
        let product = &parse_catalog(&response).unwrap().products[0];
        // 10 thousandths is one cent of a currency with two decimals.
        assert_eq!(product.price.as_ref().unwrap().amount_1000, 10);
    }

    #[test]
    fn empty_price_string_is_absent_not_zero() {
        let mut response = catalog_response();
        response["xwa_product_catalog_get_product_catalog"]["product_catalog"]["products"][0]["price"] =
            json!("");
        assert_eq!(parse_catalog(&response).unwrap().products[0].price, None);
    }

    #[test]
    fn unknown_availability_falls_back_without_failing() {
        let mut response = catalog_response();
        response["xwa_product_catalog_get_product_catalog"]["product_catalog"]["products"][0]["product_availability"] =
            json!("SOMETHING_NEW");
        assert_eq!(
            parse_catalog(&response).unwrap().products[0].availability,
            Some(ProductAvailability::Other("SOMETHING_NEW".into()))
        );
    }

    #[test]
    fn catalog_without_payload_is_an_error() {
        let err = parse_catalog(&json!({})).unwrap_err();
        assert!(matches!(
            err,
            BusinessError::MalformedResponse {
                operation: "catalog",
                ..
            }
        ));
    }

    #[test]
    fn catalog_with_non_array_products_is_an_error() {
        let response = json!({
            "xwa_product_catalog_get_product_catalog": { "product_catalog": { "products": "nope" } }
        });
        assert!(parse_catalog(&response).is_err());
    }

    #[test]
    fn product_without_an_id_is_an_error() {
        let response = json!({
            "xwa_product_catalog_get_product_catalog": {
                "product_catalog": { "products": [{ "name": "Nameless" }] }
            }
        });
        let err = parse_catalog(&response).unwrap_err();
        assert!(matches!(err, BusinessError::MalformedResponse { .. }));
    }

    #[test]
    fn parses_collections_with_inline_products() {
        let response = json!({
            "xwa_product_catalog_get_collections": {
                "paging": { "after": "collections-page-2" },
                "collections": [{
                    "id": "collection-1",
                    "name": "Stationery",
                    "status_info": { "status": "APPROVED" },
                    "products": [{ "id": "1000000000000001", "name": "Red Notebook", "currency": "BRL", "price": "12990" }]
                }, {
                    "id": "collection-2",
                    "name": "Empty"
                }]
            }
        });

        let page = parse_collections(&response).unwrap();
        let collections = &page.collections;
        assert_eq!(collections.len(), 2);
        assert_eq!(collections[0].name.as_deref(), Some("Stationery"));
        assert_eq!(collections[0].review_status.as_deref(), Some("APPROVED"));
        assert_eq!(collections[0].products[0].id, "1000000000000001");
        assert_eq!(
            collections[0].products[0]
                .price
                .as_ref()
                .unwrap()
                .amount_1000,
            12990
        );
        // A collection with no products key is empty, not malformed.
        assert!(collections[1].products.is_empty());
    }

    /// Without the cursor a caller cannot reach page 2, so a business with more
    /// collections than `collection_limit` would silently look complete.
    #[test]
    fn collections_carry_the_continuation_cursor() {
        let response = json!({
            "xwa_product_catalog_get_collections": {
                "paging": { "after": "collections-page-2" },
                "collections": []
            }
        });
        assert_eq!(
            parse_collections(&response)
                .unwrap()
                .after_cursor
                .as_deref(),
            Some("collections-page-2")
        );
    }

    #[test]
    fn collections_last_page_has_no_cursor() {
        // Absent paging object, and an empty cursor string, both mean "no more".
        let absent = json!({ "xwa_product_catalog_get_collections": { "collections": [] } });
        assert_eq!(parse_collections(&absent).unwrap().after_cursor, None);

        let blank = json!({
            "xwa_product_catalog_get_collections": { "paging": { "after": "" }, "collections": [] }
        });
        assert_eq!(parse_collections(&blank).unwrap().after_cursor, None);
    }

    /// A rejection is only actionable with the reason and the appeal route;
    /// `reject_reason` and `commerce_url` exist on a collection's `status_info`
    /// and have no counterpart on a product's.
    #[test]
    fn collections_keep_the_full_moderation_status() {
        let response = json!({
            "xwa_product_catalog_get_collections": {
                "collections": [{
                    "id": "collection-1",
                    "status_info": {
                        "status": "REJECTED",
                        "can_appeal": true,
                        "reject_reason": "PROHIBITED_ITEM",
                        "commerce_url": "https://business.example.invalid/appeal"
                    }
                }]
            }
        });

        let collection = &parse_collections(&response).unwrap().collections[0];
        assert_eq!(collection.review_status.as_deref(), Some("REJECTED"));
        assert_eq!(collection.can_appeal, Some(true));
        assert_eq!(collection.reject_reason.as_deref(), Some("PROHIBITED_ITEM"));
        assert_eq!(
            collection.commerce_url.as_deref(),
            Some("https://business.example.invalid/appeal")
        );
    }

    /// A non-object entry would otherwise index to Null on every field and
    /// surface as a real-looking nameless, empty collection.
    #[test]
    fn non_object_collection_entry_is_an_error() {
        let response = json!({
            "xwa_product_catalog_get_collections": { "collections": ["not-an-object"] }
        });
        assert!(parse_collections(&response).is_err());
    }

    /// Absent `products` is an empty collection, but a present non-array is
    /// malformed — the same distinction the catalog path makes.
    #[test]
    fn non_array_collection_products_is_an_error() {
        let response = json!({
            "xwa_product_catalog_get_collections": {
                "collections": [{ "id": "collection-1", "products": "nope" }]
            }
        });
        assert!(parse_collections(&response).is_err());
    }

    #[test]
    fn collections_without_payload_is_an_error() {
        assert!(parse_collections(&json!({})).is_err());
        assert!(parse_collections(&json!({ "xwa_product_catalog_get_collections": {} })).is_err());
    }

    #[test]
    fn parses_an_order() {
        let response = json!({
            "xwa_checkout_get_order_info": {
                "order": {
                    "creation_time_stamp": "1700000000",
                    "price_details": {
                        "currency": "BRL",
                        "subtotal_amount": "25980",
                        "total_amount": "27980"
                    },
                    "products": [{
                        "id": "1000000000000001",
                        "name": "Red Notebook",
                        "currency": "BRL",
                        "price": "12990",
                        "quantity": "2",
                        "media": { "images": [{ "id": "img-1", "request_image_url": "https://cdn.example.invalid/1-100.jpg" }] }
                    }]
                }
            }
        });

        let order = parse_order(&response).unwrap();
        assert_eq!(order.creation_timestamp, Some(1_700_000_000));

        let details = order.price_details.as_ref().unwrap();
        assert_eq!(details.currency.as_deref(), Some("BRL"));
        assert_eq!(details.subtotal.as_ref().unwrap().amount_1000, 25980);
        assert_eq!(details.total.as_ref().unwrap().amount_1000, 27980);

        let product = &order.products[0];
        assert_eq!(product.quantity, Some(2));
        assert_eq!(product.price.as_ref().unwrap().amount_1000, 12990);
        assert_eq!(product.images.len(), 1);
        // No variant_info on the wire is a product without variants, not a
        // malformed one.
        assert!(product.variant_properties.is_empty());
    }

    /// Without the chosen variant an order cannot be fulfilled: every variant
    /// of one listing shares an `id`, `name` and `price`.
    #[test]
    fn order_line_items_keep_the_chosen_variant() {
        let response = json!({
            "xwa_checkout_get_order_info": {
                "order": {
                    "products": [{
                        "id": "1000000000000001",
                        "variant_info": {
                            "variant_properties": [
                                { "name": "Size", "value": "Large" },
                                { "name": "Colour", "value": "Red" }
                            ]
                        }
                    }]
                }
            }
        });

        let order = parse_order(&response).unwrap();
        assert_eq!(
            order.products[0].variant_properties,
            vec![
                VariantProperty {
                    name: Some("Size".into()),
                    value: Some("Large".into()),
                },
                VariantProperty {
                    name: Some("Colour".into()),
                    value: Some("Red".into()),
                },
            ]
        );
    }

    #[test]
    fn order_without_payload_is_an_error() {
        assert!(parse_order(&json!({})).is_err());
        assert!(parse_order(&json!({ "xwa_checkout_get_order_info": { "order": {} } })).is_err());
    }

    /// Absent means no variants, but a present non-array is malformed — and
    /// silently reading it as variant-free would hide an unfulfillable order.
    #[test]
    fn non_array_variant_properties_is_an_error() {
        assert!(
            parse_order(&json!({
                "xwa_checkout_get_order_info": { "order": { "products": [{
                    "id": "1000000000000001",
                    "variant_info": { "variant_properties": "nope" }
                }] } }
            }))
            .is_err()
        );

        // A non-object variant_info indexes to Null, so without its own check it
        // would be indistinguishable from a product that has no variants.
        assert!(
            parse_order(&json!({
                "xwa_checkout_get_order_info": { "order": { "products": [{
                    "id": "1000000000000001",
                    "variant_info": "nope"
                }] } }
            }))
            .is_err()
        );

        // variant_info present but carrying no properties is still variant-free.
        let order = parse_order(&json!({
            "xwa_checkout_get_order_info": { "order": { "products": [{
                "id": "1000000000000001",
                "variant_info": {}
            }] } }
        }))
        .unwrap();
        assert!(order.products[0].variant_properties.is_empty());
    }

    /// A non-object line item would otherwise index to `Null` on every field
    /// and read back as a plausible product with everything absent.
    #[test]
    fn non_object_order_line_item_is_an_error() {
        assert!(
            parse_order(&json!({
                "xwa_checkout_get_order_info": { "order": { "products": ["not-an-object"] } }
            }))
            .is_err()
        );
    }

    /// Every nested object in a response is guarded the same way, because
    /// indexing a scalar yields Null for each child — so a malformed object
    /// would otherwise be indistinguishable from an absent one and degrade into
    /// plausible empty values. The paging and price cases are the ones that
    /// silently lose data: a bad `paging` truncates the catalog at that page,
    /// and a bad `price_details` returns an order with no totals.
    #[test]
    fn malformed_nested_objects_are_errors() {
        let catalog = |body| {
            parse_catalog(&json!({
                "xwa_product_catalog_get_product_catalog": { "product_catalog": body }
            }))
        };
        for (field, body) in [
            ("paging", json!({ "products": [], "paging": "nope" })),
            (
                "media",
                json!({ "products": [{ "id": "1", "media": "nope" }] }),
            ),
            (
                "status_info",
                json!({ "products": [{ "id": "1", "status_info": 7 }] }),
            ),
            (
                "compliance_info",
                json!({ "products": [{ "id": "1", "compliance_info": [] }] }),
            ),
            (
                "media.images entry",
                json!({ "products": [{ "id": "1", "media": { "images": ["x"] } }] }),
            ),
            (
                "compliance_info.importer_address",
                json!({ "products": [{ "id": "1", "compliance_info": { "importer_address": 1 } }] }),
            ),
        ] {
            assert!(catalog(body).is_err(), "catalog {field} must be rejected");
        }

        // Absent stays absent — the guard must not turn a missing object into
        // an error.
        assert!(catalog(json!({ "products": [{ "id": "1" }] })).is_ok());

        assert!(
            parse_collections(&json!({
                "xwa_product_catalog_get_collections": { "collections": [], "paging": 3 }
            }))
            .is_err(),
            "collections paging must be rejected"
        );

        let order =
            |body| parse_order(&json!({ "xwa_checkout_get_order_info": { "order": body } }));
        for (field, body) in [
            (
                "price_details",
                json!({ "products": [], "price_details": "nope" }),
            ),
            (
                "variant_properties entry",
                json!({ "products": [{ "variant_info": { "variant_properties": [1] } }] }),
            ),
        ] {
            assert!(order(body).is_err(), "order {field} must be rejected");
        }
    }

    /// WhatsApp Web gates the whole window on both ends being present
    /// (`start_date != null && end_date != null ? {...} : null`), so half a
    /// window is not a period and must not reach a caller as one.
    #[test]
    fn a_half_open_sale_window_is_dropped() {
        let with_sale = |sale| {
            parse_catalog(&json!({
                "xwa_product_catalog_get_product_catalog": { "product_catalog": { "products": [{
                    "id": "1000000000000001", "currency": "BRL", "sale_price": sale
                }] } }
            }))
            .unwrap()
            .products
            .swap_remove(0)
            .sale_price
            .unwrap()
        };

        for sale in [
            json!({ "price": "9990", "start_date": "1700000000" }),
            json!({ "price": "9990", "end_date": "1700600000" }),
        ] {
            let sale_price = with_sale(sale);
            // The price itself still stands; only the window is dropped.
            assert_eq!(sale_price.price.amount_1000, 9990);
            assert_eq!(sale_price.start_date, None);
            assert_eq!(sale_price.end_date, None);
        }

        let both = with_sale(json!({
            "price": "9990", "start_date": "1700000000", "end_date": "1700600000"
        }));
        assert_eq!(both.start_date.as_deref(), Some("1700000000"));
        assert_eq!(both.end_date.as_deref(), Some("1700600000"));
    }

    /// `shimmed_url` is sent alongside `url`, not instead of it, and arrives
    /// with no opt-in — so dropping it lost a server-provided link.
    #[test]
    fn products_keep_both_url_forms() {
        let product = &parse_catalog(&json!({
            "xwa_product_catalog_get_product_catalog": { "product_catalog": { "products": [{
                "id": "1000000000000001",
                "url": "https://shop.example.invalid/red-notebook",
                "shimmed_url": "https://l.example.invalid/?u=red-notebook"
            }] } }
        }))
        .unwrap()
        .products
        .swap_remove(0);

        assert_eq!(
            product.url.as_deref(),
            Some("https://shop.example.invalid/red-notebook")
        );
        assert_eq!(
            product.shimmed_url.as_deref(),
            Some("https://l.example.invalid/?u=red-notebook")
        );
    }

    #[test]
    fn missing_response_data_is_an_error() {
        assert!(response_data(None, "catalog").is_err());
    }

    #[test]
    fn amount_parsing_accepts_both_wire_spellings() {
        assert_eq!(parse_amount_1000(&json!("12990")), Some(12990));
        assert_eq!(parse_amount_1000(&json!(12990)), Some(12990));
        assert_eq!(parse_amount_1000(&json!("")), None);
        assert_eq!(parse_amount_1000(&json!(Value::Null)), None);
        assert_eq!(parse_amount_1000(&json!("not a number")), None);
    }
}

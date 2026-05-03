use leptos::prelude::*;
use leptos_graphql::{execute as execute_graphql, GraphqlHttpError, GraphqlRequest};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use uuid::Uuid;

use crate::model::{
    PricingChannelOption, PricingPriceListOption, PricingProductDetail, PricingProductList,
    PricingProductListItem, PricingProductTranslation, PricingResolutionContext, PricingVariant,
    StorefrontPricingData,
};
#[cfg(feature = "ssr")]
use crate::model::{PricingEffectivePrice, PricingPrice};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiError {
    Graphql(String),
    ServerFn(String),
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Graphql(error) => write!(f, "{error}"),
            Self::ServerFn(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ApiError {}

impl From<GraphqlHttpError> for ApiError {
    fn from(value: GraphqlHttpError) -> Self {
        Self::Graphql(value.to_string())
    }
}

impl From<ServerFnError> for ApiError {
    fn from(value: ServerFnError) -> Self {
        Self::ServerFn(value.to_string())
    }
}

const STOREFRONT_PRODUCTS_QUERY: &str = "query StorefrontCommerceProducts($locale: String, $filter: StorefrontProductsFilter, $channelId: UUID, $channelSlug: String) { storefrontProducts(locale: $locale, filter: $filter) { total page perPage hasNext items { id title handle sellerId vendor productType createdAt publishedAt } } storefrontPricingChannels { id slug name isActive isDefault status } storefrontActivePriceLists(channelId: $channelId, channelSlug: $channelSlug) { id name listType channelId channelSlug ruleKind adjustmentPercent } }";
const STOREFRONT_PRODUCT_QUERY: &str = "query StorefrontCommerceProduct($locale: String, $handle: String!, $currencyCode: String, $regionId: UUID, $priceListId: UUID, $channelId: UUID, $channelSlug: String, $quantity: Int) { storefrontPricingProduct(locale: $locale, handle: $handle, currencyCode: $currencyCode, regionId: $regionId, priceListId: $priceListId, channelId: $channelId, channelSlug: $channelSlug, quantity: $quantity) { id status sellerId vendor productType publishedAt translations { locale title handle description } variants { id title sku prices { currencyCode amount compareAtAmount discountPercent onSale } effectivePrice { currencyCode amount compareAtAmount discountPercent onSale regionId priceListId channelId channelSlug minQuantity maxQuantity } } } }";

#[derive(Debug, Deserialize)]
struct StorefrontProductsResponse {
    #[serde(rename = "storefrontProducts")]
    storefront_products: GraphqlPricingProductList,
    #[serde(rename = "storefrontPricingChannels", default)]
    available_channels: Vec<PricingChannelOption>,
    #[serde(rename = "storefrontActivePriceLists", default)]
    active_price_lists: Vec<PricingPriceListOption>,
}

#[derive(Debug, Deserialize)]
struct StorefrontProductResponse {
    #[serde(rename = "storefrontPricingProduct")]
    storefront_product: Option<GraphqlPricingProductDetail>,
}

#[derive(Debug, Serialize)]
struct StorefrontProductsVariables {
    locale: Option<String>,
    filter: StorefrontProductsFilter,
    #[serde(rename = "channelId")]
    channel_id: Option<String>,
    #[serde(rename = "channelSlug")]
    channel_slug: Option<String>,
}

#[derive(Debug, Serialize)]
struct StorefrontProductVariables {
    locale: Option<String>,
    handle: String,
    #[serde(rename = "currencyCode")]
    currency_code: Option<String>,
    #[serde(rename = "regionId")]
    region_id: Option<String>,
    #[serde(rename = "priceListId")]
    price_list_id: Option<String>,
    #[serde(rename = "channelId")]
    channel_id: Option<String>,
    #[serde(rename = "channelSlug")]
    channel_slug: Option<String>,
    quantity: Option<i32>,
}

#[derive(Debug, Serialize)]
struct StorefrontProductsFilter {
    vendor: Option<String>,
    #[serde(rename = "productType")]
    product_type: Option<String>,
    search: Option<String>,
    page: Option<u64>,
    #[serde(rename = "perPage")]
    per_page: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct GraphqlPricingProductList {
    items: Vec<GraphqlPricingProductListItem>,
    total: u64,
    page: u64,
    #[serde(rename = "perPage")]
    per_page: u64,
    #[serde(rename = "hasNext")]
    has_next: bool,
}

#[derive(Debug, Deserialize)]
struct GraphqlPricingProductListItem {
    id: String,
    title: String,
    handle: String,
    #[serde(rename = "sellerId", default)]
    seller_id: Option<String>,
    vendor: Option<String>,
    #[serde(rename = "productType")]
    product_type: Option<String>,
    #[serde(rename = "createdAt")]
    created_at: String,
    #[serde(rename = "publishedAt")]
    published_at: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GraphqlPricingProductDetail {
    id: String,
    status: String,
    #[serde(rename = "sellerId", default)]
    seller_id: Option<String>,
    vendor: Option<String>,
    #[serde(rename = "productType")]
    product_type: Option<String>,
    #[serde(rename = "publishedAt")]
    published_at: Option<String>,
    translations: Vec<PricingProductTranslation>,
    variants: Vec<PricingVariant>,
}

fn configured_tenant_slug() -> Option<String> {
    [
        "RUSTOK_TENANT_SLUG",
        "NEXT_PUBLIC_TENANT_SLUG",
        "NEXT_PUBLIC_DEFAULT_TENANT_SLUG",
    ]
    .into_iter()
    .find_map(|key| {
        std::env::var(key).ok().and_then(|value| {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed)
            }
        })
    })
}

fn graphql_url() -> String {
    if let Some(url) = option_env!("RUSTOK_GRAPHQL_URL") {
        return url.to_string();
    }

    #[cfg(target_arch = "wasm32")]
    {
        let origin = web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:5150".to_string());
        format!("{origin}/api/graphql")
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let base =
            std::env::var("RUSTOK_API_URL").unwrap_or_else(|_| "http://localhost:5150".to_string());
        format!("{base}/api/graphql")
    }
}

async fn request<V, T>(query: &str, variables: V) -> Result<T, ApiError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(query, Some(variables)),
        None,
        configured_tenant_slug(),
        None,
    )
    .await
    .map_err(ApiError::from)
}

pub async fn fetch_storefront_pricing(
    query: StorefrontPricingQuery,
) -> Result<StorefrontPricingData, ApiError> {
    let channel_id = parse_optional_uuid_string(query.channel_id.clone(), "channel_id")?;
    let channel_slug = sanitize_channel_slug(query.channel_slug.clone());
    let resolution_context = sanitize_resolution_context(
        query.currency_code.clone(),
        query.region_id.clone(),
        query.price_list_id.clone(),
        channel_id.clone(),
        channel_slug.clone(),
        query.quantity,
    )?;
    let currency_code = resolution_context
        .as_ref()
        .map(|context| context.currency_code.clone());
    let region_id = resolution_context
        .as_ref()
        .and_then(|context| context.region_id.clone());
    let price_list_id = resolution_context
        .as_ref()
        .and_then(|context| context.price_list_id.clone());
    let quantity = resolution_context.as_ref().map(|context| context.quantity);
    match fetch_storefront_pricing_server(StorefrontPricingQuery {
        selected_handle: query.selected_handle.clone(),
        locale: query.locale.clone(),
        currency_code: currency_code.clone(),
        region_id: region_id.clone(),
        price_list_id: price_list_id.clone(),
        channel_id: channel_id.clone(),
        channel_slug: channel_slug.clone(),
        quantity,
    })
    .await
    {
        Ok(data) => Ok(data),
        Err(_) => {
            fetch_storefront_pricing_graphql(StorefrontPricingQuery {
                selected_handle: query.selected_handle,
                locale: query.locale,
                currency_code,
                region_id,
                price_list_id,
                channel_id,
                channel_slug,
                quantity,
            })
            .await
        }
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct StorefrontPricingQuery {
    pub selected_handle: Option<String>,
    pub locale: Option<String>,
    pub currency_code: Option<String>,
    pub region_id: Option<String>,
    pub price_list_id: Option<String>,
    pub channel_id: Option<String>,
    pub channel_slug: Option<String>,
    pub quantity: Option<i32>,
}

pub async fn fetch_storefront_pricing_server(
    query: StorefrontPricingQuery,
) -> Result<StorefrontPricingData, ApiError> {
    storefront_pricing_native(query)
        .await
        .map_err(ApiError::from)
}

pub async fn fetch_storefront_pricing_graphql(
    query: StorefrontPricingQuery,
) -> Result<StorefrontPricingData, ApiError> {
    let selected_channel_id = parse_optional_uuid_string(query.channel_id.clone(), "channel_id")?;
    let selected_channel_slug = sanitize_channel_slug(query.channel_slug.clone());
    let resolution_context = sanitize_resolution_context(
        query.currency_code.clone(),
        query.region_id.clone(),
        query.price_list_id.clone(),
        query.channel_id,
        query.channel_slug,
        query.quantity,
    )?;
    let list_response: StorefrontProductsResponse = request(
        STOREFRONT_PRODUCTS_QUERY,
        StorefrontProductsVariables {
            locale: query.locale.clone(),
            filter: StorefrontProductsFilter {
                vendor: None,
                product_type: None,
                search: None,
                page: Some(1),
                per_page: Some(8),
            },
            channel_id: selected_channel_id,
            channel_slug: selected_channel_slug,
        },
    )
    .await?;

    let mut items = Vec::new();
    for item in list_response.storefront_products.items {
        let detail = if item.handle.trim().is_empty() {
            None
        } else {
            fetch_storefront_pricing_graphql_detail(StorefrontPricingDetailQuery {
                handle: item.handle.clone(),
                locale: query.locale.clone(),
                ..StorefrontPricingDetailQuery::default()
            })
            .await?
        };
        items.push(resolve_graphql_pricing_list_item(item, detail.as_ref()));
    }

    let resolved_handle = query
        .selected_handle
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| items.first().map(|item| item.handle.clone()));
    let selected_product = if let Some(handle) = resolved_handle.clone() {
        fetch_storefront_pricing_graphql_detail(StorefrontPricingDetailQuery {
            handle,
            locale: query.locale,
            currency_code: resolution_context
                .as_ref()
                .map(|value| value.currency_code.clone()),
            region_id: resolution_context
                .as_ref()
                .and_then(|value| value.region_id.clone()),
            price_list_id: resolution_context
                .as_ref()
                .and_then(|value| value.price_list_id.clone()),
            channel_id: resolution_context
                .as_ref()
                .and_then(|value| value.channel_id.clone()),
            channel_slug: resolution_context
                .as_ref()
                .and_then(|value| value.channel_slug.clone()),
            quantity: resolution_context.as_ref().map(|value| value.quantity),
        })
        .await?
    } else {
        None
    };

    Ok(StorefrontPricingData {
        products: PricingProductList {
            items,
            total: list_response.storefront_products.total,
            page: list_response.storefront_products.page,
            per_page: list_response.storefront_products.per_page,
            has_next: list_response.storefront_products.has_next,
        },
        selected_product,
        selected_handle: resolved_handle,
        resolution_context,
        available_channels: list_response.available_channels,
        active_price_lists: list_response.active_price_lists,
    })
}

async fn fetch_storefront_pricing_graphql_detail(
    query: StorefrontPricingDetailQuery,
) -> Result<Option<PricingProductDetail>, ApiError> {
    let response: StorefrontProductResponse = request(
        STOREFRONT_PRODUCT_QUERY,
        StorefrontProductVariables {
            locale: query.locale,
            handle: query.handle,
            currency_code: query.currency_code,
            region_id: query.region_id,
            price_list_id: query.price_list_id,
            channel_id: query.channel_id,
            channel_slug: query.channel_slug,
            quantity: query.quantity,
        },
    )
    .await?;
    Ok(response.storefront_product.map(map_graphql_detail))
}

#[derive(Clone, Debug, Default)]
struct StorefrontPricingDetailQuery {
    handle: String,
    locale: Option<String>,
    currency_code: Option<String>,
    region_id: Option<String>,
    price_list_id: Option<String>,
    channel_id: Option<String>,
    channel_slug: Option<String>,
    quantity: Option<i32>,
}

fn resolve_graphql_pricing_list_item(
    item: GraphqlPricingProductListItem,
    detail: Option<&PricingProductDetail>,
) -> PricingProductListItem {
    let variant_count = detail
        .map(|detail| detail.variants.len() as u64)
        .unwrap_or(0);
    let sale_variant_count = detail
        .map(|detail| {
            detail
                .variants
                .iter()
                .filter(|variant| variant.prices.iter().any(|price| price.on_sale))
                .count() as u64
        })
        .unwrap_or(0);
    let mut currencies = detail
        .map(|detail| {
            let mut set = std::collections::BTreeSet::new();
            for variant in &detail.variants {
                for price in &variant.prices {
                    set.insert(price.currency_code.clone());
                }
            }
            set.into_iter().collect::<Vec<_>>()
        })
        .unwrap_or_default();
    currencies.sort();

    PricingProductListItem {
        id: item.id,
        title: item.title,
        handle: item.handle,
        seller_id: item.seller_id,
        vendor: item.vendor,
        product_type: item.product_type,
        created_at: item.created_at,
        published_at: item.published_at,
        variant_count,
        sale_variant_count,
        currencies,
    }
}

fn map_graphql_detail(value: GraphqlPricingProductDetail) -> PricingProductDetail {
    PricingProductDetail {
        id: value.id,
        status: value.status,
        seller_id: value.seller_id,
        vendor: value.vendor,
        product_type: value.product_type,
        published_at: value.published_at,
        translations: value.translations,
        variants: value
            .variants
            .into_iter()
            .map(|variant| PricingVariant {
                id: variant.id,
                title: variant.title,
                sku: variant.sku,
                prices: variant.prices,
                effective_price: None,
            })
            .collect(),
    }
}

#[cfg(feature = "ssr")]
fn map_native_list(value: rustok_pricing::StorefrontPricingProductList) -> PricingProductList {
    PricingProductList {
        items: value.items.into_iter().map(map_native_list_item).collect(),
        total: value.total,
        page: value.page,
        per_page: value.per_page,
        has_next: value.has_next,
    }
}

#[cfg(feature = "ssr")]
fn map_native_list_item(
    value: rustok_pricing::StorefrontPricingProductListItem,
) -> PricingProductListItem {
    PricingProductListItem {
        id: value.id.to_string(),
        title: value.title,
        handle: value.handle,
        seller_id: value.seller_id,
        vendor: value.vendor,
        product_type: value.product_type,
        created_at: value.created_at.to_rfc3339(),
        published_at: value.published_at.map(|value| value.to_rfc3339()),
        variant_count: value.variant_count,
        sale_variant_count: value.sale_variant_count,
        currencies: value.currencies,
    }
}

#[cfg(feature = "ssr")]
fn map_native_detail(
    value: rustok_pricing::StorefrontPricingProductDetail,
) -> PricingProductDetail {
    PricingProductDetail {
        id: value.id.to_string(),
        status: value.status.to_string(),
        seller_id: value.seller_id,
        vendor: value.vendor,
        product_type: value.product_type,
        published_at: value.published_at.map(|value| value.to_rfc3339()),
        translations: value
            .translations
            .into_iter()
            .map(|translation| PricingProductTranslation {
                locale: translation.locale,
                title: translation.title,
                handle: translation.handle,
                description: translation.description,
            })
            .collect(),
        variants: value
            .variants
            .into_iter()
            .map(|variant| PricingVariant {
                id: variant.id.to_string(),
                title: variant.title,
                sku: variant.sku,
                prices: variant
                    .prices
                    .into_iter()
                    .map(|price| PricingPrice {
                        currency_code: price.currency_code,
                        amount: price.amount.normalize().to_string(),
                        compare_at_amount: price
                            .compare_at_amount
                            .map(|value| value.normalize().to_string()),
                        discount_percent: price
                            .discount_percent
                            .map(|value| value.normalize().to_string()),
                        on_sale: price.on_sale,
                    })
                    .collect(),
                effective_price: None,
            })
            .collect(),
    }
}

#[cfg(feature = "ssr")]
fn map_native_effective_price(value: rustok_pricing::ResolvedPrice) -> PricingEffectivePrice {
    PricingEffectivePrice {
        currency_code: value.currency_code,
        amount: value.amount.normalize().to_string(),
        compare_at_amount: value
            .compare_at_amount
            .map(|item| item.normalize().to_string()),
        discount_percent: value
            .discount_percent
            .map(|item| item.normalize().to_string()),
        on_sale: value.on_sale,
        region_id: value.region_id.map(|item| item.to_string()),
        price_list_id: value.price_list_id.map(|item| item.to_string()),
        channel_id: value.channel_id.map(|item| item.to_string()),
        channel_slug: value.channel_slug,
        min_quantity: value.min_quantity,
        max_quantity: value.max_quantity,
    }
}

#[cfg(feature = "ssr")]
fn map_native_price_list_option(
    value: rustok_pricing::ActivePriceListOption,
) -> PricingPriceListOption {
    PricingPriceListOption {
        id: value.id.to_string(),
        name: value.name,
        list_type: value.list_type,
        channel_id: value.channel_id.map(|item| item.to_string()),
        channel_slug: value.channel_slug,
        rule_kind: value.rule_kind,
        adjustment_percent: value
            .adjustment_percent
            .map(|item| item.normalize().to_string()),
    }
}

#[cfg(feature = "ssr")]
fn map_channel_option(value: rustok_channel::ChannelResponse) -> PricingChannelOption {
    PricingChannelOption {
        id: value.id.to_string(),
        slug: value.slug,
        name: value.name,
        is_active: value.is_active,
        is_default: value.is_default,
        status: value.status,
    }
}

fn text_or_none(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn parse_optional_currency_code(currency_code: Option<String>) -> Result<Option<String>, ApiError> {
    let Some(currency_code) = currency_code.and_then(text_or_none) else {
        return Ok(None);
    };
    let normalized = currency_code.to_ascii_uppercase();
    if normalized.len() != 3 || !normalized.chars().all(|ch| ch.is_ascii_alphabetic()) {
        return Err(ApiError::ServerFn(
            "currency_code must be a 3-letter code".to_string(),
        ));
    }

    Ok(Some(normalized))
}

fn parse_optional_uuid_string(
    value: Option<String>,
    field_name: &str,
) -> Result<Option<String>, ApiError> {
    let Some(value) = value.and_then(text_or_none) else {
        return Ok(None);
    };

    Uuid::parse_str(value.as_str())
        .map(|_| Some(value))
        .map_err(|_| ApiError::ServerFn(format!("Invalid {field_name}")))
}

fn sanitize_channel_slug(channel_slug: Option<String>) -> Option<String> {
    channel_slug
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value.and_then(|value| {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    })
}

fn resolve_requested_locale(
    requested: Option<String>,
    request_context_locale: Option<&str>,
    tenant_default_locale: &str,
) -> String {
    normalize_optional(requested)
        .or_else(|| {
            request_context_locale.and_then(|value| normalize_optional(Some(value.to_string())))
        })
        .or_else(|| normalize_optional(Some(tenant_default_locale.to_string())))
        .unwrap_or_default()
}

fn parse_resolution_quantity(quantity: Option<i32>) -> Result<i32, ApiError> {
    match quantity {
        Some(value) if value < 1 => Err(ApiError::ServerFn(
            "quantity must be at least 1".to_string(),
        )),
        Some(value) => Ok(value),
        None => Ok(1),
    }
}

fn sanitize_resolution_context(
    currency_code: Option<String>,
    region_id: Option<String>,
    price_list_id: Option<String>,
    channel_id: Option<String>,
    channel_slug: Option<String>,
    quantity: Option<i32>,
) -> Result<Option<PricingResolutionContext>, ApiError> {
    let channel_id = parse_optional_uuid_string(channel_id, "channel_id")?;
    let requires_currency = region_id
        .as_ref()
        .and_then(|value| text_or_none(value.clone()))
        .is_some()
        || price_list_id
            .as_ref()
            .and_then(|value| text_or_none(value.clone()))
            .is_some()
        || quantity.is_some();
    let Some(currency_code) = parse_optional_currency_code(currency_code)? else {
        if requires_currency {
            return Err(ApiError::ServerFn(
                "currency_code is required for pricing resolution context".to_string(),
            ));
        }
        return Ok(None);
    };

    Ok(Some(PricingResolutionContext {
        currency_code,
        region_id: parse_optional_uuid_string(region_id, "region_id")?,
        price_list_id: parse_optional_uuid_string(price_list_id, "price_list_id")?,
        channel_id,
        channel_slug: sanitize_channel_slug(channel_slug),
        quantity: parse_resolution_quantity(quantity)?,
    }))
}

#[cfg(feature = "ssr")]
fn normalize_public_channel_slug(channel_slug: Option<&str>) -> Option<String> {
    channel_slug
        .map(str::trim)
        .filter(|slug| !slug.is_empty())
        .map(|slug| slug.to_ascii_lowercase())
}

#[server(prefix = "/api/fn", endpoint = "pricing/storefront-data")]
async fn storefront_pricing_native(
    query: StorefrontPricingQuery,
) -> Result<StorefrontPricingData, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::loco::transactional_event_bus_from_context;
        use rustok_channel::ChannelService;
        use rustok_pricing::{PriceResolutionContext, PricingService};

        let app_ctx = expect_context::<AppContext>();
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .ok();
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let requested_locale = resolve_requested_locale(
            query.locale,
            request_context.as_ref().map(|ctx| ctx.locale.as_str()),
            tenant.default_locale.as_str(),
        );
        let explicit_channel_id = parse_optional_uuid_string(query.channel_id, "channel_id")
            .map_err(|err| ServerFnError::new(err.to_string()))?;
        let selected_channel_id = explicit_channel_id
            .as_deref()
            .and_then(|value| Uuid::parse_str(value).ok())
            .or_else(|| request_context.as_ref().and_then(|ctx| ctx.channel_id));
        let selected_channel_slug = sanitize_channel_slug(query.channel_slug).or_else(|| {
            request_context
                .as_ref()
                .and_then(|ctx| normalize_public_channel_slug(ctx.channel_slug.as_deref()))
        });
        let mut resolution_context = sanitize_resolution_context(
            query.currency_code.clone(),
            query.region_id.clone(),
            query.price_list_id.clone(),
            selected_channel_id.map(|value| value.to_string()),
            selected_channel_slug.clone(),
            query.quantity,
        )
        .map_err(|err| ServerFnError::new(err.to_string()))?;
        if let Some(context) = resolution_context.as_mut() {
            context.channel_id = selected_channel_id.map(|item| item.to_string());
            context.channel_slug = selected_channel_slug.clone();
        }
        let native_resolution_context = resolution_context.as_ref().map(|context| {
            let region_id = context
                .region_id
                .as_deref()
                .and_then(|value| Uuid::parse_str(value).ok());
            let price_list_id = context
                .price_list_id
                .as_deref()
                .and_then(|value| Uuid::parse_str(value).ok());
            let channel_id = context
                .channel_id
                .as_deref()
                .and_then(|value| Uuid::parse_str(value).ok());
            PriceResolutionContext {
                currency_code: context.currency_code.clone(),
                region_id,
                price_list_id,
                channel_id,
                channel_slug: context.channel_slug.clone(),
                quantity: Some(context.quantity),
            }
        });

        let service = PricingService::new(
            app_ctx.db.clone(),
            transactional_event_bus_from_context(&app_ctx),
        );
        let channel_service = ChannelService::new(app_ctx.db.clone());
        let (available_channels, _) = channel_service
            .list_channels(tenant.id, 1, 250)
            .await
            .map_err(ServerFnError::new)?;
        let active_price_lists = service
            .list_active_price_lists_for_channel(
                tenant.id,
                selected_channel_id,
                selected_channel_slug.as_deref(),
                Some(requested_locale.as_str()),
                Some(tenant.default_locale.as_str()),
            )
            .await
            .map_err(ServerFnError::new)?
            .into_iter()
            .map(map_native_price_list_option)
            .collect();
        let products = service
            .list_published_product_pricing_with_locale_fallback(
                tenant.id,
                requested_locale.as_str(),
                Some(tenant.default_locale.as_str()),
                selected_channel_slug.as_deref(),
                1,
                8,
            )
            .await
            .map_err(ServerFnError::new)?;
        let resolved_handle = query
            .selected_handle
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| products.items.first().map(|item| item.handle.clone()));
        let selected_product = if let Some(handle) = resolved_handle.clone() {
            let mut detail = service
                .get_published_product_pricing_by_handle_with_locale_fallback(
                    tenant.id,
                    handle.as_str(),
                    requested_locale.as_str(),
                    Some(tenant.default_locale.as_str()),
                    selected_channel_slug.as_deref(),
                )
                .await
                .map_err(ServerFnError::new)?
                .map(map_native_detail);

            if let (Some(detail_ref), Some(context)) =
                (detail.as_mut(), native_resolution_context.as_ref())
            {
                for variant in &mut detail_ref.variants {
                    let variant_id = Uuid::parse_str(&variant.id).map_err(ServerFnError::new)?;
                    let effective_price = service
                        .resolve_variant_price(tenant.id, variant_id, context.clone())
                        .await
                        .map_err(ServerFnError::new)?;
                    variant.effective_price = effective_price.map(map_native_effective_price);
                }
            }

            detail
        } else {
            None
        };

        Ok(StorefrontPricingData {
            products: map_native_list(products),
            selected_product,
            selected_handle: resolved_handle,
            resolution_context,
            available_channels: available_channels
                .into_iter()
                .map(map_channel_option)
                .collect(),
            active_price_lists,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = query;
        Err(ServerFnError::new(
            "pricing/storefront-data requires the `ssr` feature",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn storefront_pricing_resolution_context_rejects_non_letter_currency_code() {
        let error =
            sanitize_resolution_context(Some("EU1".to_string()), None, None, None, None, Some(1))
                .expect_err("invalid currency should be rejected");

        assert!(error
            .to_string()
            .contains("currency_code must be a 3-letter code"));
    }

    #[test]
    fn storefront_pricing_resolution_context_rejects_non_positive_quantity() {
        let error =
            sanitize_resolution_context(Some("EUR".to_string()), None, None, None, None, Some(0))
                .expect_err("invalid quantity should be rejected");

        assert!(error.to_string().contains("quantity must be at least 1"));
    }

    #[test]
    fn storefront_pricing_resolution_context_rejects_modifiers_without_currency_code() {
        let error = sanitize_resolution_context(
            None,
            Some(Uuid::new_v4().to_string()),
            None,
            None,
            None,
            None,
        )
        .expect_err("region_id without currency should be rejected");

        assert!(error
            .to_string()
            .contains("currency_code is required for pricing resolution context"));
    }

    #[test]
    fn storefront_pricing_resolution_context_rejects_invalid_channel_id() {
        let error = sanitize_resolution_context(
            None,
            None,
            None,
            Some("not-a-uuid".to_string()),
            None,
            None,
        )
        .expect_err("invalid channel_id should be rejected");

        assert!(error.to_string().contains("Invalid channel_id"));
    }
}

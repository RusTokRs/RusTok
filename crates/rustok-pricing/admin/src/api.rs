use leptos::prelude::*;
use leptos_graphql::{execute as execute_graphql, GraphqlHttpError, GraphqlRequest};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};
use uuid::Uuid;

use crate::model::{
    CurrentTenant, PricingAdjustmentPreview, PricingAdminBootstrap, PricingChannelOption,
    PricingDiscountDraft, PricingPriceDraft, PricingPriceListOption, PricingPriceListRuleDraft,
    PricingPriceListScopeDraft, PricingProductDetail, PricingProductList, PricingResolutionContext,
};
#[cfg(feature = "ssr")]
use crate::model::{
    PricingEffectivePrice, PricingProductListItem, PricingProductTranslation, PricingVariant,
};

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

const BOOTSTRAP_QUERY: &str = "query PricingAdminBootstrap { currentTenant { id slug name } storefrontPricingChannels { id slug name isActive isDefault status } storefrontActivePriceLists { id name listType channelId channelSlug ruleKind adjustmentPercent } }";
const ACTIVE_PRICE_LISTS_QUERY: &str = "query PricingAdminActivePriceLists($channelId: UUID, $channelSlug: String) { storefrontActivePriceLists(channelId: $channelId, channelSlug: $channelSlug) { id name listType channelId channelSlug ruleKind adjustmentPercent } }";
const PRODUCTS_QUERY: &str = "query PricingAdminProducts($tenantId: UUID!, $locale: String, $filter: ProductsFilter) { products(tenantId: $tenantId, locale: $locale, filter: $filter) { total page perPage hasNext items { id status sellerId title handle vendor productType shippingProfileSlug tags createdAt publishedAt } } }";
const PRODUCT_QUERY: &str = "query PricingAdminProduct($tenantId: UUID!, $id: UUID!, $locale: String, $currencyCode: String, $regionId: UUID, $priceListId: UUID, $channelId: UUID, $channelSlug: String, $quantity: Int) { adminPricingProduct(tenantId: $tenantId, id: $id, locale: $locale, currencyCode: $currencyCode, regionId: $regionId, priceListId: $priceListId, channelId: $channelId, channelSlug: $channelSlug, quantity: $quantity) { id status sellerId vendor productType shippingProfileSlug createdAt updatedAt publishedAt translations { locale title handle description } variants { id sku barcode shippingProfileSlug title option1 option2 option3 prices { currencyCode amount compareAtAmount discountPercent onSale priceListId channelId channelSlug minQuantity maxQuantity } effectivePrice { currencyCode amount compareAtAmount discountPercent onSale regionId priceListId channelId channelSlug minQuantity maxQuantity } } } }";

#[derive(Debug, Deserialize)]
struct BootstrapResponse {
    #[serde(rename = "currentTenant")]
    current_tenant: CurrentTenant,
    #[serde(rename = "storefrontPricingChannels", default)]
    available_channels: Vec<PricingChannelOption>,
    #[serde(rename = "storefrontActivePriceLists", default)]
    active_price_lists: Vec<PricingPriceListOption>,
}

#[derive(Debug, Deserialize)]
struct ProductsResponse {
    products: PricingProductList,
}

#[derive(Debug, Deserialize)]
struct ProductResponse {
    #[serde(rename = "adminPricingProduct")]
    product: Option<PricingProductDetail>,
}

#[derive(Debug, Deserialize)]
struct ActivePriceListsResponse {
    #[serde(rename = "storefrontActivePriceLists", default)]
    active_price_lists: Vec<PricingPriceListOption>,
}

#[derive(Debug, Serialize)]
struct TenantScopedVariables<T> {
    #[serde(rename = "tenantId")]
    tenant_id: String,
    #[serde(flatten)]
    extra: T,
}

#[derive(Debug, Serialize)]
struct ProductsVariables {
    locale: Option<String>,
    filter: ProductsFilter,
}

#[derive(Debug, Serialize)]
struct ProductVariables {
    id: String,
    locale: Option<String>,
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
struct ActivePriceListsVariables {
    #[serde(rename = "channelId")]
    channel_id: Option<String>,
    #[serde(rename = "channelSlug")]
    channel_slug: Option<String>,
}

#[derive(Debug, Serialize)]
struct ProductsFilter {
    status: Option<String>,
    vendor: Option<String>,
    search: Option<String>,
    page: Option<u64>,
    #[serde(rename = "perPage")]
    per_page: Option<u64>,
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

async fn request<V, T>(
    query: &str,
    variables: Option<V>,
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<T, ApiError>
where
    V: Serialize,
    T: for<'de> Deserialize<'de>,
{
    execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(query, variables),
        token,
        tenant_slug,
        None,
    )
    .await
    .map_err(ApiError::from)
}

pub async fn fetch_bootstrap(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<PricingAdminBootstrap, ApiError> {
    match pricing_admin_bootstrap_native().await {
        Ok(data) => Ok(data),
        Err(_) => fetch_bootstrap_graphql(token, tenant_slug).await,
    }
}

pub async fn fetch_active_price_lists(
    token: Option<String>,
    tenant_slug: Option<String>,
    channel_id: Option<String>,
    channel_slug: Option<String>,
) -> Result<Vec<PricingPriceListOption>, ApiError> {
    let channel_id =
        parse_optional_uuid_string(channel_id, "channel_id").map_err(ApiError::from)?;
    let channel_slug = sanitize_channel_slug(channel_slug);
    match pricing_admin_active_price_lists_native(channel_id.clone(), channel_slug.clone()).await {
        Ok(data) => Ok(data),
        Err(_) => {
            fetch_active_price_lists_graphql(token, tenant_slug, channel_id, channel_slug).await
        }
    }
}

pub async fn fetch_products(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    locale: Option<String>,
    search: Option<String>,
    status: Option<String>,
) -> Result<PricingProductList, ApiError> {
    match pricing_admin_products_native(
        locale.clone().unwrap_or_default(),
        search.clone(),
        status.clone(),
    )
    .await
    {
        Ok(data) => Ok(data),
        Err(_) => {
            fetch_products_graphql(
                token,
                tenant_slug,
                tenant_id,
                locale.unwrap_or_default(),
                search,
                status,
            )
            .await
        }
    }
}

pub async fn fetch_product(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
    locale: Option<String>,
    currency_code: Option<String>,
    region_id: Option<String>,
    price_list_id: Option<String>,
    channel_id: Option<String>,
    channel_slug: Option<String>,
    quantity: Option<i32>,
) -> Result<Option<PricingProductDetail>, ApiError> {
    let channel_id =
        parse_optional_uuid_string(channel_id, "channel_id").map_err(ApiError::from)?;
    let channel_slug = sanitize_channel_slug(channel_slug);
    let resolution_context =
        sanitize_resolution_context(currency_code, region_id, price_list_id, quantity)
            .map_err(ApiError::from)?;
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
    match pricing_admin_product_native(
        id.clone(),
        locale.clone().unwrap_or_default(),
        currency_code.clone(),
        region_id.clone(),
        price_list_id.clone(),
        channel_id.clone(),
        channel_slug.clone(),
        quantity,
    )
    .await
    {
        Ok(data) => Ok(data),
        Err(_) => {
            fetch_product_graphql(
                token,
                tenant_slug,
                tenant_id,
                id,
                locale.unwrap_or_default(),
                currency_code,
                region_id,
                price_list_id,
                channel_id,
                channel_slug,
                quantity,
            )
            .await
        }
    }
}

pub async fn update_variant_price(
    variant_id: String,
    payload: PricingPriceDraft,
) -> Result<(), ApiError> {
    pricing_admin_update_variant_price_native(variant_id, payload)
        .await
        .map_err(Into::into)
}

pub async fn preview_variant_discount(
    variant_id: String,
    payload: PricingDiscountDraft,
) -> Result<PricingAdjustmentPreview, ApiError> {
    pricing_admin_preview_variant_discount_native(variant_id, payload)
        .await
        .map_err(Into::into)
}

pub async fn apply_variant_discount(
    variant_id: String,
    payload: PricingDiscountDraft,
) -> Result<PricingAdjustmentPreview, ApiError> {
    pricing_admin_apply_variant_discount_native(variant_id, payload)
        .await
        .map_err(Into::into)
}

pub async fn update_price_list_rule(
    price_list_id: String,
    payload: PricingPriceListRuleDraft,
) -> Result<PricingPriceListOption, ApiError> {
    pricing_admin_update_price_list_rule_native(price_list_id, payload)
        .await
        .map_err(Into::into)
}

pub async fn update_price_list_scope(
    price_list_id: String,
    payload: PricingPriceListScopeDraft,
) -> Result<PricingPriceListOption, ApiError> {
    pricing_admin_update_price_list_scope_native(price_list_id, payload)
        .await
        .map_err(Into::into)
}

async fn fetch_bootstrap_graphql(
    token: Option<String>,
    tenant_slug: Option<String>,
) -> Result<PricingAdminBootstrap, ApiError> {
    let response: BootstrapResponse =
        request::<serde_json::Value, BootstrapResponse>(BOOTSTRAP_QUERY, None, token, tenant_slug)
            .await?;
    Ok(PricingAdminBootstrap {
        current_tenant: response.current_tenant,
        available_channels: response.available_channels,
        active_price_lists: response.active_price_lists,
    })
}

async fn fetch_active_price_lists_graphql(
    token: Option<String>,
    tenant_slug: Option<String>,
    channel_id: Option<String>,
    channel_slug: Option<String>,
) -> Result<Vec<PricingPriceListOption>, ApiError> {
    let response: ActivePriceListsResponse = request(
        ACTIVE_PRICE_LISTS_QUERY,
        Some(ActivePriceListsVariables {
            channel_id,
            channel_slug,
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.active_price_lists)
}

async fn fetch_products_graphql(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    locale: String,
    search: Option<String>,
    status: Option<String>,
) -> Result<PricingProductList, ApiError> {
    let response: ProductsResponse = request(
        PRODUCTS_QUERY,
        Some(TenantScopedVariables {
            tenant_id,
            extra: ProductsVariables {
                locale: Some(locale),
                filter: ProductsFilter {
                    status,
                    vendor: None,
                    search,
                    page: Some(1),
                    per_page: Some(24),
                },
            },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.products)
}

async fn fetch_product_graphql(
    token: Option<String>,
    tenant_slug: Option<String>,
    tenant_id: String,
    id: String,
    locale: String,
    currency_code: Option<String>,
    region_id: Option<String>,
    price_list_id: Option<String>,
    channel_id: Option<String>,
    channel_slug: Option<String>,
    quantity: Option<i32>,
) -> Result<Option<PricingProductDetail>, ApiError> {
    let response: ProductResponse = request(
        PRODUCT_QUERY,
        Some(TenantScopedVariables {
            tenant_id,
            extra: ProductVariables {
                id,
                locale: Some(locale),
                currency_code,
                region_id,
                price_list_id,
                channel_id,
                channel_slug,
                quantity,
            },
        }),
        token,
        tenant_slug,
    )
    .await?;
    Ok(response.product)
}

fn parse_optional_currency_code(
    currency_code: Option<String>,
) -> Result<Option<String>, ServerFnError> {
    let Some(currency_code) = currency_code.and_then(text_or_none) else {
        return Ok(None);
    };
    let normalized = currency_code.to_ascii_uppercase();
    if normalized.len() != 3 || !normalized.chars().all(|ch| ch.is_ascii_alphabetic()) {
        return Err(ServerFnError::new("currency_code must be a 3-letter code"));
    }

    Ok(Some(normalized))
}

#[cfg(feature = "ssr")]
fn parse_required_currency_code(currency_code: String) -> Result<String, ServerFnError> {
    parse_optional_currency_code(Some(currency_code))?
        .ok_or_else(|| ServerFnError::new("currency_code must be a 3-letter code"))
}

#[cfg(feature = "ssr")]
fn parse_decimal(value: &str, field_name: &str) -> Result<rust_decimal::Decimal, ServerFnError> {
    <rust_decimal::Decimal as std::str::FromStr>::from_str(value.trim())
        .map_err(|_| ServerFnError::new(format!("Invalid {field_name}")))
}

#[cfg(feature = "ssr")]
fn parse_optional_decimal(
    value: &str,
    field_name: &str,
) -> Result<Option<rust_decimal::Decimal>, ServerFnError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        Ok(None)
    } else {
        Ok(Some(parse_decimal(trimmed, field_name)?))
    }
}

#[cfg(feature = "ssr")]
fn parse_optional_quantity(value: &str, field_name: &str) -> Result<Option<i32>, ServerFnError> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Ok(None);
    }

    let parsed = trimmed
        .parse::<i32>()
        .map_err(|_| ServerFnError::new(format!("Invalid {field_name}")))?;
    if parsed <= 0 {
        return Err(ServerFnError::new(format!(
            "{field_name} must be greater than zero"
        )));
    }

    Ok(Some(parsed))
}

#[cfg(feature = "ssr")]
fn parse_optional_uuid(value: &str, field_name: &str) -> Result<Option<Uuid>, ServerFnError> {
    let Some(value) = text_or_none(value.to_string()) else {
        return Ok(None);
    };

    Uuid::parse_str(value.as_str())
        .map(Some)
        .map_err(|_| ServerFnError::new(format!("Invalid {field_name}")))
}

fn sanitize_channel_slug(channel_slug: Option<String>) -> Option<String> {
    channel_slug
        .map(|value| value.trim().to_ascii_lowercase())
        .filter(|value| !value.is_empty())
}

fn text_or_none(value: String) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

#[cfg(feature = "ssr")]
fn resolve_requested_locale(
    requested: Option<String>,
    request_context_locale: Option<&str>,
    tenant_default_locale: &str,
) -> String {
    requested
        .and_then(text_or_none)
        .or_else(|| request_context_locale.and_then(|value| text_or_none(value.to_string())))
        .or_else(|| text_or_none(tenant_default_locale.to_string()))
        .unwrap_or_default()
}

fn parse_optional_uuid_string(
    value: Option<String>,
    field_name: &str,
) -> Result<Option<String>, ServerFnError> {
    let Some(value) = value.and_then(text_or_none) else {
        return Ok(None);
    };

    Uuid::parse_str(value.as_str())
        .map(|_| Some(value))
        .map_err(|_| ServerFnError::new(format!("Invalid {field_name}")))
}

fn parse_resolution_quantity(quantity: Option<i32>) -> Result<i32, ServerFnError> {
    match quantity {
        Some(value) if value < 1 => Err(ServerFnError::new("quantity must be at least 1")),
        Some(value) => Ok(value),
        None => Ok(1),
    }
}

fn sanitize_resolution_context(
    currency_code: Option<String>,
    region_id: Option<String>,
    price_list_id: Option<String>,
    quantity: Option<i32>,
) -> Result<Option<PricingResolutionContext>, ServerFnError> {
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
            return Err(ServerFnError::new(
                "currency_code is required for pricing resolution context",
            ));
        }
        return Ok(None);
    };

    Ok(Some(PricingResolutionContext {
        currency_code,
        region_id: parse_optional_uuid_string(region_id, "region_id")?,
        price_list_id: parse_optional_uuid_string(price_list_id, "price_list_id")?,
        channel_id: None,
        channel_slug: None,
        quantity: parse_resolution_quantity(quantity)?,
    }))
}

#[cfg(feature = "ssr")]
fn ensure_permission(
    permissions: &[rustok_core::Permission],
    required: &[rustok_core::Permission],
    message: &str,
) -> Result<(), ServerFnError> {
    if !rustok_api::has_any_effective_permission(permissions, required) {
        return Err(ServerFnError::new(format!("Permission denied: {message}")));
    }

    Ok(())
}

#[cfg(feature = "ssr")]
fn parse_product_id(value: &str) -> Result<Uuid, ServerFnError> {
    Uuid::parse_str(value.trim()).map_err(|_| ServerFnError::new("Invalid product_id"))
}

#[cfg(feature = "ssr")]
fn map_adjustment_kind(kind: rustok_pricing::PriceAdjustmentKind) -> String {
    match kind {
        rustok_pricing::PriceAdjustmentKind::PercentageDiscount => {
            "percentage_discount".to_string()
        }
    }
}

#[cfg(feature = "ssr")]
fn map_adjustment_preview(
    value: rustok_pricing::PriceAdjustmentPreview,
) -> PricingAdjustmentPreview {
    PricingAdjustmentPreview {
        kind: map_adjustment_kind(value.kind),
        currency_code: value.currency_code,
        current_amount: value.current_amount.normalize().to_string(),
        base_amount: value.base_amount.normalize().to_string(),
        adjustment_percent: value.adjustment_percent.normalize().to_string(),
        adjusted_amount: value.adjusted_amount.normalize().to_string(),
        compare_at_amount: value
            .compare_at_amount
            .map(|item| item.normalize().to_string()),
        price_list_id: value.price_list_id.map(|item| item.to_string()),
        channel_id: value.channel_id.map(|item| item.to_string()),
        channel_slug: value.channel_slug,
    }
}

#[cfg(feature = "ssr")]
async fn update_variant_price_native_with_context(
    app_ctx: &loco_rs::app::AppContext,
    auth: &rustok_api::AuthContext,
    tenant: &rustok_api::TenantContext,
    variant_id: String,
    payload: PricingPriceDraft,
) -> Result<(), ServerFnError> {
    use rustok_core::Permission;
    use rustok_pricing::PricingService;

    ensure_permission(
        &auth.permissions,
        &[Permission::PRODUCTS_UPDATE],
        "products:update required",
    )?;

    let variant_id = parse_product_id(&variant_id)?;
    let currency_code = parse_required_currency_code(payload.currency_code)?;
    let amount = parse_decimal(&payload.amount, "amount")?;
    let compare_at_amount =
        parse_optional_decimal(&payload.compare_at_amount, "compare_at_amount")?;
    let price_list_id = parse_optional_uuid(&payload.price_list_id, "price_list_id")?;
    let channel_id = parse_optional_uuid(&payload.channel_id, "channel_id")?;
    let channel_slug = sanitize_channel_slug(text_or_none(payload.channel_slug));
    let min_quantity = parse_optional_quantity(&payload.min_quantity, "min_quantity")?;
    let max_quantity = parse_optional_quantity(&payload.max_quantity, "max_quantity")?;

    let service = PricingService::new(
        app_ctx.db.clone(),
        rustok_api::loco::transactional_event_bus_from_context(app_ctx),
    );
    if let Some(price_list_id) = price_list_id {
        service
            .set_price_list_tier_with_channel(
                tenant.id,
                auth.user_id,
                variant_id,
                price_list_id,
                currency_code.as_str(),
                amount,
                compare_at_amount,
                channel_id,
                channel_slug,
                min_quantity,
                max_quantity,
            )
            .await
            .map_err(ServerFnError::new)?;
    } else {
        service
            .set_price_tier_with_channel(
                tenant.id,
                auth.user_id,
                variant_id,
                currency_code.as_str(),
                amount,
                compare_at_amount,
                channel_id,
                channel_slug,
                min_quantity,
                max_quantity,
            )
            .await
            .map_err(ServerFnError::new)?;
    }

    Ok(())
}

#[cfg(feature = "ssr")]
async fn preview_variant_discount_native_with_context(
    app_ctx: &loco_rs::app::AppContext,
    auth: &rustok_api::AuthContext,
    tenant: &rustok_api::TenantContext,
    variant_id: String,
    payload: PricingDiscountDraft,
) -> Result<PricingAdjustmentPreview, ServerFnError> {
    use rustok_core::Permission;
    use rustok_pricing::PricingService;

    ensure_permission(
        &auth.permissions,
        &[Permission::PRODUCTS_READ, Permission::PRODUCTS_UPDATE],
        "products:read required",
    )?;

    let variant_id = parse_product_id(&variant_id)?;
    let currency_code = parse_required_currency_code(payload.currency_code)?;
    let discount_percent = parse_decimal(&payload.discount_percent, "discount_percent")?;
    let price_list_id = parse_optional_uuid(&payload.price_list_id, "price_list_id")?;
    let channel_id = parse_optional_uuid(&payload.channel_id, "channel_id")?;
    let channel_slug = sanitize_channel_slug(text_or_none(payload.channel_slug));
    let service = PricingService::new(
        app_ctx.db.clone(),
        rustok_api::loco::transactional_event_bus_from_context(app_ctx),
    );

    let preview = if let Some(price_list_id) = price_list_id {
        service
            .preview_price_list_percentage_discount_with_channel(
                tenant.id,
                variant_id,
                price_list_id,
                currency_code.as_str(),
                discount_percent,
                channel_id,
                channel_slug,
            )
            .await
    } else {
        service
            .preview_percentage_discount_with_channel(
                variant_id,
                currency_code.as_str(),
                discount_percent,
                channel_id,
                channel_slug,
            )
            .await
    };

    preview
        .map(map_adjustment_preview)
        .map_err(ServerFnError::new)
}

#[cfg(feature = "ssr")]
async fn apply_variant_discount_native_with_context(
    app_ctx: &loco_rs::app::AppContext,
    auth: &rustok_api::AuthContext,
    tenant: &rustok_api::TenantContext,
    variant_id: String,
    payload: PricingDiscountDraft,
) -> Result<PricingAdjustmentPreview, ServerFnError> {
    use rustok_core::Permission;
    use rustok_pricing::PricingService;

    ensure_permission(
        &auth.permissions,
        &[Permission::PRODUCTS_UPDATE],
        "products:update required",
    )?;

    let variant_id = parse_product_id(&variant_id)?;
    let currency_code = parse_required_currency_code(payload.currency_code)?;
    let discount_percent = parse_decimal(&payload.discount_percent, "discount_percent")?;
    let price_list_id = parse_optional_uuid(&payload.price_list_id, "price_list_id")?;
    let channel_id = parse_optional_uuid(&payload.channel_id, "channel_id")?;
    let channel_slug = sanitize_channel_slug(text_or_none(payload.channel_slug));
    let service = PricingService::new(
        app_ctx.db.clone(),
        rustok_api::loco::transactional_event_bus_from_context(app_ctx),
    );

    let preview = if let Some(price_list_id) = price_list_id {
        service
            .apply_price_list_percentage_discount_with_channel(
                tenant.id,
                auth.user_id,
                variant_id,
                price_list_id,
                currency_code.as_str(),
                discount_percent,
                channel_id,
                channel_slug,
            )
            .await
    } else {
        service
            .apply_percentage_discount_with_channel(
                tenant.id,
                auth.user_id,
                variant_id,
                currency_code.as_str(),
                discount_percent,
                channel_id,
                channel_slug,
            )
            .await
    };

    preview
        .map(map_adjustment_preview)
        .map_err(ServerFnError::new)
}

#[cfg(feature = "ssr")]
fn map_current_tenant(tenant: &rustok_api::TenantContext) -> CurrentTenant {
    CurrentTenant {
        id: tenant.id.to_string(),
        slug: tenant.slug.clone(),
        name: tenant.name.clone(),
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

#[cfg(feature = "ssr")]
async fn list_active_price_lists_native_with_context(
    app_ctx: &loco_rs::app::AppContext,
    auth: &rustok_api::AuthContext,
    tenant: &rustok_api::TenantContext,
    channel_id: Option<String>,
    channel_slug: Option<String>,
) -> Result<Vec<PricingPriceListOption>, ServerFnError> {
    use rustok_core::Permission;
    use rustok_pricing::PricingService;

    ensure_permission(
        &auth.permissions,
        &[Permission::PRODUCTS_LIST, Permission::PRODUCTS_READ],
        "products:list or products:read required",
    )?;

    let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
        .await
        .ok();
    let requested_locale = resolve_requested_locale(
        None,
        request_context.as_ref().map(|context| context.locale.as_str()),
        tenant.default_locale.as_str(),
    );
    let channel_id = parse_optional_uuid_string(channel_id, "channel_id")?.or_else(|| {
        request_context
            .as_ref()
            .and_then(|ctx| ctx.channel_id.map(|item| item.to_string()))
    });
    let channel_slug = sanitize_channel_slug(channel_slug).or_else(|| {
        request_context
            .as_ref()
            .and_then(|ctx| sanitize_channel_slug(ctx.channel_slug.clone()))
    });
    let parsed_channel_id = channel_id
        .as_deref()
        .and_then(|value| Uuid::parse_str(value).ok());
    let service = PricingService::new(
        app_ctx.db.clone(),
        rustok_api::loco::transactional_event_bus_from_context(app_ctx),
    );

    service
        .list_active_price_lists_for_channel(
            tenant.id,
            parsed_channel_id,
            channel_slug.as_deref(),
            Some(requested_locale.as_str()),
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(ServerFnError::new)
        .map(|items| {
            items
                .into_iter()
                .map(map_native_price_list_option)
                .collect()
        })
}

#[cfg(feature = "ssr")]
async fn update_price_list_rule_native_with_context(
    app_ctx: &loco_rs::app::AppContext,
    auth: &rustok_api::AuthContext,
    tenant: &rustok_api::TenantContext,
    price_list_id: String,
    payload: PricingPriceListRuleDraft,
) -> Result<PricingPriceListOption, ServerFnError> {
    use rustok_core::Permission;
    use rustok_pricing::PricingService;

    ensure_permission(
        &auth.permissions,
        &[Permission::PRODUCTS_UPDATE],
        "products:update required",
    )?;

    let price_list_id = Uuid::parse_str(price_list_id.trim())
        .map_err(|_| ServerFnError::new("Invalid price_list_id"))?;
    validate_active_price_list_for_rule_update(&app_ctx.db, tenant.id, price_list_id).await?;
    let adjustment_percent =
        parse_optional_decimal(&payload.adjustment_percent, "adjustment_percent")?;
    let service = PricingService::new(
        app_ctx.db.clone(),
        rustok_api::loco::transactional_event_bus_from_context(app_ctx),
    );
    service
        .set_price_list_percentage_rule(tenant.id, auth.user_id, price_list_id, adjustment_percent)
        .await
        .map_err(ServerFnError::new)?;

    let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
        .await
        .ok();
    let requested_locale = resolve_requested_locale(
        None,
        request_context.as_ref().map(|context| context.locale.as_str()),
        tenant.default_locale.as_str(),
    );
    let option = service
        .list_active_price_lists(
            tenant.id,
            Some(requested_locale.as_str()),
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(ServerFnError::new)?
        .into_iter()
        .find(|item| item.id == price_list_id)
        .ok_or_else(|| ServerFnError::new("price_list_id must reference an active price list"))?;

    Ok(map_native_price_list_option(option))
}

#[cfg(feature = "ssr")]
async fn validate_active_price_list_for_rule_update(
    db: &sea_orm::DatabaseConnection,
    tenant_id: Uuid,
    price_list_id: Uuid,
) -> Result<(), ServerFnError> {
    use sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

    let row = db
        .query_one(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "SELECT CASE
                WHEN lower(status) != 'active' THEN 'inactive'
                WHEN starts_at IS NOT NULL AND starts_at > CURRENT_TIMESTAMP THEN 'future'
                WHEN ends_at IS NOT NULL AND ends_at < CURRENT_TIMESTAMP THEN 'expired'
                ELSE 'active'
             END AS lifecycle
             FROM price_lists
             WHERE id = ? AND tenant_id = ?",
            vec![price_list_id.into(), tenant_id.into()],
        ))
        .await
        .map_err(ServerFnError::new)?
        .ok_or_else(|| ServerFnError::new("price_list_id was not found"))?;

    let lifecycle: String = row
        .try_get("", "lifecycle")
        .map_err(|err| ServerFnError::new(err.to_string()))?;

    match lifecycle.as_str() {
        "active" => Ok(()),
        "inactive" => Err(ServerFnError::new(
            "price_list_id must reference an active price list",
        )),
        "future" => Err(ServerFnError::new("price_list_id is not active yet")),
        "expired" => Err(ServerFnError::new("price_list_id is already expired")),
        _ => Err(ServerFnError::new(
            "price_list_id must reference an active price list",
        )),
    }
}

#[cfg(feature = "ssr")]
async fn update_price_list_scope_native_with_context(
    app_ctx: &loco_rs::app::AppContext,
    auth: &rustok_api::AuthContext,
    tenant: &rustok_api::TenantContext,
    price_list_id: String,
    payload: PricingPriceListScopeDraft,
) -> Result<PricingPriceListOption, ServerFnError> {
    use rustok_core::Permission;
    use rustok_pricing::PricingService;

    ensure_permission(
        &auth.permissions,
        &[Permission::PRODUCTS_UPDATE],
        "products:update required",
    )?;

    let price_list_id = Uuid::parse_str(price_list_id.trim())
        .map_err(|_| ServerFnError::new("Invalid price_list_id"))?;
    let channel_id = parse_optional_uuid(&payload.channel_id, "channel_id")?;
    let channel_slug = sanitize_channel_slug(text_or_none(payload.channel_slug));
    let service = PricingService::new(
        app_ctx.db.clone(),
        rustok_api::loco::transactional_event_bus_from_context(app_ctx),
    );

    service
        .set_price_list_scope(
            tenant.id,
            auth.user_id,
            price_list_id,
            channel_id,
            channel_slug,
        )
        .await
        .map(map_native_price_list_option)
        .map_err(ServerFnError::new)
}

#[cfg(feature = "ssr")]
fn map_native_list_item(
    value: rustok_pricing::AdminPricingProductListItem,
) -> PricingProductListItem {
    PricingProductListItem {
        id: value.id.to_string(),
        status: value.status.to_string(),
        seller_id: value.seller_id,
        title: value.title,
        handle: value.handle,
        vendor: value.vendor,
        product_type: value.product_type,
        shipping_profile_slug: value.shipping_profile_slug,
        tags: value.tags,
        created_at: value.created_at.to_rfc3339(),
        published_at: value.published_at.map(|item| item.to_rfc3339()),
    }
}

#[cfg(feature = "ssr")]
fn map_native_list(value: rustok_pricing::AdminPricingProductList) -> PricingProductList {
    PricingProductList {
        items: value.items.into_iter().map(map_native_list_item).collect(),
        total: value.total,
        page: value.page,
        per_page: value.per_page,
        has_next: value.has_next,
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
fn map_native_detail(value: rustok_pricing::AdminPricingProductDetail) -> PricingProductDetail {
    PricingProductDetail {
        id: value.id.to_string(),
        status: value.status.to_string(),
        seller_id: value.seller_id,
        vendor: value.vendor,
        product_type: value.product_type,
        shipping_profile_slug: value.shipping_profile_slug,
        created_at: value.created_at.to_rfc3339(),
        updated_at: value.updated_at.to_rfc3339(),
        published_at: value.published_at.map(|item| item.to_rfc3339()),
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
                sku: variant.sku,
                barcode: variant.barcode,
                shipping_profile_slug: variant.shipping_profile_slug,
                title: variant.title,
                option1: variant.option1,
                option2: variant.option2,
                option3: variant.option3,
                prices: variant
                    .prices
                    .into_iter()
                    .map(|price| crate::model::PricingPrice {
                        currency_code: price.currency_code,
                        amount: price.amount.normalize().to_string(),
                        compare_at_amount: price
                            .compare_at_amount
                            .map(|item| item.normalize().to_string()),
                        discount_percent: price
                            .discount_percent
                            .map(|item| item.normalize().to_string()),
                        on_sale: price.on_sale,
                        price_list_id: price.price_list_id.map(|item| item.to_string()),
                        channel_id: price.channel_id.map(|item| item.to_string()),
                        channel_slug: price.channel_slug,
                        min_quantity: price.min_quantity,
                        max_quantity: price.max_quantity,
                    })
                    .collect(),
                effective_price: None,
            })
            .collect(),
    }
}

#[server(prefix = "/api/fn", endpoint = "pricing/admin/bootstrap")]
async fn pricing_admin_bootstrap_native() -> Result<PricingAdminBootstrap, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};
        use rustok_channel::ChannelService;
        use rustok_core::Permission;
        use rustok_pricing::PricingService;

        let app_ctx = expect_context::<AppContext>();

        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .ok();

        ensure_permission(
            &auth.permissions,
            &[Permission::PRODUCTS_LIST, Permission::PRODUCTS_READ],
            "products:list or products:read required",
        )?;

        let service = PricingService::new(
            app_ctx.db.clone(),
            rustok_api::loco::transactional_event_bus_from_context(&app_ctx),
        );
        let channel_service = ChannelService::new(app_ctx.db.clone());
        let channel_slug = request_context
            .as_ref()
            .and_then(|ctx| sanitize_channel_slug(ctx.channel_slug.clone()));
        let (available_channels, _) = channel_service
            .list_channels(tenant.id, 1, 250)
            .await
            .map_err(ServerFnError::new)?;
        let active_price_lists = service
            .list_active_price_lists_for_channel(
                tenant.id,
                request_context.as_ref().and_then(|ctx| ctx.channel_id),
                channel_slug.as_deref(),
                request_context
                    .as_ref()
                    .map(|context| context.locale.as_str()),
                Some(tenant.default_locale.as_str()),
            )
            .await
            .map_err(ServerFnError::new)?
            .into_iter()
            .map(map_native_price_list_option)
            .collect();

        Ok(PricingAdminBootstrap {
            current_tenant: map_current_tenant(&tenant),
            available_channels: available_channels
                .into_iter()
                .map(map_channel_option)
                .collect(),
            active_price_lists,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "pricing/admin/bootstrap requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "pricing/admin/active-price-lists")]
async fn pricing_admin_active_price_lists_native(
    channel_id: Option<String>,
    channel_slug: Option<String>,
) -> Result<Vec<PricingPriceListOption>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        list_active_price_lists_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            channel_id,
            channel_slug,
        )
        .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (channel_id, channel_slug);
        Err(ServerFnError::new(
            "pricing/admin/active-price-lists requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "pricing/admin/products")]
async fn pricing_admin_products_native(
    locale: String,
    search: Option<String>,
    status: Option<String>,
) -> Result<PricingProductList, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};
        use rustok_core::Permission;
        use rustok_pricing::PricingService;

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        ensure_permission(
            &auth.permissions,
            &[Permission::PRODUCTS_LIST],
            "products:list required",
        )?;

        let requested_locale =
            resolve_requested_locale(Some(locale), None, tenant.default_locale.as_str());
        let service = PricingService::new(
            app_ctx.db.clone(),
            rustok_api::loco::transactional_event_bus_from_context(&app_ctx),
        );
        let status = status
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(|value| match value {
                "ACTIVE" => rustok_commerce_foundation::entities::product::ProductStatus::Active,
                "ARCHIVED" => {
                    rustok_commerce_foundation::entities::product::ProductStatus::Archived
                }
                _ => rustok_commerce_foundation::entities::product::ProductStatus::Draft,
            });
        let data = service
            .list_admin_product_pricing_with_locale_fallback(
                tenant.id,
                requested_locale.as_str(),
                Some(tenant.default_locale.as_str()),
                search.as_deref(),
                status,
                1,
                24,
            )
            .await
            .map_err(ServerFnError::new)?;

        Ok(map_native_list(data))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (locale, search, status);
        Err(ServerFnError::new(
            "pricing/admin/products requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "pricing/admin/product")]
async fn pricing_admin_product_native(
    product_id: String,
    locale: String,
    currency_code: Option<String>,
    region_id: Option<String>,
    price_list_id: Option<String>,
    channel_id: Option<String>,
    channel_slug: Option<String>,
    quantity: Option<i32>,
) -> Result<Option<PricingProductDetail>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};
        use rustok_core::Permission;
        use rustok_pricing::{PriceResolutionContext, PricingService};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .ok();

        ensure_permission(
            &auth.permissions,
            &[Permission::PRODUCTS_READ],
            "products:read required",
        )?;

        let requested_locale =
            resolve_requested_locale(Some(locale), None, tenant.default_locale.as_str());
        let explicit_channel_id = parse_optional_uuid_string(channel_id, "channel_id")?;
        let mut resolution_context = sanitize_resolution_context(
            currency_code.clone(),
            region_id.clone(),
            price_list_id.clone(),
            quantity,
        )?;
        if let Some(context) = resolution_context.as_mut() {
            context.channel_id = explicit_channel_id.or_else(|| {
                request_context
                    .as_ref()
                    .and_then(|ctx| ctx.channel_id.map(|item| item.to_string()))
            });
            context.channel_slug = sanitize_channel_slug(channel_slug).or_else(|| {
                request_context
                    .as_ref()
                    .and_then(|ctx| sanitize_channel_slug(ctx.channel_slug.clone()))
            });
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
            rustok_api::loco::transactional_event_bus_from_context(&app_ctx),
        );
        let product_id = parse_product_id(&product_id)?;
        let mut detail = match service
            .get_admin_product_pricing_with_locale_fallback(
                tenant.id,
                product_id,
                requested_locale.as_str(),
                Some(tenant.default_locale.as_str()),
                native_resolution_context
                    .as_ref()
                    .and_then(|context| context.price_list_id),
            )
            .await
        {
            Ok(detail) => Some(map_native_detail(detail)),
            Err(rustok_commerce_foundation::error::CommerceError::ProductNotFound(_)) => None,
            Err(err) => return Err(ServerFnError::new(err)),
        };

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

        Ok(detail)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (
            product_id,
            locale,
            currency_code,
            region_id,
            price_list_id,
            channel_id,
            channel_slug,
            quantity,
        );
        Err(ServerFnError::new(
            "pricing/admin/product requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "pricing/admin/update-variant-price")]
async fn pricing_admin_update_variant_price_native(
    variant_id: String,
    payload: PricingPriceDraft,
) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        update_variant_price_native_with_context(&app_ctx, &auth, &tenant, variant_id, payload)
            .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (variant_id, payload);
        Err(ServerFnError::new(
            "pricing/admin/update-variant-price requires the `ssr` feature",
        ))
    }
}

#[server(
    prefix = "/api/fn",
    endpoint = "pricing/admin/preview-variant-discount"
)]
async fn pricing_admin_preview_variant_discount_native(
    variant_id: String,
    payload: PricingDiscountDraft,
) -> Result<PricingAdjustmentPreview, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        preview_variant_discount_native_with_context(&app_ctx, &auth, &tenant, variant_id, payload)
            .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (variant_id, payload);
        Err(ServerFnError::new(
            "pricing/admin/preview-variant-discount requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "pricing/admin/apply-variant-discount")]
async fn pricing_admin_apply_variant_discount_native(
    variant_id: String,
    payload: PricingDiscountDraft,
) -> Result<PricingAdjustmentPreview, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        apply_variant_discount_native_with_context(&app_ctx, &auth, &tenant, variant_id, payload)
            .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (variant_id, payload);
        Err(ServerFnError::new(
            "pricing/admin/apply-variant-discount requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "pricing/admin/update-price-list-rule")]
async fn pricing_admin_update_price_list_rule_native(
    price_list_id: String,
    payload: PricingPriceListRuleDraft,
) -> Result<PricingPriceListOption, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        update_price_list_rule_native_with_context(&app_ctx, &auth, &tenant, price_list_id, payload)
            .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (price_list_id, payload);
        Err(ServerFnError::new(
            "pricing/admin/update-price-list-rule requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "pricing/admin/update-price-list-scope")]
async fn pricing_admin_update_price_list_scope_native(
    price_list_id: String,
    payload: PricingPriceListScopeDraft,
) -> Result<PricingPriceListOption, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, TenantContext};

        let app_ctx = expect_context::<AppContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(ServerFnError::new)?;

        update_price_list_scope_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            price_list_id,
            payload,
        )
        .await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (price_list_id, payload);
        Err(ServerFnError::new(
            "pricing/admin/update-price-list-scope requires the `ssr` feature",
        ))
    }
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::*;
    use loco_rs::app::{AppContext, SharedStore};
    use loco_rs::cache;
    use loco_rs::environment::Environment;
    use loco_rs::storage::{self, Storage};
    use loco_rs::tests_cfg::config::test_config;
    use rustok_api::{AuthContext, TenantContext};
    use rustok_commerce_foundation::dto::{
        CreateProductInput, CreateVariantInput, PriceInput, ProductTranslationInput,
    };
    use rustok_core::events::EventTransport;
    use rustok_core::Permission;
    use rustok_product::CatalogService;
    use rustok_test_utils::db::setup_test_db;
    use rustok_test_utils::{mock_transactional_event_bus, MockEventTransport};
    use sea_orm::{ColumnTrait, ConnectionTrait, EntityTrait, PaginatorTrait, QueryFilter};
    use serde_json::json;
    use std::str::FromStr;
    use std::sync::Arc;

    mod support {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../rustok-commerce/tests/support.rs"
        ));
    }

    fn test_app_context(db: sea_orm::DatabaseConnection) -> AppContext {
        let shared_store = Arc::new(SharedStore::default());
        let event_transport: Arc<dyn EventTransport> = Arc::new(MockEventTransport::new());
        shared_store.insert(event_transport);

        AppContext {
            environment: Environment::Test,
            db,
            queue_provider: None,
            config: test_config(),
            mailer: None,
            storage: Storage::single(storage::drivers::mem::new()).into(),
            cache: Arc::new(cache::Cache::new(cache::drivers::null::new())),
            shared_store,
        }
    }

    async fn seed_tenant_context(db: &sea_orm::DatabaseConnection, tenant_id: Uuid) {
        db.execute(sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Sqlite,
            "INSERT INTO tenants (id, name, slug, domain, settings, default_locale, is_active, created_at, updated_at)
             VALUES (?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            vec![
                tenant_id.into(),
                "Pricing Admin Test Tenant".into(),
                format!("pricing-admin-test-{tenant_id}").into(),
                sea_orm::Value::String(None),
                json!({}).to_string().into(),
                "en".into(),
                true.into(),
            ],
        ))
        .await
        .expect("tenant should be inserted");
    }

    async fn create_test_product(
        catalog: &CatalogService,
        tenant_id: Uuid,
        actor_id: Uuid,
    ) -> (Uuid, Uuid) {
        let product = catalog
            .create_product(
                tenant_id,
                actor_id,
                CreateProductInput {
                    translations: vec![ProductTranslationInput {
                        locale: "en".to_string(),
                        title: "Tiered Pricing Product".to_string(),
                        description: Some("Admin pricing transport test".to_string()),
                        handle: Some(format!("tiered-pricing-{tenant_id}")),
                        meta_title: None,
                        meta_description: None,
                    }],
                    options: vec![],
                    variants: vec![CreateVariantInput {
                        sku: Some("PRICE-TIER-1".to_string()),
                        barcode: None,
                        shipping_profile_slug: None,
                        option1: Some("Default".to_string()),
                        option2: None,
                        option3: None,
                        prices: vec![PriceInput {
                            currency_code: "USD".to_string(),
                            channel_id: None,
                            channel_slug: None,
                            amount: rust_decimal::Decimal::from_str("100.00")
                                .expect("valid decimal"),
                            compare_at_amount: None,
                        }],
                        inventory_quantity: 0,
                        inventory_policy: "deny".to_string(),
                        weight: None,
                        weight_unit: None,
                    }],
                    seller_id: None,
                    vendor: Some("Pricing Vendor".to_string()),
                    product_type: Some("Physical".to_string()),
                    shipping_profile_slug: None,
                    tags: vec![],
                    publish: false,
                    metadata: json!({}),
                },
            )
            .await
            .expect("product should be created");

        (product.id, product.variants[0].id)
    }

    async fn create_price_list(
        db: &sea_orm::DatabaseConnection,
        tenant_id: Uuid,
        status: &str,
    ) -> Uuid {
        let price_list_id = Uuid::new_v4();
        db.execute(sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Sqlite,
            "INSERT INTO price_lists (
                id,
                tenant_id,
                type,
                status,
                starts_at,
                ends_at,
                created_at,
                updated_at
            ) VALUES (?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            vec![
                price_list_id.into(),
                tenant_id.into(),
                "sale".into(),
                status.to_string().into(),
                sea_orm::Value::String(None),
                sea_orm::Value::String(None),
            ],
        ))
        .await
        .expect("price list should be inserted");

        db.execute(sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Sqlite,
            "INSERT INTO price_list_translations (
                id,
                price_list_id,
                locale,
                name,
                description
            ) VALUES (?, ?, ?, ?, ?)",
            vec![
                Uuid::new_v4().into(),
                price_list_id.into(),
                "en".into(),
                format!("Admin List-{price_list_id}").into(),
                Some("Admin pricing transport test list".to_string()).into(),
            ],
        ))
        .await
        .expect("price list translation should be inserted");

        price_list_id
    }

    async fn create_future_price_list(
        db: &sea_orm::DatabaseConnection,
        tenant_id: Uuid,
        status: &str,
    ) -> Uuid {
        let price_list_id = Uuid::new_v4();
        db.execute(sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Sqlite,
            "INSERT INTO price_lists (
                id,
                tenant_id,
                type,
                status,
                starts_at,
                ends_at,
                created_at,
                updated_at
            ) VALUES (?, ?, ?, ?, datetime('now', '+1 day'), NULL, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            vec![
                price_list_id.into(),
                tenant_id.into(),
                "sale".into(),
                status.to_string().into(),
            ],
        ))
        .await
        .expect("price list should be inserted");

        db.execute(sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Sqlite,
            "INSERT INTO price_list_translations (
                id,
                price_list_id,
                locale,
                name,
                description
            ) VALUES (?, ?, ?, ?, ?)",
            vec![
                Uuid::new_v4().into(),
                price_list_id.into(),
                "en".into(),
                format!("Admin List-{price_list_id}").into(),
                Some("Admin pricing transport test list".to_string()).into(),
            ],
        ))
        .await
        .expect("price list translation should be inserted");

        price_list_id
    }

    async fn create_expired_price_list(
        db: &sea_orm::DatabaseConnection,
        tenant_id: Uuid,
        status: &str,
    ) -> Uuid {
        let price_list_id = Uuid::new_v4();
        db.execute(sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Sqlite,
            "INSERT INTO price_lists (
                id,
                tenant_id,
                type,
                status,
                starts_at,
                ends_at,
                created_at,
                updated_at
            ) VALUES (?, ?, ?, ?, NULL, datetime('now', '-1 day'), CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
            vec![
                price_list_id.into(),
                tenant_id.into(),
                "sale".into(),
                status.to_string().into(),
            ],
        ))
        .await
        .expect("price list should be inserted");

        db.execute(sea_orm::Statement::from_sql_and_values(
            sea_orm::DatabaseBackend::Sqlite,
            "INSERT INTO price_list_translations (
                id,
                price_list_id,
                locale,
                name,
                description
            ) VALUES (?, ?, ?, ?, ?)",
            vec![
                Uuid::new_v4().into(),
                price_list_id.into(),
                "en".into(),
                format!("Admin List-{price_list_id}").into(),
                Some("Admin pricing transport test list".to_string()).into(),
            ],
        ))
        .await
        .expect("price list translation should be inserted");

        price_list_id
    }

    #[test]
    fn admin_pricing_resolution_context_rejects_non_letter_currency_code() {
        let error = sanitize_resolution_context(Some("US1".to_string()), None, None, Some(1))
            .expect_err("invalid currency should be rejected");

        assert!(error
            .to_string()
            .contains("currency_code must be a 3-letter code"));
    }

    #[test]
    fn admin_pricing_resolution_context_rejects_non_positive_quantity() {
        let error = sanitize_resolution_context(Some("USD".to_string()), None, None, Some(0))
            .expect_err("invalid quantity should be rejected");

        assert!(error.to_string().contains("quantity must be at least 1"));
    }

    #[test]
    fn admin_pricing_resolution_context_rejects_modifiers_without_currency_code() {
        let error = sanitize_resolution_context(None, None, Some(Uuid::new_v4().to_string()), None)
            .expect_err("price_list_id without currency should be rejected");

        assert!(error
            .to_string()
            .contains("currency_code is required for pricing resolution context"));
    }

    #[test]
    fn admin_pricing_write_scope_rejects_invalid_price_list_id() {
        let error = parse_optional_uuid("not-a-uuid", "price_list_id")
            .expect_err("invalid price_list_id should be rejected");

        assert!(error.to_string().contains("Invalid price_list_id"));
    }

    #[test]
    fn admin_pricing_transport_rejects_invalid_channel_id() {
        let error = parse_optional_uuid_string(Some("not-a-uuid".to_string()), "channel_id")
            .expect_err("invalid channel_id should be rejected");

        assert!(error.to_string().contains("Invalid channel_id"));
    }

    #[tokio::test]
    async fn admin_pricing_native_update_variant_price_supports_quantity_tiers() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Pricing Admin Test Tenant".to_string(),
            slug: format!("pricing-admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_UPDATE, Permission::PRODUCTS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db.clone());
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let (product_id, variant_id) = create_test_product(&catalog, tenant_id, actor_id).await;

        update_variant_price_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            variant_id.to_string(),
            PricingPriceDraft {
                currency_code: "usd".to_string(),
                amount: "85.00".to_string(),
                compare_at_amount: "100.00".to_string(),
                price_list_id: "".to_string(),
                channel_id: "".to_string(),
                channel_slug: "".to_string(),
                min_quantity: "10".to_string(),
                max_quantity: "".to_string(),
            },
        )
        .await
        .expect("native update should succeed");

        let detail = rustok_pricing::PricingService::new(db, mock_transactional_event_bus())
            .get_admin_product_pricing_with_locale_fallback(
                tenant_id,
                product_id,
                "en",
                Some("en"),
                None,
            )
            .await
            .expect("admin detail should load");

        let variant = detail
            .variants
            .into_iter()
            .find(|variant| variant.id == variant_id)
            .expect("variant should be present");
        let tier = variant
            .prices
            .into_iter()
            .find(|price| price.currency_code == "USD" && price.min_quantity == Some(10))
            .expect("tier price should be present");

        assert_eq!(
            tier.amount,
            rust_decimal::Decimal::from_str("85.00").expect("valid decimal")
        );
        assert_eq!(
            tier.compare_at_amount,
            Some(rust_decimal::Decimal::from_str("100.00").expect("valid decimal"))
        );
        assert_eq!(tier.max_quantity, None);
    }

    #[tokio::test]
    async fn admin_pricing_native_update_variant_price_supports_active_price_list_overrides() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        let price_list_id = create_price_list(&db, tenant_id, "active").await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Pricing Admin Test Tenant".to_string(),
            slug: format!("pricing-admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_UPDATE, Permission::PRODUCTS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db.clone());
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let (product_id, variant_id) = create_test_product(&catalog, tenant_id, actor_id).await;

        update_variant_price_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            variant_id.to_string(),
            PricingPriceDraft {
                currency_code: "usd".to_string(),
                amount: "79.00".to_string(),
                compare_at_amount: "100.00".to_string(),
                price_list_id: price_list_id.to_string(),
                channel_id: "".to_string(),
                channel_slug: "".to_string(),
                min_quantity: "2".to_string(),
                max_quantity: "5".to_string(),
            },
        )
        .await
        .expect("native price list override should succeed");

        let detail = rustok_pricing::PricingService::new(db, mock_transactional_event_bus())
            .get_admin_product_pricing_with_locale_fallback(
                tenant_id,
                product_id,
                "en",
                Some("en"),
                Some(price_list_id),
            )
            .await
            .expect("admin detail should load");

        let variant = detail
            .variants
            .into_iter()
            .find(|variant| variant.id == variant_id)
            .expect("variant should be present");
        let override_price = variant
            .prices
            .into_iter()
            .find(|price| {
                price.currency_code == "USD"
                    && price.price_list_id == Some(price_list_id)
                    && price.min_quantity == Some(2)
                    && price.max_quantity == Some(5)
            })
            .expect("price list override should be present");

        assert_eq!(
            override_price.amount,
            rust_decimal::Decimal::from_str("79.00").expect("valid decimal")
        );
        assert_eq!(
            override_price.compare_at_amount,
            Some(rust_decimal::Decimal::from_str("100.00").expect("valid decimal"))
        );
    }

    #[tokio::test]
    async fn admin_pricing_native_update_variant_price_supports_channel_scoped_base_rows() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let channel_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Pricing Admin Test Tenant".to_string(),
            slug: format!("pricing-admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_UPDATE, Permission::PRODUCTS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db.clone());
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let (product_id, variant_id) = create_test_product(&catalog, tenant_id, actor_id).await;

        update_variant_price_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            variant_id.to_string(),
            PricingPriceDraft {
                currency_code: "usd".to_string(),
                amount: "79.00".to_string(),
                compare_at_amount: "100.00".to_string(),
                price_list_id: "".to_string(),
                channel_id: channel_id.to_string(),
                channel_slug: "web-store".to_string(),
                min_quantity: "".to_string(),
                max_quantity: "".to_string(),
            },
        )
        .await
        .expect("native update should support channel scope");

        let detail = rustok_pricing::PricingService::new(db, mock_transactional_event_bus())
            .get_admin_product_pricing_with_locale_fallback(
                tenant_id,
                product_id,
                "en",
                Some("en"),
                None,
            )
            .await
            .expect("admin detail should load");

        let variant = detail
            .variants
            .into_iter()
            .find(|variant| variant.id == variant_id)
            .expect("variant should be present");
        let scoped_price = variant
            .prices
            .into_iter()
            .find(|price| {
                price.currency_code == "USD"
                    && price.channel_id == Some(channel_id)
                    && price.channel_slug.as_deref() == Some("web-store")
            })
            .expect("channel-scoped row should be present");

        assert_eq!(
            scoped_price.amount,
            rust_decimal::Decimal::from_str("79.00").expect("valid decimal")
        );
        assert_eq!(
            scoped_price.compare_at_amount,
            Some(rust_decimal::Decimal::from_str("100.00").expect("valid decimal"))
        );
    }

    #[tokio::test]
    async fn admin_pricing_native_update_variant_price_persists_decimal_and_legacy_cents() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Pricing Admin Test Tenant".to_string(),
            slug: format!("pricing-admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_UPDATE, Permission::PRODUCTS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db.clone());
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let (_product_id, variant_id) = create_test_product(&catalog, tenant_id, actor_id).await;

        update_variant_price_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            variant_id.to_string(),
            PricingPriceDraft {
                currency_code: "usd".to_string(),
                amount: "79.994".to_string(),
                compare_at_amount: "99.996".to_string(),
                price_list_id: "".to_string(),
                channel_id: "".to_string(),
                channel_slug: "".to_string(),
                min_quantity: "".to_string(),
                max_quantity: "".to_string(),
            },
        )
        .await
        .expect("fractional update should succeed");

        let price = rustok_commerce::entities::price::Entity::find()
            .filter(rustok_commerce::entities::price::Column::VariantId.eq(variant_id))
            .filter(rustok_commerce::entities::price::Column::CurrencyCode.eq("USD"))
            .filter(rustok_commerce::entities::price::Column::PriceListId.is_null())
            .filter(rustok_commerce::entities::price::Column::ChannelId.is_null())
            .filter(rustok_commerce::entities::price::Column::MinQuantity.is_null())
            .filter(rustok_commerce::entities::price::Column::MaxQuantity.is_null())
            .one(&db)
            .await
            .expect("price row query should succeed")
            .expect("base row should exist");

        assert_eq!(
            price.amount,
            rust_decimal::Decimal::from_str("79.994").expect("valid decimal")
        );
        assert_eq!(
            price.compare_at_amount,
            Some(rust_decimal::Decimal::from_str("99.996").expect("valid decimal"))
        );
        assert_eq!(price.legacy_amount, Some(7999));
        assert_eq!(price.legacy_compare_at_amount, Some(10000));
    }

    #[tokio::test]
    async fn admin_pricing_native_update_variant_price_clears_compare_at_on_existing_row() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Pricing Admin Test Tenant".to_string(),
            slug: format!("pricing-admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_UPDATE, Permission::PRODUCTS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db.clone());
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let (_product_id, variant_id) = create_test_product(&catalog, tenant_id, actor_id).await;

        update_variant_price_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            variant_id.to_string(),
            PricingPriceDraft {
                currency_code: "usd".to_string(),
                amount: "79.99".to_string(),
                compare_at_amount: "99.99".to_string(),
                price_list_id: "".to_string(),
                channel_id: "".to_string(),
                channel_slug: "".to_string(),
                min_quantity: "".to_string(),
                max_quantity: "".to_string(),
            },
        )
        .await
        .expect("initial update should succeed");

        update_variant_price_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            variant_id.to_string(),
            PricingPriceDraft {
                currency_code: "usd".to_string(),
                amount: "89.99".to_string(),
                compare_at_amount: "".to_string(),
                price_list_id: "".to_string(),
                channel_id: "".to_string(),
                channel_slug: "".to_string(),
                min_quantity: "".to_string(),
                max_quantity: "".to_string(),
            },
        )
        .await
        .expect("compare_at clear should succeed");

        let price = rustok_commerce::entities::price::Entity::find()
            .filter(rustok_commerce::entities::price::Column::VariantId.eq(variant_id))
            .filter(rustok_commerce::entities::price::Column::CurrencyCode.eq("USD"))
            .filter(rustok_commerce::entities::price::Column::PriceListId.is_null())
            .filter(rustok_commerce::entities::price::Column::ChannelId.is_null())
            .filter(rustok_commerce::entities::price::Column::MinQuantity.is_null())
            .filter(rustok_commerce::entities::price::Column::MaxQuantity.is_null())
            .one(&db)
            .await
            .expect("price row query should succeed")
            .expect("base row should exist");

        assert_eq!(
            price.amount,
            rust_decimal::Decimal::from_str("89.99").expect("valid decimal")
        );
        assert_eq!(price.compare_at_amount, None);
        assert_eq!(price.legacy_amount, Some(8999));
        assert_eq!(price.legacy_compare_at_amount, None);
    }

    #[tokio::test]
    async fn admin_pricing_native_update_variant_price_requires_products_update_permission() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Pricing Admin Test Tenant".to_string(),
            slug: format!("pricing-admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db.clone());
        let catalog = CatalogService::new(db, mock_transactional_event_bus());
        let (_product_id, variant_id) = create_test_product(&catalog, tenant_id, actor_id).await;

        let error = update_variant_price_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            variant_id.to_string(),
            PricingPriceDraft {
                currency_code: "USD".to_string(),
                amount: "85.00".to_string(),
                compare_at_amount: "".to_string(),
                price_list_id: "".to_string(),
                channel_id: "".to_string(),
                channel_slug: "".to_string(),
                min_quantity: "".to_string(),
                max_quantity: "".to_string(),
            },
        )
        .await
        .expect_err("permission should be required");

        assert!(error.to_string().contains("products:update required"));
    }

    #[tokio::test]
    async fn admin_pricing_native_update_variant_price_rejects_compare_at_below_amount_without_mutating_base_row(
    ) {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Pricing Admin Test Tenant".to_string(),
            slug: format!("pricing-admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_UPDATE, Permission::PRODUCTS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db.clone());
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let (product_id, variant_id) = create_test_product(&catalog, tenant_id, actor_id).await;

        let error = update_variant_price_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            variant_id.to_string(),
            PricingPriceDraft {
                currency_code: "usd".to_string(),
                amount: "120.00".to_string(),
                compare_at_amount: "110.00".to_string(),
                price_list_id: "".to_string(),
                channel_id: "".to_string(),
                channel_slug: "".to_string(),
                min_quantity: "".to_string(),
                max_quantity: "".to_string(),
            },
        )
        .await
        .expect_err("invalid compare_at should be rejected");

        assert!(error
            .to_string()
            .contains("Compare at price must be greater than amount"));

        let detail = rustok_pricing::PricingService::new(db, mock_transactional_event_bus())
            .get_admin_product_pricing_with_locale_fallback(
                tenant_id,
                product_id,
                "en",
                Some("en"),
                None,
            )
            .await
            .expect("admin detail should load");

        let variant = detail
            .variants
            .into_iter()
            .find(|variant| variant.id == variant_id)
            .expect("variant should be present");
        let base_price = variant
            .prices
            .into_iter()
            .find(|price| {
                price.currency_code == "USD"
                    && price.price_list_id.is_none()
                    && price.channel_id.is_none()
                    && price.min_quantity.is_none()
                    && price.max_quantity.is_none()
            })
            .expect("base price should exist");

        assert_eq!(
            base_price.amount,
            rust_decimal::Decimal::from_str("100.00").expect("valid decimal")
        );
        assert_eq!(base_price.compare_at_amount, None);
    }

    #[tokio::test]
    async fn admin_pricing_native_update_variant_price_rejects_compare_at_below_amount_without_mutating_price_list_override(
    ) {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        let price_list_id = create_price_list(&db, tenant_id, "active").await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Pricing Admin Test Tenant".to_string(),
            slug: format!("pricing-admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_UPDATE, Permission::PRODUCTS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db.clone());
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let (product_id, variant_id) = create_test_product(&catalog, tenant_id, actor_id).await;
        let pricing =
            rustok_pricing::PricingService::new(db.clone(), mock_transactional_event_bus());

        pricing
            .set_price_list_tier(
                tenant_id,
                actor_id,
                variant_id,
                price_list_id,
                "USD",
                rust_decimal::Decimal::from_str("79.00").expect("valid decimal"),
                Some(rust_decimal::Decimal::from_str("100.00").expect("valid decimal")),
                None,
                None,
            )
            .await
            .expect("valid override should be stored");

        let error = update_variant_price_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            variant_id.to_string(),
            PricingPriceDraft {
                currency_code: "usd".to_string(),
                amount: "120.00".to_string(),
                compare_at_amount: "110.00".to_string(),
                price_list_id: price_list_id.to_string(),
                channel_id: "".to_string(),
                channel_slug: "".to_string(),
                min_quantity: "".to_string(),
                max_quantity: "".to_string(),
            },
        )
        .await
        .expect_err("invalid compare_at should be rejected for override");

        assert!(error
            .to_string()
            .contains("Compare at price must be greater than amount"));

        let detail = rustok_pricing::PricingService::new(db, mock_transactional_event_bus())
            .get_admin_product_pricing_with_locale_fallback(
                tenant_id,
                product_id,
                "en",
                Some("en"),
                Some(price_list_id),
            )
            .await
            .expect("admin detail should load");

        let variant = detail
            .variants
            .into_iter()
            .find(|variant| variant.id == variant_id)
            .expect("variant should be present");
        let override_price = variant
            .prices
            .into_iter()
            .find(|price| {
                price.currency_code == "USD"
                    && price.price_list_id == Some(price_list_id)
                    && price.channel_id.is_none()
                    && price.min_quantity.is_none()
                    && price.max_quantity.is_none()
            })
            .expect("price list override should exist");

        assert_eq!(
            override_price.amount,
            rust_decimal::Decimal::from_str("79.00").expect("valid decimal")
        );
        assert_eq!(
            override_price.compare_at_amount,
            Some(rust_decimal::Decimal::from_str("100.00").expect("valid decimal"))
        );
    }

    #[tokio::test]
    async fn admin_pricing_native_preview_variant_discount_returns_typed_preview() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Pricing Admin Test Tenant".to_string(),
            slug: format!("pricing-admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db.clone());
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let (_product_id, variant_id) = create_test_product(&catalog, tenant_id, actor_id).await;

        let preview = preview_variant_discount_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            variant_id.to_string(),
            PricingDiscountDraft {
                currency_code: "usd".to_string(),
                discount_percent: "10".to_string(),
                price_list_id: "".to_string(),
                channel_id: "".to_string(),
                channel_slug: "".to_string(),
            },
        )
        .await
        .expect("preview should succeed");

        assert_eq!(preview.kind, "percentage_discount");
        assert_eq!(preview.currency_code, "USD");
        assert_eq!(preview.base_amount, "100");
        assert_eq!(preview.adjusted_amount, "90");
        assert_eq!(preview.adjustment_percent, "10");
    }

    #[tokio::test]
    async fn admin_pricing_native_apply_variant_discount_updates_base_row_only() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Pricing Admin Test Tenant".to_string(),
            slug: format!("pricing-admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_READ, Permission::PRODUCTS_UPDATE],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db.clone());
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let (product_id, variant_id) = create_test_product(&catalog, tenant_id, actor_id).await;
        let pricing =
            rustok_pricing::PricingService::new(db.clone(), mock_transactional_event_bus());

        pricing
            .set_price_tier(
                tenant_id,
                actor_id,
                variant_id,
                "USD",
                rust_decimal::Decimal::from_str("85.00").expect("valid decimal"),
                Some(rust_decimal::Decimal::from_str("100.00").expect("valid decimal")),
                Some(10),
                None,
            )
            .await
            .expect("tier should be created");

        let preview = apply_variant_discount_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            variant_id.to_string(),
            PricingDiscountDraft {
                currency_code: "USD".to_string(),
                discount_percent: "10".to_string(),
                price_list_id: "".to_string(),
                channel_id: "".to_string(),
                channel_slug: "".to_string(),
            },
        )
        .await
        .expect("apply should succeed");

        assert_eq!(preview.adjusted_amount, "90");

        let detail = rustok_pricing::PricingService::new(db, mock_transactional_event_bus())
            .get_admin_product_pricing_with_locale_fallback(
                tenant_id,
                product_id,
                "en",
                Some("en"),
                None,
            )
            .await
            .expect("admin detail should load");

        let variant = detail
            .variants
            .into_iter()
            .find(|variant| variant.id == variant_id)
            .expect("variant should be present");
        let base_price = variant
            .prices
            .iter()
            .find(|price| {
                price.currency_code == "USD"
                    && price.price_list_id.is_none()
                    && price.min_quantity.is_none()
                    && price.max_quantity.is_none()
            })
            .expect("base price should exist");
        let tier_price = variant
            .prices
            .iter()
            .find(|price| price.currency_code == "USD" && price.min_quantity == Some(10))
            .expect("tier price should exist");

        assert_eq!(
            base_price.amount,
            rust_decimal::Decimal::from_str("90.00").expect("valid decimal")
        );
        assert_eq!(
            tier_price.amount,
            rust_decimal::Decimal::from_str("85.00").expect("valid decimal")
        );
    }

    #[tokio::test]
    async fn admin_pricing_native_apply_variant_discount_targets_channel_scoped_base_row_only() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let channel_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Pricing Admin Test Tenant".to_string(),
            slug: format!("pricing-admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_READ, Permission::PRODUCTS_UPDATE],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db.clone());
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let (product_id, variant_id) = create_test_product(&catalog, tenant_id, actor_id).await;
        let pricing =
            rustok_pricing::PricingService::new(db.clone(), mock_transactional_event_bus());

        pricing
            .set_price_tier_with_channel(
                tenant_id,
                actor_id,
                variant_id,
                "USD",
                rust_decimal::Decimal::from_str("80.00").expect("valid decimal"),
                Some(rust_decimal::Decimal::from_str("100.00").expect("valid decimal")),
                Some(channel_id),
                Some("web-store".to_string()),
                None,
                None,
            )
            .await
            .expect("channel scoped base row should be created");

        let preview = apply_variant_discount_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            variant_id.to_string(),
            PricingDiscountDraft {
                currency_code: "USD".to_string(),
                discount_percent: "10".to_string(),
                price_list_id: "".to_string(),
                channel_id: channel_id.to_string(),
                channel_slug: "web-store".to_string(),
            },
        )
        .await
        .expect("apply should succeed");

        assert_eq!(preview.adjusted_amount, "90");
        assert_eq!(preview.channel_id, Some(channel_id.to_string()));
        assert_eq!(preview.channel_slug.as_deref(), Some("web-store"));

        let detail = rustok_pricing::PricingService::new(db, mock_transactional_event_bus())
            .get_admin_product_pricing_with_locale_fallback(
                tenant_id,
                product_id,
                "en",
                Some("en"),
                None,
            )
            .await
            .expect("admin detail should load");

        let variant = detail
            .variants
            .into_iter()
            .find(|variant| variant.id == variant_id)
            .expect("variant should be present");
        let global_base = variant
            .prices
            .iter()
            .find(|price| {
                price.currency_code == "USD"
                    && price.price_list_id.is_none()
                    && price.channel_id.is_none()
                    && price.channel_slug.is_none()
                    && price.min_quantity.is_none()
                    && price.max_quantity.is_none()
            })
            .expect("global base price should exist");
        let channel_base = variant
            .prices
            .iter()
            .find(|price| {
                price.currency_code == "USD"
                    && price.price_list_id.is_none()
                    && price.channel_id == Some(channel_id)
                    && price.channel_slug.as_deref() == Some("web-store")
                    && price.min_quantity.is_none()
                    && price.max_quantity.is_none()
            })
            .expect("channel scoped base price should exist");

        assert_eq!(
            global_base.amount,
            rust_decimal::Decimal::from_str("100.00").expect("valid decimal")
        );
        assert_eq!(
            channel_base.amount,
            rust_decimal::Decimal::from_str("90.00").expect("valid decimal")
        );
    }

    #[tokio::test]
    async fn admin_pricing_native_apply_variant_discount_requires_products_update_permission() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Pricing Admin Test Tenant".to_string(),
            slug: format!("pricing-admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db.clone());
        let catalog = CatalogService::new(db, mock_transactional_event_bus());
        let (_product_id, variant_id) = create_test_product(&catalog, tenant_id, actor_id).await;

        let error = apply_variant_discount_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            variant_id.to_string(),
            PricingDiscountDraft {
                currency_code: "USD".to_string(),
                discount_percent: "10".to_string(),
                price_list_id: "".to_string(),
                channel_id: "".to_string(),
                channel_slug: "".to_string(),
            },
        )
        .await
        .expect_err("permission should be required");

        assert!(error.to_string().contains("products:update required"));
    }

    #[tokio::test]
    async fn admin_pricing_native_preview_variant_discount_supports_active_price_list_override() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Pricing Admin Test Tenant".to_string(),
            slug: format!("pricing-admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db.clone());
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let (_product_id, variant_id) = create_test_product(&catalog, tenant_id, actor_id).await;
        let pricing =
            rustok_pricing::PricingService::new(db.clone(), mock_transactional_event_bus());
        let price_list_id = create_price_list(&db, tenant_id, "active").await;

        pricing
            .set_price_list_tier(
                tenant_id,
                actor_id,
                variant_id,
                price_list_id,
                "USD",
                rust_decimal::Decimal::from_str("80.00").expect("valid decimal"),
                Some(rust_decimal::Decimal::from_str("100.00").expect("valid decimal")),
                None,
                None,
            )
            .await
            .expect("price-list row should be created");

        let preview = preview_variant_discount_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            variant_id.to_string(),
            PricingDiscountDraft {
                currency_code: "USD".to_string(),
                discount_percent: "10".to_string(),
                price_list_id: price_list_id.to_string(),
                channel_id: "".to_string(),
                channel_slug: "".to_string(),
            },
        )
        .await
        .expect("preview should succeed");

        assert_eq!(preview.adjusted_amount, "90");
        assert_eq!(preview.price_list_id, Some(price_list_id.to_string()));
    }

    #[tokio::test]
    async fn admin_pricing_native_preview_variant_discount_rejects_future_price_list_override() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Pricing Admin Test Tenant".to_string(),
            slug: format!("pricing-admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db.clone());
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let (_product_id, variant_id) = create_test_product(&catalog, tenant_id, actor_id).await;
        let price_list_id = create_future_price_list(&db, tenant_id, "active").await;

        let error = preview_variant_discount_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            variant_id.to_string(),
            PricingDiscountDraft {
                currency_code: "USD".to_string(),
                discount_percent: "10".to_string(),
                price_list_id: price_list_id.to_string(),
                channel_id: "".to_string(),
                channel_slug: "".to_string(),
            },
        )
        .await
        .expect_err("future list should be rejected");

        assert!(error.to_string().contains("not active yet"));
    }

    #[tokio::test]
    async fn admin_pricing_native_preview_variant_discount_rejects_expired_price_list_override() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Pricing Admin Test Tenant".to_string(),
            slug: format!("pricing-admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db.clone());
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let (_product_id, variant_id) = create_test_product(&catalog, tenant_id, actor_id).await;
        let price_list_id = create_expired_price_list(&db, tenant_id, "active").await;

        let error = preview_variant_discount_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            variant_id.to_string(),
            PricingDiscountDraft {
                currency_code: "USD".to_string(),
                discount_percent: "10".to_string(),
                price_list_id: price_list_id.to_string(),
                channel_id: "".to_string(),
                channel_slug: "".to_string(),
            },
        )
        .await
        .expect_err("expired list should be rejected");

        assert!(error.to_string().contains("already expired"));
    }

    #[tokio::test]
    async fn admin_pricing_native_preview_variant_discount_rejects_channel_mismatch_for_price_list_override(
    ) {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let channel_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Pricing Admin Test Tenant".to_string(),
            slug: format!("pricing-admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db.clone());
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let (_product_id, variant_id) = create_test_product(&catalog, tenant_id, actor_id).await;
        let pricing =
            rustok_pricing::PricingService::new(db.clone(), mock_transactional_event_bus());
        let price_list_id = create_price_list(&db, tenant_id, "active").await;

        pricing
            .set_price_list_scope(
                tenant_id,
                actor_id,
                price_list_id,
                Some(channel_id),
                Some("web-store".to_string()),
            )
            .await
            .expect("scope update should succeed");

        let error = preview_variant_discount_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            variant_id.to_string(),
            PricingDiscountDraft {
                currency_code: "USD".to_string(),
                discount_percent: "10".to_string(),
                price_list_id: price_list_id.to_string(),
                channel_id: Uuid::new_v4().to_string(),
                channel_slug: "mobile-app".to_string(),
            },
        )
        .await
        .expect_err("mismatched channel should be rejected");

        assert!(error
            .to_string()
            .contains("price_list_id is not available for the requested channel"));
    }

    #[tokio::test]
    async fn admin_pricing_native_apply_variant_discount_targets_active_price_list_override_only() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Pricing Admin Test Tenant".to_string(),
            slug: format!("pricing-admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_READ, Permission::PRODUCTS_UPDATE],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db.clone());
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let (product_id, variant_id) = create_test_product(&catalog, tenant_id, actor_id).await;
        let pricing =
            rustok_pricing::PricingService::new(db.clone(), mock_transactional_event_bus());
        let price_list_id = create_price_list(&db, tenant_id, "active").await;

        pricing
            .set_price_list_tier(
                tenant_id,
                actor_id,
                variant_id,
                price_list_id,
                "USD",
                rust_decimal::Decimal::from_str("80.00").expect("valid decimal"),
                Some(rust_decimal::Decimal::from_str("100.00").expect("valid decimal")),
                None,
                None,
            )
            .await
            .expect("price-list row should be created");

        let preview = apply_variant_discount_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            variant_id.to_string(),
            PricingDiscountDraft {
                currency_code: "USD".to_string(),
                discount_percent: "10".to_string(),
                price_list_id: price_list_id.to_string(),
                channel_id: "".to_string(),
                channel_slug: "".to_string(),
            },
        )
        .await
        .expect("apply should succeed");

        assert_eq!(preview.adjusted_amount, "90");
        assert_eq!(preview.price_list_id, Some(price_list_id.to_string()));

        let detail = rustok_pricing::PricingService::new(db, mock_transactional_event_bus())
            .get_admin_product_pricing_with_locale_fallback(
                tenant_id,
                product_id,
                "en",
                Some("en"),
                Some(price_list_id),
            )
            .await
            .expect("admin detail should load");

        let variant = detail
            .variants
            .into_iter()
            .find(|variant| variant.id == variant_id)
            .expect("variant should be present");
        let base_price = variant
            .prices
            .iter()
            .find(|price| {
                price.currency_code == "USD"
                    && price.price_list_id.is_none()
                    && price.min_quantity.is_none()
                    && price.max_quantity.is_none()
            })
            .expect("base price should exist");
        let price_list_price = variant
            .prices
            .iter()
            .find(|price| {
                price.currency_code == "USD"
                    && price.price_list_id == Some(price_list_id)
                    && price.min_quantity.is_none()
                    && price.max_quantity.is_none()
            })
            .expect("price-list row should exist");

        assert_eq!(
            base_price.amount,
            rust_decimal::Decimal::from_str("100.00").expect("valid decimal")
        );
        assert_eq!(
            price_list_price.amount,
            rust_decimal::Decimal::from_str("90.00").expect("valid decimal")
        );
    }

    #[tokio::test]
    async fn admin_pricing_native_apply_variant_discount_rejects_future_price_list_override_without_writing(
    ) {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Pricing Admin Test Tenant".to_string(),
            slug: format!("pricing-admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_READ, Permission::PRODUCTS_UPDATE],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db.clone());
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let (_product_id, variant_id) = create_test_product(&catalog, tenant_id, actor_id).await;
        let price_list_id = create_future_price_list(&db, tenant_id, "active").await;

        let count_before = rustok_commerce::entities::price::Entity::find()
            .filter(rustok_commerce::entities::price::Column::VariantId.eq(variant_id))
            .filter(rustok_commerce::entities::price::Column::PriceListId.eq(price_list_id))
            .count(&db)
            .await
            .expect("price count should load");

        let error = apply_variant_discount_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            variant_id.to_string(),
            PricingDiscountDraft {
                currency_code: "USD".to_string(),
                discount_percent: "10".to_string(),
                price_list_id: price_list_id.to_string(),
                channel_id: "".to_string(),
                channel_slug: "".to_string(),
            },
        )
        .await
        .expect_err("future list should be rejected");

        let count_after = rustok_commerce::entities::price::Entity::find()
            .filter(rustok_commerce::entities::price::Column::VariantId.eq(variant_id))
            .filter(rustok_commerce::entities::price::Column::PriceListId.eq(price_list_id))
            .count(&db)
            .await
            .expect("price count should load");

        assert!(error.to_string().contains("not active yet"));
        assert_eq!(count_before, 0);
        assert_eq!(count_after, 0);
    }

    #[tokio::test]
    async fn admin_pricing_native_apply_variant_discount_rejects_expired_price_list_override_without_writing(
    ) {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Pricing Admin Test Tenant".to_string(),
            slug: format!("pricing-admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_READ, Permission::PRODUCTS_UPDATE],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db.clone());
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let (_product_id, variant_id) = create_test_product(&catalog, tenant_id, actor_id).await;
        let price_list_id = create_expired_price_list(&db, tenant_id, "active").await;

        let count_before = rustok_commerce::entities::price::Entity::find()
            .filter(rustok_commerce::entities::price::Column::VariantId.eq(variant_id))
            .filter(rustok_commerce::entities::price::Column::PriceListId.eq(price_list_id))
            .count(&db)
            .await
            .expect("price count should load");

        let error = apply_variant_discount_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            variant_id.to_string(),
            PricingDiscountDraft {
                currency_code: "USD".to_string(),
                discount_percent: "10".to_string(),
                price_list_id: price_list_id.to_string(),
                channel_id: "".to_string(),
                channel_slug: "".to_string(),
            },
        )
        .await
        .expect_err("expired list should be rejected");

        let count_after = rustok_commerce::entities::price::Entity::find()
            .filter(rustok_commerce::entities::price::Column::VariantId.eq(variant_id))
            .filter(rustok_commerce::entities::price::Column::PriceListId.eq(price_list_id))
            .count(&db)
            .await
            .expect("price count should load");

        assert!(error.to_string().contains("already expired"));
        assert_eq!(count_before, 0);
        assert_eq!(count_after, 0);
    }

    #[tokio::test]
    async fn admin_pricing_native_apply_variant_discount_rejects_channel_mismatch_without_mutating_scoped_override(
    ) {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let channel_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Pricing Admin Test Tenant".to_string(),
            slug: format!("pricing-admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_READ, Permission::PRODUCTS_UPDATE],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db.clone());
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let (product_id, variant_id) = create_test_product(&catalog, tenant_id, actor_id).await;
        let pricing =
            rustok_pricing::PricingService::new(db.clone(), mock_transactional_event_bus());
        let price_list_id = create_price_list(&db, tenant_id, "active").await;

        pricing
            .set_price_list_scope(
                tenant_id,
                actor_id,
                price_list_id,
                Some(channel_id),
                Some("web-store".to_string()),
            )
            .await
            .expect("scope update should succeed");
        pricing
            .set_price_list_tier_with_channel(
                tenant_id,
                actor_id,
                variant_id,
                price_list_id,
                "USD",
                rust_decimal::Decimal::from_str("80.00").expect("valid decimal"),
                Some(rust_decimal::Decimal::from_str("100.00").expect("valid decimal")),
                Some(channel_id),
                Some("web-store".to_string()),
                None,
                None,
            )
            .await
            .expect("scoped override should be created");

        let count_before = rustok_commerce::entities::price::Entity::find()
            .filter(rustok_commerce::entities::price::Column::VariantId.eq(variant_id))
            .filter(rustok_commerce::entities::price::Column::PriceListId.eq(price_list_id))
            .count(&db)
            .await
            .expect("price count should load");

        let error = apply_variant_discount_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            variant_id.to_string(),
            PricingDiscountDraft {
                currency_code: "USD".to_string(),
                discount_percent: "10".to_string(),
                price_list_id: price_list_id.to_string(),
                channel_id: Uuid::new_v4().to_string(),
                channel_slug: "mobile-app".to_string(),
            },
        )
        .await
        .expect_err("mismatched channel should be rejected");

        let count_after = rustok_commerce::entities::price::Entity::find()
            .filter(rustok_commerce::entities::price::Column::VariantId.eq(variant_id))
            .filter(rustok_commerce::entities::price::Column::PriceListId.eq(price_list_id))
            .count(&db)
            .await
            .expect("price count should load");

        let detail = rustok_pricing::PricingService::new(db, mock_transactional_event_bus())
            .get_admin_product_pricing_with_locale_fallback(
                tenant_id,
                product_id,
                "en",
                Some("en"),
                Some(price_list_id),
            )
            .await
            .expect("admin detail should load");
        let variant = detail
            .variants
            .into_iter()
            .find(|variant| variant.id == variant_id)
            .expect("variant should be present");
        let scoped_price = variant
            .prices
            .into_iter()
            .find(|price| price.price_list_id == Some(price_list_id))
            .expect("scoped price should exist");

        assert!(error
            .to_string()
            .contains("price_list_id is not available for the requested channel"));
        assert_eq!(count_before, 1);
        assert_eq!(count_after, 1);
        assert_eq!(
            scoped_price.amount,
            rust_decimal::Decimal::from_str("80.00").expect("valid decimal")
        );
        assert_eq!(
            scoped_price.compare_at_amount,
            Some(rust_decimal::Decimal::from_str("100.00").expect("valid decimal"))
        );
        assert_eq!(scoped_price.channel_id, Some(channel_id));
        assert_eq!(scoped_price.channel_slug.as_deref(), Some("web-store"));
    }

    #[tokio::test]
    async fn admin_pricing_native_update_price_list_rule_updates_active_option() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        let price_list_id = create_price_list(&db, tenant_id, "active").await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Pricing Admin Test Tenant".to_string(),
            slug: format!("pricing-admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_READ, Permission::PRODUCTS_UPDATE],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db);

        let option = update_price_list_rule_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            price_list_id.to_string(),
            PricingPriceListRuleDraft {
                adjustment_percent: "15".to_string(),
            },
        )
        .await
        .expect("rule update should succeed");

        assert_eq!(option.id, price_list_id.to_string());
        assert_eq!(option.rule_kind.as_deref(), Some("percentage_discount"));
        assert_eq!(option.adjustment_percent.as_deref(), Some("15"));
    }

    #[tokio::test]
    async fn admin_pricing_native_update_price_list_rule_clears_active_option_metadata() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        let price_list_id = create_price_list(&db, tenant_id, "active").await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Pricing Admin Test Tenant".to_string(),
            slug: format!("pricing-admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_READ, Permission::PRODUCTS_UPDATE],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db.clone());

        update_price_list_rule_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            price_list_id.to_string(),
            PricingPriceListRuleDraft {
                adjustment_percent: "15".to_string(),
            },
        )
        .await
        .expect("rule update should succeed");

        let option = update_price_list_rule_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            price_list_id.to_string(),
            PricingPriceListRuleDraft {
                adjustment_percent: "".to_string(),
            },
        )
        .await
        .expect("rule clear should succeed");

        assert_eq!(option.id, price_list_id.to_string());
        assert!(option.rule_kind.is_none());
        assert!(option.adjustment_percent.is_none());
    }

    #[tokio::test]
    async fn admin_pricing_native_update_price_list_rule_rejects_draft_price_list() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        let price_list_id = create_price_list(&db, tenant_id, "draft").await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Pricing Admin Test Tenant".to_string(),
            slug: format!("pricing-admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_READ, Permission::PRODUCTS_UPDATE],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db);

        let error = update_price_list_rule_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            price_list_id.to_string(),
            PricingPriceListRuleDraft {
                adjustment_percent: "15".to_string(),
            },
        )
        .await
        .expect_err("draft list should be rejected");

        assert!(error
            .to_string()
            .contains("price_list_id must reference an active price list"));
    }

    #[tokio::test]
    async fn admin_pricing_native_update_price_list_rule_rejects_future_price_list() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        let price_list_id = create_future_price_list(&db, tenant_id, "active").await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Pricing Admin Test Tenant".to_string(),
            slug: format!("pricing-admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_READ, Permission::PRODUCTS_UPDATE],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db);

        let error = update_price_list_rule_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            price_list_id.to_string(),
            PricingPriceListRuleDraft {
                adjustment_percent: "15".to_string(),
            },
        )
        .await
        .expect_err("future list should be rejected");

        assert!(error.to_string().contains("not active yet"));
    }

    #[tokio::test]
    async fn admin_pricing_native_update_price_list_rule_rejects_expired_price_list() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        let price_list_id = create_expired_price_list(&db, tenant_id, "active").await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Pricing Admin Test Tenant".to_string(),
            slug: format!("pricing-admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_READ, Permission::PRODUCTS_UPDATE],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db);

        let error = update_price_list_rule_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            price_list_id.to_string(),
            PricingPriceListRuleDraft {
                adjustment_percent: "15".to_string(),
            },
        )
        .await
        .expect_err("expired list should be rejected");

        assert!(error.to_string().contains("already expired"));
    }

    #[tokio::test]
    async fn admin_pricing_native_update_price_list_scope_updates_active_option_and_rows() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let channel_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        let price_list_id = create_price_list(&db, tenant_id, "active").await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Pricing Admin Test Tenant".to_string(),
            slug: format!("pricing-admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_READ, Permission::PRODUCTS_UPDATE],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db.clone());
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let (product_id, variant_id) = create_test_product(&catalog, tenant_id, actor_id).await;
        let pricing =
            rustok_pricing::PricingService::new(db.clone(), mock_transactional_event_bus());

        pricing
            .set_price_list_tier(
                tenant_id,
                actor_id,
                variant_id,
                price_list_id,
                "USD",
                rust_decimal::Decimal::from_str("81.00").expect("valid decimal"),
                Some(rust_decimal::Decimal::from_str("100.00").expect("valid decimal")),
                None,
                None,
            )
            .await
            .expect("price-list row should be created");

        let option = update_price_list_scope_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            price_list_id.to_string(),
            PricingPriceListScopeDraft {
                channel_id: channel_id.to_string(),
                channel_slug: "web-store".to_string(),
            },
        )
        .await
        .expect("scope update should succeed");

        assert_eq!(option.id, price_list_id.to_string());
        assert_eq!(option.channel_id, Some(channel_id.to_string()));
        assert_eq!(option.channel_slug.as_deref(), Some("web-store"));

        let detail = rustok_pricing::PricingService::new(db, mock_transactional_event_bus())
            .get_admin_product_pricing_with_locale_fallback(
                tenant_id,
                product_id,
                "en",
                Some("en"),
                Some(price_list_id),
            )
            .await
            .expect("admin detail should load");

        let variant = detail
            .variants
            .into_iter()
            .find(|variant| variant.id == variant_id)
            .expect("variant should be present");
        let scoped_price = variant
            .prices
            .into_iter()
            .find(|price| price.price_list_id == Some(price_list_id))
            .expect("price-list row should exist");

        assert_eq!(scoped_price.channel_id, Some(channel_id));
        assert_eq!(scoped_price.channel_slug.as_deref(), Some("web-store"));
    }

    #[tokio::test]
    async fn admin_pricing_native_update_price_list_scope_clears_active_option_and_rows() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let channel_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        let price_list_id = create_price_list(&db, tenant_id, "active").await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Pricing Admin Test Tenant".to_string(),
            slug: format!("pricing-admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_READ, Permission::PRODUCTS_UPDATE],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db.clone());
        let catalog = CatalogService::new(db.clone(), mock_transactional_event_bus());
        let (product_id, variant_id) = create_test_product(&catalog, tenant_id, actor_id).await;
        let pricing =
            rustok_pricing::PricingService::new(db.clone(), mock_transactional_event_bus());

        pricing
            .set_price_list_tier(
                tenant_id,
                actor_id,
                variant_id,
                price_list_id,
                "USD",
                rust_decimal::Decimal::from_str("81.00").expect("valid decimal"),
                Some(rust_decimal::Decimal::from_str("100.00").expect("valid decimal")),
                None,
                None,
            )
            .await
            .expect("price-list row should be created");

        update_price_list_scope_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            price_list_id.to_string(),
            PricingPriceListScopeDraft {
                channel_id: channel_id.to_string(),
                channel_slug: "web-store".to_string(),
            },
        )
        .await
        .expect("scope update should succeed");

        let option = update_price_list_scope_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            price_list_id.to_string(),
            PricingPriceListScopeDraft {
                channel_id: "".to_string(),
                channel_slug: "".to_string(),
            },
        )
        .await
        .expect("scope clear should succeed");

        assert!(option.channel_id.is_none());
        assert!(option.channel_slug.is_none());

        let detail = rustok_pricing::PricingService::new(db, mock_transactional_event_bus())
            .get_admin_product_pricing_with_locale_fallback(
                tenant_id,
                product_id,
                "en",
                Some("en"),
                Some(price_list_id),
            )
            .await
            .expect("admin detail should load");

        let variant = detail
            .variants
            .into_iter()
            .find(|variant| variant.id == variant_id)
            .expect("variant should be present");
        let scoped_price = variant
            .prices
            .into_iter()
            .find(|price| price.price_list_id == Some(price_list_id))
            .expect("price-list row should exist");

        assert_eq!(scoped_price.channel_id, None);
        assert_eq!(scoped_price.channel_slug, None);
    }

    #[tokio::test]
    async fn admin_pricing_native_update_price_list_scope_refreshes_active_price_list_selector() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let channel_id = Uuid::new_v4();
        let other_channel_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        let price_list_id = create_price_list(&db, tenant_id, "active").await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Pricing Admin Test Tenant".to_string(),
            slug: format!("pricing-admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_READ, Permission::PRODUCTS_UPDATE],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db);

        update_price_list_scope_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            price_list_id.to_string(),
            PricingPriceListScopeDraft {
                channel_id: channel_id.to_string(),
                channel_slug: "web-store".to_string(),
            },
        )
        .await
        .expect("scope update should succeed");

        let scoped_lists = list_active_price_lists_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            Some(channel_id.to_string()),
            Some("web-store".to_string()),
        )
        .await
        .expect("scoped list should load");
        assert!(scoped_lists
            .iter()
            .any(|item| item.id == price_list_id.to_string()));

        let mismatched_lists = list_active_price_lists_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            Some(other_channel_id.to_string()),
            Some("mobile-app".to_string()),
        )
        .await
        .expect("mismatched list should load");
        assert!(!mismatched_lists
            .iter()
            .any(|item| item.id == price_list_id.to_string()));

        update_price_list_scope_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            price_list_id.to_string(),
            PricingPriceListScopeDraft {
                channel_id: "".to_string(),
                channel_slug: "".to_string(),
            },
        )
        .await
        .expect("scope clear should succeed");

        let global_lists = list_active_price_lists_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            Some(other_channel_id.to_string()),
            Some("mobile-app".to_string()),
        )
        .await
        .expect("global list should load");
        assert!(global_lists
            .iter()
            .any(|item| item.id == price_list_id.to_string()));
    }

    #[tokio::test]
    async fn admin_pricing_native_update_price_list_rule_requires_products_update_permission() {
        let db = setup_test_db().await;
        support::ensure_commerce_schema(&db).await;
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        seed_tenant_context(&db, tenant_id).await;
        let price_list_id = create_price_list(&db, tenant_id, "active").await;

        let tenant = TenantContext {
            id: tenant_id,
            name: "Pricing Admin Test Tenant".to_string(),
            slug: format!("pricing-admin-test-{tenant_id}"),
            domain: None,
            settings: json!({}),
            default_locale: "en".to_string(),
            is_active: true,
        };
        let auth = AuthContext {
            user_id: actor_id,
            session_id: Uuid::new_v4(),
            tenant_id,
            permissions: vec![Permission::PRODUCTS_READ],
            client_id: None,
            scopes: vec![],
            grant_type: "direct".to_string(),
        };
        let app_ctx = test_app_context(db);

        let error = update_price_list_rule_native_with_context(
            &app_ctx,
            &auth,
            &tenant,
            price_list_id.to_string(),
            PricingPriceListRuleDraft {
                adjustment_percent: "15".to_string(),
            },
        )
        .await
        .expect_err("permission should be required");

        assert!(error.to_string().contains("products:update required"));
    }
}

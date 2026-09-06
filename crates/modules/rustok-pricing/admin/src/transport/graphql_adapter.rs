#![allow(clippy::too_many_arguments)]

use super::native_server_adapter::ApiError;
use crate::core::{
    parse_optional_uuid_string, sanitize_channel_slug as sanitize_core_channel_slug,
};
use crate::model::{
    CurrentTenant, PricingAdminBootstrap, PricingChannelOption, PricingPriceListOption,
    PricingProductDetail, PricingProductList,
};
use rustok_graphql::{GraphqlRequest, execute as execute_graphql};
use serde::{Deserialize, Serialize};

const BOOTSTRAP_QUERY: &str = "query PricingAdminBootstrap { currentTenant { id slug name } storefrontPricingChannels { id slug name isActive isDefault status } storefrontActivePriceLists { id name listType channelId channelSlug ruleKind adjustmentPercent } }";
const ACTIVE_PRICE_LISTS_QUERY: &str = "query PricingAdminActivePriceLists($channelId: UUID, $channelSlug: String) { storefrontActivePriceLists(channelId: $channelId, channelSlug: $channelSlug) { id name listType channelId channelSlug ruleKind adjustmentPercent } }";
const PRODUCTS_QUERY: &str = "query PricingAdminProducts($tenantId: UUID!, $locale: String, $filter: ProductsFilter) { products(tenantId: $tenantId, locale: $locale, filter: $filter) { total page perPage hasNext items { id status sellerId title handle vendor productType shippingProfileSlug tags createdAt publishedAt } } }";
const PRODUCT_QUERY: &str = "query PricingAdminProduct($tenantId: UUID!, $id: UUID!, $locale: String, $currencyCode: String, $regionId: UUID, $priceListId: UUID, $channelId: UUID, $channelSlug: String, $quantity: Int) { adminPricingProduct(tenantId: $tenantId, id: $id, locale: $locale, currencyCode: $currencyCode, regionId: $regionId, priceListId: $priceListId, channelId: $channelId, channelSlug: $channelSlug, quantity: $quantity) { id status sellerId vendor productType shippingProfileSlug createdAt updatedAt publishedAt translations { locale title handle description } variants { id sku barcode shippingProfileSlug title option1 option2 option3 prices { currencyCode amount compareAtAmount discountPercent onSale priceListId channelId channelSlug minQuantity maxQuantity } effectivePrice { currencyCode amount compareAtAmount discountPercent onSale regionId priceListId channelId channelSlug minQuantity maxQuantity } } } }";

impl From<rustok_graphql::GraphqlHttpError> for ApiError {
    fn from(value: rustok_graphql::GraphqlHttpError) -> Self {
        Self::Graphql(value.to_string())
    }
}

pub(super) fn parse_channel_id(channel_id: Option<String>) -> Result<Option<String>, ApiError> {
    parse_optional_uuid_string(channel_id, "channel_id").map_err(ApiError::from)
}

pub(super) fn sanitize_channel_slug(channel_slug: Option<String>) -> Option<String> {
    sanitize_core_channel_slug(channel_slug)
}

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
        let origin = leptos::web_sys::window()
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

pub(super) async fn fetch_bootstrap(
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

pub(super) async fn fetch_active_price_lists(
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

pub(super) async fn fetch_products(
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

#[allow(clippy::too_many_arguments)]
pub(super) async fn fetch_product(
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

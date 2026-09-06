use super::native_server_adapter::ApiError;
use crate::core::{
    StorefrontPricingQuery, parse_optional_uuid_string, sanitize_channel_slug,
    sanitize_resolution_context,
};
use crate::model::{
    PricingChannelOption, PricingPriceListOption, PricingProductDetail, PricingProductList,
    PricingProductListItem, PricingProductTranslation, PricingVariant, StorefrontPricingData,
};
use futures::future::try_join_all;
use rustok_graphql::{GraphqlRequest, execute as execute_graphql};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const STOREFRONT_PRODUCTS_QUERY: &str = "query StorefrontCommerceProducts($locale: String, $filter: StorefrontProductsFilter, $channelId: UUID, $channelSlug: String) { storefrontProducts(locale: $locale, filter: $filter) { total page perPage hasNext items { id title handle sellerId vendor productType createdAt publishedAt } } storefrontPricingChannels { id slug name isActive isDefault status } storefrontActivePriceLists(channelId: $channelId, channelSlug: $channelSlug) { id name listType channelId channelSlug ruleKind adjustmentPercent } }";
const STOREFRONT_PRODUCT_QUERY: &str = "query StorefrontCommerceProduct($locale: String, $handle: String!, $currencyCode: String, $regionId: UUID, $priceListId: UUID, $channelId: UUID, $channelSlug: String, $quantity: Int) { storefrontPricingProduct(locale: $locale, handle: $handle, currencyCode: $currencyCode, regionId: $regionId, priceListId: $priceListId, channelId: $channelId, channelSlug: $channelSlug, quantity: $quantity) { id status sellerId vendor productType publishedAt translations { locale title handle description } variants { id title sku prices { currencyCode amount compareAtAmount discountPercent onSale } effectivePrice { currencyCode amount compareAtAmount discountPercent onSale regionId priceListId channelId channelSlug minQuantity maxQuantity } } } }";

impl From<rustok_graphql::GraphqlHttpError> for ApiError {
    fn from(value: rustok_graphql::GraphqlHttpError) -> Self {
        Self::Graphql(value.to_string())
    }
}

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

pub(crate) async fn fetch_storefront_pricing(
    query: StorefrontPricingQuery,
) -> Result<StorefrontPricingData, ApiError> {
    let selected_channel_id = parse_optional_uuid_string(query.channel_id.clone(), "channel_id")
        .map_err(|err| ApiError::ServerFn(err.to_string()))?;
    let selected_channel_slug = sanitize_channel_slug(query.channel_slug.clone());
    let resolution_context = sanitize_resolution_context(
        query.currency_code.clone(),
        query.region_id.clone(),
        query.price_list_id.clone(),
        query.channel_id,
        query.channel_slug,
        query.quantity,
    )
    .map_err(|err| ApiError::ServerFn(err.to_string()))?;
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

    let StorefrontProductsResponse {
        storefront_products,
        available_channels,
        active_price_lists,
    } = list_response;
    let list_locale = query.locale.clone();
    let detailed_items = try_join_all(storefront_products.items.into_iter().map(|item| {
        let locale = list_locale.clone();
        async move {
            let detail = if item.handle.trim().is_empty() {
                None
            } else {
                fetch_storefront_pricing_detail(StorefrontPricingDetailQuery {
                    handle: item.handle.clone(),
                    locale,
                    ..StorefrontPricingDetailQuery::default()
                })
                .await?
            };
            Ok::<_, ApiError>((item, detail))
        }
    }))
    .await?;

    let mut details_by_handle = HashMap::new();
    let mut items = Vec::with_capacity(detailed_items.len());
    for (item, detail) in detailed_items {
        if let Some(detail) = detail.as_ref() {
            details_by_handle.insert(item.handle.clone(), detail.clone());
        }
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
        if resolution_context.is_none() {
            if let Some(detail) = details_by_handle.remove(&handle) {
                Some(detail)
            } else {
                fetch_storefront_pricing_detail(StorefrontPricingDetailQuery {
                    handle,
                    locale: query.locale,
                    ..StorefrontPricingDetailQuery::default()
                })
                .await?
            }
        } else {
            fetch_storefront_pricing_detail(StorefrontPricingDetailQuery {
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
        }
    } else {
        None
    };

    Ok(StorefrontPricingData {
        products: PricingProductList {
            items,
            total: storefront_products.total,
            page: storefront_products.page,
            per_page: storefront_products.per_page,
            has_next: storefront_products.has_next,
        },
        selected_product,
        selected_handle: resolved_handle,
        resolution_context,
        available_channels,
        active_price_lists,
    })
}

async fn fetch_storefront_pricing_detail(
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

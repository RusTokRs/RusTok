use super::native_server_adapter::ApiError;
use crate::core::{
    StorefrontPricingQuery, normalize_pricing_context, parse_optional_uuid_string,
    pricing_quote_matches_request, sanitize_channel_slug,
};
use crate::model::{
    PricingChannelOption, PricingPriceListOption, PricingProductDetail, PricingProductList,
    StorefrontPricingData,
};
use rustok_graphql::{GraphqlRequest, execute as execute_graphql, graphql_url};
use serde::{Deserialize, Serialize};

const STOREFRONT_PRODUCTS_QUERY: &str = "query StorefrontPricingCatalog($locale: String, $channelId: UUID, $channelSlug: String) { storefrontPricingCatalog(locale: $locale, channelId: $channelId, channelSlug: $channelSlug, page: 1, perPage: 8) { total page perPage hasNext items { id title handle sellerId vendor productType createdAt publishedAt variantCount saleVariantCount currencies } } storefrontPricingChannels { id slug name isActive isDefault status } storefrontActivePriceLists(channelId: $channelId, channelSlug: $channelSlug) { id name listType channelId channelSlug ruleKind adjustmentPercent } }";
const STOREFRONT_PRODUCT_QUERY: &str = "query StorefrontCommerceProduct($locale: String, $handle: String!, $currencyCode: String, $regionId: UUID, $priceListId: UUID, $channelId: UUID, $channelSlug: String, $quantity: Int) { storefrontPricingProduct(locale: $locale, handle: $handle, currencyCode: $currencyCode, regionId: $regionId, priceListId: $priceListId, channelId: $channelId, channelSlug: $channelSlug, quantity: $quantity) { resolutionContext { currencyCode regionId priceListId channelId channelSlug quantity } resolvedAt id status sellerId vendor productType publishedAt translations { locale title handle description } variants { id title sku prices { currencyCode amount compareAtAmount discountPercent onSale } effectivePrice { source { priceId kind ruleKind adjustmentPercent } currencyCode amount compareAtAmount discountPercent onSale regionId priceListId channelId channelSlug minQuantity maxQuantity } } } }";

impl From<rustok_graphql::GraphqlHttpError> for ApiError {
    fn from(value: rustok_graphql::GraphqlHttpError) -> Self {
        Self::Graphql(value.to_string())
    }
}

#[derive(Debug, Deserialize)]
struct StorefrontProductsResponse {
    #[serde(rename = "storefrontPricingCatalog")]
    storefront_products: PricingProductList,
    #[serde(rename = "storefrontPricingChannels")]
    available_channels: Vec<PricingChannelOption>,
    #[serde(rename = "storefrontActivePriceLists")]
    active_price_lists: Vec<PricingPriceListOption>,
}

#[derive(Debug, Deserialize)]
struct StorefrontProductResponse {
    #[serde(rename = "storefrontPricingProduct")]
    storefront_product: Option<PricingProductDetail>,
}

#[derive(Debug, Serialize)]
struct StorefrontProductsVariables {
    locale: Option<String>,
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
    let resolution_context = normalize_pricing_context(
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
            channel_id: selected_channel_id.clone(),
            channel_slug: selected_channel_slug.clone(),
        },
    )
    .await?;

    let StorefrontProductsResponse {
        storefront_products,
        available_channels,
        active_price_lists,
    } = list_response;
    let resolved_handle = query
        .selected_handle
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| {
            storefront_products
                .items
                .first()
                .map(|item| item.handle.clone())
        });
    let selected_product = if let Some(handle) = resolved_handle.clone() {
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
            channel_id: selected_channel_id.clone(),
            channel_slug: selected_channel_slug.clone(),
            quantity: resolution_context.as_ref().map(|value| value.quantity),
        })
        .await?
    } else {
        None
    };
    if selected_product
        .as_ref()
        .is_some_and(|product| !pricing_quote_matches_request(product, resolution_context.as_ref()))
    {
        return Err(ApiError::Graphql(
            "Pricing quote response is inconsistent".to_string(),
        ));
    }
    let resolution_context = selected_product
        .as_ref()
        .and_then(|product| product.resolution_context.clone());

    Ok(StorefrontPricingData {
        products: storefront_products,
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
    Ok(response.storefront_product)
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

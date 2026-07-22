#![allow(clippy::too_many_arguments)]

use super::native_server_adapter::ApiError;
use crate::core::{FetchRequest, build_pricing_context};
use crate::model::{
    ProductCatalogSearchOptions, ProductDetail, ProductList, ProductPricingDetail,
    StorefrontProductsData,
};
use rustok_graphql::{GraphqlRequest, execute as execute_graphql};
use serde::{Deserialize, Serialize};

const STOREFRONT_PRODUCTS_QUERY: &str = "query StorefrontCommerceProducts($locale: String, $filter: StorefrontProductsFilter) { storefrontProducts(locale: $locale, filter: $filter) { total page perPage hasNext items { id status title handle sellerId vendor productType tags createdAt publishedAt } } }";
const STOREFRONT_PRODUCT_QUERY: &str = "query StorefrontCommerceProduct($locale: String, $handle: String!) { storefrontProduct(locale: $locale, handle: $handle) { id status sellerId vendor productType tags publishedAt translations { locale title handle description } variants { id title sku inventoryQuantity inStock prices { currencyCode amount compareAtAmount onSale } } } }";
const STOREFRONT_PRICING_PRODUCT_QUERY: &str = "query StorefrontProductPricing($locale: String, $handle: String!, $currencyCode: String, $regionId: UUID, $priceListId: UUID, $channelId: UUID, $channelSlug: String, $quantity: Int) { storefrontPricingProduct(locale: $locale, handle: $handle, currencyCode: $currencyCode, regionId: $regionId, priceListId: $priceListId, channelId: $channelId, channelSlug: $channelSlug, quantity: $quantity) { variants { id title sku prices { currencyCode amount compareAtAmount discountPercent onSale } effectivePrice { currencyCode amount compareAtAmount discountPercent onSale priceListId channelId channelSlug } } } }";
const STOREFRONT_CATALOG_SEARCH_OPTIONS_QUERY: &str = "query StorefrontCatalogSearchOptions($locale: String!) { storefrontCatalogSearchOptions(locale: $locale) { categoryOptions { value label } attributeOptions { value label } } }";

impl From<rustok_graphql::GraphqlHttpError> for ApiError {
    fn from(value: rustok_graphql::GraphqlHttpError) -> Self {
        Self::Graphql(value.to_string())
    }
}

#[derive(Debug, Deserialize)]
struct StorefrontProductsResponse {
    #[serde(rename = "storefrontProducts")]
    storefront_products: ProductList,
}

#[derive(Debug, Deserialize)]
struct StorefrontProductResponse {
    #[serde(rename = "storefrontProduct")]
    storefront_product: Option<ProductDetail>,
}

#[derive(Debug, Deserialize)]
struct StorefrontPricingProductResponse {
    #[serde(rename = "storefrontPricingProduct")]
    storefront_pricing_product: Option<ProductPricingDetail>,
}

#[derive(Debug, Serialize)]
struct StorefrontProductsVariables {
    locale: Option<String>,
    filter: StorefrontProductsFilter,
}

#[derive(Debug, Serialize)]
struct StorefrontProductVariables {
    locale: Option<String>,
    handle: String,
}

#[derive(Debug, Serialize)]
struct StorefrontPricingProductVariables {
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

#[derive(Serialize)]
struct CatalogSearchOptionsVariables {
    locale: String,
}

#[derive(Deserialize)]
struct CatalogSearchOptionsResponse {
    #[serde(rename = "storefrontCatalogSearchOptions")]
    options: ProductCatalogSearchOptions,
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

pub async fn fetch_products(request: FetchRequest) -> Result<StorefrontProductsData, ApiError> {
    fetch_storefront_products(
        request.selected_handle,
        request.locale,
        request.currency_code,
        request.region_id,
        request.price_list_id,
        request.channel_id,
        request.channel_slug,
        request.quantity,
    )
    .await
}

pub async fn fetch_catalog_search_options(
    locale: String,
) -> Result<ProductCatalogSearchOptions, ApiError> {
    let response: CatalogSearchOptionsResponse = request(
        STOREFRONT_CATALOG_SEARCH_OPTIONS_QUERY,
        CatalogSearchOptionsVariables { locale },
    )
    .await?;
    Ok(response.options)
}

async fn fetch_storefront_products(
    selected_handle: Option<String>,
    locale: Option<String>,
    currency_code: Option<String>,
    region_id: Option<String>,
    price_list_id: Option<String>,
    channel_id: Option<String>,
    channel_slug: Option<String>,
    quantity: Option<i32>,
) -> Result<StorefrontProductsData, ApiError> {
    let products_response: StorefrontProductsResponse = request(
        STOREFRONT_PRODUCTS_QUERY,
        StorefrontProductsVariables {
            locale: locale.clone(),
            filter: StorefrontProductsFilter {
                vendor: None,
                product_type: None,
                search: None,
                page: Some(1),
                per_page: Some(12),
            },
        },
    )
    .await?;

    let resolved_handle = selected_handle.or_else(|| {
        products_response
            .storefront_products
            .items
            .first()
            .map(|item| item.handle.clone())
            .filter(|handle| !handle.is_empty())
    });

    let selected_product = if let Some(handle) = resolved_handle.clone() {
        let response: StorefrontProductResponse = request(
            STOREFRONT_PRODUCT_QUERY,
            StorefrontProductVariables {
                locale: locale.clone(),
                handle,
            },
        )
        .await?;
        response.storefront_product
    } else {
        None
    };

    let resolution_context = build_pricing_context(
        selected_product.as_ref(),
        currency_code,
        region_id,
        price_list_id,
        channel_id,
        channel_slug,
        quantity,
    );
    let selected_pricing = if let Some(handle) = resolved_handle.clone() {
        let response: StorefrontPricingProductResponse = request(
            STOREFRONT_PRICING_PRODUCT_QUERY,
            StorefrontPricingProductVariables {
                locale,
                handle,
                currency_code: resolution_context
                    .as_ref()
                    .map(|context| context.currency_code.clone()),
                region_id: resolution_context
                    .as_ref()
                    .and_then(|context| context.region_id.clone()),
                price_list_id: resolution_context
                    .as_ref()
                    .and_then(|context| context.price_list_id.clone()),
                channel_id: resolution_context
                    .as_ref()
                    .and_then(|context| context.channel_id.clone()),
                channel_slug: resolution_context
                    .as_ref()
                    .and_then(|context| context.channel_slug.clone()),
                quantity: resolution_context.as_ref().map(|context| context.quantity),
            },
        )
        .await?;
        response.storefront_pricing_product
    } else {
        None
    };

    Ok(StorefrontProductsData {
        products: products_response.storefront_products,
        selected_product,
        selected_pricing,
        selected_handle: resolved_handle,
        resolution_context,
    })
}

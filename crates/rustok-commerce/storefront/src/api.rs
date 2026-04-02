use leptos::prelude::*;
use leptos_graphql::{execute as execute_graphql, GraphqlHttpError, GraphqlRequest};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use crate::model::{ProductDetail, ProductList, StorefrontCommerceData};

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

const STOREFRONT_PRODUCTS_QUERY: &str = "query StorefrontCommerceProducts($locale: String, $filter: StorefrontProductsFilter) { storefrontProducts(locale: $locale, filter: $filter) { total page perPage hasNext items { id status title handle vendor productType tags createdAt publishedAt } } }";
const STOREFRONT_PRODUCT_QUERY: &str = "query StorefrontCommerceProduct($locale: String, $handle: String!) { storefrontProduct(locale: $locale, handle: $handle) { id status vendor productType tags publishedAt translations { locale title handle description } variants { id title sku inventoryQuantity inStock prices { currencyCode amount compareAtAmount onSale } } } }";

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
struct StorefrontProductsFilter {
    vendor: Option<String>,
    #[serde(rename = "productType")]
    product_type: Option<String>,
    search: Option<String>,
    page: Option<u64>,
    #[serde(rename = "perPage")]
    per_page: Option<u64>,
}

fn configured_tenant_slug() -> Option<String> {
    [
        "RUSTOK_TENANT_SLUG",
        "NEXT_PUBLIC_TENANT_SLUG",
        "NEXT_PUBLIC_DEFAULT_TENANT_SLUG",
    ]
    .into_iter()
    .find_map(|key| {
        std::env::var(key)
            .ok()
            .filter(|value| !value.trim().is_empty())
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

pub async fn fetch_storefront_commerce(
    selected_handle: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontCommerceData, ApiError> {
    match fetch_storefront_commerce_server(selected_handle.clone(), locale.clone()).await {
        Ok(data) => Ok(data),
        Err(_) => fetch_storefront_commerce_graphql(selected_handle, locale).await,
    }
}

pub async fn fetch_storefront_commerce_server(
    selected_handle: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontCommerceData, ApiError> {
    storefront_commerce_native(selected_handle, locale)
        .await
        .map_err(ApiError::from)
}

pub async fn fetch_storefront_commerce_graphql(
    selected_handle: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontCommerceData, ApiError> {
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
            StorefrontProductVariables { locale, handle },
        )
        .await?;
        response.storefront_product
    } else {
        None
    };

    Ok(StorefrontCommerceData {
        products: products_response.storefront_products,
        selected_product,
        selected_handle: resolved_handle,
    })
}

#[server(prefix = "/api/fn", endpoint = "commerce/storefront-data")]
async fn storefront_commerce_native(
    selected_handle: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontCommerceData, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        fetch_storefront_commerce_graphql(selected_handle, locale)
            .await
            .map_err(|err| ServerFnError::new(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (selected_handle, locale);
        Err(ServerFnError::new(
            "commerce/storefront-data requires the `ssr` feature",
        ))
    }
}

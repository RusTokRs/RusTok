use leptos::prelude::*;
use leptos_graphql::{execute as execute_graphql, GraphqlHttpError, GraphqlRequest};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use crate::model::{
    PricingPrice, PricingProductDetail, PricingProductList, PricingProductListItem,
    PricingProductTranslation, PricingVariant, StorefrontPricingData,
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

const STOREFRONT_PRODUCTS_QUERY: &str = "query StorefrontCommerceProducts($locale: String, $filter: StorefrontProductsFilter) { storefrontProducts(locale: $locale, filter: $filter) { total page perPage hasNext items { id title handle vendor productType createdAt publishedAt } } }";
const STOREFRONT_PRODUCT_QUERY: &str = "query StorefrontCommerceProduct($locale: String, $handle: String!) { storefrontProduct(locale: $locale, handle: $handle) { id status vendor productType publishedAt translations { locale title handle description } variants { id title sku prices { currencyCode amount compareAtAmount onSale } } } }";

#[derive(Debug, Deserialize)]
struct StorefrontProductsResponse {
    #[serde(rename = "storefrontProducts")]
    storefront_products: GraphqlPricingProductList,
}

#[derive(Debug, Deserialize)]
struct StorefrontProductResponse {
    #[serde(rename = "storefrontProduct")]
    storefront_product: Option<GraphqlPricingProductDetail>,
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
    selected_handle: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontPricingData, ApiError> {
    match fetch_storefront_pricing_server(selected_handle.clone(), locale.clone()).await {
        Ok(data) => Ok(data),
        Err(_) => fetch_storefront_pricing_graphql(selected_handle, locale).await,
    }
}

pub async fn fetch_storefront_pricing_server(
    selected_handle: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontPricingData, ApiError> {
    storefront_pricing_native(selected_handle, locale)
        .await
        .map_err(ApiError::from)
}

pub async fn fetch_storefront_pricing_graphql(
    selected_handle: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontPricingData, ApiError> {
    let list_response: StorefrontProductsResponse = request(
        STOREFRONT_PRODUCTS_QUERY,
        StorefrontProductsVariables {
            locale: locale.clone(),
            filter: StorefrontProductsFilter {
                vendor: None,
                product_type: None,
                search: None,
                page: Some(1),
                per_page: Some(8),
            },
        },
    )
    .await?;

    let mut items = Vec::new();
    for item in list_response.storefront_products.items {
        let detail = if item.handle.trim().is_empty() {
            None
        } else {
            fetch_storefront_pricing_graphql_detail(item.handle.clone(), locale.clone()).await?
        };
        items.push(resolve_graphql_pricing_list_item(item, detail.as_ref()));
    }

    let resolved_handle = selected_handle
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| items.first().map(|item| item.handle.clone()));
    let selected_product = if let Some(handle) = resolved_handle.clone() {
        fetch_storefront_pricing_graphql_detail(handle, locale).await?
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
    })
}

async fn fetch_storefront_pricing_graphql_detail(
    handle: String,
    locale: Option<String>,
) -> Result<Option<PricingProductDetail>, ApiError> {
    let response: StorefrontProductResponse = request(
        STOREFRONT_PRODUCT_QUERY,
        StorefrontProductVariables { locale, handle },
    )
    .await?;
    Ok(response.storefront_product.map(map_graphql_detail))
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
        vendor: value.vendor,
        product_type: value.product_type,
        published_at: value.published_at,
        translations: value.translations,
        variants: value.variants,
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
                        on_sale: price.on_sale,
                    })
                    .collect(),
            })
            .collect(),
    }
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
    selected_handle: Option<String>,
    locale: Option<String>,
) -> Result<StorefrontPricingData, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::loco::transactional_event_bus_from_context;
        use rustok_pricing::PricingService;

        let app_ctx = expect_context::<AppContext>();
        let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
            .await
            .ok();
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(ServerFnError::new)?;
        let requested_locale = locale
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| request_context.as_ref().map(|ctx| ctx.locale.clone()))
            .unwrap_or_else(|| tenant.default_locale.clone());
        let public_channel_slug = request_context
            .as_ref()
            .and_then(|ctx| normalize_public_channel_slug(ctx.channel_slug.as_deref()));

        let service = PricingService::new(
            app_ctx.db.clone(),
            transactional_event_bus_from_context(&app_ctx),
        );
        let products = service
            .list_published_product_pricing_with_locale_fallback(
                tenant.id,
                requested_locale.as_str(),
                Some(tenant.default_locale.as_str()),
                public_channel_slug.as_deref(),
                1,
                8,
            )
            .await
            .map_err(ServerFnError::new)?;
        let resolved_handle = selected_handle
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .or_else(|| products.items.first().map(|item| item.handle.clone()));
        let selected_product = if let Some(handle) = resolved_handle.clone() {
            service
                .get_published_product_pricing_by_handle_with_locale_fallback(
                    tenant.id,
                    handle.as_str(),
                    requested_locale.as_str(),
                    Some(tenant.default_locale.as_str()),
                    public_channel_slug.as_deref(),
                )
                .await
                .map_err(ServerFnError::new)?
                .map(map_native_detail)
        } else {
            None
        };

        Ok(StorefrontPricingData {
            products: map_native_list(products),
            selected_product,
            selected_handle: resolved_handle,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (selected_handle, locale);
        Err(ServerFnError::new(
            "pricing/storefront-data requires the `ssr` feature",
        ))
    }
}
